use crate::mavlink_integration::MAVLinkCommandAckTracker;
use crate::{
    assert_failsafe_ready_for_arming, dispatch_guarded_simulation_command, AutomatedFailsafeConfig,
    AutomatedFailsafeError, DispatchSafetyReport, GuardedDispatchAuditEvent,
    GuardedDispatchCommand, GuardedDispatchContext, GuardedDispatchError, GuardedDispatchOutcome,
    Mission, MissionStateTransitionError, PreflightArmError, PreflightChecklistContext,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::RuntimeMode;
use std::fmt;

pub type AutonomousRuntimeMode = RuntimeMode;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomousOperatorApproval {
    pub operator_id: String,
    pub approved_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AutonomousExecutionPlan {
    pub enabled: bool,
    pub runtime_mode: AutonomousRuntimeMode,
    pub approval: Option<AutonomousOperatorApproval>,
    pub preflight_context: PreflightChecklistContext,
    pub failsafe_config: AutomatedFailsafeConfig,
    pub commands: Vec<GuardedDispatchCommand>,
    pub dispatch_contexts: Vec<GuardedDispatchContext>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutonomousExecutionOutcome {
    pub mission_id: uuid::Uuid,
    pub approval: AutonomousOperatorApproval,
    pub executed_commands: Vec<GuardedDispatchOutcome>,
    pub audit: Vec<GuardedDispatchAuditEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutonomousExecutionErrorCode {
    Disabled,
    ApprovalRequired,
    LiveModeRejected,
    InvalidPlan,
    FailsafeNotReady,
    PreflightRejected,
    StateTransition,
    SafetyHalt,
    DispatchRejected,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AutonomousExecutionError {
    Disabled,
    ApprovalRequired,
    LiveModeRejected,
    InvalidPlan {
        reason: String,
    },
    FailsafeNotReady(AutomatedFailsafeError),
    PreflightRejected(PreflightArmError),
    StateTransition(MissionStateTransitionError),
    SafetyHalt {
        report: DispatchSafetyReport,
        abort_plan: Option<crate::AbortRecoveryPlan>,
        completed_commands: usize,
    },
    DispatchRejected(GuardedDispatchError),
}

impl AutonomousExecutionError {
    pub fn code(&self) -> AutonomousExecutionErrorCode {
        match self {
            Self::Disabled => AutonomousExecutionErrorCode::Disabled,
            Self::ApprovalRequired => AutonomousExecutionErrorCode::ApprovalRequired,
            Self::LiveModeRejected => AutonomousExecutionErrorCode::LiveModeRejected,
            Self::InvalidPlan { .. } => AutonomousExecutionErrorCode::InvalidPlan,
            Self::FailsafeNotReady(_) => AutonomousExecutionErrorCode::FailsafeNotReady,
            Self::PreflightRejected(_) => AutonomousExecutionErrorCode::PreflightRejected,
            Self::StateTransition(_) => AutonomousExecutionErrorCode::StateTransition,
            Self::SafetyHalt { .. } => AutonomousExecutionErrorCode::SafetyHalt,
            Self::DispatchRejected(_) => AutonomousExecutionErrorCode::DispatchRejected,
        }
    }
}

impl fmt::Display for AutonomousExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => formatter.write_str("autonomous execution is disabled"),
            Self::ApprovalRequired => {
                formatter.write_str("autonomous execution requires operator approval")
            }
            Self::LiveModeRejected => {
                formatter.write_str("autonomous execution is simulation-only")
            }
            Self::InvalidPlan { reason } => write!(formatter, "invalid autonomous plan: {reason}"),
            Self::FailsafeNotReady(error) => write!(formatter, "{error}"),
            Self::PreflightRejected(error) => write!(formatter, "{error}"),
            Self::StateTransition(error) => write!(formatter, "{error}"),
            Self::SafetyHalt { report, .. } => write!(
                formatter,
                "autonomous execution halted with {} safety violation(s)",
                report.violations.len()
            ),
            Self::DispatchRejected(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for AutonomousExecutionError {}

impl From<MissionStateTransitionError> for AutonomousExecutionError {
    fn from(error: MissionStateTransitionError) -> Self {
        Self::StateTransition(error)
    }
}

impl From<PreflightArmError> for AutonomousExecutionError {
    fn from(error: PreflightArmError) -> Self {
        Self::PreflightRejected(error)
    }
}

impl From<AutomatedFailsafeError> for AutonomousExecutionError {
    fn from(error: AutomatedFailsafeError) -> Self {
        Self::FailsafeNotReady(error)
    }
}

pub fn execute_autonomous_mission_in_simulation(
    mission: &mut Mission,
    plan: AutonomousExecutionPlan,
    ack_tracker: &mut MAVLinkCommandAckTracker,
) -> Result<AutonomousExecutionOutcome, AutonomousExecutionError> {
    if !plan.enabled {
        return Err(AutonomousExecutionError::Disabled);
    }
    let approval = plan
        .approval
        .ok_or(AutonomousExecutionError::ApprovalRequired)?;
    if plan.runtime_mode != RuntimeMode::Simulation {
        return Err(AutonomousExecutionError::LiveModeRejected);
    }
    if plan.commands.is_empty() {
        return Err(AutonomousExecutionError::InvalidPlan {
            reason: "at least one autonomous command is required".to_string(),
        });
    }
    if plan.commands.len() != plan.dispatch_contexts.len() {
        return Err(AutonomousExecutionError::InvalidPlan {
            reason: format!(
                "{} command(s) but {} dispatch context(s)",
                plan.commands.len(),
                plan.dispatch_contexts.len()
            ),
        });
    }

    assert_failsafe_ready_for_arming(&plan.failsafe_config)?;
    mission.arm_with_preflight_checklist(&plan.preflight_context)?;
    mission.start()?;

    let mut executed_commands = Vec::new();
    let mut audit = Vec::new();

    for (command, context) in plan.commands.into_iter().zip(plan.dispatch_contexts) {
        match dispatch_guarded_simulation_command(mission, command, context, ack_tracker) {
            Ok(outcome) => {
                audit.extend(outcome.audit.clone());
                executed_commands.push(outcome);
            }
            Err(GuardedDispatchError::SafetyHalt { report, abort_plan }) => {
                mission.abort()?;
                return Err(AutonomousExecutionError::SafetyHalt {
                    report,
                    abort_plan,
                    completed_commands: executed_commands.len(),
                });
            }
            Err(error) => {
                return Err(AutonomousExecutionError::DispatchRejected(error));
            }
        }
    }

    mission.complete()?;
    Ok(AutonomousExecutionOutcome {
        mission_id: mission.id,
        approval,
        executed_commands,
        audit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mavlink_integration::{
        MAVLinkAckConfig, MAVLinkCommandAckTracker, MAV_CMD_NAV_TAKEOFF, MAV_CMD_NAV_WAYPOINT,
    };
    use crate::{
        AbortRecoveryConfig, AbortRecoveryContext, AbortTrigger, AutomatedFailsafeConfig,
        DispatchSafetyConfig, GpsFixStatus, GpsFixType, GuardedDispatchCommand,
        GuardedDispatchContext, Mission, MissionStatus, PreflightChecklistConfig,
        PreflightChecklistContext, TelemetryFreshness, TelemetryLinkState, Waypoint, WaypointType,
    };
    use chrono::{Duration, TimeZone, Utc};
    use geo::{point, polygon};
    use uuid::Uuid;

    fn validated_mission() -> Mission {
        let area = polygon![
            (x: 0.0, y: 0.0),
            (x: 200.0, y: 0.0),
            (x: 200.0, y: 200.0),
            (x: 0.0, y: 200.0),
            (x: 0.0, y: 0.0),
        ];
        let mut mission = Mission::new(
            "Autonomous Mission".to_string(),
            "autonomous execution fixture".to_string(),
            area,
        );
        mission.add_waypoint(Waypoint::new(
            point!(x: 10.0, y: 10.0),
            20.0,
            WaypointType::Takeoff,
        ));
        mission.add_waypoint(Waypoint::new(
            point!(x: 120.0, y: 120.0),
            40.0,
            WaypointType::Survey,
        ));
        mission.add_waypoint(Waypoint::new(
            point!(x: 20.0, y: 20.0),
            0.0,
            WaypointType::Landing,
        ));
        mission.validate().expect("fixture validates");
        mission
    }

    fn fresh_link(mission_id: Uuid) -> TelemetryFreshness {
        TelemetryFreshness {
            mission_id,
            drone_id: "drone-1".to_string(),
            state: TelemetryLinkState::Fresh,
            latest_timestamp: Some(Utc.timestamp_opt(100, 0).unwrap()),
            checked_at: Utc.timestamp_opt(102, 0).unwrap(),
            age_seconds: Some(2),
        }
    }

    fn preflight_context(mission: &Mission) -> PreflightChecklistContext {
        PreflightChecklistContext {
            current_position: Some(point!(x: 20.0, y: 20.0)),
            no_fly_zones: Vec::new(),
            config: PreflightChecklistConfig {
                dispatch_safety: DispatchSafetyConfig {
                    altitude_ceiling_m: 120.0,
                },
                minimum_launch_battery_percentage: 30,
                maximum_hdop: 1.5,
                minimum_satellites: 8,
            },
            battery_percentage: 88,
            mission_budget_report: None,
            weather: None,
            airspace_constraints: Vec::new(),
            gps: GpsFixStatus {
                fix_type: GpsFixType::ThreeD,
                hdop: 0.9,
                satellites: 11,
            },
            link_freshness: fresh_link(mission.id),
            failsafe_configured: true,
        }
    }

    fn failsafe_config() -> AutomatedFailsafeConfig {
        AutomatedFailsafeConfig {
            link_loss_threshold_seconds: 5,
            critical_battery_percentage: 20,
            emergency_landing_sites: vec![point!(x: 30.0, y: 30.0)],
            abort_recovery: AbortRecoveryConfig::default(),
        }
    }

    fn dispatch_context(
        mission: &Mission,
        current_position: geo::Point<f64>,
    ) -> GuardedDispatchContext {
        let sent_at = Utc.timestamp_opt(1_800_000_100, 0).unwrap();
        GuardedDispatchContext {
            mission_id: mission.id,
            current_position: Some(current_position),
            no_fly_zones: Vec::new(),
            dispatch_safety: DispatchSafetyConfig {
                altitude_ceiling_m: 120.0,
            },
            battery_percentage: 88,
            minimum_battery_percentage: 30,
            weather: None,
            airspace_constraints: Vec::new(),
            link_state: TelemetryLinkState::Fresh,
            sent_at,
            simulated_ack_latency: Duration::milliseconds(100),
            abort_context: Some(AbortRecoveryContext {
                current_position,
                home_position: point!(x: 0.0, y: 0.0),
                battery_percentage: 88,
                emergency_landing_sites: vec![point!(x: 30.0, y: 30.0)],
                trigger: AbortTrigger::GeofenceViolation,
                triggered_at: sent_at,
            }),
            abort_config: AbortRecoveryConfig::default(),
        }
    }

    fn approved_plan(mission: &Mission) -> AutonomousExecutionPlan {
        AutonomousExecutionPlan {
            enabled: true,
            runtime_mode: AutonomousRuntimeMode::Simulation,
            approval: Some(AutonomousOperatorApproval {
                operator_id: "ops-1".to_string(),
                approved_at: Utc.timestamp_opt(1_800_000_090, 0).unwrap(),
            }),
            preflight_context: preflight_context(mission),
            failsafe_config: failsafe_config(),
            commands: vec![
                GuardedDispatchCommand {
                    correlation_id: Uuid::new_v4(),
                    mavlink_command: MAV_CMD_NAV_TAKEOFF,
                    label: "takeoff".to_string(),
                },
                GuardedDispatchCommand {
                    correlation_id: Uuid::new_v4(),
                    mavlink_command: MAV_CMD_NAV_WAYPOINT,
                    label: "survey-leg".to_string(),
                },
            ],
            dispatch_contexts: vec![
                dispatch_context(mission, point!(x: 20.0, y: 20.0)),
                dispatch_context(mission, point!(x: 120.0, y: 120.0)),
            ],
        }
    }

    #[test]
    fn autonomous_execution_is_disabled_without_operator_approval() {
        let mut mission = validated_mission();
        let mut plan = approved_plan(&mission);
        plan.approval = None;
        let mut tracker = MAVLinkCommandAckTracker::new(MAVLinkAckConfig::default());

        let error = execute_autonomous_mission_in_simulation(&mut mission, plan, &mut tracker)
            .expect_err("autonomy requires explicit approval");

        assert_eq!(error.code(), AutonomousExecutionErrorCode::ApprovalRequired);
        assert_eq!(mission.status, MissionStatus::Validated);
    }

    #[test]
    fn autonomous_execution_runs_approved_simulation_plan() {
        let mut mission = validated_mission();
        let plan = approved_plan(&mission);
        let command_ids = plan
            .commands
            .iter()
            .map(|command| command.correlation_id)
            .collect::<Vec<_>>();
        let mut tracker = MAVLinkCommandAckTracker::new(MAVLinkAckConfig::default());

        let outcome = execute_autonomous_mission_in_simulation(&mut mission, plan, &mut tracker)
            .expect("approved simulation plan should run");

        assert_eq!(mission.status, MissionStatus::Completed);
        assert_eq!(outcome.mission_id, mission.id);
        assert_eq!(outcome.executed_commands.len(), 2);
        assert_eq!(outcome.audit.len(), 4);
        assert!(command_ids
            .iter()
            .all(|command_id| tracker.record(*command_id).is_some()));
    }

    #[test]
    fn autonomous_execution_halts_and_aborts_when_midflight_safety_turns_red() {
        let mut mission = validated_mission();
        let mut plan = approved_plan(&mission);
        let first_command_id = plan.commands[0].correlation_id;
        let second_command_id = plan.commands[1].correlation_id;
        plan.dispatch_contexts[1] = dispatch_context(&mission, point!(x: 250.0, y: 250.0));
        let mut tracker = MAVLinkCommandAckTracker::new(MAVLinkAckConfig::default());

        let error = execute_autonomous_mission_in_simulation(&mut mission, plan, &mut tracker)
            .expect_err("mid-flight safety violation should halt autonomy");

        match error {
            AutonomousExecutionError::SafetyHalt {
                report,
                abort_plan,
                completed_commands,
            } => {
                assert!(!report.is_clear());
                assert!(abort_plan.is_some());
                assert_eq!(completed_commands, 1);
            }
            other => panic!("expected safety halt, got {other:?}"),
        }
        assert_eq!(mission.status, MissionStatus::Aborted);
        assert!(tracker.record(first_command_id).is_some());
        assert!(tracker.record(second_command_id).is_none());
    }
}
