//! Level-3 temporal compositing (satellite intelligence pipeline, Phase 2).
//!
//! Combines N co-registered, mask-first observations of the same grid into a
//! single composite raster per period. Deterministic methods only:
//!
//! - [`CompositeMethod::MaxNdvi`] — per pixel, the observation with the highest
//!   NDVI wins and supplies *all* bands (spectrally consistent).
//! - [`CompositeMethod::Medoid`] — per pixel, the observation whose band vector
//!   minimizes the sum of Euclidean distances to the other valid observations
//!   (spectrally consistent; ties broken by earliest date, then lowest index).
//! - [`CompositeMethod::Median`] — per-band median of valid observations. The
//!   output band vectors are synthetic aggregates, **not** real observed
//!   spectra; no observation is "selected".
//!
//! Every composite carries a [`CompositeEvidence`] object (method, period,
//! ordered input product ids, mask rule, band names, grid spatial ref, input
//! hash) and maps to an L3 [`ProductRecordDraft`] via [`composite_l3_draft`],
//! with every input product as a lineage edge — the same identity invariant as
//! [`crate::l3_product`].

use crate::evidence::{deterministic_fingerprint, AnalysisEvidenceError};
use crate::l3_product::{to_l3_draft, L3DraftContext};
use chrono::{Datelike, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use shared::product_graph::ProductRecordDraft;
use shared::schemas::{assert_raster_spatial_ref, RasterSpatialRef, RasterSpatialRefError};
use thiserror::Error;

/// Sentinel for "no observation selected" in the per-pixel selected-index
/// layer: every gap pixel, and every pixel of a [`CompositeMethod::Median`]
/// composite (median outputs are aggregates, not selections).
pub const COMPOSITE_NO_SELECTION: i32 = -1;

/// One co-registered observation entering a composite: per-band f32 arrays
/// (row-major, `width * height`) plus the batch-1 validity mask (`true` =
/// usable pixel after cloud/shadow/fill masking).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeObservation {
    /// Catalog product id of the (L2) input — becomes an L3 lineage edge.
    pub product_id: String,
    /// Source scene id (recorded in evidence).
    pub scene_id: String,
    /// Observation date (UTC calendar date).
    pub observed_on: NaiveDate,
    /// One array per request band, in request band order.
    pub bands: Vec<Vec<f32>>,
    /// `true` = valid pixel (mask-first contract from imagery_processor masks).
    pub valid_mask: Vec<bool>,
    /// Grid georeference; must equal the request grid exactly (no resampling).
    pub spatial_ref: RasterSpatialRef,
}

/// Deterministic compositing method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "method")]
pub enum CompositeMethod {
    /// Select the observation with the highest per-pixel NDVI
    /// `(nir - red) / (nir + red)`; band indices refer to the request bands.
    MaxNdvi { red_band: usize, nir_band: usize },
    /// Select the observation minimizing the summed Euclidean distance to all
    /// other valid observations' band vectors.
    Medoid,
    /// Per-band median (synthetic, not an observed spectrum).
    Median,
}

impl CompositeMethod {
    /// Stable snake_case label used in evidence and algorithm ids.
    pub fn label(&self) -> &'static str {
        match self {
            CompositeMethod::MaxNdvi { .. } => "max_ndvi",
            CompositeMethod::Medoid => "medoid",
            CompositeMethod::Median => "median",
        }
    }
}

/// A composite request over N co-registered observations of one grid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeRequest {
    pub width: u32,
    pub height: u32,
    /// Band names in array order (e.g. `["red", "nir"]`).
    pub band_names: Vec<String>,
    /// Grid georeference all observations must match.
    pub spatial_ref: RasterSpatialRef,
    pub observations: Vec<CompositeObservation>,
    pub method: CompositeMethod,
    /// Human-readable description of the upstream mask rule (e.g.
    /// `"s2_scl_keep_4_5_6_dilate_1px"`), recorded in evidence.
    pub mask_rule: String,
}

/// Per-pixel outcome code for the composite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositePixelReason {
    /// At least one valid observation contributed to this pixel.
    Composited,
    /// No observation was valid here — the pixel is a gap (bands are NaN).
    NoValidObservation,
}

/// Evidence object for one composite run (doctrine: every derived raster
/// records its inputs, mask rule, and algorithm; content-hashed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeEvidence {
    pub method: CompositeMethod,
    /// ISO dates (UTC) of the composited period, inclusive.
    pub period_start: String,
    pub period_end: String,
    /// Input catalog product ids, in observation order.
    pub input_product_ids: Vec<String>,
    /// Input scene ids, in observation order.
    pub input_scene_ids: Vec<String>,
    pub mask_rule: String,
    pub band_names: Vec<String>,
    pub spatial_ref: RasterSpatialRef,
    /// Deterministic fingerprint over the full request (pixels included).
    pub input_hash: String,
}

