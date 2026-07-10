//! Integration tests for the orchestrated L3 suite (batch S-12): the
//! `l3_recompute` job now carries a `product` selector, and the
//! `monthly_composite` success path fans out into follow-up recomputes:
//! seasonal `climatology` (>= 2 same-calendar-month composites across years),
//! `phenology` (season-end month, >= 3 months in the season), and the
//! `drought_stack` (VCI/TCI/VHI/SPI) which then enqueues the `drought_watch`
//! app run.
//!
//! Fixtures seed monthly `temporal_composite` products directly (tiny 2x2
//! GeoTIFFs on a shared grid), the way the monthly composite recompute would
//! have registered them, so a `product = "climatology"` /
//! `"drought_stack"` recompute has real inputs to consume.

use std::sync::Arc;

use anyhow::Result;
use chrono::{Duration, Utc};
use geo_hub::catalog::{self, ProductFilter};
use geo_hub::config::PipelineConfig;
use geo_hub::earth_search::EarthSearchItem;
use geo_hub::pipeline::{self, JobKind, JobStatus, L3RecomputePayload};
use geo_hub::pipeline_worker::{
    run_one_tick, BoxFuture, ItemFetchError, JobRunResult, PipelineWorkerContext, StacItemFetcher,
};
use geo_hub::satellite_derivation::{CogStoreResolver, DerivationError};
use geo_hub::{db, HubConfig};
use raster_io::object_store::memory::InMemory;
use raster_io::object_store::ObjectStore;
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde_json::json;
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use tempfile::TempDir;

const DATASET: &str = "sentinel-2-l2a";
const EPSG: u32 = 32643;
const TRANSFORM: [f64; 6] = [600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];
const NODATA: f32 = -9999.0;

// --- Fixtures ----------------------------------------------------------------

struct MemResolver(#[allow(dead_code)] Arc<InMemory>);

impl CogStoreResolver for MemResolver {
    fn resolve(
        &self,
        href: &str,
    ) -> std::result::Result<(Arc<dyn ObjectStore>, String), DerivationError> {
        Ok((self.0.clone(), href.to_string()))
    }
}

/// The worker never fetches an item in these tests (no derive jobs run);
/// a stub fetcher keeps the context complete.
struct StubFetcher;

impl StacItemFetcher for StubFetcher {
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
}

async fn worker_ctx() -> Result<(TempDir, PipelineWorkerContext)> {
    let tmp = TempDir::new()?;
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("l3_suite.db").display()
        ),
        data_root: tmp.path().join("data"),
        pipeline: PipelineConfig {
            enabled: true,
            poll_interval_ms: 1000,
            provider_min_delay_ms: 0,
            ..PipelineConfig::default()
        },
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let ctx = PipelineWorkerContext {
        pool,
        config: Arc::new(config),
        cog_resolver: Arc::new(MemResolver(Arc::new(InMemory::new()))),
        item_fetcher: Arc::new(StubFetcher),
    };
    Ok((tmp, ctx))
}

