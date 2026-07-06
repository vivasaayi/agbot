//! Integration tests for the serial pipeline worker loop (batch S-7):
//! orphan recovery on startup, derive-job execution against fixture COGs
//! (network-free, reusing the `satellite_derive` fixture pattern),
//! client-error dead-lettering, subscription cadence -> discover enqueue,
//! unimplemented-kind dead-lettering, and shutdown-signal handling.

use std::sync::Arc;
use std::time::Duration as StdDuration;

use anyhow::Result;
use chrono::{Duration, Utc};
use geo_hub::catalog::{self, ProductFilter};
use geo_hub::config::PipelineConfig;
use geo_hub::earth_search::EarthSearchItem;
use geo_hub::pipeline::{self, DerivePayload, JobKind, JobStatus, SubscriptionUpsert};
use geo_hub::pipeline_worker::{
    run_one_tick, spawn_pipeline_worker, BoxFuture, ItemFetchError, JobRunResult,
    PipelineWorkerContext, StacItemFetcher,
};
use geo_hub::satellite_derivation::{CogStoreResolver, DerivationError};
use geo_hub::utm::{utm_to_wgs84, UtmZone};
use geo_hub::{db, HubConfig};
use raster_io::object_store::memory::InMemory;
use raster_io::object_store::path::Path as ObjectPath;
use raster_io::object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use raster_io::test_util::{build_tiled_geotiff, FixtureSpec, Pixels};
use serde_json::json;
use shared::product_graph::ProductLevel;
use tempfile::TempDir;
use tokio::sync::watch;

const FIXTURE_COLLECTION: &str = "sentinel-2-l2a";
const FIXTURE_ITEM_ID: &str = "S2B_43PFN_20230128_0_L2A";
const EPSG: u16 = 32643;
const TRANSFORM_10M: [f64; 6] = [600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];
const TRANSFORM_20M: [f64; 6] = [600_000.0, 20.0, 0.0, 1_300_020.0, 0.0, -20.0];

// --- Fixtures (same pattern as tests/satellite_derive.rs) -------------------

/// Resolver mapping https://cogs.test/<path> to the in-memory store.
struct MemResolver(Arc<InMemory>);

impl CogStoreResolver for MemResolver {
    fn resolve(
        &self,
        href: &str,
    ) -> std::result::Result<(Arc<dyn ObjectStore>, String), DerivationError> {
        let path = href
            .strip_prefix("https://cogs.test/")
            .unwrap_or(href)
            .to_string();
        Ok((self.0.clone(), path))
    }
}

/// Fixture bands: red DN 2000 everywhere, nir DN 6000, SCL class 4
/// (vegetation) — a uniform clear-sky scene the derive path can process.
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
    let nir = band(Pixels::U16(vec![6000; 32 * 32]), TRANSFORM_10M, 32);
    let scl = band(Pixels::U8(vec![4u8; 16 * 16]), TRANSFORM_20M, 16);
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
/// in-memory store, decoded to the typed item the worker's fetcher returns.
fn fixture_item() -> EarthSearchItem {
    let mut item: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/earth_search_s2_item.json"))
            .expect("parse captured item");
    for (asset, name) in [("red", "red"), ("nir", "nir"), ("scl", "scl")] {
        item["assets"][asset]["href"] = json!(format!("https://cogs.test/fixtures/{name}.tif"));
    }
    serde_json::from_value(item).expect("captured item decodes as EarthSearchItem")
}

/// Test item fetcher: serves the fixture item, NotFound for anything else.
struct FixtureItemFetcher;

