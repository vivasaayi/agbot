use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use nalgebra::Vector3;

/// Common coordinate system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoCoordinate {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f32,
}

impl GeoCoordinate {
    pub fn new(lat: f64, lon: f64, alt: f32) -> Self {
        Self {
            latitude: lat,
            longitude: lon,
            altitude_m: alt,
        }
    }

    pub fn distance_to(&self, other: &GeoCoordinate) -> f32 {
        // Haversine formula for distance calculation
        let r = 6371000.0; // Earth radius in meters
        let lat1_rad = self.latitude.to_radians();
        let lat2_rad = other.latitude.to_radians();
        let delta_lat = (other.latitude - self.latitude).to_radians();
        let delta_lon = (other.longitude - self.longitude).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        (r * c) as f32
    }
}

/// Common telemetry data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Telemetry {
    pub drone_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub position: GeoCoordinate,
    pub velocity: Vector3<f32>,
    pub orientation: Vector3<f32>, // Roll, Pitch, Yaw in radians
    pub battery_level: f32,        // 0.0 to 1.0
    pub signal_strength: f32,      // 0.0 to 1.0
    pub system_status: SystemStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemStatus {
    Idle,
    Armed,
    Flying,
    Hovering,
    Landing,
    Emergency,
    Maintenance,
}

