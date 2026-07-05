//! Per-pixel phenology metrics and tier-1 rule-based land-cover
//! classification (satellite pipeline batch 10).
//!
//! **Phenology**: from a dated same-grid NDVI series (one season/year), each
//! pixel gets min/max/amplitude, peak day-of-year, season start/end (SOS/EOS
//! at a configurable fraction of the amplitude above the minimum, linearly
//! interpolated between bracketing observations), season length, and the
//! trapezoidal integral of NDVI above the minimum. The series is despiked
//! before analysis: a V-shaped single-observation dip deeper than
//! [`DESPIKE_MIN_DEPTH`] below both neighbors is replaced by the neighbor
//! mean. Clouds only bias NDVI downward, so dips are artifacts while
//! single-observation peaks are real phenology — a symmetric moving median
//! would flatten genuine peaks at monthly cadence. (Whittaker /
//! Savitzky-Golay upper-envelope smoothing is future work.)
//!
//! **Classification**: deterministic ordered rules over the phenology
//! metrics plus an optional water-index (MNDWI) summary, per the design
//! doctrine (tier 1 before any ML): water, bare/sparse, annual crop
//! (large amplitude + low trough), tree/perennial (high minimum + low
//! amplitude), grassland, else unknown. Every pixel carries the rule id
//! that fired — classifications are evidence, not opinions.

use crate::evidence::{deterministic_fingerprint, AnalysisEvidenceError};
use crate::l3_product::{to_l3_draft, L3DraftContext};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use shared::product_graph::ProductRecordDraft;
use shared::schemas::{assert_raster_spatial_ref, RasterSpatialRef, RasterSpatialRefError};
use thiserror::Error;

/// Sentinel for metric layers where the pixel is invalid.
pub const PHENOLOGY_SENTINEL: f32 = f32::NAN;

/// Default SOS/EOS threshold: minimum + this fraction of the amplitude.
pub const DEFAULT_SEASON_THRESHOLD_FRACTION: f32 = 0.5;
/// Default minimum valid observations per pixel.
pub const DEFAULT_MIN_OBSERVATIONS: u32 = 4;
/// Amplitude below this is a flat series: no season, SOS/EOS undefined.
pub const FLAT_AMPLITUDE_EPSILON: f32 = 0.05;
/// A mid-series observation this far below BOTH neighbors is treated as a
/// cloud-leak dip and replaced by the neighbor mean.
pub const DESPIKE_MIN_DEPTH: f32 = 0.1;

/// Per-pixel outcome code for phenology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhenologyPixelReason {
    Computed,
    /// Fewer valid observations than `min_observations`.
    BelowMinObservations,
    /// Amplitude under [`FLAT_AMPLITUDE_EPSILON`]: min/max/amplitude are
    /// reported but SOS/EOS/season metrics are sentinel.
    FlatSeries,
}

/// One dated NDVI raster entering the series.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhenologyObservation {
    pub product_id: String,
    pub observed_on: NaiveDate,
    pub values: Vec<f32>,
    pub valid_mask: Vec<bool>,
    /// Must equal the request grid exactly.
    pub spatial_ref: RasterSpatialRef,
}

/// A phenology computation request over one season's series.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhenologyRequest {
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    pub observations: Vec<PhenologyObservation>,
    pub min_observations: u32,
    /// SOS/EOS threshold as a fraction of amplitude above the minimum.
    pub season_threshold_fraction: f32,
}

/// Evidence object for one phenology run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhenologyEvidence {
    pub observation_product_ids: Vec<String>,
    pub observation_dates: Vec<NaiveDate>,
    pub min_observations: u32,
    pub season_threshold_fraction: f32,
    pub smoothing: String,
    pub spatial_ref: RasterSpatialRef,
    pub input_hash: String,
}

