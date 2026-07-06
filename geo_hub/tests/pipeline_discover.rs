//! Batch S-8 integration tests: discover-stage execution (STAC range search
//! fan-out into derive jobs, already-derived skip, subscription touch),
//! subscription CRUD routes, the manual pipeline trigger, and job
//! listing/retry routes.
//!
//! The STAC search seam is exercised through a fixture
//! [`StacItemFetcher::search`] implementation, so no test touches the
//! network.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use chrono::{DateTime, Duration, Utc};
use geo_hub::earth_search::EarthSearchItem;
use geo_hub::pipeline::{self, DerivePayload, DiscoverPayload, JobKind, JobStatus};
use geo_hub::pipeline_worker::{
    aoi_bounds_from_geojson, run_one_tick, BoxFuture, ItemFetchError, JobRunResult,
    PipelineWorkerContext, StacItemFetcher, StacSearchQuery,
};
use geo_hub::satellite_derivation::{CogStoreResolver, DerivationError};
use geo_hub::state::AppState;
use geo_hub::{config::PipelineConfig, db, server, HubConfig};
use raster_io::object_store::memory::InMemory;
use raster_io::object_store::ObjectStore;
use serde_json::json;
use tempfile::TempDir;
use tower::util::ServiceExt;

const FIELD: &str = "field-42";
const DATASET: &str = "sentinel2";
const ITEM_1: &str = "S2B_43PFN_20230128_0_L2A";
const ITEM_2: &str = "S2B_43PFN_20230207_0_L2A";

// --- Fixtures ----------------------------------------------------------------

/// Discover never reads COGs, so the resolver just points at an empty store.
struct EmptyStoreResolver(Arc<InMemory>);

impl CogStoreResolver for EmptyStoreResolver {
    fn resolve(
        &self,
        href: &str,
    ) -> std::result::Result<(Arc<dyn ObjectStore>, String), DerivationError> {
        Ok((self.0.clone(), href.to_string()))
    }
}

/// The captured Earth Search item, cloned into a two-scene search result.
fn fixture_items() -> Vec<EarthSearchItem> {
    let first: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/earth_search_s2_item.json"))
            .expect("parse captured item");
    let mut second = first.clone();
    second["id"] = json!(ITEM_2);
    second["properties"]["datetime"] = json!("2023-02-07T05:25:49.364000Z");
    vec![
        serde_json::from_value(first).expect("first item decodes"),
        serde_json::from_value(second).expect("second item decodes"),
    ]
}

/// Fixture search seam: records every query, returns the canned items.
struct FixtureSearchFetcher {
    items: Vec<EarthSearchItem>,
    queries: Arc<Mutex<Vec<StacSearchQuery>>>,
}

impl FixtureSearchFetcher {
    fn new(items: Vec<EarthSearchItem>) -> Self {
        Self {
            items,
            queries: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl StacItemFetcher for FixtureSearchFetcher {
    fn fetch_item<'a>(
        &'a self,
        collection: &'a str,
        item_id: &'a str,
    ) -> BoxFuture<'a, std::result::Result<EarthSearchItem, ItemFetchError>> {
        Box::pin(async move {
            Err(ItemFetchError::NotFound {
                collection: collection.to_string(),
                item_id: item_id.to_string(),
            })
        })
    }

    fn search<'a>(
        &'a self,
        query: &'a StacSearchQuery,
    ) -> BoxFuture<'a, std::result::Result<Vec<EarthSearchItem>, ItemFetchError>> {
        Box::pin(async move {
            self.queries.lock().unwrap().push(query.clone());
            Ok(self.items.clone())
        })
    }
}

fn boundary_geojson() -> serde_json::Value {
    json!({
        "type": "Polygon",
        "coordinates": [[
            [76.64, 11.34],
            [76.65, 11.34],
            [76.65, 11.35],
            [76.64, 11.35],
            [76.64, 11.34],
        ]],
    })
}

async fn hub_pool(tmp: &TempDir) -> Result<(Arc<HubConfig>, db::DbPool)> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("discover.db").display()
        ),
        data_root: tmp.path().join("data"),
        pipeline: PipelineConfig {
            enabled: true,
            poll_interval_ms: 1000,
            provider_min_delay_ms: 0,
        },
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    Ok((Arc::new(config), pool))
}