/// A completed composite: bands plus per-pixel metadata layers and summary
/// statistics. Gap pixels hold `f32::NAN` in every band.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeResult {
    pub width: u32,
    pub height: u32,
    pub band_names: Vec<String>,
    /// Composite bands, one array per band name, row-major.
    pub bands: Vec<Vec<f32>>,
    /// NDVI of the selected observation (MaxNdvi only; NaN at gaps).
    pub ndvi: Option<Vec<f32>>,
    /// Per-pixel count of valid observations.
    pub valid_count: Vec<u32>,
    /// Per-pixel selected observation index (MaxNdvi/Medoid), or
    /// [`COMPOSITE_NO_SELECTION`] for gaps and for every Median pixel.
    pub selected_index: Vec<i32>,
    pub reason_codes: Vec<CompositePixelReason>,
    /// Fraction of pixels with no valid observation.
    pub gap_fraction: f32,
    /// How many pixels each observation won (all zeros for Median).
    pub selection_counts: Vec<u32>,
    /// Earliest / latest observation date (UTC, inclusive).
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub method: CompositeMethod,
    pub mask_rule: String,
    /// Input catalog product ids, in observation order.
    pub input_product_ids: Vec<String>,
    pub spatial_ref: RasterSpatialRef,
    pub evidence: CompositeEvidence,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum TemporalCompositeError {
    #[error("composite request has no observations")]
    NoObservations,
    #[error("composite request declares no bands")]
    NoBands,
    #[error("observation {observation_index} has {actual} bands, expected {expected}")]
    BandCountMismatch {
        observation_index: usize,
        expected: usize,
        actual: usize,
    },
    #[error(
        "observation {observation_index} band {band_index} has {actual} pixels, expected {expected}"
    )]
    BandLengthMismatch {
        observation_index: usize,
        band_index: usize,
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
        "observation {observation_index} spatial ref does not match the composite grid (no resampling here)"
    )]
    SpatialRefMismatch { observation_index: usize },
    #[error("composite grid spatial metadata is invalid: {reason}")]
    SpatialRef { reason: RasterSpatialRefError },
    #[error("max-NDVI {role} band index {index} out of range for {band_count} bands")]
    InvalidBandIndex {
        role: &'static str,
        index: usize,
        band_count: usize,
    },
    #[error("max-NDVI red and NIR band indices must differ (both {index})")]
    DegenerateNdviBands { index: usize },
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] AnalysisEvidenceError),
}

