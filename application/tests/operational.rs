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
    timeout(Duration::from_secs(180), exercise(Scenario::Pool))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Explicit integrated system/realtime maximum-size outage and resource validation"]
async fn integrated_realtime_outage_recovery_and_stalled_sigterm() {
    timeout(Duration::from_secs(240), exercise(Scenario::Integrated))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Explicit phased steady-state and repeated-recovery memory profiling"]
async fn integrated_memory_profile() {
    timeout(Duration::from_secs(900), exercise(Scenario::MemoryProfile))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Explicit two-container exporter/fixture cgroup profile; requires local controller"]
async fn isolated_cgroup_memory_profile() {
    timeout(
        Duration::from_secs(900),
        exercise(Scenario::IsolatedProfile),
    )
    .await
    .unwrap();
}

#[derive(Clone, Copy)]
enum Scenario {
    Pool,
    Integrated,
    MemoryProfile,
    IsolatedProfile,
}

struct ActiveSession(Arc<AtomicUsize>);
impl Drop for ActiveSession {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(target_os = "linux")]
async fn child_usage(pid: u32) -> (u64, f64) {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let rss_kib: u64 = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let fields: Vec<_> = stat
        .rsplit_once(')')
        .unwrap()
        .1
        .split_whitespace()
        .collect();
    let ticks = fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap();
    // SAFETY: sysconf takes an integer selector and no pointers.
    let ticks_per_second = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    assert!(ticks_per_second > 0);
    (rss_kib * 1024, ticks as f64 / ticks_per_second as f64)
}

#[cfg(target_os = "macos")]
async fn child_usage(pid: u32) -> (u64, f64) {
    let output = Command::new("/bin/ps")
        .args(["-o", "rss=,time=", "-p", &pid.to_string()])
        .output()
        .await
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let mut fields = text.split_whitespace();
    let rss_kib: u64 = fields.next().unwrap().parse().unwrap();
    let seconds = fields
        .next()
        .unwrap()
        .split(':')
        .fold(0.0, |total, component| {
            total * 60.0 + component.parse::<f64>().unwrap()
        });
    (rss_kib * 1024, seconds)
}

// Sampling is test-only and bounded by the enclosing scenario deadline.
#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn sample_child(
    pid: u32,
    phase: Arc<std::sync::Mutex<&'static str>>,
    stop: tokio_util::sync::CancellationToken,
) {
    let mut samples = Vec::new();
    while !stop.is_cancelled() {
        // Copy the phase before awaiting so the mutex never spans process I/O.
        let label = *phase.lock().unwrap();
        let (rss, cpu) = child_usage(pid).await;
        samples.push((Instant::now(), rss, cpu, label));
        tokio::select! {
            _ = stop.cancelled() => break,
            _ = sleep(Duration::from_millis(500)) => {},
        }
    }
    for group in samples.chunk_by(|a, b| a.3 == b.3) {
        let first = group.first().unwrap();
        let last = group.last().unwrap();
        println!(
            "resource_summary={}",
            json!({
                "phase":first.3,"samples":group.len(),"seconds":(last.0-first.0).as_secs_f64(),
                "rss_first_bytes":first.1,"rss_last_bytes":last.1,
                "rss_min_bytes":group.iter().map(|s|s.1).min().unwrap(),
                "rss_peak_bytes":group.iter().map(|s|s.1).max().unwrap(),
                "cpu_seconds":last.2-first.2
            })
        );
    }
}

fn set_phase(phase: &std::sync::Mutex<&'static str>, label: &'static str) {
    *phase.lock().unwrap() = label;
    println!("resource_phase={label}");
}

async fn exercise(scenario: Scenario) {
    let realtime = !matches!(scenario, Scenario::Pool);
    let external = matches!(scenario, Scenario::IsolatedProfile);
    let profile = matches!(
        scenario,
        Scenario::MemoryProfile | Scenario::IsolatedProfile
    );
    let control = external.then(|| {
        let path = std::path::PathBuf::from(
            std::env::var_os("OPERATIONAL_CONTROL_DIR")
                .expect("isolated profile requires a local control directory"),
        );
        assert!(path.is_absolute() && path.is_dir());
        path
    });
    let phase = Arc::new(std::sync::Mutex::new("startup"));
    let experiment_start = Instant::now();
    let active = Arc::new(AtomicUsize::new(0));
    let subscriptions = Arc::new(AtomicUsize::new(0));
    let cleanups = Arc::new(AtomicUsize::new(0));
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
    let cpu = (0..256)
        .map(|i| (format!("cpu{i}"), json!({"usage":25.0})))
        .chain(std::iter::once(("cpu".into(), json!({"usage":25.0}))))
        .collect::<serde_json::Map<_, _>>();
    let interfaces = (0..128)
        .map(|i| {
            (
                format!("interface-{i:03}-{}", "x".repeat(50)),
                json!({
                    "link_state":"LINK_STATE_UP", "speed":1000,
                    "received_bytes_rate":1024, "sent_bytes_rate":2048
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let event = Arc::new(json!({"jsonrpc":"2.0", "method":"collection_update", "params":{
        "collection":"reporting.realtime:{\"interval\":2}", "msg":"added", "fields":{
            "cpu":cpu, "interfaces":interfaces,
            "memory":{"physical_memory_total":34359738368_u64,"physical_memory_available":1073741824,"arc_size":536870912},
            "zfs":{"demand_data_hit_percentage":95,"demand_metadata_hit_percentage":90},
            "failed_to_connect":false
        }
    }}).to_string());
    let server_active = active.clone();
    let server_subscriptions = subscriptions.clone();
    let server_cleanups = cleanups.clone();
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
                    let event = event.clone();
                    let active = server_active.clone();
                    let subscriptions = server_subscriptions.clone();
                    let cleanups = server_cleanups.clone();
                    sessions.spawn(async move {
                        let Ok(tls) = acceptor.accept(tcp).await else { return; };
                        let Ok(mut ws) = accept_async(tls).await else { return; };
                        active.fetch_add(1, Ordering::SeqCst);
                        let _guard = ActiveSession(active);
                        let mut subscription = None;
                        let mut ticks = tokio::time::interval(Duration::from_secs(2));
                        loop {
                            let message = tokio::select! {
                                _ = ticks.tick() => {
                                    if mode.load(Ordering::SeqCst) == 1 { break; }
                                    if subscription.is_some() && mode.load(Ordering::SeqCst) != 2
                                        && ws.send(Message::text(event.as_str())).await.is_err() { break; }
                                    continue;
                                }
                                message = ws.next() => message,
                            };
                            let Some(Ok(message)) = message else { break; };
                            let response = match message {
                                Message::Text(text) => {
                                    if mode.load(Ordering::SeqCst) == 1 { break; }
                                    let request: Value = serde_json::from_str(&text).unwrap();
                                    let result = match request["method"].as_str().unwrap() {
                                        "auth.login_ex" => {
                                            assert_eq!(request["params"][0]["api_key"], "SYNTHETIC_OPERATIONAL_KEY");
                                            json!({"response_type":"SUCCESS"})
                                        }
                                        "system.info" => {
                                            assert!(realtime);
                                            json!({"version":"25.04.2.6","hostname":"synthetic.invalid","uptime_seconds":12345.75,
                                                "physmem":34359738368_u64,"loadavg":[0.25,1.5,2.75]})
                                        }
                                        "pool.query" => {
                                            assert!(subscription.is_none());
                                            count.fetch_add(1, Ordering::Relaxed);
                                            json!(&*pools)
                                        }
                                        "core.subscribe" => {
                                            assert!(realtime && subscription.is_none());
                                            assert_eq!(request["params"], json!(["reporting.realtime:{\"interval\":2}"]));
                                            let id = format!("synthetic-owned-{}", subscriptions.fetch_add(1, Ordering::SeqCst));
                                            subscription = Some(id.clone());
                                            json!(id)
                                        }
                                        "core.unsubscribe" => {
                                            assert_eq!(request["params"], json!([subscription.take().unwrap()]));
                                            cleanups.fetch_add(1, Ordering::SeqCst);
                                            Value::Null
                                        }
                                        _ => panic!("unexpected method"),
                                    };
                                    json!({"jsonrpc":"2.0","id":request["id"],"result":result})
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
    let directory = match &control {
        Some(path) => tempfile::tempdir_in(path).unwrap(),
        None => tempfile::tempdir().unwrap(),
    };
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
    let mut child = if external {
        None
    } else {
        Some(
            Command::new(env!("CARGO_BIN_EXE_truenas-exporter"))
                .env_clear()
                .env("TRUENAS_TARGETS_FILE", &file)
                .env("EXPORTER_LISTEN", address.to_string())
                .env("TRUENAS_POLL_INTERVAL_SECONDS", "1")
                .env("TRUENAS_API_TIMEOUT_SECONDS", "1")
                .env("TRUENAS_CACHE_TTL_SECONDS", "3")
                .env("TRUENAS_SYSTEM_ENABLED", realtime.to_string())
                .env("TRUENAS_REALTIME_ENABLED", realtime.to_string())
                .env("TRUENAS_REALTIME_TTL_SECONDS", "3")
                .env("RUST_LOG", "error")
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .kill_on_drop(true)
                .spawn()
                .unwrap(),
        )
    };
    if let Some(control) = &control {
        let launch = json!({"environment":{
            "TRUENAS_TARGETS_FILE":file,"EXPORTER_LISTEN":address.to_string(),
            "TRUENAS_POLL_INTERVAL_SECONDS":"1","TRUENAS_API_TIMEOUT_SECONDS":"1",
            "TRUENAS_CACHE_TTL_SECONDS":"3","TRUENAS_SYSTEM_ENABLED":"true",
            "TRUENAS_REALTIME_ENABLED":"true","TRUENAS_REALTIME_TTL_SECONDS":"3",
            "RUST_LOG":"error"
        }});
        std::fs::write(control.join("launch.tmp"), launch.to_string()).unwrap();
        std::fs::rename(control.join("launch.tmp"), control.join("launch.json")).unwrap();
        timeout(Duration::from_secs(30), async {
            while !control.join("started").exists() {
                assert!(
                    !control.join("exit.json").exists(),
                    "external exporter exited before startup"
                );
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let resources_stop = tokio_util::sync::CancellationToken::new();
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let resources = child.as_ref().map(|child| {
        tokio::spawn(sample_child(
            child.id().unwrap(),
            phase.clone(),
            resources_stop.clone(),
        ))
    });
    if let Some(child) = &child {
        println!(
            "operational_exporter_pid={} realtime={realtime}",
            child.id().unwrap()
        );
    } else {
        println!("operational_exporter=external_isolated_cgroup");
    }
    timeout(Duration::from_secs(15), async {
        loop {
            if let Some(child) = &mut child {
                assert!(child.try_wait().unwrap().is_none());
            } else {
                assert!(!control.as_ref().unwrap().join("exit.json").exists());
            }
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
    assert_eq!(
        active.load(Ordering::SeqCst),
        if realtime { 32 } else { 16 }
    );
    if realtime {
        let metrics = get(address, "/metrics").await;
        assert_eq!(
            metrics
                .lines()
                .filter(|line| line.starts_with("truenas_cpu_utilization_ratio{"))
                .count(),
            16 * 257
        );
        assert_eq!(
            metrics
                .lines()
                .filter(|line| line.starts_with("truenas_network_link_up{"))
                .count(),
            16 * 128
        );
    }
    if profile {
        set_phase(&phase, "baseline_no_scrapers");
        sleep(Duration::from_secs(30)).await;
        assert!(get(address, "/readyz").await.starts_with("HTTP/1.1 200"));
    }
    set_phase(&phase, "scrape_load");
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
    for _ in 0..if profile { 120 } else { 30 } {
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
    let mut successes = 0;
    let mut busy = 0;
    if profile {
        stop_load.cancel();
        while let Some(result) = scrapers.join_next().await {
            let (a, b) = result.unwrap();
            successes += a;
            busy += b;
        }
        set_phase(&phase, "cooldown_no_scrapers");
        sleep(Duration::from_secs(45)).await;
        assert!(get(address, "/readyz").await.starts_with("HTTP/1.1 200"));
        for (outage, recovery) in [
            ("outage_1", "recovery_1"),
            ("outage_2", "recovery_2"),
            ("outage_3", "recovery_3"),
        ] {
            set_phase(&phase, outage);
            mode.store(1, Ordering::SeqCst);
            sleep(Duration::from_secs(8)).await;
            assert!(get(address, "/readyz").await.starts_with("HTTP/1.1 503"));
            let expired = get(address, "/metrics").await;
            for family in [
                "truenas_pool_size_bytes",
                "truenas_memory_total_bytes",
                "truenas_cpu_utilization_ratio",
                "truenas_memory_available_bytes",
                "truenas_network_link_up",
                "truenas_zfs_arc_size_bytes",
            ] {
                assert!(
                    !expired
                        .lines()
                        .any(|line| line.starts_with(&format!("{family}{{")))
                );
            }
            mode.store(0, Ordering::SeqCst);
            timeout(Duration::from_secs(70), async {
                while !get(address, "/readyz").await.starts_with("HTTP/1.1 200") {
                    sleep(Duration::from_millis(100)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(active.load(Ordering::SeqCst), 32);
            set_phase(&phase, recovery);
            sleep(Duration::from_secs(20)).await;
            assert!(get(address, "/readyz").await.starts_with("HTTP/1.1 200"));
        }
    }
    if !profile {
        if realtime {
            mode.store(2, Ordering::SeqCst); // Withhold events while polling and pongs continue.
            sleep(Duration::from_secs(8)).await;
            assert!(get(address, "/readyz").await.starts_with("HTTP/1.1 503"));
            stop_load.cancel();
            // Drain load before inspecting the complete metric set.
            while let Some(result) = scrapers.join_next().await {
                let (a, b) = result.unwrap();
                successes += a;
                busy += b;
            }
            let metrics = get(address, "/metrics").await;
            for family in [
                "truenas_cpu_utilization_ratio",
                "truenas_memory_available_bytes",
                "truenas_network_link_up",
                "truenas_zfs_arc_size_bytes",
            ] {
                assert!(
                    !metrics
                        .lines()
                        .any(|line| line.starts_with(&format!("{family}{{")))
                );
            }
            assert_eq!(
                metrics
                    .lines()
                    .filter(|line| line.starts_with("truenas_memory_total_bytes{"))
                    .count(),
                16
            );
            assert_eq!(
                metrics
                    .lines()
                    .filter(|line| line.starts_with("truenas_pool_size_bytes{"))
                    .count(),
                16 * 256
            );
            mode.store(0, Ordering::SeqCst);
            timeout(Duration::from_secs(70), async {
                while !get(address, "/readyz").await.starts_with("HTTP/1.1 200") {
                    sleep(Duration::from_millis(100)).await;
                }
            })
            .await
            .unwrap();
            println!("realtime_silence_expiry_and_polling_isolation=passed");
        }
        mode.store(1, Ordering::SeqCst);
        sleep(Duration::from_secs(15)).await;
        assert!(get(address, "/readyz").await.starts_with("HTTP/1.1 503"));
        stop_load.cancel();
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
        if realtime {
            for family in [
                "truenas_memory_total_bytes",
                "truenas_cpu_utilization_ratio",
                "truenas_memory_available_bytes",
                "truenas_network_link_up",
                "truenas_zfs_arc_size_bytes",
            ] {
                assert!(
                    !expired
                        .lines()
                        .any(|line| line.starts_with(&format!("{family}{{")))
                );
            }
        }
        mode.store(0, Ordering::SeqCst);
        timeout(Duration::from_secs(70), async {
            while !get(address, "/readyz").await.starts_with("HTTP/1.1 200") {
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .unwrap();
    }
    let recovered = get(address, "/metrics").await;
    for i in 0..16 {
        assert!(recovered.contains(&format!(
            "truenas_pool_size_bytes{{target=\"t{i:02}\",pool=\"pool-255-"
        )));
    }
    assert_eq!(
        active.load(Ordering::SeqCst),
        if realtime { 32 } else { 16 }
    );
    let cleanup_before = cleanups.load(Ordering::SeqCst);
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
    set_phase(&phase, "stalled_reader");
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        resources_stop.cancel();
        if let Some(resources) = resources {
            resources.await.unwrap();
        }
    }
    let started = Instant::now();
    if let Some(child) = &mut child {
        // SAFETY: kill takes no pointers. This positive PID belongs to the live child;
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
    } else {
        let control = control.as_ref().unwrap();
        std::fs::write(control.join("stop"), "SIGTERM").unwrap();
        timeout(Duration::from_secs(15), async {
            while !control.join("exit.json").exists() {
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        let exit: Value =
            serde_json::from_str(&std::fs::read_to_string(control.join("exit.json")).unwrap())
                .unwrap();
        assert_eq!(
            exit["code"], 0,
            "external exporter did not shut down successfully"
        );
    }
    if realtime {
        assert_eq!(cleanups.load(Ordering::SeqCst) - cleanup_before, 16);
    }
    timeout(Duration::from_secs(3), async {
        while active.load(Ordering::SeqCst) != 0 {
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    println!(
        "successes={successes} busy={busy} collections={} health_max_ms={} sigterm_ms={}",
        collections.load(Ordering::Relaxed),
        health_max.as_millis(),
        started.elapsed().as_millis()
    );
    println!(
        "realtime={realtime} subscriptions={} owned_cleanups={} elapsed_ms={}",
        subscriptions.load(Ordering::SeqCst),
        cleanups.load(Ordering::SeqCst),
        experiment_start.elapsed().as_millis()
    );
    if let Ok(cpu) = std::fs::read_to_string("/sys/fs/cgroup/cpu.stat") {
        println!("cgroup_cpu_stat={}", cpu.trim().replace('\n', ";"));
    }
    if let Ok(peak) = std::fs::read_to_string("/sys/fs/cgroup/memory.peak") {
        println!("cgroup_peak_bytes={}", peak.trim());
    }
    drop(stalled);
    server.abort();
    let _ = server.await;
}
