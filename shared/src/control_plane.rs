use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationRecord {
    pub org_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStatus {
    Active,
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRecord {
    pub user_id: Uuid,
    pub email: String,
    pub org_id: Uuid,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipRecord {
    pub membership_id: Uuid,
    pub user_id: Uuid,
    pub org_id: Uuid,
    #[serde(default)]
    pub role: MembershipRole,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedUserMembership {
    pub user: UserRecord,
    pub membership: MembershipRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MembershipRole {
    Admin,
    Advisor,
    Operator,
    Viewer,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlPlaneAction {
    ReadEntity,
    WriteEntity,
    ManageUsers,
    ViewAudit,
    ExportData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorizationDecision {
    Allowed,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationResult {
    pub role: MembershipRole,
    pub action: ControlPlaneAction,
    pub decision: AuthorizationDecision,
    pub reason_code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantPrincipal {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub role: MembershipRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantScope {
    pub org_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantBoundaryAuditEvent {
    pub actor_user_id: Uuid,
    pub actor_org_id: Uuid,
    pub action: ControlPlaneAction,
    pub target_ref: Option<Uuid>,
    pub target_org_id: Option<Uuid>,
    pub decision: AuthorizationDecision,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ControlPlaneError {
    #[error("organization name cannot be empty")]
    EmptyOrganizationName,
    #[error("organization not found: {org_id}")]
    OrganizationNotFound { org_id: Uuid },
    #[error("email cannot be empty")]
    EmptyEmail,
    #[error("duplicate user email: {email}")]
    DuplicateEmail { email: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TenantIsolationError {
    #[error("not found")]
    NotFound {
        audit: Option<TenantBoundaryAuditEvent>,
    },
    #[error("tenant write rejected")]
    WriteRejected { audit: TenantBoundaryAuditEvent },
    #[error(transparent)]
    ControlPlane(#[from] ControlPlaneError),
}

pub trait TenantScoped {
    fn tenant_org_id(&self) -> Uuid;
    fn tenant_record_id(&self) -> Uuid;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlPlaneRegistry {
    organizations: HashMap<Uuid, OrganizationRecord>,
    users: HashMap<Uuid, UserRecord>,
    memberships: HashMap<Uuid, MembershipRecord>,
    email_index: HashMap<String, Uuid>,
}

impl Default for MembershipRole {
    fn default() -> Self {
        Self::Viewer
    }
}

impl TenantPrincipal {
    pub fn from_membership(membership: &MembershipRecord) -> Self {
        Self {
            user_id: membership.user_id,
            org_id: membership.org_id,
            role: membership.role,
        }
    }
}

impl TenantScope {
    pub fn from_principal(principal: &TenantPrincipal, _request_org_id: Option<Uuid>) -> Self {
        Self {
            org_id: principal.org_id,
        }
    }
}

impl TenantIsolationError {
    pub fn audit_event(&self) -> Option<&TenantBoundaryAuditEvent> {
        match self {
            Self::NotFound { audit } => audit.as_ref(),
            Self::WriteRejected { audit } => Some(audit),
            Self::ControlPlane(_) => None,
        }
    }
}

impl TenantScoped for OrganizationRecord {
    fn tenant_org_id(&self) -> Uuid {
        self.org_id
    }

    fn tenant_record_id(&self) -> Uuid {
        self.org_id
    }
}

impl TenantScoped for UserRecord {
    fn tenant_org_id(&self) -> Uuid {
        self.org_id
    }

    fn tenant_record_id(&self) -> Uuid {
        self.user_id
    }
}

impl TenantScoped for MembershipRecord {
    fn tenant_org_id(&self) -> Uuid {
        self.org_id
    }

    fn tenant_record_id(&self) -> Uuid {
        self.membership_id
    }
}

impl ControlPlaneRegistry {
    pub fn create_organization(
        &mut self,
        name: String,
    ) -> Result<OrganizationRecord, ControlPlaneError> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(ControlPlaneError::EmptyOrganizationName);
        }

        let organization = OrganizationRecord {
            org_id: Uuid::new_v4(),
            name,
            created_at: Utc::now(),
        };
        self.organizations
            .insert(organization.org_id, organization.clone());
        Ok(organization)
    }

    pub fn create_user(
        &mut self,
        org_id: Uuid,
        email: String,
    ) -> Result<CreatedUserMembership, ControlPlaneError> {
        self.create_user_with_role(org_id, email, MembershipRole::Viewer)
    }

    pub fn create_user_with_role(
        &mut self,
        org_id: Uuid,
        email: String,
        role: MembershipRole,
    ) -> Result<CreatedUserMembership, ControlPlaneError> {
        if !self.organizations.contains_key(&org_id) {
            return Err(ControlPlaneError::OrganizationNotFound { org_id });
        }

        let email = normalize_email(&email)?;
        if self.email_index.contains_key(&email) {
            return Err(ControlPlaneError::DuplicateEmail { email });
        }

        let now = Utc::now();
        let user = UserRecord {
            user_id: Uuid::new_v4(),
            email: email.clone(),
            org_id,
            status: UserStatus::Active,
            created_at: now,
        };
        let membership = MembershipRecord {
            membership_id: Uuid::new_v4(),
            user_id: user.user_id,
            org_id,
            role,
            joined_at: now,
        };

        self.email_index.insert(email, user.user_id);
        self.users.insert(user.user_id, user.clone());
        self.memberships
            .insert(membership.membership_id, membership.clone());

        Ok(CreatedUserMembership { user, membership })
    }

    pub fn create_user_scoped(
        &mut self,
        principal: &TenantPrincipal,
        request_org_id: Uuid,
        email: String,
        role: MembershipRole,
    ) -> Result<CreatedUserMembership, TenantIsolationError> {
        enforce_tenant_write_scope(principal, request_org_id, ControlPlaneAction::WriteEntity)?;
        Ok(self.create_user_with_role(principal.org_id, email, role)?)
    }

    pub fn get_organization(&self, org_id: Uuid) -> Option<&OrganizationRecord> {
        self.organizations.get(&org_id)
    }

    pub fn organizations(&self) -> Vec<OrganizationRecord> {
        let mut organizations = self.organizations.values().cloned().collect::<Vec<_>>();
        organizations.sort_by(|left, right| left.name.cmp(&right.name));
        organizations
    }

    pub fn get_user(&self, user_id: Uuid) -> Option<&UserRecord> {
        self.users.get(&user_id)
    }

    pub fn get_user_scoped(
        &self,
        principal: &TenantPrincipal,
        user_id: Uuid,
    ) -> Result<&UserRecord, TenantIsolationError> {
        read_tenant_scoped(
            principal,
            self.users.get(&user_id),
            ControlPlaneAction::ReadEntity,
        )
    }

    pub fn users(&self) -> Vec<UserRecord> {
        let mut users = self.users.values().cloned().collect::<Vec<_>>();
        users.sort_by(|left, right| left.email.cmp(&right.email));
        users
    }

    pub fn memberships_for_org(&self, org_id: Uuid) -> Vec<MembershipRecord> {
        let mut memberships = self
            .memberships
            .values()
            .filter(|membership| membership.org_id == org_id)
            .cloned()
            .collect::<Vec<_>>();
        memberships.sort_by(|left, right| left.joined_at.cmp(&right.joined_at));
        memberships
    }
}

pub fn read_tenant_scoped<'a, T: TenantScoped>(
    principal: &TenantPrincipal,
    record: Option<&'a T>,
    action: ControlPlaneAction,
) -> Result<&'a T, TenantIsolationError> {
    match record {
        Some(record) if record.tenant_org_id() == principal.org_id => Ok(record),
        Some(record) => Err(TenantIsolationError::NotFound {
            audit: Some(tenant_boundary_audit_event(
                principal,
                action,
                Some(record.tenant_record_id()),
                Some(record.tenant_org_id()),
                "cross_tenant_read",
            )),
        }),
        None => Err(TenantIsolationError::NotFound { audit: None }),
    }
}

pub fn enforce_tenant_write_scope(
    principal: &TenantPrincipal,
    request_org_id: Uuid,
    action: ControlPlaneAction,
) -> Result<TenantScope, TenantIsolationError> {
    let scope = TenantScope::from_principal(principal, Some(request_org_id));
    if request_org_id != scope.org_id {
        return Err(TenantIsolationError::WriteRejected {
            audit: tenant_boundary_audit_event(
                principal,
                action,
                None,
                Some(request_org_id),
                "cross_tenant_write",
            ),
        });
    }

    Ok(scope)
}

pub fn authorize(role: MembershipRole, action: ControlPlaneAction) -> AuthorizationResult {
    let (decision, reason_code) = if role == MembershipRole::Unknown {
        (AuthorizationDecision::Denied, "unknown_role")
    } else if role_allows_action(role, action) {
        (AuthorizationDecision::Allowed, "allowed")
    } else {
        (AuthorizationDecision::Denied, "role_not_permitted")
    };

    AuthorizationResult {
        role,
        action,
        decision,
        reason_code: reason_code.to_string(),
    }
}

fn tenant_boundary_audit_event(
    principal: &TenantPrincipal,
    action: ControlPlaneAction,
    target_ref: Option<Uuid>,
    target_org_id: Option<Uuid>,
    reason_code: &str,
) -> TenantBoundaryAuditEvent {
    TenantBoundaryAuditEvent {
        actor_user_id: principal.user_id,
        actor_org_id: principal.org_id,
        action,
        target_ref,
        target_org_id,
        decision: AuthorizationDecision::Denied,
        reason_code: reason_code.to_string(),
    }
}

fn role_allows_action(role: MembershipRole, action: ControlPlaneAction) -> bool {
    match role {
        MembershipRole::Admin => true,
        MembershipRole::Advisor => matches!(
            action,
            ControlPlaneAction::ReadEntity
                | ControlPlaneAction::ViewAudit
                | ControlPlaneAction::ExportData
        ),
        MembershipRole::Operator => {
            matches!(
                action,
                ControlPlaneAction::ReadEntity | ControlPlaneAction::WriteEntity
            )
        }
        MembershipRole::Viewer => matches!(action, ControlPlaneAction::ReadEntity),
        MembershipRole::Unknown => false,
    }
}

fn normalize_email(email: &str) -> Result<String, ControlPlaneError> {
    let email = email.trim().to_ascii_lowercase();
    if email.is_empty() {
        return Err(ControlPlaneError::EmptyEmail);
    }
    Ok(email)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organization_user_and_membership_are_linked() {
        let mut registry = ControlPlaneRegistry::default();

        let org = registry
            .create_organization("AgBot Farms".to_string())
            .expect("org creates");
        let created = registry
            .create_user(org.org_id, "ops@example.com".to_string())
            .expect("user creates");

        assert_eq!(created.user.email, "ops@example.com");
        assert_eq!(created.user.org_id, org.org_id);
        assert_eq!(created.user.status, UserStatus::Active);
        assert_eq!(created.membership.org_id, org.org_id);
        assert_eq!(created.membership.user_id, created.user.user_id);
        assert!(registry.get_organization(org.org_id).is_some());
        assert_eq!(registry.organizations().len(), 1);
        assert!(registry.get_user(created.user.user_id).is_some());
        assert_eq!(registry.memberships_for_org(org.org_id).len(), 1);
    }

    #[test]
    fn duplicate_email_is_rejected_without_writing_user() {
        let mut registry = ControlPlaneRegistry::default();
        let org = registry
            .create_organization("AgBot Farms".to_string())
            .expect("org creates");
        registry
            .create_user(org.org_id, "ops@example.com".to_string())
            .expect("first user creates");

        let error = registry
            .create_user(org.org_id, "OPS@example.com".to_string())
            .expect_err("duplicate email is rejected case-insensitively");

        assert_eq!(
            error,
            ControlPlaneError::DuplicateEmail {
                email: "ops@example.com".to_string()
            }
        );
        assert_eq!(registry.users().len(), 1);
        assert_eq!(registry.memberships_for_org(org.org_id).len(), 1);
    }

    #[test]
    fn control_plane_records_round_trip_for_api_contract() {
        let mut registry = ControlPlaneRegistry::default();
        let org = registry
            .create_organization("AgBot Farms".to_string())
            .expect("org creates");
        let created = registry
            .create_user(org.org_id, "ops@example.com".to_string())
            .expect("user creates");

        let value = serde_json::to_value(&created).expect("created user serializes");

        assert_eq!(value["user"]["email"], "ops@example.com");
        assert_eq!(value["user"]["org_id"], org.org_id.to_string());
        assert_eq!(value["membership"]["org_id"], org.org_id.to_string());
    }

    #[test]
    fn viewer_write_is_denied_and_admin_write_is_allowed() {
        let viewer = authorize(MembershipRole::Viewer, ControlPlaneAction::WriteEntity);
        let admin = authorize(MembershipRole::Admin, ControlPlaneAction::WriteEntity);

        assert_eq!(viewer.decision, AuthorizationDecision::Denied);
        assert_eq!(viewer.reason_code, "role_not_permitted");
        assert_eq!(admin.decision, AuthorizationDecision::Allowed);
    }

    #[test]
    fn unknown_role_fails_closed() {
        let result = authorize(MembershipRole::Unknown, ControlPlaneAction::ReadEntity);

        assert_eq!(result.decision, AuthorizationDecision::Denied);
        assert_eq!(result.reason_code, "unknown_role");
    }

    #[test]
    fn authorization_result_serializes_contract() {
        let result = authorize(MembershipRole::Viewer, ControlPlaneAction::WriteEntity);
        let value = serde_json::to_value(&result).expect("authorization serializes");

        assert_eq!(value["role"], "Viewer");
        assert_eq!(value["action"], "WriteEntity");
        assert_eq!(value["decision"], "Denied");
        assert_eq!(value["reason_code"], "role_not_permitted");
    }

    #[test]
    fn tenant_scope_is_derived_from_principal_not_request_input() {
        let principal_org_id = Uuid::new_v4();
        let request_org_id = Uuid::new_v4();
        let principal = TenantPrincipal {
            user_id: Uuid::new_v4(),
            org_id: principal_org_id,
            role: MembershipRole::Admin,
        };

        let scope = TenantScope::from_principal(&principal, Some(request_org_id));

        assert_eq!(scope.org_id, principal_org_id);
    }

    #[test]
    fn cross_tenant_user_read_returns_not_found_with_audit_event() {
        let mut registry = ControlPlaneRegistry::default();
        let org_a = registry
            .create_organization("Org A".to_string())
            .expect("org A creates");
        let org_b = registry
            .create_organization("Org B".to_string())
            .expect("org B creates");
        let admin_a = registry
            .create_user_with_role(
                org_a.org_id,
                "admin-a@example.com".to_string(),
                MembershipRole::Admin,
            )
            .expect("admin A creates");
        let user_b = registry
            .create_user(org_b.org_id, "viewer-b@example.com".to_string())
            .expect("user B creates");
        let principal = TenantPrincipal::from_membership(&admin_a.membership);

        let error = registry
            .get_user_scoped(&principal, user_b.user.user_id)
            .expect_err("cross-tenant read is hidden");

        assert!(matches!(error, TenantIsolationError::NotFound { .. }));
        let audit = error.audit_event().expect("cross-tenant read is audited");
        assert_eq!(audit.actor_user_id, admin_a.user.user_id);
        assert_eq!(audit.actor_org_id, org_a.org_id);
        assert_eq!(audit.target_ref, Some(user_b.user.user_id));
        assert_eq!(audit.target_org_id, Some(org_b.org_id));
        assert_eq!(audit.action, ControlPlaneAction::ReadEntity);
        assert_eq!(audit.decision, AuthorizationDecision::Denied);
        assert_eq!(audit.reason_code, "cross_tenant_read");
    }

    #[test]
    fn cross_tenant_user_write_is_rejected_without_writing() {
        let mut registry = ControlPlaneRegistry::default();
        let org_a = registry
            .create_organization("Org A".to_string())
            .expect("org A creates");
        let org_b = registry
            .create_organization("Org B".to_string())
            .expect("org B creates");
        let admin_a = registry
            .create_user_with_role(
                org_a.org_id,
                "admin-a@example.com".to_string(),
                MembershipRole::Admin,
            )
            .expect("admin A creates");
        let principal = TenantPrincipal::from_membership(&admin_a.membership);
        let users_before = registry.users().len();
        let org_b_memberships_before = registry.memberships_for_org(org_b.org_id).len();

        let error = registry
            .create_user_scoped(
                &principal,
                org_b.org_id,
                "intruder@example.com".to_string(),
                MembershipRole::Viewer,
            )
            .expect_err("cross-tenant write is rejected");

        assert!(matches!(error, TenantIsolationError::WriteRejected { .. }));
        let audit = error.audit_event().expect("cross-tenant write is audited");
        assert_eq!(audit.actor_user_id, admin_a.user.user_id);
        assert_eq!(audit.actor_org_id, org_a.org_id);
        assert_eq!(audit.target_org_id, Some(org_b.org_id));
        assert_eq!(audit.action, ControlPlaneAction::WriteEntity);
        assert_eq!(audit.decision, AuthorizationDecision::Denied);
        assert_eq!(audit.reason_code, "cross_tenant_write");
        assert_eq!(registry.users().len(), users_before);
        assert_eq!(
            registry.memberships_for_org(org_b.org_id).len(),
            org_b_memberships_before
        );
    }
}
