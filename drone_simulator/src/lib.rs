use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use nalgebra::{Vector3, Point3};
use tokio::sync::{mpsc, RwLock};
use std::sync::Arc;

pub mod physics;
pub mod sensors;
pub mod flight_controller;
pub mod communication;
pub mod environment;

pub use physics::{DronePhysics, PhysicsState};
pub use sensors::{SensorSuite, SensorReading};
pub use flight_controller::{FlightController, FlightCommand};
pub use communication::{CommunicationModule, MessageType};
pub use environment::{Environment, EnvironmentConditions};

/// Core drone simulation structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drone {
    pub id: Uuid,
    pub name: String,
    pub model: String,
    pub position: Point3<f32>,
    pub velocity: Vector3<f32>,
    pub orientation: Vector3<f32>, // Roll, Pitch, Yaw in radians
    pub battery_level: f32,        // 0.0 to 1.0
    pub status: DroneStatus,
    pub capabilities: DroneCapabilities,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DroneStatus {
    Idle,
    Armed,
    TakingOff,
    Flying,
    Hovering,
    Landing,
    Emergency,
    Crashed,
    LowBattery,
    Maintenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneCapabilities {
    pub max_speed_ms: f32,
    pub max_altitude_m: f32,
    pub max_payload_kg: f32,
    pub flight_time_minutes: u32,
    pub sensors: Vec<String>,
    pub has_camera: bool,
    pub has_lidar: bool,
    pub has_multispectral: bool,
    pub has_gps: bool,
}

/// Simulation event system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimulationEvent {
    DronePositionUpdate {
        drone_id: Uuid,
        position: Point3<f32>,
        velocity: Vector3<f32>,
        timestamp: DateTime<Utc>,
    },
    SensorReading {
        drone_id: Uuid,
        sensor_type: String,
        data: serde_json::Value,
        timestamp: DateTime<Utc>,
    },
    FlightCommandReceived {
        drone_id: Uuid,
        command: FlightCommand,
        timestamp: DateTime<Utc>,
    },
    BatteryLevelChanged {
        drone_id: Uuid,
        level: f32,
        timestamp: DateTime<Utc>,
    },
    StatusChanged {
        drone_id: Uuid,
        old_status: DroneStatus,
        new_status: DroneStatus,
        timestamp: DateTime<Utc>,
    },
    EmergencyEvent {
        drone_id: Uuid,
        event_type: EmergencyType,
        description: String,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmergencyType {
    LowBattery,
    LostCommunication,
    SensorFailure,
    WeatherWarning,
    GeofenceViolation,
    SystemFailure,
}

/// Main simulation engine
pub struct SimulationEngine {
    drones: Arc<RwLock<HashMap<Uuid, DroneSimulator>>>,
    environment: Arc<RwLock<Environment>>,
    event_sender: mpsc::UnboundedSender<SimulationEvent>,
    event_receiver: Arc<RwLock<mpsc::UnboundedReceiver<SimulationEvent>>>,
    simulation_time: Arc<RwLock<DateTime<Utc>>>,
    time_scale: Arc<RwLock<f32>>, // 1.0 = real-time, 2.0 = 2x speed, etc.
    running: Arc<RwLock<bool>>,
}

/// Individual drone simulator
pub struct DroneSimulator {
    pub drone: Drone,
    pub physics: DronePhysics,
    pub sensors: SensorSuite,
    pub flight_controller: FlightController,
    pub communication: CommunicationModule,
    last_update: DateTime<Utc>,
}

impl Drone {
    pub fn new(name: String, model: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            model,
            position: Point3::new(0.0, 0.0, 0.0),
            velocity: Vector3::zeros(),
            orientation: Vector3::zeros(),
            battery_level: 1.0,
            status: DroneStatus::Idle,
            capabilities: DroneCapabilities::default(),
            created_at: Utc::now(),
        }
    }

    pub fn with_position(mut self, x: f32, y: f32, z: f32) -> Self {
        self.position = Point3::new(x, y, z);
        self
    }

    pub fn with_capabilities(mut self, capabilities: DroneCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }
}

impl Default for DroneCapabilities {
    fn default() -> Self {
        Self {
            max_speed_ms: 20.0,
            max_altitude_m: 400.0,
            max_payload_kg: 2.0,
            flight_time_minutes: 25,
            sensors: vec![
                "GPS".to_string(),
                "IMU".to_string(),
                "Barometer".to_string(),
                "Magnetometer".to_string(),
            ],
            has_camera: true,
            has_lidar: false,
            has_multispectral: false,
            has_gps: true,
        }
    }
}

impl SimulationEngine {
    pub fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Self {
            drones: Arc::new(RwLock::new(HashMap::new())),
            environment: Arc::new(RwLock::new(Environment::new())),
            event_sender,
            event_receiver: Arc::new(RwLock::new(event_receiver)),
            simulation_time: Arc::new(RwLock::new(Utc::now())),
            time_scale: Arc::new(RwLock::new(1.0)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn add_drone(&self, drone: Drone) -> Result<Uuid> {
        let id = drone.id;
        let simulator = DroneSimulator::new(drone)?;
        
        let mut drones = self.drones.write().await;
        drones.insert(id, simulator);
        
        Ok(id)
    }

    pub async fn remove_drone(&self, id: &Uuid) -> Result<()> {
        let mut drones = self.drones.write().await;
        if drones.remove(id).is_some() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Drone not found"))
        }
    }

    pub async fn get_drone(&self, id: &Uuid) -> Option<Drone> {
        let drones = self.drones.read().await;
        drones.get(id).map(|sim| sim.drone.clone())
    }

    pub async fn send_command(&self, drone_id: &Uuid, command: FlightCommand) -> Result<()> {
        let drones = self.drones.read().await;
        if let Some(simulator) = drones.get(drone_id) {
            simulator.flight_controller.send_command(command.clone()).await?;
            
            // Send event
            let event = SimulationEvent::FlightCommandReceived {
                drone_id: *drone_id,
                command,
                timestamp: Utc::now(),
            };
            let _ = self.event_sender.send(event);
            
            Ok(())
        } else {
            Err(anyhow::anyhow!("Drone not found"))
        }
    }

    pub async fn start_simulation(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = true;
        
        // Start the main simulation loop
        let drones = self.drones.clone();
        let environment = self.environment.clone();
        let event_sender = self.event_sender.clone();
        let time_scale = self.time_scale.clone();
        let simulation_time = self.simulation_time.clone();
        let running_flag = self.running.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(50)); // 20 Hz
            
            while *running_flag.read().await {
                interval.tick().await;
                
                // Update simulation time
                {
                    let mut sim_time = simulation_time.write().await;
                    let scale = *time_scale.read().await;
                    *sim_time = *sim_time + chrono::Duration::milliseconds((50.0 * scale) as i64);
                }
                
                // Update all drones
                let mut drones_guard = drones.write().await;
                let env = environment.read().await;
                
                for (_id, simulator) in drones_guard.iter_mut() {
                    if let Ok(events) = simulator.update(&env).await {
                        for event in events {
                            let _ = event_sender.send(event);
                        }
                    }
                }
            }
        });
        
        Ok(())
    }

    pub async fn stop_simulation(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }

    pub async fn set_time_scale(&self, scale: f32) {
        let mut time_scale = self.time_scale.write().await;
        *time_scale = scale.max(0.1).min(10.0); // Clamp between 0.1x and 10x
    }

    pub async fn get_simulation_time(&self) -> DateTime<Utc> {
        *self.simulation_time.read().await
    }

    pub async fn list_drones(&self) -> Vec<Drone> {
        let drones = self.drones.read().await;
        drones.values().map(|sim| sim.drone.clone()).collect()
    }

    pub fn subscribe_to_events(&self) -> mpsc::UnboundedReceiver<SimulationEvent> {
        // In a real implementation, this would create a new receiver
        // For now, we'll need to handle this differently
        let (_sender, receiver) = mpsc::unbounded_channel();
        receiver
    }
}

