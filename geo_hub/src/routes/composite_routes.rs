//! Temporal-composite route handlers (`/api/composites...`), thin wrappers
//! over `crate::composite_rasters` (satellite pipeline batch 32).

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::composite_rasters::{
    derive_composite, list_composite_products, CompositeDeriveOutcome, CompositeDeriveRequest,
    CompositeRasterError,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

impl From<CompositeRasterError> for AppError {
    fn from(err: CompositeRasterError) -> Self {
        if err.is_client_error() {
            AppError::BadRequest(err.to_string())
        } else {
            AppError::Anyhow(err.into())
        }
    }
}

pub async fn derive_composite_route(
    State(state): State<AppState>,
    Json(request): Json<CompositeDeriveRequest>,
) -> AppResult<Json<CompositeDeriveOutcome>> {
    let outcome = derive_composite(&state.pool, &state.config.data_root, &request).await?;
    Ok(Json(outcome))
}

#[derive(Debug, Deserialize)]
pub struct CompositeListQuery {
    pub field_id: Option<String>,
}

pub async fn list_composites_route(
    Query(query): Query<CompositeListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let products =
        list_composite_products(&state.pool, query.field_id.filter(|f| !f.trim().is_empty()))
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
    Ok(Json(serde_json::json!({ "composites": products })))
}