/// Build a deterministic temporal composite from co-registered observations.
pub fn compose_temporal(
    request: &CompositeRequest,
) -> Result<CompositeResult, TemporalCompositeError> {
    validate_request(request)?;

    let pixel_count = request.width as usize * request.height as usize;
    let band_count = request.band_names.len();
    let observations = &request.observations;

    let mut bands = vec![vec![f32::NAN; pixel_count]; band_count];
    let mut ndvi = match request.method {
        CompositeMethod::MaxNdvi { .. } => Some(vec![f32::NAN; pixel_count]),
        _ => None,
    };
    let mut valid_count = vec![0u32; pixel_count];
    let mut selected_index = vec![COMPOSITE_NO_SELECTION; pixel_count];
    let mut reason_codes = vec![CompositePixelReason::NoValidObservation; pixel_count];
    let mut selection_counts = vec![0u32; observations.len()];
    let mut gap_pixels = 0u32;

    for pixel in 0..pixel_count {
        let valid: Vec<usize> = (0..observations.len())
            .filter(|&index| pixel_is_valid(&observations[index], pixel))
            .collect();
        valid_count[pixel] = valid.len() as u32;
        if valid.is_empty() {
            gap_pixels += 1;
            continue;
        }
        reason_codes[pixel] = CompositePixelReason::Composited;

        match request.method {
            CompositeMethod::MaxNdvi { red_band, nir_band } => {
                if let Some((winner, winner_ndvi)) =
                    select_max_ndvi(observations, &valid, pixel, red_band, nir_band)
                {
                    copy_selected_pixel(&mut bands, &observations[winner], pixel);
                    ndvi.as_mut().expect("max-ndvi allocates ndvi")[pixel] = winner_ndvi;
                    selected_index[pixel] = winner as i32;
                    selection_counts[winner] += 1;
                } else {
                    // Every valid observation had a degenerate NDVI denominator.
                    reason_codes[pixel] = CompositePixelReason::NoValidObservation;
                    valid_count[pixel] = 0;
                    gap_pixels += 1;
                }
            }
            CompositeMethod::Medoid => {
                let winner = select_medoid(observations, &valid, pixel);
                copy_selected_pixel(&mut bands, &observations[winner], pixel);
                selected_index[pixel] = winner as i32;
                selection_counts[winner] += 1;
            }
            CompositeMethod::Median => {
                for (band_index, band) in bands.iter_mut().enumerate() {
                    let mut values: Vec<f32> = valid
                        .iter()
                        .map(|&index| observations[index].bands[band_index][pixel])
                        .collect();
                    values.sort_by(|left, right| left.total_cmp(right));
                    band[pixel] = median_of_sorted(&values);
                }
            }
        }
    }

    let period_start = observations
        .iter()
        .map(|obs| obs.observed_on)
        .min()
        .expect("observations are non-empty");
    let period_end = observations
        .iter()
        .map(|obs| obs.observed_on)
        .max()
        .expect("observations are non-empty");
    let input_product_ids: Vec<String> = observations
        .iter()
        .map(|obs| obs.product_id.clone())
        .collect();
    let input_scene_ids: Vec<String> = observations
        .iter()
        .map(|obs| obs.scene_id.clone())
        .collect();

    let input_hash = deterministic_fingerprint(&(
        "temporal_composite_v1",
        &request.method,
        &request.mask_rule,
        &request.band_names,
        &request.spatial_ref,
        request.width,
        request.height,
        observations,
    ))?;
    let evidence = CompositeEvidence {
        method: request.method.clone(),
        period_start: period_start.to_string(),
        period_end: period_end.to_string(),
        input_product_ids: input_product_ids.clone(),
        input_scene_ids,
        mask_rule: request.mask_rule.clone(),
        band_names: request.band_names.clone(),
        spatial_ref: request.spatial_ref.clone(),
        input_hash,
    };

    Ok(CompositeResult {
        width: request.width,
        height: request.height,
        band_names: request.band_names.clone(),
        bands,
        ndvi,
        valid_count,
        selected_index,
        reason_codes,
        gap_fraction: gap_pixels as f32 / pixel_count as f32,
        selection_counts,
        period_start,
        period_end,
        method: request.method.clone(),
        mask_rule: request.mask_rule.clone(),
        input_product_ids,
        spatial_ref: request.spatial_ref.clone(),
        evidence,
    })
}

fn validate_request(request: &CompositeRequest) -> Result<(), TemporalCompositeError> {
    if request.observations.is_empty() {
        return Err(TemporalCompositeError::NoObservations);
    }
    if request.band_names.is_empty() {
        return Err(TemporalCompositeError::NoBands);
    }
    assert_raster_spatial_ref(Some(&request.spatial_ref), request.width, request.height)
        .map_err(|reason| TemporalCompositeError::SpatialRef { reason })?;

    let band_count = request.band_names.len();
    if let CompositeMethod::MaxNdvi { red_band, nir_band } = request.method {
        for (role, index) in [("red", red_band), ("nir", nir_band)] {
            if index >= band_count {
                return Err(TemporalCompositeError::InvalidBandIndex {
                    role,
                    index,
                    band_count,
                });
            }
        }
        if red_band == nir_band {
            return Err(TemporalCompositeError::DegenerateNdviBands { index: red_band });
        }
    }

    let pixel_count = request.width as usize * request.height as usize;
    for (observation_index, observation) in request.observations.iter().enumerate() {
        if observation.bands.len() != band_count {
            return Err(TemporalCompositeError::BandCountMismatch {
                observation_index,
                expected: band_count,
                actual: observation.bands.len(),
            });
        }
        for (band_index, band) in observation.bands.iter().enumerate() {
            if band.len() != pixel_count {
                return Err(TemporalCompositeError::BandLengthMismatch {
                    observation_index,
                    band_index,
                    expected: pixel_count,
                    actual: band.len(),
                });
            }
        }
        if observation.valid_mask.len() != pixel_count {
            return Err(TemporalCompositeError::MaskLengthMismatch {
                observation_index,
                expected: pixel_count,
                actual: observation.valid_mask.len(),
            });
        }
        if observation.spatial_ref != request.spatial_ref {
            return Err(TemporalCompositeError::SpatialRefMismatch { observation_index });
        }
    }
    Ok(())
}

/// A pixel is usable only when the mask says valid *and* every band value is
/// finite (a masked-valid NaN would silently poison the composite).
fn pixel_is_valid(observation: &CompositeObservation, pixel: usize) -> bool {
    observation.valid_mask[pixel] && observation.bands.iter().all(|band| band[pixel].is_finite())
}

