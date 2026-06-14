use crate::Formation;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use uuid::Uuid;

/// Drone swarm management and coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneSwarm {
    pub id: Uuid,
    pub name: String,
    #[serde(default = "default_owner_id")]
    pub owner_id: String,
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

fn default_owner_id() -> String {
    "unassigned".to_string()
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormationSlot {
    pub slot_index: usize,
    pub offset_m: (f64, f64, f32),
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum FormationGeometryError {
    #[error("minimum formation spacing must be finite and non-negative")]
    InvalidMinimumSpacing,
    #[error("formation parameter is invalid: {reason}")]
    InvalidFormationParameter { reason: String },
    #[error("grid formation has capacity {capacity} but {drone_count} drone slots were requested")]
    GridCapacity { capacity: usize, drone_count: usize },
    #[error("custom formation requires {expected} positions but got {actual}")]
    CustomSlotCount { expected: usize, actual: usize },
    #[error(
        "formation slots {left_slot} and {right_slot} are {observed_m:.1}m apart, below required {minimum_m:.1}m"
    )]
    SpacingBelowMinimum {
        left_slot: usize,
        right_slot: usize,
        observed_m: f64,
        minimum_m: f64,
    },
}

pub fn generate_formation_slots(
    formation: &Formation,
    drone_count: usize,
    minimum_spacing_m: f64,
) -> std::result::Result<Vec<FormationSlot>, FormationGeometryError> {
    if !minimum_spacing_m.is_finite() || minimum_spacing_m < 0.0 {
        return Err(FormationGeometryError::InvalidMinimumSpacing);
    }

    let offsets = match formation {
        Formation::Custom { positions } => {
            if positions.len() != drone_count {
                return Err(FormationGeometryError::CustomSlotCount {
                    expected: drone_count,
                    actual: positions.len(),
                });
            }
            positions
                .iter()
                .map(|(x, y, z)| (f64::from(*x), f64::from(*y), *z))
                .collect::<Vec<_>>()
        }
        Formation::Line {
            spacing_m,
            heading_deg,
        } => {
            let spacing = validate_spacing(*spacing_m, minimum_spacing_m, "line spacing")?;
            let heading = validate_degrees(*heading_deg, "line heading")?.to_radians();
            (0..drone_count)
                .map(|index| {
                    let offset = spacing * index as f64;
                    (offset * heading.sin(), offset * heading.cos(), 0.0)
                })
                .collect::<Vec<_>>()
        }
        Formation::Grid {
            rows,
            cols,
            spacing_m,
        } => {
            let rows = *rows as usize;
            let cols = *cols as usize;
            if rows == 0 || cols == 0 {
                return Err(FormationGeometryError::InvalidFormationParameter {
                    reason: "grid rows and columns must be greater than zero".to_string(),
                });
            }
            let capacity = rows * cols;
            if capacity < drone_count {
                return Err(FormationGeometryError::GridCapacity {
                    capacity,
                    drone_count,
                });
            }
            let spacing = validate_spacing(*spacing_m, minimum_spacing_m, "grid spacing")?;
            (0..drone_count)
                .map(|index| {
                    let row = index / cols;
                    let col = index % cols;
                    (spacing * col as f64, spacing * row as f64, 0.0)
                })
                .collect::<Vec<_>>()
        }
        Formation::Circle { radius_m, center } => {
            let radius = f64::from(*radius_m);
            if !radius.is_finite() || radius < 0.0 || !center.0.is_finite() || !center.1.is_finite()
            {
                return Err(FormationGeometryError::InvalidFormationParameter {
                    reason: "circle radius and center must be finite".to_string(),
                });
            }
            (0..drone_count)
                .map(|index| {
                    let angle = (index as f64 / drone_count.max(1) as f64) * std::f64::consts::TAU;
                    (
                        center.0 + radius * angle.cos(),
                        center.1 + radius * angle.sin(),
                        0.0,
                    )
                })
                .collect::<Vec<_>>()
        }
        Formation::VFormation {
            spacing_m,
            angle_deg,
        } => {
            let spacing = validate_spacing(*spacing_m, minimum_spacing_m, "v spacing")?;
            let angle = validate_degrees(*angle_deg, "v angle")?.to_radians();
            (0..drone_count)
                .map(|index| {
                    if index == 0 {
                        return (0.0, 0.0, 0.0);
                    }
                    let side = if index % 2 == 0 { -1.0 } else { 1.0 };
                    let rank = ((index + 1) / 2) as f64;
                    (
                        side * rank * spacing * angle.sin(),
                        rank * spacing * angle.cos(),
                        0.0,
                    )
                })
                .collect::<Vec<_>>()
        }
    };

    let slots = offsets
        .into_iter()
        .enumerate()
        .map(|(slot_index, offset_m)| FormationSlot {
            slot_index,
            offset_m,
        })
        .collect::<Vec<_>>();
    validate_slot_spacing(&slots, minimum_spacing_m)?;
    Ok(slots)
}

