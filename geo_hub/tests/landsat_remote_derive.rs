//! Landsat remote derivation tests (satellite batch S-10, Task B),
//! network-free: `POST /api/satellite/derive` accepts `landsat-c2-l2` STAC
//! items, resolves the common band asset keys (`red`, `nir08`, ...), applies
//! Collection-2 Level-2 scaling (DN * 0.0000275 - 0.2) and the QA_PIXEL
//! clear mask, and registers L0/L1/L2 lineage — mirroring the Sentinel-2
//! fixture style in `tests/satellite_derive.rs`.
//!
//! Fixture geometry: 30 m bands anchored at (600000, 4700000) in EPSG:32614
//! (the captured item's zone 14N). The AOI is chosen so the snapped window
//! is exactly 4x4; every expected pixel below is hand-computed.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    extract::Extension,
    http::{Request, StatusCode},
    Router,
};
use geo_hub::earth_search::{landsat_asset_key, EarthSearchItem};
use geo_hub::landsat_derive::{instrument_for_scene, LandsatInstrument};
use geo_hub::satellite_derivation::{CogStoreResolver, DerivationError, SatelliteCogResolver};
use geo_hub::state::AppState;
use geo_hub::utm::{utm_to_wgs84, UtmZone};
use geo_hub::{db, server, HubConfig};
use imagery_processor::IndexBandRole;
use raster_io::object_store::memory::InMemory;
use raster_io::object_store::path::Path as ObjectPath;
use raster_io::object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use raster_io::test_util::{build_tiled_geotiff, FixtureSpec, Pixels};
use raster_io::GeoTiffReader;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const EPSG: u16 = 32614;
const TRANSFORM_30M: [f64; 6] = [600_000.0, 30.0, 0.0, 4_700_000.0, 0.0, -30.0];
const NODATA: f32 = -9999.0;
/// Hand-computed NDVI under Collection-2 L2 scaling (DN * 0.0000275 - 0.2):
/// red DN 10000 -> 0.075, nir DN 20000 -> 0.35, (0.35-0.075)/(0.35+0.075).
const EXPECTED_NDVI: f32 = 0.275 / 0.425;

/// Resolver mapping https://cogs.test/<path> to the in-memory store.
struct MemResolver(Arc<InMemory>);

impl CogStoreResolver for MemResolver {
    fn resolve(&self, href: &str) -> Result<(Arc<dyn ObjectStore>, String), DerivationError> {
        let path = href
            .strip_prefix("https://cogs.test/")
            .unwrap_or(href)
            .to_string();
        Ok((self.0.clone(), path))
    }
}

/// Fixture bands (30 m, 32x32): red DN 10000 everywhere; nir08 DN 20000 with
/// one fill (0) pixel at image (8, 8); QA_PIXEL "clear" (64) with one cloud
/// (bit 3 = 8) at image (10, 10).
async fn fixture_store() -> Arc<InMemory> {
    let store = Arc::new(InMemory::new());
    let band = |pixels: Pixels, nodata: &str| FixtureSpec {
        width: 32,
        height: 32,
        tile_width: 16,
        tile_height: 16,
        pixels,
        deflate: true,
        tile_gap: 0,
        epsg: EPSG,
        geo_transform: TRANSFORM_30M,
        nodata: Some(nodata.to_string()),
    };

    let red = band(Pixels::U16(vec![10_000; 32 * 32]), "0");
    let mut nir_pixels = vec![20_000u16; 32 * 32];
    nir_pixels[8 * 32 + 8] = 0; // fill DN inside the window (local (0, 0))
    let nir08 = band(Pixels::U16(nir_pixels), "0");
    let mut qa_pixels = vec![64u16; 32 * 32]; // clear bit
    qa_pixels[10 * 32 + 10] = 8; // cloud bit at window-local (2, 2)
    let qa_pixel = band(Pixels::U16(qa_pixels), "1");

    for (name, spec) in [("red", &red), ("nir08", &nir08), ("qa_pixel", &qa_pixel)] {
        store
            .put(
                &ObjectPath::from(format!("fixtures/landsat/{name}.tif")),
                PutPayload::from(build_tiled_geotiff(spec)),
            )
            .await
            .expect("put fixture COG");
    }
    store
}

/// The captured Earth Search Landsat item with band hrefs re-pointed at the
/// in-memory store. Everything else (id, proj:epsg, datetime, bbox) stays as
/// captured — including the ETM+ `_SR_B3`/`_SR_B4` file naming behind the
/// common `red`/`nir08` asset keys.
fn fixture_item() -> serde_json::Value {
    let mut item: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/earth_search_landsat_item.json"))
            .expect("parse captured Landsat item");
    for name in ["red", "nir08", "qa_pixel"] {
        item["assets"][name]["href"] =
            json!(format!("https://cogs.test/fixtures/landsat/{name}.tif"));
    }
    item
}

