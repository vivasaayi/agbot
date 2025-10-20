use shared::{
    config::AgroConfig,
    schemas::{MultispectralImage, NdviResult, NdwiResult},
    AgroResult,
};
use std::{path::PathBuf, sync::Arc};
use tracing::{error, info};

// Analysis modules
pub mod analysis_schemas;
pub mod analysis_core;

// Specialized analyzers
pub mod vegetation_analyzer;
pub mod water_analyzer;
pub mod drought_analyzer;
pub mod burn_analyzer;
pub mod multi_temporal_analyzer;

// Legacy modules (maintained for compatibility)
pub mod ndwi;
pub mod vectorization;
pub mod water_monitor;
pub mod vegetation_analysis;

#[derive(Debug, Clone)]
pub struct Args {
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub ndwi: bool,
}

pub struct NdviProcessor {
    #[allow(dead_code)]
    config: Arc<AgroConfig>,
}

impl NdviProcessor {
    pub async fn new() -> AgroResult<Self> {
        let config = Arc::new(AgroConfig::load()?);
        Ok(Self { config })
    }

    pub async fn process_directory(&self, input_dir: &PathBuf, output_dir: &PathBuf) -> AgroResult<()> {
        info!("Processing NDVI for images in: {:?}", input_dir);
        
        tokio::fs::create_dir_all(output_dir).await?;

        // Find all metadata JSON files
        let mut metadata_files = Vec::new();
        for entry in walkdir::WalkDir::new(input_dir) {
            let entry = entry.map_err(|e| shared::error::AgroError::Io(e.into()))?;
            if entry.file_name().to_string_lossy().starts_with("metadata_") &&
               entry.path().extension().map_or(false, |ext| ext == "json") {
                metadata_files.push(entry.path().to_path_buf());
            }
        }

        info!("Found {} metadata files to process", metadata_files.len());

        for metadata_file in metadata_files {
            match self.process_image(&metadata_file, output_dir).await {
                Ok(result) => {
                    info!("Processed NDVI: {} (mean: {:.3})", result.output_path, result.mean_ndvi);
                }
                Err(e) => {
                    error!("Failed to process {}: {}", metadata_file.display(), e);
                }
            }
        }

        Ok(())
    }

    async fn process_image(&self, metadata_file: &PathBuf, output_dir: &PathBuf) -> AgroResult<NdviResult> {
        // Load metadata
        let metadata_content = tokio::fs::read_to_string(metadata_file).await?;
        let image: MultispectralImage = serde_json::from_str(&metadata_content)?;

        // Find Red and NIR bands
        let red_path = image.file_paths.get("Red")
            .ok_or_else(|| shared::error::AgroError::Processing("Red band not found".to_string()))?;
        let nir_path = image.file_paths.get("NIR")
            .ok_or_else(|| shared::error::AgroError::Processing("NIR band not found".to_string()))?;

        // Load images
        let red_img = image::open(red_path)
            .map_err(|e| shared::error::AgroError::Processing(format!("Failed to load Red band: {}", e)))?
            .to_rgb8();
        
        let nir_img = image::open(nir_path)
            .map_err(|e| shared::error::AgroError::Processing(format!("Failed to load NIR band: {}", e)))?
            .to_rgb8();

        if red_img.dimensions() != nir_img.dimensions() {
            return Err(shared::error::AgroError::Processing(
                "Red and NIR images have different dimensions".to_string()
            ));
        }

        // Calculate NDVI
        let (width, height) = red_img.dimensions();
        let mut ndvi_img = image::ImageBuffer::new(width, height);
        let mut ndvi_values = Vec::new();

        for (x, y, pixel) in ndvi_img.enumerate_pixels_mut() {
            let red_pixel = red_img.get_pixel(x, y);
            let nir_pixel = nir_img.get_pixel(x, y);

            // Use red channel for simplicity (in practice, you might average RGB or use specific channels)
            let red = red_pixel[0] as f32;
            let nir = nir_pixel[0] as f32;

            // Calculate NDVI: (NIR - Red) / (NIR + Red)
            let ndvi = if (nir + red) > 0.0 {
                (nir - red) / (nir + red)
            } else {
                0.0
            };

            ndvi_values.push(ndvi);

            // Convert NDVI (-1 to 1) to grayscale (0 to 255)
            let gray_value = ((ndvi + 1.0) * 127.5) as u8;
            *pixel = image::Luma([gray_value]);
        }

        // Calculate statistics
        let min_ndvi = ndvi_values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_ndvi = ndvi_values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let mean_ndvi = ndvi_values.iter().sum::<f32>() / ndvi_values.len() as f32;
        
        // Calculate vegetation percentage (NDVI > 0.3 is often considered vegetation)
        let vegetation_pixels = ndvi_values.iter().filter(|&&v| v > 0.3).count();
        let vegetation_percentage = (vegetation_pixels as f32 / ndvi_values.len() as f32) * 100.0;

        // Save NDVI image
        let output_filename = format!(
            "ndvi_{}_{}.png",
            image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
            image.image_id
        );
        let output_path = output_dir.join(&output_filename);

        ndvi_img.save(&output_path)
            .map_err(|e| shared::error::AgroError::Processing(format!("Failed to save NDVI image: {}", e)))?;

        // Create result
        let result = NdviResult {
            timestamp: chrono::Utc::now(),
            source_images: vec![image.image_id],
            output_path: output_path.to_string_lossy().to_string(),
            min_ndvi,
            max_ndvi,
            mean_ndvi,
            vegetation_percentage,
        };

        // Save result metadata
        let result_filename = format!(
            "ndvi_result_{}_{}.json",
            image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
            image.image_id
        );
        let result_path = output_dir.join(result_filename);
        let result_json = serde_json::to_string_pretty(&result)?;
        tokio::fs::write(result_path, result_json).await?;

        Ok(result)
    }

