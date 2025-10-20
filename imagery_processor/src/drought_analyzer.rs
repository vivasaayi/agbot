use crate::analysis_core::AnalysisEngine;
use crate::analysis_schemas::*;
use ndarray::Array2;
use shared::AgroResult;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;

/// Specialized drought analysis and monitoring
pub struct DroughtAnalyzer {
    engine: AnalysisEngine,
}

impl DroughtAnalyzer {
    pub fn new() -> Self {
        Self {
            engine: AnalysisEngine::new(),
        }
    }

    /// Perform comprehensive drought analysis
    pub fn analyze_drought(
        &self,
        bands: &HashMap<String, Array2<f32>>,
        temperature_data: Option<&Array2<f32>>,
        precipitation_data: Option<&Array2<f32>>,
        source_images: Vec<Uuid>,
        output_path: String,
        historical_data: Option<&[DroughtAnalysisResult]>,
    ) -> AgroResult<DroughtAnalysisResult> {
        let start_time = std::time::Instant::now();

        // Extract required bands
        let nir = bands.get("nir").ok_or_else(|| 
            shared::error::AgroError::Processing("NIR band required".to_string()))?;
        let red = bands.get("red").ok_or_else(|| 
            shared::error::AgroError::Processing("Red band required".to_string()))?;
        let green = bands.get("green").ok_or_else(|| 
            shared::error::AgroError::Processing("Green band required".to_string()))?;

        // Compute vegetation indices for drought assessment
        let ndvi = self.engine.compute_ndvi(nir, red)?;
        let ndwi = self.engine.compute_ndwi(green, nir)?;

        // Compute Vegetation Health Index (VHI)
        let vhi = self.compute_vhi(&ndvi, temperature_data)?;

        // Compute Palmer Drought Index approximation
        let pdi = self.compute_pdi(&ndvi, &ndwi, temperature_data, precipitation_data)?;

        // Assess drought severity
        let drought_severity = self.assess_drought_severity(&vhi, &pdi)?;

        // Calculate affected area
        let affected_area_hectares = self.calculate_affected_area(&drought_severity)?;

        // Estimate drought duration (requires historical data)
        let drought_duration_days = self.estimate_drought_duration(historical_data);

        // Calculate recovery probability
        let recovery_probability = self.calculate_recovery_probability(&drought_severity, &ndvi)?;

        // Assess impact
        let impact_assessment = self.assess_impact(&drought_severity, &ndvi, affected_area_hectares)?;

        // Calculate statistics on VHI as primary drought index
        let statistics = self.engine.calculate_statistics(&vhi);

        let processing_time = start_time.elapsed().as_millis() as u64;

        // Create parameters map
        let mut parameters = HashMap::new();
        parameters.insert("vhi_method".to_string(), serde_json::Value::from("temperature_conditioned"));
        parameters.insert("drought_threshold".to_string(), serde_json::Value::from(40.0));
        parameters.insert("severe_threshold".to_string(), serde_json::Value::from(20.0));

        let analysis_result = self.engine.create_analysis_result(
            AnalysisType::Vhi,
            source_images,
            output_path,
            statistics,
            processing_time,
            parameters,
            vec!["nir".to_string(), "red".to_string(), "green".to_string()],
        );

        Ok(DroughtAnalysisResult {
            analysis_result,
            drought_severity: self.classify_overall_drought_severity(&drought_severity),
            affected_area_hectares,
            drought_duration_days,
            recovery_probability,
            impact_assessment,
        })
    }

    /// Compute Vegetation Health Index (VHI)
    /// VHI = α × VCI + (1-α) × TCI, where α = 0.5
    fn compute_vhi(&self, ndvi: &Array2<f32>, temperature: Option<&Array2<f32>>) -> AgroResult<Array2<f32>> {
        let mut vhi = Array2::zeros(ndvi.dim());
        
        // Compute VCI (Vegetation Condition Index)
        let vci = self.compute_vci(ndvi)?;

        if let Some(temp_data) = temperature {
            // Compute TCI (Temperature Condition Index)
            let tci = self.compute_tci(temp_data)?;
            
            // Combine VCI and TCI
            for ((i, j), vhi_val) in vhi.indexed_iter_mut() {
                let vci_val = vci[[i, j]];
                let tci_val = tci[[i, j]];
                
                if vci_val.is_finite() && tci_val.is_finite() {
                    *vhi_val = 0.5 * vci_val + 0.5 * tci_val;
                } else if vci_val.is_finite() {
                    *vhi_val = vci_val; // Use only VCI if TCI not available
                } else {
                    *vhi_val = f32::NAN;
                }
            }
        } else {
            // Use only VCI if temperature data not available
            vhi = vci;
        }

        Ok(vhi)
    }

