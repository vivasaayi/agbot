use clap::Parser;
use ndvi_processor::{ComprehensiveAnalysisProcessor, analysis_schemas::*};
use shared::AgroResult;
use std::path::PathBuf;
use tracing::{info, warn, error};
use ndarray::Array2;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input directory containing satellite images
    #[arg(short, long)]
    input_dir: PathBuf,

    /// Output directory for analysis results
    #[arg(short, long)]
    output_dir: PathBuf,

    /// Analysis types to perform (comma-separated)
    /// Available: ndvi,evi,savi,ndwi,nbr,drought,water,vegetation,landcover,temporal
    #[arg(short, long, default_value = "ndvi,vegetation")]
    analysis_types: String,

    /// Run all available analyses
    #[arg(long)]
    all_analyses: bool,

    /// Enable demonstration mode with synthetic data
    #[arg(long)]
    demo: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> AgroResult<()> {
    let args = Args::parse();

    // Initialize logging
    if args.verbose {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();
    }

    info!("🛰️ Starting Comprehensive Satellite Image Analysis System");
    info!("Input directory: {:?}", args.input_dir);
    info!("Output directory: {:?}", args.output_dir);

    // Create output directory
    tokio::fs::create_dir_all(&args.output_dir).await?;

    // Initialize the comprehensive analysis processor
    let processor = ComprehensiveAnalysisProcessor::new().await?;

    if args.demo {
        info!("🧪 Running demonstration mode with synthetic data");
        run_demonstration(&processor, &args.output_dir).await?;
    } else {
        // Parse analysis types
        let analysis_types = parse_analysis_types(&args.analysis_types, args.all_analyses);
        info!("📊 Analysis types: {:?}", analysis_types);

        // Process real satellite imagery
        process_satellite_imagery(&processor, &args.input_dir, &args.output_dir, analysis_types).await?;
    }

    info!("✅ Analysis completed successfully!");
    Ok(())
}

/// Run demonstration with synthetic data
async fn run_demonstration(
    processor: &ComprehensiveAnalysisProcessor,
    output_dir: &PathBuf,
) -> AgroResult<()> {
    info!("Generating synthetic satellite imagery data...");

    // Create synthetic multispectral bands (100x100 pixels)
    let rows = 100;
    let cols = 100;
    let mut bands = HashMap::new();

    // Generate realistic synthetic data
    bands.insert("blue".to_string(), generate_synthetic_band(rows, cols, 0.05, 0.15));
    bands.insert("green".to_string(), generate_synthetic_band(rows, cols, 0.03, 0.12));
    bands.insert("red".to_string(), generate_synthetic_band(rows, cols, 0.02, 0.10));
    bands.insert("nir".to_string(), generate_synthetic_band(rows, cols, 0.35, 0.85));
    bands.insert("swir1".to_string(), generate_synthetic_band(rows, cols, 0.15, 0.35));
    bands.insert("swir2".to_string(), generate_synthetic_band(rows, cols, 0.05, 0.25));

    let source_images = vec![Uuid::new_v4()];

    info!("🌱 Running comprehensive vegetation analysis...");
    let vegetation_result = processor.vegetation_analyzer.analyze_vegetation(
        &bands,
        source_images.clone(),
        output_dir.join("demo_vegetation").to_string_lossy().to_string(),
    )?;
    
    info!("Vegetation Analysis Results:");
    info!("  Overall Health: {:?}", vegetation_result.health_classification.overall_health);
    info!("  Biomass Estimate: {:.2} tons", vegetation_result.biomass_estimate.total_biomass_tons);
    info!("  Growth Stage: {:?}", vegetation_result.phenology.growth_stage);

    info!("💧 Running water body analysis...");
    let water_result = processor.water_analyzer.analyze_water(
        &bands,
        source_images.clone(),
        output_dir.join("demo_water").to_string_lossy().to_string(),
        None,
    )?;

    info!("Water Analysis Results:");
    info!("  Total Water Area: {:.2} hectares", water_result.total_water_area_hectares);
    info!("  Water Bodies Count: {}", water_result.water_bodies.len());
    info!("  Water Quality: {:?}", water_result.water_quality.overall_quality);

    info!("🔥 Running burn analysis...");
    let burn_result = processor.burn_analyzer.analyze_burn(
        &bands,
        None,
        source_images.clone(),
        output_dir.join("demo_burn").to_string_lossy().to_string(),
    )?;

    info!("Burn Analysis Results:");
    info!("  Burn Severity: {:?}", burn_result.burn_severity);
    info!("  Burned Area: {:.2} hectares", burn_result.burned_area_hectares);
    info!("  Recovery Stage: {:?}", burn_result.recovery_stage);

    info!("🌵 Running drought analysis...");
    let drought_result = processor.drought_analyzer.analyze_drought(
        &bands,
        None, // No temperature data in demo
        None, // No precipitation data in demo
        source_images.clone(),
        output_dir.join("demo_drought").to_string_lossy().to_string(),
        None,
    )?;

    info!("Drought Analysis Results:");
    info!("  Drought Severity: {:?}", drought_result.drought_severity);
    info!("  Affected Area: {:.2} hectares", drought_result.affected_area_hectares);
    info!("  Recovery Probability: {:.1}%", drought_result.recovery_probability * 100.0);

    info!("🗺️ Running land cover classification...");
    let landcover_result = processor.classify_land_cover(
        bands.clone(),
        source_images.clone(),
        output_dir.join("demo_landcover").to_string_lossy().to_string(),
    ).await?;

    info!("Land Cover Classification Results:");
    for (class, percentage) in &landcover_result.class_distribution {
        info!("  {:?}: {:.1}%", class, percentage);
    }

    // Demonstrate multi-index analysis
    info!("📈 Running comprehensive multi-index analysis...");
    let analysis_types = vec![
        AnalysisType::Ndvi,
        AnalysisType::Evi,
        AnalysisType::Savi,
        AnalysisType::Ndwi,
        AnalysisType::Nbr,
        AnalysisType::Ndbi,
        AnalysisType::Ndsi,
        AnalysisType::Bsi,
    ];

    let comprehensive_results = processor.analyze_comprehensive(
        bands,
        analysis_types.clone(),
        source_images,
        output_dir.join("demo_comprehensive").to_string_lossy().to_string(),
        None,
    ).await?;

    info!("Comprehensive Analysis completed with {} results", comprehensive_results.len());

    // Save summary report
    save_demo_report(
        output_dir,
        &vegetation_result,
        &water_result,
        &drought_result,
        &burn_result,
        &landcover_result,
    ).await?;

    Ok(())
}

