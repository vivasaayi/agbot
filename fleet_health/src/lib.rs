use serde::{Deserialize, Serialize};
use shared::{fleet_alerts::FleetAlertRecord, schemas::FleetVersionInventory};
use timeseries::{SeriesPoint, SeriesValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetComponentType {
    Airframe,
    Battery,
    Controller,
    Esc,
    Motor,
    Propeller,
    Sensor,
}

impl FleetComponentType {
    pub fn as_str(self) -> &'static str {
        match self {
            FleetComponentType::Airframe => "airframe",
            FleetComponentType::Battery => "battery",
            FleetComponentType::Controller => "controller",
            FleetComponentType::Esc => "esc",
            FleetComponentType::Motor => "motor",
            FleetComponentType::Propeller => "propeller",
            FleetComponentType::Sensor => "sensor",
        }
    }
}

impl std::str::FromStr for FleetComponentType {
    type Err = FleetHealthError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "airframe" => Ok(Self::Airframe),
            "battery" => Ok(Self::Battery),
            "controller" => Ok(Self::Controller),
            "esc" => Ok(Self::Esc),
            "motor" => Ok(Self::Motor),
            "propeller" => Ok(Self::Propeller),
            "sensor" => Ok(Self::Sensor),
            _ => Err(FleetHealthError::UnsupportedComponentType {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceHistoryEntry {
    #[serde(default)]
    pub service_id: String,
    #[serde(default)]
    pub performed_at: String,
    #[serde(default)]
    pub technician: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceWorkOrderSeverity {
    Routine,
    Degraded,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceWorkOrderStatus {
    Open,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaintenancePartUsage {
    pub part_id: String,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OpenMaintenanceWorkOrderRequest {
    #[serde(default)]
    pub wo_id: Option<String>,
    #[serde(default)]
    pub component_id: String,
    #[serde(default)]
    pub reason: String,
    pub severity: MaintenanceWorkOrderSeverity,
    #[serde(default)]
    pub opened_at: String,
    #[serde(default)]
    pub technician: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CloseMaintenanceWorkOrderRequest {
    #[serde(default)]
    pub closed_at: String,
    #[serde(default)]
    pub technician: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub parts: Vec<MaintenancePartUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaintenanceWorkOrder {
    pub wo_id: String,
    pub component_id: String,
    pub reason: String,
    pub severity: MaintenanceWorkOrderSeverity,
    pub status: MaintenanceWorkOrderStatus,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub technician: String,
    pub parts: Vec<MaintenancePartUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaintenanceWorkOrderCloseResult {
    pub component: FleetComponentRecord,
    pub work_order: MaintenanceWorkOrder,
    pub service_history_entry: ServiceHistoryEntry,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RegisterComponentRequest {
    #[serde(default)]
    pub component_id: Option<String>,
    pub component_type: FleetComponentType,
    #[serde(default)]
    pub serial: String,
    #[serde(default)]
    pub airframe_id: Option<String>,
    #[serde(default)]
    pub installed_at: Option<String>,
    #[serde(default)]
    pub removed_at: Option<String>,
    #[serde(default)]
    pub service_history: Vec<ServiceHistoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct InstallComponentRequest {
    #[serde(default)]
    pub airframe_id: String,
    #[serde(default)]
    pub installed_at: String,
    #[serde(default)]
    pub actor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DutyAccrualRequest {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub airframe_id: String,
    pub flight_hours: f64,
    #[serde(default)]
    pub cycles: u32,
    pub duty_score: f64,
    #[serde(default)]
    pub ended_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TelemetryHealthIndicatorRequest {
    #[serde(default)]
    pub source_ref: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub samples: Vec<HealthTelemetrySample>,
    #[serde(default)]
    pub telemetry_gaps: Vec<HealthTelemetryGap>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthTelemetrySample {
    #[serde(default)]
    pub component_id: String,
    pub component_type: FleetComponentType,
    #[serde(default)]
    pub ts: String,
    #[serde(default)]
    pub battery_open_circuit_voltage_v: Option<f64>,
    #[serde(default)]
    pub battery_voltage_v: Option<f64>,
    #[serde(default)]
    pub battery_current_a: Option<f64>,
    #[serde(default)]
    pub motor_vibration_g: Option<f64>,
    #[serde(default)]
    pub esc_temperature_c: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthTelemetryGap {
    #[serde(default)]
    pub component_id: String,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub ended_at: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetHealthIndicator {
    BatteryCycleCount,
    BatteryInternalResistance,
    MotorVibration,
    EscTemperature,
}

impl FleetHealthIndicator {
    pub fn as_str(self) -> &'static str {
        match self {
            FleetHealthIndicator::BatteryCycleCount => "battery_cycle_count",
            FleetHealthIndicator::BatteryInternalResistance => {
                "battery_internal_resistance_milliohm"
            }
            FleetHealthIndicator::MotorVibration => "motor_vibration_g",
            FleetHealthIndicator::EscTemperature => "esc_temperature_c",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            FleetHealthIndicator::BatteryCycleCount => "cycles",
            FleetHealthIndicator::BatteryInternalResistance => "milliohm",
            FleetHealthIndicator::MotorVibration => "g",
            FleetHealthIndicator::EscTemperature => "celsius",
        }
    }
}

impl std::str::FromStr for FleetHealthIndicator {
    type Err = FleetHealthError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "battery_cycle_count" | "battery_cycles" => Ok(Self::BatteryCycleCount),
            "battery_internal_resistance_milliohm" | "battery_internal_resistance" => {
                Ok(Self::BatteryInternalResistance)
            }
            "motor_vibration_g" | "motor_vibration" => Ok(Self::MotorVibration),
            "esc_temperature_c" | "esc_temperature" => Ok(Self::EscTemperature),
            _ => Err(FleetHealthError::UnsupportedHealthIndicator {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthIndicatorFreshness {
    Fresh,
    Stale,
}

impl HealthIndicatorFreshness {
    pub fn as_str(self) -> &'static str {
        match self {
            HealthIndicatorFreshness::Fresh => "fresh",
            HealthIndicatorFreshness::Stale => "stale",
        }
    }
}

impl std::str::FromStr for HealthIndicatorFreshness {
    type Err = FleetHealthError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "fresh" => Ok(Self::Fresh),
            "stale" => Ok(Self::Stale),
            _ => Err(FleetHealthError::UnsupportedHealthFreshness {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetHealthIndicatorSample {
    pub component_id: String,
    pub indicator: FleetHealthIndicator,
    pub value: f64,
    pub ts: String,
    pub source_ref: String,
    pub created_at: String,
    pub freshness: HealthIndicatorFreshness,
}

impl FleetHealthIndicatorSample {
    pub fn to_series_point(&self) -> SeriesPoint {
        SeriesPoint {
            entity_ref: format!("component:{}", self.component_id),
            metric: self.indicator.as_str().to_string(),
            unit: self.indicator.unit().to_string(),
            t: self.ts.clone(),
            value: SeriesValue::Scalar { value: self.value },
            source_ref: self.source_ref.clone(),
            created_at: self.created_at.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetHealthIndicatorDerivation {
    pub samples: Vec<FleetHealthIndicatorSample>,
    pub gaps: Vec<HealthTelemetryGap>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BatteryHealthTrendConfig {
    pub max_cycles: u32,
    pub resistance_degraded_at_milliohm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatteryHealthTrendReport {
    pub component_id: String,
    pub evaluated_at: String,
    pub cycle_count: u32,
    pub max_cycles: u32,
    pub latest_internal_resistance_milliohm: f64,
    pub resistance_sample_count: usize,
    pub status: ComponentHealthVerdictStatus,
    pub evidence: Vec<HealthVerdictEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HealthDegradationDetectionConfig {
    pub min_history_points: usize,
    pub recent_window_points: usize,
    pub min_adverse_slope_per_day: f64,
    pub min_adverse_delta: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct HealthDegradationDetectionRequest {
    #[serde(default)]
    pub component_id: String,
    #[serde(default)]
    pub evaluated_at: String,
    #[serde(default)]
    pub method_version: String,
    pub indicator: FleetHealthIndicator,
    #[serde(default)]
    pub samples: Vec<FleetHealthIndicatorSample>,
    pub config: HealthDegradationDetectionConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthDegradationDetectionStatus {
    Stable,
    DegradationDetected,
    InsufficientHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthDegradationReasonCode {
    WithinTrendBand,
    SustainedAdverseSlope,
    DriftExceeded,
    InsufficientHistory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthDegradationEvent {
    pub component_id: String,
    pub indicator: FleetHealthIndicator,
    pub reason_code: HealthDegradationReasonCode,
    pub window_start: String,
    pub window_end: String,
    pub slope_per_day: f64,
    pub delta: f64,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthDegradationDetectionReport {
    pub component_id: String,
    pub evaluated_at: String,
    pub method_version: String,
    pub indicator: FleetHealthIndicator,
    pub status: HealthDegradationDetectionStatus,
    pub reason_code: HealthDegradationReasonCode,
    pub window_start: Option<String>,
    pub window_end: Option<String>,
    pub slope_per_day: Option<f64>,
    pub delta: Option<f64>,
    pub evidence_refs: Vec<String>,
    pub event: Option<HealthDegradationEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthEvidenceSubjectKind {
    ComponentVerdict,
    DegradationEvent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthEvidenceInputRef {
    pub ref_kind: String,
    pub ref_id: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthEvidenceRecord {
    pub record_id: String,
    pub subject_kind: HealthEvidenceSubjectKind,
    pub component_id: String,
    pub method_version: String,
    pub reason_code: String,
    pub recorded_at: String,
    pub input_refs: Vec<HealthEvidenceInputRef>,
    pub decision_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentHealthVerdictStatus {
    Ok,
    Watch,
    Degraded,
    Critical,
}

impl ComponentHealthVerdictStatus {
    fn severity_rank(self) -> u8 {
        match self {
            ComponentHealthVerdictStatus::Ok => 0,
            ComponentHealthVerdictStatus::Watch => 1,
            ComponentHealthVerdictStatus::Degraded => 2,
            ComponentHealthVerdictStatus::Critical => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthVerdictReasonCode {
    AllIndicatorsWithinThreshold,
    WatchThresholdExceeded,
    DegradedThresholdExceeded,
    CriticalThresholdExceeded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthIndicatorThreshold {
    pub indicator: FleetHealthIndicator,
    pub watch_at: f64,
    pub degraded_at: f64,
    pub critical_at: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ComponentHealthVerdictRequest {
    #[serde(default)]
    pub component_id: String,
    #[serde(default)]
    pub evaluated_at: String,
    #[serde(default)]
    pub method_version: String,
    #[serde(default)]
    pub samples: Vec<FleetHealthIndicatorSample>,
    #[serde(default)]
    pub thresholds: Vec<HealthIndicatorThreshold>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthVerdictEvidence {
    pub indicator: FleetHealthIndicator,
    pub value: f64,
    pub threshold: f64,
    pub status: ComponentHealthVerdictStatus,
    pub reason_code: HealthVerdictReasonCode,
    pub sample_ts: String,
    pub source_ref: String,
    pub freshness: HealthIndicatorFreshness,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentHealthVerdict {
    pub component_id: String,
    pub evaluated_at: String,
    pub method_version: String,
    pub status: ComponentHealthVerdictStatus,
    pub reason_code: HealthVerdictReasonCode,
    pub indicator: Option<FleetHealthIndicator>,
    pub threshold: Option<f64>,
    pub value: Option<f64>,
    pub freshness: HealthIndicatorFreshness,
    pub evidence: Vec<HealthVerdictEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentServiceLimit {
    #[serde(default)]
    pub component_id: String,
    #[serde(default)]
    pub max_flight_hours: Option<f64>,
    #[serde(default)]
    pub max_cycles: Option<u32>,
    #[serde(default)]
    pub max_duty_score: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FleetReadinessRequest {
    #[serde(default)]
    pub airframe_id: String,
    #[serde(default)]
    pub checked_at: String,
    #[serde(default)]
    pub installed_components: Vec<FleetComponentRecord>,
    #[serde(default)]
    pub service_limits: Vec<ComponentServiceLimit>,
    #[serde(default)]
    pub health_verdicts: Vec<ComponentHealthVerdict>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetReadinessDecisionStatus {
    Permitted,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetReadinessBlockReason {
    MissingInstalledComponent,
    MissingServiceLimit,
    OverdueServiceHours,
    OverdueServiceCycles,
    OverdueDutyScore,
    MissingHealthData,
    StaleHealthData,
    CriticalHealthVerdict,
    BatteryHealthBelowThreshold,
    OpenCriticalWorkOrder,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetReadinessBlocker {
    pub reason_code: FleetReadinessBlockReason,
    pub component_ref: Option<String>,
    pub indicator: Option<FleetHealthIndicator>,
    pub observed_value: Option<f64>,
    pub threshold: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetReadinessDecision {
    pub airframe_id: String,
    pub checked_at: String,
    pub status: FleetReadinessDecisionStatus,
    pub blockers: Vec<FleetReadinessBlocker>,
    pub component_count: usize,
    pub verdict_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OtaRolloutStage {
    Canary,
    Staged,
    Fleet,
}

impl OtaRolloutStage {
    pub fn next(self) -> Option<Self> {
        match self {
            Self::Canary => Some(Self::Staged),
            Self::Staged => Some(Self::Fleet),
            Self::Fleet => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OtaRolloutDecisionStatus {
    Advance,
    HaltedRolledBack,
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OtaRolloutDecisionReason {
    StageHealthy,
    HealthRegression,
    MissingSignedRollbackTarget,
    UnsignedTargetVersion,
    MissingStageNode,
    MissingHealthReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtaArtifactVersion {
    pub artifact: String,
    pub version: String,
    pub signed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtaRolloutNode {
    pub node_id: String,
    pub stage: OtaRolloutStage,
    pub current_version: String,
    pub previous_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtaNodeHealthReport {
    pub node_id: String,
    pub status: ComponentHealthVerdictStatus,
    #[serde(default)]
    pub blocking_alerts: Vec<String>,
    pub checked_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtaRollbackAction {
    pub node_id: String,
    pub from_version: String,
    pub to_version: String,
    pub health_status: ComponentHealthVerdictStatus,
    pub blocking_alerts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtaRolloutRequest {
    pub rollout_id: String,
    pub evaluated_at: String,
    pub current_stage: OtaRolloutStage,
    pub target_version: OtaArtifactVersion,
    pub rollback_version: Option<OtaArtifactVersion>,
    pub nodes: Vec<OtaRolloutNode>,
    pub health_reports: Vec<OtaNodeHealthReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtaRolloutDecision {
    pub rollout_id: String,
    pub evaluated_at: String,
    pub current_stage: OtaRolloutStage,
    pub next_stage: Option<OtaRolloutStage>,
    pub status: OtaRolloutDecisionStatus,
    pub reason_code: OtaRolloutDecisionReason,
    pub rollback_actions: Vec<OtaRollbackAction>,
    pub evaluated_node_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutControlAction {
    Start,
    Pause,
    Abort,
}

impl RolloutControlAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Pause => "pause",
            Self::Abort => "abort",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutControlStatus {
    Started,
    Paused,
    Aborted,
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutControlReason {
    StartedByOperator,
    PausedByOperator,
    AbortedByOperator,
    SimulationValidationRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutControlRequest {
    pub rollout_id: String,
    pub actor: String,
    pub action: RolloutControlAction,
    pub version: String,
    pub stage: OtaRolloutStage,
    pub requested_at: String,
    pub simulation_validated: bool,
    pub targets_flight_nodes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutControlAuditRecord {
    pub audit_id: String,
    pub rollout_id: String,
    pub actor: String,
    pub action: RolloutControlAction,
    pub version: String,
    pub stage: OtaRolloutStage,
    pub at: String,
    pub result: RolloutControlStatus,
    pub reason_code: RolloutControlReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutControlDecision {
    pub rollout_id: String,
    pub actor: String,
    pub action: RolloutControlAction,
    pub version: String,
    pub stage: OtaRolloutStage,
    pub requested_at: String,
    pub status: RolloutControlStatus,
    pub reason_code: RolloutControlReason,
    pub audit: RolloutControlAuditRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetOperationsFeedSource {
    Inventory,
    Health,
    Alerts,
    Rollouts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetOperationsFeedSourceStatus {
    Current,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetOperationsFeedSourceState {
    pub source: FleetOperationsFeedSource,
    pub status: FleetOperationsFeedSourceStatus,
    pub observed_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetOperationsRolloutFeedState {
    Advancing,
    HaltedRolledBack,
    Refused,
    Started,
    Paused,
    Aborted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetOperationsRolloutFeedEntry {
    pub rollout_id: String,
    pub stage: OtaRolloutStage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub state: FleetOperationsRolloutFeedState,
    pub reason_code: String,
    pub updated_at: String,
    pub evaluated_node_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetOperationsDashboardFeed {
    pub generated_at: String,
    pub inventory: FleetVersionInventory,
    #[serde(default)]
    pub alerts: Vec<FleetAlertRecord>,
    #[serde(default)]
    pub rollouts: Vec<FleetOperationsRolloutFeedEntry>,
    #[serde(default)]
    pub sources: Vec<FleetOperationsFeedSourceState>,
    #[serde(default)]
    pub source_gaps: Vec<FleetOperationsFeedSourceState>,
}

impl FleetReadinessDecision {
    pub fn is_clear(&self) -> bool {
        self.status == FleetReadinessDecisionStatus::Permitted && self.blockers.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetComponentRecord {
    pub component_id: String,
    pub component_type: FleetComponentType,
    pub serial: String,
    pub airframe_id: Option<String>,
    pub installed_at: Option<String>,
    pub removed_at: Option<String>,
    pub service_history: Vec<ServiceHistoryEntry>,
    pub flight_hours: f64,
    pub cycles: u32,
    pub duty_score: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetComponentEventRecord {
    pub component_id: String,
    pub event_type: String,
    pub airframe_id: Option<String>,
    pub event_at: String,
    pub actor: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentDutyAccrualRecord {
    pub session_id: String,
    pub component_id: String,
    pub airframe_id: String,
    pub flight_hours: f64,
    pub cycles: u32,
    pub duty_score: f64,
    pub accrued_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FleetHealthError {
    #[error("component_id cannot be empty")]
    EmptyComponentId,
    #[error("component serial cannot be empty")]
    EmptySerial,
    #[error("airframe_id cannot be empty")]
    EmptyAirframeId,
    #[error("installed_at cannot be empty")]
    EmptyInstalledAt,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("session_id cannot be empty")]
    EmptySessionId,
    #[error("flight_hours must be finite and non-negative")]
    InvalidFlightHours,
    #[error("duty_score must be finite and non-negative")]
    InvalidDutyScore,
    #[error("ended_at cannot be empty")]
    EmptyEndedAt,
    #[error("source_ref cannot be empty")]
    EmptySourceRef,
    #[error("telemetry sample timestamp cannot be empty")]
    EmptyTelemetryTimestamp,
    #[error("telemetry sample set cannot be empty")]
    EmptyTelemetrySamples,
    #[error("health indicator sample set cannot be empty")]
    EmptyHealthIndicatorSamples,
    #[error("health threshold set cannot be empty")]
    EmptyHealthThresholds,
    #[error("battery trend requires at least one internal-resistance sample")]
    EmptyBatteryResistanceSamples,
    #[error("health threshold method_version cannot be empty")]
    EmptyHealthMethodVersion,
    #[error("health degradation detection config must have at least two points and finite positive slope/delta thresholds")]
    InvalidHealthDegradationConfig,
    #[error("health evidence record_id cannot be empty")]
    EmptyHealthEvidenceRecordId,
    #[error("health evidence input ref_kind cannot be empty")]
    EmptyHealthEvidenceRefKind,
    #[error("health evidence input ref_id cannot be empty")]
    EmptyHealthEvidenceRefId,
    #[error("telemetry value must be finite")]
    InvalidTelemetryValue,
    #[error("telemetry sample timestamp is invalid: {value}")]
    InvalidTelemetryTimestamp { value: String },
    #[error("health evidence record {record_id} already exists with a different decision hash")]
    HealthEvidenceOverwriteRefused { record_id: String },
    #[error(
        "health threshold must be finite, non-negative, and ordered watch <= degraded <= critical"
    )]
    InvalidHealthThreshold { indicator: FleetHealthIndicator },
    #[error("missing health threshold for {indicator:?}")]
    MissingHealthThreshold { indicator: FleetHealthIndicator },
    #[error("indicator sample belongs to component {sample_component_id}, not requested component {component_id}")]
    IndicatorComponentMismatch {
        component_id: String,
        sample_component_id: String,
    },
    #[error("service limit must have a component_id and at least one finite non-negative limit")]
    InvalidServiceLimit { component_id: String },
    #[error("battery trend requires a battery component")]
    BatteryTrendRequiresBattery { component_id: String },
    #[error("battery current must be finite and non-zero")]
    InvalidBatteryCurrent,
    #[error("telemetry gap timestamp cannot be empty")]
    EmptyTelemetryGapTimestamp,
    #[error("telemetry gap reason cannot be empty")]
    EmptyTelemetryGapReason,
    #[error("telemetry gap started_at must be at or before ended_at")]
    InvalidTelemetryGapRange,
    #[error("service_id cannot be empty")]
    EmptyServiceId,
    #[error("service performed_at cannot be empty")]
    EmptyServicePerformedAt,
    #[error("service technician cannot be empty")]
    EmptyServiceTechnician,
    #[error("service action cannot be empty")]
    EmptyServiceAction,
    #[error("work_order_id cannot be empty")]
    EmptyWorkOrderId,
    #[error("work order reason cannot be empty")]
    EmptyWorkOrderReason,
    #[error("work order part_id cannot be empty")]
    EmptyPartId,
    #[error("work order part quantity must be greater than zero")]
    InvalidPartQuantity,
    #[error("work order {wo_id} is already closed")]
    WorkOrderAlreadyClosed { wo_id: String },
    #[error("unsupported fleet component type {value}")]
    UnsupportedComponentType { value: String },
    #[error("unsupported fleet health indicator {value}")]
    UnsupportedHealthIndicator { value: String },
    #[error("unsupported health indicator freshness {value}")]
    UnsupportedHealthFreshness { value: String },
    #[error("component {component_id} is already installed on airframe {airframe_id}")]
    AlreadyInstalled {
        component_id: String,
        airframe_id: String,
    },
    #[error("rollout_id cannot be empty")]
    EmptyRolloutId,
    #[error("rollout actor cannot be empty")]
    EmptyRolloutActor,
    #[error("fleet node_id cannot be empty")]
    EmptyFleetNodeId,
    #[error("artifact name cannot be empty")]
    EmptyArtifactName,
    #[error("artifact version cannot be empty")]
    EmptyArtifactVersion,
    #[error("OTA stage has no target nodes")]
    MissingOtaStageNode,
    #[error("OTA health report is missing for node {node_id}")]
    MissingOtaHealthReport { node_id: String },
}

pub fn build_component_record(
    request: RegisterComponentRequest,
    generated_component_id: String,
    created_at: String,
) -> Result<FleetComponentRecord, FleetHealthError> {
    let component_id = match normalize_optional_text(request.component_id) {
        Some(component_id) => component_id,
        None => {
            normalize_required_text(generated_component_id, FleetHealthError::EmptyComponentId)?
        }
    };
    let airframe_id = normalize_optional_text(request.airframe_id);
    let installed_at = normalize_optional_text(request.installed_at);
    if airframe_id.is_some() && installed_at.is_none() {
        return Err(FleetHealthError::EmptyInstalledAt);
    }
    if installed_at.is_some() && airframe_id.is_none() {
        return Err(FleetHealthError::EmptyAirframeId);
    }

    let service_history = request
        .service_history
        .into_iter()
        .map(normalize_service_history_entry)
        .collect::<Result<Vec<_>, _>>()?;
    let created_at = normalize_required_text(created_at, FleetHealthError::EmptyCreatedAt)?;

    Ok(FleetComponentRecord {
        component_id,
        component_type: request.component_type,
        serial: normalize_required_text(request.serial, FleetHealthError::EmptySerial)?,
        airframe_id,
        installed_at,
        removed_at: normalize_optional_text(request.removed_at),
        service_history,
        flight_hours: 0.0,
        cycles: 0,
        duty_score: 0.0,
        created_at: created_at.clone(),
        updated_at: created_at,
    })
}

pub fn install_component(
    component: &FleetComponentRecord,
    request: InstallComponentRequest,
    updated_at: String,
) -> Result<FleetComponentRecord, FleetHealthError> {
    let airframe_id =
        normalize_required_text(request.airframe_id, FleetHealthError::EmptyAirframeId)?;
    let installed_at =
        normalize_required_text(request.installed_at, FleetHealthError::EmptyInstalledAt)?;

    if component.removed_at.is_none() {
        if let Some(current_airframe) = &component.airframe_id {
            if current_airframe != &airframe_id {
                return Err(FleetHealthError::AlreadyInstalled {
                    component_id: component.component_id.clone(),
                    airframe_id: current_airframe.clone(),
                });
            }
        }
    }

    let mut updated = component.clone();
    updated.airframe_id = Some(airframe_id);
    updated.installed_at = Some(installed_at);
    updated.removed_at = None;
    updated.updated_at = normalize_required_text(updated_at, FleetHealthError::EmptyCreatedAt)?;
    Ok(updated)
}

pub fn build_component_duty_accruals(
    request: DutyAccrualRequest,
    component_ids: &[String],
) -> Result<Vec<ComponentDutyAccrualRecord>, FleetHealthError> {
    let session_id = normalize_required_text(request.session_id, FleetHealthError::EmptySessionId)?;
    let airframe_id =
        normalize_required_text(request.airframe_id, FleetHealthError::EmptyAirframeId)?;
    validate_nonnegative_finite(request.flight_hours, FleetHealthError::InvalidFlightHours)?;
    validate_nonnegative_finite(request.duty_score, FleetHealthError::InvalidDutyScore)?;
    let accrued_at = normalize_required_text(request.ended_at, FleetHealthError::EmptyEndedAt)?;

    component_ids
        .iter()
        .map(|component_id| {
            Ok(ComponentDutyAccrualRecord {
                session_id: session_id.clone(),
                component_id: normalize_required_text(
                    component_id.clone(),
                    FleetHealthError::EmptyComponentId,
                )?,
                airframe_id: airframe_id.clone(),
                flight_hours: request.flight_hours,
                cycles: request.cycles,
                duty_score: request.duty_score,
                accrued_at: accrued_at.clone(),
            })
        })
        .collect()
}

pub fn accrue_component_duty(
    component: &FleetComponentRecord,
    accrual: &ComponentDutyAccrualRecord,
    updated_at: String,
) -> Result<FleetComponentRecord, FleetHealthError> {
    let mut updated = component.clone();
    updated.flight_hours += accrual.flight_hours;
    updated.cycles += accrual.cycles;
    updated.duty_score += accrual.duty_score;
    updated.updated_at = normalize_required_text(updated_at, FleetHealthError::EmptyCreatedAt)?;
    Ok(updated)
}

pub fn derive_health_indicators(
    request: TelemetryHealthIndicatorRequest,
) -> Result<FleetHealthIndicatorDerivation, FleetHealthError> {
    let source_ref = normalize_required_text(request.source_ref, FleetHealthError::EmptySourceRef)?;
    let created_at = normalize_required_text(request.created_at, FleetHealthError::EmptyCreatedAt)?;
    if request.samples.is_empty() {
        return Err(FleetHealthError::EmptyTelemetrySamples);
    }
    let gaps = request
        .telemetry_gaps
        .into_iter()
        .map(normalize_health_telemetry_gap)
        .collect::<Result<Vec<_>, _>>()?;
    let mut samples = Vec::new();

    for sample in request.samples {
        let sample = normalize_health_telemetry_sample(sample)?;
        let freshness = if has_later_gap(&gaps, &sample.component_id, &sample.ts) {
            HealthIndicatorFreshness::Stale
        } else {
            HealthIndicatorFreshness::Fresh
        };

        match sample.component_type {
            FleetComponentType::Battery => {
                if let (Some(open_circuit), Some(loaded), Some(current)) = (
                    sample.battery_open_circuit_voltage_v,
                    sample.battery_voltage_v,
                    sample.battery_current_a,
                ) {
                    validate_finite(open_circuit)?;
                    validate_finite(loaded)?;
                    validate_finite(current)?;
                    if current.abs() <= f64::EPSILON {
                        return Err(FleetHealthError::InvalidBatteryCurrent);
                    }
                    samples.push(FleetHealthIndicatorSample {
                        component_id: sample.component_id,
                        indicator: FleetHealthIndicator::BatteryInternalResistance,
                        value: ((open_circuit - loaded).abs() / current.abs()) * 1000.0,
                        ts: sample.ts,
                        source_ref: source_ref.clone(),
                        created_at: created_at.clone(),
                        freshness,
                    });
                }
            }
            FleetComponentType::Motor => {
                if let Some(value) = sample.motor_vibration_g {
                    validate_finite(value)?;
                    samples.push(FleetHealthIndicatorSample {
                        component_id: sample.component_id,
                        indicator: FleetHealthIndicator::MotorVibration,
                        value,
                        ts: sample.ts,
                        source_ref: source_ref.clone(),
                        created_at: created_at.clone(),
                        freshness,
                    });
                }
            }
            FleetComponentType::Esc => {
                if let Some(value) = sample.esc_temperature_c {
                    validate_finite(value)?;
                    samples.push(FleetHealthIndicatorSample {
                        component_id: sample.component_id,
                        indicator: FleetHealthIndicator::EscTemperature,
                        value,
                        ts: sample.ts,
                        source_ref: source_ref.clone(),
                        created_at: created_at.clone(),
                        freshness,
                    });
                }
            }
            _ => {}
        }
    }

    Ok(FleetHealthIndicatorDerivation { samples, gaps })
}

pub fn evaluate_battery_health_trend(
    component: &FleetComponentRecord,
    samples: Vec<FleetHealthIndicatorSample>,
    config: BatteryHealthTrendConfig,
    evaluated_at: String,
) -> Result<BatteryHealthTrendReport, FleetHealthError> {
    if component.component_type != FleetComponentType::Battery {
        return Err(FleetHealthError::BatteryTrendRequiresBattery {
            component_id: component.component_id.clone(),
        });
    }
    let component_id = normalize_required_text(
        component.component_id.clone(),
        FleetHealthError::EmptyComponentId,
    )?;
    let evaluated_at = normalize_required_text(evaluated_at, FleetHealthError::EmptyCreatedAt)?;
    if config.max_cycles == 0 {
        return Err(FleetHealthError::InvalidServiceLimit {
            component_id: component_id.clone(),
        });
    }
    if !config.resistance_degraded_at_milliohm.is_finite()
        || config.resistance_degraded_at_milliohm < 0.0
    {
        return Err(FleetHealthError::InvalidHealthThreshold {
            indicator: FleetHealthIndicator::BatteryInternalResistance,
        });
    }

    let mut resistance_samples = Vec::new();
    for sample in samples {
        let sample_component_id =
            normalize_required_text(sample.component_id, FleetHealthError::EmptyComponentId)?;
        if sample_component_id != component_id {
            return Err(FleetHealthError::IndicatorComponentMismatch {
                component_id: component_id.clone(),
                sample_component_id,
            });
        }
        if sample.indicator != FleetHealthIndicator::BatteryInternalResistance {
            continue;
        }
        validate_finite(sample.value)?;
        resistance_samples.push(FleetHealthIndicatorSample {
            component_id: sample_component_id,
            indicator: sample.indicator,
            value: sample.value,
            ts: normalize_required_text(sample.ts, FleetHealthError::EmptyTelemetryTimestamp)?,
            source_ref: normalize_required_text(
                sample.source_ref,
                FleetHealthError::EmptySourceRef,
            )?,
            created_at: sample.created_at,
            freshness: sample.freshness,
        });
    }
    if resistance_samples.is_empty() {
        return Err(FleetHealthError::EmptyBatteryResistanceSamples);
    }
    resistance_samples.sort_by(|left, right| left.ts.cmp(&right.ts));
    let latest_resistance = resistance_samples
        .last()
        .expect("non-empty resistance samples checked above");

    let cycle_status = if component.cycles > config.max_cycles {
        ComponentHealthVerdictStatus::Degraded
    } else {
        ComponentHealthVerdictStatus::Ok
    };
    let resistance_status = if latest_resistance.value >= config.resistance_degraded_at_milliohm {
        ComponentHealthVerdictStatus::Degraded
    } else {
        ComponentHealthVerdictStatus::Ok
    };
    let cycle_reason = if cycle_status == ComponentHealthVerdictStatus::Degraded {
        HealthVerdictReasonCode::DegradedThresholdExceeded
    } else {
        HealthVerdictReasonCode::AllIndicatorsWithinThreshold
    };
    let resistance_reason = if resistance_status == ComponentHealthVerdictStatus::Degraded {
        HealthVerdictReasonCode::DegradedThresholdExceeded
    } else {
        HealthVerdictReasonCode::AllIndicatorsWithinThreshold
    };
    let evidence = vec![
        HealthVerdictEvidence {
            indicator: FleetHealthIndicator::BatteryCycleCount,
            value: component.cycles as f64,
            threshold: config.max_cycles as f64,
            status: cycle_status,
            reason_code: cycle_reason,
            sample_ts: component.updated_at.clone(),
            source_ref: format!("component:{}", component.component_id),
            freshness: HealthIndicatorFreshness::Fresh,
        },
        HealthVerdictEvidence {
            indicator: FleetHealthIndicator::BatteryInternalResistance,
            value: latest_resistance.value,
            threshold: config.resistance_degraded_at_milliohm,
            status: resistance_status,
            reason_code: resistance_reason,
            sample_ts: latest_resistance.ts.clone(),
            source_ref: latest_resistance.source_ref.clone(),
            freshness: latest_resistance.freshness,
        },
    ];
    let status = evidence
        .iter()
        .max_by(|left, right| compare_verdict_evidence(left, right))
        .map(|evidence| evidence.status)
        .unwrap_or(ComponentHealthVerdictStatus::Ok);

    Ok(BatteryHealthTrendReport {
        component_id,
        evaluated_at,
        cycle_count: component.cycles,
        max_cycles: config.max_cycles,
        latest_internal_resistance_milliohm: latest_resistance.value,
        resistance_sample_count: resistance_samples.len(),
        status,
        evidence,
    })
}

pub fn detect_health_indicator_degradation(
    request: HealthDegradationDetectionRequest,
) -> Result<HealthDegradationDetectionReport, FleetHealthError> {
    let component_id =
        normalize_required_text(request.component_id, FleetHealthError::EmptyComponentId)?;
    let evaluated_at =
        normalize_required_text(request.evaluated_at, FleetHealthError::EmptyCreatedAt)?;
    let method_version = normalize_required_text(
        request.method_version,
        FleetHealthError::EmptyHealthMethodVersion,
    )?;
    validate_degradation_config(request.config)?;

    let mut normalized = Vec::new();
    for sample in request.samples {
        let sample_component_id =
            normalize_required_text(sample.component_id, FleetHealthError::EmptyComponentId)?;
        if sample_component_id != component_id {
            return Err(FleetHealthError::IndicatorComponentMismatch {
                component_id,
                sample_component_id,
            });
        }
        if sample.indicator != request.indicator {
            continue;
        }
        validate_finite(sample.value)?;
        let sample_ts =
            normalize_required_text(sample.ts, FleetHealthError::EmptyTelemetryTimestamp)?;
        let parsed_ts = parse_health_sample_timestamp(&sample_ts)?;
        let source_ref =
            normalize_required_text(sample.source_ref, FleetHealthError::EmptySourceRef)?;
        normalized.push(ParsedHealthIndicatorSample {
            sample: FleetHealthIndicatorSample {
                component_id: sample_component_id,
                indicator: sample.indicator,
                value: sample.value,
                ts: sample_ts,
                source_ref,
                created_at: sample.created_at,
                freshness: sample.freshness,
            },
            parsed_ts,
        });
    }
    normalized.sort_by(|left, right| {
        left.parsed_ts
            .cmp(&right.parsed_ts)
            .then_with(|| left.sample.source_ref.cmp(&right.sample.source_ref))
    });

    if normalized.len() < request.config.min_history_points {
        return Ok(HealthDegradationDetectionReport {
            component_id,
            evaluated_at,
            method_version,
            indicator: request.indicator,
            status: HealthDegradationDetectionStatus::InsufficientHistory,
            reason_code: HealthDegradationReasonCode::InsufficientHistory,
            window_start: None,
            window_end: None,
            slope_per_day: None,
            delta: None,
            evidence_refs: vec![],
            event: None,
        });
    }

    let recent_start = normalized.len() - request.config.recent_window_points;
    let window = &normalized[recent_start..];
    let window_start = window
        .first()
        .expect("validated recent window has at least two samples")
        .sample
        .ts
        .clone();
    let window_end = window
        .last()
        .expect("validated recent window has at least two samples")
        .sample
        .ts
        .clone();
    let slope_per_day = linear_slope_per_day(window);
    let delta = window
        .last()
        .expect("validated recent window has at least two samples")
        .sample
        .value
        - window
            .first()
            .expect("validated recent window has at least two samples")
            .sample
            .value;
    let evidence_refs = sorted_unique_evidence_refs(window);
    let reason_code = if slope_per_day >= request.config.min_adverse_slope_per_day {
        HealthDegradationReasonCode::SustainedAdverseSlope
    } else if delta >= request.config.min_adverse_delta {
        HealthDegradationReasonCode::DriftExceeded
    } else {
        HealthDegradationReasonCode::WithinTrendBand
    };
    let status = if reason_code == HealthDegradationReasonCode::WithinTrendBand {
        HealthDegradationDetectionStatus::Stable
    } else {
        HealthDegradationDetectionStatus::DegradationDetected
    };
    let event = if status == HealthDegradationDetectionStatus::DegradationDetected {
        Some(HealthDegradationEvent {
            component_id: component_id.clone(),
            indicator: request.indicator,
            reason_code,
            window_start: window_start.clone(),
            window_end: window_end.clone(),
            slope_per_day,
            delta,
            evidence_refs: evidence_refs.clone(),
        })
    } else {
        None
    };

    Ok(HealthDegradationDetectionReport {
        component_id,
        evaluated_at,
        method_version,
        indicator: request.indicator,
        status,
        reason_code,
        window_start: Some(window_start),
        window_end: Some(window_end),
        slope_per_day: Some(slope_per_day),
        delta: Some(delta),
        evidence_refs,
        event,
    })
}

pub fn evaluate_component_health_verdict(
    request: ComponentHealthVerdictRequest,
) -> Result<ComponentHealthVerdict, FleetHealthError> {
    let component_id =
        normalize_required_text(request.component_id, FleetHealthError::EmptyComponentId)?;
    let evaluated_at =
        normalize_required_text(request.evaluated_at, FleetHealthError::EmptyCreatedAt)?;
    let method_version = normalize_required_text(
        request.method_version,
        FleetHealthError::EmptyHealthMethodVersion,
    )?;
    if request.samples.is_empty() {
        return Err(FleetHealthError::EmptyHealthIndicatorSamples);
    }
    if request.thresholds.is_empty() {
        return Err(FleetHealthError::EmptyHealthThresholds);
    }

    let thresholds = request
        .thresholds
        .into_iter()
        .map(normalize_health_threshold)
        .collect::<Result<Vec<_>, _>>()?;
    let mut evidence = Vec::new();

    for sample in request.samples {
        let sample_component_id =
            normalize_required_text(sample.component_id, FleetHealthError::EmptyComponentId)?;
        if sample_component_id != component_id {
            return Err(FleetHealthError::IndicatorComponentMismatch {
                component_id,
                sample_component_id,
            });
        }
        validate_finite(sample.value)?;
        let sample_ts =
            normalize_required_text(sample.ts, FleetHealthError::EmptyTelemetryTimestamp)?;
        let source_ref =
            normalize_required_text(sample.source_ref, FleetHealthError::EmptySourceRef)?;
        let threshold = thresholds
            .iter()
            .find(|threshold| threshold.indicator == sample.indicator)
            .ok_or(FleetHealthError::MissingHealthThreshold {
                indicator: sample.indicator,
            })?;
        let (status, reason_code, threshold_value) =
            classify_health_indicator(sample.value, threshold);

        evidence.push(HealthVerdictEvidence {
            indicator: sample.indicator,
            value: sample.value,
            threshold: threshold_value,
            status,
            reason_code,
            sample_ts,
            source_ref,
            freshness: sample.freshness,
        });
    }

    let selected = evidence
        .iter()
        .max_by(|left, right| compare_verdict_evidence(left, right))
        .expect("non-empty evidence checked above");
    let freshness = if evidence
        .iter()
        .any(|item| item.freshness == HealthIndicatorFreshness::Stale)
    {
        HealthIndicatorFreshness::Stale
    } else {
        HealthIndicatorFreshness::Fresh
    };

    Ok(ComponentHealthVerdict {
        component_id,
        evaluated_at,
        method_version,
        status: selected.status,
        reason_code: selected.reason_code,
        indicator: Some(selected.indicator),
        threshold: Some(selected.threshold),
        value: Some(selected.value),
        freshness,
        evidence,
    })
}

pub fn build_health_verdict_evidence_record(
    record_id: String,
    verdict: &ComponentHealthVerdict,
    recorded_at: String,
) -> Result<HealthEvidenceRecord, FleetHealthError> {
    let record_id =
        normalize_required_text(record_id, FleetHealthError::EmptyHealthEvidenceRecordId)?;
    let component_id = normalize_required_text(
        verdict.component_id.clone(),
        FleetHealthError::EmptyComponentId,
    )?;
    let method_version = normalize_required_text(
        verdict.method_version.clone(),
        FleetHealthError::EmptyHealthMethodVersion,
    )?;
    let recorded_at = normalize_required_text(recorded_at, FleetHealthError::EmptyCreatedAt)?;
    let mut input_refs = verdict
        .evidence
        .iter()
        .map(|evidence| {
            health_evidence_input_ref(
                format!("indicator:{}", evidence.indicator.as_str()),
                evidence.source_ref.clone(),
                Some(format!(
                    "value={:.12}|threshold={:.12}|sample_ts={}|freshness={}",
                    evidence.value,
                    evidence.threshold,
                    evidence.sample_ts,
                    evidence.freshness.as_str()
                )),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    input_refs.sort_by(|left, right| {
        left.ref_kind
            .cmp(&right.ref_kind)
            .then_with(|| left.ref_id.cmp(&right.ref_id))
            .then_with(|| left.value.cmp(&right.value))
    });
    let reason_code = health_verdict_reason_code(verdict.reason_code).to_string();
    let decision_hash = health_evidence_decision_hash(
        HealthEvidenceSubjectKind::ComponentVerdict,
        &component_id,
        &method_version,
        &reason_code,
        &input_refs,
    );

    Ok(HealthEvidenceRecord {
        record_id,
        subject_kind: HealthEvidenceSubjectKind::ComponentVerdict,
        component_id,
        method_version,
        reason_code,
        recorded_at,
        input_refs,
        decision_hash,
    })
}

pub fn build_degradation_event_evidence_record(
    record_id: String,
    report: &HealthDegradationDetectionReport,
    recorded_at: String,
) -> Result<HealthEvidenceRecord, FleetHealthError> {
    let record_id =
        normalize_required_text(record_id, FleetHealthError::EmptyHealthEvidenceRecordId)?;
    let component_id = normalize_required_text(
        report.component_id.clone(),
        FleetHealthError::EmptyComponentId,
    )?;
    let method_version = normalize_required_text(
        report.method_version.clone(),
        FleetHealthError::EmptyHealthMethodVersion,
    )?;
    let recorded_at = normalize_required_text(recorded_at, FleetHealthError::EmptyCreatedAt)?;
    let reason_code = health_degradation_reason_code(report.reason_code).to_string();
    let mut input_refs = report
        .evidence_refs
        .iter()
        .map(|source_ref| {
            health_evidence_input_ref(
                format!("indicator_window:{}", report.indicator.as_str()),
                source_ref.clone(),
                Some(format!(
                    "window_start={}|window_end={}|slope_per_day={:.12}|delta={:.12}",
                    report.window_start.as_deref().unwrap_or(""),
                    report.window_end.as_deref().unwrap_or(""),
                    report.slope_per_day.unwrap_or(0.0),
                    report.delta.unwrap_or(0.0)
                )),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    input_refs.sort_by(|left, right| {
        left.ref_kind
            .cmp(&right.ref_kind)
            .then_with(|| left.ref_id.cmp(&right.ref_id))
            .then_with(|| left.value.cmp(&right.value))
    });
    let decision_hash = health_evidence_decision_hash(
        HealthEvidenceSubjectKind::DegradationEvent,
        &component_id,
        &method_version,
        &reason_code,
        &input_refs,
    );

    Ok(HealthEvidenceRecord {
        record_id,
        subject_kind: HealthEvidenceSubjectKind::DegradationEvent,
        component_id,
        method_version,
        reason_code,
        recorded_at,
        input_refs,
        decision_hash,
    })
}

pub fn append_health_evidence_record(
    records: &mut Vec<HealthEvidenceRecord>,
    record: HealthEvidenceRecord,
) -> Result<HealthEvidenceRecord, FleetHealthError> {
    if let Some(existing) = records
        .iter()
        .find(|existing| existing.record_id == record.record_id)
    {
        refuse_health_evidence_overwrite(existing, &record)?;
        return Ok(existing.clone());
    }
    records.push(record.clone());
    Ok(record)
}

pub fn refuse_health_evidence_overwrite(
    existing: &HealthEvidenceRecord,
    replacement: &HealthEvidenceRecord,
) -> Result<(), FleetHealthError> {
    if existing.record_id == replacement.record_id
        && existing.decision_hash != replacement.decision_hash
    {
        Err(FleetHealthError::HealthEvidenceOverwriteRefused {
            record_id: existing.record_id.clone(),
        })
    } else {
        Ok(())
    }
}

pub fn evaluate_fleet_readiness(
    request: FleetReadinessRequest,
) -> Result<FleetReadinessDecision, FleetHealthError> {
    let airframe_id =
        normalize_required_text(request.airframe_id, FleetHealthError::EmptyAirframeId)?;
    let checked_at = normalize_required_text(request.checked_at, FleetHealthError::EmptyCreatedAt)?;
    let service_limits = request
        .service_limits
        .into_iter()
        .map(normalize_component_service_limit)
        .collect::<Result<Vec<_>, _>>()?;
    let installed_components = request
        .installed_components
        .into_iter()
        .filter(|component| {
            component.airframe_id.as_deref() == Some(airframe_id.as_str())
                && component.removed_at.is_none()
        })
        .collect::<Vec<_>>();
    let mut blockers = Vec::new();

    if installed_components.is_empty() {
        blockers.push(readiness_blocker(
            FleetReadinessBlockReason::MissingInstalledComponent,
            None,
            None,
            None,
            None,
        ));
    }

    for component in &installed_components {
        match service_limits
            .iter()
            .find(|limit| limit.component_id == component.component_id)
        {
            Some(limit) => append_service_limit_blockers(component, limit, &mut blockers),
            None => blockers.push(readiness_blocker(
                FleetReadinessBlockReason::MissingServiceLimit,
                Some(component.component_id.clone()),
                None,
                None,
                None,
            )),
        }

        let Some(verdict) = request
            .health_verdicts
            .iter()
            .find(|verdict| verdict.component_id.trim() == component.component_id)
        else {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::MissingHealthData,
                Some(component.component_id.clone()),
                None,
                None,
                None,
            ));
            continue;
        };

        if verdict.freshness == HealthIndicatorFreshness::Stale {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::StaleHealthData,
                Some(component.component_id.clone()),
                verdict.indicator,
                verdict.value,
                verdict.threshold,
            ));
            continue;
        }

        if verdict.status == ComponentHealthVerdictStatus::Critical {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::CriticalHealthVerdict,
                Some(component.component_id.clone()),
                verdict.indicator,
                verdict.value,
                verdict.threshold,
            ));
        } else if component.component_type == FleetComponentType::Battery
            && verdict.status == ComponentHealthVerdictStatus::Degraded
        {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::BatteryHealthBelowThreshold,
                Some(component.component_id.clone()),
                verdict.indicator,
                verdict.value,
                verdict.threshold,
            ));
        }
    }

    let status = if blockers.is_empty() {
        FleetReadinessDecisionStatus::Permitted
    } else {
        FleetReadinessDecisionStatus::Blocked
    };

    Ok(FleetReadinessDecision {
        airframe_id,
        checked_at,
        status,
        blockers,
        component_count: installed_components.len(),
        verdict_count: request.health_verdicts.len(),
    })
}

pub fn open_maintenance_work_order(
    request: OpenMaintenanceWorkOrderRequest,
    generated_wo_id: String,
) -> Result<MaintenanceWorkOrder, FleetHealthError> {
    let wo_id = match request.wo_id {
        Some(wo_id) => normalize_required_text(wo_id, FleetHealthError::EmptyWorkOrderId)?,
        None => normalize_required_text(generated_wo_id, FleetHealthError::EmptyWorkOrderId)?,
    };

    Ok(MaintenanceWorkOrder {
        wo_id,
        component_id: normalize_required_text(
            request.component_id,
            FleetHealthError::EmptyComponentId,
        )?,
        reason: normalize_required_text(request.reason, FleetHealthError::EmptyWorkOrderReason)?,
        severity: request.severity,
        status: MaintenanceWorkOrderStatus::Open,
        opened_at: normalize_required_text(request.opened_at, FleetHealthError::EmptyCreatedAt)?,
        closed_at: None,
        technician: normalize_required_text(
            request.technician,
            FleetHealthError::EmptyServiceTechnician,
        )?,
        parts: Vec::new(),
    })
}

pub fn close_maintenance_work_order(
    component: &FleetComponentRecord,
    work_order: &MaintenanceWorkOrder,
    request: CloseMaintenanceWorkOrderRequest,
    updated_at: String,
) -> Result<MaintenanceWorkOrderCloseResult, FleetHealthError> {
    if work_order.status == MaintenanceWorkOrderStatus::Closed {
        return Err(FleetHealthError::WorkOrderAlreadyClosed {
            wo_id: work_order.wo_id.clone(),
        });
    }
    let component_id = normalize_required_text(
        component.component_id.clone(),
        FleetHealthError::EmptyComponentId,
    )?;
    if work_order.component_id != component_id {
        return Err(FleetHealthError::IndicatorComponentMismatch {
            component_id,
            sample_component_id: work_order.component_id.clone(),
        });
    }

    let closed_at = normalize_required_text(request.closed_at, FleetHealthError::EmptyCreatedAt)?;
    let technician =
        normalize_required_text(request.technician, FleetHealthError::EmptyServiceTechnician)?;
    let action = normalize_required_text(request.action, FleetHealthError::EmptyServiceAction)?;
    let parts = normalize_work_order_parts(request.parts)?;
    let service_history_entry = ServiceHistoryEntry {
        service_id: work_order.wo_id.clone(),
        performed_at: closed_at.clone(),
        technician: technician.clone(),
        action: action.clone(),
        notes: Some(format_work_order_parts_note(&work_order.reason, &parts)),
    };
    let mut updated_component = component.clone();
    updated_component
        .service_history
        .push(service_history_entry.clone());
    updated_component.updated_at =
        normalize_required_text(updated_at, FleetHealthError::EmptyCreatedAt)?;
    let mut closed_work_order = work_order.clone();
    closed_work_order.status = MaintenanceWorkOrderStatus::Closed;
    closed_work_order.closed_at = Some(closed_at);
    closed_work_order.technician = technician;
    closed_work_order.parts = parts;

    Ok(MaintenanceWorkOrderCloseResult {
        component: updated_component,
        work_order: closed_work_order,
        service_history_entry,
    })
}

pub fn evaluate_fleet_readiness_with_work_orders(
    request: FleetReadinessRequest,
    work_orders: &[MaintenanceWorkOrder],
) -> Result<FleetReadinessDecision, FleetHealthError> {
    let mut decision = evaluate_fleet_readiness(request)?;
    for work_order in work_orders {
        if work_order.status == MaintenanceWorkOrderStatus::Open
            && work_order.severity == MaintenanceWorkOrderSeverity::Critical
            && decision
                .blockers
                .iter()
                .all(|blocker| blocker.component_ref.as_deref() != Some(&work_order.component_id))
        {
            decision.blockers.push(readiness_blocker(
                FleetReadinessBlockReason::OpenCriticalWorkOrder,
                Some(work_order.component_id.clone()),
                None,
                None,
                None,
            ));
        }
    }
    decision.status = if decision.blockers.is_empty() {
        FleetReadinessDecisionStatus::Permitted
    } else {
        FleetReadinessDecisionStatus::Blocked
    };
    Ok(decision)
}

pub fn evaluate_ota_rollout(
    request: OtaRolloutRequest,
) -> Result<OtaRolloutDecision, FleetHealthError> {
    let rollout_id = normalize_required_text(request.rollout_id, FleetHealthError::EmptyRolloutId)?;
    let evaluated_at =
        normalize_required_text(request.evaluated_at, FleetHealthError::EmptyCreatedAt)?;
    let current_stage = request.current_stage;
    let target_version = normalize_ota_artifact_version(request.target_version)?;
    let rollback_version = request
        .rollback_version
        .map(normalize_ota_artifact_version)
        .transpose()?;
    let nodes = request
        .nodes
        .into_iter()
        .map(normalize_ota_rollout_node)
        .collect::<Result<Vec<_>, _>>()?;
    let health_reports = request
        .health_reports
        .into_iter()
        .map(normalize_ota_health_report)
        .collect::<Result<Vec<_>, _>>()?;
    let stage_nodes = nodes
        .iter()
        .filter(|node| node.stage == current_stage)
        .collect::<Vec<_>>();

    if stage_nodes.is_empty() {
        return Ok(ota_decision(
            rollout_id,
            evaluated_at,
            current_stage,
            OtaRolloutDecisionStatus::Refused,
            OtaRolloutDecisionReason::MissingStageNode,
            None,
            Vec::new(),
            0,
        ));
    }

    if !target_version.signed {
        return Ok(ota_decision(
            rollout_id,
            evaluated_at,
            current_stage,
            OtaRolloutDecisionStatus::Refused,
            OtaRolloutDecisionReason::UnsignedTargetVersion,
            None,
            Vec::new(),
            stage_nodes.len(),
        ));
    }

    let mut regressions = Vec::new();
    for node in &stage_nodes {
        let Some(report) = health_reports
            .iter()
            .find(|report| report.node_id == node.node_id)
        else {
            return Ok(ota_decision(
                rollout_id,
                evaluated_at,
                current_stage,
                OtaRolloutDecisionStatus::Refused,
                OtaRolloutDecisionReason::MissingHealthReport,
                None,
                Vec::new(),
                stage_nodes.len(),
            ));
        };

        if ota_health_regressed(report) {
            regressions.push((*node, report));
        }
    }

    if regressions.is_empty() {
        return Ok(ota_decision(
            rollout_id,
            evaluated_at,
            current_stage,
            OtaRolloutDecisionStatus::Advance,
            OtaRolloutDecisionReason::StageHealthy,
            current_stage.next(),
            Vec::new(),
            stage_nodes.len(),
        ));
    }

    let Some(rollback_version) = rollback_version.filter(|version| version.signed) else {
        return Ok(ota_decision(
            rollout_id,
            evaluated_at,
            current_stage,
            OtaRolloutDecisionStatus::Refused,
            OtaRolloutDecisionReason::MissingSignedRollbackTarget,
            None,
            Vec::new(),
            stage_nodes.len(),
        ));
    };

    let rollback_actions = regressions
        .into_iter()
        .map(|(node, report)| OtaRollbackAction {
            node_id: node.node_id.clone(),
            from_version: node.current_version.clone(),
            to_version: rollback_version.version.clone(),
            health_status: report.status,
            blocking_alerts: report.blocking_alerts.clone(),
        })
        .collect();

    Ok(ota_decision(
        rollout_id,
        evaluated_at,
        current_stage,
        OtaRolloutDecisionStatus::HaltedRolledBack,
        OtaRolloutDecisionReason::HealthRegression,
        None,
        rollback_actions,
        stage_nodes.len(),
    ))
}

pub fn apply_rollout_control(
    request: RolloutControlRequest,
) -> Result<RolloutControlDecision, FleetHealthError> {
    let rollout_id = normalize_required_text(request.rollout_id, FleetHealthError::EmptyRolloutId)?;
    let actor = normalize_required_text(request.actor, FleetHealthError::EmptyRolloutActor)?;
    let version = normalize_required_text(request.version, FleetHealthError::EmptyArtifactVersion)?;
    let requested_at =
        normalize_required_text(request.requested_at, FleetHealthError::EmptyCreatedAt)?;
    let (status, reason_code) = if request.targets_flight_nodes && !request.simulation_validated {
        (
            RolloutControlStatus::Refused,
            RolloutControlReason::SimulationValidationRequired,
        )
    } else {
        match request.action {
            RolloutControlAction::Start => (
                RolloutControlStatus::Started,
                RolloutControlReason::StartedByOperator,
            ),
            RolloutControlAction::Pause => (
                RolloutControlStatus::Paused,
                RolloutControlReason::PausedByOperator,
            ),
            RolloutControlAction::Abort => (
                RolloutControlStatus::Aborted,
                RolloutControlReason::AbortedByOperator,
            ),
        }
    };
    let audit = RolloutControlAuditRecord {
        audit_id: format!(
            "rollout-control:{}:{}:{}:{}",
            rollout_id,
            actor,
            request.action.as_str(),
            requested_at
        ),
        rollout_id: rollout_id.clone(),
        actor: actor.clone(),
        action: request.action,
        version: version.clone(),
        stage: request.stage,
        at: requested_at.clone(),
        result: status,
        reason_code,
    };

    Ok(RolloutControlDecision {
        rollout_id,
        actor,
        action: request.action,
        version,
        stage: request.stage,
        requested_at,
        status,
        reason_code,
        audit,
    })
}

pub fn build_fleet_operations_dashboard_feed(
    generated_at: impl Into<String>,
    inventory: FleetVersionInventory,
    alerts: Vec<FleetAlertRecord>,
    rollout_decisions: Vec<OtaRolloutDecision>,
    rollout_controls: Vec<RolloutControlDecision>,
    sources: Vec<FleetOperationsFeedSourceState>,
) -> FleetOperationsDashboardFeed {
    let mut rollouts = rollout_decisions
        .into_iter()
        .map(rollout_decision_feed_entry)
        .chain(rollout_controls.into_iter().map(rollout_control_feed_entry))
        .collect::<Vec<_>>();
    rollouts.sort_by(|left, right| {
        left.rollout_id
            .cmp(&right.rollout_id)
            .then(left.updated_at.cmp(&right.updated_at))
            .then(left.reason_code.cmp(&right.reason_code))
    });
    let source_gaps = sources
        .iter()
        .filter(|source| source.status == FleetOperationsFeedSourceStatus::Unavailable)
        .cloned()
        .collect();

    FleetOperationsDashboardFeed {
        generated_at: generated_at.into(),
        inventory,
        alerts,
        rollouts,
        sources,
        source_gaps,
    }
}

pub fn fleet_operations_source_current(
    source: FleetOperationsFeedSource,
    observed_at: impl Into<String>,
) -> FleetOperationsFeedSourceState {
    FleetOperationsFeedSourceState {
        source,
        status: FleetOperationsFeedSourceStatus::Current,
        observed_at: observed_at.into(),
        message: None,
    }
}

pub fn fleet_operations_source_unavailable(
    source: FleetOperationsFeedSource,
    observed_at: impl Into<String>,
    message: impl Into<String>,
) -> FleetOperationsFeedSourceState {
    FleetOperationsFeedSourceState {
        source,
        status: FleetOperationsFeedSourceStatus::Unavailable,
        observed_at: observed_at.into(),
        message: Some(message.into()),
    }
}

fn rollout_decision_feed_entry(decision: OtaRolloutDecision) -> FleetOperationsRolloutFeedEntry {
    FleetOperationsRolloutFeedEntry {
        rollout_id: decision.rollout_id,
        stage: decision.current_stage,
        version: None,
        state: match decision.status {
            OtaRolloutDecisionStatus::Advance => FleetOperationsRolloutFeedState::Advancing,
            OtaRolloutDecisionStatus::HaltedRolledBack => {
                FleetOperationsRolloutFeedState::HaltedRolledBack
            }
            OtaRolloutDecisionStatus::Refused => FleetOperationsRolloutFeedState::Refused,
        },
        reason_code: ota_rollout_reason_code(decision.reason_code).to_string(),
        updated_at: decision.evaluated_at,
        evaluated_node_count: decision.evaluated_node_count,
    }
}

fn rollout_control_feed_entry(decision: RolloutControlDecision) -> FleetOperationsRolloutFeedEntry {
    FleetOperationsRolloutFeedEntry {
        rollout_id: decision.rollout_id,
        stage: decision.stage,
        version: Some(decision.version),
        state: match decision.status {
            RolloutControlStatus::Started => FleetOperationsRolloutFeedState::Started,
            RolloutControlStatus::Paused => FleetOperationsRolloutFeedState::Paused,
            RolloutControlStatus::Aborted => FleetOperationsRolloutFeedState::Aborted,
            RolloutControlStatus::Refused => FleetOperationsRolloutFeedState::Refused,
        },
        reason_code: rollout_control_reason_code(decision.reason_code).to_string(),
        updated_at: decision.requested_at,
        evaluated_node_count: 0,
    }
}

fn ota_rollout_reason_code(reason: OtaRolloutDecisionReason) -> &'static str {
    match reason {
        OtaRolloutDecisionReason::StageHealthy => "stage_healthy",
        OtaRolloutDecisionReason::HealthRegression => "health_regression",
        OtaRolloutDecisionReason::MissingSignedRollbackTarget => "missing_signed_rollback_target",
        OtaRolloutDecisionReason::UnsignedTargetVersion => "unsigned_target_version",
        OtaRolloutDecisionReason::MissingStageNode => "missing_stage_node",
        OtaRolloutDecisionReason::MissingHealthReport => "missing_health_report",
    }
}

fn rollout_control_reason_code(reason: RolloutControlReason) -> &'static str {
    match reason {
        RolloutControlReason::StartedByOperator => "started_by_operator",
        RolloutControlReason::PausedByOperator => "paused_by_operator",
        RolloutControlReason::AbortedByOperator => "aborted_by_operator",
        RolloutControlReason::SimulationValidationRequired => "simulation_validation_required",
    }
}

pub fn component_event(
    component_id: &str,
    event_type: &str,
    airframe_id: Option<String>,
    event_at: String,
    actor: Option<String>,
    details: Option<String>,
) -> Result<FleetComponentEventRecord, FleetHealthError> {
    Ok(FleetComponentEventRecord {
        component_id: normalize_required_text(
            component_id.to_string(),
            FleetHealthError::EmptyComponentId,
        )?,
        event_type: normalize_required_text(
            event_type.to_string(),
            FleetHealthError::EmptyServiceAction,
        )?,
        airframe_id: normalize_optional_text(airframe_id),
        event_at: normalize_required_text(event_at, FleetHealthError::EmptyCreatedAt)?,
        actor: normalize_optional_text(actor),
        details: normalize_optional_text(details),
    })
}

fn normalize_ota_artifact_version(
    version: OtaArtifactVersion,
) -> Result<OtaArtifactVersion, FleetHealthError> {
    Ok(OtaArtifactVersion {
        artifact: normalize_required_text(version.artifact, FleetHealthError::EmptyArtifactName)?,
        version: normalize_required_text(version.version, FleetHealthError::EmptyArtifactVersion)?,
        signed: version.signed,
    })
}

fn normalize_ota_rollout_node(node: OtaRolloutNode) -> Result<OtaRolloutNode, FleetHealthError> {
    Ok(OtaRolloutNode {
        node_id: normalize_required_text(node.node_id, FleetHealthError::EmptyFleetNodeId)?,
        stage: node.stage,
        current_version: normalize_required_text(
            node.current_version,
            FleetHealthError::EmptyArtifactVersion,
        )?,
        previous_version: normalize_required_text(
            node.previous_version,
            FleetHealthError::EmptyArtifactVersion,
        )?,
    })
}

fn normalize_ota_health_report(
    report: OtaNodeHealthReport,
) -> Result<OtaNodeHealthReport, FleetHealthError> {
    Ok(OtaNodeHealthReport {
        node_id: normalize_required_text(report.node_id, FleetHealthError::EmptyFleetNodeId)?,
        status: report.status,
        blocking_alerts: report
            .blocking_alerts
            .into_iter()
            .filter_map(|alert| normalize_optional_text(Some(alert)))
            .collect(),
        checked_at: normalize_required_text(
            report.checked_at,
            FleetHealthError::EmptyTelemetryTimestamp,
        )?,
    })
}

fn ota_health_regressed(report: &OtaNodeHealthReport) -> bool {
    report.status.severity_rank() >= ComponentHealthVerdictStatus::Degraded.severity_rank()
        || !report.blocking_alerts.is_empty()
}

fn ota_decision(
    rollout_id: String,
    evaluated_at: String,
    current_stage: OtaRolloutStage,
    status: OtaRolloutDecisionStatus,
    reason_code: OtaRolloutDecisionReason,
    next_stage: Option<OtaRolloutStage>,
    rollback_actions: Vec<OtaRollbackAction>,
    evaluated_node_count: usize,
) -> OtaRolloutDecision {
    OtaRolloutDecision {
        rollout_id,
        evaluated_at,
        current_stage,
        next_stage,
        status,
        reason_code,
        rollback_actions,
        evaluated_node_count,
    }
}

fn normalize_health_telemetry_sample(
    sample: HealthTelemetrySample,
) -> Result<HealthTelemetrySample, FleetHealthError> {
    Ok(HealthTelemetrySample {
        component_id: normalize_required_text(
            sample.component_id,
            FleetHealthError::EmptyComponentId,
        )?,
        component_type: sample.component_type,
        ts: normalize_required_text(sample.ts, FleetHealthError::EmptyTelemetryTimestamp)?,
        battery_open_circuit_voltage_v: sample.battery_open_circuit_voltage_v,
        battery_voltage_v: sample.battery_voltage_v,
        battery_current_a: sample.battery_current_a,
        motor_vibration_g: sample.motor_vibration_g,
        esc_temperature_c: sample.esc_temperature_c,
    })
}

fn normalize_health_telemetry_gap(
    gap: HealthTelemetryGap,
) -> Result<HealthTelemetryGap, FleetHealthError> {
    let component_id =
        normalize_required_text(gap.component_id, FleetHealthError::EmptyComponentId)?;
    let started_at =
        normalize_required_text(gap.started_at, FleetHealthError::EmptyTelemetryGapTimestamp)?;
    let ended_at =
        normalize_required_text(gap.ended_at, FleetHealthError::EmptyTelemetryGapTimestamp)?;
    if started_at > ended_at {
        return Err(FleetHealthError::InvalidTelemetryGapRange);
    }
    Ok(HealthTelemetryGap {
        component_id,
        started_at,
        ended_at,
        reason: normalize_required_text(gap.reason, FleetHealthError::EmptyTelemetryGapReason)?,
    })
}

fn has_later_gap(gaps: &[HealthTelemetryGap], component_id: &str, sample_ts: &str) -> bool {
    gaps.iter()
        .any(|gap| gap.component_id == component_id && gap.started_at.as_str() > sample_ts)
}

struct ParsedHealthIndicatorSample {
    sample: FleetHealthIndicatorSample,
    parsed_ts: chrono::DateTime<chrono::Utc>,
}

fn validate_degradation_config(
    config: HealthDegradationDetectionConfig,
) -> Result<(), FleetHealthError> {
    let valid_window = config.min_history_points >= 2
        && config.recent_window_points >= 2
        && config.recent_window_points <= config.min_history_points;
    let valid_thresholds = config.min_adverse_slope_per_day.is_finite()
        && config.min_adverse_delta.is_finite()
        && config.min_adverse_slope_per_day > 0.0
        && config.min_adverse_delta > 0.0;

    if valid_window && valid_thresholds {
        Ok(())
    } else {
        Err(FleetHealthError::InvalidHealthDegradationConfig)
    }
}

fn parse_health_sample_timestamp(
    value: &str,
) -> Result<chrono::DateTime<chrono::Utc>, FleetHealthError> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|ts| ts.with_timezone(&chrono::Utc))
        .map_err(|_| FleetHealthError::InvalidTelemetryTimestamp {
            value: value.to_string(),
        })
}

fn linear_slope_per_day(window: &[ParsedHealthIndicatorSample]) -> f64 {
    let first_ts = window
        .first()
        .expect("slope requires non-empty window")
        .parsed_ts;
    let xs = window.iter().map(|point| {
        point
            .parsed_ts
            .signed_duration_since(first_ts)
            .num_seconds() as f64
            / 86_400.0
    });
    let ys = window.iter().map(|point| point.sample.value);
    let count = window.len() as f64;
    let mean_x = xs.clone().sum::<f64>() / count;
    let mean_y = ys.clone().sum::<f64>() / count;
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for point in window {
        let x = point
            .parsed_ts
            .signed_duration_since(first_ts)
            .num_seconds() as f64
            / 86_400.0;
        let y = point.sample.value;
        numerator += (x - mean_x) * (y - mean_y);
        denominator += (x - mean_x) * (x - mean_x);
    }
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

fn sorted_unique_evidence_refs(window: &[ParsedHealthIndicatorSample]) -> Vec<String> {
    let mut refs = window
        .iter()
        .map(|point| point.sample.source_ref.clone())
        .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    refs
}

fn health_evidence_input_ref(
    ref_kind: String,
    ref_id: String,
    value: Option<String>,
) -> Result<HealthEvidenceInputRef, FleetHealthError> {
    Ok(HealthEvidenceInputRef {
        ref_kind: normalize_required_text(ref_kind, FleetHealthError::EmptyHealthEvidenceRefKind)?,
        ref_id: normalize_required_text(ref_id, FleetHealthError::EmptyHealthEvidenceRefId)?,
        value,
    })
}

fn health_verdict_reason_code(reason_code: HealthVerdictReasonCode) -> &'static str {
    match reason_code {
        HealthVerdictReasonCode::AllIndicatorsWithinThreshold => "all_indicators_within_threshold",
        HealthVerdictReasonCode::WatchThresholdExceeded => "watch_threshold_exceeded",
        HealthVerdictReasonCode::DegradedThresholdExceeded => "degraded_threshold_exceeded",
        HealthVerdictReasonCode::CriticalThresholdExceeded => "critical_threshold_exceeded",
    }
}

fn health_degradation_reason_code(reason_code: HealthDegradationReasonCode) -> &'static str {
    match reason_code {
        HealthDegradationReasonCode::WithinTrendBand => "within_trend_band",
        HealthDegradationReasonCode::SustainedAdverseSlope => "sustained_adverse_slope",
        HealthDegradationReasonCode::DriftExceeded => "drift_exceeded",
        HealthDegradationReasonCode::InsufficientHistory => "insufficient_history",
    }
}

fn health_evidence_decision_hash(
    subject_kind: HealthEvidenceSubjectKind,
    component_id: &str,
    method_version: &str,
    reason_code: &str,
    input_refs: &[HealthEvidenceInputRef],
) -> String {
    let mut canonical = String::new();
    canonical.push_str(health_evidence_subject_kind(subject_kind));
    canonical.push('|');
    canonical.push_str(component_id);
    canonical.push('|');
    canonical.push_str(method_version);
    canonical.push('|');
    canonical.push_str(reason_code);
    for input_ref in input_refs {
        canonical.push('|');
        canonical.push_str(&input_ref.ref_kind);
        canonical.push('=');
        canonical.push_str(&input_ref.ref_id);
        canonical.push('=');
        canonical.push_str(input_ref.value.as_deref().unwrap_or(""));
    }
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
}

fn health_evidence_subject_kind(subject_kind: HealthEvidenceSubjectKind) -> &'static str {
    match subject_kind {
        HealthEvidenceSubjectKind::ComponentVerdict => "component_verdict",
        HealthEvidenceSubjectKind::DegradationEvent => "degradation_event",
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn validate_finite(value: f64) -> Result<(), FleetHealthError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(FleetHealthError::InvalidTelemetryValue)
    }
}

fn normalize_health_threshold(
    threshold: HealthIndicatorThreshold,
) -> Result<HealthIndicatorThreshold, FleetHealthError> {
    let valid = threshold.watch_at.is_finite()
        && threshold.degraded_at.is_finite()
        && threshold.critical_at.is_finite()
        && threshold.watch_at >= 0.0
        && threshold.degraded_at >= threshold.watch_at
        && threshold.critical_at >= threshold.degraded_at;

    if valid {
        Ok(threshold)
    } else {
        Err(FleetHealthError::InvalidHealthThreshold {
            indicator: threshold.indicator,
        })
    }
}

fn classify_health_indicator(
    value: f64,
    threshold: &HealthIndicatorThreshold,
) -> (ComponentHealthVerdictStatus, HealthVerdictReasonCode, f64) {
    if value >= threshold.critical_at {
        (
            ComponentHealthVerdictStatus::Critical,
            HealthVerdictReasonCode::CriticalThresholdExceeded,
            threshold.critical_at,
        )
    } else if value >= threshold.degraded_at {
        (
            ComponentHealthVerdictStatus::Degraded,
            HealthVerdictReasonCode::DegradedThresholdExceeded,
            threshold.degraded_at,
        )
    } else if value >= threshold.watch_at {
        (
            ComponentHealthVerdictStatus::Watch,
            HealthVerdictReasonCode::WatchThresholdExceeded,
            threshold.watch_at,
        )
    } else {
        (
            ComponentHealthVerdictStatus::Ok,
            HealthVerdictReasonCode::AllIndicatorsWithinThreshold,
            threshold.watch_at,
        )
    }
}

fn compare_verdict_evidence(
    left: &HealthVerdictEvidence,
    right: &HealthVerdictEvidence,
) -> std::cmp::Ordering {
    left.status
        .severity_rank()
        .cmp(&right.status.severity_rank())
        .then_with(|| {
            left.value
                .partial_cmp(&right.value)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn normalize_component_service_limit(
    limit: ComponentServiceLimit,
) -> Result<ComponentServiceLimit, FleetHealthError> {
    let component_id =
        normalize_required_text(limit.component_id, FleetHealthError::EmptyComponentId)?;
    let has_limit = limit.max_flight_hours.is_some()
        || limit.max_cycles.is_some()
        || limit.max_duty_score.is_some();
    let valid = has_limit
        && optional_nonnegative_finite(limit.max_flight_hours)
        && optional_nonnegative_finite(limit.max_duty_score);

    if valid {
        Ok(ComponentServiceLimit {
            component_id,
            max_flight_hours: limit.max_flight_hours,
            max_cycles: limit.max_cycles,
            max_duty_score: limit.max_duty_score,
        })
    } else {
        Err(FleetHealthError::InvalidServiceLimit { component_id })
    }
}

fn optional_nonnegative_finite(value: Option<f64>) -> bool {
    value.is_none_or(|value| value.is_finite() && value >= 0.0)
}

fn append_service_limit_blockers(
    component: &FleetComponentRecord,
    limit: &ComponentServiceLimit,
    blockers: &mut Vec<FleetReadinessBlocker>,
) {
    if let Some(max_flight_hours) = limit.max_flight_hours {
        if component.flight_hours > max_flight_hours {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::OverdueServiceHours,
                Some(component.component_id.clone()),
                None,
                Some(component.flight_hours),
                Some(max_flight_hours),
            ));
        }
    }

    if let Some(max_cycles) = limit.max_cycles {
        if component.cycles > max_cycles {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::OverdueServiceCycles,
                Some(component.component_id.clone()),
                None,
                Some(component.cycles as f64),
                Some(max_cycles as f64),
            ));
        }
    }

    if let Some(max_duty_score) = limit.max_duty_score {
        if component.duty_score > max_duty_score {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::OverdueDutyScore,
                Some(component.component_id.clone()),
                None,
                Some(component.duty_score),
                Some(max_duty_score),
            ));
        }
    }
}

fn readiness_blocker(
    reason_code: FleetReadinessBlockReason,
    component_ref: Option<String>,
    indicator: Option<FleetHealthIndicator>,
    observed_value: Option<f64>,
    threshold: Option<f64>,
) -> FleetReadinessBlocker {
    FleetReadinessBlocker {
        reason_code,
        component_ref,
        indicator,
        observed_value,
        threshold,
    }
}

fn normalize_service_history_entry(
    entry: ServiceHistoryEntry,
) -> Result<ServiceHistoryEntry, FleetHealthError> {
    Ok(ServiceHistoryEntry {
        service_id: normalize_required_text(entry.service_id, FleetHealthError::EmptyServiceId)?,
        performed_at: normalize_required_text(
            entry.performed_at,
            FleetHealthError::EmptyServicePerformedAt,
        )?,
        technician: normalize_required_text(
            entry.technician,
            FleetHealthError::EmptyServiceTechnician,
        )?,
        action: normalize_required_text(entry.action, FleetHealthError::EmptyServiceAction)?,
        notes: normalize_optional_text(entry.notes),
    })
}

fn normalize_work_order_parts(
    parts: Vec<MaintenancePartUsage>,
) -> Result<Vec<MaintenancePartUsage>, FleetHealthError> {
    parts
        .into_iter()
        .map(|part| {
            if part.quantity == 0 {
                return Err(FleetHealthError::InvalidPartQuantity);
            }
            Ok(MaintenancePartUsage {
                part_id: normalize_required_text(part.part_id, FleetHealthError::EmptyPartId)?,
                quantity: part.quantity,
            })
        })
        .collect()
}

fn format_work_order_parts_note(reason: &str, parts: &[MaintenancePartUsage]) -> String {
    if parts.is_empty() {
        return format!("work_order_reason={reason};parts=none");
    }
    let mut rendered_parts = parts
        .iter()
        .map(|part| format!("{}x{}", part.part_id, part.quantity))
        .collect::<Vec<_>>();
    rendered_parts.sort();
    format!(
        "work_order_reason={reason};parts={}",
        rendered_parts.join(",")
    )
}

fn normalize_required_text(
    value: String,
    error: FleetHealthError,
) -> Result<String, FleetHealthError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn validate_nonnegative_finite(
    value: f64,
    error: FleetHealthError,
) -> Result<(), FleetHealthError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        accrue_component_duty, append_health_evidence_record, apply_rollout_control,
        build_component_duty_accruals, build_component_record,
        build_degradation_event_evidence_record, build_fleet_operations_dashboard_feed,
        build_health_verdict_evidence_record, close_maintenance_work_order, component_event,
        derive_health_indicators, detect_health_indicator_degradation,
        evaluate_battery_health_trend, evaluate_component_health_verdict, evaluate_fleet_readiness,
        evaluate_fleet_readiness_with_work_orders, evaluate_ota_rollout,
        fleet_operations_source_current, fleet_operations_source_unavailable, install_component,
        open_maintenance_work_order, refuse_health_evidence_overwrite, BatteryHealthTrendConfig,
        CloseMaintenanceWorkOrderRequest, ComponentHealthVerdict, ComponentHealthVerdictRequest,
        ComponentHealthVerdictStatus, ComponentServiceLimit, DutyAccrualRequest,
        FleetComponentRecord, FleetComponentType, FleetHealthError, FleetHealthIndicator,
        FleetOperationsFeedSource, FleetOperationsFeedSourceStatus,
        FleetOperationsRolloutFeedState, FleetReadinessBlockReason, FleetReadinessDecisionStatus,
        FleetReadinessRequest, HealthDegradationDetectionConfig, HealthDegradationDetectionRequest,
        HealthDegradationDetectionStatus, HealthDegradationReasonCode, HealthEvidenceRecord,
        HealthEvidenceSubjectKind, HealthIndicatorFreshness, HealthIndicatorThreshold,
        HealthTelemetryGap, HealthTelemetrySample, HealthVerdictEvidence, HealthVerdictReasonCode,
        InstallComponentRequest, MaintenancePartUsage, MaintenanceWorkOrderSeverity,
        MaintenanceWorkOrderStatus, OpenMaintenanceWorkOrderRequest, OtaArtifactVersion,
        OtaNodeHealthReport, OtaRolloutDecisionReason, OtaRolloutDecisionStatus, OtaRolloutNode,
        OtaRolloutRequest, OtaRolloutStage, RegisterComponentRequest, RolloutControlAction,
        RolloutControlReason, RolloutControlRequest, RolloutControlStatus, ServiceHistoryEntry,
        TelemetryHealthIndicatorRequest,
    };
    use shared::fleet_alerts::{
        FleetAlertComparator, FleetAlertEvidence, FleetAlertKind, FleetAlertRecord,
        FleetAlertRoute, FleetAlertSeverity,
    };
    use shared::schemas::{
        FleetNodeComponentHealth, FleetNodeComponentStatus, FleetNodeHealthState,
        FleetNodeInventoryEntry, FleetNodeKind, FleetNodeRuntimeMode, FleetNodeStatus,
        FleetVersionInventory,
    };
    use timeseries::SeriesValue;

    #[test]
    fn component_record_normalizes_install_and_service_history() {
        let record = build_component_record(
            RegisterComponentRequest {
                component_id: Some(" battery-pack-001 ".to_string()),
                component_type: FleetComponentType::Battery,
                serial: " BAT-2026-001 ".to_string(),
                airframe_id: Some(" airframe-1 ".to_string()),
                installed_at: Some(" 2026-06-01T10:00:00Z ".to_string()),
                removed_at: None,
                service_history: vec![ServiceHistoryEntry {
                    service_id: " svc-001 ".to_string(),
                    performed_at: " 2026-06-01T09:30:00Z ".to_string(),
                    technician: " tech-1 ".to_string(),
                    action: " incoming_inspection ".to_string(),
                    notes: Some(" capacity check passed ".to_string()),
                }],
            },
            "generated-component".to_string(),
            " 2026-06-01T10:05:00Z ".to_string(),
        )
        .expect("component should be valid");

        assert_eq!(record.component_id, "battery-pack-001");
        assert_eq!(record.component_type, FleetComponentType::Battery);
        assert_eq!(record.serial, "BAT-2026-001");
        assert_eq!(record.airframe_id.as_deref(), Some("airframe-1"));
        assert_eq!(record.installed_at.as_deref(), Some("2026-06-01T10:00:00Z"));
        assert_eq!(record.service_history[0].service_id, "svc-001");
        assert_eq!(record.service_history[0].technician, "tech-1");
    }

    #[test]
    fn component_cannot_install_on_two_airframes_at_once() {
        let record = build_component_record(
            RegisterComponentRequest {
                component_id: Some("battery-pack-001".to_string()),
                component_type: FleetComponentType::Battery,
                serial: "BAT-2026-001".to_string(),
                airframe_id: Some("airframe-1".to_string()),
                installed_at: Some("2026-06-01T10:00:00Z".to_string()),
                removed_at: None,
                service_history: vec![],
            },
            "generated-component".to_string(),
            "2026-06-01T10:05:00Z".to_string(),
        )
        .expect("component should be valid");

        let error = install_component(
            &record,
            InstallComponentRequest {
                airframe_id: "airframe-2".to_string(),
                installed_at: "2026-06-02T10:00:00Z".to_string(),
                actor: Some("tech-2".to_string()),
            },
            "2026-06-02T10:00:00Z".to_string(),
        )
        .expect_err("double install should be rejected");

        assert_eq!(
            error,
            FleetHealthError::AlreadyInstalled {
                component_id: "battery-pack-001".to_string(),
                airframe_id: "airframe-1".to_string()
            }
        );
    }

    #[test]
    fn invalid_service_history_is_rejected() {
        let error = build_component_record(
            RegisterComponentRequest {
                component_id: Some("battery-pack-001".to_string()),
                component_type: FleetComponentType::Battery,
                serial: "BAT-2026-001".to_string(),
                airframe_id: None,
                installed_at: None,
                removed_at: None,
                service_history: vec![ServiceHistoryEntry {
                    service_id: "svc-001".to_string(),
                    performed_at: "2026-06-01T09:30:00Z".to_string(),
                    technician: "tech-1".to_string(),
                    action: " ".to_string(),
                    notes: None,
                }],
            },
            "generated-component".to_string(),
            "2026-06-01T10:05:00Z".to_string(),
        )
        .expect_err("empty service action should be rejected");

        assert_eq!(error, FleetHealthError::EmptyServiceAction);
    }

    #[test]
    fn component_events_are_normalized() {
        let event = component_event(
            " battery-pack-001 ",
            " installed ",
            Some(" airframe-1 ".to_string()),
            " 2026-06-01T10:00:00Z ".to_string(),
            Some(" tech-1 ".to_string()),
            Some(" initial install ".to_string()),
        )
        .expect("event should be valid");

        assert_eq!(event.component_id, "battery-pack-001");
        assert_eq!(event.event_type, "installed");
        assert_eq!(event.airframe_id.as_deref(), Some("airframe-1"));
        assert_eq!(event.actor.as_deref(), Some("tech-1"));
    }

    #[test]
    fn duty_accrual_builds_per_component_records_and_updates_totals() {
        let component = build_component_record(
            RegisterComponentRequest {
                component_id: Some("battery-pack-001".to_string()),
                component_type: FleetComponentType::Battery,
                serial: "BAT-2026-001".to_string(),
                airframe_id: Some("airframe-1".to_string()),
                installed_at: Some("2026-06-01T10:00:00Z".to_string()),
                removed_at: None,
                service_history: vec![],
            },
            "generated-component".to_string(),
            "2026-06-01T10:05:00Z".to_string(),
        )
        .expect("component should be valid");

        let accruals = build_component_duty_accruals(
            DutyAccrualRequest {
                session_id: " session-001 ".to_string(),
                airframe_id: " airframe-1 ".to_string(),
                flight_hours: 1.25,
                cycles: 1,
                duty_score: 0.8,
                ended_at: " 2026-06-03T12:15:00Z ".to_string(),
            },
            &[component.component_id.clone()],
        )
        .expect("accrual should be valid");

        assert_eq!(accruals.len(), 1);
        assert_eq!(accruals[0].session_id, "session-001");
        assert_eq!(accruals[0].component_id, "battery-pack-001");

        let updated =
            accrue_component_duty(&component, &accruals[0], "2026-06-03T12:15:00Z".to_string())
                .expect("totals should update");
        assert_eq!(updated.flight_hours, 1.25);
        assert_eq!(updated.cycles, 1);
        assert_eq!(updated.duty_score, 0.8);
    }

    #[test]
    fn duty_accrual_rejects_invalid_hours() {
        let error = build_component_duty_accruals(
            DutyAccrualRequest {
                session_id: "session-001".to_string(),
                airframe_id: "airframe-1".to_string(),
                flight_hours: -1.0,
                cycles: 1,
                duty_score: 0.8,
                ended_at: "2026-06-03T12:15:00Z".to_string(),
            },
            &["battery-pack-001".to_string()],
        )
        .expect_err("negative hours should be rejected");

        assert_eq!(error, FleetHealthError::InvalidFlightHours);
    }

    #[test]
    fn telemetry_health_indicators_derive_scalar_series_points() {
        let derived = derive_health_indicators(TelemetryHealthIndicatorRequest {
            source_ref: "telemetry:session-001".to_string(),
            created_at: "2026-06-12T12:20:00Z".to_string(),
            samples: vec![
                HealthTelemetrySample {
                    component_id: "battery-pack-001".to_string(),
                    component_type: FleetComponentType::Battery,
                    ts: "2026-06-12T12:00:00Z".to_string(),
                    battery_open_circuit_voltage_v: Some(16.8),
                    battery_voltage_v: Some(15.96),
                    battery_current_a: Some(28.0),
                    motor_vibration_g: None,
                    esc_temperature_c: None,
                },
                HealthTelemetrySample {
                    component_id: "motor-front-left".to_string(),
                    component_type: FleetComponentType::Motor,
                    ts: "2026-06-12T12:00:00Z".to_string(),
                    battery_open_circuit_voltage_v: None,
                    battery_voltage_v: None,
                    battery_current_a: None,
                    motor_vibration_g: Some(0.42),
                    esc_temperature_c: None,
                },
                HealthTelemetrySample {
                    component_id: "esc-front-left".to_string(),
                    component_type: FleetComponentType::Esc,
                    ts: "2026-06-12T12:00:00Z".to_string(),
                    battery_open_circuit_voltage_v: None,
                    battery_voltage_v: None,
                    battery_current_a: None,
                    motor_vibration_g: None,
                    esc_temperature_c: Some(54.5),
                },
            ],
            telemetry_gaps: vec![],
        })
        .expect("health indicators should derive");

        assert_eq!(derived.samples.len(), 3);
        let resistance = derived
            .samples
            .iter()
            .find(|sample| sample.indicator == FleetHealthIndicator::BatteryInternalResistance)
            .expect("resistance sample should exist");
        assert_eq!(resistance.component_id, "battery-pack-001");
        assert!((resistance.value - 30.0).abs() < 1e-9);
        assert_eq!(resistance.freshness, HealthIndicatorFreshness::Fresh);

        let point = resistance.to_series_point();
        assert_eq!(point.entity_ref, "component:battery-pack-001");
        assert_eq!(point.metric, "battery_internal_resistance_milliohm");
        assert_eq!(point.unit, "milliohm");
        assert_eq!(point.t, "2026-06-12T12:00:00Z");
        match point.value {
            SeriesValue::Scalar { value } => assert!((value - 30.0).abs() < 1e-9),
            SeriesValue::Raster(_) => panic!("health indicator should be scalar"),
        }
    }

    #[test]
    fn telemetry_dropout_records_gap_and_marks_last_indicator_stale_without_backfill() {
        let derived = derive_health_indicators(TelemetryHealthIndicatorRequest {
            source_ref: "telemetry:session-002".to_string(),
            created_at: "2026-06-12T12:20:00Z".to_string(),
            samples: vec![HealthTelemetrySample {
                component_id: "battery-pack-001".to_string(),
                component_type: FleetComponentType::Battery,
                ts: "2026-06-12T12:00:00Z".to_string(),
                battery_open_circuit_voltage_v: Some(16.8),
                battery_voltage_v: Some(16.24),
                battery_current_a: Some(28.0),
                motor_vibration_g: None,
                esc_temperature_c: None,
            }],
            telemetry_gaps: vec![HealthTelemetryGap {
                component_id: "battery-pack-001".to_string(),
                started_at: "2026-06-12T12:01:00Z".to_string(),
                ended_at: "2026-06-12T12:05:00Z".to_string(),
                reason: "mavlink-radio-dropout".to_string(),
            }],
        })
        .expect("health indicators should derive with gap");

        assert_eq!(derived.gaps.len(), 1);
        assert_eq!(derived.gaps[0].reason, "mavlink-radio-dropout");
        assert_eq!(derived.samples.len(), 1);
        assert_eq!(
            derived.samples[0].freshness,
            HealthIndicatorFreshness::Stale
        );
        assert_ne!(derived.samples[0].ts, "2026-06-12T12:01:00Z");
    }

    #[test]
    fn battery_cycle_count_and_resistance_trend_tracks_completed_discharge() {
        let component =
            component_for_readiness("battery-pack-001", FleetComponentType::Battery, 0.0, 0, 0.0);
        let accruals = build_component_duty_accruals(
            DutyAccrualRequest {
                session_id: "session-001".to_string(),
                airframe_id: "airframe-1".to_string(),
                flight_hours: 0.6,
                cycles: 1,
                duty_score: 0.4,
                ended_at: "2026-06-12T12:10:00Z".to_string(),
            },
            &[component.component_id.clone()],
        )
        .expect("battery discharge cycle should accrue");
        let updated =
            accrue_component_duty(&component, &accruals[0], "2026-06-12T12:10:00Z".to_string())
                .expect("battery cycle total should update");
        let derived = derive_health_indicators(TelemetryHealthIndicatorRequest {
            source_ref: "telemetry:session-001".to_string(),
            created_at: "2026-06-12T12:20:00Z".to_string(),
            samples: vec![HealthTelemetrySample {
                component_id: "battery-pack-001".to_string(),
                component_type: FleetComponentType::Battery,
                ts: "2026-06-12T12:09:00Z".to_string(),
                battery_open_circuit_voltage_v: Some(16.8),
                battery_voltage_v: Some(16.24),
                battery_current_a: Some(28.0),
                motor_vibration_g: None,
                esc_temperature_c: None,
            }],
            telemetry_gaps: vec![],
        })
        .expect("resistance sample should derive from telemetry");

        let trend = evaluate_battery_health_trend(
            &updated,
            derived.samples,
            BatteryHealthTrendConfig {
                max_cycles: 200,
                resistance_degraded_at_milliohm: 60.0,
            },
            "2026-06-12T12:30:00Z".to_string(),
        )
        .expect("battery trend should evaluate");

        assert_eq!(trend.component_id, "battery-pack-001");
        assert_eq!(trend.cycle_count, 1);
        assert_eq!(trend.resistance_sample_count, 1);
        assert!((trend.latest_internal_resistance_milliohm - 20.0).abs() < 1e-9);
        assert_eq!(trend.status, ComponentHealthVerdictStatus::Ok);
        assert_eq!(
            trend.evidence[0].indicator,
            FleetHealthIndicator::BatteryCycleCount
        );
        assert_eq!(
            trend.evidence[1].indicator,
            FleetHealthIndicator::BatteryInternalResistance
        );
    }

    #[test]
    fn battery_trend_flags_degraded_pack_over_cycle_limit_with_evidence() {
        let component = component_for_readiness(
            "battery-pack-001",
            FleetComponentType::Battery,
            80.0,
            201,
            70.0,
        );
        let trend = evaluate_battery_health_trend(
            &component,
            vec![indicator_sample(
                "battery-pack-001",
                FleetHealthIndicator::BatteryInternalResistance,
                31.0,
            )],
            BatteryHealthTrendConfig {
                max_cycles: 200,
                resistance_degraded_at_milliohm: 60.0,
            },
            "2026-06-12T12:30:00Z".to_string(),
        )
        .expect("battery trend should evaluate");

        assert_eq!(trend.status, ComponentHealthVerdictStatus::Degraded);
        let cycle_evidence = trend
            .evidence
            .iter()
            .find(|item| item.indicator == FleetHealthIndicator::BatteryCycleCount)
            .expect("cycle evidence should be present");
        assert_eq!(cycle_evidence.value, 201.0);
        assert_eq!(cycle_evidence.threshold, 200.0);
        assert_eq!(
            cycle_evidence.reason_code,
            HealthVerdictReasonCode::DegradedThresholdExceeded
        );
    }

    #[test]
    fn degradation_detector_keeps_stable_series_without_event() {
        let report = detect_health_indicator_degradation(degradation_request(vec![
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.20,
                "2026-06-01T00:00:00Z",
                "telemetry:mission-001",
            ),
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.21,
                "2026-06-08T00:00:00Z",
                "telemetry:mission-002",
            ),
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.20,
                "2026-06-15T00:00:00Z",
                "telemetry:mission-003",
            ),
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.22,
                "2026-06-22T00:00:00Z",
                "telemetry:mission-004",
            ),
        ]))
        .expect("stable series should evaluate");

        assert_eq!(report.status, HealthDegradationDetectionStatus::Stable);
        assert_eq!(
            report.reason_code,
            HealthDegradationReasonCode::WithinTrendBand
        );
        assert!(report.event.is_none());
        assert!(report.slope_per_day.expect("slope should be present") < 0.02);
    }

    #[test]
    fn degradation_detector_raises_event_for_sustained_adverse_slope() {
        let report = detect_health_indicator_degradation(degradation_request(vec![
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.20,
                "2026-06-01T00:00:00Z",
                "telemetry:mission-001",
            ),
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.35,
                "2026-06-08T00:00:00Z",
                "telemetry:mission-002",
            ),
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.50,
                "2026-06-15T00:00:00Z",
                "telemetry:mission-003",
            ),
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.65,
                "2026-06-22T00:00:00Z",
                "telemetry:mission-004",
            ),
        ]))
        .expect("degrading series should evaluate");

        assert_eq!(
            report.status,
            HealthDegradationDetectionStatus::DegradationDetected
        );
        assert_eq!(
            report.reason_code,
            HealthDegradationReasonCode::SustainedAdverseSlope
        );
        let event = report.event.expect("event should be raised");
        assert_eq!(event.window_start, "2026-06-01T00:00:00Z");
        assert_eq!(event.window_end, "2026-06-22T00:00:00Z");
        assert!(event.slope_per_day >= 0.02);
        assert_eq!(
            event.evidence_refs,
            vec![
                "telemetry:mission-001".to_string(),
                "telemetry:mission-002".to_string(),
                "telemetry:mission-003".to_string(),
                "telemetry:mission-004".to_string()
            ]
        );
    }

    #[test]
    fn degradation_detector_surfaces_insufficient_history_without_trend() {
        let report = detect_health_indicator_degradation(degradation_request(vec![
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.20,
                "2026-06-01T00:00:00Z",
                "telemetry:mission-001",
            ),
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.50,
                "2026-06-08T00:00:00Z",
                "telemetry:mission-002",
            ),
        ]))
        .expect("insufficient history should be reported, not errored");

        assert_eq!(
            report.status,
            HealthDegradationDetectionStatus::InsufficientHistory
        );
        assert_eq!(
            report.reason_code,
            HealthDegradationReasonCode::InsufficientHistory
        );
        assert!(report.event.is_none());
        assert!(report.slope_per_day.is_none());
        assert!(report.evidence_refs.is_empty());
    }

    #[test]
    fn health_verdict_evidence_rerun_produces_identical_hash() {
        let verdict = evaluate_component_health_verdict(ComponentHealthVerdictRequest {
            component_id: "motor-001".to_string(),
            evaluated_at: "2026-06-22T12:30:00Z".to_string(),
            method_version: "fleet-health-thresholds-v1".to_string(),
            samples: vec![indicator_sample(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                1.2,
            )],
            thresholds: vec![threshold(
                FleetHealthIndicator::MotorVibration,
                0.6,
                1.0,
                1.5,
            )],
        })
        .expect("verdict should evaluate");

        let first = build_health_verdict_evidence_record(
            "health-evidence:motor-001:thresholds-v1".to_string(),
            &verdict,
            "2026-06-22T12:31:00Z".to_string(),
        )
        .expect("evidence should build");
        let second = build_health_verdict_evidence_record(
            "health-evidence:motor-001:thresholds-v1".to_string(),
            &verdict,
            "2026-06-22T12:45:00Z".to_string(),
        )
        .expect("rerun evidence should build");

        assert_eq!(first.decision_hash, second.decision_hash);
        assert_eq!(first.reason_code, "degraded_threshold_exceeded");
        assert_eq!(
            first.subject_kind,
            HealthEvidenceSubjectKind::ComponentVerdict
        );
        assert_eq!(first.input_refs.len(), 1);
    }

    #[test]
    fn health_evidence_appends_method_version_change_without_overwriting_prior() {
        let old_verdict = component_verdict(
            "motor-001",
            ComponentHealthVerdictStatus::Watch,
            HealthIndicatorFreshness::Fresh,
        );
        let mut new_verdict = component_verdict(
            "motor-001",
            ComponentHealthVerdictStatus::Degraded,
            HealthIndicatorFreshness::Fresh,
        );
        new_verdict.method_version = "fleet-health-thresholds-v2".to_string();
        let first = build_health_verdict_evidence_record(
            "health-evidence:motor-001:thresholds-v1".to_string(),
            &old_verdict,
            "2026-06-22T12:31:00Z".to_string(),
        )
        .expect("old evidence should build");
        let second = build_health_verdict_evidence_record(
            "health-evidence:motor-001:thresholds-v2".to_string(),
            &new_verdict,
            "2026-06-22T13:31:00Z".to_string(),
        )
        .expect("new evidence should build");

        let mut records = Vec::new();
        append_health_evidence_record(&mut records, first.clone())
            .expect("first append should work");
        append_health_evidence_record(&mut records, second.clone())
            .expect("new method append should work");

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].method_version, "fleet-health-thresholds-v1");
        assert_eq!(records[1].method_version, "fleet-health-thresholds-v2");
        assert_ne!(records[0].decision_hash, records[1].decision_hash);
    }

    #[test]
    fn health_evidence_refuses_in_place_overwrite() {
        let existing = health_evidence_record("health-evidence:motor-001", "hash-a");
        let replacement = health_evidence_record("health-evidence:motor-001", "hash-b");
        let error = refuse_health_evidence_overwrite(&existing, &replacement)
            .expect_err("different hash under same record id should be refused");

        assert_eq!(
            error,
            FleetHealthError::HealthEvidenceOverwriteRefused {
                record_id: "health-evidence:motor-001".to_string()
            }
        );
    }

    #[test]
    fn degradation_event_evidence_retains_series_window_refs() {
        let report = detect_health_indicator_degradation(degradation_request(vec![
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.20,
                "2026-06-01T00:00:00Z",
                "telemetry:mission-001",
            ),
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.35,
                "2026-06-08T00:00:00Z",
                "telemetry:mission-002",
            ),
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.50,
                "2026-06-15T00:00:00Z",
                "telemetry:mission-003",
            ),
            indicator_sample_at(
                "motor-001",
                FleetHealthIndicator::MotorVibration,
                0.65,
                "2026-06-22T00:00:00Z",
                "telemetry:mission-004",
            ),
        ]))
        .expect("degradation report should evaluate");
        let evidence = build_degradation_event_evidence_record(
            "health-evidence:motor-001:degradation-v1".to_string(),
            &report,
            "2026-06-22T12:31:00Z".to_string(),
        )
        .expect("degradation evidence should build");

        assert_eq!(
            evidence.subject_kind,
            HealthEvidenceSubjectKind::DegradationEvent
        );
        assert_eq!(evidence.reason_code, "sustained_adverse_slope");
        assert_eq!(evidence.input_refs.len(), 4);
        assert!(evidence.input_refs[0]
            .value
            .as_deref()
            .expect("window value should be present")
            .contains("window_start=2026-06-01T00:00:00Z"));
    }

    #[test]
    fn component_verdict_is_ok_when_indicators_are_within_thresholds() {
        let verdict = evaluate_component_health_verdict(ComponentHealthVerdictRequest {
            component_id: "battery-pack-001".to_string(),
            evaluated_at: "2026-06-12T12:30:00Z".to_string(),
            method_version: "fleet-health-thresholds-v1".to_string(),
            samples: vec![indicator_sample(
                "battery-pack-001",
                FleetHealthIndicator::BatteryInternalResistance,
                31.0,
            )],
            thresholds: vec![threshold(
                FleetHealthIndicator::BatteryInternalResistance,
                60.0,
                85.0,
                110.0,
            )],
        })
        .expect("verdict should evaluate");

        assert_eq!(verdict.status, ComponentHealthVerdictStatus::Ok);
        assert_eq!(
            verdict.reason_code,
            HealthVerdictReasonCode::AllIndicatorsWithinThreshold
        );
        assert_eq!(
            verdict.indicator,
            Some(FleetHealthIndicator::BatteryInternalResistance)
        );
        assert_eq!(verdict.threshold, Some(60.0));
        assert_eq!(verdict.value, Some(31.0));
        assert_eq!(verdict.evidence.len(), 1);
    }

    #[test]
    fn critical_indicator_sets_component_verdict_with_threshold_evidence() {
        let verdict = evaluate_component_health_verdict(ComponentHealthVerdictRequest {
            component_id: "motor-front-left".to_string(),
            evaluated_at: "2026-06-12T12:30:00Z".to_string(),
            method_version: "fleet-health-thresholds-v1".to_string(),
            samples: vec![
                indicator_sample(
                    "motor-front-left",
                    FleetHealthIndicator::MotorVibration,
                    1.8,
                ),
                indicator_sample(
                    "motor-front-left",
                    FleetHealthIndicator::EscTemperature,
                    72.0,
                ),
            ],
            thresholds: vec![
                threshold(FleetHealthIndicator::MotorVibration, 0.6, 1.0, 1.5),
                threshold(FleetHealthIndicator::EscTemperature, 70.0, 85.0, 100.0),
            ],
        })
        .expect("verdict should evaluate");

        assert_eq!(verdict.status, ComponentHealthVerdictStatus::Critical);
        assert_eq!(
            verdict.reason_code,
            HealthVerdictReasonCode::CriticalThresholdExceeded
        );
        assert_eq!(
            verdict.indicator,
            Some(FleetHealthIndicator::MotorVibration)
        );
        assert_eq!(verdict.threshold, Some(1.5));
        assert_eq!(verdict.value, Some(1.8));
        assert_eq!(verdict.evidence[0].source_ref, "telemetry:session-001");
    }

    #[test]
    fn verdict_refuses_indicator_without_configured_threshold() {
        let error = evaluate_component_health_verdict(ComponentHealthVerdictRequest {
            component_id: "motor-front-left".to_string(),
            evaluated_at: "2026-06-12T12:30:00Z".to_string(),
            method_version: "fleet-health-thresholds-v1".to_string(),
            samples: vec![indicator_sample(
                "motor-front-left",
                FleetHealthIndicator::MotorVibration,
                0.7,
            )],
            thresholds: vec![threshold(
                FleetHealthIndicator::BatteryInternalResistance,
                60.0,
                85.0,
                110.0,
            )],
        })
        .expect_err("missing threshold should be rejected");

        assert_eq!(
            error,
            FleetHealthError::MissingHealthThreshold {
                indicator: FleetHealthIndicator::MotorVibration
            }
        );
    }

    #[test]
    fn readiness_allows_airframe_with_fresh_verdicts_and_service_in_limits() {
        let decision = evaluate_fleet_readiness(readiness_request(
            vec![
                component_for_readiness(
                    "battery-pack-001",
                    FleetComponentType::Battery,
                    15.0,
                    20,
                    10.0,
                ),
                component_for_readiness(
                    "motor-front-left",
                    FleetComponentType::Motor,
                    15.0,
                    20,
                    10.0,
                ),
            ],
            vec![
                service_limit("battery-pack-001", Some(100.0), Some(200), Some(100.0)),
                service_limit("motor-front-left", Some(100.0), Some(200), Some(100.0)),
            ],
            vec![
                component_verdict(
                    "battery-pack-001",
                    ComponentHealthVerdictStatus::Ok,
                    HealthIndicatorFreshness::Fresh,
                ),
                component_verdict(
                    "motor-front-left",
                    ComponentHealthVerdictStatus::Watch,
                    HealthIndicatorFreshness::Fresh,
                ),
            ],
        ))
        .expect("readiness should evaluate");

        assert_eq!(decision.status, FleetReadinessDecisionStatus::Permitted);
        assert!(decision.blockers.is_empty());
        assert_eq!(decision.component_count, 2);
    }

    #[test]
    fn readiness_hard_blocks_overdue_service_interval() {
        let decision = evaluate_fleet_readiness(readiness_request(
            vec![component_for_readiness(
                "battery-pack-001",
                FleetComponentType::Battery,
                121.0,
                20,
                10.0,
            )],
            vec![service_limit(
                "battery-pack-001",
                Some(100.0),
                Some(200),
                Some(100.0),
            )],
            vec![component_verdict(
                "battery-pack-001",
                ComponentHealthVerdictStatus::Ok,
                HealthIndicatorFreshness::Fresh,
            )],
        ))
        .expect("readiness should evaluate");

        assert_eq!(decision.status, FleetReadinessDecisionStatus::Blocked);
        assert_eq!(
            decision.blockers[0].reason_code,
            FleetReadinessBlockReason::OverdueServiceHours
        );
        assert_eq!(
            decision.blockers[0].component_ref.as_deref(),
            Some("battery-pack-001")
        );
        assert_eq!(decision.blockers[0].observed_value, Some(121.0));
        assert_eq!(decision.blockers[0].threshold, Some(100.0));
    }

    #[test]
    fn readiness_hard_blocks_active_critical_health_verdict() {
        let decision = evaluate_fleet_readiness(readiness_request(
            vec![component_for_readiness(
                "motor-front-left",
                FleetComponentType::Motor,
                15.0,
                20,
                10.0,
            )],
            vec![service_limit(
                "motor-front-left",
                Some(100.0),
                Some(200),
                Some(100.0),
            )],
            vec![component_verdict(
                "motor-front-left",
                ComponentHealthVerdictStatus::Critical,
                HealthIndicatorFreshness::Fresh,
            )],
        ))
        .expect("readiness should evaluate");

        assert_eq!(decision.status, FleetReadinessDecisionStatus::Blocked);
        assert_eq!(
            decision.blockers[0].reason_code,
            FleetReadinessBlockReason::CriticalHealthVerdict
        );
        assert_eq!(
            decision.blockers[0].component_ref.as_deref(),
            Some("motor-front-left")
        );
    }

    #[test]
    fn readiness_hard_blocks_degraded_battery_health() {
        let decision = evaluate_fleet_readiness(readiness_request(
            vec![component_for_readiness(
                "battery-pack-001",
                FleetComponentType::Battery,
                15.0,
                20,
                10.0,
            )],
            vec![service_limit(
                "battery-pack-001",
                Some(100.0),
                Some(200),
                Some(100.0),
            )],
            vec![component_verdict(
                "battery-pack-001",
                ComponentHealthVerdictStatus::Degraded,
                HealthIndicatorFreshness::Fresh,
            )],
        ))
        .expect("readiness should evaluate");

        assert_eq!(decision.status, FleetReadinessDecisionStatus::Blocked);
        assert_eq!(
            decision.blockers[0].reason_code,
            FleetReadinessBlockReason::BatteryHealthBelowThreshold
        );
        assert_eq!(
            decision.blockers[0].component_ref.as_deref(),
            Some("battery-pack-001")
        );
    }

    #[test]
    fn readiness_denies_missing_or_stale_health_data_by_default() {
        let stale = evaluate_fleet_readiness(readiness_request(
            vec![component_for_readiness(
                "battery-pack-001",
                FleetComponentType::Battery,
                15.0,
                20,
                10.0,
            )],
            vec![service_limit(
                "battery-pack-001",
                Some(100.0),
                Some(200),
                Some(100.0),
            )],
            vec![component_verdict(
                "battery-pack-001",
                ComponentHealthVerdictStatus::Ok,
                HealthIndicatorFreshness::Stale,
            )],
        ))
        .expect("readiness should evaluate");

        assert_eq!(stale.status, FleetReadinessDecisionStatus::Blocked);
        assert_eq!(
            stale.blockers[0].reason_code,
            FleetReadinessBlockReason::StaleHealthData
        );

        let missing = evaluate_fleet_readiness(readiness_request(
            vec![component_for_readiness(
                "motor-front-left",
                FleetComponentType::Motor,
                15.0,
                20,
                10.0,
            )],
            vec![service_limit(
                "motor-front-left",
                Some(100.0),
                Some(200),
                Some(100.0),
            )],
            vec![],
        ))
        .expect("readiness should evaluate");

        assert_eq!(missing.status, FleetReadinessDecisionStatus::Blocked);
        assert_eq!(
            missing.blockers[0].reason_code,
            FleetReadinessBlockReason::MissingHealthData
        );
    }

    #[test]
    fn work_order_close_records_parts_and_updates_service_history() {
        let component =
            component_for_readiness("motor-001", FleetComponentType::Motor, 10.0, 0, 4.0);
        let work_order = open_maintenance_work_order(
            OpenMaintenanceWorkOrderRequest {
                wo_id: Some("wo-001".to_string()),
                component_id: "motor-001".to_string(),
                reason: "motor vibration above degraded threshold".to_string(),
                severity: MaintenanceWorkOrderSeverity::Degraded,
                opened_at: "2026-06-22T13:00:00Z".to_string(),
                technician: "tech-1".to_string(),
            },
            "generated-wo".to_string(),
        )
        .expect("work order should open");
        let closed = close_maintenance_work_order(
            &component,
            &work_order,
            CloseMaintenanceWorkOrderRequest {
                closed_at: "2026-06-22T15:00:00Z".to_string(),
                technician: "tech-2".to_string(),
                action: "replaced motor bearing".to_string(),
                parts: vec![MaintenancePartUsage {
                    part_id: "bearing-kit-22".to_string(),
                    quantity: 1,
                }],
            },
            "2026-06-22T15:01:00Z".to_string(),
        )
        .expect("work order should close");

        assert_eq!(closed.work_order.status, MaintenanceWorkOrderStatus::Closed);
        assert_eq!(closed.work_order.parts[0].part_id, "bearing-kit-22");
        assert_eq!(closed.component.service_history.len(), 1);
        assert_eq!(closed.component.service_history[0].service_id, "wo-001");
        assert_eq!(
            closed.component.service_history[0].action,
            "replaced motor bearing"
        );
        assert!(closed.component.service_history[0]
            .notes
            .as_deref()
            .expect("parts note should be present")
            .contains("bearing-kit-22x1"));
    }

    #[test]
    fn open_critical_work_order_keeps_readiness_blocked_until_closed() {
        let component =
            component_for_readiness("motor-001", FleetComponentType::Motor, 10.0, 0, 4.0);
        let request = readiness_request(
            vec![component],
            vec![service_limit("motor-001", Some(100.0), None, Some(50.0))],
            vec![component_verdict(
                "motor-001",
                ComponentHealthVerdictStatus::Ok,
                HealthIndicatorFreshness::Fresh,
            )],
        );
        let work_order = open_maintenance_work_order(
            OpenMaintenanceWorkOrderRequest {
                wo_id: Some("wo-critical-001".to_string()),
                component_id: "motor-001".to_string(),
                reason: "propulsion critical inspection".to_string(),
                severity: MaintenanceWorkOrderSeverity::Critical,
                opened_at: "2026-06-22T13:00:00Z".to_string(),
                technician: "tech-1".to_string(),
            },
            "generated-wo".to_string(),
        )
        .expect("critical work order should open");

        let blocked = evaluate_fleet_readiness_with_work_orders(request.clone(), &[work_order])
            .expect("readiness should evaluate");
        assert_eq!(blocked.status, FleetReadinessDecisionStatus::Blocked);
        assert_eq!(
            blocked.blockers[0].reason_code,
            FleetReadinessBlockReason::OpenCriticalWorkOrder
        );

        let permitted = evaluate_fleet_readiness_with_work_orders(request, &[])
            .expect("readiness should evaluate after closure");
        assert_eq!(permitted.status, FleetReadinessDecisionStatus::Permitted);
    }

    #[test]
    fn work_order_rejects_zero_quantity_part() {
        let component =
            component_for_readiness("motor-001", FleetComponentType::Motor, 10.0, 0, 4.0);
        let work_order = open_maintenance_work_order(
            OpenMaintenanceWorkOrderRequest {
                wo_id: Some("wo-001".to_string()),
                component_id: "motor-001".to_string(),
                reason: "motor vibration above degraded threshold".to_string(),
                severity: MaintenanceWorkOrderSeverity::Degraded,
                opened_at: "2026-06-22T13:00:00Z".to_string(),
                technician: "tech-1".to_string(),
            },
            "generated-wo".to_string(),
        )
        .expect("work order should open");

        let error = close_maintenance_work_order(
            &component,
            &work_order,
            CloseMaintenanceWorkOrderRequest {
                closed_at: "2026-06-22T15:00:00Z".to_string(),
                technician: "tech-2".to_string(),
                action: "attempted parts update".to_string(),
                parts: vec![MaintenancePartUsage {
                    part_id: "bearing-kit-22".to_string(),
                    quantity: 0,
                }],
            },
            "2026-06-22T15:01:00Z".to_string(),
        )
        .expect_err("zero quantity part should be rejected");

        assert_eq!(error, FleetHealthError::InvalidPartQuantity);
    }

    #[test]
    fn ota_rollout_advances_canary_when_health_and_alerts_are_clear() {
        let decision = evaluate_ota_rollout(ota_rollout_request(
            OtaRolloutStage::Canary,
            Some(signed_version("agbot-edge", "1.9.0")),
            vec![ota_node("node-canary-1", OtaRolloutStage::Canary)],
            vec![ota_health(
                "node-canary-1",
                ComponentHealthVerdictStatus::Ok,
                vec![],
            )],
        ))
        .expect("rollout should evaluate");

        assert_eq!(decision.status, OtaRolloutDecisionStatus::Advance);
        assert_eq!(decision.current_stage, OtaRolloutStage::Canary);
        assert_eq!(decision.next_stage, Some(OtaRolloutStage::Staged));
        assert_eq!(decision.reason_code, OtaRolloutDecisionReason::StageHealthy);
        assert!(decision.rollback_actions.is_empty());
    }

    #[test]
    fn ota_rollout_rolls_back_regressed_stage_nodes() {
        let decision = evaluate_ota_rollout(ota_rollout_request(
            OtaRolloutStage::Staged,
            Some(signed_version("agbot-edge", "1.9.0")),
            vec![
                ota_node("node-staged-1", OtaRolloutStage::Staged),
                ota_node("node-staged-2", OtaRolloutStage::Staged),
            ],
            vec![
                ota_health(
                    "node-staged-1",
                    ComponentHealthVerdictStatus::Ok,
                    vec!["alert:disk-full"],
                ),
                ota_health("node-staged-2", ComponentHealthVerdictStatus::Ok, vec![]),
            ],
        ))
        .expect("rollout should evaluate");

        assert_eq!(decision.status, OtaRolloutDecisionStatus::HaltedRolledBack);
        assert_eq!(
            decision.reason_code,
            OtaRolloutDecisionReason::HealthRegression
        );
        assert_eq!(decision.next_stage, None);
        assert_eq!(decision.rollback_actions.len(), 1);
        assert_eq!(decision.rollback_actions[0].node_id, "node-staged-1");
        assert_eq!(decision.rollback_actions[0].from_version, "2.0.0");
        assert_eq!(decision.rollback_actions[0].to_version, "1.9.0");
        assert_eq!(
            decision.rollback_actions[0].blocking_alerts,
            vec!["alert:disk-full".to_string()]
        );
    }

    #[test]
    fn ota_rollout_refuses_regression_without_signed_rollback_target() {
        let decision = evaluate_ota_rollout(ota_rollout_request(
            OtaRolloutStage::Canary,
            Some(OtaArtifactVersion {
                artifact: "agbot-edge".to_string(),
                version: "1.9.0".to_string(),
                signed: false,
            }),
            vec![ota_node("node-canary-1", OtaRolloutStage::Canary)],
            vec![ota_health(
                "node-canary-1",
                ComponentHealthVerdictStatus::Critical,
                vec![],
            )],
        ))
        .expect("rollout should evaluate");

        assert_eq!(decision.status, OtaRolloutDecisionStatus::Refused);
        assert_eq!(
            decision.reason_code,
            OtaRolloutDecisionReason::MissingSignedRollbackTarget
        );
        assert_eq!(decision.current_stage, OtaRolloutStage::Canary);
        assert_eq!(decision.next_stage, None);
        assert!(decision.rollback_actions.is_empty());
    }

    #[test]
    fn rollout_control_pause_takes_effect_and_records_audit() {
        let decision = apply_rollout_control(rollout_control_request(
            RolloutControlAction::Pause,
            true,
            true,
        ))
        .expect("control action should evaluate");

        assert_eq!(decision.status, RolloutControlStatus::Paused);
        assert_eq!(decision.reason_code, RolloutControlReason::PausedByOperator);
        assert_eq!(decision.audit.actor, "ops@example.com");
        assert_eq!(decision.audit.action, RolloutControlAction::Pause);
        assert_eq!(decision.audit.version, "2.0.0");
        assert_eq!(decision.audit.stage, OtaRolloutStage::Staged);
        assert_eq!(decision.audit.result, RolloutControlStatus::Paused);
    }

    #[test]
    fn rollout_control_abort_takes_effect_and_records_audit() {
        let decision = apply_rollout_control(rollout_control_request(
            RolloutControlAction::Abort,
            true,
            true,
        ))
        .expect("control action should evaluate");

        assert_eq!(decision.status, RolloutControlStatus::Aborted);
        assert_eq!(
            decision.reason_code,
            RolloutControlReason::AbortedByOperator
        );
        assert_eq!(decision.audit.actor, "ops@example.com");
        assert_eq!(decision.audit.stage, OtaRolloutStage::Staged);
        assert_eq!(decision.audit.result, RolloutControlStatus::Aborted);
    }

    #[test]
    fn rollout_control_refuses_flight_targets_until_simulation_validates() {
        let decision = apply_rollout_control(rollout_control_request(
            RolloutControlAction::Start,
            false,
            true,
        ))
        .expect("control action should evaluate");

        assert_eq!(decision.status, RolloutControlStatus::Refused);
        assert_eq!(
            decision.reason_code,
            RolloutControlReason::SimulationValidationRequired
        );
        assert_eq!(decision.audit.actor, "ops@example.com");
        assert_eq!(decision.audit.result, RolloutControlStatus::Refused);
    }

    #[test]
    fn fleet_operations_feed_reflects_inventory_alerts_and_rollout_state() {
        let rollout = evaluate_ota_rollout(ota_rollout_request(
            OtaRolloutStage::Canary,
            Some(signed_version("agbot-edge", "1.9.0")),
            vec![ota_node("node-canary-1", OtaRolloutStage::Canary)],
            vec![ota_health(
                "node-canary-1",
                ComponentHealthVerdictStatus::Ok,
                vec![],
            )],
        ))
        .expect("rollout should evaluate");
        let control = apply_rollout_control(rollout_control_request(
            RolloutControlAction::Start,
            true,
            true,
        ))
        .expect("control action should evaluate");

        let feed = build_fleet_operations_dashboard_feed(
            "2026-06-12T13:01:00Z",
            sample_fleet_inventory(),
            vec![sample_fleet_alert()],
            vec![rollout],
            vec![control],
            vec![
                fleet_operations_source_current(
                    FleetOperationsFeedSource::Inventory,
                    "2026-06-12T13:00:59Z",
                ),
                fleet_operations_source_current(
                    FleetOperationsFeedSource::Alerts,
                    "2026-06-12T13:00:58Z",
                ),
                fleet_operations_source_current(
                    FleetOperationsFeedSource::Rollouts,
                    "2026-06-12T13:00:57Z",
                ),
            ],
        );

        assert_eq!(feed.inventory.entries[0].node_id, "node-canary-1");
        assert_eq!(feed.alerts[0].node_id, "node-canary-1");
        assert!(feed.source_gaps.is_empty());
        assert!(feed.rollouts.iter().any(|rollout| rollout.state
            == FleetOperationsRolloutFeedState::Advancing
            && rollout.reason_code == "stage_healthy"));
        assert!(feed.rollouts.iter().any(|rollout| rollout.state
            == FleetOperationsRolloutFeedState::Started
            && rollout.version.as_deref() == Some("2.0.0")));
    }

    #[test]
    fn fleet_operations_feed_surfaces_unavailable_source_gap() {
        let feed = build_fleet_operations_dashboard_feed(
            "2026-06-12T13:02:00Z",
            sample_fleet_inventory(),
            vec![],
            vec![],
            vec![],
            vec![fleet_operations_source_unavailable(
                FleetOperationsFeedSource::Alerts,
                "2026-06-12T13:01:59Z",
                "alert store unavailable",
            )],
        );

        assert_eq!(feed.alerts.len(), 0);
        assert_eq!(feed.source_gaps.len(), 1);
        assert_eq!(
            feed.source_gaps[0].source,
            FleetOperationsFeedSource::Alerts
        );
        assert_eq!(
            feed.source_gaps[0].status,
            FleetOperationsFeedSourceStatus::Unavailable
        );
        assert_eq!(
            feed.source_gaps[0].message.as_deref(),
            Some("alert store unavailable")
        );
    }

    fn sample_fleet_inventory() -> FleetVersionInventory {
        FleetVersionInventory {
            entries: vec![FleetNodeInventoryEntry {
                node_id: "node-canary-1".to_string(),
                owner_org_id: "org-ops".to_string(),
                kind: FleetNodeKind::Drone,
                runtime_mode: FleetNodeRuntimeMode::Flight,
                status: FleetNodeStatus::Enrolled,
                maintenance: false,
                version: Some("agbot-edge 2.0.0".to_string()),
                config_version: Some(4),
                components: vec![FleetNodeComponentStatus {
                    component: "flight-controller".to_string(),
                    health: FleetNodeComponentHealth::Ok,
                    message: None,
                }],
                capabilities: vec!["ota".to_string(), "multispectral".to_string()],
                health_state: Some(FleetNodeHealthState::Fresh),
                heartbeat_age_seconds: Some(4),
            }],
        }
    }

    fn sample_fleet_alert() -> FleetAlertRecord {
        FleetAlertRecord {
            alert_id: "fleet-alert:node-canary-1:low_disk".to_string(),
            node_id: "node-canary-1".to_string(),
            correlation_id: Some("rollout-2026-06-12".to_string()),
            kind: FleetAlertKind::LowDisk,
            severity: FleetAlertSeverity::Warning,
            route: FleetAlertRoute::OperatorConsole,
            evidence: FleetAlertEvidence {
                metric_name: "disk_free_gb".to_string(),
                observed_value: 8.0,
                threshold_value: 10.0,
                comparator: FleetAlertComparator::LessThanOrEqual,
            },
            message: "disk free capacity is below threshold".to_string(),
            evaluated_at: chrono::DateTime::parse_from_rfc3339("2026-06-12T13:00:30Z")
                .expect("valid timestamp")
                .with_timezone(&chrono::Utc),
        }
    }

    fn indicator_sample(
        component_id: &str,
        indicator: FleetHealthIndicator,
        value: f64,
    ) -> super::FleetHealthIndicatorSample {
        super::FleetHealthIndicatorSample {
            component_id: component_id.to_string(),
            indicator,
            value,
            ts: "2026-06-12T12:00:00Z".to_string(),
            source_ref: "telemetry:session-001".to_string(),
            created_at: "2026-06-12T12:20:00Z".to_string(),
            freshness: HealthIndicatorFreshness::Fresh,
        }
    }

    fn indicator_sample_at(
        component_id: &str,
        indicator: FleetHealthIndicator,
        value: f64,
        ts: &str,
        source_ref: &str,
    ) -> super::FleetHealthIndicatorSample {
        super::FleetHealthIndicatorSample {
            component_id: component_id.to_string(),
            indicator,
            value,
            ts: ts.to_string(),
            source_ref: source_ref.to_string(),
            created_at: "2026-06-22T12:20:00Z".to_string(),
            freshness: HealthIndicatorFreshness::Fresh,
        }
    }

    fn degradation_request(
        samples: Vec<super::FleetHealthIndicatorSample>,
    ) -> HealthDegradationDetectionRequest {
        HealthDegradationDetectionRequest {
            component_id: "motor-001".to_string(),
            evaluated_at: "2026-06-22T12:30:00Z".to_string(),
            method_version: "fleet-health-degradation-v1".to_string(),
            indicator: FleetHealthIndicator::MotorVibration,
            samples,
            config: HealthDegradationDetectionConfig {
                min_history_points: 4,
                recent_window_points: 4,
                min_adverse_slope_per_day: 0.02,
                min_adverse_delta: 0.30,
            },
        }
    }

    fn health_evidence_record(record_id: &str, decision_hash: &str) -> HealthEvidenceRecord {
        HealthEvidenceRecord {
            record_id: record_id.to_string(),
            subject_kind: HealthEvidenceSubjectKind::ComponentVerdict,
            component_id: "motor-001".to_string(),
            method_version: "fleet-health-thresholds-v1".to_string(),
            reason_code: "degraded_threshold_exceeded".to_string(),
            recorded_at: "2026-06-22T12:31:00Z".to_string(),
            input_refs: vec![],
            decision_hash: decision_hash.to_string(),
        }
    }

    fn ota_rollout_request(
        current_stage: OtaRolloutStage,
        rollback_version: Option<OtaArtifactVersion>,
        nodes: Vec<OtaRolloutNode>,
        health_reports: Vec<OtaNodeHealthReport>,
    ) -> OtaRolloutRequest {
        OtaRolloutRequest {
            rollout_id: "rollout-2026-06-12".to_string(),
            evaluated_at: "2026-06-12T13:00:00Z".to_string(),
            current_stage,
            target_version: signed_version("agbot-edge", "2.0.0"),
            rollback_version,
            nodes,
            health_reports,
        }
    }

    fn signed_version(artifact: &str, version: &str) -> OtaArtifactVersion {
        OtaArtifactVersion {
            artifact: artifact.to_string(),
            version: version.to_string(),
            signed: true,
        }
    }

    fn ota_node(node_id: &str, stage: OtaRolloutStage) -> OtaRolloutNode {
        OtaRolloutNode {
            node_id: node_id.to_string(),
            stage,
            current_version: "2.0.0".to_string(),
            previous_version: "1.9.0".to_string(),
        }
    }

    fn ota_health(
        node_id: &str,
        status: ComponentHealthVerdictStatus,
        blocking_alerts: Vec<&str>,
    ) -> OtaNodeHealthReport {
        OtaNodeHealthReport {
            node_id: node_id.to_string(),
            status,
            blocking_alerts: blocking_alerts.into_iter().map(ToOwned::to_owned).collect(),
            checked_at: "2026-06-12T13:02:00Z".to_string(),
        }
    }

    fn rollout_control_request(
        action: RolloutControlAction,
        simulation_validated: bool,
        targets_flight_nodes: bool,
    ) -> RolloutControlRequest {
        RolloutControlRequest {
            rollout_id: "rollout-2026-06-12".to_string(),
            actor: "ops@example.com".to_string(),
            action,
            version: "2.0.0".to_string(),
            stage: OtaRolloutStage::Staged,
            requested_at: "2026-06-12T14:00:00Z".to_string(),
            simulation_validated,
            targets_flight_nodes,
        }
    }

    fn threshold(
        indicator: FleetHealthIndicator,
        watch_at: f64,
        degraded_at: f64,
        critical_at: f64,
    ) -> HealthIndicatorThreshold {
        HealthIndicatorThreshold {
            indicator,
            watch_at,
            degraded_at,
            critical_at,
        }
    }

    fn readiness_request(
        installed_components: Vec<FleetComponentRecord>,
        service_limits: Vec<ComponentServiceLimit>,
        health_verdicts: Vec<ComponentHealthVerdict>,
    ) -> FleetReadinessRequest {
        FleetReadinessRequest {
            airframe_id: "airframe-1".to_string(),
            checked_at: "2026-06-12T12:45:00Z".to_string(),
            installed_components,
            service_limits,
            health_verdicts,
        }
    }

    fn component_for_readiness(
        component_id: &str,
        component_type: FleetComponentType,
        flight_hours: f64,
        cycles: u32,
        duty_score: f64,
    ) -> FleetComponentRecord {
        FleetComponentRecord {
            component_id: component_id.to_string(),
            component_type,
            serial: format!("{component_id}-serial"),
            airframe_id: Some("airframe-1".to_string()),
            installed_at: Some("2026-06-01T10:00:00Z".to_string()),
            removed_at: None,
            service_history: vec![],
            flight_hours,
            cycles,
            duty_score,
            created_at: "2026-06-01T10:05:00Z".to_string(),
            updated_at: "2026-06-12T12:00:00Z".to_string(),
        }
    }

    fn service_limit(
        component_id: &str,
        max_flight_hours: Option<f64>,
        max_cycles: Option<u32>,
        max_duty_score: Option<f64>,
    ) -> ComponentServiceLimit {
        ComponentServiceLimit {
            component_id: component_id.to_string(),
            max_flight_hours,
            max_cycles,
            max_duty_score,
        }
    }

    fn component_verdict(
        component_id: &str,
        status: ComponentHealthVerdictStatus,
        freshness: HealthIndicatorFreshness,
    ) -> ComponentHealthVerdict {
        let (reason_code, value, threshold) = match status {
            ComponentHealthVerdictStatus::Ok => (
                HealthVerdictReasonCode::AllIndicatorsWithinThreshold,
                30.0,
                60.0,
            ),
            ComponentHealthVerdictStatus::Watch => {
                (HealthVerdictReasonCode::WatchThresholdExceeded, 0.7, 0.6)
            }
            ComponentHealthVerdictStatus::Degraded => {
                (HealthVerdictReasonCode::DegradedThresholdExceeded, 1.2, 1.0)
            }
            ComponentHealthVerdictStatus::Critical => {
                (HealthVerdictReasonCode::CriticalThresholdExceeded, 1.8, 1.5)
            }
        };

        ComponentHealthVerdict {
            component_id: component_id.to_string(),
            evaluated_at: "2026-06-12T12:30:00Z".to_string(),
            method_version: "fleet-health-thresholds-v1".to_string(),
            status,
            reason_code,
            indicator: Some(FleetHealthIndicator::MotorVibration),
            threshold: Some(threshold),
            value: Some(value),
            freshness,
            evidence: vec![HealthVerdictEvidence {
                indicator: FleetHealthIndicator::MotorVibration,
                value,
                threshold,
                status,
                reason_code,
                sample_ts: "2026-06-12T12:00:00Z".to_string(),
                source_ref: "telemetry:session-001".to_string(),
                freshness,
            }],
        }
    }
}
