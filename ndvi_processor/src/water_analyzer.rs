use crate::analysis_core::AnalysisEngine;
use crate::analysis_schemas::*;
use crate::vectorization::*;
use ndarray::Array2;
use shared::AgroResult;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;

/// Specialized water body analysis with quality assessment
pub struct WaterAnalyzer {
    engine: AnalysisEngine,
    vectorizer: Vectorizer,
}

impl WaterAnalyzer {
    pub fn new() -> Self {
        Self {
            engine: AnalysisEngine::new(),
            vectorizer: Vectorizer::new(),
        }
    }

    /// Perform comprehensive water body analysis
    pub fn analyze_water(
        &self,
        bands: &HashMap<String, Array2<f32>>,
        source_images: Vec<Uuid>,
        output_path: String,
        historical_data: Option<&WaterAnalysisResult>,
    ) -> AgroResult<WaterAnalysisResult> {
        let start_time = std::time::Instant::now();

        // Extract required bands
        let green = bands.get("green").ok_or_else(|| 
            shared::error::AgroError::Processing("Green band required".to_string()))?;
        let nir = bands.get("nir").ok_or_else(|| 
            shared::error::AgroError::Processing("NIR band required".to_string()))?;

        // Compute water indices
        let ndwi = self.engine.compute_ndwi(green, nir)?;
        
        // Compute additional water indices if SWIR bands available
        let mut mndwi = None;
        let mut awei = None;
        
        if let Some(swir1) = bands.get("swir1") {
            mndwi = Some(self.engine.compute_mndwi(green, swir1)?);
            
            if let Some(swir2) = bands.get("swir2") {
                awei = Some(self.engine.compute_awei(green, nir, swir1, swir2)?);
            }
        }

        // Create water mask using multiple indices
        let water_mask = self.create_composite_water_mask(&ndwi, mndwi.as_ref(), awei.as_ref())?;

        // Vectorize water bodies
        let water_polygons = self.vectorizer.raster_to_polygons(&water_mask, 0.5)?;
        
        // Analyze individual water bodies
        let water_bodies = self.analyze_water_bodies(&water_polygons, &ndwi)?;
        
        // Calculate total water area
        let total_water_area_hectares: f64 = water_bodies.iter()
            .map(|wb| wb.area_hectares)
            .sum();

        // Assess water quality
        let water_quality = self.assess_water_quality(green, nir, bands.get("red"))?;

        // Perform temporal change analysis if historical data available
        let temporal_change = if let Some(historical) = historical_data {
            Some(self.analyze_temporal_change(&water_bodies, &historical.water_bodies)?)
        } else {
            None
        };

        // Calculate statistics
        let statistics = self.engine.calculate_statistics(&ndwi);

        let processing_time = start_time.elapsed().as_millis() as u64;

        // Create parameters map
        let mut parameters = HashMap::new();
        parameters.insert("ndwi_threshold".to_string(), serde_json::Value::from(0.3));
        parameters.insert("mndwi_threshold".to_string(), serde_json::Value::from(0.2));
        parameters.insert("composite_method".to_string(), serde_json::Value::from("weighted_average"));

        let bands_used = if awei.is_some() {
            vec!["green".to_string(), "nir".to_string(), "swir1".to_string(), "swir2".to_string()]
        } else if mndwi.is_some() {
            vec!["green".to_string(), "nir".to_string(), "swir1".to_string()]
        } else {
            vec!["green".to_string(), "nir".to_string()]
        };

        let analysis_result = self.engine.create_analysis_result(
            AnalysisType::Ndwi,
            source_images,
            output_path,
            statistics,
            processing_time,
            parameters,
            bands_used,
        );

        Ok(WaterAnalysisResult {
            analysis_result,
            water_bodies,
            total_water_area_hectares,
            water_quality,
            temporal_change,
        })
    }

    fn create_composite_water_mask(
        &self,
        ndwi: &Array2<f32>,
        mndwi: Option<&Array2<f32>>,
        awei: Option<&Array2<f32>>,
    ) -> AgroResult<Array2<f32>> {
        let mut water_mask = Array2::zeros(ndwi.dim());

        for ((i, j), mask_val) in water_mask.indexed_iter_mut() {
            let ndwi_val = ndwi[[i, j]];
            
            if !ndwi_val.is_finite() {
                *mask_val = 0.0;
                continue;
            }

            let mut water_score = 0.0;
            let mut total_weight = 0.0;

            // NDWI contribution (weight: 1.0)
            if ndwi_val > 0.3 {
                water_score += ndwi_val * 1.0;
                total_weight += 1.0;
            }

            // MNDWI contribution (weight: 1.2)
            if let Some(mndwi_array) = mndwi {
                let mndwi_val = mndwi_array[[i, j]];
                if mndwi_val.is_finite() && mndwi_val > 0.2 {
                    water_score += mndwi_val * 1.2;
                    total_weight += 1.2;
                }
            }

            // AWEI contribution (weight: 0.8)
            if let Some(awei_array) = awei {
                let awei_val = awei_array[[i, j]];
                if awei_val.is_finite() && awei_val > 0.0 {
                    water_score += (awei_val / 10.0).clamp(0.0, 1.0) * 0.8; // Normalize AWEI
                    total_weight += 0.8;
                }
            }

            *mask_val = if total_weight > 0.0 {
                (water_score / total_weight).clamp(0.0, 1.0)
            } else {
                0.0
            };
        }

        Ok(water_mask)
    }

