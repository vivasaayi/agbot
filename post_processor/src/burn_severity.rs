//! dNBR burn-severity / forest-disturbance rasters (satellite pipeline
//! batch 14, Phase 4).
//!
//! `dNBR = NBR_prefire − NBR_postfire` per pixel: positive values mean lost
//! vegetation/char (burn, clear-cut), negative values mean regrowth.
//! Severity classes follow Key & Benson (2006, FIREMON landscape
//! assessment) on unscaled dNBR: regrowth < −0.1, unburned within ±0.1,
//! then low / moderate-low / moderate-high / high severity at
//! 0.1 / 0.27 / 0.44 / 0.66. Deterministic and per-pixel, mirroring
//! `drought_indices`: the caller (geo_hub) supplies two same-grid NBR
//! rasters; no resampling here.
//!
//! "Forest loss" is a reading of the output, not a separate algorithm: the
//! disturbed fraction (dNBR >= 0.1) over a forest mask is the loss signal.
//! Masking by land-cover class (batch 10 rasters) happens downstream.

use crate::evidence::{deterministic_fingerprint, AnalysisEvidenceError};
use crate::l3_product::{to_l3_draft, L3DraftContext};
use serde::{Deserialize, Serialize};
use shared::product_graph::ProductRecordDraft;
use shared::schemas::{assert_raster_spatial_ref, RasterSpatialRef, RasterSpatialRefError};
use thiserror::Error;

/// Sentinel stored where a pixel has no dNBR. Consumers must consult
/// `reason_codes`, never trust the sentinel.
pub const DNBR_SENTINEL: f32 = f32::NAN;

/// Key & Benson class boundaries on unscaled dNBR.
pub const REGROWTH_HIGH_MAX: f32 = -0.25;
pub const REGROWTH_LOW_MAX: f32 = -0.10;
pub const UNBURNED_MAX: f32 = 0.10;
pub const LOW_SEVERITY_MAX: f32 = 0.27;
pub const MODERATE_LOW_MAX: f32 = 0.44;
pub const MODERATE_HIGH_MAX: f32 = 0.66;

/// Per-pixel outcome code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnbrPixelReason {
    Computed,
    /// The pre-fire raster has no usable value here.
    MissingPre,
    /// The post-fire raster has no usable value here.
    MissingPost,
}

/// Key & Benson (2006) burn-severity classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BurnSeverity {
    EnhancedRegrowthHigh,
    EnhancedRegrowthLow,
    Unburned,
    LowSeverity,
    ModerateLowSeverity,
    ModerateHighSeverity,
    HighSeverity,
    Invalid,
}

/// Classify an unscaled dNBR value.
pub fn classify_dnbr(value: f32) -> BurnSeverity {
    if !value.is_finite() {
        BurnSeverity::Invalid
    } else if value < REGROWTH_HIGH_MAX {
        BurnSeverity::EnhancedRegrowthHigh
    } else if value < REGROWTH_LOW_MAX {
        BurnSeverity::EnhancedRegrowthLow
    } else if value < UNBURNED_MAX {
        BurnSeverity::Unburned
    } else if value < LOW_SEVERITY_MAX {
        BurnSeverity::LowSeverity
    } else if value < MODERATE_LOW_MAX {
        BurnSeverity::ModerateLowSeverity
    } else if value < MODERATE_HIGH_MAX {
        BurnSeverity::ModerateHighSeverity
    } else {
        BurnSeverity::HighSeverity
    }
}

/// Per-class pixel counts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BurnSeverityCounts {
    pub enhanced_regrowth_high: u32,
    pub enhanced_regrowth_low: u32,
    pub unburned: u32,
    pub low_severity: u32,
    pub moderate_low_severity: u32,
    pub moderate_high_severity: u32,
    pub high_severity: u32,
    pub invalid: u32,
}

