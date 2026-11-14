use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json, value::RawValue};
use std::borrow::Cow;
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{Instant, MissedTickBehavior, interval, sleep, sleep_until, timeout};
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

use super::{
    client::{Socket, open_authenticated, read_system, retry_delay, transport_reason},
    protocol::{self, Incoming},
};
use crate::{
    collectors::realtime::Frame,
    config::{ApiConfig, RealtimeConfig},
    state::{Reason, State},
};

struct Received {
    frame: Frame,
    at: Instant,
    epoch: f64,
    duration: Duration,
}
impl Received {
    fn new(raw: &str) -> Self {
        let at = Instant::now();
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        Self {
            frame: Frame::parse(raw),
            at,
            epoch,
            duration: at.elapsed(),
        }
    }
    fn publish(self, state: &State) {
        state.complete_realtime(self.frame, self.at, self.epoch, self.duration);
    }
}

enum Parsed {
    Reply {
        id: String,
        result: Result<Value, Reason>,
    },
    Update(Received),
    Ended(Reason),
    Other,
}
fn parse(text: &str, collection: &str) -> Result<Parsed, Reason> {
    if text.len() > 262_144 {
        return Err(Reason::Protocol);
    }
    #[derive(Deserialize)]
    struct Header<'a> {
        #[serde(borrow)]
        id: Option<&'a RawValue>,
    }
    let header: Header<'_> = serde_json::from_str(text).map_err(|_| Reason::Protocol)?;
    if header.id.is_some() && text.len() > 4096 {
        return Err(Reason::Protocol);
    }
    match protocol::decode(text)? {
        Incoming::Response { id, result } => Ok(Parsed::Reply { id, result }),
        Incoming::Notification => {
            #[derive(Deserialize)]
            struct Notice<'a> {
                #[serde(borrow)]
                method: Cow<'a, str>,
                #[serde(borrow)]
                params: Option<&'a RawValue>,
            }
            #[derive(Deserialize)]
            struct Params<'a> {
                #[serde(borrow)]
                collection: Cow<'a, str>,
                #[serde(borrow)]
                msg: Option<Cow<'a, str>>,
                #[serde(borrow)]
                fields: Option<&'a RawValue>,
                #[serde(borrow)]
                error: Option<&'a RawValue>,
            }
            let notice: Notice<'_> = serde_json::from_str(text).map_err(|_| Reason::Protocol)?;
            if !matches!(
                notice.method.as_ref(),
                "collection_update" | "notify_unsubscribed"
            ) {
                return Ok(Parsed::Other);
            }
            let params: Params<'_> =
                serde_json::from_str(notice.params.ok_or(Reason::Protocol)?.get())
                    .map_err(|_| Reason::Protocol)?;
            if params.collection.as_ref() != collection {
                return Ok(Parsed::Other);
            }
            if notice.method == "notify_unsubscribed" {
                return Ok(Parsed::Ended(if params.error.is_some() {
                    Reason::Api
                } else {
                    Reason::Unavailable
                }));
            }
            if params.msg.as_deref() != Some("added") {
                return Err(Reason::Protocol);
            }
            Ok(Parsed::Update(Received::new(
                params.fields.map_or("null", RawValue::get),
            )))
        }
    }
}

