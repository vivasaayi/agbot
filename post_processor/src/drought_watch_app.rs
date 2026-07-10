//! Drought-watch application (satellite pipeline batch 36).
//!
//! Pure composition, mirroring [`crate::anomaly_app`]: registered drought
//! condition rasters (VCI/TCI/VHI, percent 0-100) become per-product
//! drought findings by deterministic severity accounting — the fraction of
//! valid pixels in the Kogan stress classes (< 30: moderate or worse)
//! against warning/critical thresholds. Coverage is honest: a raster whose
//! valid fraction is below the floor yields an `insufficient_coverage`
//! finding instead of a confident all-clear.
//!
//! The geo_hub run wrapper maps these into `drought_stress_zone`
//! application findings, which Track C's alert evaluation screens into
//! early-warning alerts.

use serde::{Deserialize, Serialize};
use shared::schemas::RecommendationPriority;

use crate::drought_indices::{classify_severity, DroughtSeverity};

/// Stressed fraction at or above this fires a warning-level finding.
pub const DEFAULT_WARNING_STRESSED_FRACTION: f32 = 0.3;
/// Stressed fraction at or above this fires a critical-level finding.
pub const DEFAULT_CRITICAL_STRESSED_FRACTION: f32 = 0.6;
/// Valid-pixel coverage below this is too thin to judge the field.
pub const DEFAULT_MIN_VALID_FRACTION: f32 = 0.25;
/// SPI at or below this counts as stressed (McKee moderate drought).
pub const SPI_STRESSED_THRESHOLD: f32 = -1.0;
/// SPI at or below this counts as extreme drought (McKee).
pub const SPI_EXTREME_THRESHOLD: f32 = -2.0;

/// One registered drought raster to evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroughtProductReading {
    /// Catalog product id (becomes the finding's evidence ref).
    pub product_id: String,
    /// `vci` / `tci` / `vhi` (Kogan percent 0-100) or `spi` (McKee
    /// z-score) — selects the stress classification scale.
    pub index_kind: String,
    /// Condition percent values, row-major.
    pub values: Vec<f32>,
    /// `true` = usable pixel.
    pub valid_mask: Vec<bool>,
}

/// Watch thresholds; `None` fields use the defaults.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct DroughtWatchConfig {
    pub warning_stressed_fraction: Option<f32>,
    pub critical_stressed_fraction: Option<f32>,
    pub min_valid_fraction: Option<f32>,
}

/// One per-product drought finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtWatchFinding {
    pub product_id: String,
    pub index_kind: String,
    /// Fraction of valid pixels below 30 (moderate drought or worse).
    pub stressed_fraction: f32,
    /// Fraction of valid pixels below 10 (extreme drought).
    pub extreme_fraction: f32,
    /// Mean condition percent over valid pixels.
    pub mean_index: f32,
    /// Valid share of all pixels.
    pub valid_fraction: f32,
    pub is_drought: bool,
    pub priority: RecommendationPriority,
    /// `stressed_fraction_critical`, `stressed_fraction_warning`,
    /// `nominal`, or `insufficient_coverage`.
    pub reason_code: &'static str,
}