impl DroneSimulator {
    pub fn new(drone: Drone) -> Result<Self> {
        Ok(Self {
            physics: DronePhysics::new(&drone),
            sensors: SensorSuite::new(&drone.capabilities),
            flight_controller: FlightController::new(&drone),
            communication: CommunicationModule::new(drone.id),
            last_update: Utc::now(),
            drone,
        })
    }

    pub async fn update(&mut self, environment: &Environment) -> Result<Vec<SimulationEvent>> {
        let now = Utc::now();
        let dt = (now - self.last_update).num_milliseconds() as f32 / 1000.0;
        self.last_update = now;

        let mut events = Vec::new();

        // Update physics
        let old_position = self.drone.position;
        let physics_state = self.physics.update(dt, environment)?;
        
        // Update drone state from physics
        self.drone.position = physics_state.position;
        self.drone.velocity = physics_state.velocity;
        self.drone.orientation = physics_state.orientation;

        // Check if position changed significantly
        if (self.drone.position - old_position).magnitude() > 0.1 {
            events.push(SimulationEvent::DronePositionUpdate {
                drone_id: self.drone.id,
                position: self.drone.position,
                velocity: self.drone.velocity,
                timestamp: now,
            });
        }

        // Update battery
        let battery_drain = self.calculate_battery_drain(dt);
        let old_battery = self.drone.battery_level;
        self.drone.battery_level = (self.drone.battery_level - battery_drain).max(0.0);

        if (old_battery - self.drone.battery_level).abs() > 0.01 {
            events.push(SimulationEvent::BatteryLevelChanged {
                drone_id: self.drone.id,
                level: self.drone.battery_level,
                timestamp: now,
            });
        }

        // Update sensors
        let sensor_readings = self.sensors.update(&physics_state, environment)?;
        for reading in sensor_readings {
            events.push(SimulationEvent::SensorReading {
                drone_id: self.drone.id,
                sensor_type: reading.sensor_type,
                data: reading.data,
                timestamp: reading.timestamp,
            });
        }

        // Update flight controller
        self.flight_controller.update(dt, &physics_state)?;

        // Check for status changes
        let new_status = self.determine_status();
        if new_status != self.drone.status {
            let old_status = self.drone.status.clone();
            self.drone.status = new_status.clone();
            
            events.push(SimulationEvent::StatusChanged {
                drone_id: self.drone.id,
                old_status,
                new_status,
                timestamp: now,
            });
        }

        // Check for emergency conditions
        if let Some(emergency) = self.check_emergency_conditions() {
            events.push(SimulationEvent::EmergencyEvent {
                drone_id: self.drone.id,
                event_type: emergency.0,
                description: emergency.1,
                timestamp: now,
            });
        }

        Ok(events)
    }

