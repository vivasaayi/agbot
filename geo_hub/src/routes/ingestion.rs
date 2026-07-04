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