    /// Compute Vegetation Condition Index (VCI)
    /// VCI = ((NDVI - NDVImin) / (NDVImax - NDVImin)) × 100
    fn compute_vci(&self, ndvi: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let ndvi_stats = self.engine.calculate_statistics(ndvi);
        let ndvi_min = ndvi_stats.min;
        let ndvi_max = ndvi_stats.max;
        
        if (ndvi_max - ndvi_min).abs() < 1e-8 {
            return Err(shared::error::AgroError::Processing(
                "Insufficient NDVI variation for VCI calculation".to_string()
            ));
        }

        let mut vci = Array2::zeros(ndvi.dim());
        
        for ((i, j), vci_val) in vci.indexed_iter_mut() {
            let ndvi_val = ndvi[[i, j]];
            
            if ndvi_val.is_finite() {
                *vci_val = ((ndvi_val - ndvi_min) / (ndvi_max - ndvi_min)) * 100.0;
            } else {
                *vci_val = f32::NAN;
            }
        }

        Ok(vci)
    }

    /// Compute Temperature Condition Index (TCI)
    /// TCI = ((Tmax - T) / (Tmax - Tmin)) × 100
    fn compute_tci(&self, temperature: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut temp_values = Vec::new();
        
        // Collect valid temperature values
        for &temp in temperature.iter() {
            if temp.is_finite() {
                temp_values.push(temp);
            }
        }

        if temp_values.is_empty() {
            return Err(shared::error::AgroError::Processing(
                "No valid temperature data for TCI calculation".to_string()
            ));
        }

        temp_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let temp_min = temp_values[0];
        let temp_max = temp_values[temp_values.len() - 1];

        if (temp_max - temp_min).abs() < 1e-8 {
            return Err(shared::error::AgroError::Processing(
                "Insufficient temperature variation for TCI calculation".to_string()
            ));
        }

        let mut tci = Array2::zeros(temperature.dim());
        
        for ((i, j), tci_val) in tci.indexed_iter_mut() {
            let temp_val = temperature[[i, j]];
            
            if temp_val.is_finite() {
                *tci_val = ((temp_max - temp_val) / (temp_max - temp_min)) * 100.0;
            } else {
                *tci_val = f32::NAN;
            }
        }

        Ok(tci)
    }

    /// Compute simplified Palmer Drought Index (PDI)
    fn compute_pdi(
        &self,
        ndvi: &Array2<f32>,
        ndwi: &Array2<f32>,
        temperature: Option<&Array2<f32>>,
        precipitation: Option<&Array2<f32>>,
    ) -> AgroResult<Array2<f32>> {
        let mut pdi = Array2::zeros(ndvi.dim());

        for ((i, j), pdi_val) in pdi.indexed_iter_mut() {
            let ndvi_val = ndvi[[i, j]];
            let ndwi_val = ndwi[[i, j]];
            
            if !ndvi_val.is_finite() || !ndwi_val.is_finite() {
                *pdi_val = f32::NAN;
                continue;
            }

            // Simplified PDI based on vegetation and water indices
            let mut pdi_score = 0.0;
            
            // NDVI contribution (drought reduces NDVI)
            pdi_score += (ndvi_val - 0.5) * 2.0; // Normalized around 0.5
            
            // NDWI contribution (drought reduces soil moisture)
            pdi_score += (ndwi_val + 0.2) * 1.5; // Adjusted for typical NDWI range

            // Temperature contribution if available (high temp increases drought)
            if let Some(temp_data) = temperature {
                let temp_val = temp_data[[i, j]];
                if temp_val.is_finite() {
                    // Assume temperature in Celsius, normalize around 25°C
                    pdi_score -= (temp_val - 25.0) * 0.1;
                }
            }

            // Precipitation contribution if available (more precip reduces drought)
            if let Some(precip_data) = precipitation {
                let precip_val = precip_data[[i, j]];
                if precip_val.is_finite() {
                    pdi_score += precip_val * 0.01; // Scale precipitation appropriately
                }
            }

            *pdi_val = pdi_score.clamp(-4.0, 4.0); // Standard PDI range
        }

        Ok(pdi)
    }

