//! Per-field water-balance summary (satellite pipeline batch 41).
//!
//! Ties the water story's two sides together deterministically:
//!
//! - **Supply**: the field's water-extent area series (m^2 per
//!   observation, from the batch-15 `water_extent` products), optionally
//!   anchored by the batch-39 seasonality areas (permanent/seasonal), plus
//!   region-mean precipitation totals (CHIRPS `precipitation` products).
//! - **Demand**: the batch-40 `et_fraction` observations (mean evaporative
//!   fraction per scene — a dimensionless demand index; mm/day awaits a
//!   reference-ET source and is NOT faked here).
//!
//! The summary classifies supply trend (first-vs-last relative area
//! change), demand level (mean ET fraction terciles), and a combined
//! reason-coded status. Every threshold is a documented constant; missing
//! evidence yields `InsufficientEvidence`, never a guessed verdict.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use shared::product_graph::ProductRecordDraft;
use thiserror::Error;

use crate::evidence::{deterministic_fingerprint, AnalysisEvidenceError};
use crate::l3_product::{to_l3_draft, L3DraftContext};

/// Relative area change beyond ±this classifies the supply trend.
pub const SUPPLY_TREND_THRESHOLD: f32 = 0.10;
/// Mean ET fraction below this = low demand; above [`DEMAND_HIGH_MIN`] =
/// high; between = moderate.
pub const DEMAND_LOW_MAX: f32 = 1.0 / 3.0;
pub const DEMAND_HIGH_MIN: f32 = 2.0 / 3.0;
/// Supply trend needs at least this many area observations.
pub const MIN_SUPPLY_OBSERVATIONS: usize = 2;

/// One supply observation: a water-extent product's area.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyObservation {
    pub product_id: String,
    pub observed_on: NaiveDate,
    pub water_area_m2: f64,
}

/// One demand observation: a scene's mean ET fraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemandObservation {
    pub product_id: String,
    pub observed_on: NaiveDate,
    pub mean_et_fraction: f32,
}

/// One precipitation observation: a product's region-mean rainfall (mm).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrecipitationObservation {
    pub product_id: String,
    pub observed_on: NaiveDate,
    pub mean_mm: f32,
}

/// Optional seasonality anchor (from the batch-39 product's parameters).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalityAnchor {
    pub product_id: String,
    pub permanent_area_m2: Option<f64>,
    pub seasonal_area_m2: Option<f64>,
}

/// Inputs to one balance summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WaterBalanceInputs {
    pub supply: Vec<SupplyObservation>,
    pub demand: Vec<DemandObservation>,
    pub precipitation: Vec<PrecipitationObservation>,
    pub seasonality: Option<SeasonalityAnchor>,
}

impl WaterBalanceInputs {
    fn is_empty(&self) -> bool {
        self.supply.is_empty()
            && self.demand.is_empty()
            && self.precipitation.is_empty()
            && self.seasonality.is_none()
    }
}

/// Direction of the water-area series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplyTrend {
    Declining,
    Stable,
    Increasing,
    /// Fewer than [`MIN_SUPPLY_OBSERVATIONS`] observations.
    Unknown,
}

/// Demand level from the mean ET fraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DemandLevel {
    Low,
    Moderate,
    High,
    /// No ET observations.
    Unknown,
}

/// The combined reason-coded verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BalanceStatus {
    /// Supply declining while demand is high.
    DeficitRisk,
    /// Supply declining under moderate demand, or stable under high.
    Watch,
    /// No stress signal in either direction.
    Adequate,
    /// Supply increasing while demand is low/moderate.
    Surplus,
    /// Supply or demand side has no evidence — no verdict is guessed.
    InsufficientEvidence,
}

/// A completed balance summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterBalanceSummary {
    pub supply_trend: SupplyTrend,
    /// `(last - first) / first` over the area series (None without trend).
    pub relative_area_change: Option<f32>,
    pub first_water_area_m2: Option<f64>,
    pub last_water_area_m2: Option<f64>,
    pub permanent_area_m2: Option<f64>,
    pub seasonal_area_m2: Option<f64>,
    pub demand_level: DemandLevel,
    pub mean_et_fraction: Option<f32>,
    /// Sum of region-mean precipitation over the window (mm).
    pub total_precipitation_mm: Option<f32>,
    pub status: BalanceStatus,
    /// Why the status was chosen (a stable machine token).
    pub status_reason: &'static str,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    /// Every contributing product id (lineage), deterministic order.
    pub input_product_ids: Vec<String>,
    pub input_hash: String,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum WaterBalanceError {
    #[error("no supply, demand, precipitation, or seasonality inputs at all")]
    NoInputs,
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] AnalysisEvidenceError),
}

