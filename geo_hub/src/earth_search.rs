//! Earth Search (Element84) STAC client (satellite pipeline batch 6).
//!
//! `https://earth-search.aws.element84.com/v1` serves Sentinel-2 L2A COGs
//! from the public `sentinel-cogs` bucket with **no auth**, which makes it
//! the free path for local band reads (the Planetary Computer mirror queried
//! by `landsat.rs` requires SAS token signing for asset access).
//!
//! # Verified asset naming (captured fixture, 2026-07)
//! `sentinel-2-l2a` items name band assets `blue`, `green`, `red`, `nir`
//! (B08, 10 m), `nir08` (B8A, 20 m), `rededge1..3`, `swir16`, `swir22`, and
//! the scene classification as `scl`; hrefs are plain HTTPS COG URLs on
//! `sentinel-cogs.s3.us-west-2.amazonaws.com`. The CRS is
//! `properties["proj:epsg"]` (integer), and each asset carries
//! `proj:shape`/`proj:transform`. `landsat-c2-l2` items use `red`, `green`,
//! `blue`, `nir08`, `swir16`, `swir22`, `qa_pixel` — but their hrefs point at
//! the **requester-pays** `s3://usgs-landsat` bucket, so Landsat is search
//! metadata only here (no free band reads). Both shapes are pinned by the
//! fixtures in `tests/fixtures/earth_search_*.json` (captured live).

use anyhow::{anyhow, Context, Result};
use chrono::{Duration as ChronoDuration, NaiveDate};
use imagery_processor::IndexBandRole;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

use crate::landsat::SatelliteDataset;

pub const EARTH_SEARCH_API: &str = "https://earth-search.aws.element84.com/v1";

/// Earth Search collection id for a dataset (same ids as Planetary Computer).
pub fn earth_search_collection_id(dataset: SatelliteDataset) -> &'static str {
    match dataset {
        SatelliteDataset::Landsat => "landsat-c2-l2",
        SatelliteDataset::Sentinel2 => "sentinel-2-l2a",
    }
}

/// One STAC asset of an Earth Search item. Unknown fields are dropped.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EarthSearchAsset {
    pub href: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// `[height, width]` of the asset grid.
    #[serde(rename = "proj:shape", skip_serializing_if = "Option::is_none")]
    pub proj_shape: Option<Vec<u64>>,
    /// Row-major affine `[a, b, c, d, e, f]` = GDAL `[c, a, b, f, d, e]`.
    #[serde(rename = "proj:transform", skip_serializing_if = "Option::is_none")]
    pub proj_transform: Option<Vec<f64>>,
}

/// A STAC item from Earth Search. `properties` stays schemaless because the
/// per-collection property sets differ; typed accessors below pull out what
/// the derivation pipeline needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarthSearchItem {
    pub id: String,
    #[serde(default)]
    pub collection: Option<String>,
    #[serde(default)]
    pub bbox: Option<Vec<f64>>,
    #[serde(default)]
    pub geometry: Option<serde_json::Value>,
    #[serde(default)]
    pub properties: serde_json::Value,
    #[serde(default)]
    pub assets: BTreeMap<String, EarthSearchAsset>,
}

impl EarthSearchItem {
    pub fn datetime(&self) -> Option<&str> {
        self.properties.get("datetime")?.as_str()
    }

    pub fn cloud_cover(&self) -> Option<f64> {
        self.properties.get("eo:cloud_cover")?.as_f64()
    }

    /// CRS EPSG code: integer `proj:epsg` (Earth Search) or `proj:code`
    /// `"EPSG:<n>"` (newer STAC proj extension).
    pub fn epsg(&self) -> Option<u32> {
        if let Some(code) = self.properties.get("proj:epsg").and_then(|v| v.as_u64()) {
            return u32::try_from(code).ok();
        }
        self.properties
            .get("proj:code")?
            .as_str()?
            .strip_prefix("EPSG:")?
            .parse()
            .ok()
    }

    /// Sentinel-2 processing baseline as a number (e.g. `"05.09"` -> 5.09).
    /// Decides the DN offset: baseline >= 04.00 is `(DN - 1000) / 10000`.
    pub fn s2_processing_baseline(&self) -> Option<f64> {
        self.properties
            .get("s2:processing_baseline")?
            .as_str()?
            .parse()
            .ok()
    }

    pub fn asset(&self, key: &str) -> Option<&EarthSearchAsset> {
        self.assets.get(key)
    }
}

