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
    boundary_to_svg, build_field_card, build_field_overview, FieldCard, FieldInput, FieldOverview,
    FindingInput, RecommendationInput, SceneInput,
};
use crate::state::AppState;
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::Json;
use chrono::Utc;
use post_processor::findings_export::FindingExportRecord;
use post_processor::grower_report::{
    render_grower_ready_pdf, FieldReportMetadata, GrowerReportRequest, SceneReportMetadata,
};
use post_processor::product_anomalies::ProductAnomalyReasonCode;
use post_processor::zone_delineation::{AnomalyZone, AnomalyZonePolygon};
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

// ---------------------------------------------------------------------------
// Portal report inbox + grower PDF generation (batch F-B3). Reports reach the
// inbox through their field: `reports.field_id -> fields.field_id` with
// `fields.owner = org`, so cross-org report IDs stay indistinguishable from
// missing ones (404). Read receipts are per account in `portal_report_reads`.
// ---------------------------------------------------------------------------

/// Stored `reports.visibility` for grower-facing PDFs generated via the portal.
const GROWER_REPORT_VISIBILITY: &str = "grower";
/// Stored `reports.format` for grower PDFs.
const GROWER_REPORT_FORMAT: &str = "pdf";
/// Season bucket used when the field row has no season.
const UNSPECIFIED_SEASON: &str = "season-unspecified";

/// One inbox entry: a report row joined to its owning field, plus the
/// caller-account read flag.
#[derive(Debug, Serialize)]
pub struct PortalReport {
    pub report_id: String,
    pub scene_id: String,
    pub field_id: String,
    pub field_name: String,
    pub title: String,
    pub format: String,
    pub visibility: String,
    pub annotation_count: i64,
    pub recommendation_count: i64,
    pub created_at: String,
    pub read: bool,
}

fn portal_report_from_row(row: &sqlx::sqlite::SqliteRow) -> PortalReport {
    PortalReport {
        report_id: row.get("report_id"),
        scene_id: row.get("scene_id"),
        field_id: row.get("field_id"),
        field_name: row.get("field_name"),
        title: row.get("title"),
        format: row.get("format"),
        visibility: row.get("visibility"),
        annotation_count: row.get("annotation_count"),
        recommendation_count: row.get("recommendation_count"),
        created_at: row.get("created_at"),
        read: row.get::<i64, _>("is_read") != 0,
    }
}

#[derive(Debug, Deserialize)]
pub struct PortalReportListQuery {
    pub unread_only: Option<bool>,
}

/// GET /api/portal/reports?unread_only= — the caller's report inbox: reports
/// for fields owned by the org, newest first, with per-account read flags.
pub async fn portal_list_reports(
    identity: PortalIdentity,
    Query(query): Query<PortalReportListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<PortalReport>>> {
    let unread_only = i64::from(query.unread_only.unwrap_or(false));
    let rows = sqlx::query(
        r#"
        SELECT r.report_id, r.scene_id, r.field_id, r.title, r.format, r.visibility,
               r.annotation_count, r.recommendation_count, r.created_at,
               f.name AS field_name,
               CASE WHEN rr.report_id IS NULL THEN 0 ELSE 1 END AS is_read
        FROM reports r
        JOIN fields f ON f.field_id = r.field_id
        LEFT JOIN portal_report_reads rr
          ON rr.report_id = r.report_id AND rr.account_id = ?2
        WHERE f.owner = ?1 AND (?3 = 0 OR rr.report_id IS NULL)
        ORDER BY r.created_at DESC, r.report_id ASC
        "#,
    )
    .bind(&identity.org_id)
    .bind(&identity.account_id)
    .bind(unread_only)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(rows.iter().map(portal_report_from_row).collect()))
}

