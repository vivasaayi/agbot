//! Land-surface-temperature engine (satellite pipeline batch 23).
//!
//! Deterministic thermal physics, pure over in-memory rasters:
//! DN -> radiance (`L = ML·DN + AL`) -> brightness temperature
//! (`TB = K2 / ln(1 + K1/L)`) -> emissivity-corrected LST
//! (`LST = TB / (1 + (λ·TB/ρ)·ln(ε))`, `ρ = h·c/σ ≈ 1.4388e-2 m·K`).
//!
//! Emissivity comes either from a caller-supplied constant or per-pixel from
//! an NDVI raster on the same grid via the standard threshold model: bare
//! soil below the soil threshold, full canopy above the vegetation
//! threshold, quadratic vegetation-fraction blend between. This mirrors the
//! CLI thermal pipeline in `imagery_processor` (the physics is extracted
//! here so `geo_hub` can register cataloged `lst` L2 products, which the
//! drought path scores into TCI).

use serde::{Deserialize, Serialize};
use shared::schemas::{assert_raster_spatial_ref, RasterSpatialRef, RasterSpatialRefError};
use thiserror::Error;

use crate::evidence::{deterministic_fingerprint, AnalysisEvidenceError};

/// NDVI at or below this is bare soil.
pub const NDVI_SOIL_THRESHOLD: f32 = 0.2;
/// NDVI at or above this is full canopy.
pub const NDVI_VEGETATION_THRESHOLD: f32 = 0.5;
pub const SOIL_EMISSIVITY: f32 = 0.97;
pub const VEGETATION_EMISSIVITY: f32 = 0.99;
/// In-memory sentinel for pixels without an LST value.
pub const LST_SENTINEL: f32 = f32::NAN;
/// Planck correction constant `ρ = h·c/σ` in m·K.
pub const PLANCK_RHO_M_K: f64 = 1.4388e-2;

/// Radiometric calibration for one thermal band (from the scene metadata).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LstCoefficients {
    /// Radiance multiplicative rescaling (`ML`).
    pub ml: f32,
    /// Radiance additive rescaling (`AL`).
    pub al: f32,
    /// Thermal conversion constant `K1` (W/(m²·sr·µm)).
    pub k1: f32,
    /// Thermal conversion constant `K2` (Kelvin).
    pub k2: f32,
    /// Band effective wavelength in micrometers (e.g. Landsat B10 10.895).
    pub lambda_um: f64,
}

/// Where per-pixel emissivity comes from.
#[derive(Debug, Clone)]
pub enum EmissivitySource {
    /// One emissivity for every pixel, in (0, 1].
    Constant(f32),
    /// Per-pixel NDVI (same grid as the thermal band); non-finite NDVI
    /// falls back to soil emissivity.
    FromNdvi { ndvi: Vec<f32> },
}

impl EmissivitySource {
    /// Stable method label recorded in evidence/parameters.
    pub fn method(&self) -> &'static str {
        match self {
            EmissivitySource::Constant(_) => "constant",
            EmissivitySource::FromNdvi { .. } => "ndvi_thresholds",
        }
    }
}

/// Threshold emissivity model: soil / quadratic-fraction blend / canopy.
pub fn emissivity_from_ndvi(ndvi: f32) -> f32 {
    if !ndvi.is_finite() || ndvi <= NDVI_SOIL_THRESHOLD {
        SOIL_EMISSIVITY
    } else if ndvi >= NDVI_VEGETATION_THRESHOLD {
        VEGETATION_EMISSIVITY
    } else {
        let vegetation_fraction = ((ndvi - NDVI_SOIL_THRESHOLD)
            / (NDVI_VEGETATION_THRESHOLD - NDVI_SOIL_THRESHOLD))
            .powi(2);
        SOIL_EMISSIVITY + vegetation_fraction * (VEGETATION_EMISSIVITY - SOIL_EMISSIVITY)
    }
}

/// One LST computation request over an in-memory DN raster.
#[derive(Debug, Clone)]
pub struct LstRequest {
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    /// Thermal-band digital numbers, row-major.
    pub dn: Vec<f32>,
    /// `true` = usable pixel (fill/nodata already excluded).
    pub valid_mask: Vec<bool>,
    pub emissivity: EmissivitySource,
    pub coefficients: LstCoefficients,
}

/// Why a pixel has (or lacks) an LST value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LstPixelReason {
    Computed,
    /// Masked or non-finite DN.
    NoObservation,
    /// `ML·DN + AL <= 0`: no physical radiance, no temperature.
    NonPositiveRadiance,
}

/// Min/max/mean over the finite values of one processing stage.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct LstStageStats {
    pub min: f32,
    pub max: f32,
    pub mean: f32,
    pub count: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct StatsAccumulator {
    min: f32,
    max: f32,
    sum: f64,
    count: u32,
}

