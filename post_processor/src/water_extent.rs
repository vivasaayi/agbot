//! Water-body extraction from water-index and SAR-backscatter rasters
//! (satellite pipeline Phase 3 item 9 + batch 18 all-weather SAR).
//!
//! A water index (MNDWI, AWEI) separates water (high) from land (low); SAR
//! backscatter is the opposite (smooth water is specular and dark). Either
//! way the optimal cut shifts per scene, so this module picks the threshold
//! with **Otsu's method** (1979): maximize the between-class variance over a
//! 256-bin histogram spanning the data's own value range (bounded optical
//! ratios and unbounded SAR dB alike). A [`WaterExtentConfig`] carries the
//! polarity ([`WaterPolarity`]), a physical fallback threshold, and the
//! minimum class-mean separation (in the raster's value units) below which
//! Otsu is degenerate.
//!
//! Otsu assumes a bimodal histogram; when a scene is effectively unimodal —
//! either Otsu class carries < [`MIN_CLASS_FRACTION`] of the pixels
//! (all-land / all-water), or the class means sit closer than the config's
//! `min_mode_separation` (a cut through a narrow noise/speckle cluster) —
//! the method is reason-coded as degenerate and classification falls back to
//! the config's physical threshold instead of amplifying noise.
//!
//! Output is a binary water mask with per-pixel classes, the water fraction
//! and area (when the GSD is known), and evidence recording the polarity and
//! which method chose the threshold. JRC global-surface-water priors are
//! future work (they would gate Otsu flips against a long-term reference).

use crate::evidence::{deterministic_fingerprint, AnalysisEvidenceError};
use crate::l3_product::{to_l3_draft, L3DraftContext};
use serde::{Deserialize, Serialize};
use shared::product_graph::ProductRecordDraft;
use shared::schemas::{assert_raster_spatial_ref, RasterSpatialRef, RasterSpatialRefError};
use thiserror::Error;

/// Histogram resolution for Otsu.
pub const HISTOGRAM_BINS: usize = 256;
/// Physical fallback for optical indices: index above zero reads as water.
pub const FIXED_WATER_THRESHOLD: f32 = 0.0;
/// Otsu is trusted only when both classes hold at least this fraction of
/// the valid pixels — below it the histogram is effectively unimodal.
pub const MIN_CLASS_FRACTION: f64 = 0.01;
/// Default minimum class-mean separation for optical indices (ratios in
/// [-1, 1]). An Otsu cut through a narrow noise cluster separates means by
/// far less than any real land/water contrast.
pub const DEFAULT_OPTICAL_MODE_SEPARATION: f32 = 0.1;
/// Default minimum separation for SAR backscatter (dB): water and land
/// backscatter differ by ~10 dB, in-class speckle by ~1-2 dB.
pub const DEFAULT_SAR_MODE_SEPARATION: f32 = 3.0;
/// Physical SAR water fallback (VV sigma0, dB): smooth water is specular
/// and dark, so backscatter BELOW this reads as water.
pub const FIXED_SAR_WATER_THRESHOLD: f32 = -15.0;

/// Which side of the threshold is water: high values (optical water indices,
/// where water > land) or low values (SAR backscatter, where smooth water
/// is dark).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaterPolarity {
    /// Water is the high-value class (MNDWI, NDWI, AWEI).
    HighValueIsWater,
    /// Water is the low-value class (SAR VV/VH backscatter).
    LowValueIsWater,
}

/// Extraction configuration: polarity, the physical fallback threshold, and
/// the minimum class-mean separation (in the raster's own value units) below
/// which Otsu is treated as degenerate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WaterExtentConfig {
    pub polarity: WaterPolarity,
    pub fixed_threshold: f32,
    pub min_mode_separation: f32,
}

impl Default for WaterExtentConfig {
    fn default() -> Self {
        Self::optical()
    }
}

