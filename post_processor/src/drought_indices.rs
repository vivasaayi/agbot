//! Climatology-based drought indices (satellite intelligence pipeline,
//! Phase 3 item 8): VCI, TCI, and VHI.
//!
//! Given a current-period single-index raster plus the matching
//! [`crate::index_climatology`] period baseline, this module computes the
//! standard remote-sensing drought indices:
//!
//! - **VCI** `100·(NDVI − NDVImin)/(NDVImax − NDVImin)` — Vegetation
//!   Condition Index (Kogan 1990/1995). High = healthy relative to the
//!   pixel's historical range.
//! - **TCI** `100·(LSTmax − LST)/(LSTmax − LSTmin)` — Temperature Condition
//!   Index (Kogan 1995), inverted so high temperature = stress = low TCI.
//! - **VHI** `α·VCI + (1−α)·TCI` — Vegetation Health Index (Kogan 1997),
//!   `α` default 0.5.
//!
//! Severity classes (Kogan drought convention, VCI/VHI, percent):
//! `< 10` extreme, `10–20` severe, `20–30` moderate, `30–40` mild, `≥ 40`
//! none. Class boundaries are lower-inclusive (a value of exactly 10 is
//! `Severe`, 40 is `NoDrought`).
//!
//! Conventions (mirroring [`crate::temporal_composite`] /
//! [`crate::index_climatology`]): grids must match exactly (typed errors, no
//! resampling); degenerate baselines (max ≈ min) are reason-coded invalid,
//! not division blow-ups; below-baseline climatology pixels propagate as a
//! distinct invalid reason; every output carries a deterministic evidence
//! fingerprint and maps to an L3 [`ProductRecordDraft`].

use crate::evidence::{deterministic_fingerprint, AnalysisEvidenceError};
use crate::index_climatology::{ClimatologyPeriodStats, ClimatologyPixelReason, IndexClimatology};
use crate::l3_product::{to_l3_draft, L3DraftContext};
use serde::{Deserialize, Serialize};
use shared::product_graph::ProductRecordDraft;
use shared::schemas::{assert_raster_spatial_ref, RasterSpatialRef, RasterSpatialRefError};
use thiserror::Error;

/// Default VHI vegetation weight `α` (Kogan: equal 0.5/0.5 blend).
pub const DEFAULT_VHI_ALPHA: f64 = 0.5;

/// Baseline range `max − min` at or below this is treated as degenerate
/// (division would blow up or amplify noise). Index values live in physical
/// units (NDVI ~[-1,1], LST in kelvin); `1e-4` is well below any meaningful
/// climatological spread while excluding exact/near ties.
pub const DEGENERATE_RANGE_EPSILON: f32 = 1.0e-4;

/// Sentinel stored in an index layer wherever the pixel is invalid. Consumers
/// must consult `reason_codes`, never trust the sentinel.
pub const DROUGHT_SENTINEL: f32 = f32::NAN;

/// Which condition index a computation produced (drives algorithm ids and
/// severity applicability).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtIndexKind {
    /// Vegetation Condition Index (from NDVI).
    Vci,
    /// Temperature Condition Index (from LST; inverted).
    Tci,
    /// Vegetation Health Index (VCI/TCI blend).
    Vhi,
}

impl DroughtIndexKind {
    /// Stable snake_case label used in evidence and algorithm ids.
    pub fn label(&self) -> &'static str {
        match self {
            DroughtIndexKind::Vci => "vci",
            DroughtIndexKind::Tci => "tci",
            DroughtIndexKind::Vhi => "vhi",
        }
    }
}

/// Per-pixel drought severity class (Kogan convention on VCI/VHI percent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtSeverity {
    /// `< 10`.
    Extreme,
    /// `[10, 20)`.
    Severe,
    /// `[20, 30)`.
    Moderate,
    /// `[30, 40)`.
    Mild,
    /// `>= 40`.
    NoDrought,
    /// Pixel index is invalid (not classifiable).
    Invalid,
}

/// Classify a valid drought-index percent into a severity class
/// (lower-inclusive boundaries at 10/20/30/40).
pub fn classify_severity(value: f32) -> DroughtSeverity {
    if !value.is_finite() {
        DroughtSeverity::Invalid
    } else if value < 10.0 {
        DroughtSeverity::Extreme
    } else if value < 20.0 {
        DroughtSeverity::Severe
    } else if value < 30.0 {
        DroughtSeverity::Moderate
    } else if value < 40.0 {
        DroughtSeverity::Mild
    } else {
        DroughtSeverity::NoDrought
    }
}

/// Per-pixel outcome code for a drought-index computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtPixelReason {
    /// Index computed from a trusted baseline and a valid current value.
    Computed,
    /// Current-period value is masked-invalid or non-finite.
    NoCurrentObservation,
    /// Climatology baseline for this pixel was below the min-years threshold.
    BelowBaseline,
    /// Baseline range `max − min` is degenerate (<= epsilon).
    DegenerateRange,
    /// (VHI only) one of VCI/TCI was invalid here.
    ComponentInvalid,
}

/// Class-count summary over all pixels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SeverityClassCounts {
    pub extreme: u32,
    pub severe: u32,
    pub moderate: u32,
    pub mild: u32,
    pub no_drought: u32,
    pub invalid: u32,
}

