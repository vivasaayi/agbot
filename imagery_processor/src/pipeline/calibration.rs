//! Sensor radiometric scaling: DN -> surface reflectance / surface temperature.
//!
//! Applies the published per-sensor linear scale/offset to raw digital numbers
//! (DN), maps fill/nodata DNs to invalid pixels, and clamps reflectance to its
//! physical [0.0, 1.0] range. Everything applied is recorded in the crate's
//! existing `RadiometricCalibrationEvidence` so downstream products stay
//! evidence-backed. Per-pixel outcomes reuse `IndexPixelValue` and its
//! `&'static str` reason codes (see `pipeline/indices.rs`).

use crate::io::{BandCalibrationCoefficients, CalibrationStatus, RadiometricCalibrationEvidence};
use crate::IndexPixelValue;
use std::collections::BTreeMap;

/// Reason code for fill/nodata DNs (Landsat C2 L2 fill DN=0, Sentinel-2 nodata DN=0).
pub const REASON_FILL: &str = "fill";
/// Reason-count key for pixels whose scaled value fell outside the valid range
/// and was clamped. The pixels remain valid; the count is evidence.
pub const REASON_CLAMPED_OUT_OF_RANGE: &str = "clamped_out_of_range";

const REFLECTANCE_RANGE: (f32, f32) = (0.0, 1.0);
const U16_MAX_DN: f32 = u16::MAX as f32;

/// Published radiometric scaling profiles for supported sensor products.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SensorProfile {
    /// Landsat Collection 2 Level-2 surface reflectance: refl = DN * 0.0000275 - 0.2.
    LandsatC2L2Sr,
    /// Landsat Collection 2 Level-2 surface temperature: K = DN * 0.00341802 + 149.0.
    LandsatC2L2St,
    /// Sentinel-2 L2A processing baseline >= 04.00: refl = (DN - 1000) / 10000.
    Sentinel2L2ABaseline0400,
    /// Sentinel-2 L2A pre-04.00 baseline: refl = DN / 10000.
    Sentinel2L2ALegacy,
    /// Generic linear scaling: value = DN * scale + offset. No fill DN, no clamping.
    Linear { scale: f32, offset: f32 },
}

impl SensorProfile {
    pub fn scale(self) -> f32 {
        match self {
            SensorProfile::LandsatC2L2Sr => 0.000_027_5,
            SensorProfile::LandsatC2L2St => 0.003_418_02,
            SensorProfile::Sentinel2L2ABaseline0400 | SensorProfile::Sentinel2L2ALegacy => 0.000_1,
            SensorProfile::Linear { scale, .. } => scale,
        }
    }

    pub fn offset(self) -> f32 {
        match self {
            SensorProfile::LandsatC2L2Sr => -0.2,
            SensorProfile::LandsatC2L2St => 149.0,
            SensorProfile::Sentinel2L2ABaseline0400 => -0.1,
            SensorProfile::Sentinel2L2ALegacy => 0.0,
            SensorProfile::Linear { offset, .. } => offset,
        }
    }

    /// DN that marks fill/nodata for this product; such pixels are invalid,
    /// never scaled (Landsat fill DN=0 must not become -0.2 reflectance, and
    /// S2 nodata DN=0 must not become -0.1).
    pub fn fill_dn(self) -> Option<u16> {
        match self {
            SensorProfile::LandsatC2L2Sr
            | SensorProfile::LandsatC2L2St
            | SensorProfile::Sentinel2L2ABaseline0400
            | SensorProfile::Sentinel2L2ALegacy => Some(0),
            SensorProfile::Linear { .. } => None,
        }
    }

    /// Valid output range the scaled value is clamped to, if any.
    /// Reflectance products clamp to [0.0, 1.0]; temperature and generic
    /// linear outputs are not clamped.
    pub fn valid_output_range(self) -> Option<(f32, f32)> {
        match self {
            SensorProfile::LandsatC2L2Sr
            | SensorProfile::Sentinel2L2ABaseline0400
            | SensorProfile::Sentinel2L2ALegacy => Some(REFLECTANCE_RANGE),
            SensorProfile::LandsatC2L2St | SensorProfile::Linear { .. } => None,
        }
    }

    pub fn calibration_status(self) -> CalibrationStatus {
        match self {
            SensorProfile::LandsatC2L2St => CalibrationStatus::CalibratedTemperatureKelvin,
            _ => CalibrationStatus::CalibratedReflectance,
        }
    }

    /// Coefficients in the shape the crate-wide calibration evidence records.
    /// When a profile has no clamp range, the representable output span of the
    /// u16 DN domain is recorded so the evidence stays concrete.
    pub fn coefficients(self) -> BandCalibrationCoefficients {
        let (output_min, output_max) = self.valid_output_range().unwrap_or_else(|| {
            let low = self.offset();
            let high = U16_MAX_DN * self.scale() + self.offset();
            (low.min(high), low.max(high))
        });
        BandCalibrationCoefficients {
            gain: self.scale(),
            offset: self.offset(),
            output_min,
            output_max,
        }
    }

