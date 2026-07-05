//! Crop-health application composition (Track B phase B2).
//!
//! A pure composition over the existing analyses: per-zone NDVI level ->
//! vegetation-health class (mirroring `ndvi_analysis`), epoch-to-epoch NDVI
//! delta -> trend direction, and zone area -> action priority (reusing
//! `zone_recommendations::priority_for_zone_area`). The geo_hub application-run
//! route maps these findings into catalog `ApplicationFinding`s with lineage.

use crate::ndvi_analysis::{NdviThresholds, VegetationHealth};
use crate::zone_recommendations::priority_for_zone_area;
use serde::{Deserialize, Serialize};
use shared::schemas::RecommendationPriority;

/// Direction of the NDVI trend for a zone between two epochs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrendDirection {
    Improving,
    Stable,
    Declining,
}

/// Per-zone input to the crop-health composition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneHealthInput {
    pub zone_id: String,
    pub mean_ndvi: f32,
    /// NDVI change vs the prior epoch (index_trend delta); 0 when unavailable.
    #[serde(default)]
    pub ndvi_delta: f32,
    pub area_m2: f32,
    /// L2 catalog product ids this zone's stats derive from.
    #[serde(default)]
    pub input_product_ids: Vec<String>,
}

/// A crop-health finding for one zone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CropHealthFinding {
    pub zone_id: String,
    pub health: VegetationHealth,
    pub trend: TrendDirection,
    pub priority: RecommendationPriority,
    pub mean_ndvi: f32,
    pub ndvi_delta: f32,
    pub area_m2: f32,
}

/// Classify a zone's mean NDVI into a vegetation-health class. Mirrors
/// `ndvi_analysis`'s assessment (no-vegetation threshold from `thresholds`, then
/// fixed bands at 0.3/0.4/0.6/0.8).
pub fn classify_vegetation_health(mean_ndvi: f32, thresholds: &NdviThresholds) -> VegetationHealth {
    if mean_ndvi < thresholds.no_vegetation {
        VegetationHealth::NoVegetation
    } else if mean_ndvi < 0.3 {
        VegetationHealth::Critical
    } else if mean_ndvi < 0.4 {
        VegetationHealth::Poor
    } else if mean_ndvi < 0.6 {
        VegetationHealth::Fair
    } else if mean_ndvi < 0.8 {
        VegetationHealth::Good
    } else {
        VegetationHealth::Excellent
    }
}

/// Classify an NDVI delta into a trend direction. `epsilon` is the dead-band
/// within which a change is treated as stable.
pub fn trend_direction(ndvi_delta: f32, epsilon: f32) -> TrendDirection {
    if ndvi_delta > epsilon {
        TrendDirection::Improving
    } else if ndvi_delta < -epsilon {
        TrendDirection::Declining
    } else {
        TrendDirection::Stable
    }
}

/// A zone is a priority-scored finding when its health is Poor/Critical or its
/// trend is declining — the workspace surfaces these first.
pub fn is_priority_zone(finding: &CropHealthFinding) -> bool {
    matches!(
        finding.health,
        VegetationHealth::Poor | VegetationHealth::Critical
    ) || finding.trend == TrendDirection::Declining
}

/// Compose per-zone crop-health findings from zone inputs.
pub fn run_crop_health(
    zones: &[ZoneHealthInput],
    thresholds: &NdviThresholds,
    trend_epsilon: f32,
) -> Vec<CropHealthFinding> {
    zones
        .iter()
        .map(|zone| CropHealthFinding {
            zone_id: zone.zone_id.clone(),
            health: classify_vegetation_health(zone.mean_ndvi, thresholds),
            trend: trend_direction(zone.ndvi_delta, trend_epsilon),
            priority: priority_for_zone_area(zone.area_m2),
            mean_ndvi: zone.mean_ndvi,
            ndvi_delta: zone.ndvi_delta,
            area_m2: zone.area_m2,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(id: &str, mean: f32, delta: f32, area: f32) -> ZoneHealthInput {
        ZoneHealthInput {
            zone_id: id.to_string(),
            mean_ndvi: mean,
            ndvi_delta: delta,
            area_m2: area,
            input_product_ids: vec![format!("scene-1:ndvi:{id}")],
        }
    }

    #[test]
    fn health_classes_match_ndvi_bands() {
        let t = NdviThresholds::default();
        assert_eq!(
            classify_vegetation_health(0.05, &t),
            VegetationHealth::NoVegetation
        );
        assert_eq!(
            classify_vegetation_health(0.25, &t),
            VegetationHealth::Critical
        );
        assert_eq!(classify_vegetation_health(0.35, &t), VegetationHealth::Poor);
        assert_eq!(classify_vegetation_health(0.5, &t), VegetationHealth::Fair);
        assert_eq!(classify_vegetation_health(0.7, &t), VegetationHealth::Good);
        assert_eq!(
            classify_vegetation_health(0.85, &t),
            VegetationHealth::Excellent
        );
    }

    #[test]
    fn trend_dead_band_is_stable() {
        assert_eq!(trend_direction(0.05, 0.02), TrendDirection::Improving);
        assert_eq!(trend_direction(-0.05, 0.02), TrendDirection::Declining);
        assert_eq!(trend_direction(0.01, 0.02), TrendDirection::Stable);
    }

    #[test]
    fn composition_flags_declining_and_unhealthy_zones() {
        let t = NdviThresholds::default();
        let zones = vec![
            zone("a", 0.75, 0.03, 3000.0),   // Good, improving -> not priority
            zone("b", 0.28, -0.10, 12000.0), // Critical, declining -> priority
            zone("c", 0.65, -0.08, 800.0),   // Good but declining -> priority
        ];
        let findings = run_crop_health(&zones, &t, 0.02);
        assert_eq!(findings.len(), 3);
        assert!(!is_priority_zone(&findings[0]));
        assert!(is_priority_zone(&findings[1]));
        assert!(is_priority_zone(&findings[2]));
        // Priority scales with zone area.
        assert_eq!(findings[1].priority, RecommendationPriority::Critical);
        assert_eq!(findings[0].priority, RecommendationPriority::High);
    }
}