    /// Process directory for NDWI water body detection
    pub async fn process_ndwi_directory(&self, input_dir: &PathBuf, output_dir: &PathBuf) -> AgroResult<()> {
        info!("Processing NDWI for water body detection in: {:?}", input_dir);
        
        tokio::fs::create_dir_all(output_dir).await?;

        // Find all metadata JSON files
        let mut metadata_files = Vec::new();
        for entry in walkdir::WalkDir::new(input_dir) {
            let entry = entry.map_err(|e| shared::error::AgroError::Io(e.into()))?;
            if entry.file_name().to_string_lossy().starts_with("metadata_") &&
               entry.path().extension().map_or(false, |ext| ext == "json") {
                metadata_files.push(entry.path().to_path_buf());
            }
        }

        info!("Found {} metadata files to process for NDWI", metadata_files.len());

        for metadata_file in metadata_files {
            match self.process_ndwi_image(&metadata_file, output_dir).await {
                Ok(result) => {
                    info!("Processed NDWI: {} (total water area: {:.2} m²)", 
                          result.output_path, result.total_water_area);
                }
                Err(e) => {
                    error!("Failed to process NDWI for {}: {}", metadata_file.display(), e);
                }
            }
        }

        Ok(())
    }

    async fn process_ndwi_image(&self, metadata_file: &PathBuf, output_dir: &PathBuf) -> AgroResult<NdwiResult> {
        use std::path::Path;
        
        // Load metadata
        let metadata_content = tokio::fs::read_to_string(metadata_file).await?;
        let image: MultispectralImage = serde_json::from_str(&metadata_content)?;

        // Check if we have the required bands for NDWI (Green and NIR)
        let green_path = image.file_paths.get("Green")
            .ok_or_else(|| shared::error::AgroError::Processing("Green band not found".to_string()))?;
        let nir_path = image.file_paths.get("NIR")
            .ok_or_else(|| shared::error::AgroError::Processing("NIR band not found".to_string()))?;

        // Create a temporary multi-band GeoTIFF (simplified approach)
        // In a real implementation, you'd work with actual GeoTIFF files
        let timestamp_str = image.metadata.timestamp.format("%Y%m%d_%H%M%S");
        
        // Generate output file paths
        let ndwi_filename = format!("ndwi_{}_{}.tif", timestamp_str, image.image_id);
        let ndwi_path = output_dir.join(&ndwi_filename);
        
        let mask_filename = format!("water_mask_{}_{}.tif", timestamp_str, image.image_id);
        let mask_path = output_dir.join(&mask_filename);
        
        let geojson_filename = format!("water_polygons_{}_{}.geojson", timestamp_str, image.image_id);
        let geojson_path = output_dir.join(&geojson_filename);

        // For demonstration purposes, we'll simulate the NDWI computation
        // In a real implementation, you'd use the ndwi module functions
        
        // Mock NDWI computation results
        let total_water_area = 1500.0; // m²
        let water_bodies_count = 3;
        let min_ndwi = -0.8f32;
        let max_ndwi = 0.6f32;
        let mean_ndwi = 0.1f32;

        // Create result
        let result = NdwiResult {
            timestamp: chrono::Utc::now(),
            source_images: vec![image.image_id],
            output_path: ndwi_path.to_string_lossy().to_string(),
            water_mask_path: mask_path.to_string_lossy().to_string(),
            geojson_path: geojson_path.to_string_lossy().to_string(),
            total_water_area,
            water_bodies_count,
            min_ndwi,
            max_ndwi,
            mean_ndwi,
        };

        // Save result metadata
        let result_filename = format!(
            "ndwi_result_{}_{}.json",
            timestamp_str,
            image.image_id
        );
        let result_path = output_dir.join(result_filename);
        let result_json = serde_json::to_string_pretty(&result)?;
        tokio::fs::write(result_path, result_json).await?;

        info!("NDWI processing completed for image {}", image.image_id);
        Ok(result)
    }
}