impl WaterExtentConfig {
    /// Optical water indices (MNDWI/NDWI/AWEI): high = water, fallback at 0.
    pub fn optical() -> Self {
        Self {
            polarity: WaterPolarity::HighValueIsWater,
            fixed_threshold: FIXED_WATER_THRESHOLD,
            min_mode_separation: DEFAULT_OPTICAL_MODE_SEPARATION,
        }
    }

    /// SAR backscatter (dB): low = water, fallback at -15 dB.
    pub fn sar_backscatter() -> Self {
        Self {
            polarity: WaterPolarity::LowValueIsWater,
            fixed_threshold: FIXED_SAR_WATER_THRESHOLD,
            min_mode_separation: DEFAULT_SAR_MODE_SEPARATION,
        }
    }
}

/// How the threshold was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdMethod {
    Otsu,
    /// Otsu was degenerate (unimodal histogram); the fixed physical
    /// threshold was used instead.
    FixedFallback,
}

/// Per-pixel classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaterClass {
    Water,
    Land,
    Invalid,
}

/// The input water-index raster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterIndexRaster {
    /// Catalog product id (identity-bearing; becomes L3 lineage).
    pub product_id: String,
    /// Index kind, e.g. `mndwi` / `aweinsh` (recorded in evidence).
    pub index_kind: String,
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    pub values: Vec<f32>,
    pub valid_mask: Vec<bool>,
    /// Pixel edge length in meters, for area accounting.
    pub gsd_m_per_px: Option<f64>,
}

/// Evidence object for one extraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterExtentEvidence {
    pub input_product_id: String,
    pub index_kind: String,
    pub method: ThresholdMethod,
    pub threshold: f32,
    /// Otsu's threshold when it was computed (also set on fallback, for
    /// audit: what Otsu WOULD have picked).
    pub otsu_threshold: Option<f32>,
    pub histogram_bins: usize,
    pub min_class_fraction: f64,
    pub polarity: WaterPolarity,
    pub spatial_ref: RasterSpatialRef,
    pub input_hash: String,
}

/// A completed water-extent mask.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterExtentResult {
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    pub classes: Vec<WaterClass>,
    pub water_pixels: u32,
    pub land_pixels: u32,
    pub invalid_pixels: u32,
    /// Water share of the valid pixels.
    pub water_fraction: f32,
    /// Water area in square meters when the GSD is known.
    pub water_area_m2: Option<f64>,
    pub valid_fraction: f32,
    pub evidence: WaterExtentEvidence,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum WaterExtentError {
    #[error("raster has {actual} values, expected {expected}")]
    LengthMismatch { expected: usize, actual: usize },
    #[error("validity mask has {actual} pixels, expected {expected}")]
    MaskMismatch { expected: usize, actual: usize },
    #[error("raster has no valid pixels to threshold")]
    NoValidPixels,
    #[error("grid spatial metadata is invalid: {reason}")]
    SpatialRef { reason: RasterSpatialRefError },
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] AnalysisEvidenceError),
}

/// Value at a fractional bin position within `[value_min, value_max]`.
/// Otsu cuts *between* bins, so the threshold is taken at `best_bin + 0.5`
/// — strictly above the low mode and below the high mode even when the two
/// modes sit on the histogram's extreme bins.
fn bin_boundary(bin_position: f64, value_min: f32, value_max: f32) -> f32 {
    let t = bin_position / (HISTOGRAM_BINS - 1) as f64;
    (f64::from(value_min) + t * f64::from(value_max - value_min)) as f32
}

/// Otsu outcome: the chosen cut plus the quality signals the caller uses
/// to decide whether to trust it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OtsuOutcome {
    /// Threshold in the raster's value units.
    pub threshold: f32,
    /// Weight fraction above the cut.
    pub high_weight: f64,
    /// Distance between the class means, in value units.
    pub mode_separation: f32,
}

