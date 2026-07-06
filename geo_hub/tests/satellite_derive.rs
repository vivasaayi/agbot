//! End-to-end test of `POST /api/satellite/derive` (satellite pipeline
//! batch 6), fully network-free:
//!
//! captured Earth Search item (fixture JSON) -> in-memory object store
//! serving tiled fixture COGs (raster_io `test-util`) -> windowed remote
//! reads -> S2 calibration + SCL mask + NDVI -> GeoTIFF product ->
//! L0/L1/L2 registration -> STAC visibility.
//!
//! Fixture geometry mirrors the real tile 43PFN grid: 10 m bands anchored at
//! (600000, 1300020) in EPSG:32643, SCL at 20 m. The AOI is chosen so the
//! snapped window is exactly 10x10 (10 m) / 5x5 (SCL) — every expected pixel
//! below is hand-computed.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    extract::Extension,
    http::{Request, StatusCode},
    Router,
};
use geo_hub::satellite_derivation::{CogStoreResolver, DerivationError, SatelliteCogResolver};
use geo_hub::state::AppState;
use geo_hub::utm::{utm_to_wgs84, UtmZone};
use geo_hub::{db, server, HubConfig};
use raster_io::object_store::memory::InMemory;
use raster_io::object_store::path::Path as ObjectPath;
use raster_io::object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use raster_io::test_util::{build_tiled_geotiff, FixtureSpec, Pixels};
use raster_io::GeoTiffReader;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const EPSG: u16 = 32643;
const TRANSFORM_10M: [f64; 6] = [600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];
const TRANSFORM_20M: [f64; 6] = [600_000.0, 20.0, 0.0, 1_300_020.0, 0.0, -20.0];
const NODATA: f32 = -9999.0;
/// Hand-computed NDVI of red DN 2000, nir DN 6000 under baseline >= 04.00:
/// red = (2000-1000)/10000 = 0.1, nir = 0.5, (0.5-0.1)/(0.5+0.1).
const EXPECTED_NDVI: f32 = 0.4 / 0.6;

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

/// Fixture bands: red DN 2000 everywhere; nir DN 6000 with one fill (0)
/// pixel at image (10, 10); SCL class 4 (vegetation) with one high-cloud
/// (9) at 20 m image pixel (7, 7).
async fn fixture_store() -> Arc<InMemory> {
    let store = Arc::new(InMemory::new());
    let band = |pixels: Pixels, transform: [f64; 6], size: u32| FixtureSpec {
        width: size,
        height: size,
        tile_width: 16,
        tile_height: 16,
        pixels,
        deflate: true,
        tile_gap: 0,
        epsg: EPSG,
        geo_transform: transform,
        nodata: Some("0".to_string()),
    };

    let red = band(Pixels::U16(vec![2000; 32 * 32]), TRANSFORM_10M, 32);
    let mut nir_pixels = vec![6000u16; 32 * 32];
    nir_pixels[10 * 32 + 10] = 0; // fill DN inside the window (local (0, 0))
    let nir = band(Pixels::U16(nir_pixels), TRANSFORM_10M, 32);
    let mut scl_pixels = vec![4u8; 16 * 16];
    scl_pixels[7 * 16 + 7] = 9; // cloud high probability at window-local (2, 2)
    let scl = band(Pixels::U8(scl_pixels), TRANSFORM_20M, 16);

    for (name, spec) in [("red", &red), ("nir", &nir), ("scl", &scl)] {
        store
            .put(
                &ObjectPath::from(format!("fixtures/{name}.tif")),
                PutPayload::from(build_tiled_geotiff(spec)),
            )
            .await
            .expect("put fixture COG");
    }
    store
}

/// The captured Earth Search item with band hrefs re-pointed at the
/// in-memory store. Everything else (id, proj:epsg, baseline, datetime,
/// bbox) stays as captured.
fn fixture_item() -> serde_json::Value {
    let mut item: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/earth_search_s2_item.json"))
            .expect("parse captured item");
    for (asset, name) in [("red", "red"), ("nir", "nir"), ("scl", "scl")] {
        item["assets"][asset]["href"] = json!(format!("https://cogs.test/fixtures/{name}.tif"));
    }
    item
}

/// AOI: the WGS84 envelope of a projected rect inset 5 m inside the target
/// snap rect (600100, 1299820)-(600200, 1299920). Projecting the envelope
/// back expands it by well under 1 m (grid convergence over 90 m), so the
/// outward snap on the 20 m SCL grid lands exactly on the target rect.
fn aoi() -> [f64; 4] {
    let zone = UtmZone {
        zone: 43,
        north: true,
    };
    let corners = [
        utm_to_wgs84(600_105.0, 1_299_825.0, zone),
        utm_to_wgs84(600_195.0, 1_299_825.0, zone),
        utm_to_wgs84(600_105.0, 1_299_915.0, zone),
        utm_to_wgs84(600_195.0, 1_299_915.0, zone),
    ];
    let min_lon = corners.iter().map(|(_, lon)| *lon).fold(f64::MAX, f64::min);
    let max_lon = corners.iter().map(|(_, lon)| *lon).fold(f64::MIN, f64::max);
    let min_lat = corners.iter().map(|(lat, _)| *lat).fold(f64::MAX, f64::min);
    let max_lat = corners.iter().map(|(lat, _)| *lat).fold(f64::MIN, f64::max);
    [min_lon, min_lat, max_lon, max_lat]
}

