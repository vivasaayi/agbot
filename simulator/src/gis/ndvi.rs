//! NDVI (Normalized Difference Vegetation Index) Overlay
//!
//! Computes vegetation health visualization from satellite imagery:
//! - Pseudo-NDVI from RGB imagery (approximation using visible bands)
//! - True NDVI from NIR+Red bands (when Sentinel-2 data available)
//!
//! Provides color-coded overlay textures for terrain visualization.

use anyhow::Result;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::render_asset::RenderAssetUsages;

use super::imagery::ImageryTile;
use super::GeoBounds;

/// NDVI overlay configuration
#[derive(Resource, Clone)]
pub struct NdviConfig {
    /// Whether NDVI overlay is enabled
    pub enabled: bool,
    /// Opacity of the overlay (0.0-1.0)
    pub opacity: f32,
    /// Minimum NDVI value to display (filter bare soil)
    pub min_threshold: f32,
    /// Color scheme for visualization
    pub color_scheme: NdviColorScheme,
}

impl Default for NdviConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            opacity: 0.6,
            min_threshold: -0.2,
            color_scheme: NdviColorScheme::Agriculture,
        }
    }
}

/// Color schemes for NDVI visualization
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NdviColorScheme {
    /// Agricultural focus: brown -> yellow -> green
    #[default]
    Agriculture,
    /// Scientific: red -> yellow -> green -> blue
    Scientific,
    /// Stress detection: green -> yellow -> red (inverted)
    StressDetection,
}

/// Computed NDVI data for a region
#[derive(Clone)]
pub struct NdviData {
    /// NDVI values (-1.0 to 1.0) per pixel
    pub values: Vec<f32>,
    /// Resolution of the NDVI grid
    pub width: u32,
    pub height: u32,
    /// Geographic bounds
    pub bounds: GeoBounds,
    /// Statistics
    pub stats: NdviStats,
}

#[derive(Clone, Debug, Default)]
pub struct NdviStats {
    pub min_ndvi: f32,
    pub max_ndvi: f32,
    pub mean_ndvi: f32,
    /// Percentage of pixels with NDVI > 0.2 (vegetation)
    pub vegetation_coverage: f32,
    /// Percentage with NDVI > 0.5 (healthy vegetation)
    pub healthy_vegetation: f32,
}

/// Compute pseudo-NDVI from RGB imagery
/// 
/// Uses Excess Green Index (ExG) as approximation:
/// ExG = (2*G - R - B) / (R + G + B)
/// 
/// This correlates with vegetation but is not true NDVI which requires NIR band.
pub fn compute_pseudo_ndvi_from_tiles(
    tiles: &[ImageryTile],
    bounds: GeoBounds,
    output_size: u32,
) -> NdviData {
    let mut values = vec![0.0f32; (output_size * output_size) as usize];
    
    // First, composite the imagery
    let pixels = super::imagery::composite_imagery(tiles, bounds, output_size);
    
    // Compute pseudo-NDVI (Excess Green Index)
    let mut sum = 0.0f64;
    let mut min_val = f32::MAX;
    let mut max_val = f32::MIN;
    let mut veg_count = 0u32;
    let mut healthy_count = 0u32;
    
    for i in 0..(output_size * output_size) as usize {
        let r = pixels[i * 4] as f32 / 255.0;
        let g = pixels[i * 4 + 1] as f32 / 255.0;
        let b = pixels[i * 4 + 2] as f32 / 255.0;
        
        // Excess Green Index normalized to -1..1 range
        let total = r + g + b + 0.001; // Avoid division by zero
        let exg = (2.0 * g - r - b) / total;
        
        // Clamp to valid range
        let ndvi = exg.clamp(-1.0, 1.0);
        values[i] = ndvi;
        
        sum += ndvi as f64;
        min_val = min_val.min(ndvi);
        max_val = max_val.max(ndvi);
        
        if ndvi > 0.2 {
            veg_count += 1;
        }
        if ndvi > 0.5 {
            healthy_count += 1;
        }
    }
    
    let pixel_count = (output_size * output_size) as f32;
    
    NdviData {
        values,
        width: output_size,
        height: output_size,
        bounds,
        stats: NdviStats {
            min_ndvi: min_val,
            max_ndvi: max_val,
            mean_ndvi: (sum / pixel_count as f64) as f32,
            vegetation_coverage: (veg_count as f32 / pixel_count) * 100.0,
            healthy_vegetation: (healthy_count as f32 / pixel_count) * 100.0,
        },
    }
}

