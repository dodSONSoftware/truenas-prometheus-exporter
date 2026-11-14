use super::protocol::{self, Incoming, Method};
use crate::{
    config::{ApiConfig, TransportConfig},
    state::{Reason, State},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::{Instant, MissedTickBehavior, interval, sleep, timeout, timeout_at},
};
use tokio_tungstenite::{
    Connector, MaybeTlsStream, WebSocketStream, connect_async_tls_with_config,
    tungstenite::{self, Message, protocol::WebSocketConfig},
};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

const CAPACITY: usize = 32;
const MAX_MESSAGE: usize = 1024 * 1024;
type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
struct Request {
    method: Method,
    deadline: Instant,
    reply: oneshot::Sender<Result<Value, Reason>>,
}

#[derive(Clone)]
pub struct Client {
    sender: mpsc::Sender<Request>,
    timeout: Duration,
    state: Arc<State>,
}

/// Establish verified WSS and close it without sending authentication or API frames.
pub async fn check_transport(config: TransportConfig) -> Result<(), Reason> {
    let ws_config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE));
    let (mut socket, _) = timeout(
        config.timeout,
        connect_async_tls_with_config(
            config.url.as_str(),
            Some(ws_config),
            true,
            Some(Connector::Rustls(config.tls)),
        ),
    )
    .await
    .map_err(|_| Reason::Timeout)?
    .map_err(transport_reason)?;
    timeout(config.timeout, socket.close(None))
        .await
        .map_err(|_| Reason::Timeout)?
        .map_err(transport_reason)
}

impl Client {
    pub fn spawn(
        config: ApiConfig,
        state: Arc<State>,
        stop: CancellationToken,
    ) -> (Self, JoinHandle<()>) {
        let (sender, receiver) = mpsc::channel(CAPACITY);
        let client = Self {
            sender,
            timeout: config.timeout,
            state: state.clone(),
        };
        let task = tokio::spawn(
            supervise(config, state, receiver, stop).instrument(tracing::Span::current()),
        );
        (client, task)
    }
    pub async fn pool_query(&self) -> Result<Value, Reason> {
        self.request(Method::PoolQuery).await
    }
    async fn request(&self, method: Method) -> Result<Value, Reason> {
        if !self.state.connected.load(Ordering::Relaxed) {
            return Err(Reason::Unavailable);
        }
        let deadline = Instant::now() + self.timeout;
        let (reply, receive) = oneshot::channel();
        timeout_at(
            deadline,
            self.sender.send(Request {
                method,
                deadline,
                reply,
            }),
        )
        .await
        .map_err(|_| Reason::Timeout)?
        .map_err(|_| Reason::Unavailable)?;
        timeout_at(deadline, receive)
            .await
            .map_err(|_| Reason::Timeout)?
            .map_err(|_| Reason::Unavailable)?
    }
}

async fn supervise(
    config: ApiConfig,
    state: Arc<State>,
    mut receiver: mpsc::Receiver<Request>,
    stop: CancellationToken,
) {
    let mut backoff = 1u64;
    loop {
        let started = Instant::now();
        let outcome = connect_and_run(&config, &state, &mut receiver, &stop).await;
        state.connected.store(false, Ordering::Relaxed);
        while let Ok(request) = receiver.try_recv() {
            let _ = request.reply.send(Err(Reason::Unavailable));
        }
        if stop.is_cancelled() || receiver.is_closed() {
            break;
        }
        if let Err(reason) = outcome {
            state.connection_failed(reason);
            if reason == Reason::Tls {
                tracing::warn!(
                    reason = reason.as_str(),
                    "API connection failed; check CA trust, certificate validity and DNS/IP certificate identity"
                );
            } else {
                tracing::warn!(
                    reason = reason.as_str(),
                    "API session unavailable; retrying with backoff"
                );
            }
        }
        if started.elapsed() >= Duration::from_secs(120) {
            backoff = 1;
        }
        let delay = Duration::from_millis(rand::random_range((backoff * 500)..=(backoff * 1000)));
        tokio::select! { _ = stop.cancelled() => break, _ = sleep(delay) => {} }
        backoff = (backoff * 2).min(60);
    }
    state.connected.store(false, Ordering::Relaxed);
}

async fn connect_and_run(
    config: &ApiConfig,
    state: &State,
    receiver: &mut mpsc::Receiver<Request>,
    stop: &CancellationToken,
) -> Result<(), Reason> {
    let socket = tokio::select! {
        biased;
        _ = stop.cancelled() => return Ok(()),
        result = open_authenticated(config) => result?,
    };
    state.connected.store(true, Ordering::Relaxed);
    tracing::info!("API authenticated");
    run_session(socket, config.timeout, receiver, stop).await
}

