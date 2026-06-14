use crate::flight_path::{FlightPath, PathType, SurveyPattern};
use crate::{
    validate_waypoint_sanity, Mission, MissionLinkage, MissionStatus, Waypoint, WaypointType,
    WaypointValidationConfig,
};
use geo::{BoundingRect, Contains, Intersects, Point, Polygon};
use serde::{Deserialize, Serialize};
use std::fmt;

const MIN_SURVEY_POINTS: usize = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PlanBoundsConfig {
    pub max_altitude_m: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlanBoundsIssueCode {
    InvalidBoundary,
    InvalidAltitudeCeiling,
    AltitudeCeilingExceeded,
    OutsideGeofence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanBoundsIssue {
    pub waypoint_index: Option<usize>,
    pub code: PlanBoundsIssueCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanBoundsError {
    pub issues: Vec<PlanBoundsIssue>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct SurveyTemplateConfig {
    pub pattern: SurveyPattern,
    pub spacing_m: f64,
    pub overlap_percent: f32,
    pub altitude_m: f32,
    pub altitude_ceiling_m: f32,
    pub speed_ms: Option<f32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SurveyTemplateErrorCode {
    InvalidBoundary,
    InvalidSpacing,
    InvalidSpeed,
    InvalidOverlap,
    InvalidAltitude,
    UnsupportedPattern,
    SpacingExceedsExtent,
    PlanBoundsViolation,
    WaypointValidationFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SurveyTemplateError {
    pub code: SurveyTemplateErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurveyTemplateResult {
    pub mission: Mission,
    pub coverage_fraction: f64,
    pub leg_count: usize,
}

pub fn validate_plan_bounds(
    waypoints: &[Waypoint],
    boundary: &Polygon<f64>,
    config: PlanBoundsConfig,
) -> Result<(), PlanBoundsError> {
    let mut issues = Vec::new();

    if !is_valid_boundary(boundary) {
        issues.push(PlanBoundsIssue {
            waypoint_index: None,
            code: PlanBoundsIssueCode::InvalidBoundary,
            message: "field boundary must be a closed polygon with at least four coordinates"
                .to_string(),
        });
    }

    if !config.max_altitude_m.is_finite() || config.max_altitude_m <= 0.0 {
        issues.push(PlanBoundsIssue {
            waypoint_index: None,
            code: PlanBoundsIssueCode::InvalidAltitudeCeiling,
            message: "altitude ceiling must be finite and positive".to_string(),
        });
    }

    if !issues.is_empty() {
        return Err(PlanBoundsError { issues });
    }

    for (index, waypoint) in waypoints.iter().enumerate() {
        if waypoint.altitude_m > config.max_altitude_m {
            issues.push(PlanBoundsIssue {
                waypoint_index: Some(index),
                code: PlanBoundsIssueCode::AltitudeCeilingExceeded,
                message: format!(
                    "waypoint altitude {:.1} m exceeds ceiling {:.1} m",
                    waypoint.altitude_m, config.max_altitude_m
                ),
            });
        }

        if !point_is_inside_or_on_boundary(boundary, &waypoint.position) {
            issues.push(PlanBoundsIssue {
                waypoint_index: Some(index),
                code: PlanBoundsIssueCode::OutsideGeofence,
                message: "waypoint lies outside the field geofence".to_string(),
            });
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(PlanBoundsError { issues })
    }
}

pub fn generate_survey_template(
    name: String,
    description: String,
    boundary: Polygon<f64>,
    linkage: MissionLinkage,
    config: SurveyTemplateConfig,
) -> Result<SurveyTemplateResult, SurveyTemplateError> {
    validate_survey_config(&boundary, config)?;
    let lane_spacing_m = effective_spacing_m(config)?;

    let survey_points = match config.pattern {
        SurveyPattern::Grid => generate_grid_points(&boundary, lane_spacing_m)?,
        SurveyPattern::Lawnmower | SurveyPattern::Zigzag => {
            generate_lawnmower_points(&boundary, lane_spacing_m)?
        }
        SurveyPattern::Perimeter => generate_perimeter_points(&boundary)?,
        SurveyPattern::Spiral | SurveyPattern::RandomWalk | SurveyPattern::AdaptiveSampling => {
            return Err(SurveyTemplateError::new(
                SurveyTemplateErrorCode::UnsupportedPattern,
                format!(
                    "{:?} survey templates are not deterministic coverage templates",
                    config.pattern
                ),
            ));
        }
    };

    let mut waypoints = waypoints_from_survey_points(&survey_points, config);
    validate_plan_bounds(
        &waypoints,
        &boundary,
        PlanBoundsConfig {
            max_altitude_m: config.altitude_ceiling_m,
        },
    )
    .map_err(|error| {
        SurveyTemplateError::new(
            SurveyTemplateErrorCode::PlanBoundsViolation,
            error.to_string(),
        )
    })?;
    validate_waypoint_sanity(&waypoints, WaypointValidationConfig::default()).map_err(|error| {
        SurveyTemplateError::new(
            SurveyTemplateErrorCode::WaypointValidationFailed,
            error.to_string(),
        )
    })?;

    let mut mission = Mission::new_linked(name, description, boundary.clone(), linkage);
    for waypoint in waypoints.drain(..) {
        mission.add_waypoint(waypoint);
    }
    mission.validate().map_err(|error| {
        SurveyTemplateError::new(
            SurveyTemplateErrorCode::WaypointValidationFailed,
            error.to_string(),
        )
    })?;
    let flight_path = FlightPath::from_waypoints(
        "Survey Template Path".to_string(),
        &mission.waypoints,
        PathType::Survey {
            pattern: config.pattern,
            overlap_percent: config.overlap_percent,
        },
    );
    mission.estimated_duration_minutes = (flight_path.estimated_duration_seconds + 59) / 60;
    mission.add_flight_path(flight_path);
    debug_assert_eq!(mission.status, MissionStatus::Validated);

    Ok(SurveyTemplateResult {
        coverage_fraction: estimate_coverage_fraction(&boundary, &survey_points, lane_spacing_m),
        leg_count: mission.waypoints.len().saturating_sub(1),
        mission,
    })
}

fn validate_survey_config(
    boundary: &Polygon<f64>,
    config: SurveyTemplateConfig,
) -> Result<(), SurveyTemplateError> {
    if !is_valid_boundary(boundary) || boundary.bounding_rect().is_none() {
        return Err(SurveyTemplateError::new(
            SurveyTemplateErrorCode::InvalidBoundary,
            "field boundary must be a closed polygon with a non-empty extent",
        ));
    }
    if !config.spacing_m.is_finite() || config.spacing_m <= 0.0 {
        return Err(SurveyTemplateError::new(
            SurveyTemplateErrorCode::InvalidSpacing,
            "survey spacing must be finite and positive",
        ));
    }
    if !config.overlap_percent.is_finite() || !(0.0..100.0).contains(&config.overlap_percent) {
        return Err(SurveyTemplateError::new(
            SurveyTemplateErrorCode::InvalidOverlap,
            "overlap percent must be in [0, 100)",
        ));
    }
    if !config.altitude_m.is_finite()
        || config.altitude_m <= 0.0
        || !config.altitude_ceiling_m.is_finite()
        || config.altitude_ceiling_m <= 0.0
        || config.altitude_m > config.altitude_ceiling_m
    {
        return Err(SurveyTemplateError::new(
            SurveyTemplateErrorCode::InvalidAltitude,
            "survey altitude must be positive and no higher than the ceiling",
        ));
    }
    if let Some(speed_ms) = config.speed_ms {
        if !speed_ms.is_finite() || speed_ms <= 0.0 {
            return Err(SurveyTemplateError::new(
                SurveyTemplateErrorCode::InvalidSpeed,
                "survey speed must be positive when provided",
            ));
        }
    }
    Ok(())
}

fn effective_spacing_m(config: SurveyTemplateConfig) -> Result<f64, SurveyTemplateError> {
    let spacing = config.spacing_m * (1.0 - f64::from(config.overlap_percent) / 100.0);
    if !spacing.is_finite() || spacing <= 0.0 {
        return Err(SurveyTemplateError::new(
            SurveyTemplateErrorCode::InvalidSpacing,
            "effective survey spacing must be finite and positive",
        ));
    }
    Ok(spacing)
}

fn generate_lawnmower_points(
    boundary: &Polygon<f64>,
    lane_spacing_m: f64,
) -> Result<Vec<Point<f64>>, SurveyTemplateError> {
    let rect = boundary.bounding_rect().ok_or_else(|| {
        SurveyTemplateError::new(
            SurveyTemplateErrorCode::InvalidBoundary,
            "field boundary must have a non-empty extent",
        )
    })?;
    let width = rect.max().x - rect.min().x;
    let height = rect.max().y - rect.min().y;
    ensure_spacing_fits_extent(width, height, lane_spacing_m)?;

    let margin = inside_margin(width, height, lane_spacing_m);
    let min_x = rect.min().x + margin;
    let max_x = rect.max().x - margin;
    let min_y = rect.min().y + margin;
    let max_y = rect.max().y - margin;

    let mut points = Vec::new();
    let mut y = min_y;
    let mut left_to_right = true;
    while y <= max_y {
        let left = Point::new(min_x, y);
        let right = Point::new(max_x, y);
        if point_is_inside_or_on_boundary(boundary, &left)
            && point_is_inside_or_on_boundary(boundary, &right)
        {
            if left_to_right {
                push_point(&mut points, left);
                push_point(&mut points, right);
            } else {
                push_point(&mut points, right);
                push_point(&mut points, left);
            }
            left_to_right = !left_to_right;
        }
        y += lane_spacing_m;
    }

    ensure_enough_survey_points(points)
}

fn generate_grid_points(
    boundary: &Polygon<f64>,
    lane_spacing_m: f64,
) -> Result<Vec<Point<f64>>, SurveyTemplateError> {
    let rect = boundary.bounding_rect().ok_or_else(|| {
        SurveyTemplateError::new(
            SurveyTemplateErrorCode::InvalidBoundary,
            "field boundary must have a non-empty extent",
        )
    })?;
    let width = rect.max().x - rect.min().x;
    let height = rect.max().y - rect.min().y;
    ensure_spacing_fits_extent(width, height, lane_spacing_m)?;

    let mut points = generate_lawnmower_points(boundary, lane_spacing_m)?;
    let margin = inside_margin(width, height, lane_spacing_m);
    let min_x = rect.min().x + margin;
    let max_x = rect.max().x - margin;
    let min_y = rect.min().y + margin;
    let max_y = rect.max().y - margin;

    let mut x = min_x;
    let mut bottom_to_top = true;
    while x <= max_x {
        let bottom = Point::new(x, min_y);
        let top = Point::new(x, max_y);
        if point_is_inside_or_on_boundary(boundary, &bottom)
            && point_is_inside_or_on_boundary(boundary, &top)
        {
            if bottom_to_top {
                push_point(&mut points, bottom);
                push_point(&mut points, top);
            } else {
                push_point(&mut points, top);
                push_point(&mut points, bottom);
            }
            bottom_to_top = !bottom_to_top;
        }
        x += lane_spacing_m;
    }

    ensure_enough_survey_points(points)
}

fn generate_perimeter_points(
    boundary: &Polygon<f64>,
) -> Result<Vec<Point<f64>>, SurveyTemplateError> {
    let points = boundary
        .exterior()
        .points()
        .map(|point| Point::new(point.x(), point.y()))
        .collect::<Vec<_>>();

    ensure_enough_survey_points(points)
}

fn waypoints_from_survey_points(
    survey_points: &[Point<f64>],
    config: SurveyTemplateConfig,
) -> Vec<Waypoint> {
    let mut waypoints = Vec::with_capacity(survey_points.len() + 2);
    let first = *survey_points
        .first()
        .expect("survey points are validated before waypoint creation");
    let last = *survey_points
        .last()
        .expect("survey points are validated before waypoint creation");

    waypoints.push(apply_speed(
        Waypoint::new(first, 0.0, WaypointType::Takeoff),
        config.speed_ms,
    ));
    for point in survey_points {
        waypoints.push(apply_speed(
            Waypoint::new(*point, config.altitude_m, WaypointType::Survey),
            config.speed_ms,
        ));
    }
    waypoints.push(apply_speed(
        Waypoint::new(last, 0.0, WaypointType::Landing),
        config.speed_ms,
    ));
    waypoints
}

fn apply_speed(waypoint: Waypoint, speed_ms: Option<f32>) -> Waypoint {
    match speed_ms {
        Some(speed_ms) => waypoint.with_speed(speed_ms),
        None => waypoint,
    }
}

fn estimate_coverage_fraction(
    boundary: &Polygon<f64>,
    survey_points: &[Point<f64>],
    lane_spacing_m: f64,
) -> f64 {
    use geo::Area;

    let area = boundary.unsigned_area();
    if area <= 0.0 {
        return 0.0;
    }
    let sampled_area = survey_points.len() as f64 * lane_spacing_m * lane_spacing_m;
    (sampled_area / area).clamp(0.0, 1.0)
}

fn ensure_spacing_fits_extent(
    width: f64,
    height: f64,
    lane_spacing_m: f64,
) -> Result<(), SurveyTemplateError> {
    if width <= 0.0 || height <= 0.0 {
        return Err(SurveyTemplateError::new(
            SurveyTemplateErrorCode::InvalidBoundary,
            "field boundary extent must be positive",
        ));
    }
    if lane_spacing_m > width || lane_spacing_m > height {
        return Err(SurveyTemplateError::new(
            SurveyTemplateErrorCode::SpacingExceedsExtent,
            format!(
                "spacing exceeds extent: {:.1} m spacing for {:.1} x {:.1} m field",
                lane_spacing_m, width, height
            ),
        ));
    }
    Ok(())
}

fn ensure_enough_survey_points(
    points: Vec<Point<f64>>,
) -> Result<Vec<Point<f64>>, SurveyTemplateError> {
    if points.len() < MIN_SURVEY_POINTS {
        return Err(SurveyTemplateError::new(
            SurveyTemplateErrorCode::SpacingExceedsExtent,
            "spacing exceeds extent and produced no survey legs",
        ));
    }
    Ok(points)
}

fn inside_margin(width: f64, height: f64, lane_spacing_m: f64) -> f64 {
    (width.min(height) / 100.0)
        .min(lane_spacing_m / 4.0)
        .max(0.001)
}

fn push_point(points: &mut Vec<Point<f64>>, point: Point<f64>) {
    if points
        .last()
        .map(|last| last.x() == point.x() && last.y() == point.y())
        .unwrap_or(false)
    {
        return;
    }
    points.push(point);
}

fn is_valid_boundary(boundary: &Polygon<f64>) -> bool {
    let coordinates = &boundary.exterior().0;
    if coordinates.len() < 4 {
        return false;
    }
    match (coordinates.first(), coordinates.last()) {
        (Some(first), Some(last)) => first.x == last.x && first.y == last.y,
        _ => false,
    }
}

fn point_is_inside_or_on_boundary(boundary: &Polygon<f64>, point: &Point<f64>) -> bool {
    boundary.contains(point) || boundary.intersects(point)
}

impl PlanBoundsError {
    pub fn primary_code(&self) -> Option<PlanBoundsIssueCode> {
        self.issues.first().map(|issue| issue.code)
    }
}

impl fmt::Display for PlanBoundsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(issue) = self.issues.first() {
            write!(formatter, "plan bounds validation failed: {:?}", issue.code)
        } else {
            formatter.write_str("plan bounds validation failed")
        }
    }
}

impl std::error::Error for PlanBoundsError {}

impl SurveyTemplateError {
    fn new(code: SurveyTemplateErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for SurveyTemplateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for SurveyTemplateError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight_path::{PathType, SurveyPattern};
    use crate::{MissionLinkage, MissionStatus, Waypoint, WaypointType};
    use geo::{point, polygon};

    fn sample_boundary() -> geo::Polygon<f64> {
        polygon![
            (x: 0.0, y: 0.0),
            (x: 100.0, y: 0.0),
            (x: 100.0, y: 60.0),
            (x: 0.0, y: 60.0),
            (x: 0.0, y: 0.0),
        ]
    }

    #[test]
    fn plan_bounds_reject_altitude_above_ceiling() {
        let boundary = sample_boundary();
        let waypoints = vec![
            Waypoint::new(point!(x: 10.0, y: 10.0), 20.0, WaypointType::Takeoff),
            Waypoint::new(point!(x: 20.0, y: 20.0), 130.0, WaypointType::Survey),
            Waypoint::new(point!(x: 30.0, y: 30.0), 0.0, WaypointType::Landing),
        ];

        let error = validate_plan_bounds(
            &waypoints,
            &boundary,
            PlanBoundsConfig {
                max_altitude_m: 120.0,
            },
        )
        .expect_err("over-ceiling waypoint should be rejected");

        assert_eq!(
            error.issues[0].code,
            PlanBoundsIssueCode::AltitudeCeilingExceeded
        );
        assert_eq!(error.issues[0].waypoint_index, Some(1));
    }

    #[test]
    fn plan_bounds_reject_outside_geofence() {
        let boundary = sample_boundary();
        let waypoints = vec![
            Waypoint::new(point!(x: 10.0, y: 10.0), 20.0, WaypointType::Takeoff),
            Waypoint::new(point!(x: 120.0, y: 20.0), 20.0, WaypointType::Survey),
            Waypoint::new(point!(x: 30.0, y: 30.0), 0.0, WaypointType::Landing),
        ];

        let error = validate_plan_bounds(
            &waypoints,
            &boundary,
            PlanBoundsConfig {
                max_altitude_m: 120.0,
            },
        )
        .expect_err("outside-geofence waypoint should be rejected");

        assert_eq!(error.issues[0].code, PlanBoundsIssueCode::OutsideGeofence);
        assert_eq!(error.issues[0].waypoint_index, Some(1));
    }

    #[test]
    fn lawnmower_template_stays_inside_boundary_and_reports_coverage() {
        let boundary = sample_boundary();
        let result = generate_survey_template(
            "North Field Capture".to_string(),
            "deterministic lawnmower coverage".to_string(),
            boundary.clone(),
            MissionLinkage::new(
                "field-1".to_string(),
                "season-2026".to_string(),
                Some("session-1".to_string()),
                "owner-1".to_string(),
            ),
            SurveyTemplateConfig {
                pattern: SurveyPattern::Lawnmower,
                spacing_m: 20.0,
                overlap_percent: 0.0,
                altitude_m: 40.0,
                altitude_ceiling_m: 120.0,
                speed_ms: Some(8.0),
            },
        )
        .expect("template should be generated");

        assert_eq!(result.mission.status, MissionStatus::Validated);
        assert_eq!(result.mission.field_id, "field-1");
        assert!(result.coverage_fraction > 0.0);
        assert!(result.coverage_fraction <= 1.0);
        assert_eq!(result.leg_count, result.mission.waypoints.len() - 1);
        assert!(matches!(
            result.mission.flight_paths[0].path_type,
            PathType::Survey {
                pattern: SurveyPattern::Lawnmower,
                ..
            }
        ));
        assert_eq!(
            result
                .mission
                .waypoints
                .first()
                .map(|waypoint| &waypoint.waypoint_type),
            Some(&WaypointType::Takeoff)
        );
        assert_eq!(
            result
                .mission
                .waypoints
                .last()
                .map(|waypoint| &waypoint.waypoint_type),
            Some(&WaypointType::Landing)
        );
        validate_plan_bounds(
            &result.mission.waypoints,
            &boundary,
            PlanBoundsConfig {
                max_altitude_m: 120.0,
            },
        )
        .expect("generated mission should stay inside its boundary");
    }

    #[test]
    fn grid_and_perimeter_templates_generate_valid_missions() {
        for pattern in [SurveyPattern::Grid, SurveyPattern::Perimeter] {
            let boundary = sample_boundary();
            let result = generate_survey_template(
                format!("{pattern:?} Capture"),
                "deterministic coverage".to_string(),
                boundary.clone(),
                MissionLinkage::unassigned(),
                SurveyTemplateConfig {
                    pattern,
                    spacing_m: 20.0,
                    overlap_percent: 10.0,
                    altitude_m: 40.0,
                    altitude_ceiling_m: 120.0,
                    speed_ms: None,
                },
            )
            .expect("template should be generated");

            assert_eq!(result.mission.status, MissionStatus::Validated);
            assert!(result.coverage_fraction > 0.0);
            validate_plan_bounds(
                &result.mission.waypoints,
                &boundary,
                PlanBoundsConfig {
                    max_altitude_m: 120.0,
                },
            )
            .expect("generated mission should stay inside its boundary");
        }
    }

    #[test]
    fn survey_template_rejects_spacing_larger_than_extent() {
        let error = generate_survey_template(
            "Too Sparse".to_string(),
            "invalid spacing".to_string(),
            sample_boundary(),
            MissionLinkage::unassigned(),
            SurveyTemplateConfig {
                pattern: SurveyPattern::Lawnmower,
                spacing_m: 500.0,
                overlap_percent: 0.0,
                altitude_m: 40.0,
                altitude_ceiling_m: 120.0,
                speed_ms: None,
            },
        )
        .expect_err("spacing larger than the field extent should fail");

        assert_eq!(error.code, SurveyTemplateErrorCode::SpacingExceedsExtent);
        assert!(error.to_string().contains("spacing exceeds extent"));
    }
}
