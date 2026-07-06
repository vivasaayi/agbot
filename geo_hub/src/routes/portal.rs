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
