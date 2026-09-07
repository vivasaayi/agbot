//! Internal STAC catalog over the geo_hub product graph (satellite pipeline
//! batch 4, design doc Phase 2 item 6).
//!
//! Exposes the existing `catalog_products` graph as a read-only STAC API
//! (spec 1.0.0 core + collections + item-search; STAC objects 1.1.0).
//!
//! # Dependency decision
//! STAC objects are hand-rolled serde structs rather than the `stac`/`rustac`
//! crates: `stac-server` 0.x targets axum 0.8 while geo_hub is on axum 0.7,
//! and STAC is only a JSON convention — plain structs keep us dependency-light
//! and let items map 1:1 from [`RegisteredProduct`] rows. Revisit if we ever
//! need STAC extensions with real schema validation.
//!
//! # Collection model
//! Collections are derived dynamically from what is actually registered:
//! - `scenes` — every L0/L1 product (source scenes and raw bands);
//! - one collection per distinct `kind` among L2/L3 derived products
//!   (e.g. `ndvi`, `temporal_composite`).
//!
//! This maps directly onto the existing catalog query axes (`level`, `kind`)
//! so listing a collection's items is a single [`catalog::list_products`]
//! call, and the collection set always reflects the DB without a migration.
//! A derived product literally named `scenes` would collide with the source
//! collection; `collection_id_for` namespaces such a kind as `derived-scenes`.
//!
//! # Item validity
//! - A record with no parseable `temporal_start` is STAC-invalid and skipped
//!   (`properties.datetime` is mandatory); listings surface the skip count.
//! - A record with no stored bbox is skipped (no spatial evidence at all).
//! - A record with a bbox in a non-geographic CRS (e.g. a UTM code) is kept
//!   with `geometry: null` and no `bbox`, and the reason is recorded in
//!   `properties["agbot:geometry_omitted_reason"]` — the stored corner values
//!   are not lon/lat so emitting them as WGS84 would be a lie.
//!
//! Links use server-relative hrefs (`/api/stac/...`): geo_hub has no
//! configured public base URL, and every internal consumer resolves against
//! the host it queried.

use crate::catalog::{self, CatalogError, ProductFilter, RegisteredProduct};
use crate::db::DbPool;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use shared::product_graph::ProductLevel;
use sqlx::Row;
use std::collections::BTreeMap;
use thiserror::Error;

pub const STAC_VERSION: &str = "1.1.0";
pub const STAC_API_ROOT: &str = "/api/stac";
pub const CATALOG_ID: &str = "agbot-geo-hub";
pub const DEFAULT_LIMIT: usize = 10;
pub const MAX_LIMIT: usize = 100;

/// Conformance classes implemented by this API.
pub const CONFORMANCE_CLASSES: [&str; 4] = [
    "https://api.stacspec.org/v1.0.0/core",
    "https://api.stacspec.org/v1.0.0/collections",
    "https://api.stacspec.org/v1.0.0/ogcapi-features",
    "https://api.stacspec.org/v1.0.0/item-search",
];

/// Failure modes of the STAC layer. `code` mirrors the STAC API error-object
/// convention (`{code, description}`).
#[derive(Debug, Error)]
pub enum StacError {
    #[error("collection {0} not found")]
    CollectionNotFound(String),
    #[error("item {item_id} not found in collection {collection_id}")]
    ItemNotFound {
        collection_id: String,
        item_id: String,
    },
    #[error("invalid datetime parameter: {0}")]
    InvalidDatetime(String),
    #[error("invalid bbox parameter: {0}")]
    InvalidBbox(String),
    #[error("invalid limit parameter: {0}")]
    InvalidLimit(String),
    #[error("invalid token parameter: {0}")]
    InvalidToken(String),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

impl StacError {
    /// STAC error-object code for the typed HTTP error body.
    pub fn code(&self) -> &'static str {
        match self {
            StacError::CollectionNotFound(_) => "CollectionNotFound",
            StacError::ItemNotFound { .. } => "ItemNotFound",
            StacError::InvalidDatetime(_) => "InvalidDatetime",
            StacError::InvalidBbox(_) => "InvalidBbox",
            StacError::InvalidLimit(_) => "InvalidLimit",
            StacError::InvalidToken(_) => "InvalidToken",
            StacError::Catalog(_) => "InternalError",
        }
    }
}

// --- STAC object types -------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StacLink {
    pub rel: String,
    pub href: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// HTTP method for the link target (STAC API pagination extension).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Body merge for POST pagination links.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

impl StacLink {
    pub fn new(rel: &str, href: String) -> Self {
        Self {
            rel: rel.to_string(),
            href,
            media_type: Some("application/json".to_string()),
            title: None,
            method: None,
            body: None,
        }
    }

    pub fn geojson(rel: &str, href: String) -> Self {
        Self {
            media_type: Some("application/geo+json".to_string()),
            ..Self::new(rel, href)
        }
    }
}

