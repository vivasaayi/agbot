//! Standardized Precipitation Index (SPI) rasters (satellite pipeline
//! batch 9).
//!
//! SPI (McKee et al. 1993) scores a period's precipitation accumulation
//! against the multi-year record for the same calendar period: fit a gamma
//! distribution to the nonzero record (Thom 1958 maximum-likelihood
//! approximation), mix in the zero-precipitation probability
//! (`H(x) = q + (1 - q) * G(x)`), and transform the cumulative probability
//! to a standard-normal quantile. SPI ~ 0 is normal, negative is drier than
//! the record, positive is wetter.
//!
//! The engine is pure and per-pixel, mirroring `drought_indices`: the caller
//! (geo_hub) groups cataloged precipitation rasters by calendar period and
//! passes one period's accumulations; grids must match exactly (no
//! resampling here). All numerics are deterministic closed-form
//! approximations with published error bounds — no RNG, no iteration-order
//! dependence:
//! - regularized lower incomplete gamma via series / continued fraction
//!   (Numerical Recipes 6.2),
//! - inverse standard-normal CDF via Acklam's rational approximation
//!   (|rel err| < 1.15e-9).

use crate::evidence::{deterministic_fingerprint, AnalysisEvidenceError};
use crate::l3_product::{to_l3_draft, L3DraftContext};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use shared::product_graph::ProductRecordDraft;
use shared::schemas::{assert_raster_spatial_ref, RasterSpatialRef, RasterSpatialRefError};
use thiserror::Error;

/// Sentinel stored where a pixel has no SPI. Consumers must consult
/// `reason_codes`, never trust the sentinel.
pub const SPI_SENTINEL: f32 = f32::NAN;

/// Cumulative probabilities are clamped to `[SPI_PROB_FLOOR, 1 - SPI_PROB_FLOOR]`
/// before the normal quantile, bounding SPI to ~±3.09 (the conventional
/// operational range; McKee's classes end at ±2).
pub const SPI_PROB_FLOOR: f64 = 0.001;

/// Precipitation at or below this (mm) counts as a zero-rain period in the
/// mixed distribution.
pub const ZERO_PRECIP_EPSILON: f32 = 1.0e-3;

/// Default minimum distinct record years (SPI literature recommends 30+;
/// short satellite-era records still produce indicative values, so the
/// enforced floor mirrors the climatology default).
pub const DEFAULT_SPI_MIN_YEARS: u32 = 5;

/// Per-pixel outcome code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpiPixelReason {
    Computed,
    /// The current-period raster has no usable value here.
    NoCurrentObservation,
    /// Fewer distinct record years with a valid sample than `min_years`.
    BelowMinYears,
    /// The record admits no gamma fit here (fewer than two distinct nonzero
    /// values, or a degenerate Thom statistic).
    DegenerateFit,
}

/// McKee SPI drought/wet classes (boundaries at ±1, ±1.5, ±2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpiClass {
    ExtremeDrought,
    SevereDrought,
    ModerateDrought,
    NearNormal,
    ModeratelyWet,
    VeryWet,
    ExtremelyWet,
    Invalid,
}

/// Classify an SPI value per McKee et al. 1993.
pub fn classify_spi(value: f32) -> SpiClass {
    if !value.is_finite() {
        SpiClass::Invalid
    } else if value <= -2.0 {
        SpiClass::ExtremeDrought
    } else if value <= -1.5 {
        SpiClass::SevereDrought
    } else if value <= -1.0 {
        SpiClass::ModerateDrought
    } else if value < 1.0 {
        SpiClass::NearNormal
    } else if value < 1.5 {
        SpiClass::ModeratelyWet
    } else if value < 2.0 {
        SpiClass::VeryWet
    } else {
        SpiClass::ExtremelyWet
    }
}

/// Per-class pixel counts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiClassCounts {
    pub extreme_drought: u32,
    pub severe_drought: u32,
    pub moderate_drought: u32,
    pub near_normal: u32,
    pub moderately_wet: u32,
    pub very_wet: u32,
    pub extremely_wet: u32,
    pub invalid: u32,
}

