//! Per-pixel index climatology (satellite intelligence pipeline, Phase 3
//! item 8).
//!
//! Aggregates a multi-year series of dated index rasters (e.g. monthly NDVI
//! composites, the [`crate::temporal_composite`] output representation) into
//! per-calendar-period, per-pixel **min / max / mean / observation-year
//! count** baselines — the input the VCI/TCI/VHI drought indices in
//! [`crate::drought_indices`] normalize against.
//!
//! Conventions (mirroring `temporal_composite`):
//! - All inputs must match the request grid exactly (typed errors, no
//!   resampling here).
//! - A pixel sample is usable only when the validity mask says valid *and*
//!   the value is finite.
//! - Pixels whose distinct-contributing-year count is below the configured
//!   minimum (default [`DEFAULT_MIN_CLIMATOLOGY_YEARS`]) are flagged
//!   [`ClimatologyPixelReason::BelowMinYears`] — never silently computed.
//! - Every climatology carries a [`ClimatologyEvidence`] object with a
//!   deterministic canonical-JSON FNV fingerprint over the full input.
//!
//! Persistence: the climatology is one self-describing serde-JSON artifact
//! ([`write_climatology_json`] / [`read_climatology_json`]). Rationale:
//! post_processor has no binary raster artifact convention today (all of its
//! analysis outputs are serde types; exports are CSV/JSON built in memory), a
//! climatology is one logical object (12+ small stat layers plus provenance)
//! that GeoTIFF would fragment into dozens of files plus a manifest, and
//! finite `f32` values round-trip serde JSON exactly (f32 -> f64 is exact and
//! serde_json's shortest-float formatting round-trips f64). Non-finite values
//! are excluded by construction: stat layers hold the `0.0` sentinel wherever
//! the reason code is not [`ClimatologyPixelReason::Ok`] — reason codes are
//! authoritative, never the sentinel. `raster_io::write_geotiff_f32` remains
//! available for exporting individual stat layers when geo_hub needs tiles.

use crate::evidence::{deterministic_fingerprint, AnalysisEvidenceError};
use crate::l3_product::{to_l3_draft, L3DraftContext};
use crate::temporal_composite::CompositeCadence;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use shared::product_graph::ProductRecordDraft;
use shared::schemas::{assert_raster_spatial_ref, RasterSpatialRef, RasterSpatialRefError};
use std::path::Path;
use thiserror::Error;

/// Default minimum distinct observation years per pixel/period before the
/// baseline is trusted (design doc: ">=5 yr archive, 10+ preferred").
pub const DEFAULT_MIN_CLIMATOLOGY_YEARS: u32 = 5;

/// Sentinel stored in min/max/mean layers wherever the pixel reason code is
/// not [`ClimatologyPixelReason::Ok`]. Keeps the JSON artifact NaN-free;
/// consumers must consult `reason_codes`, never trust the sentinel.
pub const CLIMATOLOGY_SENTINEL: f32 = 0.0;

/// A calendar period key, independent of year: a month (monthly cadence) or
/// a (month, dekad) pair (dekadal cadence; dekads are day 1-10, 11-20,
/// 21-end-of-month, numbered 1..=3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CalendarPeriod {
    /// Calendar month, 1..=12.
    pub month: u32,
    /// Dekad within the month (1..=3) for dekadal cadence; `None` for monthly.
    pub dekad: Option<u8>,
}

impl CalendarPeriod {
    /// Stable label used in evidence and parameters, e.g. `"m06"` / `"m06d2"`.
    pub fn label(&self) -> String {
        match self.dekad {
            Some(dekad) => format!("m{:02}d{dekad}", self.month),
            None => format!("m{:02}", self.month),
        }
    }
}

/// The calendar period containing `date` under the given cadence.
pub fn calendar_period_for(date: NaiveDate, cadence: CompositeCadence) -> CalendarPeriod {
    CalendarPeriod {
        month: date.month(),
        dekad: match cadence {
            CompositeCadence::Monthly => None,
            CompositeCadence::Dekadal => Some(match date.day() {
                1..=10 => 1,
                11..=20 => 2,
                _ => 3,
            }),
        },
    }
}