/// Completed per-pixel phenology metrics (row-major layers; sentinel where
/// the reason code is not `Computed`, and for season metrics on flat series).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhenologyResult {
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    pub ndvi_min: Vec<f32>,
    pub ndvi_max: Vec<f32>,
    pub amplitude: Vec<f32>,
    /// Day of year of the smoothed maximum.
    pub peak_doy: Vec<f32>,
    /// Season start/end day of year (threshold crossing, interpolated).
    pub sos_doy: Vec<f32>,
    pub eos_doy: Vec<f32>,
    pub season_length_days: Vec<f32>,
    /// Trapezoidal integral of (NDVI - ndvi_min) over the season, NDVI*days.
    pub season_integral: Vec<f32>,
    pub reason_codes: Vec<PhenologyPixelReason>,
    pub valid_fraction: f32,
    pub evidence: PhenologyEvidence,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PhenologyError {
    #[error("phenology request has no observations")]
    NoObservations,
    #[error("phenology needs at least 3 observations for the season shape (got {0} and min_observations must also be >= 3)")]
    TooFewObservations(usize),
    #[error("season_threshold_fraction must be in (0, 1), got {0}")]
    BadThresholdFraction(f32),
    #[error("observation {observation_index} has {actual} values, expected {expected}")]
    ObservationLengthMismatch {
        observation_index: usize,
        expected: usize,
        actual: usize,
    },
    #[error(
        "observation {observation_index} spatial ref does not match the grid (no resampling here)"
    )]
    SpatialRefMismatch { observation_index: usize },
    #[error("observations share a date ({0}); series must be strictly dated")]
    DuplicateDate(NaiveDate),
    #[error("grid spatial metadata is invalid: {reason}")]
    SpatialRef { reason: RasterSpatialRefError },
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] AnalysisEvidenceError),
}

fn validate_request(request: &PhenologyRequest) -> Result<usize, PhenologyError> {
    if request.observations.is_empty() {
        return Err(PhenologyError::NoObservations);
    }
    if request.observations.len() < 3 || request.min_observations < 3 {
        return Err(PhenologyError::TooFewObservations(
            request.observations.len(),
        ));
    }
    if !(request.season_threshold_fraction > 0.0 && request.season_threshold_fraction < 1.0) {
        return Err(PhenologyError::BadThresholdFraction(
            request.season_threshold_fraction,
        ));
    }
    assert_raster_spatial_ref(Some(&request.spatial_ref), request.width, request.height)
        .map_err(|reason| PhenologyError::SpatialRef { reason })?;
    let pixel_count = request.width as usize * request.height as usize;
    for (index, observation) in request.observations.iter().enumerate() {
        if observation.values.len() != pixel_count || observation.valid_mask.len() != pixel_count {
            return Err(PhenologyError::ObservationLengthMismatch {
                observation_index: index,
                expected: pixel_count,
                actual: observation.values.len().max(observation.valid_mask.len()),
            });
        }
        if observation.spatial_ref != request.spatial_ref {
            return Err(PhenologyError::SpatialRefMismatch {
                observation_index: index,
            });
        }
    }
    Ok(pixel_count)
}

/// Replace V-shaped downward single-observation dips (cloud leaks) with the
/// neighbor mean; ends and genuine peaks pass through untouched. Operates on
/// the original values so consecutive dips are judged independently.
fn despike_dips(series: &[(f64, f32)]) -> Vec<(f64, f32)> {
    if series.len() < 3 {
        return series.to_vec();
    }
    let mut out = series.to_vec();
    for i in 1..series.len() - 1 {
        let (prev, this, next) = (series[i - 1].1, series[i].1, series[i + 1].1);
        if this < prev - DESPIKE_MIN_DEPTH && this < next - DESPIKE_MIN_DEPTH {
            out[i].1 = 0.5 * (prev + next);
        }
    }
    out
}

/// First upward threshold crossing (fractional DOY, linear interpolation);
/// `None` if the series never reaches the threshold. A series starting at or
/// above the threshold starts its season at the first observation.
fn first_upward_crossing(series: &[(f64, f32)], threshold: f32) -> Option<f64> {
    if series[0].1 >= threshold {
        return Some(series[0].0);
    }
    for pair in series.windows(2) {
        let (day0, v0) = pair[0];
        let (day1, v1) = pair[1];
        if v0 < threshold && v1 >= threshold {
            let t = f64::from(threshold - v0) / f64::from(v1 - v0);
            return Some(day0 + t * (day1 - day0));
        }
    }
    None
}

/// Last downward threshold crossing; a series ending at or above the
/// threshold ends its season at the last observation.
fn last_downward_crossing(series: &[(f64, f32)], threshold: f32) -> Option<f64> {
    if series[series.len() - 1].1 >= threshold {
        return Some(series[series.len() - 1].0);
    }
    for pair in series.windows(2).rev() {
        let (day0, v0) = pair[0];
        let (day1, v1) = pair[1];
        if v0 >= threshold && v1 < threshold {
            let t = f64::from(v0 - threshold) / f64::from(v0 - v1);
            return Some(day0 + t * (day1 - day0));
        }
    }
    None
}

