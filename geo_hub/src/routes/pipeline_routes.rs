//! Satellite pipeline HTTP routes (batch S-8): per-field dataset
//! subscriptions, the manual discover trigger, and job queue
//! inspection/retry.
//!
//! Thin wrappers over `crate::pipeline` — validation and status mapping
//! live here, queue semantics stay in the domain module.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::pipeline::{
    self, DiscoverPayload, JobFilter, JobKind, JobStatus, PipelineError, SubscriptionUpsert,
};
use crate::state::AppState;

/// Datasets a subscription may watch. `hls` is accepted at the subscription
/// level (HLS products register through `/api/ingest/hls`), even though the
/// Earth Search discover seam only serves `sentinel2` and `landsat`.
const ALLOWED_DATASETS: [&str; 3] = ["landsat", "sentinel2", "hls"];

const DEFAULT_CADENCE_HOURS: i64 = 24;
const DEFAULT_MAX_CLOUD_COVER: f64 = 60.0;
const DEFAULT_LOOKBACK_DAYS: i64 = 14;

/// Priority for operator-triggered discover jobs: ahead of the cadence
/// pass's priority-0 work.
const MANUAL_RUN_PRIORITY: i64 = 10;

pub struct PipelineRouteError {
    status: StatusCode,
    code: String,
    message: String,
}

impl PipelineRouteError {
    fn bad_request(code: &str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: code.to_string(),
            message: message.into(),
        }
    }

    fn not_found(code: &str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: code.to_string(),
            message: message.into(),
        }
    }
}

impl From<PipelineError> for PipelineRouteError {
    fn from(err: PipelineError) -> Self {
        let (status, code) = match &err {
            PipelineError::JobNotFound(_) => (StatusCode::NOT_FOUND, "job_not_found"),
            PipelineError::SubscriptionNotFound(_) => {
                (StatusCode::NOT_FOUND, "subscription_not_found")
            }
            PipelineError::NotRetryable { .. } => (StatusCode::CONFLICT, "job_not_retryable"),
            PipelineError::UnknownEnum(_) => (StatusCode::BAD_REQUEST, "unknown_enum_value"),
            PipelineError::Serde(_) | PipelineError::Db(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "pipeline_storage_error")
            }
        };
        Self {
            status,
            code: code.to_string(),
            message: err.to_string(),
        }
    }
}

impl IntoResponse for PipelineRouteError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({ "code": self.code, "description": self.message })),
        )
            .into_response()
    }
}

async fn field_exists(pool: &crate::db::DbPool, field_id: &str) -> Result<bool, PipelineError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT field_id FROM fields WHERE field_id = ?")
        .bind(field_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

fn validate_dataset(dataset: &str) -> Result<(), PipelineRouteError> {
    if ALLOWED_DATASETS.contains(&dataset) {
        Ok(())
    } else {
        Err(PipelineRouteError::bad_request(
            "unknown_dataset",
            format!(
                "dataset must be one of {}: got {dataset}",
                ALLOWED_DATASETS.join(", ")
            ),
        ))
    }
}

// --- Subscriptions -------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SubscriptionBody {
    pub dataset: String,
    pub indices: Vec<String>,
    #[serde(default)]
    pub cadence_hours: Option<i64>,
    #[serde(default)]
    pub max_cloud_cover: Option<f64>,
    #[serde(default)]
    pub lookback_days: Option<i64>,
}

/// `POST /api/fields/:field_id/subscriptions`: create or update the
/// field/dataset watch.
pub async fn upsert_field_subscription(
    State(state): State<AppState>,
    Path(field_id): Path<String>,
    Json(body): Json<SubscriptionBody>,
) -> Result<Json<serde_json::Value>, PipelineRouteError> {
    validate_dataset(&body.dataset)?;
    if body.indices.is_empty() || body.indices.iter().any(|index| index.trim().is_empty()) {
        return Err(PipelineRouteError::bad_request(
            "invalid_indices",
            "indices must be a non-empty list of index keys",
        ));
    }
    let cadence_hours = body.cadence_hours.unwrap_or(DEFAULT_CADENCE_HOURS);
    let lookback_days = body.lookback_days.unwrap_or(DEFAULT_LOOKBACK_DAYS);
    let max_cloud_cover = body.max_cloud_cover.unwrap_or(DEFAULT_MAX_CLOUD_COVER);
    if cadence_hours <= 0 || lookback_days <= 0 {
        return Err(PipelineRouteError::bad_request(
            "invalid_window",
            "cadence_hours and lookback_days must be positive",
        ));
    }
    if !(0.0..=100.0).contains(&max_cloud_cover) {
        return Err(PipelineRouteError::bad_request(
            "invalid_cloud_cover",
            "max_cloud_cover must be within 0..=100",
        ));
    }
    if !field_exists(&state.pool, &field_id).await? {
        return Err(PipelineRouteError::not_found(
            "field_not_found",
            format!("field {field_id} not found"),
        ));
    }

    let record = pipeline::upsert_subscription(
        &state.pool,
        &SubscriptionUpsert {
            field_id,
            dataset: body.dataset,
            indices: body.indices,
            cadence_hours,
            max_cloud_cover,
            lookback_days,
        },
        Utc::now(),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(record).map_err(PipelineError::from)?,
    ))
}

