//! Deterministic advisor rules (Track B/D phase D2).
//!
//! Pure, LLM-free evaluators that turn analysis findings + context (moisture,
//! weather) into proposal drafts. No I/O and no clock: the same inputs always
//! yield the same draft, so the geo_hub proposal-queue adapter can funnel them
//! deterministically. Constructing a draft never implies acceptance — a draft is
//! only ever `Proposed` once queued.

use serde::{Deserialize, Serialize};

/// The kind of remedy an advisor proposal recommends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemedyKind {
    IrrigationCheck,
    ScoutMission,
    Treatment,
    Refly,
    ManualReview,
}

impl RemedyKind {
    /// The proposal-queue `action_category` for this remedy.
    pub fn action_category(self) -> &'static str {
        match self {
            RemedyKind::IrrigationCheck => "irrigation",
            RemedyKind::ScoutMission => "scout",
            RemedyKind::Treatment => "treatment",
            RemedyKind::Refly => "refly",
            RemedyKind::ManualReview => "review",
        }
    }
}

/// A proposal draft an advisor rule produced from a source finding. The geo_hub
/// adapter maps this into `proposal_queue::create_proposal` (source_kind =
/// finding).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdvisorProposal {
    /// The finding this proposal was raised from (lineage source).
    pub source_finding_id: String,
    pub field_id: Option<String>,
    pub remedy: RemedyKind,
    pub title: String,
    pub priority: String,
    pub rationale: String,
}

/// Inputs to the water-stress rule, derived from an NDVI-trend finding plus soil
/// and weather context.
#[derive(Debug, Clone)]
pub struct WaterStressInputs {
    pub finding_id: String,
    pub field_id: Option<String>,
    /// The NDVI trend finding indicates a declining canopy.
    pub ndvi_declining: bool,
    /// Latest soil-moisture reading (volumetric %), when available.
    pub soil_moisture_pct: Option<f32>,
    /// Forecast rainfall over the planning window (mm).
    pub rain_forecast_mm: f32,
}

/// Water stress warrants an irrigation check when the canopy is declining and
/// there is no relief in sight — either measured moisture is low or no
/// meaningful rain is forecast. Deterministic thresholds.
pub fn evaluate_water_stress_rule(input: &WaterStressInputs) -> Option<AdvisorProposal> {
    if !input.ndvi_declining {
        return None;
    }
    let moisture_low = input.soil_moisture_pct.map(|m| m < 20.0).unwrap_or(false);
    let dry_forecast = input.rain_forecast_mm < 5.0;
    if !(moisture_low || dry_forecast) {
        return None;
    }
    let rationale = match input.soil_moisture_pct {
        Some(m) => format!(
            "Declining NDVI with soil moisture {m:.0}% and {:.0} mm forecast rain.",
            input.rain_forecast_mm
        ),
        None => format!(
            "Declining NDVI with only {:.0} mm forecast rain and no moisture reading.",
            input.rain_forecast_mm
        ),
    };
    let priority = if moisture_low && dry_forecast {
        "high"
    } else {
        "medium"
    };
    Some(AdvisorProposal {
        source_finding_id: input.finding_id.clone(),
        field_id: input.field_id.clone(),
        remedy: RemedyKind::IrrigationCheck,
        title: "Irrigation check for water-stressed zone".to_string(),
        priority: priority.to_string(),
        rationale,
    })
}

/// Inputs to the pest-hotspot rule, derived from an anomaly finding plus weather.
#[derive(Debug, Clone)]
pub struct PestHotspotInputs {
    pub finding_id: String,
    pub field_id: Option<String>,
    /// Anomaly severity from the finding: "low" | "medium" | "high" | "critical".
    pub anomaly_severity: String,
    pub temperature_c: f32,
    pub humidity_pct: f32,
}

