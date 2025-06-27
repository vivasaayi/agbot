use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// GPS coordinates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsCoords {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
}

/// Telemetry data from flight controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Telemetry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub position: GpsCoords,
    pub battery_voltage: f32,
    pub battery_percentage: u8,
    pub armed: bool,
    pub mode: String,
    pub ground_speed: f32,
    pub air_speed: f32,
    pub heading: f32,
    pub altitude_relative: f32,
}

/// Mission waypoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Waypoint {
    pub sequence: u16,
    pub position: GpsCoords,
    pub command: u16,
    pub auto_continue: bool,
    pub param1: f32,
    pub param2: f32,
    pub param3: f32,
    pub param4: f32,
}

/// Complete mission definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    pub id: uuid::Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub waypoints: Vec<Waypoint>,
    pub home_position: GpsCoords,
}

/// LiDAR scan point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidarPoint {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub angle: f32,
    pub distance: f32,
    pub quality: u8,
}

/// LiDAR scan containing multiple points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidarScan {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub points: Vec<LidarPoint>,
    pub scan_id: uuid::Uuid,
}

/// Multispectral image metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub gps_position: Option<GpsCoords>,
    pub bands: Vec<String>,
    pub exposure_time: f32,
    pub gain: f32,
    pub width: u32,
    pub height: u32,
}

/// Captured multispectral image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultispectralImage {
    pub metadata: ImageMetadata,
    pub file_paths: HashMap<String, String>, // band_name -> file_path
    pub image_id: uuid::Uuid,
}

/// NDVI processing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NdviResult {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source_images: Vec<uuid::Uuid>,
    pub output_path: String,
    pub min_ndvi: f32,
    pub max_ndvi: f32,
    pub mean_ndvi: f32,
    pub vegetation_percentage: f32,
}

/// WebSocket message types for ground station communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    Telemetry { data: Telemetry },
    MissionStatus { mission_id: uuid::Uuid, status: String },
    LidarUpdate { scan: LidarScan },
    ImageCaptured { image: MultispectralImage },
    NdviProcessed { result: NdviResult },
    SystemStatus { status: String, message: String },
}