/// Trapezoidal integral of (value - base) clamped at 0, over [from, to].
fn integral_above(series: &[(f64, f32)], base: f32, from: f64, to: f64) -> f64 {
    let mut total = 0.0;
    for pair in series.windows(2) {
        let (day0, v0) = pair[0];
        let (day1, v1) = pair[1];
        let lo = day0.max(from);
        let hi = day1.min(to);
        if hi <= lo || day1 == day0 {
            continue;
        }
        let value_at = |day: f64| {
            let t = (day - day0) / (day1 - day0);
            f64::from(v0) + t * f64::from(v1 - v0)
        };
        let a = (value_at(lo) - f64::from(base)).max(0.0);
        let b = (value_at(hi) - f64::from(base)).max(0.0);
        total += 0.5 * (a + b) * (hi - lo);
    }
    total
}

/// Compute per-pixel phenology metrics over one season's NDVI series.
pub fn compute_phenology(request: &PhenologyRequest) -> Result<PhenologyResult, PhenologyError> {
    let pixel_count = validate_request(request)?;

    // Deterministic date order; duplicate dates are ambiguous.
    let mut order: Vec<usize> = (0..request.observations.len()).collect();
    order.sort_by_key(|&index| (request.observations[index].observed_on, index));
    for pair in order.windows(2) {
        let (a, b) = (
            request.observations[pair[0]].observed_on,
            request.observations[pair[1]].observed_on,
        );
        if a == b {
            return Err(PhenologyError::DuplicateDate(a));
        }
    }
    let base_year = request.observations[order[0]].observed_on.year();
    let doy_of = |date: NaiveDate| -> f64 {
        // Continuous day axis across year boundaries relative to the series'
        // first year.
        f64::from(date.ordinal()) + f64::from(date.year() - base_year) * 365.25
    };

    let mut ndvi_min = vec![PHENOLOGY_SENTINEL; pixel_count];
    let mut ndvi_max = vec![PHENOLOGY_SENTINEL; pixel_count];
    let mut amplitude = vec![PHENOLOGY_SENTINEL; pixel_count];
    let mut peak_doy = vec![PHENOLOGY_SENTINEL; pixel_count];
    let mut sos_doy = vec![PHENOLOGY_SENTINEL; pixel_count];
    let mut eos_doy = vec![PHENOLOGY_SENTINEL; pixel_count];
    let mut season_length = vec![PHENOLOGY_SENTINEL; pixel_count];
    let mut season_integral = vec![PHENOLOGY_SENTINEL; pixel_count];
    let mut reason_codes = vec![PhenologyPixelReason::Computed; pixel_count];
    let mut valid = 0u32;

    for pixel in 0..pixel_count {
        let mut series: Vec<(f64, f32)> = Vec::with_capacity(order.len());
        for &index in &order {
            let observation = &request.observations[index];
            let value = observation.values[pixel];
            if observation.valid_mask[pixel] && value.is_finite() {
                series.push((doy_of(observation.observed_on), value));
            }
        }
        if (series.len() as u32) < request.min_observations {
            reason_codes[pixel] = PhenologyPixelReason::BelowMinObservations;
            continue;
        }
        let series = despike_dips(&series);

        let min = series.iter().map(|(_, v)| *v).fold(f32::MAX, f32::min);
        let max = series.iter().map(|(_, v)| *v).fold(f32::MIN, f32::max);
        let amp = max - min;
        ndvi_min[pixel] = min;
        ndvi_max[pixel] = max;
        amplitude[pixel] = amp;
        // Peak: first date attaining the smoothed maximum.
        peak_doy[pixel] = series
            .iter()
            .find(|(_, v)| *v == max)
            .map(|(day, _)| *day as f32)
            .expect("max exists");
        valid += 1;

        if amp < FLAT_AMPLITUDE_EPSILON {
            reason_codes[pixel] = PhenologyPixelReason::FlatSeries;
            continue;
        }
        let threshold = min + request.season_threshold_fraction * amp;
        let (Some(sos), Some(eos)) = (
            first_upward_crossing(&series, threshold),
            last_downward_crossing(&series, threshold),
        ) else {
            // Amplitude >= epsilon guarantees the max is above threshold,
            // so both crossings exist; defensive fallthrough.
            reason_codes[pixel] = PhenologyPixelReason::FlatSeries;
            continue;
        };
        sos_doy[pixel] = sos as f32;
        eos_doy[pixel] = eos as f32;
        season_length[pixel] = (eos - sos) as f32;
        season_integral[pixel] = integral_above(&series, min, sos, eos) as f32;
    }

    let observation_product_ids: Vec<String> = order
        .iter()
        .map(|&index| request.observations[index].product_id.clone())
        .collect();
    let observation_dates: Vec<NaiveDate> = order
        .iter()
        .map(|&index| request.observations[index].observed_on)
        .collect();
    let input_hash = deterministic_fingerprint(&(
        "phenology_v1",
        &observation_product_ids,
        &observation_dates,
        request.min_observations,
        request.season_threshold_fraction,
        &request.spatial_ref,
    ))?;

    Ok(PhenologyResult {
        width: request.width,
        height: request.height,
        spatial_ref: request.spatial_ref.clone(),
        ndvi_min,
        ndvi_max,
        amplitude,
        peak_doy,
        sos_doy,
        eos_doy,
        season_length_days: season_length,
        season_integral,
        reason_codes,
        valid_fraction: valid as f32 / pixel_count as f32,
        evidence: PhenologyEvidence {
            observation_product_ids,
            observation_dates,
            min_observations: request.min_observations,
            season_threshold_fraction: request.season_threshold_fraction,
            smoothing: "v_dip_despike_0.1_neighbor_mean".to_string(),
            spatial_ref: request.spatial_ref.clone(),
            input_hash,
        },
    })
}

