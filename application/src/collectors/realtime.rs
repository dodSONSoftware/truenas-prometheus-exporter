use std::collections::HashSet;

use serde::{
    Deserialize,
    de::{MapAccess, Visitor},
};
use serde_json::value::RawValue;

use crate::state::Reason;

#[derive(Clone)]
pub struct CpuUsage {
    pub name: String,
    pub ratio: f64,
}
#[derive(Clone)]
pub struct Memory {
    pub available: u64,
}
#[derive(Clone)]
pub struct Interface {
    pub name: String,
    pub up: bool,
    pub speed: Option<f64>,
    pub receive: Option<f64>,
    pub transmit: Option<f64>,
}
#[derive(Clone)]
pub struct ArcStats {
    pub size: u64,
    pub data_hit: Option<f64>,
    pub metadata_hit: Option<f64>,
}

pub struct Frame {
    pub cpu: Result<Vec<CpuUsage>, Reason>,
    pub memory: Result<Memory, Reason>,
    pub network: Result<Vec<Interface>, Reason>,
    pub arc: Result<ArcStats, Reason>,
}

fn entries(raw: &str, maximum: usize) -> Result<Vec<(String, &RawValue)>, Reason> {
    struct Limited(usize);
    impl<'de> Visitor<'de> for Limited {
        type Value = Vec<(String, &'de RawValue)>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a bounded object")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut entries = Vec::new();
            let mut names = HashSet::new();
            while let Some((name, value)) = map.next_entry::<String, &RawValue>()? {
                if entries.len() == self.0 || !names.insert(name.clone()) {
                    return Err(serde::de::Error::custom(
                        "invalid object size or duplicate key",
                    ));
                }
                entries.push((name, value));
            }
            Ok(entries)
        }
    }
    let mut decoder = serde_json::Deserializer::from_str(raw);
    let result = serde::Deserializer::deserialize_map(&mut decoder, Limited(maximum))
        .map_err(|_| Reason::Schema)?;
    decoder.end().map_err(|_| Reason::Schema)?;
    Ok(result)
}

#[derive(Deserialize)]
struct Fields<'a> {
    failed_to_connect: Option<bool>,
    #[serde(borrow)]
    cpu: Option<&'a RawValue>,
    #[serde(borrow)]
    memory: Option<&'a RawValue>,
    #[serde(borrow)]
    interfaces: Option<&'a RawValue>,
    #[serde(borrow)]
    zfs: Option<&'a RawValue>,
}

impl Frame {
    pub fn failed(reason: Reason) -> Self {
        Self {
            cpu: Err(reason),
            memory: Err(reason),
            network: Err(reason),
            arc: Err(reason),
        }
    }
    pub fn parse(raw: &str) -> Self {
        if raw.len() > 131_072 {
            return Self::failed(Reason::Schema);
        }
        let Ok(fields) = serde_json::from_str::<Fields<'_>>(raw) else {
            return Self::failed(Reason::Schema);
        };
        if fields.failed_to_connect == Some(true) {
            return Self::failed(Reason::Unavailable);
        }
        Self {
            cpu: fields.cpu.ok_or(Reason::Schema).and_then(|v| cpu(v.get())),
            memory: fields
                .memory
                .ok_or(Reason::Schema)
                .and_then(|v| memory(v.get())),
            network: fields
                .interfaces
                .ok_or(Reason::Schema)
                .and_then(|v| network(v.get())),
            arc: fields
                .memory
                .ok_or(Reason::Schema)
                .and_then(|v| arc(v.get(), fields.zfs.map(RawValue::get))),
        }
    }
    pub fn synthetic() -> Self {
        Self {
            cpu: Ok(vec![CpuUsage {
                name: "cpu".into(),
                ratio: 0.25,
            }]),
            memory: Ok(Memory {
                available: 1_073_741_824,
            }),
            network: Ok(vec![Interface {
                name: "synthetic0".into(),
                up: true,
                speed: Some(1_000_000_000.0),
                receive: Some(1024.0),
                transmit: Some(2048.0),
            }]),
            arc: Ok(ArcStats {
                size: 536_870_912,
                data_hit: Some(0.95),
                metadata_hit: Some(0.9),
            }),
        }
    }
}

