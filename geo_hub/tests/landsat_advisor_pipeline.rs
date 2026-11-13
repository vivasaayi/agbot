//! Network-free acceptance coverage for Landsat C2 advisor automation.

use std::sync::Arc;

use anyhow::Result;
use chrono::{Duration, Utc};
use geo_hub::catalog;
use geo_hub::config::PipelineConfig;
use geo_hub::earth_search::EarthSearchItem;
use geo_hub::pipeline::{self, DerivePayload, JobKind, JobStatus};
use geo_hub::pipeline_worker::{
    run_one_tick, BoxFuture, ItemFetchError, JobRunResult, PipelineWorkerContext, StacItemFetcher,
};
use geo_hub::satellite_derivation::{CogStoreResolver, DerivationError};
use geo_hub::utm::{utm_to_wgs84, UtmZone};
use geo_hub::{db, HubConfig};
use raster_io::object_store::memory::InMemory;
use raster_io::object_store::path::Path as ObjectPath;
use raster_io::object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use raster_io::test_util::{build_tiled_geotiff, FixtureSpec, Pixels};
use serde_json::json;
use tempfile::TempDir;

const COLLECTION: &str = "landsat-c2-l2";
const ITEM_ID: &str = "LE07_L2SP_028031_20230721_20230817_02_T1";
const FIELD_ID: &str = "landsat-advisor-field";
const EPSG: u16 = 32614;
const TRANSFORM: [f64; 6] = [600_000.0, 30.0, 0.0, 4_700_960.0, 0.0, -30.0];

struct MemResolver(Arc<InMemory>);

impl CogStoreResolver for MemResolver {
    fn resolve(
        &self,
        href: &str,
    ) -> std::result::Result<(Arc<dyn ObjectStore>, String), DerivationError> {
        Ok((
            self.0.clone(),
            href.strip_prefix("https://cogs.test/")
                .unwrap_or(href)
                .to_string(),
        ))
    }
}

struct LandsatItemFetcher;

impl StacItemFetcher for LandsatItemFetcher {
    fn fetch_item<'a>(
        &'a self,
        collection: &'a str,
        item_id: &'a str,
    ) -> BoxFuture<'a, std::result::Result<EarthSearchItem, ItemFetchError>> {
        Box::pin(async move {
            if collection == COLLECTION && item_id == ITEM_ID {
                Ok(fixture_item())
            } else {
                Err(ItemFetchError::NotFound {
                    collection: collection.to_string(),
                    item_id: item_id.to_string(),
                })
            }
        })
    }
}

fn fixture_item() -> EarthSearchItem {
    let mut item: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/earth_search_landsat_item.json"))
            .expect("captured Landsat item parses");
    for key in ["red", "nir08", "qa_pixel"] {
        item["assets"][key]["href"] = json!(format!("https://cogs.test/{key}.tif"));
    }
    for key in ["qa_radsat", "cloud_qa"] {
        item["assets"][key] = json!({
            "href": format!("https://cogs.test/{key}.tif"),
            "type": "image/tiff; application=geotiff; profile=cloud-optimized"
        });
    }
    serde_json::from_value(item).expect("Landsat fixture decodes")
}

async fn fixture_store() -> Arc<InMemory> {
    let store = Arc::new(InMemory::new());
    let spec = |pixels, nodata| FixtureSpec {
        width: 32,
        height: 32,
        tile_width: 16,
        tile_height: 16,
        pixels,
        deflate: true,
        tile_gap: 0,
        epsg: EPSG,
        geo_transform: TRANSFORM,
        nodata,
    };
    for (name, pixels, nodata) in [
        (
            "red",
            Pixels::U16(vec![14_545; 32 * 32]),
            Some("0".to_string()),
        ),
        (
            "nir08",
            Pixels::U16(vec![29_091; 32 * 32]),
            Some("0".to_string()),
        ),
        ("qa_pixel", Pixels::U16(vec![64; 32 * 32]), None),
        ("qa_radsat", Pixels::U16(vec![0; 32 * 32]), None),
        ("cloud_qa", Pixels::U16(vec![0; 32 * 32]), None),
    ] {
        store
            .put(
                &ObjectPath::from(format!("{name}.tif")),
                PutPayload::from(build_tiled_geotiff(&spec(pixels, nodata))),
            )
            .await
            .expect("fixture COG writes");
    }
    store
}