    /// Build the crate's existing `RadiometricCalibrationEvidence` for the
    /// bands this profile was applied to.
    pub fn calibration_evidence<S: AsRef<str>>(
        self,
        band_names: &[S],
    ) -> RadiometricCalibrationEvidence {
        let coefficients = band_names
            .iter()
            .map(|band_name| (band_name.as_ref().to_string(), self.coefficients()))
            .collect::<BTreeMap<_, _>>();
        RadiometricCalibrationEvidence {
            status: self.calibration_status(),
            coefficients,
        }
    }
}

/// Result of scaling one band buffer: per-pixel values (with the crate's
/// reason-code pattern for invalid pixels) plus deterministic counters.
#[derive(Debug, Clone, PartialEq)]
pub struct ScaledBand {
    pub pixels: Vec<IndexPixelValue>,
    pub valid_pixel_count: usize,
    pub fill_pixel_count: usize,
    pub clamped_pixel_count: usize,
    pub reason_counts: BTreeMap<String, usize>,
}

/// Apply a sensor profile's scale/offset to a DN band buffer.
///
/// - Fill/nodata DNs become `IndexPixelValue::Invalid { reason: "fill" }`.
/// - Reflectance outputs are clamped to [0.0, 1.0]; clamped pixels stay valid
///   and are counted under `clamped_out_of_range`.
pub fn apply_radiometric_scaling(profile: SensorProfile, dns: &[u16]) -> ScaledBand {
    let scale = profile.scale();
    let offset = profile.offset();
    let fill_dn = profile.fill_dn();
    let output_range = profile.valid_output_range();

    let mut valid_pixel_count = 0usize;
    let mut fill_pixel_count = 0usize;
    let mut clamped_pixel_count = 0usize;
    let mut reason_counts: BTreeMap<String, usize> = BTreeMap::new();

    let pixels = dns
        .iter()
        .map(|&dn| {
            if fill_dn == Some(dn) {
                fill_pixel_count += 1;
                *reason_counts.entry(REASON_FILL.to_string()).or_insert(0) += 1;
                return IndexPixelValue::Invalid {
                    reason: REASON_FILL,
                };
            }

            let mut value = dn as f32 * scale + offset;
            if let Some((min, max)) = output_range {
                if value < min || value > max {
                    clamped_pixel_count += 1;
                    *reason_counts
                        .entry(REASON_CLAMPED_OUT_OF_RANGE.to_string())
                        .or_insert(0) += 1;
                    value = value.clamp(min, max);
                }
            }
            valid_pixel_count += 1;
            IndexPixelValue::Valid(value)
        })
        .collect();

    ScaledBand {
        pixels,
        valid_pixel_count,
        fill_pixel_count,
        clamped_pixel_count,
        reason_counts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32, tolerance: f32) {
        assert!(
            (actual - expected).abs() < tolerance,
            "actual {actual} did not match expected {expected}"
        );
    }

    fn single_valid(profile: SensorProfile, dn: u16) -> f32 {
        let scaled = apply_radiometric_scaling(profile, &[dn]);
        assert_eq!(scaled.valid_pixel_count, 1);
        scaled.pixels[0].value().expect("pixel should be valid")
    }

    #[test]
    fn landsat_sr_known_dn_fixtures_scale_to_reflectance() {
        // 7273 * 0.0000275 - 0.2 = 0.2000075 - 0.2 = 0.0000075
        assert_close(
            single_valid(SensorProfile::LandsatC2L2Sr, 7273),
            0.0000075,
            1e-6,
        );
        // 10000 * 0.0000275 - 0.2 = 0.275 - 0.2 = 0.075
        assert_close(
            single_valid(SensorProfile::LandsatC2L2Sr, 10000),
            0.075,
            1e-6,
        );
        // 20000 * 0.0000275 - 0.2 = 0.55 - 0.2 = 0.35
        assert_close(
            single_valid(SensorProfile::LandsatC2L2Sr, 20000),
            0.35,
            1e-6,
        );
    }

    #[test]
    fn landsat_st_known_dn_fixtures_scale_to_kelvin() {
        // 30000 * 0.00341802 + 149.0 = 102.5406 + 149.0 = 251.5406 K
        assert_close(
            single_valid(SensorProfile::LandsatC2L2St, 30000),
            251.5406,
            1e-3,
        );
        // 43600 * 0.00341802 + 149.0 = 149.025672 + 149.0 = 298.025672 K (~25 C)
        assert_close(
            single_valid(SensorProfile::LandsatC2L2St, 43600),
            298.02567,
            1e-3,
        );
    }

    #[test]
    fn sentinel2_baseline_0400_offset_differs_from_legacy() {
        // Baseline >= 04.00: (DN - 1000) / 10000
        assert_close(
            single_valid(SensorProfile::Sentinel2L2ABaseline0400, 1000),
            0.0,
            1e-7,
        );
        assert_close(
            single_valid(SensorProfile::Sentinel2L2ABaseline0400, 3500),
            0.25,
            1e-6,
        );
        // Legacy: DN / 10000
        assert_close(
            single_valid(SensorProfile::Sentinel2L2ALegacy, 3500),
            0.35,
            1e-6,
        );
        assert_close(
            single_valid(SensorProfile::Sentinel2L2ALegacy, 1000),
            0.1,
            1e-6,
        );
    }

    #[test]
    fn fill_dn_zero_maps_to_invalid_not_negative_reflectance() {
        for profile in [
            SensorProfile::LandsatC2L2Sr,
            SensorProfile::LandsatC2L2St,
            SensorProfile::Sentinel2L2ABaseline0400,
            SensorProfile::Sentinel2L2ALegacy,
        ] {
            let scaled = apply_radiometric_scaling(profile, &[0, 10000]);
            assert_eq!(
                scaled.pixels[0],
                IndexPixelValue::Invalid { reason: "fill" },
                "{profile:?} DN 0 must be fill, not a scaled value"
            );
            assert_eq!(scaled.fill_pixel_count, 1);
            assert_eq!(scaled.valid_pixel_count, 1);
            assert_eq!(scaled.reason_counts.get("fill"), Some(&1));
        }
    }

    #[test]
    fn reflectance_is_clamped_to_unit_range_and_counted() {
        // High clamp: 60000 * 0.0000275 - 0.2 = 1.45 -> 1.0
        // Low clamp: 1 * 0.0000275 - 0.2 = -0.19997... -> 0.0
        let scaled = apply_radiometric_scaling(SensorProfile::LandsatC2L2Sr, &[60000, 1, 10000]);

        assert_eq!(scaled.pixels[0], IndexPixelValue::Valid(1.0));
        assert_eq!(scaled.pixels[1], IndexPixelValue::Valid(0.0));
        assert_close(scaled.pixels[2].value().unwrap(), 0.075, 1e-6);
        assert_eq!(scaled.clamped_pixel_count, 2);
        assert_eq!(scaled.valid_pixel_count, 3);
        assert_eq!(scaled.reason_counts.get("clamped_out_of_range"), Some(&2));

        // Sentinel-2 baseline 04.00 below-offset DN also clamps low, not -0.09.
        let s2 = apply_radiometric_scaling(SensorProfile::Sentinel2L2ABaseline0400, &[100]);
        assert_eq!(s2.pixels[0], IndexPixelValue::Valid(0.0));
        assert_eq!(s2.clamped_pixel_count, 1);
    }

    #[test]
    fn kelvin_output_is_not_clamped_to_reflectance_range() {
        let scaled = apply_radiometric_scaling(SensorProfile::LandsatC2L2St, &[65535]);
        // 65535 * 0.00341802 + 149.0 = 373.01 K, well outside [0, 1].
        assert_close(scaled.pixels[0].value().unwrap(), 373.01, 0.02);
        assert_eq!(scaled.clamped_pixel_count, 0);
    }

    #[test]
    fn generic_linear_profile_applies_scale_and_offset_without_fill_or_clamp() {
        let profile = SensorProfile::Linear {
            scale: 0.5,
            offset: 1.0,
        };
        let scaled = apply_radiometric_scaling(profile, &[0, 4]);

        // DN 0 is a real value for the generic profile, not fill.
        assert_eq!(scaled.pixels[0], IndexPixelValue::Valid(1.0));
        assert_eq!(scaled.pixels[1], IndexPixelValue::Valid(3.0));
        assert_eq!(scaled.fill_pixel_count, 0);
        assert_eq!(scaled.clamped_pixel_count, 0);
        assert_eq!(scaled.valid_pixel_count, 2);
        assert!(scaled.reason_counts.is_empty());
    }

    #[test]
    fn calibration_evidence_records_applied_coefficients_per_band() {
        let evidence = SensorProfile::LandsatC2L2Sr.calibration_evidence(&["B4", "B5"]);

        assert_eq!(evidence.status, CalibrationStatus::CalibratedReflectance);
        assert_eq!(evidence.coefficients.len(), 2);
        let b4 = evidence.coefficients.get("B4").unwrap();
        assert_close(b4.gain, 0.0000275, 1e-12);
        assert_close(b4.offset, -0.2, 1e-12);
        assert_eq!((b4.output_min, b4.output_max), (0.0, 1.0));

        let st_evidence = SensorProfile::LandsatC2L2St.calibration_evidence(&["B10"]);
        assert_eq!(
            st_evidence.status,
            CalibrationStatus::CalibratedTemperatureKelvin
        );
        let b10 = st_evidence.coefficients.get("B10").unwrap();
        assert_close(b10.gain, 0.00341802, 1e-9);
        assert_close(b10.offset, 149.0, 1e-6);
        assert_close(b10.output_min, 149.0, 1e-6);
        assert_close(b10.output_max, 373.01, 0.02);

        let s2_evidence = SensorProfile::Sentinel2L2ABaseline0400.calibration_evidence(&["B04"]);
        let b04 = s2_evidence.coefficients.get("B04").unwrap();
        assert_close(b04.gain, 0.0001, 1e-12);
        assert_close(b04.offset, -0.1, 1e-12);
    }
}
