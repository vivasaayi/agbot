use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::Position3D;
use std::collections::HashMap;
use uuid::Uuid;

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
    pub heading: f32,  // degrees
    pub altitude: f32, // meters
    pub last_update: DateTime<Utc>,
    pub predicted_trajectory: Vec<Position3D>,
    pub collision_risk_level: RiskLevel,
    pub avoidance_maneuver: Option<AvoidanceManeuver>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Velocity3D {
    pub vx: f32,    // m/s
    pub vy: f32,    // m/s
    pub vz: f32,    // m/s
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
    pub separation_verification: Option<SeparationVerification>,
    pub priority: u8,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeparationVerification {
    pub pre_maneuver_distance_m: f64,
    pub post_maneuver_distance_m: f64,
    pub minimum_required_m: f64,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedConflictPair {
    pub drone1_id: Uuid,
    pub drone2_id: Uuid,
    pub risk_level: RiskLevel,
    pub time_to_conflict: std::time::Duration,
    pub predicted_distance_m: f64,
    pub threshold_m: f64,
    pub predicted_position_1: Position3D,
    pub predicted_position_2: Position3D,
}

impl PredictedConflictPair {
    pub fn involves(&self, drone_id: Uuid) -> bool {
        self.drone1_id == drone_id || self.drone2_id == drone_id
    }
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
                    distance: 20.0,
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

    pub async fn register_drone(
        &mut self,
        drone_id: Uuid,
        initial_position: Position3D,
    ) -> Result<()> {
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
            let lon_change = (tracking_info.velocity.vx as f64 * dt)
                / (111_111.0 * current_pos.latitude.to_radians().cos().abs().max(0.0001));
            let alt_change = tracking_info.velocity.vz * dt as f32;

            current_pos.latitude += lat_change;
            current_pos.longitude += lon_change;
            current_pos.altitude_m += alt_change;

            trajectory.push(current_pos.clone());
        }

        trajectory
    }

    pub async fn assess_predicted_conflicts(&self) -> Vec<PredictedConflictPair> {
        let mut drones: Vec<DroneTrackingInfo> = self.tracked_drones.values().cloned().collect();
        drones.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));

        let mut conflicts = Vec::new();
        for i in 0..drones.len() {
            for j in (i + 1)..drones.len() {
                if let Some(conflict) = self.predict_pair_conflict(&drones[i], &drones[j]).await {
                    conflicts.push(conflict);
                }
            }
        }

        conflicts.sort_by(|left, right| {
            left.time_to_conflict
                .cmp(&right.time_to_conflict)
                .then_with(|| left.drone1_id.as_bytes().cmp(right.drone1_id.as_bytes()))
                .then_with(|| left.drone2_id.as_bytes().cmp(right.drone2_id.as_bytes()))
        });
        conflicts
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

    async fn calculate_collision_risk(
        &self,
        drone1_id: Uuid,
        drone2_id: Uuid,
    ) -> Result<RiskLevel> {
        let drone1 = self
            .tracked_drones
            .get(&drone1_id)
            .ok_or_else(|| anyhow::anyhow!("Drone 1 not found"))?;
        let drone2 = self
            .tracked_drones
            .get(&drone2_id)
            .ok_or_else(|| anyhow::anyhow!("Drone 2 not found"))?;

        // Calculate current distance
        let current_distance = self.calculate_3d_distance(&drone1.position, &drone2.position);

        let predicted_conflict = self.predict_pair_conflict(drone1, drone2).await;

        // Determine risk level based on distance and trajectory
        let risk = if current_distance < 15.0 {
            RiskLevel::Critical
        } else if current_distance < self.min_separation_distance {
            RiskLevel::High
        } else if let Some(conflict) = predicted_conflict {
            conflict.risk_level
        } else if current_distance < self.min_separation_distance * 2.0 {
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

    async fn predict_pair_conflict(
        &self,
        drone1: &DroneTrackingInfo,
        drone2: &DroneTrackingInfo,
    ) -> Option<PredictedConflictPair> {
        let current_distance = self.calculate_3d_distance(&drone1.position, &drone2.position);
        if current_distance < self.min_separation_distance {
            return Some(self.build_predicted_conflict(
                drone1.id,
                drone2.id,
                std::time::Duration::ZERO,
                &drone1.position,
                &drone2.position,
                current_distance,
            ));
        }

        let trajectory1 = self.predict_trajectory(drone1).await;
        let trajectory2 = self.predict_trajectory(drone2).await;

        for (step_index, (pos1, pos2)) in trajectory1.iter().zip(trajectory2.iter()).enumerate() {
            let distance = self.calculate_3d_distance(pos1, pos2);
            if distance < self.min_separation_distance {
                return Some(self.build_predicted_conflict(
                    drone1.id,
                    drone2.id,
                    std::time::Duration::from_secs((step_index + 1) as u64),
                    pos1,
                    pos2,
                    distance,
                ));
            }
        }

        None
    }

    fn build_predicted_conflict(
        &self,
        drone1_id: Uuid,
        drone2_id: Uuid,
        time_to_conflict: std::time::Duration,
        predicted_position_1: &Position3D,
        predicted_position_2: &Position3D,
        predicted_distance_m: f64,
    ) -> PredictedConflictPair {
        PredictedConflictPair {
            drone1_id,
            drone2_id,
            risk_level: self.risk_level_for_predicted_distance(predicted_distance_m),
            time_to_conflict,
            predicted_distance_m,
            threshold_m: self.min_separation_distance,
            predicted_position_1: predicted_position_1.clone(),
            predicted_position_2: predicted_position_2.clone(),
        }
    }

    fn risk_level_for_predicted_distance(&self, distance_m: f64) -> RiskLevel {
        if distance_m < 15.0 {
            RiskLevel::Critical
        } else if distance_m < self.min_separation_distance {
            RiskLevel::High
        } else if distance_m < self.min_separation_distance * 2.0 {
            RiskLevel::Medium
        } else if distance_m < self.min_separation_distance * 4.0 {
            RiskLevel::Low
        } else {
            RiskLevel::None
        }
    }

    async fn plan_avoidance_maneuver(
        &mut self,
        drone_id: Uuid,
        conflicting_drone_id: Uuid,
    ) -> Result<()> {
        let drone = self
            .tracked_drones
            .get(&drone_id)
            .ok_or_else(|| anyhow::anyhow!("Drone not found: {}", drone_id))?
            .clone();
        let conflicting_drone = self
            .tracked_drones
            .get(&conflicting_drone_id)
            .ok_or_else(|| {
                anyhow::anyhow!("Conflicting drone not found: {}", conflicting_drone_id)
            })?
            .clone();
        let current_distance =
            self.calculate_3d_distance(&drone.position, &conflicting_drone.position);

        // Find appropriate avoidance rule
        let applicable_rule = self
            .avoidance_rules
            .iter()
            .filter(|rule| rule.enabled && current_distance <= rule.trigger_distance)
            .min_by_key(|rule| rule.priority)
            .cloned()
            .or_else(|| {
                self.avoidance_rules
                    .iter()
                    .filter(|rule| rule.enabled)
                    .max_by(|left, right| {
                        left.trigger_distance
                            .partial_cmp(&right.trigger_distance)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .cloned()
            });

        if let Some(rule) = applicable_rule {
            let (maneuver_type, target_position, verification) =
                self.plan_verified_target(&rule.maneuver_type, &drone, &conflicting_drone);
            let maneuver = AvoidanceManeuver {
                maneuver_type,
                start_time: Utc::now(),
                estimated_duration: std::time::Duration::from_secs(30),
                target_position,
                separation_verification: Some(verification),
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

    fn plan_verified_target(
        &self,
        maneuver_type: &ManeuverType,
        drone: &DroneTrackingInfo,
        conflicting_drone: &DroneTrackingInfo,
    ) -> (ManeuverType, Option<Position3D>, SeparationVerification) {
        let pre_distance = self.calculate_3d_distance(&drone.position, &conflicting_drone.position);

        if matches!(
            maneuver_type,
            ManeuverType::EmergencyStop | ManeuverType::Hover
        ) && pre_distance < self.min_separation_distance
        {
            return (
                ManeuverType::EmergencyStop,
                None,
                self.verify_target(pre_distance, None, conflicting_drone),
            );
        }

        let mut candidate =
            self.candidate_target_for_maneuver(maneuver_type, drone, conflicting_drone);
        let mut verification =
            self.verify_target(pre_distance, candidate.as_ref(), conflicting_drone);

        if !verification.verified {
            candidate = Some(self.safe_altitude_target(drone, conflicting_drone));
            verification = self.verify_target(pre_distance, candidate.as_ref(), conflicting_drone);
        }

        if !verification.verified {
            candidate = Some(self.safe_horizontal_target(drone, conflicting_drone, 0.0, 0.0));
            verification = self.verify_target(pre_distance, candidate.as_ref(), conflicting_drone);
        }

        if verification.verified {
            (maneuver_type.clone(), candidate, verification)
        } else {
            (
                ManeuverType::EmergencyStop,
                None,
                self.verify_target(pre_distance, None, conflicting_drone),
            )
        }
    }

    fn candidate_target_for_maneuver(
        &self,
        maneuver_type: &ManeuverType,
        drone: &DroneTrackingInfo,
        conflicting_drone: &DroneTrackingInfo,
    ) -> Option<Position3D> {
        match maneuver_type {
            ManeuverType::AltitudeChange { delta } => {
                Some(self.safe_altitude_target_with_delta(drone, conflicting_drone, *delta))
            }
            ManeuverType::HorizontalDeviation {
                angle_deg,
                distance,
            } => Some(self.safe_horizontal_target(drone, conflicting_drone, *angle_deg, *distance)),
            ManeuverType::SpeedReduction { factor } => {
                Some(self.speed_reduction_target(drone, *factor))
            }
            ManeuverType::EmergencyStop | ManeuverType::Hover | ManeuverType::ReturnToBase => {
                Some(drone.position.clone())
            }
        }
    }

    fn verify_target(
        &self,
        pre_distance: f64,
        target: Option<&Position3D>,
        conflicting_drone: &DroneTrackingInfo,
    ) -> SeparationVerification {
        let post_distance = target
            .map(|target| self.calculate_3d_distance(target, &conflicting_drone.position))
            .unwrap_or(pre_distance);

        SeparationVerification {
            pre_maneuver_distance_m: pre_distance,
            post_maneuver_distance_m: post_distance,
            minimum_required_m: self.min_separation_distance,
            verified: post_distance >= self.min_separation_distance,
        }
    }

    fn safe_altitude_target(
        &self,
        drone: &DroneTrackingInfo,
        conflicting_drone: &DroneTrackingInfo,
    ) -> Position3D {
        self.safe_altitude_target_with_delta(drone, conflicting_drone, 0.0)
    }

    fn safe_altitude_target_with_delta(
        &self,
        drone: &DroneTrackingInfo,
        conflicting_drone: &DroneTrackingInfo,
        requested_delta: f32,
    ) -> Position3D {
        let horizontal_distance =
            self.calculate_horizontal_distance_m(&drone.position, &conflicting_drone.position);
        let required_vertical = if horizontal_distance >= self.min_separation_distance {
            requested_delta.abs() as f64
        } else {
            (self.min_separation_distance.powi(2) - horizontal_distance.powi(2)).sqrt() + 1.0
        };
        let altitude_delta = requested_delta.abs().max(required_vertical as f32).max(1.0);
        let sign = if drone.position.altitude_m >= conflicting_drone.position.altitude_m {
            1.0
        } else {
            -1.0
        };

        let mut target = drone.position.clone();
        target.altitude_m += sign * altitude_delta;
        target
    }

    fn safe_horizontal_target(
        &self,
        drone: &DroneTrackingInfo,
        conflicting_drone: &DroneTrackingInfo,
        angle_deg: f32,
        requested_distance: f32,
    ) -> Position3D {
        let (east_m, north_m) =
            self.horizontal_vector_m(&conflicting_drone.position, &drone.position);
        let norm = (east_m.powi(2) + north_m.powi(2)).sqrt();
        let (unit_east, unit_north) = if norm > f64::EPSILON {
            (east_m / norm, north_m / norm)
        } else {
            let angle = (angle_deg as f64).to_radians();
            (angle.sin(), angle.cos())
        };
        let required_distance =
            requested_distance.max((self.min_separation_distance + 1.0) as f32) as f64;

        self.offset_position_m(
            &drone.position,
            unit_east * required_distance,
            unit_north * required_distance,
            0.0,
        )
    }

    fn speed_reduction_target(&self, drone: &DroneTrackingInfo, factor: f32) -> Position3D {
        self.offset_position_m(
            &drone.position,
            drone.velocity.vx as f64 * factor as f64,
            drone.velocity.vy as f64 * factor as f64,
            drone.velocity.vz * factor,
        )
    }

    fn calculate_horizontal_distance_m(&self, pos1: &Position3D, pos2: &Position3D) -> f64 {
        let (east_m, north_m) = self.horizontal_vector_m(pos1, pos2);
        (east_m.powi(2) + north_m.powi(2)).sqrt()
    }

    fn horizontal_vector_m(&self, from: &Position3D, to: &Position3D) -> (f64, f64) {
        let mean_lat_rad = ((from.latitude + to.latitude) / 2.0).to_radians();
        let meters_per_lat_degree = 111_111.0;
        let meters_per_lon_degree = (111_111.0 * mean_lat_rad.cos().abs()).max(1.0);
        (
            (to.longitude - from.longitude) * meters_per_lon_degree,
            (to.latitude - from.latitude) * meters_per_lat_degree,
        )
    }

    fn offset_position_m(
        &self,
        position: &Position3D,
        east_m: f64,
        north_m: f64,
        altitude_delta_m: f32,
    ) -> Position3D {
        let lat_rad = position.latitude.to_radians();
        let meters_per_lat_degree = 111_111.0;
        let meters_per_lon_degree = (111_111.0 * lat_rad.cos().abs()).max(1.0);
        Position3D {
            latitude: position.latitude + north_m / meters_per_lat_degree,
            longitude: position.longitude + east_m / meters_per_lon_degree,
            altitude_m: position.altitude_m + altitude_delta_m,
        }
    }

    pub async fn get_avoidance_command(&self, drone_id: Uuid) -> Result<Option<AvoidanceCommand>> {
        if let Some(tracking_info) = self.tracked_drones.get(&drone_id) {
            if let Some(maneuver) = &tracking_info.avoidance_maneuver {
                let command = match &maneuver.maneuver_type {
                    ManeuverType::AltitudeChange { delta } => AvoidanceCommand::ChangeAltitude {
                        target_altitude: tracking_info.altitude + delta,
                        urgency: self.risk_to_urgency(&tracking_info.collision_risk_level),
                    },
                    ManeuverType::HorizontalDeviation {
                        angle_deg,
                        distance,
                    } => AvoidanceCommand::HorizontalManeuver {
                        heading_change: *angle_deg,
                        distance: *distance,
                        urgency: self.risk_to_urgency(&tracking_info.collision_risk_level),
                    },
                    ManeuverType::SpeedReduction { factor } => AvoidanceCommand::ChangeSpeed {
                        speed_factor: *factor,
                        urgency: self.risk_to_urgency(&tracking_info.collision_risk_level),
                    },
                    ManeuverType::EmergencyStop => AvoidanceCommand::EmergencyStop {
                        reason: maneuver.reason.clone(),
                    },
                    ManeuverType::ReturnToBase => AvoidanceCommand::ReturnToBase {
                        reason: maneuver.reason.clone(),
                    },
                    ManeuverType::Hover => AvoidanceCommand::Hover {
                        duration: maneuver.estimated_duration,
                    },
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
        let drones_at_risk = self
            .tracked_drones
            .values()
            .filter(|d| d.collision_risk_level >= RiskLevel::Medium)
            .count();

        let active_maneuvers = self
            .tracked_drones
            .values()
            .filter(|d| d.avoidance_maneuver.is_some())
            .count();

        CollisionAvoidanceStatus {
            total_tracked_drones: total_drones,
            drones_at_risk,
            active_maneuvers,
            system_health: if drones_at_risk == 0 {
                1.0
            } else {
                1.0 - (drones_at_risk as f32 / total_drones as f32)
            },
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

        system
            .register_drone(drone1_id, pos1.clone())
            .await
            .unwrap();
        system
            .register_drone(drone2_id, pos2.clone())
            .await
            .unwrap();

        let velocity = Velocity3D {
            vx: 5.0,
            vy: 0.0,
            vz: 0.0,
            speed: 5.0,
        };

        system
            .update_drone_state(drone1_id, pos1, velocity.clone(), 0.0)
            .await
            .unwrap();
        system
            .update_drone_state(drone2_id, pos2, velocity, 180.0)
            .await
            .unwrap();

        let status = system.get_system_status().await;
        assert_eq!(status.total_tracked_drones, 2);
    }

    #[test]
    fn test_distance_calculation() {
        let system = CollisionAvoidanceSystem::new();
        let three_meters_lon_deg = (3.0_f64 / 6_371_000.0).to_degrees();

        let pos1 = Position3D {
            latitude: 0.0,
            longitude: 0.0,
            altitude_m: 0.0,
        };

        let pos2 = Position3D {
            latitude: 0.0,
            longitude: three_meters_lon_deg,
            altitude_m: 4.0,
        };

        let distance = system.calculate_3d_distance(&pos1, &pos2);
        assert!((distance - 5.0).abs() < 0.1); // Should be 5m (3-4-5 triangle)
    }

    #[tokio::test]
    async fn converging_close_approach_plans_target_and_verifies_minimum_separation() {
        let mut system = CollisionAvoidanceSystem::new();
        let drone1_id = Uuid::new_v4();
        let drone2_id = Uuid::new_v4();
        let thirty_meters_lat_deg = (30.0_f64 / 6_371_000.0).to_degrees();
        let pos1 = Position3D {
            latitude: 0.0,
            longitude: 0.0,
            altitude_m: 100.0,
        };
        let pos2 = Position3D {
            latitude: thirty_meters_lat_deg,
            longitude: 0.0,
            altitude_m: 100.0,
        };

        system
            .register_drone(drone1_id, pos1.clone())
            .await
            .unwrap();
        system
            .register_drone(drone2_id, pos2.clone())
            .await
            .unwrap();
        system
            .update_drone_state(
                drone1_id,
                pos1,
                Velocity3D {
                    vx: 0.0,
                    vy: 5.0,
                    vz: 0.0,
                    speed: 5.0,
                },
                0.0,
            )
            .await
            .unwrap();
        system
            .update_drone_state(
                drone2_id,
                pos2,
                Velocity3D {
                    vx: 0.0,
                    vy: -5.0,
                    vz: 0.0,
                    speed: 5.0,
                },
                180.0,
            )
            .await
            .unwrap();

        let maneuver = system
            .tracked_drones
            .values()
            .filter_map(|tracking| tracking.avoidance_maneuver.as_ref())
            .find(|maneuver| maneuver.target_position.is_some())
            .expect("close approach should produce a safe maneuver target");
        let verification = maneuver
            .separation_verification
            .as_ref()
            .expect("maneuver should record separation verification");

        assert!(maneuver.target_position.is_some());
        assert!(verification.verified);
        assert!(verification.post_maneuver_distance_m >= verification.minimum_required_m);
    }

    #[tokio::test]
    async fn unresolved_overlap_escalates_without_returning_unsafe_target() {
        let mut system = CollisionAvoidanceSystem::new();
        let drone1_id = Uuid::new_v4();
        let drone2_id = Uuid::new_v4();
        let shared_pos = Position3D {
            latitude: 0.0,
            longitude: 0.0,
            altitude_m: 100.0,
        };
        let stationary = Velocity3D {
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            speed: 0.0,
        };

        system
            .register_drone(drone1_id, shared_pos.clone())
            .await
            .unwrap();
        system
            .register_drone(drone2_id, shared_pos.clone())
            .await
            .unwrap();
        system
            .update_drone_state(drone1_id, shared_pos.clone(), stationary.clone(), 0.0)
            .await
            .unwrap();
        system
            .update_drone_state(drone2_id, shared_pos, stationary, 0.0)
            .await
            .unwrap();

        let maneuver = system
            .tracked_drones
            .values()
            .filter_map(|tracking| tracking.avoidance_maneuver.as_ref())
            .find(|maneuver| matches!(maneuver.maneuver_type, ManeuverType::EmergencyStop))
            .expect("unresolved overlap should escalate to emergency stop");
        let verification = maneuver
            .separation_verification
            .as_ref()
            .expect("emergency escalation should record failed verification");

        assert!(maneuver.target_position.is_none());
        assert!(!verification.verified);
        assert!(verification.post_maneuver_distance_m < verification.minimum_required_m);
    }

    #[tokio::test]
    async fn converging_trajectory_assessment_reports_time_to_conflict() {
        let mut system = CollisionAvoidanceSystem::new();
        let drone1_id = Uuid::new_v4();
        let drone2_id = Uuid::new_v4();
        let sixty_meters_lat_deg = (60.0_f64 / 6_371_000.0).to_degrees();
        let pos1 = Position3D {
            latitude: 0.0,
            longitude: 0.0,
            altitude_m: 100.0,
        };
        let pos2 = Position3D {
            latitude: sixty_meters_lat_deg,
            longitude: 0.0,
            altitude_m: 100.0,
        };

        system
            .register_drone(drone1_id, pos1.clone())
            .await
            .unwrap();
        system
            .register_drone(drone2_id, pos2.clone())
            .await
            .unwrap();
        system
            .update_drone_state(
                drone1_id,
                pos1,
                Velocity3D {
                    vx: 0.0,
                    vy: 10.0,
                    vz: 0.0,
                    speed: 10.0,
                },
                0.0,
            )
            .await
            .unwrap();
        system
            .update_drone_state(
                drone2_id,
                pos2,
                Velocity3D {
                    vx: 0.0,
                    vy: -10.0,
                    vz: 0.0,
                    speed: 10.0,
                },
                180.0,
            )
            .await
            .unwrap();

        let conflicts = system.assess_predicted_conflicts().await;
        assert_eq!(conflicts.len(), 1);

        let conflict = &conflicts[0];
        assert!(conflict.involves(drone1_id));
        assert!(conflict.involves(drone2_id));
        assert_eq!(conflict.time_to_conflict, std::time::Duration::from_secs(2));
        assert!(conflict.predicted_distance_m < conflict.threshold_m);
        assert!(conflict.risk_level >= RiskLevel::High);
    }

    #[tokio::test]
    async fn diverging_trajectory_assessment_reports_no_false_conflict() {
        let mut system = CollisionAvoidanceSystem::new();
        let drone1_id = Uuid::new_v4();
        let drone2_id = Uuid::new_v4();
        let forty_meters_lat_deg = (40.0_f64 / 6_371_000.0).to_degrees();
        let pos1 = Position3D {
            latitude: 0.0,
            longitude: 0.0,
            altitude_m: 100.0,
        };
        let pos2 = Position3D {
            latitude: forty_meters_lat_deg,
            longitude: 0.0,
            altitude_m: 100.0,
        };

        system
            .register_drone(drone1_id, pos1.clone())
            .await
            .unwrap();
        system
            .register_drone(drone2_id, pos2.clone())
            .await
            .unwrap();
        system
            .update_drone_state(
                drone1_id,
                pos1,
                Velocity3D {
                    vx: 0.0,
                    vy: -5.0,
                    vz: 0.0,
                    speed: 5.0,
                },
                180.0,
            )
            .await
            .unwrap();
        system
            .update_drone_state(
                drone2_id,
                pos2,
                Velocity3D {
                    vx: 0.0,
                    vy: 5.0,
                    vz: 0.0,
                    speed: 5.0,
                },
                0.0,
            )
            .await
            .unwrap();

        let conflicts = system.assess_predicted_conflicts().await;
        assert!(conflicts.is_empty());
        assert!(system
            .tracked_drones
            .values()
            .all(|drone| drone.collision_risk_level < RiskLevel::Medium));
    }
}