/// Highest NDVI wins; ties go to the earliest date, then the lowest index.
/// Observations with a zero NDVI denominator are skipped.
fn select_max_ndvi(
    observations: &[CompositeObservation],
    valid: &[usize],
    pixel: usize,
    red_band: usize,
    nir_band: usize,
) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32)> = None;
    for &index in valid {
        let red = observations[index].bands[red_band][pixel];
        let nir = observations[index].bands[nir_band][pixel];
        let denominator = nir + red;
        if denominator == 0.0 {
            continue;
        }
        let ndvi = (nir - red) / denominator;
        best = Some(match best {
            None => (index, ndvi),
            Some((best_index, best_ndvi)) => {
                if ndvi > best_ndvi
                    || (ndvi == best_ndvi
                        && date_index_key(observations, index)
                            < date_index_key(observations, best_index))
                {
                    (index, ndvi)
                } else {
                    (best_index, best_ndvi)
                }
            }
        });
    }
    best
}

/// Minimum summed Euclidean distance to the other valid band vectors wins;
/// ties go to the earliest date, then the lowest index. Distances accumulate
/// in f64 for determinism.
fn select_medoid(observations: &[CompositeObservation], valid: &[usize], pixel: usize) -> usize {
    let mut best_index = valid[0];
    let mut best_cost = f64::INFINITY;
    for &candidate in valid {
        let cost: f64 = valid
            .iter()
            .filter(|&&other| other != candidate)
            .map(|&other| {
                observations[candidate]
                    .bands
                    .iter()
                    .zip(observations[other].bands.iter())
                    .map(|(a, b)| {
                        let delta = a[pixel] as f64 - b[pixel] as f64;
                        delta * delta
                    })
                    .sum::<f64>()
                    .sqrt()
            })
            .sum();
        if cost < best_cost
            || (cost == best_cost
                && date_index_key(observations, candidate)
                    < date_index_key(observations, best_index))
        {
            best_index = candidate;
            best_cost = cost;
        }
    }
    best_index
}

fn date_index_key(observations: &[CompositeObservation], index: usize) -> (NaiveDate, usize) {
    (observations[index].observed_on, index)
}

fn copy_selected_pixel(bands: &mut [Vec<f32>], observation: &CompositeObservation, pixel: usize) {
    for (band, source) in bands.iter_mut().zip(observation.bands.iter()) {
        band[pixel] = source[pixel];
    }
}

/// Median of a non-empty total-order-sorted slice: middle element for odd
/// counts, mean of the two middle elements for even counts (synthetic value).
fn median_of_sorted(sorted: &[f32]) -> f32 {
    let count = sorted.len();
    if count % 2 == 1 {
        sorted[count / 2]
    } else {
        (sorted[count / 2 - 1] + sorted[count / 2]) / 2.0
    }
}

// ---------------------------------------------------------------------------
// Period helpers
// ---------------------------------------------------------------------------

/// Compositing cadence: calendar months, or dekads (day 1–10, 11–20,
/// 21–end-of-month).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositeCadence {
    Monthly,
    Dekadal,
}

/// One composite window: inclusive UTC date range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CompositePeriod {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

/// The composite window containing `date` under the given cadence.
pub fn composite_period_for(date: NaiveDate, cadence: CompositeCadence) -> CompositePeriod {
    let month_start = date.with_day(1).expect("day 1 exists in every month");
    let month_end = month_start
        .checked_add_months(Months::new(1))
        .and_then(|next| next.pred_opt())
        .expect("previous day of a month start exists");
    match cadence {
        CompositeCadence::Monthly => CompositePeriod {
            start: month_start,
            end: month_end,
        },
        CompositeCadence::Dekadal => match date.day() {
            1..=10 => CompositePeriod {
                start: month_start,
                end: month_start.with_day(10).expect("day 10 exists"),
            },
            11..=20 => CompositePeriod {
                start: month_start.with_day(11).expect("day 11 exists"),
                end: month_start.with_day(20).expect("day 20 exists"),
            },
            _ => CompositePeriod {
                start: month_start.with_day(21).expect("day 21 exists"),
                end: month_end,
            },
        },
    }
}

