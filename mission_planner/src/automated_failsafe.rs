use crate::{
    evaluate_abort_recovery, AbortRecoveryConfig, AbortRecoveryContext, AbortRecoveryPlan,
    AbortTrigger, Mission, TelemetryFreshness, TelemetryLinkState,
};
use chrono::{DateTime, Utc};
use geo::Point;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomatedFailsafeConfig {
    pub link_loss_threshold_seconds: i64,
    pub critical_battery_percentage: u8,
    pub emergency_landing_sites: Vec<Point<f64>>,
    pub abort_recovery: AbortRecoveryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomatedFailsafeState {
    pub link_freshness: TelemetryFreshness,
    pub battery_percentage: u8,
    pub current_position: Point<f64>,
    pub home_position: Point<f64>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutomatedFailsafeTrigger {
    LinkLoss,
    CriticalBattery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutomatedFailsafeAuditEventKind {
    Clear,
    Triggered,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomatedFailsafeAuditEvent {
    pub mission_id: Uuid,
    pub trigger: Option<AutomatedFailsafeTrigger>,
    pub event: AutomatedFailsafeAuditEventKind,
    pub checked_at: DateTime<Utc>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomatedFailsafeEvaluation {
    pub mission_id: Uuid,
    pub trigger: Option<AutomatedFailsafeTrigger>,
    pub abort_plan: Option<AbortRecoveryPlan>,
    pub audit: Vec<AutomatedFailsafeAuditEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutomatedFailsafeError {
    MissingEmergencyLandingSite,
    InvalidLinkLossThreshold,
    LinkMissionMismatch { expected: Uuid, actual: Uuid },
    AbortPlan { reason: String },
}

impl fmt::Display for AutomatedFailsafeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEmergencyLandingSite => {
                formatter.write_str("automated failsafe requires an emergency landing site")
            }
            Self::InvalidLinkLossThreshold => {
                formatter.write_str("link loss threshold must be greater than zero seconds")
            }
            Self::LinkMissionMismatch { expected, actual } => write!(
                formatter,
                "failsafe telemetry mission mismatch: expected {expected}, got {actual}"
            ),
            Self::AbortPlan { reason } => write!(formatter, "abort planning failed: {reason}"),
        }
    }
}

impl std::error::Error for AutomatedFailsafeError {}

pub fn assert_failsafe_ready_for_arming(
    config: &AutomatedFailsafeConfig,
) -> Result<(), AutomatedFailsafeError> {
    if config.link_loss_threshold_seconds <= 0 {
        return Err(AutomatedFailsafeError::InvalidLinkLossThreshold);
    }
    if config.emergency_landing_sites.is_empty() {
        return Err(AutomatedFailsafeError::MissingEmergencyLandingSite);
    }
    Ok(())
}

pub fn evaluate_automated_failsafe(
    mission: &Mission,
    state: AutomatedFailsafeState,
    config: AutomatedFailsafeConfig,
) -> Result<AutomatedFailsafeEvaluation, AutomatedFailsafeError> {
    assert_failsafe_ready_for_arming(&config)?;
    if state.link_freshness.mission_id != mission.id {
        return Err(AutomatedFailsafeError::LinkMissionMismatch {
            expected: mission.id,
            actual: state.link_freshness.mission_id,
        });
    }

    let trigger = determine_trigger(&state, &config);
    let abort_plan = trigger
        .map(|trigger| {
            evaluate_abort_recovery(
                mission,
                AbortRecoveryContext {
                    current_position: state.current_position,
                    home_position: state.home_position,
                    battery_percentage: state.battery_percentage,
                    emergency_landing_sites: config.emergency_landing_sites.clone(),
                    trigger: trigger.into(),
                    triggered_at: state.checked_at,
                },
                config.abort_recovery,
            )
            .map_err(|error| AutomatedFailsafeError::AbortPlan {
                reason: error.to_string(),
            })
        })
        .transpose()?;
    let audit = vec![AutomatedFailsafeAuditEvent {
        mission_id: mission.id,
        trigger,
        event: if trigger.is_some() {
            AutomatedFailsafeAuditEventKind::Triggered
        } else {
            AutomatedFailsafeAuditEventKind::Clear
        },
        checked_at: state.checked_at,
        message: audit_message(trigger, &state, &config),
    }];

    Ok(AutomatedFailsafeEvaluation {
        mission_id: mission.id,
        trigger,
        abort_plan,
        audit,
    })
}

fn determine_trigger(
    state: &AutomatedFailsafeState,
    config: &AutomatedFailsafeConfig,
) -> Option<AutomatedFailsafeTrigger> {
    if state.link_freshness.state != TelemetryLinkState::Fresh {
        let age_seconds = state
            .link_freshness
            .age_seconds
            .unwrap_or(config.link_loss_threshold_seconds);
        if age_seconds >= config.link_loss_threshold_seconds {
            return Some(AutomatedFailsafeTrigger::LinkLoss);
        }
    }

    if state.battery_percentage <= config.critical_battery_percentage {
        return Some(AutomatedFailsafeTrigger::CriticalBattery);
    }

    None
}

fn audit_message(
    trigger: Option<AutomatedFailsafeTrigger>,
    state: &AutomatedFailsafeState,
    config: &AutomatedFailsafeConfig,
) -> String {
    match trigger {
        Some(AutomatedFailsafeTrigger::LinkLoss) => format!(
            "automated failsafe triggered by link loss after {} second(s)",
            state
                .link_freshness
                .age_seconds
                .unwrap_or(config.link_loss_threshold_seconds)
        ),
        Some(AutomatedFailsafeTrigger::CriticalBattery) => format!(
            "automated failsafe triggered by critical battery at {}%",
            state.battery_percentage
        ),
        None => "automated failsafe clear".to_string(),
    }
}

impl From<AutomatedFailsafeTrigger> for AbortTrigger {
    fn from(trigger: AutomatedFailsafeTrigger) -> Self {
        match trigger {
            AutomatedFailsafeTrigger::LinkLoss => Self::LinkLoss,
            AutomatedFailsafeTrigger::CriticalBattery => Self::LowBattery,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AbortRecoveryConfig, AbortTrigger, Mission, MissionStatus, TelemetryFreshness,
        TelemetryLinkState, Waypoint, WaypointType,
    };
    use chrono::{TimeZone, Utc};
    use geo::{point, polygon};

    fn in_flight_mission() -> Mission {
        let area = polygon![
            (x: 0.0, y: 0.0),
            (x: 200.0, y: 0.0),
            (x: 200.0, y: 200.0),
            (x: 0.0, y: 200.0),
            (x: 0.0, y: 0.0),
        ];
        let mut mission = Mission::new(
            "Automated Failsafe".to_string(),
            "automated failsafe fixture".to_string(),
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
        mission.arm().expect("fixture arms");
        mission.start().expect("fixture starts");
        mission
    }

    fn failsafe_config() -> AutomatedFailsafeConfig {
        AutomatedFailsafeConfig {
            link_loss_threshold_seconds: 5,
            critical_battery_percentage: 20,
            emergency_landing_sites: vec![point!(x: 30.0, y: 30.0)],
            abort_recovery: AbortRecoveryConfig::default(),
        }
    }

    fn stale_link(mission: &Mission) -> TelemetryFreshness {
        TelemetryFreshness {
            mission_id: mission.id,
            drone_id: "drone-1".to_string(),
            state: TelemetryLinkState::Stale,
            latest_timestamp: Some(Utc.timestamp_opt(1_800_000_000, 0).unwrap()),
            checked_at: Utc.timestamp_opt(1_800_000_012, 0).unwrap(),
            age_seconds: Some(12),
        }
    }

    #[test]
    fn automated_failsafe_invokes_rth_on_link_loss_and_audits() {
        let mission = in_flight_mission();

        let evaluation = evaluate_automated_failsafe(
            &mission,
            AutomatedFailsafeState {
                link_freshness: stale_link(&mission),
                battery_percentage: 75,
                current_position: point!(x: 40.0, y: 30.0),
                home_position: point!(x: 0.0, y: 0.0),
                checked_at: Utc.timestamp_opt(1_800_000_012, 0).unwrap(),
            },
            failsafe_config(),
        )
        .expect("link loss should produce failsafe evaluation");

        assert_eq!(evaluation.trigger, Some(AutomatedFailsafeTrigger::LinkLoss));
        let abort_plan = evaluation
            .abort_plan
            .expect("link loss should invoke abort");
        assert_eq!(abort_plan.trigger, AbortTrigger::LinkLoss);
        assert_eq!(abort_plan.mission_id, mission.id);
        assert_eq!(evaluation.audit.len(), 1);
        assert_eq!(
            evaluation.audit[0].event,
            AutomatedFailsafeAuditEventKind::Triggered
        );
        assert!(evaluation.audit[0].message.contains("link loss"));
        assert_eq!(mission.status, MissionStatus::InFlight);
    }

    #[test]
    fn failsafe_readiness_blocks_arming_without_emergency_site() {
        let mut config = failsafe_config();
        config.emergency_landing_sites.clear();

        let error = assert_failsafe_ready_for_arming(&config)
            .expect_err("missing emergency site should block arming");

        assert_eq!(error, AutomatedFailsafeError::MissingEmergencyLandingSite);
    }
}
