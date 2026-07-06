//! Application-run route handlers (Layer 4).
//!
//! Thin HTTP wrappers over the governed application modules
//! (`crate::applications`, `crate::crop_health_run`, `crate::water_priority_run`,
//! `crate::anomaly_run`): record runs, fetch runs, list a field's findings, and
//! trigger the three built-in apps. The domain logic and provenance live in those
//! modules; these handlers only extract, delegate, and map errors.

use super::application_error;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::Json;

/// Record an application run (Track B phase B1): inputs must be cataloged L2/L3
/// products; findings are persisted with provenance lineage.
pub async fn create_application_run(
    Path(app_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<crate::applications::ApplicationRunRequest>,
) -> AppResult<Json<crate::applications::ApplicationRunRecord>> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let record = crate::applications::record_run(&state.pool, &app_id, &request, &now)
        .await
        .map_err(application_error)?;
    Ok(Json(record))
}

/// Fetch an application run by id.
pub async fn get_application_run(
    Path(run_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<crate::applications::ApplicationRunRecord>> {
    let run = crate::applications::get_run(&state.pool, &run_id)
        .await
        .map_err(application_error)?
        .ok_or(AppError::NotFound)?;
    Ok(Json(run))
}

/// List an application's runs are looked up per field; findings for a field are
/// the workspace-facing read.
pub async fn list_field_findings(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<crate::applications::StoredFinding>>> {
    let findings = crate::applications::list_field_findings(&state.pool, &field_id)
        .await
        .map_err(application_error)?;
    Ok(Json(findings))
}

/// Run the crop-health application (Track B phase B3): compose per-zone NDVI
/// stats into findings via `post_processor::crop_health_app`, then record a
/// governed run with lineage back to the zones' cataloged L2/L3 inputs.
pub async fn run_crop_health_app(
    State(state): State<AppState>,
    Json(request): Json<crate::crop_health_run::CropHealthRunRequest>,
) -> AppResult<Json<crate::applications::ApplicationRunRecord>> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let record = crate::crop_health_run::run(&state.pool, &request, &now)
        .await
        .map_err(application_error)?;
    Ok(Json(record))
}

/// Run the water-priority application (Track B phase B4): compose per-zone
/// soil-moisture/deficit stats into findings via
/// `post_processor::water_priority_app`, then record a governed run with lineage
/// back to the zones' cataloged L2/L3 inputs.
pub async fn run_water_priority_app(
    State(state): State<AppState>,
    Json(request): Json<crate::water_priority_run::WaterPriorityRunRequest>,
) -> AppResult<Json<crate::applications::ApplicationRunRecord>> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let record = crate::water_priority_run::run(&state.pool, &request, &now)
        .await
        .map_err(application_error)?;
    Ok(Json(record))
}

/// Run the anomaly-detection application (Track B phase B5): screen per-zone
/// index values via `post_processor::anomaly_app`, then record a governed run
/// with lineage back to the zones' cataloged L2/L3 inputs. Its
/// `index_anomaly_zone` findings feed Track C alert evaluation.
/// Run the drought-watch application (batch 36): evaluate registered
/// drought_index L3 rasters (VCI/TCI/VHI) into stress findings via
/// `post_processor::drought_watch_app`, recorded as a governed run. Its
/// `drought_stress_zone` findings feed Track C alert evaluation.
pub async fn run_drought_watch_app(
    State(state): State<AppState>,
    Json(request): Json<crate::drought_watch_run::DroughtWatchRunRequest>,
) -> AppResult<Json<crate::applications::ApplicationRunRecord>> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let record = crate::drought_watch_run::run(&state.pool, &request, &now)
        .await
        .map_err(application_error)?;
    Ok(Json(record))
}

pub async fn run_anomaly_app(
    State(state): State<AppState>,
    Json(request): Json<crate::anomaly_run::AnomalyRunRequest>,
) -> AppResult<Json<crate::applications::ApplicationRunRecord>> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let record = crate::anomaly_run::run(&state.pool, &request, &now)
        .await
        .map_err(application_error)?;
    Ok(Json(record))
}
