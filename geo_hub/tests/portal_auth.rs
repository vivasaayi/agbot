//! Integration tests for portal access-code auth: admin-issued access codes,
//! bearer-token sessions, and the authenticated portal identity endpoints.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::config::SecurityConfig;
use geo_hub::db::DbPool;
use geo_hub::portal_auth::hash_token;
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

/// Admin bearer token the test server is configured with; admin access-code
/// routes require it.
const ADMIN_TOKEN: &str = "test-admin-token";

struct TestApp {
    router: Router,
    pool: DbPool,
    _tmp: TempDir,
}

async fn test_app() -> Result<TestApp> {
    test_app_with_admin_token(Some(ADMIN_TOKEN.to_string())).await
}

async fn test_app_with_admin_token(admin_token: Option<String>) -> Result<TestApp> {
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("geo_hub_test.db");
    let config = HubConfig {
        bind_address: "127.0.0.1:0".to_string(),
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: tmp.path().join("data"),
        security: SecurityConfig { admin_token },
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

async fn seed_account(pool: &DbPool, account_id: &str, org_id: &str, status: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO marketplace_accounts
            (account_id, org_id, party_type, role_refs_json, status, created_at, updated_at)
        VALUES (?1, ?2, 'farmer', '[]', ?3, '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z')
        "#,
    )
    .bind(account_id)
    .bind(org_id)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(())
}

async fn request(
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
        Some(body) => builder
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))?,
        None => builder.body(Body::empty())?,
    };
    let response = app.router.clone().oneshot(request).await?;
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

async fn issue_access_code(app: &TestApp, account_id: &str) -> Result<Value> {
    let (status, body) = request(
        app,
        "POST",
        "/api/admin/portal/access-codes",
        Some(ADMIN_TOKEN),
        Some(json!({ "account_id": account_id })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "issuing access code failed: {body}");
    Ok(body)
}

async fn login(app: &TestApp, access_code: &str) -> Result<(StatusCode, Value)> {
    request(
        app,
        "POST",
        "/api/portal/login",
        None,
        Some(json!({ "access_code": access_code })),
    )
    .await
}

#[tokio::test]
async fn login_with_valid_access_code_creates_session() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1", "active").await?;
    let issued = issue_access_code(&app, "acct-1").await?;
    let access_code = issued["access_code"].as_str().unwrap().to_string();

    let (status, body) = login(&app, &access_code).await?;

    assert_eq!(status, StatusCode::OK, "login failed: {body}");
    assert_eq!(body["account_id"], "acct-1");
    assert_eq!(body["org_id"], "org-1");
    assert_eq!(body["party_type"], "farmer");
    let token = body["token"].as_str().unwrap();
    assert!(!token.is_empty());
    assert!(body["expires_at"].as_str().is_some());

    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT account_id, token_hash FROM portal_sessions WHERE account_id = 'acct-1'",
    )
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(
        row.1,
        hash_token(token),
        "session must store the token hash"
    );
    Ok(())
}

