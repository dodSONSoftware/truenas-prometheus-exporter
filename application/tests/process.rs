#![cfg(unix)]

use futures_util::{SinkExt, StreamExt};
use rustls::{ServerConfig, pki_types::PrivatePkcs8KeyDer};
use serde_json::{Value, json};
use std::{
    net::{Ipv4Addr, SocketAddr},
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    process::Command,
    time::{sleep, timeout},
};
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::{accept_async, tungstenite::Message};

async fn get(address: SocketAddr, path: &str) -> std::io::Result<String> {
    let mut stream = TcpStream::connect(address).await?;
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await?;
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).await?;
    Ok(String::from_utf8(bytes).unwrap())
}

#[tokio::test]
async fn real_process_keeps_http_alive_redacts_trace_and_stops_on_sigterm() {
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
    let marker = "SYNTHETIC_SECRET_MUST_NOT_APPEAR";
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let tls = TlsAcceptor::from(Arc::new(server_config))
            .accept(tcp)
            .await
            .unwrap();
        let mut ws = accept_async(tls).await.unwrap();
        while let Some(Ok(message)) = ws.next().await {
            match message {
                Message::Text(text) => {
                    let request: Value = serde_json::from_str(&text).unwrap();
                    let response = if request["method"] == "auth.login_ex" {
                        json!({"jsonrpc":"2.0","id":request["id"],"result":{"response_type":"SUCCESS"}})
                    } else {
                        json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":-32001,"message":marker,"data":{"error":13,"reason":marker}}})
                    };
                    if ws.send(Message::text(response.to_string())).await.is_err() {
                        break;
                    }
                }
                Message::Ping(bytes) => {
                    let _ = ws.send(Message::Pong(bytes)).await;
                }
                _ => {}
            }
        }
    });
    let directory = tempfile::tempdir().unwrap();
    let key = directory.path().join("key");
    let ca = directory.path().join("ca");
    std::fs::write(&key, marker).unwrap();
    std::fs::write(&ca, certificate.cert.pem()).unwrap();
    let reserved = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = reserved.local_addr().unwrap();
    drop(reserved);
    let mut child = Command::new(env!("CARGO_BIN_EXE_truenas-exporter"))
        .env_clear()
        .env("TRUENAS_WS_URL", url)
        .env("TRUENAS_USERNAME", marker)
        .env("TRUENAS_API_KEY_FILE", key)
        .env("TRUENAS_CA_CERT_FILE", ca)
        .env("EXPORTER_LISTEN", address.to_string())
        .env("RUST_LOG", "trace")
        .env("TRUENAS_POLL_INTERVAL_SECONDS", "1")
        .env("TRUENAS_API_TIMEOUT_SECONDS", "1")
        .env("TRUENAS_CACHE_TTL_SECONDS", "3")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    timeout(Duration::from_secs(5), async {
        loop {
            if get(address, "/metrics")
                .await
                .is_ok_and(|text| text.contains("reason=\"permission\""))
            {
                break;
            }
            assert!(
                child.try_wait().unwrap().is_none(),
                "exporter exited prematurely"
            );
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        get(address, "/healthz")
            .await
            .unwrap()
            .starts_with("HTTP/1.1 200")
    );
    assert!(
        get(address, "/readyz")
            .await
            .unwrap()
            .starts_with("HTTP/1.1 503")
    );
    let healthcheck = Command::new(env!("CARGO_BIN_EXE_truenas-exporter"))
        .arg("healthcheck")
        .env_clear()
        .env("EXPORTER_LISTEN", address.to_string())
        .output()
        .await
        .unwrap();
    assert!(healthcheck.status.success());
    let signal = Command::new("/bin/kill")
        .args(["-TERM", &child.id().unwrap().to_string()])
        .status()
        .await
        .unwrap();
    assert!(signal.success());
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
    assert!(logs.contains("permission"));
    assert!(!logs.contains(marker));
    assert!(!logs.contains("API_KEY_PLAIN"));
    assert!(!logs.contains("auth.login_ex"));
    timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn healthcheck_handles_split_bounded_status_lines() {
    let mut exact = b"HTTP/1.1 200 ".to_vec();
    exact.resize(126, b'x');
    exact.extend_from_slice(b"\r\n");
    let mut oversized = exact.clone();
    oversized.insert(12, b'x');
    for (response, expected) in [
        (b"HTTP/1.1 200 OK\r\n".to_vec(), true),
        (exact, true),
        (oversized, false),
        (b"HTTP/1.1 503 Unavailable\r\n".to_vec(), false),
        (b"HTTP/1.1 200 OK".to_vec(), false),
        (b"HTTP/1.1 200 OK\n".to_vec(), false),
        (Vec::new(), false),
    ] {
        let stalled = response.is_empty();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(stream.read_u8().await.unwrap());
            }
            assert!(request.starts_with(b"GET /healthz HTTP/1.1\r\n"));
            if response.is_empty() {
                sleep(Duration::from_secs(5)).await;
            } else {
                stream.write_all(&response[..5]).await.unwrap();
                sleep(Duration::from_millis(25)).await;
                let _ = stream.write_all(&response[5..]).await;
            }
        });
        let started = tokio::time::Instant::now();
        let output = timeout(
            Duration::from_secs(4),
            Command::new(env!("CARGO_BIN_EXE_truenas-exporter"))
                .arg("healthcheck")
                .env_clear()
                .env("EXPORTER_LISTEN", address.to_string())
                .output(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            output.status.success(),
            expected,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(started.elapsed() < Duration::from_secs(4));
        if stalled {
            assert!(String::from_utf8_lossy(&output.stderr).contains("liveness check timed out"));
            assert!(started.elapsed() >= Duration::from_secs(3));
        }
        server.abort();
        let _ = server.await;
    }
}
