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
