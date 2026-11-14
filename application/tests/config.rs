use std::collections::HashMap;
use truenas_exporter::config::Config;

#[test]
fn explicit_mock_and_static_configuration_failures() {
    let mut vars = HashMap::new();
    assert!(Config::from_values(&vars).is_err());
    vars.insert("EXPORTER_MOCK".into(), "true".into());
    let config = Config::from_values(&vars).unwrap();
    assert!(config.targets[0].api.is_none());
    assert_eq!(config.ttl.as_secs(), 180);
    for (key, value) in [
        ("TRUENAS_POLL_INTERVAL_SECONDS", "0"),
        ("TRUENAS_API_TIMEOUT_SECONDS", "garbage"),
        ("TRUENAS_CACHE_TTL_SECONDS", "70"),
        ("EXPORTER_LISTEN", "bad"),
        ("EXPORTER_MOCK", "yes"),
    ] {
        let mut bad = vars.clone();
        bad.insert(key.into(), value.into());
        assert!(Config::from_values(&bad).is_err(), "{key}");
    }
}

#[test]
fn normal_mode_requires_secure_url_readable_secret_and_valid_ca() {
    use std::io::Write;
    let mut key = tempfile::NamedTempFile::new().unwrap();
    writeln!(key, "synthetic-test-key").unwrap();
    let mut vars = HashMap::from([
        (
            "TRUENAS_WS_URL".into(),
            "wss://example.invalid/api/current".into(),
        ),
        ("TRUENAS_USERNAME".into(), "synthetic-user".into()),
        (
            "TRUENAS_API_KEY_FILE".into(),
            key.path().to_str().unwrap().into(),
        ),
    ]);
    assert!(Config::from_values(&vars).is_ok());
    for url in [
        "ws://127.0.0.1/api/current",
        "wss://user:secret@example.invalid/api/current",
        "wss://example.invalid/api/current?token=x",
        "not a url",
    ] {
        let mut bad = vars.clone();
        bad.insert("TRUENAS_WS_URL".into(), url.into());
        assert!(Config::from_values(&bad).is_err());
    }
    vars.insert(
        "TRUENAS_CA_CERT_FILE".into(),
        key.path().to_str().unwrap().into(),
    );
    assert!(Config::from_values(&vars).is_err());
    vars.remove("TRUENAS_CA_CERT_FILE");
    std::fs::write(key.path(), "").unwrap();
    assert!(Config::from_values(&vars).is_err());
    std::fs::write(key.path(), "secret\nembedded").unwrap();
    assert!(Config::from_values(&vars).is_err());
    vars.insert("TRUENAS_API_KEY_FILE".into(), "/missing-test-secret".into());
    assert!(Config::from_values(&vars).is_err());
}

#[test]
fn multiple_targets_keep_credentials_and_names_separate() {
    use serde_json::json;
    let folder = tempfile::tempdir().unwrap();
    let alpha_key = folder.path().join("alpha-key");
    let beta_key = folder.path().join("beta-key");
    std::fs::write(&alpha_key, "alpha-synthetic").unwrap();
    std::fs::write(&beta_key, "beta-synthetic").unwrap();
    let file = folder.path().join("targets.json");
    std::fs::write(&file, json!({"targets":[
        {"name":"alpha","ws_url":"wss://alpha.invalid/api/current","username":"alpha-user","api_key_file":alpha_key},
        {"name":"beta","ws_url":"wss://beta.invalid/api/current","username":"beta-user","api_key_file":beta_key}
    ]}).to_string()).unwrap();
    let values = HashMap::from([("TRUENAS_TARGETS_FILE".into(), file.to_str().unwrap().into())]);
    let config = Config::from_values(&values).unwrap();
    assert_eq!(config.targets.len(), 2);
    assert_eq!(config.targets[0].name.as_deref(), Some("alpha"));
    assert_eq!(config.targets[1].name.as_deref(), Some("beta"));
    assert_eq!(
        config.targets[0].api.as_ref().unwrap().credentials.key,
        "alpha-synthetic"
    );
    assert_eq!(
        config.targets[1].api.as_ref().unwrap().credentials.key,
        "beta-synthetic"
    );
    assert_eq!(
        config.targets[0].api.as_ref().unwrap().credentials.username,
        "alpha-user"
    );
    assert_eq!(
        config.targets[1].api.as_ref().unwrap().credentials.username,
        "beta-user"
    );
    let mut mixed = values.clone();
    mixed.insert("TRUENAS_WS_URL".into(), "wss://legacy.invalid".into());
    assert!(Config::from_values(&mixed).is_err());
    let mut mock = values.clone();
    mock.insert("EXPORTER_MOCK".into(), "true".into());
    assert!(Config::from_values(&mock).is_err());
}

