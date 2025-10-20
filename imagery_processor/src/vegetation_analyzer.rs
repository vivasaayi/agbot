use crate::analysis_core::AnalysisEngine;
use crate::analysis_schemas::*;
use ndarray::Array2;
use shared::AgroResult;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;

/// Specialized vegetation analysis with comprehensive metrics
pub struct VegetationAnalyzer {
    engine: AnalysisEngine,
}

impl VegetationAnalyzer {
    pub fn new() -> Self {
        Self {
            engine: AnalysisEngine::new(),
        }
    }

    /// Perform comprehensive vegetation analysis
    pub fn analyze_vegetation(
        &self,
        bands: &HashMap<String, Array2<f32>>,
        source_images: Vec<Uuid>,
        output_path: String,
    ) -> AgroResult<VegetationAnalysisResult> {
        let start_time = std::time::Instant::now();

        // Extract required bands
        let nir = bands.get("nir").ok_or_else(|| 
            shared::error::AgroError::Processing("NIR band required".to_string()))?;
        let red = bands.get("red").ok_or_else(|| 
            shared::error::AgroError::Processing("Red band required".to_string()))?;
        let green = bands.get("green").ok_or_else(|| 
            shared::error::AgroError::Processing("Green band required".to_string()))?;
        let blue = bands.get("blue").ok_or_else(|| 
            shared::error::AgroError::Processing("Blue band required".to_string()))?;

        // Compute primary vegetation indices
        let ndvi = self.engine.compute_ndvi(nir, red)?;
        let evi = self.engine.compute_evi(nir, red, blue)?;
        let savi = self.engine.compute_savi(nir, red, 0.5)?;
        let arvi = self.engine.compute_arvi(nir, red, blue, 1.0)?;
        let msavi = self.engine.compute_msavi(nir, red)?;
        let cvi = self.engine.compute_cvi(nir, red, green)?;

        // Compute biophysical parameters
        let lai = self.engine.compute_lai(&evi)?;
        let fcover = self.engine.compute_fcover(&ndvi, 0.05, 0.95)?;

        // Calculate statistics for primary index (NDVI)
        let statistics = self.engine.calculate_statistics(&ndvi);

        // Classify vegetation health
        let health_class = self.engine.classify_vegetation_health(&ndvi);
        let health_classification = self.analyze_health_classification(&health_class);

        // Estimate biomass
        let biomass_estimate = self.estimate_biomass(&ndvi, &lai)?;

        // Analyze phenology
        let phenology = self.analyze_phenology(&ndvi, &evi)?;

        // Detect stress indicators
        let stress_indicators = self.detect_stress(&ndvi, &evi, &lai)?;

        let processing_time = start_time.elapsed().as_millis() as u64;

        // Create parameters map
        let mut parameters = HashMap::new();
        parameters.insert("savi_l_factor".to_string(), serde_json::Value::from(0.5));
        parameters.insert("arvi_gamma".to_string(), serde_json::Value::from(1.0));
        parameters.insert("fcover_soil_ndvi".to_string(), serde_json::Value::from(0.05));
        parameters.insert("fcover_veg_ndvi".to_string(), serde_json::Value::from(0.95));

        let analysis_result = self.engine.create_analysis_result(
            AnalysisType::Ndvi,
            source_images,
            output_path,
            statistics,
            processing_time,
            parameters,
            vec!["nir".to_string(), "red".to_string(), "green".to_string(), "blue".to_string()],
        );

        Ok(VegetationAnalysisResult {
            analysis_result,
            health_classification,
            biomass_estimate,
            phenology,
            stress_indicators,
        })
    }

    fn analyze_health_classification(&self, health_map: &Array2<HealthStatus>) -> HealthClassification {
        let mut health_counts = HashMap::new();
        let total_pixels = (health_map.dim().0 * health_map.dim().1) as f32;

        // Count each health status
        for health in health_map.iter() {
            *health_counts.entry(*health).or_insert(0.0) += 1.0;
        }

        // Convert to percentages
        let health_distribution: HashMap<HealthStatus, f32> = health_counts
            .into_iter()
            .map(|(status, count)| (status, (count / total_pixels) * 100.0))
            .collect();

        // Determine overall health
        let overall_health = if health_distribution.get(&HealthStatus::Excellent).unwrap_or(&0.0) > &30.0 {
            HealthStatus::Excellent
        } else if health_distribution.get(&HealthStatus::Good).unwrap_or(&0.0) > &25.0 {
            HealthStatus::Good
        } else if health_distribution.get(&HealthStatus::Moderate).unwrap_or(&0.0) > &20.0 {
            HealthStatus::Moderate
        } else if health_distribution.get(&HealthStatus::Poor).unwrap_or(&0.0) > &15.0 {
            HealthStatus::Poor
        } else {
            HealthStatus::Critical
        };

        // Identify degraded areas
        let degraded_areas = self.identify_degraded_areas(health_map);

        HealthClassification {
            overall_health,
            health_distribution,
            degraded_areas,
        }
    }

