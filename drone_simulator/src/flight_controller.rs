use anyhow::Result;
use serde::{Deserialize, Serialize};
use nalgebra::Vector3;
use tokio::sync::mpsc;
use crate::{Drone, physics::PhysicsState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlightCommand {
    Takeoff { altitude_m: f32 },
    Land,
    GoTo { x: f32, y: f32, z: f32, speed_ms: f32 },
    Hover { duration_seconds: f32 },
    SetSpeed { speed_ms: f32 },
    SetHeading { heading_degrees: f32 },
    Emergency,
    ReturnToHome,
    OrbitPoint { x: f32, y: f32, radius_m: f32, speed_ms: f32 },
}

pub struct FlightController {
    drone_id: uuid::Uuid,
    command_receiver: mpsc::UnboundedReceiver<FlightCommand>,
    command_sender: mpsc::UnboundedSender<FlightCommand>,
    current_command: Option<FlightCommand>,
    target_position: Vector3<f32>,
    target_speed: f32,
    pid_position: PidController,
    pid_velocity: PidController,
}

pub struct PidController {
    kp: f32,
    ki: f32,
    kd: f32,
    integral: Vector3<f32>,
    last_error: Vector3<f32>,
}

impl FlightController {
    pub fn new(drone: &Drone) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        Self {
            drone_id: drone.id,
            command_receiver: receiver,
            command_sender: sender,
            current_command: None,
            target_position: Vector3::new(drone.position.x, drone.position.y, drone.position.z),
            target_speed: 5.0,
            pid_position: PidController::new(1.0, 0.1, 0.2),
            pid_velocity: PidController::new(0.5, 0.05, 0.1),
        }
    }

    pub async fn send_command(&self, command: FlightCommand) -> Result<()> {
        self.command_sender.send(command)
            .map_err(|e| anyhow::anyhow!("Failed to send command: {}", e))?;
        Ok(())
    }

    pub fn update(&mut self, dt: f32, state: &PhysicsState) -> Result<()> {
        // Process any new commands
        while let Ok(command) = self.command_receiver.try_recv() {
            self.process_command(command, state)?;
        }

        // Execute current command
        if let Some(ref command) = self.current_command.clone() {
            self.execute_command(command, dt, state)?;
        }

        Ok(())
    }

    fn process_command(&mut self, command: FlightCommand, state: &PhysicsState) -> Result<()> {
        match &command {
            FlightCommand::Takeoff { altitude_m } => {
                self.target_position = Vector3::new(
                    state.position.x,
                    *altitude_m,
                    state.position.z,
                );
            }
            FlightCommand::Land => {
                self.target_position = Vector3::new(
                    state.position.x,
                    0.0,
                    state.position.z,
                );
            }
            FlightCommand::GoTo { x, y, z, speed_ms } => {
                self.target_position = Vector3::new(*x, *y, *z);
                self.target_speed = *speed_ms;
            }
            FlightCommand::Hover { .. } => {
                self.target_position = Vector3::new(
                    state.position.x,
                    state.position.y,
                    state.position.z,
                );
            }
            FlightCommand::SetSpeed { speed_ms } => {
                self.target_speed = *speed_ms;
            }
            FlightCommand::Emergency => {
                // Immediate stop and hover
                self.target_position = Vector3::new(
                    state.position.x,
                    state.position.y,
                    state.position.z,
                );
                self.target_speed = 0.0;
            }
            _ => {
                // Handle other commands
            }
        }

        self.current_command = Some(command);
        Ok(())
    }

    fn execute_command(&mut self, _command: &FlightCommand, dt: f32, state: &PhysicsState) -> Result<()> {
        // Calculate position error
        let current_pos = Vector3::new(state.position.x, state.position.y, state.position.z);
        let position_error = self.target_position - current_pos;

        // PID control for position
        let desired_velocity = self.pid_position.update(position_error, dt);

        // Calculate velocity error
        let velocity_error = desired_velocity - state.velocity;

        // PID control for velocity (results in acceleration/thrust commands)
        let _thrust_command = self.pid_velocity.update(velocity_error, dt);

        // In a real implementation, this would send thrust commands to motors
        // For simulation, the physics engine handles the actual movement

        Ok(())
    }

    pub fn get_target_position(&self) -> Vector3<f32> {
        self.target_position
    }

    pub fn get_current_command(&self) -> Option<&FlightCommand> {
        self.current_command.as_ref()
    }
}

impl PidController {
    pub fn new(kp: f32, ki: f32, kd: f32) -> Self {
        Self {
            kp,
            ki,
            kd,
            integral: Vector3::zeros(),
            last_error: Vector3::zeros(),
        }
    }

    pub fn update(&mut self, error: Vector3<f32>, dt: f32) -> Vector3<f32> {
        // Proportional term
        let proportional = error * self.kp;

        // Integral term
        self.integral += error * dt;
        let integral = self.integral * self.ki;

        // Derivative term
        let derivative = (error - self.last_error) / dt * self.kd;
        self.last_error = error;

        // PID output
        proportional + integral + derivative
    }

    pub fn reset(&mut self) {
        self.integral = Vector3::zeros();
        self.last_error = Vector3::zeros();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Drone;

    #[test]
    fn test_flight_controller_creation() {
        let drone = Drone::new("Test".to_string(), "Quad".to_string());
        let controller = FlightController::new(&drone);
        assert_eq!(controller.drone_id, drone.id);
    }

    #[test]
    fn test_pid_controller() {
        let mut pid = PidController::new(1.0, 0.1, 0.2);
        let error = Vector3::new(1.0, 0.0, 0.0);
        let output = pid.update(error, 0.1);
        assert!(output.x > 0.0); // Should respond to positive error
    }
}
