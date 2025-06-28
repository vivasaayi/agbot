use anyhow::Result;
use image::{ImageBuffer, Rgb, RgbImage};
use nalgebra::Point3;
use serde::{Deserialize, Serialize};
use std::path::Path;
use crate::{OverlayProcessor, SensorOverlay, SensorInput, OverlayType, OverlayData, SpatialBounds};
use uuid::Uuid;
use chrono::Utc;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct NdviProcessor {
    pub config: NdviConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NdviConfig {
    pub red_band_index: usize,
    pub nir_band_index: usize,
    pub output_format: String,
    pub color_mapping: ColorMapping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorMapping {
    pub low_vegetation: [u8; 3],
    pub medium_vegetation: [u8; 3],
    pub high_vegetation: [u8; 3],
    pub water: [u8; 3],
    pub soil: [u8; 3],
}

impl NdviProcessor {
    pub fn new(config: NdviConfig) -> Self {
        Self { config }
    }

    /// Calculate NDVI from multispectral image data
    pub fn calculate_ndvi(&self, red_band: &[f32], nir_band: &[f32]) -> Result<Vec<f32>> {
        if red_band.len() != nir_band.len() {
            return Err(anyhow::anyhow!("Red and NIR bands must have the same length"));
        }

        let ndvi_values: Vec<f32> = red_band
            .iter()
            .zip(nir_band.iter())
            .map(|(red, nir)| {
                if red + nir == 0.0 {
                    0.0
                } else {
                    (nir - red) / (nir + red)
                }
            })
            .collect();

        Ok(ndvi_values)
    }

    /// Generate a colored NDVI visualization
    pub fn generate_visualization(&self, ndvi_values: &[f32], width: u32, height: u32) -> Result<RgbImage> {
        let mut image = ImageBuffer::new(width, height);

        for (i, &ndvi) in ndvi_values.iter().enumerate() {
            let x = (i as u32) % width;
            let y = (i as u32) / width;

            if y >= height {
                break;
            }

            let color = self.map_ndvi_to_color(ndvi);
            image.put_pixel(x, y, Rgb(color));
        }

        Ok(image)
    }

    /// Map NDVI value to color based on vegetation health
    fn map_ndvi_to_color(&self, ndvi: f32) -> [u8; 3] {
        match ndvi {
            _ if ndvi < -0.1 => self.config.color_mapping.water,
            _ if ndvi < 0.2 => self.config.color_mapping.soil,
            _ if ndvi < 0.4 => self.config.color_mapping.low_vegetation,
            _ if ndvi < 0.6 => self.config.color_mapping.medium_vegetation,
            _ => self.config.color_mapping.high_vegetation,
        }
    }

    /// Process a complete field scan and generate NDVI overlay
    pub async fn process_field_scan(
        &self,
        scan_data: &FieldScanData,
        output_path: &Path,
    ) -> Result<NdviOverlayResult> {
        let ndvi_values = self.calculate_ndvi(&scan_data.red_band, &scan_data.nir_band)?;
        
        let visualization = self.generate_visualization(
            &ndvi_values,
            scan_data.width,
            scan_data.height,
        )?;

        // Save visualization
        visualization.save(output_path)?;

        // Calculate statistics
        let stats = self.calculate_statistics(&ndvi_values);

        Ok(NdviOverlayResult {
            ndvi_values,
            statistics: stats,
            output_path: output_path.to_path_buf(),
            timestamp: chrono::Utc::now(),
        })
    }

    /// Calculate NDVI statistics for the processed area
    fn calculate_statistics(&self, ndvi_values: &[f32]) -> NdviStatistics {
        let valid_values: Vec<f32> = ndvi_values.iter().filter(|&&v| v.is_finite()).copied().collect();
        
        if valid_values.is_empty() {
            return NdviStatistics::default();
        }

        let mean = valid_values.iter().sum::<f32>() / valid_values.len() as f32;
        let mut sorted = valid_values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let median = if sorted.len() % 2 == 0 {
            (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
        } else {
            sorted[sorted.len() / 2]
        };

        let min = *sorted.first().unwrap();
        let max = *sorted.last().unwrap();

        // Calculate vegetation coverage percentages
        let total_pixels = valid_values.len() as f32;
        let high_vegetation = valid_values.iter().filter(|&&v| v >= 0.6).count() as f32 / total_pixels * 100.0;
        let medium_vegetation = valid_values.iter().filter(|&&v| v >= 0.4 && v < 0.6).count() as f32 / total_pixels * 100.0;
        let low_vegetation = valid_values.iter().filter(|&&v| v >= 0.2 && v < 0.4).count() as f32 / total_pixels * 100.0;

        NdviStatistics {
            mean,
            median,
            min,
            max,
            high_vegetation_percent: high_vegetation,
            medium_vegetation_percent: medium_vegetation,
            low_vegetation_percent: low_vegetation,
            total_pixels: valid_values.len(),
        }
    }
}

impl OverlayProcessor for NdviProcessor {
    fn process(&self, inputs: &[SensorInput]) -> Result<SensorOverlay> {
        // For now, create a basic NDVI overlay
        // In a real implementation, this would process the multispectral data
        let overlay = SensorOverlay {
            id: Uuid::new_v4(),
            overlay_type: OverlayType::NDVI,
            timestamp: Utc::now(),
            spatial_bounds: SpatialBounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
                min_z: None,
                max_z: None,
            },
            resolution: (100, 100),
            data: OverlayData::Grid {
                width: 100,
                height: 100,
                values: vec![0.5; 10000], // Mock NDVI values
                min_value: -1.0,
                max_value: 1.0,
            },
            metadata: HashMap::new(),
        };
        Ok(overlay)
    }

    fn can_process(&self, sensor_type: &str) -> bool {
        sensor_type == "multispectral" || sensor_type == "rgb"
    }

    fn get_overlay_type(&self) -> OverlayType {
        OverlayType::NDVI
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldScanData {
    pub red_band: Vec<f32>,
    pub nir_band: Vec<f32>,
    pub width: u32,
    pub height: u32,
    pub gps_coordinates: Vec<Point3<f64>>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NdviOverlayResult {
    pub ndvi_values: Vec<f32>,
    pub statistics: NdviStatistics,
    pub output_path: std::path::PathBuf,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NdviStatistics {
    pub mean: f32,
    pub median: f32,
    pub min: f32,
    pub max: f32,
    pub high_vegetation_percent: f32,
    pub medium_vegetation_percent: f32,
    pub low_vegetation_percent: f32,
    pub total_pixels: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ndvi_calculation() {
        let processor = NdviProcessor::new(NdviConfig::default());
        let red = vec![0.1, 0.2, 0.3];
        let nir = vec![0.3, 0.4, 0.5];
        
        let ndvi = processor.calculate_ndvi(&red, &nir).unwrap();
        assert_eq!(ndvi.len(), 3);
        assert!((ndvi[0] - 0.5).abs() < 0.001); // (0.3-0.1)/(0.3+0.1) = 0.5
    }

    #[test]
    fn test_color_mapping() {
        let processor = NdviProcessor::new(NdviConfig::default());
        
        // Test water (negative NDVI)
        let water_color = processor.map_ndvi_to_color(-0.2);
        assert_eq!(water_color, [0, 0, 255]);
        
        // Test high vegetation
        let veg_color = processor.map_ndvi_to_color(0.8);
        assert_eq!(veg_color, [0, 255, 0]);
    }
}
