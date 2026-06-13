use crate::mavlink_integration::{
    MAVLinkCommandAckError, MAVLinkCommandAckRecord, MAVLinkCommandAckTracker,
};
use crate::{
    evaluate_abort_recovery, evaluate_dispatch_safety_with_constraints, AbortRecoveryConfig,
    AbortRecoveryContext, AbortRecoveryPlan, AirspaceConstraint, DispatchSafetyConfig,
    DispatchSafetyReport, Mission, MissionStatus, NoFlyZone, TelemetryLinkState, WeatherData,
};
use chrono::{DateTime, Duration, Utc};
use geo::Point;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct GuardedDispatchContext {
    pub mission_id: Uuid,
    pub current_position: Option<Point<f64>>,
    pub no_fly_zones: Vec<NoFlyZone>,
    pub dispatch_safety: DispatchSafetyConfig,
    pub battery_percentage: u8,
    pub minimum_battery_percentage: u8,
    pub weather: Option<WeatherData>,
    pub airspace_constraints: Vec<AirspaceConstraint>,
    pub link_state: TelemetryLinkState,
    pub sent_at: DateTime<Utc>,
    pub simulated_ack_latency: Duration,
    pub abort_context: Option<AbortRecoveryContext>,
    pub abort_config: AbortRecoveryConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardedDispatchCommand {
    pub correlation_id: Uuid,
    pub mavlink_command: u16,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardedDispatchAuditEventKind {
    CommandSent,
    CommandAcked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuardedDispatchAuditEvent {
    pub mission_id: Uuid,
    pub correlation_id: Uuid,
    pub mavlink_command: u16,
    pub event: GuardedDispatchAuditEventKind,
    pub at: DateTime<Utc>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuardedDispatchOutcome {
    pub command: GuardedDispatchCommand,
    pub ack: MAVLinkCommandAckRecord,
    pub dispatch_safety: DispatchSafetyReport,
    pub audit: Vec<GuardedDispatchAuditEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GuardedDispatchError {
    MissionMismatch {
        expected: Uuid,
        actual: Uuid,
    },
    MissionNotDispatchable {
        status: MissionStatus,
    },
    SafetyHalt {
        report: DispatchSafetyReport,
        abort_plan: Option<AbortRecoveryPlan>,
    },
    BatteryBelowMinimum {
        available: u8,
        minimum: u8,
    },
    LinkNotFresh {
        state: TelemetryLinkState,
    },
    AbortPathMissing,
    InvalidAckLatency,
    Ack(MAVLinkCommandAckError),
    AbortPlan {
        reason: String,
    },
}

impl fmt::Display for GuardedDispatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissionMismatch { expected, actual } => {
                write!(
                    formatter,
                    "dispatch mission mismatch: expected {expected}, got {actual}"
                )
            }
            Self::MissionNotDispatchable { status } => {
                write!(
                    formatter,
                    "mission status {status} cannot dispatch guarded commands"
                )
            }
            Self::SafetyHalt { report, .. } => write!(
                formatter,
                "guarded dispatch halted with {} safety violation(s)",
                report.violations.len()
            ),
            Self::BatteryBelowMinimum { available, minimum } => write!(
                formatter,
                "guarded dispatch battery {available}% below minimum {minimum}%"
            ),
            Self::LinkNotFresh { state } => {
                write!(formatter, "guarded dispatch link is not fresh: {state:?}")
            }
            Self::AbortPathMissing => {
                formatter.write_str("guarded dispatch requires an abort path")
            }
            Self::InvalidAckLatency => {
                formatter.write_str("simulated ack latency must be non-negative")
            }
            Self::Ack(error) => write!(formatter, "{error}"),
            Self::AbortPlan { reason } => write!(formatter, "abort planning failed: {reason}"),
        }
    }
}

impl std::error::Error for GuardedDispatchError {}

impl From<MAVLinkCommandAckError> for GuardedDispatchError {
    fn from(error: MAVLinkCommandAckError) -> Self {
        Self::Ack(error)
    }
}

pub fn dispatch_guarded_simulation_command(
    mission: &Mission,
    command: GuardedDispatchCommand,
    context: GuardedDispatchContext,
    ack_tracker: &mut MAVLinkCommandAckTracker,
) -> Result<GuardedDispatchOutcome, GuardedDispatchError> {
    if context.mission_id != mission.id {
        return Err(GuardedDispatchError::MissionMismatch {
            expected: mission.id,
            actual: context.mission_id,
        });
    }
    if !matches!(
        mission.status,
        MissionStatus::Armed | MissionStatus::InFlight
    ) {
        return Err(GuardedDispatchError::MissionNotDispatchable {
            status: mission.status,
        });
    }

    let dispatch_safety = evaluate_dispatch_safety_with_constraints(
        mission,
        context.current_position,
        &context.no_fly_zones,
        context.weather.as_ref(),
        &context.airspace_constraints,
        context.dispatch_safety,
    );
    if !dispatch_safety.is_clear() {
        let abort_plan = context
            .abort_context
            .map(|abort_context| {
                evaluate_abort_recovery(mission, abort_context, context.abort_config).map_err(
                    |error| GuardedDispatchError::AbortPlan {
                        reason: error.to_string(),
                    },
                )
            })
            .transpose()?;
        return Err(GuardedDispatchError::SafetyHalt {
            report: dispatch_safety,
            abort_plan,
        });
    }
    if context.battery_percentage < context.minimum_battery_percentage {
        return Err(GuardedDispatchError::BatteryBelowMinimum {
            available: context.battery_percentage,
            minimum: context.minimum_battery_percentage,
        });
    }
    if context.link_state != TelemetryLinkState::Fresh {
        return Err(GuardedDispatchError::LinkNotFresh {
            state: context.link_state,
        });
    }
    if context.abort_context.is_none() {
        return Err(GuardedDispatchError::AbortPathMissing);
    }
    if context.simulated_ack_latency < Duration::zero() {
        return Err(GuardedDispatchError::InvalidAckLatency);
    }

    let sent = ack_tracker.start_command(
        command.correlation_id,
        command.mavlink_command,
        context.sent_at,
    );
    let acked = ack_tracker.handle_ack(
        command.correlation_id,
        command.mavlink_command,
        context.sent_at + context.simulated_ack_latency,
    )?;
    let audit = vec![
        GuardedDispatchAuditEvent {
            mission_id: mission.id,
            correlation_id: command.correlation_id,
            mavlink_command: command.mavlink_command,
            event: GuardedDispatchAuditEventKind::CommandSent,
            at: sent.sent_at,
            message: format!("guarded dispatch sent {}", command.label),
        },
        GuardedDispatchAuditEvent {
            mission_id: mission.id,
            correlation_id: command.correlation_id,
            mavlink_command: command.mavlink_command,
            event: GuardedDispatchAuditEventKind::CommandAcked,
            at: acked.acked_at.unwrap_or(acked.last_attempt_at),
            message: format!("guarded dispatch acked {}", command.label),
        },
    ];

    Ok(GuardedDispatchOutcome {
        command,
        ack: acked,
        dispatch_safety,
        audit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mavlink_integration::{
        MAVLinkAckConfig, MAVLinkCommandAckStatus, MAVLinkCommandAckTracker, MAV_CMD_NAV_TAKEOFF,
    };
    use crate::{
        AbortRecoveryConfig, AbortRecoveryContext, AbortTrigger, DispatchSafetyConfig, Mission,
        MissionStatus, TelemetryLinkState, Waypoint, WaypointType, WeatherData,
    };
    use chrono::{Duration, TimeZone, Utc};
    use geo::{point, polygon};
    use uuid::Uuid;

    fn armed_mission() -> Mission {
        let area = polygon![
            (x: 0.0, y: 0.0),
            (x: 200.0, y: 0.0),
            (x: 200.0, y: 200.0),
            (x: 0.0, y: 200.0),
            (x: 0.0, y: 0.0),
        ];
        let mut mission = Mission::new(
            "Guarded Dispatch".to_string(),
            "guarded dispatch fixture".to_string(),
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
        mission
    }

    fn dispatch_context(mission: &Mission) -> GuardedDispatchContext {
        let sent_at = Utc.timestamp_opt(1_800_000_100, 0).unwrap();
        GuardedDispatchContext {
            current_position: Some(point!(x: 20.0, y: 20.0)),
            no_fly_zones: Vec::new(),
            dispatch_safety: DispatchSafetyConfig {
                altitude_ceiling_m: 120.0,
            },
            battery_percentage: 80,
            minimum_battery_percentage: 30,
            weather: None,
            airspace_constraints: Vec::new(),
            link_state: TelemetryLinkState::Fresh,
            sent_at,
            simulated_ack_latency: Duration::milliseconds(120),
            abort_context: Some(AbortRecoveryContext {
                current_position: point!(x: 20.0, y: 20.0),
                home_position: point!(x: 0.0, y: 0.0),
                battery_percentage: 80,
                emergency_landing_sites: vec![point!(x: 30.0, y: 30.0)],
                trigger: AbortTrigger::GeofenceViolation,
                triggered_at: sent_at,
            }),
            abort_config: AbortRecoveryConfig::default(),
            mission_id: mission.id,
        }
    }

    #[test]
    fn guarded_dispatch_sends_simulated_command_with_ack_and_audit() {
        let mission = armed_mission();
        let correlation_id = Uuid::new_v4();
        let mut tracker = MAVLinkCommandAckTracker::new(MAVLinkAckConfig {
            timeout: Duration::seconds(1),
            max_retries: 1,
        });

        let outcome = dispatch_guarded_simulation_command(
            &mission,
            GuardedDispatchCommand {
                correlation_id,
                mavlink_command: MAV_CMD_NAV_TAKEOFF,
                label: "takeoff".to_string(),
            },
            dispatch_context(&mission),
            &mut tracker,
        )
        .expect("armed compliant mission should dispatch in simulation");

        assert_eq!(outcome.ack.status, MAVLinkCommandAckStatus::Acked);
        assert_eq!(outcome.ack.retry_count, 0);
        assert_eq!(
            tracker
                .record(correlation_id)
                .expect("command tracked")
                .status,
            MAVLinkCommandAckStatus::Acked
        );
        assert_eq!(outcome.audit.len(), 2);
        assert_eq!(
            outcome.audit[0].event,
            GuardedDispatchAuditEventKind::CommandSent
        );
        assert_eq!(
            outcome.audit[1].event,
            GuardedDispatchAuditEventKind::CommandAcked
        );
    }

    #[test]
    fn guarded_dispatch_halts_on_midflight_geofence_violation_without_sending_command() {
        let mut mission = armed_mission();
        mission.start().expect("fixture starts");
        let correlation_id = Uuid::new_v4();
        let mut context = dispatch_context(&mission);
        context.current_position = Some(point!(x: 250.0, y: 250.0));
        context.abort_context.as_mut().unwrap().current_position = point!(x: 250.0, y: 250.0);
        let mut tracker = MAVLinkCommandAckTracker::new(MAVLinkAckConfig::default());

        let error = dispatch_guarded_simulation_command(
            &mission,
            GuardedDispatchCommand {
                correlation_id,
                mavlink_command: MAV_CMD_NAV_TAKEOFF,
                label: "next-waypoint".to_string(),
            },
            context,
            &mut tracker,
        )
        .expect_err("geofence violation should halt guarded dispatch");

        match error {
            GuardedDispatchError::SafetyHalt { report, abort_plan } => {
                assert!(!report.is_clear());
                assert!(abort_plan.is_some());
            }
            other => panic!("expected safety halt, got {other:?}"),
        }
        assert!(tracker.record(correlation_id).is_none());
        assert_eq!(mission.status, MissionStatus::InFlight);
    }

    #[test]
    fn guarded_dispatch_blocks_over_wind_constraint_before_sending_command() {
        let mission = armed_mission();
        let correlation_id = Uuid::new_v4();
        let mut context = dispatch_context(&mission);
        let weather = WeatherData {
            temperature_celsius: 20.0,
            humidity_percent: 50.0,
            wind_speed_ms: 21.0,
            wind_direction_degrees: 180.0,
            precipitation_mm: 0.0,
            visibility_m: 10000.0,
            pressure_hpa: 1015.0,
            cloud_cover_percent: 30.0,
        };
        context.weather = Some(weather);
        let mut tracker = MAVLinkCommandAckTracker::new(MAVLinkAckConfig::default());

        let error = dispatch_guarded_simulation_command(
            &mission,
            GuardedDispatchCommand {
                correlation_id,
                mavlink_command: MAV_CMD_NAV_TAKEOFF,
                label: "takeoff".to_string(),
            },
            context,
            &mut tracker,
        )
        .expect_err("over-wind constraint should block dispatch before command send");

        match error {
            GuardedDispatchError::SafetyHalt { report, abort_plan } => {
                assert_eq!(
                    report.violations[0].code,
                    crate::SafetyViolationCode::WindSpeedExceeded
                );
                assert_eq!(report.violations[0].measured_value, Some(21.0));
                assert_eq!(report.violations[0].threshold_value, Some(15.0));
                assert!(abort_plan.is_some());
            }
            other => panic!("expected weather/airspace block, got {other:?}"),
        }
        assert!(tracker.record(correlation_id).is_none());
        assert_eq!(mission.status, MissionStatus::Armed);
    }
}
