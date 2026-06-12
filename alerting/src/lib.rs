use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverityHint {
    Info,
    Warning,
    Critical,
    Emergency,
}

impl AlertSeverityHint {
    pub fn as_str(self) -> &'static str {
        match self {
            AlertSeverityHint::Info => "info",
            AlertSeverityHint::Warning => "warning",
            AlertSeverityHint::Critical => "critical",
            AlertSeverityHint::Emergency => "emergency",
        }
    }
}

impl FromStr for AlertSeverityHint {
    type Err = AlertingError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "info" => Ok(AlertSeverityHint::Info),
            "warning" => Ok(AlertSeverityHint::Warning),
            "critical" => Ok(AlertSeverityHint::Critical),
            "emergency" => Ok(AlertSeverityHint::Emergency),
            other => Err(AlertingError::InvalidSeverity(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertEvent {
    pub source_domain: String,
    pub event_type: String,
    pub subject_ref: String,
    pub severity_hint: AlertSeverityHint,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub occurred_at: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertCandidateRecord {
    pub alert_candidate_id: String,
    pub source_domain: String,
    pub event_type: String,
    pub subject_ref: String,
    pub severity_hint: AlertSeverityHint,
    pub evidence_refs: Vec<String>,
    pub occurred_at: String,
    pub idempotency_key: String,
    pub accepted_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertRule {
    pub rule_id: String,
    pub event_type: String,
    pub subject_ref: Option<String>,
    pub severity: AlertSeverityHint,
    #[serde(default)]
    pub channels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FiredAlertRecord {
    pub alert_id: String,
    pub matched_rule_id: String,
    pub source_event_ref: String,
    pub source_domain: String,
    pub event_type: String,
    pub subject_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_id: Option<String>,
    pub evidence_refs: Vec<String>,
    pub severity: AlertSeverityHint,
    pub channels: Vec<String>,
    pub fired_at: String,
    pub explanation: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlertHistoryQuery {
    pub source_domain: Option<String>,
    pub field_id: Option<String>,
    pub severity: Option<AlertSeverityHint>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertHistoryPage {
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub alerts: Vec<FiredAlertRecord>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleEvaluationOutcome {
    pub fired_alerts: Vec<FiredAlertRecord>,
    pub non_match_count: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AlertEventBackbone {
    candidates: Vec<AlertCandidateRecord>,
    rejected_event_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AlertingError {
    #[error("source_domain cannot be empty")]
    EmptySourceDomain,
    #[error("event_type cannot be empty")]
    EmptyEventType,
    #[error("subject_ref cannot be empty")]
    EmptySubjectRef,
    #[error("evidence_refs cannot contain empty values")]
    EmptyEvidenceRef,
    #[error("occurred_at cannot be empty")]
    EmptyOccurredAt,
    #[error("idempotency_key cannot be empty")]
    EmptyIdempotencyKey,
    #[error("alert_id cannot be empty")]
    EmptyAlertId,
    #[error("matched_rule_id cannot be empty")]
    EmptyMatchedRuleId,
    #[error("source_event_ref cannot be empty")]
    EmptySourceEventRef,
    #[error("channels cannot contain empty values")]
    EmptyChannel,
    #[error("fired_at cannot be empty")]
    EmptyFiredAt,
    #[error("explanation cannot be empty")]
    EmptyExplanation,
    #[error("invalid alert severity {0}")]
    InvalidSeverity(String),
}

pub trait SourceAdapter {
    fn emit(&mut self, event: AlertEvent) -> Result<AlertCandidateRecord, AlertingError>;
}

impl SourceAdapter for AlertEventBackbone {
    fn emit(&mut self, event: AlertEvent) -> Result<AlertCandidateRecord, AlertingError> {
        match normalize_event(event) {
            Ok(event) => {
                let candidate = AlertCandidateRecord {
                    alert_candidate_id: format!(
                        "alert-candidate-{number:06}",
                        number = self.candidates.len() + 1
                    ),
                    source_domain: event.source_domain,
                    event_type: event.event_type,
                    subject_ref: event.subject_ref,
                    severity_hint: event.severity_hint,
                    evidence_refs: event.evidence_refs,
                    occurred_at: event.occurred_at.clone(),
                    idempotency_key: event.idempotency_key,
                    accepted_at: event.occurred_at,
                };
                self.candidates.push(candidate.clone());
                Ok(candidate)
            }
            Err(error) => {
                self.rejected_event_count += 1;
                Err(error)
            }
        }
    }
}

impl AlertEventBackbone {
    pub fn list_candidates(&self) -> Vec<AlertCandidateRecord> {
        self.candidates.clone()
    }

    pub fn rejected_event_count(&self) -> u32 {
        self.rejected_event_count
    }
}

pub fn evaluate_alert_rules(
    candidate: &AlertCandidateRecord,
    rules: &[AlertRule],
) -> RuleEvaluationOutcome {
    let mut fired_alerts = Vec::new();
    let mut non_match_count = 0;

    for rule in rules {
        if rule_matches_candidate(rule, candidate) {
            fired_alerts.push(FiredAlertRecord {
                alert_id: format!("alert:{}:{}", candidate.alert_candidate_id, rule.rule_id),
                matched_rule_id: rule.rule_id.clone(),
                source_event_ref: candidate.alert_candidate_id.clone(),
                source_domain: candidate.source_domain.clone(),
                event_type: candidate.event_type.clone(),
                subject_ref: candidate.subject_ref.clone(),
                field_id: field_id_from_subject_ref(&candidate.subject_ref),
                evidence_refs: candidate.evidence_refs.clone(),
                severity: rule.severity,
                channels: rule.channels.clone(),
                fired_at: candidate.occurred_at.clone(),
                explanation: format!(
                    "rule {} matched event_type {} for subject {}; evidence refs: {}",
                    rule.rule_id,
                    candidate.event_type,
                    candidate.subject_ref,
                    candidate.evidence_refs.join(",")
                ),
            });
        } else {
            non_match_count += 1;
        }
    }

    RuleEvaluationOutcome {
        fired_alerts,
        non_match_count,
    }
}

fn rule_matches_candidate(rule: &AlertRule, candidate: &AlertCandidateRecord) -> bool {
    rule.event_type == candidate.event_type
        && rule
            .subject_ref
            .as_deref()
            .map_or(true, |subject_ref| subject_ref == candidate.subject_ref)
}

pub fn normalize_fired_alert_record(
    record: FiredAlertRecord,
) -> Result<FiredAlertRecord, AlertingError> {
    Ok(FiredAlertRecord {
        alert_id: normalize_required_text(record.alert_id, AlertingError::EmptyAlertId)?,
        matched_rule_id: normalize_required_text(
            record.matched_rule_id,
            AlertingError::EmptyMatchedRuleId,
        )?,
        source_event_ref: normalize_required_text(
            record.source_event_ref,
            AlertingError::EmptySourceEventRef,
        )?,
        source_domain: normalize_required_text(
            record.source_domain,
            AlertingError::EmptySourceDomain,
        )?,
        event_type: normalize_required_text(record.event_type, AlertingError::EmptyEventType)?,
        subject_ref: normalize_required_text(record.subject_ref, AlertingError::EmptySubjectRef)?,
        field_id: record
            .field_id
            .map(|value| normalize_required_text(value, AlertingError::EmptySubjectRef))
            .transpose()?,
        evidence_refs: record
            .evidence_refs
            .into_iter()
            .map(|value| normalize_required_text(value, AlertingError::EmptyEvidenceRef))
            .collect::<Result<Vec<_>, _>>()?,
        severity: record.severity,
        channels: record
            .channels
            .into_iter()
            .map(|value| normalize_required_text(value, AlertingError::EmptyChannel))
            .collect::<Result<Vec<_>, _>>()?,
        fired_at: normalize_required_text(record.fired_at, AlertingError::EmptyFiredAt)?,
        explanation: normalize_required_text(record.explanation, AlertingError::EmptyExplanation)?,
    })
}

pub fn filter_alert_history(
    records: &[FiredAlertRecord],
    query: &AlertHistoryQuery,
) -> AlertHistoryPage {
    let page = query.page.max(1);
    let page_size = query.page_size.clamp(1, 100);
    let mut filtered = records
        .iter()
        .filter(|record| {
            query
                .source_domain
                .as_deref()
                .map_or(true, |source_domain| source_domain == record.source_domain)
                && query.field_id.as_deref().map_or(true, |field_id| {
                    record.field_id.as_deref() == Some(field_id)
                })
                && query
                    .severity
                    .map_or(true, |severity| severity == record.severity)
                && query
                    .start
                    .as_deref()
                    .map_or(true, |start| record.fired_at.as_str() >= start)
                && query
                    .end
                    .as_deref()
                    .map_or(true, |end| record.fired_at.as_str() <= end)
        })
        .cloned()
        .collect::<Vec<_>>();
    filtered.sort_by(|left, right| {
        right
            .fired_at
            .cmp(&left.fired_at)
            .then_with(|| left.alert_id.cmp(&right.alert_id))
    });

    let total = filtered.len();
    let offset = (page - 1) * page_size;
    let alerts = filtered.into_iter().skip(offset).take(page_size).collect();

    AlertHistoryPage {
        page,
        page_size,
        total,
        alerts,
    }
}

fn normalize_event(event: AlertEvent) -> Result<AlertEvent, AlertingError> {
    Ok(AlertEvent {
        source_domain: normalize_required_text(
            event.source_domain,
            AlertingError::EmptySourceDomain,
        )?,
        event_type: normalize_required_text(event.event_type, AlertingError::EmptyEventType)?,
        subject_ref: normalize_required_text(event.subject_ref, AlertingError::EmptySubjectRef)?,
        severity_hint: event.severity_hint,
        evidence_refs: event
            .evidence_refs
            .into_iter()
            .map(|value| normalize_required_text(value, AlertingError::EmptyEvidenceRef))
            .collect::<Result<Vec<_>, _>>()?,
        occurred_at: normalize_required_text(event.occurred_at, AlertingError::EmptyOccurredAt)?,
        idempotency_key: normalize_required_text(
            event.idempotency_key,
            AlertingError::EmptyIdempotencyKey,
        )?,
    })
}

fn normalize_required_text(value: String, error: AlertingError) -> Result<String, AlertingError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn field_id_from_subject_ref(subject_ref: &str) -> Option<String> {
    subject_ref
        .strip_prefix("field:")
        .map(str::trim)
        .filter(|field_id| !field_id.is_empty())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::{
        evaluate_alert_rules, filter_alert_history, AlertEvent, AlertEventBackbone,
        AlertHistoryQuery, AlertRule, AlertSeverityHint, AlertingError, SourceAdapter,
    };

    #[test]
    fn source_adapter_accepts_and_persists_well_formed_event() {
        let mut backbone = AlertEventBackbone::default();
        let candidate = backbone
            .emit(sensor_health_event())
            .expect("well-formed event should be accepted");

        assert_eq!(candidate.alert_candidate_id, "alert-candidate-000001");
        assert_eq!(candidate.source_domain, "27-soil-iot-sensor-network");
        assert_eq!(candidate.subject_ref, "sensor:soil-probe-001");
        assert_eq!(
            candidate.evidence_refs,
            vec!["reading:soil-probe-001:latest"]
        );
        assert_eq!(backbone.list_candidates().len(), 1);
        assert_eq!(backbone.rejected_event_count(), 0);
    }

    #[test]
    fn malformed_event_is_rejected_and_counted_without_partial_store() {
        let mut backbone = AlertEventBackbone::default();
        let mut event = sensor_health_event();
        event.source_domain = " ".to_string();

        let error = backbone
            .emit(event)
            .expect_err("missing source domain should be rejected");

        assert_eq!(error, AlertingError::EmptySourceDomain);
        assert_eq!(backbone.list_candidates().len(), 0);
        assert_eq!(backbone.rejected_event_count(), 1);
    }

    #[test]
    fn rule_engine_fires_matching_alert_with_explanation() {
        let mut backbone = AlertEventBackbone::default();
        let candidate = backbone
            .emit(sensor_health_event())
            .expect("event should be accepted");
        let outcome = evaluate_alert_rules(
            &candidate,
            &[AlertRule {
                rule_id: "rule-sensor-stale-warning".to_string(),
                event_type: "sensor_stale".to_string(),
                subject_ref: None,
                severity: AlertSeverityHint::Critical,
                channels: vec!["in_app".to_string()],
            }],
        );

        assert_eq!(outcome.fired_alerts.len(), 1);
        assert_eq!(
            outcome.fired_alerts[0].matched_rule_id,
            "rule-sensor-stale-warning"
        );
        assert_eq!(
            outcome.fired_alerts[0].source_event_ref,
            candidate.alert_candidate_id
        );
        assert_eq!(
            outcome.fired_alerts[0].source_domain,
            "27-soil-iot-sensor-network"
        );
        assert_eq!(outcome.fired_alerts[0].event_type, "sensor_stale");
        assert_eq!(outcome.fired_alerts[0].subject_ref, "sensor:soil-probe-001");
        assert_eq!(outcome.fired_alerts[0].fired_at, candidate.occurred_at);
        assert_eq!(
            outcome.fired_alerts[0].evidence_refs,
            vec!["reading:soil-probe-001:latest"]
        );
        assert!(outcome.fired_alerts[0]
            .explanation
            .contains("rule-sensor-stale-warning"));
        assert_eq!(outcome.non_match_count, 0);
    }

    #[test]
    fn rule_engine_records_observable_no_match() {
        let mut backbone = AlertEventBackbone::default();
        let candidate = backbone
            .emit(sensor_health_event())
            .expect("event should be accepted");
        let outcome = evaluate_alert_rules(
            &candidate,
            &[AlertRule {
                rule_id: "rule-weather".to_string(),
                event_type: "weather_warning".to_string(),
                subject_ref: None,
                severity: AlertSeverityHint::Warning,
                channels: vec!["in_app".to_string()],
            }],
        );

        assert!(outcome.fired_alerts.is_empty());
        assert_eq!(outcome.non_match_count, 1);
    }

    #[test]
    fn alert_history_filters_by_source_field_severity_and_time_with_pagination() {
        let records = vec![
            fired_alert(
                "alert-1",
                "27-soil-iot-sensor-network",
                Some("field-alpha"),
                AlertSeverityHint::Critical,
                "2026-06-12T10:00:00Z",
            ),
            fired_alert(
                "alert-2",
                "27-soil-iot-sensor-network",
                Some("field-alpha"),
                AlertSeverityHint::Critical,
                "2026-06-12T10:05:00Z",
            ),
            fired_alert(
                "alert-3",
                "25-predictive-maintenance-fleet-health",
                None,
                AlertSeverityHint::Warning,
                "2026-06-12T10:06:00Z",
            ),
        ];

        let page = filter_alert_history(
            &records,
            &AlertHistoryQuery {
                source_domain: Some("27-soil-iot-sensor-network".to_string()),
                field_id: Some("field-alpha".to_string()),
                severity: Some(AlertSeverityHint::Critical),
                start: Some("2026-06-12T09:59:00Z".to_string()),
                end: Some("2026-06-12T10:06:00Z".to_string()),
                page: 1,
                page_size: 1,
            },
        );

        assert_eq!(page.total, 2);
        assert_eq!(page.page, 1);
        assert_eq!(page.page_size, 1);
        assert_eq!(page.alerts.len(), 1);
        assert_eq!(page.alerts[0].alert_id, "alert-2");
    }

    fn sensor_health_event() -> AlertEvent {
        AlertEvent {
            source_domain: "27-soil-iot-sensor-network".to_string(),
            event_type: "sensor_stale".to_string(),
            subject_ref: "sensor:soil-probe-001".to_string(),
            severity_hint: AlertSeverityHint::Warning,
            evidence_refs: vec!["reading:soil-probe-001:latest".to_string()],
            occurred_at: "2026-06-12T10:00:00Z".to_string(),
            idempotency_key: "27:sensor_stale:soil-probe-001:2026-06-12T10".to_string(),
        }
    }

    fn fired_alert(
        alert_id: &str,
        source_domain: &str,
        field_id: Option<&str>,
        severity: AlertSeverityHint,
        fired_at: &str,
    ) -> super::FiredAlertRecord {
        super::FiredAlertRecord {
            alert_id: alert_id.to_string(),
            matched_rule_id: "rule-sensor-stale-critical".to_string(),
            source_event_ref: format!("candidate:{alert_id}"),
            source_domain: source_domain.to_string(),
            event_type: "sensor_stale".to_string(),
            subject_ref: field_id
                .map(|field_id| format!("field:{field_id}"))
                .unwrap_or_else(|| "component:battery-pack-001".to_string()),
            field_id: field_id.map(ToString::to_string),
            evidence_refs: vec![format!("evidence:{alert_id}")],
            severity,
            channels: vec!["in_app".to_string()],
            fired_at: fired_at.to_string(),
            explanation: "deterministic rule matched evidence".to_string(),
        }
    }
}