    fn analyze_water_bodies(
        &self,
        polygons: &[WaterPolygon],
        ndwi: &Array2<f32>,
    ) -> AgroResult<Vec<WaterBody>> {
        let mut water_bodies = Vec::new();

        for (idx, polygon) in polygons.iter().enumerate() {
            // Calculate area and perimeter
            let area_hectares = polygon.area_m2 / 10000.0;
            let perimeter_meters = self.calculate_perimeter(&polygon.coordinates);

            // Classify water body type based on shape and size
            let water_type = self.classify_water_type(area_hectares, perimeter_meters);

            // Assess turbidity from NDWI values within the polygon
            let turbidity_level = self.assess_turbidity(ndwi, &polygon.coordinates)?;

            water_bodies.push(WaterBody {
                id: format!("water_body_{}", idx),
                coordinates: polygon.coordinates.clone(),
                area_hectares,
                perimeter_meters,
                water_type,
                turbidity_level,
            });
        }

        Ok(water_bodies)
    }

    fn calculate_perimeter(&self, coordinates: &[[f64; 2]]) -> f64 {
        if coordinates.len() < 2 {
            return 0.0;
        }

        let mut perimeter = 0.0;
        for i in 0..coordinates.len() {
            let current = coordinates[i];
            let next = coordinates[(i + 1) % coordinates.len()];
            
            // Simplified distance calculation (should use proper geographic distance)
            let dx = next[0] - current[0];
            let dy = next[1] - current[1];
            perimeter += (dx * dx + dy * dy).sqrt() * 111320.0; // Approximate conversion to meters
        }

        perimeter
    }

    fn classify_water_type(&self, area_hectares: f64, perimeter_meters: f64) -> WaterType {
        let shape_index = if perimeter_meters > 0.0 {
            4.0 * std::f64::consts::PI * area_hectares * 10000.0 / (perimeter_meters * perimeter_meters)
        } else {
            0.0
        };

        match (area_hectares, shape_index) {
            (a, _) if a > 1000.0 => WaterType::Lake,
            (a, s) if a > 100.0 && s < 0.3 => WaterType::River,
            (a, s) if a > 50.0 && s > 0.7 => WaterType::Reservoir,
            (a, _) if a > 10.0 => WaterType::Pond,
            (a, s) if a > 1.0 && s < 0.4 => WaterType::River,
            (a, _) if a > 0.1 => WaterType::Wetland,
            _ => WaterType::Unknown,
        }
    }

    fn assess_turbidity(&self, ndwi: &Array2<f32>, coordinates: &[[f64; 2]]) -> AgroResult<TurbidityLevel> {
        // Sample NDWI values within the water body polygon (simplified approach)
        let mut ndwi_values = Vec::new();
        
        // For simplicity, sample a grid of points within bounding box
        if coordinates.is_empty() {
            return Ok(TurbidityLevel::Clear);
        }

        let min_x = coordinates.iter().map(|c| c[0]).fold(f64::INFINITY, f64::min);
        let max_x = coordinates.iter().map(|c| c[0]).fold(f64::NEG_INFINITY, f64::max);
        let min_y = coordinates.iter().map(|c| c[1]).fold(f64::INFINITY, f64::min);
        let max_y = coordinates.iter().map(|c| c[1]).fold(f64::NEG_INFINITY, f64::max);

        let (rows, cols) = ndwi.dim();
        
        for i in 0..rows {
            for j in 0..cols {
                // Convert array indices to coordinates (simplified)
                let x = (j as f64 / cols as f64) * (max_x - min_x) + min_x;
                let y = (i as f64 / rows as f64) * (max_y - min_y) + min_y;
                
                // Check if point is roughly within bounding box
                if x >= min_x && x <= max_x && y >= min_y && y <= max_y {
                    let ndwi_val = ndwi[[i, j]];
                    if ndwi_val.is_finite() && ndwi_val > 0.3 {
                        ndwi_values.push(ndwi_val);
                    }
                }
            }
        }

        if ndwi_values.is_empty() {
            return Ok(TurbidityLevel::Clear);
        }

        let mean_ndwi = ndwi_values.iter().sum::<f32>() / ndwi_values.len() as f32;
        let std_dev = {
            let variance = ndwi_values.iter()
                .map(|&x| (x - mean_ndwi).powi(2))
                .sum::<f32>() / ndwi_values.len() as f32;
            variance.sqrt()
        };

        // Turbidity assessment based on NDWI characteristics
        Ok(match (mean_ndwi, std_dev) {
            (m, _) if m > 0.8 => TurbidityLevel::Clear,
            (m, s) if m > 0.6 && s < 0.1 => TurbidityLevel::SlightlyTurbid,
            (m, s) if m > 0.4 || s > 0.15 => TurbidityLevel::Turbid,
            _ => TurbidityLevel::HighlyTurbid,
        })
    }