struct Session {
    socket: Socket,
    collection: String,
    owned_id: String,
    pending: Option<Received>,
}
impl Session {
    async fn connect(
        api: &ApiConfig,
        config: RealtimeConfig,
        state: &State,
    ) -> Result<Self, Reason> {
        timeout(api.timeout, async {
            let mut socket = open_authenticated(api).await?;
            if let Err(reason) = read_system(&mut socket, "version").await {
                if reason == Reason::Unsupported { state.reject_compatibility(); }
                return Err(reason);
            }
            let collection = format!("reporting.realtime:{}", json!({"interval":config.interval.as_secs()}));
            socket.send(Message::Text(json!({"jsonrpc":"2.0","id":"subscribe","method":"core.subscribe","params":[collection]}).to_string().into())).await.map_err(transport_reason)?;
            let mut pending = None;
            while let Some(message) = socket.next().await {
                match message.map_err(transport_reason)? {
                    Message::Text(text) => match parse(&text, &collection)? {
                        Parsed::Reply { id, result } if id == "subscribe" => {
                            let Value::String(owned_id) = result? else { return Err(Reason::Protocol); };
                            if owned_id.is_empty() || owned_id.len() > 128 || owned_id.chars().any(|c| c.is_control() || c.is_whitespace()) { return Err(Reason::Protocol); }
                            return Ok(Self { socket, collection, owned_id, pending });
                        }
                        Parsed::Update(event) => pending = Some(event),
                        Parsed::Ended(reason) => return Err(reason),
                        _ => {}
                    },
                    Message::Ping(bytes) => socket.send(Message::Pong(bytes)).await.map_err(transport_reason)?,
                    Message::Pong(_) => {},
                    _ => return Err(Reason::Transport),
                }
            }
            Err(Reason::Transport)
        }).await.map_err(|_| Reason::Timeout)?
    }
    async fn stream(
        &mut self,
        api: &ApiConfig,
        config: RealtimeConfig,
        state: &State,
        check: bool,
    ) -> Result<(), Reason> {
        let mut last_event = Instant::now();
        let mut last_received = Instant::now();
        if let Some(event) = self.pending.take() {
            last_event = event.at;
            event.publish(state);
        }
        let mut heartbeat = interval(api.timeout);
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            if check && state.realtime_ready_at(Instant::now()) {
                return Ok(());
            }
            tokio::select! {
                biased;
                _ = sleep_until(last_event + config.ttl) => return Err(Reason::Timeout),
                _ = heartbeat.tick() => {
                    if last_received.elapsed() >= api.timeout * 2 { return Err(Reason::Timeout); }
                    timeout(api.timeout, self.socket.send(Message::Ping(Vec::new().into()))).await.map_err(|_| Reason::Timeout)?.map_err(transport_reason)?;
                }
                message = self.socket.next() => {
                    last_received = Instant::now();
                    match message.ok_or(Reason::Transport)?.map_err(transport_reason)? {
                        Message::Text(text) => match parse(&text, &self.collection)? {
                            Parsed::Update(event) => { last_event = event.at; event.publish(state); }
                            Parsed::Ended(reason) => return Err(reason),
                            _ => {},
                        },
                        Message::Ping(bytes) => { timeout(api.timeout, self.socket.send(Message::Pong(bytes))).await.map_err(|_| Reason::Timeout)?.map_err(transport_reason)?; },
                        Message::Pong(_) => {},
                        _ => return Err(Reason::Transport),
                    }
                }
            }
        }
    }
    async fn cleanup(&mut self, api: &ApiConfig) -> Result<(), Reason> {
        timeout(api.timeout.min(Duration::from_secs(1)), async {
            self.socket.send(Message::Text(json!({"jsonrpc":"2.0","id":"unsubscribe","method":"core.unsubscribe","params":[self.owned_id]}).to_string().into())).await.map_err(transport_reason)?;
            while let Some(message) = self.socket.next().await {
                match message.map_err(transport_reason)? {
                    Message::Text(text) => match parse(&text, &self.collection)? {
                        Parsed::Reply { id, result } if id == "unsubscribe" => {
                            if result? != Value::Null { return Err(Reason::Protocol); }
                            self.socket.close(None).await.map_err(transport_reason)?;
                            return Ok(());
                        }
                        _ => {}
                    },
                    Message::Ping(bytes) => self.socket.send(Message::Pong(bytes)).await.map_err(transport_reason)?,
                    Message::Pong(_) => {},
                    _ => return Err(Reason::Transport),
                }
            }
            Err(Reason::Transport)
        }).await.map_err(|_| Reason::Timeout)?
    }
}