/// Summarize a field's water balance. Deterministic; date order is
/// established internally.
pub fn summarize_water_balance(
    inputs: &WaterBalanceInputs,
) -> Result<WaterBalanceSummary, WaterBalanceError> {
    if inputs.is_empty() {
        return Err(WaterBalanceError::NoInputs);
    }

    // Supply trend over the date-sorted area series.
    let mut supply = inputs.supply.clone();
    supply.sort_by(|a, b| {
        a.observed_on
            .cmp(&b.observed_on)
            .then(a.product_id.cmp(&b.product_id))
    });
    let (supply_trend, relative_change, first_area, last_area) =
        if supply.len() >= MIN_SUPPLY_OBSERVATIONS {
            let first = supply.first().expect("non-empty").water_area_m2;
            let last = supply.last().expect("non-empty").water_area_m2;
            let change = if first > 0.0 {
                ((last - first) / first) as f32
            } else if last > 0.0 {
                // From zero to something: unambiguous increase.
                f32::INFINITY
            } else {
                0.0
            };
            let trend = if change <= -SUPPLY_TREND_THRESHOLD {
                SupplyTrend::Declining
            } else if change >= SUPPLY_TREND_THRESHOLD {
                SupplyTrend::Increasing
            } else {
                SupplyTrend::Stable
            };
            (trend, Some(change), Some(first), Some(last))
        } else {
            (SupplyTrend::Unknown, None, None, None)
        };

    // Demand level from the mean of scene mean-fractions.
    let (demand_level, mean_et) = if inputs.demand.is_empty() {
        (DemandLevel::Unknown, None)
    } else {
        let mean = inputs
            .demand
            .iter()
            .map(|d| f64::from(d.mean_et_fraction))
            .sum::<f64>() as f32
            / inputs.demand.len() as f32;
        let level = if mean < DEMAND_LOW_MAX {
            DemandLevel::Low
        } else if mean >= DEMAND_HIGH_MIN {
            DemandLevel::High
        } else {
            DemandLevel::Moderate
        };
        (level, Some(mean))
    };

    let total_precipitation_mm = if inputs.precipitation.is_empty() {
        None
    } else {
        Some(inputs.precipitation.iter().map(|p| p.mean_mm).sum())
    };

    // Deterministic verdict table (documented in the module docs).
    let (status, status_reason) = match (supply_trend, demand_level) {
        (SupplyTrend::Unknown, _) | (_, DemandLevel::Unknown) => (
            BalanceStatus::InsufficientEvidence,
            "missing_supply_or_demand_evidence",
        ),
        (SupplyTrend::Declining, DemandLevel::High) => {
            (BalanceStatus::DeficitRisk, "supply_declining_demand_high")
        }
        (SupplyTrend::Declining, DemandLevel::Moderate) => {
            (BalanceStatus::Watch, "supply_declining_demand_moderate")
        }
        (SupplyTrend::Stable, DemandLevel::High) => {
            (BalanceStatus::Watch, "supply_stable_demand_high")
        }
        (SupplyTrend::Increasing, DemandLevel::Low | DemandLevel::Moderate) => (
            BalanceStatus::Surplus,
            "supply_increasing_demand_manageable",
        ),
        _ => (BalanceStatus::Adequate, "no_stress_signal"),
    };

    // Lineage + period over every contributing product.
    let mut input_product_ids: Vec<String> = supply
        .iter()
        .map(|s| s.product_id.clone())
        .chain(inputs.demand.iter().map(|d| d.product_id.clone()))
        .chain(inputs.precipitation.iter().map(|p| p.product_id.clone()))
        .chain(inputs.seasonality.iter().map(|s| s.product_id.clone()))
        .collect();
    input_product_ids.sort();
    input_product_ids.dedup();

    let all_dates: Vec<NaiveDate> = supply
        .iter()
        .map(|s| s.observed_on)
        .chain(inputs.demand.iter().map(|d| d.observed_on))
        .chain(inputs.precipitation.iter().map(|p| p.observed_on))
        .collect();
    let period_start = all_dates.iter().min().copied();
    let period_end = all_dates.iter().max().copied();

    let input_hash = deterministic_fingerprint(&(
        "water_balance_v1",
        inputs,
        SUPPLY_TREND_THRESHOLD,
        DEMAND_LOW_MAX,
        DEMAND_HIGH_MIN,
    ))?;

    Ok(WaterBalanceSummary {
        supply_trend,
        relative_area_change: relative_change,
        first_water_area_m2: first_area,
        last_water_area_m2: last_area,
        permanent_area_m2: inputs
            .seasonality
            .as_ref()
            .and_then(|s| s.permanent_area_m2),
        seasonal_area_m2: inputs.seasonality.as_ref().and_then(|s| s.seasonal_area_m2),
        demand_level,
        mean_et_fraction: mean_et,
        total_precipitation_mm,
        status,
        status_reason,
        period_start,
        period_end,
        input_product_ids,
        input_hash,
    })
}

