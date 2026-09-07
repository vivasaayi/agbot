//! Governed-dispatch advisory (Track E phase E4).
//!
//! A read-only, operator-facing view of the field-intelligence dispatch chain:
//! proposal -> accept/reject -> mission draft -> distinct-operator approval ->
//! governed dispatch. This module is *advisory only*. It presents the chain's
//! state and, crucially, the separation-of-duties step still required — it never
//! itself authorizes or performs a dispatch. Authorization lives in geo_hub's
//! governed-dispatch gate; the operator console only reflects it.
//!
//! The advisory is decoupled from geo_hub: the caller populates
//! [`ProposalDispatchInput`] from geo_hub API responses, and the builder derives
//! the operator-facing stage and flags. `dispatch_authorized` is never asserted
//! by this module beyond echoing an already-authorized upstream state, and any
//! accepted-but-not-yet-dispatched proposal is flagged as still requiring a
//! *distinct* operator.

use serde::{Deserialize, Serialize};

/// An operator's recorded decision on a mission draft, as reported by geo_hub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorDecisionView {
    pub operator_id: String,
    pub approved: bool,
}

/// The facts the console has about one proposal's dispatch chain. Populated from
/// geo_hub API responses (proposal queue + mission draft + approval).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalDispatchInput {
    pub proposal_id: String,
    pub field_id: String,
    pub title: String,
    /// "proposed" | "accepted" | "rejected".
    pub proposal_status: String,
    /// The reviewer who accepted the proposal, when accepted.
    pub accepted_by: Option<String>,
    /// Whether the accepted action maps to a flyable mission (a draft exists).
    pub action_flyable: bool,
    /// The operator's sign-off on the mission draft, when one has been recorded.
    pub operator_decision: Option<OperatorDecisionView>,
}

/// The operator-facing stage of a proposal in the dispatch chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchAdvisoryStage {
    /// Still in the review queue, undecided.
    AwaitingReview,
    /// Rejected in review; will not fly.
    Rejected,
    /// Accepted but not a flyable action (desk work); no dispatch path.
    AcceptedNoMission,
    /// Accepted and flyable, awaiting a distinct operator's approval.
    AwaitingOperatorApproval,
    /// A distinct operator approved; dispatch is authorized.
    DispatchAuthorized,
    /// The recorded operator is the same identity that accepted — blocked by
    /// separation of duties until a different operator signs off.
    BlockedOperatorConflict,
}

/// The read-only advisory a console renders for one proposal's dispatch chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedDispatchAdvisory {
    pub advisory_id: String,
    pub proposal_id: String,
    pub field_id: String,
    pub title: String,
    pub stage: DispatchAdvisoryStage,
    pub accepted_by: Option<String>,
    /// True while a distinct operator sign-off is still required before dispatch.
    pub requires_distinct_operator: bool,
    /// Echoes the upstream authorization state; only ever true when a distinct
    /// operator has affirmatively approved.
    pub dispatch_authorized: bool,
    pub summary: String,
}

/// Derive the operator-facing advisory for a proposal's dispatch chain. Pure and
/// deterministic; presents state only.
pub fn build_dispatch_advisory(input: &ProposalDispatchInput) -> GovernedDispatchAdvisory {
    let (stage, requires_distinct_operator, dispatch_authorized, summary) = classify(input);

    GovernedDispatchAdvisory {
        advisory_id: format!("dispatch-advisory:{}", input.proposal_id),
        proposal_id: input.proposal_id.clone(),
        field_id: input.field_id.clone(),
        title: input.title.clone(),
        stage,
        accepted_by: input.accepted_by.clone(),
        requires_distinct_operator,
        dispatch_authorized,
        summary: summary.to_string(),
    }
}

fn classify(input: &ProposalDispatchInput) -> (DispatchAdvisoryStage, bool, bool, &'static str) {
    match input.proposal_status.as_str() {
        "rejected" => (
            DispatchAdvisoryStage::Rejected,
            false,
            false,
            "Rejected in review; will not be dispatched.",
        ),
        "accepted" => classify_accepted(input),
        // "proposed" and anything unrecognized are treated as still in review.
        _ => (
            DispatchAdvisoryStage::AwaitingReview,
            false,
            false,
            "Awaiting review decision.",
        ),
    }
}

