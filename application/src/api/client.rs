use super::protocol::{self, Incoming, Method};
use crate::{
    collectors::system::{self, SystemInfo},
    config::{ApiConfig, TransportConfig, validate_ws_url},
    state::{Reason, State},
};
use futures_util::{Sink, SinkExt, StreamExt};
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
    time::{Instant, MissedTickBehavior, interval, sleep, sleep_until, timeout, timeout_at},
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

impl Request {
    fn complete(self, result: Result<Value, Reason>) {
        let result = if Instant::now() >= self.deadline {
            Err(Reason::Timeout)
        } else {
            result
        };
        let _ = self.reply.send(result);
    }
}

enum Outbound {
    Request { frame: Message, expires: Instant },
    Control(Message),
}

async fn write_frames(
    mut sink: impl Sink<Message, Error = tungstenite::Error> + Unpin,
    mut outgoing: mpsc::Receiver<Outbound>,
    control_timeout: Duration,
) -> Result<(), Reason> {
    while let Some(outbound) = outgoing.recv().await {
        let (frame, expires) = match outbound {
            Outbound::Request { frame, expires } => {
                if Instant::now() >= expires {
                    continue;
                }
                (frame, expires)
            }
            Outbound::Control(frame) => (frame, Instant::now() + control_timeout),
        };
        // A partially flushed frame has uncertain delivery. Drop the session on expiry.
        tokio::select! {
            biased;
            _ = sleep_until(expires) => return Err(Reason::Timeout),
            result = sink.send(frame) => result.map_err(transport_reason)?,
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct Client {
    sender: mpsc::Sender<Request>,
    timeout: Duration,
    state: Arc<State>,
}

/// Establish verified WSS and close it without sending authentication or API frames.
pub async fn check_transport(config: TransportConfig) -> Result<(), Reason> {
    let mut socket = open_transport(&config.url, config.tls.clone(), config.timeout).await?;
    timeout(config.timeout, socket.close(None))
        .await
        .map_err(|_| Reason::Timeout)?
        .map_err(transport_reason)
}

/// Explicit, one-shot system preflight; never scheduled by normal pool collection.
pub async fn check_system(config: ApiConfig) -> Result<SystemInfo, Reason> {
    timeout(config.timeout, async {
        let mut socket = open_authenticated(&config).await?;
        let info = read_system(&mut socket, "system").await?;
        socket.close(None).await.map_err(transport_reason)?;
        Ok(info)
    })
    .await
    .map_err(|_| Reason::Timeout)?
}

async fn read_system(socket: &mut Socket, request_id: &str) -> Result<SystemInfo, Reason> {
    socket
        .send(Message::Text(
            Method::SystemInfo.request(request_id).to_string().into(),
        ))
        .await
        .map_err(transport_reason)?;
    loop {
        match socket.next().await {
            Some(Ok(Message::Text(text))) => match protocol::decode_response(&text, |id| {
                (id == request_id).then_some(Method::SystemInfo)
            })? {
                Incoming::Response { id, result } if id == request_id => {
                    let info = system::parse(result?)?;
                    return Ok(info);
                }
                _ => {}
            },
            Some(Ok(Message::Ping(bytes))) => socket
                .send(Message::Pong(bytes))
                .await
                .map_err(transport_reason)?,
            Some(Ok(Message::Pong(_))) => {}
            Some(Err(error)) => return Err(transport_reason(error)),
            _ => return Err(Reason::Transport),
        }
    }
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
    pub async fn system_info(&self) -> Result<SystemInfo, Reason> {
        if !self.state.system_enabled() {
            return Err(Reason::Unsupported);
        }
        system::parse(self.request(Method::SystemInfo).await?)
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
        let mut authenticated_since = None;
        let outcome = connect_and_run(
            &config,
            &state,
            &mut receiver,
            &stop,
            &mut authenticated_since,
        )
        .await;
        state.connected.store(false, Ordering::Relaxed);
        while let Ok(request) = receiver.try_recv() {
            request.complete(Err(Reason::Unavailable));
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
        let delay = retry_delay(&mut backoff, authenticated_since);
        tokio::select! { _ = stop.cancelled() => break, _ = sleep(delay) => {} }
    }
    state.connected.store(false, Ordering::Relaxed);
}

fn retry_delay(backoff: &mut u64, authenticated_since: Option<Instant>) -> Duration {
    if authenticated_since.is_some_and(|since| since.elapsed() >= Duration::from_secs(120)) {
        *backoff = 1;
    }
    let delay = Duration::from_millis(rand::random_range((*backoff * 500)..=(*backoff * 1000)));
    *backoff = (*backoff * 2).min(60);
    delay
}

async fn connect_and_run(
    config: &ApiConfig,
    state: &State,
    receiver: &mut mpsc::Receiver<Request>,
    stop: &CancellationToken,
    authenticated_since: &mut Option<Instant>,
) -> Result<(), Reason> {
    let mut socket = tokio::select! {
        biased;
        _ = stop.cancelled() => return Ok(()),
        result = open_authenticated(config) => result?,
    };
    if state.system_enabled() {
        let started = Instant::now();
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let result = tokio::select! {
            biased;
            _ = stop.cancelled() => return Ok(()),
            result = timeout(config.timeout, read_system(&mut socket, "version")) => result.map_err(|_| Reason::Timeout).and_then(|r| r),
        };
        let error = result.as_ref().err().copied();
        if error.is_some() {
            state.reject_compatibility();
        }
        state.complete_system(result, Instant::now(), epoch, started.elapsed());
        if let Some(reason) = error {
            return Err(reason);
        }
        state.accept_compatibility();
    }
    *authenticated_since = Some(Instant::now());
    state.connected.store(true, Ordering::Relaxed);
    tracing::info!("API authenticated");
    run_session(socket, config.timeout, receiver, stop, state).await
}

async fn open_authenticated(config: &ApiConfig) -> Result<Socket, Reason> {
    let mut socket = open_transport(&config.url, config.tls.clone(), config.timeout).await?;
    timeout(config.timeout, authenticate(&mut socket, config))
        .await
        .map_err(|_| Reason::Timeout)??;
    Ok(socket)
}

async fn open_transport(
    url: &url::Url,
    tls: Arc<rustls::ClientConfig>,
    deadline: Duration,
) -> Result<Socket, Reason> {
    validate_ws_url(url).map_err(|_| Reason::Protocol)?;
    let ws_config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE));
    let (socket, _) = timeout(
        deadline,
        connect_async_tls_with_config(
            url.as_str(),
            Some(ws_config),
            true,
            Some(Connector::Rustls(tls)),
        ),
    )
    .await
    .map_err(|_| Reason::Timeout)?
    .map_err(transport_reason)?;
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
    state: &State,
) -> Result<(), Reason> {
    let (sink, mut stream) = socket.split();
    let (write, outgoing) = mpsc::channel(CAPACITY);
    let mut writer = tokio::spawn(write_frames(sink, outgoing, deadline));
    let mut pending: HashMap<String, Request> = HashMap::new();
    let mut sequence = 0u64;
    let mut heartbeat = interval(deadline);
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut last_received = Instant::now();
    let outcome = loop {
        // Prune independently of traffic so late responses cannot refresh expired requests.
        let now = Instant::now();
        for (_, request) in pending.extract_if(|_, r| r.reply.is_closed() || r.deadline <= now) {
            request.complete(Err(Reason::Timeout));
        }
        tokio::select! {
            _ = stop.cancelled() => break Ok(()),
            result = &mut writer => break result.unwrap_or(Err(Reason::Transport)),
            message = stream.next() => {
                last_received = Instant::now();
                let result = match message {
                    Some(Ok(Message::Text(text))) => match protocol::decode_response(&text, |id| pending.get(id).map(|request| request.method)) {
                        Ok(Incoming::Response { id, result }) => {
                            if let Some(request) = pending.remove(&id) {
                                if matches!(request.method, Method::SystemInfo) {
                                    let invalid_version = match &result {
                                        Ok(value) => system::validate_version(value).err(),
                                        Err(reason @ (Reason::Schema | Reason::Unsupported)) => Some(*reason),
                                        _ => None,
                                    };
                                    if let Some(reason) = invalid_version {
                                        state.reject_compatibility();
                                        request.complete(Err(reason));
                                        break Err(reason);
                                    }
                                }
                                request.complete(result);
                            }
                            Ok(())
                        },
                        Ok(Incoming::Notification) => Ok(()),
                        Err(reason) => Err(reason),
                    },
                    Some(Ok(Message::Ping(bytes))) => write.try_send(Outbound::Control(Message::Pong(bytes))).map_err(|_| Reason::Busy),
                    Some(Ok(Message::Pong(_))) => Ok(()),
                    Some(Err(error)) => Err(transport_reason(error)),
                    _ => Err(Reason::Transport),
                };
                if let Err(reason) = result { break Err(reason); }
            },
            request = receiver.recv() => {
                let Some(request) = request else { break Ok(()); };
                if request.reply.is_closed() || request.deadline <= Instant::now() { request.complete(Err(Reason::Timeout)); continue; }
                if pending.len() >= CAPACITY { request.complete(Err(Reason::Busy)); continue; }
                sequence += 1;
                let id = sequence.to_string();
                let frame = Message::Text(request.method.request(&id).to_string().into());
                if write.try_send(Outbound::Request { frame, expires: request.deadline }).is_err() { request.complete(Err(Reason::Busy)); continue; }
                pending.insert(id, request);
            },
            _ = heartbeat.tick() => {
                if last_received.elapsed() >= deadline * 2 { break Err(Reason::Timeout); }
                if write.try_send(Outbound::Control(Message::Ping(Vec::new().into()))).is_err() { break Err(Reason::Busy); }
            },
        }
    };
    if !writer.is_finished() {
        writer.abort();
        let _ = writer.await;
    }
    let reason = outcome
        .as_ref()
        .err()
        .copied()
        .unwrap_or(Reason::Unavailable);
    for (_, request) in pending {
        request.complete(Err(reason));
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;
    use tungstenite::protocol::Role;

    #[tokio::test(start_paused = true)]
    async fn backoff_requires_authenticated_stability() {
        let mut backoff = 1;
        for expected in [1, 2, 4, 8, 16, 32, 60, 60] {
            // Even lengthy unsuccessful connection/auth attempts have no session timestamp.
            tokio::time::advance(Duration::from_secs(300)).await;
            let delay = retry_delay(&mut backoff, None);
            assert!(
                (Duration::from_millis(expected * 500)..=Duration::from_secs(expected))
                    .contains(&delay)
            );
            assert_eq!(backoff, (expected * 2).min(60));
        }
        let authenticated = Instant::now();
        tokio::time::advance(Duration::from_secs(119)).await;
        let delay = retry_delay(&mut backoff, Some(authenticated));
        assert!((Duration::from_secs(30)..=Duration::from_secs(60)).contains(&delay));
        tokio::time::advance(Duration::from_secs(1)).await;
        let delay = retry_delay(&mut backoff, Some(authenticated));
        assert!((Duration::from_millis(500)..=Duration::from_secs(1)).contains(&delay));
        assert_eq!(backoff, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn completion_preserves_timeout_and_transport_reasons() {
        for (elapsed, outcome, expected) in [
            (0, Err(Reason::Transport), Err(Reason::Transport)),
            (1, Ok(serde_json::json!([])), Err(Reason::Timeout)),
            (1, Err(Reason::Unavailable), Err(Reason::Timeout)),
        ] {
            let (reply, receive) = oneshot::channel();
            let request = Request {
                method: Method::PoolQuery,
                deadline: Instant::now() + Duration::from_secs(1),
                reply,
            };
            tokio::time::advance(Duration::from_secs(elapsed)).await;
            request.complete(outcome);
            assert_eq!(receive.await.unwrap(), expected);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_writer_skips_expired_queue_entries() {
        let (tx, rx) = duplex(1);
        let client = WebSocketStream::from_raw_socket(tx, Role::Client, None).await;
        let mut peer = WebSocketStream::from_raw_socket(rx, Role::Server, None).await;
        let (send, queue) = mpsc::channel(3);
        let now = Instant::now();
        for (text, seconds) in [("first", 10), ("expired", 1), ("fresh", 10)] {
            send.try_send(Outbound::Request {
                frame: Message::text(text),
                expires: now + Duration::from_secs(seconds),
            })
            .unwrap();
        }
        drop(send);
        let writer = tokio::spawn(write_frames(client, queue, Duration::from_secs(10)));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(1)).await;
        for expected in ["first", "fresh"] {
            assert_eq!(peer.next().await.unwrap().unwrap(), Message::text(expected));
        }
        writer.await.unwrap().unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_flush_ends_session_at_request_deadline() {
        let (tx, _stalled_peer) = duplex(1);
        let socket = WebSocketStream::from_raw_socket(tx, Role::Client, None).await;
        let (send, queue) = mpsc::channel(1);
        let expires = Instant::now() + Duration::from_secs(2);
        send.try_send(Outbound::Request {
            frame: Message::text("stalled"),
            expires,
        })
        .unwrap();
        assert_eq!(
            write_frames(socket, queue, Duration::from_secs(10)).await,
            Err(Reason::Timeout)
        );
        assert_eq!(Instant::now(), expires);
    }
}