/// AOI: the WGS84 envelope of a projected rect inset 5 m inside the target
/// snap rect (600240, 4699640)-(600360, 4699760); the outward snap on the
/// 30 m grid lands exactly on the target 4x4 window at pixel (8, 8).
fn aoi() -> [f64; 4] {
    let zone = UtmZone {
        zone: 14,
        north: true,
    };
    let corners = [
        utm_to_wgs84(600_245.0, 4_699_645.0, zone),
        utm_to_wgs84(600_355.0, 4_699_645.0, zone),
        utm_to_wgs84(600_245.0, 4_699_755.0, zone),
        utm_to_wgs84(600_355.0, 4_699_755.0, zone),
    ];
    let min_lon = corners.iter().map(|(_, lon)| *lon).fold(f64::MAX, f64::min);
    let max_lon = corners.iter().map(|(_, lon)| *lon).fold(f64::MIN, f64::max);
    let min_lat = corners.iter().map(|(lat, _)| *lat).fold(f64::MAX, f64::min);
    let max_lat = corners.iter().map(|(lat, _)| *lat).fold(f64::MIN, f64::max);
    [min_lon, min_lat, max_lon, max_lat]
}

async fn ctx(tmp: &TempDir) -> Result<Router> {
    let db_path = tmp.path().join("landsat_remote_derive.db");
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let state = AppState {
        pool,
        config: Arc::new(config),
        scene_search_cache: Default::default(),
    };
    let resolver = SatelliteCogResolver(Arc::new(MemResolver(fixture_store().await)));
    Ok(server::build_router(state).layer(Extension(resolver)))
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> Result<(StatusCode, serde_json::Value)> {
    let req = Request::builder().method(method).uri(uri);
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&b)?))?,
        None => req.body(Body::empty())?,
    };
    let response = app.clone().oneshot(req).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024).await?;
    Ok((
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    ))
}

fn derive_body() -> serde_json::Value {
    json!({ "item": fixture_item(), "aoi": aoi(), "index": "ndvi" })
}

/// Expected 4x4 NDVI window, hand-computed:
/// - QA cloud at image (10, 10) = window-local (2, 2) -> masked (1 nodata).
/// - nir08 fill DN at image (8, 8) = window-local (0, 0) -> fill (1 nodata).
/// - everything else = (0.35 - 0.075) / (0.35 + 0.075).
fn expected_values() -> Vec<f32> {
    let mut values = vec![EXPECTED_NDVI; 16];
    values[2 * 4 + 2] = NODATA;
    values[0] = NODATA;
    values
}

#[tokio::test]
async fn landsat_remote_derive_masks_qa_pixel() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = ctx(&tmp).await?;

    let (status, body) = send(&app, "POST", "/api/satellite/derive", Some(derive_body())).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["scene_id"], "LE07_L2SP_028031_20230721_02_T1");
    assert_eq!(body["collection"], "landsat-c2-l2");
    assert_eq!(body["index"], "ndvi");
    assert_eq!(body["width_px"], 4);
    assert_eq!(body["height_px"], 4);
    assert_eq!(body["valid_pixels"], 14, "{body}");
    assert_eq!(body["invalid_pixels"], 2);
    assert_eq!(body["evidence"]["pixels"]["reasons"]["masked"], 1);
    assert_eq!(body["evidence"]["pixels"]["reasons"]["fill"], 1);
    // Landsat evidence: C2 L2 calibration + QA_PIXEL mask scheme.
    assert_eq!(
        body["evidence"]["calibration"]["profile"],
        "landsat_c2l2_sr"
    );
    assert_eq!(body["evidence"]["mask"]["scheme"], "qa_pixel");
    assert_eq!(body["evidence"]["mask"]["applied"], true);
    assert!(
        body["evidence"]["mask_fetch"]["range_requests"]
            .as_u64()
            .unwrap()
            >= 1
    );
    let product_id = body["product_id"].as_str().unwrap().to_string();

    // The written GeoTIFF is a real 30 m georeferenced product with
    // hand-computed calibrated NDVI values and nodata where masked/fill.
    let product_path = body["product_path"].as_str().unwrap();
    let mut reader = GeoTiffReader::open(product_path)?;
    let info = reader.info().clone();
    assert_eq!((info.width, info.height), (4, 4));
    assert_eq!(info.epsg, Some(u32::from(EPSG)));
    assert_eq!(
        info.geo_transform,
        Some([600_240.0, 30.0, 0.0, 4_699_760.0, 0.0, -30.0])
    );
    assert_eq!(info.nodata, Some(f64::from(NODATA)));
    let values = match reader.read_band()? {
        raster_io::RasterBand::F32(values) => values,
        other => panic!("expected f32 band, got {other:?}"),
    };
    let expected = expected_values();
    assert_eq!(values.len(), expected.len());
    for (index, (actual, expected)) in values.iter().zip(&expected).enumerate() {
        assert!(
            (actual - expected).abs() < 1e-6,
            "pixel {index}: {actual} != {expected}"
        );
    }

    // STAC lineage: red + nir08 bands and the QA_PIXEL mask.
    let (status, items) = send(&app, "GET", "/api/stac/collections/ndvi/items", None).await?;
    assert_eq!(status, StatusCode::OK, "{items}");
    assert_eq!(items["numberReturned"], 1, "{items}");
    let item = &items["features"][0];
    assert_eq!(item["id"].as_str().unwrap(), product_id);
    let derived_from: Vec<&serde_json::Value> = item["links"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["rel"] == "derived_from")
        .collect();
    assert_eq!(
        derived_from.len(),
        3,
        "red + nir08 + qa_pixel lineage: {item}"
    );

    // Idempotency: re-deriving the same request lands on the same product.
    let (status, again) = send(&app, "POST", "/api/satellite/derive", Some(derive_body())).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["product_id"].as_str().unwrap(), product_id);
    Ok(())
}

