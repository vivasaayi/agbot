use shared::{
    config::AgroConfig,
    schemas::{Telemetry, GpsCoords, WebSocketMessage},
    AgroResult,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_serial::SerialPortBuilderExt;
use tracing::{info, warn, error};

pub struct MavlinkClient {
    config: Arc<AgroConfig>,
    event_tx: broadcast::Sender<WebSocketMessage>,
}

impl MavlinkClient {
    pub async fn new(
        config: Arc<AgroConfig>,
        event_tx: broadcast::Sender<WebSocketMessage>,
    ) -> AgroResult<Self> {
        Ok(Self { config, event_tx })
    }

    pub async fn run(&self) -> AgroResult<()> {
        info!("Connecting to flight controller on {}", self.config.mavlink.serial_port);

        let mut port = tokio_serial::new(&self.config.mavlink.serial_port, self.config.mavlink.baud_rate)
            .open_native_async()
            .map_err(|e| shared::error::AgroError::Mavlink(format!("Failed to open serial port: {}", e)))?;

        let mut heartbeat_interval = tokio::time::interval(
            std::time::Duration::from_millis(self.config.mavlink.heartbeat_interval_ms)
        );

        let mut telemetry_buffer = Vec::new();

        loop {
            tokio::select! {
                _ = heartbeat_interval.tick() => {
                    self.send_heartbeat(&mut port).await?;
                }
                result = self.read_telemetry(&mut port) => {
                    match result {
                        Ok(telemetry) => {
                            telemetry_buffer.push(telemetry.clone());
                            
                            // Send telemetry update
                            let msg = WebSocketMessage::Telemetry { data: telemetry };
                            if let Err(e) = self.event_tx.send(msg) {
                                warn!("Failed to send telemetry update: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to read telemetry: {}", e);
                        }
                    }
                }
            }
        }
    }

    async fn send_heartbeat(&self, port: &mut tokio_serial::SerialStream) -> AgroResult<()> {
        use mavlink::common::*;
        
        let heartbeat = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
            custom_mode: 0,
            mavtype: MavType::MAV_TYPE_GCS,
            autopilot: MavAutopilot::MAV_AUTOPILOT_INVALID,
            base_mode: MavModeFlag::empty(),
            system_status: MavState::MAV_STATE_ACTIVE,
            mavlink_version: 3,
        });

        let header = mavlink::MavHeader {
            system_id: 255,
            component_id: 0,
            sequence: 0,
        };

        let mut buf = Vec::new();
        mavlink::write_versioned_msg(&mut buf, mavlink::MavlinkVersion::V2, header, &heartbeat)
            .map_err(|e| shared::error::AgroError::Mavlink(format!("Failed to serialize heartbeat: {}", e)))?;

        tokio::io::AsyncWriteExt::write_all(port, &buf).await
            .map_err(|e| shared::error::AgroError::Mavlink(format!("Failed to send heartbeat: {}", e)))?;

        Ok(())
    }

    async fn read_telemetry(&self, port: &mut tokio_serial::SerialStream) -> AgroResult<Telemetry> {
        use tokio::io::AsyncReadExt;
        
        let mut buf = [0u8; 1024];
        let _n = port.read(&mut buf).await
            .map_err(|e| shared::error::AgroError::Mavlink(format!("Failed to read from port: {}", e)))?;

        // Parse MAVLink messages (simplified)
        // In a real implementation, you'd use proper MAVLink parsing
        
        // For now, return mock telemetry
        Ok(Telemetry {
            timestamp: chrono::Utc::now(),
            position: GpsCoords {
                latitude: self.config.gps.home_latitude,
                longitude: self.config.gps.home_longitude,
                altitude: self.config.gps.home_altitude,
            },
            battery_voltage: 12.6,
            battery_percentage: 85,
            armed: false,
            mode: "STABILIZE".to_string(),
            ground_speed: 0.0,
            air_speed: 0.0,
            heading: 0.0,
            altitude_relative: 0.0,
        })
    }
}

pub struct SimulatedMavlinkClient {
    config: Arc<AgroConfig>,
    event_tx: broadcast::Sender<WebSocketMessage>,
}

impl SimulatedMavlinkClient {
    pub fn new(
        config: Arc<AgroConfig>,
        event_tx: broadcast::Sender<WebSocketMessage>,
    ) -> Self {
        Self { config, event_tx }
    }

    pub async fn run(&self) -> AgroResult<()> {
        info!("Starting simulated MAVLink client");

        let mut telemetry_interval = tokio::time::interval(
            std::time::Duration::from_millis(1000)
        );

        let mut battery_percentage = 100u8;
        let mut altitude = 0.0f32;

        loop {
            telemetry_interval.tick().await;

            // Simulate battery drain
            if battery_percentage > 0 && rand::random::<f32>() < 0.01 {
                battery_percentage = battery_percentage.saturating_sub(1);
            }

            // Simulate altitude changes
            altitude += (rand::random::<f32>() - 0.5) * 2.0;
            altitude = altitude.max(0.0).min(100.0);

            let telemetry = Telemetry {
                timestamp: chrono::Utc::now(),
                position: GpsCoords {
                    latitude: self.config.gps.home_latitude + (rand::random::<f64>() - 0.5) * 0.001,
                    longitude: self.config.gps.home_longitude + (rand::random::<f64>() - 0.5) * 0.001,
                    altitude: self.config.gps.home_altitude + altitude as f64,
                },
                battery_voltage: 12.6 - (100 - battery_percentage) as f32 * 0.01,
                battery_percentage,
                armed: rand::random::<f32>() > 0.7,
                mode: "SIMULATION".to_string(),
                ground_speed: rand::random::<f32>() * 10.0,
                air_speed: rand::random::<f32>() * 12.0,
                heading: rand::random::<f32>() * 360.0,
                altitude_relative: altitude,
            };

            let msg = WebSocketMessage::Telemetry { data: telemetry };
            if let Err(e) = self.event_tx.send(msg) {
                warn!("Failed to send simulated telemetry: {}", e);
            }
        }
    }
}
