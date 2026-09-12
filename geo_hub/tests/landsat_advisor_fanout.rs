use anyhow::Result;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use geo_hub::config::PipelineConfig;
use geo_hub::earth_search::EarthSearchItem;
use geo_hub::pipeline_worker::{
    run_one_tick, BoxFuture, ItemFetchError, PipelineWorkerContext, StacItemFetcher,
};
use geo_hub::satellite_derivation::{CogStoreResolver, DerivationError};
use geo_hub::state::AppState;
use geo_hub::{catalog, db, pipeline, server, HubConfig};
use raster_io::{write_geotiff_u16, GeoTiffTags};
use serde_json::json;
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

struct UnusedResolver;
impl CogStoreResolver for UnusedResolver {
    fn resolve(
        &self,
        _: &str,
    ) -> Result<(Arc<dyn raster_io::object_store::ObjectStore>, String), DerivationError> {
        unreachable!("advisor jobs do not resolve provider COGs")
    }
}
struct UnusedFetcher;
impl StacItemFetcher for UnusedFetcher {
    fn fetch_item<'a>(
        &'a self,
        collection: &'a str,
        item_id: &'a str,
    ) -> BoxFuture<'a, std::result::Result<EarthSearchItem, ItemFetchError>> {
        Box::pin(async move {
            Err(ItemFetchError::NotFound {
                collection: collection.into(),
                item_id: item_id.into(),
            })
        })
    }
}

async fn context(tmp: &TempDir) -> Result<PipelineWorkerContext> {
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", tmp.path().join("hub.db").display()),
        data_root: tmp.path().join("data"),
        pipeline: PipelineConfig {
            qa_min_confidence: 0.5,
            provider_min_delay_ms: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    Ok(PipelineWorkerContext {
        pool,
        config: Arc::new(config),
        cog_resolver: Arc::new(UnusedResolver),
        item_fetcher: Arc::new(UnusedFetcher),
    })
}

fn app(ctx: &PipelineWorkerContext) -> Router {
    server::build_router(AppState {
        pool: ctx.pool.clone(),
        config: ctx.config.clone(),
        scene_search_cache: Default::default(),
    })
}

async fn band(
    ctx: &PipelineWorkerContext,
    tmp: &TempDir,
    scene: &str,
    kind: &str,
    pixels: Vec<u16>,
) -> Result<()> {
    let path = tmp.path().join(format!("{scene}-{kind}.tif"));
    write_geotiff_u16(
        &path,
        2,
        2,
        &pixels,
        &GeoTiffTags {
            epsg: Some(32614),
            geo_transform: Some([600_000.0, 30.0, 0.0, 4_700_000.0, 0.0, -30.0]),
            nodata: None,
        },
    )?;
    catalog::register_product(
        &ctx.pool,
        &ProductRecordDraft {
            level: ProductLevel::L1,
            kind: kind.into(),
            algorithm_id: "usgs.landsat.surface_reflectance".into(),
            algorithm_version: "1.0.0".into(),
            parameters: json!({"scene_id": scene, "band": kind}),
            inputs: vec![],
            scope: ProductScope {
                farm_id: None,
                field_id: None,
                season_id: None,
                scene_id: Some(scene.into()),
                temporal_start: "2026-06-01T00:00:00Z".into(),
                temporal_end: "2026-06-01T00:00:00Z".into(),
            },
            spatial_ref: None,
            gsd_m_per_px: Some(30.0),
            artifact: Some(ProductArtifact {
                format: "tif".into(),
                path: path.to_string_lossy().into(),
                checksum_sha256: None,
            }),
            quality_mask: None,
            confidence: None,
            confidence_method: None,
            quality_summary: None,
            evidence_digests: vec![],
            source_id: Some("usgs:landsat_ot_c2_l2".into()),
        },
        "2026-06-01T00:00:00Z",
    )
    .await?;
    Ok(())
}

async fn inputs(
    ctx: &PipelineWorkerContext,
    tmp: &TempDir,
    scene: &str,
    qa: Vec<u16>,
) -> Result<()> {
    band(ctx, tmp, scene, "band_sr_b4", vec![14_545; 4]).await?;
    band(ctx, tmp, scene, "band_sr_b5", vec![29_091; 4]).await?;
    band(ctx, tmp, scene, "band_qa_pixel", qa).await?;
    band(ctx, tmp, scene, "band_qa_radsat", vec![0; 4]).await?;
    band(ctx, tmp, scene, "band_sr_qa_aerosol", vec![0; 4]).await?;
    Ok(())
}

async fn derive(app: &Router, scene: &str, field: &str) -> Result<StatusCode> {
    Ok(app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ingest/landsat/derive")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"scene_id": scene, "field_id": field, "season_id": "2026"}).to_string(),
                ))?,
        )
        .await?
        .status())
}

#[tokio::test]
async fn landsat_http_route_fans_accepted_ndvi_into_findings_but_not_failed_qa() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = context(&tmp).await?;
    let accepted = "LC08_L2SP_029031_20260601_02_T1";
    inputs(&ctx, &tmp, accepted, vec![64; 4]).await?;
    assert_eq!(
        derive(&app(&ctx), accepted, "field-ok").await?,
        StatusCode::OK
    );
    assert_eq!(
        pipeline::list_jobs(&ctx.pool, Some("field-ok"))
            .await?
            .len(),
        3
    );
    for _ in 0..3 {
        assert!(run_one_tick(&ctx).await?.claimed);
    }
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM application_findings WHERE field_id = 'field-ok'")
            .fetch_one(&ctx.pool)
            .await?;
    assert!(
        count > 0,
        "accepted Landsat NDVI must reach advisor findings"
    );

    let failed = "LC08_L2SP_029031_20260602_02_T1";
    inputs(&ctx, &tmp, failed, vec![8; 4]).await?;
    assert_eq!(
        derive(&app(&ctx), failed, "field-quarantine").await?,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert!(pipeline::list_jobs(&ctx.pool, Some("field-quarantine"))
        .await?
        .is_empty());
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM application_findings WHERE field_id = 'field-quarantine'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(count, 0);
    Ok(())
}