fn cpu(raw: &str) -> Result<Vec<CpuUsage>, Reason> {
    #[derive(Deserialize)]
    struct Usage {
        usage: f64,
    }
    let values = entries(raw, 257)?;
    if !values.iter().any(|(name, _)| name == "cpu") {
        return Err(Reason::Schema);
    }
    values
        .into_iter()
        .map(|(name, raw)| {
            if name != "cpu" {
                let index = name.strip_prefix("cpu").ok_or(Reason::Schema)?;
                let number = index.parse::<u16>().map_err(|_| Reason::Schema)?;
                if number >= 256 || index != number.to_string() {
                    return Err(Reason::Schema);
                }
            }
            let usage: Usage = serde_json::from_str(raw.get()).map_err(|_| Reason::Schema)?;
            let ratio = percentage(usage.usage)?;
            Ok(CpuUsage { name, ratio })
        })
        .collect()
}
fn memory(raw: &str) -> Result<Memory, Reason> {
    #[derive(Deserialize)]
    struct Physical {
        physical_memory_total: u64,
        physical_memory_available: u64,
    }
    let p: Physical = serde_json::from_str(raw).map_err(|_| Reason::Schema)?;
    if p.physical_memory_total == 0 || p.physical_memory_available > p.physical_memory_total {
        return Err(Reason::Schema);
    }
    Ok(Memory {
        available: p.physical_memory_available,
    })
}
fn arc(raw: &str, zfs: Option<&str>) -> Result<ArcStats, Reason> {
    #[derive(Deserialize)]
    struct Size {
        arc_size: u64,
    }
    #[derive(Default, Deserialize)]
    struct Hits {
        demand_data_hit_percentage: Option<f64>,
        demand_metadata_hit_percentage: Option<f64>,
    }
    let size: Size = serde_json::from_str(raw).map_err(|_| Reason::Schema)?;
    let hits: Hits = match zfs {
        Some(raw) => serde_json::from_str(raw).map_err(|_| Reason::Schema)?,
        None => Hits::default(),
    };
    Ok(ArcStats {
        size: size.arc_size,
        data_hit: hits
            .demand_data_hit_percentage
            .map(percentage)
            .transpose()?,
        metadata_hit: hits
            .demand_metadata_hit_percentage
            .map(percentage)
            .transpose()?,
    })
}
fn percentage(value: f64) -> Result<f64, Reason> {
    if !value.is_finite() || !(0.0..=100.0).contains(&value) {
        return Err(Reason::Schema);
    }
    Ok(value / 100.0)
}
fn nonnegative(value: f64) -> Result<f64, Reason> {
    if !value.is_finite() || value < 0.0 {
        Err(Reason::Schema)
    } else {
        Ok(value)
    }
}
fn network(raw: &str) -> Result<Vec<Interface>, Reason> {
    #[derive(Deserialize)]
    enum Link {
        #[serde(rename = "LINK_STATE_UP")]
        Up,
        #[serde(rename = "LINK_STATE_DOWN")]
        Down,
    }
    #[derive(Deserialize)]
    struct Wire {
        link_state: Link,
        speed: Option<f64>,
        received_bytes_rate: Option<f64>,
        sent_bytes_rate: Option<f64>,
    }
    entries(raw, 128)?
        .into_iter()
        .map(|(name, value)| {
            if name.is_empty()
                || name.len() > 64
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.:-@".contains(&b))
            {
                return Err(Reason::Schema);
            }
            let wire: Wire = serde_json::from_str(value.get()).map_err(|_| Reason::Schema)?;
            let receive = wire.received_bytes_rate.map(nonnegative).transpose()?;
            let transmit = wire.sent_bytes_rate.map(nonnegative).transpose()?;
            let speed = wire
                .speed
                .map(|n| nonnegative(n).and_then(|v| nonnegative(v * 1_000_000.0)))
                .transpose()?
                .filter(|n| *n > 0.0);
            let up = matches!(wire.link_state, Link::Up);
            Ok(Interface {
                name,
                up,
                speed: speed.filter(|_| up),
                receive: receive.filter(|_| up),
                transmit: transmit.filter(|_| up),
            })
        })
        .collect()
}
