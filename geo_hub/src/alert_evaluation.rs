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
    classify_alert_severity, evaluate_alert_rules, AlertCandidateRecord, AlertRule,
    AlertSeverityClassification, AlertSeverityEvidence, AlertSeverityHint, FiredAlertRecord,
};
use provenance::{ActorIdentity, ArtifactKind, LineageRecord, ProvenanceParameters};
use serde::Serialize;
use serde_json::json;
use thiserror::Error;

const SOURCE_DOMAIN: &str = "geo_hub.applications";
const EVALUATION_METHOD: &str = "alert_evaluation_v1";
const SEVERITY_METHOD_VERSION: &str = "severity_v1";

#[derive(Debug, Error)]
pub enum AlertEvaluationError {
    #[error(transparent)]
    Application(#[from] ApplicationError),
    #[error(transparent)]
    Provenance(#[from] ProvenanceStoreError),
    #[error("severity classification failed: {0}")]
    Classification(#[from] alerting::AlertingError),
    #[error(transparent)]
    Proposal(#[from] crate::proposal_queue::ProposalError),
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
    /// Evidence-based severity from the source finding's metrics (Track C phase
    /// C3). `None` when the finding kind carries no classifiable metric.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classified_severity: Option<AlertSeverityHint>,
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
        AlertRule {
            rule_id: "drought-stress-warning".to_string(),
            event_type: "drought_stress_zone".to_string(),
            subject_ref: None,
            severity: AlertSeverityHint::Warning,
            channels: Vec::new(),
        },
        AlertRule {
            rule_id: "water-balance-deficit-critical".to_string(),
            event_type: "water_balance_deficit_zone".to_string(),
            subject_ref: None,
            severity: AlertSeverityHint::Critical,
            channels: Vec::new(),
        },
        AlertRule {
            rule_id: "water-balance-watch-warning".to_string(),
            event_type: "water_balance_watch_zone".to_string(),
            subject_ref: None,
            severity: AlertSeverityHint::Warning,
            channels: Vec::new(),
        },
    ]
}

/// Evaluate a field's findings against `rules`, persisting fired alerts with
/// lineage to their source findings. Returns the alerts fired this run.
///
/// `propose_action` is the plan's opt-in flag (Track C phase C3): when true, a
/// fired alert on an actionable finding kind also enqueues a Proposed proposal
/// from that finding. Off by default — alert evaluation alone never creates
/// proposals.
pub async fn evaluate_field_alerts(
    pool: &DbPool,
    field_id: &str,
    rules: &[AlertRule],
    propose_action: bool,
    created_at: &str,
) -> Result<Vec<StoredAlert>, AlertEvaluationError> {
    let findings = applications::list_field_findings(pool, field_id).await?;
    let actor = ActorIdentity::system("geo_hub:alert_evaluation");
    let mut stored = Vec::new();

    let propose = propose_action_event_types();
    for finding in &findings {
        let candidate = finding_to_candidate(finding);
        let outcome = evaluate_alert_rules(&candidate, rules);
        let mut fired_here = false;
        for alert in outcome.fired_alerts {
            fired_here = true;
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

            // Evidence-based severity (Track C phase C3): classify from the
            // source finding's metrics, overriding the static rule severity for
            // downstream decisions. Findings without a classifiable metric keep
            // the rule severity.
            let classified = classify_from_finding(&alert, &candidate, finding, created_at)?;
            if let Some(classification) = &classified {
                persist_classification(pool, classification, created_at).await?;
            }
            stored.push(to_stored_alert(
                alert,
                &finding.finding_id,
                classified.map(|c| c.classified_severity),
            ));
        }

        // propose_action (Track C phase C3): a fired alert on an actionable
        // finding kind enqueues a Proposed action proposal from that finding —
        // nothing more. Idempotent per finding, so re-evaluation never
        // duplicates. Approval/dispatch stay downstream and gated.
        if propose_action && fired_here && propose.contains(&finding.finding.kind) {
            maybe_enqueue_proposal(pool, finding, created_at).await?;
        }
    }

    Ok(stored)
}

/// Event types whose fired alerts also enqueue a Proposed action proposal (the
/// plan's opt-in `propose_action`). These are the actionable finding kinds; any
/// other kind fires an alert without proposing.
pub fn propose_action_event_types() -> std::collections::BTreeSet<String> {
    [
        "index_anomaly_zone",
        "water_deficit_zone",
        "declining_zone",
        "drought_stress_zone",
        "water_balance_deficit_zone",
        "water_balance_watch_zone",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// The proposal `(action_category, priority)` for an actionable finding kind.
fn finding_action(kind: &str) -> Option<(&'static str, &'static str)> {
    match kind {
        "index_anomaly_zone" => Some(("scout", "high")),
        "water_deficit_zone" => Some(("irrigation", "medium")),
        "declining_zone" => Some(("review", "medium")),
        "drought_stress_zone" => Some(("irrigation", "high")),
        "water_balance_deficit_zone" => Some(("irrigation", "high")),
        "water_balance_watch_zone" => Some(("review", "medium")),
        _ => None,
    }
}

/// Enqueue a Proposed proposal from a finding that fired an actionable alert.
/// Delegates to the unified proposal queue (idempotent per source finding).
async fn maybe_enqueue_proposal(
    pool: &DbPool,
    finding: &StoredFinding,
    created_at: &str,
) -> Result<(), AlertEvaluationError> {
    let Some((action_category, priority)) = finding_action(&finding.finding.kind) else {
        return Ok(());
    };
    crate::proposal_queue::create_proposal(
        pool,
        &crate::proposal_queue::ProposalCreateRequest {
            source_kind: crate::proposal_queue::ProposalSourceKind::Finding,
            source_id: finding.finding_id.clone(),
            field_id: finding.field_id.clone(),
            title: format!("{action_category} — {}", finding.finding.kind),
            action_category: action_category.to_string(),
            priority: priority.to_string(),
            rationale: Some(format!(
                "Auto-proposed from an alert on {}",
                finding.finding.kind
            )),
        },
        created_at,
    )
    .await?;
    Ok(())
}

/// Build severity evidence from a finding's metrics and classify the alert. The
/// evidence metric + thresholds are per finding kind; kinds without a numeric
/// severity signal return `None` (the rule severity stands).
fn classify_from_finding(
    alert: &FiredAlertRecord,
    candidate: &AlertCandidateRecord,
    finding: &StoredFinding,
    _created_at: &str,
) -> Result<Option<AlertSeverityClassification>, AlertEvaluationError> {
    let Some(evidence) = severity_evidence(&finding.finding.kind, finding.finding.metrics.as_ref())
    else {
        return Ok(None);
    };
    // The deterministic evidence/rule engine owns the outcome; the candidate's
    // severity hint is advisory only.
    let classification = classify_alert_severity(alert, candidate.severity_hint, evidence)?;
    Ok(Some(classification))
}

/// Per-kind severity evidence. Thresholds are ascending (warning < critical <
/// emergency); the observed value is the finding's actionable magnitude.
fn severity_evidence(
    kind: &str,
    metrics: Option<&serde_json::Value>,
) -> Option<AlertSeverityEvidence> {
    let metrics = metrics?;
    let (metric, observed, warning, critical, emergency) = match kind {
        // Anomaly magnitude: how many std deviations from the zone-set mean.
        "index_anomaly_zone" => (
            "anomaly_zscore",
            metrics.get("z_score")?.as_f64()?.abs(),
            1.5,
            2.5,
            4.0,
        ),
        // Water deficit, in mm below target.
        "water_deficit_zone" => (
            "water_deficit_mm",
            metrics.get("water_deficit_mm")?.as_f64()?,
            5.0,
            15.0,
            30.0,
        ),
        // Crop-health decline: magnitude of the negative NDVI delta.
        "declining_zone" => (
            "ndvi_decline",
            (-metrics.get("ndvi_delta")?.as_f64()?).max(0.0),
            0.05,
            0.10,
            0.20,
        ),
        // Water-balance deficit graded by supply-decline rate: how fast the
        // water area is shrinking (relative_area_change, more negative =
        // worse). Escalates warning -> critical -> emergency.
        "water_balance_deficit_zone" => (
            "supply_decline_rate",
            (-metrics.get("relative_area_change")?.as_f64()?).max(0.0),
            0.10,
            0.30,
            0.50,
        ),
        _ => return None,
    };
    Some(AlertSeverityEvidence {
        metric: metric.to_string(),
        observed_value: observed,
        warning_threshold: warning,
        critical_threshold: critical,
        emergency_threshold: emergency,
        method_version: SEVERITY_METHOD_VERSION.to_string(),
    })
}

async fn persist_classification(
    pool: &DbPool,
    classification: &AlertSeverityClassification,
    classified_at: &str,
) -> Result<(), AlertEvaluationError> {
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO alert_severity_classification
            (alert_id, rule_severity, classified_severity, hard_override_downstream,
             metric, observed_value, threshold_value, method_version, explanation, classified_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&classification.alert_id)
    .bind(classification.rule_severity.as_str())
    .bind(classification.classified_severity.as_str())
    .bind(classification.hard_override_downstream as i64)
    .bind(&classification.metric)
    .bind(classification.observed_value)
    .bind(classification.threshold_value)
    .bind(&classification.method_version)
    .bind(&classification.explanation)
    .bind(classified_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetch the evidence-based severity classification for an alert (Track C C3).
pub async fn get_severity_classification(
    pool: &DbPool,
    alert_id: &str,
) -> Result<Option<AlertSeverityClassification>, AlertEvaluationError> {
    use sqlx::Row;
    let Some(row) = sqlx::query("SELECT * FROM alert_severity_classification WHERE alert_id = ?")
        .bind(alert_id)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(None);
    };
    let parse = |col: &str| -> AlertSeverityHint {
        row.get::<String, _>(col)
            .parse()
            .unwrap_or(AlertSeverityHint::Info)
    };
    Ok(Some(AlertSeverityClassification {
        alert_id: row.get("alert_id"),
        matched_rule_id: String::new(),
        rule_severity: parse("rule_severity"),
        source_severity_hint: parse("rule_severity"),
        classified_severity: parse("classified_severity"),
        hard_override_downstream: row.get::<i64, _>("hard_override_downstream") != 0,
        metric: row.get("metric"),
        observed_value: row.get("observed_value"),
        threshold_value: row.get::<Option<f64>, _>("threshold_value"),
        method_version: row.get("method_version"),
        explanation: row.get("explanation"),
    }))
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

fn to_stored_alert(
    alert: FiredAlertRecord,
    source_finding_id: &str,
    classified_severity: Option<AlertSeverityHint>,
) -> StoredAlert {
    StoredAlert {
        alert_id: alert.alert_id,
        matched_rule_id: alert.matched_rule_id,
        source_finding_id: source_finding_id.to_string(),
        field_id: alert.field_id,
        event_type: alert.event_type,
        subject_ref: alert.subject_ref,
        severity: alert.severity,
        classified_severity,
        channels: alert.channels,
        evidence_refs: alert.evidence_refs,
        explanation: alert.explanation,
        fired_at: alert.fired_at,
    }
}

/// List the alerts fired for a field, most recent first. Each alert's
/// evidence-based `classified_severity` (Track C C3) is joined in when present.
pub async fn list_field_alerts(
    pool: &DbPool,
    field_id: &str,
) -> Result<Vec<StoredAlert>, AlertEvaluationError> {
    use sqlx::Row;
    let rows = sqlx::query(
        r#"
        SELECT f.*, c.classified_severity AS classified_severity
        FROM fired_alerts f
        LEFT JOIN alert_severity_classification c ON c.alert_id = f.alert_id
        WHERE f.field_id = ? ORDER BY f.fired_at DESC, f.alert_id ASC
        "#,
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
        let classified_severity = row
            .get::<Option<String>, _>("classified_severity")
            .and_then(|s| s.parse().ok());
        out.push(StoredAlert {
            alert_id: row.get("alert_id"),
            matched_rule_id: row.get("matched_rule_id"),
            source_finding_id: row.get("source_finding_id"),
            field_id: row.get("field_id"),
            event_type: row.get("event_type"),
            subject_ref: row.get("subject_ref"),
            severity,
            classified_severity,
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
