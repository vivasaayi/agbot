//! Global Web Mercator XYZ tile route for catalog raster products
//! (`GET /api/catalog/products/:product_id/tiles/:z/:x/:y.png`).
//!
//! Unlike the scene-local `/api/scenes/.../tiles/...` route (which splits a
//! pre-rendered PNG in its own pixel space), these are true slippy-map tiles:
//! MapLibre/Leaflet can consume the template directly as a `raster` source.
//! Rendering (projection chain + colormap) lives in `crate::product_tiler`;
//! this file resolves the product artifact, runs the render on the blocking
//! pool, and disk-caches encoded tiles under the content-addressed product id.

use std::path::PathBuf;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};
use tokio::fs;

use crate::catalog;
use crate::error::{AppError, AppResult};
use crate::product_tiler::{
    colormap_for_kind, encode_tile_png, load_tile_source, render_web_tile, TileError,
};
use crate::state::AppState;

impl From<TileError> for AppError {
    fn from(err: TileError) -> Self {
        match err {
            TileError::ZoomTooDeep(_) | TileError::TileOutOfRange { .. } => {
                AppError::BadRequest(err.to_string())
            }
            // The product exists but is not web-tileable as stored: surface
            // the reason code to the caller instead of a blank 500.
            TileError::MissingEpsg
            | TileError::UnsupportedCrs(_)
            | TileError::MissingGeotransform
            | TileError::RotatedGrid => AppError::BadRequest(err.to_string()),
            TileError::Raster(_) | TileError::PngEncode(_) => AppError::Anyhow(err.into()),
        }
    }
}

/// Cache directory component for a product id: filesystem-safe prefix plus a
/// short digest of the full id so distinct ids never collide after
/// sanitization. Product ids are content-addressed, so the id alone is a
/// complete cache fingerprint.
fn product_cache_component(product_id: &str) -> String {
    let sanitized: String = product_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let digest = format!("{:x}", Sha256::digest(product_id.as_bytes()));
    format!("{}-{}", sanitized.trim_matches('_'), &digest[..12])
}

pub async fn catalog_product_web_tile(
    Path((product_id, z, x, y_segment)): Path<(String, u8, u32, String)>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let y = y_segment
        .strip_suffix(".png")
        .ok_or_else(|| AppError::BadRequest("tile requests must end with .png".to_string()))?
        .parse::<u32>()
        .map_err(|_| AppError::BadRequest("invalid tile y coordinate".to_string()))?;

    let product = catalog::get_product(&state.pool, &product_id)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
        .ok_or(AppError::NotFound)?;
    let artifact_path = product.path.as_deref().ok_or_else(|| {
        AppError::BadRequest(format!(
            "product {product_id} has no artifact to tile (levels without a stored raster cannot be rendered)"
        ))
    })?;
    let is_geotiff = std::path::Path::new(artifact_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("tif") || ext.eq_ignore_ascii_case("tiff"))
        .unwrap_or(false);
    if !is_geotiff {
        return Err(AppError::BadRequest(format!(
            "product {product_id} artifact is not a GeoTIFF (format {:?}); only GeoTIFF products are web-tileable",
            product.format
        )));
    }

    let tile_path = state
        .config
        .data_root
        .join("tile_cache")
        .join("catalog")
        .join(product_cache_component(&product_id))
        .join(z.to_string())
        .join(x.to_string())
        .join(format!("{y}.png"));

    if !fs::try_exists(&tile_path)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        let source_path = PathBuf::from(artifact_path);
        let kind = product.kind.clone();
        let tile_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, TileError> {
            let source = load_tile_source(&source_path)?;
            let tile = render_web_tile(&source, &colormap_for_kind(&kind), z, x, y)?;
            encode_tile_png(&tile)
        })
        .await
        .map_err(|err| AppError::Anyhow(err.into()))??;

        if let Some(parent) = tile_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|err| AppError::Anyhow(err.into()))?;
        }
        fs::write(&tile_path, tile_bytes)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
    }

    let bytes = fs::read(&tile_path)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400, immutable"),
    );
    Ok((headers, Body::from(bytes)).into_response())
}
