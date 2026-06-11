use chrono::{DateTime, Utc};
use geo::Point;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt};
use uuid::Uuid;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WaypointValidationConfig {
    pub min_leg_distance_m: f64,
    pub max_leg_distance_m: f64,
    pub max_altitude_step_m: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WaypointValidationCode {
    EmptyWaypointList,
    MissingTakeoff,
    MissingLanding,
    TakeoffNotFirst,
    LandingNotLast,
    DuplicateWaypointId,
    NonFiniteAltitude,
    NegativeAltitude,
    NonPositiveSpeed,
    ZeroLengthLeg,
    LegDistanceExceeded,
    AltitudeStepExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaypointValidationIssue {
    pub waypoint_index: Option<usize>,
    pub code: WaypointValidationCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaypointValidationError {
    pub issues: Vec<WaypointValidationIssue>,
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

impl Default for WaypointValidationConfig {
    fn default() -> Self {
        Self {
            min_leg_distance_m: 0.5,
            max_leg_distance_m: 5_000.0,
            max_altitude_step_m: 80.0,
        }
    }
}

impl WaypointValidationError {
    pub fn primary_code(&self) -> Option<WaypointValidationCode> {
        self.issues.first().map(|issue| issue.code)
    }
}

impl fmt::Display for WaypointValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(issue) = self.issues.first() {
            write!(formatter, "waypoint validation failed: {:?}", issue.code)
        } else {
            formatter.write_str("waypoint validation failed")
        }
    }
}

impl std::error::Error for WaypointValidationError {}

pub fn validate_waypoint_sanity(
    waypoints: &[Waypoint],
    config: WaypointValidationConfig,
) -> Result<(), WaypointValidationError> {
    let mut issues = Vec::new();
    if waypoints.is_empty() {
        issues.push(WaypointValidationIssue {
            waypoint_index: None,
            code: WaypointValidationCode::EmptyWaypointList,
            message: "mission must include at least one waypoint".to_string(),
        });
        return Err(WaypointValidationError { issues });
    }

    if !waypoints
        .iter()
        .any(|waypoint| waypoint.waypoint_type == WaypointType::Takeoff)
    {
        issues.push(WaypointValidationIssue {
            waypoint_index: None,
            code: WaypointValidationCode::MissingTakeoff,
            message: "mission must include a takeoff waypoint".to_string(),
        });
    } else if waypoints.first().map(|waypoint| &waypoint.waypoint_type)
        != Some(&WaypointType::Takeoff)
    {
        issues.push(WaypointValidationIssue {
            waypoint_index: Some(0),
            code: WaypointValidationCode::TakeoffNotFirst,
            message: "takeoff waypoint must be first".to_string(),
        });
    }

    if !waypoints
        .iter()
        .any(|waypoint| waypoint.waypoint_type == WaypointType::Landing)
    {
        issues.push(WaypointValidationIssue {
            waypoint_index: None,
            code: WaypointValidationCode::MissingLanding,
            message: "mission must include a landing waypoint".to_string(),
        });
    } else if waypoints.last().map(|waypoint| &waypoint.waypoint_type)
        != Some(&WaypointType::Landing)
    {
        issues.push(WaypointValidationIssue {
            waypoint_index: Some(waypoints.len() - 1),
            code: WaypointValidationCode::LandingNotLast,
            message: "landing waypoint must be last".to_string(),
        });
    }

    let mut seen_ids = HashSet::new();
    for (index, waypoint) in waypoints.iter().enumerate() {
        if !seen_ids.insert(waypoint.id) {
            issues.push(WaypointValidationIssue {
                waypoint_index: Some(index),
                code: WaypointValidationCode::DuplicateWaypointId,
                message: "waypoint id appears more than once".to_string(),
            });
        }
        if index > 0 && waypoint.waypoint_type == WaypointType::Takeoff {
            issues.push(WaypointValidationIssue {
                waypoint_index: Some(index),
                code: WaypointValidationCode::TakeoffNotFirst,
                message: "takeoff waypoint cannot appear after the first waypoint".to_string(),
            });
        }
        if index + 1 < waypoints.len() && waypoint.waypoint_type == WaypointType::Landing {
            issues.push(WaypointValidationIssue {
                waypoint_index: Some(index),
                code: WaypointValidationCode::LandingNotLast,
                message: "landing waypoint cannot appear before the last waypoint".to_string(),
            });
        }
        if !waypoint.altitude_m.is_finite() {
            issues.push(WaypointValidationIssue {
                waypoint_index: Some(index),
                code: WaypointValidationCode::NonFiniteAltitude,
                message: "waypoint altitude must be finite".to_string(),
            });
        } else if waypoint.altitude_m < 0.0 {
            issues.push(WaypointValidationIssue {
                waypoint_index: Some(index),
                code: WaypointValidationCode::NegativeAltitude,
                message: "waypoint altitude must be non-negative".to_string(),
            });
        }
        if let Some(speed_ms) = waypoint.speed_ms {
            if !speed_ms.is_finite() || speed_ms <= 0.0 {
                issues.push(WaypointValidationIssue {
                    waypoint_index: Some(index),
                    code: WaypointValidationCode::NonPositiveSpeed,
                    message: "waypoint speed must be positive and finite".to_string(),
                });
            }
        }
    }

    for (index, pair) in waypoints.windows(2).enumerate() {
        let from = &pair[0];
        let to = &pair[1];
        let dx = to.position.x() - from.position.x();
        let dy = to.position.y() - from.position.y();
        let horizontal_distance_m = (dx * dx + dy * dy).sqrt();
        let altitude_delta_m = (to.altitude_m - from.altitude_m).abs();
        let distance_3d_m =
            (horizontal_distance_m.powi(2) + f64::from(altitude_delta_m).powi(2)).sqrt();

        if distance_3d_m < config.min_leg_distance_m {
            issues.push(WaypointValidationIssue {
                waypoint_index: Some(index + 1),
                code: WaypointValidationCode::ZeroLengthLeg,
                message: "consecutive waypoints must not form a zero-length leg".to_string(),
            });
        }
        if horizontal_distance_m > config.max_leg_distance_m {
            issues.push(WaypointValidationIssue {
                waypoint_index: Some(index + 1),
                code: WaypointValidationCode::LegDistanceExceeded,
                message: format!(
                    "leg distance {:.1} m exceeds {:.1} m",
                    horizontal_distance_m, config.max_leg_distance_m
                ),
            });
        }
        if altitude_delta_m > config.max_altitude_step_m {
            issues.push(WaypointValidationIssue {
                waypoint_index: Some(index + 1),
                code: WaypointValidationCode::AltitudeStepExceeded,
                message: format!(
                    "altitude step {:.1} m exceeds {:.1} m",
                    altitude_delta_m, config.max_altitude_step_m
                ),
            });
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(WaypointValidationError { issues })
    }
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
