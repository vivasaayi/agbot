//! Integration tests for the portal alert feed and notifications summary
//! (batch F-B4): `GET /api/portal/alerts?since=&severity=` and
//! `GET /api/portal/notifications/summary`. Seeds two orgs to prove tenancy,
//! mirroring tests/portal_farms.rs.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use chrono::{Duration, SecondsFormat, Utc};
use geo_hub::db::DbPool;
use geo_hub::portal_auth::hash_token;
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use serde_json::Value;
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
    sqlx::query(
        "INSERT INTO farms (farm_id, owner, name, created_at) \
         VALUES (?1, ?2, ?3, '2026-07-01T00:00:00Z')",
    )
    .bind(farm_id)
    .bind(owner)
    .bind(name)
    .execute(pool)
    .await?;
    Ok(())
}

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
        VALUES (?1, ?2, ?3, ?4, 'corn', '2026', '{}', '2026-07-01T00:00:00Z')
        "#,
    )
    .bind(field_id)
    .bind(farm_id)
    .bind(owner)
    .bind(name)
    .execute(pool)
    .await?;
    Ok(())
}

async fn seed_alert(
    pool: &DbPool,
    alert_id: &str,
    field_id: &str,
    severity: &str,
    fired_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO fired_alerts
            (alert_id, matched_rule_id, source_finding_id, field_id, event_type,
             subject_ref, severity, explanation, fired_at)
        VALUES (?1, 'rule-1', 'finding-1', ?2, 'ndvi_drop', ?2, ?3, 'NDVI dropped', ?4)
        "#,
    )
    .bind(alert_id)
    .bind(field_id)
    .bind(severity)
    .bind(fired_at)
    .execute(pool)
    .await?;
    Ok(())
}

async fn seed_lifecycle(pool: &DbPool, alert_id: &str, state: &str, fired_at: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO alert_lifecycle
            (alert_id, source_event_ref, state, fired_at, transitions_json, updated_at)
        VALUES (?1, ?2, ?3, ?4, '[]', ?4)
        "#,
    )
    .bind(alert_id)
    .bind(format!("alert:{alert_id}"))
    .bind(state)
    .bind(fired_at)
    .execute(pool)
    .await?;
    Ok(())
}

async fn seed_report(
    pool: &DbPool,
    report_id: &str,
    field_id: &str,
    created_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO reports
            (report_id, scene_id, field_id, title, format, path, visibility,
             annotation_count, recommendation_count, created_at)
        VALUES (?1, 'scene-any', ?2, 'Report', 'pdf', '/tmp/none.pdf', 'org', 0, 0, ?3)
        "#,
    )
    .bind(report_id)
    .bind(field_id)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(())
}

async fn seed_recommendation(
    pool: &DbPool,
    recommendation_id: &str,
    field_id: &str,
    status: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO recommendations
            (recommendation_id, scene_id, field_id, title, priority, status,
             created_at, updated_at)
        VALUES (?1, 'scene-any', ?2, 'Scout', 'high', ?3,
                '2026-07-02T00:00:00Z', '2026-07-02T00:00:00Z')
        "#,
    )
    .bind(recommendation_id)
    .bind(field_id)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(())
}

fn rfc3339_days_ago(days: i64) -> String {
    (Utc::now() - Duration::days(days)).to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// The feed carries only alerts on my org's fields, newest first, joined to
/// the field name and (when present) the alert's current lifecycle state.
#[tokio::test]
async fn portal_alerts_lists_fired_alerts_for_my_fields_only() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;
    seed_farm(&app.pool, "farm-x", "org-2", "Other Farm").await?;
    seed_field(&app.pool, "field-x", "farm-x", "org-2", "Other Field").await?;

    seed_alert(
        &app.pool,
        "alert-old",
        "field-1",
        "high",
        "2026-07-03T00:00:00Z",
    )
    .await?;
    seed_alert(
        &app.pool,
        "alert-new",
        "field-1",
        "low",
        "2026-07-05T00:00:00Z",
    )
    .await?;
    seed_alert(
        &app.pool,
        "alert-x",
        "field-x",
        "high",
        "2026-07-05T12:00:00Z",
    )
    .await?;
    seed_lifecycle(
        &app.pool,
        "alert-new",
        "acknowledged",
        "2026-07-05T00:00:00Z",
    )
    .await?;

    let (status, body) = request(&app, "GET", "/api/portal/alerts", Some(&token)).await?;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    let alerts = body.as_array().expect("array body");
    assert_eq!(alerts.len(), 2, "only my org's alerts: {body}");
    assert_eq!(alerts[0]["alert_id"], "alert-new", "newest first");
    assert_eq!(alerts[0]["field_name"], "Field A");
    assert_eq!(alerts[0]["lifecycle_state"], "acknowledged");
    assert_eq!(alerts[1]["alert_id"], "alert-old");
    assert_eq!(alerts[1]["lifecycle_state"], Value::Null);
    Ok(())
}

