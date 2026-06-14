use crate::evidence::{evidence_parameters, make_analysis_evidence};
use crate::zonal_statistics::{
    compute_zonal_statistics, ProductGrid, ProductGridStatistics, ZonalStatisticsError,
};
use crate::HealthUncertaintyBand;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared::schemas::{
    assert_raster_spatial_ref, GeoBounds, RasterResolution, RasterSpatialRef,
    RasterSpatialRefError, GEO_EXTENT_ASSERTION_TOLERANCE, RASTER_RESOLUTION_RELATIVE_TOLERANCE,
};

pub const INDEX_TREND_FEATURE_FLAG_KEY: &str = "index_trend_feature_enabled";
pub const INDEX_TREND_PAYLOAD_KEY: &str = "index_trend_payload";
const INDEX_TREND_METHOD: &str = "index_trend_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexTrendCalibrationStatus {
    CalibratedReflectance,
    UncalibratedDn,
}

impl IndexTrendCalibrationStatus {
    fn is_calibrated(&self) -> bool {
        matches!(self, Self::CalibratedReflectance)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexTrendSnapshotPayload {
    pub field_id: String,
    pub scene_id: String,
    pub product_ref: String,
    pub acquired_at: DateTime<Utc>,
    pub grid: ProductGrid,
    pub calibration_status: IndexTrendCalibrationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexTrendRequest {
    pub snapshots: Vec<IndexTrendSnapshotPayload>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IndexTrendDecision {
    Available { reasons: Vec<String> },
    LowConfidence { reasons: Vec<String> },
    Unavailable { reasons: Vec<String> },
}

impl IndexTrendDecision {
    pub fn reasons(&self) -> &[String] {
        match self {
            Self::Available { reasons }
            | Self::LowConfidence { reasons }
            | Self::Unavailable { reasons } => reasons,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Available { .. } => "available",
            Self::LowConfidence { .. } => "low_confidence",
            Self::Unavailable { .. } => "unavailable",
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexTrendResult {
    pub field_id: String,
    pub current_scene_id: String,
    pub previous_scene_id: String,
    pub decision: IndexTrendDecision,
    pub delta_layer_ref: String,
    pub width: u32,
    pub height: u32,
    pub common_crs: String,
    pub common_extent: GeoBounds,
    pub common_resolution: RasterResolution,
    pub coverage_fraction: f32,
    pub delta_values: Option<Vec<f32>>,
    pub delta_nodata_mask: Option<Vec<bool>>,
    pub uncertainty: HealthUncertaintyBand,
    pub evidence_input_hash: String,
    pub delta_statistics: Option<ProductGridStatistics>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum IndexTrendError {
    #[error("index trend requires exactly two snapshots: {provided} provided")]
    InvalidSnapshotCount { provided: usize },
    #[error("a required field is missing: {field}")]
    MissingField { field: &'static str },
    #[error("index trend snapshots must be for the same field: {left} vs {right}")]
    FieldMismatch { left: String, right: String },
    #[error("snapshot {index} has invalid spatial reference: {reason}")]
    SpatialReference {
        index: usize,
        reason: RasterSpatialRefError,
    },
    #[error("index trend comparison has invalid input: {reason}")]
    InvalidInput { reason: String },
    #[error("evidence generation failed: {0}")]
    Evidence(#[from] crate::evidence::AnalysisEvidenceError),
    #[error("zonal statistics failed: {0}")]
    ZonalStatistics(#[from] ZonalStatisticsError),
}

pub fn analyze_index_trend(
    mut request: IndexTrendRequest,
) -> Result<IndexTrendResult, IndexTrendError> {
    if request.snapshots.len() != 2 {
        return Err(IndexTrendError::InvalidSnapshotCount {
            provided: request.snapshots.len(),
        });
    }

    request
        .snapshots
        .sort_by_key(|snapshot| snapshot.acquired_at);
    let earliest = request.snapshots.remove(0);
    let latest = request.snapshots.remove(0);
    if earliest.acquired_at == latest.acquired_at {
        return Err(IndexTrendError::InvalidInput {
            reason: "snapshot acquisition times must not be equal".to_string(),
        });
    }

    assert_snapshot(&earliest)?;
    assert_snapshot(&latest)?;
    let earliest_spatial = assert_snapshot_spatial_ref(&earliest, 0)?;
    let latest_spatial = assert_snapshot_spatial_ref(&latest, 1)?;

    if earliest.field_id != latest.field_id {
        return Err(IndexTrendError::FieldMismatch {
            left: earliest.field_id,
            right: latest.field_id,
        });
    }

    let mut mismatch_reasons = Vec::new();
    if earliest_spatial.crs != latest_spatial.crs {
        mismatch_reasons.push("crs mismatch".to_string());
    }
    if !extent_matches(
        earliest_spatial
            .bbox
            .as_ref()
            .ok_or_else(|| IndexTrendError::InvalidInput {
                reason: "missing bbox on earliest snapshot".to_string(),
            })?,
        latest_spatial
            .bbox
            .as_ref()
            .ok_or_else(|| IndexTrendError::InvalidInput {
                reason: "missing bbox on latest snapshot".to_string(),
            })?,
    ) {
        mismatch_reasons.push("extent mismatch".to_string());
    }
    if !dimensions_match(&earliest.grid, &latest.grid) {
        mismatch_reasons.push("dimension mismatch".to_string());
    }
    if !resolution_matches(
        earliest_spatial
            .resolution
            .ok_or_else(|| IndexTrendError::InvalidInput {
                reason: "missing resolution on earliest snapshot".to_string(),
            })?,
        latest_spatial
            .resolution
            .ok_or_else(|| IndexTrendError::InvalidInput {
                reason: "missing resolution on latest snapshot".to_string(),
            })?,
    ) {
        mismatch_reasons.push("resolution mismatch".to_string());
    }

    let decision = if !mismatch_reasons.is_empty() {
        IndexTrendDecision::Unavailable {
            reasons: mismatch_reasons,
        }
    } else if !earliest.calibration_status.is_calibrated()
        || !latest.calibration_status.is_calibrated()
    {
        IndexTrendDecision::LowConfidence {
            reasons: vec!["calibration unavailable".to_string()],
        }
    } else {
        IndexTrendDecision::Available {
            reasons: Vec::new(),
        }
    };

    let delta_layer_ref = format!(
        "index-trend:{}:{}:{}",
        earliest.scene_id, latest.scene_id, earliest.field_id
    );
    let coverage = common_coverage(&latest.grid);
    let common_crs = earliest_spatial.crs.unwrap_or_default();
    let common_extent =
        earliest_spatial
            .bbox
            .clone()
            .ok_or_else(|| IndexTrendError::InvalidInput {
                reason: "missing extent on earliest snapshot".to_string(),
            })?;
    let common_resolution =
        earliest_spatial
            .resolution
            .ok_or_else(|| IndexTrendError::InvalidInput {
                reason: "missing resolution on earliest snapshot".to_string(),
            })?;

    let (uncertainty, coverage_fraction, delta_values, delta_mask, delta_statistics) =
        match &decision {
            IndexTrendDecision::Available { .. } => {
                let delta_grid = compute_delta_grid(&latest.grid, &earliest.grid)?;
                match compute_zonal_statistics(&delta_grid, &delta_layer_ref) {
                    Ok(stats) => {
                        let coverage_fraction = stats.coverage_fraction;
                        (
                            uncertainty_for_available(
                                stats.statistics.mean_value,
                                coverage_fraction,
                            ),
                            coverage_fraction,
                            Some(delta_grid.values),
                            Some(delta_grid.nodata_mask),
                            Some(stats),
                        )
                    }
                    Err(ZonalStatisticsError::NoValidData { .. }) => {
                        return Ok(IndexTrendResult {
                            field_id: earliest.field_id,
                            current_scene_id: latest.scene_id,
                            previous_scene_id: earliest.scene_id,
                            decision: IndexTrendDecision::Unavailable {
                                reasons: vec!["no valid overlap pixels".to_string()],
                            },
                            delta_layer_ref: delta_layer_ref.clone(),
                            width: latest.grid.width,
                            height: latest.grid.height,
                            common_crs,
                            common_extent,
                            common_resolution,
                            coverage_fraction: 0.0,
                            delta_values: None,
                            delta_nodata_mask: None,
                            uncertainty: uncertainty_for_unavailable(&[
                                "no valid overlap pixels".to_string()
                            ]),
                            evidence_input_hash: String::new(),
                            delta_statistics: None,
                        });
                    }
                    Err(error) => return Err(IndexTrendError::ZonalStatistics(error)),
                }
            }
            _ => (
                uncertainty_for_unavailable(decision.reasons()),
                coverage,
                None,
                None,
                None,
            ),
        };

    let evidence = make_analysis_evidence(
        &delta_layer_ref,
        INDEX_TREND_METHOD,
        evidence_parameters(&[
            ("decision", Value::String(decision.label().to_string())),
            ("decision_reason_count", json!(decision.reasons().len())),
            (
                "decision_reasons",
                Value::Array(
                    decision
                        .reasons()
                        .iter()
                        .map(|reason| Value::String(reason.clone()))
                        .collect(),
                ),
            ),
            ("coverage_fraction", json!(coverage_fraction)),
        ]),
        &(
            &delta_layer_ref,
            &decision,
            &earliest.scene_id,
            &latest.scene_id,
            coverage_fraction,
            &common_crs,
            decision.reasons(),
        ),
    )?;

    Ok(IndexTrendResult {
        field_id: earliest.field_id,
        current_scene_id: latest.scene_id,
        previous_scene_id: earliest.scene_id,
        decision,
        delta_layer_ref,
        width: latest.grid.width,
        height: latest.grid.height,
        common_crs,
        common_extent,
        common_resolution,
        coverage_fraction,
        delta_values,
        delta_nodata_mask: delta_mask,
        uncertainty,
        evidence_input_hash: evidence.input_hash,
        delta_statistics,
    })
}

fn assert_snapshot(snapshot: &IndexTrendSnapshotPayload) -> Result<(), IndexTrendError> {
    require_text(&snapshot.field_id, "field_id")?;
    require_text(&snapshot.scene_id, "scene_id")?;
    require_text(&snapshot.product_ref, "product_ref")?;
    if snapshot.acquired_at.timestamp_millis() == 0 {
        return Err(IndexTrendError::InvalidInput {
            reason: "snapshot acquisition time is required".to_string(),
        });
    }
    Ok(())
}

fn assert_snapshot_spatial_ref(
    snapshot: &IndexTrendSnapshotPayload,
    index: usize,
) -> Result<RasterSpatialRef, IndexTrendError> {
    let spatial_ref = assert_raster_spatial_ref(
        Some(&snapshot.grid.spatial_ref),
        snapshot.grid.width,
        snapshot.grid.height,
    )
    .map_err(|error| IndexTrendError::SpatialReference {
        index,
        reason: error,
    })?;

    Ok(spatial_ref)
}

fn require_text(value: &str, field: &'static str) -> Result<(), IndexTrendError> {
    if value.trim().is_empty() {
        Err(IndexTrendError::MissingField { field })
    } else {
        Ok(())
    }
}

fn dimensions_match(left: &ProductGrid, right: &ProductGrid) -> bool {
    left.width == right.width && left.height == right.height
}

fn extent_matches(left: &GeoBounds, right: &GeoBounds) -> bool {
    approx(left.min_lon, right.min_lon, GEO_EXTENT_ASSERTION_TOLERANCE)
        && approx(left.max_lon, right.max_lon, GEO_EXTENT_ASSERTION_TOLERANCE)
        && approx(left.min_lat, right.min_lat, GEO_EXTENT_ASSERTION_TOLERANCE)
        && approx(left.max_lat, right.max_lat, GEO_EXTENT_ASSERTION_TOLERANCE)
}

fn resolution_matches(left: RasterResolution, right: RasterResolution) -> bool {
    relative_within_tolerance(left.x, right.x, RASTER_RESOLUTION_RELATIVE_TOLERANCE)
        && relative_within_tolerance(left.y, right.y, RASTER_RESOLUTION_RELATIVE_TOLERANCE)
}

fn approx(left: f64, right: f64, tolerance: f64) -> bool {
    (left - right).abs() <= tolerance
}

fn relative_within_tolerance(left: f64, right: f64, tolerance: f64) -> bool {
    let denominator = right.abs().max(1e-9);
    ((left - right).abs() / denominator) <= tolerance
}

fn compute_delta_grid(
    current: &ProductGrid,
    previous: &ProductGrid,
) -> Result<ProductGrid, IndexTrendError> {
    let expected = current.width as usize * current.height as usize;
    if current.width != previous.width || current.height != previous.height {
        return Err(IndexTrendError::InvalidInput {
            reason: "snapshot dimensions mismatch".to_string(),
        });
    }
    if current.values.len() != expected || previous.values.len() != expected {
        return Err(IndexTrendError::InvalidInput {
            reason: "grid value count does not match dimensions".to_string(),
        });
    }
    if current.nodata_mask.len() != expected || previous.nodata_mask.len() != expected {
        return Err(IndexTrendError::InvalidInput {
            reason: "nodata mask count does not match dimensions".to_string(),
        });
    }

    let mut values = Vec::with_capacity(expected);
    let mut nodata_mask = Vec::with_capacity(expected);
    for index in 0..expected {
        let current_value = current.values[index];
        let previous_value = previous.values[index];
        let current_nodata = current.nodata_mask[index];
        let previous_nodata = previous.nodata_mask[index];

        if current_nodata || previous_nodata {
            values.push(0.0);
            nodata_mask.push(true);
            continue;
        }
        if !current_value.is_finite() || !previous_value.is_finite() {
            return Err(IndexTrendError::InvalidInput {
                reason: format!("invalid index value at {index}"),
            });
        }

        values.push(current_value - previous_value);
        nodata_mask.push(false);
    }

    Ok(ProductGrid {
        width: current.width,
        height: current.height,
        values,
        nodata_mask,
        spatial_ref: current.spatial_ref.clone(),
    })
}

fn common_coverage(grid: &ProductGrid) -> f32 {
    let total = grid.width as f32 * grid.height as f32;
    if total <= 0.0 {
        return 0.0;
    }
    let valid = grid
        .nodata_mask
        .iter()
        .filter(|is_nodata| !**is_nodata)
        .count() as f32;
    valid / total
}

fn uncertainty_for_available(mean: f32, coverage_fraction: f32) -> HealthUncertaintyBand {
    let span = ((1.0 - coverage_fraction).clamp(0.0, 1.0) * 0.25) + 0.05;
    HealthUncertaintyBand {
        lower: mean - span,
        upper: mean + span,
    }
}

fn uncertainty_for_unavailable(reasons: &[String]) -> HealthUncertaintyBand {
    let span = (0.75 + (reasons.len() as f32 * 0.1)).clamp(0.75, 1.5);
    HealthUncertaintyBand {
        lower: -span,
        upper: span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::RasterSpatialRef;

    #[test]
    fn comparable_calibrated_scenes_produce_delta_and_uncertainty() {
        let result = analyze_index_trend(IndexTrendRequest {
            snapshots: vec![baseline_snapshot(), current_snapshot()],
        })
        .expect("index trend can be compared");

        assert_eq!(result.field_id, "field-01");
        assert_eq!(result.decision.label(), "available");
        let stats = result
            .delta_statistics
            .expect("available result has delta statistics");
        assert_eq!(stats.statistics.valid_pixel_count, 2);
        assert_eq!(stats.statistics.coverage_area_m2 > 0.0, true);
        assert!(result.uncertainty.upper > result.uncertainty.lower);
        assert!(!result.evidence_input_hash.is_empty());
        assert!(result.delta_values.as_ref().is_some());
    }

    #[test]
    fn extent_mismatch_marks_unavailable_not_differenced() {
        let mut latest = current_snapshot();
        latest.grid.spatial_ref.bbox = Some(GeoBounds {
            min_lon: 500001.0,
            min_lat: 4500000.0,
            max_lon: 500021.0,
            max_lat: 4500020.0,
        });
        latest.grid.spatial_ref.geo_transform = Some([
            500001.0,
            latest
                .grid
                .spatial_ref
                .resolution
                .as_ref()
                .expect("resolution set")
                .x,
            0.0,
            4500020.0,
            0.0,
            -latest
                .grid
                .spatial_ref
                .resolution
                .as_ref()
                .expect("resolution set")
                .y,
        ]);

        let result = analyze_index_trend(IndexTrendRequest {
            snapshots: vec![baseline_snapshot(), latest],
        })
        .expect("request parses");

        assert_eq!(result.decision.label(), "unavailable");
        assert!(result.delta_values.is_none());
        assert!(result
            .decision
            .reasons()
            .iter()
            .any(|reason| reason.contains("extent")));
    }

    #[test]
    fn uncalibrated_snapshot_marks_low_confidence() {
        let mut earliest = baseline_snapshot();
        earliest.calibration_status = IndexTrendCalibrationStatus::UncalibratedDn;

        let result = analyze_index_trend(IndexTrendRequest {
            snapshots: vec![earliest, current_snapshot()],
        })
        .expect("request parses");

        assert_eq!(result.decision.label(), "low_confidence");
        assert_eq!(
            result.decision.reasons(),
            &["calibration unavailable".to_string()]
        );
        assert!(result.delta_values.is_none());
    }

    #[test]
    fn invalid_snapshot_count_is_rejected() {
        let request = IndexTrendRequest {
            snapshots: vec![baseline_snapshot()],
        };

        let error = analyze_index_trend(request).expect_err("need two snapshots");
        assert!(matches!(
            error,
            IndexTrendError::InvalidSnapshotCount { provided: 1 }
        ));
    }

    fn baseline_snapshot() -> IndexTrendSnapshotPayload {
        IndexTrendSnapshotPayload {
            field_id: "field-01".to_string(),
            scene_id: "scene-2026-05-01".to_string(),
            product_ref: "layer:ndvi-2026-05-01".to_string(),
            acquired_at: DateTime::from_timestamp(1714531200, 0).expect("valid timestamp"),
            grid: ProductGrid {
                width: 2,
                height: 2,
                values: vec![0.1, 0.3, 0.5, 0.8],
                nodata_mask: vec![false, false, true, false],
                spatial_ref: base_spatial_ref(500000.0, 4500000.0, 10.0),
            },
            calibration_status: IndexTrendCalibrationStatus::CalibratedReflectance,
        }
    }

    fn current_snapshot() -> IndexTrendSnapshotPayload {
        IndexTrendSnapshotPayload {
            field_id: "field-01".to_string(),
            scene_id: "scene-2026-06-01".to_string(),
            product_ref: "layer:ndvi-2026-06-01".to_string(),
            acquired_at: DateTime::from_timestamp(1717209600, 0).expect("valid timestamp"),
            grid: ProductGrid {
                width: 2,
                height: 2,
                values: vec![0.25, 0.35, 0.5, 0.75],
                nodata_mask: vec![false, true, false, false],
                spatial_ref: base_spatial_ref(500000.0, 4500000.0, 10.0),
            },
            calibration_status: IndexTrendCalibrationStatus::CalibratedReflectance,
        }
    }

    fn base_spatial_ref(origin_lon: f64, origin_lat: f64, resolution: f64) -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32614".to_string()),
            bbox: Some(GeoBounds {
                min_lon: origin_lon,
                min_lat: origin_lat,
                max_lon: origin_lon + resolution * 2.0,
                max_lat: origin_lat + resolution * 2.0,
            }),
            geo_transform: Some([
                origin_lon,
                resolution,
                0.0,
                origin_lat + 20.0,
                0.0,
                -resolution,
            ]),
            resolution: Some(RasterResolution {
                x: resolution,
                y: resolution,
            }),
        }
    }
}
