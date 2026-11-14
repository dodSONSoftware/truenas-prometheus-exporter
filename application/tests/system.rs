use serde_json::{Value, json};
use truenas_exporter::{collectors::system, state::Reason};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/system.synthetic.json")).unwrap()
}

#[test]
fn system_sources_preserve_units_and_optional_absence() {
    let info = system::parse(fixture()).unwrap();
    assert_eq!(info.hostname, "synthetic-nas.example.invalid");
    assert_eq!(info.version, "25.04.2.6");
    assert_eq!(info.uptime_seconds, 12345.75);
    assert_eq!(info.physmem, Some(34_359_738_368));
    assert_eq!(info.loadavg, Some([0.25, 1.5, 2.75]));
    for nulls in [false, true] {
        let mut value = fixture();
        for field in ["physmem", "loadavg"] {
            if nulls {
                value[field] = Value::Null;
            } else {
                value.as_object_mut().unwrap().remove(field);
            }
        }
        let info = system::parse(value).unwrap();
        assert_eq!(info.physmem, None);
        assert_eq!(info.loadavg, None);
    }
}

#[test]
fn system_version_gate_rejects_unknown_families_and_malformed_versions() {
    for version in ["25.04.2", "25.04.2.6", "25.04.2.100"] {
        let mut value = fixture();
        value["version"] = json!(version);
        assert!(system::parse(value).is_ok());
    }
    for version in [
        "24.10.2", "25.04.1", "25.04.20", "25.10.2", "26.04.2", "25.4.2",
    ] {
        let mut value = fixture();
        value["version"] = json!(version);
        assert_eq!(system::parse(value).err(), Some(Reason::Unsupported));
    }
    for version in [
        "",
        "25.04",
        "TrueNAS-25.04.2.6",
        "25.04.2-MASTER",
        "25.04.2.6.extra",
        "25.04.2.",
        "25.04.2.99999999999999",
        "25.04.2.6\n",
    ] {
        let mut value = fixture();
        value["version"] = json!(version);
        assert_eq!(system::parse(value).err(), Some(Reason::Schema));
    }
}

#[test]
fn system_invalid_values_fail_without_coercion_or_default_zeros() {
    for (field, invalid) in [
        ("hostname", json!("")),
        ("hostname", json!("bad host")),
        ("hostname", json!("bad\nhost")),
        ("hostname", json!("x".repeat(254))),
        ("uptime_seconds", json!(-1)),
        ("uptime_seconds", json!("123")),
        ("physmem", json!(0)),
        ("physmem", json!(-1)),
        ("physmem", json!(1.5)),
        ("physmem", json!("34359738368")),
        ("physmem", json!(true)),
        ("loadavg", json!([0.0, -1.0, 2.0])),
        ("loadavg", json!([1, 2])),
        ("loadavg", json!([1, 2, 3, 4])),
        ("loadavg", json!(["1", 2, 3])),
        ("version", Value::Null),
    ] {
        let mut value = fixture();
        value[field] = invalid;
        assert_eq!(
            system::parse(value).err(),
            Some(Reason::Schema),
            "field {field}"
        );
    }
    for field in ["hostname", "version", "uptime_seconds"] {
        let mut value = fixture();
        value.as_object_mut().unwrap().remove(field);
        assert_eq!(system::parse(value).err(), Some(Reason::Schema));
        let mut value = fixture();
        value[field] = Value::Null;
        assert_eq!(system::parse(value).err(), Some(Reason::Schema));
    }
    assert_eq!(system::parse(json!([])).err(), Some(Reason::Schema));
}