#[test]
fn multi_target_boundaries_reject_ambiguous_or_unsafe_configuration() {
    use serde_json::json;
    let folder = tempfile::tempdir().unwrap();
    let key = folder.path().join("key");
    std::fs::write(&key, "synthetic").unwrap();
    let file = folder.path().join("targets.json");
    let entry = json!({"name":"alpha","ws_url":"wss://alpha.invalid/api/current","username":"test-user","api_key_file":key});
    let mut bad_name = entry.clone();
    bad_name["name"] = json!("invalid name");
    let mut relative = entry.clone();
    relative["api_key_file"] = json!("relative-key");
    let mut inline = entry.clone();
    inline["api_key"] = json!("SENSITIVE_VALUE_MUST_NOT_APPEAR");
    let cases = [
        json!({"targets":[]}),
        json!({"targets":vec![entry.clone();17]}),
        json!({"targets":[entry.clone(),entry]}),
        json!({"targets":[bad_name]}),
        json!({"targets":[relative]}),
        json!({"targets":[inline]}),
    ];
    let values = HashMap::from([("TRUENAS_TARGETS_FILE".into(), file.to_str().unwrap().into())]);
    for case in cases {
        std::fs::write(&file, case.to_string()).unwrap();
        let error = Config::from_values(&values)
            .err()
            .expect("invalid target config must fail");
        assert!(
            !error
                .to_string()
                .contains("SENSITIVE_VALUE_MUST_NOT_APPEAR")
        );
    }
}

#[test]
fn transport_configuration_does_not_require_or_load_api_credentials() {
    use truenas_exporter::config::TransportConfig;
    let values = HashMap::from([
        (
            "TRUENAS_WS_URL".into(),
            "wss://example.invalid/api/current".into(),
        ),
        (
            "TRUENAS_API_KEY_FILE".into(),
            "/nonexistent-no-key-should-be-read".into(),
        ),
    ]);
    assert!(TransportConfig::from_values(&values).is_ok());
    assert!(Config::from_values(&values).is_err());
}

#[test]
fn file_limits_apply_to_raw_bytes_and_allow_regular_symlinks() {
    use truenas_exporter::config::tls_config;
    let dir = tempfile::tempdir().unwrap();
    let key = dir.path().join("key");
    let ca = dir.path().join("ca");
    let targets = dir.path().join("targets");
    let mut values = HashMap::from([
        (
            "TRUENAS_WS_URL".into(),
            "wss://example.invalid/api/current".into(),
        ),
        ("TRUENAS_USERNAME".into(), "synthetic".into()),
        ("TRUENAS_API_KEY_FILE".into(), key.to_str().unwrap().into()),
    ]);
    let mut bytes = vec![b'x'; 16384];
    bytes[16383] = b'\n';
    std::fs::write(&key, &bytes).unwrap();
    assert!(Config::from_values(&values).is_ok());
    bytes.push(b'\n');
    std::fs::write(&key, &bytes).unwrap();
    let error = Config::from_values(&values).err().unwrap();
    assert!(
        error
            .to_string()
            .contains("TRUENAS_API_KEY_FILE file exceeds")
    );
    assert!(!error.to_string().contains("xxxx"));
    std::fs::write(&key, "synthetic").unwrap();

    let text = serde_json::json!({"targets":[{"name":"a","ws_url":"wss://example.invalid/api/current","username":"synthetic","api_key_file":key}]}).to_string();
    let mut bytes = text.into_bytes();
    bytes.resize(65536, b' ');
    std::fs::write(&targets, &bytes).unwrap();
    let file_values = HashMap::from([(
        "TRUENAS_TARGETS_FILE".into(),
        targets.to_str().unwrap().into(),
    )]);
    assert!(Config::from_values(&file_values).is_ok());
    bytes.push(b' ');
    std::fs::write(&targets, &bytes).unwrap();
    assert!(
        Config::from_values(&file_values)
            .err()
            .unwrap()
            .to_string()
            .contains("exceeds")
    );

    let cert = rcgen::generate_simple_self_signed(vec!["fixture.invalid".into()]).unwrap();
    let mut bytes = cert.cert.pem().into_bytes();
    bytes.resize(1024 * 1024, b'\n');
    std::fs::write(&ca, &bytes).unwrap();
    assert!(tls_config(Some(&ca)).is_ok());
    bytes.push(b'\n');
    std::fs::write(&ca, &bytes).unwrap();
    assert!(
        tls_config(Some(&ca))
            .err()
            .unwrap()
            .to_string()
            .contains("TRUENAS_CA_CERT_FILE file exceeds")
    );

    #[cfg(unix)]
    {
        let alias = dir.path().join("key-link");
        std::os::unix::fs::symlink(&key, &alias).unwrap();
        values.insert(
            "TRUENAS_API_KEY_FILE".into(),
            alias.to_str().unwrap().into(),
        );
        assert!(Config::from_values(&values).is_ok());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn fifo_configuration_fails_without_waiting_for_a_writer() {
    use tokio::{
        process::Command,
        time::{Duration, timeout},
    };
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    for setting in [
        "TRUENAS_TARGETS_FILE",
        "TRUENAS_API_KEY_FILE",
        "TRUENAS_CA_CERT_FILE",
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_truenas-exporter"));
        command.env_clear().kill_on_drop(true).env(setting, &fifo);
        if setting != "TRUENAS_TARGETS_FILE" {
            command
                .env("TRUENAS_WS_URL", "wss://127.0.0.1:1/api/current")
                .env("TRUENAS_USERNAME", "synthetic");
        }
        let output = timeout(Duration::from_secs(3), command.output())
            .await
            .unwrap()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(
            error.contains(&format!("{setting} must refer to a regular file")),
            "{error}"
        );
    }
}
