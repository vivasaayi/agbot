use anyhow::Result;
use bevy::prelude::*;
use flume::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use tracing::{info, warn, error};

use crate::resources::{AppConfig, AppState, TimestampedData};
use crate::drone_controller::spawn_drone;
use crate::components::DroneStatus;

pub struct CommunicationPlugin;

impl Plugin for CommunicationPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Update, process_incoming_messages);
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
        if let Err(e) = run_communication_loop(websocket_url, incoming_sender, outgoing_receiver).await {
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
    DroneUpdate(DroneUpdateMessage),
    MissionUpdate(MissionUpdateMessage),
    SystemStatus(SystemStatusMessage),
    ReplayData(Vec<TimestampedData>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutgoingMessage {
    SubscribeToUpdates,
    RequestMissionData(String),
    RequestReplayData { start_time: f64, end_time: f64 },
    SetViewMode(ViewMode),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneUpdateMessage {
    pub drone_id: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub status: String,
    pub battery_level: f32,
    pub altitude: f32,
    pub speed: f32,
    pub timestamp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionUpdateMessage {
    pub mission_id: String,
    pub waypoints: Vec<[f32; 3]>,
    pub current_waypoint: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatusMessage {
    pub connected_drones: Vec<String>,
    pub active_missions: Vec<String>,
    pub system_health: String,
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
    
    loop {
        match connect_async(&websocket_url).await {
            Ok((ws_stream, _)) => {
                info!("Connected to WebSocket successfully");
                
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
                                    match serde_json::from_str::<IncomingMessage>(&text) {
                                        Ok(parsed_msg) => {
                                            if let Err(e) = incoming_sender.send(parsed_msg) {
                                                warn!("Failed to send incoming message: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            warn!("Failed to parse incoming message: {}", e);
                                        }
                                    }
                                }
                                Some(Ok(Message::Close(_))) => {
                                    info!("WebSocket connection closed by server");
                                    break;
                                }
                                Some(Err(e)) => {
                                    error!("WebSocket error: {}", e);
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
            }
            Err(e) => {
                error!("Failed to connect to WebSocket: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
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
    let Some(channels) = channels else { return; };
    
    // Process all available messages
    while let Ok(message) = channels.incoming_receiver.try_recv() {
        match message {
            IncomingMessage::DroneUpdate(update) => {
                let drone_id = update.drone_id.clone();
                let position = Vec3::from_array(update.position);
                let rotation = Quat::from_array(update.rotation);
                
                // Find existing drone or create new one
                let mut found = false;
                for (mut transform, mut drone) in drone_query.iter_mut() {
                    if drone.id == drone_id {
                        transform.translation = position;
                        transform.rotation = rotation;
                        
                        drone.status = match update.status.as_str() {
                            "idle" => DroneStatus::Idle,
                            "flying" => DroneStatus::Flying,
                            "mission" => DroneStatus::Mission,
                            "returning" => DroneStatus::Returning,
                            "landing" => DroneStatus::Landing,
                            "error" => DroneStatus::Error,
                            _ => DroneStatus::Idle,
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
            
            IncomingMessage::MissionUpdate(update) => {
                mission_data.current_mission = Some(update.mission_id);
                mission_data.waypoints = update.waypoints.into_iter()
                    .map(Vec3::from_array)
                    .collect();
            }
            
            IncomingMessage::SystemStatus(status) => {
                info!("System status: {} drones connected, {} active missions", 
                      status.connected_drones.len(), 
                      status.active_missions.len());
            }
            
            IncomingMessage::ReplayData(data) => {
                mission_data.replay_data = data;
                mission_data.replay_index = 0;
                info!("Loaded {} data points for replay", mission_data.replay_data.len());
            }
        }
    }
}
