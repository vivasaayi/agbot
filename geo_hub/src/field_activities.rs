//! Farm activity log domain rules (batch F-B5).
//!
//! Pure builders and validators for the `field_activities` table: what a
//! portal caller may record against an owned field (planting, irrigation,
//! spraying, ...), how drafts are validated and normalized, how updates are
//! applied, and how a season summary is aggregated. Persistence and HTTP
//! concerns stay in `routes/portal.rs`; everything here is deterministic
//! except the generated activity ID.

use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

/// Maximum stored note length, in characters, after trimming.
pub const MAX_NOTE_CHARS: usize = 2000;

/// Reason-coded failure when parsing a stored or caller-supplied enum label.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActivityParseError {
    #[error(
        "invalid activity_type {0:?}; expected one of: planting, irrigation, spraying, \
         fertilizing, scouting, harvest, tillage, other"
    )]
    UnknownActivityType(String),
    #[error("invalid activity source {0:?}; expected one of: manual, recommendation, proposal")]
    UnknownActivitySource(String),
}

/// Reason-coded validation failure for an activity draft or patch.
#[derive(Debug, Error, PartialEq)]
pub enum ActivityValidationError {
    #[error(transparent)]
    Parse(#[from] ActivityParseError),
    #[error("occurred_at must be an RFC 3339 timestamp or a YYYY-MM-DD date, got {0:?}")]
    InvalidOccurredAt(String),
    #[error("quantity must be >= 0, got {0}")]
    NegativeQuantity(f64),
    #[error("quantity requires a unit")]
    QuantityRequiresUnit,
    #[error("cost must be >= 0, got {0}")]
    NegativeCost(f64),
    #[error("geometry_json must be GeoJSON with a \"type\" member: {0}")]
    InvalidGeometry(String),
    #[error("note exceeds {MAX_NOTE_CHARS} characters ({0})")]
    NoteTooLong(usize),
}

/// What kind of field operation an activity records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    Planting,
    Irrigation,
    Spraying,
    Fertilizing,
    Scouting,
    Harvest,
    Tillage,
    Other,
}

impl ActivityType {
    pub fn as_str(self) -> &'static str {
        match self {
            ActivityType::Planting => "planting",
            ActivityType::Irrigation => "irrigation",
            ActivityType::Spraying => "spraying",
            ActivityType::Fertilizing => "fertilizing",
            ActivityType::Scouting => "scouting",
            ActivityType::Harvest => "harvest",
            ActivityType::Tillage => "tillage",
            ActivityType::Other => "other",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ActivityParseError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "planting" => Ok(ActivityType::Planting),
            "irrigation" => Ok(ActivityType::Irrigation),
            "spraying" => Ok(ActivityType::Spraying),
            "fertilizing" => Ok(ActivityType::Fertilizing),
            "scouting" => Ok(ActivityType::Scouting),
            "harvest" => Ok(ActivityType::Harvest),
            "tillage" => Ok(ActivityType::Tillage),
            "other" => Ok(ActivityType::Other),
            _ => Err(ActivityParseError::UnknownActivityType(value.to_string())),
        }
    }
}

/// Where an activity record came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivitySource {
    Manual,
    Recommendation,
    Proposal,
}

impl ActivitySource {
    pub fn as_str(self) -> &'static str {
        match self {
            ActivitySource::Manual => "manual",
            ActivitySource::Recommendation => "recommendation",
            ActivitySource::Proposal => "proposal",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ActivityParseError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "manual" => Ok(ActivitySource::Manual),
            "recommendation" => Ok(ActivitySource::Recommendation),
            "proposal" => Ok(ActivitySource::Proposal),
            _ => Err(ActivityParseError::UnknownActivitySource(value.to_string())),
        }
    }
}

/// Caller-supplied shape of a new activity (request body).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ActivityDraft {
    pub activity_type: String,
    pub occurred_at: String,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub quantity: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub cost: Option<f64>,
    #[serde(default)]
    pub geometry_json: Option<String>,
}

/// Caller-supplied partial update: only provided fields change. Clearing a
/// stored optional value is intentionally unsupported in v1.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ActivityPatch {
    #[serde(default)]
    pub activity_type: Option<String>,
    #[serde(default)]
    pub occurred_at: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub quantity: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub cost: Option<f64>,
    #[serde(default)]
    pub geometry_json: Option<String>,
}

