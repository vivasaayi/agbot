use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use anyhow::Result;

/// NDVI (Normalized Difference Vegetation Index) analysis processor
pub struct NdviAnalysisProcessor {
    config: NdviAnalysisConfig,
    vegetation_indices: HashMap<Uuid, VegetationIndex>,
    threshold_settings: NdviThresholds,
    statistical_cache: HashMap<String, NdviStatistics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NdviAnalysisConfig {
    pub red_band_wavelength: f32,
    pub nir_band_wavelength: f32,
    pub cloud_threshold: f32,
    pub shadow_threshold: f32,
    pub enable_atmospheric_correction: bool,
    pub enable_shadow_detection: bool,
    pub output_resolution: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VegetationIndex {
    pub id: Uuid,
    pub location: (f64, f64), // lat, lon
    pub ndvi_value: f32,
    pub quality_score: f32,
    pub vegetation_type: VegetationType,
    pub health_status: VegetationHealth,
    pub measurement_time: DateTime<Utc>,
    pub confidence_level: f32,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VegetationType {
    Crop,
    Grass,
    Forest,
    Shrub,
    BareGround,
    Water,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VegetationHealth {
    Excellent,
    Good,
    Fair,
    Poor,
    Critical,
    NoVegetation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NdviThresholds {
    pub no_vegetation: f32,
    pub sparse_vegetation: f32,
    pub moderate_vegetation: f32,
    pub dense_vegetation: f32,
    pub water_threshold: f32,
}

impl Default for NdviThresholds {
    fn default() -> Self {
        Self {
            no_vegetation: 0.1,
            sparse_vegetation: 0.2,
            moderate_vegetation: 0.5,
            dense_vegetation: 0.8,
            water_threshold: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NdviStatistics {
    pub mean_ndvi: f32,
    pub median_ndvi: f32,
    pub std_deviation: f32,
    pub min_ndvi: f32,
    pub max_ndvi: f32,
    pub vegetation_coverage: f32, // percentage
    pub healthy_vegetation_ratio: f32,
    pub total_pixels: u32,
    pub analysis_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NdviAnalysisRequest {
    pub id: Uuid,
    pub red_band_data: Vec<u16>,
    pub nir_band_data: Vec<u16>,
    pub image_width: u32,
    pub image_height: u32,
    pub georeference_info: GeoreferenceInfo,
    pub capture_time: DateTime<Utc>,
    pub quality_mask: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoreferenceInfo {
    pub top_left_lat: f64,
    pub top_left_lon: f64,
    pub bottom_right_lat: f64,
    pub bottom_right_lon: f64,
    pub pixel_size_m: f32,
    pub coordinate_system: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NdviAnalysisResult {
    pub request_id: Uuid,
    pub ndvi_map: Vec<f32>,
    pub vegetation_indices: Vec<VegetationIndex>,
    pub statistics: NdviStatistics,
    pub processed_at: DateTime<Utc>,
    pub processing_time_ms: u64,
    pub quality_flags: Vec<QualityFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QualityFlag {
    CloudContamination,
    ShadowDetected,
    LowLightConditions,
    AtmosphericDistortion,
    SensorNoise,
    ProcessingError,
}

impl NdviAnalysisProcessor {
    pub fn new(config: NdviAnalysisConfig) -> Self {
        Self {
            config,
            vegetation_indices: HashMap::new(),
            threshold_settings: NdviThresholds::default(),
            statistical_cache: HashMap::new(),
        }
    }

    pub async fn process_ndvi_request(&mut self, request: NdviAnalysisRequest) -> Result<NdviAnalysisResult> {
        let start_time = std::time::Instant::now();
        
        // Validate input data
        self.validate_input_data(&request)?;
        
        // Calculate NDVI values
        let ndvi_map = self.calculate_ndvi(&request.red_band_data, &request.nir_band_data)?;
        
        // Apply quality filtering
        let filtered_ndvi = self.apply_quality_filtering(&ndvi_map, &request)?;
        
        // Extract vegetation indices
        let vegetation_indices = self.extract_vegetation_indices(&filtered_ndvi, &request)?;
        
        // Calculate statistics
        let statistics = self.calculate_statistics(&filtered_ndvi, &request)?;
        
        // Detect quality issues
        let quality_flags = self.detect_quality_issues(&request, &ndvi_map)?;
        
        let processing_time = start_time.elapsed().as_millis() as u64;
        
        let result = NdviAnalysisResult {
            request_id: request.id,
            ndvi_map: filtered_ndvi,
            vegetation_indices: vegetation_indices.clone(),
            statistics: statistics.clone(),
            processed_at: Utc::now(),
            processing_time_ms: processing_time,
            quality_flags,
        };
        
        // Cache results
        for index in vegetation_indices {
            self.vegetation_indices.insert(index.id, index);
        }
        
        let cache_key = format!("{}_{}", request.capture_time.timestamp(), request.id);
        self.statistical_cache.insert(cache_key, statistics);
        
        tracing::info!("Processed NDVI analysis for request {} in {}ms", request.id, processing_time);
        Ok(result)
    }
    
    fn validate_input_data(&self, request: &NdviAnalysisRequest) -> Result<()> {
        if request.red_band_data.is_empty() || request.nir_band_data.is_empty() {
            return Err(anyhow::anyhow!("Empty band data"));
        }
        
        if request.red_band_data.len() != request.nir_band_data.len() {
            return Err(anyhow::anyhow!("Band data length mismatch"));
        }
        
        let expected_pixels = (request.image_width * request.image_height) as usize;
        if request.red_band_data.len() != expected_pixels {
            return Err(anyhow::anyhow!("Image dimensions don't match data length"));
        }
        
        Ok(())
    }
    
    fn calculate_ndvi(&self, red_data: &[u16], nir_data: &[u16]) -> Result<Vec<f32>> {
        let mut ndvi_values = Vec::with_capacity(red_data.len());
        
        for (red, nir) in red_data.iter().zip(nir_data.iter()) {
            let red_f = *red as f32;
            let nir_f = *nir as f32;
            
            let ndvi = if red_f + nir_f > 0.0 {
                (nir_f - red_f) / (nir_f + red_f)
            } else {
                -1.0 // Invalid/no data
            };
            
            ndvi_values.push(ndvi.clamp(-1.0, 1.0));
        }
        
        Ok(ndvi_values)
    }
    
    fn apply_quality_filtering(&self, ndvi_map: &[f32], request: &NdviAnalysisRequest) -> Result<Vec<f32>> {
        let mut filtered = ndvi_map.to_vec();
        
        // Apply quality mask if available
        if let Some(quality_mask) = &request.quality_mask {
            for (i, &quality) in quality_mask.iter().enumerate() {
                if i < filtered.len() && quality == 0 {
                    filtered[i] = -1.0; // Mark as invalid
                }
            }
        }
        
        // Apply cloud and shadow filtering
        if self.config.enable_shadow_detection {
            self.filter_shadows(&mut filtered)?;
        }
        
        Ok(filtered)
    }
    
    fn filter_shadows(&self, ndvi_data: &mut [f32]) -> Result<()> {
        // Simple shadow detection based on NDVI values
        for value in ndvi_data.iter_mut() {
            if *value < self.config.shadow_threshold {
                *value = -1.0; // Mark as shadow
            }
        }
        Ok(())
    }
    
    fn extract_vegetation_indices(&self, ndvi_map: &[f32], request: &NdviAnalysisRequest) -> Result<Vec<VegetationIndex>> {
        let mut indices = Vec::new();
        let pixel_count = (request.image_width * request.image_height) as usize;
        
        // Sample vegetation indices at regular intervals
        let sample_interval = 100; // Every 100 pixels
        
        for i in (0..pixel_count).step_by(sample_interval) {
            if i < ndvi_map.len() && ndvi_map[i] >= 0.0 {
                let (lat, lon) = self.pixel_to_coordinates(i, request)?;
                
                let vegetation_type = self.classify_vegetation_type(ndvi_map[i]);
                let health_status = self.assess_vegetation_health(ndvi_map[i]);
                
                let index = VegetationIndex {
                    id: Uuid::new_v4(),
                    location: (lat, lon),
                    ndvi_value: ndvi_map[i],
                    quality_score: self.calculate_pixel_quality(ndvi_map[i]),
                    vegetation_type,
                    health_status,
                    measurement_time: request.capture_time,
                    confidence_level: 0.85, // TODO: Calculate based on quality factors
                    metadata: HashMap::new(),
                };
                
                indices.push(index);
            }
        }
        
        Ok(indices)
    }
    
    fn pixel_to_coordinates(&self, pixel_index: usize, request: &NdviAnalysisRequest) -> Result<(f64, f64)> {
        let row = pixel_index as u32 / request.image_width;
        let col = pixel_index as u32 % request.image_width;
        
        let lat_range = request.georeference_info.top_left_lat - request.georeference_info.bottom_right_lat;
        let lon_range = request.georeference_info.bottom_right_lon - request.georeference_info.top_left_lon;
        
        let lat = request.georeference_info.top_left_lat - (row as f64 / request.image_height as f64) * lat_range;
        let lon = request.georeference_info.top_left_lon + (col as f64 / request.image_width as f64) * lon_range;
        
        Ok((lat, lon))
    }
    
    fn classify_vegetation_type(&self, ndvi: f32) -> VegetationType {
        if ndvi < self.threshold_settings.no_vegetation {
            if ndvi < self.threshold_settings.water_threshold {
                VegetationType::Water
            } else {
                VegetationType::BareGround
            }
        } else if ndvi < self.threshold_settings.sparse_vegetation {
            VegetationType::Grass
        } else if ndvi < self.threshold_settings.moderate_vegetation {
            VegetationType::Crop
        } else if ndvi < self.threshold_settings.dense_vegetation {
            VegetationType::Shrub
        } else {
            VegetationType::Forest
        }
    }
    
    fn assess_vegetation_health(&self, ndvi: f32) -> VegetationHealth {
        if ndvi < self.threshold_settings.no_vegetation {
            VegetationHealth::NoVegetation
        } else if ndvi < 0.3 {
            VegetationHealth::Critical
        } else if ndvi < 0.4 {
            VegetationHealth::Poor
        } else if ndvi < 0.6 {
            VegetationHealth::Fair
        } else if ndvi < 0.8 {
            VegetationHealth::Good
        } else {
            VegetationHealth::Excellent
        }
    }
    
    fn calculate_pixel_quality(&self, ndvi: f32) -> f32 {
        // Quality based on NDVI validity and typical ranges
        if ndvi < -0.5 || ndvi > 1.0 {
            0.0 // Invalid
        } else if ndvi >= -0.1 && ndvi <= 0.9 {
            1.0 // High quality
        } else {
            0.7 // Medium quality
        }
    }
    
    fn calculate_statistics(&self, ndvi_map: &[f32], _request: &NdviAnalysisRequest) -> Result<NdviStatistics> {
        let valid_pixels: Vec<f32> = ndvi_map.iter()
            .filter(|&&val| val >= 0.0)
            .copied()
            .collect();
        
        if valid_pixels.is_empty() {
            return Ok(NdviStatistics {
                mean_ndvi: 0.0,
                median_ndvi: 0.0,
                std_deviation: 0.0,
                min_ndvi: 0.0,
                max_ndvi: 0.0,
                vegetation_coverage: 0.0,
                healthy_vegetation_ratio: 0.0,
                total_pixels: 0,
                analysis_time: Utc::now(),
            });
        }
        
        let mean = valid_pixels.iter().sum::<f32>() / valid_pixels.len() as f32;
        
        let mut sorted_pixels = valid_pixels.clone();
        sorted_pixels.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = sorted_pixels[sorted_pixels.len() / 2];
        
        let variance = valid_pixels.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f32>() / valid_pixels.len() as f32;
        let std_deviation = variance.sqrt();
        
        let min_ndvi = *sorted_pixels.first().unwrap();
        let max_ndvi = *sorted_pixels.last().unwrap();
        
        let vegetation_pixels = valid_pixels.iter()
            .filter(|&&val| val > self.threshold_settings.no_vegetation)
            .count();
        let vegetation_coverage = vegetation_pixels as f32 / valid_pixels.len() as f32 * 100.0;
        
        let healthy_pixels = valid_pixels.iter()
            .filter(|&&val| val > 0.4) // Healthy vegetation threshold
            .count();
        let healthy_vegetation_ratio = if vegetation_pixels > 0 {
            healthy_pixels as f32 / vegetation_pixels as f32
        } else {
            0.0
        };
        
        Ok(NdviStatistics {
            mean_ndvi: mean,
            median_ndvi: median,
            std_deviation,
            min_ndvi,
            max_ndvi,
            vegetation_coverage,
            healthy_vegetation_ratio,
            total_pixels: valid_pixels.len() as u32,
            analysis_time: Utc::now(),
        })
    }
    
    fn detect_quality_issues(&self, _request: &NdviAnalysisRequest, ndvi_map: &[f32]) -> Result<Vec<QualityFlag>> {
        let mut flags = Vec::new();
        
        // Check for excessive invalid pixels
        let invalid_count = ndvi_map.iter().filter(|&&val| val < 0.0).count();
        let invalid_ratio = invalid_count as f32 / ndvi_map.len() as f32;
        
        if invalid_ratio > 0.3 {
            flags.push(QualityFlag::CloudContamination);
        }
        
        if invalid_ratio > 0.1 {
            flags.push(QualityFlag::ShadowDetected);
        }
        
        // Check for unrealistic NDVI ranges
        let extreme_values = ndvi_map.iter()
            .filter(|&&val| val > 1.0 || val < -1.0)
            .count();
        
        if extreme_values > 0 {
            flags.push(QualityFlag::ProcessingError);
        }
        
        Ok(flags)
    }
    
    pub async fn get_historical_trends(&self, location: (f64, f64), radius_m: f32) -> Result<Vec<VegetationIndex>> {
        let mut nearby_indices = Vec::new();
        
        for index in self.vegetation_indices.values() {
            let distance = self.calculate_distance(location, index.location);
            if distance <= radius_m {
                nearby_indices.push(index.clone());
            }
        }
        
        nearby_indices.sort_by(|a, b| a.measurement_time.cmp(&b.measurement_time));
        Ok(nearby_indices)
    }
    
    fn calculate_distance(&self, pos1: (f64, f64), pos2: (f64, f64)) -> f32 {
        let lat1 = pos1.0.to_radians();
        let lat2 = pos2.0.to_radians();
        let delta_lat = (pos2.0 - pos1.0).to_radians();
        let delta_lon = (pos2.1 - pos1.1).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2) +
            lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        (6371000.0 * c) as f32 // Earth radius in meters
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ndvi_calculation() {
        let config = NdviAnalysisConfig {
            red_band_wavelength: 665.0,
            nir_band_wavelength: 842.0,
            cloud_threshold: 0.8,
            shadow_threshold: 0.1,
            enable_atmospheric_correction: false,
            enable_shadow_detection: true,
            output_resolution: 1.0,
        };
        
        let mut processor = NdviAnalysisProcessor::new(config);
        
        // Test NDVI calculation with sample data
        let red_data = vec![100, 150, 200]; // Lower values for healthy vegetation
        let nir_data = vec![300, 400, 500]; // Higher values for healthy vegetation
        
        let ndvi_result = processor.calculate_ndvi(&red_data, &nir_data).unwrap();
        
        assert_eq!(ndvi_result.len(), 3);
        assert!(ndvi_result[0] > 0.0); // Should be positive for vegetation
        assert!(ndvi_result[0] <= 1.0); // Should be within valid range
    }

    #[test]
    fn test_vegetation_classification() {
        let config = NdviAnalysisConfig {
            red_band_wavelength: 665.0,
            nir_band_wavelength: 842.0,
            cloud_threshold: 0.8,
            shadow_threshold: 0.1,
            enable_atmospheric_correction: false,
            enable_shadow_detection: true,
            output_resolution: 1.0,
        };
        
        let processor = NdviAnalysisProcessor::new(config);
        
        assert_eq!(processor.classify_vegetation_type(-0.1), VegetationType::Water);
        assert_eq!(processor.classify_vegetation_type(0.05), VegetationType::BareGround);
        assert_eq!(processor.classify_vegetation_type(0.3), VegetationType::Grass);
        assert_eq!(processor.classify_vegetation_type(0.7), VegetationType::Crop);
        assert_eq!(processor.classify_vegetation_type(0.9), VegetationType::Forest);
    }
}
