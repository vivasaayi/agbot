//! Per-field satellite time-series extraction (batch S-3).
//!
//! Covers `geo_hub::field_timeseries::extract_and_append_field_stats`:
//! a registered L2 index raster with field scope is reduced to five zonal
//! statistics (mean, median, p10, p90, valid_fraction) and appended to
//! `time_series_points` under the canonical `shared::timeseries_naming`
//! spellings — plus the derive-path hook that runs the extraction after L2
//! registration.
//!
//! Fixture: a 4x4 GeoTIFF with valid values 1..=14 and two nodata (-9999)
//! pixels, so every expected statistic is hand-computed:
//! mean 7.5, median 7.5 (linear-interpolated rank), p10 2.3, p90 12.7,
//! valid_fraction 14/16 = 0.875.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    extract::Extension,
    http::{Request, StatusCode},
    Router,
};
use geo_hub::field_timeseries::{extract_and_append_field_stats, FieldTimeseriesError};
use geo_hub::satellite_derivation::{CogStoreResolver, DerivationError, SatelliteCogResolver};
use geo_hub::state::AppState;
use geo_hub::utm::{utm_to_wgs84, UtmZone};
use geo_hub::{catalog, db, server, HubConfig};
use raster_io::object_store::memory::InMemory;
use raster_io::object_store::path::Path as ObjectPath;
use raster_io::object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use raster_io::test_util::{build_tiled_geotiff, FixtureSpec, Pixels};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde_json::json;
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use sqlx::Row;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const NODATA: f32 = -9999.0;
const OBSERVED_AT: &str = "2026-01-15T10:00:00Z";

async fn pool(tmp: &TempDir) -> Result<db::DbPool> {
    let db_path = tmp.path().join("field_timeseries.db");
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    db::connect_pool(&config).await
}

/// 4x4 index raster: valid values 1..=14 in row-major order, nodata at
/// flat indices 3 and 12.
fn write_fixture_raster(path: &Path) {
    let mut values = Vec::with_capacity(16);
    let mut next = 1.0f32;
    for index in 0..16 {
        if index == 3 || index == 12 {
            values.push(NODATA);
        } else {
            values.push(next);
            next += 1.0;
        }
    }
    write_geotiff_f32(
        path,
        4,
        4,
        &values,
        &GeoTiffTags {
            epsg: Some(32643),
            geo_transform: Some([600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0]),
            nodata: Some(f64::from(NODATA)),
        },
    )
    .expect("write fixture GeoTIFF");
}

async fn register_l2_product(
    pool: &db::DbPool,
    artifact_path: &Path,
    field_id: Option<&str>,
) -> Result<String> {
    let draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: "ndvi".to_string(),
        algorithm_id: "test.index.ndvi".to_string(),
        algorithm_version: "1.0.0".to_string(),
        // Parameters feed the content-addressed identity: vary by scope so
        // scoped/unscoped registrations are distinct products.
        parameters: json!({ "fixture": "4x4", "field": field_id }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: field_id.map(String::from),
            season_id: Some("season-2026-kharif".to_string()),
            scene_id: Some("S2B_43PFN_20260115_0_L2A".to_string()),
            temporal_start: OBSERVED_AT.to_string(),
            temporal_end: OBSERVED_AT.to_string(),
        },
        spatial_ref: None,
        gsd_m_per_px: Some(10.0),
        artifact: Some(ProductArtifact {
            format: "tif".to_string(),
            path: artifact_path.to_string_lossy().to_string(),
            checksum_sha256: None,
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some("earth-search:sentinel-2-l2a".to_string()),
    };
    Ok(catalog::register_product(pool, &draft, "2026-01-15T12:00:00Z").await?)
}

#[tokio::test]
async fn extraction_appends_five_stats_per_product() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    let raster = tmp.path().join("ndvi.tif");
    write_fixture_raster(&raster);
    let product_id = register_l2_product(&pool, &raster, Some("field-42")).await?;

    let outcome = extract_and_append_field_stats(&pool, &product_id).await?;
    assert_eq!(outcome.product_id, product_id);
    assert_eq!(outcome.points_appended, 5);
    assert_eq!(outcome.points_skipped, 0);

    // Hand-computed over sorted valid values 1..=14 (linear-interpolated
    // percentile rank q * (n - 1)).
    let expected = [
        ("mean", 7.5),
        ("median", 7.5),
        ("p10", 2.3),
        ("p90", 12.7),
        ("valid_fraction", 0.875),
    ];
    for (stat, value) in expected {
        let actual = outcome.stats.get(stat).copied().unwrap_or(f64::NAN);
        assert!(
            (actual - value).abs() < 1e-9,
            "outcome stat {stat}: {actual} != {value}"
        );
    }

    let rows = sqlx::query(
        r#"
        SELECT entity_ref, metric, t, value_kind, scalar_value, source_ref, metadata_json
        FROM time_series_points ORDER BY metric
        "#,
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(rows.len(), 5);
    let expected_metrics = [
        ("sat.ndvi.mean", 7.5),
        ("sat.ndvi.median", 7.5),
        ("sat.ndvi.p10", 2.3),
        ("sat.ndvi.p90", 12.7),
        ("sat.ndvi.valid_fraction", 0.875),
    ];
    for (row, (metric, value)) in rows.iter().zip(expected_metrics) {
        assert_eq!(row.get::<String, _>("entity_ref"), "field:field-42");
        assert_eq!(row.get::<String, _>("metric"), metric);
        assert_eq!(row.get::<String, _>("t"), OBSERVED_AT);
        assert_eq!(row.get::<String, _>("value_kind"), "scalar");
        let actual = row.get::<f64, _>("scalar_value");
        assert!((actual - value).abs() < 1e-9, "{metric}: {actual}");
        assert_eq!(
            row.get::<String, _>("source_ref"),
            format!("product:{product_id}")
        );
        let metadata: serde_json::Value =
            serde_json::from_str(&row.get::<String, _>("metadata_json"))?;
        assert_eq!(metadata["source"], "sentinel2");
        assert_eq!(metadata["scene_id"], "S2B_43PFN_20260115_0_L2A");
        assert_eq!(metadata["level"], "l2");
    }
    Ok(())
}

#[tokio::test]
async fn extraction_is_idempotent_on_rerun() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    let raster = tmp.path().join("ndvi.tif");
    write_fixture_raster(&raster);
    let product_id = register_l2_product(&pool, &raster, Some("field-42")).await?;

    let first = extract_and_append_field_stats(&pool, &product_id).await?;
    assert_eq!((first.points_appended, first.points_skipped), (5, 0));

    let second = extract_and_append_field_stats(&pool, &product_id).await?;
    assert_eq!((second.points_appended, second.points_skipped), (0, 5));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM time_series_points")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 5, "rerun must not duplicate points");
    Ok(())
}

