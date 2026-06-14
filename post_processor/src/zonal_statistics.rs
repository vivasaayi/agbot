use crate::evidence::{evidence_parameters, make_analysis_evidence};
use crate::AnalysisStatistics;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::schemas::{
    assert_raster_spatial_ref, GeoBounds, RasterResolution, RasterSpatialRef, RasterSpatialRefError,
};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductGrid {
    pub width: u32,
    pub height: u32,
    pub values: Vec<f32>,
    pub nodata_mask: Vec<bool>,
    pub spatial_ref: RasterSpatialRef,
}

#[derive(Debug, Clone)]
pub struct ProductGridStatistics {
    pub statistics: AnalysisStatistics,
    pub crs: String,
    pub extent: GeoBounds,
    pub resolution: RasterResolution,
    pub coverage_fraction: f32,
    pub nodata_pixel_count: u32,
    pub nodata_mask: Vec<bool>,
    pub evidence: crate::evidence::AnalysisEvidence,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ZonalStatisticsError {
    #[error("product grid dimensions do not match values/mask lengths: expected {expected}, values {values}, mask {mask}")]
    DimensionMismatch {
        expected: usize,
        values: usize,
        mask: usize,
    },
    #[error("product grid spatial metadata is invalid: {reason}")]
    SpatialRef { reason: RasterSpatialRefError },
    #[error("product grid value at index {index} is not finite")]
    InvalidValue { index: usize },
    #[error("product grid contains no valid data across {total_pixel_count} pixels")]
    NoValidData { total_pixel_count: u32 },
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] crate::evidence::AnalysisEvidenceError),
}

pub fn compute_zonal_statistics(
    grid: &ProductGrid,
    layer_ref: &str,
) -> Result<ProductGridStatistics, ZonalStatisticsError> {
    let expected = grid.width as usize * grid.height as usize;
    if grid.values.len() != expected || grid.nodata_mask.len() != expected {
        return Err(ZonalStatisticsError::DimensionMismatch {
            expected,
            values: grid.values.len(),
            mask: grid.nodata_mask.len(),
        });
    }

    let spatial_ref = assert_raster_spatial_ref(Some(&grid.spatial_ref), grid.width, grid.height)
        .map_err(|reason| ZonalStatisticsError::SpatialRef { reason })?;
    let mut valid_values = Vec::new();
    for (index, (value, is_nodata)) in grid.values.iter().zip(grid.nodata_mask.iter()).enumerate() {
        if *is_nodata {
            continue;
        }
        if !value.is_finite() {
            return Err(ZonalStatisticsError::InvalidValue { index });
        }
        valid_values.push(*value);
    }

    if valid_values.is_empty() {
        return Err(ZonalStatisticsError::NoValidData {
            total_pixel_count: expected as u32,
        });
    }

    valid_values.sort_by(|left, right| left.total_cmp(right));
    let valid_pixel_count = valid_values.len() as u32;
    let total_pixel_count = expected as u32;
    let nodata_pixel_count = total_pixel_count - valid_pixel_count;
    let min_value = valid_values[0];
    let max_value = *valid_values.last().expect("valid values are non-empty");
    let mean_value = valid_values.iter().sum::<f32>() / valid_values.len() as f32;
    let variance = valid_values
        .iter()
        .map(|value| {
            let delta = *value - mean_value;
            delta * delta
        })
        .sum::<f32>()
        / valid_values.len() as f32;
    let evidence = make_analysis_evidence(
        layer_ref,
        "zonal_statistics_v1",
        evidence_parameters(&[
            (
                "method",
                Value::String("compute_zonal_statistics".to_string()),
            ),
            ("include_nodata_mask", Value::Bool(true)),
            (
                "coverage_area_basis",
                Value::String("valid_pixels".to_string()),
            ),
            ("stats_precision", Value::String("f32".to_string())),
        ]),
        &(
            layer_ref,
            "zonal_statistics_v1",
            grid.width,
            grid.height,
            &grid.values,
            &grid.nodata_mask,
            &spatial_ref.crs,
            &spatial_ref.bbox,
            &spatial_ref.resolution,
            &spatial_ref.geo_transform,
        ),
    )?;
    let resolution = spatial_ref
        .resolution
        .expect("asserted spatial ref always has resolution");
    let pixel_area = (resolution.x * resolution.y) as f32;
    let coverage_area_m2 = valid_pixel_count as f32 * pixel_area;

    Ok(ProductGridStatistics {
        statistics: AnalysisStatistics {
            min_value,
            max_value,
            mean_value,
            std_deviation: variance.sqrt(),
            percentiles: percentiles(&valid_values),
            coverage_area_m2,
            valid_pixel_count,
            total_pixel_count,
        },
        crs: spatial_ref
            .crs
            .expect("asserted spatial ref always has CRS"),
        extent: spatial_ref
            .bbox
            .expect("asserted spatial ref always has extent"),
        resolution,
        coverage_fraction: valid_pixel_count as f32 / total_pixel_count as f32,
        nodata_pixel_count,
        nodata_mask: grid.nodata_mask.clone(),
        evidence,
    })
}

