use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverityHint {
    Info,
    Warning,
    Critical,
    Emergency,
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
    pub evidence_refs: Vec<String>,
    pub severity: AlertSeverityHint,
    pub channels: Vec<String>,
    pub explanation: String,
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
                evidence_refs: candidate.evidence_refs.clone(),
                severity: rule.severity,
                channels: rule.channels.clone(),
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

#[cfg(test)]
mod tests {
    use super::{
        evaluate_alert_rules, AlertEvent, AlertEventBackbone, AlertRule, AlertSeverityHint,
        AlertingError, SourceAdapter,
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
}
