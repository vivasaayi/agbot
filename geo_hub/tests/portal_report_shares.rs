use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    Router,
};
use chrono::{Duration, SecondsFormat, Utc};
use geo_hub::portal_auth::hash_token;
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

async fn setup(tmp: &TempDir) -> Result<(Router, db::DbPool)> {
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", tmp.path().join("hub.db").display()),
        data_root: tmp.path().join("data"),
        ..Default::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let app = server::build_router(AppState {
        pool: pool.clone(),
        config: Arc::new(config),
        scene_search_cache: Default::default(),
    });
    Ok((app, pool))
}

async fn identity(pool: &db::DbPool, account: &str, org: &str) -> Result<String> {
    sqlx::query("INSERT INTO marketplace_accounts (account_id, org_id, party_type, role_refs_json, status, created_at, updated_at) VALUES (?1, ?2, 'farmer', '[]', 'active', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')")
        .bind(account).bind(org).execute(pool).await?;
    let token = format!("token-{account}");
    let expiry = (Utc::now() + Duration::days(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    sqlx::query("INSERT INTO portal_sessions (session_id, token_hash, account_id, org_id, created_at, expires_at, last_seen_at) VALUES (?1, ?2, ?3, ?4, '2026-01-01T00:00:00Z', ?5, '2026-01-01T00:00:00Z')")
        .bind(format!("session-{account}")).bind(hash_token(&token)).bind(account).bind(org).bind(expiry).execute(pool).await?;
    Ok(token)
}

async fn report(pool: &db::DbPool, tmp: &TempDir, suffix: &str, org: &str) -> Result<()> {
    let field = format!("field-{suffix}");
    let scene = format!("scene-{suffix}");
    let report = format!("report-{suffix}");
    sqlx::query("INSERT INTO fields (field_id, owner, name, boundary_json, created_at) VALUES (?1, ?2, ?3, '{}', '2026-01-01T00:00:00Z')")
        .bind(&field).bind(org).bind(&field).execute(pool).await?;
    sqlx::query("INSERT INTO scenes (scene_id, owner, sensor, acquired_at, data_path, metadata_json, created_at, field_id) VALUES (?1, ?2, 'landsat', '2026-01-01T00:00:00Z', '', '{}', '2026-01-01T00:00:00Z', ?3)")
        .bind(&scene).bind(org).bind(&field).execute(pool).await?;
    let path = tmp.path().join(format!("{report}.pdf"));
    std::fs::write(&path, b"%PDF-1.4\nowned report")?;
    sqlx::query("INSERT INTO reports (report_id, scene_id, field_id, title, format, path, visibility, annotation_count, recommendation_count, created_at) VALUES (?1, ?2, ?3, ?4, 'pdf', ?5, 'grower', 0, 0, '2026-01-01T00:00:00Z')")
        .bind(report).bind(scene).bind(field).bind(suffix).bind(path.to_string_lossy().to_string()).execute(pool).await?;
    Ok(())
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
) -> Result<(StatusCode, axum::http::HeaderMap, Vec<u8>)> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(
            json!({"expires_at":"2099-01-01T00:00:00Z"}).to_string(),
        ))?)
        .await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 1024 * 1024).await?.to_vec();
    Ok((status, headers, body))
}

#[tokio::test]
async fn portal_report_shares_are_org_scoped_audited_and_stream_pdf() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = setup(&tmp).await?;
    let org1 = identity(&pool, "acct-1", "org-1").await?;
    let org2 = identity(&pool, "acct-2", "org-2").await?;
    report(&pool, &tmp, "one", "org-1").await?;
    report(&pool, &tmp, "two", "org-2").await?;

    let (status, _, body) = call(
        &app,
        "POST",
        "/api/portal/reports/report-one/shares",
        Some(&org1),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let share: Value = serde_json::from_slice(&body)?;
    let token = share["share_token"].as_str().unwrap();
    let (status, headers, body) =
        call(&app, "GET", &format!("/api/report-shares/{token}"), None).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf");
    assert!(body.starts_with(b"%PDF"));

    let (status, _, _) = call(
        &app,
        "POST",
        "/api/portal/reports/report-two/shares",
        Some(&org1),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = call(
        &app,
        "DELETE",
        &format!("/api/portal/reports/report-one/shares/{token}"),
        Some(&org2),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = call(
        &app,
        "DELETE",
        &format!("/api/portal/reports/report-one/shares/{token}"),
        Some(&org1),
    )
    .await?;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = call(&app, "GET", &format!("/api/report-shares/{token}"), None).await?;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let events: Vec<String> = sqlx::query_scalar(
        "SELECT event_type FROM report_share_events WHERE share_token = ?1 ORDER BY id",
    )
    .bind(token)
    .fetch_all(&pool)
    .await?;
    assert_eq!(
        events,
        vec![
            "share_created",
            "share_accessed",
            "share_revoked",
            "share_access_denied"
        ]
    );
    Ok(())
}
