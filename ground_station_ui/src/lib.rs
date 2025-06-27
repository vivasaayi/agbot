use clap::Parser;
use shared::{
    config::AgroConfig,
    schemas::{WebSocketMessage, Telemetry},
    AgroResult,
};
use std::sync::Arc;
use tracing::{info, error};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::StreamExt;

pub mod web_server;
pub mod cli_interface;

#[derive(Parser, Debug)]
#[command(name = "ground_station_ui")]
#[command(about = "Ground Station UI for agrodrone")]
pub struct Args {
    #[arg(long, help = "Run as web server instead of CLI")]
    pub web: bool,
    
    #[arg(long, help = "Mission control WebSocket URL")]
    pub ws_url: Option<String>,
}

pub struct GroundStationUI {
    config: Arc<AgroConfig>,
}

impl GroundStationUI {
    pub async fn new() -> AgroResult<Self> {
        let config = Arc::new(AgroConfig::load()?);
        Ok(Self { config })
    }

    pub async fn run(&self) -> AgroResult<()> {
        let args = Args::parse();

        if args.web {
            self.run_web_server().await
        } else {
            self.run_cli_interface(&args).await
        }
    }

    async fn run_web_server(&self) -> AgroResult<()> {
        info!("Starting web-based ground station UI");
        
        let server = web_server::WebServer::new(self.config.clone()).await?;
        server.run().await
    }

    async fn run_cli_interface(&self, args: &Args) -> AgroResult<()> {
        info!("Starting CLI-based ground station interface");

        let ws_url = args.ws_url.clone().unwrap_or_else(|| {
            format!("ws://{}/ws", self.config.server.ws_bind_address)
        });

        info!("Connecting to mission control at: {}", ws_url);

        let url = url::Url::parse(&ws_url)
            .map_err(|e| shared::error::AgroError::Network(format!("Invalid WebSocket URL: {}", e)))?;

        let (ws_stream, _) = connect_async(url).await
            .map_err(|e| shared::error::AgroError::Network(format!("WebSocket connection failed: {}", e)))?;

        let (_write, mut read) = ws_stream.split();

        // Spawn CLI interface task
        let cli_handle = tokio::spawn(async move {
            cli_interface::run_cli_interface().await;
        });

        // Handle incoming WebSocket messages
        let ws_handle = tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        match serde_json::from_str::<WebSocketMessage>(&text) {
                            Ok(ws_msg) => {
                                Self::handle_websocket_message(ws_msg).await;
                            }
                            Err(e) => {
                                error!("Failed to parse WebSocket message: {}", e);
                            }
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
        });

        // Wait for either task to complete
        tokio::select! {
            _ = cli_handle => info!("CLI interface finished"),
            _ = ws_handle => info!("WebSocket handler finished"),
        }

        Ok(())
    }

    async fn handle_websocket_message(msg: WebSocketMessage) {
        match msg {
            WebSocketMessage::Telemetry { data } => {
                Self::display_telemetry(&data);
            }
            WebSocketMessage::MissionStatus { mission_id, status } => {
                info!("Mission {} status: {}", mission_id, status);
            }
            WebSocketMessage::LidarUpdate { scan } => {
                info!("LiDAR scan received: {} points", scan.points.len());
            }
            WebSocketMessage::ImageCaptured { image } => {
                info!("Image captured: {}", image.image_id);
            }
            WebSocketMessage::NdviProcessed { result } => {
                info!("NDVI processed: mean={:.3}, vegetation={:.1}%", 
                      result.mean_ndvi, result.vegetation_percentage);
            }
            WebSocketMessage::SystemStatus { status, message } => {
                info!("System {}: {}", status, message);
            }
        }
    }

    fn display_telemetry(telemetry: &Telemetry) {
        println!("\n=== TELEMETRY UPDATE ===");
        println!("Time: {}", telemetry.timestamp.format("%H:%M:%S"));
        println!("Position: {:.6}, {:.6} @ {:.1}m", 
                 telemetry.position.latitude, 
                 telemetry.position.longitude, 
                 telemetry.position.altitude);
        println!("Battery: {}% ({:.1}V)", telemetry.battery_percentage, telemetry.battery_voltage);
        println!("Mode: {} (Armed: {})", telemetry.mode, telemetry.armed);
        println!("Speed: {:.1} m/s ground, {:.1} m/s air", telemetry.ground_speed, telemetry.air_speed);
        println!("Heading: {:.1}° | Alt (rel): {:.1}m", telemetry.heading, telemetry.altitude_relative);
        println!("========================\n");
    }
}
