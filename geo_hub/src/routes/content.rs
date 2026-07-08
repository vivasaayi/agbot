//! Content / knowledge-base / community route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and the domain `*_error` mappers are reached from the parent
//! module via `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn create_content_item(
    State(state): State<AppState>,
    Json(request): Json<ContentCreateRequest>,
) -> AppResult<Json<VersionedContentRecord>> {
    let (content, version) = create_versioned_content(
        request,
        format!("content-{}", Uuid::new_v4()),
        format!("content-version-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(content_error)?;
    insert_content_item_with_version(&state, &content, &version).await?;

    Ok(Json(VersionedContentRecord {
        content,
        versions: vec![version],
    }))
}

pub async fn create_success_story_item(
    State(state): State<AppState>,
    Json(request): Json<ContentSuccessStoryCreateRequest>,
) -> AppResult<Json<VersionedSuccessStoryContentRecord>> {
    let (content, version, success_story) = create_success_story_content(
        request,
        format!("content-{}", Uuid::new_v4()),
        format!("content-version-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(content_error)?;
    insert_success_story_item_with_version(&state, &content, &version, &success_story).await?;

    Ok(Json(VersionedSuccessStoryContentRecord {
        content,
        versions: vec![version],
        success_story,
    }))
}

pub async fn append_content_item_version(
    Path(content_id): Path<String>,
    Query(query): Query<ContentItemScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<ContentEditRequest>,
) -> AppResult<Json<VersionedContentRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let content = load_content_record(&state, &content_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if content.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let (updated, version) = append_content_version(
        &content,
        request.body,
        format!("content-version-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(content_error)?;
    append_content_version_record(&state, &updated, &version).await?;

    load_versioned_content(&state, &updated.content_id, &org_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn transition_content_item_workflow(
    Path(content_id): Path<String>,
    Query(query): Query<ContentItemScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<ContentWorkflowTransitionRequest>,
) -> AppResult<Json<ContentWorkflowTransitionResult>> {
    let org_id = normalize_optional_text(query.org_id.clone())
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let content = load_content_record(&state, &content_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if content.org_id != org_id {
        return Err(AppError::NotFound);
    }
    assert_content_workflow_permission(&query, &content, &request, &state).await?;
    let transition = transition_content_workflow(
        &content,
        request,
        format!("content-workflow-audit-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(content_error)?;
    persist_content_workflow_transition(&state, &transition).await?;

    Ok(Json(transition))
}

pub async fn resolve_content_permissions_route(
    Query(query): Query<ContentPermissionQuery>,
) -> AppResult<Json<ContentPermissionSet>> {
    let permissions = resolve_content_permissions(ContentPermissionResolveRequest {
        org_id: query.org_id,
        actor_org_id: query.actor_org_id,
        role_refs: parse_role_refs(query.role_refs),
    })
    .map_err(content_error)?;

    Ok(Json(permissions))
}

pub async fn search_content_items(
    Query(query): Query<ContentSearchQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<ContentSearchResult>>> {
    let org_id = normalize_optional_text(Some(query.org_id))
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let documents = load_content_search_documents(&state, &org_id).await?;
    let results = search_published_content(
        ContentSearchRequest {
            org_id,
            query: query.q,
        },
        documents,
    )
    .map_err(content_error)?;

    Ok(Json(results))
}

pub async fn list_portal_knowledge_base(
    Query(query): Query<ContentPortalEmbedQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<ContentPortalEmbed>> {
    let org_id = normalize_optional_text(Some(query.org_id))
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let actor_org_id = normalize_optional_text(Some(query.actor_org_id)).ok_or_else(|| {
        AppError::BadRequest("actor_org_id query parameter is required".to_string())
    })?;
    let documents = load_content_search_documents(&state, &org_id).await?;
    let embed = build_content_portal_embed(
        ContentPortalEmbedRequest {
            org_id,
            actor_org_id,
            role_refs: parse_role_refs(query.role_refs),
        },
        documents,
    )
    .map_err(content_error)?;

    Ok(Json(embed))
}

pub async fn get_portal_knowledge_base_item(
    Path(content_id): Path<String>,
    Query(query): Query<ContentPortalEmbedQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<ContentPortalEmbedItem>> {
    let org_id = normalize_optional_text(Some(query.org_id))
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let actor_org_id = normalize_optional_text(Some(query.actor_org_id)).ok_or_else(|| {
        AppError::BadRequest("actor_org_id query parameter is required".to_string())
    })?;
    resolve_content_permissions(ContentPermissionResolveRequest {
        org_id: org_id.clone(),
        actor_org_id,
        role_refs: parse_role_refs(query.role_refs),
    })
    .and_then(|permissions| {
        permissions
            .can_read
            .then_some(permissions)
            .ok_or(ContentError::AccessDenied {
                permission: "can_read",
            })
    })
    .map_err(content_error)?;
    let document = load_content_portal_document(&state, &content_id).await?;
    let item = document
        .and_then(|document| content_portal_embed_item(&org_id, document))
        .ok_or(AppError::NotFound)?;

    Ok(Json(item))
}

pub async fn apply_content_item_tags(
    Path(content_id): Path<String>,
    Query(query): Query<ContentItemScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<ContentTagApplyRequest>,
) -> AppResult<Json<Vec<ContentTagRecord>>> {
    let org_id = normalize_optional_text(query.org_id.clone())
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let content = load_content_record(&state, &content_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if content.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let tags = apply_content_taxonomy_tags(content_id, request, current_record_timestamp())
        .map_err(content_error)?;
    insert_content_tags(&state, &tags).await?;

    Ok(Json(tags))
}

pub async fn create_content_engagement_event_route(
    Path(content_id): Path<String>,
    Query(query): Query<ContentItemScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<ContentEngagementEventCreateRequest>,
) -> AppResult<Json<ContentEngagementEventRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let content = load_content_record(&state, &content_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if content.org_id != org_id || content.status != ContentStatus::Published {
        return Err(AppError::NotFound);
    }
    let event = create_content_engagement_event(
        &content,
        request,
        format!("content-engagement-event-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(content_error)?;
    insert_content_engagement_event(&state, &event).await?;

    Ok(Json(event))
}

pub async fn create_content_locale_variant_route(
    Path(content_id): Path<String>,
    Query(query): Query<ContentItemScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<ContentLocaleVariantCreateRequest>,
) -> AppResult<Json<ContentLocaleVariantRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let content = load_content_record(&state, &content_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if content.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let variant = create_content_locale_variant(
        &content,
        request,
        format!("content-locale-version-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(content_error)?;
    upsert_content_locale_variant(&state, &variant).await?;

    Ok(Json(variant))
}

pub async fn get_localized_content_item(
    Path(content_id): Path<String>,
    Query(query): Query<ContentLocalizedQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<ContentLocalizedRecord>> {
    let org_id = normalize_optional_text(Some(query.org_id))
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let versioned = load_versioned_content(&state, &content_id, &org_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let canonical = versioned
        .versions
        .iter()
        .find(|version| version.version_id == versioned.content.current_version)
        .ok_or(AppError::NotFound)?;
    let variant = load_content_locale_variant(&state, &content_id, &query.locale).await?;
    let localized = resolve_localized_content(
        &versioned.content,
        canonical.body.clone(),
        query.locale,
        variant,
    )
    .map_err(content_error)?;

    Ok(Json(localized))
}

pub async fn get_content_engagement_summary(
    Path(content_id): Path<String>,
    Query(query): Query<ContentEngagementSummaryQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<ContentEngagementSummary>> {
    let org_id = normalize_optional_text(Some(query.org_id))
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let period = normalize_optional_text(Some(query.period))
        .ok_or_else(|| AppError::BadRequest("period query parameter is required".to_string()))?;
    let content = load_content_record(&state, &content_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if content.org_id != org_id || content.status != ContentStatus::Published {
        return Err(AppError::NotFound);
    }
    let events = load_content_engagement_events(&state, &content_id, &org_id, &period).await?;
    let summary =
        aggregate_content_engagement(&content, &events, period, current_record_timestamp())
            .map_err(content_error)?;
    upsert_content_engagement_summary(&state, &summary).await?;

    Ok(Json(summary))
}

pub async fn list_content_items_by_tag(
    Query(query): Query<ContentTagFilterQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<ContentRecord>>> {
    let org_id = normalize_optional_text(Some(query.org_id))
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let value = normalize_optional_text(Some(query.value))
        .ok_or_else(|| AppError::BadRequest("tag value query parameter is required".to_string()))?
        .to_ascii_lowercase()
        .replace(' ', "_");
    let rows = sqlx::query(
        r#"
        SELECT c.content_id, c.content_type, c.author_id, c.org_id, c.status,
               c.current_version, c.created_at, c.updated_at
        FROM cms_contents c
        JOIN cms_content_tags t ON t.content_id = c.content_id
        WHERE c.org_id = ?1
          AND t.kind = ?2
          AND t.value = ?3
        ORDER BY c.created_at ASC, c.content_id ASC
        "#,
    )
    .bind(org_id)
    .bind(query.kind.as_str())
    .bind(value)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_content_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_content_item(
    Path(content_id): Path<String>,
    Query(query): Query<ContentItemScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<VersionedContentRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;

    load_versioned_content(&state, &content_id, &org_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn get_success_story_item(
    Path(content_id): Path<String>,
    Query(query): Query<ContentItemScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<VersionedSuccessStoryContentRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let versioned = load_versioned_content(&state, &content_id, &org_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if versioned.content.content_type != ContentType::SuccessStory {
        return Err(AppError::NotFound);
    }
    let success_story = load_success_story_record(&state, &content_id)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(VersionedSuccessStoryContentRecord {
        content: versioned.content,
        versions: versioned.versions,
        success_story,
    }))
}

pub async fn create_community_contribution_route(
    State(state): State<AppState>,
    Json(request): Json<ContentCommunityContributionCreateRequest>,
) -> AppResult<Json<ContentCommunityContributionRecord>> {
    let contribution = create_community_contribution(
        request,
        format!("content-community-contribution-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(content_error)?;
    insert_community_contribution(&state, &contribution).await?;

    Ok(Json(contribution))
}

pub async fn moderate_community_contribution_route(
    Path(contribution_id): Path<String>,
    Query(query): Query<ContentItemScopeQuery>,
    State(state): State<AppState>,
    Json(mut request): Json<ContentContributionModerationRequest>,
) -> AppResult<Json<ContentContributionModerationResult>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let contribution = load_community_contribution(&state, &contribution_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if contribution.org_id != org_id {
        return Err(AppError::NotFound);
    }
    if let Some(actor_org_id) = normalize_optional_text(query.actor_org_id) {
        request.actor_org_id = actor_org_id;
    }
    if let Some(role_refs) = query.role_refs {
        request.role_refs = parse_role_refs(Some(role_refs));
    }
    let result = moderate_community_contribution(
        &contribution,
        request,
        format!("content-community-moderation-audit-{}", Uuid::new_v4()),
        format!("content-community-{}", Uuid::new_v4()),
        format!("content-version-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(content_error)?;
    persist_community_moderation_result(&state, &result).await?;

    Ok(Json(result))
}

pub async fn list_content_items(
    Query(query): Query<ContentItemListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<ContentRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest("org_id query parameter is required for content items".to_string())
    })?;
    let content_type = query
        .content_type
        .map(|content_type| content_type.as_str().to_string());
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT content_id, content_type, author_id, org_id, status, current_version,
               created_at, updated_at
        FROM cms_contents
        WHERE org_id = ?1
          AND (?2 IS NULL OR content_type = ?2)
          AND (?3 IS NULL OR status = ?3)
        ORDER BY created_at ASC, content_id ASC
        "#,
    )
    .bind(org_id)
    .bind(content_type)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_content_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}
