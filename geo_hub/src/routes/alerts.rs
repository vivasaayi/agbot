//! Alert route handlers (Layer 4/5 trigger backbone).
//!
//! Thin HTTP wrappers over `crate::alert_evaluation` (evaluate findings into
//! fired alerts, list them, fetch evidence-based severity) and
//! `crate::alert_lifecycle` (fired -> acknowledged -> resolved). The rule engine,
//! dedup, severity classification, lineage, and propose_action live in those
//! modules.

use super::application_error;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use anyhow::Error;
use axum::extract::{Path, State};
use axum::Json;

fn alert_evaluation_error(err: crate::alert_evaluation::AlertEvaluationError) -> AppError {
    use crate::alert_evaluation::AlertEvaluationError;
    match err {
        AlertEvaluationError::Application(inner) => application_error(inner),
        other => AppError::Anyhow(Error::new(other)),
    }
}

/// Body for an alert-evaluation run (Track C phase C1): an optional rule set;
/// when omitted the default rule set is applied.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct AlertEvaluationRequest {
    #[serde(default)]
    pub rules: Option<Vec<alerting::AlertRule>>,
    /// Opt-in (Track C phase C3): also enqueue a Proposed proposal from each
    /// actionable finding that fires an alert. Off by default.
    #[serde(default)]
    pub propose_action: bool,
}

/// Evaluate a field's findings into alerts (Track C phase C1): screens stored
/// findings against a rule set, persisting fired alerts with lineage back to the
/// source finding. Idempotent.
pub async fn evaluate_field_alerts(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
    body: Option<Json<AlertEvaluationRequest>>,
) -> AppResult<Json<Vec<crate::alert_evaluation::StoredAlert>>> {
    let request = body.map(|Json(request)| request).unwrap_or_default();
    let propose_action = request.propose_action;
    let rules = request
        .rules
        .unwrap_or_else(crate::alert_evaluation::default_ruleset);
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let alerts = crate::alert_evaluation::evaluate_field_alerts(
        &state.pool,
        &field_id,
        &rules,
        propose_action,
        &now,
    )
    .await
    .map_err(alert_evaluation_error)?;
    Ok(Json(alerts))
}

/// List the alerts fired for a field (Track C phase C1).
pub async fn list_field_alerts(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<crate::alert_evaluation::StoredAlert>>> {
    let alerts = crate::alert_evaluation::list_field_alerts(&state.pool, &field_id)
        .await
        .map_err(alert_evaluation_error)?;
    Ok(Json(alerts))
}

/// Fetch an alert's evidence-based severity classification (Track C phase C3).
pub async fn get_alert_severity_classification(
    Path(alert_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<alerting::AlertSeverityClassification>> {
    let classification =
        crate::alert_evaluation::get_severity_classification(&state.pool, &alert_id)
            .await
            .map_err(alert_evaluation_error)?
            .ok_or(AppError::NotFound)?;
    Ok(Json(classification))
}

fn alert_lifecycle_error(err: crate::alert_lifecycle::AlertLifecycleError) -> AppError {
    use crate::alert_lifecycle::AlertLifecycleError;
    match err {
        AlertLifecycleError::AlertNotFound(_) => AppError::NotFound,
        // Illegal transitions / validation are the caller's fault -> 400.
        AlertLifecycleError::Alerting(_) | AlertLifecycleError::UnknownState(_) => {
            AppError::BadRequest(err.to_string())
        }
        other => AppError::Anyhow(Error::new(other)),
    }
}

/// Body for a lifecycle transition (Track C phase C2): the acting operator.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AlertLifecycleActionRequest {
    pub actor_id: String,
}

async fn transition_alert(
    state: AppState,
    alert_id: String,
    transition: crate::alert_lifecycle::LifecycleTransition,
    actor_id: String,
) -> AppResult<Json<alerting::AlertLifecycleAction>> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let action =
        crate::alert_lifecycle::transition(&state.pool, &alert_id, transition, &actor_id, &now)
            .await
            .map_err(alert_lifecycle_error)?;
    Ok(Json(action))
}

/// Acknowledge an alert (Track C phase C2): fired -> acknowledged.
pub async fn acknowledge_alert(
    Path(alert_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<AlertLifecycleActionRequest>,
) -> AppResult<Json<alerting::AlertLifecycleAction>> {
    transition_alert(
        state,
        alert_id,
        crate::alert_lifecycle::LifecycleTransition::Acknowledge,
        request.actor_id,
    )
    .await
}

/// Resolve an alert (Track C phase C2): acknowledged -> resolved.
pub async fn resolve_alert(
    Path(alert_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<AlertLifecycleActionRequest>,
) -> AppResult<Json<alerting::AlertLifecycleAction>> {
    transition_alert(
        state,
        alert_id,
        crate::alert_lifecycle::LifecycleTransition::Resolve,
        request.actor_id,
    )
    .await
}

/// Fetch an alert's lifecycle record (Track C phase C2), opening it at the
/// `fired` state if the alert exists but has no lifecycle yet.
pub async fn get_alert_lifecycle(
    Path(alert_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<alerting::AlertLifecycleRecord>> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let record = crate::alert_lifecycle::get_or_open_lifecycle(&state.pool, &alert_id, &now)
        .await
        .map_err(alert_lifecycle_error)?;
    Ok(Json(record))
}
