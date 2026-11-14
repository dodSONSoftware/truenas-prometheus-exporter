use crate::{
    api::client::Client,
    collectors::{pools, system},
    state::State,
};
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{Instant, MissedTickBehavior, interval};
use tokio_util::sync::CancellationToken;

pub async fn run(
    client: Option<Client>,
    state: Arc<State>,
    period: Duration,
    stop: CancellationToken,
) {
    if state.system_enabled() {
        tokio::join!(
            run_pools(client.clone(), state.clone(), period, stop.clone()),
            run_system(client, state, period, stop)
        );
    } else {
        run_pools(client, state, period, stop).await;
    }
}

async fn run_system(
    client: Option<Client>,
    state: Arc<State>,
    period: Duration,
    stop: CancellationToken,
) {
    let mut ticks = interval(period);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! { biased; _ = stop.cancelled() => break, _ = ticks.tick() => {} }
        let started = Instant::now();
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let result = tokio::select! {
            biased;
            _ = stop.cancelled() => break,
            result = async { match &client { Some(client) => client.system_info().await, None => Ok(system::synthetic()) } } => result,
        };
        if let Err(reason) = result {
            tracing::warn!(
                collector = "system",
                reason = reason.as_str(),
                "Collection failed"
            );
        }
        state.complete_system(result, Instant::now(), epoch, started.elapsed());
    }
}

async fn run_pools(
    client: Option<Client>,
    state: Arc<State>,
    period: Duration,
    stop: CancellationToken,
) {
    let mut ticks = interval(period);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! { biased; _ = stop.cancelled() => break, _ = ticks.tick() => {} }
        let started = Instant::now();
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let result = tokio::select! {
            biased;
            _ = stop.cancelled() => break,
            result = async { match &client { Some(client) => pools::parse(client.pool_query().await?), None => Ok(pools::synthetic()) } } => result,
        };
        if let Err(reason) = result {
            tracing::warn!(
                collector = "pools",
                reason = reason.as_str(),
                "Collection failed"
            );
        }
        state.complete(result, Instant::now(), epoch, started.elapsed());
    }
}