/// `GET /api/fields/:field_id/subscriptions`.
pub async fn list_field_subscriptions(
    State(state): State<AppState>,
    Path(field_id): Path<String>,
) -> Result<Json<serde_json::Value>, PipelineRouteError> {
    let subscriptions = pipeline::list_subscriptions(&state.pool, Some(&field_id)).await?;
    Ok(Json(json!({
        "field_id": field_id,
        "subscriptions": subscriptions,
    })))
}

#[derive(Debug, Deserialize)]
pub struct SubscriptionStatusBody {
    pub status: String,
}

/// `PATCH /api/subscriptions/:subscription_id`: pause or resume.
pub async fn patch_subscription_status(
    State(state): State<AppState>,
    Path(subscription_id): Path<String>,
    Json(body): Json<SubscriptionStatusBody>,
) -> Result<Json<serde_json::Value>, PipelineRouteError> {
    if !matches!(body.status.as_str(), "active" | "paused") {
        return Err(PipelineRouteError::bad_request(
            "invalid_status",
            format!("status must be active or paused: got {}", body.status),
        ));
    }
    pipeline::set_subscription_status(&state.pool, &subscription_id, &body.status, Utc::now())
        .await?;
    Ok(Json(json!({
        "subscription_id": subscription_id,
        "status": body.status,
    })))
}

// --- Manual trigger --------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PipelineRunBody {
    pub field_id: String,
    #[serde(default)]
    pub dataset: Option<String>,
}

/// `POST /api/pipeline/run`: enqueue discover job(s) for a field now,
/// without waiting for the subscription cadence.
pub async fn run_pipeline_now(
    State(state): State<AppState>,
    Json(body): Json<PipelineRunBody>,
) -> Result<Json<serde_json::Value>, PipelineRouteError> {
    if let Some(dataset) = &body.dataset {
        validate_dataset(dataset)?;
    }
    let subscriptions: Vec<_> = pipeline::list_subscriptions(&state.pool, Some(&body.field_id))
        .await?
        .into_iter()
        .filter(|subscription| {
            body.dataset
                .as_deref()
                .is_none_or(|dataset| subscription.dataset == dataset)
        })
        .collect();
    if subscriptions.is_empty() {
        return Err(PipelineRouteError::not_found(
            "no_subscription",
            format!(
                "no matching subscription for field {}{}",
                body.field_id,
                body.dataset
                    .as_deref()
                    .map(|dataset| format!(" dataset {dataset}"))
                    .unwrap_or_default()
            ),
        ));
    }

    let now = Utc::now();
    let mut runs = Vec::with_capacity(subscriptions.len());
    for subscription in subscriptions {
        let job_key = pipeline::discover_job_key(&subscription.field_id, &subscription.dataset);
        let payload = serde_json::to_value(DiscoverPayload {
            field_id: subscription.field_id.clone(),
            dataset: subscription.dataset.clone(),
        })
        .map_err(PipelineError::from)?;
        let outcome = pipeline::enqueue_job(
            &state.pool,
            JobKind::Discover,
            &job_key,
            &payload,
            Some(&subscription.field_id),
            Some(&subscription.dataset),
            MANUAL_RUN_PRIORITY,
            now,
            None,
            now,
        )
        .await?;
        runs.push(json!({
            "dataset": subscription.dataset,
            "job_key": job_key,
            "outcome": outcome,
        }));
    }
    Ok(Json(json!({ "field_id": body.field_id, "runs": runs })))
}

// --- Job inspection --------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct JobListQuery {
    #[serde(default)]
    pub field_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

/// `GET /api/pipeline/jobs?field_id=&status=&limit=`, newest first.
pub async fn list_pipeline_jobs(
    State(state): State<AppState>,
    Query(query): Query<JobListQuery>,
) -> Result<Json<serde_json::Value>, PipelineRouteError> {
    let status = query
        .status
        .as_deref()
        .map(JobStatus::parse)
        .transpose()
        .map_err(|err| PipelineRouteError::bad_request("invalid_status", err.to_string()))?;
    let jobs = pipeline::list_jobs_filtered(
        &state.pool,
        &JobFilter {
            field_id: query.field_id,
            status,
            limit: query.limit,
        },
    )
    .await?;
    Ok(Json(json!({ "jobs": jobs })))
}

/// `GET /api/pipeline/jobs/:job_id`.
pub async fn get_pipeline_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Json<serde_json::Value>, PipelineRouteError> {
    let job = pipeline::get_job(&state.pool, &job_id)
        .await?
        .ok_or_else(|| {
            PipelineRouteError::not_found("job_not_found", format!("job {job_id} not found"))
        })?;
    Ok(Json(
        serde_json::to_value(job).map_err(PipelineError::from)?,
    ))
}

/// `POST /api/pipeline/jobs/:job_id/retry`: reset a dead/failed job to
/// queued.
pub async fn retry_pipeline_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Json<serde_json::Value>, PipelineRouteError> {
    let job = pipeline::retry_job(&state.pool, &job_id, Utc::now()).await?;
    Ok(Json(
        serde_json::to_value(job).map_err(PipelineError::from)?,
    ))
}
