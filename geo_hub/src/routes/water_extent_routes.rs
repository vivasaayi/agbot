//! Water-extent route handlers (`/api/water-management/extent...`), thin
//! wrappers over `crate::water_extent_rasters`.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::water_extent_rasters::{
    derive_water_extent, list_water_extent_products, WaterExtentDeriveOutcome,
    WaterExtentDeriveRequest, WaterExtentRasterError,
};

impl From<WaterExtentRasterError> for AppError {
    fn from(err: WaterExtentRasterError) -> Self {
        match &err {
            WaterExtentRasterError::NotFound(_) => AppError::NotFound,
            _ if err.is_client_error() => AppError::BadRequest(err.to_string()),
            _ => AppError::Anyhow(err.into()),
        }
    }
}

pub async fn derive_water_extent_route(
    State(state): State<AppState>,
    Json(request): Json<WaterExtentDeriveRequest>,
) -> AppResult<Json<WaterExtentDeriveOutcome>> {
    let outcome = derive_water_extent(&state.pool, &state.config.data_root, &request).await?;
    Ok(Json(outcome))
}

#[derive(Debug, Deserialize)]
pub struct WaterExtentListQuery {
    pub field_id: Option<String>,
}

pub async fn list_water_extent_route(
    Query(query): Query<WaterExtentListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let products =
        list_water_extent_products(&state.pool, query.field_id.filter(|f| !f.trim().is_empty()))
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
    Ok(Json(serde_json::json!({ "water_extent": products })))
}
