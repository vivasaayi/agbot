use crate::schemas::{FarmFieldRegistry, FarmRecord, FieldRecord};
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
pub struct GrowerPortalSessionScope {
    pub grower_id: Uuid,
    pub org_id: Uuid,
    pub role: MembershipRole,
    pub farm_ids: Vec<String>,
    pub field_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerPortalAccessEvidence {
    pub grower_id: Uuid,
    pub org_id: Uuid,
    pub role: MembershipRole,
    pub action: ControlPlaneAction,
    pub field_id: String,
    pub target_org_id: Option<String>,
    pub decision: AuthorizationDecision,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerFieldActivitySummary {
    pub field_id: String,
    pub last_scene_date: Option<String>,
    pub open_recommendation_count: usize,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerPortalHomeField {
    pub field_id: String,
    pub farm_id: String,
    pub name: String,
    pub last_scene_date: Option<String>,
    pub open_recommendation_count: usize,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerPortalHomeFarm {
    pub farm_id: String,
    pub name: String,
    pub fields: Vec<GrowerPortalHomeField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerPortalHome {
    pub grower_id: Uuid,
    pub org_id: Uuid,
    pub farms: Vec<GrowerPortalHomeFarm>,
    pub empty_state: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrowerFieldAnalysisStatus {
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerLayerSummary {
    pub layer_id: String,
    pub product_type: String,
    pub source_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerFindingSummary {
    pub finding_id: String,
    pub title: String,
    pub source_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerRecommendationSummary {
    pub recommendation_id: String,
    pub title: String,
    pub source_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerFieldAnalysisSource {
    pub field_id: String,
    pub scene_id: String,
    pub captured_at: String,
    pub status: GrowerFieldAnalysisStatus,
    pub layers: Vec<GrowerLayerSummary>,
    pub latest_finding: Option<GrowerFindingSummary>,
    pub latest_recommendation: Option<GrowerRecommendationSummary>,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerPortalFieldOverview {
    pub grower_id: Uuid,
    pub org_id: Uuid,
    pub farm_id: String,
    pub field_id: String,
    pub field_name: String,
    pub latest_scene_id: Option<String>,
    pub latest_scene_date: Option<String>,
    pub layers: Vec<GrowerLayerSummary>,
    pub latest_finding: Option<GrowerFindingSummary>,
    pub latest_recommendation: Option<GrowerRecommendationSummary>,
    pub no_current_analysis: bool,
    pub evidence_refs: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecordRequest {
    pub actor_user_id: Uuid,
    pub org_id: Uuid,
    pub action: ControlPlaneAction,
    pub target_ref: Option<Uuid>,
    pub target_org_id: Option<Uuid>,
    pub decision: AuthorizationDecision,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub audit_id: Uuid,
    pub actor_user_id: Uuid,
    pub org_id: Uuid,
    pub action: ControlPlaneAction,
    pub target_ref: Option<Uuid>,
    pub target_org_id: Option<Uuid>,
    pub decision: AuthorizationDecision,
    pub reason_code: String,
    pub at: DateTime<Utc>,
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GrowerPortalAccessError {
    #[error("grower not found: {grower_id}")]
    GrowerNotFound { grower_id: Uuid },
    #[error("grower is suspended: {grower_id}")]
    GrowerSuspended { grower_id: Uuid },
    #[error("grower membership not found: {grower_id}")]
    MembershipNotFound { grower_id: Uuid },
    #[error("grower role is not permitted: {role:?}")]
    RoleDenied { role: MembershipRole },
    #[error("field not found: {field_id}")]
    FieldNotFound { field_id: String },
    #[error("grower portal access forbidden")]
    Forbidden {
        evidence: GrowerPortalAccessEvidence,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuditTrailError {
    #[error("audit record is append-only: {audit_id}")]
    AppendOnlyRecord { audit_id: Uuid },
    #[error("audit record not found: {audit_id}")]
    AuditRecordNotFound { audit_id: Uuid },
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
    #[serde(default)]
    audit_records: Vec<AuditRecord>,
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

impl GrowerPortalAccessError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::GrowerNotFound { .. } | Self::FieldNotFound { .. } => 404,
            Self::GrowerSuspended { .. }
            | Self::MembershipNotFound { .. }
            | Self::RoleDenied { .. }
            | Self::Forbidden { .. } => 403,
        }
    }
}

impl From<TenantBoundaryAuditEvent> for AuditRecordRequest {
    fn from(event: TenantBoundaryAuditEvent) -> Self {
        Self {
            actor_user_id: event.actor_user_id,
            org_id: event.actor_org_id,
            action: event.action,
            target_ref: event.target_ref,
            target_org_id: event.target_org_id,
            decision: event.decision,
            reason_code: event.reason_code,
        }
    }
}

pub fn resolve_grower_portal_session_scope(
    control_plane: &ControlPlaneRegistry,
    farm_fields: &FarmFieldRegistry,
    grower_id: Uuid,
) -> Result<GrowerPortalSessionScope, GrowerPortalAccessError> {
    let user = control_plane
        .get_user(grower_id)
        .ok_or(GrowerPortalAccessError::GrowerNotFound { grower_id })?;
    if user.status != UserStatus::Active {
        return Err(GrowerPortalAccessError::GrowerSuspended { grower_id });
    }
    let membership = control_plane
        .membership_for_user(grower_id)
        .filter(|membership| membership.org_id == user.org_id)
        .ok_or(GrowerPortalAccessError::MembershipNotFound { grower_id })?;
    let authorization = authorize(membership.role, ControlPlaneAction::ReadEntity);
    if authorization.decision != AuthorizationDecision::Allowed {
        return Err(GrowerPortalAccessError::RoleDenied {
            role: membership.role,
        });
    }

    let org_key = user.org_id.to_string();
    let farm_ids = farm_fields
        .farms_for_org(&org_key)
        .into_iter()
        .map(|farm| farm.farm_id)
        .collect();
    let field_ids = farm_fields
        .fields_for_org(&org_key)
        .into_iter()
        .map(|field| field.field_id)
        .collect();

    Ok(GrowerPortalSessionScope {
        grower_id,
        org_id: user.org_id,
        role: membership.role,
        farm_ids,
        field_ids,
    })
}

pub fn build_grower_portal_home(
    scope: &GrowerPortalSessionScope,
    farm_fields: &FarmFieldRegistry,
    activity: &[GrowerFieldActivitySummary],
) -> GrowerPortalHome {
    let org_key = scope.org_id.to_string();
    let activity_by_field = activity
        .iter()
        .map(|summary| (summary.field_id.as_str(), summary))
        .collect::<HashMap<_, _>>();
    let scoped_farm_ids = scope
        .farm_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let scoped_field_ids = scope
        .field_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let fields = farm_fields
        .fields_for_org(&org_key)
        .into_iter()
        .filter(|field| scoped_field_ids.contains(field.field_id.as_str()))
        .collect::<Vec<_>>();
    let mut farms = farm_fields
        .farms_for_org(&org_key)
        .into_iter()
        .filter(|farm| scoped_farm_ids.contains(farm.farm_id.as_str()))
        .map(|farm| {
            let farm_fields = fields
                .iter()
                .filter(|field| field.farm_id.as_deref() == Some(farm.farm_id.as_str()))
                .cloned()
                .collect();
            grower_home_farm(farm, farm_fields, &activity_by_field)
        })
        .collect::<Vec<_>>();
    farms.retain(|farm| !farm.fields.is_empty() || scoped_field_ids.is_empty());

    GrowerPortalHome {
        grower_id: scope.grower_id,
        org_id: scope.org_id,
        empty_state: fields.is_empty(),
        farms,
    }
}

fn grower_home_farm(
    farm: FarmRecord,
    mut fields: Vec<FieldRecord>,
    activity_by_field: &HashMap<&str, &GrowerFieldActivitySummary>,
) -> GrowerPortalHomeFarm {
    fields.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.field_id.cmp(&right.field_id))
    });
    let fields = fields
        .into_iter()
        .map(|field| {
            let activity = activity_by_field.get(field.field_id.as_str()).copied();
            GrowerPortalHomeField {
                field_id: field.field_id.clone(),
                farm_id: field.farm_id.unwrap_or_else(|| farm.farm_id.clone()),
                name: field.name,
                last_scene_date: activity.and_then(|summary| summary.last_scene_date.clone()),
                open_recommendation_count: activity
                    .map(|summary| summary.open_recommendation_count)
                    .unwrap_or(0),
                evidence_refs: activity
                    .map(|summary| summary.evidence_refs.clone())
                    .unwrap_or_else(|| vec![format!("field:{}:no_activity", field.field_id)]),
            }
        })
        .collect();

    GrowerPortalHomeFarm {
        farm_id: farm.farm_id,
        name: farm.name,
        fields,
    }
}

pub fn build_grower_portal_field_overview(
    scope: &GrowerPortalSessionScope,
    farm_fields: &FarmFieldRegistry,
    field_id: &str,
    analysis_sources: &[GrowerFieldAnalysisSource],
) -> Result<GrowerPortalFieldOverview, GrowerPortalAccessError> {
    let field_id = field_id.trim();
    if !scope.field_ids.iter().any(|id| id == field_id) {
        return Err(GrowerPortalAccessError::FieldNotFound {
            field_id: field_id.to_string(),
        });
    }
    let org_key = scope.org_id.to_string();
    let field = farm_fields
        .fields_for_org(&org_key)
        .into_iter()
        .find(|field| field.field_id == field_id)
        .ok_or_else(|| GrowerPortalAccessError::FieldNotFound {
            field_id: field_id.to_string(),
        })?;
    let farm_id = field
        .farm_id
        .clone()
        .ok_or_else(|| GrowerPortalAccessError::FieldNotFound {
            field_id: field_id.to_string(),
        })?;
    let latest = analysis_sources
        .iter()
        .filter(|source| source.field_id == field_id)
        .max_by(|left, right| {
            left.captured_at
                .cmp(&right.captured_at)
                .then(left.scene_id.cmp(&right.scene_id))
        });

    let Some(latest) = latest else {
        return Ok(empty_field_overview(scope, farm_id, field));
    };
    if latest.status == GrowerFieldAnalysisStatus::Failed {
        let mut overview = empty_field_overview(scope, farm_id, field);
        overview.evidence_refs = latest.evidence_refs.clone();
        return Ok(overview);
    }

    Ok(GrowerPortalFieldOverview {
        grower_id: scope.grower_id,
        org_id: scope.org_id,
        farm_id,
        field_id: field.field_id,
        field_name: field.name,
        latest_scene_id: Some(latest.scene_id.clone()),
        latest_scene_date: Some(latest.captured_at.clone()),
        layers: latest.layers.clone(),
        latest_finding: latest.latest_finding.clone(),
        latest_recommendation: latest.latest_recommendation.clone(),
        no_current_analysis: false,
        evidence_refs: latest.evidence_refs.clone(),
    })
}

fn empty_field_overview(
    scope: &GrowerPortalSessionScope,
    farm_id: String,
    field: FieldRecord,
) -> GrowerPortalFieldOverview {
    GrowerPortalFieldOverview {
        grower_id: scope.grower_id,
        org_id: scope.org_id,
        farm_id,
        field_id: field.field_id,
        field_name: field.name,
        latest_scene_id: None,
        latest_scene_date: None,
        layers: Vec::new(),
        latest_finding: None,
        latest_recommendation: None,
        no_current_analysis: true,
        evidence_refs: Vec::new(),
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
        if let Err(error) =
            enforce_tenant_write_scope(principal, request_org_id, ControlPlaneAction::WriteEntity)
        {
            self.append_tenant_error_audit(&error);
            return Err(error);
        }

        let created = self.create_user_with_role(principal.org_id, email, role)?;
        self.append_audit_record(AuditRecordRequest {
            actor_user_id: principal.user_id,
            org_id: principal.org_id,
            action: ControlPlaneAction::WriteEntity,
            target_ref: Some(created.user.user_id),
            target_org_id: Some(created.user.org_id),
            decision: AuthorizationDecision::Allowed,
            reason_code: "allowed".to_string(),
        });
        Ok(created)
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
        &mut self,
        principal: &TenantPrincipal,
        user_id: Uuid,
    ) -> Result<&UserRecord, TenantIsolationError> {
        match self
            .users
            .get(&user_id)
            .map(|user| (user.user_id, user.org_id))
        {
            Some((target_ref, target_org_id)) if target_org_id == principal.org_id => {
                self.append_audit_record(AuditRecordRequest {
                    actor_user_id: principal.user_id,
                    org_id: principal.org_id,
                    action: ControlPlaneAction::ReadEntity,
                    target_ref: Some(target_ref),
                    target_org_id: Some(target_org_id),
                    decision: AuthorizationDecision::Allowed,
                    reason_code: "allowed".to_string(),
                });
                Ok(self
                    .users
                    .get(&user_id)
                    .expect("user existed when scoped read was evaluated"))
            }
            Some((target_ref, target_org_id)) => {
                let audit = tenant_boundary_audit_event(
                    principal,
                    ControlPlaneAction::ReadEntity,
                    Some(target_ref),
                    Some(target_org_id),
                    "cross_tenant_read",
                );
                self.append_audit_record(AuditRecordRequest::from(audit.clone()));
                Err(TenantIsolationError::NotFound { audit: Some(audit) })
            }
            None => Err(TenantIsolationError::NotFound { audit: None }),
        }
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

    pub fn membership_for_user(&self, user_id: Uuid) -> Option<MembershipRecord> {
        self.memberships
            .values()
            .filter(|membership| membership.user_id == user_id)
            .min_by(|left, right| {
                left.joined_at
                    .cmp(&right.joined_at)
                    .then(left.membership_id.cmp(&right.membership_id))
            })
            .cloned()
    }

    pub fn read_grower_portal_field(
        &mut self,
        scope: &GrowerPortalSessionScope,
        farm_fields: &FarmFieldRegistry,
        field_id: &str,
    ) -> Result<FieldRecord, GrowerPortalAccessError> {
        let field_id = field_id.trim();
        if field_id.is_empty() {
            return Err(GrowerPortalAccessError::FieldNotFound {
                field_id: String::new(),
            });
        }

        let field = farm_fields.field_by_id(field_id).ok_or_else(|| {
            GrowerPortalAccessError::FieldNotFound {
                field_id: field_id.to_string(),
            }
        })?;
        let target_org_id = field.org_id.clone();
        let in_scope = target_org_id == scope.org_id.to_string()
            && scope.field_ids.iter().any(|allowed| allowed == field_id);

        if in_scope {
            self.append_audit_record(AuditRecordRequest {
                actor_user_id: scope.grower_id,
                org_id: scope.org_id,
                action: ControlPlaneAction::ReadEntity,
                target_ref: None,
                target_org_id: Some(scope.org_id),
                decision: AuthorizationDecision::Allowed,
                reason_code: "allowed".to_string(),
            });
            return Ok(field);
        }

        let reason_code = if target_org_id == scope.org_id.to_string() {
            "field_out_of_scope"
        } else {
            "cross_tenant_field_read"
        };
        let evidence = GrowerPortalAccessEvidence {
            grower_id: scope.grower_id,
            org_id: scope.org_id,
            role: scope.role,
            action: ControlPlaneAction::ReadEntity,
            field_id: field_id.to_string(),
            target_org_id: Some(target_org_id.clone()),
            decision: AuthorizationDecision::Denied,
            reason_code: reason_code.to_string(),
        };
        self.append_audit_record(AuditRecordRequest {
            actor_user_id: scope.grower_id,
            org_id: scope.org_id,
            action: ControlPlaneAction::ReadEntity,
            target_ref: None,
            target_org_id: target_org_id.parse::<Uuid>().ok(),
            decision: AuthorizationDecision::Denied,
            reason_code: reason_code.to_string(),
        });

        Err(GrowerPortalAccessError::Forbidden { evidence })
    }

    pub fn append_audit_record(&mut self, request: AuditRecordRequest) -> AuditRecord {
        self.append_audit_record_at(request, Utc::now())
    }

    pub fn append_audit_record_at(
        &mut self,
        request: AuditRecordRequest,
        at: DateTime<Utc>,
    ) -> AuditRecord {
        let record = AuditRecord {
            audit_id: Uuid::new_v4(),
            actor_user_id: request.actor_user_id,
            org_id: request.org_id,
            action: request.action,
            target_ref: request.target_ref,
            target_org_id: request.target_org_id,
            decision: request.decision,
            reason_code: request.reason_code,
            at,
        };
        self.audit_records.push(record.clone());
        record
    }

    pub fn audit_records_for_org(
        &self,
        org_id: Uuid,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> Vec<AuditRecord> {
        let mut records = self
            .audit_records
            .iter()
            .filter(|record| record.org_id == org_id)
            .filter(|record| from.as_ref().map_or(true, |from| record.at >= *from))
            .filter(|record| to.as_ref().map_or(true, |to| record.at <= *to))
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.at
                .cmp(&right.at)
                .then(left.audit_id.cmp(&right.audit_id))
        });
        records
    }

    pub fn update_audit_record(
        &mut self,
        audit_id: Uuid,
        _replacement: AuditRecordRequest,
    ) -> Result<(), AuditTrailError> {
        if self
            .audit_records
            .iter()
            .any(|record| record.audit_id == audit_id)
        {
            return Err(AuditTrailError::AppendOnlyRecord { audit_id });
        }

        Err(AuditTrailError::AuditRecordNotFound { audit_id })
    }

    fn append_tenant_error_audit(&mut self, error: &TenantIsolationError) {
        if let Some(audit) = error.audit_event().cloned() {
            self.append_audit_record(AuditRecordRequest::from(audit));
        }
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
    use crate::schemas::{
        FarmFieldEntityStatus, FarmFieldRegistry, FarmRecord, FieldBoundary, FieldRecord,
        GeoBounds, GeoPoint,
    };
    use chrono::TimeZone;

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
    fn grower_portal_session_scope_resolves_owned_farms_and_fields() {
        let mut control = ControlPlaneRegistry::default();
        let org_a = control
            .create_organization("Org A".to_string())
            .expect("org A creates");
        let org_b = control
            .create_organization("Org B".to_string())
            .expect("org B creates");
        let grower = control
            .create_user_with_role(
                org_a.org_id,
                "grower@example.com".to_string(),
                MembershipRole::Viewer,
            )
            .expect("grower creates");
        let mut farm_fields = FarmFieldRegistry::default();
        insert_test_farm_field_scope(&mut farm_fields, org_a.org_id, "farm-a", "field-a");
        insert_test_farm_field_scope(&mut farm_fields, org_b.org_id, "farm-b", "field-b");

        let scope =
            resolve_grower_portal_session_scope(&control, &farm_fields, grower.user.user_id)
                .expect("grower scope resolves");

        assert_eq!(scope.grower_id, grower.user.user_id);
        assert_eq!(scope.org_id, org_a.org_id);
        assert_eq!(scope.role, MembershipRole::Viewer);
        assert_eq!(scope.farm_ids, vec!["farm-a".to_string()]);
        assert_eq!(scope.field_ids, vec!["field-a".to_string()]);
    }

    #[test]
    fn grower_portal_home_renders_scoped_farms_fields_and_activity() {
        let mut control = ControlPlaneRegistry::default();
        let org_a = control
            .create_organization("Org A".to_string())
            .expect("org A creates");
        let org_b = control
            .create_organization("Org B".to_string())
            .expect("org B creates");
        let grower = control
            .create_user_with_role(
                org_a.org_id,
                "grower@example.com".to_string(),
                MembershipRole::Viewer,
            )
            .expect("grower creates");
        let mut farm_fields = FarmFieldRegistry::default();
        insert_test_farm_field_scope(&mut farm_fields, org_a.org_id, "farm-a", "field-a");
        insert_test_farm_field_scope(&mut farm_fields, org_a.org_id, "farm-c", "field-c");
        insert_test_farm_field_scope(&mut farm_fields, org_b.org_id, "farm-b", "field-b");
        let scope =
            resolve_grower_portal_session_scope(&control, &farm_fields, grower.user.user_id)
                .expect("grower scope resolves");

        let home = build_grower_portal_home(
            &scope,
            &farm_fields,
            &[
                GrowerFieldActivitySummary {
                    field_id: "field-a".to_string(),
                    last_scene_date: Some("2026-06-12".to_string()),
                    open_recommendation_count: 2,
                    evidence_refs: vec![
                        "scene:scene-a:captured_at:2026-06-12".to_string(),
                        "recommendation:field-a:open_count:2".to_string(),
                    ],
                },
                GrowerFieldActivitySummary {
                    field_id: "field-c".to_string(),
                    last_scene_date: None,
                    open_recommendation_count: 0,
                    evidence_refs: vec!["field:field-c:no_scene".to_string()],
                },
                GrowerFieldActivitySummary {
                    field_id: "field-b".to_string(),
                    last_scene_date: Some("2026-06-13".to_string()),
                    open_recommendation_count: 9,
                    evidence_refs: vec!["field:field-b:foreign".to_string()],
                },
            ],
        );

        assert!(!home.empty_state);
        assert_eq!(home.org_id, org_a.org_id);
        assert_eq!(
            home.farms
                .iter()
                .map(|farm| farm.farm_id.as_str())
                .collect::<Vec<_>>(),
            vec!["farm-a", "farm-c"]
        );
        assert_eq!(home.farms[0].fields[0].field_id, "field-a");
        assert_eq!(
            home.farms[0].fields[0].last_scene_date.as_deref(),
            Some("2026-06-12")
        );
        assert_eq!(home.farms[0].fields[0].open_recommendation_count, 2);
        assert!(home
            .farms
            .iter()
            .flat_map(|farm| farm.fields.iter())
            .all(|field| field.field_id != "field-b"));
    }

    #[test]
    fn grower_portal_home_renders_empty_state_for_empty_portfolio() {
        let grower_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let scope = GrowerPortalSessionScope {
            grower_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: Vec::new(),
            field_ids: Vec::new(),
        };
        let farm_fields = FarmFieldRegistry::default();

        let home = build_grower_portal_home(&scope, &farm_fields, &[]);

        assert!(home.empty_state);
        assert_eq!(home.grower_id, grower_id);
        assert!(home.farms.is_empty());
    }

    #[test]
    fn grower_portal_field_overview_shows_latest_completed_analysis() {
        let mut control = ControlPlaneRegistry::default();
        let org_a = control
            .create_organization("Org A".to_string())
            .expect("org A creates");
        let grower = control
            .create_user_with_role(
                org_a.org_id,
                "grower@example.com".to_string(),
                MembershipRole::Viewer,
            )
            .expect("grower creates");
        let mut farm_fields = FarmFieldRegistry::default();
        insert_test_farm_field_scope(&mut farm_fields, org_a.org_id, "farm-a", "field-a");
        let scope =
            resolve_grower_portal_session_scope(&control, &farm_fields, grower.user.user_id)
                .expect("grower scope resolves");

        let overview = build_grower_portal_field_overview(
            &scope,
            &farm_fields,
            "field-a",
            &[
                field_analysis_source(
                    "field-a",
                    "scene-old",
                    "2026-06-01",
                    GrowerFieldAnalysisStatus::Completed,
                ),
                field_analysis_source(
                    "field-a",
                    "scene-new",
                    "2026-06-12",
                    GrowerFieldAnalysisStatus::Completed,
                ),
            ],
        )
        .expect("field overview builds");

        assert!(!overview.no_current_analysis);
        assert_eq!(overview.latest_scene_id.as_deref(), Some("scene-new"));
        assert_eq!(overview.latest_scene_date.as_deref(), Some("2026-06-12"));
        assert_eq!(overview.layers[0].product_type, "ndvi");
        assert_eq!(
            overview
                .latest_finding
                .as_ref()
                .map(|finding| finding.finding_id.as_str()),
            Some("finding-scene-new")
        );
        assert!(overview
            .evidence_refs
            .iter()
            .any(|evidence| evidence == "scene:scene-new"));
    }

    #[test]
    fn grower_portal_field_overview_failed_latest_scene_has_no_current_analysis() {
        let mut control = ControlPlaneRegistry::default();
        let org_a = control
            .create_organization("Org A".to_string())
            .expect("org A creates");
        let grower = control
            .create_user_with_role(
                org_a.org_id,
                "grower@example.com".to_string(),
                MembershipRole::Viewer,
            )
            .expect("grower creates");
        let mut farm_fields = FarmFieldRegistry::default();
        insert_test_farm_field_scope(&mut farm_fields, org_a.org_id, "farm-a", "field-a");
        let scope =
            resolve_grower_portal_session_scope(&control, &farm_fields, grower.user.user_id)
                .expect("grower scope resolves");

        let overview = build_grower_portal_field_overview(
            &scope,
            &farm_fields,
            "field-a",
            &[
                field_analysis_source(
                    "field-a",
                    "scene-old",
                    "2026-06-01",
                    GrowerFieldAnalysisStatus::Completed,
                ),
                field_analysis_source(
                    "field-a",
                    "scene-failed",
                    "2026-06-12",
                    GrowerFieldAnalysisStatus::Failed,
                ),
            ],
        )
        .expect("field overview builds");

        assert!(overview.no_current_analysis);
        assert_eq!(overview.latest_scene_id, None);
        assert!(overview.layers.is_empty());
        assert_eq!(overview.latest_finding, None);
        assert!(overview
            .evidence_refs
            .iter()
            .any(|evidence| evidence == "scene:scene-failed"));
    }

    #[test]
    fn grower_portal_field_read_denies_cross_tenant_and_audits() {
        let mut control = ControlPlaneRegistry::default();
        let org_a = control
            .create_organization("Org A".to_string())
            .expect("org A creates");
        let org_b = control
            .create_organization("Org B".to_string())
            .expect("org B creates");
        let grower = control
            .create_user_with_role(
                org_a.org_id,
                "grower@example.com".to_string(),
                MembershipRole::Viewer,
            )
            .expect("grower creates");
        let mut farm_fields = FarmFieldRegistry::default();
        insert_test_farm_field_scope(&mut farm_fields, org_a.org_id, "farm-a", "field-a");
        insert_test_farm_field_scope(&mut farm_fields, org_b.org_id, "farm-b", "field-b");
        let scope =
            resolve_grower_portal_session_scope(&control, &farm_fields, grower.user.user_id)
                .expect("grower scope resolves");

        let allowed = control
            .read_grower_portal_field(&scope, &farm_fields, "field-a")
            .expect("owned field reads");
        assert_eq!(allowed.field_id, "field-a");

        let error = control
            .read_grower_portal_field(&scope, &farm_fields, "field-b")
            .expect_err("cross-tenant field read is denied");

        assert_eq!(error.status_code(), 403);
        let GrowerPortalAccessError::Forbidden { evidence } = error else {
            panic!("cross-tenant read returns forbidden evidence");
        };
        assert_eq!(evidence.grower_id, grower.user.user_id);
        assert_eq!(evidence.org_id, org_a.org_id);
        assert_eq!(evidence.field_id, "field-b");
        assert_eq!(evidence.target_org_id, Some(org_b.org_id.to_string()));
        assert_eq!(evidence.decision, AuthorizationDecision::Denied);
        assert_eq!(evidence.reason_code, "cross_tenant_field_read");

        let records = control.audit_records_for_org(org_a.org_id, None, None);
        let decisions = records
            .iter()
            .map(|record| (record.decision, record.reason_code.as_str()))
            .collect::<Vec<_>>();
        assert!(decisions.contains(&(AuthorizationDecision::Allowed, "allowed")));
        assert!(decisions.contains(&(AuthorizationDecision::Denied, "cross_tenant_field_read")));
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

    #[test]
    fn audit_records_are_append_only_and_queryable_by_org_and_time() {
        let mut registry = ControlPlaneRegistry::default();
        let org_a = Uuid::new_v4();
        let org_b = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let before = Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap();
        let inside = Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap();
        let after = Utc.with_ymd_and_hms(2026, 1, 1, 11, 0, 0).unwrap();
        let target_ref = Uuid::new_v4();

        registry.append_audit_record_at(
            AuditRecordRequest {
                actor_user_id: actor,
                org_id: org_a,
                action: ControlPlaneAction::ReadEntity,
                target_ref: Some(target_ref),
                target_org_id: Some(org_a),
                decision: AuthorizationDecision::Allowed,
                reason_code: "allowed".to_string(),
            },
            inside,
        );
        registry.append_audit_record_at(
            AuditRecordRequest {
                actor_user_id: actor,
                org_id: org_b,
                action: ControlPlaneAction::ReadEntity,
                target_ref: Some(Uuid::new_v4()),
                target_org_id: Some(org_b),
                decision: AuthorizationDecision::Allowed,
                reason_code: "allowed".to_string(),
            },
            inside,
        );
        registry.append_audit_record_at(
            AuditRecordRequest {
                actor_user_id: actor,
                org_id: org_a,
                action: ControlPlaneAction::ExportData,
                target_ref: None,
                target_org_id: Some(org_a),
                decision: AuthorizationDecision::Denied,
                reason_code: "role_not_permitted".to_string(),
            },
            after,
        );

        let records = registry.audit_records_for_org(org_a, Some(before), Some(inside));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].actor_user_id, actor);
        assert_eq!(records[0].org_id, org_a);
        assert_eq!(records[0].target_ref, Some(target_ref));
        assert_eq!(records[0].decision, AuthorizationDecision::Allowed);
        assert_eq!(records[0].at, inside);
    }

    #[test]
    fn audit_record_update_attempt_is_rejected() {
        let mut registry = ControlPlaneRegistry::default();
        let org_id = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let at = Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap();
        let record = registry.append_audit_record_at(
            AuditRecordRequest {
                actor_user_id: actor,
                org_id,
                action: ControlPlaneAction::ManageUsers,
                target_ref: None,
                target_org_id: Some(org_id),
                decision: AuthorizationDecision::Allowed,
                reason_code: "allowed".to_string(),
            },
            at,
        );

        let error = registry
            .update_audit_record(
                record.audit_id,
                AuditRecordRequest {
                    actor_user_id: actor,
                    org_id,
                    action: ControlPlaneAction::ManageUsers,
                    target_ref: None,
                    target_org_id: Some(org_id),
                    decision: AuthorizationDecision::Denied,
                    reason_code: "changed".to_string(),
                },
            )
            .expect_err("audit updates are rejected");
        let records = registry.audit_records_for_org(org_id, None, None);

        assert_eq!(
            error,
            AuditTrailError::AppendOnlyRecord {
                audit_id: record.audit_id
            }
        );
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].decision, AuthorizationDecision::Allowed);
        assert_eq!(records[0].reason_code, "allowed");
    }

    #[test]
    fn scoped_user_access_writes_allow_and_deny_audit_records() {
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

        registry
            .get_user_scoped(&principal, admin_a.user.user_id)
            .expect("same-org read succeeds");
        let _ = registry
            .get_user_scoped(&principal, user_b.user.user_id)
            .expect_err("cross-org read is hidden");

        let records = registry.audit_records_for_org(org_a.org_id, None, None);
        let decisions = records
            .iter()
            .map(|record| (record.decision, record.reason_code.as_str()))
            .collect::<Vec<_>>();

        assert!(decisions.contains(&(AuthorizationDecision::Allowed, "allowed")));
        assert!(decisions.contains(&(AuthorizationDecision::Denied, "cross_tenant_read")));
    }

    fn insert_test_farm_field_scope(
        registry: &mut FarmFieldRegistry,
        org_id: Uuid,
        farm_id: &str,
        field_id: &str,
    ) {
        registry
            .insert_farm(FarmRecord {
                farm_id: farm_id.to_string(),
                org_id: org_id.to_string(),
                owner: org_id.to_string(),
                name: farm_id.to_string(),
                notes: None,
                status: FarmFieldEntityStatus::Active,
                created_at: "2026-04-01T00:00:00Z".to_string(),
                updated_at: "2026-04-01T00:00:00Z".to_string(),
            })
            .expect("farm persists");
        registry
            .insert_field(FieldRecord {
                farm_id: Some(farm_id.to_string()),
                field_id: field_id.to_string(),
                org_id: org_id.to_string(),
                owner: org_id.to_string(),
                name: field_id.to_string(),
                area_ha: None,
                crop: None,
                season: None,
                notes: None,
                boundary: test_boundary(),
                extent: test_extent(),
                status: FarmFieldEntityStatus::Active,
                created_at: "2026-04-01T00:00:00Z".to_string(),
                updated_at: "2026-04-01T00:00:00Z".to_string(),
            })
            .expect("field persists");
    }

    fn field_analysis_source(
        field_id: &str,
        scene_id: &str,
        captured_at: &str,
        status: GrowerFieldAnalysisStatus,
    ) -> GrowerFieldAnalysisSource {
        GrowerFieldAnalysisSource {
            field_id: field_id.to_string(),
            scene_id: scene_id.to_string(),
            captured_at: captured_at.to_string(),
            status,
            layers: if status == GrowerFieldAnalysisStatus::Completed {
                vec![GrowerLayerSummary {
                    layer_id: format!("layer-{scene_id}"),
                    product_type: "ndvi".to_string(),
                    source_date: captured_at.to_string(),
                }]
            } else {
                Vec::new()
            },
            latest_finding: (status == GrowerFieldAnalysisStatus::Completed).then(|| {
                GrowerFindingSummary {
                    finding_id: format!("finding-{scene_id}"),
                    title: "Low vigor".to_string(),
                    source_date: captured_at.to_string(),
                }
            }),
            latest_recommendation: (status == GrowerFieldAnalysisStatus::Completed).then(|| {
                GrowerRecommendationSummary {
                    recommendation_id: format!("rec-{scene_id}"),
                    title: "Scout field edge".to_string(),
                    source_date: captured_at.to_string(),
                }
            }),
            evidence_refs: vec![format!("scene:{scene_id}")],
        }
    }

    fn test_boundary() -> FieldBoundary {
        FieldBoundary {
            crs: Some("EPSG:4326".to_string()),
            coordinates: vec![
                GeoPoint {
                    longitude: -96.5,
                    latitude: 41.2,
                },
                GeoPoint {
                    longitude: -96.2,
                    latitude: 41.2,
                },
                GeoPoint {
                    longitude: -96.2,
                    latitude: 41.4,
                },
                GeoPoint {
                    longitude: -96.5,
                    latitude: 41.4,
                },
                GeoPoint {
                    longitude: -96.5,
                    latitude: 41.2,
                },
            ],
        }
    }

    fn test_extent() -> GeoBounds {
        GeoBounds {
            min_lon: -96.5,
            min_lat: 41.2,
            max_lon: -96.2,
            max_lat: 41.4,
        }
    }
}
