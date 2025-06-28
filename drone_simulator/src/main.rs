use clap::{Parser, Subcommand};
use anyhow::Result;
use tracing::{info, error};
use tokio::time::{interval, Duration};
use std::sync::Arc;

use drone_simulator::{
    SimulationEngine, Drone, DroneCapabilities, 
    flight_controller::FlightCommand,
    environment::{Environment, EnvironmentConditions},
};

#[derive(Parser)]
#[command(name = "drone_simulator")]
#[command(about = "Agricultural drone simulation system")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a full simulation with multiple drones
    Simulate {
        #[arg(short, long, default_value = "1")]
        drones: u32,
        #[arg(short, long, default_value = "60")]
        duration_seconds: u64,
        #[arg(long, default_value = "1.0")]
        time_scale: f32,
    },
    /// Test a single drone
    TestDrone {
        #[arg(short, long, default_value = "TestDrone")]
        name: String,
    },
    /// Run interactive drone control
    Interactive,
    /// Generate sample flight data
    GenerateData {
        #[arg(short, long)]
        output: String,
        #[arg(short, long, default_value = "100")]
        waypoints: u32,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();

    match cli.command {
        Commands::Simulate { drones, duration_seconds, time_scale } => {
            run_simulation(drones, duration_seconds, time_scale).await?;
        }
        Commands::TestDrone { name } => {
            test_single_drone(name).await?;
        }
        Commands::Interactive => {
            run_interactive_mode().await?;
        }
        Commands::GenerateData { output, waypoints } => {
            generate_sample_data(output, waypoints).await?;
        }
    }

    Ok(())
}