/// One dated single-index raster entering the climatology — the same
/// representation as composite outputs: f32 values + validity + grid + date.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimatologyObservation {
    /// Catalog product id of the input (e.g. an L3 composite) — recorded as
    /// per-period provenance and as climatology L3 lineage.
    pub product_id: String,
    /// Date anchoring the observation's calendar period (e.g. the composite
    /// period start).
    pub observed_on: NaiveDate,
    /// Index values, row-major, `width * height`.
    pub values: Vec<f32>,
    /// `true` = usable pixel.
    pub valid_mask: Vec<bool>,
    /// Grid georeference; must equal the request grid exactly (no resampling).
    pub spatial_ref: RasterSpatialRef,
}

/// A climatology build request over a multi-year observation series.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimatologyRequest {
    /// Index kind label, e.g. `"ndvi"` or `"lst"`.
    pub index_kind: String,
    pub cadence: CompositeCadence,
    pub width: u32,
    pub height: u32,
    /// Grid georeference all observations must match.
    pub spatial_ref: RasterSpatialRef,
    /// Minimum distinct contributing years per pixel/period; below this the
    /// pixel is flagged [`ClimatologyPixelReason::BelowMinYears`].
    pub min_years: u32,
    pub observations: Vec<ClimatologyObservation>,
}

/// Per-pixel outcome code for one climatology period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClimatologyPixelReason {
    /// Baseline computed from at least `min_years` distinct years.
    Ok,
    /// Some valid samples, but fewer distinct years than `min_years` —
    /// stats hold [`CLIMATOLOGY_SENTINEL`], not a weak baseline.
    BelowMinYears,
    /// No valid sample at all in this period.
    NoValidObservation,
}

/// Per-pixel statistics for one calendar period.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClimatologyPeriodStats {
    pub period: CalendarPeriod,
    /// Per-pixel minimum ([`CLIMATOLOGY_SENTINEL`] where reason != Ok).
    pub min: Vec<f32>,
    /// Per-pixel maximum ([`CLIMATOLOGY_SENTINEL`] where reason != Ok).
    pub max: Vec<f32>,
    /// Per-pixel mean over all valid samples (f64 accumulation, cast to f32;
    /// [`CLIMATOLOGY_SENTINEL`] where reason != Ok).
    pub mean: Vec<f32>,
    /// Per-pixel count of distinct contributing years.
    pub year_count: Vec<u32>,
    pub reason_codes: Vec<ClimatologyPixelReason>,
}

/// Per-period provenance recorded in evidence: which years and which catalog
/// products fed the period.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClimatologyPeriodProvenance {
    pub period: CalendarPeriod,
    /// Distinct observation years, sorted ascending.
    pub years: Vec<i32>,
    /// Input product ids in (date, original index) order.
    pub product_ids: Vec<String>,
}

/// Evidence object for one climatology build (doctrine: every derived layer
/// records its inputs and rules; content-hashed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClimatologyEvidence {
    pub index_kind: String,
    pub cadence: CompositeCadence,
    pub min_years: u32,
    pub spatial_ref: RasterSpatialRef,
    pub periods: Vec<ClimatologyPeriodProvenance>,
    /// Deterministic canonical-JSON FNV fingerprint over the full request
    /// (pixels included).
    pub input_hash: String,
}

/// A completed per-pixel index climatology: one stats block per calendar
/// period that had at least one observation, sorted by period.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexClimatology {
    pub index_kind: String,
    pub cadence: CompositeCadence,
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    pub min_years: u32,
    pub periods: Vec<ClimatologyPeriodStats>,
    pub evidence: ClimatologyEvidence,
}

