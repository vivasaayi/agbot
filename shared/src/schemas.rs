use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// GPS coordinates
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GpsCoords {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoBounds {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    pub longitude: f64,
    pub latitude: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldBoundary {
    pub coordinates: Vec<GeoPoint>,
    #[serde(default)]
    pub crs: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidatedFieldBoundary {
    pub boundary: FieldBoundary,
    pub extent: GeoBounds,
    pub area_ha: f64,
}

pub const DEFAULT_RECORD_OWNER: &str = "unassigned";

fn default_record_owner() -> String {
    DEFAULT_RECORD_OWNER.to_string()
}

const DEFAULT_FARM_FIELD_PAGE_SIZE: usize = 50;
const MAX_FARM_FIELD_PAGE_SIZE: usize = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FarmFieldEntityStatus {
    Active,
    Archived,
}

impl FarmFieldEntityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            FarmFieldEntityStatus::Active => "active",
            FarmFieldEntityStatus::Archived => "archived",
        }
    }
}

impl Default for FarmFieldEntityStatus {
    fn default() -> Self {
        FarmFieldEntityStatus::Active
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FarmFieldListQuery {
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub page_size: Option<usize>,
    #[serde(default)]
    pub status: Option<FarmFieldEntityStatus>,
}

impl FarmFieldListQuery {
    pub fn normalized_page(&self) -> usize {
        self.page.unwrap_or(1).max(1)
    }

    pub fn normalized_page_size(&self) -> usize {
        match self.page_size {
            Some(0) | None => DEFAULT_FARM_FIELD_PAGE_SIZE,
            Some(size) => size.min(MAX_FARM_FIELD_PAGE_SIZE),
        }
    }

    fn status_filter(&self) -> FarmFieldEntityStatus {
        self.status.unwrap_or_default()
    }
}

impl Default for FarmFieldListQuery {
    fn default() -> Self {
        Self {
            page: Some(1),
            page_size: Some(DEFAULT_FARM_FIELD_PAGE_SIZE),
            status: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FarmFieldListPage<T> {
    pub items: Vec<T>,
    pub total_count: usize,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FarmRecord {
    pub farm_id: String,
    #[serde(default = "default_record_owner")]
    pub org_id: String,
    #[serde(default = "default_record_owner")]
    pub owner: String,
    pub name: String,
    pub notes: Option<String>,
    #[serde(default)]
    pub status: FarmFieldEntityStatus,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldRecord {
    #[serde(default)]
    pub farm_id: Option<String>,
    pub field_id: String,
    #[serde(default = "default_record_owner")]
    pub org_id: String,
    #[serde(default = "default_record_owner")]
    pub owner: String,
    pub name: String,
    #[serde(default)]
    pub area_ha: Option<f64>,
    pub crop: Option<String>,
    pub season: Option<String>,
    pub notes: Option<String>,
    pub boundary: FieldBoundary,
    pub extent: GeoBounds,
    #[serde(default)]
    pub status: FarmFieldEntityStatus,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldBoundaryRecord {
    pub field_id: String,
    pub farm_id: Option<String>,
    pub org_id: String,
    pub owner: String,
    pub name: String,
    pub boundary: FieldBoundary,
    pub extent: GeoBounds,
    pub area_ha: Option<f64>,
    pub status: FarmFieldEntityStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeasonRecord {
    pub season_id: String,
    pub field_id: String,
    pub org_id: String,
    pub start: String,
    pub end: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CropPlanRecord {
    pub crop_plan_id: String,
    pub season_id: String,
    #[serde(default)]
    pub org_id: String,
    pub crop: String,
    pub planting_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldSeasonHistory {
    pub season: SeasonRecord,
    pub crop_plans: Vec<CropPlanRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneRecord {
    pub scene_id: String,
    pub field_id: String,
    pub season_id: String,
    pub org_id: String,
    pub captured_at: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneLayerRecord {
    pub layer_id: String,
    pub scene_id: String,
    pub product_type: String,
    #[serde(default)]
    pub crs: String,
    #[serde(default)]
    pub extent: Option<GeoBounds>,
    #[serde(default)]
    pub resolution: Option<RasterResolution>,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldSceneCatalogEntry {
    pub scene: SceneRecord,
    pub layers: Vec<SceneLayerRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveSeasonResolution {
    pub field_id: String,
    pub requested_date: String,
    pub active_season: Option<SeasonRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneFieldCoverageStatus {
    Full,
    Partial,
    NoCoverage,
    NoLayers,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneFieldCoverage {
    pub scene_id: String,
    pub field_id: String,
    pub coverage_fraction: f64,
    pub status: SceneFieldCoverageStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetNodeKind {
    Drone,
    Edge,
}

impl FleetNodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FleetNodeKind::Drone => "drone",
            FleetNodeKind::Edge => "edge",
        }
    }
}

impl std::str::FromStr for FleetNodeKind {
    type Err = FleetNodeEnrollmentError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "drone" => Ok(FleetNodeKind::Drone),
            "edge" => Ok(FleetNodeKind::Edge),
            _ => Err(FleetNodeEnrollmentError::UnsupportedKind {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetNodeRuntimeMode {
    Simulation,
    Flight,
}

impl FleetNodeRuntimeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            FleetNodeRuntimeMode::Simulation => "simulation",
            FleetNodeRuntimeMode::Flight => "flight",
        }
    }
}

impl std::str::FromStr for FleetNodeRuntimeMode {
    type Err = FleetNodeEnrollmentError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "simulation" => Ok(FleetNodeRuntimeMode::Simulation),
            "flight" => Ok(FleetNodeRuntimeMode::Flight),
            _ => Err(FleetNodeEnrollmentError::UnsupportedRuntimeMode {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetNodeStatus {
    Enrolled,
    Maintenance,
}

impl FleetNodeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            FleetNodeStatus::Enrolled => "enrolled",
            FleetNodeStatus::Maintenance => "maintenance",
        }
    }
}

impl std::str::FromStr for FleetNodeStatus {
    type Err = FleetNodeEnrollmentError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "enrolled" => Ok(FleetNodeStatus::Enrolled),
            "maintenance" => Ok(FleetNodeStatus::Maintenance),
            _ => Err(FleetNodeEnrollmentError::UnsupportedStatus {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FleetNodeEnrollmentRequest {
    #[serde(default)]
    pub hardware_id: String,
    pub kind: FleetNodeKind,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub owner_org_id: String,
    pub runtime_mode: FleetNodeRuntimeMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetNodeRecord {
    pub node_id: String,
    pub hardware_id: String,
    pub kind: FleetNodeKind,
    pub capabilities: Vec<String>,
    pub owner_org_id: String,
    pub runtime_mode: FleetNodeRuntimeMode,
    pub enrolled_at: String,
    pub status: FleetNodeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetNodeIdentityBinding {
    pub record: FleetNodeRecord,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FleetNodeEnrollmentError {
    #[error("fleet node hardware_id cannot be empty")]
    EmptyHardwareId,
    #[error("fleet node node_id cannot be empty")]
    EmptyNodeId,
    #[error("fleet node owner_org_id cannot be empty")]
    EmptyOwnerOrgId,
    #[error("fleet node capabilities cannot be empty")]
    EmptyCapabilities,
    #[error("fleet node enrolled_at cannot be empty")]
    EmptyEnrolledAt,
    #[error("unsupported fleet node kind {value}")]
    UnsupportedKind { value: String },
    #[error("unsupported fleet node runtime_mode {value}")]
    UnsupportedRuntimeMode { value: String },
    #[error("unsupported fleet node status {value}")]
    UnsupportedStatus { value: String },
    #[error("existing fleet node hardware_id {existing} does not match enrollment {requested}")]
    HardwareIdMismatch { existing: String, requested: String },
}

pub fn bind_fleet_node_identity(
    request: FleetNodeEnrollmentRequest,
    existing: Option<FleetNodeRecord>,
    issued_node_id: String,
    enrolled_at: String,
) -> Result<FleetNodeIdentityBinding, FleetNodeEnrollmentError> {
    let requested_hardware_id = normalize_fleet_node_arg(
        request.hardware_id,
        FleetNodeEnrollmentError::EmptyHardwareId,
    )?;

    if let Some(existing) = existing {
        if existing.hardware_id != requested_hardware_id {
            return Err(FleetNodeEnrollmentError::HardwareIdMismatch {
                existing: existing.hardware_id,
                requested: requested_hardware_id,
            });
        }
        return Ok(FleetNodeIdentityBinding {
            record: existing,
            created: false,
        });
    }

    let node_id = normalize_fleet_node_arg(issued_node_id, FleetNodeEnrollmentError::EmptyNodeId)?;
    let owner_org_id = normalize_fleet_node_arg(
        request.owner_org_id,
        FleetNodeEnrollmentError::EmptyOwnerOrgId,
    )?;
    let enrolled_at =
        normalize_fleet_node_arg(enrolled_at, FleetNodeEnrollmentError::EmptyEnrolledAt)?;
    let capabilities = normalize_fleet_node_capabilities(request.capabilities)?;

    Ok(FleetNodeIdentityBinding {
        record: FleetNodeRecord {
            node_id,
            hardware_id: requested_hardware_id,
            kind: request.kind,
            capabilities,
            owner_org_id,
            runtime_mode: request.runtime_mode,
            enrolled_at,
            status: FleetNodeStatus::Enrolled,
        },
        created: true,
    })
}

fn normalize_fleet_node_arg(
    value: String,
    error: FleetNodeEnrollmentError,
) -> Result<String, FleetNodeEnrollmentError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_fleet_node_capabilities(
    capabilities: Vec<String>,
) -> Result<Vec<String>, FleetNodeEnrollmentError> {
    let capabilities = capabilities
        .into_iter()
        .filter_map(|capability| {
            let capability = capability.trim();
            (!capability.is_empty()).then(|| capability.to_ascii_lowercase())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    if capabilities.is_empty() {
        Err(FleetNodeEnrollmentError::EmptyCapabilities)
    } else {
        Ok(capabilities)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TractorLifecycleStatus {
    Registered,
    Available,
    InUse,
    OutOfService,
}

impl TractorLifecycleStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TractorLifecycleStatus::Registered => "registered",
            TractorLifecycleStatus::Available => "available",
            TractorLifecycleStatus::InUse => "in_use",
            TractorLifecycleStatus::OutOfService => "out_of_service",
        }
    }
}

impl Default for TractorLifecycleStatus {
    fn default() -> Self {
        TractorLifecycleStatus::Registered
    }
}

impl std::str::FromStr for TractorLifecycleStatus {
    type Err = TractorRegistryError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "registered" => Ok(TractorLifecycleStatus::Registered),
            "available" => Ok(TractorLifecycleStatus::Available),
            "in_use" => Ok(TractorLifecycleStatus::InUse),
            "out_of_service" => Ok(TractorLifecycleStatus::OutOfService),
            _ => Err(TractorRegistryError::UnsupportedStatus {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorImplementRef {
    pub implement_id: String,
    pub implement_type: String,
    #[serde(default)]
    pub working_width_m: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorRegistrationRequest {
    #[serde(default)]
    pub tractor_id: Option<String>,
    pub org_id: String,
    pub field_id: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub implement_ref: TractorImplementRef,
    #[serde(default)]
    pub status: Option<TractorLifecycleStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorRecord {
    pub tractor_id: String,
    pub org_id: String,
    pub field_id: String,
    pub capabilities: Vec<String>,
    pub implement_ref: TractorImplementRef,
    pub status: TractorLifecycleStatus,
    pub registered_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TractorMotionCommandRequest {
    #[serde(default)]
    pub command_id: Option<String>,
    pub tractor_id: String,
    pub command_type: String,
    #[serde(default)]
    pub requested_by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TractorCommandAuditDecision {
    Allowed,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TractorCommandRejectionReason {
    UnknownTractor,
    TractorOutOfService,
}

impl TractorCommandRejectionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            TractorCommandRejectionReason::UnknownTractor => "tractor_not_registered",
            TractorCommandRejectionReason::TractorOutOfService => "tractor_out_of_service",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TractorCommandAuditRecord {
    pub audit_id: String,
    #[serde(default)]
    pub command_id: Option<String>,
    pub tractor_id: String,
    #[serde(default)]
    pub org_id: Option<String>,
    #[serde(default)]
    pub field_id: Option<String>,
    pub command_type: String,
    #[serde(default)]
    pub requested_by: Option<String>,
    pub decision: TractorCommandAuditDecision,
    pub reason_code: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TractorCommandRejection {
    pub tractor_id: String,
    pub reason: TractorCommandRejectionReason,
    #[serde(default)]
    pub status: Option<TractorLifecycleStatus>,
    pub audit: TractorCommandAuditRecord,
}

impl TractorCommandRejection {
    pub fn status_code(&self) -> u16 {
        match self.reason {
            TractorCommandRejectionReason::UnknownTractor => 404,
            TractorCommandRejectionReason::TractorOutOfService => 409,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TractorRegistryError {
    #[error("tractor_id cannot be empty")]
    EmptyTractorId,
    #[error("tractor org_id cannot be empty")]
    EmptyOrgId,
    #[error("tractor field_id cannot be empty")]
    EmptyFieldId,
    #[error("tractor capabilities cannot be empty")]
    EmptyCapabilities,
    #[error("tractor implement_id cannot be empty")]
    EmptyImplementId,
    #[error("tractor implement_type cannot be empty")]
    EmptyImplementType,
    #[error("tractor registered_at cannot be empty")]
    EmptyRegisteredAt,
    #[error("tractor command_type cannot be empty")]
    EmptyCommandType,
    #[error("unsupported tractor status {value}")]
    UnsupportedStatus { value: String },
    #[error("tractor already registered: {tractor_id}")]
    DuplicateTractor { tractor_id: String },
    #[error("tractor not found: {tractor_id}")]
    TractorNotFound { tractor_id: String },
    #[error("field not found: {field_id}")]
    FieldNotFound { field_id: String },
    #[error("field {field_id} belongs to {actual_org_id}, not {expected_org_id}")]
    FieldTenantMismatch {
        field_id: String,
        expected_org_id: String,
        actual_org_id: String,
    },
    #[error("invalid tractor lifecycle transition for {tractor_id}: {from:?} -> {to:?}")]
    InvalidLifecycleTransition {
        tractor_id: String,
        from: TractorLifecycleStatus,
        to: TractorLifecycleStatus,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TractorRegistry {
    tractors: HashMap<String, TractorRecord>,
    #[serde(default)]
    command_audits: Vec<TractorCommandAuditRecord>,
}

impl TractorRegistry {
    pub fn register_tractor(
        &mut self,
        request: TractorRegistrationRequest,
        farm_fields: &FarmFieldRegistry,
        registered_at: String,
    ) -> Result<TractorRecord, TractorRegistryError> {
        let tractor_id = normalize_tractor_text(request.tractor_id.clone().unwrap_or_default())
            .ok_or(TractorRegistryError::EmptyTractorId)?;
        if self.tractors.contains_key(&tractor_id) {
            return Err(TractorRegistryError::DuplicateTractor { tractor_id });
        }

        let field_id = normalize_tractor_text(request.field_id.clone())
            .ok_or(TractorRegistryError::EmptyFieldId)?;
        let field = farm_fields.field_by_id(&field_id).ok_or_else(|| {
            TractorRegistryError::FieldNotFound {
                field_id: field_id.clone(),
            }
        })?;
        let record = build_tractor_record(
            TractorRegistrationRequest {
                tractor_id: Some(tractor_id.clone()),
                field_id,
                ..request
            },
            &field,
            registered_at,
        )?;
        self.tractors.insert(tractor_id, record.clone());
        Ok(record)
    }

    pub fn list_tractors_for_org(
        &self,
        org_id: &str,
        field_id: Option<&str>,
        status: Option<TractorLifecycleStatus>,
    ) -> Vec<TractorRecord> {
        let org_id = org_id.trim();
        let field_id = field_id.and_then(|field_id| {
            let field_id = field_id.trim();
            (!field_id.is_empty()).then_some(field_id)
        });
        let mut records = self
            .tractors
            .values()
            .filter(|record| record.org_id == org_id)
            .filter(|record| field_id.is_none_or(|field_id| record.field_id == field_id))
            .filter(|record| status.is_none_or(|status| record.status == status))
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.tractor_id.cmp(&right.tractor_id));
        records
    }

    pub fn tractor(&self, tractor_id: &str) -> Option<TractorRecord> {
        self.tractors.get(tractor_id.trim()).cloned()
    }

    pub fn transition_tractor_status(
        &mut self,
        tractor_id: &str,
        to: TractorLifecycleStatus,
        updated_at: String,
    ) -> Result<TractorRecord, TractorRegistryError> {
        let tractor_id = normalize_tractor_text(tractor_id.to_string())
            .ok_or(TractorRegistryError::EmptyTractorId)?;
        let record = self.tractors.get_mut(&tractor_id).ok_or_else(|| {
            TractorRegistryError::TractorNotFound {
                tractor_id: tractor_id.clone(),
            }
        })?;
        let from = record.status;
        if !valid_tractor_lifecycle_transition(from, to) {
            return Err(TractorRegistryError::InvalidLifecycleTransition {
                tractor_id,
                from,
                to,
            });
        }
        record.status = to;
        record.updated_at =
            normalize_tractor_text(updated_at).unwrap_or_else(|| record.updated_at.clone());
        Ok(record.clone())
    }

    pub fn validate_motion_command(
        &mut self,
        request: TractorMotionCommandRequest,
        at: String,
    ) -> Result<TractorRecord, TractorCommandRejection> {
        let tractor_id = request.tractor_id.trim().to_string();
        let command_type = request.command_type.trim().to_string();
        let command_type = if command_type.is_empty() {
            "unknown".to_string()
        } else {
            command_type
        };
        let requested_by = request.requested_by.and_then(normalize_tractor_text);
        let command_id = request.command_id.and_then(normalize_tractor_text);
        let at = normalize_tractor_text(at).unwrap_or_default();

        let Some(record) = self.tractors.get(&tractor_id).cloned() else {
            let audit = self.append_tractor_command_audit(
                command_id,
                tractor_id.clone(),
                None,
                None,
                command_type,
                requested_by,
                TractorCommandRejectionReason::UnknownTractor,
                at,
            );
            return Err(TractorCommandRejection {
                tractor_id,
                reason: TractorCommandRejectionReason::UnknownTractor,
                status: None,
                audit,
            });
        };

        if record.status == TractorLifecycleStatus::OutOfService {
            let audit = self.append_tractor_command_audit(
                command_id,
                tractor_id.clone(),
                Some(record.org_id.clone()),
                Some(record.field_id.clone()),
                command_type,
                requested_by,
                TractorCommandRejectionReason::TractorOutOfService,
                at,
            );
            return Err(TractorCommandRejection {
                tractor_id,
                reason: TractorCommandRejectionReason::TractorOutOfService,
                status: Some(record.status),
                audit,
            });
        }

        Ok(record)
    }

    pub fn command_audits(&self) -> &[TractorCommandAuditRecord] {
        &self.command_audits
    }

    fn append_tractor_command_audit(
        &mut self,
        command_id: Option<String>,
        tractor_id: String,
        org_id: Option<String>,
        field_id: Option<String>,
        command_type: String,
        requested_by: Option<String>,
        reason: TractorCommandRejectionReason,
        at: String,
    ) -> TractorCommandAuditRecord {
        let audit = TractorCommandAuditRecord {
            audit_id: format!("tractor-command-audit-{}", self.command_audits.len() + 1),
            command_id,
            tractor_id,
            org_id,
            field_id,
            command_type,
            requested_by,
            decision: TractorCommandAuditDecision::Rejected,
            reason_code: reason.as_str().to_string(),
            at,
        };
        self.command_audits.push(audit.clone());
        audit
    }
}

pub fn build_tractor_record(
    request: TractorRegistrationRequest,
    field: &FieldRecord,
    registered_at: String,
) -> Result<TractorRecord, TractorRegistryError> {
    let tractor_id = normalize_tractor_text(request.tractor_id.unwrap_or_default())
        .ok_or(TractorRegistryError::EmptyTractorId)?;
    let org_id = normalize_tractor_text(request.org_id).ok_or(TractorRegistryError::EmptyOrgId)?;
    let field_id =
        normalize_tractor_text(request.field_id).ok_or(TractorRegistryError::EmptyFieldId)?;
    if field.field_id != field_id {
        return Err(TractorRegistryError::FieldNotFound { field_id });
    }
    if field.org_id != org_id {
        return Err(TractorRegistryError::FieldTenantMismatch {
            field_id,
            expected_org_id: org_id,
            actual_org_id: field.org_id.clone(),
        });
    }

    let capabilities = normalize_tractor_capabilities(request.capabilities)?;
    let implement_ref = normalize_tractor_implement_ref(request.implement_ref)?;
    let registered_at =
        normalize_tractor_text(registered_at).ok_or(TractorRegistryError::EmptyRegisteredAt)?;
    Ok(TractorRecord {
        tractor_id,
        org_id,
        field_id,
        capabilities,
        implement_ref,
        status: request.status.unwrap_or_default(),
        registered_at: registered_at.clone(),
        updated_at: registered_at,
    })
}

fn valid_tractor_lifecycle_transition(
    from: TractorLifecycleStatus,
    to: TractorLifecycleStatus,
) -> bool {
    from == to
        || matches!(
            (from, to),
            (
                TractorLifecycleStatus::Registered,
                TractorLifecycleStatus::Available
            ) | (
                TractorLifecycleStatus::Available,
                TractorLifecycleStatus::InUse
            ) | (
                TractorLifecycleStatus::InUse,
                TractorLifecycleStatus::OutOfService
            )
        )
}

fn normalize_tractor_implement_ref(
    implement_ref: TractorImplementRef,
) -> Result<TractorImplementRef, TractorRegistryError> {
    Ok(TractorImplementRef {
        implement_id: normalize_tractor_text(implement_ref.implement_id)
            .ok_or(TractorRegistryError::EmptyImplementId)?,
        implement_type: normalize_tractor_text(implement_ref.implement_type)
            .map(|value| value.to_ascii_lowercase())
            .ok_or(TractorRegistryError::EmptyImplementType)?,
        working_width_m: implement_ref
            .working_width_m
            .filter(|working_width_m| working_width_m.is_finite() && *working_width_m > 0.0),
    })
}

fn normalize_tractor_capabilities(
    capabilities: Vec<String>,
) -> Result<Vec<String>, TractorRegistryError> {
    let capabilities = capabilities
        .into_iter()
        .filter_map(normalize_tractor_text)
        .map(|capability| capability.to_ascii_lowercase())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    if capabilities.is_empty() {
        Err(TractorRegistryError::EmptyCapabilities)
    } else {
        Ok(capabilities)
    }
}

fn normalize_tractor_text(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherForecastValue {
    pub value: f64,
    pub unit: String,
    pub source: String,
    pub fetched_at: String,
    pub valid_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherForecastVariables {
    pub temperature_celsius: WeatherForecastValue,
    pub wind_speed_mps: WeatherForecastValue,
    pub precipitation_mm: WeatherForecastValue,
    pub humidity_percent: WeatherForecastValue,
    pub radiation_w_m2: WeatherForecastValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherForecastRecord {
    pub forecast_id: String,
    pub field_ref: String,
    pub valid_time: String,
    pub vars: WeatherForecastVariables,
    pub source: String,
    pub fetched_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherProviderForecastPoint {
    pub valid_time: String,
    pub temperature_celsius: f64,
    pub wind_speed_mps: f64,
    pub precipitation_mm: f64,
    pub humidity_percent: f64,
    pub radiation_w_m2: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherProviderForecastResponse {
    pub source: String,
    pub fetched_at: String,
    pub points: Vec<WeatherProviderForecastPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeatherFetchFailureRecord {
    pub failure_id: String,
    pub field_ref: String,
    pub source: String,
    pub fetched_at: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WeatherIngestError {
    #[error("weather field_ref cannot be empty")]
    EmptyFieldRef,
    #[error("weather source cannot be empty")]
    EmptySource,
    #[error("weather fetched_at cannot be empty")]
    EmptyFetchedAt,
    #[error("weather valid_time cannot be empty")]
    EmptyValidTime,
    #[error("weather forecast contains no points")]
    EmptyForecastPoints,
    #[error("weather value {variable} is invalid: {value}")]
    InvalidValue { variable: String, value: String },
    #[error("weather failure_id cannot be empty")]
    EmptyFailureId,
    #[error("weather failure reason cannot be empty")]
    EmptyFailureReason,
}

pub fn normalize_weather_provider_forecast(
    field_ref: String,
    response: WeatherProviderForecastResponse,
) -> Result<Vec<WeatherForecastRecord>, WeatherIngestError> {
    let field_ref = normalize_weather_text(field_ref).ok_or(WeatherIngestError::EmptyFieldRef)?;
    let source = normalize_weather_text(response.source).ok_or(WeatherIngestError::EmptySource)?;
    let fetched_at =
        normalize_weather_text(response.fetched_at).ok_or(WeatherIngestError::EmptyFetchedAt)?;
    if response.points.is_empty() {
        return Err(WeatherIngestError::EmptyForecastPoints);
    }

    response
        .points
        .into_iter()
        .map(|point| {
            let valid_time = normalize_weather_text(point.valid_time)
                .ok_or(WeatherIngestError::EmptyValidTime)?;
            let vars = WeatherForecastVariables {
                temperature_celsius: weather_value(
                    "temperature_celsius",
                    point.temperature_celsius,
                    "deg_c",
                    &source,
                    &fetched_at,
                    &valid_time,
                    f64::is_finite,
                )?,
                wind_speed_mps: weather_value(
                    "wind_speed_mps",
                    point.wind_speed_mps,
                    "m/s",
                    &source,
                    &fetched_at,
                    &valid_time,
                    |value| value.is_finite() && value >= 0.0,
                )?,
                precipitation_mm: weather_value(
                    "precipitation_mm",
                    point.precipitation_mm,
                    "mm",
                    &source,
                    &fetched_at,
                    &valid_time,
                    |value| value.is_finite() && value >= 0.0,
                )?,
                humidity_percent: weather_value(
                    "humidity_percent",
                    point.humidity_percent,
                    "percent",
                    &source,
                    &fetched_at,
                    &valid_time,
                    |value| value.is_finite() && (0.0..=100.0).contains(&value),
                )?,
                radiation_w_m2: weather_value(
                    "radiation_w_m2",
                    point.radiation_w_m2,
                    "W/m^2",
                    &source,
                    &fetched_at,
                    &valid_time,
                    |value| value.is_finite() && value >= 0.0,
                )?,
            };

            Ok(WeatherForecastRecord {
                forecast_id: stable_weather_forecast_id(&field_ref, &source, &valid_time),
                field_ref: field_ref.clone(),
                valid_time,
                vars,
                source: source.clone(),
                fetched_at: fetched_at.clone(),
            })
        })
        .collect()
}

pub fn weather_fetch_failure_record(
    failure_id: String,
    field_ref: String,
    source: String,
    fetched_at: String,
    reason: String,
) -> Result<WeatherFetchFailureRecord, WeatherIngestError> {
    Ok(WeatherFetchFailureRecord {
        failure_id: normalize_weather_text(failure_id).ok_or(WeatherIngestError::EmptyFailureId)?,
        field_ref: normalize_weather_text(field_ref).ok_or(WeatherIngestError::EmptyFieldRef)?,
        source: normalize_weather_text(source).ok_or(WeatherIngestError::EmptySource)?,
        fetched_at: normalize_weather_text(fetched_at).ok_or(WeatherIngestError::EmptyFetchedAt)?,
        reason: normalize_weather_text(reason).ok_or(WeatherIngestError::EmptyFailureReason)?,
    })
}

fn weather_value(
    variable: &str,
    value: f64,
    unit: &str,
    source: &str,
    fetched_at: &str,
    valid_time: &str,
    validator: impl Fn(f64) -> bool,
) -> Result<WeatherForecastValue, WeatherIngestError> {
    if !validator(value) {
        return Err(WeatherIngestError::InvalidValue {
            variable: variable.to_string(),
            value: value.to_string(),
        });
    }
    Ok(WeatherForecastValue {
        value,
        unit: unit.to_string(),
        source: source.to_string(),
        fetched_at: fetched_at.to_string(),
        valid_time: valid_time.to_string(),
    })
}

fn stable_weather_forecast_id(field_ref: &str, source: &str, valid_time: &str) -> String {
    format!(
        "weather:{}:{}:{}",
        sanitize_weather_id_part(field_ref),
        sanitize_weather_id_part(source),
        sanitize_weather_id_part(valid_time)
    )
}

fn sanitize_weather_id_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn normalize_weather_text(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoilMoistureQaFlag {
    Valid,
    Suspect,
    Invalid,
}

impl Default for SoilMoistureQaFlag {
    fn default() -> Self {
        SoilMoistureQaFlag::Valid
    }
}

impl SoilMoistureQaFlag {
    pub fn as_str(self) -> &'static str {
        match self {
            SoilMoistureQaFlag::Valid => "valid",
            SoilMoistureQaFlag::Suspect => "suspect",
            SoilMoistureQaFlag::Invalid => "invalid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilMoistureReadingRequest {
    #[serde(default)]
    pub reading_id: Option<String>,
    #[serde(default)]
    pub field_id: Option<String>,
    #[serde(default)]
    pub zone_ref: Option<String>,
    pub value: f64,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub captured_at: String,
    #[serde(default)]
    pub qa_flag: SoilMoistureQaFlag,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilMoistureReadingRecord {
    pub reading_id: String,
    pub field_id: String,
    pub zone_ref: String,
    pub value: f64,
    pub source: String,
    pub captured_at: String,
    pub qa_flag: SoilMoistureQaFlag,
    pub ingested_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoilMoistureRejectionReason {
    MissingFieldLinkage,
    MissingZoneLinkage,
    FieldNotFound,
    InvalidValue,
    EmptySource,
    EmptyCapturedAt,
}

impl SoilMoistureRejectionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            SoilMoistureRejectionReason::MissingFieldLinkage => "missing_field_linkage",
            SoilMoistureRejectionReason::MissingZoneLinkage => "missing_zone_linkage",
            SoilMoistureRejectionReason::FieldNotFound => "field_not_found",
            SoilMoistureRejectionReason::InvalidValue => "invalid_value",
            SoilMoistureRejectionReason::EmptySource => "empty_source",
            SoilMoistureRejectionReason::EmptyCapturedAt => "empty_captured_at",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilMoistureRejectionRecord {
    pub rejection_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at: Option<String>,
    pub reason: SoilMoistureRejectionReason,
    pub rejected_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SoilMoistureReadingError {
    #[error("soil moisture reading_id cannot be empty")]
    EmptyReadingId,
    #[error("soil moisture reading requires field linkage")]
    MissingFieldLinkage,
    #[error("soil moisture reading requires zone linkage")]
    MissingZoneLinkage,
    #[error("soil moisture field not found: {field_id}")]
    FieldNotFound { field_id: String },
    #[error("soil moisture value is invalid: {value}")]
    InvalidValue { value: String },
    #[error("soil moisture source cannot be empty")]
    EmptySource,
    #[error("soil moisture captured_at cannot be empty")]
    EmptyCapturedAt,
    #[error("soil moisture ingested_at cannot be empty")]
    EmptyIngestedAt,
    #[error("soil moisture rejection_id cannot be empty")]
    EmptyRejectionId,
    #[error("soil moisture rejected_at cannot be empty")]
    EmptyRejectedAt,
    #[error("unsupported soil moisture QA flag {value}")]
    UnsupportedQaFlag { value: String },
    #[error("unsupported soil moisture rejection reason {value}")]
    UnsupportedRejectionReason { value: String },
}

pub fn build_soil_moisture_reading(
    request: SoilMoistureReadingRequest,
    field: &FieldRecord,
    generated_reading_id: String,
    ingested_at: String,
) -> Result<SoilMoistureReadingRecord, SoilMoistureReadingError> {
    let reading_id = normalize_soil_moisture_optional_text(request.reading_id)
        .or_else(|| normalize_soil_moisture_text(generated_reading_id))
        .ok_or(SoilMoistureReadingError::EmptyReadingId)?;
    let field_id = normalize_soil_moisture_optional_text(request.field_id)
        .ok_or(SoilMoistureReadingError::MissingFieldLinkage)?;
    if field.field_id != field_id {
        return Err(SoilMoistureReadingError::FieldNotFound { field_id });
    }
    let zone_ref = normalize_soil_moisture_optional_text(request.zone_ref)
        .ok_or(SoilMoistureReadingError::MissingZoneLinkage)?;
    if !(request.value.is_finite() && (0.0..=100.0).contains(&request.value)) {
        return Err(SoilMoistureReadingError::InvalidValue {
            value: request.value.to_string(),
        });
    }
    let source = normalize_soil_moisture_text(request.source)
        .ok_or(SoilMoistureReadingError::EmptySource)?;
    let captured_at = normalize_soil_moisture_text(request.captured_at)
        .ok_or(SoilMoistureReadingError::EmptyCapturedAt)?;
    let ingested_at = normalize_soil_moisture_text(ingested_at)
        .ok_or(SoilMoistureReadingError::EmptyIngestedAt)?;

    Ok(SoilMoistureReadingRecord {
        reading_id,
        field_id,
        zone_ref,
        value: request.value,
        source,
        captured_at,
        qa_flag: request.qa_flag,
        ingested_at,
    })
}

pub fn soil_moisture_rejection_record(
    rejection_id: String,
    request: &SoilMoistureReadingRequest,
    reason: SoilMoistureRejectionReason,
    rejected_at: String,
) -> Result<SoilMoistureRejectionRecord, SoilMoistureReadingError> {
    Ok(SoilMoistureRejectionRecord {
        rejection_id: normalize_soil_moisture_text(rejection_id)
            .ok_or(SoilMoistureReadingError::EmptyRejectionId)?,
        reading_id: normalize_soil_moisture_optional_text(request.reading_id.clone()),
        field_id: normalize_soil_moisture_optional_text(request.field_id.clone()),
        zone_ref: normalize_soil_moisture_optional_text(request.zone_ref.clone()),
        source: normalize_soil_moisture_text(request.source.clone()),
        captured_at: normalize_soil_moisture_text(request.captured_at.clone()),
        reason,
        rejected_at: normalize_soil_moisture_text(rejected_at)
            .ok_or(SoilMoistureReadingError::EmptyRejectedAt)?,
    })
}

pub fn parse_soil_moisture_qa_flag(
    value: &str,
) -> Result<SoilMoistureQaFlag, SoilMoistureReadingError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "valid" => Ok(SoilMoistureQaFlag::Valid),
        "suspect" => Ok(SoilMoistureQaFlag::Suspect),
        "invalid" => Ok(SoilMoistureQaFlag::Invalid),
        _ => Err(SoilMoistureReadingError::UnsupportedQaFlag {
            value: value.to_string(),
        }),
    }
}

pub fn parse_soil_moisture_rejection_reason(
    value: &str,
) -> Result<SoilMoistureRejectionReason, SoilMoistureReadingError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "missing_field_linkage" => Ok(SoilMoistureRejectionReason::MissingFieldLinkage),
        "missing_zone_linkage" => Ok(SoilMoistureRejectionReason::MissingZoneLinkage),
        "field_not_found" => Ok(SoilMoistureRejectionReason::FieldNotFound),
        "invalid_value" => Ok(SoilMoistureRejectionReason::InvalidValue),
        "empty_source" => Ok(SoilMoistureRejectionReason::EmptySource),
        "empty_captured_at" => Ok(SoilMoistureRejectionReason::EmptyCapturedAt),
        _ => Err(SoilMoistureReadingError::UnsupportedRejectionReason {
            value: value.to_string(),
        }),
    }
}

pub fn soil_moisture_rejection_reason_for_error(
    error: &SoilMoistureReadingError,
) -> SoilMoistureRejectionReason {
    match error {
        SoilMoistureReadingError::MissingFieldLinkage => {
            SoilMoistureRejectionReason::MissingFieldLinkage
        }
        SoilMoistureReadingError::MissingZoneLinkage => {
            SoilMoistureRejectionReason::MissingZoneLinkage
        }
        SoilMoistureReadingError::FieldNotFound { .. } => {
            SoilMoistureRejectionReason::FieldNotFound
        }
        SoilMoistureReadingError::InvalidValue { .. } => SoilMoistureRejectionReason::InvalidValue,
        SoilMoistureReadingError::EmptySource => SoilMoistureRejectionReason::EmptySource,
        SoilMoistureReadingError::EmptyCapturedAt => SoilMoistureRejectionReason::EmptyCapturedAt,
        SoilMoistureReadingError::EmptyReadingId
        | SoilMoistureReadingError::EmptyIngestedAt
        | SoilMoistureReadingError::EmptyRejectionId
        | SoilMoistureReadingError::EmptyRejectedAt
        | SoilMoistureReadingError::UnsupportedQaFlag { .. }
        | SoilMoistureReadingError::UnsupportedRejectionReason { .. } => {
            SoilMoistureRejectionReason::InvalidValue
        }
    }
}

fn normalize_soil_moisture_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(normalize_soil_moisture_text)
}

fn normalize_soil_moisture_text(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub const DROUGHT_INDEX_METHOD_STANDARDIZED_ANOMALY_V1: &str = "standardized_anomaly_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtIndexType {
    Spi,
    Spei,
}

impl DroughtIndexType {
    pub fn as_str(self) -> &'static str {
        match self {
            DroughtIndexType::Spi => "spi",
            DroughtIndexType::Spei => "spei",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DroughtIndexPeriod {
    pub start: String,
    pub end: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accumulation_days: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtIndexComputeRequest {
    #[serde(default)]
    pub index_id: Option<String>,
    pub field_or_region_ref: String,
    pub index_type: DroughtIndexType,
    pub period: DroughtIndexPeriod,
    pub observed_value: f64,
    pub baseline_mean: f64,
    pub baseline_std_dev: f64,
    #[serde(default)]
    pub input_refs: Vec<String>,
    #[serde(default)]
    pub computed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtIndexRecord {
    pub index_id: String,
    pub field_or_region_ref: String,
    pub index_type: DroughtIndexType,
    pub value: f64,
    pub period: DroughtIndexPeriod,
    pub input_refs: Vec<String>,
    pub method: String,
    pub computed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DroughtIndexError {
    #[error("drought index_id cannot be empty")]
    EmptyIndexId,
    #[error("drought index requires field_or_region_ref")]
    EmptyFieldOrRegionRef,
    #[error("drought index period start cannot be empty")]
    EmptyPeriodStart,
    #[error("drought index period end cannot be empty")]
    EmptyPeriodEnd,
    #[error("drought index period range is invalid: {start}..{end}")]
    InvalidPeriodRange { start: String, end: String },
    #[error("drought index accumulation_days must be positive")]
    InvalidAccumulationDays,
    #[error("drought index observed_value must be finite")]
    InvalidObservedValue,
    #[error("drought index baseline_mean must be finite")]
    InvalidBaselineMean,
    #[error("drought index baseline_std_dev must be finite and positive")]
    InvalidBaselineStdDev,
    #[error("drought index requires at least one input reference")]
    EmptyInputRefs,
    #[error("drought index computed_at cannot be empty")]
    EmptyComputedAt,
    #[error("unsupported drought index type {value}")]
    UnsupportedIndexType { value: String },
}

pub fn compute_drought_index(
    request: DroughtIndexComputeRequest,
    generated_index_id: String,
    computed_at: String,
) -> Result<DroughtIndexRecord, DroughtIndexError> {
    let index_id = normalize_drought_optional_text(request.index_id)
        .or_else(|| normalize_drought_text(generated_index_id))
        .ok_or(DroughtIndexError::EmptyIndexId)?;
    let field_or_region_ref = normalize_drought_text(request.field_or_region_ref)
        .ok_or(DroughtIndexError::EmptyFieldOrRegionRef)?;
    let period = normalize_drought_period(request.period)?;
    let input_refs = normalize_drought_input_refs(request.input_refs)?;
    if !request.observed_value.is_finite() {
        return Err(DroughtIndexError::InvalidObservedValue);
    }
    if !request.baseline_mean.is_finite() {
        return Err(DroughtIndexError::InvalidBaselineMean);
    }
    if !(request.baseline_std_dev.is_finite() && request.baseline_std_dev > 0.0) {
        return Err(DroughtIndexError::InvalidBaselineStdDev);
    }
    let computed_at = normalize_drought_optional_text(request.computed_at)
        .or_else(|| normalize_drought_text(computed_at))
        .ok_or(DroughtIndexError::EmptyComputedAt)?;
    let value = (request.observed_value - request.baseline_mean) / request.baseline_std_dev;

    Ok(DroughtIndexRecord {
        index_id,
        field_or_region_ref,
        index_type: request.index_type,
        value,
        period,
        input_refs,
        method: DROUGHT_INDEX_METHOD_STANDARDIZED_ANOMALY_V1.to_string(),
        computed_at,
    })
}

pub fn parse_drought_index_type(value: &str) -> Result<DroughtIndexType, DroughtIndexError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "spi" => Ok(DroughtIndexType::Spi),
        "spei" => Ok(DroughtIndexType::Spei),
        _ => Err(DroughtIndexError::UnsupportedIndexType {
            value: value.to_string(),
        }),
    }
}

fn normalize_drought_period(
    period: DroughtIndexPeriod,
) -> Result<DroughtIndexPeriod, DroughtIndexError> {
    let start = normalize_drought_text(period.start).ok_or(DroughtIndexError::EmptyPeriodStart)?;
    let end = normalize_drought_text(period.end).ok_or(DroughtIndexError::EmptyPeriodEnd)?;
    if start > end {
        return Err(DroughtIndexError::InvalidPeriodRange { start, end });
    }
    if period.accumulation_days == Some(0) {
        return Err(DroughtIndexError::InvalidAccumulationDays);
    }

    Ok(DroughtIndexPeriod {
        start,
        end,
        accumulation_days: period.accumulation_days,
    })
}

fn normalize_drought_input_refs(input_refs: Vec<String>) -> Result<Vec<String>, DroughtIndexError> {
    let input_refs = input_refs
        .into_iter()
        .filter_map(normalize_drought_text)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if input_refs.is_empty() {
        Err(DroughtIndexError::EmptyInputRefs)
    } else {
        Ok(input_refs)
    }
}

fn normalize_drought_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(normalize_drought_text)
}

fn normalize_drought_text(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketplacePartyType {
    Supplier,
    Buyer,
    Grower,
}

impl MarketplacePartyType {
    pub fn as_str(self) -> &'static str {
        match self {
            MarketplacePartyType::Supplier => "supplier",
            MarketplacePartyType::Buyer => "buyer",
            MarketplacePartyType::Grower => "grower",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceAccountStatus {
    Pending,
    Active,
    Suspended,
}

impl Default for MarketplaceAccountStatus {
    fn default() -> Self {
        MarketplaceAccountStatus::Active
    }
}

impl MarketplaceAccountStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            MarketplaceAccountStatus::Pending => "pending",
            MarketplaceAccountStatus::Active => "active",
            MarketplaceAccountStatus::Suspended => "suspended",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceAccountCreateRequest {
    #[serde(default)]
    pub account_id: Option<String>,
    pub org_id: String,
    pub party_type: MarketplacePartyType,
    #[serde(default)]
    pub role_refs: Vec<String>,
    #[serde(default)]
    pub status: Option<MarketplaceAccountStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceAccountRecord {
    pub account_id: String,
    pub org_id: String,
    pub party_type: MarketplacePartyType,
    pub role_refs: Vec<String>,
    pub status: MarketplaceAccountStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarketplaceAccountError {
    #[error("marketplace account_id cannot be empty")]
    EmptyAccountId,
    #[error("marketplace account org_id cannot be empty")]
    EmptyOrgId,
    #[error("marketplace account role_refs cannot be empty")]
    EmptyRoleRefs,
    #[error("marketplace organization not found: {org_id}")]
    OrganizationNotFound { org_id: String },
    #[error("marketplace account created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("marketplace account updated_at cannot be empty")]
    EmptyUpdatedAt,
    #[error(
        "invalid marketplace account lifecycle transition for {account_id}: {from:?} -> {to:?}"
    )]
    InvalidStatusTransition {
        account_id: String,
        from: MarketplaceAccountStatus,
        to: MarketplaceAccountStatus,
    },
    #[error("unsupported marketplace party type {value}")]
    UnsupportedPartyType { value: String },
    #[error("unsupported marketplace account status {value}")]
    UnsupportedStatus { value: String },
}

pub fn build_marketplace_account_record(
    request: MarketplaceAccountCreateRequest,
    org_exists: bool,
    generated_account_id: String,
    created_at: String,
) -> Result<MarketplaceAccountRecord, MarketplaceAccountError> {
    let account_id = normalize_marketplace_optional_text(request.account_id)
        .or_else(|| normalize_marketplace_text(generated_account_id))
        .ok_or(MarketplaceAccountError::EmptyAccountId)?;
    let org_id =
        normalize_marketplace_text(request.org_id).ok_or(MarketplaceAccountError::EmptyOrgId)?;
    if !org_exists {
        return Err(MarketplaceAccountError::OrganizationNotFound { org_id });
    }
    let role_refs = normalize_marketplace_role_refs(request.role_refs)?;
    let created_at =
        normalize_marketplace_text(created_at).ok_or(MarketplaceAccountError::EmptyCreatedAt)?;
    Ok(MarketplaceAccountRecord {
        account_id,
        org_id,
        party_type: request.party_type,
        role_refs,
        status: request.status.unwrap_or_default(),
        created_at: created_at.clone(),
        updated_at: created_at,
    })
}

pub fn transition_marketplace_account_status(
    record: &MarketplaceAccountRecord,
    to: MarketplaceAccountStatus,
    updated_at: String,
) -> Result<MarketplaceAccountRecord, MarketplaceAccountError> {
    if !valid_marketplace_account_transition(record.status, to) {
        return Err(MarketplaceAccountError::InvalidStatusTransition {
            account_id: record.account_id.clone(),
            from: record.status,
            to,
        });
    }
    let updated_at =
        normalize_marketplace_text(updated_at).ok_or(MarketplaceAccountError::EmptyUpdatedAt)?;
    let mut updated = record.clone();
    updated.status = to;
    updated.updated_at = updated_at;
    Ok(updated)
}

pub fn parse_marketplace_party_type(
    value: &str,
) -> Result<MarketplacePartyType, MarketplaceAccountError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "supplier" => Ok(MarketplacePartyType::Supplier),
        "buyer" => Ok(MarketplacePartyType::Buyer),
        "grower" => Ok(MarketplacePartyType::Grower),
        _ => Err(MarketplaceAccountError::UnsupportedPartyType {
            value: value.to_string(),
        }),
    }
}

pub fn parse_marketplace_account_status(
    value: &str,
) -> Result<MarketplaceAccountStatus, MarketplaceAccountError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "pending" => Ok(MarketplaceAccountStatus::Pending),
        "active" => Ok(MarketplaceAccountStatus::Active),
        "suspended" => Ok(MarketplaceAccountStatus::Suspended),
        _ => Err(MarketplaceAccountError::UnsupportedStatus {
            value: value.to_string(),
        }),
    }
}

fn valid_marketplace_account_transition(
    from: MarketplaceAccountStatus,
    to: MarketplaceAccountStatus,
) -> bool {
    from == to
        || matches!(
            (from, to),
            (
                MarketplaceAccountStatus::Pending,
                MarketplaceAccountStatus::Active
            ) | (
                MarketplaceAccountStatus::Active,
                MarketplaceAccountStatus::Suspended
            )
        )
}

fn normalize_marketplace_role_refs(
    role_refs: Vec<String>,
) -> Result<Vec<String>, MarketplaceAccountError> {
    let role_refs = role_refs
        .into_iter()
        .filter_map(normalize_marketplace_text)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if role_refs.is_empty() {
        Err(MarketplaceAccountError::EmptyRoleRefs)
    } else {
        Ok(role_refs)
    }
}

fn normalize_marketplace_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(normalize_marketplace_text)
}

fn normalize_marketplace_text(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SustainabilityMetricType {
    CarbonFootprint,
    Biomass,
    Biodiversity,
    SoilCarbon,
    SustainabilityKpi,
}

impl SustainabilityMetricType {
    pub fn as_str(self) -> &'static str {
        match self {
            SustainabilityMetricType::CarbonFootprint => "carbon_footprint",
            SustainabilityMetricType::Biomass => "biomass",
            SustainabilityMetricType::Biodiversity => "biodiversity",
            SustainabilityMetricType::SoilCarbon => "soil_carbon",
            SustainabilityMetricType::SustainabilityKpi => "sustainability_kpi",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SustainabilityRecordCreateRequest {
    #[serde(default)]
    pub record_id: Option<String>,
    pub field_id: String,
    pub season_id: String,
    pub operation_id: String,
    pub metric_type: SustainabilityMetricType,
    pub method_version: String,
    #[serde(default)]
    pub audit_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SustainabilityRecord {
    pub record_id: String,
    pub field_id: String,
    pub season_id: String,
    pub operation_id: String,
    pub metric_type: SustainabilityMetricType,
    pub method_version: String,
    pub created_at: String,
    pub audit_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SustainabilityRecordLinkage {
    pub field_id: String,
    pub season_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SustainabilityRecordError {
    #[error("sustainability record_id cannot be empty")]
    EmptyRecordId,
    #[error("sustainability field_id cannot be empty")]
    EmptyFieldId,
    #[error("sustainability season_id cannot be empty")]
    EmptySeasonId,
    #[error("sustainability operation_id cannot be empty")]
    EmptyOperationId,
    #[error("sustainability method_version cannot be empty")]
    EmptyMethodVersion,
    #[error("sustainability created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("sustainability audit_id cannot be empty")]
    EmptyAuditId,
    #[error("sustainability field not found: {field_id}")]
    FieldNotFound { field_id: String },
    #[error("sustainability season not found: {season_id} for field {field_id}")]
    SeasonNotFound { field_id: String, season_id: String },
    #[error(
        "sustainability season {requested_season_id} does not belong to field {field_id}; linked season is {linked_season_id}"
    )]
    SeasonFieldMismatch {
        field_id: String,
        requested_season_id: String,
        linked_season_id: String,
    },
    #[error("unsupported sustainability metric type {value}")]
    UnsupportedMetricType { value: String },
}

pub fn build_sustainability_record(
    request: SustainabilityRecordCreateRequest,
    linkage: Option<SustainabilityRecordLinkage>,
    generated_record_id: String,
    generated_audit_id: String,
    created_at: String,
) -> Result<SustainabilityRecord, SustainabilityRecordError> {
    let record_id = normalize_sustainability_optional_text(request.record_id)
        .or_else(|| normalize_sustainability_text(generated_record_id))
        .ok_or(SustainabilityRecordError::EmptyRecordId)?;
    let field_id = normalize_sustainability_text(request.field_id)
        .ok_or(SustainabilityRecordError::EmptyFieldId)?;
    let season_id = normalize_sustainability_text(request.season_id)
        .ok_or(SustainabilityRecordError::EmptySeasonId)?;
    let operation_id = normalize_sustainability_text(request.operation_id)
        .ok_or(SustainabilityRecordError::EmptyOperationId)?;
    let method_version = normalize_sustainability_text(request.method_version)
        .ok_or(SustainabilityRecordError::EmptyMethodVersion)?;
    let audit_id = normalize_sustainability_optional_text(request.audit_id)
        .or_else(|| normalize_sustainability_text(generated_audit_id))
        .ok_or(SustainabilityRecordError::EmptyAuditId)?;
    let created_at = normalize_sustainability_text(created_at)
        .ok_or(SustainabilityRecordError::EmptyCreatedAt)?;

    let linkage = linkage.ok_or_else(|| SustainabilityRecordError::FieldNotFound {
        field_id: field_id.clone(),
    })?;
    let linked_field_id = normalize_sustainability_text(linkage.field_id).ok_or_else(|| {
        SustainabilityRecordError::FieldNotFound {
            field_id: field_id.clone(),
        }
    })?;
    if linked_field_id != field_id {
        return Err(SustainabilityRecordError::FieldNotFound { field_id });
    }
    let linked_season_id =
        normalize_sustainability_optional_text(linkage.season_id).ok_or_else(|| {
            SustainabilityRecordError::SeasonNotFound {
                field_id: field_id.clone(),
                season_id: season_id.clone(),
            }
        })?;
    if linked_season_id != season_id {
        return Err(SustainabilityRecordError::SeasonFieldMismatch {
            field_id,
            requested_season_id: season_id,
            linked_season_id,
        });
    }

    Ok(SustainabilityRecord {
        record_id,
        field_id: linked_field_id,
        season_id: linked_season_id,
        operation_id,
        metric_type: request.metric_type,
        method_version,
        created_at,
        audit_id,
    })
}

pub fn parse_sustainability_metric_type(
    value: &str,
) -> Result<SustainabilityMetricType, SustainabilityRecordError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "carbon_footprint" => Ok(SustainabilityMetricType::CarbonFootprint),
        "biomass" => Ok(SustainabilityMetricType::Biomass),
        "biodiversity" => Ok(SustainabilityMetricType::Biodiversity),
        "soil_carbon" => Ok(SustainabilityMetricType::SoilCarbon),
        "sustainability_kpi" => Ok(SustainabilityMetricType::SustainabilityKpi),
        _ => Err(SustainabilityRecordError::UnsupportedMetricType {
            value: value.to_string(),
        }),
    }
}

fn normalize_sustainability_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(normalize_sustainability_text)
}

fn normalize_sustainability_text(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    Article,
    Guide,
    Post,
}

impl ContentType {
    pub fn as_str(self) -> &'static str {
        match self {
            ContentType::Article => "article",
            ContentType::Guide => "guide",
            ContentType::Post => "post",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentStatus {
    Draft,
    InReview,
    Published,
    Rejected,
    Unpublished,
}

impl Default for ContentStatus {
    fn default() -> Self {
        ContentStatus::Draft
    }
}

impl ContentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ContentStatus::Draft => "draft",
            ContentStatus::InReview => "in_review",
            ContentStatus::Published => "published",
            ContentStatus::Rejected => "rejected",
            ContentStatus::Unpublished => "unpublished",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentCreateRequest {
    #[serde(default)]
    pub content_id: Option<String>,
    pub content_type: ContentType,
    pub author_id: String,
    pub org_id: String,
    pub body: String,
    #[serde(default)]
    pub status: Option<ContentStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentEditRequest {
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentRecord {
    pub content_id: String,
    pub content_type: ContentType,
    pub author_id: String,
    pub org_id: String,
    pub status: ContentStatus,
    pub current_version: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentVersionRecord {
    pub version_id: String,
    pub content_id: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionedContentRecord {
    pub content: ContentRecord,
    pub versions: Vec<ContentVersionRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContentError {
    #[error("content_id cannot be empty")]
    EmptyContentId,
    #[error("content author_id cannot be empty")]
    EmptyAuthorId,
    #[error("content org_id cannot be empty")]
    EmptyOrgId,
    #[error("content version_id cannot be empty")]
    EmptyVersionId,
    #[error("content body cannot be empty")]
    EmptyBody,
    #[error("content created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("unsupported content type {value}")]
    UnsupportedContentType { value: String },
    #[error("unsupported content status {value}")]
    UnsupportedContentStatus { value: String },
}

pub fn create_versioned_content(
    request: ContentCreateRequest,
    generated_content_id: String,
    generated_version_id: String,
    created_at: String,
) -> Result<(ContentRecord, ContentVersionRecord), ContentError> {
    let content_id = normalize_content_optional_text(request.content_id)
        .or_else(|| normalize_content_text(generated_content_id))
        .ok_or(ContentError::EmptyContentId)?;
    let author_id = normalize_content_text(request.author_id).ok_or(ContentError::EmptyAuthorId)?;
    let org_id = normalize_content_text(request.org_id).ok_or(ContentError::EmptyOrgId)?;
    let version_id =
        normalize_content_text(generated_version_id).ok_or(ContentError::EmptyVersionId)?;
    let body = normalize_content_text(request.body).ok_or(ContentError::EmptyBody)?;
    let created_at = normalize_content_text(created_at).ok_or(ContentError::EmptyCreatedAt)?;
    let version = ContentVersionRecord {
        version_id: version_id.clone(),
        content_id: content_id.clone(),
        body,
        created_at: created_at.clone(),
    };
    let content = ContentRecord {
        content_id,
        content_type: request.content_type,
        author_id,
        org_id,
        status: request.status.unwrap_or_default(),
        current_version: version_id,
        created_at: created_at.clone(),
        updated_at: created_at,
    };

    Ok((content, version))
}

pub fn append_content_version(
    content: &ContentRecord,
    body: String,
    generated_version_id: String,
    created_at: String,
) -> Result<(ContentRecord, ContentVersionRecord), ContentError> {
    let version_id =
        normalize_content_text(generated_version_id).ok_or(ContentError::EmptyVersionId)?;
    let body = normalize_content_text(body).ok_or(ContentError::EmptyBody)?;
    let created_at = normalize_content_text(created_at).ok_or(ContentError::EmptyCreatedAt)?;
    let mut updated = content.clone();
    updated.current_version = version_id.clone();
    updated.updated_at = created_at.clone();
    let version = ContentVersionRecord {
        version_id,
        content_id: content.content_id.clone(),
        body,
        created_at,
    };

    Ok((updated, version))
}

pub fn parse_content_type(value: &str) -> Result<ContentType, ContentError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "article" => Ok(ContentType::Article),
        "guide" => Ok(ContentType::Guide),
        "post" => Ok(ContentType::Post),
        _ => Err(ContentError::UnsupportedContentType {
            value: value.to_string(),
        }),
    }
}

pub fn parse_content_status(value: &str) -> Result<ContentStatus, ContentError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "draft" => Ok(ContentStatus::Draft),
        "in_review" => Ok(ContentStatus::InReview),
        "published" => Ok(ContentStatus::Published),
        "rejected" => Ok(ContentStatus::Rejected),
        "unpublished" => Ok(ContentStatus::Unpublished),
        _ => Err(ContentError::UnsupportedContentStatus {
            value: value.to_string(),
        }),
    }
}

fn normalize_content_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(normalize_content_text)
}

fn normalize_content_text(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationChannelCreateRequest {
    #[serde(default)]
    pub channel_id: Option<String>,
    pub org_id: String,
    pub field_ref: String,
    #[serde(default)]
    pub member_account_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationChannelRecord {
    pub channel_id: String,
    pub org_id: String,
    pub field_ref: String,
    pub member_account_ids: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationMessageCreateRequest {
    #[serde(default)]
    pub message_id: Option<String>,
    pub author_id: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationMessageRecord {
    pub message_id: String,
    pub channel_id: String,
    pub author_id: String,
    pub body: String,
    pub sent_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationChannelThread {
    pub channel: CollaborationChannelRecord,
    pub messages: Vec<CollaborationMessageRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CollaborationError {
    #[error("collaboration channel_id cannot be empty")]
    EmptyChannelId,
    #[error("collaboration org_id cannot be empty")]
    EmptyOrgId,
    #[error("collaboration field_ref cannot be empty")]
    EmptyFieldRef,
    #[error("collaboration channel members cannot be empty")]
    EmptyMembers,
    #[error("collaboration message_id cannot be empty")]
    EmptyMessageId,
    #[error("collaboration author_id cannot be empty")]
    EmptyAuthorId,
    #[error("collaboration message body cannot be empty")]
    EmptyBody,
    #[error("collaboration timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("collaboration channel not found: {channel_id:?}")]
    ChannelNotFound { channel_id: Option<String> },
    #[error("author {author_id} is not a member of channel {channel_id}")]
    AuthorNotChannelMember {
        channel_id: String,
        author_id: String,
    },
}

pub fn build_collaboration_channel(
    request: CollaborationChannelCreateRequest,
    generated_channel_id: String,
    created_at: String,
) -> Result<CollaborationChannelRecord, CollaborationError> {
    let channel_id = normalize_collaboration_optional_text(request.channel_id)
        .or_else(|| normalize_collaboration_text(generated_channel_id))
        .ok_or(CollaborationError::EmptyChannelId)?;
    let org_id =
        normalize_collaboration_text(request.org_id).ok_or(CollaborationError::EmptyOrgId)?;
    let field_ref =
        normalize_collaboration_text(request.field_ref).ok_or(CollaborationError::EmptyFieldRef)?;
    let member_account_ids = normalize_collaboration_members(request.member_account_ids)?;
    let created_at =
        normalize_collaboration_text(created_at).ok_or(CollaborationError::EmptyTimestamp)?;

    Ok(CollaborationChannelRecord {
        channel_id,
        org_id,
        field_ref,
        member_account_ids,
        created_at,
    })
}

pub fn build_collaboration_message(
    request: CollaborationMessageCreateRequest,
    channel: Option<&CollaborationChannelRecord>,
    generated_message_id: String,
    sent_at: String,
) -> Result<CollaborationMessageRecord, CollaborationError> {
    let message_id = normalize_collaboration_optional_text(request.message_id)
        .or_else(|| normalize_collaboration_text(generated_message_id))
        .ok_or(CollaborationError::EmptyMessageId)?;
    let author_id =
        normalize_collaboration_text(request.author_id).ok_or(CollaborationError::EmptyAuthorId)?;
    let body = normalize_collaboration_text(request.body).ok_or(CollaborationError::EmptyBody)?;
    let sent_at =
        normalize_collaboration_text(sent_at).ok_or(CollaborationError::EmptyTimestamp)?;
    let channel = channel.ok_or(CollaborationError::ChannelNotFound { channel_id: None })?;
    if !channel
        .member_account_ids
        .iter()
        .any(|member_id| member_id == &author_id)
    {
        return Err(CollaborationError::AuthorNotChannelMember {
            channel_id: channel.channel_id.clone(),
            author_id,
        });
    }

    Ok(CollaborationMessageRecord {
        message_id,
        channel_id: channel.channel_id.clone(),
        author_id,
        body,
        sent_at,
    })
}

fn normalize_collaboration_members(
    members: Vec<String>,
) -> Result<Vec<String>, CollaborationError> {
    let members = members
        .into_iter()
        .filter_map(normalize_collaboration_text)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if members.is_empty() {
        Err(CollaborationError::EmptyMembers)
    } else {
        Ok(members)
    }
}

fn normalize_collaboration_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(normalize_collaboration_text)
}

fn normalize_collaboration_text(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetNodeComponentHealth {
    Ok,
    Warn,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetNodeComponentStatus {
    pub component: String,
    pub health: FleetNodeComponentHealth,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetConfigBundle {
    pub node_id: String,
    pub version: u64,
    pub payload: String,
    #[serde(default)]
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetConfigState {
    pub node_id: String,
    pub applied_version: u64,
    pub payload: String,
    pub applied_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetConfigApplyStatus {
    Applied,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetConfigRejectionReason {
    MissingSignature,
    InvalidSignature,
    OlderOrEqualVersion,
    DryRunFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetConfigApplyOutcome {
    pub node_id: String,
    pub previous_version: u64,
    pub requested_version: u64,
    pub status: FleetConfigApplyStatus,
    #[serde(default)]
    pub rejection_reason: Option<FleetConfigRejectionReason>,
    pub updated_state: FleetConfigState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetConfigDryRunDiff {
    pub field: String,
    pub current: String,
    pub proposed: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetConfigDryRunReport {
    pub node_id: String,
    pub previous_version: u64,
    pub requested_version: u64,
    pub status: FleetConfigApplyStatus,
    pub would_apply: bool,
    #[serde(default)]
    pub rejection_reason: Option<FleetConfigRejectionReason>,
    #[serde(default)]
    pub diffs: Vec<FleetConfigDryRunDiff>,
    pub bundle_signature: String,
    pub payload_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FleetConfigDistributionError {
    #[error("config node_id cannot be empty")]
    EmptyNodeId,
    #[error("config payload cannot be empty")]
    EmptyPayload,
    #[error("config applied_at cannot be empty")]
    EmptyAppliedAt,
    #[error("config signing key cannot be empty")]
    EmptySigningKey,
    #[error("config bundle node_id {actual} does not match node {expected}")]
    NodeIdMismatch { expected: String, actual: String },
    #[error("config dry-run report does not match the bundle being applied")]
    DryRunBundleMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetNodeHeartbeat {
    pub node_id: String,
    pub version: String,
    #[serde(default)]
    pub config_version: u64,
    #[serde(default)]
    pub components: Vec<FleetNodeComponentStatus>,
    pub uptime_seconds: u64,
    pub at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub runtime_mode: FleetNodeRuntimeMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetNodeHealthState {
    Fresh,
    Stale,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetNodeHealthSnapshot {
    pub node_id: String,
    pub version: String,
    pub config_version: u64,
    pub components: Vec<FleetNodeComponentStatus>,
    pub capabilities: Vec<String>,
    pub runtime_mode: FleetNodeRuntimeMode,
    pub uptime_seconds: u64,
    pub last_heartbeat_at: chrono::DateTime<chrono::Utc>,
    pub heartbeat_age_seconds: u64,
    pub state: FleetNodeHealthState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetHeartbeatEvaluation {
    pub updated_record: FleetNodeRecord,
    pub health: FleetNodeHealthSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetNodeInventoryEntry {
    pub node_id: String,
    pub owner_org_id: String,
    pub kind: FleetNodeKind,
    pub runtime_mode: FleetNodeRuntimeMode,
    pub status: FleetNodeStatus,
    pub maintenance: bool,
    pub version: Option<String>,
    pub config_version: Option<u64>,
    pub components: Vec<FleetNodeComponentStatus>,
    pub capabilities: Vec<String>,
    pub health_state: Option<FleetNodeHealthState>,
    pub heartbeat_age_seconds: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetInventoryFilter {
    #[serde(default)]
    pub owner_org_id: Option<String>,
    #[serde(default)]
    pub status: Option<FleetNodeStatus>,
    #[serde(default)]
    pub runtime_mode: Option<FleetNodeRuntimeMode>,
    #[serde(default)]
    pub include_maintenance: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetVersionInventory {
    pub entries: Vec<FleetNodeInventoryEntry>,
}

impl FleetVersionInventory {
    pub fn rollout_target_node_ids(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| !entry.maintenance)
            .map(|entry| entry.node_id.clone())
            .collect()
    }
}

impl FleetHeartbeatEvaluation {
    pub fn from_heartbeat(
        record: &FleetNodeRecord,
        heartbeat: &FleetNodeHeartbeat,
        evaluated_at: chrono::DateTime<chrono::Utc>,
        stale_after: std::time::Duration,
        down_after: std::time::Duration,
    ) -> Result<Self, FleetNodeHeartbeatError> {
        apply_fleet_node_heartbeat(
            record,
            heartbeat.clone(),
            evaluated_at,
            stale_after,
            down_after,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FleetNodeHeartbeatError {
    #[error("heartbeat node_id cannot be empty")]
    EmptyNodeId,
    #[error("heartbeat node_id {actual} does not match enrolled node {expected}")]
    NodeIdMismatch { expected: String, actual: String },
    #[error("heartbeat version cannot be empty")]
    EmptyVersion,
    #[error("heartbeat components cannot be empty")]
    EmptyComponents,
    #[error("heartbeat component name cannot be empty")]
    EmptyComponentName,
    #[error("heartbeat capabilities cannot be empty")]
    EmptyCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FleetNodeOperationError {
    #[error("flight mode required for node {node_id}, actual mode {actual:?}")]
    FlightModeRequired {
        node_id: String,
        actual: FleetNodeRuntimeMode,
    },
}

pub fn apply_fleet_node_heartbeat(
    record: &FleetNodeRecord,
    heartbeat: FleetNodeHeartbeat,
    evaluated_at: chrono::DateTime<chrono::Utc>,
    stale_after: std::time::Duration,
    down_after: std::time::Duration,
) -> Result<FleetHeartbeatEvaluation, FleetNodeHeartbeatError> {
    let heartbeat = normalize_fleet_node_heartbeat(heartbeat)?;
    if heartbeat.node_id != record.node_id {
        return Err(FleetNodeHeartbeatError::NodeIdMismatch {
            expected: record.node_id.clone(),
            actual: heartbeat.node_id,
        });
    }

    let heartbeat_age_seconds = evaluated_at
        .signed_duration_since(heartbeat.at)
        .num_seconds()
        .max(0) as u64;
    let state = if heartbeat_age_seconds > down_after.as_secs() {
        FleetNodeHealthState::Down
    } else if heartbeat_age_seconds > stale_after.as_secs() {
        FleetNodeHealthState::Stale
    } else {
        FleetNodeHealthState::Fresh
    };

    let mut updated_record = record.clone();
    updated_record.capabilities = heartbeat.capabilities.clone();
    updated_record.runtime_mode = heartbeat.runtime_mode;

    Ok(FleetHeartbeatEvaluation {
        updated_record,
        health: FleetNodeHealthSnapshot {
            node_id: heartbeat.node_id,
            version: heartbeat.version,
            config_version: heartbeat.config_version,
            components: heartbeat.components,
            capabilities: heartbeat.capabilities,
            runtime_mode: heartbeat.runtime_mode,
            uptime_seconds: heartbeat.uptime_seconds,
            last_heartbeat_at: heartbeat.at,
            heartbeat_age_seconds,
            state,
        },
    })
}

pub fn build_fleet_version_inventory(
    records: &[FleetNodeRecord],
    health_snapshots: &[FleetNodeHealthSnapshot],
    filter: FleetInventoryFilter,
) -> FleetVersionInventory {
    let health_by_node = health_snapshots
        .iter()
        .map(|health| (health.node_id.as_str(), health))
        .collect::<BTreeMap<_, _>>();
    let owner_filter = filter
        .owner_org_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let mut entries = records
        .iter()
        .filter(|record| owner_filter.is_none_or(|owner| record.owner_org_id == owner))
        .filter(|record| filter.status.is_none_or(|status| record.status == status))
        .filter(|record| {
            filter
                .runtime_mode
                .is_none_or(|mode| record.runtime_mode == mode)
        })
        .filter(|record| {
            filter.include_maintenance || record.status != FleetNodeStatus::Maintenance
        })
        .map(|record| {
            let health = health_by_node.get(record.node_id.as_str()).copied();
            FleetNodeInventoryEntry {
                node_id: record.node_id.clone(),
                owner_org_id: record.owner_org_id.clone(),
                kind: record.kind,
                runtime_mode: health
                    .map(|snapshot| snapshot.runtime_mode)
                    .unwrap_or(record.runtime_mode),
                status: record.status,
                maintenance: record.status == FleetNodeStatus::Maintenance,
                version: health.map(|snapshot| snapshot.version.clone()),
                config_version: health.map(|snapshot| snapshot.config_version),
                components: health
                    .map(|snapshot| snapshot.components.clone())
                    .unwrap_or_default(),
                capabilities: health
                    .map(|snapshot| snapshot.capabilities.clone())
                    .unwrap_or_else(|| record.capabilities.clone()),
                health_state: health.map(|snapshot| snapshot.state),
                heartbeat_age_seconds: health.map(|snapshot| snapshot.heartbeat_age_seconds),
            }
        })
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| {
        left.owner_org_id
            .cmp(&right.owner_org_id)
            .then(left.node_id.cmp(&right.node_id))
    });

    FleetVersionInventory { entries }
}

pub fn assert_flight_operation_allowed(
    record: &FleetNodeRecord,
) -> Result<(), FleetNodeOperationError> {
    if record.runtime_mode == FleetNodeRuntimeMode::Flight {
        Ok(())
    } else {
        Err(FleetNodeOperationError::FlightModeRequired {
            node_id: record.node_id.clone(),
            actual: record.runtime_mode,
        })
    }
}

fn normalize_fleet_node_heartbeat(
    heartbeat: FleetNodeHeartbeat,
) -> Result<FleetNodeHeartbeat, FleetNodeHeartbeatError> {
    let node_id =
        normalize_heartbeat_text(heartbeat.node_id, FleetNodeHeartbeatError::EmptyNodeId)?;
    let version =
        normalize_heartbeat_text(heartbeat.version, FleetNodeHeartbeatError::EmptyVersion)?;
    let capabilities = normalize_heartbeat_capabilities(heartbeat.capabilities)?;
    let components = heartbeat
        .components
        .into_iter()
        .map(normalize_heartbeat_component)
        .collect::<Result<Vec<_>, _>>()?;
    if components.is_empty() {
        return Err(FleetNodeHeartbeatError::EmptyComponents);
    }

    Ok(FleetNodeHeartbeat {
        node_id,
        version,
        config_version: heartbeat.config_version,
        components,
        uptime_seconds: heartbeat.uptime_seconds,
        at: heartbeat.at,
        capabilities,
        runtime_mode: heartbeat.runtime_mode,
    })
}

pub fn sign_fleet_config_bundle(
    node_id: &str,
    version: u64,
    payload: &str,
    signing_key: &str,
) -> String {
    let canonical = format!(
        "{}|{}|{}|{}",
        node_id.trim(),
        version,
        payload,
        signing_key.trim()
    );
    format!("agbot-config-v1:{:016x}", fnv1a64(canonical.as_bytes()))
}

pub fn verify_and_apply_fleet_config_bundle(
    current: &FleetConfigState,
    bundle: FleetConfigBundle,
    verifying_key: &str,
    applied_at: String,
) -> Result<FleetConfigApplyOutcome, FleetConfigDistributionError> {
    let validation = validate_fleet_config_bundle(current, bundle, verifying_key)?;
    if let Some(reason) = validation.rejection_reason {
        return Ok(rejected_config_outcome(
            validation.current_state,
            validation.bundle.version,
            reason,
        ));
    }

    let applied_at =
        normalize_config_text(applied_at, FleetConfigDistributionError::EmptyAppliedAt)?;
    let previous_version = validation.current_state.applied_version;
    let updated_state = FleetConfigState {
        node_id: validation.current_state.node_id.clone(),
        applied_version: validation.bundle.version,
        payload: validation.bundle.payload,
        applied_at,
    };

    Ok(FleetConfigApplyOutcome {
        node_id: updated_state.node_id.clone(),
        previous_version,
        requested_version: updated_state.applied_version,
        status: FleetConfigApplyStatus::Applied,
        rejection_reason: None,
        updated_state,
    })
}

pub fn dry_run_fleet_config_bundle(
    current: &FleetConfigState,
    bundle: FleetConfigBundle,
    verifying_key: &str,
) -> Result<FleetConfigDryRunReport, FleetConfigDistributionError> {
    let validation = validate_fleet_config_bundle(current, bundle, verifying_key)?;
    let status = if validation.rejection_reason.is_some() {
        FleetConfigApplyStatus::Rejected
    } else {
        FleetConfigApplyStatus::Applied
    };
    let would_apply = status == FleetConfigApplyStatus::Applied;
    let diffs = if would_apply {
        fleet_config_diffs(&validation.current_state, &validation.bundle)
    } else {
        Vec::new()
    };

    Ok(FleetConfigDryRunReport {
        node_id: validation.current_state.node_id,
        previous_version: validation.current_state.applied_version,
        requested_version: validation.bundle.version,
        status,
        would_apply,
        rejection_reason: validation.rejection_reason,
        diffs,
        bundle_signature: validation.bundle.signature,
        payload_fingerprint: config_payload_fingerprint(&validation.bundle.payload),
    })
}

pub fn apply_dry_run_validated_fleet_config_bundle(
    current: &FleetConfigState,
    bundle: FleetConfigBundle,
    verifying_key: &str,
    applied_at: String,
    dry_run: &FleetConfigDryRunReport,
) -> Result<FleetConfigApplyOutcome, FleetConfigDistributionError> {
    let current_state = normalize_config_state(current)?;
    let bundle = normalize_config_bundle(bundle)?;
    if !dry_run_matches_bundle(&current_state, &bundle, dry_run) {
        return Err(FleetConfigDistributionError::DryRunBundleMismatch);
    }

    if dry_run.status != FleetConfigApplyStatus::Applied || !dry_run.would_apply {
        return Ok(rejected_config_outcome(
            current_state,
            bundle.version,
            dry_run
                .rejection_reason
                .unwrap_or(FleetConfigRejectionReason::DryRunFailed),
        ));
    }

    verify_and_apply_fleet_config_bundle(current, bundle, verifying_key, applied_at)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FleetConfigValidation {
    current_state: FleetConfigState,
    bundle: FleetConfigBundle,
    rejection_reason: Option<FleetConfigRejectionReason>,
}

fn validate_fleet_config_bundle(
    current: &FleetConfigState,
    bundle: FleetConfigBundle,
    verifying_key: &str,
) -> Result<FleetConfigValidation, FleetConfigDistributionError> {
    let current_state = normalize_config_state(current)?;
    let bundle = normalize_config_bundle(bundle)?;
    let verifying_key = normalize_config_text(
        verifying_key.to_string(),
        FleetConfigDistributionError::EmptySigningKey,
    )?;

    if bundle.node_id != current_state.node_id {
        return Err(FleetConfigDistributionError::NodeIdMismatch {
            expected: current_state.node_id,
            actual: bundle.node_id,
        });
    }

    let rejection_reason = if bundle.signature.is_empty() {
        Some(FleetConfigRejectionReason::MissingSignature)
    } else {
        let expected_signature = sign_fleet_config_bundle(
            &bundle.node_id,
            bundle.version,
            &bundle.payload,
            &verifying_key,
        );
        if bundle.signature != expected_signature {
            Some(FleetConfigRejectionReason::InvalidSignature)
        } else if bundle.version <= current_state.applied_version {
            Some(FleetConfigRejectionReason::OlderOrEqualVersion)
        } else {
            None
        }
    };

    Ok(FleetConfigValidation {
        current_state,
        bundle,
        rejection_reason,
    })
}

fn rejected_config_outcome(
    current_state: FleetConfigState,
    requested_version: u64,
    reason: FleetConfigRejectionReason,
) -> FleetConfigApplyOutcome {
    FleetConfigApplyOutcome {
        node_id: current_state.node_id.clone(),
        previous_version: current_state.applied_version,
        requested_version,
        status: FleetConfigApplyStatus::Rejected,
        rejection_reason: Some(reason),
        updated_state: current_state,
    }
}

fn fleet_config_diffs(
    current: &FleetConfigState,
    bundle: &FleetConfigBundle,
) -> Vec<FleetConfigDryRunDiff> {
    let mut diffs = Vec::new();
    if current.applied_version != bundle.version {
        diffs.push(FleetConfigDryRunDiff {
            field: "applied_version".to_string(),
            current: current.applied_version.to_string(),
            proposed: bundle.version.to_string(),
        });
    }
    if current.payload != bundle.payload {
        diffs.push(FleetConfigDryRunDiff {
            field: "payload".to_string(),
            current: current.payload.clone(),
            proposed: bundle.payload.clone(),
        });
    }
    diffs
}

fn dry_run_matches_bundle(
    current: &FleetConfigState,
    bundle: &FleetConfigBundle,
    dry_run: &FleetConfigDryRunReport,
) -> bool {
    dry_run.node_id == current.node_id
        && dry_run.previous_version == current.applied_version
        && dry_run.requested_version == bundle.version
        && dry_run.bundle_signature == bundle.signature
        && dry_run.payload_fingerprint == config_payload_fingerprint(&bundle.payload)
}

fn config_payload_fingerprint(payload: &str) -> String {
    format!("{:016x}", fnv1a64(payload.as_bytes()))
}

fn normalize_config_state(
    state: &FleetConfigState,
) -> Result<FleetConfigState, FleetConfigDistributionError> {
    Ok(FleetConfigState {
        node_id: normalize_config_text(
            state.node_id.clone(),
            FleetConfigDistributionError::EmptyNodeId,
        )?,
        applied_version: state.applied_version,
        payload: normalize_config_text(
            state.payload.clone(),
            FleetConfigDistributionError::EmptyPayload,
        )?,
        applied_at: normalize_config_text(
            state.applied_at.clone(),
            FleetConfigDistributionError::EmptyAppliedAt,
        )?,
    })
}

fn normalize_config_bundle(
    bundle: FleetConfigBundle,
) -> Result<FleetConfigBundle, FleetConfigDistributionError> {
    Ok(FleetConfigBundle {
        node_id: normalize_config_text(bundle.node_id, FleetConfigDistributionError::EmptyNodeId)?,
        version: bundle.version,
        payload: normalize_config_text(bundle.payload, FleetConfigDistributionError::EmptyPayload)?,
        signature: bundle.signature.trim().to_string(),
    })
}

fn normalize_config_text(
    value: String,
    error: FleetConfigDistributionError,
) -> Result<String, FleetConfigDistributionError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn normalize_heartbeat_component(
    component: FleetNodeComponentStatus,
) -> Result<FleetNodeComponentStatus, FleetNodeHeartbeatError> {
    Ok(FleetNodeComponentStatus {
        component: normalize_heartbeat_text(
            component.component,
            FleetNodeHeartbeatError::EmptyComponentName,
        )?,
        health: component.health,
        message: component.message.and_then(|message| {
            let message = message.trim();
            (!message.is_empty()).then(|| message.to_string())
        }),
    })
}

fn normalize_heartbeat_capabilities(
    capabilities: Vec<String>,
) -> Result<Vec<String>, FleetNodeHeartbeatError> {
    let capabilities = capabilities
        .into_iter()
        .filter_map(|capability| {
            let capability = capability.trim();
            (!capability.is_empty()).then(|| capability.to_ascii_lowercase())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    if capabilities.is_empty() {
        Err(FleetNodeHeartbeatError::EmptyCapabilities)
    } else {
        Ok(capabilities)
    }
}

fn normalize_heartbeat_text(
    value: String,
    error: FleetNodeHeartbeatError,
) -> Result<String, FleetNodeHeartbeatError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FarmFieldError {
    #[error("farm_id cannot be empty")]
    EmptyFarmId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("org_id cannot be empty")]
    EmptyOrgId,
    #[error("name cannot be empty")]
    EmptyName,
    #[error("season_id cannot be empty")]
    EmptySeasonId,
    #[error("crop_plan_id cannot be empty")]
    EmptyCropPlanId,
    #[error("crop cannot be empty")]
    EmptyCrop,
    #[error("scene_id cannot be empty")]
    EmptySceneId,
    #[error("layer_id cannot be empty")]
    EmptyLayerId,
    #[error("product_type cannot be empty")]
    EmptyProductType,
    #[error("captured_at cannot be empty")]
    EmptyCapturedAt,
    #[error("source cannot be empty")]
    EmptySource,
    #[error("uri cannot be empty")]
    EmptyUri,
    #[error("field requires a farm_id: {field_id}")]
    MissingFarmId { field_id: String },
    #[error("farm not found: {farm_id}")]
    FarmNotFound { farm_id: String },
    #[error("field not found: {field_id}")]
    FieldNotFound { field_id: String },
    #[error("season not found: {season_id}")]
    SeasonNotFound { season_id: String },
    #[error("scene not found: {scene_id}")]
    SceneNotFound { scene_id: String },
    #[error("season {season_id} does not belong to field {field_id} in org {org_id}")]
    SeasonFieldMismatch {
        season_id: String,
        field_id: String,
        org_id: String,
    },
    #[error("farm {farm_id} belongs to {farm_org_id}, not {field_org_id}")]
    TenantBoundary {
        farm_id: String,
        farm_org_id: String,
        field_org_id: String,
    },
    #[error("invalid field boundary: {reason}")]
    BoundaryInvalid {
        reason: FieldBoundaryValidationError,
    },
    #[error("invalid date {value}")]
    InvalidDate { value: String },
    #[error("season date range is invalid: {start}..{end}")]
    InvalidDateRange { start: String, end: String },
    #[error("season {season_id} overlaps {overlapping_season_id} for field {field_id}")]
    SeasonOverlap {
        field_id: String,
        season_id: String,
        overlapping_season_id: String,
    },
    #[error("layer {layer_id} metadata is invalid: {reason}")]
    LayerMetadataInvalid {
        layer_id: String,
        reason: SceneLayerMetadataError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FieldBoundaryValidationError {
    #[error("field boundary must declare a CRS")]
    MissingCrs,
    #[error("field boundary must contain a closed polygon ring")]
    TooFewCoordinates,
    #[error("field boundary contains invalid geographic coordinates")]
    InvalidCoordinate,
    #[error("field boundary polygon ring is not closed")]
    RingNotClosed,
    #[error("field boundary polygon self-intersects")]
    SelfIntersection,
    #[error("field boundary area is empty")]
    EmptyArea,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SceneLayerMetadataError {
    #[error("missing CRS")]
    MissingCrs,
    #[error("missing extent")]
    MissingExtent,
    #[error("invalid extent")]
    InvalidExtent,
    #[error("missing resolution")]
    MissingResolution,
    #[error("resolution must be positive")]
    NonPositiveResolution,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FarmFieldRegistry {
    farms: HashMap<String, FarmRecord>,
    fields: HashMap<String, FieldRecord>,
    #[serde(default)]
    seasons: HashMap<String, SeasonRecord>,
    #[serde(default)]
    crop_plans: HashMap<String, CropPlanRecord>,
    #[serde(default)]
    scenes: HashMap<String, SceneRecord>,
    #[serde(default)]
    scene_layers: HashMap<String, SceneLayerRecord>,
}

impl FarmFieldRegistry {
    pub fn insert_farm(&mut self, farm: FarmRecord) -> Result<FarmRecord, FarmFieldError> {
        let mut farm = normalize_farm_record(farm)?;
        farm.owner = farm.org_id.clone();
        self.farms.insert(farm.farm_id.clone(), farm.clone());
        Ok(farm)
    }

    pub fn insert_field(&mut self, field: FieldRecord) -> Result<FieldRecord, FarmFieldError> {
        let mut field = normalize_field_record(field)?;
        let farm_id = field
            .farm_id
            .clone()
            .ok_or_else(|| FarmFieldError::MissingFarmId {
                field_id: field.field_id.clone(),
            })?;
        let farm = self
            .farms
            .get(&farm_id)
            .ok_or_else(|| FarmFieldError::FarmNotFound {
                farm_id: farm_id.clone(),
            })?;
        if farm.org_id != field.org_id {
            return Err(FarmFieldError::TenantBoundary {
                farm_id,
                farm_org_id: farm.org_id.clone(),
                field_org_id: field.org_id,
            });
        }

        let validated = validate_field_boundary(&field.boundary)
            .map_err(|reason| FarmFieldError::BoundaryInvalid { reason })?;
        field.extent = validated.extent;
        field.area_ha = Some(validated.area_ha);
        field.owner = field.org_id.clone();
        self.fields.insert(field.field_id.clone(), field.clone());
        Ok(field)
    }

    pub fn farms_for_org(&self, org_id: &str) -> Vec<FarmRecord> {
        self.list_farms_for_org(org_id, FarmFieldListQuery::default())
            .items
    }

    pub fn list_farms_for_org(
        &self,
        org_id: &str,
        query: FarmFieldListQuery,
    ) -> FarmFieldListPage<FarmRecord> {
        let status = query.status_filter();
        let mut farms = self
            .farms
            .values()
            .filter(|farm| farm.org_id == org_id && farm.status == status)
            .cloned()
            .collect::<Vec<_>>();
        farms.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.farm_id.cmp(&right.farm_id))
        });
        paginate_farm_field_entities(farms, query)
    }

    pub fn fields_for_org(&self, org_id: &str) -> Vec<FieldRecord> {
        self.list_fields_for_org(org_id, FarmFieldListQuery::default())
            .items
    }

    pub fn list_fields_for_org(
        &self,
        org_id: &str,
        query: FarmFieldListQuery,
    ) -> FarmFieldListPage<FieldRecord> {
        let status = query.status_filter();
        let mut fields = self
            .fields
            .values()
            .filter(|field| field.org_id == org_id && field.status == status)
            .cloned()
            .collect::<Vec<_>>();
        fields.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.field_id.cmp(&right.field_id))
        });
        paginate_farm_field_entities(fields, query)
    }

    pub fn list_boundaries_for_org(
        &self,
        org_id: &str,
        query: FarmFieldListQuery,
    ) -> FarmFieldListPage<FieldBoundaryRecord> {
        let status = query.status_filter();
        let mut fields = self
            .fields
            .values()
            .filter(|field| field.org_id == org_id && field.status == status)
            .cloned()
            .collect::<Vec<_>>();
        fields.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.field_id.cmp(&right.field_id))
        });
        let boundaries = fields
            .into_iter()
            .map(|field| FieldBoundaryRecord {
                field_id: field.field_id,
                farm_id: field.farm_id,
                org_id: field.org_id,
                owner: field.owner,
                name: field.name,
                boundary: field.boundary,
                extent: field.extent,
                area_ha: field.area_ha,
                status: field.status,
                created_at: field.created_at,
                updated_at: field.updated_at,
            })
            .collect::<Vec<_>>();
        paginate_farm_field_entities(boundaries, query)
    }

    pub fn farm_for_org(&self, org_id: &str, farm_id: &str) -> Option<FarmRecord> {
        self.farms
            .get(farm_id)
            .filter(|farm| farm.org_id == org_id)
            .cloned()
    }

    pub fn field_for_org(&self, org_id: &str, field_id: &str) -> Option<FieldRecord> {
        self.fields
            .get(field_id)
            .filter(|field| field.org_id == org_id)
            .cloned()
    }

    pub fn field_by_id(&self, field_id: &str) -> Option<FieldRecord> {
        self.fields.get(field_id).cloned()
    }

    pub fn insert_season(&mut self, season: SeasonRecord) -> Result<SeasonRecord, FarmFieldError> {
        let season = normalize_season_record(season)?;
        self.fields
            .get(&season.field_id)
            .filter(|field| field.org_id == season.org_id)
            .ok_or_else(|| FarmFieldError::FieldNotFound {
                field_id: season.field_id.clone(),
            })?;

        let start = parse_farm_field_date(&season.start)?;
        let end = parse_farm_field_date(&season.end)?;
        if end < start {
            return Err(FarmFieldError::InvalidDateRange {
                start: season.start,
                end: season.end,
            });
        }

        for existing in self.seasons.values() {
            if existing.field_id != season.field_id || existing.org_id != season.org_id {
                continue;
            }
            let existing_start = parse_farm_field_date(&existing.start)?;
            let existing_end = parse_farm_field_date(&existing.end)?;
            if start <= existing_end && existing_start <= end {
                return Err(FarmFieldError::SeasonOverlap {
                    field_id: season.field_id,
                    season_id: season.season_id,
                    overlapping_season_id: existing.season_id.clone(),
                });
            }
        }

        self.seasons
            .insert(season.season_id.clone(), season.clone());
        Ok(season)
    }

    pub fn insert_crop_plan(
        &mut self,
        crop_plan: CropPlanRecord,
    ) -> Result<CropPlanRecord, FarmFieldError> {
        let mut crop_plan = normalize_crop_plan_record(crop_plan)?;
        if let Some(planting_date) = crop_plan.planting_date.as_ref() {
            parse_farm_field_date(planting_date)?;
        }
        let season = self.seasons.get(&crop_plan.season_id).ok_or_else(|| {
            FarmFieldError::SeasonNotFound {
                season_id: crop_plan.season_id.clone(),
            }
        })?;
        crop_plan.org_id = season.org_id.clone();

        self.crop_plans
            .insert(crop_plan.crop_plan_id.clone(), crop_plan.clone());
        Ok(crop_plan)
    }

    pub fn season_history_for_field(
        &self,
        org_id: &str,
        field_id: &str,
    ) -> Vec<FieldSeasonHistory> {
        let mut seasons = self
            .seasons
            .values()
            .filter(|season| season.org_id == org_id && season.field_id == field_id)
            .cloned()
            .collect::<Vec<_>>();
        seasons.sort_by(|left, right| {
            parse_farm_field_date(&left.start)
                .ok()
                .cmp(&parse_farm_field_date(&right.start).ok())
                .then(left.season_id.cmp(&right.season_id))
        });

        seasons
            .into_iter()
            .map(|season| {
                let mut crop_plans = self
                    .crop_plans
                    .values()
                    .filter(|crop_plan| crop_plan.season_id == season.season_id)
                    .cloned()
                    .collect::<Vec<_>>();
                crop_plans.sort_by(|left, right| left.crop_plan_id.cmp(&right.crop_plan_id));
                FieldSeasonHistory { season, crop_plans }
            })
            .collect()
    }

    pub fn active_season_for_field(
        &self,
        org_id: &str,
        field_id: &str,
        requested_date: &str,
    ) -> Result<ActiveSeasonResolution, FarmFieldError> {
        self.fields
            .get(field_id)
            .filter(|field| field.org_id == org_id)
            .ok_or_else(|| FarmFieldError::FieldNotFound {
                field_id: field_id.to_string(),
            })?;
        let requested = parse_farm_field_date(requested_date)?;

        let mut matches = self
            .seasons
            .values()
            .filter(|season| season.org_id == org_id && season.field_id == field_id)
            .filter_map(|season| {
                let start = parse_farm_field_date(&season.start).ok()?;
                let end = parse_farm_field_date(&season.end).ok()?;
                (start <= requested && requested <= end).then(|| season.clone())
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| {
            left.start
                .cmp(&right.start)
                .then(left.season_id.cmp(&right.season_id))
        });

        Ok(ActiveSeasonResolution {
            field_id: field_id.to_string(),
            requested_date: requested_date.to_string(),
            active_season: matches.into_iter().next(),
        })
    }

    pub fn insert_scene(&mut self, scene: SceneRecord) -> Result<SceneRecord, FarmFieldError> {
        let scene = normalize_scene_record(scene)?;
        self.fields
            .get(&scene.field_id)
            .filter(|field| field.org_id == scene.org_id)
            .ok_or_else(|| FarmFieldError::FieldNotFound {
                field_id: scene.field_id.clone(),
            })?;

        let season =
            self.seasons
                .get(&scene.season_id)
                .ok_or_else(|| FarmFieldError::SeasonNotFound {
                    season_id: scene.season_id.clone(),
                })?;
        if season.field_id != scene.field_id || season.org_id != scene.org_id {
            return Err(FarmFieldError::SeasonFieldMismatch {
                season_id: scene.season_id,
                field_id: scene.field_id,
                org_id: scene.org_id,
            });
        }

        self.scenes.insert(scene.scene_id.clone(), scene.clone());
        Ok(scene)
    }

    pub fn insert_scene_layer(
        &mut self,
        layer: SceneLayerRecord,
    ) -> Result<SceneLayerRecord, FarmFieldError> {
        let layer = normalize_scene_layer_record(layer)?;
        self.scenes
            .get(&layer.scene_id)
            .ok_or_else(|| FarmFieldError::SceneNotFound {
                scene_id: layer.scene_id.clone(),
            })?;
        validate_scene_layer_metadata(&layer)?;

        self.scene_layers
            .insert(layer.layer_id.clone(), layer.clone());
        Ok(layer)
    }

    pub fn scenes_for_field_season(
        &self,
        org_id: &str,
        field_id: &str,
        season_id: &str,
    ) -> Vec<FieldSceneCatalogEntry> {
        let mut scenes = self
            .scenes
            .values()
            .filter(|scene| {
                scene.org_id == org_id && scene.field_id == field_id && scene.season_id == season_id
            })
            .cloned()
            .collect::<Vec<_>>();
        scenes.sort_by(|left, right| {
            left.captured_at
                .cmp(&right.captured_at)
                .then(left.scene_id.cmp(&right.scene_id))
        });

        scenes
            .into_iter()
            .map(|scene| {
                let mut layers = self
                    .scene_layers
                    .values()
                    .filter(|layer| layer.scene_id == scene.scene_id)
                    .cloned()
                    .collect::<Vec<_>>();
                layers.sort_by(|left, right| {
                    left.product_type
                        .cmp(&right.product_type)
                        .then(left.layer_id.cmp(&right.layer_id))
                });
                FieldSceneCatalogEntry { scene, layers }
            })
            .collect()
    }

    pub fn scene_field_coverage(
        &self,
        org_id: &str,
        field_id: &str,
        scene_id: &str,
    ) -> Result<SceneFieldCoverage, FarmFieldError> {
        let field = self
            .fields
            .get(field_id)
            .filter(|field| field.org_id == org_id)
            .ok_or_else(|| FarmFieldError::FieldNotFound {
                field_id: field_id.to_string(),
            })?;
        let scene = self
            .scenes
            .get(scene_id)
            .filter(|scene| scene.org_id == org_id && scene.field_id == field_id)
            .ok_or_else(|| FarmFieldError::SceneNotFound {
                scene_id: scene_id.to_string(),
            })?;

        let layer_extents = self
            .scene_layers
            .values()
            .filter(|layer| layer.scene_id == scene.scene_id)
            .filter_map(|layer| layer.extent.as_ref())
            .collect::<Vec<_>>();
        if layer_extents.is_empty() {
            return Ok(SceneFieldCoverage {
                scene_id: scene.scene_id.clone(),
                field_id: field.field_id.clone(),
                coverage_fraction: 0.0,
                status: SceneFieldCoverageStatus::NoLayers,
            });
        }

        let covered_area = layer_extents
            .iter()
            .map(|extent| bounds_intersection_area(&field.extent, extent))
            .sum::<f64>();
        let field_area = bounds_area(&field.extent);
        let coverage_fraction = if field_area > 0.0 {
            (covered_area / field_area).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let status = if coverage_fraction == 0.0 {
            SceneFieldCoverageStatus::NoCoverage
        } else if coverage_fraction >= 0.999_999 {
            SceneFieldCoverageStatus::Full
        } else {
            SceneFieldCoverageStatus::Partial
        };

        Ok(SceneFieldCoverage {
            scene_id: scene.scene_id.clone(),
            field_id: field.field_id.clone(),
            coverage_fraction,
            status,
        })
    }
}

fn paginate_farm_field_entities<T>(
    items: Vec<T>,
    query: FarmFieldListQuery,
) -> FarmFieldListPage<T> {
    let page = query.normalized_page();
    let page_size = query.normalized_page_size();
    let total_count = items.len();
    let start = page.saturating_sub(1).saturating_mul(page_size);
    let items = if start >= total_count {
        Vec::new()
    } else {
        items.into_iter().skip(start).take(page_size).collect()
    };

    FarmFieldListPage {
        items,
        total_count,
        page,
        page_size,
    }
}

fn normalize_farm_record(mut farm: FarmRecord) -> Result<FarmRecord, FarmFieldError> {
    farm.farm_id = normalize_farm_field_text(farm.farm_id).ok_or(FarmFieldError::EmptyFarmId)?;
    farm.org_id = normalize_farm_field_text(farm.org_id).ok_or(FarmFieldError::EmptyOrgId)?;
    farm.name = normalize_farm_field_text(farm.name).ok_or(FarmFieldError::EmptyName)?;
    farm.created_at = normalize_farm_field_text(farm.created_at).unwrap_or_default();
    farm.updated_at =
        normalize_farm_field_text(farm.updated_at).unwrap_or_else(|| farm.created_at.clone());
    Ok(farm)
}

fn normalize_field_record(mut field: FieldRecord) -> Result<FieldRecord, FarmFieldError> {
    field.field_id =
        normalize_farm_field_text(field.field_id).ok_or(FarmFieldError::EmptyFieldId)?;
    field.org_id = normalize_farm_field_text(field.org_id).ok_or(FarmFieldError::EmptyOrgId)?;
    field.name = normalize_farm_field_text(field.name).ok_or(FarmFieldError::EmptyName)?;
    field.farm_id = field.farm_id.and_then(normalize_farm_field_text);
    field.created_at = normalize_farm_field_text(field.created_at).unwrap_or_default();
    field.updated_at =
        normalize_farm_field_text(field.updated_at).unwrap_or_else(|| field.created_at.clone());
    Ok(field)
}

fn normalize_season_record(mut season: SeasonRecord) -> Result<SeasonRecord, FarmFieldError> {
    season.season_id =
        normalize_farm_field_text(season.season_id).ok_or(FarmFieldError::EmptySeasonId)?;
    season.field_id =
        normalize_farm_field_text(season.field_id).ok_or(FarmFieldError::EmptyFieldId)?;
    season.org_id = normalize_farm_field_text(season.org_id).ok_or(FarmFieldError::EmptyOrgId)?;
    season.start =
        normalize_farm_field_text(season.start).ok_or_else(|| FarmFieldError::InvalidDate {
            value: String::new(),
        })?;
    season.end =
        normalize_farm_field_text(season.end).ok_or_else(|| FarmFieldError::InvalidDate {
            value: String::new(),
        })?;
    season.label = normalize_farm_field_text(season.label).ok_or(FarmFieldError::EmptyName)?;
    Ok(season)
}

fn normalize_crop_plan_record(
    mut crop_plan: CropPlanRecord,
) -> Result<CropPlanRecord, FarmFieldError> {
    crop_plan.crop_plan_id =
        normalize_farm_field_text(crop_plan.crop_plan_id).ok_or(FarmFieldError::EmptyCropPlanId)?;
    crop_plan.season_id =
        normalize_farm_field_text(crop_plan.season_id).ok_or(FarmFieldError::EmptySeasonId)?;
    crop_plan.org_id = normalize_farm_field_text(crop_plan.org_id).unwrap_or_default();
    crop_plan.crop = normalize_farm_field_text(crop_plan.crop).ok_or(FarmFieldError::EmptyCrop)?;
    crop_plan.planting_date = crop_plan.planting_date.and_then(normalize_farm_field_text);
    Ok(crop_plan)
}

fn normalize_scene_record(mut scene: SceneRecord) -> Result<SceneRecord, FarmFieldError> {
    scene.scene_id =
        normalize_farm_field_text(scene.scene_id).ok_or(FarmFieldError::EmptySceneId)?;
    scene.field_id =
        normalize_farm_field_text(scene.field_id).ok_or(FarmFieldError::EmptyFieldId)?;
    scene.season_id =
        normalize_farm_field_text(scene.season_id).ok_or(FarmFieldError::EmptySeasonId)?;
    scene.org_id = normalize_farm_field_text(scene.org_id).ok_or(FarmFieldError::EmptyOrgId)?;
    scene.captured_at =
        normalize_farm_field_text(scene.captured_at).ok_or(FarmFieldError::EmptyCapturedAt)?;
    scene.source = normalize_farm_field_text(scene.source).ok_or(FarmFieldError::EmptySource)?;
    Ok(scene)
}

fn normalize_scene_layer_record(
    mut layer: SceneLayerRecord,
) -> Result<SceneLayerRecord, FarmFieldError> {
    layer.layer_id =
        normalize_farm_field_text(layer.layer_id).ok_or(FarmFieldError::EmptyLayerId)?;
    layer.scene_id =
        normalize_farm_field_text(layer.scene_id).ok_or(FarmFieldError::EmptySceneId)?;
    layer.product_type =
        normalize_farm_field_text(layer.product_type).ok_or(FarmFieldError::EmptyProductType)?;
    layer.crs = layer.crs.trim().to_string();
    layer.uri = normalize_farm_field_text(layer.uri).ok_or(FarmFieldError::EmptyUri)?;
    Ok(layer)
}

fn validate_scene_layer_metadata(layer: &SceneLayerRecord) -> Result<(), FarmFieldError> {
    if layer.crs.is_empty() {
        return Err(FarmFieldError::LayerMetadataInvalid {
            layer_id: layer.layer_id.clone(),
            reason: SceneLayerMetadataError::MissingCrs,
        });
    }

    let extent = layer
        .extent
        .as_ref()
        .ok_or_else(|| FarmFieldError::LayerMetadataInvalid {
            layer_id: layer.layer_id.clone(),
            reason: SceneLayerMetadataError::MissingExtent,
        })?;
    if !extent.min_lon.is_finite()
        || !extent.min_lat.is_finite()
        || !extent.max_lon.is_finite()
        || !extent.max_lat.is_finite()
        || extent.min_lon >= extent.max_lon
        || extent.min_lat >= extent.max_lat
    {
        return Err(FarmFieldError::LayerMetadataInvalid {
            layer_id: layer.layer_id.clone(),
            reason: SceneLayerMetadataError::InvalidExtent,
        });
    }

    let resolution = layer
        .resolution
        .ok_or_else(|| FarmFieldError::LayerMetadataInvalid {
            layer_id: layer.layer_id.clone(),
            reason: SceneLayerMetadataError::MissingResolution,
        })?;
    if !resolution.x.is_finite()
        || !resolution.y.is_finite()
        || resolution.x <= 0.0
        || resolution.y <= 0.0
    {
        return Err(FarmFieldError::LayerMetadataInvalid {
            layer_id: layer.layer_id.clone(),
            reason: SceneLayerMetadataError::NonPositiveResolution,
        });
    }

    Ok(())
}

fn normalize_farm_field_text(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn parse_farm_field_date(value: &str) -> Result<NaiveDate, FarmFieldError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| FarmFieldError::InvalidDate {
        value: value.to_string(),
    })
}

pub fn bounds_coverage_fraction(boundary: &GeoBounds, covered: &GeoBounds) -> f64 {
    let boundary_area = bounds_area(boundary);
    if boundary_area <= 0.0 {
        return 0.0;
    }
    (bounds_intersection_area(boundary, covered) / boundary_area).clamp(0.0, 1.0)
}

fn bounds_area(bounds: &GeoBounds) -> f64 {
    let width = (bounds.max_lon - bounds.min_lon).max(0.0);
    let height = (bounds.max_lat - bounds.min_lat).max(0.0);
    width * height
}

fn bounds_intersection_area(left: &GeoBounds, right: &GeoBounds) -> f64 {
    let min_lon = left.min_lon.max(right.min_lon);
    let max_lon = left.max_lon.min(right.max_lon);
    let min_lat = left.min_lat.max(right.min_lat);
    let max_lat = left.max_lat.min(right.max_lat);
    if max_lon <= min_lon || max_lat <= min_lat {
        return 0.0;
    }
    (max_lon - min_lon) * (max_lat - min_lat)
}

pub fn validate_field_boundary(
    boundary: &FieldBoundary,
) -> Result<ValidatedFieldBoundary, FieldBoundaryValidationError> {
    let crs = boundary
        .crs
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(FieldBoundaryValidationError::MissingCrs)?;
    let coordinates = &boundary.coordinates;
    if coordinates.len() < 4 {
        return Err(FieldBoundaryValidationError::TooFewCoordinates);
    }
    if coordinates.iter().any(|point| {
        !point.longitude.is_finite()
            || !point.latitude.is_finite()
            || point.longitude < -180.0
            || point.longitude > 180.0
            || point.latitude < -90.0
            || point.latitude > 90.0
    }) {
        return Err(FieldBoundaryValidationError::InvalidCoordinate);
    }
    if !points_approximately_equal(
        coordinates.first().expect("coordinates length checked"),
        coordinates.last().expect("coordinates length checked"),
    ) {
        return Err(FieldBoundaryValidationError::RingNotClosed);
    }
    if ring_self_intersects(coordinates) {
        return Err(FieldBoundaryValidationError::SelfIntersection);
    }

    let extent =
        bounds_from_points(coordinates).ok_or(FieldBoundaryValidationError::InvalidCoordinate)?;
    let area_ha = polygon_area_hectares(coordinates);
    if area_ha <= f64::EPSILON {
        return Err(FieldBoundaryValidationError::EmptyArea);
    }

    Ok(ValidatedFieldBoundary {
        boundary: FieldBoundary {
            coordinates: coordinates.clone(),
            crs: Some(crs.to_string()),
        },
        extent,
        area_ha,
    })
}

fn points_approximately_equal(left: &GeoPoint, right: &GeoPoint) -> bool {
    const EPSILON: f64 = 1e-9;
    (left.longitude - right.longitude).abs() <= EPSILON
        && (left.latitude - right.latitude).abs() <= EPSILON
}

fn ring_self_intersects(points: &[GeoPoint]) -> bool {
    let segment_count = points.len().saturating_sub(1);
    for left in 0..segment_count {
        for right in (left + 1)..segment_count {
            if segments_share_ring_vertex(left, right, segment_count) {
                continue;
            }
            if segments_intersect(
                &points[left],
                &points[left + 1],
                &points[right],
                &points[right + 1],
            ) {
                return true;
            }
        }
    }
    false
}

fn segments_share_ring_vertex(left: usize, right: usize, segment_count: usize) -> bool {
    left == right || left + 1 == right || (left == 0 && right + 1 == segment_count)
}

fn segments_intersect(a: &GeoPoint, b: &GeoPoint, c: &GeoPoint, d: &GeoPoint) -> bool {
    let o1 = orientation(a, b, c);
    let o2 = orientation(a, b, d);
    let o3 = orientation(c, d, a);
    let o4 = orientation(c, d, b);

    if orientation_sign(o1) != orientation_sign(o2) && orientation_sign(o3) != orientation_sign(o4)
    {
        return true;
    }

    (orientation_is_colinear(o1) && point_on_segment(a, c, b))
        || (orientation_is_colinear(o2) && point_on_segment(a, d, b))
        || (orientation_is_colinear(o3) && point_on_segment(c, a, d))
        || (orientation_is_colinear(o4) && point_on_segment(c, b, d))
}

fn orientation(a: &GeoPoint, b: &GeoPoint, c: &GeoPoint) -> f64 {
    (b.longitude - a.longitude) * (c.latitude - a.latitude)
        - (b.latitude - a.latitude) * (c.longitude - a.longitude)
}

fn orientation_sign(value: f64) -> i8 {
    if orientation_is_colinear(value) {
        0
    } else if value > 0.0 {
        1
    } else {
        -1
    }
}

fn orientation_is_colinear(value: f64) -> bool {
    value.abs() <= 1e-12
}

fn point_on_segment(start: &GeoPoint, point: &GeoPoint, end: &GeoPoint) -> bool {
    point.longitude >= start.longitude.min(end.longitude) - 1e-12
        && point.longitude <= start.longitude.max(end.longitude) + 1e-12
        && point.latitude >= start.latitude.min(end.latitude) - 1e-12
        && point.latitude <= start.latitude.max(end.latitude) + 1e-12
}

fn polygon_area_hectares(points: &[GeoPoint]) -> f64 {
    let mean_latitude =
        points.iter().map(|point| point.latitude).sum::<f64>() / points.len() as f64;
    let meters_per_degree_lat = 111_320.0;
    let meters_per_degree_lon = meters_per_degree_lat * mean_latitude.to_radians().cos();
    let area_m2 = points
        .windows(2)
        .map(|window| {
            let x1 = window[0].longitude * meters_per_degree_lon;
            let y1 = window[0].latitude * meters_per_degree_lat;
            let x2 = window[1].longitude * meters_per_degree_lon;
            let y2 = window[1].latitude * meters_per_degree_lat;
            x1 * y2 - x2 * y1
        })
        .sum::<f64>()
        .abs()
        * 0.5;
    area_m2 / 10_000.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnnotationGeometry {
    Point { coordinate: GeoPoint },
    Polygon { coordinates: Vec<GeoPoint> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditedAnnotationRecord {
    pub annotation_id: String,
    pub field_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    pub org_id: String,
    pub author_user_id: String,
    pub geometry: AnnotationGeometry,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retracted_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationChangeType {
    Created,
    Edited,
    Retracted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationChangeRecord {
    pub annotation_id: String,
    pub actor_user_id: String,
    pub before: Option<AuditedAnnotationRecord>,
    pub after: Option<AuditedAnnotationRecord>,
    pub at: String,
    pub change_type: AnnotationChangeType,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnnotationPersistenceError {
    #[error("annotation_id cannot be empty")]
    EmptyAnnotationId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("org_id cannot be empty")]
    EmptyOrgId,
    #[error("author_user_id cannot be empty")]
    EmptyAuthorUserId,
    #[error("actor_user_id cannot be empty")]
    EmptyActorUserId,
    #[error("timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("annotation already exists: {annotation_id}")]
    AnnotationAlreadyExists { annotation_id: String },
    #[error("annotation not found: {annotation_id}")]
    AnnotationNotFound { annotation_id: String },
    #[error("annotation history hard delete is rejected: {annotation_id}")]
    HistoryDeleteRejected { annotation_id: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnnotationAuditRegistry {
    annotations: HashMap<String, AuditedAnnotationRecord>,
    history: Vec<AnnotationChangeRecord>,
}

impl AnnotationAuditRegistry {
    pub fn create_annotation(
        &mut self,
        annotation: AuditedAnnotationRecord,
    ) -> Result<AuditedAnnotationRecord, AnnotationPersistenceError> {
        let annotation = normalize_audited_annotation(annotation)?;
        if self.annotations.contains_key(&annotation.annotation_id) {
            return Err(AnnotationPersistenceError::AnnotationAlreadyExists {
                annotation_id: annotation.annotation_id,
            });
        }

        self.history.push(AnnotationChangeRecord {
            annotation_id: annotation.annotation_id.clone(),
            actor_user_id: annotation.author_user_id.clone(),
            before: None,
            after: Some(annotation.clone()),
            at: annotation.created_at.clone(),
            change_type: AnnotationChangeType::Created,
        });
        self.annotations
            .insert(annotation.annotation_id.clone(), annotation.clone());
        Ok(annotation)
    }

    pub fn edit_annotation_geometry(
        &mut self,
        org_id: &str,
        annotation_id: &str,
        actor_user_id: &str,
        at: &str,
        geometry: AnnotationGeometry,
    ) -> Result<AuditedAnnotationRecord, AnnotationPersistenceError> {
        let org_id = normalize_annotation_arg(org_id, AnnotationPersistenceError::EmptyOrgId)?;
        let annotation_id =
            normalize_annotation_arg(annotation_id, AnnotationPersistenceError::EmptyAnnotationId)?;
        let actor_user_id =
            normalize_annotation_arg(actor_user_id, AnnotationPersistenceError::EmptyActorUserId)?;
        let at = normalize_annotation_arg(at, AnnotationPersistenceError::EmptyTimestamp)?;
        let before = self.annotation_for_org(&org_id, &annotation_id)?;
        let mut after = before.clone();
        after.geometry = geometry;

        self.annotations
            .insert(annotation_id.clone(), after.clone());
        self.history.push(AnnotationChangeRecord {
            annotation_id,
            actor_user_id,
            before: Some(before),
            after: Some(after.clone()),
            at,
            change_type: AnnotationChangeType::Edited,
        });
        Ok(after)
    }

    pub fn retract_annotation(
        &mut self,
        org_id: &str,
        annotation_id: &str,
        actor_user_id: &str,
        at: &str,
    ) -> Result<AuditedAnnotationRecord, AnnotationPersistenceError> {
        let org_id = normalize_annotation_arg(org_id, AnnotationPersistenceError::EmptyOrgId)?;
        let annotation_id =
            normalize_annotation_arg(annotation_id, AnnotationPersistenceError::EmptyAnnotationId)?;
        let actor_user_id =
            normalize_annotation_arg(actor_user_id, AnnotationPersistenceError::EmptyActorUserId)?;
        let at = normalize_annotation_arg(at, AnnotationPersistenceError::EmptyTimestamp)?;
        let before = self.annotation_for_org(&org_id, &annotation_id)?;
        let mut after = before.clone();
        after.retracted_at = Some(at.clone());

        self.annotations
            .insert(annotation_id.clone(), after.clone());
        self.history.push(AnnotationChangeRecord {
            annotation_id,
            actor_user_id,
            before: Some(before),
            after: Some(after.clone()),
            at,
            change_type: AnnotationChangeType::Retracted,
        });
        Ok(after)
    }

    pub fn annotations_for_org(&self, org_id: &str) -> Vec<AuditedAnnotationRecord> {
        let mut annotations = self
            .annotations
            .values()
            .filter(|annotation| annotation.org_id == org_id)
            .cloned()
            .collect::<Vec<_>>();
        annotations.sort_by(|left, right| left.annotation_id.cmp(&right.annotation_id));
        annotations
    }

    pub fn annotation_history(
        &self,
        org_id: &str,
        annotation_id: &str,
    ) -> Vec<AnnotationChangeRecord> {
        self.history
            .iter()
            .filter(|change| change.annotation_id == annotation_id)
            .filter(|change| {
                change
                    .after
                    .as_ref()
                    .or(change.before.as_ref())
                    .is_some_and(|annotation| annotation.org_id == org_id)
            })
            .cloned()
            .collect::<Vec<_>>()
    }

    pub fn delete_annotation_history(
        &self,
        org_id: &str,
        annotation_id: &str,
    ) -> Result<(), AnnotationPersistenceError> {
        let org_id = normalize_annotation_arg(org_id, AnnotationPersistenceError::EmptyOrgId)?;
        let annotation_id =
            normalize_annotation_arg(annotation_id, AnnotationPersistenceError::EmptyAnnotationId)?;
        self.annotation_for_org(&org_id, &annotation_id)?;
        Err(AnnotationPersistenceError::HistoryDeleteRejected { annotation_id })
    }

    fn annotation_for_org(
        &self,
        org_id: &str,
        annotation_id: &str,
    ) -> Result<AuditedAnnotationRecord, AnnotationPersistenceError> {
        self.annotations
            .get(annotation_id)
            .filter(|annotation| annotation.org_id == org_id)
            .cloned()
            .ok_or_else(|| AnnotationPersistenceError::AnnotationNotFound {
                annotation_id: annotation_id.to_string(),
            })
    }
}

fn normalize_audited_annotation(
    mut annotation: AuditedAnnotationRecord,
) -> Result<AuditedAnnotationRecord, AnnotationPersistenceError> {
    annotation.annotation_id = normalize_farm_field_text(annotation.annotation_id)
        .ok_or(AnnotationPersistenceError::EmptyAnnotationId)?;
    annotation.field_id = normalize_farm_field_text(annotation.field_id)
        .ok_or(AnnotationPersistenceError::EmptyFieldId)?;
    annotation.scene_id = annotation.scene_id.and_then(normalize_farm_field_text);
    annotation.org_id = normalize_farm_field_text(annotation.org_id)
        .ok_or(AnnotationPersistenceError::EmptyOrgId)?;
    annotation.author_user_id = normalize_farm_field_text(annotation.author_user_id)
        .ok_or(AnnotationPersistenceError::EmptyAuthorUserId)?;
    annotation.created_at = normalize_farm_field_text(annotation.created_at)
        .ok_or(AnnotationPersistenceError::EmptyTimestamp)?;
    annotation.retracted_at = annotation.retracted_at.and_then(normalize_farm_field_text);
    Ok(annotation)
}

fn normalize_annotation_arg(
    value: &str,
    error: AnnotationPersistenceError,
) -> Result<String, AnnotationPersistenceError> {
    normalize_farm_field_text(value.to_string()).ok_or(error)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationRecord {
    pub annotation_id: String,
    pub scene_id: String,
    pub field_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_id: Option<String>,
    pub label: String,
    pub note: Option<String>,
    pub severity: Option<String>,
    pub geometry: AnnotationGeometry,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationStatus {
    Open,
    Reviewed,
    Completed,
    Dismissed,
    Closed,
}

impl Default for RecommendationStatus {
    fn default() -> Self {
        Self::Open
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl Default for RecommendationPriority {
    fn default() -> Self {
        Self::Medium
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecommendationRecord {
    pub recommendation_id: String,
    pub scene_id: String,
    pub field_id: Option<String>,
    #[serde(default = "default_record_owner")]
    pub org_id: String,
    #[serde(default = "default_record_owner")]
    pub author_user_id: String,
    pub title: String,
    pub note: Option<String>,
    pub category: Option<String>,
    #[serde(default)]
    pub action_category: String,
    pub priority: RecommendationPriority,
    pub status: RecommendationStatus,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub annotation_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationStatusChangeType {
    Created,
    StatusChanged,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecommendationStatusChangeRecord {
    pub recommendation_id: String,
    pub actor_user_id: String,
    pub before: Option<RecommendationStatus>,
    pub after: RecommendationStatus,
    pub at: String,
    pub change_type: RecommendationStatusChangeType,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecommendationPersistenceError {
    #[error("recommendation_id cannot be empty")]
    EmptyRecommendationId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("org_id cannot be empty")]
    EmptyOrgId,
    #[error("author_user_id cannot be empty")]
    EmptyAuthorUserId,
    #[error("actor_user_id cannot be empty")]
    EmptyActorUserId,
    #[error("action_category cannot be empty")]
    EmptyActionCategory,
    #[error("timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("recommendation must cite at least one evidence ref: {recommendation_id}")]
    EvidenceRequired { recommendation_id: String },
    #[error("recommendation must start open: {recommendation_id}")]
    InvalidInitialStatus { recommendation_id: String },
    #[error("recommendation already exists: {recommendation_id}")]
    RecommendationAlreadyExists { recommendation_id: String },
    #[error("recommendation not found: {recommendation_id}")]
    RecommendationNotFound { recommendation_id: String },
    #[error("invalid recommendation status transition: {from:?} -> {to:?}")]
    InvalidStatusTransition {
        from: RecommendationStatus,
        to: RecommendationStatus,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecommendationLifecycleRegistry {
    recommendations: HashMap<String, RecommendationRecord>,
    history: Vec<RecommendationStatusChangeRecord>,
}

impl RecommendationLifecycleRegistry {
    pub fn create_recommendation(
        &mut self,
        recommendation: RecommendationRecord,
    ) -> Result<RecommendationRecord, RecommendationPersistenceError> {
        let recommendation = normalize_recommendation_record(recommendation)?;
        if self
            .recommendations
            .contains_key(&recommendation.recommendation_id)
        {
            return Err(
                RecommendationPersistenceError::RecommendationAlreadyExists {
                    recommendation_id: recommendation.recommendation_id,
                },
            );
        }

        self.history.push(RecommendationStatusChangeRecord {
            recommendation_id: recommendation.recommendation_id.clone(),
            actor_user_id: recommendation.author_user_id.clone(),
            before: None,
            after: recommendation.status,
            at: recommendation.created_at.clone(),
            change_type: RecommendationStatusChangeType::Created,
        });
        self.recommendations.insert(
            recommendation.recommendation_id.clone(),
            recommendation.clone(),
        );
        Ok(recommendation)
    }

    pub fn transition_recommendation_status(
        &mut self,
        org_id: &str,
        recommendation_id: &str,
        actor_user_id: &str,
        at: &str,
        status: RecommendationStatus,
    ) -> Result<RecommendationRecord, RecommendationPersistenceError> {
        let org_id =
            normalize_recommendation_arg(org_id, RecommendationPersistenceError::EmptyOrgId)?;
        let recommendation_id = normalize_recommendation_arg(
            recommendation_id,
            RecommendationPersistenceError::EmptyRecommendationId,
        )?;
        let actor_user_id = normalize_recommendation_arg(
            actor_user_id,
            RecommendationPersistenceError::EmptyActorUserId,
        )?;
        let at = normalize_recommendation_arg(at, RecommendationPersistenceError::EmptyTimestamp)?;
        let before = self.recommendation_for_org(&org_id, &recommendation_id)?;
        if !is_valid_recommendation_status_transition(before.status, status) {
            return Err(RecommendationPersistenceError::InvalidStatusTransition {
                from: before.status,
                to: status,
            });
        }

        let mut after = before.clone();
        after.status = status;
        after.updated_at = at.clone();
        self.recommendations
            .insert(recommendation_id.clone(), after.clone());
        self.history.push(RecommendationStatusChangeRecord {
            recommendation_id,
            actor_user_id,
            before: Some(before.status),
            after: status,
            at,
            change_type: RecommendationStatusChangeType::StatusChanged,
        });
        Ok(after)
    }

    pub fn recommendations_for_org(&self, org_id: &str) -> Vec<RecommendationRecord> {
        let Some(org_id) = normalize_farm_field_text(org_id.to_string()) else {
            return Vec::new();
        };
        let mut recommendations = self
            .recommendations
            .values()
            .filter(|recommendation| recommendation.org_id == org_id)
            .cloned()
            .collect::<Vec<_>>();
        recommendations.sort_by(|left, right| left.recommendation_id.cmp(&right.recommendation_id));
        recommendations
    }

    pub fn recommendation_history(
        &self,
        org_id: &str,
        recommendation_id: &str,
    ) -> Vec<RecommendationStatusChangeRecord> {
        let Some(org_id) = normalize_farm_field_text(org_id.to_string()) else {
            return Vec::new();
        };
        let Some(recommendation_id) = normalize_farm_field_text(recommendation_id.to_string())
        else {
            return Vec::new();
        };

        self.history
            .iter()
            .filter(|change| change.recommendation_id == recommendation_id)
            .filter(|change| {
                self.recommendations
                    .get(&change.recommendation_id)
                    .is_some_and(|recommendation| recommendation.org_id == org_id)
            })
            .cloned()
            .collect::<Vec<_>>()
    }

    fn recommendation_for_org(
        &self,
        org_id: &str,
        recommendation_id: &str,
    ) -> Result<RecommendationRecord, RecommendationPersistenceError> {
        self.recommendations
            .get(recommendation_id)
            .filter(|recommendation| recommendation.org_id == org_id)
            .cloned()
            .ok_or_else(|| RecommendationPersistenceError::RecommendationNotFound {
                recommendation_id: recommendation_id.to_string(),
            })
    }
}

fn normalize_recommendation_record(
    mut recommendation: RecommendationRecord,
) -> Result<RecommendationRecord, RecommendationPersistenceError> {
    recommendation.recommendation_id = normalize_farm_field_text(recommendation.recommendation_id)
        .ok_or(RecommendationPersistenceError::EmptyRecommendationId)?;
    recommendation.field_id = Some(
        recommendation
            .field_id
            .and_then(normalize_farm_field_text)
            .ok_or(RecommendationPersistenceError::EmptyFieldId)?,
    );
    recommendation.org_id = normalize_farm_field_text(recommendation.org_id)
        .ok_or(RecommendationPersistenceError::EmptyOrgId)?;
    recommendation.author_user_id = normalize_farm_field_text(recommendation.author_user_id)
        .ok_or(RecommendationPersistenceError::EmptyAuthorUserId)?;
    recommendation.action_category = normalize_farm_field_text(recommendation.action_category)
        .ok_or(RecommendationPersistenceError::EmptyActionCategory)?;
    recommendation.category = recommendation
        .category
        .and_then(normalize_farm_field_text)
        .or_else(|| Some(recommendation.action_category.clone()));
    recommendation.evidence_refs = normalize_recommendation_refs(recommendation.evidence_refs);
    if recommendation.evidence_refs.is_empty() {
        return Err(RecommendationPersistenceError::EvidenceRequired {
            recommendation_id: recommendation.recommendation_id,
        });
    }
    if recommendation.status != RecommendationStatus::Open {
        return Err(RecommendationPersistenceError::InvalidInitialStatus {
            recommendation_id: recommendation.recommendation_id,
        });
    }
    recommendation.created_at = normalize_farm_field_text(recommendation.created_at)
        .ok_or(RecommendationPersistenceError::EmptyTimestamp)?;
    recommendation.updated_at = normalize_farm_field_text(recommendation.updated_at)
        .ok_or(RecommendationPersistenceError::EmptyTimestamp)?;
    recommendation.annotation_ids = normalize_recommendation_refs(recommendation.annotation_ids);
    Ok(recommendation)
}

fn normalize_recommendation_refs(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter_map(normalize_farm_field_text)
        .collect::<Vec<_>>()
}

fn normalize_recommendation_arg(
    value: &str,
    error: RecommendationPersistenceError,
) -> Result<String, RecommendationPersistenceError> {
    normalize_farm_field_text(value.to_string()).ok_or(error)
}

fn is_valid_recommendation_status_transition(
    from: RecommendationStatus,
    to: RecommendationStatus,
) -> bool {
    matches!(
        (from, to),
        (RecommendationStatus::Open, RecommendationStatus::Reviewed)
            | (
                RecommendationStatus::Reviewed,
                RecommendationStatus::Completed | RecommendationStatus::Dismissed
            )
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkOrderStatus {
    Created,
    Assigned,
    InProgress,
    Done,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkOrderRecord {
    pub work_order_id: String,
    pub field_id: String,
    pub org_id: String,
    pub source_rec_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee_user_id: Option<String>,
    pub status: WorkOrderStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkOrderCreateRequest {
    pub work_order_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_recommendation: Option<RecommendationRecord>,
    pub actor_user_id: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkOrderQueueQuery {
    pub org_id: String,
    pub assignee_user_id: String,
    #[serde(default)]
    pub statuses: Vec<WorkOrderStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkOrderChangeType {
    Created,
    StatusChanged,
    Reassigned,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkOrderChangeRecord {
    pub work_order_id: String,
    pub actor_user_id: String,
    pub before: Option<WorkOrderStatus>,
    pub after: WorkOrderStatus,
    pub at: String,
    pub change_type: WorkOrderChangeType,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkOrderPersistenceError {
    #[error("work_order_id cannot be empty")]
    EmptyWorkOrderId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("org_id cannot be empty")]
    EmptyOrgId,
    #[error("source_rec_id cannot be empty")]
    EmptySourceRecommendationId,
    #[error("actor_user_id cannot be empty")]
    EmptyActorUserId,
    #[error("assignee_user_id cannot be empty")]
    EmptyAssigneeUserId,
    #[error("timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("work order requires a source recommendation: {work_order_id}")]
    MissingSourceRecommendation { work_order_id: String },
    #[error("source recommendation must be open: {recommendation_id}")]
    SourceRecommendationNotOpen { recommendation_id: String },
    #[error("work order already exists: {work_order_id}")]
    WorkOrderAlreadyExists { work_order_id: String },
    #[error("work order not found: {work_order_id}")]
    WorkOrderNotFound { work_order_id: String },
    #[error("invalid work order status transition: {from:?} -> {to:?}")]
    InvalidStatusTransition {
        from: WorkOrderStatus,
        to: WorkOrderStatus,
    },
    #[error(
        "assignee {assignee_user_id} belongs to org {actual_org_id}, expected {expected_org_id}"
    )]
    AssigneeOrgMismatch {
        assignee_user_id: String,
        expected_org_id: String,
        actual_org_id: String,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkOrderRegistry {
    work_orders: HashMap<String, WorkOrderRecord>,
    history: Vec<WorkOrderChangeRecord>,
}

impl WorkOrderRegistry {
    pub fn create_work_order_from_recommendation(
        &mut self,
        request: WorkOrderCreateRequest,
    ) -> Result<WorkOrderRecord, WorkOrderPersistenceError> {
        let work_order_id = normalize_work_order_arg(
            &request.work_order_id,
            WorkOrderPersistenceError::EmptyWorkOrderId,
        )?;
        let actor_user_id = normalize_work_order_arg(
            &request.actor_user_id,
            WorkOrderPersistenceError::EmptyActorUserId,
        )?;
        let created_at = normalize_work_order_arg(
            &request.created_at,
            WorkOrderPersistenceError::EmptyTimestamp,
        )?;
        let recommendation = request.source_recommendation.ok_or_else(|| {
            WorkOrderPersistenceError::MissingSourceRecommendation {
                work_order_id: work_order_id.clone(),
            }
        })?;
        if recommendation.status != RecommendationStatus::Open {
            return Err(WorkOrderPersistenceError::SourceRecommendationNotOpen {
                recommendation_id: recommendation.recommendation_id,
            });
        }
        if self.work_orders.contains_key(&work_order_id) {
            return Err(WorkOrderPersistenceError::WorkOrderAlreadyExists { work_order_id });
        }

        let field_id = recommendation
            .field_id
            .and_then(normalize_farm_field_text)
            .ok_or(WorkOrderPersistenceError::EmptyFieldId)?;
        let org_id = normalize_farm_field_text(recommendation.org_id)
            .ok_or(WorkOrderPersistenceError::EmptyOrgId)?;
        let source_rec_id = normalize_farm_field_text(recommendation.recommendation_id)
            .ok_or(WorkOrderPersistenceError::EmptySourceRecommendationId)?;
        let assignee_user_id = request
            .assignee_user_id
            .map(|assignee| {
                normalize_farm_field_text(assignee)
                    .ok_or(WorkOrderPersistenceError::EmptyAssigneeUserId)
            })
            .transpose()?;
        let due = request.due.and_then(normalize_farm_field_text);
        let work_order = WorkOrderRecord {
            work_order_id: work_order_id.clone(),
            field_id,
            org_id,
            source_rec_id,
            assignee_user_id,
            status: WorkOrderStatus::Created,
            due,
            created_at: created_at.clone(),
            updated_at: created_at.clone(),
        };

        self.history.push(WorkOrderChangeRecord {
            work_order_id: work_order_id.clone(),
            actor_user_id,
            before: None,
            after: WorkOrderStatus::Created,
            at: created_at,
            change_type: WorkOrderChangeType::Created,
        });
        self.work_orders.insert(work_order_id, work_order.clone());
        Ok(work_order)
    }

    pub fn assign_work_order(
        &mut self,
        org_id: &str,
        work_order_id: &str,
        actor_user_id: &str,
        assignee_user_id: &str,
        at: &str,
    ) -> Result<WorkOrderRecord, WorkOrderPersistenceError> {
        let assignee_user_id = normalize_work_order_arg(
            assignee_user_id,
            WorkOrderPersistenceError::EmptyAssigneeUserId,
        )?;
        let mut work_order = self.transition_work_order_status(
            org_id,
            work_order_id,
            actor_user_id,
            at,
            WorkOrderStatus::Assigned,
        )?;
        work_order.assignee_user_id = Some(assignee_user_id);
        let key = work_order.work_order_id.clone();
        self.work_orders.insert(key, work_order.clone());
        Ok(work_order)
    }

    pub fn reassign_work_order(
        &mut self,
        org_id: &str,
        work_order_id: &str,
        actor_user_id: &str,
        assignee_user_id: &str,
        assignee_org_id: &str,
        at: &str,
    ) -> Result<WorkOrderRecord, WorkOrderPersistenceError> {
        let org_id = normalize_work_order_arg(org_id, WorkOrderPersistenceError::EmptyOrgId)?;
        let work_order_id =
            normalize_work_order_arg(work_order_id, WorkOrderPersistenceError::EmptyWorkOrderId)?;
        let actor_user_id =
            normalize_work_order_arg(actor_user_id, WorkOrderPersistenceError::EmptyActorUserId)?;
        let assignee_user_id = normalize_work_order_arg(
            assignee_user_id,
            WorkOrderPersistenceError::EmptyAssigneeUserId,
        )?;
        let assignee_org_id =
            normalize_work_order_arg(assignee_org_id, WorkOrderPersistenceError::EmptyOrgId)?;
        let at = normalize_work_order_arg(at, WorkOrderPersistenceError::EmptyTimestamp)?;
        let before = self.work_order_for_org(&org_id, &work_order_id)?;
        if assignee_org_id != org_id {
            self.history.push(WorkOrderChangeRecord {
                work_order_id: work_order_id.clone(),
                actor_user_id,
                before: Some(before.status),
                after: before.status,
                at,
                change_type: WorkOrderChangeType::Reassigned,
            });
            return Err(WorkOrderPersistenceError::AssigneeOrgMismatch {
                assignee_user_id,
                expected_org_id: org_id,
                actual_org_id: assignee_org_id,
            });
        }

        let mut after = before.clone();
        if after.status == WorkOrderStatus::Created {
            after.status = WorkOrderStatus::Assigned;
        }
        after.assignee_user_id = Some(assignee_user_id);
        after.updated_at = at.clone();
        self.work_orders
            .insert(work_order_id.clone(), after.clone());
        self.history.push(WorkOrderChangeRecord {
            work_order_id,
            actor_user_id,
            before: Some(before.status),
            after: after.status,
            at,
            change_type: WorkOrderChangeType::Reassigned,
        });
        Ok(after)
    }

    pub fn transition_work_order_status(
        &mut self,
        org_id: &str,
        work_order_id: &str,
        actor_user_id: &str,
        at: &str,
        status: WorkOrderStatus,
    ) -> Result<WorkOrderRecord, WorkOrderPersistenceError> {
        let org_id = normalize_work_order_arg(org_id, WorkOrderPersistenceError::EmptyOrgId)?;
        let work_order_id =
            normalize_work_order_arg(work_order_id, WorkOrderPersistenceError::EmptyWorkOrderId)?;
        let actor_user_id =
            normalize_work_order_arg(actor_user_id, WorkOrderPersistenceError::EmptyActorUserId)?;
        let at = normalize_work_order_arg(at, WorkOrderPersistenceError::EmptyTimestamp)?;
        let before = self.work_order_for_org(&org_id, &work_order_id)?;
        if !is_valid_work_order_status_transition(before.status, status) {
            return Err(WorkOrderPersistenceError::InvalidStatusTransition {
                from: before.status,
                to: status,
            });
        }

        let mut after = before.clone();
        after.status = status;
        after.updated_at = at.clone();
        self.work_orders
            .insert(work_order_id.clone(), after.clone());
        self.history.push(WorkOrderChangeRecord {
            work_order_id,
            actor_user_id,
            before: Some(before.status),
            after: status,
            at,
            change_type: WorkOrderChangeType::StatusChanged,
        });
        Ok(after)
    }

    pub fn operator_work_order_queue(&self, query: WorkOrderQueueQuery) -> Vec<WorkOrderRecord> {
        let Some(org_id) = normalize_farm_field_text(query.org_id) else {
            return Vec::new();
        };
        let Some(assignee_user_id) = normalize_farm_field_text(query.assignee_user_id) else {
            return Vec::new();
        };
        let mut work_orders = self
            .work_orders
            .values()
            .filter(|work_order| work_order.org_id == org_id)
            .filter(|work_order| work_order.assignee_user_id.as_deref() == Some(&assignee_user_id))
            .filter(|work_order| {
                query.statuses.is_empty() || query.statuses.contains(&work_order.status)
            })
            .cloned()
            .collect::<Vec<_>>();
        work_orders.sort_by(|left, right| {
            left.due
                .cmp(&right.due)
                .then_with(|| left.work_order_id.cmp(&right.work_order_id))
        });
        work_orders
    }

    pub fn work_orders_for_org(&self, org_id: &str) -> Vec<WorkOrderRecord> {
        let Some(org_id) = normalize_farm_field_text(org_id.to_string()) else {
            return Vec::new();
        };
        let mut work_orders = self
            .work_orders
            .values()
            .filter(|work_order| work_order.org_id == org_id)
            .cloned()
            .collect::<Vec<_>>();
        work_orders.sort_by(|left, right| left.work_order_id.cmp(&right.work_order_id));
        work_orders
    }

    pub fn work_order_history(
        &self,
        org_id: &str,
        work_order_id: &str,
    ) -> Vec<WorkOrderChangeRecord> {
        let Some(org_id) = normalize_farm_field_text(org_id.to_string()) else {
            return Vec::new();
        };
        let Some(work_order_id) = normalize_farm_field_text(work_order_id.to_string()) else {
            return Vec::new();
        };
        self.history
            .iter()
            .filter(|change| change.work_order_id == work_order_id)
            .filter(|change| {
                self.work_orders
                    .get(&change.work_order_id)
                    .is_some_and(|work_order| work_order.org_id == org_id)
            })
            .cloned()
            .collect::<Vec<_>>()
    }

    fn work_order_for_org(
        &self,
        org_id: &str,
        work_order_id: &str,
    ) -> Result<WorkOrderRecord, WorkOrderPersistenceError> {
        self.work_orders
            .get(work_order_id)
            .filter(|work_order| work_order.org_id == org_id)
            .cloned()
            .ok_or_else(|| WorkOrderPersistenceError::WorkOrderNotFound {
                work_order_id: work_order_id.to_string(),
            })
    }
}

fn normalize_work_order_arg(
    value: &str,
    error: WorkOrderPersistenceError,
) -> Result<String, WorkOrderPersistenceError> {
    normalize_farm_field_text(value.to_string()).ok_or(error)
}

fn is_valid_work_order_status_transition(from: WorkOrderStatus, to: WorkOrderStatus) -> bool {
    matches!(
        (from, to),
        (WorkOrderStatus::Created, WorkOrderStatus::Assigned)
            | (WorkOrderStatus::Assigned, WorkOrderStatus::InProgress)
            | (
                WorkOrderStatus::InProgress,
                WorkOrderStatus::Done | WorkOrderStatus::Cancelled
            )
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
    Html,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportVisibility {
    Org,
    Shared,
}

impl Default for ReportVisibility {
    fn default() -> Self {
        Self::Org
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportRecord {
    pub report_id: String,
    pub scene_id: String,
    pub field_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season_id: Option<String>,
    #[serde(default = "default_record_owner")]
    pub org_id: String,
    #[serde(default = "default_record_owner")]
    pub generated_by: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
    pub title: String,
    pub format: ReportFormat,
    pub artifact_path: String,
    #[serde(default)]
    pub artifact_uri: String,
    pub download_url: String,
    #[serde(default)]
    pub visibility: ReportVisibility,
    pub annotation_count: usize,
    pub recommendation_count: usize,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReportPersistenceError {
    #[error("report_id cannot be empty")]
    EmptyReportId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("season_id cannot be empty")]
    EmptySeasonId,
    #[error("org_id cannot be empty")]
    EmptyOrgId,
    #[error("generated_by cannot be empty")]
    EmptyGeneratedBy,
    #[error("artifact_uri cannot be empty")]
    EmptyArtifactUri,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("report must cite at least one source ref: {report_id}")]
    MissingSourceRefs { report_id: String },
    #[error("report already exists: {report_id}")]
    ReportAlreadyExists { report_id: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReportDeliverableRegistry {
    reports: HashMap<String, ReportRecord>,
}

impl ReportDeliverableRegistry {
    pub fn create_report(
        &mut self,
        report: ReportRecord,
    ) -> Result<ReportRecord, ReportPersistenceError> {
        let report = normalize_report_record(report)?;
        if self.reports.contains_key(&report.report_id) {
            return Err(ReportPersistenceError::ReportAlreadyExists {
                report_id: report.report_id,
            });
        }
        self.reports
            .insert(report.report_id.clone(), report.clone());
        Ok(report)
    }

    pub fn reports_for_field_season(
        &self,
        org_id: &str,
        field_id: &str,
        season_id: &str,
    ) -> Vec<ReportRecord> {
        let Some(org_id) = normalize_farm_field_text(org_id.to_string()) else {
            return Vec::new();
        };
        let Some(field_id) = normalize_farm_field_text(field_id.to_string()) else {
            return Vec::new();
        };
        let Some(season_id) = normalize_farm_field_text(season_id.to_string()) else {
            return Vec::new();
        };
        let mut reports = self
            .reports
            .values()
            .filter(|report| {
                report.org_id == org_id
                    && report.field_id.as_deref() == Some(field_id.as_str())
                    && report.season_id.as_deref() == Some(season_id.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();
        reports.sort_by(|left, right| left.report_id.cmp(&right.report_id));
        reports
    }
}

fn normalize_report_record(
    mut report: ReportRecord,
) -> Result<ReportRecord, ReportPersistenceError> {
    report.report_id =
        normalize_farm_field_text(report.report_id).ok_or(ReportPersistenceError::EmptyReportId)?;
    report.field_id = Some(
        report
            .field_id
            .and_then(normalize_farm_field_text)
            .ok_or(ReportPersistenceError::EmptyFieldId)?,
    );
    report.season_id = Some(
        report
            .season_id
            .and_then(normalize_farm_field_text)
            .ok_or(ReportPersistenceError::EmptySeasonId)?,
    );
    report.org_id =
        normalize_farm_field_text(report.org_id).ok_or(ReportPersistenceError::EmptyOrgId)?;
    report.generated_by = normalize_farm_field_text(report.generated_by)
        .ok_or(ReportPersistenceError::EmptyGeneratedBy)?;
    report.artifact_uri = normalize_farm_field_text(report.artifact_uri)
        .ok_or(ReportPersistenceError::EmptyArtifactUri)?;
    report.created_at = normalize_farm_field_text(report.created_at)
        .ok_or(ReportPersistenceError::EmptyCreatedAt)?;
    report.source_refs = report
        .source_refs
        .into_iter()
        .filter_map(normalize_farm_field_text)
        .collect::<Vec<_>>();
    if report.source_refs.is_empty() {
        return Err(ReportPersistenceError::MissingSourceRefs {
            report_id: report.report_id,
        });
    }
    Ok(report)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RasterSpatialRef {
    #[serde(default)]
    pub georeferenced: bool,
    #[serde(default)]
    pub crs: Option<String>,
    #[serde(default)]
    pub bbox: Option<GeoBounds>,
    #[serde(default)]
    pub geo_transform: Option<[f64; 6]>,
    #[serde(default)]
    pub resolution: Option<RasterResolution>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RasterResolution {
    pub x: f64,
    pub y: f64,
}

pub const GEO_EXTENT_ASSERTION_TOLERANCE: f64 = 1.0e-9;
pub const RASTER_RESOLUTION_RELATIVE_TOLERANCE: f64 = 0.05;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RasterSpatialRefError {
    #[error("georeferencing missing spatial_ref")]
    MissingSpatialRef,
    #[error("georeferencing raster dimensions must be positive")]
    NonPositiveDimensions,
    #[error("georeferencing spatial_ref is not marked georeferenced")]
    NotGeoreferenced,
    #[error("georeferencing missing CRS")]
    MissingCrs,
    #[error("georeferencing missing extent bbox")]
    MissingBbox,
    #[error("georeferencing missing transform")]
    MissingTransform,
    #[error("georeferencing transform contains a non-finite value")]
    InvalidTransform,
    #[error("georeferencing requires positive resolution")]
    NonPositiveResolution,
    #[error(
        "georeferencing declared resolution {axis}={declared} differs from transform-derived {derived} beyond tolerance {tolerance}"
    )]
    ResolutionMismatch {
        axis: &'static str,
        declared: f64,
        derived: f64,
        tolerance: f64,
    },
    #[error(
        "georeferencing extent edge {edge}={actual} differs from transform-derived {expected} beyond GEO tolerance {tolerance}"
    )]
    ExtentMismatch {
        edge: &'static str,
        actual: f64,
        expected: f64,
        tolerance: f64,
    },
}

pub fn assert_raster_spatial_ref(
    spatial_ref: Option<&RasterSpatialRef>,
    width: u32,
    height: u32,
) -> Result<RasterSpatialRef, RasterSpatialRefError> {
    let spatial_ref = spatial_ref.ok_or(RasterSpatialRefError::MissingSpatialRef)?;
    if width == 0 || height == 0 {
        return Err(RasterSpatialRefError::NonPositiveDimensions);
    }
    if !spatial_ref.georeferenced {
        return Err(RasterSpatialRefError::NotGeoreferenced);
    }
    let crs = spatial_ref
        .crs
        .as_deref()
        .map(str::trim)
        .filter(|crs| !crs.is_empty())
        .ok_or(RasterSpatialRefError::MissingCrs)?;
    let bbox = spatial_ref
        .bbox
        .as_ref()
        .ok_or(RasterSpatialRefError::MissingBbox)?;
    let transform = spatial_ref
        .geo_transform
        .ok_or(RasterSpatialRefError::MissingTransform)?;
    if !transform.iter().all(|value| value.is_finite()) {
        return Err(RasterSpatialRefError::InvalidTransform);
    }

    let derived_resolution = transform_resolution(&transform)?;
    let resolution = match spatial_ref.resolution {
        Some(declared) => {
            validate_positive_resolution(declared)?;
            assert_resolution_matches("x", declared.x, derived_resolution.x)?;
            assert_resolution_matches("y", declared.y, derived_resolution.y)?;
            declared
        }
        None => derived_resolution,
    };

    let expected_bbox = transform_bbox(&transform, width, height);
    assert_extent_edge("min_lon", bbox.min_lon, expected_bbox.min_lon)?;
    assert_extent_edge("min_lat", bbox.min_lat, expected_bbox.min_lat)?;
    assert_extent_edge("max_lon", bbox.max_lon, expected_bbox.max_lon)?;
    assert_extent_edge("max_lat", bbox.max_lat, expected_bbox.max_lat)?;

    Ok(RasterSpatialRef {
        georeferenced: true,
        crs: Some(crs.to_string()),
        bbox: Some(bbox.clone()),
        geo_transform: Some(transform),
        resolution: Some(resolution),
    })
}

fn transform_resolution(transform: &[f64; 6]) -> Result<RasterResolution, RasterSpatialRefError> {
    let resolution = RasterResolution {
        x: transform[1].hypot(transform[4]),
        y: transform[2].hypot(transform[5]),
    };
    validate_positive_resolution(resolution)?;
    Ok(resolution)
}

fn validate_positive_resolution(resolution: RasterResolution) -> Result<(), RasterSpatialRefError> {
    if resolution.x.is_finite()
        && resolution.y.is_finite()
        && resolution.x > 0.0
        && resolution.y > 0.0
    {
        Ok(())
    } else {
        Err(RasterSpatialRefError::NonPositiveResolution)
    }
}

fn assert_resolution_matches(
    axis: &'static str,
    declared: f64,
    derived: f64,
) -> Result<(), RasterSpatialRefError> {
    let relative_delta = ((declared - derived) / derived).abs();
    if relative_delta <= RASTER_RESOLUTION_RELATIVE_TOLERANCE {
        Ok(())
    } else {
        Err(RasterSpatialRefError::ResolutionMismatch {
            axis,
            declared,
            derived,
            tolerance: RASTER_RESOLUTION_RELATIVE_TOLERANCE,
        })
    }
}

fn transform_bbox(transform: &[f64; 6], width: u32, height: u32) -> GeoBounds {
    let width = width as f64;
    let height = height as f64;
    let corners = [
        transform_point(transform, 0.0, 0.0),
        transform_point(transform, width, 0.0),
        transform_point(transform, 0.0, height),
        transform_point(transform, width, height),
    ];

    let mut min_lon = f64::INFINITY;
    let mut min_lat = f64::INFINITY;
    let mut max_lon = f64::NEG_INFINITY;
    let mut max_lat = f64::NEG_INFINITY;
    for (lon, lat) in corners {
        min_lon = min_lon.min(lon);
        min_lat = min_lat.min(lat);
        max_lon = max_lon.max(lon);
        max_lat = max_lat.max(lat);
    }

    GeoBounds {
        min_lon,
        min_lat,
        max_lon,
        max_lat,
    }
}

fn transform_point(transform: &[f64; 6], x: f64, y: f64) -> (f64, f64) {
    (
        transform[0] + transform[1] * x + transform[2] * y,
        transform[3] + transform[4] * x + transform[5] * y,
    )
}

fn assert_extent_edge(
    edge: &'static str,
    actual: f64,
    expected: f64,
) -> Result<(), RasterSpatialRefError> {
    if actual.is_finite()
        && expected.is_finite()
        && (actual - expected).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
    {
        Ok(())
    } else {
        Err(RasterSpatialRefError::ExtentMismatch {
            edge,
            actual,
            expected,
            tolerance: GEO_EXTENT_ASSERTION_TOLERANCE,
        })
    }
}

/// Telemetry data from flight controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Telemetry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub position: GpsCoords,
    pub battery_voltage: f32,
    pub battery_percentage: u8,
    pub armed: bool,
    pub mode: String,
    pub ground_speed: f32,
    pub air_speed: f32,
    pub heading: f32,
    pub altitude_relative: f32,
}

/// Mission waypoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Waypoint {
    pub sequence: u16,
    pub position: GpsCoords,
    pub command: u16,
    pub auto_continue: bool,
    pub param1: f32,
    pub param2: f32,
    pub param3: f32,
    pub param4: f32,
}

/// Complete mission definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    pub id: uuid::Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub waypoints: Vec<Waypoint>,
    pub home_position: GpsCoords,
}

/// LiDAR scan point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidarPoint {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub angle: f32,
    pub distance: f32,
    pub quality: u8,
}

/// LiDAR scan containing multiple points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidarScan {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub points: Vec<LidarPoint>,
    pub scan_id: uuid::Uuid,
}

/// Multispectral image metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub gps_position: Option<GpsCoords>,
    pub bands: Vec<String>,
    pub exposure_time: f32,
    pub gain: f32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub spatial_ref: Option<RasterSpatialRef>,
}

/// Captured multispectral image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultispectralImage {
    pub metadata: ImageMetadata,
    pub file_paths: HashMap<String, String>, // band_name -> file_path
    pub image_id: uuid::Uuid,
}

/// NDVI processing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NdviResult {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source_images: Vec<uuid::Uuid>,
    pub output_path: String,
    pub min_ndvi: f32,
    pub max_ndvi: f32,
    pub mean_ndvi: f32,
    pub vegetation_percentage: f32,
}

/// WebSocket message types for ground station communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    Telemetry {
        data: Telemetry,
    },
    MissionStatus {
        mission_id: uuid::Uuid,
        status: String,
    },
    LidarUpdate {
        scan: LidarScan,
    },
    ImageCaptured {
        image: MultispectralImage,
    },
    NdviProcessed {
        result: NdviResult,
    },
    SystemStatus {
        status: String,
        message: String,
    },
}

pub fn bounds_from_points(points: &[GeoPoint]) -> Option<GeoBounds> {
    let mut iter = points.iter();
    let first = iter.next()?;

    let mut min_lon = first.longitude;
    let mut max_lon = first.longitude;
    let mut min_lat = first.latitude;
    let mut max_lat = first.latitude;

    for point in iter {
        min_lon = min_lon.min(point.longitude);
        max_lon = max_lon.max(point.longitude);
        min_lat = min_lat.min(point.latitude);
        max_lat = max_lat.max(point.latitude);
    }

    Some(GeoBounds {
        min_lon,
        min_lat,
        max_lon,
        max_lat,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        append_content_version, apply_dry_run_validated_fleet_config_bundle,
        apply_fleet_node_heartbeat, assert_flight_operation_allowed, assert_raster_spatial_ref,
        bind_fleet_node_identity, bounds_from_points, build_collaboration_channel,
        build_collaboration_message, build_fleet_version_inventory,
        build_marketplace_account_record, build_soil_moisture_reading, build_sustainability_record,
        compute_drought_index, create_versioned_content, dry_run_fleet_config_bundle,
        normalize_weather_provider_forecast, sign_fleet_config_bundle,
        soil_moisture_rejection_record, transition_marketplace_account_status,
        validate_field_boundary, verify_and_apply_fleet_config_bundle,
        weather_fetch_failure_record, AnnotationAuditRegistry, AnnotationChangeType,
        AnnotationGeometry, AnnotationPersistenceError, AnnotationRecord, AuditedAnnotationRecord,
        CollaborationChannelCreateRequest, CollaborationError, CollaborationMessageCreateRequest,
        ContentCreateRequest, ContentError, ContentStatus, ContentType, CropPlanRecord,
        DroughtIndexComputeRequest, DroughtIndexError, DroughtIndexPeriod, DroughtIndexType,
        FarmFieldEntityStatus, FarmFieldError, FarmFieldListQuery, FarmFieldRegistry, FarmRecord,
        FieldBoundary, FieldBoundaryValidationError, FieldRecord, FleetConfigApplyStatus,
        FleetConfigBundle, FleetConfigRejectionReason, FleetConfigState, FleetHeartbeatEvaluation,
        FleetInventoryFilter, FleetNodeComponentHealth, FleetNodeComponentStatus,
        FleetNodeEnrollmentError, FleetNodeEnrollmentRequest, FleetNodeHealthState,
        FleetNodeHeartbeat, FleetNodeKind, FleetNodeOperationError, FleetNodeRecord,
        FleetNodeRuntimeMode, FleetNodeStatus, GeoBounds, GeoPoint,
        MarketplaceAccountCreateRequest, MarketplaceAccountError, MarketplaceAccountStatus,
        MarketplacePartyType, MultispectralImage, RasterResolution, RasterSpatialRef,
        RasterSpatialRefError, RecommendationLifecycleRegistry, RecommendationPersistenceError,
        RecommendationPriority, RecommendationRecord, RecommendationStatus,
        RecommendationStatusChangeType, ReportDeliverableRegistry, ReportFormat,
        ReportPersistenceError, ReportRecord, ReportVisibility, SceneFieldCoverageStatus,
        SceneLayerMetadataError, SceneLayerRecord, SceneRecord, SeasonRecord, SoilMoistureQaFlag,
        SoilMoistureReadingError, SoilMoistureReadingRequest, SoilMoistureRejectionReason,
        SustainabilityMetricType, SustainabilityRecordCreateRequest, SustainabilityRecordError,
        SustainabilityRecordLinkage, TractorCommandAuditDecision, TractorCommandRejectionReason,
        TractorImplementRef, TractorLifecycleStatus, TractorMotionCommandRequest,
        TractorRegistrationRequest, TractorRegistry, WeatherIngestError,
        WeatherProviderForecastPoint, WeatherProviderForecastResponse, WorkOrderChangeType,
        WorkOrderCreateRequest, WorkOrderPersistenceError, WorkOrderQueueQuery, WorkOrderRecord,
        WorkOrderRegistry, WorkOrderStatus,
    };

    #[test]
    fn fleet_node_identity_binding_normalizes_new_enrollment() {
        let binding = bind_fleet_node_identity(
            FleetNodeEnrollmentRequest {
                hardware_id: " hw-drone-001 ".to_string(),
                kind: FleetNodeKind::Drone,
                capabilities: vec![
                    "multispectral".to_string(),
                    " LiDAR ".to_string(),
                    "lidar".to_string(),
                ],
                owner_org_id: " org-alpha ".to_string(),
                runtime_mode: FleetNodeRuntimeMode::Simulation,
            },
            None,
            " node-001 ".to_string(),
            " 2026-06-12T12:00:00Z ".to_string(),
        )
        .expect("new enrollment should bind");

        assert!(binding.created);
        assert_eq!(binding.record.node_id, "node-001");
        assert_eq!(binding.record.hardware_id, "hw-drone-001");
        assert_eq!(
            binding.record.capabilities,
            vec!["lidar".to_string(), "multispectral".to_string()]
        );
        assert_eq!(binding.record.owner_org_id, "org-alpha");
        assert_eq!(
            binding.record.runtime_mode,
            FleetNodeRuntimeMode::Simulation
        );
        assert_eq!(binding.record.status, FleetNodeStatus::Enrolled);
    }

    #[test]
    fn fleet_node_identity_binding_rebinds_duplicate_hardware_to_existing_node() {
        let existing = FleetNodeRecord {
            node_id: "node-001".to_string(),
            hardware_id: "hw-drone-001".to_string(),
            kind: FleetNodeKind::Drone,
            capabilities: vec!["lidar".to_string()],
            owner_org_id: "org-alpha".to_string(),
            runtime_mode: FleetNodeRuntimeMode::Simulation,
            enrolled_at: "2026-06-12T12:00:00Z".to_string(),
            status: FleetNodeStatus::Enrolled,
        };

        let binding = bind_fleet_node_identity(
            FleetNodeEnrollmentRequest {
                hardware_id: "hw-drone-001".to_string(),
                kind: FleetNodeKind::Drone,
                capabilities: vec!["thermal".to_string()],
                owner_org_id: "org-beta".to_string(),
                runtime_mode: FleetNodeRuntimeMode::Flight,
            },
            Some(existing.clone()),
            "node-002".to_string(),
            "2026-06-12T13:00:00Z".to_string(),
        )
        .expect("duplicate enrollment should rebind");

        assert!(!binding.created);
        assert_eq!(binding.record, existing);
    }

    #[test]
    fn fleet_node_identity_binding_rejects_missing_hardware_id() {
        let error = bind_fleet_node_identity(
            FleetNodeEnrollmentRequest {
                hardware_id: "  ".to_string(),
                kind: FleetNodeKind::Edge,
                capabilities: vec!["compute".to_string()],
                owner_org_id: "org-alpha".to_string(),
                runtime_mode: FleetNodeRuntimeMode::Simulation,
            },
            None,
            "node-001".to_string(),
            "2026-06-12T12:00:00Z".to_string(),
        )
        .expect_err("blank hardware identity should be rejected");

        assert_eq!(error, FleetNodeEnrollmentError::EmptyHardwareId);
    }

    #[test]
    fn fleet_node_heartbeat_refreshes_capabilities_and_reports_fresh_health() {
        let record = sample_fleet_node(FleetNodeRuntimeMode::Simulation);

        let evaluation = apply_fleet_node_heartbeat(
            &record,
            sample_fleet_heartbeat("2026-06-12T12:00:00Z", FleetNodeRuntimeMode::Flight),
            dt("2026-06-12T12:00:05Z"),
            std::time::Duration::from_secs(10),
            std::time::Duration::from_secs(30),
        )
        .expect("heartbeat should evaluate");

        assert_eq!(
            evaluation.updated_record.runtime_mode,
            FleetNodeRuntimeMode::Flight
        );
        assert_eq!(
            evaluation.updated_record.capabilities,
            vec!["lidar".to_string(), "multispectral".to_string()]
        );
        assert_eq!(evaluation.health.state, FleetNodeHealthState::Fresh);
        assert_eq!(evaluation.health.version, "agbot-node 1.4.0");
        assert_eq!(evaluation.health.config_version, 7);
        assert_eq!(evaluation.health.heartbeat_age_seconds, 5);
        assert_eq!(evaluation.health.components.len(), 2);
    }

    #[test]
    fn fleet_node_health_marks_stale_then_down_after_silent_gap() {
        let record = sample_fleet_node(FleetNodeRuntimeMode::Flight);
        let heartbeat =
            sample_fleet_heartbeat("2026-06-12T12:00:00Z", FleetNodeRuntimeMode::Flight);

        let stale = FleetHeartbeatEvaluation::from_heartbeat(
            &record,
            &heartbeat,
            dt("2026-06-12T12:00:20Z"),
            std::time::Duration::from_secs(10),
            std::time::Duration::from_secs(60),
        )
        .expect("stale health should evaluate");
        let down = FleetHeartbeatEvaluation::from_heartbeat(
            &record,
            &heartbeat,
            dt("2026-06-12T12:01:10Z"),
            std::time::Duration::from_secs(10),
            std::time::Duration::from_secs(60),
        )
        .expect("down health should evaluate");

        assert_eq!(stale.health.state, FleetNodeHealthState::Stale);
        assert_eq!(stale.health.heartbeat_age_seconds, 20);
        assert_eq!(down.health.state, FleetNodeHealthState::Down);
        assert_eq!(down.health.heartbeat_age_seconds, 70);
    }

    #[test]
    fn fleet_version_inventory_aggregates_versions_and_excludes_maintenance_rollouts() {
        let active = sample_fleet_node(FleetNodeRuntimeMode::Flight);
        let mut maintenance = sample_fleet_node(FleetNodeRuntimeMode::Simulation);
        maintenance.node_id = "node-maint".to_string();
        maintenance.hardware_id = "hw-edge-maint".to_string();
        maintenance.kind = FleetNodeKind::Edge;
        maintenance.status = FleetNodeStatus::Maintenance;

        let active_health = apply_fleet_node_heartbeat(
            &active,
            sample_fleet_heartbeat("2026-06-12T12:00:00Z", FleetNodeRuntimeMode::Flight),
            dt("2026-06-12T12:00:05Z"),
            std::time::Duration::from_secs(10),
            std::time::Duration::from_secs(30),
        )
        .expect("active node heartbeat evaluates")
        .health;
        let mut maintenance_health = active_health.clone();
        maintenance_health.node_id = maintenance.node_id.clone();
        maintenance_health.version = "agbot-node 1.3.2".to_string();
        maintenance_health.runtime_mode = FleetNodeRuntimeMode::Simulation;
        maintenance_health.state = FleetNodeHealthState::Stale;

        let default_inventory = build_fleet_version_inventory(
            &[active.clone(), maintenance.clone()],
            &[active_health.clone(), maintenance_health.clone()],
            FleetInventoryFilter::default(),
        );

        assert_eq!(default_inventory.entries.len(), 1);
        assert_eq!(default_inventory.entries[0].node_id, "node-001");
        assert_eq!(
            default_inventory.entries[0].version.as_deref(),
            Some("agbot-node 1.4.0")
        );
        assert_eq!(default_inventory.entries[0].config_version, Some(7));
        assert_eq!(
            default_inventory.rollout_target_node_ids(),
            vec!["node-001".to_string()]
        );

        let full_inventory = build_fleet_version_inventory(
            &[active, maintenance],
            &[active_health, maintenance_health],
            FleetInventoryFilter {
                include_maintenance: true,
                ..Default::default()
            },
        );

        assert_eq!(full_inventory.entries.len(), 2);
        let maintenance_entry = full_inventory
            .entries
            .iter()
            .find(|entry| entry.node_id == "node-maint")
            .expect("maintenance node is included when requested");
        assert!(maintenance_entry.maintenance);
        assert_eq!(
            maintenance_entry.version.as_deref(),
            Some("agbot-node 1.3.2")
        );
        assert_eq!(
            full_inventory.rollout_target_node_ids(),
            vec!["node-001".to_string()]
        );
    }

    #[test]
    fn signed_newer_config_bundle_applies_and_heartbeat_reports_version() {
        let current = FleetConfigState {
            node_id: "node-001".to_string(),
            applied_version: 2,
            payload: "mavlink.rate_hz=1".to_string(),
            applied_at: "2026-06-12T11:00:00Z".to_string(),
        };
        let bundle = FleetConfigBundle {
            node_id: "node-001".to_string(),
            version: 3,
            payload: "mavlink.rate_hz=2".to_string(),
            signature: sign_fleet_config_bundle("node-001", 3, "mavlink.rate_hz=2", "fleet-key"),
        };

        let outcome = verify_and_apply_fleet_config_bundle(
            &current,
            bundle,
            "fleet-key",
            "2026-06-12T12:00:00Z".to_string(),
        )
        .expect("config bundle should be evaluated");

        assert_eq!(outcome.status, FleetConfigApplyStatus::Applied);
        assert_eq!(outcome.updated_state.applied_version, 3);
        assert_eq!(outcome.updated_state.payload, "mavlink.rate_hz=2");

        let record = sample_fleet_node(FleetNodeRuntimeMode::Flight);
        let mut heartbeat =
            sample_fleet_heartbeat("2026-06-12T12:00:00Z", FleetNodeRuntimeMode::Flight);
        heartbeat.config_version = outcome.updated_state.applied_version;
        let evaluation = FleetHeartbeatEvaluation::from_heartbeat(
            &record,
            &heartbeat,
            dt("2026-06-12T12:00:05Z"),
            std::time::Duration::from_secs(15),
            std::time::Duration::from_secs(60),
        )
        .expect("heartbeat should evaluate");

        assert_eq!(evaluation.health.config_version, 3);
    }

    #[test]
    fn fleet_config_dry_run_reports_diff_without_mutating_state() {
        let current = FleetConfigState {
            node_id: "node-001".to_string(),
            applied_version: 2,
            payload: "mavlink.rate_hz=1".to_string(),
            applied_at: "2026-06-12T11:00:00Z".to_string(),
        };
        let bundle = FleetConfigBundle {
            node_id: "node-001".to_string(),
            version: 3,
            payload: "mavlink.rate_hz=2".to_string(),
            signature: sign_fleet_config_bundle("node-001", 3, "mavlink.rate_hz=2", "fleet-key"),
        };

        let dry_run = dry_run_fleet_config_bundle(&current, bundle, "fleet-key")
            .expect("signed bundle should dry-run");

        assert_eq!(current.applied_version, 2);
        assert_eq!(current.payload, "mavlink.rate_hz=1");
        assert_eq!(dry_run.status, FleetConfigApplyStatus::Applied);
        assert!(dry_run.would_apply);
        assert_eq!(dry_run.rejection_reason, None);
        assert_eq!(dry_run.previous_version, 2);
        assert_eq!(dry_run.requested_version, 3);
        assert!(dry_run.diffs.iter().any(|diff| {
            diff.field == "applied_version" && diff.current == "2" && diff.proposed == "3"
        }));
        assert!(dry_run.diffs.iter().any(|diff| {
            diff.field == "payload"
                && diff.current == "mavlink.rate_hz=1"
                && diff.proposed == "mavlink.rate_hz=2"
        }));

        let apply_bundle = FleetConfigBundle {
            node_id: "node-001".to_string(),
            version: 3,
            payload: "mavlink.rate_hz=2".to_string(),
            signature: sign_fleet_config_bundle("node-001", 3, "mavlink.rate_hz=2", "fleet-key"),
        };
        let applied = apply_dry_run_validated_fleet_config_bundle(
            &current,
            apply_bundle,
            "fleet-key",
            "2026-06-12T12:00:00Z".to_string(),
            &dry_run,
        )
        .expect("passing dry-run should allow apply");

        assert_eq!(applied.status, FleetConfigApplyStatus::Applied);
        assert_eq!(applied.updated_state.applied_version, 3);
        assert_eq!(applied.updated_state.payload, "mavlink.rate_hz=2");
    }

    #[test]
    fn fleet_config_apply_requires_passing_dry_run() {
        let current = FleetConfigState {
            node_id: "node-001".to_string(),
            applied_version: 3,
            payload: "mavlink.rate_hz=2".to_string(),
            applied_at: "2026-06-12T12:00:00Z".to_string(),
        };
        let unsigned = FleetConfigBundle {
            node_id: "node-001".to_string(),
            version: 4,
            payload: "mavlink.rate_hz=4".to_string(),
            signature: String::new(),
        };
        let failed_dry_run = dry_run_fleet_config_bundle(&current, unsigned.clone(), "fleet-key")
            .expect("unsigned bundle should return a failed dry-run report");

        assert_eq!(failed_dry_run.status, FleetConfigApplyStatus::Rejected);
        assert!(!failed_dry_run.would_apply);
        assert_eq!(
            failed_dry_run.rejection_reason,
            Some(FleetConfigRejectionReason::MissingSignature)
        );

        let blocked_apply = apply_dry_run_validated_fleet_config_bundle(
            &current,
            unsigned,
            "fleet-key",
            "2026-06-12T12:05:00Z".to_string(),
            &failed_dry_run,
        )
        .expect("failed dry-run should block as a rejected outcome");

        assert_eq!(blocked_apply.status, FleetConfigApplyStatus::Rejected);
        assert_eq!(
            blocked_apply.rejection_reason,
            Some(FleetConfigRejectionReason::MissingSignature)
        );
        assert_eq!(blocked_apply.updated_state, current);
    }

    #[test]
    fn unsigned_or_downgrade_config_bundle_is_rejected_without_mutation() {
        let current = FleetConfigState {
            node_id: "node-001".to_string(),
            applied_version: 3,
            payload: "mavlink.rate_hz=2".to_string(),
            applied_at: "2026-06-12T12:00:00Z".to_string(),
        };
        let unsigned = FleetConfigBundle {
            node_id: "node-001".to_string(),
            version: 4,
            payload: "mavlink.rate_hz=4".to_string(),
            signature: String::new(),
        };

        let unsigned_outcome = verify_and_apply_fleet_config_bundle(
            &current,
            unsigned,
            "fleet-key",
            "2026-06-12T12:05:00Z".to_string(),
        )
        .expect("unsigned bundle should be reported as a rejection");
        assert_eq!(unsigned_outcome.status, FleetConfigApplyStatus::Rejected);
        assert_eq!(
            unsigned_outcome.rejection_reason,
            Some(FleetConfigRejectionReason::MissingSignature)
        );
        assert_eq!(unsigned_outcome.updated_state, current);

        let invalid_signature = FleetConfigBundle {
            node_id: "node-001".to_string(),
            version: 4,
            payload: "mavlink.rate_hz=4".to_string(),
            signature: "agbot-config-v1:bad-signature".to_string(),
        };
        let invalid_signature_outcome = verify_and_apply_fleet_config_bundle(
            &current,
            invalid_signature,
            "fleet-key",
            "2026-06-12T12:05:00Z".to_string(),
        )
        .expect("invalid signature should be reported as a rejection");
        assert_eq!(
            invalid_signature_outcome.rejection_reason,
            Some(FleetConfigRejectionReason::InvalidSignature)
        );
        assert_eq!(invalid_signature_outcome.updated_state, current);

        let downgrade = FleetConfigBundle {
            node_id: "node-001".to_string(),
            version: 2,
            payload: "mavlink.rate_hz=1".to_string(),
            signature: sign_fleet_config_bundle("node-001", 2, "mavlink.rate_hz=1", "fleet-key"),
        };
        let downgrade_outcome = verify_and_apply_fleet_config_bundle(
            &current,
            downgrade,
            "fleet-key",
            "2026-06-12T12:05:00Z".to_string(),
        )
        .expect("downgrade should be reported as a rejection");

        assert_eq!(downgrade_outcome.status, FleetConfigApplyStatus::Rejected);
        assert_eq!(
            downgrade_outcome.rejection_reason,
            Some(FleetConfigRejectionReason::OlderOrEqualVersion)
        );
        assert_eq!(downgrade_outcome.updated_state, current);
    }

    #[test]
    fn flight_only_operation_rejects_simulation_node() {
        let record = sample_fleet_node(FleetNodeRuntimeMode::Simulation);

        let error = assert_flight_operation_allowed(&record)
            .expect_err("simulation node should not accept flight-only work");

        assert_eq!(
            error,
            FleetNodeOperationError::FlightModeRequired {
                node_id: "node-001".to_string(),
                actual: FleetNodeRuntimeMode::Simulation
            }
        );
    }

    #[test]
    fn tractor_registry_links_registered_vehicle_to_owned_field() {
        let farm_fields = tractor_test_farm_fields();
        let mut registry = TractorRegistry::default();

        let record = registry
            .register_tractor(
                TractorRegistrationRequest {
                    tractor_id: Some(" tractor-001 ".to_string()),
                    org_id: " org-alpha ".to_string(),
                    field_id: " field-north ".to_string(),
                    capabilities: vec![
                        "planter".to_string(),
                        " RTK ".to_string(),
                        "rtk".to_string(),
                    ],
                    implement_ref: TractorImplementRef {
                        implement_id: " implement-planter-1 ".to_string(),
                        implement_type: " Planter ".to_string(),
                        working_width_m: Some(9.1),
                    },
                    status: None,
                },
                &farm_fields,
                "2026-06-13T10:00:00Z".to_string(),
            )
            .expect("tractor should register against owned field");

        assert_eq!(record.tractor_id, "tractor-001");
        assert_eq!(record.org_id, "org-alpha");
        assert_eq!(record.field_id, "field-north");
        assert_eq!(
            record.capabilities,
            vec!["planter".to_string(), "rtk".to_string()]
        );
        assert_eq!(record.implement_ref.implement_id, "implement-planter-1");
        assert_eq!(record.implement_ref.implement_type, "planter");
        assert_eq!(record.status, TractorLifecycleStatus::Registered);

        let listed = registry.list_tractors_for_org("org-alpha", None, None);
        assert_eq!(listed, vec![record]);
    }

    #[test]
    fn tractor_lifecycle_progression_and_motion_rejection_are_audited() {
        let farm_fields = tractor_test_farm_fields();
        let mut registry = TractorRegistry::default();
        registry
            .register_tractor(
                tractor_registration_request("tractor-001"),
                &farm_fields,
                "2026-06-13T10:00:00Z".to_string(),
            )
            .expect("tractor registers");

        registry
            .transition_tractor_status(
                "tractor-001",
                TractorLifecycleStatus::Available,
                "2026-06-13T10:05:00Z".to_string(),
            )
            .expect("registered tractor becomes available");
        registry
            .transition_tractor_status(
                "tractor-001",
                TractorLifecycleStatus::InUse,
                "2026-06-13T10:06:00Z".to_string(),
            )
            .expect("available tractor enters use");
        registry
            .transition_tractor_status(
                "tractor-001",
                TractorLifecycleStatus::OutOfService,
                "2026-06-13T10:07:00Z".to_string(),
            )
            .expect("in-use tractor can be taken out of service");

        let error = registry
            .validate_motion_command(
                TractorMotionCommandRequest {
                    command_id: Some("cmd-001".to_string()),
                    tractor_id: "tractor-001".to_string(),
                    command_type: "move".to_string(),
                    requested_by: Some("ops@example.com".to_string()),
                },
                "2026-06-13T10:08:00Z".to_string(),
            )
            .expect_err("out-of-service tractor rejects motion");

        assert_eq!(
            error.reason,
            TractorCommandRejectionReason::TractorOutOfService
        );
        assert_eq!(error.status_code(), 409);
        assert_eq!(registry.command_audits().len(), 1);
        assert_eq!(
            registry.command_audits()[0].decision,
            TractorCommandAuditDecision::Rejected
        );
        assert_eq!(
            registry.command_audits()[0].reason_code,
            "tractor_out_of_service"
        );

        let unknown = registry
            .validate_motion_command(
                TractorMotionCommandRequest {
                    command_id: Some("cmd-unknown".to_string()),
                    tractor_id: "tractor-missing".to_string(),
                    command_type: "move".to_string(),
                    requested_by: Some("ops@example.com".to_string()),
                },
                "2026-06-13T10:09:00Z".to_string(),
            )
            .expect_err("unknown tractor rejects motion");
        assert_eq!(
            unknown.reason,
            TractorCommandRejectionReason::UnknownTractor
        );
        assert_eq!(unknown.status_code(), 404);
        assert_eq!(registry.command_audits().len(), 2);
    }

    #[test]
    fn marketplace_account_links_party_to_one_org_and_normalizes_roles() {
        let record = build_marketplace_account_record(
            MarketplaceAccountCreateRequest {
                account_id: Some(" supplier-001 ".to_string()),
                org_id: " org-alpha ".to_string(),
                party_type: MarketplacePartyType::Supplier,
                role_refs: vec![
                    " marketplace:seller ".to_string(),
                    "inventory-admin".to_string(),
                    "marketplace:seller".to_string(),
                ],
                status: None,
            },
            true,
            "generated-account".to_string(),
            " 2026-06-13T10:00:00Z ".to_string(),
        )
        .expect("marketplace account should normalize");

        assert_eq!(record.account_id, "supplier-001");
        assert_eq!(record.org_id, "org-alpha");
        assert_eq!(record.party_type, MarketplacePartyType::Supplier);
        assert_eq!(
            record.role_refs,
            vec![
                "inventory-admin".to_string(),
                "marketplace:seller".to_string()
            ]
        );
        assert_eq!(record.status, MarketplaceAccountStatus::Active);
        assert_eq!(record.created_at, "2026-06-13T10:00:00Z");
        assert_eq!(record.updated_at, "2026-06-13T10:00:00Z");
    }

    #[test]
    fn marketplace_account_suspend_transition_is_auditable() {
        let record = build_marketplace_account_record(
            MarketplaceAccountCreateRequest {
                account_id: Some("buyer-001".to_string()),
                org_id: "org-alpha".to_string(),
                party_type: MarketplacePartyType::Buyer,
                role_refs: vec!["marketplace:buyer".to_string()],
                status: None,
            },
            true,
            "generated-account".to_string(),
            "2026-06-13T10:00:00Z".to_string(),
        )
        .expect("buyer account creates");

        let suspended = transition_marketplace_account_status(
            &record,
            MarketplaceAccountStatus::Suspended,
            "2026-06-13T11:00:00Z".to_string(),
        )
        .expect("active account can be suspended");

        assert_eq!(suspended.account_id, "buyer-001");
        assert_eq!(suspended.status, MarketplaceAccountStatus::Suspended);
        assert_eq!(suspended.updated_at, "2026-06-13T11:00:00Z");
        assert_eq!(suspended.created_at, record.created_at);
    }

    #[test]
    fn marketplace_account_rejects_unknown_org_without_record() {
        let error = build_marketplace_account_record(
            MarketplaceAccountCreateRequest {
                account_id: Some("supplier-unknown".to_string()),
                org_id: "org-missing".to_string(),
                party_type: MarketplacePartyType::Supplier,
                role_refs: vec!["marketplace:seller".to_string()],
                status: None,
            },
            false,
            "generated-account".to_string(),
            "2026-06-13T10:00:00Z".to_string(),
        )
        .expect_err("unknown org should reject");

        assert_eq!(
            error,
            MarketplaceAccountError::OrganizationNotFound {
                org_id: "org-missing".to_string()
            }
        );
    }

    #[test]
    fn sustainability_record_links_field_season_operation_and_audit() {
        let record = build_sustainability_record(
            SustainabilityRecordCreateRequest {
                record_id: Some(" sustain-001 ".to_string()),
                field_id: " field-alpha ".to_string(),
                season_id: " season-2026 ".to_string(),
                operation_id: " operation-planting-001 ".to_string(),
                metric_type: SustainabilityMetricType::CarbonFootprint,
                method_version: " carbon.identity.v1 ".to_string(),
                audit_id: None,
            },
            Some(SustainabilityRecordLinkage {
                field_id: "field-alpha".to_string(),
                season_id: Some("season-2026".to_string()),
            }),
            "generated-record".to_string(),
            "audit-generated".to_string(),
            "2026-06-13T12:00:00Z".to_string(),
        )
        .expect("sustainability record should link through the field-season spine");

        assert_eq!(record.record_id, "sustain-001");
        assert_eq!(record.field_id, "field-alpha");
        assert_eq!(record.season_id, "season-2026");
        assert_eq!(record.operation_id, "operation-planting-001");
        assert_eq!(
            record.metric_type,
            SustainabilityMetricType::CarbonFootprint
        );
        assert_eq!(record.method_version, "carbon.identity.v1");
        assert_eq!(record.audit_id, "audit-generated");
        assert_eq!(record.created_at, "2026-06-13T12:00:00Z");
    }

    #[test]
    fn sustainability_record_rejects_unknown_field_without_record() {
        let error = build_sustainability_record(
            SustainabilityRecordCreateRequest {
                record_id: Some("sustain-missing".to_string()),
                field_id: "field-missing".to_string(),
                season_id: "season-2026".to_string(),
                operation_id: "operation-planting-001".to_string(),
                metric_type: SustainabilityMetricType::CarbonFootprint,
                method_version: "carbon.identity.v1".to_string(),
                audit_id: Some("audit-missing".to_string()),
            },
            None,
            "generated-record".to_string(),
            "audit-generated".to_string(),
            "2026-06-13T12:00:00Z".to_string(),
        )
        .expect_err("unknown field should reject before record creation");

        assert_eq!(
            error,
            SustainabilityRecordError::FieldNotFound {
                field_id: "field-missing".to_string()
            }
        );
    }

    #[test]
    fn sustainability_record_rejects_season_mismatch_without_record() {
        let error = build_sustainability_record(
            SustainabilityRecordCreateRequest {
                record_id: Some("sustain-mismatch".to_string()),
                field_id: "field-alpha".to_string(),
                season_id: "season-2027".to_string(),
                operation_id: "operation-planting-001".to_string(),
                metric_type: SustainabilityMetricType::CarbonFootprint,
                method_version: "carbon.identity.v1".to_string(),
                audit_id: Some("audit-mismatch".to_string()),
            },
            Some(SustainabilityRecordLinkage {
                field_id: "field-alpha".to_string(),
                season_id: Some("season-2026".to_string()),
            }),
            "generated-record".to_string(),
            "audit-generated".to_string(),
            "2026-06-13T12:00:00Z".to_string(),
        )
        .expect_err("wrong season should reject before record creation");

        assert_eq!(
            error,
            SustainabilityRecordError::SeasonFieldMismatch {
                field_id: "field-alpha".to_string(),
                requested_season_id: "season-2027".to_string(),
                linked_season_id: "season-2026".to_string()
            }
        );
    }

    #[test]
    fn versioned_content_create_and_edit_advances_current_version() {
        let (content, first_version) = create_versioned_content(
            ContentCreateRequest {
                content_id: Some(" article-001 ".to_string()),
                content_type: ContentType::Article,
                author_id: " author-001 ".to_string(),
                org_id: " org-alpha ".to_string(),
                body: " First draft ".to_string(),
                status: None,
            },
            "generated-content".to_string(),
            "version-001".to_string(),
            "2026-06-13T13:00:00Z".to_string(),
        )
        .expect("content should create with first version");

        assert_eq!(content.content_id, "article-001");
        assert_eq!(content.content_type, ContentType::Article);
        assert_eq!(content.author_id, "author-001");
        assert_eq!(content.org_id, "org-alpha");
        assert_eq!(content.status, ContentStatus::Draft);
        assert_eq!(content.current_version, "version-001");
        assert_eq!(first_version.content_id, content.content_id);
        assert_eq!(first_version.body, "First draft");

        let (updated, second_version) = append_content_version(
            &content,
            " Updated body ".to_string(),
            "version-002".to_string(),
            "2026-06-13T14:00:00Z".to_string(),
        )
        .expect("edit should append version");

        assert_eq!(updated.content_id, content.content_id);
        assert_eq!(updated.current_version, "version-002");
        assert_eq!(updated.created_at, content.created_at);
        assert_eq!(updated.updated_at, "2026-06-13T14:00:00Z");
        assert_eq!(second_version.content_id, content.content_id);
        assert_eq!(second_version.body, "Updated body");
    }

    #[test]
    fn versioned_content_rejects_empty_body_without_record() {
        let error = create_versioned_content(
            ContentCreateRequest {
                content_id: Some("article-empty".to_string()),
                content_type: ContentType::Article,
                author_id: "author-001".to_string(),
                org_id: "org-alpha".to_string(),
                body: "   ".to_string(),
                status: None,
            },
            "generated-content".to_string(),
            "version-001".to_string(),
            "2026-06-13T13:00:00Z".to_string(),
        )
        .expect_err("empty body should reject");

        assert_eq!(error, ContentError::EmptyBody);
    }

    #[test]
    fn collaboration_channel_create_and_message_normalizes_membership() {
        let channel = build_collaboration_channel(
            CollaborationChannelCreateRequest {
                channel_id: Some(" channel-001 ".to_string()),
                org_id: " org-alpha ".to_string(),
                field_ref: " field:field-alpha ".to_string(),
                member_account_ids: vec![
                    " user-a ".to_string(),
                    "user-b".to_string(),
                    "user-a".to_string(),
                ],
            },
            "generated-channel".to_string(),
            "2026-06-13T15:00:00Z".to_string(),
        )
        .expect("channel should normalize");

        assert_eq!(channel.channel_id, "channel-001");
        assert_eq!(channel.org_id, "org-alpha");
        assert_eq!(channel.field_ref, "field:field-alpha");
        assert_eq!(channel.member_account_ids, vec!["user-a", "user-b"]);

        let message = build_collaboration_message(
            CollaborationMessageCreateRequest {
                message_id: Some(" message-001 ".to_string()),
                author_id: " user-a ".to_string(),
                body: " Scout north pivot ".to_string(),
            },
            Some(&channel),
            "generated-message".to_string(),
            "2026-06-13T15:05:00Z".to_string(),
        )
        .expect("member can post");

        assert_eq!(message.message_id, "message-001");
        assert_eq!(message.channel_id, "channel-001");
        assert_eq!(message.author_id, "user-a");
        assert_eq!(message.body, "Scout north pivot");
        assert_eq!(message.sent_at, "2026-06-13T15:05:00Z");
    }

    #[test]
    fn collaboration_message_rejects_missing_channel_without_record() {
        let error = build_collaboration_message(
            CollaborationMessageCreateRequest {
                message_id: Some("message-missing".to_string()),
                author_id: "user-a".to_string(),
                body: "hello".to_string(),
            },
            None,
            "generated-message".to_string(),
            "2026-06-13T15:05:00Z".to_string(),
        )
        .expect_err("missing channel should reject");

        assert_eq!(
            error,
            CollaborationError::ChannelNotFound { channel_id: None }
        );
    }

    #[test]
    fn collaboration_message_rejects_non_member_author() {
        let channel = build_collaboration_channel(
            CollaborationChannelCreateRequest {
                channel_id: Some("channel-001".to_string()),
                org_id: "org-alpha".to_string(),
                field_ref: "field:field-alpha".to_string(),
                member_account_ids: vec!["user-a".to_string()],
            },
            "generated-channel".to_string(),
            "2026-06-13T15:00:00Z".to_string(),
        )
        .expect("channel should create");

        let error = build_collaboration_message(
            CollaborationMessageCreateRequest {
                message_id: Some("message-denied".to_string()),
                author_id: "user-b".to_string(),
                body: "hello".to_string(),
            },
            Some(&channel),
            "generated-message".to_string(),
            "2026-06-13T15:05:00Z".to_string(),
        )
        .expect_err("non-member author should reject");

        assert_eq!(
            error,
            CollaborationError::AuthorNotChannelMember {
                channel_id: "channel-001".to_string(),
                author_id: "user-b".to_string()
            }
        );
    }

    #[test]
    fn weather_provider_forecast_normalizes_values_with_source_and_fetch_time() {
        let records = normalize_weather_provider_forecast(
            " field-north ".to_string(),
            WeatherProviderForecastResponse {
                source: " NOAA-HRRR ".to_string(),
                fetched_at: " 2026-06-13T10:00:00Z ".to_string(),
                points: vec![WeatherProviderForecastPoint {
                    valid_time: " 2026-06-13T11:00:00Z ".to_string(),
                    temperature_celsius: 22.5,
                    wind_speed_mps: 4.2,
                    precipitation_mm: 0.1,
                    humidity_percent: 64.0,
                    radiation_w_m2: 720.0,
                }],
            },
        )
        .expect("provider response should normalize");

        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.field_ref, "field-north");
        assert_eq!(record.source, "NOAA-HRRR");
        assert_eq!(record.fetched_at, "2026-06-13T10:00:00Z");
        assert_eq!(record.valid_time, "2026-06-13T11:00:00Z");
        assert_eq!(
            record.forecast_id,
            "weather:field-north:NOAA-HRRR:2026-06-13T11-00-00Z"
        );
        assert_eq!(record.vars.temperature_celsius.value, 22.5);
        assert_eq!(record.vars.temperature_celsius.unit, "deg_c");
        assert_eq!(record.vars.temperature_celsius.source, "NOAA-HRRR");
        assert_eq!(
            record.vars.temperature_celsius.fetched_at,
            "2026-06-13T10:00:00Z"
        );
        assert_eq!(
            record.vars.radiation_w_m2.valid_time,
            "2026-06-13T11:00:00Z"
        );
    }

    #[test]
    fn weather_provider_forecast_rejects_invalid_values_without_partial_records() {
        let error = normalize_weather_provider_forecast(
            "field-north".to_string(),
            WeatherProviderForecastResponse {
                source: "sample".to_string(),
                fetched_at: "2026-06-13T10:00:00Z".to_string(),
                points: vec![WeatherProviderForecastPoint {
                    valid_time: "2026-06-13T11:00:00Z".to_string(),
                    temperature_celsius: 22.5,
                    wind_speed_mps: -1.0,
                    precipitation_mm: 0.0,
                    humidity_percent: 64.0,
                    radiation_w_m2: 720.0,
                }],
            },
        )
        .expect_err("negative wind speed is invalid");

        assert_eq!(
            error,
            WeatherIngestError::InvalidValue {
                variable: "wind_speed_mps".to_string(),
                value: "-1".to_string()
            }
        );
    }

    #[test]
    fn weather_fetch_failure_record_captures_provider_reason() {
        let failure = weather_fetch_failure_record(
            " failure-001 ".to_string(),
            " field-north ".to_string(),
            " NOAA-HRRR ".to_string(),
            " 2026-06-13T10:00:00Z ".to_string(),
            " provider unreachable ".to_string(),
        )
        .expect("failure record should normalize");

        assert_eq!(failure.failure_id, "failure-001");
        assert_eq!(failure.field_ref, "field-north");
        assert_eq!(failure.source, "NOAA-HRRR");
        assert_eq!(failure.reason, "provider unreachable");
    }

    #[test]
    fn soil_moisture_reading_links_to_field_zone_and_qa_flag() {
        let field = tractor_test_farm_fields()
            .field_by_id("field-north")
            .expect("test field exists");

        let record = build_soil_moisture_reading(
            SoilMoistureReadingRequest {
                reading_id: Some(" moisture-001 ".to_string()),
                field_id: Some(" field-north ".to_string()),
                zone_ref: Some(" zone:north ".to_string()),
                value: 34.5,
                source: " probe:soil-001 ".to_string(),
                captured_at: " 2026-06-13T09:30:00Z ".to_string(),
                qa_flag: SoilMoistureQaFlag::Valid,
            },
            &field,
            "generated-reading-id".to_string(),
            " 2026-06-13T09:31:00Z ".to_string(),
        )
        .expect("linked moisture reading should normalize");

        assert_eq!(record.reading_id, "moisture-001");
        assert_eq!(record.field_id, "field-north");
        assert_eq!(record.zone_ref, "zone:north");
        assert_eq!(record.value, 34.5);
        assert_eq!(record.source, "probe:soil-001");
        assert_eq!(record.captured_at, "2026-06-13T09:30:00Z");
        assert_eq!(record.qa_flag, SoilMoistureQaFlag::Valid);
        assert_eq!(record.ingested_at, "2026-06-13T09:31:00Z");
    }

    #[test]
    fn soil_moisture_reading_rejects_missing_zone_linkage_and_audits_reason() {
        let field = tractor_test_farm_fields()
            .field_by_id("field-north")
            .expect("test field exists");
        let request = SoilMoistureReadingRequest {
            reading_id: Some("moisture-orphan".to_string()),
            field_id: Some("field-north".to_string()),
            zone_ref: Some(" ".to_string()),
            value: 34.5,
            source: "probe:soil-001".to_string(),
            captured_at: "2026-06-13T09:30:00Z".to_string(),
            qa_flag: SoilMoistureQaFlag::Valid,
        };

        let error = build_soil_moisture_reading(
            request.clone(),
            &field,
            "generated-reading-id".to_string(),
            "2026-06-13T09:31:00Z".to_string(),
        )
        .expect_err("zone linkage is required");
        assert_eq!(error, SoilMoistureReadingError::MissingZoneLinkage);

        let rejection = soil_moisture_rejection_record(
            " rejection-001 ".to_string(),
            &request,
            SoilMoistureRejectionReason::MissingZoneLinkage,
            " 2026-06-13T09:31:00Z ".to_string(),
        )
        .expect("rejection should normalize");

        assert_eq!(rejection.rejection_id, "rejection-001");
        assert_eq!(rejection.field_id.as_deref(), Some("field-north"));
        assert_eq!(rejection.zone_ref, None);
        assert_eq!(
            rejection.reason,
            SoilMoistureRejectionReason::MissingZoneLinkage
        );
        assert_eq!(rejection.rejected_at, "2026-06-13T09:31:00Z");
    }

    #[test]
    fn soil_moisture_reading_rejects_invalid_percent_value() {
        let field = tractor_test_farm_fields()
            .field_by_id("field-north")
            .expect("test field exists");

        let error = build_soil_moisture_reading(
            SoilMoistureReadingRequest {
                reading_id: None,
                field_id: Some("field-north".to_string()),
                zone_ref: Some("zone:north".to_string()),
                value: 140.0,
                source: "probe:soil-001".to_string(),
                captured_at: "2026-06-13T09:30:00Z".to_string(),
                qa_flag: SoilMoistureQaFlag::Valid,
            },
            &field,
            "generated-reading-id".to_string(),
            "2026-06-13T09:31:00Z".to_string(),
        )
        .expect_err("moisture percent outside 0..=100 should reject");

        assert_eq!(
            error,
            SoilMoistureReadingError::InvalidValue {
                value: "140".to_string()
            }
        );
    }

    #[test]
    fn drought_index_compute_persists_standardized_value_and_input_refs() {
        let record = compute_drought_index(
            DroughtIndexComputeRequest {
                index_id: Some(" drought-spi-001 ".to_string()),
                field_or_region_ref: " field:field-north ".to_string(),
                index_type: DroughtIndexType::Spi,
                period: DroughtIndexPeriod {
                    start: "2026-04-01".to_string(),
                    end: "2026-06-30".to_string(),
                    accumulation_days: Some(90),
                },
                observed_value: 45.0,
                baseline_mean: 60.0,
                baseline_std_dev: 10.0,
                input_refs: vec![
                    " weather:field-north:precip:q2 ".to_string(),
                    "water:field-north:balance:q2".to_string(),
                    "weather:field-north:precip:q2".to_string(),
                ],
                computed_at: Some(" 2026-06-13T10:00:00Z ".to_string()),
            },
            "generated-drought-index".to_string(),
            "2026-06-13T10:01:00Z".to_string(),
        )
        .expect("drought index should compute");

        assert_eq!(record.index_id, "drought-spi-001");
        assert_eq!(record.field_or_region_ref, "field:field-north");
        assert_eq!(record.index_type, DroughtIndexType::Spi);
        assert_eq!(record.value, -1.5);
        assert_eq!(record.period.start, "2026-04-01");
        assert_eq!(record.period.end, "2026-06-30");
        assert_eq!(
            record.input_refs,
            vec![
                "water:field-north:balance:q2".to_string(),
                "weather:field-north:precip:q2".to_string()
            ]
        );
        assert_eq!(record.method, "standardized_anomaly_v1");
        assert_eq!(record.computed_at, "2026-06-13T10:00:00Z");
    }

    #[test]
    fn drought_index_compute_rejects_missing_input_refs() {
        let error = compute_drought_index(
            DroughtIndexComputeRequest {
                index_id: Some("drought-spi-001".to_string()),
                field_or_region_ref: "field:field-north".to_string(),
                index_type: DroughtIndexType::Spi,
                period: DroughtIndexPeriod {
                    start: "2026-04-01".to_string(),
                    end: "2026-06-30".to_string(),
                    accumulation_days: Some(90),
                },
                observed_value: 45.0,
                baseline_mean: 60.0,
                baseline_std_dev: 10.0,
                input_refs: vec![" ".to_string()],
                computed_at: Some("2026-06-13T10:00:00Z".to_string()),
            },
            "generated-drought-index".to_string(),
            "2026-06-13T10:01:00Z".to_string(),
        )
        .expect_err("untraceable index should reject");

        assert_eq!(error, DroughtIndexError::EmptyInputRefs);
    }

    #[test]
    fn drought_index_compute_rejects_zero_baseline_std_dev() {
        let error = compute_drought_index(
            DroughtIndexComputeRequest {
                index_id: None,
                field_or_region_ref: "field:field-north".to_string(),
                index_type: DroughtIndexType::Spei,
                period: DroughtIndexPeriod {
                    start: "2026-04-01".to_string(),
                    end: "2026-06-30".to_string(),
                    accumulation_days: Some(90),
                },
                observed_value: 12.0,
                baseline_mean: 20.0,
                baseline_std_dev: 0.0,
                input_refs: vec!["weather:field-north:water-balance:q2".to_string()],
                computed_at: None,
            },
            "generated-drought-index".to_string(),
            "2026-06-13T10:01:00Z".to_string(),
        )
        .expect_err("zero baseline spread cannot produce standardized index");

        assert_eq!(error, DroughtIndexError::InvalidBaselineStdDev);
    }

    fn sample_fleet_node(runtime_mode: FleetNodeRuntimeMode) -> FleetNodeRecord {
        FleetNodeRecord {
            node_id: "node-001".to_string(),
            hardware_id: "hw-drone-001".to_string(),
            kind: FleetNodeKind::Drone,
            capabilities: vec!["multispectral".to_string()],
            owner_org_id: "org-alpha".to_string(),
            runtime_mode,
            enrolled_at: "2026-06-12T11:00:00Z".to_string(),
            status: FleetNodeStatus::Enrolled,
        }
    }

    fn sample_fleet_heartbeat(at: &str, runtime_mode: FleetNodeRuntimeMode) -> FleetNodeHeartbeat {
        FleetNodeHeartbeat {
            node_id: "node-001".to_string(),
            version: "agbot-node 1.4.0".to_string(),
            config_version: 7,
            components: vec![
                FleetNodeComponentStatus {
                    component: "flight_controller".to_string(),
                    health: FleetNodeComponentHealth::Ok,
                    message: None,
                },
                FleetNodeComponentStatus {
                    component: "camera".to_string(),
                    health: FleetNodeComponentHealth::Ok,
                    message: Some("ready".to_string()),
                },
            ],
            uptime_seconds: 3600,
            at: dt(at),
            capabilities: vec![" LiDAR ".to_string(), "multispectral".to_string()],
            runtime_mode,
        }
    }

    fn dt(value: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    fn tractor_test_farm_fields() -> FarmFieldRegistry {
        let mut registry = FarmFieldRegistry::default();
        registry
            .insert_farm(test_farm_record(
                "farm-alpha",
                "org-alpha",
                "Alpha Farm",
                FarmFieldEntityStatus::Active,
            ))
            .expect("farm inserts");
        registry
            .insert_field(test_field_record(
                "field-north",
                "farm-alpha",
                "org-alpha",
                "North Field",
                FarmFieldEntityStatus::Active,
            ))
            .expect("field inserts");
        registry
    }

    fn tractor_registration_request(tractor_id: &str) -> TractorRegistrationRequest {
        TractorRegistrationRequest {
            tractor_id: Some(tractor_id.to_string()),
            org_id: "org-alpha".to_string(),
            field_id: "field-north".to_string(),
            capabilities: vec!["rtk".to_string(), "planter".to_string()],
            implement_ref: TractorImplementRef {
                implement_id: "implement-planter-1".to_string(),
                implement_type: "planter".to_string(),
                working_width_m: Some(9.1),
            },
            status: None,
        }
    }

    #[test]
    fn multispectral_image_deserializes_without_spatial_ref() {
        let payload = serde_json::json!({
            "metadata": {
                "timestamp": "2025-01-01T00:00:00Z",
                "gps_position": null,
                "bands": ["B4", "B5"],
                "exposure_time": 1.0,
                "gain": 1.0,
                "width": 64,
                "height": 32
            },
            "file_paths": {
                "B4": "B4.tif",
                "B5": "B5.tif"
            },
            "image_id": "00000000-0000-0000-0000-000000000000"
        });

        let image: MultispectralImage =
            serde_json::from_value(payload).expect("legacy metadata should deserialize");

        assert_eq!(image.metadata.spatial_ref, None);
    }

    #[test]
    fn multispectral_image_deserializes_with_spatial_ref() {
        let payload = serde_json::json!({
            "metadata": {
                "timestamp": "2025-01-01T00:00:00Z",
                "gps_position": null,
                "bands": ["B4", "B5"],
                "exposure_time": 1.0,
                "gain": 1.0,
                "width": 64,
                "height": 32,
                "spatial_ref": {
                    "georeferenced": true,
                    "crs": "EPSG:4326",
                    "bbox": {
                        "min_lon": -74.1,
                        "min_lat": 40.6,
                        "max_lon": -73.9,
                        "max_lat": 40.8
                    },
                    "geo_transform": [-74.1, 0.0001, 0.0, 40.8, 0.0, -0.0001]
                }
            },
            "file_paths": {
                "B4": "B4.tif",
                "B5": "B5.tif"
            },
            "image_id": "00000000-0000-0000-0000-000000000000"
        });

        let image: MultispectralImage =
            serde_json::from_value(payload).expect("spatial metadata should deserialize");

        assert_eq!(
            image.metadata.spatial_ref,
            Some(RasterSpatialRef {
                georeferenced: true,
                crs: Some("EPSG:4326".to_string()),
                bbox: Some(super::GeoBounds {
                    min_lon: -74.1,
                    min_lat: 40.6,
                    max_lon: -73.9,
                    max_lat: 40.8,
                }),
                geo_transform: Some([-74.1, 0.0001, 0.0, 40.8, 0.0, -0.0001]),
                resolution: None,
            })
        );
    }

    #[test]
    fn raster_spatial_ref_asserts_extent_and_resolution() {
        let spatial_ref = RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:4326".to_string()),
            bbox: Some(GeoBounds {
                min_lon: -74.1,
                min_lat: 40.7998,
                max_lon: -74.0998,
                max_lat: 40.8,
            }),
            geo_transform: Some([-74.1, 0.0001, 0.0, 40.8, 0.0, -0.0001]),
            resolution: None,
        };

        let asserted =
            assert_raster_spatial_ref(Some(&spatial_ref), 2, 2).expect("spatial ref should assert");

        assert_eq!(asserted.crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(
            asserted.resolution,
            Some(RasterResolution {
                x: 0.0001,
                y: 0.0001
            })
        );
    }

    #[test]
    fn raster_spatial_ref_rejects_missing_crs() {
        let spatial_ref = RasterSpatialRef {
            georeferenced: true,
            crs: Some(" ".to_string()),
            bbox: Some(GeoBounds {
                min_lon: -74.1,
                min_lat: 40.7999,
                max_lon: -74.0999,
                max_lat: 40.8,
            }),
            geo_transform: Some([-74.1, 0.0001, 0.0, 40.8, 0.0, -0.0001]),
            resolution: None,
        };

        let error = assert_raster_spatial_ref(Some(&spatial_ref), 1, 1).unwrap_err();

        assert_eq!(error, RasterSpatialRefError::MissingCrs);
    }

    #[test]
    fn raster_spatial_ref_rejects_non_positive_resolution() {
        let spatial_ref = RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:4326".to_string()),
            bbox: Some(GeoBounds {
                min_lon: -74.1,
                min_lat: 40.7999,
                max_lon: -74.1,
                max_lat: 40.8,
            }),
            geo_transform: Some([-74.1, 0.0, 0.0, 40.8, 0.0, -0.0001]),
            resolution: None,
        };

        let error = assert_raster_spatial_ref(Some(&spatial_ref), 1, 1).unwrap_err();

        assert_eq!(error, RasterSpatialRefError::NonPositiveResolution);
    }

    #[test]
    fn bounds_from_points_computes_expected_bbox() {
        let bounds = bounds_from_points(&[
            GeoPoint {
                longitude: -96.5,
                latitude: 41.2,
            },
            GeoPoint {
                longitude: -96.2,
                latitude: 41.4,
            },
            GeoPoint {
                longitude: -96.7,
                latitude: 41.1,
            },
        ])
        .expect("bounds should exist");

        assert_eq!(
            bounds,
            GeoBounds {
                min_lon: -96.7,
                min_lat: 41.1,
                max_lon: -96.2,
                max_lat: 41.4,
            }
        );
    }

    #[test]
    fn field_record_round_trips_through_json() {
        let field = FieldRecord {
            farm_id: Some("farm-1".to_string()),
            field_id: "field-1".to_string(),
            org_id: "org-1".to_string(),
            owner: "org-1".to_string(),
            name: "North 80".to_string(),
            area_ha: Some(32.4),
            crop: Some("corn".to_string()),
            season: Some("2026".to_string()),
            notes: Some("pivot irrigation".to_string()),
            boundary: FieldBoundary {
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
                ],
            },
            extent: GeoBounds {
                min_lon: -96.5,
                min_lat: 41.2,
                max_lon: -96.2,
                max_lat: 41.4,
            },
            status: FarmFieldEntityStatus::Active,
            created_at: "2026-04-01T00:00:00Z".to_string(),
            updated_at: "2026-04-01T00:00:00Z".to_string(),
        };

        let value = serde_json::to_value(&field).expect("field should serialize");
        let decoded: FieldRecord = serde_json::from_value(value).expect("field should deserialize");

        assert_eq!(decoded, field);
    }

    #[test]
    fn farm_field_registry_lists_records_under_org_only() {
        let mut registry = FarmFieldRegistry::default();
        let farm = FarmRecord {
            farm_id: "farm-a".to_string(),
            org_id: "org-a".to_string(),
            owner: "org-a".to_string(),
            name: "Prairie Farm".to_string(),
            notes: None,
            status: FarmFieldEntityStatus::Active,
            created_at: "2026-04-01T00:00:00Z".to_string(),
            updated_at: "2026-04-01T00:00:00Z".to_string(),
        };
        let field = FieldRecord {
            farm_id: Some(farm.farm_id.clone()),
            field_id: "field-a".to_string(),
            org_id: "org-a".to_string(),
            owner: "org-a".to_string(),
            name: "North 80".to_string(),
            area_ha: Some(32.4),
            crop: Some("corn".to_string()),
            season: Some("2026".to_string()),
            notes: None,
            boundary: test_boundary(),
            extent: test_extent(),
            status: FarmFieldEntityStatus::Active,
            created_at: "2026-04-01T00:00:00Z".to_string(),
            updated_at: "2026-04-01T00:00:00Z".to_string(),
        };

        registry.insert_farm(farm).expect("farm persists");
        registry.insert_field(field).expect("field persists");

        let farms = registry.farms_for_org("org-a");
        let fields = registry.fields_for_org("org-a");

        assert_eq!(farms.len(), 1);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].farm_id.as_deref(), Some("farm-a"));
        assert_eq!(fields[0].org_id, "org-a");
        assert!(registry.farms_for_org("org-b").is_empty());
        assert!(registry.fields_for_org("org-b").is_empty());
    }

    #[test]
    fn farm_field_registry_paginates_active_fields_by_org() {
        let mut registry = FarmFieldRegistry::default();
        registry
            .insert_farm(test_farm_record(
                "farm-a",
                "org-a",
                "Prairie Farm",
                FarmFieldEntityStatus::Active,
            ))
            .expect("farm persists");
        registry
            .insert_farm(test_farm_record(
                "farm-b",
                "org-b",
                "Other Farm",
                FarmFieldEntityStatus::Active,
            ))
            .expect("other org farm persists");

        for (field_id, name) in [
            ("field-a", "Alpha Field"),
            ("field-b", "Beta Field"),
            ("field-c", "Gamma Field"),
        ] {
            registry
                .insert_field(test_field_record(
                    field_id,
                    "farm-a",
                    "org-a",
                    name,
                    FarmFieldEntityStatus::Active,
                ))
                .expect("field persists");
        }
        registry
            .insert_field(test_field_record(
                "field-x",
                "farm-b",
                "org-b",
                "Foreign Field",
                FarmFieldEntityStatus::Active,
            ))
            .expect("other org field persists");

        let page = registry.list_fields_for_org(
            "org-a",
            FarmFieldListQuery {
                page: Some(2),
                page_size: Some(2),
                status: Some(FarmFieldEntityStatus::Active),
            },
        );

        assert_eq!(page.total_count, 3);
        assert_eq!(page.page, 2);
        assert_eq!(page.page_size, 2);
        assert_eq!(
            page.items
                .iter()
                .map(|field| field.field_id.as_str())
                .collect::<Vec<_>>(),
            vec!["field-c"]
        );

        let beyond = registry.list_fields_for_org(
            "org-a",
            FarmFieldListQuery {
                page: Some(4),
                page_size: Some(2),
                status: Some(FarmFieldEntityStatus::Active),
            },
        );
        assert_eq!(beyond.total_count, 3);
        assert!(beyond.items.is_empty());
    }

    #[test]
    fn farm_field_registry_default_lists_exclude_archived_entities() {
        let mut registry = FarmFieldRegistry::default();
        registry
            .insert_farm(test_farm_record(
                "farm-a",
                "org-a",
                "Active Farm",
                FarmFieldEntityStatus::Active,
            ))
            .expect("farm persists");
        registry
            .insert_farm(test_farm_record(
                "farm-archived",
                "org-a",
                "Archived Farm",
                FarmFieldEntityStatus::Archived,
            ))
            .expect("archived farm persists");
        registry
            .insert_field(test_field_record(
                "field-active",
                "farm-a",
                "org-a",
                "Active Field",
                FarmFieldEntityStatus::Active,
            ))
            .expect("active field persists");
        registry
            .insert_field(test_field_record(
                "field-archived",
                "farm-a",
                "org-a",
                "Archived Field",
                FarmFieldEntityStatus::Archived,
            ))
            .expect("archived field persists");

        assert_eq!(
            registry
                .farms_for_org("org-a")
                .iter()
                .map(|farm| farm.farm_id.as_str())
                .collect::<Vec<_>>(),
            vec!["farm-a"]
        );
        assert_eq!(
            registry
                .fields_for_org("org-a")
                .iter()
                .map(|field| field.field_id.as_str())
                .collect::<Vec<_>>(),
            vec!["field-active"]
        );
        assert_eq!(
            registry
                .list_boundaries_for_org("org-a", FarmFieldListQuery::default())
                .items
                .iter()
                .map(|boundary| boundary.field_id.as_str())
                .collect::<Vec<_>>(),
            vec!["field-active"]
        );

        let archived_fields = registry.list_fields_for_org(
            "org-a",
            FarmFieldListQuery {
                status: Some(FarmFieldEntityStatus::Archived),
                ..FarmFieldListQuery::default()
            },
        );
        assert_eq!(archived_fields.total_count, 1);
        assert_eq!(archived_fields.items[0].field_id, "field-archived");

        let archived_farms = registry.list_farms_for_org(
            "org-a",
            FarmFieldListQuery {
                status: Some(FarmFieldEntityStatus::Archived),
                ..FarmFieldListQuery::default()
            },
        );
        assert_eq!(archived_farms.total_count, 1);
        assert_eq!(archived_farms.items[0].farm_id, "farm-archived");
    }

    #[test]
    fn field_with_cross_org_farm_is_rejected_without_writing() {
        let mut registry = FarmFieldRegistry::default();
        registry
            .insert_farm(FarmRecord {
                farm_id: "farm-a".to_string(),
                org_id: "org-a".to_string(),
                owner: "org-a".to_string(),
                name: "Prairie Farm".to_string(),
                notes: None,
                status: FarmFieldEntityStatus::Active,
                created_at: "2026-04-01T00:00:00Z".to_string(),
                updated_at: "2026-04-01T00:00:00Z".to_string(),
            })
            .expect("farm persists");

        let error = registry
            .insert_field(FieldRecord {
                farm_id: Some("farm-a".to_string()),
                field_id: "field-b".to_string(),
                org_id: "org-b".to_string(),
                owner: "org-b".to_string(),
                name: "Other Org Field".to_string(),
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
            .expect_err("cross-org farm link is rejected");

        assert_eq!(
            error,
            FarmFieldError::TenantBoundary {
                farm_id: "farm-a".to_string(),
                farm_org_id: "org-a".to_string(),
                field_org_id: "org-b".to_string()
            }
        );
        assert!(registry.fields_for_org("org-b").is_empty());
    }

    #[test]
    fn field_boundary_validation_computes_extent_area_and_preserves_crs() {
        let boundary = test_boundary();

        let validated = validate_field_boundary(&boundary).expect("boundary validates");

        assert_eq!(validated.boundary, boundary);
        assert_eq!(validated.extent, test_extent());
        assert!(validated.area_ha > 0.0);
    }

    #[test]
    fn field_registry_rejects_unclosed_boundary_without_writing() {
        let mut registry = FarmFieldRegistry::default();
        registry
            .insert_farm(FarmRecord {
                farm_id: "farm-a".to_string(),
                org_id: "org-a".to_string(),
                owner: "org-a".to_string(),
                name: "Prairie Farm".to_string(),
                notes: None,
                status: FarmFieldEntityStatus::Active,
                created_at: "2026-04-01T00:00:00Z".to_string(),
                updated_at: "2026-04-01T00:00:00Z".to_string(),
            })
            .expect("farm persists");

        let error = registry
            .insert_field(FieldRecord {
                farm_id: Some("farm-a".to_string()),
                field_id: "field-a".to_string(),
                org_id: "org-a".to_string(),
                owner: "org-a".to_string(),
                name: "North 80".to_string(),
                area_ha: None,
                crop: None,
                season: None,
                notes: None,
                boundary: unclosed_test_boundary(),
                extent: test_extent(),
                status: FarmFieldEntityStatus::Active,
                created_at: "2026-04-01T00:00:00Z".to_string(),
                updated_at: "2026-04-01T00:00:00Z".to_string(),
            })
            .expect_err("unclosed boundary is rejected");

        assert_eq!(
            error,
            FarmFieldError::BoundaryInvalid {
                reason: FieldBoundaryValidationError::RingNotClosed
            }
        );
        assert!(registry.fields_for_org("org-a").is_empty());
    }

    #[test]
    fn field_boundary_validation_rejects_self_intersection() {
        let boundary = FieldBoundary {
            crs: Some("EPSG:4326".to_string()),
            coordinates: vec![
                GeoPoint {
                    longitude: -96.5,
                    latitude: 41.2,
                },
                GeoPoint {
                    longitude: -96.2,
                    latitude: 41.4,
                },
                GeoPoint {
                    longitude: -96.2,
                    latitude: 41.2,
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
        };

        let error = validate_field_boundary(&boundary).expect_err("bowtie ring is rejected");

        assert_eq!(error, FieldBoundaryValidationError::SelfIntersection);
    }

    #[test]
    fn season_and_crop_plan_history_is_chronological() {
        let mut registry = registry_with_field();

        registry
            .insert_season(SeasonRecord {
                season_id: "season-2026".to_string(),
                field_id: "field-a".to_string(),
                org_id: "org-a".to_string(),
                start: "2026-03-01".to_string(),
                end: "2026-10-31".to_string(),
                label: "2026 Corn".to_string(),
            })
            .expect("2026 season persists");
        registry
            .insert_season(SeasonRecord {
                season_id: "season-2025".to_string(),
                field_id: "field-a".to_string(),
                org_id: "org-a".to_string(),
                start: "2025-03-01".to_string(),
                end: "2025-10-31".to_string(),
                label: "2025 Soy".to_string(),
            })
            .expect("2025 season persists");
        registry
            .insert_crop_plan(CropPlanRecord {
                crop_plan_id: "plan-2026".to_string(),
                season_id: "season-2026".to_string(),
                org_id: String::new(),
                crop: "corn".to_string(),
                planting_date: Some("2026-04-15".to_string()),
            })
            .expect("crop plan persists");

        let history = registry.season_history_for_field("org-a", "field-a");

        assert_eq!(history.len(), 2);
        assert_eq!(history[0].season.season_id, "season-2025");
        assert_eq!(history[1].season.season_id, "season-2026");
        assert_eq!(history[1].crop_plans.len(), 1);
        assert_eq!(history[1].crop_plans[0].crop, "corn");
        assert_eq!(history[1].crop_plans[0].org_id, "org-a");
    }

    #[test]
    fn overlapping_season_is_rejected_without_writing() {
        let mut registry = registry_with_field();
        registry
            .insert_season(SeasonRecord {
                season_id: "season-2026".to_string(),
                field_id: "field-a".to_string(),
                org_id: "org-a".to_string(),
                start: "2026-03-01".to_string(),
                end: "2026-10-31".to_string(),
                label: "2026 Corn".to_string(),
            })
            .expect("season persists");

        let error = registry
            .insert_season(SeasonRecord {
                season_id: "season-overlap".to_string(),
                field_id: "field-a".to_string(),
                org_id: "org-a".to_string(),
                start: "2026-05-01".to_string(),
                end: "2026-09-30".to_string(),
                label: "Overlapping season".to_string(),
            })
            .expect_err("overlap is rejected");

        assert_eq!(
            error,
            FarmFieldError::SeasonOverlap {
                field_id: "field-a".to_string(),
                season_id: "season-overlap".to_string(),
                overlapping_season_id: "season-2026".to_string()
            }
        );
        assert_eq!(
            registry.season_history_for_field("org-a", "field-a").len(),
            1
        );
    }

    #[test]
    fn active_season_resolution_returns_matching_season_or_none() {
        let mut registry = registry_with_field();
        registry
            .insert_season(SeasonRecord {
                season_id: "season-2025".to_string(),
                field_id: "field-a".to_string(),
                org_id: "org-a".to_string(),
                start: "2025-03-01".to_string(),
                end: "2025-10-31".to_string(),
                label: "2025 Soy".to_string(),
            })
            .expect("2025 season persists");
        registry
            .insert_season(SeasonRecord {
                season_id: "season-2026".to_string(),
                field_id: "field-a".to_string(),
                org_id: "org-a".to_string(),
                start: "2026-03-01".to_string(),
                end: "2026-10-31".to_string(),
                label: "2026 Corn".to_string(),
            })
            .expect("2026 season persists");

        let active = registry
            .active_season_for_field("org-a", "field-a", "2026-06-14")
            .expect("active season resolves");
        assert_eq!(
            active.active_season.map(|season| season.season_id),
            Some("season-2026".to_string())
        );

        let inactive = registry
            .active_season_for_field("org-a", "field-a", "2026-12-01")
            .expect("no active season is explicit");
        assert_eq!(inactive.active_season, None);
    }

    #[test]
    fn scene_and_layers_are_listable_by_field_and_season() {
        let mut registry = registry_with_field_and_season();
        registry
            .insert_scene(SceneRecord {
                scene_id: "scene-2026-04-15".to_string(),
                field_id: "field-a".to_string(),
                season_id: "season-2026".to_string(),
                org_id: "org-a".to_string(),
                captured_at: "2026-04-15T14:30:00Z".to_string(),
                source: "landsat".to_string(),
            })
            .expect("scene persists");
        registry
            .insert_scene_layer(SceneLayerRecord {
                layer_id: "layer-ndvi".to_string(),
                scene_id: "scene-2026-04-15".to_string(),
                product_type: "ndvi".to_string(),
                crs: "EPSG:4326".to_string(),
                extent: Some(test_extent()),
                resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
                uri: "s3://agbot/scenes/scene-2026-04-15/ndvi.tif".to_string(),
            })
            .expect("layer persists");

        let scenes = registry.scenes_for_field_season("org-a", "field-a", "season-2026");

        assert_eq!(scenes.len(), 1);
        assert_eq!(scenes[0].scene.scene_id, "scene-2026-04-15");
        assert_eq!(scenes[0].scene.source, "landsat");
        assert_eq!(scenes[0].layers.len(), 1);
        assert_eq!(scenes[0].layers[0].product_type, "ndvi");
        assert_eq!(scenes[0].layers[0].crs, "EPSG:4326");
        assert_eq!(scenes[0].layers[0].extent, Some(test_extent()));
        assert_eq!(
            scenes[0].layers[0].resolution,
            Some(RasterResolution { x: 10.0, y: 10.0 })
        );
    }

    #[test]
    fn scene_layer_missing_metadata_is_rejected_without_writing() {
        let mut registry = registry_with_field_and_season();
        registry
            .insert_scene(SceneRecord {
                scene_id: "scene-2026-04-15".to_string(),
                field_id: "field-a".to_string(),
                season_id: "season-2026".to_string(),
                org_id: "org-a".to_string(),
                captured_at: "2026-04-15T14:30:00Z".to_string(),
                source: "landsat".to_string(),
            })
            .expect("scene persists");

        let error = registry
            .insert_scene_layer(SceneLayerRecord {
                layer_id: "layer-missing-crs".to_string(),
                scene_id: "scene-2026-04-15".to_string(),
                product_type: "ndvi".to_string(),
                crs: " ".to_string(),
                extent: Some(test_extent()),
                resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
                uri: "s3://agbot/scenes/scene-2026-04-15/ndvi.tif".to_string(),
            })
            .expect_err("missing CRS is rejected");

        assert_eq!(
            error,
            FarmFieldError::LayerMetadataInvalid {
                layer_id: "layer-missing-crs".to_string(),
                reason: SceneLayerMetadataError::MissingCrs
            }
        );

        let error = registry
            .insert_scene_layer(SceneLayerRecord {
                layer_id: "layer-missing-extent".to_string(),
                scene_id: "scene-2026-04-15".to_string(),
                product_type: "rgb".to_string(),
                crs: "EPSG:4326".to_string(),
                extent: None,
                resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
                uri: "s3://agbot/scenes/scene-2026-04-15/rgb.tif".to_string(),
            })
            .expect_err("missing extent is rejected");

        assert_eq!(
            error,
            FarmFieldError::LayerMetadataInvalid {
                layer_id: "layer-missing-extent".to_string(),
                reason: SceneLayerMetadataError::MissingExtent
            }
        );
        assert!(
            registry.scenes_for_field_season("org-a", "field-a", "season-2026")[0]
                .layers
                .is_empty()
        );
    }

    #[test]
    fn scene_field_coverage_reports_partial_and_no_coverage() {
        let mut registry = registry_with_field_and_season();
        registry
            .insert_scene(SceneRecord {
                scene_id: "scene-coverage".to_string(),
                field_id: "field-a".to_string(),
                season_id: "season-2026".to_string(),
                org_id: "org-a".to_string(),
                captured_at: "2026-04-15T14:30:00Z".to_string(),
                source: "landsat".to_string(),
            })
            .expect("scene persists");
        registry
            .insert_scene_layer(SceneLayerRecord {
                layer_id: "layer-partial".to_string(),
                scene_id: "scene-coverage".to_string(),
                product_type: "ndvi".to_string(),
                crs: "EPSG:4326".to_string(),
                extent: Some(GeoBounds {
                    min_lon: -96.5,
                    min_lat: 41.2,
                    max_lon: -96.35,
                    max_lat: 41.4,
                }),
                resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
                uri: "s3://agbot/scenes/scene-coverage/ndvi.tif".to_string(),
            })
            .expect("layer persists");

        let coverage = registry
            .scene_field_coverage("org-a", "field-a", "scene-coverage")
            .expect("coverage computes");
        assert_eq!(coverage.status, SceneFieldCoverageStatus::Partial);
        assert!((coverage.coverage_fraction - 0.5).abs() < 1e-9);

        registry
            .insert_scene(SceneRecord {
                scene_id: "scene-no-coverage".to_string(),
                field_id: "field-a".to_string(),
                season_id: "season-2026".to_string(),
                org_id: "org-a".to_string(),
                captured_at: "2026-04-16T14:30:00Z".to_string(),
                source: "landsat".to_string(),
            })
            .expect("scene persists");
        registry
            .insert_scene_layer(SceneLayerRecord {
                layer_id: "layer-outside".to_string(),
                scene_id: "scene-no-coverage".to_string(),
                product_type: "ndvi".to_string(),
                crs: "EPSG:4326".to_string(),
                extent: Some(GeoBounds {
                    min_lon: -97.0,
                    min_lat: 40.0,
                    max_lon: -96.9,
                    max_lat: 40.1,
                }),
                resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
                uri: "s3://agbot/scenes/scene-no-coverage/ndvi.tif".to_string(),
            })
            .expect("layer persists");

        let no_coverage = registry
            .scene_field_coverage("org-a", "field-a", "scene-no-coverage")
            .expect("coverage computes");
        assert_eq!(no_coverage.status, SceneFieldCoverageStatus::NoCoverage);
        assert_eq!(no_coverage.coverage_fraction, 0.0);
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

    fn unclosed_test_boundary() -> FieldBoundary {
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

    fn test_farm_record(
        farm_id: &str,
        org_id: &str,
        name: &str,
        status: FarmFieldEntityStatus,
    ) -> FarmRecord {
        FarmRecord {
            farm_id: farm_id.to_string(),
            org_id: org_id.to_string(),
            owner: org_id.to_string(),
            name: name.to_string(),
            notes: None,
            status,
            created_at: "2026-04-01T00:00:00Z".to_string(),
            updated_at: "2026-04-01T00:00:00Z".to_string(),
        }
    }

    fn test_field_record(
        field_id: &str,
        farm_id: &str,
        org_id: &str,
        name: &str,
        status: FarmFieldEntityStatus,
    ) -> FieldRecord {
        FieldRecord {
            farm_id: Some(farm_id.to_string()),
            field_id: field_id.to_string(),
            org_id: org_id.to_string(),
            owner: org_id.to_string(),
            name: name.to_string(),
            area_ha: None,
            crop: None,
            season: None,
            notes: None,
            boundary: test_boundary(),
            extent: test_extent(),
            status,
            created_at: "2026-04-01T00:00:00Z".to_string(),
            updated_at: "2026-04-01T00:00:00Z".to_string(),
        }
    }

    fn registry_with_field() -> FarmFieldRegistry {
        let mut registry = FarmFieldRegistry::default();
        registry
            .insert_farm(FarmRecord {
                farm_id: "farm-a".to_string(),
                org_id: "org-a".to_string(),
                owner: "org-a".to_string(),
                name: "Prairie Farm".to_string(),
                notes: None,
                status: FarmFieldEntityStatus::Active,
                created_at: "2026-04-01T00:00:00Z".to_string(),
                updated_at: "2026-04-01T00:00:00Z".to_string(),
            })
            .expect("farm persists");
        registry
            .insert_field(FieldRecord {
                farm_id: Some("farm-a".to_string()),
                field_id: "field-a".to_string(),
                org_id: "org-a".to_string(),
                owner: "org-a".to_string(),
                name: "North 80".to_string(),
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
        registry
    }

    fn registry_with_field_and_season() -> FarmFieldRegistry {
        let mut registry = registry_with_field();
        registry
            .insert_season(SeasonRecord {
                season_id: "season-2026".to_string(),
                field_id: "field-a".to_string(),
                org_id: "org-a".to_string(),
                start: "2026-03-01".to_string(),
                end: "2026-10-31".to_string(),
                label: "2026 Corn".to_string(),
            })
            .expect("season persists");
        registry
    }

    #[test]
    fn annotation_create_and_edit_appends_audit_history() {
        let mut registry = AnnotationAuditRegistry::default();
        registry
            .create_annotation(AuditedAnnotationRecord {
                annotation_id: "ann-1".to_string(),
                field_id: "field-a".to_string(),
                scene_id: Some("scene-a".to_string()),
                org_id: "org-a".to_string(),
                author_user_id: "user-author".to_string(),
                geometry: AnnotationGeometry::Point {
                    coordinate: GeoPoint {
                        longitude: -96.0,
                        latitude: 41.0,
                    },
                },
                created_at: "2026-05-01T00:00:00Z".to_string(),
                retracted_at: None,
            })
            .expect("annotation persists");

        registry
            .edit_annotation_geometry(
                "org-a",
                "ann-1",
                "user-editor",
                "2026-04-30T00:00:00Z",
                AnnotationGeometry::Polygon {
                    coordinates: vec![
                        GeoPoint {
                            longitude: -96.0,
                            latitude: 41.0,
                        },
                        GeoPoint {
                            longitude: -95.9,
                            latitude: 41.0,
                        },
                        GeoPoint {
                            longitude: -95.9,
                            latitude: 41.1,
                        },
                        GeoPoint {
                            longitude: -96.0,
                            latitude: 41.0,
                        },
                    ],
                },
            )
            .expect("annotation edit persists");

        let history = registry.annotation_history("org-a", "ann-1");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].change_type, AnnotationChangeType::Created);
        assert!(history[0].before.is_none());
        assert!(matches!(
            history[0].after.as_ref().map(|record| &record.geometry),
            Some(AnnotationGeometry::Point { .. })
        ));
        assert_eq!(history[1].change_type, AnnotationChangeType::Edited);
        assert_eq!(history[1].actor_user_id, "user-editor");
        assert!(matches!(
            history[1].before.as_ref().map(|record| &record.geometry),
            Some(AnnotationGeometry::Point { .. })
        ));
        assert!(matches!(
            history[1].after.as_ref().map(|record| &record.geometry),
            Some(AnnotationGeometry::Polygon { .. })
        ));
        assert_eq!(registry.annotations_for_org("org-a").len(), 1);
        assert!(registry.annotations_for_org("org-b").is_empty());
    }

    #[test]
    fn annotation_history_delete_is_rejected_and_retract_is_soft() {
        let mut registry = AnnotationAuditRegistry::default();
        registry
            .create_annotation(AuditedAnnotationRecord {
                annotation_id: "ann-1".to_string(),
                field_id: "field-a".to_string(),
                scene_id: None,
                org_id: "org-a".to_string(),
                author_user_id: "user-author".to_string(),
                geometry: AnnotationGeometry::Point {
                    coordinate: GeoPoint {
                        longitude: -96.0,
                        latitude: 41.0,
                    },
                },
                created_at: "2026-05-01T00:00:00Z".to_string(),
                retracted_at: None,
            })
            .expect("annotation persists");

        let error = registry
            .delete_annotation_history("org-a", "ann-1")
            .expect_err("history hard delete is rejected");

        assert_eq!(
            error,
            AnnotationPersistenceError::HistoryDeleteRejected {
                annotation_id: "ann-1".to_string()
            }
        );

        let retracted = registry
            .retract_annotation("org-a", "ann-1", "user-editor", "2026-05-03T00:00:00Z")
            .expect("soft retract persists");

        assert_eq!(
            retracted.retracted_at.as_deref(),
            Some("2026-05-03T00:00:00Z")
        );
        let history = registry.annotation_history("org-a", "ann-1");
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].change_type, AnnotationChangeType::Retracted);
    }

    #[test]
    fn annotation_record_round_trips_through_json() {
        let annotation = AnnotationRecord {
            annotation_id: "ann-1".to_string(),
            scene_id: "scene-1".to_string(),
            field_id: Some("field-1".to_string()),
            author: Some("operator-1".to_string()),
            crs: Some("EPSG:4326".to_string()),
            audit_id: Some("audit-ann-1".to_string()),
            label: "Water stress".to_string(),
            note: Some("Observed near pivot edge".to_string()),
            severity: Some("high".to_string()),
            geometry: AnnotationGeometry::Point {
                coordinate: GeoPoint {
                    longitude: -96.4,
                    latitude: 41.2,
                },
            },
            created_at: "2026-04-01T00:00:00Z".to_string(),
            updated_at: "2026-04-01T00:00:00Z".to_string(),
        };

        let value = serde_json::to_value(&annotation).expect("annotation should serialize");
        let decoded: AnnotationRecord =
            serde_json::from_value(value).expect("annotation should deserialize");

        assert_eq!(decoded, annotation);
    }

    #[test]
    fn recommendation_record_round_trips_through_json() {
        let recommendation = RecommendationRecord {
            recommendation_id: "rec-1".to_string(),
            scene_id: "scene-1".to_string(),
            field_id: Some("field-1".to_string()),
            org_id: "org-a".to_string(),
            author_user_id: "user-author".to_string(),
            title: "Inspect water stress zone".to_string(),
            note: Some("Check irrigation and re-scout in 48h".to_string()),
            category: Some("irrigation".to_string()),
            action_category: "irrigation".to_string(),
            priority: RecommendationPriority::High,
            status: RecommendationStatus::Reviewed,
            evidence_refs: vec!["zone:zone-1".to_string()],
            annotation_ids: vec!["ann-1".to_string(), "ann-2".to_string()],
            created_at: "2026-04-19T00:00:00Z".to_string(),
            updated_at: "2026-04-19T01:00:00Z".to_string(),
        };

        let value = serde_json::to_value(&recommendation).expect("recommendation should serialize");
        let decoded: RecommendationRecord =
            serde_json::from_value(value).expect("recommendation should deserialize");

        assert_eq!(decoded, recommendation);
    }

    #[test]
    fn recommendation_create_and_transitions_append_audit_history() {
        let mut registry = RecommendationLifecycleRegistry::default();
        registry
            .create_recommendation(RecommendationRecord {
                recommendation_id: "rec-1".to_string(),
                scene_id: "scene-1".to_string(),
                field_id: Some("field-1".to_string()),
                org_id: "org-a".to_string(),
                author_user_id: "user-author".to_string(),
                title: "Inspect water stress zone".to_string(),
                note: Some("Check irrigation and re-scout in 48h".to_string()),
                category: Some("irrigation".to_string()),
                action_category: "irrigation".to_string(),
                priority: RecommendationPriority::High,
                status: RecommendationStatus::Open,
                evidence_refs: vec!["zone:zone-1".to_string()],
                annotation_ids: vec!["ann-1".to_string()],
                created_at: "2026-05-01T00:00:00Z".to_string(),
                updated_at: "2026-05-01T00:00:00Z".to_string(),
            })
            .expect("recommendation persists");

        registry
            .transition_recommendation_status(
                "org-a",
                "rec-1",
                "user-reviewer",
                "2026-05-02T00:00:00Z",
                RecommendationStatus::Reviewed,
            )
            .expect("review transition persists");
        let completed = registry
            .transition_recommendation_status(
                "org-a",
                "rec-1",
                "user-reviewer",
                "2026-05-03T00:00:00Z",
                RecommendationStatus::Completed,
            )
            .expect("completion transition persists");

        assert_eq!(completed.status, RecommendationStatus::Completed);
        let history = registry.recommendation_history("org-a", "rec-1");
        assert_eq!(history.len(), 3);
        assert_eq!(
            history[0].change_type,
            RecommendationStatusChangeType::Created
        );
        assert_eq!(history[0].before, None);
        assert_eq!(history[0].after, RecommendationStatus::Open);
        assert_eq!(
            history[1].change_type,
            RecommendationStatusChangeType::StatusChanged
        );
        assert_eq!(history[1].actor_user_id, "user-reviewer");
        assert_eq!(history[1].before, Some(RecommendationStatus::Open));
        assert_eq!(history[1].after, RecommendationStatus::Reviewed);
        assert_eq!(history[2].before, Some(RecommendationStatus::Reviewed));
        assert_eq!(history[2].after, RecommendationStatus::Completed);
        assert_eq!(registry.recommendations_for_org("org-a").len(), 1);
        assert!(registry.recommendations_for_org("org-b").is_empty());
        assert!(registry.recommendation_history("org-b", "rec-1").is_empty());
    }

    #[test]
    fn recommendation_rejects_missing_evidence_and_invalid_transition() {
        let mut registry = RecommendationLifecycleRegistry::default();
        let missing_evidence = registry
            .create_recommendation(RecommendationRecord {
                recommendation_id: "rec-1".to_string(),
                scene_id: "scene-1".to_string(),
                field_id: Some("field-1".to_string()),
                org_id: "org-a".to_string(),
                author_user_id: "user-author".to_string(),
                title: "Inspect water stress zone".to_string(),
                note: None,
                category: Some("irrigation".to_string()),
                action_category: "irrigation".to_string(),
                priority: RecommendationPriority::High,
                status: RecommendationStatus::Open,
                evidence_refs: Vec::new(),
                annotation_ids: Vec::new(),
                created_at: "2026-05-01T00:00:00Z".to_string(),
                updated_at: "2026-05-01T00:00:00Z".to_string(),
            })
            .expect_err("recommendation without evidence is rejected");
        assert_eq!(
            missing_evidence,
            RecommendationPersistenceError::EvidenceRequired {
                recommendation_id: "rec-1".to_string()
            }
        );

        registry
            .create_recommendation(RecommendationRecord {
                recommendation_id: "rec-1".to_string(),
                scene_id: "scene-1".to_string(),
                field_id: Some("field-1".to_string()),
                org_id: "org-a".to_string(),
                author_user_id: "user-author".to_string(),
                title: "Inspect water stress zone".to_string(),
                note: None,
                category: Some("irrigation".to_string()),
                action_category: "irrigation".to_string(),
                priority: RecommendationPriority::High,
                status: RecommendationStatus::Open,
                evidence_refs: vec!["zone:zone-1".to_string()],
                annotation_ids: Vec::new(),
                created_at: "2026-05-01T00:00:00Z".to_string(),
                updated_at: "2026-05-01T00:00:00Z".to_string(),
            })
            .expect("recommendation persists");

        let invalid = registry
            .transition_recommendation_status(
                "org-a",
                "rec-1",
                "user-reviewer",
                "2026-05-02T00:00:00Z",
                RecommendationStatus::Completed,
            )
            .expect_err("open cannot move directly to completed");
        assert_eq!(
            invalid,
            RecommendationPersistenceError::InvalidStatusTransition {
                from: RecommendationStatus::Open,
                to: RecommendationStatus::Completed
            }
        );

        registry
            .transition_recommendation_status(
                "org-a",
                "rec-1",
                "user-reviewer",
                "2026-05-02T00:00:00Z",
                RecommendationStatus::Reviewed,
            )
            .expect("review transition persists");
        registry
            .transition_recommendation_status(
                "org-a",
                "rec-1",
                "user-reviewer",
                "2026-05-03T00:00:00Z",
                RecommendationStatus::Completed,
            )
            .expect("completion transition persists");
        let reopened = registry
            .transition_recommendation_status(
                "org-a",
                "rec-1",
                "user-reviewer",
                "2026-05-04T00:00:00Z",
                RecommendationStatus::Open,
            )
            .expect_err("completed cannot reopen");
        assert_eq!(
            reopened,
            RecommendationPersistenceError::InvalidStatusTransition {
                from: RecommendationStatus::Completed,
                to: RecommendationStatus::Open
            }
        );
    }

    #[test]
    fn work_order_from_open_recommendation_appends_lifecycle_history() {
        let mut registry = WorkOrderRegistry::default();
        let work_order = registry
            .create_work_order_from_recommendation(WorkOrderCreateRequest {
                work_order_id: "wo-1".to_string(),
                source_recommendation: Some(open_recommendation()),
                actor_user_id: "grower-1".to_string(),
                created_at: "2026-05-04T00:00:00Z".to_string(),
                assignee_user_id: None,
                due: Some("2026-05-10".to_string()),
            })
            .expect("work order persists");

        assert_eq!(work_order.work_order_id, "wo-1");
        assert_eq!(work_order.source_rec_id, "rec-1");
        assert_eq!(work_order.field_id, "field-1");
        assert_eq!(work_order.org_id, "org-a");
        assert_eq!(work_order.status, WorkOrderStatus::Created);
        assert_eq!(work_order.assignee_user_id, None);

        registry
            .assign_work_order(
                "org-a",
                "wo-1",
                "manager-1",
                "operator-1",
                "2026-05-05T00:00:00Z",
            )
            .expect("assignment persists");
        registry
            .transition_work_order_status(
                "org-a",
                "wo-1",
                "operator-1",
                "2026-05-06T00:00:00Z",
                WorkOrderStatus::InProgress,
            )
            .expect("start persists");
        let done = registry
            .transition_work_order_status(
                "org-a",
                "wo-1",
                "operator-1",
                "2026-05-07T00:00:00Z",
                WorkOrderStatus::Done,
            )
            .expect("completion persists");

        assert_eq!(done.status, WorkOrderStatus::Done);
        assert_eq!(done.assignee_user_id.as_deref(), Some("operator-1"));
        let history = registry.work_order_history("org-a", "wo-1");
        assert_eq!(history.len(), 4);
        assert_eq!(history[0].change_type, WorkOrderChangeType::Created);
        assert_eq!(history[0].before, None);
        assert_eq!(history[0].after, WorkOrderStatus::Created);
        assert_eq!(history[1].after, WorkOrderStatus::Assigned);
        assert_eq!(history[2].after, WorkOrderStatus::InProgress);
        assert_eq!(history[3].after, WorkOrderStatus::Done);
        assert_eq!(registry.work_orders_for_org("org-a").len(), 1);
        assert!(registry.work_orders_for_org("org-b").is_empty());
    }

    #[test]
    fn work_order_queue_scopes_by_operator_org_and_status() {
        let mut registry = WorkOrderRegistry::default();
        create_assigned_work_order(
            &mut registry,
            "wo-1",
            "rec-1",
            "org-a",
            "field-1",
            "operator-1",
            "2026-05-10",
        );
        create_assigned_work_order(
            &mut registry,
            "wo-2",
            "rec-2",
            "org-a",
            "field-1",
            "operator-2",
            "2026-05-08",
        );
        create_assigned_work_order(
            &mut registry,
            "wo-3",
            "rec-3",
            "org-b",
            "field-2",
            "operator-1",
            "2026-05-06",
        );

        let queue = registry.operator_work_order_queue(WorkOrderQueueQuery {
            org_id: "org-a".to_string(),
            assignee_user_id: "operator-1".to_string(),
            statuses: vec![WorkOrderStatus::Assigned],
        });

        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].work_order_id, "wo-1");
        assert_eq!(queue[0].org_id, "org-a");
        assert_eq!(queue[0].assignee_user_id.as_deref(), Some("operator-1"));
        assert_eq!(queue[0].status, WorkOrderStatus::Assigned);
    }

    #[test]
    fn work_order_reassignment_rejects_cross_org_assignee_and_audits() {
        let mut registry = WorkOrderRegistry::default();
        create_assigned_work_order(
            &mut registry,
            "wo-1",
            "rec-1",
            "org-a",
            "field-1",
            "operator-1",
            "2026-05-10",
        );

        let error = registry
            .reassign_work_order(
                "org-a",
                "wo-1",
                "manager-1",
                "operator-foreign",
                "org-b",
                "2026-05-05T12:00:00Z",
            )
            .expect_err("cross-org reassignment is rejected");

        assert_eq!(
            error,
            WorkOrderPersistenceError::AssigneeOrgMismatch {
                assignee_user_id: "operator-foreign".to_string(),
                expected_org_id: "org-a".to_string(),
                actual_org_id: "org-b".to_string(),
            }
        );
        let queue = registry.operator_work_order_queue(WorkOrderQueueQuery {
            org_id: "org-a".to_string(),
            assignee_user_id: "operator-1".to_string(),
            statuses: vec![WorkOrderStatus::Assigned],
        });
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].assignee_user_id.as_deref(), Some("operator-1"));

        let history = registry.work_order_history("org-a", "wo-1");
        assert_eq!(history.len(), 3);
        assert_eq!(history[2].change_type, WorkOrderChangeType::Reassigned);
        assert_eq!(history[2].actor_user_id, "manager-1");
    }

    #[test]
    fn work_order_without_source_recommendation_is_rejected() {
        let mut registry = WorkOrderRegistry::default();
        let error = registry
            .create_work_order_from_recommendation(WorkOrderCreateRequest {
                work_order_id: "wo-1".to_string(),
                source_recommendation: None,
                actor_user_id: "grower-1".to_string(),
                created_at: "2026-05-04T00:00:00Z".to_string(),
                assignee_user_id: None,
                due: None,
            })
            .expect_err("work order requires recommendation");

        assert_eq!(
            error,
            WorkOrderPersistenceError::MissingSourceRecommendation {
                work_order_id: "wo-1".to_string()
            }
        );
        assert!(registry.work_orders_for_org("org-a").is_empty());
    }

    #[test]
    fn work_order_invalid_transition_is_rejected() {
        let mut registry = WorkOrderRegistry::default();
        registry
            .create_work_order_from_recommendation(WorkOrderCreateRequest {
                work_order_id: "wo-1".to_string(),
                source_recommendation: Some(open_recommendation()),
                actor_user_id: "grower-1".to_string(),
                created_at: "2026-05-04T00:00:00Z".to_string(),
                assignee_user_id: None,
                due: None,
            })
            .expect("work order persists");

        let error = registry
            .transition_work_order_status(
                "org-a",
                "wo-1",
                "operator-1",
                "2026-05-06T00:00:00Z",
                WorkOrderStatus::Done,
            )
            .expect_err("created cannot move directly to done");

        assert_eq!(
            error,
            WorkOrderPersistenceError::InvalidStatusTransition {
                from: WorkOrderStatus::Created,
                to: WorkOrderStatus::Done
            }
        );
    }

    fn create_assigned_work_order(
        registry: &mut WorkOrderRegistry,
        work_order_id: &str,
        recommendation_id: &str,
        org_id: &str,
        field_id: &str,
        assignee_user_id: &str,
        due: &str,
    ) -> WorkOrderRecord {
        let mut recommendation = open_recommendation();
        recommendation.recommendation_id = recommendation_id.to_string();
        recommendation.org_id = org_id.to_string();
        recommendation.field_id = Some(field_id.to_string());
        registry
            .create_work_order_from_recommendation(WorkOrderCreateRequest {
                work_order_id: work_order_id.to_string(),
                source_recommendation: Some(recommendation),
                actor_user_id: "manager-1".to_string(),
                created_at: "2026-05-04T00:00:00Z".to_string(),
                assignee_user_id: None,
                due: Some(due.to_string()),
            })
            .expect("work order persists");
        registry
            .assign_work_order(
                org_id,
                work_order_id,
                "manager-1",
                assignee_user_id,
                "2026-05-05T00:00:00Z",
            )
            .expect("assignment persists")
    }

    fn open_recommendation() -> RecommendationRecord {
        RecommendationRecord {
            recommendation_id: "rec-1".to_string(),
            scene_id: "scene-1".to_string(),
            field_id: Some("field-1".to_string()),
            org_id: "org-a".to_string(),
            author_user_id: "advisor-1".to_string(),
            title: "Scout anomaly zone zone-1".to_string(),
            note: Some("Check irrigation and re-scout in 48h".to_string()),
            category: Some("scout".to_string()),
            action_category: "scout".to_string(),
            priority: RecommendationPriority::High,
            status: RecommendationStatus::Open,
            evidence_refs: vec!["zone:zone-1".to_string()],
            annotation_ids: Vec::new(),
            created_at: "2026-05-01T00:00:00Z".to_string(),
            updated_at: "2026-05-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn report_deliverable_links_sources_and_lists_by_field_season() {
        let mut registry = ReportDeliverableRegistry::default();
        let report = registry
            .create_report(report_record(vec![
                "scene:scene-1".to_string(),
                "finding:finding-1".to_string(),
                "recommendation:rec-1".to_string(),
            ]))
            .expect("report persists");

        assert_eq!(report.report_id, "report-1");
        assert_eq!(report.field_id.as_deref(), Some("field-1"));
        assert_eq!(report.season_id.as_deref(), Some("season-2026"));
        assert_eq!(report.org_id, "org-a");
        assert_eq!(report.generated_by, "advisor-1");
        assert_eq!(report.artifact_uri, "s3://reports/report-1.pdf");
        assert_eq!(report.visibility, ReportVisibility::Org);
        assert_eq!(
            report.source_refs,
            vec![
                "scene:scene-1".to_string(),
                "finding:finding-1".to_string(),
                "recommendation:rec-1".to_string()
            ]
        );

        let reports = registry.reports_for_field_season("org-a", "field-1", "season-2026");
        assert_eq!(reports, vec![report]);
        assert!(registry
            .reports_for_field_season("org-b", "field-1", "season-2026")
            .is_empty());
    }

    #[test]
    fn report_deliverable_without_source_refs_is_rejected() {
        let mut registry = ReportDeliverableRegistry::default();
        let error = registry
            .create_report(report_record(Vec::new()))
            .expect_err("orphan report is rejected");

        assert_eq!(
            error,
            ReportPersistenceError::MissingSourceRefs {
                report_id: "report-1".to_string()
            }
        );
        assert!(registry
            .reports_for_field_season("org-a", "field-1", "season-2026")
            .is_empty());
    }

    fn report_record(source_refs: Vec<String>) -> ReportRecord {
        ReportRecord {
            report_id: "report-1".to_string(),
            scene_id: "scene-1".to_string(),
            field_id: Some("field-1".to_string()),
            season_id: Some("season-2026".to_string()),
            org_id: "org-a".to_string(),
            generated_by: "advisor-1".to_string(),
            source_refs,
            title: "North Field report".to_string(),
            format: ReportFormat::Html,
            artifact_path: "/tmp/report-1.html".to_string(),
            artifact_uri: "s3://reports/report-1.pdf".to_string(),
            download_url: "/api/scenes/scene-1/reports/report-1".to_string(),
            visibility: ReportVisibility::Org,
            annotation_count: 3,
            recommendation_count: 2,
            created_at: "2026-04-19T02:00:00Z".to_string(),
        }
    }

    #[test]
    fn report_record_round_trips_through_json() {
        let report = ReportRecord {
            report_id: "report-1".to_string(),
            scene_id: "scene-1".to_string(),
            field_id: Some("field-1".to_string()),
            season_id: Some("season-2026".to_string()),
            org_id: "org-a".to_string(),
            generated_by: "advisor-1".to_string(),
            source_refs: vec!["scene:scene-1".to_string()],
            title: "Scene 1 agronomy report".to_string(),
            format: ReportFormat::Html,
            artifact_path: "/tmp/report-1.html".to_string(),
            artifact_uri: "s3://reports/report-1.pdf".to_string(),
            download_url: "/api/scenes/scene-1/reports/report-1".to_string(),
            visibility: ReportVisibility::Org,
            annotation_count: 3,
            recommendation_count: 2,
            created_at: "2026-04-19T02:00:00Z".to_string(),
        };

        let value = serde_json::to_value(&report).expect("report should serialize");
        let decoded: ReportRecord =
            serde_json::from_value(value).expect("report should deserialize");

        assert_eq!(decoded, report);
    }
}