fn aoi_geojson() -> serde_json::Value {
    let zone = UtmZone {
        zone: 14,
        north: true,
    };
    let corners = [
        utm_to_wgs84(600_120.0, 4_700_120.0, zone),
        utm_to_wgs84(600_840.0, 4_700_120.0, zone),
        utm_to_wgs84(600_120.0, 4_700_840.0, zone),
        utm_to_wgs84(600_840.0, 4_700_840.0, zone),
    ];
    let min_lon = corners.iter().map(|(_, lon)| *lon).fold(f64::MAX, f64::min);
    let max_lon = corners.iter().map(|(_, lon)| *lon).fold(f64::MIN, f64::max);
    let min_lat = corners.iter().map(|(lat, _)| *lat).fold(f64::MAX, f64::min);
    let max_lat = corners.iter().map(|(lat, _)| *lat).fold(f64::MIN, f64::max);
    json!({
        "type": "Polygon",
        "coordinates": [[[min_lon, min_lat], [max_lon, min_lat], [max_lon, max_lat],
                         [min_lon, max_lat], [min_lon, min_lat]]]
    })
}

async fn worker_ctx(qa_min_confidence: f64) -> Result<(TempDir, PipelineWorkerContext)> {
    let tmp = TempDir::new()?;
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("landsat_advisor.db").display()
        ),
        data_root: tmp.path().join("data"),
        pipeline: PipelineConfig {
            enabled: true,
            provider_min_delay_ms: 0,
            qa_min_confidence,
            ..PipelineConfig::default()
        },
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let ctx = PipelineWorkerContext {
        pool,
        config: Arc::new(config),
        cog_resolver: Arc::new(MemResolver(fixture_store().await)),
        item_fetcher: Arc::new(LandsatItemFetcher),
    };
    Ok((tmp, ctx))
}

