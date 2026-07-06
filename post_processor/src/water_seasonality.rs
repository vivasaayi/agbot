//! Water-body seasonality analysis (satellite pipeline batch 39).
//!
//! A single water-extent mask says where water was on one date; a SERIES of
//! them says what kind of water body each pixel is. This engine folds a
//! same-grid series of binary water masks (the batch-15/18 `water_extent`
//! products) into a per-pixel **persistence** raster, classified with the
//! JRC Global Surface Water convention of separating permanent from
//! seasonal water:
//!
//! - `permanent`: water in at least [`PERMANENT_MIN_FRACTION`] of the
//!   pixel's valid observations,
//! - `seasonal`: water in at least [`SEASONAL_MIN_FRACTION`] but below the
//!   permanent floor,
//! - `ephemeral`: water at least once but below the seasonal floor,
//! - `never_water`: land in every valid observation,
//! - `insufficient`: fewer than the required valid observations — the
//!   pixel is not classified rather than guessed.
//!
//! Alongside the raster it reports the surface-area accounting the
//! water-availability story needs: min/max/mean water area over the series
//! and per-class areas (when the grid GSD is known).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use shared::product_graph::ProductRecordDraft;
use shared::schemas::{assert_raster_spatial_ref, RasterSpatialRef, RasterSpatialRefError};
use thiserror::Error;

use crate::evidence::{deterministic_fingerprint, AnalysisEvidenceError};
use crate::l3_product::{to_l3_draft, L3DraftContext};

/// Water in at least this fraction of valid observations = permanent.
pub const PERMANENT_MIN_FRACTION: f32 = 0.8;
/// Water in at least this fraction (but under permanent) = seasonal.
pub const SEASONAL_MIN_FRACTION: f32 = 0.25;
/// Pixels with fewer valid observations than this are not classified.
pub const DEFAULT_MIN_OBSERVATIONS: u32 = 3;

/// Per-pixel seasonality class (also the raster code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaterSeasonalityClass {
    /// Land in every valid observation. Code 0.
    NeverWater,
    /// Water at least once, below the seasonal floor. Code 1.
    Ephemeral,
    /// Water in [SEASONAL_MIN_FRACTION, PERMANENT_MIN_FRACTION). Code 2.
    Seasonal,
    /// Water in >= PERMANENT_MIN_FRACTION of observations. Code 3.
    Permanent,
    /// Too few valid observations to classify (raster nodata).
    Insufficient,
}

impl WaterSeasonalityClass {
    /// Raster code (Insufficient is nodata, no code).
    pub fn code(self) -> Option<u8> {
        match self {
            WaterSeasonalityClass::NeverWater => Some(0),
            WaterSeasonalityClass::Ephemeral => Some(1),
            WaterSeasonalityClass::Seasonal => Some(2),
            WaterSeasonalityClass::Permanent => Some(3),
            WaterSeasonalityClass::Insufficient => None,
        }
    }
}

/// One water-extent observation: the binary mask read back from a
/// registered `water_extent` product (1 water / 0 land / nodata invalid).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterExtentObservation {
    /// Catalog product id (becomes L3 lineage).
    pub product_id: String,
    pub observed_on: NaiveDate,
    /// Mask values, row-major: 1 = water, 0 = land.
    pub values: Vec<f32>,
    /// `true` = pixel was classified (water or land) in this observation.
    pub valid_mask: Vec<bool>,
}

/// A seasonality request over a same-grid series.
#[derive(Debug, Clone)]
pub struct WaterSeasonalityRequest {
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    pub observations: Vec<WaterExtentObservation>,
    /// Minimum valid observations to classify a pixel.
    pub min_observations: u32,
    /// Pixel edge length (m) for area accounting.
    pub gsd_m_per_px: Option<f64>,
}

/// Per-class pixel counts (and areas when the GSD is known).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SeasonalityCounts {
    pub never_water: u32,
    pub ephemeral: u32,
    pub seasonal: u32,
    pub permanent: u32,
    pub insufficient: u32,
}

