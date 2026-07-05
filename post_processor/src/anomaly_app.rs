//! Anomaly-detection application composition (Track B phase B5).
//!
//! A pure per-zone anomaly composition mirroring `product_anomalies`' pixel-level
//! detector at zone granularity: a zone's index value is flagged when it breaches
//! an absolute low/high threshold or falls outside a statistical band
//! (mean ± multiplier·std) computed over the run's zones. Reuses
//! `ProductAnomalyReasonCode` so the reason semantics match the raster detector.
//! The geo_hub anomaly application-run route maps these into catalog
//! `ApplicationFinding`s with lineage, bridging to Track C alert evaluation.

use crate::product_anomalies::ProductAnomalyReasonCode;
use crate::zone_recommendations::priority_for_zone_area;
use serde::{Deserialize, Serialize};
use shared::schemas::RecommendationPriority;

/// Default statistical-band width, in standard deviations, when the caller does
/// not override it. Matches `index_anomaly`'s default.
pub const DEFAULT_STD_DEV_MULTIPLIER: f32 = 2.0;

/// Per-zone index value to screen for anomalies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneIndexInput {
    pub zone_id: String,
    /// The zone's aggregate index value (e.g. mean NDVI, mean thermal).
    pub index_value: f32,
    pub area_m2: f32,
    /// L2/L3 catalog product ids this zone's value derives from.
    #[serde(default)]
    pub input_product_ids: Vec<String>,
}

/// Detection configuration. Absolute thresholds take precedence over the
/// statistical band; the band is only applied when neither absolute bound fires.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnomalyConfig {
    pub low_threshold: Option<f32>,
    pub high_threshold: Option<f32>,
    /// Band half-width in std deviations. `None` -> `DEFAULT_STD_DEV_MULTIPLIER`.
    pub std_dev_multiplier: Option<f32>,
}

/// A per-zone anomaly finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneAnomalyFinding {
    pub zone_id: String,
    pub index_value: f32,
    pub is_anomaly: bool,
    #[serde(default)]
    pub reason_code: Option<ProductAnomalyReasonCode>,
    /// Distance from the zone-set mean in std deviations (0 when std is 0).
    pub z_score: f32,
    pub priority: RecommendationPriority,
    pub area_m2: f32,
}

/// Population mean + std deviation of the zones' index values.
fn baseline(zones: &[ZoneIndexInput]) -> (f32, f32) {
    if zones.is_empty() {
        return (0.0, 0.0);
    }
    let n = zones.len() as f32;
    let mean = zones.iter().map(|z| z.index_value).sum::<f32>() / n;
    let variance = zones
        .iter()
        .map(|z| {
            let d = z.index_value - mean;
            d * d
        })
        .sum::<f32>()
        / n;
    (mean, variance.sqrt())
}

/// Classify one zone value against the absolute thresholds then the statistical
/// band. Returns the reason code when anomalous, else `None`.
fn classify(
    index_value: f32,
    mean: f32,
    std: f32,
    config: &AnomalyConfig,
) -> Option<ProductAnomalyReasonCode> {
    if let Some(low) = config.low_threshold {
        if index_value < low {
            return Some(ProductAnomalyReasonCode::BelowAbsoluteThreshold);
        }
    }
    if let Some(high) = config.high_threshold {
        if index_value > high {
            return Some(ProductAnomalyReasonCode::AboveAbsoluteThreshold);
        }
    }
    if std > 0.0 {
        let multiplier = config
            .std_dev_multiplier
            .unwrap_or(DEFAULT_STD_DEV_MULTIPLIER);
        let z = (index_value - mean) / std;
        if z < -multiplier {
            return Some(ProductAnomalyReasonCode::BelowStatisticalBand);
        }
        if z > multiplier {
            return Some(ProductAnomalyReasonCode::AboveStatisticalBand);
        }
    }
    None
}

/// An anomalous zone is always a priority finding — these bridge to alerting.
pub fn is_priority_zone(finding: &ZoneAnomalyFinding) -> bool {
    finding.is_anomaly
}

/// Compose per-zone anomaly findings. The statistical band is computed over the
/// supplied zones (the run's own population).
pub fn run_anomaly_detection(
    zones: &[ZoneIndexInput],
    config: &AnomalyConfig,
) -> Vec<ZoneAnomalyFinding> {
    let (mean, std) = baseline(zones);
    zones
        .iter()
        .map(|zone| {
            let reason_code = classify(zone.index_value, mean, std, config);
            let z_score = if std > 0.0 {
                (zone.index_value - mean) / std
            } else {
                0.0
            };
            ZoneAnomalyFinding {
                zone_id: zone.zone_id.clone(),
                index_value: zone.index_value,
                is_anomaly: reason_code.is_some(),
                reason_code,
                z_score,
                priority: priority_for_zone_area(zone.area_m2),
                area_m2: zone.area_m2,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(id: &str, value: f32, area: f32) -> ZoneIndexInput {
        ZoneIndexInput {
            zone_id: id.to_string(),
            index_value: value,
            area_m2: area,
            input_product_ids: vec![format!("scene-1:ndvi:{id}")],
        }
    }

    #[test]
    fn absolute_thresholds_flag_out_of_range_zones() {
        let zones = vec![
            zone("a", 0.15, 3000.0),
            zone("b", 0.5, 3000.0),
            zone("c", 0.95, 3000.0),
        ];
        let config = AnomalyConfig {
            low_threshold: Some(0.2),
            high_threshold: Some(0.8),
            std_dev_multiplier: None,
        };
        let findings = run_anomaly_detection(&zones, &config);
        assert_eq!(
            findings[0].reason_code,
            Some(ProductAnomalyReasonCode::BelowAbsoluteThreshold)
        );
        assert!(!findings[1].is_anomaly);
        assert_eq!(
            findings[2].reason_code,
            Some(ProductAnomalyReasonCode::AboveAbsoluteThreshold)
        );
    }

    #[test]
    fn statistical_band_flags_outlier_against_zone_population() {
        // One zone sits far above the others; no absolute thresholds set.
        let zones = vec![
            zone("a", 0.50, 3000.0),
            zone("b", 0.51, 3000.0),
            zone("c", 0.49, 3000.0),
            zone("d", 0.52, 3000.0),
            zone("outlier", 0.95, 12000.0),
        ];
        let config = AnomalyConfig {
            low_threshold: None,
            high_threshold: None,
            std_dev_multiplier: Some(1.5),
        };
        let findings = run_anomaly_detection(&zones, &config);
        let outlier = findings.iter().find(|f| f.zone_id == "outlier").unwrap();
        assert_eq!(
            outlier.reason_code,
            Some(ProductAnomalyReasonCode::AboveStatisticalBand)
        );
        assert_eq!(outlier.priority, RecommendationPriority::Critical);
        // The clustered zones are nominal.
        assert!(findings.iter().filter(|f| f.is_anomaly).count() == 1);
    }

    #[test]
    fn uniform_zones_have_no_anomalies() {
        let zones = vec![zone("a", 0.5, 3000.0), zone("b", 0.5, 3000.0)];
        let config = AnomalyConfig {
            std_dev_multiplier: Some(2.0),
            ..AnomalyConfig::default()
        };
        let findings = run_anomaly_detection(&zones, &config);
        assert!(findings.iter().all(|f| !f.is_anomaly));
        assert!(findings.iter().all(|f| f.z_score == 0.0));
    }
}
