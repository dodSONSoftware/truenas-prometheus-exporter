use serde::Deserialize;
use serde_json::Value;

use crate::state::Reason;

/// Fields used by the system preflight. Serial numbers and license data are ignored.
#[derive(Deserialize)]
pub struct SystemInfo {
    pub version: String,
    pub hostname: String,
    pub uptime_seconds: f64,
    pub physmem: Option<u64>,
    pub loadavg: Option<[f64; 3]>,
}

pub fn parse(value: Value) -> Result<SystemInfo, Reason> {
    let info: SystemInfo = serde_json::from_value(value).map_err(|_| Reason::Schema)?;
    if info.hostname.is_empty()
        || info.hostname.len() > 253
        || info.hostname.chars().any(char::is_control)
        || info.hostname.chars().any(char::is_whitespace)
        || !info.uptime_seconds.is_finite()
        || info.uptime_seconds < 0.0
        || info.physmem == Some(0)
        || info
            .loadavg
            .is_some_and(|values| values.iter().any(|n| !n.is_finite() || *n < 0.0))
    {
        return Err(Reason::Schema);
    }
    supported_version(&info.version)?;
    Ok(info)
}

fn supported_version(version: &str) -> Result<(), Reason> {
    // system.info uses version_short, not the branded system.version string.
    if version.is_empty()
        || version.len() > 64
        || !version.bytes().all(|c| c.is_ascii_digit() || c == b'.')
    {
        return Err(Reason::Schema);
    }
    let parts: Vec<_> = version.split('.').collect();
    if !(3..=4).contains(&parts.len()) || parts.iter().any(|part| part.parse::<u32>().is_err()) {
        return Err(Reason::Schema);
    }
    if parts[..3] != ["25", "04", "2"] {
        return Err(Reason::Unsupported);
    }
    Ok(())
}
