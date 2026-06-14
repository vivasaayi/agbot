use crate::{
    MultiDroneController, SafetyViolation, Severity, SwarmActionSafetyError, SwarmActionTarget,
    ViolationType,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SynchronizedSurveyConfig {
    pub planned_altitude_m: f32,
    pub min_separation_m: f64,
    pub overlap_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoverageOptimizationConfig {
    pub planned_altitude_m: f32,
    pub min_separation_m: f64,
    pub overlap_percent: f32,
    pub sensor_swath_width_m: f64,
    pub survey_speed_mps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SurveyExecutionStatus {
    Planned,
    Halted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SurveyLane {
    pub drone_id: Uuid,
    pub lane_index: usize,
    pub start_xy: (f64, f64),
    pub end_xy: (f64, f64),
    pub lane_width_m: f64,
    pub planned_altitude_m: f32,
    pub synchronized_start_offset_s: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SurveyAuditEvent {
    pub swarm_id: Uuid,
    pub at: DateTime<Utc>,
    pub status: SurveyExecutionStatus,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SynchronizedSurveyPlan {
    pub swarm_id: Uuid,
    pub status: SurveyExecutionStatus,
    pub lanes: Vec<SurveyLane>,
    pub coverage_fraction: f32,
    pub separation_violations: Vec<SafetyViolation>,
    pub abort_drone_ids: Vec<Uuid>,
    pub audit: Vec<SurveyAuditEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DroneCoverageTime {
    pub drone_id: Uuid,
    pub lane_count: usize,
    pub planned_time_s: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoverageOptimizationPlan {
    pub swarm_id: Uuid,
    pub status: SurveyExecutionStatus,
    pub lanes: Vec<SurveyLane>,
    pub coverage_fraction: f32,
    pub required_lane_count: usize,
    pub pass_count: usize,
    pub requires_multi_pass: bool,
    pub per_drone_time_s: Vec<DroneCoverageTime>,
    pub balanced_time_spread_s: f64,
    pub separation_violations: Vec<SafetyViolation>,
    pub audit: Vec<SurveyAuditEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SurveySeparationSample {
    pub elapsed_s: f64,
    pub positions: Vec<(Uuid, (f64, f64, f32))>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SurveyProgressReport {
    pub swarm_id: Uuid,
    pub status: SurveyExecutionStatus,
    pub checked_at: DateTime<Utc>,
    pub separation_violations: Vec<SafetyViolation>,
    pub abort_drone_ids: Vec<Uuid>,
    pub audit: Vec<SurveyAuditEvent>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SynchronizedSurveyError {
    #[error("swarm {swarm_id} not found")]
    SwarmNotFound { swarm_id: Uuid },
    #[error("swarm {swarm_id} has no drones")]
    EmptySwarm { swarm_id: Uuid },
    #[error("field boundary must contain at least three points")]
    InvalidBoundary,
    #[error("synchronized survey config is invalid: {reason}")]
    InvalidConfig { reason: String },
    #[error("synchronized survey rejected by safety constraints: {reason}")]
    SafetyRejected { reason: String },
}

impl Default for SynchronizedSurveyConfig {
    fn default() -> Self {
        Self {
            planned_altitude_m: 30.0,
            min_separation_m: 25.0,
            overlap_percent: 10.0,
        }
    }
}

impl Default for CoverageOptimizationConfig {
    fn default() -> Self {
        Self {
            planned_altitude_m: 30.0,
            min_separation_m: 25.0,
            overlap_percent: 10.0,
            sensor_swath_width_m: 40.0,
            survey_speed_mps: 10.0,
        }
    }
}

impl MultiDroneController {
    pub fn plan_synchronized_survey(
        &self,
        swarm_id: Uuid,
        field_boundary: Vec<(f64, f64)>,
        config: SynchronizedSurveyConfig,
        checked_at: DateTime<Utc>,
    ) -> Result<SynchronizedSurveyPlan, SynchronizedSurveyError> {
        plan_synchronized_survey(self, swarm_id, field_boundary, config, checked_at)
    }

    pub fn plan_coverage_optimization(
        &self,
        swarm_id: Uuid,
        field_boundary: Vec<(f64, f64)>,
        config: CoverageOptimizationConfig,
        checked_at: DateTime<Utc>,
    ) -> Result<CoverageOptimizationPlan, SynchronizedSurveyError> {
        plan_coverage_optimization(self, swarm_id, field_boundary, config, checked_at)
    }
}

pub fn plan_synchronized_survey(
    controller: &MultiDroneController,
    swarm_id: Uuid,
    field_boundary: Vec<(f64, f64)>,
    config: SynchronizedSurveyConfig,
    checked_at: DateTime<Utc>,
) -> Result<SynchronizedSurveyPlan, SynchronizedSurveyError> {
    validate_config(&config)?;
    let swarm = controller
        .get_swarm(&swarm_id)
        .ok_or(SynchronizedSurveyError::SwarmNotFound { swarm_id })?;
    let drone_ids = swarm.drone_ids();
    if drone_ids.is_empty() {
        return Err(SynchronizedSurveyError::EmptySwarm { swarm_id });
    }
    if drone_ids.len() > controller.global_constraints.max_concurrent_drones as usize {
        return Err(SynchronizedSurveyError::SafetyRejected {
            reason: format!(
                "swarm has {} drones but max concurrent drones is {}",
                drone_ids.len(),
                controller.global_constraints.max_concurrent_drones
            ),
        });
    }

    let bounds = BoundaryBounds::from_points(&field_boundary)?;
    let lanes = partition_lanes(&drone_ids, &bounds, &config);
    let endpoint_targets = lane_endpoint_targets(&lanes);
    controller
        .validate_swarm_action_targets(
            format!("swarm:{swarm_id}:synchronized_survey"),
            &endpoint_targets,
            checked_at,
        )
        .map_err(safety_error_to_survey_error)?;

    let coverage = coverage_fraction(&lanes, bounds.area_m2());
    let progress = evaluate_synchronized_survey_progress(
        &SynchronizedSurveyPlan {
            swarm_id,
            status: SurveyExecutionStatus::Planned,
            lanes: lanes.clone(),
            coverage_fraction: coverage,
            separation_violations: Vec::new(),
            abort_drone_ids: Vec::new(),
            audit: Vec::new(),
        },
        &[SurveySeparationSample {
            elapsed_s: 0.0,
            positions: lanes
                .iter()
                .map(|lane| {
                    (
                        lane.drone_id,
                        (lane.start_xy.0, lane.start_xy.1, lane.planned_altitude_m),
                    )
                })
                .collect(),
        }],
        config,
        checked_at,
    );

    let message = if progress.separation_violations.is_empty() {
        format!(
            "{} synchronized lane(s) planned over boundary with {:.1}% coverage",
            lanes.len(),
            progress_from_lanes(&lanes, bounds.area_m2()) * 100.0
        )
    } else {
        "synchronized survey rejected by planned separation breach".to_string()
    };
    let status = progress.status.clone();

    Ok(SynchronizedSurveyPlan {
        swarm_id,
        status: status.clone(),
        lanes,
        coverage_fraction: coverage,
        separation_violations: progress.separation_violations,
        abort_drone_ids: progress.abort_drone_ids,
        audit: vec![SurveyAuditEvent {
            swarm_id,
            at: checked_at,
            status,
            message,
        }],
    })
}

pub fn plan_coverage_optimization(
    controller: &MultiDroneController,
    swarm_id: Uuid,
    field_boundary: Vec<(f64, f64)>,
    config: CoverageOptimizationConfig,
    checked_at: DateTime<Utc>,
) -> Result<CoverageOptimizationPlan, SynchronizedSurveyError> {
    validate_coverage_config(&config)?;
    let swarm = controller
        .get_swarm(&swarm_id)
        .ok_or(SynchronizedSurveyError::SwarmNotFound { swarm_id })?;
    let drone_ids = swarm.drone_ids();
    if drone_ids.is_empty() {
        return Err(SynchronizedSurveyError::EmptySwarm { swarm_id });
    }
    if drone_ids.len() > controller.global_constraints.max_concurrent_drones as usize {
        return Err(SynchronizedSurveyError::SafetyRejected {
            reason: format!(
                "swarm has {} drones but max concurrent drones is {}",
                drone_ids.len(),
                controller.global_constraints.max_concurrent_drones
            ),
        });
    }

    let bounds = BoundaryBounds::from_points(&field_boundary)?;
    let required_lane_count = required_coverage_lane_count(&bounds, &config);
    let pass_count = required_lane_count.div_ceil(drone_ids.len());
    let lanes = optimized_coverage_lanes(&drone_ids, &bounds, &config, required_lane_count);
    let coverage = coverage_fraction(&lanes, bounds.area_m2());

    controller
        .validate_swarm_action_targets(
            format!("swarm:{swarm_id}:coverage_optimization"),
            &lane_endpoint_targets(&lanes),
            checked_at,
        )
        .map_err(safety_error_to_survey_error)?;

    let progress = evaluate_synchronized_survey_progress(
        &SynchronizedSurveyPlan {
            swarm_id,
            status: SurveyExecutionStatus::Planned,
            lanes: lanes.clone(),
            coverage_fraction: coverage,
            separation_violations: Vec::new(),
            abort_drone_ids: Vec::new(),
            audit: Vec::new(),
        },
        &coverage_start_samples_by_pass(&lanes),
        SynchronizedSurveyConfig {
            planned_altitude_m: config.planned_altitude_m,
            min_separation_m: config.min_separation_m,
            overlap_percent: config.overlap_percent,
        },
        checked_at,
    );

    let lane_length_m = coverage_lane_length(&bounds);
    let lane_time_s = lane_length_m / config.survey_speed_mps;
    let per_drone_time_s = per_drone_coverage_time(&drone_ids, &lanes, lane_time_s);
    let balanced_time_spread_s = balanced_time_spread(&per_drone_time_s);
    let requires_multi_pass = pass_count > 1;
    let status = progress.status.clone();
    let message = if requires_multi_pass {
        format!(
            "{} coverage lane(s) require {} pass(es); multi-pass plan covers {:.1}% of boundary",
            required_lane_count,
            pass_count,
            coverage * 100.0
        )
    } else {
        format!(
            "{} coverage lane(s) planned in one pass with {:.1}% coverage",
            required_lane_count,
            coverage * 100.0
        )
    };

    Ok(CoverageOptimizationPlan {
        swarm_id,
        status: status.clone(),
        lanes,
        coverage_fraction: coverage,
        required_lane_count,
        pass_count,
        requires_multi_pass,
        per_drone_time_s,
        balanced_time_spread_s,
        separation_violations: progress.separation_violations,
        audit: vec![SurveyAuditEvent {
            swarm_id,
            at: checked_at,
            status,
            message,
        }],
    })
}

pub fn evaluate_synchronized_survey_progress(
    plan: &SynchronizedSurveyPlan,
    samples: &[SurveySeparationSample],
    config: SynchronizedSurveyConfig,
    checked_at: DateTime<Utc>,
) -> SurveyProgressReport {
    let mut violations = Vec::new();

    for sample in samples {
        for left_index in 0..sample.positions.len() {
            for right_index in (left_index + 1)..sample.positions.len() {
                let (left_id, left_position) = sample.positions[left_index];
                let (right_id, right_position) = sample.positions[right_index];
                let distance_m = distance_3d(left_position, right_position);
                if distance_m < config.min_separation_m {
                    violations.push(separation_violation(
                        left_id,
                        left_position,
                        distance_m,
                        config.min_separation_m,
                        checked_at,
                    ));
                    violations.push(separation_violation(
                        right_id,
                        right_position,
                        distance_m,
                        config.min_separation_m,
                        checked_at,
                    ));
                }
            }
        }
    }

    let mut abort_drone_ids = violations
        .iter()
        .map(|violation| violation.drone_id)
        .collect::<Vec<_>>();
    abort_drone_ids.sort();
    abort_drone_ids.dedup();

    let status = if violations.is_empty() {
        SurveyExecutionStatus::Planned
    } else {
        SurveyExecutionStatus::Halted
    };
    let audit_message = if violations.is_empty() {
        "synchronized survey progress clear".to_string()
    } else {
        format!(
            "separation breach halted synchronized survey; {} drone(s) marked for abort",
            abort_drone_ids.len()
        )
    };

    SurveyProgressReport {
        swarm_id: plan.swarm_id,
        status: status.clone(),
        checked_at,
        separation_violations: violations,
        abort_drone_ids,
        audit: vec![SurveyAuditEvent {
            swarm_id: plan.swarm_id,
            at: checked_at,
            status,
            message: audit_message,
        }],
    }
}

#[derive(Debug, Clone, Copy)]
struct BoundaryBounds {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl BoundaryBounds {
    fn from_points(points: &[(f64, f64)]) -> Result<Self, SynchronizedSurveyError> {
        if points.len() < 3 {
            return Err(SynchronizedSurveyError::InvalidBoundary);
        }
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for (x, y) in points {
            if !x.is_finite() || !y.is_finite() {
                return Err(SynchronizedSurveyError::InvalidBoundary);
            }
            min_x = min_x.min(*x);
            max_x = max_x.max(*x);
            min_y = min_y.min(*y);
            max_y = max_y.max(*y);
        }

        if max_x <= min_x || max_y <= min_y {
            return Err(SynchronizedSurveyError::InvalidBoundary);
        }

        Ok(Self {
            min_x,
            max_x,
            min_y,
            max_y,
        })
    }

    fn width_m(self) -> f64 {
        self.max_x - self.min_x
    }

    fn height_m(self) -> f64 {
        self.max_y - self.min_y
    }

    fn area_m2(self) -> f64 {
        self.width_m() * self.height_m()
    }
}

fn validate_config(config: &SynchronizedSurveyConfig) -> Result<(), SynchronizedSurveyError> {
    if !config.min_separation_m.is_finite() || config.min_separation_m <= 0.0 {
        return Err(SynchronizedSurveyError::InvalidConfig {
            reason: "min_separation_m must be positive".to_string(),
        });
    }
    if !config.planned_altitude_m.is_finite() || config.planned_altitude_m < 0.0 {
        return Err(SynchronizedSurveyError::InvalidConfig {
            reason: "planned_altitude_m must be non-negative".to_string(),
        });
    }
    if !config.overlap_percent.is_finite() || config.overlap_percent < 0.0 {
        return Err(SynchronizedSurveyError::InvalidConfig {
            reason: "overlap_percent must be non-negative".to_string(),
        });
    }
    Ok(())
}

fn validate_coverage_config(
    config: &CoverageOptimizationConfig,
) -> Result<(), SynchronizedSurveyError> {
    validate_config(&SynchronizedSurveyConfig {
        planned_altitude_m: config.planned_altitude_m,
        min_separation_m: config.min_separation_m,
        overlap_percent: config.overlap_percent,
    })?;
    if config.overlap_percent >= 100.0 {
        return Err(SynchronizedSurveyError::InvalidConfig {
            reason: "overlap_percent must be below 100 for coverage optimization".to_string(),
        });
    }
    if !config.sensor_swath_width_m.is_finite() || config.sensor_swath_width_m <= 0.0 {
        return Err(SynchronizedSurveyError::InvalidConfig {
            reason: "sensor_swath_width_m must be positive".to_string(),
        });
    }
    if !config.survey_speed_mps.is_finite() || config.survey_speed_mps <= 0.0 {
        return Err(SynchronizedSurveyError::InvalidConfig {
            reason: "survey_speed_mps must be positive".to_string(),
        });
    }
    Ok(())
}

fn required_coverage_lane_count(
    bounds: &BoundaryBounds,
    config: &CoverageOptimizationConfig,
) -> usize {
    let coverage_width_m = bounds.width_m().max(bounds.height_m());
    let lane_spacing_m =
        config.sensor_swath_width_m * (1.0 - f64::from(config.overlap_percent) / 100.0);
    (coverage_width_m / lane_spacing_m).ceil().max(1.0) as usize
}

fn optimized_coverage_lanes(
    drone_ids: &[Uuid],
    bounds: &BoundaryBounds,
    config: &CoverageOptimizationConfig,
    required_lane_count: usize,
) -> Vec<SurveyLane> {
    let split_along_x = bounds.width_m() >= bounds.height_m();
    let coverage_width_m = if split_along_x {
        bounds.width_m()
    } else {
        bounds.height_m()
    };
    let lane_spacing_m = coverage_width_m / required_lane_count as f64;
    let lane_time_s = coverage_lane_length(bounds) / config.survey_speed_mps;

    (0..required_lane_count)
        .map(|lane_index| {
            let pass_index = lane_index / drone_ids.len();
            let drone_index = lane_index % drone_ids.len();
            let center_offset = lane_spacing_m * (lane_index as f64 + 0.5);
            let (start_xy, end_xy) = if split_along_x {
                let x = bounds.min_x + center_offset;
                ((x, bounds.min_y), (x, bounds.max_y))
            } else {
                let y = bounds.min_y + center_offset;
                ((bounds.min_x, y), (bounds.max_x, y))
            };

            SurveyLane {
                drone_id: drone_ids[drone_index],
                lane_index,
                start_xy,
                end_xy,
                lane_width_m: config.sensor_swath_width_m,
                planned_altitude_m: config.planned_altitude_m,
                synchronized_start_offset_s: pass_index as f64 * lane_time_s,
            }
        })
        .collect()
}

fn coverage_start_samples_by_pass(lanes: &[SurveyLane]) -> Vec<SurveySeparationSample> {
    let mut offsets = lanes
        .iter()
        .map(|lane| lane.synchronized_start_offset_s)
        .collect::<Vec<_>>();
    offsets.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    offsets.dedup_by(|a, b| (*a - *b).abs() <= f64::EPSILON);

    offsets
        .into_iter()
        .map(|offset| SurveySeparationSample {
            elapsed_s: offset,
            positions: lanes
                .iter()
                .filter(|lane| (lane.synchronized_start_offset_s - offset).abs() <= f64::EPSILON)
                .map(|lane| {
                    (
                        lane.drone_id,
                        (lane.start_xy.0, lane.start_xy.1, lane.planned_altitude_m),
                    )
                })
                .collect(),
        })
        .collect()
}

fn coverage_lane_length(bounds: &BoundaryBounds) -> f64 {
    bounds.width_m().min(bounds.height_m())
}

fn per_drone_coverage_time(
    drone_ids: &[Uuid],
    lanes: &[SurveyLane],
    lane_time_s: f64,
) -> Vec<DroneCoverageTime> {
    drone_ids
        .iter()
        .map(|drone_id| {
            let lane_count = lanes
                .iter()
                .filter(|lane| lane.drone_id == *drone_id)
                .count();
            DroneCoverageTime {
                drone_id: *drone_id,
                lane_count,
                planned_time_s: lane_count as f64 * lane_time_s,
            }
        })
        .collect()
}

fn balanced_time_spread(per_drone_time_s: &[DroneCoverageTime]) -> f64 {
    let min_time = per_drone_time_s
        .iter()
        .map(|time| time.planned_time_s)
        .fold(f64::INFINITY, f64::min);
    let max_time = per_drone_time_s
        .iter()
        .map(|time| time.planned_time_s)
        .fold(f64::NEG_INFINITY, f64::max);
    if min_time.is_finite() && max_time.is_finite() {
        max_time - min_time
    } else {
        0.0
    }
}

fn partition_lanes(
    drone_ids: &[Uuid],
    bounds: &BoundaryBounds,
    config: &SynchronizedSurveyConfig,
) -> Vec<SurveyLane> {
    let lane_count = drone_ids.len();
    let split_along_x = bounds.width_m() >= bounds.height_m();
    let lane_width = if split_along_x {
        bounds.width_m() / lane_count as f64
    } else {
        bounds.height_m() / lane_count as f64
    };

    drone_ids
        .iter()
        .enumerate()
        .map(|(lane_index, drone_id)| {
            let center_offset = lane_width * (lane_index as f64 + 0.5);
            let (start_xy, end_xy) = if split_along_x {
                let x = bounds.min_x + center_offset;
                ((x, bounds.min_y), (x, bounds.max_y))
            } else {
                let y = bounds.min_y + center_offset;
                ((bounds.min_x, y), (bounds.max_x, y))
            };

            SurveyLane {
                drone_id: *drone_id,
                lane_index,
                start_xy,
                end_xy,
                lane_width_m: lane_width * (1.0 + f64::from(config.overlap_percent) / 100.0),
                planned_altitude_m: config.planned_altitude_m,
                synchronized_start_offset_s: 0.0,
            }
        })
        .collect()
}

fn lane_endpoint_targets(lanes: &[SurveyLane]) -> Vec<SwarmActionTarget> {
    lanes
        .iter()
        .flat_map(|lane| {
            [
                SwarmActionTarget {
                    drone_id: lane.drone_id,
                    target_position: (lane.start_xy.0, lane.start_xy.1, lane.planned_altitude_m),
                },
                SwarmActionTarget {
                    drone_id: lane.drone_id,
                    target_position: (lane.end_xy.0, lane.end_xy.1, lane.planned_altitude_m),
                },
            ]
        })
        .collect()
}

fn coverage_fraction(lanes: &[SurveyLane], area_m2: f64) -> f32 {
    if area_m2 <= 0.0 {
        return 0.0;
    }
    progress_from_lanes(lanes, area_m2)
}

fn progress_from_lanes(lanes: &[SurveyLane], area_m2: f64) -> f32 {
    if lanes.is_empty() || area_m2 <= 0.0 {
        return 0.0;
    }
    let lane_length = distance_2d(lanes[0].start_xy, lanes[0].end_xy);
    let covered_area = lanes
        .iter()
        .map(|lane| lane.lane_width_m * lane_length)
        .sum::<f64>();
    (covered_area / area_m2).clamp(0.0, 1.0) as f32
}

fn safety_error_to_survey_error(error: SwarmActionSafetyError) -> SynchronizedSurveyError {
    SynchronizedSurveyError::SafetyRejected {
        reason: error.to_string(),
    }
}

fn separation_violation(
    drone_id: Uuid,
    position: (f64, f64, f32),
    distance_m: f64,
    required_m: f64,
    timestamp: DateTime<Utc>,
) -> SafetyViolation {
    SafetyViolation {
        drone_id,
        violation_type: ViolationType::CollisionRisk,
        description: format!(
            "Synchronized survey separation breach: {:.1}m observed, {:.1}m required",
            distance_m, required_m
        ),
        severity: Severity::Critical,
        timestamp,
        position: Some(position),
        action_ref: Some("synchronized_survey".to_string()),
    }
}

fn distance_2d(left: (f64, f64), right: (f64, f64)) -> f64 {
    (left.0 - right.0).hypot(left.1 - right.1)
}

fn distance_3d(left: (f64, f64, f32), right: (f64, f64, f32)) -> f64 {
    let altitude_delta = f64::from(left.2 - right.2);
    (left.0 - right.0)
        .hypot(left.1 - right.1)
        .hypot(altitude_delta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{swarm::FormationType, DroneSwarm, MultiDroneController};
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn active_swarm_controller(drone_ids: Vec<Uuid>) -> (MultiDroneController, Uuid) {
        let mut controller = MultiDroneController::new("survey coordinator".to_string());
        let mut swarm = DroneSwarm::new_owned(
            "north block survey".to_string(),
            drone_ids,
            FormationType::Line,
            "ops-team".to_string(),
        );
        swarm.status = crate::swarm::SwarmStatus::Active;
        let swarm_id = swarm.id;
        controller.register_swarm(swarm).unwrap();
        (controller, swarm_id)
    }

    fn rectangle_boundary() -> Vec<(f64, f64)> {
        vec![
            (0.0, 0.0),
            (120.0, 0.0),
            (120.0, 80.0),
            (0.0, 80.0),
            (0.0, 0.0),
        ]
    }

    fn rectangle_boundary_with_size(width_m: f64, height_m: f64) -> Vec<(f64, f64)> {
        vec![
            (0.0, 0.0),
            (width_m, 0.0),
            (width_m, height_m),
            (0.0, height_m),
            (0.0, 0.0),
        ]
    }

    #[test]
    fn synchronized_survey_partitions_boundary_into_deterministic_lanes() {
        let drone_ids = vec![Uuid::from_u128(3), Uuid::from_u128(1), Uuid::from_u128(2)];
        let (controller, swarm_id) = active_swarm_controller(drone_ids);

        let plan = plan_synchronized_survey(
            &controller,
            swarm_id,
            rectangle_boundary(),
            SynchronizedSurveyConfig {
                planned_altitude_m: 30.0,
                min_separation_m: 25.0,
                ..SynchronizedSurveyConfig::default()
            },
            Utc.timestamp_opt(1_800_000_000, 0).unwrap(),
        )
        .expect("survey should plan");

        assert_eq!(plan.status, SurveyExecutionStatus::Planned);
        assert_eq!(plan.lanes.len(), 3);
        assert_eq!(plan.lanes[0].drone_id, Uuid::from_u128(1));
        assert_eq!(plan.lanes[0].start_xy, (20.0, 0.0));
        assert_eq!(plan.lanes[0].end_xy, (20.0, 80.0));
        assert_eq!(plan.lanes[2].start_xy, (100.0, 0.0));
        assert!(plan.coverage_fraction >= 0.99);
        assert!(plan.separation_violations.is_empty());
        assert!(plan.audit[0].message.contains("3 synchronized lane"));
    }

    #[test]
    fn synchronized_survey_halts_and_marks_abort_on_mid_survey_separation_breach() {
        let drone_a = Uuid::from_u128(11);
        let drone_b = Uuid::from_u128(12);
        let (controller, swarm_id) = active_swarm_controller(vec![drone_a, drone_b]);
        let plan = plan_synchronized_survey(
            &controller,
            swarm_id,
            rectangle_boundary(),
            SynchronizedSurveyConfig::default(),
            Utc.timestamp_opt(1_800_000_000, 0).unwrap(),
        )
        .expect("survey should plan");

        let report = evaluate_synchronized_survey_progress(
            &plan,
            &[SurveySeparationSample {
                elapsed_s: 4.0,
                positions: vec![(drone_a, (40.0, 20.0, 30.0)), (drone_b, (48.0, 20.0, 30.0))],
            }],
            SynchronizedSurveyConfig {
                min_separation_m: 25.0,
                ..SynchronizedSurveyConfig::default()
            },
            Utc.timestamp_opt(1_800_000_004, 0).unwrap(),
        );

        assert_eq!(report.status, SurveyExecutionStatus::Halted);
        assert_eq!(report.abort_drone_ids, vec![drone_a, drone_b]);
        assert_eq!(report.separation_violations.len(), 2);
        assert!(report.audit[0].message.contains("separation breach"));
    }

    #[test]
    fn coverage_optimization_balances_three_drone_single_pass() {
        let drone_ids = vec![
            Uuid::from_u128(21),
            Uuid::from_u128(22),
            Uuid::from_u128(23),
        ];
        let (controller, swarm_id) = active_swarm_controller(drone_ids);

        let plan = plan_coverage_optimization(
            &controller,
            swarm_id,
            rectangle_boundary_with_size(90.0, 90.0),
            CoverageOptimizationConfig {
                sensor_swath_width_m: 40.0,
                overlap_percent: 10.0,
                survey_speed_mps: 10.0,
                ..CoverageOptimizationConfig::default()
            },
            Utc.timestamp_opt(1_800_000_100, 0).unwrap(),
        )
        .expect("coverage should optimize");

        assert_eq!(plan.required_lane_count, 3);
        assert_eq!(plan.pass_count, 1);
        assert!(!plan.requires_multi_pass);
        assert_eq!(plan.lanes.len(), 3);
        assert!(plan.coverage_fraction >= 0.99);
        assert!(plan.balanced_time_spread_s <= f64::EPSILON);
        assert!(plan
            .per_drone_time_s
            .iter()
            .all(|time| time.lane_count == 1 && (time.planned_time_s - 9.0).abs() < 0.001));
    }

    #[test]
    fn coverage_optimization_uses_multi_pass_when_drones_are_insufficient() {
        let drone_ids = vec![Uuid::from_u128(31), Uuid::from_u128(32)];
        let (controller, swarm_id) = active_swarm_controller(drone_ids);

        let plan = plan_coverage_optimization(
            &controller,
            swarm_id,
            rectangle_boundary_with_size(180.0, 90.0),
            CoverageOptimizationConfig {
                sensor_swath_width_m: 40.0,
                overlap_percent: 10.0,
                survey_speed_mps: 10.0,
                ..CoverageOptimizationConfig::default()
            },
            Utc.timestamp_opt(1_800_000_200, 0).unwrap(),
        )
        .expect("coverage should optimize as multi-pass");

        assert_eq!(plan.required_lane_count, 5);
        assert_eq!(plan.pass_count, 3);
        assert!(plan.requires_multi_pass);
        assert_eq!(plan.lanes.len(), 5);
        assert!(plan.coverage_fraction >= 0.99);
        assert!(
            plan.lanes
                .iter()
                .filter(|lane| lane.synchronized_start_offset_s == 0.0)
                .count()
                <= 2
        );
        assert!(plan.audit[0].message.contains("multi-pass"));
    }
}
