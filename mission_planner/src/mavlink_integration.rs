use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::{Mission, Waypoint, WaypointType};

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
                            param4: 0.0, // Absolute or relative
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
                    if let Some((last_x, last_y, last_z)) = last_position {
                        let distance = ((item.x - last_x).powi(2) + (item.y - last_y).powi(2)).sqrt();
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
