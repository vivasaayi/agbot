//! MODIS MOD13Q1 NDVI ingest route (`POST /api/fields/:field_id/modis/ingest`).
//!
//! Thin HTTP wrapper over [`crate::modis::ingest_modis_ndvi_for_field`]: a
//! manual trigger (v1) that STAC-searches MOD13Q1 v061 over the field for a
//! `{start, end}` date window, registers each intersecting tile as an external
//! L3 catalog product, and appends the five NDVI zonal statistics to the field
//! time series under the shared `sat.ndvi.*` namespace (source `"modis"`).
//!
//! The ingest runs synchronously within the request (a field-scale AOI reads
//! one coarse MODIS tile). A pipeline job kind can wrap the same domain
//! function later without touching the pipeline worker.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::modis::{
    ingest_modis_ndvi_for_field, ModisError, ModisIngestOutcome, PLANETARY_COMPUTER_STAC_SEARCH,
};
use crate::pc_sign::PcSasTokenCache;
use crate::state::AppState;

impl From<ModisError> for AppError {
    fn from(err: ModisError) -> Self {
        match err {
            ModisError::FieldNotFound(_) => AppError::NotFound,
            ModisError::InvalidDateRange(_) | ModisError::InvalidBoundary { .. } => {
                AppError::BadRequest(err.to_string())
            }
            other => AppError::Anyhow(other.into()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ModisIngestBody {
    /// Inclusive `YYYY-MM-DD` start of the search window.
    pub start: String,
    /// Inclusive `YYYY-MM-DD` end of the search window.
    pub end: String,
}

/// `POST /api/fields/:field_id/modis/ingest` with body `{ "start", "end" }`.
///
/// Registers external L3 MODIS NDVI products and appends field-scoped NDVI
/// zonal statistics. Idempotent: re-running the same window appends no new
/// points. Returns [`ModisIngestOutcome`].
pub async fn ingest_field_modis(
    State(state): State<AppState>,
    Path(field_id): Path<String>,
    Json(body): Json<ModisIngestBody>,
) -> AppResult<Json<ModisIngestOutcome>> {
    let cache = Arc::new(PcSasTokenCache::new().map_err(|err| AppError::Anyhow(err.into()))?);
    let outcome = ingest_modis_ndvi_for_field(
        &state.pool,
        &cache,
        &field_id,
        body.start.trim(),
        body.end.trim(),
        PLANETARY_COMPUTER_STAC_SEARCH,
    )
    .await?;
    Ok(Json(outcome))
}
