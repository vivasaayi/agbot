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
pub struct ThermalProcessor {
    pub config: ThermalConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalConfig {
    pub temperature_range: TemperatureRange,
    pub color_palette: ThermalColorPalette,
    pub calibration: ThermalCalibration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureRange {
    pub min_celsius: f32,
    pub max_celsius: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalColorPalette {
    pub cold: [u8; 3],      // Blue
    pub cool: [u8; 3],      // Cyan
    pub moderate: [u8; 3],  // Green
    pub warm: [u8; 3],      // Yellow
    pub hot: [u8; 3],       // Red
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalCalibration {
    pub offset: f32,
    pub scale: f32,
    pub ambient_temp: f32,
}

impl Default for ThermalConfig {
    fn default() -> Self {
        Self {
            temperature_range: TemperatureRange {
                min_celsius: -10.0,
                max_celsius: 50.0,
            },
            color_palette: ThermalColorPalette {
                cold: [0, 0, 255],      // Blue
                cool: [0, 255, 255],    // Cyan
                moderate: [0, 255, 0],  // Green
                warm: [255, 255, 0],    // Yellow
                hot: [255, 0, 0],       // Red
            },
            calibration: ThermalCalibration {
                offset: 0.0,
                scale: 1.0,
                ambient_temp: 20.0,
            },
        }
    }
}

impl ThermalProcessor {
    pub fn new(config: ThermalConfig) -> Self {
        Self { config }
    }

    /// Convert raw thermal sensor data to temperature values
    pub fn raw_to_temperature(&self, raw_values: &[u16]) -> Result<Vec<f32>> {
        let temperatures: Vec<f32> = raw_values
            .iter()
            .map(|&raw| {
                let temp = (raw as f32 * self.config.calibration.scale) + self.config.calibration.offset;
                temp - self.config.calibration.ambient_temp
            })
            .collect();

        Ok(temperatures)
    }

    /// Generate thermal visualization image
    pub fn generate_thermal_image(&self, temperatures: &[f32], width: u32, height: u32) -> Result<RgbImage> {
        let mut image = ImageBuffer::new(width, height);

        for (i, &temp) in temperatures.iter().enumerate() {
            let x = (i as u32) % width;
            let y = (i as u32) / width;

            if y >= height {
                break;
            }

            let color = self.temperature_to_color(temp);
            image.put_pixel(x, y, Rgb(color));
        }

        Ok(image)
    }

    /// Map temperature to color using thermal palette
    fn temperature_to_color(&self, temperature: f32) -> [u8; 3] {
        let normalized = (temperature - self.config.temperature_range.min_celsius)
            / (self.config.temperature_range.max_celsius - self.config.temperature_range.min_celsius);
        
        let clamped = normalized.clamp(0.0, 1.0);

        // Interpolate between colors based on temperature
        match clamped {
            t if t < 0.2 => self.interpolate_color(
                self.config.color_palette.cold,
                self.config.color_palette.cool,
                t * 5.0,
            ),
            t if t < 0.4 => self.interpolate_color(
                self.config.color_palette.cool,
                self.config.color_palette.moderate,
                (t - 0.2) * 5.0,
            ),
            t if t < 0.6 => self.interpolate_color(
                self.config.color_palette.moderate,
                self.config.color_palette.warm,
                (t - 0.4) * 5.0,
            ),
            t if t < 0.8 => self.interpolate_color(
                self.config.color_palette.warm,
                self.config.color_palette.hot,
                (t - 0.6) * 5.0,
            ),
            _ => self.config.color_palette.hot,
        }
    }

    /// Interpolate between two colors
    fn interpolate_color(&self, color1: [u8; 3], color2: [u8; 3], factor: f32) -> [u8; 3] {
        let factor = factor.clamp(0.0, 1.0);
        [
            (color1[0] as f32 * (1.0 - factor) + color2[0] as f32 * factor) as u8,
            (color1[1] as f32 * (1.0 - factor) + color2[1] as f32 * factor) as u8,
            (color1[2] as f32 * (1.0 - factor) + color2[2] as f32 * factor) as u8,
        ]
    }

    /// Process thermal field scan data
    pub async fn process_thermal_scan(
        &self,
        scan_data: &ThermalScanData,
        output_path: &Path,
    ) -> Result<ThermalOverlayResult> {
        let temperatures = self.raw_to_temperature(&scan_data.raw_thermal_data)?;
        
        let thermal_image = self.generate_thermal_image(
            &temperatures,
            scan_data.width,
            scan_data.height,
        )?;

        // Save thermal image
        thermal_image.save(output_path)?;

        // Calculate thermal statistics
        let stats = self.calculate_thermal_statistics(&temperatures);

        // Detect thermal anomalies
        let anomalies = self.detect_thermal_anomalies(&temperatures, &scan_data.gps_coordinates);

        Ok(ThermalOverlayResult {
            temperatures,
            statistics: stats,
            anomalies,
            output_path: output_path.to_path_buf(),
            timestamp: chrono::Utc::now(),
        })
    }

    /// Calculate thermal statistics
    fn calculate_thermal_statistics(&self, temperatures: &[f32]) -> ThermalStatistics {
        if temperatures.is_empty() {
            return ThermalStatistics::default();
        }

        let mean = temperatures.iter().sum::<f32>() / temperatures.len() as f32;
        let mut sorted = temperatures.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let median = if sorted.len() % 2 == 0 {
            (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
        } else {
            sorted[sorted.len() / 2]
        };

        let min = sorted[0];
        let max = sorted[sorted.len() - 1];

        // Calculate standard deviation
        let variance = temperatures
            .iter()
            .map(|&temp| (temp - mean).powi(2))
            .sum::<f32>() / temperatures.len() as f32;
        let std_dev = variance.sqrt();

        ThermalStatistics {
            mean,
            median,
            min,
            max,
            std_dev,
            hot_spots: temperatures.iter().filter(|&&t| t > mean + 2.0 * std_dev).count(),
            cold_spots: temperatures.iter().filter(|&&t| t < mean - 2.0 * std_dev).count(),
            total_pixels: temperatures.len(),
        }
    }

    /// Detect thermal anomalies that might indicate crop stress or irrigation issues
    fn detect_thermal_anomalies(&self, temperatures: &[f32], coordinates: &[Point3<f64>]) -> Vec<ThermalAnomaly> {
        let mut anomalies = Vec::new();
        
        if temperatures.is_empty() {
            return anomalies;
        }

        let mean = temperatures.iter().sum::<f32>() / temperatures.len() as f32;
        let variance = temperatures
            .iter()
            .map(|&temp| (temp - mean).powi(2))
            .sum::<f32>() / temperatures.len() as f32;
        let std_dev = variance.sqrt();

        for (i, &temp) in temperatures.iter().enumerate() {
            let severity = if temp > mean + 2.0 * std_dev {
                AnomalySeverity::High
            } else if temp > mean + std_dev {
                AnomalySeverity::Medium
            } else if temp < mean - 2.0 * std_dev {
                AnomalySeverity::High
            } else if temp < mean - std_dev {
                AnomalySeverity::Medium
            } else {
                continue;
            };

            let anomaly_type = if temp > mean {
                ThermalAnomalyType::HotSpot
            } else {
                ThermalAnomalyType::ColdSpot
            };

            let location = coordinates.get(i).cloned().unwrap_or(Point3::new(0.0, 0.0, 0.0));

            anomalies.push(ThermalAnomaly {
                anomaly_type,
                severity,
                temperature: temp,
                location,
                confidence: if severity == AnomalySeverity::High { 0.9 } else { 0.7 },
            });
        }

        anomalies
    }
}

impl OverlayProcessor for ThermalProcessor {
    fn process(&self, _inputs: &[SensorInput]) -> Result<SensorOverlay> {
        // Create a basic thermal overlay
        let overlay = SensorOverlay {
            id: Uuid::new_v4(),
            overlay_type: OverlayType::Thermal,
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
            data: OverlayData::Heatmap {
                width: 100,
                height: 100,
                intensities: vec![25.0; 10000], // Mock temperature values
                color_map: "thermal".to_string(),
            },
            metadata: HashMap::new(),
        };
        Ok(overlay)
    }

    fn can_process(&self, sensor_type: &str) -> bool {
        sensor_type == "thermal" || sensor_type == "infrared"
    }

    fn get_overlay_type(&self) -> OverlayType {
        OverlayType::Thermal
    }
}

impl Default for ThermalProcessor {
    fn default() -> Self {
        Self::new(ThermalConfig::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalScanData {
    pub raw_thermal_data: Vec<u16>,
    pub width: u32,
    pub height: u32,
    pub gps_coordinates: Vec<Point3<f64>>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalOverlayResult {
    pub temperatures: Vec<f32>,
    pub statistics: ThermalStatistics,
    pub anomalies: Vec<ThermalAnomaly>,
    pub output_path: std::path::PathBuf,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThermalStatistics {
    pub mean: f32,
    pub median: f32,
    pub min: f32,
    pub max: f32,
    pub std_dev: f32,
    pub hot_spots: usize,
    pub cold_spots: usize,
    pub total_pixels: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalAnomaly {
    pub anomaly_type: ThermalAnomalyType,
    pub severity: AnomalySeverity,
    pub temperature: f32,
    pub location: Point3<f64>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThermalAnomalyType {
    HotSpot,
    ColdSpot,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_to_temperature_conversion() {
        let processor = ThermalProcessor::new(ThermalConfig::default());
        let raw_data = vec![1000, 1500, 2000];
        
        let temperatures = processor.raw_to_temperature(&raw_data).unwrap();
        assert_eq!(temperatures.len(), 3);
    }

    #[test]
    fn test_temperature_to_color_mapping() {
        let processor = ThermalProcessor::new(ThermalConfig::default());
        
        // Test cold temperature
        let cold_color = processor.temperature_to_color(-5.0);
        assert_eq!(cold_color, [0, 0, 255]); // Should be blue
        
        // Test hot temperature
        let hot_color = processor.temperature_to_color(45.0);
        assert_eq!(hot_color, [255, 0, 0]); // Should be red
    }

    #[test]
    fn test_thermal_statistics() {
        let processor = ThermalProcessor::new(ThermalConfig::default());
        let temperatures = vec![10.0, 15.0, 20.0, 25.0, 30.0];
        
        let stats = processor.calculate_thermal_statistics(&temperatures);
        assert_eq!(stats.mean, 20.0);
        assert_eq!(stats.median, 20.0);
        assert_eq!(stats.min, 10.0);
        assert_eq!(stats.max, 30.0);
    }
}
