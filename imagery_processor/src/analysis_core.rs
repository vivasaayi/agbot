use ndarray::{Array2, Array3};
use crate::analysis_schemas::*;
use shared::AgroResult;
use std::collections::HashMap;
use chrono::Utc;
use uuid::Uuid;

/// Core analysis engine for all spectral indices and classification
pub struct AnalysisEngine {
    pub cache_results: bool,
    pub output_intermediate: bool,
    pub parallel_processing: bool,
}

impl AnalysisEngine {
    pub fn new() -> Self {
        Self {
            cache_results: true,
            output_intermediate: false,
            parallel_processing: true,
        }
    }

    /// Compute NDVI (Normalized Difference Vegetation Index)
    /// NDVI = (NIR - Red) / (NIR + Red)
    pub fn compute_ndvi(&self, nir: &Array2<f32>, red: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut ndvi = Array2::zeros(nir.dim());
        
        for ((i, j), ndvi_val) in ndvi.indexed_iter_mut() {
            let nir_val = nir[[i, j]];
            let red_val = red[[i, j]];
            
            if nir_val.is_finite() && red_val.is_finite() {
                let denominator = nir_val + red_val;
                if denominator.abs() > 1e-8 {
                    *ndvi_val = (nir_val - red_val) / denominator;
                } else {
                    *ndvi_val = f32::NAN;
                }
            } else {
                *ndvi_val = f32::NAN;
            }
        }
        
        Ok(ndvi)
    }

    /// Compute EVI (Enhanced Vegetation Index)
    /// EVI = 2.5 * ((NIR - Red) / (NIR + 6*Red - 7.5*Blue + 1))
    pub fn compute_evi(&self, nir: &Array2<f32>, red: &Array2<f32>, blue: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut evi = Array2::zeros(nir.dim());
        
        for ((i, j), evi_val) in evi.indexed_iter_mut() {
            let nir_val = nir[[i, j]];
            let red_val = red[[i, j]];
            let blue_val = blue[[i, j]];
            
            if nir_val.is_finite() && red_val.is_finite() && blue_val.is_finite() {
                let denominator = nir_val + 6.0 * red_val - 7.5 * blue_val + 1.0;
                if denominator.abs() > 1e-8 {
                    *evi_val = 2.5 * (nir_val - red_val) / denominator;
                } else {
                    *evi_val = f32::NAN;
                }
            } else {
                *evi_val = f32::NAN;
            }
        }
        
        Ok(evi)
    }

    /// Compute SAVI (Soil Adjusted Vegetation Index)
    /// SAVI = ((NIR - Red) / (NIR + Red + L)) * (1 + L)
    /// where L is a soil brightness correction factor (typically 0.5)
    pub fn compute_savi(&self, nir: &Array2<f32>, red: &Array2<f32>, l_factor: f32) -> AgroResult<Array2<f32>> {
        let mut savi = Array2::zeros(nir.dim());
        
        for ((i, j), savi_val) in savi.indexed_iter_mut() {
            let nir_val = nir[[i, j]];
            let red_val = red[[i, j]];
            
            if nir_val.is_finite() && red_val.is_finite() {
                let denominator = nir_val + red_val + l_factor;
                if denominator.abs() > 1e-8 {
                    *savi_val = ((nir_val - red_val) / denominator) * (1.0 + l_factor);
                } else {
                    *savi_val = f32::NAN;
                }
            } else {
                *savi_val = f32::NAN;
            }
        }
        
        Ok(savi)
    }

    /// Compute ARVI (Atmospherically Resistant Vegetation Index)
    /// ARVI = (NIR - RB) / (NIR + RB)
    /// where RB = Red - γ(Blue - Red), γ typically 1.0
    pub fn compute_arvi(&self, nir: &Array2<f32>, red: &Array2<f32>, blue: &Array2<f32>, gamma: f32) -> AgroResult<Array2<f32>> {
        let mut arvi = Array2::zeros(nir.dim());
        
        for ((i, j), arvi_val) in arvi.indexed_iter_mut() {
            let nir_val = nir[[i, j]];
            let red_val = red[[i, j]];
            let blue_val = blue[[i, j]];
            
            if nir_val.is_finite() && red_val.is_finite() && blue_val.is_finite() {
                let rb = red_val - gamma * (blue_val - red_val);
                let denominator = nir_val + rb;
                if denominator.abs() > 1e-8 {
                    *arvi_val = (nir_val - rb) / denominator;
                } else {
                    *arvi_val = f32::NAN;
                }
            } else {
                *arvi_val = f32::NAN;
            }
        }
        
        Ok(arvi)
    }

