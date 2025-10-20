use anyhow::Result;
use std::path::Path;

/// Comprehensive vegetation analysis module
/// Includes multiple vegetation indices and health assessments

/// NDVI - Normalized Difference Vegetation Index
/// Formula: (NIR - Red) / (NIR + Red)
/// Range: -1 to 1, higher values indicate healthier vegetation
pub fn compute_ndvi(red: &[f32], nir: &[f32]) -> Result<Vec<f32>> {
    if red.len() != nir.len() {
        anyhow::bail!("Red and NIR arrays must have the same length");
    }
    
    let ndvi: Vec<f32> = red.iter().zip(nir.iter())
        .map(|(&r, &n)| if (n + r).abs() > 1e-6 { (n - r) / (n + r) } else { 0.0 })
        .collect();
    
    Ok(ndvi)
}

/// EVI - Enhanced Vegetation Index (improved NDVI with atmospheric correction)
/// Formula: G * ((NIR - Red) / (NIR + C1*Red - C2*Blue + L))
/// More sensitive to canopy structural variations
pub fn compute_evi(red: &[f32], nir: &[f32], blue: &[f32]) -> Result<Vec<f32>> {
    if red.len() != nir.len() || red.len() != blue.len() {
        anyhow::bail!("All band arrays must have the same length");
    }
    
    let g = 2.5;     // Gain factor
    let c1 = 6.0;    // Coefficient for aerosol resistance
    let c2 = 7.5;    // Coefficient for aerosol resistance  
    let l = 1.0;     // Canopy background adjustment
    
    let evi: Vec<f32> = red.iter().zip(nir.iter()).zip(blue.iter())
        .map(|((&r, &n), &b)| {
            let denominator = n + c1 * r - c2 * b + l;
            if denominator.abs() > 1e-6 {
                g * (n - r) / denominator
            } else {
                0.0
            }
        })
        .collect();
    
    Ok(evi)
}

/// SAVI - Soil Adjusted Vegetation Index
/// Formula: ((NIR - Red) / (NIR + Red + L)) * (1 + L)
/// Minimizes soil background effects
pub fn compute_savi(red: &[f32], nir: &[f32], l_factor: f32) -> Result<Vec<f32>> {
    if red.len() != nir.len() {
        anyhow::bail!("Red and NIR arrays must have the same length");
    }
    
    let savi: Vec<f32> = red.iter().zip(nir.iter())
        .map(|(&r, &n)| {
            let denominator = n + r + l_factor;
            if denominator.abs() > 1e-6 {
                ((n - r) / denominator) * (1.0 + l_factor)
            } else {
                0.0
            }
        })
        .collect();
    
    Ok(savi)
}

/// ARVI - Atmospherically Resistant Vegetation Index
/// More resistant to atmospheric effects than NDVI
pub fn compute_arvi(red: &[f32], nir: &[f32], blue: &[f32]) -> Result<Vec<f32>> {
    if red.len() != nir.len() || red.len() != blue.len() {
        anyhow::bail!("All band arrays must have the same length");
    }
    
    let arvi: Vec<f32> = red.iter().zip(nir.iter()).zip(blue.iter())
        .map(|((&r, &n), &b)| {
            let rb = r - 2.0 * (r - b);
            let denominator = n + rb;
            if denominator.abs() > 1e-6 {
                (n - rb) / denominator
            } else {
                0.0
            }
        })
        .collect();
    
    Ok(arvi)
}

/// MSAVI - Modified Soil Adjusted Vegetation Index
/// Self-adjusting L factor to minimize soil background
pub fn compute_msavi(red: &[f32], nir: &[f32]) -> Result<Vec<f32>> {
    if red.len() != nir.len() {
        anyhow::bail!("Red and NIR arrays must have the same length");
    }
    
    let msavi: Vec<f32> = red.iter().zip(nir.iter())
        .map(|(&r, &n)| {
            let term = 2.0 * n + 1.0;
            let sqrt_term = (term * term - 8.0 * (n - r)).max(0.0).sqrt();
            (term - sqrt_term) / 2.0
        })
        .collect();
    
    Ok(msavi)
}

