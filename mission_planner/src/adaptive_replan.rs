use crate::{
    evaluate_dispatch_safety_with_constraints, AirspaceConstraint, DispatchSafetyConfig,
    DispatchSafetyReport, Mission, NoFlyZone, WeatherData,
};
use chrono::{DateTime, Utc};
use geo::Point;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdaptiveReplanStatus {
    ProposalReady,
    NoSafeReplan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdaptiveReplanErrorCode {
    MissionMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveReplanError {
    pub code: AdaptiveReplanErrorCode,
    pub message: String,
}

impl fmt::Display for AdaptiveReplanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for AdaptiveReplanError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveReplanRequest {
    pub mission: Mission,
    pub candidate_mission: Option<Mission>,
    pub changed_constraint: String,
    pub current_position: Option<Point<f64>>,
    #[serde(default)]
    pub no_fly_zones: Vec<NoFlyZone>,
    pub weather: Option<WeatherData>,
    #[serde(default)]
    pub airspace_constraints: Vec<AirspaceConstraint>,
    pub safety_config: DispatchSafetyConfig,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveReplanProposal {
    pub mission: Mission,
    pub safety_report: DispatchSafetyReport,
    pub requires_operator_confirmation: bool,
    pub applied: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveReplanAuditEvent {
    pub mission_id: uuid::Uuid,
    pub changed_constraint: String,
    pub status: AdaptiveReplanStatus,
    pub evaluated_at: DateTime<Utc>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveReplanOutcome {
    pub mission_id: uuid::Uuid,
    pub status: AdaptiveReplanStatus,
    pub proposal: Option<AdaptiveReplanProposal>,
    pub rejection_report: Option<DispatchSafetyReport>,
    pub failsafe_required: bool,
    pub audit: AdaptiveReplanAuditEvent,
}

pub fn evaluate_adaptive_replan(
    request: AdaptiveReplanRequest,
) -> Result<AdaptiveReplanOutcome, AdaptiveReplanError> {
    let mission_id = request.mission.id;
    let Some(candidate) = request.candidate_mission else {
        let report = DispatchSafetyReport {
            violations: Vec::new(),
        };
        return Ok(no_safe_replan(
            mission_id,
            request.changed_constraint,
            request.evaluated_at,
            report,
            "no candidate re-plan was available".to_string(),
        ));
    };
    if candidate.id != mission_id {
        return Err(AdaptiveReplanError {
            code: AdaptiveReplanErrorCode::MissionMismatch,
            message: format!(
                "candidate mission {} does not match active mission {}",
                candidate.id, mission_id
            ),
        });
    }

    let safety_report = evaluate_dispatch_safety_with_constraints(
        &candidate,
        request.current_position,
        &request.no_fly_zones,
        request.weather.as_ref(),
        &request.airspace_constraints,
        request.safety_config,
    );
    if !safety_report.is_clear() {
        return Ok(no_safe_replan(
            mission_id,
            request.changed_constraint,
            request.evaluated_at,
            safety_report,
            "candidate re-plan failed safety validation; failsafe is required".to_string(),
        ));
    }

    let audit = AdaptiveReplanAuditEvent {
        mission_id,
        changed_constraint: request.changed_constraint,
        status: AdaptiveReplanStatus::ProposalReady,
        evaluated_at: request.evaluated_at,
        message: "safe adaptive re-plan proposal generated; awaiting operator confirmation"
            .to_string(),
    };
    Ok(AdaptiveReplanOutcome {
        mission_id,
        status: AdaptiveReplanStatus::ProposalReady,
        proposal: Some(AdaptiveReplanProposal {
            mission: candidate,
            safety_report,
            requires_operator_confirmation: true,
            applied: false,
        }),
        rejection_report: None,
        failsafe_required: false,
        audit,
    })
}

fn no_safe_replan(
    mission_id: uuid::Uuid,
    changed_constraint: String,
    evaluated_at: DateTime<Utc>,
    report: DispatchSafetyReport,
    message: String,
) -> AdaptiveReplanOutcome {
    AdaptiveReplanOutcome {
        mission_id,
        status: AdaptiveReplanStatus::NoSafeReplan,
        proposal: None,
        rejection_report: Some(report),
        failsafe_required: true,
        audit: AdaptiveReplanAuditEvent {
            mission_id,
            changed_constraint,
            status: AdaptiveReplanStatus::NoSafeReplan,
            evaluated_at,
            message,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MissionStatus, Waypoint, WaypointType, WeatherConstraints};
    use chrono::{TimeZone, Utc};
    use geo::{point, polygon};
    use uuid::Uuid;

    #[test]
    fn adaptive_replan_generates_operator_confirmed_proposal_around_new_no_fly_zone() {
        let mission = mission_with_leg(0.0, 0.0, 10.0, 0.0);
        let candidate = mission_with_id_and_leg(mission.id, 0.0, 0.0, 10.0, 8.0);
        let no_fly_zone = NoFlyZone {
            id: "nfz-midfield".to_string(),
            boundary: polygon![
                (x: 4.0, y: -1.0),
                (x: 6.0, y: -1.0),
                (x: 6.0, y: 1.0),
                (x: 4.0, y: 1.0),
                (x: 4.0, y: -1.0),
            ],
        };

        let outcome = evaluate_adaptive_replan(AdaptiveReplanRequest {
            mission,
            candidate_mission: Some(candidate),
            changed_constraint: "new no-fly zone nfz-midfield".to_string(),
            current_position: Some(point!(x: 0.0, y: 0.0)),
            no_fly_zones: vec![no_fly_zone],
            weather: None,
            airspace_constraints: Vec::new(),
            safety_config: DispatchSafetyConfig {
                altitude_ceiling_m: 120.0,
            },
            evaluated_at: Utc.timestamp_opt(1_800_000_000, 0).unwrap(),
        })
        .expect("safe candidate should evaluate");

        assert_eq!(outcome.status, AdaptiveReplanStatus::ProposalReady);
        assert!(!outcome.failsafe_required);
        let proposal = outcome.proposal.expect("proposal");
        assert!(proposal.safety_report.is_clear());
        assert!(proposal.requires_operator_confirmation);
        assert!(!proposal.applied);
        assert_eq!(outcome.audit.status, AdaptiveReplanStatus::ProposalReady);
        assert!(outcome.audit.message.contains("operator confirmation"));
    }

    #[test]
    fn adaptive_replan_requires_failsafe_when_no_candidate_is_safe() {
        let mission = mission_with_leg(0.0, 0.0, 10.0, 0.0);
        let unsafe_candidate = mission_with_id_and_leg(mission.id, 0.0, 0.0, 10.0, 0.0);
        let no_fly_zone = NoFlyZone {
            id: "nfz-blocking".to_string(),
            boundary: polygon![
                (x: 4.0, y: -1.0),
                (x: 6.0, y: -1.0),
                (x: 6.0, y: 1.0),
                (x: 4.0, y: 1.0),
                (x: 4.0, y: -1.0),
            ],
        };

        let outcome = evaluate_adaptive_replan(AdaptiveReplanRequest {
            mission,
            candidate_mission: Some(unsafe_candidate),
            changed_constraint: "new no-fly zone nfz-blocking".to_string(),
            current_position: Some(point!(x: 0.0, y: 0.0)),
            no_fly_zones: vec![no_fly_zone],
            weather: None,
            airspace_constraints: Vec::new(),
            safety_config: DispatchSafetyConfig {
                altitude_ceiling_m: 120.0,
            },
            evaluated_at: Utc.timestamp_opt(1_800_000_030, 0).unwrap(),
        })
        .expect("unsafe candidate should return no-safe-replan outcome");

        assert_eq!(outcome.status, AdaptiveReplanStatus::NoSafeReplan);
        assert!(outcome.failsafe_required);
        assert!(outcome.proposal.is_none());
        let report = outcome.rejection_report.expect("rejection report");
        assert!(!report.is_clear());
        assert_eq!(
            report.violations[0].zone_id.as_deref(),
            Some("nfz-blocking")
        );
        assert_eq!(outcome.audit.status, AdaptiveReplanStatus::NoSafeReplan);
        assert!(outcome.audit.message.contains("failsafe"));
    }

    fn mission_with_leg(start_x: f64, start_y: f64, end_x: f64, end_y: f64) -> Mission {
        mission_with_id_and_leg(Uuid::new_v4(), start_x, start_y, end_x, end_y)
    }

    fn mission_with_id_and_leg(
        id: Uuid,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> Mission {
        Mission {
            id,
            name: "Adaptive Fixture".to_string(),
            description: "adaptive re-plan fixture".to_string(),
            created_at: Utc.timestamp_opt(1_800_000_000, 0).unwrap(),
            updated_at: Utc.timestamp_opt(1_800_000_000, 0).unwrap(),
            version: 1,
            field_id: "field-alpha".to_string(),
            season_id: "season-2026".to_string(),
            session_id: Some("session-alpha".to_string()),
            owner_id: "owner-alpha".to_string(),
            status: MissionStatus::InFlight,
            area_of_interest: polygon![
                (x: -1.0, y: -1.0),
                (x: 11.0, y: -1.0),
                (x: 11.0, y: 9.0),
                (x: -1.0, y: 9.0),
                (x: -1.0, y: -1.0),
            ],
            waypoints: vec![
                Waypoint {
                    id: Uuid::new_v4(),
                    position: point!(x: start_x, y: start_y),
                    altitude_m: 40.0,
                    waypoint_type: WaypointType::Takeoff,
                    actions: Vec::new(),
                    arrival_time: None,
                    speed_ms: Some(6.0),
                    heading_degrees: None,
                },
                Waypoint {
                    id: Uuid::new_v4(),
                    position: point!(x: end_x, y: end_y),
                    altitude_m: 40.0,
                    waypoint_type: WaypointType::Landing,
                    actions: Vec::new(),
                    arrival_time: None,
                    speed_ms: Some(6.0),
                    heading_degrees: None,
                },
            ],
            flight_paths: Vec::new(),
            estimated_duration_minutes: 6,
            estimated_battery_usage: 18.0,
            weather_constraints: WeatherConstraints {
                max_wind_speed_ms: 10.0,
                max_precipitation_mm: 0.0,
                min_visibility_m: 2_000.0,
                temperature_range_celsius: (5.0, 35.0),
            },
            metadata: Default::default(),
        }
    }
}