impl IndexClimatology {
    /// Stats for one calendar period, if any observation covered it.
    pub fn period_stats(&self, period: CalendarPeriod) -> Option<&ClimatologyPeriodStats> {
        self.periods.iter().find(|stats| stats.period == period)
    }
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ClimatologyError {
    #[error("climatology request has no observations")]
    NoObservations,
    #[error("climatology min_years must be at least 1 (got 0)")]
    ZeroMinYears,
    #[error("observation {observation_index} has {actual} values, expected {expected}")]
    ValueLengthMismatch {
        observation_index: usize,
        expected: usize,
        actual: usize,
    },
    #[error(
        "observation {observation_index} validity mask has {actual} pixels, expected {expected}"
    )]
    MaskLengthMismatch {
        observation_index: usize,
        expected: usize,
        actual: usize,
    },
    #[error(
        "observation {observation_index} spatial ref does not match the climatology grid (no resampling here)"
    )]
    SpatialRefMismatch { observation_index: usize },
    #[error("climatology grid spatial metadata is invalid: {reason}")]
    SpatialRef { reason: RasterSpatialRefError },
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] AnalysisEvidenceError),
}

/// Build a per-pixel min/max/mean/year-count climatology per calendar period.
pub fn build_index_climatology(
    request: &ClimatologyRequest,
) -> Result<IndexClimatology, ClimatologyError> {
    validate_request(request)?;

    let pixel_count = request.width as usize * request.height as usize;

    // Group observation indices by calendar period; within each period sort
    // by (date, original index) — fully deterministic, and dates ascending
    // lets distinct-year counting track only the previous year seen.
    let mut windows: std::collections::BTreeMap<CalendarPeriod, Vec<usize>> =
        std::collections::BTreeMap::new();
    for (index, observation) in request.observations.iter().enumerate() {
        windows
            .entry(calendar_period_for(
                observation.observed_on,
                request.cadence,
            ))
            .or_default()
            .push(index);
    }

    let mut periods = Vec::with_capacity(windows.len());
    let mut provenance = Vec::with_capacity(windows.len());
    for (period, mut indices) in windows {
        indices.sort_by_key(|&index| (request.observations[index].observed_on, index));

        let mut min = vec![CLIMATOLOGY_SENTINEL; pixel_count];
        let mut max = vec![CLIMATOLOGY_SENTINEL; pixel_count];
        let mut mean = vec![CLIMATOLOGY_SENTINEL; pixel_count];
        let mut year_count = vec![0u32; pixel_count];
        let mut reason_codes = vec![ClimatologyPixelReason::NoValidObservation; pixel_count];

        for pixel in 0..pixel_count {
            let mut pixel_min = f32::INFINITY;
            let mut pixel_max = f32::NEG_INFINITY;
            let mut sum = 0.0f64;
            let mut samples = 0u32;
            let mut years = 0u32;
            let mut previous_year: Option<i32> = None;
            for &index in &indices {
                let observation = &request.observations[index];
                let value = observation.values[pixel];
                if !observation.valid_mask[pixel] || !value.is_finite() {
                    continue;
                }
                pixel_min = pixel_min.min(value);
                pixel_max = pixel_max.max(value);
                sum += value as f64;
                samples += 1;
                let year = observation.observed_on.year();
                if previous_year != Some(year) {
                    years += 1;
                    previous_year = Some(year);
                }
            }
            year_count[pixel] = years;
            if samples == 0 {
                continue;
            }
            if years < request.min_years {
                reason_codes[pixel] = ClimatologyPixelReason::BelowMinYears;
                continue;
            }
            reason_codes[pixel] = ClimatologyPixelReason::Ok;
            min[pixel] = pixel_min;
            max[pixel] = pixel_max;
            mean[pixel] = (sum / samples as f64) as f32;
        }

        let mut years: Vec<i32> = indices
            .iter()
            .map(|&index| request.observations[index].observed_on.year())
            .collect();
        years.sort_unstable();
        years.dedup();
        provenance.push(ClimatologyPeriodProvenance {
            period,
            years,
            product_ids: indices
                .iter()
                .map(|&index| request.observations[index].product_id.clone())
                .collect(),
        });
        periods.push(ClimatologyPeriodStats {
            period,
            min,
            max,
            mean,
            year_count,
            reason_codes,
        });
    }

    let input_hash = deterministic_fingerprint(&(
        "index_climatology_v1",
        &request.index_kind,
        request.cadence,
        request.min_years,
        request.width,
        request.height,
        &request.spatial_ref,
        &request.observations,
    ))?;
    Ok(IndexClimatology {
        index_kind: request.index_kind.clone(),
        cadence: request.cadence,
        width: request.width,
        height: request.height,
        spatial_ref: request.spatial_ref.clone(),
        min_years: request.min_years,
        periods,
        evidence: ClimatologyEvidence {
            index_kind: request.index_kind.clone(),
            cadence: request.cadence,
            min_years: request.min_years,
            spatial_ref: request.spatial_ref.clone(),
            periods: provenance,
            input_hash,
        },
    })
}

