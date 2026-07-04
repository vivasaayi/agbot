//! Integration tests for the internal STAC API (`/api/stac/...`, satellite
//! pipeline batch 4): landing-page conformance, collections derived from the
//! seeded product graph, item retrieval with lineage links, bbox+datetime
//! search, pagination next-links, and skipped-record surfacing.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

async fn ctx(tmp: &TempDir) -> Result<Router> {
    let db_path = tmp.path().join("stac_api.db");
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
    Ok(server::build_router(state))
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
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await?;
    Ok((
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    ))
}

/// Catalog draft with a WGS84 bbox and a temporal instant.
fn draft(
    level: &str,
    kind: &str,
    scene: &str,
    bbox: [f64; 4],
    datetime: &str,
    inputs: serde_json::Value,
) -> serde_json::Value {
    json!({
        "level": level,
        "kind": kind,
        "algorithm_id": format!("{kind}.compute"),
        "algorithm_version": "1.0.0",
        "parameters": { "scene_id": scene },
        "inputs": inputs,
        "scope": {
            "field_id": "field-1",
            "scene_id": scene,
            "temporal_start": datetime,
            "temporal_end": datetime
        },
        "spatial_ref": {
            "georeferenced": true,
            "crs": "EPSG:4326",
            "bbox": {
                "min_lon": bbox[0],
                "min_lat": bbox[1],
                "max_lon": bbox[2],
                "max_lat": bbox[3]
            }
        },
        "gsd_m_per_px": 10.0,
        "artifact": { "path": format!("/data/{scene}-{kind}.tif"), "format": "tif" }
    })
}

async fn register(app: &Router, draft: serde_json::Value) -> Result<String> {
    let (status, body) = send(app, "POST", "/api/catalog/products", Some(draft)).await?;
    assert_eq!(status, StatusCode::OK, "register failed: {body}");
    Ok(body["product_id"].as_str().unwrap().to_string())
}

const BBOX_A: [f64; 4] = [-96.5, 41.0, -96.4, 41.1];
const BBOX_B: [f64; 4] = [10.0, 50.0, 10.1, 50.1];
const T_JUNE_1: &str = "2026-06-01T00:00:00Z";
const T_JUNE_20: &str = "2026-06-20T00:00:00Z";

/// Seed one L1 band (scenes collection) and two L2 ndvi products (one derived
/// from the band) at different places/times. Returns (band_id, ndvi_a, ndvi_b).
async fn seed_graph(app: &Router) -> Result<(String, String, String)> {
    let band_id = register(
        app,
        draft("l1", "band_nir", "scene-a", BBOX_A, T_JUNE_1, json!([])),
    )
    .await?;
    let ndvi_a = register(
        app,
        draft(
            "l2",
            "ndvi",
            "scene-a",
            BBOX_A,
            T_JUNE_1,
            json!([{ "product_id": band_id, "role": "band:nir" }]),
        ),
    )
    .await?;
    let ndvi_b = register(
        app,
        draft("l2", "ndvi", "scene-b", BBOX_B, T_JUNE_20, json!([])),
    )
    .await?;
    Ok((band_id, ndvi_a, ndvi_b))
}

#[tokio::test]
async fn landing_page_declares_conformance_and_links() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = ctx(&tmp).await?;

    let (status, body) = send(&app, "GET", "/api/stac", None).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], "Catalog");
    assert_eq!(body["stac_version"], "1.1.0");
    let conforms: Vec<&str> = body["conformsTo"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for class in [
        "https://api.stacspec.org/v1.0.0/core",
        "https://api.stacspec.org/v1.0.0/collections",
        "https://api.stacspec.org/v1.0.0/item-search",
    ] {
        assert!(conforms.contains(&class), "missing {class}");
    }
    let rels: Vec<&str> = body["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["rel"].as_str().unwrap())
        .collect();
    for rel in ["self", "data", "search"] {
        assert!(rels.contains(&rel), "missing link rel {rel}");
    }

    let (status, conformance) = send(&app, "GET", "/api/stac/conformance", None).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(conformance["conformsTo"].as_array().unwrap().len() >= 3);
    Ok(())
}

#[tokio::test]
async fn collections_reflect_seeded_products() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = ctx(&tmp).await?;
    seed_graph(&app).await?;

    let (status, body) = send(&app, "GET", "/api/stac/collections", None).await?;
    assert_eq!(status, StatusCode::OK);
    let collections = body["collections"].as_array().unwrap();
    let ids: Vec<&str> = collections
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["ndvi", "scenes"], "one per level/kind grouping");

    // The ndvi collection's extent is the union of both items.
    let ndvi = &collections[0];
    assert_eq!(ndvi["type"], "Collection");
    assert_eq!(ndvi["stac_version"], "1.1.0");
    let bbox = ndvi["extent"]["spatial"]["bbox"][0].as_array().unwrap();
    let bbox: Vec<f64> = bbox.iter().map(|v| v.as_f64().unwrap()).collect();
    assert_eq!(bbox, vec![-96.5, 41.0, 10.1, 50.1]);
    let interval = &ndvi["extent"]["temporal"]["interval"][0];
    assert_eq!(interval[0], T_JUNE_1);
    assert_eq!(interval[1], T_JUNE_20);

    // Single-collection fetch.
    let (status, one) = send(&app, "GET", "/api/stac/collections/scenes", None).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(one["id"], "scenes");

    // Unknown collection: 404 with the typed STAC error body.
    let (status, err) = send(&app, "GET", "/api/stac/collections/nope", None).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(err["code"], "CollectionNotFound");
    assert!(err["description"].as_str().unwrap().contains("nope"));
    Ok(())
}