/// Generate synthetic satellite band data
fn generate_synthetic_band(rows: usize, cols: usize, min_val: f32, max_val: f32) -> Array2<f32> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut band = Array2::zeros((rows, cols));

    for ((i, j), pixel) in band.indexed_iter_mut() {
        // Create some spatial patterns
        let center_x = cols as f32 / 2.0;
        let center_y = rows as f32 / 2.0;
        let distance = ((i as f32 - center_y).powi(2) + (j as f32 - center_x).powi(2)).sqrt();
        let normalized_distance = distance / (center_x.max(center_y));
        
        // Add some vegetation patterns (higher values in center, water bodies in corners)
        let base_value = if i < 20 && j < 20 {
            // Water body in top-left corner
            min_val + (max_val - min_val) * 0.1
        } else if i > 80 && j > 80 {
            // Bare soil in bottom-right
            min_val + (max_val - min_val) * 0.3
        } else {
            // Vegetation gradient from center
            min_val + (max_val - min_val) * (1.0 - normalized_distance * 0.5)
        };
        
        // Add noise
        let noise = rng.gen_range(-0.05..0.05);
        *pixel = (base_value + noise).clamp(min_val, max_val);
    }

    band
}

/// Parse analysis types from command line
fn parse_analysis_types(types_str: &str, all_analyses: bool) -> Vec<AnalysisType> {
    if all_analyses {
        return vec![
            AnalysisType::Ndvi,
            AnalysisType::Evi,
            AnalysisType::Savi,
            AnalysisType::Arvi,
            AnalysisType::Msavi,
            AnalysisType::Cvi,
            AnalysisType::Lai,
            AnalysisType::FCover,
            AnalysisType::Ndwi,
            AnalysisType::Mndwi,
            AnalysisType::Awei,
            AnalysisType::Vhi,
            AnalysisType::Tci,
            AnalysisType::Vci,
            AnalysisType::Pdi,
            AnalysisType::Nbr,
            AnalysisType::Dnbr,
            AnalysisType::Bai,
            AnalysisType::Ndbi,
            AnalysisType::Ui,
            AnalysisType::Ndsi,
            AnalysisType::Bsi,
            AnalysisType::LandCover,
        ];
    }

    let mut analysis_types = Vec::new();
    
    for type_str in types_str.split(',') {
        let analysis_type = match type_str.trim().to_lowercase().as_str() {
            "ndvi" => AnalysisType::Ndvi,
            "evi" => AnalysisType::Evi,
            "savi" => AnalysisType::Savi,
            "arvi" => AnalysisType::Arvi,
            "msavi" => AnalysisType::Msavi,
            "cvi" => AnalysisType::Cvi,
            "lai" => AnalysisType::Lai,
            "fcover" => AnalysisType::FCover,
            "ndwi" => AnalysisType::Ndwi,
            "mndwi" => AnalysisType::Mndwi,
            "awei" => AnalysisType::Awei,
            "vhi" => AnalysisType::Vhi,
            "tci" => AnalysisType::Tci,
            "vci" => AnalysisType::Vci,
            "pdi" => AnalysisType::Pdi,
            "nbr" => AnalysisType::Nbr,
            "dnbr" => AnalysisType::Dnbr,
            "bai" => AnalysisType::Bai,
            "ndbi" => AnalysisType::Ndbi,
            "ui" => AnalysisType::Ui,
            "ndsi" => AnalysisType::Ndsi,
            "bsi" => AnalysisType::Bsi,
            "drought" => AnalysisType::Vhi,
            "water" => AnalysisType::Ndwi,
            "vegetation" => AnalysisType::Ndvi,
            "landcover" => AnalysisType::LandCover,
            "burn" => AnalysisType::Nbr,
            _ => {
                warn!("Unknown analysis type: {}", type_str);
                continue;
            }
        };
        analysis_types.push(analysis_type);
    }

    if analysis_types.is_empty() {
        analysis_types.push(AnalysisType::Ndvi);
    }

    analysis_types
}