impl SeverityClassCounts {
    fn tally(classes: &[DroughtSeverity]) -> Self {
        let mut counts = SeverityClassCounts::default();
        for class in classes {
            match class {
                DroughtSeverity::Extreme => counts.extreme += 1,
                DroughtSeverity::Severe => counts.severe += 1,
                DroughtSeverity::Moderate => counts.moderate += 1,
                DroughtSeverity::Mild => counts.mild += 1,
                DroughtSeverity::NoDrought => counts.no_drought += 1,
                DroughtSeverity::Invalid => counts.invalid += 1,
            }
        }
        counts
    }
}

/// Evidence object for one drought-index run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtEvidence {
    pub index_kind: DroughtIndexKind,
    /// Catalog product id of the current-period input.
    pub current_product_id: String,
    /// Catalog product id of the climatology input(s).
    pub climatology_product_ids: Vec<String>,
    pub spatial_ref: RasterSpatialRef,
    /// VHI blend weight (only meaningful for VHI; recorded for all).
    pub alpha: f64,
    pub degenerate_range_epsilon: f32,
    /// Deterministic canonical-JSON FNV fingerprint over the full input.
    pub input_hash: String,
}

/// A completed drought-index raster with per-pixel layers and summaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtIndexResult {
    pub index_kind: DroughtIndexKind,
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    /// Index values in percent [0, 100]; [`DROUGHT_SENTINEL`] where invalid.
    pub values: Vec<f32>,
    pub reason_codes: Vec<DroughtPixelReason>,
    /// Per-pixel severity class ([`DroughtSeverity::Invalid`] where invalid).
    pub severity: Vec<DroughtSeverity>,
    pub severity_counts: SeverityClassCounts,
    /// How many valid values were clamped to [0, 100].
    pub clamp_count: u32,
    /// Fraction of pixels with a computed value.
    pub valid_fraction: f32,
    pub current_product_id: String,
    pub climatology_product_ids: Vec<String>,
    pub evidence: DroughtEvidence,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum DroughtIndexError {
    #[error("current-period raster has {actual} pixels, expected {expected}")]
    CurrentLengthMismatch { expected: usize, actual: usize },
    #[error("current-period validity mask has {actual} pixels, expected {expected}")]
    MaskLengthMismatch { expected: usize, actual: usize },
    #[error("climatology period stats have {actual} pixels, expected {expected} (grid mismatch)")]
    ClimatologyLengthMismatch { expected: usize, actual: usize },
    #[error("current-period spatial ref does not match the grid (no resampling here)")]
    CurrentSpatialRefMismatch,
    #[error("climatology spatial ref does not match the grid (no resampling here)")]
    ClimatologySpatialRefMismatch,
    #[error("grid spatial metadata is invalid: {reason}")]
    SpatialRef { reason: RasterSpatialRefError },
    #[error("VHI alpha must be within [0, 1] (got {alpha})")]
    InvalidAlpha { alpha: f64 },
    #[error("rehydrated drought raster pixel {pixel} is {value}, outside the [0, 100] percent domain (not a drought raster?)")]
    RehydratedValueOutOfRange { pixel: usize, value: f32 },
    #[error("VHI requires VCI and TCI computed on the same grid ({field} differ)")]
    ComponentGridMismatch { field: &'static str },
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] AnalysisEvidenceError),
}

/// Current-period single-index raster to score against a climatology.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroughtCurrentRaster {
    /// Catalog product id of the current-period input (identity-bearing).
    pub product_id: String,
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    /// Index values, row-major.
    pub values: Vec<f32>,
    /// `true` = usable pixel.
    pub valid_mask: Vec<bool>,
}

/// Compute VCI from a current NDVI raster and its climatology period.
/// `100·(NDVI − min)/(max − min)`, clamped to [0, 100].
pub fn compute_vci(
    current: &DroughtCurrentRaster,
    climatology: &IndexClimatology,
    period: &ClimatologyPeriodStats,
) -> Result<DroughtIndexResult, DroughtIndexError> {
    compute_condition_index(current, climatology, period, DroughtIndexKind::Vci)
}

/// Compute TCI from a current LST raster and its climatology period.
/// `100·(max − LST)/(max − min)` (inverted), clamped to [0, 100].
pub fn compute_tci(
    current: &DroughtCurrentRaster,
    climatology: &IndexClimatology,
    period: &ClimatologyPeriodStats,
) -> Result<DroughtIndexResult, DroughtIndexError> {
    compute_condition_index(current, climatology, period, DroughtIndexKind::Tci)
}

