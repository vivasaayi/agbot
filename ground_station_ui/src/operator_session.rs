use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared::control_plane::{
    authorize, AuthorizationDecision, ControlPlaneAction, MembershipRole, TenantPrincipal,
};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

pub type SharedOperatorSessionRegistry = Arc<RwLock<OperatorSessionRegistry>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatorSessionConfig {
    session_ttl: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorCredential {
    email: String,
    credential: String,
    principal: TenantPrincipal,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OperatorLoginRequest {
    pub email: String,
    pub credential: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OperatorSession {
    pub session_token: String,
    pub operator_id: Uuid,
    pub org_id: Uuid,
    pub role: MembershipRole,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizedOperatorAction {
    pub principal: TenantPrincipal,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OperatorSessionError {
    #[error("invalid operator credentials")]
    InvalidCredentials,
    #[error("operator role is not authorized for action routes")]
    RoleNotAuthorized,
    #[error("operator session token is missing")]
    MissingSession,
    #[error("operator session not found")]
    SessionNotFound,
    #[error("operator session expired")]
    SessionExpired,
}

#[derive(Debug, Clone, Default)]
pub struct OperatorSessionRegistry {
    config: OperatorSessionConfig,
    credentials_by_email: BTreeMap<String, OperatorCredential>,
    sessions_by_token: BTreeMap<String, StoredOperatorSession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredOperatorSession {
    session: OperatorSession,
    principal: TenantPrincipal,
}

impl OperatorSessionConfig {
    pub fn minutes(minutes: i64) -> Self {
        Self {
            session_ttl: Duration::minutes(minutes),
        }
    }

    pub fn seconds(seconds: i64) -> Self {
        Self {
            session_ttl: Duration::seconds(seconds),
        }
    }
}

impl Default for OperatorSessionConfig {
    fn default() -> Self {
        Self::minutes(30)
    }
}

impl OperatorCredential {
    pub fn new(
        email: impl Into<String>,
        credential: impl Into<String>,
        principal: TenantPrincipal,
    ) -> Self {
        Self {
            email: normalize_email(&email.into()),
            credential: credential.into(),
            principal,
        }
    }

    fn matches(&self, credential: &str) -> bool {
        self.credential == credential
    }
}

impl OperatorSessionRegistry {
    pub fn with_credentials(
        config: OperatorSessionConfig,
        credentials: Vec<OperatorCredential>,
    ) -> Self {
        let credentials_by_email = credentials
            .into_iter()
            .map(|credential| (credential.email.clone(), credential))
            .collect();
        Self {
            config,
            credentials_by_email,
            sessions_by_token: BTreeMap::new(),
        }
    }

    pub fn login_at(
        &mut self,
        request: OperatorLoginRequest,
        now: DateTime<Utc>,
    ) -> Result<OperatorSession, OperatorSessionError> {
        let email = normalize_email(&request.email);
        let credential = self
            .credentials_by_email
            .get(&email)
            .filter(|credential| credential.matches(&request.credential))
            .ok_or(OperatorSessionError::InvalidCredentials)?;
        if !operator_role_is_authorized(credential.principal.role) {
            return Err(OperatorSessionError::RoleNotAuthorized);
        }

        let session = OperatorSession {
            session_token: Uuid::new_v4().to_string(),
            operator_id: credential.principal.user_id,
            org_id: credential.principal.org_id,
            role: credential.principal.role,
            issued_at: now,
            expires_at: now + self.config.session_ttl,
        };
        self.sessions_by_token.insert(
            session.session_token.clone(),
            StoredOperatorSession {
                session: session.clone(),
                principal: credential.principal,
            },
        );
        Ok(session)
    }

    pub fn authorize_action_at(
        &self,
        session_token: &str,
        now: DateTime<Utc>,
    ) -> Result<AuthorizedOperatorAction, OperatorSessionError> {
        let token = session_token.trim();
        if token.is_empty() {
            return Err(OperatorSessionError::MissingSession);
        }
        let stored = self
            .sessions_by_token
            .get(token)
            .ok_or(OperatorSessionError::SessionNotFound)?;
        if now >= stored.session.expires_at {
            return Err(OperatorSessionError::SessionExpired);
        }
        if !operator_role_is_authorized(stored.principal.role) {
            return Err(OperatorSessionError::RoleNotAuthorized);
        }

        Ok(AuthorizedOperatorAction {
            principal: stored.principal,
            expires_at: stored.session.expires_at,
        })
    }

    pub fn active_session_count(&self) -> usize {
        self.sessions_by_token.len()
    }
}

pub fn shared_operator_session_registry(
    registry: OperatorSessionRegistry,
) -> SharedOperatorSessionRegistry {
    Arc::new(RwLock::new(registry))
}

fn operator_role_is_authorized(role: MembershipRole) -> bool {
    authorize(role, ControlPlaneAction::WriteEntity).decision == AuthorizationDecision::Allowed
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use shared::control_plane::{MembershipRole, TenantPrincipal};
    use uuid::Uuid;

    #[test]
    fn valid_operator_credentials_create_action_session() {
        let principal = TenantPrincipal {
            user_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            role: MembershipRole::Operator,
        };
        let mut registry = OperatorSessionRegistry::with_credentials(
            OperatorSessionConfig::minutes(15),
            vec![OperatorCredential::new(
                "ops@example.com",
                "correct horse battery staple",
                principal,
            )],
        );
        let now = Utc.with_ymd_and_hms(2026, 6, 12, 14, 0, 0).unwrap();

        let session = registry
            .login_at(
                OperatorLoginRequest {
                    email: "OPS@example.com".to_string(),
                    credential: "correct horse battery staple".to_string(),
                },
                now,
            )
            .expect("operator login should establish a session");
        let authorized = registry
            .authorize_action_at(&session.session_token, now)
            .expect("fresh session should authorize action route");

        assert_eq!(session.operator_id, principal.user_id);
        assert_eq!(session.org_id, principal.org_id);
        assert_eq!(session.role, MembershipRole::Operator);
        assert_eq!(session.issued_at, now);
        assert_eq!(
            session.expires_at,
            Utc.with_ymd_and_hms(2026, 6, 12, 14, 15, 0).unwrap()
        );
        assert_eq!(authorized.principal, principal);
    }

    #[test]
    fn viewer_credentials_do_not_create_action_session() {
        let principal = TenantPrincipal {
            user_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            role: MembershipRole::Viewer,
        };
        let mut registry = OperatorSessionRegistry::with_credentials(
            OperatorSessionConfig::minutes(15),
            vec![OperatorCredential::new(
                "viewer@example.com",
                "secret",
                principal,
            )],
        );

        let error = registry
            .login_at(
                OperatorLoginRequest {
                    email: "viewer@example.com".to_string(),
                    credential: "secret".to_string(),
                },
                Utc.with_ymd_and_hms(2026, 6, 12, 14, 0, 0).unwrap(),
            )
            .expect_err("viewer should not get an operator action session");

        assert_eq!(error, OperatorSessionError::RoleNotAuthorized);
        assert_eq!(registry.active_session_count(), 0);
    }

    #[test]
    fn expired_or_unknown_session_is_rejected_fail_closed() {
        let principal = TenantPrincipal {
            user_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            role: MembershipRole::Operator,
        };
        let mut registry = OperatorSessionRegistry::with_credentials(
            OperatorSessionConfig::minutes(1),
            vec![OperatorCredential::new(
                "ops@example.com",
                "secret",
                principal,
            )],
        );
        let issued_at = Utc.with_ymd_and_hms(2026, 6, 12, 14, 0, 0).unwrap();
        let session = registry
            .login_at(
                OperatorLoginRequest {
                    email: "ops@example.com".to_string(),
                    credential: "secret".to_string(),
                },
                issued_at,
            )
            .expect("operator login should establish a session");

        let expired = registry
            .authorize_action_at(
                &session.session_token,
                Utc.with_ymd_and_hms(2026, 6, 12, 14, 1, 1).unwrap(),
            )
            .expect_err("expired session should be rejected");
        let unknown = registry
            .authorize_action_at("missing-token", issued_at)
            .expect_err("unknown session should be rejected");

        assert_eq!(expired, OperatorSessionError::SessionExpired);
        assert_eq!(unknown, OperatorSessionError::SessionNotFound);
    }
}