fn validate_request(request: &ClimatologyRequest) -> Result<(), ClimatologyError> {
    if request.observations.is_empty() {
        return Err(ClimatologyError::NoObservations);
    }
    if request.min_years == 0 {
        return Err(ClimatologyError::ZeroMinYears);
    }
    assert_raster_spatial_ref(Some(&request.spatial_ref), request.width, request.height)
        .map_err(|reason| ClimatologyError::SpatialRef { reason })?;

    let pixel_count = request.width as usize * request.height as usize;
    for (observation_index, observation) in request.observations.iter().enumerate() {
        if observation.values.len() != pixel_count {
            return Err(ClimatologyError::ValueLengthMismatch {
                observation_index,
                expected: pixel_count,
                actual: observation.values.len(),
            });
        }
        if observation.valid_mask.len() != pixel_count {
            return Err(ClimatologyError::MaskLengthMismatch {
                observation_index,
                expected: pixel_count,
                actual: observation.valid_mask.len(),
            });
        }
        if observation.spatial_ref != request.spatial_ref {
            return Err(ClimatologyError::SpatialRefMismatch { observation_index });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Persistence (serde JSON artifact)
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ClimatologyPersistError {
    #[error("climatology artifact io failed at {path}: {message}")]
    Io { path: String, message: String },
    #[error("climatology artifact (de)serialization failed: {message}")]
    Serde { message: String },
    #[error("climatology artifact is corrupt: {message}")]
    CorruptArtifact { message: String },
}

/// Persist a climatology as one self-describing JSON artifact (metadata,
/// stats layers, and evidence; see module docs for the format rationale).
pub fn write_climatology_json(
    climatology: &IndexClimatology,
    path: &Path,
) -> Result<(), ClimatologyPersistError> {
    let json = serde_json::to_vec(climatology).map_err(|error| ClimatologyPersistError::Serde {
        message: error.to_string(),
    })?;
    std::fs::write(path, json).map_err(|error| ClimatologyPersistError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

/// Reload a climatology artifact and verify its layer shapes against the
/// declared grid.
pub fn read_climatology_json(path: &Path) -> Result<IndexClimatology, ClimatologyPersistError> {
    let bytes = std::fs::read(path).map_err(|error| ClimatologyPersistError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    let climatology: IndexClimatology =
        serde_json::from_slice(&bytes).map_err(|error| ClimatologyPersistError::Serde {
            message: error.to_string(),
        })?;
    let pixel_count = climatology.width as usize * climatology.height as usize;
    for stats in &climatology.periods {
        for (layer, length) in [
            ("min", stats.min.len()),
            ("max", stats.max.len()),
            ("mean", stats.mean.len()),
            ("year_count", stats.year_count.len()),
            ("reason_codes", stats.reason_codes.len()),
        ] {
            if length != pixel_count {
                return Err(ClimatologyPersistError::CorruptArtifact {
                    message: format!(
                        "period {} layer {layer} has {length} pixels, expected {pixel_count}",
                        stats.period.label()
                    ),
                });
            }
        }
    }
    Ok(climatology)
}

// ---------------------------------------------------------------------------
// L3 lineage
// ---------------------------------------------------------------------------

/// Scope a climatology L3 draft cannot derive from pixel data alone.
#[derive(Debug, Clone)]
pub struct ClimatologyL3Scope {
    pub field_id: String,
    pub season_id: String,
    /// Climatologies span scenes; `None` leaves the product field-scoped.
    pub scene_id: Option<String>,
    pub source_id: Option<String>,
}

/// Map a climatology to an L3 catalog draft. Every contributing product id
/// (across all periods, deduplicated in first-seen order) becomes a lineage
/// edge — same identity invariant as [`crate::l3_product`]. Confidence is
/// the fraction of pixel-periods with a trusted (`Ok`) baseline.
pub fn climatology_l3_draft(
    climatology: &IndexClimatology,
    scope: &ClimatologyL3Scope,
) -> ProductRecordDraft {
    let mut input_product_ids: Vec<String> = Vec::new();
    for provenance in &climatology.evidence.periods {
        for product_id in &provenance.product_ids {
            if !input_product_ids.contains(product_id) {
                input_product_ids.push(product_id.clone());
            }
        }
    }
    let years: Vec<i32> = climatology
        .evidence
        .periods
        .iter()
        .flat_map(|provenance| provenance.years.iter().copied())
        .collect();
    let first_year = years.iter().min().copied().unwrap_or(0);
    let last_year = years.iter().max().copied().unwrap_or(0);
    let total_pixels: usize = climatology
        .periods
        .iter()
        .map(|stats| stats.reason_codes.len())
        .sum();
    let ok_pixels: usize = climatology
        .periods
        .iter()
        .flat_map(|stats| stats.reason_codes.iter())
        .filter(|reason| **reason == ClimatologyPixelReason::Ok)
        .count();
    let confidence = if total_pixels == 0 {
        0.0
    } else {
        ok_pixels as f64 / total_pixels as f64
    };

    to_l3_draft(&L3DraftContext {
        kind: "index_climatology".to_string(),
        algorithm_id: format!("climatology.{}", climatology.index_kind),
        algorithm_version: "1.0.0".to_string(),
        field_id: scope.field_id.clone(),
        season_id: scope.season_id.clone(),
        scene_id: scope.scene_id.clone(),
        temporal_start: format!("{first_year}-01-01T00:00:00Z"),
        temporal_end: format!("{last_year}-12-31T23:59:59Z"),
        input_product_ids,
        parameters: serde_json::json!({
            "index_kind": climatology.index_kind,
            "cadence": climatology.cadence,
            "min_years": climatology.min_years,
            "statistics": ["min", "max", "mean", "year_count"],
        }),
        confidence: Some(confidence),
        confidence_method: Some("baseline_ok_pixel_fraction".to_string()),
        evidence_digests: vec![climatology.evidence.input_hash.clone()],
        source_id: scope.source_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn observation(
        id: &str,
        observed_on: NaiveDate,
        values: Vec<f32>,
        valid_mask: Vec<bool>,
    ) -> ClimatologyObservation {
        ClimatologyObservation {
            product_id: format!("product:{id}"),
            observed_on,
            values,
            valid_mask,
            spatial_ref: spatial_ref_2x1(),
        }
    }

    /// Three years of June NDVI on a 2x1 grid; pixel 1 is masked out in 2021.
    /// Plus one July observation forming a second calendar period.
    fn three_year_request(min_years: u32) -> ClimatologyRequest {
        ClimatologyRequest {
            index_kind: "ndvi".to_string(),
            cadence: CompositeCadence::Monthly,
            width: 2,
            height: 1,
            spatial_ref: spatial_ref_2x1(),
            min_years,
            observations: vec![
                observation(
                    "jun-2020",
                    date(2020, 6, 1),
                    vec![0.2, 0.5],
                    vec![true, true],
                ),
                observation(
                    "jun-2021",
                    date(2021, 6, 1),
                    vec![0.4, 0.9],
                    vec![true, false],
                ),
                observation(
                    "jun-2022",
                    date(2022, 6, 1),
                    vec![0.6, 0.7],
                    vec![true, true],
                ),
                observation(
                    "jul-2020",
                    date(2020, 7, 1),
                    vec![0.3, 0.3],
                    vec![true, true],
                ),
            ],
        }
    }

    #[test]
    fn min_max_mean_and_year_counts_match_hand_computation() {
        let climatology =
            build_index_climatology(&three_year_request(3)).expect("climatology builds");

        assert_eq!(climatology.periods.len(), 2, "June and July periods");
        let june = climatology
            .period_stats(CalendarPeriod {
                month: 6,
                dekad: None,
            })
            .expect("June stats");
        // Pixel 0: 0.2 / 0.4 / 0.6 across 2020-2022.
        assert_eq!(june.min[0], 0.2);
        assert_eq!(june.max[0], 0.6);
        assert!((june.mean[0] - 0.4).abs() < 1.0e-6);
        assert_eq!(june.year_count[0], 3);
        assert_eq!(june.reason_codes[0], ClimatologyPixelReason::Ok);
        // Pixel 1: 2021 masked -> only 2 years, below min_years = 3.
        assert_eq!(june.year_count[1], 2);
        assert_eq!(june.reason_codes[1], ClimatologyPixelReason::BelowMinYears);
        assert_eq!(june.min[1], CLIMATOLOGY_SENTINEL);
        assert_eq!(june.max[1], CLIMATOLOGY_SENTINEL);
        assert_eq!(june.mean[1], CLIMATOLOGY_SENTINEL);
    }

    #[test]
    fn single_year_period_is_below_baseline_not_silently_computed() {
        let climatology =
            build_index_climatology(&three_year_request(3)).expect("climatology builds");
        let july = climatology
            .period_stats(CalendarPeriod {
                month: 7,
                dekad: None,
            })
            .expect("July stats");
        assert!(july
            .reason_codes
            .iter()
            .all(|reason| *reason == ClimatologyPixelReason::BelowMinYears));
        assert!(july.min.iter().all(|value| *value == CLIMATOLOGY_SENTINEL));
        assert_eq!(july.year_count, vec![1, 1]);
    }

    #[test]
    fn all_masked_pixel_gets_no_valid_observation_reason() {
        let mut request = three_year_request(1);
        for observation in &mut request.observations {
            observation.valid_mask[0] = false;
        }
        let climatology = build_index_climatology(&request).expect("climatology builds");
        let june = &climatology.periods[0];
        assert_eq!(
            june.reason_codes[0],
            ClimatologyPixelReason::NoValidObservation
        );
        assert_eq!(june.year_count[0], 0);
        // Non-finite values are treated like masked pixels.
        let mut request = three_year_request(1);
        request.observations[0].values[0] = f32::NAN;
        let climatology = build_index_climatology(&request).expect("climatology builds");
        assert_eq!(climatology.periods[0].year_count[0], 2);
    }

    #[test]
    fn grid_mismatches_are_typed_errors() {
        let mut request = three_year_request(3);
        request.observations[1].values.pop();
        assert_eq!(
            build_index_climatology(&request).expect_err("short values rejected"),
            ClimatologyError::ValueLengthMismatch {
                observation_index: 1,
                expected: 2,
                actual: 1,
            }
        );

        let mut request = three_year_request(3);
        request.observations[2].valid_mask.push(true);
        assert_eq!(
            build_index_climatology(&request).expect_err("long mask rejected"),
            ClimatologyError::MaskLengthMismatch {
                observation_index: 2,
                expected: 2,
                actual: 3,
            }
        );

        let mut request = three_year_request(3);
        request.observations[0].spatial_ref.crs = Some("EPSG:32615".to_string());
        assert_eq!(
            build_index_climatology(&request).expect_err("foreign CRS rejected"),
            ClimatologyError::SpatialRefMismatch {
                observation_index: 0
            }
        );

        let mut request = three_year_request(3);
        request.min_years = 0;
        assert_eq!(
            build_index_climatology(&request).expect_err("zero min_years rejected"),
            ClimatologyError::ZeroMinYears
        );
    }

    #[test]
    fn fingerprint_is_stable_across_identical_runs_and_input_sensitive() {
        let first = build_index_climatology(&three_year_request(3)).expect("first run");
        let second = build_index_climatology(&three_year_request(3)).expect("second run");
        assert_eq!(first.evidence, second.evidence);
        assert_eq!(first.evidence.input_hash, second.evidence.input_hash);

        let mut other = three_year_request(3);
        other.observations[0].values[0] = 0.21;
        let third = build_index_climatology(&other).expect("third run");
        assert_ne!(first.evidence.input_hash, third.evidence.input_hash);
    }

    #[test]
    fn evidence_records_per_period_years_and_product_ids() {
        let climatology =
            build_index_climatology(&three_year_request(3)).expect("climatology builds");
        assert_eq!(climatology.evidence.min_years, 3);
        assert_eq!(climatology.evidence.index_kind, "ndvi");
        assert_eq!(climatology.evidence.spatial_ref, spatial_ref_2x1());
        let june = &climatology.evidence.periods[0];
        assert_eq!(june.years, vec![2020, 2021, 2022]);
        assert_eq!(
            june.product_ids,
            vec!["product:jun-2020", "product:jun-2021", "product:jun-2022"]
        );
        let july = &climatology.evidence.periods[1];
        assert_eq!(july.years, vec![2020]);
        assert_eq!(july.product_ids, vec!["product:jul-2020"]);
    }

    #[test]
    fn dekadal_cadence_groups_by_month_and_dekad() {
        assert_eq!(
            calendar_period_for(date(2026, 6, 10), CompositeCadence::Dekadal),
            CalendarPeriod {
                month: 6,
                dekad: Some(1)
            }
        );
        assert_eq!(
            calendar_period_for(date(2026, 6, 11), CompositeCadence::Dekadal),
            CalendarPeriod {
                month: 6,
                dekad: Some(2)
            }
        );
        assert_eq!(
            calendar_period_for(date(2026, 6, 28), CompositeCadence::Dekadal),
            CalendarPeriod {
                month: 6,
                dekad: Some(3)
            }
        );
        assert_eq!(
            calendar_period_for(date(2026, 6, 28), CompositeCadence::Monthly),
            CalendarPeriod {
                month: 6,
                dekad: None
            }
        );
        assert_eq!(
            CalendarPeriod {
                month: 6,
                dekad: Some(2)
            }
            .label(),
            "m06d2"
        );
    }

    #[test]
    fn serialize_then_reload_round_trips_exactly() {
        let climatology =
            build_index_climatology(&three_year_request(3)).expect("climatology builds");
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("ndvi-climatology.json");
        write_climatology_json(&climatology, &path).expect("write artifact");
        let reloaded = read_climatology_json(&path).expect("read artifact");
        assert_eq!(climatology, reloaded);
    }

    #[test]
    fn corrupt_artifact_shapes_are_rejected_on_reload() {
        let mut climatology =
            build_index_climatology(&three_year_request(3)).expect("climatology builds");
        climatology.periods[0].min.pop();
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("corrupt.json");
        write_climatology_json(&climatology, &path).expect("write artifact");
        let error = read_climatology_json(&path).expect_err("corrupt shape rejected");
        assert!(matches!(
            error,
            ClimatologyPersistError::CorruptArtifact { .. }
        ));
    }

    #[test]
    fn l3_draft_carries_all_inputs_and_baseline_confidence() {
        let climatology =
            build_index_climatology(&three_year_request(3)).expect("climatology builds");
        let scope = ClimatologyL3Scope {
            field_id: "field-1".to_string(),
            season_id: "2020-2022".to_string(),
            scene_id: None,
            source_id: None,
        };
        let draft = climatology_l3_draft(&climatology, &scope);
        assert_eq!(draft.level, ProductLevel::L3);
        assert_eq!(draft.kind, "index_climatology");
        assert_eq!(draft.algorithm_id, "climatology.ndvi");
        let input_ids: Vec<&str> = draft
            .inputs
            .iter()
            .map(|input| input.product_id.as_str())
            .collect();
        assert_eq!(
            input_ids,
            vec![
                "product:jun-2020",
                "product:jun-2021",
                "product:jun-2022",
                "product:jul-2020"
            ]
        );
        assert_eq!(draft.scope.temporal_start, "2020-01-01T00:00:00Z");
        assert_eq!(draft.scope.temporal_end, "2022-12-31T23:59:59Z");
        // 4 pixel-periods total (2 periods x 2 pixels); only June pixel 0 is Ok.
        assert_eq!(draft.confidence, Some(0.25));
        assert_eq!(
            draft.evidence_digests,
            vec![climatology.evidence.input_hash.clone()]
        );
        // Identity is stable across identical runs.
        let again = climatology_l3_draft(
            &build_index_climatology(&three_year_request(3)).expect("rebuild"),
            &scope,
        );
        assert_eq!(draft.parameters_hash(), again.parameters_hash());
    }
}
