use crate::evidence::{evidence_parameters, make_analysis_evidence};
use crate::product_anomalies::{flag_product_anomalies, AnomalyDetectionConfig, ProductAnomaly};
use crate::zonal_statistics::{
    compute_zonal_statistics, ProductGrid, ProductGridStatistics, ZonalStatisticsError,
};
use crate::zone_delineation::{delineate_anomaly_zones, AnomalyZone};
use crate::HealthUncertaintyBand;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared::schemas::{GeoBounds, RasterResolution};

pub const INDEX_ANOMALY_FEATURE_FLAG_KEY: &str = "index_anomaly_feature_enabled";
pub const INDEX_ANOMALY_PAYLOAD_KEY: &str = "index_anomaly_payload";
const INDEX_ANOMALY_METHOD: &str = "index_anomaly_v1";
const DEFAULT_STD_DEV_MULTIPLIER: f32 = 2.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexAnomalyRequest {
    pub field_id: String,
    pub scene_id: String,
    pub product_ref: String,
    pub acquired_at: DateTime<Utc>,
    pub grid: ProductGrid,
    pub low_threshold: Option<f32>,
    pub high_threshold: Option<f32>,
    pub std_dev_multiplier: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IndexAnomalyDecision {
    Available { reasons: Vec<String> },
    Unavailable { reasons: Vec<String> },
}

impl IndexAnomalyDecision {
    pub fn reasons(&self) -> &[String] {
        match self {
            Self::Available { reasons } | Self::Unavailable { reasons } => reasons,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Available { .. } => "available",
            Self::Unavailable { .. } => "unavailable",
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexAnomalyResult {
    pub field_id: String,
    pub scene_id: String,
    pub product_ref: String,
    pub decision: IndexAnomalyDecision,
    pub width: u32,
    pub height: u32,
    pub crs: String,
    pub extent: GeoBounds,
    pub resolution: RasterResolution,
    pub coverage_fraction: f32,
    pub anomaly_count: u32,
    pub anomalies: Vec<ProductAnomaly>,
    pub zones: Vec<AnomalyZone>,
    pub uncertainty: HealthUncertaintyBand,
    pub evidence_input_hash: String,
    pub layer_statistics: Option<ProductGridStatistics>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum IndexAnomalyError {
    #[error("index anomaly request requires field: {field}")]
    MissingField { field: &'static str },
    #[error("index anomaly request requires an acquisition timestamp")]
    MissingAcquisitionTime,
    #[error("index anomaly request requires a valid std-dev multiplier")]
    InvalidStdDevMultiplier,
    #[error("index anomaly request invalid input: {reason}")]
    InvalidInput { reason: String },
    #[error("index anomaly statistics failed: {0}")]
    ZonalStatistics(#[from] ZonalStatisticsError),
    #[error("index anomaly detection failed: {0}")]
    AnomalyDetection(#[from] crate::product_anomalies::AnomalyDetectionError),
    #[error("anomaly zone segmentation failed: {0}")]
    ZoneDelineation(#[from] crate::ZoneDelineationError),
    #[error("evidence generation failed: {0}")]
    Evidence(#[from] crate::evidence::AnalysisEvidenceError),
}

pub fn analyze_index_anomalies(
    request: IndexAnomalyRequest,
) -> Result<IndexAnomalyResult, IndexAnomalyError> {
    if request.acquired_at.timestamp_millis() == 0 {
        return Err(IndexAnomalyError::MissingAcquisitionTime);
    }
    require_text(&request.field_id, "field_id")?;
    require_text(&request.scene_id, "scene_id")?;
    require_text(&request.product_ref, "product_ref")?;

    let detection = request_to_detection_config(&request)?;
    let total_pixel_count = request.grid.width.saturating_mul(request.grid.height);
    let (stats, decision_reason) =
        match compute_zonal_statistics(&request.grid, &request.product_ref) {
            Ok(stats) => (
                stats,
                IndexAnomalyDecision::Available {
                    reasons: Vec::new(),
                },
            ),
            Err(ZonalStatisticsError::NoValidData { total_pixel_count }) => {
                return build_unavailable_result(
                    request,
                    vec!["no valid pixels available".to_string()],
                    total_pixel_count,
                );
            }
            Err(ZonalStatisticsError::SpatialRef { reason }) => {
                return build_unavailable_result(
                    request,
                    vec![format!("spatial reference unavailable: {reason}")],
                    total_pixel_count,
                );
            }
            Err(error) => return Err(IndexAnomalyError::ZonalStatistics(error)),
        };

    let anomalies =
        flag_product_anomalies(&request.grid, &stats, &detection, &request.product_ref)?;
    let zones = if anomalies.is_empty() {
        Vec::new()
    } else {
        delineate_anomaly_zones(&request.grid, &anomalies)?
    };

    let anomaly_indices = anomalies
        .iter()
        .map(|anomaly| anomaly.index)
        .collect::<std::collections::HashSet<_>>();
    let anomaly_count = anomaly_indices.len() as u32;
    let anomaly_fraction = if stats.statistics.total_pixel_count == 0 {
        0.0
    } else {
        anomaly_count as f32 / stats.statistics.total_pixel_count as f32
    };
    let uncertainty = uncertainty_for_available(anomaly_fraction, stats.coverage_fraction);
    let layer_statistics = Some(stats.clone());
    let evidence = make_analysis_evidence(
        &request.product_ref,
        INDEX_ANOMALY_METHOD,
        evidence_parameters(&[
            (
                "decision",
                Value::String(decision_reason.label().to_string()),
            ),
            (
                "decision_reason_count",
                json!(decision_reason.reasons().len()),
            ),
            ("anomaly_count", json!(u64::from(anomaly_count))),
            ("coverage_fraction", json!(stats.coverage_fraction)),
            (
                "anomaly_threshold_config",
                json!({
                    "low_threshold": request.low_threshold,
                    "high_threshold": request.high_threshold,
                    "std_dev_multiplier": detection.std_dev_multiplier,
                }),
            ),
            (
                "decision_reasons",
                Value::Array(
                    decision_reason
                        .reasons()
                        .iter()
                        .map(|reason| Value::String(reason.clone()))
                        .collect(),
                ),
            ),
        ]),
        &(
            &request.field_id,
            &request.scene_id,
            &request.product_ref,
            anomaly_count,
            stats.statistics.min_value,
            stats.statistics.max_value,
            stats.statistics.mean_value,
            stats.statistics.std_deviation,
            stats.statistics.valid_pixel_count,
            stats.statistics.total_pixel_count,
            stats.coverage_fraction,
            &detection,
        ),
    )?;

    Ok(IndexAnomalyResult {
        field_id: request.field_id,
        scene_id: request.scene_id,
        product_ref: request.product_ref,
        decision: decision_reason,
        width: request.grid.width,
        height: request.grid.height,
        crs: stats.crs.clone(),
        extent: stats.extent,
        resolution: stats.resolution,
        coverage_fraction: stats.coverage_fraction,
        anomaly_count,
        anomalies,
        zones,
        uncertainty,
        evidence_input_hash: evidence.input_hash,
        layer_statistics,
    })
}

fn request_to_detection_config(
    request: &IndexAnomalyRequest,
) -> Result<AnomalyDetectionConfig, IndexAnomalyError> {
    if let (Some(low), Some(high)) = (request.low_threshold, request.high_threshold) {
        if low >= high {
            return Err(IndexAnomalyError::InvalidInput {
                reason: "low_threshold must be less than high_threshold".to_string(),
            });
        }
    }

    let std_dev_multiplier = request
        .std_dev_multiplier
        .or(Some(DEFAULT_STD_DEV_MULTIPLIER));
    if let Some(multiplier) = std_dev_multiplier {
        if !multiplier.is_finite() || multiplier <= 0.0 {
            return Err(IndexAnomalyError::InvalidStdDevMultiplier);
        }
    }

    Ok(AnomalyDetectionConfig {
        low_threshold: request.low_threshold,
        high_threshold: request.high_threshold,
        std_dev_multiplier,
    })
}

fn build_unavailable_result(
    request: IndexAnomalyRequest,
    reasons: Vec<String>,
    total_pixel_count: u32,
) -> Result<IndexAnomalyResult, IndexAnomalyError> {
    let decision = IndexAnomalyDecision::Unavailable {
        reasons: reasons.clone(),
    };
    let uncertainty = uncertainty_for_unavailable(&reasons);

    let evidence = make_analysis_evidence(
        &request.product_ref,
        INDEX_ANOMALY_METHOD,
        evidence_parameters(&[
            ("decision", Value::String(decision.label().to_string())),
            ("decision_reason_count", json!(reasons.len())),
            (
                "decision_reasons",
                Value::Array(
                    reasons
                        .iter()
                        .map(|reason| Value::String(reason.clone()))
                        .collect(),
                ),
            ),
            ("total_pixel_count", json!(total_pixel_count)),
        ]),
        &(
            &request.field_id,
            &request.scene_id,
            &request.product_ref,
            &reasons,
            total_pixel_count,
        ),
    )?;

    let crs = request.grid.spatial_ref.crs.unwrap_or_else(String::new);
    let extent = request.grid.spatial_ref.bbox.unwrap_or(GeoBounds {
        min_lon: 0.0,
        min_lat: 0.0,
        max_lon: 0.0,
        max_lat: 0.0,
    });
    let resolution = request
        .grid
        .spatial_ref
        .resolution
        .unwrap_or(RasterResolution { x: 0.0, y: 0.0 });

    Ok(IndexAnomalyResult {
        field_id: request.field_id,
        scene_id: request.scene_id,
        product_ref: request.product_ref,
        decision,
        width: request.grid.width,
        height: request.grid.height,
        crs,
        extent,
        resolution,
        coverage_fraction: 0.0,
        anomaly_count: 0,
        anomalies: Vec::new(),
        zones: Vec::new(),
        uncertainty,
        evidence_input_hash: evidence.input_hash,
        layer_statistics: None,
    })
}

fn require_text(value: &str, field: &'static str) -> Result<(), IndexAnomalyError> {
    if value.trim().is_empty() {
        Err(IndexAnomalyError::MissingField { field })
    } else {
        Ok(())
    }
}

fn uncertainty_for_available(
    anomaly_fraction: f32,
    coverage_fraction: f32,
) -> HealthUncertaintyBand {
    let span = (0.08 + (1.0 - coverage_fraction).abs() * 0.45).clamp(0.05, 1.0);
    HealthUncertaintyBand {
        lower: (anomaly_fraction - span).max(0.0),
        upper: (anomaly_fraction + span).min(1.0),
    }
}

fn uncertainty_for_unavailable(reasons: &[String]) -> HealthUncertaintyBand {
    let span = (0.6 + (0.1 * reasons.len() as f32)).clamp(0.6, 1.5);
    HealthUncertaintyBand {
        lower: -span,
        upper: span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::{GeoBounds, RasterResolution, RasterSpatialRef};

    #[test]
    fn detects_threshold_anomalies_and_delineates_zones() {
        let request = request();
        let result = analyze_index_anomalies(request.clone()).expect("anomaly analysis runs");

        assert_eq!(result.decision.label(), "available");
        assert_eq!(result.anomaly_count, 2);
        assert_eq!(result.zones.len(), 2);
        assert_eq!(result.zones[0].cell_indices, vec![0]);
        assert_eq!(result.zones[1].cell_indices, vec![3]);
        assert_eq!(result.zones[0].evidence[0].method, "anomaly_detection_v1");
        assert_eq!(result.evidence_input_hash.len(), 16);
    }

    #[test]
    fn ungeoreferenced_index_product_is_marked_unavailable() {
        let mut request = request();
        request.grid.spatial_ref.georeferenced = false;
        request.grid.spatial_ref.crs = None;

        let result =
            analyze_index_anomalies(request).expect("anomaly analysis handles ungeoreferenced");

        assert_eq!(result.decision.label(), "unavailable");
        assert_eq!(result.anomaly_count, 0);
        assert_eq!(result.zones.len(), 0);
        assert_eq!(result.evidence_input_hash.len(), 16);
    }

    #[test]
    fn uniform_raster_with_statistical_only_detection_flags_zero_anomalies() {
        let mut request = request();
        request.grid.values = vec![0.4; 12];
        request.low_threshold = None;
        request.high_threshold = None;
        request.std_dev_multiplier = Some(DEFAULT_STD_DEV_MULTIPLIER);

        let result = analyze_index_anomalies(request).expect("uniform raster processes");

        assert_eq!(result.decision.label(), "available");
        assert_eq!(result.anomaly_count, 0);
        assert_eq!(result.zones.len(), 0);
    }

    #[test]
    fn invalid_std_dev_multiplier_is_rejected_with_anomaly_error() {
        let mut request = request();
        request.std_dev_multiplier = Some(-2.0);

        let error = analyze_index_anomalies(request).expect_err("invalid std dev rejected");

        assert!(matches!(
            error,
            IndexAnomalyError::InvalidStdDevMultiplier | IndexAnomalyError::InvalidInput { .. }
        ));
    }

    #[test]
    fn no_threshold_high_and_low_defaults_to_statistical_detection() {
        let mut request = request();
        request.low_threshold = None;
        request.high_threshold = None;
        request.std_dev_multiplier = None;

        let result = analyze_index_anomalies(request).expect("defaults to std-dev");

        assert_eq!(result.decision.label(), "available");
    }

    fn request() -> IndexAnomalyRequest {
        IndexAnomalyRequest {
            field_id: "field-a".to_string(),
            scene_id: "scene-2026-05-01".to_string(),
            product_ref: "layer-ndvi-2026-05-01".to_string(),
            acquired_at: DateTime::parse_from_rfc3339("2026-05-01T00:00:00Z")
                .expect("valid time")
                .with_timezone(&Utc),
            grid: ProductGrid {
                width: 4,
                height: 3,
                values: vec![
                    0.1, 0.25, 0.3, 0.85, 0.4, 0.45, 0.48, 0.5, 0.52, 0.55, 0.58, 0.6,
                ],
                nodata_mask: vec![false; 12],
                spatial_ref: RasterSpatialRef {
                    georeferenced: true,
                    crs: Some("EPSG:32614".to_string()),
                    bbox: Some(GeoBounds {
                        min_lon: 500000.0,
                        min_lat: 4500000.0,
                        max_lon: 500040.0,
                        max_lat: 4500030.0,
                    }),
                    geo_transform: Some([500000.0, 10.0, 0.0, 4500030.0, 0.0, -10.0]),
                    resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
                },
            },
            low_threshold: Some(0.2),
            high_threshold: Some(0.8),
            std_dev_multiplier: Some(DEFAULT_STD_DEV_MULTIPLIER),
        }
    }
}
