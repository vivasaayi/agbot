//! Global request-security middleware for the geo_hub HTTP surface.
//!
//! Two optional, config-gated concerns, both off by default so local/dev/test
//! runs are unaffected:
//!
//! - **Require-session gate** (`security.require_session`): rejects anonymous
//!   callers on the `/api/*` surface with 401. A short allowlist keeps the
//!   endpoints that must work pre-login reachable (health, login, the static
//!   app shell / PWA / browse assets, token-bearing share links, admin API).
//!   Validation reuses `crate::routes::resolve_portal_session`, so the gate and
//!   the per-handler `PortalIdentity` extractor enforce identical rules.
//! - **Per-IP rate limit** (`security.rate_limit_per_min`): a coarse fixed
//!   window per client IP, 429 when exceeded.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, StatusCode},
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

/// Per-IP fixed-window rate limiter. Each client IP may make at most `max`
/// requests per `window_secs`; the window resets on the first request after it
/// elapses. Coarse by design — one counter per IP, no sliding window — but
/// enough to blunt brute-force and runaway clients. `max == 0` means the
/// limiter is never constructed (see `build_router`).
pub struct RateLimiter {
    max: u32,
    window_secs: u64,
    start: Instant,
    buckets: Mutex<HashMap<IpAddr, (u64, u32)>>,
}

impl RateLimiter {
    pub fn new(max: u32, window_secs: u64) -> Self {
        Self {
            max,
            window_secs,
            start: Instant::now(),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Whether a request from `ip` is allowed right now.
    pub fn allow(&self, ip: IpAddr) -> bool {
        let now = self.start.elapsed().as_secs();
        self.check(ip, now)
    }

    /// Window-aware allow decision at an explicit clock (seconds since start),
    /// factored out so the window logic is unit-testable without wall-clock.
    fn check(&self, ip: IpAddr, now_secs: u64) -> bool {
        let mut buckets = self.buckets.lock().expect("rate limiter mutex");
        let entry = buckets.entry(ip).or_insert((now_secs, 0));
        let (window_start, count) = *entry;
        if now_secs.saturating_sub(window_start) >= self.window_secs {
            // New window.
            *entry = (now_secs, 1);
            true
        } else if count < self.max {
            *entry = (window_start, count + 1);
            true
        } else {
            false
        }
    }
}

/// The client IP for rate limiting: the peer address from `ConnectInfo` when
/// present (production, `into_make_service_with_connect_info`), else an
/// unspecified-address fallback so tests (which use `oneshot` without connect
/// info) share a single bucket.
fn client_ip(request: &Request) -> IpAddr {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
}

/// Per-IP rate-limit middleware. Applied outermost (see `build_router`) so it
/// sheds load before auth and database work. Exceeded → 429 + `Retry-After`.
pub async fn rate_limit_mw(
    State(limiter): State<Arc<RateLimiter>>,
    request: Request,
    next: Next,
) -> Response {
    if limiter.allow(client_ip(&request)) {
        next.run(request).await
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, limiter.window_secs.to_string())],
            "rate limit exceeded",
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(n: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, n))
    }

    #[test]
    fn rate_limiter_allows_up_to_max_then_blocks_within_window() {
        let limiter = RateLimiter::new(3, 60);
        // 3 allowed in the window, 4th blocked.
        assert!(limiter.check(ip(1), 0));
        assert!(limiter.check(ip(1), 10));
        assert!(limiter.check(ip(1), 20));
        assert!(!limiter.check(ip(1), 30), "over cap within window");
    }

    #[test]
    fn rate_limiter_resets_after_window_and_is_per_ip() {
        let limiter = RateLimiter::new(1, 60);
        assert!(limiter.check(ip(1), 0));
        assert!(
            !limiter.check(ip(1), 30),
            "second request same window blocked"
        );
        assert!(limiter.check(ip(1), 60), "new window after 60s");
        // A different IP has its own independent bucket.
        assert!(limiter.check(ip(2), 30));
    }

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