    /// Compute MSAVI (Modified Soil Adjusted Vegetation Index)
    /// MSAVI = (2*NIR + 1 - sqrt((2*NIR + 1)² - 8*(NIR - Red))) / 2
    pub fn compute_msavi(&self, nir: &Array2<f32>, red: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut msavi = Array2::zeros(nir.dim());
        
        for ((i, j), msavi_val) in msavi.indexed_iter_mut() {
            let nir_val = nir[[i, j]];
            let red_val = red[[i, j]];
            
            if nir_val.is_finite() && red_val.is_finite() {
                let term1 = 2.0 * nir_val + 1.0;
                let term2 = term1 * term1 - 8.0 * (nir_val - red_val);
                
                if term2 >= 0.0 {
                    *msavi_val = (term1 - term2.sqrt()) / 2.0;
                } else {
                    *msavi_val = f32::NAN;
                }
            } else {
                *msavi_val = f32::NAN;
            }
        }
        
        Ok(msavi)
    }

    /// Compute CVI (Chlorophyll Vegetation Index)
    /// CVI = (NIR / Green) * (Red / Green)
    pub fn compute_cvi(&self, nir: &Array2<f32>, red: &Array2<f32>, green: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut cvi = Array2::zeros(nir.dim());
        
        for ((i, j), cvi_val) in cvi.indexed_iter_mut() {
            let nir_val = nir[[i, j]];
            let red_val = red[[i, j]];
            let green_val = green[[i, j]];
            
            if nir_val.is_finite() && red_val.is_finite() && green_val.is_finite() && green_val.abs() > 1e-8 {
                *cvi_val = (nir_val / green_val) * (red_val / green_val);
            } else {
                *cvi_val = f32::NAN;
            }
        }
        
        Ok(cvi)
    }

    /// Compute LAI (Leaf Area Index) approximation
    /// LAI ≈ 3.618 * EVI - 0.118 (simplified empirical relationship)
    pub fn compute_lai(&self, evi: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut lai = Array2::zeros(evi.dim());
        
        for ((i, j), lai_val) in lai.indexed_iter_mut() {
            let evi_val = evi[[i, j]];
            
            if evi_val.is_finite() {
                let lai_estimate = 3.618 * evi_val - 0.118;
                *lai_val = if lai_estimate > 0.0 { lai_estimate } else { 0.0 };
            } else {
                *lai_val = f32::NAN;
            }
        }
        
        Ok(lai)
    }

    /// Compute fCover (Fractional Cover)
    /// fCover = (NDVI - NDVIsoil) / (NDVIveg - NDVIsoil)
    pub fn compute_fcover(&self, ndvi: &Array2<f32>, ndvi_soil: f32, ndvi_veg: f32) -> AgroResult<Array2<f32>> {
        let mut fcover = Array2::zeros(ndvi.dim());
        let denominator = ndvi_veg - ndvi_soil;
        
        if denominator.abs() <= 1e-8 {
            return Err(shared::error::AgroError::Processing(
                "Invalid NDVI vegetation and soil parameters".to_string()
            ));
        }
        
        for ((i, j), fcover_val) in fcover.indexed_iter_mut() {
            let ndvi_val = ndvi[[i, j]];
            
            if ndvi_val.is_finite() {
                let fc = (ndvi_val - ndvi_soil) / denominator;
                *fcover_val = fc.clamp(0.0, 1.0);
            } else {
                *fcover_val = f32::NAN;
            }
        }
        
        Ok(fcover)
    }

    /// Compute NDWI (Normalized Difference Water Index)
    /// NDWI = (Green - NIR) / (Green + NIR)
    pub fn compute_ndwi(&self, green: &Array2<f32>, nir: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut ndwi = Array2::zeros(green.dim());
        
        for ((i, j), ndwi_val) in ndwi.indexed_iter_mut() {
            let green_val = green[[i, j]];
            let nir_val = nir[[i, j]];
            
            if green_val.is_finite() && nir_val.is_finite() {
                let denominator = green_val + nir_val;
                if denominator.abs() > 1e-8 {
                    *ndwi_val = (green_val - nir_val) / denominator;
                } else {
                    *ndwi_val = f32::NAN;
                }
            } else {
                *ndwi_val = f32::NAN;
            }
        }
        
        Ok(ndwi)
    }

