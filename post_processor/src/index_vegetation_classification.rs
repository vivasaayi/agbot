use crate::evidence::{evidence_parameters, make_analysis_evidence};
use crate::zonal_statistics::ProductGrid;
use crate::HealthUncertaintyBand;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared::schemas::{
    assert_raster_spatial_ref, GeoBounds, RasterResolution, RasterSpatialRefError,
};
use std::collections::{HashMap, HashSet, VecDeque};

pub const INDEX_VEGETATION_CLASSIFICATION_FEATURE_FLAG_KEY: &str =
    "index_vegetation_classification_feature_enabled";
pub const INDEX_VEGETATION_CLASSIFICATION_PAYLOAD_KEY: &str =
    "index_vegetation_classification_payload";
const INDEX_VEGETATION_CLASSIFICATION_METHOD: &str = "index_vegetation_classification_v1";
const CLASSIFICATION_ZONE_ID_PREFIX: &str = "veg-zone";
const UNCLASSIFIED: usize = usize::MAX;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VegetationTypeSignature {
    pub name: String,
    pub mean_ndvi: f32,
    pub std_ndvi: f32,
    pub trend_ndvi: f32,
    pub mean_tolerance: f32,
    pub std_tolerance: f32,
    pub trend_tolerance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexVegetationTypeClassificationSnapshot {
    pub field_id: String,
    pub scene_id: String,
    pub product_ref: String,
    pub acquired_at: DateTime<Utc>,
    pub grid: ProductGrid,
    pub calibrated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexVegetationTypeClassificationRequest {
    pub snapshots: Vec<IndexVegetationTypeClassificationSnapshot>,
    pub signature_library: Option<Vec<VegetationTypeSignature>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum VegetationTypeClassificationDecision {
    Available { reasons: Vec<String> },
    LowConfidence { reasons: Vec<String> },
    Unavailable { reasons: Vec<String> },
}

impl VegetationTypeClassificationDecision {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VegetationTypeClassZone {
    pub zone_id: String,
    pub class_name: String,
    pub pixel_count: u32,
    pub coverage_fraction: f32,
    pub mean_confidence: f32,
    pub centroid: (f64, f64),
    pub polygon: Vec<(f64, f64)>,
    pub area_m2: f32,
    pub matched_signature_distance: f32,
    pub matched_signature_name: String,
    pub matched_signature_evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VegetationTypeClassificationClassStat {
    pub class_name: String,
    pub pixel_count: u32,
    pub coverage_fraction: f32,
    pub mean_confidence: f32,
}

#[derive(Debug, Clone)]
pub struct IndexVegetationTypeClassificationResult {
    pub field_id: String,
    pub current_scene_id: String,
    pub baseline_scene_id: String,
    pub decision: VegetationTypeClassificationDecision,
    pub width: u32,
    pub height: u32,
    pub crs: String,
    pub extent: GeoBounds,
    pub resolution: RasterResolution,
    pub coverage_fraction: f32,
    pub evidence_input_hash: String,
    pub uncertainty: HealthUncertaintyBand,
    pub mean_confidence: f32,
    pub snapshots_used: usize,
    pub zones: Vec<VegetationTypeClassZone>,
    pub class_stats: Vec<VegetationTypeClassificationClassStat>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum IndexVegetationTypeClassificationError {
    #[error("index vegetation-type request requires {field}")]
    MissingField { field: &'static str },
    #[error("index vegetation-type request requires at least two calibrated snapshots")]
    InsufficientSnapshots,
    #[error("all snapshots must match the same field: {left} vs {right}")]
    FieldMismatch { left: String, right: String },
    #[error(
        "index vegetation-type snapshot index {index} has invalid spatial reference: {reason}"
    )]
    SpatialReference {
        index: usize,
        reason: RasterSpatialRefError,
    },
    #[error(
        "index vegetation-type snapshot index {index} has invalid signature library: {reason}"
    )]
    SignatureLibrary { index: usize, reason: &'static str },
    #[error("index vegetation-type comparison failed: {reason}")]
    InvalidComparison { reason: String },
    #[error("index vegetation-type analysis failed: no valid pixels after gating")]
    NoValidPixels,
    #[error("index vegetation-type evidence generation failed: {0}")]
    Evidence(#[from] crate::evidence::AnalysisEvidenceError),
}

pub fn analyze_index_vegetation_type_classification(
    request: IndexVegetationTypeClassificationRequest,
) -> Result<IndexVegetationTypeClassificationResult, IndexVegetationTypeClassificationError> {
    let mut request = request;
    if request.snapshots.len() < 2 {
        return Err(IndexVegetationTypeClassificationError::InsufficientSnapshots);
    }

    request
        .snapshots
        .sort_by_key(|snapshot| snapshot.acquired_at);
    require_text(&request.snapshots[0].field_id, "field_id")?;
    for snapshot in &request.snapshots {
        require_text(&snapshot.scene_id, "scene_id")?;
        require_text(&snapshot.product_ref, "product_ref")?;
    }

    for i in 1..request.snapshots.len() {
        if request.snapshots[i - 1].field_id != request.snapshots[i].field_id {
            return Err(IndexVegetationTypeClassificationError::FieldMismatch {
                left: request.snapshots[i - 1].field_id.clone(),
                right: request.snapshots[i].field_id.clone(),
            });
        }
    }

    let signatures = request
        .signature_library
        .unwrap_or_else(default_signature_library);
    validate_signatures(&signatures)?;

    let mut mismatch_reasons = Vec::new();
    let base = &request.snapshots[0];
    let base_spatial = assert_snapshot_spatial_ref(base, 0)?;

    for (index, snapshot) in request.snapshots.iter().enumerate().skip(1) {
        let snapshot_spatial = assert_snapshot_spatial_ref(snapshot, index)?;
        if snapshot_spatial.crs != base_spatial.crs {
            mismatch_reasons.push("crs mismatch".to_string());
        }
        if !extents_match(
            snapshot_spatial.bbox.as_ref().ok_or_else(|| {
                IndexVegetationTypeClassificationError::InvalidComparison {
                    reason: "missing extent in base snapshot".to_string(),
                }
            })?,
            base_spatial.bbox.as_ref().ok_or_else(|| {
                IndexVegetationTypeClassificationError::InvalidComparison {
                    reason: "missing extent in reference snapshot".to_string(),
                }
            })?,
        ) {
            mismatch_reasons.push("extent mismatch".to_string());
        }
        if snapshot.grid.width != base.grid.width || snapshot.grid.height != base.grid.height {
            mismatch_reasons.push("dimension mismatch".to_string());
        }
        if !resolutions_match(
            snapshot_spatial.resolution.ok_or_else(|| {
                IndexVegetationTypeClassificationError::InvalidComparison {
                    reason: "snapshot missing resolution".to_string(),
                }
            })?,
            base_spatial.resolution.ok_or_else(|| {
                IndexVegetationTypeClassificationError::InvalidComparison {
                    reason: "reference snapshot missing resolution".to_string(),
                }
            })?,
        ) {
            mismatch_reasons.push("resolution mismatch".to_string());
        }
    }
    mismatch_reasons.sort_unstable();
    mismatch_reasons.dedup();

    let all_calibrated = request.snapshots.iter().all(|snapshot| snapshot.calibrated);
    let mut decision = if mismatch_reasons.is_empty() {
        if all_calibrated {
            VegetationTypeClassificationDecision::Available {
                reasons: Vec::new(),
            }
        } else {
            VegetationTypeClassificationDecision::LowConfidence {
                reasons: vec!["one_or_more_snapshots_uncalibrated".to_string()],
            }
        }
    } else {
        VegetationTypeClassificationDecision::Unavailable {
            reasons: mismatch_reasons,
        }
    };

    let total_pixels = base.grid.width.saturating_mul(base.grid.height);
    let (class_ids, confidence_map, zone_features, total_valid, total_pixels) = if matches!(
        decision,
        VegetationTypeClassificationDecision::Unavailable { .. }
    ) {
        (
            vec![UNCLASSIFIED; total_pixels as usize],
            vec![0.0; total_pixels as usize],
            vec![None; total_pixels as usize],
            0u32,
            total_pixels,
        )
    } else {
        classify_pixels(&request.snapshots, &signatures)?
    };

    let base_ref = &base.product_ref;
    let coverage_fraction = if total_pixels == 0 {
        0.0
    } else {
        total_valid as f32 / total_pixels as f32
    };
    if coverage_fraction == 0.0
        && !matches!(
            decision,
            VegetationTypeClassificationDecision::Unavailable { .. }
        )
    {
        decision = VegetationTypeClassificationDecision::Unavailable {
            reasons: vec!["no valid pixels".to_string()],
        };
    }

    let zones = match &decision {
        VegetationTypeClassificationDecision::Unavailable { .. } => Vec::new(),
        _ => build_zones(
            &base.grid,
            &class_ids,
            &confidence_map,
            &zone_features,
            &signatures,
            total_pixels,
            base_ref,
        )?,
    };
    let class_stats = summarize_class_stats(&class_ids, &confidence_map, &signatures, total_valid);

    let uncertainty = uncertainty_band(coverage_fraction, decision.reasons().len() as f32);
    let mean_confidence = if total_valid == 0 {
        0.0
    } else {
        confidence_map
            .iter()
            .filter(|value| **value > 0.0)
            .sum::<f32>()
            / total_valid as f32
    };

    let evidence = make_analysis_evidence(
        &zone_ref(base),
        INDEX_VEGETATION_CLASSIFICATION_METHOD,
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
            ("snapshot_count", json!(request.snapshots.len())),
            ("coverage_fraction", json!(coverage_fraction)),
            (
                "signatures",
                json!(signatures
                    .iter()
                    .map(|signature| &signature.name)
                    .collect::<Vec<_>>()),
            ),
            (
                "baseline_scene_id",
                Value::String(request.snapshots[0].scene_id.clone()),
            ),
            (
                "current_scene_id",
                Value::String(
                    request
                        .snapshots
                        .last()
                        .expect("at least one snapshot exists")
                        .scene_id
                        .clone(),
                ),
            ),
        ]),
        &(
            &base.field_id,
            base.grid.width,
            base.grid.height,
            decision.label(),
            request.snapshots.len(),
            &request.snapshots.first(),
            &request.snapshots.last(),
            &signatures,
        ),
    )?;

    Ok(IndexVegetationTypeClassificationResult {
        field_id: base.field_id.clone(),
        current_scene_id: request
            .snapshots
            .last()
            .expect("at least one snapshot exists")
            .scene_id
            .clone(),
        baseline_scene_id: request.snapshots[0].scene_id.clone(),
        decision,
        width: base.grid.width,
        height: base.grid.height,
        crs: base_spatial
            .crs
            .expect("reference spatial reference must include CRS"),
        extent: base_spatial
            .bbox
            .expect("reference spatial reference always has extent"),
        resolution: base_spatial
            .resolution
            .expect("reference spatial reference always has resolution"),
        coverage_fraction,
        evidence_input_hash: evidence.input_hash,
        uncertainty,
        mean_confidence,
        snapshots_used: request.snapshots.len(),
        zones,
        class_stats,
    })
}

fn classify_pixels(
    snapshots: &[IndexVegetationTypeClassificationSnapshot],
    signatures: &[VegetationTypeSignature],
) -> Result<
    (Vec<usize>, Vec<f32>, Vec<Option<ZoneFeature>>, u32, u32),
    IndexVegetationTypeClassificationError,
> {
    let expected_count = snapshots[0].grid.width as usize * snapshots[0].grid.height as usize;
    for (index, snapshot) in snapshots.iter().enumerate() {
        if snapshot.grid.width as usize * snapshot.grid.height as usize != expected_count {
            return Err(IndexVegetationTypeClassificationError::InvalidComparison {
                reason: format!("snapshot #{index} does not match first snapshot dimensions"),
            });
        }
        if snapshot.grid.values.len() != expected_count
            || snapshot.grid.nodata_mask.len() != expected_count
        {
            return Err(IndexVegetationTypeClassificationError::InvalidComparison {
                reason: "snapshot grid shape does not match dimension metadata".to_string(),
            });
        }
    }

    let mut class_ids = vec![UNCLASSIFIED; expected_count];
    let mut confidences = vec![0.0; expected_count];
    let mut features = vec![None; expected_count];
    let mut valid_count = 0u32;

    for index in 0..expected_count {
        let mut values = Vec::with_capacity(snapshots.len());
        for snapshot in snapshots {
            if snapshot.grid.nodata_mask[index] {
                continue;
            }
            let value = snapshot.grid.values[index];
            if !value.is_finite() {
                continue;
            }
            values.push(value);
        }
        if values.is_empty() {
            continue;
        }

        let (mean, std, trend, trend_span) = temporal_features(&values);
        let (class_id, signature_distance, confidence) =
            classify_signal(mean, std, trend, signatures, trend_span)?;
        class_ids[index] = class_id;
        confidences[index] = confidence;
        features[index] = Some(ZoneFeature {
            class_id,
            class_name: signatures[class_id].name.clone(),
            match_distance: signature_distance,
        });
        valid_count += 1;
    }

    Ok((
        class_ids,
        confidences,
        features,
        valid_count,
        expected_count as u32,
    ))
}

fn classify_signal(
    mean: f32,
    std: f32,
    trend: f32,
    signatures: &[VegetationTypeSignature],
    trend_span: f32,
) -> Result<(usize, f32, f32), IndexVegetationTypeClassificationError> {
    let mut best = None::<(usize, f32, f32)>;
    for (index, signature) in signatures.iter().enumerate() {
        let distance = normalized_distance(mean, std, trend, signature, trend_span);
        let confidence = (1.0 / (1.0 + distance)).clamp(0.0, 1.0);
        let candidate = (index, distance, confidence);
        if best.is_none()
            || candidate.1 < best.expect("best candidate exists").1 - f32::EPSILON
            || (candidate.1 <= best.expect("best candidate exists").1 + f32::EPSILON
                && signatures[index].name < signatures[best.expect("best candidate exists").0].name)
        {
            best = Some(candidate);
        }
    }

    best.ok_or(IndexVegetationTypeClassificationError::InvalidComparison {
        reason: "no signatures available to classify pixel".to_string(),
    })
}

fn normalize_distance(numerator: f32, denominator: f32) -> f32 {
    if denominator <= 0.0 {
        0.0
    } else {
        (numerator / denominator).abs()
    }
}

fn normalized_distance(
    mean: f32,
    std: f32,
    trend: f32,
    signature: &VegetationTypeSignature,
    trend_span: f32,
) -> f32 {
    let span = trend_span.max(1.0);
    normalize_distance(mean - signature.mean_ndvi, signature.mean_tolerance)
        + normalize_distance(std - signature.std_ndvi, signature.std_tolerance)
        + normalize_distance(
            trend - signature.trend_ndvi,
            (signature.trend_tolerance * span).max(1e-6),
        )
}

fn temporal_features(values: &[f32]) -> (f32, f32, f32, f32) {
    let sum = values.iter().sum::<f32>();
    let mean = sum / values.len() as f32;
    let variance = values
        .iter()
        .map(|value| {
            let delta = *value - mean;
            delta * delta
        })
        .sum::<f32>()
        / values.len() as f32;
    let std = variance.sqrt();
    let first = values.first().copied().unwrap_or(0.0);
    let last = values.last().copied().unwrap_or(0.0);
    let trend = last - first;
    let span = (last - first).abs();
    (mean, std, trend, span)
}

#[derive(Debug, Clone)]
struct ZoneFeature {
    class_id: usize,
    class_name: String,
    match_distance: f32,
}

fn build_zones(
    grid: &ProductGrid,
    class_ids: &[usize],
    confidences: &[f32],
    zone_features: &[Option<ZoneFeature>],
    signatures: &[VegetationTypeSignature],
    total_pixels: u32,
    base_ref: &str,
) -> Result<Vec<VegetationTypeClassZone>, IndexVegetationTypeClassificationError> {
    let total = grid.width as usize * grid.height as usize;
    if class_ids.len() != total
        || confidences.len() != total
        || zone_features.len() != total
        || grid.values.len() != total
        || grid.nodata_mask.len() != total
    {
        return Err(IndexVegetationTypeClassificationError::InvalidComparison {
            reason: "pixel arrays do not match grid".to_string(),
        });
    }

    let spatial_ref = assert_raster_spatial_ref(Some(&grid.spatial_ref), grid.width, grid.height)
        .map_err(|reason| {
        IndexVegetationTypeClassificationError::SpatialReference { index: 0, reason }
    })?;
    let transform = spatial_ref
        .geo_transform
        .expect("asserted spatial ref always has transform");
    let pixel_area = spatial_ref
        .resolution
        .expect("asserted spatial ref always has resolution");
    let pixel_area = (pixel_area.x * pixel_area.y) as f32;
    let mut visited = HashSet::new();
    let mut zones = Vec::new();

    for start in 0..total {
        if visited.contains(&start) || class_ids[start] == UNCLASSIFIED || grid.nodata_mask[start] {
            continue;
        }
        let feature = zone_features[start]
            .as_ref()
            .expect("classified pixel has feature");
        let class_id = feature.class_id;
        let mut queue = VecDeque::from([start]);
        visited.insert(start);
        let mut component = Vec::new();
        while let Some(index) = queue.pop_front() {
            component.push(index);
            for neighbor in neighbors(index, grid.width, grid.height) {
                if visited.contains(&neighbor) {
                    continue;
                }
                if class_ids[neighbor] == class_id && !grid.nodata_mask[neighbor] {
                    visited.insert(neighbor);
                    queue.push_back(neighbor);
                }
            }
        }
        if component.is_empty() {
            continue;
        }
        component.sort_unstable();
        let zone_number = zones.len() + 1;
        let zone = zone_from_component(
            zone_number,
            grid.width,
            &transform,
            pixel_area,
            spatial_ref.crs.as_deref().expect("asserted CRS"),
            &component,
            class_id,
            &feature.class_name,
            signatures,
            &feature,
            &component
                .iter()
                .map(|index| confidences[*index])
                .collect::<Vec<_>>(),
            total_pixels,
            base_ref,
        )?;
        zones.push(zone);
    }

    zones.sort_by(|left, right| {
        left.class_name
            .cmp(&right.class_name)
            .then_with(|| left.zone_id.cmp(&right.zone_id))
    });
    Ok(zones)
}

fn zone_from_component(
    zone_number: usize,
    width: u32,
    transform: &[f64; 6],
    pixel_area: f32,
    _crs: &str,
    cell_indices: &[usize],
    _class_id: usize,
    class_name: &str,
    signatures: &[VegetationTypeSignature],
    feature: &ZoneFeature,
    zone_confidences: &[f32],
    total_pixels: u32,
    base_ref: &str,
) -> Result<VegetationTypeClassZone, IndexVegetationTypeClassificationError> {
    let width = width as usize;
    let mut min_row = usize::MAX;
    let mut max_row = 0usize;
    let mut min_col = usize::MAX;
    let mut max_col = 0usize;
    let mut centroid_x = 0.0;
    let mut centroid_y = 0.0;

    let mut area = 0.0f32;
    for index in cell_indices {
        let row = index / width;
        let col = index % width;
        min_row = min_row.min(row);
        max_row = max_row.max(row);
        min_col = min_col.min(col);
        max_col = max_col.max(col);
        let center = transform_point(transform, col as f64 + 0.5, row as f64 + 0.5);
        centroid_x += center.0;
        centroid_y += center.1;
        area += pixel_area;
    }

    let confidence_sum: f32 = zone_confidences
        .iter()
        .copied()
        .filter(|value| *value > 0.0)
        .sum();

    let centroid_len = zone_confidences.len() as f64;
    let mean_confidence = if centroid_len > 0.0 {
        confidence_sum / centroid_len as f32
    } else {
        0.0
    };
    let pixel_count = cell_indices.len() as u32;
    let top_left = transform_point(transform, min_col as f64, min_row as f64);
    let top_right = transform_point(transform, (max_col + 1) as f64, min_row as f64);
    let bottom_right = transform_point(transform, (max_col + 1) as f64, (max_row + 1) as f64);
    let bottom_left = transform_point(transform, min_col as f64, (max_row + 1) as f64);

    let signature = signatures
        .iter()
        .find(|signature| signature.name == class_name)
        .cloned()
        .ok_or_else(
            || IndexVegetationTypeClassificationError::InvalidComparison {
                reason: "zone class has no signature".to_string(),
            },
        )?;
    let match_distance = feature.match_distance;
    let matched_signature_name = signature.name.clone();
    let evidence = make_analysis_evidence(
        base_ref,
        "vegetation_class_zone_v1",
        evidence_parameters(&[
            ("class_name", Value::String(class_name.to_string())),
            ("class_pixel_count", json!(pixel_count)),
            ("mean_confidence", json!(mean_confidence)),
            (
                "match_distance",
                Value::String(format!("{match_distance:.8}")),
            ),
        ]),
        &(
            zone_number,
            class_name,
            pixel_count,
            mean_confidence,
            match_distance,
            base_ref,
        ),
    )?;

    Ok(VegetationTypeClassZone {
        zone_id: format!("{CLASSIFICATION_ZONE_ID_PREFIX}-{zone_number}"),
        class_name: class_name.to_string(),
        pixel_count,
        coverage_fraction: if total_pixels == 0 {
            0.0
        } else {
            pixel_count as f32 / total_pixels as f32
        },
        mean_confidence,
        centroid: (
            centroid_x / centroid_len.max(1.0),
            centroid_y / centroid_len.max(1.0),
        ),
        polygon: vec![top_left, top_right, bottom_right, bottom_left, top_left],
        area_m2: area,
        matched_signature_distance: match_distance,
        matched_signature_name,
        matched_signature_evidence: evidence.input_hash,
    })
}

fn summarize_class_stats(
    class_ids: &[usize],
    confidences: &[f32],
    signatures: &[VegetationTypeSignature],
    valid_pixel_count: u32,
) -> Vec<VegetationTypeClassificationClassStat> {
    let mut count_by_class: HashMap<usize, (u32, f32)> = HashMap::new();
    for (class_id, confidence) in class_ids.iter().zip(confidences) {
        if *class_id == UNCLASSIFIED || *confidence <= 0.0 {
            continue;
        }
        let entry = count_by_class.entry(*class_id).or_insert((0u32, 0.0));
        entry.0 += 1;
        entry.1 += *confidence;
    }

    let mut stats = count_by_class
        .into_iter()
        .map(|(class_id, (count, confidence_sum))| {
            let class_name = signatures[class_id].name.clone();
            let mean_confidence = if count == 0 {
                0.0
            } else {
                confidence_sum / count as f32
            };
            VegetationTypeClassificationClassStat {
                class_name,
                pixel_count: count,
                coverage_fraction: if valid_pixel_count == 0 {
                    0.0
                } else {
                    count as f32 / valid_pixel_count as f32
                },
                mean_confidence,
            }
        })
        .collect::<Vec<_>>();
    stats.sort_by(|left, right| right.pixel_count.cmp(&left.pixel_count));
    stats
}

fn assert_snapshot_spatial_ref(
    snapshot: &IndexVegetationTypeClassificationSnapshot,
    index: usize,
) -> Result<shared::schemas::RasterSpatialRef, IndexVegetationTypeClassificationError> {
    let validated = assert_raster_spatial_ref(
        Some(&snapshot.grid.spatial_ref),
        snapshot.grid.width,
        snapshot.grid.height,
    )
    .map_err(|reason| IndexVegetationTypeClassificationError::SpatialReference { index, reason })?;
    Ok(validated)
}

fn zone_ref(snapshot: &IndexVegetationTypeClassificationSnapshot) -> String {
    format!(
        "vegetation-type-classification:{}:{}:{}",
        snapshot.field_id, snapshot.scene_id, snapshot.product_ref
    )
}

fn validate_signatures(
    signatures: &[VegetationTypeSignature],
) -> Result<(), IndexVegetationTypeClassificationError> {
    if signatures.is_empty() {
        return Err(IndexVegetationTypeClassificationError::SignatureLibrary {
            index: 0,
            reason: "signature list cannot be empty",
        });
    }
    for (index, signature) in signatures.iter().enumerate() {
        require_text(&signature.name, "signature name")?;
        if !signature.mean_tolerance.is_finite() || signature.mean_tolerance <= 0.0 {
            return Err(IndexVegetationTypeClassificationError::SignatureLibrary {
                index,
                reason: "mean_tolerance must be positive and finite",
            });
        }
        if !signature.std_tolerance.is_finite() || signature.std_tolerance <= 0.0 {
            return Err(IndexVegetationTypeClassificationError::SignatureLibrary {
                index,
                reason: "std_tolerance must be positive and finite",
            });
        }
        if !signature.trend_tolerance.is_finite() || signature.trend_tolerance <= 0.0 {
            return Err(IndexVegetationTypeClassificationError::SignatureLibrary {
                index,
                reason: "trend_tolerance must be positive and finite",
            });
        }
    }
    Ok(())
}

fn transform_point(transform: &[f64; 6], col: f64, row: f64) -> (f64, f64) {
    (
        transform[0] + col * transform[1] + row * transform[2],
        transform[3] + col * transform[4] + row * transform[5],
    )
}

fn uncertainty_band(coverage_fraction: f32, reason_count: f32) -> HealthUncertaintyBand {
    let span =
        (0.08 + (1.0 - coverage_fraction).abs() * 0.65).clamp(0.08, 1.6) + (reason_count * 0.02);
    HealthUncertaintyBand {
        lower: (0.25 - span).max(0.0),
        upper: (0.25 + span).min(1.0),
    }
}

fn require_text(
    value: &str,
    field: &'static str,
) -> Result<(), IndexVegetationTypeClassificationError> {
    if value.trim().is_empty() {
        Err(IndexVegetationTypeClassificationError::MissingField { field })
    } else {
        Ok(())
    }
}

fn default_signature_library() -> Vec<VegetationTypeSignature> {
    let mut signatures = vec![
        VegetationTypeSignature {
            name: "cotton".to_string(),
            mean_ndvi: 0.42,
            std_ndvi: 0.16,
            trend_ndvi: 0.0,
            mean_tolerance: 0.22,
            std_tolerance: 0.19,
            trend_tolerance: 0.20,
        },
        VegetationTypeSignature {
            name: "forest".to_string(),
            mean_ndvi: 0.72,
            std_ndvi: 0.08,
            trend_ndvi: 0.0,
            mean_tolerance: 0.18,
            std_tolerance: 0.16,
            trend_tolerance: 0.20,
        },
        VegetationTypeSignature {
            name: "bush".to_string(),
            mean_ndvi: 0.30,
            std_ndvi: 0.14,
            trend_ndvi: -0.01,
            mean_tolerance: 0.20,
            std_tolerance: 0.16,
            trend_tolerance: 0.22,
        },
        VegetationTypeSignature {
            name: "palm".to_string(),
            mean_ndvi: 0.60,
            std_ndvi: 0.12,
            trend_ndvi: 0.02,
            mean_tolerance: 0.20,
            std_tolerance: 0.14,
            trend_tolerance: 0.24,
        },
        VegetationTypeSignature {
            name: "rice".to_string(),
            mean_ndvi: 0.36,
            std_ndvi: 0.15,
            trend_ndvi: 0.00,
            mean_tolerance: 0.24,
            std_tolerance: 0.20,
            trend_tolerance: 0.25,
        },
    ];
    signatures.sort_by(|left, right| left.name.cmp(&right.name));
    signatures
}

fn extents_match(left: &GeoBounds, right: &GeoBounds) -> bool {
    (left.min_lon - right.min_lon).abs() <= shared::schemas::GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.max_lon - right.max_lon).abs() <= shared::schemas::GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.min_lat - right.min_lat).abs() <= shared::schemas::GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.max_lat - right.max_lat).abs() <= shared::schemas::GEO_EXTENT_ASSERTION_TOLERANCE
}