#[tokio::test]
async fn landsat_derive_without_qa_pixel_is_unmasked() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = ctx(&tmp).await?;

    // QA_PIXEL is applied opportunistically (mirrors landsat_derive's
    // local-band behavior): an item without the asset still derives, with the
    // mask recorded as not applied and only the nir fill pixel invalid.
    let mut item = fixture_item();
    item["assets"]
        .as_object_mut()
        .unwrap()
        .remove("qa_pixel")
        .expect("fixture has qa_pixel");
    let (status, body) = send(
        &app,
        "POST",
        "/api/satellite/derive",
        Some(json!({ "item": item, "aoi": aoi(), "index": "ndvi" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["valid_pixels"], 15, "{body}");
    assert_eq!(body["invalid_pixels"], 1);
    assert_eq!(body["evidence"]["pixels"]["reasons"]["fill"], 1);
    assert_eq!(body["evidence"]["mask"]["scheme"], "qa_pixel");
    assert_eq!(body["evidence"]["mask"]["applied"], false);

    // Lineage: only the two band reads, no mask product.
    let product_id = body["product_id"].as_str().unwrap();
    let (_, items) = send(&app, "GET", "/api/stac/collections/ndvi/items", None).await?;
    let item = &items["features"][0];
    assert_eq!(item["id"].as_str().unwrap(), product_id);
    let derived_from = item["links"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["rel"] == "derived_from")
        .count();
    assert_eq!(derived_from, 2, "red + nir08 only: {item}");
    Ok(())
}

/// Planetary Computer / Earth Search `landsat-c2-l2` items use the same
/// common STAC asset keys (`red`, `nir08`, `swir16`, ...) across every
/// instrument family — the TM/ETM+ vs OLI band-numbering divergence
/// (SR_B3/SR_B4 vs SR_B4/SR_B5) is hidden behind the keys. The captured
/// fixture is an ETM+ (LE07) item and proves it: `red` points at `_SR_B3`
/// and `nir08` at `_SR_B4`, so no per-instrument key table is needed at the
/// STAC layer.
#[test]
fn tm_and_oli_band_keys_resolve() {
    let item: EarthSearchItem =
        serde_json::from_str(include_str!("fixtures/earth_search_landsat_item.json"))
            .expect("parse captured Landsat item");
    assert_eq!(
        instrument_for_scene(&item.id),
        LandsatInstrument::TmEtm,
        "the captured item is an ETM+ scene"
    );

    let red = item.asset(landsat_asset_key(IndexBandRole::Red).unwrap());
    assert!(
        red.unwrap().href.ends_with("_SR_B3.TIF"),
        "ETM+ red is SR_B3 behind the common `red` key"
    );
    let nir = item.asset(landsat_asset_key(IndexBandRole::Nir).unwrap());
    assert!(
        nir.unwrap().href.ends_with("_SR_B4.TIF"),
        "ETM+ nir is SR_B4 behind the common `nir08` key"
    );

    // The role mapping itself is instrument-independent.
    assert_eq!(landsat_asset_key(IndexBandRole::Red), Some("red"));
    assert_eq!(landsat_asset_key(IndexBandRole::Nir), Some("nir08"));
    assert_eq!(landsat_asset_key(IndexBandRole::Swir1), Some("swir16"));
    assert_eq!(landsat_asset_key(IndexBandRole::Swir2), Some("swir22"));
    assert_eq!(
        landsat_asset_key(IndexBandRole::RedEdge),
        None,
        "Landsat has no red-edge band"
    );
}
