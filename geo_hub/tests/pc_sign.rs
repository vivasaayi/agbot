//! Planetary Computer SAS signing tests (satellite batch S-10, Task A),
//! fully network-free: a local axum server plays the PC token endpoint
//! (`GET /token/{collection}` -> `{ token, "msft:expiry" }`) and a second
//! local server plays Azure Blob Storage, serving a fixture COG over HTTP
//! range requests but rejecting any request whose query string lacks the
//! SAS token. Covers: token cache fetch-once/reuse, refresh inside the 60 s
//! expiry margin, pure `sign_href` query appending, and end-to-end signed
//! COG range reads through [`SasHttpStore`] / [`PcSignedCogResolver`].

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{Duration, Utc};
use geo_hub::pc_sign::{sign_href, PcSasTokenCache, PcSignedCogResolver, SasHttpStore};
use geo_hub::satellite_derivation::CogStoreResolver;
use raster_io::test_util::{build_tiled_geotiff, FixtureSpec, Pixels};
use raster_io::{RasterBand, RasterWindow, RemoteCogReader};

// --- Fake Planetary Computer token endpoint ----------------------------------

#[derive(Clone)]
struct TokenServer {
    hits: Arc<AtomicUsize>,
    /// Seconds from "now" until the issued token's `msft:expiry`.
    expiry_seconds: i64,
}

async fn token_endpoint(
    State(server): State<TokenServer>,
    AxumPath(collection): AxumPath<String>,
) -> Json<serde_json::Value> {
    let hit = server.hits.fetch_add(1, Ordering::SeqCst) + 1;
    Json(serde_json::json!({
        "token": format!("st=2026-07-06&se=2026-07-07&sig=token-{collection}-{hit}"),
        "msft:expiry": (Utc::now() + Duration::seconds(server.expiry_seconds)).to_rfc3339(),
    }))
}

/// Spawn the token server; returns (base_url, hit counter).
async fn spawn_token_server(expiry_seconds: i64) -> Result<(String, Arc<AtomicUsize>)> {
    let hits = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/token/:collection", get(token_endpoint))
        .with_state(TokenServer {
            hits: hits.clone(),
            expiry_seconds,
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("token server");
    });
    Ok((format!("http://{addr}/token"), hits))
}

// --- Fake SAS-gated blob server -----------------------------------------------

#[derive(Clone)]
struct BlobServer {
    cog: Arc<Vec<u8>>,
    /// Substring the request query must contain (the SAS signature).
    required_query: String,
    unauthorized_hits: Arc<AtomicUsize>,
}

