//! Proposal queue (Track D phase D1).
//!
//! A unified accept/reject queue over the pipeline's actionable signals: an
//! alert, a finding, or a recommendation becomes a `Proposal` an operator can
//! accept or reject. Each proposal carries lineage back to its source artifact,
//! so a backward trace closes proposal -> source -> ... -> L0 gap-free.
//!
//! One proposal per source (deterministic id): creating a proposal for a source
//! that already has one returns the existing proposal rather than duplicating.

use crate::db::DbPool;
use crate::provenance_store::{self, ProvenanceStoreError};
use provenance::{ActorIdentity, ArtifactKind, LineageRecord, ProvenanceParameters};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

const PROPOSAL_METHOD: &str = "proposal_queue_v1";

/// The kind of pipeline signal a proposal was raised from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalSourceKind {
    Alert,
    Finding,
    Recommendation,
}

impl ProposalSourceKind {
    fn as_str(self) -> &'static str {
        match self {
            ProposalSourceKind::Alert => "alert",
            ProposalSourceKind::Finding => "finding",
            ProposalSourceKind::Recommendation => "recommendation",
        }
    }

    fn parse(value: &str) -> Result<Self, ProposalError> {
        match value {
            "alert" => Ok(ProposalSourceKind::Alert),
            "finding" => Ok(ProposalSourceKind::Finding),
            "recommendation" => Ok(ProposalSourceKind::Recommendation),
            other => Err(ProposalError::UnknownEnum(other.to_string())),
        }
    }
}

/// The proposal's position in the accept/reject queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Proposed,
    Accepted,
    Rejected,
}

impl ProposalStatus {
    fn as_str(self) -> &'static str {
        match self {
            ProposalStatus::Proposed => "proposed",
            ProposalStatus::Accepted => "accepted",
            ProposalStatus::Rejected => "rejected",
        }
    }

    fn parse(value: &str) -> Result<Self, ProposalError> {
        match value {
            "proposed" => Ok(ProposalStatus::Proposed),
            "accepted" => Ok(ProposalStatus::Accepted),
            "rejected" => Ok(ProposalStatus::Rejected),
            other => Err(ProposalError::UnknownEnum(other.to_string())),
        }
    }
}

/// The accept/reject decision an operator applies to a queued proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalDecision {
    Accept,
    Reject,
}

impl ProposalDecision {
    fn target(self) -> ProposalStatus {
        match self {
            ProposalDecision::Accept => ProposalStatus::Accepted,
            ProposalDecision::Reject => ProposalStatus::Rejected,
        }
    }
}

