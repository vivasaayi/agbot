use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use anyhow::Result;
use shared::{GeoCoordinate, Mission};

/// Multi-drone coordination system
pub struct CoordinationEngine {
    active_drones: HashMap<Uuid, DroneState>,
    coordination_rules: Vec<CoordinationRule>,
    communication_range: f64, // meters
    update_interval: std::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneState {
    pub id: Uuid,
    pub position: GeoCoordinate,
    pub velocity: (f32, f32, f32),
    pub heading: f32,
    pub battery_level: f32,
    pub status: DroneOperationStatus,
    pub current_mission: Option<Uuid>,
    pub last_update: DateTime<Utc>,
    pub communication_quality: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DroneOperationStatus {
    Idle,
    InTransit,
    ExecutingMission,
    Returning,
    Emergency,
    Maintenance,
}

pub struct CoordinationRule {
    pub id: Uuid,
    pub name: String,
    pub priority: u8,
    pub condition: RuleCondition,
    pub action: RuleAction,
    pub enabled: bool,
}

impl Clone for CoordinationRule {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            priority: self.priority,
            condition: self.condition.clone(),
            action: self.action.clone(),
            enabled: self.enabled,
        }
    }
}

impl std::fmt::Debug for CoordinationRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoordinationRule")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("priority", &self.priority)
            .field("condition", &format_args!("RuleCondition"))
            .field("action", &format_args!("RuleAction"))
            .field("enabled", &self.enabled)
            .finish()
    }
}

pub enum RuleCondition {
    ProximityAlert { distance_threshold: f64 },
    BatteryLow { threshold: f32 },
    CommunicationLoss { timeout_seconds: u32 },
    WeatherCondition { condition: String },
    Custom(Box<dyn Fn(&[DroneState]) -> bool + Send + Sync>),
}

impl Clone for RuleCondition {
    fn clone(&self) -> Self {
        match self {
            RuleCondition::ProximityAlert { distance_threshold } => {
                RuleCondition::ProximityAlert { distance_threshold: *distance_threshold }
            }
            RuleCondition::BatteryLow { threshold } => {
                RuleCondition::BatteryLow { threshold: *threshold }
            }
            RuleCondition::CommunicationLoss { timeout_seconds } => {
                RuleCondition::CommunicationLoss { timeout_seconds: *timeout_seconds }
            }
            RuleCondition::WeatherCondition { condition } => {
                RuleCondition::WeatherCondition { condition: condition.clone() }
            }
            RuleCondition::Custom(_) => {
                // Cannot clone functions, so create a dummy one
                RuleCondition::Custom(Box::new(|_| false))
            }
        }
    }
}

impl std::fmt::Debug for RuleCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleCondition::ProximityAlert { distance_threshold } => {
                f.debug_struct("ProximityAlert").field("distance_threshold", distance_threshold).finish()
            }
            RuleCondition::BatteryLow { threshold } => {
                f.debug_struct("BatteryLow").field("threshold", threshold).finish()
            }
            RuleCondition::CommunicationLoss { timeout_seconds } => {
                f.debug_struct("CommunicationLoss").field("timeout_seconds", timeout_seconds).finish()
            }
            RuleCondition::WeatherCondition { condition } => {
                f.debug_struct("WeatherCondition").field("condition", condition).finish()
            }
            RuleCondition::Custom(_) => {
                f.debug_struct("Custom").field("function", &"<closure>").finish()
            }
        }
    }
}

pub enum RuleAction {
    ChangeSpeed { factor: f32 },
    ChangeAltitude { delta: f32 },
    ReturnToBase,
    LandImmediate,
    FormFormation { formation_type: String },
    SendAlert { message: String },
    Custom(Box<dyn Fn(&mut DroneState) + Send + Sync>),
}

impl Clone for RuleAction {
    fn clone(&self) -> Self {
        match self {
            RuleAction::ChangeSpeed { factor } => RuleAction::ChangeSpeed { factor: *factor },
            RuleAction::ChangeAltitude { delta } => RuleAction::ChangeAltitude { delta: *delta },
            RuleAction::ReturnToBase => RuleAction::ReturnToBase,
            RuleAction::LandImmediate => RuleAction::LandImmediate,
            RuleAction::FormFormation { formation_type } => {
                RuleAction::FormFormation { formation_type: formation_type.clone() }
            }
            RuleAction::SendAlert { message } => {
                RuleAction::SendAlert { message: message.clone() }
            }
            RuleAction::Custom(_) => {
                // Cannot clone functions, so create a dummy one
                RuleAction::Custom(Box::new(|_| {}))
            }
        }
    }
}

