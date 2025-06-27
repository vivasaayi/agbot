use clap::Parser;
use shared::{config::AgroConfig, AgroResult, RuntimeMode};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

pub mod mavlink_client;
pub mod websocket_server;
pub mod api_server;

#[derive(Parser, Debug)]
#[command(name = "mission_control")]
#[command(about = "Mission Control Service for agrodrone")]
pub struct Args {
    #[arg(long, help = "Configuration file path")]
    pub config: Option<String>,
}

pub struct MissionControlService {
    config: Arc<AgroConfig>,
    event_tx: broadcast::Sender<shared::schemas::WebSocketMessage>,
}

impl MissionControlService {
    pub async fn new() -> AgroResult<Self> {
        let config = Arc::new(AgroConfig::load()?);
        let (event_tx, _) = broadcast::channel(1000);

        Ok(Self {
            config,
            event_tx,
        })
    }

    pub async fn run(&self) -> AgroResult<()> {
        info!("Mission Control starting in {:?} mode", self.config.runtime_mode);

        // Create necessary directories
        tokio::fs::create_dir_all(&self.config.storage.data_root_path).await?;
        tokio::fs::create_dir_all(&self.config.storage.mission_data_path).await?;

        // Start MAVLink client
        let mavlink_handle = match self.config.runtime_mode {
            RuntimeMode::Flight => {
                info!("Starting MAVLink client for flight controller");
                let client = mavlink_client::MavlinkClient::new(
                    self.config.clone(),
                    self.event_tx.clone(),
                ).await?;
                Some(tokio::spawn(async move {
                    if let Err(e) = client.run().await {
                        tracing::error!("MAVLink client error: {}", e);
                    }
                }))
            }
            RuntimeMode::Simulation => {
                warn!("Running in simulation mode - MAVLink client disabled");
                let client = mavlink_client::SimulatedMavlinkClient::new(
                    self.config.clone(),
                    self.event_tx.clone(),
                );
                Some(tokio::spawn(async move {
                    if let Err(e) = client.run().await {
                        tracing::error!("Simulated MAVLink client error: {}", e);
                    }
                }))
            }
        };

        // Start WebSocket server
        let ws_server = websocket_server::WebSocketServer::new(
            self.config.clone(),
            self.event_tx.subscribe(),
        );
        let ws_handle = tokio::spawn(async move {
            if let Err(e) = ws_server.run().await {
                tracing::error!("WebSocket server error: {}", e);
            }
        });

        // Start API server
        let api_server = api_server::ApiServer::new(
            self.config.clone(),
            self.event_tx.clone(),
        );
        let api_handle = tokio::spawn(async move {
            if let Err(e) = api_server.run().await {
                tracing::error!("API server error: {}", e);
            }
        });

        // Wait for all services
        tokio::select! {
            _ = ws_handle => info!("WebSocket server finished"),
            _ = api_handle => info!("API server finished"),
            _ = async {
                if let Some(handle) = mavlink_handle {
                    handle.await.ok();
                }
            } => info!("MAVLink client finished"),
        }

        Ok(())
    }
}