    /// Compute MNDWI (Modified NDWI)
    /// MNDWI = (Green - SWIR) / (Green + SWIR)
    pub fn compute_mndwi(&self, green: &Array2<f32>, swir: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut mndwi = Array2::zeros(green.dim());
        
        for ((i, j), mndwi_val) in mndwi.indexed_iter_mut() {
            let green_val = green[[i, j]];
            let swir_val = swir[[i, j]];
            
            if green_val.is_finite() && swir_val.is_finite() {
                let denominator = green_val + swir_val;
                if denominator.abs() > 1e-8 {
                    *mndwi_val = (green_val - swir_val) / denominator;
                } else {
                    *mndwi_val = f32::NAN;
                }
            } else {
                *mndwi_val = f32::NAN;
            }
        }
        
        Ok(mndwi)
    }

    /// Compute AWEI (Automated Water Extraction Index)
    /// AWEI = 4 * (Green - SWIR) - (0.25 * NIR + 2.75 * SWIR2)
    pub fn compute_awei(&self, green: &Array2<f32>, nir: &Array2<f32>, swir1: &Array2<f32>, swir2: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut awei = Array2::zeros(green.dim());
        
        for ((i, j), awei_val) in awei.indexed_iter_mut() {
            let green_val = green[[i, j]];
            let nir_val = nir[[i, j]];
            let swir1_val = swir1[[i, j]];
            let swir2_val = swir2[[i, j]];
            
            if green_val.is_finite() && nir_val.is_finite() && swir1_val.is_finite() && swir2_val.is_finite() {
                *awei_val = 4.0 * (green_val - swir1_val) - (0.25 * nir_val + 2.75 * swir2_val);
            } else {
                *awei_val = f32::NAN;
            }
        }
        
        Ok(awei)
    }

    /// Compute NBR (Normalized Burn Ratio)
    /// NBR = (NIR - SWIR) / (NIR + SWIR)
    pub fn compute_nbr(&self, nir: &Array2<f32>, swir: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut nbr = Array2::zeros(nir.dim());
        
        for ((i, j), nbr_val) in nbr.indexed_iter_mut() {
            let nir_val = nir[[i, j]];
            let swir_val = swir[[i, j]];
            
            if nir_val.is_finite() && swir_val.is_finite() {
                let denominator = nir_val + swir_val;
                if denominator.abs() > 1e-8 {
                    *nbr_val = (nir_val - swir_val) / denominator;
                } else {
                    *nbr_val = f32::NAN;
                }
            } else {
                *nbr_val = f32::NAN;
            }
        }
        
        Ok(nbr)
    }

    /// Compute dNBR (differenced NBR for burn severity)
    pub fn compute_dnbr(&self, nbr_pre: &Array2<f32>, nbr_post: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut dnbr = Array2::zeros(nbr_pre.dim());
        
        for ((i, j), dnbr_val) in dnbr.indexed_iter_mut() {
            let pre_val = nbr_pre[[i, j]];
            let post_val = nbr_post[[i, j]];
            
            if pre_val.is_finite() && post_val.is_finite() {
                *dnbr_val = pre_val - post_val;
            } else {
                *dnbr_val = f32::NAN;
            }
        }
        
        Ok(dnbr)
    }

    /// Compute BAI (Burn Area Index)
    /// BAI = 1 / ((0.1 - Red)² + (0.06 - NIR)²)
    pub fn compute_bai(&self, red: &Array2<f32>, nir: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut bai = Array2::zeros(red.dim());
        
        for ((i, j), bai_val) in bai.indexed_iter_mut() {
            let red_val = red[[i, j]];
            let nir_val = nir[[i, j]];
            
            if red_val.is_finite() && nir_val.is_finite() {
                let term1 = (0.1 - red_val).powi(2);
                let term2 = (0.06 - nir_val).powi(2);
                let denominator = term1 + term2;
                
                if denominator > 1e-8 {
                    *bai_val = 1.0 / denominator;
                } else {
                    *bai_val = f32::NAN;
                }
            } else {
                *bai_val = f32::NAN;
            }
        }
        
        Ok(bai)
    }

    /// Compute NDBI (Normalized Difference Built-up Index)
    /// NDBI = (SWIR - NIR) / (SWIR + NIR)
    pub fn compute_ndbi(&self, swir: &Array2<f32>, nir: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut ndbi = Array2::zeros(swir.dim());
        
        for ((i, j), ndbi_val) in ndbi.indexed_iter_mut() {
            let swir_val = swir[[i, j]];
            let nir_val = nir[[i, j]];
            
            if swir_val.is_finite() && nir_val.is_finite() {
                let denominator = swir_val + nir_val;
                if denominator.abs() > 1e-8 {
                    *ndbi_val = (swir_val - nir_val) / denominator;
                } else {
                    *ndbi_val = f32::NAN;
                }
            } else {
                *ndbi_val = f32::NAN;
            }
        }
        
        Ok(ndbi)
    }