    fn identify_degraded_areas(&self, health_map: &Array2<HealthStatus>) -> Vec<DegradedArea> {
        let mut degraded_areas = Vec::new();
        let (rows, cols) = health_map.dim();

        // Simple connected component analysis for degraded areas
        let mut visited = Array2::from_elem((rows, cols), false);

        for i in 0..rows {
            for j in 0..cols {
                if !visited[[i, j]] && 
                   (health_map[[i, j]] == HealthStatus::Poor || health_map[[i, j]] == HealthStatus::Critical) {
                    
                    let mut area_pixels = Vec::new();
                    let severity = health_map[[i, j]].clone();
                    
                    // Simple flood fill to find connected degraded pixels
                    self.flood_fill(health_map, &mut visited, i, j, &severity, &mut area_pixels);
                    
                    if area_pixels.len() > 100 { // Minimum area threshold
                        // Convert pixels to coordinates (simplified)
                        let coordinates: Vec<[f64; 2]> = area_pixels.iter()
                            .map(|&(row, col)| [col as f64 * 0.00001, row as f64 * 0.00001])
                            .collect();
                        
                        let area_hectares = area_pixels.len() as f64 * 0.01; // Approximate conversion
                        
                        degraded_areas.push(DegradedArea {
                            area_id: format!("degraded_{}", degraded_areas.len()),
                            coordinates,
                            area_hectares,
                            severity,
                            likely_cause: Some("Stress or disease detected".to_string()),
                        });
                    }
                }
            }
        }

        degraded_areas
    }

    fn flood_fill(
        &self,
        health_map: &Array2<HealthStatus>,
        visited: &mut Array2<bool>,
        row: usize,
        col: usize,
        target_severity: &HealthStatus,
        area_pixels: &mut Vec<(usize, usize)>,
    ) {
        let (rows, cols) = health_map.dim();
        
        if row >= rows || col >= cols || visited[[row, col]] || 
           health_map[[row, col]] != *target_severity {
            return;
        }

        visited[[row, col]] = true;
        area_pixels.push((row, col));

        // Check 4-connected neighbors
        if row > 0 { self.flood_fill(health_map, visited, row - 1, col, target_severity, area_pixels); }
        if row + 1 < rows { self.flood_fill(health_map, visited, row + 1, col, target_severity, area_pixels); }
        if col > 0 { self.flood_fill(health_map, visited, row, col - 1, target_severity, area_pixels); }
        if col + 1 < cols { self.flood_fill(health_map, visited, row, col + 1, target_severity, area_pixels); }
    }

    fn estimate_biomass(&self, ndvi: &Array2<f32>, lai: &Array2<f32>) -> AgroResult<BiomassEstimate> {
        let mut total_biomass = 0.0;
        let mut valid_pixels = 0;
        let pixel_area = 100.0; // m² per pixel (10m resolution)

        for ((i, j), &ndvi_val) in ndvi.indexed_iter() {
            if ndvi_val.is_finite() && ndvi_val > 0.1 {
                let lai_val = lai[[i, j]];
                if lai_val.is_finite() {
                    // Empirical biomass estimation (tons/hectare)
                    // Biomass = 0.0673 * LAI^1.5 (simplified allometric relationship)
                    let biomass_density = 0.0673 * lai_val.powf(1.5);
                    total_biomass += biomass_density * (pixel_area / 10000.0); // Convert to hectares
                    valid_pixels += 1;
                }
            }
        }

        let total_area_hectares = (valid_pixels as f64 * pixel_area as f64) / 10000.0;
        let biomass_density_tons_per_hectare = if total_area_hectares > 0.0 {
            total_biomass as f64 / total_area_hectares
        } else {
            0.0
        };

        // Carbon stock estimation (approximately 47% of biomass)
        let carbon_stock_tons = total_biomass as f64 * 0.47;

        // Simple confidence interval (±20%)
        let confidence_interval = (total_biomass as f64 * 0.8, total_biomass as f64 * 1.2);

        Ok(BiomassEstimate {
            total_biomass_tons: total_biomass as f64,
            biomass_density_tons_per_hectare,
            carbon_stock_tons,
            confidence_interval,
            estimation_method: "LAI-based allometric equation".to_string(),
        })
    }

