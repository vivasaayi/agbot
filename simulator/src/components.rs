use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Debug, Clone)]
pub struct Drone {
    pub id: String,
    pub drone_type: DroneType,
    pub status: DroneStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DroneType {
    Quadcopter,
    FixedWing,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DroneStatus {
    Idle,
    Flying,
    Mission,
    Returning,
    Landing,
    Error,
}

#[derive(Component, Debug)]
pub struct DroneModel;

#[derive(Component, Debug)]
pub struct DroneTelemetryDisplay;

#[derive(Component, Debug)]
pub struct DroneTrail {
    pub points: Vec<Vec3>,
    pub max_points: usize,
}

impl Default for DroneTrail {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            max_points: 100,
        }
    }
}

#[derive(Component, Debug)]
pub struct Waypoint {
    pub index: usize,
    pub reached: bool,
}

#[derive(Component, Debug)]
pub struct SensorOverlay {
    pub ndvi_value: f32,
    pub visible: bool,
}

#[derive(Component, Debug)]
pub struct LidarPointCloud {
    pub points: Vec<Vec3>,
    pub colors: Vec<Color>,
}

#[derive(Component, Debug)]
pub struct LidarSensor {
    pub range: f32,
    pub angular_resolution: f32,
    pub scan_frequency: f32,
    pub last_scan_time: f32,
    pub is_3d: bool,
    pub vertical_fov: f32,  // For 3D LiDAR
    pub vertical_resolution: f32,  // For 3D LiDAR
}

impl Default for LidarSensor {
    fn default() -> Self {
        Self {
            range: 100.0,  // 100 meter range
            angular_resolution: 1.0,  // 1 degree per ray
            scan_frequency: 10.0,  // 10 Hz
            last_scan_time: 0.0,
            is_3d: false,  // 2D by default
            vertical_fov: 30.0,  // +/- 15 degrees
            vertical_resolution: 2.0,  // 2 degrees vertical resolution
        }
    }
}

#[derive(Component, Debug)]
pub struct LidarScanData {
    pub points: Vec<LidarPoint>,
    pub timestamp: f32,
}

#[derive(Debug, Clone)]
pub struct LidarPoint {
    pub angle: f32,
    pub distance: f32,
    pub position: Vec3,
    pub quality: u8,
}

#[derive(Component, Debug)]
pub struct TerrainTile {
    pub x: i32,
    pub z: i32,
    pub loaded: bool,
}

#[derive(Component, Debug)]
pub struct CameraController {
    pub target: Vec3,
    pub distance: f32,
    pub follow_drone: Option<Entity>,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 50.0,
            follow_drone: None,
        }
    }
}

#[derive(Component, Debug)]
pub struct HudElement {
    pub element_type: HudElementType,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub enum HudElementType {
    Compass,
    Altimeter,
    SpeedIndicator,
    BatteryLevel,
    GpsStatus,
    MissionProgress,
}

#[derive(Component, Debug)]
pub struct Selectable;

#[derive(Component, Debug)]
pub struct Selected;

#[derive(Component, Debug)]
pub struct Highlighted;
