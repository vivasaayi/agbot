//! Integration tests for the farmer-portal report inbox and grower PDF
//! generation (batch F-B3): `/api/portal/reports`, read tracking, scoped
//! downloads, and `/api/portal/fields/:field_id/grower-report`.
//! Seeds two orgs to prove tenancy, mirroring tests/portal_farms.rs.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    Router,
};
use chrono::{Duration, SecondsFormat, Utc};
use geo_hub::db::DbPool;
use geo_hub::portal_auth::hash_token;
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

struct TestApp {
    router: Router,
    pool: DbPool,
    _tmp: TempDir,
}

async fn test_app() -> Result<TestApp> {
    let tmp = TempDir::new()?;
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
        pool: pool.clone(),
        config: Arc::new(config),
        scene_search_cache: Default::default(),
    };

    Ok(TestApp {
        router: server::build_router(state),
        pool,
        _tmp: tmp,
    })
}

async fn request(
    app: &TestApp,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
) -> Result<(StatusCode, Value)> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .router
        .clone()
        .oneshot(builder.body(Body::empty())?)
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 8 * 1024 * 1024).await?;
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    Ok((status, value))
}

/// Raw request that keeps the body bytes (for download assertions).
async fn request_bytes(
    app: &TestApp,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
) -> Result<(StatusCode, Vec<u8>)> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .router
        .clone()
        .oneshot(builder.body(Body::empty())?)
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 8 * 1024 * 1024).await?;
    Ok((status, bytes.to_vec()))
}