/// Evaluate drought-watch findings for each reading. Deterministic; one
/// finding per reading, in input order.
pub fn evaluate_drought_watch(
    readings: &[DroughtProductReading],
    config: &DroughtWatchConfig,
) -> Vec<DroughtWatchFinding> {
    let warning = config
        .warning_stressed_fraction
        .unwrap_or(DEFAULT_WARNING_STRESSED_FRACTION);
    let critical = config
        .critical_stressed_fraction
        .unwrap_or(DEFAULT_CRITICAL_STRESSED_FRACTION);
    let min_valid = config
        .min_valid_fraction
        .unwrap_or(DEFAULT_MIN_VALID_FRACTION);

    readings
        .iter()
        .map(|reading| {
            let pixel_count = reading.values.len();
            let is_spi = reading.index_kind.trim().eq_ignore_ascii_case("spi");
            let mut valid = 0u32;
            let mut stressed = 0u32;
            let mut extreme = 0u32;
            let mut sum = 0f64;
            for (value, keep) in reading.values.iter().zip(&reading.valid_mask) {
                if !*keep || !value.is_finite() {
                    continue;
                }
                valid += 1;
                sum += f64::from(*value);
                if is_spi {
                    // McKee (1993) classes: <= -1 moderate+, <= -2 extreme.
                    if *value <= SPI_STRESSED_THRESHOLD {
                        stressed += 1;
                    }
                    if *value <= SPI_EXTREME_THRESHOLD {
                        extreme += 1;
                    }
                } else {
                    match classify_severity(*value) {
                        DroughtSeverity::Extreme => {
                            extreme += 1;
                            stressed += 1;
                        }
                        DroughtSeverity::Severe | DroughtSeverity::Moderate => stressed += 1,
                        _ => {}
                    }
                }
            }
            let valid_fraction = if pixel_count == 0 {
                0.0
            } else {
                valid as f32 / pixel_count as f32
            };
            let stressed_fraction = if valid == 0 {
                0.0
            } else {
                stressed as f32 / valid as f32
            };
            let extreme_fraction = if valid == 0 {
                0.0
            } else {
                extreme as f32 / valid as f32
            };
            let mean_index = if valid == 0 {
                f32::NAN
            } else {
                (sum / f64::from(valid)) as f32
            };

            let (is_drought, priority, reason_code) = if valid_fraction < min_valid {
                (false, RecommendationPriority::Low, "insufficient_coverage")
            } else if stressed_fraction >= critical {
                (
                    true,
                    RecommendationPriority::Critical,
                    "stressed_fraction_critical",
                )
            } else if stressed_fraction >= warning {
                (
                    true,
                    RecommendationPriority::High,
                    "stressed_fraction_warning",
                )
            } else {
                (false, RecommendationPriority::Low, "nominal")
            };

            DroughtWatchFinding {
                product_id: reading.product_id.clone(),
                index_kind: reading.index_kind.clone(),
                stressed_fraction,
                extreme_fraction,
                mean_index,
                valid_fraction,
                is_drought,
                priority,
                reason_code,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(id: &str, values: Vec<f32>, valid_mask: Vec<bool>) -> DroughtProductReading {
        DroughtProductReading {
            product_id: id.to_string(),
            index_kind: "vci".to_string(),
            values,
            valid_mask,
        }
    }

    #[test]
    fn stress_fractions_and_priorities_are_hand_computed() {
        // [5, 25, 45, 80]: stressed = 2/4 (5 extreme+severe band, 25
        // moderate), extreme = 1/4, mean = 38.75 -> warning (0.3..0.6).
        // [5, 5, 5, 80]: stressed 0.75 -> critical.
        // [45, 50, 60, 80]: no stress -> nominal.
        let findings = evaluate_drought_watch(
            &[
                reading("warn", vec![5.0, 25.0, 45.0, 80.0], vec![true; 4]),
                reading("crit", vec![5.0, 5.0, 5.0, 80.0], vec![true; 4]),
                reading("ok", vec![45.0, 50.0, 60.0, 80.0], vec![true; 4]),
            ],
            &DroughtWatchConfig::default(),
        );
        let warn = &findings[0];
        assert!((warn.stressed_fraction - 0.5).abs() < 1e-6);
        assert!((warn.extreme_fraction - 0.25).abs() < 1e-6);
        assert!((warn.mean_index - 38.75).abs() < 1e-4);
        assert!(warn.is_drought);
        assert_eq!(warn.priority, RecommendationPriority::High);
        assert_eq!(warn.reason_code, "stressed_fraction_warning");

        let crit = &findings[1];
        assert!((crit.stressed_fraction - 0.75).abs() < 1e-6);
        assert_eq!(crit.priority, RecommendationPriority::Critical);
        assert_eq!(crit.reason_code, "stressed_fraction_critical");

        let ok = &findings[2];
        assert_eq!(ok.stressed_fraction, 0.0);
        assert!(!ok.is_drought);
        assert_eq!(ok.reason_code, "nominal");
    }

    #[test]
    fn thin_coverage_never_reports_a_confident_verdict() {
        // Only 1 of 4 pixels valid; with a 0.5 floor that is too thin —
        // even though the one visible pixel is deep drought.
        let findings = evaluate_drought_watch(
            &[reading(
                "thin",
                vec![5.0, f32::NAN, f32::NAN, f32::NAN],
                vec![true, false, false, false],
            )],
            &DroughtWatchConfig {
                min_valid_fraction: Some(0.5),
                ..Default::default()
            },
        );
        let thin = &findings[0];
        assert_eq!(thin.reason_code, "insufficient_coverage");
        assert!(!thin.is_drought);
        assert!((thin.valid_fraction - 0.25).abs() < 1e-6);
        // The stress evidence is still reported for the audit trail.
        assert!((thin.stressed_fraction - 1.0).abs() < 1e-6);
    }

    #[test]
    fn spi_readings_use_mckee_z_score_classes() {
        // SPI [-2.5, -1.2, 0.0, 1.0]: stressed 2/4 (<= -1), extreme 1/4
        // (<= -2), mean -0.675 -> warning. A percent reading of the same
        // numbers would read entirely stressed (< 30), so the scale switch
        // is observable.
        let mut spi = reading("spi-1", vec![-2.5, -1.2, 0.0, 1.0], vec![true; 4]);
        spi.index_kind = "spi".to_string();
        let findings = evaluate_drought_watch(&[spi], &DroughtWatchConfig::default());
        let finding = &findings[0];
        assert!((finding.stressed_fraction - 0.5).abs() < 1e-6);
        assert!((finding.extreme_fraction - 0.25).abs() < 1e-6);
        assert!((finding.mean_index - -0.675).abs() < 1e-4);
        assert_eq!(finding.reason_code, "stressed_fraction_warning");
        assert_eq!(finding.priority, RecommendationPriority::High);
    }

    #[test]
    fn thresholds_are_boundary_inclusive() {
        // Exactly 30% stressed fires the warning; exactly 60% the critical.
        let findings = evaluate_drought_watch(
            &[
                reading(
                    "w",
                    vec![5.0, 5.0, 5.0, 50.0, 50.0, 50.0, 50.0, 50.0, 50.0, 50.0],
                    vec![true; 10],
                ),
                reading(
                    "c",
                    vec![5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 50.0, 50.0, 50.0, 50.0],
                    vec![true; 10],
                ),
            ],
            &DroughtWatchConfig::default(),
        );
        assert_eq!(findings[0].reason_code, "stressed_fraction_warning");
        assert_eq!(findings[1].reason_code, "stressed_fraction_critical");
    }
}
