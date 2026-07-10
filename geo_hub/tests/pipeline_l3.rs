//! Integration tests for the pipeline's L3/application stage (batch S-9):
//! a successful derive fans out into a monthly `l3_recompute` job plus
//! `app_run` jobs, the L3 handler composites the field's monthly L2 series
//! (superseding the previous composite), the app handler records governed
//! application findings with lineage and evaluates field alerts, and both
//! handlers complete as no-ops when the field has no usable inputs.
//!
//! Fixtures reuse the network-free patterns from `tests/pipeline_worker.rs`
//! (in-memory COG store + captured Earth Search item) and
//! `tests/composite_rasters.rs` (tiny cataloged NDVI GeoTIFFs on a shared
//! grid).

use std::sync::Arc;

use anyhow::Result;
use chrono::{Duration, Utc};
use geo_hub::catalog;
use geo_hub::composite_rasters;
use geo_hub::config::PipelineConfig;
use geo_hub::earth_search::EarthSearchItem;
use geo_hub::field_timeseries;
use geo_hub::pipeline::{
    self, AppRunPayload, DerivePayload, JobKind, JobStatus, L3RecomputePayload,
};
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
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde_json::json;
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use tempfile::TempDir;

const FIXTURE_COLLECTION: &str = "sentinel-2-l2a";
const FIXTURE_ITEM_ID: &str = "S2B_43PFN_20230128_0_L2A";
const EPSG: u16 = 32643;
const TRANSFORM_10M: [f64; 6] = [600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];
const TRANSFORM_20M: [f64; 6] = [600_000.0, 20.0, 0.0, 1_300_020.0, 0.0, -20.0];
const NODATA: f32 = -9999.0;

// --- Fixtures (same pattern as tests/pipeline_worker.rs) ---------------------

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

/// Fixture bands: red DN 2000, nir DN 6000, SCL class 4 (vegetation).
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

fn fixture_item() -> EarthSearchItem {
    let mut item: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/earth_search_s2_item.json"))
            .expect("parse captured item");
    for (asset, name) in [("red", "red"), ("nir", "nir"), ("scl", "scl")] {
        item["assets"][asset]["href"] = json!(format!("https://cogs.test/fixtures/{name}.tif"));
    }
    serde_json::from_value(item).expect("captured item decodes as EarthSearchItem")
}

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

async fn worker_ctx() -> Result<(TempDir, PipelineWorkerContext)> {
    let tmp = TempDir::new()?;
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", tmp.path().join("l3.db").display()),
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
        cog_resolver: Arc::new(MemResolver(fixture_store().await)),
        item_fetcher: Arc::new(FixtureItemFetcher),
    };
    Ok((tmp, ctx))
}

