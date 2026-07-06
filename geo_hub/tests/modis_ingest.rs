//! MODIS MOD13Q1 v061 NDVI ingest tests (batch S-14), fully network-free.
//!
//! Three local axum servers stand in for the external boundaries:
//! - a **STAC search** server returning a fixture MOD13Q1 FeatureCollection
//!   (`tests/fixtures/pc_modis_13q1_search.json`) with the NDVI asset href
//!   rewritten to the local blob server;
//! - a **PC SAS token** server (`GET /token/:collection`), mirroring
//!   `tests/pc_sign.rs`;
//! - a **SAS-gated blob** server serving a fixture NDVI COG over HTTP range
//!   requests, rejecting any request whose query lacks the SAS token.
//!
//! Covers: an item registering as an external L3 product with the expected
//! kind/level/scope/source_id, five `sat.ndvi.*` points appended with source
//! `"modis"` and 0.0001 scaling with the fill masked, idempotency on re-run,
//! the route's date validation, and the S-4 timeseries API surfacing `"modis"`
//! as a per-source key.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use axum::body::{to_bytes, Body};
use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderMap, Request, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Duration, Utc};
use geo_hub::modis::{ingest_modis_ndvi_with_resolver, MODIS_13Q1_COLLECTION};
use geo_hub::pc_sign::{PcSasTokenCache, SasHttpStore};
use geo_hub::satellite_derivation::{CogStoreResolver, DerivationError, UrlCogResolver};
use geo_hub::state::AppState;
use raster_io::object_store::{path::Path as ObjectPath, ObjectStore};
use geo_hub::{db, server, HubConfig};
use raster_io::test_util::{build_tiled_geotiff, FixtureSpec, Pixels};
use serde_json::json;
use tempfile::TempDir;
use tower::util::ServiceExt;

const FIELD: &str = "field-modis-1";
const REQUIRED_SIG: &str = "sig=token-modis-13Q1-061-1";

// --- Fixture NDVI COG ---------------------------------------------------------

/// 4x4 u16 NDVI COG. DNs scale by 0.0001: 2000->0.2 .. and one fill pixel
/// (65533, tagged as GDAL_NODATA) stands in for the -3000 MODIS fill (the
/// fixture generator is unsigned, so the module masks the nodata sentinel).
fn fixture_ndvi_cog() -> Vec<u8> {
    // 15 valid DNs (2000..=9000 stepping 500) + one nodata sentinel.
    let mut dns: Vec<u16> = (0..15).map(|i| 2000 + i * 500).collect();
    dns.push(65533); // nodata / fill
    build_tiled_geotiff(&FixtureSpec {
        width: 4,
        height: 4,
        tile_width: 4,
        tile_height: 4,
        pixels: Pixels::U16(dns),
        deflate: true,
        tile_gap: 0,
        epsg: 32662, // plate carree; the module does not enforce a CRS for modis
        geo_transform: [10.0, 0.0001, 0.0, 0.1, 0.0, -0.0001],
        nodata: Some("65533".to_string()),
    })
}

/// Mean of the 15 valid DNs (2000..=9000 step 500) scaled by 0.0001.
fn expected_mean_ndvi() -> f64 {
    let valid: Vec<f64> = (0..15)
        .map(|i| f64::from(2000 + i * 500) * 0.0001)
        .collect();
    valid.iter().sum::<f64>() / valid.len() as f64
}

// --- STAC search server -------------------------------------------------------

#[derive(Clone)]
struct StacServer {
    body: Arc<serde_json::Value>,
}

async fn stac_search(State(server): State<StacServer>) -> Json<serde_json::Value> {
    Json((*server.body).clone())
}

/// Spawn the STAC server returning the fixture collection with the NDVI href
/// pointed at `blob_base`. Returns the search endpoint URL.
async fn spawn_stac_server(blob_base: &str) -> Result<String> {
    let raw = include_str!("fixtures/pc_modis_13q1_search.json");
    let ndvi_href = format!("{blob_base}/modis-13Q1-061/MOD13Q1_NDVI.tif");
    let evi_href = format!("{blob_base}/modis-13Q1-061/MOD13Q1_EVI.tif");
    let substituted = raw
        .replace("__NDVI_HREF__", &ndvi_href)
        .replace("__EVI_HREF__", &evi_href);
    let body: serde_json::Value = serde_json::from_str(&substituted)?;
    let app = Router::new()
        .route("/search", post(stac_search))
        .with_state(StacServer {
            body: Arc::new(body),
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("stac server");
    });
    Ok(format!("http://{addr}/search"))
}