/// A validated, normalized activity as stored in `field_activities`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActivityRecord {
    pub activity_id: String,
    pub field_id: String,
    pub org_id: String,
    pub activity_type: ActivityType,
    /// Normalized RFC 3339 UTC timestamp.
    pub occurred_at: String,
    pub note: Option<String>,
    pub quantity: Option<f64>,
    pub unit: Option<String>,
    pub cost: Option<f64>,
    pub geometry_json: Option<String>,
    pub created_by: String,
    pub source: ActivitySource,
    /// Origin reference when `source` is not manual (e.g. a recommendation ID).
    pub linked_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Normalize a caller-supplied occurrence time: full RFC 3339 is converted to
/// UTC; a bare `YYYY-MM-DD` date becomes midnight UTC on that day.
pub fn normalize_occurred_at(value: &str) -> Result<String, ActivityValidationError> {
    let trimmed = value.trim();
    if let Ok(parsed) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(parsed
            .with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Secs, true));
    }
    if NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").is_ok() {
        return Ok(format!("{trimmed}T00:00:00Z"));
    }
    Err(ActivityValidationError::InvalidOccurredAt(
        value.to_string(),
    ))
}

/// Normalize a summary/list range bound. A bare date used as an *end* bound
/// means "through the end of that day", so it maps to 23:59:59Z instead of
/// midnight; start bounds and full timestamps behave like
/// [`normalize_occurred_at`].
pub fn normalize_range_bound(value: &str, end: bool) -> Result<String, ActivityValidationError> {
    let trimmed = value.trim();
    if end && NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").is_ok() {
        return Ok(format!("{trimmed}T23:59:59Z"));
    }
    normalize_occurred_at(trimmed)
}

fn normalize_note(note: Option<String>) -> Result<Option<String>, ActivityValidationError> {
    match note {
        None => Ok(None),
        Some(raw) => {
            let trimmed = raw.trim();
            let chars = trimmed.chars().count();
            if chars > MAX_NOTE_CHARS {
                return Err(ActivityValidationError::NoteTooLong(chars));
            }
            Ok((!trimmed.is_empty()).then(|| trimmed.to_string()))
        }
    }
}

