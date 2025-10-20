use crate::analysis_core::AnalysisEngine;
use crate::analysis_schemas::*;
use ndarray::Array2;
use shared::AgroResult;
use std::collections::HashMap;
use uuid::Uuid;

/// Comprehensive multi-temporal analysis for change detection and forecasting
pub struct MultiTemporalAnalyzer {
    engine: AnalysisEngine,
}

impl MultiTemporalAnalyzer {
    pub fn new() -> Self {
        Self {
            engine: AnalysisEngine::new(),
        }
    }

    /// Perform comprehensive multi-temporal analysis
    pub fn analyze_time_series(
        &self,
        time_series_data: &[(chrono::DateTime<chrono::Utc>, HashMap<String, Array2<f32>>)],
        analysis_type: AnalysisType,
        source_images: Vec<Uuid>,
        output_path: String,
    ) -> AgroResult<MultiTemporalResult> {
        if time_series_data.len() < 2 {
            return Err(shared::error::AgroError::Processing(
                "At least 2 time points required for temporal analysis".to_string()
            ));
        }

        let start_time = std::time::Instant::now();

        // Compute the specified index for each time point
        let mut time_series = Vec::new();
        let mut index_values = Vec::new();

        for (timestamp, bands) in time_series_data {
            let index_data = self.compute_index_for_type(&analysis_type, bands)?;
            let statistics = self.engine.calculate_statistics(&index_data);
            
            let analysis_result = self.engine.create_analysis_result(
                analysis_type.clone(),
                source_images.clone(),
                format!("{}_{}", output_path, timestamp.format("%Y%m%d")),
                statistics.clone(),
                0, // Individual processing time not tracked here
                HashMap::new(),
                self.get_required_bands(&analysis_type),
            );

            time_series.push(analysis_result);
            index_values.push(statistics.mean);
        }

        // Perform trend analysis
        let trend_analysis = self.analyze_trend(&index_values)?;

        // Detect anomalies
        let anomaly_detection = self.detect_anomalies(&time_series_data, &index_values)?;

        // Generate forecast if enough data points
        let forecasting = if index_values.len() >= 5 {
            Some(self.generate_forecast(&index_values, &trend_analysis)?)
        } else {
            None
        };

        Ok(MultiTemporalResult {
            time_series,
            trend_analysis,
            anomaly_detection,
            forecasting,
        })
    }

    /// Compute the appropriate index based on analysis type
    fn compute_index_for_type(
        &self,
        analysis_type: &AnalysisType,
        bands: &HashMap<String, Array2<f32>>,
    ) -> AgroResult<Array2<f32>> {
        match analysis_type {
            AnalysisType::Ndvi => {
                let nir = bands.get("nir").ok_or_else(|| 
                    shared::error::AgroError::Processing("NIR band required for NDVI".to_string()))?;
                let red = bands.get("red").ok_or_else(|| 
                    shared::error::AgroError::Processing("Red band required for NDVI".to_string()))?;
                self.engine.compute_ndvi(nir, red)
            },
            AnalysisType::Evi => {
                let nir = bands.get("nir").ok_or_else(|| 
                    shared::error::AgroError::Processing("NIR band required for EVI".to_string()))?;
                let red = bands.get("red").ok_or_else(|| 
                    shared::error::AgroError::Processing("Red band required for EVI".to_string()))?;
                let blue = bands.get("blue").ok_or_else(|| 
                    shared::error::AgroError::Processing("Blue band required for EVI".to_string()))?;
                self.engine.compute_evi(nir, red, blue)
            },
            AnalysisType::Ndwi => {
                let green = bands.get("green").ok_or_else(|| 
                    shared::error::AgroError::Processing("Green band required for NDWI".to_string()))?;
                let nir = bands.get("nir").ok_or_else(|| 
                    shared::error::AgroError::Processing("NIR band required for NDWI".to_string()))?;
                self.engine.compute_ndwi(green, nir)
            },
            AnalysisType::Nbr => {
                let nir = bands.get("nir").ok_or_else(|| 
                    shared::error::AgroError::Processing("NIR band required for NBR".to_string()))?;
                let swir = bands.get("swir1").or_else(|| bands.get("swir")).ok_or_else(|| 
                    shared::error::AgroError::Processing("SWIR band required for NBR".to_string()))?;
                self.engine.compute_nbr(nir, swir)
            },
            AnalysisType::Savi => {
                let nir = bands.get("nir").ok_or_else(|| 
                    shared::error::AgroError::Processing("NIR band required for SAVI".to_string()))?;
                let red = bands.get("red").ok_or_else(|| 
                    shared::error::AgroError::Processing("Red band required for SAVI".to_string()))?;
                self.engine.compute_savi(nir, red, 0.5)
            },
            _ => {
                // Default to NDVI for unsupported types
                let nir = bands.get("nir").ok_or_else(|| 
                    shared::error::AgroError::Processing("NIR band required".to_string()))?;
                let red = bands.get("red").ok_or_else(|| 
                    shared::error::AgroError::Processing("Red band required".to_string()))?;
                self.engine.compute_ndvi(nir, red)
            }
        }
    }