fn validate_spacing(
    spacing_m: f32,
    minimum_spacing_m: f64,
    label: &str,
) -> std::result::Result<f64, FormationGeometryError> {
    let spacing = f64::from(spacing_m);
    if !spacing.is_finite() || spacing < 0.0 {
        return Err(FormationGeometryError::InvalidFormationParameter {
            reason: format!("{label} must be finite and non-negative"),
        });
    }
    if spacing < minimum_spacing_m {
        return Err(FormationGeometryError::SpacingBelowMinimum {
            left_slot: 0,
            right_slot: 1,
            observed_m: spacing,
            minimum_m: minimum_spacing_m,
        });
    }
    Ok(spacing)
}

fn validate_degrees(degrees: f32, label: &str) -> std::result::Result<f64, FormationGeometryError> {
    let degrees = f64::from(degrees);
    if !degrees.is_finite() {
        return Err(FormationGeometryError::InvalidFormationParameter {
            reason: format!("{label} must be finite"),
        });
    }
    Ok(degrees)
}

fn validate_slot_spacing(
    slots: &[FormationSlot],
    minimum_spacing_m: f64,
) -> std::result::Result<(), FormationGeometryError> {
    for left_index in 0..slots.len() {
        for right_index in (left_index + 1)..slots.len() {
            let observed_m = slot_distance(slots[left_index].offset_m, slots[right_index].offset_m);
            if observed_m < minimum_spacing_m {
                return Err(FormationGeometryError::SpacingBelowMinimum {
                    left_slot: slots[left_index].slot_index,
                    right_slot: slots[right_index].slot_index,
                    observed_m,
                    minimum_m: minimum_spacing_m,
                });
            }
        }
    }
    Ok(())
}

