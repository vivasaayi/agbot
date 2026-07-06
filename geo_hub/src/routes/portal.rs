//! Farmer-portal auth routes: access-code login, bearer sessions, identity
//! (`/api/portal/me`), and the admin access-code lifecycle.
//!
//! Domain rules (generation, hashing, validity predicates) live in
//! `crate::portal_auth`; these handlers stay thin over the SQLite tables
//! `portal_access_codes` / `portal_sessions` with `marketplace_accounts` as
//! the identity anchor (`status = 'active'` required).

use super::*;
use crate::error::{AppError, AppResult};
use crate::portal_auth::{
    access_code_is_usable, generate_access_code, generate_session_token, hash_token,
    parse_stored_timestamp, session_expires_at, session_is_valid, AccessCodeStatus, SessionStatus,
};
use crate::portal_overview::{
    build_field_card, build_field_overview, FieldCard, FieldInput, FieldOverview, FindingInput,
    RecommendationInput, SceneInput,
};
use crate::state::AppState;
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;

/// Delay applied before every login rejection so timing does not reveal
/// whether the code exists, is revoked, is expired, or belongs to an
/// inactive account.
const LOGIN_FAILURE_DELAY_MS: u64 = 250;

/// Authenticated portal caller, resolved from `Authorization: Bearer <token>`.
///
/// The token is hashed and matched against `portal_sessions`, joined to
/// `marketplace_accounts`; the session must be unrevoked and unexpired and
/// the account `active`, otherwise the request is rejected with 401.
#[derive(Debug, Clone)]
pub struct PortalIdentity {
    pub account_id: String,
    pub org_id: String,
    pub party_type: String,
    pub session_id: String,
}

#[axum::async_trait]
impl FromRequestParts<AppState> for PortalIdentity {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .ok_or(AppError::Unauthorized)?;

        let row = sqlx::query(
            r#"
            SELECT s.session_id, s.account_id, s.org_id, s.expires_at, s.revoked_at,
                   a.party_type, a.status
            FROM portal_sessions s
            JOIN marketplace_accounts a ON a.account_id = s.account_id
            WHERE s.token_hash = ?1
            "#,
        )
        .bind(hash_token(token))
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| AppError::Unauthorized)?
        .ok_or(AppError::Unauthorized)?;

        let session = SessionStatus {
            revoked_at: row
                .get::<Option<String>, _>("revoked_at")
                .as_deref()
                .and_then(parse_stored_timestamp),
            expires_at: parse_stored_timestamp(&row.get::<String, _>("expires_at"))
                .ok_or(AppError::Unauthorized)?,
        };
        let account_active = row.get::<String, _>("status") == "active";
        if !session_is_valid(&session, Utc::now()) || !account_active {
            return Err(AppError::Unauthorized);
        }

        let identity = PortalIdentity {
            account_id: row.get("account_id"),
            org_id: row.get("org_id"),
            party_type: row.get("party_type"),
            session_id: row.get("session_id"),
        };

        // Best-effort activity tracking; an update failure must not fail the
        // authenticated request.
        let _ = sqlx::query("UPDATE portal_sessions SET last_seen_at = ?1 WHERE session_id = ?2")
            .bind(current_record_timestamp())
            .bind(&identity.session_id)
            .execute(&state.pool)
            .await;

        Ok(identity)
    }
}

#[derive(Debug, Deserialize)]
pub struct PortalLoginRequest {
    pub access_code: String,
}

#[derive(Debug, Serialize)]
pub struct PortalLoginResponse {
    pub token: String,
    pub account_id: String,
    pub org_id: String,
    pub party_type: String,
    pub expires_at: String,
}

/// POST /api/portal/login — exchange an access code for a bearer session.
/// Every failure path (unknown, revoked, expired, inactive account, storage
/// error) is collapsed into a delayed 401 so responses are indistinguishable.
pub async fn portal_login(
    State(state): State<AppState>,
    Json(request): Json<PortalLoginRequest>,
) -> AppResult<Json<PortalLoginResponse>> {
    match try_portal_login(&state, &request.access_code).await {
        Some(response) => Ok(Json(response)),
        None => {
            tokio::time::sleep(std::time::Duration::from_millis(LOGIN_FAILURE_DELAY_MS)).await;
            Err(AppError::Unauthorized)
        }
    }
}