/// Otsu's threshold over a histogram binned across `[value_min, value_max]`:
/// the bin cut maximizing between-class variance (first maximum wins ties,
/// deterministically). Thresholds are returned in value units.
pub fn otsu_threshold(
    histogram: &[u64; HISTOGRAM_BINS],
    value_min: f32,
    value_max: f32,
) -> Option<OtsuOutcome> {
    let total: u64 = histogram.iter().sum();
    if total == 0 {
        return None;
    }
    let total_f = total as f64;
    let weighted_sum: f64 = histogram
        .iter()
        .enumerate()
        .map(|(bin, count)| bin as f64 * *count as f64)
        .sum();

    let mut background_count = 0f64;
    let mut background_sum = 0f64;
    let mut best_variance = -1.0f64;
    let mut best_bin = 0usize;
    // Threshold after bin `t`: background = bins 0..=t, foreground = rest.
    for (bin, count) in histogram.iter().enumerate().take(HISTOGRAM_BINS - 1) {
        background_count += *count as f64;
        background_sum += bin as f64 * *count as f64;
        let foreground_count = total_f - background_count;
        if background_count == 0.0 || foreground_count == 0.0 {
            continue;
        }
        let mean_background = background_sum / background_count;
        let mean_foreground = (weighted_sum - background_sum) / foreground_count;
        let variance = (background_count / total_f)
            * (foreground_count / total_f)
            * (mean_background - mean_foreground).powi(2);
        if variance > best_variance {
            best_variance = variance;
            best_bin = bin;
        }
    }
    if best_variance <= 0.0 {
        return None;
    }
    // Class weights and mean separation at the chosen cut.
    let foreground: u64 = histogram[best_bin + 1..].iter().sum();
    let background = total - foreground;
    let background_sum: f64 = histogram[..=best_bin]
        .iter()
        .enumerate()
        .map(|(bin, count)| bin as f64 * *count as f64)
        .sum();
    let mean_background_bin = background_sum / background as f64;
    let mean_foreground_bin = (weighted_sum - background_sum) / foreground as f64;
    let bin_width = f64::from(value_max - value_min) / (HISTOGRAM_BINS - 1) as f64;
    Some(OtsuOutcome {
        threshold: bin_boundary(best_bin as f64 + 0.5, value_min, value_max),
        high_weight: foreground as f64 / total_f,
        mode_separation: ((mean_foreground_bin - mean_background_bin) * bin_width) as f32,
    })
}