impl BurnSeverityCounts {
    fn add(&mut self, class: BurnSeverity) {
        match class {
            BurnSeverity::EnhancedRegrowthHigh => self.enhanced_regrowth_high += 1,
            BurnSeverity::EnhancedRegrowthLow => self.enhanced_regrowth_low += 1,
            BurnSeverity::Unburned => self.unburned += 1,
            BurnSeverity::LowSeverity => self.low_severity += 1,
            BurnSeverity::ModerateLowSeverity => self.moderate_low_severity += 1,
            BurnSeverity::ModerateHighSeverity => self.moderate_high_severity += 1,
            BurnSeverity::HighSeverity => self.high_severity += 1,
            BurnSeverity::Invalid => self.invalid += 1,
        }
    }
}

/// One NBR raster entering the difference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NbrRaster {
    /// Catalog product id (identity-bearing; becomes L3 lineage).
    pub product_id: String,
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    pub values: Vec<f32>,
    pub valid_mask: Vec<bool>,
}

/// Evidence object for one dNBR run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DnbrEvidence {
    pub pre_product_id: String,
    pub post_product_id: String,
    pub spatial_ref: RasterSpatialRef,
    pub class_convention: String,
    /// Deterministic canonical-JSON FNV fingerprint over the full input.
    pub input_hash: String,
}

/// A completed dNBR raster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DnbrResult {
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    /// dNBR values; [`DNBR_SENTINEL`] where invalid.
    pub values: Vec<f32>,
    pub reason_codes: Vec<DnbrPixelReason>,
    pub classes: Vec<BurnSeverity>,
    pub class_counts: BurnSeverityCounts,
    /// Fraction of valid pixels with dNBR >= 0.1 (low severity or worse) —
    /// the disturbance/forest-loss signal.
    pub disturbed_fraction: f32,
    pub valid_fraction: f32,
    pub evidence: DnbrEvidence,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum DnbrError {
    #[error("{which} raster has {actual} values, expected {expected}")]
    LengthMismatch {
        which: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{which} validity mask has {actual} pixels, expected {expected}")]
    MaskMismatch {
        which: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("pre and post rasters are not on the same grid (no resampling here)")]
    GridMismatch,
    #[error("grid spatial metadata is invalid: {reason}")]
    SpatialRef { reason: RasterSpatialRefError },
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] AnalysisEvidenceError),
}

fn validate_raster(raster: &NbrRaster, which: &'static str) -> Result<usize, DnbrError> {
    assert_raster_spatial_ref(Some(&raster.spatial_ref), raster.width, raster.height)
        .map_err(|reason| DnbrError::SpatialRef { reason })?;
    let pixel_count = raster.width as usize * raster.height as usize;
    if raster.values.len() != pixel_count {
        return Err(DnbrError::LengthMismatch {
            which,
            expected: pixel_count,
            actual: raster.values.len(),
        });
    }
    if raster.valid_mask.len() != pixel_count {
        return Err(DnbrError::MaskMismatch {
            which,
            expected: pixel_count,
            actual: raster.valid_mask.len(),
        });
    }
    Ok(pixel_count)
}

/// Compute a dNBR raster from pre- and post-event NBR rasters on one grid.
pub fn compute_dnbr(pre: &NbrRaster, post: &NbrRaster) -> Result<DnbrResult, DnbrError> {
    let pixel_count = validate_raster(pre, "pre")?;
    validate_raster(post, "post")?;
    if pre.spatial_ref != post.spatial_ref || (pre.width, pre.height) != (post.width, post.height) {
        return Err(DnbrError::GridMismatch);
    }

    let mut values = vec![DNBR_SENTINEL; pixel_count];
    let mut reason_codes = vec![DnbrPixelReason::Computed; pixel_count];
    let mut classes = vec![BurnSeverity::Invalid; pixel_count];
    let mut class_counts = BurnSeverityCounts::default();
    let mut valid = 0u32;
    let mut disturbed = 0u32;
    for pixel in 0..pixel_count {
        let pre_value = pre.values[pixel];
        let post_value = post.values[pixel];
        if !pre.valid_mask[pixel] || !pre_value.is_finite() {
            reason_codes[pixel] = DnbrPixelReason::MissingPre;
        } else if !post.valid_mask[pixel] || !post_value.is_finite() {
            reason_codes[pixel] = DnbrPixelReason::MissingPost;
        } else {
            let dnbr = pre_value - post_value;
            values[pixel] = dnbr;
            classes[pixel] = classify_dnbr(dnbr);
            valid += 1;
            if dnbr >= UNBURNED_MAX {
                disturbed += 1;
            }
        }
        class_counts.add(classes[pixel]);
    }

    let input_hash = deterministic_fingerprint(&(
        "dnbr_v1",
        &pre.product_id,
        &post.product_id,
        &pre.values,
        &pre.valid_mask,
        &post.values,
        &post.valid_mask,
        &pre.spatial_ref,
    ))?;

    Ok(DnbrResult {
        width: pre.width,
        height: pre.height,
        spatial_ref: pre.spatial_ref.clone(),
        values,
        reason_codes,
        classes,
        class_counts,
        disturbed_fraction: if valid == 0 {
            0.0
        } else {
            disturbed as f32 / valid as f32
        },
        valid_fraction: valid as f32 / pixel_count as f32,
        evidence: DnbrEvidence {
            pre_product_id: pre.product_id.clone(),
            post_product_id: post.product_id.clone(),
            spatial_ref: pre.spatial_ref.clone(),
            class_convention: "key_benson_2006_unscaled".to_string(),
            input_hash,
        },
    })
}

