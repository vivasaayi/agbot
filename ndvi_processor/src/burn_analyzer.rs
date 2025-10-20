use crate::analysis_core::AnalysisEngine;
use crate::analysis_schemas::*;
use ndarray::Array2;
use shared::AgroResult;
use std::collections::HashMap;
use uuid::Uuid;

/// Specialized burn analysis for fire detection and recovery monitoring
pub struct BurnAnalyzer {
    engine: AnalysisEngine,
}

impl BurnAnalyzer {
    pub fn new() -> Self {
        Self {
            engine: AnalysisEngine::new(),
        }
    }

    /// Perform comprehensive burn analysis
    pub fn analyze_burn(
        &self,
        bands: &HashMap<String, Array2<f32>>,
        pre_fire_bands: Option<&HashMap<String, Array2<f32>>>,
        source_images: Vec<Uuid>,
        output_path: String,
    ) -> AgroResult<BurnAnalysisResult> {
        let start_time = std::time::Instant::now();

        // Extract required bands
        let nir = bands.get("nir").ok_or_else(|| 
            shared::error::AgroError::Processing("NIR band required".to_string()))?;
        let swir = bands.get("swir1").or_else(|| bands.get("swir")).ok_or_else(|| 
            shared::error::AgroError::Processing("SWIR band required".to_string()))?;
        let red = bands.get("red").ok_or_else(|| 
            shared::error::AgroError::Processing("Red band required".to_string()))?;

        // Compute burn indices
        let nbr = self.engine.compute_nbr(nir, swir)?;
        let bai = self.engine.compute_bai(red, nir)?;

        // Compute dNBR if pre-fire data available
        let dnbr = if let Some(pre_fire) = pre_fire_bands {
            let pre_nir = pre_fire.get("nir").ok_or_else(|| 
                shared::error::AgroError::Processing("Pre-fire NIR band required for dNBR".to_string()))?;
            let pre_swir = pre_fire.get("swir1").or_else(|| pre_fire.get("swir")).ok_or_else(|| 
                shared::error::AgroError::Processing("Pre-fire SWIR band required for dNBR".to_string()))?;
            
            let pre_nbr = self.engine.compute_nbr(pre_nir, pre_swir)?;
            Some(self.engine.compute_dnbr(&pre_nbr, &nbr)?)
        } else {
            None
        };

        // Assess burn severity
        let burn_severity = if let Some(ref dnbr_data) = dnbr {
            self.assess_burn_severity_dnbr(dnbr_data)?
        } else {
            self.assess_burn_severity_nbr(&nbr)?
        };

        // Calculate burned area
        let burned_area_hectares = self.calculate_burned_area(&burn_severity)?;

        // Assess recovery stage
        let recovery_stage = self.assess_recovery_stage(&nbr, &burn_severity)?;

        // Analyze fire progression if applicable
        let fire_progression = self.analyze_fire_progression(&burn_severity, &bai)?;

        // Calculate statistics
        let primary_index = dnbr.as_ref().unwrap_or(&nbr);
        let statistics = self.engine.calculate_statistics(primary_index);

        let processing_time = start_time.elapsed().as_millis() as u64;

        // Create parameters map
        let mut parameters = HashMap::new();
        parameters.insert("severity_method".to_string(), 
            serde_json::Value::from(if dnbr.is_some() { "dnbr" } else { "nbr" }));
        parameters.insert("moderate_burn_threshold".to_string(), serde_json::Value::from(0.27));
        parameters.insert("high_burn_threshold".to_string(), serde_json::Value::from(0.66));

        let analysis_result = self.engine.create_analysis_result(
            AnalysisType::Nbr,
            source_images,
            output_path,
            statistics,
            processing_time,
            parameters,
            vec!["nir".to_string(), "swir".to_string(), "red".to_string()],
        );

        Ok(BurnAnalysisResult {
            analysis_result,
            burn_severity: self.classify_overall_burn_severity(&burn_severity),
            burned_area_hectares,
            recovery_stage,
            fire_progression,
        })
    }

    /// Assess burn severity using dNBR (preferred method)
    fn assess_burn_severity_dnbr(&self, dnbr: &Array2<f32>) -> AgroResult<Array2<BurnSeverity>> {
        let mut severity_map = Array2::from_elem(dnbr.dim(), BurnSeverity::Unburned);

        for ((i, j), severity_val) in severity_map.indexed_iter_mut() {
            let dnbr_val = dnbr[[i, j]];

            if dnbr_val.is_finite() {
                *severity_val = match dnbr_val {
                    d if d < -0.1 => BurnSeverity::Unburned,
                    d if d < 0.1 => BurnSeverity::Unburned,
                    d if d < 0.27 => BurnSeverity::Low,
                    d if d < 0.44 => BurnSeverity::Moderate,
                    d if d < 0.66 => BurnSeverity::High,
                    _ => BurnSeverity::HighPostFire,
                };
            }
        }

        Ok(severity_map)
    }

    /// Assess burn severity using NBR (when pre-fire data not available)
    fn assess_burn_severity_nbr(&self, nbr: &Array2<f32>) -> AgroResult<Array2<BurnSeverity>> {
        let mut severity_map = Array2::from_elem(nbr.dim(), BurnSeverity::Unburned);
        let nbr_stats = self.engine.calculate_statistics(nbr);

        // Use statistical thresholds when pre-fire data unavailable
        let high_threshold = nbr_stats.percentile_25; // Lower NBR indicates more severe burn
        let moderate_threshold = nbr_stats.median;
        let low_threshold = nbr_stats.percentile_75;

        for ((i, j), severity_val) in severity_map.indexed_iter_mut() {
            let nbr_val = nbr[[i, j]];

            if nbr_val.is_finite() {
                *severity_val = match nbr_val {
                    n if n > low_threshold => BurnSeverity::Unburned,
                    n if n > moderate_threshold => BurnSeverity::Low,
                    n if n > high_threshold => BurnSeverity::Moderate,
                    _ => BurnSeverity::High,
                };
            }
        }

        Ok(severity_map)
    }