/// STAC landing page (`type: Catalog`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StacCatalog {
    #[serde(rename = "type")]
    pub type_: String,
    pub stac_version: String,
    pub id: String,
    pub description: String,
    #[serde(rename = "conformsTo")]
    pub conforms_to: Vec<String>,
    pub links: Vec<StacLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StacSpatialExtent {
    pub bbox: Vec<[f64; 4]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StacTemporalExtent {
    pub interval: Vec<[Option<String>; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StacExtent {
    pub spatial: StacSpatialExtent,
    pub temporal: StacTemporalExtent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StacCollection {
    #[serde(rename = "type")]
    pub type_: String,
    pub stac_version: String,
    pub id: String,
    pub description: String,
    pub license: String,
    pub extent: StacExtent,
    pub links: Vec<StacLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StacAsset {
    pub href: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StacItem {
    #[serde(rename = "type")]
    pub type_: String,
    pub stac_version: String,
    pub id: String,
    pub collection: String,
    pub geometry: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<[f64; 4]>,
    pub properties: serde_json::Map<String, serde_json::Value>,
    pub links: Vec<StacLink>,
    pub assets: BTreeMap<String, StacAsset>,
}

/// `GET .../items` and search response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StacFeatureCollection {
    #[serde(rename = "type")]
    pub type_: String,
    pub features: Vec<StacItem>,
    pub links: Vec<StacLink>,
    #[serde(rename = "numberReturned")]
    pub number_returned: usize,
    /// Records that could not be expressed as STAC items (no timestamp / no
    /// spatial evidence) and were skipped rather than crashing the listing.
    #[serde(rename = "agbot:skipped")]
    pub skipped: usize,
}

// --- Collection model --------------------------------------------------------

/// The reserved source-scene collection (all L0/L1 products).
pub const SCENES_COLLECTION_ID: &str = "scenes";

/// Which collection a product belongs to. L0/L1 products are source scenes;
/// L2/L3 products group by kind. A derived kind literally named `scenes` is
/// namespaced to avoid colliding with the source collection.
pub fn collection_id_for(level: ProductLevel, kind: &str) -> String {
    match level {
        ProductLevel::L0 | ProductLevel::L1 => SCENES_COLLECTION_ID.to_string(),
        ProductLevel::L2 | ProductLevel::L3 => {
            if kind == SCENES_COLLECTION_ID {
                format!("derived-{kind}")
            } else {
                kind.to_string()
            }
        }
    }
}

/// Catalog filter selecting exactly the members of `collection_id`, or `None`
/// if the id can never match (derived collections map back to a kind).
fn collection_member_filters(collection_id: &str) -> Vec<ProductFilter> {
    if collection_id == SCENES_COLLECTION_ID {
        return vec![
            ProductFilter {
                level: Some(ProductLevel::L0),
                ..Default::default()
            },
            ProductFilter {
                level: Some(ProductLevel::L1),
                ..Default::default()
            },
        ];
    }
    let kind = collection_id
        .strip_prefix("derived-")
        .filter(|rest| *rest == SCENES_COLLECTION_ID)
        .unwrap_or(collection_id);
    [ProductLevel::L2, ProductLevel::L3]
        .into_iter()
        .map(|level| ProductFilter {
            level: Some(level),
            kind: Some(kind.to_string()),
            ..Default::default()
        })
        .collect()
}

/// Aggregate summary of one collection as derived from the DB.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionSummary {
    pub id: String,
    pub item_count: i64,
    pub bbox: Option<[f64; 4]>,
    pub temporal_start: Option<String>,
    pub temporal_end: Option<String>,
}

/// Derive the collection set with spatial/temporal extents in one grouped
/// query over `catalog_products`.
pub async fn collection_summaries(pool: &DbPool) -> Result<Vec<CollectionSummary>, StacError> {
    let rows = sqlx::query(
        r#"
        SELECT
            CASE
                WHEN level IN ('l0', 'l1') THEN 'scenes'
                WHEN kind = 'scenes' THEN 'derived-scenes'
                ELSE kind
            END AS collection_id,
            COUNT(*) AS item_count,
            MIN(bbox_min_x) AS min_x,
            MIN(bbox_min_y) AS min_y,
            MAX(bbox_max_x) AS max_x,
            MAX(bbox_max_y) AS max_y,
            MIN(temporal_start) AS temporal_start,
            MAX(temporal_end) AS temporal_end
        FROM catalog_products
        GROUP BY collection_id
        ORDER BY collection_id ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(CatalogError::from)?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let bbox = match (
                row.get::<Option<f64>, _>("min_x"),
                row.get::<Option<f64>, _>("min_y"),
                row.get::<Option<f64>, _>("max_x"),
                row.get::<Option<f64>, _>("max_y"),
            ) {
                (Some(a), Some(b), Some(c), Some(d)) => Some([a, b, c, d]),
                _ => None,
            };
            CollectionSummary {
                id: row.get("collection_id"),
                item_count: row.get("item_count"),
                bbox,
                temporal_start: row.get("temporal_start"),
                temporal_end: row.get("temporal_end"),
            }
        })
        .collect())
}

/// Render a collection summary as a STAC Collection object.
pub fn summary_to_collection(summary: &CollectionSummary) -> StacCollection {
    let description = if summary.id == SCENES_COLLECTION_ID {
        "Source scenes and raw bands (product levels L0/L1)".to_string()
    } else {
        format!("Derived products of kind '{}' (levels L2/L3)", summary.id)
    };
    let collection_href = format!("{STAC_API_ROOT}/collections/{}", summary.id);
    StacCollection {
        type_: "Collection".to_string(),
        stac_version: STAC_VERSION.to_string(),
        id: summary.id.clone(),
        description,
        license: "proprietary".to_string(),
        extent: StacExtent {
            spatial: StacSpatialExtent {
                bbox: summary.bbox.into_iter().collect(),
            },
            temporal: StacTemporalExtent {
                interval: vec![[summary.temporal_start.clone(), summary.temporal_end.clone()]],
            },
        },
        links: vec![
            StacLink::new("self", collection_href.clone()),
            StacLink::new("root", STAC_API_ROOT.to_string()),
            StacLink::new("parent", STAC_API_ROOT.to_string()),
            StacLink::geojson("items", format!("{collection_href}/items")),
        ],
    }
}

/// Build the landing page (`GET /api/stac`).
pub fn landing_page() -> StacCatalog {
    StacCatalog {
        type_: "Catalog".to_string(),
        stac_version: STAC_VERSION.to_string(),
        id: CATALOG_ID.to_string(),
        description: "AGBot geo_hub internal STAC catalog over the product graph (scenes, \
                      derived indices, composites)"
            .to_string(),
        conforms_to: CONFORMANCE_CLASSES.iter().map(|s| s.to_string()).collect(),
        links: vec![
            StacLink::new("self", STAC_API_ROOT.to_string()),
            StacLink::new("root", STAC_API_ROOT.to_string()),
            StacLink::new("conformance", format!("{STAC_API_ROOT}/conformance")),
            StacLink::new("data", format!("{STAC_API_ROOT}/collections")),
            StacLink::geojson("search", format!("{STAC_API_ROOT}/search")),
        ],
    }
}

// --- Item mapping -------------------------------------------------------------

/// Why a catalog record could not be expressed as a STAC item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemSkipReason {
    /// `temporal_start` missing or not RFC3339 — `properties.datetime` is
    /// mandatory in STAC.
    MissingOrInvalidTimestamp,
    /// No stored bbox at all: no spatial evidence to publish.
    MissingSpatialRef,
}

/// One lineage input edge, resolved to the input's collection when the input
/// product is still registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemInputEdge {
    pub input_product_id: String,
    pub role: String,
    /// Collection of the input product, when resolvable.
    pub collection_id: Option<String>,
}

