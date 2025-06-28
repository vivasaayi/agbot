use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use image::{ImageBuffer, Rgb, RgbImage};
use nalgebra::{Point3, Vector3};

pub mod ndvi;
pub mod thermal;
pub mod lidar_overlay;
pub mod composite;

pub use ndvi::NdviProcessor;
pub use thermal::ThermalProcessor;
pub use lidar_overlay::LidarOverlayProcessor;
pub use composite::CompositeOverlayEngine;

/// Core sensor overlay data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorOverlay {
    pub id: Uuid,
    pub overlay_type: OverlayType,
    pub timestamp: DateTime<Utc>,
    pub spatial_bounds: SpatialBounds,
    pub resolution: (u32, u32),
    pub data: OverlayData,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum OverlayType {
    NDVI,
    Thermal,
    LidarElevation,
    LidarIntensity,
    Composite,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialBounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub min_z: Option<f64>,
    pub max_z: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OverlayData {
    Image {
        width: u32,
        height: u32,
        channels: u32,
        data: Vec<u8>,
    },
    Grid {
        width: u32,
        height: u32,
        values: Vec<f32>,
        min_value: f32,
        max_value: f32,
    },
    PointCloud {
        points: Vec<Point3<f32>>,
        values: Vec<f32>,
        colors: Option<Vec<Rgb<u8>>>,
    },
    Heatmap {
        width: u32,
        height: u32,
        intensities: Vec<f32>,
        color_map: String,
    },
}

/// Raw sensor input for processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorInput {
    pub sensor_id: String,
    pub sensor_type: String,
    pub timestamp: DateTime<Utc>,
    pub position: Point3<f64>,
    pub orientation: Vector3<f64>,
    pub data: SensorInputData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensorInputData {
    MultispectralImage {
        bands: HashMap<String, ImageData>,
        calibration: MultispectralCalibration,
    },
    ThermalImage {
        image: ImageData,
        temperature_range: (f32, f32),
        emissivity: f32,
    },
    LidarScan {
        points: Vec<LidarPoint>,
        intensity_range: (f32, f32),
        scan_angle_range: (f32, f32),
    },
    RgbImage {
        image: ImageData,
        camera_params: CameraParameters,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub channels: u32,
    pub pixel_data: Vec<u8>,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultispectralCalibration {
    pub dark_current: HashMap<String, f32>,
    pub gain: HashMap<String, f32>,
    pub reflectance_panel: HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidarPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub intensity: f32,
    pub return_number: u8,
    pub classification: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraParameters {
    pub focal_length_mm: f32,
    pub sensor_width_mm: f32,
    pub sensor_height_mm: f32,
    pub iso: u32,
    pub exposure_time_ms: f32,
}

/// Main overlay processing engine
pub struct OverlayEngine {
    processors: HashMap<OverlayType, Box<dyn OverlayProcessor>>,
    output_cache: HashMap<Uuid, SensorOverlay>,
    processing_queue: Vec<ProcessingJob>,
}

pub trait OverlayProcessor: Send + Sync {
    fn process(&self, inputs: &[SensorInput]) -> Result<SensorOverlay>;
    fn can_process(&self, sensor_type: &str) -> bool;
    fn get_overlay_type(&self) -> OverlayType;
}

#[derive(Debug, Clone)]
pub struct ProcessingJob {
    pub id: Uuid,
    pub overlay_type: OverlayType,
    pub inputs: Vec<SensorInput>,
    pub priority: u8,
    pub created_at: DateTime<Utc>,
}

impl OverlayEngine {
    pub fn new() -> Self {
        let mut processors: HashMap<OverlayType, Box<dyn OverlayProcessor>> = HashMap::new();
        
        // Register default processors
        processors.insert(OverlayType::NDVI, Box::new(NdviProcessor::new()));
        processors.insert(OverlayType::Thermal, Box::new(ThermalProcessor::new()));
        processors.insert(OverlayType::LidarElevation, Box::new(LidarOverlayProcessor::new()));

        Self {
            processors,
            output_cache: HashMap::new(),
            processing_queue: Vec::new(),
        }
    }

    pub fn register_processor(&mut self, processor: Box<dyn OverlayProcessor>) {
        let overlay_type = processor.get_overlay_type();
        self.processors.insert(overlay_type, processor);
    }

    pub async fn submit_job(&mut self, overlay_type: OverlayType, inputs: Vec<SensorInput>) -> Result<Uuid> {
        let job = ProcessingJob {
            id: Uuid::new_v4(),
            overlay_type,
            inputs,
            priority: 5, // Default priority
            created_at: Utc::now(),
        };

        let job_id = job.id;
        self.processing_queue.push(job);
        self.processing_queue.sort_by(|a, b| b.priority.cmp(&a.priority));

        Ok(job_id)
    }

    pub async fn process_next_job(&mut self) -> Result<Option<SensorOverlay>> {
        if let Some(job) = self.processing_queue.pop() {
            if let Some(processor) = self.processors.get(&job.overlay_type) {
                let overlay = processor.process(&job.inputs)?;
                self.output_cache.insert(job.id, overlay.clone());
                Ok(Some(overlay))
            } else {
                Err(anyhow::anyhow!("No processor available for overlay type: {:?}", job.overlay_type))
            }
        } else {
            Ok(None)
        }
    }

    pub async fn process_all_pending(&mut self) -> Result<Vec<SensorOverlay>> {
        let mut results = Vec::new();
        
        while !self.processing_queue.is_empty() {
            if let Some(overlay) = self.process_next_job().await? {
                results.push(overlay);
            }
        }
        
        Ok(results)
    }

    pub fn get_overlay(&self, id: &Uuid) -> Option<&SensorOverlay> {
        self.output_cache.get(id)
    }

    pub fn list_overlays(&self) -> Vec<&SensorOverlay> {
        self.output_cache.values().collect()
    }

    pub fn clear_cache(&mut self) {
        self.output_cache.clear();
    }

    pub fn get_queue_length(&self) -> usize {
        self.processing_queue.len()
    }
}

impl SpatialBounds {
    pub fn new(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
            min_z: None,
            max_z: None,
        }
    }

    pub fn with_elevation(mut self, min_z: f64, max_z: f64) -> Self {
        self.min_z = Some(min_z);
        self.max_z = Some(max_z);
        self
    }

    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }

    pub fn area(&self) -> f64 {
        (self.max_x - self.min_x) * (self.max_y - self.min_y)
    }
}

impl Default for OverlayEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for overlay processing
pub mod utils {
    use super::*;

    pub fn create_heatmap_image(values: &[f32], width: u32, height: u32, color_map: &str) -> Result<RgbImage> {
        if values.len() != (width * height) as usize {
            return Err(anyhow::anyhow!("Values length doesn't match dimensions"));
        }

        let min_val = values.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_val = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let range = max_val - min_val;

        let mut img = ImageBuffer::new(width, height);
        
        for (i, &value) in values.iter().enumerate() {
            let x = (i as u32) % width;
            let y = (i as u32) / width;
            
            let normalized = if range > 0.0 {
                (value - min_val) / range
            } else {
                0.5
            };

            let color = match color_map {
                "viridis" => viridis_colormap(normalized),
                "jet" => jet_colormap(normalized),
                "hot" => hot_colormap(normalized),
                _ => grayscale_colormap(normalized),
            };

            img.put_pixel(x, y, color);
        }

        Ok(img)
    }

    fn viridis_colormap(t: f32) -> Rgb<u8> {
        let t = t.clamp(0.0, 1.0);
        // Simplified viridis colormap
        let r = (0.267004 + t * (0.282623 - 0.267004)) * 255.0;
        let g = (0.004874 + t * (0.940015 - 0.004874)) * 255.0;
        let b = (0.329415 + t * (0.644450 - 0.329415)) * 255.0;
        
        Rgb([r as u8, g as u8, b as u8])
    }

    fn jet_colormap(t: f32) -> Rgb<u8> {
        let t = t.clamp(0.0, 1.0);
        let r = if t < 0.35 { 0.0 } else if t < 0.66 { (t - 0.35) / 0.31 } else { 1.0 };
        let g = if t < 0.125 { 0.0 } else if t < 0.375 { (t - 0.125) / 0.25 } else if t < 0.64 { 1.0 } else { 1.0 - (t - 0.64) / 0.36 };
        let b = if t < 0.11 { 0.5 + t / 0.22 } else if t < 0.34 { 1.0 } else if t < 0.65 { 1.0 - (t - 0.34) / 0.31 } else { 0.0 };
        
        Rgb([(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8])
    }

    fn hot_colormap(t: f32) -> Rgb<u8> {
        let t = t.clamp(0.0, 1.0);
        let r = if t < 0.33 { t / 0.33 } else { 1.0 };
        let g = if t < 0.33 { 0.0 } else if t < 0.66 { (t - 0.33) / 0.33 } else { 1.0 };
        let b = if t < 0.66 { 0.0 } else { (t - 0.66) / 0.34 };
        
        Rgb([(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8])
    }

    fn grayscale_colormap(t: f32) -> Rgb<u8> {
        let intensity = (t.clamp(0.0, 1.0) * 255.0) as u8;
        Rgb([intensity, intensity, intensity])
    }

    pub fn interpolate_grid(points: &[(f64, f64, f32)], bounds: &SpatialBounds, resolution: (u32, u32)) -> Vec<f32> {
        let (width, height) = resolution;
        let mut grid = vec![0.0f32; (width * height) as usize];
        
        let dx = (bounds.max_x - bounds.min_x) / width as f64;
        let dy = (bounds.max_y - bounds.min_y) / height as f64;
        
        for y in 0..height {
            for x in 0..width {
                let world_x = bounds.min_x + x as f64 * dx;
                let world_y = bounds.min_y + y as f64 * dy;
                
                // Simple inverse distance weighting
                let mut weighted_sum = 0.0f32;
                let mut weight_sum = 0.0f32;
                
                for &(px, py, value) in points {
                    let distance = ((world_x - px).powi(2) + (world_y - py).powi(2)).sqrt();
                    if distance < 1e-6 {
                        weighted_sum = value;
                        weight_sum = 1.0;
                        break;
                    } else {
                        let weight = 1.0 / (distance as f32 + 1e-6);
                        weighted_sum += value * weight;
                        weight_sum += weight;
                    }
                }
                
                let idx = (y * width + x) as usize;
                grid[idx] = if weight_sum > 0.0 { weighted_sum / weight_sum } else { 0.0 };
            }
        }
        
        grid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_bounds() {
        let bounds = SpatialBounds::new(0.0, 0.0, 100.0, 100.0);
        assert!(bounds.contains_point(50.0, 50.0));
        assert!(!bounds.contains_point(-10.0, 50.0));
        assert_eq!(bounds.area(), 10000.0);
    }

    #[tokio::test]
    async fn test_overlay_engine() {
        let mut engine = OverlayEngine::new();
        assert_eq!(engine.get_queue_length(), 0);
        
        let inputs = vec![];
        let _job_id = engine.submit_job(OverlayType::NDVI, inputs).await.unwrap();
        assert_eq!(engine.get_queue_length(), 1);
    }

    #[test]
    fn test_heatmap_creation() {
        let values = vec![0.0, 0.5, 1.0, 0.25];
        let result = utils::create_heatmap_image(&values, 2, 2, "viridis");
        assert!(result.is_ok());
    }
}
