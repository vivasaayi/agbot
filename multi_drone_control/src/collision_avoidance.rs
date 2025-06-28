use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use anyhow::Result;
use shared::Position3D;

/// 3D collision avoidance system for multi-drone operations
pub struct CollisionAvoidanceSystem {
    tracked_drones: HashMap<Uuid, DroneTrackingInfo>,
    avoidance_rules: Vec<AvoidanceRule>,
    min_separation_distance: f64, // meters
    prediction_horizon: std::time::Duration,
    emergency_maneuver_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneTrackingInfo {
    pub id: Uuid,
    pub position: Position3D,
    pub velocity: Velocity3D,
    pub heading: f32, // degrees
    pub altitude: f32, // meters
    pub last_update: DateTime<Utc>,
    pub predicted_trajectory: Vec<Position3D>,
    pub collision_risk_level: RiskLevel,
    pub avoidance_maneuver: Option<AvoidanceManeuver>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Velocity3D {
    pub vx: f32, // m/s
    pub vy: f32, // m/s
    pub vz: f32, // m/s
    pub speed: f32, // total speed m/s
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvoidanceRule {
    pub id: Uuid,
    pub name: String,
    pub priority: u8,
    pub trigger_distance: f64, // meters
    pub maneuver_type: ManeuverType,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ManeuverType {
    AltitudeChange { delta: f32 },
    HorizontalDeviation { angle_deg: f32, distance: f32 },
    SpeedReduction { factor: f32 },
    EmergencyStop,
    ReturnToBase,
    Hover,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvoidanceManeuver {
    pub maneuver_type: ManeuverType,
    pub start_time: DateTime<Utc>,
    pub estimated_duration: std::time::Duration,
    pub target_position: Option<Position3D>,
    pub priority: u8,
    pub reason: String,
}

impl CollisionAvoidanceSystem {
    pub fn new() -> Self {
        Self {
            tracked_drones: HashMap::new(),
            avoidance_rules: Self::default_rules(),
            min_separation_distance: 25.0, // 25 meters minimum
            prediction_horizon: std::time::Duration::from_secs(30),
            emergency_maneuver_enabled: true,
        }
    }

    fn default_rules() -> Vec<AvoidanceRule> {
        vec![
            AvoidanceRule {
                id: Uuid::new_v4(),
                name: "Critical Proximity".to_string(),
                priority: 1,
                trigger_distance: 15.0,
                maneuver_type: ManeuverType::EmergencyStop,
                enabled: true,
            },
            AvoidanceRule {
                id: Uuid::new_v4(),
                name: "Close Approach".to_string(),
                priority: 2,
                trigger_distance: 30.0,
                maneuver_type: ManeuverType::AltitudeChange { delta: 10.0 },
                enabled: true,
            },
            AvoidanceRule {
                id: Uuid::new_v4(),
                name: "Medium Distance".to_string(),
                priority: 3,
                trigger_distance: 50.0,
                maneuver_type: ManeuverType::HorizontalDeviation { 
                    angle_deg: 30.0, 
                    distance: 20.0 
                },
                enabled: true,
            },
            AvoidanceRule {
                id: Uuid::new_v4(),
                name: "Speed Reduction".to_string(),
                priority: 4,
                trigger_distance: 75.0,
                maneuver_type: ManeuverType::SpeedReduction { factor: 0.7 },
                enabled: true,
            },
        ]
    }

    pub async fn register_drone(&mut self, drone_id: Uuid, initial_position: Position3D) -> Result<()> {
        let tracking_info = DroneTrackingInfo {
            id: drone_id,
            position: initial_position.clone(),
            velocity: Velocity3D {
                vx: 0.0,
                vy: 0.0,
                vz: 0.0,
                speed: 0.0,
            },
            heading: 0.0,
            altitude: initial_position.altitude_m,
            last_update: Utc::now(),
            predicted_trajectory: vec![],
            collision_risk_level: RiskLevel::None,
            avoidance_maneuver: None,
        };

        self.tracked_drones.insert(drone_id, tracking_info);
        tracing::info!("Registered drone {} for collision avoidance", drone_id);
        Ok(())
    }

    pub async fn update_drone_state(
        &mut self,
        drone_id: Uuid,
        position: Position3D,
        velocity: Velocity3D,
        heading: f32,
    ) -> Result<()> {
        if let Some(tracking_info) = self.tracked_drones.get_mut(&drone_id) {
            tracking_info.position = position;
            tracking_info.velocity = velocity;
            tracking_info.heading = heading;
            tracking_info.altitude = tracking_info.position.altitude_m;
            tracking_info.last_update = Utc::now();
        } else {
            return Err(anyhow::anyhow!("Drone not registered: {}", drone_id));
        }

        // Separate the prediction and risk assessment to avoid borrow conflicts
        if let Some(tracking_info) = self.tracked_drones.get(&drone_id) {
            let predicted_trajectory = self.predict_trajectory(tracking_info).await;
            
            if let Some(tracking_info) = self.tracked_drones.get_mut(&drone_id) {
                tracking_info.predicted_trajectory = predicted_trajectory;
            }
        }

        // Check for collision risks
        self.assess_collision_risk(drone_id).await?;

        Ok(())
    }

    async fn predict_trajectory(&self, tracking_info: &DroneTrackingInfo) -> Vec<Position3D> {
        let mut trajectory = Vec::new();
        let mut current_pos = tracking_info.position.clone();
        let dt = 1.0; // 1 second intervals

        let prediction_steps = self.prediction_horizon.as_secs() as usize;

        for _ in 0..prediction_steps {
            // Convert velocity from m/s to lat/lon changes per second (approximate)
            // 1 degree of latitude ≈ 111,111 meters
            // 1 degree of longitude ≈ 111,111 * cos(latitude) meters
            let lat_change = (tracking_info.velocity.vy as f64 * dt) / 111_111.0;
            let lon_change = (tracking_info.velocity.vx as f64 * dt) / (111_111.0 * current_pos.latitude.cos());
            let alt_change = tracking_info.velocity.vz * dt as f32;

            current_pos.latitude += lat_change;
            current_pos.longitude += lon_change;
            current_pos.altitude_m += alt_change;

            trajectory.push(current_pos.clone());
        }

        trajectory
    }

    async fn assess_collision_risk(&mut self, drone_id: Uuid) -> Result<()> {
        let drone_ids: Vec<Uuid> = self.tracked_drones.keys().copied().collect();
        
        for other_id in drone_ids {
            if other_id == drone_id {
                continue;
            }

            let risk = self.calculate_collision_risk(drone_id, other_id).await?;
            
            if let Some(tracking_info) = self.tracked_drones.get_mut(&drone_id) {
                if risk > tracking_info.collision_risk_level {
                    tracking_info.collision_risk_level = risk.clone();
                    
                    // Trigger avoidance maneuver if necessary
                    if risk >= RiskLevel::Medium {
                        self.plan_avoidance_maneuver(drone_id, other_id).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn calculate_collision_risk(&self, drone1_id: Uuid, drone2_id: Uuid) -> Result<RiskLevel> {
        let drone1 = self.tracked_drones.get(&drone1_id)
            .ok_or_else(|| anyhow::anyhow!("Drone 1 not found"))?;
        let drone2 = self.tracked_drones.get(&drone2_id)
            .ok_or_else(|| anyhow::anyhow!("Drone 2 not found"))?;

        // Calculate current distance
        let current_distance = self.calculate_3d_distance(&drone1.position, &drone2.position);

        // Check if trajectories intersect
        let trajectory_risk = self.check_trajectory_intersection(drone1, drone2).await;

        // Determine risk level based on distance and trajectory
        let risk = if current_distance < 15.0 {
            RiskLevel::Critical
        } else if current_distance < 25.0 || trajectory_risk {
            RiskLevel::High
        } else if current_distance < 50.0 {
            RiskLevel::Medium
        } else if current_distance < 100.0 {
            RiskLevel::Low
        } else {
            RiskLevel::None
        };

        Ok(risk)
    }

    fn calculate_3d_distance(&self, pos1: &Position3D, pos2: &Position3D) -> f64 {
        // Calculate distance using the shared GeoCoordinate distance method
        // and add altitude difference
        let horizontal_distance = pos1.distance_to(pos2) as f64;
        let altitude_diff = (pos1.altitude_m - pos2.altitude_m) as f64;
        (horizontal_distance * horizontal_distance + altitude_diff * altitude_diff).sqrt()
    }

    async fn check_trajectory_intersection(&self, drone1: &DroneTrackingInfo, drone2: &DroneTrackingInfo) -> bool {
        for (i, pos1) in drone1.predicted_trajectory.iter().enumerate() {
            if let Some(pos2) = drone2.predicted_trajectory.get(i) {
                let distance = self.calculate_3d_distance(pos1, pos2);
                if distance < self.min_separation_distance {
                    return true;
                }
            }
        }
        false
    }

    async fn plan_avoidance_maneuver(&mut self, drone_id: Uuid, _conflicting_drone_id: Uuid) -> Result<()> {
        // Find appropriate avoidance rule
        let applicable_rule = self.avoidance_rules.iter()
            .filter(|rule| rule.enabled)
            .min_by_key(|rule| rule.priority)
            .cloned();

        if let Some(rule) = applicable_rule {
            let maneuver = AvoidanceManeuver {
                maneuver_type: rule.maneuver_type.clone(),
                start_time: Utc::now(),
                estimated_duration: std::time::Duration::from_secs(30),
                target_position: None, // TODO: Calculate target position
                priority: rule.priority,
                reason: format!("Collision avoidance: {}", rule.name),
            };

            if let Some(tracking_info) = self.tracked_drones.get_mut(&drone_id) {
                tracking_info.avoidance_maneuver = Some(maneuver);
                tracing::warn!("Planned avoidance maneuver for drone {}", drone_id);
            }
        }

        Ok(())
    }

    pub async fn get_avoidance_command(&self, drone_id: Uuid) -> Result<Option<AvoidanceCommand>> {
        if let Some(tracking_info) = self.tracked_drones.get(&drone_id) {
            if let Some(maneuver) = &tracking_info.avoidance_maneuver {
                let command = match &maneuver.maneuver_type {
                    ManeuverType::AltitudeChange { delta } => {
                        AvoidanceCommand::ChangeAltitude {
                            target_altitude: tracking_info.altitude + delta,
                            urgency: self.risk_to_urgency(&tracking_info.collision_risk_level),
                        }
                    }
                    ManeuverType::HorizontalDeviation { angle_deg, distance } => {
                        AvoidanceCommand::HorizontalManeuver {
                            heading_change: *angle_deg,
                            distance: *distance,
                            urgency: self.risk_to_urgency(&tracking_info.collision_risk_level),
                        }
                    }
                    ManeuverType::SpeedReduction { factor } => {
                        AvoidanceCommand::ChangeSpeed {
                            speed_factor: *factor,
                            urgency: self.risk_to_urgency(&tracking_info.collision_risk_level),
                        }
                    }
                    ManeuverType::EmergencyStop => {
                        AvoidanceCommand::EmergencyStop {
                            reason: maneuver.reason.clone(),
                        }
                    }
                    ManeuverType::ReturnToBase => {
                        AvoidanceCommand::ReturnToBase {
                            reason: maneuver.reason.clone(),
                        }
                    }
                    ManeuverType::Hover => {
                        AvoidanceCommand::Hover {
                            duration: maneuver.estimated_duration,
                        }
                    }
                };

                return Ok(Some(command));
            }
        }

        Ok(None)
    }

    fn risk_to_urgency(&self, risk: &RiskLevel) -> CommandUrgency {
        match risk {
            RiskLevel::Critical => CommandUrgency::Emergency,
            RiskLevel::High => CommandUrgency::High,
            RiskLevel::Medium => CommandUrgency::Medium,
            RiskLevel::Low => CommandUrgency::Low,
            RiskLevel::None => CommandUrgency::Low,
        }
    }

    pub async fn clear_avoidance_maneuver(&mut self, drone_id: Uuid) -> Result<()> {
        if let Some(tracking_info) = self.tracked_drones.get_mut(&drone_id) {
            tracking_info.avoidance_maneuver = None;
            tracking_info.collision_risk_level = RiskLevel::None;
            tracing::info!("Cleared avoidance maneuver for drone {}", drone_id);
        }
        Ok(())
    }

    pub async fn get_system_status(&self) -> CollisionAvoidanceStatus {
        let total_drones = self.tracked_drones.len();
        let drones_at_risk = self.tracked_drones.values()
            .filter(|d| d.collision_risk_level >= RiskLevel::Medium)
            .count();
        
        let active_maneuvers = self.tracked_drones.values()
            .filter(|d| d.avoidance_maneuver.is_some())
            .count();

        CollisionAvoidanceStatus {
            total_tracked_drones: total_drones,
            drones_at_risk,
            active_maneuvers,
            system_health: if drones_at_risk == 0 { 1.0 } else { 1.0 - (drones_at_risk as f32 / total_drones as f32) },
            last_update: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AvoidanceCommand {
    ChangeAltitude {
        target_altitude: f32,
        urgency: CommandUrgency,
    },
    HorizontalManeuver {
        heading_change: f32,
        distance: f32,
        urgency: CommandUrgency,
    },
    ChangeSpeed {
        speed_factor: f32,
        urgency: CommandUrgency,
    },
    EmergencyStop {
        reason: String,
    },
    ReturnToBase {
        reason: String,
    },
    Hover {
        duration: std::time::Duration,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandUrgency {
    Low,
    Medium,
    High,
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollisionAvoidanceStatus {
    pub total_tracked_drones: usize,
    pub drones_at_risk: usize,
    pub active_maneuvers: usize,
    pub system_health: f32,
    pub last_update: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_collision_avoidance_system() {
        let mut system = CollisionAvoidanceSystem::new();

        let drone1_id = Uuid::new_v4();
        let drone2_id = Uuid::new_v4();

        let pos1 = Position3D {
            latitude: 40.7128,
            longitude: -74.0060,
            altitude_m: 100.0,
        };

        let pos2 = Position3D {
            latitude: 40.7129,
            longitude: -74.0061,
            altitude_m: 100.0,
        };

        system.register_drone(drone1_id, pos1.clone()).await.unwrap();
        system.register_drone(drone2_id, pos2.clone()).await.unwrap();

        let velocity = Velocity3D {
            vx: 5.0,
            vy: 0.0,
            vz: 0.0,
            speed: 5.0,
        };

        system.update_drone_state(drone1_id, pos1, velocity.clone(), 0.0).await.unwrap();
        system.update_drone_state(drone2_id, pos2, velocity, 180.0).await.unwrap();

        let status = system.get_system_status().await;
        assert_eq!(status.total_tracked_drones, 2);
    }

    #[test]
    fn test_distance_calculation() {
        let system = CollisionAvoidanceSystem::new();
        
        let pos1 = Position3D {
            latitude: 40.7128,
            longitude: -74.0060,
            altitude_m: 0.0,
        };
        
        let pos2 = Position3D {
            latitude: 40.7129,
            longitude: -74.0061,
            altitude_m: 0.0,
        };
        
        let distance = system.calculate_3d_distance(&pos1, &pos2);
        assert!((distance - 5.0).abs() < 0.1); // Should be 5m (3-4-5 triangle)
    }
}
