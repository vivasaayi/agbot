use anyhow::Result;
use image::{ImageBuffer, Rgba, RgbaImage};
use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use crate::{OverlayProcessor, SensorOverlay, SensorInput, OverlayType, OverlayData, SpatialBounds, RgbColor};
use uuid::Uuid;
use chrono::Utc;

/// LiDAR overlay processor for 3D mapping and terrain analysis
#[derive(Debug, Clone)]
pub struct LidarOverlayProcessor {
    pub config: LidarConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidarConfig {
    pub point_cloud_resolution: f32,
    pub height_color_mapping: HeightColorMapping,
    pub occupancy_grid_resolution: f32,
    pub max_range: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeightColorMapping {
    pub ground_level: [u8; 4],     // RGBA for ground
    pub low_vegetation: [u8; 4],   // RGBA for low plants
    pub medium_vegetation: [u8; 4], // RGBA for crops
    pub high_vegetation: [u8; 4],  // RGBA for trees
    pub obstacles: [u8; 4],        // RGBA for obstacles
}

impl LidarOverlayProcessor {
    pub fn new(config: LidarConfig) -> Self {
        Self { config }
    }

    /// Process LiDAR point cloud data into a 2D height map overlay
    pub async fn process_point_cloud(
        &self,
        point_cloud: &PointCloudData,
        output_path: &Path,
    ) -> Result<LidarOverlayResult> {
        // Convert point cloud to grid
        let height_map = self.create_height_map(&point_cloud.points)?;
        
        // Generate visualization
        let overlay_image = self.generate_height_overlay(&height_map)?;
        
        // Save overlay image
        overlay_image.save(output_path)?;

        // Create occupancy grid
        let occupancy_grid = self.create_occupancy_grid(&point_cloud.points)?;
        
        // Detect obstacles and features
        let features = self.extract_terrain_features(&height_map, &point_cloud.points);
        
        // Calculate terrain statistics
        let stats = self.calculate_terrain_statistics(&height_map);

        Ok(LidarOverlayResult {
            height_map,
            occupancy_grid,
            features,
            statistics: stats,
            output_path: output_path.to_path_buf(),
            timestamp: chrono::Utc::now(),
        })
    }

    /// Create a 2D height map from 3D point cloud
    fn create_height_map(&self, points: &[Point3<f32>]) -> Result<HeightMap> {
        let mut grid: HashMap<(i32, i32), Vec<f32>> = HashMap::new();
        let resolution = self.config.point_cloud_resolution;

        // Group points by grid cell
        for point in points {
            let grid_x = (point.x / resolution).round() as i32;
            let grid_y = (point.y / resolution).round() as i32;
            
            grid.entry((grid_x, grid_y))
                .or_insert_with(Vec::new)
                .push(point.z);
        }

        // Calculate average height for each cell
        let mut height_data = HashMap::new();
        for ((x, y), heights) in grid {
            let avg_height = heights.iter().sum::<f32>() / heights.len() as f32;
            height_data.insert((x, y), avg_height);
        }

        // Find bounds
        let min_x = height_data.keys().map(|(x, _)| *x).min().unwrap_or(0);
        let max_x = height_data.keys().map(|(x, _)| *x).max().unwrap_or(0);
        let min_y = height_data.keys().map(|(_, y)| *y).min().unwrap_or(0);
        let max_y = height_data.keys().map(|(_, y)| *y).max().unwrap_or(0);

        Ok(HeightMap {
            data: height_data,
            bounds: GridBounds { min_x, max_x, min_y, max_y },
            resolution,
        })
    }

    /// Generate a colored height overlay image
    fn generate_height_overlay(&self, height_map: &HeightMap) -> Result<RgbaImage> {
        let width = (height_map.bounds.max_x - height_map.bounds.min_x + 1) as u32;
        let height = (height_map.bounds.max_y - height_map.bounds.min_y + 1) as u32;
        
        let mut image = ImageBuffer::new(width, height);

        // Find height range for normalization
        let heights: Vec<f32> = height_map.data.values().cloned().collect();
        let min_height = heights.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_height = heights.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

        for ((grid_x, grid_y), &height_val) in &height_map.data {
            let img_x = (grid_x - height_map.bounds.min_x) as u32;
            let img_y = (grid_y - height_map.bounds.min_y) as u32;

            if img_x < width && img_y < height {
                let color = self.height_to_color(height_val, min_height, max_height);
                image.put_pixel(img_x, img_y, Rgba(color));
            }
        }

        Ok(image)
    }

    /// Map height value to color based on terrain type
    fn height_to_color(&self, height: f32, min_height: f32, max_height: f32) -> [u8; 4] {
        let relative_height = height - min_height;
        let height_range = max_height - min_height;
        
        if height_range == 0.0 {
            return self.config.height_color_mapping.ground_level;
        }

        let normalized_height = relative_height / height_range;

        match normalized_height {
            h if h < 0.1 => self.config.height_color_mapping.ground_level,
            h if h < 0.3 => self.config.height_color_mapping.low_vegetation,
            h if h < 0.6 => self.config.height_color_mapping.medium_vegetation,
            h if h < 0.9 => self.config.height_color_mapping.high_vegetation,
            _ => self.config.height_color_mapping.obstacles,
        }
    }

    /// Create occupancy grid for obstacle detection
    fn create_occupancy_grid(&self, points: &[Point3<f32>]) -> Result<OccupancyGrid> {
        let mut grid: HashMap<(i32, i32), u8> = HashMap::new();
        let resolution = self.config.occupancy_grid_resolution;

        for point in points {
            let grid_x = (point.x / resolution).round() as i32;
            let grid_y = (point.y / resolution).round() as i32;
            
            // Mark cell as occupied (255 = occupied, 0 = free)
            grid.insert((grid_x, grid_y), 255);
        }

        let min_x = grid.keys().map(|(x, _)| *x).min().unwrap_or(0);
        let max_x = grid.keys().map(|(x, _)| *x).max().unwrap_or(0);
        let min_y = grid.keys().map(|(_, y)| *y).min().unwrap_or(0);
        let max_y = grid.keys().map(|(_, y)| *y).max().unwrap_or(0);

        Ok(OccupancyGrid {
            data: grid,
            bounds: GridBounds { min_x, max_x, min_y, max_y },
            resolution,
        })
    }

    /// Extract terrain features like rows, obstacles, and boundaries
    fn extract_terrain_features(&self, height_map: &HeightMap, points: &[Point3<f32>]) -> Vec<TerrainFeature> {
        let mut features = Vec::new();

        // Detect crop rows using height variation analysis
        features.extend(self.detect_crop_rows(height_map));
        
        // Detect obstacles using height thresholds
        features.extend(self.detect_obstacles(height_map));
        
        // Detect field boundaries
        features.extend(self.detect_field_boundaries(points));

        features
    }

    /// Detect crop rows based on height patterns
    fn detect_crop_rows(&self, height_map: &HeightMap) -> Vec<TerrainFeature> {
        let mut features = Vec::new();
        
        // Simple row detection based on height variation patterns
        // This is a simplified implementation - real row detection would be more sophisticated
        for ((x, y), &height) in &height_map.data {
            // Look for regular patterns in height that might indicate crop rows
            if height > 0.2 && height < 2.0 { // Typical crop height range
                features.push(TerrainFeature {
                    feature_type: TerrainFeatureType::CropRow,
                    location: Point3::new(*x as f32 * height_map.resolution, *y as f32 * height_map.resolution, height),
                    confidence: 0.7,
                    metadata: format!("Height: {:.2}m", height),
                });
            }
        }

        features
    }

    /// Detect obstacles based on height thresholds
    fn detect_obstacles(&self, height_map: &HeightMap) -> Vec<TerrainFeature> {
        let mut features = Vec::new();
        
        let heights: Vec<f32> = height_map.data.values().cloned().collect();
        if heights.is_empty() {
            return features;
        }

        let mean_height = heights.iter().sum::<f32>() / heights.len() as f32;
        let obstacle_threshold = mean_height + 2.0; // Objects 2m above average

        for ((x, y), &height) in &height_map.data {
            if height > obstacle_threshold {
                features.push(TerrainFeature {
                    feature_type: TerrainFeatureType::Obstacle,
                    location: Point3::new(*x as f32 * height_map.resolution, *y as f32 * height_map.resolution, height),
                    confidence: 0.9,
                    metadata: format!("Height: {:.2}m above threshold", height - obstacle_threshold),
                });
            }
        }

        features
    }

    /// Detect field boundaries
    fn detect_field_boundaries(&self, points: &[Point3<f32>]) -> Vec<TerrainFeature> {
        let mut features = Vec::new();
        
        if points.is_empty() {
            return features;
        }

        // Find the convex hull points as potential boundary markers
        let min_x = points.iter().map(|p| p.x).fold(f32::INFINITY, |a, b| a.min(b));
        let max_x = points.iter().map(|p| p.x).fold(f32::NEG_INFINITY, |a, b| a.max(b));
        let min_y = points.iter().map(|p| p.y).fold(f32::INFINITY, |a, b| a.min(b));
        let max_y = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, |a, b| a.max(b));

        // Add corner points as boundary markers
        let boundary_points = vec![
            Point3::new(min_x, min_y, 0.0),
            Point3::new(max_x, min_y, 0.0),
            Point3::new(max_x, max_y, 0.0),
            Point3::new(min_x, max_y, 0.0),
        ];

        for point in boundary_points {
            features.push(TerrainFeature {
                feature_type: TerrainFeatureType::FieldBoundary,
                location: point,
                confidence: 0.8,
                metadata: "Detected field boundary".to_string(),
            });
        }

        features
    }

    /// Calculate terrain statistics
    fn calculate_terrain_statistics(&self, height_map: &HeightMap) -> TerrainStatistics {
        let heights: Vec<f32> = height_map.data.values().cloned().collect();
        
        if heights.is_empty() {
            return TerrainStatistics::default();
        }

        let mean = heights.iter().sum::<f32>() / heights.len() as f32;
        let mut sorted_heights = heights.clone();
        sorted_heights.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let median = if sorted_heights.len() % 2 == 0 {
            (sorted_heights[sorted_heights.len() / 2 - 1] + sorted_heights[sorted_heights.len() / 2]) / 2.0
        } else {
            sorted_heights[sorted_heights.len() / 2]
        };

        let min = sorted_heights[0];
        let max = sorted_heights[sorted_heights.len() - 1];

        let variance = heights.iter().map(|&h| (h - mean).powi(2)).sum::<f32>() / heights.len() as f32;
        let std_dev = variance.sqrt();

        let area_covered = heights.len() as f32 * height_map.resolution.powi(2);

        TerrainStatistics {
            mean_height: mean,
            median_height: median,
            min_height: min,
            max_height: max,
            height_std_dev: std_dev,
            area_covered_m2: area_covered,
            total_points: heights.len(),
        }
    }
}

impl OverlayProcessor for LidarOverlayProcessor {
    fn process(&self, _inputs: &[SensorInput]) -> Result<SensorOverlay> {
        // Create a basic LiDAR elevation overlay
        let overlay = SensorOverlay {
            id: Uuid::new_v4(),
            overlay_type: OverlayType::LidarElevation,
            timestamp: Utc::now(),
            spatial_bounds: SpatialBounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
                min_z: Some(0.0),
                max_z: Some(10.0),
            },
            resolution: (100, 100),
            data: OverlayData::PointCloud {
                points: vec![Point3::new(0.0, 0.0, 0.0); 1000], // Mock points
                values: vec![5.0; 1000], // Mock elevation values
                colors: Some(vec![RgbColor { r: 0, g: 255, b: 0 }; 1000]), // Green
            },
            metadata: HashMap::new(),
        };
        Ok(overlay)
    }

