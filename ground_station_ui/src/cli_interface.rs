use crate::{
    ActionAckStatus, MissionControlActionAck, MissionControlActionClient,
    MissionControlActionRequest, OperatorActionAuditRecord, OperatorActionError,
    OperatorActionKind, SharedLinkState, SharedMessageDispatchState, SharedOperatorActionAuditLog,
    SharedOperatorActionState, SharedOperatorSessionRegistry,
};
use serde::Serialize;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliCommandOutcome {
    pub success: bool,
    pub status: String,
    pub message: String,
    pub ack: Option<MissionControlActionAck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliStatusSnapshot {
    pub lines: Vec<String>,
}

pub async fn cli_status_snapshot(
    link_state: SharedLinkState,
    dispatch_state: SharedMessageDispatchState,
) -> CliStatusSnapshot {
    let snapshot = link_state.read().await.snapshot();
    let dispatch = dispatch_state.read().await.clone();
    let freshness = dispatch.telemetry_freshness();
    let mut lines = vec![
        "System Status:".to_string(),
        format!("  WebSocket: {}", snapshot.state),
        format!("  Reconnect attempts: {}", snapshot.reconnect_attempts),
        format!("  Next retry: {} ms", snapshot.next_backoff.as_millis()),
        format!("  Telemetry: {}", freshness.state),
    ];
    if let Some(age) = freshness.last_update_age_seconds {
        lines.push(format!("  Telemetry age: {age} s"));
    }
    if let Some(telemetry) = dispatch.telemetry_tile_snapshot() {
        lines.push(format!(
            "  Position: {:.6}, {:.6} @ {:.1} m{}",
            telemetry.latitude,
            telemetry.longitude,
            telemetry.altitude_m,
            if telemetry.stale { " (stale)" } else { "" }
        ));
        lines.push(format!(
            "  Battery: {}% ({:.1} V)",
            telemetry.battery_percentage, telemetry.battery_voltage
        ));
        lines.push(format!(
            "  Mode: {} (armed: {})",
            telemetry.mode, telemetry.armed
        ));
    }
    lines.push(format!(
        "  Capture events: {}",
        dispatch.capture_events(None).len()
    ));
    lines.push(format!("  Malformed frames: {}", dispatch.malformed_frames));
    if let Some(error) = snapshot.last_error {
        lines.push(format!("  Last error: {error}"));
    }
    lines.push(format!("  Last state change: {}", snapshot.updated_at));
    CliStatusSnapshot { lines }
}

pub async fn submit_cli_operator_action(
    session_token: Option<&str>,
    action: OperatorActionKind,
    target_mission_id: Uuid,
    sessions: SharedOperatorSessionRegistry,
    operator_action_state: SharedOperatorActionState,
    mission_control_actions: &dyn MissionControlActionClient,
    operator_action_audit_log: SharedOperatorActionAuditLog,
) -> CliCommandOutcome {
    let Some(session_token) = session_token else {
        return CliCommandOutcome {
            success: false,
            status: "unauthorized".to_string(),
            message: "operator session token is missing".to_string(),
            ack: None,
        };
    };
    let authorized = match sessions
        .read()
        .await
        .authorize_action_at(session_token, chrono::Utc::now())
    {
        Ok(authorized) => authorized,
        Err(error) => {
            return CliCommandOutcome {
                success: false,
                status: "unauthorized".to_string(),
                message: error.to_string(),
                ack: None,
            };
        }
    };
    if let Err(error) = operator_action_state
        .read()
        .await
        .ensure_simulation_validated()
    {
        return cli_error_outcome(error, None);
    }

    let request = MissionControlActionRequest::new(
        authorized.principal,
        action,
        target_mission_id,
        chrono::Utc::now(),
    );
    let result = mission_control_actions.submit_operator_action(request.clone());
    let audit_record = match &result {
        Ok(ack) => OperatorActionAuditRecord::from_ack(&request, ack),
        Err(error) => OperatorActionAuditRecord::from_error(&request, error, chrono::Utc::now()),
    };
    if let Err(error) = operator_action_audit_log.write().await.record(audit_record) {
        return cli_error_outcome(error, None);
    }

    match result {
        Ok(ack) => cli_ack_outcome(ack),
        Err(error) => cli_error_outcome(error, None),
    }
}

fn cli_ack_outcome(ack: MissionControlActionAck) -> CliCommandOutcome {
    let status = match ack.status {
        ActionAckStatus::Accepted => "accepted",
        ActionAckStatus::Rejected => "rejected",
        ActionAckStatus::TimedOut => "timed_out",
    };
    CliCommandOutcome {
        success: ack.status == ActionAckStatus::Accepted,
        status: status.to_string(),
        message: ack.message.clone(),
        ack: Some(ack),
    }
}

fn cli_error_outcome(
    error: OperatorActionError,
    ack: Option<MissionControlActionAck>,
) -> CliCommandOutcome {
    let status = match &error {
        OperatorActionError::UnsupportedAction { .. } => "unsupported",
        OperatorActionError::SimulationLoopNotValidated => "disabled",
        OperatorActionError::MissionControlRejected { .. } => "rejected",
        OperatorActionError::MissionControlNoAck { .. } => "timed_out",
        OperatorActionError::MissionControlUnavailable { .. } => "unavailable",
        OperatorActionError::AuditWriteFailed { .. } => "audit_failed",
    };
    CliCommandOutcome {
        success: false,
        status: status.to_string(),
        message: error.to_string(),
        ack,
    }
}

pub async fn run_cli_interface(
    link_state: SharedLinkState,
    dispatch_state: SharedMessageDispatchState,
) {
    info!("CLI Ground Station Interface");
    info!("Commands: help, status, quit");

    let stdin = io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    loop {
        print!("> ");

        if let Ok(Some(line)) = lines.next_line().await {
            let command = line.trim().to_lowercase();

            match command.as_str() {
                "help" => {
                    println!("Available commands:");
                    println!("  help   - Show this help message");
                    println!("  status - Show system status");
                    println!("  quit   - Exit the application");
                }
                "status" => {
                    for line in cli_status_snapshot(link_state.clone(), dispatch_state.clone())
                        .await
                        .lines
                    {
                        println!("{line}");
                    }
                }
                "quit" | "exit" => {
                    println!("Goodbye!");
                    break;
                }
                "" => {
                    // Empty command, do nothing
                }
                _ => {
                    println!(
                        "Unknown command: {}. Type 'help' for available commands.",
                        command
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        shared_link_state, shared_message_dispatch_state, shared_operator_action_audit_log,
        shared_operator_action_state, shared_operator_session_registry, OperatorActionAuditLog,
        OperatorActionState, OperatorSessionRegistry, ReconnectPolicy,
    };
    use shared::schemas::{GpsCoords, Telemetry, WebSocketMessage};
    use std::sync::Mutex;

    #[tokio::test]
    async fn cli_status_reports_real_link_and_telemetry_values() {
        let link_state = shared_link_state(ReconnectPolicy::default());
        link_state.write().await.mark_connected();
        let dispatch_state = shared_message_dispatch_state();
        dispatch_state
            .write()
            .await
            .dispatch_message(&WebSocketMessage::Telemetry {
                data: sample_telemetry(),
            });

        let snapshot = cli_status_snapshot(link_state, dispatch_state).await;

        assert!(snapshot
            .lines
            .iter()
            .any(|line| line == "  WebSocket: Connected"));
        assert!(snapshot
            .lines
            .iter()
            .any(|line| line == "  Telemetry: Fresh"));
        assert!(snapshot
            .lines
            .iter()
            .any(|line| line.contains("Position: 42.000000, -71.000000")));
        assert!(snapshot
            .lines
            .iter()
            .any(|line| line == "  Battery: 72% (15.4 V)"));
        assert!(snapshot
            .lines
            .iter()
            .any(|line| line == "  Mode: GUIDED (armed: true)"));
    }

    #[tokio::test]
    async fn unauthenticated_cli_action_is_refused_before_mission_control_dispatch() {
        let client = RecordingMissionControlActionClient::default();
        let outcome = submit_cli_operator_action(
            None,
            OperatorActionKind::ReturnToHome,
            Uuid::new_v4(),
            shared_operator_session_registry(OperatorSessionRegistry::default()),
            shared_operator_action_state(validated_action_state()),
            &client,
            shared_operator_action_audit_log(OperatorActionAuditLog::default()),
        )
        .await;

        assert!(!outcome.success);
        assert_eq!(outcome.status, "unauthorized");
        assert_eq!(outcome.message, "operator session token is missing");
        assert_eq!(client.request_count(), 0);
    }

    fn validated_action_state() -> OperatorActionState {
        let mut state = OperatorActionState::default();
        state.mark_simulation_validated("flight_sim_cpp acceptance pass");
        state
    }

    fn sample_telemetry() -> Telemetry {
        Telemetry {
            timestamp: chrono::Utc::now(),
            position: GpsCoords {
                latitude: 42.0,
                longitude: -71.0,
                altitude: 120.0,
            },
            battery_voltage: 15.4,
            battery_percentage: 72,
            armed: true,
            mode: "GUIDED".to_string(),
            ground_speed: 6.5,
            air_speed: 7.0,
            heading: 180.0,
            altitude_relative: 45.0,
        }
    }

    #[derive(Default)]
    struct RecordingMissionControlActionClient {
        requests: Mutex<Vec<MissionControlActionRequest>>,
    }

    impl RecordingMissionControlActionClient {
        fn request_count(&self) -> usize {
            self.requests
                .lock()
                .expect("request lock should not be poisoned")
                .len()
        }
    }

    impl MissionControlActionClient for RecordingMissionControlActionClient {
        fn submit_operator_action(
            &self,
            request: MissionControlActionRequest,
        ) -> Result<MissionControlActionAck, OperatorActionError> {
            self.requests
                .lock()
                .expect("request lock should not be poisoned")
                .push(request.clone());
            Ok(MissionControlActionAck::accepted(
                request.action,
                request.target_mission_id,
                "accepted",
                chrono::Utc::now(),
            ))
        }
    }
}