    /// Assess drought severity based on VHI and PDI
    fn assess_drought_severity(&self, vhi: &Array2<f32>, pdi: &Array2<f32>) -> AgroResult<Array2<DroughtSeverity>> {
        let mut severity_map = Array2::from_elem(vhi.dim(), DroughtSeverity::None);

        for ((i, j), severity_val) in severity_map.indexed_iter_mut() {
            let vhi_val = vhi[[i, j]];
            let pdi_val = pdi[[i, j]];

            if !vhi_val.is_finite() {
                continue;
            }

            // Primary classification based on VHI
            let vhi_severity = match vhi_val {
                v if v >= 50.0 => DroughtSeverity::None,
                v if v >= 40.0 => DroughtSeverity::Mild,
                v if v >= 30.0 => DroughtSeverity::Moderate,
                v if v >= 20.0 => DroughtSeverity::Severe,
                _ => DroughtSeverity::Extreme,
            };

            // Adjust based on PDI if available
            let final_severity = if pdi_val.is_finite() {
                match (vhi_severity, pdi_val) {
                    (DroughtSeverity::None, p) if p < -1.0 => DroughtSeverity::Mild,
                    (DroughtSeverity::Mild, p) if p < -2.0 => DroughtSeverity::Moderate,
                    (DroughtSeverity::Moderate, p) if p < -3.0 => DroughtSeverity::Severe,
                    (DroughtSeverity::Severe, p) if p < -3.5 => DroughtSeverity::Extreme,
                    (s, p) if p > 1.0 => {
                        // PDI indicates wetter conditions, reduce severity
                        match s {
                            DroughtSeverity::Extreme => DroughtSeverity::Severe,
                            DroughtSeverity::Severe => DroughtSeverity::Moderate,
                            DroughtSeverity::Moderate => DroughtSeverity::Mild,
                            DroughtSeverity::Mild => DroughtSeverity::None,
                            DroughtSeverity::None => DroughtSeverity::None,
                        }
                    },
                    (s, _) => s,
                }
            } else {
                vhi_severity
            };

            *severity_val = final_severity;
        }

        Ok(severity_map)
    }

    fn calculate_affected_area(&self, severity_map: &Array2<DroughtSeverity>) -> AgroResult<f64> {
        let pixel_area = 100.0; // m² per pixel (10m resolution)
        let mut affected_pixels = 0;

        for severity in severity_map.iter() {
            if matches!(*severity, DroughtSeverity::Mild | DroughtSeverity::Moderate | 
                                  DroughtSeverity::Severe | DroughtSeverity::Extreme) {
                affected_pixels += 1;
            }
        }

        Ok((affected_pixels as f64 * pixel_area) / 10000.0) // Convert to hectares
    }