/// Mission waypoint structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Waypoint {
    pub id: Uuid,
    pub position: GeoCoordinate,
    pub actions: Vec<WaypointAction>,
    pub arrival_conditions: Option<ArrivalConditions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WaypointAction {
    TakePhoto,
    StartVideo { duration_seconds: u32 },
    CollectLidar { duration_seconds: u32 },
    CollectMultispectral,
    Hover { duration_seconds: u32 },
    Wait { duration_seconds: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrivalConditions {
    pub acceptable_radius_m: f32,
    pub max_speed_ms: f32,
    pub heading_tolerance_deg: f32,
}

/// Mission definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    pub id: Uuid,
    pub name: String,
    pub waypoints: Vec<Waypoint>,
    pub flight_parameters: FlightParameters,
    pub safety_constraints: SafetyConstraints,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightParameters {
    pub max_speed_ms: f32,
    pub cruise_altitude_m: f32,
    pub takeoff_altitude_m: f32,
    pub return_to_home_altitude_m: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConstraints {
    pub max_wind_speed_ms: f32,
    pub min_battery_level: f32,
    pub geofence_boundaries: Vec<GeoCoordinate>,
    pub no_fly_zones: Vec<NoFlyZone>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoFlyZone {
    pub id: Uuid,
    pub name: String,
    pub boundary: Vec<GeoCoordinate>,
    pub altitude_restriction: Option<(f32, f32)>,
    pub active: bool,
}

/// Sensor data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    pub sensor_id: String,
    pub timestamp: DateTime<Utc>,
    pub position: GeoCoordinate,
    pub data: SensorData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensorData {
    GPS {
        hdop: f32,
        satellites: u8,
        fix_type: String,
    },
    IMU {
        acceleration: Vector3<f32>,
        angular_velocity: Vector3<f32>,
        magnetic_field: Vector3<f32>,
    },
    Camera {
        image_id: Uuid,
        resolution: (u32, u32),
        format: String,
    },
    LiDAR {
        point_count: u32,
        scan_id: Uuid,
        range_m: f32,
    },
    Multispectral {
        bands: std::collections::HashMap<String, f32>,
        reflectance_values: std::collections::HashMap<String, f32>,
    },
    Weather {
        temperature_celsius: f32,
        humidity_percent: f32,
        wind_speed_ms: f32,
        wind_direction_deg: f32,
    },
}

// Define Position as an alias for GeoCoordinate for backward compatibility
pub type Position = GeoCoordinate;
pub type Position3D = GeoCoordinate;

/// Event types for inter-service communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgroEvent {
    DroneStatusChanged {
        drone_id: Uuid,
        old_status: SystemStatus,
        new_status: SystemStatus,
        timestamp: DateTime<Utc>,
    },
    MissionStarted {
        mission_id: Uuid,
        drone_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    MissionCompleted {
        mission_id: Uuid,
        drone_id: Uuid,
        success: bool,
        timestamp: DateTime<Utc>,
    },
    SensorDataReceived {
        drone_id: Uuid,
        sensor_reading: SensorReading,
    },
    EmergencyAlert {
        drone_id: Uuid,
        alert_type: String,
        description: String,
        position: GeoCoordinate,
        timestamp: DateTime<Utc>,
    },
    WeatherUpdate {
        conditions: WeatherConditions,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherConditions {
    pub temperature_celsius: f32,
    pub wind_speed_ms: f32,
    pub wind_direction_deg: f32,
    pub visibility_m: f32,
    pub precipitation_mm: f32,
    pub pressure_hpa: f32,
}

/// Configuration structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgroConfig {
    pub simulation_mode: bool,
    pub data_directory: String,
    pub log_level: String,
    pub api_port: u16,
    pub websocket_port: u16,
    pub max_concurrent_drones: u32,
    pub default_flight_altitude_m: f32,
    pub emergency_landing_battery_level: f32,
}

impl Default for AgroConfig {
    fn default() -> Self {
        Self {
            simulation_mode: true,
            data_directory: "./data".to_string(),
            log_level: "info".to_string(),
            api_port: 8080,
            websocket_port: 8081,
            max_concurrent_drones: 10,
            default_flight_altitude_m: 50.0,
            emergency_landing_battery_level: 0.15,
        }
    }
}

/// API response structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: Utc::now(),
        }
    }
}

/// Pagination for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
    pub total_items: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub pagination: Pagination,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebSocketMessage {
    TelemetryUpdate(Telemetry),
    MissionUpdate {
        mission_id: Uuid,
        status: String,
        progress_percent: f32,
    },
    SensorData(SensorReading),
    Alert {
        level: AlertLevel,
        message: String,
        drone_id: Option<Uuid>,
    },
    SystemStatus {
        active_drones: u32,
        active_missions: u32,
        system_health: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

/// Processing status for long-running operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingStatus {
    Queued,
    InProgress { progress_percent: f32 },
    Completed,
    Failed { error: String },
    Cancelled,
}

/// File metadata for data management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub id: Uuid,
    pub file_name: String,
    pub file_path: String,
    pub file_size_bytes: u64,
    pub mime_type: String,
    pub checksum: String,
    pub created_at: DateTime<Utc>,
    pub associated_drone_id: Option<Uuid>,
    pub associated_mission_id: Option<Uuid>,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub cpu_usage_percent: f32,
    pub memory_usage_mb: u64,
    pub disk_usage_gb: f64,
    pub network_throughput_mbps: f32,
    pub active_connections: u32,
    pub processed_messages_per_second: f32,
    pub timestamp: DateTime<Utc>,
}

/// Validation helpers
pub trait Validate {
    fn validate(&self) -> Result<(), String>;
}

impl Validate for GeoCoordinate {
    fn validate(&self) -> Result<(), String> {
        if self.latitude < -90.0 || self.latitude > 90.0 {
            return Err("Latitude must be between -90 and 90 degrees".to_string());
        }
        if self.longitude < -180.0 || self.longitude > 180.0 {
            return Err("Longitude must be between -180 and 180 degrees".to_string());
        }
        if self.altitude_m < -500.0 || self.altitude_m > 10000.0 {
            return Err("Altitude must be between -500 and 10000 meters".to_string());
        }
        Ok(())
    }
}

impl Validate for Mission {
    fn validate(&self) -> Result<(), String> {
        if self.waypoints.is_empty() {
            return Err("Mission must have at least one waypoint".to_string());
        }
        
        for waypoint in &self.waypoints {
            waypoint.position.validate()?;
        }
        
        if self.flight_parameters.max_speed_ms <= 0.0 || self.flight_parameters.max_speed_ms > 50.0 {
            return Err("Max speed must be between 0 and 50 m/s".to_string());
        }
        
        Ok(())
    }
}

/// Utility functions
pub mod utils {
    use super::*;

    pub fn generate_mission_id() -> Uuid {
        Uuid::new_v4()
    }

    pub fn format_duration(seconds: f32) -> String {
        let hours = (seconds / 3600.0) as u32;
        let minutes = ((seconds % 3600.0) / 60.0) as u32;
        let secs = (seconds % 60.0) as u32;
        
        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, secs)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, secs)
        } else {
            format!("{}s", secs)
        }
    }

    pub fn format_distance(meters: f32) -> String {
        if meters >= 1000.0 {
            format!("{:.2} km", meters / 1000.0)
        } else {
            format!("{:.1} m", meters)
        }
    }

    pub fn calculate_bearing(from: &GeoCoordinate, to: &GeoCoordinate) -> f32 {
        let lat1 = from.latitude.to_radians();
        let lat2 = to.latitude.to_radians();
        let delta_lon = (to.longitude - from.longitude).to_radians();

        let y = delta_lon.sin() * lat2.cos();
        let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * delta_lon.cos();

        let bearing_rad = y.atan2(x);
        let bearing_deg = bearing_rad.to_degrees();

        ((bearing_deg + 360.0) % 360.0) as f32
    }
}
