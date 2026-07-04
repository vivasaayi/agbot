//! Proposal-to-mission draft (Track E phase E1).
//!
//! The bridge from the review queue to flight: an *accepted* proposal becomes a
//! deterministic mission-plan draft. A draft is inert — it describes the intended
//! mission and carries the reviewer who accepted the proposal, but it never
//! authorizes dispatch. Governed dispatch (E2/E3) requires a *distinct* operator
//! to dry-run and approve the draft, and only then does it reach
//! `mission_planner::guarded_dispatch`. A draft therefore always leaves
//! `dispatch_authorized = false`.

use crate::proposal_queue::{Proposal, ProposalStatus};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MissionDraftError {
    #[error("only an accepted proposal can be drafted into a mission (proposal {proposal_id} is {status})")]
    NotAccepted {
        proposal_id: String,
        status: &'static str,
    },
    #[error("accepted proposal {0} has no reviewer of record")]
    MissingReviewer(String),
    #[error("proposal {0} has no field to fly")]
    MissingField(String),
    #[error("proposal action_category {0} does not map to a flyable mission")]
    UnflyableAction(String),
}

/// The kind of mission an accepted proposal calls for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionKind {
    /// Re-fly the area to collect fresh imagery.
    Survey,
    /// Apply a treatment prescription over the area.
    Treatment,
    /// Fly a targeted scouting pattern to inspect a hotspot.
    Scout,
}

impl MissionKind {
    fn as_str(self) -> &'static str {
        match self {
            MissionKind::Survey => "survey",
            MissionKind::Treatment => "treatment",
            MissionKind::Scout => "scout",
        }
    }
}

/// Map a proposal's `action_category` to a flyable mission kind. Actions that are
/// desk work (irrigation checks, manual reviews) are intentionally not flyable.
fn mission_kind_for(action_category: &str) -> Option<MissionKind> {
    match action_category {
        "refly" | "survey" => Some(MissionKind::Survey),
        "treatment" => Some(MissionKind::Treatment),
        "refly_and_treatment" => Some(MissionKind::Treatment),
        "scout" | "scouting" => Some(MissionKind::Scout),
        _ => None,
    }
}

/// A deterministic, inert mission-plan draft derived from an accepted proposal.
/// It records provenance (`source_proposal_id`, `source_id`) and the accepting
/// `reviewed_by` so the governed-dispatch path can require a *distinct* operator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionPlanDraft {
    pub draft_id: String,
    pub source_proposal_id: String,
    /// The proposal's lineage source (finding / alert / recommendation).
    pub source_id: String,
    pub field_id: String,
    pub mission_kind: MissionKind,
    pub objective: String,
    pub priority: String,
    /// The reviewer who accepted the proposal. Dispatch must be operated by a
    /// different identity (enforced in E2).
    pub accepted_by: String,
    /// A draft never authorizes dispatch on its own.
    pub dispatch_authorized: bool,
}

/// Turn an accepted proposal into a mission-plan draft. Deterministic and inert:
/// no dispatch, no I/O. Fails unless the proposal is `Accepted`, has a reviewer,
/// a field, and a flyable action.
pub fn draft_mission_for_proposal(
    proposal: &Proposal,
) -> Result<MissionPlanDraft, MissionDraftError> {
    if proposal.status != ProposalStatus::Accepted {
        return Err(MissionDraftError::NotAccepted {
            proposal_id: proposal.proposal_id.clone(),
            status: status_str(proposal.status),
        });
    }
    let accepted_by = proposal
        .reviewed_by
        .clone()
        .ok_or_else(|| MissionDraftError::MissingReviewer(proposal.proposal_id.clone()))?;
    let field_id = proposal
        .field_id
        .clone()
        .ok_or_else(|| MissionDraftError::MissingField(proposal.proposal_id.clone()))?;
    let mission_kind = mission_kind_for(&proposal.action_category)
        .ok_or_else(|| MissionDraftError::UnflyableAction(proposal.action_category.clone()))?;

    Ok(MissionPlanDraft {
        draft_id: format!("mission-draft:{}", proposal.proposal_id),
        source_proposal_id: proposal.proposal_id.clone(),
        source_id: proposal.source_id.clone(),
        field_id,
        mission_kind,
        objective: format!("{} — {}", mission_kind.as_str(), proposal.title),
        priority: proposal.priority.clone(),
        accepted_by,
        dispatch_authorized: false,
    })
}

fn status_str(status: ProposalStatus) -> &'static str {
    match status {
        ProposalStatus::Proposed => "proposed",
        ProposalStatus::Accepted => "accepted",
        ProposalStatus::Rejected => "rejected",
    }
}