/// Verified Sentinel-2 L2A asset key for an index band role.
pub fn s2_asset_key(role: IndexBandRole) -> &'static str {
    match role {
        IndexBandRole::Blue => "blue",
        IndexBandRole::Green => "green",
        IndexBandRole::Red => "red",
        IndexBandRole::Nir => "nir",
        IndexBandRole::RedEdge => "rededge1",
        IndexBandRole::Swir1 => "swir16",
        IndexBandRole::Swir2 => "swir22",
    }
}

/// Sentinel-2 scene-classification asset key.
pub const S2_SCL_ASSET_KEY: &str = "scl";

/// Verified Landsat C2 L2 asset key for an index band role (search metadata
/// only: Earth Search Landsat hrefs are requester-pays `s3://usgs-landsat`).
pub fn landsat_asset_key(role: IndexBandRole) -> Option<&'static str> {
    match role {
        IndexBandRole::Blue => Some("blue"),
        IndexBandRole::Green => Some("green"),
        IndexBandRole::Red => Some("red"),
        IndexBandRole::Nir => Some("nir08"),
        IndexBandRole::RedEdge => None,
        IndexBandRole::Swir1 => Some("swir16"),
        IndexBandRole::Swir2 => Some("swir22"),
    }
}

#[derive(Debug, Deserialize)]
struct ItemCollection {
    #[serde(default)]
    features: Vec<EarthSearchItem>,
}

/// Parse an Earth Search `POST /search` response body. Pure.
pub fn parse_search_response(body: &str) -> Result<Vec<EarthSearchItem>> {
    let collection: ItemCollection =
        serde_json::from_str(body).context("failed to parse Earth Search search response")?;
    Ok(collection.features)
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")
}

/// Search Earth Search for scenes around a point within a day window,
/// filtered by max cloud cover, best (least cloudy) first.
pub async fn search_items(
    dataset: SatelliteDataset,
    latitude: f64,
    longitude: f64,
    target_date: &str,
    days: u8,
    limit: usize,
    max_cloud_cover: f64,
) -> Result<Vec<EarthSearchItem>> {
    let date = NaiveDate::parse_from_str(target_date, "%Y-%m-%d")
        .with_context(|| format!("invalid target date: {target_date}"))?;
    let half_window = i64::from(days.saturating_sub(1)) / 2;
    let start = date - ChronoDuration::days(half_window);
    let end = date + ChronoDuration::days(i64::from(days.max(1)) - half_window - 1);
    let body = serde_json::json!({
        "collections": [earth_search_collection_id(dataset)],
        "intersects": { "type": "Point", "coordinates": [longitude, latitude] },
        "datetime": format!("{start}T00:00:00Z/{end}T23:59:59Z"),
        "limit": limit.clamp(1, 25),
        "query": { "eo:cloud_cover": { "lt": max_cloud_cover } },
    });

    let response = http_client()?
        .post(format!("{EARTH_SEARCH_API}/search"))
        .header(reqwest::header::USER_AGENT, "agbot-geo-hub/0.1")
        .json(&body)
        .send()
        .await
        .context("failed to call Earth Search STAC search")?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "Earth Search STAC search failed with {status}: {text}"
        ));
    }
    let text = response.text().await?;
    let mut items = parse_search_response(&text)?;
    items.sort_by(|left, right| {
        let left_cloud = left.cloud_cover().unwrap_or(f64::MAX);
        let right_cloud = right.cloud_cover().unwrap_or(f64::MAX);
        left_cloud
            .partial_cmp(&right_cloud)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(items)
}

/// Earth Search collection id for a subscription dataset key. Accepts both
/// the short subscription form (`sentinel2`, `landsat`) and the collection
/// id itself. `hls` (and anything else) has no Earth Search collection and
/// maps to `None` — HLS lives on NASA LP DAAC, not Element84.
pub fn collection_for_dataset(dataset: &str) -> Option<&'static str> {
    match dataset {
        "sentinel2" | "sentinel-2-l2a" => Some("sentinel-2-l2a"),
        "landsat" | "landsat-c2-l2" => Some("landsat-c2-l2"),
        _ => None,
    }
}

/// Build the STAC `POST /search` body for an explicit bbox + date range.
/// Pure, so the request shape is unit-testable without network access.
pub fn build_range_search_body(
    collections: &[&str],
    bbox: [f64; 4],
    start_iso: &str,
    end_iso: &str,
    max_cloud_cover: f64,
    limit: usize,
) -> serde_json::Value {
    serde_json::json!({
        "collections": collections,
        "bbox": bbox,
        "datetime": format!("{start_iso}/{end_iso}"),
        "limit": limit.clamp(1, 100),
        "query": { "eo:cloud_cover": { "lt": max_cloud_cover } },
    })
}

