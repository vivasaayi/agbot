use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::{GeoCoordinate, RuntimeMode};
use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, RwLock};
use tokio::time::sleep;
use uuid::Uuid;

pub mod collision_avoidance;
pub mod coordinated_approval;
pub mod coordination;
pub mod mission_assignment;
pub mod swarm;
pub mod swarm_command;
pub mod synchronized_survey;

pub use collision_avoidance::{AvoidanceManeuver, CollisionAvoidanceSystem};
pub use coordinated_approval::{
    authorize_coordinated_execution, dry_run_coordinated_execution, ApprovalAuditEvent,
    ApprovalGateConfig, ApprovalGateError, CoordinatedExecutionDecision,
    CoordinatedExecutionDryRun, CoordinatedExecutionStatus, OperatorApproval,
};
pub use coordination::{
    CoordinationEngine, CoordinationRuleAuditEvent, CoordinationRuleExecution,
    CoordinationRuleExecutionKind, CoordinationStatus, DroneLinkHealth, DroneLinkStatus,
    DroneTelemetryFreshness, DroneTelemetrySnapshot, FormationAssignment,
    FormationOptimizationConfig, FormationOptimizationReport, LinkQualityReport,
    SwarmTelemetryReport, SwarmTelemetryStatus,
};
use coordination::{DroneOperationStatus, DroneState};
pub use mission_assignment::{
    AssignmentBatchReport, AssignmentFailureReason, DroneAssignment, MissionAssignmentEngine,
    UnassignableMission,
};
pub use swarm::{DroneSwarm, SwarmController, SwarmStatus};
pub use swarm_command::{
    dry_run_swarm_command, execute_audited_swarm_command, SwarmCommandAuditEvent,
    SwarmCommandConfig, SwarmCommandError, SwarmCommandOutcome, SwarmCommandRoute,
    SwarmCommandStatus,
};
pub use synchronized_survey::{
    evaluate_synchronized_survey_progress, plan_coverage_optimization, plan_synchronized_survey,
    CoverageOptimizationConfig, CoverageOptimizationPlan, DroneCoverageTime, SurveyExecutionStatus,
    SurveyLane, SurveyProgressReport, SurveySeparationSample, SynchronizedSurveyConfig,
    SynchronizedSurveyError, SynchronizedSurveyPlan,
};

const AUTONOMOUS_SURVEY_ENV_VAR: &str = "AUTONOMOUS_SURVEY_ENABLED";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomousSurveyConfig {
    pub enabled: bool,
    pub runtime_mode: RuntimeMode,
}

impl AutonomousSurveyConfig {
    pub fn from_env() -> Self {
        let enabled = env::var(AUTONOMOUS_SURVEY_ENV_VAR)
            .ok()
            .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));
        let runtime_mode = env::var("RUNTIME_MODE")
            .ok()
            .as_deref()
            .and_then(|value| RuntimeMode::from_str(value).ok())
            .unwrap_or(RuntimeMode::Simulation);

        Self {
            enabled,
            runtime_mode,
        }
    }
}

