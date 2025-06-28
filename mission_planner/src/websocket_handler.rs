use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::{Mission, MissionPlannerService};
use crate::mavlink_integration::{MAVLinkConverter, MAVLinkMission};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    // Client to Server
    DeployMission { mission: Mission },
    DeployMAVLinkMission { data: String },
    SubscribeToUpdates,
    GetMissionStatus { mission_id: Uuid },
    
    // Server to Client
    MissionDeployed { mission_id: Uuid, success: bool, error: Option<String> },
    MissionStatus { mission_id: Uuid, status: String, progress: f32 },
    DroneTelemetry { drone_id: String, telemetry: DroneTelemettry },
    SystemStatus { connected_drones: Vec<String>, active_missions: Vec<Uuid> },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneTelemettry {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f32,
    pub heading: f32,
    pub speed: f32,
    pub battery_level: f32,
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct WebSocketHandler {
    mission_service: Arc<Mutex<MissionPlannerService>>,
    broadcast_tx: broadcast::Sender<WebSocketMessage>,
}

impl WebSocketHandler {
    pub fn new(mission_service: Arc<Mutex<MissionPlannerService>>) -> Self {
        let (broadcast_tx, _) = broadcast::channel(100);
        Self {
            mission_service,
            broadcast_tx,
        }
    }

    pub async fn handle_upgrade(
        ws: WebSocketUpgrade,
        State(handler): State<Arc<WebSocketHandler>>,
    ) -> Response {
        ws.on_upgrade(move |socket| handler.handle_socket(socket))
    }

    async fn handle_socket(self: Arc<Self>, socket: WebSocket) {
        let (sender, mut receiver) = socket.split();
        let mut broadcast_rx = self.broadcast_tx.subscribe();

        // Create a channel for sending messages from the message handler
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

        // Spawn task to handle outgoing messages
        let broadcast_task = tokio::spawn(async move {
            let mut sender = sender;
            loop {
                tokio::select! {
                    // Handle broadcast messages
                    msg = broadcast_rx.recv() => {
                        match msg {
                            Ok(msg) => {
                                let json_msg = match serde_json::to_string(&msg) {
                                    Ok(json) => json,
                                    Err(e) => {
                                        error!("Failed to serialize message: {}", e);
                                        continue;
                                    }
                                };
                                
                                if sender.send(Message::Text(json_msg)).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    // Handle direct messages from handler
                    msg = rx.recv() => {
                        match msg {
                            Some(msg) => {
                                if sender.send(msg).await.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        // Handle incoming messages
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Err(e) = self.handle_message(text, &tx).await {
                        error!("Error handling message: {}", e);
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket connection closed");
                    break;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        broadcast_task.abort();
    }

    async fn handle_message(
        &self,
        text: String,
        sender: &tokio::sync::mpsc::UnboundedSender<Message>,
    ) -> Result<()> {
        let message: WebSocketMessage = serde_json::from_str(&text)?;

        match message {
            WebSocketMessage::DeployMission { mission } => {
                self.handle_deploy_mission(mission, sender).await?;
            }
            WebSocketMessage::DeployMAVLinkMission { data } => {
                self.handle_deploy_mavlink_mission(data, sender).await?;
            }
            WebSocketMessage::SubscribeToUpdates => {
                info!("Client subscribed to updates");
                // Send current system status
                let status_msg = WebSocketMessage::SystemStatus {
                    connected_drones: vec!["drone_001".to_string()], // Mock data
                    active_missions: vec![],
                };
                let json_msg = serde_json::to_string(&status_msg)?;
                sender.send(Message::Text(json_msg))?;
            }
            WebSocketMessage::GetMissionStatus { mission_id } => {
                // Mock mission status
                let status_msg = WebSocketMessage::MissionStatus {
                    mission_id,
                    status: "In Progress".to_string(),
                    progress: 0.5,
                };
                let json_msg = serde_json::to_string(&status_msg)?;
                sender.send(Message::Text(json_msg))?;
            }
            _ => {
                warn!("Received unexpected message type");
            }
        }

        Ok(())
    }

    async fn handle_deploy_mission(
        &self,
        mission: Mission,
        sender: &tokio::sync::mpsc::UnboundedSender<Message>,
    ) -> Result<()> {
        info!("Deploying mission: {} with {} waypoints", mission.name, mission.waypoints.len());

        // Convert to MAVLink
        let mavlink_mission = MAVLinkConverter::mission_to_mavlink(&mission)?;
        let waypoint_file = MAVLinkConverter::to_waypoint_file(&mavlink_mission);

        // Save mission
        let mut service = self.mission_service.lock().await;
        service.create_mission(mission.clone()).await?;

        // TODO: Send to actual drone system
        info!("MAVLink waypoint file generated ({} items)", mavlink_mission.count);
        info!("Mission waypoints:\n{}", waypoint_file);

        // Simulate deployment
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let response = WebSocketMessage::MissionDeployed {
            mission_id: mission.id,
            success: true,
            error: None,
        };

        let json_msg = serde_json::to_string(&response)?;
        sender.send(Message::Text(json_msg))?;

        // Broadcast mission status updates
        let _ = self.broadcast_tx.send(WebSocketMessage::MissionStatus {
            mission_id: mission.id,
            status: "Deployed".to_string(),
            progress: 0.0,
        });

        Ok(())
    }

    async fn handle_deploy_mavlink_mission(
        &self,
        mavlink_data: String,
        sender: &tokio::sync::mpsc::UnboundedSender<Message>,
    ) -> Result<()> {
        info!("Deploying MAVLink mission data ({} bytes)", mavlink_data.len());

        // TODO: Parse and validate MAVLink data
        // TODO: Send to actual drone system

        // Simulate deployment
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let mission_id = Uuid::new_v4();
        let response = WebSocketMessage::MissionDeployed {
            mission_id,
            success: true,
            error: None,
        };

        let json_msg = serde_json::to_string(&response)?;
        sender.send(Message::Text(json_msg)).map_err(|e| anyhow::anyhow!("Send error: {}", e))?;

        Ok(())
    }

    pub async fn broadcast_telemetry(&self, telemetry: DroneTelemettry) {
        let msg = WebSocketMessage::DroneTelemetry {
            drone_id: "drone_001".to_string(),
            telemetry,
        };
        let _ = self.broadcast_tx.send(msg);
    }

    pub async fn broadcast_mission_status(&self, mission_id: Uuid, status: String, progress: f32) {
        let msg = WebSocketMessage::MissionStatus {
            mission_id,
            status,
            progress,
        };
        let _ = self.broadcast_tx.send(msg);
    }
}
