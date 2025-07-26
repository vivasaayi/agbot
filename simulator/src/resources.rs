use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use config::{Config, ConfigError, File};
use std::path::Path;

#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub websocket_url: String,
    pub grpc_endpoint: String,
    pub terrain_data_path: String,
    pub mission_data_path: String,
    pub enable_inspector: bool,
    pub camera: CameraConfig,
    pub rendering: RenderingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    pub initial_position: Vec3,
    pub movement_speed: f32,
    pub rotation_speed: f32,
    pub zoom_speed: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderingConfig {
    pub show_ndvi_overlay: bool,
    pub show_lidar_points: bool,
    pub show_sensor_data: bool,
    pub terrain_resolution: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            websocket_url: "ws://localhost:8080/ws".to_string(),
            grpc_endpoint: "http://localhost:50051".to_string(),
            terrain_data_path: "./data/terrain".to_string(),
            mission_data_path: "./missions".to_string(),
            enable_inspector: true,
            camera: CameraConfig {
                initial_position: Vec3::new(0.0, 50.0, 50.0),
                movement_speed: 10.0,
                rotation_speed: 2.0,
                zoom_speed: 5.0,
            },
            rendering: RenderingConfig {
                show_ndvi_overlay: true,
                show_lidar_points: true,
                show_sensor_data: true,
                terrain_resolution: 512,
            },
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = "visualizer_config.toml";
        
        let mut settings = Config::builder();
        
        // Add default values
        settings = settings.add_source(Config::try_from(&AppConfig::default())?);
        
        // Try to load from file if it exists
        if Path::new(config_path).exists() {
            settings = settings.add_source(File::with_name(config_path));
        }
        
        // Build and deserialize
        settings.build()?.try_deserialize()
    }
}

#[derive(Resource, Default, Debug)]
pub struct AppState {
    pub paused: bool,
    pub show_ui: bool,
    pub show_inspector: bool,
    pub current_time: f64,
    pub time_scale: f32,
    pub replay_mode: bool,
    pub connected: bool,
}

#[derive(Resource, Default, Debug)]
pub struct DroneRegistry {
    pub drones: Vec<Entity>,
}

#[derive(Resource, Default, Debug)]
pub struct TerrainData {
    pub heightmap: Option<Handle<Image>>,
    pub ndvi_overlay: Option<Handle<Image>>,
    pub loaded: bool,
}

#[derive(Resource, Default, Debug)]
pub struct MissionData {
    pub current_mission: Option<String>,
    pub waypoints: Vec<Vec3>,
    pub replay_data: Vec<TimestampedData>,
    pub replay_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampedData {
    pub timestamp: f64,
    pub drone_id: String,
    pub position: Vec3,
    pub rotation: Quat,
    pub telemetry: DroneTelemety,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneTelemety {
    pub battery_level: f32,
    pub altitude: f32,
    pub speed: f32,
    pub heading: f32,
    pub gps_fix: bool,
    pub sensor_data: Option<SensorData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorData {
    pub ndvi_value: f32,
    pub temperature: f32,
    pub humidity: f32,
    pub lidar_points: Vec<Vec3>,
}