/// Serve `bytes` honoring a `Range: bytes=a-b` header like a real blob store
/// (clamping past-EOF ranges instead of erroring, as HTTP servers do).
fn range_response(bytes: &[u8], range_header: Option<&str>) -> Response {
    let total = bytes.len();
    let spec = range_header.and_then(|header| header.strip_prefix("bytes="));
    let Some(spec) = spec else {
        return Response::builder()
            .status(StatusCode::OK)
            .header("content-length", total.to_string())
            .body(Body::from(bytes.to_vec()))
            .expect("response");
    };
    let (start_text, end_text) = spec.split_once('-').unwrap_or((spec, ""));
    let (start, end) = if start_text.is_empty() {
        // Suffix range `bytes=-N`.
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
    if !uri.query().unwrap_or("").contains(&server.required_query) {
        server.unauthorized_hits.fetch_add(1, Ordering::SeqCst);
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let range = headers
        .get("range")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    range_response(&server.cog, range.as_deref())
}

/// 16x16 u16 fixture COG with pixel value = row-major index.
fn fixture_cog() -> Vec<u8> {
    build_tiled_geotiff(&FixtureSpec {
        width: 16,
        height: 16,
        tile_width: 16,
        tile_height: 16,
        pixels: Pixels::U16((0..256u16).collect()),
        deflate: true,
        tile_gap: 0,
        epsg: 32614,
        geo_transform: [600_000.0, 30.0, 0.0, 4_700_000.0, 0.0, -30.0],
        nodata: Some("0".to_string()),
    })
}

/// Spawn the blob server; returns (base_url, unauthorized-hit counter).
async fn spawn_blob_server(required_query: &str) -> Result<(String, Arc<AtomicUsize>)> {
    let unauthorized_hits = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/landsat-c2/*path", get(blob_endpoint))
        .with_state(BlobServer {
            cog: Arc::new(fixture_cog()),
            required_query: required_query.to_string(),
            unauthorized_hits: unauthorized_hits.clone(),
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("blob server");
    });
    Ok((format!("http://{addr}"), unauthorized_hits))
}

// --- Tests --------------------------------------------------------------------

#[test]
fn sign_href_appends_token_only_for_blob_hosts() {
    let token = "st=2026&se=2027&sig=abc%2F123";
    // No query yet -> `?`.
    assert_eq!(
        sign_href(
            "https://landsateuwest.blob.core.windows.net/landsat-c2/scene/B4.TIF",
            token
        ),
        format!("https://landsateuwest.blob.core.windows.net/landsat-c2/scene/B4.TIF?{token}")
    );
    // Existing query -> `&`.
    assert_eq!(
        sign_href(
            "https://landsateuwest.blob.core.windows.net/landsat-c2/scene/B4.TIF?a=1",
            token
        ),
        format!("https://landsateuwest.blob.core.windows.net/landsat-c2/scene/B4.TIF?a=1&{token}")
    );
    // Non-blob hosts pass through unchanged.
    for href in [
        "https://sentinel-cogs.s3.us-west-2.amazonaws.com/tiles/B04.tif",
        "s3://usgs-landsat/collection02/file.TIF",
        "https://example.com/blob.core.windows.net/trick.tif",
        "not a url",
    ] {
        assert_eq!(sign_href(href, token), href, "{href}");
    }
    // Empty token is a no-op.
    assert_eq!(
        sign_href(
            "https://landsateuwest.blob.core.windows.net/landsat-c2/x.TIF",
            ""
        ),
        "https://landsateuwest.blob.core.windows.net/landsat-c2/x.TIF"
    );
}

#[tokio::test]
async fn token_cache_fetches_once_then_reuses() -> Result<()> {
    let (base_url, hits) = spawn_token_server(3600).await?;
    let cache = PcSasTokenCache::with_base_url(&base_url)?;

    let first = cache.token_for("landsat-c2-l2").await?;
    let second = cache.token_for("landsat-c2-l2").await?;
    assert_eq!(first, second, "fresh token is reused");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "one fetch for two calls");
    assert!(first.contains("sig=token-landsat-c2-l2-1"), "{first}");

    // A different collection is its own cache entry.
    let other = cache.token_for("sentinel-2-l2a").await?;
    assert!(other.contains("sig=token-sentinel-2-l2a-2"), "{other}");
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn token_cache_refreshes_within_expiry_margin() -> Result<()> {
    // Tokens expiring in 30 s are inside the 60 s refresh margin: every call
    // refetches instead of serving the nearly-expired cached token.
    let (base_url, hits) = spawn_token_server(30).await?;
    let cache = PcSasTokenCache::with_base_url(&base_url)?;

    let first = cache.token_for("landsat-c2-l2").await?;
    let second = cache.token_for("landsat-c2-l2").await?;
    assert_ne!(first, second, "near-expiry token is not reused");
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn sas_http_store_reads_cog_with_signed_range_requests() -> Result<()> {
    let (token_base, token_hits) = spawn_token_server(3600).await?;
    // The blob server rejects requests missing the first issued signature.
    let (blob_base, unauthorized_hits) = spawn_blob_server("sig=token-landsat-c2-l2-1").await?;
    let cache = Arc::new(PcSasTokenCache::with_base_url(&token_base)?);
    let store = Arc::new(SasHttpStore::new(&blob_base, cache, "landsat-c2-l2")?);

    let reader = RemoteCogReader::open(store, "landsat-c2/scene/B4.TIF").await?;
    let info = reader.info().clone();
    assert_eq!((info.width, info.height), (16, 16));
    assert_eq!(info.epsg, Some(32614));

    let band = reader
        .read_window(RasterWindow {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        })
        .await?;
    let values = match band {
        RasterBand::U16(values) => values,
        other => panic!("expected u16 band, got {other:?}"),
    };
    assert_eq!(&values[..4], &[0, 1, 2, 3], "first row of the fixture");

    assert_eq!(
        token_hits.load(Ordering::SeqCst),
        1,
        "one token fetch covers every range read"
    );
    assert_eq!(
        unauthorized_hits.load(Ordering::SeqCst),
        0,
        "every blob request carried the SAS query"
    );
    Ok(())
}

#[tokio::test]
async fn resolver_routes_blob_hrefs_to_sas_store_and_delegates_others() -> Result<()> {
    let (token_base, _) = spawn_token_server(3600).await?;
    let cache = Arc::new(PcSasTokenCache::with_base_url(&token_base)?);
    let resolver = PcSignedCogResolver::new(cache, "landsat-c2-l2");

    // Blob hrefs get the SAS store keyed by container/path.
    let (store, location) = resolver
        .resolve("https://landsateuwest.blob.core.windows.net/landsat-c2/oli-tirs/2024/B4.TIF")
        .expect("blob href resolves");
    assert_eq!(location, "landsat-c2/oli-tirs/2024/B4.TIF");
    assert!(
        store
            .to_string()
            .contains("landsateuwest.blob.core.windows.net"),
        "store targets the blob host: {store}"
    );

    // Plain HTTPS hrefs delegate to the URL resolver unchanged.
    let (_, location) = resolver
        .resolve("https://example.com/cogs/B04.tif")
        .expect("plain https href resolves");
    assert_eq!(location, "cogs/B04.tif");
    Ok(())
}