/// Is `crs` a geographic (lon/lat) CRS we can emit as GeoJSON WGS84? A missing
/// CRS is treated as geographic: `GeoBounds` fields are named `min_lon` /
/// `min_lat`, so unlabeled catalog bboxes are lon/lat by contract.
fn crs_is_geographic(crs: Option<&str>) -> bool {
    match crs {
        None => true,
        Some(text) => {
            let normalized = text.trim().to_ascii_uppercase().replace(' ', "");
            normalized.is_empty()
                || normalized == "EPSG:4326"
                || normalized == "OGC:CRS84"
                || normalized == "CRS84"
                || normalized == "WGS84"
        }
    }
}

/// Normalize a stored/queried timestamp to RFC3339 UTC (`Z` suffix).
fn normalize_rfc3339(value: &str) -> Option<String> {
    let parsed = DateTime::parse_from_rfc3339(value.trim()).ok()?;
    Some(
        parsed
            .with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Secs, true),
    )
}

/// GeoJSON Polygon from a lon/lat bbox: exterior ring, counterclockwise
/// winding (RFC 7946), closed.
fn bbox_polygon(bbox: &[f64; 4]) -> serde_json::Value {
    let [min_x, min_y, max_x, max_y] = *bbox;
    serde_json::json!({
        "type": "Polygon",
        "coordinates": [[
            [min_x, min_y],
            [max_x, min_y],
            [max_x, max_y],
            [min_x, max_y],
            [min_x, min_y],
        ]],
    })
}

fn media_type_for_format(format: Option<&str>) -> Option<String> {
    let format = format?.trim().to_ascii_lowercase();
    let media = match format.as_str() {
        "tif" | "tiff" | "geotiff" | "cog" => "image/tiff; application=geotiff",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "json" | "geojson" => "application/json",
        "" => return None,
        _ => "application/octet-stream",
    };
    Some(media.to_string())
}

