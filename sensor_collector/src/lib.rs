use clap::Parser;
use shared::{config::AgroConfig, AgroResult, RuntimeMode};
use std::sync::Arc;
use tracing::{info, warn};

pub mod lidar_reader;
pub mod camera_reader;

#[derive(Parser, Debug)]
#[command(name = "sensor_collector")]
#[command(about = "Sensor Collector Service for agrodrone")]
pub struct Args {
    #[arg(long, help = "Configuration file path")]
    pub config: Option<String>,
}

pub struct SensorCollectorService {
    config: Arc<AgroConfig>,
}

impl SensorCollectorService {
    pub async fn new() -> AgroResult<Self> {
        let config = Arc::new(AgroConfig::load()?);
        Ok(Self { config })
    }

    pub async fn run(&self) -> AgroResult<()> {
        info!("Sensor Collector starting in {:?} mode", self.config.runtime_mode);

        // Create data directories
        let data_dir = &self.config.storage.data_root_path;
        tokio::fs::create_dir_all(data_dir).await?;
        
        let lidar_dir = data_dir.join("lidar");
        let camera_dir = data_dir.join("camera");
        tokio::fs::create_dir_all(&lidar_dir).await?;
        tokio::fs::create_dir_all(&camera_dir).await?;

        // Start LiDAR reader
        let lidar_handle = match self.config.runtime_mode {
            RuntimeMode::Flight => {
                info!("Starting LiDAR reader for RPLIDAR A3");
                let reader = lidar_reader::LidarReader::new(
                    self.config.clone(),
                    lidar_dir,
                ).await?;
                Some(tokio::spawn(async move {
                    if let Err(e) = reader.run().await {
                        tracing::error!("LiDAR reader error: {}", e);
                    }
                }))
            }
            RuntimeMode::Simulation => {
                warn!("Running in simulation mode - starting simulated LiDAR");
                let reader = lidar_reader::SimulatedLidarReader::new(
                    self.config.clone(),
                    lidar_dir,
                );
                Some(tokio::spawn(async move {
                    if let Err(e) = reader.run().await {
                        tracing::error!("Simulated LiDAR reader error: {}", e);
                    }
                }))
            }
        };

        // Start camera reader
        let camera_handle = match self.config.runtime_mode {
            RuntimeMode::Flight => {
                info!("Starting multispectral camera reader");
                let reader = camera_reader::CameraReader::new(
                    self.config.clone(),
                    camera_dir,
                ).await?;
                Some(tokio::spawn(async move {
                    if let Err(e) = reader.run().await {
                        tracing::error!("Camera reader error: {}", e);
                    }
                }))
            }
            RuntimeMode::Simulation => {
                warn!("Running in simulation mode - starting simulated camera");
                let reader = camera_reader::SimulatedCameraReader::new(
                    self.config.clone(),
                    camera_dir,
                );
                Some(tokio::spawn(async move {
                    if let Err(e) = reader.run().await {
                        tracing::error!("Simulated camera reader error: {}", e);
                    }
                }))
            }
        };

        // Wait for all readers
        tokio::select! {
            _ = async {
                if let Some(handle) = lidar_handle {
                    handle.await.ok();
                }
            } => info!("LiDAR reader finished"),
            _ = async {
                if let Some(handle) = camera_handle {
                    handle.await.ok();
                }
            } => info!("Camera reader finished"),
        }

        Ok(())
    }
}