impl SpiClassCounts {
    fn add(&mut self, class: SpiClass) {
        match class {
            SpiClass::ExtremeDrought => self.extreme_drought += 1,
            SpiClass::SevereDrought => self.severe_drought += 1,
            SpiClass::ModerateDrought => self.moderate_drought += 1,
            SpiClass::NearNormal => self.near_normal += 1,
            SpiClass::ModeratelyWet => self.moderately_wet += 1,
            SpiClass::VeryWet => self.very_wet += 1,
            SpiClass::ExtremelyWet => self.extremely_wet += 1,
            SpiClass::Invalid => self.invalid += 1,
        }
    }
}

/// One dated precipitation-accumulation raster in the record (all for the
/// same calendar period, one per year; the caller groups by period).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiObservation {
    /// Catalog product id (identity-bearing; becomes L3 lineage).
    pub product_id: String,
    /// Date anchoring the accumulation's year.
    pub observed_on: NaiveDate,
    /// Accumulated precipitation (mm), row-major.
    pub values: Vec<f32>,
    /// `true` = usable pixel.
    pub valid_mask: Vec<bool>,
    /// Must equal the request grid exactly.
    pub spatial_ref: RasterSpatialRef,
}

/// The current-period accumulation to score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiCurrentRaster {
    pub product_id: String,
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    pub values: Vec<f32>,
    pub valid_mask: Vec<bool>,
}

/// An SPI computation request. By convention the record includes the current
/// year's accumulation as an observation too (the fit uses the full record).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiRequest {
    pub current: SpiCurrentRaster,
    pub observations: Vec<SpiObservation>,
    pub min_years: u32,
}

/// Evidence object for one SPI run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpiEvidence {
    pub current_product_id: String,
    pub observation_product_ids: Vec<String>,
    pub record_years: Vec<i32>,
    pub min_years: u32,
    pub spatial_ref: RasterSpatialRef,
    pub zero_precip_epsilon: f32,
    pub probability_floor: f64,
    /// Deterministic canonical-JSON FNV fingerprint over the full input.
    pub input_hash: String,
}

/// A completed SPI raster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpiResult {
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    /// SPI values (~[-3.09, 3.09]); [`SPI_SENTINEL`] where invalid.
    pub values: Vec<f32>,
    pub reason_codes: Vec<SpiPixelReason>,
    pub classes: Vec<SpiClass>,
    pub class_counts: SpiClassCounts,
    /// Pixels whose cumulative probability hit [`SPI_PROB_FLOOR`].
    pub clamp_count: u32,
    pub valid_fraction: f32,
    pub evidence: SpiEvidence,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum SpiError {
    #[error("SPI request has no observations")]
    NoObservations,
    #[error("SPI min_years must be at least 1 (got 0)")]
    ZeroMinYears,
    #[error("current raster has {actual} values, expected {expected}")]
    CurrentLengthMismatch { expected: usize, actual: usize },
    #[error("current validity mask has {actual} pixels, expected {expected}")]
    CurrentMaskMismatch { expected: usize, actual: usize },
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
    #[error("grid spatial metadata is invalid: {reason}")]
    SpatialRef { reason: RasterSpatialRefError },
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] AnalysisEvidenceError),
}

// ---------------------------------------------------------------------------
// Deterministic special functions
// ---------------------------------------------------------------------------

/// ln Γ(x) for x > 0 (Lanczos approximation, |rel err| < 2e-10).
pub fn ln_gamma(x: f64) -> f64 {
    const COEFFS: [f64; 6] = [
        76.180_091_729_471_46,
        -86.505_320_329_416_77,
        24.014_098_240_830_91,
        -1.231_739_572_450_155,
        0.120_865_097_386_617_5e-2,
        -0.539_523_938_495_3e-5,
    ];
    let mut y = x;
    let tmp = x + 5.5;
    let tmp = tmp - (x + 0.5) * tmp.ln();
    let mut series = 1.000_000_000_190_015;
    for coeff in COEFFS {
        y += 1.0;
        series += coeff / y;
    }
    -tmp + (2.506_628_274_631_000_5 * series / x).ln()
}

