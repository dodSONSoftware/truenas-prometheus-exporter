use anyhow::{Context, Result, bail};
use std::{
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
    config::{Config, TransportConfig, listen_address},
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
    let filter = match std::env::var("RUST_LOG") {
        Ok(value) => {
            tracing_subscriber::EnvFilter::try_new(value).context("invalid RUST_LOG filter")?
        }
        Err(_) => tracing_subscriber::EnvFilter::new("info"),
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
    let served = axum::serve(listener, http::router_targets(states))
        .with_graceful_shutdown(stop.clone().cancelled_owned())
        .await;
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
    let mut address = listen_address(std::env::var("EXPORTER_LISTEN").ok().as_deref())?;
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
        let mut status = vec![0; 128];
        let size = stream.read(&mut status).await?;
        if !status[..size].starts_with(b"HTTP/1.1 200 ") {
            bail!("liveness check failed");
        }
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("liveness check timed out")?
}
