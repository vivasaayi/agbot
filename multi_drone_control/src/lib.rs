use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

pub mod collision_avoidance;
pub mod coordination;
pub mod mission_assignment;
pub mod swarm;

pub use collision_avoidance::{AvoidanceManeuver, CollisionAvoidanceSystem};
pub use coordination::{CoordinationEngine, CoordinationStatus};
pub use mission_assignment::{DroneAssignment, MissionAssignmentEngine};
pub use swarm::{DroneSwarm, SwarmController, SwarmStatus};

/// Multi-drone control system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiDroneController {
    pub id: Uuid,
    pub name: String,
    pub swarms: HashMap<Uuid, DroneSwarm>,
    pub global_constraints: GlobalConstraints,
    pub communication_range_m: f32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SwarmRegistryEntry {
    pub swarm_id: Uuid,
    pub drone_ids: Vec<Uuid>,
    pub owner_id: String,
    pub status: SwarmStatus,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SwarmRegistryError {
    #[error("swarm already exists: {swarm_id}")]
    SwarmAlreadyExists { swarm_id: Uuid },
    #[error("swarm not found: {swarm_id}")]
    SwarmNotFound { swarm_id: Uuid },
    #[error(
        "drone {drone_id} is already in active swarm {existing_swarm_id}; requested swarm {requested_swarm_id}"
    )]
    ActiveDroneMembershipConflict {
        drone_id: Uuid,
        existing_swarm_id: Uuid,
        requested_swarm_id: Uuid,
    },
    #[error("invalid swarm transition for {swarm_id}: {from:?} -> {to:?}")]
    InvalidStatusTransition {
        swarm_id: Uuid,
        from: SwarmStatus,
        to: SwarmStatus,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConstraints {
    pub max_altitude_m: f32,
    pub geofence_boundaries: Vec<(f64, f64)>,
    pub no_fly_zones: Vec<NoFlyZone>,
    pub max_concurrent_drones: u32,
    pub emergency_landing_sites: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoFlyZone {
    pub id: Uuid,
    pub name: String,
    pub boundary: Vec<(f64, f64)>,
    pub altitude_restriction: Option<(f32, f32)>,
    pub reason: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneStatus {
    pub id: Uuid,
    pub position: (f64, f64, f32),
    pub velocity: (f32, f32, f32),
    pub battery_level: f32,
    pub status: String,
    pub assigned_mission: Option<Uuid>,
    pub last_update: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlCommand {
    AssignMission {
        drone_id: Uuid,
        mission_id: Uuid,
    },
    FormSwarm {
        drone_ids: Vec<Uuid>,
        formation: Formation,
    },
    ExecuteCoordinatedAction {
        swarm_id: Uuid,
        action: CoordinatedAction,
    },
    EmergencyLand {
        drone_ids: Vec<Uuid>,
    },
    ReturnToBase {
        drone_ids: Vec<Uuid>,
    },
    UpdateConstraints {
        constraints: GlobalConstraints,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Formation {
    Line {
        spacing_m: f32,
        heading_deg: f32,
    },
    Grid {
        rows: u32,
        cols: u32,
        spacing_m: f32,
    },
    Circle {
        radius_m: f32,
        center: (f64, f64),
    },
    VFormation {
        spacing_m: f32,
        angle_deg: f32,
    },
    Custom {
        positions: Vec<(f32, f32, f32)>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoordinatedAction {
    SynchronizedSurvey {
        area: Vec<(f64, f64)>,
        overlap_percent: f32,
    },
    PatternSearch {
        search_type: String,
        area: Vec<(f64, f64)>,
    },
    CoverageOptimization {
        target_coverage: f32,
    },
    DataCollection {
        collection_points: Vec<(f64, f64, f32)>,
    },
}

/// Main control service
pub struct MultiDroneControlService {
    controller: Arc<RwLock<MultiDroneController>>,
    drone_statuses: Arc<RwLock<HashMap<Uuid, DroneStatus>>>,
    coordination_engine: Arc<RwLock<CoordinationEngine>>,
    mission_assigner: Arc<RwLock<MissionAssignmentEngine>>,
    collision_avoidance: Arc<RwLock<CollisionAvoidanceSystem>>,
    command_sender: mpsc::UnboundedSender<ControlCommand>,
    command_receiver: Arc<RwLock<mpsc::UnboundedReceiver<ControlCommand>>>,
}

impl MultiDroneController {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            swarms: HashMap::new(),
            global_constraints: GlobalConstraints::default(),
            communication_range_m: 1000.0,
            created_at: Utc::now(),
        }
    }

    pub fn register_swarm(
        &mut self,
        swarm: DroneSwarm,
    ) -> std::result::Result<(), SwarmRegistryError> {
        let swarm_id = swarm.id;

        if self.swarms.contains_key(&swarm_id) {
            return Err(SwarmRegistryError::SwarmAlreadyExists { swarm_id });
        }

        if Self::status_participates_in_active_membership(swarm.status) {
            let drone_ids = swarm.drone_ids();
            if let Some((drone_id, existing_swarm_id)) =
                self.active_membership_conflict(swarm_id, &drone_ids)
            {
                return Err(SwarmRegistryError::ActiveDroneMembershipConflict {
                    drone_id,
                    existing_swarm_id,
                    requested_swarm_id: swarm_id,
                });
            }
        }

        self.swarms.insert(swarm_id, swarm);
        Ok(())
    }

    pub fn add_swarm(&mut self, swarm: DroneSwarm) -> std::result::Result<(), SwarmRegistryError> {
        self.register_swarm(swarm)
    }

    pub fn remove_swarm(
        &mut self,
        swarm_id: &Uuid,
    ) -> std::result::Result<DroneSwarm, SwarmRegistryError> {
        self.swarms
            .remove(swarm_id)
            .ok_or(SwarmRegistryError::SwarmNotFound {
                swarm_id: *swarm_id,
            })
    }

    pub fn get_swarm(&self, swarm_id: &Uuid) -> Option<&DroneSwarm> {
        self.swarms.get(swarm_id)
    }

    pub fn list_swarm_registry(&self) -> Vec<SwarmRegistryEntry> {
        let mut entries: Vec<SwarmRegistryEntry> = self
            .swarms
            .values()
            .map(|swarm| SwarmRegistryEntry {
                swarm_id: swarm.id,
                drone_ids: swarm.drone_ids(),
                owner_id: swarm.owner_id.clone(),
                status: swarm.status,
            })
            .collect();
        entries.sort_by_key(|entry| entry.swarm_id);
        entries
    }

    pub fn transition_swarm_status(
        &mut self,
        swarm_id: Uuid,
        next_status: SwarmStatus,
    ) -> std::result::Result<SwarmStatus, SwarmRegistryError> {
        let current_status = self
            .swarms
            .get(&swarm_id)
            .ok_or(SwarmRegistryError::SwarmNotFound { swarm_id })?
            .status;

        if current_status == next_status {
            return Ok(next_status);
        }

        if !Self::is_valid_swarm_transition(current_status, next_status) {
            return Err(SwarmRegistryError::InvalidStatusTransition {
                swarm_id,
                from: current_status,
                to: next_status,
            });
        }

        if Self::status_participates_in_active_membership(next_status) {
            let drone_ids = self
                .swarms
                .get(&swarm_id)
                .ok_or(SwarmRegistryError::SwarmNotFound { swarm_id })?
                .drone_ids();
            if let Some((drone_id, existing_swarm_id)) =
                self.active_membership_conflict(swarm_id, &drone_ids)
            {
                return Err(SwarmRegistryError::ActiveDroneMembershipConflict {
                    drone_id,
                    existing_swarm_id,
                    requested_swarm_id: swarm_id,
                });
            }
        }

        let swarm = self
            .swarms
            .get_mut(&swarm_id)
            .ok_or(SwarmRegistryError::SwarmNotFound { swarm_id })?;
        swarm.status = next_status;
        Ok(next_status)
    }

    pub fn list_all_drones(&self) -> Vec<Uuid> {
        let mut drone_ids: Vec<Uuid> = self
            .swarms
            .values()
            .flat_map(|swarm| swarm.drones.keys())
            .cloned()
            .collect();
        drone_ids.sort();
        drone_ids
    }

    pub fn update_constraints(&mut self, constraints: GlobalConstraints) {
        self.global_constraints = constraints;
    }

    fn active_membership_conflict(
        &self,
        requested_swarm_id: Uuid,
        requested_drone_ids: &[Uuid],
    ) -> Option<(Uuid, Uuid)> {
        let mut existing_swarms: Vec<(&Uuid, &DroneSwarm)> = self
            .swarms
            .iter()
            .filter(|(swarm_id, swarm)| {
                **swarm_id != requested_swarm_id
                    && Self::status_participates_in_active_membership(swarm.status)
            })
            .collect();
        existing_swarms.sort_by_key(|(swarm_id, _)| **swarm_id);

        let mut requested_ids = requested_drone_ids.to_vec();
        requested_ids.sort();

        for (existing_swarm_id, existing_swarm) in existing_swarms {
            for drone_id in &requested_ids {
                if existing_swarm.drones.contains_key(drone_id) {
                    return Some((*drone_id, *existing_swarm_id));
                }
            }
        }

        None
    }

    fn status_participates_in_active_membership(status: SwarmStatus) -> bool {
        matches!(
            status,
            SwarmStatus::Forming
                | SwarmStatus::Active
                | SwarmStatus::Dispersing
                | SwarmStatus::Emergency
        )
    }

    fn is_valid_swarm_transition(from: SwarmStatus, to: SwarmStatus) -> bool {
        matches!(
            (from, to),
            (SwarmStatus::Inactive, SwarmStatus::Forming)
                | (SwarmStatus::Forming, SwarmStatus::Active)
                | (SwarmStatus::Forming, SwarmStatus::Inactive)
                | (SwarmStatus::Forming, SwarmStatus::Emergency)
                | (SwarmStatus::Active, SwarmStatus::Dispersing)
                | (SwarmStatus::Active, SwarmStatus::Emergency)
                | (SwarmStatus::Dispersing, SwarmStatus::Inactive)
                | (SwarmStatus::Dispersing, SwarmStatus::Emergency)
                | (SwarmStatus::Emergency, SwarmStatus::Dispersing)
                | (SwarmStatus::Emergency, SwarmStatus::Inactive)
        )
    }
}

impl Default for GlobalConstraints {
    fn default() -> Self {
        Self {
            max_altitude_m: 400.0,
            geofence_boundaries: vec![
                (-1000.0, -1000.0),
                (1000.0, -1000.0),
                (1000.0, 1000.0),
                (-1000.0, 1000.0),
            ],
            no_fly_zones: Vec::new(),
            max_concurrent_drones: 10,
            emergency_landing_sites: vec![(0.0, 0.0), (500.0, 500.0), (-500.0, -500.0)],
        }
    }
}

impl MultiDroneControlService {
    pub fn new(controller_name: String) -> Self {
        let controller = MultiDroneController::new(controller_name);
        let (command_sender, command_receiver) = mpsc::unbounded_channel();

        Self {
            controller: Arc::new(RwLock::new(controller)),
            drone_statuses: Arc::new(RwLock::new(HashMap::new())),
            coordination_engine: Arc::new(RwLock::new(CoordinationEngine::new())),
            mission_assigner: Arc::new(RwLock::new(MissionAssignmentEngine::new(
                mission_assignment::AssignmentAlgorithm::FirstAvailable,
            ))),
            collision_avoidance: Arc::new(RwLock::new(CollisionAvoidanceSystem::new())),
            command_sender,
            command_receiver: Arc::new(RwLock::new(command_receiver)),
        }
    }

    pub async fn send_command(&self, command: ControlCommand) -> Result<()> {
        self.command_sender
            .send(command)
            .map_err(|e| anyhow::anyhow!("Failed to send command: {}", e))?;
        Ok(())
    }

    pub async fn update_drone_status(&self, status: DroneStatus) {
        let mut statuses = self.drone_statuses.write().await;
        statuses.insert(status.id, status);
    }

    pub async fn get_drone_status(&self, drone_id: &Uuid) -> Option<DroneStatus> {
        let statuses = self.drone_statuses.read().await;
        statuses.get(drone_id).cloned()
    }

    pub async fn list_active_drones(&self) -> Vec<DroneStatus> {
        let statuses = self.drone_statuses.read().await;
        statuses.values().cloned().collect()
    }

    pub async fn process_commands(&self) -> Result<()> {
        let mut receiver = self.command_receiver.write().await;

        while let Ok(command) = receiver.try_recv() {
            self.handle_command(command).await?;
        }

        Ok(())
    }

    async fn handle_command(&self, command: ControlCommand) -> Result<()> {
        match command {
            ControlCommand::AssignMission {
                drone_id,
                mission_id,
            } => {
                self.mission_assigner
                    .write()
                    .await
                    .assign_mission(drone_id, mission_id)
                    .await?;
            }
            ControlCommand::FormSwarm {
                drone_ids,
                formation,
            } => {
                let formation_type = match formation {
                    Formation::Line { .. } => swarm::FormationType::Line,
                    Formation::Grid { .. } => swarm::FormationType::Grid,
                    Formation::Circle { .. } => swarm::FormationType::Circle,
                    Formation::VFormation { .. } => swarm::FormationType::V,
                    Formation::Custom { positions } => swarm::FormationType::Custom(
                        positions
                            .into_iter()
                            .map(|(x, y, _)| (x as f64, y as f64))
                            .collect(),
                    ),
                };
                let mut swarm =
                    DroneSwarm::new("Auto-Swarm".to_string(), drone_ids, formation_type);
                swarm.status = swarm::SwarmStatus::Forming;
                let mut controller = self.controller.write().await;
                controller.register_swarm(swarm)?;
            }
            ControlCommand::ExecuteCoordinatedAction { swarm_id, action } => {
                let action_str = format!("{:?}", action);
                self.coordination_engine
                    .write()
                    .await
                    .execute_action(swarm_id, action_str)
                    .await?;
            }
            ControlCommand::EmergencyLand { drone_ids } => {
                for drone_id in drone_ids {
                    // Send emergency land command to each drone
                    tracing::warn!("Emergency landing initiated for drone: {}", drone_id);
                }
            }
            ControlCommand::ReturnToBase { drone_ids } => {
                for drone_id in drone_ids {
                    // Send return to base command
                    tracing::info!("Return to base initiated for drone: {}", drone_id);
                }
            }
            ControlCommand::UpdateConstraints { constraints } => {
                let mut controller = self.controller.write().await;
                controller.update_constraints(constraints);
            }
        }

        Ok(())
    }

    pub async fn check_safety_violations(&self) -> Result<Vec<SafetyViolation>> {
        let statuses = self.drone_statuses.read().await;
        let controller = self.controller.read().await;
        let mut violations = Vec::new();

        for status in statuses.values() {
            // Check altitude violations
            if status.position.2 > controller.global_constraints.max_altitude_m {
                violations.push(SafetyViolation {
                    drone_id: status.id,
                    violation_type: ViolationType::AltitudeExceeded,
                    description: format!(
                        "Altitude {:.1}m exceeds maximum {:.1}m",
                        status.position.2, controller.global_constraints.max_altitude_m
                    ),
                    severity: Severity::High,
                    timestamp: Utc::now(),
                });
            }

            // Check geofence violations
            if !self.is_within_geofence(&status.position, &controller.global_constraints) {
                violations.push(SafetyViolation {
                    drone_id: status.id,
                    violation_type: ViolationType::GeofenceViolation,
                    description: "Drone outside geofence boundary".to_string(),
                    severity: Severity::Critical,
                    timestamp: Utc::now(),
                });
            }

            // Check no-fly zone violations
            for zone in &controller.global_constraints.no_fly_zones {
                if zone.active && self.is_in_no_fly_zone(&status.position, zone) {
                    violations.push(SafetyViolation {
                        drone_id: status.id,
                        violation_type: ViolationType::NoFlyZoneViolation,
                        description: format!("Drone in no-fly zone: {}", zone.name),
                        severity: Severity::Critical,
                        timestamp: Utc::now(),
                    });
                }
            }
        }

        Ok(violations)
    }

    fn is_within_geofence(
        &self,
        position: &(f64, f64, f32),
        constraints: &GlobalConstraints,
    ) -> bool {
        // Simple point-in-polygon check (for convex polygons)
        let (x, y, _) = *position;
        let boundary = &constraints.geofence_boundaries;

        if boundary.len() < 3 {
            return true; // No geofence defined
        }

        let mut inside = false;
        let mut j = boundary.len() - 1;

        for i in 0..boundary.len() {
            let (xi, yi) = boundary[i];
            let (xj, yj) = boundary[j];

            if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
                inside = !inside;
            }
            j = i;
        }

        inside
    }

    fn is_in_no_fly_zone(&self, position: &(f64, f64, f32), zone: &NoFlyZone) -> bool {
        let (x, y, z) = *position;

        // Check altitude restriction if present
        if let Some((min_alt, max_alt)) = zone.altitude_restriction {
            if z < min_alt || z > max_alt {
                return false;
            }
        }

        // Check if point is in zone boundary (simple point-in-polygon)
        let boundary = &zone.boundary;
        if boundary.len() < 3 {
            return false;
        }

        let mut inside = false;
        let mut j = boundary.len() - 1;

        for i in 0..boundary.len() {
            let (xi, yi) = boundary[i];
            let (xj, yj) = boundary[j];

            if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
                inside = !inside;
            }
            j = i;
        }

        inside
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyViolation {
    pub drone_id: Uuid,
    pub violation_type: ViolationType,
    pub description: String,
    pub severity: Severity,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationType {
    AltitudeExceeded,
    GeofenceViolation,
    NoFlyZoneViolation,
    CollisionRisk,
    CommunicationLoss,
    BatteryLow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_creation() {
        let controller = MultiDroneController::new("Test Controller".to_string());
        assert_eq!(controller.name, "Test Controller");
        assert!(controller.swarms.is_empty());
    }

    #[test]
    fn test_register_swarm_persists_owner_drone_ids_and_status() {
        let mut controller = MultiDroneController::new("Test Controller".to_string());
        let mut drone_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let mut expected_drone_ids = drone_ids.clone();
        expected_drone_ids.sort();

        let mut swarm = DroneSwarm::new_owned(
            "North Block".to_string(),
            std::mem::take(&mut drone_ids),
            swarm::FormationType::Line,
            "ops-team".to_string(),
        );
        swarm.status = swarm::SwarmStatus::Forming;
        let swarm_id = swarm.id;

        controller.register_swarm(swarm).unwrap();

        let entries = controller.list_swarm_registry();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].swarm_id, swarm_id);
        assert_eq!(entries[0].owner_id, "ops-team");
        assert_eq!(entries[0].drone_ids, expected_drone_ids);
        assert_eq!(entries[0].status, swarm::SwarmStatus::Forming);
    }

    #[test]
    fn test_swarm_lifecycle_transitions_are_deterministic() {
        let mut controller = MultiDroneController::new("Test Controller".to_string());
        let swarm = DroneSwarm::new_owned(
            "Lifecycle".to_string(),
            vec![Uuid::new_v4()],
            swarm::FormationType::Grid,
            "ops-team".to_string(),
        );
        let swarm_id = swarm.id;
        controller.register_swarm(swarm).unwrap();

        let invalid = controller
            .transition_swarm_status(swarm_id, swarm::SwarmStatus::Active)
            .unwrap_err();
        assert!(matches!(
            invalid,
            SwarmRegistryError::InvalidStatusTransition {
                from: swarm::SwarmStatus::Inactive,
                to: swarm::SwarmStatus::Active,
                ..
            }
        ));

        controller
            .transition_swarm_status(swarm_id, swarm::SwarmStatus::Forming)
            .unwrap();
        controller
            .transition_swarm_status(swarm_id, swarm::SwarmStatus::Active)
            .unwrap();

        assert_eq!(
            controller.get_swarm(&swarm_id).unwrap().status,
            swarm::SwarmStatus::Active
        );
    }

    #[test]
    fn test_active_drone_double_membership_is_rejected() {
        let mut controller = MultiDroneController::new("Test Controller".to_string());
        let shared_drone_id = Uuid::new_v4();

        let mut first = DroneSwarm::new_owned(
            "First".to_string(),
            vec![shared_drone_id],
            swarm::FormationType::Line,
            "ops-team".to_string(),
        );
        first.status = swarm::SwarmStatus::Active;
        let first_swarm_id = first.id;
        controller.register_swarm(first).unwrap();

        let mut second = DroneSwarm::new_owned(
            "Second".to_string(),
            vec![shared_drone_id],
            swarm::FormationType::Line,
            "ops-team".to_string(),
        );
        second.status = swarm::SwarmStatus::Active;
        let second_swarm_id = second.id;

        let err = controller.register_swarm(second).unwrap_err();

        assert!(matches!(
            err,
            SwarmRegistryError::ActiveDroneMembershipConflict {
                drone_id,
                existing_swarm_id,
                requested_swarm_id,
            } if drone_id == shared_drone_id
                && existing_swarm_id == first_swarm_id
                && requested_swarm_id == second_swarm_id
        ));
        assert!(controller.get_swarm(&second_swarm_id).is_none());
    }

    #[test]
    fn test_register_list_remove_contract() {
        let mut controller = MultiDroneController::new("Test Controller".to_string());
        let swarm = DroneSwarm::new_owned(
            "Contract".to_string(),
            vec![Uuid::new_v4()],
            swarm::FormationType::Circle,
            "ops-team".to_string(),
        );
        let swarm_id = swarm.id;

        controller.register_swarm(swarm).unwrap();
        assert_eq!(controller.list_swarm_registry().len(), 1);

        let removed = controller.remove_swarm(&swarm_id).unwrap();
        assert_eq!(removed.id, swarm_id);
        assert!(controller.list_swarm_registry().is_empty());
    }

    #[tokio::test]
    async fn test_service_creation() {
        let service = MultiDroneControlService::new("Test Service".to_string());
        let drones = service.list_active_drones().await;
        assert!(drones.is_empty());
    }

    #[tokio::test]
    async fn test_command_sending() {
        let service = MultiDroneControlService::new("Test Service".to_string());
        let command = ControlCommand::EmergencyLand {
            drone_ids: vec![Uuid::new_v4()],
        };
        let result = service.send_command(command).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_form_swarm_command_rejects_duplicate_active_membership() {
        let service = MultiDroneControlService::new("Test Service".to_string());
        let drone_id = Uuid::new_v4();

        service
            .send_command(ControlCommand::FormSwarm {
                drone_ids: vec![drone_id],
                formation: Formation::Line {
                    spacing_m: 5.0,
                    heading_deg: 0.0,
                },
            })
            .await
            .unwrap();
        service.process_commands().await.unwrap();

        service
            .send_command(ControlCommand::FormSwarm {
                drone_ids: vec![drone_id],
                formation: Formation::Grid {
                    rows: 1,
                    cols: 1,
                    spacing_m: 5.0,
                },
            })
            .await
            .unwrap();
        let err = service.process_commands().await.unwrap_err();

        assert!(err.to_string().contains("already in active swarm"));
        let controller = service.controller.read().await;
        let registry = controller.list_swarm_registry();
        assert_eq!(registry.len(), 1);
        assert_eq!(registry[0].drone_ids, vec![drone_id]);
        assert_eq!(registry[0].status, swarm::SwarmStatus::Forming);
    }
}