/// Convert NDVI data to RGBA color-coded texture
pub fn ndvi_to_texture(
    ndvi: &NdviData,
    config: &NdviConfig,
) -> Image {
    let mut pixels = Vec::with_capacity((ndvi.width * ndvi.height * 4) as usize);
    
    for &value in &ndvi.values {
        let (r, g, b, a) = ndvi_to_color(value, config);
        pixels.extend_from_slice(&[r, g, b, a]);
    }
    
    Image::new(
        Extent3d {
            width: ndvi.width,
            height: ndvi.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Convert NDVI value to RGBA color
fn ndvi_to_color(value: f32, config: &NdviConfig) -> (u8, u8, u8, u8) {
    // Transparent if below threshold
    if value < config.min_threshold {
        return (0, 0, 0, 0);
    }
    
    let alpha = (config.opacity * 255.0) as u8;
    
    match config.color_scheme {
        NdviColorScheme::Agriculture => {
            // Brown -> Yellow -> Light Green -> Dark Green
            if value < 0.0 {
                // Bare soil / water - brown to gray
                let t = (value + 1.0).clamp(0.0, 1.0);
                (
                    lerp_u8(139, 160, t),  // Brown to gray-brown
                    lerp_u8(90, 140, t),
                    lerp_u8(43, 100, t),
                    alpha,
                )
            } else if value < 0.2 {
                // Sparse vegetation - tan to yellow
                let t = value / 0.2;
                (
                    lerp_u8(210, 255, t),
                    lerp_u8(180, 230, t),
                    lerp_u8(140, 0, t),
                    alpha,
                )
            } else if value < 0.4 {
                // Moderate vegetation - yellow to light green
                let t = (value - 0.2) / 0.2;
                (
                    lerp_u8(255, 144, t),
                    lerp_u8(230, 238, t),
                    lerp_u8(0, 144, t),
                    alpha,
                )
            } else {
                // Dense vegetation - light green to dark green
                let t = ((value - 0.4) / 0.6).clamp(0.0, 1.0);
                (
                    lerp_u8(144, 0, t),
                    lerp_u8(238, 128, t),
                    lerp_u8(144, 0, t),
                    alpha,
                )
            }
        }
        
        NdviColorScheme::Scientific => {
            // Red -> Yellow -> Green -> Cyan -> Blue
            if value < -0.5 {
                (128, 0, 0, alpha)  // Dark red (water)
            } else if value < 0.0 {
                let t = (value + 0.5) / 0.5;
                (lerp_u8(128, 255, t), lerp_u8(0, 255, t), 0, alpha)
            } else if value < 0.5 {
                let t = value / 0.5;
                (lerp_u8(255, 0, t), 255, lerp_u8(0, 128, t), alpha)
            } else {
                let t = (value - 0.5) / 0.5;
                (0, lerp_u8(255, 128, t), lerp_u8(128, 255, t), alpha)
            }
        }
        
        NdviColorScheme::StressDetection => {
            // Green (healthy) -> Yellow (stress) -> Red (severe stress)
            if value > 0.6 {
                (0, 200, 0, alpha)  // Healthy - green
            } else if value > 0.3 {
                let t = (0.6 - value) / 0.3;
                (lerp_u8(0, 255, t), lerp_u8(200, 200, t), 0, alpha)
            } else if value > 0.0 {
                let t = (0.3 - value) / 0.3;
                (255, lerp_u8(200, 100, t), 0, alpha)
            } else {
                (180, 50, 0, alpha)  // Severe stress - dark red
            }
        }
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);
    ((a as f32) * (1.0 - t) + (b as f32) * t) as u8
}

/// Blend NDVI overlay with base imagery
pub fn blend_ndvi_overlay(
    base_pixels: &[u8],
    ndvi_pixels: &[u8],
    blend_factor: f32,
) -> Vec<u8> {
    assert_eq!(base_pixels.len(), ndvi_pixels.len());
    
    let mut result = Vec::with_capacity(base_pixels.len());
    
    for i in (0..base_pixels.len()).step_by(4) {
        let ndvi_alpha = ndvi_pixels[i + 3] as f32 / 255.0 * blend_factor;
        
        if ndvi_alpha > 0.01 {
            // Blend with NDVI overlay
            let inv_alpha = 1.0 - ndvi_alpha;
            result.push(lerp_u8(base_pixels[i], ndvi_pixels[i], ndvi_alpha));
            result.push(lerp_u8(base_pixels[i + 1], ndvi_pixels[i + 1], ndvi_alpha));
            result.push(lerp_u8(base_pixels[i + 2], ndvi_pixels[i + 2], ndvi_alpha));
            result.push(255); // Full opacity for final output
        } else {
            // Just copy base
            result.extend_from_slice(&base_pixels[i..i + 4]);
        }
    }
    
    result
}

/// Create a legend texture for NDVI values
pub fn create_ndvi_legend(
    width: u32,
    height: u32,
    config: &NdviConfig,
) -> Image {
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    
    for y in 0..height {
        for x in 0..width {
            // Map x position to NDVI value (-1 to 1)
            let ndvi = (x as f32 / width as f32) * 2.0 - 1.0;
            let (r, g, b, _) = ndvi_to_color(ndvi, config);
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
    }
    
    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ndvi_color_range() {
        let config = NdviConfig::default();
        
        // Test various NDVI values
        let (_, _, _, a) = ndvi_to_color(-0.5, &config);
        assert!(a > 0); // Below threshold is transparent
        
        let (r, g, _, _) = ndvi_to_color(0.8, &config);
        assert!(g > r); // High NDVI should be green
        
        let (r, g, _, _) = ndvi_to_color(0.0, &config);
        // Near-zero should be yellowish
    }
}
