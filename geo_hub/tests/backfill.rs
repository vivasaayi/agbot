//! Batch S-11 integration tests: resumable historical backfill.
//!
//! A backfill run walks a long date range (1982+) in 90-day chunks, one
//! chunk per `backfill_enumerate` job execution: search the STAC seam for
//! the chunk, fan `item x index` out into priority -10 derive jobs tagged
//! with the backfill id, advance the per-dataset cursor, and chain the next
//! enumerate job. Pausing parks the chain; resuming re-enqueues it and the
//! run continues from the stored cursor, not from the start.
//!
//! The STAC seam is a fixture [`StacItemFetcher`], so no test touches the
//! network.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::backfill;
use geo_hub::earth_search::EarthSearchItem;
use geo_hub::pipeline::{self, JobKind, JobStatus};
use geo_hub::pipeline_worker::{
    run_one_tick, BoxFuture, ItemFetchError, JobRunResult, PipelineWorkerContext, StacItemFetcher,
    StacSearchQuery,
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

// --- Fixtures ----------------------------------------------------------------

/// Enumerate never reads COGs, so the resolver points at an empty store.
struct EmptyStoreResolver(Arc<InMemory>);

impl CogStoreResolver for EmptyStoreResolver {
    fn resolve(
        &self,
        href: &str,
    ) -> std::result::Result<(Arc<dyn ObjectStore>, String), DerivationError> {
        Ok((self.0.clone(), href.to_string()))
    }
}

/// Fixture search seam: records every range query and returns one canned
/// item per query whose id encodes `(dataset, chunk start)`, so every chunk
/// produces a distinct derive job key. `fetch_item` fails transiently, so
/// fanned-out derive jobs park with retry backoff instead of dead-lettering,
/// keeping them observable while the enumerate chain drains.
struct FixtureSearchFetcher {
    queries: Arc<Mutex<Vec<StacSearchQuery>>>,
}

impl FixtureSearchFetcher {
    fn new() -> Self {
        Self {
            queries: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

fn canned_item(id: &str, datetime: &str) -> EarthSearchItem {
    serde_json::from_value(json!({
        "id": id,
        "properties": { "datetime": datetime },
    }))
    .expect("canned item decodes")
}

impl StacItemFetcher for FixtureSearchFetcher {
    fn fetch_item<'a>(
        &'a self,
        _collection: &'a str,
        _item_id: &'a str,
    ) -> BoxFuture<'a, std::result::Result<EarthSearchItem, ItemFetchError>> {
        Box::pin(async move {
            Err(ItemFetchError::Upstream(
                "fixture store has no rasters".to_string(),
            ))
        })
    }

    fn search<'a>(
        &'a self,
        query: &'a StacSearchQuery,
    ) -> BoxFuture<'a, std::result::Result<Vec<EarthSearchItem>, ItemFetchError>> {
        Box::pin(async move {
            self.queries.lock().unwrap().push(query.clone());
            let start_date = &query.start_iso[..10];
            let id = format!("{}_{}", query.dataset, start_date);
            Ok(vec![canned_item(&id, &format!("{start_date}T10:00:00Z"))])
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
            tmp.path().join("backfill.db").display()
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

/// Create a backfill run through the route and return its id.
async fn start_backfill(app: &Router, datasets: &[&str], start: &str, end: &str) -> Result<String> {
    let (status, body) = send_json(
        app,
        "POST",
        &format!("/api/fields/{FIELD}/backfill"),
        Some(json!({
            "datasets": datasets,
            "indices": ["ndvi"],
            "start": start,
            "end": end,
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    Ok(body["backfill_id"]
        .as_str()
        .expect("backfill_id")
        .to_string())
}

/// Tick the worker until the run reaches `expected_status` (bounded).
async fn tick_until_status(
    ctx: &PipelineWorkerContext,
    pool: &db::DbPool,
    backfill_id: &str,
    expected_status: &str,
    max_ticks: usize,
) -> Result<()> {
    for _ in 0..max_ticks {
        let run = backfill::get_backfill_run(pool, backfill_id)
            .await?
            .expect("run exists");
        if run.status == expected_status {
            return Ok(());
        }
        run_one_tick(ctx).await?;
    }
    let run = backfill::get_backfill_run(pool, backfill_id).await?;
    panic!("run never reached {expected_status}: {run:?}");
}

// --- Enumerate chunk walking -----------------------------------------------------

#[tokio::test]
async fn enumerate_chunks_date_range_and_advances_cursor() -> Result<()> {
    let tmp = TempDir::new()?;
    let (config, pool) = hub_pool(&tmp).await?;
    seed_field(&pool, FIELD).await?;
    let app = router(Arc::clone(&config), pool.clone());

    // 2024-01-01 .. 2024-04-01 = one full 90-day chunk plus a 1-day tail,
    // per dataset.
    let backfill_id =
        start_backfill(&app, &["sentinel2", "landsat"], "2024-01-01", "2024-04-01").await?;

    let fetcher = Arc::new(FixtureSearchFetcher::new());
    let queries = Arc::clone(&fetcher.queries);
    let ctx = worker_ctx(Arc::clone(&config), pool.clone(), fetcher);

    // First tick executes the first enumerate job: one chunk of the first
    // dataset.
    let outcome = run_one_tick(&ctx).await?;
    assert!(outcome.claimed);
    assert_eq!(outcome.kind, Some(JobKind::BackfillEnumerate));
    assert_eq!(outcome.result, Some(JobRunResult::Succeeded));
    assert!(outcome.hit_provider, "enumerate talks to the STAC provider");

    let run = backfill::get_backfill_run(&pool, &backfill_id)
        .await?
        .expect("run exists");
    assert_eq!(run.status, "running");
    assert_eq!(
        run.cursor.get("sentinel2").map(String::as_str),
        Some("2024-03-31"),
        "cursor advanced 90 days: {:?}",
        run.cursor
    );
    assert!(!run.cursor.contains_key("landsat"), "landsat untouched");
    assert_eq!(run.scenes_discovered, 1);
    assert_eq!(run.jobs_enqueued, 1);

    // The fanned-out derive job runs at backfill priority with the run id.
    let derive_key = pipeline::derive_job_key("sentinel2", "sentinel2_2024-01-01", "ndvi", FIELD);
    let derive = pipeline::find_job_by_key(&pool, &derive_key)
        .await?
        .expect("derive job enqueued");
    assert_eq!(derive.kind, JobKind::Derive);
    assert_eq!(derive.priority, -10);
    assert_eq!(derive.backfill_id.as_deref(), Some(backfill_id.as_str()));
    assert_eq!(derive.field_id.as_deref(), Some(FIELD));

    // The chain re-enqueued itself under a per-chunk job key.
    let next_key = pipeline::backfill_enum_job_key(&backfill_id, "sentinel2:2024-03-31");
    let chained = pipeline::find_job_by_key(&pool, &next_key)
        .await?
        .expect("chained enumerate job");
    assert_eq!(chained.kind, JobKind::BackfillEnumerate);
    assert_eq!(chained.status, JobStatus::Queued);
    assert_eq!(chained.priority, -10);

    // Drain the whole run: 4 chunks total across both datasets.
    tick_until_status(&ctx, &pool, &backfill_id, "completed", 40).await?;

    let recorded = queries.lock().unwrap().clone();
    let ranges: Vec<(String, String, String)> = recorded
        .iter()
        .map(|q| {
            (
                q.dataset.clone(),
                q.start_iso[..10].to_string(),
                q.end_iso[..10].to_string(),
            )
        })
        .collect();
    assert_eq!(
        ranges,
        vec![
            ("sentinel2".into(), "2024-01-01".into(), "2024-03-31".into()),
            ("sentinel2".into(), "2024-03-31".into(), "2024-04-01".into()),
            ("landsat".into(), "2024-01-01".into(), "2024-03-31".into()),
            ("landsat".into(), "2024-03-31".into(), "2024-04-01".into()),
        ],
        "datasets walked in order, 90-day chunks with a partial tail"
    );
    for query in &recorded {
        assert_eq!(query.max_cloud_cover, 70.0, "default cloud ceiling");
    }

    let run = backfill::get_backfill_run(&pool, &backfill_id)
        .await?
        .expect("run exists");
    assert_eq!(run.status, "completed");
    assert_eq!(
        run.cursor.get("sentinel2").map(String::as_str),
        Some("2024-04-01")
    );
    assert_eq!(
        run.cursor.get("landsat").map(String::as_str),
        Some("2024-04-01")
    );
    assert_eq!(run.scenes_discovered, 4);
    assert_eq!(run.jobs_enqueued, 4);

    // One derive job per chunk x index, all tagged and prioritized.
    let jobs = pipeline::list_jobs(&pool, Some(FIELD)).await?;
    let derives: Vec<_> = jobs.iter().filter(|j| j.kind == JobKind::Derive).collect();
    assert_eq!(derives.len(), 4, "{jobs:?}");
    for job in derives {
        assert_eq!(job.priority, -10);
        assert_eq!(job.backfill_id.as_deref(), Some(backfill_id.as_str()));
    }
    Ok(())
}

#[tokio::test]
async fn backfill_resumes_from_cursor_after_restart() -> Result<()> {
    let tmp = TempDir::new()?;
    let (config, pool) = hub_pool(&tmp).await?;
    seed_field(&pool, FIELD).await?;
    let app = router(Arc::clone(&config), pool.clone());

    let backfill_id = start_backfill(&app, &["sentinel2"], "2024-01-01", "2024-04-01").await?;

    let fetcher = Arc::new(FixtureSearchFetcher::new());
    let queries = Arc::clone(&fetcher.queries);
    let ctx = worker_ctx(Arc::clone(&config), pool.clone(), fetcher);

    // Process the first chunk, then pause mid-run.
    let outcome = run_one_tick(&ctx).await?;
    assert_eq!(outcome.kind, Some(JobKind::BackfillEnumerate));
    let (status, body) = send_json(
        &app,
        "POST",
        &format!("/api/backfills/{backfill_id}/pause"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "paused");

    // The chained enumerate job parks as a no-op ("restart" of the chain):
    // tick until it leaves the queue without doing work.
    let parked_key = pipeline::backfill_enum_job_key(&backfill_id, "sentinel2:2024-03-31");
    for _ in 0..5 {
        let job = pipeline::find_job_by_key(&pool, &parked_key)
            .await?
            .expect("chained job exists");
        if job.status == JobStatus::Succeeded {
            break;
        }
        run_one_tick(&ctx).await?;
    }
    let parked = pipeline::find_job_by_key(&pool, &parked_key)
        .await?
        .expect("chained job exists");
    assert_eq!(parked.status, JobStatus::Succeeded, "parked as no-op");
    assert_eq!(queries.lock().unwrap().len(), 1, "no search while paused");
    let run = backfill::get_backfill_run(&pool, &backfill_id)
        .await?
        .expect("run exists");
    assert_eq!(run.status, "paused");
    assert_eq!(
        run.cursor.get("sentinel2").map(String::as_str),
        Some("2024-03-31"),
        "cursor survives the pause"
    );

    // Resume re-enqueues the enumerate chain.
    let (status, body) = send_json(
        &app,
        "POST",
        &format!("/api/backfills/{backfill_id}/resume"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "running");
    let resumed = pipeline::find_job_by_key(&pool, &parked_key)
        .await?
        .expect("chained job exists");
    assert_eq!(resumed.status, JobStatus::Queued, "resume re-enqueues");

    tick_until_status(&ctx, &pool, &backfill_id, "completed", 20).await?;

    // The post-resume search continued from the cursor, not from the start.
    let recorded = queries.lock().unwrap().clone();
    assert_eq!(recorded.len(), 2, "one search per chunk, no re-walk");
    assert_eq!(&recorded[1].start_iso[..10], "2024-03-31");
    assert_eq!(&recorded[1].end_iso[..10], "2024-04-01");
    Ok(())
}

#[tokio::test]
async fn backfill_progress_counts_jobs() -> Result<()> {
    let tmp = TempDir::new()?;
    let (config, pool) = hub_pool(&tmp).await?;
    seed_field(&pool, FIELD).await?;
    let app = router(Arc::clone(&config), pool.clone());

    let backfill_id = start_backfill(&app, &["sentinel2"], "2024-01-01", "2024-04-01").await?;

    let fetcher = Arc::new(FixtureSearchFetcher::new());
    let ctx = worker_ctx(Arc::clone(&config), pool.clone(), fetcher);
    run_one_tick(&ctx).await?;

    // After one chunk: the first enumerate succeeded; one derive plus the
    // chained enumerate are queued. All three carry the backfill id.
    let (status, body) =
        send_json(&app, "GET", &format!("/api/backfills/{backfill_id}"), None).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["backfill_id"], backfill_id.as_str());
    assert_eq!(body["status"], "running");
    assert_eq!(body["cursor"]["sentinel2"], "2024-03-31");
    assert_eq!(body["progress"]["by_status"]["succeeded"], 1, "{body}");
    assert_eq!(body["progress"]["by_status"]["queued"], 2, "{body}");
    assert_eq!(body["progress"]["total"], 3, "{body}");
    assert_eq!(body["scenes_discovered"], 1);
    assert_eq!(body["jobs_enqueued"], 1);

    // The field listing shows the run.
    let (status, body) =
        send_json(&app, "GET", &format!("/api/fields/{FIELD}/backfills"), None).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let runs = body["backfills"].as_array().expect("backfills array");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["backfill_id"], backfill_id.as_str());

    // Unknown run ids are 404.
    let (status, _) = send_json(&app, "GET", "/api/backfills/backfill:none", None).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

// --- Validation ---------------------------------------------------------------

#[tokio::test]
async fn hls_dataset_rejected() -> Result<()> {
    let tmp = TempDir::new()?;
    let (config, pool) = hub_pool(&tmp).await?;
    seed_field(&pool, FIELD).await?;
    let app = router(config, pool);

    let (status, body) = send_json(
        &app,
        "POST",
        &format!("/api/fields/{FIELD}/backfill"),
        Some(json!({
            "datasets": ["sentinel2", "hls"],
            "indices": ["ndvi"],
            "start": "2024-01-01",
            "end": "2024-04-01",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let message = body["description"].as_str().unwrap_or_default();
    assert!(message.contains("hls"), "error names the dataset: {body}");

    // Other validation failures.
    let bad_bodies = [
        json!({ "datasets": [], "indices": ["ndvi"], "start": "2024-01-01", "end": "2024-04-01" }),
        json!({ "datasets": ["landsat"], "indices": [], "start": "2024-01-01", "end": "2024-04-01" }),
        json!({ "datasets": ["landsat"], "indices": ["ndvi"], "start": "2024-04-01", "end": "2024-01-01" }),
        json!({ "datasets": ["landsat"], "indices": ["ndvi"], "start": "2024-01-01", "end": "2024-01-01" }),
        json!({ "datasets": ["landsat"], "indices": ["ndvi"], "start": "junk", "end": "2024-04-01" }),
        json!({ "datasets": ["modis"], "indices": ["ndvi"], "start": "2024-01-01", "end": "2024-04-01" }),
    ];
    for bad in bad_bodies {
        let (status, body) = send_json(
            &app,
            "POST",
            &format!("/api/fields/{FIELD}/backfill"),
            Some(bad.clone()),
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad} -> {body}");
    }

    // Unknown fields are 404.
    let (status, _) = send_json(
        &app,
        "POST",
        "/api/fields/no-such-field/backfill",
        Some(json!({
            "datasets": ["landsat"],
            "indices": ["ndvi"],
            "start": "2024-01-01",
            "end": "2024-04-01",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn paused_run_parks_enumerate_job() -> Result<()> {
    let tmp = TempDir::new()?;
    let (config, pool) = hub_pool(&tmp).await?;
    seed_field(&pool, FIELD).await?;
    let app = router(Arc::clone(&config), pool.clone());

    let backfill_id = start_backfill(&app, &["sentinel2"], "2024-01-01", "2024-04-01").await?;
    let (status, body) = send_json(
        &app,
        "POST",
        &format!("/api/backfills/{backfill_id}/pause"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");

    let fetcher = Arc::new(FixtureSearchFetcher::new());
    let queries = Arc::clone(&fetcher.queries);
    let ctx = worker_ctx(config, pool.clone(), fetcher);

    let outcome = run_one_tick(&ctx).await?;
    assert!(outcome.claimed);
    assert_eq!(outcome.kind, Some(JobKind::BackfillEnumerate));
    assert_eq!(
        outcome.result,
        Some(JobRunResult::Succeeded),
        "paused run completes the job as a parked no-op"
    );

    assert!(queries.lock().unwrap().is_empty(), "no search while paused");
    let run = backfill::get_backfill_run(&pool, &backfill_id)
        .await?
        .expect("run exists");
    assert_eq!(run.status, "paused");
    assert!(run.cursor.is_empty(), "cursor untouched");
    assert_eq!(run.scenes_discovered, 0);

    // Nothing new was scheduled: the only tagged job is the parked enumerate.
    let jobs = pipeline::list_jobs(&pool, Some(FIELD)).await?;
    let tagged: Vec<_> = jobs
        .iter()
        .filter(|j| j.backfill_id.as_deref() == Some(backfill_id.as_str()))
        .collect();
    assert_eq!(tagged.len(), 1, "{jobs:?}");
    assert_eq!(tagged[0].kind, JobKind::BackfillEnumerate);
    assert_eq!(tagged[0].status, JobStatus::Succeeded);
    Ok(())
}
