use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{
    sync::{Arc, atomic::Ordering},
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

type Socket = WebSocketStream<TcpStream>;
fn config(url: String, deadline: Duration) -> ApiConfig {
    ApiConfig {
        url: url.parse().unwrap(),
        credentials: Credentials {
            username: "test-user".into(),
            key: "synthetic-key".into(),
        },
        tls: tls_config(None).unwrap(),
        timeout: deadline,
    }
}
async fn listen() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/api/current", listener.local_addr().unwrap());
    (listener, url)
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
async fn login(listener: &TcpListener) -> Socket {
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
async fn malformed_oversized_and_stalled_servers_fail_boundedly() {
    for payload in [
        Some("not-json".to_owned()),
        Some("x".repeat(1024 * 1024 + 1)),
        None,
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
        assert!(matches!(result, Err(Reason::Protocol | Reason::Timeout)));
        stop.cancel();
        timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
        server.abort();
        let _ = server.await;
    }
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
