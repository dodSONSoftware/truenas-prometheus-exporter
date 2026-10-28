#![cfg(unix)]

use futures_util::{SinkExt, StreamExt};
use rustls::{ServerConfig, pki_types::PrivatePkcs8KeyDer};
use serde_json::{Value, json};
use std::{net::SocketAddr, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    process::Command,
    task::JoinHandle,
    time::{sleep, timeout},
};
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::{accept_async, tungstenite::Message};

async fn mock_server(
    username: &'static str,
    key: &'static str,
    size: u64,
    deny_after_first: bool,
) -> (String, String, JoinHandle<()>) {
    let certificate = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let ca = certificate.cert.pem();
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![certificate.cert.der().clone()],
            PrivatePkcs8KeyDer::from(certificate.key_pair.serialize_der()).into(),
        )
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("wss://{}/api/current", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let tls = TlsAcceptor::from(Arc::new(config))
            .accept(tcp)
            .await
            .unwrap();
        let mut ws = accept_async(tls).await.unwrap();
        let mut authenticated = false;
        let mut collected = false;
        while let Some(Ok(message)) = ws.next().await {
            match message {
                Message::Ping(bytes) => {
                    let _ = ws.send(Message::Pong(bytes)).await;
                }
                Message::Close(_) => break,
                Message::Text(text) => {
                    let request: Value = serde_json::from_str(&text).unwrap();
                    let response = if !authenticated {
                        assert_eq!(request["method"], "auth.login_ex");
                        assert_eq!(request["params"][0]["username"], username);
                        assert_eq!(request["params"][0]["api_key"], key);
                        authenticated = true;
                        json!({"jsonrpc":"2.0","id":request["id"],"result":{"response_type":"SUCCESS"}})
                    } else {
                        assert_eq!(request["method"], "pool.query");
                        if collected && deny_after_first {
                            json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":-32001,"message":"fixture permission denied","data":{"error":13}}})
                        } else {
                            collected = true;
                            json!({"jsonrpc":"2.0","id":request["id"],"result":[{"name":"same-pool","size":size,"allocated":size/2,"free":size/2,"healthy":true,"warning":false,"status":"ONLINE"}]})
                        }
                    };
                    if ws.send(Message::text(response.to_string())).await.is_err() {
                        break;
                    }
                }
                _ => {}
            }
        }
    });
    (url, ca, task)
}

async fn get(address: SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    String::from_utf8(response).unwrap()
}

#[tokio::test]
async fn two_real_workers_isolate_credentials_tls_snapshots_and_failures() {
    let (alpha_url, alpha_ca, alpha) =
        mock_server("alpha-user", "ALPHA_TEST_KEY", 1_000_000, false).await;
    let (beta_url, beta_ca, beta) =
        mock_server("beta-user", "BETA_TEST_KEY", 2_000_000, true).await;
    let folder = tempfile::tempdir().unwrap();
    let mut targets = Vec::new();
    for (name, url, ca, key) in [
        ("alpha", alpha_url, alpha_ca, "ALPHA_TEST_KEY"),
        ("beta", beta_url, beta_ca, "BETA_TEST_KEY"),
    ] {
        let ca_file = folder.path().join(format!("{name}-ca"));
        std::fs::write(&ca_file, ca).unwrap();
        let key_file = folder.path().join(format!("{name}-key"));
        std::fs::write(&key_file, key).unwrap();
        targets.push(json!({"name":name,"ws_url":url,"username":format!("{name}-user"),"api_key_file":key_file,"ca_cert_file":ca_file}));
    }
    let config = folder.path().join("targets.json");
    std::fs::write(&config, json!({"targets":targets}).to_string()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut child = Command::new(env!("CARGO_BIN_EXE_truenas-exporter"))
        .env_clear()
        .env("TRUENAS_TARGETS_FILE", config)
        .env("EXPORTER_LISTEN", address.to_string())
        .env("TRUENAS_POLL_INTERVAL_SECONDS", "1")
        .env("TRUENAS_API_TIMEOUT_SECONDS", "1")
        .env("TRUENAS_CACHE_TTL_SECONDS", "3")
        .env("RUST_LOG", "trace")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    timeout(Duration::from_secs(5), async {
        loop {
            if TcpStream::connect(address).await.is_ok() {
                break;
            }
            assert!(child.try_wait().unwrap().is_none());
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    timeout(Duration::from_secs(5), async {
        loop {
            let text = get(address, "/metrics").await;
            if text.contains("truenas_pool_size_bytes{target=\"alpha\",pool=\"same-pool\"} 1000000")
                && text
                    .contains("truenas_pool_size_bytes{target=\"beta\",pool=\"same-pool\"} 2000000")
            {
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(get(address, "/readyz").await.starts_with("HTTP/1.1 200"));
    timeout(Duration::from_secs(6), async {
        loop {
            let text = get(address, "/metrics").await;
            if !text
                .lines()
                .any(|line| line.starts_with("truenas_pool_size_bytes{target=\"beta\""))
            {
                assert!(text.contains(
                    "truenas_pool_size_bytes{target=\"alpha\",pool=\"same-pool\"} 1000000"
                ));
                assert!(text.contains(
                    "truenas_exporter_collection_success{target=\"alpha\",collector=\"pools\"} 1"
                ));
                assert!(text.contains(
                    "truenas_exporter_collection_success{target=\"beta\",collector=\"pools\"} 0"
                ));
                assert_eq!(text.matches("# HELP truenas_pool_size_bytes ").count(), 1);
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(get(address, "/readyz").await.starts_with("HTTP/1.1 503"));
    assert!(get(address, "/healthz").await.starts_with("HTTP/1.1 200"));
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
    for secret in ["ALPHA_TEST_KEY", "BETA_TEST_KEY", "alpha-user", "beta-user"] {
        assert!(!logs.contains(secret));
    }
    timeout(Duration::from_secs(2), alpha)
        .await
        .unwrap()
        .unwrap();
    timeout(Duration::from_secs(2), beta)
        .await
        .unwrap()
        .unwrap();
}