fn worker_ctx(
    config: Arc<HubConfig>,
    pool: db::DbPool,
    fetcher: Arc<dyn StacItemFetcher>,
) -> PipelineWorkerContext {
    PipelineWorkerContext {
        pool,
        config,
        cog_resolver: Arc::new(EmptyStoreResolver(Arc::new(InMemory::new()))),
        item_fetcher: fetcher,
    }
}

fn router(config: Arc<HubConfig>, pool: db::DbPool) -> Router {
    server::build_router(AppState {
        pool,
        config,
        scene_search_cache: Default::default(),
    })
}

async fn seed_field(pool: &db::DbPool, field_id: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO fields (field_id, name, boundary_json, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(field_id)
    .bind(format!("Field {field_id}"))
    .bind(boundary_geojson().to_string())
    .bind("2026-07-01T00:00:00Z")
    .execute(pool)
    .await?;
    Ok(())
}

async fn seed_subscription(pool: &db::DbPool, field_id: &str) -> Result<String> {
    let record = pipeline::upsert_subscription(
        pool,
        &pipeline::SubscriptionUpsert {
            field_id: field_id.to_string(),
            dataset: DATASET.to_string(),
            indices: vec!["ndvi".to_string(), "ndmi".to_string()],
            cadence_hours: 24,
            max_cloud_cover: 60.0,
            lookback_days: 14,
        },
        Utc::now(),
    )
    .await?;
    Ok(record.subscription_id)
}

async fn enqueue_discover(pool: &db::DbPool, field_id: &str) -> Result<()> {
    let now = Utc::now();
    pipeline::enqueue_job(
        pool,
        JobKind::Discover,
        &pipeline::discover_job_key(field_id, DATASET),
        &serde_json::to_value(DiscoverPayload {
            field_id: field_id.to_string(),
            dataset: DATASET.to_string(),
        })?,
        Some(field_id),
        Some(DATASET),
        0,
        now - Duration::minutes(1),
        None,
        now,
    )
    .await?;
    Ok(())
}

async fn enqueue_derive_marker(
    pool: &db::DbPool,
    item_id: &str,
    index: &str,
    run_after: DateTime<Utc>,
) -> Result<String> {
    let job_key = pipeline::derive_job_key(DATASET, item_id, index, FIELD);
    pipeline::enqueue_job(
        pool,
        JobKind::Derive,
        &job_key,
        &json!({ "marker": "pre-existing" }),
        Some(FIELD),
        Some(DATASET),
        0,
        run_after,
        None,
        Utc::now(),
    )
    .await?;
    Ok(job_key)
}

// --- HTTP helpers --------------------------------------------------------------

async fn send_json(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> Result<(StatusCode, serde_json::Value)> {
    let builder = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(value) => builder
            .header("content-type", "application/json")
            .body(Body::from(value.to_string()))?,
        None => builder.body(Body::empty())?,
    };
    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024).await?;
    let value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes).to_string()))
    };
    Ok((status, value))
}

// --- Discover execution ----------------------------------------------------------