pub async fn run(
    api: ApiConfig,
    config: RealtimeConfig,
    state: Arc<State>,
    stop: CancellationToken,
) {
    if config.validate().is_err() {
        state.complete_realtime(
            Frame::failed(Reason::Schema),
            Instant::now(),
            0.0,
            Duration::ZERO,
        );
        return;
    }
    let mut backoff = 1;
    loop {
        let mut since = None;
        let outcome = tokio::select! {
            biased;
            _ = stop.cancelled() => break,
            result = Session::connect(&api, config, &state) => result,
        };
        let result = match outcome {
            Ok(mut session) => {
                since = Some(Instant::now());
                state.realtime_subscription(true);
                let result = tokio::select! { biased; _ = stop.cancelled() => Ok(()), result = session.stream(&api, config, &state, false) => result };
                state.realtime_subscription(false);
                if stop.is_cancelled() {
                    if let Err(reason) = session.cleanup(&api).await {
                        tracing::warn!(
                            reason = reason.as_str(),
                            "Realtime cleanup incomplete; connection closing"
                        );
                    }
                    break;
                }
                result
            }
            Err(reason) => {
                if matches!(
                    reason,
                    Reason::Unsupported | Reason::Schema | Reason::Protocol
                ) {
                    state.reject_realtime();
                }
                Err(reason)
            }
        };
        if let Err(reason) = result {
            let epoch = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            state.complete_realtime(Frame::failed(reason), Instant::now(), epoch, Duration::ZERO);
            tracing::warn!(
                reason = reason.as_str(),
                "Realtime stream unavailable; retrying with backoff"
            );
        }
        let delay = retry_delay(&mut backoff, since);
        tokio::select! { _ = stop.cancelled() => break, _ = sleep(delay) => {} }
    }
    state.realtime_subscription(false);
}

pub async fn check(api: ApiConfig, config: RealtimeConfig) -> Result<(), Reason> {
    config.validate().map_err(|_| Reason::Schema)?;
    let state = State::with_realtime(None, config.ttl, false, true, Some(config));
    let mut session = Session::connect(&api, config, &state).await?;
    state.realtime_subscription(true);
    let result = timeout(
        api.timeout + config.interval * 2,
        session.stream(&api, config, &state, true),
    )
    .await
    .map_err(|_| Reason::Timeout)
    .and_then(|v| v);
    let cleanup = session.cleanup(&api).await;
    result.and(cleanup)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event() -> Value {
        serde_json::from_str(include_str!("../../tests/fixtures/realtime.synthetic.json")).unwrap()
    }
    #[test]
    fn notification_scope_and_wire_bounds_are_strict() {
        let collection = "reporting.realtime:{\"interval\":2}";
        let mut value = event();
        value["params"]["collection"] = json!("foreign");
        assert!(matches!(
            parse(&value.to_string(), collection),
            Ok(Parsed::Other)
        ));
        value = event();
        value["params"]["msg"] = json!("changed");
        assert!(matches!(
            parse(&value.to_string(), collection),
            Err(Reason::Protocol)
        ));
        value = event();
        value["params"]["fields"] = Value::Null;
        let Parsed::Update(received) = parse(&value.to_string(), collection).unwrap() else {
            panic!("update expected")
        };
        assert!(received.frame.cpu.is_err());
        let oversized = json!({"jsonrpc":"2.0","id":"subscribe","result":"x".repeat(4096)});
        assert!(matches!(
            parse(&oversized.to_string(), collection),
            Err(Reason::Protocol)
        ));
        assert!(matches!(
            parse(&" ".repeat(262145), collection),
            Err(Reason::Protocol)
        ));
    }
    #[tokio::test(start_paused = true)]
    async fn buffered_event_keeps_original_receipt_time() {
        let rt = RealtimeConfig {
            interval: Duration::from_secs(2),
            ttl: Duration::from_secs(3),
        };
        let state = State::with_realtime(None, Duration::from_secs(30), true, true, Some(rt));
        let Parsed::Update(received) =
            parse(&event().to_string(), "reporting.realtime:{\"interval\":2}").unwrap()
        else {
            panic!("update expected")
        };
        tokio::time::advance(Duration::from_secs(4)).await;
        received.publish(&state);
        assert!(!state.realtime_ready_at(Instant::now()));
    }
}
