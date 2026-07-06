//! LST route handlers (`/api/thermal/lst/...`).
//!
//! Thin wrapper over `crate::lst_rasters`: a thermal DN GeoTIFF plus its
//! radiometric calibration registers as an `lst` L2 catalog product, which
//! the drought raster path scores into TCI (and, blended, VHI).

use axum::extract::State;
use axum::Json;

use crate::error::{AppError, AppResult};
use crate::lst_rasters::{derive_lst_raster, LstDeriveOutcome, LstDeriveRequest, LstRasterError};
use crate::state::AppState;

impl From<LstRasterError> for AppError {
    fn from(err: LstRasterError) -> Self {
        match &err {
            LstRasterError::NdviNotFound(_) => AppError::NotFound,
            _ if err.is_client_error() => AppError::BadRequest(err.to_string()),
            _ => AppError::Anyhow(err.into()),
        }
    }
}

impl From<crate::et_rasters::EtRasterError> for AppError {
    fn from(err: crate::et_rasters::EtRasterError) -> Self {
        match &err {
            crate::et_rasters::EtRasterError::NotFound(_) => AppError::NotFound,
            _ if err.is_client_error() => AppError::BadRequest(err.to_string()),
            _ => AppError::Anyhow(err.into()),
        }
    }
}

/// Derive the Ts-VI triangle ET fraction from a same-grid lst + ndvi pair
/// (batch 40): the demand side of water availability, evaporative fraction
/// in [0, 1] with self-calibrated edges in the evidence.
pub async fn derive_et_fraction_route(
    State(state): State<AppState>,
    Json(request): Json<crate::et_rasters::EtDeriveRequest>,
) -> AppResult<Json<crate::et_rasters::EtDeriveOutcome>> {
    let outcome =
        crate::et_rasters::derive_et_fraction(&state.pool, &state.config.data_root, &request)
            .await?;
    Ok(Json(outcome))
}

pub async fn derive_lst_route(
    State(state): State<AppState>,
    Json(request): Json<LstDeriveRequest>,
) -> AppResult<Json<LstDeriveOutcome>> {
    let outcome = derive_lst_raster(&state.pool, &state.config.data_root, &request).await?;
    Ok(Json(outcome))
}
