//! Proposal adapters (Track D phase D4).
//!
//! The unified proposal queue ([`crate::proposal_queue`]) is source-agnostic: it
//! only needs a lineage-tracked source id and some display metadata. These
//! adapters funnel the pipeline's two existing draft producers into it:
//!
//! * the deterministic copilot advisor rules ([`copilot::advisor_rules`]), and
//! * the crop closed-loop proposals ([`crop_intelligence::CropClosedLoopProposal`]).
//!
//! Both map onto a `finding`-sourced proposal, so a backward trace from the
//! queued proposal closes through the source finding to L0. An accepted proposal
//! then materializes as a [`RecommendationRecord`] authored by the advisor, which
//! is the durable, operator-visible outcome of the loop.

use crate::db::DbPool;
use crate::proposal_queue::{
    self, Proposal, ProposalCreateRequest, ProposalError, ProposalSourceKind,
};
use copilot::advisor_rules::AdvisorProposal;
use crop_intelligence::{CropClosedLoopAction, CropClosedLoopProposal};
use shared::schemas::{RecommendationPriority, RecommendationRecord, RecommendationStatus};

/// Author id stamped on recommendations that a copilot/closed-loop proposal
/// produced once accepted.
pub const ADVISOR_AUTHOR: &str = "copilot-advisor";

fn recommendation_priority(priority: &str) -> RecommendationPriority {
    match priority {
        "critical" => RecommendationPriority::Critical,
        "high" => RecommendationPriority::High,
        "low" => RecommendationPriority::Low,
        _ => RecommendationPriority::Medium,
    }
}

/// The proposal-queue `action_category` for a closed-loop action.
fn closed_loop_action_category(action: CropClosedLoopAction) -> &'static str {
    match action {
        CropClosedLoopAction::Refly => "refly",
        CropClosedLoopAction::Treatment => "treatment",
        CropClosedLoopAction::ReflyAndTreatment => "refly_and_treatment",
    }
}

/// Map a copilot advisor draft into a queue create-request. The advisor's source
/// finding becomes the proposal's lineage source.
pub fn advisor_proposal_request(draft: &AdvisorProposal) -> ProposalCreateRequest {
    ProposalCreateRequest {
        source_kind: ProposalSourceKind::Finding,
        source_id: draft.source_finding_id.clone(),
        field_id: draft.field_id.clone(),
        title: draft.title.clone(),
        action_category: draft.remedy.action_category().to_string(),
        priority: draft.priority.clone(),
        rationale: Some(draft.rationale.clone()),
    }
}

/// Map a crop closed-loop proposal into a queue create-request. The first finding
/// evidence is the lineage source; a proposal with no findings cannot be queued
/// (there would be nothing to trace to), so this returns `None`.
pub fn crop_closed_loop_request(
    proposal: &CropClosedLoopProposal,
) -> Option<ProposalCreateRequest> {
    let source = proposal.findings.first()?;
    let priority = if proposal.confidence_floor >= 0.9 {
        "high"
    } else {
        "medium"
    };
    Some(ProposalCreateRequest {
        source_kind: ProposalSourceKind::Finding,
        source_id: source.finding_id.clone(),
        field_id: Some(proposal.field_id.clone()),
        title: format!(
            "Closed-loop {} for field {}",
            closed_loop_action_category(proposal.action),
            proposal.field_id
        ),
        action_category: closed_loop_action_category(proposal.action).to_string(),
        priority: priority.to_string(),
        rationale: Some(format!(
            "Requested by {} at confidence floor {:.2}.",
            proposal.requested_by, proposal.confidence_floor
        )),
    })
}

/// Funnel a copilot advisor draft into the unified queue. Idempotent per source
/// finding (delegates to [`proposal_queue::create_proposal`]).
pub async fn queue_advisor_proposal(
    pool: &DbPool,
    draft: &AdvisorProposal,
    created_at: &str,
) -> Result<Proposal, ProposalError> {
    proposal_queue::create_proposal(pool, &advisor_proposal_request(draft), created_at).await
}

/// Funnel a crop closed-loop proposal into the unified queue. Returns `None` when
/// the proposal carries no finding evidence to source lineage from.
pub async fn queue_crop_closed_loop_proposal(
    pool: &DbPool,
    proposal: &CropClosedLoopProposal,
    created_at: &str,
) -> Result<Option<Proposal>, ProposalError> {
    let Some(request) = crop_closed_loop_request(proposal) else {
        return Ok(None);
    };
    proposal_queue::create_proposal(pool, &request, created_at)
        .await
        .map(Some)
}