async fn ctx(tmp: &TempDir) -> Result<Router> {
    let db_path = tmp.path().join("satellite_derive.db");
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

/// Expected 10x10 NDVI window, hand-computed:
/// - SCL window is (5,5)-(9,9) of the 20 m grid; the cloud at 20 m image
///   pixel (7,7) is window-local (2,2); dilation radius 1 rejects local
///   (1..=3, 1..=3), i.e. 10 m local pixels x,y in 2..=7 (36 masked).
/// - nir fill DN at 10 m image (10,10) = window-local (0,0) (1 nodata).
/// - everything else = (0.5 - 0.1) / (0.5 + 0.1).
fn expected_values() -> Vec<f32> {
    let mut values = vec![EXPECTED_NDVI; 100];
    for y in 2..=7usize {
        for x in 2..=7usize {
            values[y * 10 + x] = NODATA;
        }
    }
    values[0] = NODATA;
    values
}

#[tokio::test]
async fn derive_route_produces_masked_ndvi_geotiff_and_registers_lineage() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = ctx(&tmp).await?;

    let (status, body) = send(&app, "POST", "/api/satellite/derive", Some(derive_body())).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["scene_id"], "S2B_43PFN_20230128_0_L2A");
    assert_eq!(body["index"], "ndvi");
    assert_eq!(body["collection"], "sentinel-2-l2a");
    assert_eq!(body["width_px"], 10);
    assert_eq!(body["height_px"], 10);
    assert_eq!(body["valid_pixels"], 63, "{body}");
    assert_eq!(body["invalid_pixels"], 37);
    assert_eq!(body["execution"], "synchronous");
    assert_eq!(body["evidence"]["pixels"]["reasons"]["masked"], 36);
    assert_eq!(body["evidence"]["pixels"]["reasons"]["fill"], 1);
    // Fetch evidence: windowed reads issued at least one range request per band.
    assert!(
        body["evidence"]["scl_fetch"]["range_requests"]
            .as_u64()
            .unwrap()
            >= 1
    );
    let product_id = body["product_id"].as_str().unwrap().to_string();

    // The written GeoTIFF is a real georeferenced product with hand-computed
    // calibrated NDVI values and nodata where masked/fill.
    let product_path = body["product_path"].as_str().unwrap();
    let mut reader = GeoTiffReader::open(product_path)?;
    let info = reader.info().clone();
    assert_eq!((info.width, info.height), (10, 10));
    assert_eq!(info.epsg, Some(u32::from(EPSG)));
    assert_eq!(
        info.geo_transform,
        Some([600_100.0, 10.0, 0.0, 1_299_920.0, 0.0, -10.0])
    );
    assert_eq!(info.nodata, Some(f64::from(NODATA)));
    let band = reader.read_band()?;
    let values = match band {
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

    // L2 registration with lineage: red + nir bands and the SCL mask.
    let (status, product) = send(
        &app,
        "GET",
        &format!("/api/catalog/products/{product_id}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{product}");
    let product_text = product.to_string();
    assert!(
        product_text.contains("\"l2\"") || product_text.contains("\"L2\""),
        "{product}"
    );

    // The product appears as a STAC item in the ndvi collection with
    // derived_from lineage links and a WGS84 bbox.
    let (status, items) = send(&app, "GET", "/api/stac/collections/ndvi/items", None).await?;
    assert_eq!(status, StatusCode::OK, "{items}");
    assert_eq!(items["numberReturned"], 1, "{items}");
    let item = &items["features"][0];
    assert_eq!(item["id"].as_str().unwrap(), product_id);
    assert_eq!(item["properties"]["processing:level"], "L2");
    assert_eq!(
        item["properties"]["agbot:scene_id"],
        "S2B_43PFN_20230128_0_L2A"
    );
    let bbox: Vec<f64> = item["bbox"]
        .as_array()
        .expect("derived item carries a WGS84 bbox")
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let aoi = aoi();
    assert!(
        bbox[0] <= aoi[0] && bbox[2] >= aoi[2],
        "bbox covers the AOI lon range"
    );
    assert!(
        bbox[1] <= aoi[1] && bbox[3] >= aoi[3],
        "bbox covers the AOI lat range"
    );
    let derived_from: Vec<&serde_json::Value> = item["links"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["rel"] == "derived_from")
        .collect();
    assert_eq!(derived_from.len(), 3, "red + nir + scl lineage: {item}");

    // The scenes collection carries the L0 raw scene + 3 L1 band references.
    let (_, scenes) = send(
        &app,
        "GET",
        "/api/stac/collections/scenes/items?limit=100",
        None,
    )
    .await?;
    assert_eq!(scenes["numberReturned"], 4, "{scenes}");

    // Idempotency: re-deriving the same request lands on the same product.
    let (status, again) = send(&app, "POST", "/api/satellite/derive", Some(derive_body())).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["product_id"].as_str().unwrap(), product_id);
    Ok(())
}

