//! Change-detection route handlers (`/api/change-detection/...`), thin
//! wrappers over `crate::dnbr_rasters`.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::dnbr_rasters::{
    derive_dnbr, list_dnbr_products, DnbrDeriveOutcome, DnbrDeriveRequest, DnbrRasterError,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

impl From<DnbrRasterError> for AppError {
    fn from(err: DnbrRasterError) -> Self {
        match &err {
            DnbrRasterError::NotFound(_) => AppError::NotFound,
            _ if err.is_client_error() => AppError::BadRequest(err.to_string()),
            _ => AppError::Anyhow(err.into()),
        }
    }
}

pub async fn derive_dnbr_route(
    State(state): State<AppState>,
    Json(request): Json<DnbrDeriveRequest>,
) -> AppResult<Json<DnbrDeriveOutcome>> {
    let outcome = derive_dnbr(&state.pool, &state.config.data_root, &request).await?;
    Ok(Json(outcome))
}

#[derive(Debug, Deserialize)]
pub struct DnbrListQuery {
    pub field_id: Option<String>,
}

pub async fn list_dnbr_route(
    Query(query): Query<DnbrListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let products = list_dnbr_products(&state.pool, query.field_id.filter(|f| !f.trim().is_empty()))
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    Ok(Json(serde_json::json!({ "dnbr": products })))
}