    /// Compute UI (Urban Index)
    /// UI = (SWIR2 - NIR) / (SWIR2 + NIR)
    pub fn compute_ui(&self, swir2: &Array2<f32>, nir: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut ui = Array2::zeros(swir2.dim());
        
        for ((i, j), ui_val) in ui.indexed_iter_mut() {
            let swir2_val = swir2[[i, j]];
            let nir_val = nir[[i, j]];
            
            if swir2_val.is_finite() && nir_val.is_finite() {
                let denominator = swir2_val + nir_val;
                if denominator.abs() > 1e-8 {
                    *ui_val = (swir2_val - nir_val) / denominator;
                } else {
                    *ui_val = f32::NAN;
                }
            } else {
                *ui_val = f32::NAN;
            }
        }
        
        Ok(ui)
    }

    /// Compute NDSI (Normalized Difference Snow Index)
    /// NDSI = (Green - SWIR) / (Green + SWIR)
    pub fn compute_ndsi(&self, green: &Array2<f32>, swir: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut ndsi = Array2::zeros(green.dim());
        
        for ((i, j), ndsi_val) in ndsi.indexed_iter_mut() {
            let green_val = green[[i, j]];
            let swir_val = swir[[i, j]];
            
            if green_val.is_finite() && swir_val.is_finite() {
                let denominator = green_val + swir_val;
                if denominator.abs() > 1e-8 {
                    *ndsi_val = (green_val - swir_val) / denominator;
                } else {
                    *ndsi_val = f32::NAN;
                }
            } else {
                *ndsi_val = f32::NAN;
            }
        }
        
        Ok(ndsi)
    }

    /// Compute BSI (Bare Soil Index)
    /// BSI = ((SWIR + Red) - (NIR + Blue)) / ((SWIR + Red) + (NIR + Blue))
    pub fn compute_bsi(&self, swir: &Array2<f32>, red: &Array2<f32>, nir: &Array2<f32>, blue: &Array2<f32>) -> AgroResult<Array2<f32>> {
        let mut bsi = Array2::zeros(swir.dim());
        
        for ((i, j), bsi_val) in bsi.indexed_iter_mut() {
            let swir_val = swir[[i, j]];
            let red_val = red[[i, j]];
            let nir_val = nir[[i, j]];
            let blue_val = blue[[i, j]];
            
            if swir_val.is_finite() && red_val.is_finite() && nir_val.is_finite() && blue_val.is_finite() {
                let numerator = (swir_val + red_val) - (nir_val + blue_val);
                let denominator = (swir_val + red_val) + (nir_val + blue_val);
                
                if denominator.abs() > 1e-8 {
                    *bsi_val = numerator / denominator;
                } else {
                    *bsi_val = f32::NAN;
                }
            } else {
                *bsi_val = f32::NAN;
            }
        }
        
        Ok(bsi)
    }

    /// Classify vegetation health based on NDVI values
    pub fn classify_vegetation_health(&self, ndvi: &Array2<f32>) -> Array2<HealthStatus> {
        let mut health = Array2::from_elem(ndvi.dim(), HealthStatus::NoData);
        
        for ((i, j), health_val) in health.indexed_iter_mut() {
            let ndvi_val = ndvi[[i, j]];
            
            if ndvi_val.is_finite() {
                *health_val = match ndvi_val {
                    v if v >= 0.8 => HealthStatus::Excellent,
                    v if v >= 0.6 => HealthStatus::Good,
                    v if v >= 0.4 => HealthStatus::Moderate,
                    v if v >= 0.2 => HealthStatus::Poor,
                    v if v >= 0.0 => HealthStatus::Critical,
                    _ => HealthStatus::NoData,
                };
            }
        }
        
        health
    }

