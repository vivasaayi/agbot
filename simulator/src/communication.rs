use anyhow::Result;
use bevy::prelude::*;
use flume::{Receiver, Sender};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

use crate::components::DroneStatus;
use crate::drone_controller::spawn_drone;
use crate::resources::{AppConfig, AppState};
use shared::schemas::{Telemetry, WebSocketMessage};

pub struct CommunicationPlugin;

impl Plugin for CommunicationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, process_incoming_messages);
    }
}

#[derive(Resource)]
pub struct CommunicationChannels {
    pub incoming_receiver: Receiver<IncomingMessage>,
    pub outgoing_sender: Sender<OutgoingMessage>,
}

// Setup function to be called from main before Bevy starts
pub async fn setup_communication_task(config: &AppConfig) -> CommunicationChannels {
    info!("Setting up communication channels...");

    let (incoming_sender, incoming_receiver) = flume::unbounded();
    let (outgoing_sender, outgoing_receiver) = flume::unbounded();

    // Spawn the communication task
    let websocket_url = config.websocket_url.clone();
    tokio::spawn(async move {
        if let Err(e) =
            run_communication_loop(websocket_url, incoming_sender, outgoing_receiver).await
        {
            error!("Communication loop failed: {}", e);
        }
    });

    CommunicationChannels {
        incoming_receiver,
        outgoing_sender,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IncomingMessage {
    Telemetry(Telemetry),
    MissionStatus {
        mission_id: uuid::Uuid,
        status: String,
    },
    LidarUpdate(shared::schemas::LidarScan),
    ImageCaptured(shared::schemas::MultispectralImage),
    NdviProcessed(shared::schemas::NdviResult),
    SystemStatus {
        status: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutgoingMessage {
    SubscribeToUpdates,
    RequestMissionData(String),
    RequestReplayData { start_time: f64, end_time: f64 },
    SetViewMode(ViewMode),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViewMode {
    Live,
    Replay,
}

async fn run_communication_loop(
    websocket_url: String,
    incoming_sender: Sender<IncomingMessage>,
    outgoing_receiver: Receiver<OutgoingMessage>,
) -> Result<()> {
    info!("Attempting to connect to WebSocket at {}", websocket_url);

    // Exponential backoff configuration
    const INITIAL_BACKOFF_SECS: u64 = 1;
    const MAX_BACKOFF_SECS: u64 = 60;
    const BACKOFF_MULTIPLIER: u64 = 2;

    let mut backoff_secs = INITIAL_BACKOFF_SECS;
    let mut consecutive_failures = 0u32;

    loop {
        match connect_async(&websocket_url).await {
            Ok((ws_stream, _)) => {
                info!("Connected to WebSocket successfully");

                // Reset backoff on successful connection
                backoff_secs = INITIAL_BACKOFF_SECS;
                consecutive_failures = 0;

                let (mut ws_sender, mut ws_receiver) = ws_stream.split();

                // Send subscription message
                let subscribe_msg = OutgoingMessage::SubscribeToUpdates;
                let msg_json = serde_json::to_string(&subscribe_msg)?;
                ws_sender.send(Message::Text(msg_json)).await?;

                // Handle incoming and outgoing messages
                loop {
                    tokio::select! {
                        // Handle incoming WebSocket messages
                        msg = ws_receiver.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    match serde_json::from_str::<WebSocketMessage>(&text) {
                                        Ok(ws_msg) => {
                                            // Convert WebSocketMessage to IncomingMessage
                                            let incoming_msg = match ws_msg {
                                                WebSocketMessage::Telemetry { data } => IncomingMessage::Telemetry(data),
                                                WebSocketMessage::MissionStatus { mission_id, status } => IncomingMessage::MissionStatus { mission_id, status },
                                                WebSocketMessage::LidarUpdate { scan } => IncomingMessage::LidarUpdate(scan),
                                                WebSocketMessage::ImageCaptured { image } => IncomingMessage::ImageCaptured(image),
                                                WebSocketMessage::NdviProcessed { result } => IncomingMessage::NdviProcessed(result),
                                                WebSocketMessage::SystemStatus { status, message } => IncomingMessage::SystemStatus { status, message },
                                            };

                                            if let Err(e) = incoming_sender.send(incoming_msg) {
                                                warn!("Failed to send incoming message: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            warn!("Failed to parse incoming WebSocket message: {}", e);
                                        }
                                    }
                                }
                                Some(Ok(Message::Ping(data))) => {
                                    // Respond to ping with pong to keep connection alive
                                    if let Err(e) = ws_sender.send(Message::Pong(data)).await {
                                        warn!("Failed to send pong: {}", e);
                                        break;
                                    }
                                }
                                Some(Ok(Message::Close(_))) => {
                                    info!("WebSocket connection closed by server");
                                    break;
                                }
                                Some(Err(e)) => {
                                    warn!("WebSocket error: {} - will reconnect", e);
                                    break;
                                }
                                None => {
                                    info!("WebSocket stream ended");
                                    break;
                                }
                                _ => {}
                            }
                        }

                        // Handle outgoing messages
                        msg = outgoing_receiver.recv_async() => {
                            match msg {
                                Ok(outgoing_msg) => {
                                    let msg_json = serde_json::to_string(&outgoing_msg)?;
                                    if let Err(e) = ws_sender.send(Message::Text(msg_json)).await {
                                        error!("Failed to send outgoing message: {}", e);
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!("Outgoing message channel error: {}", e);
                                }
                            }
                        }
                    }
                }

                // Connection dropped, apply backoff before reconnecting
                info!(
                    "Connection lost, reconnecting in {} seconds...",
                    backoff_secs
                );
            }
            Err(e) => {
                consecutive_failures += 1;

                // Log with appropriate severity based on failure count
                if consecutive_failures <= 3 {
                    warn!(
                        "Failed to connect to WebSocket (attempt {}): {} - retrying in {}s",
                        consecutive_failures, e, backoff_secs
                    );
                } else if consecutive_failures % 10 == 0 {
                    // Only log every 10th failure after initial attempts to reduce spam
                    warn!(
                        "WebSocket still unreachable after {} attempts - is the server running?",
                        consecutive_failures
                    );
                }
            }
        }

        // Wait with exponential backoff
        tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;

        // Increase backoff for next attempt, capped at max
        backoff_secs = (backoff_secs * BACKOFF_MULTIPLIER).min(MAX_BACKOFF_SECS);
    }
}

fn process_incoming_messages(
    mut commands: Commands,
    channels: Option<Res<CommunicationChannels>>,
    mut app_state: ResMut<AppState>,
    mut drone_registry: ResMut<crate::resources::DroneRegistry>,
    mut mission_data: ResMut<crate::resources::MissionData>,
    mut drone_query: Query<(&mut Transform, &mut crate::components::Drone)>,
) {
    let Some(channels) = channels else {
        return;
    };

    // Process all available messages
    while let Ok(message) = channels.incoming_receiver.try_recv() {
        match message {
            IncomingMessage::Telemetry(telemetry) => {
                // For now, we'll use a generated drone ID based on telemetry
                // In a real implementation, this would come from the telemetry data
                let drone_id = "drone_1".to_string();

                // Convert GPS coordinates to local ENU coordinates
                // This is a simplified conversion - in practice you'd use proper geodesy
                let position = Vec3::new(
                    telemetry.position.longitude as f32,
                    telemetry.altitude_relative,
                    telemetry.position.latitude as f32,
                );

                // Create rotation from heading
                let rotation = Quat::from_rotation_y(telemetry.heading.to_radians() as f32);

                // Find existing drone or create new one
                let mut found = false;
                for (mut transform, mut drone) in drone_query.iter_mut() {
                    if drone.id == drone_id {
                        transform.translation = position;
                        transform.rotation = rotation;

                        drone.status = if telemetry.armed {
                            DroneStatus::Flying
                        } else {
                            DroneStatus::Idle
                        };

                        found = true;
                        break;
                    }
                }

                if !found {
                    // Spawn new drone
                    spawn_drone(&mut commands, &mut drone_registry, drone_id, position);
                }

                app_state.connected = true;
            }

            IncomingMessage::MissionStatus { mission_id, status } => {
                mission_data.current_mission = Some(mission_id.to_string());
                info!("Mission {} status: {}", mission_id, status);
            }

            IncomingMessage::LidarUpdate(scan) => {
                // Handle LiDAR scan updates
                info!("Received LiDAR scan with {} points", scan.points.len());
            }

            IncomingMessage::ImageCaptured(image) => {
                // Handle captured image updates
                info!("Received multispectral image: {}", image.image_id);
            }

            IncomingMessage::NdviProcessed(result) => {
                // Handle NDVI processing results
                info!(
                    "NDVI processed: mean={:.3}, vegetation={:.1}%",
                    result.mean_ndvi, result.vegetation_percentage
                );
            }

            IncomingMessage::SystemStatus { status, message } => {
                info!("System status: {} - {}", status, message);
            }
        }
    }
}
