use crate::state::{State as ExporterState, metrics_for_targets};
use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use std::sync::Arc;
use tokio::time::Instant;

type Targets = Arc<Vec<Arc<ExporterState>>>;

pub fn router(state: Arc<ExporterState>) -> Router {
    router_targets(vec![state])
}

pub fn router_targets(states: Vec<Arc<ExporterState>>) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok\n" }))
        .route("/readyz", get(ready))
        .route("/metrics", get(metrics))
        .with_state(Arc::new(states))
}
async fn ready(State(states): State<Targets>) -> impl IntoResponse {
    let now = Instant::now();
    if !states.is_empty() && states.iter().all(|state| state.ready_at(now)) {
        (StatusCode::OK, "ready\n")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "not ready\n")
    }
}
async fn metrics(State(states): State<Targets>) -> impl IntoResponse {
    match metrics_for_targets(&states, Instant::now()) {
        Ok(text) => (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                "application/openmetrics-text; version=1.0.0; charset=utf-8",
            )],
            text,
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain")],
            "encoding failed\n".into(),
        ),
    }
}
