use anyhow::Result;
use crate::{Mission, FlightPath};
use crate::flight_path::{PathType, SurveyPattern};
use geo::Point;

pub struct MissionOptimizer {
    /// Maximum flight time in minutes
    pub max_flight_time_minutes: u32,
    /// Battery safety margin (0.0 to 1.0)
    pub battery_safety_margin: f32,
    /// Default cruise speed in m/s
    pub cruise_speed_ms: f32,
}

impl MissionOptimizer {
    pub fn new() -> Self {
        Self {
            max_flight_time_minutes: 25, // Typical drone battery life
            battery_safety_margin: 0.2,  // 20% safety margin
            cruise_speed_ms: 10.0,       // 10 m/s cruise speed
        }
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
        let mut total_time_seconds = 0u32;
        let mut total_distance_m = 0.0f32;

        for path in &mission.flight_paths {
            total_time_seconds += path.estimated_duration_seconds;
            total_distance_m += path.total_distance_m;
        }

        // Add time for actions at waypoints
        for waypoint in &mission.waypoints {
            for action in &waypoint.actions {
                total_time_seconds += self.estimate_action_time(action);
            }
        }

        mission.estimated_duration_minutes = (total_time_seconds + 59) / 60; // Round up

        // Battery usage estimation (simplified model)
        // Base consumption + distance-based consumption + action-based consumption
        let base_consumption = 0.3; // 30% for basic flight systems
        let distance_consumption = total_distance_m / 10000.0; // 10% per 10km
        let action_consumption = mission.waypoints.len() as f32 * 0.02; // 2% per waypoint with actions

        mission.estimated_battery_usage = base_consumption + distance_consumption + action_consumption;
        mission.estimated_battery_usage = mission.estimated_battery_usage.min(1.0); // Cap at 100%

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

    fn estimate_action_time(&self, action: &crate::waypoint::Action) -> u32 {
        use crate::waypoint::Action;
        
        match action {
            Action::TakePhoto { .. } => 2,
            Action::StartVideo { duration_seconds, .. } => *duration_seconds,
            Action::StopVideo { .. } => 1,
            Action::CollectLidar { duration_seconds, .. } => *duration_seconds,
            Action::CollectMultispectral { .. } => 5,
            Action::Hover { duration_seconds } => *duration_seconds,
            Action::SetSpeed { .. } => 1,
            Action::Wait { duration_seconds } => *duration_seconds,
            Action::Custom { .. } => 10, // Default estimate
        }
    }
}

impl Default for MissionOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

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
    use geo::{coord, polygon, point};

    #[test]
    fn test_optimizer_basic() {
        let area = polygon![
            (x: 0.0, y: 0.0),
            (x: 1.0, y: 0.0),
            (x: 1.0, y: 1.0),
            (x: 0.0, y: 1.0),
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
            point!(x: 0.5, y: 0.5),
            150.0,
            WaypointType::DataCollection,
        ));
        mission.add_waypoint(Waypoint::new(
            point!(x: 1.0, y: 1.0),
            100.0,
            WaypointType::Landing,
        ));
        
        let optimizer = MissionOptimizer::new();
        let optimized = optimizer.optimize_mission(&mission).unwrap();
        
        assert!(!optimized.flight_paths.is_empty());
        assert!(optimized.estimated_duration_minutes > 0);
        assert!(optimized.estimated_battery_usage > 0.0);
    }
}
