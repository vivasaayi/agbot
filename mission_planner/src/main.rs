use clap::{Parser, Subcommand};
use anyhow::Result;
use tracing::{info, error};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put, delete},
    Router,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use serde_json::json;

use mission_planner::{
    Mission, MissionPlannerService, Waypoint, WaypointType, 
    weather_integration::{WeatherIntegration, FlightConditionResult},
    mission_optimizer::MissionOptimizer,
};

#[derive(Parser)]
#[command(name = "mission_planner")]
#[command(about = "Agricultural drone mission planning system")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the mission planning web server
    Serve {
        #[arg(short, long, default_value = "3000")]
        port: u16,
        #[arg(short, long, default_value = "0.0.0.0")]
        host: String,
    },
    /// Plan a mission from a GeoJSON file
    Plan {
        #[arg(short, long)]
        input: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Validate weather conditions for a location
    Weather {
        #[arg(long)]
        lat: f64,
        #[arg(long)]
        lon: f64,
    },
}

type AppState = Arc<Mutex<MissionPlannerService>>;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { port, host } => {
            start_server(host, port).await?;
        }
        Commands::Plan { input, output } => {
            plan_mission_from_file(input, output).await?;
        }
        Commands::Weather { lat, lon } => {
            check_weather(lat, lon).await?;
        }
    }

    Ok(())
}

async fn start_server(host: String, port: u16) -> Result<()> {
    info!("Starting mission planner server on {}:{}", host, port);

    let service = MissionPlannerService::new();
    let state = Arc::new(Mutex::new(service));

    let app = Router::new()
        .route("/api/missions", get(list_missions))
        .route("/api/missions", post(create_mission))
        .route("/api/missions/:id", get(get_mission))
        .route("/api/missions/:id", put(update_mission))
        .route("/api/missions/:id", delete(delete_mission))
        .route("/api/missions/:id/optimize", post(optimize_mission))
        .route("/api/weather", get(get_weather))
        .route("/health", get(health_check))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port)).await?;
    info!("Mission planner server listening on {}", listener.local_addr()?);
    
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({ "status": "healthy" }))
}

async fn list_missions(State(state): State<AppState>) -> Result<Json<Vec<Mission>>, StatusCode> {
    let service = state.lock().await;
    let missions = service.list_missions().await;
    Ok(Json(missions.into_iter().cloned().collect()))
}

async fn create_mission(
    State(state): State<AppState>,
    Json(mission): Json<Mission>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut service = state.lock().await;
    match service.create_mission(mission).await {
        Ok(id) => Ok(Json(json!({ "id": id }))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_mission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Mission>, StatusCode> {
    let service = state.lock().await;
    match service.get_mission(&id).await {
        Some(mission) => Ok(Json(mission.clone())),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn update_mission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(mission): Json<Mission>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut service = state.lock().await;
    match service.update_mission(&id, mission).await {
        Ok(_) => Ok(Json(json!({ "success": true }))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn delete_mission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut service = state.lock().await;
    match service.delete_mission(&id).await {
        Ok(_) => Ok(Json(json!({ "success": true }))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn optimize_mission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Mission>, StatusCode> {
    let mut service = state.lock().await;
    
    if let Some(mission) = service.get_mission(&id).await {
        let optimizer = MissionOptimizer::new();
        match optimizer.optimize_mission(mission) {
            Ok(optimized) => {
                let _ = service.update_mission(&id, optimized.clone()).await;
                Ok(Json(optimized))
            }
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn get_weather() -> Result<Json<serde_json::Value>, StatusCode> {
    let weather_integration = WeatherIntegration::new(None);
    
    // Mock coordinates for demonstration
    match weather_integration.get_current_weather(40.7128, -74.0060).await {
        Ok(weather) => Ok(Json(serde_json::to_value(weather).unwrap())),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn plan_mission_from_file(input: String, output: Option<String>) -> Result<()> {
    info!("Planning mission from file: {}", input);
    
    // Read GeoJSON file (simplified for demo)
    let geojson_content = tokio::fs::read_to_string(&input).await?;
    info!("Read {} bytes from {}", geojson_content.len(), input);
    
    // In a real implementation, parse GeoJSON and create waypoints
    // For now, create a simple demo mission
    let area = geo::polygon![
        (x: -74.0060, y: 40.7128),
        (x: -74.0050, y: 40.7128),
        (x: -74.0050, y: 40.7138),
        (x: -74.0060, y: 40.7138),
        (x: -74.0060, y: 40.7128),
    ];
    
    let mut mission = Mission::new(
        "Demo Mission".to_string(),
        "Mission created from GeoJSON".to_string(),
        area,
    );
    
    // Add some waypoints
    mission.add_waypoint(Waypoint::new(
        geo::point!(x: -74.0060, y: 40.7128),
        100.0,
        WaypointType::Takeoff,
    ));
    
    mission.add_waypoint(Waypoint::new(
        geo::point!(x: -74.0055, y: 40.7133),
        150.0,
        WaypointType::DataCollection,
    ));
    
    mission.add_waypoint(Waypoint::new(
        geo::point!(x: -74.0050, y: 40.7138),
        100.0,
        WaypointType::Landing,
    ));
    
    // Optimize the mission
    let optimizer = MissionOptimizer::new();
    let optimized = optimizer.optimize_mission(&mission)?;
    
    // Output the result
    let output_file = output.unwrap_or_else(|| "mission_plan.json".to_string());
    let mission_json = serde_json::to_string_pretty(&optimized)?;
    tokio::fs::write(&output_file, mission_json).await?;
    
    info!("Mission plan saved to: {}", output_file);
    info!("Estimated duration: {} minutes", optimized.estimated_duration_minutes);
    info!("Estimated battery usage: {:.1}%", optimized.estimated_battery_usage * 100.0);
    
    Ok(())
}

async fn check_weather(lat: f64, lon: f64) -> Result<()> {
    info!("Checking weather conditions for lat: {}, lon: {}", lat, lon);
    
    let weather_integration = WeatherIntegration::new(None);
    let weather = weather_integration.get_current_weather(lat, lon).await?;
    
    println!("Current Weather Conditions:");
    println!("  Temperature: {:.1}°C", weather.temperature_celsius);
    println!("  Wind Speed: {:.1} m/s", weather.wind_speed_ms);
    println!("  Wind Direction: {:.0}°", weather.wind_direction_degrees);
    println!("  Precipitation: {:.1} mm", weather.precipitation_mm);
    println!("  Visibility: {:.0} m", weather.visibility_m);
    println!("  Humidity: {:.0}%", weather.humidity_percent);
    
    // Check flight conditions
    let constraints = mission_planner::WeatherConstraints::default();
    let result = weather_integration.check_flight_conditions(&weather, &constraints);
    
    println!("\nFlight Conditions:");
    println!("  Flight Safe: {}", if result.flight_safe { "YES" } else { "NO" });
    println!("  Weather Score: {:.1}/100", result.weather_score);
    
    if !result.issues.is_empty() {
        println!("  Issues:");
        for issue in &result.issues {
            println!("    - {}", issue);
        }
    }
    
    if !result.warnings.is_empty() {
        println!("  Warnings:");
        for warning in &result.warnings {
            println!("    - {}", warning);
        }
    }
    
    Ok(())
}
