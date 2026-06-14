use crate::{
    plan_synchronized_survey, CoordinatedAction, MultiDroneController, SynchronizedSurveyConfig,
    SynchronizedSurveyError,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ApprovalGateConfig {
    pub min_predicted_separation_m: f64,
    pub planned_altitude_m: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CoordinatedExecutionStatus {
    WaitingForApproval,
    Approved,
    Rejected,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApprovalAuditEvent {
    pub approval_id: String,
    pub swarm_id: Uuid,
    pub at: DateTime<Utc>,
    pub status: CoordinatedExecutionStatus,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoordinatedExecutionDryRun {
    pub approval_id: String,
    pub swarm_id: Uuid,
    pub action_kind: String,
    pub status: CoordinatedExecutionStatus,
    pub predicted_min_separation_m: f64,
    pub safety_violation_count: usize,
    pub audit: Vec<ApprovalAuditEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OperatorApproval {
    pub approved: bool,
    pub approved_by: String,
    pub approved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoordinatedExecutionDecision {
    pub approval_id: String,
    pub swarm_id: Uuid,
    pub permitted: bool,
    pub status: CoordinatedExecutionStatus,
    pub approved_by: Option<String>,
    pub audit: Vec<ApprovalAuditEvent>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum ApprovalGateError {
    #[error("coordinated action dry-run rejected: {reason}")]
    SafetyRejected { reason: String },
    #[error("coordinated action dry-run produced unsafe separation: {predicted_min_separation_m:.1}m below {required_min_separation_m:.1}m")]
    SeparationBelowMinimum {
        predicted_min_separation_m: f64,
        required_min_separation_m: f64,
    },
}

impl Default for ApprovalGateConfig {
    fn default() -> Self {
        Self {
            min_predicted_separation_m: 25.0,
            planned_altitude_m: 30.0,
        }
    }
}

impl MultiDroneController {
    pub fn dry_run_coordinated_execution(
        &self,
        swarm_id: Uuid,
        action: CoordinatedAction,
        config: ApprovalGateConfig,
        checked_at: DateTime<Utc>,
    ) -> Result<CoordinatedExecutionDryRun, ApprovalGateError> {
        dry_run_coordinated_execution(self, swarm_id, action, config, checked_at)
    }
}

pub fn dry_run_coordinated_execution(
    controller: &MultiDroneController,
    swarm_id: Uuid,
    action: CoordinatedAction,
    config: ApprovalGateConfig,
    checked_at: DateTime<Utc>,
) -> Result<CoordinatedExecutionDryRun, ApprovalGateError> {
    let action_kind = action_kind(&action).to_string();
    let approval_id = approval_id(swarm_id, &action_kind, checked_at);
    let (predicted_min_separation_m, safety_violation_count) =
        dry_run_safety(controller, swarm_id, &action, config, checked_at)?;

    if predicted_min_separation_m < config.min_predicted_separation_m {
        return Err(ApprovalGateError::SeparationBelowMinimum {
            predicted_min_separation_m,
            required_min_separation_m: config.min_predicted_separation_m,
        });
    }

    Ok(CoordinatedExecutionDryRun {
        approval_id: approval_id.clone(),
        swarm_id,
        action_kind,
        status: CoordinatedExecutionStatus::WaitingForApproval,
        predicted_min_separation_m,
        safety_violation_count,
        audit: vec![ApprovalAuditEvent {
            approval_id,
            swarm_id,
            at: checked_at,
            status: CoordinatedExecutionStatus::WaitingForApproval,
            message: "dry-run complete; waiting for operator approval".to_string(),
        }],
    })
}

pub fn authorize_coordinated_execution(
    dry_run: &CoordinatedExecutionDryRun,
    approval: Option<OperatorApproval>,
    checked_at: DateTime<Utc>,
) -> CoordinatedExecutionDecision {
    let Some(approval) = approval else {
        return decision(
            dry_run,
            false,
            CoordinatedExecutionStatus::Blocked,
            None,
            checked_at,
            "coordinated execution blocked: missing operator approval".to_string(),
        );
    };

    if !approval.approved {
        return decision(
            dry_run,
            false,
            CoordinatedExecutionStatus::Rejected,
            Some(approval.approved_by),
            approval.approved_at,
            "coordinated execution rejected by operator".to_string(),
        );
    }

    if dry_run.status != CoordinatedExecutionStatus::WaitingForApproval
        || dry_run.safety_violation_count > 0
    {
        return decision(
            dry_run,
            false,
            CoordinatedExecutionStatus::Blocked,
            Some(approval.approved_by),
            checked_at,
            "coordinated execution blocked: dry-run is not executable".to_string(),
        );
    }

    let approved_by = approval.approved_by;
    decision(
        dry_run,
        true,
        CoordinatedExecutionStatus::Approved,
        Some(approved_by.clone()),
        approval.approved_at,
        format!("coordinated execution approved by {approved_by}"),
    )
}

fn dry_run_safety(
    controller: &MultiDroneController,
    swarm_id: Uuid,
    action: &CoordinatedAction,
    config: ApprovalGateConfig,
    checked_at: DateTime<Utc>,
) -> Result<(f64, usize), ApprovalGateError> {
    match action {
        CoordinatedAction::SynchronizedSurvey {
            area,
            overlap_percent,
        } => {
            let plan = plan_synchronized_survey(
                controller,
                swarm_id,
                area.clone(),
                SynchronizedSurveyConfig {
                    planned_altitude_m: config.planned_altitude_m,
                    min_separation_m: config.min_predicted_separation_m,
                    overlap_percent: *overlap_percent,
                },
                checked_at,
            )
            .map_err(survey_error_to_gate_error)?;

            Ok((
                predicted_min_lane_separation(&plan.lanes).unwrap_or(f64::INFINITY),
                plan.separation_violations.len(),
            ))
        }
        _ => {
            let report = controller
                .validate_coordinated_action(swarm_id, action, checked_at)
                .map_err(|error| ApprovalGateError::SafetyRejected {
                    reason: error.to_string(),
                })?;
            Ok((f64::INFINITY, report.violations.len()))
        }
    }
}

fn decision(
    dry_run: &CoordinatedExecutionDryRun,
    permitted: bool,
    status: CoordinatedExecutionStatus,
    approved_by: Option<String>,
    at: DateTime<Utc>,
    message: String,
) -> CoordinatedExecutionDecision {
    CoordinatedExecutionDecision {
        approval_id: dry_run.approval_id.clone(),
        swarm_id: dry_run.swarm_id,
        permitted,
        status,
        approved_by,
        audit: vec![ApprovalAuditEvent {
            approval_id: dry_run.approval_id.clone(),
            swarm_id: dry_run.swarm_id,
            at,
            status,
            message,
        }],
    }
}

fn action_kind(action: &CoordinatedAction) -> &'static str {
    match action {
        CoordinatedAction::SynchronizedSurvey { .. } => "synchronized_survey",
        CoordinatedAction::PatternSearch { .. } => "pattern_search",
        CoordinatedAction::CoverageOptimization { .. } => "coverage_optimization",
        CoordinatedAction::DataCollection { .. } => "data_collection",
    }
}

fn approval_id(swarm_id: Uuid, action_kind: &str, checked_at: DateTime<Utc>) -> String {
    format!(
        "approval:{swarm_id}:{action_kind}:{}",
        checked_at.timestamp_millis()
    )
}

fn predicted_min_lane_separation(lanes: &[crate::SurveyLane]) -> Option<f64> {
    let mut min_distance: Option<f64> = None;
    for left_index in 0..lanes.len() {
        for right_index in (left_index + 1)..lanes.len() {
            let left = &lanes[left_index];
            let right = &lanes[right_index];
            let start_distance = distance_3d(
                (left.start_xy.0, left.start_xy.1, left.planned_altitude_m),
                (right.start_xy.0, right.start_xy.1, right.planned_altitude_m),
            );
            let end_distance = distance_3d(
                (left.end_xy.0, left.end_xy.1, left.planned_altitude_m),
                (right.end_xy.0, right.end_xy.1, right.planned_altitude_m),
            );
            let lane_distance = start_distance.min(end_distance);
            min_distance =
                Some(min_distance.map_or(lane_distance, |current| current.min(lane_distance)));
        }
    }
    min_distance
}

fn survey_error_to_gate_error(error: SynchronizedSurveyError) -> ApprovalGateError {
    ApprovalGateError::SafetyRejected {
        reason: error.to_string(),
    }
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
    use crate::{swarm::FormationType, CoordinatedAction, DroneSwarm, MultiDroneController};
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn controller_with_active_swarm() -> (MultiDroneController, Uuid) {
        let drone_ids = vec![Uuid::from_u128(101), Uuid::from_u128(102)];
        let mut controller = MultiDroneController::new("approval coordinator".to_string());
        let mut swarm = DroneSwarm::new_owned(
            "approval swarm".to_string(),
            drone_ids,
            FormationType::Line,
            "ops-team".to_string(),
        );
        swarm.status = crate::swarm::SwarmStatus::Active;
        let swarm_id = swarm.id;
        controller.register_swarm(swarm).unwrap();
        (controller, swarm_id)
    }

    fn survey_action() -> CoordinatedAction {
        CoordinatedAction::SynchronizedSurvey {
            area: vec![
                (0.0, 0.0),
                (120.0, 0.0),
                (120.0, 80.0),
                (0.0, 80.0),
                (0.0, 0.0),
            ],
            overlap_percent: 10.0,
        }
    }

    #[test]
    fn coordinated_action_dry_run_waits_for_approval_and_blocks_unapproved_execution() {
        let (controller, swarm_id) = controller_with_active_swarm();
        let checked_at = Utc.timestamp_opt(1_800_000_100, 0).unwrap();

        let dry_run = dry_run_coordinated_execution(
            &controller,
            swarm_id,
            survey_action(),
            ApprovalGateConfig::default(),
            checked_at,
        )
        .expect("dry run should validate");

        assert_eq!(
            dry_run.status,
            CoordinatedExecutionStatus::WaitingForApproval
        );
        assert!(dry_run.predicted_min_separation_m >= 25.0);
        assert!(dry_run.audit[0]
            .message
            .contains("waiting for operator approval"));

        let decision = authorize_coordinated_execution(&dry_run, None, checked_at);

        assert!(!decision.permitted);
        assert_eq!(decision.status, CoordinatedExecutionStatus::Blocked);
        assert!(decision.audit[0].message.contains("blocked"));
    }

    #[test]
    fn explicit_operator_approval_permits_coordinated_execution_and_audits() {
        let (controller, swarm_id) = controller_with_active_swarm();
        let checked_at = Utc.timestamp_opt(1_800_000_100, 0).unwrap();
        let dry_run = dry_run_coordinated_execution(
            &controller,
            swarm_id,
            survey_action(),
            ApprovalGateConfig::default(),
            checked_at,
        )
        .expect("dry run should validate");

        let decision = authorize_coordinated_execution(
            &dry_run,
            Some(OperatorApproval {
                approved: true,
                approved_by: "operator-1".to_string(),
                approved_at: Utc.timestamp_opt(1_800_000_130, 0).unwrap(),
            }),
            Utc.timestamp_opt(1_800_000_130, 0).unwrap(),
        );

        assert!(decision.permitted);
        assert_eq!(decision.status, CoordinatedExecutionStatus::Approved);
        assert!(decision.audit[0].message.contains("operator-1"));
    }
}
