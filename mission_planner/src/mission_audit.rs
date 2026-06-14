use crate::{
    mavlink_integration::MAVLinkCommandAckRecord, GuardedDispatchAuditEvent,
    GuardedDispatchAuditEventKind, MissionStatus, MissionTelemetrySample, SafetyViolation,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared::schemas::Telemetry;
use std::collections::{BTreeSet, HashMap};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionAuditEventKind {
    CommandSent,
    CommandAcked,
    CommandFailed,
    TelemetrySample,
    SafetyViolation,
    ModeTransition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionAuditEvent {
    pub id: Uuid,
    pub mission_id: Uuid,
    pub sequence: u64,
    pub occurred_at: DateTime<Utc>,
    pub kind: MissionAuditEventKind,
    pub correlation_id: Option<Uuid>,
    pub drone_id: Option<String>,
    pub message: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionAuditTimeline {
    pub mission_id: Uuid,
    pub events: Vec<MissionAuditEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionAuditGapCode {
    MissingCommandAck,
    MissingCommandSent,
    MissingExecutedCommandAudit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionAuditGap {
    pub code: MissionAuditGapCode,
    pub mission_id: Uuid,
    pub correlation_id: Option<Uuid>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionAuditValidationReport {
    pub mission_id: Uuid,
    pub gaps: Vec<MissionAuditGap>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionReplayEventKind {
    Command,
    Telemetry,
    Safety,
    ModeTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionReplayCommandStatus {
    Sent,
    Acked,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionReplayCommand {
    pub correlation_id: Uuid,
    pub mavlink_command: u16,
    pub status: MissionReplayCommandStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionReplayEvent {
    pub sequence: u64,
    pub occurred_at: DateTime<Utc>,
    pub kind: MissionReplayEventKind,
    pub command: Option<MissionReplayCommand>,
    pub telemetry: Option<Telemetry>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionReplay {
    pub mission_id: Uuid,
    pub events: Vec<MissionReplayEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionReplayErrorCode {
    AuditGap,
    CorruptPayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionReplayError {
    pub code: MissionReplayErrorCode,
    pub message: String,
    pub validation_report: Option<MissionAuditValidationReport>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MissionAuditLog {
    events_by_mission: HashMap<Uuid, Vec<MissionAuditEvent>>,
    next_sequence_by_mission: HashMap<Uuid, u64>,
}

impl MissionAuditEvent {
    pub fn command_sent(
        mission_id: Uuid,
        correlation_id: Uuid,
        mavlink_command: u16,
        occurred_at: DateTime<Utc>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            mission_id,
            sequence: 0,
            occurred_at,
            kind: MissionAuditEventKind::CommandSent,
            correlation_id: Some(correlation_id),
            drone_id: None,
            message: message.into(),
            payload: json!({
                "mavlink_command": mavlink_command,
            }),
        }
    }

    pub fn command_ack(
        mission_id: Uuid,
        correlation_id: Uuid,
        mavlink_command: u16,
        occurred_at: DateTime<Utc>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            mission_id,
            sequence: 0,
            occurred_at,
            kind: MissionAuditEventKind::CommandAcked,
            correlation_id: Some(correlation_id),
            drone_id: None,
            message: message.into(),
            payload: json!({
                "mavlink_command": mavlink_command,
            }),
        }
    }

    pub fn command_failed(
        mission_id: Uuid,
        correlation_id: Uuid,
        mavlink_command: u16,
        occurred_at: DateTime<Utc>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            mission_id,
            sequence: 0,
            occurred_at,
            kind: MissionAuditEventKind::CommandFailed,
            correlation_id: Some(correlation_id),
            drone_id: None,
            message: message.into(),
            payload: json!({
                "mavlink_command": mavlink_command,
            }),
        }
    }

    pub fn from_guarded_dispatch(event: &GuardedDispatchAuditEvent) -> Self {
        let kind = match event.event {
            GuardedDispatchAuditEventKind::CommandSent => MissionAuditEventKind::CommandSent,
            GuardedDispatchAuditEventKind::CommandAcked => MissionAuditEventKind::CommandAcked,
        };
        Self {
            id: Uuid::new_v4(),
            mission_id: event.mission_id,
            sequence: 0,
            occurred_at: event.at,
            kind,
            correlation_id: Some(event.correlation_id),
            drone_id: None,
            message: event.message.clone(),
            payload: json!({
                "mavlink_command": event.mavlink_command,
                "guarded_dispatch_event": event.event,
            }),
        }
    }

    pub fn from_telemetry_sample(sample: &MissionTelemetrySample) -> Self {
        Self {
            id: Uuid::new_v4(),
            mission_id: sample.mission_id,
            sequence: 0,
            occurred_at: sample.telemetry.timestamp,
            kind: MissionAuditEventKind::TelemetrySample,
            correlation_id: None,
            drone_id: Some(sample.drone_id.clone()),
            message: format!("telemetry sample recorded for {}", sample.drone_id),
            payload: json!({
                "drone_id": sample.drone_id,
                "telemetry": sample.telemetry,
            }),
        }
    }

    pub fn from_safety_violation(
        mission_id: Uuid,
        occurred_at: DateTime<Utc>,
        violation: &SafetyViolation,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            mission_id,
            sequence: 0,
            occurred_at,
            kind: MissionAuditEventKind::SafetyViolation,
            correlation_id: None,
            drone_id: None,
            message: violation.message.clone(),
            payload: json!({
                "violation": violation,
            }),
        }
    }

    pub fn mode_transition(
        mission_id: Uuid,
        from: MissionStatus,
        to: MissionStatus,
        occurred_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            mission_id,
            sequence: 0,
            occurred_at,
            kind: MissionAuditEventKind::ModeTransition,
            correlation_id: None,
            drone_id: None,
            message: format!("mission mode transitioned from {from} to {to}"),
            payload: json!({
                "from": from,
                "to": to,
            }),
        }
    }
}

impl MissionAuditLog {
    pub fn append(&mut self, mut event: MissionAuditEvent) -> MissionAuditEvent {
        let next_sequence = self
            .next_sequence_by_mission
            .entry(event.mission_id)
            .or_insert(1);
        event.sequence = *next_sequence;
        *next_sequence = next_sequence.saturating_add(1);

        self.events_by_mission
            .entry(event.mission_id)
            .or_default()
            .push(event.clone());
        event
    }

    pub fn timeline(&self, mission_id: Uuid) -> MissionAuditTimeline {
        let mut events = self
            .events_by_mission
            .get(&mission_id)
            .cloned()
            .unwrap_or_default();
        events.sort_by(|left, right| {
            left.occurred_at
                .cmp(&right.occurred_at)
                .then_with(|| left.sequence.cmp(&right.sequence))
        });

        MissionAuditTimeline { mission_id, events }
    }

    pub fn validate_mission(&self, mission_id: Uuid) -> MissionAuditValidationReport {
        validate_mission_audit_timeline(&self.timeline(mission_id))
    }

    pub fn validate_mission_against_commands(
        &self,
        mission_id: Uuid,
        expected_commands: &[MAVLinkCommandAckRecord],
    ) -> MissionAuditValidationReport {
        let timeline = self.timeline(mission_id);
        let mut report = validate_mission_audit_timeline(&timeline);
        let audited_commands: BTreeSet<Uuid> = timeline
            .events
            .iter()
            .filter_map(|event| match event.kind {
                MissionAuditEventKind::CommandSent
                | MissionAuditEventKind::CommandAcked
                | MissionAuditEventKind::CommandFailed => event.correlation_id,
                MissionAuditEventKind::TelemetrySample
                | MissionAuditEventKind::SafetyViolation
                | MissionAuditEventKind::ModeTransition => None,
            })
            .collect();

        for command in expected_commands
            .iter()
            .filter(|command| !audited_commands.contains(&command.correlation_id))
        {
            report.gaps.push(MissionAuditGap {
                code: MissionAuditGapCode::MissingExecutedCommandAudit,
                mission_id,
                correlation_id: Some(command.correlation_id),
                message: format!(
                    "executed command {} is missing from mission audit",
                    command.correlation_id
                ),
            });
        }

        report
    }
}

impl MissionAuditValidationReport {
    pub fn is_clear(&self) -> bool {
        self.gaps.is_empty()
    }
}

pub fn validate_mission_audit_timeline(
    timeline: &MissionAuditTimeline,
) -> MissionAuditValidationReport {
    let mut sent_commands = BTreeSet::new();
    let mut terminal_commands = BTreeSet::new();

    for event in &timeline.events {
        match event.kind {
            MissionAuditEventKind::CommandSent => {
                if let Some(correlation_id) = event.correlation_id {
                    sent_commands.insert(correlation_id);
                }
            }
            MissionAuditEventKind::CommandAcked | MissionAuditEventKind::CommandFailed => {
                if let Some(correlation_id) = event.correlation_id {
                    terminal_commands.insert(correlation_id);
                }
            }
            MissionAuditEventKind::TelemetrySample
            | MissionAuditEventKind::SafetyViolation
            | MissionAuditEventKind::ModeTransition => {}
        }
    }

    let mut gaps = Vec::new();
    for correlation_id in sent_commands.difference(&terminal_commands) {
        gaps.push(MissionAuditGap {
            code: MissionAuditGapCode::MissingCommandAck,
            mission_id: timeline.mission_id,
            correlation_id: Some(*correlation_id),
            message: format!("command {correlation_id} is missing ack or failure audit entry"),
        });
    }
    for correlation_id in terminal_commands.difference(&sent_commands) {
        gaps.push(MissionAuditGap {
            code: MissionAuditGapCode::MissingCommandSent,
            mission_id: timeline.mission_id,
            correlation_id: Some(*correlation_id),
            message: format!("command {correlation_id} has terminal audit entry without send"),
        });
    }

    MissionAuditValidationReport {
        mission_id: timeline.mission_id,
        gaps,
    }
}

pub fn replay_mission_from_audit(
    timeline: &MissionAuditTimeline,
) -> Result<MissionReplay, MissionReplayError> {
    let validation_report = validate_mission_audit_timeline(timeline);
    if !validation_report.is_clear() {
        return Err(MissionReplayError {
            code: MissionReplayErrorCode::AuditGap,
            message: format!(
                "mission audit gap prevents replay: {} gap(s)",
                validation_report.gaps.len()
            ),
            validation_report: Some(validation_report),
        });
    }

    let events = timeline
        .events
        .iter()
        .map(replay_event_from_audit)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(MissionReplay {
        mission_id: timeline.mission_id,
        events,
    })
}

fn replay_event_from_audit(
    event: &MissionAuditEvent,
) -> Result<MissionReplayEvent, MissionReplayError> {
    match event.kind {
        MissionAuditEventKind::CommandSent
        | MissionAuditEventKind::CommandAcked
        | MissionAuditEventKind::CommandFailed => {
            let correlation_id = event.correlation_id.ok_or_else(|| {
                corrupt_replay_payload(event, "command audit entry missing correlation_id")
            })?;
            let mavlink_command = event
                .payload
                .get("mavlink_command")
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .ok_or_else(|| {
                    corrupt_replay_payload(event, "command audit entry missing mavlink_command")
                })?;
            let status = match event.kind {
                MissionAuditEventKind::CommandSent => MissionReplayCommandStatus::Sent,
                MissionAuditEventKind::CommandAcked => MissionReplayCommandStatus::Acked,
                MissionAuditEventKind::CommandFailed => MissionReplayCommandStatus::Failed,
                MissionAuditEventKind::TelemetrySample
                | MissionAuditEventKind::SafetyViolation
                | MissionAuditEventKind::ModeTransition => unreachable!(),
            };

            Ok(MissionReplayEvent {
                sequence: event.sequence,
                occurred_at: event.occurred_at,
                kind: MissionReplayEventKind::Command,
                command: Some(MissionReplayCommand {
                    correlation_id,
                    mavlink_command,
                    status,
                }),
                telemetry: None,
                message: event.message.clone(),
            })
        }
        MissionAuditEventKind::TelemetrySample => {
            let telemetry = event
                .payload
                .get("telemetry")
                .cloned()
                .ok_or_else(|| {
                    corrupt_replay_payload(event, "telemetry audit entry missing telemetry payload")
                })
                .and_then(|value| {
                    serde_json::from_value(value).map_err(|error| MissionReplayError {
                        code: MissionReplayErrorCode::CorruptPayload,
                        message: format!(
                            "telemetry audit entry {} has corrupt payload: {error}",
                            event.id
                        ),
                        validation_report: None,
                    })
                })?;

            Ok(MissionReplayEvent {
                sequence: event.sequence,
                occurred_at: event.occurred_at,
                kind: MissionReplayEventKind::Telemetry,
                command: None,
                telemetry: Some(telemetry),
                message: event.message.clone(),
            })
        }
        MissionAuditEventKind::SafetyViolation => Ok(MissionReplayEvent {
            sequence: event.sequence,
            occurred_at: event.occurred_at,
            kind: MissionReplayEventKind::Safety,
            command: None,
            telemetry: None,
            message: event.message.clone(),
        }),
        MissionAuditEventKind::ModeTransition => Ok(MissionReplayEvent {
            sequence: event.sequence,
            occurred_at: event.occurred_at,
            kind: MissionReplayEventKind::ModeTransition,
            command: None,
            telemetry: None,
            message: event.message.clone(),
        }),
    }
}

fn corrupt_replay_payload(
    event: &MissionAuditEvent,
    message: impl Into<String>,
) -> MissionReplayError {
    MissionReplayError {
        code: MissionReplayErrorCode::CorruptPayload,
        message: format!("audit event {} is corrupt: {}", event.id, message.into()),
        validation_report: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mavlink_integration::{MAVLinkCommandAckRecord, MAVLinkCommandAckStatus},
        GuardedDispatchAuditEvent, GuardedDispatchAuditEventKind, MissionTelemetrySample,
        SafetyViolation, SafetyViolationCode, SafetyViolationSeverity,
    };
    use chrono::{TimeZone, Utc};
    use shared::schemas::{GpsCoords, Telemetry};
    use uuid::Uuid;

    fn telemetry_sample(
        mission_id: Uuid,
        drone_id: &str,
        timestamp_seconds: i64,
    ) -> MissionTelemetrySample {
        MissionTelemetrySample {
            mission_id,
            drone_id: drone_id.to_string(),
            telemetry: Telemetry {
                timestamp: Utc.timestamp_opt(timestamp_seconds, 0).unwrap(),
                position: GpsCoords {
                    latitude: 41.0,
                    longitude: -96.0,
                    altitude: 400.0,
                },
                battery_voltage: 15.8,
                battery_percentage: 82,
                armed: true,
                mode: "AUTO".to_string(),
                ground_speed: 6.0,
                air_speed: 6.5,
                heading: 90.0,
                altitude_relative: 40.0,
            },
        }
    }

    #[test]
    fn mission_audit_log_reconstructs_command_telemetry_safety_timeline() {
        let mission_id = Uuid::new_v4();
        let command_id = Uuid::new_v4();
        let mut audit = MissionAuditLog::default();

        audit.append(MissionAuditEvent::mode_transition(
            mission_id,
            crate::MissionStatus::Armed,
            crate::MissionStatus::InFlight,
            Utc.timestamp_opt(100, 0).unwrap(),
        ));
        audit.append(MissionAuditEvent::from_guarded_dispatch(
            &GuardedDispatchAuditEvent {
                mission_id,
                correlation_id: command_id,
                mavlink_command: crate::mavlink_integration::MAV_CMD_NAV_TAKEOFF,
                event: GuardedDispatchAuditEventKind::CommandSent,
                at: Utc.timestamp_opt(101, 0).unwrap(),
                message: "guarded dispatch sent takeoff".to_string(),
            },
        ));
        audit.append(MissionAuditEvent::from_telemetry_sample(&telemetry_sample(
            mission_id, "drone-1", 102,
        )));
        audit.append(MissionAuditEvent::from_safety_violation(
            mission_id,
            Utc.timestamp_opt(103, 0).unwrap(),
            &SafetyViolation {
                code: SafetyViolationCode::WindSpeedExceeded,
                severity: SafetyViolationSeverity::Blocker,
                waypoint_index: None,
                zone_id: None,
                measured_value: Some(18.0),
                threshold_value: Some(15.0),
                unit: Some("m/s".to_string()),
                message: "wind speed 18.0 m/s exceeds dispatch limit 15.0 m/s".to_string(),
            },
        ));
        audit.append(MissionAuditEvent::from_guarded_dispatch(
            &GuardedDispatchAuditEvent {
                mission_id,
                correlation_id: command_id,
                mavlink_command: crate::mavlink_integration::MAV_CMD_NAV_TAKEOFF,
                event: GuardedDispatchAuditEventKind::CommandAcked,
                at: Utc.timestamp_opt(104, 0).unwrap(),
                message: "guarded dispatch acked takeoff".to_string(),
            },
        ));

        let timeline = audit.timeline(mission_id);

        assert_eq!(timeline.mission_id, mission_id);
        assert_eq!(
            timeline
                .events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            vec![
                MissionAuditEventKind::ModeTransition,
                MissionAuditEventKind::CommandSent,
                MissionAuditEventKind::TelemetrySample,
                MissionAuditEventKind::SafetyViolation,
                MissionAuditEventKind::CommandAcked,
            ]
        );
        assert_eq!(
            timeline.events[1].correlation_id,
            Some(command_id),
            "command events keep correlation IDs for reconstruction"
        );
        assert_eq!(timeline.events[0].sequence, 1);
        assert_eq!(timeline.events[4].sequence, 5);
    }

    #[test]
    fn mission_audit_validation_detects_missing_command_ack() {
        let mission_id = Uuid::new_v4();
        let command_id = Uuid::new_v4();
        let mut audit = MissionAuditLog::default();
        audit.append(MissionAuditEvent::command_sent(
            mission_id,
            command_id,
            crate::mavlink_integration::MAV_CMD_NAV_TAKEOFF,
            Utc.timestamp_opt(101, 0).unwrap(),
            "takeoff command sent",
        ));

        let report = audit.validate_mission(mission_id);

        assert!(!report.is_clear());
        assert_eq!(report.gaps.len(), 1);
        assert_eq!(report.gaps[0].code, MissionAuditGapCode::MissingCommandAck);
        assert_eq!(report.gaps[0].correlation_id, Some(command_id));
        assert!(report.gaps[0].message.contains("missing ack"));
    }

    #[test]
    fn mission_audit_validation_detects_missing_executed_command_audit() {
        let mission_id = Uuid::new_v4();
        let command_id = Uuid::new_v4();
        let sent_at = Utc.timestamp_opt(101, 0).unwrap();
        let audit = MissionAuditLog::default();
        let expected_command = MAVLinkCommandAckRecord {
            correlation_id: command_id,
            command: crate::mavlink_integration::MAV_CMD_NAV_TAKEOFF,
            status: MAVLinkCommandAckStatus::Acked,
            sent_at,
            last_attempt_at: sent_at,
            deadline: Utc.timestamp_opt(102, 0).unwrap(),
            retry_count: 0,
            acked_at: Some(Utc.timestamp_opt(101, 500_000_000).unwrap()),
            latency_ms: Some(500),
            failure_reason: None,
        };

        let report = audit.validate_mission_against_commands(mission_id, &[expected_command]);

        assert!(!report.is_clear());
        assert_eq!(report.gaps.len(), 1);
        assert_eq!(
            report.gaps[0].code,
            MissionAuditGapCode::MissingExecutedCommandAudit
        );
        assert_eq!(report.gaps[0].correlation_id, Some(command_id));
        assert!(report.gaps[0]
            .message
            .contains("missing from mission audit"));
    }

    #[test]
    fn mission_replay_reconstructs_positions_and_commands_from_audit() {
        let mission_id = Uuid::new_v4();
        let command_id = Uuid::new_v4();
        let mut audit = MissionAuditLog::default();
        audit.append(MissionAuditEvent::command_sent(
            mission_id,
            command_id,
            crate::mavlink_integration::MAV_CMD_NAV_TAKEOFF,
            Utc.timestamp_opt(100, 0).unwrap(),
            "takeoff command sent",
        ));
        audit.append(MissionAuditEvent::from_telemetry_sample(&telemetry_sample(
            mission_id, "drone-1", 101,
        )));
        audit.append(MissionAuditEvent::command_ack(
            mission_id,
            command_id,
            crate::mavlink_integration::MAV_CMD_NAV_TAKEOFF,
            Utc.timestamp_opt(102, 0).unwrap(),
            "takeoff command acked",
        ));

        let replay = replay_mission_from_audit(&audit.timeline(mission_id))
            .expect("complete audit should replay");

        assert_eq!(replay.mission_id, mission_id);
        assert_eq!(
            replay
                .events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            vec![
                MissionReplayEventKind::Command,
                MissionReplayEventKind::Telemetry,
                MissionReplayEventKind::Command,
            ]
        );
        assert_eq!(
            replay.events[0]
                .command
                .as_ref()
                .map(|command| command.status),
            Some(MissionReplayCommandStatus::Sent)
        );
        assert_eq!(
            replay.events[1]
                .telemetry
                .as_ref()
                .map(|telemetry| telemetry.position.latitude),
            Some(41.0)
        );
        assert_eq!(
            replay.events[2]
                .command
                .as_ref()
                .map(|command| command.status),
            Some(MissionReplayCommandStatus::Acked)
        );
        let encoded = serde_json::to_value(&replay).expect("replay exports as JSON");
        assert_eq!(encoded["mission_id"], mission_id.to_string());
        assert_eq!(encoded["events"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn mission_replay_reports_audit_gap_instead_of_fabricating_track() {
        let mission_id = Uuid::new_v4();
        let command_id = Uuid::new_v4();
        let mut audit = MissionAuditLog::default();
        audit.append(MissionAuditEvent::command_sent(
            mission_id,
            command_id,
            crate::mavlink_integration::MAV_CMD_NAV_TAKEOFF,
            Utc.timestamp_opt(100, 0).unwrap(),
            "takeoff command sent",
        ));

        let error = replay_mission_from_audit(&audit.timeline(mission_id))
            .expect_err("corrupted audit should not replay");

        assert_eq!(error.code, MissionReplayErrorCode::AuditGap);
        assert_eq!(
            error
                .validation_report
                .as_ref()
                .map(|report| report.gaps[0].code),
            Some(MissionAuditGapCode::MissingCommandAck)
        );
        assert!(error.message.contains("audit gap"));
    }
}
