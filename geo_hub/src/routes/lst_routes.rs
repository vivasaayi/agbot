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

pub async fn derive_lst_route(
    State(state): State<AppState>,
    Json(request): Json<LstDeriveRequest>,
) -> AppResult<Json<LstDeriveOutcome>> {
    let outcome = derive_lst_raster(&state.pool, &state.config.data_root, &request).await?;
    Ok(Json(outcome))
}
