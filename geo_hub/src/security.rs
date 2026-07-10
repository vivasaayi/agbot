//! Global request-security middleware for the geo_hub HTTP surface.
//!
//! Today this is a single concern: an optional **require-session gate** that,
//! when enabled (`security.require_session`), rejects anonymous callers on the
//! `/api/*` surface with 401. It is off by default so local/dev/test runs stay
//! unauthenticated; exposed deployments turn it on. A short allowlist keeps the
//! endpoints that must work pre-login reachable (health, login, the static app
//! shell / PWA / browse assets, and token-bearing public share links).
//!
//! Session validation reuses `crate::routes::resolve_portal_session`, so the
//! middleware and the per-handler `PortalIdentity` extractor enforce identical
//! validity rules.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::error::AppError;
use crate::routes::{bearer_token, resolve_portal_session};
use crate::state::AppState;

/// Paths reachable without a session even when the require-session gate is on.
///
/// These are: health/readiness probes, the access-code login endpoint, the
/// static app shell / PWA / browse assets, and token-authenticated public
/// share links (which carry their own capability token in the path, not a
/// session).
pub fn is_public_path(path: &str) -> bool {
    const EXACT: &[&str] = &[
        "/",
        "/app",
        "/browse",
        "/browse/app.js",
        "/browse/style.css",
        "/health",
        "/ready",
        "/api/ingest/health",
        "/api/portal/login",
        "/portal",
    ];
    if EXACT.contains(&path) {
        return true;
    }
    // Static file mounts (ServeDir), token-bearing public share links, and the
    // admin API — which is authenticated by its own `AdminIdentity` bearer
    // token (`GEO_HUB__SECURITY__ADMIN_TOKEN`), not a portal session, so it is
    // exempt from the session gate (never open regardless).
    path.starts_with("/workspace")
        || path.starts_with("/portal/")
        || path.starts_with("/api/report-shares/")
        || path.starts_with("/api/admin/")
}

/// Require-session middleware. Passes through when the gate is disabled or the
/// path is public; otherwise demands a valid `Authorization: Bearer <token>`
/// portal session and rejects with 401 when absent or invalid.
pub async fn require_session_mw(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if !state.config.security.require_session || is_public_path(request.uri().path()) {
        return next.run(request).await;
    }

    // Copy the token out before handing `request` to `next.run`, which consumes
    // it (and therefore the borrow of its headers).
    let token = match bearer_token(request.headers()) {
        Some(token) => token.to_string(),
        None => return AppError::Unauthorized.into_response(),
    };

    match resolve_portal_session(&state, &token).await {
        Ok(_identity) => next.run(request).await,
        Err(err) => err.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_paths_are_allowlisted() {
        for path in [
            "/",
            "/app",
            "/browse",
            "/browse/app.js",
            "/browse/style.css",
            "/health",
            "/ready",
            "/api/ingest/health",
            "/api/portal/login",
            "/portal",
            "/portal/index.html",
            "/portal/sw.js",
            "/workspace",
            "/workspace/js/app.js",
            "/api/report-shares/abc123",
            // Admin API is exempt from the session gate (own AdminIdentity token).
            "/api/admin/portal/access-codes",
            "/api/admin/portal/access-codes/code-1/revoke",
        ] {
            assert!(is_public_path(path), "expected gate-exempt: {path}");
        }
    }

    #[test]
    fn api_surface_requires_session() {
        for path in [
            "/api/portal/me",
            "/api/portal/farms",
            "/api/farms",
            "/api/fields",
            "/api/satellite/derive",
            "/api/catalog/products",
            // A near-miss on an allowlist prefix must not slip through.
            "/api/portal/login-attempts",
            "/api/report-shares",
        ] {
            assert!(!is_public_path(path), "expected gated: {path}");
        }
    }
}
