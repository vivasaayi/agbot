use clap::Parser;
use shared::{
    config::AgroConfig,
    schemas::{Telemetry, WebSocketMessage},
    AgroResult,
};
use std::sync::Arc;
use tokio::sync::watch;
use tracing::{error, info};

pub mod cli_interface;
pub mod link_client;
pub mod web_server;

pub use link_client::{
    run_websocket_client_until, run_websocket_client_with_handler_until, shared_link_state,
    ConnectionState, LinkStateMachine, LinkStateSnapshot, ReconnectPolicy, SharedLinkState,
};

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
    link_state: SharedLinkState,
}

impl GroundStationUI {
    pub async fn new() -> AgroResult<Self> {
        let config = Arc::new(AgroConfig::load()?);
        let link_state = shared_link_state(ReconnectPolicy::default());
        Ok(Self { config, link_state })
    }

    pub async fn run(&self) -> AgroResult<()> {
        let args = Args::parse();

        if args.web {
            self.run_web_server(&args).await
        } else {
            self.run_cli_interface(&args).await
        }
    }

    async fn run_web_server(&self, args: &Args) -> AgroResult<()> {
        info!("Starting web-based ground station UI");

        let server =
            web_server::WebServer::new(self.config.clone(), self.link_state.clone()).await?;
        let (stop_tx, stop_rx) = watch::channel(false);
        let ws_url = self.mission_control_ws_url(args);
        let link_state = self.link_state.clone();
        let ws_handle = tokio::spawn(async move {
            if let Err(err) = run_websocket_client_with_handler_until(
                ws_url,
                link_state,
                stop_rx,
                GroundStationUI::handle_websocket_message,
            )
            .await
            {
                error!("WebSocket client stopped: {}", err);
            }
        });

        let result = server.run().await;
        let _ = stop_tx.send(true);
        let _ = ws_handle.await;
        result
    }

    async fn run_cli_interface(&self, args: &Args) -> AgroResult<()> {
        info!("Starting CLI-based ground station interface");

        let ws_url = self.mission_control_ws_url(args);

        info!("Connecting to mission control at: {}", ws_url);

        let (stop_tx, stop_rx) = watch::channel(false);
        let link_state = self.link_state.clone();
        let ws_handle = tokio::spawn(async move {
            if let Err(err) = run_websocket_client_with_handler_until(
                ws_url,
                link_state,
                stop_rx,
                GroundStationUI::handle_websocket_message,
            )
            .await
            {
                error!("WebSocket client stopped: {}", err);
            }
        });

        cli_interface::run_cli_interface(self.link_state.clone()).await;
        let _ = stop_tx.send(true);
        let _ = ws_handle.await;

        Ok(())
    }

    pub fn link_state(&self) -> SharedLinkState {
        self.link_state.clone()
    }

    fn mission_control_ws_url(&self, args: &Args) -> String {
        args.ws_url
            .clone()
            .unwrap_or_else(|| format!("ws://{}/ws", self.config.server.ws_bind_address))
    }

    fn handle_websocket_message(msg: WebSocketMessage) {
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
                info!(
                    "NDVI processed: mean={:.3}, vegetation={:.1}%",
                    result.mean_ndvi, result.vegetation_percentage
                );
            }
            WebSocketMessage::SystemStatus { status, message } => {
                info!("System {}: {}", status, message);
            }
        }
    }

    fn display_telemetry(telemetry: &Telemetry) {
        println!("\n=== TELEMETRY UPDATE ===");
        println!("Time: {}", telemetry.timestamp.format("%H:%M:%S"));
        println!(
            "Position: {:.6}, {:.6} @ {:.1}m",
            telemetry.position.latitude, telemetry.position.longitude, telemetry.position.altitude
        );
        println!(
            "Battery: {}% ({:.1}V)",
            telemetry.battery_percentage, telemetry.battery_voltage
        );
        println!("Mode: {} (Armed: {})", telemetry.mode, telemetry.armed);
        println!(
            "Speed: {:.1} m/s ground, {:.1} m/s air",
            telemetry.ground_speed, telemetry.air_speed
        );
        println!(
            "Heading: {:.1}° | Alt (rel): {:.1}m",
            telemetry.heading, telemetry.altitude_relative
        );
        println!("========================\n");
    }
}