async fn seed_account(pool: &DbPool, account_id: &str, org_id: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO marketplace_accounts
            (account_id, org_id, party_type, role_refs_json, status, created_at, updated_at)
        VALUES (?1, ?2, 'farmer', '[]', 'active', '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z')
        "#,
    )
    .bind(account_id)
    .bind(org_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert an unexpired session directly and return its bearer token.
async fn seed_session(pool: &DbPool, account_id: &str, org_id: &str) -> Result<String> {
    let token = format!("test-token-{account_id}");
    let expires_at = (Utc::now() + Duration::days(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    sqlx::query(
        r#"
        INSERT INTO portal_sessions
            (session_id, token_hash, account_id, org_id,
             created_at, expires_at, last_seen_at)
        VALUES (?1, ?2, ?3, ?4, '2026-07-01T00:00:00Z', ?5, '2026-07-01T00:00:00Z')
        "#,
    )
    .bind(format!("sess-{account_id}"))
    .bind(hash_token(&token))
    .bind(account_id)
    .bind(org_id)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(token)
}

async fn seed_farm(pool: &DbPool, farm_id: &str, owner: &str, name: &str) -> Result<()> {
    sqlx::query("INSERT INTO farms (farm_id, owner, name, created_at) VALUES (?1, ?2, ?3, '2026-07-01T00:00:00Z')")
        .bind(farm_id)
        .bind(owner)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}

const SQUARE_BOUNDARY: &str =
    r#"{"type":"Polygon","coordinates":[[[0.0,0.0],[10.0,0.0],[10.0,10.0],[0.0,10.0],[0.0,0.0]]]}"#;

async fn seed_field(
    pool: &DbPool,
    field_id: &str,
    farm_id: &str,
    owner: &str,
    name: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO fields (field_id, farm_id, owner, name, crop, season, boundary_json, created_at)
        VALUES (?1, ?2, ?3, ?4, 'corn', '2026', ?5, '2026-07-01T00:00:00Z')
        "#,
    )
    .bind(field_id)
    .bind(farm_id)
    .bind(owner)
    .bind(name)
    .bind(SQUARE_BOUNDARY)
    .execute(pool)
    .await?;
    Ok(())
}

async fn seed_scene(
    pool: &DbPool,
    scene_id: &str,
    field_id: &str,
    acquired_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO scenes
            (scene_id, owner, sensor, acquired_at, data_path, metadata_json,
             created_at, field_id)
        VALUES (?1, 'org-any', 'sentinel-2', ?2, '/tmp/none.tif', '{}',
                '2026-07-01T00:00:00Z', ?3)
        "#,
    )
    .bind(scene_id)
    .bind(acquired_at)
    .bind(field_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Register a derived layer for the scene so grower layer_refs are non-empty.
async fn seed_catalog_product(
    pool: &DbPool,
    product_id: &str,
    scene_id: &str,
    field_id: &str,
    kind: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO catalog_products
            (product_id, level, kind, algorithm_id, algorithm_version,
             parameters_json, parameters_hash, field_id, scene_id, created_at)
        VALUES (?1, 'l2', ?2, 'alg-ndvi', '1.0', '{}', ?3, ?4, ?5, '2026-07-01T00:00:00Z')
        "#,
    )
    .bind(product_id)
    .bind(kind)
    .bind(format!("hash-{product_id}"))
    .bind(field_id)
    .bind(scene_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn seed_report(
    pool: &DbPool,
    report_id: &str,
    scene_id: &str,
    field_id: &str,
    title: &str,
    format: &str,
    path: &str,
    created_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO reports
            (report_id, scene_id, field_id, title, format, path, visibility,
             annotation_count, recommendation_count, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'org', 0, 0, ?7)
        "#,
    )
    .bind(report_id)
    .bind(scene_id)
    .bind(field_id)
    .bind(title)
    .bind(format)
    .bind(path)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(())
}

async fn seed_finding(
    pool: &DbPool,
    finding_id: &str,
    field_id: &str,
    severity: &str,
    created_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO application_findings
            (finding_id, run_id, app_id, field_id, kind, severity, created_at)
        VALUES (?1, 'run-1', 'app-1', ?2, 'stress_zone', ?3, ?4)
        "#,
    )
    .bind(finding_id)
    .bind(field_id)
    .bind(severity)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(())
}

async fn seed_recommendation(
    pool: &DbPool,
    recommendation_id: &str,
    field_id: &str,
    title: &str,
    priority: &str,
    status: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO recommendations
            (recommendation_id, scene_id, field_id, title, priority, status,
             created_at, updated_at)
        VALUES (?1, 'scene-any', ?2, ?3, ?4, ?5, '2026-07-02T00:00:00Z', '2026-07-02T00:00:00Z')
        "#,
    )
    .bind(recommendation_id)
    .bind(field_id)
    .bind(title)
    .bind(priority)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(())
}

/// The inbox lists only reports for fields owned by the caller's org, newest
/// first, each carrying a `read` flag and its field name.
#[tokio::test]
async fn portal_reports_lists_reports_for_my_fields_with_read_flags() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;

    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;
    seed_farm(&app.pool, "farm-x", "org-2", "Other Farm").await?;
    seed_field(&app.pool, "field-x", "farm-x", "org-2", "Other Field").await?;

    seed_report(
        &app.pool,
        "report-old",
        "scene-1",
        "field-1",
        "Older report",
        "html",
        "/tmp/none-old.html",
        "2026-07-01T00:00:00Z",
    )
    .await?;
    seed_report(
        &app.pool,
        "report-new",
        "scene-2",
        "field-1",
        "Newer report",
        "html",
        "/tmp/none-new.html",
        "2026-07-03T00:00:00Z",
    )
    .await?;
    seed_report(
        &app.pool,
        "report-other-org",
        "scene-x",
        "field-x",
        "Other org report",
        "html",
        "/tmp/none-x.html",
        "2026-07-02T00:00:00Z",
    )
    .await?;

    // Pre-seed one read row for this account: report-old is already read.
    sqlx::query(
        "INSERT INTO portal_report_reads (account_id, report_id, read_at) \
         VALUES ('acct-1', 'report-old', '2026-07-04T00:00:00Z')",
    )
    .execute(&app.pool)
    .await?;

    let (status, body) = request(&app, "GET", "/api/portal/reports", Some(&token)).await?;

    assert_eq!(status, StatusCode::OK, "reports failed: {body}");
    let reports = body.as_array().expect("reports array");
    assert_eq!(reports.len(), 2, "only org-1 reports: {body}");
    assert!(!body.to_string().contains("report-other-org"));
    // Newest first.
    assert_eq!(reports[0]["report_id"], "report-new");
    assert_eq!(reports[0]["read"], false);
    assert_eq!(reports[0]["field_name"], "Field A");
    assert_eq!(reports[0]["field_id"], "field-1");
    assert_eq!(reports[1]["report_id"], "report-old");
    assert_eq!(reports[1]["read"], true);
    Ok(())
}

/// Marking a report read is idempotent, per-account, and flips it out of the
/// `unread_only` view.
#[tokio::test]
async fn marking_report_read_flips_unread_count() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;
    seed_report(
        &app.pool,
        "report-1",
        "scene-1",
        "field-1",
        "Report",
        "html",
        "/tmp/none.html",
        "2026-07-01T00:00:00Z",
    )
    .await?;

    let (status, body) = request(
        &app,
        "GET",
        "/api/portal/reports?unread_only=true",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "unread list failed: {body}");
    assert_eq!(body.as_array().expect("array").len(), 1);

    let (status, body) = request(
        &app,
        "POST",
        "/api/portal/reports/report-1/read",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "mark read failed: {body}");

    // Idempotent second mark.
    let (status, _) = request(
        &app,
        "POST",
        "/api/portal/reports/report-1/read",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = request(
        &app,
        "GET",
        "/api/portal/reports?unread_only=true",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().expect("array").len(), 0, "unread: {body}");

    let (_, body) = request(&app, "GET", "/api/portal/reports", Some(&token)).await?;
    assert_eq!(body[0]["read"], true, "full list keeps report: {body}");
    Ok(())
}