/// Register a field-scoped single-band `temporal_composite` L3 (band_names =
/// [kind]) for `month`, the way the monthly composite recompute would.
async fn register_composite(
    ctx: &PipelineWorkerContext,
    tmp: &TempDir,
    field_id: &str,
    kind: &str,
    month: &str,
    values: Vec<f32>,
) -> Result<String> {
    let path = tmp
        .path()
        .join(format!("composite_{field_id}_{kind}_{month}.tif"));
    write_geotiff_f32(
        &path,
        2,
        2,
        &values,
        &GeoTiffTags {
            epsg: Some(EPSG),
            geo_transform: Some(TRANSFORM),
            nodata: Some(f64::from(NODATA)),
        },
    )?;
    let (year, mon) = month.split_once('-').expect("YYYY-MM");
    let draft = ProductRecordDraft {
        level: ProductLevel::L3,
        kind: "temporal_composite".to_string(),
        algorithm_id: "test.pipeline_l3_suite.composite".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: json!({ "band_names": [kind], "month": month }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: Some(field_id.to_string()),
            season_id: Some("2026-kharif".to_string()),
            scene_id: None,
            temporal_start: format!("{year}-{mon}-01T00:00:00Z"),
            temporal_end: format!("{year}-{mon}-28T23:59:59Z"),
        },
        spatial_ref: None,
        gsd_m_per_px: Some(10.0),
        artifact: Some(ProductArtifact {
            format: "tif".to_string(),
            path: path.to_string_lossy().to_string(),
            checksum_sha256: None,
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some("earth-search:sentinel-2-l2a".to_string()),
    };
    Ok(catalog::register_product(&ctx.pool, &draft, "2026-07-05T00:00:00Z").await?)
}

async fn subscribe(ctx: &PipelineWorkerContext, field_id: &str) -> Result<()> {
    let record = pipeline::upsert_subscription(
        &ctx.pool,
        &pipeline::SubscriptionUpsert {
            field_id: field_id.to_string(),
            dataset: DATASET.to_string(),
            indices: vec!["ndvi".to_string()],
            cadence_hours: 24,
            max_cloud_cover: 40.0,
            lookback_days: 14,
        },
        Utc::now(),
    )
    .await?;
    // Mark it checked so the cadence pass does not interleave a discover job
    // during these single-stepped L3 ticks (the drought-stack gate only needs
    // the subscription to be active, not due).
    pipeline::touch_subscription_checked(&ctx.pool, &record.subscription_id, Utc::now()).await?;
    Ok(())
}

/// Enqueue an `l3_recompute` for one product selector, ready to run now.
async fn enqueue_l3_product(
    ctx: &PipelineWorkerContext,
    field_id: &str,
    index: &str,
    product: &str,
    bucket: &str,
) -> Result<()> {
    let now = Utc::now();
    let payload = json!({
        "field_id": field_id,
        "dataset": DATASET,
        "index": index,
        "month": bucket,
        "product": product,
    });
    let job_key = pipeline::l3_suite_job_key(field_id, DATASET, index, product, bucket);
    pipeline::enqueue_job(
        &ctx.pool,
        JobKind::L3Recompute,
        &job_key,
        &payload,
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

/// Drain the queue by ticking until no job is claimed (bounded).
async fn drain(ctx: &PipelineWorkerContext) -> Result<()> {
    for _ in 0..64 {
        let outcome = run_one_tick(ctx).await?;
        if !outcome.claimed {
            break;
        }
    }
    Ok(())
}

// --- Payload default ---------------------------------------------------------

#[test]
fn l3_payload_defaults_product_to_monthly_composite() {
    let payload: L3RecomputePayload = serde_json::from_value(json!({
        "field_id": "f", "dataset": DATASET, "index": "ndvi", "month": "2026-06",
    }))
    .expect("payload without product decodes");
    assert_eq!(payload.product, "monthly_composite");
}

// --- climatology -------------------------------------------------------------

/// A new month's composite triggers a climatology recompute; with two same
/// calendar-month composites across years the NDVI climatology registers.
#[tokio::test]
async fn new_month_composite_enqueues_climatology() -> Result<()> {
    let (tmp, ctx) = worker_ctx().await?;
    // Two Junes across years -> the calendar-June climatology has 2 years.
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2024-06", vec![0.2; 4]).await?;
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2025-06", vec![0.6; 4]).await?;

    enqueue_l3_product(&ctx, "field-1", "ndvi", "climatology", "2025-06").await?;
    let outcome = run_one_tick(&ctx).await?;
    assert_eq!(outcome.kind, Some(JobKind::L3Recompute));
    assert_eq!(outcome.result, Some(JobRunResult::Succeeded), "{outcome:?}");

    let climatologies = catalog::list_products(
        &ctx.pool,
        &ProductFilter {
            kind: Some("index_climatology".to_string()),
            level: Some(ProductLevel::L3),
            status: Some("registered".to_string()),
            field_id: Some("field-1".to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;
    assert_eq!(climatologies.len(), 1, "{climatologies:?}");
    assert_eq!(climatologies[0].parameters["index_kind"], "ndvi");
    Ok(())
}

/// With only one calendar-month composite the climatology recompute is a
/// logged no-op (needs >= 2 years), not a failure.
#[tokio::test]
async fn climatology_requires_two_years_else_noop() -> Result<()> {
    let (tmp, ctx) = worker_ctx().await?;
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2025-06", vec![0.6; 4]).await?;

    enqueue_l3_product(&ctx, "field-1", "ndvi", "climatology", "2025-06").await?;
    let outcome = run_one_tick(&ctx).await?;
    assert_eq!(outcome.result, Some(JobRunResult::Succeeded), "{outcome:?}");

    let climatologies = catalog::list_products(
        &ctx.pool,
        &ProductFilter {
            kind: Some("index_climatology".to_string()),
            level: Some(ProductLevel::L3),
            status: Some("registered".to_string()),
            field_id: Some("field-1".to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;
    assert!(
        climatologies.is_empty(),
        "no climatology from one year: {climatologies:?}"
    );
    Ok(())
}

// --- phenology ---------------------------------------------------------------

/// A season-end month (09/10) composite triggers a phenology recompute; with
/// >= 3 months in the season a `phenology` L3 registers.
#[tokio::test]
async fn season_end_composite_enqueues_phenology() -> Result<()> {
    let (tmp, ctx) = worker_ctx().await?;
    // Three months of the 2026 kharif season, rising NDVI.
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2026-07", vec![0.2; 4]).await?;
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2026-08", vec![0.5; 4]).await?;
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2026-09", vec![0.7; 4]).await?;

    enqueue_l3_product(&ctx, "field-1", "ndvi", "phenology", "2026-09").await?;
    let outcome = run_one_tick(&ctx).await?;
    assert_eq!(outcome.result, Some(JobRunResult::Succeeded), "{outcome:?}");

    let phenology = catalog::list_products(
        &ctx.pool,
        &ProductFilter {
            kind: Some("phenology".to_string()),
            level: Some(ProductLevel::L3),
            status: Some("registered".to_string()),
            field_id: Some("field-1".to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;
    assert_eq!(phenology.len(), 1, "phenology L3 registered: {phenology:?}");
    Ok(())
}

/// Fewer than three months in the season -> phenology is a logged no-op.
#[tokio::test]
async fn phenology_requires_three_months_else_noop() -> Result<()> {
    let (tmp, ctx) = worker_ctx().await?;
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2026-08", vec![0.5; 4]).await?;
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2026-09", vec![0.7; 4]).await?;

    enqueue_l3_product(&ctx, "field-1", "ndvi", "phenology", "2026-09").await?;
    let outcome = run_one_tick(&ctx).await?;
    assert_eq!(outcome.result, Some(JobRunResult::Succeeded), "{outcome:?}");

    let phenology = catalog::list_products(
        &ctx.pool,
        &ProductFilter {
            kind: Some("phenology".to_string()),
            level: Some(ProductLevel::L3),
            status: Some("registered".to_string()),
            field_id: Some("field-1".to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;
    assert!(
        phenology.is_empty(),
        "no phenology from 2 months: {phenology:?}"
    );
    Ok(())
}

// --- drought stack -----------------------------------------------------------

/// The drought stack for a subscribed field: with an NDVI climatology present
/// (built from composites) VCI registers; VHI is skipped (no LST) as a logged
/// no-op, not an error; and a `drought_watch` app run is enqueued.
#[tokio::test]
async fn drought_stack_recomputes_for_subscribed_field() -> Result<()> {
    let (tmp, ctx) = worker_ctx().await?;
    subscribe(&ctx, "field-1").await?;
    // NDVI composites across years -> climatology, plus a current month.
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2024-06", vec![0.2; 4]).await?;
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2025-06", vec![0.6; 4]).await?;
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2026-06", vec![0.4; 4]).await?;

    // Climatology must exist first (VCI gates on it).
    enqueue_l3_product(&ctx, "field-1", "ndvi", "climatology", "2026-06").await?;
    run_one_tick(&ctx).await?;

    enqueue_l3_product(&ctx, "field-1", "ndvi", "drought_stack", "2026-06").await?;
    let outcome = run_one_tick(&ctx).await?;
    assert_eq!(outcome.kind, Some(JobKind::L3Recompute));
    assert_eq!(
        outcome.result,
        Some(JobRunResult::Succeeded),
        "drought stack completes (VHI skip is a no-op, not an error): {outcome:?}"
    );

    // VCI registered.
    let (_climatologies, droughts) = geo_hub::drought_rasters::list_drought_raster_products(
        &ctx.pool,
        Some("field-1".to_string()),
    )
    .await?;
    assert!(
        droughts.iter().any(|p| p.parameters["index_kind"] == "vci"),
        "VCI registered: {droughts:?}"
    );
    // No VHI (no LST/TCI available).
    assert!(
        !droughts.iter().any(|p| p.parameters["index_kind"] == "vhi"),
        "VHI skipped without LST: {droughts:?}"
    );

    // drought_watch AppRun enqueued.
    let jobs = pipeline::list_jobs(&ctx.pool, Some("field-1")).await?;
    assert!(
        jobs.iter().any(|j| {
            j.kind == JobKind::AppRun
                && j.payload_json.contains("drought_watch")
                && j.status == JobStatus::Queued
        }),
        "drought_watch app run enqueued: {jobs:?}"
    );
    Ok(())
}

/// The enqueued `drought_watch` app run, when executed, records stress
/// findings and fires a Track C alert on the stressed drought product.
#[tokio::test]
async fn drought_watch_app_run_fires_alerts() -> Result<()> {
    let (tmp, ctx) = worker_ctx().await?;
    subscribe(&ctx, "field-1").await?;
    // A current NDVI that scores low against the baseline -> stressed VCI.
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2024-06", vec![0.6; 4]).await?;
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2025-06", vec![0.9; 4]).await?;
    register_composite(&ctx, &tmp, "field-1", "ndvi", "2026-06", vec![0.6; 4]).await?;

    enqueue_l3_product(&ctx, "field-1", "ndvi", "climatology", "2026-06").await?;
    run_one_tick(&ctx).await?;
    enqueue_l3_product(&ctx, "field-1", "ndvi", "drought_stack", "2026-06").await?;
    // Run drought_stack, then drain the enqueued drought_watch app run.
    drain(&ctx).await?;

    let jobs = pipeline::list_jobs(&ctx.pool, Some("field-1")).await?;
    let app_runs: Vec<_> = jobs
        .iter()
        .filter(|j| j.kind == JobKind::AppRun && j.payload_json.contains("drought_watch"))
        .collect();
    assert!(!app_runs.is_empty(), "drought_watch app run was enqueued");
    assert!(
        app_runs.iter().all(|j| j.status == JobStatus::Succeeded),
        "drought_watch app run succeeded: {app_runs:?}"
    );

    let findings = geo_hub::applications::list_field_findings(&ctx.pool, "field-1").await?;
    assert!(
        findings.iter().any(|f| f.app_id == "drought_watch"),
        "drought_watch findings recorded: {findings:?}"
    );
    Ok(())
}
