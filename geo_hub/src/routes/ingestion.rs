//! Source-ingestion route handlers (Layer 1).
//!
//! Thin HTTP wrappers over the ingest health view (`crate::ingest`) and the
//! normalized drone-session ingest contract (`crate::ingest_contract`). Named
//! `ingestion` (not `ingest`) to avoid colliding with the `crate::ingest` import
//! used throughout the parent module.

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use anyhow::Error;
use axum::extract::State;
use axum::Json;

/// The scene-ingest health view (retry/backoff state of the ingest pipeline).
pub async fn get_ingest_health(
    State(state): State<AppState>,
) -> AppResult<Json<crate::ingest::SceneIngestHealth>> {
    Ok(Json(crate::ingest::load_ingest_health(&state.pool).await?))
}

/// Ingest a completed drone capture session (Track A batch 6): validate the
/// manifest's integrity checksums, reject an already-ingested session, and
/// commit the scene + L0 capture products into the catalog with lineage.
pub async fn ingest_drone_session(
    State(state): State<AppState>,
    Json(manifest): Json<shared::drone_ingest::DroneIngestManifest>,
) -> AppResult<Json<crate::ingest_contract::IngestReceipt>> {
    use crate::ingest_contract::IngestError;
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let actor = provenance::ActorIdentity::system("geo_hub:drone-ingest");
    let receipt = crate::ingest_contract::commit_drone_ingest(&state.pool, &manifest, &actor, &now)
        .await
        .map_err(|err| match err {
            IngestError::InvalidManifest(_)
            | IngestError::DuplicateSession(_)
            | IngestError::InvalidSourceKind(_) => AppError::BadRequest(err.to_string()),
            other => AppError::Anyhow(Error::new(other)),
        })?;
    Ok(Json(receipt))
}

/// HLS error mapping: registration failures are server-side (bad directory,
/// unreadable rasters) unless a specific granule grid mismatch, which is
/// the caller's data problem.
impl From<crate::hls::HlsError> for AppError {
    fn from(err: crate::hls::HlsError) -> Self {
        match &err {
            crate::hls::HlsError::GridMismatch { .. } | crate::hls::HlsError::Index(_) => {
                AppError::BadRequest(err.to_string())
            }
            _ => AppError::Anyhow(err.into()),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct HlsRegisterRequest {
    /// Server-local directory holding downloaded HLS band GeoTIFFs.
    pub dir: String,
}

/// Register a directory of HLS v2.0 band GeoTIFFs as harmonized `ndvi` L2
/// products (batch 19): both HLSL30 and HLSS30 granules land on one grid and
/// feed a single densified NDVI time series.
pub async fn register_hls(
    State(state): State<AppState>,
    Json(request): Json<HlsRegisterRequest>,
) -> AppResult<Json<crate::hls::HlsRegisterOutcome>> {
    let outcome =
        crate::hls::register_hls_dir(&state.pool, std::path::Path::new(&request.dir)).await?;
    Ok(Json(outcome))
}

impl From<crate::sen2cor_derive::Sen2CorDeriveError> for AppError {
    fn from(err: crate::sen2cor_derive::Sen2CorDeriveError) -> Self {
        match &err {
            crate::sen2cor_derive::Sen2CorDeriveError::BandNotFound { .. } => AppError::NotFound,
            _ if err.is_client_error() => AppError::BadRequest(err.to_string()),
            _ => AppError::Anyhow(err.into()),
        }
    }
}

/// Derive NDVI locally from a registered Sen2Cor scene's 10 m JP2 bands
/// (batch 24): decode red/NIR via `raster_io`'s JP2 reader, calibrate to
/// reflectance, register an `ndvi` L2 with lineage to both band products.
pub async fn derive_sen2cor_ndvi_route(
    State(state): State<AppState>,
    Json(request): Json<crate::sen2cor_derive::Sen2CorNdviRequest>,
) -> AppResult<Json<crate::sen2cor_derive::Sen2CorNdviOutcome>> {
    let outcome =
        crate::sen2cor_derive::derive_sen2cor_ndvi(&state.pool, &state.config.data_root, &request)
            .await?;
    Ok(Json(outcome))
}