    fn analyze_phenology(&self, ndvi: &Array2<f32>, evi: &Array2<f32>) -> AgroResult<PhenologyMetrics> {
        // Calculate mean NDVI and EVI for phenology assessment
        let ndvi_stats = self.engine.calculate_statistics(ndvi);
        let evi_stats = self.engine.calculate_statistics(evi);

        // Simple phenology classification based on vegetation indices
        let growth_stage = if ndvi_stats.mean > 0.7 && evi_stats.mean > 0.4 {
            GrowthStage::Maturity
        } else if ndvi_stats.mean > 0.5 && evi_stats.mean > 0.3 {
            GrowthStage::Flowering
        } else if ndvi_stats.mean > 0.3 && evi_stats.mean > 0.2 {
            GrowthStage::Vegetative
        } else if ndvi_stats.mean > 0.1 && evi_stats.mean > 0.1 {
            GrowthStage::Emergence
        } else if ndvi_stats.mean < 0.2 {
            GrowthStage::Dormant
        } else {
            GrowthStage::Unknown
        };

        Ok(PhenologyMetrics {
            growth_stage,
            days_since_planting: None, // Would require temporal data
            days_to_harvest: None, // Would require crop type and temporal data
            peak_green_date: None, // Would require time series
            senescence_start: None, // Would require time series
        })
    }

    fn detect_stress(&self, ndvi: &Array2<f32>, evi: &Array2<f32>, lai: &Array2<f32>) -> AgroResult<StressIndicators> {
        let ndvi_stats = self.engine.calculate_statistics(ndvi);
        let evi_stats = self.engine.calculate_statistics(evi);
        let lai_stats = self.engine.calculate_statistics(lai);

        // Simple stress detection based on index values and variability
        let water_stress = if ndvi_stats.mean < 0.3 || evi_stats.mean < 0.2 {
            StressLevel::High
        } else if ndvi_stats.mean < 0.5 || evi_stats.mean < 0.3 {
            StressLevel::Moderate
        } else if ndvi_stats.std_dev > 0.2 {
            StressLevel::Low
        } else {
            StressLevel::None
        };

        let nutrient_stress = if lai_stats.mean < 1.0 && ndvi_stats.mean < 0.4 {
            StressLevel::High
        } else if lai_stats.mean < 2.0 && ndvi_stats.mean < 0.6 {
            StressLevel::Moderate
        } else {
            StressLevel::Low
        };

        let disease_pressure = if ndvi_stats.std_dev > 0.25 && evi_stats.std_dev > 0.15 {
            StressLevel::Moderate
        } else if ndvi_stats.std_dev > 0.3 {
            StressLevel::High
        } else {
            StressLevel::Low
        };

        let heat_stress = if ndvi_stats.mean < 0.35 && evi_stats.mean < 0.25 {
            StressLevel::Moderate
        } else {
            StressLevel::Low
        };

        let mut recommendations = Vec::new();
        
        match water_stress {
            StressLevel::High => recommendations.push("Increase irrigation frequency".to_string()),
            StressLevel::Moderate => recommendations.push("Monitor soil moisture levels".to_string()),
            _ => {}
        }

        match nutrient_stress {
            StressLevel::High => recommendations.push("Apply nitrogen fertilizer".to_string()),
            StressLevel::Moderate => recommendations.push("Conduct soil nutrient analysis".to_string()),
            _ => {}
        }

        if disease_pressure == StressLevel::High {
            recommendations.push("Inspect for disease symptoms and consider treatment".to_string());
        }

        Ok(StressIndicators {
            water_stress,
            nutrient_stress,
            disease_pressure,
            heat_stress,
            recommendations,
        })
    }
}

impl Default for VegetationAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
