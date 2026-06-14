use crate::{SharedLinkState, SharedMessageDispatchState};
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tracing::info;

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
                    let snapshot = link_state.read().await.snapshot();
                    let dispatch = dispatch_state.read().await.clone();
                    let freshness = dispatch.telemetry_freshness();
                    println!("System Status:");
                    println!("  WebSocket: {}", snapshot.state);
                    println!("  Reconnect attempts: {}", snapshot.reconnect_attempts);
                    println!("  Next retry: {} ms", snapshot.next_backoff.as_millis());
                    println!("  Telemetry: {}", freshness.state);
                    if let Some(age) = freshness.last_update_age_seconds {
                        println!("  Telemetry age: {} s", age);
                    }
                    if let Some(telemetry) = dispatch.telemetry_tile_snapshot() {
                        println!(
                            "  Position: {:.6}, {:.6} @ {:.1} m{}",
                            telemetry.latitude,
                            telemetry.longitude,
                            telemetry.altitude_m,
                            if telemetry.stale { " (stale)" } else { "" }
                        );
                        println!(
                            "  Battery: {}% ({:.1} V)",
                            telemetry.battery_percentage, telemetry.battery_voltage
                        );
                        println!("  Mode: {} (armed: {})", telemetry.mode, telemetry.armed);
                    }
                    println!("  Capture events: {}", dispatch.capture_events(None).len());
                    println!("  Malformed frames: {}", dispatch.malformed_frames);
                    if let Some(error) = snapshot.last_error {
                        println!("  Last error: {}", error);
                    }
                    println!("  Last state change: {}", snapshot.updated_at);
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
