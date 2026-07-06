//! Farmer-portal aggregation logic (batch F-B2): pure functions that turn
//! already-fetched field rows (findings, recommendations, scenes, alerts)
//! into per-field cards and a single-field overview.
//!
//! No I/O here — handlers in `routes/portal.rs` fetch org-scoped rows and
//! delegate assembly to this module so ordering and counting rules stay
//! deterministic and unit-testable.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use std::collections::BTreeMap;

use crate::portal_auth::parse_stored_timestamp;

/// Window (days) for the "recent alerts" badge on cards and overviews.
pub const RECENT_ALERT_WINDOW_DAYS: i64 = 7;

/// How many findings the overview keeps, newest first.
pub const RECENT_FINDING_LIMIT: usize = 20;

/// How many open recommendations the overview keeps, highest priority first.
pub const OPEN_RECOMMENDATION_LIMIT: usize = 5;

/// Bucket used when a finding row carries no severity.
pub const UNSPECIFIED_SEVERITY: &str = "unspecified";

const RECOMMENDATION_OPEN_STATUS: &str = "open";

// ---------------------------------------------------------------------------
// Inputs: plain row data, already fetched and already org-scoped.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct FieldInput {
    pub field_id: String,
    pub farm_id: Option<String>,
    pub name: String,
    pub crop: Option<String>,
    pub season: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingInput {
    pub finding_id: String,
    pub kind: String,
    pub severity: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecommendationInput {
    pub recommendation_id: String,
    pub title: String,
    pub category: Option<String>,
    pub priority: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SceneInput {
    pub scene_id: String,
    pub sensor: String,
    pub acquired_at: String,
}

// ---------------------------------------------------------------------------
// Outputs.
// ---------------------------------------------------------------------------

/// One field on the portal dashboard, with badge counts.
#[derive(Debug, Clone, Serialize)]
pub struct FieldCard {
    pub field_id: String,
    pub farm_id: Option<String>,
    pub name: String,
    pub crop: Option<String>,
    pub season: Option<String>,
    /// Severity of the most recent finding that carries a severity.
    pub latest_finding_severity: Option<String>,
    pub open_recommendations: usize,
    pub latest_scene_at: Option<String>,
    pub recent_alerts_7d: usize,
}

/// Detail view for one field.
#[derive(Debug, Clone, Serialize)]
pub struct FieldOverview {
    pub field: FieldInput,
    pub latest_scene: Option<SceneInput>,
    /// Count of every finding for the field, keyed by severity bucket.
    pub findings_by_severity: BTreeMap<String, usize>,
    /// Newest findings first, capped at [`RECENT_FINDING_LIMIT`].
    pub recent_findings: Vec<FindingInput>,
    /// Open recommendations, highest priority first, capped at
    /// [`OPEN_RECOMMENDATION_LIMIT`].
    pub open_recommendations: Vec<RecommendationInput>,
    /// Total open recommendations before capping.
    pub open_recommendation_count: usize,
    /// Alerts fired within [`RECENT_ALERT_WINDOW_DAYS`] of `now`.
    pub recent_alert_count: usize,
}

// ---------------------------------------------------------------------------
// Ordering helpers.
// ---------------------------------------------------------------------------

/// Rank a finding/alert severity: lower is more severe. Unknown labels sort
/// after the known scale so bad data never outranks a real critical.
pub fn severity_rank(severity: &str) -> u8 {
    match severity.trim().to_ascii_lowercase().as_str() {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    }
}

/// Rank a recommendation priority: lower is more urgent
/// (critical > high > medium > low).
pub fn priority_rank(priority: &str) -> u8 {
    match priority.trim().to_ascii_lowercase().as_str() {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    }
}

/// Count timestamps that fall within the trailing `window_days` of `now`
/// (future timestamps count as recent rather than silently vanishing).
pub fn count_within_days(timestamps: &[String], now: DateTime<Utc>, window_days: i64) -> usize {
    let cutoff = now - Duration::days(window_days);
    timestamps
        .iter()
        .filter_map(|raw| parse_stored_timestamp(raw))
        .filter(|ts| *ts >= cutoff)
        .count()
}

/// Severity of the most recent finding that has a severity, by `created_at`.
pub fn latest_finding_severity(findings: &[FindingInput]) -> Option<String> {
    findings
        .iter()
        .filter(|finding| finding.severity.is_some())
        .max_by(|a, b| a.created_at.cmp(&b.created_at))
        .and_then(|finding| finding.severity.clone())
}

fn count_open(recommendations: &[RecommendationInput]) -> usize {
    recommendations
        .iter()
        .filter(|rec| rec.status == RECOMMENDATION_OPEN_STATUS)
        .count()
}

// ---------------------------------------------------------------------------
// Assembly.
// ---------------------------------------------------------------------------

/// Build a dashboard card from a field's already-fetched rows.
pub fn build_field_card(
    field: FieldInput,
    findings: &[FindingInput],
    recommendations: &[RecommendationInput],
    latest_scene_at: Option<String>,
    alert_fired_ats: &[String],
    now: DateTime<Utc>,
) -> FieldCard {
    FieldCard {
        latest_finding_severity: latest_finding_severity(findings),
        open_recommendations: count_open(recommendations),
        latest_scene_at,
        recent_alerts_7d: count_within_days(alert_fired_ats, now, RECENT_ALERT_WINDOW_DAYS),
        field_id: field.field_id,
        farm_id: field.farm_id,
        name: field.name,
        crop: field.crop,
        season: field.season,
    }
}

/// Build the field detail overview from a field's already-fetched rows.
pub fn build_field_overview(
    field: FieldInput,
    latest_scene: Option<SceneInput>,
    findings: Vec<FindingInput>,
    recommendations: Vec<RecommendationInput>,
    alert_fired_ats: &[String],
    now: DateTime<Utc>,
) -> FieldOverview {
    let mut findings_by_severity: BTreeMap<String, usize> = BTreeMap::new();
    for finding in &findings {
        let bucket = finding
            .severity
            .as_deref()
            .map(str::to_string)
            .unwrap_or_else(|| UNSPECIFIED_SEVERITY.to_string());
        *findings_by_severity.entry(bucket).or_insert(0) += 1;
    }

    let mut recent_findings = findings;
    recent_findings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    recent_findings.truncate(RECENT_FINDING_LIMIT);

    let mut open_recommendations: Vec<RecommendationInput> = recommendations
        .into_iter()
        .filter(|rec| rec.status == RECOMMENDATION_OPEN_STATUS)
        .collect();
    let open_recommendation_count = open_recommendations.len();
    open_recommendations.sort_by(|a, b| {
        priority_rank(&a.priority)
            .cmp(&priority_rank(&b.priority))
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.recommendation_id.cmp(&b.recommendation_id))
    });
    open_recommendations.truncate(OPEN_RECOMMENDATION_LIMIT);

    FieldOverview {
        field,
        latest_scene,
        findings_by_severity,
        recent_findings,
        open_recommendations,
        open_recommendation_count,
        recent_alert_count: count_within_days(alert_fired_ats, now, RECENT_ALERT_WINDOW_DAYS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn field() -> FieldInput {
        FieldInput {
            field_id: "field-1".to_string(),
            farm_id: Some("farm-1".to_string()),
            name: "Field A".to_string(),
            crop: Some("corn".to_string()),
            season: Some("2026".to_string()),
        }
    }

    fn finding(id: &str, severity: Option<&str>, created_at: &str) -> FindingInput {
        FindingInput {
            finding_id: id.to_string(),
            kind: "stress_zone".to_string(),
            severity: severity.map(str::to_string),
            created_at: created_at.to_string(),
        }
    }

    fn recommendation(
        id: &str,
        priority: &str,
        status: &str,
        created_at: &str,
    ) -> RecommendationInput {
        RecommendationInput {
            recommendation_id: id.to_string(),
            title: id.to_string(),
            category: None,
            priority: priority.to_string(),
            status: status.to_string(),
            created_at: created_at.to_string(),
        }
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 6, 12, 0, 0).unwrap()
    }

    #[test]
    fn severity_rank_orders_critical_before_low_and_unknown_last() {
        assert!(severity_rank("critical") < severity_rank("high"));
        assert!(severity_rank("high") < severity_rank("medium"));
        assert!(severity_rank("medium") < severity_rank("low"));
        assert!(severity_rank("low") < severity_rank("weird"));
        assert_eq!(severity_rank(" HIGH "), severity_rank("high"));
    }

    #[test]
    fn priority_rank_orders_critical_high_medium_low_then_unknown() {
        assert!(priority_rank("critical") < priority_rank("high"));
        assert!(priority_rank("high") < priority_rank("medium"));
        assert!(priority_rank("medium") < priority_rank("low"));
        assert!(priority_rank("low") < priority_rank("someday"));
    }

    #[test]
    fn count_within_days_uses_trailing_window() {
        let stamps = vec![
            "2026-07-05T00:00:00Z".to_string(), // 1.5 days ago: in
            "2026-06-30T00:00:00Z".to_string(), // 6.5 days ago: in
            "2026-06-01T00:00:00Z".to_string(), // out
            "not-a-timestamp".to_string(),      // ignored
        ];
        assert_eq!(count_within_days(&stamps, now(), 7), 2);
    }

    #[test]
    fn latest_finding_severity_picks_most_recent_with_severity() {
        let findings = vec![
            finding("f-1", Some("critical"), "2026-07-01T00:00:00Z"),
            finding("f-2", Some("low"), "2026-07-03T00:00:00Z"),
            finding("f-3", None, "2026-07-04T00:00:00Z"), // newest but unrated
        ];
        assert_eq!(latest_finding_severity(&findings).as_deref(), Some("low"));
        assert_eq!(latest_finding_severity(&[]), None);
    }

    #[test]
    fn build_field_card_counts_open_recommendations_and_recent_alerts() {
        let findings = vec![finding("f-1", Some("high"), "2026-07-03T00:00:00Z")];
        let recs = vec![
            recommendation("r-1", "high", "open", "2026-07-01T00:00:00Z"),
            recommendation("r-2", "low", "open", "2026-07-02T00:00:00Z"),
            recommendation("r-3", "critical", "dismissed", "2026-07-02T00:00:00Z"),
        ];
        let alerts = vec![
            "2026-07-05T00:00:00Z".to_string(),
            "2026-05-01T00:00:00Z".to_string(),
        ];

        let card = build_field_card(
            field(),
            &findings,
            &recs,
            Some("2026-07-01T10:00:00Z".to_string()),
            &alerts,
            now(),
        );

        assert_eq!(card.field_id, "field-1");
        assert_eq!(card.latest_finding_severity.as_deref(), Some("high"));
        assert_eq!(card.open_recommendations, 2);
        assert_eq!(
            card.latest_scene_at.as_deref(),
            Some("2026-07-01T10:00:00Z")
        );
        assert_eq!(card.recent_alerts_7d, 1);
    }

    #[test]
    fn build_field_overview_buckets_findings_and_caps_recent_list() {
        let mut findings = vec![
            finding("f-none", None, "2026-01-01T00:00:00Z"),
            finding("f-high", Some("high"), "2026-06-30T00:00:00Z"),
        ];
        for index in 0..25 {
            findings.push(finding(
                &format!("f-{index:02}"),
                Some("low"),
                &format!("2026-07-01T00:00:{index:02}Z"),
            ));
        }

        let overview = build_field_overview(field(), None, findings, Vec::new(), &[], now());

        assert_eq!(overview.findings_by_severity.get("high"), Some(&1));
        assert_eq!(overview.findings_by_severity.get("low"), Some(&25));
        assert_eq!(
            overview.findings_by_severity.get(UNSPECIFIED_SEVERITY),
            Some(&1)
        );
        assert_eq!(overview.recent_findings.len(), RECENT_FINDING_LIMIT);
        // Newest first: the last generated low finding leads.
        assert_eq!(overview.recent_findings[0].finding_id, "f-24");
        assert_eq!(overview.open_recommendation_count, 0);
        assert!(overview.latest_scene.is_none());
    }

    #[test]
    fn build_field_overview_ranks_open_recommendations_and_caps_at_five() {
        let recs = vec![
            recommendation("r-low-1", "low", "open", "2026-07-01T00:00:00Z"),
            recommendation("r-low-2", "low", "open", "2026-07-02T00:00:00Z"),
            recommendation("r-low-3", "low", "open", "2026-07-03T00:00:00Z"),
            recommendation("r-med", "medium", "open", "2026-07-01T00:00:00Z"),
            recommendation("r-high", "high", "open", "2026-07-01T00:00:00Z"),
            recommendation("r-crit", "critical", "open", "2026-07-04T00:00:00Z"),
            recommendation("r-done", "critical", "completed", "2026-07-01T00:00:00Z"),
        ];

        let overview = build_field_overview(field(), None, Vec::new(), recs, &[], now());

        assert_eq!(overview.open_recommendation_count, 6);
        let ids: Vec<&str> = overview
            .open_recommendations
            .iter()
            .map(|rec| rec.recommendation_id.as_str())
            .collect();
        assert_eq!(ids, vec!["r-crit", "r-high", "r-med", "r-low-1", "r-low-2"]);
    }
}