/// Sort items chronologically (oldest first, undated items last), then by id
/// for a stable order. Pure.
pub fn sort_items_by_datetime(items: &mut [EarthSearchItem]) {
    items.sort_by(|left, right| {
        match (left.datetime(), right.datetime()) {
            (Some(l), Some(r)) => l.cmp(r),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| left.id.cmp(&right.id))
    });
}

/// Search Earth Search over an explicit bbox (WGS84
/// `[min_lon, min_lat, max_lon, max_lat]`, e.g. a field-boundary envelope)
/// and an explicit ISO date range, filtered by max cloud cover. Returns
/// items oldest first — the natural order for discover fan-out.
pub async fn search_items_range(
    collections: &[&str],
    bbox: [f64; 4],
    start_iso: &str,
    end_iso: &str,
    max_cloud_cover: f64,
    limit: usize,
) -> Result<Vec<EarthSearchItem>> {
    let body = build_range_search_body(
        collections,
        bbox,
        start_iso,
        end_iso,
        max_cloud_cover,
        limit,
    );
    let response = http_client()?
        .post(format!("{EARTH_SEARCH_API}/search"))
        .header(reqwest::header::USER_AGENT, "agbot-geo-hub/0.1")
        .json(&body)
        .send()
        .await
        .context("failed to call Earth Search STAC search")?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "Earth Search STAC search failed with {status}: {text}"
        ));
    }
    let text = response.text().await?;
    let mut items = parse_search_response(&text)?;
    sort_items_by_datetime(&mut items);
    Ok(items)
}

/// Fetch a single item by collection + id.
pub async fn fetch_item(collection: &str, item_id: &str) -> Result<EarthSearchItem> {
    let url = format!("{EARTH_SEARCH_API}/collections/{collection}/items/{item_id}");
    let response = http_client()?
        .get(&url)
        .header(reqwest::header::USER_AGENT, "agbot-geo-hub/0.1")
        .send()
        .await
        .with_context(|| format!("failed to fetch Earth Search item {item_id}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "Earth Search item fetch failed with {status}: {text}"
        ));
    }
    let text = response.text().await?;
    serde_json::from_str(&text).context("failed to parse Earth Search item")
}

#[cfg(test)]
mod tests {
    use super::*;

    const S2_FIXTURE: &str = include_str!("../tests/fixtures/earth_search_s2_item.json");
    const LANDSAT_FIXTURE: &str = include_str!("../tests/fixtures/earth_search_landsat_item.json");

    fn s2_item() -> EarthSearchItem {
        serde_json::from_str(S2_FIXTURE).expect("parse captured S2 item")
    }

    #[test]
    fn s2_fixture_exposes_verified_band_assets_and_projection() {
        let item = s2_item();
        assert_eq!(item.id, "S2B_43PFN_20230128_0_L2A");
        assert_eq!(item.collection.as_deref(), Some("sentinel-2-l2a"));
        assert_eq!(item.epsg(), Some(32643));
        assert_eq!(item.datetime(), Some("2023-01-28T05:25:49.364000Z"));
        assert_eq!(item.s2_processing_baseline(), Some(5.09));
        assert!(item.cloud_cover().unwrap() < 1.0);

        // Role -> verified Earth Search asset key -> COG href.
        let red = item.asset(s2_asset_key(IndexBandRole::Red)).unwrap();
        assert!(red.href.ends_with("S2B_43PFN_20230128_0_L2A/B04.tif"));
        assert!(red
            .href
            .starts_with("https://sentinel-cogs.s3.us-west-2.amazonaws.com/"));
        assert_eq!(red.proj_shape.as_deref(), Some(&[10980, 10980][..]));
        assert_eq!(
            red.proj_transform.as_deref(),
            Some(&[10.0, 0.0, 600_000.0, 0.0, -10.0, 1_300_020.0][..])
        );

        let nir = item.asset(s2_asset_key(IndexBandRole::Nir)).unwrap();
        assert!(nir.href.ends_with("/B08.tif"), "nir asset is B08 (10 m)");
        let swir1 = item.asset(s2_asset_key(IndexBandRole::Swir1)).unwrap();
        assert!(swir1.href.ends_with("/B11.tif"));
        assert_eq!(
            swir1.proj_transform.as_deref().map(|t| t[0]),
            Some(20.0),
            "swir16 is a 20 m grid"
        );
        let scl = item.asset(S2_SCL_ASSET_KEY).unwrap();
        assert!(scl.href.ends_with("/SCL.tif"));
    }

