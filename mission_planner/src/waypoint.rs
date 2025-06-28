use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use geo::Point;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Waypoint {
    pub id: Uuid,
    pub position: Point<f64>,
    pub altitude_m: f32,
    pub waypoint_type: WaypointType,
    pub actions: Vec<Action>,
    pub arrival_time: Option<DateTime<Utc>>,
    pub speed_ms: Option<f32>,
    pub heading_degrees: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WaypointType {
    Takeoff,
    Navigation,
    DataCollection,
    Survey,
    Emergency,
    Landing,
    Hover,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    TakePhoto {
        camera_id: String,
        settings: CameraSettings,
    },
    StartVideo {
        camera_id: String,
        duration_seconds: u32,
    },
    StopVideo {
        camera_id: String,
    },
    CollectLidar {
        duration_seconds: u32,
        resolution: LidarResolution,
    },
    CollectMultispectral {
        bands: Vec<String>,
        exposure_settings: ExposureSettings,
    },
    Hover {
        duration_seconds: u32,
    },
    SetSpeed {
        speed_ms: f32,
    },
    Wait {
        duration_seconds: u32,
    },
    Custom {
        action_type: String,
        parameters: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraSettings {
    pub iso: u32,
    pub shutter_speed: String,
    pub aperture: String,
    pub white_balance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LidarResolution {
    Low,
    Medium,
    High,
    Ultra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureSettings {
    pub bands: Vec<MultispectralBand>,
    pub exposure_time_ms: u32,
    pub gain: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultispectralBand {
    pub name: String,
    pub wavelength_nm: u32,
    pub bandwidth_nm: u32,
}

impl Waypoint {
    pub fn new(position: Point<f64>, altitude: f32, waypoint_type: WaypointType) -> Self {
        Self {
            id: Uuid::new_v4(),
            position,
            altitude_m: altitude,
            waypoint_type,
            actions: Vec::new(),
            arrival_time: None,
            speed_ms: None,
            heading_degrees: None,
        }
    }

    pub fn with_action(mut self, action: Action) -> Self {
        self.actions.push(action);
        self
    }

    pub fn with_speed(mut self, speed_ms: f32) -> Self {
        self.speed_ms = Some(speed_ms);
        self
    }

    pub fn with_heading(mut self, heading_degrees: f32) -> Self {
        self.heading_degrees = Some(heading_degrees);
        self
    }

    pub fn add_action(&mut self, action: Action) {
        self.actions.push(action);
    }
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            iso: 100,
            shutter_speed: "1/60".to_string(),
            aperture: "f/2.8".to_string(),
            white_balance: "auto".to_string(),
        }
    }
}

impl Default for ExposureSettings {
    fn default() -> Self {
        Self {
            bands: vec![
                MultispectralBand {
                    name: "Red".to_string(),
                    wavelength_nm: 650,
                    bandwidth_nm: 50,
                },
                MultispectralBand {
                    name: "Green".to_string(),
                    wavelength_nm: 550,
                    bandwidth_nm: 50,
                },
                MultispectralBand {
                    name: "Blue".to_string(),
                    wavelength_nm: 450,
                    bandwidth_nm: 50,
                },
                MultispectralBand {
                    name: "NIR".to_string(),
                    wavelength_nm: 850,
                    bandwidth_nm: 50,
                },
            ],
            exposure_time_ms: 100,
            gain: 1.0,
        }
    }
}
