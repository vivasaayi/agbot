//! Copilot conversations / turns route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and domain `*_error` mappers are reached from the parent via
//! `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn start_copilot_conversation_handler(
    State(state): State<AppState>,
    Json(request): Json<CopilotConversationStartRequest>,
) -> AppResult<Json<CopilotConversationRecord>> {
    let conversation = start_copilot_conversation(
        request,
        format!("copilot-conversation-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(copilot_conversation_error)?;
    assert_copilot_field_exists(&state, &conversation.field_id).await?;
    insert_copilot_conversation(&state, &conversation).await?;

    Ok(Json(conversation))
}

pub async fn list_copilot_conversations(
    Query(query): Query<CopilotConversationListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<CopilotConversationRecord>>> {
    let field_id = normalize_optional_text(query.field_id)
        .ok_or_else(|| AppError::BadRequest("field_id query parameter is required".to_string()))?;
    assert_copilot_field_exists(&state, &field_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT conversation_id, field_id, created_at
        FROM copilot_conversations
        WHERE field_id = ?1
        ORDER BY created_at ASC, conversation_id ASC
        "#,
    )
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_copilot_conversation(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn create_copilot_turn_handler(
    Path(conversation_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CopilotTurnCreateRequest>,
) -> AppResult<Json<CopilotTurnRecord>> {
    let conversation = load_copilot_conversation(&state, &conversation_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let turn = create_copilot_turn(
        &conversation,
        request,
        format!("copilot-turn-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(copilot_conversation_error)?;
    insert_copilot_turn(&state, &turn).await?;

    Ok(Json(turn))
}

pub async fn list_copilot_turns(
    Path(conversation_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<CopilotTurnRecord>>> {
    load_copilot_conversation(&state, &conversation_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let rows = sqlx::query(
        r#"
        SELECT conversation_id, field_id, turn_id, role, created_at
        FROM copilot_turns
        WHERE conversation_id = ?1
        ORDER BY created_at ASC, rowid ASC
        "#,
    )
    .bind(conversation_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_copilot_turn(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