fn compute_condition_index(
    current: &DroughtCurrentRaster,
    climatology: &IndexClimatology,
    period: &ClimatologyPeriodStats,
    kind: DroughtIndexKind,
) -> Result<DroughtIndexResult, DroughtIndexError> {
    let pixel_count = validate_condition_inputs(current, climatology, period)?;

    let mut values = vec![DROUGHT_SENTINEL; pixel_count];
    let mut reason_codes = vec![DroughtPixelReason::Computed; pixel_count];
    let mut severity = vec![DroughtSeverity::Invalid; pixel_count];
    let mut clamp_count = 0u32;
    let mut valid = 0u32;

    for pixel in 0..pixel_count {
        let value = current.values[pixel];
        if !current.valid_mask[pixel] || !value.is_finite() {
            reason_codes[pixel] = DroughtPixelReason::NoCurrentObservation;
            continue;
        }
        if period.reason_codes[pixel] != ClimatologyPixelReason::Ok {
            reason_codes[pixel] = DroughtPixelReason::BelowBaseline;
            continue;
        }
        let min = period.min[pixel];
        let max = period.max[pixel];
        let range = max - min;
        if range <= DEGENERATE_RANGE_EPSILON {
            reason_codes[pixel] = DroughtPixelReason::DegenerateRange;
            continue;
        }
        let raw = match kind {
            DroughtIndexKind::Vci => 100.0 * (value - min) / range,
            DroughtIndexKind::Tci => 100.0 * (max - value) / range,
            DroughtIndexKind::Vhi => unreachable!("VHI is computed via compute_vhi"),
        };
        let clamped = raw.clamp(0.0, 100.0);
        if clamped != raw {
            clamp_count += 1;
        }
        values[pixel] = clamped;
        severity[pixel] = classify_severity(clamped);
        valid += 1;
    }

    finish_result(
        FinishContext {
            kind,
            current_product_id: current.product_id.clone(),
            climatology_product_ids: climatology_product_ids(climatology),
            spatial_ref: current.spatial_ref.clone(),
            alpha: DEFAULT_VHI_ALPHA,
            width: current.width,
            height: current.height,
        },
        values,
        reason_codes,
        severity,
        clamp_count,
        valid,
        &(
            "drought_condition_v1",
            kind,
            &current.product_id,
            &current.values,
            &current.valid_mask,
            &period.min,
            &period.max,
            &period.reason_codes,
            &current.spatial_ref,
        ),
    )
}

/// Compute VHI `α·VCI + (1−α)·TCI` per pixel. Requires VCI and TCI computed
/// on the same grid; a pixel is valid only where both components are valid.
pub fn compute_vhi(
    vci: &DroughtIndexResult,
    tci: &DroughtIndexResult,
    alpha: f64,
) -> Result<DroughtIndexResult, DroughtIndexError> {
    if !(0.0..=1.0).contains(&alpha) {
        return Err(DroughtIndexError::InvalidAlpha { alpha });
    }
    if vci.width != tci.width || vci.height != tci.height {
        return Err(DroughtIndexError::ComponentGridMismatch {
            field: "dimensions",
        });
    }
    if vci.spatial_ref != tci.spatial_ref {
        return Err(DroughtIndexError::ComponentGridMismatch {
            field: "spatial_ref",
        });
    }
    if vci.values.len() != tci.values.len() {
        return Err(DroughtIndexError::ComponentGridMismatch { field: "length" });
    }

    let pixel_count = vci.values.len();
    let mut values = vec![DROUGHT_SENTINEL; pixel_count];
    let mut reason_codes = vec![DroughtPixelReason::Computed; pixel_count];
    let mut severity = vec![DroughtSeverity::Invalid; pixel_count];
    let mut valid = 0u32;

    for pixel in 0..pixel_count {
        let vci_value = vci.values[pixel];
        let tci_value = tci.values[pixel];
        if !vci_value.is_finite() || !tci_value.is_finite() {
            reason_codes[pixel] = DroughtPixelReason::ComponentInvalid;
            continue;
        }
        // Blend of two already-clamped [0, 100] values stays in range.
        let blended = (alpha * vci_value as f64 + (1.0 - alpha) * tci_value as f64) as f32;
        values[pixel] = blended;
        severity[pixel] = classify_severity(blended);
        valid += 1;
    }

    let mut climatology_product_ids = vci.climatology_product_ids.clone();
    for product_id in &tci.climatology_product_ids {
        if !climatology_product_ids.contains(product_id) {
            climatology_product_ids.push(product_id.clone());
        }
    }
    // VHI's current inputs are the VCI and TCI current products.
    let mut current_ids = vec![vci.current_product_id.clone()];
    if tci.current_product_id != vci.current_product_id {
        current_ids.push(tci.current_product_id.clone());
    }

    finish_result(
        FinishContext {
            kind: DroughtIndexKind::Vhi,
            current_product_id: current_ids.join(","),
            climatology_product_ids,
            spatial_ref: vci.spatial_ref.clone(),
            alpha,
            width: vci.width,
            height: vci.height,
        },
        values,
        reason_codes,
        severity,
        0,
        valid,
        &(
            "drought_vhi_v1",
            alpha,
            &vci.evidence.input_hash,
            &tci.evidence.input_hash,
        ),
    )
}

/// A persisted drought-index raster read back from its registered GeoTIFF,
/// ready to rehydrate into a [`DroughtIndexResult`] so stored VCI/TCI L3
/// products can feed [`compute_vhi`] without recomputing their climatology.
#[derive(Debug, Clone)]
pub struct RehydratedDroughtRaster {
    pub kind: DroughtIndexKind,
    /// Catalog product id of the persisted drought product itself (it is
    /// the current-period input from the blend's point of view).
    pub product_id: String,
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    /// Index percent values, row-major (nodata already mapped out of
    /// `valid_mask`).
    pub values: Vec<f32>,
    pub valid_mask: Vec<bool>,
}