/// Extract a water mask from a water-index / backscatter raster: Otsu
/// threshold over the data's own value range, with a polarity-aware
/// reason-coded fixed fallback on degenerate (unimodal) histograms.
pub fn extract_water_extent(
    raster: &WaterIndexRaster,
    config: &WaterExtentConfig,
) -> Result<WaterExtentResult, WaterExtentError> {
    assert_raster_spatial_ref(Some(&raster.spatial_ref), raster.width, raster.height)
        .map_err(|reason| WaterExtentError::SpatialRef { reason })?;
    let pixel_count = raster.width as usize * raster.height as usize;
    if raster.values.len() != pixel_count {
        return Err(WaterExtentError::LengthMismatch {
            expected: pixel_count,
            actual: raster.values.len(),
        });
    }
    if raster.valid_mask.len() != pixel_count {
        return Err(WaterExtentError::MaskMismatch {
            expected: pixel_count,
            actual: raster.valid_mask.len(),
        });
    }

    // Data-driven histogram range over the valid values (works for bounded
    // optical ratios and unbounded SAR dB alike).
    let mut value_min = f32::MAX;
    let mut value_max = f32::MIN;
    let mut valid = 0u32;
    for pixel in 0..pixel_count {
        let value = raster.values[pixel];
        if raster.valid_mask[pixel] && value.is_finite() {
            value_min = value_min.min(value);
            value_max = value_max.max(value);
            valid += 1;
        }
    }
    if valid == 0 {
        return Err(WaterExtentError::NoValidPixels);
    }

    let (otsu, threshold, method) = if value_max <= value_min {
        // Single value: no cut possible, fall back.
        (None, config.fixed_threshold, ThresholdMethod::FixedFallback)
    } else {
        let bin_of = |value: f32| {
            let t = f64::from(
                (value.clamp(value_min, value_max) - value_min) / (value_max - value_min),
            );
            ((t * (HISTOGRAM_BINS - 1) as f64).round()) as usize
        };
        let mut histogram = [0u64; HISTOGRAM_BINS];
        for pixel in 0..pixel_count {
            let value = raster.values[pixel];
            if raster.valid_mask[pixel] && value.is_finite() {
                histogram[bin_of(value)] += 1;
            }
        }
        let otsu = otsu_threshold(&histogram, value_min, value_max);
        match otsu {
            Some(outcome)
                if outcome.high_weight >= MIN_CLASS_FRACTION
                    && outcome.high_weight <= 1.0 - MIN_CLASS_FRACTION
                    && outcome.mode_separation.abs() >= config.min_mode_separation =>
            {
                (otsu, outcome.threshold, ThresholdMethod::Otsu)
            }
            _ => (otsu, config.fixed_threshold, ThresholdMethod::FixedFallback),
        }
    };

    let is_water = |value: f32| match config.polarity {
        WaterPolarity::HighValueIsWater => value > threshold,
        WaterPolarity::LowValueIsWater => value < threshold,
    };
    let mut classes = vec![WaterClass::Invalid; pixel_count];
    let mut water_pixels = 0u32;
    let mut land_pixels = 0u32;
    for (pixel, class) in classes.iter_mut().enumerate() {
        let value = raster.values[pixel];
        if !raster.valid_mask[pixel] || !value.is_finite() {
            continue;
        }
        if is_water(value) {
            *class = WaterClass::Water;
            water_pixels += 1;
        } else {
            *class = WaterClass::Land;
            land_pixels += 1;
        }
    }

    let input_hash = deterministic_fingerprint(&(
        "water_extent_v2",
        &raster.product_id,
        &raster.index_kind,
        &raster.values,
        &raster.valid_mask,
        &raster.spatial_ref,
        config,
    ))?;

    Ok(WaterExtentResult {
        width: raster.width,
        height: raster.height,
        spatial_ref: raster.spatial_ref.clone(),
        classes,
        water_pixels,
        land_pixels,
        invalid_pixels: pixel_count as u32 - valid,
        water_fraction: water_pixels as f32 / valid as f32,
        water_area_m2: raster
            .gsd_m_per_px
            .map(|gsd| f64::from(water_pixels) * gsd * gsd),
        valid_fraction: valid as f32 / pixel_count as f32,
        evidence: WaterExtentEvidence {
            input_product_id: raster.product_id.clone(),
            index_kind: raster.index_kind.clone(),
            method,
            threshold,
            otsu_threshold: otsu.map(|outcome| outcome.threshold),
            histogram_bins: HISTOGRAM_BINS,
            min_class_fraction: MIN_CLASS_FRACTION,
            polarity: config.polarity,
            spatial_ref: raster.spatial_ref.clone(),
            input_hash,
        },
    })
}

// ---------------------------------------------------------------------------
// L3 lineage
// ---------------------------------------------------------------------------

/// Scope a water-extent L3 draft cannot derive from pixel data alone.
#[derive(Debug, Clone)]
pub struct WaterExtentL3Scope {
    pub field_id: String,
    pub season_id: String,
    pub scene_id: Option<String>,
    pub temporal_start: String,
    pub temporal_end: String,
    pub source_id: Option<String>,
}

