//! Integration tests for the farm activity log (batch F-B5):
//! `POST/GET /api/portal/fields/:field_id/activities`,
//! `PUT/DELETE /api/portal/activities/:activity_id`,
//! `GET /api/portal/fields/:field_id/activities/summary`, and the
//! `log_activity` companion on the recommendation status transition.
//! Seeds two orgs to prove tenancy.

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
        VALUES (?1, 'scene-any', ?2, 'Scout the NW zone', 'high', ?3,
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

/// One org, one field, one authenticated session.
async fn seed_org_with_field(app: &TestApp) -> Result<String> {
    seed_account(&app.pool, "acct-1", "org-1").await?;
    let token = seed_session(&app.pool, "acct-1", "org-1").await?;
    seed_farm(&app.pool, "farm-1", "org-1", "North Farm").await?;
    seed_field(&app.pool, "field-1", "farm-1", "org-1", "Field A").await?;
    Ok(token)
}

async fn create_activity(app: &TestApp, token: &str, body: Value) -> Result<(StatusCode, Value)> {
    request_json(
        app,
        "POST",
        "/api/portal/fields/field-1/activities",
        Some(token),
        Some(body),
    )
    .await
}

#[tokio::test]
async fn create_activity_persists_and_lists_in_date_order() -> Result<()> {
    let app = test_app().await?;
    let token = seed_org_with_field(&app).await?;

    // Date-only occurrence must normalize to midnight UTC.
    let (status, body) = create_activity(
        &app,
        &token,
        json!({"activity_type": "planting", "occurred_at": "2026-05-01",
               "note": "  drilled corn  "}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "create failed: {body}");
    assert_eq!(body["occurred_at"], "2026-05-01T00:00:00Z");
    assert_eq!(body["note"], "drilled corn", "note must be trimmed");
    assert_eq!(body["source"], "manual");
    assert_eq!(body["created_by"], "acct-1");
    assert!(body["activity_id"]
        .as_str()
        .unwrap()
        .starts_with("activity-"));

    let (status, body) = create_activity(
        &app,
        &token,
        json!({"activity_type": "irrigation", "occurred_at": "2026-05-20T06:30:00Z",
               "quantity": 12.5, "unit": "mm", "cost": 40.0}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "create failed: {body}");

    let (status, body) = create_activity(
        &app,
        &token,
        json!({"activity_type": "scouting", "occurred_at": "2026-05-10T09:00:00Z",
               "geometry_json": "{\"type\": \"Point\", \"coordinates\": [1.0, 2.0]}"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "create failed: {body}");

    let (status, body) = request_json(
        &app,
        "GET",
        "/api/portal/fields/field-1/activities",
        Some(&token),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "list failed: {body}");
    assert_eq!(body["total"], 3);
    assert_eq!(body["page"], 1);
    assert_eq!(body["page_size"], 50);
    let activities = body["activities"].as_array().expect("activities array");
    assert_eq!(activities.len(), 3);
    let occurred: Vec<&str> = activities
        .iter()
        .map(|activity| activity["occurred_at"].as_str().unwrap())
        .collect();
    assert_eq!(
        occurred,
        vec![
            "2026-05-20T06:30:00Z",
            "2026-05-10T09:00:00Z",
            "2026-05-01T00:00:00Z"
        ],
        "newest occurrence first"
    );

    // Filters: window and activity_type.
    let (status, body) = request_json(
        &app,
        "GET",
        "/api/portal/fields/field-1/activities?from=2026-05-05&to=2026-05-15",
        Some(&token),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "filtered list failed: {body}");
    assert_eq!(body["total"], 1);
    assert_eq!(body["activities"][0]["activity_type"], "scouting");

    let (status, body) = request_json(
        &app,
        "GET",
        "/api/portal/fields/field-1/activities?activity_type=irrigation",
        Some(&token),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "typed list failed: {body}");
    assert_eq!(body["total"], 1);
    assert_eq!(body["activities"][0]["quantity"], 12.5);
    assert_eq!(body["activities"][0]["unit"], "mm");

    // Pagination: page_size=2 splits 3 rows into 2 + 1.
    let (status, body) = request_json(
        &app,
        "GET",
        "/api/portal/fields/field-1/activities?page=2&page_size=2",
        Some(&token),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "paged list failed: {body}");
    assert_eq!(body["total"], 3);
    assert_eq!(body["activities"].as_array().unwrap().len(), 1);
    assert_eq!(body["activities"][0]["occurred_at"], "2026-05-01T00:00:00Z");
    Ok(())
}

#[tokio::test]
async fn activity_validation_rejects_bad_type_and_negative_quantity() -> Result<()> {
    let app = test_app().await?;
    let token = seed_org_with_field(&app).await?;

    let (status, body) = create_activity(
        &app,
        &token,
        json!({"activity_type": "mowing", "occurred_at": "2026-05-01"}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    let reason = body.as_str().expect("reason string");
    assert!(
        reason.contains("invalid activity_type") && reason.contains("mowing"),
        "reason must name the bad type: {reason}"
    );

    let (status, body) = create_activity(
        &app,
        &token,
        json!({"activity_type": "irrigation", "occurred_at": "2026-05-01",
               "quantity": -4.0, "unit": "mm"}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    let reason = body.as_str().expect("reason string");
    assert!(
        reason.contains("quantity must be >= 0"),
        "reason must explain the quantity rule: {reason}"
    );

    let (status, body) = create_activity(
        &app,
        &token,
        json!({"activity_type": "irrigation", "occurred_at": "2026-05-01",
               "quantity": 4.0}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert!(
        body.as_str().expect("reason string").contains("unit"),
        "quantity without unit must name the unit rule: {body}"
    );

    let (status, body) = create_activity(
        &app,
        &token,
        json!({"activity_type": "scouting", "occurred_at": "last tuesday"}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert!(
        body.as_str()
            .expect("reason string")
            .contains("occurred_at"),
        "bad timestamp must name occurred_at: {body}"
    );

    // Nothing was persisted.
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM field_activities")
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(count, 0, "rejected drafts must not persist");
    Ok(())
}

#[tokio::test]
async fn activity_update_and_delete_are_org_scoped() -> Result<()> {
    let app = test_app().await?;
    let token = seed_org_with_field(&app).await?;

    let (status, created) = create_activity(
        &app,
        &token,
        json!({"activity_type": "fertilizing", "occurred_at": "2026-05-04",
               "quantity": 100.0, "unit": "kg", "cost": 250.0}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "create failed: {created}");
    let activity_id = created["activity_id"].as_str().unwrap().to_string();

    // Patch changes only the provided fields.
    let (status, updated) = request_json(
        &app,
        "PUT",
        &format!("/api/portal/activities/{activity_id}"),
        Some(&token),
        Some(json!({"quantity": 120.0, "note": "topped up N"})),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "update failed: {updated}");
    assert_eq!(updated["quantity"], 120.0);
    assert_eq!(updated["unit"], "kg", "unit untouched by patch");
    assert_eq!(updated["cost"], 250.0, "cost untouched by patch");
    assert_eq!(updated["note"], "topped up N");

    let stored = sqlx::query("SELECT quantity, note FROM field_activities WHERE activity_id = ?1")
        .bind(&activity_id)
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(stored.get::<f64, _>("quantity"), 120.0);
    assert_eq!(stored.get::<String, _>("note"), "topped up N");

    // Invalid patch is rejected and leaves the row alone.
    let (status, body) = request_json(
        &app,
        "PUT",
        &format!("/api/portal/activities/{activity_id}"),
        Some(&token),
        Some(json!({"cost": -1.0})),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");

    // Hard delete removes the row.
    let (status, body) = request_json(
        &app,
        "DELETE",
        &format!("/api/portal/activities/{activity_id}"),
        Some(&token),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "delete failed: {body}");
    assert_eq!(body["status"], "deleted");
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM field_activities WHERE activity_id = ?1")
            .bind(&activity_id)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(count, 0, "delete must be hard");

    // Deleting again is 404.
    let (status, _) = request_json(
        &app,
        "DELETE",
        &format!("/api/portal/activities/{activity_id}"),
        Some(&token),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn cross_org_activity_access_is_not_found() -> Result<()> {
    let app = test_app().await?;
    let token = seed_org_with_field(&app).await?;

    // Other org with its own field and activity.
    seed_account(&app.pool, "acct-2", "org-2").await?;
    let other_token = seed_session(&app.pool, "acct-2", "org-2").await?;
    seed_farm(&app.pool, "farm-x", "org-2", "Other Farm").await?;
    seed_field(&app.pool, "field-x", "farm-x", "org-2", "Other Field").await?;
    let (status, other) = request_json(
        &app,
        "POST",
        "/api/portal/fields/field-x/activities",
        Some(&other_token),
        Some(json!({"activity_type": "harvest", "occurred_at": "2026-06-01"})),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "other-org create failed: {other}");
    let other_id = other["activity_id"].as_str().unwrap().to_string();

    // org-1 cannot create on, list, or summarize the other org's field.
    let (status, _) = request_json(
        &app,
        "POST",
        "/api/portal/fields/field-x/activities",
        Some(&token),
        Some(json!({"activity_type": "harvest", "occurred_at": "2026-06-01"})),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-org create must 404");
    let (status, _) = request_json(
        &app,
        "GET",
        "/api/portal/fields/field-x/activities",
        Some(&token),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-org list must 404");
    let (status, _) = request_json(
        &app,
        "GET",
        "/api/portal/fields/field-x/activities/summary",
        Some(&token),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-org summary must 404");

    // Update/delete on the other org's activity are 404 and leave it intact.
    let (status, _) = request_json(
        &app,
        "PUT",
        &format!("/api/portal/activities/{other_id}"),
        Some(&token),
        Some(json!({"note": "hijack"})),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-org update must 404");
    let (status, _) = request_json(
        &app,
        "DELETE",
        &format!("/api/portal/activities/{other_id}"),
        Some(&token),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-org delete must 404");
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM field_activities WHERE activity_id = ?1")
            .bind(&other_id)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(count, 1, "other org's activity must survive");

    // Missing IDs behave identically to cross-org ones.
    let (status, _) = request_json(
        &app,
        "DELETE",
        "/api/portal/activities/activity-missing",
        Some(&token),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn season_summary_aggregates_totals_by_type() -> Result<()> {
    let app = test_app().await?;
    let token = seed_org_with_field(&app).await?;

    for body in [
        json!({"activity_type": "irrigation", "occurred_at": "2026-05-01",
               "quantity": 10.0, "unit": "mm", "cost": 40.0}),
        json!({"activity_type": "irrigation", "occurred_at": "2026-05-15",
               "quantity": 15.0, "unit": "mm", "cost": 60.0}),
        json!({"activity_type": "fertilizing", "occurred_at": "2026-05-05",
               "quantity": 50.0, "unit": "kg", "cost": 200.0}),
        json!({"activity_type": "fertilizing", "occurred_at": "2026-05-06",
               "quantity": 20.0, "unit": "l"}),
        json!({"activity_type": "scouting", "occurred_at": "2026-05-07"}),
        // Outside the summary window.
        json!({"activity_type": "harvest", "occurred_at": "2026-09-01",
               "quantity": 5.0, "unit": "t", "cost": 500.0}),
    ] {
        let (status, created) = create_activity(&app, &token, body).await?;
        assert_eq!(status, StatusCode::OK, "seed create failed: {created}");
    }

    let (status, body) = request_json(
        &app,
        "GET",
        "/api/portal/fields/field-1/activities/summary?from=2026-05-01&to=2026-05-31",
        Some(&token),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "summary failed: {body}");
    assert_eq!(body["total_count"], 5);
    assert_eq!(body["total_cost"], 300.0);
    let irrigation = &body["by_type"]["irrigation"];
    assert_eq!(irrigation["count"], 2);
    assert_eq!(irrigation["total_quantity"]["mm"], 25.0);
    assert_eq!(irrigation["total_cost"], 100.0);
    let fertilizing = &body["by_type"]["fertilizing"];
    assert_eq!(fertilizing["count"], 2);
    assert_eq!(fertilizing["total_quantity"]["kg"], 50.0);
    assert_eq!(fertilizing["total_quantity"]["l"], 20.0);
    assert_eq!(fertilizing["total_cost"], 200.0);
    let scouting = &body["by_type"]["scouting"];
    assert_eq!(scouting["count"], 1);
    assert_eq!(scouting["total_cost"], 0.0);
    assert!(
        body["by_type"].get("harvest").is_none(),
        "September harvest is outside the window: {body}"
    );

    // Unbounded summary includes everything.
    let (status, body) = request_json(
        &app,
        "GET",
        "/api/portal/fields/field-1/activities/summary",
        Some(&token),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "summary failed: {body}");
    assert_eq!(body["total_count"], 6);
    assert_eq!(body["total_cost"], 800.0);
    assert_eq!(body["by_type"]["harvest"]["total_quantity"]["t"], 5.0);
    Ok(())
}

#[tokio::test]
async fn completing_recommendation_with_log_activity_creates_linked_activity() -> Result<()> {
    let app = test_app().await?;
    let token = seed_org_with_field(&app).await?;
    seed_recommendation(&app.pool, "rec-1", "field-1", "open").await?;

    let (status, body) = request_json(
        &app,
        "PUT",
        "/api/portal/recommendations/rec-1/status",
        Some(&token),
        Some(json!({
            "status": "completed",
            "log_activity": {
                "activity_type": "spraying",
                "occurred_at": "2026-06-10",
                "quantity": 2.5,
                "unit": "l/ha",
                "cost": 80.0,
                "note": "spot-sprayed the NW zone"
            }
        })),
    )
    .await?;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["recommendation_id"], "rec-1");
    assert_eq!(body["status"], "completed");
    let activity_id = body["logged_activity_id"]
        .as_str()
        .expect("logged_activity_id present")
        .to_string();

    let row = sqlx::query(
        "SELECT field_id, org_id, activity_type, occurred_at, source, linked_ref, created_by \
         FROM field_activities WHERE activity_id = ?1",
    )
    .bind(&activity_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(row.get::<String, _>("field_id"), "field-1");
    assert_eq!(row.get::<String, _>("org_id"), "org-1");
    assert_eq!(row.get::<String, _>("activity_type"), "spraying");
    assert_eq!(row.get::<String, _>("occurred_at"), "2026-06-10T00:00:00Z");
    assert_eq!(row.get::<String, _>("source"), "recommendation");
    assert_eq!(row.get::<String, _>("linked_ref"), "rec-1");
    assert_eq!(row.get::<String, _>("created_by"), "acct-1");

    // The linked activity shows up in the field's log.
    let (status, listing) = request_json(
        &app,
        "GET",
        "/api/portal/fields/field-1/activities",
        Some(&token),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "list failed: {listing}");
    assert_eq!(listing["total"], 1);
    assert_eq!(listing["activities"][0]["activity_id"], activity_id);
    assert_eq!(listing["activities"][0]["source"], "recommendation");

    // A bad companion draft must reject the whole request and leave the
    // recommendation untouched.
    seed_recommendation(&app.pool, "rec-2", "field-1", "open").await?;
    let (status, body) = request_json(
        &app,
        "PUT",
        "/api/portal/recommendations/rec-2/status",
        Some(&token),
        Some(json!({
            "status": "completed",
            "log_activity": {"activity_type": "mowing", "occurred_at": "2026-06-10"}
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    let (stored_status,): (String,) =
        sqlx::query_as("SELECT status FROM recommendations WHERE recommendation_id = 'rec-2'")
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(stored_status, "open", "bad draft must not move the status");

    // A non-completed transition ignores log_activity (no extra rows).
    seed_recommendation(&app.pool, "rec-3", "field-1", "open").await?;
    let (status, body) = request_json(
        &app,
        "PUT",
        "/api/portal/recommendations/rec-3/status",
        Some(&token),
        Some(json!({
            "status": "reviewed",
            "log_activity": {"activity_type": "scouting", "occurred_at": "2026-06-11"}
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert!(body.get("logged_activity_id").is_none(), "body: {body}");
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM field_activities")
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(count, 1, "only the rec-1 companion exists");

    // Bare status body (pre-F-B5 caller) still works.
    seed_recommendation(&app.pool, "rec-4", "field-1", "open").await?;
    let (status, body) = request_json(
        &app,
        "PUT",
        "/api/portal/recommendations/rec-4/status",
        Some(&token),
        Some(json!({"status": "completed"})),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["status"], "completed");
    assert!(body.get("logged_activity_id").is_none(), "body: {body}");
    Ok(())
}

#[tokio::test]
async fn activity_routes_without_token_are_unauthorized() -> Result<()> {
    let app = test_app().await?;
    for (method, uri) in [
        ("GET", "/api/portal/fields/field-1/activities"),
        ("POST", "/api/portal/fields/field-1/activities"),
        ("GET", "/api/portal/fields/field-1/activities/summary"),
        ("PUT", "/api/portal/activities/activity-1"),
        ("DELETE", "/api/portal/activities/activity-1"),
    ] {
        let body = matches!(method, "POST" | "PUT")
            .then(|| json!({"activity_type": "scouting", "occurred_at": "2026-06-01"}));
        let (status, _) = request_json(&app, method, uri, None, body).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
    Ok(())
}