    fn can_process(&self, sensor_type: &str) -> bool {
        sensor_type == "lidar" || sensor_type == "point_cloud"
    }

    fn get_overlay_type(&self) -> OverlayType {
        OverlayType::LidarElevation
    }
}

// Data structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointCloudData {
    pub points: Vec<Point3<f32>>,
    pub intensities: Vec<f32>,
    pub gps_origin: Point3<f64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeightMap {
    pub data: HashMap<(i32, i32), f32>,
    pub bounds: GridBounds,
    pub resolution: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OccupancyGrid {
    pub data: HashMap<(i32, i32), u8>,
    pub bounds: GridBounds,
    pub resolution: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridBounds {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidarOverlayResult {
    pub height_map: HeightMap,
    pub occupancy_grid: OccupancyGrid,
    pub features: Vec<TerrainFeature>,
    pub statistics: TerrainStatistics,
    pub output_path: std::path::PathBuf,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainFeature {
    pub feature_type: TerrainFeatureType,
    pub location: Point3<f32>,
    pub confidence: f32,
    pub metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerrainFeatureType {
    CropRow,
    Obstacle,
    FieldBoundary,
    WaterFeature,
    RoadPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TerrainStatistics {
    pub mean_height: f32,
    pub median_height: f32,
    pub min_height: f32,
    pub max_height: f32,
    pub height_std_dev: f32,
    pub area_covered_m2: f32,
    pub total_points: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_height_map_creation() {
        let processor = LidarOverlayProcessor::new(LidarConfig::default());
        let points = vec![
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(0.1, 0.0, 1.1),
            Point3::new(0.0, 0.1, 0.9),
        ];
        
        let height_map = processor.create_height_map(&points).unwrap();
        assert!(!height_map.data.is_empty());
    }

    #[test]
    fn test_occupancy_grid_creation() {
        let processor = LidarOverlayProcessor::new(LidarConfig::default());
        let points = vec![Point3::new(1.0, 1.0, 1.0), Point3::new(2.0, 2.0, 2.0)];
        
        let grid = processor.create_occupancy_grid(&points).unwrap();
        assert!(!grid.data.is_empty());
    }

    #[test]
    fn test_height_to_color_mapping() {
        let processor = LidarOverlayProcessor::new(LidarConfig::default());
        
        // Test ground level
        let ground_color = processor.height_to_color(0.05, 0.0, 10.0);
        assert_eq!(ground_color, [139, 69, 19, 255]);
        
        // Test high vegetation
        let tree_color = processor.height_to_color(9.5, 0.0, 10.0);
        assert_eq!(tree_color, [255, 0, 0, 255]); // Should be obstacles color
    }
}
