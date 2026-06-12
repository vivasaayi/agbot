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
    use super::{AlertEvent, AlertEventBackbone, AlertSeverityHint, AlertingError, SourceAdapter};

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
