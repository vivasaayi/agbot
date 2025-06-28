use anyhow::Result;
use clap::{Arg, Command};
use sensor_overlay_engine::{
    CompositeOverlayEngine, CompositeConfig, CompositeScanData,
    NdviProcessor, ThermalProcessor, LidarOverlayProcessor,
    ndvi::{NdviConfig, FieldScanData},
    thermal::{ThermalConfig, ThermalScanData},
    lidar_overlay::{LidarConfig, PointCloudData},
};
use std::path::PathBuf;
use tokio;
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let matches = Command::new("sensor-overlay-engine")
        .version("1.0.0")
        .about("Agricultural sensor overlay processing engine")
        .subcommand(
            Command::new("process")
                .about("Process sensor data and generate overlays")
                .arg(
                    Arg::new("input-dir")
                        .long("input-dir")
                        .short('i')
                        .value_name("DIR")
                        .help("Input directory containing sensor data")
                        .required(true)
                )
                .arg(
                    Arg::new("output-dir")
                        .long("output-dir")
                        .short('o')
                        .value_name("DIR")
                        .help("Output directory for processed overlays")
                        .required(true)
                )
                .arg(
                    Arg::new("overlay-types")
                        .long("overlay-types")
                        .short('t')
                        .value_name("TYPES")
                        .help("Comma-separated list of overlay types (ndvi,thermal,lidar)")
                        .default_value("ndvi,thermal,lidar")
                )
                .arg(
                    Arg::new("config")
                        .long("config")
                        .short('c')
                        .value_name("FILE")
                        .help("Configuration file path")
                )
        )
        .subcommand(
            Command::new("server")
                .about("Start overlay processing server")
                .arg(
                    Arg::new("port")
                        .long("port")
                        .short('p')
                        .value_name("PORT")
                        .help("Server port")
                        .default_value("3003")
                )
                .arg(
                    Arg::new("host")
                        .long("host")
                        .value_name("HOST")
                        .help("Server host")
                        .default_value("0.0.0.0")
                )
        )
        .get_matches();

    match matches.subcommand() {
        Some(("process", sub_matches)) => {
            let input_dir = PathBuf::from(sub_matches.get_one::<String>("input-dir").unwrap());
            let output_dir = PathBuf::from(sub_matches.get_one::<String>("output-dir").unwrap());
            let overlay_types = sub_matches.get_one::<String>("overlay-types").unwrap();
            let config_file = sub_matches.get_one::<String>("config");

            process_sensor_data(input_dir, output_dir, overlay_types, config_file).await?;
        }
        Some(("server", sub_matches)) => {
            let port = sub_matches.get_one::<String>("port").unwrap().parse::<u16>()?;
            let host = sub_matches.get_one::<String>("host").unwrap();

            start_overlay_server(host, port).await?;
        }
        _ => {
            eprintln!("No subcommand provided. Use --help for usage information.");
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn process_sensor_data(
    input_dir: PathBuf,
    output_dir: PathBuf,
    overlay_types: &str,
    config_file: Option<&String>,
) -> Result<()> {
    info!("Starting sensor overlay processing");
    info!("Input directory: {:?}", input_dir);
    info!("Output directory: {:?}", output_dir);

    // Create output directory if it doesn't exist
    tokio::fs::create_dir_all(&output_dir).await?;

    // Load configuration
    let config = load_config(config_file).await?;
    
    // Parse overlay types
    let requested_types: Vec<&str> = overlay_types.split(',').collect();
    info!("Requested overlay types: {:?}", requested_types);

    // Initialize processors
    let ndvi_processor = NdviProcessor::new(NdviConfig::default());
    let thermal_processor = ThermalProcessor::new(ThermalConfig::default());
    let lidar_processor = LidarOverlayProcessor::new(LidarConfig::default());

    let composite_engine = CompositeOverlayEngine::new(
        config,
        ndvi_processor,
        thermal_processor,
        lidar_processor,
    );

    // Scan for sensor data files
    let scan_data = load_sensor_data(&input_dir).await?;
    
    if scan_data.is_empty() {
        warn!("No sensor data found in input directory");
        return Ok(());
    }

    info!("Found {} sensor data files", scan_data.len());

    // Process each scan
    for (index, data) in scan_data.iter().enumerate() {
        let scan_output_dir = output_dir.join(format!("scan_{:03}", index));
        tokio::fs::create_dir_all(&scan_output_dir).await?;

        match composite_engine.process_field_scan(data, &scan_output_dir).await {
            Ok(result) => {
                info!("Successfully processed scan {}: {:?}", index, result.composite_image_path);
                
                // Print analysis results
                print_analysis_results(&result.analysis);
            }
            Err(e) => {
                error!("Failed to process scan {}: {}", index, e);
            }
        }
    }

    info!("Sensor overlay processing completed");
    Ok(())
}

async fn start_overlay_server(host: &str, port: u16) -> Result<()> {
    info!("Starting sensor overlay server on {}:{}", host, port);

    // Create HTTP server using warp or axum
    use warp::Filter;

    let health = warp::path("health")
        .and(warp::get())
        .map(|| "OK");

    let process_route = warp::path("process")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(handle_process_request);

    let routes = health
        .or(process_route)
        .with(warp::cors().allow_any_origin());

    info!("Server listening on http://{}:{}", host, port);
    warp::serve(routes)
        .run((host.parse::<std::net::IpAddr>()?, port))
        .await;

    Ok(())
}

async fn handle_process_request(
    request: ProcessRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    info!("Received process request: {:?}", request);
    
    // Convert request to scan data (simplified)
    let scan_data = CompositeScanData {
        ndvi_data: None, // Would load from request data
        thermal_data: None,
        lidar_data: None,
        rgb_image: None,
        gps_reference: nalgebra::Point3::new(0.0, 0.0, 0.0),
        timestamp: chrono::Utc::now(),
    };

    // Initialize processors
    let config = CompositeConfig::default();
    let ndvi_processor = NdviProcessor::new(NdviConfig::default());
    let thermal_processor = ThermalProcessor::new(ThermalConfig::default());
    let lidar_processor = LidarOverlayProcessor::new(LidarConfig::default());

    let composite_engine = CompositeOverlayEngine::new(
        config,
        ndvi_processor,
        thermal_processor,
        lidar_processor,
    );

    // Create temporary output directory
    let output_dir = std::env::temp_dir().join(format!("overlay_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&output_dir).await.map_err(|_| warp::reject::reject())?;

    // Process the scan
    match composite_engine.process_field_scan(&scan_data, &output_dir).await {
        Ok(result) => {
            let response = ProcessResponse {
                success: true,
                message: "Processing completed successfully".to_string(),
                output_path: result.composite_image_path.to_string_lossy().to_string(),
                analysis: Some(result.analysis),
            };
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let response = ProcessResponse {
                success: false,
                message: format!("Processing failed: {}", e),
                output_path: String::new(),
                analysis: None,
            };
            Ok(warp::reply::json(&response))
        }
    }
}

async fn load_config(config_file: Option<&String>) -> Result<CompositeConfig> {
    if let Some(config_path) = config_file {
        info!("Loading configuration from: {}", config_path);
        let config_data = tokio::fs::read_to_string(config_path).await?;
        let config: CompositeConfig = serde_json::from_str(&config_data)?;
        Ok(config)
    } else {
        info!("Using default configuration");
        Ok(CompositeConfig::default())
    }
}

async fn load_sensor_data(input_dir: &PathBuf) -> Result<Vec<CompositeScanData>> {
    let mut scan_data = Vec::new();

    // This is a simplified implementation
    // In practice, you would scan for specific file patterns (e.g., .tiff, .las, .json)
    // and parse them into the appropriate data structures

    let mut dir_entries = tokio::fs::read_dir(input_dir).await?;
    
    while let Some(entry) = dir_entries.next_entry().await? {
        if entry.file_type().await?.is_file() {
            // Create mock scan data for demonstration
            // In practice, parse actual sensor files
            let mock_scan = CompositeScanData {
                ndvi_data: Some(create_mock_ndvi_data()),
                thermal_data: Some(create_mock_thermal_data()),
                lidar_data: Some(create_mock_lidar_data()),
                rgb_image: None,
                gps_reference: nalgebra::Point3::new(0.0, 0.0, 0.0),
                timestamp: chrono::Utc::now(),
            };
            scan_data.push(mock_scan);
        }
    }

    Ok(scan_data)
}

fn create_mock_ndvi_data() -> FieldScanData {
    FieldScanData {
        red_band: vec![0.1, 0.2, 0.15, 0.25],
        nir_band: vec![0.3, 0.4, 0.35, 0.45],
        width: 2,
        height: 2,
        gps_coordinates: vec![
            nalgebra::Point3::new(0.0, 0.0, 0.0),
            nalgebra::Point3::new(1.0, 0.0, 0.0),
            nalgebra::Point3::new(0.0, 1.0, 0.0),
            nalgebra::Point3::new(1.0, 1.0, 0.0),
        ],
        timestamp: chrono::Utc::now(),
    }
}

fn create_mock_thermal_data() -> ThermalScanData {
    ThermalScanData {
        raw_thermal_data: vec![1000, 1100, 1050, 1150],
        width: 2,
        height: 2,
        gps_coordinates: vec![
            nalgebra::Point3::new(0.0, 0.0, 0.0),
            nalgebra::Point3::new(1.0, 0.0, 0.0),
            nalgebra::Point3::new(0.0, 1.0, 0.0),
            nalgebra::Point3::new(1.0, 1.0, 0.0),
        ],
        timestamp: chrono::Utc::now(),
    }
}

fn create_mock_lidar_data() -> PointCloudData {
    PointCloudData {
        points: vec![
            nalgebra::Point3::new(0.0, 0.0, 1.0),
            nalgebra::Point3::new(1.0, 0.0, 1.1),
            nalgebra::Point3::new(0.0, 1.0, 0.9),
            nalgebra::Point3::new(1.0, 1.0, 1.2),
        ],
        intensities: vec![100.0, 110.0, 90.0, 120.0],
        gps_origin: nalgebra::Point3::new(0.0, 0.0, 0.0),
        timestamp: chrono::Utc::now(),
    }
}

fn print_analysis_results(analysis: &sensor_overlay_engine::composite::CompositeAnalysis) {
    println!("\n=== Analysis Results ===");
    
    if let Some(health_score) = analysis.vegetation_health_score {
        println!("Vegetation Health Score: {:.1}%", health_score);
    }
    
    if let Some(coverage) = analysis.vegetation_coverage {
        println!("Vegetation Coverage: {:.1}%", coverage);
    }
    
    if let Some(anomalies) = analysis.temperature_anomalies {
        println!("Temperature Anomalies: {}", anomalies);
    }
    
    if let Some(stress) = analysis.stress_indicators {
        println!("Stress Indicators: {:.1}%", stress);
    }
    
    if let Some(complexity) = analysis.terrain_complexity {
        println!("Terrain Complexity: {:.1}%", complexity);
    }
    
    if let Some(obstacles) = analysis.obstacle_count {
        println!("Obstacles Detected: {}", obstacles);
    }

    if !analysis.recommendations.is_empty() {
        println!("\nRecommendations:");
        for (i, rec) in analysis.recommendations.iter().enumerate() {
            println!("  {}. {}", i + 1, rec);
        }
    }
    println!("========================\n");
}

// Request/Response types for the HTTP API
#[derive(Debug, serde::Deserialize)]
struct ProcessRequest {
    data_type: String,
    data_path: String,
    overlay_types: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
struct ProcessResponse {
    success: bool,
    message: String,
    output_path: String,
    analysis: Option<sensor_overlay_engine::composite::CompositeAnalysis>,
}
