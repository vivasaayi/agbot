use chrono::{Datelike, NaiveDate};
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
pub struct SeasonCropPlanRolloverSuggestion {
    pub field_id: String,
    pub org_id: String,
    #[serde(default)]
    pub source_history_refs: Vec<String>,
    pub requires_approval: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_season: Option<SeasonRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_crop_plan: Option<CropPlanRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_basis_reason: Option<String>,
}

impl SeasonCropPlanRolloverSuggestion {
    pub fn has_proposal(&self) -> bool {
        self.proposed_season.is_some() || self.proposed_crop_plan.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessAuditDecision {
    Allowed,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessAuditEvent {
    pub audit_id: String,
    pub actor_id: String,
    pub org_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_org_id: Option<String>,
    pub action: String,
    pub decision: AccessAuditDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    pub at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessAnomalyThresholds {
    pub denied_cross_org_attempts: usize,
    pub bulk_export_count: usize,
}

impl Default for AccessAnomalyThresholds {
    fn default() -> Self {
        Self {
            denied_cross_org_attempts: 3,
            bulk_export_count: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessAnomalySignal {
    CrossOrgProbe,
    BulkExport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessAnomalyAdvisory {
    pub actor_id: String,
    pub signal: AccessAnomalySignal,
    pub observed_count: usize,
    pub threshold: usize,
    #[serde(default)]
    pub evidence_audit_ids: Vec<String>,
    pub requires_approval: bool,
    pub auto_blocked: bool,
    pub summary: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TractorGuidancePoint {
    pub x_m: f64,
    pub y_m: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TractorGuidancePath {
    pub start: TractorGuidancePoint,
    pub end: TractorGuidancePoint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorGuidanceConfig {
    pub runtime_mode: String,
    pub max_cross_track_error_m: f64,
    pub correction_gain: f64,
    pub advance_m_per_tick: f64,
    pub max_ticks: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorGuidanceTelemetry {
    pub tick: usize,
    pub position: TractorGuidancePoint,
    pub cross_track_error_m: f64,
    pub halted: bool,
    #[serde(default)]
    pub fault: Option<TractorGuidanceFault>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorGuidanceRunResult {
    pub runtime_mode: String,
    pub halted: bool,
    #[serde(default)]
    pub fault: Option<TractorGuidanceFault>,
    pub max_observed_cross_track_error_m: f64,
    pub telemetry: Vec<TractorGuidanceTelemetry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TractorGuidanceFault {
    CrossTrackErrorExceeded,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TractorGuidanceError {
    #[error("tractor guidance only runs in simulation mode")]
    RuntimeModeNotSimulation { runtime_mode: String },
    #[error("tractor guidance path length must be positive")]
    InvalidPath,
    #[error("tractor guidance max cross-track error must be positive")]
    InvalidCrossTrackBound,
    #[error("tractor guidance correction gain must be finite and between 0 and 1")]
    InvalidCorrectionGain,
    #[error("tractor guidance tick advance must be positive")]
    InvalidTickAdvance,
    #[error("tractor guidance max_ticks must be positive")]
    InvalidMaxTicks,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorSwathCoverageRequest {
    pub field_boundary: FieldBoundary,
    #[serde(default)]
    pub exclusion_boundaries: Vec<FieldBoundary>,
    pub implement_width_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorSwathSegment {
    pub start: GeoPoint,
    pub end: GeoPoint,
    pub width_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorSwathCoveragePlan {
    pub crs: String,
    pub swaths: Vec<TractorSwathSegment>,
    pub coverage_fraction: f64,
    pub all_swaths_inside_boundary: bool,
    pub avoided_exclusions: bool,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TractorSwathPlanningError {
    #[error(transparent)]
    InvalidBoundary(#[from] FieldBoundaryValidationError),
    #[error("tractor swath implement width must be positive")]
    InvalidImplementWidth,
    #[error("tractor swath exclusion CRS mismatch: {exclusion_crs} != {field_crs}")]
    ExclusionCrsMismatch {
        field_crs: String,
        exclusion_crs: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorFieldOpsTelemetrySample {
    pub timestamp: String,
    pub position: TractorGuidancePoint,
    pub speed_mps: f64,
    pub implement_enabled: bool,
    pub implement_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorFieldOpsCoverageTally {
    pub distance_m: f64,
    pub covered_area_m2: f64,
    pub coverage_fraction: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TractorFieldOpsSafetyEventType {
    TelemetryDropout,
    ManualEstop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorFieldOpsSafetyEvent {
    pub event_type: TractorFieldOpsSafetyEventType,
    pub at: String,
    pub reason_code: String,
    pub details: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorFieldOpsSessionLog {
    pub session_id: String,
    pub tractor_id: String,
    pub field_id: String,
    pub started_at: String,
    pub telemetry: Vec<TractorFieldOpsTelemetrySample>,
    pub coverage: TractorFieldOpsCoverageTally,
    pub safety_events: Vec<TractorFieldOpsSafetyEvent>,
    pub telemetry_gap_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorFieldOpsSessionRequest {
    pub session_id: String,
    pub tractor_id: String,
    pub field_id: String,
    pub started_at: String,
    pub telemetry: Vec<TractorFieldOpsTelemetrySample>,
    pub implement_width_m: f64,
    pub planned_area_m2: f64,
    pub max_telemetry_gap_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TractorFieldOpsSessionError {
    #[error("tractor field-ops session_id cannot be empty")]
    EmptySessionId,
    #[error("tractor field-ops tractor_id cannot be empty")]
    EmptyTractorId,
    #[error("tractor field-ops field_id cannot be empty")]
    EmptyFieldId,
    #[error("tractor field-ops started_at cannot be empty")]
    EmptyStartedAt,
    #[error("tractor field-ops requires at least one telemetry sample")]
    EmptyTelemetry,
    #[error("tractor field-ops implement width must be positive")]
    InvalidImplementWidth,
    #[error("tractor field-ops planned area must be positive")]
    InvalidPlannedArea,
    #[error("tractor field-ops telemetry gap threshold must be positive")]
    InvalidTelemetryGapThreshold,
    #[error("tractor field-ops timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TractorFieldOpsReplayFrameType {
    Telemetry,
    SafetyEvent,
    TelemetryGap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorFieldOpsReplayFrame {
    pub at: String,
    pub frame_type: TractorFieldOpsReplayFrameType,
    #[serde(default)]
    pub telemetry: Option<TractorFieldOpsTelemetrySample>,
    #[serde(default)]
    pub safety_event: Option<TractorFieldOpsSafetyEvent>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorFieldOpsReplay {
    pub session_id: String,
    pub tractor_id: String,
    pub field_id: String,
    pub read_only: bool,
    pub frames: Vec<TractorFieldOpsReplayFrame>,
    pub gap_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorGeofenceEvaluationRequest {
    pub tractor_id: String,
    pub field_id: String,
    pub boundary_ref: String,
    pub boundary: FieldBoundary,
    pub current_position: GeoPoint,
    pub predicted_position: GeoPoint,
    pub position_crs: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TractorGeofenceDecision {
    Permitted,
    Halted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorGeofenceEvaluation {
    pub tractor_id: String,
    pub field_id: String,
    pub boundary_ref: String,
    pub decision: TractorGeofenceDecision,
    pub reason_code: String,
    pub position: GeoPoint,
    pub predicted_position: GeoPoint,
    pub boundary_crs: String,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TractorGeofenceError {
    #[error(transparent)]
    InvalidBoundary(#[from] FieldBoundaryValidationError),
    #[error("tractor geofence tractor_id cannot be empty")]
    EmptyTractorId,
    #[error("tractor geofence field_id cannot be empty")]
    EmptyFieldId,
    #[error("tractor geofence boundary_ref cannot be empty")]
    EmptyBoundaryRef,
    #[error("tractor geofence position CRS cannot be empty")]
    EmptyPositionCrs,
    #[error("tractor geofence CRS mismatch: position {position_crs} != boundary {boundary_crs}")]
    CrsMismatch {
        position_crs: String,
        boundary_crs: String,
    },
    #[error("tractor geofence position contains invalid coordinates")]
    InvalidPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TractorOperatorApproval {
    pub approval_id: String,
    pub tractor_id: String,
    pub approved_by: String,
    pub approved_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TractorEstopState {
    pub tractor_id: String,
    pub active: bool,
    pub triggered_by: Option<String>,
    pub triggered_at: Option<String>,
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TractorMotionGateDecision {
    Allowed,
    Refused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TractorMotionGateAudit {
    pub tractor_id: String,
    pub command_id: Option<String>,
    pub decision: TractorMotionGateDecision,
    pub reason_code: String,
    pub actor: Option<String>,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TractorMotionGateEvaluation {
    pub tractor_id: String,
    pub command_id: Option<String>,
    pub decision: TractorMotionGateDecision,
    pub halted: bool,
    pub approval_id: Option<String>,
    pub audit: TractorMotionGateAudit,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TractorMotionGateError {
    #[error("tractor motion gate tractor_id cannot be empty")]
    EmptyTractorId,
    #[error("tractor motion gate timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("tractor motion gate approval timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorObstacleDetectionRequest {
    pub tractor_id: String,
    pub path: TractorGuidancePath,
    pub current_position: TractorGuidancePoint,
    pub obstacles: Vec<TractorGuidancePoint>,
    pub path_width_m: f64,
    pub stopping_distance_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorObstacleEvent {
    pub distance_m: f64,
    pub position: TractorGuidancePoint,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorObstacleDetection {
    pub tractor_id: String,
    pub halted: bool,
    pub event: Option<TractorObstacleEvent>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TractorObstacleDetectionError {
    #[error("tractor obstacle detector tractor_id cannot be empty")]
    EmptyTractorId,
    #[error("tractor obstacle path width must be positive")]
    InvalidPathWidth,
    #[error("tractor obstacle stopping distance must be positive")]
    InvalidStoppingDistance,
    #[error(transparent)]
    Guidance(#[from] TractorGuidanceError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorPrescriptionZone {
    pub zone_id: String,
    pub crs: String,
    pub extent: GeoBounds,
    pub rate: f64,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorPrescriptionExecutionRequest {
    pub runtime_mode: String,
    pub field_id: String,
    pub field_crs: String,
    pub field_extent: GeoBounds,
    pub zones: Vec<TractorPrescriptionZone>,
    pub geofence: TractorGeofenceEvaluation,
    pub motion_gate: TractorMotionGateEvaluation,
    pub obstacle: TractorObstacleDetection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorPrescriptionAppliedRate {
    pub zone_id: String,
    pub rate: f64,
    pub reason_code: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorPrescriptionExecutionLog {
    pub field_id: String,
    pub runtime_mode: String,
    pub applied_rates: Vec<TractorPrescriptionAppliedRate>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TractorPrescriptionExecutionError {
    #[error("tractor prescription execution only runs in simulation mode")]
    RuntimeModeNotSimulation { runtime_mode: String },
    #[error("tractor prescription field_id cannot be empty")]
    EmptyFieldId,
    #[error("tractor prescription field CRS cannot be empty")]
    EmptyFieldCrs,
    #[error("tractor prescription requires at least one zone")]
    EmptyZones,
    #[error("tractor prescription zone {zone_id} CRS mismatch: {zone_crs} != {field_crs}")]
    ZoneCrsMismatch {
        zone_id: String,
        field_crs: String,
        zone_crs: String,
    },
    #[error("tractor prescription zone {zone_id} extent is outside field extent")]
    ZoneExtentMismatch { zone_id: String },
    #[error("tractor prescription zone {zone_id} rate is invalid")]
    InvalidRate { zone_id: String },
    #[error("tractor prescription blocked by safety prerequisite: {reason_code}")]
    SafetyPrerequisiteFailed { reason_code: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TractorImplementCommand {
    Enable,
    Disable,
    SetRate { rate: f64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorImplementAdapterSpec {
    pub implement_id: String,
    pub implement_type: String,
    pub min_rate: f64,
    pub max_rate: f64,
    pub default_rate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorImplementState {
    pub implement_id: String,
    pub enabled: bool,
    pub current_rate: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TractorImplementDecision {
    Applied,
    Refused,
    ForcedOff,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorImplementSetpointLog {
    pub implement_id: String,
    pub command: TractorImplementCommand,
    pub decision: TractorImplementDecision,
    pub requested_rate: Option<f64>,
    pub applied_rate: Option<f64>,
    pub reason_code: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorImplementAdapterResult {
    pub state: TractorImplementState,
    pub log: TractorImplementSetpointLog,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldOperationalWindow {
    pub field_id: String,
    pub source: String,
    pub fetched_at: String,
    pub valid_from: String,
    pub valid_until: String,
    pub allowed: bool,
    pub reason_code: String,
    #[serde(default)]
    pub gating_inputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorWeatherWindowGateRequest {
    pub field_id: String,
    pub requested_start_at: String,
    pub max_window_age_seconds: i64,
    #[serde(default)]
    pub window: Option<FieldOperationalWindow>,
    pub motion_gate: TractorMotionGateEvaluation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TractorWeatherWindowDecision {
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorWeatherWindowGate {
    pub field_id: String,
    pub decision: TractorWeatherWindowDecision,
    pub reason_code: String,
    pub requested_start_at: String,
    #[serde(default)]
    pub window_source: Option<String>,
    pub gating_inputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorSwathReservation {
    pub tractor_id: String,
    pub swath: TractorSwathSegment,
    pub priority: u8,
    pub starts_at: String,
    pub ends_at: String,
    pub geofence: TractorGeofenceEvaluation,
    pub motion_gate: TractorMotionGateEvaluation,
    pub obstacle: TractorObstacleDetection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TractorDeconflictionRequest {
    pub field_id: String,
    pub evaluated_at: String,
    pub reservations: Vec<TractorSwathReservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TractorDeconflictionDecision {
    Proceed,
    Halted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TractorDeconflictionReservationDecision {
    pub tractor_id: String,
    pub decision: TractorDeconflictionDecision,
    pub reason_code: String,
    #[serde(default)]
    pub conflict_with: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TractorDeconflictionEvent {
    pub halted_tractor_id: String,
    pub priority_tractor_id: String,
    pub reason_code: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TractorDeconflictionPlan {
    pub field_id: String,
    pub all_clear: bool,
    pub decisions: Vec<TractorDeconflictionReservationDecision>,
    pub events: Vec<TractorDeconflictionEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TractorImplementControlError {
    #[error("tractor implement control implement_id cannot be empty")]
    EmptyImplementId,
    #[error("tractor implement control implement_type cannot be empty")]
    EmptyImplementType,
    #[error("tractor implement control rate bounds are invalid")]
    InvalidRateBounds,
    #[error("tractor implement control timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("tractor implement control timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TractorDeconflictionError {
    #[error("tractor deconfliction field_id cannot be empty")]
    EmptyFieldId,
    #[error("tractor deconfliction evaluated_at cannot be empty")]
    EmptyEvaluatedAt,
    #[error("tractor deconfliction requires at least one reservation")]
    EmptyReservations,
    #[error("tractor deconfliction tractor_id cannot be empty")]
    EmptyTractorId,
    #[error("tractor deconfliction swath is invalid")]
    InvalidSwath,
    #[error("tractor deconfliction reservation timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TractorWeatherWindowGateError {
    #[error("tractor weather window field_id cannot be empty")]
    EmptyFieldId,
    #[error("tractor weather window requested_start_at cannot be empty")]
    EmptyRequestedStartAt,
    #[error("tractor weather window max age must be positive")]
    InvalidMaxWindowAge,
    #[error("tractor weather window timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
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

pub fn tractor_cross_track_error_m(
    path: TractorGuidancePath,
    point: TractorGuidancePoint,
) -> Result<f64, TractorGuidanceError> {
    let (unit_x, unit_y, _length) = tractor_guidance_unit_vector(path)?;
    let dx = point.x_m - path.start.x_m;
    let dy = point.y_m - path.start.y_m;
    Ok(dx * unit_y - dy * unit_x)
}

pub fn run_tractor_straight_path_guidance(
    path: TractorGuidancePath,
    initial_position: TractorGuidancePoint,
    disturbances: &[TractorGuidancePoint],
    config: TractorGuidanceConfig,
) -> Result<TractorGuidanceRunResult, TractorGuidanceError> {
    validate_tractor_guidance_config(&config)?;
    let (unit_x, unit_y, path_length) = tractor_guidance_unit_vector(path)?;
    let normal_x = -unit_y;
    let normal_y = unit_x;
    let mut position = initial_position;
    let mut telemetry = Vec::new();
    let mut max_observed_cross_track_error_m = 0.0_f64;
    let mut halted = false;
    let mut fault = None;

    for tick in 0..config.max_ticks {
        position.x_m += unit_x * config.advance_m_per_tick;
        position.y_m += unit_y * config.advance_m_per_tick;
        if let Some(disturbance) = disturbances.get(tick) {
            position.x_m += disturbance.x_m;
            position.y_m += disturbance.y_m;
        }

        let error = tractor_cross_track_error_m(path, position)?;
        position.x_m += normal_x * error * config.correction_gain;
        position.y_m += normal_y * error * config.correction_gain;
        let corrected_error = tractor_cross_track_error_m(path, position)?;
        let abs_error = corrected_error.abs();
        max_observed_cross_track_error_m = max_observed_cross_track_error_m.max(abs_error);
        if abs_error > config.max_cross_track_error_m {
            halted = true;
            fault = Some(TractorGuidanceFault::CrossTrackErrorExceeded);
        }

        telemetry.push(TractorGuidanceTelemetry {
            tick,
            position,
            cross_track_error_m: corrected_error,
            halted,
            fault,
        });

        let along_track_m =
            (position.x_m - path.start.x_m) * unit_x + (position.y_m - path.start.y_m) * unit_y;
        if halted || along_track_m >= path_length {
            break;
        }
    }

    Ok(TractorGuidanceRunResult {
        runtime_mode: "simulation".to_string(),
        halted,
        fault,
        max_observed_cross_track_error_m,
        telemetry,
    })
}

fn validate_tractor_guidance_config(
    config: &TractorGuidanceConfig,
) -> Result<(), TractorGuidanceError> {
    if !config.runtime_mode.eq_ignore_ascii_case("simulation") {
        return Err(TractorGuidanceError::RuntimeModeNotSimulation {
            runtime_mode: config.runtime_mode.clone(),
        });
    }
    if !config.max_cross_track_error_m.is_finite() || config.max_cross_track_error_m <= 0.0 {
        return Err(TractorGuidanceError::InvalidCrossTrackBound);
    }
    if !config.correction_gain.is_finite()
        || config.correction_gain < 0.0
        || config.correction_gain > 1.0
    {
        return Err(TractorGuidanceError::InvalidCorrectionGain);
    }
    if !config.advance_m_per_tick.is_finite() || config.advance_m_per_tick <= 0.0 {
        return Err(TractorGuidanceError::InvalidTickAdvance);
    }
    if config.max_ticks == 0 {
        return Err(TractorGuidanceError::InvalidMaxTicks);
    }
    Ok(())
}

fn tractor_guidance_unit_vector(
    path: TractorGuidancePath,
) -> Result<(f64, f64, f64), TractorGuidanceError> {
    let dx = path.end.x_m - path.start.x_m;
    let dy = path.end.y_m - path.start.y_m;
    let length = (dx * dx + dy * dy).sqrt();
    if !length.is_finite() || length <= 0.0 {
        return Err(TractorGuidanceError::InvalidPath);
    }
    Ok((dx / length, dy / length, length))
}

pub fn plan_tractor_swath_coverage(
    request: TractorSwathCoverageRequest,
) -> Result<TractorSwathCoveragePlan, TractorSwathPlanningError> {
    if !request.implement_width_m.is_finite() || request.implement_width_m <= 0.0 {
        return Err(TractorSwathPlanningError::InvalidImplementWidth);
    }
    let validated = validate_field_boundary(&request.field_boundary)?;
    let field_crs = validated
        .boundary
        .crs
        .clone()
        .ok_or(FieldBoundaryValidationError::MissingCrs)?;
    let field_bounds = validated.extent;
    let mut exclusion_bounds = Vec::new();
    for exclusion in &request.exclusion_boundaries {
        let exclusion_crs = exclusion
            .crs
            .clone()
            .ok_or(FieldBoundaryValidationError::MissingCrs)?;
        if exclusion_crs != field_crs {
            return Err(TractorSwathPlanningError::ExclusionCrsMismatch {
                field_crs,
                exclusion_crs,
            });
        }
        let validated_exclusion = validate_field_boundary(exclusion)?;
        exclusion_bounds.push(validated_exclusion.extent);
    }

    let mut swaths = Vec::new();
    let mut y = field_bounds.min_lat + request.implement_width_m / 2.0;
    while y < field_bounds.max_lat {
        let row_segments = tractor_swath_row_segments(&field_bounds, &exclusion_bounds, y);
        for (start_lon, end_lon) in row_segments {
            if end_lon > start_lon {
                swaths.push(TractorSwathSegment {
                    start: GeoPoint {
                        longitude: start_lon,
                        latitude: y,
                    },
                    end: GeoPoint {
                        longitude: end_lon,
                        latitude: y,
                    },
                    width_m: request.implement_width_m,
                });
            }
        }
        y += request.implement_width_m;
    }

    let covered_area = swaths
        .iter()
        .map(|swath| {
            (swath.end.longitude - swath.start.longitude).abs() * request.implement_width_m
        })
        .sum::<f64>();
    let field_area = (field_bounds.max_lon - field_bounds.min_lon).abs()
        * (field_bounds.max_lat - field_bounds.min_lat).abs();
    let coverage_fraction = if field_area > 0.0 {
        (covered_area / field_area).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let all_swaths_inside_boundary = swaths.iter().all(|swath| {
        tractor_point_inside_bounds(&swath.start, &field_bounds)
            && tractor_point_inside_bounds(&swath.end, &field_bounds)
    });
    let avoided_exclusions = swaths.iter().all(|swath| {
        exclusion_bounds
            .iter()
            .all(|exclusion| !tractor_swath_intersects_bounds(swath, exclusion))
    });

    Ok(TractorSwathCoveragePlan {
        crs: field_crs,
        swaths,
        coverage_fraction,
        all_swaths_inside_boundary,
        avoided_exclusions,
    })
}

fn tractor_swath_row_segments(
    field_bounds: &GeoBounds,
    exclusion_bounds: &[GeoBounds],
    y: f64,
) -> Vec<(f64, f64)> {
    let mut segments = vec![(field_bounds.min_lon, field_bounds.max_lon)];
    for exclusion in exclusion_bounds
        .iter()
        .filter(|bounds| y >= bounds.min_lat && y <= bounds.max_lat)
    {
        let mut next = Vec::new();
        for (start, end) in segments {
            if exclusion.max_lon <= start || exclusion.min_lon >= end {
                next.push((start, end));
                continue;
            }
            if exclusion.min_lon > start {
                next.push((start, exclusion.min_lon));
            }
            if exclusion.max_lon < end {
                next.push((exclusion.max_lon, end));
            }
        }
        segments = next;
    }
    segments
}

fn tractor_point_inside_bounds(point: &GeoPoint, bounds: &GeoBounds) -> bool {
    point.longitude >= bounds.min_lon
        && point.longitude <= bounds.max_lon
        && point.latitude >= bounds.min_lat
        && point.latitude <= bounds.max_lat
}

fn tractor_swath_intersects_bounds(swath: &TractorSwathSegment, bounds: &GeoBounds) -> bool {
    swath.start.latitude >= bounds.min_lat
        && swath.start.latitude <= bounds.max_lat
        && swath.start.longitude < bounds.max_lon
        && swath.end.longitude > bounds.min_lon
}

pub fn build_tractor_field_ops_session_log(
    request: TractorFieldOpsSessionRequest,
) -> Result<TractorFieldOpsSessionLog, TractorFieldOpsSessionError> {
    let session_id = normalize_tractor_text(request.session_id)
        .ok_or(TractorFieldOpsSessionError::EmptySessionId)?;
    let tractor_id = normalize_tractor_text(request.tractor_id)
        .ok_or(TractorFieldOpsSessionError::EmptyTractorId)?;
    let field_id = normalize_tractor_text(request.field_id)
        .ok_or(TractorFieldOpsSessionError::EmptyFieldId)?;
    let started_at = normalize_tractor_text(request.started_at)
        .ok_or(TractorFieldOpsSessionError::EmptyStartedAt)?;
    if request.telemetry.is_empty() {
        return Err(TractorFieldOpsSessionError::EmptyTelemetry);
    }
    if !request.implement_width_m.is_finite() || request.implement_width_m <= 0.0 {
        return Err(TractorFieldOpsSessionError::InvalidImplementWidth);
    }
    if !request.planned_area_m2.is_finite() || request.planned_area_m2 <= 0.0 {
        return Err(TractorFieldOpsSessionError::InvalidPlannedArea);
    }
    if request.max_telemetry_gap_seconds <= 0 {
        return Err(TractorFieldOpsSessionError::InvalidTelemetryGapThreshold);
    }

    let mut telemetry = request.telemetry;
    for sample in &mut telemetry {
        sample.timestamp = normalize_tractor_text(sample.timestamp.clone()).ok_or_else(|| {
            TractorFieldOpsSessionError::InvalidTimestamp {
                timestamp: sample.timestamp.clone(),
            }
        })?;
        parse_tractor_field_ops_timestamp(&sample.timestamp)?;
    }
    telemetry.sort_by(|left, right| left.timestamp.cmp(&right.timestamp));

    let distance_m = tractor_field_ops_distance_m(&telemetry);
    let covered_area_m2 = distance_m * request.implement_width_m;
    let coverage_fraction = (covered_area_m2 / request.planned_area_m2).clamp(0.0, 1.0);
    let safety_events =
        tractor_field_ops_gap_events(&telemetry, request.max_telemetry_gap_seconds)?;

    Ok(TractorFieldOpsSessionLog {
        session_id,
        tractor_id,
        field_id,
        started_at,
        telemetry,
        coverage: TractorFieldOpsCoverageTally {
            distance_m,
            covered_area_m2,
            coverage_fraction,
        },
        telemetry_gap_count: safety_events.len(),
        safety_events,
    })
}

fn tractor_field_ops_distance_m(samples: &[TractorFieldOpsTelemetrySample]) -> f64 {
    samples
        .windows(2)
        .map(|window| {
            let dx = window[1].position.x_m - window[0].position.x_m;
            let dy = window[1].position.y_m - window[0].position.y_m;
            (dx * dx + dy * dy).sqrt()
        })
        .sum()
}

fn tractor_field_ops_gap_events(
    samples: &[TractorFieldOpsTelemetrySample],
    max_gap_seconds: i64,
) -> Result<Vec<TractorFieldOpsSafetyEvent>, TractorFieldOpsSessionError> {
    let mut events = Vec::new();
    for window in samples.windows(2) {
        let previous = parse_tractor_field_ops_timestamp(&window[0].timestamp)?;
        let current = parse_tractor_field_ops_timestamp(&window[1].timestamp)?;
        let gap_seconds = current.signed_duration_since(previous).num_seconds();
        if gap_seconds > max_gap_seconds {
            events.push(TractorFieldOpsSafetyEvent {
                event_type: TractorFieldOpsSafetyEventType::TelemetryDropout,
                at: window[1].timestamp.clone(),
                reason_code: "telemetry_dropout".to_string(),
                details: format!(
                    "telemetry gap {}s exceeded threshold {}s after {}",
                    gap_seconds, max_gap_seconds, window[0].timestamp
                ),
            });
        }
    }
    Ok(events)
}

fn parse_tractor_field_ops_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, TractorFieldOpsSessionError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| TractorFieldOpsSessionError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn build_tractor_field_ops_replay(
    session: &TractorFieldOpsSessionLog,
) -> Result<TractorFieldOpsReplay, TractorFieldOpsSessionError> {
    let mut frames = Vec::new();
    for sample in &session.telemetry {
        parse_tractor_field_ops_timestamp(&sample.timestamp)?;
        frames.push(TractorFieldOpsReplayFrame {
            at: sample.timestamp.clone(),
            frame_type: TractorFieldOpsReplayFrameType::Telemetry,
            telemetry: Some(sample.clone()),
            safety_event: None,
            note: "telemetry_sample".to_string(),
        });
    }
    for event in &session.safety_events {
        parse_tractor_field_ops_timestamp(&event.at)?;
        frames.push(TractorFieldOpsReplayFrame {
            at: event.at.clone(),
            frame_type: TractorFieldOpsReplayFrameType::SafetyEvent,
            telemetry: None,
            safety_event: Some(event.clone()),
            note: event.reason_code.clone(),
        });
        if event.event_type == TractorFieldOpsSafetyEventType::TelemetryDropout {
            frames.push(TractorFieldOpsReplayFrame {
                at: event.at.clone(),
                frame_type: TractorFieldOpsReplayFrameType::TelemetryGap,
                telemetry: None,
                safety_event: Some(event.clone()),
                note: "explicit_gap_no_interpolation".to_string(),
            });
        }
    }
    frames.sort_by(|left, right| {
        left.at
            .cmp(&right.at)
            .then(replay_frame_order(left.frame_type).cmp(&replay_frame_order(right.frame_type)))
            .then(left.note.cmp(&right.note))
    });

    Ok(TractorFieldOpsReplay {
        session_id: session.session_id.clone(),
        tractor_id: session.tractor_id.clone(),
        field_id: session.field_id.clone(),
        read_only: true,
        gap_count: frames
            .iter()
            .filter(|frame| frame.frame_type == TractorFieldOpsReplayFrameType::TelemetryGap)
            .count(),
        frames,
    })
}

fn replay_frame_order(frame_type: TractorFieldOpsReplayFrameType) -> u8 {
    match frame_type {
        TractorFieldOpsReplayFrameType::Telemetry => 0,
        TractorFieldOpsReplayFrameType::SafetyEvent => 1,
        TractorFieldOpsReplayFrameType::TelemetryGap => 2,
    }
}

pub fn evaluate_tractor_geofence(
    request: TractorGeofenceEvaluationRequest,
) -> Result<TractorGeofenceEvaluation, TractorGeofenceError> {
    let tractor_id =
        normalize_tractor_text(request.tractor_id).ok_or(TractorGeofenceError::EmptyTractorId)?;
    let field_id =
        normalize_tractor_text(request.field_id).ok_or(TractorGeofenceError::EmptyFieldId)?;
    let boundary_ref = normalize_tractor_text(request.boundary_ref)
        .ok_or(TractorGeofenceError::EmptyBoundaryRef)?;
    let position_crs = normalize_tractor_text(request.position_crs)
        .ok_or(TractorGeofenceError::EmptyPositionCrs)?;
    validate_tractor_geofence_position(&request.current_position)?;
    validate_tractor_geofence_position(&request.predicted_position)?;

    let validated = validate_field_boundary(&request.boundary)?;
    let boundary_crs = validated
        .boundary
        .crs
        .clone()
        .ok_or(FieldBoundaryValidationError::MissingCrs)?;
    if boundary_crs != position_crs {
        return Err(TractorGeofenceError::CrsMismatch {
            position_crs,
            boundary_crs,
        });
    }

    let current_inside =
        tractor_point_inside_polygon(&request.current_position, &validated.boundary.coordinates);
    let predicted_inside =
        tractor_point_inside_polygon(&request.predicted_position, &validated.boundary.coordinates);
    let path_crosses_boundary = tractor_motion_crosses_boundary(
        &request.current_position,
        &request.predicted_position,
        &validated.boundary.coordinates,
    );
    let (decision, reason_code) = if current_inside && predicted_inside && !path_crosses_boundary {
        (TractorGeofenceDecision::Permitted, "inside_geofence")
    } else {
        (TractorGeofenceDecision::Halted, "geofence_predicted_breach")
    };

    Ok(TractorGeofenceEvaluation {
        tractor_id,
        field_id,
        boundary_ref,
        decision,
        reason_code: reason_code.to_string(),
        position: request.current_position,
        predicted_position: request.predicted_position,
        boundary_crs,
    })
}

fn validate_tractor_geofence_position(point: &GeoPoint) -> Result<(), TractorGeofenceError> {
    if point.longitude.is_finite() && point.latitude.is_finite() {
        Ok(())
    } else {
        Err(TractorGeofenceError::InvalidPosition)
    }
}

fn tractor_point_inside_polygon(point: &GeoPoint, ring: &[GeoPoint]) -> bool {
    if ring.len() < 4 {
        return false;
    }
    if ring
        .windows(2)
        .any(|edge| point_on_segment(&edge[0], point, &edge[1]))
    {
        return true;
    }

    let mut inside = false;
    let mut previous = ring.last().expect("ring length checked");
    for current in ring {
        let crosses_lat =
            (current.latitude > point.latitude) != (previous.latitude > point.latitude);
        if crosses_lat {
            let lon_at_lat = (previous.longitude - current.longitude)
                * (point.latitude - current.latitude)
                / (previous.latitude - current.latitude)
                + current.longitude;
            if point.longitude < lon_at_lat {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

fn tractor_motion_crosses_boundary(start: &GeoPoint, end: &GeoPoint, ring: &[GeoPoint]) -> bool {
    ring.windows(2)
        .any(|edge| segments_intersect(start, end, &edge[0], &edge[1]))
}

pub fn evaluate_tractor_motion_gate(
    request: &TractorMotionCommandRequest,
    estop: Option<&TractorEstopState>,
    approval: Option<&TractorOperatorApproval>,
    at: &str,
) -> Result<TractorMotionGateEvaluation, TractorMotionGateError> {
    let tractor_id = normalize_tractor_text(request.tractor_id.clone())
        .ok_or(TractorMotionGateError::EmptyTractorId)?;
    let at =
        normalize_tractor_text(at.to_string()).ok_or(TractorMotionGateError::EmptyTimestamp)?;
    parse_tractor_motion_gate_timestamp(&at)?;
    let command_id = request.command_id.clone().and_then(normalize_tractor_text);

    if estop
        .filter(|state| state.tractor_id.trim() == tractor_id && state.active)
        .is_some()
    {
        return Ok(tractor_motion_gate_result(
            tractor_id,
            command_id,
            TractorMotionGateDecision::Refused,
            true,
            None,
            request.requested_by.clone(),
            at,
            "estop_active",
        ));
    }

    let approval = approval.filter(|approval| approval.tractor_id.trim() == tractor_id);
    let Some(approval) = approval else {
        return Ok(tractor_motion_gate_result(
            tractor_id,
            command_id,
            TractorMotionGateDecision::Refused,
            false,
            None,
            request.requested_by.clone(),
            at,
            "operator_approval_required",
        ));
    };
    parse_tractor_motion_gate_timestamp(&approval.approved_at)?;
    if let Some(expires_at) = &approval.expires_at {
        let expires = parse_tractor_motion_gate_timestamp(expires_at)?;
        let requested_at = parse_tractor_motion_gate_timestamp(&at)?;
        if requested_at > expires {
            return Ok(tractor_motion_gate_result(
                tractor_id,
                command_id,
                TractorMotionGateDecision::Refused,
                false,
                None,
                request.requested_by.clone(),
                at,
                "operator_approval_expired",
            ));
        }
    }

    Ok(tractor_motion_gate_result(
        tractor_id,
        command_id,
        TractorMotionGateDecision::Allowed,
        false,
        Some(approval.approval_id.clone()),
        request.requested_by.clone(),
        at,
        "operator_approved",
    ))
}

fn tractor_motion_gate_result(
    tractor_id: String,
    command_id: Option<String>,
    decision: TractorMotionGateDecision,
    halted: bool,
    approval_id: Option<String>,
    actor: Option<String>,
    at: String,
    reason_code: &str,
) -> TractorMotionGateEvaluation {
    TractorMotionGateEvaluation {
        tractor_id: tractor_id.clone(),
        command_id: command_id.clone(),
        decision,
        halted,
        approval_id,
        audit: TractorMotionGateAudit {
            tractor_id,
            command_id,
            decision,
            reason_code: reason_code.to_string(),
            actor: actor.and_then(normalize_tractor_text),
            at,
        },
    }
}

fn parse_tractor_motion_gate_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, TractorMotionGateError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| TractorMotionGateError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn detect_tractor_obstacle(
    request: TractorObstacleDetectionRequest,
) -> Result<TractorObstacleDetection, TractorObstacleDetectionError> {
    let tractor_id = normalize_tractor_text(request.tractor_id)
        .ok_or(TractorObstacleDetectionError::EmptyTractorId)?;
    if !request.path_width_m.is_finite() || request.path_width_m <= 0.0 {
        return Err(TractorObstacleDetectionError::InvalidPathWidth);
    }
    if !request.stopping_distance_m.is_finite() || request.stopping_distance_m <= 0.0 {
        return Err(TractorObstacleDetectionError::InvalidStoppingDistance);
    }
    let (unit_x, unit_y, path_length) = tractor_guidance_unit_vector(request.path)?;
    let current_along =
        tractor_path_along_track_m(request.path, request.current_position, unit_x, unit_y);
    let half_width = request.path_width_m / 2.0;
    let mut nearest_event = None;

    for obstacle in request.obstacles {
        let along = tractor_path_along_track_m(request.path, obstacle, unit_x, unit_y);
        let lateral = tractor_path_lateral_error_m(request.path, obstacle, unit_x, unit_y).abs();
        let distance_ahead = along - current_along;
        if along < 0.0
            || along > path_length
            || lateral > half_width
            || distance_ahead < 0.0
            || distance_ahead > request.stopping_distance_m
        {
            continue;
        }
        let event = TractorObstacleEvent {
            distance_m: distance_ahead,
            position: obstacle,
            reason_code: "obstacle_in_path".to_string(),
        };
        if nearest_event
            .as_ref()
            .is_none_or(|existing: &TractorObstacleEvent| event.distance_m < existing.distance_m)
        {
            nearest_event = Some(event);
        }
    }

    Ok(TractorObstacleDetection {
        tractor_id,
        halted: nearest_event.is_some(),
        event: nearest_event,
    })
}

fn tractor_path_along_track_m(
    path: TractorGuidancePath,
    point: TractorGuidancePoint,
    unit_x: f64,
    unit_y: f64,
) -> f64 {
    (point.x_m - path.start.x_m) * unit_x + (point.y_m - path.start.y_m) * unit_y
}

fn tractor_path_lateral_error_m(
    path: TractorGuidancePath,
    point: TractorGuidancePoint,
    unit_x: f64,
    unit_y: f64,
) -> f64 {
    let dx = point.x_m - path.start.x_m;
    let dy = point.y_m - path.start.y_m;
    dx * unit_y - dy * unit_x
}

pub fn execute_tractor_prescription(
    request: TractorPrescriptionExecutionRequest,
) -> Result<TractorPrescriptionExecutionLog, TractorPrescriptionExecutionError> {
    if !request.runtime_mode.eq_ignore_ascii_case("simulation") {
        return Err(
            TractorPrescriptionExecutionError::RuntimeModeNotSimulation {
                runtime_mode: request.runtime_mode,
            },
        );
    }
    let field_id = normalize_tractor_text(request.field_id)
        .ok_or(TractorPrescriptionExecutionError::EmptyFieldId)?;
    let field_crs = normalize_tractor_text(request.field_crs)
        .ok_or(TractorPrescriptionExecutionError::EmptyFieldCrs)?;
    if request.zones.is_empty() {
        return Err(TractorPrescriptionExecutionError::EmptyZones);
    }
    if request.geofence.decision != TractorGeofenceDecision::Permitted {
        return Err(
            TractorPrescriptionExecutionError::SafetyPrerequisiteFailed {
                reason_code: request.geofence.reason_code,
            },
        );
    }
    if request.motion_gate.decision != TractorMotionGateDecision::Allowed {
        return Err(
            TractorPrescriptionExecutionError::SafetyPrerequisiteFailed {
                reason_code: request.motion_gate.audit.reason_code,
            },
        );
    }
    if request.obstacle.halted {
        return Err(
            TractorPrescriptionExecutionError::SafetyPrerequisiteFailed {
                reason_code: request
                    .obstacle
                    .event
                    .as_ref()
                    .map(|event| event.reason_code.clone())
                    .unwrap_or_else(|| "obstacle_halt".to_string()),
            },
        );
    }

    let mut applied_rates = Vec::new();
    for zone in request.zones {
        let zone_id =
            normalize_tractor_text(zone.zone_id).unwrap_or_else(|| "unknown-zone".to_string());
        if zone.crs != field_crs {
            return Err(TractorPrescriptionExecutionError::ZoneCrsMismatch {
                zone_id,
                field_crs,
                zone_crs: zone.crs,
            });
        }
        if !tractor_bounds_contains(&request.field_extent, &zone.extent) {
            return Err(TractorPrescriptionExecutionError::ZoneExtentMismatch { zone_id });
        }
        if !zone.rate.is_finite() || zone.rate < 0.0 {
            return Err(TractorPrescriptionExecutionError::InvalidRate { zone_id });
        }
        applied_rates.push(TractorPrescriptionAppliedRate {
            zone_id,
            rate: zone.rate,
            reason_code: "prescription_rate_applied".to_string(),
            evidence_refs: zone.evidence_refs,
        });
    }
    applied_rates.sort_by(|left, right| left.zone_id.cmp(&right.zone_id));

    Ok(TractorPrescriptionExecutionLog {
        field_id,
        runtime_mode: "simulation".to_string(),
        applied_rates,
    })
}

pub fn apply_tractor_implement_command(
    spec: TractorImplementAdapterSpec,
    current: TractorImplementState,
    command: TractorImplementCommand,
    motion_gate: &TractorMotionGateEvaluation,
    at: &str,
) -> Result<TractorImplementAdapterResult, TractorImplementControlError> {
    let implement_id = normalize_tractor_text(spec.implement_id)
        .ok_or(TractorImplementControlError::EmptyImplementId)?;
    let _implement_type = normalize_tractor_text(spec.implement_type)
        .ok_or(TractorImplementControlError::EmptyImplementType)?;
    if !spec.min_rate.is_finite()
        || !spec.max_rate.is_finite()
        || !spec.default_rate.is_finite()
        || spec.min_rate < 0.0
        || spec.min_rate > spec.max_rate
    {
        return Err(TractorImplementControlError::InvalidRateBounds);
    }
    let at = normalize_tractor_text(at.to_string())
        .ok_or(TractorImplementControlError::EmptyTimestamp)?;
    parse_tractor_implement_timestamp(&at)?;

    let safe_current_rate = clamp_tractor_implement_rate(
        if current.current_rate.is_finite() {
            current.current_rate
        } else {
            spec.default_rate
        },
        spec.min_rate,
        spec.max_rate,
    );
    let requested_rate = match &command {
        TractorImplementCommand::SetRate { rate } => Some(*rate),
        TractorImplementCommand::Enable | TractorImplementCommand::Disable => None,
    };

    if motion_gate.halted || motion_gate.decision != TractorMotionGateDecision::Allowed {
        let reason_code = if motion_gate.halted {
            "tractor_halted"
        } else {
            "motion_not_approved"
        };
        return Ok(tractor_implement_result(
            TractorImplementState {
                implement_id,
                enabled: false,
                current_rate: safe_current_rate,
            },
            command,
            TractorImplementDecision::ForcedOff,
            requested_rate,
            Some(safe_current_rate),
            reason_code,
            at,
        ));
    }

    let (state, decision, applied_rate, reason_code) = match command.clone() {
        TractorImplementCommand::Enable => {
            let rate =
                clamp_tractor_implement_rate(spec.default_rate, spec.min_rate, spec.max_rate);
            (
                TractorImplementState {
                    implement_id,
                    enabled: true,
                    current_rate: rate,
                },
                TractorImplementDecision::Applied,
                Some(rate),
                "implement_enabled",
            )
        }
        TractorImplementCommand::Disable => (
            TractorImplementState {
                implement_id,
                enabled: false,
                current_rate: safe_current_rate,
            },
            TractorImplementDecision::Applied,
            Some(safe_current_rate),
            "implement_disabled",
        ),
        TractorImplementCommand::SetRate { rate } => {
            if !rate.is_finite() || rate < spec.min_rate || rate > spec.max_rate {
                (
                    TractorImplementState {
                        implement_id,
                        enabled: current.enabled,
                        current_rate: safe_current_rate,
                    },
                    TractorImplementDecision::Refused,
                    Some(safe_current_rate),
                    "rate_out_of_bounds",
                )
            } else {
                (
                    TractorImplementState {
                        implement_id,
                        enabled: current.enabled,
                        current_rate: rate,
                    },
                    TractorImplementDecision::Applied,
                    Some(rate),
                    "rate_applied",
                )
            }
        }
    };

    Ok(tractor_implement_result(
        state,
        command,
        decision,
        requested_rate,
        applied_rate,
        reason_code,
        at,
    ))
}

fn tractor_implement_result(
    state: TractorImplementState,
    command: TractorImplementCommand,
    decision: TractorImplementDecision,
    requested_rate: Option<f64>,
    applied_rate: Option<f64>,
    reason_code: &str,
    at: String,
) -> TractorImplementAdapterResult {
    TractorImplementAdapterResult {
        log: TractorImplementSetpointLog {
            implement_id: state.implement_id.clone(),
            command,
            decision,
            requested_rate,
            applied_rate,
            reason_code: reason_code.to_string(),
            at,
        },
        state,
    }
}

fn clamp_tractor_implement_rate(rate: f64, min_rate: f64, max_rate: f64) -> f64 {
    rate.max(min_rate).min(max_rate)
}

fn parse_tractor_implement_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, TractorImplementControlError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| TractorImplementControlError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn evaluate_tractor_weather_window_gate(
    request: TractorWeatherWindowGateRequest,
) -> Result<TractorWeatherWindowGate, TractorWeatherWindowGateError> {
    let field_id = normalize_tractor_text(request.field_id)
        .ok_or(TractorWeatherWindowGateError::EmptyFieldId)?;
    let requested_start_at = normalize_tractor_text(request.requested_start_at)
        .ok_or(TractorWeatherWindowGateError::EmptyRequestedStartAt)?;
    if request.max_window_age_seconds <= 0 {
        return Err(TractorWeatherWindowGateError::InvalidMaxWindowAge);
    }
    let requested_at = parse_tractor_weather_window_timestamp(&requested_start_at)?;

    if request.motion_gate.decision != TractorMotionGateDecision::Allowed
        || request.motion_gate.halted
    {
        return Ok(tractor_weather_window_gate_result(
            field_id,
            TractorWeatherWindowDecision::Blocked,
            "motion_gate_not_allowed",
            requested_start_at,
            None,
            vec![request.motion_gate.audit.reason_code],
        ));
    }

    let Some(window) = request.window else {
        return Ok(tractor_weather_window_gate_result(
            field_id,
            TractorWeatherWindowDecision::Blocked,
            "weather_window_missing",
            requested_start_at,
            None,
            vec!["window:missing".to_string()],
        ));
    };

    let window_field_id = normalize_tractor_text(window.field_id).unwrap_or_default();
    let source = normalize_tractor_text(window.source).unwrap_or_else(|| "unknown".to_string());
    let reason_code = normalize_tractor_text(window.reason_code)
        .unwrap_or_else(|| "window_unspecified".to_string());
    let fetched_at = parse_tractor_weather_window_timestamp(&window.fetched_at)?;
    let valid_from = parse_tractor_weather_window_timestamp(&window.valid_from)?;
    let valid_until = parse_tractor_weather_window_timestamp(&window.valid_until)?;
    let mut inputs = window.gating_inputs;
    inputs.push(format!("source:{source}"));
    inputs.push(format!("fetched_at:{}", window.fetched_at));
    inputs.push(format!("valid_from:{}", window.valid_from));
    inputs.push(format!("valid_until:{}", window.valid_until));
    inputs.push(format!("window_reason:{reason_code}"));

    if window_field_id != field_id {
        return Ok(tractor_weather_window_gate_result(
            field_id,
            TractorWeatherWindowDecision::Blocked,
            "weather_window_field_mismatch",
            requested_start_at,
            Some(source),
            inputs,
        ));
    }
    let age_seconds = requested_at.signed_duration_since(fetched_at).num_seconds();
    if age_seconds < 0 || age_seconds > request.max_window_age_seconds {
        return Ok(tractor_weather_window_gate_result(
            field_id,
            TractorWeatherWindowDecision::Blocked,
            "weather_window_stale",
            requested_start_at,
            Some(source),
            inputs,
        ));
    }
    if requested_at < valid_from || requested_at > valid_until {
        return Ok(tractor_weather_window_gate_result(
            field_id,
            TractorWeatherWindowDecision::Blocked,
            "outside_weather_window",
            requested_start_at,
            Some(source),
            inputs,
        ));
    }
    if !window.allowed {
        return Ok(tractor_weather_window_gate_result(
            field_id,
            TractorWeatherWindowDecision::Blocked,
            &reason_code,
            requested_start_at,
            Some(source),
            inputs,
        ));
    }

    Ok(tractor_weather_window_gate_result(
        field_id,
        TractorWeatherWindowDecision::Allowed,
        "weather_window_allowed",
        requested_start_at,
        Some(source),
        inputs,
    ))
}

fn tractor_weather_window_gate_result(
    field_id: String,
    decision: TractorWeatherWindowDecision,
    reason_code: &str,
    requested_start_at: String,
    window_source: Option<String>,
    gating_inputs: Vec<String>,
) -> TractorWeatherWindowGate {
    TractorWeatherWindowGate {
        field_id,
        decision,
        reason_code: reason_code.to_string(),
        requested_start_at,
        window_source,
        gating_inputs,
    }
}

fn parse_tractor_weather_window_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, TractorWeatherWindowGateError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| TractorWeatherWindowGateError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn deconflict_tractor_swath_reservations(
    request: TractorDeconflictionRequest,
) -> Result<TractorDeconflictionPlan, TractorDeconflictionError> {
    let field_id =
        normalize_tractor_text(request.field_id).ok_or(TractorDeconflictionError::EmptyFieldId)?;
    let evaluated_at = normalize_tractor_text(request.evaluated_at)
        .ok_or(TractorDeconflictionError::EmptyEvaluatedAt)?;
    parse_tractor_deconfliction_timestamp(&evaluated_at)?;
    if request.reservations.is_empty() {
        return Err(TractorDeconflictionError::EmptyReservations);
    }

    let mut reservations = Vec::new();
    let mut decisions = Vec::new();
    let mut events = Vec::new();
    for reservation in request.reservations {
        let tractor_id = normalize_tractor_text(reservation.tractor_id.clone())
            .ok_or(TractorDeconflictionError::EmptyTractorId)?;
        validate_tractor_deconfliction_swath(&reservation.swath)?;
        let starts_at = parse_tractor_deconfliction_timestamp(&reservation.starts_at)?;
        let ends_at = parse_tractor_deconfliction_timestamp(&reservation.ends_at)?;
        if ends_at <= starts_at {
            return Err(TractorDeconflictionError::InvalidTimestamp {
                timestamp: reservation.ends_at,
            });
        }

        let safety_reason = tractor_deconfliction_safety_reason(&reservation);
        decisions.push(TractorDeconflictionReservationDecision {
            tractor_id: tractor_id.clone(),
            decision: if safety_reason.is_some() {
                TractorDeconflictionDecision::Halted
            } else {
                TractorDeconflictionDecision::Proceed
            },
            reason_code: safety_reason.unwrap_or("reserved".to_string()),
            conflict_with: None,
        });
        reservations.push((tractor_id, reservation, starts_at, ends_at));
    }

    for left_idx in 0..reservations.len() {
        for right_idx in (left_idx + 1)..reservations.len() {
            if decisions[left_idx].decision == TractorDeconflictionDecision::Halted
                || decisions[right_idx].decision == TractorDeconflictionDecision::Halted
            {
                continue;
            }
            let left = &reservations[left_idx];
            let right = &reservations[right_idx];
            if !tractor_reservations_conflict(left, right) {
                continue;
            }

            let (halted_idx, priority_idx) =
                tractor_deconfliction_halt_order(left_idx, right_idx, &reservations);
            decisions[halted_idx].decision = TractorDeconflictionDecision::Halted;
            decisions[halted_idx].reason_code = "swath_time_conflict".to_string();
            decisions[halted_idx].conflict_with = Some(decisions[priority_idx].tractor_id.clone());
            events.push(TractorDeconflictionEvent {
                halted_tractor_id: decisions[halted_idx].tractor_id.clone(),
                priority_tractor_id: decisions[priority_idx].tractor_id.clone(),
                reason_code: "swath_time_conflict".to_string(),
                at: evaluated_at.clone(),
            });
        }
    }

    decisions.sort_by(|left, right| left.tractor_id.cmp(&right.tractor_id));
    events.sort_by(|left, right| {
        left.halted_tractor_id
            .cmp(&right.halted_tractor_id)
            .then_with(|| left.priority_tractor_id.cmp(&right.priority_tractor_id))
    });
    let all_clear = decisions
        .iter()
        .all(|decision| decision.decision == TractorDeconflictionDecision::Proceed);

    Ok(TractorDeconflictionPlan {
        field_id,
        all_clear,
        decisions,
        events,
    })
}

fn tractor_deconfliction_safety_reason(reservation: &TractorSwathReservation) -> Option<String> {
    if reservation.geofence.decision != TractorGeofenceDecision::Permitted {
        return Some(reservation.geofence.reason_code.clone());
    }
    if reservation.motion_gate.decision != TractorMotionGateDecision::Allowed
        || reservation.motion_gate.halted
    {
        return Some(reservation.motion_gate.audit.reason_code.clone());
    }
    if reservation.obstacle.halted {
        return Some(
            reservation
                .obstacle
                .event
                .as_ref()
                .map(|event| event.reason_code.clone())
                .unwrap_or_else(|| "obstacle_halt".to_string()),
        );
    }
    None
}

fn tractor_reservations_conflict(
    left: &(
        String,
        TractorSwathReservation,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    ),
    right: &(
        String,
        TractorSwathReservation,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    ),
) -> bool {
    left.2 < right.3
        && right.2 < left.3
        && tractor_swath_footprints_overlap(&left.1.swath, &right.1.swath)
}

fn tractor_deconfliction_halt_order(
    left_idx: usize,
    right_idx: usize,
    reservations: &[(
        String,
        TractorSwathReservation,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    )],
) -> (usize, usize) {
    let left = &reservations[left_idx];
    let right = &reservations[right_idx];
    if left.1.priority > right.1.priority {
        (left_idx, right_idx)
    } else if right.1.priority > left.1.priority {
        (right_idx, left_idx)
    } else if left.0 > right.0 {
        (left_idx, right_idx)
    } else {
        (right_idx, left_idx)
    }
}

fn tractor_swath_footprints_overlap(
    left: &TractorSwathSegment,
    right: &TractorSwathSegment,
) -> bool {
    if segments_intersect(&left.start, &left.end, &right.start, &right.end) {
        return true;
    }
    let left_bounds = tractor_swath_footprint_bounds(left);
    let right_bounds = tractor_swath_footprint_bounds(right);
    left_bounds.min_lon <= right_bounds.max_lon
        && left_bounds.max_lon >= right_bounds.min_lon
        && left_bounds.min_lat <= right_bounds.max_lat
        && left_bounds.max_lat >= right_bounds.min_lat
}

fn tractor_swath_footprint_bounds(swath: &TractorSwathSegment) -> GeoBounds {
    let half_width = swath.width_m / 2.0;
    GeoBounds {
        min_lon: swath.start.longitude.min(swath.end.longitude) - half_width,
        min_lat: swath.start.latitude.min(swath.end.latitude) - half_width,
        max_lon: swath.start.longitude.max(swath.end.longitude) + half_width,
        max_lat: swath.start.latitude.max(swath.end.latitude) + half_width,
    }
}

fn validate_tractor_deconfliction_swath(
    swath: &TractorSwathSegment,
) -> Result<(), TractorDeconflictionError> {
    if !swath.start.longitude.is_finite()
        || !swath.start.latitude.is_finite()
        || !swath.end.longitude.is_finite()
        || !swath.end.latitude.is_finite()
        || !swath.width_m.is_finite()
        || swath.width_m <= 0.0
        || (swath.start.longitude == swath.end.longitude
            && swath.start.latitude == swath.end.latitude)
    {
        return Err(TractorDeconflictionError::InvalidSwath);
    }
    Ok(())
}

fn parse_tractor_deconfliction_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, TractorDeconflictionError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| TractorDeconflictionError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

fn tractor_bounds_contains(outer: &GeoBounds, inner: &GeoBounds) -> bool {
    inner.min_lon >= outer.min_lon
        && inner.max_lon <= outer.max_lon
        && inner.min_lat >= outer.min_lat
        && inner.max_lat <= outer.max_lat
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherForecastValue {
    pub value: f64,
    pub unit: String,
    pub source: String,
    pub fetched_at: String,
    pub valid_time: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherFreshnessState {
    Fresh,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherFreshnessAnnotatedValue {
    pub value: WeatherForecastValue,
    pub freshness_state: WeatherFreshnessState,
    pub age_seconds: i64,
    pub stale_after_seconds: i64,
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherFreshnessAnnotatedRecord {
    pub forecast_id: String,
    pub field_ref: String,
    pub valid_time: String,
    pub source: String,
    pub fetched_at: String,
    pub temperature_celsius: WeatherFreshnessAnnotatedValue,
    pub wind_speed_mps: WeatherFreshnessAnnotatedValue,
    pub precipitation_mm: WeatherFreshnessAnnotatedValue,
    pub humidity_percent: WeatherFreshnessAnnotatedValue,
    pub radiation_w_m2: WeatherFreshnessAnnotatedValue,
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherSensorSample {
    pub observed_at: String,
    pub temperature_celsius: f64,
    pub wind_speed_mps: f64,
    pub precipitation_mm: f64,
    pub humidity_percent: f64,
    pub radiation_w_m2: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherSensorStreamIngestRequest {
    pub sensor_id: String,
    pub field_ref: String,
    pub fetched_at: String,
    pub evaluated_at: String,
    pub stale_after_seconds: i64,
    pub max_gap_seconds: i64,
    pub samples: Vec<WeatherSensorSample>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeatherSensorGapEvent {
    pub sensor_id: String,
    pub field_ref: String,
    pub from: String,
    pub to: String,
    pub gap_seconds: i64,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherSensorStreamIngest {
    pub sensor_id: String,
    pub field_ref: String,
    pub source: String,
    pub fetched_at: String,
    pub records: Vec<WeatherForecastRecord>,
    pub freshness: Vec<WeatherFreshnessAnnotatedRecord>,
    pub gap_events: Vec<WeatherSensorGapEvent>,
    pub sample_count: usize,
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherHistoryEntry {
    pub sequence: usize,
    pub record: WeatherFreshnessAnnotatedRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeatherHistoryQuery {
    pub field_ref: String,
    pub start_time: String,
    pub end_time: String,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherHistoryQueryResult {
    pub field_ref: String,
    pub total_count: usize,
    pub offset: usize,
    pub limit: usize,
    pub empty: bool,
    pub records: Vec<WeatherHistoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherOperationalWindowThresholds {
    pub max_wind_speed_mps: f64,
    pub max_precipitation_mm: f64,
    pub min_temperature_celsius: f64,
    pub max_temperature_celsius: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherOperationalWindowRequest {
    pub field_ref: String,
    pub thresholds: WeatherOperationalWindowThresholds,
    pub records: Vec<WeatherFreshnessAnnotatedRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeatherOperationalWindowGap {
    pub reason_code: String,
    pub details: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherOperationalWindow {
    pub field_ref: String,
    pub start: String,
    pub end: String,
    pub gating_vars: Vec<String>,
    pub thresholds: Vec<String>,
    pub freshness: Vec<WeatherFreshnessState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherOperationalWindowReport {
    pub field_ref: String,
    pub windows: Vec<WeatherOperationalWindow>,
    pub gaps: Vec<WeatherOperationalWindowGap>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherRiskThresholds {
    pub frost_temperature_celsius: f64,
    pub heat_temperature_celsius: f64,
    pub wind_speed_mps: f64,
    pub precipitation_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherRiskType {
    Frost,
    Heat,
    Wind,
    Precipitation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherRiskAlert {
    pub field_ref: String,
    pub risk_type: WeatherRiskType,
    pub value: f64,
    pub threshold: f64,
    pub valid_time: String,
    pub source: String,
    pub freshness: WeatherFreshnessState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherGrowingDegreeDayRequest {
    pub field_ref: String,
    pub date: String,
    pub base_temperature_celsius: f64,
    pub records: Vec<WeatherFreshnessAnnotatedRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherGrowingDegreeDayStatus {
    Computed,
    NoData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherGrowingDegreeDay {
    pub field_ref: String,
    pub date: String,
    pub status: WeatherGrowingDegreeDayStatus,
    #[serde(default)]
    pub gdd_celsius_days: Option<f64>,
    #[serde(default)]
    pub min_temperature_celsius: Option<f64>,
    #[serde(default)]
    pub max_temperature_celsius: Option<f64>,
    pub base_temperature_celsius: f64,
    pub method: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherReferenceEtInput {
    pub field_ref: String,
    pub date: String,
    #[serde(default)]
    pub temperature_celsius: Option<WeatherFreshnessAnnotatedValue>,
    #[serde(default)]
    pub humidity_percent: Option<WeatherFreshnessAnnotatedValue>,
    #[serde(default)]
    pub wind_speed_mps: Option<WeatherFreshnessAnnotatedValue>,
    #[serde(default)]
    pub radiation_w_m2: Option<WeatherFreshnessAnnotatedValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterWeatherInputContractRequest {
    pub field_ref: String,
    pub date: String,
    pub records: Vec<WeatherFreshnessAnnotatedRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaterWeatherInputStatus {
    Valid,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterWeatherInputContract {
    pub field_ref: String,
    pub date: String,
    pub status: WaterWeatherInputStatus,
    #[serde(default)]
    pub temperature_celsius: Option<WeatherFreshnessAnnotatedValue>,
    #[serde(default)]
    pub humidity_percent: Option<WeatherFreshnessAnnotatedValue>,
    #[serde(default)]
    pub wind_speed_mps: Option<WeatherFreshnessAnnotatedValue>,
    #[serde(default)]
    pub radiation_w_m2: Option<WeatherFreshnessAnnotatedValue>,
    #[serde(default)]
    pub precipitation_mm: Option<WeatherFreshnessAnnotatedValue>,
    pub et_blocked: bool,
    pub degradation_reasons: Vec<String>,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WaterWeatherInputContractError {
    #[error("water weather input field_ref cannot be empty")]
    EmptyFieldRef,
    #[error("water weather input date is invalid: {date}")]
    InvalidDate { date: String },
    #[error("water weather input timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterEvapotranspirationRequest {
    pub weather: WaterWeatherInputContract,
    #[serde(default)]
    pub crop_coefficient: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaterEvapotranspirationStatus {
    Computed,
    InsufficientInputs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterEvapotranspiration {
    pub field_ref: String,
    pub date: String,
    pub status: WaterEvapotranspirationStatus,
    #[serde(default)]
    pub reference_et_mm_day: Option<f64>,
    #[serde(default)]
    pub crop_et_mm_day: Option<f64>,
    pub crop_coefficient: f64,
    pub method: String,
    pub input_refs: Vec<String>,
    pub degradation_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WaterEvapotranspirationError {
    #[error("water ET crop coefficient is invalid: {value}")]
    InvalidCropCoefficient { value: String },
    #[error("water ET weather reference calculation failed: {0}")]
    Weather(#[from] WeatherReferenceEtError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaterNeedZone {
    pub zone_ref: String,
    pub crs: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneWaterNeedRequest {
    pub field_ref: String,
    pub crs: String,
    pub zones: Vec<WaterNeedZone>,
    pub soil_readings: Vec<SoilMoistureReadingRecord>,
    pub moisture_proxies: Vec<RemoteSensingMoistureProxyRecord>,
    pub evapotranspiration: WaterEvapotranspiration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneWaterNeedStatus {
    Computed,
    InsufficientEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneWaterNeed {
    pub field_ref: String,
    pub zone_ref: String,
    pub crs: String,
    pub status: ZoneWaterNeedStatus,
    #[serde(default)]
    pub water_need_mm: Option<f64>,
    pub evidence_refs: Vec<String>,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ZoneWaterNeedError {
    #[error("zone water need field_ref cannot be empty")]
    EmptyFieldRef,
    #[error("zone water need CRS cannot be empty")]
    EmptyCrs,
    #[error("zone water need zone_ref cannot be empty")]
    EmptyZoneRef,
    #[error("zone water need requires at least one zone")]
    MissingZones,
    #[error("zone water need CRS mismatch for {zone_ref}: zone {zone_crs}, request {request_crs}")]
    CrsMismatch {
        zone_ref: String,
        zone_crs: String,
        request_crs: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationScheduleRequest {
    pub field_ref: String,
    pub generated_at: String,
    pub scheduled_start: String,
    pub application_rate_mm_per_hour: f64,
    pub water_needs: Vec<ZoneWaterNeed>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationScheduleEntry {
    pub zone_ref: String,
    pub amount_mm: f64,
    pub start_time: String,
    pub duration_minutes: u32,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrrigationScheduleExclusion {
    pub zone_ref: String,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationSchedule {
    pub field_ref: String,
    pub generated_at: String,
    pub entries: Vec<IrrigationScheduleEntry>,
    pub exclusions: Vec<IrrigationScheduleExclusion>,
    pub method: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IrrigationScheduleError {
    #[error("irrigation schedule field_ref cannot be empty")]
    EmptyFieldRef,
    #[error("irrigation schedule generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("irrigation schedule start cannot be empty")]
    EmptyScheduledStart,
    #[error("irrigation schedule timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
    #[error("irrigation schedule application rate must be positive")]
    InvalidApplicationRate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationValveSpec {
    pub zone_ref: String,
    pub min_amount_mm: f64,
    pub max_amount_mm: f64,
    pub max_duration_minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationValveDryRunRequest {
    pub dry_run_id: String,
    pub checked_at: String,
    pub schedule: IrrigationSchedule,
    pub valves: Vec<IrrigationValveSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrrigationValveDryRunStatus {
    Passed,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrrigationValveActionStatus {
    Planned,
    Applied,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationValveActionAudit {
    pub zone_ref: String,
    pub amount_mm: f64,
    pub duration_minutes: u32,
    pub status: IrrigationValveActionStatus,
    pub reason_code: String,
    pub at: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationValveDryRun {
    pub dry_run_id: String,
    pub field_ref: String,
    pub status: IrrigationValveDryRunStatus,
    pub checked_at: String,
    pub schedule_fingerprint: String,
    pub audits: Vec<IrrigationValveActionAudit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationValveExecuteRequest {
    pub executed_at: String,
    pub schedule: IrrigationSchedule,
    pub dry_run: IrrigationValveDryRun,
    pub abort_requested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrrigationValveExecutionStatus {
    Applied,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationValveExecution {
    pub field_ref: String,
    pub status: IrrigationValveExecutionStatus,
    pub executed_at: String,
    pub dry_run_id: String,
    pub audits: Vec<IrrigationValveActionAudit>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IrrigationValveControlError {
    #[error("irrigation valve dry_run_id cannot be empty")]
    EmptyDryRunId,
    #[error("irrigation valve timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("irrigation valve timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
    #[error("irrigation valve zone_ref cannot be empty")]
    EmptyZoneRef,
    #[error("irrigation valve bounds are invalid for {zone_ref}")]
    InvalidBounds { zone_ref: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherReferenceEtStatus {
    Computed,
    InsufficientInputs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherReferenceEt {
    pub field_ref: String,
    pub date: String,
    pub status: WeatherReferenceEtStatus,
    #[serde(default)]
    pub reference_et_mm_day: Option<f64>,
    pub method: String,
    pub input_refs: Vec<String>,
    pub freshness: Vec<WeatherFreshnessState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherAlertRouteTarget {
    OperatorConsole,
    FarmersPortal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeatherAlertRoutingTarget {
    pub target: WeatherAlertRouteTarget,
    pub reachable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherAlertRoutingRequest {
    pub alert: WeatherRiskAlert,
    pub recipient_id: String,
    pub owned_field_refs: Vec<String>,
    pub targets: Vec<WeatherAlertRoutingTarget>,
    pub routed_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherAlertDeliveryStatus {
    Delivered,
    Queued,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherAlertDeliveryAudit {
    pub target: WeatherAlertRouteTarget,
    pub status: WeatherAlertDeliveryStatus,
    pub reason_code: String,
    pub recipient_id: String,
    pub field_ref: String,
    pub routed_at: String,
    pub evidence_payload: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherAlertRoutingResult {
    pub delivered_count: usize,
    pub queued_count: usize,
    pub rejected_count: usize,
    pub audits: Vec<WeatherAlertDeliveryAudit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherCropStageThresholdSet {
    pub crop_stage: String,
    pub threshold_set_name: String,
    pub thresholds: WeatherRiskThresholds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherCropStageRiskRequest {
    pub field_ref: String,
    #[serde(default)]
    pub crop_stage: Option<String>,
    pub default_thresholds: WeatherRiskThresholds,
    pub stage_thresholds: Vec<WeatherCropStageThresholdSet>,
    pub records: Vec<WeatherFreshnessAnnotatedRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherCropStageRiskAlert {
    pub alert: WeatherRiskAlert,
    pub crop_stage: String,
    pub threshold_set_name: String,
    pub fallback_applied: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherForecastVerificationRequest {
    pub forecast: WeatherForecastRecord,
    pub observations: Vec<WeatherFreshnessAnnotatedRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherForecastVerificationStatus {
    Verified,
    NotVerifiable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherForecastErrorMetric {
    pub variable: String,
    pub forecast_value: f64,
    pub observed_value: f64,
    pub absolute_error: f64,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherForecastVerification {
    pub field_ref: String,
    pub source: String,
    pub valid_time: String,
    pub status: WeatherForecastVerificationStatus,
    pub metrics: Vec<WeatherForecastErrorMetric>,
    pub evidence_refs: Vec<String>,
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
pub struct WeatherFieldForecastResolutionRequest {
    pub field_id: String,
    #[serde(default)]
    pub boundary: Option<FieldBoundary>,
    pub forecast_location: GeoPoint,
    pub forecast_crs: String,
    pub records: Vec<WeatherForecastRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherFieldForecastResolution {
    pub field_id: String,
    pub forecast_location: GeoPoint,
    pub field_centroid: GeoPoint,
    pub field_crs: String,
    pub records: Vec<WeatherForecastRecord>,
    pub evidence_refs: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WeatherFieldForecastResolutionError {
    #[error("weather field forecast field_id cannot be empty")]
    EmptyFieldId,
    #[error("weather field forecast has no field geometry")]
    NoFieldGeometry,
    #[error("weather field forecast forecast_crs cannot be empty")]
    EmptyForecastCrs,
    #[error("weather field forecast contains no records")]
    EmptyForecastRecords,
    #[error("weather field forecast CRS mismatch: forecast {forecast_crs} != field {field_crs}")]
    CrsMismatch {
        forecast_crs: String,
        field_crs: String,
    },
    #[error("weather field forecast location contains invalid coordinates")]
    InvalidForecastLocation,
    #[error("weather field forecast location is outside the field boundary")]
    ForecastOutsideField,
    #[error(transparent)]
    InvalidBoundary(#[from] FieldBoundaryValidationError),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WeatherFreshnessError {
    #[error("weather freshness observed_at cannot be empty")]
    EmptyObservedAt,
    #[error("weather freshness max age must be positive")]
    InvalidMaxAge,
    #[error("weather freshness timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WeatherSensorIngestError {
    #[error("weather sensor_id cannot be empty")]
    EmptySensorId,
    #[error("weather sensor field_ref cannot be empty")]
    EmptyFieldRef,
    #[error("weather sensor fetched_at cannot be empty")]
    EmptyFetchedAt,
    #[error("weather sensor evaluated_at cannot be empty")]
    EmptyEvaluatedAt,
    #[error("weather sensor stream contains no samples")]
    EmptySamples,
    #[error("weather sensor stale threshold must be positive")]
    InvalidStaleThreshold,
    #[error("weather sensor gap threshold must be positive")]
    InvalidGapThreshold,
    #[error("weather sensor sample timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
    #[error(transparent)]
    Weather(#[from] WeatherIngestError),
    #[error(transparent)]
    Freshness(#[from] WeatherFreshnessError),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WeatherHistoryError {
    #[error("weather history field_ref cannot be empty")]
    EmptyFieldRef,
    #[error("weather history limit must be positive")]
    InvalidLimit,
    #[error("weather history date range is invalid")]
    InvalidDateRange,
    #[error("weather history timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WeatherOperationalWindowError {
    #[error("weather operational window field_ref cannot be empty")]
    EmptyFieldRef,
    #[error("weather operational window thresholds are invalid")]
    InvalidThresholds,
    #[error("weather operational window timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WeatherRiskAlertError {
    #[error("weather risk alert thresholds are invalid")]
    InvalidThresholds,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WeatherGrowingDegreeDayError {
    #[error("weather GDD field_ref cannot be empty")]
    EmptyFieldRef,
    #[error("weather GDD date is invalid: {date}")]
    InvalidDate { date: String },
    #[error("weather GDD base temperature is invalid")]
    InvalidBaseTemperature,
    #[error("weather GDD timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WeatherReferenceEtError {
    #[error("weather ET field_ref cannot be empty")]
    EmptyFieldRef,
    #[error("weather ET date is invalid: {date}")]
    InvalidDate { date: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WeatherAlertRoutingError {
    #[error("weather alert routing recipient_id cannot be empty")]
    EmptyRecipientId,
    #[error("weather alert routing routed_at cannot be empty")]
    EmptyRoutedAt,
    #[error("weather alert routing requires at least one target")]
    EmptyTargets,
    #[error("weather alert routing timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WeatherCropStageRiskError {
    #[error("weather crop-stage risk field_ref cannot be empty")]
    EmptyFieldRef,
    #[error(transparent)]
    Risk(#[from] WeatherRiskAlertError),
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

pub fn resolve_weather_forecast_to_field(
    request: WeatherFieldForecastResolutionRequest,
) -> Result<WeatherFieldForecastResolution, WeatherFieldForecastResolutionError> {
    let field_id = normalize_weather_text(request.field_id)
        .ok_or(WeatherFieldForecastResolutionError::EmptyFieldId)?;
    let forecast_crs = normalize_weather_text(request.forecast_crs)
        .ok_or(WeatherFieldForecastResolutionError::EmptyForecastCrs)?;
    if request.records.is_empty() {
        return Err(WeatherFieldForecastResolutionError::EmptyForecastRecords);
    }
    if !request.forecast_location.longitude.is_finite()
        || !request.forecast_location.latitude.is_finite()
    {
        return Err(WeatherFieldForecastResolutionError::InvalidForecastLocation);
    }
    let boundary = request
        .boundary
        .ok_or(WeatherFieldForecastResolutionError::NoFieldGeometry)?;
    let validated = validate_field_boundary(&boundary)?;
    let field_crs = validated
        .boundary
        .crs
        .clone()
        .ok_or(FieldBoundaryValidationError::MissingCrs)?;
    if forecast_crs != field_crs {
        return Err(WeatherFieldForecastResolutionError::CrsMismatch {
            forecast_crs,
            field_crs,
        });
    }
    if !tractor_point_inside_bounds(&request.forecast_location, &validated.extent)
        || !tractor_point_inside_polygon(
            &request.forecast_location,
            &validated.boundary.coordinates,
        )
    {
        return Err(WeatherFieldForecastResolutionError::ForecastOutsideField);
    }

    let records = request
        .records
        .into_iter()
        .map(|mut record| {
            record.field_ref = field_id.clone();
            record
        })
        .collect();
    let field_centroid = GeoPoint {
        longitude: (validated.extent.min_lon + validated.extent.max_lon) / 2.0,
        latitude: (validated.extent.min_lat + validated.extent.max_lat) / 2.0,
    };

    Ok(WeatherFieldForecastResolution {
        field_id,
        forecast_location: request.forecast_location,
        field_centroid,
        field_crs,
        records,
        evidence_refs: vec![
            "field_boundary:validated".to_string(),
            "forecast_location:inside_field".to_string(),
        ],
    })
}

pub fn evaluate_weather_value_freshness(
    value: WeatherForecastValue,
    observed_at: &str,
    stale_after_seconds: i64,
) -> Result<WeatherFreshnessAnnotatedValue, WeatherFreshnessError> {
    if stale_after_seconds <= 0 {
        return Err(WeatherFreshnessError::InvalidMaxAge);
    }
    let observed_at = normalize_weather_text(observed_at.to_string())
        .ok_or(WeatherFreshnessError::EmptyObservedAt)?;
    let observed_at = parse_weather_freshness_timestamp(&observed_at)?;
    let fetched_at = parse_weather_freshness_timestamp(&value.fetched_at)?;
    parse_weather_freshness_timestamp(&value.valid_time)?;
    let age_seconds = observed_at.signed_duration_since(fetched_at).num_seconds();
    let stale = age_seconds < 0 || age_seconds > stale_after_seconds;
    Ok(WeatherFreshnessAnnotatedValue {
        value,
        freshness_state: if stale {
            WeatherFreshnessState::Stale
        } else {
            WeatherFreshnessState::Fresh
        },
        age_seconds,
        stale_after_seconds,
        stale,
    })
}

pub fn annotate_weather_record_freshness(
    record: WeatherForecastRecord,
    observed_at: &str,
    stale_after_seconds: i64,
) -> Result<WeatherFreshnessAnnotatedRecord, WeatherFreshnessError> {
    let temperature_celsius = evaluate_weather_value_freshness(
        record.vars.temperature_celsius,
        observed_at,
        stale_after_seconds,
    )?;
    let wind_speed_mps = evaluate_weather_value_freshness(
        record.vars.wind_speed_mps,
        observed_at,
        stale_after_seconds,
    )?;
    let precipitation_mm = evaluate_weather_value_freshness(
        record.vars.precipitation_mm,
        observed_at,
        stale_after_seconds,
    )?;
    let humidity_percent = evaluate_weather_value_freshness(
        record.vars.humidity_percent,
        observed_at,
        stale_after_seconds,
    )?;
    let radiation_w_m2 = evaluate_weather_value_freshness(
        record.vars.radiation_w_m2,
        observed_at,
        stale_after_seconds,
    )?;
    let stale = [
        &temperature_celsius,
        &wind_speed_mps,
        &precipitation_mm,
        &humidity_percent,
        &radiation_w_m2,
    ]
    .iter()
    .any(|value| value.stale);

    Ok(WeatherFreshnessAnnotatedRecord {
        forecast_id: record.forecast_id,
        field_ref: record.field_ref,
        valid_time: record.valid_time,
        source: record.source,
        fetched_at: record.fetched_at,
        temperature_celsius,
        wind_speed_mps,
        precipitation_mm,
        humidity_percent,
        radiation_w_m2,
        stale,
    })
}

fn parse_weather_freshness_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, WeatherFreshnessError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| WeatherFreshnessError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn ingest_weather_sensor_stream(
    request: WeatherSensorStreamIngestRequest,
) -> Result<WeatherSensorStreamIngest, WeatherSensorIngestError> {
    let sensor_id =
        normalize_weather_text(request.sensor_id).ok_or(WeatherSensorIngestError::EmptySensorId)?;
    let field_ref =
        normalize_weather_text(request.field_ref).ok_or(WeatherSensorIngestError::EmptyFieldRef)?;
    let fetched_at = normalize_weather_text(request.fetched_at)
        .ok_or(WeatherSensorIngestError::EmptyFetchedAt)?;
    let evaluated_at = normalize_weather_text(request.evaluated_at)
        .ok_or(WeatherSensorIngestError::EmptyEvaluatedAt)?;
    if request.stale_after_seconds <= 0 {
        return Err(WeatherSensorIngestError::InvalidStaleThreshold);
    }
    if request.max_gap_seconds <= 0 {
        return Err(WeatherSensorIngestError::InvalidGapThreshold);
    }
    if request.samples.is_empty() {
        return Err(WeatherSensorIngestError::EmptySamples);
    }
    parse_weather_sensor_timestamp(&fetched_at)?;
    parse_weather_sensor_timestamp(&evaluated_at)?;

    let mut samples = Vec::new();
    for sample in request.samples {
        let observed_at = normalize_weather_text(sample.observed_at.clone()).ok_or(
            WeatherSensorIngestError::InvalidTimestamp {
                timestamp: String::new(),
            },
        )?;
        let parsed_observed_at = parse_weather_sensor_timestamp(&observed_at)?;
        samples.push((sample, observed_at, parsed_observed_at));
    }
    samples.sort_by(|left, right| left.2.cmp(&right.2));

    let mut records = Vec::new();
    for (sample, observed_at, _) in &samples {
        let vars = WeatherForecastVariables {
            temperature_celsius: weather_value(
                "temperature_celsius",
                sample.temperature_celsius,
                "deg_c",
                "sensor",
                &fetched_at,
                observed_at,
                f64::is_finite,
            )?,
            wind_speed_mps: weather_value(
                "wind_speed_mps",
                sample.wind_speed_mps,
                "m/s",
                "sensor",
                &fetched_at,
                observed_at,
                |value| value.is_finite() && value >= 0.0,
            )?,
            precipitation_mm: weather_value(
                "precipitation_mm",
                sample.precipitation_mm,
                "mm",
                "sensor",
                &fetched_at,
                observed_at,
                |value| value.is_finite() && value >= 0.0,
            )?,
            humidity_percent: weather_value(
                "humidity_percent",
                sample.humidity_percent,
                "percent",
                "sensor",
                &fetched_at,
                observed_at,
                |value| value.is_finite() && (0.0..=100.0).contains(&value),
            )?,
            radiation_w_m2: weather_value(
                "radiation_w_m2",
                sample.radiation_w_m2,
                "W/m^2",
                "sensor",
                &fetched_at,
                observed_at,
                |value| value.is_finite() && value >= 0.0,
            )?,
        };
        records.push(WeatherForecastRecord {
            forecast_id: stable_weather_forecast_id(
                &field_ref,
                &format!("sensor-{sensor_id}"),
                observed_at,
            ),
            field_ref: field_ref.clone(),
            valid_time: observed_at.clone(),
            vars,
            source: "sensor".to_string(),
            fetched_at: fetched_at.clone(),
        });
    }

    let freshness = records
        .iter()
        .cloned()
        .map(|record| {
            annotate_weather_record_freshness(record, &evaluated_at, request.stale_after_seconds)
                .map_err(WeatherSensorIngestError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let gap_events =
        weather_sensor_gap_events(&sensor_id, &field_ref, &samples, request.max_gap_seconds);
    let stale = freshness.iter().any(|record| record.stale) || !gap_events.is_empty();

    Ok(WeatherSensorStreamIngest {
        sensor_id,
        field_ref,
        source: "sensor".to_string(),
        fetched_at,
        sample_count: records.len(),
        records,
        freshness,
        gap_events,
        stale,
    })
}

fn weather_sensor_gap_events(
    sensor_id: &str,
    field_ref: &str,
    samples: &[(WeatherSensorSample, String, chrono::DateTime<chrono::Utc>)],
    max_gap_seconds: i64,
) -> Vec<WeatherSensorGapEvent> {
    let mut events = Vec::new();
    for window in samples.windows(2) {
        let gap_seconds = window[1].2.signed_duration_since(window[0].2).num_seconds();
        if gap_seconds > max_gap_seconds {
            events.push(WeatherSensorGapEvent {
                sensor_id: sensor_id.to_string(),
                field_ref: field_ref.to_string(),
                from: window[0].1.clone(),
                to: window[1].1.clone(),
                gap_seconds,
                reason_code: "sensor_stream_gap".to_string(),
            });
        }
    }
    events
}

fn parse_weather_sensor_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, WeatherSensorIngestError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| WeatherSensorIngestError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn append_weather_history_records(
    mut existing: Vec<WeatherHistoryEntry>,
    records: Vec<WeatherFreshnessAnnotatedRecord>,
) -> Vec<WeatherHistoryEntry> {
    let mut next_sequence = existing
        .iter()
        .map(|entry| entry.sequence)
        .max()
        .unwrap_or(0)
        + 1;
    for record in records {
        existing.push(WeatherHistoryEntry {
            sequence: next_sequence,
            record,
        });
        next_sequence += 1;
    }
    existing
}

pub fn query_weather_history(
    entries: &[WeatherHistoryEntry],
    query: WeatherHistoryQuery,
) -> Result<WeatherHistoryQueryResult, WeatherHistoryError> {
    let field_ref =
        normalize_weather_text(query.field_ref).ok_or(WeatherHistoryError::EmptyFieldRef)?;
    if query.limit == 0 {
        return Err(WeatherHistoryError::InvalidLimit);
    }
    let start_time = parse_weather_history_timestamp(&query.start_time)?;
    let end_time = parse_weather_history_timestamp(&query.end_time)?;
    if end_time < start_time {
        return Err(WeatherHistoryError::InvalidDateRange);
    }

    let mut matching = entries
        .iter()
        .filter(|entry| entry.record.field_ref == field_ref)
        .filter_map(|entry| {
            parse_weather_history_timestamp(&entry.record.valid_time)
                .ok()
                .filter(|valid_time| *valid_time >= start_time && *valid_time <= end_time)
                .map(|_| entry.clone())
        })
        .collect::<Vec<_>>();
    matching.sort_by(|left, right| {
        left.record
            .valid_time
            .cmp(&right.record.valid_time)
            .then_with(|| left.sequence.cmp(&right.sequence))
    });
    let total_count = matching.len();
    let records = matching
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .collect::<Vec<_>>();

    Ok(WeatherHistoryQueryResult {
        field_ref,
        total_count,
        offset: query.offset,
        limit: query.limit,
        empty: total_count == 0,
        records,
    })
}

fn parse_weather_history_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, WeatherHistoryError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| WeatherHistoryError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn advise_weather_operational_windows(
    request: WeatherOperationalWindowRequest,
) -> Result<WeatherOperationalWindowReport, WeatherOperationalWindowError> {
    let field_ref = normalize_weather_text(request.field_ref)
        .ok_or(WeatherOperationalWindowError::EmptyFieldRef)?;
    validate_weather_operational_thresholds(&request.thresholds)?;
    if request.records.is_empty() {
        return Ok(WeatherOperationalWindowReport {
            field_ref,
            windows: Vec::new(),
            gaps: vec![WeatherOperationalWindowGap {
                reason_code: "missing_forecast_inputs".to_string(),
                details: "no annotated weather records were provided".to_string(),
            }],
        });
    }

    let mut records = request
        .records
        .into_iter()
        .filter(|record| record.field_ref == field_ref)
        .collect::<Vec<_>>();
    records.sort_by(|left, right| left.valid_time.cmp(&right.valid_time));
    if records.is_empty() {
        return Ok(WeatherOperationalWindowReport {
            field_ref,
            windows: Vec::new(),
            gaps: vec![WeatherOperationalWindowGap {
                reason_code: "missing_field_forecast_inputs".to_string(),
                details: "no annotated records matched the requested field".to_string(),
            }],
        });
    }
    let mut gaps = Vec::new();
    if records.iter().any(|record| record.stale) {
        for record in records.iter().filter(|record| record.stale) {
            gaps.push(WeatherOperationalWindowGap {
                reason_code: "stale_forecast_input".to_string(),
                details: format!("{} at {} is stale", record.forecast_id, record.valid_time),
            });
        }
        return Ok(WeatherOperationalWindowReport {
            field_ref,
            windows: Vec::new(),
            gaps,
        });
    }

    let mut windows = Vec::new();
    let mut current_start: Option<String> = None;
    let mut current_end: Option<String> = None;
    for record in &records {
        parse_weather_operational_window_timestamp(&record.valid_time)?;
        if weather_record_passes_operational_thresholds(record, &request.thresholds) {
            current_start.get_or_insert_with(|| record.valid_time.clone());
            current_end = Some(record.valid_time.clone());
        } else {
            if let (Some(start), Some(end)) = (current_start.take(), current_end.take()) {
                windows.push(weather_operational_window(
                    &field_ref,
                    start,
                    end,
                    &request.thresholds,
                ));
            }
            gaps.push(weather_operational_threshold_gap(
                record,
                &request.thresholds,
            ));
        }
    }
    if let (Some(start), Some(end)) = (current_start.take(), current_end.take()) {
        windows.push(weather_operational_window(
            &field_ref,
            start,
            end,
            &request.thresholds,
        ));
    }

    Ok(WeatherOperationalWindowReport {
        field_ref,
        windows,
        gaps,
    })
}

fn validate_weather_operational_thresholds(
    thresholds: &WeatherOperationalWindowThresholds,
) -> Result<(), WeatherOperationalWindowError> {
    if !thresholds.max_wind_speed_mps.is_finite()
        || thresholds.max_wind_speed_mps < 0.0
        || !thresholds.max_precipitation_mm.is_finite()
        || thresholds.max_precipitation_mm < 0.0
        || !thresholds.min_temperature_celsius.is_finite()
        || !thresholds.max_temperature_celsius.is_finite()
        || thresholds.min_temperature_celsius > thresholds.max_temperature_celsius
    {
        return Err(WeatherOperationalWindowError::InvalidThresholds);
    }
    Ok(())
}

fn weather_record_passes_operational_thresholds(
    record: &WeatherFreshnessAnnotatedRecord,
    thresholds: &WeatherOperationalWindowThresholds,
) -> bool {
    record.wind_speed_mps.value.value <= thresholds.max_wind_speed_mps
        && record.precipitation_mm.value.value <= thresholds.max_precipitation_mm
        && record.temperature_celsius.value.value >= thresholds.min_temperature_celsius
        && record.temperature_celsius.value.value <= thresholds.max_temperature_celsius
}

fn weather_operational_threshold_gap(
    record: &WeatherFreshnessAnnotatedRecord,
    thresholds: &WeatherOperationalWindowThresholds,
) -> WeatherOperationalWindowGap {
    let mut failures = Vec::new();
    if record.wind_speed_mps.value.value > thresholds.max_wind_speed_mps {
        failures.push(format!(
            "wind_speed_mps:{}>{}",
            record.wind_speed_mps.value.value, thresholds.max_wind_speed_mps
        ));
    }
    if record.precipitation_mm.value.value > thresholds.max_precipitation_mm {
        failures.push(format!(
            "precipitation_mm:{}>{}",
            record.precipitation_mm.value.value, thresholds.max_precipitation_mm
        ));
    }
    if record.temperature_celsius.value.value < thresholds.min_temperature_celsius
        || record.temperature_celsius.value.value > thresholds.max_temperature_celsius
    {
        failures.push(format!(
            "temperature_celsius:{} outside {}..{}",
            record.temperature_celsius.value.value,
            thresholds.min_temperature_celsius,
            thresholds.max_temperature_celsius
        ));
    }
    WeatherOperationalWindowGap {
        reason_code: "threshold_exceeded".to_string(),
        details: format!("{}: {}", record.valid_time, failures.join(",")),
    }
}

fn weather_operational_window(
    field_ref: &str,
    start: String,
    end: String,
    thresholds: &WeatherOperationalWindowThresholds,
) -> WeatherOperationalWindow {
    WeatherOperationalWindow {
        field_ref: field_ref.to_string(),
        start,
        end,
        gating_vars: vec![
            "wind_speed_mps".to_string(),
            "precipitation_mm".to_string(),
            "temperature_celsius".to_string(),
        ],
        thresholds: vec![
            format!("max_wind_speed_mps:{}", thresholds.max_wind_speed_mps),
            format!("max_precipitation_mm:{}", thresholds.max_precipitation_mm),
            format!(
                "temperature_celsius:{}..{}",
                thresholds.min_temperature_celsius, thresholds.max_temperature_celsius
            ),
        ],
        freshness: vec![WeatherFreshnessState::Fresh],
    }
}

fn parse_weather_operational_window_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, WeatherOperationalWindowError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| WeatherOperationalWindowError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn evaluate_weather_risk_alerts(
    records: &[WeatherFreshnessAnnotatedRecord],
    thresholds: WeatherRiskThresholds,
) -> Result<Vec<WeatherRiskAlert>, WeatherRiskAlertError> {
    validate_weather_risk_thresholds(&thresholds)?;
    let mut alerts = Vec::new();
    for record in records {
        if record.temperature_celsius.value.value <= thresholds.frost_temperature_celsius {
            alerts.push(weather_risk_alert(
                record,
                WeatherRiskType::Frost,
                record.temperature_celsius.value.value,
                thresholds.frost_temperature_celsius,
                record.temperature_celsius.freshness_state,
            ));
        }
        if record.temperature_celsius.value.value >= thresholds.heat_temperature_celsius {
            alerts.push(weather_risk_alert(
                record,
                WeatherRiskType::Heat,
                record.temperature_celsius.value.value,
                thresholds.heat_temperature_celsius,
                record.temperature_celsius.freshness_state,
            ));
        }
        if record.wind_speed_mps.value.value >= thresholds.wind_speed_mps {
            alerts.push(weather_risk_alert(
                record,
                WeatherRiskType::Wind,
                record.wind_speed_mps.value.value,
                thresholds.wind_speed_mps,
                record.wind_speed_mps.freshness_state,
            ));
        }
        if record.precipitation_mm.value.value >= thresholds.precipitation_mm {
            alerts.push(weather_risk_alert(
                record,
                WeatherRiskType::Precipitation,
                record.precipitation_mm.value.value,
                thresholds.precipitation_mm,
                record.precipitation_mm.freshness_state,
            ));
        }
    }
    alerts.sort_by(|left, right| {
        left.valid_time
            .cmp(&right.valid_time)
            .then_with(|| format!("{:?}", left.risk_type).cmp(&format!("{:?}", right.risk_type)))
    });
    Ok(alerts)
}

fn validate_weather_risk_thresholds(
    thresholds: &WeatherRiskThresholds,
) -> Result<(), WeatherRiskAlertError> {
    if !thresholds.frost_temperature_celsius.is_finite()
        || !thresholds.heat_temperature_celsius.is_finite()
        || thresholds.frost_temperature_celsius > thresholds.heat_temperature_celsius
        || !thresholds.wind_speed_mps.is_finite()
        || thresholds.wind_speed_mps < 0.0
        || !thresholds.precipitation_mm.is_finite()
        || thresholds.precipitation_mm < 0.0
    {
        return Err(WeatherRiskAlertError::InvalidThresholds);
    }
    Ok(())
}

fn weather_risk_alert(
    record: &WeatherFreshnessAnnotatedRecord,
    risk_type: WeatherRiskType,
    value: f64,
    threshold: f64,
    freshness: WeatherFreshnessState,
) -> WeatherRiskAlert {
    WeatherRiskAlert {
        field_ref: record.field_ref.clone(),
        risk_type,
        value,
        threshold,
        valid_time: record.valid_time.clone(),
        source: record.source.clone(),
        freshness,
    }
}

pub fn compute_weather_growing_degree_day(
    request: WeatherGrowingDegreeDayRequest,
) -> Result<WeatherGrowingDegreeDay, WeatherGrowingDegreeDayError> {
    let field_ref = normalize_weather_text(request.field_ref)
        .ok_or(WeatherGrowingDegreeDayError::EmptyFieldRef)?;
    if !request.base_temperature_celsius.is_finite() {
        return Err(WeatherGrowingDegreeDayError::InvalidBaseTemperature);
    }
    let date = NaiveDate::parse_from_str(&request.date, "%Y-%m-%d").map_err(|_| {
        WeatherGrowingDegreeDayError::InvalidDate {
            date: request.date.clone(),
        }
    })?;

    let mut temperatures = Vec::new();
    let mut evidence_refs = Vec::new();
    for record in request.records {
        if record.field_ref != field_ref {
            continue;
        }
        let valid_time = parse_weather_gdd_timestamp(&record.valid_time)?;
        if valid_time.date_naive() != date {
            continue;
        }
        temperatures.push(record.temperature_celsius.value.value);
        evidence_refs.push(format!(
            "{}:{}:{}",
            record.forecast_id, record.valid_time, record.source
        ));
    }

    let method = "simple_average_max_min_minus_base_celsius".to_string();
    if temperatures.is_empty() {
        return Ok(WeatherGrowingDegreeDay {
            field_ref,
            date: request.date,
            status: WeatherGrowingDegreeDayStatus::NoData,
            gdd_celsius_days: None,
            min_temperature_celsius: None,
            max_temperature_celsius: None,
            base_temperature_celsius: request.base_temperature_celsius,
            method,
            evidence_refs: vec!["temperature:no_data".to_string()],
        });
    }

    let min_temperature_celsius = temperatures.iter().copied().fold(f64::INFINITY, f64::min);
    let max_temperature_celsius = temperatures
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let gdd_celsius_days = (((min_temperature_celsius + max_temperature_celsius) / 2.0)
        - request.base_temperature_celsius)
        .max(0.0);

    Ok(WeatherGrowingDegreeDay {
        field_ref,
        date: request.date,
        status: WeatherGrowingDegreeDayStatus::Computed,
        gdd_celsius_days: Some(gdd_celsius_days),
        min_temperature_celsius: Some(min_temperature_celsius),
        max_temperature_celsius: Some(max_temperature_celsius),
        base_temperature_celsius: request.base_temperature_celsius,
        method,
        evidence_refs,
    })
}

fn parse_weather_gdd_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, WeatherGrowingDegreeDayError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| WeatherGrowingDegreeDayError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn validate_water_weather_input_contract(
    request: WaterWeatherInputContractRequest,
) -> Result<WaterWeatherInputContract, WaterWeatherInputContractError> {
    let field_ref = normalize_weather_text(request.field_ref)
        .ok_or(WaterWeatherInputContractError::EmptyFieldRef)?;
    let date = NaiveDate::parse_from_str(&request.date, "%Y-%m-%d").map_err(|_| {
        WaterWeatherInputContractError::InvalidDate {
            date: request.date.clone(),
        }
    })?;

    let mut records = request
        .records
        .into_iter()
        .filter(|record| record.field_ref == field_ref)
        .map(|record| {
            parse_water_weather_input_timestamp(&record.valid_time)
                .map(|timestamp| (timestamp, record))
        })
        .collect::<Result<Vec<_>, _>>()?;
    records.retain(|(timestamp, _)| timestamp.date_naive() == date);
    records.sort_by(|left, right| left.0.cmp(&right.0));

    let Some((_, record)) = records.into_iter().next() else {
        return Ok(WaterWeatherInputContract {
            field_ref,
            date: date.to_string(),
            status: WaterWeatherInputStatus::Degraded,
            temperature_celsius: None,
            humidity_percent: None,
            wind_speed_mps: None,
            radiation_w_m2: None,
            precipitation_mm: None,
            et_blocked: true,
            degradation_reasons: vec!["missing_weather_input".to_string()],
            evidence_refs: Vec::new(),
        });
    };

    let mut degradation_reasons = Vec::new();
    if record.stale
        || record.temperature_celsius.stale
        || record.humidity_percent.stale
        || record.wind_speed_mps.stale
        || record.radiation_w_m2.stale
        || record.precipitation_mm.stale
    {
        degradation_reasons.push("stale_weather_input".to_string());
    }
    let status = if degradation_reasons.is_empty() {
        WaterWeatherInputStatus::Valid
    } else {
        WaterWeatherInputStatus::Degraded
    };

    Ok(WaterWeatherInputContract {
        field_ref,
        date: date.to_string(),
        status,
        temperature_celsius: Some(record.temperature_celsius),
        humidity_percent: Some(record.humidity_percent),
        wind_speed_mps: Some(record.wind_speed_mps),
        radiation_w_m2: Some(record.radiation_w_m2),
        precipitation_mm: Some(record.precipitation_mm),
        et_blocked: status == WaterWeatherInputStatus::Degraded,
        degradation_reasons,
        evidence_refs: vec![format!("weather_record:{}", record.forecast_id)],
    })
}

fn parse_water_weather_input_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, WaterWeatherInputContractError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| WaterWeatherInputContractError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn compute_water_evapotranspiration(
    request: WaterEvapotranspirationRequest,
) -> Result<WaterEvapotranspiration, WaterEvapotranspirationError> {
    let crop_coefficient = request.crop_coefficient.unwrap_or(1.0);
    if !(crop_coefficient.is_finite() && crop_coefficient >= 0.0) {
        return Err(WaterEvapotranspirationError::InvalidCropCoefficient {
            value: crop_coefficient.to_string(),
        });
    }
    let method = "agbot_water_et_v1_reference_et_crop_coefficient".to_string();
    if request.weather.et_blocked || request.weather.status == WaterWeatherInputStatus::Degraded {
        return Ok(water_et_insufficient(
            request.weather.field_ref,
            request.weather.date,
            crop_coefficient,
            method,
            request.weather.degradation_reasons,
        ));
    }

    let reference = compute_weather_reference_et(WeatherReferenceEtInput {
        field_ref: request.weather.field_ref.clone(),
        date: request.weather.date.clone(),
        temperature_celsius: request.weather.temperature_celsius,
        humidity_percent: request.weather.humidity_percent,
        wind_speed_mps: request.weather.wind_speed_mps,
        radiation_w_m2: request.weather.radiation_w_m2,
    })?;
    let Some(reference_et_mm_day) = reference.reference_et_mm_day else {
        return Ok(water_et_insufficient(
            request.weather.field_ref,
            request.weather.date,
            crop_coefficient,
            method,
            reference.input_refs,
        ));
    };

    Ok(WaterEvapotranspiration {
        field_ref: reference.field_ref,
        date: reference.date,
        status: WaterEvapotranspirationStatus::Computed,
        reference_et_mm_day: Some(reference_et_mm_day),
        crop_et_mm_day: Some(reference_et_mm_day * crop_coefficient),
        crop_coefficient,
        method,
        input_refs: reference.input_refs,
        degradation_reasons: Vec::new(),
    })
}

fn water_et_insufficient(
    field_ref: String,
    date: String,
    crop_coefficient: f64,
    method: String,
    degradation_reasons: Vec<String>,
) -> WaterEvapotranspiration {
    WaterEvapotranspiration {
        field_ref,
        date,
        status: WaterEvapotranspirationStatus::InsufficientInputs,
        reference_et_mm_day: None,
        crop_et_mm_day: None,
        crop_coefficient,
        method,
        input_refs: Vec::new(),
        degradation_reasons,
    }
}

pub fn map_zone_water_need(
    request: ZoneWaterNeedRequest,
) -> Result<Vec<ZoneWaterNeed>, ZoneWaterNeedError> {
    let field_ref =
        normalize_weather_text(request.field_ref).ok_or(ZoneWaterNeedError::EmptyFieldRef)?;
    let request_crs = normalize_weather_text(request.crs).ok_or(ZoneWaterNeedError::EmptyCrs)?;
    if request.zones.is_empty() {
        return Err(ZoneWaterNeedError::MissingZones);
    }

    let crop_et = request.evapotranspiration.crop_et_mm_day;
    let et_ready = request.evapotranspiration.status == WaterEvapotranspirationStatus::Computed;
    let mut results = Vec::with_capacity(request.zones.len());
    for zone in request.zones {
        let zone_ref =
            normalize_weather_text(zone.zone_ref).ok_or(ZoneWaterNeedError::EmptyZoneRef)?;
        let zone_crs = normalize_weather_text(zone.crs).ok_or(ZoneWaterNeedError::EmptyCrs)?;
        if zone_crs != request_crs {
            return Err(ZoneWaterNeedError::CrsMismatch {
                zone_ref,
                zone_crs,
                request_crs,
            });
        }

        let evidence =
            zone_moisture_evidence(&zone_ref, &request.soil_readings, &request.moisture_proxies);
        let Some((moisture_fraction, evidence_refs)) = evidence else {
            results.push(zone_water_need_insufficient(
                &field_ref,
                &zone_ref,
                &request_crs,
                "missing_moisture_evidence",
            ));
            continue;
        };
        let Some(crop_et_mm_day) = crop_et.filter(|value| et_ready && value.is_finite()) else {
            results.push(zone_water_need_insufficient(
                &field_ref,
                &zone_ref,
                &request_crs,
                "missing_et_evidence",
            ));
            continue;
        };
        let water_need_mm = (crop_et_mm_day * (1.0 - moisture_fraction)).max(0.0);
        let mut evidence_refs = evidence_refs;
        evidence_refs.extend(request.evapotranspiration.input_refs.clone());
        results.push(ZoneWaterNeed {
            field_ref: field_ref.clone(),
            zone_ref,
            crs: request_crs.clone(),
            status: ZoneWaterNeedStatus::Computed,
            water_need_mm: Some(water_need_mm),
            evidence_refs,
            reason_code: "computed_from_moisture_and_et".to_string(),
        });
    }

    Ok(results)
}

fn zone_moisture_evidence(
    zone_ref: &str,
    soil_readings: &[SoilMoistureReadingRecord],
    moisture_proxies: &[RemoteSensingMoistureProxyRecord],
) -> Option<(f64, Vec<String>)> {
    soil_readings
        .iter()
        .find(|reading| {
            reading.zone_ref == zone_ref && reading.qa_flag != SoilMoistureQaFlag::Invalid
        })
        .map(|reading| {
            (
                (reading.value / 100.0).clamp(0.0, 1.0),
                vec![format!("soil_moisture:{}", reading.reading_id)],
            )
        })
        .or_else(|| {
            moisture_proxies
                .iter()
                .find(|proxy| proxy.zone_ref == zone_ref)
                .map(|proxy| {
                    (
                        ((proxy.value + 1.0) / 2.0).clamp(0.0, 1.0),
                        vec![format!("moisture_proxy:{}", proxy.proxy_id)],
                    )
                })
        })
}

fn zone_water_need_insufficient(
    field_ref: &str,
    zone_ref: &str,
    crs: &str,
    reason_code: &str,
) -> ZoneWaterNeed {
    ZoneWaterNeed {
        field_ref: field_ref.to_string(),
        zone_ref: zone_ref.to_string(),
        crs: crs.to_string(),
        status: ZoneWaterNeedStatus::InsufficientEvidence,
        water_need_mm: None,
        evidence_refs: Vec::new(),
        reason_code: reason_code.to_string(),
    }
}

pub fn schedule_irrigation_plan(
    request: IrrigationScheduleRequest,
) -> Result<IrrigationSchedule, IrrigationScheduleError> {
    let field_ref =
        normalize_weather_text(request.field_ref).ok_or(IrrigationScheduleError::EmptyFieldRef)?;
    let generated_at = normalize_weather_text(request.generated_at)
        .ok_or(IrrigationScheduleError::EmptyGeneratedAt)?;
    parse_irrigation_schedule_timestamp(&generated_at)?;
    let scheduled_start = normalize_weather_text(request.scheduled_start)
        .ok_or(IrrigationScheduleError::EmptyScheduledStart)?;
    let mut next_start = parse_irrigation_schedule_timestamp(&scheduled_start)?;
    if !(request.application_rate_mm_per_hour.is_finite()
        && request.application_rate_mm_per_hour > 0.0)
    {
        return Err(IrrigationScheduleError::InvalidApplicationRate);
    }

    let mut needs = request.water_needs;
    needs.sort_by(|left, right| left.zone_ref.cmp(&right.zone_ref));
    let mut entries = Vec::new();
    let mut exclusions = Vec::new();
    for need in needs {
        if need.status != ZoneWaterNeedStatus::Computed {
            exclusions.push(IrrigationScheduleExclusion {
                zone_ref: need.zone_ref,
                reason_code: need.reason_code,
            });
            continue;
        }
        let Some(amount_mm) = need
            .water_need_mm
            .filter(|value| value.is_finite() && *value > 0.0)
        else {
            exclusions.push(IrrigationScheduleExclusion {
                zone_ref: need.zone_ref,
                reason_code: "non_positive_water_need".to_string(),
            });
            continue;
        };
        let duration_minutes =
            ((amount_mm / request.application_rate_mm_per_hour) * 60.0).ceil() as u32;
        let start_time = next_start.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        next_start += chrono::Duration::minutes(duration_minutes as i64);
        entries.push(IrrigationScheduleEntry {
            zone_ref: need.zone_ref,
            amount_mm,
            start_time,
            duration_minutes,
            evidence_refs: need.evidence_refs,
        });
    }

    Ok(IrrigationSchedule {
        field_ref,
        generated_at,
        entries,
        exclusions,
        method: "agbot_irrigation_schedule_v1_sequential_zone_water_need".to_string(),
    })
}

fn parse_irrigation_schedule_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, IrrigationScheduleError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| IrrigationScheduleError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn dry_run_irrigation_valve_plan(
    request: IrrigationValveDryRunRequest,
) -> Result<IrrigationValveDryRun, IrrigationValveControlError> {
    let dry_run_id = normalize_weather_text(request.dry_run_id)
        .ok_or(IrrigationValveControlError::EmptyDryRunId)?;
    let checked_at = normalize_weather_text(request.checked_at)
        .ok_or(IrrigationValveControlError::EmptyTimestamp)?;
    parse_irrigation_valve_timestamp(&checked_at)?;
    let valves = normalize_irrigation_valve_specs(request.valves)?;

    let mut audits = Vec::new();
    for entry in &request.schedule.entries {
        let Some(valve) = valves.iter().find(|valve| valve.zone_ref == entry.zone_ref) else {
            audits.push(irrigation_valve_audit(
                entry,
                IrrigationValveActionStatus::Rejected,
                "valve_not_found",
                &checked_at,
            ));
            continue;
        };
        let within_bounds = entry.amount_mm >= valve.min_amount_mm
            && entry.amount_mm <= valve.max_amount_mm
            && entry.duration_minutes <= valve.max_duration_minutes;
        if within_bounds {
            audits.push(irrigation_valve_audit(
                entry,
                IrrigationValveActionStatus::Planned,
                "dry_run_passed",
                &checked_at,
            ));
        } else {
            audits.push(irrigation_valve_audit(
                entry,
                IrrigationValveActionStatus::Rejected,
                "valve_bounds_exceeded",
                &checked_at,
            ));
        }
    }

    let status = if audits
        .iter()
        .all(|audit| audit.status == IrrigationValveActionStatus::Planned)
    {
        IrrigationValveDryRunStatus::Passed
    } else {
        IrrigationValveDryRunStatus::Rejected
    };

    Ok(IrrigationValveDryRun {
        dry_run_id,
        field_ref: request.schedule.field_ref.clone(),
        status,
        checked_at,
        schedule_fingerprint: irrigation_schedule_fingerprint(&request.schedule),
        audits,
    })
}

pub fn execute_irrigation_valve_plan(
    request: IrrigationValveExecuteRequest,
) -> Result<IrrigationValveExecution, IrrigationValveControlError> {
    let executed_at = normalize_weather_text(request.executed_at)
        .ok_or(IrrigationValveControlError::EmptyTimestamp)?;
    parse_irrigation_valve_timestamp(&executed_at)?;
    let expected_fingerprint = irrigation_schedule_fingerprint(&request.schedule);
    let rejection_reason = if request.abort_requested {
        Some("operator_abort")
    } else if request.dry_run.status != IrrigationValveDryRunStatus::Passed {
        Some("dry_run_not_passed")
    } else if request.dry_run.schedule_fingerprint != expected_fingerprint {
        Some("dry_run_schedule_mismatch")
    } else {
        None
    };

    let audits = if let Some(reason_code) = rejection_reason {
        request
            .schedule
            .entries
            .iter()
            .map(|entry| {
                irrigation_valve_audit(
                    entry,
                    IrrigationValveActionStatus::Rejected,
                    reason_code,
                    &executed_at,
                )
            })
            .collect::<Vec<_>>()
    } else {
        request
            .schedule
            .entries
            .iter()
            .map(|entry| {
                irrigation_valve_audit(
                    entry,
                    IrrigationValveActionStatus::Applied,
                    "valve_action_applied",
                    &executed_at,
                )
            })
            .collect::<Vec<_>>()
    };
    let status = if rejection_reason.is_some() {
        IrrigationValveExecutionStatus::Rejected
    } else {
        IrrigationValveExecutionStatus::Applied
    };

    Ok(IrrigationValveExecution {
        field_ref: request.schedule.field_ref,
        status,
        executed_at,
        dry_run_id: request.dry_run.dry_run_id,
        audits,
    })
}

fn normalize_irrigation_valve_specs(
    valves: Vec<IrrigationValveSpec>,
) -> Result<Vec<IrrigationValveSpec>, IrrigationValveControlError> {
    valves
        .into_iter()
        .map(|valve| {
            let zone_ref = normalize_weather_text(valve.zone_ref)
                .ok_or(IrrigationValveControlError::EmptyZoneRef)?;
            if !(valve.min_amount_mm.is_finite()
                && valve.max_amount_mm.is_finite()
                && valve.min_amount_mm >= 0.0
                && valve.max_amount_mm >= valve.min_amount_mm
                && valve.max_duration_minutes > 0)
            {
                return Err(IrrigationValveControlError::InvalidBounds { zone_ref });
            }
            Ok(IrrigationValveSpec {
                zone_ref,
                min_amount_mm: valve.min_amount_mm,
                max_amount_mm: valve.max_amount_mm,
                max_duration_minutes: valve.max_duration_minutes,
            })
        })
        .collect()
}

fn irrigation_valve_audit(
    entry: &IrrigationScheduleEntry,
    status: IrrigationValveActionStatus,
    reason_code: &str,
    at: &str,
) -> IrrigationValveActionAudit {
    IrrigationValveActionAudit {
        zone_ref: entry.zone_ref.clone(),
        amount_mm: entry.amount_mm,
        duration_minutes: entry.duration_minutes,
        status,
        reason_code: reason_code.to_string(),
        at: at.to_string(),
        evidence_refs: entry.evidence_refs.clone(),
    }
}

fn irrigation_schedule_fingerprint(schedule: &IrrigationSchedule) -> String {
    let mut parts = vec![format!("field:{}", schedule.field_ref)];
    parts.extend(schedule.entries.iter().map(|entry| {
        format!(
            "{}:{:.6}:{}:{}",
            entry.zone_ref, entry.amount_mm, entry.duration_minutes, entry.start_time
        )
    }));
    parts.join("|")
}

fn parse_irrigation_valve_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, IrrigationValveControlError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| IrrigationValveControlError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn compute_weather_reference_et(
    input: WeatherReferenceEtInput,
) -> Result<WeatherReferenceEt, WeatherReferenceEtError> {
    let field_ref =
        normalize_weather_text(input.field_ref).ok_or(WeatherReferenceEtError::EmptyFieldRef)?;
    let date = NaiveDate::parse_from_str(&input.date, "%Y-%m-%d").map_err(|_| {
        WeatherReferenceEtError::InvalidDate {
            date: input.date.clone(),
        }
    })?;
    let method = "agbot_reference_et_v1_radiation_temperature_humidity_wind".to_string();

    let Some(temperature) = input.temperature_celsius else {
        return Ok(weather_reference_et_insufficient(
            field_ref,
            input.date,
            method,
            "temperature_celsius",
        ));
    };
    let Some(humidity) = input.humidity_percent else {
        return Ok(weather_reference_et_insufficient(
            field_ref,
            input.date,
            method,
            "humidity_percent",
        ));
    };
    let Some(wind) = input.wind_speed_mps else {
        return Ok(weather_reference_et_insufficient(
            field_ref,
            input.date,
            method,
            "wind_speed_mps",
        ));
    };
    let Some(radiation) = input.radiation_w_m2 else {
        return Ok(weather_reference_et_insufficient(
            field_ref,
            input.date,
            method,
            "radiation_w_m2",
        ));
    };

    let radiation_mj_m2_day = radiation.value.value * 0.0864;
    let reference_et_mm_day =
        (radiation_mj_m2_day * 0.12 + temperature.value.value * 0.03 + wind.value.value * 0.15
            - humidity.value.value * 0.02)
            .max(0.0);

    Ok(WeatherReferenceEt {
        field_ref,
        date: date.to_string(),
        status: WeatherReferenceEtStatus::Computed,
        reference_et_mm_day: Some(reference_et_mm_day),
        method,
        input_refs: vec![
            format!("temperature_celsius:{}", temperature.value.valid_time),
            format!("humidity_percent:{}", humidity.value.valid_time),
            format!("wind_speed_mps:{}", wind.value.valid_time),
            format!("radiation_w_m2:{}", radiation.value.valid_time),
        ],
        freshness: vec![
            temperature.freshness_state,
            humidity.freshness_state,
            wind.freshness_state,
            radiation.freshness_state,
        ],
    })
}

fn weather_reference_et_insufficient(
    field_ref: String,
    date: String,
    method: String,
    missing_input: &str,
) -> WeatherReferenceEt {
    WeatherReferenceEt {
        field_ref,
        date,
        status: WeatherReferenceEtStatus::InsufficientInputs,
        reference_et_mm_day: None,
        method,
        input_refs: vec![format!("missing:{missing_input}")],
        freshness: Vec::new(),
    }
}

pub fn route_weather_alert(
    request: WeatherAlertRoutingRequest,
) -> Result<WeatherAlertRoutingResult, WeatherAlertRoutingError> {
    let recipient_id = normalize_weather_text(request.recipient_id)
        .ok_or(WeatherAlertRoutingError::EmptyRecipientId)?;
    let routed_at =
        normalize_weather_text(request.routed_at).ok_or(WeatherAlertRoutingError::EmptyRoutedAt)?;
    parse_weather_alert_routing_timestamp(&routed_at)?;
    if request.targets.is_empty() {
        return Err(WeatherAlertRoutingError::EmptyTargets);
    }
    let field_owned = request
        .owned_field_refs
        .iter()
        .filter_map(|field_ref| normalize_weather_text(field_ref.clone()))
        .any(|field_ref| field_ref == request.alert.field_ref);

    let mut audits = Vec::new();
    for target in request.targets {
        let (status, reason_code) = if !field_owned {
            (
                WeatherAlertDeliveryStatus::Rejected,
                "field_scope_not_owned",
            )
        } else if target.reachable {
            (WeatherAlertDeliveryStatus::Delivered, "alert_delivered")
        } else {
            (
                WeatherAlertDeliveryStatus::Queued,
                "target_unreachable_queued",
            )
        };
        audits.push(WeatherAlertDeliveryAudit {
            target: target.target,
            status,
            reason_code: reason_code.to_string(),
            recipient_id: recipient_id.clone(),
            field_ref: request.alert.field_ref.clone(),
            routed_at: routed_at.clone(),
            evidence_payload: weather_alert_evidence_payload(&request.alert),
        });
    }

    Ok(WeatherAlertRoutingResult {
        delivered_count: audits
            .iter()
            .filter(|audit| audit.status == WeatherAlertDeliveryStatus::Delivered)
            .count(),
        queued_count: audits
            .iter()
            .filter(|audit| audit.status == WeatherAlertDeliveryStatus::Queued)
            .count(),
        rejected_count: audits
            .iter()
            .filter(|audit| audit.status == WeatherAlertDeliveryStatus::Rejected)
            .count(),
        audits,
    })
}

fn weather_alert_evidence_payload(alert: &WeatherRiskAlert) -> Vec<String> {
    vec![
        format!("risk_type:{:?}", alert.risk_type),
        format!("value:{}", alert.value),
        format!("threshold:{}", alert.threshold),
        format!("valid_time:{}", alert.valid_time),
        format!("source:{}", alert.source),
        format!("freshness:{:?}", alert.freshness),
    ]
}

fn parse_weather_alert_routing_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, WeatherAlertRoutingError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| WeatherAlertRoutingError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn evaluate_crop_stage_weather_risks(
    request: WeatherCropStageRiskRequest,
) -> Result<Vec<WeatherCropStageRiskAlert>, WeatherCropStageRiskError> {
    let field_ref = normalize_weather_text(request.field_ref)
        .ok_or(WeatherCropStageRiskError::EmptyFieldRef)?;
    let requested_stage = request
        .crop_stage
        .and_then(normalize_weather_text)
        .unwrap_or_else(|| "unknown".to_string());
    let selected = request
        .stage_thresholds
        .iter()
        .find(|thresholds| thresholds.crop_stage == requested_stage);
    let (thresholds, threshold_set_name, crop_stage, fallback_applied) =
        if let Some(selected) = selected {
            (
                selected.thresholds.clone(),
                selected.threshold_set_name.clone(),
                selected.crop_stage.clone(),
                false,
            )
        } else {
            (
                request.default_thresholds,
                "default_thresholds".to_string(),
                requested_stage,
                true,
            )
        };
    let records = request
        .records
        .into_iter()
        .filter(|record| record.field_ref == field_ref)
        .collect::<Vec<_>>();
    let alerts = evaluate_weather_risk_alerts(&records, thresholds)?;

    Ok(alerts
        .into_iter()
        .map(|alert| WeatherCropStageRiskAlert {
            alert,
            crop_stage: crop_stage.clone(),
            threshold_set_name: threshold_set_name.clone(),
            fallback_applied,
        })
        .collect())
}

pub fn verify_weather_forecast_accuracy(
    request: WeatherForecastVerificationRequest,
) -> WeatherForecastVerification {
    let matching = request.observations.into_iter().find(|observation| {
        observation.field_ref == request.forecast.field_ref
            && observation.valid_time == request.forecast.valid_time
    });
    let Some(observation) = matching else {
        return WeatherForecastVerification {
            field_ref: request.forecast.field_ref,
            source: request.forecast.source,
            valid_time: request.forecast.valid_time,
            status: WeatherForecastVerificationStatus::NotVerifiable,
            metrics: Vec::new(),
            evidence_refs: vec!["observation:not_found".to_string()],
        };
    };

    let metrics = vec![
        weather_forecast_error_metric(
            "temperature_celsius",
            &request.forecast.vars.temperature_celsius,
            &observation.temperature_celsius.value,
        ),
        weather_forecast_error_metric(
            "wind_speed_mps",
            &request.forecast.vars.wind_speed_mps,
            &observation.wind_speed_mps.value,
        ),
        weather_forecast_error_metric(
            "precipitation_mm",
            &request.forecast.vars.precipitation_mm,
            &observation.precipitation_mm.value,
        ),
        weather_forecast_error_metric(
            "humidity_percent",
            &request.forecast.vars.humidity_percent,
            &observation.humidity_percent.value,
        ),
        weather_forecast_error_metric(
            "radiation_w_m2",
            &request.forecast.vars.radiation_w_m2,
            &observation.radiation_w_m2.value,
        ),
    ];

    WeatherForecastVerification {
        field_ref: request.forecast.field_ref,
        source: request.forecast.source,
        valid_time: request.forecast.valid_time,
        status: WeatherForecastVerificationStatus::Verified,
        metrics,
        evidence_refs: vec![
            format!("forecast:{}", request.forecast.forecast_id),
            format!("observation:{}", observation.forecast_id),
        ],
    }
}

fn weather_forecast_error_metric(
    variable: &str,
    forecast: &WeatherForecastValue,
    observation: &WeatherForecastValue,
) -> WeatherForecastErrorMetric {
    WeatherForecastErrorMetric {
        variable: variable.to_string(),
        forecast_value: forecast.value,
        observed_value: observation.value,
        absolute_error: (forecast.value - observation.value).abs(),
        unit: forecast.unit.clone(),
    }
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
pub enum RemoteSensingMoistureIndex {
    Ndwi,
    Ndmi,
}

impl RemoteSensingMoistureIndex {
    pub fn as_str(self) -> &'static str {
        match self {
            RemoteSensingMoistureIndex::Ndwi => "ndwi",
            RemoteSensingMoistureIndex::Ndmi => "ndmi",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteSensingMoistureZoneValue {
    pub zone_ref: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteSensingMoistureProxyLayer {
    pub layer_id: String,
    pub field_id: String,
    pub index: RemoteSensingMoistureIndex,
    pub source: String,
    pub captured_at: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub spatial_ref: Option<RasterSpatialRef>,
    pub zone_values: Vec<RemoteSensingMoistureZoneValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteSensingMoistureProxyRecord {
    pub proxy_id: String,
    pub field_id: String,
    pub zone_ref: String,
    pub value: f64,
    pub index: RemoteSensingMoistureIndex,
    pub source: String,
    pub captured_at: String,
    pub evidence_kind: String,
    pub crs: String,
    pub extent: GeoBounds,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RemoteSensingMoistureProxyError {
    #[error("remote-sensing moisture proxy layer_id cannot be empty")]
    EmptyLayerId,
    #[error("remote-sensing moisture proxy field_id cannot be empty")]
    EmptyFieldId,
    #[error("remote-sensing moisture proxy source cannot be empty")]
    EmptySource,
    #[error("remote-sensing moisture proxy captured_at cannot be empty")]
    EmptyCapturedAt,
    #[error("remote-sensing moisture proxy field mismatch: {field_id}")]
    FieldMismatch { field_id: String },
    #[error("remote-sensing moisture proxy CRS mismatch: layer {layer_crs}, field {field_crs}")]
    CrsMismatch {
        layer_crs: String,
        field_crs: String,
    },
    #[error("remote-sensing moisture proxy extent mismatch")]
    ExtentMismatch,
    #[error("remote-sensing moisture proxy requires at least one zone value")]
    MissingZoneValues,
    #[error("remote-sensing moisture proxy zone_ref cannot be empty")]
    EmptyZoneRef,
    #[error("remote-sensing moisture proxy value is invalid: {value}")]
    InvalidValue { value: String },
    #[error("remote-sensing moisture proxy spatial reference invalid: {0}")]
    SpatialRef(#[from] RasterSpatialRefError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationEventRequest {
    #[serde(default)]
    pub event_id: Option<String>,
    pub field_id: String,
    pub zone_ref: String,
    pub applied_amount_mm: f64,
    pub source: String,
    pub timestamp: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationEventRecord {
    pub event_id: String,
    pub field_id: String,
    pub zone_ref: String,
    pub applied_amount_mm: f64,
    pub source: String,
    pub timestamp: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrrigationHistoryQuery {
    pub field_id: String,
    pub start_time: String,
    pub end_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationHistoryQueryResult {
    pub field_id: String,
    pub total_count: usize,
    pub empty: bool,
    pub events: Vec<IrrigationEventRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IrrigationHistoryError {
    #[error("irrigation event_id cannot be empty")]
    EmptyEventId,
    #[error("irrigation field_id cannot be empty")]
    EmptyFieldId,
    #[error("irrigation zone_ref cannot be empty")]
    EmptyZoneRef,
    #[error("irrigation field not found: {field_id}")]
    FieldNotFound { field_id: String },
    #[error("irrigation applied amount is invalid: {value}")]
    InvalidAppliedAmount { value: String },
    #[error("irrigation source cannot be empty")]
    EmptySource,
    #[error("irrigation timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("irrigation actor cannot be empty")]
    EmptyActor,
    #[error("irrigation timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
    #[error("irrigation history date range is invalid")]
    InvalidDateRange,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterUseBaseline {
    pub field_id: String,
    pub zone_ref: String,
    pub baseline_amount_mm: f64,
    pub method: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterUseSavingsReportRequest {
    pub field_id: String,
    pub start_time: String,
    pub end_time: String,
    pub events: Vec<IrrigationEventRecord>,
    pub baselines: Vec<WaterUseBaseline>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaterUseSavingsStatus {
    Computed,
    NoBaseline,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterUseSavingsZoneReport {
    pub field_id: String,
    pub zone_ref: String,
    pub status: WaterUseSavingsStatus,
    pub applied_amount_mm: f64,
    #[serde(default)]
    pub baseline_amount_mm: Option<f64>,
    #[serde(default)]
    pub savings_mm: Option<f64>,
    #[serde(default)]
    pub baseline_method: Option<String>,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterUseSavingsReport {
    pub field_id: String,
    pub start_time: String,
    pub end_time: String,
    pub zones: Vec<WaterUseSavingsZoneReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WaterUseSavingsReportError {
    #[error("water-use report field_id cannot be empty")]
    EmptyFieldId,
    #[error("water-use report timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("water-use report timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
    #[error("water-use report date range is invalid")]
    InvalidDateRange,
    #[error("water-use baseline zone_ref cannot be empty")]
    EmptyZoneRef,
    #[error("water-use baseline amount is invalid: {value}")]
    InvalidBaselineAmount { value: String },
    #[error("water-use baseline method cannot be empty")]
    EmptyBaselineMethod,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterAlertThresholds {
    pub low_moisture_water_need_mm: f64,
    pub over_irrigation_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaterAlertType {
    LowMoisture,
    OverIrrigation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterAlert {
    pub field_ref: String,
    pub zone_ref: String,
    pub alert_type: WaterAlertType,
    pub value: f64,
    pub threshold: f64,
    pub evidence_freshness: WeatherFreshnessState,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterAlertRoutingRequest {
    pub field_ref: String,
    pub recipient_id: String,
    pub owned_field_refs: Vec<String>,
    pub routed_at: String,
    pub thresholds: WaterAlertThresholds,
    pub water_needs: Vec<ZoneWaterNeed>,
    pub savings_reports: Vec<WaterUseSavingsZoneReport>,
    pub evidence_freshness: WeatherFreshnessState,
    pub targets: Vec<WeatherAlertRoutingTarget>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterAlertRoutingResult {
    pub alerts: Vec<WaterAlert>,
    pub delivered_count: usize,
    pub queued_count: usize,
    pub rejected_count: usize,
    pub audits: Vec<WeatherAlertDeliveryAudit>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WaterAlertError {
    #[error("water alert field_ref cannot be empty")]
    EmptyFieldRef,
    #[error("water alert recipient_id cannot be empty")]
    EmptyRecipientId,
    #[error("water alert routed_at cannot be empty")]
    EmptyRoutedAt,
    #[error("water alert routed_at is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
    #[error("water alert thresholds are invalid")]
    InvalidThresholds,
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

pub fn ingest_remote_sensing_moisture_proxy_layer(
    layer: RemoteSensingMoistureProxyLayer,
    field: &FieldRecord,
) -> Result<Vec<RemoteSensingMoistureProxyRecord>, RemoteSensingMoistureProxyError> {
    let layer_id = normalize_soil_moisture_text(layer.layer_id)
        .ok_or(RemoteSensingMoistureProxyError::EmptyLayerId)?;
    let field_id = normalize_soil_moisture_text(layer.field_id)
        .ok_or(RemoteSensingMoistureProxyError::EmptyFieldId)?;
    if field.field_id != field_id {
        return Err(RemoteSensingMoistureProxyError::FieldMismatch { field_id });
    }
    let source = normalize_soil_moisture_text(layer.source)
        .ok_or(RemoteSensingMoistureProxyError::EmptySource)?;
    let captured_at = normalize_soil_moisture_text(layer.captured_at)
        .ok_or(RemoteSensingMoistureProxyError::EmptyCapturedAt)?;
    let spatial_ref =
        assert_raster_spatial_ref(layer.spatial_ref.as_ref(), layer.width, layer.height)?;
    let layer_crs = spatial_ref
        .crs
        .as_ref()
        .expect("asserted spatial ref includes CRS")
        .clone();
    let field_crs = field
        .boundary
        .crs
        .as_deref()
        .map(str::trim)
        .filter(|crs| !crs.is_empty())
        .unwrap_or("EPSG:4326")
        .to_string();
    if layer_crs != field_crs {
        return Err(RemoteSensingMoistureProxyError::CrsMismatch {
            layer_crs,
            field_crs,
        });
    }
    let layer_extent = spatial_ref
        .bbox
        .as_ref()
        .expect("asserted spatial ref includes bbox");
    if !geo_bounds_match(layer_extent, &field.extent) {
        return Err(RemoteSensingMoistureProxyError::ExtentMismatch);
    }
    if layer.zone_values.is_empty() {
        return Err(RemoteSensingMoistureProxyError::MissingZoneValues);
    }

    let mut records = Vec::with_capacity(layer.zone_values.len());
    for zone_value in layer.zone_values {
        let zone_ref = normalize_soil_moisture_text(zone_value.zone_ref)
            .ok_or(RemoteSensingMoistureProxyError::EmptyZoneRef)?;
        if !(zone_value.value.is_finite() && (-1.0..=1.0).contains(&zone_value.value)) {
            return Err(RemoteSensingMoistureProxyError::InvalidValue {
                value: zone_value.value.to_string(),
            });
        }
        records.push(RemoteSensingMoistureProxyRecord {
            proxy_id: stable_moisture_proxy_id(
                &field.field_id,
                &zone_ref,
                layer.index,
                &captured_at,
            ),
            field_id: field.field_id.clone(),
            zone_ref,
            value: zone_value.value,
            index: layer.index,
            source: source.clone(),
            captured_at: captured_at.clone(),
            evidence_kind: "proxy".to_string(),
            crs: field_crs.clone(),
            extent: field.extent.clone(),
            evidence_refs: vec![format!("layer:{layer_id}")],
        });
    }

    Ok(records)
}

pub fn append_irrigation_history_event(
    mut existing: Vec<IrrigationEventRecord>,
    request: IrrigationEventRequest,
    field: &FieldRecord,
    generated_event_id: String,
) -> Result<Vec<IrrigationEventRecord>, IrrigationHistoryError> {
    let event_id = normalize_soil_moisture_optional_text(request.event_id)
        .or_else(|| normalize_soil_moisture_text(generated_event_id))
        .ok_or(IrrigationHistoryError::EmptyEventId)?;
    let field_id = normalize_soil_moisture_text(request.field_id)
        .ok_or(IrrigationHistoryError::EmptyFieldId)?;
    if field.field_id != field_id {
        return Err(IrrigationHistoryError::FieldNotFound { field_id });
    }
    let zone_ref = normalize_soil_moisture_text(request.zone_ref)
        .ok_or(IrrigationHistoryError::EmptyZoneRef)?;
    if !(request.applied_amount_mm.is_finite() && request.applied_amount_mm >= 0.0) {
        return Err(IrrigationHistoryError::InvalidAppliedAmount {
            value: request.applied_amount_mm.to_string(),
        });
    }
    let source =
        normalize_soil_moisture_text(request.source).ok_or(IrrigationHistoryError::EmptySource)?;
    let timestamp = normalize_soil_moisture_text(request.timestamp)
        .ok_or(IrrigationHistoryError::EmptyTimestamp)?;
    parse_irrigation_timestamp(&timestamp)?;
    let actor =
        normalize_soil_moisture_text(request.actor).ok_or(IrrigationHistoryError::EmptyActor)?;

    existing.push(IrrigationEventRecord {
        event_id,
        field_id,
        zone_ref,
        applied_amount_mm: request.applied_amount_mm,
        source,
        timestamp,
        actor,
    });
    Ok(existing)
}

pub fn query_irrigation_history(
    events: &[IrrigationEventRecord],
    query: IrrigationHistoryQuery,
) -> Result<IrrigationHistoryQueryResult, IrrigationHistoryError> {
    let field_id =
        normalize_soil_moisture_text(query.field_id).ok_or(IrrigationHistoryError::EmptyFieldId)?;
    let start_time = normalize_soil_moisture_text(query.start_time)
        .ok_or(IrrigationHistoryError::EmptyTimestamp)?;
    let end_time = normalize_soil_moisture_text(query.end_time)
        .ok_or(IrrigationHistoryError::EmptyTimestamp)?;
    let start = parse_irrigation_timestamp(&start_time)?;
    let end = parse_irrigation_timestamp(&end_time)?;
    if end < start {
        return Err(IrrigationHistoryError::InvalidDateRange);
    }

    let mut matching = events
        .iter()
        .filter(|event| event.field_id == field_id)
        .filter_map(|event| {
            parse_irrigation_timestamp(&event.timestamp)
                .ok()
                .filter(|timestamp| *timestamp >= start && *timestamp <= end)
                .map(|_| event.clone())
        })
        .collect::<Vec<_>>();
    matching.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then(left.event_id.cmp(&right.event_id))
    });

    Ok(IrrigationHistoryQueryResult {
        field_id,
        total_count: matching.len(),
        empty: matching.is_empty(),
        events: matching,
    })
}

pub fn report_water_use_savings(
    request: WaterUseSavingsReportRequest,
) -> Result<WaterUseSavingsReport, WaterUseSavingsReportError> {
    let field_id = normalize_soil_moisture_text(request.field_id)
        .ok_or(WaterUseSavingsReportError::EmptyFieldId)?;
    let start_time = normalize_soil_moisture_text(request.start_time)
        .ok_or(WaterUseSavingsReportError::EmptyTimestamp)?;
    let end_time = normalize_soil_moisture_text(request.end_time)
        .ok_or(WaterUseSavingsReportError::EmptyTimestamp)?;
    let start = parse_water_use_report_timestamp(&start_time)?;
    let end = parse_water_use_report_timestamp(&end_time)?;
    if end < start {
        return Err(WaterUseSavingsReportError::InvalidDateRange);
    }

    let baselines = normalize_water_use_baselines(request.baselines, &field_id)?;
    let mut applied_by_zone: BTreeMap<String, (f64, Vec<String>)> = BTreeMap::new();
    for event in request.events {
        if event.field_id != field_id {
            continue;
        }
        let Ok(timestamp) = parse_water_use_report_timestamp(&event.timestamp) else {
            continue;
        };
        if timestamp < start || timestamp > end {
            continue;
        }
        let entry = applied_by_zone
            .entry(event.zone_ref.clone())
            .or_insert_with(|| (0.0, Vec::new()));
        entry.0 += event.applied_amount_mm;
        entry.1.push(format!("irrigation_event:{}", event.event_id));
    }

    let mut zones = Vec::new();
    for (zone_ref, (applied_amount_mm, evidence_refs)) in applied_by_zone {
        if let Some(baseline) = baselines.get(&zone_ref) {
            zones.push(WaterUseSavingsZoneReport {
                field_id: field_id.clone(),
                zone_ref,
                status: WaterUseSavingsStatus::Computed,
                applied_amount_mm,
                baseline_amount_mm: Some(baseline.baseline_amount_mm),
                savings_mm: Some(baseline.baseline_amount_mm - applied_amount_mm),
                baseline_method: Some(baseline.method.clone()),
                evidence_refs,
            });
        } else {
            zones.push(WaterUseSavingsZoneReport {
                field_id: field_id.clone(),
                zone_ref,
                status: WaterUseSavingsStatus::NoBaseline,
                applied_amount_mm,
                baseline_amount_mm: None,
                savings_mm: None,
                baseline_method: None,
                evidence_refs,
            });
        }
    }

    Ok(WaterUseSavingsReport {
        field_id,
        start_time,
        end_time,
        zones,
    })
}

fn normalize_water_use_baselines(
    baselines: Vec<WaterUseBaseline>,
    field_id: &str,
) -> Result<BTreeMap<String, WaterUseBaseline>, WaterUseSavingsReportError> {
    let mut normalized = BTreeMap::new();
    for baseline in baselines {
        let baseline_field_id = normalize_soil_moisture_text(baseline.field_id)
            .ok_or(WaterUseSavingsReportError::EmptyFieldId)?;
        if baseline_field_id != field_id {
            continue;
        }
        let zone_ref = normalize_soil_moisture_text(baseline.zone_ref)
            .ok_or(WaterUseSavingsReportError::EmptyZoneRef)?;
        if !(baseline.baseline_amount_mm.is_finite() && baseline.baseline_amount_mm >= 0.0) {
            return Err(WaterUseSavingsReportError::InvalidBaselineAmount {
                value: baseline.baseline_amount_mm.to_string(),
            });
        }
        let method = normalize_soil_moisture_text(baseline.method)
            .ok_or(WaterUseSavingsReportError::EmptyBaselineMethod)?;
        normalized.insert(
            zone_ref.clone(),
            WaterUseBaseline {
                field_id: baseline_field_id,
                zone_ref,
                baseline_amount_mm: baseline.baseline_amount_mm,
                method,
            },
        );
    }
    Ok(normalized)
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

fn stable_moisture_proxy_id(
    field_id: &str,
    zone_ref: &str,
    index: RemoteSensingMoistureIndex,
    captured_at: &str,
) -> String {
    format!(
        "moisture-proxy:{}:{}:{}:{}",
        sanitize_moisture_proxy_id_part(field_id),
        sanitize_moisture_proxy_id_part(zone_ref),
        index.as_str(),
        sanitize_moisture_proxy_id_part(captured_at)
    )
}

fn sanitize_moisture_proxy_id_part(value: &str) -> String {
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

fn geo_bounds_match(left: &GeoBounds, right: &GeoBounds) -> bool {
    (left.min_lon - right.min_lon).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.min_lat - right.min_lat).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.max_lon - right.max_lon).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.max_lat - right.max_lat).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
}

fn parse_irrigation_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, IrrigationHistoryError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| IrrigationHistoryError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

fn parse_water_use_report_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, WaterUseSavingsReportError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| WaterUseSavingsReportError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

pub fn evaluate_and_route_water_alerts(
    request: WaterAlertRoutingRequest,
) -> Result<WaterAlertRoutingResult, WaterAlertError> {
    let field_ref =
        normalize_weather_text(request.field_ref).ok_or(WaterAlertError::EmptyFieldRef)?;
    let recipient_id =
        normalize_weather_text(request.recipient_id).ok_or(WaterAlertError::EmptyRecipientId)?;
    let routed_at =
        normalize_weather_text(request.routed_at).ok_or(WaterAlertError::EmptyRoutedAt)?;
    parse_water_alert_timestamp(&routed_at)?;
    if !(request.thresholds.low_moisture_water_need_mm.is_finite()
        && request.thresholds.low_moisture_water_need_mm >= 0.0
        && request.thresholds.over_irrigation_mm.is_finite()
        && request.thresholds.over_irrigation_mm >= 0.0)
    {
        return Err(WaterAlertError::InvalidThresholds);
    }

    let mut alerts = Vec::new();
    for need in request.water_needs {
        if need.field_ref == field_ref && need.status == ZoneWaterNeedStatus::Computed {
            if let Some(water_need_mm) = need.water_need_mm {
                if water_need_mm >= request.thresholds.low_moisture_water_need_mm {
                    alerts.push(WaterAlert {
                        field_ref: field_ref.clone(),
                        zone_ref: need.zone_ref,
                        alert_type: WaterAlertType::LowMoisture,
                        value: water_need_mm,
                        threshold: request.thresholds.low_moisture_water_need_mm,
                        evidence_freshness: request.evidence_freshness,
                        evidence_refs: need.evidence_refs,
                    });
                }
            }
        }
    }
    for report in request.savings_reports {
        if report.field_id == field_ref && report.status == WaterUseSavingsStatus::Computed {
            let Some(savings_mm) = report.savings_mm else {
                continue;
            };
            let over_irrigation_mm = (-savings_mm).max(0.0);
            if over_irrigation_mm >= request.thresholds.over_irrigation_mm {
                alerts.push(WaterAlert {
                    field_ref: field_ref.clone(),
                    zone_ref: report.zone_ref,
                    alert_type: WaterAlertType::OverIrrigation,
                    value: over_irrigation_mm,
                    threshold: request.thresholds.over_irrigation_mm,
                    evidence_freshness: request.evidence_freshness,
                    evidence_refs: report.evidence_refs,
                });
            }
        }
    }
    alerts.sort_by(|left, right| {
        left.zone_ref
            .cmp(&right.zone_ref)
            .then_with(|| format!("{:?}", left.alert_type).cmp(&format!("{:?}", right.alert_type)))
    });

    let mut delivered_count = 0;
    let mut queued_count = 0;
    let mut rejected_count = 0;
    let mut audits = Vec::new();
    for alert in &alerts {
        for target in &request.targets {
            let (status, reason_code) = if !request.owned_field_refs.contains(&field_ref) {
                rejected_count += 1;
                (
                    WeatherAlertDeliveryStatus::Rejected,
                    "field_scope_not_owned",
                )
            } else if target.reachable {
                delivered_count += 1;
                (WeatherAlertDeliveryStatus::Delivered, "alert_delivered")
            } else {
                queued_count += 1;
                (
                    WeatherAlertDeliveryStatus::Queued,
                    "target_unreachable_queued",
                )
            };
            let mut evidence_payload = alert.evidence_refs.clone();
            evidence_payload.push(format!("freshness:{:?}", alert.evidence_freshness));
            evidence_payload.push(format!("threshold:{}", alert.threshold));
            audits.push(WeatherAlertDeliveryAudit {
                target: target.target,
                status,
                reason_code: reason_code.to_string(),
                recipient_id: recipient_id.clone(),
                field_ref: field_ref.clone(),
                routed_at: routed_at.clone(),
                evidence_payload,
            });
        }
    }

    Ok(WaterAlertRoutingResult {
        alerts,
        delivered_count,
        queued_count,
        rejected_count,
        audits,
    })
}

fn parse_water_alert_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, WaterAlertError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc))
        .map_err(|_| WaterAlertError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtStressIndex {
    Ndvi,
    Ndmi,
}

impl DroughtStressIndex {
    pub fn as_str(self) -> &'static str {
        match self {
            DroughtStressIndex::Ndvi => "ndvi",
            DroughtStressIndex::Ndmi => "ndmi",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtStressEvidenceLayer {
    #[serde(default)]
    pub evidence_id: Option<String>,
    pub field_or_region_ref: String,
    pub source_scene_ref: String,
    pub index: DroughtStressIndex,
    pub value: f64,
    pub source: String,
    pub captured_at: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub spatial_ref: Option<RasterSpatialRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtStressEvidenceRecord {
    pub evidence_id: String,
    pub field_or_region_ref: String,
    pub source_scene_ref: String,
    pub index: DroughtStressIndex,
    pub value: f64,
    pub source: String,
    pub captured_at: String,
    pub evidence_kind: String,
    pub crs: String,
    pub extent: GeoBounds,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtEvidenceFusionRequest {
    pub field_or_region_ref: String,
    pub period: DroughtIndexPeriod,
    pub crs: String,
    pub extent: GeoBounds,
    #[serde(default)]
    pub stress_evidence: Option<DroughtStressEvidenceRecord>,
    #[serde(default)]
    pub weather_records: Vec<WeatherFreshnessAnnotatedRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtEvidenceFusionStatus {
    Complete,
    Degraded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtEvidenceInputStatus {
    Present,
    Missing,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtEvidenceInputSummary {
    pub input: String,
    pub status: DroughtEvidenceInputStatus,
    pub freshness: WeatherFreshnessState,
    pub coverage_fraction: f64,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtEvidenceFusionRecord {
    pub field_or_region_ref: String,
    pub period: DroughtIndexPeriod,
    pub status: DroughtEvidenceFusionStatus,
    pub crs: String,
    pub extent: GeoBounds,
    #[serde(default)]
    pub stress_evidence: Option<DroughtStressEvidenceRecord>,
    pub weather_records: Vec<WeatherFreshnessAnnotatedRecord>,
    pub inputs: Vec<DroughtEvidenceInputSummary>,
    pub evidence_refs: Vec<String>,
    pub degradation_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtBaselineTrendRequest {
    pub field_or_region_ref: String,
    pub index_type: DroughtIndexType,
    pub min_samples: usize,
    #[serde(default)]
    pub history: Vec<DroughtIndexRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtBaselineTrendStatus {
    Computed,
    InsufficientBaseline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtTrendDirection {
    Improving,
    Worsening,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtBaselineTrendRecord {
    pub field_or_region_ref: String,
    pub index_type: DroughtIndexType,
    pub status: DroughtBaselineTrendStatus,
    pub sample_count: usize,
    #[serde(default)]
    pub period: Option<DroughtIndexPeriod>,
    #[serde(default)]
    pub baseline_value: Option<f64>,
    #[serde(default)]
    pub trend_slope_per_day: Option<f64>,
    #[serde(default)]
    pub trend_direction: Option<DroughtTrendDirection>,
    pub method: String,
    pub evidence_refs: Vec<String>,
    pub degradation_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtRiskThresholds {
    pub moderate: f64,
    pub high: f64,
    pub severe: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtRiskScoreRequest {
    pub field_or_region_ref: String,
    pub thresholds: DroughtRiskThresholds,
    #[serde(default)]
    pub index_record: Option<DroughtIndexRecord>,
    #[serde(default)]
    pub stress_evidence: Option<DroughtStressEvidenceRecord>,
    #[serde(default)]
    pub baseline: Option<DroughtBaselineTrendRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtRiskScoreStatus {
    Computed,
    InsufficientEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtRiskBand {
    Low,
    Moderate,
    High,
    Severe,
}

impl DroughtRiskBand {
    pub fn as_forecast_ref(self) -> &'static str {
        match self {
            DroughtRiskBand::Low => "low",
            DroughtRiskBand::Moderate => "moderate",
            DroughtRiskBand::High => "high",
            DroughtRiskBand::Severe => "severe",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtRiskScoreRecord {
    pub field_or_region_ref: String,
    #[serde(default)]
    pub index_type: Option<DroughtIndexType>,
    pub status: DroughtRiskScoreStatus,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub band: Option<DroughtRiskBand>,
    pub thresholds: DroughtRiskThresholds,
    pub evidence_refs: Vec<String>,
    pub degradation_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtForecastRequest {
    pub feature_enabled: bool,
    pub requested_at: String,
    pub risk_score_computed_at: String,
    pub max_score_age_days: i64,
    pub horizon_days: u32,
    #[serde(default)]
    pub risk_score: Option<DroughtRiskScoreRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtForecastStatus {
    Forecast,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtForecastUncertaintyBand {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtForecastRecord {
    pub field_or_region_ref: String,
    pub status: DroughtForecastStatus,
    pub horizon_days: u32,
    #[serde(default)]
    pub predicted_value: Option<f64>,
    #[serde(default)]
    pub predicted_band: Option<DroughtRiskBand>,
    #[serde(default)]
    pub uncertainty: Option<DroughtForecastUncertaintyBand>,
    pub evidence_refs: Vec<String>,
    pub unavailable_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtAlertRoutingRequest {
    pub risk_score: DroughtRiskScoreRecord,
    pub warning_threshold: f64,
    pub recipient_id: String,
    pub owned_field_refs: Vec<String>,
    pub targets: Vec<WeatherAlertRoutingTarget>,
    pub routed_at: String,
    pub freshness: WeatherFreshnessState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtRiskAlert {
    pub field_or_region_ref: String,
    pub value: f64,
    pub band: DroughtRiskBand,
    pub warning_threshold: f64,
    pub routed_at: String,
    pub freshness: WeatherFreshnessState,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtAlertDeliveryAudit {
    pub target: WeatherAlertRouteTarget,
    pub status: WeatherAlertDeliveryStatus,
    pub reason_code: String,
    pub recipient_id: String,
    pub field_or_region_ref: String,
    pub routed_at: String,
    pub evidence_payload: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtAlertRoutingResult {
    pub alerts: Vec<DroughtRiskAlert>,
    pub delivered_count: usize,
    pub queued_count: usize,
    pub rejected_count: usize,
    pub audits: Vec<DroughtAlertDeliveryAudit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtMitigationRequest {
    pub risk_score: DroughtRiskScoreRecord,
    pub generated_at: String,
    pub min_risk_value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtMitigationStatus {
    Recommended,
    NotQualified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtMitigationActionTarget {
    Irrigation16,
    Advisor09,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtMitigationRecommendation {
    pub field_or_region_ref: String,
    pub status: DroughtMitigationStatus,
    pub generated_at: String,
    #[serde(default)]
    pub risk_ref: Option<String>,
    #[serde(default)]
    pub action_target: Option<DroughtMitigationActionTarget>,
    #[serde(default)]
    pub recommendation: Option<String>,
    pub evidence_refs: Vec<String>,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtReportRequest {
    pub report_id: String,
    pub generated_at: String,
    pub index_record: DroughtIndexRecord,
    pub baseline: DroughtBaselineTrendRecord,
    pub risk_score: DroughtRiskScoreRecord,
    #[serde(default)]
    pub forecast: Option<DroughtForecastRecord>,
    #[serde(default)]
    pub mitigation: Option<DroughtMitigationRecommendation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtReportSectionKind {
    Index,
    BaselineTrend,
    RiskScore,
    Forecast,
    Mitigation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtReportSection {
    pub kind: DroughtReportSectionKind,
    pub title: String,
    pub summary: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtReportRecord {
    pub report_id: String,
    pub field_or_region_ref: String,
    pub generated_at: String,
    pub sections: Vec<DroughtReportSection>,
    pub evidence_refs: Vec<String>,
    pub freshness_assertions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtHistoryEntryKind {
    Index,
    RiskScore,
    Alert,
    Recommendation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtHistoryEntry {
    pub sequence: u64,
    pub field_or_region_ref: String,
    pub occurred_at: String,
    pub kind: DroughtHistoryEntryKind,
    pub record_ref: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtHistoryQuery {
    pub field_or_region_ref: String,
    pub start_time: String,
    pub end_time: String,
    pub offset: usize,
    pub limit: usize,
    #[serde(default)]
    pub entries: Vec<DroughtHistoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtHistoryQueryResult {
    pub field_or_region_ref: String,
    pub total_count: usize,
    pub offset: usize,
    pub limit: usize,
    pub entries: Vec<DroughtHistoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtAdvisoryLoopRequest {
    pub feature_enabled: bool,
    pub deterministic_scoring_reliable: bool,
    #[serde(default)]
    pub latest_risk_score: Option<DroughtRiskScoreRecord>,
    #[serde(default)]
    pub latest_recommendation: Option<DroughtMitigationRecommendation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DroughtAdvisoryLoopStatus {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroughtAdvisoryLoopEvaluation {
    pub status: DroughtAdvisoryLoopStatus,
    pub evidence_refs: Vec<String>,
    pub disabled_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DroughtStressEvidenceError {
    #[error("drought stress evidence_id cannot be empty")]
    EmptyEvidenceId,
    #[error("drought stress field_or_region_ref cannot be empty")]
    EmptyFieldOrRegionRef,
    #[error("drought stress source_scene_ref cannot be empty")]
    EmptySourceSceneRef,
    #[error("drought stress source cannot be empty")]
    EmptySource,
    #[error("drought stress captured_at cannot be empty")]
    EmptyCapturedAt,
    #[error("drought stress field/region mismatch: {field_or_region_ref}")]
    FieldOrRegionMismatch { field_or_region_ref: String },
    #[error("drought stress value is invalid: {value}")]
    InvalidValue { value: String },
    #[error("drought stress CRS mismatch: layer {layer_crs}, field {field_crs}")]
    CrsMismatch {
        layer_crs: String,
        field_crs: String,
    },
    #[error("drought stress extent mismatch")]
    ExtentMismatch,
    #[error("drought stress spatial reference invalid: {0}")]
    SpatialRef(#[from] RasterSpatialRefError),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DroughtEvidenceFusionError {
    #[error("drought evidence fusion field_or_region_ref cannot be empty")]
    EmptyFieldOrRegionRef,
    #[error("drought evidence fusion period start cannot be empty")]
    EmptyPeriodStart,
    #[error("drought evidence fusion period end cannot be empty")]
    EmptyPeriodEnd,
    #[error("drought evidence fusion period range is invalid: {start}..{end}")]
    InvalidPeriodRange { start: String, end: String },
    #[error("drought evidence fusion CRS cannot be empty")]
    EmptyCrs,
    #[error("drought evidence fusion CRS mismatch for {input}: input {input_crs}, request {request_crs}")]
    CrsMismatch {
        input: String,
        input_crs: String,
        request_crs: String,
    },
    #[error("drought evidence fusion extent mismatch for {input}")]
    ExtentMismatch { input: String },
    #[error("drought evidence fusion timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DroughtBaselineTrendError {
    #[error("drought baseline field_or_region_ref cannot be empty")]
    EmptyFieldOrRegionRef,
    #[error("drought baseline min_samples must be at least 2")]
    InvalidMinSamples,
    #[error("drought baseline timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DroughtRiskScoreError {
    #[error("drought risk field_or_region_ref cannot be empty")]
    EmptyFieldOrRegionRef,
    #[error("drought risk thresholds must be finite and ordered within 0..=1")]
    InvalidThresholds,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DroughtForecastError {
    #[error("drought forecast requested_at cannot be empty")]
    EmptyRequestedAt,
    #[error("drought forecast risk_score_computed_at cannot be empty")]
    EmptyRiskScoreComputedAt,
    #[error("drought forecast timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
    #[error("drought forecast horizon_days must be positive")]
    InvalidHorizon,
    #[error("drought forecast max_score_age_days must be positive")]
    InvalidMaxScoreAge,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DroughtAlertRoutingError {
    #[error("drought alert routing recipient_id cannot be empty")]
    EmptyRecipientId,
    #[error("drought alert routing routed_at cannot be empty")]
    EmptyRoutedAt,
    #[error("drought alert routing requires at least one target")]
    EmptyTargets,
    #[error("drought alert routing warning_threshold must be finite within 0..=1")]
    InvalidThreshold,
    #[error("drought alert routing timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DroughtMitigationError {
    #[error("drought mitigation generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("drought mitigation min_risk_value must be finite within 0..=1")]
    InvalidMinRiskValue,
    #[error("drought mitigation timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DroughtReportError {
    #[error("drought report report_id cannot be empty")]
    EmptyReportId,
    #[error("drought report generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("drought report timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
    #[error("drought report baseline is missing or not computed")]
    MissingBaseline,
    #[error("drought report risk score is missing or not computed")]
    MissingRiskScore,
    #[error("drought report field/region mismatch for {input}")]
    FieldOrRegionMismatch { input: String },
    #[error("drought report section {section} must cite evidence")]
    MissingSectionEvidence { section: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DroughtHistoryError {
    #[error("drought history field_or_region_ref cannot be empty")]
    EmptyFieldOrRegionRef,
    #[error("drought history limit must be positive")]
    InvalidLimit,
    #[error("drought history date range is invalid")]
    InvalidDateRange,
    #[error("drought history timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
    #[error("drought history entry {record_ref} must cite evidence")]
    MissingEvidence { record_ref: String },
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

pub fn ingest_drought_stress_evidence(
    layer: DroughtStressEvidenceLayer,
    field: &FieldRecord,
) -> Result<DroughtStressEvidenceRecord, DroughtStressEvidenceError> {
    let field_or_region_ref = normalize_drought_text(layer.field_or_region_ref)
        .ok_or(DroughtStressEvidenceError::EmptyFieldOrRegionRef)?;
    if field.field_id != field_or_region_ref {
        return Err(DroughtStressEvidenceError::FieldOrRegionMismatch {
            field_or_region_ref,
        });
    }
    let source_scene_ref = normalize_drought_text(layer.source_scene_ref)
        .ok_or(DroughtStressEvidenceError::EmptySourceSceneRef)?;
    let source =
        normalize_drought_text(layer.source).ok_or(DroughtStressEvidenceError::EmptySource)?;
    let captured_at = normalize_drought_text(layer.captured_at)
        .ok_or(DroughtStressEvidenceError::EmptyCapturedAt)?;
    let evidence_id = normalize_drought_optional_text(layer.evidence_id)
        .or_else(|| {
            Some(stable_drought_stress_evidence_id(
                &field_or_region_ref,
                layer.index,
                &captured_at,
            ))
        })
        .and_then(normalize_drought_text)
        .ok_or(DroughtStressEvidenceError::EmptyEvidenceId)?;
    if !(layer.value.is_finite() && (-1.0..=1.0).contains(&layer.value)) {
        return Err(DroughtStressEvidenceError::InvalidValue {
            value: layer.value.to_string(),
        });
    }
    let spatial_ref =
        assert_raster_spatial_ref(layer.spatial_ref.as_ref(), layer.width, layer.height)?;
    let layer_crs = spatial_ref
        .crs
        .as_ref()
        .expect("asserted spatial ref includes CRS")
        .clone();
    let field_crs = field
        .boundary
        .crs
        .as_deref()
        .map(str::trim)
        .filter(|crs| !crs.is_empty())
        .unwrap_or("EPSG:4326")
        .to_string();
    if layer_crs != field_crs {
        return Err(DroughtStressEvidenceError::CrsMismatch {
            layer_crs,
            field_crs,
        });
    }
    let layer_extent = spatial_ref
        .bbox
        .as_ref()
        .expect("asserted spatial ref includes bbox");
    if !geo_bounds_match(layer_extent, &field.extent) {
        return Err(DroughtStressEvidenceError::ExtentMismatch);
    }

    Ok(DroughtStressEvidenceRecord {
        evidence_id,
        field_or_region_ref: field.field_id.clone(),
        source_scene_ref: source_scene_ref.clone(),
        index: layer.index,
        value: layer.value,
        source,
        captured_at,
        evidence_kind: "observed".to_string(),
        crs: field_crs,
        extent: field.extent.clone(),
        evidence_refs: vec![format!("scene:{source_scene_ref}")],
    })
}

pub fn fuse_drought_evidence(
    request: DroughtEvidenceFusionRequest,
) -> Result<DroughtEvidenceFusionRecord, DroughtEvidenceFusionError> {
    let field_or_region_ref = normalize_drought_text(request.field_or_region_ref)
        .ok_or(DroughtEvidenceFusionError::EmptyFieldOrRegionRef)?;
    let period = normalize_drought_fusion_period(request.period)?;
    let period_start = parse_drought_fusion_timestamp(&period.start)?;
    let period_end = parse_drought_fusion_timestamp(&period.end)?;
    let crs = normalize_drought_text(request.crs).ok_or(DroughtEvidenceFusionError::EmptyCrs)?;

    let mut inputs = Vec::new();
    let mut evidence_refs = BTreeSet::new();
    let mut degradation_reasons = Vec::new();

    let stress_evidence = match request.stress_evidence {
        Some(record) => {
            if record.crs != crs {
                return Err(DroughtEvidenceFusionError::CrsMismatch {
                    input: "stress_evidence".to_string(),
                    input_crs: record.crs,
                    request_crs: crs,
                });
            }
            if !geo_bounds_match(&record.extent, &request.extent) {
                return Err(DroughtEvidenceFusionError::ExtentMismatch {
                    input: "stress_evidence".to_string(),
                });
            }
            let captured_at = parse_drought_fusion_timestamp(&record.captured_at)?;
            let present = record.field_or_region_ref == field_or_region_ref
                && captured_at >= period_start
                && captured_at <= period_end;
            if present {
                evidence_refs.insert(format!("drought_stress:{}", record.evidence_id));
                evidence_refs.extend(record.evidence_refs.iter().cloned());
                inputs.push(DroughtEvidenceInputSummary {
                    input: "stress_evidence".to_string(),
                    status: DroughtEvidenceInputStatus::Present,
                    freshness: WeatherFreshnessState::Fresh,
                    coverage_fraction: 1.0,
                    evidence_refs: vec![format!("drought_stress:{}", record.evidence_id)],
                });
            } else {
                degradation_reasons.push("missing_stress_evidence".to_string());
                inputs.push(DroughtEvidenceInputSummary {
                    input: "stress_evidence".to_string(),
                    status: DroughtEvidenceInputStatus::Missing,
                    freshness: WeatherFreshnessState::Stale,
                    coverage_fraction: 0.0,
                    evidence_refs: Vec::new(),
                });
            }
            Some(record)
        }
        None => {
            degradation_reasons.push("missing_stress_evidence".to_string());
            inputs.push(DroughtEvidenceInputSummary {
                input: "stress_evidence".to_string(),
                status: DroughtEvidenceInputStatus::Missing,
                freshness: WeatherFreshnessState::Stale,
                coverage_fraction: 0.0,
                evidence_refs: Vec::new(),
            });
            None
        }
    };

    let mut weather_records = request
        .weather_records
        .into_iter()
        .filter_map(|record| {
            if record.field_ref != field_or_region_ref {
                return None;
            }
            match parse_drought_fusion_timestamp(&record.valid_time) {
                Ok(valid_time) if valid_time >= period_start && valid_time <= period_end => {
                    Some(Ok(record))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    weather_records.sort_by(|left, right| left.valid_time.cmp(&right.valid_time));

    if weather_records.is_empty() {
        degradation_reasons.push("missing_weather_input".to_string());
        inputs.push(DroughtEvidenceInputSummary {
            input: "weather".to_string(),
            status: DroughtEvidenceInputStatus::Missing,
            freshness: WeatherFreshnessState::Stale,
            coverage_fraction: 0.0,
            evidence_refs: Vec::new(),
        });
    } else {
        let fresh_count = weather_records
            .iter()
            .filter(|record| !record.stale)
            .count();
        let weather_stale = fresh_count != weather_records.len();
        if weather_stale {
            degradation_reasons.push("stale_weather_input".to_string());
        }
        let weather_refs = weather_records
            .iter()
            .map(|record| format!("weather_forecast:{}", record.forecast_id))
            .collect::<Vec<_>>();
        evidence_refs.extend(weather_refs.iter().cloned());
        inputs.push(DroughtEvidenceInputSummary {
            input: "weather".to_string(),
            status: if weather_stale {
                DroughtEvidenceInputStatus::Stale
            } else {
                DroughtEvidenceInputStatus::Present
            },
            freshness: if weather_stale {
                WeatherFreshnessState::Stale
            } else {
                WeatherFreshnessState::Fresh
            },
            coverage_fraction: fresh_count as f64 / weather_records.len() as f64,
            evidence_refs: weather_refs,
        });
    }

    let status = if degradation_reasons.is_empty() {
        DroughtEvidenceFusionStatus::Complete
    } else {
        DroughtEvidenceFusionStatus::Degraded
    };

    Ok(DroughtEvidenceFusionRecord {
        field_or_region_ref,
        period,
        status,
        crs,
        extent: request.extent,
        stress_evidence,
        weather_records,
        inputs,
        evidence_refs: evidence_refs.into_iter().collect(),
        degradation_reasons,
    })
}

pub fn compute_drought_baseline_trend(
    request: DroughtBaselineTrendRequest,
) -> Result<DroughtBaselineTrendRecord, DroughtBaselineTrendError> {
    let field_or_region_ref = normalize_drought_text(request.field_or_region_ref)
        .ok_or(DroughtBaselineTrendError::EmptyFieldOrRegionRef)?;
    if request.min_samples < 2 {
        return Err(DroughtBaselineTrendError::InvalidMinSamples);
    }

    let mut samples = request
        .history
        .into_iter()
        .filter(|record| {
            record.field_or_region_ref == field_or_region_ref
                && record.index_type == request.index_type
        })
        .map(|record| {
            let sample_time = parse_drought_baseline_timestamp(&record.period.end)?;
            Ok((sample_time, record))
        })
        .collect::<Result<Vec<_>, DroughtBaselineTrendError>>()?;
    samples.sort_by(|left, right| left.0.cmp(&right.0));

    let sample_count = samples.len();
    let evidence_refs = samples
        .iter()
        .map(|(_, record)| format!("drought_index:{}", record.index_id))
        .collect::<Vec<_>>();
    if sample_count < request.min_samples {
        return Ok(DroughtBaselineTrendRecord {
            field_or_region_ref,
            index_type: request.index_type,
            status: DroughtBaselineTrendStatus::InsufficientBaseline,
            sample_count,
            period: None,
            baseline_value: None,
            trend_slope_per_day: None,
            trend_direction: None,
            method: "historical_mean_linear_trend_v1".to_string(),
            evidence_refs,
            degradation_reasons: vec!["insufficient_baseline_history".to_string()],
        });
    }

    let baseline_value =
        samples.iter().map(|(_, record)| record.value).sum::<f64>() / sample_count as f64;
    let first_time = samples
        .first()
        .map(|(sample_time, _)| *sample_time)
        .expect("sample count checked");
    let xs = samples
        .iter()
        .map(|(sample_time, _)| {
            sample_time.signed_duration_since(first_time).num_seconds() as f64 / 86_400.0
        })
        .collect::<Vec<_>>();
    let mean_x = xs.iter().sum::<f64>() / sample_count as f64;
    let mean_y = baseline_value;
    let denominator = xs
        .iter()
        .map(|x| {
            let centered = x - mean_x;
            centered * centered
        })
        .sum::<f64>();
    let numerator = xs
        .iter()
        .zip(samples.iter())
        .map(|(x, (_, record))| (x - mean_x) * (record.value - mean_y))
        .sum::<f64>();
    let trend_slope_per_day = if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    };
    let trend_direction = if trend_slope_per_day > 0.000_001 {
        DroughtTrendDirection::Improving
    } else if trend_slope_per_day < -0.000_001 {
        DroughtTrendDirection::Worsening
    } else {
        DroughtTrendDirection::Stable
    };
    let first = samples.first().expect("sample count checked").1.clone();
    let last = samples.last().expect("sample count checked").1.clone();

    Ok(DroughtBaselineTrendRecord {
        field_or_region_ref,
        index_type: request.index_type,
        status: DroughtBaselineTrendStatus::Computed,
        sample_count,
        period: Some(DroughtIndexPeriod {
            start: first.period.start,
            end: last.period.end,
            accumulation_days: last.period.accumulation_days,
        }),
        baseline_value: Some(baseline_value),
        trend_slope_per_day: Some(trend_slope_per_day),
        trend_direction: Some(trend_direction),
        method: "historical_mean_linear_trend_v1".to_string(),
        evidence_refs,
        degradation_reasons: Vec::new(),
    })
}

pub fn compute_drought_risk_score(
    request: DroughtRiskScoreRequest,
) -> Result<DroughtRiskScoreRecord, DroughtRiskScoreError> {
    let field_or_region_ref = normalize_drought_text(request.field_or_region_ref)
        .ok_or(DroughtRiskScoreError::EmptyFieldOrRegionRef)?;
    if !valid_drought_risk_thresholds(&request.thresholds) {
        return Err(DroughtRiskScoreError::InvalidThresholds);
    }

    let mut evidence_refs = Vec::new();
    let mut degradation_reasons = Vec::new();

    let index_record = match request.index_record {
        Some(record) if record.field_or_region_ref == field_or_region_ref => {
            evidence_refs.push(format!("drought_index:{}", record.index_id));
            Some(record)
        }
        Some(_) | None => {
            degradation_reasons.push("missing_index_evidence".to_string());
            None
        }
    };
    let index_type = index_record.as_ref().map(|record| record.index_type);

    let stress_evidence = match request.stress_evidence {
        Some(record) if record.field_or_region_ref == field_or_region_ref => {
            evidence_refs.push(format!("drought_stress:{}", record.evidence_id));
            Some(record)
        }
        Some(_) | None => {
            degradation_reasons.push("missing_stress_evidence".to_string());
            None
        }
    };

    let baseline = match (request.baseline, index_type) {
        (Some(record), Some(index_type))
            if record.field_or_region_ref == field_or_region_ref
                && record.index_type == index_type
                && record.status == DroughtBaselineTrendStatus::Computed =>
        {
            evidence_refs.push(format!(
                "drought_baseline:{}:{}:{}",
                record.field_or_region_ref,
                record.index_type.as_str(),
                record.method
            ));
            evidence_refs.extend(record.evidence_refs.iter().cloned());
            Some(record)
        }
        (Some(_), _) | (None, _) => {
            degradation_reasons.push("missing_baseline_evidence".to_string());
            None
        }
    };

    if !(index_record.is_some() && stress_evidence.is_some() && baseline.is_some()) {
        evidence_refs.sort();
        evidence_refs.dedup();
        return Ok(DroughtRiskScoreRecord {
            field_or_region_ref,
            index_type,
            status: DroughtRiskScoreStatus::InsufficientEvidence,
            value: None,
            band: None,
            thresholds: request.thresholds,
            evidence_refs,
            degradation_reasons,
        });
    }

    let index_record = index_record.expect("presence checked");
    let stress_evidence = stress_evidence.expect("presence checked");
    let baseline = baseline.expect("presence checked");
    let baseline_value = baseline
        .baseline_value
        .expect("computed baseline includes value");
    let index_component = clamp_unit(-index_record.value / 2.0);
    let stress_component = clamp_unit(-stress_evidence.value);
    let baseline_component = clamp_unit((baseline_value - index_record.value) / 2.0);
    let value = (index_component * 0.55) + (stress_component * 0.25) + (baseline_component * 0.20);
    let band = drought_risk_band(value, &request.thresholds);

    evidence_refs.sort();
    evidence_refs.dedup();
    Ok(DroughtRiskScoreRecord {
        field_or_region_ref,
        index_type: Some(index_record.index_type),
        status: DroughtRiskScoreStatus::Computed,
        value: Some(value),
        band: Some(band),
        thresholds: request.thresholds,
        evidence_refs,
        degradation_reasons: Vec::new(),
    })
}

pub fn forecast_drought_risk(
    request: DroughtForecastRequest,
) -> Result<DroughtForecastRecord, DroughtForecastError> {
    if request.horizon_days == 0 {
        return Err(DroughtForecastError::InvalidHorizon);
    }
    if request.max_score_age_days <= 0 {
        return Err(DroughtForecastError::InvalidMaxScoreAge);
    }
    let requested_at = normalize_drought_text(request.requested_at)
        .ok_or(DroughtForecastError::EmptyRequestedAt)?;
    let risk_score_computed_at = normalize_drought_text(request.risk_score_computed_at)
        .ok_or(DroughtForecastError::EmptyRiskScoreComputedAt)?;
    let requested_at = parse_drought_forecast_timestamp(&requested_at)?;
    let risk_score_computed_at = parse_drought_forecast_timestamp(&risk_score_computed_at)?;

    let mut unavailable_reasons = Vec::new();
    if !request.feature_enabled {
        unavailable_reasons.push("forecast_feature_disabled".to_string());
    }
    let risk_score = match request.risk_score {
        Some(record)
            if record.status == DroughtRiskScoreStatus::Computed
                && record.value.is_some()
                && record.band.is_some() =>
        {
            Some(record)
        }
        Some(_) | None => {
            unavailable_reasons.push("missing_risk_score".to_string());
            None
        }
    };
    let stale = requested_at
        .signed_duration_since(risk_score_computed_at)
        .num_days()
        > request.max_score_age_days;
    if stale {
        unavailable_reasons.push("stale_risk_score".to_string());
    }

    if !unavailable_reasons.is_empty() {
        return Ok(DroughtForecastRecord {
            field_or_region_ref: risk_score
                .as_ref()
                .map(|record| record.field_or_region_ref.clone())
                .unwrap_or_default(),
            status: DroughtForecastStatus::Unavailable,
            horizon_days: request.horizon_days,
            predicted_value: None,
            predicted_band: None,
            uncertainty: None,
            evidence_refs: risk_score
                .as_ref()
                .map(|record| record.evidence_refs.clone())
                .unwrap_or_default(),
            unavailable_reasons,
        });
    }

    let risk_score = risk_score.expect("availability checked");
    let current_value = risk_score.value.expect("computed score includes value");
    let predicted_value = clamp_unit(current_value + (request.horizon_days as f64 / 90.0) * 0.05);
    let predicted_band = drought_risk_band(predicted_value, &risk_score.thresholds);
    let uncertainty = if request.horizon_days <= 14 {
        DroughtForecastUncertaintyBand::Low
    } else if request.horizon_days <= 45 {
        DroughtForecastUncertaintyBand::Medium
    } else {
        DroughtForecastUncertaintyBand::High
    };
    let mut evidence_refs = risk_score.evidence_refs.clone();
    evidence_refs.push(format!(
        "drought_risk_score:{}:{}",
        risk_score.field_or_region_ref,
        risk_score
            .band
            .expect("computed score includes band")
            .as_forecast_ref()
    ));
    evidence_refs.sort();
    evidence_refs.dedup();

    Ok(DroughtForecastRecord {
        field_or_region_ref: risk_score.field_or_region_ref,
        status: DroughtForecastStatus::Forecast,
        horizon_days: request.horizon_days,
        predicted_value: Some(predicted_value),
        predicted_band: Some(predicted_band),
        uncertainty: Some(uncertainty),
        evidence_refs,
        unavailable_reasons: Vec::new(),
    })
}

pub fn evaluate_and_route_drought_alerts(
    request: DroughtAlertRoutingRequest,
) -> Result<DroughtAlertRoutingResult, DroughtAlertRoutingError> {
    let recipient_id = normalize_drought_text(request.recipient_id)
        .ok_or(DroughtAlertRoutingError::EmptyRecipientId)?;
    let routed_at =
        normalize_drought_text(request.routed_at).ok_or(DroughtAlertRoutingError::EmptyRoutedAt)?;
    parse_drought_alert_timestamp(&routed_at)?;
    if request.targets.is_empty() {
        return Err(DroughtAlertRoutingError::EmptyTargets);
    }
    if !(request.warning_threshold.is_finite() && (0.0..=1.0).contains(&request.warning_threshold))
    {
        return Err(DroughtAlertRoutingError::InvalidThreshold);
    }

    let risk_value = request.risk_score.value.unwrap_or_default();
    let risk_band = request.risk_score.band.unwrap_or(DroughtRiskBand::Low);
    if request.risk_score.status != DroughtRiskScoreStatus::Computed
        || risk_value < request.warning_threshold
    {
        return Ok(DroughtAlertRoutingResult {
            alerts: Vec::new(),
            delivered_count: 0,
            queued_count: 0,
            rejected_count: 0,
            audits: Vec::new(),
        });
    }

    let owned_field_refs = request
        .owned_field_refs
        .into_iter()
        .filter_map(normalize_drought_text)
        .collect::<BTreeSet<_>>();
    let field_owned = owned_field_refs.contains(&request.risk_score.field_or_region_ref);
    let mut evidence_refs = request.risk_score.evidence_refs.clone();
    evidence_refs.push(format!(
        "drought_risk_score:{}:{}",
        request.risk_score.field_or_region_ref,
        risk_band.as_forecast_ref()
    ));
    evidence_refs.push(format!("freshness:{:?}", request.freshness));
    evidence_refs.sort();
    evidence_refs.dedup();
    let alert = DroughtRiskAlert {
        field_or_region_ref: request.risk_score.field_or_region_ref.clone(),
        value: risk_value,
        band: risk_band,
        warning_threshold: request.warning_threshold,
        routed_at: routed_at.clone(),
        freshness: request.freshness,
        evidence_refs: evidence_refs.clone(),
    };

    let mut delivered_count = 0;
    let mut queued_count = 0;
    let mut rejected_count = 0;
    let audits = request
        .targets
        .into_iter()
        .map(|target| {
            let (status, reason_code) = if !field_owned {
                rejected_count += 1;
                (
                    WeatherAlertDeliveryStatus::Rejected,
                    "field_scope_not_owned",
                )
            } else if target.reachable {
                delivered_count += 1;
                (WeatherAlertDeliveryStatus::Delivered, "delivered")
            } else {
                queued_count += 1;
                (WeatherAlertDeliveryStatus::Queued, "target_unreachable")
            };
            DroughtAlertDeliveryAudit {
                target: target.target,
                status,
                reason_code: reason_code.to_string(),
                recipient_id: recipient_id.clone(),
                field_or_region_ref: request.risk_score.field_or_region_ref.clone(),
                routed_at: routed_at.clone(),
                evidence_payload: evidence_refs.clone(),
            }
        })
        .collect();

    Ok(DroughtAlertRoutingResult {
        alerts: vec![alert],
        delivered_count,
        queued_count,
        rejected_count,
        audits,
    })
}

pub fn derive_drought_mitigation_recommendation(
    request: DroughtMitigationRequest,
) -> Result<DroughtMitigationRecommendation, DroughtMitigationError> {
    let generated_at = normalize_drought_text(request.generated_at)
        .ok_or(DroughtMitigationError::EmptyGeneratedAt)?;
    parse_drought_mitigation_timestamp(&generated_at)?;
    if !(request.min_risk_value.is_finite() && (0.0..=1.0).contains(&request.min_risk_value)) {
        return Err(DroughtMitigationError::InvalidMinRiskValue);
    }

    let value = request.risk_score.value.unwrap_or_default();
    let band = request.risk_score.band.unwrap_or(DroughtRiskBand::Low);
    let risk_ref = format!(
        "drought_risk_score:{}:{}",
        request.risk_score.field_or_region_ref,
        band.as_forecast_ref()
    );
    if request.risk_score.status != DroughtRiskScoreStatus::Computed
        || value < request.min_risk_value
    {
        return Ok(DroughtMitigationRecommendation {
            field_or_region_ref: request.risk_score.field_or_region_ref,
            status: DroughtMitigationStatus::NotQualified,
            generated_at,
            risk_ref: None,
            action_target: None,
            recommendation: None,
            evidence_refs: request.risk_score.evidence_refs,
            reason_code: "risk_below_mitigation_threshold".to_string(),
        });
    }

    let action_target = if matches!(band, DroughtRiskBand::High | DroughtRiskBand::Severe) {
        DroughtMitigationActionTarget::Irrigation16
    } else {
        DroughtMitigationActionTarget::Advisor09
    };
    let recommendation = match action_target {
        DroughtMitigationActionTarget::Irrigation16 => {
            "review_irrigation_schedule_for_drought_stress".to_string()
        }
        DroughtMitigationActionTarget::Advisor09 => {
            "review_crop_advisor_drought_mitigation_plan".to_string()
        }
    };
    let mut evidence_refs = request.risk_score.evidence_refs;
    evidence_refs.push(risk_ref.clone());
    evidence_refs.sort();
    evidence_refs.dedup();

    Ok(DroughtMitigationRecommendation {
        field_or_region_ref: request.risk_score.field_or_region_ref,
        status: DroughtMitigationStatus::Recommended,
        generated_at,
        risk_ref: Some(risk_ref),
        action_target: Some(action_target),
        recommendation: Some(recommendation),
        evidence_refs,
        reason_code: "risk_qualifies_for_mitigation".to_string(),
    })
}

pub fn assemble_drought_report(
    request: DroughtReportRequest,
) -> Result<DroughtReportRecord, DroughtReportError> {
    let report_id =
        normalize_drought_text(request.report_id).ok_or(DroughtReportError::EmptyReportId)?;
    let generated_at =
        normalize_drought_text(request.generated_at).ok_or(DroughtReportError::EmptyGeneratedAt)?;
    parse_drought_report_timestamp(&generated_at)?;
    let field_or_region_ref = request.index_record.field_or_region_ref.clone();

    if request.baseline.field_or_region_ref != field_or_region_ref {
        return Err(DroughtReportError::FieldOrRegionMismatch {
            input: "baseline".to_string(),
        });
    }
    if request.baseline.status != DroughtBaselineTrendStatus::Computed {
        return Err(DroughtReportError::MissingBaseline);
    }
    if request.risk_score.field_or_region_ref != field_or_region_ref {
        return Err(DroughtReportError::FieldOrRegionMismatch {
            input: "risk_score".to_string(),
        });
    }
    if request.risk_score.status != DroughtRiskScoreStatus::Computed {
        return Err(DroughtReportError::MissingRiskScore);
    }
    if let Some(forecast) = request.forecast.as_ref() {
        if forecast.field_or_region_ref != field_or_region_ref {
            return Err(DroughtReportError::FieldOrRegionMismatch {
                input: "forecast".to_string(),
            });
        }
    }
    if let Some(mitigation) = request.mitigation.as_ref() {
        if mitigation.field_or_region_ref != field_or_region_ref {
            return Err(DroughtReportError::FieldOrRegionMismatch {
                input: "mitigation".to_string(),
            });
        }
    }

    let mut sections = Vec::new();
    sections.push(DroughtReportSection {
        kind: DroughtReportSectionKind::Index,
        title: "Drought index".to_string(),
        summary: format!(
            "{} value {:.3} for {}..{}",
            request.index_record.index_type.as_str(),
            request.index_record.value,
            request.index_record.period.start,
            request.index_record.period.end
        ),
        evidence_refs: drought_report_index_evidence(&request.index_record),
    });
    sections.push(DroughtReportSection {
        kind: DroughtReportSectionKind::BaselineTrend,
        title: "Historical baseline and trend".to_string(),
        summary: format!(
            "baseline {:.3}, trend {:?}, samples {}",
            request
                .baseline
                .baseline_value
                .expect("computed baseline includes value"),
            request
                .baseline
                .trend_direction
                .expect("computed baseline includes trend"),
            request.baseline.sample_count
        ),
        evidence_refs: request.baseline.evidence_refs.clone(),
    });
    sections.push(DroughtReportSection {
        kind: DroughtReportSectionKind::RiskScore,
        title: "Deterministic drought risk".to_string(),
        summary: format!(
            "risk {:.3}, band {:?}",
            request
                .risk_score
                .value
                .expect("computed risk includes value"),
            request
                .risk_score
                .band
                .expect("computed risk includes band")
        ),
        evidence_refs: request.risk_score.evidence_refs.clone(),
    });
    if let Some(forecast) = request.forecast {
        if forecast.status == DroughtForecastStatus::Forecast {
            sections.push(DroughtReportSection {
                kind: DroughtReportSectionKind::Forecast,
                title: "Gated forecast".to_string(),
                summary: format!(
                    "forecast {:?} over {} days with {:?} uncertainty",
                    forecast.predicted_band.expect("forecast includes band"),
                    forecast.horizon_days,
                    forecast.uncertainty.expect("forecast includes uncertainty")
                ),
                evidence_refs: forecast.evidence_refs,
            });
        }
    }
    if let Some(mitigation) = request.mitigation {
        if mitigation.status == DroughtMitigationStatus::Recommended {
            sections.push(DroughtReportSection {
                kind: DroughtReportSectionKind::Mitigation,
                title: "Mitigation recommendation".to_string(),
                summary: format!(
                    "{:?}: {}",
                    mitigation
                        .action_target
                        .expect("recommended mitigation includes target"),
                    mitigation
                        .recommendation
                        .clone()
                        .expect("recommended mitigation includes recommendation")
                ),
                evidence_refs: mitigation.evidence_refs,
            });
        }
    }

    for section in &sections {
        if section.evidence_refs.is_empty() {
            return Err(DroughtReportError::MissingSectionEvidence {
                section: section.title.clone(),
            });
        }
    }
    let evidence_refs = sections
        .iter()
        .flat_map(|section| section.evidence_refs.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    Ok(DroughtReportRecord {
        report_id,
        field_or_region_ref,
        generated_at,
        sections,
        evidence_refs,
        freshness_assertions: vec![
            "deterministic_sections_first".to_string(),
            "required_evidence_present".to_string(),
        ],
    })
}

pub fn query_drought_history(
    query: DroughtHistoryQuery,
) -> Result<DroughtHistoryQueryResult, DroughtHistoryError> {
    let field_or_region_ref = normalize_drought_text(query.field_or_region_ref)
        .ok_or(DroughtHistoryError::EmptyFieldOrRegionRef)?;
    if query.limit == 0 {
        return Err(DroughtHistoryError::InvalidLimit);
    }
    let start_time = normalize_drought_text(query.start_time).ok_or_else(|| {
        DroughtHistoryError::InvalidTimestamp {
            timestamp: String::new(),
        }
    })?;
    let end_time = normalize_drought_text(query.end_time).ok_or_else(|| {
        DroughtHistoryError::InvalidTimestamp {
            timestamp: String::new(),
        }
    })?;
    let start_time = parse_drought_history_timestamp(&start_time)?;
    let end_time = parse_drought_history_timestamp(&end_time)?;
    if start_time > end_time {
        return Err(DroughtHistoryError::InvalidDateRange);
    }

    let mut entries = Vec::new();
    for entry in query.entries {
        if entry.evidence_refs.is_empty() {
            return Err(DroughtHistoryError::MissingEvidence {
                record_ref: entry.record_ref,
            });
        }
        if entry.field_or_region_ref != field_or_region_ref {
            continue;
        }
        let occurred_at = parse_drought_history_timestamp(&entry.occurred_at)?;
        if occurred_at >= start_time && occurred_at <= end_time {
            entries.push(entry);
        }
    }
    entries.sort_by(|left, right| {
        left.occurred_at
            .cmp(&right.occurred_at)
            .then(left.sequence.cmp(&right.sequence))
    });
    let total_count = entries.len();
    let entries = entries
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .collect::<Vec<_>>();

    Ok(DroughtHistoryQueryResult {
        field_or_region_ref,
        total_count,
        offset: query.offset,
        limit: query.limit,
        entries,
    })
}

pub fn evaluate_drought_advisory_loop(
    request: DroughtAdvisoryLoopRequest,
) -> DroughtAdvisoryLoopEvaluation {
    let mut disabled_reasons = Vec::new();
    let mut evidence_refs = Vec::new();
    if !request.feature_enabled {
        disabled_reasons.push("advisory_loop_feature_disabled".to_string());
    }
    if !request.deterministic_scoring_reliable {
        disabled_reasons.push("deterministic_scoring_not_reliable".to_string());
    }
    match request.latest_risk_score {
        Some(record) if record.status == DroughtRiskScoreStatus::Computed => {
            evidence_refs.extend(record.evidence_refs);
        }
        Some(_) | None => disabled_reasons.push("missing_reliable_risk_score".to_string()),
    }
    match request.latest_recommendation {
        Some(record) if record.status == DroughtMitigationStatus::Recommended => {
            evidence_refs.extend(record.evidence_refs);
        }
        Some(_) | None => {
            disabled_reasons.push("missing_evidence_backed_recommendation".to_string())
        }
    }
    evidence_refs.sort();
    evidence_refs.dedup();

    DroughtAdvisoryLoopEvaluation {
        status: if disabled_reasons.is_empty() {
            DroughtAdvisoryLoopStatus::Enabled
        } else {
            DroughtAdvisoryLoopStatus::Disabled
        },
        evidence_refs,
        disabled_reasons,
    }
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

fn normalize_drought_fusion_period(
    period: DroughtIndexPeriod,
) -> Result<DroughtIndexPeriod, DroughtEvidenceFusionError> {
    let start =
        normalize_drought_text(period.start).ok_or(DroughtEvidenceFusionError::EmptyPeriodStart)?;
    let end =
        normalize_drought_text(period.end).ok_or(DroughtEvidenceFusionError::EmptyPeriodEnd)?;
    let parsed_start = parse_drought_fusion_timestamp(&start)?;
    let parsed_end = parse_drought_fusion_timestamp(&end)?;
    if parsed_start > parsed_end {
        return Err(DroughtEvidenceFusionError::InvalidPeriodRange { start, end });
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

fn stable_drought_stress_evidence_id(
    field_or_region_ref: &str,
    index: DroughtStressIndex,
    captured_at: &str,
) -> String {
    format!(
        "drought-stress:{}:{}:{}",
        sanitize_moisture_proxy_id_part(field_or_region_ref),
        index.as_str(),
        sanitize_moisture_proxy_id_part(captured_at)
    )
}

fn normalize_drought_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(normalize_drought_text)
}

fn normalize_drought_text(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_drought_fusion_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::FixedOffset>, DroughtEvidenceFusionError> {
    chrono::DateTime::parse_from_rfc3339(timestamp).map_err(|_| {
        DroughtEvidenceFusionError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        }
    })
}

fn parse_drought_baseline_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, DroughtBaselineTrendError> {
    if let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(timestamp) {
        return Ok(timestamp.with_timezone(&chrono::Utc));
    }
    let date = chrono::NaiveDate::parse_from_str(timestamp, "%Y-%m-%d").map_err(|_| {
        DroughtBaselineTrendError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        }
    })?;
    let timestamp = date
        .and_hms_opt(0, 0, 0)
        .expect("midnight is valid for a parsed date");
    Ok(chrono::DateTime::from_naive_utc_and_offset(
        timestamp,
        chrono::Utc,
    ))
}

fn parse_drought_forecast_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, DroughtForecastError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| DroughtForecastError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

fn parse_drought_alert_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, DroughtAlertRoutingError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| DroughtAlertRoutingError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

fn parse_drought_mitigation_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, DroughtMitigationError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| DroughtMitigationError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

fn parse_drought_report_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, DroughtReportError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| DroughtReportError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

fn drought_report_index_evidence(record: &DroughtIndexRecord) -> Vec<String> {
    let mut evidence_refs = vec![format!("drought_index:{}", record.index_id)];
    evidence_refs.extend(record.input_refs.iter().cloned());
    evidence_refs.sort();
    evidence_refs.dedup();
    evidence_refs
}

fn parse_drought_history_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, DroughtHistoryError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| DroughtHistoryError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

fn valid_drought_risk_thresholds(thresholds: &DroughtRiskThresholds) -> bool {
    thresholds.moderate.is_finite()
        && thresholds.high.is_finite()
        && thresholds.severe.is_finite()
        && thresholds.moderate >= 0.0
        && thresholds.moderate < thresholds.high
        && thresholds.high < thresholds.severe
        && thresholds.severe <= 1.0
}

fn drought_risk_band(value: f64, thresholds: &DroughtRiskThresholds) -> DroughtRiskBand {
    if value >= thresholds.severe {
        DroughtRiskBand::Severe
    } else if value >= thresholds.high {
        DroughtRiskBand::High
    } else if value >= thresholds.moderate {
        DroughtRiskBand::Moderate
    } else {
        DroughtRiskBand::Low
    }
}

fn clamp_unit(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceCatalogItemKind {
    Input,
    Produce,
}

impl MarketplaceCatalogItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MarketplaceCatalogItemKind::Input => "input",
            MarketplaceCatalogItemKind::Produce => "produce",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceCatalogCategory {
    Seed,
    Fertilizer,
    CropProtection,
    Service,
    Grain,
    Produce,
}

impl MarketplaceCatalogCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            MarketplaceCatalogCategory::Seed => "seed",
            MarketplaceCatalogCategory::Fertilizer => "fertilizer",
            MarketplaceCatalogCategory::CropProtection => "crop_protection",
            MarketplaceCatalogCategory::Service => "service",
            MarketplaceCatalogCategory::Grain => "grain",
            MarketplaceCatalogCategory::Produce => "produce",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceUnitOfMeasure {
    Kg,
    Tonne,
    Litre,
    Bag,
    Bushel,
    Crate,
    Acre,
}

impl MarketplaceUnitOfMeasure {
    pub fn as_str(self) -> &'static str {
        match self {
            MarketplaceUnitOfMeasure::Kg => "kg",
            MarketplaceUnitOfMeasure::Tonne => "tonne",
            MarketplaceUnitOfMeasure::Litre => "litre",
            MarketplaceUnitOfMeasure::Bag => "bag",
            MarketplaceUnitOfMeasure::Bushel => "bushel",
            MarketplaceUnitOfMeasure::Crate => "crate",
            MarketplaceUnitOfMeasure::Acre => "acre",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceCatalogItemCreateRequest {
    #[serde(default)]
    pub item_id: Option<String>,
    pub org_id: String,
    pub kind: MarketplaceCatalogItemKind,
    pub category: MarketplaceCatalogCategory,
    pub name: String,
    pub unit_of_measure: MarketplaceUnitOfMeasure,
    pub owner_account_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceCatalogItemRecord {
    pub item_id: String,
    pub org_id: String,
    pub kind: MarketplaceCatalogItemKind,
    pub category: MarketplaceCatalogCategory,
    pub name: String,
    pub unit_of_measure: MarketplaceUnitOfMeasure,
    pub owner_account_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplacePortalEntry {
    pub org_id: String,
    pub account_id: String,
    pub label: String,
    pub href: String,
    pub visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceListingStatus {
    Draft,
    Published,
    Closed,
}

impl MarketplaceListingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            MarketplaceListingStatus::Draft => "draft",
            MarketplaceListingStatus::Published => "published",
            MarketplaceListingStatus::Closed => "closed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceAvailabilityWindow {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceListingPublishRequest {
    #[serde(default)]
    pub listing_id: Option<String>,
    pub item_id: String,
    pub org_id: String,
    pub price: f64,
    pub currency: String,
    pub available_qty: f64,
    pub window: MarketplaceAvailabilityWindow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceListingRecord {
    pub listing_id: String,
    pub item_id: String,
    pub org_id: String,
    pub price: f64,
    pub currency: String,
    pub available_qty: f64,
    pub window: MarketplaceAvailabilityWindow,
    pub status: MarketplaceListingStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceInventoryUpsertRequest {
    #[serde(default)]
    pub inventory_id: Option<String>,
    pub item_id: String,
    pub org_id: String,
    pub on_hand: f64,
    #[serde(default)]
    pub reserved: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceInventoryRecord {
    pub inventory_id: String,
    pub item_id: String,
    pub org_id: String,
    pub on_hand: f64,
    pub reserved: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceOrderStatus {
    Placed,
    Confirmed,
    Fulfilled,
    Closed,
    Cancelled,
}

impl MarketplaceOrderStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            MarketplaceOrderStatus::Placed => "placed",
            MarketplaceOrderStatus::Confirmed => "confirmed",
            MarketplaceOrderStatus::Fulfilled => "fulfilled",
            MarketplaceOrderStatus::Closed => "closed",
            MarketplaceOrderStatus::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceOrderCreateRequest {
    #[serde(default)]
    pub order_id: Option<String>,
    pub org_id: String,
    pub listing_ref: String,
    pub buyer_account_id: String,
    pub qty: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceOrderRecord {
    pub order_id: String,
    pub org_id: String,
    pub listing_ref: String,
    pub buyer_account_id: String,
    pub qty: f64,
    pub line_total: f64,
    pub status: MarketplaceOrderStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceOrderAuditRecord {
    pub audit_id: String,
    pub order_id: String,
    pub from_status: Option<MarketplaceOrderStatus>,
    pub to_status: MarketplaceOrderStatus,
    pub actor_id: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceFulfillmentStatus {
    Pending,
    Shipped,
    Delivered,
    Failed,
}

impl MarketplaceFulfillmentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            MarketplaceFulfillmentStatus::Pending => "pending",
            MarketplaceFulfillmentStatus::Shipped => "shipped",
            MarketplaceFulfillmentStatus::Delivered => "delivered",
            MarketplaceFulfillmentStatus::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceFulfillmentCreateRequest {
    #[serde(default)]
    pub fulfillment_id: Option<String>,
    pub order_ref: String,
    pub org_id: String,
    pub carrier_ref: String,
    pub tracking_ref: String,
    pub actor_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceFulfillmentRecord {
    pub fulfillment_id: String,
    pub order_ref: String,
    pub org_id: String,
    pub carrier_ref: String,
    pub tracking_ref: String,
    pub status: MarketplaceFulfillmentStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceFulfillmentAuditRecord {
    pub audit_id: String,
    pub fulfillment_id: String,
    pub from_status: Option<MarketplaceFulfillmentStatus>,
    pub to_status: MarketplaceFulfillmentStatus,
    pub actor_id: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceRatingCreateRequest {
    #[serde(default)]
    pub rating_id: Option<String>,
    pub order_ref: String,
    pub rater_account_id: String,
    pub ratee_account_id: String,
    pub score: f64,
    pub comment: Option<String>,
    pub org_scope: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceRatingRecord {
    pub rating_id: String,
    pub order_ref: String,
    pub rater_account_id: String,
    pub ratee_account_id: String,
    pub score: f64,
    pub comment: Option<String>,
    pub org_scope: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceRatingAggregate {
    pub account_id: String,
    pub org_scope: String,
    pub rating_count: u32,
    pub average_score: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceDemandForecastStatus {
    Ready,
    NoBasis,
}

impl MarketplaceDemandForecastStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            MarketplaceDemandForecastStatus::Ready => "ready",
            MarketplaceDemandForecastStatus::NoBasis => "no_basis",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceDemandForecastRequest {
    #[serde(default)]
    pub forecast_id: Option<String>,
    pub org_id: String,
    pub field_id: String,
    pub item_kind: MarketplaceCatalogItemKind,
    pub horizon: String,
    #[serde(default)]
    pub ai_assisted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceDemandUncertaintyBand {
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceDemandForecastRecord {
    pub forecast_id: String,
    pub org_id: String,
    pub field_id: String,
    pub item_kind: MarketplaceCatalogItemKind,
    pub horizon: String,
    pub value: Option<f64>,
    pub evidence_refs: Vec<String>,
    pub status: MarketplaceDemandForecastStatus,
    pub uncertainty_band: Option<MarketplaceDemandUncertaintyBand>,
    pub method: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceReportPeriod {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceOrgReportRequest {
    pub org_id: String,
    pub period: MarketplaceReportPeriod,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceOrderStatusCount {
    pub status: MarketplaceOrderStatus,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketplaceOrgReport {
    pub org_id: String,
    pub period: MarketplaceReportPeriod,
    pub sales_total: f64,
    pub procurement_total: f64,
    pub order_counts_by_status: Vec<MarketplaceOrderStatusCount>,
    pub listing_count: u32,
    pub inventory_on_hand_total: f64,
    pub inventory_reserved_total: f64,
    pub source_order_ids: Vec<String>,
    pub generated_at: String,
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarketplaceCatalogError {
    #[error("marketplace catalog item_id cannot be empty")]
    EmptyItemId,
    #[error("marketplace catalog org_id cannot be empty")]
    EmptyOrgId,
    #[error("marketplace catalog item name cannot be empty")]
    EmptyName,
    #[error("marketplace catalog owner_account_id cannot be empty")]
    EmptyOwnerAccountId,
    #[error("marketplace catalog created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("marketplace catalog owner account is required")]
    MissingOwnerAccount,
    #[error("marketplace catalog owner account {account_id} is not active")]
    OwnerAccountNotActive { account_id: String },
    #[error("marketplace catalog owner account {account_id} belongs to {account_org_id}, not {item_org_id}")]
    OwnerOrgMismatch {
        account_id: String,
        account_org_id: String,
        item_org_id: String,
    },
    #[error("marketplace catalog owner account {account_id} is not allowed to own catalog items")]
    OwnerPartyNotAllowed { account_id: String },
    #[error("unsupported marketplace catalog item kind {value}")]
    UnsupportedItemKind { value: String },
    #[error("unsupported marketplace catalog category {value}")]
    UnsupportedCategory { value: String },
    #[error("unsupported marketplace catalog unit {value}")]
    UnsupportedUnit { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarketplacePortalEntryError {
    #[error("marketplace portal org_id cannot be empty")]
    EmptyOrgId,
    #[error("marketplace portal account is required")]
    MissingAccount,
    #[error("marketplace portal account {account_id} belongs to {account_org_id}, not {org_id}")]
    OrgMismatch {
        account_id: String,
        account_org_id: String,
        org_id: String,
    },
    #[error("marketplace portal account {account_id} is not active")]
    AccountNotActive { account_id: String },
    #[error("marketplace portal account {account_id} has no marketplace access role")]
    MissingMarketplaceRole { account_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarketplaceListingError {
    #[error("marketplace listing_id cannot be empty")]
    EmptyListingId,
    #[error("marketplace listing item_id cannot be empty")]
    EmptyItemId,
    #[error("marketplace listing org_id cannot be empty")]
    EmptyOrgId,
    #[error("marketplace listing currency cannot be empty")]
    EmptyCurrency,
    #[error("marketplace listing window start cannot be empty")]
    EmptyWindowStart,
    #[error("marketplace listing window end cannot be empty")]
    EmptyWindowEnd,
    #[error("marketplace listing catalog item is required")]
    MissingCatalogItem,
    #[error(
        "marketplace listing catalog item {item_id} belongs to {item_org_id}, not {listing_org_id}"
    )]
    CatalogOrgMismatch {
        item_id: String,
        item_org_id: String,
        listing_org_id: String,
    },
    #[error("marketplace listing price must be finite and positive")]
    InvalidPrice,
    #[error("marketplace listing quantity must be finite and positive")]
    InvalidQuantity,
    #[error("marketplace listing window range is invalid")]
    InvalidWindowRange,
    #[error("marketplace listing timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
    #[error("unsupported marketplace listing status {value}")]
    UnsupportedStatus { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarketplaceInventoryError {
    #[error("marketplace inventory_id cannot be empty")]
    EmptyInventoryId,
    #[error("marketplace inventory item_id cannot be empty")]
    EmptyItemId,
    #[error("marketplace inventory org_id cannot be empty")]
    EmptyOrgId,
    #[error("marketplace inventory catalog item is required")]
    MissingCatalogItem,
    #[error(
        "marketplace inventory catalog item {item_id} belongs to {item_org_id}, not {inventory_org_id}"
    )]
    CatalogOrgMismatch {
        item_id: String,
        item_org_id: String,
        inventory_org_id: String,
    },
    #[error("marketplace inventory quantity must be finite and non-negative")]
    InvalidQuantity,
    #[error("marketplace inventory reserved quantity cannot exceed on-hand quantity")]
    ReservedExceedsOnHand,
    #[error("marketplace inventory has insufficient available quantity")]
    InsufficientAvailableQuantity,
    #[error("marketplace inventory has insufficient reserved quantity")]
    InsufficientReservedQuantity,
    #[error("marketplace inventory timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarketplaceOrderError {
    #[error("marketplace order_id cannot be empty")]
    EmptyOrderId,
    #[error("marketplace order org_id cannot be empty")]
    EmptyOrgId,
    #[error("marketplace order listing_ref cannot be empty")]
    EmptyListingRef,
    #[error("marketplace order buyer_account_id cannot be empty")]
    EmptyBuyerAccountId,
    #[error("marketplace order actor_id cannot be empty")]
    EmptyActorId,
    #[error("marketplace order listing is required")]
    MissingListing,
    #[error("marketplace order buyer account is required")]
    MissingBuyerAccount,
    #[error(
        "marketplace order listing {listing_id} belongs to {listing_org_id}, not {order_org_id}"
    )]
    ListingOrgMismatch {
        listing_id: String,
        listing_org_id: String,
        order_org_id: String,
    },
    #[error("marketplace order buyer account {account_id} belongs to {account_org_id}, not {order_org_id}")]
    BuyerOrgMismatch {
        account_id: String,
        account_org_id: String,
        order_org_id: String,
    },
    #[error("marketplace order buyer account {account_id} is not active")]
    BuyerAccountNotActive { account_id: String },
    #[error("marketplace order listing {listing_id} is not published")]
    ListingNotPublished { listing_id: String },
    #[error("marketplace order quantity must be finite and positive")]
    InvalidQuantity,
    #[error("marketplace order quantity exceeds listing availability")]
    InsufficientListingQuantity,
    #[error("invalid marketplace order transition for {order_id}: {from:?} -> {to:?}")]
    InvalidStatusTransition {
        order_id: String,
        from: MarketplaceOrderStatus,
        to: MarketplaceOrderStatus,
    },
    #[error("unsupported marketplace order status {value}")]
    UnsupportedStatus { value: String },
    #[error("marketplace order timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarketplaceFulfillmentError {
    #[error("marketplace fulfillment_id cannot be empty")]
    EmptyFulfillmentId,
    #[error("marketplace fulfillment order_ref cannot be empty")]
    EmptyOrderRef,
    #[error("marketplace fulfillment org_id cannot be empty")]
    EmptyOrgId,
    #[error("marketplace fulfillment carrier_ref cannot be empty")]
    EmptyCarrierRef,
    #[error("marketplace fulfillment tracking_ref cannot be empty")]
    EmptyTrackingRef,
    #[error("marketplace fulfillment actor_id cannot be empty")]
    EmptyActorId,
    #[error("marketplace fulfillment order is required")]
    MissingOrder,
    #[error("marketplace fulfillment order {order_id} belongs to {order_org_id}, not {fulfillment_org_id}")]
    OrderOrgMismatch {
        order_id: String,
        order_org_id: String,
        fulfillment_org_id: String,
    },
    #[error("marketplace fulfillment order {order_id} must be confirmed")]
    OrderNotConfirmed { order_id: String },
    #[error("invalid marketplace fulfillment transition for {fulfillment_id}: {from:?} -> {to:?}")]
    InvalidStatusTransition {
        fulfillment_id: String,
        from: MarketplaceFulfillmentStatus,
        to: MarketplaceFulfillmentStatus,
    },
    #[error("unsupported marketplace fulfillment status {value}")]
    UnsupportedStatus { value: String },
    #[error("marketplace fulfillment timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarketplaceRatingError {
    #[error("marketplace rating_id cannot be empty")]
    EmptyRatingId,
    #[error("marketplace rating order_ref cannot be empty")]
    EmptyOrderRef,
    #[error("marketplace rating rater_account_id cannot be empty")]
    EmptyRaterAccountId,
    #[error("marketplace rating ratee_account_id cannot be empty")]
    EmptyRateeAccountId,
    #[error("marketplace rating org_scope cannot be empty")]
    EmptyOrgScope,
    #[error("marketplace rating order is required")]
    MissingOrder,
    #[error("marketplace rating order {order_id} belongs to {order_org_id}, not {org_scope}")]
    OrderOrgMismatch {
        order_id: String,
        order_org_id: String,
        org_scope: String,
    },
    #[error("marketplace rating order {order_id} must be fulfilled or closed")]
    OrderNotRateable { order_id: String },
    #[error("marketplace rating rater {rater_account_id} did not participate in order {order_id}")]
    RaterNotParticipant {
        order_id: String,
        rater_account_id: String,
    },
    #[error("marketplace rating ratee {ratee_account_id} did not participate in order {order_id}")]
    RateeNotParticipant {
        order_id: String,
        ratee_account_id: String,
    },
    #[error("marketplace rating already exists for order {order_id} and rater {rater_account_id}")]
    DuplicateRaterForOrder {
        order_id: String,
        rater_account_id: String,
    },
    #[error("marketplace rating score must be finite and between 1 and 5")]
    InvalidScore,
    #[error("marketplace rating timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarketplaceDemandForecastError {
    #[error("marketplace demand forecast_id cannot be empty")]
    EmptyForecastId,
    #[error("marketplace demand org_id cannot be empty")]
    EmptyOrgId,
    #[error("marketplace demand field_id cannot be empty")]
    EmptyFieldId,
    #[error("marketplace demand horizon cannot be empty")]
    EmptyHorizon,
    #[error("marketplace demand field belongs to {field_org_id}, not {forecast_org_id}")]
    FieldOrgMismatch {
        field_org_id: String,
        forecast_org_id: String,
    },
    #[error("marketplace demand timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
    #[error("unsupported marketplace demand forecast status {value}")]
    UnsupportedStatus { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarketplaceReportError {
    #[error("marketplace report org_id cannot be empty")]
    EmptyOrgId,
    #[error("marketplace report period start cannot be empty")]
    EmptyPeriodStart,
    #[error("marketplace report period end cannot be empty")]
    EmptyPeriodEnd,
    #[error("marketplace report period range is invalid")]
    InvalidPeriodRange,
    #[error("marketplace report timestamp is invalid: {timestamp}")]
    InvalidTimestamp { timestamp: String },
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

pub fn build_marketplace_portal_entry(
    account: Option<&MarketplaceAccountRecord>,
    org_id: String,
) -> Result<MarketplacePortalEntry, MarketplacePortalEntryError> {
    let org_id =
        normalize_marketplace_text(org_id).ok_or(MarketplacePortalEntryError::EmptyOrgId)?;
    let account = account.ok_or(MarketplacePortalEntryError::MissingAccount)?;
    if account.org_id != org_id {
        return Err(MarketplacePortalEntryError::OrgMismatch {
            account_id: account.account_id.clone(),
            account_org_id: account.org_id.clone(),
            org_id,
        });
    }
    if account.status != MarketplaceAccountStatus::Active {
        return Err(MarketplacePortalEntryError::AccountNotActive {
            account_id: account.account_id.clone(),
        });
    }
    let has_marketplace_role = account
        .role_refs
        .iter()
        .any(|role_ref| role_ref == "marketplace:access" || role_ref.starts_with("marketplace:"));
    if !has_marketplace_role {
        return Err(MarketplacePortalEntryError::MissingMarketplaceRole {
            account_id: account.account_id.clone(),
        });
    }

    Ok(MarketplacePortalEntry {
        org_id: account.org_id.clone(),
        account_id: account.account_id.clone(),
        label: "Marketplace".to_string(),
        href: format!("/marketplace?org_id={}", account.org_id),
        visible: true,
    })
}

pub fn build_marketplace_catalog_item_record(
    request: MarketplaceCatalogItemCreateRequest,
    owner_account: Option<&MarketplaceAccountRecord>,
    generated_item_id: String,
    created_at: String,
) -> Result<MarketplaceCatalogItemRecord, MarketplaceCatalogError> {
    let item_id = normalize_marketplace_optional_text(request.item_id)
        .or_else(|| normalize_marketplace_text(generated_item_id))
        .ok_or(MarketplaceCatalogError::EmptyItemId)?;
    let org_id =
        normalize_marketplace_text(request.org_id).ok_or(MarketplaceCatalogError::EmptyOrgId)?;
    let name =
        normalize_marketplace_text(request.name).ok_or(MarketplaceCatalogError::EmptyName)?;
    let owner_account_id = normalize_marketplace_text(request.owner_account_id)
        .ok_or(MarketplaceCatalogError::EmptyOwnerAccountId)?;
    let created_at =
        normalize_marketplace_text(created_at).ok_or(MarketplaceCatalogError::EmptyCreatedAt)?;
    let owner_account = owner_account.ok_or(MarketplaceCatalogError::MissingOwnerAccount)?;
    if owner_account.account_id != owner_account_id {
        return Err(MarketplaceCatalogError::MissingOwnerAccount);
    }
    if owner_account.status != MarketplaceAccountStatus::Active {
        return Err(MarketplaceCatalogError::OwnerAccountNotActive {
            account_id: owner_account.account_id.clone(),
        });
    }
    if owner_account.org_id != org_id {
        return Err(MarketplaceCatalogError::OwnerOrgMismatch {
            account_id: owner_account.account_id.clone(),
            account_org_id: owner_account.org_id.clone(),
            item_org_id: org_id,
        });
    }
    if !matches!(
        owner_account.party_type,
        MarketplacePartyType::Supplier | MarketplacePartyType::Grower
    ) {
        return Err(MarketplaceCatalogError::OwnerPartyNotAllowed {
            account_id: owner_account.account_id.clone(),
        });
    }

    Ok(MarketplaceCatalogItemRecord {
        item_id,
        org_id,
        kind: request.kind,
        category: request.category,
        name,
        unit_of_measure: request.unit_of_measure,
        owner_account_id,
        created_at,
    })
}

pub fn publish_marketplace_listing_record(
    request: MarketplaceListingPublishRequest,
    catalog_item: Option<&MarketplaceCatalogItemRecord>,
    generated_listing_id: String,
    created_at: String,
) -> Result<MarketplaceListingRecord, MarketplaceListingError> {
    let listing_id = normalize_marketplace_optional_text(request.listing_id)
        .or_else(|| normalize_marketplace_text(generated_listing_id))
        .ok_or(MarketplaceListingError::EmptyListingId)?;
    let item_id =
        normalize_marketplace_text(request.item_id).ok_or(MarketplaceListingError::EmptyItemId)?;
    let org_id =
        normalize_marketplace_text(request.org_id).ok_or(MarketplaceListingError::EmptyOrgId)?;
    let currency = normalize_marketplace_text(request.currency)
        .ok_or(MarketplaceListingError::EmptyCurrency)?;
    let window_from = normalize_marketplace_text(request.window.from)
        .ok_or(MarketplaceListingError::EmptyWindowStart)?;
    let window_to = normalize_marketplace_text(request.window.to)
        .ok_or(MarketplaceListingError::EmptyWindowEnd)?;
    let parsed_from = parse_marketplace_listing_timestamp(&window_from)?;
    let parsed_to = parse_marketplace_listing_timestamp(&window_to)?;
    if parsed_to < parsed_from {
        return Err(MarketplaceListingError::InvalidWindowRange);
    }
    if !(request.price.is_finite() && request.price > 0.0) {
        return Err(MarketplaceListingError::InvalidPrice);
    }
    if !(request.available_qty.is_finite() && request.available_qty > 0.0) {
        return Err(MarketplaceListingError::InvalidQuantity);
    }
    let catalog_item = catalog_item.ok_or(MarketplaceListingError::MissingCatalogItem)?;
    if catalog_item.item_id != item_id || catalog_item.org_id != org_id {
        return Err(MarketplaceListingError::CatalogOrgMismatch {
            item_id: catalog_item.item_id.clone(),
            item_org_id: catalog_item.org_id.clone(),
            listing_org_id: org_id,
        });
    }
    let created_at = normalize_marketplace_text(created_at).ok_or(
        MarketplaceListingError::InvalidTimestamp {
            timestamp: String::new(),
        },
    )?;
    parse_marketplace_listing_timestamp(&created_at)?;

    Ok(MarketplaceListingRecord {
        listing_id,
        item_id,
        org_id,
        price: request.price,
        currency,
        available_qty: request.available_qty,
        window: MarketplaceAvailabilityWindow {
            from: window_from,
            to: window_to,
        },
        status: MarketplaceListingStatus::Published,
        created_at: created_at.clone(),
        updated_at: created_at,
    })
}

pub fn close_marketplace_listing_record(
    record: &MarketplaceListingRecord,
    updated_at: String,
) -> Result<MarketplaceListingRecord, MarketplaceListingError> {
    let updated_at = normalize_marketplace_text(updated_at).ok_or(
        MarketplaceListingError::InvalidTimestamp {
            timestamp: String::new(),
        },
    )?;
    parse_marketplace_listing_timestamp(&updated_at)?;
    let mut updated = record.clone();
    updated.status = MarketplaceListingStatus::Closed;
    updated.updated_at = updated_at;
    Ok(updated)
}

pub fn build_marketplace_inventory_record(
    request: MarketplaceInventoryUpsertRequest,
    catalog_item: Option<&MarketplaceCatalogItemRecord>,
    generated_inventory_id: String,
    updated_at: String,
) -> Result<MarketplaceInventoryRecord, MarketplaceInventoryError> {
    let inventory_id = normalize_marketplace_optional_text(request.inventory_id)
        .or_else(|| normalize_marketplace_text(generated_inventory_id))
        .ok_or(MarketplaceInventoryError::EmptyInventoryId)?;
    let item_id = normalize_marketplace_text(request.item_id)
        .ok_or(MarketplaceInventoryError::EmptyItemId)?;
    let org_id =
        normalize_marketplace_text(request.org_id).ok_or(MarketplaceInventoryError::EmptyOrgId)?;
    let reserved = request.reserved.unwrap_or(0.0);
    validate_marketplace_inventory_quantities(request.on_hand, reserved)?;
    let catalog_item = catalog_item.ok_or(MarketplaceInventoryError::MissingCatalogItem)?;
    if catalog_item.item_id != item_id || catalog_item.org_id != org_id {
        return Err(MarketplaceInventoryError::CatalogOrgMismatch {
            item_id: catalog_item.item_id.clone(),
            item_org_id: catalog_item.org_id.clone(),
            inventory_org_id: org_id,
        });
    }
    let updated_at = normalize_marketplace_text(updated_at).ok_or(
        MarketplaceInventoryError::InvalidTimestamp {
            timestamp: String::new(),
        },
    )?;
    parse_marketplace_inventory_timestamp(&updated_at)?;

    Ok(MarketplaceInventoryRecord {
        inventory_id,
        item_id,
        org_id,
        on_hand: request.on_hand,
        reserved,
        updated_at,
    })
}

pub fn reserve_marketplace_inventory(
    record: &MarketplaceInventoryRecord,
    qty: f64,
    updated_at: String,
) -> Result<MarketplaceInventoryRecord, MarketplaceInventoryError> {
    validate_marketplace_inventory_delta(qty)?;
    if qty > marketplace_inventory_available(record) {
        return Err(MarketplaceInventoryError::InsufficientAvailableQuantity);
    }
    let mut updated = record.clone();
    updated.reserved += qty;
    updated.updated_at = normalize_marketplace_inventory_timestamp(updated_at)?;
    validate_marketplace_inventory_quantities(updated.on_hand, updated.reserved)?;
    Ok(updated)
}

pub fn fulfill_marketplace_inventory(
    record: &MarketplaceInventoryRecord,
    qty: f64,
    updated_at: String,
) -> Result<MarketplaceInventoryRecord, MarketplaceInventoryError> {
    validate_marketplace_inventory_delta(qty)?;
    if qty > record.reserved {
        return Err(MarketplaceInventoryError::InsufficientReservedQuantity);
    }
    let mut updated = record.clone();
    updated.on_hand -= qty;
    updated.reserved -= qty;
    updated.updated_at = normalize_marketplace_inventory_timestamp(updated_at)?;
    validate_marketplace_inventory_quantities(updated.on_hand, updated.reserved)?;
    Ok(updated)
}

pub fn release_marketplace_inventory(
    record: &MarketplaceInventoryRecord,
    qty: f64,
    updated_at: String,
) -> Result<MarketplaceInventoryRecord, MarketplaceInventoryError> {
    validate_marketplace_inventory_delta(qty)?;
    if qty > record.reserved {
        return Err(MarketplaceInventoryError::InsufficientReservedQuantity);
    }
    let mut updated = record.clone();
    updated.reserved -= qty;
    updated.updated_at = normalize_marketplace_inventory_timestamp(updated_at)?;
    validate_marketplace_inventory_quantities(updated.on_hand, updated.reserved)?;
    Ok(updated)
}

pub fn marketplace_inventory_available(record: &MarketplaceInventoryRecord) -> f64 {
    record.on_hand - record.reserved
}

pub fn place_marketplace_order_record(
    request: MarketplaceOrderCreateRequest,
    listing: Option<&MarketplaceListingRecord>,
    buyer_account: Option<&MarketplaceAccountRecord>,
    generated_order_id: String,
    created_at: String,
) -> Result<(MarketplaceOrderRecord, MarketplaceOrderAuditRecord), MarketplaceOrderError> {
    let order_id = normalize_marketplace_optional_text(request.order_id)
        .or_else(|| normalize_marketplace_text(generated_order_id))
        .ok_or(MarketplaceOrderError::EmptyOrderId)?;
    let org_id =
        normalize_marketplace_text(request.org_id).ok_or(MarketplaceOrderError::EmptyOrgId)?;
    let listing_ref = normalize_marketplace_text(request.listing_ref)
        .ok_or(MarketplaceOrderError::EmptyListingRef)?;
    let buyer_account_id = normalize_marketplace_text(request.buyer_account_id)
        .ok_or(MarketplaceOrderError::EmptyBuyerAccountId)?;
    validate_marketplace_order_qty(request.qty)?;
    let listing = listing.ok_or(MarketplaceOrderError::MissingListing)?;
    if listing.listing_id != listing_ref || listing.org_id != org_id {
        return Err(MarketplaceOrderError::ListingOrgMismatch {
            listing_id: listing.listing_id.clone(),
            listing_org_id: listing.org_id.clone(),
            order_org_id: org_id,
        });
    }
    if listing.status != MarketplaceListingStatus::Published {
        return Err(MarketplaceOrderError::ListingNotPublished {
            listing_id: listing.listing_id.clone(),
        });
    }
    if request.qty > listing.available_qty {
        return Err(MarketplaceOrderError::InsufficientListingQuantity);
    }
    let buyer_account = buyer_account.ok_or(MarketplaceOrderError::MissingBuyerAccount)?;
    if buyer_account.account_id != buyer_account_id || buyer_account.org_id != org_id {
        return Err(MarketplaceOrderError::BuyerOrgMismatch {
            account_id: buyer_account.account_id.clone(),
            account_org_id: buyer_account.org_id.clone(),
            order_org_id: org_id,
        });
    }
    if buyer_account.status != MarketplaceAccountStatus::Active {
        return Err(MarketplaceOrderError::BuyerAccountNotActive {
            account_id: buyer_account.account_id.clone(),
        });
    }
    let created_at = normalize_marketplace_order_timestamp(created_at)?;
    let line_total = listing.price * request.qty;
    let record = MarketplaceOrderRecord {
        order_id: order_id.clone(),
        org_id,
        listing_ref,
        buyer_account_id: buyer_account_id.clone(),
        qty: request.qty,
        line_total,
        status: MarketplaceOrderStatus::Placed,
        created_at: created_at.clone(),
        updated_at: created_at.clone(),
    };
    let audit = MarketplaceOrderAuditRecord {
        audit_id: format!("{order_id}:placed:{created_at}"),
        order_id,
        from_status: None,
        to_status: MarketplaceOrderStatus::Placed,
        actor_id: buyer_account_id,
        occurred_at: created_at,
    };
    Ok((record, audit))
}

pub fn transition_marketplace_order_status(
    record: &MarketplaceOrderRecord,
    to: MarketplaceOrderStatus,
    actor_id: String,
    occurred_at: String,
) -> Result<(MarketplaceOrderRecord, MarketplaceOrderAuditRecord), MarketplaceOrderError> {
    let actor_id =
        normalize_marketplace_text(actor_id).ok_or(MarketplaceOrderError::EmptyActorId)?;
    let occurred_at = normalize_marketplace_order_timestamp(occurred_at)?;
    if !valid_marketplace_order_transition(record.status, to) {
        return Err(MarketplaceOrderError::InvalidStatusTransition {
            order_id: record.order_id.clone(),
            from: record.status,
            to,
        });
    }
    let mut updated = record.clone();
    updated.status = to;
    updated.updated_at = occurred_at.clone();
    let audit = MarketplaceOrderAuditRecord {
        audit_id: format!("{}:{}:{occurred_at}", record.order_id, to.as_str()),
        order_id: record.order_id.clone(),
        from_status: Some(record.status),
        to_status: to,
        actor_id,
        occurred_at,
    };
    Ok((updated, audit))
}

pub fn create_marketplace_fulfillment_record(
    request: MarketplaceFulfillmentCreateRequest,
    order: Option<&MarketplaceOrderRecord>,
    generated_fulfillment_id: String,
    created_at: String,
) -> Result<
    (
        MarketplaceFulfillmentRecord,
        MarketplaceFulfillmentAuditRecord,
    ),
    MarketplaceFulfillmentError,
> {
    let fulfillment_id = normalize_marketplace_optional_text(request.fulfillment_id)
        .or_else(|| normalize_marketplace_text(generated_fulfillment_id))
        .ok_or(MarketplaceFulfillmentError::EmptyFulfillmentId)?;
    let order_ref = normalize_marketplace_text(request.order_ref)
        .ok_or(MarketplaceFulfillmentError::EmptyOrderRef)?;
    let org_id = normalize_marketplace_text(request.org_id)
        .ok_or(MarketplaceFulfillmentError::EmptyOrgId)?;
    let carrier_ref = normalize_marketplace_text(request.carrier_ref)
        .ok_or(MarketplaceFulfillmentError::EmptyCarrierRef)?;
    let tracking_ref = normalize_marketplace_text(request.tracking_ref)
        .ok_or(MarketplaceFulfillmentError::EmptyTrackingRef)?;
    let actor_id = normalize_marketplace_text(request.actor_id)
        .ok_or(MarketplaceFulfillmentError::EmptyActorId)?;
    let order = order.ok_or(MarketplaceFulfillmentError::MissingOrder)?;
    if order.order_id != order_ref || order.org_id != org_id {
        return Err(MarketplaceFulfillmentError::OrderOrgMismatch {
            order_id: order.order_id.clone(),
            order_org_id: order.org_id.clone(),
            fulfillment_org_id: org_id,
        });
    }
    if order.status != MarketplaceOrderStatus::Confirmed {
        return Err(MarketplaceFulfillmentError::OrderNotConfirmed {
            order_id: order.order_id.clone(),
        });
    }
    let created_at = normalize_marketplace_fulfillment_timestamp(created_at)?;
    let record = MarketplaceFulfillmentRecord {
        fulfillment_id: fulfillment_id.clone(),
        order_ref,
        org_id,
        carrier_ref,
        tracking_ref,
        status: MarketplaceFulfillmentStatus::Pending,
        created_at: created_at.clone(),
        updated_at: created_at.clone(),
    };
    let audit = MarketplaceFulfillmentAuditRecord {
        audit_id: format!("{fulfillment_id}:pending:{created_at}"),
        fulfillment_id,
        from_status: None,
        to_status: MarketplaceFulfillmentStatus::Pending,
        actor_id,
        occurred_at: created_at,
    };
    Ok((record, audit))
}

pub fn transition_marketplace_fulfillment_status(
    record: &MarketplaceFulfillmentRecord,
    to: MarketplaceFulfillmentStatus,
    actor_id: String,
    occurred_at: String,
) -> Result<
    (
        MarketplaceFulfillmentRecord,
        MarketplaceFulfillmentAuditRecord,
    ),
    MarketplaceFulfillmentError,
> {
    let actor_id =
        normalize_marketplace_text(actor_id).ok_or(MarketplaceFulfillmentError::EmptyActorId)?;
    let occurred_at = normalize_marketplace_fulfillment_timestamp(occurred_at)?;
    if !valid_marketplace_fulfillment_transition(record.status, to) {
        return Err(MarketplaceFulfillmentError::InvalidStatusTransition {
            fulfillment_id: record.fulfillment_id.clone(),
            from: record.status,
            to,
        });
    }
    let mut updated = record.clone();
    updated.status = to;
    updated.updated_at = occurred_at.clone();
    let audit = MarketplaceFulfillmentAuditRecord {
        audit_id: format!("{}:{}:{occurred_at}", record.fulfillment_id, to.as_str()),
        fulfillment_id: record.fulfillment_id.clone(),
        from_status: Some(record.status),
        to_status: to,
        actor_id,
        occurred_at,
    };
    Ok((updated, audit))
}

pub fn create_marketplace_rating_record(
    request: MarketplaceRatingCreateRequest,
    order: Option<&MarketplaceOrderRecord>,
    participant_account_ids: &[String],
    existing_ratings: &[MarketplaceRatingRecord],
    generated_rating_id: String,
    created_at: String,
) -> Result<MarketplaceRatingRecord, MarketplaceRatingError> {
    let rating_id = normalize_marketplace_optional_text(request.rating_id)
        .or_else(|| normalize_marketplace_text(generated_rating_id))
        .ok_or(MarketplaceRatingError::EmptyRatingId)?;
    let order_ref = normalize_marketplace_text(request.order_ref)
        .ok_or(MarketplaceRatingError::EmptyOrderRef)?;
    let rater_account_id = normalize_marketplace_text(request.rater_account_id)
        .ok_or(MarketplaceRatingError::EmptyRaterAccountId)?;
    let ratee_account_id = normalize_marketplace_text(request.ratee_account_id)
        .ok_or(MarketplaceRatingError::EmptyRateeAccountId)?;
    let org_scope = normalize_marketplace_text(request.org_scope)
        .ok_or(MarketplaceRatingError::EmptyOrgScope)?;
    if !(request.score.is_finite() && (1.0..=5.0).contains(&request.score)) {
        return Err(MarketplaceRatingError::InvalidScore);
    }
    let order = order.ok_or(MarketplaceRatingError::MissingOrder)?;
    if order.order_id != order_ref || order.org_id != org_scope {
        return Err(MarketplaceRatingError::OrderOrgMismatch {
            order_id: order.order_id.clone(),
            order_org_id: order.org_id.clone(),
            org_scope,
        });
    }
    if !matches!(
        order.status,
        MarketplaceOrderStatus::Fulfilled | MarketplaceOrderStatus::Closed
    ) {
        return Err(MarketplaceRatingError::OrderNotRateable {
            order_id: order.order_id.clone(),
        });
    }
    let participants = participant_account_ids
        .iter()
        .filter_map(|participant| normalize_marketplace_text(participant.clone()))
        .collect::<BTreeSet<_>>();
    if !participants.contains(&rater_account_id) {
        return Err(MarketplaceRatingError::RaterNotParticipant {
            order_id: order.order_id.clone(),
            rater_account_id,
        });
    }
    if !participants.contains(&ratee_account_id) || rater_account_id == ratee_account_id {
        return Err(MarketplaceRatingError::RateeNotParticipant {
            order_id: order.order_id.clone(),
            ratee_account_id,
        });
    }
    if existing_ratings.iter().any(|rating| {
        rating.order_ref == order.order_id && rating.rater_account_id == rater_account_id
    }) {
        return Err(MarketplaceRatingError::DuplicateRaterForOrder {
            order_id: order.order_id.clone(),
            rater_account_id,
        });
    }
    let created_at = normalize_marketplace_rating_timestamp(created_at)?;
    Ok(MarketplaceRatingRecord {
        rating_id,
        order_ref,
        rater_account_id,
        ratee_account_id,
        score: request.score,
        comment: request.comment.and_then(normalize_marketplace_text),
        org_scope: order.org_id.clone(),
        created_at,
    })
}

pub fn aggregate_marketplace_ratings(
    account_id: String,
    org_scope: String,
    ratings: &[MarketplaceRatingRecord],
) -> Result<MarketplaceRatingAggregate, MarketplaceRatingError> {
    let account_id = normalize_marketplace_text(account_id)
        .ok_or(MarketplaceRatingError::EmptyRateeAccountId)?;
    let org_scope =
        normalize_marketplace_text(org_scope).ok_or(MarketplaceRatingError::EmptyOrgScope)?;
    let scoped = ratings
        .iter()
        .filter(|rating| rating.ratee_account_id == account_id && rating.org_scope == org_scope)
        .collect::<Vec<_>>();
    let rating_count = scoped.len() as u32;
    let average_score = (rating_count > 0)
        .then(|| scoped.iter().map(|rating| rating.score).sum::<f64>() / f64::from(rating_count));
    Ok(MarketplaceRatingAggregate {
        account_id,
        org_scope,
        rating_count,
        average_score,
    })
}

pub fn compute_marketplace_demand_forecast(
    request: MarketplaceDemandForecastRequest,
    field: Option<&FieldRecord>,
    product_evidence_refs: Vec<String>,
    generated_forecast_id: String,
    created_at: String,
) -> Result<MarketplaceDemandForecastRecord, MarketplaceDemandForecastError> {
    let forecast_id = normalize_marketplace_optional_text(request.forecast_id)
        .or_else(|| normalize_marketplace_text(generated_forecast_id))
        .ok_or(MarketplaceDemandForecastError::EmptyForecastId)?;
    let org_id = normalize_marketplace_text(request.org_id)
        .ok_or(MarketplaceDemandForecastError::EmptyOrgId)?;
    let field_id = normalize_marketplace_text(request.field_id)
        .ok_or(MarketplaceDemandForecastError::EmptyFieldId)?;
    let horizon = normalize_marketplace_text(request.horizon)
        .ok_or(MarketplaceDemandForecastError::EmptyHorizon)?;
    let created_at = normalize_marketplace_demand_timestamp(created_at)?;
    let mut evidence_refs = product_evidence_refs
        .into_iter()
        .filter_map(normalize_marketplace_text)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let field = field.filter(|field| field.field_id == field_id && field.org_id == org_id);
    let Some(field) = field else {
        return Ok(marketplace_demand_no_basis_record(
            forecast_id,
            org_id,
            field_id,
            request.item_kind,
            horizon,
            evidence_refs,
            created_at,
        ));
    };
    evidence_refs.push(format!(
        "field:{}:area_ha:{:.4}",
        field.field_id,
        field.area_ha.unwrap_or_default()
    ));
    if let Some(crop) = field
        .crop
        .as_ref()
        .and_then(|crop| normalize_marketplace_text(crop.clone()))
    {
        evidence_refs.push(format!("field:{}:crop:{crop}", field.field_id));
    }
    if let Some(season) = field
        .season
        .as_ref()
        .and_then(|season| normalize_marketplace_text(season.clone()))
    {
        evidence_refs.push(format!("field:{}:season:{season}", field.field_id));
    }
    evidence_refs.sort();
    evidence_refs.dedup();

    let area_ha = field.area_ha.filter(|area| area.is_finite() && *area > 0.0);
    let has_product_evidence = evidence_refs
        .iter()
        .any(|evidence_ref| evidence_ref.starts_with("product:"));
    let Some(area_ha) = area_ha else {
        return Ok(marketplace_demand_no_basis_record(
            forecast_id,
            org_id,
            field_id,
            request.item_kind,
            horizon,
            evidence_refs,
            created_at,
        ));
    };
    if !has_product_evidence {
        return Ok(marketplace_demand_no_basis_record(
            forecast_id,
            org_id,
            field_id,
            request.item_kind,
            horizon,
            evidence_refs,
            created_at,
        ));
    }

    let value = marketplace_demand_rate_per_ha(request.item_kind) * area_ha;
    let uncertainty_band = request
        .ai_assisted
        .then(|| MarketplaceDemandUncertaintyBand {
            low: value * 0.85,
            high: value * 1.15,
        });
    Ok(MarketplaceDemandForecastRecord {
        forecast_id,
        org_id,
        field_id,
        item_kind: request.item_kind,
        horizon,
        value: Some(value),
        evidence_refs,
        status: MarketplaceDemandForecastStatus::Ready,
        uncertainty_band,
        method: if request.ai_assisted {
            "agbot_marketplace_demand_v1_ai_assisted_baseline".to_string()
        } else {
            "agbot_marketplace_demand_v1_deterministic_baseline".to_string()
        },
        created_at,
    })
}

pub fn assemble_marketplace_org_report(
    request: MarketplaceOrgReportRequest,
    orders: &[MarketplaceOrderRecord],
    listings: &[MarketplaceListingRecord],
    inventory: &[MarketplaceInventoryRecord],
    generated_at: String,
) -> Result<MarketplaceOrgReport, MarketplaceReportError> {
    let org_id =
        normalize_marketplace_text(request.org_id).ok_or(MarketplaceReportError::EmptyOrgId)?;
    let period_from = normalize_marketplace_text(request.period.from)
        .ok_or(MarketplaceReportError::EmptyPeriodStart)?;
    let period_to = normalize_marketplace_text(request.period.to)
        .ok_or(MarketplaceReportError::EmptyPeriodEnd)?;
    let parsed_from = parse_marketplace_report_timestamp(&period_from)?;
    let parsed_to = parse_marketplace_report_timestamp(&period_to)?;
    if parsed_to < parsed_from {
        return Err(MarketplaceReportError::InvalidPeriodRange);
    }
    let generated_at = normalize_marketplace_report_timestamp(generated_at)?;

    let scoped_orders = orders
        .iter()
        .filter(|order| order.org_id == org_id)
        .filter(|order| {
            parse_marketplace_report_timestamp(&order.created_at)
                .map(|created_at| created_at >= parsed_from && created_at <= parsed_to)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    let mut status_counts = BTreeMap::<MarketplaceOrderStatus, u32>::new();
    let mut source_order_ids = BTreeSet::<String>::new();
    let mut sales_total = 0.0;
    for order in scoped_orders {
        *status_counts.entry(order.status).or_default() += 1;
        source_order_ids.insert(order.order_id.clone());
        if matches!(
            order.status,
            MarketplaceOrderStatus::Confirmed
                | MarketplaceOrderStatus::Fulfilled
                | MarketplaceOrderStatus::Closed
        ) {
            sales_total += order.line_total;
        }
    }
    let order_counts_by_status = status_counts
        .into_iter()
        .map(|(status, count)| MarketplaceOrderStatusCount { status, count })
        .collect::<Vec<_>>();
    let listing_count = listings
        .iter()
        .filter(|listing| listing.org_id == org_id)
        .count() as u32;
    let inventory_on_hand_total = inventory
        .iter()
        .filter(|record| record.org_id == org_id)
        .map(|record| record.on_hand)
        .sum();
    let inventory_reserved_total = inventory
        .iter()
        .filter(|record| record.org_id == org_id)
        .map(|record| record.reserved)
        .sum();

    Ok(MarketplaceOrgReport {
        org_id,
        period: MarketplaceReportPeriod {
            from: period_from,
            to: period_to,
        },
        sales_total,
        procurement_total: sales_total,
        order_counts_by_status,
        listing_count,
        inventory_on_hand_total,
        inventory_reserved_total,
        source_order_ids: source_order_ids.into_iter().collect(),
        generated_at,
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

pub fn parse_marketplace_catalog_item_kind(
    value: &str,
) -> Result<MarketplaceCatalogItemKind, MarketplaceCatalogError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "input" => Ok(MarketplaceCatalogItemKind::Input),
        "produce" => Ok(MarketplaceCatalogItemKind::Produce),
        _ => Err(MarketplaceCatalogError::UnsupportedItemKind {
            value: value.to_string(),
        }),
    }
}

pub fn parse_marketplace_catalog_category(
    value: &str,
) -> Result<MarketplaceCatalogCategory, MarketplaceCatalogError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "seed" => Ok(MarketplaceCatalogCategory::Seed),
        "fertilizer" => Ok(MarketplaceCatalogCategory::Fertilizer),
        "crop_protection" => Ok(MarketplaceCatalogCategory::CropProtection),
        "service" => Ok(MarketplaceCatalogCategory::Service),
        "grain" => Ok(MarketplaceCatalogCategory::Grain),
        "produce" => Ok(MarketplaceCatalogCategory::Produce),
        _ => Err(MarketplaceCatalogError::UnsupportedCategory {
            value: value.to_string(),
        }),
    }
}

pub fn parse_marketplace_unit_of_measure(
    value: &str,
) -> Result<MarketplaceUnitOfMeasure, MarketplaceCatalogError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "kg" => Ok(MarketplaceUnitOfMeasure::Kg),
        "tonne" => Ok(MarketplaceUnitOfMeasure::Tonne),
        "litre" => Ok(MarketplaceUnitOfMeasure::Litre),
        "bag" => Ok(MarketplaceUnitOfMeasure::Bag),
        "bushel" => Ok(MarketplaceUnitOfMeasure::Bushel),
        "crate" => Ok(MarketplaceUnitOfMeasure::Crate),
        "acre" => Ok(MarketplaceUnitOfMeasure::Acre),
        _ => Err(MarketplaceCatalogError::UnsupportedUnit {
            value: value.to_string(),
        }),
    }
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

pub fn parse_marketplace_listing_status(
    value: &str,
) -> Result<MarketplaceListingStatus, MarketplaceListingError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "draft" => Ok(MarketplaceListingStatus::Draft),
        "published" => Ok(MarketplaceListingStatus::Published),
        "closed" => Ok(MarketplaceListingStatus::Closed),
        _ => Err(MarketplaceListingError::UnsupportedStatus {
            value: value.to_string(),
        }),
    }
}

pub fn parse_marketplace_order_status(
    value: &str,
) -> Result<MarketplaceOrderStatus, MarketplaceOrderError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "placed" => Ok(MarketplaceOrderStatus::Placed),
        "confirmed" => Ok(MarketplaceOrderStatus::Confirmed),
        "fulfilled" => Ok(MarketplaceOrderStatus::Fulfilled),
        "closed" => Ok(MarketplaceOrderStatus::Closed),
        "cancelled" => Ok(MarketplaceOrderStatus::Cancelled),
        _ => Err(MarketplaceOrderError::UnsupportedStatus {
            value: value.to_string(),
        }),
    }
}

pub fn parse_marketplace_fulfillment_status(
    value: &str,
) -> Result<MarketplaceFulfillmentStatus, MarketplaceFulfillmentError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "pending" => Ok(MarketplaceFulfillmentStatus::Pending),
        "shipped" => Ok(MarketplaceFulfillmentStatus::Shipped),
        "delivered" => Ok(MarketplaceFulfillmentStatus::Delivered),
        "failed" => Ok(MarketplaceFulfillmentStatus::Failed),
        _ => Err(MarketplaceFulfillmentError::UnsupportedStatus {
            value: value.to_string(),
        }),
    }
}

pub fn parse_marketplace_demand_forecast_status(
    value: &str,
) -> Result<MarketplaceDemandForecastStatus, MarketplaceDemandForecastError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "ready" => Ok(MarketplaceDemandForecastStatus::Ready),
        "no_basis" => Ok(MarketplaceDemandForecastStatus::NoBasis),
        _ => Err(MarketplaceDemandForecastError::UnsupportedStatus {
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

fn valid_marketplace_order_transition(
    from: MarketplaceOrderStatus,
    to: MarketplaceOrderStatus,
) -> bool {
    matches!(
        (from, to),
        (
            MarketplaceOrderStatus::Placed,
            MarketplaceOrderStatus::Confirmed
        ) | (
            MarketplaceOrderStatus::Placed,
            MarketplaceOrderStatus::Cancelled
        ) | (
            MarketplaceOrderStatus::Confirmed,
            MarketplaceOrderStatus::Fulfilled
        ) | (
            MarketplaceOrderStatus::Confirmed,
            MarketplaceOrderStatus::Cancelled
        ) | (
            MarketplaceOrderStatus::Fulfilled,
            MarketplaceOrderStatus::Closed
        )
    )
}

fn valid_marketplace_fulfillment_transition(
    from: MarketplaceFulfillmentStatus,
    to: MarketplaceFulfillmentStatus,
) -> bool {
    matches!(
        (from, to),
        (
            MarketplaceFulfillmentStatus::Pending,
            MarketplaceFulfillmentStatus::Shipped
        ) | (
            MarketplaceFulfillmentStatus::Pending,
            MarketplaceFulfillmentStatus::Failed
        ) | (
            MarketplaceFulfillmentStatus::Shipped,
            MarketplaceFulfillmentStatus::Delivered
        ) | (
            MarketplaceFulfillmentStatus::Shipped,
            MarketplaceFulfillmentStatus::Failed
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

fn parse_marketplace_listing_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, MarketplaceListingError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| MarketplaceListingError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

fn validate_marketplace_inventory_quantities(
    on_hand: f64,
    reserved: f64,
) -> Result<(), MarketplaceInventoryError> {
    if !(on_hand.is_finite() && on_hand >= 0.0 && reserved.is_finite() && reserved >= 0.0) {
        return Err(MarketplaceInventoryError::InvalidQuantity);
    }
    if reserved > on_hand {
        return Err(MarketplaceInventoryError::ReservedExceedsOnHand);
    }
    Ok(())
}

fn validate_marketplace_inventory_delta(qty: f64) -> Result<(), MarketplaceInventoryError> {
    if !(qty.is_finite() && qty > 0.0) {
        return Err(MarketplaceInventoryError::InvalidQuantity);
    }
    Ok(())
}

fn normalize_marketplace_inventory_timestamp(
    timestamp: String,
) -> Result<String, MarketplaceInventoryError> {
    let timestamp = normalize_marketplace_text(timestamp).ok_or(
        MarketplaceInventoryError::InvalidTimestamp {
            timestamp: String::new(),
        },
    )?;
    parse_marketplace_inventory_timestamp(&timestamp)?;
    Ok(timestamp)
}

fn parse_marketplace_inventory_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, MarketplaceInventoryError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| MarketplaceInventoryError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

fn validate_marketplace_order_qty(qty: f64) -> Result<(), MarketplaceOrderError> {
    if !(qty.is_finite() && qty > 0.0) {
        return Err(MarketplaceOrderError::InvalidQuantity);
    }
    Ok(())
}

fn normalize_marketplace_order_timestamp(
    timestamp: String,
) -> Result<String, MarketplaceOrderError> {
    let timestamp =
        normalize_marketplace_text(timestamp).ok_or(MarketplaceOrderError::InvalidTimestamp {
            timestamp: String::new(),
        })?;
    parse_marketplace_order_timestamp(&timestamp)?;
    Ok(timestamp)
}

fn parse_marketplace_order_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, MarketplaceOrderError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| MarketplaceOrderError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

fn normalize_marketplace_fulfillment_timestamp(
    timestamp: String,
) -> Result<String, MarketplaceFulfillmentError> {
    let timestamp = normalize_marketplace_text(timestamp).ok_or(
        MarketplaceFulfillmentError::InvalidTimestamp {
            timestamp: String::new(),
        },
    )?;
    parse_marketplace_fulfillment_timestamp(&timestamp)?;
    Ok(timestamp)
}

fn parse_marketplace_fulfillment_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, MarketplaceFulfillmentError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| MarketplaceFulfillmentError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
}

fn normalize_marketplace_rating_timestamp(
    timestamp: String,
) -> Result<String, MarketplaceRatingError> {
    let timestamp =
        normalize_marketplace_text(timestamp).ok_or(MarketplaceRatingError::InvalidTimestamp {
            timestamp: String::new(),
        })?;
    chrono::DateTime::parse_from_rfc3339(&timestamp).map_err(|_| {
        MarketplaceRatingError::InvalidTimestamp {
            timestamp: timestamp.clone(),
        }
    })?;
    Ok(timestamp)
}

fn normalize_marketplace_demand_timestamp(
    timestamp: String,
) -> Result<String, MarketplaceDemandForecastError> {
    let timestamp = normalize_marketplace_text(timestamp).ok_or(
        MarketplaceDemandForecastError::InvalidTimestamp {
            timestamp: String::new(),
        },
    )?;
    chrono::DateTime::parse_from_rfc3339(&timestamp).map_err(|_| {
        MarketplaceDemandForecastError::InvalidTimestamp {
            timestamp: timestamp.clone(),
        }
    })?;
    Ok(timestamp)
}

fn marketplace_demand_rate_per_ha(item_kind: MarketplaceCatalogItemKind) -> f64 {
    match item_kind {
        MarketplaceCatalogItemKind::Input => 1.0,
        MarketplaceCatalogItemKind::Produce => 7.5,
    }
}

fn marketplace_demand_no_basis_record(
    forecast_id: String,
    org_id: String,
    field_id: String,
    item_kind: MarketplaceCatalogItemKind,
    horizon: String,
    evidence_refs: Vec<String>,
    created_at: String,
) -> MarketplaceDemandForecastRecord {
    MarketplaceDemandForecastRecord {
        forecast_id,
        org_id,
        field_id,
        item_kind,
        horizon,
        value: None,
        evidence_refs,
        status: MarketplaceDemandForecastStatus::NoBasis,
        uncertainty_band: None,
        method: "agbot_marketplace_demand_v1_no_basis".to_string(),
        created_at,
    }
}

fn normalize_marketplace_report_timestamp(
    timestamp: String,
) -> Result<String, MarketplaceReportError> {
    let timestamp =
        normalize_marketplace_text(timestamp).ok_or(MarketplaceReportError::InvalidTimestamp {
            timestamp: String::new(),
        })?;
    parse_marketplace_report_timestamp(&timestamp)?;
    Ok(timestamp)
}

fn parse_marketplace_report_timestamp(
    timestamp: &str,
) -> Result<chrono::DateTime<chrono::Utc>, MarketplaceReportError> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| MarketplaceReportError::InvalidTimestamp {
            timestamp: timestamp.to_string(),
        })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarbonFootprintInputKind {
    DieselLiters,
    FertilizerNitrogenKg,
    ElectricityKwh,
    FieldPasses,
}

impl CarbonFootprintInputKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CarbonFootprintInputKind::DieselLiters => "diesel_liters",
            CarbonFootprintInputKind::FertilizerNitrogenKg => "fertilizer_nitrogen_kg",
            CarbonFootprintInputKind::ElectricityKwh => "electricity_kwh",
            CarbonFootprintInputKind::FieldPasses => "field_passes",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CarbonFootprintInput {
    pub kind: CarbonFootprintInputKind,
    pub quantity: f64,
    pub unit: String,
    pub evidence_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CarbonEmissionFactor {
    pub input_kind: CarbonFootprintInputKind,
    pub factor_kg_co2e_per_unit: f64,
    pub factor_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CarbonFootprintFactorSet {
    pub version: String,
    pub factors: Vec<CarbonEmissionFactor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CarbonFootprintComputeRequest {
    #[serde(default)]
    pub footprint_id: Option<String>,
    pub record_id: String,
    pub operation_id: String,
    pub inputs: Vec<CarbonFootprintInput>,
    pub factor_set: CarbonFootprintFactorSet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarbonFootprintStatus {
    Computed,
    InsufficientInputs,
}

impl CarbonFootprintStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CarbonFootprintStatus::Computed => "computed",
            CarbonFootprintStatus::InsufficientInputs => "insufficient_inputs",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CarbonFootprintResult {
    pub footprint_id: String,
    pub record_id: String,
    pub operation_id: String,
    pub value_co2e: Option<f64>,
    pub inputs: Vec<CarbonFootprintInput>,
    pub factor_set_version: String,
    pub factors: Vec<CarbonEmissionFactor>,
    pub evidence_refs: Vec<String>,
    pub status: CarbonFootprintStatus,
    pub result_hash: String,
    pub computed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiomassLayerInput {
    pub layer_ref: String,
    pub width: u32,
    pub height: u32,
    pub values: Vec<f64>,
    pub spatial_ref: RasterSpatialRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiomassEstimateRequest {
    #[serde(default)]
    pub estimate_id: Option<String>,
    pub record_id: String,
    pub canopy_layer: BiomassLayerInput,
    pub vegetation_index_layer: BiomassLayerInput,
    pub method_version: String,
    #[serde(default = "default_biomass_coefficient")]
    pub biomass_coefficient: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiomassEstimateResult {
    pub estimate_id: String,
    pub record_id: String,
    pub biomass_value: f64,
    pub area: f64,
    pub crs: String,
    pub extent: GeoBounds,
    pub resolution: RasterResolution,
    pub source_layer_refs: Vec<String>,
    pub method_version: String,
    pub result_hash: String,
    pub computed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SustainabilityBaselineCreateRequest {
    #[serde(default)]
    pub baseline_id: Option<String>,
    pub field_id: String,
    pub season_id: String,
    pub metric_type: SustainabilityMetricType,
    pub metric_value: f64,
    pub source_record_id: String,
    pub method_version: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SustainabilityBaselineRecord {
    pub baseline_id: String,
    pub field_id: String,
    pub season_id: String,
    pub metric_type: SustainabilityMetricType,
    pub metric_value: f64,
    pub source_record_id: String,
    pub method_version: String,
    pub evidence_refs: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SustainabilityComparisonRequest {
    #[serde(default)]
    pub comparison_id: Option<String>,
    pub field_id: String,
    pub baseline_season_id: String,
    pub current_season_id: String,
    pub metric_type: SustainabilityMetricType,
    pub current_value: f64,
    pub current_source_record_id: String,
    pub method_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SustainabilityComparisonStatus {
    Compared,
    NoBaseline,
}

impl SustainabilityComparisonStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SustainabilityComparisonStatus::Compared => "compared",
            SustainabilityComparisonStatus::NoBaseline => "no_baseline",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SustainabilityTrend {
    Increased,
    Decreased,
    Unchanged,
    NoBaseline,
}

impl SustainabilityTrend {
    pub fn as_str(self) -> &'static str {
        match self {
            SustainabilityTrend::Increased => "increased",
            SustainabilityTrend::Decreased => "decreased",
            SustainabilityTrend::Unchanged => "unchanged",
            SustainabilityTrend::NoBaseline => "no_baseline",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SustainabilityComparisonResult {
    pub comparison_id: String,
    pub field_id: String,
    pub baseline_season_id: String,
    pub current_season_id: String,
    pub metric_type: SustainabilityMetricType,
    pub baseline_value: Option<f64>,
    pub current_value: f64,
    pub delta: Option<f64>,
    pub trend: SustainabilityTrend,
    pub status: SustainabilityComparisonStatus,
    pub baseline_source_record_id: Option<String>,
    pub current_source_record_id: String,
    pub evidence_refs: Vec<String>,
    pub method_version: String,
    pub result_hash: String,
    pub compared_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SustainabilityKpiDirection {
    HigherIsBetter,
    LowerIsBetter,
}

impl SustainabilityKpiDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            SustainabilityKpiDirection::HigherIsBetter => "higher_is_better",
            SustainabilityKpiDirection::LowerIsBetter => "lower_is_better",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SustainabilityKpiTrackingRequest {
    #[serde(default)]
    pub kpi_id: Option<String>,
    pub field_id: String,
    pub season_id: String,
    pub metric_ref: String,
    #[serde(default)]
    pub current_value: Option<f64>,
    pub target_value: f64,
    pub direction: SustainabilityKpiDirection,
    #[serde(default = "default_sustainability_kpi_at_risk_fraction")]
    pub at_risk_fraction: f64,
    pub method_version: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SustainabilityKpiStatus {
    OnTrack,
    AtRisk,
    Met,
    NoData,
}

impl SustainabilityKpiStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SustainabilityKpiStatus::OnTrack => "on_track",
            SustainabilityKpiStatus::AtRisk => "at_risk",
            SustainabilityKpiStatus::Met => "met",
            SustainabilityKpiStatus::NoData => "no_data",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SustainabilityKpiTrackingResult {
    pub kpi_id: String,
    pub field_id: String,
    pub season_id: String,
    pub metric_ref: String,
    pub current_value: Option<f64>,
    pub target_value: f64,
    pub direction: SustainabilityKpiDirection,
    pub at_risk_fraction: f64,
    pub status: SustainabilityKpiStatus,
    pub evidence_refs: Vec<String>,
    pub method_version: String,
    pub result_hash: String,
    pub computed_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SustainabilityMrvOutputKind {
    CarbonFootprint,
    BiomassEstimate,
    SustainabilityKpi,
}

impl SustainabilityMrvOutputKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SustainabilityMrvOutputKind::CarbonFootprint => "carbon_footprint",
            SustainabilityMrvOutputKind::BiomassEstimate => "biomass_estimate",
            SustainabilityMrvOutputKind::SustainabilityKpi => "sustainability_kpi",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SustainabilityMrvTrailCreateRequest {
    #[serde(default)]
    pub trail_id: Option<String>,
    pub output_ref: String,
    pub output_kind: SustainabilityMrvOutputKind,
    #[serde(default)]
    pub input_layer_refs: Vec<String>,
    pub method: String,
    pub method_version: String,
    #[serde(default)]
    pub crs: Option<String>,
    #[serde(default)]
    pub extent: Option<GeoBounds>,
    #[serde(default)]
    pub parameters: BTreeMap<String, String>,
    pub audit_id: String,
    pub result_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SustainabilityMrvTrail {
    pub trail_id: String,
    pub output_ref: String,
    pub output_kind: SustainabilityMrvOutputKind,
    pub input_layer_refs: Vec<String>,
    pub method: String,
    pub method_version: String,
    pub crs: String,
    pub extent: GeoBounds,
    pub parameters: BTreeMap<String, String>,
    pub audit_id: String,
    pub result_hash: String,
    pub rederived_result_hash: String,
    pub certification_ready: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SustainabilityCertificationOutputItem {
    pub output_ref: String,
    pub output_kind: SustainabilityMrvOutputKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub method_version: String,
    pub result_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SustainabilityCertificationEvidencePackRequest {
    #[serde(default)]
    pub pack_id: Option<String>,
    pub claim_id: String,
    pub claim_type: String,
    pub field_id: String,
    pub season_id: String,
    #[serde(default)]
    pub claimed_output_refs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<SustainabilityCertificationOutputItem>,
    #[serde(default)]
    pub evidence_layer_refs: Vec<String>,
    #[serde(default)]
    pub mrv_trails: Vec<SustainabilityMrvTrail>,
    pub method_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SustainabilityCertificationEvidencePack {
    pub pack_id: String,
    pub claim_id: String,
    pub claim_type: String,
    pub field_id: String,
    pub season_id: String,
    pub claimed_output_refs: Vec<String>,
    pub outputs: Vec<SustainabilityCertificationOutputItem>,
    pub evidence_layer_refs: Vec<String>,
    pub mrv_trails: Vec<SustainabilityMrvTrail>,
    pub audit_ids: Vec<String>,
    pub result_hash: String,
    pub pack_hash: String,
    pub method_version: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiodiversityImageryLayer {
    pub layer_ref: String,
    pub width: u32,
    pub height: u32,
    pub values: Vec<f64>,
    pub spatial_ref: RasterSpatialRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiodiversityProxyRequest {
    #[serde(default)]
    pub proxy_id: Option<String>,
    pub field_id: String,
    pub layer: BiodiversityImageryLayer,
    pub method_version: String,
    #[serde(default = "default_biodiversity_cover_threshold")]
    pub cover_threshold: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BiodiversityProxyStatus {
    Computed,
    NoSignal,
}

impl BiodiversityProxyStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            BiodiversityProxyStatus::Computed => "computed",
            BiodiversityProxyStatus::NoSignal => "no_signal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiodiversityProxyResult {
    pub proxy_id: String,
    pub field_id: String,
    pub heterogeneity_score: Option<f64>,
    pub cover_fraction: Option<f64>,
    pub uncertainty: f64,
    pub status: BiodiversityProxyStatus,
    pub crs: String,
    pub extent: GeoBounds,
    pub source_layer_refs: Vec<String>,
    pub method_version: String,
    pub result_hash: String,
    pub computed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCarbonEvidenceInput {
    pub evidence_ref: String,
    pub value: f64,
    #[serde(default = "default_soil_carbon_weight")]
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCarbonPracticeInput {
    pub practice_ref: String,
    pub carbon_delta: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCarbonProxyRequest {
    #[serde(default)]
    pub proxy_id: Option<String>,
    pub record_id: String,
    pub field_id: String,
    #[serde(default)]
    pub index_inputs: Vec<SoilCarbonEvidenceInput>,
    #[serde(default)]
    pub biomass_inputs: Vec<SoilCarbonEvidenceInput>,
    #[serde(default)]
    pub practice_inputs: Vec<SoilCarbonPracticeInput>,
    pub method_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoilCarbonProxyStatus {
    Computed,
    InsufficientEvidence,
}

impl SoilCarbonProxyStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SoilCarbonProxyStatus::Computed => "computed",
            SoilCarbonProxyStatus::InsufficientEvidence => "insufficient_evidence",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCarbonUncertaintyBand {
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCarbonProxyResult {
    pub proxy_id: String,
    pub record_id: String,
    pub field_id: String,
    pub proxy_value: Option<f64>,
    pub uncertainty_band: Option<SoilCarbonUncertaintyBand>,
    pub status: SoilCarbonProxyStatus,
    pub evidence_refs: Vec<String>,
    pub method_version: String,
    pub result_hash: String,
    pub computed_at: String,
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CarbonFootprintError {
    #[error("carbon footprint_id cannot be empty")]
    EmptyFootprintId,
    #[error("carbon footprint record_id cannot be empty")]
    EmptyRecordId,
    #[error("carbon footprint operation_id cannot be empty")]
    EmptyOperationId,
    #[error("carbon factor set version cannot be empty")]
    EmptyFactorSetVersion,
    #[error("carbon footprint computed_at cannot be empty")]
    EmptyComputedAt,
    #[error("carbon footprint input quantity is invalid")]
    InvalidInputQuantity,
    #[error("carbon footprint emission factor is invalid")]
    InvalidEmissionFactor,
    #[error("unsupported carbon footprint input kind {value}")]
    UnsupportedInputKind { value: String },
    #[error("unsupported carbon footprint status {value}")]
    UnsupportedStatus { value: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum BiomassEstimateError {
    #[error("biomass estimate_id cannot be empty")]
    EmptyEstimateId,
    #[error("biomass record_id cannot be empty")]
    EmptyRecordId,
    #[error("biomass method_version cannot be empty")]
    EmptyMethodVersion,
    #[error("biomass computed_at cannot be empty")]
    EmptyComputedAt,
    #[error("biomass layer_ref cannot be empty")]
    EmptyLayerRef,
    #[error("biomass grid values length does not match width * height for {layer_ref}")]
    GridSizeMismatch { layer_ref: String },
    #[error("biomass grid value is invalid for {layer_ref}")]
    InvalidGridValue { layer_ref: String },
    #[error("biomass coefficient is invalid")]
    InvalidCoefficient,
    #[error("biomass raster spatial reference invalid: {0}")]
    SpatialRef(#[from] RasterSpatialRefError),
    #[error("biomass CRS mismatch: canopy {canopy_crs}, vegetation index {index_crs}")]
    CrsMismatch {
        canopy_crs: String,
        index_crs: String,
    },
    #[error("biomass extent mismatch between canopy and vegetation index")]
    ExtentMismatch,
    #[error("biomass resolution mismatch between canopy and vegetation index")]
    ResolutionMismatch,
    #[error("biomass dimensions mismatch between canopy and vegetation index")]
    DimensionMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SustainabilityBaselineError {
    #[error("sustainability baseline_id cannot be empty")]
    EmptyBaselineId,
    #[error("sustainability comparison_id cannot be empty")]
    EmptyComparisonId,
    #[error("sustainability field_id cannot be empty")]
    EmptyFieldId,
    #[error("sustainability baseline season_id cannot be empty")]
    EmptyBaselineSeasonId,
    #[error("sustainability current season_id cannot be empty")]
    EmptyCurrentSeasonId,
    #[error("sustainability source_record_id cannot be empty")]
    EmptySourceRecordId,
    #[error("sustainability method_version cannot be empty")]
    EmptyMethodVersion,
    #[error("sustainability timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("sustainability metric value is invalid")]
    InvalidMetricValue,
    #[error("unsupported sustainability comparison status {value}")]
    UnsupportedComparisonStatus { value: String },
    #[error("unsupported sustainability trend {value}")]
    UnsupportedTrend { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SustainabilityKpiError {
    #[error("sustainability KPI kpi_id cannot be empty")]
    EmptyKpiId,
    #[error("sustainability KPI field_id cannot be empty")]
    EmptyFieldId,
    #[error("sustainability KPI season_id cannot be empty")]
    EmptySeasonId,
    #[error("sustainability KPI metric_ref cannot be empty")]
    EmptyMetricRef,
    #[error("sustainability KPI method_version cannot be empty")]
    EmptyMethodVersion,
    #[error("sustainability KPI computed_at cannot be empty")]
    EmptyComputedAt,
    #[error("sustainability KPI current_value is invalid")]
    InvalidCurrentValue,
    #[error("sustainability KPI target_value is invalid")]
    InvalidTargetValue,
    #[error("sustainability KPI at_risk_fraction is invalid")]
    InvalidAtRiskFraction,
    #[error("unsupported sustainability KPI direction {value}")]
    UnsupportedDirection { value: String },
    #[error("unsupported sustainability KPI status {value}")]
    UnsupportedStatus { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SustainabilityMrvTrailError {
    #[error("MRV trail_id cannot be empty")]
    EmptyTrailId,
    #[error("MRV output_ref cannot be empty")]
    EmptyOutputRef,
    #[error("MRV input_layer_refs cannot be empty")]
    EmptyInputLayerRefs,
    #[error("MRV method cannot be empty")]
    EmptyMethod,
    #[error("MRV method_version cannot be empty")]
    EmptyMethodVersion,
    #[error("MRV CRS cannot be empty")]
    EmptyCrs,
    #[error("MRV extent is required")]
    MissingExtent,
    #[error("MRV extent is invalid")]
    InvalidExtent,
    #[error("MRV audit_id cannot be empty")]
    EmptyAuditId,
    #[error("MRV result_hash cannot be empty")]
    EmptyResultHash,
    #[error("MRV created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("unsupported MRV output kind {value}")]
    UnsupportedOutputKind { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SustainabilityCertificationEvidencePackError {
    #[error("certification evidence pack_id cannot be empty")]
    EmptyPackId,
    #[error("certification claim_id cannot be empty")]
    EmptyClaimId,
    #[error("certification claim_type cannot be empty")]
    EmptyClaimType,
    #[error("certification field_id cannot be empty")]
    EmptyFieldId,
    #[error("certification season_id cannot be empty")]
    EmptySeasonId,
    #[error("certification method_version cannot be empty")]
    EmptyMethodVersion,
    #[error("certification created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("certification claimed_output_refs cannot be empty")]
    EmptyClaimedOutputRefs,
    #[error("certification output_ref cannot be empty")]
    EmptyOutputRef,
    #[error("certification output method_version cannot be empty for {output_ref}")]
    EmptyOutputMethodVersion { output_ref: String },
    #[error("certification output result_hash cannot be empty for {output_ref}")]
    EmptyOutputResultHash { output_ref: String },
    #[error("certification output value is invalid for {output_ref}")]
    InvalidOutputValue { output_ref: String },
    #[error("certification claimed output is missing: {output_ref}")]
    MissingClaimedOutput { output_ref: String },
    #[error("certification MRV trail is missing for claimed output {output_ref}")]
    MissingMrvTrail { output_ref: String },
    #[error("certification MRV trail is not certification-ready for claimed output {output_ref}")]
    IncompleteMrvTrail { output_ref: String },
    #[error("certification MRV result hash mismatch for claimed output {output_ref}")]
    MrvResultHashMismatch { output_ref: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum BiodiversityProxyError {
    #[error("biodiversity proxy_id cannot be empty")]
    EmptyProxyId,
    #[error("biodiversity field_id cannot be empty")]
    EmptyFieldId,
    #[error("biodiversity layer_ref cannot be empty")]
    EmptyLayerRef,
    #[error("biodiversity method_version cannot be empty")]
    EmptyMethodVersion,
    #[error("biodiversity computed_at cannot be empty")]
    EmptyComputedAt,
    #[error("biodiversity grid values length does not match width * height for {layer_ref}")]
    GridSizeMismatch { layer_ref: String },
    #[error("biodiversity grid value is invalid for {layer_ref}")]
    InvalidGridValue { layer_ref: String },
    #[error("biodiversity cover threshold is invalid")]
    InvalidCoverThreshold,
    #[error("biodiversity raster spatial reference invalid: {0}")]
    SpatialRef(#[from] RasterSpatialRefError),
    #[error("unsupported biodiversity proxy status {value}")]
    UnsupportedStatus { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SoilCarbonProxyError {
    #[error("soil-carbon proxy_id cannot be empty")]
    EmptyProxyId,
    #[error("soil-carbon record_id cannot be empty")]
    EmptyRecordId,
    #[error("soil-carbon field_id cannot be empty")]
    EmptyFieldId,
    #[error("soil-carbon method_version cannot be empty")]
    EmptyMethodVersion,
    #[error("soil-carbon computed_at cannot be empty")]
    EmptyComputedAt,
    #[error("soil-carbon evidence_ref cannot be empty")]
    EmptyEvidenceRef,
    #[error("soil-carbon practice_ref cannot be empty")]
    EmptyPracticeRef,
    #[error("soil-carbon evidence value is invalid")]
    InvalidEvidenceValue,
    #[error("soil-carbon evidence weight is invalid")]
    InvalidEvidenceWeight,
    #[error("soil-carbon practice delta is invalid")]
    InvalidPracticeDelta,
    #[error("unsupported soil-carbon proxy status {value}")]
    UnsupportedStatus { value: String },
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

pub fn compute_carbon_footprint(
    request: CarbonFootprintComputeRequest,
    generated_footprint_id: String,
    computed_at: String,
) -> Result<CarbonFootprintResult, CarbonFootprintError> {
    let footprint_id = normalize_sustainability_optional_text(request.footprint_id)
        .or_else(|| normalize_sustainability_text(generated_footprint_id))
        .ok_or(CarbonFootprintError::EmptyFootprintId)?;
    let record_id = normalize_sustainability_text(request.record_id)
        .ok_or(CarbonFootprintError::EmptyRecordId)?;
    let operation_id = normalize_sustainability_text(request.operation_id)
        .ok_or(CarbonFootprintError::EmptyOperationId)?;
    let factor_set_version = normalize_sustainability_text(request.factor_set.version)
        .ok_or(CarbonFootprintError::EmptyFactorSetVersion)?;
    let computed_at =
        normalize_sustainability_text(computed_at).ok_or(CarbonFootprintError::EmptyComputedAt)?;

    let inputs = normalize_carbon_inputs(request.inputs)?;
    let factors = normalize_carbon_factors(request.factor_set.factors)?;
    let required = [
        CarbonFootprintInputKind::DieselLiters,
        CarbonFootprintInputKind::FertilizerNitrogenKg,
        CarbonFootprintInputKind::ElectricityKwh,
        CarbonFootprintInputKind::FieldPasses,
    ];
    let has_required_inputs = required
        .iter()
        .all(|kind| inputs.iter().any(|input| input.kind == *kind));
    let has_required_factors = required
        .iter()
        .all(|kind| factors.iter().any(|factor| factor.input_kind == *kind));
    let evidence_refs = carbon_footprint_evidence_refs(&inputs, &factors, &factor_set_version);

    let value_co2e = if has_required_inputs && has_required_factors {
        Some(
            inputs
                .iter()
                .map(|input| {
                    let factor = factors
                        .iter()
                        .find(|factor| factor.input_kind == input.kind)
                        .expect("required factor checked");
                    input.quantity * factor.factor_kg_co2e_per_unit
                })
                .sum::<f64>(),
        )
    } else {
        None
    };
    let status = if value_co2e.is_some() {
        CarbonFootprintStatus::Computed
    } else {
        CarbonFootprintStatus::InsufficientInputs
    };
    let result_hash = carbon_footprint_hash(
        &record_id,
        &operation_id,
        value_co2e,
        &inputs,
        &factor_set_version,
        &factors,
        status,
    );

    Ok(CarbonFootprintResult {
        footprint_id,
        record_id,
        operation_id,
        value_co2e,
        inputs,
        factor_set_version,
        factors,
        evidence_refs,
        status,
        result_hash,
        computed_at,
    })
}

pub fn estimate_biomass(
    request: BiomassEstimateRequest,
    generated_estimate_id: String,
    computed_at: String,
) -> Result<BiomassEstimateResult, BiomassEstimateError> {
    let estimate_id = normalize_sustainability_optional_text(request.estimate_id)
        .or_else(|| normalize_sustainability_text(generated_estimate_id))
        .ok_or(BiomassEstimateError::EmptyEstimateId)?;
    let record_id = normalize_sustainability_text(request.record_id)
        .ok_or(BiomassEstimateError::EmptyRecordId)?;
    let method_version = normalize_sustainability_text(request.method_version)
        .ok_or(BiomassEstimateError::EmptyMethodVersion)?;
    let computed_at =
        normalize_sustainability_text(computed_at).ok_or(BiomassEstimateError::EmptyComputedAt)?;
    if !(request.biomass_coefficient.is_finite() && request.biomass_coefficient > 0.0) {
        return Err(BiomassEstimateError::InvalidCoefficient);
    }

    let canopy = validate_biomass_layer(request.canopy_layer)?;
    let index = validate_biomass_layer(request.vegetation_index_layer)?;
    if canopy.width != index.width || canopy.height != index.height {
        return Err(BiomassEstimateError::DimensionMismatch);
    }
    let canopy_crs = canopy
        .spatial_ref
        .crs
        .clone()
        .ok_or(RasterSpatialRefError::MissingCrs)?;
    let index_crs = index
        .spatial_ref
        .crs
        .clone()
        .ok_or(RasterSpatialRefError::MissingCrs)?;
    if canopy_crs != index_crs {
        return Err(BiomassEstimateError::CrsMismatch {
            canopy_crs,
            index_crs,
        });
    }
    let extent = canopy
        .spatial_ref
        .bbox
        .clone()
        .ok_or(RasterSpatialRefError::MissingBbox)?;
    let index_extent = index
        .spatial_ref
        .bbox
        .clone()
        .ok_or(RasterSpatialRefError::MissingBbox)?;
    if !biomass_bounds_equal(&extent, &index_extent) {
        return Err(BiomassEstimateError::ExtentMismatch);
    }
    let resolution = canopy
        .spatial_ref
        .resolution
        .ok_or(RasterSpatialRefError::NonPositiveResolution)?;
    let index_resolution = index
        .spatial_ref
        .resolution
        .ok_or(RasterSpatialRefError::NonPositiveResolution)?;
    if !biomass_resolution_equal(resolution, index_resolution) {
        return Err(BiomassEstimateError::ResolutionMismatch);
    }

    let cell_area = resolution.x * resolution.y;
    let mut biomass_value = 0.0;
    let mut valid_cells = 0_u32;
    for (height_m, vegetation_index) in canopy.values.iter().zip(index.values.iter()) {
        if *height_m > 0.0 && *vegetation_index > 0.0 {
            biomass_value += height_m
                * vegetation_index.clamp(0.0, 1.0)
                * cell_area
                * request.biomass_coefficient;
            valid_cells += 1;
        }
    }
    let area = f64::from(valid_cells) * cell_area;
    let source_layer_refs = vec![canopy.layer_ref.clone(), index.layer_ref.clone()];
    let result_hash = biomass_estimate_hash(
        &record_id,
        biomass_value,
        area,
        &canopy_crs,
        &extent,
        resolution,
        &source_layer_refs,
        &method_version,
    );

    Ok(BiomassEstimateResult {
        estimate_id,
        record_id,
        biomass_value,
        area,
        crs: canopy_crs,
        extent,
        resolution,
        source_layer_refs,
        method_version,
        result_hash,
        computed_at,
    })
}

pub fn create_sustainability_baseline(
    request: SustainabilityBaselineCreateRequest,
    generated_baseline_id: String,
    created_at: String,
) -> Result<SustainabilityBaselineRecord, SustainabilityBaselineError> {
    let baseline_id = normalize_sustainability_optional_text(request.baseline_id)
        .or_else(|| normalize_sustainability_text(generated_baseline_id))
        .ok_or(SustainabilityBaselineError::EmptyBaselineId)?;
    let field_id = normalize_sustainability_text(request.field_id)
        .ok_or(SustainabilityBaselineError::EmptyFieldId)?;
    let season_id = normalize_sustainability_text(request.season_id)
        .ok_or(SustainabilityBaselineError::EmptyBaselineSeasonId)?;
    let source_record_id = normalize_sustainability_text(request.source_record_id)
        .ok_or(SustainabilityBaselineError::EmptySourceRecordId)?;
    let method_version = normalize_sustainability_text(request.method_version)
        .ok_or(SustainabilityBaselineError::EmptyMethodVersion)?;
    let created_at = normalize_sustainability_text(created_at)
        .ok_or(SustainabilityBaselineError::EmptyTimestamp)?;
    if !request.metric_value.is_finite() {
        return Err(SustainabilityBaselineError::InvalidMetricValue);
    }
    let evidence_refs = normalize_sustainability_refs(
        request.evidence_refs,
        [
            format!("baseline_source:{source_record_id}"),
            format!("baseline_season:{season_id}"),
        ],
    );

    Ok(SustainabilityBaselineRecord {
        baseline_id,
        field_id,
        season_id,
        metric_type: request.metric_type,
        metric_value: request.metric_value,
        source_record_id,
        method_version,
        evidence_refs,
        created_at,
    })
}

pub fn compare_sustainability_baseline(
    baseline: Option<&SustainabilityBaselineRecord>,
    request: SustainabilityComparisonRequest,
    generated_comparison_id: String,
    compared_at: String,
) -> Result<SustainabilityComparisonResult, SustainabilityBaselineError> {
    let comparison_id = normalize_sustainability_optional_text(request.comparison_id)
        .or_else(|| normalize_sustainability_text(generated_comparison_id))
        .ok_or(SustainabilityBaselineError::EmptyComparisonId)?;
    let field_id = normalize_sustainability_text(request.field_id)
        .ok_or(SustainabilityBaselineError::EmptyFieldId)?;
    let baseline_season_id = normalize_sustainability_text(request.baseline_season_id)
        .ok_or(SustainabilityBaselineError::EmptyBaselineSeasonId)?;
    let current_season_id = normalize_sustainability_text(request.current_season_id)
        .ok_or(SustainabilityBaselineError::EmptyCurrentSeasonId)?;
    let current_source_record_id = normalize_sustainability_text(request.current_source_record_id)
        .ok_or(SustainabilityBaselineError::EmptySourceRecordId)?;
    let method_version = normalize_sustainability_text(request.method_version)
        .ok_or(SustainabilityBaselineError::EmptyMethodVersion)?;
    let compared_at = normalize_sustainability_text(compared_at)
        .ok_or(SustainabilityBaselineError::EmptyTimestamp)?;
    if !request.current_value.is_finite() {
        return Err(SustainabilityBaselineError::InvalidMetricValue);
    }

    let matched_baseline = baseline.filter(|baseline| {
        baseline.field_id == field_id
            && baseline.season_id == baseline_season_id
            && baseline.metric_type == request.metric_type
    });
    let baseline_value = matched_baseline.map(|baseline| baseline.metric_value);
    let delta = baseline_value.map(|baseline_value| request.current_value - baseline_value);
    let (status, trend) = match delta {
        Some(delta) if delta > 0.0 => (
            SustainabilityComparisonStatus::Compared,
            SustainabilityTrend::Increased,
        ),
        Some(delta) if delta < 0.0 => (
            SustainabilityComparisonStatus::Compared,
            SustainabilityTrend::Decreased,
        ),
        Some(_) => (
            SustainabilityComparisonStatus::Compared,
            SustainabilityTrend::Unchanged,
        ),
        None => (
            SustainabilityComparisonStatus::NoBaseline,
            SustainabilityTrend::NoBaseline,
        ),
    };
    let evidence_refs = sustainability_comparison_refs(
        matched_baseline,
        &baseline_season_id,
        &current_season_id,
        &current_source_record_id,
    );
    let baseline_source_record_id =
        matched_baseline.map(|baseline| baseline.source_record_id.clone());
    let result_hash = sustainability_comparison_hash(
        &field_id,
        &baseline_season_id,
        &current_season_id,
        request.metric_type,
        baseline_value,
        request.current_value,
        delta,
        status,
        trend,
        &baseline_source_record_id,
        &current_source_record_id,
        &method_version,
    );

    Ok(SustainabilityComparisonResult {
        comparison_id,
        field_id,
        baseline_season_id,
        current_season_id,
        metric_type: request.metric_type,
        baseline_value,
        current_value: request.current_value,
        delta,
        trend,
        status,
        baseline_source_record_id,
        current_source_record_id,
        evidence_refs,
        method_version,
        result_hash,
        compared_at,
    })
}

pub fn compute_sustainability_kpi(
    request: SustainabilityKpiTrackingRequest,
    generated_kpi_id: String,
    computed_at: String,
) -> Result<SustainabilityKpiTrackingResult, SustainabilityKpiError> {
    let kpi_id = normalize_sustainability_optional_text(request.kpi_id)
        .or_else(|| normalize_sustainability_text(generated_kpi_id))
        .ok_or(SustainabilityKpiError::EmptyKpiId)?;
    let field_id = normalize_sustainability_text(request.field_id)
        .ok_or(SustainabilityKpiError::EmptyFieldId)?;
    let season_id = normalize_sustainability_text(request.season_id)
        .ok_or(SustainabilityKpiError::EmptySeasonId)?;
    let metric_ref = normalize_sustainability_text(request.metric_ref)
        .ok_or(SustainabilityKpiError::EmptyMetricRef)?;
    let method_version = normalize_sustainability_text(request.method_version)
        .ok_or(SustainabilityKpiError::EmptyMethodVersion)?;
    let computed_at = normalize_sustainability_text(computed_at)
        .ok_or(SustainabilityKpiError::EmptyComputedAt)?;
    if !request.target_value.is_finite() {
        return Err(SustainabilityKpiError::InvalidTargetValue);
    }
    if !(request.at_risk_fraction.is_finite()
        && request.at_risk_fraction >= 0.0
        && request.at_risk_fraction <= 1.0)
    {
        return Err(SustainabilityKpiError::InvalidAtRiskFraction);
    }
    if request
        .current_value
        .is_some_and(|current_value| !current_value.is_finite())
    {
        return Err(SustainabilityKpiError::InvalidCurrentValue);
    }

    let evidence_refs = normalize_sustainability_refs(
        request.evidence_refs,
        [
            format!("metric:{metric_ref}"),
            format!("field:{field_id}"),
            format!("season:{season_id}"),
        ],
    );
    let status = sustainability_kpi_status(
        request.current_value,
        request.target_value,
        request.direction,
        request.at_risk_fraction,
    );
    let result_hash = sustainability_kpi_hash(
        &field_id,
        &season_id,
        &metric_ref,
        request.current_value,
        request.target_value,
        request.direction,
        request.at_risk_fraction,
        status,
        &evidence_refs,
        &method_version,
    );

    Ok(SustainabilityKpiTrackingResult {
        kpi_id,
        field_id,
        season_id,
        metric_ref,
        current_value: request.current_value,
        target_value: request.target_value,
        direction: request.direction,
        at_risk_fraction: request.at_risk_fraction,
        status,
        evidence_refs,
        method_version,
        result_hash,
        computed_at,
    })
}

pub fn create_sustainability_mrv_trail(
    request: SustainabilityMrvTrailCreateRequest,
    generated_trail_id: String,
    created_at: String,
) -> Result<SustainabilityMrvTrail, SustainabilityMrvTrailError> {
    let trail_id = normalize_sustainability_optional_text(request.trail_id)
        .or_else(|| normalize_sustainability_text(generated_trail_id))
        .ok_or(SustainabilityMrvTrailError::EmptyTrailId)?;
    let output_ref = normalize_sustainability_text(request.output_ref)
        .ok_or(SustainabilityMrvTrailError::EmptyOutputRef)?;
    let input_layer_refs = request
        .input_layer_refs
        .into_iter()
        .filter_map(normalize_sustainability_text)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if input_layer_refs.is_empty() {
        return Err(SustainabilityMrvTrailError::EmptyInputLayerRefs);
    }
    let method = normalize_sustainability_text(request.method)
        .ok_or(SustainabilityMrvTrailError::EmptyMethod)?;
    let method_version = normalize_sustainability_text(request.method_version)
        .ok_or(SustainabilityMrvTrailError::EmptyMethodVersion)?;
    let crs = normalize_sustainability_optional_text(request.crs)
        .ok_or(SustainabilityMrvTrailError::EmptyCrs)?;
    let extent = request
        .extent
        .ok_or(SustainabilityMrvTrailError::MissingExtent)?;
    if !sustainability_extent_is_valid(&extent) {
        return Err(SustainabilityMrvTrailError::InvalidExtent);
    }
    let audit_id = normalize_sustainability_text(request.audit_id)
        .ok_or(SustainabilityMrvTrailError::EmptyAuditId)?;
    let result_hash = normalize_sustainability_text(request.result_hash)
        .ok_or(SustainabilityMrvTrailError::EmptyResultHash)?;
    let created_at = normalize_sustainability_text(created_at)
        .ok_or(SustainabilityMrvTrailError::EmptyCreatedAt)?;
    let parameters = request
        .parameters
        .into_iter()
        .filter_map(|(key, value)| {
            normalize_sustainability_text(key).map(|key| {
                (
                    key,
                    normalize_sustainability_text(value)
                        .unwrap_or_else(|| "unspecified".to_string()),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let rederived_result_hash = sustainability_mrv_rederived_hash(
        &output_ref,
        request.output_kind,
        &input_layer_refs,
        &method,
        &method_version,
        &crs,
        &extent,
        &parameters,
        &audit_id,
        &result_hash,
    );

    Ok(SustainabilityMrvTrail {
        trail_id,
        output_ref,
        output_kind: request.output_kind,
        input_layer_refs,
        method,
        method_version,
        crs,
        extent,
        parameters,
        audit_id,
        result_hash,
        rederived_result_hash,
        certification_ready: true,
        created_at,
    })
}

pub fn build_sustainability_certification_evidence_pack(
    request: SustainabilityCertificationEvidencePackRequest,
    generated_pack_id: String,
    created_at: String,
) -> Result<SustainabilityCertificationEvidencePack, SustainabilityCertificationEvidencePackError> {
    let pack_id = normalize_sustainability_optional_text(request.pack_id)
        .or_else(|| normalize_sustainability_text(generated_pack_id))
        .ok_or(SustainabilityCertificationEvidencePackError::EmptyPackId)?;
    let claim_id = normalize_sustainability_text(request.claim_id)
        .ok_or(SustainabilityCertificationEvidencePackError::EmptyClaimId)?;
    let claim_type = normalize_sustainability_text(request.claim_type)
        .ok_or(SustainabilityCertificationEvidencePackError::EmptyClaimType)?;
    let field_id = normalize_sustainability_text(request.field_id)
        .ok_or(SustainabilityCertificationEvidencePackError::EmptyFieldId)?;
    let season_id = normalize_sustainability_text(request.season_id)
        .ok_or(SustainabilityCertificationEvidencePackError::EmptySeasonId)?;
    let method_version = normalize_sustainability_text(request.method_version)
        .ok_or(SustainabilityCertificationEvidencePackError::EmptyMethodVersion)?;
    let created_at = normalize_sustainability_text(created_at)
        .ok_or(SustainabilityCertificationEvidencePackError::EmptyCreatedAt)?;
    let claimed_output_refs = request
        .claimed_output_refs
        .into_iter()
        .filter_map(normalize_sustainability_text)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if claimed_output_refs.is_empty() {
        return Err(SustainabilityCertificationEvidencePackError::EmptyClaimedOutputRefs);
    }

    let outputs = normalize_sustainability_certification_outputs(request.outputs)?;
    let outputs_by_ref = outputs
        .iter()
        .map(|output| (output.output_ref.clone(), output))
        .collect::<BTreeMap<_, _>>();
    let trails_by_output = request
        .mrv_trails
        .iter()
        .map(|trail| (trail.output_ref.clone(), trail))
        .collect::<BTreeMap<_, _>>();

    for output_ref in &claimed_output_refs {
        let output = outputs_by_ref.get(output_ref).ok_or_else(|| {
            SustainabilityCertificationEvidencePackError::MissingClaimedOutput {
                output_ref: output_ref.clone(),
            }
        })?;
        let trail = trails_by_output.get(output_ref).ok_or_else(|| {
            SustainabilityCertificationEvidencePackError::MissingMrvTrail {
                output_ref: output_ref.clone(),
            }
        })?;
        if !trail.certification_ready
            || trail.input_layer_refs.is_empty()
            || trail.method.trim().is_empty()
            || trail.method_version.trim().is_empty()
            || trail.crs.trim().is_empty()
            || !sustainability_extent_is_valid(&trail.extent)
            || trail.audit_id.trim().is_empty()
            || trail.result_hash.trim().is_empty()
        {
            return Err(
                SustainabilityCertificationEvidencePackError::IncompleteMrvTrail {
                    output_ref: output_ref.clone(),
                },
            );
        }
        if trail.result_hash != output.result_hash {
            return Err(
                SustainabilityCertificationEvidencePackError::MrvResultHashMismatch {
                    output_ref: output_ref.clone(),
                },
            );
        }
    }

    let mut evidence_layer_refs = request
        .evidence_layer_refs
        .into_iter()
        .filter_map(normalize_sustainability_text)
        .collect::<BTreeSet<_>>();
    for output_ref in &claimed_output_refs {
        let trail = trails_by_output
            .get(output_ref)
            .expect("MRV trail presence checked");
        evidence_layer_refs.extend(trail.input_layer_refs.iter().cloned());
    }
    let evidence_layer_refs = evidence_layer_refs.into_iter().collect::<Vec<_>>();
    let mrv_trails = claimed_output_refs
        .iter()
        .map(|output_ref| {
            (*trails_by_output
                .get(output_ref)
                .expect("MRV trail presence checked"))
            .clone()
        })
        .collect::<Vec<_>>();
    let outputs = claimed_output_refs
        .iter()
        .map(|output_ref| {
            (*outputs_by_ref
                .get(output_ref)
                .expect("output presence checked"))
            .clone()
        })
        .collect::<Vec<_>>();
    let audit_ids = mrv_trails
        .iter()
        .map(|trail| trail.audit_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let result_hash = sustainability_certification_result_hash(&claim_id, &outputs, &mrv_trails);
    let pack_hash = sustainability_certification_pack_hash(
        &claim_id,
        &claim_type,
        &field_id,
        &season_id,
        &claimed_output_refs,
        &outputs,
        &evidence_layer_refs,
        &mrv_trails,
        &audit_ids,
        &result_hash,
        &method_version,
    );

    Ok(SustainabilityCertificationEvidencePack {
        pack_id,
        claim_id,
        claim_type,
        field_id,
        season_id,
        claimed_output_refs,
        outputs,
        evidence_layer_refs,
        mrv_trails,
        audit_ids,
        result_hash,
        pack_hash,
        method_version,
        created_at,
    })
}

pub fn compute_biodiversity_proxy(
    request: BiodiversityProxyRequest,
    generated_proxy_id: String,
    computed_at: String,
) -> Result<BiodiversityProxyResult, BiodiversityProxyError> {
    let proxy_id = normalize_sustainability_optional_text(request.proxy_id)
        .or_else(|| normalize_sustainability_text(generated_proxy_id))
        .ok_or(BiodiversityProxyError::EmptyProxyId)?;
    let field_id = normalize_sustainability_text(request.field_id)
        .ok_or(BiodiversityProxyError::EmptyFieldId)?;
    let method_version = normalize_sustainability_text(request.method_version)
        .ok_or(BiodiversityProxyError::EmptyMethodVersion)?;
    let computed_at = normalize_sustainability_text(computed_at)
        .ok_or(BiodiversityProxyError::EmptyComputedAt)?;
    if !(request.cover_threshold.is_finite() && (-1.0..=1.0).contains(&request.cover_threshold)) {
        return Err(BiodiversityProxyError::InvalidCoverThreshold);
    }
    let layer = validate_biodiversity_layer(request.layer)?;
    let crs = layer
        .spatial_ref
        .crs
        .clone()
        .ok_or(RasterSpatialRefError::MissingCrs)?;
    let extent = layer
        .spatial_ref
        .bbox
        .clone()
        .ok_or(RasterSpatialRefError::MissingBbox)?;
    let values = layer
        .values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    let status = if values.len() < 2 || biodiversity_uniform(&values) {
        BiodiversityProxyStatus::NoSignal
    } else {
        BiodiversityProxyStatus::Computed
    };
    let (heterogeneity_score, cover_fraction, uncertainty) =
        if status == BiodiversityProxyStatus::Computed {
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            let variance = values
                .iter()
                .map(|value| (value - mean).powi(2))
                .sum::<f64>()
                / values.len() as f64;
            let cover_fraction = values
                .iter()
                .filter(|value| **value >= request.cover_threshold)
                .count() as f64
                / values.len() as f64;
            let heterogeneity_score = (variance.sqrt() * cover_fraction).clamp(0.0, 1.0);
            let uncertainty = (1.0 - cover_fraction).clamp(0.0, 1.0);
            (Some(heterogeneity_score), Some(cover_fraction), uncertainty)
        } else {
            (None, None, 1.0)
        };
    let source_layer_refs = vec![layer.layer_ref.clone()];
    let result_hash = biodiversity_proxy_hash(
        &field_id,
        heterogeneity_score,
        cover_fraction,
        uncertainty,
        status,
        &crs,
        &extent,
        &source_layer_refs,
        &method_version,
    );

    Ok(BiodiversityProxyResult {
        proxy_id,
        field_id,
        heterogeneity_score,
        cover_fraction,
        uncertainty,
        status,
        crs,
        extent,
        source_layer_refs,
        method_version,
        result_hash,
        computed_at,
    })
}

pub fn compute_soil_carbon_proxy(
    request: SoilCarbonProxyRequest,
    generated_proxy_id: String,
    computed_at: String,
) -> Result<SoilCarbonProxyResult, SoilCarbonProxyError> {
    let proxy_id = normalize_sustainability_optional_text(request.proxy_id)
        .or_else(|| normalize_sustainability_text(generated_proxy_id))
        .ok_or(SoilCarbonProxyError::EmptyProxyId)?;
    let record_id = normalize_sustainability_text(request.record_id)
        .ok_or(SoilCarbonProxyError::EmptyRecordId)?;
    let field_id = normalize_sustainability_text(request.field_id)
        .ok_or(SoilCarbonProxyError::EmptyFieldId)?;
    let method_version = normalize_sustainability_text(request.method_version)
        .ok_or(SoilCarbonProxyError::EmptyMethodVersion)?;
    let computed_at =
        normalize_sustainability_text(computed_at).ok_or(SoilCarbonProxyError::EmptyComputedAt)?;
    let index_inputs = normalize_soil_carbon_evidence(request.index_inputs)?;
    let biomass_inputs = normalize_soil_carbon_evidence(request.biomass_inputs)?;
    let practice_inputs = normalize_soil_carbon_practices(request.practice_inputs)?;

    let mut evidence_refs = BTreeSet::new();
    evidence_refs.extend(index_inputs.iter().map(|input| input.evidence_ref.clone()));
    evidence_refs.extend(
        biomass_inputs
            .iter()
            .map(|input| input.evidence_ref.clone()),
    );
    evidence_refs.extend(
        practice_inputs
            .iter()
            .map(|input| input.practice_ref.clone()),
    );
    let evidence_refs = evidence_refs.into_iter().collect::<Vec<_>>();

    let has_sufficient_evidence =
        !index_inputs.is_empty() && !biomass_inputs.is_empty() && !practice_inputs.is_empty();
    let (proxy_value, uncertainty_band, status) = if has_sufficient_evidence {
        let index_component = weighted_average(&index_inputs);
        let biomass_component = weighted_average(&biomass_inputs);
        let practice_component = practice_inputs
            .iter()
            .map(|input| input.carbon_delta)
            .sum::<f64>()
            / practice_inputs.len() as f64;
        let proxy_value =
            (index_component * 0.35) + (biomass_component * 0.50) + practice_component;
        let evidence_count = index_inputs.len() + biomass_inputs.len() + practice_inputs.len();
        let half_width = (proxy_value.abs() * 0.20 + 0.5) / (evidence_count as f64).sqrt();
        (
            Some(proxy_value),
            Some(SoilCarbonUncertaintyBand {
                low: proxy_value - half_width,
                high: proxy_value + half_width,
            }),
            SoilCarbonProxyStatus::Computed,
        )
    } else {
        (None, None, SoilCarbonProxyStatus::InsufficientEvidence)
    };
    let result_hash = soil_carbon_proxy_hash(
        &record_id,
        &field_id,
        proxy_value,
        &uncertainty_band,
        status,
        &evidence_refs,
        &method_version,
    );

    Ok(SoilCarbonProxyResult {
        proxy_id,
        record_id,
        field_id,
        proxy_value,
        uncertainty_band,
        status,
        evidence_refs,
        method_version,
        result_hash,
        computed_at,
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

pub fn parse_carbon_footprint_input_kind(
    value: &str,
) -> Result<CarbonFootprintInputKind, CarbonFootprintError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "diesel_liters" => Ok(CarbonFootprintInputKind::DieselLiters),
        "fertilizer_nitrogen_kg" => Ok(CarbonFootprintInputKind::FertilizerNitrogenKg),
        "electricity_kwh" => Ok(CarbonFootprintInputKind::ElectricityKwh),
        "field_passes" => Ok(CarbonFootprintInputKind::FieldPasses),
        _ => Err(CarbonFootprintError::UnsupportedInputKind {
            value: value.to_string(),
        }),
    }
}

pub fn parse_carbon_footprint_status(
    value: &str,
) -> Result<CarbonFootprintStatus, CarbonFootprintError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "computed" => Ok(CarbonFootprintStatus::Computed),
        "insufficient_inputs" => Ok(CarbonFootprintStatus::InsufficientInputs),
        _ => Err(CarbonFootprintError::UnsupportedStatus {
            value: value.to_string(),
        }),
    }
}

pub fn parse_sustainability_comparison_status(
    value: &str,
) -> Result<SustainabilityComparisonStatus, SustainabilityBaselineError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "compared" => Ok(SustainabilityComparisonStatus::Compared),
        "no_baseline" => Ok(SustainabilityComparisonStatus::NoBaseline),
        _ => Err(SustainabilityBaselineError::UnsupportedComparisonStatus {
            value: value.to_string(),
        }),
    }
}

pub fn parse_sustainability_trend(
    value: &str,
) -> Result<SustainabilityTrend, SustainabilityBaselineError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "increased" => Ok(SustainabilityTrend::Increased),
        "decreased" => Ok(SustainabilityTrend::Decreased),
        "unchanged" => Ok(SustainabilityTrend::Unchanged),
        "no_baseline" => Ok(SustainabilityTrend::NoBaseline),
        _ => Err(SustainabilityBaselineError::UnsupportedTrend {
            value: value.to_string(),
        }),
    }
}

pub fn parse_sustainability_kpi_direction(
    value: &str,
) -> Result<SustainabilityKpiDirection, SustainabilityKpiError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "higher_is_better" => Ok(SustainabilityKpiDirection::HigherIsBetter),
        "lower_is_better" => Ok(SustainabilityKpiDirection::LowerIsBetter),
        _ => Err(SustainabilityKpiError::UnsupportedDirection {
            value: value.to_string(),
        }),
    }
}

pub fn parse_sustainability_kpi_status(
    value: &str,
) -> Result<SustainabilityKpiStatus, SustainabilityKpiError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "on_track" => Ok(SustainabilityKpiStatus::OnTrack),
        "at_risk" => Ok(SustainabilityKpiStatus::AtRisk),
        "met" => Ok(SustainabilityKpiStatus::Met),
        "no_data" => Ok(SustainabilityKpiStatus::NoData),
        _ => Err(SustainabilityKpiError::UnsupportedStatus {
            value: value.to_string(),
        }),
    }
}

pub fn parse_sustainability_mrv_output_kind(
    value: &str,
) -> Result<SustainabilityMrvOutputKind, SustainabilityMrvTrailError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "carbon_footprint" => Ok(SustainabilityMrvOutputKind::CarbonFootprint),
        "biomass_estimate" => Ok(SustainabilityMrvOutputKind::BiomassEstimate),
        "sustainability_kpi" => Ok(SustainabilityMrvOutputKind::SustainabilityKpi),
        _ => Err(SustainabilityMrvTrailError::UnsupportedOutputKind {
            value: value.to_string(),
        }),
    }
}

pub fn parse_biodiversity_proxy_status(
    value: &str,
) -> Result<BiodiversityProxyStatus, BiodiversityProxyError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "computed" => Ok(BiodiversityProxyStatus::Computed),
        "no_signal" => Ok(BiodiversityProxyStatus::NoSignal),
        _ => Err(BiodiversityProxyError::UnsupportedStatus {
            value: value.to_string(),
        }),
    }
}

pub fn parse_soil_carbon_proxy_status(
    value: &str,
) -> Result<SoilCarbonProxyStatus, SoilCarbonProxyError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "computed" => Ok(SoilCarbonProxyStatus::Computed),
        "insufficient_evidence" => Ok(SoilCarbonProxyStatus::InsufficientEvidence),
        _ => Err(SoilCarbonProxyError::UnsupportedStatus {
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

fn normalize_sustainability_refs<const N: usize>(
    refs: Vec<String>,
    fallback_refs: [String; N],
) -> Vec<String> {
    let mut normalized = refs
        .into_iter()
        .filter_map(normalize_sustainability_text)
        .collect::<BTreeSet<_>>();
    for fallback_ref in fallback_refs {
        if let Some(fallback_ref) = normalize_sustainability_text(fallback_ref) {
            normalized.insert(fallback_ref);
        }
    }
    normalized.into_iter().collect()
}

fn sustainability_comparison_refs(
    baseline: Option<&SustainabilityBaselineRecord>,
    baseline_season_id: &str,
    current_season_id: &str,
    current_source_record_id: &str,
) -> Vec<String> {
    let mut refs = BTreeSet::from([
        format!("baseline_season:{baseline_season_id}"),
        format!("current_season:{current_season_id}"),
        format!("current_source:{current_source_record_id}"),
    ]);
    if let Some(baseline) = baseline {
        refs.insert(format!("baseline:{}", baseline.baseline_id));
        refs.insert(format!("baseline_source:{}", baseline.source_record_id));
        refs.extend(baseline.evidence_refs.iter().cloned());
    }
    refs.into_iter().collect()
}

fn sustainability_comparison_hash(
    field_id: &str,
    baseline_season_id: &str,
    current_season_id: &str,
    metric_type: SustainabilityMetricType,
    baseline_value: Option<f64>,
    current_value: f64,
    delta: Option<f64>,
    status: SustainabilityComparisonStatus,
    trend: SustainabilityTrend,
    baseline_source_record_id: &Option<String>,
    current_source_record_id: &str,
    method_version: &str,
) -> String {
    let canonical = format!(
        "field={field_id}|baseline_season={baseline_season_id}|current_season={current_season_id}|metric={}|baseline={:.6}|current={current_value:.6}|delta={:.6}|status={}|trend={}|baseline_source={}|current_source={current_source_record_id}|method={method_version}",
        metric_type.as_str(),
        baseline_value.unwrap_or(-1.0),
        delta.unwrap_or(0.0),
        status.as_str(),
        trend.as_str(),
        baseline_source_record_id.as_deref().unwrap_or("none")
    );
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
}

fn sustainability_kpi_status(
    current_value: Option<f64>,
    target_value: f64,
    direction: SustainabilityKpiDirection,
    at_risk_fraction: f64,
) -> SustainabilityKpiStatus {
    let Some(current_value) = current_value else {
        return SustainabilityKpiStatus::NoData;
    };
    match direction {
        SustainabilityKpiDirection::HigherIsBetter if current_value >= target_value => {
            SustainabilityKpiStatus::Met
        }
        SustainabilityKpiDirection::HigherIsBetter
            if current_value >= target_value * at_risk_fraction =>
        {
            SustainabilityKpiStatus::OnTrack
        }
        SustainabilityKpiDirection::HigherIsBetter => SustainabilityKpiStatus::AtRisk,
        SustainabilityKpiDirection::LowerIsBetter if current_value <= target_value => {
            SustainabilityKpiStatus::Met
        }
        SustainabilityKpiDirection::LowerIsBetter
            if at_risk_fraction == 0.0 || current_value <= target_value / at_risk_fraction =>
        {
            SustainabilityKpiStatus::OnTrack
        }
        SustainabilityKpiDirection::LowerIsBetter => SustainabilityKpiStatus::AtRisk,
    }
}

fn default_sustainability_kpi_at_risk_fraction() -> f64 {
    0.9
}

fn sustainability_kpi_hash(
    field_id: &str,
    season_id: &str,
    metric_ref: &str,
    current_value: Option<f64>,
    target_value: f64,
    direction: SustainabilityKpiDirection,
    at_risk_fraction: f64,
    status: SustainabilityKpiStatus,
    evidence_refs: &[String],
    method_version: &str,
) -> String {
    let mut canonical = format!(
        "field={field_id}|season={season_id}|metric={metric_ref}|current={:.6}|target={target_value:.6}|direction={}|risk_fraction={at_risk_fraction:.6}|status={}|method={method_version}",
        current_value.unwrap_or(-1.0),
        direction.as_str(),
        status.as_str()
    );
    for evidence_ref in evidence_refs {
        canonical.push_str(&format!("|evidence:{evidence_ref}"));
    }
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
}

fn sustainability_extent_is_valid(extent: &GeoBounds) -> bool {
    extent.min_lon.is_finite()
        && extent.min_lat.is_finite()
        && extent.max_lon.is_finite()
        && extent.max_lat.is_finite()
        && extent.min_lon < extent.max_lon
        && extent.min_lat < extent.max_lat
}

fn sustainability_mrv_rederived_hash(
    output_ref: &str,
    output_kind: SustainabilityMrvOutputKind,
    input_layer_refs: &[String],
    method: &str,
    method_version: &str,
    crs: &str,
    extent: &GeoBounds,
    parameters: &BTreeMap<String, String>,
    audit_id: &str,
    result_hash: &str,
) -> String {
    let mut canonical = format!(
        "output={output_ref}|kind={}|method={method}|version={method_version}|crs={crs}|extent={:.9},{:.9},{:.9},{:.9}|audit={audit_id}|result={result_hash}",
        output_kind.as_str(),
        extent.min_lon,
        extent.min_lat,
        extent.max_lon,
        extent.max_lat
    );
    for input_layer_ref in input_layer_refs {
        canonical.push_str(&format!("|input:{input_layer_ref}"));
    }
    for (key, value) in parameters {
        canonical.push_str(&format!("|param:{key}={value}"));
    }
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
}

fn normalize_sustainability_certification_outputs(
    outputs: Vec<SustainabilityCertificationOutputItem>,
) -> Result<Vec<SustainabilityCertificationOutputItem>, SustainabilityCertificationEvidencePackError>
{
    outputs
        .into_iter()
        .map(|output| {
            let output_ref = normalize_sustainability_text(output.output_ref)
                .ok_or(SustainabilityCertificationEvidencePackError::EmptyOutputRef)?;
            let method_version =
                normalize_sustainability_text(output.method_version).ok_or_else(|| {
                    SustainabilityCertificationEvidencePackError::EmptyOutputMethodVersion {
                        output_ref: output_ref.clone(),
                    }
                })?;
            let result_hash =
                normalize_sustainability_text(output.result_hash).ok_or_else(|| {
                    SustainabilityCertificationEvidencePackError::EmptyOutputResultHash {
                        output_ref: output_ref.clone(),
                    }
                })?;
            if output.value.is_some_and(|value| !value.is_finite()) {
                return Err(
                    SustainabilityCertificationEvidencePackError::InvalidOutputValue { output_ref },
                );
            }
            Ok(SustainabilityCertificationOutputItem {
                output_ref,
                output_kind: output.output_kind,
                value: output.value,
                unit: normalize_sustainability_optional_text(output.unit),
                method_version,
                result_hash,
            })
        })
        .collect::<Result<Vec<_>, _>>()
}

fn sustainability_certification_result_hash(
    claim_id: &str,
    outputs: &[SustainabilityCertificationOutputItem],
    mrv_trails: &[SustainabilityMrvTrail],
) -> String {
    let mut canonical = format!("claim={claim_id}");
    for output in outputs {
        canonical.push_str(&format!(
            "|output:{}|kind:{}|value:{:.6}|unit:{}|method:{}|result:{}",
            output.output_ref,
            output.output_kind.as_str(),
            output.value.unwrap_or(-1.0),
            output.unit.as_deref().unwrap_or("none"),
            output.method_version,
            output.result_hash
        ));
    }
    for trail in mrv_trails {
        canonical.push_str(&format!(
            "|trail:{}|output:{}|mrv_result:{}|rederived:{}|audit:{}",
            trail.trail_id,
            trail.output_ref,
            trail.result_hash,
            trail.rederived_result_hash,
            trail.audit_id
        ));
    }
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
}

fn sustainability_certification_pack_hash(
    claim_id: &str,
    claim_type: &str,
    field_id: &str,
    season_id: &str,
    claimed_output_refs: &[String],
    outputs: &[SustainabilityCertificationOutputItem],
    evidence_layer_refs: &[String],
    mrv_trails: &[SustainabilityMrvTrail],
    audit_ids: &[String],
    result_hash: &str,
    method_version: &str,
) -> String {
    let mut canonical = format!(
        "claim={claim_id}|type={claim_type}|field={field_id}|season={season_id}|result={result_hash}|method={method_version}"
    );
    for output_ref in claimed_output_refs {
        canonical.push_str(&format!("|claimed:{output_ref}"));
    }
    for output in outputs {
        canonical.push_str(&format!(
            "|output:{}:{}:{}",
            output.output_ref,
            output.output_kind.as_str(),
            output.result_hash
        ));
    }
    for evidence_layer_ref in evidence_layer_refs {
        canonical.push_str(&format!("|layer:{evidence_layer_ref}"));
    }
    for trail in mrv_trails {
        canonical.push_str(&format!(
            "|mrv:{}:{}:{}:{}:{}",
            trail.trail_id,
            trail.output_ref,
            trail.method,
            trail.method_version,
            trail.rederived_result_hash
        ));
    }
    for audit_id in audit_ids {
        canonical.push_str(&format!("|audit:{audit_id}"));
    }
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
}

fn validate_biodiversity_layer(
    mut layer: BiodiversityImageryLayer,
) -> Result<BiodiversityImageryLayer, BiodiversityProxyError> {
    layer.layer_ref = normalize_sustainability_text(layer.layer_ref)
        .ok_or(BiodiversityProxyError::EmptyLayerRef)?;
    let expected_len = layer.width as usize * layer.height as usize;
    if expected_len != layer.values.len() {
        return Err(BiodiversityProxyError::GridSizeMismatch {
            layer_ref: layer.layer_ref,
        });
    }
    if layer.values.iter().any(|value| !value.is_finite()) {
        return Err(BiodiversityProxyError::InvalidGridValue {
            layer_ref: layer.layer_ref,
        });
    }
    layer.spatial_ref =
        assert_raster_spatial_ref(Some(&layer.spatial_ref), layer.width, layer.height)?;
    Ok(layer)
}

fn biodiversity_uniform(values: &[f64]) -> bool {
    let first = values.first().copied().unwrap_or_default();
    values
        .iter()
        .all(|value| (*value - first).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE)
}

fn default_biodiversity_cover_threshold() -> f64 {
    0.3
}

fn biodiversity_proxy_hash(
    field_id: &str,
    heterogeneity_score: Option<f64>,
    cover_fraction: Option<f64>,
    uncertainty: f64,
    status: BiodiversityProxyStatus,
    crs: &str,
    extent: &GeoBounds,
    source_layer_refs: &[String],
    method_version: &str,
) -> String {
    let mut canonical = format!(
        "field={field_id}|heterogeneity={:.6}|cover={:.6}|uncertainty={uncertainty:.6}|status={}|crs={crs}|extent={:.9},{:.9},{:.9},{:.9}|method={method_version}",
        heterogeneity_score.unwrap_or(-1.0),
        cover_fraction.unwrap_or(-1.0),
        status.as_str(),
        extent.min_lon,
        extent.min_lat,
        extent.max_lon,
        extent.max_lat
    );
    for source_layer_ref in source_layer_refs {
        canonical.push_str(&format!("|source:{source_layer_ref}"));
    }
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
}

fn normalize_soil_carbon_evidence(
    inputs: Vec<SoilCarbonEvidenceInput>,
) -> Result<Vec<SoilCarbonEvidenceInput>, SoilCarbonProxyError> {
    inputs
        .into_iter()
        .map(|input| {
            let evidence_ref = normalize_sustainability_text(input.evidence_ref)
                .ok_or(SoilCarbonProxyError::EmptyEvidenceRef)?;
            if !input.value.is_finite() {
                return Err(SoilCarbonProxyError::InvalidEvidenceValue);
            }
            if !(input.weight.is_finite() && input.weight > 0.0) {
                return Err(SoilCarbonProxyError::InvalidEvidenceWeight);
            }
            Ok(SoilCarbonEvidenceInput {
                evidence_ref,
                value: input.value,
                weight: input.weight,
            })
        })
        .collect()
}

fn normalize_soil_carbon_practices(
    inputs: Vec<SoilCarbonPracticeInput>,
) -> Result<Vec<SoilCarbonPracticeInput>, SoilCarbonProxyError> {
    inputs
        .into_iter()
        .map(|input| {
            let practice_ref = normalize_sustainability_text(input.practice_ref)
                .ok_or(SoilCarbonProxyError::EmptyPracticeRef)?;
            if !input.carbon_delta.is_finite() {
                return Err(SoilCarbonProxyError::InvalidPracticeDelta);
            }
            Ok(SoilCarbonPracticeInput {
                practice_ref,
                carbon_delta: input.carbon_delta,
            })
        })
        .collect()
}

fn weighted_average(inputs: &[SoilCarbonEvidenceInput]) -> f64 {
    let weighted_sum = inputs
        .iter()
        .map(|input| input.value * input.weight)
        .sum::<f64>();
    let weight_sum = inputs.iter().map(|input| input.weight).sum::<f64>();
    weighted_sum / weight_sum
}

fn default_soil_carbon_weight() -> f64 {
    1.0
}

fn soil_carbon_proxy_hash(
    record_id: &str,
    field_id: &str,
    proxy_value: Option<f64>,
    uncertainty_band: &Option<SoilCarbonUncertaintyBand>,
    status: SoilCarbonProxyStatus,
    evidence_refs: &[String],
    method_version: &str,
) -> String {
    let mut canonical = format!(
        "record={record_id}|field={field_id}|proxy={:.6}|low={:.6}|high={:.6}|status={}|method={method_version}",
        proxy_value.unwrap_or(-1.0),
        uncertainty_band.as_ref().map(|band| band.low).unwrap_or(-1.0),
        uncertainty_band
            .as_ref()
            .map(|band| band.high)
            .unwrap_or(-1.0),
        status.as_str()
    );
    for evidence_ref in evidence_refs {
        canonical.push_str(&format!("|evidence:{evidence_ref}"));
    }
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
}

fn validate_biomass_layer(
    mut layer: BiomassLayerInput,
) -> Result<BiomassLayerInput, BiomassEstimateError> {
    layer.layer_ref = normalize_sustainability_text(layer.layer_ref)
        .ok_or(BiomassEstimateError::EmptyLayerRef)?;
    let expected_len = layer.width as usize * layer.height as usize;
    if expected_len != layer.values.len() {
        return Err(BiomassEstimateError::GridSizeMismatch {
            layer_ref: layer.layer_ref,
        });
    }
    if layer.values.iter().any(|value| !value.is_finite()) {
        return Err(BiomassEstimateError::InvalidGridValue {
            layer_ref: layer.layer_ref,
        });
    }
    layer.spatial_ref =
        assert_raster_spatial_ref(Some(&layer.spatial_ref), layer.width, layer.height)?;
    Ok(layer)
}

fn biomass_bounds_equal(left: &GeoBounds, right: &GeoBounds) -> bool {
    (left.min_lon - right.min_lon).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.min_lat - right.min_lat).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.max_lon - right.max_lon).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.max_lat - right.max_lat).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
}

fn biomass_resolution_equal(left: RasterResolution, right: RasterResolution) -> bool {
    (left.x - right.x).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.y - right.y).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
}

fn default_biomass_coefficient() -> f64 {
    0.48
}

fn biomass_estimate_hash(
    record_id: &str,
    biomass_value: f64,
    area: f64,
    crs: &str,
    extent: &GeoBounds,
    resolution: RasterResolution,
    source_layer_refs: &[String],
    method_version: &str,
) -> String {
    let mut canonical = format!(
        "record={record_id}|biomass={biomass_value:.6}|area={area:.6}|crs={crs}|extent={:.9},{:.9},{:.9},{:.9}|resolution={:.6},{:.6}|method={method_version}",
        extent.min_lon,
        extent.min_lat,
        extent.max_lon,
        extent.max_lat,
        resolution.x,
        resolution.y
    );
    for source_layer_ref in source_layer_refs {
        canonical.push_str(&format!("|source:{source_layer_ref}"));
    }
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
}

fn normalize_carbon_inputs(
    inputs: Vec<CarbonFootprintInput>,
) -> Result<Vec<CarbonFootprintInput>, CarbonFootprintError> {
    let mut normalized = Vec::new();
    for input in inputs {
        if !(input.quantity.is_finite() && input.quantity >= 0.0) {
            return Err(CarbonFootprintError::InvalidInputQuantity);
        }
        normalized.push(CarbonFootprintInput {
            kind: input.kind,
            quantity: input.quantity,
            unit: normalize_sustainability_text(input.unit).unwrap_or_else(|| "unit".to_string()),
            evidence_ref: normalize_sustainability_text(input.evidence_ref)
                .unwrap_or_else(|| format!("operation_input:{}", input.kind.as_str())),
        });
    }
    normalized.sort_by_key(|input| input.kind);
    Ok(normalized)
}

fn normalize_carbon_factors(
    factors: Vec<CarbonEmissionFactor>,
) -> Result<Vec<CarbonEmissionFactor>, CarbonFootprintError> {
    let mut normalized = Vec::new();
    for factor in factors {
        if !(factor.factor_kg_co2e_per_unit.is_finite() && factor.factor_kg_co2e_per_unit >= 0.0) {
            return Err(CarbonFootprintError::InvalidEmissionFactor);
        }
        normalized.push(CarbonEmissionFactor {
            input_kind: factor.input_kind,
            factor_kg_co2e_per_unit: factor.factor_kg_co2e_per_unit,
            factor_ref: normalize_sustainability_text(factor.factor_ref)
                .unwrap_or_else(|| format!("factor:{}", factor.input_kind.as_str())),
        });
    }
    normalized.sort_by_key(|factor| factor.input_kind);
    Ok(normalized)
}

fn carbon_footprint_evidence_refs(
    inputs: &[CarbonFootprintInput],
    factors: &[CarbonEmissionFactor],
    factor_set_version: &str,
) -> Vec<String> {
    let mut refs = BTreeSet::from([format!("factor_set:{factor_set_version}")]);
    for input in inputs {
        refs.insert(input.evidence_ref.clone());
    }
    for factor in factors {
        refs.insert(factor.factor_ref.clone());
    }
    refs.into_iter().collect()
}

fn carbon_footprint_hash(
    record_id: &str,
    operation_id: &str,
    value_co2e: Option<f64>,
    inputs: &[CarbonFootprintInput],
    factor_set_version: &str,
    factors: &[CarbonEmissionFactor],
    status: CarbonFootprintStatus,
) -> String {
    let mut canonical = format!(
        "record={record_id}|operation={operation_id}|status={}|value={:.6}|factor_set={factor_set_version}",
        status.as_str(),
        value_co2e.unwrap_or(-1.0)
    );
    for input in inputs {
        canonical.push_str(&format!(
            "|input:{}:{:.6}:{}:{}",
            input.kind.as_str(),
            input.quantity,
            input.unit,
            input.evidence_ref
        ));
    }
    for factor in factors {
        canonical.push_str(&format!(
            "|factor:{}:{:.6}:{}",
            factor.input_kind.as_str(),
            factor.factor_kg_co2e_per_unit,
            factor.factor_ref
        ));
    }
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
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

    pub fn suggest_next_season_rollover(
        &self,
        org_id: &str,
        field_id: &str,
    ) -> Result<SeasonCropPlanRolloverSuggestion, FarmFieldError> {
        let org_id =
            normalize_farm_field_text(org_id.to_string()).ok_or(FarmFieldError::EmptyOrgId)?;
        let field_id =
            normalize_farm_field_text(field_id.to_string()).ok_or(FarmFieldError::EmptyFieldId)?;
        self.fields
            .get(&field_id)
            .filter(|field| field.org_id == org_id)
            .ok_or_else(|| FarmFieldError::FieldNotFound {
                field_id: field_id.clone(),
            })?;

        let history = self.season_history_for_field(&org_id, &field_id);
        let Some(latest) = history.last() else {
            return Ok(SeasonCropPlanRolloverSuggestion {
                field_id,
                org_id,
                source_history_refs: Vec::new(),
                requires_approval: true,
                proposed_season: None,
                proposed_crop_plan: None,
                no_basis_reason: Some("no persisted season history for field".to_string()),
            });
        };

        let next_start = add_one_calendar_year(&parse_farm_field_date(&latest.season.start)?);
        let next_end = add_one_calendar_year(&parse_farm_field_date(&latest.season.end)?);
        let next_year = next_start.year();
        let latest_crop_plan = latest.crop_plans.first().cloned();
        let proposed_crop = latest_crop_plan
            .as_ref()
            .map(|crop_plan| crop_plan.crop.clone());
        let proposed_season = SeasonRecord {
            season_id: format!("season-{field_id}-{next_year}"),
            field_id: field_id.clone(),
            org_id: org_id.clone(),
            start: next_start.format("%Y-%m-%d").to_string(),
            end: next_end.format("%Y-%m-%d").to_string(),
            label: proposed_crop
                .as_ref()
                .map(|crop| format!("{next_year} {crop}"))
                .unwrap_or_else(|| format!("{next_year} rollover from {}", latest.season.label)),
        };
        let proposed_crop_plan = if let Some(crop_plan) = latest_crop_plan {
            let planting_date = crop_plan
                .planting_date
                .as_deref()
                .map(parse_farm_field_date)
                .transpose()?
                .map(|date| add_one_calendar_year(&date).format("%Y-%m-%d").to_string());
            Some(CropPlanRecord {
                crop_plan_id: format!("plan-{field_id}-{next_year}"),
                season_id: proposed_season.season_id.clone(),
                org_id: org_id.clone(),
                crop: crop_plan.crop,
                planting_date,
            })
        } else {
            None
        };
        let mut source_history_refs = vec![format!("season:{}", latest.season.season_id)];
        source_history_refs.extend(
            latest
                .crop_plans
                .iter()
                .map(|crop_plan| format!("crop_plan:{}", crop_plan.crop_plan_id)),
        );

        Ok(SeasonCropPlanRolloverSuggestion {
            field_id,
            org_id,
            source_history_refs,
            requires_approval: true,
            proposed_season: Some(proposed_season),
            proposed_crop_plan,
            no_basis_reason: None,
        })
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

fn add_one_calendar_year(date: &NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year() + 1, date.month(), date.day())
        .or_else(|| NaiveDate::from_ymd_opt(date.year() + 1, date.month(), 28))
        .expect("valid rollover date")
}

pub fn evaluate_access_anomaly_advisories(
    events: &[AccessAuditEvent],
    thresholds: AccessAnomalyThresholds,
) -> Vec<AccessAnomalyAdvisory> {
    let mut cross_org_by_actor: BTreeMap<String, Vec<&AccessAuditEvent>> = BTreeMap::new();
    let mut exports_by_actor: BTreeMap<String, Vec<&AccessAuditEvent>> = BTreeMap::new();

    for event in events {
        if event.actor_id.trim().is_empty() {
            continue;
        }
        if is_denied_cross_org_attempt(event) {
            cross_org_by_actor
                .entry(event.actor_id.clone())
                .or_default()
                .push(event);
        }
        if is_allowed_export_access(event) {
            exports_by_actor
                .entry(event.actor_id.clone())
                .or_default()
                .push(event);
        }
    }

    let mut advisories = Vec::new();
    append_access_anomaly_advisories(
        &mut advisories,
        cross_org_by_actor,
        AccessAnomalySignal::CrossOrgProbe,
        thresholds.denied_cross_org_attempts,
    );
    append_access_anomaly_advisories(
        &mut advisories,
        exports_by_actor,
        AccessAnomalySignal::BulkExport,
        thresholds.bulk_export_count,
    );
    advisories
}

fn append_access_anomaly_advisories(
    advisories: &mut Vec<AccessAnomalyAdvisory>,
    grouped_events: BTreeMap<String, Vec<&AccessAuditEvent>>,
    signal: AccessAnomalySignal,
    threshold: usize,
) {
    if threshold == 0 {
        return;
    }
    for (actor_id, events) in grouped_events {
        if events.len() < threshold {
            continue;
        }
        advisories.push(AccessAnomalyAdvisory {
            actor_id: actor_id.clone(),
            signal,
            observed_count: events.len(),
            threshold,
            evidence_audit_ids: events
                .iter()
                .map(|event| event.audit_id.clone())
                .collect::<Vec<_>>(),
            requires_approval: true,
            auto_blocked: false,
            summary: match signal {
                AccessAnomalySignal::CrossOrgProbe => format!(
                    "actor {actor_id} has {} denied cross-org access attempts",
                    events.len()
                ),
                AccessAnomalySignal::BulkExport => {
                    format!("actor {actor_id} has {} export access events", events.len())
                }
            },
        });
    }
}

fn is_denied_cross_org_attempt(event: &AccessAuditEvent) -> bool {
    event.decision == AccessAuditDecision::Denied
        && (event
            .target_org_id
            .as_ref()
            .is_some_and(|target_org_id| target_org_id != &event.org_id)
            || event.reason_code.as_ref().is_some_and(|reason| {
                let reason = reason.to_ascii_lowercase();
                reason.contains("cross_org") || reason.contains("cross-tenant")
            }))
}

fn is_allowed_export_access(event: &AccessAuditEvent) -> bool {
    event.decision == AccessAuditDecision::Allowed
        && event.action.to_ascii_lowercase().contains("export")
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenDataPublishRequest {
    pub source_layer_ref: String,
    pub license: String,
    pub attribution: String,
    #[serde(default)]
    pub owner_identifier: Option<String>,
    #[serde(default)]
    pub field_identifier: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenDataPublication {
    pub open_data_id: String,
    pub source_layer_ref: String,
    pub license: String,
    pub attribution: String,
    pub anonymized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenDataPublishRefusalReason {
    MissingLicense,
    MissingAttribution,
    OwnerIdentifierPresent,
    FieldIdentifierPresent,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpenDataPublishError {
    #[error("open-data publication refused: {reason:?}")]
    Refused {
        reason: OpenDataPublishRefusalReason,
    },
}

pub fn prepare_open_data_publication(
    request: OpenDataPublishRequest,
    generated_open_data_id: String,
) -> Result<OpenDataPublication, OpenDataPublishError> {
    let license = request.license.trim().to_string();
    if license.is_empty() {
        return Err(OpenDataPublishError::Refused {
            reason: OpenDataPublishRefusalReason::MissingLicense,
        });
    }
    let attribution = request.attribution.trim().to_string();
    if attribution.is_empty() {
        return Err(OpenDataPublishError::Refused {
            reason: OpenDataPublishRefusalReason::MissingAttribution,
        });
    }
    if request
        .owner_identifier
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(OpenDataPublishError::Refused {
            reason: OpenDataPublishRefusalReason::OwnerIdentifierPresent,
        });
    }
    if request
        .field_identifier
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(OpenDataPublishError::Refused {
            reason: OpenDataPublishRefusalReason::FieldIdentifierPresent,
        });
    }

    Ok(OpenDataPublication {
        open_data_id: generated_open_data_id.trim().to_string(),
        source_layer_ref: request.source_layer_ref.trim().to_string(),
        license,
        attribution,
        anonymized: true,
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
    use std::collections::BTreeMap;

    use super::{
        advise_weather_operational_windows, aggregate_marketplace_ratings,
        annotate_weather_record_freshness, append_content_version, append_irrigation_history_event,
        append_weather_history_records, apply_dry_run_validated_fleet_config_bundle,
        apply_fleet_node_heartbeat, apply_tractor_implement_command, assemble_drought_report,
        assemble_marketplace_org_report, assert_flight_operation_allowed,
        assert_raster_spatial_ref, bind_fleet_node_identity, bounds_from_points,
        build_collaboration_channel, build_collaboration_message, build_fleet_version_inventory,
        build_marketplace_account_record, build_marketplace_inventory_record,
        build_marketplace_portal_entry, build_soil_moisture_reading,
        build_sustainability_certification_evidence_pack, build_sustainability_record,
        build_tractor_field_ops_replay, build_tractor_field_ops_session_log,
        close_marketplace_listing_record, compare_sustainability_baseline,
        compute_biodiversity_proxy, compute_carbon_footprint, compute_drought_baseline_trend,
        compute_drought_index, compute_drought_risk_score, compute_marketplace_demand_forecast,
        compute_soil_carbon_proxy, compute_sustainability_kpi, compute_water_evapotranspiration,
        compute_weather_growing_degree_day, compute_weather_reference_et,
        create_marketplace_fulfillment_record, create_marketplace_rating_record,
        create_sustainability_baseline, create_sustainability_mrv_trail, create_versioned_content,
        deconflict_tractor_swath_reservations, derive_drought_mitigation_recommendation,
        detect_tractor_obstacle, dry_run_fleet_config_bundle, dry_run_irrigation_valve_plan,
        estimate_biomass, evaluate_access_anomaly_advisories, evaluate_and_route_drought_alerts,
        evaluate_and_route_water_alerts, evaluate_crop_stage_weather_risks,
        evaluate_drought_advisory_loop, evaluate_tractor_geofence, evaluate_tractor_motion_gate,
        evaluate_tractor_weather_window_gate, evaluate_weather_risk_alerts,
        evaluate_weather_value_freshness, execute_irrigation_valve_plan,
        execute_tractor_prescription, forecast_drought_risk, fulfill_marketplace_inventory,
        fuse_drought_evidence, ingest_drought_stress_evidence,
        ingest_remote_sensing_moisture_proxy_layer, ingest_weather_sensor_stream,
        map_zone_water_need, marketplace_inventory_available, normalize_weather_provider_forecast,
        place_marketplace_order_record, plan_tractor_swath_coverage,
        publish_marketplace_listing_record, query_drought_history, query_irrigation_history,
        query_weather_history, release_marketplace_inventory, report_water_use_savings,
        reserve_marketplace_inventory, resolve_weather_forecast_to_field, route_weather_alert,
        run_tractor_straight_path_guidance, schedule_irrigation_plan, sign_fleet_config_bundle,
        soil_moisture_rejection_record, tractor_cross_track_error_m,
        transition_marketplace_account_status, transition_marketplace_fulfillment_status,
        transition_marketplace_order_status, validate_field_boundary,
        validate_water_weather_input_contract, verify_and_apply_fleet_config_bundle,
        verify_weather_forecast_accuracy, weather_fetch_failure_record,
        zone_water_need_insufficient, AccessAnomalySignal, AccessAnomalyThresholds,
        AccessAuditDecision, AccessAuditEvent, AnnotationAuditRegistry, AnnotationChangeType,
        AnnotationGeometry, AnnotationPersistenceError, AnnotationRecord, AuditedAnnotationRecord,
        BiodiversityImageryLayer, BiodiversityProxyRequest, BiodiversityProxyStatus,
        BiomassEstimateError, BiomassEstimateRequest, BiomassLayerInput, CarbonEmissionFactor,
        CarbonFootprintComputeRequest, CarbonFootprintFactorSet, CarbonFootprintInput,
        CarbonFootprintInputKind, CarbonFootprintStatus, CollaborationChannelCreateRequest,
        CollaborationError, CollaborationMessageCreateRequest, ContentCreateRequest, ContentError,
        ContentStatus, ContentType, CropPlanRecord, DroughtAdvisoryLoopRequest,
        DroughtAdvisoryLoopStatus, DroughtAlertRoutingRequest, DroughtBaselineTrendError,
        DroughtBaselineTrendRequest, DroughtBaselineTrendStatus, DroughtEvidenceFusionError,
        DroughtEvidenceFusionRequest, DroughtEvidenceFusionStatus, DroughtEvidenceInputStatus,
        DroughtForecastRequest, DroughtForecastStatus, DroughtForecastUncertaintyBand,
        DroughtHistoryEntry, DroughtHistoryEntryKind, DroughtHistoryError, DroughtHistoryQuery,
        DroughtIndexComputeRequest, DroughtIndexError, DroughtIndexPeriod, DroughtIndexType,
        DroughtMitigationActionTarget, DroughtMitigationError, DroughtMitigationRequest,
        DroughtMitigationStatus, DroughtReportError, DroughtReportRequest,
        DroughtReportSectionKind, DroughtRiskBand, DroughtRiskScoreError, DroughtRiskScoreRequest,
        DroughtRiskScoreStatus, DroughtRiskThresholds, DroughtStressEvidenceError,
        DroughtStressEvidenceLayer, DroughtStressIndex, DroughtTrendDirection,
        FarmFieldEntityStatus, FarmFieldError, FarmFieldListQuery, FarmFieldRegistry, FarmRecord,
        FieldBoundary, FieldBoundaryValidationError, FieldOperationalWindow, FieldRecord,
        FleetConfigApplyStatus, FleetConfigBundle, FleetConfigRejectionReason, FleetConfigState,
        FleetHeartbeatEvaluation, FleetInventoryFilter, FleetNodeComponentHealth,
        FleetNodeComponentStatus, FleetNodeEnrollmentError, FleetNodeEnrollmentRequest,
        FleetNodeHealthState, FleetNodeHeartbeat, FleetNodeKind, FleetNodeOperationError,
        FleetNodeRecord, FleetNodeRuntimeMode, FleetNodeStatus, GeoBounds, GeoPoint,
        IrrigationEventRecord, IrrigationEventRequest, IrrigationHistoryQuery,
        IrrigationScheduleRequest, IrrigationValveActionStatus, IrrigationValveDryRunRequest,
        IrrigationValveDryRunStatus, IrrigationValveExecuteRequest, IrrigationValveExecutionStatus,
        IrrigationValveSpec, MarketplaceAccountCreateRequest, MarketplaceAccountError,
        MarketplaceAccountRecord, MarketplaceAccountStatus, MarketplaceAvailabilityWindow,
        MarketplaceCatalogCategory, MarketplaceCatalogError, MarketplaceCatalogItemCreateRequest,
        MarketplaceCatalogItemKind, MarketplaceDemandForecastRequest,
        MarketplaceDemandForecastStatus, MarketplaceFulfillmentCreateRequest,
        MarketplaceFulfillmentError, MarketplaceFulfillmentStatus, MarketplaceInventoryError,
        MarketplaceInventoryUpsertRequest, MarketplaceListingError,
        MarketplaceListingPublishRequest, MarketplaceListingStatus, MarketplaceOrderCreateRequest,
        MarketplaceOrderError, MarketplaceOrderStatus, MarketplaceOrgReportRequest,
        MarketplacePartyType, MarketplacePortalEntryError, MarketplaceRatingCreateRequest,
        MarketplaceRatingError, MarketplaceReportPeriod, MarketplaceUnitOfMeasure,
        MultispectralImage, OpenDataPublishError, OpenDataPublishRefusalReason,
        OpenDataPublishRequest, RasterResolution, RasterSpatialRef, RasterSpatialRefError,
        RecommendationLifecycleRegistry, RecommendationPersistenceError, RecommendationPriority,
        RecommendationRecord, RecommendationStatus, RecommendationStatusChangeType,
        RemoteSensingMoistureIndex, RemoteSensingMoistureProxyError,
        RemoteSensingMoistureProxyLayer, RemoteSensingMoistureZoneValue, ReportDeliverableRegistry,
        ReportFormat, ReportPersistenceError, ReportRecord, ReportVisibility,
        SceneFieldCoverageStatus, SceneLayerMetadataError, SceneLayerRecord, SceneRecord,
        SeasonRecord, SoilCarbonEvidenceInput, SoilCarbonPracticeInput, SoilCarbonProxyRequest,
        SoilCarbonProxyStatus, SoilMoistureQaFlag, SoilMoistureReadingError,
        SoilMoistureReadingRequest, SoilMoistureRejectionReason,
        SustainabilityBaselineCreateRequest, SustainabilityBaselineRecord,
        SustainabilityCertificationEvidencePackError,
        SustainabilityCertificationEvidencePackRequest, SustainabilityCertificationOutputItem,
        SustainabilityComparisonRequest, SustainabilityComparisonStatus,
        SustainabilityKpiDirection, SustainabilityKpiStatus, SustainabilityKpiTrackingRequest,
        SustainabilityMetricType, SustainabilityMrvOutputKind, SustainabilityMrvTrailCreateRequest,
        SustainabilityMrvTrailError, SustainabilityRecordCreateRequest, SustainabilityRecordError,
        SustainabilityRecordLinkage, SustainabilityTrend, TractorCommandAuditDecision,
        TractorCommandRejectionReason, TractorDeconflictionDecision, TractorDeconflictionRequest,
        TractorEstopState, TractorFieldOpsReplayFrameType, TractorFieldOpsSafetyEvent,
        TractorFieldOpsSafetyEventType, TractorFieldOpsSessionRequest,
        TractorFieldOpsTelemetrySample, TractorGeofenceDecision, TractorGeofenceError,
        TractorGeofenceEvaluationRequest, TractorGuidanceConfig, TractorGuidanceError,
        TractorGuidanceFault, TractorGuidancePath, TractorGuidancePoint,
        TractorImplementAdapterSpec, TractorImplementCommand, TractorImplementDecision,
        TractorImplementRef, TractorImplementState, TractorLifecycleStatus,
        TractorMotionCommandRequest, TractorMotionGateDecision, TractorObstacleDetection,
        TractorObstacleDetectionRequest, TractorObstacleEvent, TractorOperatorApproval,
        TractorPrescriptionExecutionError, TractorPrescriptionExecutionRequest,
        TractorPrescriptionZone, TractorRegistrationRequest, TractorRegistry,
        TractorSwathCoverageRequest, TractorSwathReservation, TractorSwathSegment,
        TractorWeatherWindowDecision, TractorWeatherWindowGateRequest, WaterAlertRoutingRequest,
        WaterAlertThresholds, WaterAlertType, WaterEvapotranspirationRequest,
        WaterEvapotranspirationStatus, WaterNeedZone, WaterUseBaseline,
        WaterUseSavingsReportRequest, WaterUseSavingsStatus, WaterWeatherInputContractRequest,
        WaterWeatherInputStatus, WeatherAlertDeliveryStatus, WeatherAlertRouteTarget,
        WeatherAlertRoutingRequest, WeatherAlertRoutingTarget, WeatherCropStageRiskRequest,
        WeatherCropStageThresholdSet, WeatherFieldForecastResolutionError,
        WeatherFieldForecastResolutionRequest, WeatherForecastRecord, WeatherForecastValue,
        WeatherForecastVerificationRequest, WeatherForecastVerificationStatus,
        WeatherFreshnessState, WeatherGrowingDegreeDayRequest, WeatherGrowingDegreeDayStatus,
        WeatherHistoryQuery, WeatherIngestError, WeatherOperationalWindowRequest,
        WeatherOperationalWindowThresholds, WeatherProviderForecastPoint,
        WeatherProviderForecastResponse, WeatherReferenceEtInput, WeatherReferenceEtStatus,
        WeatherRiskThresholds, WeatherRiskType, WeatherSensorIngestError, WeatherSensorSample,
        WeatherSensorStreamIngestRequest, WorkOrderChangeType, WorkOrderCreateRequest,
        WorkOrderPersistenceError, WorkOrderQueueQuery, WorkOrderRecord, WorkOrderRegistry,
        WorkOrderStatus, ZoneWaterNeed, ZoneWaterNeedError, ZoneWaterNeedRequest,
        ZoneWaterNeedStatus,
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
    fn tractor_cross_track_error_math_is_signed_and_deterministic() {
        let path = TractorGuidancePath {
            start: TractorGuidancePoint { x_m: 0.0, y_m: 0.0 },
            end: TractorGuidancePoint {
                x_m: 10.0,
                y_m: 0.0,
            },
        };

        let left_error =
            tractor_cross_track_error_m(path, TractorGuidancePoint { x_m: 5.0, y_m: 3.0 })
                .expect("straight path has a valid cross-track error");
        let right_error = tractor_cross_track_error_m(
            path,
            TractorGuidancePoint {
                x_m: 5.0,
                y_m: -2.0,
            },
        )
        .expect("straight path has a valid cross-track error");

        assert_eq!(left_error, -3.0);
        assert_eq!(right_error, 2.0);
    }

    #[test]
    fn tractor_guidance_simulation_keeps_error_within_bound() {
        let result = run_tractor_straight_path_guidance(
            tractor_guidance_test_path(),
            TractorGuidancePoint {
                x_m: 0.0,
                y_m: 0.75,
            },
            &[
                TractorGuidancePoint {
                    x_m: 0.0,
                    y_m: 0.25,
                },
                TractorGuidancePoint {
                    x_m: 0.0,
                    y_m: -0.25,
                },
            ],
            tractor_guidance_test_config(1.0, 1.0),
        )
        .expect("simulation guidance should run");

        assert!(!result.halted);
        assert_eq!(result.fault, None);
        assert!(!result.telemetry.is_empty());
        assert!(result
            .telemetry
            .iter()
            .all(|tick| tick.cross_track_error_m.abs() <= 1.0));
    }

    #[test]
    fn tractor_guidance_unrecoverable_disturbance_halts_with_fault() {
        let result = run_tractor_straight_path_guidance(
            tractor_guidance_test_path(),
            TractorGuidancePoint { x_m: 0.0, y_m: 0.0 },
            &[TractorGuidancePoint { x_m: 0.0, y_m: 5.0 }],
            tractor_guidance_test_config(1.0, 0.25),
        )
        .expect("simulation guidance should run");

        assert!(result.halted);
        assert_eq!(
            result.fault,
            Some(TractorGuidanceFault::CrossTrackErrorExceeded)
        );
        let last = result.telemetry.last().expect("halt tick is recorded");
        assert!(last.halted);
        assert_eq!(
            last.fault,
            Some(TractorGuidanceFault::CrossTrackErrorExceeded)
        );
        assert!(result.max_observed_cross_track_error_m > 1.0);
    }

    #[test]
    fn tractor_guidance_rejects_non_simulation_runtime() {
        let error = run_tractor_straight_path_guidance(
            tractor_guidance_test_path(),
            TractorGuidancePoint { x_m: 0.0, y_m: 0.0 },
            &[],
            TractorGuidanceConfig {
                runtime_mode: "production".to_string(),
                ..tractor_guidance_test_config(1.0, 1.0)
            },
        )
        .expect_err("real motion is hard-disabled for 14-02");

        assert_eq!(
            error,
            TractorGuidanceError::RuntimeModeNotSimulation {
                runtime_mode: "production".to_string()
            }
        );
    }

    #[test]
    fn tractor_swath_planner_generates_inside_boundary_coverage() {
        let plan = plan_tractor_swath_coverage(TractorSwathCoverageRequest {
            field_boundary: tractor_swath_rectangle(0.0, 0.0, 10.0, 10.0, "EPSG:3857"),
            exclusion_boundaries: Vec::new(),
            implement_width_m: 2.0,
        })
        .expect("rectangular field should plan");

        assert_eq!(plan.crs, "EPSG:3857");
        assert_eq!(plan.swaths.len(), 5);
        assert!(plan.all_swaths_inside_boundary);
        assert!(plan.avoided_exclusions);
        assert_eq!(plan.coverage_fraction, 1.0);
        assert!(plan
            .swaths
            .iter()
            .all(|swath| swath.start.longitude == 0.0 && swath.end.longitude == 10.0));
    }

    #[test]
    fn tractor_swath_planner_clips_paths_around_exclusion() {
        let plan = plan_tractor_swath_coverage(TractorSwathCoverageRequest {
            field_boundary: tractor_swath_rectangle(0.0, 0.0, 10.0, 10.0, "EPSG:3857"),
            exclusion_boundaries: vec![tractor_swath_rectangle(4.0, 0.0, 6.0, 10.0, "EPSG:3857")],
            implement_width_m: 2.0,
        })
        .expect("field with exclusion should plan");

        assert!(plan.all_swaths_inside_boundary);
        assert!(plan.avoided_exclusions);
        assert!(plan.coverage_fraction < 1.0);
        assert_eq!(plan.swaths.len(), 10);
        assert!(plan
            .swaths
            .iter()
            .all(|swath| swath.end.longitude <= 4.0 || swath.start.longitude >= 6.0));
    }

    #[test]
    fn tractor_field_ops_session_persists_telemetry_and_coverage() {
        let session = build_tractor_field_ops_session_log(TractorFieldOpsSessionRequest {
            session_id: " session-001 ".to_string(),
            tractor_id: " tractor-001 ".to_string(),
            field_id: " field-north ".to_string(),
            started_at: " 2026-06-15T10:00:00Z ".to_string(),
            telemetry: vec![
                tractor_field_ops_sample("2026-06-15T10:00:02Z", 6.0, 0.0, 2.0, true),
                tractor_field_ops_sample("2026-06-15T10:00:00Z", 0.0, 0.0, 2.0, true),
                tractor_field_ops_sample("2026-06-15T10:00:01Z", 3.0, 0.0, 2.0, true),
            ],
            implement_width_m: 2.0,
            planned_area_m2: 24.0,
            max_telemetry_gap_seconds: 2,
        })
        .expect("field ops session should persist");

        assert_eq!(session.session_id, "session-001");
        assert_eq!(session.tractor_id, "tractor-001");
        assert_eq!(session.field_id, "field-north");
        assert_eq!(session.telemetry.len(), 3);
        assert_eq!(session.telemetry[0].timestamp, "2026-06-15T10:00:00Z");
        assert_eq!(session.telemetry[2].position.x_m, 6.0);
        assert_eq!(session.coverage.distance_m, 6.0);
        assert_eq!(session.coverage.covered_area_m2, 12.0);
        assert_eq!(session.coverage.coverage_fraction, 0.5);
        assert!(session.safety_events.is_empty());
        assert_eq!(session.telemetry_gap_count, 0);
    }

    #[test]
    fn tractor_field_ops_session_flags_telemetry_dropout() {
        let session = build_tractor_field_ops_session_log(TractorFieldOpsSessionRequest {
            session_id: "session-gap".to_string(),
            tractor_id: "tractor-001".to_string(),
            field_id: "field-north".to_string(),
            started_at: "2026-06-15T10:00:00Z".to_string(),
            telemetry: vec![
                tractor_field_ops_sample("2026-06-15T10:00:00Z", 0.0, 0.0, 2.0, true),
                tractor_field_ops_sample("2026-06-15T10:00:10Z", 2.0, 0.0, 2.0, true),
            ],
            implement_width_m: 2.0,
            planned_area_m2: 10.0,
            max_telemetry_gap_seconds: 3,
        })
        .expect("field ops session with gap should persist");

        assert_eq!(session.telemetry_gap_count, 1);
        assert_eq!(session.safety_events.len(), 1);
        assert_eq!(
            session.safety_events[0].event_type,
            TractorFieldOpsSafetyEventType::TelemetryDropout
        );
        assert_eq!(session.safety_events[0].reason_code, "telemetry_dropout");
        assert!(session.safety_events[0].details.contains("10s"));
        assert_eq!(session.coverage.distance_m, 2.0);
    }

    #[test]
    fn tractor_replay_is_deterministic_and_read_only() {
        let session = build_tractor_field_ops_session_log(tractor_field_ops_session_request())
            .expect("session log should build");

        let first = build_tractor_field_ops_replay(&session).expect("replay should build");
        let second = build_tractor_field_ops_replay(&session).expect("replay should build again");

        assert_eq!(first, second);
        assert!(first.read_only);
        assert_eq!(first.session_id, "session-001");
        assert_eq!(first.frames.len(), 3);
        assert!(first
            .frames
            .iter()
            .all(|frame| frame.frame_type == TractorFieldOpsReplayFrameType::Telemetry));
        assert_eq!(first.frames[0].at, "2026-06-15T10:00:00Z");
        assert_eq!(first.frames[2].at, "2026-06-15T10:00:02Z");
    }

    #[test]
    fn tractor_replay_renders_safety_events_on_timeline() {
        let mut session = build_tractor_field_ops_session_log(tractor_field_ops_session_request())
            .expect("session log should build");
        session.safety_events.push(TractorFieldOpsSafetyEvent {
            event_type: TractorFieldOpsSafetyEventType::ManualEstop,
            at: "2026-06-15T10:00:01Z".to_string(),
            reason_code: "operator_estop".to_string(),
            details: "operator stopped tractor".to_string(),
        });

        let replay = build_tractor_field_ops_replay(&session).expect("replay should build");

        assert!(replay.frames.iter().any(|frame| {
            frame.frame_type == TractorFieldOpsReplayFrameType::SafetyEvent
                && frame.note == "operator_estop"
                && frame.at == "2026-06-15T10:00:01Z"
        }));
    }

    #[test]
    fn tractor_replay_shows_gap_without_fabricated_path() {
        let session = build_tractor_field_ops_session_log(TractorFieldOpsSessionRequest {
            session_id: "session-gap".to_string(),
            tractor_id: "tractor-001".to_string(),
            field_id: "field-north".to_string(),
            started_at: "2026-06-15T10:00:00Z".to_string(),
            telemetry: vec![
                tractor_field_ops_sample("2026-06-15T10:00:00Z", 0.0, 0.0, 2.0, true),
                tractor_field_ops_sample("2026-06-15T10:00:10Z", 2.0, 0.0, 2.0, true),
            ],
            implement_width_m: 2.0,
            planned_area_m2: 10.0,
            max_telemetry_gap_seconds: 3,
        })
        .expect("field ops session with gap should persist");

        let replay = build_tractor_field_ops_replay(&session).expect("replay should build");

        assert_eq!(replay.gap_count, 1);
        let gap = replay
            .frames
            .iter()
            .find(|frame| frame.frame_type == TractorFieldOpsReplayFrameType::TelemetryGap)
            .expect("gap frame is explicit");
        assert_eq!(gap.at, "2026-06-15T10:00:10Z");
        assert_eq!(gap.telemetry, None);
        assert_eq!(gap.note, "explicit_gap_no_interpolation");
    }

    #[test]
    fn tractor_geofence_permits_move_inside_boundary() {
        let evaluation = evaluate_tractor_geofence(tractor_geofence_request(
            GeoPoint {
                longitude: 2.0,
                latitude: 2.0,
            },
            GeoPoint {
                longitude: 8.0,
                latitude: 8.0,
            },
            "EPSG:3857",
        ))
        .expect("inside geofence evaluates");

        assert_eq!(evaluation.decision, TractorGeofenceDecision::Permitted);
        assert_eq!(evaluation.reason_code, "inside_geofence");
        assert_eq!(evaluation.boundary_ref, "field-north-boundary");
        assert_eq!(evaluation.boundary_crs, "EPSG:3857");
    }

    #[test]
    fn tractor_geofence_halts_predicted_boundary_crossing() {
        let evaluation = evaluate_tractor_geofence(tractor_geofence_request(
            GeoPoint {
                longitude: 8.0,
                latitude: 8.0,
            },
            GeoPoint {
                longitude: 12.0,
                latitude: 8.0,
            },
            "EPSG:3857",
        ))
        .expect("predicted breach evaluates");

        assert_eq!(evaluation.decision, TractorGeofenceDecision::Halted);
        assert_eq!(evaluation.reason_code, "geofence_predicted_breach");
        assert_eq!(evaluation.predicted_position.longitude, 12.0);
    }

    #[test]
    fn tractor_geofence_rejects_crs_mismatch() {
        let error = evaluate_tractor_geofence(tractor_geofence_request(
            GeoPoint {
                longitude: 2.0,
                latitude: 2.0,
            },
            GeoPoint {
                longitude: 8.0,
                latitude: 8.0,
            },
            "EPSG:4326",
        ))
        .expect_err("position CRS must match boundary CRS");

        assert_eq!(
            error,
            TractorGeofenceError::CrsMismatch {
                position_crs: "EPSG:4326".to_string(),
                boundary_crs: "EPSG:3857".to_string()
            }
        );
    }

    #[test]
    fn tractor_motion_gate_estop_preempts_approval() {
        let evaluation = evaluate_tractor_motion_gate(
            &tractor_motion_gate_command(),
            Some(&TractorEstopState {
                tractor_id: "tractor-001".to_string(),
                active: true,
                triggered_by: Some("ops@example.com".to_string()),
                triggered_at: Some("2026-06-15T10:00:00Z".to_string()),
                reason_code: Some("operator_estop".to_string()),
            }),
            Some(&tractor_operator_approval()),
            "2026-06-15T10:00:02Z",
        )
        .expect("motion gate evaluates");

        assert_eq!(evaluation.decision, TractorMotionGateDecision::Refused);
        assert!(evaluation.halted);
        assert_eq!(evaluation.approval_id, None);
        assert_eq!(evaluation.audit.reason_code, "estop_active");
    }

    #[test]
    fn tractor_motion_gate_allows_approved_motion() {
        let evaluation = evaluate_tractor_motion_gate(
            &tractor_motion_gate_command(),
            None,
            Some(&tractor_operator_approval()),
            "2026-06-15T10:00:02Z",
        )
        .expect("motion gate evaluates");

        assert_eq!(evaluation.decision, TractorMotionGateDecision::Allowed);
        assert!(!evaluation.halted);
        assert_eq!(evaluation.approval_id.as_deref(), Some("approval-001"));
        assert_eq!(evaluation.audit.reason_code, "operator_approved");
        assert_eq!(evaluation.audit.actor.as_deref(), Some("ops@example.com"));
    }

    #[test]
    fn tractor_motion_gate_refuses_unapproved_motion() {
        let evaluation = evaluate_tractor_motion_gate(
            &tractor_motion_gate_command(),
            None,
            None,
            "2026-06-15T10:00:02Z",
        )
        .expect("motion gate evaluates");

        assert_eq!(evaluation.decision, TractorMotionGateDecision::Refused);
        assert!(!evaluation.halted);
        assert_eq!(evaluation.approval_id, None);
        assert_eq!(evaluation.audit.reason_code, "operator_approval_required");
    }

    #[test]
    fn tractor_obstacle_detector_does_not_false_halt_clear_path() {
        let detection = detect_tractor_obstacle(TractorObstacleDetectionRequest {
            tractor_id: "tractor-001".to_string(),
            path: tractor_guidance_test_path(),
            current_position: TractorGuidancePoint { x_m: 0.0, y_m: 0.0 },
            obstacles: vec![
                TractorGuidancePoint { x_m: 5.0, y_m: 5.0 },
                TractorGuidancePoint {
                    x_m: 30.0,
                    y_m: 0.0,
                },
            ],
            path_width_m: 2.0,
            stopping_distance_m: 10.0,
        })
        .expect("obstacle detector evaluates");

        assert!(!detection.halted);
        assert_eq!(detection.event, None);
    }

    #[test]
    fn tractor_obstacle_detector_halts_for_obstacle_in_path() {
        let detection = detect_tractor_obstacle(TractorObstacleDetectionRequest {
            tractor_id: "tractor-001".to_string(),
            path: tractor_guidance_test_path(),
            current_position: TractorGuidancePoint { x_m: 0.0, y_m: 0.0 },
            obstacles: vec![
                TractorGuidancePoint { x_m: 8.0, y_m: 0.5 },
                TractorGuidancePoint {
                    x_m: 4.0,
                    y_m: 0.25,
                },
            ],
            path_width_m: 2.0,
            stopping_distance_m: 10.0,
        })
        .expect("obstacle detector evaluates");

        let event = detection.event.expect("obstacle event records");
        assert!(detection.halted);
        assert_eq!(event.reason_code, "obstacle_in_path");
        assert_eq!(event.distance_m, 4.0);
        assert_eq!(event.position.x_m, 4.0);
    }

    #[test]
    fn tractor_prescription_execution_applies_rates_per_zone() {
        let log = execute_tractor_prescription(tractor_prescription_request(vec![
            tractor_prescription_zone("zone-b", "EPSG:3857", 5.0, 5.0, 9.0, 9.0, 22.0),
            tractor_prescription_zone("zone-a", "EPSG:3857", 0.0, 0.0, 4.0, 4.0, 12.5),
        ]))
        .expect("valid prescription executes");

        assert_eq!(log.runtime_mode, "simulation");
        assert_eq!(log.applied_rates.len(), 2);
        assert_eq!(log.applied_rates[0].zone_id, "zone-a");
        assert_eq!(log.applied_rates[0].rate, 12.5);
        assert_eq!(log.applied_rates[1].zone_id, "zone-b");
        assert_eq!(
            log.applied_rates[1].reason_code,
            "prescription_rate_applied"
        );
        assert!(log.applied_rates[1]
            .evidence_refs
            .contains(&"zone:zone-b".to_string()));
    }

    #[test]
    fn tractor_prescription_execution_refuses_crs_mismatch() {
        let error = execute_tractor_prescription(tractor_prescription_request(vec![
            tractor_prescription_zone("zone-a", "EPSG:4326", 0.0, 0.0, 4.0, 4.0, 12.5),
        ]))
        .expect_err("CRS mismatch refuses execution");

        assert_eq!(
            error,
            TractorPrescriptionExecutionError::ZoneCrsMismatch {
                zone_id: "zone-a".to_string(),
                field_crs: "EPSG:3857".to_string(),
                zone_crs: "EPSG:4326".to_string()
            }
        );
    }

    #[test]
    fn tractor_prescription_execution_requires_safety_prerequisites() {
        let mut request = tractor_prescription_request(vec![tractor_prescription_zone(
            "zone-a",
            "EPSG:3857",
            0.0,
            0.0,
            4.0,
            4.0,
            12.5,
        )]);
        request.obstacle = TractorObstacleDetection {
            tractor_id: "tractor-001".to_string(),
            halted: true,
            event: Some(TractorObstacleEvent {
                distance_m: 2.0,
                position: TractorGuidancePoint { x_m: 2.0, y_m: 0.0 },
                reason_code: "obstacle_in_path".to_string(),
            }),
        };

        let error = execute_tractor_prescription(request)
            .expect_err("obstacle halt blocks prescription execution");

        assert_eq!(
            error,
            TractorPrescriptionExecutionError::SafetyPrerequisiteFailed {
                reason_code: "obstacle_in_path".to_string()
            }
        );
    }

    #[test]
    fn tractor_implement_adapter_applies_valid_rate_and_logs() {
        let result = apply_tractor_implement_command(
            tractor_implement_spec(),
            tractor_implement_state(true, 10.0),
            TractorImplementCommand::SetRate { rate: 22.0 },
            &tractor_allowed_motion_gate(),
            "2026-06-15T10:00:03Z",
        )
        .expect("valid implement setpoint applies");

        assert!(result.state.enabled);
        assert_eq!(result.state.current_rate, 22.0);
        assert_eq!(result.log.decision, TractorImplementDecision::Applied);
        assert_eq!(result.log.requested_rate, Some(22.0));
        assert_eq!(result.log.applied_rate, Some(22.0));
        assert_eq!(result.log.reason_code, "rate_applied");
    }

    #[test]
    fn tractor_implement_adapter_refuses_out_of_range_rate() {
        let result = apply_tractor_implement_command(
            tractor_implement_spec(),
            tractor_implement_state(true, 10.0),
            TractorImplementCommand::SetRate { rate: 40.0 },
            &tractor_allowed_motion_gate(),
            "2026-06-15T10:00:03Z",
        )
        .expect("out-of-range setpoint is refused without unsafe rate");

        assert!(result.state.enabled);
        assert_eq!(result.state.current_rate, 10.0);
        assert!(result.state.current_rate <= tractor_implement_spec().max_rate);
        assert_eq!(result.log.decision, TractorImplementDecision::Refused);
        assert_eq!(result.log.requested_rate, Some(40.0));
        assert_eq!(result.log.applied_rate, Some(10.0));
        assert_eq!(result.log.reason_code, "rate_out_of_bounds");
    }

    #[test]
    fn tractor_implement_adapter_forces_off_when_halted() {
        let result = apply_tractor_implement_command(
            tractor_implement_spec(),
            tractor_implement_state(true, 18.0),
            TractorImplementCommand::Enable,
            &tractor_halted_motion_gate(),
            "2026-06-15T10:00:03Z",
        )
        .expect("halted tractor forces implement off");

        assert!(!result.state.enabled);
        assert_eq!(result.state.current_rate, 18.0);
        assert_eq!(result.log.decision, TractorImplementDecision::ForcedOff);
        assert_eq!(result.log.reason_code, "tractor_halted");
    }

    #[test]
    fn tractor_weather_window_gate_allows_valid_window() {
        let gate = evaluate_tractor_weather_window_gate(tractor_weather_window_gate_request(Some(
            tractor_field_window(
                true,
                "2026-06-15T09:55:00Z",
                "2026-06-15T10:00:00Z",
                "2026-06-15T11:00:00Z",
                "safe_field_window",
            ),
        )))
        .expect("valid weather window should evaluate");

        assert_eq!(gate.decision, TractorWeatherWindowDecision::Allowed);
        assert_eq!(gate.reason_code, "weather_window_allowed");
        assert_eq!(gate.window_source.as_deref(), Some("domain-15"));
        assert!(gate.gating_inputs.contains(&"wind_mps:3.2".to_string()));
    }

    #[test]
    fn tractor_weather_window_gate_blocks_stale_window() {
        let gate = evaluate_tractor_weather_window_gate(tractor_weather_window_gate_request(Some(
            tractor_field_window(
                true,
                "2026-06-15T08:30:00Z",
                "2026-06-15T10:00:00Z",
                "2026-06-15T11:00:00Z",
                "safe_field_window",
            ),
        )))
        .expect("stale weather window should evaluate");

        assert_eq!(gate.decision, TractorWeatherWindowDecision::Blocked);
        assert_eq!(gate.reason_code, "weather_window_stale");
        assert!(gate
            .gating_inputs
            .contains(&"fetched_at:2026-06-15T08:30:00Z".to_string()));
    }

    #[test]
    fn tractor_weather_window_gate_blocks_missing_or_outside_window() {
        let missing =
            evaluate_tractor_weather_window_gate(tractor_weather_window_gate_request(None))
                .expect("missing weather window should evaluate");

        assert_eq!(missing.decision, TractorWeatherWindowDecision::Blocked);
        assert_eq!(missing.reason_code, "weather_window_missing");

        let outside = evaluate_tractor_weather_window_gate(tractor_weather_window_gate_request(
            Some(tractor_field_window(
                true,
                "2026-06-15T09:55:00Z",
                "2026-06-15T10:30:00Z",
                "2026-06-15T11:00:00Z",
                "safe_field_window",
            )),
        ))
        .expect("outside weather window should evaluate");

        assert_eq!(outside.decision, TractorWeatherWindowDecision::Blocked);
        assert_eq!(outside.reason_code, "outside_weather_window");
    }

    #[test]
    fn tractor_deconfliction_allows_non_overlapping_swaths() {
        let plan = deconflict_tractor_swath_reservations(TractorDeconflictionRequest {
            field_id: "field-north".to_string(),
            evaluated_at: "2026-06-15T10:00:00Z".to_string(),
            reservations: vec![
                tractor_swath_reservation("tractor-001", 1, 1.0, 0.0, 10.0),
                tractor_swath_reservation("tractor-002", 2, 4.0, 0.0, 10.0),
            ],
        })
        .expect("non-overlapping swaths should deconflict");

        assert!(plan.all_clear);
        assert!(plan.events.is_empty());
        assert!(plan
            .decisions
            .iter()
            .all(|decision| decision.decision == TractorDeconflictionDecision::Proceed));
    }

    #[test]
    fn tractor_deconfliction_halts_lower_priority_on_conflict() {
        let plan = deconflict_tractor_swath_reservations(TractorDeconflictionRequest {
            field_id: "field-north".to_string(),
            evaluated_at: "2026-06-15T10:00:00Z".to_string(),
            reservations: vec![
                tractor_swath_reservation("tractor-001", 1, 1.0, 0.0, 10.0),
                tractor_swath_reservation("tractor-002", 3, 1.2, 0.0, 10.0),
            ],
        })
        .expect("conflicting swaths should deconflict");

        assert!(!plan.all_clear);
        assert_eq!(plan.events.len(), 1);
        assert_eq!(plan.events[0].halted_tractor_id, "tractor-002");
        let halted = plan
            .decisions
            .iter()
            .find(|decision| decision.tractor_id == "tractor-002")
            .expect("halted tractor decision is present");
        assert_eq!(halted.decision, TractorDeconflictionDecision::Halted);
        assert_eq!(halted.reason_code, "swath_time_conflict");
        assert_eq!(halted.conflict_with.as_deref(), Some("tractor-001"));
    }

    #[test]
    fn tractor_deconfliction_halts_failed_safety_prerequisite() {
        let mut reservation = tractor_swath_reservation("tractor-002", 2, 4.0, 0.0, 10.0);
        reservation.obstacle = TractorObstacleDetection {
            tractor_id: "tractor-002".to_string(),
            halted: true,
            event: Some(TractorObstacleEvent {
                distance_m: 1.0,
                position: TractorGuidancePoint { x_m: 1.0, y_m: 0.0 },
                reason_code: "obstacle_in_path".to_string(),
            }),
        };
        let plan = deconflict_tractor_swath_reservations(TractorDeconflictionRequest {
            field_id: "field-north".to_string(),
            evaluated_at: "2026-06-15T10:00:00Z".to_string(),
            reservations: vec![reservation],
        })
        .expect("failed safety prerequisite should halt");

        assert!(!plan.all_clear);
        assert_eq!(
            plan.decisions[0].decision,
            TractorDeconflictionDecision::Halted
        );
        assert_eq!(plan.decisions[0].reason_code, "obstacle_in_path");
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
    fn marketplace_catalog_item_persists_for_active_owner_account() {
        let owner = marketplace_account_fixture(
            "supplier-001",
            "org-alpha",
            MarketplacePartyType::Supplier,
            MarketplaceAccountStatus::Active,
        );

        let item = super::build_marketplace_catalog_item_record(
            MarketplaceCatalogItemCreateRequest {
                item_id: Some(" seed-corn-001 ".to_string()),
                org_id: " org-alpha ".to_string(),
                kind: MarketplaceCatalogItemKind::Input,
                category: MarketplaceCatalogCategory::Seed,
                name: " Hybrid corn seed ".to_string(),
                unit_of_measure: MarketplaceUnitOfMeasure::Bag,
                owner_account_id: " supplier-001 ".to_string(),
            },
            Some(&owner),
            "generated-item".to_string(),
            " 2026-06-14T09:00:00Z ".to_string(),
        )
        .expect("active supplier should own catalog item");

        assert_eq!(item.item_id, "seed-corn-001");
        assert_eq!(item.org_id, "org-alpha");
        assert_eq!(item.kind, MarketplaceCatalogItemKind::Input);
        assert_eq!(item.category, MarketplaceCatalogCategory::Seed);
        assert_eq!(item.name, "Hybrid corn seed");
        assert_eq!(item.unit_of_measure, MarketplaceUnitOfMeasure::Bag);
        assert_eq!(item.owner_account_id, "supplier-001");
    }

    #[test]
    fn marketplace_catalog_item_rejects_non_active_or_cross_org_owner() {
        let suspended = marketplace_account_fixture(
            "supplier-001",
            "org-alpha",
            MarketplacePartyType::Supplier,
            MarketplaceAccountStatus::Suspended,
        );
        let error = super::build_marketplace_catalog_item_record(
            marketplace_catalog_item_request(MarketplaceUnitOfMeasure::Bag),
            Some(&suspended),
            "generated-item".to_string(),
            "2026-06-14T09:00:00Z".to_string(),
        )
        .expect_err("suspended account cannot own catalog item");
        assert_eq!(
            error,
            MarketplaceCatalogError::OwnerAccountNotActive {
                account_id: "supplier-001".to_string()
            }
        );

        let foreign_org = marketplace_account_fixture(
            "supplier-001",
            "org-beta",
            MarketplacePartyType::Supplier,
            MarketplaceAccountStatus::Active,
        );
        let error = super::build_marketplace_catalog_item_record(
            marketplace_catalog_item_request(MarketplaceUnitOfMeasure::Bag),
            Some(&foreign_org),
            "generated-item".to_string(),
            "2026-06-14T09:00:00Z".to_string(),
        )
        .expect_err("cross-org account cannot own catalog item");
        assert_eq!(
            error,
            MarketplaceCatalogError::OwnerOrgMismatch {
                account_id: "supplier-001".to_string(),
                account_org_id: "org-beta".to_string(),
                item_org_id: "org-alpha".to_string()
            }
        );
    }

    #[test]
    fn marketplace_catalog_unit_vocabulary_rejects_unknown_unit() {
        let error = super::parse_marketplace_unit_of_measure("pallet")
            .expect_err("unsupported unit should reject");

        assert_eq!(
            error,
            MarketplaceCatalogError::UnsupportedUnit {
                value: "pallet".to_string()
            }
        );
    }

    #[test]
    fn marketplace_portal_entry_renders_for_marketplace_role() {
        let account = marketplace_account_fixture(
            "grower-001",
            "org-alpha",
            MarketplacePartyType::Grower,
            MarketplaceAccountStatus::Active,
        );

        let entry = build_marketplace_portal_entry(Some(&account), " org-alpha ".to_string())
            .expect("marketplace role should show portal entry");

        assert_eq!(entry.org_id, "org-alpha");
        assert_eq!(entry.account_id, "grower-001");
        assert_eq!(entry.label, "Marketplace");
        assert_eq!(entry.href, "/marketplace?org_id=org-alpha");
        assert!(entry.visible);
    }

    #[test]
    fn marketplace_portal_entry_rejects_account_without_access_role() {
        let mut account = marketplace_account_fixture(
            "grower-001",
            "org-alpha",
            MarketplacePartyType::Grower,
            MarketplaceAccountStatus::Active,
        );
        account.role_refs = vec!["portal:viewer".to_string()];

        let error = build_marketplace_portal_entry(Some(&account), "org-alpha".to_string())
            .expect_err("missing marketplace role should hide entry");

        assert_eq!(
            error,
            MarketplacePortalEntryError::MissingMarketplaceRole {
                account_id: "grower-001".to_string()
            }
        );
    }

    #[test]
    fn marketplace_listing_publishes_from_catalog_item() {
        let item = marketplace_catalog_item_fixture("seed-corn-001", "org-alpha");

        let listing = publish_marketplace_listing_record(
            marketplace_listing_request("2026-06-14T09:00:00Z", "2026-07-14T09:00:00Z"),
            Some(&item),
            "generated-listing".to_string(),
            "2026-06-14T08:00:00Z".to_string(),
        )
        .expect("valid listing should publish");

        assert_eq!(listing.listing_id, "listing-seed-corn-001");
        assert_eq!(listing.item_id, "seed-corn-001");
        assert_eq!(listing.org_id, "org-alpha");
        assert_eq!(listing.price, 125.0);
        assert_eq!(listing.currency, "USD");
        assert_eq!(listing.available_qty, 40.0);
        assert_eq!(listing.status, MarketplaceListingStatus::Published);
    }

    #[test]
    fn marketplace_listing_close_updates_status() {
        let item = marketplace_catalog_item_fixture("seed-corn-001", "org-alpha");
        let listing = publish_marketplace_listing_record(
            marketplace_listing_request("2026-06-14T09:00:00Z", "2026-07-14T09:00:00Z"),
            Some(&item),
            "generated-listing".to_string(),
            "2026-06-14T08:00:00Z".to_string(),
        )
        .expect("valid listing should publish");

        let closed = close_marketplace_listing_record(&listing, "2026-06-15T08:00:00Z".to_string())
            .expect("listing should close");

        assert_eq!(closed.status, MarketplaceListingStatus::Closed);
        assert_eq!(closed.updated_at, "2026-06-15T08:00:00Z");
        assert_eq!(closed.created_at, listing.created_at);
    }

    #[test]
    fn marketplace_listing_rejects_inverted_window() {
        let item = marketplace_catalog_item_fixture("seed-corn-001", "org-alpha");

        let error = publish_marketplace_listing_record(
            marketplace_listing_request("2026-07-14T09:00:00Z", "2026-06-14T09:00:00Z"),
            Some(&item),
            "generated-listing".to_string(),
            "2026-06-14T08:00:00Z".to_string(),
        )
        .expect_err("inverted availability window should reject");

        assert_eq!(error, MarketplaceListingError::InvalidWindowRange);
    }

    #[test]
    fn marketplace_inventory_reserves_available_stock() {
        let item = marketplace_catalog_item_fixture("seed-corn-001", "org-alpha");
        let inventory = build_marketplace_inventory_record(
            marketplace_inventory_request(40.0, Some(0.0)),
            Some(&item),
            "generated-inventory".to_string(),
            "2026-06-14T08:00:00Z".to_string(),
        )
        .expect("valid inventory should build");

        let reserved =
            reserve_marketplace_inventory(&inventory, 25.0, "2026-06-14T09:00:00Z".to_string())
                .expect("available stock should reserve");

        assert_eq!(reserved.on_hand, 40.0);
        assert_eq!(reserved.reserved, 25.0);
        assert_eq!(marketplace_inventory_available(&reserved), 15.0);
    }

    #[test]
    fn marketplace_inventory_rejects_over_reserve() {
        let item = marketplace_catalog_item_fixture("seed-corn-001", "org-alpha");
        let inventory = build_marketplace_inventory_record(
            marketplace_inventory_request(40.0, Some(10.0)),
            Some(&item),
            "generated-inventory".to_string(),
            "2026-06-14T08:00:00Z".to_string(),
        )
        .expect("valid inventory should build");

        let error =
            reserve_marketplace_inventory(&inventory, 31.0, "2026-06-14T09:00:00Z".to_string())
                .expect_err("reservation should not exceed available stock");

        assert_eq!(
            error,
            MarketplaceInventoryError::InsufficientAvailableQuantity
        );
    }

    #[test]
    fn marketplace_inventory_fulfills_and_releases_reserved_stock() {
        let item = marketplace_catalog_item_fixture("seed-corn-001", "org-alpha");
        let inventory = build_marketplace_inventory_record(
            marketplace_inventory_request(40.0, Some(20.0)),
            Some(&item),
            "generated-inventory".to_string(),
            "2026-06-14T08:00:00Z".to_string(),
        )
        .expect("valid inventory should build");

        let fulfilled =
            fulfill_marketplace_inventory(&inventory, 12.0, "2026-06-14T10:00:00Z".to_string())
                .expect("reserved stock should fulfill");
        assert_eq!(fulfilled.on_hand, 28.0);
        assert_eq!(fulfilled.reserved, 8.0);

        let released =
            release_marketplace_inventory(&fulfilled, 8.0, "2026-06-14T11:00:00Z".to_string())
                .expect("remaining reserved stock should release");
        assert_eq!(released.on_hand, 28.0);
        assert_eq!(released.reserved, 0.0);
    }

    #[test]
    fn marketplace_order_places_with_line_total_and_audit() {
        let listing = marketplace_listing_fixture();
        let buyer = marketplace_account_fixture(
            "buyer-001",
            "org-alpha",
            MarketplacePartyType::Grower,
            MarketplaceAccountStatus::Active,
        );

        let (order, audit) = place_marketplace_order_record(
            marketplace_order_request(3.0),
            Some(&listing),
            Some(&buyer),
            "generated-order".to_string(),
            "2026-06-14T12:00:00Z".to_string(),
        )
        .expect("valid order should place");

        assert_eq!(order.status, MarketplaceOrderStatus::Placed);
        assert_eq!(order.line_total, 375.0);
        assert_eq!(audit.from_status, None);
        assert_eq!(audit.to_status, MarketplaceOrderStatus::Placed);
    }

    #[test]
    fn marketplace_order_transitions_through_fulfilled_and_closed() {
        let listing = marketplace_listing_fixture();
        let buyer = marketplace_account_fixture(
            "buyer-001",
            "org-alpha",
            MarketplacePartyType::Grower,
            MarketplaceAccountStatus::Active,
        );
        let (order, _) = place_marketplace_order_record(
            marketplace_order_request(3.0),
            Some(&listing),
            Some(&buyer),
            "generated-order".to_string(),
            "2026-06-14T12:00:00Z".to_string(),
        )
        .expect("valid order should place");

        let (confirmed, confirm_audit) = transition_marketplace_order_status(
            &order,
            MarketplaceOrderStatus::Confirmed,
            "supplier-001".to_string(),
            "2026-06-14T13:00:00Z".to_string(),
        )
        .expect("placed order should confirm");
        let (fulfilled, _) = transition_marketplace_order_status(
            &confirmed,
            MarketplaceOrderStatus::Fulfilled,
            "supplier-001".to_string(),
            "2026-06-14T14:00:00Z".to_string(),
        )
        .expect("confirmed order should fulfill");
        let (closed, _) = transition_marketplace_order_status(
            &fulfilled,
            MarketplaceOrderStatus::Closed,
            "buyer-001".to_string(),
            "2026-06-14T15:00:00Z".to_string(),
        )
        .expect("fulfilled order should close");

        assert_eq!(
            confirm_audit.from_status,
            Some(MarketplaceOrderStatus::Placed)
        );
        assert_eq!(closed.status, MarketplaceOrderStatus::Closed);
    }

    #[test]
    fn marketplace_order_rejects_illegal_transition() {
        let listing = marketplace_listing_fixture();
        let buyer = marketplace_account_fixture(
            "buyer-001",
            "org-alpha",
            MarketplacePartyType::Grower,
            MarketplaceAccountStatus::Active,
        );
        let (order, _) = place_marketplace_order_record(
            marketplace_order_request(3.0),
            Some(&listing),
            Some(&buyer),
            "generated-order".to_string(),
            "2026-06-14T12:00:00Z".to_string(),
        )
        .expect("valid order should place");
        let (cancelled, _) = transition_marketplace_order_status(
            &order,
            MarketplaceOrderStatus::Cancelled,
            "buyer-001".to_string(),
            "2026-06-14T13:00:00Z".to_string(),
        )
        .expect("placed order should cancel");

        let error = transition_marketplace_order_status(
            &cancelled,
            MarketplaceOrderStatus::Confirmed,
            "supplier-001".to_string(),
            "2026-06-14T14:00:00Z".to_string(),
        )
        .expect_err("cancelled order should not confirm");

        assert_eq!(
            error,
            MarketplaceOrderError::InvalidStatusTransition {
                order_id: "order-seed-corn-001".to_string(),
                from: MarketplaceOrderStatus::Cancelled,
                to: MarketplaceOrderStatus::Confirmed,
            }
        );
    }

    #[test]
    fn marketplace_fulfillment_records_pending_shipment_for_confirmed_order() {
        let order = marketplace_order_fixture(
            "order-001",
            MarketplaceOrderStatus::Confirmed,
            125.0,
            "2026-06-15T09:00:00Z",
        );
        let (fulfillment, audit) = create_marketplace_fulfillment_record(
            marketplace_fulfillment_request("order-001", "org-alpha"),
            Some(&order),
            "generated-fulfillment".to_string(),
            "2026-06-15T10:00:00Z".to_string(),
        )
        .expect("confirmed order should accept fulfillment");

        assert_eq!(fulfillment.status, MarketplaceFulfillmentStatus::Pending);
        assert_eq!(fulfillment.carrier_ref, "carrier:opaque");
        assert_eq!(audit.to_status, MarketplaceFulfillmentStatus::Pending);
    }

    #[test]
    fn marketplace_fulfillment_transitions_to_delivered() {
        let order = marketplace_order_fixture(
            "order-001",
            MarketplaceOrderStatus::Confirmed,
            125.0,
            "2026-06-15T09:00:00Z",
        );
        let (fulfillment, _) = create_marketplace_fulfillment_record(
            marketplace_fulfillment_request("order-001", "org-alpha"),
            Some(&order),
            "generated-fulfillment".to_string(),
            "2026-06-15T10:00:00Z".to_string(),
        )
        .expect("confirmed order should accept fulfillment");

        let (shipped, _) = transition_marketplace_fulfillment_status(
            &fulfillment,
            MarketplaceFulfillmentStatus::Shipped,
            "ops-001".to_string(),
            "2026-06-15T11:00:00Z".to_string(),
        )
        .expect("pending fulfillment should ship");
        let (delivered, audit) = transition_marketplace_fulfillment_status(
            &shipped,
            MarketplaceFulfillmentStatus::Delivered,
            "ops-001".to_string(),
            "2026-06-16T11:00:00Z".to_string(),
        )
        .expect("shipped fulfillment should deliver");

        assert_eq!(delivered.status, MarketplaceFulfillmentStatus::Delivered);
        assert_eq!(
            audit.from_status,
            Some(MarketplaceFulfillmentStatus::Shipped)
        );
    }

    #[test]
    fn marketplace_fulfillment_rejects_cross_tenant_order() {
        let order = marketplace_order_fixture(
            "order-001",
            MarketplaceOrderStatus::Confirmed,
            125.0,
            "2026-06-15T09:00:00Z",
        );

        let error = create_marketplace_fulfillment_record(
            marketplace_fulfillment_request("order-001", "org-beta"),
            Some(&order),
            "generated-fulfillment".to_string(),
            "2026-06-15T10:00:00Z".to_string(),
        )
        .expect_err("foreign order should reject");

        assert_eq!(
            error,
            MarketplaceFulfillmentError::OrderOrgMismatch {
                order_id: "order-001".to_string(),
                order_org_id: "org-alpha".to_string(),
                fulfillment_org_id: "org-beta".to_string(),
            }
        );
    }

    #[test]
    fn marketplace_rating_persists_for_fulfilled_order_participant() {
        let order = marketplace_order_fixture(
            "order-001",
            MarketplaceOrderStatus::Fulfilled,
            125.0,
            "2026-06-15T09:00:00Z",
        );

        let rating = create_marketplace_rating_record(
            marketplace_rating_request("order-001", "buyer-001", "supplier-001", 5.0),
            Some(&order),
            &marketplace_rating_participants(),
            &[],
            "generated-rating".to_string(),
            "2026-06-17T09:00:00Z".to_string(),
        )
        .expect("participant should rate fulfilled order");

        assert_eq!(rating.ratee_account_id, "supplier-001");
        assert_eq!(rating.score, 5.0);
    }

    #[test]
    fn marketplace_rating_rejects_non_participant() {
        let order = marketplace_order_fixture(
            "order-001",
            MarketplaceOrderStatus::Fulfilled,
            125.0,
            "2026-06-15T09:00:00Z",
        );

        let error = create_marketplace_rating_record(
            marketplace_rating_request("order-001", "viewer-001", "supplier-001", 4.0),
            Some(&order),
            &marketplace_rating_participants(),
            &[],
            "generated-rating".to_string(),
            "2026-06-17T09:00:00Z".to_string(),
        )
        .expect_err("non-participant should not rate");

        assert_eq!(
            error,
            MarketplaceRatingError::RaterNotParticipant {
                order_id: "order-001".to_string(),
                rater_account_id: "viewer-001".to_string(),
            }
        );
    }

    #[test]
    fn marketplace_rating_aggregate_and_duplicate_gate_are_deterministic() {
        let order = marketplace_order_fixture(
            "order-001",
            MarketplaceOrderStatus::Closed,
            125.0,
            "2026-06-15T09:00:00Z",
        );
        let first = create_marketplace_rating_record(
            marketplace_rating_request("order-001", "buyer-001", "supplier-001", 5.0),
            Some(&order),
            &marketplace_rating_participants(),
            &[],
            "generated-rating".to_string(),
            "2026-06-17T09:00:00Z".to_string(),
        )
        .expect("first rating should persist");

        let duplicate = create_marketplace_rating_record(
            marketplace_rating_request("order-001", "buyer-001", "supplier-001", 4.0),
            Some(&order),
            &marketplace_rating_participants(),
            std::slice::from_ref(&first),
            "generated-rating-2".to_string(),
            "2026-06-18T09:00:00Z".to_string(),
        )
        .expect_err("same party should rate order once");
        assert_eq!(
            duplicate,
            MarketplaceRatingError::DuplicateRaterForOrder {
                order_id: "order-001".to_string(),
                rater_account_id: "buyer-001".to_string(),
            }
        );

        let aggregate = aggregate_marketplace_ratings(
            "supplier-001".to_string(),
            "org-alpha".to_string(),
            std::slice::from_ref(&first),
        )
        .expect("aggregate should compute");
        assert_eq!(aggregate.rating_count, 1);
        assert_eq!(aggregate.average_score, Some(5.0));
    }

    #[test]
    fn marketplace_demand_forecast_uses_area_and_product_evidence() {
        let forecast = compute_marketplace_demand_forecast(
            marketplace_demand_request(MarketplaceCatalogItemKind::Input, false),
            Some(&marketplace_demand_field_fixture(Some(20.0))),
            vec!["product:yield-map-001".to_string()],
            "generated-forecast".to_string(),
            "2026-06-15T09:00:00Z".to_string(),
        )
        .expect("forecast should compute");

        assert_eq!(forecast.status, MarketplaceDemandForecastStatus::Ready);
        assert_eq!(forecast.value, Some(20.0));
        assert!(forecast
            .evidence_refs
            .contains(&"product:yield-map-001".to_string()));
        assert!(forecast
            .evidence_refs
            .contains(&"field:field-alpha:area_ha:20.0000".to_string()));
    }

    #[test]
    fn marketplace_demand_forecast_ai_includes_uncertainty_and_evidence() {
        let forecast = compute_marketplace_demand_forecast(
            marketplace_demand_request(MarketplaceCatalogItemKind::Produce, true),
            Some(&marketplace_demand_field_fixture(Some(10.0))),
            vec![
                "product:yield-map-001".to_string(),
                "product:health-ndvi-001".to_string(),
            ],
            "generated-forecast".to_string(),
            "2026-06-15T09:00:00Z".to_string(),
        )
        .expect("ai-assisted forecast should compute");

        assert_eq!(forecast.value, Some(75.0));
        assert_eq!(
            forecast.uncertainty_band.as_ref().map(|band| band.low),
            Some(63.75)
        );
        assert!(forecast
            .evidence_refs
            .iter()
            .any(|evidence_ref| evidence_ref == "product:health-ndvi-001"));
    }

    #[test]
    fn marketplace_demand_forecast_returns_no_basis_without_evidence() {
        let forecast = compute_marketplace_demand_forecast(
            marketplace_demand_request(MarketplaceCatalogItemKind::Input, false),
            Some(&marketplace_demand_field_fixture(None)),
            Vec::new(),
            "generated-forecast".to_string(),
            "2026-06-15T09:00:00Z".to_string(),
        )
        .expect("no-basis forecast should be valid");

        assert_eq!(forecast.status, MarketplaceDemandForecastStatus::NoBasis);
        assert_eq!(forecast.value, None);
        assert_eq!(forecast.uncertainty_band, None);
    }

    #[test]
    fn marketplace_report_aggregates_orders_inventory_and_sources() {
        let report = assemble_marketplace_org_report(
            marketplace_report_request(),
            &[
                marketplace_order_fixture(
                    "order-001",
                    MarketplaceOrderStatus::Closed,
                    125.0,
                    "2026-06-15T09:00:00Z",
                ),
                marketplace_order_fixture(
                    "order-002",
                    MarketplaceOrderStatus::Placed,
                    50.0,
                    "2026-06-16T09:00:00Z",
                ),
            ],
            &[marketplace_listing_fixture()],
            &[marketplace_inventory_fixture(40.0, 5.0)],
            "2026-06-20T09:00:00Z".to_string(),
        )
        .expect("report should aggregate");

        assert_eq!(report.sales_total, 125.0);
        assert_eq!(report.procurement_total, 125.0);
        assert_eq!(report.listing_count, 1);
        assert_eq!(report.inventory_on_hand_total, 40.0);
        assert_eq!(report.inventory_reserved_total, 5.0);
        assert_eq!(report.source_order_ids, vec!["order-001", "order-002"]);
        assert!(report
            .order_counts_by_status
            .iter()
            .any(|count| count.status == MarketplaceOrderStatus::Closed && count.count == 1));
    }

    #[test]
    fn marketplace_report_empty_period_returns_zero_report() {
        let report = assemble_marketplace_org_report(
            marketplace_report_request(),
            &[],
            &[],
            &[],
            "2026-06-20T09:00:00Z".to_string(),
        )
        .expect("empty period should still report");

        assert_eq!(report.sales_total, 0.0);
        assert_eq!(report.procurement_total, 0.0);
        assert_eq!(report.order_counts_by_status.len(), 0);
        assert_eq!(report.source_order_ids.len(), 0);
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
    fn carbon_footprint_computes_value_and_retains_evidence() {
        let result = compute_carbon_footprint(
            carbon_footprint_request(true),
            "generated-footprint".to_string(),
            "2026-06-14T12:00:00Z".to_string(),
        )
        .expect("complete carbon inputs should compute");

        assert_eq!(result.footprint_id, "footprint-001");
        assert_eq!(result.record_id, "sustain-001");
        assert_eq!(result.operation_id, "operation-planting-001");
        assert_eq!(result.status, CarbonFootprintStatus::Computed);
        assert_eq!(result.value_co2e, Some(168.8));
        assert_eq!(result.factor_set_version, "agbot-carbon-factors-v1");
        assert!(result
            .evidence_refs
            .contains(&"factor_set:agbot-carbon-factors-v1".to_string()));
        assert!(result
            .evidence_refs
            .contains(&"input:fuel-log-001".to_string()));
        assert!(result
            .evidence_refs
            .contains(&"factor:diesel:v1".to_string()));
        assert!(!result.result_hash.is_empty());
    }

    #[test]
    fn carbon_footprint_same_inputs_reproduce_hash() {
        let first = compute_carbon_footprint(
            carbon_footprint_request(true),
            "generated-footprint-001".to_string(),
            "2026-06-14T12:00:00Z".to_string(),
        )
        .expect("first run should compute");
        let mut request = carbon_footprint_request(true);
        request.footprint_id = Some("footprint-002".to_string());
        let second = compute_carbon_footprint(
            request,
            "generated-footprint-002".to_string(),
            "2026-06-15T12:00:00Z".to_string(),
        )
        .expect("second run should compute");

        assert_eq!(first.value_co2e, second.value_co2e);
        assert_eq!(first.result_hash, second.result_hash);
    }

    #[test]
    fn carbon_footprint_missing_inputs_returns_insufficient_inputs() {
        let result = compute_carbon_footprint(
            carbon_footprint_request(false),
            "generated-footprint".to_string(),
            "2026-06-14T12:00:00Z".to_string(),
        )
        .expect("missing inputs should be explicit, not fatal");

        assert_eq!(result.status, CarbonFootprintStatus::InsufficientInputs);
        assert_eq!(result.value_co2e, None);
        assert!(result
            .evidence_refs
            .contains(&"factor_set:agbot-carbon-factors-v1".to_string()));
        assert!(result
            .evidence_refs
            .contains(&"input:fuel-log-001".to_string()));
        assert!(!result.result_hash.is_empty());
    }

    #[test]
    fn biomass_estimate_computes_value_and_preserves_georeference() {
        let result = estimate_biomass(
            biomass_estimate_request(),
            "generated-biomass".to_string(),
            "2026-06-15T12:00:00Z".to_string(),
        )
        .expect("matching georeferenced layers should estimate biomass");

        assert_eq!(result.estimate_id, "biomass-001");
        assert_eq!(result.record_id, "sustain-biomass-001");
        assert_eq!(result.biomass_value, 48.0);
        assert_eq!(result.area, 200.0);
        assert_eq!(result.crs, "EPSG:32614");
        assert_eq!(
            result.extent,
            GeoBounds {
                min_lon: 0.0,
                min_lat: 0.0,
                max_lon: 20.0,
                max_lat: 20.0,
            }
        );
        assert_eq!(result.resolution, RasterResolution { x: 10.0, y: 10.0 });
        assert_eq!(
            result.source_layer_refs,
            vec!["layer:canopy-height-001", "layer:ndvi-001"]
        );
        assert!(!result.result_hash.is_empty());
    }

    #[test]
    fn biomass_estimate_same_georeferenced_inputs_reproduce_hash() {
        let first = estimate_biomass(
            biomass_estimate_request(),
            "generated-biomass-001".to_string(),
            "2026-06-15T12:00:00Z".to_string(),
        )
        .expect("first biomass estimate should compute");
        let mut request = biomass_estimate_request();
        request.estimate_id = Some("biomass-002".to_string());
        let second = estimate_biomass(
            request,
            "generated-biomass-002".to_string(),
            "2026-06-16T12:00:00Z".to_string(),
        )
        .expect("second biomass estimate should compute");

        assert_eq!(first.biomass_value, second.biomass_value);
        assert_eq!(first.extent, second.extent);
        assert_eq!(first.result_hash, second.result_hash);
    }

    #[test]
    fn biomass_estimate_rejects_crs_or_extent_mismatch() {
        let mut crs_mismatch = biomass_estimate_request();
        crs_mismatch.vegetation_index_layer.spatial_ref.crs = Some("EPSG:4326".to_string());
        let error = estimate_biomass(
            crs_mismatch,
            "generated-biomass".to_string(),
            "2026-06-15T12:00:00Z".to_string(),
        )
        .expect_err("CRS mismatch should reject");
        assert_eq!(
            error,
            BiomassEstimateError::CrsMismatch {
                canopy_crs: "EPSG:32614".to_string(),
                index_crs: "EPSG:4326".to_string(),
            }
        );

        let mut extent_mismatch = biomass_estimate_request();
        extent_mismatch.vegetation_index_layer.spatial_ref.bbox = Some(GeoBounds {
            min_lon: 0.0,
            min_lat: 10.0,
            max_lon: 20.0,
            max_lat: 30.0,
        });
        extent_mismatch
            .vegetation_index_layer
            .spatial_ref
            .geo_transform = Some([0.0, 10.0, 0.0, 30.0, 0.0, -10.0]);
        let error = estimate_biomass(
            extent_mismatch,
            "generated-biomass".to_string(),
            "2026-06-15T12:00:00Z".to_string(),
        )
        .expect_err("extent mismatch should reject");
        assert_eq!(error, BiomassEstimateError::ExtentMismatch);
    }

    #[test]
    fn sustainability_baseline_comparison_returns_delta_trend_and_evidence() {
        let baseline = sustainability_baseline_record();
        let result = compare_sustainability_baseline(
            Some(&baseline),
            sustainability_comparison_request(130.0),
            "generated-comparison".to_string(),
            "2026-06-16T12:00:00Z".to_string(),
        )
        .expect("stored baseline should compare");

        assert_eq!(result.comparison_id, "comparison-001");
        assert_eq!(result.baseline_value, Some(100.0));
        assert_eq!(result.current_value, 130.0);
        assert_eq!(result.delta, Some(30.0));
        assert_eq!(result.trend, SustainabilityTrend::Increased);
        assert_eq!(result.status, SustainabilityComparisonStatus::Compared);
        assert_eq!(
            result.baseline_source_record_id.as_deref(),
            Some("sustain-baseline-2025")
        );
        assert!(result
            .evidence_refs
            .contains(&"baseline:baseline-001".to_string()));
        assert!(result
            .evidence_refs
            .contains(&"current_source:sustain-current-2026".to_string()));
    }

    #[test]
    fn sustainability_baseline_comparison_same_inputs_reproduce_hash() {
        let baseline = sustainability_baseline_record();
        let first = compare_sustainability_baseline(
            Some(&baseline),
            sustainability_comparison_request(100.0),
            "generated-comparison-001".to_string(),
            "2026-06-16T12:00:00Z".to_string(),
        )
        .expect("first comparison should compute");
        let mut request = sustainability_comparison_request(100.0);
        request.comparison_id = Some("comparison-002".to_string());
        let second = compare_sustainability_baseline(
            Some(&baseline),
            request,
            "generated-comparison-002".to_string(),
            "2026-06-17T12:00:00Z".to_string(),
        )
        .expect("second comparison should compute");

        assert_eq!(first.delta, Some(0.0));
        assert_eq!(first.trend, SustainabilityTrend::Unchanged);
        assert_eq!(first.result_hash, second.result_hash);
    }

    #[test]
    fn sustainability_baseline_comparison_without_baseline_fabricates_no_delta() {
        let result = compare_sustainability_baseline(
            None,
            sustainability_comparison_request(130.0),
            "generated-comparison".to_string(),
            "2026-06-16T12:00:00Z".to_string(),
        )
        .expect("missing baseline should return no_baseline status");

        assert_eq!(result.status, SustainabilityComparisonStatus::NoBaseline);
        assert_eq!(result.trend, SustainabilityTrend::NoBaseline);
        assert_eq!(result.baseline_value, None);
        assert_eq!(result.delta, None);
        assert!(result
            .evidence_refs
            .contains(&"current_source:sustain-current-2026".to_string()));
    }

    #[test]
    fn sustainability_kpi_thresholds_return_met_on_track_and_at_risk() {
        let met = compute_sustainability_kpi(
            sustainability_kpi_request(Some(102.0), SustainabilityKpiDirection::HigherIsBetter),
            "generated-kpi".to_string(),
            "2026-06-17T12:00:00Z".to_string(),
        )
        .expect("KPI with current value should compute");
        assert_eq!(met.status, SustainabilityKpiStatus::Met);
        assert_eq!(met.metric_ref, "water_use_efficiency");
        assert_eq!(met.current_value, Some(102.0));
        assert_eq!(met.target_value, 100.0);
        assert!(met
            .evidence_refs
            .contains(&"metric:water_use_efficiency".to_string()));
        assert!(met
            .evidence_refs
            .contains(&"sensor:water-meter-001".to_string()));

        let on_track = compute_sustainability_kpi(
            sustainability_kpi_request(Some(95.0), SustainabilityKpiDirection::HigherIsBetter),
            "generated-kpi".to_string(),
            "2026-06-17T12:00:00Z".to_string(),
        )
        .expect("KPI inside threshold band should compute");
        assert_eq!(on_track.status, SustainabilityKpiStatus::OnTrack);

        let at_risk = compute_sustainability_kpi(
            sustainability_kpi_request(Some(85.0), SustainabilityKpiDirection::HigherIsBetter),
            "generated-kpi".to_string(),
            "2026-06-17T12:00:00Z".to_string(),
        )
        .expect("KPI below threshold band should compute");
        assert_eq!(at_risk.status, SustainabilityKpiStatus::AtRisk);
        assert_ne!(on_track.result_hash, at_risk.result_hash);
    }

    #[test]
    fn sustainability_kpi_missing_current_value_returns_no_data_with_evidence() {
        let result = compute_sustainability_kpi(
            sustainability_kpi_request(None, SustainabilityKpiDirection::LowerIsBetter),
            "generated-kpi".to_string(),
            "2026-06-17T12:00:00Z".to_string(),
        )
        .expect("missing KPI current value should return no_data");

        assert_eq!(result.status, SustainabilityKpiStatus::NoData);
        assert_eq!(result.current_value, None);
        assert_eq!(result.direction, SustainabilityKpiDirection::LowerIsBetter);
        assert!(result
            .evidence_refs
            .contains(&"field:field-sustain".to_string()));
        assert!(result
            .evidence_refs
            .contains(&"season:season-2026".to_string()));
        assert!(result
            .evidence_refs
            .contains(&"sensor:water-meter-001".to_string()));
        assert!(!result.result_hash.is_empty());
    }

    #[test]
    fn sustainability_mrv_trail_requires_complete_certification_fields() {
        let trail = create_sustainability_mrv_trail(
            sustainability_mrv_request(),
            "generated-trail".to_string(),
            "2026-06-17T12:00:00Z".to_string(),
        )
        .expect("complete MRV trail should finalize");

        assert_eq!(trail.trail_id, "mrv-001");
        assert_eq!(trail.output_ref, "biomass-001");
        assert_eq!(
            trail.output_kind,
            SustainabilityMrvOutputKind::BiomassEstimate
        );
        assert_eq!(
            trail.input_layer_refs,
            vec!["layer:canopy-height-001", "layer:ndvi-001"]
        );
        assert_eq!(trail.method, "biomass_canopy_ndvi");
        assert_eq!(trail.method_version, "biomass.canopy_ndvi.v1");
        assert_eq!(trail.crs, "EPSG:32614");
        assert!(trail.certification_ready);
        assert!(!trail.rederived_result_hash.is_empty());
    }

    #[test]
    fn sustainability_mrv_trail_same_inputs_rederive_same_hash() {
        let first = create_sustainability_mrv_trail(
            sustainability_mrv_request(),
            "generated-trail-001".to_string(),
            "2026-06-17T12:00:00Z".to_string(),
        )
        .expect("first MRV trail should finalize");
        let mut request = sustainability_mrv_request();
        request.trail_id = Some("mrv-002".to_string());
        let second = create_sustainability_mrv_trail(
            request,
            "generated-trail-002".to_string(),
            "2026-06-18T12:00:00Z".to_string(),
        )
        .expect("second MRV trail should finalize");

        assert_eq!(first.rederived_result_hash, second.rederived_result_hash);
    }

    #[test]
    fn sustainability_mrv_trail_rejects_incomplete_trail() {
        let mut request = sustainability_mrv_request();
        request.input_layer_refs.clear();
        let error = create_sustainability_mrv_trail(
            request,
            "generated-trail".to_string(),
            "2026-06-17T12:00:00Z".to_string(),
        )
        .expect_err("missing input layers should reject certification trail");

        assert_eq!(error, SustainabilityMrvTrailError::EmptyInputLayerRefs);
    }

    #[test]
    fn sustainability_certification_pack_assembles_complete_claim_bundle() {
        let pack = build_sustainability_certification_evidence_pack(
            sustainability_certification_pack_request(),
            "generated-pack".to_string(),
            "2026-06-18T12:00:00Z".to_string(),
        )
        .expect("complete claimed output and MRV trail should assemble");

        assert_eq!(pack.pack_id, "cert-pack-001");
        assert_eq!(pack.claim_id, "claim-regenerative-001");
        assert_eq!(pack.claim_type, "regenerative_biomass_gain");
        assert_eq!(pack.field_id, "field-sustain");
        assert_eq!(pack.season_id, "season-2026");
        assert_eq!(pack.claimed_output_refs, vec!["biomass-001"]);
        assert_eq!(pack.outputs.len(), 1);
        assert_eq!(pack.outputs[0].result_hash, "result-hash-biomass-001");
        assert_eq!(pack.mrv_trails.len(), 1);
        assert_eq!(pack.mrv_trails[0].trail_id, "mrv-001");
        assert_eq!(pack.audit_ids, vec!["audit-biomass-001"]);
        assert!(pack
            .evidence_layer_refs
            .contains(&"layer:canopy-height-001".to_string()));
        assert!(pack
            .evidence_layer_refs
            .contains(&"layer:ndvi-001".to_string()));
        assert!(!pack.result_hash.is_empty());
        assert!(!pack.pack_hash.is_empty());

        let mut second_request = sustainability_certification_pack_request();
        second_request.pack_id = Some("cert-pack-002".to_string());
        let second = build_sustainability_certification_evidence_pack(
            second_request,
            "generated-pack-002".to_string(),
            "2026-06-19T12:00:00Z".to_string(),
        )
        .expect("same evidence should assemble deterministically");
        assert_eq!(pack.result_hash, second.result_hash);
        assert_eq!(pack.pack_hash, second.pack_hash);
    }

    #[test]
    fn sustainability_certification_pack_rejects_missing_claimed_output() {
        let mut request = sustainability_certification_pack_request();
        request.claimed_output_refs = vec!["biomass-missing".to_string()];
        let error = build_sustainability_certification_evidence_pack(
            request,
            "generated-pack".to_string(),
            "2026-06-18T12:00:00Z".to_string(),
        )
        .expect_err("claimed outputs must be backed by an output item");

        assert_eq!(
            error,
            SustainabilityCertificationEvidencePackError::MissingClaimedOutput {
                output_ref: "biomass-missing".to_string()
            }
        );
    }

    #[test]
    fn sustainability_certification_pack_rejects_incomplete_mrv_trail() {
        let mut request = sustainability_certification_pack_request();
        request.mrv_trails[0].certification_ready = false;
        let error = build_sustainability_certification_evidence_pack(
            request,
            "generated-pack".to_string(),
            "2026-06-18T12:00:00Z".to_string(),
        )
        .expect_err("non-certification-ready MRV trail must refuse generation");

        assert_eq!(
            error,
            SustainabilityCertificationEvidencePackError::IncompleteMrvTrail {
                output_ref: "biomass-001".to_string()
            }
        );
    }

    #[test]
    fn biodiversity_proxy_computes_heterogeneity_and_cover() {
        let result = compute_biodiversity_proxy(
            biodiversity_proxy_request(vec![0.1, 0.4, 0.6, 0.9]),
            "generated-proxy".to_string(),
            "2026-06-18T12:00:00Z".to_string(),
        )
        .expect("varied georeferenced imagery should compute");

        assert_eq!(result.proxy_id, "biodiversity-001");
        assert_eq!(result.field_id, "field-biodiversity");
        assert_eq!(result.status, BiodiversityProxyStatus::Computed);
        assert_eq!(result.cover_fraction, Some(0.75));
        assert!(result.heterogeneity_score.is_some_and(|score| score > 0.0));
        assert_eq!(result.crs, "EPSG:32614");
        assert_eq!(
            result.extent,
            GeoBounds {
                min_lon: 0.0,
                min_lat: 0.0,
                max_lon: 20.0,
                max_lat: 20.0,
            }
        );
        assert_eq!(result.source_layer_refs, vec!["layer:ndvi-biodiversity"]);
        assert!(!result.result_hash.is_empty());
    }

    #[test]
    fn biodiversity_proxy_same_inputs_reproduce_hash() {
        let first = compute_biodiversity_proxy(
            biodiversity_proxy_request(vec![0.1, 0.4, 0.6, 0.9]),
            "generated-proxy-001".to_string(),
            "2026-06-18T12:00:00Z".to_string(),
        )
        .expect("first biodiversity proxy should compute");
        let mut request = biodiversity_proxy_request(vec![0.1, 0.4, 0.6, 0.9]);
        request.proxy_id = Some("biodiversity-002".to_string());
        let second = compute_biodiversity_proxy(
            request,
            "generated-proxy-002".to_string(),
            "2026-06-19T12:00:00Z".to_string(),
        )
        .expect("second biodiversity proxy should compute");

        assert_eq!(first.result_hash, second.result_hash);
    }

    #[test]
    fn biodiversity_proxy_uniform_grid_returns_no_signal() {
        let result = compute_biodiversity_proxy(
            biodiversity_proxy_request(vec![0.4, 0.4, 0.4, 0.4]),
            "generated-proxy".to_string(),
            "2026-06-18T12:00:00Z".to_string(),
        )
        .expect("uniform imagery should return no_signal");

        assert_eq!(result.status, BiodiversityProxyStatus::NoSignal);
        assert_eq!(result.heterogeneity_score, None);
        assert_eq!(result.cover_fraction, None);
        assert_eq!(result.uncertainty, 1.0);
    }

    #[test]
    fn soil_carbon_proxy_computes_value_with_uncertainty_band() {
        let result = compute_soil_carbon_proxy(
            soil_carbon_proxy_request(true),
            "generated-soil-carbon".to_string(),
            "2026-06-19T12:00:00Z".to_string(),
        )
        .expect("complete soil-carbon evidence should compute");

        assert_eq!(result.proxy_id, "soil-carbon-001");
        assert_eq!(result.record_id, "sustain-soil-carbon-001");
        assert_eq!(result.field_id, "field-soil-carbon");
        assert_eq!(result.status, SoilCarbonProxyStatus::Computed);
        assert!(result.proxy_value.is_some_and(|value| value > 0.0));
        let band = result
            .uncertainty_band
            .expect("computed soil-carbon proxy must include a band");
        let value = result
            .proxy_value
            .expect("computed proxy should have value");
        assert!(band.low < value);
        assert!(band.high > value);
        assert!(result
            .evidence_refs
            .contains(&"layer:ndvi-soil-carbon".to_string()));
        assert!(result
            .evidence_refs
            .contains(&"biomass:estimate-001".to_string()));
        assert!(result
            .evidence_refs
            .contains(&"practice:cover-crop-2026".to_string()));
        assert!(!result.result_hash.is_empty());
    }

    #[test]
    fn soil_carbon_proxy_same_inputs_reproduce_hash() {
        let first = compute_soil_carbon_proxy(
            soil_carbon_proxy_request(true),
            "generated-soil-carbon-001".to_string(),
            "2026-06-19T12:00:00Z".to_string(),
        )
        .expect("first soil-carbon proxy should compute");
        let mut request = soil_carbon_proxy_request(true);
        request.proxy_id = Some("soil-carbon-002".to_string());
        let second = compute_soil_carbon_proxy(
            request,
            "generated-soil-carbon-002".to_string(),
            "2026-06-20T12:00:00Z".to_string(),
        )
        .expect("second soil-carbon proxy should compute");

        assert_eq!(first.proxy_value, second.proxy_value);
        assert_eq!(first.uncertainty_band, second.uncertainty_band);
        assert_eq!(first.result_hash, second.result_hash);
    }

    #[test]
    fn soil_carbon_proxy_insufficient_evidence_returns_unavailable_without_band() {
        let result = compute_soil_carbon_proxy(
            soil_carbon_proxy_request(false),
            "generated-soil-carbon".to_string(),
            "2026-06-19T12:00:00Z".to_string(),
        )
        .expect("insufficient evidence should be explicit, not fatal");

        assert_eq!(result.status, SoilCarbonProxyStatus::InsufficientEvidence);
        assert_eq!(result.proxy_value, None);
        assert_eq!(result.uncertainty_band, None);
        assert!(result
            .evidence_refs
            .contains(&"layer:ndvi-soil-carbon".to_string()));
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

    fn carbon_footprint_request(include_required: bool) -> CarbonFootprintComputeRequest {
        let mut inputs = vec![
            CarbonFootprintInput {
                kind: CarbonFootprintInputKind::DieselLiters,
                quantity: 10.0,
                unit: "liters".to_string(),
                evidence_ref: "input:fuel-log-001".to_string(),
            },
            CarbonFootprintInput {
                kind: CarbonFootprintInputKind::FertilizerNitrogenKg,
                quantity: 20.0,
                unit: "kg_n".to_string(),
                evidence_ref: "input:fertilizer-ticket-001".to_string(),
            },
            CarbonFootprintInput {
                kind: CarbonFootprintInputKind::ElectricityKwh,
                quantity: 15.0,
                unit: "kwh".to_string(),
                evidence_ref: "input:meter-001".to_string(),
            },
            CarbonFootprintInput {
                kind: CarbonFootprintInputKind::FieldPasses,
                quantity: 2.0,
                unit: "passes".to_string(),
                evidence_ref: "input:coverage-log-001".to_string(),
            },
        ];
        let mut factors = vec![
            CarbonEmissionFactor {
                input_kind: CarbonFootprintInputKind::DieselLiters,
                factor_kg_co2e_per_unit: 2.68,
                factor_ref: "factor:diesel:v1".to_string(),
            },
            CarbonEmissionFactor {
                input_kind: CarbonFootprintInputKind::FertilizerNitrogenKg,
                factor_kg_co2e_per_unit: 6.3,
                factor_ref: "factor:nitrogen:v1".to_string(),
            },
            CarbonEmissionFactor {
                input_kind: CarbonFootprintInputKind::ElectricityKwh,
                factor_kg_co2e_per_unit: 0.4,
                factor_ref: "factor:electricity:v1".to_string(),
            },
            CarbonEmissionFactor {
                input_kind: CarbonFootprintInputKind::FieldPasses,
                factor_kg_co2e_per_unit: 5.0,
                factor_ref: "factor:field-pass:v1".to_string(),
            },
        ];
        if !include_required {
            inputs.retain(|input| input.kind != CarbonFootprintInputKind::FieldPasses);
            factors.retain(|factor| factor.input_kind != CarbonFootprintInputKind::FieldPasses);
        }

        CarbonFootprintComputeRequest {
            footprint_id: Some("footprint-001".to_string()),
            record_id: "sustain-001".to_string(),
            operation_id: "operation-planting-001".to_string(),
            inputs,
            factor_set: CarbonFootprintFactorSet {
                version: "agbot-carbon-factors-v1".to_string(),
                factors,
            },
        }
    }

    fn sustainability_mrv_request() -> SustainabilityMrvTrailCreateRequest {
        SustainabilityMrvTrailCreateRequest {
            trail_id: Some("mrv-001".to_string()),
            output_ref: "biomass-001".to_string(),
            output_kind: SustainabilityMrvOutputKind::BiomassEstimate,
            input_layer_refs: vec![
                "layer:ndvi-001".to_string(),
                "layer:canopy-height-001".to_string(),
            ],
            method: "biomass_canopy_ndvi".to_string(),
            method_version: "biomass.canopy_ndvi.v1".to_string(),
            crs: Some("EPSG:32614".to_string()),
            extent: Some(GeoBounds {
                min_lon: 0.0,
                min_lat: 0.0,
                max_lon: 20.0,
                max_lat: 20.0,
            }),
            parameters: BTreeMap::from([
                ("biomass_coefficient".to_string(), "0.48".to_string()),
                (
                    "source_record_id".to_string(),
                    "sustain-biomass-001".to_string(),
                ),
            ]),
            audit_id: "audit-biomass-001".to_string(),
            result_hash: "result-hash-biomass-001".to_string(),
        }
    }

    fn sustainability_certification_pack_request() -> SustainabilityCertificationEvidencePackRequest
    {
        let mrv_trail = create_sustainability_mrv_trail(
            sustainability_mrv_request(),
            "generated-trail".to_string(),
            "2026-06-17T12:00:00Z".to_string(),
        )
        .expect("fixture MRV trail should be certification-ready");

        SustainabilityCertificationEvidencePackRequest {
            pack_id: Some("cert-pack-001".to_string()),
            claim_id: "claim-regenerative-001".to_string(),
            claim_type: "regenerative_biomass_gain".to_string(),
            field_id: "field-sustain".to_string(),
            season_id: "season-2026".to_string(),
            claimed_output_refs: vec!["biomass-001".to_string()],
            outputs: vec![SustainabilityCertificationOutputItem {
                output_ref: "biomass-001".to_string(),
                output_kind: SustainabilityMrvOutputKind::BiomassEstimate,
                value: Some(48.0),
                unit: Some("kg_biomass".to_string()),
                method_version: "biomass.canopy_ndvi.v1".to_string(),
                result_hash: "result-hash-biomass-001".to_string(),
            }],
            evidence_layer_refs: vec!["layer:claim-boundary-001".to_string()],
            mrv_trails: vec![mrv_trail],
            method_version: "sustainability.certification_pack.v1".to_string(),
        }
    }

    fn biodiversity_proxy_request(values: Vec<f64>) -> BiodiversityProxyRequest {
        BiodiversityProxyRequest {
            proxy_id: Some("biodiversity-001".to_string()),
            field_id: "field-biodiversity".to_string(),
            layer: BiodiversityImageryLayer {
                layer_ref: "layer:ndvi-biodiversity".to_string(),
                width: 2,
                height: 2,
                values,
                spatial_ref: RasterSpatialRef {
                    georeferenced: true,
                    crs: Some("EPSG:32614".to_string()),
                    bbox: Some(GeoBounds {
                        min_lon: 0.0,
                        min_lat: 0.0,
                        max_lon: 20.0,
                        max_lat: 20.0,
                    }),
                    geo_transform: Some([0.0, 10.0, 0.0, 20.0, 0.0, -10.0]),
                    resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
                },
            },
            method_version: "biodiversity.imagery_proxy.v1".to_string(),
            cover_threshold: 0.3,
        }
    }

    fn soil_carbon_proxy_request(include_sufficient_evidence: bool) -> SoilCarbonProxyRequest {
        let mut request = SoilCarbonProxyRequest {
            proxy_id: Some("soil-carbon-001".to_string()),
            record_id: "sustain-soil-carbon-001".to_string(),
            field_id: "field-soil-carbon".to_string(),
            index_inputs: vec![SoilCarbonEvidenceInput {
                evidence_ref: "layer:ndvi-soil-carbon".to_string(),
                value: 3.2,
                weight: 0.8,
            }],
            biomass_inputs: vec![SoilCarbonEvidenceInput {
                evidence_ref: "biomass:estimate-001".to_string(),
                value: 5.6,
                weight: 1.2,
            }],
            practice_inputs: vec![SoilCarbonPracticeInput {
                practice_ref: "practice:cover-crop-2026".to_string(),
                carbon_delta: 0.7,
            }],
            method_version: "soil_carbon.proxy.v1".to_string(),
        };
        if !include_sufficient_evidence {
            request.biomass_inputs.clear();
            request.practice_inputs.clear();
        }
        request
    }

    fn sustainability_baseline_record() -> SustainabilityBaselineRecord {
        create_sustainability_baseline(
            SustainabilityBaselineCreateRequest {
                baseline_id: Some("baseline-001".to_string()),
                field_id: "field-sustain".to_string(),
                season_id: "season-2025".to_string(),
                metric_type: SustainabilityMetricType::Biomass,
                metric_value: 100.0,
                source_record_id: "sustain-baseline-2025".to_string(),
                method_version: "sustainability.baseline.v1".to_string(),
                evidence_refs: vec!["biomass:baseline-2025".to_string()],
            },
            "generated-baseline".to_string(),
            "2026-06-16T00:00:00Z".to_string(),
        )
        .expect("baseline should normalize")
    }

    fn sustainability_comparison_request(current_value: f64) -> SustainabilityComparisonRequest {
        SustainabilityComparisonRequest {
            comparison_id: Some("comparison-001".to_string()),
            field_id: "field-sustain".to_string(),
            baseline_season_id: "season-2025".to_string(),
            current_season_id: "season-2026".to_string(),
            metric_type: SustainabilityMetricType::Biomass,
            current_value,
            current_source_record_id: "sustain-current-2026".to_string(),
            method_version: "sustainability.baseline_compare.v1".to_string(),
        }
    }

    fn sustainability_kpi_request(
        current_value: Option<f64>,
        direction: SustainabilityKpiDirection,
    ) -> SustainabilityKpiTrackingRequest {
        SustainabilityKpiTrackingRequest {
            kpi_id: Some("kpi-001".to_string()),
            field_id: "field-sustain".to_string(),
            season_id: "season-2026".to_string(),
            metric_ref: "water_use_efficiency".to_string(),
            current_value,
            target_value: 100.0,
            direction,
            at_risk_fraction: 0.9,
            method_version: "sustainability.kpi.v1".to_string(),
            evidence_refs: vec![
                "sensor:water-meter-001".to_string(),
                " metric:water_use_efficiency ".to_string(),
            ],
        }
    }

    fn biomass_estimate_request() -> BiomassEstimateRequest {
        BiomassEstimateRequest {
            estimate_id: Some("biomass-001".to_string()),
            record_id: "sustain-biomass-001".to_string(),
            canopy_layer: biomass_layer("layer:canopy-height-001", vec![1.0, 2.0, 0.0, 4.0]),
            vegetation_index_layer: biomass_layer("layer:ndvi-001", vec![0.5, 0.25, 0.8, -0.1]),
            method_version: "biomass.canopy_ndvi.v1".to_string(),
            biomass_coefficient: 0.48,
        }
    }

    fn biomass_layer(layer_ref: &str, values: Vec<f64>) -> BiomassLayerInput {
        BiomassLayerInput {
            layer_ref: layer_ref.to_string(),
            width: 2,
            height: 2,
            values,
            spatial_ref: RasterSpatialRef {
                georeferenced: true,
                crs: Some("EPSG:32614".to_string()),
                bbox: Some(GeoBounds {
                    min_lon: 0.0,
                    min_lat: 0.0,
                    max_lon: 20.0,
                    max_lat: 20.0,
                }),
                geo_transform: Some([0.0, 10.0, 0.0, 20.0, 0.0, -10.0]),
                resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
            },
        }
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
    fn weather_field_forecast_resolution_keys_records_on_field_boundary() {
        let resolution =
            resolve_weather_forecast_to_field(weather_field_resolution_request(Some((
                tractor_swath_rectangle(0.0, 0.0, 10.0, 10.0, "EPSG:4326"),
                GeoPoint {
                    longitude: 5.0,
                    latitude: 5.0,
                },
                "EPSG:4326",
            ))))
            .expect("forecast location inside field should resolve");

        assert_eq!(resolution.field_id, "field-north");
        assert_eq!(resolution.field_crs, "EPSG:4326");
        assert_eq!(resolution.records.len(), 1);
        assert_eq!(resolution.records[0].field_ref, "field-north");
        assert_eq!(resolution.field_centroid.longitude, 5.0);
        assert!(resolution
            .evidence_refs
            .contains(&"forecast_location:inside_field".to_string()));
    }

    #[test]
    fn weather_field_forecast_resolution_requires_field_geometry() {
        let error = resolve_weather_forecast_to_field(weather_field_resolution_request(None))
            .expect_err("missing field boundary must fail explicitly");

        assert_eq!(error, WeatherFieldForecastResolutionError::NoFieldGeometry);
    }

    #[test]
    fn weather_field_forecast_resolution_rejects_crs_or_location_mismatch() {
        let crs_error =
            resolve_weather_forecast_to_field(weather_field_resolution_request(Some((
                tractor_swath_rectangle(0.0, 0.0, 10.0, 10.0, "EPSG:4326"),
                GeoPoint {
                    longitude: 5.0,
                    latitude: 5.0,
                },
                "EPSG:3857",
            ))))
            .expect_err("forecast CRS must match field CRS");

        assert_eq!(
            crs_error,
            WeatherFieldForecastResolutionError::CrsMismatch {
                forecast_crs: "EPSG:3857".to_string(),
                field_crs: "EPSG:4326".to_string()
            }
        );

        let outside_error =
            resolve_weather_forecast_to_field(weather_field_resolution_request(Some((
                tractor_swath_rectangle(0.0, 0.0, 10.0, 10.0, "EPSG:4326"),
                GeoPoint {
                    longitude: 20.0,
                    latitude: 20.0,
                },
                "EPSG:4326",
            ))))
            .expect_err("forecast location outside field must fail");

        assert_eq!(
            outside_error,
            WeatherFieldForecastResolutionError::ForecastOutsideField
        );
    }

    #[test]
    fn weather_freshness_marks_recent_value_fresh_with_age() {
        let annotated = evaluate_weather_value_freshness(
            weather_forecast_value("2026-06-13T10:00:00Z", "2026-06-13T11:00:00Z"),
            "2026-06-13T10:10:00Z",
            900,
        )
        .expect("fresh weather value should annotate");

        assert_eq!(annotated.freshness_state, WeatherFreshnessState::Fresh);
        assert_eq!(annotated.age_seconds, 600);
        assert_eq!(annotated.value.source, "NOAA-HRRR");
        assert!(!annotated.stale);
    }

    #[test]
    fn weather_freshness_marks_aged_value_stale() {
        let annotated = evaluate_weather_value_freshness(
            weather_forecast_value("2026-06-13T10:00:00Z", "2026-06-13T11:00:00Z"),
            "2026-06-13T10:30:01Z",
            1800,
        )
        .expect("stale weather value should annotate");

        assert_eq!(annotated.freshness_state, WeatherFreshnessState::Stale);
        assert_eq!(annotated.age_seconds, 1801);
        assert_eq!(annotated.stale_after_seconds, 1800);
        assert!(annotated.stale);
    }

    #[test]
    fn weather_record_freshness_propagates_stale_flag_downstream() {
        let mut request = weather_field_resolution_request(Some((
            tractor_swath_rectangle(0.0, 0.0, 10.0, 10.0, "EPSG:4326"),
            GeoPoint {
                longitude: 5.0,
                latitude: 5.0,
            },
            "EPSG:4326",
        )));
        let record = request.records.remove(0);

        let annotated = annotate_weather_record_freshness(record, "2026-06-13T10:30:01Z", 1800)
            .expect("weather record should annotate freshness");

        assert!(annotated.stale);
        assert_eq!(
            annotated.wind_speed_mps.freshness_state,
            WeatherFreshnessState::Stale
        );
        assert_eq!(annotated.wind_speed_mps.value.value, 4.2);
        assert_eq!(annotated.source, "NOAA-HRRR");
    }

    #[test]
    fn weather_sensor_stream_ingests_samples_with_provenance_and_freshness() {
        let ingest = ingest_weather_sensor_stream(weather_sensor_stream_request(vec![
            weather_sensor_sample("2026-06-13T10:00:00Z", 22.5),
            weather_sensor_sample("2026-06-13T10:05:00Z", 23.0),
        ]))
        .expect("sensor stream should ingest");

        assert_eq!(ingest.sensor_id, "sensor-north-001");
        assert_eq!(ingest.field_ref, "field-north");
        assert_eq!(ingest.source, "sensor");
        assert_eq!(ingest.sample_count, 2);
        assert!(ingest.gap_events.is_empty());
        assert!(!ingest.stale);
        assert_eq!(ingest.records[0].source, "sensor");
        assert_eq!(ingest.records[0].vars.temperature_celsius.source, "sensor");
        assert_eq!(
            ingest.freshness[0].temperature_celsius.freshness_state,
            WeatherFreshnessState::Fresh
        );
    }

    #[test]
    fn weather_sensor_stream_records_gap_and_stale_state() {
        let ingest = ingest_weather_sensor_stream(weather_sensor_stream_request(vec![
            weather_sensor_sample("2026-06-13T10:00:00Z", 22.5),
            weather_sensor_sample("2026-06-13T10:30:00Z", 23.0),
        ]))
        .expect("sensor gap should ingest with event");

        assert!(ingest.stale);
        assert_eq!(ingest.gap_events.len(), 1);
        assert_eq!(ingest.gap_events[0].reason_code, "sensor_stream_gap");
        assert_eq!(ingest.gap_events[0].gap_seconds, 1800);
    }

    #[test]
    fn weather_sensor_stream_rejects_invalid_sample_without_partial_ingest() {
        let error = ingest_weather_sensor_stream(weather_sensor_stream_request(vec![
            weather_sensor_sample("2026-06-13T10:00:00Z", 22.5),
            WeatherSensorSample {
                observed_at: "2026-06-13T10:05:00Z".to_string(),
                temperature_celsius: 23.0,
                wind_speed_mps: -1.0,
                precipitation_mm: 0.0,
                humidity_percent: 64.0,
                radiation_w_m2: 720.0,
            },
        ]))
        .expect_err("invalid sample should reject stream");

        assert_eq!(
            error,
            WeatherSensorIngestError::Weather(WeatherIngestError::InvalidValue {
                variable: "wind_speed_mps".to_string(),
                value: "-1".to_string()
            })
        );
    }

    #[test]
    fn weather_history_query_returns_field_range_with_freshness() {
        let ingest = ingest_weather_sensor_stream(weather_sensor_stream_request(vec![
            weather_sensor_sample("2026-06-13T10:00:00Z", 22.5),
            weather_sensor_sample("2026-06-13T10:05:00Z", 23.0),
        ]))
        .expect("history fixture should ingest");
        let history = append_weather_history_records(Vec::new(), ingest.freshness);

        let result = query_weather_history(
            &history,
            weather_history_query(
                "field-north",
                "2026-06-13T09:59:00Z",
                "2026-06-13T10:06:00Z",
            ),
        )
        .expect("history query should succeed");

        assert!(!result.empty);
        assert_eq!(result.total_count, 2);
        assert_eq!(result.records[0].sequence, 1);
        assert_eq!(result.records[0].record.source, "sensor");
        assert_eq!(
            result.records[0].record.temperature_celsius.freshness_state,
            WeatherFreshnessState::Fresh
        );
    }

    #[test]
    fn weather_history_query_paginates_append_only_order() {
        let ingest = ingest_weather_sensor_stream(weather_sensor_stream_request(vec![
            weather_sensor_sample("2026-06-13T10:00:00Z", 22.5),
            weather_sensor_sample("2026-06-13T10:05:00Z", 23.0),
        ]))
        .expect("history fixture should ingest");
        let history = append_weather_history_records(Vec::new(), ingest.freshness);
        let mut query = weather_history_query(
            "field-north",
            "2026-06-13T09:59:00Z",
            "2026-06-13T10:06:00Z",
        );
        query.offset = 1;
        query.limit = 1;

        let result = query_weather_history(&history, query).expect("paginated history query works");

        assert_eq!(result.total_count, 2);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].sequence, 2);
        assert_eq!(result.records[0].record.valid_time, "2026-06-13T10:05:00Z");
    }

    #[test]
    fn weather_history_query_empty_field_returns_empty_result() {
        let result = query_weather_history(
            &[],
            weather_history_query(
                "field-north",
                "2026-06-13T09:59:00Z",
                "2026-06-13T10:06:00Z",
            ),
        )
        .expect("empty history is a valid result");

        assert!(result.empty);
        assert_eq!(result.total_count, 0);
        assert!(result.records.is_empty());
    }

    #[test]
    fn weather_operational_window_advisor_emits_fresh_safe_window() {
        let report = advise_weather_operational_windows(weather_window_request(vec![
            weather_window_record("2026-06-13T10:00:00Z", 3.0, 0.0, 22.0, false),
            weather_window_record("2026-06-13T10:15:00Z", 4.0, 0.1, 23.0, false),
        ]))
        .expect("safe forecast should advise a window");

        assert!(report.gaps.is_empty());
        assert_eq!(report.windows.len(), 1);
        assert_eq!(report.windows[0].start, "2026-06-13T10:00:00Z");
        assert_eq!(report.windows[0].end, "2026-06-13T10:15:00Z");
        assert!(report.windows[0]
            .gating_vars
            .contains(&"wind_speed_mps".to_string()));
        assert!(report.windows[0]
            .thresholds
            .contains(&"max_precipitation_mm:0.5".to_string()));
        assert_eq!(
            report.windows[0].freshness,
            vec![WeatherFreshnessState::Fresh]
        );
    }

    #[test]
    fn weather_operational_window_advisor_reports_threshold_gap() {
        let report = advise_weather_operational_windows(weather_window_request(vec![
            weather_window_record("2026-06-13T10:00:00Z", 3.0, 0.0, 22.0, false),
            weather_window_record("2026-06-13T10:15:00Z", 12.0, 0.0, 22.0, false),
        ]))
        .expect("threshold gap should evaluate");

        assert_eq!(report.windows.len(), 1);
        assert_eq!(report.gaps.len(), 1);
        assert_eq!(report.gaps[0].reason_code, "threshold_exceeded");
        assert!(report.gaps[0].details.contains("wind_speed_mps:12>6"));
    }

    #[test]
    fn weather_operational_window_advisor_blocks_stale_or_missing_inputs() {
        let stale = advise_weather_operational_windows(weather_window_request(vec![
            weather_window_record("2026-06-13T10:00:00Z", 3.0, 0.0, 22.0, true),
        ]))
        .expect("stale forecast should evaluate");

        assert!(stale.windows.is_empty());
        assert_eq!(stale.gaps[0].reason_code, "stale_forecast_input");

        let missing = advise_weather_operational_windows(weather_window_request(Vec::new()))
            .expect("missing forecast should evaluate");

        assert!(missing.windows.is_empty());
        assert_eq!(missing.gaps[0].reason_code, "missing_forecast_inputs");
    }

    #[test]
    fn weather_risk_alerts_raise_threshold_breaches_with_evidence() {
        let alerts = evaluate_weather_risk_alerts(
            &[
                weather_window_record("2026-06-13T04:00:00Z", 3.0, 0.0, 0.5, false),
                weather_window_record("2026-06-13T14:00:00Z", 12.0, 2.0, 36.0, true),
            ],
            weather_risk_thresholds(),
        )
        .expect("risk alerts should evaluate");

        assert!(alerts
            .iter()
            .any(|alert| alert.risk_type == WeatherRiskType::Frost
                && alert.value == 0.5
                && alert.threshold == 2.0
                && alert.source == "NOAA-HRRR"
                && alert.freshness == WeatherFreshnessState::Fresh));
        assert!(alerts
            .iter()
            .any(|alert| alert.risk_type == WeatherRiskType::Heat
                && alert.value == 36.0
                && alert.threshold == 35.0
                && alert.freshness == WeatherFreshnessState::Stale));
        assert!(alerts
            .iter()
            .any(|alert| alert.risk_type == WeatherRiskType::Wind && alert.value == 12.0));
        assert!(alerts
            .iter()
            .any(|alert| alert.risk_type == WeatherRiskType::Precipitation && alert.value == 2.0));
    }

    #[test]
    fn weather_risk_alerts_do_not_raise_false_alarm_within_thresholds() {
        let alerts = evaluate_weather_risk_alerts(
            &[weather_window_record(
                "2026-06-13T10:00:00Z",
                3.0,
                0.0,
                22.0,
                false,
            )],
            weather_risk_thresholds(),
        )
        .expect("safe weather should evaluate");

        assert!(alerts.is_empty());
    }

    #[test]
    fn weather_gdd_computes_known_day_with_method_and_base() {
        let gdd = compute_weather_growing_degree_day(weather_gdd_request(
            "2026-06-13",
            vec![
                weather_window_record("2026-06-13T06:00:00Z", 3.0, 0.0, 10.0, false),
                weather_window_record("2026-06-13T15:00:00Z", 3.0, 0.0, 30.0, false),
            ],
        ))
        .expect("GDD should compute");

        assert_eq!(gdd.status, WeatherGrowingDegreeDayStatus::Computed);
        assert_eq!(gdd.gdd_celsius_days, Some(10.0));
        assert_eq!(gdd.min_temperature_celsius, Some(10.0));
        assert_eq!(gdd.max_temperature_celsius, Some(30.0));
        assert_eq!(gdd.base_temperature_celsius, 10.0);
        assert_eq!(gdd.method, "simple_average_max_min_minus_base_celsius");
        assert_eq!(gdd.evidence_refs.len(), 2);
    }

    #[test]
    fn weather_gdd_marks_missing_day_no_data() {
        let gdd = compute_weather_growing_degree_day(weather_gdd_request(
            "2026-06-14",
            vec![weather_window_record(
                "2026-06-13T06:00:00Z",
                3.0,
                0.0,
                10.0,
                false,
            )],
        ))
        .expect("missing day should evaluate");

        assert_eq!(gdd.status, WeatherGrowingDegreeDayStatus::NoData);
        assert_eq!(gdd.gdd_celsius_days, None);
        assert_eq!(gdd.min_temperature_celsius, None);
        assert_eq!(gdd.max_temperature_celsius, None);
        assert_eq!(gdd.evidence_refs, vec!["temperature:no_data".to_string()]);
    }

    #[test]
    fn weather_reference_et_computes_known_case_with_method_and_inputs() {
        let et = compute_weather_reference_et(weather_reference_et_input(true))
            .expect("complete ET inputs should compute");

        assert_eq!(et.status, WeatherReferenceEtStatus::Computed);
        assert_eq!(et.reference_et_mm_day, Some(7.47496));
        assert_eq!(
            et.method,
            "agbot_reference_et_v1_radiation_temperature_humidity_wind"
        );
        assert!(et
            .input_refs
            .contains(&"temperature_celsius:2026-06-13T10:00:00Z".to_string()));
        assert_eq!(et.freshness.len(), 4);
    }

    #[test]
    fn weather_reference_et_reports_insufficient_inputs() {
        let et = compute_weather_reference_et(weather_reference_et_input(false))
            .expect("missing ET inputs should produce insufficient-input result");

        assert_eq!(et.status, WeatherReferenceEtStatus::InsufficientInputs);
        assert_eq!(et.reference_et_mm_day, None);
        assert_eq!(et.input_refs, vec!["missing:radiation_w_m2".to_string()]);
    }

    #[test]
    fn water_weather_input_contract_validates_complete_fresh_inputs() {
        let contract =
            validate_water_weather_input_contract(water_weather_input_contract_request(false))
                .expect("fresh weather input contract should validate");

        assert_eq!(contract.field_ref, "field-north");
        assert_eq!(contract.date, "2026-06-13");
        assert_eq!(contract.status, WaterWeatherInputStatus::Valid);
        assert!(!contract.et_blocked);
        assert!(contract.degradation_reasons.is_empty());
        assert_eq!(
            contract
                .temperature_celsius
                .as_ref()
                .expect("temperature is present")
                .value
                .value,
            22.0
        );
        assert_eq!(
            contract
                .precipitation_mm
                .as_ref()
                .expect("precipitation is present")
                .value
                .value,
            0.0
        );
        assert!(contract
            .evidence_refs
            .iter()
            .any(|evidence| evidence.starts_with("weather_record:weather:field-north")));
    }

    #[test]
    fn water_weather_input_contract_degrades_stale_inputs() {
        let contract =
            validate_water_weather_input_contract(water_weather_input_contract_request(true))
                .expect("stale weather input contract should degrade");

        assert_eq!(contract.status, WaterWeatherInputStatus::Degraded);
        assert!(contract.et_blocked);
        assert_eq!(
            contract.degradation_reasons,
            vec!["stale_weather_input".to_string()]
        );
        assert!(contract.temperature_celsius.is_some());
    }

    #[test]
    fn water_weather_input_contract_degrades_missing_inputs() {
        let contract = validate_water_weather_input_contract(WaterWeatherInputContractRequest {
            field_ref: "field-north".to_string(),
            date: "2026-06-13".to_string(),
            records: Vec::new(),
        })
        .expect("missing weather input should degrade, not error");

        assert_eq!(contract.status, WaterWeatherInputStatus::Degraded);
        assert!(contract.et_blocked);
        assert_eq!(
            contract.degradation_reasons,
            vec!["missing_weather_input".to_string()]
        );
        assert!(contract.temperature_celsius.is_none());
        assert!(contract.evidence_refs.is_empty());
    }

    #[test]
    fn water_evapotranspiration_computes_reference_and_crop_et() {
        let weather =
            validate_water_weather_input_contract(water_weather_input_contract_request(false))
                .expect("fresh weather input contract should validate");

        let et = compute_water_evapotranspiration(WaterEvapotranspirationRequest {
            weather,
            crop_coefficient: Some(1.15),
        })
        .expect("water ET should compute");

        assert_eq!(et.status, WaterEvapotranspirationStatus::Computed);
        assert_eq!(et.reference_et_mm_day, Some(7.47496));
        assert_eq!(et.crop_et_mm_day, Some(8.596204));
        assert_eq!(et.crop_coefficient, 1.15);
        assert_eq!(et.method, "agbot_water_et_v1_reference_et_crop_coefficient");
        assert!(et
            .input_refs
            .contains(&"temperature_celsius:2026-06-13T10:00:00Z".to_string()));
        assert!(et.degradation_reasons.is_empty());
    }

    #[test]
    fn water_evapotranspiration_reports_insufficient_inputs() {
        let weather = validate_water_weather_input_contract(WaterWeatherInputContractRequest {
            field_ref: "field-north".to_string(),
            date: "2026-06-13".to_string(),
            records: Vec::new(),
        })
        .expect("missing weather input should degrade");

        let et = compute_water_evapotranspiration(WaterEvapotranspirationRequest {
            weather,
            crop_coefficient: None,
        })
        .expect("insufficient weather input should not error");

        assert_eq!(et.status, WaterEvapotranspirationStatus::InsufficientInputs);
        assert_eq!(et.reference_et_mm_day, None);
        assert_eq!(et.crop_et_mm_day, None);
        assert_eq!(et.crop_coefficient, 1.0);
        assert_eq!(
            et.degradation_reasons,
            vec!["missing_weather_input".to_string()]
        );
    }

    #[test]
    fn zone_water_need_maps_proxy_moisture_and_et_per_zone() {
        let request = zone_water_need_request(false);

        let needs = map_zone_water_need(request).expect("zone water need should map");

        assert_eq!(needs.len(), 2);
        assert_eq!(needs[0].zone_ref, "zone:north");
        assert_eq!(needs[0].status, ZoneWaterNeedStatus::Computed);
        assert!((needs[0].water_need_mm.expect("need should compute") - 2.49289916).abs() < 1e-6);
        assert_eq!(needs[0].crs, "EPSG:4326");
        assert_eq!(needs[0].reason_code, "computed_from_moisture_and_et");
        assert!(needs[0]
            .evidence_refs
            .iter()
            .any(|evidence| evidence.starts_with("moisture_proxy:moisture-proxy")));
        assert!(needs[0]
            .evidence_refs
            .contains(&"temperature_celsius:2026-06-13T10:00:00Z".to_string()));
        assert_eq!(needs[1].zone_ref, "zone:missing");
        assert_eq!(needs[1].status, ZoneWaterNeedStatus::InsufficientEvidence);
        assert_eq!(needs[1].water_need_mm, None);
        assert_eq!(needs[1].reason_code, "missing_moisture_evidence");
    }

    #[test]
    fn zone_water_need_rejects_zone_crs_mismatch() {
        let error = map_zone_water_need(zone_water_need_request(true))
            .expect_err("zone CRS mismatch should reject");

        assert_eq!(
            error,
            ZoneWaterNeedError::CrsMismatch {
                zone_ref: "zone:north".to_string(),
                zone_crs: "EPSG:3857".to_string(),
                request_crs: "EPSG:4326".to_string()
            }
        );
    }

    #[test]
    fn irrigation_schedule_generates_sequential_plan_from_water_need() {
        let needs = map_zone_water_need(zone_water_need_request(false))
            .expect("zone water need should map");

        let schedule = schedule_irrigation_plan(irrigation_schedule_request(needs))
            .expect("computed water needs should schedule");

        assert_eq!(schedule.field_ref, "field-north");
        assert_eq!(
            schedule.method,
            "agbot_irrigation_schedule_v1_sequential_zone_water_need"
        );
        assert_eq!(schedule.entries.len(), 1);
        assert_eq!(schedule.entries[0].zone_ref, "zone:north");
        assert!((schedule.entries[0].amount_mm - 2.49289916).abs() < 1e-6);
        assert_eq!(schedule.entries[0].start_time, "2026-06-13T12:00:00Z");
        assert_eq!(schedule.entries[0].duration_minutes, 30);
        assert!(schedule.entries[0]
            .evidence_refs
            .iter()
            .any(|evidence| evidence.starts_with("moisture_proxy:")));
        assert_eq!(schedule.exclusions.len(), 1);
        assert_eq!(schedule.exclusions[0].zone_ref, "zone:missing");
        assert_eq!(
            schedule.exclusions[0].reason_code,
            "missing_moisture_evidence"
        );
    }

    #[test]
    fn irrigation_schedule_excludes_insufficient_evidence_zones() {
        let schedule = schedule_irrigation_plan(irrigation_schedule_request(vec![
            zone_water_need_insufficient(
                "field-north",
                "zone:dry-run-blocked",
                "EPSG:4326",
                "missing_et_evidence",
            ),
        ]))
        .expect("insufficient evidence zones should be excluded");

        assert!(schedule.entries.is_empty());
        assert_eq!(schedule.exclusions.len(), 1);
        assert_eq!(schedule.exclusions[0].zone_ref, "zone:dry-run-blocked");
        assert_eq!(schedule.exclusions[0].reason_code, "missing_et_evidence");
    }

    #[test]
    fn irrigation_valve_dry_run_and_execute_apply_bounded_plan() {
        let schedule = irrigation_schedule_fixture();
        let dry_run =
            dry_run_irrigation_valve_plan(irrigation_valve_dry_run_request(schedule.clone(), 10.0))
                .expect("bounded valve dry-run should pass");

        assert_eq!(dry_run.status, IrrigationValveDryRunStatus::Passed);
        assert_eq!(dry_run.audits.len(), 1);
        assert_eq!(
            dry_run.audits[0].status,
            IrrigationValveActionStatus::Planned
        );
        assert_eq!(dry_run.audits[0].reason_code, "dry_run_passed");

        let execution = execute_irrigation_valve_plan(irrigation_valve_execute_request(
            schedule, dry_run, false,
        ))
        .expect("passing dry-run should execute");

        assert_eq!(execution.status, IrrigationValveExecutionStatus::Applied);
        assert_eq!(execution.audits.len(), 1);
        assert_eq!(
            execution.audits[0].status,
            IrrigationValveActionStatus::Applied
        );
        assert_eq!(execution.audits[0].reason_code, "valve_action_applied");
    }

    #[test]
    fn irrigation_valve_execute_refuses_out_of_bounds_or_unpassed_dry_run() {
        let schedule = irrigation_schedule_fixture();
        let dry_run =
            dry_run_irrigation_valve_plan(irrigation_valve_dry_run_request(schedule.clone(), 1.0))
                .expect("out-of-bounds dry-run should still audit");

        assert_eq!(dry_run.status, IrrigationValveDryRunStatus::Rejected);
        assert_eq!(
            dry_run.audits[0].status,
            IrrigationValveActionStatus::Rejected
        );
        assert_eq!(dry_run.audits[0].reason_code, "valve_bounds_exceeded");

        let execution = execute_irrigation_valve_plan(irrigation_valve_execute_request(
            schedule, dry_run, false,
        ))
        .expect("failed dry-run should refuse execution with audit");

        assert_eq!(execution.status, IrrigationValveExecutionStatus::Rejected);
        assert_eq!(
            execution.audits[0].status,
            IrrigationValveActionStatus::Rejected
        );
        assert_eq!(execution.audits[0].reason_code, "dry_run_not_passed");
    }

    #[test]
    fn weather_alert_routing_delivers_owned_field_to_console_and_portal() {
        let result = route_weather_alert(weather_alert_routing_request(vec![
            WeatherAlertRoutingTarget {
                target: WeatherAlertRouteTarget::OperatorConsole,
                reachable: true,
            },
            WeatherAlertRoutingTarget {
                target: WeatherAlertRouteTarget::FarmersPortal,
                reachable: true,
            },
        ]))
        .expect("owned alert should route");

        assert_eq!(result.delivered_count, 2);
        assert_eq!(result.queued_count, 0);
        assert!(result.audits.iter().all(|audit| {
            audit.status == WeatherAlertDeliveryStatus::Delivered
                && audit.reason_code == "alert_delivered"
                && audit.field_ref == "field-north"
                && audit.evidence_payload.contains(&"threshold:10".to_string())
        }));
    }

    #[test]
    fn weather_alert_routing_queues_unreachable_target() {
        let result = route_weather_alert(weather_alert_routing_request(vec![
            WeatherAlertRoutingTarget {
                target: WeatherAlertRouteTarget::OperatorConsole,
                reachable: false,
            },
        ]))
        .expect("unreachable target should queue");

        assert_eq!(result.delivered_count, 0);
        assert_eq!(result.queued_count, 1);
        assert_eq!(result.audits[0].status, WeatherAlertDeliveryStatus::Queued);
        assert_eq!(result.audits[0].reason_code, "target_unreachable_queued");
    }

    #[test]
    fn weather_alert_routing_rejects_unowned_field_scope() {
        let mut request = weather_alert_routing_request(vec![WeatherAlertRoutingTarget {
            target: WeatherAlertRouteTarget::FarmersPortal,
            reachable: true,
        }]);
        request.owned_field_refs = vec!["field-south".to_string()];

        let result = route_weather_alert(request).expect("unowned field should audit rejection");

        assert_eq!(result.delivered_count, 0);
        assert_eq!(result.rejected_count, 1);
        assert_eq!(
            result.audits[0].status,
            WeatherAlertDeliveryStatus::Rejected
        );
        assert_eq!(result.audits[0].reason_code, "field_scope_not_owned");
    }

    #[test]
    fn weather_crop_stage_risk_applies_sensitive_stage_threshold() {
        let alerts =
            evaluate_crop_stage_weather_risks(weather_crop_stage_request(Some("flowering")))
                .expect("stage-aware alert should evaluate");

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].crop_stage, "flowering");
        assert_eq!(alerts[0].threshold_set_name, "flowering_frost_sensitive");
        assert!(!alerts[0].fallback_applied);
        assert_eq!(alerts[0].alert.risk_type, WeatherRiskType::Frost);
        assert_eq!(alerts[0].alert.threshold, 5.0);
    }

    #[test]
    fn weather_crop_stage_risk_unknown_stage_uses_default_thresholds() {
        let mut request = weather_crop_stage_request(Some("unknown-stage"));
        request.records = vec![weather_window_record(
            "2026-06-13T04:00:00Z",
            3.0,
            0.0,
            1.0,
            false,
        )];
        let alerts = evaluate_crop_stage_weather_risks(request)
            .expect("unknown stage should evaluate with fallback");

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].crop_stage, "unknown-stage");
        assert_eq!(alerts[0].threshold_set_name, "default_thresholds");
        assert!(alerts[0].fallback_applied);
        assert_eq!(alerts[0].alert.threshold, 2.0);
    }

    #[test]
    fn weather_forecast_verification_computes_error_metrics_for_matching_observation() {
        let verification =
            verify_weather_forecast_accuracy(weather_forecast_verification_request(true));

        assert_eq!(
            verification.status,
            WeatherForecastVerificationStatus::Verified
        );
        assert_eq!(verification.field_ref, "field-north");
        assert_eq!(verification.source, "NOAA-HRRR");
        assert_eq!(verification.valid_time, "2026-06-13T10:00:00Z");
        assert_eq!(verification.metrics.len(), 5);
        let temperature = verification
            .metrics
            .iter()
            .find(|metric| metric.variable == "temperature_celsius")
            .expect("temperature metric should be present");
        assert_eq!(temperature.forecast_value, 22.0);
        assert_eq!(temperature.observed_value, 20.5);
        assert_eq!(temperature.absolute_error, 1.5);
        assert_eq!(temperature.unit, "deg_c");
        assert!(verification
            .evidence_refs
            .iter()
            .any(|evidence| evidence.starts_with("forecast:weather:field-north:NOAA-HRRR")));
        assert!(verification
            .evidence_refs
            .iter()
            .any(|evidence| evidence.starts_with("observation:weather:field-north:sensor")));
    }

    #[test]
    fn weather_forecast_verification_reports_not_verifiable_without_observation() {
        let verification =
            verify_weather_forecast_accuracy(weather_forecast_verification_request(false));

        assert_eq!(
            verification.status,
            WeatherForecastVerificationStatus::NotVerifiable
        );
        assert_eq!(verification.field_ref, "field-north");
        assert_eq!(verification.source, "NOAA-HRRR");
        assert_eq!(verification.valid_time, "2026-06-13T10:00:00Z");
        assert!(verification.metrics.is_empty());
        assert_eq!(
            verification.evidence_refs,
            vec!["observation:not_found".to_string()]
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
    fn remote_sensing_moisture_proxy_ingests_zone_values_as_proxy_evidence() {
        let field = tractor_test_farm_fields()
            .field_by_id("field-north")
            .expect("test field exists");

        let records =
            ingest_remote_sensing_moisture_proxy_layer(moisture_proxy_layer("EPSG:4326"), &field)
                .expect("matching NDMI layer should ingest");

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].field_id, "field-north");
        assert_eq!(records[0].zone_ref, "zone:north");
        assert_eq!(records[0].value, 0.42);
        assert_eq!(records[0].index, RemoteSensingMoistureIndex::Ndmi);
        assert_eq!(records[0].source, "imagery_processor");
        assert_eq!(records[0].captured_at, "2026-06-13T10:00:00Z");
        assert_eq!(records[0].evidence_kind, "proxy");
        assert_eq!(records[0].crs, "EPSG:4326");
        assert_eq!(records[0].extent, field.extent);
        assert!(records[0]
            .proxy_id
            .starts_with("moisture-proxy:field-north:zone-north:ndmi"));
        assert_eq!(
            records[0].evidence_refs,
            vec!["layer:layer-ndmi-field-north".to_string()]
        );
    }

    #[test]
    fn remote_sensing_moisture_proxy_rejects_crs_mismatch_without_records() {
        let field = tractor_test_farm_fields()
            .field_by_id("field-north")
            .expect("test field exists");

        let error =
            ingest_remote_sensing_moisture_proxy_layer(moisture_proxy_layer("EPSG:3857"), &field)
                .expect_err("mismatched CRS should refuse proxy ingest");

        assert_eq!(
            error,
            RemoteSensingMoistureProxyError::CrsMismatch {
                layer_crs: "EPSG:3857".to_string(),
                field_crs: "EPSG:4326".to_string()
            }
        );
    }

    #[test]
    fn irrigation_history_query_returns_field_range_in_timestamp_order() {
        let field = tractor_test_farm_fields()
            .field_by_id("field-north")
            .expect("test field exists");
        let history = append_irrigation_history_event(
            Vec::new(),
            irrigation_event_request("event-002", "2026-06-13T11:00:00Z", 8.0),
            &field,
            "generated-event".to_string(),
        )
        .expect("first irrigation event appends");
        let history = append_irrigation_history_event(
            history,
            irrigation_event_request("event-001", "2026-06-13T10:00:00Z", 6.0),
            &field,
            "generated-event".to_string(),
        )
        .expect("second irrigation event appends");

        let result = query_irrigation_history(
            &history,
            IrrigationHistoryQuery {
                field_id: " field-north ".to_string(),
                start_time: "2026-06-13T09:00:00Z".to_string(),
                end_time: "2026-06-13T12:00:00Z".to_string(),
            },
        )
        .expect("history query should succeed");

        assert!(!result.empty);
        assert_eq!(result.total_count, 2);
        assert_eq!(result.events[0].event_id, "event-001");
        assert_eq!(result.events[0].zone_ref, "zone:north");
        assert_eq!(result.events[0].applied_amount_mm, 6.0);
        assert_eq!(result.events[0].source, "valve-controller");
        assert_eq!(result.events[0].actor, "ops@example.com");
        assert_eq!(result.events[1].event_id, "event-002");
    }

    #[test]
    fn irrigation_history_query_returns_explicit_empty_result() {
        let result = query_irrigation_history(
            &[],
            IrrigationHistoryQuery {
                field_id: "field-north".to_string(),
                start_time: "2026-06-13T09:00:00Z".to_string(),
                end_time: "2026-06-13T12:00:00Z".to_string(),
            },
        )
        .expect("empty irrigation history is valid");

        assert_eq!(result.field_id, "field-north");
        assert_eq!(result.total_count, 0);
        assert!(result.empty);
        assert!(result.events.is_empty());
    }

    #[test]
    fn water_use_savings_report_computes_applied_vs_baseline() {
        let report = report_water_use_savings(water_use_savings_report_request(true))
            .expect("water-use savings report should compute");

        assert_eq!(report.field_id, "field-north");
        assert_eq!(report.zones.len(), 2);
        assert_eq!(report.zones[0].zone_ref, "zone:north");
        assert_eq!(report.zones[0].status, WaterUseSavingsStatus::Computed);
        assert_eq!(report.zones[0].applied_amount_mm, 14.0);
        assert_eq!(report.zones[0].baseline_amount_mm, Some(20.0));
        assert_eq!(report.zones[0].savings_mm, Some(6.0));
        assert_eq!(
            report.zones[0].baseline_method.as_deref(),
            Some("seasonal_baseline_v1")
        );
        assert_eq!(
            report.zones[0].evidence_refs,
            vec![
                "irrigation_event:event-001".to_string(),
                "irrigation_event:event-002".to_string()
            ]
        );
    }

    #[test]
    fn water_use_savings_report_marks_no_baseline_without_zero_assumption() {
        let report = report_water_use_savings(water_use_savings_report_request(false))
            .expect("missing baseline should report no-baseline status");

        let missing = report
            .zones
            .iter()
            .find(|zone| zone.zone_ref == "zone:south")
            .expect("zone without baseline should be present");
        assert_eq!(missing.status, WaterUseSavingsStatus::NoBaseline);
        assert_eq!(missing.applied_amount_mm, 5.0);
        assert_eq!(missing.baseline_amount_mm, None);
        assert_eq!(missing.savings_mm, None);
        assert_eq!(missing.baseline_method, None);
    }

    #[test]
    fn water_alerts_route_low_moisture_and_over_irrigation() {
        let result = evaluate_and_route_water_alerts(water_alert_routing_request(true, true))
            .expect("water alerts should evaluate and route");

        assert_eq!(result.alerts.len(), 2);
        assert!(result.alerts.iter().any(|alert| {
            alert.alert_type == WaterAlertType::LowMoisture
                && alert.zone_ref == "zone:north"
                && alert.evidence_freshness == WeatherFreshnessState::Fresh
        }));
        assert!(result.alerts.iter().any(|alert| {
            alert.alert_type == WaterAlertType::OverIrrigation && alert.zone_ref == "zone:south"
        }));
        assert_eq!(result.delivered_count, 4);
        assert_eq!(result.queued_count, 0);
        assert_eq!(result.rejected_count, 0);
        assert!(result.audits.iter().all(|audit| {
            audit.status == WeatherAlertDeliveryStatus::Delivered
                && audit.field_ref == "field-north"
                && audit.reason_code == "alert_delivered"
                && audit
                    .evidence_payload
                    .iter()
                    .any(|item| item == "freshness:Fresh")
        }));
    }

    #[test]
    fn water_alerts_do_not_raise_false_alarm_within_thresholds() {
        let result = evaluate_and_route_water_alerts(water_alert_routing_request(false, true))
            .expect("within-threshold evidence should not alert");

        assert!(result.alerts.is_empty());
        assert!(result.audits.is_empty());
        assert_eq!(result.delivered_count, 0);
    }

    #[test]
    fn water_alerts_reject_unowned_field_scope() {
        let result = evaluate_and_route_water_alerts(water_alert_routing_request(true, false))
            .expect("unowned field should audit rejection");

        assert_eq!(result.alerts.len(), 2);
        assert_eq!(result.rejected_count, 4);
        assert!(result
            .audits
            .iter()
            .all(|audit| audit.status == WeatherAlertDeliveryStatus::Rejected
                && audit.reason_code == "field_scope_not_owned"));
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

    #[test]
    fn drought_stress_evidence_ingests_georeferenced_scene_layer() {
        let field = tractor_test_farm_fields()
            .field_by_id("field-north")
            .expect("test field exists");

        let record = ingest_drought_stress_evidence(drought_stress_layer("EPSG:4326"), &field)
            .expect("matching drought stress layer should ingest");

        assert_eq!(record.field_or_region_ref, "field-north");
        assert_eq!(record.source_scene_ref, "scene-landsat-001");
        assert_eq!(record.index, DroughtStressIndex::Ndvi);
        assert_eq!(record.value, -0.18);
        assert_eq!(record.evidence_kind, "observed");
        assert_eq!(record.crs, "EPSG:4326");
        assert_eq!(record.extent, field.extent);
        assert_eq!(
            record.evidence_refs,
            vec!["scene:scene-landsat-001".to_string()]
        );
        assert!(record
            .evidence_id
            .starts_with("drought-stress:field-north:ndvi"));
    }

    #[test]
    fn drought_stress_evidence_rejects_crs_mismatch() {
        let field = tractor_test_farm_fields()
            .field_by_id("field-north")
            .expect("test field exists");

        let error = ingest_drought_stress_evidence(drought_stress_layer("EPSG:3857"), &field)
            .expect_err("mismatched CRS should refuse stress evidence");

        assert_eq!(
            error,
            DroughtStressEvidenceError::CrsMismatch {
                layer_crs: "EPSG:3857".to_string(),
                field_crs: "EPSG:4326".to_string()
            }
        );
    }

    #[test]
    fn drought_evidence_fusion_joins_stress_and_weather_inputs() {
        let record = fuse_drought_evidence(drought_evidence_fusion_request(
            Some(drought_stress_record("EPSG:4326")),
            vec![weather_window_record(
                "2026-06-13T10:00:00Z",
                4.2,
                0.0,
                22.0,
                false,
            )],
            "EPSG:4326",
        ))
        .expect("fresh stress and weather evidence should fuse");

        assert_eq!(record.field_or_region_ref, "field-north");
        assert_eq!(record.status, DroughtEvidenceFusionStatus::Complete);
        assert_eq!(record.crs, "EPSG:4326");
        assert_eq!(record.weather_records.len(), 1);
        assert!(record.stress_evidence.is_some());
        assert!(record.degradation_reasons.is_empty());
        assert!(record
            .evidence_refs
            .iter()
            .any(|item| item.starts_with("drought_stress:drought-stress:field-north:ndvi")));
        assert!(record
            .evidence_refs
            .iter()
            .any(|item| item.starts_with("weather_forecast:weather:field-north")));
        let stress = record
            .inputs
            .iter()
            .find(|input| input.input == "stress_evidence")
            .expect("stress input summary exists");
        assert_eq!(stress.status, DroughtEvidenceInputStatus::Present);
        assert_eq!(stress.coverage_fraction, 1.0);
        let weather = record
            .inputs
            .iter()
            .find(|input| input.input == "weather")
            .expect("weather input summary exists");
        assert_eq!(weather.status, DroughtEvidenceInputStatus::Present);
        assert_eq!(weather.freshness, WeatherFreshnessState::Fresh);
        assert_eq!(weather.coverage_fraction, 1.0);
    }

    #[test]
    fn drought_evidence_fusion_marks_missing_weather_degraded() {
        let record = fuse_drought_evidence(drought_evidence_fusion_request(
            Some(drought_stress_record("EPSG:4326")),
            Vec::new(),
            "EPSG:4326",
        ))
        .expect("missing weather should degrade without failing");

        assert_eq!(record.status, DroughtEvidenceFusionStatus::Degraded);
        assert_eq!(record.weather_records.len(), 0);
        assert_eq!(
            record.degradation_reasons,
            vec!["missing_weather_input".to_string()]
        );
        let weather = record
            .inputs
            .iter()
            .find(|input| input.input == "weather")
            .expect("weather input summary exists");
        assert_eq!(weather.status, DroughtEvidenceInputStatus::Missing);
        assert_eq!(weather.coverage_fraction, 0.0);
    }

    #[test]
    fn drought_evidence_fusion_marks_stale_weather_degraded() {
        let record = fuse_drought_evidence(drought_evidence_fusion_request(
            Some(drought_stress_record("EPSG:4326")),
            vec![weather_window_record(
                "2026-06-13T10:00:00Z",
                4.2,
                0.0,
                22.0,
                true,
            )],
            "EPSG:4326",
        ))
        .expect("stale weather should degrade without failing");

        assert_eq!(record.status, DroughtEvidenceFusionStatus::Degraded);
        assert_eq!(
            record.degradation_reasons,
            vec!["stale_weather_input".to_string()]
        );
        let weather = record
            .inputs
            .iter()
            .find(|input| input.input == "weather")
            .expect("weather input summary exists");
        assert_eq!(weather.status, DroughtEvidenceInputStatus::Stale);
        assert_eq!(weather.freshness, WeatherFreshnessState::Stale);
        assert_eq!(weather.coverage_fraction, 0.0);
    }

    #[test]
    fn drought_evidence_fusion_rejects_crs_mismatch() {
        let error = fuse_drought_evidence(drought_evidence_fusion_request(
            Some(drought_stress_record("EPSG:4326")),
            vec![weather_window_record(
                "2026-06-13T10:00:00Z",
                4.2,
                0.0,
                22.0,
                false,
            )],
            "EPSG:3857",
        ))
        .expect_err("mismatched fusion CRS should fail");

        assert_eq!(
            error,
            DroughtEvidenceFusionError::CrsMismatch {
                input: "stress_evidence".to_string(),
                input_crs: "EPSG:4326".to_string(),
                request_crs: "EPSG:3857".to_string()
            }
        );
    }

    #[test]
    fn drought_baseline_trend_computes_mean_slope_and_evidence_period() {
        let record = compute_drought_baseline_trend(DroughtBaselineTrendRequest {
            field_or_region_ref: " field-north ".to_string(),
            index_type: DroughtIndexType::Spi,
            min_samples: 3,
            history: vec![
                drought_index_record(
                    "spi-001",
                    "field-north",
                    DroughtIndexType::Spi,
                    -1.2,
                    "2026-04-01",
                ),
                drought_index_record(
                    "spi-002",
                    "field-north",
                    DroughtIndexType::Spi,
                    -0.6,
                    "2026-05-01",
                ),
                drought_index_record(
                    "spi-003",
                    "field-north",
                    DroughtIndexType::Spi,
                    0.0,
                    "2026-05-31",
                ),
                drought_index_record(
                    "spei-ignored",
                    "field-north",
                    DroughtIndexType::Spei,
                    2.0,
                    "2026-05-31",
                ),
                drought_index_record(
                    "field-ignored",
                    "field-south",
                    DroughtIndexType::Spi,
                    2.0,
                    "2026-05-31",
                ),
            ],
        })
        .expect("sufficient drought history should compute");

        assert_eq!(record.status, DroughtBaselineTrendStatus::Computed);
        assert_eq!(record.field_or_region_ref, "field-north");
        assert_eq!(record.index_type, DroughtIndexType::Spi);
        assert_eq!(record.sample_count, 3);
        assert_eq!(record.baseline_value, Some(-0.6));
        assert_eq!(record.trend_slope_per_day, Some(0.02));
        assert_eq!(
            record.trend_direction,
            Some(DroughtTrendDirection::Improving)
        );
        assert_eq!(
            record.period,
            Some(DroughtIndexPeriod {
                start: "2026-04-01".to_string(),
                end: "2026-05-31".to_string(),
                accumulation_days: Some(30),
            })
        );
        assert_eq!(
            record.evidence_refs,
            vec![
                "drought_index:spi-001".to_string(),
                "drought_index:spi-002".to_string(),
                "drought_index:spi-003".to_string()
            ]
        );
        assert!(record.degradation_reasons.is_empty());
    }

    #[test]
    fn drought_baseline_trend_reports_insufficient_history() {
        let record = compute_drought_baseline_trend(DroughtBaselineTrendRequest {
            field_or_region_ref: "field-north".to_string(),
            index_type: DroughtIndexType::Spi,
            min_samples: 3,
            history: vec![
                drought_index_record(
                    "spi-001",
                    "field-north",
                    DroughtIndexType::Spi,
                    -1.2,
                    "2026-04-01",
                ),
                drought_index_record(
                    "spi-002",
                    "field-north",
                    DroughtIndexType::Spi,
                    -0.6,
                    "2026-05-01",
                ),
            ],
        })
        .expect("insufficient history should be a reportable state");

        assert_eq!(
            record.status,
            DroughtBaselineTrendStatus::InsufficientBaseline
        );
        assert_eq!(record.sample_count, 2);
        assert_eq!(record.baseline_value, None);
        assert_eq!(record.trend_slope_per_day, None);
        assert_eq!(record.trend_direction, None);
        assert_eq!(
            record.degradation_reasons,
            vec!["insufficient_baseline_history".to_string()]
        );
    }

    #[test]
    fn drought_baseline_trend_rejects_invalid_min_samples() {
        let error = compute_drought_baseline_trend(DroughtBaselineTrendRequest {
            field_or_region_ref: "field-north".to_string(),
            index_type: DroughtIndexType::Spi,
            min_samples: 1,
            history: Vec::new(),
        })
        .expect_err("trend needs at least two samples");

        assert_eq!(error, DroughtBaselineTrendError::InvalidMinSamples);
    }

    #[test]
    fn drought_risk_score_computes_band_and_cites_all_inputs() {
        let record = compute_drought_risk_score(drought_risk_score_request(true))
            .expect("complete deterministic evidence should score");

        assert_eq!(record.status, DroughtRiskScoreStatus::Computed);
        assert_eq!(record.field_or_region_ref, "field-north");
        assert_eq!(record.index_type, Some(DroughtIndexType::Spi));
        assert_eq!(record.value, Some(0.435));
        assert_eq!(record.band, Some(DroughtRiskBand::Moderate));
        assert_eq!(record.thresholds.moderate, 0.3);
        assert!(record
            .evidence_refs
            .iter()
            .any(|item| item == "drought_index:spi-current"));
        assert!(record
            .evidence_refs
            .iter()
            .any(|item| item.starts_with("drought_stress:drought-stress:field-north:ndvi")));
        assert!(record.evidence_refs.iter().any(|item| {
            item == "drought_baseline:field-north:spi:historical_mean_linear_trend_v1"
        }));
        assert!(record.degradation_reasons.is_empty());
    }

    #[test]
    fn drought_risk_score_reports_insufficient_evidence_without_partial_score() {
        let record = compute_drought_risk_score(drought_risk_score_request(false))
            .expect("missing stress evidence should be a reportable state");

        assert_eq!(record.status, DroughtRiskScoreStatus::InsufficientEvidence);
        assert_eq!(record.value, None);
        assert_eq!(record.band, None);
        assert_eq!(
            record.degradation_reasons,
            vec!["missing_stress_evidence".to_string()]
        );
        assert!(record
            .evidence_refs
            .iter()
            .any(|item| item == "drought_index:spi-current"));
    }

    #[test]
    fn drought_risk_score_rejects_invalid_thresholds() {
        let mut request = drought_risk_score_request(true);
        request.thresholds = DroughtRiskThresholds {
            moderate: 0.7,
            high: 0.6,
            severe: 0.8,
        };

        let error = compute_drought_risk_score(request)
            .expect_err("unordered thresholds should reject risk scoring");

        assert_eq!(error, DroughtRiskScoreError::InvalidThresholds);
    }

    #[test]
    fn drought_forecast_runs_only_with_fresh_deterministic_score() {
        let record =
            forecast_drought_risk(drought_forecast_request(true, true, "2026-06-14T10:00:00Z"))
                .expect("fresh deterministic score should unlock forecast");

        assert_eq!(record.status, DroughtForecastStatus::Forecast);
        assert_eq!(record.field_or_region_ref, "field-north");
        assert_eq!(record.horizon_days, 30);
        assert_eq!(record.predicted_value, Some(0.45166666666666666));
        assert_eq!(record.predicted_band, Some(DroughtRiskBand::Moderate));
        assert_eq!(
            record.uncertainty,
            Some(DroughtForecastUncertaintyBand::Medium)
        );
        assert!(record
            .evidence_refs
            .iter()
            .any(|item| item == "drought_risk_score:field-north:moderate"));
        assert!(record.unavailable_reasons.is_empty());
    }

    #[test]
    fn drought_forecast_is_unavailable_when_feature_disabled() {
        let record = forecast_drought_risk(drought_forecast_request(
            false,
            true,
            "2026-06-14T10:00:00Z",
        ))
        .expect("disabled forecast should return unavailable state");

        assert_eq!(record.status, DroughtForecastStatus::Unavailable);
        assert_eq!(record.predicted_value, None);
        assert_eq!(
            record.unavailable_reasons,
            vec!["forecast_feature_disabled".to_string()]
        );
    }

    #[test]
    fn drought_forecast_is_unavailable_with_stale_or_missing_score() {
        let stale =
            forecast_drought_risk(drought_forecast_request(true, true, "2026-06-25T10:00:00Z"))
                .expect("stale score should return unavailable state");
        assert_eq!(stale.status, DroughtForecastStatus::Unavailable);
        assert_eq!(
            stale.unavailable_reasons,
            vec!["stale_risk_score".to_string()]
        );

        let missing = forecast_drought_risk(drought_forecast_request(
            true,
            false,
            "2026-06-14T10:00:00Z",
        ))
        .expect("missing score should return unavailable state");
        assert_eq!(missing.status, DroughtForecastStatus::Unavailable);
        assert_eq!(
            missing.unavailable_reasons,
            vec!["missing_risk_score".to_string()]
        );
    }

    #[test]
    fn drought_alerts_route_threshold_crossing_to_operator_and_portal() {
        let result = evaluate_and_route_drought_alerts(drought_alert_routing_request(0.3, true))
            .expect("threshold-crossing drought risk should route");

        assert_eq!(result.alerts.len(), 1);
        assert_eq!(result.delivered_count, 2);
        assert_eq!(result.queued_count, 0);
        assert_eq!(result.rejected_count, 0);
        assert_eq!(result.audits.len(), 2);
        assert!(result.alerts[0]
            .evidence_refs
            .iter()
            .any(|item| item == "drought_risk_score:field-north:moderate"));
        assert!(result.audits.iter().all(|audit| {
            audit.status == WeatherAlertDeliveryStatus::Delivered
                && audit.reason_code == "delivered"
                && audit
                    .evidence_payload
                    .iter()
                    .any(|item| item == "freshness:Fresh")
        }));
    }

    #[test]
    fn drought_alerts_do_not_fire_below_threshold() {
        let result = evaluate_and_route_drought_alerts(drought_alert_routing_request(0.9, true))
            .expect("below-threshold drought risk should not alert");

        assert!(result.alerts.is_empty());
        assert!(result.audits.is_empty());
        assert_eq!(result.delivered_count, 0);
    }

    #[test]
    fn drought_alerts_reject_unowned_field_scope() {
        let result = evaluate_and_route_drought_alerts(drought_alert_routing_request(0.3, false))
            .expect("unowned field should audit rejection");

        assert_eq!(result.alerts.len(), 1);
        assert_eq!(result.rejected_count, 2);
        assert!(result
            .audits
            .iter()
            .all(|audit| audit.status == WeatherAlertDeliveryStatus::Rejected
                && audit.reason_code == "field_scope_not_owned"));
    }

    #[test]
    fn drought_mitigation_recommends_irrigation_for_high_risk() {
        let record = derive_drought_mitigation_recommendation(drought_mitigation_request(
            0.6,
            Some((0.72, DroughtRiskBand::High)),
        ))
        .expect("high risk should produce mitigation recommendation");

        assert_eq!(record.status, DroughtMitigationStatus::Recommended);
        assert_eq!(
            record.action_target,
            Some(DroughtMitigationActionTarget::Irrigation16)
        );
        assert_eq!(
            record.recommendation,
            Some("review_irrigation_schedule_for_drought_stress".to_string())
        );
        assert_eq!(
            record.risk_ref,
            Some("drought_risk_score:field-north:high".to_string())
        );
        assert!(record
            .evidence_refs
            .iter()
            .any(|item| item == "drought_risk_score:field-north:high"));
    }

    #[test]
    fn drought_mitigation_returns_no_recommendation_without_qualifying_risk() {
        let record =
            derive_drought_mitigation_recommendation(drought_mitigation_request(0.9, None))
                .expect("below-threshold risk should be a no-op");

        assert_eq!(record.status, DroughtMitigationStatus::NotQualified);
        assert_eq!(record.action_target, None);
        assert_eq!(record.recommendation, None);
        assert_eq!(record.risk_ref, None);
        assert_eq!(record.reason_code, "risk_below_mitigation_threshold");
    }

    #[test]
    fn drought_mitigation_rejects_invalid_threshold() {
        let error = derive_drought_mitigation_recommendation(drought_mitigation_request(1.2, None))
            .expect_err("invalid mitigation threshold should reject");

        assert_eq!(error, DroughtMitigationError::InvalidMinRiskValue);
    }

    #[test]
    fn drought_report_assembles_ordered_sections_with_evidence() {
        let report = assemble_drought_report(drought_report_request(true, true, true))
            .expect("complete deterministic drought evidence should assemble");

        assert_eq!(report.report_id, "drought-report-field-north");
        assert_eq!(report.field_or_region_ref, "field-north");
        assert_eq!(
            report
                .sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            vec![
                DroughtReportSectionKind::Index,
                DroughtReportSectionKind::BaselineTrend,
                DroughtReportSectionKind::RiskScore,
                DroughtReportSectionKind::Forecast,
                DroughtReportSectionKind::Mitigation
            ]
        );
        assert!(report
            .sections
            .iter()
            .all(|section| !section.evidence_refs.is_empty()));
        assert!(report
            .evidence_refs
            .iter()
            .any(|item| item == "drought_index:spi-current"));
        assert!(report
            .freshness_assertions
            .iter()
            .any(|item| item == "deterministic_sections_first"));
    }

    #[test]
    fn drought_report_can_omit_optional_forecast_and_mitigation() {
        let report = assemble_drought_report(drought_report_request(true, false, false))
            .expect("required deterministic evidence should be enough for report");

        assert_eq!(
            report
                .sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            vec![
                DroughtReportSectionKind::Index,
                DroughtReportSectionKind::BaselineTrend,
                DroughtReportSectionKind::RiskScore
            ]
        );
    }

    #[test]
    fn drought_report_rejects_missing_required_evidence() {
        let error = assemble_drought_report(drought_report_request(false, false, false))
            .expect_err("missing computed baseline should reject report");

        assert_eq!(error, DroughtReportError::MissingBaseline);
    }

    #[test]
    fn drought_history_query_returns_ordered_evidence_records() {
        let result = query_drought_history(drought_history_query(false))
            .expect("evidence-backed drought history should query");

        assert_eq!(result.field_or_region_ref, "field-north");
        assert_eq!(result.total_count, 4);
        assert_eq!(
            result
                .entries
                .iter()
                .map(|entry| entry.kind)
                .collect::<Vec<_>>(),
            vec![
                DroughtHistoryEntryKind::Index,
                DroughtHistoryEntryKind::RiskScore,
                DroughtHistoryEntryKind::Alert,
                DroughtHistoryEntryKind::Recommendation
            ]
        );
        assert!(result
            .entries
            .iter()
            .all(|entry| !entry.evidence_refs.is_empty()));
    }

    #[test]
    fn drought_history_query_rejects_entries_without_evidence() {
        let error = query_drought_history(drought_history_query(true))
            .expect_err("history entries without evidence should reject");

        assert_eq!(
            error,
            DroughtHistoryError::MissingEvidence {
                record_ref: "drought_index:missing-evidence".to_string()
            }
        );
    }

    #[test]
    fn drought_advisory_loop_stays_disabled_without_reliable_gate() {
        let evaluation = evaluate_drought_advisory_loop(DroughtAdvisoryLoopRequest {
            feature_enabled: false,
            deterministic_scoring_reliable: false,
            latest_risk_score: Some(
                compute_drought_risk_score(drought_risk_score_request(true))
                    .expect("risk score fixture should compute"),
            ),
            latest_recommendation: Some(
                derive_drought_mitigation_recommendation(drought_mitigation_request(
                    0.6,
                    Some((0.72, DroughtRiskBand::High)),
                ))
                .expect("mitigation fixture should recommend"),
            ),
        });

        assert_eq!(evaluation.status, DroughtAdvisoryLoopStatus::Disabled);
        assert_eq!(
            evaluation.disabled_reasons,
            vec![
                "advisory_loop_feature_disabled".to_string(),
                "deterministic_scoring_not_reliable".to_string()
            ]
        );
        assert!(!evaluation.evidence_refs.is_empty());
    }

    #[test]
    fn drought_advisory_loop_can_enable_with_reliable_evidence() {
        let evaluation = evaluate_drought_advisory_loop(DroughtAdvisoryLoopRequest {
            feature_enabled: true,
            deterministic_scoring_reliable: true,
            latest_risk_score: Some(
                compute_drought_risk_score(drought_risk_score_request(true))
                    .expect("risk score fixture should compute"),
            ),
            latest_recommendation: Some(
                derive_drought_mitigation_recommendation(drought_mitigation_request(
                    0.6,
                    Some((0.72, DroughtRiskBand::High)),
                ))
                .expect("mitigation fixture should recommend"),
            ),
        });

        assert_eq!(evaluation.status, DroughtAdvisoryLoopStatus::Enabled);
        assert!(evaluation.disabled_reasons.is_empty());
        assert!(evaluation
            .evidence_refs
            .iter()
            .any(|item| item == "drought_risk_score:field-north:high"));
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

    fn moisture_proxy_layer(crs: &str) -> RemoteSensingMoistureProxyLayer {
        RemoteSensingMoistureProxyLayer {
            layer_id: " layer-ndmi-field-north ".to_string(),
            field_id: " field-north ".to_string(),
            index: RemoteSensingMoistureIndex::Ndmi,
            source: " imagery_processor ".to_string(),
            captured_at: " 2026-06-13T10:00:00Z ".to_string(),
            width: 1,
            height: 1,
            spatial_ref: Some(RasterSpatialRef {
                georeferenced: true,
                crs: Some(crs.to_string()),
                bbox: Some(GeoBounds {
                    min_lon: -96.5,
                    min_lat: 41.2,
                    max_lon: -96.2,
                    max_lat: 41.4,
                }),
                geo_transform: Some([-96.5, 0.3, 0.0, 41.4, 0.0, -0.2]),
                resolution: Some(RasterResolution { x: 0.3, y: 0.2 }),
            }),
            zone_values: vec![
                RemoteSensingMoistureZoneValue {
                    zone_ref: " zone:north ".to_string(),
                    value: 0.42,
                },
                RemoteSensingMoistureZoneValue {
                    zone_ref: "zone:south".to_string(),
                    value: 0.21,
                },
            ],
        }
    }

    fn drought_stress_layer(crs: &str) -> DroughtStressEvidenceLayer {
        DroughtStressEvidenceLayer {
            evidence_id: None,
            field_or_region_ref: " field-north ".to_string(),
            source_scene_ref: " scene-landsat-001 ".to_string(),
            index: DroughtStressIndex::Ndvi,
            value: -0.18,
            source: " imagery_processor ".to_string(),
            captured_at: " 2026-06-13T10:00:00Z ".to_string(),
            width: 1,
            height: 1,
            spatial_ref: Some(RasterSpatialRef {
                georeferenced: true,
                crs: Some(crs.to_string()),
                bbox: Some(GeoBounds {
                    min_lon: -96.5,
                    min_lat: 41.2,
                    max_lon: -96.2,
                    max_lat: 41.4,
                }),
                geo_transform: Some([-96.5, 0.3, 0.0, 41.4, 0.0, -0.2]),
                resolution: Some(RasterResolution { x: 0.3, y: 0.2 }),
            }),
        }
    }

    fn drought_index_record(
        index_id: &str,
        field_or_region_ref: &str,
        index_type: DroughtIndexType,
        value: f64,
        period_end: &str,
    ) -> super::DroughtIndexRecord {
        super::DroughtIndexRecord {
            index_id: index_id.to_string(),
            field_or_region_ref: field_or_region_ref.to_string(),
            index_type,
            value,
            period: DroughtIndexPeriod {
                start: period_end.to_string(),
                end: period_end.to_string(),
                accumulation_days: Some(30),
            },
            input_refs: vec![format!("weather:{field_or_region_ref}:{period_end}")],
            method: "standardized_anomaly_v1".to_string(),
            computed_at: "2026-06-13T10:00:00Z".to_string(),
        }
    }

    fn drought_baseline_record() -> super::DroughtBaselineTrendRecord {
        compute_drought_baseline_trend(DroughtBaselineTrendRequest {
            field_or_region_ref: "field-north".to_string(),
            index_type: DroughtIndexType::Spi,
            min_samples: 3,
            history: vec![
                drought_index_record(
                    "spi-001",
                    "field-north",
                    DroughtIndexType::Spi,
                    -1.2,
                    "2026-04-01",
                ),
                drought_index_record(
                    "spi-002",
                    "field-north",
                    DroughtIndexType::Spi,
                    -0.6,
                    "2026-05-01",
                ),
                drought_index_record(
                    "spi-003",
                    "field-north",
                    DroughtIndexType::Spi,
                    0.0,
                    "2026-05-31",
                ),
            ],
        })
        .expect("baseline fixture should compute")
    }

    fn drought_risk_score_request(include_stress: bool) -> DroughtRiskScoreRequest {
        DroughtRiskScoreRequest {
            field_or_region_ref: " field-north ".to_string(),
            thresholds: DroughtRiskThresholds {
                moderate: 0.3,
                high: 0.6,
                severe: 0.8,
            },
            index_record: Some(drought_index_record(
                "spi-current",
                "field-north",
                DroughtIndexType::Spi,
                -1.2,
                "2026-06-13",
            )),
            stress_evidence: include_stress.then(|| drought_stress_record("EPSG:4326")),
            baseline: Some(drought_baseline_record()),
        }
    }

    fn drought_forecast_request(
        feature_enabled: bool,
        include_score: bool,
        requested_at: &str,
    ) -> DroughtForecastRequest {
        DroughtForecastRequest {
            feature_enabled,
            requested_at: requested_at.to_string(),
            risk_score_computed_at: "2026-06-13T10:00:00Z".to_string(),
            max_score_age_days: 3,
            horizon_days: 30,
            risk_score: include_score.then(|| {
                compute_drought_risk_score(drought_risk_score_request(true))
                    .expect("risk score fixture should compute")
            }),
        }
    }

    fn drought_alert_routing_request(
        warning_threshold: f64,
        owned_field: bool,
    ) -> DroughtAlertRoutingRequest {
        DroughtAlertRoutingRequest {
            risk_score: compute_drought_risk_score(drought_risk_score_request(true))
                .expect("risk score fixture should compute"),
            warning_threshold,
            recipient_id: " grower-001 ".to_string(),
            owned_field_refs: if owned_field {
                vec!["field-north".to_string()]
            } else {
                vec!["field-south".to_string()]
            },
            targets: vec![
                WeatherAlertRoutingTarget {
                    target: WeatherAlertRouteTarget::OperatorConsole,
                    reachable: true,
                },
                WeatherAlertRoutingTarget {
                    target: WeatherAlertRouteTarget::FarmersPortal,
                    reachable: true,
                },
            ],
            routed_at: " 2026-06-14T10:05:00Z ".to_string(),
            freshness: WeatherFreshnessState::Fresh,
        }
    }

    fn drought_mitigation_request(
        min_risk_value: f64,
        override_score: Option<(f64, DroughtRiskBand)>,
    ) -> DroughtMitigationRequest {
        let mut risk_score = compute_drought_risk_score(drought_risk_score_request(true))
            .expect("risk score fixture should compute");
        if let Some((value, band)) = override_score {
            risk_score.value = Some(value);
            risk_score.band = Some(band);
        }
        DroughtMitigationRequest {
            risk_score,
            generated_at: " 2026-06-14T10:10:00Z ".to_string(),
            min_risk_value,
        }
    }

    fn drought_report_request(
        include_required_baseline: bool,
        include_forecast: bool,
        include_mitigation: bool,
    ) -> DroughtReportRequest {
        let index_record = drought_index_record(
            "spi-current",
            "field-north",
            DroughtIndexType::Spi,
            -1.2,
            "2026-06-13",
        );
        let mut baseline = drought_baseline_record();
        if !include_required_baseline {
            baseline.status = DroughtBaselineTrendStatus::InsufficientBaseline;
            baseline.baseline_value = None;
            baseline.trend_slope_per_day = None;
            baseline.trend_direction = None;
        }
        let risk_score = compute_drought_risk_score(drought_risk_score_request(true))
            .expect("risk score fixture should compute");
        let forecast = include_forecast.then(|| {
            forecast_drought_risk(drought_forecast_request(true, true, "2026-06-14T10:00:00Z"))
                .expect("forecast fixture should compute")
        });
        let mitigation = include_mitigation.then(|| {
            derive_drought_mitigation_recommendation(drought_mitigation_request(
                0.6,
                Some((0.72, DroughtRiskBand::High)),
            ))
            .expect("mitigation fixture should recommend")
        });

        DroughtReportRequest {
            report_id: " drought-report-field-north ".to_string(),
            generated_at: " 2026-06-14T10:30:00Z ".to_string(),
            index_record,
            baseline,
            risk_score,
            forecast,
            mitigation,
        }
    }

    fn drought_history_query(include_missing_evidence: bool) -> DroughtHistoryQuery {
        let mut entries = vec![
            DroughtHistoryEntry {
                sequence: 1,
                field_or_region_ref: "field-north".to_string(),
                occurred_at: "2026-06-13T10:00:00Z".to_string(),
                kind: DroughtHistoryEntryKind::Index,
                record_ref: "drought_index:spi-current".to_string(),
                evidence_refs: vec!["weather:field-north:2026-06-13".to_string()],
            },
            DroughtHistoryEntry {
                sequence: 2,
                field_or_region_ref: "field-north".to_string(),
                occurred_at: "2026-06-13T10:05:00Z".to_string(),
                kind: DroughtHistoryEntryKind::RiskScore,
                record_ref: "drought_risk_score:field-north:moderate".to_string(),
                evidence_refs: vec!["drought_index:spi-current".to_string()],
            },
            DroughtHistoryEntry {
                sequence: 3,
                field_or_region_ref: "field-north".to_string(),
                occurred_at: "2026-06-14T10:05:00Z".to_string(),
                kind: DroughtHistoryEntryKind::Alert,
                record_ref: "drought_alert:field-north:moderate".to_string(),
                evidence_refs: vec!["drought_risk_score:field-north:moderate".to_string()],
            },
            DroughtHistoryEntry {
                sequence: 4,
                field_or_region_ref: "field-north".to_string(),
                occurred_at: "2026-06-14T10:10:00Z".to_string(),
                kind: DroughtHistoryEntryKind::Recommendation,
                record_ref: "drought_mitigation:field-north:irrigation16".to_string(),
                evidence_refs: vec!["drought_risk_score:field-north:high".to_string()],
            },
            DroughtHistoryEntry {
                sequence: 5,
                field_or_region_ref: "field-south".to_string(),
                occurred_at: "2026-06-14T10:10:00Z".to_string(),
                kind: DroughtHistoryEntryKind::Recommendation,
                record_ref: "drought_mitigation:field-south:advisor09".to_string(),
                evidence_refs: vec!["drought_risk_score:field-south:moderate".to_string()],
            },
        ];
        if include_missing_evidence {
            entries.push(DroughtHistoryEntry {
                sequence: 6,
                field_or_region_ref: "field-north".to_string(),
                occurred_at: "2026-06-15T10:10:00Z".to_string(),
                kind: DroughtHistoryEntryKind::Index,
                record_ref: "drought_index:missing-evidence".to_string(),
                evidence_refs: Vec::new(),
            });
        }

        DroughtHistoryQuery {
            field_or_region_ref: " field-north ".to_string(),
            start_time: "2026-06-13T00:00:00Z".to_string(),
            end_time: "2026-06-15T00:00:00Z".to_string(),
            offset: 0,
            limit: 10,
            entries,
        }
    }

    fn drought_stress_record(crs: &str) -> super::DroughtStressEvidenceRecord {
        let field = tractor_test_farm_fields()
            .field_by_id("field-north")
            .expect("test field exists");
        ingest_drought_stress_evidence(drought_stress_layer(crs), &field)
            .expect("stress fixture should ingest")
    }

    fn drought_evidence_fusion_request(
        stress_evidence: Option<super::DroughtStressEvidenceRecord>,
        weather_records: Vec<super::WeatherFreshnessAnnotatedRecord>,
        crs: &str,
    ) -> DroughtEvidenceFusionRequest {
        let field = tractor_test_farm_fields()
            .field_by_id("field-north")
            .expect("test field exists");
        DroughtEvidenceFusionRequest {
            field_or_region_ref: " field-north ".to_string(),
            period: DroughtIndexPeriod {
                start: " 2026-06-13T09:00:00Z ".to_string(),
                end: " 2026-06-13T12:00:00Z ".to_string(),
                accumulation_days: None,
            },
            crs: crs.to_string(),
            extent: field.extent,
            stress_evidence,
            weather_records,
        }
    }

    fn marketplace_account_fixture(
        account_id: &str,
        org_id: &str,
        party_type: MarketplacePartyType,
        status: MarketplaceAccountStatus,
    ) -> MarketplaceAccountRecord {
        MarketplaceAccountRecord {
            account_id: account_id.to_string(),
            org_id: org_id.to_string(),
            party_type,
            role_refs: vec!["marketplace:seller".to_string()],
            status,
            created_at: "2026-06-14T09:00:00Z".to_string(),
            updated_at: "2026-06-14T09:00:00Z".to_string(),
        }
    }

    fn marketplace_catalog_item_request(
        unit_of_measure: MarketplaceUnitOfMeasure,
    ) -> MarketplaceCatalogItemCreateRequest {
        MarketplaceCatalogItemCreateRequest {
            item_id: Some("seed-corn-001".to_string()),
            org_id: "org-alpha".to_string(),
            kind: MarketplaceCatalogItemKind::Input,
            category: MarketplaceCatalogCategory::Seed,
            name: "Hybrid corn seed".to_string(),
            unit_of_measure,
            owner_account_id: "supplier-001".to_string(),
        }
    }

    fn marketplace_catalog_item_fixture(
        item_id: &str,
        org_id: &str,
    ) -> super::MarketplaceCatalogItemRecord {
        super::MarketplaceCatalogItemRecord {
            item_id: item_id.to_string(),
            org_id: org_id.to_string(),
            kind: MarketplaceCatalogItemKind::Input,
            category: MarketplaceCatalogCategory::Seed,
            name: "Hybrid corn seed".to_string(),
            unit_of_measure: MarketplaceUnitOfMeasure::Bag,
            owner_account_id: "supplier-001".to_string(),
            created_at: "2026-06-14T08:00:00Z".to_string(),
        }
    }

    fn marketplace_listing_request(
        window_from: &str,
        window_to: &str,
    ) -> MarketplaceListingPublishRequest {
        MarketplaceListingPublishRequest {
            listing_id: Some("listing-seed-corn-001".to_string()),
            item_id: "seed-corn-001".to_string(),
            org_id: "org-alpha".to_string(),
            price: 125.0,
            currency: "USD".to_string(),
            available_qty: 40.0,
            window: MarketplaceAvailabilityWindow {
                from: window_from.to_string(),
                to: window_to.to_string(),
            },
        }
    }

    fn marketplace_listing_fixture() -> super::MarketplaceListingRecord {
        super::MarketplaceListingRecord {
            listing_id: "listing-seed-corn-001".to_string(),
            item_id: "seed-corn-001".to_string(),
            org_id: "org-alpha".to_string(),
            price: 125.0,
            currency: "USD".to_string(),
            available_qty: 40.0,
            window: MarketplaceAvailabilityWindow {
                from: "2026-06-14T09:00:00Z".to_string(),
                to: "2026-07-14T09:00:00Z".to_string(),
            },
            status: MarketplaceListingStatus::Published,
            created_at: "2026-06-14T08:00:00Z".to_string(),
            updated_at: "2026-06-14T08:00:00Z".to_string(),
        }
    }

    fn marketplace_inventory_request(
        on_hand: f64,
        reserved: Option<f64>,
    ) -> MarketplaceInventoryUpsertRequest {
        MarketplaceInventoryUpsertRequest {
            inventory_id: Some("inventory-seed-corn-001".to_string()),
            item_id: "seed-corn-001".to_string(),
            org_id: "org-alpha".to_string(),
            on_hand,
            reserved,
        }
    }

    fn marketplace_order_request(qty: f64) -> MarketplaceOrderCreateRequest {
        MarketplaceOrderCreateRequest {
            order_id: Some("order-seed-corn-001".to_string()),
            org_id: "org-alpha".to_string(),
            listing_ref: "listing-seed-corn-001".to_string(),
            buyer_account_id: "buyer-001".to_string(),
            qty,
        }
    }

    fn marketplace_order_fixture(
        order_id: &str,
        status: MarketplaceOrderStatus,
        line_total: f64,
        created_at: &str,
    ) -> super::MarketplaceOrderRecord {
        super::MarketplaceOrderRecord {
            order_id: order_id.to_string(),
            org_id: "org-alpha".to_string(),
            listing_ref: "listing-seed-corn-001".to_string(),
            buyer_account_id: "buyer-001".to_string(),
            qty: 1.0,
            line_total,
            status,
            created_at: created_at.to_string(),
            updated_at: created_at.to_string(),
        }
    }

    fn marketplace_fulfillment_request(
        order_ref: &str,
        org_id: &str,
    ) -> MarketplaceFulfillmentCreateRequest {
        MarketplaceFulfillmentCreateRequest {
            fulfillment_id: Some("fulfillment-order-001".to_string()),
            order_ref: order_ref.to_string(),
            org_id: org_id.to_string(),
            carrier_ref: "carrier:opaque".to_string(),
            tracking_ref: "tracking:opaque".to_string(),
            actor_id: "ops-001".to_string(),
        }
    }

    fn marketplace_rating_request(
        order_ref: &str,
        rater_account_id: &str,
        ratee_account_id: &str,
        score: f64,
    ) -> MarketplaceRatingCreateRequest {
        MarketplaceRatingCreateRequest {
            rating_id: Some(format!("rating-{order_ref}-{rater_account_id}")),
            order_ref: order_ref.to_string(),
            rater_account_id: rater_account_id.to_string(),
            ratee_account_id: ratee_account_id.to_string(),
            score,
            comment: Some("Reliable counterparty".to_string()),
            org_scope: "org-alpha".to_string(),
        }
    }

    fn marketplace_rating_participants() -> Vec<String> {
        vec!["buyer-001".to_string(), "supplier-001".to_string()]
    }

    fn marketplace_inventory_fixture(
        on_hand: f64,
        reserved: f64,
    ) -> super::MarketplaceInventoryRecord {
        super::MarketplaceInventoryRecord {
            inventory_id: "inventory-seed-corn-001".to_string(),
            item_id: "seed-corn-001".to_string(),
            org_id: "org-alpha".to_string(),
            on_hand,
            reserved,
            updated_at: "2026-06-15T09:00:00Z".to_string(),
        }
    }

    fn marketplace_report_request() -> MarketplaceOrgReportRequest {
        MarketplaceOrgReportRequest {
            org_id: "org-alpha".to_string(),
            period: MarketplaceReportPeriod {
                from: "2026-06-01T00:00:00Z".to_string(),
                to: "2026-06-30T23:59:59Z".to_string(),
            },
        }
    }

    fn marketplace_demand_request(
        item_kind: MarketplaceCatalogItemKind,
        ai_assisted: bool,
    ) -> MarketplaceDemandForecastRequest {
        MarketplaceDemandForecastRequest {
            forecast_id: Some("forecast-seed-corn-001".to_string()),
            org_id: "org-alpha".to_string(),
            field_id: "field-alpha".to_string(),
            item_kind,
            horizon: "2026-season".to_string(),
            ai_assisted,
        }
    }

    fn marketplace_demand_field_fixture(area_ha: Option<f64>) -> FieldRecord {
        FieldRecord {
            farm_id: Some("farm-alpha".to_string()),
            field_id: "field-alpha".to_string(),
            org_id: "org-alpha".to_string(),
            owner: "org-alpha".to_string(),
            name: "Alpha Field".to_string(),
            area_ha,
            crop: Some("corn".to_string()),
            season: Some("2026".to_string()),
            notes: None,
            boundary: FieldBoundary {
                crs: Some("EPSG:4326".to_string()),
                coordinates: vec![
                    GeoPoint {
                        longitude: -96.7,
                        latitude: 41.1,
                    },
                    GeoPoint {
                        longitude: -96.2,
                        latitude: 41.1,
                    },
                    GeoPoint {
                        longitude: -96.2,
                        latitude: 41.4,
                    },
                    GeoPoint {
                        longitude: -96.7,
                        latitude: 41.1,
                    },
                ],
            },
            extent: GeoBounds {
                min_lon: -96.7,
                min_lat: 41.1,
                max_lon: -96.2,
                max_lat: 41.4,
            },
            status: FarmFieldEntityStatus::Active,
            created_at: "2026-06-15T09:00:00Z".to_string(),
            updated_at: "2026-06-15T09:00:00Z".to_string(),
        }
    }

    fn irrigation_event_request(
        event_id: &str,
        timestamp: &str,
        applied_amount_mm: f64,
    ) -> IrrigationEventRequest {
        IrrigationEventRequest {
            event_id: Some(format!(" {event_id} ")),
            field_id: " field-north ".to_string(),
            zone_ref: " zone:north ".to_string(),
            applied_amount_mm,
            source: " valve-controller ".to_string(),
            timestamp: format!(" {timestamp} "),
            actor: " ops@example.com ".to_string(),
        }
    }

    fn water_use_savings_report_request(
        include_south_baseline: bool,
    ) -> WaterUseSavingsReportRequest {
        let field = tractor_test_farm_fields()
            .field_by_id("field-north")
            .expect("test field exists");
        let history = append_irrigation_history_event(
            Vec::new(),
            irrigation_event_request("event-001", "2026-06-13T10:00:00Z", 6.0),
            &field,
            "generated-event".to_string(),
        )
        .expect("first event appends");
        let mut history = append_irrigation_history_event(
            history,
            irrigation_event_request("event-002", "2026-06-13T11:00:00Z", 8.0),
            &field,
            "generated-event".to_string(),
        )
        .expect("second event appends");
        history.push(IrrigationEventRecord {
            event_id: "event-003".to_string(),
            field_id: "field-north".to_string(),
            zone_ref: "zone:south".to_string(),
            applied_amount_mm: 5.0,
            source: "valve-controller".to_string(),
            timestamp: "2026-06-13T11:30:00Z".to_string(),
            actor: "ops@example.com".to_string(),
        });

        let mut baselines = vec![WaterUseBaseline {
            field_id: " field-north ".to_string(),
            zone_ref: " zone:north ".to_string(),
            baseline_amount_mm: 20.0,
            method: " seasonal_baseline_v1 ".to_string(),
        }];
        if include_south_baseline {
            baselines.push(WaterUseBaseline {
                field_id: "field-north".to_string(),
                zone_ref: "zone:south".to_string(),
                baseline_amount_mm: 7.0,
                method: "seasonal_baseline_v1".to_string(),
            });
        }

        WaterUseSavingsReportRequest {
            field_id: " field-north ".to_string(),
            start_time: " 2026-06-13T09:00:00Z ".to_string(),
            end_time: " 2026-06-13T12:00:00Z ".to_string(),
            events: history,
            baselines,
        }
    }

    fn water_alert_routing_request(
        trigger_alerts: bool,
        owned_field: bool,
    ) -> WaterAlertRoutingRequest {
        let water_needs =
            map_zone_water_need(zone_water_need_request(false)).expect("water needs should map");
        let savings_reports = if trigger_alerts {
            vec![super::WaterUseSavingsZoneReport {
                field_id: "field-north".to_string(),
                zone_ref: "zone:south".to_string(),
                status: WaterUseSavingsStatus::Computed,
                applied_amount_mm: 12.0,
                baseline_amount_mm: Some(5.0),
                savings_mm: Some(-7.0),
                baseline_method: Some("seasonal_baseline_v1".to_string()),
                evidence_refs: vec!["irrigation_event:event-over-001".to_string()],
            }]
        } else {
            vec![super::WaterUseSavingsZoneReport {
                field_id: "field-north".to_string(),
                zone_ref: "zone:south".to_string(),
                status: WaterUseSavingsStatus::Computed,
                applied_amount_mm: 5.0,
                baseline_amount_mm: Some(12.0),
                savings_mm: Some(7.0),
                baseline_method: Some("seasonal_baseline_v1".to_string()),
                evidence_refs: vec!["irrigation_event:event-safe-001".to_string()],
            }]
        };

        WaterAlertRoutingRequest {
            field_ref: " field-north ".to_string(),
            recipient_id: " grower-001 ".to_string(),
            owned_field_refs: if owned_field {
                vec!["field-north".to_string()]
            } else {
                vec!["field-south".to_string()]
            },
            routed_at: " 2026-06-13T12:05:00Z ".to_string(),
            thresholds: if trigger_alerts {
                WaterAlertThresholds {
                    low_moisture_water_need_mm: 2.0,
                    over_irrigation_mm: 5.0,
                }
            } else {
                WaterAlertThresholds {
                    low_moisture_water_need_mm: 99.0,
                    over_irrigation_mm: 99.0,
                }
            },
            water_needs,
            savings_reports,
            evidence_freshness: WeatherFreshnessState::Fresh,
            targets: vec![
                WeatherAlertRoutingTarget {
                    target: WeatherAlertRouteTarget::OperatorConsole,
                    reachable: true,
                },
                WeatherAlertRoutingTarget {
                    target: WeatherAlertRouteTarget::FarmersPortal,
                    reachable: true,
                },
            ],
        }
    }

    fn tractor_guidance_test_path() -> TractorGuidancePath {
        TractorGuidancePath {
            start: TractorGuidancePoint { x_m: 0.0, y_m: 0.0 },
            end: TractorGuidancePoint {
                x_m: 20.0,
                y_m: 0.0,
            },
        }
    }

    fn tractor_guidance_test_config(
        max_cross_track_error_m: f64,
        correction_gain: f64,
    ) -> TractorGuidanceConfig {
        TractorGuidanceConfig {
            runtime_mode: "simulation".to_string(),
            max_cross_track_error_m,
            correction_gain,
            advance_m_per_tick: 2.0,
            max_ticks: 10,
        }
    }

    fn tractor_swath_rectangle(
        min_lon: f64,
        min_lat: f64,
        max_lon: f64,
        max_lat: f64,
        crs: &str,
    ) -> FieldBoundary {
        FieldBoundary {
            coordinates: vec![
                GeoPoint {
                    longitude: min_lon,
                    latitude: min_lat,
                },
                GeoPoint {
                    longitude: max_lon,
                    latitude: min_lat,
                },
                GeoPoint {
                    longitude: max_lon,
                    latitude: max_lat,
                },
                GeoPoint {
                    longitude: min_lon,
                    latitude: max_lat,
                },
                GeoPoint {
                    longitude: min_lon,
                    latitude: min_lat,
                },
            ],
            crs: Some(crs.to_string()),
        }
    }

    fn tractor_field_ops_sample(
        timestamp: &str,
        x_m: f64,
        y_m: f64,
        speed_mps: f64,
        implement_enabled: bool,
    ) -> TractorFieldOpsTelemetrySample {
        TractorFieldOpsTelemetrySample {
            timestamp: timestamp.to_string(),
            position: TractorGuidancePoint { x_m, y_m },
            speed_mps,
            implement_enabled,
            implement_rate: Some(1.0),
        }
    }

    fn tractor_field_ops_session_request() -> TractorFieldOpsSessionRequest {
        TractorFieldOpsSessionRequest {
            session_id: "session-001".to_string(),
            tractor_id: "tractor-001".to_string(),
            field_id: "field-north".to_string(),
            started_at: "2026-06-15T10:00:00Z".to_string(),
            telemetry: vec![
                tractor_field_ops_sample("2026-06-15T10:00:00Z", 0.0, 0.0, 2.0, true),
                tractor_field_ops_sample("2026-06-15T10:00:01Z", 3.0, 0.0, 2.0, true),
                tractor_field_ops_sample("2026-06-15T10:00:02Z", 6.0, 0.0, 2.0, true),
            ],
            implement_width_m: 2.0,
            planned_area_m2: 24.0,
            max_telemetry_gap_seconds: 2,
        }
    }

    fn tractor_geofence_request(
        current_position: GeoPoint,
        predicted_position: GeoPoint,
        position_crs: &str,
    ) -> TractorGeofenceEvaluationRequest {
        TractorGeofenceEvaluationRequest {
            tractor_id: "tractor-001".to_string(),
            field_id: "field-north".to_string(),
            boundary_ref: "field-north-boundary".to_string(),
            boundary: tractor_swath_rectangle(0.0, 0.0, 10.0, 10.0, "EPSG:3857"),
            current_position,
            predicted_position,
            position_crs: position_crs.to_string(),
        }
    }

    fn tractor_motion_gate_command() -> TractorMotionCommandRequest {
        TractorMotionCommandRequest {
            command_id: Some("cmd-001".to_string()),
            tractor_id: "tractor-001".to_string(),
            command_type: "move".to_string(),
            requested_by: Some("ops@example.com".to_string()),
        }
    }

    fn tractor_operator_approval() -> TractorOperatorApproval {
        TractorOperatorApproval {
            approval_id: "approval-001".to_string(),
            tractor_id: "tractor-001".to_string(),
            approved_by: "ops@example.com".to_string(),
            approved_at: "2026-06-15T09:59:00Z".to_string(),
            expires_at: Some("2026-06-15T10:05:00Z".to_string()),
        }
    }

    fn tractor_allowed_motion_gate() -> super::TractorMotionGateEvaluation {
        evaluate_tractor_motion_gate(
            &tractor_motion_gate_command(),
            None,
            Some(&tractor_operator_approval()),
            "2026-06-15T10:00:02Z",
        )
        .expect("motion gate prerequisite should pass")
    }

    fn tractor_halted_motion_gate() -> super::TractorMotionGateEvaluation {
        evaluate_tractor_motion_gate(
            &tractor_motion_gate_command(),
            Some(&TractorEstopState {
                tractor_id: "tractor-001".to_string(),
                active: true,
                triggered_by: Some("ops@example.com".to_string()),
                triggered_at: Some("2026-06-15T10:00:00Z".to_string()),
                reason_code: Some("operator_estop".to_string()),
            }),
            Some(&tractor_operator_approval()),
            "2026-06-15T10:00:02Z",
        )
        .expect("estop halt should evaluate")
    }

    fn tractor_implement_spec() -> TractorImplementAdapterSpec {
        TractorImplementAdapterSpec {
            implement_id: "sprayer-001".to_string(),
            implement_type: "sprayer".to_string(),
            min_rate: 0.0,
            max_rate: 30.0,
            default_rate: 12.0,
        }
    }

    fn tractor_implement_state(enabled: bool, current_rate: f64) -> TractorImplementState {
        TractorImplementState {
            implement_id: "sprayer-001".to_string(),
            enabled,
            current_rate,
        }
    }

    fn tractor_weather_window_gate_request(
        window: Option<FieldOperationalWindow>,
    ) -> TractorWeatherWindowGateRequest {
        TractorWeatherWindowGateRequest {
            field_id: "field-north".to_string(),
            requested_start_at: "2026-06-15T10:00:02Z".to_string(),
            max_window_age_seconds: 900,
            window,
            motion_gate: tractor_allowed_motion_gate(),
        }
    }

    fn tractor_field_window(
        allowed: bool,
        fetched_at: &str,
        valid_from: &str,
        valid_until: &str,
        reason_code: &str,
    ) -> FieldOperationalWindow {
        FieldOperationalWindow {
            field_id: "field-north".to_string(),
            source: "domain-15".to_string(),
            fetched_at: fetched_at.to_string(),
            valid_from: valid_from.to_string(),
            valid_until: valid_until.to_string(),
            allowed,
            reason_code: reason_code.to_string(),
            gating_inputs: vec!["wind_mps:3.2".to_string(), "precip_mm:0.0".to_string()],
        }
    }

    fn tractor_swath_reservation(
        tractor_id: &str,
        priority: u8,
        y_m: f64,
        start_x_m: f64,
        end_x_m: f64,
    ) -> TractorSwathReservation {
        TractorSwathReservation {
            tractor_id: tractor_id.to_string(),
            swath: TractorSwathSegment {
                start: GeoPoint {
                    longitude: start_x_m,
                    latitude: y_m,
                },
                end: GeoPoint {
                    longitude: end_x_m,
                    latitude: y_m,
                },
                width_m: 1.0,
            },
            priority,
            starts_at: "2026-06-15T10:00:00Z".to_string(),
            ends_at: "2026-06-15T10:30:00Z".to_string(),
            geofence: evaluate_tractor_geofence(tractor_geofence_request(
                GeoPoint {
                    longitude: 2.0,
                    latitude: 2.0,
                },
                GeoPoint {
                    longitude: 8.0,
                    latitude: 8.0,
                },
                "EPSG:3857",
            ))
            .expect("geofence prerequisite should pass"),
            motion_gate: tractor_allowed_motion_gate(),
            obstacle: TractorObstacleDetection {
                tractor_id: tractor_id.to_string(),
                halted: false,
                event: None,
            },
        }
    }

    fn weather_field_resolution_request(
        field_geometry: Option<(FieldBoundary, GeoPoint, &str)>,
    ) -> WeatherFieldForecastResolutionRequest {
        let (boundary, forecast_location, forecast_crs) = field_geometry
            .map(|(boundary, location, crs)| (Some(boundary), location, crs.to_string()))
            .unwrap_or((
                None,
                GeoPoint {
                    longitude: 5.0,
                    latitude: 5.0,
                },
                "EPSG:4326".to_string(),
            ));
        WeatherFieldForecastResolutionRequest {
            field_id: "field-north".to_string(),
            boundary,
            forecast_location,
            forecast_crs,
            records: normalize_weather_provider_forecast(
                "station-alpha".to_string(),
                WeatherProviderForecastResponse {
                    source: "NOAA-HRRR".to_string(),
                    fetched_at: "2026-06-13T10:00:00Z".to_string(),
                    points: vec![WeatherProviderForecastPoint {
                        valid_time: "2026-06-13T11:00:00Z".to_string(),
                        temperature_celsius: 22.5,
                        wind_speed_mps: 4.2,
                        precipitation_mm: 0.1,
                        humidity_percent: 64.0,
                        radiation_w_m2: 720.0,
                    }],
                },
            )
            .expect("weather fixture should normalize"),
        }
    }

    fn weather_forecast_value(fetched_at: &str, valid_time: &str) -> WeatherForecastValue {
        WeatherForecastValue {
            value: 22.5,
            unit: "deg_c".to_string(),
            source: "NOAA-HRRR".to_string(),
            fetched_at: fetched_at.to_string(),
            valid_time: valid_time.to_string(),
        }
    }

    fn weather_sensor_stream_request(
        samples: Vec<WeatherSensorSample>,
    ) -> WeatherSensorStreamIngestRequest {
        WeatherSensorStreamIngestRequest {
            sensor_id: "sensor-north-001".to_string(),
            field_ref: "field-north".to_string(),
            fetched_at: "2026-06-13T10:10:00Z".to_string(),
            evaluated_at: "2026-06-13T10:10:00Z".to_string(),
            stale_after_seconds: 900,
            max_gap_seconds: 600,
            samples,
        }
    }

    fn weather_sensor_sample(observed_at: &str, temperature_celsius: f64) -> WeatherSensorSample {
        WeatherSensorSample {
            observed_at: observed_at.to_string(),
            temperature_celsius,
            wind_speed_mps: 4.2,
            precipitation_mm: 0.0,
            humidity_percent: 64.0,
            radiation_w_m2: 720.0,
        }
    }

    fn weather_history_query(
        field_ref: &str,
        start_time: &str,
        end_time: &str,
    ) -> WeatherHistoryQuery {
        WeatherHistoryQuery {
            field_ref: field_ref.to_string(),
            start_time: start_time.to_string(),
            end_time: end_time.to_string(),
            offset: 0,
            limit: 50,
        }
    }

    fn weather_window_request(
        records: Vec<super::WeatherFreshnessAnnotatedRecord>,
    ) -> WeatherOperationalWindowRequest {
        WeatherOperationalWindowRequest {
            field_ref: "field-north".to_string(),
            thresholds: WeatherOperationalWindowThresholds {
                max_wind_speed_mps: 6.0,
                max_precipitation_mm: 0.5,
                min_temperature_celsius: 5.0,
                max_temperature_celsius: 32.0,
            },
            records,
        }
    }

    fn weather_window_record(
        valid_time: &str,
        wind_speed_mps: f64,
        precipitation_mm: f64,
        temperature_celsius: f64,
        stale: bool,
    ) -> super::WeatherFreshnessAnnotatedRecord {
        let fetched_at = if stale {
            "2026-06-13T08:00:00Z"
        } else {
            "2026-06-13T09:55:00Z"
        };
        let record = normalize_weather_provider_forecast(
            "field-north".to_string(),
            WeatherProviderForecastResponse {
                source: "NOAA-HRRR".to_string(),
                fetched_at: fetched_at.to_string(),
                points: vec![WeatherProviderForecastPoint {
                    valid_time: valid_time.to_string(),
                    temperature_celsius,
                    wind_speed_mps,
                    precipitation_mm,
                    humidity_percent: 64.0,
                    radiation_w_m2: 720.0,
                }],
            },
        )
        .expect("weather window fixture should normalize")
        .remove(0);
        annotate_weather_record_freshness(record, "2026-06-13T10:00:00Z", 900)
            .expect("weather window fixture should annotate")
    }

    fn weather_risk_thresholds() -> WeatherRiskThresholds {
        WeatherRiskThresholds {
            frost_temperature_celsius: 2.0,
            heat_temperature_celsius: 35.0,
            wind_speed_mps: 10.0,
            precipitation_mm: 1.0,
        }
    }

    fn weather_gdd_request(
        date: &str,
        records: Vec<super::WeatherFreshnessAnnotatedRecord>,
    ) -> WeatherGrowingDegreeDayRequest {
        WeatherGrowingDegreeDayRequest {
            field_ref: "field-north".to_string(),
            date: date.to_string(),
            base_temperature_celsius: 10.0,
            records,
        }
    }

    fn weather_reference_et_input(include_radiation: bool) -> WeatherReferenceEtInput {
        let record = weather_window_record("2026-06-13T10:00:00Z", 4.2, 0.0, 22.0, false);
        WeatherReferenceEtInput {
            field_ref: "field-north".to_string(),
            date: "2026-06-13".to_string(),
            temperature_celsius: Some(record.temperature_celsius.clone()),
            humidity_percent: Some(record.humidity_percent.clone()),
            wind_speed_mps: Some(record.wind_speed_mps.clone()),
            radiation_w_m2: include_radiation.then_some(record.radiation_w_m2.clone()),
        }
    }

    fn water_weather_input_contract_request(stale: bool) -> WaterWeatherInputContractRequest {
        WaterWeatherInputContractRequest {
            field_ref: " field-north ".to_string(),
            date: "2026-06-13".to_string(),
            records: vec![weather_window_record(
                "2026-06-13T10:00:00Z",
                4.2,
                0.0,
                22.0,
                stale,
            )],
        }
    }

    fn zone_water_need_request(crs_mismatch: bool) -> ZoneWaterNeedRequest {
        let field = tractor_test_farm_fields()
            .field_by_id("field-north")
            .expect("test field exists");
        let moisture_proxies =
            ingest_remote_sensing_moisture_proxy_layer(moisture_proxy_layer("EPSG:4326"), &field)
                .expect("proxy fixture should ingest");
        let weather =
            validate_water_weather_input_contract(water_weather_input_contract_request(false))
                .expect("fresh weather input contract should validate");
        let evapotranspiration = compute_water_evapotranspiration(WaterEvapotranspirationRequest {
            weather,
            crop_coefficient: Some(1.15),
        })
        .expect("water ET fixture should compute");

        ZoneWaterNeedRequest {
            field_ref: " field-north ".to_string(),
            crs: " EPSG:4326 ".to_string(),
            zones: vec![
                WaterNeedZone {
                    zone_ref: " zone:north ".to_string(),
                    crs: if crs_mismatch {
                        "EPSG:3857".to_string()
                    } else {
                        "EPSG:4326".to_string()
                    },
                },
                WaterNeedZone {
                    zone_ref: "zone:missing".to_string(),
                    crs: "EPSG:4326".to_string(),
                },
            ],
            soil_readings: Vec::new(),
            moisture_proxies,
            evapotranspiration,
        }
    }

    fn irrigation_schedule_request(water_needs: Vec<ZoneWaterNeed>) -> IrrigationScheduleRequest {
        IrrigationScheduleRequest {
            field_ref: " field-north ".to_string(),
            generated_at: " 2026-06-13T11:55:00Z ".to_string(),
            scheduled_start: " 2026-06-13T12:00:00Z ".to_string(),
            application_rate_mm_per_hour: 5.0,
            water_needs,
        }
    }

    fn irrigation_schedule_fixture() -> super::IrrigationSchedule {
        let needs = map_zone_water_need(zone_water_need_request(false))
            .expect("zone water need should map");
        schedule_irrigation_plan(irrigation_schedule_request(needs))
            .expect("schedule fixture should build")
    }

    fn irrigation_valve_dry_run_request(
        schedule: super::IrrigationSchedule,
        max_amount_mm: f64,
    ) -> IrrigationValveDryRunRequest {
        IrrigationValveDryRunRequest {
            dry_run_id: " dry-run-001 ".to_string(),
            checked_at: " 2026-06-13T11:58:00Z ".to_string(),
            schedule,
            valves: vec![IrrigationValveSpec {
                zone_ref: " zone:north ".to_string(),
                min_amount_mm: 0.0,
                max_amount_mm,
                max_duration_minutes: 60,
            }],
        }
    }

    fn irrigation_valve_execute_request(
        schedule: super::IrrigationSchedule,
        dry_run: super::IrrigationValveDryRun,
        abort_requested: bool,
    ) -> IrrigationValveExecuteRequest {
        IrrigationValveExecuteRequest {
            executed_at: " 2026-06-13T12:00:00Z ".to_string(),
            schedule,
            dry_run,
            abort_requested,
        }
    }

    fn weather_alert_routing_request(
        targets: Vec<WeatherAlertRoutingTarget>,
    ) -> WeatherAlertRoutingRequest {
        let alert = evaluate_weather_risk_alerts(
            &[weather_window_record(
                "2026-06-13T10:00:00Z",
                12.0,
                0.0,
                22.0,
                false,
            )],
            weather_risk_thresholds(),
        )
        .expect("alert fixture should evaluate")
        .into_iter()
        .find(|alert| alert.risk_type == WeatherRiskType::Wind)
        .expect("wind alert should be present");

        WeatherAlertRoutingRequest {
            alert,
            recipient_id: "grower-001".to_string(),
            owned_field_refs: vec!["field-north".to_string()],
            targets,
            routed_at: "2026-06-13T10:01:00Z".to_string(),
        }
    }

    fn weather_crop_stage_request(crop_stage: Option<&str>) -> WeatherCropStageRiskRequest {
        WeatherCropStageRiskRequest {
            field_ref: "field-north".to_string(),
            crop_stage: crop_stage.map(str::to_string),
            default_thresholds: weather_risk_thresholds(),
            stage_thresholds: vec![WeatherCropStageThresholdSet {
                crop_stage: "flowering".to_string(),
                threshold_set_name: "flowering_frost_sensitive".to_string(),
                thresholds: WeatherRiskThresholds {
                    frost_temperature_celsius: 5.0,
                    heat_temperature_celsius: 35.0,
                    wind_speed_mps: 10.0,
                    precipitation_mm: 1.0,
                },
            }],
            records: vec![weather_window_record(
                "2026-06-13T04:00:00Z",
                3.0,
                0.0,
                4.0,
                false,
            )],
        }
    }

    fn weather_forecast_verification_request(
        with_observation: bool,
    ) -> WeatherForecastVerificationRequest {
        let forecast = weather_forecast_record(
            "field-north",
            "NOAA-HRRR",
            "2026-06-13T09:55:00Z",
            "2026-06-13T10:00:00Z",
            22.0,
            4.0,
            0.2,
            60.0,
            700.0,
        );
        let observation_time = if with_observation {
            "2026-06-13T10:00:00Z"
        } else {
            "2026-06-13T10:05:00Z"
        };
        let observation = weather_forecast_record(
            "field-north",
            "sensor",
            observation_time,
            observation_time,
            20.5,
            5.0,
            0.1,
            65.0,
            720.0,
        );
        let observations =
            vec![
                annotate_weather_record_freshness(observation, "2026-06-13T10:10:00Z", 900)
                    .expect("observation fixture should annotate"),
            ];

        WeatherForecastVerificationRequest {
            forecast,
            observations,
        }
    }

    fn weather_forecast_record(
        field_ref: &str,
        source: &str,
        fetched_at: &str,
        valid_time: &str,
        temperature_celsius: f64,
        wind_speed_mps: f64,
        precipitation_mm: f64,
        humidity_percent: f64,
        radiation_w_m2: f64,
    ) -> WeatherForecastRecord {
        normalize_weather_provider_forecast(
            field_ref.to_string(),
            WeatherProviderForecastResponse {
                source: source.to_string(),
                fetched_at: fetched_at.to_string(),
                points: vec![WeatherProviderForecastPoint {
                    valid_time: valid_time.to_string(),
                    temperature_celsius,
                    wind_speed_mps,
                    precipitation_mm,
                    humidity_percent,
                    radiation_w_m2,
                }],
            },
        )
        .expect("forecast fixture should normalize")
        .remove(0)
    }

    fn tractor_prescription_request(
        zones: Vec<TractorPrescriptionZone>,
    ) -> TractorPrescriptionExecutionRequest {
        TractorPrescriptionExecutionRequest {
            runtime_mode: "simulation".to_string(),
            field_id: "field-north".to_string(),
            field_crs: "EPSG:3857".to_string(),
            field_extent: GeoBounds {
                min_lon: 0.0,
                min_lat: 0.0,
                max_lon: 10.0,
                max_lat: 10.0,
            },
            zones,
            geofence: evaluate_tractor_geofence(tractor_geofence_request(
                GeoPoint {
                    longitude: 2.0,
                    latitude: 2.0,
                },
                GeoPoint {
                    longitude: 8.0,
                    latitude: 8.0,
                },
                "EPSG:3857",
            ))
            .expect("geofence prerequisite should pass"),
            motion_gate: evaluate_tractor_motion_gate(
                &tractor_motion_gate_command(),
                None,
                Some(&tractor_operator_approval()),
                "2026-06-15T10:00:02Z",
            )
            .expect("motion gate prerequisite should pass"),
            obstacle: TractorObstacleDetection {
                tractor_id: "tractor-001".to_string(),
                halted: false,
                event: None,
            },
        }
    }

    fn tractor_prescription_zone(
        zone_id: &str,
        crs: &str,
        min_lon: f64,
        min_lat: f64,
        max_lon: f64,
        max_lat: f64,
        rate: f64,
    ) -> TractorPrescriptionZone {
        TractorPrescriptionZone {
            zone_id: zone_id.to_string(),
            crs: crs.to_string(),
            extent: GeoBounds {
                min_lon,
                min_lat,
                max_lon,
                max_lat,
            },
            rate,
            evidence_refs: vec![format!("zone:{zone_id}")],
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
    fn open_data_publication_accepts_license_attribution_and_anonymizes() {
        let publication = super::prepare_open_data_publication(
            OpenDataPublishRequest {
                source_layer_ref: "scene-alpha:ndvi".to_string(),
                license: " CC-BY-4.0 ".to_string(),
                attribution: " AGBot demo ".to_string(),
                owner_identifier: None,
                field_identifier: None,
            },
            "open-data:scene-alpha:ndvi".to_string(),
        )
        .expect("publication should pass");

        assert_eq!(publication.open_data_id, "open-data:scene-alpha:ndvi");
        assert_eq!(publication.source_layer_ref, "scene-alpha:ndvi");
        assert_eq!(publication.license, "CC-BY-4.0");
        assert_eq!(publication.attribution, "AGBot demo");
        assert!(publication.anonymized);
    }

    #[test]
    fn open_data_publication_rejects_missing_license() {
        let error = super::prepare_open_data_publication(
            OpenDataPublishRequest {
                source_layer_ref: "scene-alpha:ndvi".to_string(),
                license: " ".to_string(),
                attribution: "AGBot demo".to_string(),
                owner_identifier: None,
                field_identifier: None,
            },
            "open-data:scene-alpha:ndvi".to_string(),
        )
        .expect_err("missing license should reject");

        assert_eq!(
            error,
            OpenDataPublishError::Refused {
                reason: OpenDataPublishRefusalReason::MissingLicense,
            }
        );
    }

    #[test]
    fn open_data_publication_rejects_deanonymizing_field_identifier() {
        let error = super::prepare_open_data_publication(
            OpenDataPublishRequest {
                source_layer_ref: "scene-alpha:ndvi".to_string(),
                license: "CC-BY-4.0".to_string(),
                attribution: "AGBot demo".to_string(),
                owner_identifier: None,
                field_identifier: Some("field-alpha".to_string()),
            },
            "open-data:scene-alpha:ndvi".to_string(),
        )
        .expect_err("field identifier should reject");

        assert_eq!(
            error,
            OpenDataPublishError::Refused {
                reason: OpenDataPublishRefusalReason::FieldIdentifierPresent,
            }
        );
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
    fn next_season_rollover_suggestion_uses_latest_history_and_cites_sources() {
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
        registry
            .insert_crop_plan(CropPlanRecord {
                crop_plan_id: "plan-2026".to_string(),
                season_id: "season-2026".to_string(),
                org_id: String::new(),
                crop: "corn".to_string(),
                planting_date: Some("2026-04-15".to_string()),
            })
            .expect("crop plan persists");

        let suggestion = registry
            .suggest_next_season_rollover("org-a", "field-a")
            .expect("suggestion should derive from history");

        assert!(suggestion.requires_approval);
        assert!(suggestion.has_proposal());
        assert_eq!(
            suggestion.source_history_refs,
            vec![
                "season:season-2026".to_string(),
                "crop_plan:plan-2026".to_string()
            ]
        );
        let proposed_season = suggestion
            .proposed_season
            .expect("season proposal should exist");
        assert_eq!(proposed_season.season_id, "season-field-a-2027");
        assert_eq!(proposed_season.start, "2027-03-01");
        assert_eq!(proposed_season.end, "2027-10-31");
        assert_eq!(proposed_season.label, "2027 corn");
        let proposed_crop_plan = suggestion
            .proposed_crop_plan
            .expect("crop-plan proposal should exist");
        assert_eq!(proposed_crop_plan.crop_plan_id, "plan-field-a-2027");
        assert_eq!(proposed_crop_plan.season_id, "season-field-a-2027");
        assert_eq!(proposed_crop_plan.crop, "corn");
        assert_eq!(
            proposed_crop_plan.planting_date.as_deref(),
            Some("2027-04-15")
        );
    }

    #[test]
    fn rollover_suggestion_does_not_write_without_approval() {
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
            .insert_crop_plan(CropPlanRecord {
                crop_plan_id: "plan-2026".to_string(),
                season_id: "season-2026".to_string(),
                org_id: String::new(),
                crop: "corn".to_string(),
                planting_date: Some("2026-04-15".to_string()),
            })
            .expect("crop plan persists");

        let before = registry.season_history_for_field("org-a", "field-a");
        let suggestion = registry
            .suggest_next_season_rollover("org-a", "field-a")
            .expect("suggestion should not mutate");
        let after = registry.season_history_for_field("org-a", "field-a");

        assert_eq!(before, after);
        assert!(suggestion.requires_approval);
        assert_eq!(
            suggestion.proposed_season.map(|season| season.season_id),
            Some("season-field-a-2027".to_string())
        );
    }

    #[test]
    fn rollover_suggestion_reports_no_basis_without_history() {
        let registry = registry_with_field();

        let suggestion = registry
            .suggest_next_season_rollover("org-a", "field-a")
            .expect("empty history should be explicit");

        assert!(!suggestion.has_proposal());
        assert!(suggestion.requires_approval);
        assert_eq!(suggestion.source_history_refs, Vec::<String>::new());
        assert_eq!(
            suggestion.no_basis_reason.as_deref(),
            Some("no persisted season history for field")
        );
    }

    #[test]
    fn access_anomaly_flags_denied_cross_org_spike_with_evidence() {
        let events = vec![
            access_event("audit-1", "actor-a", AccessAuditDecision::Denied),
            access_event("audit-2", "actor-a", AccessAuditDecision::Denied),
            access_event("audit-3", "actor-a", AccessAuditDecision::Denied),
            AccessAuditEvent {
                audit_id: "audit-other".to_string(),
                actor_id: "actor-b".to_string(),
                org_id: "org-a".to_string(),
                target_org_id: Some("org-b".to_string()),
                action: "field:read".to_string(),
                decision: AccessAuditDecision::Denied,
                reason_code: Some("cross_org_denied".to_string()),
                at: "2026-06-12T10:04:00Z".to_string(),
            },
        ];

        let advisories =
            evaluate_access_anomaly_advisories(&events, AccessAnomalyThresholds::default());

        assert_eq!(advisories.len(), 1);
        assert_eq!(advisories[0].actor_id, "actor-a");
        assert_eq!(advisories[0].signal, AccessAnomalySignal::CrossOrgProbe);
        assert_eq!(advisories[0].observed_count, 3);
        assert_eq!(
            advisories[0].evidence_audit_ids,
            vec![
                "audit-1".to_string(),
                "audit-2".to_string(),
                "audit-3".to_string()
            ]
        );
    }

    #[test]
    fn access_anomaly_baseline_traffic_has_no_false_positive() {
        let events = vec![
            AccessAuditEvent {
                audit_id: "audit-allowed-read".to_string(),
                actor_id: "actor-a".to_string(),
                org_id: "org-a".to_string(),
                target_org_id: Some("org-a".to_string()),
                action: "field:read".to_string(),
                decision: AccessAuditDecision::Allowed,
                reason_code: None,
                at: "2026-06-12T10:00:00Z".to_string(),
            },
            access_event("audit-denied-one", "actor-a", AccessAuditDecision::Denied),
            AccessAuditEvent {
                audit_id: "audit-export-one".to_string(),
                actor_id: "actor-a".to_string(),
                org_id: "org-a".to_string(),
                target_org_id: Some("org-a".to_string()),
                action: "field_records:export".to_string(),
                decision: AccessAuditDecision::Allowed,
                reason_code: None,
                at: "2026-06-12T10:02:00Z".to_string(),
            },
        ];

        let advisories =
            evaluate_access_anomaly_advisories(&events, AccessAnomalyThresholds::default());

        assert!(advisories.is_empty());
    }

    #[test]
    fn access_anomaly_is_advisory_only_not_auto_blocking() {
        let events = (1..=5)
            .map(|index| AccessAuditEvent {
                audit_id: format!("audit-export-{index}"),
                actor_id: "actor-a".to_string(),
                org_id: "org-a".to_string(),
                target_org_id: Some("org-a".to_string()),
                action: "field_records:export".to_string(),
                decision: AccessAuditDecision::Allowed,
                reason_code: None,
                at: format!("2026-06-12T10:0{index}:00Z"),
            })
            .collect::<Vec<_>>();

        let advisories =
            evaluate_access_anomaly_advisories(&events, AccessAnomalyThresholds::default());

        assert_eq!(advisories.len(), 1);
        assert_eq!(advisories[0].signal, AccessAnomalySignal::BulkExport);
        assert!(advisories[0].requires_approval);
        assert!(!advisories[0].auto_blocked);
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

    fn access_event(
        audit_id: &str,
        actor_id: &str,
        decision: AccessAuditDecision,
    ) -> AccessAuditEvent {
        AccessAuditEvent {
            audit_id: audit_id.to_string(),
            actor_id: actor_id.to_string(),
            org_id: "org-a".to_string(),
            target_org_id: Some("org-b".to_string()),
            action: "field:read".to_string(),
            decision,
            reason_code: Some("cross_org_denied".to_string()),
            at: "2026-06-12T10:00:00Z".to_string(),
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