impl std::fmt::Debug for RuleAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleAction::ChangeSpeed { factor } => {
                f.debug_struct("ChangeSpeed").field("factor", factor).finish()
            }
            RuleAction::ChangeAltitude { delta } => {
                f.debug_struct("ChangeAltitude").field("delta", delta).finish()
            }
            RuleAction::ReturnToBase => f.debug_struct("ReturnToBase").finish(),
            RuleAction::LandImmediate => f.debug_struct("LandImmediate").finish(),
            RuleAction::FormFormation { formation_type } => {
                f.debug_struct("FormFormation").field("formation_type", formation_type).finish()
            }
            RuleAction::SendAlert { message } => {
                f.debug_struct("SendAlert").field("message", message).finish()
            }
            RuleAction::Custom(_) => {
                f.debug_struct("Custom").field("function", &"<closure>").finish()
            }
        }
    }
}

impl CoordinationEngine {
    pub fn new() -> Self {
        Self {
            active_drones: HashMap::new(),
            coordination_rules: Self::default_rules(),
            communication_range: 1000.0, // 1km
            update_interval: std::time::Duration::from_millis(500),
        }
    }

    fn default_rules() -> Vec<CoordinationRule> {
        vec![
            CoordinationRule {
                id: Uuid::new_v4(),
                name: "Collision Avoidance".to_string(),
                priority: 1,
                condition: RuleCondition::ProximityAlert { distance_threshold: 50.0 },
                action: RuleAction::ChangeAltitude { delta: 10.0 },
                enabled: true,
            },
            CoordinationRule {
                id: Uuid::new_v4(),
                name: "Low Battery Return".to_string(),
                priority: 2,
                condition: RuleCondition::BatteryLow { threshold: 0.2 },
                action: RuleAction::ReturnToBase,
                enabled: true,
            },
            CoordinationRule {
                id: Uuid::new_v4(),
                name: "Communication Loss".to_string(),
                priority: 1,
                condition: RuleCondition::CommunicationLoss { timeout_seconds: 30 },
                action: RuleAction::LandImmediate,
                enabled: true,
            },
        ]
    }

    pub async fn register_drone(&mut self, drone_id: Uuid, initial_state: DroneState) -> Result<()> {
        self.active_drones.insert(drone_id, initial_state);
        tracing::info!("Registered drone {} for coordination", drone_id);
        Ok(())
    }

    pub async fn unregister_drone(&mut self, drone_id: Uuid) -> Result<()> {
        self.active_drones.remove(&drone_id);
        tracing::info!("Unregistered drone {} from coordination", drone_id);
        Ok(())
    }

    pub async fn update_drone_state(&mut self, drone_id: Uuid, state: DroneState) -> Result<()> {
        if let Some(existing_state) = self.active_drones.get_mut(&drone_id) {
            *existing_state = state;
            
            // Check coordination rules
            self.evaluate_rules(drone_id).await?;
        } else {
            return Err(anyhow::anyhow!("Drone not registered: {}", drone_id));
        }
        Ok(())
    }

    pub async fn evaluate_rules(&self, _drone_id: Uuid) -> Result<()> {
        let states: Vec<&DroneState> = self.active_drones.values().collect();
        
        for rule in &self.coordination_rules {
            if !rule.enabled {
                continue;
            }

            let should_trigger = match &rule.condition {
                RuleCondition::ProximityAlert { distance_threshold } => {
                    self.check_proximity_violations(*distance_threshold, &states)
                }
                RuleCondition::BatteryLow { threshold } => {
                    states.iter().any(|s| s.battery_level < *threshold)
                }
                RuleCondition::CommunicationLoss { timeout_seconds } => {
                    let timeout = chrono::Duration::seconds(*timeout_seconds as i64);
                    let cutoff = Utc::now() - timeout;
                    states.iter().any(|s| s.last_update < cutoff)
                }
                RuleCondition::WeatherCondition { condition: _ } => {
                    // TODO: Integrate with weather data
                    false
                }
                RuleCondition::Custom(_) => {
                    // TODO: Implement custom rule evaluation
                    false
                }
            };

            if should_trigger {
                tracing::warn!("Coordination rule triggered: {}", rule.name);
                // TODO: Execute rule action
            }
        }

        Ok(())
    }

