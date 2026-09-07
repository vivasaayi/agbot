//! MODIS MOD13Q1 v061 NDVI as a points-only time-series source (batch S-14).
//!
//! MODIS is ingested differently from the L1/L2 satellite derive paths: there
//! is no local L2 index raster and no per-pixel product artifact. Instead each
//! 16-day MOD13Q1 tile that intersects a field becomes
//!
//! - one **external L3 catalog product** (`kind = "modis_ndvi"`, `level = l3`,
//!   field scope) whose `path` is the *unsigned* remote NDVI asset href, so the
//!   catalog records the provenance of the observation without copying pixels;
//! - five **zonal statistics** appended to `time_series_points` under the
//!   canonical `sat.ndvi.*` metric namespace (the same namespace the S-3
//!   extraction uses for Landsat/Sentinel-2), tagged `"source": "modis"` in
//!   `metadata_json` so the S-4 field-timeseries API surfaces MODIS as its own
//!   per-source series alongside the higher-resolution sensors.
//!
//! The NDVI asset is read over the field AOI as a SAS-signed ranged COG read
//! through [`PcSignedCogResolver`] (collection `modis-13Q1-061`). MOD13Q1 NDVI
//! is stored as scaled Int16 with a `-3000` fill: DN is scaled by
//! [`MOD13Q1_NDVI_SCALE`] and both the `-3000` fill and the COG's GDAL_NODATA
//! tag are masked before the zonal reduction.
//!
//! Both writes are idempotent: catalog registration dedupes on
//! `(kind, parameters_hash)` (content-addressed), and the time-series append
//! uses `INSERT OR IGNORE` on the `(entity_ref, metric, t, source_ref)` primary
//! key, so a re-run of the same window appends zero rows.
//!
//! v1 exposes a manual trigger (`POST /api/fields/:id/modis/ingest`). A
//! pipeline job kind can wrap [`ingest_modis_ndvi_for_field`] later without
//! changing this module — deliberately kept out of the pipeline worker so the
//! ingest logic has one home.

use std::sync::Arc;

use serde::Serialize;
use serde_json::json;
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use shared::schemas::GeoBounds;
use shared::timeseries_naming::{
    field_entity_ref, product_source_ref, satellite_metric, SOURCE_MODIS,
};
use thiserror::Error;

use crate::db::DbPool;
use crate::field_timeseries::zonal_stats;
use crate::pc_sign::{PcSasTokenCache, PcSignedCogResolver};
use crate::satellite_derivation::CogStoreResolver;

/// Planetary Computer STAC search endpoint (POST). Tests inject a local base
/// URL; production uses the real Planetary Computer search API.
pub const PLANETARY_COMPUTER_STAC_SEARCH: &str =
    "https://planetarycomputer.microsoft.com/api/stac/v1/search";

/// MOD13Q1 v061 collection id on Planetary Computer. Used both as the STAC
/// search collection and the SAS token collection.
pub const MODIS_13Q1_COLLECTION: &str = "modis-13Q1-061";

/// The 250m 16-day NDVI asset key in a MOD13Q1 STAC item.
pub const MODIS_NDVI_ASSET: &str = "250m_16_days_NDVI";

/// MOD13Q1 NDVI is stored as scaled Int16: true NDVI = DN * 0.0001.
pub const MOD13Q1_NDVI_SCALE: f64 = 0.0001;

/// MOD13Q1 NDVI fill value (masked pixels). Applied on the raw DN before
/// scaling.
pub const MOD13Q1_NDVI_FILL: f64 = -3000.0;

const USER_AGENT: &str = "agbot-geo-hub/0.1";
const ALGORITHM_ID: &str = "modis.mod13q1.ndvi_ingest";
const ALGORITHM_VERSION: &str = "1.0.0";

/// Catalog `source_id` for the MOD13Q1 collection. `field_timeseries::
/// source_family` maps any id containing "modis" to [`SOURCE_MODIS`].
pub fn modis_source_id() -> String {
    format!("planetary-computer:{MODIS_13Q1_COLLECTION}")
}