    fn get_required_bands(&self, analysis_type: &AnalysisType) -> Vec<String> {
        match analysis_type {
            AnalysisType::Ndvi => vec!["nir".to_string(), "red".to_string()],
            AnalysisType::Evi => vec!["nir".to_string(), "red".to_string(), "blue".to_string()],
            AnalysisType::Ndwi => vec!["green".to_string(), "nir".to_string()],
            AnalysisType::Nbr => vec!["nir".to_string(), "swir".to_string()],
            AnalysisType::Savi => vec!["nir".to_string(), "red".to_string()],
            _ => vec!["nir".to_string(), "red".to_string()],
        }
    }

    /// Analyze trend in time series data
    fn analyze_trend(&self, values: &[f32]) -> AgroResult<TrendAnalysis> {
        if values.len() < 3 {
            return Err(shared::error::AgroError::Processing(
                "At least 3 time points required for trend analysis".to_string()
            ));
        }

        // Simple linear regression to determine trend
        let n = values.len() as f32;
        let x_mean = (n - 1.0) / 2.0; // Time points are 0, 1, 2, ..., n-1
        let y_mean = values.iter().sum::<f32>() / n;

        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for (i, &y) in values.iter().enumerate() {
            let x = i as f32;
            numerator += (x - x_mean) * (y - y_mean);
            denominator += (x - x_mean).powi(2);
        }

        let slope = if denominator.abs() > 1e-8 {
            numerator / denominator
        } else {
            0.0
        };

        // Determine trend direction and strength
        let overall_trend = match slope {
            s if s > 0.01 => Trend::Increasing,
            s if s < -0.01 => Trend::Decreasing,
            _ => Trend::Stable,
        };

        let trend_strength = slope.abs().clamp(0.0, 1.0);

        // Simple seasonality detection (look for cyclic patterns)
        let seasonal_component = self.detect_seasonality(values);

        // Detect breakpoints (significant changes in trend)
        let breakpoints = self.detect_breakpoints(values);

        Ok(TrendAnalysis {
            overall_trend,
            trend_strength,
            seasonal_component,
            breakpoints,
        })
    }

    fn detect_seasonality(&self, values: &[f32]) -> bool {
        if values.len() < 8 {
            return false;
        }

        // Simple autocorrelation check for periodicity
        let mut autocorr_scores = Vec::new();
        
        for lag in 2..=values.len()/3 {
            let mut correlation = 0.0;
            let valid_pairs = values.len() - lag;
            
            for i in 0..valid_pairs {
                correlation += values[i] * values[i + lag];
            }
            
            autocorr_scores.push(correlation / valid_pairs as f32);
        }

        // Check if any lag shows strong correlation (simplified)
        autocorr_scores.iter().any(|&score| score > 0.7)
    }

    fn detect_breakpoints(&self, values: &[f32]) -> Vec<chrono::DateTime<chrono::Utc>> {
        let mut breakpoints = Vec::new();
        
        if values.len() < 6 {
            return breakpoints;
        }

        // Simple breakpoint detection using moving averages
        let window_size = 3;
        let mut moving_averages = Vec::new();

        for i in window_size..values.len()-window_size {
            let before_avg: f32 = values[i-window_size..i].iter().sum::<f32>() / window_size as f32;
            let after_avg: f32 = values[i+1..=i+window_size].iter().sum::<f32>() / window_size as f32;
            
            let change = (after_avg - before_avg).abs();
            moving_averages.push((i, change));
        }

        // Find significant changes (above 75th percentile)
        let mut changes: Vec<f32> = moving_averages.iter().map(|(_, change)| *change).collect();
        changes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        if let Some(&threshold) = changes.get(changes.len() * 3 / 4) {
            for (idx, change) in moving_averages {
                if change > threshold {
                    // Convert index to approximate timestamp (simplified)
                    let days_offset = idx as i64 * 30; // Assume monthly data
                    let breakpoint = chrono::Utc::now() - chrono::Duration::days(
                        (values.len() as i64 - idx as i64) * 30
                    );
                    breakpoints.push(breakpoint);
                }
            }
        }

        breakpoints
    }

