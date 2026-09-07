//! Historical backfill HTTP routes (batch S-11): start a resumable 1982+
//! range walk for a field, inspect its cursor/progress, and pause/resume
//! the enumerate chain.
//!
//! Thin wrappers over `crate::backfill` — validation lives in the domain
//! module, status mapping lives here.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::backfill::{self, BackfillError, CreateBackfillRequest};
use crate::state::AppState;

pub struct BackfillRouteError {
    status: StatusCode,
    code: String,
    message: String,
}

impl BackfillRouteError {
    fn not_found(code: &str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: code.to_string(),
            message: message.into(),
        }
    }

    fn conflict(code: &str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: code.to_string(),
            message: message.into(),
        }
    }
}

impl From<BackfillError> for BackfillRouteError {
    fn from(err: BackfillError) -> Self {
        let (status, code) = match &err {
            BackfillError::NotFound(_) => (StatusCode::NOT_FOUND, "backfill_not_found"),
            BackfillError::Validation(_) => (StatusCode::BAD_REQUEST, "invalid_backfill"),
            BackfillError::Pipeline(_) | BackfillError::Serde(_) | BackfillError::Db(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "backfill_storage_error")
            }
        };
        Self {
            status,
            code: code.to_string(),
            message: err.to_string(),
        }
    }
}

impl IntoResponse for BackfillRouteError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({ "code": self.code, "description": self.message })),
        )
            .into_response()
    }
}

async fn field_exists(pool: &crate::db::DbPool, field_id: &str) -> Result<bool, BackfillError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT field_id FROM fields WHERE field_id = ?")
        .bind(field_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

async fn load_run(
    pool: &crate::db::DbPool,
    backfill_id: &str,
) -> Result<backfill::BackfillRun, BackfillRouteError> {
    backfill::get_backfill_run(pool, backfill_id)
        .await?
        .ok_or_else(|| {
            BackfillRouteError::not_found(
                "backfill_not_found",
                format!("backfill run {backfill_id} not found"),
            )
        })
}

#[derive(Debug, Deserialize)]
pub struct BackfillBody {
    pub datasets: Vec<String>,
    pub indices: Vec<String>,
    /// Inclusive range start, `YYYY-MM-DD`.
    pub start: String,
    /// Exclusive range end, `YYYY-MM-DD`.
    pub end: String,
    #[serde(default)]
    pub max_cloud_cover: Option<f64>,
}

/// `POST /api/fields/:field_id/backfill`: create a run and enqueue the
/// first link of its enumerate chain.
pub async fn start_field_backfill(
    State(state): State<AppState>,
    Path(field_id): Path<String>,
    Json(body): Json<BackfillBody>,
) -> Result<Json<serde_json::Value>, BackfillRouteError> {
    if !field_exists(&state.pool, &field_id).await? {
        return Err(BackfillRouteError::not_found(
            "field_not_found",
            format!("field {field_id} not found"),
        ));
    }
    let now = Utc::now();
    let run = backfill::create_backfill_run(
        &state.pool,
        &CreateBackfillRequest {
            field_id,
            datasets: body.datasets,
            indices: body.indices,
            start_date: body.start,
            end_date: body.end,
            max_cloud_cover: body.max_cloud_cover,
        },
        now,
    )
    .await?;
    backfill::enqueue_enumerate_job(&state.pool, &run, now, now).await?;
    Ok(Json(
        serde_json::to_value(&run).map_err(BackfillError::from)?,
    ))
}

/// `GET /api/fields/:field_id/backfills`: the field's runs, newest first.
pub async fn list_field_backfills(
    State(state): State<AppState>,
    Path(field_id): Path<String>,
) -> Result<Json<serde_json::Value>, BackfillRouteError> {
    let runs = backfill::list_backfill_runs(&state.pool, &field_id).await?;
    Ok(Json(json!({ "field_id": field_id, "backfills": runs })))
}

/// `GET /api/backfills/:backfill_id`: the run plus its pipeline-job
/// progress counts.
pub async fn get_backfill(
    State(state): State<AppState>,
    Path(backfill_id): Path<String>,
) -> Result<Json<serde_json::Value>, BackfillRouteError> {
    let run = load_run(&state.pool, &backfill_id).await?;
    let progress = backfill::progress(&state.pool, &backfill_id).await?;
    let mut body = serde_json::to_value(&run).map_err(BackfillError::from)?;
    body["progress"] = serde_json::to_value(&progress).map_err(BackfillError::from)?;
    Ok(Json(body))
}

/// `POST /api/backfills/:backfill_id/pause`: park the enumerate chain. The
/// in-flight link completes as a no-op; the cursor keeps the resume point.
pub async fn pause_backfill(
    State(state): State<AppState>,
    Path(backfill_id): Path<String>,
) -> Result<Json<serde_json::Value>, BackfillRouteError> {
    let run = load_run(&state.pool, &backfill_id).await?;
    if run.status != "running" {
        return Err(BackfillRouteError::conflict(
            "backfill_not_running",
            format!("backfill run {backfill_id} is {}, not running", run.status),
        ));
    }
    backfill::set_status(&state.pool, &backfill_id, "paused", Utc::now()).await?;
    Ok(Json(
        json!({ "backfill_id": backfill_id, "status": "paused" }),
    ))
}

/// `POST /api/backfills/:backfill_id/resume`: set the run running again and
/// re-enqueue the enumerate chain at the stored cursor.
pub async fn resume_backfill(
    State(state): State<AppState>,
    Path(backfill_id): Path<String>,
) -> Result<Json<serde_json::Value>, BackfillRouteError> {
    let run = load_run(&state.pool, &backfill_id).await?;
    if run.status != "paused" {
        return Err(BackfillRouteError::conflict(
            "backfill_not_paused",
            format!("backfill run {backfill_id} is {}, not paused", run.status),
        ));
    }
    let now = Utc::now();
    backfill::set_status(&state.pool, &backfill_id, "running", now).await?;
    let run = load_run(&state.pool, &backfill_id).await?;
    let outcome = backfill::enqueue_enumerate_job(&state.pool, &run, now, now).await?;
    Ok(Json(json!({
        "backfill_id": backfill_id,
        "status": "running",
        "enumerate_job": outcome,
    })))
}
