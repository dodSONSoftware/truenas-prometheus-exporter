use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{
    sync::{Arc, OnceLock, atomic::Ordering},
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    time::{sleep, timeout},
};
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use truenas_exporter::{
    api::{
        client::Client,
        protocol::{self, Incoming, Method},
    },
    config::{ApiConfig, Credentials, tls_config},
    state::{Reason, State},
};

type Socket = WebSocketStream<tokio_rustls::server::TlsStream<TcpStream>>;
struct FixtureTls {
    pem: String,
    server: Arc<rustls::ServerConfig>,
    client: Arc<rustls::ClientConfig>,
}
fn fixture_tls() -> &'static FixtureTls {
    static TLS: OnceLock<FixtureTls> = OnceLock::new();
    TLS.get_or_init(|| {
        let certificate = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
        let mut roots = rustls::RootCertStore::empty();
        roots.add(certificate.cert.der().clone()).unwrap();
        let server = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.key_pair.serialize_der())
                    .into(),
            )
            .unwrap();
        FixtureTls {
            pem: certificate.cert.pem(),
            server: Arc::new(server),
            client: Arc::new(
                rustls::ClientConfig::builder()
                    .with_root_certificates(roots)
                    .with_no_client_auth(),
            ),
        }
    })
}
struct FixtureListener(TcpListener);
impl FixtureListener {
    async fn accept(
        &self,
    ) -> std::io::Result<(
        tokio_rustls::server::TlsStream<TcpStream>,
        std::net::SocketAddr,
    )> {
        let (tcp, address) = self.0.accept().await?;
        let tls = tokio_rustls::TlsAcceptor::from(fixture_tls().server.clone())
            .accept(tcp)
            .await?;
        Ok((tls, address))
    }
}
fn config(url: String, deadline: Duration) -> ApiConfig {
    ApiConfig {
        url: url.parse().unwrap(),
        credentials: Credentials {
            username: "test-user".into(),
            key: "synthetic-key".into(),
        },
        tls: fixture_tls().client.clone(),
        timeout: deadline,
    }
}
async fn listen() -> (FixtureListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("wss://{}/api/current", listener.local_addr().unwrap());
    (FixtureListener(listener), url)
}
async fn text(socket: &mut Socket) -> Value {
    loop {
        match socket.next().await.unwrap().unwrap() {
            Message::Text(s) => return serde_json::from_str(&s).unwrap(),
            Message::Ping(bytes) => socket.send(Message::Pong(bytes)).await.unwrap(),
            _ => {}
        }
    }
}
async fn login(listener: &FixtureListener) -> Socket {
    let (stream, _) = listener.accept().await.unwrap();
    let mut socket = accept_async(stream).await.unwrap();
    let request = text(&mut socket).await;
    assert_eq!(request["method"], "auth.login_ex");
    assert_eq!(request["params"][0]["mechanism"], "API_KEY_PLAIN");
    assert_eq!(request["params"][0]["username"], "test-user");
    assert_eq!(request["params"][0]["api_key"], "synthetic-key");
    assert_eq!(request["params"][0]["login_options"]["user_info"], false);
    socket
        .send(Message::text(
            json!({"jsonrpc":"2.0", "id":request["id"], "result":{"response_type":"SUCCESS"}})
                .to_string(),
        ))
        .await
        .unwrap();
    socket
}
async fn ready(state: &State) {
    timeout(Duration::from_secs(4), async {
        while !state.connected.load(Ordering::Relaxed) {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn concurrent_out_of_order_unknown_ids_permission_and_unsupported() {
    let (listener, url) = listen().await;
    let server = tokio::spawn(async move {
        let mut ws = login(&listener).await;
        let first = text(&mut ws).await;
        let second = text(&mut ws).await;
        assert_ne!(first["id"], second["id"]);
        assert_eq!(first["params"], json!([]));
        assert_eq!(first["method"], "pool.query");
        ws.send(Message::text(
            json!({"jsonrpc":"2.0","id":"unknown","result":[]}).to_string(),
        ))
        .await
        .unwrap();
        for (request, value) in [
            (second, json!([{"name":"second"}])),
            (first, json!([{"name":"first"}])),
        ] {
            ws.send(Message::text(
                json!({"jsonrpc":"2.0","id":request["id"],"result":value}).to_string(),
            ))
            .await
            .unwrap();
        }
        for error in [
            json!({"code":-32001,"message":"sensitive","data":{"error":13}}),
            json!({"code":-32601,"message":"not supported"}),
        ] {
            let request = text(&mut ws).await;
            ws.send(Message::text(
                json!({"jsonrpc":"2.0","id":request["id"],"error":error}).to_string(),
            ))
            .await
            .unwrap();
        }
        // Keep the connection open until the client cancels.
        while let Some(Ok(message)) = ws.next().await {
            if let Message::Ping(bytes) = message {
                let _ = ws.send(Message::Pong(bytes)).await;
            }
        }
    });
    let state = State::new(Duration::from_secs(180), false);
    let stop = CancellationToken::new();
    let (client, task) = Client::spawn(
        config(url, Duration::from_secs(2)),
        state.clone(),
        stop.clone(),
    );
    ready(&state).await;
    let (first, second) = tokio::join!(client.pool_query(), client.pool_query());
    assert_eq!(first.unwrap()[0]["name"], "first");
    assert_eq!(second.unwrap()[0]["name"], "second");
    assert_eq!(client.pool_query().await.unwrap_err(), Reason::Permission);
    assert_eq!(client.pool_query().await.unwrap_err(), Reason::Unsupported);
    stop.cancel();
    timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
    assert!(!state.connected.load(Ordering::Relaxed));
    timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn connection_loss_reauthenticates_and_recovers() {
    let (listener, url) = listen().await;
    let server = tokio::spawn(async move {
        let mut first = login(&listener).await;
        let _ = text(&mut first).await;
        first.close(None).await.unwrap();
        drop(first);
        let mut second = login(&listener).await;
        let request = text(&mut second).await;
        second
            .send(Message::text(
                json!({"jsonrpc":"2.0","id":request["id"],"result":[]}).to_string(),
            ))
            .await
            .unwrap();
        while let Some(Ok(_)) = second.next().await {}
    });
    let state = State::new(Duration::from_secs(180), false);
    let stop = CancellationToken::new();
    let (client, task) = Client::spawn(
        config(url, Duration::from_secs(2)),
        state.clone(),
        stop.clone(),
    );
    ready(&state).await;
    assert!(client.pool_query().await.is_err());
    timeout(Duration::from_secs(1), async {
        while state.connected.load(Ordering::Relaxed) {
            sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    ready(&state).await;
    assert!(
        client
            .pool_query()
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    stop.cancel();
    task.await.unwrap();
    timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn malformed_and_stalled_servers_report_distinct_errors() {
    for (payload, expected) in [
        (Some("not-json"), Reason::Protocol),
        (None, Reason::Timeout),
    ] {
        let (listener, url) = listen().await;
        let server = tokio::spawn(async move {
            let mut ws = login(&listener).await;
            let _ = text(&mut ws).await;
            if let Some(payload) = payload {
                let _ = ws.send(Message::text(payload)).await;
            }
            sleep(Duration::from_secs(3)).await;
        });
        let state = State::new(Duration::from_secs(180), false);
        let stop = CancellationToken::new();
        let (client, task) = Client::spawn(
            config(url, Duration::from_millis(300)),
            state.clone(),
            stop.clone(),
        );
        ready(&state).await;
        let result = timeout(Duration::from_secs(1), client.pool_query())
            .await
            .unwrap();
        assert_eq!(result, Err(expected));
        stop.cancel();
        timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
        server.abort();
        let _ = server.await;
    }
}

const WIRE_LIMIT: usize = 1024 * 1024;

#[tokio::test]
async fn websocket_response_size_boundaries() {
    use tokio_tungstenite::tungstenite::protocol::frame::{
        Frame,
        coding::{Data, OpCode},
    };

    for fragmented in [false, true] {
        for length in [WIRE_LIMIT - 1, WIRE_LIMIT, WIRE_LIMIT + 1] {
            let (listener, url) = listen().await;
            let server = tokio::spawn(async move {
                let mut ws = login(&listener).await;
                let request = text(&mut ws).await;
                let mut bytes = json!({"jsonrpc":"2.0","id":request["id"],"result":[]})
                    .to_string()
                    .into_bytes();
                bytes.resize(length, b' '); // Trailing whitespace keeps the JSON valid.
                if fragmented {
                    let tail = bytes.split_off(length / 2);
                    ws.send(Message::Frame(Frame::message(
                        bytes,
                        OpCode::Data(Data::Text),
                        false,
                    )))
                    .await
                    .unwrap();
                    ws.send(Message::Frame(Frame::message(
                        tail,
                        OpCode::Data(Data::Continue),
                        true,
                    )))
                    .await
                    .unwrap();
                } else {
                    // The receiver may reject the oversized frame as soon as it reads its header.
                    let sent = ws
                        .send(Message::text(String::from_utf8(bytes).unwrap()))
                        .await;
                    if length <= WIRE_LIMIT {
                        sent.unwrap();
                    }
                }
                while let Some(Ok(_)) = ws.next().await {}
            });
            let state = State::new(Duration::from_secs(180), false);
            let stop = CancellationToken::new();
            let (client, task) = Client::spawn(
                config(url, Duration::from_secs(3)),
                state.clone(),
                stop.clone(),
            );
            ready(&state).await;
            let expected = if length <= WIRE_LIMIT {
                Ok(json!([]))
            } else {
                Err(Reason::Protocol)
            };
            assert_eq!(
                client.pool_query().await,
                expected,
                "fragmented={fragmented}, length={length}"
            );
            stop.cancel();
            task.await.unwrap();
            timeout(Duration::from_secs(1), server)
                .await
                .unwrap()
                .unwrap();
        }
    }
}

#[tokio::test]
async fn oversized_frame_header_is_rejected_before_payload() {
    use tokio::io::AsyncWriteExt;

    let (listener, url) = listen().await;
    let server = tokio::spawn(async move {
        let mut ws = login(&listener).await;
        let _request = text(&mut ws).await;
        // Final unmasked text frame with a 64-bit payload length. Withhold the body
        // so the aggregate message limit cannot mask a missing frame limit.
        let mut header = vec![0x81, 127];
        header.extend_from_slice(&((WIRE_LIMIT + 1) as u64).to_be_bytes());
        ws.get_mut().write_all(&header).await.unwrap();
        while let Some(Ok(_)) = ws.next().await {}
    });
    let state = State::new(Duration::from_secs(180), false);
    let stop = CancellationToken::new();
    let (client, task) = Client::spawn(
        config(url, Duration::from_secs(2)),
        state.clone(),
        stop.clone(),
    );
    ready(&state).await;
    assert_eq!(client.pool_query().await, Err(Reason::Protocol));
    stop.cancel();
    task.await.unwrap();
    timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn auth_failures_remain_observable_and_shutdown_interrupts_backoff() {
    let (listener, url) = listen().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = accept_async(stream).await.unwrap();
        let request = text(&mut ws).await;
        ws.send(Message::text(
            json!({"jsonrpc":"2.0","id":request["id"],"result":{"response_type":"EXPIRED"}})
                .to_string(),
        ))
        .await
        .unwrap();
        sleep(Duration::from_secs(3)).await;
    });
    let state = State::new(Duration::from_secs(180), false);
    let stop = CancellationToken::new();
    let (_client, task) = Client::spawn(
        config(url, Duration::from_secs(1)),
        state.clone(),
        stop.clone(),
    );
    timeout(Duration::from_secs(1), async {
        while !state
            .metrics_at(tokio::time::Instant::now())
            .unwrap()
            .contains("reason=\"expired\"")
        {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(!state.connected.load(Ordering::Relaxed));
    stop.cancel();
    timeout(Duration::from_millis(200), task)
        .await
        .unwrap()
        .unwrap();
    server.abort();
    let _ = server.await;
}

#[test]
fn strict_protocol_envelopes_auth_variants_and_method_contract() {
    let credentials = Credentials {
        username: "test".into(),
        key: "synthetic".into(),
    };
    assert_eq!(
        protocol::login("a", &credentials)["params"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(Method::PoolQuery.request("p")["method"], "pool.query");
    for (variant, expected) in [
        ("SUCCESS", Ok(())),
        ("AUTH_ERR", Err(Reason::Auth)),
        ("EXPIRED", Err(Reason::Expired)),
        ("OTP_REQUIRED", Err(Reason::Otp)),
        ("REDIRECT", Err(Reason::Redirect)),
        ("unknown", Err(Reason::Protocol)),
    ] {
        assert_eq!(
            protocol::authenticate(json!({"response_type":variant})),
            expected
        );
    }
    for invalid in [
        "{}",
        "[]",
        "null",
        "{\"jsonrpc\":\"1.0\",\"id\":\"a\",\"result\":[]}",
        "{\"jsonrpc\":\"2.0\",\"id\":\"a\",\"result\":[],\"error\":{}}",
        "{\"jsonrpc\":\"2.0\",\"id\":null,\"result\":[]}",
    ] {
        assert!(protocol::decode(invalid).is_err());
    }
    assert!(matches!(
        protocol::decode("{\"jsonrpc\":\"2.0\",\"method\":\"collection_update\",\"params\":{}}"),
        Ok(Incoming::Notification)
    ));
}

#[tokio::test]
async fn verified_tls_accepts_private_ca_and_rejects_untrusted_or_wrong_name() {
    use rustls::ServerConfig;
    use rustls::pki_types::PrivatePkcs8KeyDer;
    use tokio_rustls::TlsAcceptor;
    for (trust, matching, expected) in [
        (true, true, true),
        (false, true, false),
        (true, false, false),
    ] {
        let san = if matching {
            "127.0.0.1"
        } else {
            "wrong.example.invalid"
        };
        let certificate = rcgen::generate_simple_self_signed(vec![san.to_owned()]).unwrap();
        let server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.cert.der().clone()],
                PrivatePkcs8KeyDer::from(certificate.key_pair.serialize_der()).into(),
            )
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut config = config(
            format!("wss://{}/api/current", listener.local_addr().unwrap()),
            Duration::from_secs(1),
        );
        let mut ca = tempfile::NamedTempFile::new().unwrap();
        if trust {
            use std::io::Write;
            ca.write_all(certificate.cert.pem().as_bytes()).unwrap();
            config.tls = tls_config(Some(ca.path())).unwrap();
        }
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            if let Ok(tls) = TlsAcceptor::from(Arc::new(server_config)).accept(tcp).await {
                let mut ws = accept_async(tls).await.unwrap();
                if let Some(Ok(Message::Text(request))) = ws.next().await {
                    let request: Value = serde_json::from_str(&request).unwrap();
                    ws.send(Message::text(json!({"jsonrpc":"2.0","id":request["id"],"result":{"response_type":"SUCCESS"}}).to_string())).await.unwrap();
                    while let Some(Ok(_)) = ws.next().await {}
                }
            }
        });
        let state = State::new(Duration::from_secs(180), false);
        let stop = CancellationToken::new();
        let (_client, task) = Client::spawn(config, state.clone(), stop.clone());
        if expected {
            ready(&state).await;
        } else {
            timeout(Duration::from_secs(2), async {
                while !state
                    .metrics_at(tokio::time::Instant::now())
                    .unwrap()
                    .contains("reason=\"tls\"")
                {
                    sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            assert!(!state.connected.load(Ordering::Relaxed));
        }
        stop.cancel();
        task.await.unwrap();
        timeout(Duration::from_secs(1), server)
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn authenticated_fixture_collection_expires_during_permission_failures() {
    use truenas_exporter::scheduler;
    let (listener, url) = listen().await;
    let server = tokio::spawn(async move {
        let mut ws = login(&listener).await;
        let request = text(&mut ws).await;
        let fixture: Value =
            serde_json::from_str(include_str!("fixtures/pools.synthetic.json")).unwrap();
        ws.send(Message::text(
            json!({"jsonrpc":"2.0","id":request["id"],"result":fixture}).to_string(),
        ))
        .await
        .unwrap();
        while let Some(Ok(message)) = ws.next().await {
            match message {
                Message::Ping(bytes) => {
                    let _ = ws.send(Message::Pong(bytes)).await;
                }
                Message::Text(text) => {
                    let request: Value = serde_json::from_str(&text).unwrap();
                    let _ = ws.send(Message::text(json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":-32001,"message":"redacted","data":{"error":13}}}).to_string())).await;
                }
                _ => {}
            }
        }
    });
    let state = State::new(Duration::from_millis(150), false);
    let stop = CancellationToken::new();
    let (client, task) = Client::spawn(
        config(url, Duration::from_secs(1)),
        state.clone(),
        stop.clone(),
    );
    ready(&state).await;
    let collector = tokio::spawn(scheduler::run(
        Some(client),
        state.clone(),
        Duration::from_millis(30),
        stop.clone(),
    ));
    timeout(Duration::from_secs(1), async {
        while !state.ready_at(tokio::time::Instant::now()) {
            sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        state
            .metrics_at(tokio::time::Instant::now())
            .unwrap()
            .contains("24000277250048")
    );
    timeout(Duration::from_secs(1), async {
        while !state
            .metrics_at(tokio::time::Instant::now())
            .unwrap()
            .contains("reason=\"permission\"")
        {
            sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    assert!(state.ready_at(tokio::time::Instant::now()));
    sleep(Duration::from_millis(160)).await;
    assert!(!state.ready_at(tokio::time::Instant::now()));
    let text = state.metrics_at(tokio::time::Instant::now()).unwrap();
    assert!(
        !text
            .lines()
            .any(|l| l.starts_with("truenas_pool_size_bytes{"))
    );
    assert!(text.contains("truenas_exporter_last_success_timestamp_seconds"));
    stop.cancel();
    collector.await.unwrap();
    task.await.unwrap();
    timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn in_flight_limit_and_request_deadlines_are_bounded() {
    let (listener, url) = listen().await;
    let server = tokio::spawn(async move {
        let mut ws = login(&listener).await;
        while let Some(Ok(message)) = ws.next().await {
            if let Message::Ping(bytes) = message {
                let _ = ws.send(Message::Pong(bytes)).await;
            }
            // Deliberately do not respond to queries.
        }
    });
    let state = State::new(Duration::from_secs(180), false);
    let stop = CancellationToken::new();
    let (client, task) = Client::spawn(
        config(url, Duration::from_millis(300)),
        state.clone(),
        stop.clone(),
    );
    ready(&state).await;
    let calls = (0..64).map(|_| client.pool_query());
    let results = timeout(
        Duration::from_secs(1),
        futures_util::future::join_all(calls),
    )
    .await
    .unwrap();
    assert!(results.iter().any(|r| matches!(r, Err(Reason::Busy))));
    assert!(results.iter().any(|r| matches!(r, Err(Reason::Timeout))));
    assert!(results.iter().all(Result::is_err));
    stop.cancel();
    task.await.unwrap();
    timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn shutdown_interrupts_authentication_wait() {
    let (listener, url) = listen().await;
    let (received, wait) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut ws = accept_async(tcp).await.unwrap();
        let _ = text(&mut ws).await;
        let _ = received.send(());
        while let Some(Ok(_)) = ws.next().await {}
    });
    let state = State::new(Duration::from_secs(180), false);
    let stop = CancellationToken::new();
    let (_client, task) = Client::spawn(config(url, Duration::from_secs(60)), state, stop.clone());
    timeout(Duration::from_secs(1), wait)
        .await
        .unwrap()
        .unwrap();
    stop.cancel();
    timeout(Duration::from_millis(200), task)
        .await
        .unwrap()
        .unwrap();
    timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn transport_check_closes_verified_websocket_without_authentication() {
    use rustls::{ServerConfig, pki_types::PrivatePkcs8KeyDer};
    use tokio_rustls::TlsAcceptor;
    use truenas_exporter::{api::client::check_transport, config::TransportConfig};
    let certificate = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![certificate.cert.der().clone()],
            PrivatePkcs8KeyDer::from(certificate.key_pair.serialize_der()).into(),
        )
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("wss://{}/api/current", listener.local_addr().unwrap());
    let ca = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(ca.path(), certificate.cert.pem()).unwrap();
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let tls = TlsAcceptor::from(Arc::new(server_config))
            .accept(tcp)
            .await
            .unwrap();
        let mut ws = accept_async(tls).await.unwrap();
        let first = timeout(Duration::from_secs(2), ws.next()).await.unwrap();
        assert!(matches!(first, Some(Ok(Message::Close(_)))));
    });
    let transport = TransportConfig {
        url: url.parse().unwrap(),
        tls: tls_config(Some(ca.path())).unwrap(),
        timeout: Duration::from_secs(2),
    };
    check_transport(transport).await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn pool_count_limit_preserves_snapshot_and_session() {
    let (listener, url) = listen().await;
    let server = tokio::spawn(async move {
        let mut ws = login(&listener).await;
        for count in [0, 1, 256, 257, 75_000, 1] {
            let request = text(&mut ws).await;
            let entries = (0..count)
                .map(|i| {
                    if count == 75_000 {
                        r#"{"name":"a"}"#.to_owned()
                    } else {
                        format!(r#"{{"name":"pool-{i}","size":42}}"#)
                    }
                })
                .collect::<Vec<_>>()
                .join(",");
            let response = format!(
                r#"{{"jsonrpc":"2.0","id":{},"result":[{entries}]}}"#,
                request["id"]
            );
            assert!(response.len() < WIRE_LIMIT);
            ws.send(Message::text(response)).await.unwrap();
        }
        while let Some(Ok(_)) = ws.next().await {}
    });
    let ttl = Duration::from_secs(10);
    let state = State::new(ttl, false);
    let stop = CancellationToken::new();
    let (client, task) = Client::spawn(
        config(url, Duration::from_secs(3)),
        state.clone(),
        stop.clone(),
    );
    ready(&state).await;
    let start = tokio::time::Instant::now();
    for (step, count) in [0, 1, 256, 257, 75_000, 1].into_iter().enumerate() {
        let result = client
            .pool_query()
            .await
            .and_then(truenas_exporter::collectors::pools::parse);
        let now = start + Duration::from_secs(step as u64);
        if count > 256 {
            assert!(matches!(result, Err(Reason::Schema)));
            state.complete(result, now, step as f64, Duration::ZERO);
            let expiry = start + Duration::from_secs(2) + ttl;
            assert!(state.ready_at(expiry - Duration::from_nanos(1)));
            assert!(!state.ready_at(expiry));
            let metrics = state.metrics_at(now).unwrap();
            assert!(metrics.contains(
                "truenas_exporter_last_success_timestamp_seconds{collector=\"pools\"} 2"
            ));
            assert!(metrics.contains("truenas_pool_size_bytes{pool=\"pool-255\"} 42"));
        } else {
            let pools = result.unwrap();
            assert_eq!(pools.len(), count);
            state.complete(Ok(pools), now, step as f64, Duration::ZERO);
            assert!(state.ready_at(now));
        }
    }
    stop.cancel();
    task.await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn late_response_cannot_complete_the_next_query() {
    let (listener, url) = listen().await;
    let server = tokio::spawn(async move {
        let mut ws = login(&listener).await;
        let expired = text(&mut ws).await;
        let current = text(&mut ws).await;
        for (request, name) in [(expired, "expired"), (current, "fresh")] {
            ws.send(Message::text(
                json!({"jsonrpc":"2.0","id":request["id"],"result":[{"name":name}]}).to_string(),
            ))
            .await
            .unwrap();
        }
        while let Some(Ok(_)) = ws.next().await {}
    });
    let stop = CancellationToken::new();
    let state = State::new(Duration::from_secs(180), false);
    let (client, task) = Client::spawn(
        config(url, Duration::from_millis(500)),
        state.clone(),
        stop.clone(),
    );
    ready(&state).await;
    assert_eq!(client.pool_query().await, Err(Reason::Timeout));
    assert_eq!(
        client.pool_query().await.unwrap(),
        json!([{"name":"fresh"}])
    );
    stop.cancel();
    task.await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn direct_transport_entries_reject_unsafe_urls_before_connecting() {
    use truenas_exporter::{api::client::check_transport, config::TransportConfig};
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    for url in [
        format!("ws://{address}/api/current"),
        format!("wss://user:secret@{address}/api/current"),
        format!("wss://{address}/api/current?token=secret"),
        format!("wss://{address}/api/current#fragment"),
    ] {
        let api = config(url.clone(), Duration::from_secs(1));
        assert_eq!(
            check_transport(TransportConfig {
                url: api.url.clone(),
                tls: api.tls.clone(),
                timeout: api.timeout,
            })
            .await,
            Err(Reason::Protocol)
        );
        let state = State::new(Duration::from_secs(180), false);
        let stop = CancellationToken::new();
        let (_client, task) = Client::spawn(api, state.clone(), stop.clone());
        timeout(Duration::from_secs(1), async {
            while !state
                .metrics_at(tokio::time::Instant::now())
                .unwrap()
                .contains("reason=\"protocol\"")
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!state.connected.load(Ordering::Relaxed));
        stop.cancel();
        task.await.unwrap();
    }
    assert!(
        timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
}

#[test]
fn error_and_notification_field_types_are_validated() {
    for error in [
        json!({"code":-32601}),
        json!({"code":-32601,"message":null}),
        json!({"code":-32601,"message":17}),
        json!({"code":"-32601","message":"marker"}),
        json!({"message":"marker"}),
        json!(null),
    ] {
        assert!(matches!(
            protocol::decode(&json!({"jsonrpc":"2.0","id":"1","error":error}).to_string()),
            Err(Reason::Protocol)
        ));
    }
    for params in [json!(null), json!(true), json!(1), json!("marker")] {
        assert!(matches!(
            protocol::decode(
                &json!({"jsonrpc":"2.0","method":"notification","params":params}).to_string()
            ),
            Err(Reason::Protocol)
        ));
    }
    for text in [
        r#"{"jsonrpc":"2.0","method":"notification"}"#,
        r#"{"jsonrpc":"2.0","method":"notification","params":{}}"#,
        r#"{"jsonrpc":"2.0","method":"notification","params":[]}"#,
    ] {
        assert!(matches!(protocol::decode(text), Ok(Incoming::Notification)));
    }
    for (error, expected) in [
        (json!({"code":-32601,"message":""}), Reason::Unsupported),
        (
            json!({"code":-32001,"message":"marker","data":{"error":13}}),
            Reason::Permission,
        ),
        (
            json!({"code":-32001,"message":"marker","data":"any JSON is valid error data"}),
            Reason::Api,
        ),
    ] {
        assert!(
            matches!(protocol::decode(&json!({"jsonrpc":"2.0","id":"1","error":error}).to_string()), Ok(Incoming::Response{result:Err(reason),..}) if reason==expected)
        );
    }
}

#[tokio::test]
async fn malformed_envelope_fails_all_pending_queries_and_reconnects() {
    for notification in [false, true] {
        let (listener, url) = listen().await;
        let server = tokio::spawn(async move {
            let mut ws = login(&listener).await;
            let request = text(&mut ws).await;
            let _second = text(&mut ws).await;
            let bad = if notification {
                json!({"jsonrpc":"2.0","method":"notification","params":"PRIVATE_MARKER"})
            } else {
                json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":-32601,"message":123}})
            };
            ws.send(Message::text(bad.to_string())).await.unwrap();
            while let Some(Ok(_)) = ws.next().await {}
            let mut ws = login(&listener).await;
            let request = text(&mut ws).await;
            ws.send(Message::text(
                json!({"jsonrpc":"2.0","id":request["id"],"result":[]}).to_string(),
            ))
            .await
            .unwrap();
            while let Some(Ok(_)) = ws.next().await {}
        });
        let state = State::new(Duration::from_secs(180), false);
        let stop = CancellationToken::new();
        let (client, task) = Client::spawn(
            config(url, Duration::from_secs(3)),
            state.clone(),
            stop.clone(),
        );
        ready(&state).await;
        let (first, second) = tokio::join!(client.pool_query(), client.pool_query());
        assert_eq!(first, Err(Reason::Protocol));
        assert_eq!(second, Err(Reason::Protocol));
        timeout(Duration::from_secs(1), async {
            while state.connected.load(Ordering::Relaxed) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        ready(&state).await;
        assert_eq!(client.pool_query().await.unwrap(), json!([]));
        stop.cancel();
        task.await.unwrap();
        server.await.unwrap();
    }
}

// Only the correlation ID is changed when replaying the saved wire envelope.
#[tokio::test]
async fn captured_appliance_pool_envelopes_replay_offline() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    for (fixture, allocated, free) in [
        (
            include_str!("../../fixtures/truenas-25.04.2.6/pool.query.host.response.json"),
            12_573_765_242_880_u64,
            11_426_512_007_168_u64,
        ),
        (
            include_str!("../../fixtures/truenas-25.04.2.6/pool.query.container.response.json"),
            12_573_763_424_256_u64,
            11_426_513_825_792_u64,
        ),
    ] {
        let (listener, url) = listen().await;
        let mut envelope: Value = serde_json::from_str(fixture).unwrap();
        let server = tokio::spawn(async move {
            let mut socket = login(&listener).await;
            let request = text(&mut socket).await;
            assert_eq!(request["method"], "pool.query");
            assert_eq!(request["params"], json!([]));
            envelope["id"] = request["id"].clone();
            socket
                .send(Message::text(envelope.to_string()))
                .await
                .unwrap();
            while let Some(Ok(message)) = socket.next().await {
                if let Message::Ping(bytes) = message {
                    let _ = socket.send(Message::Pong(bytes)).await;
                }
            }
        });
        let state = State::for_target("captured-target".into(), Duration::from_secs(180), false);
        let stop = CancellationToken::new();
        let (client, task) = Client::spawn(
            config(url, Duration::from_secs(2)),
            state.clone(),
            stop.clone(),
        );
        ready(&state).await;
        let result = client.pool_query().await.unwrap();
        assert!(result[0]["fragmentation"].is_string());
        let pools = truenas_exporter::collectors::pools::parse(result).unwrap();
        assert_eq!(pools.len(), 1);
        assert_eq!(pools[0].size, Some(24_000_277_250_048));
        assert_eq!(pools[0].allocated, Some(allocated));
        assert_eq!(pools[0].free, Some(free));
        assert_eq!(pools[0].healthy, Some(true));
        assert_eq!(pools[0].warning, Some(true));
        assert_eq!(pools[0].fragmentation, Some(13.0));
        let now = tokio::time::Instant::now();
        state.complete(Ok(pools), now, 1000.0, Duration::ZERO);
        assert!(state.ready_at(now));
        let app = truenas_exporter::http::router(state.clone());
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/readyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let metrics = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        for expected in [
            "truenas_pool_size_bytes{target=\"captured-target\",pool=\"captured-pool-1\"} 24000277250048",
            "truenas_pool_healthy{target=\"captured-target\",pool=\"captured-pool-1\"} 1",
            "truenas_pool_warning{target=\"captured-target\",pool=\"captured-pool-1\"} 1",
            "truenas_pool_fragmentation_percent{target=\"captured-target\",pool=\"captured-pool-1\"} 13",
        ] {
            assert!(metrics.contains(expected), "missing {expected}");
        }
        assert!(metrics.contains(&format!("truenas_pool_allocated_bytes{{target=\"captured-target\",pool=\"captured-pool-1\"}} {allocated}")));
        assert!(metrics.contains(&format!(
            "truenas_pool_free_bytes{{target=\"captured-target\",pool=\"captured-pool-1\"}} {free}"
        )));
        assert!(metrics.contains("FEAT_DISABLED"));
        assert!(metrics.contains("ONLINE"));
        assert!(!metrics.contains("truenas_pool_scrub_errors"));
        stop.cancel();
        timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn system_preflight_is_allowlisted_typed_and_isolated_from_pool_collection() {
    for (result, expected) in [
        (
            json!({"result": serde_json::from_str::<Value>(include_str!("fixtures/system.synthetic.json")).unwrap()}),
            None,
        ),
        (
            json!({"result": {"version":"25.10.2", "hostname":"synthetic", "uptime_seconds":1}}),
            Some(Reason::Unsupported),
        ),
        (
            json!({"result": {"version":"25.04.2.6", "hostname":"synthetic", "uptime_seconds":"1"}}),
            Some(Reason::Schema),
        ),
        (
            json!({"error":{"code":-32001,"message":"synthetic-sensitive-error", "data":{"error":13}}}),
            Some(Reason::Permission),
        ),
        (
            json!({"error":{"code":-32601,"message":"synthetic-sensitive-error"}}),
            Some(Reason::Unsupported),
        ),
    ] {
        let (listener, url) = listen().await;
        let server = tokio::spawn(async move {
            let mut socket = login(&listener).await;
            let request = text(&mut socket).await;
            assert_eq!(
                request,
                json!({"jsonrpc":"2.0","id":"system","method":"system.info","params":[]})
            );
            let mut response = result;
            response["jsonrpc"] = json!("2.0");
            response["id"] = request["id"].clone();
            socket
                .send(Message::text(response.to_string()))
                .await
                .unwrap();
            while let Some(Ok(message)) = socket.next().await {
                assert!(
                    !matches!(message, Message::Text(_)),
                    "unexpected extra API call"
                );
            }
        });
        let outcome =
            truenas_exporter::api::client::check_system(config(url, Duration::from_secs(2))).await;
        assert_eq!(outcome.err(), expected);
        timeout(Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn system_preflight_times_out_without_retries_or_followup_calls() {
    let (listener, url) = listen().await;
    let server = tokio::spawn(async move {
        let mut socket = login(&listener).await;
        let request = text(&mut socket).await;
        assert_eq!(request["method"], "system.info");
        // Wrong IDs and unrelated notifications cannot satisfy or extend the deadline.
        socket
            .send(Message::text(
                json!({"jsonrpc":"2.0","id":"other","result":{}}).to_string(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::text(
                json!({"jsonrpc":"2.0","method":"collection_update","params":{}}).to_string(),
            ))
            .await
            .unwrap();
        while let Some(Ok(message)) = socket.next().await {
            assert!(!matches!(message, Message::Text(_)));
        }
    });
    let result = timeout(
        Duration::from_secs(2),
        truenas_exporter::api::client::check_system(config(url, Duration::from_millis(200))),
    )
    .await
    .unwrap();
    assert_eq!(result.err(), Some(Reason::Timeout));
    timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn system_preflight_cli_reports_only_bounded_results() {
    for success in [true, false] {
        let (listener, url) = listen().await;
        let server = tokio::spawn(async move {
            let mut socket = login(&listener).await;
            let request = text(&mut socket).await;
            assert_eq!(request["method"], "system.info");
            let mut response = if success {
                let mut info: Value =
                    serde_json::from_str(include_str!("fixtures/system.synthetic.json")).unwrap();
                info["hostname"] = json!("synthetic-private-host");
                json!({"result": info})
            } else {
                json!({"error":{"code":-32001,"message":"synthetic-private-error","data":{"error":13,"reason":"synthetic-private-detail"}}})
            };
            response["id"] = request["id"].clone();
            response["jsonrpc"] = json!("2.0");
            socket
                .send(Message::text(response.to_string()))
                .await
                .unwrap();
            while let Some(Ok(message)) = socket.next().await {
                assert!(!matches!(message, Message::Text(_)));
            }
        });
        let temp = tempfile::tempdir().unwrap();
        let key = temp.path().join("key");
        let ca = temp.path().join("ca.pem");
        std::fs::write(&key, "synthetic-key").unwrap();
        std::fs::write(&ca, &fixture_tls().pem).unwrap();
        let output = timeout(
            Duration::from_secs(5),
            tokio::process::Command::new(env!("CARGO_BIN_EXE_truenas-exporter"))
                .arg("check-system")
                .env_clear()
                .env("TRUENAS_WS_URL", url)
                .env("TRUENAS_USERNAME", "test-user")
                .env("TRUENAS_API_KEY_FILE", key)
                .env("TRUENAS_CA_CERT_FILE", ca)
                .env("TRUENAS_API_TIMEOUT_SECONDS", "2")
                .env("RUST_LOG", "trace")
                .kill_on_drop(true)
                .output(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(output.status.success(), success);
        let text = format!(
            "{}{}",
            String::from_utf8(output.stdout).unwrap(),
            String::from_utf8(output.stderr).unwrap()
        );
        assert!(text.contains(if success {
            "25.04.2.x compatibility passed"
        } else {
            "System preflight failed: permission"
        }));
        for marker in [
            "synthetic-key",
            "test-user",
            "synthetic-private-host",
            "synthetic-private-serial",
            "synthetic-private-error",
            "synthetic-private-detail",
        ] {
            assert!(!text.contains(marker), "leaked {marker}");
        }
        timeout(Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
    }
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_truenas-exporter"))
        .arg("check-system")
        .env_clear()
        .env("EXPORTER_MOCK", "true")
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("exactly one non-mock target")
    );
}
