use crate::state::{State as ExporterState, metrics_for_targets};
use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use std::sync::Arc;
use tokio::{sync::Semaphore, time::Instant};

struct HttpState {
    targets: Vec<Arc<ExporterState>>,
    encoding: Arc<Semaphore>,
}
type Targets = Arc<HttpState>;

pub fn router(state: Arc<ExporterState>) -> Router {
    router_targets(vec![state])
}

pub fn router_targets(states: Vec<Arc<ExporterState>>) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok\n" }))
        .route("/readyz", get(ready))
        .route("/metrics", get(metrics))
        .with_state(Arc::new(HttpState {
            targets: states,
            encoding: Arc::new(Semaphore::new(1)),
        }))
}
async fn ready(State(states): State<Targets>) -> impl IntoResponse {
    let now = Instant::now();
    if !states.targets.is_empty() && states.targets.iter().all(|state| state.ready_at(now)) {
        (StatusCode::OK, "ready\n")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "not ready\n")
    }
}
async fn metrics(State(states): State<Targets>) -> impl IntoResponse {
    let Ok(permit) = states.encoding.clone().try_acquire_owned() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CONTENT_TYPE, "text/plain")],
            "metrics encoder busy\n".to_owned(),
        );
    };
    // Keep admission inside the job so cancellation of an HTTP request cannot
    // release capacity while its blocking encoder is still running.
    let encoded = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        metrics_for_targets(&states.targets, Instant::now())
    })
    .await;
    match encoded {
        Ok(Ok(text)) => (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                "application/openmetrics-text; version=1.0.0; charset=utf-8",
            )],
            text,
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain")],
            "encoding failed\n".into(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn busy_encoder_leaves_health_and_readiness_available() {
        let snapshot = ExporterState::new(std::time::Duration::from_secs(3), true);
        snapshot.complete(
            Ok(crate::collectors::pools::synthetic()),
            Instant::now(),
            1.0,
            std::time::Duration::ZERO,
        );
        let state = Arc::new(HttpState {
            targets: vec![snapshot],
            encoding: Arc::new(Semaphore::new(1)),
        });
        let permit = state.encoding.clone().acquire_owned().await.unwrap();
        let app = Router::new()
            .route("/metrics", get(metrics))
            .route("/readyz", get(ready))
            .route("/healthz", get(|| async { "ok\n" }))
            .with_state(state);
        for (path, expected) in [
            ("/metrics", StatusCode::SERVICE_UNAVAILABLE),
            ("/readyz", StatusCode::OK),
            ("/healthz", StatusCode::OK),
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
        }
        drop(permit);
        assert_eq!(
            app.oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
            StatusCode::OK
        );
    }
}
