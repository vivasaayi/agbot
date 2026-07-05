//! Water-priority application composition (Track B phase B4).
//!
//! A pure composition over per-zone soil-moisture / water-deficit stats: mean
//! volumetric soil-moisture -> water-stress class (fixed bands), water deficit
//! (mm below target) + stress -> whether the zone needs irrigation now, and zone
//! area -> action priority (reusing `zone_recommendations::priority_for_zone_area`,
//! mirroring `crop_health_app`). The geo_hub water-priority application-run route
//! maps these findings into catalog `ApplicationFinding`s with lineage.

use crate::zone_recommendations::priority_for_zone_area;
use serde::{Deserialize, Serialize};
use shared::schemas::RecommendationPriority;

/// Water-stress class for a zone based on mean volumetric soil moisture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaterStress {
    Saturated,
    Adequate,
    Mild,
    Moderate,
    Severe,
}

/// Thresholds (volumetric soil-moisture fractions) delimiting the stress bands.
#[derive(Debug, Clone, Copy)]
pub struct WaterStressThresholds {
    /// At or above this the soil is saturated (drainage concern, not deficit).
    pub saturated: f32,
    /// At or above this moisture is adequate.
    pub adequate: f32,
    /// At or above this stress is mild.
    pub mild: f32,
    /// At or above this stress is moderate; below it is severe.
    pub moderate: f32,
}

impl Default for WaterStressThresholds {
    fn default() -> Self {
        Self {
            saturated: 0.40,
            adequate: 0.30,
            mild: 0.22,
            moderate: 0.15,
        }
    }
}

/// Per-zone input to the water-priority composition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneWaterInput {
    pub zone_id: String,
    /// Mean volumetric soil moisture (0..1).
    pub mean_soil_moisture: f32,
    /// Water deficit below the zone's target, in mm. 0 when at/above target.
    #[serde(default)]
    pub water_deficit_mm: f32,
    pub area_m2: f32,
    /// L2/L3 catalog product ids this zone's stats derive from.
    #[serde(default)]
    pub input_product_ids: Vec<String>,
}

/// A water-priority finding for one zone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterPriorityFinding {
    pub zone_id: String,
    pub stress: WaterStress,
    pub needs_irrigation: bool,
    pub priority: RecommendationPriority,
    pub mean_soil_moisture: f32,
    pub water_deficit_mm: f32,
    pub area_m2: f32,
}

/// Classify a zone's mean soil moisture into a water-stress class.
pub fn classify_water_stress(
    mean_soil_moisture: f32,
    thresholds: &WaterStressThresholds,
) -> WaterStress {
    if mean_soil_moisture >= thresholds.saturated {
        WaterStress::Saturated
    } else if mean_soil_moisture >= thresholds.adequate {
        WaterStress::Adequate
    } else if mean_soil_moisture >= thresholds.mild {
        WaterStress::Mild
    } else if mean_soil_moisture >= thresholds.moderate {
        WaterStress::Moderate
    } else {
        WaterStress::Severe
    }
}

/// A zone needs irrigation when its deficit exceeds `deficit_threshold_mm` or its
/// stress is moderate/severe. A saturated zone never needs irrigation.
pub fn needs_irrigation(
    stress: WaterStress,
    water_deficit_mm: f32,
    deficit_threshold_mm: f32,
) -> bool {
    if stress == WaterStress::Saturated {
        return false;
    }
    water_deficit_mm > deficit_threshold_mm
        || matches!(stress, WaterStress::Moderate | WaterStress::Severe)
}

/// A zone is a priority finding when it needs irrigation — the workspace surfaces
/// these first.
pub fn is_priority_zone(finding: &WaterPriorityFinding) -> bool {
    finding.needs_irrigation
}

/// Compose per-zone water-priority findings from zone inputs.
pub fn run_water_priority(
    zones: &[ZoneWaterInput],
    thresholds: &WaterStressThresholds,
    deficit_threshold_mm: f32,
) -> Vec<WaterPriorityFinding> {
    zones
        .iter()
        .map(|zone| {
            let stress = classify_water_stress(zone.mean_soil_moisture, thresholds);
            WaterPriorityFinding {
                zone_id: zone.zone_id.clone(),
                stress,
                needs_irrigation: needs_irrigation(
                    stress,
                    zone.water_deficit_mm,
                    deficit_threshold_mm,
                ),
                priority: priority_for_zone_area(zone.area_m2),
                mean_soil_moisture: zone.mean_soil_moisture,
                water_deficit_mm: zone.water_deficit_mm,
                area_m2: zone.area_m2,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(id: &str, moisture: f32, deficit: f32, area: f32) -> ZoneWaterInput {
        ZoneWaterInput {
            zone_id: id.to_string(),
            mean_soil_moisture: moisture,
            water_deficit_mm: deficit,
            area_m2: area,
            input_product_ids: vec![format!("scene-1:soil_moisture:{id}")],
        }
    }

    #[test]
    fn stress_classes_match_moisture_bands() {
        let t = WaterStressThresholds::default();
        assert_eq!(classify_water_stress(0.45, &t), WaterStress::Saturated);
        assert_eq!(classify_water_stress(0.32, &t), WaterStress::Adequate);
        assert_eq!(classify_water_stress(0.24, &t), WaterStress::Mild);
        assert_eq!(classify_water_stress(0.18, &t), WaterStress::Moderate);
        assert_eq!(classify_water_stress(0.10, &t), WaterStress::Severe);
    }

    #[test]
    fn irrigation_triggered_by_deficit_or_stress_but_not_when_saturated() {
        // Deficit over threshold triggers even when stress is only mild.
        assert!(needs_irrigation(WaterStress::Mild, 8.0, 5.0));
        // Moderate/severe stress triggers regardless of deficit.
        assert!(needs_irrigation(WaterStress::Severe, 0.0, 5.0));
        // Adequate + small deficit does not.
        assert!(!needs_irrigation(WaterStress::Adequate, 2.0, 5.0));
        // Saturated never irrigates, even with a (spurious) deficit.
        assert!(!needs_irrigation(WaterStress::Saturated, 20.0, 5.0));
    }

    #[test]
    fn composition_flags_dry_zones_and_scales_priority_by_area() {
        let t = WaterStressThresholds::default();
        let zones = vec![
            zone("a", 0.33, 1.0, 3000.0),   // Adequate, tiny deficit -> not priority
            zone("b", 0.09, 18.0, 15000.0), // Severe + big deficit -> priority
            zone("c", 0.45, 0.0, 800.0),    // Saturated -> not priority
        ];
        let findings = run_water_priority(&zones, &t, 5.0);
        assert_eq!(findings.len(), 3);
        assert!(!is_priority_zone(&findings[0]));
        assert!(is_priority_zone(&findings[1]));
        assert!(!is_priority_zone(&findings[2]));
        assert_eq!(findings[1].priority, RecommendationPriority::Critical);
        assert_eq!(findings[1].stress, WaterStress::Severe);
        assert_eq!(findings[2].stress, WaterStress::Saturated);
    }
}