// ---------------------------------------------------------------------------
// Tier-1 rule-based land cover
// ---------------------------------------------------------------------------

/// Land-cover classes assigned by the tier-1 rules. Raster codes are stable
/// (`class_code`): water 1, bare 2, annual crop 3, tree/perennial 4,
/// grassland 5, unknown 6; invalid pixels are nodata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LandCoverClass {
    Water,
    BareOrSparse,
    AnnualCrop,
    TreeOrPerennial,
    Grassland,
    Unknown,
    Invalid,
}

impl LandCoverClass {
    pub fn class_code(self) -> Option<u8> {
        match self {
            LandCoverClass::Water => Some(1),
            LandCoverClass::BareOrSparse => Some(2),
            LandCoverClass::AnnualCrop => Some(3),
            LandCoverClass::TreeOrPerennial => Some(4),
            LandCoverClass::Grassland => Some(5),
            LandCoverClass::Unknown => Some(6),
            LandCoverClass::Invalid => None,
        }
    }
}

/// Thresholds for the ordered tier-1 rules (defaults from the design doc's
/// separability notes: crop amplitude 0.3-0.4 + trough; tree high minimum +
/// low amplitude; MNDWI positive = water).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LandCoverRuleConfig {
    /// Rule 1: water when the mean water index (MNDWI) exceeds this.
    pub water_index_threshold: f32,
    /// Rule 2: bare/sparse when NDVI max stays under this.
    pub bare_ndvi_max: f32,
    /// Rule 3: annual crop when amplitude >= this and the trough is low.
    pub crop_amplitude_min: f32,
    pub crop_trough_max: f32,
    /// Rule 4: tree/perennial when NDVI min >= this and amplitude <= this.
    pub tree_ndvi_min: f32,
    pub tree_amplitude_max: f32,
    /// Rule 5: grassland when NDVI max >= this (moderate cover, no crop
    /// pulse, no perennial floor).
    pub grass_ndvi_max_min: f32,
}

impl Default for LandCoverRuleConfig {
    fn default() -> Self {
        Self {
            water_index_threshold: 0.05,
            bare_ndvi_max: 0.25,
            crop_amplitude_min: 0.35,
            crop_trough_max: 0.40,
            tree_ndvi_min: 0.45,
            tree_amplitude_max: 0.25,
            grass_ndvi_max_min: 0.35,
        }
    }
}

/// Per-pixel classification with the rule that fired. (Serialize-only: the
/// static rule-id strings are not round-tripped; the GeoTIFF + catalog
/// parameters are the persistent form.)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LandCoverResult {
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    pub classes: Vec<LandCoverClass>,
    /// Rule id per pixel (`water_index`, `bare_ndvi`, `crop_pulse`,
    /// `perennial_floor`, `grass_cover`, `no_rule`, `invalid_phenology`).
    pub rule_ids: Vec<&'static str>,
    pub class_counts: Vec<(LandCoverClass, u32)>,
    pub valid_fraction: f32,
    pub config: LandCoverRuleConfig,
    pub input_hash: String,
}

