use crate::zonal_statistics::{ProductGrid, ProductGridStatistics};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductAnomalyReasonCode {
    BelowAbsoluteThreshold,
    AboveAbsoluteThreshold,
    BelowStatisticalBand,
    AboveStatisticalBand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductAnomaly {
    pub index: usize,
    pub row: u32,
    pub col: u32,
    pub value: f32,
    pub threshold: f32,
    pub reason_code: ProductAnomalyReasonCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnomalyDetectionConfig {
    pub low_threshold: Option<f32>,
    pub high_threshold: Option<f32>,
    pub std_dev_multiplier: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AnomalyDetectionError {
    #[error("product grid dimensions do not match values/mask lengths: expected {expected}, values {values}, mask {mask}")]
    DimensionMismatch {
        expected: usize,
        values: usize,
        mask: usize,
    },
    #[error("std_dev_multiplier must be positive and finite")]
    InvalidStdDevMultiplier,
    #[error("product grid value at index {index} is not finite")]
    InvalidValue { index: usize },
}

pub fn flag_product_anomalies(
    grid: &ProductGrid,
    stats: &ProductGridStatistics,
    config: &AnomalyDetectionConfig,
) -> Result<Vec<ProductAnomaly>, AnomalyDetectionError> {
    let expected = grid.width as usize * grid.height as usize;
    if grid.values.len() != expected || grid.nodata_mask.len() != expected {
        return Err(AnomalyDetectionError::DimensionMismatch {
            expected,
            values: grid.values.len(),
            mask: grid.nodata_mask.len(),
        });
    }

    let statistical_thresholds = match config.std_dev_multiplier {
        Some(multiplier) if multiplier.is_finite() && multiplier > 0.0 => {
            let std_dev = stats.statistics.std_deviation;
            (std_dev > 0.0).then(|| {
                (
                    stats.statistics.mean_value - multiplier * std_dev,
                    stats.statistics.mean_value + multiplier * std_dev,
                )
            })
        }
        Some(_) => return Err(AnomalyDetectionError::InvalidStdDevMultiplier),
        None => None,
    };

    let mut flags = Vec::new();
    for (index, (value, is_nodata)) in grid.values.iter().zip(grid.nodata_mask.iter()).enumerate() {
        if *is_nodata {
            continue;
        }
        if !value.is_finite() {
            return Err(AnomalyDetectionError::InvalidValue { index });
        }

        if let Some(threshold) = config.low_threshold {
            if *value < threshold {
                flags.push(anomaly(
                    grid,
                    index,
                    *value,
                    threshold,
                    ProductAnomalyReasonCode::BelowAbsoluteThreshold,
                ));
            }
        }
        if let Some(threshold) = config.high_threshold {
            if *value > threshold {
                flags.push(anomaly(
                    grid,
                    index,
                    *value,
                    threshold,
                    ProductAnomalyReasonCode::AboveAbsoluteThreshold,
                ));
            }
        }

        if let Some((low_threshold, high_threshold)) = statistical_thresholds {
            if *value < low_threshold {
                flags.push(anomaly(
                    grid,
                    index,
                    *value,
                    low_threshold,
                    ProductAnomalyReasonCode::BelowStatisticalBand,
                ));
            } else if *value > high_threshold {
                flags.push(anomaly(
                    grid,
                    index,
                    *value,
                    high_threshold,
                    ProductAnomalyReasonCode::AboveStatisticalBand,
                ));
            }
        }
    }

    flags.sort_by(|left, right| {
        left.index
            .cmp(&right.index)
            .then_with(|| reason_rank(left.reason_code).cmp(&reason_rank(right.reason_code)))
    });
    Ok(flags)
}

fn anomaly(
    grid: &ProductGrid,
    index: usize,
    value: f32,
    threshold: f32,
    reason_code: ProductAnomalyReasonCode,
) -> ProductAnomaly {
    ProductAnomaly {
        index,
        row: (index / grid.width as usize) as u32,
        col: (index % grid.width as usize) as u32,
        value,
        threshold,
        reason_code,
    }
}

fn reason_rank(reason_code: ProductAnomalyReasonCode) -> u8 {
    match reason_code {
        ProductAnomalyReasonCode::BelowAbsoluteThreshold => 0,
        ProductAnomalyReasonCode::AboveAbsoluteThreshold => 1,
        ProductAnomalyReasonCode::BelowStatisticalBand => 2,
        ProductAnomalyReasonCode::AboveStatisticalBand => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zonal_statistics::{compute_zonal_statistics, ProductGrid};
    use shared::schemas::{GeoBounds, RasterResolution, RasterSpatialRef};

    #[test]
    fn absolute_threshold_flags_carry_reason_threshold_and_value() {
        let grid = product_grid(vec![0.1, 0.4, 0.8, 0.95]);
        let stats = compute_zonal_statistics(&grid).expect("stats compute");
        let config = AnomalyDetectionConfig {
            low_threshold: Some(0.2),
            high_threshold: Some(0.9),
            std_dev_multiplier: None,
        };

        let flags = flag_product_anomalies(&grid, &stats, &config).expect("flags compute");

        assert_eq!(flags.len(), 2);
        assert_eq!(flags[0].index, 0);
        assert_eq!(flags[0].row, 0);
        assert_eq!(flags[0].col, 0);
        assert_eq!(flags[0].value, 0.1);
        assert_eq!(flags[0].threshold, 0.2);
        assert_eq!(
            flags[0].reason_code,
            ProductAnomalyReasonCode::BelowAbsoluteThreshold
        );
        assert_eq!(flags[1].index, 3);
        assert_eq!(flags[1].value, 0.95);
        assert_eq!(flags[1].threshold, 0.9);
        assert_eq!(
            flags[1].reason_code,
            ProductAnomalyReasonCode::AboveAbsoluteThreshold
        );
    }

    #[test]
    fn statistical_outliers_use_mean_and_std_dev_thresholds() {
        let grid = ProductGrid {
            width: 5,
            height: 1,
            values: vec![0.1, 0.5, 0.5, 0.5, 0.9],
            nodata_mask: vec![false; 5],
            spatial_ref: spatial_ref(5, 1),
        };
        let stats = compute_zonal_statistics(&grid).expect("stats compute");
        let config = AnomalyDetectionConfig {
            low_threshold: None,
            high_threshold: None,
            std_dev_multiplier: Some(1.0),
        };

        let flags = flag_product_anomalies(&grid, &stats, &config).expect("flags compute");

        assert_eq!(flags.len(), 2);
        assert_eq!(
            flags[0].reason_code,
            ProductAnomalyReasonCode::BelowStatisticalBand
        );
        assert!((flags[0].threshold - 0.2470178).abs() < 1.0e-6);
        assert_eq!(flags[0].value, 0.1);
        assert_eq!(
            flags[1].reason_code,
            ProductAnomalyReasonCode::AboveStatisticalBand
        );
        assert!((flags[1].threshold - 0.7529822).abs() < 1.0e-6);
        assert_eq!(flags[1].value, 0.9);
    }

    #[test]
    fn uniform_raster_returns_no_statistical_false_positives() {
        let grid = product_grid(vec![0.5, 0.5, 0.5, 0.5]);
        let stats = compute_zonal_statistics(&grid).expect("stats compute");
        let config = AnomalyDetectionConfig {
            low_threshold: None,
            high_threshold: None,
            std_dev_multiplier: Some(1.0),
        };

        let flags = flag_product_anomalies(&grid, &stats, &config).expect("flags compute");

        assert!(flags.is_empty());
    }

    fn product_grid(values: Vec<f32>) -> ProductGrid {
        ProductGrid {
            width: 2,
            height: 2,
            values,
            nodata_mask: vec![false; 4],
            spatial_ref: spatial_ref(2, 2),
        }
    }

    fn spatial_ref(width: u32, height: u32) -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32614".to_string()),
            bbox: Some(GeoBounds {
                min_lon: 500000.0,
                min_lat: 4500000.0,
                max_lon: 500000.0 + width as f64 * 10.0,
                max_lat: 4500000.0 + height as f64 * 10.0,
            }),
            geo_transform: Some([
                500000.0,
                10.0,
                0.0,
                4500000.0 + height as f64 * 10.0,
                0.0,
                -10.0,
            ]),
            resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
        }
    }
}
