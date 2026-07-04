//! Governed dispatch (Track E phase E3).
//!
//! The final gate between a governed approval and flight. geo_hub is the
//! governance authority, not the flight controller: it does not own mission or
//! telemetry state. So governed dispatch takes the flight inputs (mission,
//! command, context, ack tracker) from the caller and wraps them in two
//! non-negotiable guarantees before handing off to
//! `mission_planner::dispatch_guarded_simulation_command`:
//!
//! 1. **No dispatch without authorization.** The only thing that authorizes
//!    dispatch is a [`MissionDispatchApproval`] with `dispatch_authorized = true`,
//!    which the E2 gate produces only after a *distinct* operator approves. There
//!    is no auth-token, flag, or request field that can bypass this — authorization
//!    is carried by the approval value itself.
//! 2. **Every dispatch is on the ledger.** A successful dispatch appends an
//!    `Action` lineage record sourced from the proposal, so a backward trace from
//!    the action closes action -> proposal -> ... -> L0 gap-free.

use crate::db::DbPool;
use crate::proposal_mission::MissionDispatchApproval;
use crate::provenance_store::{self, ProvenanceStoreError};
use mission_planner::mavlink_integration::MAVLinkCommandAckTracker;
use mission_planner::{
    dispatch_guarded_simulation_command, GuardedDispatchCommand, GuardedDispatchContext,
    GuardedDispatchError, GuardedDispatchOutcome, Mission,
};
use provenance::{ActorIdentity, ArtifactKind, LineageRecord, ProvenanceParameters};
use serde_json::json;
use thiserror::Error;

const GOVERNED_DISPATCH_METHOD: &str = "governed_dispatch_v1";

#[derive(Debug, Error)]
pub enum GovernedDispatchError {
    #[error("dispatch for {draft_id} is not authorized; only an E2-approved, distinctly-operated approval may dispatch")]
    NotAuthorized { draft_id: String },
    #[error(transparent)]
    Dispatch(#[from] GuardedDispatchError),
    #[error(transparent)]
    Provenance(#[from] ProvenanceStoreError),
}

/// The lineage id an authorized dispatch records its `Action` under.
pub fn action_id_for(approval: &MissionDispatchApproval) -> String {
    format!("action:{}", approval.draft_id)
}

/// Refuse any approval that is not authorized for dispatch. Separated so the
/// no-bypass guarantee is unit-testable without constructing flight state.
pub fn assert_dispatch_authorized(
    approval: &MissionDispatchApproval,
) -> Result<(), GovernedDispatchError> {
    if !approval.dispatch_authorized {
        return Err(GovernedDispatchError::NotAuthorized {
            draft_id: approval.draft_id.clone(),
        });
    }
    Ok(())
}

/// Governed dispatch: refuse unless the approval authorizes it, invoke
/// `mission_planner::dispatch_guarded_simulation_command`, then record the
/// resulting flight `Action` on the provenance ledger sourced from the proposal.
///
/// The safety envelope (geofence, battery, link, abort path) is enforced *inside*
/// `guarded_dispatch`; this function adds the governance and provenance envelope
/// around it.
pub async fn governed_dispatch(
    pool: &DbPool,
    approval: &MissionDispatchApproval,
    mission: &Mission,
    command: GuardedDispatchCommand,
    context: GuardedDispatchContext,
    ack_tracker: &mut MAVLinkCommandAckTracker,
    at: &str,
) -> Result<GuardedDispatchOutcome, GovernedDispatchError> {
    assert_dispatch_authorized(approval)?;

    let outcome =
        dispatch_guarded_simulation_command(mission, command, context, ack_tracker)?;

    provenance_store::append_lineage(
        pool,
        &LineageRecord {
            artifact_id: action_id_for(approval),
            kind: ArtifactKind::Action,
            inputs: vec![approval.source_proposal_id.clone()],
            method: GOVERNED_DISPATCH_METHOD.to_string(),
            parameters: ProvenanceParameters::from_json(json!({
                "mission_kind": approval.mission_kind,
                "authorized_by": approval.authorized_by,
                "accepted_by": approval.accepted_by,
                "correlation_id": outcome.command.correlation_id,
            })),
            operator: "geo_hub:governed_dispatch".to_string(),
            actor: ActorIdentity::system("geo_hub:governed_dispatch"),
            created_at: at.to_string(),
        },
    )
    .await?;

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proposal_mission::{DispatchGateStatus, MissionKind};

    fn approval(dispatch_authorized: bool) -> MissionDispatchApproval {
        MissionDispatchApproval {
            draft_id: "mission-draft:proposal:finding:1".to_string(),
            source_proposal_id: "proposal:finding:1".to_string(),
            field_id: "field-1".to_string(),
            mission_kind: MissionKind::Scout,
            accepted_by: "agronomist-1".to_string(),
            status: if dispatch_authorized {
                DispatchGateStatus::Approved
            } else {
                DispatchGateStatus::Rejected
            },
            authorized_by: Some("operator-9".to_string()),
            dispatch_authorized,
        }
    }

    #[test]
    fn unauthorized_approval_is_refused() {
        let err = assert_dispatch_authorized(&approval(false)).unwrap_err();
        assert!(matches!(err, GovernedDispatchError::NotAuthorized { .. }));
    }

    #[test]
    fn authorized_approval_passes_the_guard() {
        assert!(assert_dispatch_authorized(&approval(true)).is_ok());
    }

    #[test]
    fn action_id_is_derived_from_the_draft() {
        assert_eq!(
            action_id_for(&approval(true)),
            "action:mission-draft:proposal:finding:1"
        );
    }
}