#[derive(Debug, Error)]
pub enum ModisError {
    #[error("failed to build HTTP client: {0}")]
    Client(#[source] reqwest::Error),
    #[error("field {0} not found")]
    FieldNotFound(String),
    #[error("field {field_id} boundary is invalid: {message}")]
    InvalidBoundary { field_id: String, message: String },
    #[error("invalid date range: {0}")]
    InvalidDateRange(String),
    #[error("MODIS STAC search failed with {status}: {body}")]
    SearchStatus { status: u16, body: String },
    #[error("MODIS STAC search request failed: {0}")]
    SearchRequest(#[source] reqwest::Error),
    #[error("MODIS STAC response is invalid: {0}")]
    InvalidResponse(String),
    #[error("failed to read MODIS NDVI asset over the field AOI: {0}")]
    Raster(String),
    #[error(transparent)]
    Catalog(#[from] crate::catalog::CatalogError),
    #[error("time-series persistence failed: {0}")]
    Db(#[from] sqlx::Error),
}

/// What one MODIS ingest run touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModisIngestOutcome {
    /// MOD13Q1 items the STAC search returned for the field/date window.
    pub items_found: usize,
    /// External L3 products registered (new registrations only; re-runs of an
    /// already-registered item still count here since registration is
    /// idempotent and returns the existing id).
    pub products_registered: usize,
    /// Time-series points newly appended across all items.
    pub points_appended: usize,
    /// Time-series points skipped because they already existed (idempotency).
    pub points_skipped: usize,
}

// --- STAC wire shapes (self-contained; the landsat search shapes are private) --

#[derive(Debug, serde::Deserialize)]
struct StacFeatureCollection {
    #[serde(default)]
    features: Vec<StacFeature>,
}

#[derive(Debug, serde::Deserialize)]
struct StacFeature {
    id: String,
    #[serde(default)]
    properties: StacProperties,
    #[serde(default)]
    assets: std::collections::BTreeMap<String, StacAsset>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct StacProperties {
    #[serde(default)]
    datetime: Option<String>,
    #[serde(default)]
    start_datetime: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct StacAsset {
    href: String,
}

/// Scale MOD13Q1 raw NDVI DNs to NDVI, masking the `-3000` fill and any pixel
/// equal to the optional GDAL_NODATA sentinel. Masked pixels become `NaN` so
/// the downstream zonal reduction (which rejects non-finite pixels) drops them
/// from every statistic including `valid_fraction`.
pub(crate) fn scale_modis_ndvi(raw: &[f32], nodata: Option<f32>) -> Vec<f32> {
    raw.iter()
        .map(|dn| {
            let dn = f64::from(*dn);
            let is_fill = (dn - MOD13Q1_NDVI_FILL).abs() < f64::EPSILON;
            let is_nodata = nodata.is_some_and(|n| (dn - f64::from(n)).abs() < f64::EPSILON);
            if is_fill || is_nodata {
                f32::NAN
            } else {
                (dn * MOD13Q1_NDVI_SCALE) as f32
            }
        })
        .collect()
}

/// Field WGS84 bounding box parsed from the field's `boundary_json` GeoJSON
/// geometry (Polygon / MultiPolygon). Returns the envelope of every coordinate.
async fn field_bbox(pool: &DbPool, field_id: &str) -> Result<GeoBounds, ModisError> {
    let boundary_json: Option<String> =
        sqlx::query_scalar("SELECT boundary_json FROM fields WHERE field_id = ?")
            .bind(field_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| ModisError::FieldNotFound(field_id.to_string()))?;
    let boundary_json = boundary_json.ok_or_else(|| ModisError::InvalidBoundary {
        field_id: field_id.to_string(),
        message: "field has no boundary geometry".to_string(),
    })?;
    let geometry: serde_json::Value =
        serde_json::from_str(&boundary_json).map_err(|err| ModisError::InvalidBoundary {
            field_id: field_id.to_string(),
            message: format!("boundary is not valid JSON: {err}"),
        })?;
    bbox_from_geometry(&geometry).ok_or_else(|| ModisError::InvalidBoundary {
        field_id: field_id.to_string(),
        message: "boundary geometry has no coordinates".to_string(),
    })
}

/// Envelope of every `[lon, lat]` position nested anywhere under a GeoJSON
/// geometry's `coordinates`. Position-shape agnostic (Polygon rings,
/// MultiPolygon, etc.) since it recurses to the numeric leaves.
fn bbox_from_geometry(geometry: &serde_json::Value) -> Option<GeoBounds> {
    let coordinates = geometry.get("coordinates")?;
    let mut bounds: Option<GeoBounds> = None;
    collect_positions(coordinates, &mut bounds);
    bounds
}

fn collect_positions(value: &serde_json::Value, bounds: &mut Option<GeoBounds>) {
    let serde_json::Value::Array(items) = value else {
        return;
    };
    // A position is `[lon, lat, ...]`: first two entries numeric.
    if items.len() >= 2 && items[0].is_number() && items[1].is_number() {
        if let (Some(lon), Some(lat)) = (items[0].as_f64(), items[1].as_f64()) {
            match bounds {
                Some(b) => {
                    b.min_lon = b.min_lon.min(lon);
                    b.max_lon = b.max_lon.max(lon);
                    b.min_lat = b.min_lat.min(lat);
                    b.max_lat = b.max_lat.max(lat);
                }
                None => {
                    *bounds = Some(GeoBounds {
                        min_lon: lon,
                        min_lat: lat,
                        max_lon: lon,
                        max_lat: lat,
                    });
                }
            }
        }
        return;
    }
    for item in items {
        collect_positions(item, bounds);
    }
}

/// STAC search MOD13Q1 v061 over the field bbox and `[start, end]` date range.
/// `stac_base_url` is the full search endpoint (tests inject a local server).
async fn search_modis_items(
    http: &reqwest::Client,
    stac_base_url: &str,
    bbox: &GeoBounds,
    start: &str,
    end: &str,
) -> Result<Vec<StacFeature>, ModisError> {
    let body = json!({
        "collections": [MODIS_13Q1_COLLECTION],
        "bbox": [bbox.min_lon, bbox.min_lat, bbox.max_lon, bbox.max_lat],
        "datetime": format!("{start}T00:00:00Z/{end}T23:59:59Z"),
        "limit": 100,
    });
    let response = http
        .post(stac_base_url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .json(&body)
        .send()
        .await
        .map_err(ModisError::SearchRequest)?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(ModisError::SearchStatus {
            status: status.as_u16(),
            body: text.chars().take(300).collect(),
        });
    }
    let collection: StacFeatureCollection =
        serde_json::from_str(&text).map_err(|err| ModisError::InvalidResponse(err.to_string()))?;
    Ok(collection.features)
}

/// The external L3 catalog draft for one MOD13Q1 item. Identity folds the item
/// id + the NDVI href into the parameters, so re-registering the same item is a
/// content-addressed no-op and distinct items stay distinct.
fn modis_l3_draft(
    field_id: &str,
    item_id: &str,
    ndvi_href: &str,
    observed_at: &str,
) -> ProductRecordDraft {
    ProductRecordDraft {
        level: ProductLevel::L3,
        kind: "modis_ndvi".to_string(),
        algorithm_id: ALGORITHM_ID.to_string(),
        algorithm_version: ALGORITHM_VERSION.to_string(),
        parameters: json!({
            "item_id": item_id,
            "collection": MODIS_13Q1_COLLECTION,
            "asset": MODIS_NDVI_ASSET,
            "ndvi_href": ndvi_href,
        }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: Some(field_id.to_string()),
            season_id: None,
            scene_id: None,
            temporal_start: observed_at.to_string(),
            temporal_end: observed_at.to_string(),
        },
        spatial_ref: None,
        gsd_m_per_px: Some(250.0),
        // External reference: the artifact path is the *unsigned* remote NDVI
        // href (mirrors the L1 remote-href reference pattern; no pixels copied).
        artifact: Some(ProductArtifact {
            format: "cog".to_string(),
            path: ndvi_href.to_string(),
            checksum_sha256: None,
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some(modis_source_id()),
    }
}

/// The item's observation timestamp: `datetime`, else `start_datetime`.
fn item_datetime(item: &StacFeature) -> Option<String> {
    item.properties
        .datetime
        .clone()
        .or_else(|| item.properties.start_datetime.clone())
}

/// Ingest MOD13Q1 v061 NDVI for one field over `[start, end]` (inclusive
/// `YYYY-MM-DD` dates). See the module docs for the full contract.
///
/// `stac_base_url` is the STAC search endpoint; `cache` provides SAS tokens for
/// the `modis-13Q1-061` collection. Both the catalog registration and the
/// time-series append are idempotent, so re-running the same window is safe.
pub async fn ingest_modis_ndvi_for_field(
    pool: &DbPool,
    cache: &Arc<PcSasTokenCache>,
    field_id: &str,
    start: &str,
    end: &str,
    stac_base_url: &str,
) -> Result<ModisIngestOutcome, ModisError> {
    validate_date(start)?;
    validate_date(end)?;
    if start > end {
        return Err(ModisError::InvalidDateRange(format!(
            "start {start} is after end {end}"
        )));
    }

    let resolver = PcSignedCogResolver::new(cache.clone(), MODIS_13Q1_COLLECTION);
    ingest_modis_ndvi_with_resolver(pool, &resolver, field_id, start, end, stac_base_url).await
}

/// Resolver-injectable core of [`ingest_modis_ndvi_for_field`]. Production
/// passes a [`PcSignedCogResolver`]; tests inject a resolver backed by a local
/// SAS-gated blob server (the blob-host gate in `PcSignedCogResolver` would
/// otherwise route a `127.0.0.1` href to the plain URL store). Same contract
/// and idempotency as the public entry point.
pub async fn ingest_modis_ndvi_with_resolver(
    pool: &DbPool,
    resolver: &dyn CogStoreResolver,
    field_id: &str,
    start: &str,
    end: &str,
    stac_base_url: &str,
) -> Result<ModisIngestOutcome, ModisError> {
    validate_date(start)?;
    validate_date(end)?;
    if start > end {
        return Err(ModisError::InvalidDateRange(format!(
            "start {start} is after end {end}"
        )));
    }

    let bbox = field_bbox(pool, field_id).await?;
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(ModisError::Client)?;
    let items = search_modis_items(&http, stac_base_url, &bbox, start, end).await?;

    let entity_ref = field_entity_ref(field_id);
    let metadata_json = json!({ "source": SOURCE_MODIS, "level": "l3" }).to_string();
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let mut outcome = ModisIngestOutcome {
        items_found: items.len(),
        products_registered: 0,
        points_appended: 0,
        points_skipped: 0,
    };

    for item in &items {
        let Some(ndvi_asset) = item.assets.get(MODIS_NDVI_ASSET) else {
            continue;
        };
        let Some(observed_at) = item_datetime(item) else {
            continue;
        };

        // 1. Register the external L3 product (content-addressed idempotent).
        let draft = modis_l3_draft(field_id, &item.id, &ndvi_asset.href, &observed_at);
        let product_id = crate::catalog::register_product(pool, &draft, &created_at).await?;
        outcome.products_registered += 1;

        // 2. Read the NDVI asset over the AOI, scale + mask, reduce to stats.
        let stats = read_modis_ndvi_stats(resolver, &ndvi_asset.href).await?;
        let source_ref = product_source_ref(&product_id);
        for (stat, value) in stats {
            let metric = satellite_metric("ndvi", stat);
            let result = sqlx::query(
                r#"
                INSERT OR IGNORE INTO time_series_points (
                    entity_ref, metric, t, value_kind, scalar_value, source_ref,
                    created_at, metadata_json
                )
                VALUES (?1, ?2, ?3, 'scalar', ?4, ?5, ?6, ?7)
                "#,
            )
            .bind(&entity_ref)
            .bind(&metric)
            .bind(&observed_at)
            .bind(value)
            .bind(&source_ref)
            .bind(&created_at)
            .bind(&metadata_json)
            .execute(pool)
            .await?;
            if result.rows_affected() > 0 {
                outcome.points_appended += 1;
            } else {
                outcome.points_skipped += 1;
            }
        }
    }

    Ok(outcome)
}

/// Read the NDVI COG over the field AOI (whole tile in v1), scale + mask, and
/// reduce to the five zonal statistics. Returns an empty vec when the tile has
/// no valid pixel over the field.
async fn read_modis_ndvi_stats(
    resolver: &dyn CogStoreResolver,
    ndvi_href: &str,
) -> Result<Vec<(shared::timeseries_naming::ZonalStat, f64)>, ModisError> {
    let (store, location) = resolver
        .resolve(ndvi_href)
        .map_err(|err| ModisError::Raster(err.to_string()))?;
    let reader = raster_io::RemoteCogReader::open(store, &location)
        .await
        .map_err(|err| ModisError::Raster(err.to_string()))?;
    let nodata = reader.info().nodata.map(|n| n as f32);
    let band = reader
        .read_band()
        .await
        .map_err(|err| ModisError::Raster(err.to_string()))?;
    let scaled = scale_modis_ndvi(&band.to_f32(), nodata);
    // Fill/nodata pixels are already NaN; pass no extra sentinel.
    Ok(zonal_stats(&scaled, None).unwrap_or_default())
}

/// `YYYY-MM-DD` validation (the STAC search interpolates the raw string into
/// the datetime range, so malformed dates are rejected up front).
fn validate_date(date: &str) -> Result<(), ModisError> {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|err| ModisError::InvalidDateRange(format!("{date:?}: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::timeseries_naming::ZonalStat;

    #[test]
    fn scale_masks_fill_and_scales_valid_dns() {
        // DNs: 2000 -> 0.2, 5000 -> 0.5, fill -3000 -> masked, 8000 -> 0.8.
        let raw = [2000.0f32, 5000.0, -3000.0, 8000.0];
        let scaled = scale_modis_ndvi(&raw, None);
        assert!((scaled[0] - 0.2).abs() < 1e-6);
        assert!((scaled[1] - 0.5).abs() < 1e-6);
        assert!(scaled[2].is_nan(), "fill -3000 masked to NaN");
        assert!((scaled[3] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn scale_masks_gdal_nodata_sentinel_too() {
        // 65533 is the fixture's u16 nodata sentinel standing in for fill.
        let raw = [2000.0f32, 65533.0, 4000.0];
        let scaled = scale_modis_ndvi(&raw, Some(65533.0));
        assert!((scaled[0] - 0.2).abs() < 1e-6);
        assert!(scaled[1].is_nan(), "gdal nodata masked");
        assert!((scaled[2] - 0.4).abs() < 1e-6);
    }

    #[test]
    fn stats_over_scaled_masked_band_ignore_fill() {
        let raw = [2000.0f32, 4000.0, -3000.0, 6000.0, 8000.0];
        let scaled = scale_modis_ndvi(&raw, None);
        let stats = zonal_stats(&scaled, None).expect("valid pixels present");
        let stat = |wanted: ZonalStat| {
            stats
                .iter()
                .find(|(s, _)| *s == wanted)
                .expect("stat present")
                .1
        };
        // Valid NDVI: 0.2, 0.4, 0.6, 0.8 -> mean 0.5.
        assert!((stat(ZonalStat::Mean) - 0.5).abs() < 1e-6);
        // 4 valid of 5 pixels.
        assert!((stat(ZonalStat::ValidFraction) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn bbox_envelopes_polygon_ring() {
        let geometry = json!({
            "type": "Polygon",
            "coordinates": [[
                [10.0, 0.0], [10.1, 0.0], [10.1, 0.1], [10.0, 0.1], [10.0, 0.0]
            ]],
        });
        let bbox = bbox_from_geometry(&geometry).expect("bbox");
        assert!((bbox.min_lon - 10.0).abs() < 1e-9);
        assert!((bbox.max_lon - 10.1).abs() < 1e-9);
        assert!((bbox.min_lat - 0.0).abs() < 1e-9);
        assert!((bbox.max_lat - 0.1).abs() < 1e-9);
    }

    #[test]
    fn source_id_maps_to_modis_family() {
        assert_eq!(
            crate::field_timeseries::source_family(Some(&modis_source_id())),
            SOURCE_MODIS
        );
    }

    #[test]
    fn invalid_dates_are_rejected() {
        assert!(validate_date("2026-01-09").is_ok());
        assert!(validate_date("not-a-date").is_err());
        assert!(validate_date("2026-13-40").is_err());
    }
}
