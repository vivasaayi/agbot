//! Water-extent route handlers (`/api/water-management/extent...`), thin
//! wrappers over `crate::water_extent_rasters`.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::water_extent_rasters::{
    derive_water_extent, list_water_extent_products, register_jrc_dir, register_sentinel1_dir,
    JrcRegisterOutcome, SarRegisterOutcome, WaterExtentDeriveOutcome, WaterExtentDeriveRequest,
    WaterExtentRasterError,
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

#[derive(Debug, Deserialize)]
pub struct SarRegisterRequest {
    /// Server-local directory of calibrated S1 backscatter GeoTIFFs.
    pub dir: String,
}

pub async fn register_sentinel1_route(
    State(state): State<AppState>,
    Json(request): Json<SarRegisterRequest>,
) -> AppResult<Json<SarRegisterOutcome>> {
    let outcome = register_sentinel1_dir(&state.pool, std::path::Path::new(&request.dir)).await?;
    Ok(Json(outcome))
}

#[derive(Debug, Deserialize)]
pub struct JrcRegisterRequest {
    /// Server-local directory of JRC GSW occurrence GeoTIFFs.
    pub dir: String,
}

/// Register JRC Global Surface Water occurrence rasters (batch 25) — the
/// long-term priors the extent derive accepts as `prior_product_id`.
pub async fn register_jrc_route(
    State(state): State<AppState>,
    Json(request): Json<JrcRegisterRequest>,
) -> AppResult<Json<JrcRegisterOutcome>> {
    let outcome = register_jrc_dir(&state.pool, std::path::Path::new(&request.dir)).await?;
    Ok(Json(outcome))
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
