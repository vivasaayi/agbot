use crate::survey_template::{validate_plan_bounds, PlanBoundsConfig, PlanBoundsIssueCode};
use crate::weather_integration::WeatherData;
use crate::{Mission, MissionStateTransitionError};
use geo::{Contains, Intersects, LineString, Point, Polygon};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DispatchSafetyConfig {
    pub altitude_ceiling_m: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoFlyZone {
    pub id: String,
    pub boundary: Polygon<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AirspaceConstraint {
    pub id: String,
    pub boundary: Polygon<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafetyViolationCode {
    InvalidGeofence,
    WaypointOutsideGeofence,
    CurrentPositionOutsideGeofence,
    AltitudeCeilingExceeded,
    NoFlyZoneIntersection,
    WindSpeedExceeded,
    PrecipitationExceeded,
    AirspaceIntersection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafetyViolationSeverity {
    Blocker,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SafetyViolation {
    pub code: SafetyViolationCode,
    pub severity: SafetyViolationSeverity,
    pub waypoint_index: Option<usize>,
    pub zone_id: Option<String>,
    #[serde(default)]
    pub measured_value: Option<f32>,
    #[serde(default)]
    pub threshold_value: Option<f32>,
    #[serde(default)]
    pub unit: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DispatchSafetyReport {
    pub violations: Vec<SafetyViolation>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DispatchSafetyError {
    State(MissionStateTransitionError),
    SafetyViolation(DispatchSafetyReport),
}

impl DispatchSafetyReport {
    pub fn is_clear(&self) -> bool {
        self.violations.is_empty()
    }
}

impl fmt::Display for DispatchSafetyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State(error) => write!(formatter, "{error}"),
            Self::SafetyViolation(report) => write!(
                formatter,
                "dispatch safety rejected mission with {} blocker(s)",
                report.violations.len()
            ),
        }
    }
}

impl std::error::Error for DispatchSafetyError {}

impl From<MissionStateTransitionError> for DispatchSafetyError {
    fn from(error: MissionStateTransitionError) -> Self {
        Self::State(error)
    }
}

pub fn evaluate_dispatch_safety(
    mission: &Mission,
    current_position: Option<Point<f64>>,
    no_fly_zones: &[NoFlyZone],
    config: DispatchSafetyConfig,
) -> DispatchSafetyReport {
    evaluate_dispatch_safety_with_constraints(
        mission,
        current_position,
        no_fly_zones,
        None,
        &[],
        config,
    )
}

pub fn evaluate_dispatch_safety_with_constraints(
    mission: &Mission,
    current_position: Option<Point<f64>>,
    no_fly_zones: &[NoFlyZone],
    weather: Option<&WeatherData>,
    airspace_constraints: &[AirspaceConstraint],
    config: DispatchSafetyConfig,
) -> DispatchSafetyReport {
    let mut violations = Vec::new();

    if let Err(error) = validate_plan_bounds(
        &mission.waypoints,
        &mission.area_of_interest,
        PlanBoundsConfig {
            max_altitude_m: config.altitude_ceiling_m,
        },
    ) {
        violations.extend(error.issues.into_iter().map(|issue| SafetyViolation {
            code: match issue.code {
                PlanBoundsIssueCode::AltitudeCeilingExceeded => {
                    SafetyViolationCode::AltitudeCeilingExceeded
                }
                PlanBoundsIssueCode::OutsideGeofence => {
                    SafetyViolationCode::WaypointOutsideGeofence
                }
                PlanBoundsIssueCode::InvalidBoundary
                | PlanBoundsIssueCode::InvalidAltitudeCeiling => {
                    SafetyViolationCode::InvalidGeofence
                }
            },
            severity: SafetyViolationSeverity::Blocker,
            waypoint_index: issue.waypoint_index,
            zone_id: None,
            measured_value: None,
            threshold_value: None,
            unit: None,
            message: issue.message,
        }));
    }

    if let Some(position) = current_position {
        if !point_is_inside_or_on_boundary(&mission.area_of_interest, &position) {
            violations.push(SafetyViolation {
                code: SafetyViolationCode::CurrentPositionOutsideGeofence,
                severity: SafetyViolationSeverity::Blocker,
                waypoint_index: None,
                zone_id: None,
                measured_value: None,
                threshold_value: None,
                unit: None,
                message: "current aircraft position lies outside the mission geofence".to_string(),
            });
        }
    }

    if let Some(weather) = weather {
        if weather.wind_speed_ms > mission.weather_constraints.max_wind_speed_ms {
            violations.push(SafetyViolation {
                code: SafetyViolationCode::WindSpeedExceeded,
                severity: SafetyViolationSeverity::Blocker,
                waypoint_index: None,
                zone_id: None,
                measured_value: Some(weather.wind_speed_ms),
                threshold_value: Some(mission.weather_constraints.max_wind_speed_ms),
                unit: Some("m/s".to_string()),
                message: format!(
                    "wind speed {:.1} m/s exceeds dispatch limit {:.1} m/s",
                    weather.wind_speed_ms, mission.weather_constraints.max_wind_speed_ms
                ),
            });
        }

        if weather.precipitation_mm > mission.weather_constraints.max_precipitation_mm {
            violations.push(SafetyViolation {
                code: SafetyViolationCode::PrecipitationExceeded,
                severity: SafetyViolationSeverity::Blocker,
                waypoint_index: None,
                zone_id: None,
                measured_value: Some(weather.precipitation_mm),
                threshold_value: Some(mission.weather_constraints.max_precipitation_mm),
                unit: Some("mm".to_string()),
                message: format!(
                    "precipitation {:.1} mm exceeds dispatch limit {:.1} mm",
                    weather.precipitation_mm, mission.weather_constraints.max_precipitation_mm
                ),
            });
        }
    }

    for zone in no_fly_zones {
        for (leg_index, pair) in mission.waypoints.windows(2).enumerate() {
            let leg = LineString::from(vec![
                (pair[0].position.x(), pair[0].position.y()),
                (pair[1].position.x(), pair[1].position.y()),
            ]);
            if zone.boundary.intersects(&leg) {
                violations.push(SafetyViolation {
                    code: SafetyViolationCode::NoFlyZoneIntersection,
                    severity: SafetyViolationSeverity::Blocker,
                    waypoint_index: Some(leg_index + 1),
                    zone_id: Some(zone.id.clone()),
                    measured_value: None,
                    threshold_value: None,
                    unit: None,
                    message: format!("mission leg intersects no-fly zone {}", zone.id),
                });
            }
        }
    }

    for constraint in airspace_constraints {
        for (leg_index, pair) in mission.waypoints.windows(2).enumerate() {
            let leg = LineString::from(vec![
                (pair[0].position.x(), pair[0].position.y()),
                (pair[1].position.x(), pair[1].position.y()),
            ]);
            if constraint.boundary.intersects(&leg) {
                violations.push(SafetyViolation {
                    code: SafetyViolationCode::AirspaceIntersection,
                    severity: SafetyViolationSeverity::Blocker,
                    waypoint_index: Some(leg_index + 1),
                    zone_id: Some(constraint.id.clone()),
                    measured_value: None,
                    threshold_value: None,
                    unit: None,
                    message: format!(
                        "mission leg intersects airspace constraint {}",
                        constraint.id
                    ),
                });
            }
        }
    }

    DispatchSafetyReport { violations }
}

fn point_is_inside_or_on_boundary(boundary: &Polygon<f64>, point: &Point<f64>) -> bool {
    boundary.contains(point) || boundary.intersects(point)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Mission, Waypoint, WaypointType, WeatherData};
    use geo::{point, polygon};

    fn sample_mission() -> Mission {
        let area = polygon![
            (x: 0.0, y: 0.0),
            (x: 100.0, y: 0.0),
            (x: 100.0, y: 100.0),
            (x: 0.0, y: 100.0),
            (x: 0.0, y: 0.0),
        ];
        let mut mission = Mission::new(
            "Dispatch Mission".to_string(),
            "dispatch safety fixture".to_string(),
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

    fn sample_weather() -> WeatherData {
        WeatherData {
            temperature_celsius: 20.0,
            humidity_percent: 50.0,
            wind_speed_ms: 5.0,
            wind_direction_degrees: 180.0,
            precipitation_mm: 0.0,
            visibility_m: 10000.0,
            pressure_hpa: 1015.0,
            cloud_cover_percent: 30.0,
        }
    }

    #[test]
    fn dispatch_safety_allows_compliant_mission() {
        let mission = sample_mission();

        let report = evaluate_dispatch_safety(
            &mission,
            Some(point!(x: 20.0, y: 20.0)),
            &[],
            DispatchSafetyConfig {
                altitude_ceiling_m: 120.0,
            },
        );

        assert!(report.is_clear());
        assert!(report.violations.is_empty());
    }

    #[test]
    fn dispatch_safety_allows_arm_transition_for_compliant_mission() {
        let mut mission = sample_mission();
        mission
            .validate()
            .expect("fixture validates before dispatch");

        let report = mission
            .arm_with_dispatch_safety(
                Some(point!(x: 20.0, y: 20.0)),
                &[],
                DispatchSafetyConfig {
                    altitude_ceiling_m: 120.0,
                },
            )
            .expect("compliant dispatch may arm");

        assert!(report.is_clear());
        assert_eq!(mission.status, crate::MissionStatus::Armed);
    }

    #[test]
    fn dispatch_safety_blocks_no_fly_intersection() {
        let mission = sample_mission();
        let no_fly = NoFlyZone {
            id: "nfz-1".to_string(),
            boundary: polygon![
                (x: 45.0, y: 45.0),
                (x: 55.0, y: 45.0),
                (x: 55.0, y: 55.0),
                (x: 45.0, y: 55.0),
                (x: 45.0, y: 45.0),
            ],
        };

        let report = evaluate_dispatch_safety(
            &mission,
            Some(point!(x: 20.0, y: 20.0)),
            &[no_fly],
            DispatchSafetyConfig {
                altitude_ceiling_m: 120.0,
            },
        );

        assert!(!report.is_clear());
        assert_eq!(
            report.violations[0].code,
            SafetyViolationCode::NoFlyZoneIntersection
        );
        assert_eq!(report.violations[0].zone_id.as_deref(), Some("nfz-1"));
        assert_eq!(
            report.violations[0].severity,
            SafetyViolationSeverity::Blocker
        );
    }

    #[test]
    fn dispatch_safety_allows_weather_under_thresholds() {
        let mission = sample_mission();

        let report = evaluate_dispatch_safety_with_constraints(
            &mission,
            Some(point!(x: 20.0, y: 20.0)),
            &[],
            Some(&sample_weather()),
            &[],
            DispatchSafetyConfig {
                altitude_ceiling_m: 120.0,
            },
        );

        assert!(report.is_clear());
        assert!(report.violations.is_empty());
    }

    #[test]
    fn dispatch_safety_blocks_wind_and_precipitation_thresholds() {
        let mission = sample_mission();
        let mut weather = sample_weather();
        weather.wind_speed_ms = 18.5;
        weather.precipitation_mm = 3.25;

        let report = evaluate_dispatch_safety_with_constraints(
            &mission,
            Some(point!(x: 20.0, y: 20.0)),
            &[],
            Some(&weather),
            &[],
            DispatchSafetyConfig {
                altitude_ceiling_m: 120.0,
            },
        );

        assert!(!report.is_clear());
        assert_eq!(report.violations.len(), 2);
        assert_eq!(
            report.violations[0].code,
            SafetyViolationCode::WindSpeedExceeded
        );
        assert_eq!(report.violations[0].measured_value, Some(18.5));
        assert_eq!(report.violations[0].threshold_value, Some(15.0));
        assert_eq!(report.violations[0].unit.as_deref(), Some("m/s"));
        assert!(report.violations[0].message.contains("18.5"));
        assert!(report.violations[0].message.contains("15.0"));
        assert_eq!(
            report.violations[1].code,
            SafetyViolationCode::PrecipitationExceeded
        );
        assert_eq!(report.violations[1].measured_value, Some(3.25));
        assert_eq!(report.violations[1].threshold_value, Some(2.0));
        assert_eq!(report.violations[1].unit.as_deref(), Some("mm"));
    }

    #[test]
    fn dispatch_safety_blocks_airspace_intersection() {
        let mission = sample_mission();
        let restricted_airspace = AirspaceConstraint {
            id: "temporary-flight-restriction".to_string(),
            boundary: polygon![
                (x: 45.0, y: 45.0),
                (x: 55.0, y: 45.0),
                (x: 55.0, y: 55.0),
                (x: 45.0, y: 55.0),
                (x: 45.0, y: 45.0),
            ],
        };

        let report = evaluate_dispatch_safety_with_constraints(
            &mission,
            Some(point!(x: 20.0, y: 20.0)),
            &[],
            Some(&sample_weather()),
            &[restricted_airspace],
            DispatchSafetyConfig {
                altitude_ceiling_m: 120.0,
            },
        );

        assert!(!report.is_clear());
        assert_eq!(report.violations.len(), 1);
        assert_eq!(
            report.violations[0].code,
            SafetyViolationCode::AirspaceIntersection
        );
        assert_eq!(
            report.violations[0].zone_id.as_deref(),
            Some("temporary-flight-restriction")
        );
        assert_eq!(report.violations[0].waypoint_index, Some(1));
        assert_eq!(
            report.violations[0].severity,
            SafetyViolationSeverity::Blocker
        );
    }

    #[test]
    fn dispatch_safety_preserves_no_fly_zone_round_trip() {
        let mission = sample_mission();
        let no_fly = NoFlyZone {
            id: "nfz-1".to_string(),
            boundary: polygon![
                (x: 45.0, y: 45.0),
                (x: 55.0, y: 45.0),
                (x: 55.0, y: 55.0),
                (x: 45.0, y: 55.0),
                (x: 45.0, y: 45.0),
            ],
        };
        let encoded = serde_json::to_string(&no_fly).expect("no-fly zone serializes");
        let decoded: NoFlyZone = serde_json::from_str(&encoded).expect("no-fly zone deserializes");

        let report = evaluate_dispatch_safety(
            &mission,
            Some(point!(x: 20.0, y: 20.0)),
            &[decoded],
            DispatchSafetyConfig {
                altitude_ceiling_m: 120.0,
            },
        );

        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].zone_id.as_deref(), Some("nfz-1"));
    }

    #[test]
    fn dispatch_safety_blocks_arm_transition() {
        let mut mission = sample_mission();
        mission
            .validate()
            .expect("fixture validates before dispatch");
        let no_fly = NoFlyZone {
            id: "nfz-1".to_string(),
            boundary: polygon![
                (x: 45.0, y: 45.0),
                (x: 55.0, y: 45.0),
                (x: 55.0, y: 55.0),
                (x: 45.0, y: 55.0),
                (x: 45.0, y: 45.0),
            ],
        };

        let error = mission
            .arm_with_dispatch_safety(
                Some(point!(x: 20.0, y: 20.0)),
                &[no_fly],
                DispatchSafetyConfig {
                    altitude_ceiling_m: 120.0,
                },
            )
            .expect_err("no-fly intersection must block arming");

        match error {
            crate::DispatchSafetyError::SafetyViolation(report) => {
                assert_eq!(
                    report.violations[0].code,
                    SafetyViolationCode::NoFlyZoneIntersection
                );
            }
            crate::DispatchSafetyError::State(error) => {
                panic!("expected safety violation, got state error: {error}");
            }
        }
        assert_eq!(mission.status, crate::MissionStatus::Validated);
    }
}
