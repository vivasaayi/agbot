use crate::{
    ControlCommand, Formation, MultiDroneController, SafetyViolation, Severity, ViolationType,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct SwarmCommandConfig {
    pub min_formation_separation_m: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SwarmCommandStatus {
    DryRunPassed,
    Executed,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwarmCommandRoute {
    pub drone_id: Uuid,
    pub target_position: (f64, f64, f32),
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwarmCommandAuditEvent {
    pub command_kind: String,
    pub at: DateTime<Utc>,
    pub status: SwarmCommandStatus,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwarmCommandOutcome {
    pub command_kind: String,
    pub status: SwarmCommandStatus,
    pub routes: Vec<SwarmCommandRoute>,
    pub violations: Vec<SafetyViolation>,
    pub audit: Vec<SwarmCommandAuditEvent>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SwarmCommandError {
    #[error("unsupported swarm command for audited dry-run")]
    UnsupportedCommand,
    #[error("drone {drone_id} not found in any registered swarm")]
    DroneNotFound { drone_id: Uuid },
    #[error("emergency landing requires at least one configured emergency site")]
    MissingEmergencyLandingSite,
    #[error("formation requires {expected} positions but got {actual}")]
    FormationPositionCount { expected: usize, actual: usize },
}

impl Default for SwarmCommandConfig {
    fn default() -> Self {
        Self {
            min_formation_separation_m: 25.0,
        }
    }
}

impl MultiDroneController {
    pub fn dry_run_swarm_command(
        &self,
        command: &ControlCommand,
        config: SwarmCommandConfig,
        checked_at: DateTime<Utc>,
    ) -> Result<SwarmCommandOutcome, SwarmCommandError> {
        dry_run_swarm_command(self, command, config, checked_at)
    }
}

pub fn dry_run_swarm_command(
    controller: &MultiDroneController,
    command: &ControlCommand,
    config: SwarmCommandConfig,
    checked_at: DateTime<Utc>,
) -> Result<SwarmCommandOutcome, SwarmCommandError> {
    match command {
        ControlCommand::EmergencyLand { drone_ids } => {
            let routes = emergency_land_routes(controller, drone_ids)?;
            Ok(outcome(
                "emergency_land",
                SwarmCommandStatus::DryRunPassed,
                routes,
                Vec::new(),
                checked_at,
                "emergency_land dry-run routed drones to emergency sites".to_string(),
            ))
        }
        ControlCommand::ReturnToBase { drone_ids } => {
            let routes = sorted_drone_ids(drone_ids)
                .into_iter()
                .map(|drone_id| {
                    require_drone_position(controller, drone_id)?;
                    Ok(SwarmCommandRoute {
                        drone_id,
                        target_position: (0.0, 0.0, 0.0),
                        reason: "return_to_base".to_string(),
                    })
                })
                .collect::<Result<Vec<_>, SwarmCommandError>>()?;
            Ok(outcome(
                "return_to_base",
                SwarmCommandStatus::DryRunPassed,
                routes,
                Vec::new(),
                checked_at,
                "return_to_base dry-run routed drones to base".to_string(),
            ))
        }
        ControlCommand::FormSwarm {
            drone_ids,
            formation,
        } => dry_run_form_swarm(controller, drone_ids, formation, config, checked_at),
        _ => Err(SwarmCommandError::UnsupportedCommand),
    }
}

pub fn execute_audited_swarm_command(
    controller: &MultiDroneController,
    command: &ControlCommand,
    config: SwarmCommandConfig,
    executed_at: DateTime<Utc>,
) -> Result<SwarmCommandOutcome, SwarmCommandError> {
    let mut dry_run = dry_run_swarm_command(controller, command, config, executed_at)?;
    if dry_run.status == SwarmCommandStatus::Rejected {
        return Ok(dry_run);
    }

    dry_run.status = SwarmCommandStatus::Executed;
    dry_run.audit = vec![SwarmCommandAuditEvent {
        command_kind: dry_run.command_kind.clone(),
        at: executed_at,
        status: SwarmCommandStatus::Executed,
        message: format!(
            "{} executed with {} route(s)",
            dry_run.command_kind,
            dry_run.routes.len()
        ),
    }];
    Ok(dry_run)
}

fn dry_run_form_swarm(
    controller: &MultiDroneController,
    drone_ids: &[Uuid],
    formation: &Formation,
    config: SwarmCommandConfig,
    checked_at: DateTime<Utc>,
) -> Result<SwarmCommandOutcome, SwarmCommandError> {
    let drone_ids = sorted_drone_ids(drone_ids);
    for drone_id in &drone_ids {
        require_drone_position(controller, *drone_id)?;
    }
    let positions = formation_positions(formation, drone_ids.len())?;
    let routes = drone_ids
        .iter()
        .zip(positions.iter())
        .map(|(drone_id, position)| SwarmCommandRoute {
            drone_id: *drone_id,
            target_position: *position,
            reason: "form_swarm".to_string(),
        })
        .collect::<Vec<_>>();
    let violations = formation_separation_violations(&routes, config, checked_at);

    if !violations.is_empty() {
        return Ok(outcome(
            "form_swarm",
            SwarmCommandStatus::Rejected,
            Vec::new(),
            violations,
            checked_at,
            "unsafe form_swarm rejected before execution".to_string(),
        ));
    }

    Ok(outcome(
        "form_swarm",
        SwarmCommandStatus::DryRunPassed,
        routes,
        Vec::new(),
        checked_at,
        "form_swarm dry-run passed".to_string(),
    ))
}

fn emergency_land_routes(
    controller: &MultiDroneController,
    drone_ids: &[Uuid],
) -> Result<Vec<SwarmCommandRoute>, SwarmCommandError> {
    if controller
        .global_constraints
        .emergency_landing_sites
        .is_empty()
    {
        return Err(SwarmCommandError::MissingEmergencyLandingSite);
    }

    sorted_drone_ids(drone_ids)
        .into_iter()
        .map(|drone_id| {
            let current_position = require_drone_position(controller, drone_id)?;
            let site = nearest_emergency_site(
                (current_position.0, current_position.1),
                &controller.global_constraints.emergency_landing_sites,
            );
            Ok(SwarmCommandRoute {
                drone_id,
                target_position: (site.0, site.1, 0.0),
                reason: "nearest_emergency_site".to_string(),
            })
        })
        .collect()
}

fn require_drone_position(
    controller: &MultiDroneController,
    drone_id: Uuid,
) -> Result<(f64, f64, f32), SwarmCommandError> {
    controller
        .swarms
        .values()
        .find_map(|swarm| swarm.drones.get(&drone_id).map(|drone| drone.position))
        .ok_or(SwarmCommandError::DroneNotFound { drone_id })
}

fn nearest_emergency_site(position: (f64, f64), sites: &[(f64, f64)]) -> (f64, f64) {
    sites
        .iter()
        .copied()
        .min_by(|left, right| {
            distance_2d(position, *left)
                .partial_cmp(&distance_2d(position, *right))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("emergency site list is non-empty")
}

fn formation_positions(
    formation: &Formation,
    drone_count: usize,
) -> Result<Vec<(f64, f64, f32)>, SwarmCommandError> {
    match formation {
        Formation::Custom { positions } => {
            if positions.len() != drone_count {
                return Err(SwarmCommandError::FormationPositionCount {
                    expected: drone_count,
                    actual: positions.len(),
                });
            }
            Ok(positions
                .iter()
                .map(|(x, y, z)| (f64::from(*x), f64::from(*y), *z))
                .collect())
        }
        Formation::Line {
            spacing_m,
            heading_deg,
        } => {
            let heading = f64::from(*heading_deg).to_radians();
            Ok((0..drone_count)
                .map(|index| {
                    let offset = f64::from(*spacing_m) * index as f64;
                    (offset * heading.sin(), offset * heading.cos(), 0.0)
                })
                .collect())
        }
        Formation::Grid {
            rows: _,
            cols,
            spacing_m,
        } => {
            let cols = (*cols).max(1) as usize;
            Ok((0..drone_count)
                .map(|index| {
                    let row = index / cols;
                    let col = index % cols;
                    (
                        f64::from(*spacing_m) * col as f64,
                        f64::from(*spacing_m) * row as f64,
                        0.0,
                    )
                })
                .collect())
        }
        Formation::Circle { radius_m, center } => Ok((0..drone_count)
            .map(|index| {
                let angle = (index as f64 / drone_count.max(1) as f64) * std::f64::consts::TAU;
                (
                    center.0 + f64::from(*radius_m) * angle.cos(),
                    center.1 + f64::from(*radius_m) * angle.sin(),
                    0.0,
                )
            })
            .collect()),
        Formation::VFormation {
            spacing_m,
            angle_deg,
        } => Ok((0..drone_count)
            .map(|index| {
                if index == 0 {
                    return (0.0, 0.0, 0.0);
                }
                let side = if index % 2 == 0 { -1.0 } else { 1.0 };
                let rank = ((index + 1) / 2) as f64;
                let angle = f64::from(*angle_deg).to_radians();
                (
                    side * rank * f64::from(*spacing_m) * angle.sin(),
                    rank * f64::from(*spacing_m) * angle.cos(),
                    0.0,
                )
            })
            .collect()),
    }
}

fn formation_separation_violations(
    routes: &[SwarmCommandRoute],
    config: SwarmCommandConfig,
    checked_at: DateTime<Utc>,
) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();
    for left_index in 0..routes.len() {
        for right_index in (left_index + 1)..routes.len() {
            let left = &routes[left_index];
            let right = &routes[right_index];
            let distance_m = distance_3d(left.target_position, right.target_position);
            if distance_m < config.min_formation_separation_m {
                violations.push(formation_violation(
                    left.drone_id,
                    left.target_position,
                    distance_m,
                    config.min_formation_separation_m,
                    checked_at,
                ));
                violations.push(formation_violation(
                    right.drone_id,
                    right.target_position,
                    distance_m,
                    config.min_formation_separation_m,
                    checked_at,
                ));
            }
        }
    }
    violations
}

fn formation_violation(
    drone_id: Uuid,
    position: (f64, f64, f32),
    distance_m: f64,
    required_m: f64,
    timestamp: DateTime<Utc>,
) -> SafetyViolation {
    SafetyViolation {
        drone_id,
        violation_type: ViolationType::CollisionRisk,
        description: format!(
            "FormSwarm separation breach: {:.1}m observed, {:.1}m required",
            distance_m, required_m
        ),
        severity: Severity::Critical,
        timestamp,
        position: Some(position),
        action_ref: Some("form_swarm".to_string()),
    }
}

fn outcome(
    command_kind: &str,
    status: SwarmCommandStatus,
    routes: Vec<SwarmCommandRoute>,
    violations: Vec<SafetyViolation>,
    at: DateTime<Utc>,
    message: String,
) -> SwarmCommandOutcome {
    SwarmCommandOutcome {
        command_kind: command_kind.to_string(),
        status,
        routes,
        violations,
        audit: vec![SwarmCommandAuditEvent {
            command_kind: command_kind.to_string(),
            at,
            status,
            message,
        }],
    }
}

fn sorted_drone_ids(drone_ids: &[Uuid]) -> Vec<Uuid> {
    let mut ids = drone_ids.to_vec();
    ids.sort();
    ids
}

fn distance_2d(left: (f64, f64), right: (f64, f64)) -> f64 {
    (left.0 - right.0).hypot(left.1 - right.1)
}

fn distance_3d(left: (f64, f64, f32), right: (f64, f64, f32)) -> f64 {
    let altitude_delta = f64::from(left.2 - right.2);
    (left.0 - right.0)
        .hypot(left.1 - right.1)
        .hypot(altitude_delta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        swarm::FormationType, ControlCommand, DroneSwarm, Formation, GlobalConstraints,
        MultiDroneController,
    };
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn controller_with_swarm() -> (MultiDroneController, Uuid, Uuid) {
        let drone_a = Uuid::from_u128(201);
        let drone_b = Uuid::from_u128(202);
        let mut controller = MultiDroneController::new("command coordinator".to_string());
        controller.global_constraints = GlobalConstraints {
            emergency_landing_sites: vec![(0.0, 0.0), (100.0, 0.0)],
            ..GlobalConstraints::default()
        };
        let mut swarm = DroneSwarm::new_owned(
            "command swarm".to_string(),
            vec![drone_a, drone_b],
            FormationType::Line,
            "ops-team".to_string(),
        );
        swarm.status = crate::swarm::SwarmStatus::Active;
        swarm.drones.get_mut(&drone_a).unwrap().position = (10.0, 0.0, 25.0);
        swarm.drones.get_mut(&drone_b).unwrap().position = (90.0, 0.0, 25.0);
        controller.register_swarm(swarm).unwrap();
        (controller, drone_a, drone_b)
    }

    #[test]
    fn emergency_land_routes_each_drone_to_nearest_site_and_audits() {
        let (controller, drone_a, drone_b) = controller_with_swarm();

        let outcome = execute_audited_swarm_command(
            &controller,
            &ControlCommand::EmergencyLand {
                drone_ids: vec![drone_b, drone_a],
            },
            SwarmCommandConfig::default(),
            Utc.timestamp_opt(1_800_000_200, 0).unwrap(),
        )
        .expect("emergency land should route");

        assert_eq!(outcome.status, SwarmCommandStatus::Executed);
        assert_eq!(outcome.routes.len(), 2);
        assert_eq!(outcome.routes[0].drone_id, drone_a);
        assert_eq!(outcome.routes[0].target_position, (0.0, 0.0, 0.0));
        assert_eq!(outcome.routes[1].drone_id, drone_b);
        assert_eq!(outcome.routes[1].target_position, (100.0, 0.0, 0.0));
        assert!(outcome.audit[0].message.contains("emergency_land executed"));
    }

    #[test]
    fn unsafe_form_swarm_dry_run_rejects_before_execution() {
        let (controller, drone_a, drone_b) = controller_with_swarm();

        let outcome = dry_run_swarm_command(
            &controller,
            &ControlCommand::FormSwarm {
                drone_ids: vec![drone_a, drone_b],
                formation: Formation::Custom {
                    positions: vec![(0.0, 0.0, 30.0), (5.0, 0.0, 30.0)],
                },
            },
            SwarmCommandConfig {
                min_formation_separation_m: 25.0,
            },
            Utc.timestamp_opt(1_800_000_200, 0).unwrap(),
        )
        .expect("dry-run should produce a rejection report");

        assert_eq!(outcome.status, SwarmCommandStatus::Rejected);
        assert_eq!(outcome.violations.len(), 2);
        assert!(outcome.routes.is_empty());
        assert!(outcome.audit[0]
            .message
            .contains("unsafe form_swarm rejected"));
    }
}