/// Downloading (or marking read) another org's report must be 404, never 403.
#[tokio::test]
async fn portal_report_download_cross_org_is_not_found() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-x", "org-2", "Other Farm").await?;
    seed_field(&app.pool, "field-x", "farm-x", "org-2", "Other Field").await?;
    seed_report(
        &app.pool,
        "report-x",
        "scene-x",
        "field-x",
        "Other org report",
        "html",
        "/tmp/none-x.html",
        "2026-07-01T00:00:00Z",
    )
    .await?;

    let (status, _) = request(
        &app,
        "GET",
        "/api/portal/reports/report-x/download",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-org download is 404");

    let (status, _) = request(
        &app,
        "POST",
        "/api/portal/reports/report-x/read",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-org mark-read is 404");

    let (status, _) = request(
        &app,
        "GET",
        "/api/portal/reports/report-missing/download",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "missing report is 404");
    Ok(())
}

/// Generating a grower report renders a real PDF on disk, inserts a reports
/// row, and surfaces it unread in the inbox; the download streams the PDF.
#[tokio::test]
async fn grower_report_generation_persists_pdf_and_inbox_row() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;
    seed_scene(&app.pool, "scene-1", "field-1", "2026-07-01T10:00:00Z").await?;
    seed_catalog_product(&app.pool, "prod-ndvi", "scene-1", "field-1", "ndvi").await?;
    seed_finding(
        &app.pool,
        "finding-1",
        "field-1",
        "high",
        "2026-07-02T00:00:00Z",
    )
    .await?;
    seed_recommendation(&app.pool, "rec-1", "field-1", "Scout NW", "high", "open").await?;
    seed_recommendation(&app.pool, "rec-2", "field-1", "Done", "low", "completed").await?;

    let (status, body) = request(
        &app,
        "POST",
        "/api/portal/fields/field-1/grower-report",
        Some(&token),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "generation failed: {body}");
    assert_eq!(body["field_id"], "field-1");
    assert_eq!(body["scene_id"], "scene-1");
    assert_eq!(body["format"], "pdf");
    assert_eq!(body["visibility"], "grower");
    assert_eq!(body["read"], false);
    let title = body["title"].as_str().expect("title");
    assert!(
        title.starts_with("Grower report — Field A"),
        "title: {title}"
    );
    let report_id = body["report_id"].as_str().expect("report_id").to_string();

    // The PDF exists on disk and is a real PDF.
    let (path,): (String,) = sqlx::query_as("SELECT path FROM reports WHERE report_id = ?1")
        .bind(&report_id)
        .fetch_one(&app.pool)
        .await?;
    let bytes = std::fs::read(PathBuf::from(&path))?;
    assert!(bytes.starts_with(b"%PDF"), "not a PDF: {path}");

    // It appears in the inbox, unread.
    let (status, body) = request(&app, "GET", "/api/portal/reports", Some(&token)).await?;
    assert_eq!(status, StatusCode::OK);
    let inbox = body.as_array().expect("array");
    assert_eq!(inbox.len(), 1, "inbox: {body}");
    assert_eq!(inbox[0]["report_id"], report_id.as_str());
    assert_eq!(inbox[0]["read"], false);

    // And the portal download streams the same PDF bytes.
    let (status, downloaded) = request_bytes(
        &app,
        "GET",
        &format!("/api/portal/reports/{report_id}/download"),
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(downloaded.starts_with(b"%PDF"));
    Ok(())
}

/// A field with no linked scene cannot produce a grower report: 400 with a
/// clear message, and nothing is persisted.
#[tokio::test]
async fn grower_report_for_field_without_scene_is_bad_request() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;

    let (status, body) = request(
        &app,
        "POST",
        "/api/portal/fields/field-1/grower-report",
        Some(&token),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST, "no-scene must be 400");
    assert!(
        body.to_string().contains("no linked scene"),
        "clear message expected: {body}"
    );
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reports")
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(count, 0, "nothing persisted");
    Ok(())
}

/// Generating a grower report for another org's field must be 404.
#[tokio::test]
async fn grower_report_cross_org_field_is_not_found() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-x", "org-2", "Other Farm").await?;
    seed_field(&app.pool, "field-x", "farm-x", "org-2", "Other Field").await?;
    seed_scene(&app.pool, "scene-x", "field-x", "2026-07-01T10:00:00Z").await?;

    let (status, _) = request(
        &app,
        "POST",
        "/api/portal/fields/field-x/grower-report",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-org must be 404");
    Ok(())
}