async fn enqueue_derive(ctx: &PipelineWorkerContext) -> Result<()> {
    let now = Utc::now();
    let payload = serde_json::to_value(DerivePayload {
        dataset: COLLECTION.to_string(),
        collection: COLLECTION.to_string(),
        item_id: ITEM_ID.to_string(),
        index: "ndvi".to_string(),
        field_id: FIELD_ID.to_string(),
        season_id: Some("2023".to_string()),
        aoi_geojson: aoi_geojson(),
    })?;
    pipeline::enqueue_job(
        &ctx.pool,
        JobKind::Derive,
        &pipeline::derive_job_key(COLLECTION, ITEM_ID, "ndvi", FIELD_ID),
        &payload,
        Some(FIELD_ID),
        Some(COLLECTION),
        0,
        now - Duration::minutes(1),
        None,
        now,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn qa_accepted_landsat_ndvi_runs_advisor_apps_with_finding_lineage() -> Result<()> {
    let (_tmp, ctx) = worker_ctx(0.25).await?;
    let aoi = aoi_geojson();
    let coordinates: Vec<_> = aoi["coordinates"][0]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| json!({"longitude":p[0],"latitude":p[1]}))
        .collect();
    let boundary_json = json!({"crs":"EPSG:4326","coordinates":coordinates});
    let boundary: shared::schemas::FieldBoundary = serde_json::from_value(boundary_json.clone())?;
    let expected_area = shared::schemas::validate_field_boundary(&boundary)?.area_ha * 10000.0;
    sqlx::query("INSERT INTO fields (field_id,owner,name,boundary_json,created_at) VALUES (?,'org','Field',?,datetime('now'))")
        .bind(FIELD_ID).bind(boundary_json.to_string()).execute(&ctx.pool).await?;
    enqueue_derive(&ctx).await?;

    let derive = run_one_tick(&ctx).await?;
    assert_eq!(derive.kind, Some(JobKind::Derive));
    assert_eq!(derive.result, Some(JobRunResult::Succeeded), "{derive:?}");

    let products = catalog::list_products(
        &ctx.pool,
        &catalog::ProductFilter {
            field_id: Some(FIELD_ID.to_string()),
            level: Some(shared::product_graph::ProductLevel::L2),
            kind: Some("ndvi".to_string()),
            ..catalog::ProductFilter::default()
        },
    )
    .await?;
    assert_eq!(products.len(), 1);
    let ndvi = &products[0];
    assert_eq!(
        ndvi.status, "registered",
        "confidence={:?} quality={:?}",
        ndvi.confidence, ndvi.quality_summary
    );
    assert!(ndvi.confidence.is_some_and(|confidence| confidence >= 0.99));

    for app_id in ["crop_health", "anomaly_detection"] {
        let key = pipeline::app_job_key(app_id, FIELD_ID, "2023-07-21");
        let job = pipeline::find_job_by_key(&ctx.pool, &key)
            .await?
            .unwrap_or_else(|| panic!("{app_id} was automatically enqueued"));
        assert_eq!(job.status, JobStatus::Queued);
    }

    // Drain follow-up work until both advisor applications have run. The L3
    // rollup may enqueue its own deterministic follow-ups between them.
    for _ in 0..12 {
        let outcome = run_one_tick(&ctx).await?;
        assert_eq!(outcome.result, Some(JobRunResult::Succeeded), "{outcome:?}");
        let findings = geo_hub::applications::list_field_findings(&ctx.pool, FIELD_ID).await?;
        if findings
            .iter()
            .any(|finding| finding.app_id == "crop_health")
            && findings
                .iter()
                .any(|finding| finding.app_id == "anomaly_detection")
        {
            break;
        }
    }

    let findings = geo_hub::applications::list_field_findings(&ctx.pool, FIELD_ID).await?;
    assert!(findings
        .iter()
        .any(|finding| finding.app_id == "crop_health"));
    assert!(findings
        .iter()
        .any(|finding| finding.app_id == "anomaly_detection"));
    for finding in &findings {
        let area = finding
            .finding
            .metrics
            .as_ref()
            .and_then(|m| m["area_m2"].as_f64())
            .expect("finding area");
        assert!(area > 10000.0, "area must be square metres, got {area}");
        assert!(
            (area - expected_area).abs() < 1.0,
            "field area mismatch: {area} vs {expected_area}"
        );

        assert!(finding.finding.evidence_refs.contains(&ndvi.product_id));
        let lineage =
            geo_hub::provenance_store::trace_backward(&ctx.pool, &finding.finding_id).await?;
        assert!(lineage
            .records
            .iter()
            .any(|record| record.artifact_id == ndvi.product_id));
    }
    Ok(())
}

#[tokio::test]
async fn qa_quarantined_landsat_ndvi_does_not_enqueue_advisor_apps() -> Result<()> {
    let (_tmp, ctx) = worker_ctx(1.01).await?;
    enqueue_derive(&ctx).await?;
    let derive = run_one_tick(&ctx).await?;
    assert_eq!(derive.result, Some(JobRunResult::Succeeded), "{derive:?}");

    let products = catalog::list_products(
        &ctx.pool,
        &catalog::ProductFilter {
            field_id: Some(FIELD_ID.to_string()),
            level: Some(shared::product_graph::ProductLevel::L2),
            status: Some(catalog::STATUS_FAILED_QA.to_string()),
            ..catalog::ProductFilter::default()
        },
    )
    .await?;
    assert_eq!(products.len(), 1);
    for app_id in ["crop_health", "anomaly_detection"] {
        assert!(pipeline::find_job_by_key(
            &ctx.pool,
            &pipeline::app_job_key(app_id, FIELD_ID, "2023-07-21")
        )
        .await?
        .is_none());
    }
    assert!(
        geo_hub::applications::list_field_findings(&ctx.pool, FIELD_ID)
            .await?
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn acceptance_landsat_is_visible_in_authenticated_farmer_portal() -> Result<()> {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let (_tmp, ctx) = worker_ctx(0.25).await?;
    let token = "farmer-display-test";
    sqlx::query("INSERT INTO marketplace_accounts (account_id,org_id,party_type,role_refs_json,status,created_at,updated_at) VALUES ('farmer','org','farmer','[]','active',datetime('now'),datetime('now'))").execute(&ctx.pool).await?;
    sqlx::query("INSERT INTO portal_sessions (session_id,token_hash,account_id,org_id,created_at,expires_at,last_seen_at) VALUES ('session',?,'farmer','org',datetime('now'),?,datetime('now'))")
        .bind(geo_hub::portal_auth::hash_token(token)).bind((Utc::now()+Duration::days(1)).to_rfc3339()).execute(&ctx.pool).await?;
    let aoi = aoi_geojson();
    let coordinates: Vec<_> = aoi["coordinates"][0]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| json!({"longitude":p[0],"latitude":p[1]}))
        .collect();
    sqlx::query("INSERT INTO fields (field_id,owner,name,crop,season,boundary_json,status,created_at) VALUES (?,'org','My field','corn','2023',?,'active',datetime('now'))")
        .bind(FIELD_ID).bind(json!({"crs":"EPSG:4326","coordinates":coordinates}).to_string()).execute(&ctx.pool).await?;
    enqueue_derive(&ctx).await?;
    assert_eq!(
        run_one_tick(&ctx).await?.result,
        Some(JobRunResult::Succeeded)
    );
    let keys: Vec<_> = ["crop_health", "anomaly_detection"]
        .iter()
        .map(|app| pipeline::app_job_key(app, FIELD_ID, "2023-07-21"))
        .collect();
    let mut completed = false;
    for _ in 0..32 {
        let mut done = true;
        for key in &keys {
            let job = pipeline::find_job_by_key(&ctx.pool, key)
                .await?
                .expect("advisor job enqueued");
            done &= job.status == JobStatus::Succeeded;
        }
        if done {
            completed = true;
            break;
        }
        let tick = run_one_tick(&ctx).await?;
        assert_eq!(tick.result, Some(JobRunResult::Succeeded), "{tick:?}");
    }
    assert!(completed, "advisor jobs did not finish");
    assert!(
        !geo_hub::applications::list_field_findings(&ctx.pool, FIELD_ID)
            .await?
            .is_empty()
    );
    let mut config = (*ctx.config).clone();
    config.security.require_session = true;
    let app = geo_hub::server::build_router(geo_hub::state::AppState {
        pool: ctx.pool.clone(),
        config: Arc::new(config),
        scene_search_cache: Default::default(),
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/portal/fields/{FIELD_ID}/overview"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let overview: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await?)?;
    assert_eq!(overview["latest_scene"]["scene_id"], fixture_item().id);
    let product_id = overview["latest_layer"]["product_id"]
        .as_str()
        .expect("accepted catalog layer");
    assert_eq!(overview["latest_layer"]["kind"], "ndvi");
    assert!(overview["latest_layer"]["bbox"].is_array());
    assert!(overview["monitoring"]["status"].is_string());
    let (lat, lon) = utm_to_wgs84(
        600_480.0,
        4_700_480.0,
        UtmZone {
            zone: 14,
            north: true,
        },
    );
    let (x, y) = geo_hub::product_tiler::tile_containing(lat, lon, 15);
    let tile = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/catalog/products/{product_id}/tiles/15/{x}/{y}.png"
                ))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(tile.status(), StatusCode::OK);
    let bytes = to_bytes(tile.into_body(), 1024 * 1024).await?;
    assert!(image::load_from_memory(&bytes)?
        .to_rgba8()
        .pixels()
        .any(|p| p[3] > 0));
    let history = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/fields/{FIELD_ID}/timeseries?metric=sat.ndvi.mean"
                ))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(history.status(), StatusCode::OK);
    let series: serde_json::Value =
        serde_json::from_slice(&to_bytes(history.into_body(), 1024 * 1024).await?)?;
    assert_eq!(
        series["per_source"]["landsat"].as_array().map(Vec::len),
        Some(1)
    );
    let generated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/portal/fields/{FIELD_ID}/grower-report"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    let status = generated.status();
    let body = to_bytes(generated.into_body(), 1024 * 1024).await?;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let report: serde_json::Value = serde_json::from_slice(&body)?;
    let downloaded = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/portal/reports/{}/download",
                    report["report_id"].as_str().unwrap()
                ))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(downloaded.status(), StatusCode::OK);
    assert!(to_bytes(downloaded.into_body(), 8 * 1024 * 1024)
        .await?
        .starts_with(b"%PDF"));
    if std::env::var("AGBOT_BROWSER_ACCEPTANCE").as_deref() == Ok("1") {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let base = format!("http://{}", listener.local_addr()?);
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let result = tokio::process::Command::new("node")
            .arg(root.join("scripts/test-farmer-products.cjs"))
            .env("AGBOT_TEST_URL", &base)
            .env("AGBOT_TEST_TOKEN", token)
            .env("AGBOT_TEST_FIELD_ID", FIELD_ID)
            .env("AGBOT_TEST_DISPOSABLE", "1")
            .status()
            .await;
        server.abort();
        assert!(
            result?.success(),
            "farmer product browser acceptance failed"
        );
    }
    Ok(())
}