    fn assess_water_quality(
        &self,
        green: &Array2<f32>,
        nir: &Array2<f32>,
        red: Option<&Array2<f32>>,
    ) -> AgroResult<WaterQuality> {
        // Simplified water quality assessment
        let ndwi = self.engine.compute_ndwi(green, nir)?;
        let ndwi_stats = self.engine.calculate_statistics(&ndwi);

        // Estimate chlorophyll concentration from red/green ratio (if available)
        let chlorophyll_concentration = if let Some(red_band) = red {
            let mut chl_sum = 0.0;
            let mut valid_pixels = 0;

            for ((i, j), &ndwi_val) in ndwi.indexed_iter() {
                if ndwi_val > 0.3 { // Water pixels only
                    let green_val = green[[i, j]];
                    let red_val = red_band[[i, j]];
                    
                    if green_val.is_finite() && red_val.is_finite() && red_val > 0.0 {
                        // Simplified chlorophyll estimation
                        let ratio = green_val / red_val;
                        chl_sum += ratio * 10.0; // Rough conversion to μg/L
                        valid_pixels += 1;
                    }
                }
            }

            if valid_pixels > 0 {
                Some(chl_sum / valid_pixels as f32)
            } else {
                None
            }
        } else {
            None
        };

        // Assess algae presence based on chlorophyll
        let algae_presence = if let Some(chl) = chlorophyll_concentration {
            match chl {
                c if c > 100.0 => AlgaeLevel::Bloom,
                c if c > 50.0 => AlgaeLevel::High,
                c if c > 20.0 => AlgaeLevel::Moderate,
                c if c > 5.0 => AlgaeLevel::Low,
                _ => AlgaeLevel::None,
            }
        } else {
            AlgaeLevel::None
        };

        // Overall water quality assessment
        let overall_quality = match (ndwi_stats.mean, algae_presence) {
            (m, AlgaeLevel::Bloom) if m > 0.7 => WaterQualityLevel::Poor,
            (m, AlgaeLevel::High) if m > 0.6 => WaterQualityLevel::Moderate,
            (m, _) if m > 0.8 => WaterQualityLevel::Excellent,
            (m, _) if m > 0.6 => WaterQualityLevel::Good,
            (m, _) if m > 0.4 => WaterQualityLevel::Moderate,
            _ => WaterQualityLevel::Poor,
        };

        // Simple pollution indicators
        let mut pollution_indicators = Vec::new();
        if let Some(chl) = chlorophyll_concentration {
            if chl > 30.0 {
                pollution_indicators.push(PollutionIndicator {
                    indicator_type: "Eutrophication".to_string(),
                    level: chl,
                    threshold_exceeded: true,
                    source_likely: Some("Agricultural runoff".to_string()),
                });
            }
        }

        Ok(WaterQuality {
            overall_quality,
            chlorophyll_concentration,
            turbidity_ntu: None, // Would need specific calibration
            algae_presence,
            pollution_indicators,
        })
    }

    fn analyze_temporal_change(
        &self,
        current_bodies: &[WaterBody],
        historical_bodies: &[WaterBody],
    ) -> AgroResult<TemporalChange> {
        let current_total_area: f64 = current_bodies.iter().map(|wb| wb.area_hectares).sum();
        let historical_total_area: f64 = historical_bodies.iter().map(|wb| wb.area_hectares).sum();

        let change_percentage = if historical_total_area > 0.0 {
            ((current_total_area - historical_total_area) / historical_total_area) * 100.0
        } else {
            0.0
        };

        let change_type = match change_percentage {
            p if p > 5.0 => ChangeType::Increase,
            p if p < -5.0 => ChangeType::Decrease,
            p if p.abs() <= 5.0 => ChangeType::Stable,
            _ => ChangeType::Fluctuating,
        };

        let trend = match change_percentage {
            p if p > 10.0 => Trend::Increasing,
            p if p < -10.0 => Trend::Decreasing,
            _ => Trend::Stable,
        };

        // Simple anomaly detection based on change magnitude
        let anomaly_detected = change_percentage.abs() > 25.0;

        Ok(TemporalChange {
            change_percentage: change_percentage as f32,
            change_type,
            trend,
            seasonality: false, // Would require longer time series
            anomaly_detected,
        })
    }
}

impl Default for WaterAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