#[tokio::test]
async fn discover_enqueues_derive_jobs_from_stac_fixture() -> Result<()> {
    let tmp = TempDir::new()?;
    let (config, pool) = hub_pool(&tmp).await?;
    seed_field(&pool, FIELD).await?;
    seed_subscription(&pool, FIELD).await?;
    enqueue_discover(&pool, FIELD).await?;

    let fetcher = Arc::new(FixtureSearchFetcher::new(fixture_items()));
    let queries = Arc::clone(&fetcher.queries);
    let ctx = worker_ctx(config, pool.clone(), fetcher);

    let before = Utc::now();
    let outcome = run_one_tick(&ctx).await?;

    assert!(outcome.claimed);
    assert_eq!(outcome.kind, Some(JobKind::Discover));
    assert_eq!(outcome.result, Some(JobRunResult::Succeeded));
    assert!(outcome.hit_provider, "discover talks to the STAC provider");
    // The execution touch marks the subscription checked, so the same tick's
    // cadence pass must not re-enqueue the just-finished discover job.
    assert_eq!(outcome.discover_enqueued, 0);

    // The search window came from the subscription: bbox = boundary envelope,
    // cloud filter from the subscription, [now - lookback_days, now] range
    // because the subscription had never been checked.
    let recorded = queries.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1);
    let query = &recorded[0];
    assert_eq!(query.dataset, DATASET);
    assert_eq!(query.max_cloud_cover, 60.0);
    let expected_bbox = aoi_bounds_from_geojson(&boundary_geojson()).unwrap();
    assert!((query.bbox[0] - expected_bbox.min_lon).abs() < 1e-9);
    assert!((query.bbox[1] - expected_bbox.min_lat).abs() < 1e-9);
    assert!((query.bbox[2] - expected_bbox.max_lon).abs() < 1e-9);
    assert!((query.bbox[3] - expected_bbox.max_lat).abs() < 1e-9);
    let start: DateTime<Utc> = query.start_iso.parse()?;
    let end: DateTime<Utc> = query.end_iso.parse()?;
    assert!(end >= before - Duration::minutes(1) && end <= Utc::now());
    let lookback = end - start;
    assert_eq!(lookback.num_days(), 14, "start = end - lookback_days");

    // 2 items x 2 indices -> 4 derive jobs, plus the succeeded discover job.
    let jobs = pipeline::list_jobs(&pool, Some(FIELD)).await?;
    assert_eq!(jobs.len(), 5, "{jobs:?}");
    let discover = jobs
        .iter()
        .find(|job| job.kind == JobKind::Discover)
        .expect("discover job present");
    assert_eq!(discover.status, JobStatus::Succeeded);

    for item_id in [ITEM_1, ITEM_2] {
        for index in ["ndvi", "ndmi"] {
            let job_key = pipeline::derive_job_key(DATASET, item_id, index, FIELD);
            let job = jobs
                .iter()
                .find(|job| job.job_key == job_key)
                .unwrap_or_else(|| panic!("missing derive job {job_key}"));
            assert_eq!(job.kind, JobKind::Derive);
            assert_eq!(job.status, JobStatus::Queued);
            assert_eq!(job.priority, 0);
            assert_eq!(job.field_id.as_deref(), Some(FIELD));
            assert_eq!(job.dataset.as_deref(), Some(DATASET));
            let payload: DerivePayload = serde_json::from_str(&job.payload_json)?;
            assert_eq!(payload.dataset, DATASET);
            assert_eq!(payload.collection, "sentinel-2-l2a");
            assert_eq!(payload.item_id, item_id);
            assert_eq!(payload.index, index);
            assert_eq!(payload.field_id, FIELD);
            assert_eq!(
                payload.aoi_geojson,
                boundary_geojson(),
                "derive aoi is the field boundary"
            );
        }
    }

    // The discover execution recorded the check on the subscription.
    let subscriptions = pipeline::list_subscriptions(&pool, Some(FIELD)).await?;
    assert!(subscriptions[0].last_checked_at.is_some());
    Ok(())
}