// ---------------------------------------------------------------------------
// L3 lineage
// ---------------------------------------------------------------------------

/// Scope a dNBR L3 draft cannot derive from pixel data alone.
#[derive(Debug, Clone)]
pub struct DnbrL3Scope {
    pub field_id: String,
    pub season_id: String,
    pub scene_id: Option<String>,
    /// Pre-event start .. post-event end.
    pub temporal_start: String,
    pub temporal_end: String,
    pub source_id: Option<String>,
}

/// Map a dNBR result to an L3 catalog draft (kind `dnbr`). Lineage = pre +
/// post NBR products.
pub fn dnbr_l3_draft(result: &DnbrResult, scope: &DnbrL3Scope) -> ProductRecordDraft {
    to_l3_draft(&L3DraftContext {
        kind: "dnbr".to_string(),
        algorithm_id: "change.dnbr".to_string(),
        algorithm_version: "1.0.0".to_string(),
        field_id: scope.field_id.clone(),
        season_id: scope.season_id.clone(),
        scene_id: scope.scene_id.clone(),
        temporal_start: scope.temporal_start.clone(),
        temporal_end: scope.temporal_end.clone(),
        input_product_ids: vec![
            result.evidence.pre_product_id.clone(),
            result.evidence.post_product_id.clone(),
        ],
        parameters: serde_json::json!({
            "index_kind": "dnbr",
            "direction": "pre_minus_post",
            "class_convention": result.evidence.class_convention,
            "class_bounds": [REGROWTH_HIGH_MAX, REGROWTH_LOW_MAX, UNBURNED_MAX,
                             LOW_SEVERITY_MAX, MODERATE_LOW_MAX, MODERATE_HIGH_MAX],
            "pre_product_id": result.evidence.pre_product_id,
            "post_product_id": result.evidence.post_product_id,
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

    fn raster(id: &str, values: Vec<f32>, valid_mask: Vec<bool>) -> NbrRaster {
        NbrRaster {
            product_id: id.to_string(),
            width: 2,
            height: 2,
            spatial_ref: spatial_ref_2x2(),
            values,
            valid_mask,
        }
    }

    #[test]
    fn dnbr_is_pre_minus_post_with_hand_computed_classes() {
        // Pixel 0: 0.5 -> 0.5  = 0.0   unburned
        // Pixel 1: 0.6 -> 0.1  = 0.5   moderate-high severity
        // Pixel 2: 0.2 -> 0.5  = -0.3  enhanced regrowth (high)
        // Pixel 3: pre invalid          missing_pre
        let pre = raster(
            "pre",
            vec![0.5, 0.6, 0.2, 0.4],
            vec![true, true, true, false],
        );
        let post = raster("post", vec![0.5, 0.1, 0.5, 0.4], vec![true; 4]);
        let result = compute_dnbr(&pre, &post).unwrap();

        assert!(result.values[0].abs() < 1e-6);
        assert!((result.values[1] - 0.5).abs() < 1e-6);
        assert!((result.values[2] + 0.3).abs() < 1e-6);
        assert!(result.values[3].is_nan());
        assert_eq!(result.classes[0], BurnSeverity::Unburned);
        assert_eq!(result.classes[1], BurnSeverity::ModerateHighSeverity);
        assert_eq!(result.classes[2], BurnSeverity::EnhancedRegrowthHigh);
        assert_eq!(result.classes[3], BurnSeverity::Invalid);
        assert_eq!(result.reason_codes[3], DnbrPixelReason::MissingPre);
        assert_eq!(result.class_counts.unburned, 1);
        assert_eq!(result.class_counts.moderate_high_severity, 1);
        assert_eq!(result.class_counts.enhanced_regrowth_high, 1);
        assert_eq!(result.class_counts.invalid, 1);
        // One disturbed pixel of three valid.
        assert!((result.disturbed_fraction - 1.0 / 3.0).abs() < 1e-6);
        assert!((result.valid_fraction - 0.75).abs() < 1e-6);
    }

    #[test]
    fn key_benson_boundaries_are_pinned() {
        assert_eq!(classify_dnbr(-0.26), BurnSeverity::EnhancedRegrowthHigh);
        assert_eq!(classify_dnbr(-0.25), BurnSeverity::EnhancedRegrowthLow);
        assert_eq!(classify_dnbr(-0.10), BurnSeverity::Unburned);
        assert_eq!(classify_dnbr(0.0), BurnSeverity::Unburned);
        assert_eq!(classify_dnbr(0.10), BurnSeverity::LowSeverity);
        assert_eq!(classify_dnbr(0.27), BurnSeverity::ModerateLowSeverity);
        assert_eq!(classify_dnbr(0.44), BurnSeverity::ModerateHighSeverity);
        assert_eq!(classify_dnbr(0.66), BurnSeverity::HighSeverity);
        assert_eq!(classify_dnbr(1.2), BurnSeverity::HighSeverity);
        assert_eq!(classify_dnbr(f32::NAN), BurnSeverity::Invalid);
    }

    #[test]
    fn missing_post_and_grid_mismatch_are_reason_coded() {
        let pre = raster("pre", vec![0.5; 4], vec![true; 4]);
        let mut post = raster("post", vec![0.1; 4], vec![true; 4]);
        post.valid_mask[2] = false;
        let result = compute_dnbr(&pre, &post).unwrap();
        assert_eq!(result.reason_codes[2], DnbrPixelReason::MissingPost);

        let mut shifted = raster("post", vec![0.1; 4], vec![true; 4]);
        shifted.spatial_ref.geo_transform = Some([601000.0, 10.0, 0.0, 1300020.0, 0.0, -10.0]);
        shifted.spatial_ref.bbox = Some(GeoBounds {
            min_lon: 601000.0,
            min_lat: 1300000.0,
            max_lon: 601020.0,
            max_lat: 1300020.0,
        });
        assert!(matches!(
            compute_dnbr(&pre, &shifted),
            Err(DnbrError::GridMismatch)
        ));
    }

    #[test]
    fn draft_carries_kind_lineage_and_identity() {
        let pre = raster("pre-nbr", vec![0.5; 4], vec![true; 4]);
        let post = raster("post-nbr", vec![0.1; 4], vec![true; 4]);
        let result = compute_dnbr(&pre, &post).unwrap();
        let draft = dnbr_l3_draft(
            &result,
            &DnbrL3Scope {
                field_id: "field-1".to_string(),
                season_id: "season-2026".to_string(),
                scene_id: None,
                temporal_start: "2026-05-01T00:00:00Z".to_string(),
                temporal_end: "2026-07-01T23:59:59Z".to_string(),
                source_id: None,
            },
        );
        assert_eq!(draft.kind, "dnbr");
        let inputs: Vec<&str> = draft.inputs.iter().map(|i| i.product_id.as_str()).collect();
        assert_eq!(inputs, vec!["pre-nbr", "post-nbr"]);
        assert_eq!(
            draft.evidence_digests,
            vec![result.evidence.input_hash.clone()]
        );
        // Deterministic: identical inputs, identical hash.
        let again = compute_dnbr(&pre, &post).unwrap();
        assert_eq!(again.evidence.input_hash, result.evidence.input_hash);
    }
}