/// Map one catalog product row (plus its resolved lineage edges) onto a STAC
/// Item, or explain why it cannot be one.
pub fn product_to_item(
    product: &RegisteredProduct,
    inputs: &[ItemInputEdge],
) -> Result<StacItem, ItemSkipReason> {
    let datetime = product
        .temporal_start
        .as_deref()
        .and_then(normalize_rfc3339)
        .ok_or(ItemSkipReason::MissingOrInvalidTimestamp)?;
    let stored_bbox = product.bbox.ok_or(ItemSkipReason::MissingSpatialRef)?;

    let geographic = crs_is_geographic(product.crs.as_deref());
    let (geometry, bbox, geometry_omitted_reason) = if geographic {
        (Some(bbox_polygon(&stored_bbox)), Some(stored_bbox), None)
    } else {
        // The stored corners are projected coordinates (e.g. UTM meters);
        // publishing them as WGS84 lon/lat would be wrong. Geometry stays
        // null and the bbox is omitted, with the reason on the item.
        (
            None,
            None,
            Some(format!(
                "stored bbox is in non-geographic CRS {}; no reprojection available",
                product.crs.as_deref().unwrap_or("unknown")
            )),
        )
    };

    let mut properties = serde_json::Map::new();
    properties.insert("datetime".to_string(), serde_json::json!(datetime));
    if let Some(end) = product.temporal_end.as_deref().and_then(normalize_rfc3339) {
        if end != datetime {
            properties.insert(
                "start_datetime".to_string(),
                serde_json::json!(datetime.clone()),
            );
            properties.insert("end_datetime".to_string(), serde_json::json!(end));
        }
    }
    if let Some(crs) = product
        .crs
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        properties.insert("proj:code".to_string(), serde_json::json!(crs));
    }
    properties.insert(
        "processing:level".to_string(),
        serde_json::json!(product.level.as_str().to_ascii_uppercase()),
    );
    properties.insert(
        "agbot:product_kind".to_string(),
        serde_json::json!(product.kind),
    );
    if let Some(scene_id) = &product.scene_id {
        properties.insert("agbot:scene_id".to_string(), serde_json::json!(scene_id));
    }
    if let Some(gsd) = product.gsd_m_per_px {
        properties.insert("gsd".to_string(), serde_json::json!(gsd));
    }
    if let Some(reason) = geometry_omitted_reason {
        properties.insert(
            "agbot:geometry_omitted_reason".to_string(),
            serde_json::json!(reason),
        );
    }

    let collection_id = collection_id_for(product.level, &product.kind);
    let collection_href = format!("{STAC_API_ROOT}/collections/{collection_id}");
    let mut links = vec![
        StacLink::geojson(
            "self",
            format!("{collection_href}/items/{}", product.product_id),
        ),
        StacLink::new("collection", collection_href.clone()),
        StacLink::new("parent", collection_href),
        StacLink::new("root", STAC_API_ROOT.to_string()),
    ];
    for edge in inputs {
        // Lineage: point at the input's STAC item when its collection is
        // known, else fall back to the catalog product endpoint.
        let href = match &edge.collection_id {
            Some(collection) => format!(
                "{STAC_API_ROOT}/collections/{collection}/items/{}",
                edge.input_product_id
            ),
            None => format!("/api/catalog/products/{}", edge.input_product_id),
        };
        let mut link = StacLink::new("derived_from", href);
        link.title = Some(edge.role.clone());
        links.push(link);
    }

    let mut assets = BTreeMap::new();
    let media_type = media_type_for_format(product.format.as_deref());
    match (&product.scene_id, &product.path) {
        (Some(scene_id), _) => {
            assets.insert(
                "data".to_string(),
                StacAsset {
                    href: format!("/api/scenes/{scene_id}/products/{}", product.kind),
                    media_type: media_type.clone(),
                    title: Some(format!("{} product", product.kind)),
                    roles: vec!["data".to_string()],
                },
            );
            assets.insert(
                "tiles".to_string(),
                StacAsset {
                    href: format!(
                        "/api/scenes/{scene_id}/products/{}/tiles/{{z}}/{{x}}/{{y}}.png",
                        product.kind
                    ),
                    media_type: Some("image/png".to_string()),
                    title: Some("XYZ tile template".to_string()),
                    roles: vec!["visual".to_string()],
                },
            );
        }
        (None, Some(path)) => {
            assets.insert(
                "data".to_string(),
                StacAsset {
                    href: path.clone(),
                    media_type,
                    title: Some(format!("{} artifact", product.kind)),
                    roles: vec!["data".to_string()],
                },
            );
        }
        (None, None) => {}
    }
    // GeoTIFF artifacts additionally get a true Web Mercator XYZ template
    // (the scene-local `tiles` asset splits the product image in its own
    // pixel space and cannot back a slippy-map raster source).
    let is_geotiff_artifact = product
        .path
        .as_deref()
        .map(|path| {
            let lower = path.to_ascii_lowercase();
            lower.ends_with(".tif") || lower.ends_with(".tiff")
        })
        .unwrap_or(false);
    if is_geotiff_artifact {
        assets.insert(
            "tiles_web".to_string(),
            StacAsset {
                href: format!(
                    "/api/catalog/products/{}/tiles/{{z}}/{{x}}/{{y}}.png",
                    product.product_id
                ),
                media_type: Some("image/png".to_string()),
                title: Some("Web Mercator XYZ tile template".to_string()),
                roles: vec!["visual".to_string()],
            },
        );
    }

    Ok(StacItem {
        type_: "Feature".to_string(),
        stac_version: STAC_VERSION.to_string(),
        id: product.product_id.clone(),
        collection: collection_id,
        geometry,
        bbox,
        properties,
        links,
        assets,
    })
}

/// Resolve lineage input edges (with the input product's collection) for a set
/// of products in one query.
pub async fn load_input_edges(
    pool: &DbPool,
    product_ids: &[String],
) -> Result<BTreeMap<String, Vec<ItemInputEdge>>, StacError> {
    let mut edges: BTreeMap<String, Vec<ItemInputEdge>> = BTreeMap::new();
    if product_ids.is_empty() {
        return Ok(edges);
    }
    let placeholders = vec!["?"; product_ids.len()].join(", ");
    let sql = format!(
        r#"
        SELECT e.product_id, e.input_product_id, e.role, p.level, p.kind
        FROM catalog_product_inputs e
        LEFT JOIN catalog_products p ON p.product_id = e.input_product_id
        WHERE e.product_id IN ({placeholders})
        ORDER BY e.product_id ASC, e.role ASC, e.input_product_id ASC
        "#
    );
    let mut query = sqlx::query(&sql);
    for id in product_ids {
        query = query.bind(id);
    }
    let rows = query.fetch_all(pool).await.map_err(CatalogError::from)?;
    for row in rows {
        let product_id: String = row.get("product_id");
        let level: Option<String> = row.get("level");
        let kind: Option<String> = row.get("kind");
        let collection_id = match (level, kind) {
            (Some(level), Some(kind)) => level
                .parse::<ProductLevel>()
                .ok()
                .map(|level| collection_id_for(level, &kind)),
            _ => None,
        };
        edges.entry(product_id).or_default().push(ItemInputEdge {
            input_product_id: row.get("input_product_id"),
            role: row.get("role"),
            collection_id,
        });
    }
    Ok(edges)
}