/// CVI - Chlorophyll Vegetation Index
/// Estimates chlorophyll content
pub fn compute_cvi(red: &[f32], green: &[f32], nir: &[f32]) -> Result<Vec<f32>> {
    if red.len() != nir.len() || red.len() != green.len() {
        anyhow::bail!("All band arrays must have the same length");
    }
    
    let cvi: Vec<f32> = red.iter().zip(green.iter()).zip(nir.iter())
        .map(|((&r, &g), &n)| {
            if g.abs() > 1e-6 {
                (n / g) * (r / g)
            } else {
                0.0
            }
        })
        .collect();
    
    Ok(cvi)
}

/// LAI - Leaf Area Index estimation from NDVI
/// Empirical relationship to estimate leaf area
pub fn estimate_lai_from_ndvi(ndvi: &[f32]) -> Vec<f32> {
    ndvi.iter()
        .map(|&nd| {
            if nd > 0.0 {
                // Empirical formula: LAI = 3.618 * EVI - 0.118
                // Using NDVI approximation
                (3.618 * nd - 0.118).max(0.0)
            } else {
                0.0
            }
        })
        .collect()
}

/// fCover - Fractional vegetation cover
/// Estimates the fraction of ground covered by vegetation
pub fn compute_fractional_cover(ndvi: &[f32]) -> Vec<f32> {
    let ndvi_soil = 0.2;  // NDVI of bare soil
    let ndvi_veg = 0.8;   // NDVI of full vegetation
    
    ndvi.iter()
        .map(|&nd| {
            if nd <= ndvi_soil {
                0.0
            } else if nd >= ndvi_veg {
                1.0
            } else {
                ((nd - ndvi_soil) / (ndvi_veg - ndvi_soil)).powi(2)
            }
        })
        .collect()
}

/// Vegetation health classification
#[derive(Debug, Clone, PartialEq)]
pub enum VegetationHealth {
    NoVegetation,
    Stressed,
    Moderate,
    Healthy,
    VeryHealthy,
}

pub fn classify_vegetation_health(ndvi: &[f32]) -> Vec<VegetationHealth> {
    ndvi.iter()
        .map(|&nd| {
            if nd < 0.1 {
                VegetationHealth::NoVegetation
            } else if nd < 0.3 {
                VegetationHealth::Stressed
            } else if nd < 0.5 {
                VegetationHealth::Moderate
            } else if nd < 0.7 {
                VegetationHealth::Healthy
            } else {
                VegetationHealth::VeryHealthy
            }
        })
        .collect()
}

/// Compute vegetation statistics
#[derive(Debug, Clone)]
pub struct VegetationStats {
    pub mean_ndvi: f32,
    pub std_ndvi: f32,
    pub vegetation_cover_percent: f32,
    pub healthy_vegetation_percent: f32,
    pub stressed_vegetation_percent: f32,
    pub mean_lai: f32,
}

pub fn compute_vegetation_statistics(ndvi: &[f32]) -> VegetationStats {
    let health_classes = classify_vegetation_health(ndvi);
    let lai = estimate_lai_from_ndvi(ndvi);
    let fcover = compute_fractional_cover(ndvi);
    
    let mean_ndvi = ndvi.iter().sum::<f32>() / ndvi.len() as f32;
    let variance = ndvi.iter()
        .map(|&x| (x - mean_ndvi).powi(2))
        .sum::<f32>() / ndvi.len() as f32;
    let std_ndvi = variance.sqrt();
    
    let vegetation_pixels = health_classes.iter()
        .filter(|&&ref h| *h != VegetationHealth::NoVegetation)
        .count();
    let vegetation_cover_percent = (vegetation_pixels as f32 / ndvi.len() as f32) * 100.0;
    
    let healthy_pixels = health_classes.iter()
        .filter(|&&ref h| matches!(*h, VegetationHealth::Healthy | VegetationHealth::VeryHealthy))
        .count();
    let healthy_vegetation_percent = (healthy_pixels as f32 / ndvi.len() as f32) * 100.0;
    
    let stressed_pixels = health_classes.iter()
        .filter(|&&ref h| *h == VegetationHealth::Stressed)
        .count();
    let stressed_vegetation_percent = (stressed_pixels as f32 / ndvi.len() as f32) * 100.0;
    
    let mean_lai = lai.iter().sum::<f32>() / lai.len() as f32;
    
    VegetationStats {
        mean_ndvi,
        std_ndvi,
        vegetation_cover_percent,
        healthy_vegetation_percent,
        stressed_vegetation_percent,
        mean_lai,
    }
}
