#![cfg(unix)]
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    process::Command,
    task::JoinSet,
    time::{sleep, timeout},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

async fn get(address: std::net::SocketAddr, path: &str) -> String {
    let mut socket = TcpStream::connect(address).await.unwrap();
    socket
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .unwrap();
    let mut bytes = Vec::new();
    socket.read_to_end(&mut bytes).await.unwrap();
    String::from_utf8(bytes).unwrap()
}
#[tokio::test]
async fn daemon_uses_isolated_realtime_socket_and_closes_owned_subscription_on_sigterm() {
    let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let tls = Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![cert.cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()).into(),
            )
            .unwrap(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("wss://{}/api/current", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut workers = JoinSet::new();
        for _ in 0..2 {
            let (tcp, _) = listener.accept().await.unwrap();
            let tls = tls.clone();
            workers.spawn(async move {
                let stream=tokio_rustls::TlsAcceptor::from(tls).accept(tcp).await.unwrap();let mut socket=accept_async(stream).await.unwrap();
                let mut subscription=None;let mut ticks=tokio::time::interval(Duration::from_secs(2));let mut owned_cleanup=false;
                loop {
                    tokio::select! {
                        _=ticks.tick(),if subscription.is_some()=>{
                            let mut event:Value=serde_json::from_str(include_str!("fixtures/realtime.synthetic.json")).unwrap();
                            event["params"]["collection"]=json!(subscription.as_ref().unwrap());
                            if socket.send(Message::text(event.to_string())).await.is_err(){break;}
                        }
                        message=socket.next()=>match message {
                            Some(Ok(Message::Ping(bytes)))=>{let _=socket.send(Message::Pong(bytes)).await;},
                            Some(Ok(Message::Text(text)))=>{
                                let request:Value=serde_json::from_str(&text).unwrap();
                                let result=match request["method"].as_str().unwrap() {
                                    "auth.login_ex"=>{assert_eq!(request["params"][0]["api_key"],"SYNTHETIC_RT_SECRET");json!({"response_type":"SUCCESS"})},
                                    "system.info"=>serde_json::from_str::<Value>(include_str!("../../fixtures/truenas-25.04.2.6/system.info.host.response.json")).unwrap()["result"].clone(),
                                    "pool.query"=>{assert!(subscription.is_none());json!([{"name":"synthetic-pool","size":1024}])},
                                    "core.subscribe"=>{assert!(subscription.is_none());assert_eq!(request["params"],json!(["reporting.realtime:{\"interval\":2}"]));subscription=Some(request["params"][0].as_str().unwrap().to_string());json!("SYNTHETIC_OWNED_SUBSCRIPTION")},
                                    "core.unsubscribe"=>{assert!(subscription.is_some());assert_eq!(request["params"],json!(["SYNTHETIC_OWNED_SUBSCRIPTION"]));subscription=None;owned_cleanup=true;Value::Null},
                                    _=>panic!("unexpected API method"),
                                };
                                if socket.send(Message::text(json!({"jsonrpc":"2.0","id":request["id"],"result":result}).to_string())).await.is_err(){break;}
                            },
                            _=>break,
                        }
                    }
                }
                owned_cleanup
            });
        }
        let mut cleanup = false;
        while let Some(result) = workers.join_next().await {
            cleanup |= result.unwrap();
        }
        assert!(cleanup);
    });
    let folder = tempfile::tempdir().unwrap();
    let ca = folder.path().join("ca.pem");
    let key = folder.path().join("key");
    std::fs::write(&ca, cert.cert.pem()).unwrap();
    std::fs::write(&key, "SYNTHETIC_RT_SECRET").unwrap();
    let reserved = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = reserved.local_addr().unwrap();
    drop(reserved);
    let mut child = Command::new(env!("CARGO_BIN_EXE_truenas-exporter"))
        .env_clear()
        .env("TRUENAS_WS_URL", url)
        .env("TRUENAS_USERNAME", "synthetic-user")
        .env("TRUENAS_API_KEY_FILE", key)
        .env("TRUENAS_CA_CERT_FILE", ca)
        .env("TRUENAS_SYSTEM_ENABLED", "true")
        .env("TRUENAS_REALTIME_ENABLED", "true")
        .env("TRUENAS_REALTIME_TTL_SECONDS", "3")
        .env("TRUENAS_POLL_INTERVAL_SECONDS", "1")
        .env("TRUENAS_API_TIMEOUT_SECONDS", "1")
        .env("TRUENAS_CACHE_TTL_SECONDS", "3")
        .env("EXPORTER_LISTEN", address.to_string())
        .env("RUST_LOG", "trace")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    timeout(Duration::from_secs(6), async {
        while TcpStream::connect(address).await.is_err() {
            assert!(child.try_wait().unwrap().is_none());
            sleep(Duration::from_millis(20)).await;
        }
        loop {
            if get(address, "/readyz").await.starts_with("HTTP/1.1 200") {
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let metrics = get(address, "/metrics").await;
    for expected in [
        "truenas_realtime_subscribed 1",
        "truenas_api_compatible 1",
        "truenas_cpu_utilization_ratio{cpu=\"cpu\"} 0.25",
        "truenas_memory_available_bytes 1073741824",
        "truenas_zfs_arc_size_bytes 536870912",
        "truenas_network_receive_bytes_per_second{interface=\"synthetic0\"} 1024",
        "truenas_pool_size_bytes{pool=\"synthetic-pool\"} 1024",
    ] {
        assert!(metrics.contains(expected), "missing {expected}");
    }
    assert_eq!(
        metrics
            .matches("# HELP truenas_memory_total_bytes ")
            .count(),
        1
    );
    assert!(
        !metrics.contains("truenas_network_receive_bytes_per_second{interface=\"synthetic1\"}")
    );
    assert!(
        Command::new("/bin/kill")
            .args(["-TERM", &child.id().unwrap().to_string()])
            .status()
            .await
            .unwrap()
            .success()
    );
    let output = timeout(Duration::from_secs(3), child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    assert!(output.status.success());
    let logs = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for private in [
        "SYNTHETIC_RT_SECRET",
        "SYNTHETIC_OWNED_SUBSCRIPTION",
        "captured-host-1",
        "core.subscribe",
        "core.unsubscribe",
    ] {
        assert!(!logs.contains(private));
    }
    timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
}