fn normalize_unit(unit: Option<String>) -> Option<String> {
    unit.and_then(|raw| {
        let trimmed = raw.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

/// Quantity must be a finite non-negative number and must carry a unit.
fn validate_quantity_unit(
    quantity: Option<f64>,
    unit: Option<&str>,
) -> Result<(), ActivityValidationError> {
    if let Some(value) = quantity {
        if !value.is_finite() || value < 0.0 {
            return Err(ActivityValidationError::NegativeQuantity(value));
        }
        if unit.is_none() {
            return Err(ActivityValidationError::QuantityRequiresUnit);
        }
    }
    Ok(())
}

fn validate_cost(cost: Option<f64>) -> Result<(), ActivityValidationError> {
    if let Some(value) = cost {
        if !value.is_finite() || value < 0.0 {
            return Err(ActivityValidationError::NegativeCost(value));
        }
    }
    Ok(())
}

/// Geometry, when present, must be valid JSON carrying a GeoJSON `"type"`
/// member. Returned in compact serialized form.
fn validate_geometry_json(raw: &str) -> Result<String, ActivityValidationError> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|err| ActivityValidationError::InvalidGeometry(err.to_string()))?;
    match value.get("type").and_then(|member| member.as_str()) {
        Some(_) => Ok(value.to_string()),
        None => Err(ActivityValidationError::InvalidGeometry(
            "missing \"type\" member".to_string(),
        )),
    }
}

/// Validate and normalize a draft into a storable record with a fresh
/// activity ID. `now` is the caller-supplied record timestamp (RFC 3339).
pub fn build_activity_record(
    draft: ActivityDraft,
    field_id: &str,
    org_id: &str,
    created_by: &str,
    now: &str,
) -> Result<ActivityRecord, ActivityValidationError> {
    let activity_type = ActivityType::parse(&draft.activity_type)?;
    let occurred_at = normalize_occurred_at(&draft.occurred_at)?;
    let note = normalize_note(draft.note)?;
    let unit = normalize_unit(draft.unit);
    validate_quantity_unit(draft.quantity, unit.as_deref())?;
    validate_cost(draft.cost)?;
    let geometry_json = draft
        .geometry_json
        .as_deref()
        .map(validate_geometry_json)
        .transpose()?;

    Ok(ActivityRecord {
        activity_id: format!("activity-{}", Uuid::new_v4()),
        field_id: field_id.to_string(),
        org_id: org_id.to_string(),
        activity_type,
        occurred_at,
        note,
        quantity: draft.quantity,
        unit,
        cost: draft.cost,
        geometry_json,
        created_by: created_by.to_string(),
        source: ActivitySource::Manual,
        linked_ref: None,
        created_at: now.to_string(),
        updated_at: now.to_string(),
    })
}

/// Apply a partial update: only provided fields change, with the same
/// validations as creation. The quantity/unit pair is re-validated against
/// the *effective* (post-patch) values so a patch cannot leave a quantity
/// without a unit. `updated_at` moves to `now`; identity, provenance
/// (`source`/`linked_ref`), and `created_at` never change.
pub fn apply_activity_update(
    record: &ActivityRecord,
    patch: ActivityPatch,
    now: &str,
) -> Result<ActivityRecord, ActivityValidationError> {
    let mut updated = record.clone();

    if let Some(activity_type) = patch.activity_type.as_deref() {
        updated.activity_type = ActivityType::parse(activity_type)?;
    }
    if let Some(occurred_at) = patch.occurred_at.as_deref() {
        updated.occurred_at = normalize_occurred_at(occurred_at)?;
    }
    if patch.note.is_some() {
        updated.note = normalize_note(patch.note)?;
    }
    if let Some(unit) = normalize_unit(patch.unit) {
        updated.unit = Some(unit);
    }
    if let Some(quantity) = patch.quantity {
        updated.quantity = Some(quantity);
    }
    validate_quantity_unit(updated.quantity, updated.unit.as_deref())?;
    if let Some(cost) = patch.cost {
        validate_cost(Some(cost))?;
        updated.cost = Some(cost);
    }
    if let Some(geometry_json) = patch.geometry_json.as_deref() {
        updated.geometry_json = Some(validate_geometry_json(geometry_json)?);
    }

    updated.updated_at = now.to_string();
    Ok(updated)
}

/// Build an activity from a completed recommendation: same validation as a
/// manual draft, but provenance points back at the recommendation.
pub fn activity_from_recommendation(
    recommendation_id: &str,
    draft: ActivityDraft,
    field_id: &str,
    org_id: &str,
    created_by: &str,
    now: &str,
) -> Result<ActivityRecord, ActivityValidationError> {
    let mut record = build_activity_record(draft, field_id, org_id, created_by, now)?;
    record.source = ActivitySource::Recommendation;
    record.linked_ref = Some(recommendation_id.to_string());
    Ok(record)
}

/// Per-activity-type aggregate within a summary window.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ActivityTypeSummary {
    pub count: u64,
    /// Quantities only sum within the same unit: unit label -> total.
    pub total_quantity: BTreeMap<String, f64>,
    pub total_cost: f64,
}

/// Season summary over a set of activities.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ActivitySummary {
    pub total_count: u64,
    pub total_cost: f64,
    /// Keyed by [`ActivityType::as_str`].
    pub by_type: BTreeMap<String, ActivityTypeSummary>,
}