/// Process real satellite imagery
async fn process_satellite_imagery(
    processor: &ComprehensiveAnalysisProcessor,
    input_dir: &PathBuf,
    output_dir: &PathBuf,
    analysis_types: Vec<AnalysisType>,
) -> AgroResult<()> {
    warn!("Real satellite imagery processing not yet implemented in this demo.");
    warn!("Please use --demo flag to run with synthetic data.");
    warn!("To process real data, you would need to:");
    warn!("  1. Load GeoTIFF files from the input directory");
    warn!("  2. Extract individual bands (blue, green, red, nir, swir1, swir2)");
    warn!("  3. Convert to ndarray::Array2<f32> format");
    warn!("  4. Call the appropriate analysis methods");
    
    info!("For now, running demonstration mode...");
    run_demonstration(processor, output_dir).await?;
    
    Ok(())
}

/// Save a comprehensive demo report
async fn save_demo_report(
    output_dir: &PathBuf,
    vegetation_result: &VegetationAnalysisResult,
    water_result: &WaterAnalysisResult,
    drought_result: &DroughtAnalysisResult,
    burn_result: &BurnAnalysisResult,
    landcover_result: &LandCoverResult,
) -> AgroResult<()> {
    let report_path = output_dir.join("comprehensive_analysis_report.md");
    
    let report_content = format!(
        r#"# Comprehensive Satellite Image Analysis Report

## Analysis Overview
Generated: {}

## 🌱 Vegetation Analysis
- **Overall Health**: {:?}
- **Total Biomass**: {:.2} tons
- **Biomass Density**: {:.2} tons/hectare
- **Carbon Stock**: {:.2} tons
- **Growth Stage**: {:?}
- **Water Stress**: {:?}
- **Nutrient Stress**: {:?}

## 💧 Water Body Analysis
- **Total Water Area**: {:.2} hectares
- **Number of Water Bodies**: {}
- **Overall Water Quality**: {:?}
- **Algae Presence**: {:?}

## 🌵 Drought Analysis
- **Drought Severity**: {:?}
- **Affected Area**: {:.2} hectares
- **Recovery Probability**: {:.1}%
- **Crop Yield Impact**: {:.1}%

## 🔥 Burn Analysis
- **Burn Severity**: {:?}
- **Burned Area**: {:.2} hectares
- **Recovery Stage**: {:?}

## 🗺️ Land Cover Classification
{}

## 📊 Analysis Statistics
- **Total Processing Time**: <1 second (synthetic data)
- **Spatial Resolution**: 10m per pixel
- **Image Dimensions**: 100x100 pixels
- **Total Area Analyzed**: 100 hectares

## 📝 Recommendations
Based on the analysis results:

{}

## 🔬 Technical Details
This analysis was performed using the Comprehensive Satellite Image Analysis System,
which implements over 20 different spectral indices and classification algorithms
specifically designed for agricultural and environmental monitoring.

### Indices Computed:
- NDVI (Normalized Difference Vegetation Index)
- EVI (Enhanced Vegetation Index)
- SAVI (Soil Adjusted Vegetation Index)
- NDWI (Normalized Difference Water Index)
- NBR (Normalized Burn Ratio)
- VHI (Vegetation Health Index)
- And many more...

### Analysis Capabilities:
- Vegetation health assessment
- Water body detection and quality analysis
- Drought monitoring and impact assessment
- Burn severity mapping and recovery tracking
- Land cover classification
- Multi-temporal change detection
- Anomaly detection and forecasting
"#,
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        vegetation_result.health_classification.overall_health,
        vegetation_result.biomass_estimate.total_biomass_tons,
        vegetation_result.biomass_estimate.biomass_density_tons_per_hectare,
        vegetation_result.biomass_estimate.carbon_stock_tons,
        vegetation_result.phenology.growth_stage,
        vegetation_result.stress_indicators.water_stress,
        vegetation_result.stress_indicators.nutrient_stress,
        water_result.total_water_area_hectares,
        water_result.water_bodies.len(),
        water_result.water_quality.overall_quality,
        water_result.water_quality.algae_presence,
        drought_result.drought_severity,
        drought_result.affected_area_hectares,
        drought_result.recovery_probability * 100.0,
        drought_result.impact_assessment.crop_yield_impact,
        burn_result.burn_severity,
        burn_result.burned_area_hectares,
        burn_result.recovery_stage,
        format_land_cover_distribution(&landcover_result.class_distribution),
        generate_recommendations(vegetation_result, water_result, drought_result, burn_result),
    );

    tokio::fs::write(report_path, report_content).await?;
    info!("📄 Comprehensive analysis report saved to: comprehensive_analysis_report.md");

    Ok(())
}

