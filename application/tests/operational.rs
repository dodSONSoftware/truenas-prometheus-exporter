#![cfg(unix)]

use futures_util::{SinkExt, StreamExt};
use rustls::{ServerConfig, pki_types::PrivatePkcs8KeyDer};
use serde_json::{Value, json};
use std::{
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpSocket, TcpStream},
    process::Command,
    task::JoinSet,
    time::{Instant, sleep, timeout},
};
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::{accept_async, tungstenite::Message};

async fn get(address: std::net::SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).await.unwrap();
    String::from_utf8(bytes).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Explicit sustained, maximum-size process and shutdown validation"]
async fn sustained_collection_outage_recovery_and_stalled_sigterm() {
    timeout(Duration::from_secs(180), exercise()).await.unwrap();
}

async fn exercise() {
    let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.cert.der().clone()],
            PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()).into(),
        )
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("wss://{}/api/current", listener.local_addr().unwrap());
    let mode = Arc::new(AtomicUsize::new(0));
    let collections = Arc::new(AtomicUsize::new(0));
    let server_mode = mode.clone();
    let server_collections = collections.clone();
    let pools = Arc::new(json!(
        (0..256)
            .map(|i| json!({
                "name":format!("pool-{i:03}-{}", "x".repeat(119)),
                "size":1000,"allocated":400,"free":600,"healthy":true,"warning":false,
                "fragmentation":13,"status":"ONLINE"
            }))
            .collect::<Vec<_>>()
    ));
    let server = tokio::spawn(async move {
        let acceptor = TlsAcceptor::from(Arc::new(server_config));
        let mut sessions = JoinSet::new();
        loop {
            tokio::select! {
                connection = listener.accept() => {
                    let (tcp, _) = connection.unwrap();
                    let acceptor = acceptor.clone();
                    let mode = server_mode.clone();
                    let count = server_collections.clone();
                    let pools = pools.clone();
                    sessions.spawn(async move {
                        let Ok(tls) = acceptor.accept(tcp).await else { return; };
                        let Ok(mut ws) = accept_async(tls).await else { return; };
                        while let Some(Ok(message)) = ws.next().await {
                            let response = match message {
                                Message::Text(text) => {
                                    let request: Value = serde_json::from_str(&text).unwrap();
                                    match request["method"].as_str().unwrap() {
                                        "auth.login_ex" => {
                                            assert_eq!(request["params"][0]["api_key"], "SYNTHETIC_OPERATIONAL_KEY");
                                            json!({"jsonrpc":"2.0","id":request["id"],"result":{"response_type":"SUCCESS"}})
                                        }
                                        "pool.query" => {
                                            count.fetch_add(1, Ordering::Relaxed);
                                            if mode.load(Ordering::Relaxed) == 1 {
                                                return; // Interrupt every live transport during the outage.
                                            }
                                            json!({"jsonrpc":"2.0","id":request["id"],"result":&*pools})
                                        }
                                        _ => panic!("unexpected method"),
                                    }
                                }
                                Message::Ping(bytes) => { if ws.send(Message::Pong(bytes)).await.is_err(){break;} continue; }
                                _ => break,
                            };
                            if ws.send(Message::text(response.to_string())).await.is_err(){break;}
                        }
                    });
                }
                result = sessions.join_next(), if !sessions.is_empty() => { result.unwrap().unwrap(); }
            }
        }
    });
    let directory = tempfile::tempdir().unwrap();
    let key = directory.path().join("key");
    let ca = directory.path().join("ca");
    let file = directory.path().join("targets.json");
    std::fs::write(&key, "SYNTHETIC_OPERATIONAL_KEY").unwrap();
    std::fs::write(&ca, cert.cert.pem()).unwrap();
    std::fs::write(
        &file,
        json!({"targets":(0..16).map(|i|json!({
        "name":format!("t{i:02}"),"ws_url":url,"username":"synthetic",
        "api_key_file":key,"ca_cert_file":ca
    })).collect::<Vec<_>>()} )
        .to_string(),
    )
    .unwrap();
    let reserve = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = reserve.local_addr().unwrap();
    drop(reserve);
    let mut child = Command::new(env!("CARGO_BIN_EXE_truenas-exporter"))
        .env_clear()
        .env("TRUENAS_TARGETS_FILE", &file)
        .env("EXPORTER_LISTEN", address.to_string())
        .env("TRUENAS_POLL_INTERVAL_SECONDS", "1")
        .env("TRUENAS_API_TIMEOUT_SECONDS", "1")
        .env("TRUENAS_CACHE_TTL_SECONDS", "3")
        .env("RUST_LOG", "error")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    timeout(Duration::from_secs(15), async {
        loop {
            assert!(child.try_wait().unwrap().is_none());
            if TcpStream::connect(address).await.is_ok()
                && get(address, "/readyz").await.starts_with("HTTP/1.1 200")
            {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let stop_load = tokio_util::sync::CancellationToken::new();
    let mut scrapers = JoinSet::new();
    for _ in 0..4 {
        let stop = stop_load.clone();
        scrapers.spawn(async move {
            let mut successes = 0;
            let mut busy = 0;
            loop {
                if stop.is_cancelled() {
                    break;
                }
                let response = get(address, "/metrics").await;
                if response.starts_with("HTTP/1.1 200") {
                    assert_eq!(response.matches("# EOF").count(), 1);
                    successes += 1;
                } else {
                    assert!(response.starts_with("HTTP/1.1 503"));
                    assert!(response.contains("metrics encoder busy"));
                    busy += 1;
                }
                sleep(Duration::from_millis(50)).await;
            }
            (successes, busy)
        });
    }
    let initial_count = collections.load(Ordering::Relaxed);
    let mut health_max = Duration::ZERO;
    for _ in 0..30 {
        let start = Instant::now();
        assert!(
            timeout(Duration::from_secs(3), get(address, "/healthz"))
                .await
                .unwrap()
                .starts_with("HTTP/1.1 200")
        );
        health_max = health_max.max(start.elapsed());
        sleep(Duration::from_secs(1)).await;
    }
    assert!(collections.load(Ordering::Relaxed) > initial_count + 16);
    mode.store(1, Ordering::Relaxed);
    sleep(Duration::from_secs(15)).await;
    assert!(get(address, "/readyz").await.starts_with("HTTP/1.1 503"));
    stop_load.cancel();
    let mut successes = 0;
    let mut busy = 0;
    while let Some(result) = scrapers.join_next().await {
        let (a, b) = result.unwrap();
        successes += a;
        busy += b;
    }
    let expired = get(address, "/metrics").await;
    assert!(
        !expired
            .lines()
            .any(|line| line.starts_with("truenas_pool_size_bytes{"))
    );
    assert!(expired.contains("truenas_exporter_last_success_timestamp_seconds{"));
    mode.store(0, Ordering::Relaxed);
    timeout(Duration::from_secs(45), async {
        while !get(address, "/readyz").await.starts_with("HTTP/1.1 200") {
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    let recovered = get(address, "/metrics").await;
    for i in 0..16 {
        assert!(recovered.contains(&format!(
            "truenas_pool_size_bytes{{target=\"t{i:02}\",pool=\"pool-255-"
        )));
    }
    let socket = TcpSocket::new_v4().unwrap();
    socket.set_recv_buffer_size(1024).unwrap();
    let mut stalled = socket.connect(address).await.unwrap();
    stalled
        .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut prefix = [0; 256];
    let size = stalled.read(&mut prefix).await.unwrap();
    assert!(prefix[..size].starts_with(b"HTTP/1.1 200"));
    sleep(Duration::from_millis(200)).await;
    let started = Instant::now();
    // SAFETY: kill takes no pointers. The positive PID belongs to this live child;
    // kill_on_drop covers assertion failures.
    assert_eq!(
        unsafe { libc::kill(child.id().unwrap() as libc::pid_t, libc::SIGTERM) },
        0
    );
    let status = timeout(Duration::from_secs(5), child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success());
    println!(
        "successes={successes} busy={busy} collections={} health_max_ms={} sigterm_ms={}",
        collections.load(Ordering::Relaxed),
        health_max.as_millis(),
        started.elapsed().as_millis()
    );
    if let Ok(peak) = std::fs::read_to_string("/sys/fs/cgroup/memory.peak") {
        println!("cgroup_peak_bytes={}", peak.trim());
    }
    drop(stalled);
    server.abort();
    let _ = server.await;
}
