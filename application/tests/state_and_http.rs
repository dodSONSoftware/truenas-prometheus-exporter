use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::time::Duration;
use tokio::time::Instant;
use tower::ServiceExt;
use truenas_exporter::{
    collectors::pools,
    http,
    state::{Reason, State},
};

fn fixture() -> Vec<pools::Pool> {
    pools::parse(serde_json::from_str(include_str!("fixtures/pools.synthetic.json")).unwrap())
        .unwrap()
}
fn sample(text: &str, metric: &str) -> bool {
    text.lines()
        .any(|line| line.starts_with(metric) && !line.starts_with('#'))
}

#[tokio::test(start_paused = true)]
async fn failures_preserve_values_until_exact_expiry_then_recover() {
    let state = State::new(Duration::from_secs(180), false);
    let start = Instant::now();
    assert!(!state.ready_at(start));
    let initial = state.metrics_at(start).unwrap();
    assert!(!sample(&initial, "truenas_pool_size_bytes{"));
    assert!(!sample(&initial, "truenas_exporter_snapshot_age_seconds{"));
    state.complete(Ok(fixture()), start, 1000.0, Duration::from_millis(10));
    assert!(state.ready_at(start));
    let good = state.metrics_at(start).unwrap();
    assert!(good.contains("# HELP truenas_pool_size_bytes"));
    assert!(good.contains("# TYPE truenas_pool_size_bytes gauge"));
    assert!(good.contains("truenas_pool_size_bytes{pool=\"example-pool\"} 24000277250048"));
    assert!(good.contains("truenas_pool_healthy{pool=\"example-pool\"} 1"));
    assert!(good.contains("truenas_pool_warning{pool=\"example-pool\"} 1"));
    assert!(good.contains("FEAT_DISABLED"));
    assert!(!sample(&good, "truenas_pool_scrub_errors"));
    state.complete(
        Err(Reason::Permission),
        start + Duration::from_secs(60),
        1060.0,
        Duration::from_secs(1),
    );
    let failed = state.metrics_at(start + Duration::from_secs(179)).unwrap();
    assert!(state.ready_at(start + Duration::from_secs(179)));
    assert!(failed.contains("truenas_exporter_collection_success{collector=\"pools\"} 0"));
    assert!(sample(&failed, "truenas_pool_size_bytes{"));
    assert!(failed.contains(
        "truenas_exporter_collection_errors_total{collector=\"pools\",reason=\"permission\"} 1"
    ));
    let expired = state.metrics_at(start + Duration::from_secs(180)).unwrap();
    assert!(!state.ready_at(start + Duration::from_secs(180)));
    assert!(!sample(&expired, "truenas_pool_size_bytes{"));
    assert!(sample(
        &expired,
        "truenas_exporter_last_success_timestamp_seconds{"
    ));
    state.complete(
        Ok(vec![]),
        start + Duration::from_secs(181),
        900.0,
        Duration::ZERO,
    );
    assert!(state.ready_at(start + Duration::from_secs(181)));
    assert!(!sample(
        &state.metrics_at(start + Duration::from_secs(181)).unwrap(),
        "truenas_pool_size_bytes{"
    ));
}

