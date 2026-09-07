//! Integration tests for session-scoped portal reads (batch F-B2):
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

async fn seed_alert(pool: &DbPool, alert_id: &str, field_id: &str, fired_at: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO fired_alerts
            (alert_id, matched_rule_id, source_finding_id, field_id, event_type,
             subject_ref, severity, fired_at)
        VALUES (?1, 'rule-1', 'finding-1', ?2, 'ndvi_drop', ?2, 'high', ?3)
        "#,
    )
    .bind(alert_id)
    .bind(field_id)
    .bind(fired_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Two orgs, each with farms/fields; the caller sees only its own.
#[tokio::test]
async fn portal_farms_lists_only_my_org_farms() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;

    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_farm(&app.pool, "farm-2", "org-1", "South Farm").await?;
    seed_farm(&app.pool, "farm-x", "org-2", "Other Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;
    seed_field(&app.pool, "field-2", "farm-1", "org-1", "Field B").await?;
    seed_field(&app.pool, "field-x", "farm-x", "org-2", "Other Field").await?;

    let (status, body) = request(&app, "GET", "/api/portal/farms", Some(&token)).await?;

    assert_eq!(status, StatusCode::OK, "farms failed: {body}");
    let farms = body.as_array().expect("farms array");
    assert_eq!(farms.len(), 2, "only org-1 farms: {body}");
    let ids: Vec<&str> = farms
        .iter()
        .map(|farm| farm["farm_id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"farm-1"));
    assert!(ids.contains(&"farm-2"));
    assert!(!body.to_string().contains("farm-x"));
    let farm_1 = farms
        .iter()
        .find(|farm| farm["farm_id"] == "farm-1")
        .unwrap();
    assert_eq!(farm_1["field_count"], 2);
    let farm_2 = farms
        .iter()
        .find(|farm| farm["farm_id"] == "farm-2")
        .unwrap();
    assert_eq!(farm_2["field_count"], 0);
    Ok(())
}

/// Field cards must carry exact badge counts derived from findings,
/// recommendations, scenes, and fired alerts.
#[tokio::test]
async fn portal_fields_returns_cards_with_badge_counts() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;

    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;
    // Other org: must never surface in cards.
    seed_farm(&app.pool, "farm-x", "org-2", "Other Farm").await?;
    seed_field(&app.pool, "field-x", "farm-x", "org-2", "Other Field").await?;

    // Findings: latest (by created_at) has severity "high".
    seed_finding(
        &app.pool,
        "finding-1",
        "field-1",
        "medium",
        "2026-07-01T00:00:00Z",
    )
    .await?;
    seed_finding(
        &app.pool,
        "finding-2",
        "field-1",
        "high",
        "2026-07-03T00:00:00Z",
    )
    .await?;
    seed_finding(
        &app.pool,
        "finding-x",
        "field-x",
        "critical",
        "2026-07-04T00:00:00Z",
    )
    .await?;

    // Recommendations: two open, one dismissed.
    seed_recommendation(&app.pool, "rec-1", "field-1", "Scout NW", "high", "open").await?;
    seed_recommendation(&app.pool, "rec-2", "field-1", "Irrigate", "medium", "open").await?;
    seed_recommendation(&app.pool, "rec-3", "field-1", "Old", "low", "dismissed").await?;

    // Scenes: latest acquired_at wins.
    seed_scene(&app.pool, "scene-1", "field-1", "2026-06-20T10:00:00Z").await?;
    seed_scene(&app.pool, "scene-2", "field-1", "2026-07-01T10:00:00Z").await?;

    // Alerts: two inside the 7-day window, one outside.
    let now = Utc::now();
    let recent_1 = (now - Duration::days(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let recent_2 = (now - Duration::days(6)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let old = (now - Duration::days(30)).to_rfc3339_opts(SecondsFormat::Secs, true);
    seed_alert(&app.pool, "alert-1", "field-1", &recent_1).await?;
    seed_alert(&app.pool, "alert-2", "field-1", &recent_2).await?;
    seed_alert(&app.pool, "alert-3", "field-1", &old).await?;

    let (status, body) = request(&app, "GET", "/api/portal/fields", Some(&token)).await?;

    assert_eq!(status, StatusCode::OK, "fields failed: {body}");
    let cards = body.as_array().expect("cards array");
    assert_eq!(cards.len(), 1, "only org-1 fields: {body}");
    let card = &cards[0];
    assert_eq!(card["field_id"], "field-1");
    assert_eq!(card["farm_id"], "farm-1");
    assert_eq!(card["name"], "Field A");
    assert_eq!(card["crop"], "corn");
    assert_eq!(card["season"], "2026");
    assert_eq!(card["latest_finding_severity"], "high");
    assert_eq!(card["open_recommendations"], 2);
    assert_eq!(card["latest_scene_at"], "2026-07-01T10:00:00Z");
    assert_eq!(card["recent_alerts_7d"], 2);
    Ok(())
}

/// The overview aggregates findings by severity, ranks open recommendations
/// by priority, and reports the latest linked scene.
#[tokio::test]
async fn portal_field_overview_aggregates_findings_recommendations_and_latest_scene() -> Result<()>
{
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;

    seed_finding(
        &app.pool,
        "finding-1",
        "field-1",
        "high",
        "2026-07-01T00:00:00Z",
    )
    .await?;
    seed_finding(
        &app.pool,
        "finding-2",
        "field-1",
        "high",
        "2026-07-02T00:00:00Z",
    )
    .await?;
    seed_finding(
        &app.pool,
        "finding-3",
        "field-1",
        "low",
        "2026-07-03T00:00:00Z",
    )
    .await?;

    // Six open recommendations across priorities: top 5 must be priority-ordered
    // critical > high > medium > low, count must still be 6.
    seed_recommendation(&app.pool, "rec-low-1", "field-1", "Low 1", "low", "open").await?;
    seed_recommendation(&app.pool, "rec-low-2", "field-1", "Low 2", "low", "open").await?;
    seed_recommendation(&app.pool, "rec-med", "field-1", "Medium", "medium", "open").await?;
    seed_recommendation(&app.pool, "rec-high", "field-1", "High", "high", "open").await?;
    seed_recommendation(
        &app.pool, "rec-crit", "field-1", "Critical", "critical", "open",
    )
    .await?;
    seed_recommendation(&app.pool, "rec-low-3", "field-1", "Low 3", "low", "open").await?;
    seed_recommendation(
        &app.pool,
        "rec-closed",
        "field-1",
        "Closed",
        "critical",
        "closed",
    )
    .await?;

    seed_scene(&app.pool, "scene-1", "field-1", "2026-06-20T10:00:00Z").await?;
    seed_scene(&app.pool, "scene-2", "field-1", "2026-07-01T10:00:00Z").await?;

    let now = Utc::now();
    let recent = (now - Duration::days(2)).to_rfc3339_opts(SecondsFormat::Secs, true);
    seed_alert(&app.pool, "alert-1", "field-1", &recent).await?;

    let (status, body) = request(
        &app,
        "GET",
        "/api/portal/fields/field-1/overview",
        Some(&token),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "overview failed: {body}");
    assert_eq!(body["field"]["field_id"], "field-1");
    assert_eq!(body["field"]["name"], "Field A");
    assert_eq!(body["findings_by_severity"]["high"], 2);
    assert_eq!(body["findings_by_severity"]["low"], 1);
    assert_eq!(body["recent_findings"].as_array().unwrap().len(), 3);
    // Latest finding first.
    assert_eq!(body["recent_findings"][0]["finding_id"], "finding-3");
    assert_eq!(body["open_recommendation_count"], 6);
    let open = body["open_recommendations"].as_array().unwrap();
    assert_eq!(open.len(), 5, "top 5 only: {body}");
    assert_eq!(open[0]["priority"], "critical");
    assert_eq!(open[0]["recommendation_id"], "rec-crit");
    assert_eq!(open[1]["priority"], "high");
    assert_eq!(open[2]["priority"], "medium");
    assert_eq!(open[3]["priority"], "low");
    assert_eq!(open[4]["priority"], "low");
    assert_eq!(body["latest_scene"]["scene_id"], "scene-2");
    assert_eq!(body["latest_scene"]["acquired_at"], "2026-07-01T10:00:00Z");
    assert_eq!(body["recent_alert_count"], 1);
    Ok(())
}

/// A field belonging to another org must look nonexistent (404, never 403).
#[tokio::test]
async fn portal_field_overview_cross_org_is_not_found() -> Result<()> {
    let app = test_app().await?;
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-x", "org-2", "Other Farm").await?;
    seed_field(&app.pool, "field-x", "farm-x", "org-2", "Other Field").await?;

    let (status, _) = request(
        &app,
        "GET",
        "/api/portal/fields/field-x/overview",
        Some(&token),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-org must be 404");

    let (status, _) = request(
        &app,
        "GET",
        "/api/portal/fields/field-missing/overview",
        Some(&token),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "missing field must be indistinguishable from cross-org"
    );
    Ok(())
}

#[tokio::test]
async fn portal_routes_without_token_are_unauthorized() -> Result<()> {
    let app = test_app().await?;
    for uri in [
        "/api/portal/farms",
        "/api/portal/fields",
        "/api/portal/fields/field-1/overview",
    ] {
        let (status, _) = request(&app, "GET", uri, None).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri} must require auth");
    }
    Ok(())
}
