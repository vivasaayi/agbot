use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::control_plane::{MembershipRole, TenantPrincipal};
use std::{str::FromStr, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

pub type SharedOperatorActionState = Arc<RwLock<OperatorActionState>>;
pub type SharedMissionControlActionClient = Arc<dyn MissionControlActionClient>;

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
}
