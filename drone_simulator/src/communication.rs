use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Telemetry(TelemetryMessage),
    Command(CommandMessage),
    Status(StatusMessage),
    Emergency(EmergencyMessage),
    Heartbeat(HeartbeatMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryMessage {
    pub drone_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub position: (f32, f32, f32),
    pub velocity: (f32, f32, f32),
    pub orientation: (f32, f32, f32),
    pub battery_level: f32,
    pub signal_strength: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMessage {
    pub drone_id: Uuid,
    pub command_id: Uuid,
    pub command: String,
    pub parameters: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusMessage {
    pub drone_id: Uuid,
    pub status: String,
    pub details: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyMessage {
    pub drone_id: Uuid,
    pub emergency_type: String,
    pub description: String,
    pub position: (f32, f32, f32),
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatMessage {
    pub drone_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub system_time: DateTime<Utc>,
    pub uptime_ms: u64,
}

pub struct CommunicationModule {
    drone_id: Uuid,
    message_sender: mpsc::UnboundedSender<MessageType>,
    message_receiver: mpsc::UnboundedReceiver<MessageType>,
    outbound_sender: mpsc::UnboundedSender<MessageType>,
    signal_strength: f32,
    last_heartbeat: DateTime<Utc>,
    heartbeat_interval_ms: u64,
}

impl CommunicationModule {
    pub fn new(drone_id: Uuid) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let (outbound_sender, _) = mpsc::unbounded_channel();

        Self {
            drone_id,
            message_sender: sender,
            message_receiver: receiver,
            outbound_sender,
            signal_strength: 1.0,
            last_heartbeat: Utc::now(),
            heartbeat_interval_ms: 1000, // 1 second
        }
    }

    pub async fn send_message(&self, message: MessageType) -> Result<()> {
        // Simulate signal degradation
        if self.signal_strength < 0.3 {
            return Err(anyhow::anyhow!("Signal too weak to send message"));
        }

        self.outbound_sender.send(message)
            .map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;
        
        Ok(())
    }

    pub fn receive_message(&mut self) -> Option<MessageType> {
        self.message_receiver.try_recv().ok()
    }

    pub async fn send_telemetry(
        &self,
        position: (f32, f32, f32),
        velocity: (f32, f32, f32),
        orientation: (f32, f32, f32),
        battery_level: f32,
    ) -> Result<()> {
        let telemetry = TelemetryMessage {
            drone_id: self.drone_id,
            timestamp: Utc::now(),
            position,
            velocity,
            orientation,
            battery_level,
            signal_strength: self.signal_strength,
        };

        self.send_message(MessageType::Telemetry(telemetry)).await
    }

    pub async fn send_status(&self, status: String, details: String) -> Result<()> {
        let status_msg = StatusMessage {
            drone_id: self.drone_id,
            status,
            details,
            timestamp: Utc::now(),
        };

        self.send_message(MessageType::Status(status_msg)).await
    }

    pub async fn send_emergency(&self, emergency_type: String, description: String, position: (f32, f32, f32)) -> Result<()> {
        let emergency = EmergencyMessage {
            drone_id: self.drone_id,
            emergency_type,
            description,
            position,
            timestamp: Utc::now(),
        };

        self.send_message(MessageType::Emergency(emergency)).await
    }

    pub async fn update(&mut self) -> Result<()> {
        let now = Utc::now();
        
        // Send heartbeat if needed
        if (now - self.last_heartbeat).num_milliseconds() > self.heartbeat_interval_ms as i64 {
            self.send_heartbeat().await?;
            self.last_heartbeat = now;
        }

        // Update signal strength based on conditions
        self.update_signal_strength();

        Ok(())
    }

    async fn send_heartbeat(&self) -> Result<()> {
        let heartbeat = HeartbeatMessage {
            drone_id: self.drone_id,
            timestamp: Utc::now(),
            system_time: Utc::now(),
            uptime_ms: 0, // Simplified
        };

        self.send_message(MessageType::Heartbeat(heartbeat)).await
    }

    fn update_signal_strength(&mut self) {
        // Simplified signal strength model
        // In reality, this would depend on distance, obstacles, interference, etc.
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        // Add some random variation
        let variation = rng.gen_range(-0.05..0.05);
        self.signal_strength = (self.signal_strength + variation).clamp(0.0, 1.0);
    }

    pub fn get_signal_strength(&self) -> f32 {
        self.signal_strength
    }

    pub fn set_signal_strength(&mut self, strength: f32) {
        self.signal_strength = strength.clamp(0.0, 1.0);
    }

    pub fn is_connected(&self) -> bool {
        self.signal_strength > 0.1
    }

    pub fn get_subscriber(&self) -> mpsc::UnboundedSender<MessageType> {
        self.outbound_sender.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_communication_module() {
        let drone_id = Uuid::new_v4();
        let comm = CommunicationModule::new(drone_id);
        assert_eq!(comm.drone_id, drone_id);
        assert!(comm.is_connected());
    }

    #[tokio::test]
    async fn test_send_telemetry() {
        let drone_id = Uuid::new_v4();
        let comm = CommunicationModule::new(drone_id);
        
        let result = comm.send_telemetry(
            (0.0, 0.0, 100.0),
            (1.0, 0.0, 0.0),
            (0.0, 0.0, 0.0),
            0.8,
        ).await;
        
        assert!(result.is_ok());
    }
}
