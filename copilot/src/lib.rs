use serde::{Deserialize, Serialize};
use shared::schemas::{
    RecommendationLifecycleRegistry, RecommendationPersistenceError, RecommendationPriority,
    RecommendationRecord, RecommendationStatus,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Finding,
    ImageryProduct,
    LidarProduct,
    Report,
    Trend,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceCandidate {
    pub evidence_id: String,
    pub kind: EvidenceKind,
    pub field_id: String,
    pub scene_ref: Option<String>,
    pub zone_ref: Option<String>,
    pub ledger_ref: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceIndexEntry {
    pub evidence_id: String,
    pub kind: EvidenceKind,
    pub field_id: String,
    pub scene_ref: Option<String>,
    pub zone_ref: Option<String>,
    pub ledger_ref: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRetrievalIndex {
    pub field_id: String,
    pub entries: Vec<EvidenceIndexEntry>,
    pub rejected_items: Vec<RejectedEvidenceItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRejectionReason {
    DuplicateEvidenceId,
    EmptyEvidenceId,
    EmptyLedgerRef,
    EmptySummary,
    FieldMismatch,
    UnresolvedLedgerRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RejectedEvidenceItem {
    pub evidence_id: Option<String>,
    pub ledger_ref: Option<String>,
    pub reason: EvidenceRejectionReason,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CopilotIndexError {
    #[error("field_id cannot be empty")]
    EmptyFieldId,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CopilotConversationStartRequest {
    #[serde(default)]
    pub conversation_id: Option<String>,
    pub field_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotConversationRecord {
    pub conversation_id: String,
    pub field_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopilotTurnRole {
    User,
    Assistant,
    System,
}

impl CopilotTurnRole {
    pub fn as_str(self) -> &'static str {
        match self {
            CopilotTurnRole::User => "user",
            CopilotTurnRole::Assistant => "assistant",
            CopilotTurnRole::System => "system",
        }
    }
}

impl std::str::FromStr for CopilotTurnRole {
    type Err = CopilotConversationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "user" => Ok(CopilotTurnRole::User),
            "assistant" => Ok(CopilotTurnRole::Assistant),
            "system" => Ok(CopilotTurnRole::System),
            _ => Err(CopilotConversationError::UnsupportedRole {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CopilotTurnCreateRequest {
    #[serde(default)]
    pub turn_id: Option<String>,
    pub field_id: String,
    pub role: CopilotTurnRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotTurnRecord {
    pub conversation_id: String,
    pub field_id: String,
    pub turn_id: String,
    pub role: CopilotTurnRole,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotFieldContext {
    pub conversation_id: String,
    pub field_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_scene: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_zone: Option<String>,
    pub last_evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotContextResolution {
    pub context: CopilotFieldContext,
    pub rejected_evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CopilotContextUpdateRequest {
    #[serde(default)]
    pub field_id: Option<String>,
    #[serde(default)]
    pub active_scene: Option<String>,
    #[serde(default)]
    pub active_zone: Option<String>,
    #[serde(default)]
    pub retrieved_evidence: Vec<EvidenceIndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CopilotConversationError {
    #[error("conversation_id cannot be empty")]
    EmptyConversationId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("turn_id cannot be empty")]
    EmptyTurnId,
    #[error("timestamp cannot be empty")]
    EmptyTimestamp,
    #[error(
        "turn field {turn_field_id} does not match conversation field {conversation_field_id}"
    )]
    FieldScopeMismatch {
        conversation_field_id: String,
        turn_field_id: String,
    },
    #[error("turn role {value} is invalid")]
    UnsupportedRole { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CopilotContextError {
    #[error("conversation_id cannot be empty")]
    EmptyConversationId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("context conversation {context_conversation_id} does not match conversation {conversation_id}")]
    ConversationScopeMismatch {
        conversation_id: String,
        context_conversation_id: String,
    },
    #[error(
        "context field {context_field_id} does not match requested field {requested_field_id}"
    )]
    FieldScopeMismatch {
        context_field_id: String,
        requested_field_id: String,
    },
}

pub trait LedgerEvidenceResolver {
    fn resolves_ledger_ref(&self, ledger_ref: &str) -> bool;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopilotAnswerRequest {
    pub question: String,
    pub retrieved_evidence: Vec<EvidenceIndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopilotAnswer {
    pub text: String,
    pub cited_evidence_ids: Vec<String>,
    pub confidence: f64,
    pub model_provider: String,
    pub model_id: String,
    pub model_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotAnswerClaim {
    pub text: String,
    pub cited_evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundedCopilotAnswer {
    pub text: String,
    pub claims: Vec<CopilotAnswerClaim>,
    pub cited_evidence_ids: Vec<String>,
    pub confidence: f64,
    pub model_provider: String,
    pub model_id: String,
    pub model_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundedCopilotQuestionRequest {
    pub question: String,
    pub retrieved_evidence: Vec<EvidenceIndexEntry>,
    pub claims: Vec<CopilotAnswerClaim>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopilotExplanationRequest {
    pub question: String,
    pub field_id: String,
    pub zone_ref: String,
    pub retrieved_evidence: Vec<EvidenceIndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopilotExplanation {
    pub answer: GroundedCopilotAnswer,
    pub no_comparable_history: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopilotRefusalReason {
    NoEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotRefusal {
    pub refused: bool,
    pub reason: CopilotRefusalReason,
    pub needed_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundedCopilotTurn {
    pub refused: bool,
    pub refusal: Option<CopilotRefusal>,
    pub answer: Option<GroundedCopilotAnswer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopilotTurnAuditRequest {
    pub conversation_id: String,
    pub turn_id: String,
    pub field_id: String,
    pub question: String,
    pub turn: GroundedCopilotTurn,
    pub interface_version: String,
    pub ts: String,
    pub ledger_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopilotTurnAuditRecord {
    pub audit_id: String,
    pub conversation_id: String,
    pub turn_id: String,
    pub field_id: String,
    pub question: String,
    pub refused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<CopilotRefusalReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    pub cited_evidence_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    pub interface_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
    pub ts: String,
    pub ledger_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopilotAuditedTurn {
    pub turn: GroundedCopilotTurn,
    pub audit: CopilotTurnAuditRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFreshnessStatus {
    Fresh,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFreshnessRecord {
    pub evidence_id: String,
    pub status: EvidenceFreshnessStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopilotConfidenceLevel {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UncertaintyReasonCode {
    FullyCitedFreshEvidence,
    PartialEvidenceCoverage,
    StaleEvidence,
    MissingFreshness,
    ModelConfidenceLow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopilotUncertaintyMarker {
    pub level: CopilotConfidenceLevel,
    pub coverage: f64,
    pub confidence: f64,
    pub reason_codes: Vec<UncertaintyReasonCode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UncertaintyAnnotatedAnswer {
    pub answer: GroundedCopilotAnswer,
    pub uncertainty: CopilotUncertaintyMarker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopilotRecommendationDraftStatus {
    Draft,
    Approved,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopilotRecommendationDraftRequest {
    pub draft_id: String,
    pub org_id: String,
    pub field_id: String,
    pub scene_id: String,
    pub author_user_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub action_category: String,
    pub priority: RecommendationPriority,
    pub zone_ref: String,
    pub answer: GroundedCopilotAnswer,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopilotRecommendationDraft {
    pub draft_id: String,
    pub org_id: String,
    pub field_id: String,
    pub scene_id: String,
    pub author_user_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub action_category: String,
    pub priority: RecommendationPriority,
    pub zone_ref: String,
    pub cited_evidence_ids: Vec<String>,
    pub answer: GroundedCopilotAnswer,
    pub status: CopilotRecommendationDraftStatus,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotRecommendationApproval {
    pub recommendation_id: String,
    pub reviewer_user_id: String,
    pub reviewed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovedCopilotRecommendation {
    pub draft: CopilotRecommendationDraft,
    pub recommendation: RecommendationRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CopilotRecommendationDraftError {
    #[error("draft_id cannot be empty")]
    EmptyDraftId,
    #[error("org_id cannot be empty")]
    EmptyOrgId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("scene_id cannot be empty")]
    EmptySceneId,
    #[error("author_user_id cannot be empty")]
    EmptyAuthorUserId,
    #[error("title cannot be empty")]
    EmptyTitle,
    #[error("action_category cannot be empty")]
    EmptyActionCategory,
    #[error("zone_ref cannot be empty")]
    EmptyZoneRef,
    #[error("timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("reviewer_user_id cannot be empty")]
    EmptyReviewerUserId,
    #[error("recommendation_id cannot be empty")]
    EmptyRecommendationId,
    #[error("recommendation draft requires at least one cited evidence id")]
    NoCitedEvidence,
    #[error("recommendation draft has already been approved: {draft_id}")]
    DraftAlreadyApproved { draft_id: String },
    #[error(transparent)]
    RecommendationPersistence(#[from] RecommendationPersistenceError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeterministicAnswerFixture {
    pub question: String,
    pub text: String,
    pub cited_evidence_ids: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeterministicCopilotModel {
    model_provider: String,
    model_id: String,
    model_version: String,
    fixtures: BTreeMap<String, DeterministicAnswerFixture>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnavailableCopilotModel {
    adapter_name: String,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CopilotModelError {
    #[error("question cannot be empty")]
    EmptyQuestion,
    #[error("answer text cannot be empty")]
    EmptyAnswerText,
    #[error("model_provider cannot be empty")]
    EmptyModelProvider,
    #[error("model_id cannot be empty")]
    EmptyModelId,
    #[error("model_version cannot be empty")]
    EmptyModelVersion,
    #[error("cited_evidence_ids cannot contain empty values")]
    EmptyCitation,
    #[error("confidence must be finite and between 0 and 1")]
    InvalidConfidence,
    #[error("no deterministic answer fixture matched question {question}")]
    FixtureNotFound { question: String },
    #[error("cited evidence {evidence_id} was not in retrieved evidence")]
    CitationNotRetrieved { evidence_id: String },
    #[error("copilot model adapter unavailable: {reason}")]
    AdapterUnavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CopilotGroundingError {
    #[error("answer text cannot be empty")]
    EmptyAnswerText,
    #[error("grounded answer must contain at least one claim")]
    NoClaims,
    #[error("claim text cannot be empty")]
    EmptyClaimText,
    #[error("claim citation cannot be empty")]
    EmptyCitation { claim: String },
    #[error("claim has no cited evidence: {claim}")]
    UncitedClaim { claim: String },
    #[error("answer-level citation {evidence_id} was not in retrieved evidence")]
    AnswerCitationNotRetrieved { evidence_id: String },
    #[error("claim citation {evidence_id} was not in retrieved evidence for claim {claim}")]
    CitationNotRetrieved { claim: String, evidence_id: String },
    #[error("confidence must be finite and between 0 and 1")]
    InvalidConfidence,
    #[error("model_provider cannot be empty")]
    EmptyModelProvider,
    #[error("model_id cannot be empty")]
    EmptyModelId,
    #[error("model_version cannot be empty")]
    EmptyModelVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CopilotExplanationError {
    #[error("question cannot be empty")]
    EmptyQuestion,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("zone_ref cannot be empty")]
    EmptyZoneRef,
    #[error("explanation requires finding evidence")]
    MissingFindingEvidence,
    #[error("explanation requires zone evidence")]
    MissingZoneEvidence,
    #[error("explanation requires domain 28 change/trend evidence")]
    MissingChangeEvidence,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CopilotTurnError {
    #[error("question cannot be empty")]
    EmptyQuestion,
    #[error(transparent)]
    Model(#[from] CopilotModelError),
    #[error(transparent)]
    Grounding(#[from] CopilotGroundingError),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CopilotAuditError {
    #[error("conversation_id cannot be empty")]
    EmptyConversationId,
    #[error("turn_id cannot be empty")]
    EmptyTurnId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("question cannot be empty")]
    EmptyQuestion,
    #[error("interface_version cannot be empty")]
    EmptyInterfaceVersion,
    #[error("audit timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("ledger_ref cannot be empty")]
    EmptyLedgerRef,
    #[error("completed answer must cite at least one evidence id before audit")]
    AnswerWithoutCitations,
    #[error("copilot audit write failed: {reason}")]
    AuditWriteFailed { reason: String },
}

pub trait CopilotModel {
    fn answer(&self, request: CopilotAnswerRequest) -> Result<CopilotAnswer, CopilotModelError>;
}

pub trait CopilotAuditSink {
    fn write_turn_audit(&mut self, record: &CopilotTurnAuditRecord) -> Result<(), String>;
}

impl DeterministicCopilotModel {
    pub fn new(
        model_provider: String,
        model_id: String,
        model_version: String,
        fixtures: Vec<DeterministicAnswerFixture>,
    ) -> Result<Self, CopilotModelError> {
        let model_provider =
            normalize_model_text(model_provider, CopilotModelError::EmptyModelProvider)?;
        let model_id = normalize_model_text(model_id, CopilotModelError::EmptyModelId)?;
        let model_version =
            normalize_model_text(model_version, CopilotModelError::EmptyModelVersion)?;
        let mut normalized_fixtures = BTreeMap::new();

        for fixture in fixtures {
            let normalized = normalize_fixture(fixture)?;
            normalized_fixtures.insert(normalized.question.clone(), normalized);
        }

        Ok(Self {
            model_provider,
            model_id,
            model_version,
            fixtures: normalized_fixtures,
        })
    }
}

impl CopilotModel for DeterministicCopilotModel {
    fn answer(&self, request: CopilotAnswerRequest) -> Result<CopilotAnswer, CopilotModelError> {
        let question = normalize_model_text(request.question, CopilotModelError::EmptyQuestion)?;
        let fixture =
            self.fixtures
                .get(&question)
                .ok_or_else(|| CopilotModelError::FixtureNotFound {
                    question: question.clone(),
                })?;
        let retrieved_ids = request
            .retrieved_evidence
            .into_iter()
            .filter_map(|entry| normalize_text(entry.evidence_id))
            .collect::<BTreeSet<_>>();

        for evidence_id in &fixture.cited_evidence_ids {
            if !retrieved_ids.contains(evidence_id) {
                return Err(CopilotModelError::CitationNotRetrieved {
                    evidence_id: evidence_id.clone(),
                });
            }
        }

        Ok(CopilotAnswer {
            text: fixture.text.clone(),
            cited_evidence_ids: fixture.cited_evidence_ids.clone(),
            confidence: fixture.confidence,
            model_provider: self.model_provider.clone(),
            model_id: self.model_id.clone(),
            model_version: self.model_version.clone(),
        })
    }
}

impl UnavailableCopilotModel {
    pub fn new(adapter_name: String, reason: String) -> Self {
        Self {
            adapter_name: normalize_text(adapter_name).unwrap_or_else(|| "unknown".to_string()),
            reason: normalize_text(reason).unwrap_or_else(|| "unavailable".to_string()),
        }
    }
}

impl CopilotModel for UnavailableCopilotModel {
    fn answer(&self, _request: CopilotAnswerRequest) -> Result<CopilotAnswer, CopilotModelError> {
        Err(CopilotModelError::AdapterUnavailable {
            reason: format!("{}: {}", self.adapter_name, self.reason),
        })
    }
}

pub fn answer_grounded_question(
    model: &impl CopilotModel,
    request: GroundedCopilotQuestionRequest,
) -> Result<GroundedCopilotTurn, CopilotTurnError> {
    let question = normalize_text(request.question).ok_or(CopilotTurnError::EmptyQuestion)?;
    let retrieved_evidence = relevant_evidence_for_question(&question, request.retrieved_evidence);
    if retrieved_evidence.is_empty() {
        return Ok(no_evidence_refusal());
    }

    let answer = model.answer(CopilotAnswerRequest {
        question,
        retrieved_evidence: retrieved_evidence.clone(),
    })?;
    let grounded = post_check_grounded_answer(answer, request.claims, &retrieved_evidence)?;

    Ok(GroundedCopilotTurn {
        refused: false,
        refusal: None,
        answer: Some(grounded),
    })
}

pub fn explain_zone_finding_change(
    request: CopilotExplanationRequest,
) -> Result<CopilotExplanation, CopilotExplanationError> {
    let question =
        normalize_text(request.question).ok_or(CopilotExplanationError::EmptyQuestion)?;
    let field_id = normalize_text(request.field_id).ok_or(CopilotExplanationError::EmptyFieldId)?;
    let zone_ref = normalize_text(request.zone_ref).ok_or(CopilotExplanationError::EmptyZoneRef)?;
    let mut scoped = request
        .retrieved_evidence
        .into_iter()
        .filter(|entry| {
            normalize_text(entry.evidence_id.clone()).is_some()
                && normalize_text(entry.ledger_ref.clone()).is_some()
                && normalize_text(entry.field_id.clone()).as_deref() == Some(field_id.as_str())
        })
        .collect::<Vec<_>>();
    scoped.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));

    let finding = scoped
        .iter()
        .find(|entry| entry.kind == EvidenceKind::Finding)
        .ok_or(CopilotExplanationError::MissingFindingEvidence)?;
    let zone = scoped
        .iter()
        .find(|entry| {
            entry.zone_ref.as_deref() == Some(zone_ref.as_str())
                && entry.evidence_id != finding.evidence_id
                && entry.kind != EvidenceKind::Trend
        })
        .ok_or(CopilotExplanationError::MissingZoneEvidence)?;
    let change = scoped
        .iter()
        .find(|entry| entry.kind == EvidenceKind::Trend)
        .ok_or(CopilotExplanationError::MissingChangeEvidence)?;
    let no_comparable_history = is_no_baseline_change(change);
    let cited_evidence_ids = BTreeSet::from([
        finding.evidence_id.clone(),
        zone.evidence_id.clone(),
        change.evidence_id.clone(),
    ])
    .into_iter()
    .collect::<Vec<_>>();

    let change_text = if no_comparable_history {
        "Domain 28 reports no comparable baseline/history for this change request; no change magnitude is inferred."
            .to_string()
    } else {
        format!("Domain 28 change context: {}", change.summary)
    };
    let text = format!(
        "{} Finding: {} Zone context: {} {}",
        question, finding.summary, zone.summary, change_text
    );
    let claims = vec![
        CopilotAnswerClaim {
            text: format!("Finding evidence: {}", finding.summary),
            cited_evidence_ids: vec![finding.evidence_id.clone()],
        },
        CopilotAnswerClaim {
            text: format!("Zone evidence: {}", zone.summary),
            cited_evidence_ids: vec![zone.evidence_id.clone()],
        },
        CopilotAnswerClaim {
            text: change_text,
            cited_evidence_ids: vec![change.evidence_id.clone()],
        },
    ];

    Ok(CopilotExplanation {
        answer: GroundedCopilotAnswer {
            text,
            claims,
            cited_evidence_ids,
            confidence: if no_comparable_history { 0.74 } else { 0.86 },
            model_provider: "deterministic".to_string(),
            model_id: "copilot-explain-zone-finding-change".to_string(),
            model_version: "v1".to_string(),
        },
        no_comparable_history,
    })
}

pub fn finalize_audited_turn(
    sink: &mut impl CopilotAuditSink,
    request: CopilotTurnAuditRequest,
) -> Result<CopilotAuditedTurn, CopilotAuditError> {
    let conversation_id = normalize_audit_text(
        request.conversation_id,
        CopilotAuditError::EmptyConversationId,
    )?;
    let turn_id = normalize_audit_text(request.turn_id, CopilotAuditError::EmptyTurnId)?;
    let field_id = normalize_audit_text(request.field_id, CopilotAuditError::EmptyFieldId)?;
    let question = normalize_audit_text(request.question, CopilotAuditError::EmptyQuestion)?;
    let interface_version = normalize_audit_text(
        request.interface_version,
        CopilotAuditError::EmptyInterfaceVersion,
    )?;
    let ts = normalize_audit_text(request.ts, CopilotAuditError::EmptyTimestamp)?;
    let ledger_ref = normalize_audit_text(request.ledger_ref, CopilotAuditError::EmptyLedgerRef)?;

    let (answer, cited_evidence_ids, confidence, model_provider, model_id, model_version) =
        if request.turn.refused {
            (None, Vec::new(), None, None, None, None)
        } else {
            let answer = request
                .turn
                .answer
                .as_ref()
                .ok_or(CopilotAuditError::AnswerWithoutCitations)?;
            let cited_evidence_ids = normalize_audit_citations(answer.cited_evidence_ids.clone())?;
            (
                Some(answer.text.clone()),
                cited_evidence_ids,
                Some(answer.confidence),
                Some(answer.model_provider.clone()),
                Some(answer.model_id.clone()),
                Some(answer.model_version.clone()),
            )
        };
    let refusal_reason = request.turn.refusal.as_ref().map(|refusal| refusal.reason);

    let audit = CopilotTurnAuditRecord {
        audit_id: format!("copilot-audit:{conversation_id}:{turn_id}:{ts}"),
        conversation_id,
        turn_id,
        field_id,
        question,
        refused: request.turn.refused,
        refusal_reason,
        answer,
        cited_evidence_ids,
        confidence,
        interface_version,
        model_provider,
        model_id,
        model_version,
        ts,
        ledger_ref,
    };
    sink.write_turn_audit(&audit)
        .map_err(|reason| CopilotAuditError::AuditWriteFailed { reason })?;

    Ok(CopilotAuditedTurn {
        turn: request.turn,
        audit,
    })
}

pub fn annotate_answer_uncertainty(
    answer: GroundedCopilotAnswer,
    freshness_records: Vec<EvidenceFreshnessRecord>,
) -> UncertaintyAnnotatedAnswer {
    let claim_count = answer.claims.len();
    let cited_claim_count = answer
        .claims
        .iter()
        .filter(|claim| !claim.cited_evidence_ids.is_empty())
        .count();
    let coverage = if claim_count == 0 {
        0.0
    } else {
        cited_claim_count as f64 / claim_count as f64
    };
    let freshness_by_evidence = freshness_records
        .into_iter()
        .filter_map(|record| normalize_text(record.evidence_id).map(|id| (id, record.status)))
        .collect::<BTreeMap<_, _>>();
    let mut reason_codes = BTreeSet::new();
    let mut has_stale = false;
    let mut has_missing_freshness = false;

    if coverage < 1.0 {
        reason_codes.insert(UncertaintyReasonCode::PartialEvidenceCoverage);
    }

    for evidence_id in &answer.cited_evidence_ids {
        match freshness_by_evidence.get(evidence_id) {
            Some(EvidenceFreshnessStatus::Fresh) => {}
            Some(EvidenceFreshnessStatus::Stale) => {
                has_stale = true;
                reason_codes.insert(UncertaintyReasonCode::StaleEvidence);
            }
            Some(EvidenceFreshnessStatus::Unknown) | None => {
                has_missing_freshness = true;
                reason_codes.insert(UncertaintyReasonCode::MissingFreshness);
            }
        }
    }

    if answer.confidence < 0.75 {
        reason_codes.insert(UncertaintyReasonCode::ModelConfidenceLow);
    }

    let level = if coverage < 1.0 || has_stale || has_missing_freshness || answer.confidence < 0.5 {
        CopilotConfidenceLevel::Low
    } else if answer.confidence < 0.75 {
        CopilotConfidenceLevel::Medium
    } else {
        reason_codes.insert(UncertaintyReasonCode::FullyCitedFreshEvidence);
        CopilotConfidenceLevel::High
    };

    UncertaintyAnnotatedAnswer {
        uncertainty: CopilotUncertaintyMarker {
            level,
            coverage,
            confidence: answer.confidence,
            reason_codes: reason_codes.into_iter().collect(),
        },
        answer,
    }
}

pub fn draft_recommendation_from_answer(
    request: CopilotRecommendationDraftRequest,
) -> Result<CopilotRecommendationDraft, CopilotRecommendationDraftError> {
    let draft_id = normalize_draft_text(
        request.draft_id,
        CopilotRecommendationDraftError::EmptyDraftId,
    )?;
    let org_id = normalize_draft_text(request.org_id, CopilotRecommendationDraftError::EmptyOrgId)?;
    let field_id = normalize_draft_text(
        request.field_id,
        CopilotRecommendationDraftError::EmptyFieldId,
    )?;
    let scene_id = normalize_draft_text(
        request.scene_id,
        CopilotRecommendationDraftError::EmptySceneId,
    )?;
    let author_user_id = normalize_draft_text(
        request.author_user_id,
        CopilotRecommendationDraftError::EmptyAuthorUserId,
    )?;
    let title = normalize_draft_text(request.title, CopilotRecommendationDraftError::EmptyTitle)?;
    let action_category = normalize_draft_text(
        request.action_category,
        CopilotRecommendationDraftError::EmptyActionCategory,
    )?;
    let zone_ref = normalize_draft_text(
        request.zone_ref,
        CopilotRecommendationDraftError::EmptyZoneRef,
    )?;
    let created_at = normalize_draft_text(
        request.created_at,
        CopilotRecommendationDraftError::EmptyTimestamp,
    )?;
    let cited_evidence_ids = normalize_draft_citations(request.answer.cited_evidence_ids.clone())?;

    Ok(CopilotRecommendationDraft {
        draft_id,
        org_id,
        field_id,
        scene_id,
        author_user_id,
        title,
        note: normalize_optional_text(request.note),
        action_category,
        priority: request.priority,
        zone_ref,
        cited_evidence_ids,
        answer: request.answer,
        status: CopilotRecommendationDraftStatus::Draft,
        created_at: created_at.clone(),
        updated_at: created_at,
        reviewed_by: None,
        reviewed_at: None,
    })
}

pub fn approve_recommendation_draft(
    registry: &mut RecommendationLifecycleRegistry,
    draft: CopilotRecommendationDraft,
    approval: CopilotRecommendationApproval,
) -> Result<ApprovedCopilotRecommendation, CopilotRecommendationDraftError> {
    if draft.status != CopilotRecommendationDraftStatus::Draft {
        return Err(CopilotRecommendationDraftError::DraftAlreadyApproved {
            draft_id: draft.draft_id,
        });
    }

    let recommendation_id = normalize_draft_text(
        approval.recommendation_id,
        CopilotRecommendationDraftError::EmptyRecommendationId,
    )?;
    let reviewer_user_id = normalize_draft_text(
        approval.reviewer_user_id,
        CopilotRecommendationDraftError::EmptyReviewerUserId,
    )?;
    let reviewed_at = normalize_draft_text(
        approval.reviewed_at,
        CopilotRecommendationDraftError::EmptyTimestamp,
    )?;

    let recommendation = registry.create_recommendation(RecommendationRecord {
        recommendation_id,
        scene_id: draft.scene_id.clone(),
        field_id: Some(draft.field_id.clone()),
        org_id: draft.org_id.clone(),
        author_user_id: reviewer_user_id.clone(),
        title: draft.title.clone(),
        note: draft.note.clone(),
        category: Some(draft.action_category.clone()),
        action_category: draft.action_category.clone(),
        priority: draft.priority,
        status: RecommendationStatus::Open,
        evidence_refs: draft.cited_evidence_ids.clone(),
        annotation_ids: Vec::new(),
        created_at: reviewed_at.clone(),
        updated_at: reviewed_at.clone(),
    })?;

    let mut approved_draft = draft;
    approved_draft.status = CopilotRecommendationDraftStatus::Approved;
    approved_draft.updated_at = reviewed_at.clone();
    approved_draft.reviewed_by = Some(reviewer_user_id);
    approved_draft.reviewed_at = Some(reviewed_at);

    Ok(ApprovedCopilotRecommendation {
        draft: approved_draft,
        recommendation,
    })
}

pub fn start_copilot_conversation(
    request: CopilotConversationStartRequest,
    generated_conversation_id: String,
    created_at: String,
) -> Result<CopilotConversationRecord, CopilotConversationError> {
    let conversation_id = normalize_optional_conversation_text(request.conversation_id)
        .or_else(|| normalize_conversation_text(generated_conversation_id))
        .ok_or(CopilotConversationError::EmptyConversationId)?;
    let field_id = normalize_conversation_text(request.field_id)
        .ok_or(CopilotConversationError::EmptyFieldId)?;
    let created_at =
        normalize_conversation_text(created_at).ok_or(CopilotConversationError::EmptyTimestamp)?;

    Ok(CopilotConversationRecord {
        conversation_id,
        field_id,
        created_at,
    })
}

pub fn create_copilot_turn(
    conversation: &CopilotConversationRecord,
    request: CopilotTurnCreateRequest,
    generated_turn_id: String,
    created_at: String,
) -> Result<CopilotTurnRecord, CopilotConversationError> {
    let turn_id = normalize_optional_conversation_text(request.turn_id)
        .or_else(|| normalize_conversation_text(generated_turn_id))
        .ok_or(CopilotConversationError::EmptyTurnId)?;
    let turn_field_id = normalize_conversation_text(request.field_id)
        .ok_or(CopilotConversationError::EmptyFieldId)?;
    let conversation_field_id = normalize_conversation_text(conversation.field_id.clone())
        .ok_or(CopilotConversationError::EmptyFieldId)?;
    if turn_field_id != conversation_field_id {
        return Err(CopilotConversationError::FieldScopeMismatch {
            conversation_field_id,
            turn_field_id,
        });
    }
    let created_at =
        normalize_conversation_text(created_at).ok_or(CopilotConversationError::EmptyTimestamp)?;

    Ok(CopilotTurnRecord {
        conversation_id: conversation.conversation_id.clone(),
        field_id: turn_field_id,
        turn_id,
        role: request.role,
        created_at,
    })
}

pub fn resolve_copilot_field_context(
    conversation: &CopilotConversationRecord,
    previous: Option<&CopilotFieldContext>,
    request: CopilotContextUpdateRequest,
) -> Result<CopilotContextResolution, CopilotContextError> {
    let conversation_id = normalize_context_text(
        conversation.conversation_id.clone(),
        CopilotContextError::EmptyConversationId,
    )?;
    let conversation_field_id = normalize_context_text(
        conversation.field_id.clone(),
        CopilotContextError::EmptyFieldId,
    )?;

    if let Some(previous) = previous {
        let context_conversation_id = normalize_context_text(
            previous.conversation_id.clone(),
            CopilotContextError::EmptyConversationId,
        )?;
        if context_conversation_id != conversation_id {
            return Err(CopilotContextError::ConversationScopeMismatch {
                conversation_id,
                context_conversation_id,
            });
        }
        if previous.field_id != conversation_field_id {
            return Err(CopilotContextError::FieldScopeMismatch {
                context_field_id: previous.field_id.clone(),
                requested_field_id: conversation_field_id,
            });
        }
    }

    let requested_field_id = request
        .field_id
        .and_then(normalize_text)
        .unwrap_or_else(|| conversation_field_id.clone());
    if requested_field_id != conversation_field_id {
        return Err(CopilotContextError::FieldScopeMismatch {
            context_field_id: conversation_field_id,
            requested_field_id,
        });
    }

    let previous_scene = previous.and_then(|context| context.active_scene.clone());
    let previous_zone = previous.and_then(|context| context.active_zone.clone());
    let active_scene = normalize_optional_text(request.active_scene).or(previous_scene);
    let active_zone = normalize_optional_text(request.active_zone).or(previous_zone);
    let mut rejected_evidence_ids = Vec::new();
    let mut last_evidence_ids = request
        .retrieved_evidence
        .into_iter()
        .filter_map(|entry| {
            let evidence_id = normalize_text(entry.evidence_id)?;
            let evidence_field = normalize_text(entry.field_id)?;
            if evidence_field == conversation_field_id {
                Some(evidence_id)
            } else {
                rejected_evidence_ids.push(evidence_id);
                None
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if last_evidence_ids.is_empty() {
        last_evidence_ids = previous
            .map(|context| context.last_evidence_ids.clone())
            .unwrap_or_default();
    }
    rejected_evidence_ids.sort();
    rejected_evidence_ids.dedup();

    Ok(CopilotContextResolution {
        context: CopilotFieldContext {
            conversation_id,
            field_id: conversation_field_id,
            active_scene,
            active_zone,
            last_evidence_ids,
        },
        rejected_evidence_ids,
    })
}

pub fn post_check_grounded_answer(
    answer: CopilotAnswer,
    claims: Vec<CopilotAnswerClaim>,
    retrieved_evidence: &[EvidenceIndexEntry],
) -> Result<GroundedCopilotAnswer, CopilotGroundingError> {
    let text = normalize_grounding_text(answer.text, CopilotGroundingError::EmptyAnswerText)?;
    if !answer.confidence.is_finite() || !(0.0..=1.0).contains(&answer.confidence) {
        return Err(CopilotGroundingError::InvalidConfidence);
    }
    let model_provider = normalize_grounding_text(
        answer.model_provider,
        CopilotGroundingError::EmptyModelProvider,
    )?;
    let model_id = normalize_grounding_text(answer.model_id, CopilotGroundingError::EmptyModelId)?;
    let model_version = normalize_grounding_text(
        answer.model_version,
        CopilotGroundingError::EmptyModelVersion,
    )?;
    if claims.is_empty() {
        return Err(CopilotGroundingError::NoClaims);
    }

    let retrieved_ids = retrieved_evidence
        .iter()
        .filter_map(|entry| {
            let evidence_id = normalize_text(entry.evidence_id.clone())?;
            normalize_text(entry.ledger_ref.clone())?;
            Some(evidence_id)
        })
        .collect::<BTreeSet<_>>();

    for evidence_id in answer.cited_evidence_ids {
        let evidence_id = normalize_grounding_text(
            evidence_id,
            CopilotGroundingError::EmptyCitation {
                claim: "answer".to_string(),
            },
        )?;
        if !retrieved_ids.contains(&evidence_id) {
            return Err(CopilotGroundingError::AnswerCitationNotRetrieved { evidence_id });
        }
    }

    let mut normalized_claims = Vec::new();
    let mut cited_evidence_ids = BTreeSet::new();
    for claim in claims {
        let claim_text =
            normalize_grounding_text(claim.text, CopilotGroundingError::EmptyClaimText)?;
        if claim.cited_evidence_ids.is_empty() {
            return Err(CopilotGroundingError::UncitedClaim { claim: claim_text });
        }

        let mut normalized_citations = Vec::new();
        for evidence_id in claim.cited_evidence_ids {
            let evidence_id = normalize_grounding_text(
                evidence_id,
                CopilotGroundingError::EmptyCitation {
                    claim: claim_text.clone(),
                },
            )?;
            if !retrieved_ids.contains(&evidence_id) {
                return Err(CopilotGroundingError::CitationNotRetrieved {
                    claim: claim_text,
                    evidence_id,
                });
            }
            cited_evidence_ids.insert(evidence_id.clone());
            normalized_citations.push(evidence_id);
        }

        normalized_claims.push(CopilotAnswerClaim {
            text: claim_text,
            cited_evidence_ids: normalized_citations,
        });
    }

    Ok(GroundedCopilotAnswer {
        text,
        claims: normalized_claims,
        cited_evidence_ids: cited_evidence_ids.into_iter().collect(),
        confidence: answer.confidence,
        model_provider,
        model_id,
        model_version,
    })
}

fn no_evidence_refusal() -> GroundedCopilotTurn {
    GroundedCopilotTurn {
        refused: true,
        refusal: Some(CopilotRefusal {
            refused: true,
            reason: CopilotRefusalReason::NoEvidence,
            needed_evidence: vec![
                "resolvable indexed evidence relevant to the question".to_string()
            ],
        }),
        answer: None,
    }
}

fn relevant_evidence_for_question(
    question: &str,
    retrieved_evidence: Vec<EvidenceIndexEntry>,
) -> Vec<EvidenceIndexEntry> {
    let question_tokens = meaningful_tokens(question);
    if question_tokens.is_empty() {
        return Vec::new();
    }

    retrieved_evidence
        .into_iter()
        .filter(|entry| {
            normalize_text(entry.evidence_id.clone()).is_some()
                && normalize_text(entry.ledger_ref.clone()).is_some()
                && evidence_tokens(entry)
                    .iter()
                    .any(|token| question_tokens.contains(token))
        })
        .collect()
}

fn evidence_tokens(entry: &EvidenceIndexEntry) -> BTreeSet<String> {
    let mut text = format!(
        "{} {} {}",
        entry.summary,
        entry.evidence_id,
        evidence_kind_label(entry.kind)
    );
    if let Some(scene_ref) = &entry.scene_ref {
        text.push(' ');
        text.push_str(scene_ref);
    }
    if let Some(zone_ref) = &entry.zone_ref {
        text.push(' ');
        text.push_str(zone_ref);
    }
    meaningful_tokens(&text)
}

fn evidence_kind_label(kind: EvidenceKind) -> &'static str {
    match kind {
        EvidenceKind::Finding => "finding",
        EvidenceKind::ImageryProduct => "imagery product",
        EvidenceKind::LidarProduct => "lidar product",
        EvidenceKind::Report => "report",
        EvidenceKind::Trend => "trend change",
    }
}

fn is_no_baseline_change(entry: &EvidenceIndexEntry) -> bool {
    let summary = entry.summary.to_ascii_lowercase();
    summary.contains("no baseline")
        || summary.contains("no comparable history")
        || summary.contains("insufficient baseline")
}

fn meaningful_tokens(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter_map(|token| {
            let token = token.trim().to_ascii_lowercase();
            if token.len() >= 3 && !is_copilot_stopword(&token) {
                Some(token)
            } else {
                None
            }
        })
        .collect()
}

fn is_copilot_stopword(token: &str) -> bool {
    matches!(
        token,
        "the"
            | "and"
            | "for"
            | "with"
            | "why"
            | "what"
            | "when"
            | "where"
            | "how"
            | "this"
            | "that"
            | "are"
            | "was"
            | "were"
            | "does"
            | "did"
            | "field"
            | "zone"
            | "crop"
            | "flight"
            | "last"
            | "since"
    )
}

pub fn build_evidence_retrieval_index(
    field_id: String,
    candidates: Vec<EvidenceCandidate>,
    resolver: &impl LedgerEvidenceResolver,
) -> Result<EvidenceRetrievalIndex, CopilotIndexError> {
    let field_id = normalize_required_text(field_id, CopilotIndexError::EmptyFieldId)?;
    let mut entries = Vec::new();
    let mut rejected_items = Vec::new();
    let mut seen_evidence_ids = BTreeSet::new();

    for candidate in candidates {
        let evidence_id = normalize_text(candidate.evidence_id);
        let ledger_ref = normalize_text(candidate.ledger_ref);

        let Some(evidence_id) = evidence_id else {
            rejected_items.push(rejected_item(
                None,
                ledger_ref,
                EvidenceRejectionReason::EmptyEvidenceId,
            ));
            continue;
        };

        if !seen_evidence_ids.insert(evidence_id.clone()) {
            rejected_items.push(rejected_item(
                Some(evidence_id),
                ledger_ref,
                EvidenceRejectionReason::DuplicateEvidenceId,
            ));
            continue;
        }

        let candidate_field_id = normalize_text(candidate.field_id);
        if candidate_field_id.as_deref() != Some(field_id.as_str()) {
            rejected_items.push(rejected_item(
                Some(evidence_id),
                ledger_ref,
                EvidenceRejectionReason::FieldMismatch,
            ));
            continue;
        }

        let Some(ledger_ref) = ledger_ref else {
            rejected_items.push(rejected_item(
                Some(evidence_id),
                None,
                EvidenceRejectionReason::EmptyLedgerRef,
            ));
            continue;
        };

        let Some(summary) = normalize_text(candidate.summary) else {
            rejected_items.push(rejected_item(
                Some(evidence_id),
                Some(ledger_ref),
                EvidenceRejectionReason::EmptySummary,
            ));
            continue;
        };

        if !resolver.resolves_ledger_ref(&ledger_ref) {
            rejected_items.push(rejected_item(
                Some(evidence_id),
                Some(ledger_ref),
                EvidenceRejectionReason::UnresolvedLedgerRef,
            ));
            continue;
        }

        entries.push(EvidenceIndexEntry {
            evidence_id,
            kind: candidate.kind,
            field_id: field_id.clone(),
            scene_ref: normalize_optional_text(candidate.scene_ref),
            zone_ref: normalize_optional_text(candidate.zone_ref),
            ledger_ref,
            summary,
        });
    }

    entries.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    rejected_items.sort_by(|left, right| {
        left.evidence_id
            .cmp(&right.evidence_id)
            .then(left.ledger_ref.cmp(&right.ledger_ref))
            .then((left.reason as u8).cmp(&(right.reason as u8)))
    });

    Ok(EvidenceRetrievalIndex {
        field_id,
        entries,
        rejected_items,
    })
}

fn rejected_item(
    evidence_id: Option<String>,
    ledger_ref: Option<String>,
    reason: EvidenceRejectionReason,
) -> RejectedEvidenceItem {
    RejectedEvidenceItem {
        evidence_id,
        ledger_ref,
        reason,
    }
}

fn normalize_required_text(
    value: String,
    error: CopilotIndexError,
) -> Result<String, CopilotIndexError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_model_text(
    value: String,
    error: CopilotModelError,
) -> Result<String, CopilotModelError> {
    normalize_text(value).ok_or(error)
}

fn normalize_grounding_text(
    value: String,
    error: CopilotGroundingError,
) -> Result<String, CopilotGroundingError> {
    normalize_text(value).ok_or(error)
}

fn normalize_audit_text(
    value: String,
    error: CopilotAuditError,
) -> Result<String, CopilotAuditError> {
    normalize_text(value).ok_or(error)
}

fn normalize_audit_citations(values: Vec<String>) -> Result<Vec<String>, CopilotAuditError> {
    let cited_evidence_ids = values
        .into_iter()
        .filter_map(normalize_text)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if cited_evidence_ids.is_empty() {
        Err(CopilotAuditError::AnswerWithoutCitations)
    } else {
        Ok(cited_evidence_ids)
    }
}

fn normalize_draft_text(
    value: String,
    error: CopilotRecommendationDraftError,
) -> Result<String, CopilotRecommendationDraftError> {
    normalize_text(value).ok_or(error)
}

fn normalize_conversation_text(value: String) -> Option<String> {
    normalize_text(value)
}

fn normalize_context_text(
    value: String,
    error: CopilotContextError,
) -> Result<String, CopilotContextError> {
    normalize_text(value).ok_or(error)
}

fn normalize_optional_conversation_text(value: Option<String>) -> Option<String> {
    value.and_then(normalize_conversation_text)
}

fn normalize_draft_citations(
    values: Vec<String>,
) -> Result<Vec<String>, CopilotRecommendationDraftError> {
    let cited_evidence_ids = values
        .into_iter()
        .filter_map(normalize_text)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if cited_evidence_ids.is_empty() {
        Err(CopilotRecommendationDraftError::NoCitedEvidence)
    } else {
        Ok(cited_evidence_ids)
    }
}

fn normalize_fixture(
    fixture: DeterministicAnswerFixture,
) -> Result<DeterministicAnswerFixture, CopilotModelError> {
    let question = normalize_model_text(fixture.question, CopilotModelError::EmptyQuestion)?;
    let text = normalize_model_text(fixture.text, CopilotModelError::EmptyAnswerText)?;
    validate_confidence(fixture.confidence)?;
    let cited_evidence_ids = fixture
        .cited_evidence_ids
        .into_iter()
        .map(|evidence_id| normalize_model_text(evidence_id, CopilotModelError::EmptyCitation))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DeterministicAnswerFixture {
        question,
        text,
        cited_evidence_ids,
        confidence: fixture.confidence,
    })
}

fn validate_confidence(confidence: f64) -> Result<(), CopilotModelError> {
    if confidence.is_finite() && (0.0..=1.0).contains(&confidence) {
        Ok(())
    } else {
        Err(CopilotModelError::InvalidConfidence)
    }
}

fn normalize_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(normalize_text)
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, collections::BTreeSet};

    use super::{
        annotate_answer_uncertainty, answer_grounded_question, approve_recommendation_draft,
        build_evidence_retrieval_index, create_copilot_turn, draft_recommendation_from_answer,
        explain_zone_finding_change, finalize_audited_turn, post_check_grounded_answer,
        resolve_copilot_field_context, start_copilot_conversation, CopilotAnswer,
        CopilotAnswerClaim, CopilotAnswerRequest, CopilotAuditError, CopilotAuditSink,
        CopilotConfidenceLevel, CopilotContextError, CopilotContextUpdateRequest,
        CopilotConversationError, CopilotConversationStartRequest, CopilotExplanationError,
        CopilotExplanationRequest, CopilotFieldContext, CopilotGroundingError, CopilotIndexError,
        CopilotModel, CopilotModelError, CopilotRecommendationApproval,
        CopilotRecommendationDraftError, CopilotRecommendationDraftRequest,
        CopilotRecommendationDraftStatus, CopilotRefusalReason, CopilotTurnAuditRecord,
        CopilotTurnAuditRequest, CopilotTurnCreateRequest, CopilotTurnRole,
        DeterministicAnswerFixture, DeterministicCopilotModel, EvidenceCandidate,
        EvidenceFreshnessRecord, EvidenceFreshnessStatus, EvidenceIndexEntry, EvidenceKind,
        EvidenceRejectionReason, GroundedCopilotAnswer, GroundedCopilotQuestionRequest,
        LedgerEvidenceResolver, UnavailableCopilotModel, UncertaintyReasonCode,
    };
    use shared::schemas::{RecommendationLifecycleRegistry, RecommendationPriority};

    struct FixtureLedger {
        refs: BTreeSet<String>,
    }

    impl LedgerEvidenceResolver for FixtureLedger {
        fn resolves_ledger_ref(&self, ledger_ref: &str) -> bool {
            self.refs.contains(ledger_ref)
        }
    }

    #[test]
    fn evidence_index_requires_resolvable_ledger_refs() {
        let ledger = FixtureLedger {
            refs: BTreeSet::from(["ledger:30:ndvi:001".to_string()]),
        };
        let index = build_evidence_retrieval_index(
            " field-001 ".to_string(),
            vec![
                EvidenceCandidate {
                    evidence_id: "evidence-ndvi-001".to_string(),
                    kind: EvidenceKind::ImageryProduct,
                    field_id: "field-001".to_string(),
                    scene_ref: Some("scene-2026-06-01".to_string()),
                    zone_ref: Some("zone-ne".to_string()),
                    ledger_ref: "ledger:30:ndvi:001".to_string(),
                    summary: "NDVI in the northeast zone dropped below 0.42.".to_string(),
                },
                EvidenceCandidate {
                    evidence_id: "evidence-trend-missing-ledger".to_string(),
                    kind: EvidenceKind::Trend,
                    field_id: "field-001".to_string(),
                    scene_ref: None,
                    zone_ref: Some("zone-ne".to_string()),
                    ledger_ref: "ledger:30:trend:missing".to_string(),
                    summary: "Unresolved trend should not be citable.".to_string(),
                },
            ],
            &ledger,
        )
        .expect("index should build");

        assert_eq!(index.field_id, "field-001");
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].evidence_id, "evidence-ndvi-001");
        assert_eq!(index.entries[0].ledger_ref, "ledger:30:ndvi:001");
        assert_eq!(index.rejected_items.len(), 1);
        assert_eq!(
            index.rejected_items[0].evidence_id.as_deref(),
            Some("evidence-trend-missing-ledger")
        );
        assert_eq!(
            index.rejected_items[0].reason,
            EvidenceRejectionReason::UnresolvedLedgerRef
        );
    }

    #[test]
    fn evidence_index_is_field_scoped_and_deterministically_ordered() {
        let ledger = FixtureLedger {
            refs: BTreeSet::from([
                "ledger:30:report:002".to_string(),
                "ledger:30:finding:001".to_string(),
            ]),
        };
        let index = build_evidence_retrieval_index(
            "field-001".to_string(),
            vec![
                EvidenceCandidate {
                    evidence_id: "evidence-report-002".to_string(),
                    kind: EvidenceKind::Report,
                    field_id: "field-001".to_string(),
                    scene_ref: None,
                    zone_ref: None,
                    ledger_ref: "ledger:30:report:002".to_string(),
                    summary: "Advisor report summarized northeast-zone stress.".to_string(),
                },
                EvidenceCandidate {
                    evidence_id: "evidence-other-field".to_string(),
                    kind: EvidenceKind::Finding,
                    field_id: "field-002".to_string(),
                    scene_ref: None,
                    zone_ref: None,
                    ledger_ref: "ledger:30:finding:001".to_string(),
                    summary: "Other-field finding must not enter this field index.".to_string(),
                },
                EvidenceCandidate {
                    evidence_id: "evidence-finding-001".to_string(),
                    kind: EvidenceKind::Finding,
                    field_id: "field-001".to_string(),
                    scene_ref: Some("scene-2026-06-01".to_string()),
                    zone_ref: Some("zone-ne".to_string()),
                    ledger_ref: "ledger:30:finding:001".to_string(),
                    summary: "Finding cites the same stressed northeast zone.".to_string(),
                },
            ],
            &ledger,
        )
        .expect("index should build");

        let evidence_ids = index
            .entries
            .iter()
            .map(|entry| entry.evidence_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            evidence_ids,
            vec!["evidence-finding-001", "evidence-report-002"]
        );
        assert_eq!(index.rejected_items.len(), 1);
        assert_eq!(
            index.rejected_items[0].reason,
            EvidenceRejectionReason::FieldMismatch
        );
    }

    #[test]
    fn evidence_index_rejects_empty_field_scope() {
        let ledger = FixtureLedger {
            refs: BTreeSet::new(),
        };
        let error = build_evidence_retrieval_index(" ".to_string(), Vec::new(), &ledger)
            .expect_err("empty field scope should be rejected");

        assert_eq!(error, CopilotIndexError::EmptyFieldId);
    }

    #[test]
    fn conversation_and_turn_identity_are_field_scoped() {
        let conversation = start_copilot_conversation(
            CopilotConversationStartRequest {
                conversation_id: Some(" conversation-001 ".to_string()),
                field_id: " field-001 ".to_string(),
            },
            "generated-conversation".to_string(),
            " 2026-06-13T16:00:00Z ".to_string(),
        )
        .expect("conversation should normalize");

        assert_eq!(conversation.conversation_id, "conversation-001");
        assert_eq!(conversation.field_id, "field-001");
        assert_eq!(conversation.created_at, "2026-06-13T16:00:00Z");

        let turn = create_copilot_turn(
            &conversation,
            CopilotTurnCreateRequest {
                turn_id: Some(" turn-001 ".to_string()),
                field_id: " field-001 ".to_string(),
                role: CopilotTurnRole::User,
            },
            "generated-turn".to_string(),
            "2026-06-13T16:01:00Z".to_string(),
        )
        .expect("turn should inherit field scope");

        assert_eq!(turn.conversation_id, "conversation-001");
        assert_eq!(turn.field_id, "field-001");
        assert_eq!(turn.turn_id, "turn-001");
        assert_eq!(turn.role, CopilotTurnRole::User);
    }

    #[test]
    fn copilot_turn_rejects_cross_field_scope() {
        let conversation = start_copilot_conversation(
            CopilotConversationStartRequest {
                conversation_id: Some("conversation-001".to_string()),
                field_id: "field-001".to_string(),
            },
            "generated-conversation".to_string(),
            "2026-06-13T16:00:00Z".to_string(),
        )
        .expect("conversation should normalize");

        let error = create_copilot_turn(
            &conversation,
            CopilotTurnCreateRequest {
                turn_id: Some("turn-foreign".to_string()),
                field_id: "field-foreign".to_string(),
                role: CopilotTurnRole::Assistant,
            },
            "generated-turn".to_string(),
            "2026-06-13T16:01:00Z".to_string(),
        )
        .expect_err("turn must stay scoped to the conversation field");

        assert_eq!(
            error,
            CopilotConversationError::FieldScopeMismatch {
                conversation_field_id: "field-001".to_string(),
                turn_field_id: "field-foreign".to_string()
            }
        );
    }

    #[test]
    fn field_context_carries_scene_zone_and_evidence_into_followup() {
        let conversation = conversation_record("conversation-001", "field-001");
        let first = resolve_copilot_field_context(
            &conversation,
            None,
            CopilotContextUpdateRequest {
                field_id: None,
                active_scene: Some("scene-2026-06-12".to_string()),
                active_zone: Some("zone-ne".to_string()),
                retrieved_evidence: vec![retrieved_evidence("evidence-ndvi-001")],
            },
        )
        .expect("initial context should resolve");

        let followup = resolve_copilot_field_context(
            &conversation,
            Some(&first.context),
            CopilotContextUpdateRequest {
                field_id: None,
                active_scene: None,
                active_zone: None,
                retrieved_evidence: vec![],
            },
        )
        .expect("follow-up should carry context");

        assert_eq!(followup.context.field_id, "field-001");
        assert_eq!(
            followup.context.active_scene.as_deref(),
            Some("scene-2026-06-12")
        );
        assert_eq!(followup.context.active_zone.as_deref(), Some("zone-ne"));
        assert_eq!(
            followup.context.last_evidence_ids,
            vec!["evidence-ndvi-001"]
        );
        assert!(followup.rejected_evidence_ids.is_empty());
    }

    #[test]
    fn field_context_isolates_other_field_evidence() {
        let conversation = conversation_record("conversation-001", "field-001");
        let resolution = resolve_copilot_field_context(
            &conversation,
            None,
            CopilotContextUpdateRequest {
                field_id: None,
                active_scene: Some("scene-2026-06-12".to_string()),
                active_zone: None,
                retrieved_evidence: vec![
                    retrieved_evidence("evidence-ndvi-001"),
                    retrieved_evidence_for_field("evidence-other-field", "field-foreign"),
                ],
            },
        )
        .expect("context should resolve with isolated evidence");

        assert_eq!(
            resolution.context.last_evidence_ids,
            vec!["evidence-ndvi-001"]
        );
        assert_eq!(
            resolution.rejected_evidence_ids,
            vec!["evidence-other-field"]
        );
    }

    #[test]
    fn field_context_rejects_cross_field_followup_scope() {
        let conversation = conversation_record("conversation-001", "field-001");
        let previous = CopilotFieldContext {
            conversation_id: "conversation-001".to_string(),
            field_id: "field-001".to_string(),
            active_scene: None,
            active_zone: None,
            last_evidence_ids: vec!["evidence-ndvi-001".to_string()],
        };

        let error = resolve_copilot_field_context(
            &conversation,
            Some(&previous),
            CopilotContextUpdateRequest {
                field_id: Some("field-foreign".to_string()),
                active_scene: None,
                active_zone: None,
                retrieved_evidence: vec![],
            },
        )
        .expect_err("cross-field follow-up should be rejected");

        assert_eq!(
            error,
            CopilotContextError::FieldScopeMismatch {
                context_field_id: "field-001".to_string(),
                requested_field_id: "field-foreign".to_string()
            }
        );
    }

    #[test]
    fn explain_zone_finding_change_cites_finding_zone_and_28_change() {
        let explanation = explain_zone_finding_change(CopilotExplanationRequest {
            question: "explain the NE zone".to_string(),
            field_id: "field-001".to_string(),
            zone_ref: "zone-ne".to_string(),
            retrieved_evidence: vec![
                evidence_entry(
                    "finding-09-001",
                    EvidenceKind::Finding,
                    Some("zone-ne"),
                    "09 finding: northeast zone is stressed with low NDVI.",
                ),
                evidence_entry(
                    "zone-10-ne",
                    EvidenceKind::ImageryProduct,
                    Some("zone-ne"),
                    "Zone NE boundary covers the stressed canopy cluster.",
                ),
                evidence_entry(
                    "change-28-001",
                    EvidenceKind::Trend,
                    Some("zone-ne"),
                    "28 ranked change event: NDVI declined 0.18 versus aligned baseline pair.",
                ),
            ],
        })
        .expect("explanation should build");

        assert!(!explanation.no_comparable_history);
        assert_eq!(
            explanation.answer.cited_evidence_ids,
            vec![
                "change-28-001".to_string(),
                "finding-09-001".to_string(),
                "zone-10-ne".to_string()
            ]
        );
        assert_eq!(explanation.answer.claims.len(), 3);
        assert!(explanation.answer.text.contains("Domain 28 change context"));
    }

    #[test]
    fn explain_change_with_no_baseline_does_not_invent_history() {
        let explanation = explain_zone_finding_change(CopilotExplanationRequest {
            question: "what changed since last flight?".to_string(),
            field_id: "field-001".to_string(),
            zone_ref: "zone-ne".to_string(),
            retrieved_evidence: vec![
                evidence_entry(
                    "finding-09-001",
                    EvidenceKind::Finding,
                    Some("zone-ne"),
                    "09 finding: northeast zone has low vigor.",
                ),
                evidence_entry(
                    "zone-10-ne",
                    EvidenceKind::ImageryProduct,
                    Some("zone-ne"),
                    "Zone NE boundary covers the stressed canopy cluster.",
                ),
                evidence_entry(
                    "change-28-no-baseline",
                    EvidenceKind::Trend,
                    Some("zone-ne"),
                    "28 reports no baseline for this zone, so no comparable history exists.",
                ),
            ],
        })
        .expect("no-baseline explanation should build");

        assert!(explanation.no_comparable_history);
        assert!(explanation
            .answer
            .text
            .contains("no comparable baseline/history"));
        assert!(!explanation.answer.text.contains("declined 0."));
        assert_eq!(
            explanation.answer.claims[2].cited_evidence_ids,
            vec!["change-28-no-baseline".to_string()]
        );
    }

    #[test]
    fn explain_zone_refuses_without_28_change_evidence() {
        let error = explain_zone_finding_change(CopilotExplanationRequest {
            question: "explain the NE zone".to_string(),
            field_id: "field-001".to_string(),
            zone_ref: "zone-ne".to_string(),
            retrieved_evidence: vec![
                evidence_entry(
                    "finding-09-001",
                    EvidenceKind::Finding,
                    Some("zone-ne"),
                    "09 finding: northeast zone is stressed.",
                ),
                evidence_entry(
                    "zone-10-ne",
                    EvidenceKind::ImageryProduct,
                    Some("zone-ne"),
                    "Zone NE boundary covers the stressed canopy cluster.",
                ),
            ],
        })
        .expect_err("28 change evidence is required");

        assert_eq!(error, CopilotExplanationError::MissingChangeEvidence);
    }

    #[test]
    fn deterministic_model_returns_fixture_answer_with_citations_and_version() {
        let model = DeterministicCopilotModel::new(
            "test-double".to_string(),
            "fixture-rag".to_string(),
            "2026-06-12".to_string(),
            vec![DeterministicAnswerFixture {
                question: "why is the northeast zone stressed?".to_string(),
                text: "The northeast zone is stressed because NDVI dropped below threshold."
                    .to_string(),
                cited_evidence_ids: vec!["evidence-ndvi-001".to_string()],
                confidence: 0.82,
            }],
        )
        .expect("fixture model should be valid");

        let answer = model
            .answer(CopilotAnswerRequest {
                question: " why is the northeast zone stressed? ".to_string(),
                retrieved_evidence: vec![retrieved_evidence("evidence-ndvi-001")],
            })
            .expect("fixture answer should be returned");

        assert_eq!(
            answer.text,
            "The northeast zone is stressed because NDVI dropped below threshold."
        );
        assert_eq!(answer.cited_evidence_ids, vec!["evidence-ndvi-001"]);
        assert_eq!(answer.confidence, 0.82);
        assert_eq!(answer.model_provider, "test-double");
        assert_eq!(answer.model_id, "fixture-rag");
        assert_eq!(answer.model_version, "2026-06-12");
    }

    #[test]
    fn deterministic_model_rejects_fixture_citation_not_in_retrieved_evidence() {
        let model = DeterministicCopilotModel::new(
            "test-double".to_string(),
            "fixture-rag".to_string(),
            "2026-06-12".to_string(),
            vec![DeterministicAnswerFixture {
                question: "what changed?".to_string(),
                text: "NDVI changed in the northeast zone.".to_string(),
                cited_evidence_ids: vec!["evidence-missing".to_string()],
                confidence: 0.7,
            }],
        )
        .expect("fixture model should be valid");

        let error = model
            .answer(CopilotAnswerRequest {
                question: "what changed?".to_string(),
                retrieved_evidence: vec![retrieved_evidence("evidence-ndvi-001")],
            })
            .expect_err("unretrieved citation should fail");

        assert_eq!(
            error,
            CopilotModelError::CitationNotRetrieved {
                evidence_id: "evidence-missing".to_string()
            }
        );
    }

    #[test]
    fn unavailable_model_surfaces_failure_without_fabricated_answer() {
        let model = UnavailableCopilotModel::new(
            "live-adapter".to_string(),
            "deployment model timed out".to_string(),
        );
        let error = model
            .answer(CopilotAnswerRequest {
                question: "why is the crop stressed?".to_string(),
                retrieved_evidence: vec![retrieved_evidence("evidence-ndvi-001")],
            })
            .expect_err("unavailable model should fail cleanly");

        assert_eq!(
            error,
            CopilotModelError::AdapterUnavailable {
                reason: "live-adapter: deployment model timed out".to_string()
            }
        );
    }

    #[test]
    fn grounded_answer_post_check_accepts_claims_with_resolvable_citations() {
        let model = DeterministicCopilotModel::new(
            "test-double".to_string(),
            "fixture-rag".to_string(),
            "2026-06-12".to_string(),
            vec![DeterministicAnswerFixture {
                question: "why is the northeast zone stressed?".to_string(),
                text: "NDVI dropped below threshold in the northeast zone.".to_string(),
                cited_evidence_ids: vec!["evidence-ndvi-001".to_string()],
                confidence: 0.82,
            }],
        )
        .expect("fixture model should be valid");
        let ledger = FixtureLedger {
            refs: BTreeSet::from(["ledger:30:ndvi:001".to_string()]),
        };
        let index = build_evidence_retrieval_index(
            "field-001".to_string(),
            vec![EvidenceCandidate {
                evidence_id: "evidence-ndvi-001".to_string(),
                kind: EvidenceKind::ImageryProduct,
                field_id: "field-001".to_string(),
                scene_ref: Some("scene-2026-06-01".to_string()),
                zone_ref: Some("zone-ne".to_string()),
                ledger_ref: "ledger:30:ndvi:001".to_string(),
                summary: "NDVI in the northeast zone dropped below threshold.".to_string(),
            }],
            &ledger,
        )
        .expect("index should build");
        let retrieved = index.entries.clone();
        let answer = model
            .answer(CopilotAnswerRequest {
                question: "why is the northeast zone stressed?".to_string(),
                retrieved_evidence: retrieved.clone(),
            })
            .expect("fixture answer should be returned");

        let grounded = post_check_grounded_answer(
            answer,
            vec![CopilotAnswerClaim {
                text: "NDVI dropped below threshold in the northeast zone.".to_string(),
                cited_evidence_ids: vec!["evidence-ndvi-001".to_string()],
            }],
            &retrieved,
        )
        .expect("answer should be grounded");

        assert_eq!(grounded.claims.len(), 1);
        assert_eq!(grounded.cited_evidence_ids, vec!["evidence-ndvi-001"]);
        assert_eq!(grounded.model_provider, "test-double");
    }

    #[test]
    fn grounded_answer_post_check_rejects_uncited_claim() {
        let error = post_check_grounded_answer(
            fixture_answer(),
            vec![CopilotAnswerClaim {
                text: "The northeast zone is stressed.".to_string(),
                cited_evidence_ids: vec![],
            }],
            &[retrieved_evidence("evidence-ndvi-001")],
        )
        .expect_err("uncited claim should be rejected");

        assert_eq!(
            error,
            CopilotGroundingError::UncitedClaim {
                claim: "The northeast zone is stressed.".to_string()
            }
        );
    }

    #[test]
    fn grounded_answer_post_check_rejects_unresolved_claim_citation() {
        let error = post_check_grounded_answer(
            fixture_answer(),
            vec![CopilotAnswerClaim {
                text: "The northeast zone is stressed.".to_string(),
                cited_evidence_ids: vec!["evidence-missing".to_string()],
            }],
            &[retrieved_evidence("evidence-ndvi-001")],
        )
        .expect_err("unresolved citation should be rejected");

        assert_eq!(
            error,
            CopilotGroundingError::CitationNotRetrieved {
                claim: "The northeast zone is stressed.".to_string(),
                evidence_id: "evidence-missing".to_string()
            }
        );
    }

    #[test]
    fn grounding_guard_refuses_empty_retrieval_without_calling_model() {
        let model = RecordingCopilotModel::new(fixture_answer());

        let turn = answer_grounded_question(
            &model,
            grounded_question_request("why is the northeast zone stressed?", vec![]),
        )
        .expect("guardrail should return a refusal");

        assert!(turn.refused);
        assert_eq!(
            turn.refusal.as_ref().map(|refusal| refusal.reason),
            Some(CopilotRefusalReason::NoEvidence)
        );
        assert!(turn.answer.is_none());
        assert!(!model.was_called());
    }

    #[test]
    fn grounding_guard_refuses_unresolved_index_evidence_before_model_call() {
        let ledger = FixtureLedger {
            refs: BTreeSet::new(),
        };
        let index = build_evidence_retrieval_index(
            "field-001".to_string(),
            vec![EvidenceCandidate {
                evidence_id: "evidence-ndvi-001".to_string(),
                kind: EvidenceKind::ImageryProduct,
                field_id: "field-001".to_string(),
                scene_ref: Some("scene-2026-06-01".to_string()),
                zone_ref: Some("zone-ne".to_string()),
                ledger_ref: "ledger:30:missing".to_string(),
                summary: "NDVI in the northeast zone dropped below threshold.".to_string(),
            }],
            &ledger,
        )
        .expect("index should build with rejected evidence");
        let model = RecordingCopilotModel::new(fixture_answer());

        let turn = answer_grounded_question(
            &model,
            grounded_question_request("why is the northeast zone stressed?", index.entries.clone()),
        )
        .expect("guardrail should return a refusal");

        assert!(index.entries.is_empty());
        assert!(turn.refused);
        assert_eq!(
            turn.refusal.as_ref().map(|refusal| refusal.reason),
            Some(CopilotRefusalReason::NoEvidence)
        );
        assert!(!model.was_called());
    }

    #[test]
    fn grounding_guard_answers_when_relevant_evidence_exists() {
        let model = RecordingCopilotModel::new(fixture_answer());

        let turn = answer_grounded_question(
            &model,
            grounded_question_request(
                "why is the northeast zone stressed?",
                vec![retrieved_evidence("evidence-ndvi-001")],
            ),
        )
        .expect("grounded answer should return");

        assert!(!turn.refused);
        assert!(turn.refusal.is_none());
        assert!(turn.answer.is_some());
        assert!(model.was_called());
    }

    #[test]
    fn completed_turn_writes_cited_audit_record_before_finalizing() {
        let turn = answer_grounded_question(
            &RecordingCopilotModel::new(fixture_answer()),
            grounded_question_request(
                "why is the northeast zone stressed?",
                vec![retrieved_evidence("evidence-ndvi-001")],
            ),
        )
        .expect("grounded answer should return");
        let mut sink = RecordingAuditSink::default();

        let finalized = finalize_audited_turn(&mut sink, audit_request(turn))
            .expect("audit write should finalize turn");

        assert!(!finalized.turn.refused);
        assert_eq!(sink.records.len(), 1);
        assert_eq!(
            finalized.audit.question,
            "why is the northeast zone stressed?"
        );
        assert_eq!(
            finalized.audit.answer.as_deref(),
            Some("The northeast zone is stressed.")
        );
        assert_eq!(
            finalized.audit.cited_evidence_ids,
            vec!["evidence-ndvi-001"]
        );
        assert_eq!(finalized.audit.confidence, Some(0.82));
        assert_eq!(finalized.audit.interface_version, "copilot-interface-v1");
        assert_eq!(finalized.audit.ledger_ref, "ledger:30:copilot:turn-001");
        assert_eq!(
            finalized.audit.model_provider.as_deref(),
            Some("test-double")
        );
    }

    #[test]
    fn audit_write_failure_blocks_final_answer() {
        let turn = answer_grounded_question(
            &RecordingCopilotModel::new(fixture_answer()),
            grounded_question_request(
                "why is the northeast zone stressed?",
                vec![retrieved_evidence("evidence-ndvi-001")],
            ),
        )
        .expect("grounded answer should return");
        let mut sink = FailingAuditSink;

        let error = finalize_audited_turn(&mut sink, audit_request(turn))
            .expect_err("failed audit must block finalization");

        assert_eq!(
            error,
            CopilotAuditError::AuditWriteFailed {
                reason: "ledger unavailable".to_string()
            }
        );
    }

    #[test]
    fn refused_turn_is_audited_without_model_citations() {
        let turn = answer_grounded_question(
            &RecordingCopilotModel::new(fixture_answer()),
            grounded_question_request("why is the northeast zone stressed?", vec![]),
        )
        .expect("no-evidence turn should refuse");
        let mut sink = RecordingAuditSink::default();

        let finalized = finalize_audited_turn(&mut sink, audit_request(turn))
            .expect("refusal audit should finalize");

        assert!(finalized.turn.refused);
        assert_eq!(
            finalized.audit.refusal_reason,
            Some(CopilotRefusalReason::NoEvidence)
        );
        assert!(finalized.audit.answer.is_none());
        assert!(finalized.audit.cited_evidence_ids.is_empty());
        assert_eq!(sink.records.len(), 1);
    }

    #[test]
    fn uncertainty_marker_is_high_for_fully_cited_fresh_answer() {
        let annotated = annotate_answer_uncertainty(
            grounded_answer_with_claims(vec![CopilotAnswerClaim {
                text: "The northeast zone is stressed.".to_string(),
                cited_evidence_ids: vec!["evidence-ndvi-001".to_string()],
            }]),
            vec![freshness(
                "evidence-ndvi-001",
                EvidenceFreshnessStatus::Fresh,
            )],
        );

        assert_eq!(annotated.uncertainty.level, CopilotConfidenceLevel::High);
        assert_eq!(annotated.uncertainty.coverage, 1.0);
        assert!(annotated
            .uncertainty
            .reason_codes
            .contains(&UncertaintyReasonCode::FullyCitedFreshEvidence));
    }

    #[test]
    fn uncertainty_marker_is_low_for_stale_evidence() {
        let annotated = annotate_answer_uncertainty(
            grounded_answer_with_claims(vec![CopilotAnswerClaim {
                text: "The northeast zone is stressed.".to_string(),
                cited_evidence_ids: vec!["evidence-ndvi-001".to_string()],
            }]),
            vec![freshness(
                "evidence-ndvi-001",
                EvidenceFreshnessStatus::Stale,
            )],
        );

        assert_eq!(annotated.uncertainty.level, CopilotConfidenceLevel::Low);
        assert!(annotated
            .uncertainty
            .reason_codes
            .contains(&UncertaintyReasonCode::StaleEvidence));
    }

    #[test]
    fn uncertainty_marker_is_low_for_partial_claim_coverage() {
        let annotated = annotate_answer_uncertainty(
            grounded_answer_with_claims(vec![
                CopilotAnswerClaim {
                    text: "The northeast zone is stressed.".to_string(),
                    cited_evidence_ids: vec!["evidence-ndvi-001".to_string()],
                },
                CopilotAnswerClaim {
                    text: "Potassium deficiency is likely.".to_string(),
                    cited_evidence_ids: vec![],
                },
            ]),
            vec![freshness(
                "evidence-ndvi-001",
                EvidenceFreshnessStatus::Fresh,
            )],
        );

        assert_eq!(annotated.uncertainty.level, CopilotConfidenceLevel::Low);
        assert_eq!(annotated.uncertainty.coverage, 0.5);
        assert!(annotated
            .uncertainty
            .reason_codes
            .contains(&UncertaintyReasonCode::PartialEvidenceCoverage));
    }

    #[test]
    fn recommendation_draft_is_reviewable_and_inert_until_approval() {
        let answer = grounded_answer_with_claims(vec![CopilotAnswerClaim {
            text: "The northeast zone needs irrigation follow-up.".to_string(),
            cited_evidence_ids: vec!["evidence-ndvi-001".to_string()],
        }]);
        let mut registry = RecommendationLifecycleRegistry::default();

        let draft = draft_recommendation_from_answer(CopilotRecommendationDraftRequest {
            draft_id: " draft-copilot-001 ".to_string(),
            org_id: "org-a".to_string(),
            field_id: "field-001".to_string(),
            scene_id: "scene-2026-06-12".to_string(),
            author_user_id: "copilot".to_string(),
            title: "Review irrigation in NE zone".to_string(),
            note: Some("Grounded draft from a cited answer.".to_string()),
            action_category: "irrigation".to_string(),
            priority: RecommendationPriority::High,
            zone_ref: "zone-ne".to_string(),
            answer,
            created_at: "2026-06-12T14:00:00Z".to_string(),
        })
        .expect("grounded answer should produce a draft");

        assert_eq!(draft.status, CopilotRecommendationDraftStatus::Draft);
        assert_eq!(draft.draft_id, "draft-copilot-001");
        assert_eq!(draft.zone_ref, "zone-ne");
        assert_eq!(draft.cited_evidence_ids, vec!["evidence-ndvi-001"]);
        assert!(registry.recommendations_for_org("org-a").is_empty());

        let approved = approve_recommendation_draft(
            &mut registry,
            draft,
            CopilotRecommendationApproval {
                recommendation_id: "rec-copilot-001".to_string(),
                reviewer_user_id: "advisor-1".to_string(),
                reviewed_at: "2026-06-12T14:05:00Z".to_string(),
            },
        )
        .expect("advisor approval should activate the recommendation");

        assert_eq!(
            approved.draft.status,
            CopilotRecommendationDraftStatus::Approved
        );
        let active = registry.recommendations_for_org("org-a");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].recommendation_id, "rec-copilot-001");
        assert_eq!(active[0].author_user_id, "advisor-1");
        assert_eq!(active[0].evidence_refs, vec!["evidence-ndvi-001"]);
        assert_eq!(active[0].field_id.as_deref(), Some("field-001"));
    }

    #[test]
    fn recommendation_draft_rejects_uncited_answer() {
        let error = draft_recommendation_from_answer(CopilotRecommendationDraftRequest {
            draft_id: "draft-copilot-001".to_string(),
            org_id: "org-a".to_string(),
            field_id: "field-001".to_string(),
            scene_id: "scene-2026-06-12".to_string(),
            author_user_id: "copilot".to_string(),
            title: "Review irrigation in NE zone".to_string(),
            note: None,
            action_category: "irrigation".to_string(),
            priority: RecommendationPriority::High,
            zone_ref: "zone-ne".to_string(),
            answer: GroundedCopilotAnswer {
                text: "Uncited draft should not be allowed.".to_string(),
                claims: vec![CopilotAnswerClaim {
                    text: "The zone needs attention.".to_string(),
                    cited_evidence_ids: vec![],
                }],
                cited_evidence_ids: vec![],
                confidence: 0.6,
                model_provider: "test-double".to_string(),
                model_id: "fixture-rag".to_string(),
                model_version: "2026-06-12".to_string(),
            },
            created_at: "2026-06-12T14:00:00Z".to_string(),
        })
        .expect_err("uncited answer should not produce a draft");

        assert_eq!(error, CopilotRecommendationDraftError::NoCitedEvidence);
    }

    fn retrieved_evidence(evidence_id: &str) -> EvidenceIndexEntry {
        retrieved_evidence_for_field(evidence_id, "field-001")
    }

    fn retrieved_evidence_for_field(evidence_id: &str, field_id: &str) -> EvidenceIndexEntry {
        EvidenceIndexEntry {
            evidence_id: evidence_id.to_string(),
            kind: EvidenceKind::ImageryProduct,
            field_id: field_id.to_string(),
            scene_ref: Some("scene-2026-06-01".to_string()),
            zone_ref: Some("zone-ne".to_string()),
            ledger_ref: format!("ledger:30:{evidence_id}"),
            summary: "NDVI in the northeast zone dropped below threshold.".to_string(),
        }
    }

    fn evidence_entry(
        evidence_id: &str,
        kind: EvidenceKind,
        zone_ref: Option<&str>,
        summary: &str,
    ) -> EvidenceIndexEntry {
        EvidenceIndexEntry {
            evidence_id: evidence_id.to_string(),
            kind,
            field_id: "field-001".to_string(),
            scene_ref: Some("scene-2026-06-01".to_string()),
            zone_ref: zone_ref.map(ToOwned::to_owned),
            ledger_ref: format!("ledger:30:{evidence_id}"),
            summary: summary.to_string(),
        }
    }

    fn conversation_record(
        conversation_id: &str,
        field_id: &str,
    ) -> super::CopilotConversationRecord {
        super::CopilotConversationRecord {
            conversation_id: conversation_id.to_string(),
            field_id: field_id.to_string(),
            created_at: "2026-06-13T16:00:00Z".to_string(),
        }
    }

    fn grounded_answer_with_claims(claims: Vec<CopilotAnswerClaim>) -> GroundedCopilotAnswer {
        GroundedCopilotAnswer {
            text: claims
                .iter()
                .map(|claim| claim.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            cited_evidence_ids: claims
                .iter()
                .flat_map(|claim| claim.cited_evidence_ids.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            claims,
            confidence: 0.82,
            model_provider: "test-double".to_string(),
            model_id: "fixture-rag".to_string(),
            model_version: "2026-06-12".to_string(),
        }
    }

    fn freshness(evidence_id: &str, status: EvidenceFreshnessStatus) -> EvidenceFreshnessRecord {
        EvidenceFreshnessRecord {
            evidence_id: evidence_id.to_string(),
            status,
        }
    }

    fn grounded_question_request(
        question: &str,
        retrieved_evidence: Vec<EvidenceIndexEntry>,
    ) -> GroundedCopilotQuestionRequest {
        GroundedCopilotQuestionRequest {
            question: question.to_string(),
            retrieved_evidence,
            claims: vec![CopilotAnswerClaim {
                text: "The northeast zone is stressed.".to_string(),
                cited_evidence_ids: vec!["evidence-ndvi-001".to_string()],
            }],
        }
    }

    fn fixture_answer() -> super::CopilotAnswer {
        super::CopilotAnswer {
            text: "The northeast zone is stressed.".to_string(),
            cited_evidence_ids: vec!["evidence-ndvi-001".to_string()],
            confidence: 0.82,
            model_provider: "test-double".to_string(),
            model_id: "fixture-rag".to_string(),
            model_version: "2026-06-12".to_string(),
        }
    }

    fn audit_request(turn: super::GroundedCopilotTurn) -> CopilotTurnAuditRequest {
        CopilotTurnAuditRequest {
            conversation_id: "conversation-001".to_string(),
            turn_id: "turn-001".to_string(),
            field_id: "field-001".to_string(),
            question: "why is the northeast zone stressed?".to_string(),
            turn,
            interface_version: "copilot-interface-v1".to_string(),
            ts: "2026-06-13T16:02:00Z".to_string(),
            ledger_ref: "ledger:30:copilot:turn-001".to_string(),
        }
    }

    #[derive(Default)]
    struct RecordingAuditSink {
        records: Vec<CopilotTurnAuditRecord>,
    }

    impl CopilotAuditSink for RecordingAuditSink {
        fn write_turn_audit(&mut self, record: &CopilotTurnAuditRecord) -> Result<(), String> {
            self.records.push(record.clone());
            Ok(())
        }
    }

    struct FailingAuditSink;

    impl CopilotAuditSink for FailingAuditSink {
        fn write_turn_audit(&mut self, _record: &CopilotTurnAuditRecord) -> Result<(), String> {
            Err("ledger unavailable".to_string())
        }
    }

    struct RecordingCopilotModel {
        answer: CopilotAnswer,
        called: Cell<bool>,
    }

    impl RecordingCopilotModel {
        fn new(answer: CopilotAnswer) -> Self {
            Self {
                answer,
                called: Cell::new(false),
            }
        }

        fn was_called(&self) -> bool {
            self.called.get()
        }
    }

    impl CopilotModel for RecordingCopilotModel {
        fn answer(
            &self,
            _request: CopilotAnswerRequest,
        ) -> Result<CopilotAnswer, CopilotModelError> {
            self.called.set(true);
            Ok(self.answer.clone())
        }
    }
}