#[tokio::test]
async fn login_with_unknown_code_is_unauthorized() -> Result<()> {
    let app = test_app().await?;
    let (status, _) = login(&app, "agb-doesnotexist").await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn revoked_or_expired_access_code_is_unauthorized() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1", "active").await?;

    // Revoked code.
    let issued = issue_access_code(&app, "acct-1").await?;
    let code_id = issued["code_id"].as_str().unwrap().to_string();
    let access_code = issued["access_code"].as_str().unwrap().to_string();
    let (status, _) = request(
        &app,
        "POST",
        &format!("/api/admin/portal/access-codes/{code_id}/revoke"),
        Some(ADMIN_TOKEN),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = login(&app, &access_code).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "revoked code must fail");

    // Expired code.
    let (status, issued) = request(
        &app,
        "POST",
        "/api/admin/portal/access-codes",
        Some(ADMIN_TOKEN),
        Some(json!({
            "account_id": "acct-1",
            "expires_at": "2020-01-01T00:00:00Z"
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let expired_code = issued["access_code"].as_str().unwrap().to_string();
    let (status, _) = login(&app, &expired_code).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "expired code must fail");
    Ok(())
}

#[tokio::test]
async fn inactive_account_cannot_login() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1", "active").await?;
    let issued = issue_access_code(&app, "acct-1").await?;
    let access_code = issued["access_code"].as_str().unwrap().to_string();

    sqlx::query("UPDATE marketplace_accounts SET status = 'suspended' WHERE account_id = 'acct-1'")
        .execute(&app.pool)
        .await?;

    let (status, _) = login(&app, &access_code).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn me_returns_account_and_org_for_bearer_token() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1", "active").await?;
    sqlx::query(
        r#"
        INSERT INTO farms (farm_id, owner, name, created_at)
        VALUES ('farm-1', 'org-1', 'North Farm', '2026-07-01T00:00:00Z')
        "#,
    )
    .execute(&app.pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO fields (field_id, farm_id, owner, name, boundary_json, created_at)
        VALUES ('field-1', 'farm-1', 'org-1', 'North Field', '{}', '2026-07-01T00:00:00Z')
        "#,
    )
    .execute(&app.pool)
    .await?;

    let issued = issue_access_code(&app, "acct-1").await?;
    let access_code = issued["access_code"].as_str().unwrap().to_string();
    let (_, session) = login(&app, &access_code).await?;
    let token = session["token"].as_str().unwrap().to_string();

    let (status, body) = request(&app, "GET", "/api/portal/me", Some(&token), None).await?;

    assert_eq!(status, StatusCode::OK, "me failed: {body}");
    assert_eq!(body["account_id"], "acct-1");
    assert_eq!(body["org_id"], "org-1");
    assert_eq!(body["party_type"], "farmer");
    assert_eq!(body["farm_count"], 1);
    assert_eq!(body["field_count"], 1);
    Ok(())
}

#[tokio::test]
async fn expired_session_is_unauthorized() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1", "active").await?;

    let token = "expired-session-token";
    sqlx::query(
        r#"
        INSERT INTO portal_sessions
            (session_id, token_hash, account_id, org_id,
             created_at, expires_at, last_seen_at)
        VALUES ('sess-1', ?1, 'acct-1', 'org-1',
                '2026-01-01T00:00:00Z', '2026-02-01T00:00:00Z', '2026-01-01T00:00:00Z')
        "#,
    )
    .bind(hash_token(token))
    .execute(&app.pool)
    .await?;

    let (status, _) = request(&app, "GET", "/api/portal/me", Some(token), None).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn logout_revokes_session() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1", "active").await?;
    let issued = issue_access_code(&app, "acct-1").await?;
    let access_code = issued["access_code"].as_str().unwrap().to_string();
    let (_, session) = login(&app, &access_code).await?;
    let token = session["token"].as_str().unwrap().to_string();

    let (status, _) = request(&app, "GET", "/api/portal/me", Some(&token), None).await?;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = request(&app, "POST", "/api/portal/logout", Some(&token), None).await?;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = request(&app, "GET", "/api/portal/me", Some(&token), None).await?;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "revoked session must fail"
    );
    Ok(())
}

#[tokio::test]
async fn issued_access_code_is_stored_hashed_and_returned_once() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1", "active").await?;
    let issued = issue_access_code(&app, "acct-1").await?;
    let access_code = issued["access_code"].as_str().unwrap().to_string();
    assert!(access_code.starts_with("agb-"));

    let (code_hash,): (String,) =
        sqlx::query_as("SELECT code_hash FROM portal_access_codes WHERE account_id = 'acct-1'")
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(code_hash.len(), 64, "code_hash must be sha256 hex");
    assert!(code_hash
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_ne!(code_hash, access_code, "plaintext must never be stored");
    assert_eq!(code_hash, hash_token(&access_code));

    // The list route masks: no hash, no plaintext.
    let (status, listing) = request(
        &app,
        "GET",
        "/api/admin/portal/access-codes?account_id=acct-1",
        Some(ADMIN_TOKEN),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let listing_text = listing.to_string();
    assert!(
        !listing_text.contains(&code_hash),
        "list must not expose hashes"
    );
    assert!(
        !listing_text.contains(&access_code),
        "list must not expose plaintext"
    );
    assert_eq!(listing[0]["code_id"], issued["code_id"]);
    Ok(())
}

#[tokio::test]
async fn mint_route_rejects_missing_or_wrong_admin_token() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1", "active").await?;

    // No bearer token at all -> 401.
    let (status, _) = request(
        &app,
        "POST",
        "/api/admin/portal/access-codes",
        None,
        Some(json!({ "account_id": "acct-1" })),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "missing token must be 401"
    );

    // Wrong token -> 401.
    let (status, _) = request(
        &app,
        "POST",
        "/api/admin/portal/access-codes",
        Some("not-the-admin-token"),
        Some(json!({ "account_id": "acct-1" })),
    )
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "wrong token must be 401");

    // No code was minted.
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM portal_access_codes WHERE account_id = 'acct-1'")
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(count, 0, "unauthorized calls must not mint codes");
    Ok(())
}

#[tokio::test]
async fn admin_api_is_disabled_when_no_token_configured() -> Result<()> {
    // Fail-closed: with no admin token configured the admin API is off (403),
    // even for a caller presenting some bearer token.
    let app = test_app_with_admin_token(None).await?;
    seed_account(&app.pool, "acct-1", "org-1", "active").await?;

    let (status, _) = request(
        &app,
        "POST",
        "/api/admin/portal/access-codes",
        Some("anything"),
        Some(json!({ "account_id": "acct-1" })),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "admin API must be disabled without a configured token"
    );

    // List and revoke are gated the same way.
    let (status, _) = request(
        &app,
        "GET",
        "/api/admin/portal/access-codes",
        Some("anything"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    Ok(())
}
