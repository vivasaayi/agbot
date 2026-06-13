use crate::flight_path::{PathType, SurveyPattern};
use crate::{FlightPath, Mission};
use anyhow::Result;
use geo::Point;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

pub struct MissionOptimizer {
    /// Maximum flight time in minutes
    pub max_flight_time_minutes: u32,
    /// Battery safety margin (0.0 to 1.0)
    pub battery_safety_margin: f32,
    /// Default cruise speed in m/s
    pub cruise_speed_ms: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MissionBudgetConfig {
    pub cruise_speed_ms: f32,
    pub max_flight_time_minutes: u32,
    pub battery_capacity_percent: f32,
    pub reserve_battery_percent: f32,
    pub base_draw_percent: f32,
    pub flight_draw_percent_per_meter: f32,
    pub action_draw_percent_per_second: f32,
    pub waypoint_draw_percent: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionBudgetReport {
    pub mission_id: Uuid,
    pub total_distance_m: f32,
    pub estimated_time_seconds: u32,
    pub estimated_time_minutes: u32,
    pub battery_draw_percent: f32,
    pub available_budget_percent: f32,
    pub battery_margin_percent: f32,
    pub over_time_budget: bool,
    pub over_battery_budget: bool,
    pub arm_blocked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionBudgetErrorCode {
    InvalidConfig,
    OverBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionBudgetError {
    pub code: MissionBudgetErrorCode,
    pub message: String,
}

impl Default for MissionBudgetConfig {
    fn default() -> Self {
        Self {
            cruise_speed_ms: 10.0,
            max_flight_time_minutes: 25,
            battery_capacity_percent: 100.0,
            reserve_battery_percent: 20.0,
            base_draw_percent: 30.0,
            flight_draw_percent_per_meter: 0.001,
            action_draw_percent_per_second: 0.02,
            waypoint_draw_percent: 2.0,
        }
    }
}

impl MissionOptimizer {
    pub fn new() -> Self {
        Self {
            max_flight_time_minutes: 25, // Typical drone battery life
            battery_safety_margin: 0.2,  // 20% safety margin
            cruise_speed_ms: 10.0,       // 10 m/s cruise speed
        }
    }

    pub fn budget_config(&self) -> MissionBudgetConfig {
        MissionBudgetConfig {
            cruise_speed_ms: self.cruise_speed_ms,
            max_flight_time_minutes: self.max_flight_time_minutes,
            reserve_battery_percent: self.battery_safety_margin * 100.0,
            ..MissionBudgetConfig::default()
        }
    }

    pub fn evaluate_budget(
        &self,
        mission: &Mission,
    ) -> std::result::Result<MissionBudgetReport, MissionBudgetError> {
        evaluate_mission_budget(mission, self.budget_config())
    }

    pub fn optimize_mission(&self, mission: &Mission) -> Result<Mission> {
        let mut optimized = mission.clone();

        // Optimize waypoint order using a simple nearest neighbor approach
        self.optimize_waypoint_order(&mut optimized)?;

        // Generate optimized flight paths
        self.generate_flight_paths(&mut optimized)?;

        // Calculate time and battery estimates
        self.calculate_estimates(&mut optimized)?;

        // Check if mission fits within constraints
        self.validate_constraints(&optimized)?;

        Ok(optimized)
    }

    fn optimize_waypoint_order(&self, mission: &mut Mission) -> Result<()> {
        if mission.waypoints.len() <= 2 {
            return Ok(());
        }

        // Simple nearest neighbor TSP approximation
        let mut optimized_waypoints = Vec::new();
        let mut remaining: Vec<_> = (1..mission.waypoints.len()).collect();

        // Start with first waypoint (usually takeoff)
        optimized_waypoints.push(mission.waypoints[0].clone());
        let mut current_idx = 0;

        while !remaining.is_empty() {
            let current_pos = &mission.waypoints[current_idx].position;

            // Find nearest remaining waypoint
            let (nearest_idx, nearest_pos) = remaining
                .iter()
                .enumerate()
                .min_by(|(_, &a), (_, &b)| {
                    let dist_a = distance(current_pos, &mission.waypoints[a].position);
                    let dist_b = distance(current_pos, &mission.waypoints[b].position);
                    dist_a.partial_cmp(&dist_b).unwrap()
                })
                .map(|(idx, &wp_idx)| (idx, wp_idx))
                .unwrap();

            optimized_waypoints.push(mission.waypoints[nearest_pos].clone());
            current_idx = nearest_pos;
            remaining.remove(nearest_idx);
        }

        mission.waypoints = optimized_waypoints;
        Ok(())
    }

    fn generate_flight_paths(&self, mission: &mut Mission) -> Result<()> {
        mission.flight_paths.clear();

        if mission.waypoints.is_empty() {
            return Ok(());
        }

        // Determine path type based on mission characteristics
        let path_type = if self.is_survey_mission(mission) {
            PathType::Survey {
                pattern: SurveyPattern::Grid,
                overlap_percent: 30.0,
            }
        } else {
            PathType::Direct
        };

        // Generate main flight path
        let main_path = FlightPath::from_waypoints(
            "Main Flight Path".to_string(),
            &mission.waypoints,
            path_type,
        );

        mission.flight_paths.push(main_path);
        Ok(())
    }

    fn calculate_estimates(&self, mission: &mut Mission) -> Result<()> {
        let budget = self.evaluate_budget(mission)?;
        mission.estimated_duration_minutes = budget.estimated_time_minutes;
        mission.estimated_battery_usage = (budget.battery_draw_percent / 100.0).min(1.0);

        Ok(())
    }

    fn validate_constraints(&self, mission: &Mission) -> Result<()> {
        // Check flight time constraint
        if mission.estimated_duration_minutes > self.max_flight_time_minutes {
            return Err(anyhow::anyhow!(
                "Mission duration ({} min) exceeds maximum flight time ({} min)",
                mission.estimated_duration_minutes,
                self.max_flight_time_minutes
            ));
        }

        // Check battery constraint
        let max_safe_battery = 1.0 - self.battery_safety_margin;
        if mission.estimated_battery_usage > max_safe_battery {
            return Err(anyhow::anyhow!(
                "Mission battery usage ({:.1}%) exceeds safe limit ({:.1}%)",
                mission.estimated_battery_usage * 100.0,
                max_safe_battery * 100.0
            ));
        }

        Ok(())
    }

    fn is_survey_mission(&self, mission: &Mission) -> bool {
        // Check if this is a survey mission based on waypoint density and area coverage
        let area_km2 = calculate_polygon_area(&mission.area_of_interest);
        let waypoint_density = mission.waypoints.len() as f64 / area_km2.max(0.01);

        // If we have many waypoints relative to area, it's likely a survey
        waypoint_density > 5.0
    }
}

pub fn evaluate_mission_budget(
    mission: &Mission,
    config: MissionBudgetConfig,
) -> std::result::Result<MissionBudgetReport, MissionBudgetError> {
    validate_budget_config(config)?;

    let total_distance_m = mission_path_distance_m(mission);
    let flight_time_seconds = (total_distance_m / config.cruise_speed_ms).ceil() as u32;
    let action_time_seconds = mission_action_time_seconds(mission);
    let estimated_time_seconds = flight_time_seconds.saturating_add(action_time_seconds);
    let estimated_time_minutes = estimated_time_seconds.div_ceil(60);

    let battery_draw_percent = config.base_draw_percent
        + total_distance_m * config.flight_draw_percent_per_meter
        + action_time_seconds as f32 * config.action_draw_percent_per_second
        + mission.waypoints.len() as f32 * config.waypoint_draw_percent;
    let available_budget_percent =
        (config.battery_capacity_percent - config.reserve_battery_percent).max(0.0);
    let battery_margin_percent = available_budget_percent - battery_draw_percent;
    let over_time_budget = estimated_time_minutes > config.max_flight_time_minutes;
    let over_battery_budget = battery_margin_percent < 0.0;

    Ok(MissionBudgetReport {
        mission_id: mission.id,
        total_distance_m,
        estimated_time_seconds,
        estimated_time_minutes,
        battery_draw_percent,
        available_budget_percent,
        battery_margin_percent,
        over_time_budget,
        over_battery_budget,
        arm_blocked: over_time_budget || over_battery_budget,
    })
}

pub fn assert_mission_budget_allows_arming(
    report: &MissionBudgetReport,
) -> std::result::Result<(), MissionBudgetError> {
    if report.arm_blocked {
        return Err(MissionBudgetError {
            code: MissionBudgetErrorCode::OverBudget,
            message: format!(
                "mission battery budget margin is {:.1}% and time budget exceeded: {}",
                report.battery_margin_percent, report.over_time_budget
            ),
        });
    }
    Ok(())
}

fn validate_budget_config(
    config: MissionBudgetConfig,
) -> std::result::Result<(), MissionBudgetError> {
    let valid = config.cruise_speed_ms.is_finite()
        && config.cruise_speed_ms > 0.0
        && config.battery_capacity_percent.is_finite()
        && config.battery_capacity_percent > 0.0
        && config.reserve_battery_percent.is_finite()
        && config.reserve_battery_percent >= 0.0
        && config.reserve_battery_percent < config.battery_capacity_percent
        && config.base_draw_percent.is_finite()
        && config.base_draw_percent >= 0.0
        && config.flight_draw_percent_per_meter.is_finite()
        && config.flight_draw_percent_per_meter >= 0.0
        && config.action_draw_percent_per_second.is_finite()
        && config.action_draw_percent_per_second >= 0.0
        && config.waypoint_draw_percent.is_finite()
        && config.waypoint_draw_percent >= 0.0;
    if !valid {
        return Err(MissionBudgetError {
            code: MissionBudgetErrorCode::InvalidConfig,
            message: "mission budget config requires finite positive speed/capacity and non-negative draw coefficients".to_string(),
        });
    }
    Ok(())
}

fn mission_path_distance_m(mission: &Mission) -> f32 {
    if mission.flight_paths.is_empty() {
        return FlightPath::from_waypoints(
            "budget evaluation path".to_string(),
            &mission.waypoints,
            PathType::Direct,
        )
        .total_distance_m;
    }
    mission
        .flight_paths
        .iter()
        .map(|path| path.total_distance_m)
        .sum()
}

fn mission_action_time_seconds(mission: &Mission) -> u32 {
    mission
        .waypoints
        .iter()
        .flat_map(|waypoint| waypoint.actions.iter())
        .map(estimate_action_time)
        .sum()
}

impl Default for MissionOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

fn estimate_action_time(action: &crate::waypoint::Action) -> u32 {
    use crate::waypoint::Action;

    match action {
        Action::TakePhoto { .. } => 2,
        Action::StartVideo {
            duration_seconds, ..
        } => *duration_seconds,
        Action::StopVideo { .. } => 1,
        Action::CollectLidar {
            duration_seconds, ..
        } => *duration_seconds,
        Action::CollectMultispectral { .. } => 5,
        Action::Hover { duration_seconds } => *duration_seconds,
        Action::SetSpeed { .. } => 1,
        Action::Wait { duration_seconds } => *duration_seconds,
        Action::Custom { .. } => 10,
    }
}

impl fmt::Display for MissionBudgetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for MissionBudgetError {}

fn distance(a: &Point<f64>, b: &Point<f64>) -> f64 {
    let dx = b.x() - a.x();
    let dy = b.y() - a.y();
    (dx * dx + dy * dy).sqrt()
}

fn calculate_polygon_area(polygon: &geo::Polygon<f64>) -> f64 {
    use geo::Area;
    polygon.unsigned_area() / 1_000_000.0 // Convert m² to km²
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Mission, Waypoint, WaypointType};
    use geo::{point, polygon};

    #[test]
    fn test_optimizer_basic() {
        let area = polygon![
            (x: 0.0, y: 0.0),
            (x: 0.01, y: 0.0),
            (x: 0.01, y: 0.01),
            (x: 0.0, y: 0.01),
            (x: 0.0, y: 0.0),
        ];

        let mut mission = Mission::new(
            "Test Mission".to_string(),
            "A test mission".to_string(),
            area,
        );

        // Add some waypoints
        mission.add_waypoint(Waypoint::new(
            point!(x: 0.0, y: 0.0),
            100.0,
            WaypointType::Takeoff,
        ));
        mission.add_waypoint(Waypoint::new(
            point!(x: 0.005, y: 0.005),
            150.0,
            WaypointType::DataCollection,
        ));
        mission.add_waypoint(Waypoint::new(
            point!(x: 0.01, y: 0.01),
            100.0,
            WaypointType::Landing,
        ));

        let optimizer = MissionOptimizer::new();
        let optimized = optimizer.optimize_mission(&mission).unwrap();

        assert!(!optimized.flight_paths.is_empty());
        assert!(optimized.estimated_duration_minutes > 0);
        assert!(optimized.estimated_battery_usage > 0.0);
    }

    #[test]
    fn mission_budget_report_returns_distance_time_battery_and_margin() {
        let mut mission = sample_budget_mission();
        mission.waypoints[1]
            .actions
            .push(crate::waypoint::Action::Hover {
                duration_seconds: 10,
            });
        let config = MissionBudgetConfig {
            cruise_speed_ms: 10.0,
            max_flight_time_minutes: 30,
            battery_capacity_percent: 100.0,
            reserve_battery_percent: 20.0,
            base_draw_percent: 5.0,
            flight_draw_percent_per_meter: 0.001,
            action_draw_percent_per_second: 0.05,
            waypoint_draw_percent: 0.5,
        };

        let report =
            evaluate_mission_budget(&mission, config).expect("budget report should compute");

        assert!(report.total_distance_m > 2_200.0);
        assert!(report.estimated_time_seconds >= 232);
        assert!(report.battery_draw_percent > 0.0);
        assert_eq!(report.available_budget_percent, 80.0);
        assert!(report.battery_margin_percent > 0.0);
        assert!(!report.over_battery_budget);
        assert!(!report.arm_blocked);
        assert_mission_budget_allows_arming(&report).expect("budget should allow arming");
    }

    #[test]
    fn mission_budget_report_flags_over_budget_and_blocks_arming() {
        let mission = sample_budget_mission();
        let report = evaluate_mission_budget(
            &mission,
            MissionBudgetConfig {
                cruise_speed_ms: 10.0,
                max_flight_time_minutes: 30,
                battery_capacity_percent: 100.0,
                reserve_battery_percent: 25.0,
                base_draw_percent: 10.0,
                flight_draw_percent_per_meter: 0.05,
                action_draw_percent_per_second: 0.0,
                waypoint_draw_percent: 0.0,
            },
        )
        .expect("budget report should compute");

        assert!(report.over_battery_budget);
        assert!(report.arm_blocked);
        let error = assert_mission_budget_allows_arming(&report)
            .expect_err("over-budget mission should block arming");
        assert_eq!(error.code, MissionBudgetErrorCode::OverBudget);
        assert!(error.message.contains("battery budget"));
    }

    fn sample_budget_mission() -> Mission {
        let area = polygon![
            (x: 0.0, y: 0.0),
            (x: 0.02, y: 0.0),
            (x: 0.02, y: 0.02),
            (x: 0.0, y: 0.02),
            (x: 0.0, y: 0.0),
        ];
        let mut mission = Mission::new(
            "Budget Mission".to_string(),
            "A deterministic budget fixture".to_string(),
            area,
        );
        mission.add_waypoint(Waypoint::new(
            point!(x: 0.0, y: 0.0),
            100.0,
            WaypointType::Takeoff,
        ));
        mission.add_waypoint(Waypoint::new(
            point!(x: 0.01, y: 0.0),
            120.0,
            WaypointType::Survey,
        ));
        mission.add_waypoint(Waypoint::new(
            point!(x: 0.02, y: 0.0),
            100.0,
            WaypointType::Landing,
        ));
        mission
    }
}
