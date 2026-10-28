use std::collections::HashSet;

use serde::Deserialize;
use serde_json::Value;

use crate::state::Reason;

#[derive(Clone, Deserialize)]
pub struct Pool {
    pub name: String,
    pub size: Option<u64>,
    pub allocated: Option<u64>,
    pub free: Option<u64>,
    pub healthy: Option<bool>,
    pub warning: Option<bool>,
    pub status: Option<String>,
    pub status_code: Option<String>,
    #[serde(default, deserialize_with = "fragmentation")]
    pub fragmentation: Option<f64>,
}

fn fragmentation<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    let value = Option::<Value>::deserialize(d)?;
    match value {
        None => Ok(None),
        Some(Value::Number(n)) => n
            .as_f64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("invalid fragmentation")),
        Some(Value::String(s)) => s.parse::<f64>().map(Some).map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom("invalid fragmentation type")),
    }
}

pub fn parse(value: Value) -> Result<Vec<Pool>, Reason> {
    let pools: Vec<Pool> = serde_json::from_value(value).map_err(|_| Reason::Schema)?;
    if pools.len() > 256 {
        return Err(Reason::Schema);
    }
    let mut names = HashSet::new();
    for pool in &pools {
        if pool.name.is_empty()
            || pool.name.len() > 128
            || pool.name.chars().any(char::is_control)
            || !names.insert(&pool.name)
            || pool
                .fragmentation
                .is_some_and(|n| !n.is_finite() || !(0.0..=100.0).contains(&n))
            || pool.status.iter().chain(pool.status_code.iter()).any(|s| {
                s.is_empty()
                    || s.len() > 64
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            })
            || pool.size.is_some_and(|size| {
                pool.allocated.is_some_and(|n| n > size) || pool.free.is_some_and(|n| n > size)
            })
        {
            return Err(Reason::Schema);
        }
    }
    Ok(pools)
}

pub fn synthetic() -> Vec<Pool> {
    vec![Pool {
        name: "example-pool".into(),
        size: Some(1_000_000),
        allocated: Some(400_000),
        free: Some(600_000),
        healthy: Some(true),
        warning: Some(true),
        status: Some("ONLINE".into()),
        status_code: Some("FEAT_DISABLED".into()),
        fragmentation: Some(13.0),
    }]
}
