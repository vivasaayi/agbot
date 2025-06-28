use anyhow::Result;
use serde::{Deserialize, Serialize};
use nalgebra::{Vector3, Point3};
use crate::{Drone, environment::Environment};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsState {
    pub position: Point3<f32>,
    pub velocity: Vector3<f32>,
    pub acceleration: Vector3<f32>,
    pub orientation: Vector3<f32>, // Roll, Pitch, Yaw
    pub angular_velocity: Vector3<f32>,
    pub forces: Vector3<f32>,
    pub torques: Vector3<f32>,
}

pub struct DronePhysics {
    pub state: PhysicsState,
    pub mass: f32,           // kg
    pub drag_coefficient: f32,
    pub max_thrust: f32,     // Newtons
    pub inertia: Vector3<f32>, // Moment of inertia
}

impl DronePhysics {
    pub fn new(drone: &Drone) -> Self {
        Self {
            state: PhysicsState {
                position: drone.position,
                velocity: drone.velocity,
                acceleration: Vector3::zeros(),
                orientation: drone.orientation,
                angular_velocity: Vector3::zeros(),
                forces: Vector3::zeros(),
                torques: Vector3::zeros(),
            },
            mass: 2.5,           // 2.5 kg typical drone
            drag_coefficient: 0.1,
            max_thrust: 30.0,    // 30N max thrust
            inertia: Vector3::new(0.1, 0.1, 0.2), // kg⋅m²
        }
    }

    pub fn update(&mut self, dt: f32, environment: &Environment) -> Result<PhysicsState> {
        // Reset forces
        self.state.forces = Vector3::zeros();
        self.state.torques = Vector3::zeros();

        // Apply gravity
        self.state.forces.y -= self.mass * 9.81;

        // Apply wind forces from environment
        let wind_force = self.calculate_wind_force(environment);
        self.state.forces += wind_force;

        // Apply drag
        let drag_force = self.calculate_drag_force();
        self.state.forces += drag_force;

        // Apply thrust (from flight controller)
        let thrust_force = self.calculate_thrust_force();
        self.state.forces += thrust_force;

        // Update linear motion
        self.state.acceleration = self.state.forces / self.mass;
        self.state.velocity += self.state.acceleration * dt;
        self.state.position += self.state.velocity * dt;

        // Ground collision
        if self.state.position.z <= 0.0 {
            self.state.position.z = 0.0;
            if self.state.velocity.z < 0.0 {
                self.state.velocity.z = 0.0;
            }
        }

        // Update angular motion (simplified)
        let angular_acc = Vector3::new(
            self.state.torques.x / self.inertia.x,
            self.state.torques.y / self.inertia.y,
            self.state.torques.z / self.inertia.z,
        );
        
        self.state.angular_velocity += angular_acc * dt;
        self.state.orientation += self.state.angular_velocity * dt;

        // Apply damping to angular velocity
        self.state.angular_velocity *= 0.95;

        Ok(self.state.clone())
    }

    fn calculate_wind_force(&self, environment: &Environment) -> Vector3<f32> {
        let conditions = environment.get_conditions();
        let wind_velocity = Vector3::new(
            conditions.wind_speed_ms * conditions.wind_direction_rad.cos(),
            0.0, // No vertical wind for simplicity
            conditions.wind_speed_ms * conditions.wind_direction_rad.sin(),
        );

        // Wind force based on relative velocity
        let relative_velocity = wind_velocity - self.state.velocity;
        let force_magnitude = 0.5 * conditions.air_density * 
                             self.drag_coefficient * 
                             relative_velocity.magnitude_squared();
        
        if relative_velocity.magnitude() > 0.0 {
            relative_velocity.normalize() * force_magnitude
        } else {
            Vector3::zeros()
        }
    }

    fn calculate_drag_force(&self) -> Vector3<f32> {
        if self.state.velocity.magnitude() > 0.0 {
            let drag_magnitude = 0.5 * 1.225 * // Air density at sea level
                                 self.drag_coefficient * 
                                 self.state.velocity.magnitude_squared();
            -self.state.velocity.normalize() * drag_magnitude
        } else {
            Vector3::zeros()
        }
    }

    fn calculate_thrust_force(&self) -> Vector3<f32> {
        // This would be controlled by the flight controller
        // For now, apply basic hover thrust
        let hover_thrust = self.mass * 9.81;
        Vector3::new(0.0, hover_thrust, 0.0)
    }

    pub fn apply_thrust(&mut self, thrust: Vector3<f32>) {
        self.state.forces += thrust;
    }

    pub fn apply_torque(&mut self, torque: Vector3<f32>) {
        self.state.torques += torque;
    }

    pub fn get_state(&self) -> &PhysicsState {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::EnvironmentConditions;

    #[test]
    fn test_physics_creation() {
        let drone = Drone::new("Test".to_string(), "Quad".to_string());
        let physics = DronePhysics::new(&drone);
        assert_eq!(physics.mass, 2.5);
    }

    #[test]
    fn test_physics_update() {
        let drone = Drone::new("Test".to_string(), "Quad".to_string());
        let mut physics = DronePhysics::new(&drone);
        let environment = Environment::new();
        
        let state = physics.update(0.1, &environment).unwrap();
        // Should have some acceleration due to gravity
        assert!(state.acceleration.y < 0.0);
    }
}
