//! Evapotranspiration fraction from the LST–NDVI triangle (satellite
//! pipeline batch 40) — the demand side of water availability.
//!
//! Research grounding (sources in `docs/design/satellite-intelligence-
//! pipeline.md` batch-40 entry): USGS's operational SSEBop needs
//! climatological air temperature and a precomputed dT even for its ET
//! fraction, so it is NOT derivable from a scene alone. The **Ts–VI
//! triangle** (Jiang & Islam 1999/2001; Carlson 2007 review) is: over a
//! scene spanning a range of moisture and vegetation, the LST–NDVI scatter
//! is bounded by a warm **dry edge** (max LST per NDVI interval, moisture-
//! limited) and a cold **wet edge** (scene minimum LST, energy-limited),
//! and a pixel's Priestley–Taylor moisture parameter φ interpolates
//! linearly between them:
//!
//! ```text
//! φ/φmax = (Ts_max(bin) − Ts) / (Ts_max(bin) − Ts_min)
//! ```
//!
//! This module publishes exactly that normalized fraction as
//! `et_fraction` ∈ [0, 1] (1 = wet-edge, evaporating at the potential
//! rate; 0 = dry-edge, no evaporative cooling). It is an **instantaneous
//! evaporative-fraction proxy** — standard practice as a standalone
//! product (EF "self-preservation") — and conversion to mm/day is
//! deliberately deferred until a reference-ET source exists (Hargreaves–
//! Samani needs only Tmin/Tmax plus the [`extraterrestrial_radiation_mj`]
//! implemented here from FAO-56, verified against its worked example).
//!
//! Documented limitations enforced as guards, not ignored: the scene must
//! span enough NDVI range for edges to mean anything
//! ([`MIN_NDVI_RANGE`]), each NDVI bin needs enough pixels
//! ([`MIN_BIN_PIXELS`]), and a degenerate edge (dry = wet) reason-codes
//! its pixels instead of dividing by ~0. Expected accuracy per the
//! validation literature: EF RMSE ≈ 0.15–0.20 vs flux towers.

use serde::{Deserialize, Serialize};
use shared::schemas::{assert_raster_spatial_ref, RasterSpatialRef, RasterSpatialRefError};
use thiserror::Error;

use crate::evidence::{deterministic_fingerprint, AnalysisEvidenceError};

/// NDVI intervals the dry edge is fitted over.
pub const NDVI_BINS: usize = 10;
/// A bin's dry edge is trusted only with at least this many pixels.
pub const MIN_BIN_PIXELS: u32 = 3;
/// The scene's valid NDVI span must be at least this wide, or the
/// triangle's premise (full moisture/vegetation range) does not hold.
pub const MIN_NDVI_RANGE: f32 = 0.3;
/// Dry and wet edge must differ by at least this (Kelvin) per bin.
pub const MIN_EDGE_SEPARATION_K: f32 = 1.0;
/// In-memory sentinel for pixels without a fraction.
pub const ET_SENTINEL: f32 = f32::NAN;

/// FAO-56 solar constant (MJ m^-2 min^-1).
pub const SOLAR_CONSTANT_MJ_M2_MIN: f64 = 0.0820;

/// Extraterrestrial radiation Ra (MJ m^-2 day^-1) from latitude and day of
/// year — FAO-56 equations 21/23/24/25, verbatim:
/// `Ra = (24*60/pi) * Gsc * dr * [ws*sin(phi)*sin(delta) +
/// cos(phi)*cos(delta)*sin(ws)]` with `dr = 1 + 0.033*cos(2*pi*J/365)`,
/// `delta = 0.409*sin(2*pi*J/365 - 1.39)`,
/// `ws = arccos(-tan(phi)*tan(delta))`. Deterministic; the only inputs
/// Hargreaves–Samani ETo will need beyond air temperature.
pub fn extraterrestrial_radiation_mj(latitude_deg: f64, day_of_year: u32) -> f64 {
    let phi = latitude_deg.to_radians();
    let j = f64::from(day_of_year);
    let day_angle = 2.0 * std::f64::consts::PI * j / 365.0;
    let dr = 1.0 + 0.033 * day_angle.cos();
    let delta = 0.409 * (day_angle - 1.39).sin();
    // Clamp for polar day/night where |tan(phi)*tan(delta)| > 1.
    let ws = (-phi.tan() * delta.tan()).clamp(-1.0, 1.0).acos();
    (24.0 * 60.0 / std::f64::consts::PI)
        * SOLAR_CONSTANT_MJ_M2_MIN
        * dr
        * (ws * phi.sin() * delta.sin() + phi.cos() * delta.cos() * ws.sin())
}

