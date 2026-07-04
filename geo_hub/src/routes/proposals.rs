//! Proposal-queue route handlers (Layer 5).
//!
//! Thin HTTP wrappers over `crate::proposal_queue`: raise a proposal from a
//! pipeline signal, list a field's queue, fetch one, and accept/reject with a
//! reviewer identity. Governance (source validation, idempotency, transition
//! guards, lineage) lives in the queue module.

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use anyhow::Error;
use axum::extract::{Path, State};
use axum::Json;

fn proposal_error(err: crate::proposal_queue::ProposalError) -> AppError {
    use crate::proposal_queue::ProposalError;
    match err {
        ProposalError::NotFound(_) => AppError::NotFound,
        ProposalError::SourceNotFound(_)
        | ProposalError::InvalidTransition { .. }
        | ProposalError::UnknownEnum(_) => AppError::BadRequest(err.to_string()),
        other => AppError::Anyhow(Error::new(other)),
    }
}

/// Raise a proposal from a pipeline signal (Track D phase D1). Idempotent per
/// source: the source must be a lineage-tracked artifact.
pub async fn create_proposal(
    State(state): State<AppState>,
    Json(request): Json<crate::proposal_queue::ProposalCreateRequest>,
) -> AppResult<Json<crate::proposal_queue::Proposal>> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let proposal = crate::proposal_queue::create_proposal(&state.pool, &request, &now)
        .await
        .map_err(proposal_error)?;
    Ok(Json(proposal))
}

/// List a field's proposal queue (Track D phase D1).
pub async fn list_field_proposals(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<crate::proposal_queue::Proposal>>> {
    let proposals = crate::proposal_queue::list_field_proposals(&state.pool, &field_id)
        .await
        .map_err(proposal_error)?;
    Ok(Json(proposals))
}

/// Fetch a single proposal (Track D phase D1).
pub async fn get_proposal(
    Path(proposal_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<crate::proposal_queue::Proposal>> {
    let proposal = crate::proposal_queue::get_proposal(&state.pool, &proposal_id)
        .await
        .map_err(proposal_error)?
        .ok_or(AppError::NotFound)?;
    Ok(Json(proposal))
}

async fn decide_proposal(
    state: AppState,
    proposal_id: String,
    decision: crate::proposal_queue::ProposalDecision,
    reviewer_id: String,
) -> AppResult<Json<crate::proposal_queue::Proposal>> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let proposal =
        crate::proposal_queue::decide(&state.pool, &proposal_id, decision, &reviewer_id, &now)
            .await
            .map_err(proposal_error)?;
    Ok(Json(proposal))
}

/// Accept a queued proposal (Track D phase D1): proposed -> accepted.
pub async fn accept_proposal(
    Path(proposal_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<crate::proposal_queue::ProposalDecisionRequest>,
) -> AppResult<Json<crate::proposal_queue::Proposal>> {
    decide_proposal(
        state,
        proposal_id,
        crate::proposal_queue::ProposalDecision::Accept,
        request.reviewer_id,
    )
    .await
}

/// Reject a queued proposal (Track D phase D1): proposed -> rejected.
pub async fn reject_proposal(
    Path(proposal_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<crate::proposal_queue::ProposalDecisionRequest>,
) -> AppResult<Json<crate::proposal_queue::Proposal>> {
    decide_proposal(
        state,
        proposal_id,
        crate::proposal_queue::ProposalDecision::Reject,
        request.reviewer_id,
    )
    .await
}
