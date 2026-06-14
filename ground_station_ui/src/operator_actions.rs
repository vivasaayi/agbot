use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::control_plane::{MembershipRole, TenantPrincipal};
use std::{str::FromStr, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

pub type SharedOperatorActionState = Arc<RwLock<OperatorActionState>>;
pub type SharedMissionControlActionClient = Arc<dyn MissionControlActionClient>;
pub type SharedOperatorActionAuditLog = Arc<RwLock<OperatorActionAuditLog>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperatorActionKind {
    Dispatch,
    Pause,
    ReturnToHome,
    Abort,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OperatorActionError {
    #[error("operator actions are disabled until the simulation loop is validated")]
    SimulationLoopNotValidated,
    #[error("unsupported operator action: {action}")]
    UnsupportedAction { action: String },
    #[error("mission_control rejected the action: {reason}")]
    MissionControlRejected { reason: String },
    #[error("mission_control did not acknowledge the action: {reason}")]
    MissionControlNoAck { reason: String },
    #[error("mission_control action path is unavailable: {reason}")]
    MissionControlUnavailable { reason: String },
    #[error("operator action audit write failed: {reason}")]
    AuditWriteFailed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorActionState {
    pub simulation_validated: bool,
    pub simulation_evidence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionControlActionRequest {
    pub operator_id: Uuid,
    pub org_id: Uuid,
    pub operator_role: MembershipRole,
    pub action: OperatorActionKind,
    pub target_mission_id: Uuid,
    pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionAckStatus {
    Accepted,
    Rejected,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionControlActionAck {
    pub ack_id: Uuid,
    pub action: OperatorActionKind,
    pub target_mission_id: Uuid,
    pub status: ActionAckStatus,
    pub message: String,
    pub guardrail_checked: bool,
    pub acked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorActionAuditResult {
    Accepted,
    Rejected,
    TimedOut,
    Disabled,
    Unsupported,
    Unavailable,
    AuditFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorActionAuditRecord {
    pub audit_id: Uuid,
    pub operator_id: Uuid,
    pub org_id: Uuid,
    pub operator_role: MembershipRole,
    pub action: OperatorActionKind,
    pub target_mission_id: Uuid,
    pub requested_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub result: OperatorActionAuditResult,
    pub ack: Option<MissionControlActionAck>,
    pub failure_reason: Option<String>,
}

pub trait MissionControlActionClient: Send + Sync {
    fn submit_operator_action(
        &self,
        request: MissionControlActionRequest,
    ) -> Result<MissionControlActionAck, OperatorActionError>;
}

#[derive(Debug, Clone, Default)]
pub struct RejectingMissionControlActionClient;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorActionSubmission {
    pub request: MissionControlActionRequest,
    pub ack: MissionControlActionAck,
}

#[derive(Debug, Clone, Default)]
pub struct OperatorActionAuditLog {
    records: Vec<OperatorActionAuditRecord>,
    fail_next_write: Option<String>,
}

impl FromStr for OperatorActionKind {
    type Err = OperatorActionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dispatch" => Ok(Self::Dispatch),
            "pause" => Ok(Self::Pause),
            "return-to-home" | "return_to_home" | "rth" => Ok(Self::ReturnToHome),
            "abort" => Ok(Self::Abort),
            action => Err(OperatorActionError::UnsupportedAction {
                action: action.to_string(),
            }),
        }
    }
}

impl OperatorActionState {
    pub fn mark_simulation_validated(&mut self, evidence: impl Into<String>) {
        self.simulation_validated = true;
        self.simulation_evidence = Some(evidence.into());
    }

    pub fn ensure_simulation_validated(&self) -> Result<(), OperatorActionError> {
        if self.simulation_validated {
            Ok(())
        } else {
            Err(OperatorActionError::SimulationLoopNotValidated)
        }
    }
}

impl Default for OperatorActionState {
    fn default() -> Self {
        Self {
            simulation_validated: false,
            simulation_evidence: None,
        }
    }
}

impl MissionControlActionRequest {
    pub fn new(
        principal: TenantPrincipal,
        action: OperatorActionKind,
        target_mission_id: Uuid,
        requested_at: DateTime<Utc>,
    ) -> Self {
        Self {
            operator_id: principal.user_id,
            org_id: principal.org_id,
            operator_role: principal.role,
            action,
            target_mission_id,
            requested_at,
        }
    }
}

impl MissionControlActionAck {
    pub fn accepted(
        action: OperatorActionKind,
        target_mission_id: Uuid,
        message: impl Into<String>,
        acked_at: DateTime<Utc>,
    ) -> Self {
        Self::new(
            action,
            target_mission_id,
            ActionAckStatus::Accepted,
            message,
            true,
            acked_at,
        )
    }

    pub fn rejected(
        action: OperatorActionKind,
        target_mission_id: Uuid,
        message: impl Into<String>,
        acked_at: DateTime<Utc>,
    ) -> Self {
        Self::new(
            action,
            target_mission_id,
            ActionAckStatus::Rejected,
            message,
            true,
            acked_at,
        )
    }

    pub fn timed_out(
        action: OperatorActionKind,
        target_mission_id: Uuid,
        message: impl Into<String>,
        acked_at: DateTime<Utc>,
    ) -> Self {
        Self::new(
            action,
            target_mission_id,
            ActionAckStatus::TimedOut,
            message,
            false,
            acked_at,
        )
    }

    fn new(
        action: OperatorActionKind,
        target_mission_id: Uuid,
        status: ActionAckStatus,
        message: impl Into<String>,
        guardrail_checked: bool,
        acked_at: DateTime<Utc>,
    ) -> Self {
        Self {
            ack_id: Uuid::new_v4(),
            action,
            target_mission_id,
            status,
            message: message.into(),
            guardrail_checked,
            acked_at,
        }
    }
}

impl OperatorActionAuditRecord {
    pub fn from_ack(request: &MissionControlActionRequest, ack: &MissionControlActionAck) -> Self {
        Self {
            audit_id: Uuid::new_v4(),
            operator_id: request.operator_id,
            org_id: request.org_id,
            operator_role: request.operator_role,
            action: request.action,
            target_mission_id: request.target_mission_id,
            requested_at: request.requested_at,
            completed_at: ack.acked_at,
            result: match ack.status {
                ActionAckStatus::Accepted => OperatorActionAuditResult::Accepted,
                ActionAckStatus::Rejected => OperatorActionAuditResult::Rejected,
                ActionAckStatus::TimedOut => OperatorActionAuditResult::TimedOut,
            },
            ack: Some(ack.clone()),
            failure_reason: None,
        }
    }

    pub fn from_error(
        request: &MissionControlActionRequest,
        error: &OperatorActionError,
        completed_at: DateTime<Utc>,
    ) -> Self {
        Self {
            audit_id: Uuid::new_v4(),
            operator_id: request.operator_id,
            org_id: request.org_id,
            operator_role: request.operator_role,
            action: request.action,
            target_mission_id: request.target_mission_id,
            requested_at: request.requested_at,
            completed_at,
            result: match error {
                OperatorActionError::SimulationLoopNotValidated => {
                    OperatorActionAuditResult::Disabled
                }
                OperatorActionError::UnsupportedAction { .. } => {
                    OperatorActionAuditResult::Unsupported
                }
                OperatorActionError::MissionControlRejected { .. } => {
                    OperatorActionAuditResult::Rejected
                }
                OperatorActionError::MissionControlNoAck { .. } => {
                    OperatorActionAuditResult::TimedOut
                }
                OperatorActionError::MissionControlUnavailable { .. } => {
                    OperatorActionAuditResult::Unavailable
                }
                OperatorActionError::AuditWriteFailed { .. } => {
                    OperatorActionAuditResult::AuditFailed
                }
            },
            ack: None,
            failure_reason: Some(error.to_string()),
        }
    }
}

impl OperatorActionAuditLog {
    pub fn record(&mut self, record: OperatorActionAuditRecord) -> Result<(), OperatorActionError> {
        if let Some(reason) = self.fail_next_write.take() {
            return Err(OperatorActionError::AuditWriteFailed { reason });
        }

        self.records.push(record);
        Ok(())
    }

    pub fn all_records(&self) -> Vec<OperatorActionAuditRecord> {
        self.records.clone()
    }

    pub fn records_for_org(&self, org_id: Uuid) -> Vec<OperatorActionAuditRecord> {
        self.records
            .iter()
            .filter(|record| record.org_id == org_id)
            .cloned()
            .collect()
    }

    pub fn fail_next_write(&mut self, reason: impl Into<String>) {
        self.fail_next_write = Some(reason.into());
    }
}

impl MissionControlActionClient for RejectingMissionControlActionClient {
    fn submit_operator_action(
        &self,
        _request: MissionControlActionRequest,
    ) -> Result<MissionControlActionAck, OperatorActionError> {
        Err(OperatorActionError::MissionControlUnavailable {
            reason: "no mission_control action client is configured".to_string(),
        })
    }
}

pub fn shared_operator_action_state(state: OperatorActionState) -> SharedOperatorActionState {
    Arc::new(RwLock::new(state))
}

pub fn shared_operator_action_audit_log(
    log: OperatorActionAuditLog,
) -> SharedOperatorActionAuditLog {
    Arc::new(RwLock::new(log))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use shared::control_plane::{MembershipRole, TenantPrincipal};
    use uuid::Uuid;

    #[test]
    fn supported_operator_actions_parse_from_api_paths() {
        assert_eq!(
            "dispatch".parse::<OperatorActionKind>().unwrap(),
            OperatorActionKind::Dispatch
        );
        assert_eq!(
            "pause".parse::<OperatorActionKind>().unwrap(),
            OperatorActionKind::Pause
        );
        assert_eq!(
            "return-to-home".parse::<OperatorActionKind>().unwrap(),
            OperatorActionKind::ReturnToHome
        );
        assert_eq!(
            "rth".parse::<OperatorActionKind>().unwrap(),
            OperatorActionKind::ReturnToHome
        );
        assert_eq!(
            "abort".parse::<OperatorActionKind>().unwrap(),
            OperatorActionKind::Abort
        );
        assert!("land-now".parse::<OperatorActionKind>().is_err());
    }

    #[test]
    fn simulation_gate_fails_closed_until_validated() {
        let mut gate = OperatorActionState::default();

        assert_eq!(
            gate.ensure_simulation_validated(),
            Err(OperatorActionError::SimulationLoopNotValidated)
        );

        gate.mark_simulation_validated("flight_sim_cpp:headless-regression");

        assert!(gate.ensure_simulation_validated().is_ok());
        assert_eq!(
            gate.simulation_evidence.as_deref(),
            Some("flight_sim_cpp:headless-regression")
        );
    }

    #[test]
    fn mission_control_request_carries_operator_identity_and_action() {
        let principal = TenantPrincipal {
            user_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            role: MembershipRole::Operator,
        };
        let mission_id = Uuid::new_v4();
        let requested_at = Utc.with_ymd_and_hms(2026, 6, 12, 16, 0, 0).unwrap();

        let request = MissionControlActionRequest::new(
            principal,
            OperatorActionKind::Abort,
            mission_id,
            requested_at,
        );

        assert_eq!(request.operator_id, principal.user_id);
        assert_eq!(request.org_id, principal.org_id);
        assert_eq!(request.operator_role, MembershipRole::Operator);
        assert_eq!(request.action, OperatorActionKind::Abort);
        assert_eq!(request.target_mission_id, mission_id);
        assert_eq!(request.requested_at, requested_at);
    }

    #[test]
    fn audit_record_captures_request_and_ack_evidence() {
        let principal = TenantPrincipal {
            user_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            role: MembershipRole::Operator,
        };
        let mission_id = Uuid::new_v4();
        let requested_at = Utc.with_ymd_and_hms(2026, 6, 12, 16, 5, 0).unwrap();
        let acked_at = Utc.with_ymd_and_hms(2026, 6, 12, 16, 5, 1).unwrap();
        let request = MissionControlActionRequest::new(
            principal,
            OperatorActionKind::Abort,
            mission_id,
            requested_at,
        );
        let ack = MissionControlActionAck::rejected(
            OperatorActionKind::Abort,
            mission_id,
            "guardrail blocked abort mode",
            acked_at,
        );

        let record = OperatorActionAuditRecord::from_ack(&request, &ack);

        assert_eq!(record.operator_id, principal.user_id);
        assert_eq!(record.org_id, principal.org_id);
        assert_eq!(record.operator_role, MembershipRole::Operator);
        assert_eq!(record.action, OperatorActionKind::Abort);
        assert_eq!(record.target_mission_id, mission_id);
        assert_eq!(record.requested_at, requested_at);
        assert_eq!(record.completed_at, acked_at);
        assert_eq!(record.result, OperatorActionAuditResult::Rejected);
        assert_eq!(record.ack, Some(ack));
        assert_eq!(record.failure_reason, None);
    }

    #[test]
    fn audit_log_appends_records_and_filters_by_org() {
        let org_a = Uuid::new_v4();
        let org_b = Uuid::new_v4();
        let record_a = OperatorActionAuditRecord::from_error(
            &MissionControlActionRequest {
                operator_id: Uuid::new_v4(),
                org_id: org_a,
                operator_role: MembershipRole::Operator,
                action: OperatorActionKind::Pause,
                target_mission_id: Uuid::new_v4(),
                requested_at: Utc.with_ymd_and_hms(2026, 6, 12, 16, 7, 0).unwrap(),
            },
            &OperatorActionError::MissionControlNoAck {
                reason: "ack deadline elapsed".to_string(),
            },
            Utc.with_ymd_and_hms(2026, 6, 12, 16, 7, 5).unwrap(),
        );
        let record_b = OperatorActionAuditRecord::from_error(
            &MissionControlActionRequest {
                operator_id: Uuid::new_v4(),
                org_id: org_b,
                operator_role: MembershipRole::Operator,
                action: OperatorActionKind::Dispatch,
                target_mission_id: Uuid::new_v4(),
                requested_at: Utc.with_ymd_and_hms(2026, 6, 12, 16, 8, 0).unwrap(),
            },
            &OperatorActionError::MissionControlUnavailable {
                reason: "bridge offline".to_string(),
            },
            Utc.with_ymd_and_hms(2026, 6, 12, 16, 8, 3).unwrap(),
        );
        let mut log = OperatorActionAuditLog::default();

        log.record(record_a.clone()).unwrap();
        log.record(record_b.clone()).unwrap();

        assert_eq!(log.all_records(), vec![record_a.clone(), record_b]);
        assert_eq!(log.records_for_org(org_a), vec![record_a]);
    }

    #[test]
    fn audit_log_write_failure_is_reason_coded() {
        let principal = TenantPrincipal {
            user_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            role: MembershipRole::Operator,
        };
        let mission_id = Uuid::new_v4();
        let requested_at = Utc.with_ymd_and_hms(2026, 6, 12, 16, 10, 0).unwrap();
        let request = MissionControlActionRequest::new(
            principal,
            OperatorActionKind::ReturnToHome,
            mission_id,
            requested_at,
        );
        let record = OperatorActionAuditRecord::from_error(
            &request,
            &OperatorActionError::MissionControlNoAck {
                reason: "ack deadline elapsed".to_string(),
            },
            requested_at,
        );
        let mut log = OperatorActionAuditLog::default();
        log.fail_next_write("audit storage unavailable");

        assert_eq!(
            log.record(record),
            Err(OperatorActionError::AuditWriteFailed {
                reason: "audit storage unavailable".to_string()
            })
        );
        assert!(log.all_records().is_empty());
    }
}