impl StacItemFetcher for FixtureItemFetcher {
    fn fetch_item<'a>(
        &'a self,
        collection: &'a str,
        item_id: &'a str,
    ) -> BoxFuture<'a, std::result::Result<EarthSearchItem, ItemFetchError>> {
        Box::pin(async move {
            if collection == FIXTURE_COLLECTION && item_id == FIXTURE_ITEM_ID {
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

/// AOI polygon whose envelope matches the `satellite_derive` test AOI: a
/// projected rect inset 5 m inside (600100, 1299820)-(600200, 1299920) in
/// EPSG:32643, inverse-projected to WGS84.
fn aoi_polygon_geojson() -> serde_json::Value {
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
    json!({
        "type": "Polygon",
        "coordinates": [[
            [min_lon, min_lat],
            [max_lon, min_lat],
            [max_lon, max_lat],
            [min_lon, max_lat],
            [min_lon, min_lat],
        ]],
    })
}

async fn worker_ctx(poll_interval_ms: u64) -> Result<(TempDir, PipelineWorkerContext)> {
    let tmp = TempDir::new()?;
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("worker.db").display()
        ),
        data_root: tmp.path().join("data"),
        pipeline: PipelineConfig {
            enabled: true,
            poll_interval_ms,
            provider_min_delay_ms: 0,
        },
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let ctx = PipelineWorkerContext {
        pool,
        config: Arc::new(config),
        cog_resolver: Arc::new(MemResolver(fixture_store().await)),
        item_fetcher: Arc::new(FixtureItemFetcher),
    };
    Ok((tmp, ctx))
}

fn derive_payload(item_id: &str) -> serde_json::Value {
    serde_json::to_value(DerivePayload {
        dataset: FIXTURE_COLLECTION.to_string(),
        collection: FIXTURE_COLLECTION.to_string(),
        item_id: item_id.to_string(),
        index: "ndvi".to_string(),
        field_id: "field-42".to_string(),
        season_id: Some("season-2026-kharif".to_string()),
        aoi_geojson: aoi_polygon_geojson(),
    })
    .expect("derive payload serializes")
}

async fn enqueue_derive(ctx: &PipelineWorkerContext, item_id: &str) -> Result<()> {
    let now = Utc::now();
    pipeline::enqueue_job(
        &ctx.pool,
        JobKind::Derive,
        &pipeline::derive_job_key(FIXTURE_COLLECTION, item_id, "ndvi", "field-42"),
        &derive_payload(item_id),
        Some("field-42"),
        Some(FIXTURE_COLLECTION),
        0,
        now - Duration::minutes(1),
        None,
        now,
    )
    .await?;
    Ok(())
}

// --- Tests -------------------------------------------------------------------

#[tokio::test]
async fn worker_resets_orphaned_running_jobs_on_start() -> Result<()> {
    let (_tmp, ctx) = worker_ctx(10).await?;

    // Seed a job orphaned mid-run by a "crashed" worker. Its run_after is far
    // in the future so the live loop cannot re-claim it after the reset —
    // the assertion observes the recovered `queued` state, not a re-run.
    let future = Utc::now() + Duration::days(30);
    pipeline::enqueue_job(
        &ctx.pool,
        JobKind::Discover,
        &pipeline::discover_job_key("field-1", FIXTURE_COLLECTION),
        &json!({ "field_id": "field-1", "dataset": FIXTURE_COLLECTION }),
        Some("field-1"),
        Some(FIXTURE_COLLECTION),
        0,
        future,
        None,
        Utc::now(),
    )
    .await?;
    let orphan = pipeline::claim_next_job(&ctx.pool, future + Duration::hours(1))
        .await?
        .expect("claim seeded job");
    assert_eq!(orphan.status, JobStatus::Running);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handle = spawn_pipeline_worker(ctx.clone(), shutdown_rx);

    // The startup pass must move the orphan back to queued.
    let deadline = tokio::time::Instant::now() + StdDuration::from_secs(5);
    loop {
        let jobs = pipeline::list_jobs(&ctx.pool, None).await?;
        if jobs[0].status == JobStatus::Queued {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "orphaned job never reset: {:?}",
            jobs[0].status
        );
        tokio::time::sleep(StdDuration::from_millis(10)).await;
    }

    shutdown_tx.send(true)?;
    tokio::time::timeout(StdDuration::from_secs(5), handle).await??;

    let jobs = pipeline::list_jobs(&ctx.pool, None).await?;
    assert_eq!(jobs[0].status, JobStatus::Queued);
    assert_eq!(jobs[0].claimed_at, None);
    Ok(())
}

#[tokio::test]
async fn worker_drains_derive_job_and_registers_product() -> Result<()> {
    let (_tmp, ctx) = worker_ctx(1000).await?;
    enqueue_derive(&ctx, FIXTURE_ITEM_ID).await?;

    let outcome = run_one_tick(&ctx).await?;

    assert!(outcome.claimed);
    assert_eq!(outcome.kind, Some(JobKind::Derive));
    assert_eq!(outcome.result, Some(JobRunResult::Succeeded));
    assert!(outcome.hit_provider, "derive jobs touch the COG provider");

    let jobs = pipeline::list_jobs(&ctx.pool, None).await?;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].status, JobStatus::Succeeded);
    assert_eq!(jobs[0].last_error, None);

    // The derive registered a field-scoped L2 ndvi product.
    let products = catalog::list_products(
        &ctx.pool,
        &ProductFilter {
            field_id: Some("field-42".to_string()),
            level: Some(ProductLevel::L2),
            kind: Some("ndvi".to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;
    assert_eq!(products.len(), 1, "{products:?}");
    assert_eq!(products[0].season_id.as_deref(), Some("season-2026-kharif"));
    assert_eq!(products[0].scene_id.as_deref(), Some(FIXTURE_ITEM_ID));

    // Field scope means the S-3 stats hook appended time-series points.
    let (points,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM time_series_points WHERE entity_ref = ?")
            .bind("field:field-42")
            .fetch_one(&ctx.pool)
            .await?;
    assert!(points > 0, "expected time_series_points rows for the field");
    Ok(())
}

#[tokio::test]
async fn worker_marks_client_error_jobs_dead() -> Result<()> {
    let (_tmp, ctx) = worker_ctx(1000).await?;
    enqueue_derive(&ctx, "S2X_NO_SUCH_ITEM").await?;

    let outcome = run_one_tick(&ctx).await?;

    assert!(outcome.claimed);
    match outcome.result {
        Some(JobRunResult::Failed {
            ref error,
            client_error,
        }) => {
            assert!(client_error, "missing item is a client error: {error}");
            assert!(error.contains("S2X_NO_SUCH_ITEM"), "{error}");
        }
        other => panic!("expected client failure, got {other:?}"),
    }

    let jobs = pipeline::list_jobs(&ctx.pool, None).await?;
    assert_eq!(jobs[0].status, JobStatus::Dead);
    let last_error = jobs[0].last_error.as_deref().expect("last_error recorded");
    assert!(last_error.contains("S2X_NO_SUCH_ITEM"), "{last_error}");
    Ok(())
}

#[tokio::test]
async fn worker_enqueues_discover_for_due_subscription() -> Result<()> {
    let (_tmp, ctx) = worker_ctx(1000).await?;

    let subscription = pipeline::upsert_subscription(
        &ctx.pool,
        &SubscriptionUpsert {
            field_id: "field-7".to_string(),
            dataset: FIXTURE_COLLECTION.to_string(),
            indices: vec!["ndvi".to_string()],
            cadence_hours: 24,
            max_cloud_cover: 60.0,
            lookback_days: 14,
        },
        Utc::now() - Duration::days(3),
    )
    .await?;
    // Stale check: last run two days ago against a 24h cadence.
    let stale = Utc::now() - Duration::days(2);
    pipeline::touch_subscription_checked(&ctx.pool, &subscription.subscription_id, stale).await?;

    let outcome = run_one_tick(&ctx).await?;

    assert!(!outcome.claimed, "no job was ready before the cadence pass");
    assert_eq!(outcome.discover_enqueued, 1);

    let jobs = pipeline::list_jobs(&ctx.pool, None).await?;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].kind, JobKind::Discover);
    assert_eq!(jobs[0].status, JobStatus::Queued);
    assert_eq!(jobs[0].field_id.as_deref(), Some("field-7"));

    // The subscription was touched: it is no longer due, and a second tick
    // deduplicates on the discover job_key instead of enqueueing again.
    let subscriptions = pipeline::list_subscriptions(&ctx.pool, Some("field-7")).await?;
    let checked = subscriptions[0]
        .last_checked_at
        .as_deref()
        .expect("touched");
    assert!(checked > pipeline::format_ts(stale).as_str());
    assert!(pipeline::due_subscriptions(&ctx.pool, Utc::now())
        .await?
        .is_empty());
    Ok(())
}

#[tokio::test]
async fn unimplemented_kinds_go_dead() -> Result<()> {
    let (_tmp, ctx) = worker_ctx(1000).await?;

    let now = Utc::now();
    pipeline::enqueue_job(
        &ctx.pool,
        JobKind::AppRun,
        &pipeline::app_job_key("drought_watch", "field-42", "2026-07-06"),
        &json!({ "app_id": "drought_watch", "field_id": "field-42", "date": "2026-07-06" }),
        Some("field-42"),
        None,
        0,
        now - Duration::minutes(1),
        None,
        now,
    )
    .await?;

    let outcome = run_one_tick(&ctx).await?;

    assert!(outcome.claimed);
    assert_eq!(outcome.kind, Some(JobKind::AppRun));
    match outcome.result {
        Some(JobRunResult::Failed {
            ref error,
            client_error,
        }) => {
            assert!(client_error, "unimplemented handler must not retry-loop");
            assert!(error.contains("handler not implemented"), "{error}");
        }
        other => panic!("expected handler-not-implemented failure, got {other:?}"),
    }

    let jobs = pipeline::list_jobs(&ctx.pool, None).await?;
    assert_eq!(jobs[0].status, JobStatus::Dead);
    assert!(jobs[0]
        .last_error
        .as_deref()
        .expect("last_error recorded")
        .contains("handler not implemented"));
    Ok(())
}

#[tokio::test]
async fn worker_shuts_down_on_signal() -> Result<()> {
    let (_tmp, ctx) = worker_ctx(1000).await?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handle = spawn_pipeline_worker(ctx, shutdown_rx);

    shutdown_tx.send(true)?;
    tokio::time::timeout(StdDuration::from_secs(5), handle).await??;
    Ok(())
}