/// Load a report (plus its artifact path) only if its field belongs to the
/// caller's org; cross-org and missing reports are both 404.
async fn owned_portal_report(
    state: &AppState,
    identity: &PortalIdentity,
    report_id: &str,
) -> AppResult<(PortalReport, String)> {
    let row = sqlx::query(
        r#"
        SELECT r.report_id, r.scene_id, r.field_id, r.title, r.format, r.visibility,
               r.annotation_count, r.recommendation_count, r.created_at, r.path,
               f.name AS field_name,
               CASE WHEN rr.report_id IS NULL THEN 0 ELSE 1 END AS is_read
        FROM reports r
        JOIN fields f ON f.field_id = r.field_id
        LEFT JOIN portal_report_reads rr
          ON rr.report_id = r.report_id AND rr.account_id = ?3
        WHERE r.report_id = ?1 AND f.owner = ?2
        "#,
    )
    .bind(report_id)
    .bind(&identity.org_id)
    .bind(&identity.account_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?
    .ok_or(AppError::NotFound)?;

    Ok((portal_report_from_row(&row), row.get("path")))
}

#[derive(Debug, Serialize)]
pub struct PortalReportReadResponse {
    pub report_id: String,
    pub read: bool,
}

/// POST /api/portal/reports/:report_id/read — idempotently record that the
/// caller's account has read an owned report.
pub async fn portal_mark_report_read(
    identity: PortalIdentity,
    Path(report_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<PortalReportReadResponse>> {
    owned_portal_report(&state, &identity, &report_id).await?;

    sqlx::query(
        "INSERT OR IGNORE INTO portal_report_reads (account_id, report_id, read_at) \
         VALUES (?1, ?2, ?3)",
    )
    .bind(&identity.account_id)
    .bind(&report_id)
    .bind(current_record_timestamp())
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(PortalReportReadResponse {
        report_id,
        read: true,
    }))
}

fn portal_report_content_type(format: &str) -> &'static str {
    match format {
        "pdf" => "application/pdf",
        "html" => "text/html; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// GET /api/portal/reports/:report_id/download — stream an owned report's
/// artifact with a content type derived from the stored format (mirrors
/// `report_file_response` for the org-side download).
pub async fn portal_download_report(
    identity: PortalIdentity,
    Path(report_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let (report, path) = owned_portal_report(&state, &identity, &report_id).await?;

    let report_path = PathBuf::from(&path);
    let file = File::open(&report_path)
        .await
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => AppError::NotFound,
            _ => AppError::Anyhow(error.into()),
        })?;
    let body = Body::from_stream(ReaderStream::new(file));

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(portal_report_content_type(&report.format)),
    );
    if let Some(filename) = report_path.file_name().and_then(|name| name.to_str()) {
        if let Ok(value) = HeaderValue::from_str(&format!("inline; filename=\"{filename}\"")) {
            headers.insert(header::CONTENT_DISPOSITION, value);
        }
    }

    Ok((headers, body).into_response())
}

/// Derived layer kinds for a scene, as `layer:<kind>` refs. Prefers the
/// satellite product catalog (`catalog_products`) and falls back to the
/// legacy `products` table for scenes processed before the catalog existed.
async fn scene_layer_refs(state: &AppState, scene_id: &str) -> AppResult<Vec<String>> {
    let mut kinds: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT kind FROM catalog_products WHERE scene_id = ?1 ORDER BY kind ASC",
    )
    .bind(scene_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;
    if kinds.is_empty() {
        kinds = sqlx::query_as(
            "SELECT DISTINCT kind FROM products WHERE scene_id = ?1 ORDER BY kind ASC",
        )
        .bind(scene_id)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?;
    }
    Ok(kinds
        .into_iter()
        .map(|(kind,)| format!("layer:{kind}"))
        .collect())
}

/// Map a stored severity label onto the report priority scale.
fn grower_priority_from_severity(severity: Option<&str>) -> RecommendationPriority {
    match severity
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("critical") => RecommendationPriority::Critical,
        Some("high") => RecommendationPriority::High,
        Some("medium") => RecommendationPriority::Medium,
        _ => RecommendationPriority::Low,
    }
}