#[tokio::test]
async fn authenticated_grower_pdf_delivery_and_revocable_advisor_share_are_tenant_scoped(
) -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "grower-1", "org-grower").await?;
    seed_account(&app.pool, "advisor-2", "org-advisor").await?;
    let grower_token = seed_session(&app.pool, "grower-1", "org-grower").await?;
    let advisor_token = seed_session(&app.pool, "advisor-2", "org-advisor").await?;

    seed_farm(&app.pool, "farm-grower", "org-grower", "Grower Farm").await?;
    seed_field(
        &app.pool,
        "field-grower",
        "farm-grower",
        "org-grower",
        "North Forty",
    )
    .await?;
    seed_scene(
        &app.pool,
        "scene-grower",
        "field-grower",
        "2026-07-15T10:00:00Z",
    )
    .await?;
    seed_catalog_product(
        &app.pool,
        "product-grower-ndvi",
        "scene-grower",
        "field-grower",
        "ndvi",
    )
    .await?;
    seed_finding(
        &app.pool,
        "finding-grower",
        "field-grower",
        "high",
        "2026-07-15T11:00:00Z",
    )
    .await?;
    seed_recommendation(
        &app.pool,
        "recommendation-grower",
        "field-grower",
        "Scout the north irrigation line",
        "high",
        "open",
    )
    .await?;

    seed_farm(&app.pool, "farm-advisor", "org-advisor", "Advisor Farm").await?;
    seed_field(
        &app.pool,
        "field-advisor",
        "farm-advisor",
        "org-advisor",
        "Foreign Field",
    )
    .await?;
    seed_scene(
        &app.pool,
        "scene-advisor",
        "field-advisor",
        "2026-07-15T10:00:00Z",
    )
    .await?;

    let (status, generated) = request(
        &app,
        "POST",
        "/api/portal/fields/field-grower/grower-report",
        Some(&grower_token),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "generation failed: {generated}");
    let report_id = generated["report_id"]
        .as_str()
        .expect("generated report id")
        .to_string();
    assert_eq!(generated["format"], "pdf");

    let (status, inbox) = request(&app, "GET", "/api/portal/reports", Some(&grower_token)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(inbox.as_array().map(Vec::len), Some(1));
    assert_eq!(inbox[0]["report_id"], report_id);

    let download = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/portal/reports/{report_id}/download"))
                .header("authorization", format!("Bearer {grower_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(download.status(), StatusCode::OK);
    assert_eq!(
        download
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/pdf")
    );
    let pdf = to_bytes(download.into_body(), 8 * 1024 * 1024).await?;
    assert!(pdf.starts_with(b"%PDF"));

    let share_response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/portal/reports/{report_id}/shares"))
                .header("authorization", format!("Bearer {grower_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"expires_at":"2099-01-01T00:00:00Z"}"#))?,
        )
        .await?;
    assert_eq!(share_response.status(), StatusCode::OK);
    let body = to_bytes(share_response.into_body(), 64 * 1024).await?;
    let share: Value = serde_json::from_slice(&body)?;
    let share_token = share["share_token"]
        .as_str()
        .expect("share token")
        .to_string();
    let public_path = share["url_path"]
        .as_str()
        .expect("public share path")
        .to_string();

    let (status, public_pdf) = request_bytes(&app, "GET", &public_path, None).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(public_pdf.starts_with(b"%PDF"));
    let created_events: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM report_share_events WHERE share_token = ?1")
            .bind(&share_token)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(created_events, 2, "creation and public access are audited");

    // A caller from another org cannot discover, download, generate, or share
    // the grower's report even when it knows the resource identifiers.
    let (status, foreign_inbox) =
        request(&app, "GET", "/api/portal/reports", Some(&advisor_token)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(foreign_inbox.as_array().map(Vec::len), Some(0));
    let (status, _) = request(
        &app,
        "GET",
        &format!("/api/portal/reports/{report_id}/download"),
        Some(&advisor_token),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = request(
        &app,
        "POST",
        "/api/portal/fields/field-grower/grower-report",
        Some(&advisor_token),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let foreign_share = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/portal/reports/{report_id}/shares"))
                .header("authorization", format!("Bearer {advisor_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))?,
        )
        .await?;
    assert_eq!(foreign_share.status(), StatusCode::NOT_FOUND);

    let (status, _) = request(
        &app,
        "DELETE",
        &format!("/api/portal/reports/{report_id}/shares/{share_token}"),
        Some(&grower_token),
    )
    .await?;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = request_bytes(&app, "GET", &public_path, None).await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let final_events: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM report_share_events WHERE share_token = ?1")
            .bind(&share_token)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(
        final_events, 4,
        "create, access, revoke, and revoked denial are audited"
    );
    Ok(())
}