// ---------------------------------------------------------------------------
// Governed dispatch gate (Track E phase E2): dry-run + operator approval.
//
// Mirrors the two-step shape of multi_drone_control::coordinated_approval
// (dry-run -> operator decision) without coupling to a swarm controller. The
// governance invariant is separation of duties: the operator who authorizes
// dispatch must be a *different* identity from the reviewer who accepted the
// proposal. dispatch_authorized flips true here and *only* here, after a
// distinct, affirmative operator approval.
// ---------------------------------------------------------------------------

/// The gate's position awaiting operator sign-off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchGateStatus {
    WaitingForApproval,
    Approved,
    Rejected,
}

/// A deterministic dry-run of a mission draft: it restates the intended mission
/// and parks it awaiting a distinct operator's approval. Inert — no dispatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionDispatchDryRun {
    pub draft_id: String,
    pub source_proposal_id: String,
    pub field_id: String,
    pub mission_kind: MissionKind,
    /// The reviewer who accepted the proposal; the operator must differ.
    pub accepted_by: String,
    pub status: DispatchGateStatus,
}

/// An operator's sign-off on a dry-run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperatorApproval {
    pub operator_id: String,
    pub approved: bool,
    pub approved_at: String,
}

/// The outcome of the approval gate. `dispatch_authorized` is true only when a
/// distinct operator affirmatively approved a waiting dry-run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionDispatchApproval {
    pub draft_id: String,
    pub source_proposal_id: String,
    pub field_id: String,
    pub mission_kind: MissionKind,
    pub accepted_by: String,
    pub status: DispatchGateStatus,
    pub authorized_by: Option<String>,
    pub dispatch_authorized: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DispatchApprovalError {
    #[error("dispatch operator {operator_id} is the same identity that accepted the proposal; separation of duties requires a distinct operator")]
    OperatorIsReviewer { operator_id: String },
    #[error("dispatch dry-run for {draft_id} is not awaiting approval (status {status:?})")]
    NotAwaitingApproval {
        draft_id: String,
        status: DispatchGateStatus,
    },
}

/// Dry-run a mission draft into a gate record awaiting operator approval.
/// Deterministic and inert.
pub fn dry_run_mission_dispatch(draft: &MissionPlanDraft) -> MissionDispatchDryRun {
    MissionDispatchDryRun {
        draft_id: draft.draft_id.clone(),
        source_proposal_id: draft.source_proposal_id.clone(),
        field_id: draft.field_id.clone(),
        mission_kind: draft.mission_kind,
        accepted_by: draft.accepted_by.clone(),
        status: DispatchGateStatus::WaitingForApproval,
    }
}