#[tokio::test]
async fn discover_skips_already_derived_items() -> Result<()> {
    let tmp = TempDir::new()?;
    let (config, pool) = hub_pool(&tmp).await?;
    seed_field(&pool, FIELD).await?;
    seed_subscription(&pool, FIELD).await?;

    // (item1, ndvi) already derived: its job ran to `succeeded`. Discover
    // must skip it entirely (a plain re-enqueue would reset the terminal
    // row back to queued and re-derive the product).
    let done_key =
        enqueue_derive_marker(&pool, ITEM_1, "ndvi", Utc::now() - Duration::hours(1)).await?;
    let done = pipeline::claim_next_job(&pool, Utc::now())
        .await?
        .expect("claim seeded derive job");
    assert_eq!(done.job_key, done_key);
    pipeline::complete_job(&pool, &done.job_id, Utc::now()).await?;

    // (item1, ndmi) still pending in the queue (run_after in the future so
    // this tick cannot claim it): dedupe keeps the existing row untouched.
    let pending_key =
        enqueue_derive_marker(&pool, ITEM_1, "ndmi", Utc::now() + Duration::hours(1)).await?;

    enqueue_discover(&pool, FIELD).await?;
    let fetcher = Arc::new(FixtureSearchFetcher::new(fixture_items()));
    let ctx = worker_ctx(config, pool.clone(), fetcher);

    let outcome = run_one_tick(&ctx).await?;
    assert_eq!(outcome.kind, Some(JobKind::Discover));
    assert_eq!(outcome.result, Some(JobRunResult::Succeeded));

    let jobs = pipeline::list_jobs(&pool, Some(FIELD)).await?;
    // 1 discover + 4 derive keys total; only the 2 item2 jobs are new.
    assert_eq!(jobs.len(), 5, "{jobs:?}");

    let done_job = jobs.iter().find(|job| job.job_key == done_key).unwrap();
    assert_eq!(
        done_job.status,
        JobStatus::Succeeded,
        "already-derived job must stay succeeded, not be reset"
    );
    assert_eq!(
        done_job.payload_json,
        json!({"marker": "pre-existing"}).to_string()
    );

    let pending_job = jobs.iter().find(|job| job.job_key == pending_key).unwrap();
    assert_eq!(pending_job.status, JobStatus::Queued);
    assert_eq!(
        pending_job.payload_json,
        json!({"marker": "pre-existing"}).to_string(),
        "queued duplicate must deduplicate, not overwrite"
    );

    let new_jobs: Vec<_> = jobs
        .iter()
        .filter(|job| {
            job.kind == JobKind::Derive && job.job_key != done_key && job.job_key != pending_key
        })
        .collect();
    assert_eq!(new_jobs.len(), 2, "only item2's indices are newly enqueued");
    for job in new_jobs {
        assert!(job.job_key.contains(ITEM_2), "{}", job.job_key);
        assert_eq!(job.status, JobStatus::Queued);
    }
    Ok(())
}

// --- Routes -------------------------------------------------------------------