/// Ra in equivalent evaporation (mm/day): `0.408 * Ra_MJ` (FAO-56 eq. 20).
pub fn extraterrestrial_radiation_mm(latitude_deg: f64, day_of_year: u32) -> f64 {
    0.408 * extraterrestrial_radiation_mj(latitude_deg, day_of_year)
}

/// Why a pixel has (or lacks) an ET fraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EtPixelReason {
    Computed,
    /// LST or NDVI masked/non-finite here.
    NoObservation,
    /// The pixel's NDVI bin had too few pixels for a trusted dry edge.
    ThinBin,
    /// Dry and wet edges closer than [`MIN_EDGE_SEPARATION_K`].
    DegenerateEdge,
}

/// One triangle computation request over co-registered LST + NDVI.
#[derive(Debug, Clone)]
pub struct EtFractionRequest {
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    /// Land surface temperature, Kelvin, row-major.
    pub lst: Vec<f32>,
    pub lst_valid: Vec<bool>,
    /// NDVI on the same grid.
    pub ndvi: Vec<f32>,
    pub ndvi_valid: Vec<bool>,
}

/// Evidence for one triangle run: the self-calibrated edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EtFractionEvidence {
    /// Scene wet edge (minimum valid LST, Kelvin).
    pub wet_edge_k: f32,
    /// Per-bin dry edge (max LST, Kelvin); NaN for thin bins.
    pub dry_edge_k: Vec<f32>,
    /// NDVI domain the bins span.
    pub ndvi_min: f32,
    pub ndvi_max: f32,
    pub ndvi_bins: usize,
    pub min_bin_pixels: u32,
    pub min_edge_separation_k: f32,
    pub input_hash: String,
}

/// A completed ET-fraction raster.
#[derive(Debug, Clone)]
pub struct EtFractionResult {
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    /// Fraction in [0, 1]; [`ET_SENTINEL`] where not computed.
    pub values: Vec<f32>,
    pub reason_codes: Vec<EtPixelReason>,
    pub valid_fraction: f32,
    /// Mean fraction over computed pixels.
    pub mean_fraction: f32,
    pub evidence: EtFractionEvidence,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum EtFractionError {
    #[error("{field} has {actual} entries, expected {expected} (width*height)")]
    LengthMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("grid spatial metadata is invalid: {reason}")]
    SpatialRef { reason: RasterSpatialRefError },
    #[error("no pixels have both a valid LST and a valid NDVI")]
    NoValidPixels,
    #[error("scene NDVI range {range:.3} is below {MIN_NDVI_RANGE} — the triangle needs a scene spanning bare-dry to vegetated conditions (documented method limitation)")]
    InsufficientNdviRange { range: f32 },
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] AnalysisEvidenceError),
}

