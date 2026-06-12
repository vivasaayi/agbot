use crate::{Mission, MissionStateTransitionError};
use chrono::{DateTime, Utc};
use geo::Point;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AbortRecoveryConfig {
    pub return_home_reserve_battery_percentage: u8,
    pub battery_percent_per_meter: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbortRecoveryContext {
    pub current_position: Point<f64>,
    pub home_position: Point<f64>,
    pub battery_percentage: u8,
    pub emergency_landing_sites: Vec<Point<f64>>,
    pub trigger: AbortTrigger,
    pub triggered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbortTrigger {
    OperatorAbort,
    LinkLoss,
    LowBattery,
    GeofenceViolation,
    SystemFault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbortRecoveryAction {
    ReturnToHome,
    LandAtEmergencySite,
    LandInPlace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbortRecoveryCommand {
    ReturnToHome,
    Land,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbortRecoveryAuditEvent {
    pub mission_id: uuid::Uuid,
    pub trigger: AbortTrigger,
    pub action: AbortRecoveryAction,
    pub command: AbortRecoveryCommand,
    pub target_position: Point<f64>,
    pub available_battery_percentage: f32,
    pub required_return_battery_percentage: f32,
    pub triggered_at: DateTime<Utc>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbortRecoveryPlan {
    pub mission_id: uuid::Uuid,
    pub trigger: AbortTrigger,
    pub action: AbortRecoveryAction,
    pub command: AbortRecoveryCommand,
    pub target_position: Point<f64>,
    pub distance_to_home_m: f64,
    pub available_battery_percentage: f32,
    pub required_return_battery_percentage: f32,
    pub audit: AbortRecoveryAuditEvent,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AbortRecoveryError {
    InvalidConfig { reason: String },
    State(MissionStateTransitionError),
}

impl Default for AbortRecoveryConfig {
    fn default() -> Self {
        Self {
            return_home_reserve_battery_percentage: 10,
            battery_percent_per_meter: 0.05,
        }
    }
}

impl fmt::Display for AbortRecoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { reason } => write!(formatter, "invalid abort config: {reason}"),
            Self::State(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for AbortRecoveryError {}

impl From<MissionStateTransitionError> for AbortRecoveryError {
    fn from(error: MissionStateTransitionError) -> Self {
        Self::State(error)
    }
}

pub fn abort_mission_with_recovery(
    mission: &mut Mission,
    context: AbortRecoveryContext,
    config: AbortRecoveryConfig,
) -> Result<AbortRecoveryPlan, AbortRecoveryError> {
    let plan = evaluate_abort_recovery(mission, context, config)?;
    mission.abort()?;
    Ok(plan)
}

pub fn evaluate_abort_recovery(
    mission: &Mission,
    context: AbortRecoveryContext,
    config: AbortRecoveryConfig,
) -> Result<AbortRecoveryPlan, AbortRecoveryError> {
    validate_config(config)?;
    let distance_to_home_m = distance_m(context.current_position, context.home_position);
    let required_return_battery_percentage = distance_to_home_m
        * f64::from(config.battery_percent_per_meter)
        + f64::from(config.return_home_reserve_battery_percentage);
    let available_battery_percentage = f32::from(context.battery_percentage);

    let (action, command, target_position, message) = if f64::from(available_battery_percentage)
        >= required_return_battery_percentage
    {
        (
            AbortRecoveryAction::ReturnToHome,
            AbortRecoveryCommand::ReturnToHome,
            context.home_position,
            format!(
                "return-to-home selected with {:.1}% battery available and {:.1}% required",
                available_battery_percentage, required_return_battery_percentage
            ),
        )
    } else if let Some(site) =
        nearest_landing_site(context.current_position, &context.emergency_landing_sites)
    {
        (
                AbortRecoveryAction::LandAtEmergencySite,
                AbortRecoveryCommand::Land,
                site,
                format!(
                    "return-to-home unreachable: {:.1}% battery available, {:.1}% required; landing at nearest emergency site",
                    available_battery_percentage, required_return_battery_percentage
                ),
            )
    } else {
        (
                AbortRecoveryAction::LandInPlace,
                AbortRecoveryCommand::Land,
                context.current_position,
                format!(
                    "return-to-home unreachable: {:.1}% battery available, {:.1}% required; landing in place",
                    available_battery_percentage, required_return_battery_percentage
                ),
            )
    };
    let required_return_battery_percentage = required_return_battery_percentage as f32;
    let audit = AbortRecoveryAuditEvent {
        mission_id: mission.id,
        trigger: context.trigger,
        action,
        command,
        target_position,
        available_battery_percentage,
        required_return_battery_percentage,
        triggered_at: context.triggered_at,
        message,
    };

    Ok(AbortRecoveryPlan {
        mission_id: mission.id,
        trigger: context.trigger,
        action,
        command,
        target_position,
        distance_to_home_m,
        available_battery_percentage,
        required_return_battery_percentage,
        audit,
    })
}

fn validate_config(config: AbortRecoveryConfig) -> Result<(), AbortRecoveryError> {
    if !config.battery_percent_per_meter.is_finite() || config.battery_percent_per_meter < 0.0 {
        return Err(AbortRecoveryError::InvalidConfig {
            reason: "battery_percent_per_meter must be finite and non-negative".to_string(),
        });
    }
    Ok(())
}

fn nearest_landing_site(current: Point<f64>, sites: &[Point<f64>]) -> Option<Point<f64>> {
    sites.iter().copied().min_by(|left, right| {
        distance_m(current, *left)
            .partial_cmp(&distance_m(current, *right))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

fn distance_m(left: Point<f64>, right: Point<f64>) -> f64 {
    let dx = left.x() - right.x();
    let dy = left.y() - right.y();
    dx.hypot(dy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Mission, MissionStatus, Waypoint, WaypointType};
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
            "Abort Recovery".to_string(),
            "abort recovery fixture".to_string(),
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

    fn recovery_config() -> AbortRecoveryConfig {
        AbortRecoveryConfig {
            return_home_reserve_battery_percentage: 10,
            battery_percent_per_meter: 0.05,
        }
    }

    #[test]
    fn abort_recovery_commands_rth_and_audits_operator_abort() {
        let mut mission = in_flight_mission();
        let triggered_at = Utc.timestamp_opt(1_800_000_000, 0).unwrap();

        let plan = abort_mission_with_recovery(
            &mut mission,
            AbortRecoveryContext {
                current_position: point!(x: 30.0, y: 40.0),
                home_position: point!(x: 0.0, y: 0.0),
                battery_percentage: 80,
                emergency_landing_sites: vec![point!(x: 60.0, y: 60.0)],
                trigger: AbortTrigger::OperatorAbort,
                triggered_at,
            },
            recovery_config(),
        )
        .expect("RTH should be reachable");

        assert_eq!(mission.status, MissionStatus::Aborted);
        assert_eq!(plan.action, AbortRecoveryAction::ReturnToHome);
        assert_eq!(plan.command, AbortRecoveryCommand::ReturnToHome);
        assert_eq!(plan.target_position, point!(x: 0.0, y: 0.0));
        assert_eq!(plan.audit.trigger, AbortTrigger::OperatorAbort);
        assert_eq!(plan.audit.command, AbortRecoveryCommand::ReturnToHome);
        assert_eq!(plan.audit.triggered_at, triggered_at);
        assert!(plan.audit.message.contains("return-to-home"));
    }

    #[test]
    fn abort_recovery_lands_at_nearest_emergency_site_when_rth_unreachable() {
        let mut mission = in_flight_mission();

        let plan = abort_mission_with_recovery(
            &mut mission,
            AbortRecoveryContext {
                current_position: point!(x: 100.0, y: 100.0),
                home_position: point!(x: 0.0, y: 0.0),
                battery_percentage: 12,
                emergency_landing_sites: vec![
                    point!(x: 130.0, y: 100.0),
                    point!(x: 105.0, y: 105.0),
                ],
                trigger: AbortTrigger::LowBattery,
                triggered_at: Utc.timestamp_opt(1_800_000_030, 0).unwrap(),
            },
            recovery_config(),
        )
        .expect("low battery should fall back to emergency landing");

        assert_eq!(mission.status, MissionStatus::Aborted);
        assert_eq!(plan.action, AbortRecoveryAction::LandAtEmergencySite);
        assert_eq!(plan.command, AbortRecoveryCommand::Land);
        assert_eq!(plan.target_position, point!(x: 105.0, y: 105.0));
        assert!(plan.required_return_battery_percentage > plan.available_battery_percentage);
        assert!(plan.audit.message.contains("return-to-home unreachable"));
    }
}