fn slot_distance(left: (f64, f64, f32), right: (f64, f64, f32)) -> f64 {
    let altitude_delta = f64::from(left.2 - right.2);
    (left.0 - right.0)
        .hypot(left.1 - right.1)
        .hypot(altitude_delta)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwarmMessage {
    HeartBeat {
        drone_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    PositionUpdate {
        drone_id: Uuid,
        position: (f64, f64, f32),
    },
    MissionUpdate {
        drone_id: Uuid,
        mission_id: Uuid,
        status: String,
    },
    Emergency {
        drone_id: Uuid,
        message: String,
    },
    FormationChange {
        formation: FormationType,
    },
    Command {
        target: CommandTarget,
        payload: serde_json::Value,
    },
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
            owner_id: default_owner_id(),
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
        let swarm = swarms
            .get_mut(&swarm_id)
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
        let swarm = swarms
            .get_mut(&swarm_id)
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
        let swarm = swarms
            .get(&swarm_id)
            .ok_or_else(|| anyhow::anyhow!("Swarm not found"))?;
        Ok(swarm.status.clone())
    }

    pub async fn list_swarms(&self) -> Vec<(Uuid, String, usize)> {
        let swarms = self.swarms.read().await;
        swarms
            .values()
            .map(|s| (s.id, s.name.clone(), s.drones.len()))
            .collect()
    }

    pub async fn broadcast_to_swarm(&self, swarm_id: Uuid, message: SwarmMessage) -> Result<()> {
        let swarms = self.swarms.read().await;
        let swarm = swarms
            .get(&swarm_id)
            .ok_or_else(|| anyhow::anyhow!("Swarm not found"))?;

        let _ = swarm.communication_channel.send(message);
        Ok(())
    }

    pub async fn get_swarm_health(&self, swarm_id: Uuid) -> Result<SwarmHealth> {
        let swarms = self.swarms.read().await;
        let swarm = swarms
            .get(&swarm_id)
            .ok_or_else(|| anyhow::anyhow!("Swarm not found"))?;

        let total_drones = swarm.drones.len();
        let active_drones = swarm
            .drones
            .values()
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
            owner_id: default_owner_id(),
            drones,
            formation,
            leader_id: None,
            communication_channel: sender,
            status: SwarmStatus::Inactive,
        }
    }

    pub fn new_owned(
        name: String,
        drone_ids: Vec<Uuid>,
        formation: FormationType,
        owner_id: String,
    ) -> Self {
        let mut swarm = Self::new(name, drone_ids, formation);
        swarm.owner_id = owner_id;
        swarm
    }

    pub fn drone_ids(&self) -> Vec<Uuid> {
        let mut drone_ids: Vec<Uuid> = self.drones.keys().copied().collect();
        drone_ids.sort();
        drone_ids
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

    fn minimum_distance(slots: &[FormationSlot]) -> f64 {
        let mut minimum = f64::INFINITY;
        for left in 0..slots.len() {
            for right in (left + 1)..slots.len() {
                minimum = minimum.min(slot_distance(slots[left].offset_m, slots[right].offset_m));
            }
        }
        minimum
    }

    #[test]
    fn grid_formation_generates_non_overlapping_slots_at_minimum_spacing() {
        let slots = generate_formation_slots(
            &Formation::Grid {
                rows: 2,
                cols: 2,
                spacing_m: 25.0,
            },
            4,
            25.0,
        )
        .expect("grid geometry should generate");

        assert_eq!(slots.len(), 4);
        assert_eq!(slots[0].offset_m, (0.0, 0.0, 0.0));
        assert_eq!(slots[1].offset_m, (25.0, 0.0, 0.0));
        assert_eq!(slots[2].offset_m, (0.0, 25.0, 0.0));
        assert!(minimum_distance(&slots) >= 25.0);
    }

    #[test]
    fn formation_spacing_below_minimum_is_rejected() {
        let err = generate_formation_slots(
            &Formation::Line {
                spacing_m: 10.0,
                heading_deg: 0.0,
            },
            2,
            25.0,
        )
        .expect_err("unsafe spacing should reject");

        assert!(matches!(
            err,
            FormationGeometryError::SpacingBelowMinimum {
                observed_m,
                minimum_m,
                ..
            } if observed_m == 10.0 && minimum_m == 25.0
        ));
    }

    #[test]
    fn formation_generator_covers_line_circle_v_and_custom_slots() {
        let line = generate_formation_slots(
            &Formation::Line {
                spacing_m: 20.0,
                heading_deg: 90.0,
            },
            3,
            10.0,
        )
        .expect("line slots should generate");
        assert_eq!(line.len(), 3);
        assert!((line[2].offset_m.0 - 40.0).abs() < 0.001);

        let circle = generate_formation_slots(
            &Formation::Circle {
                radius_m: 20.0,
                center: (5.0, -5.0),
            },
            4,
            10.0,
        )
        .expect("circle slots should generate");
        assert_eq!(circle.len(), 4);
        assert_eq!(circle[0].offset_m, (25.0, -5.0, 0.0));

        let v = generate_formation_slots(
            &Formation::VFormation {
                spacing_m: 20.0,
                angle_deg: 45.0,
            },
            3,
            10.0,
        )
        .expect("v slots should generate");
        assert_eq!(v[0].offset_m, (0.0, 0.0, 0.0));
        assert!(v[1].offset_m.0 > 0.0);
        assert!(v[2].offset_m.0 < 0.0);

        let custom = generate_formation_slots(
            &Formation::Custom {
                positions: vec![(0.0, 0.0, 20.0), (30.0, 0.0, 20.0)],
            },
            2,
            25.0,
        )
        .expect("custom slots should generate");
        assert_eq!(custom[1].offset_m, (30.0, 0.0, 20.0));
    }

    #[tokio::test]
    async fn test_swarm_creation() {
        let controller = SwarmController::new();
        let swarm_id = controller
            .create_swarm("Test Swarm".to_string(), FormationType::Grid)
            .await
            .unwrap();

        let swarms = controller.list_swarms().await;
        assert_eq!(swarms.len(), 1);
        assert_eq!(swarms[0].0, swarm_id);
        assert_eq!(swarms[0].1, "Test Swarm");
    }

    #[tokio::test]
    async fn test_drone_management() {
        let controller = SwarmController::new();
        let swarm_id = controller
            .create_swarm("Test Swarm".to_string(), FormationType::Line)
            .await
            .unwrap();

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

        controller
            .add_drone_to_swarm(swarm_id, drone.clone())
            .await
            .unwrap();

        let health = controller.get_swarm_health(swarm_id).await.unwrap();
        assert_eq!(health.total_drones, 1);
        assert_eq!(health.active_drones, 1);
    }
}