    fn estimate_drought_duration(&self, historical_data: Option<&[DroughtAnalysisResult]>) -> Option<u32> {
        if let Some(history) = historical_data {
            if history.len() >= 2 {
                // Simple estimation: count consecutive periods with drought
                let mut drought_periods = 0;
                for result in history.iter().rev() { // Most recent first
                    if matches!(result.drought_severity, 
                               DroughtSeverity::Mild | DroughtSeverity::Moderate |
                               DroughtSeverity::Severe | DroughtSeverity::Extreme) {
                        drought_periods += 1;
                    } else {
                        break;
                    }
                }
                
                // Assume each analysis represents ~30 days
                Some(drought_periods * 30)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn calculate_recovery_probability(
        &self,
        severity_map: &Array2<DroughtSeverity>,
        ndvi: &Array2<f32>,
    ) -> AgroResult<f32> {
        let mut recovery_scores = Vec::new();

        for ((i, j), severity) in severity_map.indexed_iter() {
            let ndvi_val = ndvi[[i, j]];
            
            if ndvi_val.is_finite() {
                let recovery_score = match *severity {
                    DroughtSeverity::None => 1.0,
                    DroughtSeverity::Mild => 0.8 + ndvi_val * 0.2,
                    DroughtSeverity::Moderate => 0.6 + ndvi_val * 0.3,
                    DroughtSeverity::Severe => 0.3 + ndvi_val * 0.4,
                    DroughtSeverity::Extreme => 0.1 + ndvi_val * 0.2,
                };
                
                recovery_scores.push(recovery_score.clamp(0.0, 1.0));
            }
        }

        if recovery_scores.is_empty() {
            Ok(0.0)
        } else {
            Ok(recovery_scores.iter().sum::<f32>() / recovery_scores.len() as f32)
        }
    }

    fn assess_impact(
        &self,
        severity_map: &Array2<DroughtSeverity>,
        ndvi: &Array2<f32>,
        affected_area: f64,
    ) -> AgroResult<ImpactAssessment> {
        let ndvi_stats = self.engine.calculate_statistics(ndvi);
        
        // Estimate crop yield impact based on NDVI reduction
        let normal_ndvi = 0.7; // Assumed normal NDVI for healthy crops
        let yield_impact = if ndvi_stats.mean > 0.0 {
            ((normal_ndvi - ndvi_stats.mean) / normal_ndvi * 100.0).clamp(0.0, 100.0)
        } else {
            100.0
        };

        // Simple economic loss estimation (USD per hectare affected)
        let loss_per_hectare = match severity_map.iter().max() {
            Some(DroughtSeverity::Extreme) => 5000.0,
            Some(DroughtSeverity::Severe) => 3000.0,
            Some(DroughtSeverity::Moderate) => 1500.0,
            Some(DroughtSeverity::Mild) => 500.0,
            _ => 0.0,
        };
        
        let economic_loss_estimate = Some(affected_area * loss_per_hectare);

        // Water resources impact (percentage of normal capacity)
        let water_resources_impact = match severity_map.iter().max() {
            Some(DroughtSeverity::Extreme) => 80.0,
            Some(DroughtSeverity::Severe) => 60.0,
            Some(DroughtSeverity::Moderate) => 40.0,
            Some(DroughtSeverity::Mild) => 20.0,
            _ => 0.0,
        };

        // Ecosystem impact (biodiversity and habitat stress)
        let ecosystem_impact = (yield_impact * 0.6).clamp(0.0, 100.0);

        // Affected population (rough estimate based on area)
        let population_density = 50.0; // people per km²
        let affected_population = Some((affected_area / 100.0 * population_density) as u32);

        Ok(ImpactAssessment {
            crop_yield_impact: yield_impact,
            economic_loss_estimate,
            water_resources_impact,
            ecosystem_impact,
            affected_population,
        })
    }

    fn classify_overall_drought_severity(&self, severity_map: &Array2<DroughtSeverity>) -> DroughtSeverity {
        let mut severity_counts = HashMap::new();
        
        for severity in severity_map.iter() {
            *severity_counts.entry(*severity).or_insert(0) += 1;
        }

        // Determine overall severity based on most severe condition affecting >10% of area
        let total_pixels = severity_map.len();
        let threshold = total_pixels / 10; // 10% threshold

        if severity_counts.get(&DroughtSeverity::Extreme).unwrap_or(&0) > &threshold {
            DroughtSeverity::Extreme
        } else if severity_counts.get(&DroughtSeverity::Severe).unwrap_or(&0) > &threshold {
            DroughtSeverity::Severe
        } else if severity_counts.get(&DroughtSeverity::Moderate).unwrap_or(&0) > &threshold {
            DroughtSeverity::Moderate
        } else if severity_counts.get(&DroughtSeverity::Mild).unwrap_or(&0) > &threshold {
            DroughtSeverity::Mild
        } else {
            DroughtSeverity::None
        }
    }
}

impl Default for DroughtAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
