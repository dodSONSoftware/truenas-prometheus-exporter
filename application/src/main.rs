use anyhow::{Context, Result, bail};
use std::{
    future::IntoFuture,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};
use truenas_exporter::{
    api::client::{Client, check_transport},
    config::{Config, TransportConfig, environment_value, listen_address},
    http, scheduler,
    state::State,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [arg] if arg == "healthcheck" => return healthcheck().await,
        [arg] if arg == "check-transport" => {
            check_transport(TransportConfig::from_env()?)
                .await
                .map_err(|reason| {
                    anyhow::anyhow!("WSS transport check failed: {}", reason.as_str())
                })?;
            println!(
                "TLS certificate and hostname verification passed; WebSocket HTTP 101; authentication not attempted"
            );
            return Ok(());
        }
        [arg] if arg == "--help" => {
            println!(
                "truenas-exporter [healthcheck|check-transport|--help|--version]\nConfiguration: environment; see docs/configuration.md. EXPORTER_MOCK=true runs offline."
            );
            return Ok(());
        }
        [arg] if arg == "--version" => {
            println!("truenas-exporter {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        [] => {}
        _ => bail!("unknown argument; use --help"),
    }
    let filter = match environment_value("RUST_LOG")? {
        Some(value) => tracing_subscriber::EnvFilter::try_new(value)
            .map_err(|_| anyhow::anyhow!("invalid RUST_LOG filter"))?,
        None => tracing_subscriber::EnvFilter::new("info"),
    };
    // Dependencies may trace raw WebSocket frames. Only our redacted events may be emitted.
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer().with_filter(tracing_subscriber::filter::filter_fn(
                |metadata| metadata.target().starts_with("truenas_exporter"),
            )),
        )
        .init();
    let config = Config::from_env()?;
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .context("cannot bind exporter listener")?;
    let stop = CancellationToken::new();
    let mut states = Vec::new();
    let mut workers = Vec::new();
    for target in config.targets {
        let span = tracing::info_span!(
            "target",
            target = target.name.as_deref().unwrap_or("default")
        );
        let state = match target.name {
            Some(name) => State::for_target(name, config.ttl, target.api.is_none()),
            None => State::new(config.ttl, target.api.is_none()),
        };
        let client = match target.api {
            Some(api) => {
                let (client, connection) =
                    span.in_scope(|| Client::spawn(api, state.clone(), stop.clone()));
                workers.push(connection);
                Some(client)
            }
            None => None,
        };
        workers.push(tokio::spawn(
            scheduler::run(client, state.clone(), config.poll_interval, stop.clone())
                .instrument(span),
        ));
        states.push(state);
    }
    let signal_stop = stop.clone();
    let signal = tokio::spawn(async move {
        shutdown_signal().await;
        signal_stop.cancel();
    });
    tracing::info!(targets = states.len(), "Exporter HTTP server starting");
    let served = serve_http(listener, http::router_targets(states), stop.clone()).await;
    stop.cancel();
    for worker in workers {
        worker.await.context("background worker failed")?;
    }
    signal.abort();
    let _ = signal.await;
    served.context("HTTP server failed")
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn healthcheck() -> Result<()> {
    let mut address = listen_address(environment_value("EXPORTER_LISTEN")?.as_deref())?;
    if address.ip().is_unspecified() {
        address.set_ip(match address.ip() {
            IpAddr::V4(_) => Ipv4Addr::LOCALHOST.into(),
            IpAddr::V6(_) => Ipv6Addr::LOCALHOST.into(),
        });
    }
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut stream = TcpStream::connect(address).await?;
        stream
            .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await?;
        let mut status = [0; 128];
        let mut length = 0;
        loop {
            if length == status.len() {
                bail!("liveness status line exceeds 128 bytes");
            }
            status[length] = stream
                .read_u8()
                .await
                .context("incomplete liveness status line")?;
            length += 1;
            if status[length - 1] == b'\n' {
                break;
            }
        }
        if !status[..length].starts_with(b"HTTP/1.1 200 ") || !status[..length].ends_with(b"\r\n") {
            bail!("liveness check failed");
        }
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("liveness check timed out")?
}

async fn serve_http(
    listener: tokio::net::TcpListener,
    router: axum::Router,
    stop: CancellationToken,
) -> std::io::Result<()> {
    let server = axum::serve(listener, router)
        .with_graceful_shutdown(stop.clone().cancelled_owned())
        .into_future();
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => return result,
        _ = stop.cancelled() => {}
    }
    match tokio::time::timeout(Duration::from_secs(2), &mut server).await {
        Ok(result) => result,
        Err(_) => {
            // Main joins cancelled workers and exits the runtime, which drops
            // the remaining HTTP connection tasks.
            tracing::warn!("HTTP drain deadline reached; process shutting down");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        net::{TcpListener, TcpSocket},
        time::{Instant, timeout},
    };
    use truenas_exporter::collectors::pools;

    #[tokio::test]
    async fn shutdown_bounds_the_wait_for_a_stalled_metrics_response() {
        let states = (0..16)
            .map(|n| {
                let state =
                    State::for_target(format!("target-{n}"), Duration::from_secs(180), false);
                let data = serde_json::json!(
                    (0..256)
                        .map(|i| serde_json::json!({
                            "name":format!("pool-{i:03}-{}", "x".repeat(119)),
                            "size":1000,"allocated":400,"free":600,
                            "healthy":true,"warning":false,"fragmentation":13,
                            "status":"ONLINE"
                        }))
                        .collect::<Vec<_>>()
                );
                state.complete(
                    Ok(pools::parse(data).unwrap()),
                    Instant::now(),
                    1.0,
                    Duration::ZERO,
                );
                state
            })
            .collect::<Vec<_>>();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let stop = CancellationToken::new();
        let server = tokio::spawn(serve_http(
            listener,
            http::router_targets(states),
            stop.clone(),
        ));
        let socket = TcpSocket::new_v4().unwrap();
        socket.set_recv_buffer_size(1024).unwrap();
        let mut peer = socket.connect(address).await.unwrap();
        peer.write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut first = [0; 256];
        let length = timeout(Duration::from_secs(3), peer.read(&mut first))
            .await
            .unwrap()
            .unwrap();
        assert!(first[..length].starts_with(b"HTTP/1.1 200"));
        // Keep the tiny receive window full while a multi-megabyte response remains.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!server.is_finished());
        let started = Instant::now();
        stop.cancel();
        timeout(Duration::from_secs(3), server)
            .await
            .expect("HTTP drain exceeded stop budget")
            .unwrap()
            .unwrap();
        eprintln!("stalled HTTP drain completed in {:?}", started.elapsed());
        drop(peer);
    }
    #[tokio::test]
    async fn shutdown_drains_a_reading_client() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let stop = CancellationToken::new();
        let router = axum::Router::new().route(
            "/fixture",
            axum::routing::get(|| async { "x".repeat(4 * 1024 * 1024) }),
        );
        let server = tokio::spawn(serve_http(listener, router, stop.clone()));
        let socket = TcpSocket::new_v4().unwrap();
        let mut peer = socket.connect(address).await.unwrap();
        peer.write_all(b"GET /fixture HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut prefix = [0; 256];
        let length = peer.read(&mut prefix).await.unwrap();
        assert!(prefix[..length].starts_with(b"HTTP/1.1 200"));
        stop.cancel();
        let mut rest = Vec::new();
        timeout(Duration::from_secs(2), peer.read_to_end(&mut rest))
            .await
            .unwrap()
            .unwrap();
        let mut response = prefix[..length].to_vec();
        response.extend_from_slice(&rest);
        let body = response
            .windows(4)
            .position(|bytes| bytes == b"\r\n\r\n")
            .unwrap()
            + 4;
        assert_eq!(&response[body..], vec![b'x'; 4 * 1024 * 1024]);
        timeout(Duration::from_secs(1), server)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}