/// Group dated observations into composite windows. Returns windows sorted by
/// start date; within each window, observation indices are sorted by
/// (date, original index) — fully deterministic.
pub fn group_observation_dates(
    dates: &[NaiveDate],
    cadence: CompositeCadence,
) -> Vec<(CompositePeriod, Vec<usize>)> {
    let mut windows: std::collections::BTreeMap<CompositePeriod, Vec<usize>> =
        std::collections::BTreeMap::new();
    for (index, &date) in dates.iter().enumerate() {
        windows
            .entry(composite_period_for(date, cadence))
            .or_default()
            .push(index);
    }
    windows
        .into_iter()
        .map(|(period, mut indices)| {
            indices.sort_by_key(|&index| (dates[index], index));
            (period, indices)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// L3 lineage
// ---------------------------------------------------------------------------

/// Scope a composite L3 draft cannot derive from pixel data alone.
#[derive(Debug, Clone)]
pub struct CompositeL3Scope {
    pub field_id: String,
    pub season_id: String,
    /// Composites span scenes; `None` leaves the product field-scoped.
    pub scene_id: Option<String>,
    pub source_id: Option<String>,
}

/// Map a completed composite to an L3 catalog draft. Every input product id
/// becomes a lineage edge (`l2_input`), following [`crate::l3_product`]:
/// an L3 composite that omitted its inputs would collapse with every other
/// run of the same parameters. Confidence is the valid-coverage fraction
/// (`1 - gap_fraction`), a directly observed quantity.
pub fn composite_l3_draft(
    result: &CompositeResult,
    scope: &CompositeL3Scope,
) -> ProductRecordDraft {
    to_l3_draft(&L3DraftContext {
        kind: "temporal_composite".to_string(),
        algorithm_id: format!("temporal_composite.{}", result.method.label()),
        algorithm_version: "1.0.0".to_string(),
        field_id: scope.field_id.clone(),
        season_id: scope.season_id.clone(),
        scene_id: scope.scene_id.clone(),
        temporal_start: format!("{}T00:00:00Z", result.period_start),
        temporal_end: format!("{}T23:59:59Z", result.period_end),
        input_product_ids: result.input_product_ids.clone(),
        parameters: serde_json::json!({
            "method": result.method,
            "mask_rule": result.mask_rule,
            "band_names": result.band_names,
            "period_start": result.period_start.to_string(),
            "period_end": result.period_end.to_string(),
        }),
        confidence: Some(1.0 - result.gap_fraction as f64),
        confidence_method: Some("valid_coverage_fraction".to_string()),
        evidence_digests: vec![result.evidence.input_hash.clone()],
        source_id: scope.source_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::product_graph::ProductLevel;
    use shared::schemas::{GeoBounds, RasterResolution};

    fn spatial_ref_2x2() -> RasterSpatialRef {
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

    fn spatial_ref_1x1() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32614".to_string()),
            bbox: Some(GeoBounds {
                min_lon: 500000.0,
                min_lat: 4500000.0,
                max_lon: 500010.0,
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
        bands: Vec<Vec<f32>>,
        valid_mask: Vec<bool>,
        spatial_ref: RasterSpatialRef,
    ) -> CompositeObservation {
        CompositeObservation {
            product_id: format!("product:{id}"),
            scene_id: format!("scene:{id}"),
            observed_on,
            bands,
            valid_mask,
            spatial_ref,
        }
    }

    /// Three flat 2x2 observations: obs0 NDVI 0.6, obs1 NDVI 0.8 (pixel 0
    /// masked out), obs2 NDVI 0.0.
    fn max_ndvi_request() -> CompositeRequest {
        let sref = spatial_ref_2x2();
        CompositeRequest {
            width: 2,
            height: 2,
            band_names: vec!["red".to_string(), "nir".to_string()],
            spatial_ref: sref.clone(),
            observations: vec![
                observation(
                    "a",
                    date(2026, 6, 3),
                    vec![vec![0.2; 4], vec![0.8; 4]],
                    vec![true; 4],
                    sref.clone(),
                ),
                observation(
                    "b",
                    date(2026, 6, 12),
                    vec![vec![0.1; 4], vec![0.9; 4]],
                    vec![false, true, true, true],
                    sref.clone(),
                ),
                observation(
                    "c",
                    date(2026, 6, 25),
                    vec![vec![0.5; 4], vec![0.5; 4]],
                    vec![true; 4],
                    sref,
                ),
            ],
            method: CompositeMethod::MaxNdvi {
                red_band: 0,
                nir_band: 1,
            },
            mask_rule: "s2_scl_keep_4_5_6".to_string(),
        }
    }

    #[test]
    fn max_ndvi_selects_highest_ndvi_and_falls_back_when_masked() {
        let result = compose_temporal(&max_ndvi_request()).expect("composite");

        // Pixel 0: obs1 (NDVI 0.8) is masked out -> obs0 (0.6) beats obs2 (0.0).
        assert_eq!(result.selected_index, vec![0, 1, 1, 1]);
        assert_eq!(result.valid_count, vec![2, 3, 3, 3]);
        assert_eq!(result.selection_counts, vec![1, 3, 0]);
        let ndvi = result.ndvi.as_ref().expect("max-ndvi emits ndvi");
        assert!((ndvi[0] - 0.6).abs() < 1.0e-6);
        assert!((ndvi[1] - 0.8).abs() < 1.0e-6);
        assert_eq!(result.gap_fraction, 0.0);
        assert!(result
            .reason_codes
            .iter()
            .all(|reason| *reason == CompositePixelReason::Composited));
    }

    #[test]
    fn max_ndvi_output_bands_are_spectrally_consistent_with_the_winner() {
        let result = compose_temporal(&max_ndvi_request()).expect("composite");

        // All bands of a pixel come from the single selected observation.
        assert_eq!(result.bands[0], vec![0.2, 0.1, 0.1, 0.1]); // red
        assert_eq!(result.bands[1], vec![0.8, 0.9, 0.9, 0.9]); // nir
    }

    #[test]
    fn medoid_selects_the_central_band_vector() {
        // 1x1 grid, 2 bands, vectors a=(0,0), b=(1,0), c=(10,0):
        // cost(a)=1+10=11, cost(b)=1+9=10, cost(c)=10+9=19 -> medoid is b.
        let sref = spatial_ref_1x1();
        let request = CompositeRequest {
            width: 1,
            height: 1,
            band_names: vec!["red".to_string(), "nir".to_string()],
            spatial_ref: sref.clone(),
            observations: vec![
                observation(
                    "a",
                    date(2026, 6, 1),
                    vec![vec![0.0], vec![0.0]],
                    vec![true],
                    sref.clone(),
                ),
                observation(
                    "b",
                    date(2026, 6, 11),
                    vec![vec![1.0], vec![0.0]],
                    vec![true],
                    sref.clone(),
                ),
                observation(
                    "c",
                    date(2026, 6, 21),
                    vec![vec![10.0], vec![0.0]],
                    vec![true],
                    sref,
                ),
            ],
            method: CompositeMethod::Medoid,
            mask_rule: "qa_pixel_clear".to_string(),
        };

        let result = compose_temporal(&request).expect("composite");
        assert_eq!(result.selected_index, vec![1]);
        assert_eq!(result.selection_counts, vec![0, 1, 0]);
        assert_eq!(result.bands[0], vec![1.0]);
        assert_eq!(result.bands[1], vec![0.0]);
        assert!(result.ndvi.is_none());
    }

    #[test]
    fn medoid_ties_break_by_earliest_date_then_lowest_index() {
        // Two identical vectors tie on cost; the later-listed one has the
        // earlier date and must win.
        let sref = spatial_ref_1x1();
        let mut request = CompositeRequest {
            width: 1,
            height: 1,
            band_names: vec!["red".to_string()],
            spatial_ref: sref.clone(),
            observations: vec![
                observation(
                    "late",
                    date(2026, 6, 20),
                    vec![vec![0.0]],
                    vec![true],
                    sref.clone(),
                ),
                observation(
                    "early",
                    date(2026, 6, 5),
                    vec![vec![0.0]],
                    vec![true],
                    sref.clone(),
                ),
                observation("far", date(2026, 6, 10), vec![vec![7.0]], vec![true], sref),
            ],
            method: CompositeMethod::Medoid,
            mask_rule: "qa_pixel_clear".to_string(),
        };
        let result = compose_temporal(&request).expect("composite");
        assert_eq!(result.selected_index, vec![1], "earliest date wins the tie");

        // Equal dates -> lowest index wins.
        request.observations[1].observed_on = date(2026, 6, 20);
        let result = compose_temporal(&request).expect("composite");
        assert_eq!(
            result.selected_index,
            vec![0],
            "lowest index wins on equal dates"
        );
    }

    #[test]
    fn median_composites_per_band_values() {
        // 1x1 grid, 1 band, 4 observations [1, 2, 3, 10]; the last is masked
        // out at first -> median of [1, 2, 3] = 2; with all 4 valid the even
        // count averages the middle pair: (2 + 3) / 2 = 2.5.
        let sref = spatial_ref_1x1();
        let mut request = CompositeRequest {
            width: 1,
            height: 1,
            band_names: vec!["red".to_string()],
            spatial_ref: sref.clone(),
            observations: vec![
                observation(
                    "a",
                    date(2026, 6, 1),
                    vec![vec![1.0]],
                    vec![true],
                    sref.clone(),
                ),
                observation(
                    "b",
                    date(2026, 6, 8),
                    vec![vec![2.0]],
                    vec![true],
                    sref.clone(),
                ),
                observation(
                    "c",
                    date(2026, 6, 15),
                    vec![vec![3.0]],
                    vec![true],
                    sref.clone(),
                ),
                observation("d", date(2026, 6, 22), vec![vec![10.0]], vec![false], sref),
            ],
            method: CompositeMethod::Median,
            mask_rule: "qa_pixel_clear".to_string(),
        };

        let odd = compose_temporal(&request).expect("composite");
        assert_eq!(odd.bands[0], vec![2.0]);
        assert_eq!(odd.selected_index, vec![COMPOSITE_NO_SELECTION]);
        assert_eq!(odd.selection_counts, vec![0, 0, 0, 0]);

        request.observations[3].valid_mask = vec![true];
        let even = compose_temporal(&request).expect("composite");
        assert_eq!(even.bands[0], vec![2.5]);
        assert_eq!(even.valid_count, vec![4]);
    }

    #[test]
    fn all_invalid_pixel_becomes_a_gap_with_reason_code_and_counts() {
        let mut request = max_ndvi_request();
        for observation in &mut request.observations {
            observation.valid_mask[3] = false;
        }

        let result = compose_temporal(&request).expect("composite");
        assert_eq!(
            result.reason_codes[3],
            CompositePixelReason::NoValidObservation
        );
        assert_eq!(result.selected_index[3], COMPOSITE_NO_SELECTION);
        assert_eq!(result.valid_count[3], 0);
        assert!(result.bands[0][3].is_nan() && result.bands[1][3].is_nan());
        assert!(result.ndvi.as_ref().expect("ndvi layer")[3].is_nan());
        assert_eq!(result.gap_fraction, 0.25);
        assert_eq!(result.selection_counts, vec![1, 2, 0]);
    }

    #[test]
    fn dimension_mismatch_is_a_typed_error() {
        let mut request = max_ndvi_request();
        request.observations[1].bands[0].pop();

        let error = compose_temporal(&request).expect_err("short band is rejected");
        assert_eq!(
            error,
            TemporalCompositeError::BandLengthMismatch {
                observation_index: 1,
                band_index: 0,
                expected: 4,
                actual: 3,
            }
        );

        let mut request = max_ndvi_request();
        request.observations[2].valid_mask.push(true);
        let error = compose_temporal(&request).expect_err("long mask is rejected");
        assert_eq!(
            error,
            TemporalCompositeError::MaskLengthMismatch {
                observation_index: 2,
                expected: 4,
                actual: 5,
            }
        );
    }

    #[test]
    fn spatial_ref_mismatch_is_a_typed_error_without_resampling() {
        let mut request = max_ndvi_request();
        request.observations[2].spatial_ref.crs = Some("EPSG:32615".to_string());

        let error = compose_temporal(&request).expect_err("foreign CRS is rejected");
        assert_eq!(
            error,
            TemporalCompositeError::SpatialRefMismatch {
                observation_index: 2
            }
        );
    }

    #[test]
    fn dekadal_windows_split_months_into_three_deterministic_dekads() {
        let dates = vec![
            date(2026, 1, 1),
            date(2026, 1, 10),
            date(2026, 1, 11),
            date(2026, 1, 21),
            date(2026, 1, 31),
            date(2026, 2, 21), // third February dekad ends on day 28
        ];
        let windows = group_observation_dates(&dates, CompositeCadence::Dekadal);

        assert_eq!(windows.len(), 4);
        assert_eq!(
            windows[0],
            (
                CompositePeriod {
                    start: date(2026, 1, 1),
                    end: date(2026, 1, 10)
                },
                vec![0, 1]
            )
        );
        assert_eq!(
            windows[1],
            (
                CompositePeriod {
                    start: date(2026, 1, 11),
                    end: date(2026, 1, 20)
                },
                vec![2]
            )
        );
        assert_eq!(
            windows[2],
            (
                CompositePeriod {
                    start: date(2026, 1, 21),
                    end: date(2026, 1, 31)
                },
                vec![3, 4]
            )
        );
        assert_eq!(
            windows[3],
            (
                CompositePeriod {
                    start: date(2026, 2, 21),
                    end: date(2026, 2, 28)
                },
                vec![5]
            )
        );

        // Leap-year February third dekad ends on day 29.
        let leap = composite_period_for(date(2024, 2, 25), CompositeCadence::Dekadal);
        assert_eq!(leap.end, date(2024, 2, 29));
    }

    #[test]
    fn monthly_windows_respect_month_boundaries() {
        let dates = vec![date(2026, 1, 31), date(2026, 2, 1), date(2026, 2, 28)];
        let windows = group_observation_dates(&dates, CompositeCadence::Monthly);

        assert_eq!(windows.len(), 2);
        assert_eq!(
            windows[0],
            (
                CompositePeriod {
                    start: date(2026, 1, 1),
                    end: date(2026, 1, 31)
                },
                vec![0]
            )
        );
        assert_eq!(
            windows[1],
            (
                CompositePeriod {
                    start: date(2026, 2, 1),
                    end: date(2026, 2, 28)
                },
                vec![1, 2]
            )
        );
    }

    #[test]
    fn l3_draft_carries_all_input_products_and_method_parameters() {
        let result = compose_temporal(&max_ndvi_request()).expect("composite");
        let scope = CompositeL3Scope {
            field_id: "field-1".to_string(),
            season_id: "2026".to_string(),
            scene_id: None,
            source_id: None,
        };
        let draft = composite_l3_draft(&result, &scope);

        assert_eq!(draft.level, ProductLevel::L3);
        assert_eq!(draft.kind, "temporal_composite");
        assert_eq!(draft.algorithm_id, "temporal_composite.max_ndvi");
        let input_ids: Vec<&str> = draft
            .inputs
            .iter()
            .map(|input| input.product_id.as_str())
            .collect();
        assert_eq!(input_ids, vec!["product:a", "product:b", "product:c"]);
        assert!(draft.inputs.iter().all(|input| input.role == "l2_input"));
        assert_eq!(draft.parameters["method"]["method"], "max_ndvi");
        assert_eq!(draft.parameters["method"]["red_band"], 0);
        assert_eq!(draft.parameters["mask_rule"], "s2_scl_keep_4_5_6");
        assert_eq!(draft.parameters["period_start"], "2026-06-03");
        assert_eq!(draft.parameters["period_end"], "2026-06-25");
        assert_eq!(draft.scope.temporal_start, "2026-06-03T00:00:00Z");
        assert_eq!(draft.confidence, Some(1.0));
        assert_eq!(
            draft.evidence_digests,
            vec![result.evidence.input_hash.clone()]
        );
    }

    #[test]
    fn identical_runs_produce_stable_evidence_hash_and_l3_identity() {
        let scope = CompositeL3Scope {
            field_id: "field-1".to_string(),
            season_id: "2026".to_string(),
            scene_id: None,
            source_id: None,
        };
        let first = compose_temporal(&max_ndvi_request()).expect("first run");
        let second = compose_temporal(&max_ndvi_request()).expect("second run");

        assert_eq!(first.evidence, second.evidence);
        assert_eq!(first.evidence.input_hash, second.evidence.input_hash);
        assert_eq!(
            composite_l3_draft(&first, &scope).parameters_hash(),
            composite_l3_draft(&second, &scope).parameters_hash()
        );

        // Different inputs -> different identity (no collapse).
        let mut other = max_ndvi_request();
        other.observations[0].product_id = "product:z".to_string();
        let third = compose_temporal(&other).expect("third run");
        assert_ne!(
            composite_l3_draft(&first, &scope).parameters_hash(),
            composite_l3_draft(&third, &scope).parameters_hash()
        );
    }

    #[test]
    fn evidence_records_method_period_inputs_mask_rule_and_grid() {
        let result = compose_temporal(&max_ndvi_request()).expect("composite");
        let evidence = &result.evidence;

        assert_eq!(
            evidence.method,
            CompositeMethod::MaxNdvi {
                red_band: 0,
                nir_band: 1
            }
        );
        assert_eq!(evidence.period_start, "2026-06-03");
        assert_eq!(evidence.period_end, "2026-06-25");
        assert_eq!(
            evidence.input_product_ids,
            vec!["product:a", "product:b", "product:c"]
        );
        assert_eq!(
            evidence.input_scene_ids,
            vec!["scene:a", "scene:b", "scene:c"]
        );
        assert_eq!(evidence.mask_rule, "s2_scl_keep_4_5_6");
        assert_eq!(evidence.band_names, vec!["red", "nir"]);
        assert_eq!(evidence.spatial_ref, spatial_ref_2x2());
        assert!(!evidence.input_hash.is_empty());
    }

    #[test]
    fn invalid_max_ndvi_band_indices_are_typed_errors() {
        let mut request = max_ndvi_request();
        request.method = CompositeMethod::MaxNdvi {
            red_band: 0,
            nir_band: 5,
        };
        assert_eq!(
            compose_temporal(&request).expect_err("out of range nir"),
            TemporalCompositeError::InvalidBandIndex {
                role: "nir",
                index: 5,
                band_count: 2
            }
        );

        request.method = CompositeMethod::MaxNdvi {
            red_band: 1,
            nir_band: 1,
        };
        assert_eq!(
            compose_temporal(&request).expect_err("degenerate bands"),
            TemporalCompositeError::DegenerateNdviBands { index: 1 }
        );
    }
}