/// Recover the anomaly reason code from a finding's metrics when the
/// producing application recorded one; otherwise bucket it as a statistical
/// anomaly, the generic satellite-finding reason.
fn grower_reason_code(metrics: &serde_json::Value) -> ProductAnomalyReasonCode {
    match metrics.get("reason").and_then(|value| value.as_str()) {
        Some("below_absolute_threshold") => ProductAnomalyReasonCode::BelowAbsoluteThreshold,
        Some("above_absolute_threshold") => ProductAnomalyReasonCode::AboveAbsoluteThreshold,
        Some("above_statistical_band") => ProductAnomalyReasonCode::AboveStatisticalBand,
        _ => ProductAnomalyReasonCode::BelowStatisticalBand,
    }
}

/// Map one `application_findings` row into the grower-report export shape.
/// Application findings often carry no zone geometry; when
/// `zone_geometry_json` does not decode as an [`AnomalyZone`], a minimal
/// placeholder zone (empty polygon, zero area, unspecified CRS) keeps the
/// finding visible in the PDF instead of dropping it.
fn grower_finding_from_row(row: &sqlx::sqlite::SqliteRow) -> FindingExportRecord {
    let finding_id: String = row.get("finding_id");
    let kind: String = row.get("kind");
    let severity: Option<String> = row.get("severity");
    let metrics: serde_json::Value = row
        .get::<Option<String>, _>("metrics_json")
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or(serde_json::Value::Null);

    let zone = row
        .get::<Option<String>, _>("zone_geometry_json")
        .as_deref()
        .and_then(|raw| serde_json::from_str::<AnomalyZone>(raw).ok())
        .unwrap_or_else(|| AnomalyZone {
            zone_id: metrics
                .get("zone_id")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("zone-{kind}")),
            cell_indices: Vec::new(),
            polygon: AnomalyZonePolygon {
                coordinates: Vec::new(),
            },
            area_m2: metrics
                .get("area_m2")
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0) as f32,
            centroid: (0.0, 0.0),
            crs: "unspecified".to_string(),
            evidence: Vec::new(),
        });

    let evidence_refs = row
        .get::<Option<String>, _>("evidence_refs_json")
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
        .filter(|refs| !refs.is_empty())
        .unwrap_or_else(|| vec![format!("finding:{finding_id}")]);

    FindingExportRecord {
        finding_id,
        zone,
        reason: grower_reason_code(&metrics),
        priority: grower_priority_from_severity(severity.as_deref()),
        evidence_refs,
    }
}

async fn grower_findings(state: &AppState, field_id: &str) -> AppResult<Vec<FindingExportRecord>> {
    let rows = sqlx::query(
        "SELECT finding_id, kind, severity, zone_geometry_json, metrics_json, evidence_refs_json \
         FROM application_findings WHERE field_id = ?1 \
         ORDER BY created_at DESC, finding_id ASC",
    )
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;
    Ok(rows.iter().map(grower_finding_from_row).collect())
}

async fn grower_open_recommendations(
    state: &AppState,
    field_id: &str,
) -> AppResult<Vec<RecommendationRecord>> {
    let rows = sqlx::query(
        "SELECT recommendation_id, scene_id, field_id, title, note, category, priority, status, \
                evidence_refs_json, created_at, updated_at \
         FROM recommendations WHERE field_id = ?1 AND status = 'open' \
         ORDER BY created_at ASC, recommendation_id ASC",
    )
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut records = Vec::with_capacity(rows.len());
    for row in &rows {
        records.push(decode_recommendation_record(state, row).await?);
    }
    Ok(records)
}

/// Fallback map view for fields whose boundary is missing or not a polygon:
/// the grower report requires a non-empty map view, and a labelled placeholder
/// is more honest than refusing the report outright.
fn placeholder_map_svg(field_name: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 40\">\
         <text x=\"10\" y=\"25\">Field {} (no boundary map available)</text></svg>",
        field_name.replace(['<', '>'], "")
    )
}