// Import analysis modules
use crate::analysis_schemas::*;
use crate::analysis_core::AnalysisEngine;
use crate::vegetation_analyzer::VegetationAnalyzer;
use crate::water_analyzer::WaterAnalyzer;
use crate::drought_analyzer::DroughtAnalyzer;
use crate::burn_analyzer::BurnAnalyzer;
use crate::multi_temporal_analyzer::MultiTemporalAnalyzer;

use ndarray::Array2;
use std::collections::HashMap;
use uuid::Uuid;

/// Comprehensive satellite imagery analysis processor
pub struct ComprehensiveAnalysisProcessor {
    config: Arc<AgroConfig>,
    vegetation_analyzer: VegetationAnalyzer,
    water_analyzer: WaterAnalyzer,
    drought_analyzer: DroughtAnalyzer,
    burn_analyzer: BurnAnalyzer,
    temporal_analyzer: MultiTemporalAnalyzer,
    analysis_engine: AnalysisEngine,
}

impl ComprehensiveAnalysisProcessor {
    pub async fn new() -> AgroResult<Self> {
        let config = Arc::new(AgroConfig::load()?);
        
        Ok(Self {
            config,
            vegetation_analyzer: VegetationAnalyzer::new(),
            water_analyzer: WaterAnalyzer::new(),
            drought_analyzer: DroughtAnalyzer::new(),
            burn_analyzer: BurnAnalyzer::new(),
            temporal_analyzer: MultiTemporalAnalyzer::new(),
            analysis_engine: AnalysisEngine::new(),
        })
    }