/// Classify each pixel from phenology metrics plus an optional mean water
/// index raster (same grid; `None` skips the water rule entirely).
pub fn classify_land_cover(
    phenology: &PhenologyResult,
    water_index_mean: Option<&[f32]>,
    config: &LandCoverRuleConfig,
) -> Result<LandCoverResult, PhenologyError> {
    let pixel_count = phenology.ndvi_min.len();
    if let Some(water) = water_index_mean {
        if water.len() != pixel_count {
            return Err(PhenologyError::ObservationLengthMismatch {
                observation_index: 0,
                expected: pixel_count,
                actual: water.len(),
            });
        }
    }

    let mut classes = vec![LandCoverClass::Invalid; pixel_count];
    let mut rule_ids = vec!["invalid_phenology"; pixel_count];
    let mut valid = 0u32;
    for pixel in 0..pixel_count {
        if phenology.reason_codes[pixel] == PhenologyPixelReason::BelowMinObservations {
            continue;
        }
        let min = phenology.ndvi_min[pixel];
        let max = phenology.ndvi_max[pixel];
        let amp = phenology.amplitude[pixel];
        let water = water_index_mean
            .map(|values| values[pixel])
            .filter(|v| v.is_finite());
        valid += 1;

        let (class, rule) = if water.is_some_and(|w| w > config.water_index_threshold) {
            (LandCoverClass::Water, "water_index")
        } else if max < config.bare_ndvi_max {
            (LandCoverClass::BareOrSparse, "bare_ndvi")
        } else if amp >= config.crop_amplitude_min && min <= config.crop_trough_max {
            (LandCoverClass::AnnualCrop, "crop_pulse")
        } else if min >= config.tree_ndvi_min && amp <= config.tree_amplitude_max {
            (LandCoverClass::TreeOrPerennial, "perennial_floor")
        } else if max >= config.grass_ndvi_max_min {
            (LandCoverClass::Grassland, "grass_cover")
        } else {
            (LandCoverClass::Unknown, "no_rule")
        };
        classes[pixel] = class;
        rule_ids[pixel] = rule;
    }

    let mut class_counts: Vec<(LandCoverClass, u32)> = Vec::new();
    for class in [
        LandCoverClass::Water,
        LandCoverClass::BareOrSparse,
        LandCoverClass::AnnualCrop,
        LandCoverClass::TreeOrPerennial,
        LandCoverClass::Grassland,
        LandCoverClass::Unknown,
        LandCoverClass::Invalid,
    ] {
        let count = classes.iter().filter(|c| **c == class).count() as u32;
        if count > 0 {
            class_counts.push((class, count));
        }
    }
    let input_hash = deterministic_fingerprint(&(
        "landcover_rules_v1",
        &phenology.evidence.input_hash,
        config,
        water_index_mean.is_some(),
    ))?;

    Ok(LandCoverResult {
        width: phenology.width,
        height: phenology.height,
        spatial_ref: phenology.spatial_ref.clone(),
        classes,
        rule_ids,
        class_counts,
        valid_fraction: valid as f32 / pixel_count as f32,
        config: config.clone(),
        input_hash,
    })
}

// ---------------------------------------------------------------------------
// L3 lineage
// ---------------------------------------------------------------------------

/// Scope the L3 drafts cannot derive from pixel data alone.
#[derive(Debug, Clone)]
pub struct PhenologyL3Scope {
    pub field_id: String,
    pub season_id: String,
    pub scene_id: Option<String>,
    pub temporal_start: String,
    pub temporal_end: String,
    pub source_id: Option<String>,
}

/// Map a phenology result to an L3 draft (kind `phenology`). Lineage = every
/// series observation.
pub fn phenology_l3_draft(
    result: &PhenologyResult,
    scope: &PhenologyL3Scope,
) -> ProductRecordDraft {
    let mut input_product_ids: Vec<String> = Vec::new();
    for product_id in &result.evidence.observation_product_ids {
        if !input_product_ids.contains(product_id) {
            input_product_ids.push(product_id.clone());
        }
    }
    to_l3_draft(&L3DraftContext {
        kind: "phenology".to_string(),
        algorithm_id: "phenology.season_metrics".to_string(),
        algorithm_version: "1.0.0".to_string(),
        field_id: scope.field_id.clone(),
        season_id: scope.season_id.clone(),
        scene_id: scope.scene_id.clone(),
        temporal_start: scope.temporal_start.clone(),
        temporal_end: scope.temporal_end.clone(),
        input_product_ids,
        parameters: serde_json::json!({
            "metrics": ["ndvi_min", "ndvi_max", "amplitude", "peak_doy",
                        "sos_doy", "eos_doy", "season_length_days", "season_integral"],
            "season_threshold_fraction": result.evidence.season_threshold_fraction,
            "min_observations": result.evidence.min_observations,
            "smoothing": result.evidence.smoothing,
            "observation_dates": result.evidence.observation_dates,
        }),
        confidence: Some(f64::from(result.valid_fraction)),
        confidence_method: Some("valid_coverage_fraction".to_string()),
        evidence_digests: vec![result.evidence.input_hash.clone()],
        source_id: scope.source_id.clone(),
    })
}