/// POST /api/portal/fields/:field_id/grower-report — assemble a
/// [`GrowerReportRequest`] from the field's stored rows, render the grower
/// PDF, persist it under the same `reports/<scene_id>/` convention as scene
/// reports, and register it in the inbox (unread, visibility `grower`).
pub async fn portal_generate_grower_report(
    identity: PortalIdentity,
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<PortalReport>> {
    let field = owned_field(&state, &identity.org_id, &field_id).await?;

    let scene = field_latest_scene(&state, &field.field_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "field {field_id} has no linked scene; a grower report needs at least one \
                 processed satellite scene"
            ))
        })?;

    let layer_refs = scene_layer_refs(&state, &scene.scene_id).await?;
    if layer_refs.is_empty() {
        return Err(AppError::BadRequest(format!(
            "scene {} has no derived layers; run the pipeline before generating a grower report",
            scene.scene_id
        )));
    }

    let findings = grower_findings(&state, &field.field_id).await?;
    let recommendations = grower_open_recommendations(&state, &field.field_id).await?;

    let (boundary_json,): (String,) =
        sqlx::query_as("SELECT boundary_json FROM fields WHERE field_id = ?1")
            .bind(&field.field_id)
            .fetch_one(&state.pool)
            .await
            .map_err(Error::from)?;
    let map_view_svg =
        boundary_to_svg(&boundary_json).unwrap_or_else(|| placeholder_map_svg(&field.name));

    let generated_at = current_record_timestamp();
    let generated_date = generated_at.get(..10).unwrap_or(&generated_at);
    let title = format!("Grower report — {} {}", field.name, generated_date);
    let report_id = Uuid::new_v4().to_string();

    let request = GrowerReportRequest {
        report_id: report_id.clone(),
        title: title.clone(),
        field: FieldReportMetadata {
            field_id: field.field_id.clone(),
            field_name: field.name.clone(),
            org_id: identity.org_id.clone(),
            season_id: field
                .season
                .clone()
                .unwrap_or_else(|| UNSPECIFIED_SEASON.to_string()),
        },
        scene: SceneReportMetadata {
            scene_id: scene.scene_id.clone(),
            captured_at: scene.acquired_at.clone(),
            layer_refs,
        },
        map_view_svg,
        findings,
        recommendations,
        generated_at: generated_at.clone(),
    };

    let pdf =
        render_grower_ready_pdf(&request).map_err(|err| AppError::BadRequest(err.to_string()))?;

    let report_dir = state.config.data_root.join("reports").join(&scene.scene_id);
    fs::create_dir_all(&report_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let artifact_path = report_dir.join(format!("{report_id}.pdf"));
    fs::write(&artifact_path, &pdf)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let artifact_uri = artifact_path.to_string_lossy().to_string();

    let recommendation_count = request.recommendations.len() as i64;
    sqlx::query(
        r#"
        INSERT INTO reports (
            report_id, scene_id, field_id, title, format, path, visibility,
            annotation_count, recommendation_count, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9)
        "#,
    )
    .bind(&report_id)
    .bind(&scene.scene_id)
    .bind(&field.field_id)
    .bind(&title)
    .bind(GROWER_REPORT_FORMAT)
    .bind(&artifact_uri)
    .bind(GROWER_REPORT_VISIBILITY)
    .bind(recommendation_count)
    .bind(&generated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(PortalReport {
        report_id,
        scene_id: scene.scene_id,
        field_id: field.field_id,
        field_name: field.name,
        title,
        format: GROWER_REPORT_FORMAT.to_string(),
        visibility: GROWER_REPORT_VISIBILITY.to_string(),
        annotation_count: 0,
        recommendation_count,
        created_at: generated_at,
        read: false,
    }))
}