/// Evidence for one seasonality run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterSeasonalityEvidence {
    pub input_product_ids: Vec<String>,
    pub permanent_min_fraction: f32,
    pub seasonal_min_fraction: f32,
    pub min_observations: u32,
    pub spatial_ref: RasterSpatialRef,
    pub input_hash: String,
}

/// A completed seasonality analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterSeasonalityResult {
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    /// Per-pixel class.
    pub classes: Vec<WaterSeasonalityClass>,
    /// Per-pixel water fraction over valid observations (NaN when
    /// insufficient).
    pub persistence: Vec<f32>,
    pub counts: SeasonalityCounts,
    /// Water area per observation (m^2), date order — the availability
    /// time series. `None` without a GSD.
    pub water_area_series_m2: Option<Vec<(NaiveDate, f64)>>,
    /// Min/mean/max of the per-observation water areas (m^2).
    pub water_area_min_m2: Option<f64>,
    pub water_area_mean_m2: Option<f64>,
    pub water_area_max_m2: Option<f64>,
    /// Permanent + seasonal area (m^2) — the dependable + seasonal supply.
    pub permanent_area_m2: Option<f64>,
    pub seasonal_area_m2: Option<f64>,
    pub observation_count: usize,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub evidence: WaterSeasonalityEvidence,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum WaterSeasonalityError {
    #[error("no observations to analyze")]
    NoObservations,
    #[error("observation {product_id} has {actual} pixels, expected {expected}")]
    LengthMismatch {
        product_id: String,
        expected: usize,
        actual: usize,
    },
    #[error("grid spatial metadata is invalid: {reason}")]
    SpatialRef { reason: RasterSpatialRefError },
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] AnalysisEvidenceError),
}