/// Rehydrate a persisted drought raster into a [`DroughtIndexResult`].
/// Severity is re-derived from the values; invalid pixels are reason-coded
/// [`DroughtPixelReason::NoCurrentObservation`]. Valid values must be in
/// the drought percent domain `[0, 100]` — anything else means the artifact
/// is not a drought raster and is refused.
pub fn drought_result_from_raster(
    raster: &RehydratedDroughtRaster,
) -> Result<DroughtIndexResult, DroughtIndexError> {
    assert_raster_spatial_ref(Some(&raster.spatial_ref), raster.width, raster.height)
        .map_err(|reason| DroughtIndexError::SpatialRef { reason })?;
    let pixel_count = raster.width as usize * raster.height as usize;
    if raster.values.len() != pixel_count {
        return Err(DroughtIndexError::CurrentLengthMismatch {
            expected: pixel_count,
            actual: raster.values.len(),
        });
    }
    if raster.valid_mask.len() != pixel_count {
        return Err(DroughtIndexError::MaskLengthMismatch {
            expected: pixel_count,
            actual: raster.valid_mask.len(),
        });
    }

    let mut values = vec![DROUGHT_SENTINEL; pixel_count];
    let mut reason_codes = vec![DroughtPixelReason::Computed; pixel_count];
    let mut severity = vec![DroughtSeverity::Invalid; pixel_count];
    let mut valid = 0u32;
    for pixel in 0..pixel_count {
        let value = raster.values[pixel];
        if !raster.valid_mask[pixel] || !value.is_finite() {
            reason_codes[pixel] = DroughtPixelReason::NoCurrentObservation;
            continue;
        }
        if !(0.0..=100.0).contains(&value) {
            return Err(DroughtIndexError::RehydratedValueOutOfRange { pixel, value });
        }
        values[pixel] = value;
        severity[pixel] = classify_severity(value);
        valid += 1;
    }

    finish_result(
        FinishContext {
            kind: raster.kind,
            current_product_id: raster.product_id.clone(),
            climatology_product_ids: Vec::new(),
            spatial_ref: raster.spatial_ref.clone(),
            alpha: DEFAULT_VHI_ALPHA,
            width: raster.width,
            height: raster.height,
        },
        values,
        reason_codes,
        severity,
        0,
        valid,
        &(
            "drought_rehydrate_v1",
            raster.kind,
            &raster.product_id,
            &raster.values,
            &raster.valid_mask,
            &raster.spatial_ref,
        ),
    )
}

struct FinishContext {
    kind: DroughtIndexKind,
    current_product_id: String,
    climatology_product_ids: Vec<String>,
    spatial_ref: RasterSpatialRef,
    alpha: f64,
    width: u32,
    height: u32,
}

#[allow(clippy::too_many_arguments)]
fn finish_result<T: Serialize>(
    ctx: FinishContext,
    values: Vec<f32>,
    reason_codes: Vec<DroughtPixelReason>,
    severity: Vec<DroughtSeverity>,
    clamp_count: u32,
    valid: u32,
    hash_input: &T,
) -> Result<DroughtIndexResult, DroughtIndexError> {
    let pixel_count = values.len();
    let severity_counts = SeverityClassCounts::tally(&severity);
    let input_hash = deterministic_fingerprint(hash_input)?;
    let evidence = DroughtEvidence {
        index_kind: ctx.kind,
        current_product_id: ctx.current_product_id.clone(),
        climatology_product_ids: ctx.climatology_product_ids.clone(),
        spatial_ref: ctx.spatial_ref.clone(),
        alpha: ctx.alpha,
        degenerate_range_epsilon: DEGENERATE_RANGE_EPSILON,
        input_hash,
    };
    Ok(DroughtIndexResult {
        index_kind: ctx.kind,
        width: ctx.width,
        height: ctx.height,
        spatial_ref: ctx.spatial_ref,
        values,
        reason_codes,
        severity,
        severity_counts,
        clamp_count,
        valid_fraction: if pixel_count == 0 {
            0.0
        } else {
            valid as f32 / pixel_count as f32
        },
        current_product_id: ctx.current_product_id,
        climatology_product_ids: ctx.climatology_product_ids,
        evidence,
    })
}

fn climatology_product_ids(climatology: &IndexClimatology) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for provenance in &climatology.evidence.periods {
        for product_id in &provenance.product_ids {
            if !ids.contains(product_id) {
                ids.push(product_id.clone());
            }
        }
    }
    ids
}

