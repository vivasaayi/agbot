//! Portal authentication domain rules: access-code and session-token
//! generation, sha256 hashing (only hashes are stored at rest), and the
//! validity predicates used by the login flow and the bearer-token extractor.
//!
//! Pure functions only — no database or HTTP concerns live here.

use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Sessions live for 30 days from creation.
pub const SESSION_TTL_DAYS: i64 = 30;

/// Generate a new plaintext portal access code (returned to the admin once,
/// never stored).
pub fn generate_access_code() -> String {
    format!("agb-{}", Uuid::new_v4().simple())
}

/// Generate a new plaintext bearer session token (returned to the client
/// once, never stored).
pub fn generate_session_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

/// Sha256 of the token as lowercase hex — the only form persisted at rest.
pub fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

/// When a session created at `now` stops being valid.
pub fn session_expires_at(now: DateTime<Utc>) -> DateTime<Utc> {
    now + Duration::days(SESSION_TTL_DAYS)
}

/// Validity-relevant slice of a `portal_access_codes` row.
#[derive(Debug, Clone, Default)]
pub struct AccessCodeStatus {
    pub revoked_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Validity-relevant slice of a `portal_sessions` row.
#[derive(Debug, Clone)]
pub struct SessionStatus {
    pub revoked_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
}

/// An access code is usable when it is not revoked and (if it carries an
/// expiry at all) that expiry is still in the future.
pub fn access_code_is_usable(record: &AccessCodeStatus, now: DateTime<Utc>) -> bool {
    record.revoked_at.is_none() && record.expires_at.is_none_or(|expires| expires > now)
}

/// A session is valid when it is not revoked and its expiry is in the future.
pub fn session_is_valid(record: &SessionStatus, now: DateTime<Utc>) -> bool {
    record.revoked_at.is_none() && record.expires_at > now
}

/// Parse an RFC 3339 timestamp column as stored by geo_hub. Returns `None`
/// for malformed values so callers can treat them as invalid rather than 500.
pub fn parse_stored_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(iso: &str) -> DateTime<Utc> {
        parse_stored_timestamp(iso).expect("test timestamp")
    }

    #[test]
    fn generated_access_code_has_prefix_and_is_unique() {
        let first = generate_access_code();
        let second = generate_access_code();
        assert!(first.starts_with("agb-"));
        assert_eq!(first.len(), 4 + 32);
        assert_ne!(first, second);
    }

    #[test]
    fn generated_session_token_is_64_chars_and_unique() {
        let first = generate_session_token();
        let second = generate_session_token();
        assert_eq!(first.len(), 64);
        assert_ne!(first, second);
    }

    #[test]
    fn hash_token_is_deterministic_lowercase_sha256_hex() {
        let hash = hash_token("agb-test");
        assert_eq!(hash.len(), 64);
        assert!(hash
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_eq!(hash, hash_token("agb-test"));
        assert_ne!(hash, hash_token("agb-other"));
    }

    #[test]
    fn session_expiry_is_thirty_days_out() {
        let now = Utc.with_ymd_and_hms(2026, 7, 1, 12, 0, 0).unwrap();
        assert_eq!(session_expires_at(now), now + Duration::days(30));
    }

    #[test]
    fn access_code_usability_covers_revoked_expired_and_open_ended() {
        let now = at("2026-07-06T00:00:00Z");

        let open_ended = AccessCodeStatus::default();
        assert!(access_code_is_usable(&open_ended, now));

        let future = AccessCodeStatus {
            revoked_at: None,
            expires_at: Some(at("2026-08-01T00:00:00Z")),
        };
        assert!(access_code_is_usable(&future, now));

        let expired = AccessCodeStatus {
            revoked_at: None,
            expires_at: Some(at("2026-07-01T00:00:00Z")),
        };
        assert!(!access_code_is_usable(&expired, now));

        let revoked = AccessCodeStatus {
            revoked_at: Some(at("2026-07-05T00:00:00Z")),
            expires_at: None,
        };
        assert!(!access_code_is_usable(&revoked, now));
    }

    #[test]
    fn session_validity_requires_unrevoked_and_unexpired() {
        let now = at("2026-07-06T00:00:00Z");

        let valid = SessionStatus {
            revoked_at: None,
            expires_at: at("2026-08-01T00:00:00Z"),
        };
        assert!(session_is_valid(&valid, now));

        let expired = SessionStatus {
            revoked_at: None,
            expires_at: at("2026-07-01T00:00:00Z"),
        };
        assert!(!session_is_valid(&expired, now));

        let revoked = SessionStatus {
            revoked_at: Some(now),
            expires_at: at("2026-08-01T00:00:00Z"),
        };
        assert!(!session_is_valid(&revoked, now));
    }

    #[test]
    fn malformed_stored_timestamp_parses_to_none() {
        assert!(parse_stored_timestamp("not-a-timestamp").is_none());
        assert!(parse_stored_timestamp("2026-07-06T00:00:00Z").is_some());
    }
}
