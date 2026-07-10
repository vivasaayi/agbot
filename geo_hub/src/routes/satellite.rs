//! Satellite derivation route (`POST /api/satellite/derive`, batch 6).
//!
//! Thin HTTP wrapper over `crate::satellite_derivation`: decodes the request
//! (inline Earth Search STAC item or `collection` + `item_id` to fetch live),
//! runs the derivation **synchronously within the request** (documented
//! choice: a field-scale AOI reads a handful of COG tiles and completes in
//! seconds; the `*_run.rs` application-run pattern is for multi-scene jobs
//! and can wrap this later), and returns the registered product references.
//!
//! Tests inject an in-memory COG store through the optional
//! [`SatelliteCogResolver`] request extension; production falls back to
//! [`UrlCogResolver`] (plain HTTPS range reads).

use crate::earth_search::{self, EarthSearchItem};
use crate::satellite_derivation::{
    derive_satellite_index, index_kind_from_key, DerivationError, DeriveRequest,
    SatelliteCogResolver, UrlCogResolver,
};
use crate::state::AppState;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use shared::schemas::GeoBounds;

#[derive(Debug, Deserialize)]
pub struct SatelliteDeriveBody {
    /// Inline Earth Search STAC item (preferred: no network round trip).
    #[serde(default)]
    pub item: Option<EarthSearchItem>,
    /// Alternative to `item`: fetch the item live from Earth Search.
    #[serde(default)]
    pub collection: Option<String>,
    #[serde(default)]
    pub item_id: Option<String>,
    /// WGS84 `[min_lon, min_lat, max_lon, max_lat]`.
    pub aoi: [f64; 4],
    /// Index kind key, e.g. `ndvi`, `mndwi`, `ndmi`.
    pub index: String,
    /// Optional field scope threaded into every registered product so the
    /// per-field time series can query the catalog by field.
    #[serde(default)]
    pub field_id: Option<String>,
    /// Optional season scope, alongside `field_id`.
    #[serde(default)]
    pub season_id: Option<String>,
}

pub struct SatelliteDeriveError {
    status: StatusCode,
    code: String,
    message: String,
}

impl SatelliteDeriveError {
    fn bad_request(code: &str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: code.to_string(),
            message: message.into(),
        }
    }
}

impl From<DerivationError> for SatelliteDeriveError {
    fn from(err: DerivationError) -> Self {
        let status = if err.is_client_error() {
            StatusCode::UNPROCESSABLE_ENTITY
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        let code = match &err {
            DerivationError::UnsupportedDataset(_) => "unsupported_dataset",
            DerivationError::UnknownIndexKind(_) => "unknown_index_kind",
            DerivationError::MissingAsset { .. } => "missing_asset",
            DerivationError::MissingEpsg(_) => "missing_epsg",
            DerivationError::MissingDatetime(_) => "missing_datetime",
            DerivationError::InvalidAoi(_) => "invalid_aoi",
            DerivationError::AoiOutsideScene { .. } => "aoi_outside_scene",
            DerivationError::Utm(_) => "utm_projection_failed",
            _ => "derivation_failed",
        };
        Self {
            status,
            code: code.to_string(),
            message: err.to_string(),
        }
    }
}

impl IntoResponse for SatelliteDeriveError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({
                "code": self.code,
                "description": self.message,
            })),
        )
            .into_response()
    }
}

/// `POST /api/satellite/derive`.
pub async fn satellite_derive(
    State(state): State<AppState>,
    resolver: Option<Extension<SatelliteCogResolver>>,
    Json(body): Json<SatelliteDeriveBody>,
) -> Result<Json<serde_json::Value>, SatelliteDeriveError> {
    let index = index_kind_from_key(&body.index)?;
    let [min_lon, min_lat, max_lon, max_lat] = body.aoi;
    let aoi = GeoBounds {
        min_lon,
        min_lat,
        max_lon,
        max_lat,
    };

    let item = match (
        body.item,
        body.collection.as_deref(),
        body.item_id.as_deref(),
    ) {
        (Some(item), _, _) => item,
        (None, Some(collection), Some(item_id)) => earth_search::fetch_item(collection, item_id)
            .await
            .map_err(|err| SatelliteDeriveError {
                status: StatusCode::BAD_GATEWAY,
                code: "earth_search_fetch_failed".to_string(),
                message: err.to_string(),
            })?,
        _ => {
            return Err(SatelliteDeriveError::bad_request(
                "missing_item",
                "provide either an inline STAC `item` or both `collection` and `item_id`",
            ))
        }
    };

    let request = DeriveRequest {
        item,
        aoi,
        index,
        field_id: body.field_id,
        season_id: body.season_id,
    };
    let default_resolver = UrlCogResolver;
    let resolver_ref: &dyn crate::satellite_derivation::CogStoreResolver = match &resolver {
        Some(Extension(SatelliteCogResolver(inner))) => inner.as_ref(),
        None => &default_resolver,
    };
    let outcome =
        derive_satellite_index(&state.pool, &state.config.data_root, resolver_ref, &request)
            .await?;

    Ok(Json(serde_json::json!({
        "product_id": outcome.product_id,
        "scene_id": outcome.scene_id,
        "collection": outcome.collection,
        "index": outcome.index_kind,
        "product_path": outcome.product_path,
        "stac_item_href": outcome.stac_item_href,
        "width_px": outcome.width,
        "height_px": outcome.height,
        "valid_pixels": outcome.valid_pixels,
        "invalid_pixels": outcome.invalid_pixels,
        "execution": "synchronous",
        "evidence": outcome.evidence,
    })))
}