/// Map a land-cover result to an L3 draft (kind `landcover_rule`). Lineage
/// is attached by the caller (phenology product + water-index products).
pub fn landcover_l3_draft(
    result: &LandCoverResult,
    input_product_ids: Vec<String>,
    scope: &PhenologyL3Scope,
) -> ProductRecordDraft {
    to_l3_draft(&L3DraftContext {
        kind: "landcover_rule".to_string(),
        algorithm_id: "landcover.tier1_rules".to_string(),
        algorithm_version: "1.0.0".to_string(),
        field_id: scope.field_id.clone(),
        season_id: scope.season_id.clone(),
        scene_id: scope.scene_id.clone(),
        temporal_start: scope.temporal_start.clone(),
        temporal_end: scope.temporal_end.clone(),
        input_product_ids,
        parameters: serde_json::json!({
            "rules": ["water_index", "bare_ndvi", "crop_pulse",
                      "perennial_floor", "grass_cover", "no_rule"],
            "config": result.config,
            "class_codes": {
                "water": 1, "bare_or_sparse": 2, "annual_crop": 3,
                "tree_or_perennial": 4, "grassland": 5, "unknown": 6,
            },
            "class_counts": result.class_counts.iter().map(|(class, count)| {
                serde_json::json!({ "class": class, "count": count })
            }).collect::<Vec<_>>(),
        }),
        confidence: Some(f64::from(result.valid_fraction)),
        confidence_method: Some("valid_coverage_fraction".to_string()),
        evidence_digests: vec![result.input_hash.clone()],
        source_id: scope.source_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::{GeoBounds, RasterResolution};

    fn spatial_ref() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32643".to_string()),
            bbox: Some(GeoBounds {
                min_lon: 600000.0,
                min_lat: 1300010.0,
                max_lon: 600010.0,
                max_lat: 1300020.0,
            }),
            geo_transform: Some([600000.0, 10.0, 0.0, 1300020.0, 0.0, -10.0]),
            resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
        }
    }

    fn date(m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, d).unwrap()
    }

    /// One-pixel series helper.
    fn request_1px(series: &[(NaiveDate, f32)]) -> PhenologyRequest {
        PhenologyRequest {
            width: 1,
            height: 1,
            spatial_ref: spatial_ref(),
            observations: series
                .iter()
                .enumerate()
                .map(|(i, (day, value))| PhenologyObservation {
                    product_id: format!("p{i}"),
                    observed_on: *day,
                    values: vec![*value],
                    valid_mask: vec![true],
                    spatial_ref: spatial_ref(),
                })
                .collect(),
            min_observations: 4,
            season_threshold_fraction: 0.5,
        }
    }

    #[test]
    fn crop_season_metrics_are_hand_computed() {
        // DOY 32 (Feb 1) 0.2, DOY 91 (Apr 1) 0.5, DOY 152 (Jun 1) 0.8,
        // DOY 213 (Aug 1) 0.5, DOY 274 (Oct 1) 0.2. No V-dips, so
        // despiking is a no-op — the single-observation peak survives.
        // min 0.2, max 0.8, amplitude 0.6, threshold 0.5.
        // SOS: crossing between 32 and 91 at exactly 0.5 -> DOY 91 (v hits
        // threshold at the endpoint: t=(0.5-0.2)/0.3=1 -> 91).
        // EOS: last downward crossing between 152 and 213: v0 0.8 >= 0.5,
        // v1 0.5 >= threshold? 0.5 >= 0.5 -> the tail (274, 0.2) is below,
        // so crossing is between 213 (0.5) and 274 (0.2): t=(0.5-0.5)/0.3=0
        // -> DOY 213.
        let request = request_1px(&[
            (date(2, 1), 0.2),
            (date(4, 1), 0.5),
            (date(6, 1), 0.8),
            (date(8, 1), 0.5),
            (date(10, 1), 0.2),
        ]);
        let result = compute_phenology(&request).unwrap();
        assert_eq!(result.reason_codes[0], PhenologyPixelReason::Computed);
        assert!((result.ndvi_min[0] - 0.2).abs() < 1e-6);
        assert!((result.ndvi_max[0] - 0.8).abs() < 1e-6);
        assert!((result.amplitude[0] - 0.6).abs() < 1e-6);
        assert!((result.peak_doy[0] - 152.0).abs() < 1e-3);
        assert!(
            (result.sos_doy[0] - 91.0).abs() < 1e-3,
            "{}",
            result.sos_doy[0]
        );
        assert!(
            (result.eos_doy[0] - 213.0).abs() < 1e-3,
            "{}",
            result.eos_doy[0]
        );
        assert!((result.season_length_days[0] - 122.0).abs() < 1e-3);
        // Integral of (NDVI-0.2) from 91 to 213: trapezoids
        // [91,152]: (0.3+0.6)/2*61 = 27.45; [152,213]: (0.6+0.3)/2*61 = 27.45.
        assert!(
            (result.season_integral[0] - 54.9).abs() < 1e-2,
            "{}",
            result.season_integral[0]
        );
    }

    #[test]
    fn interpolated_sos_lands_between_observations() {
        // 0.2 at DOY 32, 0.6 at DOY 92 (60 days): threshold 0.5 of amp 0.4
        // over min 0.2 = 0.4 -> crossing at t=(0.4-0.2)/0.4=0.5 -> DOY 62.
        let request = request_1px(&[
            (date(2, 1), 0.2),
            (date(4, 2), 0.6),
            (date(6, 1), 0.6),
            (date(8, 1), 0.2),
        ]);
        let result = compute_phenology(&request).unwrap();
        assert!(
            (result.sos_doy[0] - 62.0).abs() < 1e-3,
            "{}",
            result.sos_doy[0]
        );
    }

    #[test]
    fn despike_suppresses_a_single_cloud_dip_but_keeps_peaks() {
        // A single low outlier (cloud leak) mid-season must not create a
        // trough: the V-dip is replaced by the neighbor mean.
        let request = request_1px(&[
            (date(4, 1), 0.7),
            (date(5, 1), 0.7),
            (date(6, 1), 0.1), // spike down
            (date(7, 1), 0.7),
            (date(8, 1), 0.7),
        ]);
        let result = compute_phenology(&request).unwrap();
        assert!(
            (result.ndvi_min[0] - 0.7).abs() < 1e-6,
            "{}",
            result.ndvi_min[0]
        );
        assert_eq!(result.reason_codes[0], PhenologyPixelReason::FlatSeries);
    }

    #[test]
    fn flat_and_sparse_series_are_reason_coded() {
        // Flat perennial: metrics present, season metrics sentinel.
        let request = request_1px(&[
            (date(2, 1), 0.62),
            (date(4, 1), 0.60),
            (date(6, 1), 0.63),
            (date(8, 1), 0.61),
        ]);
        let result = compute_phenology(&request).unwrap();
        assert_eq!(result.reason_codes[0], PhenologyPixelReason::FlatSeries);
        assert!((result.ndvi_min[0] - 0.60).abs() < 1e-6);
        assert!(result.sos_doy[0].is_nan());

        // Sparse: 3 valid of 5 with min_observations 4.
        let mut request = request_1px(&[
            (date(2, 1), 0.2),
            (date(4, 1), 0.5),
            (date(6, 1), 0.8),
            (date(8, 1), 0.5),
            (date(10, 1), 0.2),
        ]);
        request.observations[1].valid_mask[0] = false;
        request.observations[3].valid_mask[0] = false;
        let result = compute_phenology(&request).unwrap();
        assert_eq!(
            result.reason_codes[0],
            PhenologyPixelReason::BelowMinObservations
        );
        assert!(result.ndvi_min[0].is_nan());
        assert_eq!(result.valid_fraction, 0.0);
    }

    #[test]
    fn duplicate_dates_are_rejected() {
        let request = request_1px(&[
            (date(2, 1), 0.2),
            (date(2, 1), 0.3),
            (date(6, 1), 0.8),
            (date(8, 1), 0.2),
        ]);
        let result = compute_phenology(&request);
        assert!(
            matches!(result, Err(PhenologyError::DuplicateDate(_))),
            "{result:?}"
        );
    }

    fn phenology_for(series: &[(NaiveDate, f32)]) -> PhenologyResult {
        compute_phenology(&request_1px(series)).unwrap()
    }

    #[test]
    fn tier1_rules_classify_the_design_archetypes() {
        let config = LandCoverRuleConfig::default();
        // Annual crop: pulse 0.2 -> 0.8 -> 0.2.
        let crop = phenology_for(&[
            (date(2, 1), 0.2),
            (date(4, 1), 0.5),
            (date(6, 1), 0.8),
            (date(8, 1), 0.5),
            (date(10, 1), 0.2),
        ]);
        let result = classify_land_cover(&crop, None, &config).unwrap();
        assert_eq!(result.classes[0], LandCoverClass::AnnualCrop);
        assert_eq!(result.rule_ids[0], "crop_pulse");

        // Tree/perennial: high floor, low amplitude.
        let tree = phenology_for(&[
            (date(2, 1), 0.62),
            (date(4, 1), 0.60),
            (date(6, 1), 0.72),
            (date(8, 1), 0.65),
        ]);
        let result = classify_land_cover(&tree, None, &config).unwrap();
        assert_eq!(result.classes[0], LandCoverClass::TreeOrPerennial);
        assert_eq!(result.rule_ids[0], "perennial_floor");

        // Bare: never greens up.
        let bare = phenology_for(&[
            (date(2, 1), 0.10),
            (date(4, 1), 0.15),
            (date(6, 1), 0.18),
            (date(8, 1), 0.12),
        ]);
        let result = classify_land_cover(&bare, None, &config).unwrap();
        assert_eq!(result.classes[0], LandCoverClass::BareOrSparse);

        // Water overrides everything when the water index says so.
        let result = classify_land_cover(&bare, Some(&[0.4]), &config).unwrap();
        assert_eq!(result.classes[0], LandCoverClass::Water);
        assert_eq!(result.rule_ids[0], "water_index");

        // Grassland: moderate cover, no crop pulse, no perennial floor.
        let grass = phenology_for(&[
            (date(2, 1), 0.30),
            (date(4, 1), 0.40),
            (date(6, 1), 0.50),
            (date(8, 1), 0.35),
        ]);
        let result = classify_land_cover(&grass, None, &config).unwrap();
        assert_eq!(result.classes[0], LandCoverClass::Grassland);
        assert_eq!(result.rule_ids[0], "grass_cover");
    }

    #[test]
    fn invalid_phenology_pixels_stay_invalid_and_counts_add_up() {
        let mut request = request_1px(&[
            (date(2, 1), 0.2),
            (date(4, 1), 0.5),
            (date(6, 1), 0.8),
            (date(8, 1), 0.2),
        ]);
        for observation in &mut request.observations {
            observation.valid_mask[0] = false;
        }
        let phenology = compute_phenology(&request).unwrap();
        let result =
            classify_land_cover(&phenology, None, &LandCoverRuleConfig::default()).unwrap();
        assert_eq!(result.classes[0], LandCoverClass::Invalid);
        assert_eq!(result.rule_ids[0], "invalid_phenology");
        assert_eq!(result.class_counts, vec![(LandCoverClass::Invalid, 1)]);
        assert_eq!(result.valid_fraction, 0.0);
    }

    #[test]
    fn drafts_carry_kind_lineage_and_identity() {
        let phenology = phenology_for(&[
            (date(2, 1), 0.2),
            (date(4, 1), 0.5),
            (date(6, 1), 0.8),
            (date(8, 1), 0.2),
        ]);
        let scope = PhenologyL3Scope {
            field_id: "field-1".to_string(),
            season_id: "season-2026".to_string(),
            scene_id: None,
            temporal_start: "2026-02-01T00:00:00Z".to_string(),
            temporal_end: "2026-10-01T23:59:59Z".to_string(),
            source_id: None,
        };
        let draft = phenology_l3_draft(&phenology, &scope);
        assert_eq!(draft.kind, "phenology");
        assert_eq!(draft.inputs.len(), 4);

        let classification =
            classify_land_cover(&phenology, None, &LandCoverRuleConfig::default()).unwrap();
        let draft = landcover_l3_draft(
            &classification,
            vec!["phenology-product".to_string()],
            &scope,
        );
        assert_eq!(draft.kind, "landcover_rule");
        assert_eq!(draft.inputs[0].product_id, "phenology-product");
    }
}