async fn open_authenticated(config: &ApiConfig) -> Result<Socket, Reason> {
    let ws_config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE));
    let (mut socket, _) = timeout(
        config.timeout,
        connect_async_tls_with_config(
            config.url.as_str(),
            Some(ws_config),
            true,
            Some(Connector::Rustls(config.tls.clone())),
        ),
    )
    .await
    .map_err(|_| Reason::Timeout)?
    .map_err(transport_reason)?;
    timeout(config.timeout, authenticate(&mut socket, config))
        .await
        .map_err(|_| Reason::Timeout)??;
    Ok(socket)
}

async fn authenticate(socket: &mut Socket, config: &ApiConfig) -> Result<(), Reason> {
    socket
        .send(Message::Text(
            protocol::login("auth", &config.credentials)
                .to_string()
                .into(),
        ))
        .await
        .map_err(transport_reason)?;
    while let Some(message) = socket.next().await {
        match message.map_err(transport_reason)? {
            Message::Text(text) => match protocol::decode(&text)? {
                Incoming::Response { id, result } if id == "auth" => {
                    return protocol::authenticate(result?);
                }
                _ => {}
            },
            Message::Ping(bytes) => socket
                .send(Message::Pong(bytes))
                .await
                .map_err(transport_reason)?,
            Message::Pong(_) => {}
            _ => return Err(Reason::Protocol),
        }
    }
    Err(Reason::Transport)
}

async fn run_session(
    socket: Socket,
    deadline: Duration,
    receiver: &mut mpsc::Receiver<Request>,
    stop: &CancellationToken,
) -> Result<(), Reason> {
    let (mut sink, mut stream) = socket.split();
    let (write, mut outgoing) = mpsc::channel::<Message>(CAPACITY);
    let (failed, mut writer_failure) = oneshot::channel();
    let writer = tokio::spawn(async move {
        while let Some(message) = outgoing.recv().await {
            let result = timeout(deadline, sink.send(message)).await;
            let reason = match result {
                Ok(Ok(())) => continue,
                Ok(Err(e)) => transport_reason(e),
                Err(_) => Reason::Timeout,
            };
            let _ = failed.send(reason);
            return;
        }
    });
    let mut pending: HashMap<String, Request> = HashMap::new();
    let mut sequence = 0u64;
    let mut heartbeat = interval(deadline);
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut last_received = Instant::now();
    let outcome = loop {
        // Prune independently of traffic so late responses cannot refresh expired requests.
        let now = Instant::now();
        pending.retain(|_, r| !r.reply.is_closed() && r.deadline > now);
        tokio::select! {
            _ = stop.cancelled() => break Ok(()),
            result = &mut writer_failure => break Err(result.unwrap_or(Reason::Transport)),
            message = stream.next() => {
                last_received = Instant::now();
                let result = match message {
                    Some(Ok(Message::Text(text))) => match protocol::decode_pool_response(&text) {
                        Ok(Incoming::Response { id, result }) => {
                            if let Some(request) = pending.remove(&id)
                                && request.deadline > Instant::now()
                            {
                                let _ = request.reply.send(result);
                            }
                            Ok(())
                        },
                        Ok(Incoming::Notification) => Ok(()),
                        Err(reason) => Err(reason),
                    },
                    Some(Ok(Message::Ping(bytes))) => write.try_send(Message::Pong(bytes)).map_err(|_| Reason::Busy),
                    Some(Ok(Message::Pong(_))) => Ok(()),
                    Some(Err(error)) => Err(transport_reason(error)),
                    _ => Err(Reason::Transport),
                };
                if let Err(reason) = result { break Err(reason); }
            },
            request = receiver.recv() => {
                let Some(request) = request else { break Ok(()); };
                if request.reply.is_closed() || request.deadline <= Instant::now() { continue; }
                if pending.len() >= CAPACITY { let _ = request.reply.send(Err(Reason::Busy)); continue; }
                sequence += 1;
                let id = sequence.to_string();
                let frame = Message::Text(request.method.request(&id).to_string().into());
                if write.try_send(frame).is_err() { let _ = request.reply.send(Err(Reason::Busy)); continue; }
                pending.insert(id, request);
            },
            _ = heartbeat.tick() => {
                if last_received.elapsed() >= deadline * 2 { break Err(Reason::Timeout); }
                if write.try_send(Message::Ping(Vec::new().into())).is_err() { break Err(Reason::Busy); }
            },
        }
    };
    writer.abort();
    let _ = writer.await;
    let reason = outcome
        .as_ref()
        .err()
        .copied()
        .unwrap_or(Reason::Unavailable);
    for (_, request) in pending {
        let _ = request.reply.send(Err(reason));
    }
    outcome
}

fn transport_reason(error: tungstenite::Error) -> Reason {
    match error {
        tungstenite::Error::Tls(_) => Reason::Tls,
        tungstenite::Error::Io(e) if e.kind() == std::io::ErrorKind::InvalidData => Reason::Tls,
        tungstenite::Error::Protocol(_)
        | tungstenite::Error::Capacity(_)
        | tungstenite::Error::Utf8(_) => Reason::Protocol,
        _ => Reason::Transport,
    }
}
