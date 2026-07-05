//! Alert rules / subscriptions / fired-alert store route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and domain `*_error` mappers are reached from the parent via
//! `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn create_alert_rule(
    State(state): State<AppState>,
    Json(request): Json<AlertRuleCreateRequest>,
) -> AppResult<Json<AlertRuleRecord>> {
    let record = build_alert_rule_record(
        request,
        format!("alert-rule-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(alerting_error)?;
    insert_alert_rule_record(&state, &record).await?;
    Ok(Json(record))
}

pub async fn list_alert_rules(
    Query(query): Query<AlertRuleListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AlertRuleRecord>>> {
    let status = query.status.map(|status| status.as_str().to_string());
    let event_type = normalize_optional_text(query.event_type);
    let include_versions = query.include_versions.unwrap_or(false);
    let rows = if include_versions {
        sqlx::query(
            r#"
            SELECT rule_id, version, event_type, subject_ref, severity, channels_json, status,
                   created_at, updated_at
            FROM alert_rules
            WHERE (?1 IS NULL OR status = ?1)
              AND (?2 IS NULL OR event_type = ?2)
            ORDER BY rule_id ASC, version ASC
            "#,
        )
        .bind(&status)
        .bind(&event_type)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?
    } else {
        sqlx::query(
            r#"
            SELECT rules.rule_id, rules.version, rules.event_type, rules.subject_ref, rules.severity,
                   rules.channels_json, rules.status, rules.created_at, rules.updated_at
            FROM alert_rules AS rules
            JOIN (
                SELECT rule_id, MAX(version) AS version
                FROM alert_rules
                GROUP BY rule_id
            ) AS latest
              ON latest.rule_id = rules.rule_id AND latest.version = rules.version
            WHERE (?1 IS NULL OR rules.status = ?1)
              AND (?2 IS NULL OR rules.event_type = ?2)
            ORDER BY rules.updated_at DESC, rules.rule_id ASC
            "#,
        )
        .bind(&status)
        .bind(&event_type)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?
    };

    rows.into_iter()
        .map(|row| decode_alert_rule_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_alert_rule_versions(
    Path(rule_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AlertRuleRecord>>> {
    let rule_id = normalize_optional_text(Some(rule_id))
        .ok_or_else(|| AppError::BadRequest("rule_id is required".to_string()))?;
    let rows = sqlx::query(
        r#"
        SELECT rule_id, version, event_type, subject_ref, severity, channels_json, status,
               created_at, updated_at
        FROM alert_rules
        WHERE rule_id = ?1
        ORDER BY version ASC
        "#,
    )
    .bind(rule_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;
    if rows.is_empty() {
        return Err(AppError::NotFound);
    }

    rows.into_iter()
        .map(|row| decode_alert_rule_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn update_alert_rule(
    Path(rule_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<AlertRuleUpdateRequest>,
) -> AppResult<Json<AlertRuleRecord>> {
    let rule_id = normalize_optional_text(Some(rule_id))
        .ok_or_else(|| AppError::BadRequest("rule_id is required".to_string()))?;
    let current = load_latest_alert_rule(&state, &rule_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let updated = version_alert_rule_record(&current, request, current_record_timestamp())
        .map_err(alerting_error)?;
    insert_alert_rule_record(&state, &updated).await?;
    Ok(Json(updated))
}

pub async fn update_alert_rule_status(
    Path(rule_id): Path<String>,
    State(state): State<AppState>,
    Json(mut request): Json<AlertRuleStatusUpdateRequest>,
) -> AppResult<Json<AlertRuleRecord>> {
    let rule_id = normalize_optional_text(Some(rule_id))
        .ok_or_else(|| AppError::BadRequest("rule_id is required".to_string()))?;
    let current = load_latest_alert_rule(&state, &rule_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if normalize_optional_text(Some(request.occurred_at.clone())).is_none() {
        request.occurred_at = current_record_timestamp();
    }
    let (updated, audit) = transition_alert_rule_status(
        &current,
        request,
        format!("alert-rule-audit-{}", Uuid::new_v4()),
    )
    .map_err(alerting_error)?;
    insert_alert_rule_record(&state, &updated).await?;
    insert_alert_rule_audit(&state, &audit).await?;
    Ok(Json(updated))
}

pub async fn create_alert_rule_subscription(
    Path(rule_id): Path<String>,
    State(state): State<AppState>,
    Json(mut request): Json<AlertRuleSubscriptionCreateRequest>,
) -> AppResult<Json<AlertRuleSubscriptionRecord>> {
    let rule_id = normalize_optional_text(Some(rule_id))
        .ok_or_else(|| AppError::BadRequest("rule_id is required".to_string()))?;
    if let Some(body_rule_id) = normalize_optional_text(Some(request.rule_id.clone())) {
        if body_rule_id != rule_id {
            return Err(AppError::BadRequest(format!(
                "request rule_id {} does not match path rule_id {}",
                body_rule_id, rule_id
            )));
        }
    }
    let rule = load_latest_alert_rule(&state, &rule_id)
        .await?
        .ok_or(AppError::NotFound)?;
    request.rule_id = rule_id;
    let subscription = build_alert_rule_subscription(
        request,
        &rule,
        format!("alert-rule-subscription-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(alerting_error)?;
    insert_alert_rule_subscription(&state, &subscription).await?;
    Ok(Json(subscription))
}

pub async fn list_alert_rule_subscriptions(
    Path(rule_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AlertRuleSubscriptionRecord>>> {
    let rule_id = normalize_optional_text(Some(rule_id))
        .ok_or_else(|| AppError::BadRequest("rule_id is required".to_string()))?;
    load_latest_alert_rule(&state, &rule_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let rows = sqlx::query(
        r#"
        SELECT subscription_id, rule_id, recipient_id, recipient_role, channels_json, created_at
        FROM alert_rule_subscriptions
        WHERE rule_id = ?1
        ORDER BY created_at ASC, subscription_id ASC
        "#,
    )
    .bind(rule_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_alert_rule_subscription(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn store_fired_alert(
    State(state): State<AppState>,
    Json(record): Json<FiredAlertRecord>,
) -> AppResult<Json<FiredAlertRecord>> {
    let record = normalize_fired_alert_record(record).map_err(alerting_error)?;
    insert_fired_alert_record(&state, &record).await?;
    Ok(Json(record))
}

pub async fn list_fired_alerts(
    Query(query): Query<AlertHistoryListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<AlertHistoryPage>> {
    let source_domain = normalize_optional_text(query.source_domain);
    let field_id = normalize_optional_text(query.field_id);
    let severity = query.severity.map(|severity| severity.as_str().to_string());
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM alert_fired_alerts
        WHERE (?1 IS NULL OR source_domain = ?1)
          AND (?2 IS NULL OR field_id = ?2)
          AND (?3 IS NULL OR severity = ?3)
          AND (?4 IS NULL OR fired_at >= ?4)
          AND (?5 IS NULL OR fired_at <= ?5)
        "#,
    )
    .bind(&source_domain)
    .bind(&field_id)
    .bind(&severity)
    .bind(&start)
    .bind(&end)
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    let rows = sqlx::query(
        r#"
        SELECT alert_id, matched_rule_id, source_event_ref, source_domain, event_type, subject_ref,
               field_id, evidence_refs_json, severity, channels_json, fired_at, explanation
        FROM alert_fired_alerts
        WHERE (?1 IS NULL OR source_domain = ?1)
          AND (?2 IS NULL OR field_id = ?2)
          AND (?3 IS NULL OR severity = ?3)
          AND (?4 IS NULL OR fired_at >= ?4)
          AND (?5 IS NULL OR fired_at <= ?5)
        ORDER BY fired_at DESC, alert_id ASC
        LIMIT ?6 OFFSET ?7
        "#,
    )
    .bind(source_domain)
    .bind(field_id)
    .bind(severity)
    .bind(start)
    .bind(end)
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let alerts = rows
        .into_iter()
        .map(|row| decode_fired_alert_record(&row))
        .collect::<AppResult<Vec<_>>>()?;

    Ok(Json(AlertHistoryPage {
        page,
        page_size,
        total: total as usize,
        alerts,
    }))
}

pub async fn get_fired_alert(
    Path(alert_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<FiredAlertRecord>> {
    let alert_id = normalize_optional_text(Some(alert_id))
        .ok_or_else(|| AppError::BadRequest("alert_id is required".to_string()))?;
    let row = sqlx::query(
        r#"
        SELECT alert_id, matched_rule_id, source_event_ref, source_domain, event_type, subject_ref,
               field_id, evidence_refs_json, severity, channels_json, fired_at, explanation
        FROM alert_fired_alerts
        WHERE alert_id = ?1
        "#,
    )
    .bind(alert_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_fired_alert_record(&row))
        .transpose()?
        .map(Json)
        .ok_or(AppError::NotFound)
}