    /// Detect anomalies in the time series
    fn detect_anomalies(
        &self,
        time_series_data: &[(chrono::DateTime<chrono::Utc>, HashMap<String, Array2<f32>>)],
        values: &[f32],
    ) -> AgroResult<AnomalyDetection> {
        if values.len() < 3 {
            return Ok(AnomalyDetection {
                anomalies: Vec::new(),
                threshold: 0.0,
                detection_method: "insufficient_data".to_string(),
            });
        }

        // Calculate statistical threshold (2 standard deviations)
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        let variance = values.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f32>() / values.len() as f32;
        let std_dev = variance.sqrt();
        let threshold = 2.0 * std_dev;

        let mut anomalies = Vec::new();

        for (i, (&value, (timestamp, _))) in values.iter().zip(time_series_data.iter()).enumerate() {
            let deviation = (value - mean).abs();
            
            if deviation > threshold {
                let severity = deviation / std_dev;
                
                // Determine probable cause based on context
                let probable_cause = if value < mean - threshold {
                    Some("Vegetation stress, drought, or disturbance".to_string())
                } else {
                    Some("Unusual vegetation growth or data artifact".to_string())
                };

                anomalies.push(Anomaly {
                    timestamp: *timestamp,
                    severity,
                    spatial_extent: None, // Would require spatial analysis
                    probable_cause,
                });
            }
        }

        Ok(AnomalyDetection {
            anomalies,
            threshold,
            detection_method: "statistical_outlier_2sigma".to_string(),
        })
    }

    /// Generate forecast based on trend analysis
    fn generate_forecast(
        &self,
        values: &[f32],
        trend_analysis: &TrendAnalysis,
    ) -> AgroResult<Forecast> {
        let forecast_horizon_days = 90; // 3 months ahead
        let forecast_points = 3; // Monthly predictions

        // Simple linear extrapolation based on trend
        let last_value = values[values.len() - 1];
        let trend_per_period = trend_analysis.trend_strength * 
            match trend_analysis.overall_trend {
                Trend::Increasing => 1.0,
                Trend::Decreasing => -1.0,
                _ => 0.0,
            };

        let mut predictions = Vec::new();
        let mut confidence_intervals = Vec::new();

        // Calculate prediction uncertainty
        let historical_variance = {
            let mean = values.iter().sum::<f32>() / values.len() as f32;
            values.iter()
                .map(|&x| (x - mean).powi(2))
                .sum::<f32>() / values.len() as f32
        };
        let uncertainty = historical_variance.sqrt();

        for i in 1..=forecast_points {
            let predicted_value = last_value + (trend_per_period * i as f32);
            let confidence_decay = 1.0 - (i as f32 * 0.1); // Confidence decreases with time
            let confidence = (0.8 * confidence_decay).clamp(0.1, 0.9);
            
            let interval_width = uncertainty * (2.0 - confidence);
            
            predictions.push(PredictionPoint {
                timestamp: chrono::Utc::now() + chrono::Duration::days(i as i64 * 30),
                predicted_value,
                confidence,
            });
            
            confidence_intervals.push((
                predicted_value - interval_width,
                predicted_value + interval_width,
            ));
        }

        // Calculate model accuracy based on historical fit
        let model_accuracy = self.calculate_model_accuracy(values, trend_analysis);

        Ok(Forecast {
            predictions,
            confidence_intervals,
            model_accuracy,
            forecast_horizon_days,
        })
    }

    fn calculate_model_accuracy(&self, values: &[f32], trend_analysis: &TrendAnalysis) -> f32 {
        if values.len() < 3 {
            return 0.0;
        }

        // Simple R-squared calculation for linear trend
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        let total_ss = values.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f32>();

        if total_ss <= 1e-8 {
            return 1.0; // Perfect fit for constant values
        }

        // Calculate residuals from linear trend
        let slope = trend_analysis.trend_strength * 
            match trend_analysis.overall_trend {
                Trend::Increasing => 1.0,
                Trend::Decreasing => -1.0,
                _ => 0.0,
            };

        let residual_ss = values.iter().enumerate()
            .map(|(i, &y)| {
                let predicted = values[0] + slope * i as f32;
                (y - predicted).powi(2)
            })
            .sum::<f32>();

        let r_squared = 1.0 - (residual_ss / total_ss);
        r_squared.clamp(0.0, 1.0)
    }
}

impl Default for MultiTemporalAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