fn validate_condition_inputs(
    current: &DroughtCurrentRaster,
    climatology: &IndexClimatology,
    period: &ClimatologyPeriodStats,
) -> Result<usize, DroughtIndexError> {
    assert_raster_spatial_ref(Some(&current.spatial_ref), current.width, current.height)
        .map_err(|reason| DroughtIndexError::SpatialRef { reason })?;
    let pixel_count = current.width as usize * current.height as usize;
    if current.values.len() != pixel_count {
        return Err(DroughtIndexError::CurrentLengthMismatch {
            expected: pixel_count,
            actual: current.values.len(),
        });
    }
    if current.valid_mask.len() != pixel_count {
        return Err(DroughtIndexError::MaskLengthMismatch {
            expected: pixel_count,
            actual: current.valid_mask.len(),
        });
    }
    if current.spatial_ref != climatology.spatial_ref {
        return Err(DroughtIndexError::ClimatologySpatialRefMismatch);
    }
    for (layer_len, _label) in [
        (period.min.len(), "min"),
        (period.max.len(), "max"),
        (period.reason_codes.len(), "reason_codes"),
    ] {
        if layer_len != pixel_count {
            return Err(DroughtIndexError::ClimatologyLengthMismatch {
                expected: pixel_count,
                actual: layer_len,
            });
        }
    }
    Ok(pixel_count)
}

// ---------------------------------------------------------------------------
// L3 lineage
// ---------------------------------------------------------------------------

/// Scope a drought-index L3 draft cannot derive from pixel data alone.
#[derive(Debug, Clone)]
pub struct DroughtL3Scope {
    pub field_id: String,
    pub season_id: String,
    pub scene_id: Option<String>,
    /// Current period ISO bounds for scope (e.g. the composite window).
    pub temporal_start: String,
    pub temporal_end: String,
    pub source_id: Option<String>,
}

