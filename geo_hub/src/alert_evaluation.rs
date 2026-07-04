//! Alert evaluation (Track C phase C1).
//!
//! Screens a field's stored application findings into alerts via the shared
//! `alerting` rule engine, persisting each fired alert with lineage back to the
//! finding it derived from. The anomaly application's `index_anomaly_zone` and
//! the water-priority application's `water_deficit_zone` findings are the
//! primary alert inputs; the default rule set also fires on `declining_zone`.
//!
//! Evaluation is idempotent: fired-alert ids and lineage records are keyed by
//! (finding, rule), so re-running over the same findings is a no-op.

use crate::applications::{self, ApplicationError, StoredFinding};
use crate::db::DbPool;
use crate::provenance_store::{self, ProvenanceStoreError};
use alerting::{
    evaluate_alert_rules, AlertCandidateRecord, AlertRule, AlertSeverityHint, FiredAlertRecord,
};
use provenance::{ActorIdentity, ArtifactKind, LineageRecord, ProvenanceParameters};
use serde::Serialize;
use serde_json::json;
use thiserror::Error;

const SOURCE_DOMAIN: &str = "geo_hub.applications";
const EVALUATION_METHOD: &str = "alert_evaluation_v1";

#[derive(Debug, Error)]
pub enum AlertEvaluationError {
    #[error(transparent)]
    Application(#[from] ApplicationError),
    #[error(transparent)]
    Provenance(#[from] ProvenanceStoreError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("failed to serialize {what}: {source}")]
    Serialize {
        what: &'static str,
        source: serde_json::Error,
    },
}

/// A persisted alert with its source finding linkage.
#[derive(Debug, Clone, Serialize)]
pub struct StoredAlert {
    pub alert_id: String,
    pub matched_rule_id: String,
    pub source_finding_id: String,
    pub field_id: Option<String>,
    pub event_type: String,
    pub subject_ref: String,
    pub severity: AlertSeverityHint,
    pub channels: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub explanation: String,
    pub fired_at: String,
}

/// The default rule set: fire on the actionable finding kinds emitted by the
/// Track B applications. Anomalies are critical; water deficit and declining
/// crop health are warnings.
pub fn default_ruleset() -> Vec<AlertRule> {
    vec![
        AlertRule {
            rule_id: "anomaly-critical".to_string(),
            event_type: "index_anomaly_zone".to_string(),
            subject_ref: None,
            severity: AlertSeverityHint::Critical,
            channels: Vec::new(),
        },
        AlertRule {
            rule_id: "water-deficit-warning".to_string(),
            event_type: "water_deficit_zone".to_string(),
            subject_ref: None,
            severity: AlertSeverityHint::Warning,
            channels: Vec::new(),
        },
        AlertRule {
            rule_id: "declining-warning".to_string(),
            event_type: "declining_zone".to_string(),
            subject_ref: None,
            severity: AlertSeverityHint::Warning,
            channels: Vec::new(),
        },
    ]
}

/// Evaluate a field's findings against `rules`, persisting fired alerts with
/// lineage to their source findings. Returns the alerts fired this run.
pub async fn evaluate_field_alerts(
    pool: &DbPool,
    field_id: &str,
    rules: &[AlertRule],
    created_at: &str,
) -> Result<Vec<StoredAlert>, AlertEvaluationError> {
    let findings = applications::list_field_findings(pool, field_id).await?;
    let actor = ActorIdentity::system("geo_hub:alert_evaluation");
    let mut stored = Vec::new();

    for finding in &findings {
        let candidate = finding_to_candidate(finding);
        let outcome = evaluate_alert_rules(&candidate, rules);
        for alert in outcome.fired_alerts {
            persist_alert(pool, &alert, &finding.finding_id).await?;
            // Lineage: the alert derives from the finding that produced it, so a
            // backward trace closes alert -> finding -> L2/L3 -> L0.
            provenance_store::append_lineage(
                pool,
                &LineageRecord {
                    artifact_id: alert.alert_id.clone(),
                    kind: ArtifactKind::Alert,
                    inputs: vec![finding.finding_id.clone()],
                    method: EVALUATION_METHOD.to_string(),
                    parameters: ProvenanceParameters::from_json(json!({
                        "matched_rule_id": alert.matched_rule_id,
                        "severity": alert.severity.as_str(),
                    })),
                    operator: format!("alert_rule:{}", alert.matched_rule_id),
                    actor: actor.clone(),
                    created_at: created_at.to_string(),
                },
            )
            .await?;
            stored.push(to_stored_alert(alert, &finding.finding_id));
        }
    }

    Ok(stored)
}

/// Map a stored finding into an alert candidate. The candidate's evidence ref is
/// the finding id, so the fired alert's lineage input is the finding itself.
fn finding_to_candidate(finding: &StoredFinding) -> AlertCandidateRecord {
    let field_id = finding.field_id.clone().unwrap_or_default();
    AlertCandidateRecord {
        alert_candidate_id: finding.finding_id.clone(),
        source_domain: SOURCE_DOMAIN.to_string(),
        event_type: finding.finding.kind.clone(),
        subject_ref: format!("field:{field_id}"),
        severity_hint: severity_hint_from(finding.finding.severity.as_deref()),
        evidence_refs: vec![finding.finding_id.clone()],
        occurred_at: finding.created_at.clone(),
        idempotency_key: format!("{}:{}", finding.finding.kind, finding.finding_id),
        accepted_at: finding.created_at.clone(),
    }
}

/// Map a finding's severity string to an alert severity hint (used only to
/// annotate the candidate; rule matching is by event type + subject).
fn severity_hint_from(severity: Option<&str>) -> AlertSeverityHint {
    match severity {
        Some("critical") => AlertSeverityHint::Critical,
        Some("high") => AlertSeverityHint::Warning,
        _ => AlertSeverityHint::Info,
    }
}

async fn persist_alert(
    pool: &DbPool,
    alert: &FiredAlertRecord,
    source_finding_id: &str,
) -> Result<(), AlertEvaluationError> {
    let channels_json = serde_json::to_string(&alert.channels).map_err(|source| {
        AlertEvaluationError::Serialize {
            what: "channels",
            source,
        }
    })?;
    let evidence_json = serde_json::to_string(&alert.evidence_refs).map_err(|source| {
        AlertEvaluationError::Serialize {
            what: "evidence_refs",
            source,
        }
    })?;
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO fired_alerts
            (alert_id, matched_rule_id, source_finding_id, field_id, event_type,
             subject_ref, severity, channels_json, evidence_refs_json, explanation, fired_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&alert.alert_id)
    .bind(&alert.matched_rule_id)
    .bind(source_finding_id)
    .bind(&alert.field_id)
    .bind(&alert.event_type)
    .bind(&alert.subject_ref)
    .bind(alert.severity.as_str())
    .bind(channels_json)
    .bind(evidence_json)
    .bind(&alert.explanation)
    .bind(&alert.fired_at)
    .execute(pool)
    .await?;
    Ok(())
}

fn to_stored_alert(alert: FiredAlertRecord, source_finding_id: &str) -> StoredAlert {
    StoredAlert {
        alert_id: alert.alert_id,
        matched_rule_id: alert.matched_rule_id,
        source_finding_id: source_finding_id.to_string(),
        field_id: alert.field_id,
        event_type: alert.event_type,
        subject_ref: alert.subject_ref,
        severity: alert.severity,
        channels: alert.channels,
        evidence_refs: alert.evidence_refs,
        explanation: alert.explanation,
        fired_at: alert.fired_at,
    }
}

/// List the alerts fired for a field, most recent first.
pub async fn list_field_alerts(
    pool: &DbPool,
    field_id: &str,
) -> Result<Vec<StoredAlert>, AlertEvaluationError> {
    use sqlx::Row;
    let rows = sqlx::query(
        "SELECT * FROM fired_alerts WHERE field_id = ? ORDER BY fired_at DESC, alert_id ASC",
    )
    .bind(field_id)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let severity: AlertSeverityHint = row
            .get::<String, _>("severity")
            .parse()
            .unwrap_or(AlertSeverityHint::Info);
        out.push(StoredAlert {
            alert_id: row.get("alert_id"),
            matched_rule_id: row.get("matched_rule_id"),
            source_finding_id: row.get("source_finding_id"),
            field_id: row.get("field_id"),
            event_type: row.get("event_type"),
            subject_ref: row.get("subject_ref"),
            severity,
            channels: row
                .get::<Option<String>, _>("channels_json")
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            evidence_refs: row
                .get::<Option<String>, _>("evidence_refs_json")
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            explanation: row
                .get::<Option<String>, _>("explanation")
                .unwrap_or_default(),
            fired_at: row.get("fired_at"),
        });
    }
    Ok(out)
}

/// Reconstruct the `alerting::FiredAlertRecord` for a persisted alert, so the
/// lifecycle engine (Track C phase C2) can act on it. Returns `None` when the
/// alert id is unknown.
pub async fn get_fired_alert(
    pool: &DbPool,
    alert_id: &str,
) -> Result<Option<FiredAlertRecord>, AlertEvaluationError> {
    use sqlx::Row;
    let Some(row) = sqlx::query("SELECT * FROM fired_alerts WHERE alert_id = ?")
        .bind(alert_id)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(None);
    };
    let severity: AlertSeverityHint = row
        .get::<String, _>("severity")
        .parse()
        .unwrap_or(AlertSeverityHint::Info);
    Ok(Some(FiredAlertRecord {
        alert_id: row.get("alert_id"),
        matched_rule_id: row.get("matched_rule_id"),
        source_event_ref: row.get("source_finding_id"),
        source_domain: SOURCE_DOMAIN.to_string(),
        event_type: row.get("event_type"),
        subject_ref: row.get("subject_ref"),
        field_id: row.get("field_id"),
        evidence_refs: row
            .get::<Option<String>, _>("evidence_refs_json")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        severity,
        channels: row
            .get::<Option<String>, _>("channels_json")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        fired_at: row.get("fired_at"),
        explanation: row
            .get::<Option<String>, _>("explanation")
            .unwrap_or_default(),
    }))
}
