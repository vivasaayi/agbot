//! Proposal -> mission plan generation (governed hand-off, plan step 2).
//!
//! Turns an approved agent/operator proposal into a *real*, waypoint-level survey
//! mission by reusing the survey-template generator over the field boundary. This
//! is the flight-planning half of the hand-off: geo_hub owns the governance draft
//! and approval gate (`geo_hub::proposal_mission` / `proposal_dispatch`), while
//! this module — living next to `survey_template.rs`/`flight_path.rs` — produces
//! the concrete `Mission` those gates ultimately dispatch. Pure and deterministic:
//! no I/O, no clock.

use crate::survey_template::{
    generate_survey_template, SurveyTemplateConfig, SurveyTemplateError, SurveyTemplateResult,
};
use crate::MissionLinkage;
use geo::Polygon;

/// The inputs that turn an approved proposal into a survey mission plan: the
/// proposal it originates from (for tagging), a human objective, the field
/// boundary to survey, the mission's field/season linkage, and the survey config
/// (pattern, spacing, altitude).
#[derive(Debug, Clone)]
pub struct ProposalMissionRequest {
    pub proposal_id: String,
    pub objective: String,
    pub boundary: Polygon<f64>,
    pub linkage: MissionLinkage,
    pub config: SurveyTemplateConfig,
}

/// The mission-name prefix that tags a plan with its originating proposal, so the
/// stored plan is traceable back to the proposal.
pub fn proposal_mission_name(proposal_id: &str) -> String {
    format!("proposal-mission:{proposal_id}")
}

/// Build a waypoint-level survey mission plan for an approved proposal by reusing
/// the survey-template generator over the field boundary. The resulting mission's
/// name embeds the proposal id so the collaboration plan stays tagged to its
/// origin. Validation (geofence, altitude ceiling, waypoint sanity) is enforced
/// inside `generate_survey_template`.
pub fn build_mission_plan_from_proposal(
    request: ProposalMissionRequest,
) -> Result<SurveyTemplateResult, SurveyTemplateError> {
    generate_survey_template(
        proposal_mission_name(&request.proposal_id),
        request.objective,
        request.boundary,
        request.linkage,
        request.config,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight_path::SurveyPattern;
    use crate::MissionStatus;
    use geo::polygon;

    fn field_boundary() -> Polygon<f64> {
        polygon![
            (x: 0.0, y: 0.0),
            (x: 100.0, y: 0.0),
            (x: 100.0, y: 60.0),
            (x: 0.0, y: 60.0),
            (x: 0.0, y: 0.0),
        ]
    }

    fn request(proposal_id: &str) -> ProposalMissionRequest {
        ProposalMissionRequest {
            proposal_id: proposal_id.to_string(),
            objective: "scout — Scout pest hotspot".to_string(),
            boundary: field_boundary(),
            linkage: MissionLinkage::new(
                "field-1".to_string(),
                "season-2026".to_string(),
                None,
                "owner-1".to_string(),
            ),
            config: SurveyTemplateConfig {
                pattern: SurveyPattern::Lawnmower,
                spacing_m: 20.0,
                overlap_percent: 0.0,
                altitude_m: 40.0,
                altitude_ceiling_m: 120.0,
                speed_ms: None,
            },
        }
    }

    #[test]
    fn proposal_becomes_a_real_waypoint_mission_tagged_to_its_origin() {
        let result = build_mission_plan_from_proposal(request("proposal:finding:1"))
            .expect("valid boundary + config plans a mission");
        // A concrete mission with survey waypoints, not an abstract draft.
        assert!(
            result.mission.waypoints.len() >= 2,
            "generated a waypoint plan"
        );
        assert!(result.leg_count >= 1);
        assert!(result.coverage_fraction > 0.0);
        // Tagged to the proposal so the stored plan traces back to its origin.
        assert_eq!(
            result.mission.name,
            "proposal-mission:proposal:finding:1"
        );
        assert_eq!(result.mission.field_id, "field-1");
    }

    #[test]
    fn generated_plan_is_dispatchable_after_validation() {
        let mut result = build_mission_plan_from_proposal(request("proposal:finding:2")).unwrap();
        // The survey template returns an already-validated plan; it can be armed
        // directly — the precondition guarded_dispatch requires.
        assert_eq!(result.mission.status, MissionStatus::Validated);
        result.mission.arm().expect("plan arms");
        assert_eq!(result.mission.status, MissionStatus::Armed);
    }

    #[test]
    fn distinct_proposals_tag_distinct_missions() {
        let a = build_mission_plan_from_proposal(request("proposal:a")).unwrap();
        let b = build_mission_plan_from_proposal(request("proposal:b")).unwrap();
        assert_ne!(a.mission.name, b.mission.name);
    }
}