// --- Search / query primitives -------------------------------------------------

/// A `datetime` query parameter: a single instant or a (half-)open interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatetimeInterval {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

/// Parse a STAC `datetime` parameter: `instant`, `start/end`, `../end`,
/// `start/..`. A fully open `../..` or malformed text is a typed error.
pub fn parse_datetime_param(text: &str) -> Result<DatetimeInterval, StacError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(StacError::InvalidDatetime("empty datetime".to_string()));
    }
    let parse = |part: &str| -> Result<DateTime<Utc>, StacError> {
        DateTime::parse_from_rfc3339(part)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|err| StacError::InvalidDatetime(format!("{part}: {err}")))
    };
    match text.split_once('/') {
        None => {
            let instant = parse(text)?;
            Ok(DatetimeInterval {
                start: Some(instant),
                end: Some(instant),
            })
        }
        Some((start_text, end_text)) => {
            let start = match start_text.trim() {
                ".." | "" => None,
                part => Some(parse(part)?),
            };
            let end = match end_text.trim() {
                ".." | "" => None,
                part => Some(parse(part)?),
            };
            if start.is_none() && end.is_none() {
                return Err(StacError::InvalidDatetime(
                    "interval cannot be open on both ends".to_string(),
                ));
            }
            if let (Some(start), Some(end)) = (start, end) {
                if start > end {
                    return Err(StacError::InvalidDatetime(format!(
                        "interval start {start} is after end {end}"
                    )));
                }
            }
            Ok(DatetimeInterval { start, end })
        }
    }
}

/// Parse a 4-element `min_lon,min_lat,max_lon,max_lat` bbox.
pub fn parse_bbox_values(values: &[f64]) -> Result<[f64; 4], StacError> {
    if values.len() != 4 {
        return Err(StacError::InvalidBbox(format!(
            "expected 4 elements, got {}",
            values.len()
        )));
    }
    let bbox = [values[0], values[1], values[2], values[3]];
    if bbox.iter().any(|v| !v.is_finite()) {
        return Err(StacError::InvalidBbox("non-finite coordinate".to_string()));
    }
    if bbox[0] > bbox[2] || bbox[1] > bbox[3] {
        return Err(StacError::InvalidBbox(
            "min corner must not exceed max corner".to_string(),
        ));
    }
    Ok(bbox)
}

/// Axis-aligned bbox intersection (closed edges: touching boxes intersect).
pub fn bboxes_intersect(a: &[f64; 4], b: &[f64; 4]) -> bool {
    a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1]
}

/// Clamp a requested page size to `1..=MAX_LIMIT`, defaulting to
/// [`DEFAULT_LIMIT`].
pub fn clamp_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

/// Offset pagination: the page slice plus the next offset when more items
/// remain.
pub fn paginate<T>(items: Vec<T>, offset: usize, limit: usize) -> (Vec<T>, Option<usize>) {
    let total = items.len();
    let page: Vec<T> = items.into_iter().skip(offset).take(limit).collect();
    let consumed = offset.saturating_add(page.len());
    let next = (consumed < total).then_some(consumed);
    (page, next)
}

/// Normalized search request shared by GET and POST `/api/stac/search` and by
/// the per-collection items listing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchRequest {
    pub collections: Option<Vec<String>>,
    pub ids: Option<Vec<String>>,
    pub bbox: Option<[f64; 4]>,
    pub datetime: Option<DatetimeInterval>,
    pub limit: usize,
    pub offset: usize,
}

/// Result page of a search: mapped items, next offset, skipped-record count.
#[derive(Debug)]
pub struct SearchPage {
    pub items: Vec<StacItem>,
    pub next_offset: Option<usize>,
    pub skipped: usize,
}

fn rfc3339_utc(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Execute a search over the catalog. Level/kind/bbox/datetime constraints are
/// pushed into the catalog SQL per collection; `ids` are post-filtered.
/// Ordering follows the catalog listing order (created_at DESC, product_id
/// ASC), deduplicated by product id, then offset/limit paginated — the
/// internal catalog is small, so in-memory pagination over the filtered set is
/// deliberate and keeps this layered on `catalog::list_products` instead of a
/// parallel SQL builder.
pub async fn search(pool: &DbPool, request: &SearchRequest) -> Result<SearchPage, StacError> {
    // Resolve which collections to scan; unknown requested ids are 404s.
    let summaries = collection_summaries(pool).await?;
    let known: Vec<String> = summaries.iter().map(|s| s.id.clone()).collect();
    let scan: Vec<String> = match &request.collections {
        Some(requested) => {
            for id in requested {
                if !known.contains(id) {
                    return Err(StacError::CollectionNotFound(id.clone()));
                }
            }
            requested.clone()
        }
        None => known,
    };

    let temporal_start = request
        .datetime
        .as_ref()
        .and_then(|d| d.start.as_ref())
        .map(rfc3339_utc);
    let temporal_end = request
        .datetime
        .as_ref()
        .and_then(|d| d.end.as_ref())
        .map(rfc3339_utc);

    let mut records: Vec<RegisteredProduct> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for collection_id in &scan {
        for mut filter in collection_member_filters(collection_id) {
            filter.bbox = request.bbox;
            // STAC datetime overlap == catalog temporal filter semantics:
            // keep products whose interval overlaps [start, end].
            filter.temporal_start = temporal_start.clone();
            filter.temporal_end = temporal_end.clone();
            for record in catalog::list_products(pool, &filter).await? {
                if seen.insert(record.product_id.clone()) {
                    records.push(record);
                }
            }
        }
    }

    if let Some(ids) = &request.ids {
        records.retain(|record| ids.contains(&record.product_id));
    }

    // Deterministic global order across collections (list order is per query).
    records.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.product_id.cmp(&b.product_id))
    });

    // Map records to items, skipping STAC-invalid ones; paginate over the
    // valid items so pages are dense.
    let ids: Vec<String> = records.iter().map(|r| r.product_id.clone()).collect();
    let edges = load_input_edges(pool, &ids).await?;
    let mut skipped = 0usize;
    let mut items = Vec::new();
    for record in &records {
        let inputs = edges
            .get(&record.product_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        match product_to_item(record, inputs) {
            Ok(item) => items.push(item),
            Err(_) => skipped += 1,
        }
    }
    let (page, next_offset) = paginate(items, request.offset, request.limit);
    Ok(SearchPage {
        items: page,
        next_offset,
        skipped,
    })
}

