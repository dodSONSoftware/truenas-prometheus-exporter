use crate::collectors::pools::Pool;
use prometheus_client::{
    encoding::{EncodeLabelValue, LabelValueEncoder, text::encode},
    metrics::{counter::Counter, family::Family, gauge::Gauge},
    registry::Registry,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    Unavailable,
    Timeout,
    Transport,
    Tls,
    Auth,
    Expired,
    Otp,
    Redirect,
    Permission,
    Unsupported,
    Api,
    Protocol,
    Schema,
    Busy,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::Transport => "transport",
            Self::Tls => "tls",
            Self::Auth => "auth",
            Self::Expired => "expired",
            Self::Otp => "otp",
            Self::Redirect => "redirect",
            Self::Permission => "permission",
            Self::Unsupported => "unsupported",
            Self::Api => "api",
            Self::Protocol => "protocol",
            Self::Schema => "schema",
            Self::Busy => "busy",
        }
    }
}

#[derive(Clone)]
struct Snapshot {
    pools: Vec<Pool>,
    at: Instant,
    epoch: f64,
}
#[derive(Clone, Default)]
struct Collection {
    snapshot: Option<Snapshot>,
    attempt: Option<(f64, f64)>,
    success: bool,
    errors: BTreeMap<Reason, u64>,
}

pub struct State {
    pub connected: AtomicBool,
    pub mock: bool,
    target: Option<String>,
    ttl: Duration,
    collection: RwLock<Collection>,
    api_errors: RwLock<BTreeMap<Reason, u64>>,
}

type FloatGauge = Gauge<f64, std::sync::atomic::AtomicU64>;
type Labels = Vec<(String, String)>;
type MetricLabels = Vec<(String, LabelValue)>;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct LabelValue(String);

impl EncodeLabelValue for LabelValue {
    fn encode(&self, encoder: &mut LabelValueEncoder<'_>) -> Result<(), std::fmt::Error> {
        use std::fmt::Write;
        // prometheus-client's string encoder writes raw values; escape at this boundary.
        for character in self.0.chars() {
            match character {
                '\\' => encoder.write_str("\\\\")?,
                '"' => encoder.write_str("\\\"")?,
                '\n' => encoder.write_str("\\n")?,
                c => encoder.write_char(c)?,
            }
        }
        Ok(())
    }
}

fn metric_labels(labels: Labels) -> MetricLabels {
    labels
        .into_iter()
        .map(|(key, value)| (key, LabelValue(value)))
        .collect()
}

#[derive(Default)]
struct Metrics {
    registry: Registry,
    target: Option<String>,
    gauges: BTreeMap<&'static str, Family<MetricLabels, FloatGauge>>,
    counters: BTreeMap<&'static str, Family<MetricLabels, Counter>>,
}

impl Metrics {
    fn gauge_family(
        &mut self,
        name: &'static str,
        help: &'static str,
        values: impl IntoIterator<Item = (Labels, f64)>,
    ) {
        let family = self.gauges.entry(name).or_insert_with(|| {
            let family = Family::default();
            self.registry.register(name, help, family.clone());
            family
        });
        for (mut labels, value) in values {
            if let Some(target) = &self.target {
                labels.insert(0, ("target".into(), target.clone()));
            }
            family.get_or_create(&metric_labels(labels)).set(value);
        }
    }

    fn counter_family(
        &mut self,
        name: &'static str,
        help: &'static str,
        values: impl IntoIterator<Item = (Labels, u64)>,
    ) {
        let family = self.counters.entry(name).or_insert_with(|| {
            let family = Family::default();
            self.registry.register(name, help, family.clone());
            family
        });
        for (mut labels, value) in values {
            if let Some(target) = &self.target {
                labels.insert(0, ("target".into(), target.clone()));
            }
            family.get_or_create(&metric_labels(labels)).inc_by(value);
        }
    }

    fn encode(&self) -> Result<String, std::fmt::Error> {
        let mut text = String::new();
        encode(&mut text, &self.registry)?;
        Ok(text)
    }
}

pub fn metrics_for_targets(states: &[Arc<State>], now: Instant) -> Result<String, std::fmt::Error> {
    let mut metrics = Metrics::default();
    for state in states {
        state.record_metrics(&mut metrics, now);
    }
    metrics.encode()
}

impl State {
    pub fn new(ttl: Duration, mock: bool) -> Arc<Self> {
        Self::create(None, ttl, mock)
    }

    pub fn for_target(name: String, ttl: Duration, mock: bool) -> Arc<Self> {
        Self::create(Some(name), ttl, mock)
    }

    fn create(target: Option<String>, ttl: Duration, mock: bool) -> Arc<Self> {
        Arc::new(Self {
            target,
            connected: AtomicBool::new(false),
            mock,
            ttl,
            collection: RwLock::new(Collection::default()),
            api_errors: RwLock::new(BTreeMap::new()),
        })
    }
    pub fn connection_failed(&self, reason: Reason) {
        *self
            .api_errors
            .write()
            .expect("API error lock poisoned")
            .entry(reason)
            .or_default() += 1;
    }
    pub fn complete(
        &self,
        result: Result<Vec<Pool>, Reason>,
        now: Instant,
        epoch: f64,
        duration: Duration,
    ) {
        let mut c = self.collection.write().expect("state lock poisoned");
        c.attempt = Some((epoch, duration.as_secs_f64()));
        match result {
            Ok(pools) => {
                c.snapshot = Some(Snapshot {
                    pools,
                    at: now,
                    epoch: epoch + duration.as_secs_f64(),
                });
                c.success = true;
            }
            Err(reason) => {
                c.success = false;
                *c.errors.entry(reason).or_default() += 1;
            }
        }
    }
    pub fn ready_at(&self, now: Instant) -> bool {
        self.collection
            .read()
            .expect("state lock poisoned")
            .snapshot
            .as_ref()
            .is_some_and(|s| now.saturating_duration_since(s.at) < self.ttl)
    }
    pub fn metrics_at(&self, now: Instant) -> Result<String, std::fmt::Error> {
        let mut registry = Metrics::default();
        self.record_metrics(&mut registry, now);
        registry.encode()
    }