/// Regularized lower incomplete gamma P(a, x) for a > 0, x >= 0
/// (Numerical Recipes 6.2: series for x < a + 1, continued fraction beyond).
pub fn regularized_gamma_p(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    const MAX_ITER: usize = 200;
    const EPS: f64 = 3.0e-9;
    if x < a + 1.0 {
        // Series representation.
        let mut ap = a;
        let mut sum = 1.0 / a;
        let mut del = sum;
        for _ in 0..MAX_ITER {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * EPS {
                break;
            }
        }
        (sum * (-x + a * x.ln() - ln_gamma(a)).exp()).clamp(0.0, 1.0)
    } else {
        // Continued fraction for Q(a, x); P = 1 - Q.
        const FPMIN: f64 = 1.0e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / FPMIN;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..=MAX_ITER {
            let an = -(i as f64) * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < FPMIN {
                d = FPMIN;
            }
            c = b + an / c;
            if c.abs() < FPMIN {
                c = FPMIN;
            }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < EPS {
                break;
            }
        }
        let q = (-x + a * x.ln() - ln_gamma(a)).exp() * h;
        (1.0 - q).clamp(0.0, 1.0)
    }
}

/// Inverse standard-normal CDF (probit) via Acklam's rational approximation
/// (|rel err| < 1.15e-9). `p` must be in (0, 1).
pub fn probit(p: f64) -> f64 {
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    const P_LOW: f64 = 0.02425;

    if p < P_LOW {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - P_LOW {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    }
}

/// Thom (1958) maximum-likelihood gamma fit: `(shape alpha, scale beta)`
/// from the nonzero sample mean and mean-log. `None` when the statistic is
/// degenerate (identical samples, non-finite intermediates).
fn thom_gamma_fit(nonzero: &[f64]) -> Option<(f64, f64)> {
    if nonzero.len() < 2 {
        return None;
    }
    let n = nonzero.len() as f64;
    let mean = nonzero.iter().sum::<f64>() / n;
    let mean_ln = nonzero.iter().map(|v| v.ln()).sum::<f64>() / n;
    let a_stat = mean.ln() - mean_ln;
    if !a_stat.is_finite() || a_stat <= 0.0 {
        return None;
    }
    let alpha = (1.0 + (1.0 + 4.0 * a_stat / 3.0).sqrt()) / (4.0 * a_stat);
    let beta = mean / alpha;
    if alpha.is_finite() && beta.is_finite() && alpha > 0.0 && beta > 0.0 {
        Some((alpha, beta))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// SPI computation
// ---------------------------------------------------------------------------

fn validate_request(request: &SpiRequest) -> Result<usize, SpiError> {
    if request.observations.is_empty() {
        return Err(SpiError::NoObservations);
    }
    if request.min_years == 0 {
        return Err(SpiError::ZeroMinYears);
    }
    let current = &request.current;
    assert_raster_spatial_ref(Some(&current.spatial_ref), current.width, current.height)
        .map_err(|reason| SpiError::SpatialRef { reason })?;
    let pixel_count = current.width as usize * current.height as usize;
    if current.values.len() != pixel_count {
        return Err(SpiError::CurrentLengthMismatch {
            expected: pixel_count,
            actual: current.values.len(),
        });
    }
    if current.valid_mask.len() != pixel_count {
        return Err(SpiError::CurrentMaskMismatch {
            expected: pixel_count,
            actual: current.valid_mask.len(),
        });
    }
    for (index, observation) in request.observations.iter().enumerate() {
        if observation.values.len() != pixel_count || observation.valid_mask.len() != pixel_count {
            return Err(SpiError::ObservationLengthMismatch {
                observation_index: index,
                expected: pixel_count,
                actual: observation.values.len().max(observation.valid_mask.len()),
            });
        }
        if observation.spatial_ref != current.spatial_ref {
            return Err(SpiError::SpatialRefMismatch {
                observation_index: index,
            });
        }
    }
    Ok(pixel_count)
}

/// Compute an SPI raster: per pixel, fit the gamma/zero-mixture to the
/// record and transform the current accumulation's cumulative probability
/// to a standard-normal quantile.
pub fn compute_spi(request: &SpiRequest) -> Result<SpiResult, SpiError> {
    let pixel_count = validate_request(request)?;
    let current = &request.current;

    // Deterministic record order: (date, original index).
    let mut order: Vec<usize> = (0..request.observations.len()).collect();
    order.sort_by_key(|&index| (request.observations[index].observed_on, index));

    let mut values = vec![SPI_SENTINEL; pixel_count];
    let mut reason_codes = vec![SpiPixelReason::Computed; pixel_count];
    let mut classes = vec![SpiClass::Invalid; pixel_count];
    let mut class_counts = SpiClassCounts::default();
    let mut clamp_count = 0u32;
    let mut valid = 0u32;

    for pixel in 0..pixel_count {
        let reason = 'pixel: {
            let x = current.values[pixel];
            if !current.valid_mask[pixel] || !x.is_finite() {
                break 'pixel SpiPixelReason::NoCurrentObservation;
            }
            // Collect the pixel record and its distinct years.
            let mut samples: Vec<f64> = Vec::with_capacity(order.len());
            let mut years: Vec<i32> = Vec::new();
            for &index in &order {
                let observation = &request.observations[index];
                let value = observation.values[pixel];
                if !observation.valid_mask[pixel] || !value.is_finite() || value < 0.0 {
                    continue;
                }
                samples.push(f64::from(value));
                let year = observation.observed_on.year();
                if !years.contains(&year) {
                    years.push(year);
                }
            }
            if (years.len() as u32) < request.min_years {
                break 'pixel SpiPixelReason::BelowMinYears;
            }
            let zeros = samples
                .iter()
                .filter(|v| **v <= f64::from(ZERO_PRECIP_EPSILON))
                .count();
            let nonzero: Vec<f64> = samples
                .iter()
                .copied()
                .filter(|v| *v > f64::from(ZERO_PRECIP_EPSILON))
                .collect();
            let q = zeros as f64 / samples.len() as f64;

            let x = f64::from(x);
            let cumulative = if x <= f64::from(ZERO_PRECIP_EPSILON) {
                // Zero rain now: probability mass at/below zero is q. (An
                // all-zero record gives q = 1 -> floor-clamped SPI, still
                // deterministic and reason-free: dry now, always dry.)
                q
            } else {
                let Some((alpha, beta)) = thom_gamma_fit(&nonzero) else {
                    break 'pixel SpiPixelReason::DegenerateFit;
                };
                q + (1.0 - q) * regularized_gamma_p(alpha, x / beta)
            };

            let floored = cumulative.clamp(SPI_PROB_FLOOR, 1.0 - SPI_PROB_FLOOR);
            if floored != cumulative {
                clamp_count += 1;
            }
            let spi = probit(floored) as f32;
            values[pixel] = spi;
            classes[pixel] = classify_spi(spi);
            valid += 1;
            SpiPixelReason::Computed
        };
        reason_codes[pixel] = reason;
        class_counts.add(classes[pixel]);
    }

    let record_years: Vec<i32> = {
        let mut years: Vec<i32> = request
            .observations
            .iter()
            .map(|observation| observation.observed_on.year())
            .collect();
        years.sort_unstable();
        years.dedup();
        years
    };
    let observation_product_ids: Vec<String> = order
        .iter()
        .map(|&index| request.observations[index].product_id.clone())
        .collect();
    let input_hash = deterministic_fingerprint(&(
        "spi_v1",
        &current.product_id,
        &current.values,
        &current.valid_mask,
        &observation_product_ids,
        request.min_years,
        &current.spatial_ref,
    ))?;

    Ok(SpiResult {
        width: current.width,
        height: current.height,
        spatial_ref: current.spatial_ref.clone(),
        values,
        reason_codes,
        classes,
        class_counts,
        clamp_count,
        valid_fraction: valid as f32 / pixel_count as f32,
        evidence: SpiEvidence {
            current_product_id: current.product_id.clone(),
            observation_product_ids,
            record_years,
            min_years: request.min_years,
            spatial_ref: current.spatial_ref.clone(),
            zero_precip_epsilon: ZERO_PRECIP_EPSILON,
            probability_floor: SPI_PROB_FLOOR,
            input_hash,
        },
    })
}

// ---------------------------------------------------------------------------
// L3 lineage
// ---------------------------------------------------------------------------

/// Scope an SPI L3 draft cannot derive from pixel data alone.
#[derive(Debug, Clone)]
pub struct SpiL3Scope {
    pub field_id: String,
    pub season_id: String,
    pub scene_id: Option<String>,
    pub temporal_start: String,
    pub temporal_end: String,
    pub source_id: Option<String>,
}

/// Map an SPI result to an L3 catalog draft (kind `spi`). Lineage = current
/// product + every record observation, deduplicated in record order.
pub fn spi_l3_draft(result: &SpiResult, scope: &SpiL3Scope) -> ProductRecordDraft {
    let mut input_product_ids = vec![result.evidence.current_product_id.clone()];
    for product_id in &result.evidence.observation_product_ids {
        if !input_product_ids.contains(product_id) {
            input_product_ids.push(product_id.clone());
        }
    }
    to_l3_draft(&L3DraftContext {
        kind: "spi".to_string(),
        algorithm_id: "drought.spi".to_string(),
        algorithm_version: "1.0.0".to_string(),
        field_id: scope.field_id.clone(),
        season_id: scope.season_id.clone(),
        scene_id: scope.scene_id.clone(),
        temporal_start: scope.temporal_start.clone(),
        temporal_end: scope.temporal_end.clone(),
        input_product_ids,
        parameters: serde_json::json!({
            "index_kind": "spi",
            "distribution": "gamma_thom1958_zero_mixture",
            "probability_floor": SPI_PROB_FLOOR,
            "zero_precip_epsilon": ZERO_PRECIP_EPSILON,
            "class_convention": "mckee_1993_pm_1_1p5_2",
            "record_years": result.evidence.record_years,
            "min_years": result.evidence.min_years,
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
            crs: Some("EPSG:4326".to_string()),
            bbox: Some(GeoBounds {
                min_lon: 76.0,
                min_lat: 11.0,
                max_lon: 76.1,
                max_lat: 11.1,
            }),
            geo_transform: Some([76.0, 0.05, 0.0, 11.1, 0.0, -0.05]),
            resolution: Some(RasterResolution { x: 0.05, y: 0.05 }),
        }
    }

    fn date(y: i32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, 6, 1).unwrap()
    }

    fn observation(id: &str, year: i32, values: Vec<f32>) -> SpiObservation {
        let mask = vec![true; values.len()];
        SpiObservation {
            product_id: id.to_string(),
            observed_on: date(year),
            valid_mask: mask,
            values,
            spatial_ref: spatial_ref_2x2(),
        }
    }

    fn current(values: Vec<f32>) -> SpiCurrentRaster {
        let mask = vec![true; values.len()];
        SpiCurrentRaster {
            product_id: "current".to_string(),
            width: 2,
            height: 2,
            spatial_ref: spatial_ref_2x2(),
            valid_mask: mask,
            values,
        }
    }

    #[test]
    fn incomplete_gamma_matches_closed_forms() {
        // P(1, x) = 1 - e^{-x} (exponential CDF).
        for x in [0.1, 0.5, 1.0, 2.5, 7.0] {
            let expected = 1.0 - (-x as f64).exp();
            assert!(
                (regularized_gamma_p(1.0, x) - expected).abs() < 1e-8,
                "P(1, {x})"
            );
        }
        // P(0.5, x) = erf(sqrt(x)); erf(1) = 0.8427007929.
        assert!((regularized_gamma_p(0.5, 1.0) - 0.842_700_792_9).abs() < 1e-8);
        // Bounds.
        assert_eq!(regularized_gamma_p(2.0, 0.0), 0.0);
        assert!(regularized_gamma_p(2.0, 1e6) > 1.0 - 1e-12);
    }

    #[test]
    fn probit_matches_published_quantiles() {
        assert!(probit(0.5).abs() < 1e-9);
        assert!((probit(0.975) - 1.959_963_985).abs() < 1e-7);
        assert!((probit(0.025) + 1.959_963_985).abs() < 1e-7);
        assert!((probit(0.841_344_746_1) - 1.0).abs() < 1e-7);
        // Antisymmetry across the branch boundaries.
        for p in [0.001, 0.02, 0.3, 0.6, 0.99] {
            assert!((probit(p) + probit(1.0 - p)).abs() < 1e-7, "p={p}");
        }
    }

    #[test]
    fn zero_rain_now_with_half_zero_record_is_exactly_spi_zero() {
        // Record per pixel: {0, 0, 0, 10, 20, 30} over six years -> q = 0.5.
        // Current = 0 -> H = q = 0.5 -> SPI = probit(0.5) = 0 exactly.
        let observations: Vec<SpiObservation> = (0..6)
            .map(|i| {
                let value = if i < 3 { 0.0 } else { 10.0 * (i - 2) as f32 };
                observation(&format!("p{i}"), 2020 + i as i32, vec![value; 4])
            })
            .collect();
        let request = SpiRequest {
            current: current(vec![0.0; 4]),
            observations,
            min_years: 5,
        };
        let result = compute_spi(&request).unwrap();
        for pixel in 0..4 {
            assert_eq!(result.reason_codes[pixel], SpiPixelReason::Computed);
            assert!(result.values[pixel].abs() < 1e-9);
            assert_eq!(result.classes[pixel], SpiClass::NearNormal);
        }
        assert_eq!(result.valid_fraction, 1.0);
        assert_eq!(result.clamp_count, 0);
    }

    #[test]
    fn spi_is_monotonic_and_signed_around_the_record() {
        // Wet record spread; drier current -> negative, wetter -> positive.
        let observations: Vec<SpiObservation> = [10.0f32, 20.0, 30.0, 40.0, 50.0]
            .iter()
            .enumerate()
            .map(|(i, v)| observation(&format!("p{i}"), 2020 + i as i32, vec![*v; 4]))
            .collect();
        let spi_of = |x: f32| {
            let request = SpiRequest {
                current: current(vec![x; 4]),
                observations: observations.clone(),
                min_years: 5,
            };
            compute_spi(&request).unwrap().values[0]
        };
        let low = spi_of(5.0);
        let mid = spi_of(28.0);
        let high = spi_of(80.0);
        assert!(low < mid && mid < high, "{low} {mid} {high}");
        assert!(low < 0.0, "drier than the whole record: {low}");
        assert!(high > 0.0, "wetter than the whole record: {high}");
    }

    #[test]
    fn extreme_current_hits_the_probability_floor_clamp() {
        let observations: Vec<SpiObservation> = [10.0f32, 12.0, 14.0, 16.0, 18.0]
            .iter()
            .enumerate()
            .map(|(i, v)| observation(&format!("p{i}"), 2020 + i as i32, vec![*v; 4]))
            .collect();
        let request = SpiRequest {
            current: current(vec![10_000.0; 4]),
            observations,
            min_years: 5,
        };
        let result = compute_spi(&request).unwrap();
        // probit(0.999) = 3.0902323...
        for pixel in 0..4 {
            assert!((result.values[pixel] - 3.090_232_3).abs() < 1e-4);
            assert_eq!(result.classes[pixel], SpiClass::ExtremelyWet);
        }
        assert_eq!(result.clamp_count, 4);
        assert_eq!(result.class_counts.extremely_wet, 4);
    }

    #[test]
    fn reason_codes_cover_no_current_below_min_years_and_degenerate_fit() {
        // Pixel 0: current invalid. Pixel 1: only observation years 2020-2022
        // valid (< min_years 5). Pixel 2: identical nonzero record (Thom
        // statistic 0 -> degenerate). Pixel 3: computed.
        let mut observations: Vec<SpiObservation> = (0..5)
            .map(|i| {
                let mut values = vec![10.0 + i as f32; 4];
                values[2] = 25.0; // identical across years at pixel 2
                observation(&format!("p{i}"), 2020 + i as i32, values)
            })
            .collect();
        for observation in observations.iter_mut().skip(3) {
            observation.valid_mask[1] = false; // years 2023-2024 invalid at pixel 1
        }
        let mut current_raster = current(vec![12.0; 4]);
        current_raster.valid_mask[0] = false;

        let result = compute_spi(&SpiRequest {
            current: current_raster,
            observations,
            min_years: 5,
        })
        .unwrap();
        assert_eq!(result.reason_codes[0], SpiPixelReason::NoCurrentObservation);
        assert_eq!(result.reason_codes[1], SpiPixelReason::BelowMinYears);
        assert_eq!(result.reason_codes[2], SpiPixelReason::DegenerateFit);
        assert_eq!(result.reason_codes[3], SpiPixelReason::Computed);
        assert!(result.values[0].is_nan());
        assert_eq!(result.class_counts.invalid, 3);
        assert!((result.valid_fraction - 0.25).abs() < 1e-6);
    }

    #[test]
    fn mckee_classes_are_pinned_at_the_boundaries() {
        assert_eq!(classify_spi(-2.01), SpiClass::ExtremeDrought);
        assert_eq!(classify_spi(-2.0), SpiClass::ExtremeDrought);
        assert_eq!(classify_spi(-1.7), SpiClass::SevereDrought);
        assert_eq!(classify_spi(-1.2), SpiClass::ModerateDrought);
        assert_eq!(classify_spi(0.0), SpiClass::NearNormal);
        assert_eq!(classify_spi(1.2), SpiClass::ModeratelyWet);
        assert_eq!(classify_spi(1.7), SpiClass::VeryWet);
        assert_eq!(classify_spi(2.0), SpiClass::ExtremelyWet);
        assert_eq!(classify_spi(f32::NAN), SpiClass::Invalid);
    }

    #[test]
    fn spi_l3_draft_carries_lineage_and_identity() {
        let observations: Vec<SpiObservation> = [10.0f32, 20.0, 30.0, 40.0, 50.0]
            .iter()
            .enumerate()
            .map(|(i, v)| observation(&format!("p{i}"), 2020 + i as i32, vec![*v; 4]))
            .collect();
        let result = compute_spi(&SpiRequest {
            current: current(vec![25.0; 4]),
            observations,
            min_years: 5,
        })
        .unwrap();
        let draft = spi_l3_draft(
            &result,
            &SpiL3Scope {
                field_id: "field-1".to_string(),
                season_id: "season-2026".to_string(),
                scene_id: None,
                temporal_start: "2026-06-01T00:00:00Z".to_string(),
                temporal_end: "2026-06-30T23:59:59Z".to_string(),
                source_id: Some("chirps".to_string()),
            },
        );
        assert_eq!(draft.kind, "spi");
        assert_eq!(draft.level, shared::product_graph::ProductLevel::L3);
        let inputs: Vec<&str> = draft.inputs.iter().map(|i| i.product_id.as_str()).collect();
        assert_eq!(inputs[0], "current");
        for id in ["p0", "p1", "p2", "p3", "p4"] {
            assert!(inputs.contains(&id));
        }
        assert_eq!(
            draft.evidence_digests,
            vec![result.evidence.input_hash.clone()]
        );
        // Deterministic identity: same input, same hash.
        let again = compute_spi(&SpiRequest {
            current: current(vec![25.0; 4]),
            observations: [10.0f32, 20.0, 30.0, 40.0, 50.0]
                .iter()
                .enumerate()
                .map(|(i, v)| observation(&format!("p{i}"), 2020 + i as i32, vec![*v; 4]))
                .collect(),
            min_years: 5,
        })
        .unwrap();
        assert_eq!(again.evidence.input_hash, result.evidence.input_hash);
    }
}