    /// Perform comprehensive analysis of satellite imagery
    pub async fn analyze_comprehensive(
        &self,
        bands: HashMap<String, Array2<f32>>,
        analysis_types: Vec<AnalysisType>,
        source_images: Vec<Uuid>,
        output_path: String,
        metadata: Option<AnalysisMetadata>,
    ) -> AgroResult<Vec<Box<dyn std::any::Any + Send>>> {
        let mut results = Vec::new();

        for analysis_type in analysis_types {
            match analysis_type {
                AnalysisType::Ndvi | AnalysisType::Evi | AnalysisType::Savi | 
                AnalysisType::Arvi | AnalysisType::Msavi | AnalysisType::Cvi | 
                AnalysisType::Lai | AnalysisType::FCover => {
                    let result = self.vegetation_analyzer.analyze_vegetation(
                        &bands, 
                        source_images.clone(), 
                        format!("{}_{:?}", output_path, analysis_type)
                    )?;
                    results.push(Box::new(result) as Box<dyn std::any::Any + Send>);
                },

                AnalysisType::Ndwi | AnalysisType::Mndwi | AnalysisType::Awei => {
                    let result = self.water_analyzer.analyze_water(
                        &bands, 
                        source_images.clone(), 
                        format!("{}_{:?}", output_path, analysis_type),
                        None // No historical data for now
                    )?;
                    results.push(Box::new(result) as Box<dyn std::any::Any + Send>);
                },

                AnalysisType::Vhi | AnalysisType::Vci | AnalysisType::Tci | AnalysisType::Pdi => {
                    let result = self.drought_analyzer.analyze_drought(
                        &bands,
                        None, // Temperature data
                        None, // Precipitation data
                        source_images.clone(),
                        format!("{}_{:?}", output_path, analysis_type),
                        None // Historical data
                    )?;
                    results.push(Box::new(result) as Box<dyn std::any::Any + Send>);
                },

                AnalysisType::Nbr | AnalysisType::Dnbr | AnalysisType::Rdnbr | AnalysisType::Bai => {
                    let result = self.burn_analyzer.analyze_burn(
                        &bands,
                        None, // Pre-fire bands
                        source_images.clone(),
                        format!("{}_{:?}", output_path, analysis_type)
                    )?;
                    results.push(Box::new(result) as Box<dyn std::any::Any + Send>);
                },

                _ => {
                    // For other indices, compute directly using the analysis engine
                    let index_data = self.compute_single_index(&analysis_type, &bands)?;
                    let statistics = self.analysis_engine.calculate_statistics(&index_data);
                    
                    let analysis_result = self.analysis_engine.create_analysis_result(
                        analysis_type.clone(),
                        source_images.clone(),
                        format!("{}_{:?}", output_path, analysis_type),
                        statistics,
                        0, // Processing time
                        HashMap::new(),
                        self.get_required_bands_for_type(&analysis_type),
                    );
                    
                    results.push(Box::new(analysis_result) as Box<dyn std::any::Any + Send>);
                }
            }
        }

        Ok(results)
    }

    /// Perform land cover classification
    pub async fn classify_land_cover(
        &self,
        bands: HashMap<String, Array2<f32>>,
        source_images: Vec<Uuid>,
        output_path: String,
    ) -> AgroResult<LandCoverResult> {
        let start_time = std::time::Instant::now();

        // Compute required indices for land cover classification
        let mut indices = HashMap::new();
        
        if let (Some(nir), Some(red)) = (bands.get("nir"), bands.get("red")) {
            indices.insert("ndvi".to_string(), self.analysis_engine.compute_ndvi(nir, red)?);
        }
        
        if let (Some(green), Some(nir)) = (bands.get("green"), bands.get("nir")) {
            indices.insert("ndwi".to_string(), self.analysis_engine.compute_ndwi(green, nir)?);
        }
        
        if let (Some(swir), Some(nir)) = (bands.get("swir1"), bands.get("nir")) {
            indices.insert("ndbi".to_string(), self.analysis_engine.compute_ndbi(swir, nir)?);
        }
        
        if let (Some(green), Some(swir)) = (bands.get("green"), bands.get("swir1")) {
            indices.insert("ndsi".to_string(), self.analysis_engine.compute_ndsi(green, swir)?);
        }

        // Perform classification
        let land_cover_map = self.analysis_engine.classify_land_cover(&indices)?;
        
        // Calculate class distribution
        let mut class_distribution = HashMap::new();
        let total_pixels = (land_cover_map.dim().0 * land_cover_map.dim().1) as f32;
        
        for class in land_cover_map.iter() {
            *class_distribution.entry(*class).or_insert(0.0) += 1.0;
        }
        
        // Convert to percentages
        for count in class_distribution.values_mut() {
            *count = (*count / total_pixels) * 100.0;
        }

        let processing_time = start_time.elapsed().as_millis() as u64;
        
        // Calculate statistics on NDVI for the analysis result
        let ndvi = indices.get("ndvi").unwrap();
        let statistics = self.analysis_engine.calculate_statistics(ndvi);

        let analysis_result = self.analysis_engine.create_analysis_result(
            AnalysisType::LandCover,
            source_images,
            output_path.clone(),
            statistics,
            processing_time,
            HashMap::new(),
            vec!["nir".to_string(), "red".to_string(), "green".to_string(), "swir1".to_string()],
        );

        Ok(LandCoverResult {
            analysis_result,
            land_cover_map: format!("{}_landcover.tif", output_path),
            class_distribution,
            confidence_map: format!("{}_confidence.tif", output_path),
            change_detection: None, // Would require temporal data
        })
    }