async fn try_portal_login(state: &AppState, access_code: &str) -> Option<PortalLoginResponse> {
    let row = sqlx::query(
        r#"
        SELECT c.code_id, c.account_id, c.org_id, c.expires_at, c.revoked_at,
               a.party_type, a.status
        FROM portal_access_codes c
        JOIN marketplace_accounts a ON a.account_id = c.account_id
        WHERE c.code_hash = ?1
        "#,
    )
    .bind(hash_token(access_code.trim()))
    .fetch_optional(&state.pool)
    .await
    .ok()??;

    let now = Utc::now();
    let code = AccessCodeStatus {
        revoked_at: row
            .get::<Option<String>, _>("revoked_at")
            .as_deref()
            .and_then(parse_stored_timestamp),
        expires_at: row
            .get::<Option<String>, _>("expires_at")
            .as_deref()
            .and_then(parse_stored_timestamp),
    };
    if !access_code_is_usable(&code, now) || row.get::<String, _>("status") != "active" {
        return None;
    }

    let token = generate_session_token();
    let created_at = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let expires_at = session_expires_at(now).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let account_id: String = row.get("account_id");
    let org_id: String = row.get("org_id");
    let party_type: String = row.get("party_type");

    sqlx::query(
        r#"
        INSERT INTO portal_sessions
            (session_id, token_hash, account_id, org_id,
             created_at, expires_at, last_seen_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?5)
        "#,
    )
    .bind(format!("portal-session-{}", Uuid::new_v4()))
    .bind(hash_token(&token))
    .bind(&account_id)
    .bind(&org_id)
    .bind(&created_at)
    .bind(&expires_at)
    .execute(&state.pool)
    .await
    .ok()?;

    // Best-effort usage tracking on the access code.
    let _ = sqlx::query("UPDATE portal_access_codes SET last_used_at = ?1 WHERE code_id = ?2")
        .bind(&created_at)
        .bind(row.get::<String, _>("code_id"))
        .execute(&state.pool)
        .await;

    Some(PortalLoginResponse {
        token,
        account_id,
        org_id,
        party_type,
        expires_at,
    })
}

#[derive(Debug, Serialize)]
pub struct PortalLogoutResponse {
    pub session_id: String,
    pub status: String,
}

/// POST /api/portal/logout — revoke the caller's own session.
pub async fn portal_logout(
    identity: PortalIdentity,
    State(state): State<AppState>,
) -> AppResult<Json<PortalLogoutResponse>> {
    sqlx::query(
        "UPDATE portal_sessions SET revoked_at = ?1 WHERE session_id = ?2 AND revoked_at IS NULL",
    )
    .bind(current_record_timestamp())
    .bind(&identity.session_id)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(PortalLogoutResponse {
        session_id: identity.session_id,
        status: "revoked".to_string(),
    }))
}

#[derive(Debug, Serialize)]
pub struct PortalMeResponse {
    pub account_id: String,
    pub org_id: String,
    pub party_type: String,
    pub farm_count: i64,
    pub field_count: i64,
}