#[tokio::test]
async fn extraction_skips_product_without_field_scope() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    let raster = tmp.path().join("ndvi.tif");
    write_fixture_raster(&raster);
    let product_id = register_l2_product(&pool, &raster, None).await?;

    let err = extract_and_append_field_stats(&pool, &product_id)
        .await
        .expect_err("unscoped product must be a reason-coded skip");
    assert!(
        matches!(err, FieldTimeseriesError::NoFieldScope(ref id) if id == &product_id),
        "unexpected error: {err}"
    );

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM time_series_points")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 0);
    Ok(())
}

// --- Derive-path hook ---------------------------------------------------------
//
// Fixture machinery mirrors `tests/satellite_derive.rs`: a captured Earth
// Search item re-pointed at in-memory fixture COGs, driven through
// `POST /api/satellite/derive` with a field scope.

const EPSG: u16 = 32643;
const TRANSFORM_10M: [f64; 6] = [600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];
const TRANSFORM_20M: [f64; 6] = [600_000.0, 20.0, 0.0, 1_300_020.0, 0.0, -20.0];
/// NDVI of red DN 2000, nir DN 6000 under baseline >= 04.00.
const EXPECTED_NDVI: f64 = 0.4 / 0.6;

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
    nir_pixels[10 * 32 + 10] = 0; // fill DN inside the window
    let nir = band(Pixels::U16(nir_pixels), TRANSFORM_10M, 32);
    let mut scl_pixels = vec![4u8; 16 * 16];
    scl_pixels[7 * 16 + 7] = 9; // high-probability cloud inside the window
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

fn fixture_item() -> serde_json::Value {
    let mut item: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/earth_search_s2_item.json"))
            .expect("parse captured item");
    for name in ["red", "nir", "scl"] {
        item["assets"][name]["href"] = json!(format!("https://cogs.test/fixtures/{name}.tif"));
    }
    item
}

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

async fn derive_ctx(tmp: &TempDir) -> Result<(Router, db::DbPool)> {
    let db_path = tmp.path().join("derive_hook.db");
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let state = AppState {
        pool: pool.clone(),
        config: Arc::new(config),
        scene_search_cache: Default::default(),
    };
    let resolver = SatelliteCogResolver(Arc::new(MemResolver(fixture_store().await)));
    Ok((server::build_router(state).layer(Extension(resolver)), pool))
}

#[tokio::test]
async fn derive_appends_field_stats() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = derive_ctx(&tmp).await?;

    let body = json!({
        "item": fixture_item(),
        "aoi": aoi(),
        "index": "ndvi",
        "field_id": "field-42",
        "season_id": "season-2026-kharif",
    });
    let request = Request::builder()
        .method("POST")
        .uri("/api/satellite/derive")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body)?))?;
    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024).await?;
    let response: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(status, StatusCode::OK, "{response}");
    let product_id = response["product_id"].as_str().unwrap();

    // The derive hook appended one point per zonal stat for the field. All
    // 63 valid pixels are the same calibrated NDVI, so every value stat is
    // EXPECTED_NDVI and valid_fraction is 63/100.
    let rows = sqlx::query(
        r#"
        SELECT metric, scalar_value, source_ref
        FROM time_series_points WHERE entity_ref = 'field:field-42' ORDER BY metric
        "#,
    )
    .fetch_all(&pool)
    .await?;
    let metrics: Vec<String> = rows.iter().map(|row| row.get("metric")).collect();
    assert_eq!(
        metrics,
        vec![
            "sat.ndvi.mean",
            "sat.ndvi.median",
            "sat.ndvi.p10",
            "sat.ndvi.p90",
            "sat.ndvi.valid_fraction",
        ],
        "derive hook must append all five stats"
    );
    for row in &rows {
        assert_eq!(
            row.get::<String, _>("source_ref"),
            format!("product:{product_id}")
        );
        let metric: String = row.get("metric");
        let value: f64 = row.get("scalar_value");
        let expected = if metric == "sat.ndvi.valid_fraction" {
            0.63
        } else {
            EXPECTED_NDVI
        };
        assert!(
            (value - expected).abs() < 1e-6,
            "{metric}: {value} != {expected}"
        );
    }
    Ok(())
}