impl StatsAccumulator {
    fn new() -> Self {
        Self {
            min: f32::INFINITY,
            max: f32::NEG_INFINITY,
            sum: 0.0,
            count: 0,
        }
    }

    fn record(&mut self, value: f32) {
        if value.is_finite() {
            self.min = self.min.min(value);
            self.max = self.max.max(value);
            self.sum += f64::from(value);
            self.count += 1;
        }
    }

    fn finish(self) -> LstStageStats {
        if self.count == 0 {
            return LstStageStats::default();
        }
        LstStageStats {
            min: self.min,
            max: self.max,
            mean: (self.sum / f64::from(self.count)) as f32,
            count: self.count,
        }
    }
}

/// Reproducibility evidence for one LST result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LstEvidence {
    pub coefficients: LstCoefficients,
    /// `constant` or `ndvi_thresholds`.
    pub emissivity_method: String,
    pub planck_rho_m_k: f64,
    /// Deterministic fingerprint of every identity-bearing input.
    pub input_hash: String,
}

/// Result of one LST computation.
#[derive(Debug, Clone)]
pub struct LstResult {
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    /// LST in Kelvin, row-major; [`LST_SENTINEL`] where not computed.
    pub values: Vec<f32>,
    pub reason_codes: Vec<LstPixelReason>,
    /// Fraction of pixels with a computed LST.
    pub valid_fraction: f32,
    pub radiance_stats: LstStageStats,
    pub brightness_temperature_stats: LstStageStats,
    pub lst_stats: LstStageStats,
    pub emissivity_stats: LstStageStats,
    pub evidence: LstEvidence,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum LstError {
    #[error("{field} has {actual} entries, expected {expected} (width·height)")]
    LengthMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("grid spatial metadata is invalid: {reason}")]
    SpatialRef { reason: RasterSpatialRefError },
    #[error("thermal coefficient {name} must be finite and positive (got {value})")]
    InvalidCoefficient { name: &'static str, value: f64 },
    #[error("constant emissivity must be within (0, 1] (got {value})")]
    InvalidEmissivity { value: f32 },
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] AnalysisEvidenceError),
}

fn validate(request: &LstRequest) -> Result<usize, LstError> {
    assert_raster_spatial_ref(Some(&request.spatial_ref), request.width, request.height)
        .map_err(|reason| LstError::SpatialRef { reason })?;
    let pixel_count = request.width as usize * request.height as usize;
    for (field, actual) in [
        ("dn", request.dn.len()),
        ("valid_mask", request.valid_mask.len()),
    ] {
        if actual != pixel_count {
            return Err(LstError::LengthMismatch {
                field,
                expected: pixel_count,
                actual,
            });
        }
    }
    let c = &request.coefficients;
    for (name, value) in [
        ("k1", f64::from(c.k1)),
        ("k2", f64::from(c.k2)),
        ("lambda_um", c.lambda_um),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(LstError::InvalidCoefficient { name, value });
        }
    }
    for (name, value) in [("ml", c.ml), ("al", c.al)] {
        if !value.is_finite() {
            return Err(LstError::InvalidCoefficient {
                name,
                value: f64::from(value),
            });
        }
    }
    match &request.emissivity {
        EmissivitySource::Constant(value) => {
            if !value.is_finite() || *value <= 0.0 || *value > 1.0 {
                return Err(LstError::InvalidEmissivity { value: *value });
            }
        }
        EmissivitySource::FromNdvi { ndvi } => {
            if ndvi.len() != pixel_count {
                return Err(LstError::LengthMismatch {
                    field: "emissivity ndvi",
                    expected: pixel_count,
                    actual: ndvi.len(),
                });
            }
        }
    }
    Ok(pixel_count)
}