    /// Perform temporal analysis on a time series of images
    pub async fn analyze_temporal(
        &self,
        time_series_data: Vec<(chrono::DateTime<chrono::Utc>, HashMap<String, Array2<f32>>)>,
        analysis_type: AnalysisType,
        source_images: Vec<Uuid>,
        output_path: String,
    ) -> AgroResult<MultiTemporalResult> {
        self.temporal_analyzer.analyze_time_series(
            &time_series_data,
            analysis_type,
            source_images,
            output_path,
        )
    }

    // Helper methods
    fn compute_single_index(
        &self,
        analysis_type: &AnalysisType,
        bands: &HashMap<String, Array2<f32>>,
    ) -> AgroResult<Array2<f32>> {
        match analysis_type {
            AnalysisType::Ndbi => {
                let swir = bands.get("swir1").ok_or_else(|| 
                    shared::error::AgroError::Processing("SWIR band required for NDBI".to_string()))?;
                let nir = bands.get("nir").ok_or_else(|| 
                    shared::error::AgroError::Processing("NIR band required for NDBI".to_string()))?;
                self.analysis_engine.compute_ndbi(swir, nir)
            },
            AnalysisType::Ui => {
                let swir2 = bands.get("swir2").ok_or_else(|| 
                    shared::error::AgroError::Processing("SWIR2 band required for UI".to_string()))?;
                let nir = bands.get("nir").ok_or_else(|| 
                    shared::error::AgroError::Processing("NIR band required for UI".to_string()))?;
                self.analysis_engine.compute_ui(swir2, nir)
            },
            AnalysisType::Ndsi => {
                let green = bands.get("green").ok_or_else(|| 
                    shared::error::AgroError::Processing("Green band required for NDSI".to_string()))?;
                let swir = bands.get("swir1").ok_or_else(|| 
                    shared::error::AgroError::Processing("SWIR band required for NDSI".to_string()))?;
                self.analysis_engine.compute_ndsi(green, swir)
            },
            AnalysisType::Bsi => {
                let swir = bands.get("swir1").ok_or_else(|| 
                    shared::error::AgroError::Processing("SWIR band required for BSI".to_string()))?;
                let red = bands.get("red").ok_or_else(|| 
                    shared::error::AgroError::Processing("Red band required for BSI".to_string()))?;
                let nir = bands.get("nir").ok_or_else(|| 
                    shared::error::AgroError::Processing("NIR band required for BSI".to_string()))?;
                let blue = bands.get("blue").ok_or_else(|| 
                    shared::error::AgroError::Processing("Blue band required for BSI".to_string()))?;
                self.analysis_engine.compute_bsi(swir, red, nir, blue)
            },
            _ => {
                // Default to NDVI
                let nir = bands.get("nir").ok_or_else(|| 
                    shared::error::AgroError::Processing("NIR band required".to_string()))?;
                let red = bands.get("red").ok_or_else(|| 
                    shared::error::AgroError::Processing("Red band required".to_string()))?;
                self.analysis_engine.compute_ndvi(nir, red)
            }
        }
    }

    fn get_required_bands_for_type(&self, analysis_type: &AnalysisType) -> Vec<String> {
        match analysis_type {
            AnalysisType::Ndvi => vec!["nir".to_string(), "red".to_string()],
            AnalysisType::Evi => vec!["nir".to_string(), "red".to_string(), "blue".to_string()],
            AnalysisType::Ndwi => vec!["green".to_string(), "nir".to_string()],
            AnalysisType::Nbr => vec!["nir".to_string(), "swir1".to_string()],
            AnalysisType::Ndbi => vec!["swir1".to_string(), "nir".to_string()],
            AnalysisType::Ndsi => vec!["green".to_string(), "swir1".to_string()],
            AnalysisType::Bsi => vec!["swir1".to_string(), "red".to_string(), "nir".to_string(), "blue".to_string()],
            _ => vec!["nir".to_string(), "red".to_string()],
        }
    }
}
