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
    derive_drought_raster, derive_vhi_raster, list_drought_raster_products, DroughtRasterError,
    DroughtRasterOutcome, DroughtRasterRequest, VhiDeriveOutcome, VhiDeriveRequest,
};
use crate::error::{AppError, AppResult};
use crate::spi_rasters::{
    derive_spi_raster, fetch_chirps, list_spi_products, register_chirps_dir, ChirpsFetchOutcome,
    ChirpsFetchRequest, ChirpsFetcherHandle, ChirpsRegisterOutcome, HttpChirpsFetcher,
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

/// Blend two registered same-grid VCI + TCI drought products into a VHI L3.
pub async fn derive_vhi_raster_route(
    State(state): State<AppState>,
    Json(request): Json<VhiDeriveRequest>,
) -> AppResult<Json<VhiDeriveOutcome>> {
    let outcome = derive_vhi_raster(&state.pool, &state.config.data_root, &request).await?;
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

/// Download a CHIRPS archive slice and register it. Tests inject an
/// in-memory fetcher through the optional [`ChirpsFetcherHandle`] extension;
/// production falls back to plain HTTPS.
pub async fn fetch_chirps_route(
    State(state): State<AppState>,
    fetcher: Option<axum::extract::Extension<ChirpsFetcherHandle>>,
    Json(request): Json<ChirpsFetchRequest>,
) -> AppResult<Json<ChirpsFetchOutcome>> {
    let default_fetcher = HttpChirpsFetcher::default();
    let fetcher: &dyn crate::spi_rasters::ChirpsFetcher = match &fetcher {
        Some(axum::extract::Extension(ChirpsFetcherHandle(inner))) => inner.as_ref(),
        None => &default_fetcher,
    };
    let outcome = fetch_chirps(&state.pool, &state.config.data_root, fetcher, &request).await?;
    Ok(Json(outcome))
}

pub async fn derive_spi_raster_route(
    State(state): State<AppState>,
    Json(request): Json<SpiDeriveRequest>,
) -> AppResult<Json<SpiDeriveOutcome>> {
    let outcome = derive_spi_raster(&state.pool, &state.config.data_root, &request).await?;
    Ok(Json(outcome))
}