/// `since` keeps alerts fired at/after the bound; `severity` narrows by exact
/// (case-insensitive) label; both compose.
#[tokio::test]
async fn portal_alerts_since_and_severity_filters() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;

    seed_alert(
        &app.pool,
        "alert-early-high",
        "field-1",
        "high",
        "2026-07-01T00:00:00Z",
    )
    .await?;
    seed_alert(
        &app.pool,
        "alert-late-high",
        "field-1",
        "high",
        "2026-07-05T00:00:00Z",
    )
    .await?;
    seed_alert(
        &app.pool,
        "alert-late-low",
        "field-1",
        "low",
        "2026-07-05T06:00:00Z",
    )
    .await?;

    let (status, body) = request(
        &app,
        "GET",
        "/api/portal/alerts?since=2026-07-04T00:00:00Z",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let ids: Vec<&str> = body
        .as_array()
        .expect("array body")
        .iter()
        .map(|alert| alert["alert_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["alert-late-low", "alert-late-high"]);

    let (status, body) = request(
        &app,
        "GET",
        "/api/portal/alerts?severity=HIGH",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let ids: Vec<&str> = body
        .as_array()
        .expect("array body")
        .iter()
        .map(|alert| alert["alert_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["alert-late-high", "alert-early-high"]);

    let (status, body) = request(
        &app,
        "GET",
        "/api/portal/alerts?since=2026-07-04T00:00:00Z&severity=high",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let ids: Vec<&str> = body
        .as_array()
        .expect("array body")
        .iter()
        .map(|alert| alert["alert_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["alert-late-high"]);

    // Malformed `since` is rejected rather than silently matching nothing.
    let (status, _) = request(
        &app,
        "GET",
        "/api/portal/alerts?since=yesterday",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}

/// The summary counts only my org's artifacts: unread reports for my account,
/// open recommendations, and alerts fired within the trailing 7 days.
#[tokio::test]
async fn notifications_summary_counts_unread_reports_open_recommendations_and_alerts() -> Result<()>
{
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_account(&app.pool, "acct-2", "org-2").await?;
    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;
    seed_farm(&app.pool, "farm-x", "org-2", "Other Farm").await?;
    seed_field(&app.pool, "field-x", "farm-x", "org-2", "Other Field").await?;

    // Reports: two for org-1 (one already read by acct-1), one for org-2.
    seed_report(&app.pool, "report-read", "field-1", "2026-07-01T00:00:00Z").await?;
    seed_report(
        &app.pool,
        "report-unread",
        "field-1",
        "2026-07-02T00:00:00Z",
    )
    .await?;
    seed_report(&app.pool, "report-x", "field-x", "2026-07-02T00:00:00Z").await?;
    let (status, body) = request(
        &app,
        "POST",
        "/api/portal/reports/report-read/read",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    // Recommendations: one open + one completed for org-1, one open for org-2.
    seed_recommendation(&app.pool, "rec-open", "field-1", "open").await?;
    seed_recommendation(&app.pool, "rec-done", "field-1", "completed").await?;
    seed_recommendation(&app.pool, "rec-x", "field-x", "open").await?;

    // Alerts: one recent + one stale for org-1, one recent for org-2.
    seed_alert(
        &app.pool,
        "alert-recent",
        "field-1",
        "high",
        &rfc3339_days_ago(1),
    )
    .await?;
    seed_alert(
        &app.pool,
        "alert-stale",
        "field-1",
        "high",
        &rfc3339_days_ago(30),
    )
    .await?;
    seed_alert(
        &app.pool,
        "alert-x",
        "field-x",
        "high",
        &rfc3339_days_ago(1),
    )
    .await?;

    let (status, body) = request(
        &app,
        "GET",
        "/api/portal/notifications/summary",
        Some(&token),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["unread_reports"], 1, "body: {body}");
    assert_eq!(body["open_recommendations"], 1, "body: {body}");
    assert_eq!(body["alerts_last_7d"], 1, "body: {body}");
    Ok(())
}
