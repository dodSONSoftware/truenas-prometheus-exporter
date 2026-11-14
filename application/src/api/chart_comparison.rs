//! Explicit one-shot diagnostic. The scheduled exporter never calls this module.
use super::{
    client::{Socket, open_authenticated, read_system},
    protocol::{self, Incoming},
};
use crate::{collectors::realtime::Frame, config::ApiConfig, state::Reason};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::{Instant, timeout, timeout_at};
use tokio_tungstenite::tungstenite::Message;
const COLLECTION: &str = "reporting.realtime:{\"interval\":2}";
const GRAPHS: [(&str, &str, &str); 8] = [
    ("demanddatahitpercentage", "ddh_p", "ddh"),
    ("demandmetadatahitpercentage", "dmh_p", "dmh"),
    ("demanddatahitspersecond", "ddhit", "ddhit"),
    ("demanddataiohitspersecond", "ddioh", "ddioh"),
    ("demanddatamissespersecond", "ddmis", "ddmis"),
    ("demandmetadatahitspersecond", "dmhit", "dmhit"),
    ("demandmetadataiohitspersecond", "dmioh", "dmioh"),
    ("demandmetadatamissespersecond", "dmmis", "dmmis"),
];
struct Session {
    socket: Socket,
    calls: usize,
    chart_bytes: usize,
    owned: Option<String>,
    events: Vec<Value>,
    charts: Vec<Value>,
    interface: Option<String>,
    start: Instant,
    epoch: f64,
}
impl Session {
    async fn text(&mut self) -> Result<String, Reason> {
        loop {
            match self
                .socket
                .next()
                .await
                .ok_or(Reason::Transport)?
                .map_err(|_| Reason::Transport)?
            {
                Message::Text(text) if text.len() <= 1_048_576 => return Ok(text.to_string()),
                Message::Ping(bytes) => self
                    .socket
                    .send(Message::Pong(bytes))
                    .await
                    .map_err(|_| Reason::Transport)?,
                Message::Pong(_) => {}
                _ => return Err(Reason::Protocol),
            }
        }
    }
    fn event(&mut self, text: &str, retain: bool) -> Result<(), Reason> {
        let v: Value = serde_json::from_str(text).map_err(|_| Reason::Protocol)?;
        if v["method"] != "collection_update"
            || v["params"]["collection"] != COLLECTION
            || v["params"]["msg"] != "added"
        {
            return Err(Reason::Protocol);
        }
        let raw = v["params"]["fields"].to_string();
        if raw.len() > 131_072 {
            return Err(Reason::Schema);
        }
        let f = Frame::parse(&raw);
        let cpu = f.cpu?;
        let memory = f.memory?;
        let arc = f.arc?;
        let network = f.network?;
        if network.len() != 1 {
            return Err(Reason::Schema);
        }
        let n = &network[0];
        if self.interface.as_ref().is_some_and(|i| i != &n.name) {
            return Err(Reason::Schema);
        }
        self.interface = Some(n.name.clone());
        if retain && self.events.len() < 30 {
            self.events.push(
                json!({
"receipt_epoch":self.epoch+self.start.elapsed().as_secs_f64(),
                "cpu":cpu.iter().map(|c|json!({
"cpu":c.name,"ratio":c.ratio
})).collect::<Vec<_>>(),
                "available_bytes":memory.available,"arc_size_bytes":arc.size,
                "arc_data_hit_ratio":arc.data_hit,"arc_metadata_hit_ratio":arc.metadata_hit,
                "interface":"interface-0","up":n.up,"bits_per_second":n.speed,
                "rx_bytes_per_second":n.receive,"tx_bytes_per_second":n.transmit
}),
            );
        }
        Ok(())
    }
    async fn call(&mut self, method: &str, params: Value, chart: bool) -> Result<Value, Reason> {
        if self.calls >= 13 {
            return Err(Reason::Protocol);
        }
        self.calls += 1;
        let id = format!("comparison-{}", self.calls);
        timeout(Duration::from_secs(10), async {
            self.socket
                .send(Message::text(
                    json!({
                    "jsonrpc":"2.0","id":id,"method":method,"params":params
                    })
                    .to_string(),
                ))
                .await
                .map_err(|_| Reason::Transport)?;
            loop {
                let text = self.text().await?;
                match protocol::decode(&text)? {
                    Incoming::Response { id: got, result } if got == id => {
                        if chart {
                            self.chart_bytes += text.len();
                            if self.chart_bytes > 9 * 1_048_576 {
                                return Err(Reason::Schema);
                            }
                        }
                        return result;
                    }
                    Incoming::Notification => {
                        let notice: Value =
                            serde_json::from_str(&text).map_err(|_| Reason::Protocol)?;
                        if method == "core.unsubscribe"
                            && notice["method"] == "notify_unsubscribed"
                            && notice["params"]["collection"] == COLLECTION
                            && notice["params"]["error"].is_null()
                        {
                            continue;
                        }
                        self.event(&text, self.owned.is_none())?;
                    }
                    _ => return Err(Reason::Protocol),
                }
            }
        })
        .await
        .map_err(|_| Reason::Timeout)?
    }
    async fn acquire(&mut self) -> Result<(), Reason> {
        let id = self
            .call("core.subscribe", json!([COLLECTION]), false)
            .await?;
        let id = id
            .as_str()
            .filter(|id| {
                !id.is_empty()
                    && id.len() <= 128
                    && !id.chars().any(|c| c.is_control() || c.is_whitespace())
            })
            .ok_or(Reason::Protocol)?;
        self.owned = Some(id.to_string());
        while self.events.len() < 30 {
            let text = self.text().await?;
            self.event(&text, true)?;
        }
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Reason> {
        if let Some(id) = self.owned.take() {
            let reply = timeout(
                Duration::from_secs(5),
                self.call("core.unsubscribe", json!([id]), false),
            )
            .await
            .map_err(|_| Reason::Timeout)??;
            if !reply.is_null() {
                return Err(Reason::Protocol);
            }
        }
        Ok(())
    }
    async fn charts(&mut self) -> Result<(), Reason> {
        let a = self
            .events
            .first()
            .and_then(|v| v["receipt_epoch"].as_f64())
            .ok_or(Reason::Schema)?
            .floor() as u64
            + 2;
        let b = self
            .events
            .last()
            .and_then(|v| v["receipt_epoch"].as_f64())
            .ok_or(Reason::Schema)?
            .floor() as u64
            - 2;
        if b <= a || b - a > 60 {
            return Err(Reason::Schema);
        }
        let query = json!({
        "start":a,"end":b,"aggregate":true
        });
        let interface = self.interface.clone().ok_or(Reason::Schema)?;
        let result = self
            .call(
                "reporting.netdata_get_data",
                json!([[{
"name":"cpu","identifier":null
},{
"name":"memory","identifier":null
},{
"name":"interface","identifier":interface
},{
"name":"arcsize","identifier":null
}],query]),
                true,
            )
            .await?;
        self.charts = validate_charts(
            result,
            &["cpu", "memory", "interface", "arcsize"],
            a,
            b,
            &interface,
        )?;
        for (name, _, _) in GRAPHS {
            let result = self
                .call("reporting.netdata_graph", json!([name, query]), true)
                .await?;
            self.charts
                .extend(validate_charts(result, &[name], a, b, &interface)?);
        }
        Ok(())
    }
}
fn validate_charts(
    value: Value,
    names: &[&str],
    a: u64,
    b: u64,
    interface: &str,
) -> Result<Vec<Value>, Reason> {
    let charts = value.as_array().ok_or(Reason::Schema)?;
    if charts.len() != names.len() {
        return Err(Reason::Schema);
    }
    let mut seen = std::collections::HashSet::new();
    charts
        .iter()
        .map(|c| {
            let name = c["name"].as_str().ok_or(Reason::Schema)?;
            if !names.contains(&name) || !seen.insert(name) || c["start"] != a || c["end"] != b {
                return Err(Reason::Schema);
            }
            let identifier = c["identifier"].as_str().ok_or(Reason::Schema)?;
            if identifier != if name == "interface" { interface } else { name } {
                return Err(Reason::Schema);
            }
            let legend = c["legend"].as_array().ok_or(Reason::Schema)?;
            if legend.len() < 2 || legend.len() > 32 || legend[0] != "time" {
                return Err(Reason::Schema);
            }
            let mut cols = std::collections::HashSet::new();
            for col in &legend[1..] {
                let col = col.as_str().ok_or(Reason::Schema)?;
                let allowed = match name {
                    "cpu" => {
                        col == "cpu"
                            || col
                                .strip_prefix("cpu")
                                .and_then(|n| n.parse::<u16>().ok())
                                .is_some_and(|n| n < 256 && col == format!("cpu{n}"))
                    }
                    "memory" => col == "available",
                    "arcsize" => col == "size",
                    "interface" => matches!(col, "received" | "sent"),
                    _ => GRAPHS
                        .iter()
                        .any(|(n, id, label)| *n == name && (col == *id || col == *label)),
                };
                if !allowed || !cols.insert(col) {
                    return Err(Reason::Schema);
                }
            }
            let data = c["data"].as_array().ok_or(Reason::Schema)?;
            if data.is_empty() || data.len() > 300 {
                return Err(Reason::Schema);
            }
            let mut last = None;
            for row in data {
                let row = row.as_array().ok_or(Reason::Schema)?;
                if row.len() != legend.len() {
                    return Err(Reason::Schema);
                }
                let t = row[0].as_f64().ok_or(Reason::Schema)?;
                if !t.is_finite() || t < a as f64 || t > b as f64 || last.is_some_and(|v| t <= v) {
                    return Err(Reason::Schema);
                }
                last = Some(t);
                if row[1..]
                    .iter()
                    .any(|n| !n.is_null() && !n.as_f64().is_some_and(f64::is_finite))
                {
                    return Err(Reason::Schema);
                }
            }
            Ok(json!({
            "name":name,"identifier":if name=="interface"{
            "interface-0"
            }else{
            name
            },"start":a,"end":b,"legend":legend,"data":data
            }))
        })
        .collect()
}
/// At most 13 fixed diagnostic RPCs; sanitized partial evidence is returned on failure.
pub async fn run(api: ApiConfig) -> Result<Value, Reason> {
    timeout(Duration::from_secs(180), observe(api))
        .await
        .map_err(|_| Reason::Timeout)?
}

async fn observe(api: ApiConfig) -> Result<Value, Reason> {
    let start = Instant::now();
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Reason::Unavailable)?
        .as_secs_f64();
    let mut socket = timeout(Duration::from_secs(10), open_authenticated(&api))
        .await
        .map_err(|_| Reason::Timeout)??;
    let info = timeout(
        Duration::from_secs(10),
        read_system(&mut socket, "comparison-system"),
    )
    .await
    .map_err(|_| Reason::Timeout)??;
    if info.version != "25.04.2.6" {
        return Err(Reason::Unsupported);
    }
    let mut session = Session {
        socket,
        calls: 2,
        chart_bytes: 0,
        owned: None,
        events: Vec::new(),
        charts: Vec::new(),
        interface: None,
        start,
        epoch,
    };
    let acquired = timeout(Duration::from_secs(70), session.acquire())
        .await
        .map_err(|_| Reason::Timeout)
        .and_then(|r| r);
    let had_owned = session.owned.is_some();
    let cleanup = session.cleanup().await;
    let cleanup_acknowledged = had_owned && cleanup.is_ok();
    let result = match (acquired, cleanup) {
        (Ok(()), Ok(())) => timeout_at(start + Duration::from_secs(175), session.charts())
            .await
            .map_err(|_| Reason::Timeout)
            .and_then(|r| r),
        (Err(error), _) | (_, Err(error)) => Err(error),
    };
    let error = result.err().map(|e| e.as_str());
    let closed = timeout(Duration::from_secs(5), session.socket.close(None))
        .await
        .is_ok_and(|r| r.is_ok());
    Ok(json!({
        "version": info.version,
        "physical_total_bytes": info.physmem,
        "rpc_calls": session.calls,
        "chart_reply_bytes": session.chart_bytes,
        "elapsed_seconds": start.elapsed().as_secs_f64(),
        "socket_closed": closed,
        "error": error,
        "events": session.events,
        "charts": session.charts,
        "cleanup_acknowledged": cleanup_acknowledged
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Credentials;
    use std::sync::Arc;
    use tokio::net::{TcpListener, TcpStream};
    use tokio_tungstenite::{WebSocketStream, accept_async};
    type Server = WebSocketStream<tokio_rustls::server::TlsStream<TcpStream>>;
    async fn request(socket: &mut Server, method: &str) -> Value {
        let text = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["method"], method);
        value
    }
    async fn reply(socket: &mut Server, request: &Value, result: Value) {
        socket
            .send(Message::text(
                json!({
                "jsonrpc":"2.0","id":request["id"],"result":result
                })
                .to_string(),
            ))
            .await
            .unwrap();
    }
    fn chart(name: &str, column: &str, a: u64, b: u64) -> Value {
        json!({
        "name":name,"identifier":if name=="interface"{
        "synthetic0"
        }else{
        name
        },"start":a,"end":b,"legend":["time",column],"data":[[a,1],[b,null]]
        })
    }
    async fn fixture(invalid: bool) -> (ApiConfig, tokio::task::JoinHandle<()>) {
        let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
        let mut roots = rustls::RootCertStore::empty();
        roots.add(cert.cert.der().clone()).unwrap();
        let tls = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![cert.cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()).into(),
            )
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = ApiConfig {
            url: format!("wss://{}/api/current", listener.local_addr().unwrap())
                .parse()
                .unwrap(),
            credentials: Credentials {
                username: "synthetic".into(),
                key: "synthetic".into(),
            },
            tls: Arc::new(
                rustls::ClientConfig::builder()
                    .with_root_certificates(roots)
                    .with_no_client_auth(),
            ),
            timeout: Duration::from_secs(10),
        };
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let stream = tokio_rustls::TlsAcceptor::from(Arc::new(tls))
                .accept(stream)
                .await
                .unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let req = request(&mut socket, "auth.login_ex").await;
            reply(
                &mut socket,
                &req,
                json!({
                "response_type":"SUCCESS"
                }),
            )
            .await;
            let req = request(&mut socket, "system.info").await;
            reply(
                &mut socket,
                &req,
                json!({
                "version":"25.04.2.6","hostname":"synthetic","uptime_seconds":10,"physmem":100
                }),
            )
            .await;
            let req = request(&mut socket, "core.subscribe").await;
            assert_eq!(req["params"], json!([COLLECTION]));
            reply(&mut socket, &req, json!("owned-synthetic")).await;
            let mut fields: Value =
                serde_json::from_str(include_str!("../../tests/fixtures/realtime.synthetic.json"))
                    .unwrap();
            // Use a genuine-shape fixture with synthetic measurement values.
            if fields.get("cpu").is_none() {
                fields = fields["params"]["fields"].clone();
            }
            fields["interfaces"]
                .as_object_mut()
                .unwrap()
                .remove("synthetic1");
            if invalid {
                fields["interfaces"] = json!({});
            }
            let count = if invalid { 1 } else { 30 };
            for _ in 0..count {
                socket
                    .send(Message::text(
                        json!({
                        "jsonrpc":"2.0","method":"collection_update","params":{
                        "collection":COLLECTION,"msg":"added","fields":fields
                        }
                        })
                        .to_string(),
                    ))
                    .await
                    .unwrap();
                if !invalid {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
            }
            let req = request(&mut socket, "core.unsubscribe").await;
            assert_eq!(req["params"], json!(["owned-synthetic"]));
            socket
                .send(Message::text(
                    json!({
                        "jsonrpc":"2.0", "method":"notify_unsubscribed",
                        "params":{"collection":COLLECTION,"error":null}
                    })
                    .to_string(),
                ))
                .await
                .unwrap();
            reply(&mut socket, &req, Value::Null).await;
            if !invalid {
                let req = request(&mut socket, "reporting.netdata_get_data").await;
                assert_eq!(req["params"][0].as_array().unwrap().len(), 4);
                let a = req["params"][1]["start"].as_u64().unwrap();
                let b = req["params"][1]["end"].as_u64().unwrap();
                assert!(b > a && b - a <= 60);
                reply(
                    &mut socket,
                    &req,
                    json!([
                        chart("cpu", "cpu", a, b),
                        chart("memory", "available", a, b),
                        chart("interface", "received", a, b),
                        chart("arcsize", "size", a, b)
                    ]),
                )
                .await;
                for (name, column, _) in GRAPHS {
                    let req = request(&mut socket, "reporting.netdata_graph").await;
                    assert_eq!(
                        req["params"],
                        json!([name,{
                        "start":a,"end":b,"aggregate":true
                        }])
                    );
                    reply(&mut socket, &req, json!([chart(name, column, a, b)])).await;
                }
            }
            assert!(matches!(socket.next().await, Some(Ok(Message::Close(_)))));
        });
        (api, task)
    }
    #[tokio::test]
    async fn loopback_executes_thirteen_calls_and_sanitizes() {
        let (api, server) = fixture(false).await;
        let report = run(api).await.unwrap();
        server.await.unwrap();
        assert!(report["error"].is_null(), "{report}");
        assert_eq!(report["rpc_calls"], 13);
        assert_eq!(report["charts"].as_array().unwrap().len(), 12);
        assert_eq!(report["events"].as_array().unwrap().len(), 30);
        assert_eq!(report["cleanup_acknowledged"], true);
        assert!(!report.to_string().contains("synthetic0"));
        assert!(!report.to_string().contains("owned-synthetic"));
    }
    #[tokio::test]
    async fn invalid_event_unsubscribes_and_skips_charts() {
        let (api, server) = fixture(true).await;
        let report = run(api).await.unwrap();
        server.await.unwrap();
        assert_eq!(report["rpc_calls"], 4);
        assert_eq!(report["error"], "schema");
        assert_eq!(report["cleanup_acknowledged"], true);
    }
    #[test]
    fn rejects_unbounded_or_ambiguous_chart_data() {
        let valid = chart("cpu", "cpu", 10, 20);
        assert!(validate_charts(json!([valid.clone()]), &["cpu"], 10, 20, "synthetic0").is_ok());
        for value in [
            json!([]),
            json!([valid.clone(), valid.clone()]),
            json!([{
             "name":"unknown"
            }]),
        ] {
            assert!(validate_charts(value, &["cpu"], 10, 20, "synthetic0").is_err());
        }
        for (key, value) in [
            ("data", json!([])),
            ("data", json!([[9, 1]])),
            ("data", json!([[10, 1], [10, 2]])),
            ("data", json!(vec![json!([10, 1]); 301])),
            ("legend", json!(["time", "secret-path"])),
        ] {
            let mut v = valid.clone();
            v[key] = value;
            assert!(validate_charts(json!([v]), &["cpu"], 10, 20, "synthetic0").is_err());
        }
    }
}
