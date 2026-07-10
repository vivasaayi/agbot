//! Integration tests for portal recommendation acknowledgement (batch F-B4):
//! `PUT /api/portal/recommendations/:recommendation_id/status`.
//!
//! The handler mirrors the org-side `update_scene_recommendation` write
//! exactly: one UPDATE of `recommendations` (status + updated_at). The org
//! route records no separate transition row — recommendation lineage is
//! written only at create time — so these tests assert the row mutation and
//! nothing else.

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
use serde_json::{json, Value};
use sqlx::Row;
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

async fn request_json(
    app: &TestApp,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> Result<(StatusCode, Value)> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let request = match body {
        Some(value) => builder
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&value)?))?,
        None => builder.body(Body::empty())?,
    };
    let response = app.router.clone().oneshot(request).await?;
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

const SEEDED_UPDATED_AT: &str = "2026-07-02T00:00:00Z";

async fn seed_recommendation(
    pool: &DbPool,
    recommendation_id: &str,
    field_id: Option<&str>,
    status: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO recommendations
            (recommendation_id, scene_id, field_id, title, priority, status,
             created_at, updated_at)
        VALUES (?1, 'scene-any', ?2, 'Scout the NW zone', 'high', ?3, ?4, ?4)
        "#,
    )
    .bind(recommendation_id)
    .bind(field_id)
    .bind(status)
    .bind(SEEDED_UPDATED_AT)
    .execute(pool)
    .await?;
    Ok(())
}

async fn recommendation_row(pool: &DbPool, recommendation_id: &str) -> Result<(String, String)> {
    let row = sqlx::query(
        "SELECT status, updated_at FROM recommendations \
                           WHERE recommendation_id = ?1",
    )
    .bind(recommendation_id)
    .fetch_one(pool)
    .await?;
    Ok((row.get("status"), row.get("updated_at")))
}

/// open -> completed via the portal: the response carries the new status, the
/// stored row is updated, and updated_at moves. The mirrored org-side write is
/// exactly one UPDATE (status + updated_at) — no transition table exists, so
/// no extra rows are asserted.
#[tokio::test]
async fn portal_recommendation_ack_transitions_status() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;
    seed_recommendation(&app.pool, "rec-1", Some("field-1"), "open").await?;

    let (status, body) = request_json(
        &app,
        "PUT",
        "/api/portal/recommendations/rec-1/status",
        Some(&token),
        Some(json!({"status": "completed"})),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["recommendation_id"], "rec-1");
    assert_eq!(body["status"], "completed");
    assert_eq!(body["field_id"], "field-1");
    assert_ne!(
        body["updated_at"].as_str().expect("updated_at present"),
        SEEDED_UPDATED_AT,
        "updated_at must change on transition"
    );

    let (stored_status, stored_updated_at) = recommendation_row(&app.pool, "rec-1").await?;
    assert_eq!(stored_status, "completed");
    assert_ne!(stored_updated_at, SEEDED_UPDATED_AT);
    Ok(())
}

/// Unknown status strings are rejected with 400 and the row is untouched.
#[tokio::test]
async fn portal_recommendation_invalid_status_is_bad_request() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;
    seed_recommendation(&app.pool, "rec-1", Some("field-1"), "open").await?;

    let (status, body) = request_json(
        &app,
        "PUT",
        "/api/portal/recommendations/rec-1/status",
        Some(&token),
        Some(json!({"status": "acknowledged"})),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    let (stored_status, stored_updated_at) = recommendation_row(&app.pool, "rec-1").await?;
    assert_eq!(stored_status, "open");
    assert_eq!(stored_updated_at, SEEDED_UPDATED_AT);
    Ok(())
}

/// A recommendation on another org's field is indistinguishable from a
/// missing one: 404, and the other org's row is untouched.
#[tokio::test]
async fn portal_recommendation_cross_org_is_not_found() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-x", "org-2", "Other Farm").await?;
    seed_field(&app.pool, "field-x", "farm-x", "org-2", "Other Field").await?;
    seed_recommendation(&app.pool, "rec-x", Some("field-x"), "open").await?;

    let (status, body) = request_json(
        &app,
        "PUT",
        "/api/portal/recommendations/rec-x/status",
        Some(&token),
        Some(json!({"status": "completed"})),
    )
    .await?;

    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    let (stored_status, _) = recommendation_row(&app.pool, "rec-x").await?;
    assert_eq!(stored_status, "open");
    Ok(())
}

/// A recommendation with no field cannot be tied to any org, so the portal
/// treats it as missing (404).
#[tokio::test]
async fn portal_recommendation_without_field_is_not_found() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_recommendation(&app.pool, "rec-orphan", None, "open").await?;

    let (status, body) = request_json(
        &app,
        "PUT",
        "/api/portal/recommendations/rec-orphan/status",
        Some(&token),
        Some(json!({"status": "completed"})),
    )
    .await?;

    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    let (stored_status, _) = recommendation_row(&app.pool, "rec-orphan").await?;
    assert_eq!(stored_status, "open");
    Ok(())
}
