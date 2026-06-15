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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertIdempotencyDecision {
    pub idempotency_key: String,
    pub alert_candidate_id: String,
    pub first_seen_at: String,
    pub reemitted_at: String,
    pub duplicate_count: usize,
    pub decision: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertRuleStatus {
    Active,
    Disabled,
}

impl AlertRuleStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            AlertRuleStatus::Active => "active",
            AlertRuleStatus::Disabled => "disabled",
        }
    }
}

impl FromStr for AlertRuleStatus {
    type Err = AlertingError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            other => Err(AlertingError::InvalidRuleStatus(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AlertRuleCreateRequest {
    #[serde(default)]
    pub rule_id: Option<String>,
    #[serde(default)]
    pub event_type: String,
    #[serde(default)]
    pub subject_ref: Option<String>,
    pub severity: AlertSeverityHint,
    #[serde(default)]
    pub channels: Vec<String>,
    #[serde(default)]
    pub status: Option<AlertRuleStatus>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AlertRuleUpdateRequest {
    #[serde(default)]
    pub event_type: String,
    #[serde(default)]
    pub subject_ref: Option<String>,
    pub severity: AlertSeverityHint,
    #[serde(default)]
    pub channels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AlertRuleStatusUpdateRequest {
    pub status: AlertRuleStatus,
    #[serde(default)]
    pub actor_id: String,
    #[serde(default)]
    pub occurred_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertRuleRecord {
    pub rule_id: String,
    pub version: u32,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_ref: Option<String>,
    pub severity: AlertSeverityHint,
    pub channels: Vec<String>,
    pub status: AlertRuleStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertRuleAuditRecord {
    pub audit_id: String,
    pub rule_id: String,
    pub version: u32,
    pub previous_status: AlertRuleStatus,
    pub new_status: AlertRuleStatus,
    pub actor_id: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AlertRuleSubscriptionCreateRequest {
    #[serde(default)]
    pub subscription_id: Option<String>,
    #[serde(default)]
    pub rule_id: String,
    #[serde(default)]
    pub recipient_id: String,
    #[serde(default)]
    pub recipient_role: String,
    #[serde(default)]
    pub channels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertRuleSubscriptionRecord {
    pub subscription_id: String,
    pub rule_id: String,
    pub recipient_id: String,
    pub recipient_role: String,
    pub channels: Vec<String>,
    pub created_at: String,
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
pub struct AlertMessageTemplate {
    pub template_id: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedAlertMessage {
    pub template_id: String,
    pub alert_id: String,
    pub message: String,
    pub variables: Vec<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

impl FromStr for AlertChannel {
    type Err = AlertingError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "in_app" => Ok(AlertChannel::InApp),
            "email" => Ok(AlertChannel::Email),
            "sms" => Ok(AlertChannel::Sms),
            "webhook" => Ok(AlertChannel::Webhook),
            "push" => Ok(AlertChannel::Push),
            other => Err(AlertingError::InvalidChannel(other.to_string())),
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
pub enum AlertLifecycleState {
    Fired,
    Acknowledged,
    Resolved,
    AutoResolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertLifecycleTransition {
    pub alert_id: String,
    pub from: AlertLifecycleState,
    pub to: AlertLifecycleState,
    pub actor_id: String,
    pub at: String,
    pub audit_detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertLifecycleRecord {
    pub alert_id: String,
    pub source_event_ref: String,
    pub state: AlertLifecycleState,
    pub fired_at: String,
    pub transitions: Vec<AlertLifecycleTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertLifecycleAction {
    pub alert_id: String,
    pub state: AlertLifecycleState,
    pub transition: Option<AlertLifecycleTransition>,
    pub idempotent: bool,
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
pub struct ChannelDeliveryRecord {
    pub requested_channel: AlertChannel,
    pub delivery_channel: AlertChannel,
    pub fallback_used: bool,
    pub tracked_delivery: TrackedDelivery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnroutableDeliveryOutcome {
    pub alert_id: String,
    pub recipient_id: String,
    pub requested_channel: AlertChannel,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiChannelDeliveryResult {
    pub deliveries: Vec<ChannelDeliveryRecord>,
    pub unroutable: Vec<UnroutableDeliveryOutcome>,
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
    idempotency_index: BTreeMap<String, usize>,
    idempotency_decisions: Vec<AlertIdempotencyDecision>,
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
    #[error("rule_id cannot be empty")]
    EmptyRuleId,
    #[error("subscription rule_id {actual} does not match rule {expected}")]
    RuleIdMismatch { expected: String, actual: String },
    #[error("rule version must be greater than zero")]
    InvalidRuleVersion,
    #[error("rule status change audit_id cannot be empty")]
    EmptyRuleAuditId,
    #[error("rule status change actor_id cannot be empty")]
    EmptyRuleActorId,
    #[error("subscription_id cannot be empty")]
    EmptySubscriptionId,
    #[error("source_event_ref cannot be empty")]
    EmptySourceEventRef,
    #[error("channels cannot contain empty values")]
    EmptyChannel,
    #[error("fired_at cannot be empty")]
    EmptyFiredAt,
    #[error("explanation cannot be empty")]
    EmptyExplanation,
    #[error("template_id cannot be empty")]
    EmptyTemplateId,
    #[error("template body cannot be empty")]
    EmptyTemplateBody,
    #[error("template variable cannot be empty")]
    EmptyTemplateVariable,
    #[error("template variable {variable} is missing from alert evidence")]
    MissingTemplateVariable { variable: String },
    #[error("template variable {variable} is not closed")]
    UnclosedTemplateVariable { variable: String },
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
    #[error("lifecycle actor_id cannot be empty")]
    EmptyLifecycleActorId,
    #[error("lifecycle timestamp cannot be empty")]
    EmptyLifecycleTimestamp,
    #[error("invalid lifecycle transition for {alert_id}: {from:?} -> {attempted:?}")]
    InvalidLifecycleTransition {
        alert_id: String,
        from: AlertLifecycleState,
        attempted: AlertLifecycleState,
    },
    #[error(
        "invalid lifecycle timestamp order for {alert_id}: previous {previous_at} after attempted {attempted_at}"
    )]
    InvalidLifecycleTimestampOrder {
        alert_id: String,
        previous_at: String,
        attempted_at: String,
    },
    #[error("severity evidence must be finite with warning <= critical <= emergency thresholds")]
    InvalidSeverityEvidence,
    #[error("dedup window requires non-empty window_start <= window_end")]
    InvalidDedupWindow,
    #[error("retry policy requires max_attempts > 0 and bounded positive backoff")]
    InvalidRetryPolicy,
    #[error("invalid alert severity {0}")]
    InvalidSeverity(String),
    #[error("invalid alert rule status {0}")]
    InvalidRuleStatus(String),
    #[error("invalid alert channel {0}")]
    InvalidChannel(String),
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
                if let Some(index) = self.idempotency_index.get(&event.idempotency_key).copied() {
                    let candidate = &mut self.candidates[index];
                    let first_seen_at = candidate.accepted_at.clone();
                    candidate.occurred_at = event.occurred_at.clone();
                    candidate.accepted_at = event.occurred_at.clone();
                    for evidence_ref in event.evidence_refs {
                        if !candidate.evidence_refs.contains(&evidence_ref) {
                            candidate.evidence_refs.push(evidence_ref);
                        }
                    }
                    candidate.evidence_refs.sort();
                    let duplicate_count = self
                        .idempotency_decisions
                        .iter()
                        .filter(|decision| decision.idempotency_key == candidate.idempotency_key)
                        .count()
                        + 1;
                    self.idempotency_decisions.push(AlertIdempotencyDecision {
                        idempotency_key: candidate.idempotency_key.clone(),
                        alert_candidate_id: candidate.alert_candidate_id.clone(),
                        first_seen_at,
                        reemitted_at: candidate.accepted_at.clone(),
                        duplicate_count,
                        decision: "collapsed_to_existing_candidate".to_string(),
                    });
                    return Ok(candidate.clone());
                }
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
                self.idempotency_index
                    .insert(candidate.idempotency_key.clone(), self.candidates.len());
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

    pub fn list_idempotency_decisions(&self) -> Vec<AlertIdempotencyDecision> {
        self.idempotency_decisions.clone()
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

pub fn build_alert_rule_record(
    request: AlertRuleCreateRequest,
    generated_rule_id: String,
    created_at: String,
) -> Result<AlertRuleRecord, AlertingError> {
    let rule_id = normalize_optional_text(request.rule_id)
        .or_else(|| normalize_optional_text(Some(generated_rule_id)))
        .ok_or(AlertingError::EmptyRuleId)?;
    let created_at = normalize_required_text(created_at, AlertingError::EmptyOccurredAt)?;

    Ok(AlertRuleRecord {
        rule_id,
        version: 1,
        event_type: normalize_required_text(request.event_type, AlertingError::EmptyEventType)?,
        subject_ref: normalize_optional_text(request.subject_ref),
        severity: request.severity,
        channels: normalize_channels(request.channels)?,
        status: request.status.unwrap_or(AlertRuleStatus::Active),
        created_at: created_at.clone(),
        updated_at: created_at,
    })
}

pub fn version_alert_rule_record(
    current: &AlertRuleRecord,
    request: AlertRuleUpdateRequest,
    updated_at: String,
) -> Result<AlertRuleRecord, AlertingError> {
    validate_alert_rule_record(current)?;
    let updated_at = normalize_required_text(updated_at, AlertingError::EmptyOccurredAt)?;

    Ok(AlertRuleRecord {
        rule_id: current.rule_id.clone(),
        version: current
            .version
            .checked_add(1)
            .ok_or(AlertingError::InvalidRuleVersion)?,
        event_type: normalize_required_text(request.event_type, AlertingError::EmptyEventType)?,
        subject_ref: normalize_optional_text(request.subject_ref),
        severity: request.severity,
        channels: normalize_channels(request.channels)?,
        status: current.status,
        created_at: current.created_at.clone(),
        updated_at,
    })
}

pub fn transition_alert_rule_status(
    current: &AlertRuleRecord,
    request: AlertRuleStatusUpdateRequest,
    generated_audit_id: String,
) -> Result<(AlertRuleRecord, AlertRuleAuditRecord), AlertingError> {
    validate_alert_rule_record(current)?;
    let audit_id = normalize_required_text(generated_audit_id, AlertingError::EmptyRuleAuditId)?;
    let actor_id = normalize_required_text(request.actor_id, AlertingError::EmptyRuleActorId)?;
    let occurred_at = normalize_required_text(request.occurred_at, AlertingError::EmptyOccurredAt)?;
    let next_version = current
        .version
        .checked_add(1)
        .ok_or(AlertingError::InvalidRuleVersion)?;

    let mut next = current.clone();
    next.version = next_version;
    next.status = request.status;
    next.updated_at = occurred_at.clone();

    Ok((
        next,
        AlertRuleAuditRecord {
            audit_id,
            rule_id: current.rule_id.clone(),
            version: next_version,
            previous_status: current.status,
            new_status: request.status,
            actor_id,
            occurred_at,
        },
    ))
}

pub fn build_alert_rule_subscription(
    request: AlertRuleSubscriptionCreateRequest,
    rule: &AlertRuleRecord,
    generated_subscription_id: String,
    created_at: String,
) -> Result<AlertRuleSubscriptionRecord, AlertingError> {
    validate_alert_rule_record(rule)?;
    let subscription_id = normalize_optional_text(request.subscription_id)
        .or_else(|| normalize_optional_text(Some(generated_subscription_id)))
        .ok_or(AlertingError::EmptySubscriptionId)?;
    let request_rule_id = normalize_required_text(request.rule_id, AlertingError::EmptyRuleId)?;
    if request_rule_id != rule.rule_id {
        return Err(AlertingError::RuleIdMismatch {
            expected: rule.rule_id.clone(),
            actual: request_rule_id,
        });
    }
    let channels = normalize_channels(request.channels)?;
    if channels.is_empty() {
        return Err(AlertingError::EmptyChannel);
    }

    Ok(AlertRuleSubscriptionRecord {
        subscription_id,
        rule_id: rule.rule_id.clone(),
        recipient_id: normalize_required_text(
            request.recipient_id,
            AlertingError::EmptyRecipientId,
        )?,
        recipient_role: normalize_required_text(
            request.recipient_role,
            AlertingError::EmptyRecipientRole,
        )?,
        channels,
        created_at: normalize_required_text(created_at, AlertingError::EmptyOccurredAt)?,
    })
}

pub fn evaluate_managed_alert_rules(
    candidate: &AlertCandidateRecord,
    rules: &[AlertRuleRecord],
) -> RuleEvaluationOutcome {
    let active_rules = rules
        .iter()
        .filter(|rule| rule.status == AlertRuleStatus::Active)
        .map(|rule| AlertRule {
            rule_id: rule.rule_id.clone(),
            event_type: rule.event_type.clone(),
            subject_ref: rule.subject_ref.clone(),
            severity: rule.severity,
            channels: rule.channels.clone(),
        })
        .collect::<Vec<_>>();
    evaluate_alert_rules(candidate, &active_rules)
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

pub fn render_alert_message_template(
    template: AlertMessageTemplate,
    alert: &FiredAlertRecord,
    evidence: BTreeMap<String, String>,
) -> Result<RenderedAlertMessage, AlertingError> {
    let template_id =
        normalize_required_text(template.template_id, AlertingError::EmptyTemplateId)?;
    let body = normalize_required_text(template.body, AlertingError::EmptyTemplateBody)?;
    let alert = normalize_fired_alert_record(alert.clone())?;
    let values = alert_template_values(&alert, evidence)?;
    let (message, variables) = render_template_body(&body, &values)?;

    Ok(RenderedAlertMessage {
        template_id,
        alert_id: alert.alert_id,
        message,
        variables,
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

pub fn open_alert_lifecycle(
    alert: &FiredAlertRecord,
) -> Result<AlertLifecycleRecord, AlertingError> {
    let alert = normalize_fired_alert_record(alert.clone())?;
    Ok(AlertLifecycleRecord {
        alert_id: alert.alert_id,
        source_event_ref: alert.source_event_ref,
        state: AlertLifecycleState::Fired,
        fired_at: alert.fired_at,
        transitions: Vec::new(),
    })
}

pub fn acknowledge_alert(
    lifecycle: &mut AlertLifecycleRecord,
    actor_id: String,
    acknowledged_at: String,
) -> Result<AlertLifecycleAction, AlertingError> {
    let actor_id = normalize_required_text(actor_id, AlertingError::EmptyLifecycleActorId)?;
    let acknowledged_at =
        normalize_required_text(acknowledged_at, AlertingError::EmptyLifecycleTimestamp)?;

    match lifecycle.state {
        AlertLifecycleState::Fired => apply_lifecycle_transition(
            lifecycle,
            AlertLifecycleState::Acknowledged,
            actor_id,
            acknowledged_at,
        ),
        AlertLifecycleState::Acknowledged => idempotent_lifecycle_action(lifecycle),
        AlertLifecycleState::Resolved | AlertLifecycleState::AutoResolved => {
            Err(AlertingError::InvalidLifecycleTransition {
                alert_id: lifecycle.alert_id.clone(),
                from: lifecycle.state,
                attempted: AlertLifecycleState::Acknowledged,
            })
        }
    }
}

pub fn resolve_alert(
    lifecycle: &mut AlertLifecycleRecord,
    actor_id: String,
    resolved_at: String,
) -> Result<AlertLifecycleAction, AlertingError> {
    let actor_id = normalize_required_text(actor_id, AlertingError::EmptyLifecycleActorId)?;
    let resolved_at = normalize_required_text(resolved_at, AlertingError::EmptyLifecycleTimestamp)?;

    match lifecycle.state {
        AlertLifecycleState::Acknowledged => apply_lifecycle_transition(
            lifecycle,
            AlertLifecycleState::Resolved,
            actor_id,
            resolved_at,
        ),
        AlertLifecycleState::Resolved | AlertLifecycleState::AutoResolved => {
            idempotent_lifecycle_action(lifecycle)
        }
        AlertLifecycleState::Fired => Err(AlertingError::InvalidLifecycleTransition {
            alert_id: lifecycle.alert_id.clone(),
            from: lifecycle.state,
            attempted: AlertLifecycleState::Resolved,
        }),
    }
}

pub fn auto_resolve_alert(
    lifecycle: &mut AlertLifecycleRecord,
    source_ref: String,
    resolved_at: String,
) -> Result<AlertLifecycleAction, AlertingError> {
    let source_ref = normalize_required_text(source_ref, AlertingError::EmptyLifecycleActorId)?;
    let resolved_at = normalize_required_text(resolved_at, AlertingError::EmptyLifecycleTimestamp)?;

    match lifecycle.state {
        AlertLifecycleState::Fired | AlertLifecycleState::Acknowledged => {
            apply_lifecycle_transition(
                lifecycle,
                AlertLifecycleState::AutoResolved,
                source_ref,
                resolved_at,
            )
        }
        AlertLifecycleState::Resolved | AlertLifecycleState::AutoResolved => {
            idempotent_lifecycle_action(lifecycle)
        }
    }
}

pub fn deliver_alert<A: ChannelAdapter + ?Sized>(
    adapter: &mut A,
    alert: &FiredAlertRecord,
    recipient: AlertRecipient,
) -> Result<DeliveryOutcome, AlertingError> {
    let alert = normalize_fired_alert_record(alert.clone())?;
    let recipient = normalize_alert_recipient(recipient)?;
    Ok(adapter.send(&alert, &recipient))
}

pub fn run_tracked_delivery<A: ChannelAdapter + ?Sized>(
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

pub fn alert_channels_from_strings(
    channels: &[String],
) -> Result<Vec<AlertChannel>, AlertingError> {
    channels
        .iter()
        .map(|channel| AlertChannel::from_str(channel))
        .collect()
}

pub fn deliver_alert_multi_channel(
    adapters: &mut [&mut dyn ChannelAdapter],
    alert: &FiredAlertRecord,
    recipient: AlertRecipient,
    requested_channels: Vec<AlertChannel>,
    fallback_channel: Option<AlertChannel>,
    policy: DeliveryRetryPolicy,
) -> Result<MultiChannelDeliveryResult, AlertingError> {
    let alert = normalize_fired_alert_record(alert.clone())?;
    let recipient = normalize_alert_recipient(recipient)?;
    let policy = normalize_retry_policy(policy)?;
    let mut result = MultiChannelDeliveryResult::default();

    for requested_channel in requested_channels {
        let (delivery_channel, fallback_used) =
            if adapter_index_for_channel(adapters, requested_channel).is_some() {
                (requested_channel, false)
            } else if let Some(fallback_channel) = fallback_channel {
                if adapter_index_for_channel(adapters, fallback_channel).is_some() {
                    (fallback_channel, true)
                } else {
                    result.unroutable.push(unroutable_delivery(
                        &alert,
                        &recipient,
                        requested_channel,
                        "requested and fallback channels are unconfigured",
                    ));
                    continue;
                }
            } else {
                result.unroutable.push(unroutable_delivery(
                    &alert,
                    &recipient,
                    requested_channel,
                    "requested channel is unconfigured",
                ));
                continue;
            };

        let adapter_index = adapter_index_for_channel(adapters, delivery_channel)
            .expect("adapter availability checked above");
        let tracked_delivery = run_tracked_delivery(
            &mut *adapters[adapter_index],
            &alert,
            recipient.clone(),
            policy.clone(),
        )?;
        result.deliveries.push(ChannelDeliveryRecord {
            requested_channel,
            delivery_channel,
            fallback_used,
            tracked_delivery,
        });
    }

    Ok(result)
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

fn alert_template_values(
    alert: &FiredAlertRecord,
    evidence: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, AlertingError> {
    let mut values = BTreeMap::from([
        ("alert_id".to_string(), alert.alert_id.clone()),
        ("matched_rule_id".to_string(), alert.matched_rule_id.clone()),
        (
            "source_event_ref".to_string(),
            alert.source_event_ref.clone(),
        ),
        ("source_domain".to_string(), alert.source_domain.clone()),
        ("event_type".to_string(), alert.event_type.clone()),
        ("subject_ref".to_string(), alert.subject_ref.clone()),
        ("severity".to_string(), alert.severity.as_str().to_string()),
        ("channels".to_string(), alert.channels.join(",")),
        ("fired_at".to_string(), alert.fired_at.clone()),
        ("explanation".to_string(), alert.explanation.clone()),
        ("evidence_refs".to_string(), alert.evidence_refs.join(",")),
    ]);
    if let Some(field_id) = &alert.field_id {
        values.insert("field_id".to_string(), field_id.clone());
    }
    for (key, value) in evidence {
        let key = normalize_required_text(key, AlertingError::EmptyTemplateVariable)?;
        let value = normalize_required_text(
            value,
            AlertingError::MissingTemplateVariable {
                variable: key.clone(),
            },
        )?;
        values.insert(key, value);
    }
    Ok(values)
}

fn render_template_body(
    body: &str,
    values: &BTreeMap<String, String>,
) -> Result<(String, Vec<String>), AlertingError> {
    let mut rendered = String::with_capacity(body.len());
    let mut variables = Vec::new();
    let mut rest = body;

    while let Some(start) = rest.find("{{") {
        rendered.push_str(&rest[..start]);
        let after_open = &rest[start + 2..];
        let Some(end) = after_open.find("}}") else {
            return Err(AlertingError::UnclosedTemplateVariable {
                variable: after_open.trim().to_string(),
            });
        };
        let variable = after_open[..end].trim();
        if variable.is_empty() {
            return Err(AlertingError::EmptyTemplateVariable);
        }
        let value = values
            .get(variable)
            .ok_or_else(|| AlertingError::MissingTemplateVariable {
                variable: variable.to_string(),
            })?;
        rendered.push_str(value);
        variables.push(variable.to_string());
        rest = &after_open[end + 2..];
    }

    rendered.push_str(rest);
    variables.sort();
    variables.dedup();
    Ok((rendered, variables))
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

fn apply_lifecycle_transition(
    lifecycle: &mut AlertLifecycleRecord,
    to: AlertLifecycleState,
    actor_id: String,
    at: String,
) -> Result<AlertLifecycleAction, AlertingError> {
    let previous_at = validate_lifecycle_record(lifecycle)?;
    if at < previous_at {
        return Err(AlertingError::InvalidLifecycleTimestampOrder {
            alert_id: lifecycle.alert_id.clone(),
            previous_at,
            attempted_at: at,
        });
    }

    let from = lifecycle.state;
    let transition = AlertLifecycleTransition {
        alert_id: lifecycle.alert_id.clone(),
        from,
        to,
        actor_id,
        at,
        audit_detail: format!(
            "alert {} lifecycle transition {:?}->{:?} from source event {}",
            lifecycle.alert_id, from, to, lifecycle.source_event_ref
        ),
    };
    lifecycle.state = to;
    lifecycle.transitions.push(transition.clone());

    Ok(AlertLifecycleAction {
        alert_id: lifecycle.alert_id.clone(),
        state: lifecycle.state,
        transition: Some(transition),
        idempotent: false,
    })
}

fn idempotent_lifecycle_action(
    lifecycle: &AlertLifecycleRecord,
) -> Result<AlertLifecycleAction, AlertingError> {
    validate_lifecycle_record(lifecycle)?;
    Ok(AlertLifecycleAction {
        alert_id: lifecycle.alert_id.clone(),
        state: lifecycle.state,
        transition: None,
        idempotent: true,
    })
}

fn validate_lifecycle_record(lifecycle: &AlertLifecycleRecord) -> Result<String, AlertingError> {
    normalize_required_text(lifecycle.alert_id.clone(), AlertingError::EmptyAlertId)?;
    normalize_required_text(
        lifecycle.source_event_ref.clone(),
        AlertingError::EmptySourceEventRef,
    )?;
    let fired_at =
        normalize_required_text(lifecycle.fired_at.clone(), AlertingError::EmptyFiredAt)?;
    for transition in &lifecycle.transitions {
        normalize_required_text(transition.alert_id.clone(), AlertingError::EmptyAlertId)?;
        normalize_required_text(
            transition.actor_id.clone(),
            AlertingError::EmptyLifecycleActorId,
        )?;
        normalize_required_text(
            transition.at.clone(),
            AlertingError::EmptyLifecycleTimestamp,
        )?;
        normalize_required_text(
            transition.audit_detail.clone(),
            AlertingError::EmptyExplanation,
        )?;
    }

    Ok(lifecycle
        .transitions
        .last()
        .map(|transition| transition.at.clone())
        .unwrap_or(fired_at))
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

fn adapter_index_for_channel(
    adapters: &[&mut dyn ChannelAdapter],
    channel: AlertChannel,
) -> Option<usize> {
    adapters
        .iter()
        .position(|adapter| adapter.channel() == channel)
}

fn unroutable_delivery(
    alert: &FiredAlertRecord,
    recipient: &AlertRecipient,
    requested_channel: AlertChannel,
    reason: &str,
) -> UnroutableDeliveryOutcome {
    UnroutableDeliveryOutcome {
        alert_id: alert.alert_id.clone(),
        recipient_id: recipient.recipient_id.clone(),
        requested_channel,
        reason: reason.to_string(),
    }
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

fn validate_alert_rule_record(rule: &AlertRuleRecord) -> Result<(), AlertingError> {
    normalize_required_text(rule.rule_id.clone(), AlertingError::EmptyRuleId)?;
    if rule.version == 0 {
        return Err(AlertingError::InvalidRuleVersion);
    }
    normalize_required_text(rule.event_type.clone(), AlertingError::EmptyEventType)?;
    if let Some(subject_ref) = &rule.subject_ref {
        normalize_required_text(subject_ref.clone(), AlertingError::EmptySubjectRef)?;
    }
    normalize_channels(rule.channels.clone())?;
    normalize_required_text(rule.created_at.clone(), AlertingError::EmptyOccurredAt)?;
    normalize_required_text(rule.updated_at.clone(), AlertingError::EmptyOccurredAt)?;
    Ok(())
}

fn normalize_channels(channels: Vec<String>) -> Result<Vec<String>, AlertingError> {
    channels
        .into_iter()
        .map(|value| normalize_required_text(value, AlertingError::EmptyChannel))
        .collect()
}

fn normalize_required_text(value: String, error: AlertingError) -> Result<String, AlertingError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
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
    use std::collections::BTreeMap;

    use super::{
        acknowledge_alert, alert_channels_from_strings, auto_resolve_alert,
        build_alert_rule_record, build_alert_rule_subscription, classify_alert_severity,
        compute_alert_dedup_key, deduplicate_alert_stream, deliver_alert,
        deliver_alert_multi_channel, evaluate_alert_rules, evaluate_managed_alert_rules,
        filter_alert_history, open_alert_lifecycle, render_alert_message_template, resolve_alert,
        route_alert_to_recipients, run_tracked_delivery, transition_alert_rule_status,
        version_alert_rule_record, AlertChannel, AlertDedupWindow, AlertEvent, AlertEventBackbone,
        AlertHistoryQuery, AlertLifecycleRecord, AlertLifecycleState, AlertMessageTemplate,
        AlertRecipient, AlertRoutingRule, AlertRule, AlertRuleCreateRequest, AlertRuleStatus,
        AlertRuleStatusUpdateRequest, AlertRuleSubscriptionCreateRequest,
        AlertRuleSubscriptionRecord, AlertRuleUpdateRequest, AlertSeverityEvidence,
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
    fn source_adapter_collapses_reemitted_event_by_idempotency_key() {
        let mut backbone = AlertEventBackbone::default();
        let first = backbone
            .emit(sensor_health_event())
            .expect("first event should be accepted");
        let mut reemit = sensor_health_event();
        reemit.occurred_at = "2026-06-12T10:01:00Z".to_string();
        reemit
            .evidence_refs
            .push("reading:soil-probe-001:retry".to_string());

        let second = backbone
            .emit(reemit)
            .expect("duplicate idempotency key should be accepted");

        assert_eq!(first.alert_candidate_id, second.alert_candidate_id);
        assert_eq!(backbone.list_candidates().len(), 1);
        let candidate = &backbone.list_candidates()[0];
        assert_eq!(candidate.occurred_at, "2026-06-12T10:01:00Z");
        assert_eq!(
            candidate.evidence_refs,
            vec![
                "reading:soil-probe-001:latest".to_string(),
                "reading:soil-probe-001:retry".to_string()
            ]
        );
        let decisions = backbone.list_idempotency_decisions();
        assert_eq!(decisions.len(), 1);
        assert_eq!(
            decisions[0].decision,
            "collapsed_to_existing_candidate".to_string()
        );
        assert_eq!(decisions[0].duplicate_count, 1);
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
    fn alert_template_renders_from_alert_and_evidence_fields() {
        let alert = fired_alert(
            "alert-sensor-stale-001",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Critical,
            "2026-06-12T10:02:00Z",
        );
        let mut evidence = BTreeMap::new();
        evidence.insert("metric".to_string(), "soil_probe_age_minutes".to_string());
        evidence.insert("observed_value".to_string(), "74".to_string());
        evidence.insert("threshold".to_string(), "60".to_string());

        let rendered = render_alert_message_template(
            AlertMessageTemplate {
                template_id: "sensor-stale-critical-v1".to_string(),
                body: "{{severity}} {{event_type}} for {{field_id}}: {{metric}}={{observed_value}} over {{threshold}} at {{fired_at}}".to_string(),
            },
            &alert,
            evidence,
        )
        .expect("complete evidence should render");

        assert_eq!(rendered.template_id, "sensor-stale-critical-v1");
        assert_eq!(rendered.alert_id, "alert-sensor-stale-001");
        assert_eq!(
            rendered.message,
            "critical sensor_stale for field-alpha: soil_probe_age_minutes=74 over 60 at 2026-06-12T10:02:00Z"
        );
        assert_eq!(
            rendered.variables,
            vec![
                "event_type".to_string(),
                "field_id".to_string(),
                "fired_at".to_string(),
                "metric".to_string(),
                "observed_value".to_string(),
                "severity".to_string(),
                "threshold".to_string(),
            ]
        );
    }

    #[test]
    fn alert_template_missing_evidence_field_fails_without_partial_message() {
        let alert = fired_alert(
            "alert-sensor-stale-001",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Critical,
            "2026-06-12T10:02:00Z",
        );
        let mut evidence = BTreeMap::new();
        evidence.insert("metric".to_string(), "soil_probe_age_minutes".to_string());

        let error = render_alert_message_template(
            AlertMessageTemplate {
                template_id: "sensor-stale-critical-v1".to_string(),
                body: "{{severity}} {{metric}} over {{threshold}}".to_string(),
            },
            &alert,
            evidence,
        )
        .expect_err("missing template variable should refuse render");

        assert_eq!(
            error,
            AlertingError::MissingTemplateVariable {
                variable: "threshold".to_string()
            }
        );
    }

    #[test]
    fn rule_management_creates_versions_and_disables_firing_with_audit() {
        let rule = build_alert_rule_record(
            AlertRuleCreateRequest {
                rule_id: Some(" rule-sensor-stale ".to_string()),
                event_type: " sensor_stale ".to_string(),
                subject_ref: Some(" sensor:soil-probe-001 ".to_string()),
                severity: AlertSeverityHint::Critical,
                channels: vec![" in_app ".to_string()],
                status: None,
            },
            "generated-rule".to_string(),
            "2026-06-12T10:00:00Z".to_string(),
        )
        .expect("valid rule should build");
        assert_eq!(rule.rule_id, "rule-sensor-stale");
        assert_eq!(rule.version, 1);
        assert_eq!(rule.status, AlertRuleStatus::Active);
        assert_eq!(rule.subject_ref.as_deref(), Some("sensor:soil-probe-001"));

        let edited = version_alert_rule_record(
            &rule,
            AlertRuleUpdateRequest {
                event_type: "sensor_stale".to_string(),
                subject_ref: None,
                severity: AlertSeverityHint::Warning,
                channels: vec!["email".to_string()],
            },
            "2026-06-12T10:05:00Z".to_string(),
        )
        .expect("rule edit should create next version");
        assert_eq!(edited.version, 2);
        assert_eq!(edited.status, AlertRuleStatus::Active);
        assert_eq!(edited.severity, AlertSeverityHint::Warning);

        let (disabled, audit) = transition_alert_rule_status(
            &edited,
            AlertRuleStatusUpdateRequest {
                status: AlertRuleStatus::Disabled,
                actor_id: "ops-admin".to_string(),
                occurred_at: "2026-06-12T10:10:00Z".to_string(),
            },
            "audit-rule-disable".to_string(),
        )
        .expect("status change should version and audit");

        assert_eq!(disabled.version, 3);
        assert_eq!(disabled.status, AlertRuleStatus::Disabled);
        assert_eq!(audit.previous_status, AlertRuleStatus::Active);
        assert_eq!(audit.new_status, AlertRuleStatus::Disabled);
        assert_eq!(audit.actor_id, "ops-admin");

        let mut backbone = AlertEventBackbone::default();
        let candidate = backbone
            .emit(sensor_health_event())
            .expect("event should be accepted");
        let outcome = evaluate_managed_alert_rules(&candidate, &[disabled]);
        assert!(outcome.fired_alerts.is_empty());
    }

    #[test]
    fn malformed_rule_is_rejected_before_it_can_fire() {
        let error = build_alert_rule_record(
            AlertRuleCreateRequest {
                rule_id: Some("rule-invalid".to_string()),
                event_type: " ".to_string(),
                subject_ref: None,
                severity: AlertSeverityHint::Warning,
                channels: vec!["in_app".to_string()],
                status: None,
            },
            "generated-rule".to_string(),
            "2026-06-12T10:00:00Z".to_string(),
        )
        .expect_err("empty event_type is an invalid predicate");

        assert_eq!(error, AlertingError::EmptyEventType);
    }

    #[test]
    fn subscription_binds_recipient_role_and_channels_to_rule() {
        let rule = build_alert_rule_record(
            AlertRuleCreateRequest {
                rule_id: Some("rule-sensor-stale".to_string()),
                event_type: "sensor_stale".to_string(),
                subject_ref: None,
                severity: AlertSeverityHint::Critical,
                channels: vec!["in_app".to_string()],
                status: None,
            },
            "generated-rule".to_string(),
            "2026-06-12T10:00:00Z".to_string(),
        )
        .expect("valid rule should build");

        let subscription = build_alert_rule_subscription(
            AlertRuleSubscriptionCreateRequest {
                subscription_id: Some(" subscription-001 ".to_string()),
                rule_id: " rule-sensor-stale ".to_string(),
                recipient_id: " ops-user-001 ".to_string(),
                recipient_role: " operator ".to_string(),
                channels: vec![" in_app ".to_string(), " email ".to_string()],
            },
            &rule,
            "generated-subscription".to_string(),
            "2026-06-12T10:01:00Z".to_string(),
        )
        .expect("subscription should build");

        assert_eq!(subscription.subscription_id, "subscription-001");
        assert_eq!(subscription.rule_id, "rule-sensor-stale");
        assert_eq!(subscription.recipient_id, "ops-user-001");
        assert_eq!(subscription.recipient_role, "operator");
        assert_eq!(subscription.channels, vec!["in_app", "email"]);
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
    fn multi_channel_delivery_fans_out_email_and_sms_from_subscription() {
        let alert = fired_alert(
            "alert-multi-channel",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Critical,
            "2026-06-12T10:00:00Z",
        );
        let subscription = AlertRuleSubscriptionRecord {
            subscription_id: "sub-ag-001".to_string(),
            rule_id: "rule-sensor-stale-critical".to_string(),
            recipient_id: "ag-001".to_string(),
            recipient_role: "agronomist".to_string(),
            channels: vec!["email".to_string(), "sms".to_string()],
            created_at: "2026-06-12T09:00:00Z".to_string(),
        };
        let requested_channels = alert_channels_from_strings(&subscription.channels)
            .expect("subscription channels should parse");
        let mut email = MockChannelAdapter::succeeding(AlertChannel::Email);
        let mut sms = MockChannelAdapter::succeeding(AlertChannel::Sms);
        let mut adapters: Vec<&mut dyn super::ChannelAdapter> = vec![&mut email, &mut sms];

        let result = deliver_alert_multi_channel(
            &mut adapters,
            &alert,
            role_recipient(&subscription.recipient_id, &subscription.recipient_role),
            requested_channels,
            None,
            retry_policy(),
        )
        .expect("multi-channel delivery should run");

        assert_eq!(result.deliveries.len(), 2);
        assert!(result.unroutable.is_empty());
        assert_eq!(result.deliveries[0].requested_channel, AlertChannel::Email);
        assert_eq!(result.deliveries[0].delivery_channel, AlertChannel::Email);
        assert_eq!(
            result.deliveries[0].tracked_delivery.final_state,
            DeliveryState::Delivered
        );
        assert_eq!(result.deliveries[1].requested_channel, AlertChannel::Sms);
        assert_eq!(result.deliveries[1].delivery_channel, AlertChannel::Sms);
        assert_eq!(
            result.deliveries[1].tracked_delivery.final_state,
            DeliveryState::Delivered
        );
        assert_eq!(email.recorded_outcomes().len(), 1);
        assert_eq!(sms.recorded_outcomes().len(), 1);
    }

    #[test]
    fn unconfigured_channel_uses_fallback_or_records_unroutable() {
        let alert = fired_alert(
            "alert-unconfigured-channel",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Warning,
            "2026-06-12T10:00:00Z",
        );
        let mut in_app = InAppChannelAdapter::default();
        let mut fallback_adapters: Vec<&mut dyn super::ChannelAdapter> = vec![&mut in_app];

        let fallback_result = deliver_alert_multi_channel(
            &mut fallback_adapters,
            &alert,
            alert_recipient("ops-001"),
            vec![AlertChannel::Webhook],
            Some(AlertChannel::InApp),
            retry_policy(),
        )
        .expect("fallback delivery should run");

        assert_eq!(fallback_result.deliveries.len(), 1);
        assert!(fallback_result.unroutable.is_empty());
        assert_eq!(
            fallback_result.deliveries[0].requested_channel,
            AlertChannel::Webhook
        );
        assert_eq!(
            fallback_result.deliveries[0].delivery_channel,
            AlertChannel::InApp
        );
        assert!(fallback_result.deliveries[0].fallback_used);
        assert_eq!(in_app.feed_for("ops-001").len(), 1);

        let mut no_adapters: Vec<&mut dyn super::ChannelAdapter> = Vec::new();
        let unroutable_result = deliver_alert_multi_channel(
            &mut no_adapters,
            &alert,
            alert_recipient("ops-001"),
            vec![AlertChannel::Push],
            None,
            retry_policy(),
        )
        .expect("unroutable outcome should be recorded");

        assert!(unroutable_result.deliveries.is_empty());
        assert_eq!(unroutable_result.unroutable.len(), 1);
        assert_eq!(
            unroutable_result.unroutable[0].requested_channel,
            AlertChannel::Push
        );
        assert!(unroutable_result.unroutable[0]
            .reason
            .contains("unconfigured"));
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

    #[test]
    fn lifecycle_records_acknowledgement_and_resolution_with_actor_timestamp() {
        let alert = fired_alert(
            "alert-lifecycle",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Critical,
            "2026-06-12T10:00:00Z",
        );
        let mut lifecycle =
            open_alert_lifecycle(&alert).expect("fired alert should open lifecycle");

        assert_eq!(lifecycle.alert_id, "alert-lifecycle");
        assert_eq!(lifecycle.state, AlertLifecycleState::Fired);
        assert_eq!(lifecycle.fired_at, "2026-06-12T10:00:00Z");
        assert!(lifecycle.transitions.is_empty());

        let ack = acknowledge_alert(
            &mut lifecycle,
            "ops-001".to_string(),
            "2026-06-12T10:01:00Z".to_string(),
        )
        .expect("fired alert should acknowledge");

        assert_eq!(ack.state, AlertLifecycleState::Acknowledged);
        assert!(!ack.idempotent);
        let ack_transition = ack.transition.expect("ack should record transition");
        assert_eq!(ack_transition.from, AlertLifecycleState::Fired);
        assert_eq!(ack_transition.to, AlertLifecycleState::Acknowledged);
        assert_eq!(ack_transition.actor_id, "ops-001");
        assert_eq!(ack_transition.at, "2026-06-12T10:01:00Z");

        let resolved = resolve_alert(
            &mut lifecycle,
            "ops-002".to_string(),
            "2026-06-12T10:05:00Z".to_string(),
        )
        .expect("acknowledged alert should resolve");

        assert_eq!(resolved.state, AlertLifecycleState::Resolved);
        assert!(!resolved.idempotent);
        assert_eq!(lifecycle.state, AlertLifecycleState::Resolved);
        assert_eq!(lifecycle.transitions.len(), 2);
        assert_eq!(
            lifecycle.transitions[1].from,
            AlertLifecycleState::Acknowledged
        );
        assert_eq!(lifecycle.transitions[1].to, AlertLifecycleState::Resolved);
        assert_eq!(lifecycle.transitions[1].actor_id, "ops-002");
        assert_eq!(lifecycle.transitions[1].at, "2026-06-12T10:05:00Z");
    }

    #[test]
    fn resolving_already_resolved_alert_is_idempotent_without_duplicate_transition() {
        let alert = fired_alert(
            "alert-double-resolve",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Critical,
            "2026-06-12T10:00:00Z",
        );
        let mut lifecycle =
            open_alert_lifecycle(&alert).expect("fired alert should open lifecycle");
        acknowledge_alert(
            &mut lifecycle,
            "ops-001".to_string(),
            "2026-06-12T10:01:00Z".to_string(),
        )
        .expect("ack should succeed");
        resolve_alert(
            &mut lifecycle,
            "ops-002".to_string(),
            "2026-06-12T10:05:00Z".to_string(),
        )
        .expect("first resolve should succeed");

        let duplicate = resolve_alert(
            &mut lifecycle,
            "ops-003".to_string(),
            "2026-06-12T10:06:00Z".to_string(),
        )
        .expect("double resolve should be a no-op");

        assert_eq!(duplicate.state, AlertLifecycleState::Resolved);
        assert!(duplicate.idempotent);
        assert_eq!(duplicate.transition, None);
        assert_eq!(lifecycle.transitions.len(), 2);
    }

    #[test]
    fn manual_resolution_requires_acknowledgement() {
        let alert = fired_alert(
            "alert-direct-resolve",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Critical,
            "2026-06-12T10:00:00Z",
        );
        let mut lifecycle =
            open_alert_lifecycle(&alert).expect("fired alert should open lifecycle");

        let error = resolve_alert(
            &mut lifecycle,
            "ops-001".to_string(),
            "2026-06-12T10:05:00Z".to_string(),
        )
        .expect_err("manual resolve should require acknowledgement first");

        assert_eq!(
            error,
            AlertingError::InvalidLifecycleTransition {
                alert_id: "alert-direct-resolve".to_string(),
                from: AlertLifecycleState::Fired,
                attempted: AlertLifecycleState::Resolved,
            }
        );
        assert_eq!(lifecycle.state, AlertLifecycleState::Fired);
        assert!(lifecycle.transitions.is_empty());
    }

    #[test]
    fn auto_resolve_records_source_condition_clear_transition() {
        let alert = fired_alert(
            "alert-auto-resolve",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Warning,
            "2026-06-12T10:00:00Z",
        );
        let mut lifecycle =
            open_alert_lifecycle(&alert).expect("fired alert should open lifecycle");

        let action = auto_resolve_alert(
            &mut lifecycle,
            "source:sensor_stale_clear".to_string(),
            "2026-06-12T10:03:00Z".to_string(),
        )
        .expect("source-cleared condition should auto-resolve");

        assert_eq!(action.state, AlertLifecycleState::AutoResolved);
        assert!(!action.idempotent);
        let transition = action
            .transition
            .expect("auto-resolve should record transition");
        assert_eq!(transition.from, AlertLifecycleState::Fired);
        assert_eq!(transition.to, AlertLifecycleState::AutoResolved);
        assert_eq!(transition.actor_id, "source:sensor_stale_clear");
        assert_eq!(transition.at, "2026-06-12T10:03:00Z");
        assert_eq!(lifecycle.transitions.len(), 1);
    }

    #[test]
    fn lifecycle_rejects_transition_before_fired_or_previous_transition_time() {
        let alert = fired_alert(
            "alert-non-monotonic",
            "27-soil-iot-sensor-network",
            Some("field-alpha"),
            AlertSeverityHint::Critical,
            "2026-06-12T10:00:00Z",
        );
        let mut lifecycle =
            open_alert_lifecycle(&alert).expect("fired alert should open lifecycle");

        let early_ack = acknowledge_alert(
            &mut lifecycle,
            "ops-001".to_string(),
            "2026-06-12T09:59:59Z".to_string(),
        )
        .expect_err("ack before fired_at should be rejected");

        assert_eq!(
            early_ack,
            AlertingError::InvalidLifecycleTimestampOrder {
                alert_id: "alert-non-monotonic".to_string(),
                previous_at: "2026-06-12T10:00:00Z".to_string(),
                attempted_at: "2026-06-12T09:59:59Z".to_string(),
            }
        );
        assert!(lifecycle.transitions.is_empty());

        acknowledge_alert(
            &mut lifecycle,
            "ops-001".to_string(),
            "2026-06-12T10:01:00Z".to_string(),
        )
        .expect("ack should succeed");
        let early_resolve = resolve_alert(
            &mut lifecycle,
            "ops-002".to_string(),
            "2026-06-12T10:00:30Z".to_string(),
        )
        .expect_err("resolve before ack timestamp should be rejected");

        assert_eq!(
            early_resolve,
            AlertingError::InvalidLifecycleTimestampOrder {
                alert_id: "alert-non-monotonic".to_string(),
                previous_at: "2026-06-12T10:01:00Z".to_string(),
                attempted_at: "2026-06-12T10:00:30Z".to_string(),
            }
        );
        assert_eq!(lifecycle.transitions.len(), 1);
    }

    #[test]
    fn lifecycle_idempotent_path_validates_public_record_fields() {
        let mut malformed = AlertLifecycleRecord {
            alert_id: " ".to_string(),
            source_event_ref: "candidate:malformed".to_string(),
            state: AlertLifecycleState::Resolved,
            fired_at: "2026-06-12T10:00:00Z".to_string(),
            transitions: Vec::new(),
        };

        let error = resolve_alert(
            &mut malformed,
            "ops-001".to_string(),
            "2026-06-12T10:05:00Z".to_string(),
        )
        .expect_err("idempotent resolve should still validate public lifecycle record");

        assert_eq!(error, AlertingError::EmptyAlertId);
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
