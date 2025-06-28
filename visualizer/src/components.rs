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