/// GET /api/portal/me — the caller's identity plus org-scoped farm/field
/// counts (farms and fields are owned by the org via their `owner` column).
pub async fn portal_me(
    identity: PortalIdentity,
    State(state): State<AppState>,
) -> AppResult<Json<PortalMeResponse>> {
    let (farm_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM farms WHERE owner = ?1")
        .bind(&identity.org_id)
        .fetch_one(&state.pool)
        .await
        .map_err(Error::from)?;
    let (field_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM fields WHERE owner = ?1")
        .bind(&identity.org_id)
        .fetch_one(&state.pool)
        .await
        .map_err(Error::from)?;

    Ok(Json(PortalMeResponse {
        account_id: identity.account_id,
        org_id: identity.org_id,
        party_type: identity.party_type,
        farm_count,
        field_count,
    }))
}

// ---------------------------------------------------------------------------
// Admin access-code lifecycle.
//
// v1 intentionally ships these without authentication: the whole geo_hub API
// is deployed on a trusted network today and no operator auth layer exists
// yet. When one lands, these routes must be gated by it.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PortalAccessCodeIssueRequest {
    pub account_id: String,
    pub label: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PortalAccessCodeIssueResponse {
    pub code_id: String,
    /// Plaintext access code — returned exactly once; only its hash is stored.
    pub access_code: String,
    pub account_id: String,
    pub org_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// POST /api/admin/portal/access-codes — mint an access code for an active
/// marketplace account. The plaintext code appears only in this response.
pub async fn issue_portal_access_code(
    State(state): State<AppState>,
    Json(request): Json<PortalAccessCodeIssueRequest>,
) -> AppResult<Json<PortalAccessCodeIssueResponse>> {
    let account_id = normalize_optional_text(Some(request.account_id))
        .ok_or_else(|| AppError::BadRequest("account_id is required".to_string()))?;
    let expires_at = normalize_optional_text(request.expires_at);
    if let Some(expires_at) = expires_at.as_deref() {
        if parse_stored_timestamp(expires_at).is_none() {
            return Err(AppError::BadRequest(
                "expires_at must be an RFC 3339 timestamp".to_string(),
            ));
        }
    }

    let account =
        sqlx::query("SELECT org_id, status FROM marketplace_accounts WHERE account_id = ?1")
            .bind(&account_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(Error::from)?
            .ok_or(AppError::NotFound)?;
    if account.get::<String, _>("status") != "active" {
        return Err(AppError::BadRequest(format!(
            "account {account_id} is not active"
        )));
    }
    let org_id: String = account.get("org_id");

    let code_id = format!("portal-code-{}", Uuid::new_v4());
    let access_code = generate_access_code();
    sqlx::query(
        r#"
        INSERT INTO portal_access_codes
            (code_id, code_hash, account_id, org_id, label, created_at, expires_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&code_id)
    .bind(hash_token(&access_code))
    .bind(&account_id)
    .bind(&org_id)
    .bind(normalize_optional_text(request.label))
    .bind(current_record_timestamp())
    .bind(&expires_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(PortalAccessCodeIssueResponse {
        code_id,
        access_code,
        account_id,
        org_id,
        expires_at,
    }))
}

#[derive(Debug, Deserialize)]
pub struct PortalAccessCodeListQuery {
    pub account_id: Option<String>,
}

/// Masked view of an access code: never exposes the hash or plaintext.
#[derive(Debug, Serialize)]
pub struct PortalAccessCodeSummary {
    pub code_id: String,
    pub account_id: String,
    pub org_id: String,
    pub label: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
    pub last_used_at: Option<String>,
}

fn portal_access_code_summary(row: &sqlx::sqlite::SqliteRow) -> PortalAccessCodeSummary {
    PortalAccessCodeSummary {
        code_id: row.get("code_id"),
        account_id: row.get("account_id"),
        org_id: row.get("org_id"),
        label: row.get("label"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
        last_used_at: row.get("last_used_at"),
    }
}

/// GET /api/admin/portal/access-codes?account_id= — masked listing.
pub async fn list_portal_access_codes(
    Query(query): Query<PortalAccessCodeListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<PortalAccessCodeSummary>>> {
    let account_id = normalize_optional_text(query.account_id);
    let rows = sqlx::query(
        r#"
        SELECT code_id, account_id, org_id, label,
               created_at, expires_at, revoked_at, last_used_at
        FROM portal_access_codes
        WHERE (?1 IS NULL OR account_id = ?1)
        ORDER BY created_at ASC, code_id ASC
        "#,
    )
    .bind(account_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(rows.iter().map(portal_access_code_summary).collect()))
}

/// POST /api/admin/portal/access-codes/:code_id/revoke — revoke a code so it
/// can no longer be exchanged for sessions (existing sessions are unaffected).
pub async fn revoke_portal_access_code(
    Path(code_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<PortalAccessCodeSummary>> {
    sqlx::query(
        "UPDATE portal_access_codes SET revoked_at = ?1 WHERE code_id = ?2 AND revoked_at IS NULL",
    )
    .bind(current_record_timestamp())
    .bind(&code_id)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    let row = sqlx::query(
        r#"
        SELECT code_id, account_id, org_id, label,
               created_at, expires_at, revoked_at, last_used_at
        FROM portal_access_codes
        WHERE code_id = ?1
        "#,
    )
    .bind(&code_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?
    .ok_or(AppError::NotFound)?;

    Ok(Json(portal_access_code_summary(&row)))
}

// ---------------------------------------------------------------------------
// Session-scoped portal reads (batch F-B2): farms, field cards, and the
// per-field overview. Aggregation rules live in `crate::portal_overview`;
// these handlers only run org-scoped queries and delegate assembly.
// ---------------------------------------------------------------------------

/// A farm owned by the caller's org, as loaded by [`owned_farm`].
#[derive(Debug, Clone, Serialize)]
pub struct PortalFarm {
    pub farm_id: String,
    pub name: String,
    pub notes: Option<String>,
    pub status: String,
}

fn portal_farm_from_row(row: &sqlx::sqlite::SqliteRow) -> PortalFarm {
    PortalFarm {
        farm_id: row.get("farm_id"),
        name: row.get("name"),
        notes: row.get("notes"),
        status: row.get("status"),
    }
}

fn field_input_from_row(row: &sqlx::sqlite::SqliteRow) -> FieldInput {
    FieldInput {
        field_id: row.get("field_id"),
        farm_id: row.get("farm_id"),
        name: row.get("name"),
        crop: row.get("crop"),
        season: row.get("season"),
    }
}

/// Load a field only if it belongs to `org_id`. A field owned by another org
/// is reported as [`AppError::NotFound`] — never `Forbidden` — so the route
/// does not leak which field IDs exist.
pub async fn owned_field(state: &AppState, org_id: &str, field_id: &str) -> AppResult<FieldInput> {
    let row = sqlx::query(
        "SELECT field_id, farm_id, name, crop, season FROM fields \
         WHERE field_id = ?1 AND owner = ?2",
    )
    .bind(field_id)
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?
    .ok_or(AppError::NotFound)?;
    Ok(field_input_from_row(&row))
}

/// Load a farm only if it belongs to `org_id`; cross-org access is
/// indistinguishable from a missing farm (404, no existence leak).
pub async fn owned_farm(state: &AppState, org_id: &str, farm_id: &str) -> AppResult<PortalFarm> {
    let row = sqlx::query(
        "SELECT farm_id, name, notes, status FROM farms WHERE farm_id = ?1 AND owner = ?2",
    )
    .bind(farm_id)
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?
    .ok_or(AppError::NotFound)?;
    Ok(portal_farm_from_row(&row))
}

#[derive(Debug, Serialize)]
pub struct PortalFarmSummary {
    #[serde(flatten)]
    pub farm: PortalFarm,
    pub field_count: i64,
}

/// GET /api/portal/farms — the caller's org farms with per-farm field counts.
pub async fn portal_list_farms(
    identity: PortalIdentity,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<PortalFarmSummary>>> {
    let rows = sqlx::query(
        r#"
        SELECT f.farm_id, f.name, f.notes, f.status,
               (SELECT COUNT(*) FROM fields fl
                WHERE fl.farm_id = f.farm_id AND fl.owner = ?1) AS field_count
        FROM farms f
        WHERE f.owner = ?1
        ORDER BY f.name ASC, f.farm_id ASC
        "#,
    )
    .bind(&identity.org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(
        rows.iter()
            .map(|row| PortalFarmSummary {
                farm: portal_farm_from_row(row),
                field_count: row.get("field_count"),
            })
            .collect(),
    ))
}

async fn field_findings(state: &AppState, field_id: &str) -> AppResult<Vec<FindingInput>> {
    let rows = sqlx::query(
        "SELECT finding_id, kind, severity, created_at FROM application_findings \
         WHERE field_id = ?1 ORDER BY created_at DESC, finding_id ASC",
    )
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;
    Ok(rows
        .iter()
        .map(|row| FindingInput {
            finding_id: row.get("finding_id"),
            kind: row.get("kind"),
            severity: row.get("severity"),
            created_at: row.get("created_at"),
        })
        .collect())
}

async fn field_recommendations(
    state: &AppState,
    field_id: &str,
) -> AppResult<Vec<RecommendationInput>> {
    let rows = sqlx::query(
        "SELECT recommendation_id, title, category, priority, status, created_at \
         FROM recommendations WHERE field_id = ?1 \
         ORDER BY created_at ASC, recommendation_id ASC",
    )
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;
    Ok(rows
        .iter()
        .map(|row| RecommendationInput {
            recommendation_id: row.get("recommendation_id"),
            title: row.get("title"),
            category: row.get("category"),
            priority: row.get("priority"),
            status: row.get("status"),
            created_at: row.get("created_at"),
        })
        .collect())
}

async fn field_latest_scene(state: &AppState, field_id: &str) -> AppResult<Option<SceneInput>> {
    let row = sqlx::query(
        "SELECT scene_id, sensor, acquired_at FROM scenes \
         WHERE field_id = ?1 ORDER BY acquired_at DESC, scene_id ASC LIMIT 1",
    )
    .bind(field_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;
    Ok(row.map(|row| SceneInput {
        scene_id: row.get("scene_id"),
        sensor: row.get("sensor"),
        acquired_at: row.get("acquired_at"),
    }))
}

async fn field_alert_fired_ats(state: &AppState, field_id: &str) -> AppResult<Vec<String>> {
    let rows =
        sqlx::query_as::<_, (String,)>("SELECT fired_at FROM fired_alerts WHERE field_id = ?1")
            .bind(field_id)
            .fetch_all(&state.pool)
            .await
            .map_err(Error::from)?;
    Ok(rows.into_iter().map(|(fired_at,)| fired_at).collect())
}

/// GET /api/portal/fields — one dashboard card per field in the caller's org.
/// Per-field queries run in a simple loop: farms hold tens of fields and the
/// store is local SQLite, so bounded sequential reads beat query complexity.
pub async fn portal_list_fields(
    identity: PortalIdentity,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FieldCard>>> {
    let field_rows = sqlx::query(
        "SELECT field_id, farm_id, name, crop, season FROM fields \
         WHERE owner = ?1 ORDER BY name ASC, field_id ASC",
    )
    .bind(&identity.org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let now = Utc::now();
    let mut cards = Vec::with_capacity(field_rows.len());
    for row in &field_rows {
        let field = field_input_from_row(row);
        let findings = field_findings(&state, &field.field_id).await?;
        let recommendations = field_recommendations(&state, &field.field_id).await?;
        let latest_scene = field_latest_scene(&state, &field.field_id).await?;
        let alert_fired_ats = field_alert_fired_ats(&state, &field.field_id).await?;
        cards.push(build_field_card(
            field,
            &findings,
            &recommendations,
            latest_scene.map(|scene| scene.acquired_at),
            &alert_fired_ats,
            now,
        ));
    }

    Ok(Json(cards))
}

/// GET /api/portal/fields/:field_id/overview — aggregated detail for one
/// owned field. Ownership is checked first via [`owned_field`], so cross-org
/// and nonexistent fields are both 404.
pub async fn portal_field_overview(
    identity: PortalIdentity,
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<FieldOverview>> {
    let field = owned_field(&state, &identity.org_id, &field_id).await?;

    let findings = field_findings(&state, &field.field_id).await?;
    let recommendations = field_recommendations(&state, &field.field_id).await?;
    let latest_scene = field_latest_scene(&state, &field.field_id).await?;
    let alert_fired_ats = field_alert_fired_ats(&state, &field.field_id).await?;

    Ok(Json(build_field_overview(
        field,
        latest_scene,
        findings,
        recommendations,
        &alert_fired_ats,
        Utc::now(),
    )))
}
