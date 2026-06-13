use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

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
}

impl FleetNodeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            FleetNodeStatus::Enrolled => "enrolled",
        }
    }
}

impl std::str::FromStr for FleetNodeStatus {
    type Err = FleetNodeEnrollmentError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "enrolled" => Ok(FleetNodeStatus::Enrolled),
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

    if bundle.signature.is_empty() {
        return Ok(rejected_config_outcome(
            current_state,
            bundle.version,
            FleetConfigRejectionReason::MissingSignature,
        ));
    }

    let expected_signature = sign_fleet_config_bundle(
        &bundle.node_id,
        bundle.version,
        &bundle.payload,
        &verifying_key,
    );
    if bundle.signature != expected_signature {
        return Ok(rejected_config_outcome(
            current_state,
            bundle.version,
            FleetConfigRejectionReason::InvalidSignature,
        ));
    }

    if bundle.version <= current_state.applied_version {
        return Ok(rejected_config_outcome(
            current_state,
            bundle.version,
            FleetConfigRejectionReason::OlderOrEqualVersion,
        ));
    }

    let applied_at =
        normalize_config_text(applied_at, FleetConfigDistributionError::EmptyAppliedAt)?;
    let previous_version = current_state.applied_version;
    let updated_state = FleetConfigState {
        node_id: current_state.node_id.clone(),
        applied_version: bundle.version,
        payload: bundle.payload,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkOrderChangeType {
    Created,
    StatusChanged,
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
        apply_fleet_node_heartbeat, assert_flight_operation_allowed, assert_raster_spatial_ref,
        bind_fleet_node_identity, bounds_from_points, sign_fleet_config_bundle,
        validate_field_boundary, verify_and_apply_fleet_config_bundle, AnnotationAuditRegistry,
        AnnotationChangeType, AnnotationGeometry, AnnotationPersistenceError, AnnotationRecord,
        AuditedAnnotationRecord, CropPlanRecord, FarmFieldEntityStatus, FarmFieldError,
        FarmFieldListQuery, FarmFieldRegistry, FarmRecord, FieldBoundary,
        FieldBoundaryValidationError, FieldRecord, FleetConfigApplyStatus, FleetConfigBundle,
        FleetConfigRejectionReason, FleetConfigState, FleetHeartbeatEvaluation,
        FleetNodeComponentHealth, FleetNodeComponentStatus, FleetNodeEnrollmentError,
        FleetNodeEnrollmentRequest, FleetNodeHealthState, FleetNodeHeartbeat, FleetNodeKind,
        FleetNodeOperationError, FleetNodeRecord, FleetNodeRuntimeMode, FleetNodeStatus, GeoBounds,
        GeoPoint, MultispectralImage, RasterResolution, RasterSpatialRef, RasterSpatialRefError,
        RecommendationLifecycleRegistry, RecommendationPersistenceError, RecommendationPriority,
        RecommendationRecord, RecommendationStatus, RecommendationStatusChangeType,
        ReportDeliverableRegistry, ReportFormat, ReportPersistenceError, ReportRecord,
        ReportVisibility, SceneLayerMetadataError, SceneLayerRecord, SceneRecord, SeasonRecord,
        WorkOrderChangeType, WorkOrderCreateRequest, WorkOrderPersistenceError, WorkOrderRegistry,
        WorkOrderStatus,
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