/// Aggregate activities whose `occurred_at` falls inside the inclusive
/// `[from, to]` window (either bound optional). Bounds must already be
/// normalized RFC 3339 UTC strings (see [`normalize_range_bound`]) so plain
/// string comparison is chronological.
pub fn summarize_activities(
    records: &[ActivityRecord],
    from: Option<&str>,
    to: Option<&str>,
) -> ActivitySummary {
    let mut summary = ActivitySummary::default();
    for record in records {
        if from.is_some_and(|bound| record.occurred_at.as_str() < bound)
            || to.is_some_and(|bound| record.occurred_at.as_str() > bound)
        {
            continue;
        }
        summary.total_count += 1;
        let entry = summary
            .by_type
            .entry(record.activity_type.as_str().to_string())
            .or_default();
        entry.count += 1;
        if let (Some(quantity), Some(unit)) = (record.quantity, record.unit.as_deref()) {
            *entry.total_quantity.entry(unit.to_string()).or_insert(0.0) += quantity;
        }
        if let Some(cost) = record.cost {
            entry.total_cost += cost;
            summary.total_cost += cost;
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-07-06T12:00:00Z";

    fn draft(activity_type: &str, occurred_at: &str) -> ActivityDraft {
        ActivityDraft {
            activity_type: activity_type.to_string(),
            occurred_at: occurred_at.to_string(),
            ..ActivityDraft::default()
        }
    }

    fn build(draft: ActivityDraft) -> Result<ActivityRecord, ActivityValidationError> {
        build_activity_record(draft, "field-1", "org-1", "acct-1", NOW)
    }

    #[test]
    fn activity_type_round_trips_and_rejects_unknown() {
        for label in [
            "planting",
            "irrigation",
            "spraying",
            "fertilizing",
            "scouting",
            "harvest",
            "tillage",
            "other",
        ] {
            assert_eq!(ActivityType::parse(label).unwrap().as_str(), label);
        }
        assert_eq!(
            ActivityType::parse("mowing"),
            Err(ActivityParseError::UnknownActivityType(
                "mowing".to_string()
            ))
        );
        assert_eq!(
            ActivitySource::parse("import"),
            Err(ActivityParseError::UnknownActivitySource(
                "import".to_string()
            ))
        );
    }

    #[test]
    fn build_normalizes_date_only_and_offset_timestamps() {
        let record = build(draft("planting", "2026-05-01")).unwrap();
        assert_eq!(record.occurred_at, "2026-05-01T00:00:00Z");
        assert_eq!(record.source, ActivitySource::Manual);
        assert_eq!(record.linked_ref, None);
        assert!(record.activity_id.starts_with("activity-"));
        assert_eq!(record.created_at, NOW);
        assert_eq!(record.updated_at, NOW);

        let record = build(draft("harvest", "2026-05-01T10:00:00+02:00")).unwrap();
        assert_eq!(record.occurred_at, "2026-05-01T08:00:00Z");

        assert_eq!(
            build(draft("harvest", "yesterday")),
            Err(ActivityValidationError::InvalidOccurredAt(
                "yesterday".to_string()
            ))
        );
    }

    #[test]
    fn build_enforces_quantity_unit_cost_rules() {
        let mut with_quantity = draft("irrigation", "2026-05-02");
        with_quantity.quantity = Some(-3.0);
        with_quantity.unit = Some("mm".to_string());
        assert_eq!(
            build(with_quantity),
            Err(ActivityValidationError::NegativeQuantity(-3.0))
        );

        let mut missing_unit = draft("irrigation", "2026-05-02");
        missing_unit.quantity = Some(12.0);
        assert_eq!(
            build(missing_unit),
            Err(ActivityValidationError::QuantityRequiresUnit)
        );

        let mut blank_unit = draft("irrigation", "2026-05-02");
        blank_unit.quantity = Some(12.0);
        blank_unit.unit = Some("   ".to_string());
        assert_eq!(
            build(blank_unit),
            Err(ActivityValidationError::QuantityRequiresUnit)
        );

        let mut bad_cost = draft("spraying", "2026-05-02");
        bad_cost.cost = Some(-1.5);
        assert_eq!(
            build(bad_cost),
            Err(ActivityValidationError::NegativeCost(-1.5))
        );
    }

    #[test]
    fn build_validates_geometry_and_note() {
        let mut bad_geometry = draft("scouting", "2026-05-03");
        bad_geometry.geometry_json = Some("{\"coordinates\": []}".to_string());
        assert!(matches!(
            build(bad_geometry),
            Err(ActivityValidationError::InvalidGeometry(_))
        ));

        let mut good_geometry = draft("scouting", "2026-05-03");
        good_geometry.geometry_json =
            Some("{\"type\": \"Point\", \"coordinates\": [1.0, 2.0]}".to_string());
        let record = build(good_geometry).unwrap();
        assert!(record.geometry_json.unwrap().contains("\"type\""));

        let mut long_note = draft("scouting", "2026-05-03");
        long_note.note = Some("x".repeat(MAX_NOTE_CHARS + 1));
        assert_eq!(
            build(long_note),
            Err(ActivityValidationError::NoteTooLong(MAX_NOTE_CHARS + 1))
        );

        let mut padded_note = draft("scouting", "2026-05-03");
        padded_note.note = Some("  looks dry on the west edge  ".to_string());
        assert_eq!(
            build(padded_note).unwrap().note.as_deref(),
            Some("looks dry on the west edge")
        );

        let mut blank_note = draft("scouting", "2026-05-03");
        blank_note.note = Some("   ".to_string());
        assert_eq!(build(blank_note).unwrap().note, None);
    }

    #[test]
    fn update_changes_only_provided_fields_and_revalidates() {
        let mut seed = draft("irrigation", "2026-05-02");
        seed.quantity = Some(10.0);
        seed.unit = Some("mm".to_string());
        seed.note = Some("first pass".to_string());
        let record = build(seed).unwrap();

        let updated = apply_activity_update(
            &record,
            ActivityPatch {
                quantity: Some(14.0),
                note: Some("second pass".to_string()),
                ..ActivityPatch::default()
            },
            "2026-07-07T00:00:00Z",
        )
        .unwrap();
        assert_eq!(updated.quantity, Some(14.0));
        assert_eq!(updated.unit.as_deref(), Some("mm"));
        assert_eq!(updated.note.as_deref(), Some("second pass"));
        assert_eq!(updated.occurred_at, record.occurred_at);
        assert_eq!(updated.created_at, record.created_at);
        assert_eq!(updated.updated_at, "2026-07-07T00:00:00Z");

        // Patching quantity onto a record without a unit must fail.
        let unitless = build(draft("scouting", "2026-05-03")).unwrap();
        assert_eq!(
            apply_activity_update(
                &unitless,
                ActivityPatch {
                    quantity: Some(2.0),
                    ..ActivityPatch::default()
                },
                NOW,
            ),
            Err(ActivityValidationError::QuantityRequiresUnit)
        );

        assert_eq!(
            apply_activity_update(
                &record,
                ActivityPatch {
                    activity_type: Some("mowing".to_string()),
                    ..ActivityPatch::default()
                },
                NOW,
            ),
            Err(ActivityValidationError::Parse(
                ActivityParseError::UnknownActivityType("mowing".to_string())
            ))
        );
    }

    #[test]
    fn recommendation_activity_carries_source_and_linked_ref() {
        let record = activity_from_recommendation(
            "rec-9",
            draft("spraying", "2026-06-01"),
            "field-1",
            "org-1",
            "acct-1",
            NOW,
        )
        .unwrap();
        assert_eq!(record.source, ActivitySource::Recommendation);
        assert_eq!(record.linked_ref.as_deref(), Some("rec-9"));
    }

    #[test]
    fn summary_filters_window_and_aggregates_by_type_and_unit() {
        let mut records = Vec::new();
        let mut push = |activity_type: &str,
                        occurred_at: &str,
                        quantity: Option<(f64, &str)>,
                        cost: Option<f64>| {
            let mut d = draft(activity_type, occurred_at);
            if let Some((value, unit)) = quantity {
                d.quantity = Some(value);
                d.unit = Some(unit.to_string());
            }
            d.cost = cost;
            records.push(build(d).unwrap());
        };
        push("irrigation", "2026-05-01", Some((10.0, "mm")), Some(40.0));
        push("irrigation", "2026-05-10", Some((15.0, "mm")), Some(60.0));
        push("fertilizing", "2026-05-05", Some((50.0, "kg")), Some(200.0));
        push("fertilizing", "2026-05-06", Some((20.0, "l")), None);
        push("scouting", "2026-05-07", None, None);
        // Outside the window on both sides.
        push("irrigation", "2026-04-01", Some((99.0, "mm")), Some(999.0));
        push("harvest", "2026-09-01", Some((5.0, "t")), Some(500.0));

        let summary = summarize_activities(
            &records,
            Some("2026-05-01T00:00:00Z"),
            Some("2026-05-31T23:59:59Z"),
        );

        assert_eq!(summary.total_count, 5);
        assert_eq!(summary.total_cost, 300.0);
        let irrigation = &summary.by_type["irrigation"];
        assert_eq!(irrigation.count, 2);
        assert_eq!(irrigation.total_quantity["mm"], 25.0);
        assert_eq!(irrigation.total_cost, 100.0);
        let fertilizing = &summary.by_type["fertilizing"];
        assert_eq!(fertilizing.count, 2);
        assert_eq!(fertilizing.total_quantity["kg"], 50.0);
        assert_eq!(fertilizing.total_quantity["l"], 20.0);
        assert_eq!(fertilizing.total_cost, 200.0);
        let scouting = &summary.by_type["scouting"];
        assert_eq!(scouting.count, 1);
        assert!(scouting.total_quantity.is_empty());
        assert_eq!(scouting.total_cost, 0.0);
        assert!(!summary.by_type.contains_key("harvest"));
    }

    #[test]
    fn range_bounds_normalize_dates_inclusively() {
        assert_eq!(
            normalize_range_bound("2026-05-01", false).unwrap(),
            "2026-05-01T00:00:00Z"
        );
        assert_eq!(
            normalize_range_bound("2026-05-31", true).unwrap(),
            "2026-05-31T23:59:59Z"
        );
        assert_eq!(
            normalize_range_bound("2026-05-31T12:00:00Z", true).unwrap(),
            "2026-05-31T12:00:00Z"
        );
        assert!(normalize_range_bound("soon", true).is_err());
    }
}