    fn calculate_burned_area(&self, severity_map: &Array2<BurnSeverity>) -> AgroResult<f64> {
        let pixel_area = 100.0; // m² per pixel (10m resolution)
        let mut burned_pixels = 0;

        for severity in severity_map.iter() {
            if !matches!(*severity, BurnSeverity::Unburned) {
                burned_pixels += 1;
            }
        }

        Ok((burned_pixels as f64 * pixel_area) / 10000.0) // Convert to hectares
    }

    fn assess_recovery_stage(
        &self,
        nbr: &Array2<f32>,
        severity_map: &Array2<BurnSeverity>,
    ) -> AgroResult<RecoveryStage> {
        let mut recovery_indicators = Vec::new();

        for ((i, j), severity) in severity_map.indexed_iter() {
            if !matches!(*severity, BurnSeverity::Unburned) {
                let nbr_val = nbr[[i, j]];
                if nbr_val.is_finite() {
                    // Higher NBR indicates better recovery
                    let recovery_score = match severity {
                        BurnSeverity::Low => nbr_val + 0.2,
                        BurnSeverity::Moderate => nbr_val + 0.1,
                        BurnSeverity::High => nbr_val,
                        BurnSeverity::HighPostFire => nbr_val - 0.1,
                        BurnSeverity::Unburned => 1.0,
                    };
                    recovery_indicators.push(recovery_score);
                }
            }
        }

        if recovery_indicators.is_empty() {
            return Ok(RecoveryStage::Recovered);
        }

        let mean_recovery = recovery_indicators.iter().sum::<f32>() / recovery_indicators.len() as f32;

        Ok(match mean_recovery {
            r if r > 0.5 => RecoveryStage::LongTerm,
            r if r > 0.3 => RecoveryStage::MediumTerm,
            r if r > 0.1 => RecoveryStage::ShortTerm,
            _ => RecoveryStage::Immediate,
        })
    }

    fn analyze_fire_progression(
        &self,
        severity_map: &Array2<BurnSeverity>,
        bai: &Array2<f32>,
    ) -> AgroResult<Option<FireProgression>> {
        // Check if this appears to be an active fire (high BAI values)
        let bai_stats = self.engine.calculate_statistics(bai);
        
        if bai_stats.max < 50.0 { // Threshold for active fire detection
            return Ok(None);
        }

        // Find fire perimeter and calculate progression metrics
        let (rows, cols) = severity_map.dim();
        let mut fire_pixels = Vec::new();
        let mut high_intensity_pixels = Vec::new();

        for ((i, j), severity) in severity_map.indexed_iter() {
            if !matches!(*severity, BurnSeverity::Unburned) {
                fire_pixels.push((i, j));
                
                if matches!(*severity, BurnSeverity::High | BurnSeverity::HighPostFire) {
                    high_intensity_pixels.push((i, j));
                }
            }
        }

        if fire_pixels.is_empty() {
            return Ok(None);
        }

        // Estimate progression rate (simplified)
        let total_burned_area = fire_pixels.len() as f32 * 0.01; // hectares
        let progression_rate = total_burned_area / 1.0; // Assume 1 day progression (would need temporal data)

        // Estimate fire direction (simplified - use center of mass shift)
        let center_i = fire_pixels.iter().map(|(i, _)| *i).sum::<usize>() as f32 / fire_pixels.len() as f32;
        let center_j = fire_pixels.iter().map(|(_, j)| *j).sum::<usize>() as f32 / fire_pixels.len() as f32;
        
        // Direction estimation (would be more accurate with temporal data)
        let direction = if center_j > cols as f32 / 2.0 { 90.0 } else { 270.0 };

        // Fire intensity based on BAI statistics
        let intensity = (bai_stats.mean / 100.0).clamp(0.0, 1.0);

        // Containment probability (higher for smaller, less intense fires)
        let containment_probability = if total_burned_area < 100.0 && intensity < 0.7 {
            0.8
        } else if total_burned_area < 500.0 && intensity < 0.5 {
            0.6
        } else if total_burned_area < 1000.0 {
            0.4
        } else {
            0.2
        };

        Ok(Some(FireProgression {
            progression_rate,
            direction,
            intensity,
            containment_probability,
        }))
    }

    fn classify_overall_burn_severity(&self, severity_map: &Array2<BurnSeverity>) -> BurnSeverity {
        let mut severity_counts = HashMap::new();
        
        for severity in severity_map.iter() {
            *severity_counts.entry(*severity).or_insert(0) += 1;
        }

        let total_pixels = severity_map.len();
        let burned_pixels = total_pixels - severity_counts.get(&BurnSeverity::Unburned).unwrap_or(&0);

        if burned_pixels == 0 {
            return BurnSeverity::Unburned;
        }

        // Classify based on most severe condition affecting significant area
        let high_threshold = burned_pixels / 10; // 10% of burned area
        let moderate_threshold = burned_pixels / 5; // 20% of burned area

        if severity_counts.get(&BurnSeverity::HighPostFire).unwrap_or(&0) > &high_threshold {
            BurnSeverity::HighPostFire
        } else if severity_counts.get(&BurnSeverity::High).unwrap_or(&0) > &high_threshold {
            BurnSeverity::High
        } else if severity_counts.get(&BurnSeverity::Moderate).unwrap_or(&0) > &moderate_threshold {
            BurnSeverity::Moderate
        } else {
            BurnSeverity::Low
        }
    }
}

impl Default for BurnAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
