use crate::schemas::{
    FarmFieldRegistry, FarmRecord, FieldRecord, GeoBounds, MarketplaceAccountRecord,
    MarketplaceAccountStatus, RecommendationLifecycleRegistry, RecommendationPriority,
    RecommendationRecord, RecommendationStatus, RecommendationStatusChangeRecord,
    RecommendationStatusChangeType, ReportRecord, SceneLayerRecord,
};
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
pub struct GrowerPortalPreferenceUpdate {
    pub default_farm_id: Option<String>,
    pub default_field_id: Option<String>,
    pub units: String,
    pub prefs: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerPortalPreferences {
    pub grower_id: Uuid,
    pub default_farm_id: Option<String>,
    pub default_field_id: Option<String>,
    pub units: String,
    pub prefs: HashMap<String, String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrowerPortalOpenView {
    Home,
    Field { field_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerPortalOpenViewResolution {
    pub grower_id: Uuid,
    pub view: GrowerPortalOpenView,
    pub notice: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerPortalPreferenceStore {
    preferences: HashMap<Uuid, GrowerPortalPreferences>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerReportInboxQuery {
    pub field_id: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

impl Default for GrowerReportInboxQuery {
    fn default() -> Self {
        Self {
            field_id: None,
            date_from: None,
            date_to: None,
            offset: 0,
            limit: 50,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerReportInboxRow {
    pub report_id: String,
    pub field_id: String,
    pub generated_at: String,
    pub status: String,
    pub title: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerReportInboxPage {
    pub rows: Vec<GrowerReportInboxRow>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerRecommendationRow {
    pub recommendation_id: String,
    pub field_id: String,
    pub priority: RecommendationPriority,
    pub status: RecommendationStatus,
    pub title: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrowerFieldMapOverlay {
    pub grower_id: Uuid,
    pub org_id: Uuid,
    pub field_id: String,
    pub layer_id: String,
    pub product_type: String,
    pub crs: String,
    pub field_extent: GeoBounds,
    pub layer_extent: GeoBounds,
    pub read_only: bool,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrowerNotificationEventType {
    ReportPublished,
    RecommendationCreated,
    RiskAlert,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerNotificationSourceEvent {
    pub event_type: GrowerNotificationEventType,
    pub source_ref: String,
    pub field_id: String,
    pub created_at: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerNotificationFeedItem {
    pub notification_id: String,
    pub event_type: GrowerNotificationEventType,
    pub source_ref: String,
    pub field_id: String,
    pub created_at: String,
    pub title: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerNotificationFeed {
    pub grower_id: Uuid,
    pub org_id: Uuid,
    pub items: Vec<GrowerNotificationFeedItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerRecommendationHistoryRow {
    pub recommendation_id: String,
    pub actor_user_id: String,
    pub before: Option<RecommendationStatus>,
    pub after: RecommendationStatus,
    pub at: String,
    pub change_type: RecommendationStatusChangeType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerRecommendationHistory {
    pub recommendation_id: String,
    pub field_id: String,
    pub rows: Vec<GrowerRecommendationHistoryRow>,
    pub empty_state: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerFieldSummaryFindingRow {
    pub field_id: String,
    pub finding: GrowerFindingSummary,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerFieldSummaryExportRequest {
    pub field_id: String,
    pub generated_at: String,
    pub findings: Vec<GrowerFieldSummaryFindingRow>,
    pub recommendations: Vec<GrowerRecommendationRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerFieldSummaryExportArtifact {
    pub export_id: String,
    pub grower_id: Uuid,
    pub org_id: Uuid,
    pub field_id: String,
    pub generated_at: String,
    pub findings: Vec<GrowerFieldSummaryFindingRow>,
    pub recommendations: Vec<GrowerRecommendationRow>,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerFieldSummaryShareRecord {
    pub share_id: String,
    pub export_id: String,
    pub grower_id: Uuid,
    pub org_id: Uuid,
    pub field_id: String,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerFieldSummaryExportStore {
    exports: HashMap<String, GrowerFieldSummaryExportArtifact>,
    shares: HashMap<String, GrowerFieldSummaryShareRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrowerMobileConnectivity {
    Online,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerMobileAppShell {
    pub grower_id: Uuid,
    pub org_id: Uuid,
    pub home: GrowerPortalHome,
    pub field_overviews: Vec<GrowerPortalFieldOverview>,
    pub report_inbox: GrowerReportInboxPage,
    pub notification_feed: GrowerNotificationFeed,
    pub connectivity: GrowerMobileConnectivity,
    pub loaded_at: String,
    pub rendered_at: String,
    pub freshness_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerMarketplaceEntry {
    pub grower_id: Uuid,
    pub org_id: Uuid,
    pub account_id: String,
    pub href: String,
    pub identity_context: HashMap<String, String>,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowerMarketplaceEntryResolution {
    pub entry: Option<GrowerMarketplaceEntry>,
    pub hidden_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GrowerFieldMapError {
    #[error(transparent)]
    Access(#[from] GrowerPortalAccessError),
    #[error("layer {layer_id} is not in scope for field {field_id}")]
    LayerOutOfScope { field_id: String, layer_id: String },
    #[error("layer {layer_id} missing CRS")]
    MissingLayerCrs { layer_id: String },
    #[error("field {field_id} missing boundary CRS")]
    MissingFieldCrs { field_id: String },
    #[error("layer {layer_id} missing extent")]
    MissingLayerExtent { layer_id: String },
    #[error("CRS mismatch for field {field_id} and layer {layer_id}: field {field_crs} != layer {layer_crs}")]
    CrsMismatch {
        field_id: String,
        layer_id: String,
        field_crs: String,
        layer_crs: String,
    },
    #[error("extent mismatch for field {field_id} and layer {layer_id}: {edge} field={field_value} layer={layer_value}")]
    ExtentMismatch {
        field_id: String,
        layer_id: String,
        edge: &'static str,
        field_value: f64,
        layer_value: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GrowerFieldSummaryShareError {
    #[error(transparent)]
    Access(#[from] GrowerPortalAccessError),
    #[error("export not found: {export_id}")]
    ExportNotFound { export_id: String },
    #[error("share not found: {share_id}")]
    ShareNotFound { share_id: String },
    #[error("share revoked: {share_id}")]
    ShareRevoked { share_id: String },
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

impl GrowerPortalPreferenceStore {
    pub fn save_preferences(
        &mut self,
        scope: &GrowerPortalSessionScope,
        update: GrowerPortalPreferenceUpdate,
        updated_at: impl Into<String>,
    ) -> GrowerPortalPreferences {
        let default_farm_id = update
            .default_farm_id
            .filter(|farm_id| scope.farm_ids.iter().any(|scoped| scoped == farm_id));
        let default_field_id = update
            .default_field_id
            .filter(|field_id| scope.field_ids.iter().any(|scoped| scoped == field_id));
        let units = if update.units.trim().is_empty() {
            "metric".to_string()
        } else {
            update.units.trim().to_string()
        };
        let preferences = GrowerPortalPreferences {
            grower_id: scope.grower_id,
            default_farm_id,
            default_field_id,
            units,
            prefs: update.prefs,
            updated_at: updated_at.into(),
        };
        self.preferences
            .insert(scope.grower_id, preferences.clone());
        preferences
    }

    pub fn preferences_for(&self, grower_id: Uuid) -> Option<&GrowerPortalPreferences> {
        self.preferences.get(&grower_id)
    }

    pub fn resolve_open_view(
        &self,
        scope: &GrowerPortalSessionScope,
    ) -> GrowerPortalOpenViewResolution {
        let Some(preferences) = self.preferences.get(&scope.grower_id) else {
            return GrowerPortalOpenViewResolution {
                grower_id: scope.grower_id,
                view: GrowerPortalOpenView::Home,
                notice: None,
            };
        };
        let Some(field_id) = preferences.default_field_id.as_ref() else {
            return GrowerPortalOpenViewResolution {
                grower_id: scope.grower_id,
                view: GrowerPortalOpenView::Home,
                notice: None,
            };
        };
        if scope.field_ids.iter().any(|scoped| scoped == field_id) {
            GrowerPortalOpenViewResolution {
                grower_id: scope.grower_id,
                view: GrowerPortalOpenView::Field {
                    field_id: field_id.clone(),
                },
                notice: None,
            }
        } else {
            GrowerPortalOpenViewResolution {
                grower_id: scope.grower_id,
                view: GrowerPortalOpenView::Home,
                notice: Some(format!(
                    "Default field {field_id} is no longer in scope; opening home"
                )),
            }
        }
    }
}

pub fn list_grower_report_inbox(
    scope: &GrowerPortalSessionScope,
    reports: &[ReportRecord],
    query: GrowerReportInboxQuery,
) -> GrowerReportInboxPage {
    let scoped_field_ids = scope
        .field_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let query_field = query.field_id.as_deref();
    let mut rows = reports
        .iter()
        .filter_map(|report| {
            let field_id = report.field_id.as_deref()?;
            if !scoped_field_ids.contains(field_id) {
                return None;
            }
            if query_field.is_some_and(|requested| requested != field_id) {
                return None;
            }
            if query
                .date_from
                .as_deref()
                .is_some_and(|from| report.created_at.as_str() < from)
            {
                return None;
            }
            if query
                .date_to
                .as_deref()
                .is_some_and(|to| report.created_at.as_str() > to)
            {
                return None;
            }
            Some(grower_report_row(report, field_id))
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .generated_at
            .cmp(&left.generated_at)
            .then(left.report_id.cmp(&right.report_id))
    });
    let total = rows.len();
    let limit = query.limit.max(1);
    let rows = rows.into_iter().skip(query.offset).take(limit).collect();

    GrowerReportInboxPage {
        rows,
        total,
        offset: query.offset,
        limit,
    }
}

pub fn read_grower_report(
    scope: &GrowerPortalSessionScope,
    reports: &[ReportRecord],
    report_id: &str,
) -> Result<ReportRecord, GrowerPortalAccessError> {
    let report = reports
        .iter()
        .find(|report| report.report_id == report_id)
        .ok_or_else(|| GrowerPortalAccessError::FieldNotFound {
            field_id: report_id.to_string(),
        })?;
    let Some(field_id) = report.field_id.as_deref() else {
        return Err(GrowerPortalAccessError::FieldNotFound {
            field_id: report_id.to_string(),
        });
    };
    if !scope.field_ids.iter().any(|scoped| scoped == field_id) {
        return Err(GrowerPortalAccessError::Forbidden {
            evidence: GrowerPortalAccessEvidence {
                grower_id: scope.grower_id,
                org_id: scope.org_id,
                role: scope.role,
                action: ControlPlaneAction::ReadEntity,
                field_id: field_id.to_string(),
                target_org_id: Some(report.org_id.clone()),
                decision: AuthorizationDecision::Denied,
                reason_code: "report_out_of_scope".to_string(),
            },
        });
    }
    Ok(report.clone())
}

fn grower_report_row(report: &ReportRecord, field_id: &str) -> GrowerReportInboxRow {
    GrowerReportInboxRow {
        report_id: report.report_id.clone(),
        field_id: field_id.to_string(),
        generated_at: report.created_at.clone(),
        status: format!("{:?}", report.visibility),
        title: report.title.clone(),
        evidence_refs: report.source_refs.clone(),
    }
}

pub fn list_grower_recommendations(
    scope: &GrowerPortalSessionScope,
    recommendations: &[RecommendationRecord],
) -> Vec<GrowerRecommendationRow> {
    let scoped_field_ids = scope
        .field_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let mut rows = recommendations
        .iter()
        .filter_map(|recommendation| {
            let field_id = recommendation.field_id.as_deref()?;
            scoped_field_ids
                .contains(field_id)
                .then(|| grower_recommendation_row(recommendation, field_id))
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.recommendation_id.cmp(&right.recommendation_id));
    rows
}

pub fn acknowledge_grower_recommendation(
    scope: &GrowerPortalSessionScope,
    registry: &mut RecommendationLifecycleRegistry,
    recommendation_id: &str,
    at: &str,
) -> Result<RecommendationRecord, GrowerPortalAccessError> {
    let org_id = scope.org_id.to_string();
    let recommendation = registry
        .recommendations_for_org(&org_id)
        .into_iter()
        .find(|recommendation| recommendation.recommendation_id == recommendation_id)
        .ok_or_else(|| GrowerPortalAccessError::FieldNotFound {
            field_id: recommendation_id.to_string(),
        })?;
    let field_id =
        recommendation
            .field_id
            .clone()
            .ok_or_else(|| GrowerPortalAccessError::FieldNotFound {
                field_id: recommendation_id.to_string(),
            })?;
    if !scope.field_ids.iter().any(|scoped| scoped == &field_id) {
        return Err(GrowerPortalAccessError::Forbidden {
            evidence: GrowerPortalAccessEvidence {
                grower_id: scope.grower_id,
                org_id: scope.org_id,
                role: scope.role,
                action: ControlPlaneAction::WriteEntity,
                field_id,
                target_org_id: Some(recommendation.org_id),
                decision: AuthorizationDecision::Denied,
                reason_code: "recommendation_out_of_scope".to_string(),
            },
        });
    }

    registry
        .transition_recommendation_status(
            &org_id,
            recommendation_id,
            &scope.grower_id.to_string(),
            at,
            RecommendationStatus::Reviewed,
        )
        .map_err(|_| GrowerPortalAccessError::FieldNotFound {
            field_id: recommendation_id.to_string(),
        })
}

fn grower_recommendation_row(
    recommendation: &RecommendationRecord,
    field_id: &str,
) -> GrowerRecommendationRow {
    GrowerRecommendationRow {
        recommendation_id: recommendation.recommendation_id.clone(),
        field_id: field_id.to_string(),
        priority: recommendation.priority,
        status: recommendation.status,
        title: recommendation.title.clone(),
        evidence_refs: recommendation.evidence_refs.clone(),
    }
}

pub fn build_grower_field_map_overlay(
    scope: &GrowerPortalSessionScope,
    farm_fields: &FarmFieldRegistry,
    field_id: &str,
    layer: &SceneLayerRecord,
) -> Result<GrowerFieldMapOverlay, GrowerFieldMapError> {
    let org_key = scope.org_id.to_string();
    let field = farm_fields
        .fields_for_org(&org_key)
        .into_iter()
        .find(|field| field.field_id == field_id)
        .ok_or_else(|| GrowerPortalAccessError::FieldNotFound {
            field_id: field_id.to_string(),
        })?;
    if !scope.field_ids.iter().any(|scoped| scoped == field_id) {
        return Err(GrowerPortalAccessError::Forbidden {
            evidence: GrowerPortalAccessEvidence {
                grower_id: scope.grower_id,
                org_id: scope.org_id,
                role: scope.role,
                action: ControlPlaneAction::ReadEntity,
                field_id: field_id.to_string(),
                target_org_id: Some(field.org_id.clone()),
                decision: AuthorizationDecision::Denied,
                reason_code: "field_out_of_scope".to_string(),
            },
        }
        .into());
    }
    if layer.scene_id.trim().is_empty() || layer.layer_id.trim().is_empty() {
        return Err(GrowerFieldMapError::LayerOutOfScope {
            field_id: field_id.to_string(),
            layer_id: layer.layer_id.clone(),
        });
    }
    let field_crs = field
        .boundary
        .crs
        .as_deref()
        .map(str::trim)
        .filter(|crs| !crs.is_empty())
        .ok_or_else(|| GrowerFieldMapError::MissingFieldCrs {
            field_id: field_id.to_string(),
        })?;
    let layer_crs = layer
        .crs
        .trim()
        .is_empty()
        .then_some(())
        .map_or(Some(layer.crs.trim()), |_| None)
        .ok_or_else(|| GrowerFieldMapError::MissingLayerCrs {
            layer_id: layer.layer_id.clone(),
        })?;
    if field_crs != layer_crs {
        return Err(GrowerFieldMapError::CrsMismatch {
            field_id: field_id.to_string(),
            layer_id: layer.layer_id.clone(),
            field_crs: field_crs.to_string(),
            layer_crs: layer_crs.to_string(),
        });
    }
    let layer_extent =
        layer
            .extent
            .clone()
            .ok_or_else(|| GrowerFieldMapError::MissingLayerExtent {
                layer_id: layer.layer_id.clone(),
            })?;
    assert_overlay_extent_edge(
        field_id,
        &layer.layer_id,
        "min_lon",
        field.extent.min_lon,
        layer_extent.min_lon,
    )?;
    assert_overlay_extent_edge(
        field_id,
        &layer.layer_id,
        "min_lat",
        field.extent.min_lat,
        layer_extent.min_lat,
    )?;
    assert_overlay_extent_edge(
        field_id,
        &layer.layer_id,
        "max_lon",
        field.extent.max_lon,
        layer_extent.max_lon,
    )?;
    assert_overlay_extent_edge(
        field_id,
        &layer.layer_id,
        "max_lat",
        field.extent.max_lat,
        layer_extent.max_lat,
    )?;

    Ok(GrowerFieldMapOverlay {
        grower_id: scope.grower_id,
        org_id: scope.org_id,
        field_id: field.field_id,
        layer_id: layer.layer_id.clone(),
        product_type: layer.product_type.clone(),
        crs: field_crs.to_string(),
        field_extent: field.extent,
        layer_extent,
        read_only: true,
        evidence_refs: vec![
            format!("field:{field_id}:boundary"),
            format!("layer:{}:{}", layer.layer_id, layer.product_type),
        ],
    })
}

fn assert_overlay_extent_edge(
    field_id: &str,
    layer_id: &str,
    edge: &'static str,
    field_value: f64,
    layer_value: f64,
) -> Result<(), GrowerFieldMapError> {
    if (field_value - layer_value).abs() <= crate::schemas::GEO_EXTENT_ASSERTION_TOLERANCE {
        Ok(())
    } else {
        Err(GrowerFieldMapError::ExtentMismatch {
            field_id: field_id.to_string(),
            layer_id: layer_id.to_string(),
            edge,
            field_value,
            layer_value,
        })
    }
}

pub fn build_grower_notification_feed(
    scope: &GrowerPortalSessionScope,
    events: &[GrowerNotificationSourceEvent],
) -> GrowerNotificationFeed {
    let scoped_field_ids = scope
        .field_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let mut items = events
        .iter()
        .filter(|event| scoped_field_ids.contains(event.field_id.as_str()))
        .map(grower_notification_item)
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then(left.notification_id.cmp(&right.notification_id))
    });

    GrowerNotificationFeed {
        grower_id: scope.grower_id,
        org_id: scope.org_id,
        items,
    }
}

fn grower_notification_item(event: &GrowerNotificationSourceEvent) -> GrowerNotificationFeedItem {
    GrowerNotificationFeedItem {
        notification_id: format!(
            "notification:{}:{}:{}",
            event.field_id, event.source_ref, event.created_at
        ),
        event_type: event.event_type,
        source_ref: event.source_ref.clone(),
        field_id: event.field_id.clone(),
        created_at: event.created_at.clone(),
        title: event.title.clone(),
        evidence_refs: vec![format!(
            "event:{:?}:{}:{}",
            event.event_type, event.source_ref, event.created_at
        )],
    }
}

pub fn grower_recommendation_history(
    scope: &GrowerPortalSessionScope,
    registry: &RecommendationLifecycleRegistry,
    recommendation_id: &str,
) -> Result<GrowerRecommendationHistory, GrowerPortalAccessError> {
    let org_id = scope.org_id.to_string();
    let recommendation = registry
        .recommendations_for_org(&org_id)
        .into_iter()
        .find(|recommendation| recommendation.recommendation_id == recommendation_id)
        .ok_or_else(|| GrowerPortalAccessError::FieldNotFound {
            field_id: recommendation_id.to_string(),
        })?;
    let field_id =
        recommendation
            .field_id
            .clone()
            .ok_or_else(|| GrowerPortalAccessError::FieldNotFound {
                field_id: recommendation_id.to_string(),
            })?;
    if !scope.field_ids.iter().any(|scoped| scoped == &field_id) {
        return Err(GrowerPortalAccessError::Forbidden {
            evidence: GrowerPortalAccessEvidence {
                grower_id: scope.grower_id,
                org_id: scope.org_id,
                role: scope.role,
                action: ControlPlaneAction::ReadEntity,
                field_id,
                target_org_id: Some(recommendation.org_id),
                decision: AuthorizationDecision::Denied,
                reason_code: "recommendation_history_out_of_scope".to_string(),
            },
        });
    }
    let mut rows = registry
        .recommendation_history(&org_id, recommendation_id)
        .into_iter()
        .collect::<Vec<_>>();

    Ok(build_grower_recommendation_history_view(
        recommendation_id,
        field_id,
        &mut rows,
    ))
}

pub fn build_grower_recommendation_history_view(
    recommendation_id: &str,
    field_id: String,
    changes: &mut [RecommendationStatusChangeRecord],
) -> GrowerRecommendationHistory {
    changes.sort_by(|left, right| left.at.cmp(&right.at));
    let rows = changes
        .iter()
        .cloned()
        .map(grower_recommendation_history_row)
        .collect::<Vec<_>>();
    GrowerRecommendationHistory {
        recommendation_id: recommendation_id.to_string(),
        field_id,
        empty_state: rows.is_empty(),
        rows,
    }
}

fn grower_recommendation_history_row(
    change: RecommendationStatusChangeRecord,
) -> GrowerRecommendationHistoryRow {
    GrowerRecommendationHistoryRow {
        recommendation_id: change.recommendation_id,
        actor_user_id: change.actor_user_id,
        before: change.before,
        after: change.after,
        at: change.at,
        change_type: change.change_type,
    }
}

impl GrowerFieldSummaryExportStore {
    pub fn export_field_summary(
        &mut self,
        control: &mut ControlPlaneRegistry,
        scope: &GrowerPortalSessionScope,
        request: GrowerFieldSummaryExportRequest,
    ) -> Result<GrowerFieldSummaryExportArtifact, GrowerPortalAccessError> {
        let field_id = request.field_id.trim();
        if field_id.is_empty() {
            audit_grower_field_summary_export(
                control,
                scope,
                "",
                AuthorizationDecision::Denied,
                "grower_field_summary_export_missing_field",
            );
            return Err(GrowerPortalAccessError::FieldNotFound {
                field_id: String::new(),
            });
        }
        if !scope.field_ids.iter().any(|scoped| scoped == field_id) {
            return Err(grower_field_summary_export_forbidden(
                control,
                scope,
                field_id,
                "grower_field_summary_export_field_out_of_scope",
            ));
        }

        if let Some(out_of_scope) = request
            .findings
            .iter()
            .map(|finding| finding.field_id.as_str())
            .chain(
                request
                    .recommendations
                    .iter()
                    .map(|recommendation| recommendation.field_id.as_str()),
            )
            .find(|candidate| {
                *candidate != field_id || !scope.field_ids.iter().any(|scoped| scoped == *candidate)
            })
        {
            return Err(grower_field_summary_export_forbidden(
                control,
                scope,
                out_of_scope,
                "grower_field_summary_export_contains_out_of_scope_data",
            ));
        }

        let export_id = format!(
            "grower-export:{}:{}:{}",
            scope.grower_id, field_id, request.generated_at
        );
        let evidence_refs = grower_field_summary_export_evidence_refs(&request);
        let artifact = GrowerFieldSummaryExportArtifact {
            export_id: export_id.clone(),
            grower_id: scope.grower_id,
            org_id: scope.org_id,
            field_id: field_id.to_string(),
            generated_at: request.generated_at,
            findings: request.findings,
            recommendations: request.recommendations,
            evidence_refs,
        };

        audit_grower_field_summary_export(
            control,
            scope,
            field_id,
            AuthorizationDecision::Allowed,
            "grower_field_summary_exported",
        );
        self.exports.insert(export_id, artifact.clone());
        Ok(artifact)
    }

    pub fn create_share(
        &mut self,
        control: &mut ControlPlaneRegistry,
        scope: &GrowerPortalSessionScope,
        export_id: &str,
        created_at: &str,
    ) -> Result<GrowerFieldSummaryShareRecord, GrowerFieldSummaryShareError> {
        let artifact = self.exports.get(export_id).ok_or_else(|| {
            audit_grower_field_summary_export(
                control,
                scope,
                "",
                AuthorizationDecision::Denied,
                "grower_field_summary_share_export_not_found",
            );
            GrowerFieldSummaryShareError::ExportNotFound {
                export_id: export_id.to_string(),
            }
        })?;
        ensure_export_visible_to_scope(
            control,
            scope,
            artifact,
            "grower_field_summary_share_out_of_scope",
        )?;

        let share_id = format!("grower-share:{}:{}", artifact.export_id, created_at);
        let share = GrowerFieldSummaryShareRecord {
            share_id: share_id.clone(),
            export_id: artifact.export_id.clone(),
            grower_id: scope.grower_id,
            org_id: scope.org_id,
            field_id: artifact.field_id.clone(),
            created_at: created_at.to_string(),
            revoked_at: None,
        };
        audit_grower_field_summary_export(
            control,
            scope,
            &artifact.field_id,
            AuthorizationDecision::Allowed,
            "grower_field_summary_share_created",
        );
        self.shares.insert(share_id, share.clone());
        Ok(share)
    }

    pub fn revoke_share(
        &mut self,
        control: &mut ControlPlaneRegistry,
        scope: &GrowerPortalSessionScope,
        share_id: &str,
        revoked_at: &str,
    ) -> Result<GrowerFieldSummaryShareRecord, GrowerFieldSummaryShareError> {
        let share = self.shares.get_mut(share_id).ok_or_else(|| {
            GrowerFieldSummaryShareError::ShareNotFound {
                share_id: share_id.to_string(),
            }
        })?;
        if share.org_id != scope.org_id
            || !scope
                .field_ids
                .iter()
                .any(|field_id| field_id == &share.field_id)
        {
            return Err(GrowerFieldSummaryShareError::Access(
                grower_field_summary_export_forbidden(
                    control,
                    scope,
                    &share.field_id,
                    "grower_field_summary_share_revoke_out_of_scope",
                ),
            ));
        }
        share.revoked_at = Some(revoked_at.to_string());
        audit_grower_field_summary_export(
            control,
            scope,
            &share.field_id,
            AuthorizationDecision::Allowed,
            "grower_field_summary_share_revoked",
        );
        Ok(share.clone())
    }

    pub fn read_shared_export(
        &mut self,
        control: &mut ControlPlaneRegistry,
        scope: &GrowerPortalSessionScope,
        share_id: &str,
    ) -> Result<GrowerFieldSummaryExportArtifact, GrowerFieldSummaryShareError> {
        let share = self.shares.get(share_id).ok_or_else(|| {
            GrowerFieldSummaryShareError::ShareNotFound {
                share_id: share_id.to_string(),
            }
        })?;
        if let Some(_revoked_at) = &share.revoked_at {
            audit_grower_field_summary_export(
                control,
                scope,
                &share.field_id,
                AuthorizationDecision::Denied,
                "grower_field_summary_share_revoked",
            );
            return Err(GrowerFieldSummaryShareError::ShareRevoked {
                share_id: share_id.to_string(),
            });
        }
        if share.org_id != scope.org_id
            || !scope
                .field_ids
                .iter()
                .any(|field_id| field_id == &share.field_id)
        {
            return Err(GrowerFieldSummaryShareError::Access(
                grower_field_summary_export_forbidden(
                    control,
                    scope,
                    &share.field_id,
                    "grower_field_summary_share_read_out_of_scope",
                ),
            ));
        }

        let artifact = self.exports.get(&share.export_id).ok_or_else(|| {
            GrowerFieldSummaryShareError::ExportNotFound {
                export_id: share.export_id.clone(),
            }
        })?;
        audit_grower_field_summary_export(
            control,
            scope,
            &artifact.field_id,
            AuthorizationDecision::Allowed,
            "grower_field_summary_share_read",
        );
        Ok(artifact.clone())
    }
}

fn ensure_export_visible_to_scope(
    control: &mut ControlPlaneRegistry,
    scope: &GrowerPortalSessionScope,
    artifact: &GrowerFieldSummaryExportArtifact,
    reason_code: &str,
) -> Result<(), GrowerFieldSummaryShareError> {
    if artifact.org_id == scope.org_id
        && artifact.grower_id == scope.grower_id
        && scope
            .field_ids
            .iter()
            .any(|field_id| field_id == &artifact.field_id)
    {
        return Ok(());
    }
    Err(GrowerFieldSummaryShareError::Access(
        grower_field_summary_export_forbidden(control, scope, &artifact.field_id, reason_code),
    ))
}

fn grower_field_summary_export_evidence_refs(
    request: &GrowerFieldSummaryExportRequest,
) -> Vec<String> {
    let mut refs = Vec::new();
    for finding in &request.findings {
        refs.extend(finding.evidence_refs.iter().cloned());
        refs.push(format!("finding:{}", finding.finding.finding_id));
    }
    for recommendation in &request.recommendations {
        refs.extend(recommendation.evidence_refs.iter().cloned());
        refs.push(format!(
            "recommendation:{}",
            recommendation.recommendation_id
        ));
    }
    refs.sort();
    refs.dedup();
    refs
}

fn grower_field_summary_export_forbidden(
    control: &mut ControlPlaneRegistry,
    scope: &GrowerPortalSessionScope,
    field_id: &str,
    reason_code: &str,
) -> GrowerPortalAccessError {
    audit_grower_field_summary_export(
        control,
        scope,
        field_id,
        AuthorizationDecision::Denied,
        reason_code,
    );
    GrowerPortalAccessError::Forbidden {
        evidence: GrowerPortalAccessEvidence {
            grower_id: scope.grower_id,
            org_id: scope.org_id,
            role: scope.role,
            action: ControlPlaneAction::ExportData,
            field_id: field_id.to_string(),
            target_org_id: Some(scope.org_id.to_string()),
            decision: AuthorizationDecision::Denied,
            reason_code: reason_code.to_string(),
        },
    }
}

fn audit_grower_field_summary_export(
    control: &mut ControlPlaneRegistry,
    scope: &GrowerPortalSessionScope,
    _field_id: &str,
    decision: AuthorizationDecision,
    reason_code: &str,
) {
    control.append_audit_record(AuditRecordRequest {
        actor_user_id: scope.grower_id,
        org_id: scope.org_id,
        action: ControlPlaneAction::ExportData,
        target_ref: None,
        target_org_id: Some(scope.org_id),
        decision,
        reason_code: reason_code.to_string(),
    });
}

pub fn build_grower_mobile_app_shell(
    scope: &GrowerPortalSessionScope,
    farm_fields: &FarmFieldRegistry,
    activity: &[GrowerFieldActivitySummary],
    analysis_sources: &[GrowerFieldAnalysisSource],
    reports: &[ReportRecord],
    report_query: GrowerReportInboxQuery,
    notification_events: &[GrowerNotificationSourceEvent],
    loaded_at: &str,
) -> Result<GrowerMobileAppShell, GrowerPortalAccessError> {
    let home = build_grower_portal_home(scope, farm_fields, activity);
    let mut field_overviews = scope
        .field_ids
        .iter()
        .map(|field_id| {
            build_grower_portal_field_overview(scope, farm_fields, field_id, analysis_sources)
        })
        .collect::<Result<Vec<_>, _>>()?;
    field_overviews.sort_by(|left, right| left.field_id.cmp(&right.field_id));
    let report_inbox = list_grower_report_inbox(scope, reports, report_query);
    let notification_feed = build_grower_notification_feed(scope, notification_events);

    Ok(GrowerMobileAppShell {
        grower_id: scope.grower_id,
        org_id: scope.org_id,
        home,
        field_overviews,
        report_inbox,
        notification_feed,
        connectivity: GrowerMobileConnectivity::Online,
        loaded_at: loaded_at.to_string(),
        rendered_at: loaded_at.to_string(),
        freshness_label: None,
    })
}

pub fn build_grower_mobile_offline_shell(
    cached: &GrowerMobileAppShell,
    opened_at: &str,
) -> GrowerMobileAppShell {
    let mut shell = cached.clone();
    shell.connectivity = GrowerMobileConnectivity::Offline;
    shell.rendered_at = opened_at.to_string();
    shell.freshness_label = Some(format!("offline / as of {}", cached.loaded_at));
    shell
}

pub fn resolve_grower_marketplace_entry(
    scope: &GrowerPortalSessionScope,
    marketplace_accounts: &[MarketplaceAccountRecord],
    marketplace_base_url: &str,
) -> GrowerMarketplaceEntryResolution {
    let org_id = scope.org_id.to_string();
    let account = marketplace_accounts
        .iter()
        .filter(|account| account.org_id == org_id)
        .filter(|account| account.status == MarketplaceAccountStatus::Active)
        .filter(|account| {
            account
                .role_refs
                .iter()
                .any(|role_ref| role_ref.starts_with("marketplace:"))
        })
        .min_by(|left, right| left.account_id.cmp(&right.account_id));

    let Some(account) = account else {
        return GrowerMarketplaceEntryResolution {
            entry: None,
            hidden_reason: Some("marketplace_disabled".to_string()),
        };
    };

    let mut identity_context = HashMap::new();
    identity_context.insert("grower_id".to_string(), scope.grower_id.to_string());
    identity_context.insert("org_id".to_string(), scope.org_id.to_string());
    identity_context.insert("account_id".to_string(), account.account_id.clone());
    let href = format!(
        "{}/orgs/{}/marketplace?grower_id={}&account_id={}",
        marketplace_base_url.trim_end_matches('/'),
        scope.org_id,
        scope.grower_id,
        account.account_id
    );

    GrowerMarketplaceEntryResolution {
        entry: Some(GrowerMarketplaceEntry {
            grower_id: scope.grower_id,
            org_id: scope.org_id,
            account_id: account.account_id.clone(),
            href,
            identity_context,
            evidence_refs: vec![
                format!("marketplace-account:{}", account.account_id),
                format!("org:{}", scope.org_id),
            ],
        }),
        hidden_reason: None,
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
        GeoBounds, GeoPoint, RecommendationLifecycleRegistry, RecommendationPriority,
        RecommendationRecord, RecommendationStatus, RecommendationStatusChangeType, ReportFormat,
        ReportRecord, ReportVisibility, SceneLayerRecord,
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
    fn grower_portal_preferences_save_and_open_default_field_for_same_grower() {
        let grower_id = Uuid::new_v4();
        let other_grower_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let scope = GrowerPortalSessionScope {
            grower_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };
        let mut store = GrowerPortalPreferenceStore::default();
        let mut prefs = HashMap::new();
        prefs.insert("map_style".to_string(), "satellite".to_string());

        let saved = store.save_preferences(
            &scope,
            GrowerPortalPreferenceUpdate {
                default_farm_id: Some("farm-a".to_string()),
                default_field_id: Some("field-a".to_string()),
                units: "imperial".to_string(),
                prefs,
            },
            "2026-06-15T13:04:00Z",
        );

        assert_eq!(saved.grower_id, grower_id);
        assert_eq!(saved.default_field_id.as_deref(), Some("field-a"));
        assert_eq!(
            store.resolve_open_view(&scope).view,
            GrowerPortalOpenView::Field {
                field_id: "field-a".to_string()
            }
        );
        assert!(store.preferences_for(other_grower_id).is_none());
    }

    #[test]
    fn grower_portal_preferences_fall_back_home_when_default_field_leaves_scope() {
        let grower_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let original_scope = GrowerPortalSessionScope {
            grower_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };
        let reduced_scope = GrowerPortalSessionScope {
            grower_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: Vec::new(),
        };
        let mut store = GrowerPortalPreferenceStore::default();
        store.save_preferences(
            &original_scope,
            GrowerPortalPreferenceUpdate {
                default_farm_id: Some("farm-a".to_string()),
                default_field_id: Some("field-a".to_string()),
                units: "metric".to_string(),
                prefs: HashMap::new(),
            },
            "2026-06-15T13:04:00Z",
        );

        let resolved = store.resolve_open_view(&reduced_scope);

        assert_eq!(resolved.view, GrowerPortalOpenView::Home);
        assert!(resolved
            .notice
            .as_deref()
            .is_some_and(|notice| notice.contains("no longer in scope")));
    }

    #[test]
    fn grower_report_inbox_lists_newest_first_with_filters_and_pagination() {
        let grower_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let scope = GrowerPortalSessionScope {
            grower_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string(), "field-c".to_string()],
        };
        let reports = vec![
            report_record("report-old", "field-a", org_id, "2026-06-01T00:00:00Z"),
            report_record("report-new", "field-a", org_id, "2026-06-12T00:00:00Z"),
            report_record(
                "report-other-field",
                "field-c",
                org_id,
                "2026-06-10T00:00:00Z",
            ),
            report_record(
                "report-foreign",
                "field-b",
                Uuid::new_v4(),
                "2026-06-13T00:00:00Z",
            ),
        ];

        let page = list_grower_report_inbox(
            &scope,
            &reports,
            GrowerReportInboxQuery {
                field_id: Some("field-a".to_string()),
                date_from: Some("2026-06-01T00:00:00Z".to_string()),
                date_to: Some("2026-06-30T00:00:00Z".to_string()),
                offset: 0,
                limit: 1,
            },
        );

        assert_eq!(page.total, 2);
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].report_id, "report-new");
        assert_eq!(page.rows[0].field_id, "field-a");
        assert_eq!(page.rows[0].generated_at, "2026-06-12T00:00:00Z");
    }

    #[test]
    fn grower_report_read_denies_out_of_scope_report() {
        let grower_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let foreign_org_id = Uuid::new_v4();
        let scope = GrowerPortalSessionScope {
            grower_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };
        let reports = vec![
            report_record("report-owned", "field-a", org_id, "2026-06-01T00:00:00Z"),
            report_record(
                "report-foreign",
                "field-b",
                foreign_org_id,
                "2026-06-12T00:00:00Z",
            ),
        ];

        let owned =
            read_grower_report(&scope, &reports, "report-owned").expect("owned report should read");
        assert_eq!(owned.report_id, "report-owned");

        let error = read_grower_report(&scope, &reports, "report-foreign")
            .expect_err("foreign report should be denied");
        let GrowerPortalAccessError::Forbidden { evidence } = error else {
            panic!("foreign report returns forbidden evidence");
        };
        assert_eq!(evidence.reason_code, "report_out_of_scope");
        let expected_org_id = foreign_org_id.to_string();
        assert_eq!(
            evidence.target_org_id.as_deref(),
            Some(expected_org_id.as_str())
        );
    }

    #[test]
    fn grower_recommendations_list_scoped_rows_and_acknowledge_with_audit() {
        let grower_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let scope = GrowerPortalSessionScope {
            grower_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };
        let mut registry = RecommendationLifecycleRegistry::default();
        registry
            .create_recommendation(recommendation_record(
                "rec-owned",
                "field-a",
                org_id,
                RecommendationPriority::High,
            ))
            .expect("owned recommendation persists");
        registry
            .create_recommendation(recommendation_record(
                "rec-foreign-field",
                "field-b",
                org_id,
                RecommendationPriority::Low,
            ))
            .expect("foreign field recommendation persists");
        let rows = list_grower_recommendations(
            &scope,
            &registry.recommendations_for_org(&org_id.to_string()),
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].recommendation_id, "rec-owned");
        assert_eq!(rows[0].field_id, "field-a");
        assert_eq!(rows[0].status, RecommendationStatus::Open);

        let acknowledged = acknowledge_grower_recommendation(
            &scope,
            &mut registry,
            "rec-owned",
            "2026-06-15T13:06:00Z",
        )
        .expect("owned recommendation acknowledges");

        assert_eq!(acknowledged.status, RecommendationStatus::Reviewed);
        let history = registry.recommendation_history(&org_id.to_string(), "rec-owned");
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].actor_user_id, grower_id.to_string());
        assert_eq!(history[1].at, "2026-06-15T13:06:00Z");
        assert_eq!(history[1].before, Some(RecommendationStatus::Open));
        assert_eq!(history[1].after, RecommendationStatus::Reviewed);
        assert_eq!(
            history[1].change_type,
            RecommendationStatusChangeType::StatusChanged
        );
    }

    #[test]
    fn grower_recommendation_acknowledge_rejects_out_of_scope_recommendation() {
        let grower_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let scope = GrowerPortalSessionScope {
            grower_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };
        let mut registry = RecommendationLifecycleRegistry::default();
        registry
            .create_recommendation(recommendation_record(
                "rec-foreign-field",
                "field-b",
                org_id,
                RecommendationPriority::Low,
            ))
            .expect("foreign field recommendation persists");

        let error = acknowledge_grower_recommendation(
            &scope,
            &mut registry,
            "rec-foreign-field",
            "2026-06-15T13:06:00Z",
        )
        .expect_err("out-of-scope recommendation is denied");

        let GrowerPortalAccessError::Forbidden { evidence } = error else {
            panic!("out-of-scope recommendation returns forbidden evidence");
        };
        assert_eq!(evidence.reason_code, "recommendation_out_of_scope");
        assert_eq!(evidence.field_id, "field-b");
        let record = registry
            .recommendations_for_org(&org_id.to_string())
            .into_iter()
            .find(|recommendation| recommendation.recommendation_id == "rec-foreign-field")
            .expect("recommendation remains present");
        assert_eq!(record.status, RecommendationStatus::Open);
    }

    #[test]
    fn grower_field_map_overlay_accepts_matching_boundary_and_layer_extent() {
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

        let overlay = build_grower_field_map_overlay(
            &scope,
            &farm_fields,
            "field-a",
            &scene_layer("layer-ndvi", "EPSG:4326", Some(test_extent())),
        )
        .expect("matching layer should overlay");

        assert_eq!(overlay.field_id, "field-a");
        assert_eq!(overlay.product_type, "ndvi");
        assert_eq!(overlay.crs, "EPSG:4326");
        assert!(overlay.read_only);
        assert_eq!(overlay.field_extent, overlay.layer_extent);
    }

    #[test]
    fn grower_field_map_overlay_refuses_crs_mismatch() {
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

        let error = build_grower_field_map_overlay(
            &scope,
            &farm_fields,
            "field-a",
            &scene_layer("layer-ndvi", "EPSG:3857", Some(test_extent())),
        )
        .expect_err("mismatched CRS should be refused");

        assert!(matches!(error, GrowerFieldMapError::CrsMismatch { .. }));
    }

    #[test]
    fn grower_field_map_overlay_refuses_extent_mismatch() {
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
        let mut mismatched = test_extent();
        mismatched.max_lon += 0.1;

        let error = build_grower_field_map_overlay(
            &scope,
            &farm_fields,
            "field-a",
            &scene_layer("layer-ndvi", "EPSG:4326", Some(mismatched)),
        )
        .expect_err("mismatched extent should be refused");

        assert!(matches!(
            error,
            GrowerFieldMapError::ExtentMismatch {
                edge: "max_lon",
                ..
            }
        ));
    }

    #[test]
    fn grower_notification_feed_delivers_owned_report_event_with_evidence() {
        let grower_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let scope = GrowerPortalSessionScope {
            grower_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };

        let feed = build_grower_notification_feed(
            &scope,
            &[
                notification_event(
                    GrowerNotificationEventType::ReportPublished,
                    "report:report-a",
                    "field-a",
                    "2026-06-15T13:08:00Z",
                ),
                notification_event(
                    GrowerNotificationEventType::RecommendationCreated,
                    "recommendation:rec-a",
                    "field-a",
                    "2026-06-15T13:09:00Z",
                ),
            ],
        );

        assert_eq!(feed.items.len(), 2);
        assert_eq!(feed.items[0].source_ref, "recommendation:rec-a");
        assert_eq!(feed.items[1].source_ref, "report:report-a");
        assert_eq!(feed.items[1].field_id, "field-a");
        assert!(feed.items[1]
            .evidence_refs
            .iter()
            .any(|evidence| evidence.contains("report:report-a")));
    }

    #[test]
    fn grower_notification_feed_suppresses_out_of_scope_events() {
        let grower_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let scope = GrowerPortalSessionScope {
            grower_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };

        let feed = build_grower_notification_feed(
            &scope,
            &[
                notification_event(
                    GrowerNotificationEventType::ReportPublished,
                    "report:owned",
                    "field-a",
                    "2026-06-15T13:08:00Z",
                ),
                notification_event(
                    GrowerNotificationEventType::ReportPublished,
                    "report:foreign",
                    "field-b",
                    "2026-06-15T13:09:00Z",
                ),
            ],
        );

        assert_eq!(feed.items.len(), 1);
        assert_eq!(feed.items[0].source_ref, "report:owned");
        assert!(feed
            .items
            .iter()
            .all(|item| item.field_id.as_str() == "field-a"));
    }

    #[test]
    fn grower_recommendation_history_renders_full_audit_trail() {
        let grower_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let scope = GrowerPortalSessionScope {
            grower_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };
        let mut registry = RecommendationLifecycleRegistry::default();
        registry
            .create_recommendation(recommendation_record(
                "rec-owned",
                "field-a",
                org_id,
                RecommendationPriority::High,
            ))
            .expect("owned recommendation persists");
        registry
            .transition_recommendation_status(
                &org_id.to_string(),
                "rec-owned",
                &grower_id.to_string(),
                "2026-06-15T13:09:00Z",
                RecommendationStatus::Reviewed,
            )
            .expect("recommendation acknowledges");
        registry
            .transition_recommendation_status(
                &org_id.to_string(),
                "rec-owned",
                "advisor",
                "2026-06-16T13:09:00Z",
                RecommendationStatus::Completed,
            )
            .expect("recommendation completes");

        let history = grower_recommendation_history(&scope, &registry, "rec-owned")
            .expect("history should render");

        assert!(!history.empty_state);
        assert_eq!(history.rows.len(), 3);
        assert_eq!(history.rows[0].before, None);
        assert_eq!(history.rows[0].after, RecommendationStatus::Open);
        assert_eq!(history.rows[1].before, Some(RecommendationStatus::Open));
        assert_eq!(history.rows[1].after, RecommendationStatus::Reviewed);
        assert_eq!(history.rows[1].actor_user_id, grower_id.to_string());
        assert_eq!(history.rows[2].before, Some(RecommendationStatus::Reviewed));
        assert_eq!(history.rows[2].after, RecommendationStatus::Completed);
    }

    #[test]
    fn grower_recommendation_history_reports_empty_state_without_transitions() {
        let empty =
            build_grower_recommendation_history_view("rec-draft", "field-a".to_string(), &mut []);

        assert_eq!(empty.recommendation_id, "rec-draft");
        assert_eq!(empty.field_id, "field-a");
        assert!(empty.empty_state);
        assert!(empty.rows.is_empty());
    }

    #[test]
    fn grower_field_summary_export_scopes_artifact_and_audits() {
        let mut control = ControlPlaneRegistry::default();
        let org_id = control
            .create_organization("Org A".to_string())
            .expect("org creates")
            .org_id;
        let grower = control
            .create_user_with_role(
                org_id,
                "grower@example.com".to_string(),
                MembershipRole::Viewer,
            )
            .expect("grower creates");
        let scope = GrowerPortalSessionScope {
            grower_id: grower.user.user_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };
        let recommendation = grower_recommendation_row(
            &recommendation_record("rec-owned", "field-a", org_id, RecommendationPriority::High),
            "field-a",
        );
        let mut store = GrowerFieldSummaryExportStore::default();

        let artifact = store
            .export_field_summary(
                &mut control,
                &scope,
                grower_field_summary_export_request(
                    "field-a",
                    "2026-06-15T14:00:00Z",
                    vec![grower_field_summary_finding("finding-owned", "field-a")],
                    vec![recommendation],
                ),
            )
            .expect("owned field summary exports");

        assert_eq!(artifact.field_id, "field-a");
        assert_eq!(artifact.grower_id, grower.user.user_id);
        assert_eq!(artifact.org_id, org_id);
        assert_eq!(artifact.findings.len(), 1);
        assert_eq!(artifact.recommendations.len(), 1);
        assert!(artifact
            .evidence_refs
            .contains(&"finding:finding-owned".to_string()));
        assert!(artifact
            .evidence_refs
            .contains(&"recommendation:rec-owned".to_string()));

        let records = control.audit_records_for_org(org_id, None, None);
        assert!(records.iter().any(|record| {
            record.action == ControlPlaneAction::ExportData
                && record.decision == AuthorizationDecision::Allowed
                && record.reason_code == "grower_field_summary_exported"
        }));
    }

    #[test]
    fn grower_field_summary_share_revoke_denies_access() {
        let mut control = ControlPlaneRegistry::default();
        let org_id = control
            .create_organization("Org A".to_string())
            .expect("org creates")
            .org_id;
        let grower = control
            .create_user_with_role(
                org_id,
                "grower@example.com".to_string(),
                MembershipRole::Viewer,
            )
            .expect("grower creates");
        let scope = GrowerPortalSessionScope {
            grower_id: grower.user.user_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };
        let mut store = GrowerFieldSummaryExportStore::default();
        let artifact = store
            .export_field_summary(
                &mut control,
                &scope,
                grower_field_summary_export_request(
                    "field-a",
                    "2026-06-15T14:00:00Z",
                    vec![grower_field_summary_finding("finding-owned", "field-a")],
                    Vec::new(),
                ),
            )
            .expect("owned field summary exports");
        let share = store
            .create_share(
                &mut control,
                &scope,
                &artifact.export_id,
                "2026-06-15T14:05:00Z",
            )
            .expect("share creates");

        let shared = store
            .read_shared_export(&mut control, &scope, &share.share_id)
            .expect("active share reads");
        assert_eq!(shared.export_id, artifact.export_id);

        let revoked = store
            .revoke_share(
                &mut control,
                &scope,
                &share.share_id,
                "2026-06-15T14:10:00Z",
            )
            .expect("share revokes");
        assert_eq!(revoked.revoked_at.as_deref(), Some("2026-06-15T14:10:00Z"));
        let error = store
            .read_shared_export(&mut control, &scope, &share.share_id)
            .expect_err("revoked share is denied");

        assert_eq!(
            error,
            GrowerFieldSummaryShareError::ShareRevoked {
                share_id: share.share_id.clone()
            }
        );
        let records = control.audit_records_for_org(org_id, None, None);
        assert!(records
            .iter()
            .any(|record| record.reason_code == "grower_field_summary_share_created"));
        assert!(records
            .iter()
            .any(|record| record.reason_code == "grower_field_summary_share_revoked"));
        assert!(records.iter().any(|record| {
            record.reason_code == "grower_field_summary_share_revoked"
                && record.decision == AuthorizationDecision::Denied
        }));
    }

    #[test]
    fn grower_field_summary_export_refuses_out_of_scope_data() {
        let mut control = ControlPlaneRegistry::default();
        let org_id = control
            .create_organization("Org A".to_string())
            .expect("org creates")
            .org_id;
        let grower = control
            .create_user_with_role(
                org_id,
                "grower@example.com".to_string(),
                MembershipRole::Viewer,
            )
            .expect("grower creates");
        let scope = GrowerPortalSessionScope {
            grower_id: grower.user.user_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };
        let mut store = GrowerFieldSummaryExportStore::default();

        let error = store
            .export_field_summary(
                &mut control,
                &scope,
                grower_field_summary_export_request(
                    "field-a",
                    "2026-06-15T14:00:00Z",
                    vec![grower_field_summary_finding("finding-foreign", "field-b")],
                    Vec::new(),
                ),
            )
            .expect_err("out-of-scope finding is rejected");

        let GrowerPortalAccessError::Forbidden { evidence } = error else {
            panic!("out-of-scope export returns forbidden evidence");
        };
        assert_eq!(evidence.action, ControlPlaneAction::ExportData);
        assert_eq!(evidence.field_id, "field-b");
        assert_eq!(
            evidence.reason_code,
            "grower_field_summary_export_contains_out_of_scope_data"
        );
        let records = control.audit_records_for_org(org_id, None, None);
        assert!(records.iter().any(|record| {
            record.action == ControlPlaneAction::ExportData
                && record.decision == AuthorizationDecision::Denied
                && record.reason_code == "grower_field_summary_export_contains_out_of_scope_data"
        }));
    }

    #[test]
    fn grower_mobile_app_shell_reuses_scoped_portal_apis() {
        let mut farm_fields = FarmFieldRegistry::default();
        let org_id = Uuid::new_v4();
        insert_test_farm_field_scope(&mut farm_fields, org_id, "farm-a", "field-a");
        let scope = GrowerPortalSessionScope {
            grower_id: Uuid::new_v4(),
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };
        let activity = vec![GrowerFieldActivitySummary {
            field_id: "field-a".to_string(),
            last_scene_date: Some("2026-06-15T12:00:00Z".to_string()),
            open_recommendation_count: 1,
            evidence_refs: vec!["scene:scene-a".to_string()],
        }];
        let analyses = vec![field_analysis_source(
            "field-a",
            "scene-a",
            "2026-06-15T12:00:00Z",
            GrowerFieldAnalysisStatus::Completed,
        )];
        let reports = vec![report_record(
            "report-a",
            "field-a",
            org_id,
            "2026-06-15T13:00:00Z",
        )];
        let events = vec![notification_event(
            GrowerNotificationEventType::ReportPublished,
            "report:report-a",
            "field-a",
            "2026-06-15T13:05:00Z",
        )];

        let mobile = build_grower_mobile_app_shell(
            &scope,
            &farm_fields,
            &activity,
            &analyses,
            &reports,
            GrowerReportInboxQuery::default(),
            &events,
            "2026-06-15T14:00:00Z",
        )
        .expect("mobile shell renders");

        assert_eq!(
            mobile.home,
            build_grower_portal_home(&scope, &farm_fields, &activity)
        );
        assert_eq!(
            mobile.report_inbox,
            list_grower_report_inbox(&scope, &reports, GrowerReportInboxQuery::default())
        );
        assert_eq!(
            mobile.notification_feed,
            build_grower_notification_feed(&scope, &events)
        );
        assert_eq!(mobile.field_overviews.len(), 1);
        assert_eq!(mobile.field_overviews[0].field_id, "field-a");
        assert_eq!(mobile.connectivity, GrowerMobileConnectivity::Online);
        assert_eq!(mobile.freshness_label, None);
    }

    #[test]
    fn grower_mobile_offline_shell_uses_cached_data_with_freshness_label() {
        let mut farm_fields = FarmFieldRegistry::default();
        let org_id = Uuid::new_v4();
        insert_test_farm_field_scope(&mut farm_fields, org_id, "farm-a", "field-a");
        let scope = GrowerPortalSessionScope {
            grower_id: Uuid::new_v4(),
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };
        let cached = build_grower_mobile_app_shell(
            &scope,
            &farm_fields,
            &[],
            &[],
            &[],
            GrowerReportInboxQuery::default(),
            &[],
            "2026-06-15T14:00:00Z",
        )
        .expect("initial mobile load caches");

        let offline = build_grower_mobile_offline_shell(&cached, "2026-06-15T16:30:00Z");

        assert_eq!(offline.connectivity, GrowerMobileConnectivity::Offline);
        assert_eq!(offline.rendered_at, "2026-06-15T16:30:00Z");
        assert_eq!(
            offline.freshness_label.as_deref(),
            Some("offline / as of 2026-06-15T14:00:00Z")
        );
        assert_eq!(offline.home, cached.home);
        assert_eq!(offline.field_overviews, cached.field_overviews);
        assert_eq!(offline.report_inbox, cached.report_inbox);
        assert_eq!(offline.notification_feed, cached.notification_feed);
    }

    #[test]
    fn grower_marketplace_entry_renders_scoped_identity_link() {
        let org_id = Uuid::new_v4();
        let grower_id = Uuid::new_v4();
        let scope = GrowerPortalSessionScope {
            grower_id,
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };

        let resolution = resolve_grower_marketplace_entry(
            &scope,
            &[marketplace_account(
                "market-account-a",
                org_id,
                MarketplaceAccountStatus::Active,
            )],
            "https://agbot.example",
        );

        let entry = resolution.entry.expect("marketplace entry renders");
        assert_eq!(resolution.hidden_reason, None);
        assert_eq!(entry.grower_id, grower_id);
        assert_eq!(entry.org_id, org_id);
        assert_eq!(entry.account_id, "market-account-a");
        assert_eq!(
            entry.href,
            format!(
                "https://agbot.example/orgs/{org_id}/marketplace?grower_id={grower_id}&account_id=market-account-a"
            )
        );
        assert_eq!(
            entry.identity_context.get("grower_id"),
            Some(&grower_id.to_string())
        );
        assert_eq!(
            entry.identity_context.get("org_id"),
            Some(&org_id.to_string())
        );
        assert!(entry
            .evidence_refs
            .contains(&"marketplace-account:market-account-a".to_string()));
    }

    #[test]
    fn grower_marketplace_entry_hides_when_org_disabled() {
        let org_id = Uuid::new_v4();
        let other_org = Uuid::new_v4();
        let scope = GrowerPortalSessionScope {
            grower_id: Uuid::new_v4(),
            org_id,
            role: MembershipRole::Viewer,
            farm_ids: vec!["farm-a".to_string()],
            field_ids: vec!["field-a".to_string()],
        };

        let resolution = resolve_grower_marketplace_entry(
            &scope,
            &[
                marketplace_account(
                    "market-account-pending",
                    org_id,
                    MarketplaceAccountStatus::Pending,
                ),
                marketplace_account(
                    "market-account-other",
                    other_org,
                    MarketplaceAccountStatus::Active,
                ),
            ],
            "https://agbot.example",
        );

        assert_eq!(resolution.entry, None);
        assert_eq!(
            resolution.hidden_reason.as_deref(),
            Some("marketplace_disabled")
        );
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

    fn report_record(
        report_id: &str,
        field_id: &str,
        org_id: Uuid,
        created_at: &str,
    ) -> ReportRecord {
        ReportRecord {
            report_id: report_id.to_string(),
            scene_id: format!("scene-{report_id}"),
            field_id: Some(field_id.to_string()),
            season_id: None,
            org_id: org_id.to_string(),
            generated_by: "advisor".to_string(),
            source_refs: vec![format!("scene:scene-{report_id}")],
            title: format!("Report {report_id}"),
            format: ReportFormat::Html,
            artifact_path: format!("/reports/{report_id}.html"),
            artifact_uri: format!("file:///reports/{report_id}.html"),
            download_url: format!("/api/reports/{report_id}"),
            visibility: ReportVisibility::Org,
            annotation_count: 0,
            recommendation_count: 1,
            created_at: created_at.to_string(),
        }
    }

    fn recommendation_record(
        recommendation_id: &str,
        field_id: &str,
        org_id: Uuid,
        priority: RecommendationPriority,
    ) -> RecommendationRecord {
        RecommendationRecord {
            recommendation_id: recommendation_id.to_string(),
            scene_id: format!("scene-{recommendation_id}"),
            field_id: Some(field_id.to_string()),
            org_id: org_id.to_string(),
            author_user_id: "advisor".to_string(),
            title: format!("Recommendation {recommendation_id}"),
            note: Some("Scout this area".to_string()),
            category: Some("scouting".to_string()),
            action_category: "scouting".to_string(),
            priority,
            status: RecommendationStatus::Open,
            evidence_refs: vec![format!("finding:{recommendation_id}")],
            annotation_ids: Vec::new(),
            created_at: "2026-06-15T12:00:00Z".to_string(),
            updated_at: "2026-06-15T12:00:00Z".to_string(),
        }
    }

    fn scene_layer(layer_id: &str, crs: &str, extent: Option<GeoBounds>) -> SceneLayerRecord {
        SceneLayerRecord {
            layer_id: layer_id.to_string(),
            scene_id: "scene-a".to_string(),
            product_type: "ndvi".to_string(),
            crs: crs.to_string(),
            extent,
            resolution: None,
            uri: format!("file:///layers/{layer_id}.tif"),
        }
    }

    fn notification_event(
        event_type: GrowerNotificationEventType,
        source_ref: &str,
        field_id: &str,
        created_at: &str,
    ) -> GrowerNotificationSourceEvent {
        GrowerNotificationSourceEvent {
            event_type,
            source_ref: source_ref.to_string(),
            field_id: field_id.to_string(),
            created_at: created_at.to_string(),
            title: format!("Notification {source_ref}"),
        }
    }

    fn grower_field_summary_export_request(
        field_id: &str,
        generated_at: &str,
        findings: Vec<GrowerFieldSummaryFindingRow>,
        recommendations: Vec<GrowerRecommendationRow>,
    ) -> GrowerFieldSummaryExportRequest {
        GrowerFieldSummaryExportRequest {
            field_id: field_id.to_string(),
            generated_at: generated_at.to_string(),
            findings,
            recommendations,
        }
    }

    fn grower_field_summary_finding(
        finding_id: &str,
        field_id: &str,
    ) -> GrowerFieldSummaryFindingRow {
        GrowerFieldSummaryFindingRow {
            field_id: field_id.to_string(),
            finding: GrowerFindingSummary {
                finding_id: finding_id.to_string(),
                title: format!("Finding {finding_id}"),
                source_date: "2026-06-15T13:00:00Z".to_string(),
            },
            evidence_refs: vec![format!("scene:scene-{finding_id}")],
        }
    }

    fn marketplace_account(
        account_id: &str,
        org_id: Uuid,
        status: MarketplaceAccountStatus,
    ) -> MarketplaceAccountRecord {
        MarketplaceAccountRecord {
            account_id: account_id.to_string(),
            org_id: org_id.to_string(),
            party_type: crate::schemas::MarketplacePartyType::Grower,
            role_refs: vec!["marketplace:grower".to_string()],
            status,
            created_at: "2026-06-15T13:00:00Z".to_string(),
            updated_at: "2026-06-15T13:00:00Z".to_string(),
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
