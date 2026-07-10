//! Integration tests for the farmer PWA shell served at `/portal`
//! (batch F-B6). Mirrors the `/workspace` static-serving patterns in
//! `tests/workspace_static.rs`: content-type checks over the live router
//! plus route-manifest discipline for the portal API client.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    Router,
};
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

async fn test_router(tmp: &TempDir) -> Result<Router> {
    let db_path = tmp.path().join("geo_hub_test.db");
    let config = HubConfig {
        bind_address: "127.0.0.1:0".to_string(),
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };

    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;

    let state = AppState {
        pool,
        config: Arc::new(config),
        scene_search_cache: Default::default(),
    };

    Ok(server::build_router(state))
}

async fn get(app: Router, uri: &str) -> Result<axum::response::Response> {
    Ok(app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())?,
        )
        .await?)
}

fn content_type(response: &axum::response::Response) -> String {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn portal_index_is_served_as_html() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = test_router(&tmp).await?;

    let response = get(app, "/portal/").await?;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        content_type(&response).starts_with("text/html"),
        "expected text/html content type, got {}",
        content_type(&response)
    );

    let body = to_bytes(response.into_body(), 1024 * 1024).await?;
    let body = String::from_utf8(body.to_vec())?;
    assert!(
        body.contains("AGBot Farm"),
        "index.html should contain the portal app name"
    );
    assert!(
        body.contains("manifest.json"),
        "index.html should link the web app manifest"
    );

    Ok(())
}

#[tokio::test]
async fn portal_manifest_and_service_worker_are_served() -> Result<()> {
    let tmp = TempDir::new()?;

    let app = test_router(&tmp).await?;
    let response = get(app, "/portal/manifest.json").await?;
    assert_eq!(response.status(), StatusCode::OK);
    let manifest_type = content_type(&response);
    assert!(
        manifest_type.contains("json"),
        "expected a JSON content type for the manifest \
         (application/json or application/manifest+json), got {manifest_type}"
    );
    let body = to_bytes(response.into_body(), 1024 * 1024).await?;
    let manifest: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(manifest["name"], "AGBot Farm");
    assert_eq!(manifest["start_url"], "/portal/");
    assert_eq!(manifest["display"], "standalone");

    let app = test_router(&tmp).await?;
    let response = get(app, "/portal/sw.js").await?;
    assert_eq!(response.status(), StatusCode::OK);
    let sw_type = content_type(&response);
    assert!(
        sw_type.contains("javascript"),
        "expected a javascript content type for the service worker, got {sw_type}"
    );

    Ok(())
}

#[tokio::test]
async fn portal_icons_served_as_png() -> Result<()> {
    let tmp = TempDir::new()?;

    for icon in ["/portal/icons/icon-192.png", "/portal/icons/icon-512.png"] {
        let app = test_router(&tmp).await?;
        let response = get(app, icon).await?;
        assert_eq!(response.status(), StatusCode::OK, "{icon} must be served");
        let icon_type = content_type(&response);
        assert!(
            icon_type.contains("image/png"),
            "expected image/png for {icon}, got {icon_type}"
        );
        let body = to_bytes(response.into_body(), 4 * 1024 * 1024).await?;
        assert!(
            body.starts_with(&[0x89, b'P', b'N', b'G']),
            "{icon} must be a real PNG file (magic bytes missing)"
        );
    }

    Ok(())
}

/// Extract every string literal starting with "/api/" from a source file.
fn extract_api_literals(source: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("\"/api/") {
        let after_quote = &rest[start + 1..];
        match after_quote.find('"') {
            Some(end) => {
                let literal = &after_quote[..end];
                if !literals.iter().any(|existing| existing == literal) {
                    literals.push(literal.to_string());
                }
                rest = &after_quote[end + 1..];
            }
            None => break,
        }
    }
    literals
}

/// True when a client-side path matches a registered axum route path.
/// Route `:param` segments match any client segment; client `${...}`
/// interpolation segments match any route segment.
fn path_matches_route(route: &str, client_path: &str) -> bool {
    let route_segments: Vec<&str> = route.split('/').filter(|s| !s.is_empty()).collect();
    let client_segments: Vec<&str> = client_path.split('/').filter(|s| !s.is_empty()).collect();
    if route_segments.len() != client_segments.len() {
        return false;
    }
    route_segments
        .iter()
        .zip(client_segments.iter())
        .all(|(route_seg, client_seg)| {
            route_seg.starts_with(':') || client_seg.contains("${") || route_seg == client_seg
        })
}

/// Route-manifest test: every "/api/..." literal referenced by the portal
/// front-end (web/portal/js/api.js) must correspond to a route registered in
/// the geo_hub router. The registered route list is extracted from the route
/// path literals in src/server.rs (the single place routes are declared), so
/// the manifest cannot drift from the real router. Parameterless endpoints
/// are additionally exercised against the live router to prove they are
/// routable: an unregistered path hits the 404 fallback, while registered
/// portal endpoints answer 401 (auth required) or 405 (POST-only).
#[tokio::test]
async fn portal_api_js_only_references_registered_routes() -> Result<()> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let api_js = std::fs::read_to_string(manifest_dir.join("web/portal/js/api.js"))?;
    let client_paths = extract_api_literals(&api_js);
    assert!(
        !client_paths.is_empty(),
        "web/portal/js/api.js should declare at least one /api/ endpoint"
    );

    let server_source = std::fs::read_to_string(manifest_dir.join("src/server.rs"))?;
    let registered_routes = extract_api_literals(&server_source);
    assert!(
        !registered_routes.is_empty(),
        "src/server.rs should register /api/ routes"
    );

    for client_path in &client_paths {
        assert!(
            registered_routes
                .iter()
                .any(|route| path_matches_route(route, client_path)),
            "web/portal/js/api.js references `{client_path}`, which does not match any \
             route registered in src/server.rs"
        );
    }

    // Prove the parameterless endpoints are actually routable, not just
    // present as literals: an unregistered path would hit the 404 fallback.
    let tmp = TempDir::new()?;
    for client_path in client_paths
        .iter()
        .filter(|path| !path.contains(':') && !path.contains("${"))
    {
        let app = test_router(&tmp).await?;
        let response = get(app, client_path).await?;
        assert_ne!(
            response.status(),
            StatusCode::NOT_FOUND,
            "`{client_path}` is referenced by portal api.js but the router returned 404"
        );
    }

    Ok(())
}

/// Recursively collect `.js` files under `dir`.
fn collect_js_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_js_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("js") {
            out.push(path);
        }
    }
}

/// Single-URL-file convention: only `web/portal/js/api.js` may contain
/// `/api/` string literals. Every other portal module must route requests
/// through the api.js client, so backend URLs cannot drift across the
/// front-end. (The service worker at web/portal/sw.js is outside js/ and
/// legitimately prefix-matches "/api/" for its network-first rule.)
#[test]
fn only_portal_api_js_contains_backend_url_literals() {
    let js_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("web/portal/js");
    let mut files = Vec::new();
    collect_js_files(&js_root, &mut files);
    assert!(!files.is_empty(), "expected web/portal/js/*.js files");

    for file in files {
        if file.file_name().and_then(|n| n.to_str()) == Some("api.js") {
            continue;
        }
        let source = std::fs::read_to_string(&file).unwrap();
        assert!(
            !source.contains("/api/"),
            "{} contains a `/api/` literal; route all requests through api.js",
            file.display()
        );
    }
}
