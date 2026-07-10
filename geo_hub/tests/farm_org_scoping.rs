//! Cross-tenant scoping for the non-portal farm/field API (`/api/farms`,
//! `/api/fields`, exports). An authenticated session forces org scoping from
//! the principal — client-supplied `org_id` is ignored and cross-org path
//! reads 404. Anonymous callers keep the legacy (unlocked) query-param
//! behavior. Two orgs are seeded to prove tenancy. See batch 3.1c.

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
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await?;
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

async fn seed_field(
    pool: &DbPool,
    field_id: &str,
    farm_id: &str,
    owner: &str,
    name: &str,
) -> Result<()> {
    // A minimal-but-valid boundary (>= 3 in-range coords) so the field decode
    // path in list/export succeeds.
    const BOUNDARY: &str = r#"{"coordinates":[{"longitude":0.0,"latitude":0.0},{"longitude":0.1,"latitude":0.0},{"longitude":0.1,"latitude":0.1}]}"#;
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
    .bind(BOUNDARY)
    .execute(pool)
    .await?;
    Ok(())
}

async fn two_org_app() -> Result<(TestApp, String)> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_farm(&app.pool, "farm-2", "org-1", "South Farm").await?;
    seed_farm(&app.pool, "farm-x", "org-2", "Other Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;
    seed_field(&app.pool, "field-x", "farm-x", "org-2", "Other Field").await?;
    Ok((app, token))
}

fn item_ids(page: &Value, id_key: &str) -> Vec<String> {
    page["items"]
        .as_array()
        .expect("items array")
        .iter()
        .map(|item| item[id_key].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn farms_list_is_scoped_to_session_org_and_ignores_client_param() -> Result<()> {
    let (app, token) = two_org_app().await?;

    // Authenticated: only org-1 farms, even though a cross-org param is passed.
    let (status, body) = request(&app, "GET", "/api/farms?org_id=org-2", Some(&token)).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids = item_ids(&body, "farm_id");
    assert_eq!(ids.len(), 2, "only org-1 farms: {body}");
    assert!(ids.contains(&"farm-1".to_string()));
    assert!(ids.contains(&"farm-2".to_string()));
    assert!(!body.to_string().contains("farm-x"), "no cross-org leak");
    Ok(())
}

#[tokio::test]
async fn fields_list_and_export_are_scoped_to_session_org() -> Result<()> {
    let (app, token) = two_org_app().await?;

    let (status, body) = request(&app, "GET", "/api/fields", Some(&token)).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids = item_ids(&body, "field_id");
    assert_eq!(ids, vec!["field-1".to_string()], "only org-1 field: {body}");

    // Export is likewise scoped: only org-1's field appears.
    let (status, export) = request(&app, "GET", "/api/fields/export/geojson", Some(&token)).await?;
    assert_eq!(status, StatusCode::OK, "{export}");
    let export_text = export.to_string();
    assert!(export_text.contains("field-1"));
    assert!(
        !export_text.contains("field-x"),
        "export must not leak org-2"
    );
    Ok(())
}

#[tokio::test]
async fn get_farm_cross_org_reads_as_not_found() -> Result<()> {
    let (app, token) = two_org_app().await?;

    // Own farm: OK.
    let (status, _) = request(&app, "GET", "/api/farms/farm-1", Some(&token)).await?;
    assert_eq!(status, StatusCode::OK);

    // Another org's farm is indistinguishable from a missing one.
    let (status, _) = request(&app, "GET", "/api/farms/farm-x", Some(&token)).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-org farm must be 404");

    let (status, _) = request(&app, "GET", "/api/farms/farm-missing", Some(&token)).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn anonymous_caller_keeps_unlocked_behavior() -> Result<()> {
    // With no session and the gate off (default), the legacy query-param scoping
    // still applies — this documents the unlocked single-user mode.
    let (app, _token) = two_org_app().await?;

    // No token, no filter: sees every org's farms.
    let (status, body) = request(&app, "GET", "/api/farms", None).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids = item_ids(&body, "farm_id");
    assert_eq!(ids.len(), 3, "unlocked mode lists all orgs: {body}");

    // A path read of any farm still works anonymously.
    let (status, _) = request(&app, "GET", "/api/farms/farm-x", None).await?;
    assert_eq!(status, StatusCode::OK);
    Ok(())
}