/// Multi-drone control system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiDroneController {
    pub id: Uuid,
    pub name: String,
    pub swarms: HashMap<Uuid, DroneSwarm>,
    pub global_constraints: GlobalConstraints,
    #[serde(default)]
    pub swarm_constraints: HashMap<Uuid, GlobalConstraints>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlobalConstraints {
    pub max_altitude_m: f32,
    pub geofence_boundaries: Vec<(f64, f64)>,
    pub no_fly_zones: Vec<NoFlyZone>,
    pub max_concurrent_drones: u32,
    pub emergency_landing_sites: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoFlyZone {
    pub id: Uuid,
    pub name: String,
    pub boundary: Vec<(f64, f64)>,
    pub altitude_restriction: Option<(f32, f32)>,
    pub reason: String,
    pub active: bool,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum GlobalConstraintValidationError {
    #[error("global constraints require a finite positive max altitude")]
    InvalidMaxAltitude,
    #[error("global constraints require a geofence polygon with at least three points")]
    EmptyGeofence,
    #[error("global constraints contain an invalid geofence coordinate")]
    InvalidGeofenceCoordinate,
    #[error("global constraints require max_concurrent_drones greater than zero")]
    InvalidMaxConcurrentDrones,
    #[error("global constraints require at least one emergency landing site")]
    MissingEmergencyLandingSite,
    #[error("global constraints contain invalid emergency landing site {index}")]
    InvalidEmergencyLandingSite { index: usize },
    #[error("no-fly zone {zone_id} is invalid: {reason}")]
    InvalidNoFlyZone { zone_id: Uuid, reason: String },
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SwarmConstraintPersistenceError {
    #[error("swarm not found for constraints: {swarm_id}")]
    SwarmNotFound { swarm_id: Uuid },
    #[error(transparent)]
    InvalidConstraints(#[from] GlobalConstraintValidationError),
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
        operator_approval: Option<OperatorApproval>,
    },
    AbortCoordinatedAction {
        swarm_id: Uuid,
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

impl CoordinatedAction {
    fn action_kind(&self) -> &'static str {
        match self {
            CoordinatedAction::SynchronizedSurvey { .. } => "synchronized_survey",
            CoordinatedAction::PatternSearch { .. } => "pattern_search",
            CoordinatedAction::CoverageOptimization { .. } => "coverage_optimization",
            CoordinatedAction::DataCollection { .. } => "data_collection",
        }
    }

    fn target_positions(&self) -> Vec<(f64, f64, f32)> {
        match self {
            CoordinatedAction::SynchronizedSurvey { area, .. }
            | CoordinatedAction::PatternSearch { area, .. } => {
                area.iter().map(|(x, y)| (*x, *y, 0.0)).collect()
            }
            CoordinatedAction::CoverageOptimization { .. } => Vec::new(),
            CoordinatedAction::DataCollection { collection_points } => collection_points.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwarmActionTarget {
    pub drone_id: Uuid,
    pub target_position: (f64, f64, f32),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwarmActionConstraintReport {
    pub action_ref: String,
    pub target_count: usize,
    pub checked_at: DateTime<Utc>,
    pub violations: Vec<SafetyViolation>,
}

impl SwarmActionConstraintReport {
    pub fn passed(&self) -> bool {
        self.violations.is_empty()
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SwarmActionSafetyError {
    #[error("swarm {swarm_id} not found for action {action_ref}")]
    SwarmNotFound { swarm_id: Uuid, action_ref: String },
    #[error("swarm {swarm_id} has no drones for action {action_ref}")]
    EmptySwarm { swarm_id: Uuid, action_ref: String },
    #[error("swarm action {action_ref} rejected with {violation_count} safety violation(s)")]
    Rejected {
        action_ref: String,
        violation_count: usize,
        report: SwarmActionConstraintReport,
    },
}

impl SwarmActionSafetyError {
    pub fn rejected_report(&self) -> Option<&SwarmActionConstraintReport> {
        match self {
            SwarmActionSafetyError::Rejected { report, .. } => Some(report),
            _ => None,
        }
    }

    pub fn report(&self) -> &SwarmActionConstraintReport {
        self.rejected_report()
            .expect("only rejected swarm action safety errors carry a report")
    }
}

/// Main control service
#[derive(Clone)]
pub struct MultiDroneControlService {
    controller: Arc<RwLock<MultiDroneController>>,
    drone_statuses: Arc<RwLock<HashMap<Uuid, DroneStatus>>>,
    coordination_engine: Arc<RwLock<CoordinationEngine>>,
    mission_assigner: Arc<RwLock<MissionAssignmentEngine>>,
    collision_avoidance: Arc<RwLock<CollisionAvoidanceSystem>>,
    safety_audit_log: Arc<RwLock<SafetyViolationAuditLog>>,
    autonomy_config: AutonomousSurveyConfig,
    active_autonomous_surveys: Arc<RwLock<HashMap<Uuid, watch::Sender<bool>>>>,
    command_sender: mpsc::UnboundedSender<ControlCommand>,
    command_receiver: Arc<RwLock<mpsc::UnboundedReceiver<ControlCommand>>>,
}

#[derive(Debug, Clone, Copy)]
enum AutonomousSurveyOutcome {
    Completed,
    AbortRequested,
    SafetyViolation,
}

impl MultiDroneController {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            swarms: HashMap::new(),
            global_constraints: GlobalConstraints::default(),
            swarm_constraints: HashMap::new(),
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

    pub fn update_constraints(
        &mut self,
        constraints: GlobalConstraints,
    ) -> std::result::Result<(), GlobalConstraintValidationError> {
        constraints.validate()?;
        self.global_constraints = constraints;
        Ok(())
    }

    pub fn set_global_constraints(
        &mut self,
        constraints: GlobalConstraints,
    ) -> std::result::Result<(), GlobalConstraintValidationError> {
        self.update_constraints(constraints)
    }

    pub fn save_swarm_constraints(
        &mut self,
        swarm_id: Uuid,
        constraints: GlobalConstraints,
    ) -> std::result::Result<&GlobalConstraints, SwarmConstraintPersistenceError> {
        if !self.swarms.contains_key(&swarm_id) {
            return Err(SwarmConstraintPersistenceError::SwarmNotFound { swarm_id });
        }
        constraints.validate()?;
        self.swarm_constraints.insert(swarm_id, constraints);
        Ok(self
            .swarm_constraints
            .get(&swarm_id)
            .expect("saved constraints must be retrievable"))
    }

    pub fn get_swarm_constraints(&self, swarm_id: Uuid) -> Option<&GlobalConstraints> {
        self.swarm_constraints.get(&swarm_id)
    }

    pub fn effective_constraints_for_swarm(&self, swarm_id: Uuid) -> &GlobalConstraints {
        self.swarm_constraints
            .get(&swarm_id)
            .unwrap_or(&self.global_constraints)
    }

    pub fn validate_swarm_action_targets(
        &self,
        action_ref: impl Into<String>,
        targets: &[SwarmActionTarget],
        checked_at: DateTime<Utc>,
    ) -> std::result::Result<SwarmActionConstraintReport, SwarmActionSafetyError> {
        let action_ref = action_ref.into().trim().to_string();
        self.validate_swarm_action_targets_with_constraints(
            action_ref,
            targets,
            checked_at,
            &self.global_constraints,
        )
    }

    fn validate_swarm_action_targets_with_constraints(
        &self,
        action_ref: String,
        targets: &[SwarmActionTarget],
        checked_at: DateTime<Utc>,
        constraints: &GlobalConstraints,
    ) -> std::result::Result<SwarmActionConstraintReport, SwarmActionSafetyError> {
        let mut violations = Vec::new();

        for target in targets {
            violations.extend(self.target_constraint_violations(
                &action_ref,
                target,
                checked_at,
                constraints,
            ));
        }

        let report = SwarmActionConstraintReport {
            action_ref: action_ref.clone(),
            target_count: targets.len(),
            checked_at,
            violations,
        };

        if report.passed() {
            Ok(report)
        } else {
            Err(SwarmActionSafetyError::Rejected {
                action_ref,
                violation_count: report.violations.len(),
                report,
            })
        }
    }

    pub fn validate_coordinated_action(
        &self,
        swarm_id: Uuid,
        action: &CoordinatedAction,
        checked_at: DateTime<Utc>,
    ) -> std::result::Result<SwarmActionConstraintReport, SwarmActionSafetyError> {
        let action_ref = format!("swarm:{swarm_id}:{}", action.action_kind());
        let swarm =
            self.swarms
                .get(&swarm_id)
                .ok_or_else(|| SwarmActionSafetyError::SwarmNotFound {
                    swarm_id,
                    action_ref: action_ref.clone(),
                })?;
        let target_positions = action.target_positions();

        if target_positions.is_empty() {
            return self.validate_swarm_action_targets_with_constraints(
                action_ref,
                &[],
                checked_at,
                self.effective_constraints_for_swarm(swarm_id),
            );
        }

        let drone_ids = swarm.drone_ids();
        if drone_ids.is_empty() {
            return Err(SwarmActionSafetyError::EmptySwarm {
                swarm_id,
                action_ref,
            });
        }

        let targets = target_positions
            .into_iter()
            .enumerate()
            .map(|(index, target_position)| SwarmActionTarget {
                drone_id: drone_ids[index % drone_ids.len()],
                target_position,
            })
            .collect::<Vec<_>>();

        self.validate_swarm_action_targets_with_constraints(
            action_ref,
            &targets,
            checked_at,
            self.effective_constraints_for_swarm(swarm_id),
        )
    }

    fn target_constraint_violations(
        &self,
        action_ref: &str,
        target: &SwarmActionTarget,
        checked_at: DateTime<Utc>,
        constraints: &GlobalConstraints,
    ) -> Vec<SafetyViolation> {
        let mut violations = Vec::new();

        if target.target_position.2 > constraints.max_altitude_m {
            violations.push(SafetyViolation {
                drone_id: target.drone_id,
                violation_type: ViolationType::AltitudeExceeded,
                description: format!(
                    "Target altitude {:.1}m exceeds maximum {:.1}m",
                    target.target_position.2, constraints.max_altitude_m
                ),
                severity: Severity::High,
                timestamp: checked_at,
                position: Some(target.target_position),
                action_ref: Some(action_ref.to_string()),
            });
        }

        if !Self::target_within_geofence(&target.target_position, constraints) {
            violations.push(SafetyViolation {
                drone_id: target.drone_id,
                violation_type: ViolationType::GeofenceViolation,
                description: "Target outside geofence boundary".to_string(),
                severity: Severity::Critical,
                timestamp: checked_at,
                position: Some(target.target_position),
                action_ref: Some(action_ref.to_string()),
            });
        }

        for zone in &constraints.no_fly_zones {
            if zone.active && Self::target_in_no_fly_zone(&target.target_position, zone) {
                violations.push(SafetyViolation {
                    drone_id: target.drone_id,
                    violation_type: ViolationType::NoFlyZoneViolation,
                    description: format!("Target inside no-fly zone: {}", zone.name),
                    severity: Severity::Critical,
                    timestamp: checked_at,
                    position: Some(target.target_position),
                    action_ref: Some(action_ref.to_string()),
                });
            }
        }

        violations
    }

    fn target_within_geofence(position: &(f64, f64, f32), constraints: &GlobalConstraints) -> bool {
        if constraints.geofence_boundaries.len() < 3 {
            return true;
        }

        Self::point_in_polygon(position.0, position.1, &constraints.geofence_boundaries)
    }

    fn target_in_no_fly_zone(position: &(f64, f64, f32), zone: &NoFlyZone) -> bool {
        if let Some((min_alt, max_alt)) = zone.altitude_restriction {
            if position.2 < min_alt || position.2 > max_alt {
                return false;
            }
        }

        if zone.boundary.len() < 3 {
            return false;
        }

        Self::point_in_polygon(position.0, position.1, &zone.boundary)
    }

    fn point_in_polygon(x: f64, y: f64, boundary: &[(f64, f64)]) -> bool {
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

impl GlobalConstraints {
    pub fn validate(&self) -> std::result::Result<(), GlobalConstraintValidationError> {
        if !self.max_altitude_m.is_finite() || self.max_altitude_m <= 0.0 {
            return Err(GlobalConstraintValidationError::InvalidMaxAltitude);
        }

        if self.geofence_boundaries.len() < 3 {
            return Err(GlobalConstraintValidationError::EmptyGeofence);
        }

        if self
            .geofence_boundaries
            .iter()
            .any(|(x, y)| !x.is_finite() || !y.is_finite())
        {
            return Err(GlobalConstraintValidationError::InvalidGeofenceCoordinate);
        }

        if self.max_concurrent_drones == 0 {
            return Err(GlobalConstraintValidationError::InvalidMaxConcurrentDrones);
        }

        if self.emergency_landing_sites.is_empty() {
            return Err(GlobalConstraintValidationError::MissingEmergencyLandingSite);
        }

        for (index, (x, y)) in self.emergency_landing_sites.iter().enumerate() {
            if !x.is_finite() || !y.is_finite() {
                return Err(GlobalConstraintValidationError::InvalidEmergencyLandingSite { index });
            }
        }

        for zone in &self.no_fly_zones {
            if zone.boundary.len() < 3 {
                return Err(GlobalConstraintValidationError::InvalidNoFlyZone {
                    zone_id: zone.id,
                    reason: "boundary requires at least three points".to_string(),
                });
            }
            if zone
                .boundary
                .iter()
                .any(|(x, y)| !x.is_finite() || !y.is_finite())
            {
                return Err(GlobalConstraintValidationError::InvalidNoFlyZone {
                    zone_id: zone.id,
                    reason: "boundary contains non-finite coordinate".to_string(),
                });
            }
            if let Some((min_altitude, max_altitude)) = zone.altitude_restriction {
                if !min_altitude.is_finite()
                    || !max_altitude.is_finite()
                    || min_altitude > max_altitude
                {
                    return Err(GlobalConstraintValidationError::InvalidNoFlyZone {
                        zone_id: zone.id,
                        reason: "altitude restriction is invalid".to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

impl MultiDroneControlService {
    pub fn new(controller_name: String) -> Self {
        Self::new_with_config(controller_name, AutonomousSurveyConfig::from_env())
    }

    pub fn new_with_config(
        controller_name: String,
        autonomy_config: AutonomousSurveyConfig,
    ) -> Self {
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
            safety_audit_log: Arc::new(RwLock::new(SafetyViolationAuditLog::default())),
            autonomy_config,
            active_autonomous_surveys: Arc::new(RwLock::new(HashMap::new())),
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
        let mut status = status;

        let executed_rules = {
            let drone_state = Self::drone_state_from_status(&status);
            status.status =
                Self::drone_operation_status_as_service_status(&drone_state.status, &status.status);

            let mut engine = self.coordination_engine.write().await;
            match engine
                .update_drone_state_with_rules(drone_state.id, drone_state.clone())
                .await
            {
                Ok(actions) => actions,
                Err(_) => {
                    if let Err(error) = engine
                        .register_drone(drone_state.id, drone_state.clone())
                        .await
                    {
                        tracing::warn!(
                            "failed to register drone {} for coordination rules: {error}",
                            drone_state.id
                        );
                        Vec::new()
                    } else {
                        engine
                            .update_drone_state_with_rules(drone_state.id, drone_state.clone())
                            .await
                            .unwrap_or_else(|error| {
                                tracing::warn!(
                                    "failed to evaluate coordination rules for drone {}: {error}",
                                    drone_state.id
                                );
                                Vec::new()
                            })
                    }
                }
            }
        };

        let emergency_sites = {
            let controller = self.controller.read().await;
            controller
                .global_constraints
                .emergency_landing_sites
                .clone()
        };

        {
            let mut statuses = self.drone_statuses.write().await;
            statuses.insert(status.id, status.clone());
            for execution in executed_rules {
                Self::apply_coordination_rule_execution(
                    &mut statuses,
                    &execution,
                    &emergency_sites,
                );
            }
        }
    }

    fn drone_state_from_status(status: &DroneStatus) -> DroneState {
        DroneState {
            id: status.id,
            position: GeoCoordinate {
                latitude: status.position.0,
                longitude: status.position.1,
                altitude_m: status.position.2,
            },
            velocity: status.velocity,
            heading: 0.0,
            battery_level: status.battery_level,
            status: Self::drone_operation_status_from_service_status(&status.status),
            current_mission: status.assigned_mission,
            last_update: status.last_update,
            communication_quality: 1.0,
        }
    }

    fn drone_operation_status_from_service_status(status: &str) -> DroneOperationStatus {
        match status.to_lowercase().as_str() {
            "idle" => DroneOperationStatus::Idle,
            "in_transit" => DroneOperationStatus::InTransit,
            "returning" | "return_to_base" => DroneOperationStatus::Returning,
            "emergency" | "emergency_landing" => DroneOperationStatus::Emergency,
            "maintenance" => DroneOperationStatus::Maintenance,
            "executing_mission" | "in_mission" => DroneOperationStatus::ExecutingMission,
            _ => DroneOperationStatus::ExecutingMission,
        }
    }

    fn drone_operation_status_as_service_status(
        operation_status: &DroneOperationStatus,
        fallback_status: &str,
    ) -> String {
        match operation_status {
            DroneOperationStatus::Idle => "idle".to_string(),
            DroneOperationStatus::InTransit => "in_transit".to_string(),
            DroneOperationStatus::ExecutingMission => fallback_status.to_string(),
            DroneOperationStatus::Returning => "return_to_base".to_string(),
            DroneOperationStatus::Emergency => "emergency".to_string(),
            DroneOperationStatus::Maintenance => "maintenance".to_string(),
        }
    }

    fn apply_coordination_rule_execution(
        statuses: &mut HashMap<Uuid, DroneStatus>,
        execution: &CoordinationRuleExecution,
        emergency_sites: &[(f64, f64)],
    ) {
        let Some(status) = statuses.get_mut(&execution.drone_id) else {
            return;
        };

        match execution.action {
            CoordinationRuleExecutionKind::ReturnToBase => {
                status.status = "return_to_base".to_string();
            }
            CoordinationRuleExecutionKind::LandImmediate => {
                status.status = "emergency".to_string();
                status.position.2 = 0.0;
            }
            CoordinationRuleExecutionKind::LandAtNearestEmergencySite => {
                if let Some((site_x, site_y)) = Self::nearest_emergency_site(
                    (status.position.0, status.position.1),
                    emergency_sites,
                ) {
                    status.status = "emergency".to_string();
                    status.position = (site_x, site_y, 0.0);
                } else {
                    status.status = "emergency".to_string();
                    status.position.2 = 0.0;
                }
            }
            CoordinationRuleExecutionKind::AvoidanceAltitude { delta } => {
                status.position.2 = (status.position.2 + delta).max(0.0);
            }
        }
    }

    fn nearest_emergency_site(
        position: (f64, f64),
        emergency_sites: &[(f64, f64)],
    ) -> Option<(f64, f64)> {
        emergency_sites.iter().copied().min_by(|left, right| {
            Self::distance_2d(position, *left)
                .partial_cmp(&Self::distance_2d(position, *right))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    fn distance_2d(left: (f64, f64), right: (f64, f64)) -> f64 {
        let dx = left.0 - right.0;
        let dy = left.1 - right.1;
        (dx * dx + dy * dy).sqrt()
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
            ControlCommand::ExecuteCoordinatedAction {
                swarm_id,
                action,
                operator_approval,
            } => match action {
                CoordinatedAction::SynchronizedSurvey { .. } => {
                    self.start_autonomous_survey(swarm_id, action, operator_approval)
                        .await?
                }
                _ => {
                    let action_str = format!("{:?}", action);
                    let validation_result = {
                        let controller = self.controller.read().await;
                        controller.validate_coordinated_action(swarm_id, &action, Utc::now())
                    };
                    if let Err(err) = validation_result {
                        if let Some(report) = err.rejected_report() {
                            self.audit_safety_violations(&report.violations, Utc::now())
                                .await;
                        }
                        return Err(anyhow::anyhow!(err.to_string()));
                    }
                    self.coordination_engine
                        .write()
                        .await
                        .execute_action(swarm_id, action_str)
                        .await?;
                }
            },
            ControlCommand::AbortCoordinatedAction { swarm_id } => {
                self.abort_autonomous_survey(swarm_id).await?;
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
                controller.update_constraints(constraints)?;
            }
        }

        Ok(())
    }

    async fn audit_safety_violations(
        &self,
        violations: &[SafetyViolation],
        recorded_at: DateTime<Utc>,
    ) {
        let mut audit_log = self.safety_audit_log.write().await;
        for violation in violations {
            audit_log.append(violation.clone(), recorded_at);
        }
    }

    async fn start_autonomous_survey(
        &self,
        swarm_id: Uuid,
        action: CoordinatedAction,
        operator_approval: Option<OperatorApproval>,
    ) -> Result<()> {
        if !self.autonomy_config.enabled {
            return Err(anyhow::anyhow!(
                "autonomous coordinated execution is disabled by configuration"
            ));
        }

        if self.autonomy_config.runtime_mode != RuntimeMode::Simulation {
            return Err(anyhow::anyhow!(
                "autonomous coordinated execution is simulation-only and requires runtime mode SIMULATION"
            ));
        }

        if let CoordinatedAction::SynchronizedSurvey {
            area,
            overlap_percent,
        } = &action
        {
            let checked_at = Utc::now();
            let approval_config = ApprovalGateConfig {
                min_predicted_separation_m: self.autonomy_config_min_separation_m(),
                planned_altitude_m: self.autonomy_config_planned_altitude_m(),
            };
            let dry_run = {
                let controller = self.controller.read().await;
                dry_run_coordinated_execution(
                    &controller,
                    swarm_id,
                    action.clone(),
                    approval_config,
                    checked_at,
                )
            }?;

            let decision = authorize_coordinated_execution(&dry_run, operator_approval, checked_at);
            if !decision.permitted {
                let blocked_reason = decision
                    .audit
                    .first()
                    .map(|entry| entry.message.clone())
                    .unwrap_or_else(|| "coordinated execution blocked by operator".to_string());
                return Err(anyhow::anyhow!(blocked_reason));
            }

            let survey_config = SynchronizedSurveyConfig {
                planned_altitude_m: approval_config.planned_altitude_m,
                min_separation_m: approval_config.min_predicted_separation_m,
                overlap_percent: *overlap_percent,
            };
            let plan = {
                let controller = self.controller.read().await;
                controller.plan_synchronized_survey(
                    swarm_id,
                    area.clone(),
                    survey_config.clone(),
                    checked_at,
                )?
            };

            if plan.status != SurveyExecutionStatus::Planned {
                return Err(anyhow::anyhow!(
                    "autonomous coordinated survey plan is not executable in planned state"
                ));
            }

            if plan
                .separation_violations
                .iter()
                .any(|violation| violation.severity == Severity::Critical)
            {
                return Err(anyhow::anyhow!(
                    "autonomous coordinated survey plan contains critical separation violations"
                ));
            }

            {
                let active = self.active_autonomous_surveys.write().await;
                if active.contains_key(&swarm_id) {
                    return Err(anyhow::anyhow!(
                        "an autonomous survey is already running for this swarm"
                    ));
                }
            }

            let (abort_tx, abort_rx) = watch::channel(false);
            {
                let mut active = self.active_autonomous_surveys.write().await;
                active.insert(swarm_id, abort_tx);
            }

            let service = self.clone();
            let step_plan = plan;
            let step_survey_config = survey_config;
            tokio::spawn(async move {
                let outcome = service
                    .run_autonomous_survey_session(
                        swarm_id,
                        step_plan,
                        step_survey_config,
                        abort_rx,
                    )
                    .await;
                let mut active = service.active_autonomous_surveys.write().await;
                active.remove(&swarm_id);
                if let Err(error) = outcome {
                    tracing::warn!(
                        "autonomous survey for swarm {swarm_id} finished with error: {error}"
                    );
                }
            });

            Ok(())
        } else {
            unreachable!("only synchronized survey actions are routed to autonomous execution")
        }
    }

    async fn abort_autonomous_survey(&self, swarm_id: Uuid) -> Result<()> {
        let abort_tx = {
            let sessions = self.active_autonomous_surveys.read().await;
            sessions.get(&swarm_id).cloned().ok_or_else(|| {
                anyhow::anyhow!("no running autonomous survey for requested swarm")
            })?
        };
        abort_tx.send(true).map_err(|error| {
            anyhow::anyhow!("failed to request abort for autonomous survey: {error}")
        })?;
        self.fail_safe_land_for_swarm(swarm_id).await
    }

    async fn run_autonomous_survey_session(
        &self,
        swarm_id: Uuid,
        plan: SynchronizedSurveyPlan,
        config: SynchronizedSurveyConfig,
        mut abort_rx: watch::Receiver<bool>,
    ) -> Result<AutonomousSurveyOutcome> {
        const SURVEY_STEPS: usize = 12;
        let mut step = 0usize;

        while step <= SURVEY_STEPS {
            tokio::select! {
                _ = abort_rx.changed() => {
                    if *abort_rx.borrow() {
                        self.fail_safe_land_for_swarm(swarm_id).await?;
                        return Ok(AutonomousSurveyOutcome::AbortRequested);
                    }
                },
                _ = sleep(Duration::from_millis(20)) => {}
            }

            let progress = evaluate_synchronized_survey_progress(
                &plan,
                &[SurveySeparationSample {
                    elapsed_s: step as f64,
                    positions: plan
                        .lanes
                        .iter()
                        .map(|lane| {
                            let span = step as f64 / SURVEY_STEPS as f64;
                            let position = self.interpolate_position(
                                lane.start_xy,
                                lane.end_xy,
                                span,
                                lane.planned_altitude_m,
                            );
                            (lane.drone_id, position)
                        })
                        .collect(),
                }],
                config.clone(),
                Utc::now(),
            );

            if !progress.separation_violations.is_empty() {
                self.fail_safe_land_for_swarm(swarm_id).await?;
                return Ok(AutonomousSurveyOutcome::SafetyViolation);
            }

            let violations = self.check_safety_violations().await?;
            if !violations.is_empty() {
                self.fail_safe_land_for_swarm(swarm_id).await?;
                return Ok(AutonomousSurveyOutcome::SafetyViolation);
            }

            step += 1;
        }

        self.complete_autonomous_survey(swarm_id).await
    }

    async fn complete_autonomous_survey(&self, _swarm_id: Uuid) -> Result<AutonomousSurveyOutcome> {
        Ok(AutonomousSurveyOutcome::Completed)
    }

    async fn fail_safe_land_for_swarm(&self, swarm_id: Uuid) -> Result<()> {
        let drone_ids = {
            let controller = self.controller.read().await;
            controller
                .get_swarm(&swarm_id)
                .ok_or_else(|| anyhow::anyhow!("swarm {swarm_id} not found"))?
                .drone_ids()
        };

        let command = ControlCommand::EmergencyLand { drone_ids };
        let controller = self.controller.read().await;
        let outcome = execute_audited_swarm_command(
            &controller,
            &command,
            SwarmCommandConfig::default(),
            Utc::now(),
        )?;

        if outcome.status != SwarmCommandStatus::Executed {
            return Err(anyhow::anyhow!(format!(
                "fail-safe for swarm {swarm_id} could not be executed"
            )));
        }

        Ok(())
    }

    fn autonomy_config_min_separation_m(&self) -> f64 {
        25.0
    }

    fn autonomy_config_planned_altitude_m(&self) -> f32 {
        30.0
    }

    fn interpolate_position(
        &self,
        start_xy: (f64, f64),
        end_xy: (f64, f64),
        span: f64,
        altitude_m: f32,
    ) -> (f64, f64, f32) {
        let normalized = span.clamp(0.0, 1.0);
        let x = start_xy.0 + (end_xy.0 - start_xy.0) * normalized;
        let y = start_xy.1 + (end_xy.1 - start_xy.1) * normalized;
        (x, y, altitude_m)
    }

    #[cfg(test)]
    async fn is_autonomous_survey_active(&self, swarm_id: Uuid) -> bool {
        self.active_autonomous_surveys
            .read()
            .await
            .contains_key(&swarm_id)
    }

    pub async fn list_safety_violation_audit_records(&self) -> Vec<SafetyViolationAuditRecord> {
        self.safety_audit_log.read().await.records()
    }

    pub async fn check_safety_audit_completeness(
        &self,
        expected: &[SafetyViolation],
    ) -> SafetyAuditCompletenessReport {
        self.safety_audit_log
            .read()
            .await
            .check_completeness(expected)
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
                    position: Some(status.position),
                    action_ref: None,
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
                    position: Some(status.position),
                    action_ref: None,
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
                        position: Some(status.position),
                        action_ref: None,
                    });
                }
            }
        }

        drop(controller);
        drop(statuses);
        self.audit_safety_violations(&violations, Utc::now()).await;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SafetyViolation {
    pub drone_id: Uuid,
    pub violation_type: ViolationType,
    pub description: String,
    pub severity: Severity,
    pub timestamp: DateTime<Utc>,
    pub position: Option<(f64, f64, f32)>,
    pub action_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SafetyViolationAuditRecord {
    pub audit_id: String,
    pub sequence: u64,
    pub violation: SafetyViolation,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SafetyViolationAuditLog {
    records: Vec<SafetyViolationAuditRecord>,
}

impl SafetyViolationAuditLog {
    pub fn append(
        &mut self,
        violation: SafetyViolation,
        recorded_at: DateTime<Utc>,
    ) -> SafetyViolationAuditRecord {
        let sequence = self.records.len() as u64 + 1;
        let record = SafetyViolationAuditRecord {
            audit_id: format!("safety-violation-{sequence:06}"),
            sequence,
            violation,
            recorded_at,
        };
        self.records.push(record.clone());
        record
    }

    pub fn records(&self) -> Vec<SafetyViolationAuditRecord> {
        self.records.clone()
    }

    pub fn check_completeness(
        &self,
        expected: &[SafetyViolation],
    ) -> SafetyAuditCompletenessReport {
        let gaps = expected
            .iter()
            .filter(|expected_violation| {
                !self
                    .records
                    .iter()
                    .any(|record| record.violation == **expected_violation)
            })
            .map(SafetyViolationAuditGap::from)
            .collect::<Vec<_>>();

        SafetyAuditCompletenessReport {
            expected_count: expected.len(),
            audited_count: self.records.len(),
            gaps,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SafetyAuditCompletenessReport {
    pub expected_count: usize,
    pub audited_count: usize,
    pub gaps: Vec<SafetyViolationAuditGap>,
}

impl SafetyAuditCompletenessReport {
    pub fn passed(&self) -> bool {
        self.gaps.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SafetyViolationAuditGap {
    pub drone_id: Uuid,
    pub violation_type: ViolationType,
    pub severity: Severity,
    pub timestamp: DateTime<Utc>,
    pub position: Option<(f64, f64, f32)>,
    pub action_ref: Option<String>,
}

impl From<&SafetyViolation> for SafetyViolationAuditGap {
    fn from(violation: &SafetyViolation) -> Self {
        Self {
            drone_id: violation.drone_id,
            violation_type: violation.violation_type.clone(),
            severity: violation.severity.clone(),
            timestamp: violation.timestamp,
            position: violation.position,
            action_ref: violation.action_ref.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ViolationType {
    AltitudeExceeded,
    GeofenceViolation,
    NoFlyZoneViolation,
    CollisionRisk,
    CommunicationLoss,
    BatteryLow,
}

impl ViolationType {
    pub fn all() -> Vec<Self> {
        vec![
            ViolationType::AltitudeExceeded,
            ViolationType::GeofenceViolation,
            ViolationType::NoFlyZoneViolation,
            ViolationType::CollisionRisk,
            ViolationType::CommunicationLoss,
            ViolationType::BatteryLow,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn all() -> Vec<Self> {
        vec![
            Severity::Low,
            Severity::Medium,
            Severity::High,
            Severity::Critical,
        ]
    }
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

    #[test]
    fn complete_swarm_constraints_persist_and_round_trip() {
        let mut controller = MultiDroneController::new("Constraint Controller".to_string());
        let swarm = DroneSwarm::new_owned(
            "Constrained".to_string(),
            vec![Uuid::new_v4()],
            swarm::FormationType::Grid,
            "ops-team".to_string(),
        );
        let swarm_id = swarm.id;
        controller.register_swarm(swarm).unwrap();
        let constraints = constrained_controller().global_constraints;

        let saved = controller
            .save_swarm_constraints(swarm_id, constraints.clone())
            .expect("complete constraints save");

        assert_eq!(saved, &constraints);
        assert_eq!(
            controller.get_swarm_constraints(swarm_id),
            Some(&constraints)
        );

        let serialized = serde_json::to_string(&controller).expect("controller serializes");
        let restored: MultiDroneController =
            serde_json::from_str(&serialized).expect("controller deserializes");
        assert_eq!(restored.get_swarm_constraints(swarm_id), Some(&constraints));
    }

    #[test]
    fn swarm_constraints_reject_missing_emergency_landing_site() {
        let mut controller = MultiDroneController::new("Constraint Controller".to_string());
        let swarm = DroneSwarm::new_owned(
            "Constrained".to_string(),
            vec![Uuid::new_v4()],
            swarm::FormationType::Line,
            "ops-team".to_string(),
        );
        let swarm_id = swarm.id;
        controller.register_swarm(swarm).unwrap();
        let mut constraints = constrained_controller().global_constraints;
        constraints.emergency_landing_sites.clear();

        let err = controller
            .save_swarm_constraints(swarm_id, constraints)
            .expect_err("missing emergency landing site must reject save");

        assert!(matches!(
            err,
            SwarmConstraintPersistenceError::InvalidConstraints(
                GlobalConstraintValidationError::MissingEmergencyLandingSite
            )
        ));
        assert!(controller.get_swarm_constraints(swarm_id).is_none());
    }

    #[test]
    fn coordinated_action_uses_saved_swarm_constraints() {
        let mut controller = MultiDroneController::new("Constraint Controller".to_string());
        let swarm = DroneSwarm::new_owned(
            "Constrained".to_string(),
            vec![Uuid::new_v4()],
            swarm::FormationType::Line,
            "ops-team".to_string(),
        );
        let swarm_id = swarm.id;
        controller.register_swarm(swarm).unwrap();
        controller
            .save_swarm_constraints(swarm_id, constrained_controller().global_constraints)
            .expect("swarm constraints save");
        let action = CoordinatedAction::DataCollection {
            collection_points: vec![(5.0, 5.0, 50.0)],
        };

        let err = controller
            .validate_coordinated_action(swarm_id, &action, fixed_time())
            .expect_err("swarm-specific no-fly zone must reject target");
        let report = err.report();

        assert_eq!(report.target_count, 1);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(
            report.violations[0].violation_type,
            ViolationType::NoFlyZoneViolation
        );
    }

    #[test]
    fn swarm_action_targets_inside_constraints_pass_pre_execution_check() {
        let controller = constrained_controller();
        let drone_a = Uuid::new_v4();
        let drone_b = Uuid::new_v4();
        let targets = vec![
            SwarmActionTarget {
                drone_id: drone_a,
                target_position: (30.0, 30.0, 40.0),
            },
            SwarmActionTarget {
                drone_id: drone_b,
                target_position: (-20.0, 20.0, 45.0),
            },
        ];

        let report = controller
            .validate_swarm_action_targets("survey:north-block", &targets, fixed_time())
            .expect("all targets are inside constraints");

        assert!(report.passed());
        assert_eq!(report.action_ref, "survey:north-block");
        assert_eq!(report.target_count, 2);
        assert!(report.violations.is_empty());
    }

    #[test]
    fn no_fly_target_rejects_entire_swarm_action_without_partial_pass() {
        let controller = constrained_controller();
        let safe_drone = Uuid::new_v4();
        let unsafe_drone = Uuid::new_v4();
        let targets = vec![
            SwarmActionTarget {
                drone_id: safe_drone,
                target_position: (-40.0, -40.0, 50.0),
            },
            SwarmActionTarget {
                drone_id: unsafe_drone,
                target_position: (5.0, 5.0, 50.0),
            },
        ];

        let err = controller
            .validate_swarm_action_targets("survey:north-block", &targets, fixed_time())
            .expect_err("one no-fly target must reject the whole action");
        let report = err.report();

        assert!(!report.passed());
        assert_eq!(report.action_ref, "survey:north-block");
        assert_eq!(report.target_count, 2);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].drone_id, unsafe_drone);
        assert_eq!(
            report.violations[0].violation_type,
            ViolationType::NoFlyZoneViolation
        );
        assert_eq!(report.violations[0].severity, Severity::Critical);
        assert_eq!(
            report.violations[0].action_ref.as_deref(),
            Some("survey:north-block")
        );
        assert_eq!(report.violations[0].position, Some((5.0, 5.0, 50.0)));
    }

    #[test]
    fn geofence_and_altitude_target_violations_are_reported_per_drone() {
        let controller = constrained_controller();
        let geofence_drone = Uuid::new_v4();
        let altitude_drone = Uuid::new_v4();
        let targets = vec![
            SwarmActionTarget {
                drone_id: geofence_drone,
                target_position: (150.0, 10.0, 50.0),
            },
            SwarmActionTarget {
                drone_id: altitude_drone,
                target_position: (10.0, 10.0, 121.0),
            },
        ];

        let err = controller
            .validate_swarm_action_targets("survey:north-block", &targets, fixed_time())
            .expect_err("geofence and altitude target violations must reject the action");
        let report = err.report();
        let violation_types = report
            .violations
            .iter()
            .map(|violation| violation.violation_type.clone())
            .collect::<Vec<_>>();

        assert_eq!(report.violations.len(), 2);
        assert!(violation_types.contains(&ViolationType::GeofenceViolation));
        assert!(violation_types.contains(&ViolationType::AltitudeExceeded));
    }

    #[test]
    fn safety_violation_audit_log_appends_context_and_detects_dropped_gap() {
        let mut log = SafetyViolationAuditLog::default();
        let audited = sample_violation(
            Uuid::new_v4(),
            ViolationType::GeofenceViolation,
            Severity::Critical,
            (150.0, 10.0, 50.0),
        );
        let dropped = sample_violation(
            Uuid::new_v4(),
            ViolationType::AltitudeExceeded,
            Severity::High,
            (10.0, 10.0, 121.0),
        );

        let record = log.append(audited.clone(), fixed_time());

        assert_eq!(record.audit_id, "safety-violation-000001");
        assert_eq!(record.sequence, 1);
        assert_eq!(record.violation, audited);
        assert_eq!(log.records().len(), 1);

        let report = log.check_completeness(&[audited, dropped.clone()]);

        assert!(!report.passed());
        assert_eq!(report.expected_count, 2);
        assert_eq!(report.audited_count, 1);
        assert_eq!(report.gaps.len(), 1);
        assert_eq!(report.gaps[0].drone_id, dropped.drone_id);
        assert_eq!(
            report.gaps[0].violation_type,
            ViolationType::AltitudeExceeded
        );
        assert_eq!(
            report.gaps[0].action_ref.as_deref(),
            Some("survey:north-block")
        );
    }

    #[tokio::test]
    async fn service_check_safety_violations_persists_geofence_breach_context() {
        let service = MultiDroneControlService::new("Test Service".to_string());
        let drone_id = Uuid::new_v4();
        {
            let mut controller = service.controller.write().await;
            controller.global_constraints = constrained_controller().global_constraints;
        }
        service
            .update_drone_status(DroneStatus {
                id: drone_id,
                position: (150.0, 10.0, 50.0),
                velocity: (0.0, 0.0, 0.0),
                battery_level: 0.9,
                status: "in_mission".to_string(),
                assigned_mission: None,
                last_update: fixed_time(),
            })
            .await;

        let violations = service.check_safety_violations().await.unwrap();
        let records = service.list_safety_violation_audit_records().await;

        assert_eq!(violations.len(), 1);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].audit_id, "safety-violation-000001");
        assert_eq!(
            records[0].violation.violation_type,
            ViolationType::GeofenceViolation
        );
        assert_eq!(records[0].violation.drone_id, drone_id);
        assert_eq!(records[0].violation.position, Some((150.0, 10.0, 50.0)));
    }

    #[test]
    fn safety_violation_taxonomy_has_six_types_and_four_severities() {
        assert_eq!(ViolationType::all().len(), 6);
        assert_eq!(Severity::all().len(), 4);
        assert!(ViolationType::all().contains(&ViolationType::NoFlyZoneViolation));
        assert!(Severity::all().contains(&Severity::Critical));
    }

    #[tokio::test]
    async fn test_service_creation() {
        let service = MultiDroneControlService::new("Test Service".to_string());
        let drones = service.list_active_drones().await;
        assert!(drones.is_empty());
    }

    #[tokio::test]
    async fn service_update_drone_status_applies_low_battery_rule_to_nearest_site() {
        let service = MultiDroneControlService::new("Test Service".to_string());
        {
            let mut controller = service.controller.write().await;
            controller.global_constraints.emergency_landing_sites =
                vec![(10.0, 10.0), (100.0, 100.0)];
        }
        let drone_id = Uuid::new_v4();
        let stale_position = (15.0, 12.0, 120.0);

        service
            .update_drone_status(DroneStatus {
                id: drone_id,
                position: stale_position,
                velocity: (0.0, 0.0, 0.0),
                battery_level: 0.1,
                status: "executing_mission".to_string(),
                assigned_mission: None,
                last_update: Utc::now(),
            })
            .await;

        let status = service.get_drone_status(&drone_id).await.unwrap();
        let audit_log = service
            .coordination_engine
            .read()
            .await
            .coordination_rule_audit_log()
            .to_vec();
        let matching_audits = audit_log
            .iter()
            .filter(|event| event.drone_id == Some(drone_id))
            .collect::<Vec<_>>();

        assert_eq!(status.status, "emergency");
        assert_eq!(status.position, (10.0, 10.0, 0.0));
        assert_eq!(matching_audits.len(), 1);
        assert_eq!(matching_audits[0].action, "LandAtNearestEmergencySite");
    }

    #[tokio::test]
    async fn service_update_drone_status_prefers_return_to_base_over_low_battery_on_conflict() {
        let service = MultiDroneControlService::new("Test Service".to_string());
        let drone_id = Uuid::new_v4();
        let stale_time = Utc::now() - chrono::Duration::seconds(90);

        service
            .update_drone_status(DroneStatus {
                id: drone_id,
                position: (20.0, 20.0, 90.0),
                velocity: (0.0, 0.0, 0.0),
                battery_level: 0.1,
                status: "executing_mission".to_string(),
                assigned_mission: None,
                last_update: stale_time,
            })
            .await;

        let status = service.get_drone_status(&drone_id).await.unwrap();
        let audit_log = service
            .coordination_engine
            .read()
            .await
            .coordination_rule_audit_log()
            .to_vec();
        let matching_audit = audit_log
            .iter()
            .find(|event| event.drone_id == Some(drone_id))
            .expect("rule should be executed");

        assert_eq!(status.status, "return_to_base");
        assert_eq!(matching_audit.action, "ReturnToBase");
    }

    #[tokio::test]
    async fn service_update_drone_status_applies_proximity_avoidance_for_neighbors() {
        let service = MultiDroneControlService::new("Test Service".to_string());
        let west_id = Uuid::new_v4();
        let now = Utc::now();

        service
            .update_drone_status(DroneStatus {
                id: west_id,
                position: (40.0, -74.0, 100.0),
                velocity: (0.0, 0.0, 0.0),
                battery_level: 0.8,
                status: "executing_mission".to_string(),
                assigned_mission: None,
                last_update: now,
            })
            .await;
        let east_id = Uuid::new_v4();
        service
            .update_drone_status(DroneStatus {
                id: east_id,
                position: (41.0, -74.0, 100.0),
                velocity: (0.0, 0.0, 0.0),
                battery_level: 0.8,
                status: "executing_mission".to_string(),
                assigned_mission: None,
                last_update: now,
            })
            .await;
        service
            .update_drone_status(DroneStatus {
                id: east_id,
                position: (40.0002, -74.0003, 100.0),
                velocity: (0.0, 0.0, 0.0),
                battery_level: 0.1,
                status: "executing_mission".to_string(),
                assigned_mission: None,
                last_update: now,
            })
            .await;

        let west_status = service.get_drone_status(&west_id).await.unwrap();
        let east_status = service.get_drone_status(&east_id).await.unwrap();

        assert_eq!(west_status.position.2, 110.0);
        assert_eq!(east_status.position.2, 110.0);
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

    #[tokio::test]
    async fn execute_coordinated_action_rechecks_constraints_before_execution() {
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

        let swarm_id = {
            let mut controller = service.controller.write().await;
            controller.global_constraints = constrained_controller().global_constraints;
            controller.list_swarm_registry()[0].swarm_id
        };

        service
            .send_command(ControlCommand::ExecuteCoordinatedAction {
                swarm_id,
                action: CoordinatedAction::DataCollection {
                    collection_points: vec![(5.0, 5.0, 50.0)],
                },
                operator_approval: None,
            })
            .await
            .unwrap();
        let err = service.process_commands().await.unwrap_err();

        assert!(err.to_string().contains("rejected with 1 safety violation"));
    }

    #[tokio::test]
    async fn autonomous_survey_rejects_when_autonomy_disabled() {
        let service = MultiDroneControlService::new_with_config(
            "Test Service".to_string(),
            AutonomousSurveyConfig {
                enabled: false,
                runtime_mode: RuntimeMode::Simulation,
            },
        );
        let drone_id = Uuid::new_v4();

        service
            .send_command(ControlCommand::FormSwarm {
                drone_ids: vec![drone_id],
                formation: Formation::Line {
                    spacing_m: 25.0,
                    heading_deg: 0.0,
                },
            })
            .await
            .unwrap();
        service.process_commands().await.unwrap();

        let swarm_id = {
            let controller = service.controller.read().await;
            controller.list_swarm_registry()[0].swarm_id
        };

        service
            .send_command(ControlCommand::ExecuteCoordinatedAction {
                swarm_id,
                action: CoordinatedAction::SynchronizedSurvey {
                    area: vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
                    overlap_percent: 15.0,
                },
                operator_approval: Some(OperatorApproval {
                    approved: true,
                    approved_by: "ops".to_string(),
                    approved_at: Utc::now(),
                }),
            })
            .await
            .unwrap();
        let err = service.process_commands().await.unwrap_err();

        assert!(err
            .to_string()
            .contains("autonomous coordinated execution is disabled"));
    }

    #[tokio::test]
    async fn autonomous_survey_rejects_when_runtime_not_simulation() {
        let service = MultiDroneControlService::new_with_config(
            "Test Service".to_string(),
            AutonomousSurveyConfig {
                enabled: true,
                runtime_mode: RuntimeMode::Flight,
            },
        );
        let drone_id = Uuid::new_v4();

        service
            .send_command(ControlCommand::FormSwarm {
                drone_ids: vec![drone_id],
                formation: Formation::Line {
                    spacing_m: 25.0,
                    heading_deg: 0.0,
                },
            })
            .await
            .unwrap();
        service.process_commands().await.unwrap();

        let swarm_id = {
            let controller = service.controller.read().await;
            controller.list_swarm_registry()[0].swarm_id
        };

        service
            .send_command(ControlCommand::ExecuteCoordinatedAction {
                swarm_id,
                action: CoordinatedAction::SynchronizedSurvey {
                    area: vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
                    overlap_percent: 15.0,
                },
                operator_approval: Some(OperatorApproval {
                    approved: true,
                    approved_by: "ops".to_string(),
                    approved_at: Utc::now(),
                }),
            })
            .await
            .unwrap();
        let err = service.process_commands().await.unwrap_err();

        assert!(err
            .to_string()
            .contains("autonomous coordinated execution is simulation-only"));
    }

    #[tokio::test]
    async fn autonomous_survey_can_be_aborted_mid_mission() {
        let service = MultiDroneControlService::new_with_config(
            "Test Service".to_string(),
            AutonomousSurveyConfig {
                enabled: true,
                runtime_mode: RuntimeMode::Simulation,
            },
        );
        let drone_ids = vec![Uuid::new_v4(), Uuid::new_v4()];

        service
            .send_command(ControlCommand::FormSwarm {
                drone_ids: drone_ids.clone(),
                formation: Formation::Line {
                    spacing_m: 25.0,
                    heading_deg: 0.0,
                },
            })
            .await
            .unwrap();
        service.process_commands().await.unwrap();

        let swarm_id = {
            let controller = service.controller.read().await;
            controller.list_swarm_registry()[0].swarm_id
        };

        service
            .send_command(ControlCommand::ExecuteCoordinatedAction {
                swarm_id,
                action: CoordinatedAction::SynchronizedSurvey {
                    area: vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
                    overlap_percent: 15.0,
                },
                operator_approval: Some(OperatorApproval {
                    approved: true,
                    approved_by: "ops".to_string(),
                    approved_at: Utc::now(),
                }),
            })
            .await
            .unwrap();
        service.process_commands().await.unwrap();

        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(service.is_autonomous_survey_active(swarm_id).await);

        service
            .send_command(ControlCommand::AbortCoordinatedAction { swarm_id })
            .await
            .unwrap();
        service.process_commands().await.unwrap();

        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(!service.is_autonomous_survey_active(swarm_id).await);
    }

    #[tokio::test]
    async fn autonomous_survey_stops_on_red_safety_violation() {
        let service = MultiDroneControlService::new_with_config(
            "Test Service".to_string(),
            AutonomousSurveyConfig {
                enabled: true,
                runtime_mode: RuntimeMode::Simulation,
            },
        );
        let drone_id = Uuid::new_v4();

        service
            .send_command(ControlCommand::FormSwarm {
                drone_ids: vec![drone_id],
                formation: Formation::Line {
                    spacing_m: 25.0,
                    heading_deg: 0.0,
                },
            })
            .await
            .unwrap();
        service.process_commands().await.unwrap();

        {
            let mut controller = service.controller.write().await;
            controller.global_constraints.max_altitude_m = 80.0;
        }

        let swarm_id = {
            let controller = service.controller.read().await;
            controller.list_swarm_registry()[0].swarm_id
        };
        service
            .send_command(ControlCommand::ExecuteCoordinatedAction {
                swarm_id,
                action: CoordinatedAction::SynchronizedSurvey {
                    area: vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
                    overlap_percent: 15.0,
                },
                operator_approval: Some(OperatorApproval {
                    approved: true,
                    approved_by: "ops".to_string(),
                    approved_at: Utc::now(),
                }),
            })
            .await
            .unwrap();
        service.process_commands().await.unwrap();

        service
            .update_drone_status(DroneStatus {
                id: drone_id,
                position: (0.0, 0.0, 120.0),
                velocity: (0.0, 0.0, 0.0),
                battery_level: 0.9,
                status: "in_mission".to_string(),
                assigned_mission: None,
                last_update: Utc::now(),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(160)).await;
        assert!(!service.is_autonomous_survey_active(swarm_id).await);
    }

    fn constrained_controller() -> MultiDroneController {
        let mut controller = MultiDroneController::new("Safety Controller".to_string());
        controller.global_constraints = GlobalConstraints {
            max_altitude_m: 120.0,
            geofence_boundaries: vec![
                (-100.0, -100.0),
                (100.0, -100.0),
                (100.0, 100.0),
                (-100.0, 100.0),
            ],
            no_fly_zones: vec![NoFlyZone {
                id: Uuid::new_v4(),
                name: "Farmhouse".to_string(),
                boundary: vec![(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)],
                altitude_restriction: Some((0.0, 120.0)),
                reason: "people and structures".to_string(),
                active: true,
            }],
            max_concurrent_drones: 4,
            emergency_landing_sites: vec![(0.0, -80.0)],
        };
        controller
    }

    fn fixed_time() -> DateTime<Utc> {
        "2026-06-12T12:00:00Z"
            .parse()
            .expect("fixed timestamp should parse")
    }

    fn sample_violation(
        drone_id: Uuid,
        violation_type: ViolationType,
        severity: Severity,
        position: (f64, f64, f32),
    ) -> SafetyViolation {
        SafetyViolation {
            drone_id,
            violation_type,
            description: "sample violation".to_string(),
            severity,
            timestamp: fixed_time(),
            position: Some(position),
            action_ref: Some("survey:north-block".to_string()),
        }
    }
}
