use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use geo::{Point, Polygon};

pub mod waypoint;
pub mod flight_path;
pub mod mission_optimizer;
pub mod weather_integration;
pub mod mavlink_integration;
pub mod websocket_handler;

pub use waypoint::{Waypoint, WaypointType, Action};
pub use flight_path::{FlightPath, PathSegment};
pub use mission_optimizer::MissionOptimizer;

/// Core mission planning structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub area_of_interest: Polygon<f64>,
    pub waypoints: Vec<Waypoint>,
    pub flight_paths: Vec<FlightPath>,
    pub estimated_duration_minutes: u32,
    pub estimated_battery_usage: f32,
    pub weather_constraints: WeatherConstraints,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherConstraints {
    pub max_wind_speed_ms: f32,
    pub max_precipitation_mm: f32,
    pub min_visibility_m: f32,
    pub temperature_range_celsius: (f32, f32),
}

impl Mission {
    pub fn new(name: String, description: String, area: Polygon<f64>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            created_at: now,
            updated_at: now,
            area_of_interest: area,
            waypoints: Vec::new(),
            flight_paths: Vec::new(),
            estimated_duration_minutes: 0,
            estimated_battery_usage: 0.0,
            weather_constraints: WeatherConstraints::default(),
            metadata: HashMap::new(),
        }
    }

    pub fn add_waypoint(&mut self, waypoint: Waypoint) {
        self.waypoints.push(waypoint);
        self.updated_at = Utc::now();
    }

    pub fn add_flight_path(&mut self, path: FlightPath) {
        self.flight_paths.push(path);
        self.updated_at = Utc::now();
    }

    pub fn optimize(&mut self) -> Result<()> {
        let optimizer = MissionOptimizer::new();
        let optimized = optimizer.optimize_mission(self)?;
        
        self.waypoints = optimized.waypoints;
        self.flight_paths = optimized.flight_paths;
        self.estimated_duration_minutes = optimized.estimated_duration_minutes;
        self.estimated_battery_usage = optimized.estimated_battery_usage;
        self.updated_at = Utc::now();
        
        Ok(())
    }
}

impl Default for WeatherConstraints {
    fn default() -> Self {
        Self {
            max_wind_speed_ms: 15.0,
            max_precipitation_mm: 2.0,
            min_visibility_m: 1000.0,
            temperature_range_celsius: (-10.0, 45.0),
        }
    }
}

/// Mission planning service
pub struct MissionPlannerService {
    missions: HashMap<Uuid, Mission>,
}

impl MissionPlannerService {
    pub fn new() -> Self {
        Self {
            missions: HashMap::new(),
        }
    }

    pub async fn create_mission(&mut self, mission: Mission) -> Result<Uuid> {
        let id = mission.id;
        self.missions.insert(id, mission);
        Ok(id)
    }

    pub async fn get_mission(&self, id: &Uuid) -> Option<&Mission> {
        self.missions.get(id)
    }

    pub async fn update_mission(&mut self, id: &Uuid, mission: Mission) -> Result<()> {
        if self.missions.contains_key(id) {
            self.missions.insert(*id, mission);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Mission not found"))
        }
    }

    pub async fn list_missions(&self) -> Vec<&Mission> {
        self.missions.values().collect()
    }

    pub async fn delete_mission(&mut self, id: &Uuid) -> Result<()> {
        if self.missions.remove(id).is_some() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Mission not found"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{coord, polygon};

    #[test]
    fn test_mission_creation() {
        let area = polygon![
            (x: 0.0, y: 0.0),
            (x: 1.0, y: 0.0),
            (x: 1.0, y: 1.0),
            (x: 0.0, y: 1.0),
            (x: 0.0, y: 0.0),
        ];
        
        let mission = Mission::new(
            "Test Mission".to_string(),
            "A test mission".to_string(),
            area,
        );
        
        assert_eq!(mission.name, "Test Mission");
        assert_eq!(mission.description, "A test mission");
        assert!(mission.waypoints.is_empty());
        assert!(mission.flight_paths.is_empty());
    }

    #[tokio::test]
    async fn test_mission_service() {
        let mut service = MissionPlannerService::new();
        
        let area = polygon![
            (x: 0.0, y: 0.0),
            (x: 1.0, y: 0.0),
            (x: 1.0, y: 1.0),
            (x: 0.0, y: 1.0),
            (x: 0.0, y: 0.0),
        ];
        
        let mission = Mission::new(
            "Test Mission".to_string(),
            "A test mission".to_string(),
            area,
        );
        
        let id = service.create_mission(mission).await.unwrap();
        let retrieved = service.get_mission(&id).await.unwrap();
        
        assert_eq!(retrieved.name, "Test Mission");
    }
}