#[tokio::test]
async fn derive_registers_field_scope() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = ctx(&tmp).await?;

    let mut body = derive_body();
    body["field_id"] = json!("field-42");
    body["season_id"] = json!("season-2026-kharif");
    let (status, response) = send(&app, "POST", "/api/satellite/derive", Some(body)).await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    let product_id = response["product_id"].as_str().unwrap().to_string();

    // The L2 index product row carries the requested field/season scope.
    let (status, product) = send(
        &app,
        "GET",
        &format!("/api/catalog/products/{product_id}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{product}");
    assert_eq!(product["field_id"], "field-42", "{product}");
    assert_eq!(product["season_id"], "season-2026-kharif", "{product}");

    // Every registration site threads the scope: filtering the catalog by
    // field_id returns the L0 raw scene, 3 L1 bands (red, nir, scl), and the
    // L2 index product.
    let (status, products) =
        send(&app, "GET", "/api/catalog/products?field_id=field-42", None).await?;
    assert_eq!(status, StatusCode::OK, "{products}");
    let products = products.as_array().expect("product list");
    assert_eq!(products.len(), 5, "L0 + 3xL1 + L2: {products:?}");
    for product in products {
        assert_eq!(product["field_id"], "field-42", "{product}");
        assert_eq!(product["season_id"], "season-2026-kharif", "{product}");
    }

    // Omitting the scope keeps the existing unscoped behavior.
    let tmp2 = TempDir::new()?;
    let app2 = ctx(&tmp2).await?;
    let (status, response) =
        send(&app2, "POST", "/api/satellite/derive", Some(derive_body())).await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    let product_id = response["product_id"].as_str().unwrap();
    let (_, product) = send(
        &app2,
        "GET",
        &format!("/api/catalog/products/{product_id}"),
        None,
    )
    .await?;
    assert_eq!(product["field_id"], serde_json::Value::Null, "{product}");
    assert_eq!(product["season_id"], serde_json::Value::Null, "{product}");
    Ok(())
}

#[tokio::test]
async fn derive_route_rejects_bad_requests_with_reason_codes() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = ctx(&tmp).await?;

    // Unknown index kind.
    let (status, body) = send(
        &app,
        "POST",
        "/api/satellite/derive",
        Some(json!({ "item": fixture_item(), "aoi": aoi(), "index": "bogus" })),
    )
    .await?;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "unknown_index_kind");

    // No item and no collection/item_id pair.
    let (status, body) = send(
        &app,
        "POST",
        "/api/satellite/derive",
        Some(json!({ "aoi": aoi(), "index": "ndvi" })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "missing_item");

    // Landsat items are search-only (requester-pays assets).
    let landsat: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/earth_search_landsat_item.json"))?;
    let (status, body) = send(
        &app,
        "POST",
        "/api/satellite/derive",
        Some(json!({ "item": landsat, "aoi": aoi(), "index": "ndvi" })),
    )
    .await?;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "unsupported_dataset");

    // AOI outside the scene grid.
    let (status, body) = send(
        &app,
        "POST",
        "/api/satellite/derive",
        Some(json!({ "item": fixture_item(), "aoi": [10.0, 50.0, 10.1, 50.1], "index": "ndvi" })),
    )
    .await?;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "aoi_outside_scene");
    Ok(())
}

/// Manual live verification against Earth Search + sentinel-cogs.
///
/// Run with:
/// `cargo test -p geo_hub --test satellite_derive -- --ignored live_earth_search`
///
/// Fetches the captured item's real siblings live, then derives a ~1 km NDVI
/// window from the public sentinel-cogs bucket via HTTPS range reads.
#[tokio::test]
#[ignore = "network: hits earth-search.aws.element84.com and sentinel-cogs.s3.us-west-2.amazonaws.com"]
async fn live_earth_search_derivation_smoke() -> Result<()> {
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("live.db");
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
    let app = server::build_router(state); // default UrlCogResolver

    // ~1 km AOI inside tile 43PFN (Karnataka/Tamil Nadu border region).
    let (status, body) = send(
        &app,
        "POST",
        "/api/satellite/derive",
        Some(json!({
            "collection": "sentinel-2-l2a",
            "item_id": "S2B_43PFN_20230128_0_L2A",
            "aoi": [76.64, 11.34, 76.65, 11.35],
            "index": "ndvi",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["valid_pixels"].as_u64().unwrap() > 0, "{body}");
    println!(
        "live derivation: {} ({}x{}, {} valid px) -> {}",
        body["product_id"],
        body["width_px"],
        body["height_px"],
        body["valid_pixels"],
        body["product_path"]
    );
    Ok(())
}