    /// Classify land cover based on multiple indices
    pub fn classify_land_cover(&self, indices: &HashMap<String, Array2<f32>>) -> AgroResult<Array2<LandCoverType>> {
        let ndvi = indices.get("ndvi").ok_or_else(|| 
            shared::error::AgroError::Processing("NDVI required for land cover classification".to_string()))?;
        
        let dims = ndvi.dim();
        let mut land_cover = Array2::from_elem(dims, LandCoverType::Unknown);
        
        // Get optional indices
        let ndwi = indices.get("ndwi");
        let ndbi = indices.get("ndbi");
        let ndsi = indices.get("ndsi");
        
        for ((i, j), lc_val) in land_cover.indexed_iter_mut() {
            let ndvi_val = ndvi[[i, j]];
            
            if !ndvi_val.is_finite() {
                continue;
            }
            
            // Water detection (priority 1)
            if let Some(ndwi_array) = ndwi {
                let ndwi_val = ndwi_array[[i, j]];
                if ndwi_val.is_finite() && ndwi_val > 0.3 {
                    *lc_val = LandCoverType::Water;
                    continue;
                }
            }
            
            // Snow detection (priority 2)
            if let Some(ndsi_array) = ndsi {
                let ndsi_val = ndsi_array[[i, j]];
                if ndsi_val.is_finite() && ndsi_val > 0.4 {
                    *lc_val = LandCoverType::Snow;
                    continue;
                }
            }
            
            // Urban detection (priority 3)
            if let Some(ndbi_array) = ndbi {
                let ndbi_val = ndbi_array[[i, j]];
                if ndbi_val.is_finite() && ndbi_val > 0.1 && ndvi_val < 0.2 {
                    *lc_val = LandCoverType::Urban;
                    continue;
                }
            }
            
            // Vegetation classification based on NDVI
            *lc_val = match ndvi_val {
                v if v >= 0.7 => LandCoverType::Forest,
                v if v >= 0.4 => LandCoverType::Grassland,
                v if v >= 0.2 => LandCoverType::Cropland,
                v if v >= 0.0 => LandCoverType::BareSoil,
                _ => LandCoverType::Unknown,
            };
        }
        
        Ok(land_cover)
    }

    /// Calculate comprehensive statistics for any index
    pub fn calculate_statistics(&self, data: &Array2<f32>) -> IndexStatistics {
        let mut valid_values = Vec::new();
        let total_pixels = (data.dim().0 * data.dim().1) as u64;
        
        // Collect valid (non-NaN) values
        for &value in data.iter() {
            if value.is_finite() {
                valid_values.push(value);
            }
        }
        
        let valid_pixels = valid_values.len() as u64;
        let coverage_percentage = if total_pixels > 0 {
            (valid_pixels as f32 / total_pixels as f32) * 100.0
        } else {
            0.0
        };
        
        if valid_values.is_empty() {
            return IndexStatistics {
                min: f32::NAN,
                max: f32::NAN,
                mean: f32::NAN,
                median: f32::NAN,
                std_dev: f32::NAN,
                percentile_25: f32::NAN,
                percentile_75: f32::NAN,
                valid_pixels,
                total_pixels,
                coverage_percentage,
            };
        }
        
        valid_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let min = valid_values[0];
        let max = valid_values[valid_values.len() - 1];
        let mean = valid_values.iter().sum::<f32>() / valid_values.len() as f32;
        
        let median = if valid_values.len() % 2 == 0 {
            let mid = valid_values.len() / 2;
            (valid_values[mid - 1] + valid_values[mid]) / 2.0
        } else {
            valid_values[valid_values.len() / 2]
        };
        
        let variance = valid_values.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f32>() / valid_values.len() as f32;
        let std_dev = variance.sqrt();
        
        let percentile_25_idx = (valid_values.len() as f32 * 0.25) as usize;
        let percentile_75_idx = (valid_values.len() as f32 * 0.75) as usize;
        
        let percentile_25 = valid_values[percentile_25_idx.min(valid_values.len() - 1)];
        let percentile_75 = valid_values[percentile_75_idx.min(valid_values.len() - 1)];
        
        IndexStatistics {
            min,
            max,
            mean,
            median,
            std_dev,
            percentile_25,
            percentile_75,
            valid_pixels,
            total_pixels,
            coverage_percentage,
        }
    }

    /// Create a comprehensive analysis result
    pub fn create_analysis_result(
        &self,
        analysis_type: AnalysisType,
        source_images: Vec<Uuid>,
        output_path: String,
        statistics: IndexStatistics,
        processing_time_ms: u64,
        parameters: HashMap<String, serde_json::Value>,
        bands_used: Vec<String>,
    ) -> AnalysisResult {
        let quality_flags = vec![
            QualityFlag::AtmosphericCorrection(true),
            QualityFlag::SensorCalibration(true),
            QualityFlag::GeometricCorrection(true),
        ];

        let metadata = AnalysisMetadata {
            processing_time_ms,
            parameters,
            quality_flags,
            coordinate_system: "EPSG:4326".to_string(),
            spatial_resolution: 10.0, // meters
            bands_used,
        };

        AnalysisResult {
            analysis_id: Uuid::new_v4(),
            analysis_type,
            timestamp: Utc::now(),
            source_images,
            output_path,
            statistics,
            metadata,
        }
    }
}

impl Default for AnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}