/// Map a drought-index result to an L3 catalog draft. Lineage = current-period
/// product + climatology product ids (all `l2_input` edges — same invariant as
/// [`crate::l3_product`]). Confidence is the valid-coverage fraction, mirroring
/// [`crate::temporal_composite::composite_l3_draft`].
pub fn drought_l3_draft(result: &DroughtIndexResult, scope: &DroughtL3Scope) -> ProductRecordDraft {
    let mut input_product_ids: Vec<String> = Vec::new();
    // Current product id may be a comma-joined pair for VHI.
    for product_id in result.current_product_id.split(',') {
        let product_id = product_id.to_string();
        if !product_id.is_empty() && !input_product_ids.contains(&product_id) {
            input_product_ids.push(product_id);
        }
    }
    for product_id in &result.climatology_product_ids {
        if !input_product_ids.contains(product_id) {
            input_product_ids.push(product_id.clone());
        }
    }

    to_l3_draft(&L3DraftContext {
        kind: "drought_index".to_string(),
        algorithm_id: format!("drought.{}", result.index_kind.label()),
        algorithm_version: "1.0.0".to_string(),
        field_id: scope.field_id.clone(),
        season_id: scope.season_id.clone(),
        scene_id: scope.scene_id.clone(),
        temporal_start: scope.temporal_start.clone(),
        temporal_end: scope.temporal_end.clone(),
        input_product_ids,
        parameters: serde_json::json!({
            "index_kind": result.index_kind,
            "alpha": result.evidence.alpha,
            "degenerate_range_epsilon": result.evidence.degenerate_range_epsilon,
            "severity_convention": "kogan_vci_percent_10_20_30_40",
        }),
        confidence: Some(result.valid_fraction as f64),
        confidence_method: Some("valid_coverage_fraction".to_string()),
        evidence_digests: vec![result.evidence.input_hash.clone()],
        source_id: scope.source_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_climatology::{
        build_index_climatology, CalendarPeriod, ClimatologyObservation, ClimatologyRequest,
    };
    use crate::temporal_composite::CompositeCadence;
    use chrono::NaiveDate;
    use shared::product_graph::ProductLevel;
    use shared::schemas::{GeoBounds, RasterResolution};

    fn spatial_ref_2x1() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32614".to_string()),
            bbox: Some(GeoBounds {
                min_lon: 500000.0,
                min_lat: 4500000.0,
                max_lon: 500020.0,
                max_lat: 4500010.0,
            }),
            geo_transform: Some([500000.0, 10.0, 0.0, 4500010.0, 0.0, -10.0]),
            resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
        }
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid test date")
    }

    fn obs(id: &str, on: NaiveDate, values: Vec<f32>) -> ClimatologyObservation {
        ClimatologyObservation {
            product_id: format!("clim:{id}"),
            observed_on: on,
            values,
            valid_mask: vec![true; 2],
            spatial_ref: spatial_ref_2x1(),
        }
    }

    /// June climatology on a 2x1 grid: pixel 0 min 0.2 max 0.6 (range 0.4),
    /// pixel 1 min 0.5 max 0.5 (degenerate range).
    fn ndvi_climatology() -> IndexClimatology {
        let request = ClimatologyRequest {
            index_kind: "ndvi".to_string(),
            cadence: CompositeCadence::Monthly,
            width: 2,
            height: 1,
            spatial_ref: spatial_ref_2x1(),
            min_years: 3,
            observations: vec![
                obs("2020", date(2020, 6, 1), vec![0.2, 0.5]),
                obs("2021", date(2021, 6, 1), vec![0.4, 0.5]),
                obs("2022", date(2022, 6, 1), vec![0.6, 0.5]),
            ],
        };
        build_index_climatology(&request).expect("climatology builds")
    }

    fn june_period(climatology: &IndexClimatology) -> ClimatologyPeriodStats {
        climatology
            .period_stats(CalendarPeriod {
                month: 6,
                dekad: None,
            })
            .expect("June stats")
            .clone()
    }

    fn current(values: Vec<f32>, mask: Vec<bool>) -> DroughtCurrentRaster {
        DroughtCurrentRaster {
            product_id: "current:2026-06".to_string(),
            width: 2,
            height: 1,
            spatial_ref: spatial_ref_2x1(),
            values,
            valid_mask: mask,
        }
    }

    #[test]
    fn vci_matches_hand_computation_and_flags_degenerate_range() {
        let climatology = ndvi_climatology();
        let period = june_period(&climatology);
        // Pixel 0: NDVI 0.4, min 0.2 max 0.6 -> 100*(0.2)/(0.4) = 50.
        // Pixel 1: degenerate range.
        let current = current(vec![0.4, 0.55], vec![true, true]);
        let result = compute_vci(&current, &climatology, &period).expect("vci");
        assert!((result.values[0] - 50.0).abs() < 1.0e-4);
        assert_eq!(result.reason_codes[0], DroughtPixelReason::Computed);
        assert_eq!(result.severity[0], DroughtSeverity::NoDrought);
        assert!(result.values[1].is_nan());
        assert_eq!(result.reason_codes[1], DroughtPixelReason::DegenerateRange);
        assert_eq!(result.severity[1], DroughtSeverity::Invalid);
        assert_eq!(result.valid_fraction, 0.5);
    }

    #[test]
    fn vci_clamps_out_of_range_values_and_counts_clamps() {
        let climatology = ndvi_climatology();
        let period = june_period(&climatology);
        // Pixel 0: NDVI 0.8 above max 0.6 -> raw 150 -> clamped 100.
        let high = current(vec![0.8, 0.1], vec![true, false]);
        let result = compute_vci(&high, &climatology, &period).expect("vci");
        assert_eq!(result.values[0], 100.0);
        assert_eq!(result.clamp_count, 1);
        assert_eq!(result.severity[0], DroughtSeverity::NoDrought);
        // Below-min NDVI also clamps to 0.
        let low = current(vec![0.0, 0.1], vec![true, false]);
        let result = compute_vci(&low, &climatology, &period).expect("vci");
        assert_eq!(result.values[0], 0.0);
        assert_eq!(result.severity[0], DroughtSeverity::Extreme);
    }

    #[test]
    fn below_baseline_climatology_pixels_propagate_as_invalid() {
        // min_years 5 forces every pixel below baseline (only 3 years).
        let request = ClimatologyRequest {
            index_kind: "ndvi".to_string(),
            cadence: CompositeCadence::Monthly,
            width: 2,
            height: 1,
            spatial_ref: spatial_ref_2x1(),
            min_years: 5,
            observations: vec![
                obs("2020", date(2020, 6, 1), vec![0.2, 0.5]),
                obs("2021", date(2021, 6, 1), vec![0.4, 0.6]),
                obs("2022", date(2022, 6, 1), vec![0.6, 0.7]),
            ],
        };
        let climatology = build_index_climatology(&request).expect("climatology");
        let period = june_period(&climatology);
        let current = current(vec![0.4, 0.6], vec![true, true]);
        let result = compute_vci(&current, &climatology, &period).expect("vci");
        assert!(result
            .reason_codes
            .iter()
            .all(|reason| *reason == DroughtPixelReason::BelowBaseline));
        assert_eq!(result.valid_fraction, 0.0);
    }

    #[test]
    fn masked_current_pixel_is_no_current_observation() {
        let climatology = ndvi_climatology();
        let period = june_period(&climatology);
        let current = current(vec![0.4, 0.55], vec![false, true]);
        let result = compute_vci(&current, &climatology, &period).expect("vci");
        assert_eq!(
            result.reason_codes[0],
            DroughtPixelReason::NoCurrentObservation
        );
    }

    /// LST climatology on a 2x1 grid: both pixels min 300 max 320 (range 20).
    fn lst_climatology() -> IndexClimatology {
        let request = ClimatologyRequest {
            index_kind: "lst".to_string(),
            cadence: CompositeCadence::Monthly,
            width: 2,
            height: 1,
            spatial_ref: spatial_ref_2x1(),
            min_years: 3,
            observations: vec![
                obs("2020", date(2020, 6, 1), vec![300.0, 300.0]),
                obs("2021", date(2021, 6, 1), vec![310.0, 310.0]),
                obs("2022", date(2022, 6, 1), vec![320.0, 320.0]),
            ],
        };
        build_index_climatology(&request).expect("climatology builds")
    }

    #[test]
    fn tci_inverts_so_a_hot_pixel_scores_low() {
        let climatology = lst_climatology();
        let period = june_period(&climatology);
        // Pixel 0 hot (318 -> TCI 100*(320-318)/20 = 10),
        // pixel 1 cool (302 -> TCI 100*(320-302)/20 = 90).
        let current = current(vec![318.0, 302.0], vec![true, true]);
        let result = compute_tci(&current, &climatology, &period).expect("tci");
        assert!((result.values[0] - 10.0).abs() < 1.0e-3);
        assert!((result.values[1] - 90.0).abs() < 1.0e-3);
        assert!(
            result.values[0] < result.values[1],
            "hotter pixel scores lower TCI"
        );
        assert_eq!(result.index_kind, DroughtIndexKind::Tci);
    }

    #[test]
    fn vhi_blends_vci_and_tci_only_where_both_valid() {
        let ndvi_clim = ndvi_climatology();
        let ndvi_period = june_period(&ndvi_clim);
        // Pixel 0 VCI 50; pixel 1 degenerate -> VCI invalid.
        let vci = compute_vci(
            &current(vec![0.4, 0.55], vec![true, true]),
            &ndvi_clim,
            &ndvi_period,
        )
        .expect("vci");

        let lst_clim = lst_climatology();
        let lst_period = june_period(&lst_clim);
        // Pixel 0 TCI: 310 -> 100*(320-310)/20 = 50; pixel 1 TCI 90.
        let tci = compute_tci(
            &current(vec![310.0, 302.0], vec![true, true]),
            &lst_clim,
            &lst_period,
        )
        .expect("tci");

        let vhi = compute_vhi(&vci, &tci, 0.5).expect("vhi");
        // Pixel 0: 0.5*50 + 0.5*50 = 50.
        assert!((vhi.values[0] - 50.0).abs() < 1.0e-4);
        assert_eq!(vhi.reason_codes[0], DroughtPixelReason::Computed);
        // Pixel 1: VCI invalid -> VHI component invalid.
        assert!(vhi.values[1].is_nan());
        assert_eq!(vhi.reason_codes[1], DroughtPixelReason::ComponentInvalid);

        // Alpha weighting shifts toward the vegetation term.
        let vci2 = compute_vci(
            &current(vec![0.6, 0.55], vec![true, true]),
            &ndvi_clim,
            &ndvi_period,
        )
        .expect("vci"); // pixel 0 VCI 100
        let vhi_alpha = compute_vhi(&vci2, &tci, 0.75).expect("vhi");
        // 0.75*100 + 0.25*50 = 87.5.
        assert!((vhi_alpha.values[0] - 87.5).abs() < 1.0e-3);
    }

    #[test]
    fn vhi_rejects_bad_alpha_and_mismatched_grids() {
        let clim = ndvi_climatology();
        let period = june_period(&clim);
        let vci =
            compute_vci(&current(vec![0.4, 0.55], vec![true, true]), &clim, &period).expect("vci");
        let tci = vci.clone();
        assert_eq!(
            compute_vhi(&vci, &tci, 1.5).expect_err("alpha out of range"),
            DroughtIndexError::InvalidAlpha { alpha: 1.5 }
        );
        let mut wrong = tci.clone();
        wrong.width = 1;
        wrong.height = 1;
        assert_eq!(
            compute_vhi(&vci, &wrong, 0.5).expect_err("dim mismatch"),
            DroughtIndexError::ComponentGridMismatch {
                field: "dimensions"
            }
        );
    }

    #[test]
    fn severity_classes_use_lower_inclusive_boundaries() {
        assert_eq!(classify_severity(9.99), DroughtSeverity::Extreme);
        assert_eq!(classify_severity(10.0), DroughtSeverity::Severe);
        assert_eq!(classify_severity(19.99), DroughtSeverity::Severe);
        assert_eq!(classify_severity(20.0), DroughtSeverity::Moderate);
        assert_eq!(classify_severity(29.99), DroughtSeverity::Moderate);
        assert_eq!(classify_severity(30.0), DroughtSeverity::Mild);
        assert_eq!(classify_severity(39.99), DroughtSeverity::Mild);
        assert_eq!(classify_severity(40.0), DroughtSeverity::NoDrought);
        assert_eq!(classify_severity(100.0), DroughtSeverity::NoDrought);
        assert_eq!(classify_severity(f32::NAN), DroughtSeverity::Invalid);
    }

    #[test]
    fn severity_counts_tally_the_class_layer() {
        let climatology = ndvi_climatology();
        let period = june_period(&climatology);
        // Pixel 0 VCI 50 (NoDrought), pixel 1 degenerate (Invalid).
        let result = compute_vci(
            &current(vec![0.4, 0.55], vec![true, true]),
            &climatology,
            &period,
        )
        .expect("vci");
        assert_eq!(result.severity_counts.no_drought, 1);
        assert_eq!(result.severity_counts.invalid, 1);
        assert_eq!(result.severity_counts.extreme, 0);
    }

    #[test]
    fn identical_runs_produce_stable_evidence_hashes() {
        let climatology = ndvi_climatology();
        let period = june_period(&climatology);
        let raster = current(vec![0.4, 0.55], vec![true, true]);
        let first = compute_vci(&raster, &climatology, &period).expect("vci");
        let second = compute_vci(&raster, &climatology, &period).expect("vci");
        assert_eq!(first.evidence.input_hash, second.evidence.input_hash);
        // A different current value changes the hash.
        let third = compute_vci(
            &current(vec![0.5, 0.55], vec![true, true]),
            &climatology,
            &period,
        )
        .expect("vci");
        assert_ne!(first.evidence.input_hash, third.evidence.input_hash);
    }

    #[test]
    fn l3_draft_carries_lineage_parameters_and_stable_identity() {
        let climatology = ndvi_climatology();
        let period = june_period(&climatology);
        let result = compute_vci(
            &current(vec![0.4, 0.55], vec![true, true]),
            &climatology,
            &period,
        )
        .expect("vci");
        let scope = DroughtL3Scope {
            field_id: "field-1".to_string(),
            season_id: "2026".to_string(),
            scene_id: None,
            temporal_start: "2026-06-01T00:00:00Z".to_string(),
            temporal_end: "2026-06-30T23:59:59Z".to_string(),
            source_id: None,
        };
        let draft = drought_l3_draft(&result, &scope);
        assert_eq!(draft.level, ProductLevel::L3);
        assert_eq!(draft.kind, "drought_index");
        assert_eq!(draft.algorithm_id, "drought.vci");
        let input_ids: Vec<&str> = draft
            .inputs
            .iter()
            .map(|input| input.product_id.as_str())
            .collect();
        assert_eq!(
            input_ids,
            vec!["current:2026-06", "clim:2020", "clim:2021", "clim:2022"]
        );
        assert!(draft.inputs.iter().all(|input| input.role == "l2_input"));
        assert_eq!(draft.parameters["index_kind"], "vci");
        assert_eq!(draft.parameters["alpha"], 0.5);
        assert_eq!(draft.confidence, Some(result.valid_fraction as f64));

        let again = drought_l3_draft(
            &compute_vci(
                &current(vec![0.4, 0.55], vec![true, true]),
                &climatology,
                &period,
            )
            .expect("vci"),
            &scope,
        );
        assert_eq!(draft.parameters_hash(), again.parameters_hash());
    }

    #[test]
    fn current_grid_mismatch_is_a_typed_error() {
        let climatology = ndvi_climatology();
        let period = june_period(&climatology);
        let mut raster = current(vec![0.4, 0.55], vec![true, true]);
        raster.values.pop();
        assert_eq!(
            compute_vci(&raster, &climatology, &period).expect_err("short values"),
            DroughtIndexError::CurrentLengthMismatch {
                expected: 2,
                actual: 1
            }
        );
        let mut raster = current(vec![0.4, 0.55], vec![true, true]);
        raster.spatial_ref.crs = Some("EPSG:32615".to_string());
        assert_eq!(
            compute_vci(&raster, &climatology, &period).expect_err("foreign CRS"),
            DroughtIndexError::ClimatologySpatialRefMismatch
        );
    }

    fn rehydrated(
        kind: DroughtIndexKind,
        id: &str,
        values: Vec<f32>,
        valid_mask: Vec<bool>,
    ) -> RehydratedDroughtRaster {
        RehydratedDroughtRaster {
            kind,
            product_id: id.to_string(),
            width: 2,
            height: 1,
            spatial_ref: spatial_ref_2x1(),
            values,
            valid_mask,
        }
    }

    #[test]
    fn rehydrated_rasters_blend_into_vhi() {
        // Persisted VCI [80, 20] and TCI [40, nodata]: severity re-derived
        // (80 no_drought, 20 moderate), then the equal-weight blend gives
        // pixel 0 = 0.5*80 + 0.5*40 = 60 and pixel 1 component-invalid.
        let vci = drought_result_from_raster(&rehydrated(
            DroughtIndexKind::Vci,
            "l3:vci",
            vec![80.0, 20.0],
            vec![true, true],
        ))
        .expect("vci rehydrates");
        assert_eq!(vci.severity_counts.no_drought, 1);
        assert_eq!(vci.severity_counts.moderate, 1);
        assert_eq!(vci.valid_fraction, 1.0);

        let tci = drought_result_from_raster(&rehydrated(
            DroughtIndexKind::Tci,
            "l3:tci",
            vec![40.0, -9999.0],
            vec![true, false],
        ))
        .expect("tci rehydrates");
        assert_eq!(
            tci.reason_codes[1],
            DroughtPixelReason::NoCurrentObservation
        );

        let vhi = compute_vhi(&vci, &tci, 0.5).expect("vhi");
        assert!((vhi.values[0] - 60.0).abs() < 1.0e-4);
        assert_eq!(vhi.severity[0], DroughtSeverity::NoDrought);
        assert_eq!(vhi.reason_codes[1], DroughtPixelReason::ComponentInvalid);
        // VHI lineage inputs are the two persisted component products.
        assert_eq!(vhi.current_product_id, "l3:vci,l3:tci");
    }

    #[test]
    fn rehydration_refuses_values_outside_the_percent_domain() {
        // A valid pixel at 0.4 percent is fine; 300 (e.g. an LST raster
        // passed by mistake) is refused as not-a-drought-raster.
        assert!(drought_result_from_raster(&rehydrated(
            DroughtIndexKind::Vci,
            "l3:vci",
            vec![0.4, 100.0],
            vec![true, true],
        ))
        .is_ok());
        assert_eq!(
            drought_result_from_raster(&rehydrated(
                DroughtIndexKind::Tci,
                "l3:tci",
                vec![300.0, 40.0],
                vec![true, true],
            ))
            .expect_err("out of domain"),
            DroughtIndexError::RehydratedValueOutOfRange {
                pixel: 0,
                value: 300.0
            }
        );
    }
}
