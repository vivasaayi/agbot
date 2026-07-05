//! Land-cover route handlers (`/api/landcover/...`), thin wrappers over
//! `crate::landcover_rasters` (phenology + tier-1 classification).

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::landcover_rasters::{
    derive_landcover, list_landcover_products, LandCoverDeriveRequest, LandCoverError,
    LandCoverOutcome,
};
use crate::state::AppState;

impl From<LandCoverError> for AppError {
    fn from(err: LandCoverError) -> Self {
        if err.is_client_error() {
            AppError::BadRequest(err.to_string())
        } else {
            AppError::Anyhow(err.into())
        }
    }
}

pub async fn derive_landcover_route(
    State(state): State<AppState>,
    Json(request): Json<LandCoverDeriveRequest>,
) -> AppResult<Json<LandCoverOutcome>> {
    let outcome = derive_landcover(&state.pool, &state.config.data_root, &request).await?;
    Ok(Json(outcome))
}

#[derive(Debug, Deserialize)]
pub struct LandCoverListQuery {
    pub field_id: Option<String>,
}

pub async fn list_landcover_route(
    Query(query): Query<LandCoverListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let (phenology, landcover) =
        list_landcover_products(&state.pool, query.field_id.filter(|f| !f.trim().is_empty()))
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
    Ok(Json(serde_json::json!({
        "phenology": phenology,
        "landcover": landcover,
    })))
}
