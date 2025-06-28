use serde::{Deserialize, Serialize};
use uuid::Uuid;
use geo::Point;
use crate::Waypoint;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightPath {
    pub id: Uuid,
    pub name: String,
    pub segments: Vec<PathSegment>,
    pub total_distance_m: f32,
    pub estimated_duration_seconds: u32,
    pub path_type: PathType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathSegment {
    pub start_waypoint_id: Uuid,
    pub end_waypoint_id: Uuid,
    pub distance_m: f32,
    pub bearing_degrees: f32,
    pub estimated_time_seconds: u32,
    pub altitude_profile: AltitudeProfile,
    pub speed_profile: SpeedProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PathType {
    Direct,
    Survey {
        pattern: SurveyPattern,
        overlap_percent: f32,
    },
    Search {
        search_type: SearchType,
    },
    Emergency,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SurveyPattern {
    Grid,
    Zigzag,
    Spiral,
    RandomWalk,
    AdaptiveSampling,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchType {
    AreaSearch,
    LineSearch,
    PatternSearch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AltitudeProfile {
    pub start_altitude_m: f32,
    pub end_altitude_m: f32,
    pub profile_type: AltitudeProfileType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AltitudeProfileType {
    Constant,
    Linear,
    Curved { control_points: Vec<Point<f64>> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedProfile {
    pub start_speed_ms: f32,
    pub end_speed_ms: f32,
    pub profile_type: SpeedProfileType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpeedProfileType {
    Constant,
    Linear,
    Accelerate,
    Decelerate,
}

impl FlightPath {
    pub fn new(name: String, path_type: PathType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            segments: Vec::new(),
            total_distance_m: 0.0,
            estimated_duration_seconds: 0,
            path_type,
        }
    }

    pub fn add_segment(&mut self, segment: PathSegment) {
        self.total_distance_m += segment.distance_m;
        self.estimated_duration_seconds += segment.estimated_time_seconds;
        self.segments.push(segment);
    }

    pub fn from_waypoints(name: String, waypoints: &[Waypoint], path_type: PathType) -> Self {
        let mut path = Self::new(name, path_type);
        
        for window in waypoints.windows(2) {
            let start = &window[0];
            let end = &window[1];
            
            let distance = calculate_distance(&start.position, &end.position);
            let bearing = calculate_bearing(&start.position, &end.position);
            
            // Simple time estimation based on average speed
            let avg_speed = 10.0; // m/s
            let time = (distance / avg_speed) as u32;
            
            let segment = PathSegment {
                start_waypoint_id: start.id,
                end_waypoint_id: end.id,
                distance_m: distance,
                bearing_degrees: bearing,
                estimated_time_seconds: time,
                altitude_profile: AltitudeProfile {
                    start_altitude_m: start.altitude_m,
                    end_altitude_m: end.altitude_m,
                    profile_type: AltitudeProfileType::Linear,
                },
                speed_profile: SpeedProfile {
                    start_speed_ms: start.speed_ms.unwrap_or(avg_speed),
                    end_speed_ms: end.speed_ms.unwrap_or(avg_speed),
                    profile_type: SpeedProfileType::Constant,
                },
            };
            
            path.add_segment(segment);
        }
        
        path
    }
}

fn calculate_distance(start: &Point<f64>, end: &Point<f64>) -> f32 {
    // Simple Euclidean distance - in real applications use haversine for lat/lon
    let dx = end.x() - start.x();
    let dy = end.y() - start.y();
    ((dx * dx + dy * dy).sqrt() * 111_320.0) as f32 // Convert degrees to meters approximately
}

fn calculate_bearing(start: &Point<f64>, end: &Point<f64>) -> f32 {
    let dx = end.x() - start.x();
    let dy = end.y() - start.y();
    dy.atan2(dx).to_degrees() as f32
}
