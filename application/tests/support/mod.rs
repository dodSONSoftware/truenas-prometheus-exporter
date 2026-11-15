use anyhow::{Context, Result, bail};
use std::{net::SocketAddr, process::Stdio, time::Duration};
use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
    time::timeout,
};

pub fn unused_address() -> SocketAddr {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}

pub struct StartedExporter {
    pub child: Child,
    pub address: SocketAddr,
    pub startup_stdout: Vec<u8>,
}

/// Wait for the info-level startup line before callers send HTTP probes.
/// Return consumed stdout so redaction assertions still inspect every log byte.
pub async fn start_exporter(command: &mut Command) -> Result<StartedExporter> {
    start_exporter_with_ports(command, unused_address).await
}

/// Port selection cannot reserve a child listener atomically. Retry only the
/// confirmed bind collision, never an assertion or an unrelated startup error.
pub async fn start_exporter_with_ports(
    command: &mut Command,
    mut next_address: impl FnMut() -> SocketAddr,
) -> Result<StartedExporter> {
    timeout(Duration::from_secs(5), async {
        for attempt in 1..=3 {
            let address = next_address();
            let mut child = command
                .env("EXPORTER_LISTEN", address.to_string())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .context("spawn synthetic exporter")?;
            let mut startup_stdout = Vec::new();
            let stdout = child.stdout.as_mut().unwrap();
            loop {
                match stdout.read_u8().await {
                    Ok(byte) => {
                        startup_stdout.push(byte);
                        if startup_stdout.len() > 64 * 1024 {
                            bail!("exporter startup output exceeds 64 KiB");
                        }
                        if byte == b'\n'
                            && String::from_utf8_lossy(&startup_stdout)
                                .contains("Exporter HTTP server starting")
                        {
                            return Ok(StartedExporter {
                                child,
                                address,
                                startup_stdout,
                            });
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(error) => return Err(error.into()),
                }
            }
            let output = child.wait_with_output().await?;
            let stderr = String::from_utf8_lossy(&output.stderr);
            let bind_collision = output.status.code() == Some(1)
                && stderr.starts_with("Error: cannot bind exporter listener\n")
                && stderr.contains("Address already in use (os error ");
            if !bind_collision || attempt == 3 {
                bail!(
                    "exporter exited before HTTP startup ({}): {stderr}",
                    output.status
                );
            }
        }
        unreachable!()
    })
    .await
    .context("exporter startup exceeded five seconds")?
}