/// Scope a balance L3 draft cannot derive from the inputs alone.
#[derive(Debug, Clone)]
pub struct WaterBalanceL3Scope {
    pub field_id: String,
    pub season_id: String,
}

/// Map a balance summary to an L3 catalog draft (kind `water_balance`,
/// JSON artifact) with lineage to every contributing product.
pub fn water_balance_l3_draft(
    summary: &WaterBalanceSummary,
    scope: &WaterBalanceL3Scope,
) -> ProductRecordDraft {
    to_l3_draft(&L3DraftContext {
        kind: "water_balance".to_string(),
        algorithm_id: "water.balance_summary".to_string(),
        algorithm_version: "1.0.0".to_string(),
        field_id: scope.field_id.clone(),
        season_id: scope.season_id.clone(),
        scene_id: None,
        temporal_start: summary
            .period_start
            .map(|d| format!("{d}T00:00:00Z"))
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
        temporal_end: summary
            .period_end
            .map(|d| format!("{d}T23:59:59Z"))
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
        input_product_ids: summary.input_product_ids.clone(),
        parameters: serde_json::json!({
            "status": summary.status,
            "status_reason": summary.status_reason,
            "supply_trend": summary.supply_trend,
            "relative_area_change": summary.relative_area_change,
            "demand_level": summary.demand_level,
            "mean_et_fraction": summary.mean_et_fraction,
            "total_precipitation_mm": summary.total_precipitation_mm,
            "permanent_area_m2": summary.permanent_area_m2,
            "seasonal_area_m2": summary.seasonal_area_m2,
            "thresholds": {
                "supply_trend": SUPPLY_TREND_THRESHOLD,
                "demand_low_max": DEMAND_LOW_MAX,
                "demand_high_min": DEMAND_HIGH_MIN,
            },
        }),
        confidence: None,
        confidence_method: None,
        evidence_digests: vec![summary.input_hash.clone()],
        source_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, d).unwrap()
    }

    fn supply(id: &str, on: NaiveDate, area: f64) -> SupplyObservation {
        SupplyObservation {
            product_id: id.to_string(),
            observed_on: on,
            water_area_m2: area,
        }
    }

    fn demand(id: &str, on: NaiveDate, fraction: f32) -> DemandObservation {
        DemandObservation {
            product_id: id.to_string(),
            observed_on: on,
            mean_et_fraction: fraction,
        }
    }

    #[test]
    fn deficit_risk_is_declining_supply_under_high_demand() {
        // Area 300 -> 100: change (100-300)/300 = -2/3 -> declining.
        // ET means 0.8 / 0.7 -> mean 0.75 >= 2/3 -> high demand.
        let summary = summarize_water_balance(&WaterBalanceInputs {
            supply: vec![
                supply("w1", date(1, 15), 300.0),
                supply("w2", date(6, 15), 100.0),
            ],
            demand: vec![demand("e1", date(3, 1), 0.8), demand("e2", date(5, 1), 0.7)],
            precipitation: vec![PrecipitationObservation {
                product_id: "p1".to_string(),
                observed_on: date(2, 1),
                mean_mm: 5.0,
            }],
            seasonality: Some(SeasonalityAnchor {
                product_id: "s1".to_string(),
                permanent_area_m2: Some(100.0),
                seasonal_area_m2: Some(200.0),
            }),
        })
        .unwrap();
        assert_eq!(summary.supply_trend, SupplyTrend::Declining);
        assert!((summary.relative_area_change.unwrap() - (-2.0 / 3.0)).abs() < 1e-6);
        assert_eq!(summary.demand_level, DemandLevel::High);
        assert!((summary.mean_et_fraction.unwrap() - 0.75).abs() < 1e-6);
        assert_eq!(summary.status, BalanceStatus::DeficitRisk);
        assert_eq!(summary.status_reason, "supply_declining_demand_high");
        assert_eq!(summary.total_precipitation_mm, Some(5.0));
        assert_eq!(summary.permanent_area_m2, Some(100.0));
        // Lineage covers all six inputs (2 supply + 2 demand + precip +
        // seasonality), sorted + deduped.
        assert_eq!(summary.input_product_ids.len(), 6);
        assert_eq!(summary.period_start, Some(date(1, 15)));
        assert_eq!(summary.period_end, Some(date(6, 15)));
    }

    #[test]
    fn thresholds_and_other_verdicts_are_pinned() {
        // ±10% is the trend boundary: +9% = stable, +11% = increasing.
        let base = |last: f64, et: f32| WaterBalanceInputs {
            supply: vec![
                supply("w1", date(1, 1), 100.0),
                supply("w2", date(2, 1), last),
            ],
            demand: vec![demand("e1", date(1, 15), et)],
            ..Default::default()
        };
        let stable = summarize_water_balance(&base(109.0, 0.5)).unwrap();
        assert_eq!(stable.supply_trend, SupplyTrend::Stable);
        assert_eq!(stable.status, BalanceStatus::Adequate);

        let surplus = summarize_water_balance(&base(111.0, 0.2)).unwrap();
        assert_eq!(surplus.supply_trend, SupplyTrend::Increasing);
        assert_eq!(surplus.status, BalanceStatus::Surplus);

        // Stable supply under high demand is a watch, not adequate.
        let watch = summarize_water_balance(&base(100.0, 0.7)).unwrap();
        assert_eq!(watch.status, BalanceStatus::Watch);
        assert_eq!(watch.status_reason, "supply_stable_demand_high");

        // Declining under moderate demand is also a watch.
        let watch2 = summarize_water_balance(&base(50.0, 0.5)).unwrap();
        assert_eq!(watch2.status, BalanceStatus::Watch);
        assert_eq!(watch2.status_reason, "supply_declining_demand_moderate");
    }

    #[test]
    fn missing_evidence_never_guesses() {
        // One supply point: trend unknown -> insufficient evidence, even
        // with demand present.
        let summary = summarize_water_balance(&WaterBalanceInputs {
            supply: vec![supply("w1", date(1, 1), 100.0)],
            demand: vec![demand("e1", date(1, 15), 0.9)],
            ..Default::default()
        })
        .unwrap();
        assert_eq!(summary.supply_trend, SupplyTrend::Unknown);
        assert_eq!(summary.status, BalanceStatus::InsufficientEvidence);

        // Demand missing likewise.
        let summary = summarize_water_balance(&WaterBalanceInputs {
            supply: vec![
                supply("w1", date(1, 1), 100.0),
                supply("w2", date(2, 1), 50.0),
            ],
            ..Default::default()
        })
        .unwrap();
        assert_eq!(summary.demand_level, DemandLevel::Unknown);
        assert_eq!(summary.status, BalanceStatus::InsufficientEvidence);

        // Nothing at all is a typed error.
        assert!(matches!(
            summarize_water_balance(&WaterBalanceInputs::default()),
            Err(WaterBalanceError::NoInputs)
        ));
    }

    #[test]
    fn draft_is_identity_bearing_with_full_lineage() {
        let summary = summarize_water_balance(&WaterBalanceInputs {
            supply: vec![
                supply("w1", date(1, 1), 100.0),
                supply("w2", date(2, 1), 40.0),
            ],
            demand: vec![demand("e1", date(1, 15), 0.9)],
            ..Default::default()
        })
        .unwrap();
        let draft = water_balance_l3_draft(
            &summary,
            &WaterBalanceL3Scope {
                field_id: "field-1".to_string(),
                season_id: "2026".to_string(),
            },
        );
        assert_eq!(draft.kind, "water_balance");
        assert_eq!(draft.inputs.len(), 3);
        assert_eq!(draft.parameters["status"], "deficit_risk");
        let threshold = draft.parameters["thresholds"]["supply_trend"]
            .as_f64()
            .unwrap();
        assert!((threshold - 0.1).abs() < 1e-6, "{threshold}");
    }
}
