use anyhow::Result;
use std::path::Path;

/// Compute NDWI from band data arrays
/// Returns a vector of NDWI values and raster shape (rows, cols)
/// Note: This is a simplified implementation for demonstration
pub fn compute_ndwi_from_arrays(green: &[f32], nir: &[f32], width: usize, height: usize) -> Result<(Vec<f32>, (usize, usize))> {
    if green.len() != nir.len() || green.len() != width * height {
        anyhow::bail!("Input arrays must have the same length and match dimensions");
    }
    
    let ndwi: Vec<f32> = green.iter().zip(nir.iter())
        .map(|(&g, &n)| if (g + n).abs() > 1e-6 { (g - n) / (g + n) } else { 0.0 })
        .collect();
    
    Ok((ndwi, (height, width)))
}

/// Threshold NDWI to create a binary water mask (1 = water, 0 = non-water)
pub fn threshold_ndwi(ndwi: &[f32], threshold: f32) -> Vec<u8> {
    ndwi.iter().map(|&v| if v > threshold { 1 } else { 0 }).collect()
}

/// Compute water statistics from NDWI array
pub fn compute_water_stats(ndwi: &[f32]) -> WaterStats {
    let min_ndwi = ndwi.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max_ndwi = ndwi.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    let mean_ndwi = ndwi.iter().sum::<f32>() / ndwi.len() as f32;
    
    WaterStats {
        min_ndwi,
        max_ndwi,
        mean_ndwi,
    }
}

/// Write NDWI data to a simple text format (for demonstration)
pub fn write_ndwi_text(ndwi: &[f32], shape: (usize, usize), output_path: &Path) -> Result<()> {
    let content = format!(
        "# NDWI Data\n# Shape: {}x{}\n# Values:\n{}",
        shape.1, shape.0,
        ndwi.iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
    std::fs::write(output_path, content)?;
    Ok(())
}

/// Write binary mask to text format (for demonstration)
pub fn write_mask_text(mask: &[u8], shape: (usize, usize), output_path: &Path) -> Result<()> {
    let content = format!(
        "# Water Mask\n# Shape: {}x{}\n# Values (1=water, 0=land):\n{}",
        shape.1, shape.0,
        mask.iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
    std::fs::write(output_path, content)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct WaterStats {
    pub min_ndwi: f32,
    pub max_ndwi: f32,
    pub mean_ndwi: f32,
}