/// Authorize dispatch of a dry-run. Enforces separation of duties (operator must
/// differ from the accepting reviewer) and requires an affirmative approval. Only
/// an approved, distinctly-operated dry-run yields `dispatch_authorized = true`;
/// a non-approving operator produces a `Rejected` gate that never authorizes.
pub fn authorize_mission_dispatch(
    dry_run: &MissionDispatchDryRun,
    operator: &OperatorApproval,
) -> Result<MissionDispatchApproval, DispatchApprovalError> {
    if dry_run.status != DispatchGateStatus::WaitingForApproval {
        return Err(DispatchApprovalError::NotAwaitingApproval {
            draft_id: dry_run.draft_id.clone(),
            status: dry_run.status,
        });
    }
    // Separation of duties: the operator cannot be the reviewer who accepted.
    if operator.operator_id == dry_run.accepted_by {
        return Err(DispatchApprovalError::OperatorIsReviewer {
            operator_id: operator.operator_id.clone(),
        });
    }

    let (status, dispatch_authorized) = if operator.approved {
        (DispatchGateStatus::Approved, true)
    } else {
        (DispatchGateStatus::Rejected, false)
    };

    Ok(MissionDispatchApproval {
        draft_id: dry_run.draft_id.clone(),
        source_proposal_id: dry_run.source_proposal_id.clone(),
        field_id: dry_run.field_id.clone(),
        mission_kind: dry_run.mission_kind,
        accepted_by: dry_run.accepted_by.clone(),
        status,
        authorized_by: Some(operator.operator_id.clone()),
        dispatch_authorized,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proposal_queue::ProposalSourceKind;

    fn proposal(status: ProposalStatus, action: &str) -> Proposal {
        Proposal {
            proposal_id: "proposal:finding:1".to_string(),
            source_kind: ProposalSourceKind::Finding,
            source_id: "finding:1".to_string(),
            field_id: Some("field-1".to_string()),
            title: "Scout pest hotspot".to_string(),
            action_category: action.to_string(),
            priority: "high".to_string(),
            status,
            rationale: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            reviewed_by: Some("agronomist-1".to_string()),
            reviewed_at: Some("2026-01-02T00:00:00Z".to_string()),
        }
    }

    #[test]
    fn accepted_scout_proposal_drafts_a_scout_mission() {
        let draft = draft_mission_for_proposal(&proposal(ProposalStatus::Accepted, "scout"))
            .expect("accepted + flyable");
        assert_eq!(draft.mission_kind, MissionKind::Scout);
        assert_eq!(draft.field_id, "field-1");
        assert_eq!(draft.source_id, "finding:1");
        assert_eq!(draft.accepted_by, "agronomist-1");
        // A draft never authorizes dispatch on its own.
        assert!(!draft.dispatch_authorized);
    }

    #[test]
    fn refly_and_treatment_maps_to_treatment_mission() {
        let draft =
            draft_mission_for_proposal(&proposal(ProposalStatus::Accepted, "refly_and_treatment"))
                .unwrap();
        assert_eq!(draft.mission_kind, MissionKind::Treatment);
    }

    #[test]
    fn proposed_proposal_cannot_be_drafted() {
        let err = draft_mission_for_proposal(&proposal(ProposalStatus::Proposed, "scout"))
            .unwrap_err();
        assert!(matches!(err, MissionDraftError::NotAccepted { .. }));
    }

    #[test]
    fn rejected_proposal_cannot_be_drafted() {
        let err = draft_mission_for_proposal(&proposal(ProposalStatus::Rejected, "scout"))
            .unwrap_err();
        assert!(matches!(err, MissionDraftError::NotAccepted { .. }));
    }

    #[test]
    fn desk_work_action_is_not_flyable() {
        // Irrigation checks are desk work, not a mission.
        let err = draft_mission_for_proposal(&proposal(ProposalStatus::Accepted, "irrigation"))
            .unwrap_err();
        assert!(matches!(err, MissionDraftError::UnflyableAction(_)));
    }

    fn accepted_draft() -> MissionPlanDraft {
        draft_mission_for_proposal(&proposal(ProposalStatus::Accepted, "scout")).unwrap()
    }

    #[test]
    fn distinct_operator_approval_authorizes_dispatch() {
        let dry_run = dry_run_mission_dispatch(&accepted_draft());
        let decision = authorize_mission_dispatch(
            &dry_run,
            &OperatorApproval {
                operator_id: "operator-9".to_string(), // distinct from reviewer agronomist-1
                approved: true,
                approved_at: "2026-01-03T00:00:00Z".to_string(),
            },
        )
        .expect("distinct operator approval");
        assert_eq!(decision.status, DispatchGateStatus::Approved);
        assert!(decision.dispatch_authorized);
        assert_eq!(decision.authorized_by.as_deref(), Some("operator-9"));
    }

    #[test]
    fn reviewer_cannot_also_operate_dispatch() {
        let dry_run = dry_run_mission_dispatch(&accepted_draft());
        // agronomist-1 accepted the proposal, so they cannot also authorize.
        let err = authorize_mission_dispatch(
            &dry_run,
            &OperatorApproval {
                operator_id: "agronomist-1".to_string(),
                approved: true,
                approved_at: "2026-01-03T00:00:00Z".to_string(),
            },
        )
        .unwrap_err();
        assert!(matches!(
            err,
            DispatchApprovalError::OperatorIsReviewer { .. }
        ));
    }

    #[test]
    fn non_approving_operator_does_not_authorize() {
        let dry_run = dry_run_mission_dispatch(&accepted_draft());
        let decision = authorize_mission_dispatch(
            &dry_run,
            &OperatorApproval {
                operator_id: "operator-9".to_string(),
                approved: false,
                approved_at: "2026-01-03T00:00:00Z".to_string(),
            },
        )
        .unwrap();
        assert_eq!(decision.status, DispatchGateStatus::Rejected);
        assert!(!decision.dispatch_authorized);
    }

    #[test]
    fn a_decided_dry_run_cannot_be_reauthorized() {
        let dry_run = MissionDispatchDryRun {
            status: DispatchGateStatus::Approved,
            ..dry_run_mission_dispatch(&accepted_draft())
        };
        let err = authorize_mission_dispatch(
            &dry_run,
            &OperatorApproval {
                operator_id: "operator-9".to_string(),
                approved: true,
                approved_at: "2026-01-03T00:00:00Z".to_string(),
            },
        )
        .unwrap_err();
        assert!(matches!(
            err,
            DispatchApprovalError::NotAwaitingApproval { .. }
        ));
    }
}
