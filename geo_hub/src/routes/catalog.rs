//! Catalog register/read route handlers (Layer 1 — the viewer's read API).
//!
//! Thin HTTP wrappers over `crate::catalog`: register a product draft and list /
//! fetch cataloged products with field/season/level/kind/scene/source/time/bbox
//! filters. The registry, identity, and lineage live in `crate::catalog`.

use super::normalize_optional_text;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use anyhow::Error;
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::Row;

/// Map a catalog registration error onto an HTTP error: graph-validation
/// failures are client errors, everything else is a server error.
fn catalog_error(err: crate::catalog::CatalogError) -> AppError {
    use crate::catalog::CatalogError;
    match err {
        CatalogError::InputNotFound { .. } | CatalogError::MaskNotRegistered { .. } => {
            AppError::BadRequest(err.to_string())
        }
        other => AppError::Anyhow(Error::new(other)),
    }
}

/// Query filter for `GET /api/catalog/products`. All set fields are ANDed.
#[derive(Debug, Default, Deserialize)]
pub struct CatalogProductsQuery {
    pub farm_id: Option<String>,
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub scene_id: Option<String>,
    pub source_id: Option<String>,
    pub level: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub temporal_start: Option<String>,
    pub temporal_end: Option<String>,
    /// `min_x,min_y,max_x,max_y`.
    pub bbox: Option<String>,
}

impl CatalogProductsQuery {
    fn into_filter(self) -> Result<crate::catalog::ProductFilter, AppError> {
        let level = match self
            .level
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(text) => Some(match text.to_ascii_lowercase().as_str() {
                "l0" => shared::product_graph::ProductLevel::L0,
                "l1" => shared::product_graph::ProductLevel::L1,
                "l2" => shared::product_graph::ProductLevel::L2,
                "l3" => shared::product_graph::ProductLevel::L3,
                other => {
                    return Err(AppError::BadRequest(format!("invalid level: {other}")));
                }
            }),
            None => None,
        };
        let bbox = match self
            .bbox
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(text) => {
                let parts: Vec<f64> = text
                    .split(',')
                    .map(|p| p.trim().parse::<f64>())
                    .collect::<Result<_, _>>()
                    .map_err(|_| {
                        AppError::BadRequest("bbox must be min_x,min_y,max_x,max_y".to_string())
                    })?;
                if parts.len() != 4 {
                    return Err(AppError::BadRequest(
                        "bbox must have 4 comma-separated numbers".to_string(),
                    ));
                }
                Some([parts[0], parts[1], parts[2], parts[3]])
            }
            None => None,
        };
        Ok(crate::catalog::ProductFilter {
            farm_id: normalize_optional_text(self.farm_id),
            field_id: normalize_optional_text(self.field_id),
            season_id: normalize_optional_text(self.season_id),
            scene_id: normalize_optional_text(self.scene_id),
            source_id: normalize_optional_text(self.source_id),
            level,
            kind: normalize_optional_text(self.kind),
            status: normalize_optional_text(self.status),
            temporal_start: normalize_optional_text(self.temporal_start),
            temporal_end: normalize_optional_text(self.temporal_end),
            bbox,
        })
    }
}

/// Register a product draft into the catalog (Track A batch 8). Producers POST a
/// `ProductRecordDraft`; returns the deterministic `product_id`.
pub async fn register_catalog_product(
    State(state): State<AppState>,
    Json(draft): Json<shared::product_graph::ProductRecordDraft>,
) -> AppResult<Json<serde_json::Value>> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let product_id = crate::catalog::register_product(&state.pool, &draft, &now)
        .await
        .map_err(catalog_error)?;
    Ok(Json(serde_json::json!({ "product_id": product_id })))
}

/// The catalog read API behind the workspace: list products by
/// field/season/level/kind/scene/source/time/bbox.
pub async fn list_catalog_products(
    State(state): State<AppState>,
    Query(query): Query<CatalogProductsQuery>,
) -> AppResult<Json<Vec<crate::catalog::RegisteredProduct>>> {
    let filter = query.into_filter()?;
    let products = crate::catalog::list_products(&state.pool, &filter)
        .await
        .map_err(catalog_error)?;
    Ok(Json(products))
}

/// Query filter for `GET /api/catalog/sources`.
#[derive(Debug, Default, Deserialize)]
pub struct CatalogSourcesQuery {
    pub status: Option<String>,
    pub source_kind: Option<String>,
}

/// A registered ingestion source (read view over `catalog_sources`). This is
/// the missing read side of the source registry — until now the table was
/// write-only from the ingest contract.
#[derive(Debug, Serialize)]
pub struct CatalogSource {
    pub source_id: String,
    pub source_kind: String,
    pub platform: Option<String>,
    pub sensor: Option<String>,
    pub status: String,
    pub registered_at: String,
    /// Parsed `config_json` if present and valid JSON, otherwise null.
    pub config: Option<serde_json::Value>,
}

/// List registered ingestion sources, most-recently-registered first.
/// Optional `status` and `source_kind` filters are ANDed.
pub async fn list_catalog_sources(
    State(state): State<AppState>,
    Query(query): Query<CatalogSourcesQuery>,
) -> AppResult<Json<Vec<CatalogSource>>> {
    let status = normalize_optional_text(query.status);
    let source_kind = normalize_optional_text(query.source_kind);

    let rows = sqlx::query(
        r#"
        SELECT source_id, source_kind, platform, sensor, config_json, status, registered_at
        FROM catalog_sources
        WHERE (?1 IS NULL OR status = ?1)
          AND (?2 IS NULL OR source_kind = ?2)
        ORDER BY registered_at DESC, source_id ASC
        "#,
    )
    .bind(&status)
    .bind(&source_kind)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let sources = rows
        .iter()
        .map(|row| {
            let config = row
                .get::<Option<String>, _>("config_json")
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
            CatalogSource {
                source_id: row.get("source_id"),
                source_kind: row.get("source_kind"),
                platform: row.get("platform"),
                sensor: row.get("sensor"),
                status: row.get("status"),
                registered_at: row.get("registered_at"),
                config,
            }
        })
        .collect();

    Ok(Json(sources))
}

/// Fetch a single catalog product by id.
pub async fn get_catalog_product(
    Path(product_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<crate::catalog::RegisteredProduct>> {
    let product = crate::catalog::get_product(&state.pool, &product_id)
        .await
        .map_err(catalog_error)?
        .ok_or(AppError::NotFound)?;
    Ok(Json(product))
}
