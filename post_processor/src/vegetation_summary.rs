use crate::zonal_statistics::{compute_zonal_statistics, ProductGrid};
use crate::AnalysisStatistics;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::schemas::{GeoBounds, RasterResolution};
use timeseries::{
    evaluate_series_cadence_health, MetricDefinition, MetricKind, SeriesCadenceHealthConfig,
    SeriesFreshnessState, SeriesPoint, SeriesValue, TimeRange, TimeSeriesEngine, ZonalTrendConfig,
    ZonalTrendTarget,
};

pub const DEFAULT_LOW_VIGOR_NDVI_THRESHOLD: f32 = 0.35;

#[derive(Debug, Clone)]
pub struct VegetationSummaryInput {
    pub field_id: String,
    pub scene_id: String,
    pub product_ref: String,
    pub acquired_at: DateTime<Utc>,
    pub grid: ProductGrid,
    pub low_vigor_threshold: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VegetationSummary {
    pub field_id: String,
    pub scene_id: String,
    pub product_ref: String,
    pub acquired_at: DateTime<Utc>,
    pub source_product: VegetationSourceProduct,
    pub statistics: AnalysisStatistics,
    pub crs: String,
    pub extent: GeoBounds,
    pub resolution: RasterResolution,
    pub coverage_fraction: f32,
    pub nodata_pixel_count: u32,
    pub low_vigor_threshold: f32,
    pub low_vigor_fraction: f32,
    pub trend: VegetationTrend,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VegetationSourceProduct {
    pub product_ref: String,
    pub scene_id: String,
    pub acquired_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum VegetationTrend {
    NoBaseline {
        reason: String,
    },
    Delta {
        baseline_scene_id: String,
        baseline_product_ref: String,
        baseline_acquired_at: DateTime<Utc>,
        mean_ndvi_delta: f32,
        low_vigor_fraction_delta: f32,
        evidence_refs: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum VegetationSummaryError {
    #[error("{field} is required")]
    MissingField { field: &'static str },
    #[error("low_vigor_threshold must be finite")]
    InvalidLowVigorThreshold,
    #[error("vegetation summary statistics failed: {0}")]
    Statistics(#[from] crate::ZonalStatisticsError),
}

pub fn summarize_vegetation(
    input: VegetationSummaryInput,
    prior: Option<&VegetationSummary>,
) -> Result<VegetationSummary, VegetationSummaryError> {
    require_text(&input.field_id, "field_id")?;
    require_text(&input.scene_id, "scene_id")?;
    require_text(&input.product_ref, "product_ref")?;
    if !input.low_vigor_threshold.is_finite() {
        return Err(VegetationSummaryError::InvalidLowVigorThreshold);
    }

    let stats = compute_zonal_statistics(&input.grid, &input.product_ref)?;
    let low_vigor_fraction = low_vigor_fraction(&input.grid, input.low_vigor_threshold);
    let source_product = VegetationSourceProduct {
        product_ref: input.product_ref.clone(),
        scene_id: input.scene_id.clone(),
        acquired_at: input.acquired_at,
    };
    let trend = vegetation_trend_from_timeseries(
        &input,
        &stats.crs,
        &stats.extent,
        stats.statistics.mean_value,
        low_vigor_fraction,
        prior,
    );

    Ok(VegetationSummary {
        field_id: input.field_id,
        scene_id: input.scene_id,
        product_ref: input.product_ref,
        acquired_at: input.acquired_at,
        source_product,
        statistics: stats.statistics,
        crs: stats.crs,
        extent: stats.extent,
        resolution: stats.resolution,
        coverage_fraction: stats.coverage_fraction,
        nodata_pixel_count: stats.nodata_pixel_count,
        low_vigor_threshold: input.low_vigor_threshold,
        low_vigor_fraction,
        trend,
    })
}

fn require_text(value: &str, field: &'static str) -> Result<(), VegetationSummaryError> {
    if value.trim().is_empty() {
        Err(VegetationSummaryError::MissingField { field })
    } else {
        Ok(())
    }
}

fn low_vigor_fraction(grid: &ProductGrid, threshold: f32) -> f32 {
    let mut valid = 0_usize;
    let mut low = 0_usize;
    for (value, is_nodata) in grid.values.iter().zip(grid.nodata_mask.iter()) {
        if *is_nodata {
            continue;
        }
        valid += 1;
        if *value < threshold {
            low += 1;
        }
    }

    if valid == 0 {
        0.0
    } else {
        low as f32 / valid as f32
    }
}

fn is_comparable_baseline(
    input: &VegetationSummaryInput,
    current_crs: &str,
    current_extent: &GeoBounds,
    baseline: &VegetationSummary,
) -> bool {
    input.field_id == baseline.field_id
        && current_crs == baseline.crs
        && current_extent == &baseline.extent
        && input.acquired_at > baseline.acquired_at
}

fn vegetation_trend_from_timeseries(
    input: &VegetationSummaryInput,
    current_crs: &str,
    current_extent: &GeoBounds,
    mean_ndvi: f32,
    low_vigor_fraction: f32,
    prior: Option<&VegetationSummary>,
) -> VegetationTrend {
    let Some(baseline) = prior else {
        return VegetationTrend::NoBaseline {
            reason: timeseries_no_baseline_reason(input),
        };
    };
    if !is_comparable_baseline(input, current_crs, current_extent, baseline) {
        return VegetationTrend::NoBaseline {
            reason: "prior scene is not comparable".to_string(),
        };
    }

    let mut engine = TimeSeriesEngine::default();
    if engine
        .register_metric(MetricDefinition {
            metric: "ndvi_mean".to_string(),
            unit: "index".to_string(),
            kind: MetricKind::Scalar,
            expected_cadence: "per_flight".to_string(),
        })
        .and_then(|_| {
            engine.append(ndvi_mean_point(
                &input.field_id,
                baseline.acquired_at,
                baseline.statistics.mean_value,
                &baseline.product_ref,
            ))
        })
        .and_then(|_| {
            engine.append(ndvi_mean_point(
                &input.field_id,
                input.acquired_at,
                mean_ndvi,
                &input.product_ref,
            ))
        })
        .is_err()
    {
        return VegetationTrend::NoBaseline {
            reason: "timeseries trend unavailable".to_string(),
        };
    }

    let page = engine.query(timeseries::SeriesQuery {
        entity_ref: format!("field:{}", input.field_id),
        metric: "ndvi_mean".to_string(),
        range: TimeRange::default(),
        limit: None,
        cursor: None,
    });
    let Some(baseline_point) = page.points.first() else {
        return VegetationTrend::NoBaseline {
            reason: "no baseline".to_string(),
        };
    };
    let SeriesValue::Scalar {
        value: baseline_value,
    } = baseline_point.value
    else {
        return VegetationTrend::NoBaseline {
            reason: "timeseries trend unavailable".to_string(),
        };
    };
    let trend = match engine.compute_zonal_trend(
        ZonalTrendTarget {
            entity_ref: format!("field:{}", input.field_id),
            metric: "ndvi_mean".to_string(),
            zone_ref: format!("field:{}", input.field_id),
            zone_crs: current_crs.to_string(),
            range: TimeRange::default(),
        },
        ZonalTrendConfig {
            min_points: 2,
            flat_slope_epsilon: 0.000001,
        },
    ) {
        Ok(trend) => trend,
        Err(_) => {
            return VegetationTrend::NoBaseline {
                reason: "no baseline".to_string(),
            };
        }
    };

    VegetationTrend::Delta {
        baseline_scene_id: baseline.scene_id.clone(),
        baseline_product_ref: baseline.product_ref.clone(),
        baseline_acquired_at: baseline.acquired_at,
        mean_ndvi_delta: mean_ndvi - baseline_value as f32,
        low_vigor_fraction_delta: low_vigor_fraction - baseline.low_vigor_fraction,
        evidence_refs: trend.evidence_refs,
    }
}

fn timeseries_no_baseline_reason(input: &VegetationSummaryInput) -> String {
    let health = evaluate_series_cadence_health(
        &[],
        format!("field:{}", input.field_id),
        "ndvi_mean".to_string(),
        input.acquired_at.to_rfc3339(),
        SeriesCadenceHealthConfig {
            expected_cadence_days: 14,
            stale_after_days: 30,
        },
    );
    match health.map(|report| report.state) {
        Ok(SeriesFreshnessState::NoBaseline) => "no baseline".to_string(),
        _ => "timeseries trend unavailable".to_string(),
    }
}

fn ndvi_mean_point(
    field_id: &str,
    acquired_at: DateTime<Utc>,
    value: f32,
    source_ref: &str,
) -> SeriesPoint {
    SeriesPoint {
        entity_ref: format!("field:{field_id}"),
        metric: "ndvi_mean".to_string(),
        unit: "index".to_string(),
        t: acquired_at.to_rfc3339(),
        value: SeriesValue::Scalar {
            value: f64::from(value),
        },
        source_ref: source_ref.to_string(),
        created_at: acquired_at.to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::{GeoBounds, RasterResolution, RasterSpatialRef};

    #[test]
    fn vegetation_summary_computes_low_vigor_fraction_and_delta() {
        let baseline = summarize_vegetation(
            input(
                "scene-2026-05-01",
                "ndvi-2026-05-01",
                "2026-05-01T00:00:00Z",
                vec![0.2, 0.5, 0.6, 0.8],
                vec![false; 4],
            ),
            None,
        )
        .expect("baseline summary");

        let summary = summarize_vegetation(
            input(
                "scene-2026-05-15",
                "ndvi-2026-05-15",
                "2026-05-15T00:00:00Z",
                vec![0.1, 0.2, 0.5, 0.7],
                vec![false; 4],
            ),
            Some(&baseline),
        )
        .expect("current summary");

        assert_eq!(summary.source_product.product_ref, "ndvi-2026-05-15");
        assert_eq!(summary.source_product.scene_id, "scene-2026-05-15");
        assert!((summary.statistics.mean_value - 0.375).abs() < 1.0e-6);
        assert_eq!(summary.statistics.valid_pixel_count, 4);
        assert_eq!(summary.low_vigor_fraction, 0.5);
        assert!(matches!(
            summary.trend,
            VegetationTrend::Delta {
                ref baseline_scene_id,
                ref baseline_product_ref,
                mean_ndvi_delta,
                low_vigor_fraction_delta,
                ref evidence_refs,
                ..
            } if baseline_scene_id == "scene-2026-05-01"
                && baseline_product_ref == "ndvi-2026-05-01"
                && (mean_ndvi_delta - -0.15).abs() < 1.0e-6
                && (low_vigor_fraction_delta - 0.25).abs() < 1.0e-6
                && evidence_refs == &vec![
                    "ndvi-2026-05-01".to_string(),
                    "ndvi-2026-05-15".to_string()
                ]
        ));
    }

    #[test]
    fn vegetation_summary_marks_no_baseline_when_prior_is_absent() {
        let summary = summarize_vegetation(
            input(
                "scene-2026-05-15",
                "ndvi-2026-05-15",
                "2026-05-15T00:00:00Z",
                vec![0.1, 0.2, 0.8, 0.9],
                vec![false, false, false, true],
            ),
            None,
        )
        .expect("summary without baseline");

        assert_eq!(summary.statistics.valid_pixel_count, 3);
        assert_eq!(summary.nodata_pixel_count, 1);
        assert!((summary.low_vigor_fraction - (2.0 / 3.0)).abs() < 1.0e-6);
        assert_eq!(
            summary.trend,
            VegetationTrend::NoBaseline {
                reason: "no baseline".to_string()
            }
        );
    }

    #[test]
    fn vegetation_summary_refuses_empty_source_product() {
        let mut request = input(
            "scene-2026-05-15",
            "ndvi-2026-05-15",
            "2026-05-15T00:00:00Z",
            vec![0.4, 0.5, 0.6, 0.7],
            vec![false; 4],
        );
        request.product_ref.clear();

        let error = summarize_vegetation(request, None).expect_err("product ref is required");

        assert_eq!(
            error,
            VegetationSummaryError::MissingField {
                field: "product_ref"
            }
        );
    }

    fn input(
        scene_id: &str,
        product_ref: &str,
        acquired_at: &str,
        values: Vec<f32>,
        nodata_mask: Vec<bool>,
    ) -> VegetationSummaryInput {
        VegetationSummaryInput {
            field_id: "field-a".to_string(),
            scene_id: scene_id.to_string(),
            product_ref: product_ref.to_string(),
            acquired_at: DateTime::parse_from_rfc3339(acquired_at)
                .expect("valid time")
                .with_timezone(&Utc),
            grid: ProductGrid {
                width: 2,
                height: 2,
                values,
                nodata_mask,
                spatial_ref: spatial_ref(),
            },
            low_vigor_threshold: DEFAULT_LOW_VIGOR_NDVI_THRESHOLD,
        }
    }

    fn spatial_ref() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32614".to_string()),
            bbox: Some(GeoBounds {
                min_lon: 500000.0,
                min_lat: 4500000.0,
                max_lon: 500020.0,
                max_lat: 4500020.0,
            }),
            geo_transform: Some([500000.0, 10.0, 0.0, 4500020.0, 0.0, -10.0]),
            resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
        }
    }
}
