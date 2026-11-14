use std::{
    collections::{HashMap, HashSet},
    fs::OpenOptions,
    io::Read,
    net::SocketAddr,
    path::Path,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use rustls::{
    ClientConfig, RootCertStore,
    pki_types::{CertificateDer, pem::PemObject},
};
use serde::Deserialize;
use url::Url;

pub struct Credentials {
    pub username: String,
    pub key: String,
}

pub struct ApiConfig {
    pub url: Url,
    pub credentials: Credentials,
    pub tls: Arc<ClientConfig>,
    pub timeout: Duration,
}

pub struct TransportConfig {
    pub url: Url,
    pub tls: Arc<ClientConfig>,
    pub timeout: Duration,
}

impl TransportConfig {
    pub fn from_env() -> Result<Self> {
        Self::from_values(&std::env::vars().collect())
    }

    pub fn from_values(values: &HashMap<String, String>) -> Result<Self> {
        let url = Url::parse(required(values, "TRUENAS_WS_URL")?)
            .map_err(|_| anyhow::anyhow!("TRUENAS_WS_URL must be a valid WSS URL"))?;
        ensure!(
            url.scheme() == "wss" && url.host_str().is_some(),
            "TRUENAS_WS_URL requires wss and a host"
        );
        ensure!(
            url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "API URL must not contain credentials, query or fragment"
        );
        Ok(Self {
            url,
            tls: tls_config(values.get("TRUENAS_CA_CERT_FILE").map(Path::new))?,
            timeout: api_timeout(values)?,
        })
    }
}

pub struct TargetConfig {
    pub name: Option<String>,
    pub api: Option<ApiConfig>,
}

pub struct Config {
    pub listen: SocketAddr,
    pub poll_interval: Duration,
    pub ttl: Duration,
    pub targets: Vec<TargetConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetsFile {
    targets: Vec<TargetEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetEntry {
    name: String,
    ws_url: String,
    username: String,
    api_key_file: String,
    ca_cert_file: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Self::from_values(&std::env::vars().collect())
    }

    pub fn from_values(values: &HashMap<String, String>) -> Result<Self> {
        let listen = listen_address(values.get("EXPORTER_LISTEN").map(String::as_str))?;
        let mock = match values.get("EXPORTER_MOCK").map(String::as_str) {
            None | Some("false") => false,
            Some("true") => true,
            _ => bail!("EXPORTER_MOCK must be true or false"),
        };
        let poll = seconds(values, "TRUENAS_POLL_INTERVAL_SECONDS", 60, 86400)?;
        let timeout = api_timeout(values)?.as_secs();
        let ttl = seconds(values, "TRUENAS_CACHE_TTL_SECONDS", poll * 3, 604800)?;
        ensure!(
            ttl > poll + timeout,
            "cache TTL must exceed poll interval plus API timeout"
        );
        let targets = if let Some(path) = values.get("TRUENAS_TARGETS_FILE") {
            ensure!(
                !mock,
                "EXPORTER_MOCK cannot be combined with TRUENAS_TARGETS_FILE"
            );
            for key in [
                "TRUENAS_TARGET",
                "TRUENAS_WS_URL",
                "TRUENAS_USERNAME",
                "TRUENAS_API_KEY_FILE",
                "TRUENAS_CA_CERT_FILE",
            ] {
                ensure!(
                    !values.contains_key(key),
                    "TRUENAS_TARGETS_FILE cannot be combined with legacy target settings"
                );
            }
            load_targets(path, values)?
        } else {
            let name = values.get("TRUENAS_TARGET").cloned();
            if let Some(name) = &name {
                validate_name(name)?;
            }
            vec![TargetConfig {
                name,
                api: if mock { None } else { Some(load_api(values)?) },
            }]
        };
        Ok(Self {
            listen,
            poll_interval: Duration::from_secs(poll),
            ttl: Duration::from_secs(ttl),
            targets,
        })
    }
}

fn load_targets(path: &str, global: &HashMap<String, String>) -> Result<Vec<TargetConfig>> {
    let bytes = read_config_file(Path::new(path), 65536, "TRUENAS_TARGETS_FILE")?;
    let file: TargetsFile = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("invalid TRUENAS_TARGETS_FILE JSON/schema"))?;
    ensure!(
        !file.targets.is_empty() && file.targets.len() <= 16,
        "configure between 1 and 16 targets"
    );
    let mut names = HashSet::new();
    let mut targets = Vec::new();
    for entry in file.targets {
        validate_name(&entry.name)?;
        ensure!(
            names.insert(entry.name.clone()),
            "target names must be unique"
        );
        ensure!(
            Path::new(&entry.api_key_file).is_absolute(),
            "target API key paths must be absolute"
        );
        if let Some(ca) = &entry.ca_cert_file {
            ensure!(
                Path::new(ca).is_absolute(),
                "target CA paths must be absolute"
            );
        }
        let mut values = global.clone();
        values.insert("TRUENAS_WS_URL".into(), entry.ws_url);
        values.insert("TRUENAS_USERNAME".into(), entry.username);
        values.insert("TRUENAS_API_KEY_FILE".into(), entry.api_key_file);
        if let Some(ca) = entry.ca_cert_file {
            values.insert("TRUENAS_CA_CERT_FILE".into(), ca);
        }
        targets.push(TargetConfig {
            name: Some(entry.name),
            api: Some(load_api(&values)?),
        });
    }
    Ok(targets)
}

fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.')),
        "target names must be 1..64 ASCII letters, digits, dots, underscores or hyphens"
    );
    Ok(())
}

