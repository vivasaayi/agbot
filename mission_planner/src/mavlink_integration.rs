use crate::{Mission, WaypointType};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};
use uuid::Uuid;

/// MAVLink message types and conversion utilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MAVLinkMissionItem {
    pub seq: u16,
    pub frame: u8,
    pub command: u16,
    pub current: u8,
    pub autocontinue: u8,
    pub param1: f32,
    pub param2: f32,
    pub param3: f32,
    pub param4: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub mission_type: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MAVLinkMission {
    pub target_system: u8,
    pub target_component: u8,
    pub count: u16,
    pub items: Vec<MAVLinkMissionItem>,
}

// MAVLink command IDs
pub const MAV_CMD_NAV_TAKEOFF: u16 = 22;
pub const MAV_CMD_NAV_WAYPOINT: u16 = 16;
pub const MAV_CMD_NAV_LAND: u16 = 21;
pub const MAV_CMD_DO_SET_SERVO: u16 = 183;
pub const MAV_CMD_IMAGE_START_CAPTURE: u16 = 2000;
pub const MAV_CMD_IMAGE_STOP_CAPTURE: u16 = 2001;
pub const MAV_CMD_DO_DIGICAM_CONTROL: u16 = 203;

// MAVLink frames
pub const MAV_FRAME_GLOBAL_RELATIVE_ALT: u8 = 3;
pub const MAV_FRAME_MISSION: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MAVLinkAckConfig {
    pub timeout: Duration,
    pub max_retries: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MAVLinkCommandAckStatus {
    Pending,
    Acked,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MAVLinkCommandAckRecord {
    pub correlation_id: Uuid,
    pub command: u16,
    pub status: MAVLinkCommandAckStatus,
    pub sent_at: DateTime<Utc>,
    pub last_attempt_at: DateTime<Utc>,
    pub deadline: DateTime<Utc>,
    pub retry_count: u8,
    pub acked_at: Option<DateTime<Utc>>,
    pub latency_ms: Option<i64>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MAVLinkCommandAckErrorCode {
    UnknownCommand,
    CommandMismatch,
    TerminalCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MAVLinkCommandAckError {
    pub code: MAVLinkCommandAckErrorCode,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct MAVLinkCommandAckTracker {
    config: MAVLinkAckConfig,
    records: HashMap<Uuid, MAVLinkCommandAckRecord>,
}

impl Default for MAVLinkAckConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::seconds(1),
            max_retries: 2,
        }
    }
}

impl Default for MAVLinkCommandAckTracker {
    fn default() -> Self {
        Self::new(MAVLinkAckConfig::default())
    }
}

impl MAVLinkCommandAckTracker {
    pub fn new(config: MAVLinkAckConfig) -> Self {
        Self {
            config,
            records: HashMap::new(),
        }
    }

    pub fn start_command(
        &mut self,
        correlation_id: Uuid,
        command: u16,
        sent_at: DateTime<Utc>,
    ) -> MAVLinkCommandAckRecord {
        let record = MAVLinkCommandAckRecord {
            correlation_id,
            command,
            status: MAVLinkCommandAckStatus::Pending,
            sent_at,
            last_attempt_at: sent_at,
            deadline: sent_at + self.config.timeout,
            retry_count: 0,
            acked_at: None,
            latency_ms: None,
            failure_reason: None,
        };
        self.records.insert(correlation_id, record.clone());
        record
    }

    pub fn handle_ack(
        &mut self,
        correlation_id: Uuid,
        ack_command: u16,
        acked_at: DateTime<Utc>,
    ) -> Result<MAVLinkCommandAckRecord, MAVLinkCommandAckError> {
        let record = self
            .records
            .get_mut(&correlation_id)
            .ok_or_else(|| MAVLinkCommandAckError::unknown(correlation_id))?;
        if record.status != MAVLinkCommandAckStatus::Pending {
            return Err(MAVLinkCommandAckError {
                code: MAVLinkCommandAckErrorCode::TerminalCommand,
                message: format!("command {} is already {:?}", correlation_id, record.status),
            });
        }
        if record.command != ack_command {
            return Err(MAVLinkCommandAckError {
                code: MAVLinkCommandAckErrorCode::CommandMismatch,
                message: format!(
                    "ack command {} does not match pending command {}",
                    ack_command, record.command
                ),
            });
        }

        record.status = MAVLinkCommandAckStatus::Acked;
        record.acked_at = Some(acked_at);
        record.latency_ms = Some((acked_at - record.sent_at).num_milliseconds().max(0));
        Ok(record.clone())
    }

    pub fn tick(
        &mut self,
        correlation_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<MAVLinkCommandAckRecord, MAVLinkCommandAckError> {
        let record = self
            .records
            .get_mut(&correlation_id)
            .ok_or_else(|| MAVLinkCommandAckError::unknown(correlation_id))?;
        if record.status != MAVLinkCommandAckStatus::Pending {
            return Ok(record.clone());
        }
        if now < record.deadline {
            return Ok(record.clone());
        }

        if record.retry_count < self.config.max_retries {
            record.retry_count += 1;
            record.last_attempt_at = now;
            record.deadline = now + self.config.timeout;
            return Ok(record.clone());
        }

        record.status = MAVLinkCommandAckStatus::Failed;
        record.failure_reason = Some(format!(
            "ack timeout after {} retries for command {}",
            record.retry_count, record.command
        ));
        Ok(record.clone())
    }

    pub fn record(&self, correlation_id: Uuid) -> Option<&MAVLinkCommandAckRecord> {
        self.records.get(&correlation_id)
    }
}

impl MAVLinkCommandAckError {
    fn unknown(correlation_id: Uuid) -> Self {
        Self {
            code: MAVLinkCommandAckErrorCode::UnknownCommand,
            message: format!("unknown command correlation id {}", correlation_id),
        }
    }
}

impl fmt::Display for MAVLinkCommandAckError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for MAVLinkCommandAckError {}

pub struct MAVLinkConverter;

impl MAVLinkConverter {
    pub fn mission_to_mavlink(mission: &Mission) -> Result<MAVLinkMission> {
        let mut items = Vec::new();
        let mut seq = 0u16;

        // Add takeoff command for first waypoint
        if let Some(first_waypoint) = mission.waypoints.first() {
            items.push(MAVLinkMissionItem {
                seq,
                frame: MAV_FRAME_GLOBAL_RELATIVE_ALT,
                command: MAV_CMD_NAV_TAKEOFF,
                current: if seq == 0 { 1 } else { 0 },
                autocontinue: 1,
                param1: 0.0, // Pitch
                param2: 0.0, // Empty
                param3: 0.0, // Empty
                param4: 0.0, // Yaw angle
                x: first_waypoint.position.x() as f32,
                y: first_waypoint.position.y() as f32,
                z: first_waypoint.altitude_m,
                mission_type: 0,
            });
            seq += 1;
        }

        // Convert waypoints to MAVLink items
        for waypoint in &mission.waypoints {
            let command = match waypoint.waypoint_type {
                WaypointType::Takeoff => MAV_CMD_NAV_TAKEOFF,
                WaypointType::Landing => MAV_CMD_NAV_LAND,
                _ => MAV_CMD_NAV_WAYPOINT,
            };

            items.push(MAVLinkMissionItem {
                seq,
                frame: MAV_FRAME_GLOBAL_RELATIVE_ALT,
                command,
                current: 0,
                autocontinue: 1,
                param1: 0.0, // Hold time
                param2: 3.0, // Accept radius (meters)
                param3: 0.0, // Pass radius
                param4: 0.0, // Yaw
                x: waypoint.position.x() as f32,
                y: waypoint.position.y() as f32,
                z: waypoint.altitude_m,
                mission_type: 0,
            });

            // Add action commands
            for action in &waypoint.actions {
                seq += 1;
                match action {
                    crate::waypoint::Action::TakePhoto { .. } => {
                        items.push(MAVLinkMissionItem {
                            seq,
                            frame: MAV_FRAME_MISSION,
                            command: MAV_CMD_IMAGE_START_CAPTURE,
                            current: 0,
                            autocontinue: 1,
                            param1: 0.0, // Camera ID
                            param2: 0.0, // Interval
                            param3: 1.0, // Total images
                            param4: 0.0, // Sequence number
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                            mission_type: 0,
                        });
                    }
                    crate::waypoint::Action::Hover { duration_seconds } => {
                        // Modify the previous waypoint to include hold time
                        if let Some(last_item) = items.last_mut() {
                            last_item.param1 = *duration_seconds as f32;
                        }
                    }
                    crate::waypoint::Action::SetSpeed { speed_ms } => {
                        items.push(MAVLinkMissionItem {
                            seq,
                            frame: MAV_FRAME_MISSION,
                            command: 178, // MAV_CMD_DO_CHANGE_SPEED
                            current: 0,
                            autocontinue: 1,
                            param1: 1.0, // Speed type (1 = ground speed)
                            param2: *speed_ms,
                            param3: -1.0, // Throttle (-1 = no change)
                            param4: 0.0,  // Absolute or relative
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                            mission_type: 0,
                        });
                    }
                    _ => {
                        // Skip unsupported actions for now
                        seq -= 1; // Don't increment seq for unsupported actions
                    }
                }
            }

            seq += 1;
        }

        // Add landing command if not already present
        let has_landing = items.iter().any(|item| item.command == MAV_CMD_NAV_LAND);
        if !has_landing && !mission.waypoints.is_empty() {
            if let Some(last_waypoint) = mission.waypoints.last() {
                items.push(MAVLinkMissionItem {
                    seq,
                    frame: MAV_FRAME_GLOBAL_RELATIVE_ALT,
                    command: MAV_CMD_NAV_LAND,
                    current: 0,
                    autocontinue: 1,
                    param1: 0.0, // Abort altitude
                    param2: 0.0, // Precision land mode
                    param3: 0.0, // Empty
                    param4: 0.0, // Yaw angle
                    x: last_waypoint.position.x() as f32,
                    y: last_waypoint.position.y() as f32,
                    z: 0.0, // Land altitude
                    mission_type: 0,
                });
            }
        }

        Ok(MAVLinkMission {
            target_system: 1,
            target_component: 1,
            count: items.len() as u16,
            items,
        })
    }

    pub fn to_waypoint_file(mavlink_mission: &MAVLinkMission) -> String {
        let mut output = String::new();
        output.push_str("QGC WPL 110\n");

        for item in &mavlink_mission.items {
            let line = format!(
                "{}\t{}\t{}\t{}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.8}\t{:.8}\t{:.6}\t{}\n",
                item.seq,
                item.current,
                item.frame,
                item.command,
                item.param1,
                item.param2,
                item.param3,
                item.param4,
                item.x,
                item.y,
                item.z,
                item.autocontinue
            );
            output.push_str(&line);
        }

        output
    }

    pub fn estimate_flight_time(mavlink_mission: &MAVLinkMission, cruise_speed_ms: f32) -> f32 {
        let mut total_time = 0.0;
        let mut last_position: Option<(f32, f32, f32)> = None;

        for item in &mavlink_mission.items {
            match item.command {
                MAV_CMD_NAV_TAKEOFF => {
                    total_time += 30.0; // Assume 30 seconds for takeoff
                    last_position = Some((item.x, item.y, item.z));
                }
                MAV_CMD_NAV_WAYPOINT => {
                    if let Some((last_x, last_y, _last_z)) = last_position {
                        let distance =
                            ((item.x - last_x).powi(2) + (item.y - last_y).powi(2)).sqrt();
                        let flight_time = distance * 111_320.0 / cruise_speed_ms; // Convert degrees to meters
                        total_time += flight_time;
                        total_time += item.param1; // Add hold time
                    }
                    last_position = Some((item.x, item.y, item.z));
                }
                MAV_CMD_NAV_LAND => {
                    total_time += 60.0; // Assume 60 seconds for landing
                }
                _ => {
                    // Add time for other commands if needed
                }
            }
        }

        total_time
    }
}

#[cfg(test)]
mod command_ack_tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};
    use uuid::Uuid;

    #[test]
    fn command_ack_marks_command_acked_with_latency() {
        let command_id = Uuid::new_v4();
        let mut tracker = MAVLinkCommandAckTracker::new(MAVLinkAckConfig {
            timeout: Duration::seconds(1),
            max_retries: 2,
        });
        let sent_at = Utc.timestamp_opt(100, 0).unwrap();

        tracker.start_command(command_id, MAV_CMD_NAV_TAKEOFF, sent_at);
        let record = tracker
            .handle_ack(
                command_id,
                MAV_CMD_NAV_TAKEOFF,
                sent_at + Duration::milliseconds(150),
            )
            .expect("matching ack should succeed");

        assert_eq!(record.status, MAVLinkCommandAckStatus::Acked);
        assert_eq!(record.latency_ms, Some(150));
        assert_eq!(record.retry_count, 0);
    }

    #[test]
    fn command_timeout_retries_then_fails_without_ack() {
        let command_id = Uuid::new_v4();
        let mut tracker = MAVLinkCommandAckTracker::new(MAVLinkAckConfig {
            timeout: Duration::seconds(1),
            max_retries: 1,
        });
        let sent_at = Utc.timestamp_opt(100, 0).unwrap();

        tracker.start_command(command_id, MAV_CMD_NAV_TAKEOFF, sent_at);
        let retry = tracker
            .tick(command_id, sent_at + Duration::seconds(1))
            .expect("known command should tick");
        assert_eq!(retry.status, MAVLinkCommandAckStatus::Pending);
        assert_eq!(retry.retry_count, 1);

        let failed = tracker
            .tick(command_id, sent_at + Duration::seconds(2))
            .expect("known command should tick");
        assert_eq!(failed.status, MAVLinkCommandAckStatus::Failed);
        assert_eq!(failed.retry_count, 1);
        assert!(failed
            .failure_reason
            .as_deref()
            .unwrap()
            .contains("ack timeout"));
    }
}