    #[test]
    fn landsat_fixture_assets_are_requester_pays_s3() {
        let item: EarthSearchItem =
            serde_json::from_str(LANDSAT_FIXTURE).expect("parse captured Landsat item");
        assert_eq!(item.collection.as_deref(), Some("landsat-c2-l2"));
        assert_eq!(item.epsg(), Some(32614));
        for role in [IndexBandRole::Red, IndexBandRole::Nir, IndexBandRole::Swir1] {
            let key = landsat_asset_key(role).unwrap();
            let asset = item.asset(key).unwrap();
            assert!(
                asset.href.starts_with("s3://usgs-landsat/"),
                "Landsat band {key} lives on the requester-pays bucket: {}",
                asset.href
            );
        }
        assert!(item.asset("qa_pixel").is_some());
        assert_eq!(landsat_asset_key(IndexBandRole::RedEdge), None);
    }

    #[test]
    fn search_response_parsing_yields_items() {
        let body = serde_json::json!({
            "type": "FeatureCollection",
            "features": [serde_json::from_str::<serde_json::Value>(S2_FIXTURE).unwrap()],
        })
        .to_string();
        let items = parse_search_response(&body).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "S2B_43PFN_20230128_0_L2A");

        assert!(parse_search_response("{}").unwrap().is_empty());
        assert!(parse_search_response("not json").is_err());
    }

    #[test]
    fn collection_for_dataset_maps_short_and_full_forms() {
        assert_eq!(collection_for_dataset("sentinel2"), Some("sentinel-2-l2a"));
        assert_eq!(
            collection_for_dataset("sentinel-2-l2a"),
            Some("sentinel-2-l2a")
        );
        assert_eq!(collection_for_dataset("landsat"), Some("landsat-c2-l2"));
        assert_eq!(
            collection_for_dataset("landsat-c2-l2"),
            Some("landsat-c2-l2")
        );
        assert_eq!(
            collection_for_dataset("hls"),
            None,
            "HLS is not on Earth Search"
        );
        assert_eq!(collection_for_dataset("modis"), None);
    }

    #[test]
    fn range_search_body_carries_bbox_range_and_cloud_filter() {
        let body = build_range_search_body(
            &["sentinel-2-l2a"],
            [76.64, 11.34, 76.65, 11.35],
            "2026-06-22T00:00:00Z",
            "2026-07-06T00:00:00Z",
            60.0,
            50,
        );
        assert_eq!(body["collections"], serde_json::json!(["sentinel-2-l2a"]));
        assert_eq!(
            body["bbox"],
            serde_json::json!([76.64, 11.34, 76.65, 11.35])
        );
        assert_eq!(
            body["datetime"],
            "2026-06-22T00:00:00Z/2026-07-06T00:00:00Z"
        );
        assert_eq!(body["limit"], 50);
        assert_eq!(body["query"]["eo:cloud_cover"]["lt"], 60.0);
        // Limit clamps into the API-accepted page size.
        let clamped = build_range_search_body(&["x"], [0.0; 4], "a", "b", 10.0, 100_000);
        assert_eq!(clamped["limit"], 100);
    }

    #[test]
    fn sort_items_by_datetime_orders_oldest_first_undated_last() {
        let item = |id: &str, datetime: Option<&str>| -> EarthSearchItem {
            let properties = match datetime {
                Some(dt) => serde_json::json!({ "datetime": dt }),
                None => serde_json::json!({}),
            };
            serde_json::from_value(serde_json::json!({
                "id": id,
                "properties": properties,
                "assets": {},
            }))
            .unwrap()
        };
        let mut items = vec![
            item("c", Some("2026-02-01T00:00:00Z")),
            item("undated", None),
            item("a", Some("2026-01-01T00:00:00Z")),
            item("b", Some("2026-01-01T00:00:00Z")),
        ];
        sort_items_by_datetime(&mut items);
        let ids: Vec<&str> = items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c", "undated"]);
    }

    #[test]
    fn proj_code_string_form_is_also_accepted() {
        let item: EarthSearchItem = serde_json::from_str(
            r#"{"id":"x","properties":{"proj:code":"EPSG:32719"},"assets":{}}"#,
        )
        .unwrap();
        assert_eq!(item.epsg(), Some(32719));
    }
}
