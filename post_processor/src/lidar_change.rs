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

pub const LIDAR_CHANGE_FEATURE_FLAG_KEY: &str = "lidar_change_advisory_feature_enabled";
pub const LIDAR_CHANGE_PAYLOAD_KEY: &str = "lidar_change_advisory_payload";
const LIDAR_CHANGE_METHOD: &str = "lidar_change_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidarChangeSnapshotPayload {
    pub field_id: String,
    pub scene_id: String,
    pub occupancy_product_ref: String,
    pub chm_product_ref: String,
    pub acquired_at: DateTime<Utc>,
    pub occupancy_grid: ProductGrid,
    pub chm_grid: ProductGrid,
    pub segmentation_reliable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidarChangeRequest {
    pub snapshots: Vec<LidarChangeSnapshotPayload>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LidarChangeDecision {
    Available { reasons: Vec<String> },
    LowConfidence { reasons: Vec<String> },
    Unavailable { reasons: Vec<String> },
}

impl LidarChangeDecision {
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
pub struct LidarChangeResult {
    pub field_id: String,
    pub current_scene_id: String,
    pub previous_scene_id: String,
    pub decision: LidarChangeDecision,
    pub change_layer_ref: String,
    pub width: u32,
    pub height: u32,
    pub common_crs: String,
    pub common_extent: GeoBounds,
    pub common_resolution: RasterResolution,
    pub coverage_fraction: f32,
    pub obstacle_change_values: Option<Vec<f32>>,
    pub obstacle_change_nodata_mask: Option<Vec<bool>>,
    pub uncertainty: HealthUncertaintyBand,
    pub evidence_input_hash: String,
    pub change_statistics: Option<ProductGridStatistics>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LidarChangeError {
    #[error("lidar change requires exactly two snapshots: {provided} provided")]
    InvalidSnapshotCount { provided: usize },
    #[error("a required field is missing: {field}")]
    MissingField { field: &'static str },
    #[error("all snapshots must be for the same field: {left} vs {right}")]
    FieldMismatch { left: String, right: String },
    #[error("snapshot {index} has invalid spatial reference: {reason}")]
    SpatialReference {
        index: usize,
        reason: RasterSpatialRefError,
    },
    #[error("lidar change comparison has invalid input: {reason}")]
    InvalidInput { reason: String },
    #[error("grid mismatch prevented safe comparison: {reason}")]
    GridMismatch { reason: String },
    #[error("evidence generation failed: {0}")]
    Evidence(#[from] crate::evidence::AnalysisEvidenceError),
    #[error("zonal statistics failed: {0}")]
    ZonalStatistics(#[from] ZonalStatisticsError),
}

pub fn analyze_lidar_change(
    mut request: LidarChangeRequest,
) -> Result<LidarChangeResult, LidarChangeError> {
    if request.snapshots.len() != 2 {
        return Err(LidarChangeError::InvalidSnapshotCount {
            provided: request.snapshots.len(),
        });
    }

    request
        .snapshots
        .sort_by_key(|snapshot| snapshot.acquired_at);
    let previous = request.snapshots.remove(0);
    let current = request.snapshots.remove(0);
    if previous.acquired_at == current.acquired_at {
        return Err(LidarChangeError::InvalidInput {
            reason: "snapshot acquisition times must not be equal".to_string(),
        });
    }

    validate_snapshot(&previous)?;
    validate_snapshot(&current)?;

    if previous.field_id != current.field_id {
        return Err(LidarChangeError::FieldMismatch {
            left: previous.field_id,
            right: current.field_id,
        });
    }

    let previous_occupancy = assert_snapshot_spatial_ref(&previous, 0, true)?;
    let current_occupancy = assert_snapshot_spatial_ref(&current, 1, true)?;
    let previous_chm = assert_snapshot_spatial_ref(&previous, 0, false)?;
    let current_chm = assert_snapshot_spatial_ref(&current, 1, false)?;

    let mut mismatch_reasons = Vec::new();
    if previous_occupancy.crs != current_occupancy.crs {
        mismatch_reasons.push("occupancy crs mismatch".to_string());
    }
    if previous_chm.crs != current_chm.crs {
        mismatch_reasons.push("chm crs mismatch".to_string());
    }
    if !extent_matches(
        previous_occupancy
            .bbox
            .as_ref()
            .ok_or_else(|| LidarChangeError::InvalidInput {
                reason: "missing occupancy bbox on previous snapshot".to_string(),
            })?,
        current_occupancy
            .bbox
            .as_ref()
            .ok_or_else(|| LidarChangeError::InvalidInput {
                reason: "missing occupancy bbox on current snapshot".to_string(),
            })?,
    ) {
        mismatch_reasons.push("occupancy extent mismatch".to_string());
    }
    if !extent_matches(
        previous_chm
            .bbox
            .as_ref()
            .ok_or_else(|| LidarChangeError::InvalidInput {
                reason: "missing chm bbox on previous snapshot".to_string(),
            })?,
        current_chm
            .bbox
            .as_ref()
            .ok_or_else(|| LidarChangeError::InvalidInput {
                reason: "missing chm bbox on current snapshot".to_string(),
            })?,
    ) {
        mismatch_reasons.push("chm extent mismatch".to_string());
    }
    if !dimensions_match(&previous.occupancy_grid, &current.occupancy_grid) {
        mismatch_reasons.push("occupancy dimension mismatch".to_string());
    }
    if !dimensions_match(&previous.chm_grid, &current.chm_grid) {
        mismatch_reasons.push("chm dimension mismatch".to_string());
    }
    if !resolution_matches(
        previous_occupancy
            .resolution
            .ok_or_else(|| LidarChangeError::InvalidInput {
                reason: "missing occupancy resolution on previous snapshot".to_string(),
            })?,
        current_occupancy
            .resolution
            .ok_or_else(|| LidarChangeError::InvalidInput {
                reason: "missing occupancy resolution on current snapshot".to_string(),
            })?,
    ) {
        mismatch_reasons.push("occupancy resolution mismatch".to_string());
    }
    if !resolution_matches(
        previous_chm
            .resolution
            .ok_or_else(|| LidarChangeError::InvalidInput {
                reason: "missing chm resolution on previous snapshot".to_string(),
            })?,
        current_chm
            .resolution
            .ok_or_else(|| LidarChangeError::InvalidInput {
                reason: "missing chm resolution on current snapshot".to_string(),
            })?,
    ) {
        mismatch_reasons.push("chm resolution mismatch".to_string());
    }

    let decision = if !mismatch_reasons.is_empty() {
        LidarChangeDecision::Unavailable {
            reasons: mismatch_reasons,
        }
    } else if !previous.segmentation_reliable || !current.segmentation_reliable {
        LidarChangeDecision::LowConfidence {
            reasons: vec!["segmentation reliability is low".to_string()],
        }
    } else {
        LidarChangeDecision::Available {
            reasons: Vec::new(),
        }
    };

    let change_layer_ref = format!(
        "lidar-change:{}:{}:{}",
        previous.scene_id, current.scene_id, previous.field_id
    );
    let coverage = common_coverage(&current.occupancy_grid, &current.chm_grid);
    let common_crs = previous_occupancy.crs.unwrap_or_default();
    let common_extent =
        previous_occupancy
            .bbox
            .clone()
            .ok_or_else(|| LidarChangeError::InvalidInput {
                reason: "missing occupancy extent on previous snapshot".to_string(),
            })?;
    let common_resolution =
        previous_occupancy
            .resolution
            .ok_or_else(|| LidarChangeError::InvalidInput {
                reason: "missing occupancy resolution on previous snapshot".to_string(),
            })?;

    let (
        uncertainty,
        coverage_fraction,
        obstacle_change_values,
        obstacle_change_nodata_mask,
        change_statistics,
    ) = match &decision {
        LidarChangeDecision::Available { .. } => {
            let change_grid = compute_change_grid(&current, &previous)?;
            match compute_zonal_statistics(&change_grid, &change_layer_ref) {
                Ok(stats) => {
                    let coverage_fraction = stats.coverage_fraction;
                    (
                        uncertainty_for_available(stats.statistics.mean_value, coverage_fraction),
                        coverage_fraction,
                        Some(change_grid.values),
                        Some(change_grid.nodata_mask),
                        Some(stats),
                    )
                }
                Err(ZonalStatisticsError::NoValidData { .. }) => {
                    return Ok(LidarChangeResult {
                        field_id: previous.field_id,
                        current_scene_id: current.scene_id,
                        previous_scene_id: previous.scene_id,
                        decision: LidarChangeDecision::Unavailable {
                            reasons: vec!["no valid overlap pixels".to_string()],
                        },
                        change_layer_ref: change_layer_ref.clone(),
                        width: current.occupancy_grid.width,
                        height: current.occupancy_grid.height,
                        common_crs,
                        common_extent,
                        common_resolution,
                        coverage_fraction: 0.0,
                        obstacle_change_values: None,
                        obstacle_change_nodata_mask: None,
                        uncertainty: uncertainty_for_unavailable(&[
                            "no valid overlap pixels".to_string()
                        ]),
                        evidence_input_hash: String::new(),
                        change_statistics: None,
                    });
                }
                Err(error) => return Err(LidarChangeError::ZonalStatistics(error)),
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
        &change_layer_ref,
        LIDAR_CHANGE_METHOD,
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
            &change_layer_ref,
            &decision,
            &previous.scene_id,
            &current.scene_id,
            coverage_fraction,
            &common_crs,
            decision.reasons(),
            &current.occupancy_grid.width,
            &current.occupancy_grid.height,
            &previous.acquired_at,
            &current.acquired_at,
            &current.segmentation_reliable,
            &previous.segmentation_reliable,
        ),
    )?;

    Ok(LidarChangeResult {
        field_id: previous.field_id,
        current_scene_id: current.scene_id,
        previous_scene_id: previous.scene_id,
        decision,
        change_layer_ref,
        width: current.occupancy_grid.width,
        height: current.occupancy_grid.height,
        common_crs,
        common_extent,
        common_resolution,
        coverage_fraction,
        obstacle_change_values,
        obstacle_change_nodata_mask,
        uncertainty,
        evidence_input_hash: evidence.input_hash,
        change_statistics,
    })
}

fn validate_snapshot(snapshot: &LidarChangeSnapshotPayload) -> Result<(), LidarChangeError> {
    require_text(&snapshot.field_id, "field_id")?;
    require_text(&snapshot.scene_id, "scene_id")?;
    require_text(&snapshot.occupancy_product_ref, "occupancy_product_ref")?;
    require_text(&snapshot.chm_product_ref, "chm_product_ref")?;
    if snapshot.acquired_at.timestamp_millis() == 0 {
        return Err(LidarChangeError::MissingField {
            field: "acquired_at",
        });
    }

    Ok(())
}

fn assert_snapshot_spatial_ref(
    snapshot: &LidarChangeSnapshotPayload,
    index: usize,
    occupancy: bool,
) -> Result<RasterSpatialRef, LidarChangeError> {
    let grid = if occupancy {
        &snapshot.occupancy_grid
    } else {
        &snapshot.chm_grid
    };
    let spatial_ref = assert_raster_spatial_ref(Some(&grid.spatial_ref), grid.width, grid.height)
        .map_err(|error| LidarChangeError::SpatialReference {
        index,
        reason: error,
    })?;
    Ok(spatial_ref)
}

fn require_text(value: &str, field: &'static str) -> Result<(), LidarChangeError> {
    if value.trim().is_empty() {
        Err(LidarChangeError::MissingField { field })
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

fn compute_change_grid(
    current: &LidarChangeSnapshotPayload,
    previous: &LidarChangeSnapshotPayload,
) -> Result<ProductGrid, LidarChangeError> {
    let width = current.occupancy_grid.width;
    let height = current.occupancy_grid.height;
    let expected = width as usize * height as usize;

    if current.occupancy_grid.width != previous.occupancy_grid.width
        || current.occupancy_grid.height != previous.occupancy_grid.height
    {
        return Err(LidarChangeError::GridMismatch {
            reason: "occupancy dimensions mismatch".to_string(),
        });
    }
    if current.chm_grid.width != previous.chm_grid.width
        || current.chm_grid.height != previous.chm_grid.height
    {
        return Err(LidarChangeError::GridMismatch {
            reason: "chm dimensions mismatch".to_string(),
        });
    }
    if current.occupancy_grid.values.len() != expected
        || previous.occupancy_grid.values.len() != expected
    {
        return Err(LidarChangeError::InvalidInput {
            reason: "occupancy value count does not match dimensions".to_string(),
        });
    }
    if current.occupancy_grid.nodata_mask.len() != expected
        || previous.occupancy_grid.nodata_mask.len() != expected
    {
        return Err(LidarChangeError::InvalidInput {
            reason: "occupancy nodata mask count does not match dimensions".to_string(),
        });
    }
    if current.chm_grid.values.len() != expected || previous.chm_grid.values.len() != expected {
        return Err(LidarChangeError::InvalidInput {
            reason: "chm value count does not match dimensions".to_string(),
        });
    }
    if current.chm_grid.nodata_mask.len() != expected
        || previous.chm_grid.nodata_mask.len() != expected
    {
        return Err(LidarChangeError::InvalidInput {
            reason: "chm nodata mask count does not match dimensions".to_string(),
        });
    }

    let mut values = Vec::with_capacity(expected);
    let mut nodata_mask = Vec::with_capacity(expected);
    for index in 0..expected {
        let curr_occupancy = current.occupancy_grid.values[index];
        let prev_occupancy = previous.occupancy_grid.values[index];
        let curr_chm = current.chm_grid.values[index];
        let prev_chm = previous.chm_grid.values[index];
        let curr_occupancy_nodata = current.occupancy_grid.nodata_mask[index];
        let prev_occupancy_nodata = previous.occupancy_grid.nodata_mask[index];
        let curr_chm_nodata = current.chm_grid.nodata_mask[index];
        let prev_chm_nodata = previous.chm_grid.nodata_mask[index];

        if curr_occupancy_nodata || prev_occupancy_nodata || curr_chm_nodata || prev_chm_nodata {
            values.push(0.0);
            nodata_mask.push(true);
            continue;
        }
        if !curr_occupancy.is_finite()
            || !prev_occupancy.is_finite()
            || !curr_chm.is_finite()
            || !prev_chm.is_finite()
        {
            return Err(LidarChangeError::InvalidInput {
                reason: format!("invalid occupancy/chm value at index {index}"),
            });
        }

        values.push(((curr_occupancy - prev_occupancy).abs() + (curr_chm - prev_chm).abs()) / 2.0);
        nodata_mask.push(false);
    }

    Ok(ProductGrid {
        width,
        height,
        values,
        nodata_mask,
        spatial_ref: current.occupancy_grid.spatial_ref.clone(),
    })
}

fn common_coverage(occupancy: &ProductGrid, chm: &ProductGrid) -> f32 {
    if occupancy.width == 0 || occupancy.height == 0 {
        return 0.0;
    }
    let total = occupancy.width as f32 * occupancy.height as f32;
    if total <= 0.0 {
        return 0.0;
    }
    let valid = occupancy
        .nodata_mask
        .iter()
        .zip(chm.nodata_mask.iter())
        .filter(|(occ_nodata, chm_nodata)| !**occ_nodata && !**chm_nodata)
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
    fn comparable_reliable_snapshots_produce_change_and_uncertainty() {
        let result = analyze_lidar_change(LidarChangeRequest {
            snapshots: vec![previous_snapshot(), current_snapshot()],
        })
        .expect("valid request can be analyzed");

        assert_eq!(result.field_id, "field-01");
        assert_eq!(result.decision.label(), "available");
        let stats = result
            .change_statistics
            .expect("available result has change statistics");
        assert_eq!(stats.statistics.valid_pixel_count, 3);
        assert_eq!(stats.statistics.coverage_area_m2 > 0.0, true);
        assert!(result.uncertainty.upper > result.uncertainty.lower);
        assert!(!result.evidence_input_hash.is_empty());
        assert!(result.obstacle_change_values.as_ref().is_some());
    }

    #[test]
    fn mismatched_extent_marks_unavailable_not_differenced() {
        let mut latest = current_snapshot();
        latest.chm_grid.spatial_ref.geo_transform =
            Some([500001.0, 10.0, 0.0, 4500020.0, 0.0, -10.0]);
        latest.chm_grid.spatial_ref.bbox = Some(GeoBounds {
            min_lon: 500001.0,
            min_lat: 4500000.0,
            max_lon: 500021.0,
            max_lat: 4500020.0,
        });
        let result = analyze_lidar_change(LidarChangeRequest {
            snapshots: vec![previous_snapshot(), latest],
        })
        .expect("request parses");

        assert_eq!(result.decision.label(), "unavailable");
        assert!(result.obstacle_change_values.is_none());
        assert!(result
            .decision
            .reasons()
            .iter()
            .any(|reason| reason.contains("chm extent mismatch")));
    }

    #[test]
    fn unreliable_segmentation_marks_low_confidence() {
        let mut previous = previous_snapshot();
        previous.segmentation_reliable = false;
        let result = analyze_lidar_change(LidarChangeRequest {
            snapshots: vec![previous, current_snapshot()],
        })
        .expect("request parses");

        assert_eq!(result.decision.label(), "low_confidence");
        assert_eq!(
            result.decision.reasons(),
            &["segmentation reliability is low".to_string()]
        );
        assert!(result.obstacle_change_values.is_none());
    }

    #[test]
    fn invalid_snapshot_count_is_rejected() {
        let request = LidarChangeRequest {
            snapshots: vec![previous_snapshot()],
        };

        let error = analyze_lidar_change(request).expect_err("needs two snapshots");
        assert!(matches!(
            error,
            LidarChangeError::InvalidSnapshotCount { provided: 1 }
        ));
    }

    fn previous_snapshot() -> LidarChangeSnapshotPayload {
        LidarChangeSnapshotPayload {
            field_id: "field-01".to_string(),
            scene_id: "scene-2026-05-01".to_string(),
            occupancy_product_ref: "occupancy-layer-2026-05-01".to_string(),
            chm_product_ref: "chm-layer-2026-05-01".to_string(),
            acquired_at: DateTime::from_timestamp(1714531200, 0).expect("valid timestamp"),
            occupancy_grid: ProductGrid {
                width: 2,
                height: 2,
                values: vec![0.0, 1.0, 0.0, 1.0],
                nodata_mask: vec![false, false, true, false],
                spatial_ref: base_spatial_ref(500000.0, 4500000.0, 10.0, "EPSG:32614"),
            },
            chm_grid: ProductGrid {
                width: 2,
                height: 2,
                values: vec![3.0, 3.5, 2.5, 4.0],
                nodata_mask: vec![false, false, true, false],
                spatial_ref: base_spatial_ref(500000.0, 4500000.0, 10.0, "EPSG:32614"),
            },
            segmentation_reliable: true,
        }
    }

    fn current_snapshot() -> LidarChangeSnapshotPayload {
        LidarChangeSnapshotPayload {
            field_id: "field-01".to_string(),
            scene_id: "scene-2026-06-01".to_string(),
            occupancy_product_ref: "occupancy-layer-2026-06-01".to_string(),
            chm_product_ref: "chm-layer-2026-06-01".to_string(),
            acquired_at: DateTime::from_timestamp(1717209600, 0).expect("valid timestamp"),
            occupancy_grid: ProductGrid {
                width: 2,
                height: 2,
                values: vec![0.0, 0.8, 0.2, 1.0],
                nodata_mask: vec![false, false, true, false],
                spatial_ref: base_spatial_ref(500000.0, 4500000.0, 10.0, "EPSG:32614"),
            },
            chm_grid: ProductGrid {
                width: 2,
                height: 2,
                values: vec![3.2, 3.7, 2.5, 3.8],
                nodata_mask: vec![false, false, true, false],
                spatial_ref: base_spatial_ref(500000.0, 4500000.0, 10.0, "EPSG:32614"),
            },
            segmentation_reliable: true,
        }
    }

    fn base_spatial_ref(
        origin_lon: f64,
        origin_lat: f64,
        resolution: f64,
        crs: &str,
    ) -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some(crs.to_string()),
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