#[tokio::test]
async fn subscription_crud_routes_roundtrip() -> Result<()> {
    let tmp = TempDir::new()?;
    let (config, pool) = hub_pool(&tmp).await?;
    seed_field(&pool, FIELD).await?;
    let app = router(config, pool);

    // Create with defaults for the optional knobs.
    let (status, body) = send_json(
        &app,
        "POST",
        &format!("/api/fields/{FIELD}/subscriptions"),
        Some(json!({ "dataset": "sentinel2", "indices": ["ndvi", "ndmi"] })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["field_id"], FIELD);
    assert_eq!(body["dataset"], "sentinel2");
    assert_eq!(body["indices"], json!(["ndvi", "ndmi"]));
    assert_eq!(body["cadence_hours"], 24);
    assert_eq!(body["max_cloud_cover"], 60.0);
    assert_eq!(body["lookback_days"], 14);
    assert_eq!(body["status"], "active");
    let subscription_id = body["subscription_id"].as_str().unwrap().to_string();

    // Upsert with explicit knobs keeps the id and updates parameters.
    let (status, body) = send_json(
        &app,
        "POST",
        &format!("/api/fields/{FIELD}/subscriptions"),
        Some(json!({
            "dataset": "sentinel2",
            "indices": ["ndvi"],
            "cadence_hours": 48,
            "max_cloud_cover": 30.0,
            "lookback_days": 7,
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["subscription_id"], subscription_id.as_str());
    assert_eq!(body["indices"], json!(["ndvi"]));
    assert_eq!(body["cadence_hours"], 48);

    // List reflects the update.
    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/fields/{FIELD}/subscriptions"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let subscriptions = body["subscriptions"].as_array().expect("subscriptions");
    assert_eq!(subscriptions.len(), 1);
    assert_eq!(subscriptions[0]["lookback_days"], 7);

    // Pause, then verify through the list.
    let (status, body) = send_json(
        &app,
        "PATCH",
        &format!("/api/subscriptions/{subscription_id}"),
        Some(json!({ "status": "paused" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, body) = send_json(
        &app,
        "GET",
        &format!("/api/fields/{FIELD}/subscriptions"),
        None,
    )
    .await?;
    assert_eq!(body["subscriptions"][0]["status"], "paused");

    // Validation failures.
    let (status, _) = send_json(
        &app,
        "POST",
        &format!("/api/fields/{FIELD}/subscriptions"),
        Some(json!({ "dataset": "modis", "indices": ["ndvi"] })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unknown dataset");
    let (status, _) = send_json(
        &app,
        "POST",
        &format!("/api/fields/{FIELD}/subscriptions"),
        Some(json!({ "dataset": "landsat", "indices": [] })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "empty indices");
    let (status, _) = send_json(
        &app,
        "POST",
        "/api/fields/no-such-field/subscriptions",
        Some(json!({ "dataset": "landsat", "indices": ["ndvi"] })),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "unknown field");
    let (status, _) = send_json(
        &app,
        "PATCH",
        &format!("/api/subscriptions/{subscription_id}"),
        Some(json!({ "status": "bogus" })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "invalid status");
    let (status, _) = send_json(
        &app,
        "PATCH",
        "/api/subscriptions/sub:none:none",
        Some(json!({ "status": "paused" })),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "unknown subscription");
    Ok(())
}

#[tokio::test]
async fn manual_run_enqueues_discover() -> Result<()> {
    let tmp = TempDir::new()?;
    let (config, pool) = hub_pool(&tmp).await?;
    seed_field(&pool, FIELD).await?;
    seed_subscription(&pool, FIELD).await?;
    let app = router(config, pool.clone());

    let (status, body) = send_json(
        &app,
        "POST",
        "/api/pipeline/run",
        Some(json!({ "field_id": FIELD })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["field_id"], FIELD);
    let runs = body["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["dataset"], DATASET);
    assert_eq!(runs[0]["outcome"], "enqueued");
    assert_eq!(
        runs[0]["job_key"],
        pipeline::discover_job_key(FIELD, DATASET)
    );

    // The discover job is visible through the jobs listing.
    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/pipeline/jobs?field_id={FIELD}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let jobs = body["jobs"].as_array().expect("jobs");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["kind"], "discover");
    assert_eq!(jobs[0]["status"], "queued");

    // Status filter works and a bogus status is rejected.
    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/pipeline/jobs?field_id={FIELD}&status=queued&limit=10"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["jobs"].as_array().unwrap().len(), 1);
    let (status, _) = send_json(
        &app,
        "GET",
        &format!("/api/pipeline/jobs?field_id={FIELD}&status=bogus"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Re-running deduplicates on the discover job key.
    let (status, body) = send_json(
        &app,
        "POST",
        "/api/pipeline/run",
        Some(json!({ "field_id": FIELD, "dataset": DATASET })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["runs"][0]["outcome"], "deduplicated");

    // No subscription -> nothing to run.
    let (status, _) = send_json(
        &app,
        "POST",
        "/api/pipeline/run",
        Some(json!({ "field_id": "no-such-field" })),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send_json(
        &app,
        "POST",
        "/api/pipeline/run",
        Some(json!({ "field_id": FIELD, "dataset": "landsat" })),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "no landsat subscription");
    Ok(())
}

#[tokio::test]
async fn retry_resets_dead_job() -> Result<()> {
    let tmp = TempDir::new()?;
    let (config, pool) = hub_pool(&tmp).await?;
    let app = router(config, pool.clone());

    // Seed a dead job: claim it, then fail it with a client error.
    enqueue_discover(&pool, FIELD).await?;
    let job = pipeline::claim_next_job(&pool, Utc::now())
        .await?
        .expect("claim seeded job");
    pipeline::fail_job(&pool, &job, "boom: permanent", true, Utc::now()).await?;

    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/pipeline/jobs/{}", job.job_id),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "dead");

    let (status, body) = send_json(
        &app,
        "POST",
        &format!("/api/pipeline/jobs/{}/retry", job.job_id),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "queued");
    assert_eq!(body["attempts"], 0);

    let (status, body) = send_json(
        &app,
        "GET",
        &format!("/api/pipeline/jobs/{}", job.job_id),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "queued");
    assert_eq!(body["attempts"], 0);

    // A queued job is not retryable; unknown ids are 404.
    let (status, _) = send_json(
        &app,
        "POST",
        &format!("/api/pipeline/jobs/{}/retry", job.job_id),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = send_json(&app, "POST", "/api/pipeline/jobs/job:none/retry", None).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send_json(&app, "GET", "/api/pipeline/jobs/job:none", None).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}