fn resolutions_match(left: RasterResolution, right: RasterResolution) -> bool {
    relative_match(
        left.x,
        right.x,
        shared::schemas::RASTER_RESOLUTION_RELATIVE_TOLERANCE,
    ) && relative_match(
        left.y,
        right.y,
        shared::schemas::RASTER_RESOLUTION_RELATIVE_TOLERANCE,
    )
}

fn relative_match(left: f64, right: f64, tolerance: f64) -> bool {
    let denominator = right.abs().max(1e-9);
    ((left - right).abs() / denominator) <= tolerance
}

fn neighbors(index: usize, width: u32, height: u32) -> Vec<usize> {
    let width = width as usize;
    let height = height as usize;
    let row = index / width;
    let col = index % width;
    let mut cells = Vec::with_capacity(4);
    if col > 0 {
        cells.push(index - 1);
    }
    if col + 1 < width {
        cells.push(index + 1);
    }
    if row > 0 {
        cells.push(index - width);
    }
    if row + 1 < height {
        cells.push(index + width);
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::{GeoBounds, RasterResolution, RasterSpatialRef};

    #[test]
    fn classification_matches_nearest_signature_and_delineates_zone() {
        let request = request(vec![
            snapshot(
                "scene-2026-05-01",
                "2026-05-01T00:00:00Z",
                vec![0.7, 0.72, 0.68, 0.74],
            ),
            snapshot(
                "scene-2026-06-01",
                "2026-06-01T00:00:00Z",
                vec![0.71, 0.73, 0.67, 0.75],
            ),
        ]);
        let result =
            analyze_index_vegetation_type_classification(request).expect("classification succeeds");

        assert_eq!(result.snapshots_used, 2);
        assert_eq!(result.current_scene_id, "scene-2026-06-01");
        assert_eq!(result.decision.label(), "available");
        assert_eq!(result.class_stats.len(), 1);
        assert_eq!(result.class_stats[0].class_name, "forest");
        assert_eq!(result.zones.len(), 1);
        assert_eq!(result.zones[0].class_name, "forest");
        assert!((result.zones[0].mean_confidence - 1.0).abs() < 0.7);
        assert_eq!(result.zones[0].matched_signature_name, "forest");
        assert_eq!(result.zones[0].coverage_fraction, 1.0);
        assert!(!result.zones[0].matched_signature_evidence.is_empty());
    }

    #[test]
    fn uncalibrated_snapshots_mark_low_confidence() {
        let mut request = request(vec![
            snapshot(
                "scene-2026-05-01",
                "2026-05-01T00:00:00Z",
                vec![0.45, 0.47, 0.49, 0.48],
            ),
            snapshot(
                "scene-2026-06-01",
                "2026-06-01T00:00:00Z",
                vec![0.43, 0.46, 0.44, 0.45],
            ),
        ]);
        request.snapshots[1].calibrated = false;

        let result = analyze_index_vegetation_type_classification(request)
            .expect("classification still executes with reduced confidence");
        assert_eq!(result.decision.label(), "low_confidence");
    }

    #[test]
    fn insufficient_snapshots_fails_fast() {
        let request = request(vec![snapshot(
            "scene-2026-05-01",
            "2026-05-01T00:00:00Z",
            vec![0.2],
        )]);
        let error = analyze_index_vegetation_type_classification(request)
            .expect_err("single snapshot is insufficient");
        assert!(matches!(
            error,
            IndexVegetationTypeClassificationError::InsufficientSnapshots
        ));
    }

    #[test]
    fn mismatched_crs_is_unavailable() {
        let request = request(vec![
            snapshot(
                "scene-2026-05-01",
                "2026-05-01T00:00:00Z",
                vec![0.41, 0.42, 0.43, 0.44],
            ),
            snapshot_with_crs(
                "scene-2026-06-01",
                "2026-06-01T00:00:00Z",
                vec![0.42, 0.41, 0.45, 0.46],
                "EPSG:3857",
            ),
        ]);
        let result = analyze_index_vegetation_type_classification(request)
            .expect("analysis returns an unavailable result");

        assert!(matches!(
            result.decision,
            VegetationTypeClassificationDecision::Unavailable { .. }
        ));
    }

    fn request(
        snapshots: Vec<IndexVegetationTypeClassificationSnapshot>,
    ) -> IndexVegetationTypeClassificationRequest {
        IndexVegetationTypeClassificationRequest {
            snapshots,
            signature_library: Some(vec![
                VegetationTypeSignature {
                    name: "cotton".to_string(),
                    mean_ndvi: 0.42,
                    std_ndvi: 0.16,
                    trend_ndvi: 0.0,
                    mean_tolerance: 0.22,
                    std_tolerance: 0.19,
                    trend_tolerance: 0.20,
                },
                VegetationTypeSignature {
                    name: "forest".to_string(),
                    mean_ndvi: 0.72,
                    std_ndvi: 0.08,
                    trend_ndvi: 0.0,
                    mean_tolerance: 0.18,
                    std_tolerance: 0.16,
                    trend_tolerance: 0.20,
                },
                VegetationTypeSignature {
                    name: "bush".to_string(),
                    mean_ndvi: 0.30,
                    std_ndvi: 0.14,
                    trend_ndvi: -0.01,
                    mean_tolerance: 0.20,
                    std_tolerance: 0.16,
                    trend_tolerance: 0.22,
                },
                VegetationTypeSignature {
                    name: "palm".to_string(),
                    mean_ndvi: 0.60,
                    std_ndvi: 0.12,
                    trend_ndvi: 0.02,
                    mean_tolerance: 0.20,
                    std_tolerance: 0.14,
                    trend_tolerance: 0.24,
                },
                VegetationTypeSignature {
                    name: "rice".to_string(),
                    mean_ndvi: 0.36,
                    std_ndvi: 0.15,
                    trend_ndvi: 0.0,
                    mean_tolerance: 0.24,
                    std_tolerance: 0.20,
                    trend_tolerance: 0.25,
                },
            ]),
        }
    }

    fn snapshot(
        scene_id: &str,
        captured_at: &str,
        values: Vec<f32>,
    ) -> IndexVegetationTypeClassificationSnapshot {
        snapshot_with_crs(scene_id, captured_at, values, "EPSG:32614")
    }

    fn snapshot_with_crs(
        scene_id: &str,
        captured_at: &str,
        values: Vec<f32>,
        crs: &str,
    ) -> IndexVegetationTypeClassificationSnapshot {
        IndexVegetationTypeClassificationSnapshot {
            field_id: "field-a".to_string(),
            scene_id: scene_id.to_string(),
            product_ref: format!("layer-{scene_id}"),
            acquired_at: chrono::DateTime::parse_from_rfc3339(captured_at)
                .expect("capture date is valid")
                .with_timezone(&Utc),
            grid: ProductGrid {
                width: 2,
                height: 2,
                values,
                nodata_mask: vec![false; 4],
                spatial_ref: RasterSpatialRef {
                    georeferenced: true,
                    crs: Some(crs.to_string()),
                    bbox: Some(GeoBounds {
                        min_lon: 500000.0,
                        min_lat: 4500000.0,
                        max_lon: 500020.0,
                        max_lat: 4500020.0,
                    }),
                    geo_transform: Some([500000.0, 10.0, 0.0, 4500020.0, 0.0, -10.0]),
                    resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
                },
            },
            calibrated: true,
        }
    }
}