// --- SAS token server ---------------------------------------------------------

async fn token_endpoint(AxumPath(collection): AxumPath<String>) -> Json<serde_json::Value> {
    Json(json!({
        "token": format!("st=2026-07-06&se=2026-07-07&sig=token-{collection}-1"),
        "msft:expiry": (Utc::now() + Duration::seconds(3600)).to_rfc3339(),
    }))
}

async fn spawn_token_server() -> Result<String> {
    let app = Router::new().route("/token/:collection", get(token_endpoint));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("token server");
    });
    Ok(format!("http://{addr}/token"))
}

// --- SAS-gated blob server ----------------------------------------------------

#[derive(Clone)]
struct BlobServer {
    cog: Arc<Vec<u8>>,
    unauthorized_hits: Arc<AtomicUsize>,
}

fn range_response(bytes: &[u8], range_header: Option<&str>) -> Response {
    let total = bytes.len();
    let spec = range_header.and_then(|h| h.strip_prefix("bytes="));
    let Some(spec) = spec else {
        return Response::builder()
            .status(StatusCode::OK)
            .header("content-length", total.to_string())
            .body(Body::from(bytes.to_vec()))
            .expect("response");
    };
    let (start_text, end_text) = spec.split_once('-').unwrap_or((spec, ""));
    let (start, end) = if start_text.is_empty() {
        let n: usize = end_text.parse().unwrap_or(0);
        (total.saturating_sub(n), total.saturating_sub(1))
    } else {
        let start: usize = start_text.parse().unwrap_or(0);
        let end: usize = if end_text.is_empty() {
            total.saturating_sub(1)
        } else {
            end_text.parse().unwrap_or(total - 1)
        };
        (start, end.min(total.saturating_sub(1)))
    };
    if start > end || start >= total {
        return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    }
    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header("content-range", format!("bytes {start}-{end}/{total}"))
        .header("content-length", (end - start + 1).to_string())
        .body(Body::from(bytes[start..=end].to_vec()))
        .expect("response")
}

