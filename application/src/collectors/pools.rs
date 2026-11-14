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

const MAX_POOLS: usize = 256;

// Count borrowed element slices before allocating any pool objects. The envelope
// decoder has already validated JSON syntax across the entire raw result.
pub(crate) fn bounded_result(raw: &str) -> Result<Value, Reason> {
    use serde::de::{SeqAccess, Visitor};
    use serde_json::value::RawValue;

    struct PoolList;
    impl<'de> Visitor<'de> for PoolList {
        type Value = Vec<&'de RawValue>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an array of up to 256 pools")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut entries = Vec::new();
            while let Some(entry) = seq.next_element()? {
                if entries.len() == MAX_POOLS {
                    return Err(serde::de::Error::custom("too many pools"));
                }
                entries.push(entry);
            }
            Ok(entries)
        }
    }
    let entries = serde::Deserializer::deserialize_seq(
        &mut serde_json::Deserializer::from_str(raw),
        PoolList,
    )
    .map_err(|_| Reason::Schema)?;
    entries
        .into_iter()
        .map(|entry| serde_json::from_str(entry.get()).map_err(|_| Reason::Schema))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

pub fn parse(value: Value) -> Result<Vec<Pool>, Reason> {
    if !value
        .as_array()
        .is_some_and(|entries| entries.len() <= MAX_POOLS)
    {
        return Err(Reason::Schema);
    }
    let pools: Vec<Pool> = serde_json::from_value(value).map_err(|_| Reason::Schema)?;
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