fn percentiles(sorted_values: &[f32]) -> HashMap<String, f32> {
    [0_u32, 25, 50, 75, 100]
        .into_iter()
        .map(|percentile| {
            let rank = ((percentile as f32 / 100.0) * sorted_values.len() as f32).floor() as usize;
            let index = rank.min(sorted_values.len() - 1);
            (percentile.to_string(), sorted_values[index])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::{GeoBounds, RasterResolution, RasterSpatialRef};

    #[test]
    fn computes_statistics_for_georeferenced_product_grid() {
        let grid = ProductGrid {
            width: 2,
            height: 2,
            values: vec![0.1, 0.3, 0.5, 0.9],
            nodata_mask: vec![false; 4],
            spatial_ref: spatial_ref(),
        };

        let result =
            compute_zonal_statistics(&grid, "layer:ndvi-2026-05-01").expect("statistics compute");

        assert_eq!(result.statistics.min_value, 0.1);
        assert_eq!(result.statistics.max_value, 0.9);
        assert!((result.statistics.mean_value - 0.45).abs() < 1.0e-6);
        assert!((result.statistics.std_deviation - 0.29580396).abs() < 1.0e-6);
        assert_eq!(result.statistics.percentiles["50"], 0.5);
        assert_eq!(result.statistics.valid_pixel_count, 4);
        assert_eq!(result.statistics.total_pixel_count, 4);
        assert_eq!(result.statistics.coverage_area_m2, 400.0);
        assert_eq!(result.nodata_pixel_count, 0);
        assert_eq!(result.coverage_fraction, 1.0);
        assert_eq!(result.crs, "EPSG:32614");
        assert_eq!(result.extent, extent());
        assert_eq!(result.resolution, RasterResolution { x: 10.0, y: 10.0 });
        assert_eq!(result.nodata_mask, vec![false; 4]);
        assert_eq!(result.evidence.layer_ref, "layer:ndvi-2026-05-01");
        assert_eq!(result.evidence.method, "zonal_statistics_v1");
        assert_eq!(
            result.evidence.parameters.get("coverage_area_basis"),
            Some(&Value::String("valid_pixels".to_string()))
        );
    }

    #[test]
    fn reproducible_statistics_emit_same_evidence_hash_for_same_inputs() {
        let grid = ProductGrid {
            width: 2,
            height: 2,
            values: vec![0.1, 0.3, 0.5, 0.9],
            nodata_mask: vec![false; 4],
            spatial_ref: spatial_ref(),
        };

        let first = compute_zonal_statistics(&grid, "layer:ndvi-2026-05-01").expect("first stats");
        let second =
            compute_zonal_statistics(&grid, "layer:ndvi-2026-05-01").expect("second stats");

        assert_eq!(first.evidence.input_hash, second.evidence.input_hash);
    }

    #[test]
    fn nodata_mask_is_excluded_from_statistics_and_coverage() {
        let grid = ProductGrid {
            width: 2,
            height: 2,
            values: vec![0.1, -9999.0, 0.5, 0.9],
            nodata_mask: vec![false, true, false, false],
            spatial_ref: spatial_ref(),
        };

        let result =
            compute_zonal_statistics(&grid, "layer:ndvi-2026-05-01").expect("statistics compute");

        assert_eq!(result.statistics.min_value, 0.1);
        assert_eq!(result.statistics.max_value, 0.9);
        assert!((result.statistics.mean_value - 0.5).abs() < 1.0e-6);
        assert_eq!(result.statistics.valid_pixel_count, 3);
        assert_eq!(result.statistics.total_pixel_count, 4);
        assert_eq!(result.statistics.coverage_area_m2, 300.0);
        assert_eq!(result.nodata_pixel_count, 1);
        assert_eq!(result.coverage_fraction, 0.75);
        assert_eq!(result.nodata_mask, vec![false, true, false, false]);
        assert_eq!(result.evidence.layer_ref, "layer:ndvi-2026-05-01");
    }

    #[test]
    fn all_nodata_grid_returns_explicit_no_valid_data_error() {
        let grid = ProductGrid {
            width: 2,
            height: 2,
            values: vec![-9999.0; 4],
            nodata_mask: vec![true; 4],
            spatial_ref: spatial_ref(),
        };

        let error = compute_zonal_statistics(&grid, "layer:ndvi-2026-05-01")
            .expect_err("all nodata is rejected");

        assert_eq!(
            error,
            ZonalStatisticsError::NoValidData {
                total_pixel_count: 4
            }
        );
    }

    fn spatial_ref() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32614".to_string()),
            bbox: Some(extent()),
            geo_transform: Some([500000.0, 10.0, 0.0, 4500020.0, 0.0, -10.0]),
            resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
        }
    }

    fn extent() -> GeoBounds {
        GeoBounds {
            min_lon: 500000.0,
            min_lat: 4500000.0,
            max_lon: 500020.0,
            max_lat: 4500020.0,
        }
    }
}