async fn run_simulation(num_drones: u32, duration_seconds: u64, time_scale: f32) -> Result<()> {
    info!("Starting simulation with {} drones for {} seconds ({}x speed)", 
          num_drones, duration_seconds, time_scale);

    let engine = Arc::new(SimulationEngine::new());
    
    // Set up environment with some weather conditions
    let conditions = EnvironmentConditions {
        temperature_celsius: 22.0,
        wind_speed_ms: 8.0,
        visibility_m: 12000.0,
        ..Default::default()
    };
    
    // Create drones
    let mut drone_ids = Vec::new();
    for i in 0..num_drones {
        let capabilities = DroneCapabilities {
            max_speed_ms: 15.0,
            max_altitude_m: 120.0,
            flight_time_minutes: 30,
            has_camera: true,
            has_lidar: i % 2 == 0, // Every other drone has LiDAR
            has_multispectral: i % 3 == 0, // Every third drone has multispectral
            ..Default::default()
        };

        let drone = Drone::new(
            format!("AgriDrone-{:03}", i + 1),
            "Quadcopter-X4".to_string(),
        )
        .with_position(i as f32 * 10.0, 0.0, i as f32 * 10.0)
        .with_capabilities(capabilities);

        let id = engine.add_drone(drone).await?;
        drone_ids.push(id);
        info!("Added drone: {}", id);
    }

    // Set time scale
    engine.set_time_scale(time_scale).await;

    // Start simulation
    engine.start_simulation().await?;
    info!("Simulation started");

    // Send some commands to drones
    tokio::spawn({
        let engine = engine.clone();
        let drone_ids = drone_ids.clone();
        async move {
            // Wait a bit then send takeoff commands
            tokio::time::sleep(Duration::from_secs(2)).await;
            
            for (i, &id) in drone_ids.iter().enumerate() {
                let altitude = 50.0 + (i as f32 * 10.0);
                if let Err(e) = engine.send_command(&id, FlightCommand::Takeoff { altitude_m: altitude }).await {
                    error!("Failed to send takeoff command to drone {}: {}", id, e);
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }

            // After takeoff, send some movement commands
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            for (i, &id) in drone_ids.iter().enumerate() {
                let x = (i as f32 * 20.0) + 100.0;
                let z = (i as f32 * 15.0) + 50.0;
                if let Err(e) = engine.send_command(&id, FlightCommand::GoTo { 
                    x, y: 100.0, z, speed_ms: 8.0 
                }).await {
                    error!("Failed to send goto command to drone {}: {}", id, e);
                }
            }
        }
    });

    // Monitor simulation
    let mut status_interval = interval(Duration::from_secs(5));
    let mut elapsed = 0;

    while elapsed < duration_seconds {
        status_interval.tick().await;
        elapsed += 5;

        let drones = engine.list_drones().await;
        info!("Simulation status ({}s elapsed):", elapsed);
        for drone in drones {
            info!("  {}: Position({:.1}, {:.1}, {:.1}) Battery({:.1}%) Status({:?})",
                  drone.name,
                  drone.position.x, drone.position.y, drone.position.z,
                  drone.battery_level * 100.0,
                  drone.status);
        }
        
        let sim_time = engine.get_simulation_time().await;
        info!("  Simulation time: {}", sim_time.format("%H:%M:%S"));
    }

    // Stop simulation
    engine.stop_simulation().await;
    info!("Simulation completed");

    Ok(())
}

async fn test_single_drone(name: String) -> Result<()> {
    info!("Testing single drone: {}", name);

    let engine = SimulationEngine::new();
    let drone = Drone::new(name.clone(), "TestQuad".to_string())
        .with_position(0.0, 0.0, 0.0);

    let id = engine.add_drone(drone).await?;
    engine.start_simulation().await?;

    // Test sequence
    info!("Starting test sequence for {}", name);
    
    // Takeoff
    engine.send_command(&id, FlightCommand::Takeoff { altitude_m: 50.0 }).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Move around
    engine.send_command(&id, FlightCommand::GoTo { x: 100.0, y: 50.0, z: 0.0, speed_ms: 10.0 }).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;

    engine.send_command(&id, FlightCommand::GoTo { x: 100.0, y: 50.0, z: 100.0, speed_ms: 10.0 }).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;

    engine.send_command(&id, FlightCommand::GoTo { x: 0.0, y: 50.0, z: 100.0, speed_ms: 10.0 }).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Return home and land
    engine.send_command(&id, FlightCommand::GoTo { x: 0.0, y: 50.0, z: 0.0, speed_ms: 10.0 }).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;

    engine.send_command(&id, FlightCommand::Land).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Get final status
    if let Some(drone) = engine.get_drone(&id).await {
        info!("Test completed. Final status:");
        info!("  Position: ({:.1}, {:.1}, {:.1})", drone.position.x, drone.position.y, drone.position.z);
        info!("  Battery: {:.1}%", drone.battery_level * 100.0);
        info!("  Status: {:?}", drone.status);
    }

    engine.stop_simulation().await;
    Ok(())
}

async fn run_interactive_mode() -> Result<()> {
    info!("Starting interactive drone control mode");
    info!("Commands: takeoff <altitude>, goto <x> <y> <z>, land, hover, status, quit");

    let engine = SimulationEngine::new();
    let drone = Drone::new("InteractiveDrone".to_string(), "UserControlled".to_string());
    let id = engine.add_drone(drone).await?;
    
    engine.start_simulation().await?;

    // Simple command loop (in a real implementation, you'd use a proper CLI library)
    loop {
        println!("Enter command: ");
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() {
            break;
        }

        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0].to_lowercase().as_str() {
            "takeoff" => {
                let altitude = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(50.0);
                engine.send_command(&id, FlightCommand::Takeoff { altitude_m: altitude }).await?;
                println!("Taking off to {}m", altitude);
            }
            "goto" => {
                if parts.len() >= 4 {
                    if let (Ok(x), Ok(y), Ok(z)) = (parts[1].parse(), parts[2].parse(), parts[3].parse()) {
                        engine.send_command(&id, FlightCommand::GoTo { x, y, z, speed_ms: 10.0 }).await?;
                        println!("Going to ({}, {}, {})", x, y, z);
                    }
                }
            }
            "land" => {
                engine.send_command(&id, FlightCommand::Land).await?;
                println!("Landing");
            }
            "hover" => {
                let duration = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(10.0);
                engine.send_command(&id, FlightCommand::Hover { duration_seconds: duration }).await?;
                println!("Hovering for {}s", duration);
            }
            "status" => {
                if let Some(drone) = engine.get_drone(&id).await {
                    println!("Drone Status:");
                    println!("  Position: ({:.1}, {:.1}, {:.1})", drone.position.x, drone.position.y, drone.position.z);
                    println!("  Velocity: ({:.1}, {:.1}, {:.1})", drone.velocity.x, drone.velocity.y, drone.velocity.z);
                    println!("  Battery: {:.1}%", drone.battery_level * 100.0);
                    println!("  Status: {:?}", drone.status);
                }
            }
            "quit" | "exit" => {
                break;
            }
            _ => {
                println!("Unknown command. Available: takeoff, goto, land, hover, status, quit");
            }
        }
    }

    engine.stop_simulation().await;
    println!("Interactive mode ended");
    Ok(())
}

async fn generate_sample_data(output: String, _waypoints: u32) -> Result<()> {
    info!("Generating sample flight data to: {}", output);

    // Create sample flight data
    let sample_data = serde_json::json!({
        "mission_id": uuid::Uuid::new_v4(),
        "drone_id": uuid::Uuid::new_v4(),
        "timestamp": chrono::Utc::now(),
        "flight_path": [
            {"x": 0.0, "y": 0.0, "z": 0.0, "timestamp": "2024-01-01T12:00:00Z"},
            {"x": 100.0, "y": 50.0, "z": 0.0, "timestamp": "2024-01-01T12:01:00Z"},
            {"x": 100.0, "y": 50.0, "z": 100.0, "timestamp": "2024-01-01T12:02:00Z"},
            {"x": 0.0, "y": 50.0, "z": 100.0, "timestamp": "2024-01-01T12:03:00Z"},
            {"x": 0.0, "y": 0.0, "z": 0.0, "timestamp": "2024-01-01T12:04:00Z"}
        ],
        "sensor_data": {
            "camera_captures": 25,
            "lidar_scans": 12,
            "gps_fixes": 240,
            "battery_readings": 240
        },
        "summary": {
            "total_distance_m": 282.84,
            "flight_time_minutes": 4.5,
            "max_altitude_m": 50.0,
            "battery_used_percent": 15.2
        }
    });

    let json_string = serde_json::to_string_pretty(&sample_data)?;
    tokio::fs::write(&output, json_string).await?;

    info!("Sample data generated successfully");
    Ok(())
}
