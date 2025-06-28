use anyhow::Result;
use image::{ImageBuffer, Rgba, RgbaImage};
use nalgebra::Point3;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::ndvi::{NdviProcessor, NdviOverlayResult};
use crate::thermal::{ThermalProcessor, ThermalOverlayResult};
use crate::lidar_overlay::{LidarOverlayProcessor, LidarOverlayResult};

/// Composite overlay engine that combines multiple sensor data types
#[derive(Debug, Clone)]
pub struct CompositeOverlayEngine {
    pub config: CompositeConfig,
    pub ndvi_processor: NdviProcessor,
    pub thermal_processor: ThermalProcessor,
    pub lidar_processor: LidarOverlayProcessor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeConfig {
    pub overlay_types: Vec<OverlayType>,
    pub blending_mode: BlendingMode,
    pub opacity_settings: OpacitySettings,
    pub output_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum OverlayType {
    Ndvi,
    Thermal,
    Lidar,
    Rgb,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlendingMode {
    Alpha,
    Multiply,
    Overlay,
    Screen,
    HardLight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpacitySettings {
    pub ndvi_opacity: f32,
    pub thermal_opacity: f32,
    pub lidar_opacity: f32,
    pub rgb_opacity: f32,
}

impl Default for CompositeConfig {
    fn default() -> Self {
        Self {
            overlay_types: vec![OverlayType::Ndvi, OverlayType::Thermal, OverlayType::Lidar],
            blending_mode: BlendingMode::Alpha,
            opacity_settings: OpacitySettings {
                ndvi_opacity: 0.7,
                thermal_opacity: 0.5,
                lidar_opacity: 0.6,
                rgb_opacity: 1.0,
            },
            output_format: "PNG".to_string(),
        }
    }
}

impl CompositeOverlayEngine {
    pub fn new(
        config: CompositeConfig,
        ndvi_processor: NdviProcessor,
        thermal_processor: ThermalProcessor,
        lidar_processor: LidarOverlayProcessor,
    ) -> Self {
        Self {
            config,
            ndvi_processor,
            thermal_processor,
            lidar_processor,
        }
    }

    /// Process a complete multi-sensor field scan and create composite overlays
    pub async fn process_field_scan(
        &self,
        scan_data: &CompositeScanData,
        output_dir: &Path,
    ) -> Result<CompositeOverlayResult> {
        let mut overlay_results = Vec::new();

        // Process NDVI if available and requested
        if self.config.overlay_types.contains(&OverlayType::Ndvi) && scan_data.ndvi_data.is_some() {
            let ndvi_output = output_dir.join("ndvi_overlay.png");
            let ndvi_result = self.ndvi_processor
                .process_field_scan(scan_data.ndvi_data.as_ref().unwrap(), &ndvi_output)
                .await?;
            overlay_results.push(IndividualOverlayResult::Ndvi(ndvi_result));
        }

        // Process Thermal if available and requested
        if self.config.overlay_types.contains(&OverlayType::Thermal) && scan_data.thermal_data.is_some() {
            let thermal_output = output_dir.join("thermal_overlay.png");
            let thermal_result = self.thermal_processor
                .process_thermal_scan(scan_data.thermal_data.as_ref().unwrap(), &thermal_output)
                .await?;
            overlay_results.push(IndividualOverlayResult::Thermal(thermal_result));
        }

        // Process LiDAR if available and requested
        if self.config.overlay_types.contains(&OverlayType::Lidar) && scan_data.lidar_data.is_some() {
            let lidar_output = output_dir.join("lidar_overlay.png");
            let lidar_result = self.lidar_processor
                .process_point_cloud(scan_data.lidar_data.as_ref().unwrap(), &lidar_output)
                .await?;
            overlay_results.push(IndividualOverlayResult::Lidar(lidar_result));
        }

        // Create composite overlay
        let composite_image = self.create_composite_overlay(&overlay_results, scan_data)?;
        let composite_output = output_dir.join("composite_overlay.png");
        composite_image.save(&composite_output)?;

        // Generate analysis report
        let analysis = self.analyze_composite_data(&overlay_results);

        Ok(CompositeOverlayResult {
            individual_overlays: overlay_results,
            composite_image_path: composite_output,
            analysis,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Create a composite overlay by blending multiple sensor data types
    fn create_composite_overlay(
        &self,
        overlay_results: &[IndividualOverlayResult],
        scan_data: &CompositeScanData,
    ) -> Result<RgbaImage> {
        // Determine output image dimensions
        let (width, height) = self.determine_composite_dimensions(scan_data);
        let mut composite = ImageBuffer::new(width, height);

        // Initialize with base RGB image if available
        if let Some(rgb_data) = &scan_data.rgb_image {
            self.apply_rgb_base(&mut composite, rgb_data)?;
        } else {
            // Fill with transparent background
            for pixel in composite.pixels_mut() {
                *pixel = Rgba([0, 0, 0, 0]);
            }
        }

        // Blend each overlay type
        for overlay_result in overlay_results {
            match overlay_result {
                IndividualOverlayResult::Ndvi(ndvi_result) => {
                    self.blend_ndvi_overlay(&mut composite, ndvi_result)?;
                }
                IndividualOverlayResult::Thermal(thermal_result) => {
                    self.blend_thermal_overlay(&mut composite, thermal_result)?;
                }
                IndividualOverlayResult::Lidar(lidar_result) => {
                    self.blend_lidar_overlay(&mut composite, lidar_result)?;
                }
            }
        }

        Ok(composite)
    }

    /// Determine the dimensions for the composite image
    fn determine_composite_dimensions(&self, scan_data: &CompositeScanData) -> (u32, u32) {
        // Use RGB image dimensions if available
        if let Some(rgb_data) = &scan_data.rgb_image {
            return (rgb_data.width, rgb_data.height);
        }

        // Otherwise, use NDVI dimensions if available
        if let Some(ndvi_data) = &scan_data.ndvi_data {
            return (ndvi_data.width, ndvi_data.height);
        }

        // Default dimensions
        (1024, 1024)
    }

    /// Apply RGB base image to composite
    fn apply_rgb_base(&self, composite: &mut RgbaImage, rgb_data: &RgbImageData) -> Result<()> {
        for (i, &rgb_pixel) in rgb_data.data.iter().enumerate() {
            let x = (i as u32) % rgb_data.width;
            let y = (i as u32) / rgb_data.width;

            if x < composite.width() && y < composite.height() {
                let rgba_pixel = Rgba([
                    rgb_pixel[0],
                    rgb_pixel[1],
                    rgb_pixel[2],
                    (255.0 * self.config.opacity_settings.rgb_opacity) as u8,
                ]);
                composite.put_pixel(x, y, rgba_pixel);
            }
        }
        Ok(())
    }

    /// Blend NDVI overlay into composite
    fn blend_ndvi_overlay(&self, composite: &mut RgbaImage, ndvi_result: &NdviOverlayResult) -> Result<()> {
        let overlay_image = image::open(&ndvi_result.output_path)?;
        let overlay_rgba = overlay_image.to_rgba8();

        for (x, y, overlay_pixel) in overlay_rgba.enumerate_pixels() {
            if x < composite.width() && y < composite.height() {
                let base_pixel = composite.get_pixel(x, y);
                let blended = self.blend_pixels(
                    *base_pixel,
                    *overlay_pixel,
                    self.config.opacity_settings.ndvi_opacity,
                );
                composite.put_pixel(x, y, blended);
            }
        }
        Ok(())
    }

    /// Blend thermal overlay into composite
    fn blend_thermal_overlay(&self, composite: &mut RgbaImage, thermal_result: &ThermalOverlayResult) -> Result<()> {
        let overlay_image = image::open(&thermal_result.output_path)?;
        let overlay_rgba = overlay_image.to_rgba8();

        for (x, y, overlay_pixel) in overlay_rgba.enumerate_pixels() {
            if x < composite.width() && y < composite.height() {
                let base_pixel = composite.get_pixel(x, y);
                let blended = self.blend_pixels(
                    *base_pixel,
                    *overlay_pixel,
                    self.config.opacity_settings.thermal_opacity,
                );
                composite.put_pixel(x, y, blended);
            }
        }
        Ok(())
    }

    /// Blend LiDAR overlay into composite
    fn blend_lidar_overlay(&self, composite: &mut RgbaImage, lidar_result: &LidarOverlayResult) -> Result<()> {
        let overlay_image = image::open(&lidar_result.output_path)?;
        let overlay_rgba = overlay_image.to_rgba8();

        for (x, y, overlay_pixel) in overlay_rgba.enumerate_pixels() {
            if x < composite.width() && y < composite.height() {
                let base_pixel = composite.get_pixel(x, y);
                let blended = self.blend_pixels(
                    *base_pixel,
                    *overlay_pixel,
                    self.config.opacity_settings.lidar_opacity,
                );
                composite.put_pixel(x, y, blended);
            }
        }
        Ok(())
    }

    /// Blend two pixels based on the configured blending mode
    fn blend_pixels(&self, base: Rgba<u8>, overlay: Rgba<u8>, opacity: f32) -> Rgba<u8> {
        let alpha = (overlay.0[3] as f32 / 255.0) * opacity;
        
        match self.config.blending_mode {
            BlendingMode::Alpha => self.alpha_blend(base, overlay, alpha),
            BlendingMode::Multiply => self.multiply_blend(base, overlay, alpha),
            BlendingMode::Overlay => self.overlay_blend(base, overlay, alpha),
            BlendingMode::Screen => self.screen_blend(base, overlay, alpha),
            BlendingMode::HardLight => self.hard_light_blend(base, overlay, alpha),
        }
    }

    /// Alpha blending
    fn alpha_blend(&self, base: Rgba<u8>, overlay: Rgba<u8>, alpha: f32) -> Rgba<u8> {
        let inv_alpha = 1.0 - alpha;
        Rgba([
            (base.0[0] as f32 * inv_alpha + overlay.0[0] as f32 * alpha) as u8,
            (base.0[1] as f32 * inv_alpha + overlay.0[1] as f32 * alpha) as u8,
            (base.0[2] as f32 * inv_alpha + overlay.0[2] as f32 * alpha) as u8,
            ((base.0[3] as f32 * inv_alpha + overlay.0[3] as f32 * alpha).min(255.0)) as u8,
        ])
    }

    /// Multiply blending
    fn multiply_blend(&self, base: Rgba<u8>, overlay: Rgba<u8>, alpha: f32) -> Rgba<u8> {
        let blended = Rgba([
            ((base.0[0] as f32 * overlay.0[0] as f32) / 255.0) as u8,
            ((base.0[1] as f32 * overlay.0[1] as f32) / 255.0) as u8,
            ((base.0[2] as f32 * overlay.0[2] as f32) / 255.0) as u8,
            overlay.0[3],
        ]);
        self.alpha_blend(base, blended, alpha)
    }

    /// Overlay blending
    fn overlay_blend(&self, base: Rgba<u8>, overlay: Rgba<u8>, alpha: f32) -> Rgba<u8> {
        let blend_channel = |base: u8, overlay: u8| {
            if base < 128 {
                (2.0 * base as f32 * overlay as f32 / 255.0) as u8
            } else {
                (255.0 - 2.0 * (255.0 - base as f32) * (255.0 - overlay as f32) / 255.0) as u8
            }
        };

        let blended = Rgba([
            blend_channel(base.0[0], overlay.0[0]),
            blend_channel(base.0[1], overlay.0[1]),
            blend_channel(base.0[2], overlay.0[2]),
            overlay.0[3],
        ]);
        self.alpha_blend(base, blended, alpha)
    }

    /// Screen blending
    fn screen_blend(&self, base: Rgba<u8>, overlay: Rgba<u8>, alpha: f32) -> Rgba<u8> {
        let blended = Rgba([
            (255.0 - (255.0 - base.0[0] as f32) * (255.0 - overlay.0[0] as f32) / 255.0) as u8,
            (255.0 - (255.0 - base.0[1] as f32) * (255.0 - overlay.0[1] as f32) / 255.0) as u8,
            (255.0 - (255.0 - base.0[2] as f32) * (255.0 - overlay.0[2] as f32) / 255.0) as u8,
            overlay.0[3],
        ]);
        self.alpha_blend(base, blended, alpha)
    }

    /// Hard light blending
    fn hard_light_blend(&self, base: Rgba<u8>, overlay: Rgba<u8>, alpha: f32) -> Rgba<u8> {
        let blend_channel = |base: u8, overlay: u8| {
            if overlay < 128 {
                (2.0 * base as f32 * overlay as f32 / 255.0) as u8
            } else {
                (255.0 - 2.0 * (255.0 - base as f32) * (255.0 - overlay as f32) / 255.0) as u8
            }
        };

        let blended = Rgba([
            blend_channel(base.0[0], overlay.0[0]),
            blend_channel(base.0[1], overlay.0[1]),
            blend_channel(base.0[2], overlay.0[2]),
            overlay.0[3],
        ]);
        self.alpha_blend(base, blended, alpha)
    }

    /// Analyze the composite data and generate insights
    fn analyze_composite_data(&self, overlay_results: &[IndividualOverlayResult]) -> CompositeAnalysis {
        let mut analysis = CompositeAnalysis::default();

        for overlay_result in overlay_results {
            match overlay_result {
                IndividualOverlayResult::Ndvi(ndvi_result) => {
                    analysis.vegetation_health_score = Some(self.calculate_vegetation_health_score(&ndvi_result.statistics));
                    analysis.vegetation_coverage = Some(
                        ndvi_result.statistics.high_vegetation_percent + ndvi_result.statistics.medium_vegetation_percent
                    );
                }
                IndividualOverlayResult::Thermal(thermal_result) => {
                    analysis.temperature_anomalies = Some(thermal_result.anomalies.len());
                    analysis.stress_indicators = Some(self.assess_thermal_stress(&thermal_result.statistics));
                }
                IndividualOverlayResult::Lidar(lidar_result) => {
                    analysis.terrain_complexity = Some(self.calculate_terrain_complexity(&lidar_result.statistics));
                    analysis.obstacle_count = Some(
                        lidar_result.features.iter()
                            .filter(|f| matches!(f.feature_type, crate::lidar_overlay::TerrainFeatureType::Obstacle))
                            .count()
                    );
                }
            }
        }

        // Generate recommendations based on combined data
        analysis.recommendations = self.generate_recommendations(&analysis);

        analysis
    }

    /// Calculate vegetation health score from NDVI statistics
    fn calculate_vegetation_health_score(&self, ndvi_stats: &crate::ndvi::NdviStatistics) -> f32 {
        // Simple scoring based on vegetation coverage and mean NDVI
        let vegetation_factor = (ndvi_stats.high_vegetation_percent + ndvi_stats.medium_vegetation_percent) / 100.0;
        let ndvi_factor = (ndvi_stats.mean + 1.0) / 2.0; // Normalize NDVI from [-1,1] to [0,1]
        
        (vegetation_factor * 0.6 + ndvi_factor * 0.4) * 100.0
    }

    /// Assess thermal stress indicators
    fn assess_thermal_stress(&self, thermal_stats: &crate::thermal::ThermalStatistics) -> f32 {
        // Higher standard deviation and more hot spots indicate more stress
        let temp_variation = thermal_stats.std_dev / 10.0; // Normalize by expected variation
        let hot_spot_factor = thermal_stats.hot_spots as f32 / thermal_stats.total_pixels as f32;
        
        (temp_variation + hot_spot_factor * 2.0).min(1.0) * 100.0
    }

    /// Calculate terrain complexity score
    fn calculate_terrain_complexity(&self, terrain_stats: &crate::lidar_overlay::TerrainStatistics) -> f32 {
        // Higher height variation indicates more complex terrain
        let height_variation = terrain_stats.height_std_dev / (terrain_stats.max_height - terrain_stats.min_height + 0.1);
        (height_variation * 100.0).min(100.0)
    }

    /// Generate actionable recommendations based on analysis
    fn generate_recommendations(&self, analysis: &CompositeAnalysis) -> Vec<String> {
        let mut recommendations = Vec::new();

        if let Some(health_score) = analysis.vegetation_health_score {
            if health_score < 50.0 {
                recommendations.push("Low vegetation health detected. Consider soil testing and fertilization.".to_string());
            } else if health_score > 80.0 {
                recommendations.push("Excellent vegetation health. Maintain current management practices.".to_string());
            }
        }

        if let Some(stress) = analysis.stress_indicators {
            if stress > 70.0 {
                recommendations.push("High thermal stress detected. Check irrigation systems and consider water management.".to_string());
            }
        }

        if let Some(obstacles) = analysis.obstacle_count {
            if obstacles > 10 {
                recommendations.push(format!("Multiple obstacles detected ({}). Review field safety and navigation paths.", obstacles));
            }
        }

        if recommendations.is_empty() {
            recommendations.push("No immediate issues detected. Continue monitoring.".to_string());
        }

        recommendations
    }
}

// Data structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeScanData {
    pub ndvi_data: Option<crate::ndvi::FieldScanData>,
    pub thermal_data: Option<crate::thermal::ThermalScanData>,
    pub lidar_data: Option<crate::lidar_overlay::PointCloudData>,
    pub rgb_image: Option<RgbImageData>,
    pub gps_reference: Point3<f64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RgbImageData {
    pub data: Vec<[u8; 3]>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndividualOverlayResult {
    Ndvi(NdviOverlayResult),
    Thermal(ThermalOverlayResult),
    Lidar(LidarOverlayResult),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeOverlayResult {
    pub individual_overlays: Vec<IndividualOverlayResult>,
    pub composite_image_path: std::path::PathBuf,
    pub analysis: CompositeAnalysis,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompositeAnalysis {
    pub vegetation_health_score: Option<f32>,
    pub vegetation_coverage: Option<f32>,
    pub temperature_anomalies: Option<usize>,
    pub stress_indicators: Option<f32>,
    pub terrain_complexity: Option<f32>,
    pub obstacle_count: Option<usize>,
    pub recommendations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ndvi::NdviConfig;
    use crate::thermal::ThermalConfig;
    use crate::lidar_overlay::LidarConfig;

    #[test]
    fn test_composite_engine_creation() {
        let config = CompositeConfig::default();
        let ndvi_processor = NdviProcessor::new(NdviConfig::default());
        let thermal_processor = ThermalProcessor::new(ThermalConfig::default());
        let lidar_processor = LidarOverlayProcessor::new(LidarConfig::default());

        let engine = CompositeOverlayEngine::new(
            config,
            ndvi_processor,
            thermal_processor,
            lidar_processor,
        );

        assert_eq!(engine.config.overlay_types.len(), 3);
    }

    #[test]
    fn test_alpha_blending() {
        let config = CompositeConfig::default();
        let ndvi_processor = NdviProcessor::new(NdviConfig::default());
        let thermal_processor = ThermalProcessor::new(ThermalConfig::default());
        let lidar_processor = LidarOverlayProcessor::new(LidarConfig::default());

        let engine = CompositeOverlayEngine::new(
            config,
            ndvi_processor,
            thermal_processor,
            lidar_processor,
        );

        let base = Rgba([255, 0, 0, 255]);
        let overlay = Rgba([0, 255, 0, 255]);
        let blended = engine.alpha_blend(base, overlay, 0.5);

        assert_eq!(blended.0[0], 127); // 50% blend of red channel
        assert_eq!(blended.0[1], 127); // 50% blend of green channel
    }

    #[test]
    fn test_vegetation_health_score() {
        let config = CompositeConfig::default();
        let ndvi_processor = NdviProcessor::new(NdviConfig::default());
        let thermal_processor = ThermalProcessor::new(ThermalConfig::default());
        let lidar_processor = LidarOverlayProcessor::new(LidarConfig::default());

        let engine = CompositeOverlayEngine::new(
            config,
            ndvi_processor,
            thermal_processor,
            lidar_processor,
        );

        let ndvi_stats = crate::ndvi::NdviStatistics {
            mean: 0.6,
            high_vegetation_percent: 40.0,
            medium_vegetation_percent: 30.0,
            ..Default::default()
        };

        let health_score = engine.calculate_vegetation_health_score(&ndvi_stats);
        assert!(health_score > 50.0 && health_score < 90.0);
    }
}