    fn check_proximity_violations(&self, threshold: f64, states: &[&DroneState]) -> bool {
        for (i, state1) in states.iter().enumerate() {
            for state2 in states.iter().skip(i + 1) {
                let distance = self.calculate_distance(&state1.position, &state2.position);
                if distance < threshold {
                    return true;
                }
            }
        }
        false
    }

    fn calculate_distance(&self, pos1: &GeoCoordinate, pos2: &GeoCoordinate) -> f64 {
        let lat1 = pos1.latitude.to_radians();
        let lat2 = pos2.latitude.to_radians();
        let delta_lat = (pos2.latitude - pos1.latitude).to_radians();
        let delta_lon = (pos2.longitude - pos1.longitude).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2) +
            lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0_f64 * a.sqrt().atan2((1.0 - a).sqrt());

        6371000.0 * c // Earth radius in meters
    }

    pub async fn get_coordination_status(&self) -> CoordinationStatus {
        let total_drones = self.active_drones.len();
        let active_drones = self.active_drones.values()
            .filter(|s| !matches!(s.status, DroneOperationStatus::Maintenance))
            .count();

        let coordination_quality = if total_drones > 0 {
            self.active_drones.values()
                .map(|s| s.communication_quality)
                .sum::<f32>() / total_drones as f32
        } else {
            1.0
        };

        CoordinationStatus {
            total_drones,
            active_drones,
            coordination_quality,
            active_rules: self.coordination_rules.iter().filter(|r| r.enabled).count(),
            last_update: Utc::now(),
        }
    }

    pub async fn optimize_formations(&mut self) -> Result<()> {
        // TODO: Implement formation optimization algorithms
        tracing::info!("Formation optimization not yet implemented");
        Ok(())
    }

    pub async fn handle_emergency(&mut self, drone_id: Uuid, emergency_type: EmergencyType) -> Result<()> {
        if let Some(state) = self.active_drones.get_mut(&drone_id) {
            state.status = DroneOperationStatus::Emergency;
            
            match emergency_type {
                EmergencyType::BatteryDepleted => {
                    tracing::error!("Battery depleted for drone {}", drone_id);
                }
                EmergencyType::SystemFailure => {
                    tracing::error!("System failure for drone {}", drone_id);
                }
                EmergencyType::WeatherHazard => {
                    tracing::error!("Weather hazard affecting drone {}", drone_id);
                }
                EmergencyType::CollisionRisk => {
                    tracing::error!("Collision risk for drone {}", drone_id);
                }
            }
        }
        
        Ok(())
    }

    pub async fn execute_action(&mut self, _swarm_id: Uuid, _action: String) -> Result<()> {
        // TODO: Implement action execution logic
        tracing::info!("Action execution not yet implemented");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationStatus {
    pub total_drones: usize,
    pub active_drones: usize,
    pub coordination_quality: f32,
    pub active_rules: usize,
    pub last_update: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmergencyType {
    BatteryDepleted,
    SystemFailure,
    WeatherHazard,
    CollisionRisk,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coordination_engine() {
        let mut engine = CoordinationEngine::new();
        
        let drone_id = Uuid::new_v4();
        let state = DroneState {
            id: drone_id,
            position: GeoCoordinate {
                latitude: 40.7128,
                longitude: -74.0060,
                altitude_m: 100.0,
            },
            velocity: (5.0, 0.0, 0.0),
            heading: 0.0,
            battery_level: 0.8,
            status: DroneOperationStatus::Idle,
            current_mission: None,
            last_update: Utc::now(),
            communication_quality: 0.95,
        };

        engine.register_drone(drone_id, state).await.unwrap();
        
        let status = engine.get_coordination_status().await;
        assert_eq!(status.total_drones, 1);
        assert_eq!(status.active_drones, 1);
    }

    #[test]
    fn test_distance_calculation() {
        let engine = CoordinationEngine::new();
        
        let pos1 = GeoCoordinate {
            latitude: 40.7128,
            longitude: -74.0060,
            altitude_m: 100.0,
        };
        
        let pos2 = GeoCoordinate {
            latitude: 40.7129,
            longitude: -74.0061,
            altitude_m: 100.0,
        };
        
        let distance = engine.calculate_distance(&pos1, &pos2);
        assert!(distance > 0.0);
        assert!(distance < 200.0); // Should be less than 200m for this small difference
    }
}