/// Compute the triangle ET fraction: per-scene self-calibrated edges, no
/// meteorology.
pub fn compute_et_fraction(
    request: &EtFractionRequest,
) -> Result<EtFractionResult, EtFractionError> {
    assert_raster_spatial_ref(Some(&request.spatial_ref), request.width, request.height)
        .map_err(|reason| EtFractionError::SpatialRef { reason })?;
    let pixel_count = request.width as usize * request.height as usize;
    for (field, actual) in [
        ("lst", request.lst.len()),
        ("lst_valid", request.lst_valid.len()),
        ("ndvi", request.ndvi.len()),
        ("ndvi_valid", request.ndvi_valid.len()),
    ] {
        if actual != pixel_count {
            return Err(EtFractionError::LengthMismatch {
                field,
                expected: pixel_count,
                actual,
            });
        }
    }

    // Jointly-valid pixels + scene NDVI domain and wet edge.
    let usable = |pixel: usize| {
        request.lst_valid[pixel]
            && request.lst[pixel].is_finite()
            && request.ndvi_valid[pixel]
            && request.ndvi[pixel].is_finite()
    };
    let mut ndvi_min = f32::MAX;
    let mut ndvi_max = f32::MIN;
    let mut wet_edge = f32::MAX;
    let mut any = false;
    for pixel in 0..pixel_count {
        if !usable(pixel) {
            continue;
        }
        any = true;
        ndvi_min = ndvi_min.min(request.ndvi[pixel]);
        ndvi_max = ndvi_max.max(request.ndvi[pixel]);
        wet_edge = wet_edge.min(request.lst[pixel]);
    }
    if !any {
        return Err(EtFractionError::NoValidPixels);
    }
    let range = ndvi_max - ndvi_min;
    if range < MIN_NDVI_RANGE {
        return Err(EtFractionError::InsufficientNdviRange { range });
    }

    // Dry edge: max LST per NDVI bin, trusted only with enough pixels.
    let bin_of = |ndvi: f32| {
        let t = ((ndvi - ndvi_min) / range).clamp(0.0, 1.0);
        ((t * NDVI_BINS as f32) as usize).min(NDVI_BINS - 1)
    };
    let mut dry_edge = [f32::NEG_INFINITY; NDVI_BINS];
    let mut bin_counts = [0u32; NDVI_BINS];
    for pixel in 0..pixel_count {
        if !usable(pixel) {
            continue;
        }
        let bin = bin_of(request.ndvi[pixel]);
        bin_counts[bin] += 1;
        dry_edge[bin] = dry_edge[bin].max(request.lst[pixel]);
    }

    // Per-pixel fraction between the pixel's bin dry edge and the wet edge.
    let mut values = vec![ET_SENTINEL; pixel_count];
    let mut reason_codes = vec![EtPixelReason::Computed; pixel_count];
    let mut valid = 0u32;
    let mut sum = 0f64;
    for pixel in 0..pixel_count {
        if !usable(pixel) {
            reason_codes[pixel] = EtPixelReason::NoObservation;
            continue;
        }
        let bin = bin_of(request.ndvi[pixel]);
        if bin_counts[bin] < MIN_BIN_PIXELS {
            reason_codes[pixel] = EtPixelReason::ThinBin;
            continue;
        }
        let dry = dry_edge[bin];
        if dry - wet_edge < MIN_EDGE_SEPARATION_K {
            reason_codes[pixel] = EtPixelReason::DegenerateEdge;
            continue;
        }
        let fraction = ((dry - request.lst[pixel]) / (dry - wet_edge)).clamp(0.0, 1.0);
        values[pixel] = fraction;
        valid += 1;
        sum += f64::from(fraction);
    }

    let dry_edge_out: Vec<f32> = dry_edge
        .iter()
        .zip(&bin_counts)
        .map(|(edge, count)| {
            if *count >= MIN_BIN_PIXELS {
                *edge
            } else {
                f32::NAN
            }
        })
        .collect();
    let input_hash = deterministic_fingerprint(&(
        "et_fraction_triangle_v1",
        &request.lst,
        &request.lst_valid,
        &request.ndvi,
        &request.ndvi_valid,
        NDVI_BINS,
        MIN_BIN_PIXELS,
        MIN_EDGE_SEPARATION_K,
        &request.spatial_ref,
    ))?;

    Ok(EtFractionResult {
        width: request.width,
        height: request.height,
        spatial_ref: request.spatial_ref.clone(),
        values,
        reason_codes,
        valid_fraction: valid as f32 / pixel_count as f32,
        mean_fraction: if valid == 0 {
            f32::NAN
        } else {
            (sum / f64::from(valid)) as f32
        },
        evidence: EtFractionEvidence {
            wet_edge_k: wet_edge,
            dry_edge_k: dry_edge_out,
            ndvi_min,
            ndvi_max,
            ndvi_bins: NDVI_BINS,
            min_bin_pixels: MIN_BIN_PIXELS,
            min_edge_separation_k: MIN_EDGE_SEPARATION_K,
            input_hash,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::{GeoBounds, RasterResolution};

    #[test]
    fn fao56_worked_example_8_reproduces() {
        // FAO-56 Example 8: 3 September (J = 246), latitude 20°S ->
        // Ra = 32.2 MJ m^-2 day^-1.
        let ra = extraterrestrial_radiation_mj(-20.0, 246);
        assert!((ra - 32.2).abs() < 0.1, "Ra {ra}");
        // Equivalent evaporation ~13.1 mm/day (0.408 * 32.2).
        let mm = extraterrestrial_radiation_mm(-20.0, 246);
        assert!((mm - 13.1).abs() < 0.1, "Ra_mm {mm}");
        // Equator at equinox is near the annual maximum (~37-38 MJ).
        let equator = extraterrestrial_radiation_mj(0.0, 80);
        assert!((36.0..39.0).contains(&equator), "{equator}");
    }

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

    /// A 4x2 scene with two NDVI clusters (bare ~0.1, vegetated ~0.8) so
    /// the bins are well-populated. LST spans 290..310 K.
    fn request() -> EtFractionRequest {
        // pixels:      0     1     2     3     4     5     6     7
        let ndvi = vec![0.10, 0.11, 0.12, 0.09, 0.80, 0.81, 0.79, 0.80];
        let lst = vec![310.0, 300.0, 305.0, 290.0, 300.0, 290.0, 295.0, 292.5];
        EtFractionRequest {
            width: 4,
            height: 2,
            spatial_ref: spatial_ref(4, 2),
            lst_valid: vec![true; 8],
            ndvi_valid: vec![true; 8],
            lst,
            ndvi,
        }
    }

    #[test]
    fn triangle_fractions_are_hand_computed() {
        // Wet edge = scene min LST = 290. Bare bin (bin 0) dry edge = 310;
        // vegetated bin (bin 9) dry edge = 300.
        //   pixel 0: (310-310)/20 = 0     (hot bare: no ET)
        //   pixel 1: (310-300)/20 = 0.5
        //   pixel 3: (310-290)/20 = 1     (cool bare: wet)
        //   pixel 4: (300-300)/10 = 0     (hot canopy)
        //   pixel 5: (300-290)/10 = 1
        //   pixel 7: (300-292.5)/10 = 0.75
        let result = compute_et_fraction(&request()).unwrap();
        assert_eq!(result.evidence.wet_edge_k, 290.0);
        assert_eq!(result.evidence.dry_edge_k[0], 310.0);
        assert_eq!(result.evidence.dry_edge_k[NDVI_BINS - 1], 300.0);
        for (pixel, expected) in [
            (0usize, 0.0f32),
            (1, 0.5),
            (3, 1.0),
            (4, 0.0),
            (5, 1.0),
            (7, 0.75),
        ] {
            assert!(
                (result.values[pixel] - expected).abs() < 1e-6,
                "pixel {pixel}: {} != {expected}",
                result.values[pixel]
            );
        }
        assert_eq!(result.valid_fraction, 1.0);
        assert!(result
            .reason_codes
            .iter()
            .all(|r| *r == EtPixelReason::Computed));
    }

    #[test]
    fn masked_pixels_and_thin_bins_are_reason_coded() {
        let mut req = request();
        req.lst_valid[2] = false; // knock a bare pixel out
                                  // Move pixel 6 to a lonely middle bin (only 1 pixel < MIN_BIN_PIXELS).
        req.ndvi[6] = 0.45;
        let result = compute_et_fraction(&req).unwrap();
        assert_eq!(result.reason_codes[2], EtPixelReason::NoObservation);
        assert_eq!(result.reason_codes[6], EtPixelReason::ThinBin);
        assert!(result.values[6].is_nan());
        assert!(result.valid_fraction < 1.0);
    }

    #[test]
    fn flat_scenes_are_refused_per_the_documented_limitation() {
        // All NDVI ~0.5: no moisture/vegetation range, edges meaningless.
        let mut req = request();
        req.ndvi = vec![0.5; 8];
        assert!(matches!(
            compute_et_fraction(&req),
            Err(EtFractionError::InsufficientNdviRange { .. })
        ));

        // Isothermal scene: edges closer than the separation floor.
        let mut req = request();
        req.lst = vec![300.0; 8];
        let result = compute_et_fraction(&req).unwrap();
        assert!(result
            .reason_codes
            .iter()
            .all(|r| *r == EtPixelReason::DegenerateEdge));
        assert_eq!(result.valid_fraction, 0.0);
    }
}