/// A pest hotspot warrants a scouting mission when a significant anomaly
/// coincides with pest-favorable weather (warm and humid). Deterministic.
pub fn evaluate_pest_hotspot_rule(input: &PestHotspotInputs) -> Option<AdvisorProposal> {
    let severe = matches!(input.anomaly_severity.as_str(), "high" | "critical");
    let pest_favorable = input.temperature_c >= 18.0 && input.humidity_pct >= 60.0;
    if !(severe && pest_favorable) {
        return None;
    }
    let priority = if input.anomaly_severity == "critical" {
        "high"
    } else {
        "medium"
    };
    Some(AdvisorProposal {
        source_finding_id: input.finding_id.clone(),
        field_id: input.field_id.clone(),
        remedy: RemedyKind::ScoutMission,
        title: "Scout pest hotspot".to_string(),
        priority: priority.to_string(),
        rationale: format!(
            "{} anomaly under pest-favorable weather ({:.0} C, {:.0}% RH).",
            input.anomaly_severity, input.temperature_c, input.humidity_pct
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_stress_fires_on_declining_ndvi_and_dry_conditions() {
        let base = WaterStressInputs {
            finding_id: "f1".to_string(),
            field_id: Some("field-1".to_string()),
            ndvi_declining: true,
            soil_moisture_pct: Some(15.0),
            rain_forecast_mm: 1.0,
        };
        let proposal = evaluate_water_stress_rule(&base).expect("should fire");
        assert_eq!(proposal.remedy, RemedyKind::IrrigationCheck);
        assert_eq!(proposal.priority, "high");
        assert_eq!(proposal.source_finding_id, "f1");
    }

    #[test]
    fn water_stress_silent_when_healthy_or_wet() {
        // Not declining -> no proposal.
        let mut input = WaterStressInputs {
            finding_id: "f1".to_string(),
            field_id: None,
            ndvi_declining: false,
            soil_moisture_pct: Some(10.0),
            rain_forecast_mm: 0.0,
        };
        assert!(evaluate_water_stress_rule(&input).is_none());
        // Declining but moist + rain coming -> no proposal.
        input.ndvi_declining = true;
        input.soil_moisture_pct = Some(35.0);
        input.rain_forecast_mm = 20.0;
        assert!(evaluate_water_stress_rule(&input).is_none());
    }

    #[test]
    fn water_stress_medium_when_only_one_signal() {
        let input = WaterStressInputs {
            finding_id: "f1".to_string(),
            field_id: None,
            ndvi_declining: true,
            soil_moisture_pct: Some(35.0), // not low
            rain_forecast_mm: 1.0,         // but dry forecast
        };
        assert_eq!(
            evaluate_water_stress_rule(&input).unwrap().priority,
            "medium"
        );
    }

    #[test]
    fn pest_hotspot_fires_on_severe_anomaly_and_favorable_weather() {
        let input = PestHotspotInputs {
            finding_id: "a1".to_string(),
            field_id: Some("field-1".to_string()),
            anomaly_severity: "critical".to_string(),
            temperature_c: 24.0,
            humidity_pct: 75.0,
        };
        let proposal = evaluate_pest_hotspot_rule(&input).expect("should fire");
        assert_eq!(proposal.remedy, RemedyKind::ScoutMission);
        assert_eq!(proposal.priority, "high");
    }

    #[test]
    fn pest_hotspot_silent_when_mild_or_unfavorable() {
        // Low severity -> silent.
        let mut input = PestHotspotInputs {
            finding_id: "a1".to_string(),
            field_id: None,
            anomaly_severity: "low".to_string(),
            temperature_c: 24.0,
            humidity_pct: 75.0,
        };
        assert!(evaluate_pest_hotspot_rule(&input).is_none());
        // Severe but cold/dry -> silent.
        input.anomaly_severity = "high".to_string();
        input.temperature_c = 8.0;
        input.humidity_pct = 30.0;
        assert!(evaluate_pest_hotspot_rule(&input).is_none());
    }
}