    fn calculate_battery_drain(&self, dt: f32) -> f32 {
        // Simplified battery model
        let base_drain = 0.0001 * dt; // Base systems
        let flight_drain = if matches!(self.drone.status, DroneStatus::Flying | DroneStatus::Hovering) {
            0.0005 * dt * self.drone.velocity.magnitude()
        } else {
            0.0
        };
        
        base_drain + flight_drain
    }

    fn determine_status(&self) -> DroneStatus {
        if self.drone.battery_level < 0.1 {
            DroneStatus::LowBattery
        } else if self.drone.position.z < 1.0 {
            if self.drone.velocity.magnitude() < 0.5 {
                DroneStatus::Idle
            } else {
                DroneStatus::TakingOff
            }
        } else if self.drone.velocity.magnitude() < 1.0 {
            DroneStatus::Hovering
        } else {
            DroneStatus::Flying
        }
    }

    fn check_emergency_conditions(&self) -> Option<(EmergencyType, String)> {
        if self.drone.battery_level < 0.05 {
            Some((
                EmergencyType::LowBattery,
                "Critical battery level - immediate landing required".to_string(),
            ))
        } else if self.drone.position.z > self.drone.capabilities.max_altitude_m {
            Some((
                EmergencyType::GeofenceViolation,
                "Maximum altitude exceeded".to_string(),
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_drone_creation() {
        let drone = Drone::new("Test Drone".to_string(), "Quadcopter".to_string());
        assert_eq!(drone.name, "Test Drone");
        assert_eq!(drone.model, "Quadcopter");
        assert!(matches!(drone.status, DroneStatus::Idle));
    }

    #[tokio::test]
    async fn test_simulation_engine() {
        let engine = SimulationEngine::new();
        let drone = Drone::new("Test Drone".to_string(), "Quadcopter".to_string());
        let id = drone.id;
        
        engine.add_drone(drone).await.unwrap();
        let retrieved = engine.get_drone(&id).await.unwrap();
        assert_eq!(retrieved.name, "Test Drone");
    }
}