/// Fetch a single item by collection + id, verifying membership.
pub async fn get_item(
    pool: &DbPool,
    collection_id: &str,
    item_id: &str,
) -> Result<StacItem, StacError> {
    let not_found = || StacError::ItemNotFound {
        collection_id: collection_id.to_string(),
        item_id: item_id.to_string(),
    };
    let product = catalog::get_product(pool, item_id)
        .await?
        .ok_or_else(not_found)?;
    if collection_id_for(product.level, &product.kind) != collection_id {
        return Err(not_found());
    }
    let edges = load_input_edges(pool, std::slice::from_ref(&product.product_id)).await?;
    let inputs = edges
        .get(&product.product_id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    product_to_item(&product, inputs).map_err(|_| not_found())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn product(level: ProductLevel, kind: &str) -> RegisteredProduct {
        RegisteredProduct {
            product_id: "prod-1".to_string(),
            level,
            kind: kind.to_string(),
            algorithm_id: "test.algo".to_string(),
            algorithm_version: "1.0.0".to_string(),
            parameters: serde_json::json!({}),
            parameters_hash: "hash".to_string(),
            path: Some("/data/prod-1.tif".to_string()),
            format: Some("tif".to_string()),
            checksum_sha256: None,
            crs: Some("EPSG:4326".to_string()),
            bbox: Some([-96.5, 41.0, -96.4, 41.1]),
            gsd_m_per_px: Some(10.0),
            temporal_start: Some("2026-06-01T00:00:00Z".to_string()),
            temporal_end: Some("2026-06-01T00:00:00Z".to_string()),
            farm_id: None,
            field_id: Some("field-1".to_string()),
            season_id: None,
            scene_id: Some("scene-1".to_string()),
            source_id: None,
            quality_mask_product_id: None,
            confidence: None,
            confidence_method: None,
            quality_summary: None,
            status: "registered".to_string(),
            superseded_by: None,
            provenance_id: None,
            created_at: "2026-06-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn collection_id_maps_levels_and_kinds() {
        assert_eq!(collection_id_for(ProductLevel::L0, "usgs_scene"), "scenes");
        assert_eq!(collection_id_for(ProductLevel::L1, "band_nir"), "scenes");
        assert_eq!(collection_id_for(ProductLevel::L2, "ndvi"), "ndvi");
        assert_eq!(
            collection_id_for(ProductLevel::L3, "temporal_composite"),
            "temporal_composite"
        );
        // A derived kind named "scenes" must not collide with the source
        // collection.
        assert_eq!(
            collection_id_for(ProductLevel::L2, "scenes"),
            "derived-scenes"
        );
    }

    #[test]
    fn item_maps_bbox_geometry_and_properties() {
        let item = product_to_item(&product(ProductLevel::L2, "ndvi"), &[]).unwrap();
        assert_eq!(item.type_, "Feature");
        assert_eq!(item.stac_version, STAC_VERSION);
        assert_eq!(item.collection, "ndvi");
        assert_eq!(item.bbox, Some([-96.5, 41.0, -96.4, 41.1]));

        // Exterior ring: closed, counterclockwise (RFC 7946).
        let geometry = item.geometry.as_ref().unwrap();
        assert_eq!(geometry["type"], "Polygon");
        let ring = geometry["coordinates"][0].as_array().unwrap();
        assert_eq!(ring.len(), 5);
        assert_eq!(ring[0], ring[4]);
        let signed_area: f64 = (0..4)
            .map(|i| {
                let a = ring[i].as_array().unwrap();
                let b = ring[i + 1].as_array().unwrap();
                a[0].as_f64().unwrap() * b[1].as_f64().unwrap()
                    - b[0].as_f64().unwrap() * a[1].as_f64().unwrap()
            })
            .sum();
        assert!(signed_area > 0.0, "exterior ring must wind CCW");

        assert_eq!(item.properties["datetime"], "2026-06-01T00:00:00Z");
        assert_eq!(item.properties["proj:code"], "EPSG:4326");
        assert_eq!(item.properties["processing:level"], "L2");
        assert_eq!(item.properties["agbot:product_kind"], "ndvi");
        assert_eq!(item.properties["agbot:scene_id"], "scene-1");
        assert_eq!(item.properties["gsd"], 10.0);
    }

    #[test]
    fn item_datetime_is_normalized_to_utc_rfc3339() {
        let mut record = product(ProductLevel::L2, "ndvi");
        record.temporal_start = Some("2026-06-01T02:00:00+02:00".to_string());
        record.temporal_end = record.temporal_start.clone();
        let item = product_to_item(&record, &[]).unwrap();
        assert_eq!(item.properties["datetime"], "2026-06-01T00:00:00Z");
    }

    #[test]
    fn item_interval_sets_start_and_end_datetimes() {
        let mut record = product(ProductLevel::L3, "temporal_composite");
        record.temporal_start = Some("2026-06-01T00:00:00Z".to_string());
        record.temporal_end = Some("2026-06-30T23:59:59Z".to_string());
        let item = product_to_item(&record, &[]).unwrap();
        assert_eq!(item.properties["datetime"], "2026-06-01T00:00:00Z");
        assert_eq!(item.properties["start_datetime"], "2026-06-01T00:00:00Z");
        assert_eq!(item.properties["end_datetime"], "2026-06-30T23:59:59Z");
        assert_eq!(item.properties["processing:level"], "L3");
    }

    #[test]
    fn item_without_timestamp_is_skipped() {
        let mut record = product(ProductLevel::L2, "ndvi");
        record.temporal_start = None;
        assert_eq!(
            product_to_item(&record, &[]).unwrap_err(),
            ItemSkipReason::MissingOrInvalidTimestamp
        );
        let mut record = product(ProductLevel::L2, "ndvi");
        record.temporal_start = Some("not-a-date".to_string());
        assert_eq!(
            product_to_item(&record, &[]).unwrap_err(),
            ItemSkipReason::MissingOrInvalidTimestamp
        );
    }

    #[test]
    fn item_without_bbox_is_skipped() {
        let mut record = product(ProductLevel::L2, "ndvi");
        record.bbox = None;
        assert_eq!(
            product_to_item(&record, &[]).unwrap_err(),
            ItemSkipReason::MissingSpatialRef
        );
    }

    #[test]
    fn projected_crs_yields_null_geometry_with_reason() {
        let mut record = product(ProductLevel::L2, "ndvi");
        record.crs = Some("EPSG:32614".to_string());
        let item = product_to_item(&record, &[]).unwrap();
        assert!(item.geometry.is_none());
        assert!(item.bbox.is_none());
        let reason = item.properties["agbot:geometry_omitted_reason"]
            .as_str()
            .unwrap();
        assert!(reason.contains("EPSG:32614"), "{reason}");
        assert_eq!(item.properties["proj:code"], "EPSG:32614");
    }

    #[test]
    fn missing_crs_with_bbox_is_treated_as_geographic() {
        let mut record = product(ProductLevel::L2, "ndvi");
        record.crs = None;
        let item = product_to_item(&record, &[]).unwrap();
        assert!(item.geometry.is_some());
        assert_eq!(item.bbox, Some([-96.5, 41.0, -96.4, 41.1]));
        assert!(!item.properties.contains_key("proj:code"));
    }

    #[test]
    fn lineage_edges_become_derived_from_links() {
        let edges = vec![
            ItemInputEdge {
                input_product_id: "band-1".to_string(),
                role: "band:nir".to_string(),
                collection_id: Some("scenes".to_string()),
            },
            ItemInputEdge {
                input_product_id: "gone".to_string(),
                role: "mask".to_string(),
                collection_id: None,
            },
        ];
        let item = product_to_item(&product(ProductLevel::L2, "ndvi"), &edges).unwrap();
        let derived: Vec<&StacLink> = item
            .links
            .iter()
            .filter(|l| l.rel == "derived_from")
            .collect();
        assert_eq!(derived.len(), 2);
        assert_eq!(derived[0].href, "/api/stac/collections/scenes/items/band-1");
        assert_eq!(derived[0].title.as_deref(), Some("band:nir"));
        // Unresolvable input collection falls back to the catalog endpoint.
        assert_eq!(derived[1].href, "/api/catalog/products/gone");
    }

    #[test]
    fn item_links_include_self_collection_root() {
        let item = product_to_item(&product(ProductLevel::L2, "ndvi"), &[]).unwrap();
        let rel_href = |rel: &str| {
            item.links
                .iter()
                .find(|l| l.rel == rel)
                .map(|l| l.href.clone())
        };
        assert_eq!(
            rel_href("self").as_deref(),
            Some("/api/stac/collections/ndvi/items/prod-1")
        );
        assert_eq!(
            rel_href("collection").as_deref(),
            Some("/api/stac/collections/ndvi")
        );
        assert_eq!(rel_href("root").as_deref(), Some("/api/stac"));
    }

    #[test]
    fn scene_backed_item_gets_serving_and_tile_assets() {
        let item = product_to_item(&product(ProductLevel::L2, "ndvi"), &[]).unwrap();
        assert_eq!(
            item.assets["data"].href,
            "/api/scenes/scene-1/products/ndvi"
        );
        assert_eq!(
            item.assets["data"].media_type.as_deref(),
            Some("image/tiff; application=geotiff")
        );
        assert_eq!(
            item.assets["tiles"].href,
            "/api/scenes/scene-1/products/ndvi/tiles/{z}/{x}/{y}.png"
        );
    }

    #[test]
    fn artifact_only_item_gets_path_asset() {
        let mut record = product(ProductLevel::L3, "temporal_composite");
        record.scene_id = None;
        let item = product_to_item(&record, &[]).unwrap();
        assert_eq!(item.assets["data"].href, "/data/prod-1.tif");
        assert!(!item.assets.contains_key("tiles"));
    }

    #[test]
    fn geotiff_artifacts_get_a_web_mercator_tile_asset() {
        // Scene-backed and artifact-only GeoTIFFs both get the global
        // template; non-GeoTIFF artifacts do not.
        let item = product_to_item(&product(ProductLevel::L2, "ndvi"), &[]).unwrap();
        assert_eq!(
            item.assets["tiles_web"].href,
            "/api/catalog/products/prod-1/tiles/{z}/{x}/{y}.png"
        );
        assert_eq!(item.assets["tiles_web"].roles, vec!["visual".to_string()]);

        let mut artifact_only = product(ProductLevel::L3, "temporal_composite");
        artifact_only.scene_id = None;
        let item = product_to_item(&artifact_only, &[]).unwrap();
        assert!(item.assets.contains_key("tiles_web"));

        let mut png_product = product(ProductLevel::L2, "ndvi");
        png_product.path = Some("/data/prod-1.png".to_string());
        png_product.format = Some("png".to_string());
        let item = product_to_item(&png_product, &[]).unwrap();
        assert!(!item.assets.contains_key("tiles_web"));
    }

    #[test]
    fn datetime_param_single_instant() {
        let interval = parse_datetime_param("2026-06-01T00:00:00Z").unwrap();
        let expected = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        assert_eq!(interval.start, Some(expected));
        assert_eq!(interval.end, Some(expected));
    }

    #[test]
    fn datetime_param_closed_interval() {
        let interval = parse_datetime_param("2026-06-01T00:00:00Z/2026-06-30T00:00:00Z").unwrap();
        assert_eq!(
            interval.start,
            Some(Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap())
        );
        assert_eq!(
            interval.end,
            Some(Utc.with_ymd_and_hms(2026, 6, 30, 0, 0, 0).unwrap())
        );
    }

    #[test]
    fn datetime_param_open_ends() {
        let open_start = parse_datetime_param("../2026-06-30T00:00:00Z").unwrap();
        assert_eq!(open_start.start, None);
        assert!(open_start.end.is_some());

        let open_end = parse_datetime_param("2026-06-01T00:00:00Z/..").unwrap();
        assert!(open_end.start.is_some());
        assert_eq!(open_end.end, None);
    }

    #[test]
    fn datetime_param_malformed_is_typed_error() {
        for bad in ["", "not-a-date", "../..", "2026-06-01", "a/b"] {
            let err = parse_datetime_param(bad).unwrap_err();
            assert!(
                matches!(err, StacError::InvalidDatetime(_)),
                "{bad}: {err:?}"
            );
        }
        // Reversed interval is also malformed.
        let err = parse_datetime_param("2026-06-30T00:00:00Z/2026-06-01T00:00:00Z").unwrap_err();
        assert!(matches!(err, StacError::InvalidDatetime(_)));
    }

    #[test]
    fn bbox_intersection_logic() {
        let a = [0.0, 0.0, 10.0, 10.0];
        assert!(bboxes_intersect(&a, &[5.0, 5.0, 15.0, 15.0]));
        assert!(bboxes_intersect(&a, &[10.0, 10.0, 20.0, 20.0]), "touching");
        assert!(bboxes_intersect(&a, &[2.0, 2.0, 3.0, 3.0]), "contained");
        assert!(!bboxes_intersect(&a, &[11.0, 0.0, 20.0, 10.0]));
        assert!(!bboxes_intersect(&a, &[0.0, 11.0, 10.0, 20.0]));
    }

    #[test]
    fn bbox_param_validation() {
        assert_eq!(
            parse_bbox_values(&[-1.0, -1.0, 1.0, 1.0]).unwrap(),
            [-1.0, -1.0, 1.0, 1.0]
        );
        assert!(matches!(
            parse_bbox_values(&[1.0, 2.0, 3.0]).unwrap_err(),
            StacError::InvalidBbox(_)
        ));
        assert!(matches!(
            parse_bbox_values(&[3.0, 0.0, 1.0, 1.0]).unwrap_err(),
            StacError::InvalidBbox(_)
        ));
        assert!(matches!(
            parse_bbox_values(&[f64::NAN, 0.0, 1.0, 1.0]).unwrap_err(),
            StacError::InvalidBbox(_)
        ));
    }

    #[test]
    fn limit_clamping() {
        assert_eq!(clamp_limit(None), DEFAULT_LIMIT);
        assert_eq!(clamp_limit(Some(0)), 1);
        assert_eq!(clamp_limit(Some(50)), 50);
        assert_eq!(clamp_limit(Some(1000)), MAX_LIMIT);
    }

    #[test]
    fn pagination_next_offset() {
        let items: Vec<u32> = (0..25).collect();
        let (page, next) = paginate(items.clone(), 0, 10);
        assert_eq!(page.len(), 10);
        assert_eq!(next, Some(10));
        let (page, next) = paginate(items.clone(), 20, 10);
        assert_eq!(page.len(), 5);
        assert_eq!(next, None);
        let (page, next) = paginate(items, 100, 10);
        assert!(page.is_empty());
        assert_eq!(next, None);
    }

    #[test]
    fn landing_page_declares_conformance_and_links() {
        let landing = landing_page();
        assert_eq!(landing.type_, "Catalog");
        assert_eq!(landing.stac_version, STAC_VERSION);
        assert!(landing
            .conforms_to
            .iter()
            .any(|c| c.ends_with("/item-search")));
        for rel in ["self", "data", "search", "conformance"] {
            assert!(
                landing.links.iter().any(|l| l.rel == rel),
                "missing link rel {rel}"
            );
        }
    }
}