async fn blob_endpoint(State(server): State<BlobServer>, uri: Uri, headers: HeaderMap) -> Response {
    if !uri.query().unwrap_or("").contains(REQUIRED_SIG) {
        server.unauthorized_hits.fetch_add(1, Ordering::SeqCst);
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let range = headers
        .get("range")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    range_response(&server.cog, range.as_deref())
}

async fn spawn_blob_server() -> Result<(String, Arc<AtomicUsize>)> {
    let unauthorized_hits = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/modis-13Q1-061/*path", get(blob_endpoint))
        .with_state(BlobServer {
            cog: Arc::new(fixture_ndvi_cog()),
            unauthorized_hits: unauthorized_hits.clone(),
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("blob server");
    });
    Ok((format!("http://{addr}"), unauthorized_hits))
}

// --- Test resolver ------------------------------------------------------------

/// Routes the localhost blob href through a SAS-signed [`SasHttpStore`] (the
/// production `PcSignedCogResolver` gates on `*.blob.core.windows.net`, so a
/// `127.0.0.1` href would otherwise fall through to the plain URL store). Every
/// other href delegates to the plain resolver.
struct LocalSasResolver {
    blob_base: String,
    cache: Arc<PcSasTokenCache>,
}

impl CogStoreResolver for LocalSasResolver {
    fn resolve(&self, href: &str) -> Result<(Arc<dyn ObjectStore>, String), DerivationError> {
        let url = url::Url::parse(href).map_err(|err| DerivationError::Resolve {
            href: href.to_string(),
            message: err.to_string(),
        })?;
        let base = format!(
            "{}://{}",
            url.scheme(),
            url.host_str().unwrap_or_default()
        );
        let base = match url.port() {
            Some(port) => format!("{base}:{port}"),
            None => base,
        };
        if base != self.blob_base {
            return UrlCogResolver.resolve(href);
        }
        let location = ObjectPath::from_url_path(url.path().trim_start_matches('/'))
            .map_err(|err| DerivationError::Resolve {
                href: href.to_string(),
                message: err.to_string(),
            })?;
        let store = SasHttpStore::new(&self.blob_base, self.cache.clone(), MODIS_13Q1_COLLECTION)
            .map_err(|err| DerivationError::Resolve {
                href: href.to_string(),
                message: err.to_string(),
            })?;
        Ok((Arc::new(store), location.to_string()))
    }
}

// --- Harness ------------------------------------------------------------------

async fn ctx(tmp: &TempDir) -> Result<(AppState, db::DbPool)> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("modis_ingest.db").display()
        ),
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
    Ok((state, pool))
}

async fn seed_field(pool: &db::DbPool) -> Result<()> {
    // Boundary overlapping the fixture item bbox [9.99,-0.01,10.11,0.11].
    let boundary = json!({
        "type": "Polygon",
        "coordinates": [[
            [10.0, 0.0], [10.1, 0.0], [10.1, 0.1], [10.0, 0.1], [10.0, 0.0]
        ]],
    });
    sqlx::query(
        "INSERT INTO fields (field_id, name, boundary_json, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(FIELD)
    .bind("MODIS Field")
    .bind(boundary.to_string())
    .bind("2026-01-01T00:00:00Z")
    .execute(pool)
    .await?;
    Ok(())
}

/// Spin up all three servers and return (state, pool, stac_url, resolver,
/// unauthorized-hit counter).
async fn full_ctx(
    tmp: &TempDir,
) -> Result<(
    AppState,
    db::DbPool,
    String,
    LocalSasResolver,
    Arc<AtomicUsize>,
)> {
    let (state, pool) = ctx(tmp).await?;
    seed_field(&pool).await?;
    let (blob_base, unauthorized) = spawn_blob_server().await?;
    let stac_url = spawn_stac_server(&blob_base).await?;
    let token_base = spawn_token_server().await?;
    let cache = Arc::new(PcSasTokenCache::with_base_url(&token_base)?);
    let resolver = LocalSasResolver { blob_base, cache };
    Ok((state, pool, stac_url, resolver, unauthorized))
}

// --- Tests --------------------------------------------------------------------

#[tokio::test]
async fn modis_item_registers_as_external_l3_and_appends_points() -> Result<()> {
    let tmp = TempDir::new()?;
    let (state, pool, stac_url, resolver, unauthorized) = full_ctx(&tmp).await?;

    let outcome = ingest_modis_ndvi_with_resolver(
        &pool,
        &resolver,
        FIELD,
        "2026-01-01",
        "2026-01-31",
        &stac_url,
    )
    .await?;

    assert_eq!(outcome.items_found, 1);
    assert_eq!(outcome.products_registered, 1);
    assert_eq!(outcome.points_appended, 5, "five zonal stats appended");
    assert_eq!(outcome.points_skipped, 0);
    assert_eq!(
        unauthorized.load(Ordering::SeqCst),
        0,
        "every blob read carried the SAS token"
    );

    // External L3 product: kind/level/scope/source_id.
    let row = sqlx::query(
        "SELECT product_id, level, kind, source_id, field_id, path FROM catalog_products \
         WHERE kind = 'modis_ndvi'",
    )
    .fetch_one(&pool)
    .await?;
    use sqlx::Row;
    assert_eq!(row.get::<String, _>("level"), "l3");
    assert_eq!(row.get::<String, _>("kind"), "modis_ndvi");
    assert_eq!(
        row.get::<String, _>("source_id"),
        "planetary-computer:modis-13Q1-061"
    );
    assert_eq!(row.get::<String, _>("field_id"), FIELD);
    // Path is the unsigned remote NDVI href (external reference, no pixels).
    let path: String = row.get("path");
    assert!(
        path.ends_with("MOD13Q1_NDVI.tif"),
        "external href path: {path}"
    );
    assert!(!path.contains("sig="), "stored href stays unsigned: {path}");

    // Five sat.ndvi.* points, source "modis", 0.0001 scaling with fill masked.
    let points = sqlx::query(
        "SELECT metric, scalar_value, metadata_json FROM time_series_points \
         WHERE entity_ref = ? AND metric LIKE 'sat.ndvi.%' ORDER BY metric",
    )
    .bind(format!("field:{FIELD}"))
    .fetch_all(&pool)
    .await?;
    assert_eq!(points.len(), 5);
    let metrics: Vec<String> = points
        .iter()
        .map(|r| r.get::<String, _>("metric"))
        .collect();
    assert_eq!(
        metrics,
        vec![
            "sat.ndvi.mean",
            "sat.ndvi.median",
            "sat.ndvi.p10",
            "sat.ndvi.p90",
            "sat.ndvi.valid_fraction",
        ]
    );
    for row in &points {
        let metadata: serde_json::Value =
            serde_json::from_str(&row.get::<String, _>("metadata_json"))?;
        assert_eq!(metadata["source"], "modis");
    }
    let mean = points
        .iter()
        .find(|r| r.get::<String, _>("metric") == "sat.ndvi.mean")
        .map(|r| r.get::<f64, _>("scalar_value"))
        .unwrap();
    assert!(
        (mean - expected_mean_ndvi()).abs() < 1e-6,
        "mean {mean} matches scaled valid DNs (fill masked)"
    );
    let valid_fraction = points
        .iter()
        .find(|r| r.get::<String, _>("metric") == "sat.ndvi.valid_fraction")
        .map(|r| r.get::<f64, _>("scalar_value"))
        .unwrap();
    assert!(
        (valid_fraction - 15.0 / 16.0).abs() < 1e-6,
        "15 valid of 16 pixels (one fill masked)"
    );

    drop(state);
    Ok(())
}

#[tokio::test]
async fn modis_ingest_is_idempotent() -> Result<()> {
    let tmp = TempDir::new()?;
    let (_state, pool, stac_url, resolver, _unauthorized) = full_ctx(&tmp).await?;

    let first = ingest_modis_ndvi_with_resolver(
        &pool,
        &resolver,
        FIELD,
        "2026-01-01",
        "2026-01-31",
        &stac_url,
    )
    .await?;
    assert_eq!(first.points_appended, 5);

    let second = ingest_modis_ndvi_with_resolver(
        &pool,
        &resolver,
        FIELD,
        "2026-01-01",
        "2026-01-31",
        &stac_url,
    )
    .await?;
    assert_eq!(second.points_appended, 0, "re-run appends nothing");
    assert_eq!(second.points_skipped, 5, "all five points already present");
    assert_eq!(second.products_registered, 1, "registration deduped");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM time_series_points WHERE metric LIKE 'sat.ndvi.%'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(count, 5, "no duplicate rows after re-run");
    Ok(())
}

#[tokio::test]
async fn modis_ingest_route_validates_dates() -> Result<()> {
    let tmp = TempDir::new()?;
    let (state, pool) = ctx(&tmp).await?;
    seed_field(&pool).await?;
    let app = server::build_router(state);

    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/fields/{FIELD}/modis/ingest"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "start": "not-a-date", "end": "2026-01-31" }).to_string(),
        ))?;
    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("invalid date range"), "body: {text}");
    Ok(())
}

#[tokio::test]
async fn modis_appears_as_source_in_timeseries_api() -> Result<()> {
    let tmp = TempDir::new()?;
    let (state, pool, stac_url, resolver, _unauthorized) = full_ctx(&tmp).await?;

    ingest_modis_ndvi_with_resolver(
        &pool,
        &resolver,
        FIELD,
        "2026-01-01",
        "2026-01-31",
        &stac_url,
    )
    .await?;

    let app = server::build_router(state);
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/fields/{FIELD}/timeseries?metric=sat.ndvi.mean"
        ))
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await?;
    let value: serde_json::Value = serde_json::from_slice(&body)?;
    assert!(
        value["per_source"].get("modis").is_some(),
        "modis is a per_source key: {}",
        value["per_source"]
    );
    let modis_points = value["per_source"]["modis"].as_array().unwrap();
    assert_eq!(modis_points.len(), 1, "one mean observation for the window");
    assert_eq!(modis_points[0]["source"], "modis");
    Ok(())
}
