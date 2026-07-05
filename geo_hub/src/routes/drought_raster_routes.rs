//! Drought raster route handlers (`/api/drought-management/rasters...`).
//!
//! Thin wrappers over `crate::drought_rasters`: the existing
//! `/api/drought-management/indices` routes serve scalar per-field records;
//! these serve the raster path — climatology + VCI/TCI GeoTIFF products
//! derived from the catalog, browseable via `/api/stac` and web-tileable via
//! the catalog product tiler.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::drought_rasters::{
    derive_drought_raster, list_drought_raster_products, DroughtRasterError, DroughtRasterOutcome,
    DroughtRasterRequest,
};
use crate::error::{AppError, AppResult};
use crate::spi_rasters::{
    derive_spi_raster, list_spi_products, register_chirps_dir, ChirpsRegisterOutcome,
    SpiDeriveOutcome, SpiDeriveRequest, SpiRasterError,
};
use crate::state::AppState;

impl From<DroughtRasterError> for AppError {
    fn from(err: DroughtRasterError) -> Self {
        match &err {
            DroughtRasterError::CurrentNotFound(_) => AppError::NotFound,
            _ if err.is_client_error() => AppError::BadRequest(err.to_string()),
            _ => AppError::Anyhow(err.into()),
        }
    }
}

pub async fn derive_drought_raster_route(
    State(state): State<AppState>,
    Json(request): Json<DroughtRasterRequest>,
) -> AppResult<Json<DroughtRasterOutcome>> {
    let outcome = derive_drought_raster(&state.pool, &state.config.data_root, &request).await?;
    Ok(Json(outcome))
}

#[derive(Debug, Deserialize)]
pub struct DroughtRasterListQuery {
    pub field_id: Option<String>,
}

pub async fn list_drought_rasters_route(
    Query(query): Query<DroughtRasterListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let field_id = query.field_id.filter(|f| !f.trim().is_empty());
    let (climatologies, droughts) = list_drought_raster_products(&state.pool, field_id.clone())
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let spi = list_spi_products(&state.pool, field_id)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    Ok(Json(serde_json::json!({
        "climatologies": climatologies,
        "drought_indices": droughts,
        "spi": spi,
    })))
}

impl From<SpiRasterError> for AppError {
    fn from(err: SpiRasterError) -> Self {
        match &err {
            SpiRasterError::CurrentNotFound(_) => AppError::NotFound,
            SpiRasterError::Shared(DroughtRasterError::CurrentNotFound(_)) => AppError::NotFound,
            _ if err.is_client_error() => AppError::BadRequest(err.to_string()),
            _ => AppError::Anyhow(err.into()),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct ChirpsRegisterRequest {
    /// Server-local directory holding CHIRPS monthly GeoTIFFs.
    pub dir: String,
}

pub async fn register_chirps_route(
    State(state): State<AppState>,
    Json(request): Json<ChirpsRegisterRequest>,
) -> AppResult<Json<ChirpsRegisterOutcome>> {
    let outcome = register_chirps_dir(&state.pool, std::path::Path::new(&request.dir)).await?;
    Ok(Json(outcome))
}

pub async fn derive_spi_raster_route(
    State(state): State<AppState>,
    Json(request): Json<SpiDeriveRequest>,
) -> AppResult<Json<SpiDeriveOutcome>> {
    let outcome = derive_spi_raster(&state.pool, &state.config.data_root, &request).await?;
    Ok(Json(outcome))
}
