use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
    Router,
};
use shared::{
    config::AgroConfig,
    schemas::{Mission, WebSocketMessage},
    AgroResult,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

pub struct ApiServer {
    config: Arc<AgroConfig>,
    event_tx: broadcast::Sender<WebSocketMessage>,
}

impl ApiServer {
    pub fn new(
        config: Arc<AgroConfig>,
        event_tx: broadcast::Sender<WebSocketMessage>,
    ) -> Self {
        Self { config, event_tx }
    }

    pub async fn run(&self) -> AgroResult<()> {
        let app_state = ApiState {
            config: self.config.clone(),
            event_tx: self.event_tx.clone(),
        };

        let app = Router::new()
            .route("/health", get(health_check))
            .route("/missions", post(upload_mission))
            .route("/missions", get(list_missions))
            .route("/telemetry", get(get_current_telemetry))
            .with_state(app_state);

        let listener = tokio::net::TcpListener::bind(&self.config.server.api_bind_address).await?;
        info!("API server listening on {}", self.config.server.api_bind_address);

        axum::serve(listener, app).await?;

        Ok(())
    }
}

#[derive(Clone)]
struct ApiState {
    config: Arc<AgroConfig>,
    event_tx: broadcast::Sender<WebSocketMessage>,
}

async fn health_check() -> &'static str {
    "OK"
}

async fn upload_mission(
    State(state): State<ApiState>,
    Json(mission): Json<Mission>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    info!("Received mission upload: {}", mission.name);

    // Save mission to file
    let mission_path = state
        .config
        .storage
        .mission_data_path
        .join(format!("{}.json", mission.id));

    let mission_json = serde_json::to_string_pretty(&mission)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tokio::fs::write(&mission_path, mission_json)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Send mission status update
    let msg = WebSocketMessage::MissionStatus {
        mission_id: mission.id,
        status: "uploaded".to_string(),
    };

    if let Err(e) = state.event_tx.send(msg) {
        warn!("Failed to send mission status: {}", e);
    }

    Ok(ResponseJson(serde_json::json!({
        "success": true,
        "mission_id": mission.id,
        "message": "Mission uploaded successfully"
    })))
}

async fn list_missions(
    State(state): State<ApiState>,
) -> Result<ResponseJson<Vec<Mission>>, StatusCode> {
    let mut missions = Vec::new();

    let mut entries = tokio::fs::read_dir(&state.config.storage.mission_data_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        if let Some(extension) = entry.path().extension() {
            if extension == "json" {
                let content = tokio::fs::read_to_string(entry.path())
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                if let Ok(mission) = serde_json::from_str::<Mission>(&content) {
                    missions.push(mission);
                }
            }
        }
    }

    Ok(ResponseJson(missions))
}

async fn get_current_telemetry(
    State(_state): State<ApiState>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    // Return mock telemetry for now
    // In a real implementation, you'd get the latest telemetry from the MAVLink client
    Ok(ResponseJson(serde_json::json!({
        "message": "Use WebSocket connection for real-time telemetry"
    })))
}