fn classify_accepted(
    input: &ProposalDispatchInput,
) -> (DispatchAdvisoryStage, bool, bool, &'static str) {
    if !input.action_flyable {
        return (
            DispatchAdvisoryStage::AcceptedNoMission,
            false,
            false,
            "Accepted, but the action is not a flyable mission.",
        );
    }

    match &input.operator_decision {
        // Separation of duties: the operator cannot be the accepting reviewer.
        Some(decision) if same_identity(decision, input) => (
            DispatchAdvisoryStage::BlockedOperatorConflict,
            true,
            false,
            "Blocked: dispatch operator must differ from the reviewer who accepted.",
        ),
        Some(decision) if decision.approved => (
            DispatchAdvisoryStage::DispatchAuthorized,
            false,
            true,
            "Approved by a distinct operator; dispatch is authorized.",
        ),
        Some(_) => (
            DispatchAdvisoryStage::AwaitingOperatorApproval,
            true,
            false,
            "Operator declined; a distinct operator approval is still required.",
        ),
        None => (
            DispatchAdvisoryStage::AwaitingOperatorApproval,
            true,
            false,
            "Accepted; awaiting a distinct operator's dispatch approval.",
        ),
    }
}

fn same_identity(decision: &OperatorDecisionView, input: &ProposalDispatchInput) -> bool {
    input
        .accepted_by
        .as_deref()
        .is_some_and(|reviewer| reviewer == decision.operator_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accepted_flyable() -> ProposalDispatchInput {
        ProposalDispatchInput {
            proposal_id: "proposal:finding:1".to_string(),
            field_id: "field-1".to_string(),
            title: "Scout pest hotspot".to_string(),
            proposal_status: "accepted".to_string(),
            accepted_by: Some("agronomist-1".to_string()),
            action_flyable: true,
            operator_decision: None,
        }
    }

    #[test]
    fn proposed_is_awaiting_review() {
        let advisory = build_dispatch_advisory(&ProposalDispatchInput {
            proposal_status: "proposed".to_string(),
            accepted_by: None,
            ..accepted_flyable()
        });
        assert_eq!(advisory.stage, DispatchAdvisoryStage::AwaitingReview);
        assert!(!advisory.dispatch_authorized);
    }

    #[test]
    fn accepted_flyable_awaits_distinct_operator() {
        let advisory = build_dispatch_advisory(&accepted_flyable());
        assert_eq!(
            advisory.stage,
            DispatchAdvisoryStage::AwaitingOperatorApproval
        );
        assert!(advisory.requires_distinct_operator);
        assert!(!advisory.dispatch_authorized);
    }

    #[test]
    fn distinct_operator_approval_shows_authorized() {
        let advisory = build_dispatch_advisory(&ProposalDispatchInput {
            operator_decision: Some(OperatorDecisionView {
                operator_id: "operator-9".to_string(),
                approved: true,
            }),
            ..accepted_flyable()
        });
        assert_eq!(advisory.stage, DispatchAdvisoryStage::DispatchAuthorized);
        assert!(advisory.dispatch_authorized);
        assert!(!advisory.requires_distinct_operator);
    }

    #[test]
    fn reviewer_operating_is_blocked_by_separation_of_duties() {
        let advisory = build_dispatch_advisory(&ProposalDispatchInput {
            operator_decision: Some(OperatorDecisionView {
                operator_id: "agronomist-1".to_string(), // same as accepted_by
                approved: true,
            }),
            ..accepted_flyable()
        });
        assert_eq!(
            advisory.stage,
            DispatchAdvisoryStage::BlockedOperatorConflict
        );
        assert!(advisory.requires_distinct_operator);
        // Never authorized despite an "approved" self-sign-off.
        assert!(!advisory.dispatch_authorized);
    }

    #[test]
    fn accepted_desk_work_has_no_mission() {
        let advisory = build_dispatch_advisory(&ProposalDispatchInput {
            action_flyable: false,
            ..accepted_flyable()
        });
        assert_eq!(advisory.stage, DispatchAdvisoryStage::AcceptedNoMission);
        assert!(!advisory.requires_distinct_operator);
    }

    #[test]
    fn rejected_will_not_dispatch() {
        let advisory = build_dispatch_advisory(&ProposalDispatchInput {
            proposal_status: "rejected".to_string(),
            ..accepted_flyable()
        });
        assert_eq!(advisory.stage, DispatchAdvisoryStage::Rejected);
        assert!(!advisory.dispatch_authorized);
    }
}