    fn record_metrics(&self, registry: &mut Metrics, now: Instant) {
        let c = self.collection.read().expect("state lock poisoned").clone();
        registry.target = self.target.clone();
        scalar(
            registry,
            "truenas_api_connected",
            "Authenticated API transport connected",
            f64::from(self.connected.load(Ordering::Relaxed)),
        );
        scalar(
            registry,
            "truenas_exporter_mock_mode",
            "Synthetic offline mode enabled",
            f64::from(self.mock),
        );
        let api_errors = self
            .api_errors
            .read()
            .expect("API error lock poisoned")
            .clone();
        registry.counter_family(
            "truenas_exporter_connection_errors",
            "Failed connection or authentication attempts",
            api_errors
                .into_iter()
                .map(|(reason, count)| (vec![("reason".into(), reason.as_str().into())], count)),
        );
        let labels = vec![("collector".into(), "pools".into())];
        let age = c
            .snapshot
            .as_ref()
            .map(|s| now.saturating_duration_since(s.at));
        let fresh = age.is_some_and(|age| age < self.ttl);
        family(
            registry,
            "truenas_exporter_collection_success",
            "Latest attempt succeeded and snapshot is fresh",
            [(labels.clone(), f64::from(c.success && fresh))],
        );
        if let Some((epoch, duration)) = c.attempt {
            family(
                registry,
                "truenas_exporter_last_attempt_timestamp_seconds",
                "Latest collection attempt epoch",
                [(labels.clone(), epoch)],
            );
            family(
                registry,
                "truenas_exporter_collection_duration_seconds",
                "Latest attempt duration",
                [(labels.clone(), duration)],
            );
        }
        registry.counter_family(
            "truenas_exporter_collection_errors",
            "Failed collection attempts",
            c.errors.iter().map(|(reason, count)| {
                let mut ls = labels.clone();
                ls.push(("reason".into(), reason.as_str().into()));
                (ls, *count)
            }),
        );
        if let Some(snapshot) = &c.snapshot {
            family(
                registry,
                "truenas_exporter_last_success_timestamp_seconds",
                "Latest valid snapshot epoch",
                [(labels.clone(), snapshot.epoch)],
            );
            family(
                registry,
                "truenas_exporter_snapshot_age_seconds",
                "Monotonic age of latest valid snapshot",
                [(labels, age.expect("snapshot age").as_secs_f64())],
            );
            if fresh {
                pool_metrics(registry, &snapshot.pools);
            }
        }
    }
}

fn scalar(registry: &mut Metrics, name: &'static str, help: &'static str, value: f64) {
    if registry.target.is_some() {
        registry.gauge_family(name, help, [(Vec::new(), value)]);
    } else {
        let gauge = FloatGauge::default();
        gauge.set(value);
        registry.registry.register(name, help, gauge);
    }
}

fn family(
    registry: &mut Metrics,
    name: &'static str,
    help: &'static str,
    values: impl IntoIterator<Item = (Labels, f64)>,
) {
    registry.gauge_family(name, help, values);
}

fn pool_metrics(registry: &mut Metrics, pools: &[Pool]) {
    for (name, help, field) in [
        ("truenas_pool_size_bytes", "Pool total size in bytes", 0),
        (
            "truenas_pool_allocated_bytes",
            "Pool allocated space in bytes",
            1,
        ),
        ("truenas_pool_free_bytes", "Pool free space in bytes", 2),
        ("truenas_pool_healthy", "TrueNAS pool healthy flag", 3),
        ("truenas_pool_warning", "TrueNAS pool warning flag", 4),
        (
            "truenas_pool_fragmentation_percent",
            "Pool fragmentation percentage",
            5,
        ),
    ] {
        family(
            registry,
            name,
            help,
            pools.iter().filter_map(|p| {
                let value = match field {
                    0 => p.size.map(|n| n as f64),
                    1 => p.allocated.map(|n| n as f64),
                    2 => p.free.map(|n| n as f64),
                    3 => p.healthy.map(f64::from),
                    4 => p.warning.map(f64::from),
                    _ => p.fragmentation,
                }?;
                Some((vec![("pool".into(), p.name.clone())], value))
            }),
        );
    }
    family(
        registry,
        "truenas_pool_info",
        "Pool condition metadata",
        pools.iter().map(|p| {
            (
                vec![
                    ("pool".into(), p.name.clone()),
                    ("status".into(), pool_status(p.status.as_deref()).into()),
                    (
                        "status_code".into(),
                        pool_status_code(p.status_code.as_deref()).into(),
                    ),
                ],
                1.0,
            )
        }),
    );
}

fn pool_status(value: Option<&str>) -> &str {
    match value {
        Some(
            status @ ("ONLINE" | "DEGRADED" | "FAULTED" | "OFFLINE" | "UNAVAIL" | "REMOVED"
            | "SUSPENDED"),
        ) => status,
        _ => "UNKNOWN",
    }
}

fn pool_status_code(value: Option<&str>) -> &str {
    match value {
        Some("FEAT_DISABLED") => "FEAT_DISABLED",
        None => "UNKNOWN",
        Some(_) => "OTHER",
    }
}