fn format_land_cover_distribution(distribution: &HashMap<LandCoverType, f32>) -> String {
    let mut lines = Vec::new();
    for (class, percentage) in distribution {
        lines.push(format!("- **{:?}**: {:.1}%", class, percentage));
    }
    lines.join("\n")
}

fn generate_recommendations(
    vegetation_result: &VegetationAnalysisResult,
    water_result: &WaterAnalysisResult,
    drought_result: &DroughtAnalysisResult,
    burn_result: &BurnAnalysisResult,
) -> String {
    let mut recommendations = Vec::new();

    // Vegetation recommendations
    match vegetation_result.health_classification.overall_health {
        HealthStatus::Poor | HealthStatus::Critical => {
            recommendations.push("🌱 **Vegetation**: Immediate intervention required - consider irrigation, fertilization, or pest management".to_string());
        },
        HealthStatus::Moderate => {
            recommendations.push("🌱 **Vegetation**: Monitor closely and consider preventive measures".to_string());
        },
        _ => {
            recommendations.push("🌱 **Vegetation**: Continue current management practices".to_string());
        }
    }

    // Add stress-specific recommendations
    for rec in &vegetation_result.stress_indicators.recommendations {
        recommendations.push(format!("🌱 **Action**: {}", rec));
    }

    // Water recommendations
    match water_result.water_quality.overall_quality {
        WaterQualityLevel::Poor | WaterQualityLevel::VeryPoor => {
            recommendations.push("💧 **Water Quality**: Water quality concerns detected - test for pollutants and consider treatment".to_string());
        },
        _ => {
            recommendations.push("💧 **Water Quality**: Water quality appears adequate".to_string());
        }
    }

    // Drought recommendations
    match drought_result.drought_severity {
        DroughtSeverity::Severe | DroughtSeverity::Extreme => {
            recommendations.push("🌵 **Drought**: Severe drought conditions - implement water conservation and emergency irrigation".to_string());
        },
        DroughtSeverity::Moderate => {
            recommendations.push("🌵 **Drought**: Monitor soil moisture and prepare contingency plans".to_string());
        },
        _ => {}
    }

    // Burn recommendations
    match burn_result.burn_severity {
        BurnSeverity::High | BurnSeverity::HighPostFire => {
            recommendations.push("🔥 **Fire Recovery**: Implement erosion control and plan restoration activities".to_string());
        },
        _ => {}
    }

    if recommendations.is_empty() {
        recommendations.push("✅ **Overall**: No immediate concerns detected - continue monitoring".to_string());
    }

    recommendations.join("\n")
}