#[tokio::test(start_paused = true)]
async fn expiry_without_a_new_attempt_changes_success_and_http_readiness() {
    let state = State::new(Duration::from_secs(3), true);
    let app = http::router(state.clone());
    async fn get(app: axum::Router, path: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }
    assert_eq!(get(app.clone(), "/healthz").await.0, StatusCode::OK);
    assert_eq!(
        get(app.clone(), "/readyz").await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    state.complete(Ok(fixture()), Instant::now(), 100.0, Duration::ZERO);
    assert_eq!(get(app.clone(), "/readyz").await.0, StatusCode::OK);
    tokio::time::advance(Duration::from_secs(3)).await;
    assert_eq!(
        get(app.clone(), "/readyz").await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let (status, body) = get(app, "/metrics").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("truenas_exporter_collection_success{collector=\"pools\"} 0"));
    assert!(!sample(&body, "truenas_pool_size_bytes{"));
}

#[test]
fn missing_null_invalid_and_duplicate_pools() {
    use serde_json::json;
    let pool =
        pools::parse(json!([{"name":"minimal", "size":null, "healthy":true, "warning":true}]))
            .unwrap()
            .remove(0);
    assert!(pool.size.is_none());
    assert_eq!(pool.healthy, Some(true));
    assert_eq!(pool.warning, Some(true));
    assert!(pools::parse(json!([])).unwrap().is_empty());
    assert!(pools::parse(json!([{"name":"zero", "size":0, "allocated":0, "free":0}])).is_ok());
    for value in [
        json!([{"size":1}]),
        json!([{"name":"a","size":-1}]),
        json!([{"name":"a","fragmentation":"NaN"}]),
        json!([{"name":"a","fragmentation":101}]),
        json!([{"name":"a","size":0,"allocated":1}]),
        json!([{"name":"a"},{"name":"a"}]),
        json!([{"name":"a","status":"sensitive detail"}]),
        json!({"name":"a"}),
    ] {
        assert!(matches!(pools::parse(value), Err(Reason::Schema)));
    }
}

#[tokio::test]
async fn metric_labels_are_escaped_and_missing_values_are_absent() {
    let state = State::new(Duration::from_secs(10), false);
    let value = serde_json::json!([{"name":"pool\"\\x", "size":null, "status":"NEW_STATE", "status_code":"UNRECOGNIZED_CODE"}]);
    state.complete(
        Ok(pools::parse(value).unwrap()),
        Instant::now(),
        1.0,
        Duration::ZERO,
    );
    let text = state.metrics_at(Instant::now()).unwrap();
    assert!(text.contains("pool=\"pool\\\"\\\\x\""));
    assert!(!sample(&text, "truenas_pool_size_bytes{"));
    assert!(text.contains("status=\"UNKNOWN\""));
    assert!(text.contains("status_code=\"OTHER\""));
}

#[tokio::test(start_paused = true)]
async fn named_targets_have_independent_expiry_and_one_metric_descriptor() {
    use truenas_exporter::state::metrics_for_targets;
    let alpha = State::for_target("alpha".into(), Duration::from_secs(3), false);
    let beta = State::for_target("beta".into(), Duration::from_secs(6), false);
    let start = Instant::now();
    alpha.complete(Ok(fixture()), start, 100.0, Duration::ZERO);
    let mut other = fixture();
    other[0].size = Some(2_000_000);
    other[0].allocated = Some(800_000);
    other[0].free = Some(1_200_000);
    beta.complete(Ok(other), start, 100.0, Duration::ZERO);
    let states = vec![alpha.clone(), beta.clone()];
    let first = metrics_for_targets(&states, start).unwrap();
    assert!(first.contains(
        "truenas_pool_size_bytes{target=\"alpha\",pool=\"example-pool\"} 24000277250048"
    ));
    assert!(
        first.contains("truenas_pool_size_bytes{target=\"beta\",pool=\"example-pool\"} 2000000")
    );
    assert_eq!(first.matches("# HELP truenas_pool_size_bytes ").count(), 1);
    assert_eq!(first.matches("# TYPE truenas_pool_size_bytes ").count(), 1);
    assert_eq!(first.matches("# EOF").count(), 1);
    alpha.complete(
        Err(Reason::Permission),
        start + Duration::from_secs(1),
        101.0,
        Duration::ZERO,
    );
    let expired = metrics_for_targets(&states, start + Duration::from_secs(3)).unwrap();
    assert!(
        !expired
            .lines()
            .any(|line| line.starts_with("truenas_pool_size_bytes{target=\"alpha\""))
    );
    assert!(expired.contains("truenas_pool_size_bytes{target=\"beta\""));
    assert!(
        expired.contains(
            "truenas_exporter_collection_success{target=\"alpha\",collector=\"pools\"} 0"
        )
    );
    assert!(
        expired
            .contains("truenas_exporter_collection_success{target=\"beta\",collector=\"pools\"} 1")
    );
    let app = http::router_targets(states);
    tokio::time::advance(Duration::from_secs(3)).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/readyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
