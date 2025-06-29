use anyhow::Result;
use clap::{Parser, Subcommand};
use geo::polygon;
use serde_json;
use uuid::Uuid;

use mission_planner::{Mission, MissionPlannerService, Waypoint, WaypointType};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Database URL
    #[arg(short, long, default_value = "postgres://postgres:password@localhost:5432/agbot")]
    database_url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new mission
    Create {
        /// Mission name
        #[arg(short, long)]
        name: String,
        /// Mission description
        #[arg(short, long)]
        description: String,
        /// Area coordinates as JSON polygon
        #[arg(short, long)]
        area: Option<String>,
    },
    /// List all missions
    List {
        /// Maximum number of missions to return
        #[arg(short, long, default_value = "10")]
        limit: i64,
        /// Number of missions to skip
        #[arg(short, long, default_value = "0")]
        offset: i64,
    },
    /// Get a specific mission
    Get {
        /// Mission ID
        id: String,
    },
    /// Update a mission
    Update {
        /// Mission ID
        id: String,
        /// New mission name
        #[arg(short, long)]
        name: Option<String>,
        /// New mission description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Delete a mission
    Delete {
        /// Mission ID
        id: String,
    },
    /// Search missions
    Search {
        /// Search query
        query: String,
    },
    /// Get mission statistics
    Stats,
    /// Add waypoint to a mission
    AddWaypoint {
        /// Mission ID
        mission_id: String,
        /// Latitude
        #[arg(short, long)]
        lat: f64,
        /// Longitude
        #[arg(short, long)]
        lon: f64,
        /// Altitude in meters
        #[arg(short, long, default_value = "100.0")]
        altitude: f32,
        /// Waypoint type
        #[arg(short, long, default_value = "navigation")]
        waypoint_type: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize the mission planner service
    let service = MissionPlannerService::new(&cli.database_url).await?;

    match cli.command {
        Commands::Create { name, description, area } => {
            let area_polygon = if let Some(area_json) = area {
                serde_json::from_str(&area_json)?
            } else {
                // Default 1km x 1km area at origin
                polygon![
                    (x: 0.0, y: 0.0),
                    (x: 0.01, y: 0.0),
                    (x: 0.01, y: 0.01),
                    (x: 0.0, y: 0.01),
                    (x: 0.0, y: 0.0),
                ]
            };

            let mission = Mission::new(name, description, area_polygon);
            let id = service.create_mission(mission).await?;
            println!("Created mission with ID: {}", id);
        }

        Commands::List { limit, offset } => {
            let missions = service.list_missions(Some(limit), Some(offset)).await?;
            println!("Found {} missions:", missions.len());
            for mission in missions {
                println!("  {} - {} ({})", mission.id, mission.name, mission.created_at.format("%Y-%m-%d %H:%M"));
            }
        }

        Commands::Get { id } => {
            let mission_id = Uuid::parse_str(&id)?;
            if let Some(mission) = service.get_mission(&mission_id).await? {
                println!("Mission: {}", serde_json::to_string_pretty(&mission)?);
            } else {
                println!("Mission not found");
            }
        }

        Commands::Update { id, name, description } => {
            let mission_id = Uuid::parse_str(&id)?;
            if let Some(mut mission) = service.get_mission(&mission_id).await? {
                if let Some(new_name) = name {
                    mission.name = new_name;
                }
                if let Some(new_description) = description {
                    mission.description = new_description;
                }
                mission.updated_at = chrono::Utc::now();
                
                service.update_mission(mission).await?;
                println!("Mission updated successfully");
            } else {
                println!("Mission not found");
            }
        }

        Commands::Delete { id } => {
            let mission_id = Uuid::parse_str(&id)?;
            service.delete_mission(&mission_id).await?;
            println!("Mission deleted successfully");
        }

        Commands::Search { query } => {
            let missions = service.search_missions(&query).await?;
            println!("Found {} missions matching '{}':", missions.len(), query);
            for mission in missions {
                println!("  {} - {} ({})", mission.id, mission.name, mission.created_at.format("%Y-%m-%d %H:%M"));
            }
        }

        Commands::Stats => {
            let stats = service.get_mission_stats().await?;
            println!("Mission Statistics:");
            println!("  Total missions: {}", stats.total_missions);
            println!("  Average duration: {:.1} minutes", stats.average_duration_minutes);
            println!("  Average battery usage: {:.1}%", stats.average_battery_usage);
            if let Some(oldest) = stats.oldest_mission {
                println!("  Oldest mission: {}", oldest.format("%Y-%m-%d %H:%M"));
            }
            if let Some(newest) = stats.newest_mission {
                println!("  Newest mission: {}", newest.format("%Y-%m-%d %H:%M"));
            }
        }

        Commands::AddWaypoint { mission_id, lat, lon, altitude, waypoint_type } => {
            let mission_id = Uuid::parse_str(&mission_id)?;
            if let Some(mut mission) = service.get_mission(&mission_id).await? {
                let waypoint_type = match waypoint_type.to_lowercase().as_str() {
                    "takeoff" => WaypointType::Takeoff,
                    "landing" => WaypointType::Landing,
                    "survey" => WaypointType::Survey,
                    "datacollection" => WaypointType::DataCollection,
                    "emergency" => WaypointType::Emergency,
                    "hover" => WaypointType::Hover,
                    _ => WaypointType::Navigation,
                };

                let waypoint = Waypoint {
                    id: Uuid::new_v4(),
                    position: geo::Point::new(lon, lat),
                    altitude_m: altitude,
                    waypoint_type,
                    actions: Vec::new(),
                    arrival_time: None,
                    speed_ms: None,
                    heading_degrees: None,
                };

                mission.add_waypoint(waypoint);
                service.update_mission(mission).await?;
                println!("Waypoint added successfully");
            } else {
                println!("Mission not found");
            }
        }
    }

    Ok(())
}
