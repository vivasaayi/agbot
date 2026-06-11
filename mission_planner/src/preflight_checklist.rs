use crate::{
    evaluate_dispatch_safety, DispatchSafetyConfig, DispatchSafetyReport, Mission,
    MissionStateTransitionError, NoFlyZone, TelemetryFreshness, TelemetryLinkState,
};
use geo::Point;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PreflightChecklistConfig {
    pub dispatch_safety: DispatchSafetyConfig,
    pub minimum_launch_battery_percentage: u8,
    pub maximum_hdop: f32,
    pub minimum_satellites: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightChecklistContext {
    pub current_position: Option<Point<f64>>,
    pub no_fly_zones: Vec<NoFlyZone>,
    pub config: PreflightChecklistConfig,
    pub battery_percentage: u8,
    pub gps: GpsFixStatus,
    pub link_freshness: TelemetryFreshness,
    pub failsafe_configured: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GpsFixStatus {
    pub fix_type: GpsFixType,
    pub hdop: f32,
    pub satellites: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpsFixType {
    NoFix,
    TwoD,
    ThreeD,
    RtkFloat,
    RtkFixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreflightCheckName {
    DispatchSafety,
    BatteryBudget,
    GpsFix,
    LinkFreshness,
    FailsafeConfigured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreflightCheckStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreflightCheckResult {
    pub name: PreflightCheckName,
    pub status: PreflightCheckStatus,
    pub message: String,
    pub measured_value: Option<String>,
    pub required_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreflightChecklistReport {
    pub clear: bool,
    pub checks: Vec<PreflightCheckResult>,
    pub dispatch_safety: DispatchSafetyReport,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PreflightArmError {
    State(MissionStateTransitionError),
    Checklist(PreflightChecklistReport),
}

impl PreflightChecklistConfig {
    pub fn default_with_dispatch(dispatch_safety: DispatchSafetyConfig) -> Self {
        Self {
            dispatch_safety,
            minimum_launch_battery_percentage: 30,
            maximum_hdop: 1.5,
            minimum_satellites: 8,
        }
    }
}

impl PreflightCheckResult {
    pub fn passed(
        name: PreflightCheckName,
        message: impl Into<String>,
        measured_value: Option<String>,
        required_value: Option<String>,
    ) -> Self {
        Self {
            name,
            status: PreflightCheckStatus::Passed,
            message: message.into(),
            measured_value,
            required_value,
        }
    }

    pub fn failed(
        name: PreflightCheckName,
        message: impl Into<String>,
        measured_value: Option<String>,
        required_value: Option<String>,
    ) -> Self {
        Self {
            name,
            status: PreflightCheckStatus::Failed,
            message: message.into(),
            measured_value,
            required_value,
        }
    }

    pub fn is_failed(&self) -> bool {
        self.status == PreflightCheckStatus::Failed
    }
}

impl PreflightChecklistReport {
    fn new(checks: Vec<PreflightCheckResult>, dispatch_safety: DispatchSafetyReport) -> Self {
        let clear = checks
            .iter()
            .all(|check| check.status == PreflightCheckStatus::Passed);
        Self {
            clear,
            checks,
            dispatch_safety,
        }
    }

    pub fn is_clear(&self) -> bool {
        self.clear
    }

    pub fn failed_check_names(&self) -> Vec<PreflightCheckName> {
        self.checks
            .iter()
            .filter(|check| check.is_failed())
            .map(|check| check.name)
            .collect()
    }
}

impl GpsFixType {
    fn is_launch_capable(self) -> bool {
        matches!(self, Self::ThreeD | Self::RtkFloat | Self::RtkFixed)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::NoFix => "NoFix",
            Self::TwoD => "TwoD",
            Self::ThreeD => "ThreeD",
            Self::RtkFloat => "RtkFloat",
            Self::RtkFixed => "RtkFixed",
        }
    }
}

impl fmt::Display for PreflightArmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State(error) => write!(formatter, "{error}"),
            Self::Checklist(report) => write!(
                formatter,
                "preflight checklist rejected arming with {} failing check(s)",
                report.failed_check_names().len()
            ),
        }
    }
}

impl std::error::Error for PreflightArmError {}

impl From<MissionStateTransitionError> for PreflightArmError {
    fn from(error: MissionStateTransitionError) -> Self {
        Self::State(error)
    }
}

pub fn evaluate_preflight_checklist(
    mission: &Mission,
    context: &PreflightChecklistContext,
) -> PreflightChecklistReport {
    let dispatch_safety = evaluate_dispatch_safety(
        mission,
        context.current_position,
        &context.no_fly_zones,
        context.config.dispatch_safety,
    );

    let checks = vec![
        dispatch_safety_check(&dispatch_safety),
        battery_check(context),
        gps_check(context),
        link_check(mission, context),
        failsafe_check(context),
    ];

    PreflightChecklistReport::new(checks, dispatch_safety)
}

fn dispatch_safety_check(report: &DispatchSafetyReport) -> PreflightCheckResult {
    if report.is_clear() {
        PreflightCheckResult::passed(
            PreflightCheckName::DispatchSafety,
            "dispatch geofence and no-fly checks passed",
            Some("0 violations".to_string()),
            Some("0 violations".to_string()),
        )
    } else {
        PreflightCheckResult::failed(
            PreflightCheckName::DispatchSafety,
            "dispatch safety violations block arming",
            Some(format!("{} violation(s)", report.violations.len())),
            Some("0 violations".to_string()),
        )
    }
}

fn battery_check(context: &PreflightChecklistContext) -> PreflightCheckResult {
    let measured = format!("{}%", context.battery_percentage);
    let required = format!(">= {}%", context.config.minimum_launch_battery_percentage);

    if context.battery_percentage >= context.config.minimum_launch_battery_percentage {
        PreflightCheckResult::passed(
            PreflightCheckName::BatteryBudget,
            "battery is above launch budget",
            Some(measured),
            Some(required),
        )
    } else {
        PreflightCheckResult::failed(
            PreflightCheckName::BatteryBudget,
            "battery is below launch budget",
            Some(measured),
            Some(required),
        )
    }
}

fn gps_check(context: &PreflightChecklistContext) -> PreflightCheckResult {
    let hdop_ok = context.gps.hdop.is_finite() && context.gps.hdop <= context.config.maximum_hdop;
    let satellites_ok = context.gps.satellites >= context.config.minimum_satellites;
    let fix_ok = context.gps.fix_type.is_launch_capable();
    let measured = format!(
        "{}, HDOP {:.1}, {} satellites",
        context.gps.fix_type.as_str(),
        context.gps.hdop,
        context.gps.satellites
    );
    let required = format!(
        "3D fix, HDOP <= {:.1}, satellites >= {}",
        context.config.maximum_hdop, context.config.minimum_satellites
    );

    if fix_ok && hdop_ok && satellites_ok {
        PreflightCheckResult::passed(
            PreflightCheckName::GpsFix,
            "GPS fix quality is launch capable",
            Some(measured),
            Some(required),
        )
    } else {
        PreflightCheckResult::failed(
            PreflightCheckName::GpsFix,
            "GPS fix quality is not launch capable",
            Some(measured),
            Some(required),
        )
    }
}

fn link_check(mission: &Mission, context: &PreflightChecklistContext) -> PreflightCheckResult {
    let mission_matches = context.link_freshness.mission_id == mission.id;
    let link_fresh = context.link_freshness.state == TelemetryLinkState::Fresh;
    let measured = format!(
        "{:?}, age {}",
        context.link_freshness.state,
        context
            .link_freshness
            .age_seconds
            .map(|age| format!("{age}s"))
            .unwrap_or_else(|| "unknown".to_string())
    );

    if mission_matches && link_fresh {
        PreflightCheckResult::passed(
            PreflightCheckName::LinkFreshness,
            "telemetry link is fresh for this mission",
            Some(measured),
            Some("Fresh".to_string()),
        )
    } else {
        PreflightCheckResult::failed(
            PreflightCheckName::LinkFreshness,
            "telemetry link freshness blocks arming",
            Some(measured),
            Some("Fresh".to_string()),
        )
    }
}

fn failsafe_check(context: &PreflightChecklistContext) -> PreflightCheckResult {
    if context.failsafe_configured {
        PreflightCheckResult::passed(
            PreflightCheckName::FailsafeConfigured,
            "failsafe configuration is present",
            Some("configured".to_string()),
            Some("configured".to_string()),
        )
    } else {
        PreflightCheckResult::failed(
            PreflightCheckName::FailsafeConfigured,
            "failsafe configuration is missing",
            Some("missing".to_string()),
            Some("configured".to_string()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DispatchSafetyConfig, Mission, MissionStatus, NoFlyZone, Waypoint, WaypointType};
    use chrono::{TimeZone, Utc};
    use geo::{point, polygon};
    use uuid::Uuid;

    fn sample_mission() -> Mission {
        let area = polygon![
            (x: 0.0, y: 0.0),
            (x: 100.0, y: 0.0),
            (x: 100.0, y: 100.0),
            (x: 0.0, y: 100.0),
            (x: 0.0, y: 0.0),
        ];
        let mut mission = Mission::new(
            "Preflight Mission".to_string(),
            "preflight checklist fixture".to_string(),
            area,
        );
        mission.add_waypoint(Waypoint::new(
            point!(x: 10.0, y: 10.0),
            20.0,
            WaypointType::Takeoff,
        ));
        mission.add_waypoint(Waypoint::new(
            point!(x: 90.0, y: 90.0),
            40.0,
            WaypointType::Survey,
        ));
        mission.add_waypoint(Waypoint::new(
            point!(x: 95.0, y: 95.0),
            0.0,
            WaypointType::Landing,
        ));
        mission
    }

    fn fresh_link(mission_id: Uuid) -> crate::TelemetryFreshness {
        crate::TelemetryFreshness {
            mission_id,
            drone_id: "drone-1".to_string(),
            state: crate::TelemetryLinkState::Fresh,
            latest_timestamp: Some(Utc.timestamp_opt(100, 0).unwrap()),
            checked_at: Utc.timestamp_opt(102, 0).unwrap(),
            age_seconds: Some(2),
        }
    }

    fn checklist_context(mission_id: Uuid) -> PreflightChecklistContext {
        PreflightChecklistContext {
            current_position: Some(point!(x: 20.0, y: 20.0)),
            no_fly_zones: Vec::new(),
            config: PreflightChecklistConfig {
                dispatch_safety: DispatchSafetyConfig {
                    altitude_ceiling_m: 120.0,
                },
                minimum_launch_battery_percentage: 30,
                maximum_hdop: 1.5,
                minimum_satellites: 8,
            },
            battery_percentage: 82,
            gps: GpsFixStatus {
                fix_type: GpsFixType::ThreeD,
                hdop: 0.8,
                satellites: 12,
            },
            link_freshness: fresh_link(mission_id),
            failsafe_configured: true,
        }
    }

    fn assert_single_failure(report: &PreflightChecklistReport, expected: PreflightCheckName) {
        assert_eq!(report.failed_check_names(), vec![expected]);
        assert_eq!(
            report
                .checks
                .iter()
                .filter(|check| check.is_failed())
                .count(),
            1
        );
    }

    #[test]
    fn preflight_checklist_allows_arm_when_all_checks_pass() {
        let mut mission = sample_mission();
        mission.validate().expect("fixture validates");
        let context = checklist_context(mission.id);

        let report = mission
            .arm_with_preflight_checklist(&context)
            .expect("all checklist checks pass");

        assert!(report.is_clear());
        assert_eq!(mission.status, MissionStatus::Armed);
        assert_eq!(
            report
                .checks
                .iter()
                .map(|check| check.name)
                .collect::<Vec<_>>(),
            vec![
                PreflightCheckName::DispatchSafety,
                PreflightCheckName::BatteryBudget,
                PreflightCheckName::GpsFix,
                PreflightCheckName::LinkFreshness,
                PreflightCheckName::FailsafeConfigured,
            ]
        );
    }

    #[test]
    fn preflight_checklist_blocks_low_battery_by_name() {
        let mut mission = sample_mission();
        mission.validate().expect("fixture validates");
        let mut context = checklist_context(mission.id);
        context.battery_percentage = 24;

        let error = mission
            .arm_with_preflight_checklist(&context)
            .expect_err("low battery must block arming");

        match error {
            crate::PreflightArmError::Checklist(report) => {
                assert_single_failure(&report, PreflightCheckName::BatteryBudget);
                assert_eq!(
                    report
                        .checks
                        .iter()
                        .find(|check| check.name == PreflightCheckName::BatteryBudget)
                        .and_then(|check| check.measured_value.as_deref()),
                    Some("24%")
                );
            }
            crate::PreflightArmError::State(error) => {
                panic!("expected checklist error, got state error: {error}");
            }
        }
        assert_eq!(mission.status, MissionStatus::Validated);
    }

    #[test]
    fn preflight_checklist_reports_each_failed_item() {
        let mission = sample_mission();

        let mut no_fly_context = checklist_context(mission.id);
        no_fly_context.no_fly_zones = vec![NoFlyZone {
            id: "nfz-1".to_string(),
            boundary: polygon![
                (x: 45.0, y: 45.0),
                (x: 55.0, y: 45.0),
                (x: 55.0, y: 55.0),
                (x: 45.0, y: 55.0),
                (x: 45.0, y: 45.0),
            ],
        }];
        assert_single_failure(
            &evaluate_preflight_checklist(&mission, &no_fly_context),
            PreflightCheckName::DispatchSafety,
        );

        let mut gps_context = checklist_context(mission.id);
        gps_context.gps = GpsFixStatus {
            fix_type: GpsFixType::NoFix,
            hdop: 3.4,
            satellites: 4,
        };
        assert_single_failure(
            &evaluate_preflight_checklist(&mission, &gps_context),
            PreflightCheckName::GpsFix,
        );

        let mut link_context = checklist_context(mission.id);
        link_context.link_freshness.state = crate::TelemetryLinkState::Stale;
        assert_single_failure(
            &evaluate_preflight_checklist(&mission, &link_context),
            PreflightCheckName::LinkFreshness,
        );

        let mut failsafe_context = checklist_context(mission.id);
        failsafe_context.failsafe_configured = false;
        assert_single_failure(
            &evaluate_preflight_checklist(&mission, &failsafe_context),
            PreflightCheckName::FailsafeConfigured,
        );
    }

    #[test]
    fn preflight_checklist_result_serializes_contract() {
        let mission = sample_mission();
        let mut context = checklist_context(mission.id);
        context.battery_percentage = 24;

        let report = evaluate_preflight_checklist(&mission, &context);
        let json = serde_json::to_value(&report).expect("report serializes");

        assert_eq!(json["clear"], false);
        assert_eq!(json["checks"][1]["name"], "BatteryBudget");
        assert_eq!(json["checks"][1]["status"], "Failed");
        assert_eq!(json["checks"][1]["measured_value"], "24%");
        assert_eq!(json["checks"][1]["required_value"], ">= 30%");
    }
}