#[tokio::test]
async fn item_listing_and_retrieval_with_lineage() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = ctx(&tmp).await?;
    let (band_id, ndvi_a, _) = seed_graph(&app).await?;

    let (status, body) = send(&app, "GET", "/api/stac/collections/ndvi/items", None).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], "FeatureCollection");
    assert_eq!(body["numberReturned"], 2);

    // Single item: geometry/bbox/properties and the derived_from lineage link.
    let (status, item) = send(
        &app,
        "GET",
        &format!("/api/stac/collections/ndvi/items/{ndvi_a}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{item}");
    assert_eq!(item["type"], "Feature");
    assert_eq!(item["collection"], "ndvi");
    assert_eq!(item["geometry"]["type"], "Polygon");
    assert_eq!(item["properties"]["datetime"], T_JUNE_1);
    assert_eq!(item["properties"]["processing:level"], "L2");
    assert_eq!(item["properties"]["agbot:product_kind"], "ndvi");
    assert_eq!(item["properties"]["proj:code"], "EPSG:4326");
    let derived: Vec<&serde_json::Value> = item["links"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["rel"] == "derived_from")
        .collect();
    assert_eq!(derived.len(), 1);
    assert_eq!(
        derived[0]["href"],
        format!("/api/stac/collections/scenes/items/{band_id}")
    );
    assert!(item["assets"]["data"]["href"]
        .as_str()
        .unwrap()
        .contains("scene-a"));

    // Wrong-collection and unknown-item lookups are typed 404s.
    let (status, err) = send(
        &app,
        "GET",
        &format!("/api/stac/collections/scenes/items/{ndvi_a}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(err["code"], "ItemNotFound");
    Ok(())
}

#[tokio::test]
async fn search_filters_by_bbox_datetime_collections_and_ids() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = ctx(&tmp).await?;
    let (band_id, ndvi_a, ndvi_b) = seed_graph(&app).await?;

    // bbox around BBOX_A only.
    let (status, body) = send(
        &app,
        "GET",
        "/api/stac/search?collections=ndvi&bbox=-97.0,40.0,-96.0,42.0",
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["numberReturned"], 1, "{body}");
    assert_eq!(body["features"][0]["id"], ndvi_a);

    // datetime open-start interval keeps only the later product.
    let (status, body) = send(
        &app,
        "GET",
        "/api/stac/search?collections=ndvi&datetime=2026-06-10T00:00:00Z/..",
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["numberReturned"], 1);
    assert_eq!(body["features"][0]["id"], ndvi_b);

    // Cross-collection search without a collections filter sees all three.
    let (_, body) = send(&app, "GET", "/api/stac/search?limit=100", None).await?;
    assert_eq!(body["numberReturned"], 3);

    // POST body form: bbox + datetime + ids.
    let (status, body) = send(
        &app,
        "POST",
        "/api/stac/search",
        Some(json!({
            "collections": ["ndvi", "scenes"],
            "ids": [ndvi_a, band_id],
            "bbox": [-97.0, 40.0, -96.0, 42.0],
            "datetime": "2026-05-01T00:00:00Z/2026-06-10T00:00:00Z"
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["numberReturned"], 2, "{body}");

    // Malformed datetime is a typed 400.
    let (status, err) = send(&app, "GET", "/api/stac/search?datetime=not-a-date", None).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["code"], "InvalidDatetime");

    // Unknown collection in search is a typed 404.
    let (status, err) = send(&app, "GET", "/api/stac/search?collections=nope", None).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(err["code"], "CollectionNotFound");
    Ok(())
}

#[tokio::test]
async fn items_pagination_follows_next_links() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = ctx(&tmp).await?;
    for scene in ["s1", "s2", "s3"] {
        register(
            &app,
            draft("l2", "ndvi", scene, BBOX_A, T_JUNE_1, json!([])),
        )
        .await?;
    }

    let (status, page1) = send(
        &app,
        "GET",
        "/api/stac/collections/ndvi/items?limit=2",
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page1["numberReturned"], 2);
    let next = page1["links"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["rel"] == "next")
        .expect("next link on the first page");
    let next_href = next["href"].as_str().unwrap();
    assert!(next_href.contains("token=2"), "{next_href}");

    let (status, page2) = send(&app, "GET", next_href, None).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page2["numberReturned"], 1);
    assert!(
        !page2["links"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["rel"] == "next"),
        "no next link on the last page"
    );

    // No overlap between pages.
    let id_of = |page: &serde_json::Value, i: usize| page["features"][i]["id"].clone();
    assert_ne!(id_of(&page1, 0), id_of(&page2, 0));
    assert_ne!(id_of(&page1, 1), id_of(&page2, 0));
    Ok(())
}

#[tokio::test]
async fn stac_invalid_records_are_skipped_with_count() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = ctx(&tmp).await?;
    register(
        &app,
        draft("l2", "ndvi", "scene-ok", BBOX_A, T_JUNE_1, json!([])),
    )
    .await?;
    // A registered product with no spatial_ref: valid in the catalog, not
    // expressible as a STAC item.
    let no_spatial = json!({
        "level": "l2",
        "kind": "ndvi",
        "algorithm_id": "ndvi.compute",
        "algorithm_version": "1.0.0",
        "parameters": { "scene_id": "scene-nospatial" },
        "inputs": [],
        "scope": {
            "scene_id": "scene-nospatial",
            "temporal_start": T_JUNE_1,
            "temporal_end": T_JUNE_1
        }
    });
    let (status, _) = send(&app, "POST", "/api/catalog/products", Some(no_spatial)).await?;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(&app, "GET", "/api/stac/collections/ndvi/items", None).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["numberReturned"], 1, "{body}");
    assert_eq!(body["agbot:skipped"], 1);
    Ok(())
}
