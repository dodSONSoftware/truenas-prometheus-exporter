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
        Self::from_values(&environment_values(&[
            "TRUENAS_WS_URL",
            "TRUENAS_CA_CERT_FILE",
            "TRUENAS_API_TIMEOUT_SECONDS",
        ])?)
    }

    pub fn from_values(values: &HashMap<String, String>) -> Result<Self> {
        transport_config(
            required(values, "TRUENAS_WS_URL")?,
            values.get("TRUENAS_CA_CERT_FILE").map(Path::new),
            api_timeout(values)?,
        )
    }
}

pub struct TargetConfig {
    pub name: Option<String>,
    pub api: Option<ApiConfig>,
}

pub struct Config {
    pub system_enabled: bool,
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
        Self::from_values(&environment_values(&[
            "EXPORTER_LISTEN",
            "EXPORTER_MOCK",
            "TRUENAS_SYSTEM_ENABLED",
            "TRUENAS_POLL_INTERVAL_SECONDS",
            "TRUENAS_API_TIMEOUT_SECONDS",
            "TRUENAS_CACHE_TTL_SECONDS",
            "TRUENAS_TARGETS_FILE",
            "TRUENAS_TARGET",
            "TRUENAS_WS_URL",
            "TRUENAS_USERNAME",
            "TRUENAS_API_KEY_FILE",
            "TRUENAS_CA_CERT_FILE",
        ])?)
    }

    pub fn from_values(values: &HashMap<String, String>) -> Result<Self> {
        let system_enabled = match values.get("TRUENAS_SYSTEM_ENABLED").map(String::as_str) {
            None | Some("false") => false,
            Some("true") => true,
            _ => bail!("TRUENAS_SYSTEM_ENABLED must be true or false"),
        };
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
            load_targets(path, Duration::from_secs(timeout))?
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
            system_enabled,
            listen,
            poll_interval: Duration::from_secs(poll),
            ttl: Duration::from_secs(ttl),
            targets,
        })
    }
}

fn load_targets(path: &str, timeout: Duration) -> Result<Vec<TargetConfig>> {
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
        targets.push(TargetConfig {
            name: Some(entry.name),
            api: Some(load_api_fields(
                transport_config(
                    &entry.ws_url,
                    entry.ca_cert_file.as_deref().map(Path::new),
                    timeout,
                )?,
                &entry.username,
                Path::new(&entry.api_key_file),
            )?),
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
    load_api_fields(
        transport,
        required(values, "TRUENAS_USERNAME")?,
        Path::new(required(values, "TRUENAS_API_KEY_FILE")?),
    )
}

fn load_api_fields(transport: TransportConfig, username: &str, path: &Path) -> Result<ApiConfig> {
    ensure!(
        !username.trim().is_empty(),
        "missing required TRUENAS_USERNAME"
    );
    ensure!(
        !username.chars().any(char::is_control),
        "username contains control characters"
    );
    let key = String::from_utf8(read_config_file(path, 16384, "TRUENAS_API_KEY_FILE")?)
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
        credentials: Credentials {
            username: username.to_owned(),
            key,
        },
    })
}

fn transport_config(url: &str, ca: Option<&Path>, timeout: Duration) -> Result<TransportConfig> {
    let url =
        Url::parse(url).map_err(|_| anyhow::anyhow!("TRUENAS_WS_URL must be a valid WSS URL"))?;
    validate_ws_url(&url)?;
    Ok(TransportConfig {
        url,
        tls: tls_config(ca)?,
        timeout,
    })
}

/// Read a supported textual setting without including its value in errors.
pub fn environment_value(key: &str) -> Result<Option<String>> {
    match std::env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => bail!("{key} must contain valid Unicode"),
    }
}

fn environment_values(keys: &[&str]) -> Result<HashMap<String, String>> {
    let mut values = HashMap::new();
    for key in keys {
        if let Some(value) = environment_value(key)? {
            values.insert((*key).to_owned(), value);
        }
    }
    Ok(values)
}

pub fn listen_address(value: Option<&str>) -> Result<SocketAddr> {
    let address: SocketAddr = value
        .unwrap_or("0.0.0.0:9110")
        .parse()
        .context("EXPORTER_LISTEN must be a numeric socket address")?;
    ensure!(
        address.port() != 0,
        "EXPORTER_LISTEN port must be between 1 and 65535"
    );
    Ok(address)
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

pub(crate) fn validate_ws_url(url: &Url) -> Result<()> {
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
    Ok(())
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
