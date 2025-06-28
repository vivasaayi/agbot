use std::collections::HashMap;
use uuid::Uuid;
use tokio::sync::{broadcast, mpsc, RwLock};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use anyhow::Result;

/// Drone swarm management and coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneSwarm {
    pub id: Uuid,
    pub name: String,
    pub drones: HashMap<Uuid, DroneInfo>,
    pub formation: FormationType,
    pub leader_id: Option<Uuid>,
    #[serde(skip, default = "default_broadcast_channel")]
    pub communication_channel: broadcast::Sender<SwarmMessage>,
    pub status: SwarmStatus,
}

fn default_broadcast_channel() -> broadcast::Sender<SwarmMessage> {
    let (sender, _) = broadcast::channel(100);
    sender
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneInfo {
    pub id: Uuid,
    pub name: String,
    pub model: String,
    pub capabilities: DroneCapabilities,
    pub position: (f64, f64, f32), // lat, lon, alt
    pub battery_level: f32,
    pub status: DroneStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub assigned_mission: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneCapabilities {
    pub max_speed: f32,
    pub max_altitude: f32,
    pub max_range_km: f32,
    pub payload_capacity_kg: f32,
    pub flight_time_minutes: u32,
    pub sensors: Vec<SensorType>,
    pub special_features: Vec<String>,
}

impl Default for DroneCapabilities {
    fn default() -> Self {
        Self {
            max_speed: 50.0,
            max_altitude: 1000.0,
            max_range_km: 10.0,
            payload_capacity_kg: 2.0,
            flight_time_minutes: 30,
            sensors: vec![SensorType::RGB],
            special_features: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SensorType {
    RGB,
    Multispectral,
    Thermal,
    Lidar,
    Radar,
    GPS,
    IMU,
    Magnetometer,
    Barometer,
    WeatherStation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DroneStatus {
    Idle,
    InMission,
    Returning,
    Charging,
    Maintenance,
    Error,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SwarmStatus {
    Inactive,
    Forming,
    Active,
    Dispersing,
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FormationType {
    Line,
    Grid,
    V,
    Circle,
    Custom(Vec<(f64, f64)>), // relative positions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwarmMessage {
    HeartBeat { drone_id: Uuid, timestamp: DateTime<Utc> },
    PositionUpdate { drone_id: Uuid, position: (f64, f64, f32) },
    MissionUpdate { drone_id: Uuid, mission_id: Uuid, status: String },
    Emergency { drone_id: Uuid, message: String },
    FormationChange { formation: FormationType },
    Command { target: CommandTarget, payload: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandTarget {
    All,
    Drone(Uuid),
    Group(Vec<Uuid>),
    Leader,
}

/// Swarm controller for managing multiple drones
pub struct SwarmController {
    swarms: Arc<RwLock<HashMap<Uuid, DroneSwarm>>>,
    message_router: mpsc::UnboundedSender<SwarmMessage>,
    coordination_interval: std::time::Duration,
    max_drones_per_swarm: usize,
}

impl SwarmController {
    pub fn new() -> Self {
        let (sender, _receiver) = mpsc::unbounded_channel();
        
        Self {
            swarms: Arc::new(RwLock::new(HashMap::new())),
            message_router: sender,
            coordination_interval: std::time::Duration::from_secs(1),
            max_drones_per_swarm: 50,
        }
    }

    pub async fn create_swarm(&self, name: String, formation: FormationType) -> Result<Uuid> {
        let swarm_id = Uuid::new_v4();
        let (tx, _rx) = broadcast::channel(1000);
        
        let swarm = DroneSwarm {
            id: swarm_id,
            name,
            drones: HashMap::new(),
            formation,
            leader_id: None,
            communication_channel: tx,
            status: SwarmStatus::Inactive,
        };

        let mut swarms = self.swarms.write().await;
        swarms.insert(swarm_id, swarm);
        
        tracing::info!("Created swarm: {}", swarm_id);
        Ok(swarm_id)
    }

    pub async fn add_drone_to_swarm(&self, swarm_id: Uuid, drone: DroneInfo) -> Result<()> {
        let mut swarms = self.swarms.write().await;
        let swarm = swarms.get_mut(&swarm_id)
            .ok_or_else(|| anyhow::anyhow!("Swarm not found"))?;

        if swarm.drones.len() >= self.max_drones_per_swarm {
            return Err(anyhow::anyhow!("Swarm at maximum capacity"));
        }

        // Auto-assign leader if first drone
        if swarm.drones.is_empty() {
            swarm.leader_id = Some(drone.id);
        }

        let drone_id = drone.id;
        swarm.drones.insert(drone.id, drone);
        tracing::info!("Added drone {} to swarm {}", drone_id, swarm_id);
        Ok(())
    }

    pub async fn remove_drone_from_swarm(&self, swarm_id: Uuid, drone_id: Uuid) -> Result<()> {
        let mut swarms = self.swarms.write().await;
        let swarm = swarms.get_mut(&swarm_id)
            .ok_or_else(|| anyhow::anyhow!("Swarm not found"))?;

        swarm.drones.remove(&drone_id);

        // Reassign leader if necessary
        if swarm.leader_id == Some(drone_id) {
            swarm.leader_id = swarm.drones.keys().next().copied();
        }

        tracing::info!("Removed drone {} from swarm {}", drone_id, swarm_id);
        Ok(())
    }

    pub async fn get_swarm_status(&self, swarm_id: Uuid) -> Result<SwarmStatus> {
        let swarms = self.swarms.read().await;
        let swarm = swarms.get(&swarm_id)
            .ok_or_else(|| anyhow::anyhow!("Swarm not found"))?;
        Ok(swarm.status.clone())
    }

    pub async fn list_swarms(&self) -> Vec<(Uuid, String, usize)> {
        let swarms = self.swarms.read().await;
        swarms.values()
            .map(|s| (s.id, s.name.clone(), s.drones.len()))
            .collect()
    }

    pub async fn broadcast_to_swarm(&self, swarm_id: Uuid, message: SwarmMessage) -> Result<()> {
        let swarms = self.swarms.read().await;
        let swarm = swarms.get(&swarm_id)
            .ok_or_else(|| anyhow::anyhow!("Swarm not found"))?;

        let _ = swarm.communication_channel.send(message);
        Ok(())
    }

    pub async fn get_swarm_health(&self, swarm_id: Uuid) -> Result<SwarmHealth> {
        let swarms = self.swarms.read().await;
        let swarm = swarms.get(&swarm_id)
            .ok_or_else(|| anyhow::anyhow!("Swarm not found"))?;

        let total_drones = swarm.drones.len();
        let active_drones = swarm.drones.values()
            .filter(|d| matches!(d.status, DroneStatus::InMission | DroneStatus::Idle))
            .count();
        
        let avg_battery = if total_drones > 0 {
            swarm.drones.values().map(|d| d.battery_level).sum::<f32>() / total_drones as f32
        } else {
            0.0
        };

        let formation_integrity = self.calculate_formation_integrity(swarm).await;

        Ok(SwarmHealth {
            total_drones,
            active_drones,
            average_battery_level: avg_battery,
            formation_integrity,
            communication_quality: 0.95, // TODO: Calculate from actual data
            last_update: Utc::now(),
        })
    }

    async fn calculate_formation_integrity(&self, _swarm: &DroneSwarm) -> f32 {
        // TODO: Calculate how well drones are maintaining formation
        0.9
    }

    pub async fn emergency_land_swarm(&self, swarm_id: Uuid) -> Result<()> {
        let message = SwarmMessage::Command {
            target: CommandTarget::All,
            payload: serde_json::json!({
                "command": "emergency_land",
                "priority": "critical"
            }),
        };

        self.broadcast_to_swarm(swarm_id, message).await?;
        
        let mut swarms = self.swarms.write().await;
        if let Some(swarm) = swarms.get_mut(&swarm_id) {
            swarm.status = SwarmStatus::Emergency;
        }

        tracing::warn!("Emergency land initiated for swarm {}", swarm_id);
        Ok(())
    }
}

impl DroneSwarm {
    pub fn new(name: String, drone_ids: Vec<Uuid>, formation: FormationType) -> Self {
        let (sender, _receiver) = broadcast::channel(100);
        let mut drones = HashMap::new();
        
        // Create DroneInfo entries for each drone ID
        for drone_id in drone_ids {
            let drone_info = DroneInfo {
                id: drone_id,
                name: format!("Drone-{}", drone_id),
                model: "Default".to_string(),
                capabilities: DroneCapabilities::default(),
                position: (0.0, 0.0, 0.0),
                battery_level: 100.0,
                status: DroneStatus::Idle,
                last_heartbeat: chrono::Utc::now(),
                assigned_mission: None,
            };
            drones.insert(drone_id, drone_info);
        }
        
        Self {
            id: Uuid::new_v4(),
            name,
            drones,
            formation,
            leader_id: None,
            communication_channel: sender,
            status: SwarmStatus::Inactive,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmHealth {
    pub total_drones: usize,
    pub active_drones: usize,
    pub average_battery_level: f32,
    pub formation_integrity: f32,
    pub communication_quality: f32,
    pub last_update: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_swarm_creation() {
        let controller = SwarmController::new();
        let swarm_id = controller.create_swarm(
            "Test Swarm".to_string(),
            FormationType::Grid
        ).await.unwrap();

        let swarms = controller.list_swarms().await;
        assert_eq!(swarms.len(), 1);
        assert_eq!(swarms[0].0, swarm_id);
        assert_eq!(swarms[0].1, "Test Swarm");
    }

    #[tokio::test]
    async fn test_drone_management() {
        let controller = SwarmController::new();
        let swarm_id = controller.create_swarm(
            "Test Swarm".to_string(),
            FormationType::Line
        ).await.unwrap();

        let drone = DroneInfo {
            id: Uuid::new_v4(),
            name: "Test Drone".to_string(),
            model: "TestModel".to_string(),
            capabilities: DroneCapabilities {
                max_speed: 15.0,
                max_altitude: 120.0,
                max_range_km: 10.0,
                payload_capacity_kg: 2.0,
                flight_time_minutes: 30,
                sensors: vec![SensorType::RGB, SensorType::GPS],
                special_features: vec!["obstacle_avoidance".to_string()],
            },
            position: (40.7128, -74.0060, 0.0),
            battery_level: 0.95,
            status: DroneStatus::Idle,
            last_heartbeat: Utc::now(),
            assigned_mission: None,
        };

        controller.add_drone_to_swarm(swarm_id, drone.clone()).await.unwrap();
        
        let health = controller.get_swarm_health(swarm_id).await.unwrap();
        assert_eq!(health.total_drones, 1);
        assert_eq!(health.active_drones, 1);
    }
}
