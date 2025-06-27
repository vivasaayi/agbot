use shared::{
    config::AgroConfig,
    schemas::{LidarPoint, LidarScan},
    AgroResult,
};
use std::{path::PathBuf, sync::Arc};
use tokio_serial::SerialPortBuilderExt;
use tracing::{info, error};

pub struct LidarReader {
    config: Arc<AgroConfig>,
    data_dir: PathBuf,
}

impl LidarReader {
    pub async fn new(config: Arc<AgroConfig>, data_dir: PathBuf) -> AgroResult<Self> {
        Ok(Self { config, data_dir })
    }

    pub async fn run(&self) -> AgroResult<()> {
        info!("Connecting to RPLIDAR A3 on {}", self.config.lidar.serial_port);

        let mut port = tokio_serial::new(&self.config.lidar.serial_port, self.config.lidar.baud_rate)
            .open_native_async()
            .map_err(|e| shared::error::AgroError::Sensor(format!("Failed to open LiDAR port: {}", e)))?;

        let mut scan_interval = tokio::time::interval(
            std::time::Duration::from_secs_f32(1.0 / self.config.lidar.scan_frequency)
        );

        loop {
            scan_interval.tick().await;

            match self.read_scan(&mut port).await {
                Ok(scan) => {
                    self.save_scan(&scan).await?;
                    info!("Captured LiDAR scan with {} points", scan.points.len());
                }
                Err(e) => {
                    error!("Failed to read LiDAR scan: {}", e);
                }
            }
        }
    }

    async fn read_scan(&self, port: &mut tokio_serial::SerialStream) -> AgroResult<LidarScan> {
        use tokio::io::AsyncReadExt;

        // Simplified RPLIDAR A3 protocol implementation
        // In a real implementation, you'd implement the full protocol
        
        let mut buf = [0u8; 1024];
        let _n = port.read(&mut buf).await
            .map_err(|e| shared::error::AgroError::Sensor(format!("Failed to read from LiDAR: {}", e)))?;

        // Parse scan data (mock implementation)
        let mut points = Vec::new();
        let timestamp = chrono::Utc::now();

        // Generate mock scan points (360 degrees)
        for i in 0..360 {
            let angle = i as f32;
            let distance = 1000.0 + (angle * 0.01745).sin() * 500.0; // Mock distance
            let quality = 47; // Mock quality

            points.push(LidarPoint {
                timestamp,
                angle,
                distance,
                quality,
            });
        }

        Ok(LidarScan {
            timestamp,
            points,
            scan_id: uuid::Uuid::new_v4(),
        })
    }

    async fn save_scan(&self, scan: &LidarScan) -> AgroResult<()> {
        let filename = format!(
            "scan_{}_{}.json",
            scan.timestamp.format("%Y%m%d_%H%M%S"),
            scan.scan_id
        );
        let filepath = self.data_dir.join(filename);

        let json = serde_json::to_string_pretty(scan)?;
        tokio::fs::write(filepath, json).await?;

        Ok(())
    }
}

pub struct SimulatedLidarReader {
    config: Arc<AgroConfig>,
    data_dir: PathBuf,
}

impl SimulatedLidarReader {
    pub fn new(config: Arc<AgroConfig>, data_dir: PathBuf) -> Self {
        Self { config, data_dir }
    }

    pub async fn run(&self) -> AgroResult<()> {
        info!("Starting simulated LiDAR reader");

        let mut scan_interval = tokio::time::interval(
            std::time::Duration::from_secs_f32(1.0 / self.config.lidar.scan_frequency)
        );

        loop {
            scan_interval.tick().await;

            let scan = self.generate_simulated_scan();
            self.save_scan(&scan).await?;
            info!("Generated simulated LiDAR scan with {} points", scan.points.len());
        }
    }

    fn generate_simulated_scan(&self) -> LidarScan {
        let timestamp = chrono::Utc::now();
        let mut points = Vec::new();

        // Generate simulated scan points with some obstacles
        for i in 0..360 {
            let angle = i as f32;
            let mut distance = 2000.0; // Base distance

            // Add some simulated obstacles
            if (angle >= 45.0 && angle <= 75.0) || (angle >= 285.0 && angle <= 315.0) {
                distance = 800.0 + (angle * 0.01745).sin() * 200.0;
            }

            // Add noise
            distance += (rand::random::<f32>() - 0.5) * 100.0;

            points.push(LidarPoint {
                timestamp,
                angle,
                distance,
                quality: 47,
            });
        }

        LidarScan {
            timestamp,
            points,
            scan_id: uuid::Uuid::new_v4(),
        }
    }

    async fn save_scan(&self, scan: &LidarScan) -> AgroResult<()> {
        let filename = format!(
            "sim_scan_{}_{}.json",
            scan.timestamp.format("%Y%m%d_%H%M%S"),
            scan.scan_id
        );
        let filepath = self.data_dir.join(filename);

        let json = serde_json::to_string_pretty(scan)?;
        tokio::fs::write(filepath, json).await?;

        Ok(())
    }
}
