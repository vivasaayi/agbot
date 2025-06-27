use crate::{RuntimeMode, AgroResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgroConfig {
    pub runtime_mode: RuntimeMode,
    pub mavlink: MavlinkConfig,
    pub lidar: LidarConfig,
    pub camera: CameraConfig,
    pub storage: StorageConfig,
    pub server: ServerConfig,
    pub gps: GpsConfig,
    pub processing: ProcessingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MavlinkConfig {
    pub serial_port: String,
    pub baud_rate: u32,
    pub timeout_ms: u64,
    pub heartbeat_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidarConfig {
    pub serial_port: String,
    pub baud_rate: u32,
    pub timeout_ms: u64,
    pub scan_frequency: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    pub device: String,
    pub multispectral_bands: u8,
    pub capture_interval_ms: u64,
    pub exposure_time: f32,
    pub gain: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub data_root_path: PathBuf,
    pub mission_data_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub ws_bind_address: String,
    pub api_bind_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsConfig {
    pub home_latitude: f64,
    pub home_longitude: f64,
    pub home_altitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    pub ndvi_output_format: String,
    pub lidar_grid_resolution: f32,
}

impl AgroConfig {
    pub fn load() -> AgroResult<Self> {
        dotenvy::dotenv().ok();

        let runtime_mode = std::env::var("RUNTIME_MODE")
            .unwrap_or_else(|_| "SIMULATION".to_string())
            .parse()?;

        let config = AgroConfig {
            runtime_mode,
            mavlink: MavlinkConfig {
                serial_port: std::env::var("MAVLINK_SERIAL_PORT")
                    .unwrap_or_else(|_| "/dev/ttyUSB0".to_string()),
                baud_rate: std::env::var("MAVLINK_BAUD_RATE")
                    .unwrap_or_else(|_| "57600".to_string())
                    .parse()
                    .unwrap_or(57600),
                timeout_ms: 1000,
                heartbeat_interval_ms: 1000,
            },
            lidar: LidarConfig {
                serial_port: std::env::var("LIDAR_SERIAL_PORT")
                    .unwrap_or_else(|_| "/dev/ttyUSB1".to_string()),
                baud_rate: std::env::var("LIDAR_BAUD_RATE")
                    .unwrap_or_else(|_| "230400".to_string())
                    .parse()
                    .unwrap_or(230400),
                timeout_ms: 1000,
                scan_frequency: 10.0,
            },
            camera: CameraConfig {
                device: std::env::var("CAMERA_DEVICE")
                    .unwrap_or_else(|_| "/dev/video0".to_string()),
                multispectral_bands: std::env::var("MULTISPECTRAL_BANDS")
                    .unwrap_or_else(|_| "4".to_string())
                    .parse()
                    .unwrap_or(4),
                capture_interval_ms: 5000,
                exposure_time: 1.0 / 60.0,
                gain: 1.0,
            },
            storage: StorageConfig {
                data_root_path: std::env::var("DATA_ROOT_PATH")
                    .unwrap_or_else(|_| "/tmp/agrodrone/data".to_string())
                    .into(),
                mission_data_path: std::env::var("MISSION_DATA_PATH")
                    .unwrap_or_else(|_| "/tmp/agrodrone/missions".to_string())
                    .into(),
            },
            server: ServerConfig {
                ws_bind_address: std::env::var("WS_BIND_ADDRESS")
                    .unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
                api_bind_address: std::env::var("API_BIND_ADDRESS")
                    .unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            },
            gps: GpsConfig {
                home_latitude: std::env::var("HOME_LATITUDE")
                    .unwrap_or_else(|_| "37.7749".to_string())
                    .parse()
                    .unwrap_or(37.7749),
                home_longitude: std::env::var("HOME_LONGITUDE")
                    .unwrap_or_else(|_| "-122.4194".to_string())
                    .parse()
                    .unwrap_or(-122.4194),
                home_altitude: std::env::var("HOME_ALTITUDE")
                    .unwrap_or_else(|_| "100.0".to_string())
                    .parse()
                    .unwrap_or(100.0),
            },
            processing: ProcessingConfig {
                ndvi_output_format: std::env::var("NDVI_OUTPUT_FORMAT")
                    .unwrap_or_else(|_| "GEOTIFF".to_string()),
                lidar_grid_resolution: std::env::var("LIDAR_GRID_RESOLUTION")
                    .unwrap_or_else(|_| "0.1".to_string())
                    .parse()
                    .unwrap_or(0.1),
            },
        };

        Ok(config)
    }
}
