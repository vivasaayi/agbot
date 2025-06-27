use axum::{
    extract::{ws::{WebSocket, WebSocketUpgrade, Message}, State},
    response::Response,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use shared::{config::AgroConfig, schemas::WebSocketMessage, AgroResult};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

pub struct WebSocketServer {
    config: Arc<AgroConfig>,
    event_rx: broadcast::Receiver<WebSocketMessage>,
}

impl WebSocketServer {
    pub fn new(
        config: Arc<AgroConfig>,
        event_rx: broadcast::Receiver<WebSocketMessage>,
    ) -> Self {
        Self { config, event_rx }
    }

    pub async fn run(&self) -> AgroResult<()> {
        let app_state = AppState {
            event_tx: self.event_rx.resubscribe().into(),
        };

        let app = Router::new()
            .route("/ws", get(websocket_handler))
            .with_state(app_state);

        let listener = tokio::net::TcpListener::bind(&self.config.server.ws_bind_address).await?;
        info!("WebSocket server listening on {}", self.config.server.ws_bind_address);

        axum::serve(listener, app).await?;

        Ok(())
    }
}

#[derive(Clone)]
struct AppState {
    event_tx: Arc<broadcast::Receiver<WebSocketMessage>>,
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    info!("New WebSocket connection established");

    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = state.event_tx.resubscribe();

    // Spawn task to handle incoming messages from client
    let recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {                match msg {
                    Ok(Message::Text(_)) | Ok(Message::Binary(_)) => {
                        // Handle incoming messages from client if needed
                        info!("Received message from client: {:?}", msg);
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket close message received");
                        break;
                    }
                    Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                        // Handle ping/pong frames
                    }
                Err(e) => {
                    warn!("WebSocket error: {}", e);
                    break;
                }
            }
        }
    });

    // Spawn task to send events to client
    let send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match serde_json::to_string(&event) {
                Ok(json) => {
                    if sender
                        .send(Message::Text(json))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    warn!("Failed to serialize event: {}", e);
                }
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = recv_task => {},
        _ = send_task => {},
    }

    info!("WebSocket connection closed");
}
