use anyhow::Result;
use clap::{Arg, Command};
use sensor_overlay_engine::{
    CompositeOverlayEngine, 
    NdviProcessor, ThermalProcessor, LidarOverlayProcessor,
    ndvi::{NdviConfig, FieldScanData, ColorMapping},
    thermal::{ThermalConfig, ThermalScanData, TemperatureRange, ThermalColorPalette, ThermalCalibration},
    lidar_overlay::{LidarConfig, PointCloudData, HeightColorMapping},
    composite::{CompositeConfig, CompositeScanData},
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
        .get_matches();

    match matches.subcommand() {
        Some(("process", sub_matches)) => {
            let input_dir = PathBuf::from(sub_matches.get_one::<String>("input-dir").unwrap());
            let output_dir = PathBuf::from(sub_matches.get_one::<String>("output-dir").unwrap());
            let overlay_types = sub_matches.get_one::<String>("overlay-types").unwrap();
            let config_file = sub_matches.get_one::<String>("config");

            process_sensor_data(input_dir, output_dir, overlay_types, config_file).await?;
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

    // Initialize processors with concrete configurations
    let ndvi_config = NdviConfig {
        red_band_index: 0,
        nir_band_index: 1,
        output_format: "PNG".to_string(),
        color_mapping: ColorMapping {
            low_vegetation: [255, 0, 0],
            medium_vegetation: [255, 255, 0],
            high_vegetation: [0, 255, 0],
            water: [0, 0, 255],
            soil: [139, 69, 19],
        },
    };

    let thermal_config = ThermalConfig {
        temperature_range: TemperatureRange {
            min_celsius: -10.0,
            max_celsius: 50.0,
        },
        color_palette: ThermalColorPalette {
            cold: [0, 0, 255],
            cool: [0, 255, 255],
            moderate: [0, 255, 0],
            warm: [255, 255, 0],
            hot: [255, 0, 0],
        },
        calibration: ThermalCalibration {
            offset: 0.0,
            scale: 1.0,
            ambient_temp: 20.0,
        },
    };

    let lidar_config = LidarConfig {
        point_cloud_resolution: 0.1,
        height_color_mapping: HeightColorMapping {
            ground_level: [139, 69, 19, 255],
            low_vegetation: [50, 205, 50, 255],
            medium_vegetation: [34, 139, 34, 255],
            high_vegetation: [0, 100, 0, 255],
            obstacles: [255, 0, 0, 255],
        },
        occupancy_grid_resolution: 0.2,
        max_range: 100.0,
    };

    let ndvi_processor = NdviProcessor::new(ndvi_config);
    let thermal_processor = ThermalProcessor::new(thermal_config);
    let lidar_processor = LidarOverlayProcessor::new(lidar_config);

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