/// Map a water-extent result to an L3 catalog draft (kind `water_extent`).
pub fn water_extent_l3_draft(
    result: &WaterExtentResult,
    scope: &WaterExtentL3Scope,
) -> ProductRecordDraft {
    to_l3_draft(&L3DraftContext {
        kind: "water_extent".to_string(),
        algorithm_id: "water.extent_otsu".to_string(),
        algorithm_version: "1.0.0".to_string(),
        field_id: scope.field_id.clone(),
        season_id: scope.season_id.clone(),
        scene_id: scope.scene_id.clone(),
        temporal_start: scope.temporal_start.clone(),
        temporal_end: scope.temporal_end.clone(),
        input_product_ids: vec![result.evidence.input_product_id.clone()],
        parameters: serde_json::json!({
            "index_kind": result.evidence.index_kind,
            "method": result.evidence.method,
            "polarity": result.evidence.polarity,
            "threshold": result.evidence.threshold,
            "otsu_threshold": result.evidence.otsu_threshold,
            "water_pixels": result.water_pixels,
            "water_fraction": result.water_fraction,
            "water_area_m2": result.water_area_m2,
            "mask_codes": { "land": 0, "water": 1 },
        }),
        confidence: Some(f64::from(result.valid_fraction)),
        confidence_method: Some("valid_coverage_fraction".to_string()),
        evidence_digests: vec![result.evidence.input_hash.clone()],
        source_id: scope.source_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::{GeoBounds, RasterResolution};

    fn spatial_ref_4x4() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32643".to_string()),
            bbox: Some(GeoBounds {
                min_lon: 600000.0,
                min_lat: 1299980.0,
                max_lon: 600040.0,
                max_lat: 1300020.0,
            }),
            geo_transform: Some([600000.0, 10.0, 0.0, 1300020.0, 0.0, -10.0]),
            resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
        }
    }

    fn raster(values: Vec<f32>, valid_mask: Vec<bool>) -> WaterIndexRaster {
        WaterIndexRaster {
            product_id: "mndwi-1".to_string(),
            index_kind: "mndwi".to_string(),
            width: 4,
            height: 4,
            spatial_ref: spatial_ref_4x4(),
            values,
            valid_mask,
            gsd_m_per_px: Some(10.0),
        }
    }

    #[test]
    fn otsu_splits_a_bimodal_lake_scene() {
        // 8 water pixels at +0.5, 7 land at -0.5, 1 nodata: Otsu must cut
        // strictly between the modes, classifying exactly 8 water pixels.
        let mut values = vec![-0.5f32; 16];
        for pixel in 0..8 {
            values[pixel] = 0.5;
        }
        let mut mask = vec![true; 16];
        mask[15] = false;
        let result =
            extract_water_extent(&raster(values, mask), &WaterExtentConfig::optical()).unwrap();

        assert_eq!(result.evidence.method, ThresholdMethod::Otsu);
        let threshold = result.evidence.threshold;
        assert!(
            threshold > -0.5 && threshold < 0.5,
            "threshold {threshold} must fall between the modes"
        );
        assert_eq!(result.water_pixels, 8);
        assert_eq!(result.land_pixels, 7);
        assert_eq!(result.invalid_pixels, 1);
        assert!((result.water_fraction - 8.0 / 15.0).abs() < 1e-6);
        // 8 pixels x (10 m)^2.
        assert_eq!(result.water_area_m2, Some(800.0));
        assert_eq!(result.classes[0], WaterClass::Water);
        assert_eq!(result.classes[8], WaterClass::Land);
        assert_eq!(result.classes[15], WaterClass::Invalid);
    }

    #[test]
    fn unimodal_scene_falls_back_to_the_fixed_threshold() {
        // All-land scene with tiny noise: Otsu would split the noise; the
        // class-fraction guard must reason-code it and use index > 0.
        let values: Vec<f32> = (0..16).map(|i| -0.4 + 0.001 * i as f32).collect();
        let result = extract_water_extent(
            &raster(values, vec![true; 16]),
            &WaterExtentConfig::optical(),
        )
        .unwrap();
        assert_eq!(result.evidence.method, ThresholdMethod::FixedFallback);
        assert_eq!(result.evidence.threshold, FIXED_WATER_THRESHOLD);
        assert_eq!(result.water_pixels, 0);
        assert_eq!(result.land_pixels, 16);
        // The audit trail still records what Otsu would have picked.
        assert!(result.evidence.otsu_threshold.is_some());
    }

    #[test]
    fn all_water_scene_also_falls_back() {
        let result = extract_water_extent(
            &raster(vec![0.6; 16], vec![true; 16]),
            &WaterExtentConfig::optical(),
        )
        .unwrap();
        assert_eq!(result.evidence.method, ThresholdMethod::FixedFallback);
        assert_eq!(result.water_pixels, 16);
        assert_eq!(result.water_fraction, 1.0);
    }

    #[test]
    fn no_valid_pixels_is_an_error() {
        assert!(matches!(
            extract_water_extent(
                &raster(vec![0.5; 16], vec![false; 16]),
                &WaterExtentConfig::optical()
            ),
            Err(WaterExtentError::NoValidPixels)
        ));
    }

    #[test]
    fn draft_carries_kind_lineage_and_identity() {
        let mut values = vec![-0.5f32; 16];
        for pixel in 0..8 {
            values[pixel] = 0.5;
        }
        let result = extract_water_extent(
            &raster(values, vec![true; 16]),
            &WaterExtentConfig::optical(),
        )
        .unwrap();
        let draft = water_extent_l3_draft(
            &result,
            &WaterExtentL3Scope {
                field_id: "field-1".to_string(),
                season_id: "season-2026".to_string(),
                scene_id: None,
                temporal_start: "2026-06-01T00:00:00Z".to_string(),
                temporal_end: "2026-06-01T23:59:59Z".to_string(),
                source_id: None,
            },
        );
        assert_eq!(draft.kind, "water_extent");
        assert_eq!(draft.inputs.len(), 1);
        assert_eq!(draft.inputs[0].product_id, "mndwi-1");
        assert_eq!(draft.parameters["water_pixels"], 8);
        assert_eq!(
            draft.evidence_digests,
            vec![result.evidence.input_hash.clone()]
        );
    }

    #[test]
    fn sar_low_backscatter_is_classified_as_water() {
        // SAR VV sigma0 (dB): smooth water is dark (-20), land is bright
        // (-6). 8 water + 7 land + 1 nodata; low-value polarity classifies
        // the dark pixels as water, and the fallback/threshold live in dB.
        let mut values = vec![-6.0f32; 16];
        for pixel in 0..8 {
            values[pixel] = -20.0;
        }
        let mut mask = vec![true; 16];
        mask[15] = false;
        let mut sar = raster(values, mask);
        sar.index_kind = "sar_vv".to_string();
        let result = extract_water_extent(&sar, &WaterExtentConfig::sar_backscatter()).unwrap();

        assert_eq!(result.evidence.method, ThresholdMethod::Otsu);
        assert_eq!(result.evidence.polarity, WaterPolarity::LowValueIsWater);
        let threshold = result.evidence.threshold;
        assert!(
            threshold > -20.0 && threshold < -6.0,
            "dB threshold {threshold}"
        );
        // The DARK pixels (0..8) are water under low-value polarity.
        assert_eq!(result.water_pixels, 8);
        assert_eq!(result.classes[0], WaterClass::Water);
        assert_eq!(result.classes[8], WaterClass::Land);

        // Uniform-bright SAR noise (~1 dB spread, << 3 dB SAR separation)
        // must fall back to the physical -15 dB threshold, not split speckle.
        let noise: Vec<f32> = (0..16).map(|i| -6.0 + 0.05 * i as f32).collect();
        let mut sar_noise = raster(noise, vec![true; 16]);
        sar_noise.index_kind = "sar_vv".to_string();
        let result =
            extract_water_extent(&sar_noise, &WaterExtentConfig::sar_backscatter()).unwrap();
        assert_eq!(result.evidence.method, ThresholdMethod::FixedFallback);
        assert_eq!(result.evidence.threshold, FIXED_SAR_WATER_THRESHOLD);
        // All bright (~-6 dB) -> all land under the -15 dB fallback.
        assert_eq!(result.water_pixels, 0);
    }
}