/// Fold a water-extent series into per-pixel seasonality. Deterministic;
/// observations are processed in the given order (callers sort by date).
pub fn analyze_water_seasonality(
    request: &WaterSeasonalityRequest,
) -> Result<WaterSeasonalityResult, WaterSeasonalityError> {
    assert_raster_spatial_ref(Some(&request.spatial_ref), request.width, request.height)
        .map_err(|reason| WaterSeasonalityError::SpatialRef { reason })?;
    if request.observations.is_empty() {
        return Err(WaterSeasonalityError::NoObservations);
    }
    let pixel_count = request.width as usize * request.height as usize;
    for observation in &request.observations {
        if observation.values.len() != pixel_count || observation.valid_mask.len() != pixel_count {
            return Err(WaterSeasonalityError::LengthMismatch {
                product_id: observation.product_id.clone(),
                expected: pixel_count,
                actual: if observation.values.len() != pixel_count {
                    observation.values.len()
                } else {
                    observation.valid_mask.len()
                },
            });
        }
    }

    // Per-pixel valid/water tallies + per-observation water areas.
    let mut valid_counts = vec![0u32; pixel_count];
    let mut water_counts = vec![0u32; pixel_count];
    let mut area_series = Vec::new();
    for observation in &request.observations {
        let mut water_pixels = 0u64;
        for pixel in 0..pixel_count {
            let value = observation.values[pixel];
            if !observation.valid_mask[pixel] || !value.is_finite() {
                continue;
            }
            valid_counts[pixel] += 1;
            if value >= 0.5 {
                water_counts[pixel] += 1;
                water_pixels += 1;
            }
        }
        area_series.push((observation.observed_on, water_pixels));
    }

    let mut classes = vec![WaterSeasonalityClass::Insufficient; pixel_count];
    let mut persistence = vec![f32::NAN; pixel_count];
    let mut counts = SeasonalityCounts::default();
    for pixel in 0..pixel_count {
        if valid_counts[pixel] < request.min_observations {
            counts.insufficient += 1;
            continue;
        }
        let fraction = water_counts[pixel] as f32 / valid_counts[pixel] as f32;
        persistence[pixel] = fraction;
        let class = if water_counts[pixel] == 0 {
            counts.never_water += 1;
            WaterSeasonalityClass::NeverWater
        } else if fraction >= PERMANENT_MIN_FRACTION {
            counts.permanent += 1;
            WaterSeasonalityClass::Permanent
        } else if fraction >= SEASONAL_MIN_FRACTION {
            counts.seasonal += 1;
            WaterSeasonalityClass::Seasonal
        } else {
            counts.ephemeral += 1;
            WaterSeasonalityClass::Ephemeral
        };
        classes[pixel] = class;
    }

    let pixel_area = request.gsd_m_per_px.map(|gsd| gsd * gsd);
    let water_area_series_m2 = pixel_area.map(|area| {
        area_series
            .iter()
            .map(|(date, pixels)| (*date, *pixels as f64 * area))
            .collect::<Vec<_>>()
    });
    let (min_a, mean_a, max_a) = match &water_area_series_m2 {
        Some(series) if !series.is_empty() => {
            let areas: Vec<f64> = series.iter().map(|(_, a)| *a).collect();
            let min = areas.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = areas.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let mean = areas.iter().sum::<f64>() / areas.len() as f64;
            (Some(min), Some(mean), Some(max))
        }
        _ => (None, None, None),
    };

    let period_start = request
        .observations
        .iter()
        .map(|o| o.observed_on)
        .min()
        .expect("non-empty");
    let period_end = request
        .observations
        .iter()
        .map(|o| o.observed_on)
        .max()
        .expect("non-empty");

    let input_product_ids: Vec<String> = request
        .observations
        .iter()
        .map(|o| o.product_id.clone())
        .collect();
    let input_hash = deterministic_fingerprint(&(
        "water_seasonality_v1",
        &input_product_ids,
        &valid_counts,
        &water_counts,
        request.min_observations,
        PERMANENT_MIN_FRACTION,
        SEASONAL_MIN_FRACTION,
        &request.spatial_ref,
    ))?;

    Ok(WaterSeasonalityResult {
        width: request.width,
        height: request.height,
        spatial_ref: request.spatial_ref.clone(),
        classes,
        persistence,
        counts: counts.clone(),
        permanent_area_m2: pixel_area.map(|a| f64::from(counts.permanent) * a),
        seasonal_area_m2: pixel_area.map(|a| f64::from(counts.seasonal) * a),
        water_area_series_m2,
        water_area_min_m2: min_a,
        water_area_mean_m2: mean_a,
        water_area_max_m2: max_a,
        observation_count: request.observations.len(),
        period_start,
        period_end,
        evidence: WaterSeasonalityEvidence {
            input_product_ids,
            permanent_min_fraction: PERMANENT_MIN_FRACTION,
            seasonal_min_fraction: SEASONAL_MIN_FRACTION,
            min_observations: request.min_observations,
            spatial_ref: request.spatial_ref.clone(),
            input_hash,
        },
    })
}

/// Scope a seasonality L3 draft cannot derive from pixels alone.
#[derive(Debug, Clone)]
pub struct WaterSeasonalityL3Scope {
    pub field_id: String,
    pub season_id: String,
    pub source_id: Option<String>,
}