/// Register a field-scoped 2x2 NDVI L2 GeoTIFF on the shared 10 m grid
/// (same pattern as tests/composite_rasters.rs, plus field/season scope so
/// the field-scoped L3 recompute and the time-series extractor can see it).
async fn register_field_ndvi(
    ctx: &PipelineWorkerContext,
    tmp: &TempDir,
    field_id: &str,
    stamp: &str,
    values: Vec<f32>,
) -> Result<String> {
    let path = tmp.path().join(format!("ndvi_{field_id}_{stamp}.tif"));
    write_geotiff_f32(
        &path,
        2,
        2,
        &values,
        &GeoTiffTags {
            epsg: Some(u32::from(EPSG)),
            geo_transform: Some(TRANSFORM_10M),
            nodata: Some(f64::from(NODATA)),
        },
    )?;
    let draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: "ndvi".to_string(),
        algorithm_id: "test.pipeline_l3".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: json!({ "stamp": stamp, "field": field_id }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: Some(field_id.to_string()),
            season_id: Some("2026-kharif".to_string()),
            scene_id: Some(format!("scene-{stamp}")),
            temporal_start: format!("{stamp}T10:30:00Z"),
            temporal_end: format!("{stamp}T10:30:00Z"),
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

async fn enqueue_l3(ctx: &PipelineWorkerContext, field_id: &str, month: &str) -> Result<()> {
    let now = Utc::now();
    pipeline::enqueue_job(
        &ctx.pool,
        JobKind::L3Recompute,
        &pipeline::l3_job_key(field_id, FIXTURE_COLLECTION, "ndvi", month),
        &serde_json::to_value(L3RecomputePayload {
            field_id: field_id.to_string(),
            dataset: FIXTURE_COLLECTION.to_string(),
            index: "ndvi".to_string(),
            month: month.to_string(),
            product: "monthly_composite".to_string(),
        })?,
        Some(field_id),
        Some(FIXTURE_COLLECTION),
        0,
        now - Duration::minutes(1),
        None,
        now,
    )
    .await?;
    Ok(())
}

async fn enqueue_app(
    ctx: &PipelineWorkerContext,
    app_id: &str,
    field_id: &str,
    date: &str,
) -> Result<()> {
    let now = Utc::now();
    pipeline::enqueue_job(
        &ctx.pool,
        JobKind::AppRun,
        &pipeline::app_job_key(app_id, field_id, date),
        &serde_json::to_value(AppRunPayload {
            app_id: app_id.to_string(),
            field_id: field_id.to_string(),
            date: date.to_string(),
        })?,
        Some(field_id),
        None,
        0,
        now - Duration::minutes(1),
        None,
        now,
    )
    .await?;
    Ok(())
}

// --- Tests ---------------------------------------------------------------------

/// A successful derive enqueues the monthly L3 recompute (debounced job key)
/// and the app-run jobs for the index's applicable applications, dated by
/// the L2 product's acquisition (fixture scene: 2023-01-28).
#[tokio::test]
async fn new_l2_enqueues_monthly_composite_and_app_jobs() -> Result<()> {
    let (_tmp, ctx) = worker_ctx().await?;
    let now = Utc::now();
    pipeline::enqueue_job(
        &ctx.pool,
        JobKind::Derive,
        &pipeline::derive_job_key(FIXTURE_COLLECTION, FIXTURE_ITEM_ID, "ndvi", "field-42"),
        &serde_json::to_value(DerivePayload {
            dataset: FIXTURE_COLLECTION.to_string(),
            collection: FIXTURE_COLLECTION.to_string(),
            item_id: FIXTURE_ITEM_ID.to_string(),
            index: "ndvi".to_string(),
            field_id: "field-42".to_string(),
            season_id: Some("season-2026-kharif".to_string()),
            aoi_geojson: aoi_polygon_geojson(),
        })?,
        Some("field-42"),
        Some(FIXTURE_COLLECTION),
        0,
        now - Duration::minutes(1),
        None,
        now,
    )
    .await?;

    let outcome = run_one_tick(&ctx).await?;
    assert_eq!(outcome.kind, Some(JobKind::Derive));
    assert_eq!(outcome.result, Some(JobRunResult::Succeeded));

    // Monthly L3 recompute for the scene's acquisition month.
    let l3_key = pipeline::l3_job_key("field-42", FIXTURE_COLLECTION, "ndvi", "2023-01");
    let l3 = pipeline::find_job_by_key(&ctx.pool, &l3_key)
        .await?
        .expect("l3_recompute job enqueued after successful derive");
    assert_eq!(l3.kind, JobKind::L3Recompute);
    assert_eq!(l3.status, JobStatus::Queued);
    let l3_payload: L3RecomputePayload = serde_json::from_str(&l3.payload_json)?;
    assert_eq!(l3_payload.field_id, "field-42");
    assert_eq!(l3_payload.dataset, FIXTURE_COLLECTION);
    assert_eq!(l3_payload.index, "ndvi");
    assert_eq!(l3_payload.month, "2023-01");

    // App runs for ndvi: crop_health + anomaly_detection, dated by the scene.
    for app_id in ["crop_health", "anomaly_detection"] {
        let key = pipeline::app_job_key(app_id, "field-42", "2023-01-28");
        let job = pipeline::find_job_by_key(&ctx.pool, &key)
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("app_run job for {app_id} enqueued"));
        assert_eq!(job.kind, JobKind::AppRun);
        assert_eq!(job.status, JobStatus::Queued);
        let payload: AppRunPayload = serde_json::from_str(&job.payload_json)?;
        assert_eq!(payload.app_id, app_id);
        assert_eq!(payload.field_id, "field-42");
        assert_eq!(payload.date, "2023-01-28");
    }
    Ok(())
}

/// The monthly composite recompute registers a field-scoped
/// `temporal_composite` L3; a re-run over an extended series registers a new
/// composite and supersedes the previous one in the catalog.
#[tokio::test]
async fn composite_recompute_supersedes_previous() -> Result<()> {
    let (tmp, ctx) = worker_ctx().await?;
    register_field_ndvi(
        &ctx,
        &tmp,
        "field-1",
        "2026-06-01",
        vec![0.2, 0.4, NODATA, 0.6],
    )
    .await?;
    register_field_ndvi(
        &ctx,
        &tmp,
        "field-1",
        "2026-06-11",
        vec![0.4, 0.2, NODATA, 0.8],
    )
    .await?;

    enqueue_l3(&ctx, "field-1", "2026-06").await?;
    let outcome = run_one_tick(&ctx).await?;
    assert_eq!(outcome.kind, Some(JobKind::L3Recompute));
    assert_eq!(outcome.result, Some(JobRunResult::Succeeded), "{outcome:?}");

    let composites =
        composite_rasters::list_composite_products(&ctx.pool, Some("field-1".to_string())).await?;
    assert_eq!(composites.len(), 1, "{composites:?}");
    let first = composites[0].clone();
    assert_eq!(
        first.field_id.as_deref(),
        Some("field-1"),
        "composite carries field scope"
    );
    assert!(first
        .temporal_start
        .as_deref()
        .expect("composite temporal_start")
        .starts_with("2026-06"));

    // A third observation lands in the month: recompute registers a new
    // composite and the previous one is superseded.
    register_field_ndvi(
        &ctx,
        &tmp,
        "field-1",
        "2026-06-21",
        vec![0.6, 0.8, NODATA, 0.7],
    )
    .await?;
    enqueue_l3(&ctx, "field-1", "2026-06").await?;
    // The first composite tick fanned out climatology/drought sibling jobs
    // (S-12), so a single tick may claim one of those instead of the composite
    // recompute. Drain the queue the way the real worker loop does so the
    // recompute actually runs before asserting supersede.
    for _ in 0..64 {
        if !run_one_tick(&ctx).await?.claimed {
            break;
        }
    }

    let registered =
        composite_rasters::list_composite_products(&ctx.pool, Some("field-1".to_string())).await?;
    assert_eq!(
        registered.len(),
        1,
        "exactly one live composite: {registered:?}"
    );
    let second = &registered[0];
    assert_ne!(
        second.product_id, first.product_id,
        "extended series is a new product"
    );

    let old = catalog::get_product(&ctx.pool, &first.product_id)
        .await?
        .expect("previous composite still in catalog");
    assert_eq!(old.status, "superseded");
    assert_eq!(
        old.superseded_by.as_deref(),
        Some(second.product_id.as_str())
    );
    Ok(())
}

/// An app_run job for crop_health assembles the field's NDVI observation
/// series into a governed application run: findings persist with lineage to
/// the input L2 products, and alert evaluation fires the default declining
/// rule on the seeded NDVI drop.
#[tokio::test]
async fn app_run_job_records_findings_with_lineage() -> Result<()> {
    let (tmp, ctx) = worker_ctx().await?;
    // NDVI drops 0.6 -> 0.3 between epochs: trend declining (delta -0.3).
    let first = register_field_ndvi(&ctx, &tmp, "field-1", "2026-06-01", vec![0.6; 4]).await?;
    let second = register_field_ndvi(&ctx, &tmp, "field-1", "2026-06-11", vec![0.3; 4]).await?;
    // The derive hook normally appends these stats; seeding replays it.
    field_timeseries::extract_and_append_field_stats(&ctx.pool, &first).await?;
    field_timeseries::extract_and_append_field_stats(&ctx.pool, &second).await?;

    enqueue_app(&ctx, "crop_health", "field-1", "2026-06-11").await?;
    let outcome = run_one_tick(&ctx).await?;
    assert_eq!(outcome.kind, Some(JobKind::AppRun));
    assert_eq!(outcome.result, Some(JobRunResult::Succeeded), "{outcome:?}");

    // Findings recorded, keyed to the crop_health app.
    let findings = geo_hub::applications::list_field_findings(&ctx.pool, "field-1").await?;
    assert!(!findings.is_empty(), "application findings recorded");
    let declining = findings
        .iter()
        .find(|f| f.finding.kind == "declining_zone")
        .expect("NDVI drop surfaces a declining_zone finding");
    assert_eq!(declining.app_id, "crop_health");
    assert!(declining.finding.evidence_refs.contains(&first));
    assert!(declining.finding.evidence_refs.contains(&second));

    // Lineage traces the finding back to both input L2 products.
    let trace = geo_hub::provenance_store::trace_backward(&ctx.pool, &declining.finding_id).await?;
    assert!(trace.records.iter().any(|r| r.artifact_id == first));
    assert!(trace.records.iter().any(|r| r.artifact_id == second));

    // The alert hook ran: the default ruleset fires on declining_zone.
    let alerts = geo_hub::alert_evaluation::list_field_alerts(&ctx.pool, "field-1").await?;
    let declining_alert = alerts
        .iter()
        .find(|a| a.event_type == "declining_zone")
        .expect("declining alert fired");
    assert_eq!(declining_alert.matched_rule_id, "declining-warning");
    assert_eq!(declining_alert.source_finding_id, declining.finding_id);
    Ok(())
}

/// L3 recompute and app runs over a field with no cataloged inputs complete
/// as logged no-ops, not failures.
#[tokio::test]
async fn l3_with_no_inputs_completes_as_noop() -> Result<()> {
    let (_tmp, ctx) = worker_ctx().await?;

    enqueue_l3(&ctx, "field-empty", "2026-06").await?;
    let outcome = run_one_tick(&ctx).await?;
    assert_eq!(outcome.kind, Some(JobKind::L3Recompute));
    assert_eq!(outcome.result, Some(JobRunResult::Succeeded), "{outcome:?}");
    let composites =
        composite_rasters::list_composite_products(&ctx.pool, Some("field-empty".to_string()))
            .await?;
    assert!(
        composites.is_empty(),
        "no composite registered without inputs"
    );

    enqueue_app(&ctx, "crop_health", "field-empty", "2026-06-11").await?;
    let outcome = run_one_tick(&ctx).await?;
    assert_eq!(outcome.kind, Some(JobKind::AppRun));
    assert_eq!(outcome.result, Some(JobRunResult::Succeeded), "{outcome:?}");
    let findings = geo_hub::applications::list_field_findings(&ctx.pool, "field-empty").await?;
    assert!(findings.is_empty(), "no findings without inputs");

    let jobs = pipeline::list_jobs(&ctx.pool, None).await?;
    assert!(
        jobs.iter().all(|j| j.status == JobStatus::Succeeded),
        "{jobs:?}"
    );
    Ok(())
}
