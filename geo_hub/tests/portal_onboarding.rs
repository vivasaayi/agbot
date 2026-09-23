//! Farmer self-service onboarding acceptance.
//!
//! `/api/portal/farms`, `/api/portal/fields`, and
//! `/api/portal/fields/:field_id/overview`. Seeds two orgs to prove tenancy.

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

async fn test_app(enabled: bool) -> Result<TestApp> {
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("geo_hub_test.db");
    let mut config = HubConfig {
        bind_address: "127.0.0.1:0".to_string(),
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };

    config.pipeline.enabled = enabled;
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
    body: Value,
) -> Result<(StatusCode, Value)> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .router
        .clone()
        .oneshot(
            builder
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))?,
        )
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

fn draft() -> Value {
    serde_json::json!({"farm_name":"My farm","name":"North field","crop":"corn","season":"2026",
      "boundary":{"crs":"EPSG:4326","coordinates":[
        {"longitude":-96.5,"latitude":41.2},{"longitude":-96.49,"latitude":41.2},
        {"longitude":-96.49,"latitude":41.21},{"longitude":-96.5,"latitude":41.2}]}})
}

#[tokio::test]
async fn acceptance_onboarding_is_atomic_owned_and_starts_monitoring() -> Result<()> {
    let app = test_app(false).await?;
    seed_account(&app.pool, "alice", "org-a").await?;
    seed_account(&app.pool, "bob", "org-b").await?;
    let alice = seed_session(&app.pool, "alice", "org-a").await?;
    let bob = seed_session(&app.pool, "bob", "org-b").await?;
    assert_eq!(
        request(&app, "POST", "/api/portal/fields", None, draft())
            .await?
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, created) =
        request(&app, "POST", "/api/portal/fields", Some(&alice), draft()).await?;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["field"]["owner"], "org-a");
    assert_eq!(created["monitoring"]["status"], "worker_disabled");
    let id = created["field"]["field_id"].as_str().unwrap();
    let farm = created["field"]["farm_id"].as_str().unwrap();
    let (status, _) = request(
        &app,
        "GET",
        &format!("/api/portal/fields/{id}"),
        Some(&bob),
        Value::Null,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let sub: (String, String) =
        sqlx::query_as("SELECT dataset,indices_json FROM satellite_subscriptions WHERE field_id=?")
            .bind(id)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(sub, ("landsat".into(), "[\"ndvi\"]".into()));
    let job: (String, String) =
        sqlx::query_as("SELECT kind,status FROM pipeline_jobs WHERE field_id=?")
            .bind(id)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(job, ("discover".into(), "queued".into()));
    let mut foreign = draft();
    foreign.as_object_mut().unwrap().remove("farm_name");
    foreign["farm_id"] = farm.into();
    assert_eq!(
        request(&app, "POST", "/api/portal/fields", Some(&bob), foreign)
            .await?
            .0,
        StatusCode::NOT_FOUND
    );
    let mut invalid = draft();
    invalid["boundary"]["coordinates"][2] = invalid["boundary"]["coordinates"][1].clone();
    assert_eq!(
        request(&app, "POST", "/api/portal/fields", Some(&alice), invalid)
            .await?
            .0,
        StatusCode::BAD_REQUEST
    );
    for table in [
        "farms",
        "fields",
        "satellite_subscriptions",
        "pipeline_jobs",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&app.pool)
            .await?;
        assert_eq!(count, 1, "{table}");
    }
    Ok(())
}

#[tokio::test]
async fn enabled_monitoring_reuses_owned_farm_and_rejects_owner_input() -> Result<()> {
    let app = test_app(true).await?;
    seed_account(&app.pool, "alice", "org-a").await?;
    let token = seed_session(&app.pool, "alice", "org-a").await?;
    let (_, created) = request(&app, "POST", "/api/portal/fields", Some(&token), draft()).await?;
    assert_eq!(created["monitoring"]["status"], "queued");
    let mut second = draft();
    second.as_object_mut().unwrap().remove("farm_name");
    second["farm_id"] = created["field"]["farm_id"].clone();
    let (status, _) = request(&app, "POST", "/api/portal/fields", Some(&token), second).await?;
    assert_eq!(status, StatusCode::OK);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM farms")
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(count, 1);
    for key in ["owner", "org_id", "field_id"] {
        let mut spoof = draft();
        spoof[key] = "foreign".into();
        assert_eq!(
            request(&app, "POST", "/api/portal/fields", Some(&token), spoof)
                .await?
                .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    Ok(())
}

#[tokio::test]
async fn failed_queue_insert_rolls_back_entire_onboarding() -> Result<()> {
    let app = test_app(true).await?;
    seed_account(&app.pool, "alice", "org-a").await?;
    let token = seed_session(&app.pool, "alice", "org-a").await?;
    sqlx::query("CREATE TRIGGER reject_initial_job BEFORE INSERT ON pipeline_jobs BEGIN SELECT RAISE(ABORT, 'test storage failure'); END")
      .execute(&app.pool).await?;
    assert_eq!(
        request(&app, "POST", "/api/portal/fields", Some(&token), draft())
            .await?
            .0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    for table in [
        "farms",
        "fields",
        "satellite_subscriptions",
        "pipeline_jobs",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&app.pool)
            .await?;
        assert_eq!(count, 0, "{table}");
    }
    Ok(())
}