/// Map a seasonality result to an L3 catalog draft (kind
/// `water_seasonality`), with lineage to every extent observation.
pub fn water_seasonality_l3_draft(
    result: &WaterSeasonalityResult,
    scope: &WaterSeasonalityL3Scope,
) -> ProductRecordDraft {
    to_l3_draft(&L3DraftContext {
        kind: "water_seasonality".to_string(),
        algorithm_id: "water.seasonality_persistence".to_string(),
        algorithm_version: "1.0.0".to_string(),
        field_id: scope.field_id.clone(),
        season_id: scope.season_id.clone(),
        scene_id: None,
        temporal_start: format!("{}T00:00:00Z", result.period_start),
        temporal_end: format!("{}T23:59:59Z", result.period_end),
        input_product_ids: result.evidence.input_product_ids.clone(),
        parameters: serde_json::json!({
            "permanent_min_fraction": result.evidence.permanent_min_fraction,
            "seasonal_min_fraction": result.evidence.seasonal_min_fraction,
            "min_observations": result.evidence.min_observations,
            "observation_count": result.observation_count,
            "counts": result.counts,
            "permanent_area_m2": result.permanent_area_m2,
            "seasonal_area_m2": result.seasonal_area_m2,
            "water_area_min_m2": result.water_area_min_m2,
            "water_area_mean_m2": result.water_area_mean_m2,
            "water_area_max_m2": result.water_area_max_m2,
            "class_codes": { "never_water": 0, "ephemeral": 1, "seasonal": 2, "permanent": 3 },
        }),
        confidence: Some(
            1.0 - f64::from(result.counts.insufficient) / f64::from(result.width * result.height),
        ),
        confidence_method: Some("classified_coverage_fraction".to_string()),
        evidence_digests: vec![result.evidence.input_hash.clone()],
        source_id: scope.source_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::{GeoBounds, RasterResolution};

    fn spatial_ref_2x2() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32643".to_string()),
            bbox: Some(GeoBounds {
                min_lon: 600000.0,
                min_lat: 1300000.0,
                max_lon: 600020.0,
                max_lat: 1300020.0,
            }),
            geo_transform: Some([600000.0, 10.0, 0.0, 1300020.0, 0.0, -10.0]),
            resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
        }
    }

    fn date(m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, d).unwrap()
    }

    fn observation(id: &str, on: NaiveDate, values: [f32; 4]) -> WaterExtentObservation {
        WaterExtentObservation {
            product_id: id.to_string(),
            observed_on: on,
            valid_mask: values.iter().map(|v| v.is_finite()).collect(),
            values: values.to_vec(),
        }
    }

    #[test]
    fn persistence_classes_are_hand_computed() {
        // Pixel 0: water 4/4 -> permanent. Pixel 1: water 2/4 (0.5) ->
        // seasonal. Pixel 2: water 0/4 -> never. Pixel 3: valid only twice
        // (nodata twice) with min_observations 3 -> insufficient.
        let request = WaterSeasonalityRequest {
            width: 2,
            height: 2,
            spatial_ref: spatial_ref_2x2(),
            observations: vec![
                observation("m1", date(1, 15), [1.0, 1.0, 0.0, 1.0]),
                observation("m2", date(4, 15), [1.0, 1.0, 0.0, f32::NAN]),
                observation("m3", date(7, 15), [1.0, 0.0, 0.0, 0.0]),
                observation("m4", date(10, 15), [1.0, 0.0, 0.0, f32::NAN]),
            ],
            min_observations: 3,
            gsd_m_per_px: Some(10.0),
        };
        let result = analyze_water_seasonality(&request).unwrap();
        assert_eq!(result.classes[0], WaterSeasonalityClass::Permanent);
        assert_eq!(result.classes[1], WaterSeasonalityClass::Seasonal);
        assert_eq!(result.classes[2], WaterSeasonalityClass::NeverWater);
        assert_eq!(result.classes[3], WaterSeasonalityClass::Insufficient);
        assert!((result.persistence[0] - 1.0).abs() < 1e-6);
        assert!((result.persistence[1] - 0.5).abs() < 1e-6);
        assert!(result.persistence[3].is_nan());
        assert_eq!(result.counts.permanent, 1);
        assert_eq!(result.counts.seasonal, 1);
        assert_eq!(result.counts.never_water, 1);
        assert_eq!(result.counts.insufficient, 1);

        // Areas: 100 m^2/pixel. Water pixels per obs: 3, 2, 1, 1 ->
        // 300/200/100/100 m^2; min 100, mean 175, max 300.
        let series = result.water_area_series_m2.as_ref().unwrap();
        let areas: Vec<f64> = series.iter().map(|(_, a)| *a).collect();
        assert_eq!(areas, vec![300.0, 200.0, 100.0, 100.0]);
        assert_eq!(result.water_area_min_m2, Some(100.0));
        assert_eq!(result.water_area_mean_m2, Some(175.0));
        assert_eq!(result.water_area_max_m2, Some(300.0));
        assert_eq!(result.permanent_area_m2, Some(100.0));
        assert_eq!(result.seasonal_area_m2, Some(100.0));
        assert_eq!(result.period_start, date(1, 15));
        assert_eq!(result.period_end, date(10, 15));
    }

    #[test]
    fn class_boundaries_are_pinned() {
        // 5 observations: 4/5 = 0.8 -> permanent (boundary inclusive);
        // 1/5 = 0.2 -> ephemeral (below the 0.25 seasonal floor);
        // 2/5 = 0.4 -> seasonal.
        let mk = |w0: f32, w1: f32, w2: f32| [w0, w1, w2, 0.0];
        let request = WaterSeasonalityRequest {
            width: 2,
            height: 2,
            spatial_ref: spatial_ref_2x2(),
            observations: vec![
                observation("o1", date(1, 1), mk(1.0, 1.0, 1.0)),
                observation("o2", date(2, 1), mk(1.0, 0.0, 1.0)),
                observation("o3", date(3, 1), mk(1.0, 0.0, 0.0)),
                observation("o4", date(4, 1), mk(1.0, 0.0, 0.0)),
                observation("o5", date(5, 1), mk(0.0, 0.0, 0.0)),
            ],
            min_observations: 3,
            gsd_m_per_px: None,
        };
        let result = analyze_water_seasonality(&request).unwrap();
        assert_eq!(result.classes[0], WaterSeasonalityClass::Permanent); // 0.8
        assert_eq!(result.classes[1], WaterSeasonalityClass::Ephemeral); // 0.2
        assert_eq!(result.classes[2], WaterSeasonalityClass::Seasonal); // 0.4
        assert!(result.water_area_series_m2.is_none(), "no GSD, no areas");
    }

    #[test]
    fn draft_carries_lineage_counts_and_identity() {
        let request = WaterSeasonalityRequest {
            width: 2,
            height: 2,
            spatial_ref: spatial_ref_2x2(),
            observations: vec![
                observation("w1", date(1, 1), [1.0, 0.0, 0.0, 0.0]),
                observation("w2", date(2, 1), [1.0, 0.0, 0.0, 0.0]),
                observation("w3", date(3, 1), [1.0, 0.0, 0.0, 0.0]),
            ],
            min_observations: 3,
            gsd_m_per_px: Some(10.0),
        };
        let result = analyze_water_seasonality(&request).unwrap();
        let draft = water_seasonality_l3_draft(
            &result,
            &WaterSeasonalityL3Scope {
                field_id: "field-1".to_string(),
                season_id: "2026".to_string(),
                source_id: None,
            },
        );
        assert_eq!(draft.kind, "water_seasonality");
        assert_eq!(draft.inputs.len(), 3);
        assert_eq!(draft.parameters["counts"]["permanent"], 1);
        assert_eq!(draft.parameters["class_codes"]["permanent"], 3);
        // Full coverage -> confidence 1.
        assert!((draft.confidence.unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn bad_inputs_are_typed_errors() {
        let request = WaterSeasonalityRequest {
            width: 2,
            height: 2,
            spatial_ref: spatial_ref_2x2(),
            observations: vec![],
            min_observations: 3,
            gsd_m_per_px: None,
        };
        assert!(matches!(
            analyze_water_seasonality(&request),
            Err(WaterSeasonalityError::NoObservations)
        ));

        let short = WaterSeasonalityRequest {
            width: 2,
            height: 2,
            spatial_ref: spatial_ref_2x2(),
            observations: vec![WaterExtentObservation {
                product_id: "short".to_string(),
                observed_on: date(1, 1),
                values: vec![1.0; 3],
                valid_mask: vec![true; 3],
            }],
            min_observations: 1,
            gsd_m_per_px: None,
        };
        assert!(matches!(
            analyze_water_seasonality(&short),
            Err(WaterSeasonalityError::LengthMismatch {
                expected: 4,
                actual: 3,
                ..
            })
        ));
    }
}
