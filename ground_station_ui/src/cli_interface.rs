use tracing::info;
use tokio::io::{self, AsyncBufReadExt, BufReader};

pub async fn run_cli_interface() {
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
                    println!("System Status:");
                    println!("  WebSocket: Connected");
                    println!("  Telemetry: Receiving");
                    println!("  Last update: {}", chrono::Utc::now().format("%H:%M:%S"));
                }
                "quit" | "exit" => {
                    println!("Goodbye!");
                    break;
                }
                "" => {
                    // Empty command, do nothing
                }
                _ => {
                    println!("Unknown command: {}. Type 'help' for available commands.", command);
                }
            }
        }
    }
}
