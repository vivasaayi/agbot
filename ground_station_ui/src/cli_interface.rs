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
                    println!("System Status:");
                    println!("  WebSocket: {}", snapshot.state);
                    println!("  Reconnect attempts: {}", snapshot.reconnect_attempts);
                    println!("  Next retry: {} ms", snapshot.next_backoff.as_millis());
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