fn load_api(values: &HashMap<String, String>) -> Result<ApiConfig> {
    let transport = TransportConfig::from_values(values)?;
    let username = required(values, "TRUENAS_USERNAME")?.to_owned();
    ensure!(
        !username.chars().any(char::is_control),
        "username contains control characters"
    );
    let path = required(values, "TRUENAS_API_KEY_FILE")?;
    let key = String::from_utf8(read_config_file(
        Path::new(path),
        16384,
        "TRUENAS_API_KEY_FILE",
    )?)
    .map_err(|_| anyhow::anyhow!("TRUENAS_API_KEY_FILE must contain UTF-8"))?;
    let key = key.trim_end_matches(['\r', '\n']).to_owned();
    ensure!(
        !key.is_empty() && !key.chars().any(char::is_control),
        "API key must be nonempty and contain no control characters"
    );
    Ok(ApiConfig {
        url: transport.url,
        tls: transport.tls,
        timeout: transport.timeout,
        credentials: Credentials { username, key },
    })
}

pub fn listen_address(value: Option<&str>) -> Result<SocketAddr> {
    value
        .unwrap_or("0.0.0.0:9110")
        .parse()
        .context("EXPORTER_LISTEN must be a numeric socket address")
}

fn required<'a>(values: &'a HashMap<String, String>, key: &str) -> Result<&'a str> {
    values
        .get(key)
        .map(String::as_str)
        .filter(|s| !s.trim().is_empty())
        .with_context(|| format!("missing required {key}"))
}

fn seconds(values: &HashMap<String, String>, key: &str, default: u64, max: u64) -> Result<u64> {
    let value = match values.get(key) {
        Some(s) => s
            .parse()
            .with_context(|| format!("{key} must be an integer"))?,
        None => default,
    };
    ensure!(
        value > 0 && value <= max,
        "{key} is outside its allowed range"
    );
    Ok(value)
}

pub fn tls_config(ca: Option<&Path>) -> Result<Arc<ClientConfig>> {
    let mut roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if let Some(path) = ca {
        let data = read_config_file(path, 1024 * 1024, "TRUENAS_CA_CERT_FILE")?;
        let certs = CertificateDer::pem_slice_iter(&data)
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("invalid CA PEM")?;
        ensure!(!certs.is_empty(), "CA file contains no certificates");
        for cert in certs {
            roots.add(cert).context("invalid CA certificate")?;
        }
    }
    Ok(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ))
}

fn api_timeout(values: &HashMap<String, String>) -> Result<Duration> {
    Ok(Duration::from_secs(seconds(
        values,
        "TRUENAS_API_TIMEOUT_SECONDS",
        10,
        300,
    )?))
}

// Nonblocking open prevents a FIFO without a writer from stalling before we can
// inspect the opened handle. Symlinks to ordinary mounted files remain supported.
fn read_config_file(path: &Path, limit: usize, setting: &str) -> Result<Vec<u8>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .with_context(|| format!("cannot open {setting} file"))?;
    ensure!(
        file.metadata()
            .with_context(|| format!("cannot inspect {setting} file"))?
            .is_file(),
        "{setting} must refer to a regular file"
    );
    read_bounded(file, limit, setting)
}

fn read_bounded(reader: impl Read, limit: usize, setting: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("cannot read {setting} file"))?;
    ensure!(bytes.len() <= limit, "{setting} file exceeds {limit} bytes");
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct GrowingSource(usize);
    impl Read for GrowingSource {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            buf.fill(b'x');
            self.0 += buf.len();
            Ok(buf.len())
        }
    }

    #[test]
    fn bounded_read_stops_a_growing_source() {
        let mut reader = GrowingSource(0);
        assert!(read_bounded(&mut reader, 8, "fixture").is_err());
        assert_eq!(reader.0, 9);
    }
}
