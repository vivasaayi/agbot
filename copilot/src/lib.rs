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

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CopilotTurnError {
    #[error("question cannot be empty")]
    EmptyQuestion,
    #[error(transparent)]
    Model(#[from] CopilotModelError),
    #[error(transparent)]
    Grounding(#[from] CopilotGroundingError),
}

pub trait CopilotModel {
    fn answer(&self, request: CopilotAnswerRequest) -> Result<CopilotAnswer, CopilotModelError>;
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

fn normalize_draft_text(
    value: String,
    error: CopilotRecommendationDraftError,
) -> Result<String, CopilotRecommendationDraftError> {
    normalize_text(value).ok_or(error)
}

fn normalize_conversation_text(value: String) -> Option<String> {
    normalize_text(value)
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
        post_check_grounded_answer, start_copilot_conversation, CopilotAnswer, CopilotAnswerClaim,
        CopilotAnswerRequest, CopilotConfidenceLevel, CopilotConversationError,
        CopilotConversationStartRequest, CopilotGroundingError, CopilotIndexError, CopilotModel,
        CopilotModelError, CopilotRecommendationApproval, CopilotRecommendationDraftError,
        CopilotRecommendationDraftRequest, CopilotRecommendationDraftStatus, CopilotRefusalReason,
        CopilotTurnCreateRequest, CopilotTurnRole, DeterministicAnswerFixture,
        DeterministicCopilotModel, EvidenceCandidate, EvidenceFreshnessRecord,
        EvidenceFreshnessStatus, EvidenceIndexEntry, EvidenceKind, EvidenceRejectionReason,
        GroundedCopilotAnswer, GroundedCopilotQuestionRequest, LedgerEvidenceResolver,
        UnavailableCopilotModel, UncertaintyReasonCode,
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
        EvidenceIndexEntry {
            evidence_id: evidence_id.to_string(),
            kind: EvidenceKind::ImageryProduct,
            field_id: "field-001".to_string(),
            scene_ref: Some("scene-2026-06-01".to_string()),
            zone_ref: Some("zone-ne".to_string()),
            ledger_ref: format!("ledger:30:{evidence_id}"),
            summary: "NDVI in the northeast zone dropped below threshold.".to_string(),
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
