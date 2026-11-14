use serde_json::{Value, json};
use std::{collections::HashMap, time::Duration};
use tokio::time::Instant;
use truenas_exporter::{
    collectors::realtime::Frame,
    config::{Config, RealtimeConfig},
    state::{Reason, State},
};
fn fields() -> Value {
    serde_json::from_str::<Value>(include_str!("fixtures/realtime.synthetic.json")).unwrap()["params"]["fields"].clone()
}
#[test]
fn selected_fields_convert_once_and_down_link_rates_stay_absent() {
    let frame = Frame::parse(&fields().to_string());
    let cpu = frame.cpu.unwrap();
    assert_eq!(cpu[0].ratio, 0.25);
    assert_eq!(cpu[1].ratio, 0.75);
    assert_eq!(frame.memory.unwrap().available, 1_073_741_824);
    let arc = frame.arc.unwrap();
    assert_eq!(arc.size, 536_870_912);
    assert_eq!(arc.data_hit, Some(0.95));
    let network = frame.network.unwrap();
    assert_eq!(network[0].speed, Some(1_000_000_000.0));
    assert_eq!(network[0].receive, Some(1024.0));
    assert!(!network[1].up);
    assert_eq!(network[1].receive, None);
    assert_eq!(network[1].speed, None);
    let mut value = fields();
    value["interfaces"]["synthetic0"]
        .as_object_mut()
        .unwrap()
        .remove("received_bytes_rate");
    assert_eq!(
        Frame::parse(&value.to_string()).network.unwrap()[0].receive,
        None
    );
    value["interfaces"]["synthetic1"]["received_bytes_rate"] = json!(0);
    assert_eq!(
        Frame::parse(&value.to_string()).network.unwrap()[1].receive,
        None
    );
}
#[test]
fn invalid_groups_do_not_invalidate_valid_siblings() {
    let mut value = fields();
    value["cpu"]["cpu"]["usage"] = json!(101);
    let frame = Frame::parse(&value.to_string());
    assert!(frame.cpu.is_err());
    assert!(frame.memory.is_ok());
    assert!(frame.network.is_ok());
    assert!(frame.arc.is_ok());
    value = fields();
    value["memory"]["physical_memory_available"] = json!(5_000_000_000_u64);
    let frame = Frame::parse(&value.to_string());
    assert!(frame.memory.is_err());
    assert!(frame.arc.is_ok());
    value = fields();
    value["memory"]["arc_size"] = json!(-1);
    let frame = Frame::parse(&value.to_string());
    assert!(frame.memory.is_ok());
    assert!(frame.arc.is_err());
    value = fields();
    value["interfaces"]["synthetic0"]["received_bytes_rate"] = json!(-1);
    assert!(Frame::parse(&value.to_string()).network.is_err());
    value = fields();
    value["zfs"] = Value::Null;
    assert!(Frame::parse(&value.to_string()).arc.is_ok());
    let frame = Frame::parse("{\"failed_to_connect\":true}");
    assert_eq!(frame.cpu.err(), Some(Reason::Unavailable));
    assert!(frame.memory.is_err() && frame.arc.is_err() && frame.network.is_err());
    let frame = Frame::parse("{}");
    assert!(
        frame.cpu.is_err() && frame.memory.is_err() && frame.network.is_err() && frame.arc.is_err()
    );
}
#[test]
fn cardinality_names_and_types_are_bounded() {
    let mut value = fields();
    value["cpu"] = json!({"cpu":{"usage":0},"cpu256":{"usage":0}});
    assert!(Frame::parse(&value.to_string()).cpu.is_err());
    value = fields();
    value["cpu"] = json!({"cpu":{"usage":0},"cpu01":{"usage":0}});
    assert!(Frame::parse(&value.to_string()).cpu.is_err());
    value = fields();
    value["interfaces"] = Value::Object(
        (0..129)
            .map(|n| (format!("test{n}"), json!({"link_state":"LINK_STATE_UP"})))
            .collect(),
    );
    assert!(Frame::parse(&value.to_string()).network.is_err());
    value["interfaces"] = json!({"bad name":{"link_state":"LINK_STATE_UP"}});
    assert!(Frame::parse(&value.to_string()).network.is_err());
    value["interfaces"] = json!({"test":{"link_state":"UNKNOWN"}});
    assert!(Frame::parse(&value.to_string()).network.is_err());
    value["interfaces"] = json!({"test":{"link_state":"LINK_STATE_UP","speed":1e308}});
    assert!(Frame::parse(&value.to_string()).network.is_err());
    value = fields();
    value["cpu"]["cpu"]["usage"] = json!("25");
    assert!(Frame::parse(&value.to_string()).cpu.is_err());
    assert!(Frame::parse(&" ".repeat(131073)).cpu.is_err());
    let frame = Frame::parse("{\"cpu\":{\"cpu\":{\"usage\":1},\"cpu\":{\"usage\":2}}}");
    assert!(frame.cpu.is_err());
}
#[tokio::test(start_paused = true)]
async fn independent_event_ttls_do_not_refresh_bad_groups_or_duplicate_total_memory() {
    let rt = RealtimeConfig {
        interval: Duration::from_secs(2),
        ttl: Duration::from_secs(6),
    };
    let state = State::with_realtime(
        Some("alpha".into()),
        Duration::from_secs(30),
        true,
        true,
        Some(rt),
    );
    let start = Instant::now();
    state.complete_realtime(
        Frame::parse(&fields().to_string()),
        start,
        100.0,
        Duration::ZERO,
    );
    assert!(state.realtime_ready_at(start));
    let mut partial = fields();
    partial["cpu"] = Value::Null;
    state.complete_realtime(
        Frame::parse(&partial.to_string()),
        start + Duration::from_secs(4),
        104.0,
        Duration::ZERO,
    );
    assert!(state.realtime_ready_at(start + Duration::from_secs(5)));
    assert!(!state.realtime_ready_at(start + Duration::from_secs(6)));
    let metrics = state.metrics_at(start + Duration::from_secs(6)).unwrap();
    assert!(
        !metrics
            .lines()
            .any(|l| l.starts_with("truenas_cpu_utilization_ratio{"))
    );
    assert!(metrics.contains("truenas_memory_available_bytes{target=\"alpha\"} 1073741824"));
    assert!(
        !metrics
            .lines()
            .any(|l| l.starts_with("truenas_memory_total_bytes"))
    );
    assert!(metrics.contains("collector=\"cpu\",reason=\"schema\""));
    state.complete_realtime(
        Frame::parse(&fields().to_string()),
        start + Duration::from_secs(7),
        107.0,
        Duration::ZERO,
    );
    assert!(state.realtime_ready_at(start + Duration::from_secs(7)));
    let mut empty = fields();
    empty["interfaces"] = json!({});
    state.complete_realtime(
        Frame::parse(&empty.to_string()),
        start + Duration::from_secs(8),
        108.0,
        Duration::ZERO,
    );
    assert!(
        !state
            .metrics_at(start + Duration::from_secs(8))
            .unwrap()
            .lines()
            .any(|l| l.starts_with("truenas_network_link_up{"))
    );
}
#[test]
fn config_is_opt_in_dependency_checked_and_interval_ttl_bounded() {
    assert!(
        RealtimeConfig {
            interval: Duration::from_millis(2500),
            ttl: Duration::from_secs(6)
        }
        .validate()
        .is_err()
    );
    let mut vars = HashMap::from([("EXPORTER_MOCK".into(), "true".into())]);
    assert!(Config::from_values(&vars).unwrap().realtime.is_none());
    vars.insert("TRUENAS_REALTIME_ENABLED".into(), "true".into());
    assert!(Config::from_values(&vars).is_err());
    vars.insert("TRUENAS_SYSTEM_ENABLED".into(), "true".into());
    assert_eq!(
        Config::from_values(&vars)
            .unwrap()
            .realtime
            .unwrap()
            .ttl
            .as_secs(),
        6
    );
    for (key, value) in [
        ("TRUENAS_REALTIME_INTERVAL_SECONDS", "1"),
        ("TRUENAS_REALTIME_INTERVAL_SECONDS", "301"),
        ("TRUENAS_REALTIME_TTL_SECONDS", "2"),
        ("TRUENAS_REALTIME_ENABLED", "yes"),
    ] {
        let mut invalid = vars.clone();
        invalid.insert(key.into(), value.into());
        assert!(Config::from_values(&invalid).is_err());
    }
}