#[derive(Debug, Error)]
pub enum ProposalError {
    #[error("proposal source {0} is not a known lineage-tracked artifact")]
    SourceNotFound(String),
    #[error("proposal {0} not found")]
    NotFound(String),
    #[error("cannot {decision} a proposal in state {from}")]
    InvalidTransition {
        decision: &'static str,
        from: &'static str,
    },
    #[error("unknown enum value: {0}")]
    UnknownEnum(String),
    #[error(transparent)]
    Provenance(#[from] ProvenanceStoreError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Request to raise a proposal from a source signal.
#[derive(Debug, Clone, Deserialize)]
pub struct ProposalCreateRequest {
    pub source_kind: ProposalSourceKind,
    pub source_id: String,
    #[serde(default)]
    pub field_id: Option<String>,
    pub title: String,
    pub action_category: String,
    pub priority: String,
    #[serde(default)]
    pub rationale: Option<String>,
}

/// Request body for an accept/reject transition.
#[derive(Debug, Clone, Deserialize)]
pub struct ProposalDecisionRequest {
    pub reviewer_id: String,
}

/// A queued proposal.
#[derive(Debug, Clone, Serialize)]
pub struct Proposal {
    pub proposal_id: String,
    pub source_kind: ProposalSourceKind,
    pub source_id: String,
    pub field_id: Option<String>,
    pub title: String,
    pub action_category: String,
    pub priority: String,
    pub status: ProposalStatus,
    pub rationale: Option<String>,
    pub created_at: String,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<String>,
}

fn proposal_id_for(source_id: &str) -> String {
    format!("proposal:{source_id}")
}

/// Raise a proposal from a source signal. The source must be a known lineage-
/// tracked artifact (alert / finding / recommendation). Idempotent: if the
/// source already has a proposal, the existing one is returned unchanged.
pub async fn create_proposal(
    pool: &DbPool,
    request: &ProposalCreateRequest,
    created_at: &str,
) -> Result<Proposal, ProposalError> {
    // The source must exist in the provenance ledger so the proposal's lineage
    // closes to L0.
    if provenance_store::get_lineage(pool, &request.source_id)
        .await?
        .is_none()
    {
        return Err(ProposalError::SourceNotFound(request.source_id.clone()));
    }

    let proposal_id = proposal_id_for(&request.source_id);
    if let Some(existing) = get_proposal(pool, &proposal_id).await? {
        return Ok(existing);
    }

    sqlx::query(
        r#"
        INSERT INTO proposals
            (proposal_id, source_kind, source_id, field_id, title, action_category,
             priority, status, rationale, created_at, reviewed_by, reviewed_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, 'proposed', ?, ?, NULL, NULL)
        "#,
    )
    .bind(&proposal_id)
    .bind(request.source_kind.as_str())
    .bind(&request.source_id)
    .bind(&request.field_id)
    .bind(&request.title)
    .bind(&request.action_category)
    .bind(&request.priority)
    .bind(&request.rationale)
    .bind(created_at)
    .execute(pool)
    .await?;

    // Lineage: the proposal derives from its source signal.
    provenance_store::append_lineage(
        pool,
        &LineageRecord {
            artifact_id: proposal_id.clone(),
            kind: ArtifactKind::Proposal,
            inputs: vec![request.source_id.clone()],
            method: PROPOSAL_METHOD.to_string(),
            parameters: ProvenanceParameters::from_json(json!({
                "source_kind": request.source_kind.as_str(),
                "action_category": request.action_category,
            })),
            operator: "geo_hub:proposal_queue".to_string(),
            actor: ActorIdentity::system("geo_hub:proposal_queue"),
            created_at: created_at.to_string(),
        },
    )
    .await?;

    get_proposal(pool, &proposal_id)
        .await?
        .ok_or_else(|| ProposalError::NotFound(proposal_id))
}

/// Apply an accept/reject decision. Only a `proposed` proposal can transition;
/// re-applying the same decision is idempotent, but flipping a decided proposal
/// is rejected.
pub async fn decide(
    pool: &DbPool,
    proposal_id: &str,
    decision: ProposalDecision,
    reviewer_id: &str,
    at: &str,
) -> Result<Proposal, ProposalError> {
    let proposal = get_proposal(pool, proposal_id)
        .await?
        .ok_or_else(|| ProposalError::NotFound(proposal_id.to_string()))?;

    let target = decision.target();
    match proposal.status {
        ProposalStatus::Proposed => {}
        // Idempotent: same terminal decision is a no-op.
        current if current == target => return Ok(proposal),
        current => {
            return Err(ProposalError::InvalidTransition {
                decision: match decision {
                    ProposalDecision::Accept => "accept",
                    ProposalDecision::Reject => "reject",
                },
                from: current.as_str(),
            })
        }
    }

    sqlx::query(
        r#"
        UPDATE proposals
        SET status = ?, reviewed_by = ?, reviewed_at = ?
        WHERE proposal_id = ?
        "#,
    )
    .bind(target.as_str())
    .bind(reviewer_id)
    .bind(at)
    .bind(proposal_id)
    .execute(pool)
    .await?;

    get_proposal(pool, proposal_id)
        .await?
        .ok_or_else(|| ProposalError::NotFound(proposal_id.to_string()))
}

fn row_to_proposal(row: &sqlx::sqlite::SqliteRow) -> Result<Proposal, ProposalError> {
    use sqlx::Row;
    Ok(Proposal {
        proposal_id: row.get("proposal_id"),
        source_kind: ProposalSourceKind::parse(&row.get::<String, _>("source_kind"))?,
        source_id: row.get("source_id"),
        field_id: row.get("field_id"),
        title: row.get("title"),
        action_category: row.get("action_category"),
        priority: row.get("priority"),
        status: ProposalStatus::parse(&row.get::<String, _>("status"))?,
        rationale: row.get("rationale"),
        created_at: row.get("created_at"),
        reviewed_by: row.get("reviewed_by"),
        reviewed_at: row.get("reviewed_at"),
    })
}

/// Fetch a proposal by id.
pub async fn get_proposal(
    pool: &DbPool,
    proposal_id: &str,
) -> Result<Option<Proposal>, ProposalError> {
    let Some(row) = sqlx::query("SELECT * FROM proposals WHERE proposal_id = ?")
        .bind(proposal_id)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(None);
    };
    Ok(Some(row_to_proposal(&row)?))
}

/// List a field's proposals, newest first (the unified queue view).
pub async fn list_field_proposals(
    pool: &DbPool,
    field_id: &str,
) -> Result<Vec<Proposal>, ProposalError> {
    let rows = sqlx::query(
        "SELECT * FROM proposals WHERE field_id = ? ORDER BY created_at DESC, proposal_id ASC",
    )
    .bind(field_id)
    .fetch_all(pool)
    .await?;
    rows.iter().map(row_to_proposal).collect()
}
