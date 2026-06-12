use futures_util::StreamExt;
use serde::Serialize;
use shared::{schemas::WebSocketMessage, AgroResult};
use std::{fmt, sync::Arc, time::Duration};
use tokio::sync::{watch, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{error, info};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Connecting,
    Connected,
    Stale,
    Lost,
    Reconnecting,
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            ConnectionState::Connecting => "Connecting",
            ConnectionState::Connected => "Connected",
            ConnectionState::Stale => "Stale",
            ConnectionState::Lost => "Lost",
            ConnectionState::Reconnecting => "Reconnecting",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectPolicy {
    initial_backoff: Duration,
    max_backoff: Duration,
    multiplier: u32,
}

impl ReconnectPolicy {
    pub fn new(initial_backoff: Duration, max_backoff: Duration, multiplier: u32) -> Self {
        Self {
            initial_backoff,
            max_backoff: max_backoff.max(initial_backoff),
            multiplier: multiplier.max(1),
        }
    }

    fn delay_for_attempt(&self, attempts: u32) -> Duration {
        let mut delay = self.initial_backoff;
        for _ in 1..attempts.max(1) {
            delay = delay.saturating_mul(self.multiplier);
            if delay >= self.max_backoff {
                return self.max_backoff;
            }
        }
        delay.min(self.max_backoff)
    }
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self::new(Duration::from_millis(500), Duration::from_secs(30), 2)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkStateSnapshot {
    pub state: ConnectionState,
    pub reconnect_attempts: u32,
    #[serde(serialize_with = "duration_as_millis")]
    pub next_backoff: Duration,
    pub last_error: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct LinkStateMachine {
    policy: ReconnectPolicy,
    snapshot: LinkStateSnapshot,
}

pub type SharedLinkState = Arc<RwLock<LinkStateMachine>>;

impl LinkStateMachine {
    pub fn new(policy: ReconnectPolicy) -> Self {
        Self {
            policy,
            snapshot: LinkStateSnapshot {
                state: ConnectionState::Connecting,
                reconnect_attempts: 0,
                next_backoff: policy.delay_for_attempt(0),
                last_error: None,
                updated_at: current_timestamp(),
            },
        }
    }

    pub fn snapshot(&self) -> LinkStateSnapshot {
        self.snapshot.clone()
    }

    pub fn mark_connecting(&mut self) {
        self.transition(ConnectionState::Connecting, None);
    }

    pub fn mark_connected(&mut self) {
        self.snapshot.reconnect_attempts = 0;
        self.snapshot.next_backoff = self.policy.delay_for_attempt(0);
        self.transition(ConnectionState::Connected, None);
    }

    pub fn mark_stale(&mut self) {
        self.transition(ConnectionState::Stale, None);
    }

    pub fn mark_lost(&mut self, error: impl Into<String>) {
        self.snapshot.reconnect_attempts = self.snapshot.reconnect_attempts.saturating_add(1);
        self.snapshot.next_backoff = self
            .policy
            .delay_for_attempt(self.snapshot.reconnect_attempts);
        self.transition(ConnectionState::Lost, Some(error.into()));
    }

    pub fn mark_reconnecting(&mut self) {
        self.transition(ConnectionState::Reconnecting, None);
    }

    fn transition(&mut self, state: ConnectionState, error: Option<String>) {
        self.snapshot.state = state;
        self.snapshot.last_error = error;
        self.snapshot.updated_at = current_timestamp();
    }
}

pub fn shared_link_state(policy: ReconnectPolicy) -> SharedLinkState {
    Arc::new(RwLock::new(LinkStateMachine::new(policy)))
}

pub async fn run_websocket_client_until(
    ws_url: String,
    link_state: SharedLinkState,
    stop_rx: watch::Receiver<bool>,
) -> AgroResult<()> {
    run_websocket_client_with_handler_until(ws_url, link_state, stop_rx, |_| {}).await
}

pub async fn run_websocket_client_with_handler_until<F>(
    ws_url: String,
    link_state: SharedLinkState,
    mut stop_rx: watch::Receiver<bool>,
    mut handle_message: F,
) -> AgroResult<()>
where
    F: FnMut(WebSocketMessage) + Send,
{
    let mut first_attempt = true;
    loop {
        if *stop_rx.borrow() {
            return Ok(());
        }

        {
            let mut state = link_state.write().await;
            if first_attempt {
                state.mark_connecting();
            } else {
                state.mark_reconnecting();
            }
        }
        first_attempt = false;

        match connect_async(ws_url.as_str()).await {
            Ok((ws_stream, _)) => {
                info!("Connected to mission control WebSocket at {}", ws_url);
                link_state.write().await.mark_connected();
                let (_write, mut read) = ws_stream.split();

                loop {
                    tokio::select! {
                        changed = stop_rx.changed() => {
                            if changed.is_ok() && *stop_rx.borrow() {
                                return Ok(());
                            }
                        }
                        message = read.next() => {
                            match message {
                                Some(Ok(Message::Text(text))) => {
                                    match serde_json::from_str::<WebSocketMessage>(&text) {
                                        Ok(ws_msg) => handle_message(ws_msg),
                                        Err(err) => error!("Failed to parse WebSocket message: {}", err),
                                    }
                                }
                                Some(Ok(Message::Close(_))) => {
                                    link_state.write().await.mark_lost("server closed connection");
                                    break;
                                }
                                Some(Ok(_)) => {}
                                Some(Err(err)) => {
                                    link_state.write().await.mark_lost(err.to_string());
                                    break;
                                }
                                None => {
                                    link_state.write().await.mark_lost("server closed stream");
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Err(err) => {
                link_state.write().await.mark_lost(err.to_string());
            }
        }

        let delay = link_state.read().await.snapshot().next_backoff;
        if !wait_for_retry(delay, &mut stop_rx).await {
            return Ok(());
        }
    }
}

async fn wait_for_retry(delay: Duration, stop_rx: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => true,
        changed = stop_rx.changed() => {
            changed.is_err() || !*stop_rx.borrow()
        }
    }
}

fn duration_as_millis<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u64(duration.as_millis().min(u64::MAX as u128) as u64)
}

fn current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc as StdArc,
    };
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    #[test]
    fn link_state_machine_tracks_drop_and_bounded_backoff() {
        let policy = ReconnectPolicy::new(Duration::from_millis(100), Duration::from_secs(1), 2);
        let mut link = LinkStateMachine::new(policy);
        assert_eq!(link.snapshot().state, ConnectionState::Connecting);

        link.mark_connected();
        assert_eq!(link.snapshot().state, ConnectionState::Connected);

        link.mark_lost("server closed");
        let lost = link.snapshot();
        assert_eq!(lost.state, ConnectionState::Lost);
        assert_eq!(lost.next_backoff, Duration::from_millis(100));

        link.mark_reconnecting();
        assert_eq!(link.snapshot().state, ConnectionState::Reconnecting);
        link.mark_lost("retry failed");
        assert_eq!(link.snapshot().next_backoff, Duration::from_millis(200));

        for _ in 0..8 {
            link.mark_reconnecting();
            link.mark_lost("still down");
        }
        assert_eq!(link.snapshot().next_backoff, Duration::from_secs(1));
    }

    #[tokio::test]
    async fn websocket_client_recovers_after_stub_server_drop() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accepted = StdArc::new(AtomicUsize::new(0));
        let server_accepted = accepted.clone();
        let server = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let connection_index = server_accepted.fetch_add(1, Ordering::SeqCst);
                let websocket = accept_async(stream).await.unwrap();
                if connection_index == 0 {
                    drop(websocket);
                } else {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        });

        let state = shared_link_state(ReconnectPolicy::new(
            Duration::from_millis(10),
            Duration::from_millis(50),
            2,
        ));
        let (stop_tx, stop_rx) = watch::channel(false);
        let client_state = state.clone();
        let client = tokio::spawn(async move {
            run_websocket_client_until(format!("ws://{addr}"), client_state, stop_rx)
                .await
                .unwrap();
        });

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let snapshot = state.read().await.snapshot();
                if accepted.load(Ordering::SeqCst) >= 2
                    && snapshot.state == ConnectionState::Connected
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();

        stop_tx.send(true).unwrap();
        client.await.unwrap();
        server.abort();
    }

    #[tokio::test]
    async fn unreachable_server_surfaces_lost_with_bounded_backoff() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let state = shared_link_state(ReconnectPolicy::new(
            Duration::from_millis(5),
            Duration::from_millis(20),
            2,
        ));
        let (stop_tx, stop_rx) = watch::channel(false);
        let client_state = state.clone();
        let client = tokio::spawn(async move {
            run_websocket_client_until(format!("ws://{addr}"), client_state, stop_rx)
                .await
                .unwrap();
        });

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let snapshot = state.read().await.snapshot();
                if snapshot.state == ConnectionState::Lost && snapshot.reconnect_attempts > 0 {
                    assert!(snapshot.next_backoff <= Duration::from_millis(20));
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();

        stop_tx.send(true).unwrap();
        client.await.unwrap();
    }
}