/// Materialize an accepted proposal as a recommendation draft authored by the
/// advisor. The proposal's source is carried as evidence so the recommendation
/// stays traceable back to the finding. Pure: persistence is the caller's job.
pub fn recommendation_draft_from_accepted(
    proposal: &Proposal,
    scene_id: &str,
    at: &str,
) -> RecommendationRecord {
    RecommendationRecord {
        recommendation_id: format!("rec:{}", proposal.proposal_id),
        scene_id: scene_id.to_string(),
        field_id: proposal.field_id.clone(),
        org_id: shared::schemas::DEFAULT_RECORD_OWNER.to_string(),
        author_user_id: ADVISOR_AUTHOR.to_string(),
        title: proposal.title.clone(),
        note: proposal.rationale.clone(),
        category: Some(proposal.action_category.clone()),
        action_category: proposal.action_category.clone(),
        priority: recommendation_priority(&proposal.priority),
        status: RecommendationStatus::Open,
        evidence_refs: vec![proposal.source_id.clone()],
        annotation_ids: Vec::new(),
        created_at: at.to_string(),
        updated_at: at.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copilot::advisor_rules::RemedyKind;
    use crop_intelligence::{
        CropClosedLoopFindingEvidence, CropModelTask, DetectionZoneGeometry,
    };
    use shared::schemas::GeoBounds;

    fn zone_geometry() -> DetectionZoneGeometry {
        DetectionZoneGeometry {
            crs: "EPSG:4326".to_string(),
            bbox: GeoBounds {
                min_lon: 0.0,
                min_lat: 0.0,
                max_lon: 1.0,
                max_lat: 1.0,
            },
        }
    }

    fn advisor_draft() -> AdvisorProposal {
        AdvisorProposal {
            source_finding_id: "finding:1".to_string(),
            field_id: Some("field-1".to_string()),
            remedy: RemedyKind::IrrigationCheck,
            title: "Irrigation check".to_string(),
            priority: "high".to_string(),
            rationale: "Declining NDVI.".to_string(),
        }
    }

    #[test]
    fn advisor_request_sources_from_finding() {
        let req = advisor_proposal_request(&advisor_draft());
        assert_eq!(req.source_kind, ProposalSourceKind::Finding);
        assert_eq!(req.source_id, "finding:1");
        assert_eq!(req.action_category, "irrigation");
        assert_eq!(req.field_id.as_deref(), Some("field-1"));
    }

    #[test]
    fn closed_loop_without_findings_is_not_queueable() {
        let proposal = CropClosedLoopProposal {
            proposal_id: "clp:1".to_string(),
            action: CropClosedLoopAction::Treatment,
            field_id: "field-1".to_string(),
            requested_by: "agronomist".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            approval_status: crop_intelligence::CropClosedLoopApprovalStatus::Pending,
            approval_required: true,
            dispatch_authorized: false,
            confidence_floor: 0.95,
            refly_area: None,
            treatment_prescription_ref: None,
            findings: Vec::new(),
            evidence_refs: Vec::new(),
        };
        assert!(crop_closed_loop_request(&proposal).is_none());
    }

    #[test]
    fn closed_loop_with_findings_sources_from_first_finding() {
        let evidence = CropClosedLoopFindingEvidence {
            finding_id: "finding:9".to_string(),
            finding_type: CropModelTask::PestDetection,
            zone_id: None,
            detection_id: "det:1".to_string(),
            confidence: 0.95,
            evidence_refs: vec!["tile:1".to_string()],
            zone_geometry: zone_geometry(),
        };
        let proposal = CropClosedLoopProposal {
            proposal_id: "clp:1".to_string(),
            action: CropClosedLoopAction::ReflyAndTreatment,
            field_id: "field-1".to_string(),
            requested_by: "agronomist".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            approval_status: crop_intelligence::CropClosedLoopApprovalStatus::Pending,
            approval_required: true,
            dispatch_authorized: false,
            confidence_floor: 0.95,
            refly_area: None,
            treatment_prescription_ref: None,
            findings: vec![evidence],
            evidence_refs: Vec::new(),
        };
        let req = crop_closed_loop_request(&proposal).expect("has findings");
        assert_eq!(req.source_id, "finding:9");
        assert_eq!(req.action_category, "refly_and_treatment");
        assert_eq!(req.priority, "high");
    }

    #[test]
    fn accepted_proposal_becomes_advisor_recommendation() {
        let proposal = Proposal {
            proposal_id: "proposal:finding:1".to_string(),
            source_kind: ProposalSourceKind::Finding,
            source_id: "finding:1".to_string(),
            field_id: Some("field-1".to_string()),
            title: "Irrigation check".to_string(),
            action_category: "irrigation".to_string(),
            priority: "high".to_string(),
            status: proposal_queue::ProposalStatus::Accepted,
            rationale: Some("Declining NDVI.".to_string()),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            reviewed_by: Some("operator-1".to_string()),
            reviewed_at: Some("2026-01-02T00:00:00Z".to_string()),
        };
        let rec = recommendation_draft_from_accepted(&proposal, "scene-1", "2026-01-02T00:00:00Z");
        assert_eq!(rec.author_user_id, ADVISOR_AUTHOR);
        assert_eq!(rec.priority, RecommendationPriority::High);
        assert_eq!(rec.evidence_refs, vec!["finding:1".to_string()]);
        assert_eq!(rec.scene_id, "scene-1");
    }
}
