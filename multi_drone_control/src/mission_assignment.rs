use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use anyhow::Result;
use shared::{Mission, GeoCoordinate};

/// Mission assignment and scheduling system for multi-drone operations
pub struct MissionAssignmentEngine {
    pending_missions: HashMap<Uuid, MissionRequest>,
    assigned_missions: HashMap<Uuid, DroneAssignment>,
    drone_capabilities: HashMap<Uuid, DroneCapabilities>,
    assignment_algorithm: AssignmentAlgorithm,
    load_balancing_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionRequest {
    pub id: Uuid,
    pub mission: Mission,
    pub priority: u8,
    pub required_capabilities: Vec<String>,
    pub preferred_drone: Option<Uuid>,
    pub deadline: Option<DateTime<Utc>>,
    pub estimated_duration: std::time::Duration,
    pub max_drones: usize,
    pub min_drones: usize,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneAssignment {
    pub drone_id: Uuid,
    pub mission_id: Uuid,
    pub assigned_at: DateTime<Utc>,
    pub estimated_completion: DateTime<Utc>,
    pub status: AssignmentStatus,
    pub role: DroneRole,
    pub workload_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneCapabilities {
    pub id: Uuid,
    pub flight_time_minutes: u32,
    pub max_speed: f32,
    pub payload_capacity: f32,
    pub sensor_types: Vec<String>,
    pub special_capabilities: Vec<String>,
    pub current_battery: f32,
    pub maintenance_schedule: Option<DateTime<Utc>>,
    pub availability_status: AvailabilityStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssignmentStatus {
    Assigned,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DroneRole {
    Primary,
    Secondary,
    Support,
    Backup,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AvailabilityStatus {
    Available,
    Busy,
    Maintenance,
    Charging,
    Reserved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignmentAlgorithm {
    FirstAvailable,
    BestFit,
    LoadBalanced,
    PriorityBased,
    Auction,
}

impl MissionAssignmentEngine {
    pub fn new(algorithm: AssignmentAlgorithm) -> Self {
        Self {
            pending_missions: HashMap::new(),
            assigned_missions: HashMap::new(),
            drone_capabilities: HashMap::new(),
            assignment_algorithm: algorithm,
            load_balancing_enabled: true,
        }
    }

    pub async fn register_drone(&mut self, capabilities: DroneCapabilities) -> Result<()> {
        let drone_id = capabilities.id;
        self.drone_capabilities.insert(capabilities.id, capabilities);
        tracing::info!("Registered drone {} for mission assignment", drone_id);
        Ok(())
    }

    pub async fn submit_mission(&mut self, request: MissionRequest) -> Result<Uuid> {
        let mission_id = request.id;
        self.pending_missions.insert(mission_id, request);
        
        // Attempt immediate assignment
        self.process_pending_missions().await?;
        
        tracing::info!("Submitted mission {} for assignment", mission_id);
        Ok(mission_id)
    }

    pub async fn process_pending_missions(&mut self) -> Result<usize> {
        let mut assigned_count = 0;
        let pending_ids: Vec<Uuid> = self.pending_missions.keys().copied().collect();

        for mission_id in pending_ids {
            if let Some(request) = self.pending_missions.get(&mission_id) {
                if let Some(assignments) = self.find_best_assignment(request).await? {
                    // Remove from pending and add to assigned
                    self.pending_missions.remove(&mission_id);
                    
                    for assignment in assignments {
                        self.assigned_missions.insert(assignment.drone_id, assignment);
                        assigned_count += 1;
                    }
                    
                    tracing::info!("Assigned mission {} to {} drones", mission_id, assigned_count);
                }
            }
        }

        Ok(assigned_count)
    }

    async fn find_best_assignment(&self, request: &MissionRequest) -> Result<Option<Vec<DroneAssignment>>> {
        match self.assignment_algorithm {
            AssignmentAlgorithm::FirstAvailable => self.assign_first_available(request).await,
            AssignmentAlgorithm::BestFit => self.assign_best_fit(request).await,
            AssignmentAlgorithm::LoadBalanced => self.assign_load_balanced(request).await,
            AssignmentAlgorithm::PriorityBased => self.assign_priority_based(request).await,
            AssignmentAlgorithm::Auction => self.assign_auction_based(request).await,
        }
    }

    async fn assign_first_available(&self, request: &MissionRequest) -> Result<Option<Vec<DroneAssignment>>> {
        let available_drones: Vec<&DroneCapabilities> = self.drone_capabilities.values()
            .filter(|d| d.availability_status == AvailabilityStatus::Available)
            .filter(|d| self.drone_matches_requirements(d, request))
            .take(request.max_drones)
            .collect();

        if available_drones.len() < request.min_drones {
            return Ok(None);
        }

        let assignments = available_drones.into_iter()
            .enumerate()
            .map(|(i, drone)| DroneAssignment {
                drone_id: drone.id,
                mission_id: request.id,
                assigned_at: Utc::now(),
                estimated_completion: Utc::now() + chrono::Duration::from_std(request.estimated_duration).unwrap(),
                status: AssignmentStatus::Assigned,
                role: if i == 0 { DroneRole::Primary } else { DroneRole::Secondary },
                workload_score: self.calculate_workload_score(drone.id),
            })
            .collect();

        Ok(Some(assignments))
    }

    async fn assign_best_fit(&self, request: &MissionRequest) -> Result<Option<Vec<DroneAssignment>>> {
        let mut scored_drones: Vec<(f32, &DroneCapabilities)> = self.drone_capabilities.values()
            .filter(|d| d.availability_status == AvailabilityStatus::Available)
            .filter(|d| self.drone_matches_requirements(d, request))
            .map(|d| (self.calculate_fitness_score(d, request), d))
            .collect();

        scored_drones.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        let selected_drones: Vec<&DroneCapabilities> = scored_drones.into_iter()
            .take(request.max_drones)
            .map(|(_, drone)| drone)
            .collect();

        if selected_drones.len() < request.min_drones {
            return Ok(None);
        }

        let assignments = selected_drones.into_iter()
            .enumerate()
            .map(|(i, drone)| DroneAssignment {
                drone_id: drone.id,
                mission_id: request.id,
                assigned_at: Utc::now(),
                estimated_completion: Utc::now() + chrono::Duration::from_std(request.estimated_duration).unwrap(),
                status: AssignmentStatus::Assigned,
                role: if i == 0 { DroneRole::Primary } else { DroneRole::Secondary },
                workload_score: self.calculate_workload_score(drone.id),
            })
            .collect();

        Ok(Some(assignments))
    }

    async fn assign_load_balanced(&self, _request: &MissionRequest) -> Result<Option<Vec<DroneAssignment>>> {
        // TODO: Implement load balancing algorithm
        Ok(None)
    }

    async fn assign_priority_based(&self, _request: &MissionRequest) -> Result<Option<Vec<DroneAssignment>>> {
        // TODO: Implement priority-based assignment
        Ok(None)
    }

    async fn assign_auction_based(&self, _request: &MissionRequest) -> Result<Option<Vec<DroneAssignment>>> {
        // TODO: Implement auction-based assignment
        Ok(None)
    }

    fn drone_matches_requirements(&self, drone: &DroneCapabilities, request: &MissionRequest) -> bool {
        // Check if drone has required capabilities
        for required_cap in &request.required_capabilities {
            if !drone.special_capabilities.contains(required_cap) &&
               !drone.sensor_types.contains(required_cap) {
                return false;
            }
        }

        // Check battery level
        if drone.current_battery < 0.2 {
            return false;
        }

        // Check maintenance schedule
        if let Some(maintenance_time) = drone.maintenance_schedule {
            let mission_end = Utc::now() + chrono::Duration::from_std(request.estimated_duration).unwrap();
            if maintenance_time < mission_end {
                return false;
            }
        }

        true
    }

    fn calculate_fitness_score(&self, drone: &DroneCapabilities, request: &MissionRequest) -> f32 {
        let mut score = 0.0;

        // Battery level factor (higher is better)
        score += drone.current_battery * 30.0;

        // Capability match factor
        let matching_caps = request.required_capabilities.iter()
            .filter(|cap| drone.special_capabilities.contains(cap) || drone.sensor_types.contains(cap))
            .count() as f32;
        score += matching_caps * 20.0;

        // Flight time factor
        let required_minutes = request.estimated_duration.as_secs() / 60;
        if drone.flight_time_minutes as u64 >= required_minutes {
            score += 25.0;
        } else {
            score -= 10.0;
        }

        // Load balancing factor
        if self.load_balancing_enabled {
            let workload = self.calculate_workload_score(drone.id);
            score -= workload * 15.0;
        }

        score
    }

    fn calculate_workload_score(&self, drone_id: Uuid) -> f32 {
        let active_missions = self.assigned_missions.values()
            .filter(|a| a.drone_id == drone_id)
            .filter(|a| matches!(a.status, AssignmentStatus::Assigned | AssignmentStatus::InProgress))
            .count();

        active_missions as f32 * 10.0
    }

    pub async fn update_assignment_status(&mut self, drone_id: Uuid, status: AssignmentStatus) -> Result<()> {
        if let Some(assignment) = self.assigned_missions.get_mut(&drone_id) {
            assignment.status = status.clone();
            tracing::info!("Updated assignment status for drone {} to {:?}", drone_id, status);
        }
        Ok(())
    }

    pub async fn cancel_mission(&mut self, mission_id: Uuid) -> Result<()> {
        // Remove from pending
        self.pending_missions.remove(&mission_id);

        // Cancel assigned missions
        let assigned_drones: Vec<Uuid> = self.assigned_missions.values()
            .filter(|a| a.mission_id == mission_id)
            .map(|a| a.drone_id)
            .collect();

        for drone_id in assigned_drones {
            self.assigned_missions.remove(&drone_id);
        }

        tracing::info!("Cancelled mission {}", mission_id);
        Ok(())
    }

    pub async fn get_assignment_statistics(&self) -> AssignmentStatistics {
        let total_pending = self.pending_missions.len();
        let total_assigned = self.assigned_missions.len();
        let completed_assignments = self.assigned_missions.values()
            .filter(|a| a.status == AssignmentStatus::Completed)
            .count();

        let average_workload = if !self.drone_capabilities.is_empty() {
            self.drone_capabilities.keys()
                .map(|id| self.calculate_workload_score(*id))
                .sum::<f32>() / self.drone_capabilities.len() as f32
        } else {
            0.0
        };

        AssignmentStatistics {
            total_pending_missions: total_pending,
            total_assigned_missions: total_assigned,
            completed_assignments,
            average_drone_workload: average_workload,
            assignment_success_rate: if total_assigned > 0 {
                completed_assignments as f32 / total_assigned as f32
            } else {
                0.0
            },
            last_update: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentStatistics {
    pub total_pending_missions: usize,
    pub total_assigned_missions: usize,
    pub completed_assignments: usize,
    pub average_drone_workload: f32,
    pub assignment_success_rate: f32,
    pub last_update: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_mission_assignment() {
        let mut engine = MissionAssignmentEngine::new(AssignmentAlgorithm::FirstAvailable);

        // Register a drone
        let capabilities = DroneCapabilities {
            id: Uuid::new_v4(),
            flight_time_minutes: 60,
            max_speed: 15.0,
            payload_capacity: 2.0,
            sensor_types: vec!["RGB".to_string(), "GPS".to_string()],
            special_capabilities: vec!["autonomous_flight".to_string()],
            current_battery: 0.9,
            maintenance_schedule: None,
            availability_status: AvailabilityStatus::Available,
        };

        engine.register_drone(capabilities).await.unwrap();

        // Submit a mission
        let mission_request = MissionRequest {
            id: Uuid::new_v4(),
            mission: Mission {
                id: Uuid::new_v4(),
                name: "Test Mission".to_string(),
                waypoints: vec![],
                flight_parameters: shared::FlightParameters {
                    max_speed_ms: 15.0,
                    cruise_altitude_m: 100.0,
                    takeoff_altitude_m: 50.0,
                    return_to_home_altitude_m: 120.0,
                },
                safety_constraints: shared::SafetyConstraints {
                    max_wind_speed_ms: 10.0,
                    min_battery_level: 0.2,
                    geofence_boundaries: vec![],
                    no_fly_zones: vec![],
                },
                created_at: Utc::now(),
            },
            priority: 5,
            required_capabilities: vec!["RGB".to_string()],
            preferred_drone: None,
            deadline: None,
            estimated_duration: Duration::from_secs(1800), // 30 minutes
            max_drones: 1,
            min_drones: 1,
            created_at: Utc::now(),
        };

        engine.submit_mission(mission_request).await.unwrap();

        let stats = engine.get_assignment_statistics().await;
        assert_eq!(stats.total_assigned_missions, 1);
    }
}