/// Compute emissivity-corrected LST (Kelvin) for every usable pixel.
pub fn compute_lst(request: &LstRequest) -> Result<LstResult, LstError> {
    let pixel_count = validate(request)?;
    let c = request.coefficients;
    let lambda_m = c.lambda_um * 1e-6;

    let mut values = vec![LST_SENTINEL; pixel_count];
    let mut reason_codes = vec![LstPixelReason::Computed; pixel_count];
    let mut radiance = StatsAccumulator::new();
    let mut brightness = StatsAccumulator::new();
    let mut lst = StatsAccumulator::new();
    let mut emissivity = StatsAccumulator::new();
    let mut valid = 0u32;

    for pixel in 0..pixel_count {
        let dn = request.dn[pixel];
        if !request.valid_mask[pixel] || !dn.is_finite() {
            reason_codes[pixel] = LstPixelReason::NoObservation;
            continue;
        }
        // Radiance in f64 for a deterministic, precision-stable chain.
        let l = f64::from(c.ml) * f64::from(dn) + f64::from(c.al);
        if l <= 0.0 {
            reason_codes[pixel] = LstPixelReason::NonPositiveRadiance;
            continue;
        }
        radiance.record(l as f32);
        let tb = f64::from(c.k2) / (f64::from(c.k1) / l).ln_1p();
        brightness.record(tb as f32);
        let eps = match &request.emissivity {
            EmissivitySource::Constant(value) => *value,
            EmissivitySource::FromNdvi { ndvi } => emissivity_from_ndvi(ndvi[pixel]),
        };
        emissivity.record(eps);
        let lst_k = (tb / (1.0 + (lambda_m * tb / PLANCK_RHO_M_K) * f64::from(eps).ln())) as f32;
        values[pixel] = lst_k;
        lst.record(lst_k);
        valid += 1;
    }

    let ndvi_for_hash: &[f32] = match &request.emissivity {
        EmissivitySource::Constant(_) => &[],
        EmissivitySource::FromNdvi { ndvi } => ndvi,
    };
    let constant_for_hash = match &request.emissivity {
        EmissivitySource::Constant(value) => Some(*value),
        EmissivitySource::FromNdvi { .. } => None,
    };
    let input_hash = deterministic_fingerprint(&(
        "lst_v1",
        &request.dn,
        &request.valid_mask,
        request.emissivity.method(),
        constant_for_hash,
        ndvi_for_hash,
        c,
        &request.spatial_ref,
    ))?;

    Ok(LstResult {
        width: request.width,
        height: request.height,
        spatial_ref: request.spatial_ref.clone(),
        valid_fraction: if pixel_count == 0 {
            0.0
        } else {
            valid as f32 / pixel_count as f32
        },
        values,
        reason_codes,
        radiance_stats: radiance.finish(),
        brightness_temperature_stats: brightness.finish(),
        lst_stats: lst.finish(),
        emissivity_stats: emissivity.finish(),
        evidence: LstEvidence {
            coefficients: c,
            emissivity_method: request.emissivity.method().to_string(),
            planck_rho_m_k: PLANCK_RHO_M_K,
            input_hash,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::{GeoBounds, RasterResolution};

    fn spatial_ref(width: u32, height: u32) -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32643".to_string()),
            bbox: Some(GeoBounds {
                min_lon: 600_000.0,
                min_lat: 1_300_020.0 - f64::from(height) * 30.0,
                max_lon: 600_000.0 + f64::from(width) * 30.0,
                max_lat: 1_300_020.0,
            }),
            geo_transform: Some([600_000.0, 30.0, 0.0, 1_300_020.0, 0.0, -30.0]),
            resolution: Some(RasterResolution { x: 30.0, y: 30.0 }),
        }
    }

    /// DN chosen so `K1 / L = e - 1`, making `TB = K2 / ln(e) = K2` exactly.
    fn brightness_calibrated_dn() -> f32 {
        // ml = 0.001, al = 0: L = dn/1000 must equal 1/(e-1).
        (1000.0 / (std::f64::consts::E - 1.0)) as f32
    }

    fn request(dn: Vec<f32>, emissivity: EmissivitySource) -> LstRequest {
        let width = dn.len() as u32;
        LstRequest {
            width,
            height: 1,
            spatial_ref: spatial_ref(width, 1),
            valid_mask: vec![true; dn.len()],
            dn,
            emissivity,
            coefficients: LstCoefficients {
                ml: 0.001,
                al: 0.0,
                k1: 1.0,
                k2: 300.0,
                lambda_um: 10.895,
            },
        }
    }

    #[test]
    fn unit_emissivity_recovers_brightness_temperature() {
        // With eps = 1, ln(eps) = 0 and LST = TB. The DN is calibrated so
        // TB = K2 = 300 K exactly (see brightness_calibrated_dn).
        let result = compute_lst(&request(
            vec![brightness_calibrated_dn()],
            EmissivitySource::Constant(1.0),
        ))
        .expect("computes");
        assert!(
            (result.values[0] - 300.0).abs() < 1e-3,
            "{}",
            result.values[0]
        );
        assert_eq!(result.reason_codes[0], LstPixelReason::Computed);
        assert_eq!(result.valid_fraction, 1.0);
        assert!((result.brightness_temperature_stats.mean - 300.0).abs() < 1e-3);
    }

    #[test]
    fn emissivity_correction_matches_hand_computation() {
        // TB = 300 K (calibrated DN), eps = 0.99, lambda = 10.895 um:
        // LST = 300 / (1 + (10.895e-6 * 300 / 1.4388e-2) * ln(0.99))
        //     = 300 / (1 + 0.2271650 * (-0.0100503)) = 300 / 0.9977169
        //     ≈ 300.6865 K — warmer than TB, as sub-unit emissivity demands.
        let expected = (300.0 / (1.0 + (10.895e-6 * 300.0 / 1.4388e-2) * 0.99f64.ln())) as f32;
        let result = compute_lst(&request(
            vec![brightness_calibrated_dn()],
            EmissivitySource::Constant(0.99),
        ))
        .expect("computes");
        assert!(
            (result.values[0] - expected).abs() < 1e-3,
            "{}",
            result.values[0]
        );
        assert!((expected - 300.6865).abs() < 1e-3);
    }

    #[test]
    fn emissivity_from_ndvi_thresholds_are_pinned() {
        assert_eq!(emissivity_from_ndvi(0.1), SOIL_EMISSIVITY);
        assert_eq!(emissivity_from_ndvi(0.2), SOIL_EMISSIVITY);
        assert_eq!(emissivity_from_ndvi(0.5), VEGETATION_EMISSIVITY);
        assert_eq!(emissivity_from_ndvi(0.9), VEGETATION_EMISSIVITY);
        assert_eq!(emissivity_from_ndvi(f32::NAN), SOIL_EMISSIVITY);
        // Quadratic blend at ndvi = 0.35: fraction = ((0.35-0.2)/0.3)^2 =
        // 0.25, eps = 0.97 + 0.25 * 0.02 = 0.975.
        assert!((emissivity_from_ndvi(0.35) - 0.975).abs() < 1e-6);
    }

    #[test]
    fn ndvi_emissivity_is_per_pixel() {
        // Same DN everywhere; soil pixel (eps 0.97) must come out warmer
        // than the canopy pixel (eps 0.99), both warmer than TB = 300 K.
        let dn = brightness_calibrated_dn();
        let result = compute_lst(&request(
            vec![dn, dn],
            EmissivitySource::FromNdvi {
                ndvi: vec![0.1, 0.9],
            },
        ))
        .expect("computes");
        let (soil, canopy) = (result.values[0], result.values[1]);
        assert!(soil > canopy, "soil {soil} vs canopy {canopy}");
        assert!(canopy > 300.0);
        assert_eq!(result.evidence.emissivity_method, "ndvi_thresholds");
        assert_eq!(result.emissivity_stats.min, SOIL_EMISSIVITY);
        assert_eq!(result.emissivity_stats.max, VEGETATION_EMISSIVITY);
    }

    #[test]
    fn invalid_pixels_are_reason_coded_not_dropped() {
        let dn = brightness_calibrated_dn();
        let mut req = request(
            // Pixel 1: DN 0 with al = -1 gives L = -1 (non-positive).
            vec![dn, 0.0, dn],
            EmissivitySource::Constant(1.0),
        );
        req.coefficients.al = -1.0;
        req.coefficients.ml = 0.001 + 1.0 / dn; // keep pixel 0/2 radiance positive
        req.valid_mask[2] = false;
        let result = compute_lst(&req).expect("computes");
        assert_eq!(result.reason_codes[0], LstPixelReason::Computed);
        assert_eq!(result.reason_codes[1], LstPixelReason::NonPositiveRadiance);
        assert_eq!(result.reason_codes[2], LstPixelReason::NoObservation);
        assert!(result.values[1].is_nan());
        assert!(result.values[2].is_nan());
        assert!((result.valid_fraction - 1.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn validation_rejects_bad_inputs() {
        let dn = brightness_calibrated_dn();
        let mut bad_k1 = request(vec![dn], EmissivitySource::Constant(1.0));
        bad_k1.coefficients.k1 = 0.0;
        assert!(matches!(
            compute_lst(&bad_k1),
            Err(LstError::InvalidCoefficient { name: "k1", .. })
        ));

        assert!(matches!(
            compute_lst(&request(vec![dn], EmissivitySource::Constant(1.2))),
            Err(LstError::InvalidEmissivity { .. })
        ));

        let mut short_mask = request(vec![dn, dn], EmissivitySource::Constant(1.0));
        short_mask.valid_mask.pop();
        assert!(matches!(
            compute_lst(&short_mask),
            Err(LstError::LengthMismatch {
                field: "valid_mask",
                ..
            })
        ));

        assert!(matches!(
            compute_lst(&request(
                vec![dn, dn],
                EmissivitySource::FromNdvi { ndvi: vec![0.5] }
            )),
            Err(LstError::LengthMismatch {
                field: "emissivity ndvi",
                ..
            })
        ));
    }

    #[test]
    fn evidence_fingerprint_is_deterministic_and_input_sensitive() {
        let dn = brightness_calibrated_dn();
        let req = request(vec![dn], EmissivitySource::Constant(0.99));
        let a = compute_lst(&req).expect("computes");
        let b = compute_lst(&req).expect("computes");
        assert_eq!(a.evidence.input_hash, b.evidence.input_hash);
        let c =
            compute_lst(&request(vec![dn], EmissivitySource::Constant(0.98))).expect("computes");
        assert_ne!(a.evidence.input_hash, c.evidence.input_hash);
    }
}
