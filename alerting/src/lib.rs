use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertSeverityEvidence {
    pub metric: String,
    pub observed_value: f64,
    pub warning_threshold: f64,
    pub critical_threshold: f64,
    pub emergency_threshold: f64,
    pub method_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertSeverityClassification {
    pub alert_id: String,
    pub matched_rule_id: String,
    pub rule_severity: AlertSeverityHint,
    pub source_severity_hint: AlertSeverityHint,
    pub classified_severity: AlertSeverityHint,
    pub hard_override_downstream: bool,
    pub metric: String,
    pub observed_value: f64,
    pub threshold_value: Option<f64>,
    pub method_version: String,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertDedupKey {
    pub source_domain: String,
    pub subject_ref: String,
    pub rule_id: String,
}

impl AlertDedupKey {
    pub fn stable_key(&self) -> String {
        format!(
            "{}|{}|{}",
            self.source_domain, self.subject_ref, self.rule_id
        )
    }
}

impl fmt::Display for AlertDedupKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.stable_key())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertDedupWindow {
    pub window_start: String,
    pub window_end: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertDedupSummary {
    pub dedup_key: AlertDedupKey,
    pub surfaced_alert_id: String,
    pub occurrence_count: usize,
    pub suppressed_alert_ids: Vec<String>,
    pub first_fired_at: String,
    pub last_fired_at: String,
    pub severity: AlertSeverityHint,
    pub bypassed_suppression: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AlertDedupResult {
    pub surfaced_alerts: Vec<FiredAlertRecord>,
    pub summaries: Vec<AlertDedupSummary>,
    pub suppressed_count: usize,
    pub bypassed_alert_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertChannel {
    InApp,
    Email,
    Sms,
    Webhook,
    Push,
}

impl AlertChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            AlertChannel::InApp => "in_app",
            AlertChannel::Email => "email",
            AlertChannel::Sms => "sms",
            AlertChannel::Webhook => "webhook",
            AlertChannel::Push => "push",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertRecipient {
    pub recipient_id: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertRoutingRule {
    pub rule_id: String,
    pub source_domain: Option<String>,
    pub field_id: Option<String>,
    pub severity: Option<AlertSeverityHint>,
    pub role: String,
    pub recipients: Vec<AlertRecipient>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertRoutingDecision {
    pub alert_id: String,
    pub rule_id: Option<String>,
    pub recipient_id: String,
    pub role: String,
    pub source_domain: String,
    pub field_id: Option<String>,
    pub severity: AlertSeverityHint,
    pub channels: Vec<String>,
    pub default_operator: bool,
    pub audit_detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertRoutingOutcome {
    pub alert_id: String,
    pub recipients: Vec<AlertRecipient>,
    pub decisions: Vec<AlertRoutingDecision>,
    pub unrouted: bool,
    pub default_operator_used: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Delivered,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryOutcome {
    pub delivery_id: String,
    pub alert_id: String,
    pub recipient_id: String,
    pub channel: AlertChannel,
    pub status: DeliveryStatus,
    pub attempted_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivered_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    Queued,
    Sending,
    Delivered,
    Failed,
    Retrying,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryRetryPolicy {
    pub max_attempts: usize,
    pub base_backoff_seconds: u64,
    pub max_backoff_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryStateTransition {
    pub attempt_number: usize,
    pub from: DeliveryState,
    pub to: DeliveryState,
    pub backoff_seconds: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackedDelivery {
    pub delivery_id: String,
    pub final_state: DeliveryState,
    pub attempts: Vec<DeliveryOutcome>,
    pub transitions: Vec<DeliveryStateTransition>,
    pub last_error: Option<String>,
    pub max_attempts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InAppFeedItem {
    pub recipient_id: String,
    pub alert_id: String,
    pub delivered_at: String,
    pub summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InAppChannelAdapter {
    feed_items: Vec<InAppFeedItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockChannelAdapter {
    channel: AlertChannel,
    failure: Option<String>,
    failures_remaining: usize,
    always_fail: bool,
    outcomes: Vec<DeliveryOutcome>,
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
    #[error("severity metric cannot be empty")]
    EmptySeverityMetric,
    #[error("severity method_version cannot be empty")]
    EmptySeverityMethodVersion,
    #[error("recipient_id cannot be empty")]
    EmptyRecipientId,
    #[error("recipient role cannot be empty")]
    EmptyRecipientRole,
    #[error("routing rule_id cannot be empty")]
    EmptyRoutingRuleId,
    #[error("routing rule requires at least one recipient")]
    EmptyRoutingRecipientList,
    #[error("routing rule {rule_id} expected recipient role {expected_role}, got {actual_role}")]
    RoutingRecipientRoleMismatch {
        rule_id: String,
        expected_role: String,
        actual_role: String,
    },
    #[error(
        "routing recipient {recipient_id} matched conflicting roles {first_role} and {second_role}"
    )]
    RoutingRecipientRoleConflict {
        recipient_id: String,
        first_role: String,
        second_role: String,
    },
    #[error("severity evidence must be finite with warning <= critical <= emergency thresholds")]
    InvalidSeverityEvidence,
    #[error("dedup window requires non-empty window_start <= window_end")]
    InvalidDedupWindow,
    #[error("retry policy requires max_attempts > 0 and bounded positive backoff")]
    InvalidRetryPolicy,
    #[error("invalid alert severity {0}")]
    InvalidSeverity(String),
}

pub trait SourceAdapter {
    fn emit(&mut self, event: AlertEvent) -> Result<AlertCandidateRecord, AlertingError>;
}

pub trait ChannelAdapter {
    fn channel(&self) -> AlertChannel;

    fn send(&mut self, alert: &FiredAlertRecord, recipient: &AlertRecipient) -> DeliveryOutcome;
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

impl ChannelAdapter for InAppChannelAdapter {
    fn channel(&self) -> AlertChannel {
        AlertChannel::InApp
    }

    fn send(&mut self, alert: &FiredAlertRecord, recipient: &AlertRecipient) -> DeliveryOutcome {
        let outcome = delivered_outcome(self.channel(), alert, recipient);
        self.feed_items.push(InAppFeedItem {
            recipient_id: recipient.recipient_id.clone(),
            alert_id: alert.alert_id.clone(),
            delivered_at: outcome
                .delivered_at
                .clone()
                .unwrap_or_else(|| alert.fired_at.clone()),
            summary: alert.explanation.clone(),
        });
        outcome
    }
}

impl InAppChannelAdapter {
    pub fn feed_for(&self, recipient_id: &str) -> Vec<InAppFeedItem> {
        self.feed_items
            .iter()
            .filter(|item| item.recipient_id == recipient_id)
            .cloned()
            .collect()
    }
}

impl MockChannelAdapter {
    pub fn succeeding(channel: AlertChannel) -> Self {
        Self {
            channel,
            failure: None,
            failures_remaining: 0,
            always_fail: false,
            outcomes: Vec::new(),
        }
    }

    pub fn failing(channel: AlertChannel, error: String) -> Self {
        Self {
            channel,
            failure: Some(error),
            failures_remaining: usize::MAX,
            always_fail: true,
            outcomes: Vec::new(),
        }
    }

    pub fn flaky(channel: AlertChannel, failures_before_success: usize, error: String) -> Self {
        Self {
            channel,
            failure: Some(error),
            failures_remaining: failures_before_success,
            always_fail: false,
            outcomes: Vec::new(),
        }
    }

    pub fn recorded_outcomes(&self) -> Vec<DeliveryOutcome> {
        self.outcomes.clone()
    }
}

impl ChannelAdapter for MockChannelAdapter {
    fn channel(&self) -> AlertChannel {
        self.channel
    }

    fn send(&mut self, alert: &FiredAlertRecord, recipient: &AlertRecipient) -> DeliveryOutcome {
        let should_fail = self.always_fail || self.failures_remaining > 0;
        let outcome = if should_fail {
            if self.failures_remaining > 0 {
                self.failures_remaining -= 1;
            }
            failed_outcome(
                self.channel,
                alert,
                recipient,
                self.failure.as_deref().unwrap_or("channel delivery failed"),
            )
        } else {
            delivered_outcome(self.channel, alert, recipient)
        };
        self.outcomes.push(outcome.clone());
        outcome
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

pub fn classify_alert_severity(
    alert: &FiredAlertRecord,
    source_severity_hint: AlertSeverityHint,
    evidence: AlertSeverityEvidence,
) -> Result<AlertSeverityClassification, AlertingError> {
    let alert = normalize_fired_alert_record(alert.clone())?;
    let evidence = normalize_severity_evidence(evidence)?;
    let (classified_severity, threshold_value, threshold_name) =
        severity_from_evidence_or_rule(alert.severity, &evidence);
    let hard_override_downstream = matches!(
        classified_severity,
        AlertSeverityHint::Critical | AlertSeverityHint::Emergency
    );
    let hint_clause = if source_severity_hint == classified_severity {
        format!("source hint {} agreed", source_severity_hint.as_str())
    } else {
        format!(
            "source hint {} ignored in favor of deterministic rule/evidence",
            source_severity_hint.as_str()
        )
    };

    Ok(AlertSeverityClassification {
        alert_id: alert.alert_id,
        matched_rule_id: alert.matched_rule_id,
        rule_severity: alert.severity,
        source_severity_hint,
        classified_severity,
        hard_override_downstream,
        metric: evidence.metric.clone(),
        observed_value: evidence.observed_value,
        threshold_value,
        method_version: evidence.method_version.clone(),
        explanation: format!(
            "severity classified as {} for metric {} observed={}; rule_severity={}; warning_threshold={}; critical_threshold={}; emergency_threshold={}; {}; method={}",
            classified_severity.as_str(),
            evidence.metric,
            evidence.observed_value,
            alert.severity.as_str(),
            evidence.warning_threshold,
            evidence.critical_threshold,
            evidence.emergency_threshold,
            hint_clause,
            evidence.method_version
        ) + threshold_name.map_or("", |name| name),
    })
}

pub fn compute_alert_dedup_key(alert: &FiredAlertRecord) -> Result<AlertDedupKey, AlertingError> {
    let alert = normalize_fired_alert_record(alert.clone())?;
    Ok(AlertDedupKey {
        source_domain: alert.source_domain,
        subject_ref: alert.subject_ref,
        rule_id: alert.matched_rule_id,
    })
}

pub fn deduplicate_alert_stream(
    alerts: &[FiredAlertRecord],
    window: AlertDedupWindow,
) -> Result<AlertDedupResult, AlertingError> {
    let window = normalize_dedup_window(window)?;
    let mut ordered_alerts = alerts
        .iter()
        .cloned()
        .map(normalize_fired_alert_record)
        .collect::<Result<Vec<_>, _>>()?;
    ordered_alerts.sort_by(|left, right| {
        left.fired_at
            .cmp(&right.fired_at)
            .then_with(|| left.alert_id.cmp(&right.alert_id))
    });

    let mut result = AlertDedupResult::default();
    let mut active_summary_by_key: BTreeMap<String, usize> = BTreeMap::new();

    for alert in ordered_alerts {
        let dedup_key = compute_alert_dedup_key(&alert)?;
        if outside_dedup_window(&alert, &window) {
            result.surfaced_alerts.push(alert.clone());
            result.summaries.push(single_alert_summary(
                dedup_key,
                &alert,
                false,
                "outside dedup window",
            ));
            continue;
        }

        if alert_bypasses_dedup_suppression(&alert) {
            result.bypassed_alert_ids.push(alert.alert_id.clone());
            result.surfaced_alerts.push(alert.clone());
            result.summaries.push(single_alert_summary(
                dedup_key,
                &alert,
                true,
                "severity bypassed dedup suppression",
            ));
            continue;
        }

        let key_value = dedup_key.stable_key();
        if let Some(summary_index) = active_summary_by_key.get(&key_value).copied() {
            let summary = &mut result.summaries[summary_index];
            summary.occurrence_count += 1;
            summary.suppressed_alert_ids.push(alert.alert_id.clone());
            summary.last_fired_at = alert.fired_at.clone();
            summary.summary = aggregation_summary_text(summary);
            result.suppressed_count += 1;
        } else {
            result.surfaced_alerts.push(alert.clone());
            result
                .summaries
                .push(single_alert_summary(dedup_key, &alert, false, "surfaced"));
            active_summary_by_key.insert(key_value, result.summaries.len() - 1);
        }
    }

    Ok(result)
}

pub fn route_alert_to_recipients(
    alert: &FiredAlertRecord,
    rules: &[AlertRoutingRule],
    default_operator: AlertRecipient,
) -> Result<AlertRoutingOutcome, AlertingError> {
    let alert = normalize_fired_alert_record(alert.clone())?;
    let default_operator = normalize_alert_recipient(default_operator)?;
    let normalized_rules = rules
        .iter()
        .cloned()
        .map(normalize_routing_rule)
        .collect::<Result<Vec<_>, _>>()?;

    let mut recipients_by_id: BTreeMap<String, AlertRecipient> = BTreeMap::new();
    let mut decisions = Vec::new();
    for rule in &normalized_rules {
        if !routing_rule_matches_alert(rule, &alert) {
            continue;
        }
        for recipient in &rule.recipients {
            if let Some(existing) = recipients_by_id.get(&recipient.recipient_id) {
                if existing.role != recipient.role {
                    return Err(AlertingError::RoutingRecipientRoleConflict {
                        recipient_id: recipient.recipient_id.clone(),
                        first_role: existing.role.clone(),
                        second_role: recipient.role.clone(),
                    });
                }
            } else {
                recipients_by_id.insert(recipient.recipient_id.clone(), recipient.clone());
            }
            decisions.push(routing_decision(&alert, Some(rule), recipient, false));
        }
    }

    if recipients_by_id.is_empty() {
        let decision = routing_decision(&alert, None, &default_operator, true);
        return Ok(AlertRoutingOutcome {
            alert_id: alert.alert_id,
            recipients: vec![default_operator],
            decisions: vec![decision],
            unrouted: true,
            default_operator_used: true,
        });
    }

    decisions.sort_by(|left, right| {
        left.rule_id
            .cmp(&right.rule_id)
            .then_with(|| left.recipient_id.cmp(&right.recipient_id))
    });

    Ok(AlertRoutingOutcome {
        alert_id: alert.alert_id,
        recipients: recipients_by_id.into_values().collect(),
        decisions,
        unrouted: false,
        default_operator_used: false,
    })
}

pub fn deliver_alert<A: ChannelAdapter>(
    adapter: &mut A,
    alert: &FiredAlertRecord,
    recipient: AlertRecipient,
) -> Result<DeliveryOutcome, AlertingError> {
    let alert = normalize_fired_alert_record(alert.clone())?;
    let recipient = normalize_alert_recipient(recipient)?;
    Ok(adapter.send(&alert, &recipient))
}

pub fn run_tracked_delivery<A: ChannelAdapter>(
    adapter: &mut A,
    alert: &FiredAlertRecord,
    recipient: AlertRecipient,
    policy: DeliveryRetryPolicy,
) -> Result<TrackedDelivery, AlertingError> {
    let policy = normalize_retry_policy(policy)?;
    let mut state = DeliveryState::Queued;
    let mut attempts = Vec::new();
    let mut transitions = Vec::new();

    for attempt_number in 1..=policy.max_attempts {
        transitions.push(delivery_transition(
            attempt_number,
            state,
            DeliveryState::Sending,
            None,
            None,
        ));

        let outcome = deliver_alert(adapter, alert, recipient.clone())?;
        let delivery_id = outcome.delivery_id.clone();

        match outcome.status {
            DeliveryStatus::Delivered => {
                attempts.push(outcome);
                transitions.push(delivery_transition(
                    attempt_number,
                    DeliveryState::Sending,
                    DeliveryState::Delivered,
                    None,
                    None,
                ));
                return Ok(TrackedDelivery {
                    delivery_id,
                    final_state: DeliveryState::Delivered,
                    attempts,
                    transitions,
                    last_error: None,
                    max_attempts: policy.max_attempts,
                });
            }
            DeliveryStatus::Failed => {
                let failure_error = outcome.error.clone();
                attempts.push(outcome);
                if attempt_number < policy.max_attempts {
                    let backoff_seconds = retry_backoff_seconds(&policy, attempt_number);
                    transitions.push(delivery_transition(
                        attempt_number,
                        DeliveryState::Sending,
                        DeliveryState::Failed,
                        None,
                        failure_error.clone(),
                    ));
                    transitions.push(delivery_transition(
                        attempt_number,
                        DeliveryState::Failed,
                        DeliveryState::Retrying,
                        Some(backoff_seconds),
                        failure_error.clone(),
                    ));
                    state = DeliveryState::Retrying;
                } else {
                    transitions.push(delivery_transition(
                        attempt_number,
                        DeliveryState::Sending,
                        DeliveryState::Failed,
                        None,
                        failure_error.clone(),
                    ));
                    return Ok(TrackedDelivery {
                        delivery_id,
                        final_state: DeliveryState::Failed,
                        attempts,
                        transitions,
                        last_error: failure_error,
                        max_attempts: policy.max_attempts,
                    });
                }
            }
        }
    }

    Err(AlertingError::InvalidRetryPolicy)
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

fn normalize_severity_evidence(
    evidence: AlertSeverityEvidence,
) -> Result<AlertSeverityEvidence, AlertingError> {
    let metric = normalize_required_text(evidence.metric, AlertingError::EmptySeverityMetric)?;
    let method_version = normalize_required_text(
        evidence.method_version,
        AlertingError::EmptySeverityMethodVersion,
    )?;
    if !evidence.observed_value.is_finite()
        || !evidence.warning_threshold.is_finite()
        || !evidence.critical_threshold.is_finite()
        || !evidence.emergency_threshold.is_finite()
        || evidence.warning_threshold > evidence.critical_threshold
        || evidence.critical_threshold > evidence.emergency_threshold
    {
        return Err(AlertingError::InvalidSeverityEvidence);
    }

    Ok(AlertSeverityEvidence {
        metric,
        observed_value: evidence.observed_value,
        warning_threshold: evidence.warning_threshold,
        critical_threshold: evidence.critical_threshold,
        emergency_threshold: evidence.emergency_threshold,
        method_version,
    })
}

fn normalize_dedup_window(window: AlertDedupWindow) -> Result<AlertDedupWindow, AlertingError> {
    let window_start =
        normalize_required_text(window.window_start, AlertingError::InvalidDedupWindow)?;
    let window_end = normalize_required_text(window.window_end, AlertingError::InvalidDedupWindow)?;
    if window_start > window_end {
        return Err(AlertingError::InvalidDedupWindow);
    }

    Ok(AlertDedupWindow {
        window_start,
        window_end,
    })
}

fn normalize_alert_recipient(recipient: AlertRecipient) -> Result<AlertRecipient, AlertingError> {
    Ok(AlertRecipient {
        recipient_id: normalize_required_text(
            recipient.recipient_id,
            AlertingError::EmptyRecipientId,
        )?,
        role: normalize_required_text(recipient.role, AlertingError::EmptyRecipientRole)?,
    })
}

fn normalize_routing_rule(rule: AlertRoutingRule) -> Result<AlertRoutingRule, AlertingError> {
    let rule_id = normalize_required_text(rule.rule_id, AlertingError::EmptyRoutingRuleId)?;
    let source_domain = rule
        .source_domain
        .map(|value| normalize_required_text(value, AlertingError::EmptySourceDomain))
        .transpose()?;
    let field_id = rule
        .field_id
        .map(|value| normalize_required_text(value, AlertingError::EmptySubjectRef))
        .transpose()?;
    let role = normalize_required_text(rule.role, AlertingError::EmptyRecipientRole)?;
    if rule.recipients.is_empty() {
        return Err(AlertingError::EmptyRoutingRecipientList);
    }
    let recipients = rule
        .recipients
        .into_iter()
        .map(normalize_alert_recipient)
        .collect::<Result<Vec<_>, _>>()?;
    for recipient in &recipients {
        if recipient.role != role {
            return Err(AlertingError::RoutingRecipientRoleMismatch {
                rule_id,
                expected_role: role,
                actual_role: recipient.role.clone(),
            });
        }
    }

    Ok(AlertRoutingRule {
        rule_id,
        source_domain,
        field_id,
        severity: rule.severity,
        role,
        recipients,
    })
}

fn normalize_retry_policy(
    policy: DeliveryRetryPolicy,
) -> Result<DeliveryRetryPolicy, AlertingError> {
    if policy.max_attempts == 0
        || policy.base_backoff_seconds == 0
        || policy.max_backoff_seconds == 0
        || policy.base_backoff_seconds > policy.max_backoff_seconds
    {
        return Err(AlertingError::InvalidRetryPolicy);
    }

    Ok(policy)
}

fn severity_from_evidence_or_rule(
    rule_severity: AlertSeverityHint,
    evidence: &AlertSeverityEvidence,
) -> (AlertSeverityHint, Option<f64>, Option<&'static str>) {
    if evidence.observed_value >= evidence.emergency_threshold {
        (
            AlertSeverityHint::Emergency,
            Some(evidence.emergency_threshold),
            Some("; threshold=emergency_threshold"),
        )
    } else if evidence.observed_value >= evidence.critical_threshold {
        (
            AlertSeverityHint::Critical,
            Some(evidence.critical_threshold),
            Some("; threshold=critical_threshold"),
        )
    } else if evidence.observed_value >= evidence.warning_threshold {
        (
            AlertSeverityHint::Warning,
            Some(evidence.warning_threshold),
            Some("; threshold=warning_threshold"),
        )
    } else {
        (rule_severity, None, None)
    }
}

fn outside_dedup_window(alert: &FiredAlertRecord, window: &AlertDedupWindow) -> bool {
    alert.fired_at < window.window_start || alert.fired_at > window.window_end
}

fn alert_bypasses_dedup_suppression(alert: &FiredAlertRecord) -> bool {
    matches!(
        alert.severity,
        AlertSeverityHint::Critical | AlertSeverityHint::Emergency
    )
}

fn routing_rule_matches_alert(rule: &AlertRoutingRule, alert: &FiredAlertRecord) -> bool {
    rule.source_domain
        .as_deref()
        .map_or(true, |source_domain| source_domain == alert.source_domain)
        && rule
            .field_id
            .as_deref()
            .map_or(true, |field_id| alert.field_id.as_deref() == Some(field_id))
        && rule
            .severity
            .map_or(true, |severity| severity == alert.severity)
}

fn routing_decision(
    alert: &FiredAlertRecord,
    rule: Option<&AlertRoutingRule>,
    recipient: &AlertRecipient,
    default_operator: bool,
) -> AlertRoutingDecision {
    let rule_id = rule.map(|rule| rule.rule_id.clone());
    let audit_detail = if let Some(rule) = rule {
        format!(
            "routing rule {} matched alert {} for source_domain {} field {} severity {}; recipient {} role {}; channels {}",
            rule.rule_id,
            alert.alert_id,
            alert.source_domain,
            alert.field_id.as_deref().unwrap_or("none"),
            alert.severity.as_str(),
            recipient.recipient_id,
            recipient.role,
            alert.channels.join(",")
        )
    } else {
        format!(
            "alert {} unrouted: no routing rule matched source_domain {} field {} severity {}; surfaced to default operator {} role {}; channels {}",
            alert.alert_id,
            alert.source_domain,
            alert.field_id.as_deref().unwrap_or("none"),
            alert.severity.as_str(),
            recipient.recipient_id,
            recipient.role,
            alert.channels.join(",")
        )
    };

    AlertRoutingDecision {
        alert_id: alert.alert_id.clone(),
        rule_id,
        recipient_id: recipient.recipient_id.clone(),
        role: recipient.role.clone(),
        source_domain: alert.source_domain.clone(),
        field_id: alert.field_id.clone(),
        severity: alert.severity,
        channels: alert.channels.clone(),
        default_operator,
        audit_detail,
    }
}

fn retry_backoff_seconds(policy: &DeliveryRetryPolicy, failed_attempt_number: usize) -> u64 {
    let mut backoff = policy.base_backoff_seconds;
    for _ in 1..failed_attempt_number {
        backoff = backoff.saturating_mul(2).min(policy.max_backoff_seconds);
    }
    backoff.min(policy.max_backoff_seconds)
}

fn delivery_transition(
    attempt_number: usize,
    from: DeliveryState,
    to: DeliveryState,
    backoff_seconds: Option<u64>,
    error: Option<String>,
) -> DeliveryStateTransition {
    DeliveryStateTransition {
        attempt_number,
        from,
        to,
        backoff_seconds,
        error,
    }
}

fn delivered_outcome(
    channel: AlertChannel,
    alert: &FiredAlertRecord,
    recipient: &AlertRecipient,
) -> DeliveryOutcome {
    DeliveryOutcome {
        delivery_id: delivery_id(channel, alert, recipient),
        alert_id: alert.alert_id.clone(),
        recipient_id: recipient.recipient_id.clone(),
        channel,
        status: DeliveryStatus::Delivered,
        attempted_at: alert.fired_at.clone(),
        delivered_at: Some(alert.fired_at.clone()),
        error: None,
    }
}

fn failed_outcome(
    channel: AlertChannel,
    alert: &FiredAlertRecord,
    recipient: &AlertRecipient,
    error: &str,
) -> DeliveryOutcome {
    let normalized_error = error.trim();
    DeliveryOutcome {
        delivery_id: delivery_id(channel, alert, recipient),
        alert_id: alert.alert_id.clone(),
        recipient_id: recipient.recipient_id.clone(),
        channel,
        status: DeliveryStatus::Failed,
        attempted_at: alert.fired_at.clone(),
        delivered_at: None,
        error: Some(if normalized_error.is_empty() {
            "channel delivery failed".to_string()
        } else {
            normalized_error.to_string()
        }),
    }
}

fn delivery_id(
    channel: AlertChannel,
    alert: &FiredAlertRecord,
    recipient: &AlertRecipient,
) -> String {
    format!(
        "delivery:{}:{}:{}",
        channel.as_str(),
        recipient.recipient_id,
        alert.alert_id
    )
}

fn single_alert_summary(
    dedup_key: AlertDedupKey,
    alert: &FiredAlertRecord,
    bypassed_suppression: bool,
    reason: &str,
) -> AlertDedupSummary {
    let mut summary = AlertDedupSummary {
        dedup_key,
        surfaced_alert_id: alert.alert_id.clone(),
        occurrence_count: 1,
        suppressed_alert_ids: Vec::new(),
        first_fired_at: alert.fired_at.clone(),
        last_fired_at: alert.fired_at.clone(),
        severity: alert.severity,
        bypassed_suppression,
        summary: String::new(),
    };
    summary.summary = if bypassed_suppression {
        format!(
            "alert {} surfaced immediately; {}",
            summary.surfaced_alert_id, reason
        )
    } else {
        aggregation_summary_text(&summary)
    };
    summary
}

fn aggregation_summary_text(summary: &AlertDedupSummary) -> String {
    format!(
        "{} occurrences aggregated for {}; surfaced {}; suppressed {} repeats from {} through {}",
        summary.occurrence_count,
        summary.dedup_key.stable_key(),
        summary.surfaced_alert_id,
        summary.suppressed_alert_ids.len(),
        summary.first_fired_at,
        summary.last_fired_at
    )
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
        classify_alert_severity, compute_alert_dedup_key, deduplicate_alert_stream, deliver_alert,
        evaluate_alert_rules, filter_alert_history, route_alert_to_recipients,
        run_tracked_delivery, AlertChannel, AlertDedupWindow, AlertEvent, AlertEventBackbone,
        AlertHistoryQuery, AlertRecipient, AlertRoutingRule, AlertRule, AlertSeverityEvidence,
        AlertSeverityHint, AlertingError, DeliveryRetryPolicy, DeliveryState, DeliveryStatus,
        InAppChannelAdapter, MockChannelAdapter, SourceAdapter,
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

    #[test]
    fn severity_classifier_derives_critical_from_threshold_evidence() {
        let alert = fired_alert(
            "alert-critical",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Warning,
            "2026-06-12T10:00:00Z",
        );

        let classification = classify_alert_severity(
            &alert,
            AlertSeverityHint::Warning,
            severity_evidence(92.0, 50.0, 90.0, 99.0),
        )
        .expect("severity should classify");

        assert_eq!(
            classification.classified_severity,
            AlertSeverityHint::Critical
        );
        assert_eq!(classification.rule_severity, AlertSeverityHint::Warning);
        assert_eq!(
            classification.source_severity_hint,
            AlertSeverityHint::Warning
        );
        assert_eq!(classification.threshold_value, Some(90.0));
        assert!(classification.hard_override_downstream);
        assert!(classification.explanation.contains("critical_threshold=90"));
    }

    #[test]
    fn severity_classifier_resolves_hint_conflict_with_rule_and_evidence() {
        let alert = fired_alert(
            "alert-conflict",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Critical,
            "2026-06-12T10:00:00Z",
        );

        let classification = classify_alert_severity(
            &alert,
            AlertSeverityHint::Info,
            severity_evidence(12.0, 50.0, 90.0, 99.0),
        )
        .expect("severity should classify");

        assert_eq!(
            classification.classified_severity,
            AlertSeverityHint::Critical
        );
        assert_eq!(classification.rule_severity, AlertSeverityHint::Critical);
        assert_eq!(classification.source_severity_hint, AlertSeverityHint::Info);
        assert_eq!(classification.threshold_value, None);
        assert!(classification.hard_override_downstream);
        assert!(classification
            .explanation
            .contains("source hint info ignored"));
    }

    #[test]
    fn dedup_key_uses_source_subject_and_rule() {
        let alert = fired_alert(
            "alert-dedup-key",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Warning,
            "2026-06-12T10:00:00Z",
        );

        let key = compute_alert_dedup_key(&alert).expect("dedup key should compute");

        assert_eq!(key.source_domain, "27-soil-iot-sensor-network");
        assert_eq!(key.subject_ref, "field:field-alpha");
        assert_eq!(key.rule_id, "rule-sensor-stale-critical");
        assert_eq!(
            key.stable_key(),
            "27-soil-iot-sensor-network|field:field-alpha|rule-sensor-stale-critical"
        );
    }

    #[test]
    fn dedup_window_counts_repeats_on_one_surfaced_alert() {
        let alerts = vec![
            fired_alert(
                "alert-repeat-001",
                "27-soil-iot-sensor-network",
                Some("field-alpha"),
                AlertSeverityHint::Warning,
                "2026-06-12T10:00:00Z",
            ),
            fired_alert(
                "alert-repeat-002",
                "27-soil-iot-sensor-network",
                Some("field-alpha"),
                AlertSeverityHint::Warning,
                "2026-06-12T10:01:00Z",
            ),
            fired_alert(
                "alert-repeat-003",
                "27-soil-iot-sensor-network",
                Some("field-alpha"),
                AlertSeverityHint::Warning,
                "2026-06-12T10:02:00Z",
            ),
        ];

        let result = deduplicate_alert_stream(&alerts, dedup_window())
            .expect("warning repeats should aggregate");

        assert_eq!(result.surfaced_alerts.len(), 1);
        assert_eq!(result.surfaced_alerts[0].alert_id, "alert-repeat-001");
        assert_eq!(result.suppressed_count, 2);
        assert_eq!(result.summaries.len(), 1);
        assert_eq!(result.summaries[0].occurrence_count, 3);
        assert_eq!(
            result.summaries[0].suppressed_alert_ids,
            vec!["alert-repeat-002", "alert-repeat-003"]
        );
        assert!(result.summaries[0]
            .summary
            .contains("3 occurrences aggregated"));
    }

    #[test]
    fn storm_stream_surfaces_one_alert_with_occurrence_count() {
        let mut alerts = Vec::new();
        for index in 0..100 {
            alerts.push(fired_alert(
                &format!("alert-storm-{index:03}"),
                "27-soil-iot-sensor-network",
                Some("field-alpha"),
                AlertSeverityHint::Warning,
                &format!("2026-06-12T10:{:02}:00Z", index % 60),
            ));
        }

        let result = deduplicate_alert_stream(&alerts, dedup_window())
            .expect("storm stream should aggregate");

        assert_eq!(result.surfaced_alerts.len(), 1);
        assert_eq!(result.summaries[0].occurrence_count, 100);
        assert_eq!(result.suppressed_count, 99);
    }

    #[test]
    fn critical_alert_bypasses_dedup_suppression() {
        let alerts = vec![
            fired_alert(
                "alert-warning-001",
                "27-soil-iot-sensor-network",
                Some("field-alpha"),
                AlertSeverityHint::Warning,
                "2026-06-12T10:00:00Z",
            ),
            fired_alert(
                "alert-critical-bypass",
                "27-soil-iot-sensor-network",
                Some("field-alpha"),
                AlertSeverityHint::Critical,
                "2026-06-12T10:01:00Z",
            ),
            fired_alert(
                "alert-warning-002",
                "27-soil-iot-sensor-network",
                Some("field-alpha"),
                AlertSeverityHint::Warning,
                "2026-06-12T10:02:00Z",
            ),
        ];

        let result = deduplicate_alert_stream(&alerts, dedup_window())
            .expect("critical alert should bypass suppression");

        assert_eq!(result.surfaced_alerts.len(), 2);
        assert_eq!(result.surfaced_alerts[0].alert_id, "alert-warning-001");
        assert_eq!(result.surfaced_alerts[1].alert_id, "alert-critical-bypass");
        assert_eq!(result.suppressed_count, 1);
        assert_eq!(result.bypassed_alert_ids, vec!["alert-critical-bypass"]);
    }

    #[test]
    fn in_app_delivery_records_outcome_and_feed_item() {
        let alert = fired_alert(
            "alert-in-app",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Warning,
            "2026-06-12T10:00:00Z",
        );
        let recipient = alert_recipient("ops-001");
        let mut adapter = InAppChannelAdapter::default();

        let outcome =
            deliver_alert(&mut adapter, &alert, recipient).expect("in-app delivery should run");

        assert_eq!(outcome.delivery_id, "delivery:in_app:ops-001:alert-in-app");
        assert_eq!(outcome.alert_id, "alert-in-app");
        assert_eq!(outcome.recipient_id, "ops-001");
        assert_eq!(outcome.channel, AlertChannel::InApp);
        assert_eq!(outcome.status, DeliveryStatus::Delivered);
        assert_eq!(outcome.error, None);

        let feed = adapter.feed_for("ops-001");
        assert_eq!(feed.len(), 1);
        assert_eq!(feed[0].alert_id, "alert-in-app");
        assert_eq!(feed[0].recipient_id, "ops-001");
        assert_eq!(feed[0].delivered_at, "2026-06-12T10:00:00Z");
    }

    #[test]
    fn mock_channel_adapter_records_successful_delivery_outcome() {
        let alert = fired_alert(
            "alert-email",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Warning,
            "2026-06-12T10:00:00Z",
        );
        let mut adapter = MockChannelAdapter::succeeding(AlertChannel::Email);

        let outcome = deliver_alert(&mut adapter, &alert, alert_recipient("ag-001"))
            .expect("mock channel should run");

        assert_eq!(outcome.channel, AlertChannel::Email);
        assert_eq!(outcome.status, DeliveryStatus::Delivered);
        assert_eq!(adapter.recorded_outcomes(), vec![outcome]);
    }

    #[test]
    fn channel_adapter_error_is_recorded_as_failed_delivery_outcome() {
        let alert = fired_alert(
            "alert-webhook",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Warning,
            "2026-06-12T10:00:00Z",
        );
        let mut adapter =
            MockChannelAdapter::failing(AlertChannel::Webhook, "provider timeout".to_string());

        let outcome = deliver_alert(&mut adapter, &alert, alert_recipient("ops-001"))
            .expect("adapter failure should be recorded, not returned as an error");

        assert_eq!(outcome.channel, AlertChannel::Webhook);
        assert_eq!(outcome.status, DeliveryStatus::Failed);
        assert_eq!(outcome.error, Some("provider timeout".to_string()));
        assert_eq!(adapter.recorded_outcomes().len(), 1);
        assert_eq!(adapter.recorded_outcomes()[0], outcome);
    }

    #[test]
    fn delivery_tracking_retries_transient_failure_to_delivered() {
        let alert = fired_alert(
            "alert-flaky",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Warning,
            "2026-06-12T10:00:00Z",
        );
        let mut adapter =
            MockChannelAdapter::flaky(AlertChannel::Email, 2, "temporary timeout".to_string());

        let tracked = run_tracked_delivery(
            &mut adapter,
            &alert,
            alert_recipient("ops-001"),
            retry_policy(),
        )
        .expect("transient channel should eventually deliver");

        assert_eq!(tracked.final_state, DeliveryState::Delivered);
        assert_eq!(tracked.attempts.len(), 3);
        assert_eq!(tracked.last_error, None);
        assert_eq!(tracked.transitions[0].from, DeliveryState::Queued);
        assert_eq!(tracked.transitions[0].to, DeliveryState::Sending);
        assert_eq!(tracked.transitions[1].from, DeliveryState::Sending);
        assert_eq!(tracked.transitions[1].to, DeliveryState::Failed);
        assert_eq!(tracked.transitions[2].from, DeliveryState::Failed);
        assert_eq!(tracked.transitions[2].to, DeliveryState::Retrying);
        assert_eq!(tracked.transitions[2].backoff_seconds, Some(5));
        assert_eq!(tracked.transitions[5].backoff_seconds, Some(10));
        assert_eq!(tracked.transitions[7].to, DeliveryState::Delivered);
    }

    #[test]
    fn delivery_tracking_exhausts_attempt_cap_to_terminal_failed() {
        let alert = fired_alert(
            "alert-down",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Warning,
            "2026-06-12T10:00:00Z",
        );
        let mut adapter =
            MockChannelAdapter::failing(AlertChannel::Webhook, "provider down".to_string());

        let tracked = run_tracked_delivery(
            &mut adapter,
            &alert,
            alert_recipient("ops-001"),
            DeliveryRetryPolicy {
                max_attempts: 2,
                base_backoff_seconds: 5,
                max_backoff_seconds: 20,
            },
        )
        .expect("terminal channel failure should be recorded");

        assert_eq!(tracked.final_state, DeliveryState::Failed);
        assert_eq!(tracked.attempts.len(), 2);
        assert_eq!(tracked.last_error, Some("provider down".to_string()));
        assert_eq!(
            tracked.transitions.last().unwrap().from,
            DeliveryState::Sending
        );
        assert_eq!(
            tracked.transitions.last().unwrap().to,
            DeliveryState::Failed
        );
        assert_eq!(tracked.transitions.last().unwrap().backoff_seconds, None);
    }

    #[test]
    fn routing_matches_critical_field_alert_to_agronomist_and_audits_decision() {
        let alert = fired_alert(
            "alert-critical-field",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Critical,
            "2026-06-12T10:00:00Z",
        );
        let outcome = route_alert_to_recipients(
            &alert,
            &[routing_rule(
                "route-field-alpha-critical-agronomist",
                Some("27-soil-iot-sensor-network"),
                Some("field-alpha"),
                Some(AlertSeverityHint::Critical),
                "agronomist",
                vec![role_recipient("ag-001", "agronomist")],
            )],
            role_recipient("ops-default", "operator"),
        )
        .expect("critical field alert should route");

        assert!(!outcome.unrouted);
        assert!(!outcome.default_operator_used);
        assert_eq!(
            outcome.recipients,
            vec![role_recipient("ag-001", "agronomist")]
        );
        assert_eq!(outcome.decisions.len(), 1);
        assert_eq!(
            outcome.decisions[0].rule_id.as_deref(),
            Some("route-field-alpha-critical-agronomist")
        );
        assert_eq!(outcome.decisions[0].recipient_id, "ag-001");
        assert_eq!(outcome.decisions[0].role, "agronomist");
        assert_eq!(
            outcome.decisions[0].field_id.as_deref(),
            Some("field-alpha")
        );
        assert_eq!(outcome.decisions[0].severity, AlertSeverityHint::Critical);
        assert!(outcome.decisions[0]
            .audit_detail
            .contains("route-field-alpha-critical-agronomist"));
    }

    #[test]
    fn routing_flags_unrouted_alert_and_surfaces_default_operator() {
        let alert = fired_alert(
            "alert-unmatched",
            "25-predictive-maintenance-fleet-health",
            None,
            AlertSeverityHint::Warning,
            "2026-06-12T10:00:00Z",
        );
        let outcome = route_alert_to_recipients(
            &alert,
            &[routing_rule(
                "route-field-alpha-critical-agronomist",
                Some("27-soil-iot-sensor-network"),
                Some("field-alpha"),
                Some(AlertSeverityHint::Critical),
                "agronomist",
                vec![role_recipient("ag-001", "agronomist")],
            )],
            role_recipient("ops-default", "operator"),
        )
        .expect("unmatched alert should route to default operator");

        assert!(outcome.unrouted);
        assert!(outcome.default_operator_used);
        assert_eq!(
            outcome.recipients,
            vec![role_recipient("ops-default", "operator")]
        );
        assert_eq!(outcome.decisions.len(), 1);
        assert_eq!(outcome.decisions[0].rule_id, None);
        assert_eq!(outcome.decisions[0].recipient_id, "ops-default");
        assert!(outcome.decisions[0].default_operator);
        assert!(outcome.decisions[0].audit_detail.contains("unrouted"));
    }

    #[test]
    fn routing_rule_rejects_recipient_role_mismatch() {
        let alert = fired_alert(
            "alert-role-mismatch",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Critical,
            "2026-06-12T10:00:00Z",
        );

        let error = route_alert_to_recipients(
            &alert,
            &[routing_rule(
                "route-field-alpha-critical-agronomist",
                Some("27-soil-iot-sensor-network"),
                Some("field-alpha"),
                Some(AlertSeverityHint::Critical),
                "agronomist",
                vec![role_recipient("ops-001", "operator")],
            )],
            role_recipient("ops-default", "operator"),
        )
        .expect_err("routing rule should reject recipients outside its role");

        assert_eq!(
            error,
            AlertingError::RoutingRecipientRoleMismatch {
                rule_id: "route-field-alpha-critical-agronomist".to_string(),
                expected_role: "agronomist".to_string(),
                actual_role: "operator".to_string(),
            }
        );
    }

    #[test]
    fn routing_rejects_cross_rule_recipient_role_conflict() {
        let alert = fired_alert(
            "alert-cross-rule-role-conflict",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Critical,
            "2026-06-12T10:00:00Z",
        );

        let error = route_alert_to_recipients(
            &alert,
            &[
                routing_rule(
                    "route-agronomist",
                    Some("27-soil-iot-sensor-network"),
                    Some("field-alpha"),
                    Some(AlertSeverityHint::Critical),
                    "agronomist",
                    vec![role_recipient("shared-user-001", "agronomist")],
                ),
                routing_rule(
                    "route-operator",
                    Some("27-soil-iot-sensor-network"),
                    Some("field-alpha"),
                    Some(AlertSeverityHint::Critical),
                    "operator",
                    vec![role_recipient("shared-user-001", "operator")],
                ),
            ],
            role_recipient("ops-default", "operator"),
        )
        .expect_err("same recipient cannot be routed under conflicting roles");

        assert_eq!(
            error,
            AlertingError::RoutingRecipientRoleConflict {
                recipient_id: "shared-user-001".to_string(),
                first_role: "agronomist".to_string(),
                second_role: "operator".to_string(),
            }
        );
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

    fn severity_evidence(
        observed_value: f64,
        warning_threshold: f64,
        critical_threshold: f64,
        emergency_threshold: f64,
    ) -> AlertSeverityEvidence {
        AlertSeverityEvidence {
            metric: "battery_temperature_celsius".to_string(),
            observed_value,
            warning_threshold,
            critical_threshold,
            emergency_threshold,
            method_version: "severity-thresholds-v1".to_string(),
        }
    }

    fn dedup_window() -> AlertDedupWindow {
        AlertDedupWindow {
            window_start: "2026-06-12T10:00:00Z".to_string(),
            window_end: "2026-06-12T10:59:59Z".to_string(),
        }
    }

    fn alert_recipient(recipient_id: &str) -> AlertRecipient {
        AlertRecipient {
            recipient_id: recipient_id.to_string(),
            role: "operator".to_string(),
        }
    }

    fn role_recipient(recipient_id: &str, role: &str) -> AlertRecipient {
        AlertRecipient {
            recipient_id: recipient_id.to_string(),
            role: role.to_string(),
        }
    }

    fn routing_rule(
        rule_id: &str,
        source_domain: Option<&str>,
        field_id: Option<&str>,
        severity: Option<AlertSeverityHint>,
        role: &str,
        recipients: Vec<AlertRecipient>,
    ) -> AlertRoutingRule {
        AlertRoutingRule {
            rule_id: rule_id.to_string(),
            source_domain: source_domain.map(ToString::to_string),
            field_id: field_id.map(ToString::to_string),
            severity,
            role: role.to_string(),
            recipients,
        }
    }

    fn retry_policy() -> DeliveryRetryPolicy {
        DeliveryRetryPolicy {
            max_attempts: 3,
            base_backoff_seconds: 5,
            max_backoff_seconds: 20,
        }
    }
}
