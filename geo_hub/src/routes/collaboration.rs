//! Real-time collaboration / mission-plan / streams route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and the domain `*_error` mappers are reached from the parent
//! module via `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn create_collaboration_channel(
    State(state): State<AppState>,
    Json(request): Json<CollaborationChannelCreateRequest>,
) -> AppResult<Json<CollaborationChannelRecord>> {
    let channel = build_collaboration_channel(
        request,
        format!("collab-channel-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;
    validate_collaboration_field_ref(&state, &channel.org_id, &channel.field_ref).await?;
    insert_collaboration_channel(&state, &channel).await?;

    Ok(Json(channel))
}

pub async fn resolve_collaboration_permissions_route(
    Query(query): Query<CollaborationPermissionQuery>,
) -> AppResult<Json<CollaborationPermissionSet>> {
    let permissions =
        shared::schemas::resolve_collaboration_permissions(CollaborationPermissionResolveRequest {
            org_id: query.org_id,
            actor_org_id: query.actor_org_id,
            role_refs: parse_role_refs(query.role_refs),
        })
        .map_err(collaboration_error)?;

    Ok(Json(permissions))
}

pub async fn authorize_collaboration_action_route(
    State(state): State<AppState>,
    Json(request): Json<CollaborationActionAuthorizeRequest>,
) -> AppResult<Json<CollaborationPermissionDecision>> {
    let actor_id = normalize_optional_text(Some(request.actor_id.clone()))
        .ok_or_else(|| collaboration_error(CollaborationError::EmptyActorId))?;
    let channel_id = normalize_optional_text(request.channel_id.clone());
    let decision = authorize_collaboration_action(
        CollaborationPermissionResolveRequest {
            org_id: request.org_id,
            actor_org_id: request.actor_org_id,
            role_refs: request.role_refs,
        },
        request.action,
    )
    .map_err(collaboration_error)?;
    insert_collaboration_permission_audit(&state, &decision, &actor_id, channel_id.as_deref())
        .await?;
    if !decision.allowed {
        return Err(AppError::Forbidden(
            CollaborationError::AccessDenied {
                permission: decision.action.permission_name(),
            }
            .to_string(),
        ));
    }

    Ok(Json(decision))
}

pub async fn list_collaboration_channels(
    Query(query): Query<CollaborationChannelListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<CollaborationChannelRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for collaboration channels".to_string(),
        )
    })?;
    let field_ref = normalize_optional_text(query.field_ref);
    let rows = sqlx::query(
        r#"
        SELECT channel_id, org_id, field_ref, member_account_ids_json, created_at
        FROM collab_channels
        WHERE org_id = ?1
          AND (?2 IS NULL OR field_ref = ?2)
        ORDER BY created_at ASC, channel_id ASC
        "#,
    )
    .bind(org_id)
    .bind(field_ref)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_collaboration_channel(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_collaboration_channel(
    Path(channel_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<CollaborationChannelThread>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;

    load_collaboration_thread(&state, &channel_id, &org_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn post_collaboration_message(
    Path(channel_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<CollaborationMessageCreateRequest>,
) -> AppResult<Json<CollaborationMessageRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let channel = load_collaboration_channel(&state, &channel_id).await?;
    let Some(channel) = channel else {
        return Err(AppError::BadRequest(format!(
            "collaboration channel {channel_id} does not exist"
        )));
    };
    if channel.org_id != org_id {
        return Err(AppError::NotFound);
    }
    assert_collaboration_action_permission(
        &state,
        &channel.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Post,
        Some(&channel.channel_id),
    )
    .await?;
    let message = build_collaboration_message(
        request,
        Some(&channel),
        format!("collab-message-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;
    insert_collaboration_message(&state, &message, &channel.org_id).await?;

    Ok(Json(message))
}

pub async fn update_collaboration_presence_route(
    Path(channel_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<CollaborationPresenceUpdateRequest>,
) -> AppResult<Json<CollaborationPresenceRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let channel = load_collaboration_channel(&state, &channel_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!("collaboration channel {channel_id} does not exist"))
        })?;
    if channel.org_id != org_id {
        return Err(AppError::NotFound);
    }
    assert_collaboration_action_permission(
        &state,
        &channel.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Join,
        Some(&channel.channel_id),
    )
    .await?;
    let record = update_collaboration_presence(&channel, request, current_record_timestamp())
        .map_err(collaboration_error)?;
    upsert_collaboration_presence(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_collaboration_presence(
    Path(channel_id): Path<String>,
    Query(query): Query<CollaborationPresenceListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<CollaborationPresenceRecord>>> {
    let channel = load_collaboration_channel(&state, &channel_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if channel.org_id != query.org_id {
        return Err(AppError::NotFound);
    }
    let mut records = load_collaboration_presence_records(&state, &channel_id).await?;
    if let Some(stale_before) = normalize_optional_text(query.stale_before) {
        records =
            expire_lapsed_collaboration_presence(records, stale_before, current_record_timestamp())
                .map_err(collaboration_error)?;
        upsert_collaboration_presence_records(&state, &records).await?;
    }

    Ok(Json(records))
}

pub async fn create_collaboration_notifications(
    Path(channel_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<CollaborationNotificationEventRequest>,
) -> AppResult<Json<Vec<CollaborationNotificationRecord>>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let channel = load_collaboration_channel(&state, &channel_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!("collaboration channel {channel_id} does not exist"))
        })?;
    if channel.org_id != org_id {
        return Err(AppError::NotFound);
    }
    assert_collaboration_action_permission(
        &state,
        &channel.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Post,
        Some(&channel.channel_id),
    )
    .await?;
    let notifications = build_collaboration_notifications(
        &channel,
        request,
        format!("collab-event-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;
    insert_collaboration_notifications(&state, &notifications).await?;

    Ok(Json(notifications))
}

pub async fn raise_collaboration_emergency_alert_route(
    Path(channel_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<CollaborationEmergencyAlertCreateRequest>,
) -> AppResult<Json<CollaborationEmergencyAlertRaiseResult>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let actor_id = normalize_optional_text(query.actor_id.clone())
        .ok_or_else(|| collaboration_error(CollaborationError::EmptyActorId))?;
    let channel = load_collaboration_channel(&state, &channel_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!("collaboration channel {channel_id} does not exist"))
        })?;
    if channel.org_id != org_id {
        return Err(AppError::NotFound);
    }
    assert_collaboration_action_permission(
        &state,
        &channel.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Alert,
        Some(&channel.channel_id),
    )
    .await?;
    let result = raise_collaboration_emergency_alert(
        &channel,
        request,
        format!("collab-alert-{}", Uuid::new_v4()),
        format!("collab-alert-audit-{}", Uuid::new_v4()),
        actor_id,
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;
    persist_collaboration_emergency_alert_raise(&state, &result).await?;

    Ok(Json(result))
}

pub async fn transition_collaboration_emergency_alert_route(
    Path(alert_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<CollaborationEmergencyAlertTransitionRequest>,
) -> AppResult<Json<CollaborationEmergencyAlertTransitionResult>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let alert = load_collaboration_emergency_alert(&state, &alert_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if alert.org_id != org_id {
        return Err(AppError::NotFound);
    }
    assert_collaboration_action_permission(
        &state,
        &alert.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Alert,
        Some(&alert.channel_id),
    )
    .await?;
    let result = transition_collaboration_emergency_alert(
        &alert,
        request,
        format!("collab-alert-audit-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;
    persist_collaboration_emergency_alert_transition(&state, &result).await?;

    Ok(Json(result))
}

pub async fn record_collaboration_session_route(
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<CollaborationSessionRecordRequest>,
) -> AppResult<Json<CollaborationSessionReplay>> {
    assert_collaboration_action_permission(
        &state,
        &request.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Post,
        None,
    )
    .await?;
    let replay = record_collaboration_session(
        request,
        format!("collab-session-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;
    persist_collaboration_session_replay(&state, &replay).await?;

    Ok(Json(replay))
}

pub async fn replay_collaboration_session_route(
    Path(session_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<CollaborationSessionReplay>> {
    let replay = load_collaboration_session_replay(&state, &session_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    if replay.session.org_id != org_id {
        return Err(AppError::NotFound);
    }
    assert_collaboration_action_permission(
        &state,
        &replay.session.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Join,
        None,
    )
    .await?;

    Ok(Json(replay))
}

pub async fn create_collaboration_session_annotation_route(
    Path(session_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
    Json(mut request): Json<CollaborationSessionAnnotationCreateRequest>,
) -> AppResult<Json<CollaborationSessionAnnotationRecord>> {
    let replay = load_collaboration_session_replay(&state, &session_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    if replay.session.org_id != org_id {
        return Err(AppError::NotFound);
    }
    assert_collaboration_action_permission(
        &state,
        &replay.session.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Annotate,
        None,
    )
    .await?;
    if !scene_exists(&state, &request.scene_id).await? {
        return Err(AppError::NotFound);
    }
    let stream = load_collaboration_stream(&state, &request.stream_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if stream.org_id != replay.session.org_id {
        return Err(AppError::NotFound);
    }
    if stream.state == CollaborationStreamState::Ended {
        return Err(AppError::BadRequest(
            "collaboration stream is not active".to_string(),
        ));
    }

    request.annotation.author = Some(request.actor_id.clone());
    let generated_link_id = format!("collab-session-annotation-{}", Uuid::new_v4());
    request.annotation.audit_id = Some(generated_link_id.clone());
    let annotation = build_annotation_record(&state, &request.scene_id, request.annotation).await?;
    let link = link_collaboration_session_annotation(
        CollaborationSessionAnnotationLinkRequest {
            session_id,
            org_id: replay.session.org_id,
            scene_id: annotation.scene_id.clone(),
            annotation_id: annotation.annotation_id.clone(),
            actor_id: request.actor_id,
            connection_active: request.connection_active,
        },
        generated_link_id,
        annotation.created_at.clone(),
    )
    .map_err(collaboration_error)?;
    persist_collaboration_session_annotation(&state, &link, &annotation).await?;

    Ok(Json(CollaborationSessionAnnotationRecord {
        link,
        annotation,
    }))
}

pub async fn list_collaboration_session_annotations_route(
    Path(session_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<CollaborationSessionAnnotationRecord>>> {
    let replay = load_collaboration_session_replay(&state, &session_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    if replay.session.org_id != org_id {
        return Err(AppError::NotFound);
    }
    assert_collaboration_action_permission(
        &state,
        &replay.session.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Join,
        None,
    )
    .await?;

    Ok(Json(
        load_collaboration_session_annotations(&state, &session_id, &org_id).await?,
    ))
}

pub async fn collaboration_operator_console_feed_route(
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<CollaborationOperatorConsoleFeed>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    assert_collaboration_action_permission(
        &state,
        &org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Stream,
        None,
    )
    .await?;
    let streams = load_collaboration_operator_console_streams(&state, &org_id).await?;
    let alerts = load_collaboration_operator_console_active_alerts(&state, &org_id).await?;
    let feed = build_collaboration_operator_console_feed(
        org_id,
        streams,
        alerts,
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;

    Ok(Json(feed))
}

pub async fn collaboration_portal_feed_route(
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<CollaborationPortalFeed>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    assert_collaboration_action_permission(
        &state,
        &org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Join,
        None,
    )
    .await?;
    let streams = load_collaboration_operator_console_streams(&state, &org_id).await?;
    let alerts = load_collaboration_operator_console_active_alerts(&state, &org_id).await?;
    let feed = build_collaboration_operator_console_feed(
        org_id,
        streams,
        alerts,
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;

    Ok(Json(feed))
}

pub async fn collaboration_portal_stream_route(
    Path(stream_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<CollaborationLiveStreamRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    assert_collaboration_action_permission(
        &state,
        &org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Join,
        None,
    )
    .await?;
    let stream = load_collaboration_stream(&state, &stream_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if stream.org_id != org_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(stream))
}

pub async fn collaboration_portal_alert_route(
    Path(alert_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<CollaborationEmergencyAlertRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    assert_collaboration_action_permission(
        &state,
        &org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Join,
        None,
    )
    .await?;
    let alert = load_collaboration_emergency_alert(&state, &alert_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if alert.org_id != org_id || alert.state == CollaborationEmergencyAlertState::Resolved {
        return Err(AppError::NotFound);
    }

    Ok(Json(alert))
}

pub async fn create_collaboration_mission_plan_route(
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<CollaborationMissionPlanCreateRequest>,
) -> AppResult<Json<CollaborationMissionPlanRecord>> {
    assert_collaboration_action_permission(
        &state,
        &request.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Annotate,
        None,
    )
    .await?;
    let plan = create_collaboration_mission_plan(
        request,
        format!("collab-mission-plan-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;
    upsert_collaboration_mission_plan(&state, &plan).await?;

    Ok(Json(plan))
}

pub async fn edit_collaboration_mission_plan_route(
    Path(plan_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<CollaborationMissionWaypointEditRequest>,
) -> AppResult<Json<CollaborationMissionEditResult>> {
    let plan = load_collaboration_mission_plan(&state, &plan_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    if plan.org_id != org_id {
        return Err(AppError::NotFound);
    }
    assert_collaboration_action_permission(
        &state,
        &plan.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Annotate,
        None,
    )
    .await?;
    let result = apply_collaboration_mission_edit(
        &plan,
        request,
        format!("collab-mission-edit-audit-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;
    persist_collaboration_mission_edit_result(&state, &result).await?;

    Ok(Json(result))
}

pub async fn dispatch_collaboration_mission_plan_route(
    Path(plan_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<CollaborationMissionDispatchRequest>,
) -> AppResult<Json<CollaborationMissionDispatchResult>> {
    let plan = load_collaboration_mission_plan(&state, &plan_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    if plan.org_id != org_id {
        return Err(AppError::NotFound);
    }
    assert_collaboration_action_permission(
        &state,
        &plan.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Dispatch,
        None,
    )
    .await?;
    let result = evaluate_collaboration_mission_dispatch(
        &plan,
        request,
        format!("collab-mission-dispatch-audit-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;
    insert_collaboration_mission_dispatch_audit(&state, &result.audit).await?;

    Ok(Json(result))
}

pub async fn start_collaboration_stream_route(
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<CollaborationStreamStartRequest>,
) -> AppResult<Json<CollaborationLiveStreamRecord>> {
    assert_collaboration_action_permission(
        &state,
        &request.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Stream,
        None,
    )
    .await?;
    let stream = start_collaboration_stream(
        request,
        format!("collab-stream-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;
    insert_collaboration_stream(&state, &stream).await?;

    Ok(Json(stream))
}

pub async fn relay_collaboration_stream_frame_route(
    Path(stream_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<CollaborationStreamFrameRelayRequest>,
) -> AppResult<Json<CollaborationStreamRelayResult>> {
    let stream = load_collaboration_stream(&state, &stream_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    if stream.org_id != org_id {
        return Err(AppError::NotFound);
    }
    assert_collaboration_action_permission(
        &state,
        &stream.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Stream,
        None,
    )
    .await?;
    let next_sequence = next_collaboration_stream_sequence(&state, &stream_id).await?;
    let result = relay_collaboration_stream_frame(
        &stream,
        request,
        format!("collab-frame-{}", Uuid::new_v4()),
        next_sequence,
    )
    .map_err(collaboration_error)?;
    persist_collaboration_stream_relay_result(&state, &result).await?;

    Ok(Json(result))
}

pub async fn list_collaboration_stream_frames(
    Path(stream_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<CollaborationStreamFrameRecord>>> {
    let stream = load_collaboration_stream(&state, &stream_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    if stream.org_id != org_id {
        return Err(AppError::NotFound);
    }
    assert_collaboration_action_permission(
        &state,
        &stream.org_id,
        query.actor_org_id,
        query.actor_id,
        query.role_refs,
        CollaborationAction::Join,
        None,
    )
    .await?;
    let frames = load_collaboration_stream_frames(&state, &stream_id).await?;

    Ok(Json(frames))
}

