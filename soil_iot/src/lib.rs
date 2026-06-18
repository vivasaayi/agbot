use alerting::{AlertCandidateRecord, AlertEvent, AlertSeverityHint, AlertingError, SourceAdapter};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use timeseries::{SeriesPoint, SeriesValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoilSensorType {
    ElectricalConductivity,
    MicroWeather,
    SoilMoisture,
    SoilTemperature,
}

impl SoilSensorType {
    pub fn as_str(self) -> &'static str {
        match self {
            SoilSensorType::ElectricalConductivity => "electrical_conductivity",
            SoilSensorType::MicroWeather => "micro_weather",
            SoilSensorType::SoilMoisture => "soil_moisture",
            SoilSensorType::SoilTemperature => "soil_temperature",
        }
    }
}

impl std::str::FromStr for SoilSensorType {
    type Err = SoilIotError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "electrical_conductivity" | "ec" => Ok(Self::ElectricalConductivity),
            "micro_weather" | "weather" => Ok(Self::MicroWeather),
            "soil_moisture" => Ok(Self::SoilMoisture),
            "soil_temperature" => Ok(Self::SoilTemperature),
            _ => Err(SoilIotError::UnsupportedSensorType {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoilDeviceStatus {
    Registered,
    Active,
    Maintenance,
    Retired,
}

impl SoilDeviceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SoilDeviceStatus::Registered => "registered",
            SoilDeviceStatus::Active => "active",
            SoilDeviceStatus::Maintenance => "maintenance",
            SoilDeviceStatus::Retired => "retired",
        }
    }
}

impl std::str::FromStr for SoilDeviceStatus {
    type Err = SoilIotError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "registered" => Ok(Self::Registered),
            "active" => Ok(Self::Active),
            "maintenance" => Ok(Self::Maintenance),
            "retired" => Ok(Self::Retired),
            _ => Err(SoilIotError::UnsupportedStatus {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoPosition {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default)]
    pub crs: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RegisterSoilDeviceRequest {
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub org_id: String,
    #[serde(default)]
    pub field_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub sensor_type: SoilSensorType,
    pub position: GeoPosition,
    #[serde(default)]
    pub calibration_profile_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilDeviceRecord {
    pub device_id: String,
    pub org_id: String,
    pub field_id: String,
    pub zone_id: Option<String>,
    pub sensor_type: SoilSensorType,
    pub position: GeoPosition,
    pub calibration_profile_ref: String,
    pub status: SoilDeviceStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoilDeviceConfigPushStatus {
    Pending,
    Applied,
    Failed,
}

impl SoilDeviceConfigPushStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SoilDeviceConfigPushStatus::Pending => "pending",
            SoilDeviceConfigPushStatus::Applied => "applied",
            SoilDeviceConfigPushStatus::Failed => "failed",
        }
    }
}

impl std::str::FromStr for SoilDeviceConfigPushStatus {
    type Err = SoilIotError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "applied" => Ok(Self::Applied),
            "failed" => Ok(Self::Failed),
            _ => Err(SoilIotError::UnsupportedConfigPushStatus {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SoilDeviceConfigPushRequest {
    #[serde(default)]
    pub push_id: Option<String>,
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub config_version: String,
    #[serde(default)]
    pub pushed_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SoilDeviceConfigPushStatusUpdate {
    pub push_status: SoilDeviceConfigPushStatus,
    #[serde(default)]
    pub failure_reason: Option<String>,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilDeviceConfigPushRecord {
    pub push_id: String,
    pub device_id: String,
    pub config_version: String,
    pub pushed_at: String,
    pub push_status: SoilDeviceConfigPushStatus,
    pub failure_reason: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ReadingDeviceCheckRequest {
    #[serde(default)]
    pub device_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingRejectionReason {
    RetiredDevice,
    UnknownDevice,
}

impl ReadingRejectionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            ReadingRejectionReason::RetiredDevice => "retired_device",
            ReadingRejectionReason::UnknownDevice => "unknown_device",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadingRejectionRecord {
    pub rejection_id: String,
    pub device_id: String,
    pub reason: ReadingRejectionReason,
    pub rejected_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayReadingMetric {
    BatteryVoltage,
    ElectricalConductivity,
    SoilMoisturePercent,
    SoilTemperatureCelsius,
}

impl GatewayReadingMetric {
    pub fn as_str(self) -> &'static str {
        match self {
            GatewayReadingMetric::BatteryVoltage => "battery_voltage",
            GatewayReadingMetric::ElectricalConductivity => "electrical_conductivity",
            GatewayReadingMetric::SoilMoisturePercent => "soil_moisture_percent",
            GatewayReadingMetric::SoilTemperatureCelsius => "soil_temperature_celsius",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            GatewayReadingMetric::BatteryVoltage => "volt",
            GatewayReadingMetric::ElectricalConductivity => "millisiemens_per_cm",
            GatewayReadingMetric::SoilMoisturePercent => "percent",
            GatewayReadingMetric::SoilTemperatureCelsius => "celsius",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawGatewayReading {
    pub payload_id: String,
    pub device_id: String,
    pub metric: GatewayReadingMetric,
    pub raw_value: f64,
    pub gateway_ts: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatewayReadingRecord {
    pub payload_id: String,
    pub device_id: String,
    pub metric: GatewayReadingMetric,
    pub raw_value: f64,
    pub gateway_ts: String,
    pub received_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingGeolocationStatus {
    Located,
    NoGeolocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeolocatedSoilReading {
    pub payload_id: String,
    pub device_id: String,
    pub metric: GatewayReadingMetric,
    pub raw_value: f64,
    pub ts: String,
    pub received_at: String,
    pub field_id: String,
    pub zone_id: Option<String>,
    pub position: Option<GeoPosition>,
    pub geolocation_status: ReadingGeolocationStatus,
    pub excluded_from_geospatial_products: bool,
    #[serde(default)]
    pub qa_flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationProfile {
    pub profile_ref: String,
    pub metric: GatewayReadingMetric,
    pub scale: f64,
    pub offset: f64,
    pub valid_min: f64,
    pub valid_max: f64,
    pub method_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingQaReason {
    OutOfRange,
    Stuck,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadingQaFlag {
    pub reason_code: ReadingQaReason,
    pub profile_ref: String,
    pub method_version: String,
    pub raw_value: f64,
    pub calibrated_value: f64,
    pub valid_min: f64,
    pub valid_max: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidatedSoilReading {
    pub payload_id: String,
    pub device_id: String,
    pub metric: GatewayReadingMetric,
    pub raw_value: f64,
    pub calibrated_value: f64,
    pub ts: String,
    pub received_at: String,
    pub field_id: String,
    pub zone_id: Option<String>,
    pub source_qa_flags: Vec<String>,
    pub qa_flags: Vec<ReadingQaFlag>,
    pub excluded_from_products: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ValidationEvidenceRequest {
    #[serde(default)]
    pub readings: Vec<GeolocatedSoilReading>,
    pub profile: CalibrationProfile,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationQaEvidenceRecord {
    pub payload_id: String,
    pub device_id: String,
    pub reason_code: ReadingQaReason,
    pub profile_ref: String,
    pub method_version: String,
    pub raw_value: f64,
    pub calibrated_value: f64,
    pub valid_min: f64,
    pub valid_max: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidatedSeriesEvidenceReport {
    pub profile_ref: String,
    pub method_version: String,
    pub output_hash: String,
    pub reading_count: usize,
    pub qa_flag_count: usize,
    #[serde(default)]
    pub validated_readings: Vec<ValidatedSoilReading>,
    #[serde(default)]
    pub qa_evidence: Vec<ValidationQaEvidenceRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoilDeviceFreshnessState {
    Fresh,
    Stale,
    NeverSeen,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoilCaptureFreshnessConfig {
    pub expected_interval_seconds: u64,
    pub method_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilDeviceFreshnessRecord {
    pub device_id: String,
    pub field_id: String,
    pub zone_id: Option<String>,
    pub last_seen: Option<String>,
    pub age_seconds: Option<u64>,
    pub expected_interval_seconds: u64,
    pub state: SoilDeviceFreshnessState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilZoneCoverageRecord {
    pub field_id: String,
    pub zone_id: String,
    pub device_count: usize,
    pub fresh_device_count: usize,
    pub stale_device_count: usize,
    pub never_seen_device_count: usize,
    pub coverage_fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCaptureFreshnessCoverageReport {
    pub field_id: String,
    pub evaluated_at: String,
    pub method_version: String,
    pub devices: Vec<SoilDeviceFreshnessRecord>,
    pub zones: Vec<SoilZoneCoverageRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilZoneGeometry {
    pub zone_id: String,
    pub crs: String,
    pub polygon: Vec<Vec<GeoJsonCoordinate>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoJsonCoordinate {
    pub lon: f64,
    pub lat: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCoverageGapZoneSummary {
    pub field_id: String,
    pub zone_id: String,
    pub device_count: usize,
    pub fresh_device_count: usize,
    pub stale_device_count: usize,
    pub never_seen_device_count: usize,
    pub coverage_fraction: f64,
    pub is_gap: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCoverageGapExport {
    pub field_id: String,
    pub evaluated_at: String,
    pub method_version: String,
    pub crs: String,
    pub zone_summaries: Vec<SoilCoverageGapZoneSummary>,
    pub gap_csv: String,
    pub gap_geojson: SoilCoverageGapFeatureCollection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCoverageGapFeatureCollection {
    #[serde(rename = "type")]
    pub collection_type: String,
    pub crs: SoilCoverageGapCrs,
    pub features: Vec<SoilCoverageGapFeature>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCoverageGapCrs {
    #[serde(rename = "type")]
    pub crs_type: String,
    pub properties: SoilCoverageGapCrsProperties,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCoverageGapCrsProperties {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCoverageGapFeature {
    #[serde(rename = "type")]
    pub feature_type: String,
    pub id: String,
    pub geometry: SoilCoverageGapGeometry,
    pub properties: SoilCoverageGapFeatureProperties,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCoverageGapGeometry {
    #[serde(rename = "type")]
    pub geometry_type: String,
    pub coordinates: Vec<Vec<[f64; 2]>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCoverageGapFeatureProperties {
    pub field_id: String,
    pub zone_id: String,
    pub device_count: usize,
    pub fresh_device_count: usize,
    pub stale_device_count: usize,
    pub never_seen_device_count: usize,
    pub coverage_fraction: f64,
    pub is_gap: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StuckSensorWindowConfig {
    pub min_samples: usize,
    pub variance_threshold: f64,
    pub range_threshold: f64,
    pub valid_min: f64,
    pub valid_max: f64,
    pub rail_tolerance: f64,
    pub method_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StuckSensorRail {
    Lower,
    Upper,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StuckSensorDetection {
    pub device_id: String,
    pub metric: GatewayReadingMetric,
    pub reason_code: ReadingQaReason,
    pub window: usize,
    pub sample_count: usize,
    pub window_start_ts: String,
    pub window_end_ts: String,
    pub observed_variance: f64,
    pub observed_range: f64,
    pub observed_min: f64,
    pub observed_max: f64,
    pub variance_threshold: f64,
    pub range_threshold: f64,
    pub pinned_at_rail: Option<StuckSensorRail>,
    pub rail_tolerance: f64,
    pub method_version: String,
    pub evidence_payload_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorHealthReasonCode {
    Disconnected,
    LowBattery,
    StaleReading,
}

impl SensorHealthReasonCode {
    pub fn as_str(self) -> &'static str {
        match self {
            SensorHealthReasonCode::Disconnected => "disconnected",
            SensorHealthReasonCode::LowBattery => "low_battery",
            SensorHealthReasonCode::StaleReading => "stale_reading",
        }
    }

    fn metric_name(self) -> &'static str {
        match self {
            SensorHealthReasonCode::Disconnected => "link_status",
            SensorHealthReasonCode::LowBattery => "battery_voltage",
            SensorHealthReasonCode::StaleReading => "freshness_age_seconds",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorHealthLinkStatus {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorHealthEventKind {
    Fired,
    Resolved,
}

impl SensorHealthEventKind {
    fn as_str(self) -> &'static str {
        match self {
            SensorHealthEventKind::Fired => "fired",
            SensorHealthEventKind::Resolved => "resolved",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensorHealthThresholds {
    pub low_battery_voltage: f64,
    pub stale_after_seconds: u64,
    pub method_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensorHealthSnapshot {
    pub device_id: String,
    pub field_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery_voltage: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_age_seconds: Option<u64>,
    pub link_status: SensorHealthLinkStatus,
    pub evidence_ref: String,
    pub evaluated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensorHealthEvidence {
    pub evidence_ref: String,
    pub metric: String,
    pub observed_value: f64,
    pub threshold_value: f64,
    pub rule_ref: String,
}

impl SensorHealthEvidence {
    pub fn ref_string(&self) -> String {
        format!(
            "{}:{}={}:threshold={}:rule={}",
            self.evidence_ref,
            self.metric,
            self.observed_value,
            self.threshold_value,
            self.rule_ref
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensorHealthEvent {
    pub device_id: String,
    pub field_id: String,
    pub reason_code: SensorHealthReasonCode,
    pub kind: SensorHealthEventKind,
    pub severity_hint: AlertSeverityHint,
    pub evidence: SensorHealthEvidence,
    pub occurred_at: String,
    pub idempotency_key: String,
}

impl SensorHealthEvent {
    pub fn to_alert_event(&self) -> AlertEvent {
        AlertEvent {
            source_domain: "soil_iot".to_string(),
            event_type: sensor_health_event_type(self.reason_code, self.kind),
            subject_ref: format!("field:{}:device:{}", self.field_id, self.device_id),
            severity_hint: self.severity_hint,
            evidence_refs: vec![self.evidence.ref_string()],
            occurred_at: self.occurred_at.clone(),
            idempotency_key: self.idempotency_key.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensorHealthMonitorState {
    active_conditions: BTreeSet<SensorHealthConditionKey>,
}

impl SensorHealthMonitorState {
    pub fn active_condition_count(&self) -> usize {
        self.active_conditions.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct SensorHealthConditionKey {
    device_id: String,
    reason_code: SensorHealthReasonCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrrigationTriggerSuppressionReason {
    InsufficientEvidence,
    QaFlagged,
    StaleData,
    UnsupportedMetric,
    WithinThreshold,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneSoilMoistureProduct {
    pub product_id: String,
    pub field_id: String,
    pub zone_id: String,
    pub metric: GatewayReadingMetric,
    pub value: f64,
    pub freshness_age_seconds: u64,
    #[serde(default)]
    pub qa_flags: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub generated_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ZoneSoilProductRequest {
    #[serde(default)]
    pub product_id: Option<String>,
    #[serde(default)]
    pub field_id: String,
    #[serde(default)]
    pub zone_id: String,
    pub metric: GatewayReadingMetric,
    #[serde(default)]
    pub readings: Vec<ValidatedSoilReading>,
    pub max_freshness_age_seconds: u64,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub method_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneSoilProductFreshness {
    Fresh,
    Stale,
    InsufficientEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneSoilProductSummary {
    pub product_id: String,
    pub field_id: String,
    pub zone_id: String,
    pub metric: GatewayReadingMetric,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_value: Option<f64>,
    pub sample_count: usize,
    pub freshness: ZoneSoilProductFreshness,
    pub freshness_age_seconds: u64,
    pub method_version: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub excluded_payload_ids: Vec<String>,
    pub generated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AerialIndexZoneProduct {
    pub product_id: String,
    pub field_id: String,
    pub zone_id: String,
    pub index_name: String,
    pub mean_value: f64,
    pub captured_at: String,
    pub crs: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SoilAerialFusionRequest {
    pub soil_product: ZoneSoilProductSummary,
    pub soil_crs: String,
    pub aerial_product: AerialIndexZoneProduct,
    pub max_temporal_gap_seconds: u64,
    pub max_agreement_delta: f64,
    #[serde(default)]
    pub method_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoilAerialFusionOutcome {
    Agreement,
    Divergence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoilAerialFusionRefusalReason {
    FieldMismatch,
    ZoneMismatch,
    CrsMismatch,
    TemporalMismatch,
    UnsupportedSoilMetric,
    UnsupportedAerialIndex,
    MissingSoilValue,
    MissingEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilAerialFusionSummary {
    pub field_id: String,
    pub zone_id: String,
    pub soil_product_id: String,
    pub aerial_product_id: String,
    pub soil_metric: GatewayReadingMetric,
    pub soil_value: f64,
    pub soil_crs: String,
    pub soil_generated_at: String,
    pub aerial_index: String,
    pub aerial_mean_value: f64,
    pub aerial_crs: String,
    pub aerial_captured_at: String,
    pub temporal_gap_seconds: u64,
    pub normalized_delta: f64,
    pub outcome: SoilAerialFusionOutcome,
    pub method_version: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilAerialFusionEvaluation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<SoilAerialFusionSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused_reason: Option<SoilAerialFusionRefusalReason>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationTriggerConfig {
    pub low_moisture_threshold: f64,
    pub max_freshness_age_seconds: u64,
    pub method_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationTriggerInput {
    pub contract_version: String,
    pub trigger_id: String,
    pub field_id: String,
    pub zone_id: String,
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
    pub trigger_ts: String,
    pub evidence_refs: Vec<String>,
    pub method_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrrigationTriggerEvaluation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<IrrigationTriggerInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppressed_reason: Option<IrrigationTriggerSuppressionReason>,
    pub evidence_refs: Vec<String>,
}

impl GeolocatedSoilReading {
    pub fn to_series_point(&self) -> SeriesPoint {
        SeriesPoint {
            entity_ref: format!("device:{}", self.device_id),
            metric: self.metric.as_str().to_string(),
            unit: self.metric.unit().to_string(),
            t: self.ts.clone(),
            value: SeriesValue::Scalar {
                value: self.raw_value,
            },
            source_ref: format!("soil-iot:{}", self.payload_id),
            created_at: self.received_at.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayPayloadRejectionReason {
    MalformedPayload,
    RetiredDevice,
    UnknownDevice,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatewayPayloadRejection {
    pub payload_id: Option<String>,
    pub device_id: Option<String>,
    pub reason: GatewayPayloadRejectionReason,
    pub details: String,
    pub rejected_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatewayIngestSummary {
    pub accepted_readings: Vec<GatewayReadingRecord>,
    pub rejections: Vec<GatewayPayloadRejection>,
    pub rejected_payload_count: u32,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GatewayIngestError {
    #[error("payload_id cannot be empty")]
    EmptyPayloadId,
    #[error("gateway reading device_id cannot be empty")]
    EmptyGatewayDeviceId,
    #[error("gateway timestamp cannot be empty")]
    EmptyGatewayTimestamp,
    #[error("received_at cannot be empty")]
    EmptyReceivedAt,
    #[error("gateway raw_value must be finite")]
    InvalidRawValue,
    #[error("malformed gateway payload {payload_id}: {reason}")]
    MalformedPayload { payload_id: String, reason: String },
}

pub trait GatewayAdapter {
    fn subscribe(&mut self) -> Result<(), GatewayIngestError>;
    fn next_reading(&mut self) -> Option<Result<RawGatewayReading, GatewayIngestError>>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulatedGateway {
    readings: VecDeque<Result<RawGatewayReading, GatewayIngestError>>,
    subscribed: bool,
}

impl SimulatedGateway {
    pub fn new(readings: Vec<Result<RawGatewayReading, GatewayIngestError>>) -> Self {
        Self {
            readings: VecDeque::from(readings),
            subscribed: false,
        }
    }
}

impl GatewayAdapter for SimulatedGateway {
    fn subscribe(&mut self) -> Result<(), GatewayIngestError> {
        self.subscribed = true;
        Ok(())
    }

    fn next_reading(&mut self) -> Option<Result<RawGatewayReading, GatewayIngestError>> {
        if self.subscribed {
            self.readings.pop_front()
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SoilIotError {
    #[error("device_id cannot be empty")]
    EmptyDeviceId,
    #[error("rejection_id cannot be empty")]
    EmptyRejectionId,
    #[error("org_id cannot be empty")]
    EmptyOrgId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("calibration_profile_ref cannot be empty")]
    EmptyCalibrationProfileRef,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("config push_id cannot be empty")]
    EmptyConfigPushId,
    #[error("config_version cannot be empty")]
    EmptyConfigVersion,
    #[error("config pushed_at cannot be empty")]
    EmptyConfigPushedAt,
    #[error("config push updated_at cannot be empty")]
    EmptyConfigPushUpdatedAt,
    #[error("config push failure_reason cannot be empty for failed pushes")]
    EmptyConfigPushFailureReason,
    #[error("config push failure_reason is only allowed for failed pushes")]
    UnexpectedConfigPushFailureReason,
    #[error("soil IoT position CRS cannot be empty")]
    EmptyCrs,
    #[error("unsupported soil IoT position CRS {value}; expected EPSG:4326")]
    UnsupportedCrs { value: String },
    #[error("soil IoT position latitude/longitude is out of bounds")]
    InvalidPosition,
    #[error("unsupported soil sensor type {value}")]
    UnsupportedSensorType { value: String },
    #[error("unsupported soil device status {value}")]
    UnsupportedStatus { value: String },
    #[error("unsupported config push status {value}")]
    UnsupportedConfigPushStatus { value: String },
    #[error("retired device {device_id} cannot leave retired status")]
    RetiredDeviceCannotTransition { device_id: String },
    #[error("config push {push_id} cannot transition from {from:?} to {to:?}")]
    InvalidConfigPushTransition {
        push_id: String,
        from: SoilDeviceConfigPushStatus,
        to: SoilDeviceConfigPushStatus,
    },
    #[error("calibration method_version cannot be empty")]
    EmptyCalibrationMethodVersion,
    #[error("soil reading raw value must be finite")]
    InvalidReadingValue,
    #[error("calibration profile {profile_ref} must be finite with valid_min <= valid_max")]
    InvalidCalibrationProfile { profile_ref: String },
    #[error("calibration profile metric {profile_metric:?} does not match reading metric {reading_metric:?}")]
    CalibrationMetricMismatch {
        reading_metric: GatewayReadingMetric,
        profile_metric: GatewayReadingMetric,
    },
    #[error("validation evidence series cannot be empty")]
    EmptyValidationEvidenceSeries,
    #[error("validation evidence hash input could not be serialized")]
    ValidationEvidenceSerializationFailed,
    #[error("stuck-sensor method_version cannot be empty")]
    EmptyStuckWindowMethodVersion,
    #[error("sensor-health method_version cannot be empty")]
    EmptySensorHealthMethodVersion,
    #[error("freshness coverage method_version cannot be empty")]
    EmptyFreshnessMethodVersion,
    #[error("sensor-health evaluated_at cannot be empty")]
    EmptySensorHealthEvaluatedAt,
    #[error("sensor-health evidence_ref cannot be empty")]
    EmptySensorHealthEvidenceRef,
    #[error("zone soil product_id cannot be empty")]
    EmptyZoneSoilProductId,
    #[error("zone_id cannot be empty")]
    EmptyZoneId,
    #[error("zone soil product generated_at cannot be empty")]
    EmptyZoneSoilGeneratedAt,
    #[error("zone soil product method_version cannot be empty")]
    EmptyZoneSoilProductMethodVersion,
    #[error("zone soil product readings cannot be empty")]
    EmptyZoneSoilProductReadings,
    #[error("zone soil product freshness window must be greater than zero")]
    InvalidZoneSoilProductFreshnessWindow,
    #[error("unsupported zone soil product metric {metric:?}")]
    UnsupportedZoneSoilProductMetric { metric: GatewayReadingMetric },
    #[error("zone soil product readings must belong to field {expected_field_id} and zone {expected_zone_id}")]
    MixedZoneSoilProductScope {
        expected_field_id: String,
        expected_zone_id: String,
    },
    #[error("aerial index product_id cannot be empty")]
    EmptyAerialIndexProductId,
    #[error("aerial index name cannot be empty")]
    EmptyAerialIndexName,
    #[error("aerial index captured_at cannot be empty")]
    EmptyAerialIndexCapturedAt,
    #[error("aerial index CRS cannot be empty")]
    EmptyAerialIndexCrs,
    #[error("soil-aerial fusion method_version cannot be empty")]
    EmptySoilAerialFusionMethodVersion,
    #[error("soil-aerial fusion config must have max_temporal_gap_seconds > 0 and finite max_agreement_delta >= 0")]
    InvalidSoilAerialFusionConfig,
    #[error("soil-aerial fusion evidence_refs cannot contain empty values")]
    EmptySoilAerialFusionEvidenceRef,
    #[error("irrigation trigger method_version cannot be empty")]
    EmptyIrrigationTriggerMethodVersion,
    #[error("irrigation trigger evidence_refs cannot contain empty values")]
    EmptyIrrigationEvidenceRef,
    #[error(
        "stuck-sensor window config must be finite with min_samples > 1 and valid_min <= valid_max"
    )]
    InvalidStuckWindowConfig,
    #[error("sensor-health thresholds must be finite with low_battery_voltage >= 0")]
    InvalidSensorHealthThresholds,
    #[error("freshness expected_interval_seconds must be greater than zero")]
    InvalidFreshnessInterval,
    #[error("freshness timestamp is invalid: {value}")]
    InvalidFreshnessTimestamp { value: String },
    #[error("freshness devices must belong to one field, saw {left} and {right}")]
    MixedFreshnessFieldScope { left: String, right: String },
    #[error("coverage gap export CRS cannot be empty")]
    EmptyCoverageGapCrs,
    #[error("coverage gap export is missing geometry for uncovered zone {zone_id}")]
    MissingCoverageGapZoneGeometry { zone_id: String },
    #[error("coverage gap export geometry for zone {zone_id} is invalid")]
    InvalidCoverageGapZoneGeometry { zone_id: String },
    #[error("irrigation trigger config must be finite with low_moisture_threshold >= 0")]
    InvalidIrrigationTriggerConfig,
    #[error("stuck-sensor window must contain one device and metric")]
    MixedStuckWindowSeries,
}

pub fn build_soil_device_record(
    request: RegisterSoilDeviceRequest,
    generated_device_id: String,
    created_at: String,
) -> Result<SoilDeviceRecord, SoilIotError> {
    let device_id = match normalize_optional_text(request.device_id) {
        Some(device_id) => device_id,
        None => normalize_required_text(generated_device_id, SoilIotError::EmptyDeviceId)?,
    };
    let position = normalize_position(request.position)?;
    let created_at = normalize_required_text(created_at, SoilIotError::EmptyCreatedAt)?;

    Ok(SoilDeviceRecord {
        device_id,
        org_id: normalize_required_text(request.org_id, SoilIotError::EmptyOrgId)?,
        field_id: normalize_required_text(request.field_id, SoilIotError::EmptyFieldId)?,
        zone_id: normalize_optional_text(request.zone_id),
        sensor_type: request.sensor_type,
        position,
        calibration_profile_ref: normalize_required_text(
            request.calibration_profile_ref,
            SoilIotError::EmptyCalibrationProfileRef,
        )?,
        status: SoilDeviceStatus::Active,
        created_at: created_at.clone(),
        updated_at: created_at,
    })
}

pub fn transition_soil_device_status(
    record: &SoilDeviceRecord,
    status: SoilDeviceStatus,
    updated_at: String,
) -> Result<SoilDeviceRecord, SoilIotError> {
    if record.status == SoilDeviceStatus::Retired && status != SoilDeviceStatus::Retired {
        return Err(SoilIotError::RetiredDeviceCannotTransition {
            device_id: record.device_id.clone(),
        });
    }

    let mut updated = record.clone();
    updated.status = status;
    updated.updated_at = normalize_required_text(updated_at, SoilIotError::EmptyCreatedAt)?;
    Ok(updated)
}

pub fn build_soil_config_push_record(
    request: SoilDeviceConfigPushRequest,
    generated_push_id: String,
) -> Result<SoilDeviceConfigPushRecord, SoilIotError> {
    let push_id = match normalize_optional_text(request.push_id) {
        Some(push_id) => push_id,
        None => normalize_required_text(generated_push_id, SoilIotError::EmptyConfigPushId)?,
    };
    let pushed_at = normalize_required_text(request.pushed_at, SoilIotError::EmptyConfigPushedAt)?;

    Ok(SoilDeviceConfigPushRecord {
        push_id,
        device_id: normalize_required_text(request.device_id, SoilIotError::EmptyDeviceId)?,
        config_version: normalize_required_text(
            request.config_version,
            SoilIotError::EmptyConfigVersion,
        )?,
        pushed_at: pushed_at.clone(),
        push_status: SoilDeviceConfigPushStatus::Pending,
        failure_reason: None,
        updated_at: pushed_at,
    })
}

pub fn transition_soil_config_push_status(
    record: &SoilDeviceConfigPushRecord,
    update: SoilDeviceConfigPushStatusUpdate,
) -> Result<SoilDeviceConfigPushRecord, SoilIotError> {
    let updated_at =
        normalize_required_text(update.updated_at, SoilIotError::EmptyConfigPushUpdatedAt)?;
    let failure_reason = normalize_optional_text(update.failure_reason);

    match update.push_status {
        SoilDeviceConfigPushStatus::Failed => {
            if failure_reason.is_none() {
                return Err(SoilIotError::EmptyConfigPushFailureReason);
            }
        }
        SoilDeviceConfigPushStatus::Pending | SoilDeviceConfigPushStatus::Applied => {
            if failure_reason.is_some() {
                return Err(SoilIotError::UnexpectedConfigPushFailureReason);
            }
        }
    }

    if record.push_status != SoilDeviceConfigPushStatus::Pending {
        return Err(SoilIotError::InvalidConfigPushTransition {
            push_id: record.push_id.clone(),
            from: record.push_status,
            to: update.push_status,
        });
    }

    if update.push_status == SoilDeviceConfigPushStatus::Pending {
        return Err(SoilIotError::InvalidConfigPushTransition {
            push_id: record.push_id.clone(),
            from: record.push_status,
            to: update.push_status,
        });
    }

    let mut updated = record.clone();
    updated.push_status = update.push_status;
    updated.failure_reason = failure_reason;
    updated.updated_at = updated_at;
    Ok(updated)
}

pub fn reading_rejection_for_device(
    device: Option<&SoilDeviceRecord>,
    device_id: String,
    generated_rejection_id: String,
    rejected_at: String,
) -> Result<Option<ReadingRejectionRecord>, SoilIotError> {
    let requested_device_id = normalize_required_text(device_id, SoilIotError::EmptyDeviceId)?;
    let reason = match device {
        None => Some(ReadingRejectionReason::UnknownDevice),
        Some(device) if device.status == SoilDeviceStatus::Retired => {
            Some(ReadingRejectionReason::RetiredDevice)
        }
        Some(_) => None,
    };

    reason
        .map(|reason| {
            Ok(ReadingRejectionRecord {
                rejection_id: normalize_required_text(
                    generated_rejection_id,
                    SoilIotError::EmptyRejectionId,
                )?,
                device_id: requested_device_id,
                reason,
                rejected_at: normalize_required_text(rejected_at, SoilIotError::EmptyCreatedAt)?,
            })
        })
        .transpose()
}

pub fn decode_gateway_payload(
    payload_id: String,
    payload_json: &str,
) -> Result<RawGatewayReading, GatewayIngestError> {
    let payload_id = normalize_gateway_text(payload_id, GatewayIngestError::EmptyPayloadId)?;
    let payload = serde_json::from_str::<GatewayPayloadBody>(payload_json).map_err(|error| {
        GatewayIngestError::MalformedPayload {
            payload_id: payload_id.clone(),
            reason: error.to_string(),
        }
    })?;

    validate_raw_gateway_reading(RawGatewayReading {
        payload_id,
        device_id: payload.device_id,
        metric: payload.metric,
        raw_value: payload.raw_value,
        gateway_ts: payload.gateway_ts,
    })
}

pub fn ingest_gateway_readings(
    gateway: &mut impl GatewayAdapter,
    registered_devices: &[SoilDeviceRecord],
    received_at: String,
) -> Result<GatewayIngestSummary, GatewayIngestError> {
    let received_at = normalize_gateway_text(received_at, GatewayIngestError::EmptyReceivedAt)?;
    gateway.subscribe()?;
    let devices = registered_devices
        .iter()
        .map(|device| (device.device_id.clone(), device))
        .collect::<BTreeMap<_, _>>();
    let mut accepted_readings = Vec::new();
    let mut rejections = Vec::new();

    while let Some(next) = gateway.next_reading() {
        match next {
            Ok(reading) => {
                let reading = match validate_raw_gateway_reading(reading) {
                    Ok(reading) => reading,
                    Err(error) => {
                        rejections.push(rejection_from_gateway_error(error, &received_at));
                        continue;
                    }
                };
                match devices.get(&reading.device_id).copied() {
                    None => rejections.push(GatewayPayloadRejection {
                        payload_id: Some(reading.payload_id),
                        device_id: Some(reading.device_id),
                        reason: GatewayPayloadRejectionReason::UnknownDevice,
                        details: "reading device is not registered".to_string(),
                        rejected_at: received_at.clone(),
                    }),
                    Some(device) if device.status == SoilDeviceStatus::Retired => {
                        rejections.push(GatewayPayloadRejection {
                            payload_id: Some(reading.payload_id),
                            device_id: Some(reading.device_id),
                            reason: GatewayPayloadRejectionReason::RetiredDevice,
                            details: "reading device is retired".to_string(),
                            rejected_at: received_at.clone(),
                        })
                    }
                    Some(_) => accepted_readings.push(GatewayReadingRecord {
                        payload_id: reading.payload_id,
                        device_id: reading.device_id,
                        metric: reading.metric,
                        raw_value: reading.raw_value,
                        gateway_ts: reading.gateway_ts,
                        received_at: received_at.clone(),
                    }),
                }
            }
            Err(error) => rejections.push(rejection_from_gateway_error(error, &received_at)),
        }
    }

    let rejected_payload_count = rejections.len() as u32;
    Ok(GatewayIngestSummary {
        accepted_readings,
        rejections,
        rejected_payload_count,
    })
}

pub fn build_geolocated_soil_reading(
    device: &SoilDeviceRecord,
    reading: GatewayReadingRecord,
) -> Result<GeolocatedSoilReading, GatewayIngestError> {
    let payload_id =
        normalize_gateway_text(reading.payload_id, GatewayIngestError::EmptyPayloadId)?;
    let device_id =
        normalize_gateway_text(reading.device_id, GatewayIngestError::EmptyGatewayDeviceId)?;
    let ts = normalize_gateway_text(
        reading.gateway_ts,
        GatewayIngestError::EmptyGatewayTimestamp,
    )?;
    let received_at =
        normalize_gateway_text(reading.received_at, GatewayIngestError::EmptyReceivedAt)?;
    if !reading.raw_value.is_finite() {
        return Err(GatewayIngestError::InvalidRawValue);
    }

    let position = normalize_position(device.position.clone()).ok();
    let geolocation_status = if position.is_some() {
        ReadingGeolocationStatus::Located
    } else {
        ReadingGeolocationStatus::NoGeolocation
    };
    let excluded_from_geospatial_products =
        geolocation_status == ReadingGeolocationStatus::NoGeolocation;
    let qa_flags = if excluded_from_geospatial_products {
        vec!["no_geolocation".to_string()]
    } else {
        Vec::new()
    };

    Ok(GeolocatedSoilReading {
        payload_id,
        device_id,
        metric: reading.metric,
        raw_value: reading.raw_value,
        ts,
        received_at,
        field_id: device.field_id.clone(),
        zone_id: device.zone_id.clone(),
        position,
        geolocation_status,
        excluded_from_geospatial_products,
        qa_flags,
    })
}

pub fn validate_and_calibrate_reading(
    reading: GeolocatedSoilReading,
    profile: CalibrationProfile,
) -> Result<ValidatedSoilReading, SoilIotError> {
    let profile = normalize_calibration_profile(profile)?;
    if profile.metric != reading.metric {
        return Err(SoilIotError::CalibrationMetricMismatch {
            reading_metric: reading.metric,
            profile_metric: profile.metric,
        });
    }
    if !reading.raw_value.is_finite() {
        return Err(SoilIotError::InvalidReadingValue);
    }

    let calibrated_value = profile.scale.mul_add(reading.raw_value, profile.offset);
    if !calibrated_value.is_finite() {
        return Err(SoilIotError::InvalidCalibrationProfile {
            profile_ref: profile.profile_ref,
        });
    }

    let mut qa_flags = Vec::new();
    if calibrated_value < profile.valid_min || calibrated_value > profile.valid_max {
        qa_flags.push(ReadingQaFlag {
            reason_code: ReadingQaReason::OutOfRange,
            profile_ref: profile.profile_ref.clone(),
            method_version: profile.method_version.clone(),
            raw_value: reading.raw_value,
            calibrated_value,
            valid_min: profile.valid_min,
            valid_max: profile.valid_max,
        });
    }
    let excluded_from_products = reading.excluded_from_geospatial_products || !qa_flags.is_empty();

    Ok(ValidatedSoilReading {
        payload_id: reading.payload_id,
        device_id: reading.device_id,
        metric: reading.metric,
        raw_value: reading.raw_value,
        calibrated_value,
        ts: reading.ts,
        received_at: reading.received_at,
        field_id: reading.field_id,
        zone_id: reading.zone_id,
        source_qa_flags: reading.qa_flags,
        qa_flags,
        excluded_from_products,
    })
}

pub fn rederive_validated_series_evidence(
    request: ValidationEvidenceRequest,
) -> Result<ValidatedSeriesEvidenceReport, SoilIotError> {
    if request.readings.is_empty() {
        return Err(SoilIotError::EmptyValidationEvidenceSeries);
    }
    let profile = normalize_calibration_profile(request.profile)?;
    let mut validated_readings = Vec::with_capacity(request.readings.len());
    let mut qa_evidence = Vec::new();

    for reading in request.readings {
        let validated = validate_and_calibrate_reading(reading, profile.clone())?;
        for flag in &validated.qa_flags {
            qa_evidence.push(ValidationQaEvidenceRecord {
                payload_id: validated.payload_id.clone(),
                device_id: validated.device_id.clone(),
                reason_code: flag.reason_code,
                profile_ref: flag.profile_ref.clone(),
                method_version: flag.method_version.clone(),
                raw_value: flag.raw_value,
                calibrated_value: flag.calibrated_value,
                valid_min: flag.valid_min,
                valid_max: flag.valid_max,
            });
        }
        validated_readings.push(validated);
    }

    let output_hash = validation_evidence_hash(&validated_readings, &qa_evidence)?;
    Ok(ValidatedSeriesEvidenceReport {
        profile_ref: profile.profile_ref,
        method_version: profile.method_version,
        output_hash,
        reading_count: validated_readings.len(),
        qa_flag_count: qa_evidence.len(),
        validated_readings,
        qa_evidence,
    })
}

pub fn evaluate_soil_capture_freshness_coverage(
    devices: &[SoilDeviceRecord],
    readings: &[ValidatedSoilReading],
    config: SoilCaptureFreshnessConfig,
    evaluated_at: String,
) -> Result<SoilCaptureFreshnessCoverageReport, SoilIotError> {
    let config = normalize_freshness_config(config)?;
    let evaluated_at_dt = parse_freshness_ts(&evaluated_at)?;
    let evaluated_at = normalize_required_text(evaluated_at, SoilIotError::EmptyCreatedAt)?;
    let mut field_id: Option<String> = None;
    let mut latest_by_device: BTreeMap<String, &ValidatedSoilReading> = BTreeMap::new();

    for reading in readings {
        let reading_ts = parse_freshness_ts(&reading.ts)?;
        latest_by_device
            .entry(reading.device_id.clone())
            .and_modify(|current| {
                if parse_freshness_ts(&current.ts)
                    .map(|current_ts| reading_ts > current_ts)
                    .unwrap_or(false)
                {
                    *current = reading;
                }
            })
            .or_insert(reading);
    }

    let mut device_records = Vec::new();
    let mut zone_accumulators: BTreeMap<String, ZoneCoverageAccumulator> = BTreeMap::new();
    for device in devices
        .iter()
        .filter(|device| device.status != SoilDeviceStatus::Retired)
    {
        let device_field_id =
            normalize_required_text(device.field_id.clone(), SoilIotError::EmptyFieldId)?;
        match field_id.as_ref() {
            Some(existing) if existing != &device_field_id => {
                return Err(SoilIotError::MixedFreshnessFieldScope {
                    left: existing.clone(),
                    right: device_field_id,
                });
            }
            None => field_id = Some(device_field_id.clone()),
            _ => {}
        }

        let latest = latest_by_device.get(&device.device_id).copied();
        let (last_seen, age_seconds, state) = match latest {
            Some(reading) => {
                let last_seen_dt = parse_freshness_ts(&reading.ts)?;
                let age_seconds = evaluated_at_dt
                    .signed_duration_since(last_seen_dt)
                    .num_seconds()
                    .max(0) as u64;
                let state = if age_seconds <= config.expected_interval_seconds {
                    SoilDeviceFreshnessState::Fresh
                } else {
                    SoilDeviceFreshnessState::Stale
                };
                (Some(reading.ts.clone()), Some(age_seconds), state)
            }
            None => (None, None, SoilDeviceFreshnessState::NeverSeen),
        };
        let zone_id = device
            .zone_id
            .clone()
            .unwrap_or_else(|| "unassigned".to_string());
        let entry =
            zone_accumulators
                .entry(zone_id.clone())
                .or_insert_with(|| ZoneCoverageAccumulator {
                    field_id: device_field_id.clone(),
                    zone_id,
                    ..ZoneCoverageAccumulator::default()
                });
        entry.device_count += 1;
        match state {
            SoilDeviceFreshnessState::Fresh => entry.fresh_device_count += 1,
            SoilDeviceFreshnessState::Stale => entry.stale_device_count += 1,
            SoilDeviceFreshnessState::NeverSeen => entry.never_seen_device_count += 1,
        }
        device_records.push(SoilDeviceFreshnessRecord {
            device_id: device.device_id.clone(),
            field_id: device_field_id,
            zone_id: device.zone_id.clone(),
            last_seen,
            age_seconds,
            expected_interval_seconds: config.expected_interval_seconds,
            state,
        });
    }

    device_records.sort_by(|left, right| left.device_id.cmp(&right.device_id));
    let zones = zone_accumulators
        .into_values()
        .map(|zone| SoilZoneCoverageRecord {
            field_id: zone.field_id,
            zone_id: zone.zone_id,
            device_count: zone.device_count,
            fresh_device_count: zone.fresh_device_count,
            stale_device_count: zone.stale_device_count,
            never_seen_device_count: zone.never_seen_device_count,
            coverage_fraction: if zone.device_count > 0 {
                zone.fresh_device_count as f64 / zone.device_count as f64
            } else {
                0.0
            },
        })
        .collect();

    Ok(SoilCaptureFreshnessCoverageReport {
        field_id: field_id.unwrap_or_default(),
        evaluated_at,
        method_version: config.method_version,
        devices: device_records,
        zones,
    })
}

pub fn export_soil_coverage_gaps(
    report: SoilCaptureFreshnessCoverageReport,
    zone_geometries: Vec<SoilZoneGeometry>,
    crs: String,
) -> Result<SoilCoverageGapExport, SoilIotError> {
    let crs = normalize_required_text(crs, SoilIotError::EmptyCoverageGapCrs)?;
    let geometry_by_zone = zone_geometries
        .into_iter()
        .filter_map(|geometry| {
            normalize_optional_text(Some(geometry.zone_id.clone())).map(|zone_id| {
                (
                    zone_id,
                    SoilZoneGeometry {
                        zone_id: geometry.zone_id,
                        crs: geometry.crs,
                        polygon: geometry.polygon,
                    },
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut zone_summaries = report
        .zones
        .iter()
        .map(|zone| SoilCoverageGapZoneSummary {
            field_id: zone.field_id.clone(),
            zone_id: zone.zone_id.clone(),
            device_count: zone.device_count,
            fresh_device_count: zone.fresh_device_count,
            stale_device_count: zone.stale_device_count,
            never_seen_device_count: zone.never_seen_device_count,
            coverage_fraction: zone.coverage_fraction,
            is_gap: zone.fresh_device_count == 0,
        })
        .collect::<Vec<_>>();
    zone_summaries.sort_by(|left, right| left.zone_id.cmp(&right.zone_id));

    let mut gap_features = Vec::new();
    for summary in zone_summaries.iter().filter(|summary| summary.is_gap) {
        let geometry = geometry_by_zone.get(&summary.zone_id).ok_or_else(|| {
            SoilIotError::MissingCoverageGapZoneGeometry {
                zone_id: summary.zone_id.clone(),
            }
        })?;
        validate_gap_geometry(geometry, &crs)?;
        gap_features.push(SoilCoverageGapFeature {
            feature_type: "Feature".to_string(),
            id: summary.zone_id.clone(),
            geometry: SoilCoverageGapGeometry {
                geometry_type: "Polygon".to_string(),
                coordinates: geometry
                    .polygon
                    .iter()
                    .map(|ring| ring.iter().map(|coord| [coord.lon, coord.lat]).collect())
                    .collect(),
            },
            properties: SoilCoverageGapFeatureProperties {
                field_id: summary.field_id.clone(),
                zone_id: summary.zone_id.clone(),
                device_count: summary.device_count,
                fresh_device_count: summary.fresh_device_count,
                stale_device_count: summary.stale_device_count,
                never_seen_device_count: summary.never_seen_device_count,
                coverage_fraction: summary.coverage_fraction,
                is_gap: true,
            },
        });
    }

    let gap_csv = render_gap_csv(&zone_summaries);
    Ok(SoilCoverageGapExport {
        field_id: report.field_id,
        evaluated_at: report.evaluated_at,
        method_version: report.method_version,
        crs: crs.clone(),
        zone_summaries,
        gap_csv,
        gap_geojson: SoilCoverageGapFeatureCollection {
            collection_type: "FeatureCollection".to_string(),
            crs: SoilCoverageGapCrs {
                crs_type: "name".to_string(),
                properties: SoilCoverageGapCrsProperties { name: crs },
            },
            features: gap_features,
        },
    })
}

pub fn detect_stuck_sensor_window(
    readings: &[ValidatedSoilReading],
    config: StuckSensorWindowConfig,
) -> Result<Option<StuckSensorDetection>, SoilIotError> {
    let config = normalize_stuck_window_config(config)?;
    if readings.len() < config.min_samples {
        return Ok(None);
    }

    let first = &readings[0];
    let mut values = Vec::with_capacity(readings.len());
    let mut evidence_payload_ids = Vec::with_capacity(readings.len());
    for reading in readings {
        if reading.device_id != first.device_id || reading.metric != first.metric {
            return Err(SoilIotError::MixedStuckWindowSeries);
        }
        if !reading.calibrated_value.is_finite() {
            return Err(SoilIotError::InvalidReadingValue);
        }
        if reading.excluded_from_products || !reading.qa_flags.is_empty() {
            return Ok(None);
        }
        values.push(reading.calibrated_value);
        evidence_payload_ids.push(reading.payload_id.clone());
    }

    let window = values.len();
    let mean = values.iter().sum::<f64>() / window as f64;
    let observed_variance = values
        .iter()
        .map(|value| {
            let delta = value - mean;
            delta * delta
        })
        .sum::<f64>()
        / window as f64;
    let (observed_min, observed_max) = values.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(observed_min, observed_max), value| (observed_min.min(*value), observed_max.max(*value)),
    );
    let observed_range = observed_max - observed_min;
    let pinned_at_rail = pinned_rail(&values, &config);
    let flatlined =
        observed_variance <= config.variance_threshold && observed_range <= config.range_threshold;
    let rail_pinned = pinned_at_rail.is_some() && observed_range <= config.range_threshold;
    let stuck = flatlined || rail_pinned;

    if stuck {
        Ok(Some(StuckSensorDetection {
            device_id: first.device_id.clone(),
            metric: first.metric,
            reason_code: ReadingQaReason::Stuck,
            window,
            sample_count: window,
            window_start_ts: first.ts.clone(),
            window_end_ts: readings[window - 1].ts.clone(),
            observed_variance,
            observed_range,
            observed_min,
            observed_max,
            variance_threshold: config.variance_threshold,
            range_threshold: config.range_threshold,
            pinned_at_rail,
            rail_tolerance: config.rail_tolerance,
            method_version: config.method_version,
            evidence_payload_ids,
        }))
    } else {
        Ok(None)
    }
}

pub fn evaluate_sensor_health_snapshot(
    snapshot: SensorHealthSnapshot,
    thresholds: SensorHealthThresholds,
    state: &mut SensorHealthMonitorState,
) -> Result<Vec<SensorHealthEvent>, SoilIotError> {
    let snapshot = normalize_sensor_health_snapshot(snapshot)?;
    let thresholds = normalize_sensor_health_thresholds(thresholds)?;
    let observations = sensor_health_observations(&snapshot, &thresholds)?;
    let mut events = Vec::new();

    for (reason_code, observation) in observations {
        let key = SensorHealthConditionKey {
            device_id: snapshot.device_id.clone(),
            reason_code,
        };
        let active = state.active_conditions.contains(&key);

        match (observation.breached, active) {
            (true, false) => {
                state.active_conditions.insert(key);
                events.push(sensor_health_event(
                    &snapshot,
                    &thresholds,
                    reason_code,
                    SensorHealthEventKind::Fired,
                    observation,
                ));
            }
            (true, true) => {}
            (false, true) => {
                state.active_conditions.remove(&key);
                events.push(sensor_health_event(
                    &snapshot,
                    &thresholds,
                    reason_code,
                    SensorHealthEventKind::Resolved,
                    observation,
                ));
            }
            (false, false) => {}
        }
    }

    Ok(events)
}

pub fn emit_sensor_health_alert_events(
    adapter: &mut impl SourceAdapter,
    events: &[SensorHealthEvent],
) -> Result<Vec<AlertCandidateRecord>, AlertingError> {
    events
        .iter()
        .map(|event| adapter.emit(event.to_alert_event()))
        .collect()
}

pub fn build_zone_soil_product_summary(
    request: ZoneSoilProductRequest,
    generated_product_id: String,
) -> Result<ZoneSoilProductSummary, SoilIotError> {
    let product_id = match request.product_id {
        Some(product_id) => {
            normalize_required_text(product_id, SoilIotError::EmptyZoneSoilProductId)?
        }
        None => {
            normalize_required_text(generated_product_id, SoilIotError::EmptyZoneSoilProductId)?
        }
    };
    let field_id = normalize_required_text(request.field_id, SoilIotError::EmptyFieldId)?;
    let zone_id = normalize_required_text(request.zone_id, SoilIotError::EmptyZoneId)?;
    let generated_at =
        normalize_required_text(request.generated_at, SoilIotError::EmptyZoneSoilGeneratedAt)?;
    let method_version = normalize_required_text(
        request.method_version,
        SoilIotError::EmptyZoneSoilProductMethodVersion,
    )?;
    if request.max_freshness_age_seconds == 0 {
        return Err(SoilIotError::InvalidZoneSoilProductFreshnessWindow);
    }
    if request.readings.is_empty() {
        return Err(SoilIotError::EmptyZoneSoilProductReadings);
    }
    if !matches!(
        request.metric,
        GatewayReadingMetric::ElectricalConductivity
            | GatewayReadingMetric::SoilMoisturePercent
            | GatewayReadingMetric::SoilTemperatureCelsius
    ) {
        return Err(SoilIotError::UnsupportedZoneSoilProductMetric {
            metric: request.metric,
        });
    }

    let generated_at_dt = parse_freshness_ts(&generated_at)?;
    let mut values = Vec::new();
    let mut evidence_refs = BTreeSet::new();
    let mut excluded_payload_ids = BTreeSet::new();
    let mut included_max_age = 0;
    let mut stale_min_age: Option<u64> = None;
    let mut saw_in_scope_candidate = false;

    for reading in &request.readings {
        if reading.field_id != field_id || reading.zone_id.as_deref() != Some(zone_id.as_str()) {
            return Err(SoilIotError::MixedZoneSoilProductScope {
                expected_field_id: field_id,
                expected_zone_id: zone_id,
            });
        }
        if reading.metric != request.metric {
            excluded_payload_ids.insert(reading.payload_id.clone());
            continue;
        }
        saw_in_scope_candidate = true;

        let reading_ts = parse_freshness_ts(&reading.ts)?;
        let age_seconds = generated_at_dt
            .signed_duration_since(reading_ts)
            .num_seconds()
            .max(0) as u64;
        if reading.excluded_from_products || !reading.qa_flags.is_empty() {
            excluded_payload_ids.insert(reading.payload_id.clone());
            continue;
        }
        if age_seconds > request.max_freshness_age_seconds {
            stale_min_age = Some(stale_min_age.map_or(age_seconds, |age| age.min(age_seconds)));
            excluded_payload_ids.insert(reading.payload_id.clone());
            continue;
        }
        if !reading.calibrated_value.is_finite() {
            excluded_payload_ids.insert(reading.payload_id.clone());
            continue;
        }

        values.push(reading.calibrated_value);
        evidence_refs.insert(format!("soil-iot:{}", reading.payload_id));
        included_max_age = included_max_age.max(age_seconds);
    }

    let (mean_value, min_value, max_value, freshness, freshness_age_seconds) = if values.is_empty()
    {
        let freshness = if stale_min_age.is_some() {
            ZoneSoilProductFreshness::Stale
        } else {
            ZoneSoilProductFreshness::InsufficientEvidence
        };
        (None, None, None, freshness, stale_min_age.unwrap_or(0))
    } else {
        let sum = values.iter().sum::<f64>();
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (
            Some(sum / values.len() as f64),
            Some(min),
            Some(max),
            ZoneSoilProductFreshness::Fresh,
            included_max_age,
        )
    };

    if !saw_in_scope_candidate {
        return Ok(ZoneSoilProductSummary {
            product_id,
            field_id,
            zone_id,
            metric: request.metric,
            mean_value: None,
            min_value: None,
            max_value: None,
            sample_count: 0,
            freshness: ZoneSoilProductFreshness::InsufficientEvidence,
            freshness_age_seconds: 0,
            method_version,
            evidence_refs: Vec::new(),
            excluded_payload_ids: excluded_payload_ids.into_iter().collect(),
            generated_at,
        });
    }

    Ok(ZoneSoilProductSummary {
        product_id,
        field_id,
        zone_id,
        metric: request.metric,
        mean_value,
        min_value,
        max_value,
        sample_count: values.len(),
        freshness,
        freshness_age_seconds,
        method_version,
        evidence_refs: evidence_refs.into_iter().collect(),
        excluded_payload_ids: excluded_payload_ids.into_iter().collect(),
        generated_at,
    })
}

pub fn evaluate_soil_aerial_fusion(
    request: SoilAerialFusionRequest,
) -> Result<SoilAerialFusionEvaluation, SoilIotError> {
    let method_version = normalize_required_text(
        request.method_version,
        SoilIotError::EmptySoilAerialFusionMethodVersion,
    )?;
    if request.max_temporal_gap_seconds == 0
        || !request.max_agreement_delta.is_finite()
        || request.max_agreement_delta < 0.0
    {
        return Err(SoilIotError::InvalidSoilAerialFusionConfig);
    }

    let soil = request.soil_product;
    let soil_product_id =
        normalize_required_text(soil.product_id, SoilIotError::EmptyZoneSoilProductId)?;
    let soil_field_id = normalize_required_text(soil.field_id, SoilIotError::EmptyFieldId)?;
    let soil_zone_id = normalize_required_text(soil.zone_id, SoilIotError::EmptyZoneId)?;
    let soil_generated_at =
        normalize_required_text(soil.generated_at, SoilIotError::EmptyZoneSoilGeneratedAt)?;
    let soil_crs = normalize_required_text(request.soil_crs, SoilIotError::EmptyCrs)?;
    let aerial = normalize_aerial_index_zone_product(request.aerial_product)?;
    let evidence_refs = soil_aerial_evidence_refs(
        &soil_product_id,
        &soil.evidence_refs,
        &aerial.product_id,
        &aerial.evidence_refs,
    )?;

    let soil_dt = parse_freshness_ts(&soil_generated_at)?;
    let aerial_dt = parse_freshness_ts(&aerial.captured_at)?;
    let temporal_gap_seconds = soil_dt
        .signed_duration_since(aerial_dt)
        .num_seconds()
        .unsigned_abs();

    let refused_reason = if soil_field_id != aerial.field_id {
        Some(SoilAerialFusionRefusalReason::FieldMismatch)
    } else if soil_zone_id != aerial.zone_id {
        Some(SoilAerialFusionRefusalReason::ZoneMismatch)
    } else if soil_crs != aerial.crs {
        Some(SoilAerialFusionRefusalReason::CrsMismatch)
    } else if temporal_gap_seconds > request.max_temporal_gap_seconds {
        Some(SoilAerialFusionRefusalReason::TemporalMismatch)
    } else if soil.metric != GatewayReadingMetric::SoilMoisturePercent {
        Some(SoilAerialFusionRefusalReason::UnsupportedSoilMetric)
    } else if !aerial.index_name.eq_ignore_ascii_case("ndvi") {
        Some(SoilAerialFusionRefusalReason::UnsupportedAerialIndex)
    } else if soil.mean_value.is_none() {
        Some(SoilAerialFusionRefusalReason::MissingSoilValue)
    } else if evidence_refs.is_empty() {
        Some(SoilAerialFusionRefusalReason::MissingEvidence)
    } else {
        None
    };

    if let Some(reason) = refused_reason {
        return Ok(SoilAerialFusionEvaluation {
            summary: None,
            refused_reason: Some(reason),
            evidence_refs,
        });
    }

    let soil_value = soil.mean_value.expect("checked above");
    let normalized_soil_moisture = (soil_value / 100.0).clamp(0.0, 1.0);
    let normalized_delta = normalized_soil_moisture - aerial.mean_value;
    let outcome = if normalized_delta.abs() <= request.max_agreement_delta {
        SoilAerialFusionOutcome::Agreement
    } else {
        SoilAerialFusionOutcome::Divergence
    };

    Ok(SoilAerialFusionEvaluation {
        summary: Some(SoilAerialFusionSummary {
            field_id: soil_field_id,
            zone_id: soil_zone_id,
            soil_product_id,
            aerial_product_id: aerial.product_id,
            soil_metric: soil.metric,
            soil_value,
            soil_crs,
            soil_generated_at,
            aerial_index: aerial.index_name,
            aerial_mean_value: aerial.mean_value,
            aerial_crs: aerial.crs,
            aerial_captured_at: aerial.captured_at,
            temporal_gap_seconds,
            normalized_delta,
            outcome,
            method_version,
            evidence_refs: evidence_refs.clone(),
        }),
        refused_reason: None,
        evidence_refs,
    })
}

pub fn evaluate_irrigation_trigger(
    product: ZoneSoilMoistureProduct,
    config: IrrigationTriggerConfig,
) -> Result<IrrigationTriggerEvaluation, SoilIotError> {
    let product = normalize_zone_soil_moisture_product(product)?;
    let config = normalize_irrigation_trigger_config(config)?;
    let evidence_refs = product.evidence_refs.clone();

    let suppressed_reason = if product.metric != GatewayReadingMetric::SoilMoisturePercent {
        Some(IrrigationTriggerSuppressionReason::UnsupportedMetric)
    } else if evidence_refs.is_empty() {
        Some(IrrigationTriggerSuppressionReason::InsufficientEvidence)
    } else if !product.qa_flags.is_empty() {
        Some(IrrigationTriggerSuppressionReason::QaFlagged)
    } else if product.freshness_age_seconds > config.max_freshness_age_seconds {
        Some(IrrigationTriggerSuppressionReason::StaleData)
    } else if product.value >= config.low_moisture_threshold {
        Some(IrrigationTriggerSuppressionReason::WithinThreshold)
    } else {
        None
    };

    if let Some(reason) = suppressed_reason {
        return Ok(IrrigationTriggerEvaluation {
            trigger: None,
            suppressed_reason: Some(reason),
            evidence_refs,
        });
    }

    Ok(IrrigationTriggerEvaluation {
        trigger: Some(IrrigationTriggerInput {
            contract_version: "water_management.irrigation_trigger.v1".to_string(),
            trigger_id: format!(
                "irrigation-trigger:{}:{}:{}:{}",
                product.field_id, product.zone_id, product.product_id, config.method_version
            ),
            field_id: product.field_id,
            zone_id: product.zone_id,
            metric: product.metric.as_str().to_string(),
            value: product.value,
            threshold: config.low_moisture_threshold,
            trigger_ts: product.generated_at,
            evidence_refs: evidence_refs.clone(),
            method_version: config.method_version,
        }),
        suppressed_reason: None,
        evidence_refs,
    })
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct GatewayPayloadBody {
    device_id: String,
    metric: GatewayReadingMetric,
    raw_value: f64,
    gateway_ts: String,
}

fn validate_raw_gateway_reading(
    reading: RawGatewayReading,
) -> Result<RawGatewayReading, GatewayIngestError> {
    let payload_id =
        normalize_gateway_text(reading.payload_id, GatewayIngestError::EmptyPayloadId)?;
    let device_id =
        normalize_gateway_text(reading.device_id, GatewayIngestError::EmptyGatewayDeviceId)?;
    let gateway_ts = normalize_gateway_text(
        reading.gateway_ts,
        GatewayIngestError::EmptyGatewayTimestamp,
    )?;
    if !reading.raw_value.is_finite() {
        return Err(GatewayIngestError::InvalidRawValue);
    }

    Ok(RawGatewayReading {
        payload_id,
        device_id,
        metric: reading.metric,
        raw_value: reading.raw_value,
        gateway_ts,
    })
}

fn rejection_from_gateway_error(
    error: GatewayIngestError,
    rejected_at: &str,
) -> GatewayPayloadRejection {
    match error {
        GatewayIngestError::MalformedPayload { payload_id, reason } => GatewayPayloadRejection {
            payload_id: Some(payload_id),
            device_id: None,
            reason: GatewayPayloadRejectionReason::MalformedPayload,
            details: reason,
            rejected_at: rejected_at.to_string(),
        },
        other => GatewayPayloadRejection {
            payload_id: None,
            device_id: None,
            reason: GatewayPayloadRejectionReason::MalformedPayload,
            details: other.to_string(),
            rejected_at: rejected_at.to_string(),
        },
    }
}

fn normalize_calibration_profile(
    profile: CalibrationProfile,
) -> Result<CalibrationProfile, SoilIotError> {
    let profile_ref = normalize_required_text(
        profile.profile_ref,
        SoilIotError::EmptyCalibrationProfileRef,
    )?;
    let method_version = normalize_required_text(
        profile.method_version,
        SoilIotError::EmptyCalibrationMethodVersion,
    )?;
    let valid = profile.scale.is_finite()
        && profile.offset.is_finite()
        && profile.valid_min.is_finite()
        && profile.valid_max.is_finite()
        && profile.valid_min <= profile.valid_max;

    if valid {
        Ok(CalibrationProfile {
            profile_ref,
            metric: profile.metric,
            scale: profile.scale,
            offset: profile.offset,
            valid_min: profile.valid_min,
            valid_max: profile.valid_max,
            method_version,
        })
    } else {
        Err(SoilIotError::InvalidCalibrationProfile { profile_ref })
    }
}

#[derive(Default)]
struct ZoneCoverageAccumulator {
    field_id: String,
    zone_id: String,
    device_count: usize,
    fresh_device_count: usize,
    stale_device_count: usize,
    never_seen_device_count: usize,
}

fn normalize_freshness_config(
    config: SoilCaptureFreshnessConfig,
) -> Result<SoilCaptureFreshnessConfig, SoilIotError> {
    if config.expected_interval_seconds == 0 {
        return Err(SoilIotError::InvalidFreshnessInterval);
    }
    Ok(SoilCaptureFreshnessConfig {
        expected_interval_seconds: config.expected_interval_seconds,
        method_version: normalize_required_text(
            config.method_version,
            SoilIotError::EmptyFreshnessMethodVersion,
        )?,
    })
}

fn parse_freshness_ts(value: &str) -> Result<chrono::DateTime<chrono::FixedOffset>, SoilIotError> {
    chrono::DateTime::parse_from_rfc3339(value).map_err(|_| {
        SoilIotError::InvalidFreshnessTimestamp {
            value: value.to_string(),
        }
    })
}

fn normalize_stuck_window_config(
    config: StuckSensorWindowConfig,
) -> Result<StuckSensorWindowConfig, SoilIotError> {
    let method_version = normalize_required_text(
        config.method_version,
        SoilIotError::EmptyStuckWindowMethodVersion,
    )?;
    let valid = config.min_samples > 1
        && config.variance_threshold.is_finite()
        && config.variance_threshold >= 0.0
        && config.range_threshold.is_finite()
        && config.range_threshold >= 0.0
        && config.valid_min.is_finite()
        && config.valid_max.is_finite()
        && config.valid_min <= config.valid_max
        && config.rail_tolerance.is_finite()
        && config.rail_tolerance >= 0.0;

    if valid {
        Ok(StuckSensorWindowConfig {
            min_samples: config.min_samples,
            variance_threshold: config.variance_threshold,
            range_threshold: config.range_threshold,
            valid_min: config.valid_min,
            valid_max: config.valid_max,
            rail_tolerance: config.rail_tolerance,
            method_version,
        })
    } else {
        Err(SoilIotError::InvalidStuckWindowConfig)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SensorHealthObservation {
    breached: bool,
    observed_value: f64,
    threshold_value: f64,
}

fn sensor_health_observations(
    snapshot: &SensorHealthSnapshot,
    thresholds: &SensorHealthThresholds,
) -> Result<Vec<(SensorHealthReasonCode, SensorHealthObservation)>, SoilIotError> {
    let mut observations = Vec::new();

    if let Some(battery_voltage) = snapshot.battery_voltage {
        if !battery_voltage.is_finite() {
            return Err(SoilIotError::InvalidReadingValue);
        }
        observations.push((
            SensorHealthReasonCode::LowBattery,
            SensorHealthObservation {
                breached: battery_voltage < thresholds.low_battery_voltage,
                observed_value: battery_voltage,
                threshold_value: thresholds.low_battery_voltage,
            },
        ));
    }

    if let Some(freshness_age_seconds) = snapshot.freshness_age_seconds {
        observations.push((
            SensorHealthReasonCode::StaleReading,
            SensorHealthObservation {
                breached: freshness_age_seconds > thresholds.stale_after_seconds,
                observed_value: freshness_age_seconds as f64,
                threshold_value: thresholds.stale_after_seconds as f64,
            },
        ));
    }

    let disconnected = snapshot.link_status == SensorHealthLinkStatus::Disconnected;
    observations.push((
        SensorHealthReasonCode::Disconnected,
        SensorHealthObservation {
            breached: disconnected,
            observed_value: if disconnected { 0.0 } else { 1.0 },
            threshold_value: 1.0,
        },
    ));

    observations.sort_by_key(|(reason_code, _)| *reason_code);
    Ok(observations)
}

fn sensor_health_event(
    snapshot: &SensorHealthSnapshot,
    thresholds: &SensorHealthThresholds,
    reason_code: SensorHealthReasonCode,
    kind: SensorHealthEventKind,
    observation: SensorHealthObservation,
) -> SensorHealthEvent {
    SensorHealthEvent {
        device_id: snapshot.device_id.clone(),
        field_id: snapshot.field_id.clone(),
        reason_code,
        kind,
        severity_hint: sensor_health_severity(reason_code, kind),
        evidence: SensorHealthEvidence {
            evidence_ref: snapshot.evidence_ref.clone(),
            metric: reason_code.metric_name().to_string(),
            observed_value: observation.observed_value,
            threshold_value: observation.threshold_value,
            rule_ref: thresholds.method_version.clone(),
        },
        occurred_at: snapshot.evaluated_at.clone(),
        idempotency_key: format!(
            "soil_iot:sensor_health:{}:{}:{}:{}",
            snapshot.device_id,
            reason_code.as_str(),
            kind.as_str(),
            snapshot.evaluated_at
        ),
    }
}

fn sensor_health_event_type(
    reason_code: SensorHealthReasonCode,
    kind: SensorHealthEventKind,
) -> String {
    match kind {
        SensorHealthEventKind::Fired => format!("soil_sensor_health_{}", reason_code.as_str()),
        SensorHealthEventKind::Resolved => {
            format!("soil_sensor_health_{}_resolved", reason_code.as_str())
        }
    }
}

fn sensor_health_severity(
    reason_code: SensorHealthReasonCode,
    kind: SensorHealthEventKind,
) -> AlertSeverityHint {
    if kind == SensorHealthEventKind::Resolved {
        AlertSeverityHint::Info
    } else {
        match reason_code {
            SensorHealthReasonCode::Disconnected => AlertSeverityHint::Critical,
            SensorHealthReasonCode::LowBattery | SensorHealthReasonCode::StaleReading => {
                AlertSeverityHint::Warning
            }
        }
    }
}

fn normalize_sensor_health_snapshot(
    snapshot: SensorHealthSnapshot,
) -> Result<SensorHealthSnapshot, SoilIotError> {
    Ok(SensorHealthSnapshot {
        device_id: normalize_required_text(snapshot.device_id, SoilIotError::EmptyDeviceId)?,
        field_id: normalize_required_text(snapshot.field_id, SoilIotError::EmptyFieldId)?,
        battery_voltage: snapshot.battery_voltage,
        freshness_age_seconds: snapshot.freshness_age_seconds,
        link_status: snapshot.link_status,
        evidence_ref: normalize_required_text(
            snapshot.evidence_ref,
            SoilIotError::EmptySensorHealthEvidenceRef,
        )?,
        evaluated_at: normalize_required_text(
            snapshot.evaluated_at,
            SoilIotError::EmptySensorHealthEvaluatedAt,
        )?,
    })
}

fn normalize_sensor_health_thresholds(
    thresholds: SensorHealthThresholds,
) -> Result<SensorHealthThresholds, SoilIotError> {
    let method_version = normalize_required_text(
        thresholds.method_version,
        SoilIotError::EmptySensorHealthMethodVersion,
    )?;
    if !thresholds.low_battery_voltage.is_finite() || thresholds.low_battery_voltage < 0.0 {
        return Err(SoilIotError::InvalidSensorHealthThresholds);
    }

    Ok(SensorHealthThresholds {
        low_battery_voltage: thresholds.low_battery_voltage,
        stale_after_seconds: thresholds.stale_after_seconds,
        method_version,
    })
}

fn normalize_zone_soil_moisture_product(
    product: ZoneSoilMoistureProduct,
) -> Result<ZoneSoilMoistureProduct, SoilIotError> {
    if !product.value.is_finite() {
        return Err(SoilIotError::InvalidReadingValue);
    }

    Ok(ZoneSoilMoistureProduct {
        product_id: normalize_required_text(
            product.product_id,
            SoilIotError::EmptyZoneSoilProductId,
        )?,
        field_id: normalize_required_text(product.field_id, SoilIotError::EmptyFieldId)?,
        zone_id: normalize_required_text(product.zone_id, SoilIotError::EmptyZoneId)?,
        metric: product.metric,
        value: product.value,
        freshness_age_seconds: product.freshness_age_seconds,
        qa_flags: normalize_text_values(
            product.qa_flags,
            SoilIotError::EmptyIrrigationEvidenceRef,
        )?,
        evidence_refs: normalize_text_values(
            product.evidence_refs,
            SoilIotError::EmptyIrrigationEvidenceRef,
        )?,
        generated_at: normalize_required_text(
            product.generated_at,
            SoilIotError::EmptyZoneSoilGeneratedAt,
        )?,
    })
}

fn normalize_aerial_index_zone_product(
    product: AerialIndexZoneProduct,
) -> Result<AerialIndexZoneProduct, SoilIotError> {
    if !product.mean_value.is_finite() {
        return Err(SoilIotError::InvalidReadingValue);
    }

    Ok(AerialIndexZoneProduct {
        product_id: normalize_required_text(
            product.product_id,
            SoilIotError::EmptyAerialIndexProductId,
        )?,
        field_id: normalize_required_text(product.field_id, SoilIotError::EmptyFieldId)?,
        zone_id: normalize_required_text(product.zone_id, SoilIotError::EmptyZoneId)?,
        index_name: normalize_required_text(
            product.index_name,
            SoilIotError::EmptyAerialIndexName,
        )?,
        mean_value: product.mean_value,
        captured_at: normalize_required_text(
            product.captured_at,
            SoilIotError::EmptyAerialIndexCapturedAt,
        )?,
        crs: normalize_required_text(product.crs, SoilIotError::EmptyAerialIndexCrs)?,
        evidence_refs: normalize_text_values(
            product.evidence_refs,
            SoilIotError::EmptySoilAerialFusionEvidenceRef,
        )?,
    })
}

fn soil_aerial_evidence_refs(
    soil_product_id: &str,
    soil_refs: &[String],
    aerial_product_id: &str,
    aerial_refs: &[String],
) -> Result<Vec<String>, SoilIotError> {
    let mut refs = BTreeSet::new();
    refs.insert(format!("soil-product:{soil_product_id}"));
    refs.insert(format!("aerial-index:{aerial_product_id}"));
    for evidence_ref in soil_refs.iter().chain(aerial_refs.iter()) {
        refs.insert(normalize_required_text(
            evidence_ref.clone(),
            SoilIotError::EmptySoilAerialFusionEvidenceRef,
        )?);
    }
    Ok(refs.into_iter().collect())
}

fn normalize_irrigation_trigger_config(
    config: IrrigationTriggerConfig,
) -> Result<IrrigationTriggerConfig, SoilIotError> {
    let method_version = normalize_required_text(
        config.method_version,
        SoilIotError::EmptyIrrigationTriggerMethodVersion,
    )?;
    if !config.low_moisture_threshold.is_finite() || config.low_moisture_threshold < 0.0 {
        return Err(SoilIotError::InvalidIrrigationTriggerConfig);
    }

    Ok(IrrigationTriggerConfig {
        low_moisture_threshold: config.low_moisture_threshold,
        max_freshness_age_seconds: config.max_freshness_age_seconds,
        method_version,
    })
}

fn validation_evidence_hash(
    validated_readings: &[ValidatedSoilReading],
    qa_evidence: &[ValidationQaEvidenceRecord],
) -> Result<String, SoilIotError> {
    #[derive(Serialize)]
    struct HashInput<'a> {
        validated_readings: &'a [ValidatedSoilReading],
        qa_evidence: &'a [ValidationQaEvidenceRecord],
    }

    let bytes = serde_json::to_vec(&HashInput {
        validated_readings,
        qa_evidence,
    })
    .map_err(|_| SoilIotError::ValidationEvidenceSerializationFailed)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("sha256:{}", to_hex(&digest)))
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn normalize_text_values(
    values: Vec<String>,
    error: SoilIotError,
) -> Result<Vec<String>, SoilIotError> {
    values
        .into_iter()
        .map(|value| normalize_required_text(value, error.clone()))
        .collect::<Result<BTreeSet<_>, _>>()
        .map(|values| values.into_iter().collect())
}

fn validate_gap_geometry(geometry: &SoilZoneGeometry, crs: &str) -> Result<(), SoilIotError> {
    let zone_id = normalize_optional_text(Some(geometry.zone_id.clone()))
        .unwrap_or_else(|| "unknown".to_string());
    let geometry_crs = normalize_required_text(geometry.crs.clone(), SoilIotError::EmptyCrs)?;
    if geometry_crs != crs {
        return Err(SoilIotError::UnsupportedCrs {
            value: geometry_crs,
        });
    }
    if geometry.polygon.is_empty()
        || geometry.polygon[0].len() < 4
        || !geometry
            .polygon
            .iter()
            .flatten()
            .all(valid_geojson_coordinate)
    {
        return Err(SoilIotError::InvalidCoverageGapZoneGeometry { zone_id });
    }
    Ok(())
}

fn valid_geojson_coordinate(coord: &GeoJsonCoordinate) -> bool {
    coord.lat.is_finite()
        && coord.lon.is_finite()
        && (-90.0..=90.0).contains(&coord.lat)
        && (-180.0..=180.0).contains(&coord.lon)
}

fn render_gap_csv(summaries: &[SoilCoverageGapZoneSummary]) -> String {
    let mut csv = "field_id,zone_id,device_count,fresh_device_count,stale_device_count,never_seen_device_count,coverage_fraction,is_gap\n".to_string();
    for summary in summaries.iter().filter(|summary| summary.is_gap) {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{:.6},{}\n",
            csv_escape(&summary.field_id),
            csv_escape(&summary.zone_id),
            summary.device_count,
            summary.fresh_device_count,
            summary.stale_device_count,
            summary.never_seen_device_count,
            summary.coverage_fraction,
            summary.is_gap
        ));
    }
    csv
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn pinned_rail(values: &[f64], config: &StuckSensorWindowConfig) -> Option<StuckSensorRail> {
    if values
        .iter()
        .all(|value| (*value - config.valid_min).abs() <= config.rail_tolerance)
    {
        Some(StuckSensorRail::Lower)
    } else if values
        .iter()
        .all(|value| (*value - config.valid_max).abs() <= config.rail_tolerance)
    {
        Some(StuckSensorRail::Upper)
    } else {
        None
    }
}

fn normalize_position(position: GeoPosition) -> Result<GeoPosition, SoilIotError> {
    let crs = normalize_required_text(position.crs, SoilIotError::EmptyCrs)?;
    if crs != "EPSG:4326" {
        return Err(SoilIotError::UnsupportedCrs { value: crs });
    }
    if !position.latitude.is_finite()
        || !position.longitude.is_finite()
        || !(-90.0..=90.0).contains(&position.latitude)
        || !(-180.0..=180.0).contains(&position.longitude)
    {
        return Err(SoilIotError::InvalidPosition);
    }

    Ok(GeoPosition {
        latitude: position.latitude,
        longitude: position.longitude,
        crs,
    })
}

fn normalize_gateway_text(
    value: String,
    error: GatewayIngestError,
) -> Result<String, GatewayIngestError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_required_text(value: String, error: SoilIotError) -> Result<String, SoilIotError> {
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

#[cfg(test)]
mod tests {
    use super::{
        build_geolocated_soil_reading, build_soil_config_push_record, build_soil_device_record,
        build_zone_soil_product_summary, decode_gateway_payload, detect_stuck_sensor_window,
        emit_sensor_health_alert_events, evaluate_irrigation_trigger,
        evaluate_sensor_health_snapshot, evaluate_soil_aerial_fusion,
        evaluate_soil_capture_freshness_coverage, export_soil_coverage_gaps,
        ingest_gateway_readings, reading_rejection_for_device, rederive_validated_series_evidence,
        transition_soil_config_push_status, transition_soil_device_status,
        validate_and_calibrate_reading, AerialIndexZoneProduct, CalibrationProfile,
        GatewayIngestError, GatewayPayloadRejectionReason, GatewayReadingMetric,
        GatewayReadingRecord, GeoJsonCoordinate, GeoPosition, IrrigationTriggerConfig,
        IrrigationTriggerSuppressionReason, RawGatewayReading, ReadingGeolocationStatus,
        ReadingQaFlag, ReadingQaReason, ReadingRejectionReason, RegisterSoilDeviceRequest,
        SensorHealthEventKind, SensorHealthLinkStatus, SensorHealthMonitorState,
        SensorHealthReasonCode, SensorHealthSnapshot, SensorHealthThresholds, SimulatedGateway,
        SoilAerialFusionOutcome, SoilAerialFusionRefusalReason, SoilAerialFusionRequest,
        SoilCaptureFreshnessConfig, SoilCaptureFreshnessCoverageReport,
        SoilDeviceConfigPushRequest, SoilDeviceConfigPushStatus, SoilDeviceConfigPushStatusUpdate,
        SoilDeviceFreshnessState, SoilDeviceStatus, SoilIotError, SoilSensorType,
        SoilZoneCoverageRecord, SoilZoneGeometry, StuckSensorRail, StuckSensorWindowConfig,
        ValidatedSoilReading, ValidationEvidenceRequest, ZoneSoilMoistureProduct,
        ZoneSoilProductFreshness, ZoneSoilProductRequest, ZoneSoilProductSummary,
    };
    use alerting::{AlertEventBackbone, AlertSeverityHint};
    use timeseries::SeriesValue;

    #[test]
    fn device_registry_builds_active_geolocated_record() {
        let record = build_soil_device_record(
            RegisterSoilDeviceRequest {
                device_id: Some(" soil-probe-001 ".to_string()),
                org_id: " org-001 ".to_string(),
                field_id: " field-001 ".to_string(),
                zone_id: Some(" zone-ne ".to_string()),
                sensor_type: SoilSensorType::SoilMoisture,
                position: GeoPosition {
                    latitude: 38.5816,
                    longitude: -121.4944,
                    crs: " EPSG:4326 ".to_string(),
                },
                calibration_profile_ref: " calibration:soil-probe-001:v1 ".to_string(),
            },
            "generated-device".to_string(),
            "2026-06-12T10:00:00Z".to_string(),
        )
        .expect("device should be valid");

        assert_eq!(record.device_id, "soil-probe-001");
        assert_eq!(record.org_id, "org-001");
        assert_eq!(record.field_id, "field-001");
        assert_eq!(record.zone_id.as_deref(), Some("zone-ne"));
        assert_eq!(record.status, SoilDeviceStatus::Active);
        assert_eq!(record.position.crs, "EPSG:4326");
    }

    #[test]
    fn device_lifecycle_allows_maintenance_then_retirement() {
        let record = valid_device();
        let maintenance = transition_soil_device_status(
            &record,
            SoilDeviceStatus::Maintenance,
            "2026-06-13T10:00:00Z".to_string(),
        )
        .expect("active device can enter maintenance");
        assert_eq!(maintenance.status, SoilDeviceStatus::Maintenance);

        let retired = transition_soil_device_status(
            &maintenance,
            SoilDeviceStatus::Retired,
            "2026-06-14T10:00:00Z".to_string(),
        )
        .expect("maintenance device can retire");
        let error = transition_soil_device_status(
            &retired,
            SoilDeviceStatus::Active,
            "2026-06-15T10:00:00Z".to_string(),
        )
        .expect_err("retired devices cannot reactivate");

        assert_eq!(
            error,
            SoilIotError::RetiredDeviceCannotTransition {
                device_id: "soil-probe-001".to_string()
            }
        );
    }

    #[test]
    fn config_push_records_pending_history_then_applied_ack() {
        let record = build_soil_config_push_record(
            SoilDeviceConfigPushRequest {
                push_id: Some(" push-001 ".to_string()),
                device_id: " soil-probe-001 ".to_string(),
                config_version: " firmware:soil:v3 ".to_string(),
                pushed_at: " 2026-06-12T10:00:00Z ".to_string(),
            },
            "generated-push".to_string(),
        )
        .expect("config push should build");

        assert_eq!(record.push_id, "push-001");
        assert_eq!(record.device_id, "soil-probe-001");
        assert_eq!(record.config_version, "firmware:soil:v3");
        assert_eq!(record.pushed_at, "2026-06-12T10:00:00Z");
        assert_eq!(record.push_status, SoilDeviceConfigPushStatus::Pending);
        assert_eq!(record.failure_reason, None);

        let applied = transition_soil_config_push_status(
            &record,
            SoilDeviceConfigPushStatusUpdate {
                push_status: SoilDeviceConfigPushStatus::Applied,
                failure_reason: None,
                updated_at: "2026-06-12T10:00:05Z".to_string(),
            },
        )
        .expect("pending push can be acknowledged as applied");

        assert_eq!(applied.push_status, SoilDeviceConfigPushStatus::Applied);
        assert_eq!(applied.config_version, "firmware:soil:v3");
        assert_eq!(applied.failure_reason, None);
        assert_eq!(applied.updated_at, "2026-06-12T10:00:05Z");
    }

    #[test]
    fn config_push_timeout_marks_failed_with_reason_and_terminal_status() {
        let record = build_soil_config_push_record(
            SoilDeviceConfigPushRequest {
                push_id: Some("push-timeout".to_string()),
                device_id: "soil-probe-001".to_string(),
                config_version: "firmware:soil:v4".to_string(),
                pushed_at: "2026-06-12T10:00:00Z".to_string(),
            },
            "generated-push".to_string(),
        )
        .expect("config push should build");

        let failed = transition_soil_config_push_status(
            &record,
            SoilDeviceConfigPushStatusUpdate {
                push_status: SoilDeviceConfigPushStatus::Failed,
                failure_reason: Some(" ack_timeout ".to_string()),
                updated_at: "2026-06-12T10:15:00Z".to_string(),
            },
        )
        .expect("pending push can fail after ack window");

        assert_eq!(failed.push_status, SoilDeviceConfigPushStatus::Failed);
        assert_eq!(failed.failure_reason.as_deref(), Some("ack_timeout"));

        let error = transition_soil_config_push_status(
            &failed,
            SoilDeviceConfigPushStatusUpdate {
                push_status: SoilDeviceConfigPushStatus::Applied,
                failure_reason: None,
                updated_at: "2026-06-12T10:16:00Z".to_string(),
            },
        )
        .expect_err("failed pushes are terminal history");

        assert_eq!(
            error,
            SoilIotError::InvalidConfigPushTransition {
                push_id: "push-timeout".to_string(),
                from: SoilDeviceConfigPushStatus::Failed,
                to: SoilDeviceConfigPushStatus::Applied,
            }
        );
    }

    #[test]
    fn config_push_failed_status_requires_reason() {
        let record = build_soil_config_push_record(
            SoilDeviceConfigPushRequest {
                push_id: Some("push-timeout".to_string()),
                device_id: "soil-probe-001".to_string(),
                config_version: "firmware:soil:v4".to_string(),
                pushed_at: "2026-06-12T10:00:00Z".to_string(),
            },
            "generated-push".to_string(),
        )
        .expect("config push should build");

        let error = transition_soil_config_push_status(
            &record,
            SoilDeviceConfigPushStatusUpdate {
                push_status: SoilDeviceConfigPushStatus::Failed,
                failure_reason: Some(" ".to_string()),
                updated_at: "2026-06-12T10:15:00Z".to_string(),
            },
        )
        .expect_err("failed pushes require a durable reason");

        assert_eq!(error, SoilIotError::EmptyConfigPushFailureReason);
    }

    #[test]
    fn reading_from_unknown_or_retired_device_is_rejected() {
        let unknown = reading_rejection_for_device(
            None,
            "soil-probe-missing".to_string(),
            "rejection-001".to_string(),
            "2026-06-12T10:00:00Z".to_string(),
        )
        .expect("rejection should build")
        .expect("unknown device should reject");
        assert_eq!(unknown.reason, ReadingRejectionReason::UnknownDevice);

        let retired = transition_soil_device_status(
            &valid_device(),
            SoilDeviceStatus::Retired,
            "2026-06-13T10:00:00Z".to_string(),
        )
        .expect("device can retire");
        let rejection = reading_rejection_for_device(
            Some(&retired),
            retired.device_id.clone(),
            "rejection-002".to_string(),
            "2026-06-14T10:00:00Z".to_string(),
        )
        .expect("rejection should build")
        .expect("retired device should reject");
        assert_eq!(rejection.reason, ReadingRejectionReason::RetiredDevice);
    }

    #[test]
    fn device_registry_rejects_invalid_position() {
        let error = build_soil_device_record(
            RegisterSoilDeviceRequest {
                device_id: Some("soil-probe-001".to_string()),
                org_id: "org-001".to_string(),
                field_id: "field-001".to_string(),
                zone_id: None,
                sensor_type: SoilSensorType::SoilMoisture,
                position: GeoPosition {
                    latitude: 120.0,
                    longitude: -121.4944,
                    crs: "EPSG:4326".to_string(),
                },
                calibration_profile_ref: "calibration:soil-probe-001:v1".to_string(),
            },
            "generated-device".to_string(),
            "2026-06-12T10:00:00Z".to_string(),
        )
        .expect_err("invalid position should be rejected");

        assert_eq!(error, SoilIotError::InvalidPosition);
    }

    #[test]
    fn simulated_gateway_ingest_records_registered_device_readings() {
        let mut gateway = SimulatedGateway::new(vec![Ok(RawGatewayReading {
            payload_id: "payload-001".to_string(),
            device_id: "soil-probe-001".to_string(),
            metric: GatewayReadingMetric::SoilMoisturePercent,
            raw_value: 34.5,
            gateway_ts: "2026-06-12T10:00:00Z".to_string(),
        })]);
        let device = valid_device();
        let result =
            ingest_gateway_readings(&mut gateway, &[device], "2026-06-12T10:00:03Z".to_string())
                .expect("ingest should succeed");

        assert_eq!(result.accepted_readings.len(), 1);
        assert_eq!(result.accepted_readings[0].device_id, "soil-probe-001");
        assert_eq!(result.accepted_readings[0].raw_value, 34.5);
        assert_eq!(
            result.accepted_readings[0].received_at,
            "2026-06-12T10:00:03Z"
        );
        assert_eq!(result.rejected_payload_count, 0);
    }

    #[test]
    fn malformed_gateway_payload_is_rejected_and_counted() {
        let error = decode_gateway_payload("payload-bad".to_string(), "{not-json")
            .expect_err("malformed payload should fail decode");
        assert!(matches!(error, GatewayIngestError::MalformedPayload { .. }));

        let mut gateway = SimulatedGateway::new(vec![Err(error)]);
        let result = ingest_gateway_readings(
            &mut gateway,
            &[valid_device()],
            "2026-06-12T10:00:03Z".to_string(),
        )
        .expect("malformed payload should be counted, not fatal");

        assert_eq!(result.accepted_readings.len(), 0);
        assert_eq!(result.rejected_payload_count, 1);
        assert_eq!(result.rejections.len(), 1);
        assert_eq!(
            result.rejections[0].reason,
            GatewayPayloadRejectionReason::MalformedPayload
        );
    }

    #[test]
    fn geolocated_reading_inherits_device_position_and_series_contract() {
        let reading = build_geolocated_soil_reading(&valid_device(), valid_gateway_record())
            .expect("reading should geolocate");

        assert_eq!(reading.device_id, "soil-probe-001");
        assert_eq!(reading.field_id, "field-001");
        assert_eq!(reading.zone_id.as_deref(), Some("zone-ne"));
        assert_eq!(
            reading.geolocation_status,
            ReadingGeolocationStatus::Located
        );
        assert!(!reading.excluded_from_geospatial_products);
        assert_eq!(
            reading
                .position
                .as_ref()
                .map(|position| position.crs.as_str()),
            Some("EPSG:4326")
        );

        let point = reading.to_series_point();
        assert_eq!(point.entity_ref, "device:soil-probe-001");
        assert_eq!(point.metric, "soil_moisture_percent");
        assert_eq!(point.unit, "percent");
        assert_eq!(point.t, "2026-06-12T10:00:00Z");
        match point.value {
            SeriesValue::Scalar { value } => assert_eq!(value, 34.5),
            SeriesValue::Raster(_) => panic!("soil readings should be scalar"),
        }
    }

    #[test]
    fn reading_with_invalid_device_position_is_flagged_no_geolocation_without_default_point() {
        let mut device = valid_device();
        device.position.latitude = 120.0;

        let reading = build_geolocated_soil_reading(&device, valid_gateway_record())
            .expect("reading should be retained with no-geolocation flag");

        assert_eq!(
            reading.geolocation_status,
            ReadingGeolocationStatus::NoGeolocation
        );
        assert!(reading.position.is_none());
        assert!(reading.excluded_from_geospatial_products);
    }

    #[test]
    fn validation_applies_linear_calibration_and_retains_raw_value() {
        let reading = build_geolocated_soil_reading(&valid_device(), valid_gateway_record())
            .expect("reading should geolocate");

        let validated = validate_and_calibrate_reading(
            reading,
            calibration_profile(
                GatewayReadingMetric::SoilMoisturePercent,
                0.5,
                1.0,
                0.0,
                100.0,
            ),
        )
        .expect("reading should validate");

        assert_eq!(validated.raw_value, 34.5);
        assert_eq!(validated.calibrated_value, 18.25);
        assert!(validated.qa_flags.is_empty());
        assert!(!validated.excluded_from_products);
    }

    #[test]
    fn validation_flags_out_of_range_reading_and_retains_it() {
        let mut raw = valid_gateway_record();
        raw.raw_value = 250.0;
        let reading =
            build_geolocated_soil_reading(&valid_device(), raw).expect("reading should geolocate");

        let validated = validate_and_calibrate_reading(
            reading,
            calibration_profile(
                GatewayReadingMetric::SoilMoisturePercent,
                1.0,
                0.0,
                0.0,
                100.0,
            ),
        )
        .expect("out-of-range readings are retained with QA flags");

        assert_eq!(validated.raw_value, 250.0);
        assert_eq!(validated.calibrated_value, 250.0);
        assert!(validated.excluded_from_products);
        assert_eq!(validated.qa_flags.len(), 1);
        assert_eq!(
            validated.qa_flags[0].reason_code,
            ReadingQaReason::OutOfRange
        );
        assert_eq!(validated.qa_flags[0].raw_value, 250.0);
    }

    #[test]
    fn validation_evidence_rederivation_is_deterministic() {
        let readings = validation_evidence_fixture_readings();
        let profile = calibration_profile(
            GatewayReadingMetric::SoilMoisturePercent,
            1.0,
            0.0,
            0.0,
            100.0,
        );

        let first = rederive_validated_series_evidence(ValidationEvidenceRequest {
            readings: readings.clone(),
            profile: profile.clone(),
        })
        .expect("validation evidence should be derived");
        let second =
            rederive_validated_series_evidence(ValidationEvidenceRequest { readings, profile })
                .expect("same validation evidence should be re-derived");

        assert_eq!(first.output_hash, second.output_hash);
        assert!(first.output_hash.starts_with("sha256:"));
        assert_eq!(first.reading_count, 2);
        assert_eq!(first.qa_flag_count, 1);
        assert_eq!(first.validated_readings, second.validated_readings);
        assert_eq!(first.qa_evidence, second.qa_evidence);
    }

    #[test]
    fn validation_evidence_retains_qa_rule_thresholds_and_raw_value() {
        let report = rederive_validated_series_evidence(ValidationEvidenceRequest {
            readings: validation_evidence_fixture_readings(),
            profile: calibration_profile(
                GatewayReadingMetric::SoilMoisturePercent,
                1.0,
                0.0,
                0.0,
                100.0,
            ),
        })
        .expect("validation evidence should be derived");

        assert_eq!(report.qa_evidence.len(), 1);
        let flag = &report.qa_evidence[0];
        assert_eq!(flag.payload_id, "payload-out-of-range");
        assert_eq!(flag.device_id, "soil-probe-001");
        assert_eq!(flag.reason_code, ReadingQaReason::OutOfRange);
        assert_eq!(flag.profile_ref, "calibration:soil-probe-001:v1");
        assert_eq!(flag.method_version, "linear-v1");
        assert_eq!(flag.raw_value, 250.0);
        assert_eq!(flag.calibrated_value, 250.0);
        assert_eq!(flag.valid_min, 0.0);
        assert_eq!(flag.valid_max, 100.0);
    }

    #[test]
    fn validation_rejects_profile_for_wrong_metric() {
        let reading = build_geolocated_soil_reading(&valid_device(), valid_gateway_record())
            .expect("reading should geolocate");

        let error = validate_and_calibrate_reading(
            reading,
            calibration_profile(
                GatewayReadingMetric::SoilTemperatureCelsius,
                1.0,
                0.0,
                -40.0,
                80.0,
            ),
        )
        .expect_err("wrong metric profile should fail");

        assert_eq!(
            error,
            SoilIotError::CalibrationMetricMismatch {
                reading_metric: GatewayReadingMetric::SoilMoisturePercent,
                profile_metric: GatewayReadingMetric::SoilTemperatureCelsius
            }
        );
    }

    #[test]
    fn freshness_coverage_counts_fresh_and_stale_devices_per_zone() {
        let devices = vec![
            soil_device("soil-probe-001", "zone-ne"),
            soil_device("soil-probe-002", "zone-ne"),
            soil_device("soil-probe-003", "zone-sw"),
        ];
        let report = evaluate_soil_capture_freshness_coverage(
            &devices,
            &[
                validated_reading(
                    "payload-001",
                    "soil-probe-001",
                    "zone-ne",
                    "2026-06-12T10:09:30Z",
                ),
                validated_reading(
                    "payload-002",
                    "soil-probe-002",
                    "zone-ne",
                    "2026-06-12T10:00:00Z",
                ),
            ],
            freshness_config(),
            "2026-06-12T10:10:00Z".to_string(),
        )
        .expect("freshness report should evaluate");

        assert_eq!(report.field_id, "field-001");
        assert_eq!(report.devices.len(), 3);
        assert_eq!(report.devices[0].state, SoilDeviceFreshnessState::Fresh);
        assert_eq!(report.devices[0].age_seconds, Some(30));
        assert_eq!(report.devices[1].state, SoilDeviceFreshnessState::Stale);
        assert_eq!(report.devices[1].age_seconds, Some(600));
        assert_eq!(report.devices[2].state, SoilDeviceFreshnessState::NeverSeen);

        let zone_ne = report
            .zones
            .iter()
            .find(|zone| zone.zone_id == "zone-ne")
            .expect("zone-ne coverage should exist");
        assert_eq!(zone_ne.device_count, 2);
        assert_eq!(zone_ne.fresh_device_count, 1);
        assert_eq!(zone_ne.stale_device_count, 1);
        assert_eq!(zone_ne.coverage_fraction, 0.5);
    }

    #[test]
    fn freshness_marks_silent_device_stale_after_interval_elapses() {
        let devices = vec![soil_device("soil-probe-001", "zone-ne")];
        let report = evaluate_soil_capture_freshness_coverage(
            &devices,
            &[validated_reading(
                "payload-old",
                "soil-probe-001",
                "zone-ne",
                "2026-06-12T10:00:00Z",
            )],
            freshness_config(),
            "2026-06-12T10:06:01Z".to_string(),
        )
        .expect("freshness report should evaluate");

        assert_eq!(report.devices[0].state, SoilDeviceFreshnessState::Stale);
        assert_eq!(report.devices[0].age_seconds, Some(361));
        assert_eq!(report.zones[0].fresh_device_count, 0);
        assert_eq!(report.zones[0].stale_device_count, 1);
        assert_eq!(report.zones[0].coverage_fraction, 0.0);
    }

    #[test]
    fn coverage_gap_export_outputs_geojson_and_csv_for_uncovered_zones() {
        let export = export_soil_coverage_gaps(
            coverage_report(vec![
                coverage_zone("zone-ne", 2, 1, 1, 0),
                coverage_zone("zone-sw", 1, 0, 0, 1),
            ]),
            vec![zone_geometry("zone-ne"), zone_geometry("zone-sw")],
            "EPSG:4326".to_string(),
        )
        .expect("gap export should build");

        assert_eq!(export.zone_summaries.len(), 2);
        assert!(!export.zone_summaries[0].is_gap);
        assert!(export.zone_summaries[1].is_gap);
        assert!(export.gap_csv.starts_with("field_id,zone_id"));
        assert!(export
            .gap_csv
            .contains("field-001,zone-sw,1,0,0,1,0.000000,true"));
        assert!(!export.gap_csv.contains("zone-ne,2,1"));
        assert_eq!(export.gap_geojson.collection_type, "FeatureCollection");
        assert_eq!(export.gap_geojson.crs.properties.name, "EPSG:4326");
        assert_eq!(export.gap_geojson.features.len(), 1);
        assert_eq!(export.gap_geojson.features[0].id, "zone-sw");
        assert_eq!(
            export.gap_geojson.features[0].properties.fresh_device_count,
            0
        );
        assert_eq!(
            export.gap_geojson.features[0].geometry.coordinates[0][0],
            [-121.50, 38.58]
        );
    }

    #[test]
    fn coverage_gap_export_returns_valid_empty_gap_set_when_fully_covered() {
        let export = export_soil_coverage_gaps(
            coverage_report(vec![
                coverage_zone("zone-ne", 2, 2, 0, 0),
                coverage_zone("zone-sw", 1, 1, 0, 0),
            ]),
            vec![zone_geometry("zone-ne"), zone_geometry("zone-sw")],
            "EPSG:4326".to_string(),
        )
        .expect("fully covered export should build");

        assert!(export.zone_summaries.iter().all(|zone| !zone.is_gap));
        assert!(export.gap_geojson.features.is_empty());
        assert_eq!(
            export.gap_csv,
            "field_id,zone_id,device_count,fresh_device_count,stale_device_count,never_seen_device_count,coverage_fraction,is_gap\n"
        );
    }

    #[test]
    fn coverage_gap_export_requires_geometry_for_uncovered_zone() {
        let error = export_soil_coverage_gaps(
            coverage_report(vec![coverage_zone("zone-sw", 1, 0, 0, 1)]),
            vec![zone_geometry("zone-ne")],
            "EPSG:4326".to_string(),
        )
        .expect_err("uncovered zone without geometry should fail");

        assert_eq!(
            error,
            SoilIotError::MissingCoverageGapZoneGeometry {
                zone_id: "zone-sw".to_string()
            }
        );
    }

    #[test]
    fn stuck_sensor_detection_ignores_normal_variation() {
        let readings = validated_window([34.5, 35.1, 33.8, 34.9]);

        let detection = detect_stuck_sensor_window(&readings, stuck_window_config())
            .expect("normal window should be evaluable");

        assert!(detection.is_none());
    }

    #[test]
    fn stuck_sensor_detection_flags_flatline_with_variance_evidence() {
        let readings = validated_window([34.5, 34.5, 34.5, 34.5]);

        let detection = detect_stuck_sensor_window(&readings, stuck_window_config())
            .expect("flatline window should be evaluable")
            .expect("flatline window should be flagged");

        assert_eq!(detection.reason_code, ReadingQaReason::Stuck);
        assert_eq!(detection.window, 4);
        assert_eq!(detection.sample_count, 4);
        assert_eq!(detection.window_start_ts, "2026-06-12T10:00:00Z");
        assert_eq!(detection.window_end_ts, "2026-06-12T10:00:03Z");
        assert_eq!(detection.observed_variance, 0.0);
        assert_eq!(detection.observed_range, 0.0);
        assert_eq!(detection.observed_min, 34.5);
        assert_eq!(detection.observed_max, 34.5);
        assert_eq!(detection.evidence_payload_ids.len(), 4);
        assert_eq!(detection.pinned_at_rail, None);
    }

    #[test]
    fn stuck_sensor_detection_flags_values_pinned_at_upper_rail() {
        let readings = validated_window([99.98, 100.0, 99.99, 100.0]);
        let mut config = stuck_window_config();
        config.variance_threshold = 0.0;

        let detection = detect_stuck_sensor_window(&readings, config)
            .expect("rail-pinned window should be evaluable")
            .expect("rail-pinned window should be flagged");

        assert_eq!(detection.reason_code, ReadingQaReason::Stuck);
        assert_eq!(detection.pinned_at_rail, Some(StuckSensorRail::Upper));
        assert!(detection.observed_variance > 0.0);
    }

    #[test]
    fn stuck_sensor_detection_requires_complete_clean_window() {
        let short_window = validated_window([34.5, 34.5, 34.5]);

        let short_detection = detect_stuck_sensor_window(&short_window, stuck_window_config())
            .expect("short clean window should be evaluable");
        assert!(short_detection.is_none());

        let mut excluded_window = validated_window([34.5, 34.5, 34.5, 34.5]);
        excluded_window[0].excluded_from_products = true;
        let excluded_detection =
            detect_stuck_sensor_window(&excluded_window, stuck_window_config())
                .expect("excluded window should be evaluable");
        assert!(excluded_detection.is_none());

        let mut flagged_window = validated_window([34.5, 34.5, 34.5, 34.5]);
        flagged_window[0].qa_flags.push(ReadingQaFlag {
            reason_code: ReadingQaReason::OutOfRange,
            profile_ref: "calibration:soil-probe-001:v1".to_string(),
            method_version: "linear-v1".to_string(),
            raw_value: 250.0,
            calibrated_value: 250.0,
            valid_min: 0.0,
            valid_max: 100.0,
        });
        let flagged_detection = detect_stuck_sensor_window(&flagged_window, stuck_window_config())
            .expect("flagged window should be evaluable");
        assert!(flagged_detection.is_none());
    }

    #[test]
    fn stuck_sensor_detection_rejects_mixed_device_or_metric_window() {
        let mut readings = validated_window([34.5, 34.5, 34.5, 34.5]);
        readings[1].device_id = "soil-probe-002".to_string();

        let error = detect_stuck_sensor_window(&readings, stuck_window_config())
            .expect_err("mixed device windows should fail");

        assert_eq!(error, SoilIotError::MixedStuckWindowSeries);

        let mut readings = validated_window([34.5, 34.5, 34.5, 34.5]);
        readings[1].metric = GatewayReadingMetric::SoilTemperatureCelsius;

        let error = detect_stuck_sensor_window(&readings, stuck_window_config())
            .expect_err("mixed metric windows should fail");

        assert_eq!(error, SoilIotError::MixedStuckWindowSeries);
    }

    #[test]
    fn sensor_health_monitor_emits_low_battery_alert_with_evidence() {
        let mut monitor = SensorHealthMonitorState::default();

        let events = evaluate_sensor_health_snapshot(
            sensor_health_snapshot(Some(3.1), Some(45), SensorHealthLinkStatus::Connected),
            sensor_health_thresholds(),
            &mut monitor,
        )
        .expect("health snapshot should evaluate");

        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.kind, SensorHealthEventKind::Fired);
        assert_eq!(event.reason_code, SensorHealthReasonCode::LowBattery);
        assert_eq!(event.severity_hint, AlertSeverityHint::Warning);
        assert_eq!(event.evidence.metric, "battery_voltage");
        assert_eq!(event.evidence.observed_value, 3.1);
        assert_eq!(event.evidence.threshold_value, 3.3);
        assert_eq!(event.evidence.rule_ref, "soil-sensor-health-thresholds-v1");

        let mut backbone = AlertEventBackbone::default();
        let candidates = emit_sensor_health_alert_events(&mut backbone, &events)
            .expect("sensor health event should match alerting contract");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source_domain, "soil_iot");
        assert_eq!(candidates[0].event_type, "soil_sensor_health_low_battery");
        assert_eq!(
            candidates[0].subject_ref,
            "field:field-001:device:soil-probe-001"
        );
        assert_eq!(
            candidates[0].evidence_refs,
            vec![event.evidence.ref_string()]
        );
        assert_eq!(backbone.list_candidates().len(), 1);
    }

    #[test]
    fn sensor_health_monitor_resolves_recovered_condition_without_refiring() {
        let mut monitor = SensorHealthMonitorState::default();
        let thresholds = sensor_health_thresholds();

        let first = evaluate_sensor_health_snapshot(
            sensor_health_snapshot(Some(3.1), Some(45), SensorHealthLinkStatus::Connected),
            thresholds.clone(),
            &mut monitor,
        )
        .expect("low battery should evaluate");
        assert_eq!(first.len(), 1);

        let repeated = evaluate_sensor_health_snapshot(
            sensor_health_snapshot(Some(3.0), Some(46), SensorHealthLinkStatus::Connected),
            thresholds.clone(),
            &mut monitor,
        )
        .expect("repeated low battery should evaluate");
        assert!(repeated.is_empty());

        let recovered = evaluate_sensor_health_snapshot(
            sensor_health_snapshot(Some(3.7), Some(47), SensorHealthLinkStatus::Connected),
            thresholds,
            &mut monitor,
        )
        .expect("recovery should evaluate");

        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].kind, SensorHealthEventKind::Resolved);
        assert_eq!(recovered[0].reason_code, SensorHealthReasonCode::LowBattery);
        assert_eq!(monitor.active_condition_count(), 0);
    }

    #[test]
    fn sensor_health_monitor_detects_stale_and_disconnected_without_spam() {
        let mut monitor = SensorHealthMonitorState::default();

        let first = evaluate_sensor_health_snapshot(
            sensor_health_snapshot(Some(3.8), Some(900), SensorHealthLinkStatus::Disconnected),
            sensor_health_thresholds(),
            &mut monitor,
        )
        .expect("stale disconnected device should evaluate");

        assert_eq!(first.len(), 2);
        assert!(first
            .iter()
            .any(|event| event.reason_code == SensorHealthReasonCode::StaleReading));
        assert!(first
            .iter()
            .any(|event| event.reason_code == SensorHealthReasonCode::Disconnected));

        let repeated = evaluate_sensor_health_snapshot(
            sensor_health_snapshot(Some(3.8), Some(901), SensorHealthLinkStatus::Disconnected),
            sensor_health_thresholds(),
            &mut monitor,
        )
        .expect("repeated stale disconnected device should evaluate");

        assert!(repeated.is_empty());
        assert_eq!(monitor.active_condition_count(), 2);
    }

    #[test]
    fn zone_soil_product_summary_averages_fresh_moisture_readings() {
        let mut first = validated_reading(
            "payload-moisture-001",
            "soil-probe-001",
            "zone-ne",
            "2026-06-12T11:58:30Z",
        );
        first.calibrated_value = 21.0;
        let mut second = validated_reading(
            "payload-moisture-002",
            "soil-probe-002",
            "zone-ne",
            "2026-06-12T11:59:00Z",
        );
        second.calibrated_value = 29.0;

        let summary = build_zone_soil_product_summary(
            zone_product_request(
                GatewayReadingMetric::SoilMoisturePercent,
                vec![first, second],
            ),
            "generated-product".to_string(),
        )
        .expect("fresh moisture readings should summarize");

        assert_eq!(summary.product_id, "generated-product");
        assert_eq!(summary.field_id, "field-001");
        assert_eq!(summary.zone_id, "zone-ne");
        assert_eq!(summary.metric, GatewayReadingMetric::SoilMoisturePercent);
        assert_eq!(summary.freshness, ZoneSoilProductFreshness::Fresh);
        assert_eq!(summary.sample_count, 2);
        assert_eq!(summary.mean_value, Some(25.0));
        assert_eq!(summary.min_value, Some(21.0));
        assert_eq!(summary.max_value, Some(29.0));
        assert_eq!(summary.freshness_age_seconds, 90);
        assert_eq!(
            summary.evidence_refs,
            vec![
                "soil-iot:payload-moisture-001".to_string(),
                "soil-iot:payload-moisture-002".to_string()
            ]
        );
        assert!(summary.excluded_payload_ids.is_empty());
    }

    #[test]
    fn zone_soil_product_summary_excludes_qa_flagged_ec_readings() {
        let mut valid = validated_reading(
            "payload-ec-001",
            "soil-probe-001",
            "zone-ne",
            "2026-06-12T11:59:45Z",
        );
        valid.metric = GatewayReadingMetric::ElectricalConductivity;
        valid.raw_value = 1.2;
        valid.calibrated_value = 1.2;
        let mut flagged = validated_reading(
            "payload-ec-qa",
            "soil-probe-002",
            "zone-ne",
            "2026-06-12T11:59:50Z",
        );
        flagged.metric = GatewayReadingMetric::ElectricalConductivity;
        flagged.raw_value = 9.8;
        flagged.calibrated_value = 9.8;
        flagged.excluded_from_products = true;
        flagged.qa_flags.push(ReadingQaFlag {
            reason_code: ReadingQaReason::OutOfRange,
            profile_ref: "calibration:ec:v1".to_string(),
            method_version: "soil-calibration-v1".to_string(),
            raw_value: 9.8,
            calibrated_value: 9.8,
            valid_min: 0.0,
            valid_max: 5.0,
        });

        let summary = build_zone_soil_product_summary(
            zone_product_request(
                GatewayReadingMetric::ElectricalConductivity,
                vec![valid, flagged],
            ),
            "zone-ec-product-001".to_string(),
        )
        .expect("fresh clean EC readings should summarize");

        assert_eq!(summary.freshness, ZoneSoilProductFreshness::Fresh);
        assert_eq!(summary.sample_count, 1);
        assert_eq!(summary.mean_value, Some(1.2));
        assert_eq!(summary.evidence_refs, vec!["soil-iot:payload-ec-001"]);
        assert_eq!(summary.excluded_payload_ids, vec!["payload-ec-qa"]);
    }

    #[test]
    fn zone_soil_product_summary_marks_stale_temperature_without_value() {
        let mut stale = validated_reading(
            "payload-temp-stale",
            "soil-probe-001",
            "zone-ne",
            "2026-06-12T11:45:00Z",
        );
        stale.metric = GatewayReadingMetric::SoilTemperatureCelsius;
        stale.raw_value = 18.4;
        stale.calibrated_value = 18.4;

        let summary = build_zone_soil_product_summary(
            zone_product_request(GatewayReadingMetric::SoilTemperatureCelsius, vec![stale]),
            "zone-temp-product-001".to_string(),
        )
        .expect("stale temperature readings should produce a stale product");

        assert_eq!(summary.freshness, ZoneSoilProductFreshness::Stale);
        assert_eq!(summary.sample_count, 0);
        assert_eq!(summary.mean_value, None);
        assert_eq!(summary.min_value, None);
        assert_eq!(summary.max_value, None);
        assert_eq!(summary.freshness_age_seconds, 900);
        assert!(summary.evidence_refs.is_empty());
        assert_eq!(summary.excluded_payload_ids, vec!["payload-temp-stale"]);
    }

    #[test]
    fn zone_soil_product_summary_rejects_battery_products() {
        let mut reading = validated_reading(
            "payload-battery-001",
            "soil-probe-001",
            "zone-ne",
            "2026-06-12T11:59:00Z",
        );
        reading.metric = GatewayReadingMetric::BatteryVoltage;

        let error = build_zone_soil_product_summary(
            zone_product_request(GatewayReadingMetric::BatteryVoltage, vec![reading]),
            "zone-battery-product-001".to_string(),
        )
        .expect_err("battery voltage is not a zone soil agronomic product");

        assert_eq!(
            error,
            SoilIotError::UnsupportedZoneSoilProductMetric {
                metric: GatewayReadingMetric::BatteryVoltage
            }
        );
    }

    #[test]
    fn soil_aerial_fusion_reports_cited_divergence_for_aligned_ndvi() {
        let evaluation = evaluate_soil_aerial_fusion(fusion_request(
            zone_soil_product_summary(Some(18.0), ZoneSoilProductFreshness::Fresh),
            "EPSG:4326",
            aerial_ndvi_product(
                "field-001",
                "zone-ne",
                "EPSG:4326",
                "2026-06-12T11:55:00Z",
                0.72,
            ),
        ))
        .expect("aligned soil and NDVI products should evaluate");

        assert!(evaluation.refused_reason.is_none());
        let summary = evaluation.summary.expect("aligned products should fuse");
        assert_eq!(summary.field_id, "field-001");
        assert_eq!(summary.zone_id, "zone-ne");
        assert_eq!(
            summary.soil_metric,
            GatewayReadingMetric::SoilMoisturePercent
        );
        assert_eq!(summary.soil_value, 18.0);
        assert_eq!(summary.soil_crs, "EPSG:4326");
        assert_eq!(summary.aerial_index, "ndvi");
        assert_eq!(summary.aerial_mean_value, 0.72);
        assert_eq!(summary.aerial_crs, "EPSG:4326");
        assert_eq!(summary.temporal_gap_seconds, 300);
        assert!((summary.normalized_delta - -0.54).abs() < 1e-9);
        assert_eq!(summary.outcome, SoilAerialFusionOutcome::Divergence);
        assert!(summary
            .evidence_refs
            .contains(&"soil-product:zone-soil-001".to_string()));
        assert!(summary
            .evidence_refs
            .contains(&"aerial-index:ndvi-zone-001".to_string()));
        assert!(summary
            .evidence_refs
            .contains(&"soil-iot:payload-moisture-001".to_string()));
        assert!(summary
            .evidence_refs
            .contains(&"imagery:ndvi-zone-001".to_string()));
    }

    #[test]
    fn soil_aerial_fusion_refuses_crs_mismatch() {
        let evaluation = evaluate_soil_aerial_fusion(fusion_request(
            zone_soil_product_summary(Some(42.0), ZoneSoilProductFreshness::Fresh),
            "EPSG:4326",
            aerial_ndvi_product(
                "field-001",
                "zone-ne",
                "EPSG:32614",
                "2026-06-12T11:55:00Z",
                0.4,
            ),
        ))
        .expect("CRS mismatch should be a refusal, not a blind fusion");

        assert!(evaluation.summary.is_none());
        assert_eq!(
            evaluation.refused_reason,
            Some(SoilAerialFusionRefusalReason::CrsMismatch)
        );
        assert!(evaluation
            .evidence_refs
            .contains(&"aerial-index:ndvi-zone-001".to_string()));
    }

    #[test]
    fn soil_aerial_fusion_refuses_temporal_mismatch() {
        let evaluation = evaluate_soil_aerial_fusion(fusion_request(
            zone_soil_product_summary(Some(42.0), ZoneSoilProductFreshness::Fresh),
            "EPSG:4326",
            aerial_ndvi_product(
                "field-001",
                "zone-ne",
                "EPSG:4326",
                "2026-06-12T10:30:00Z",
                0.4,
            ),
        ))
        .expect("temporal mismatch should be a refusal");

        assert!(evaluation.summary.is_none());
        assert_eq!(
            evaluation.refused_reason,
            Some(SoilAerialFusionRefusalReason::TemporalMismatch)
        );
        assert!(evaluation
            .evidence_refs
            .contains(&"soil-product:zone-soil-001".to_string()));
    }

    #[test]
    fn irrigation_trigger_emits_domain16_payload_for_fresh_low_moisture() {
        let evaluation = evaluate_irrigation_trigger(
            zone_moisture_product(21.4, 120, vec![], vec!["soil-iot:reading-001"]),
            irrigation_trigger_config(),
        )
        .expect("fresh clean low-moisture product should evaluate");

        assert!(evaluation.suppressed_reason.is_none());
        let trigger = evaluation.trigger.expect("low moisture should trigger");
        assert_eq!(
            trigger.contract_version,
            "water_management.irrigation_trigger.v1"
        );
        assert_eq!(trigger.field_id, "field-001");
        assert_eq!(trigger.zone_id, "zone-ne");
        assert_eq!(trigger.metric, "soil_moisture_percent");
        assert_eq!(trigger.value, 21.4);
        assert_eq!(trigger.threshold, 25.0);
        assert_eq!(trigger.trigger_ts, "2026-06-12T12:00:00Z");
        assert_eq!(trigger.evidence_refs, vec!["soil-iot:reading-001"]);
        assert!(trigger
            .trigger_id
            .contains("irrigation-trigger:field-001:zone-ne"));
    }

    #[test]
    fn irrigation_trigger_suppresses_stale_or_flagged_products_with_reason() {
        let stale = evaluate_irrigation_trigger(
            zone_moisture_product(18.0, 900, vec![], vec!["soil-iot:reading-stale"]),
            irrigation_trigger_config(),
        )
        .expect("stale product should evaluate");
        assert!(stale.trigger.is_none());
        assert_eq!(
            stale.suppressed_reason,
            Some(IrrigationTriggerSuppressionReason::StaleData)
        );
        assert_eq!(stale.evidence_refs, vec!["soil-iot:reading-stale"]);

        let flagged = evaluate_irrigation_trigger(
            zone_moisture_product(
                18.0,
                120,
                vec!["stuck_sensor".to_string()],
                vec!["soil-iot:reading-flagged"],
            ),
            irrigation_trigger_config(),
        )
        .expect("flagged product should evaluate");
        assert!(flagged.trigger.is_none());
        assert_eq!(
            flagged.suppressed_reason,
            Some(IrrigationTriggerSuppressionReason::QaFlagged)
        );
        assert_eq!(flagged.evidence_refs, vec!["soil-iot:reading-flagged"]);
    }

    fn valid_device() -> super::SoilDeviceRecord {
        build_soil_device_record(
            RegisterSoilDeviceRequest {
                device_id: Some("soil-probe-001".to_string()),
                org_id: "org-001".to_string(),
                field_id: "field-001".to_string(),
                zone_id: Some("zone-ne".to_string()),
                sensor_type: SoilSensorType::SoilMoisture,
                position: GeoPosition {
                    latitude: 38.5816,
                    longitude: -121.4944,
                    crs: "EPSG:4326".to_string(),
                },
                calibration_profile_ref: "calibration:soil-probe-001:v1".to_string(),
            },
            "generated-device".to_string(),
            "2026-06-12T10:00:00Z".to_string(),
        )
        .expect("device should be valid")
    }

    fn valid_gateway_record() -> GatewayReadingRecord {
        GatewayReadingRecord {
            payload_id: "payload-001".to_string(),
            device_id: "soil-probe-001".to_string(),
            metric: GatewayReadingMetric::SoilMoisturePercent,
            raw_value: 34.5,
            gateway_ts: "2026-06-12T10:00:00Z".to_string(),
            received_at: "2026-06-12T10:00:03Z".to_string(),
        }
    }

    fn soil_device(device_id: &str, zone_id: &str) -> super::SoilDeviceRecord {
        let mut device = valid_device();
        device.device_id = device_id.to_string();
        device.zone_id = Some(zone_id.to_string());
        device
    }

    fn validated_reading(
        payload_id: &str,
        device_id: &str,
        zone_id: &str,
        ts: &str,
    ) -> ValidatedSoilReading {
        ValidatedSoilReading {
            payload_id: payload_id.to_string(),
            device_id: device_id.to_string(),
            metric: GatewayReadingMetric::SoilMoisturePercent,
            raw_value: 34.5,
            calibrated_value: 34.5,
            ts: ts.to_string(),
            received_at: ts.to_string(),
            field_id: "field-001".to_string(),
            zone_id: Some(zone_id.to_string()),
            source_qa_flags: vec![],
            qa_flags: vec![],
            excluded_from_products: false,
        }
    }

    fn validation_evidence_fixture_readings() -> Vec<super::GeolocatedSoilReading> {
        let mut clean_raw = valid_gateway_record();
        clean_raw.payload_id = "payload-clean".to_string();
        clean_raw.raw_value = 34.5;
        let clean = build_geolocated_soil_reading(&valid_device(), clean_raw)
            .expect("clean reading should geolocate");

        let mut out_of_range_raw = valid_gateway_record();
        out_of_range_raw.payload_id = "payload-out-of-range".to_string();
        out_of_range_raw.raw_value = 250.0;
        let out_of_range = build_geolocated_soil_reading(&valid_device(), out_of_range_raw)
            .expect("out-of-range reading should still geolocate");

        vec![clean, out_of_range]
    }

    fn freshness_config() -> SoilCaptureFreshnessConfig {
        SoilCaptureFreshnessConfig {
            expected_interval_seconds: 300,
            method_version: "soil-capture-freshness-v1".to_string(),
        }
    }

    fn coverage_report(zones: Vec<SoilZoneCoverageRecord>) -> SoilCaptureFreshnessCoverageReport {
        SoilCaptureFreshnessCoverageReport {
            field_id: "field-001".to_string(),
            evaluated_at: "2026-06-12T10:10:00Z".to_string(),
            method_version: "soil-capture-freshness-v1".to_string(),
            devices: Vec::new(),
            zones,
        }
    }

    fn coverage_zone(
        zone_id: &str,
        device_count: usize,
        fresh_device_count: usize,
        stale_device_count: usize,
        never_seen_device_count: usize,
    ) -> SoilZoneCoverageRecord {
        SoilZoneCoverageRecord {
            field_id: "field-001".to_string(),
            zone_id: zone_id.to_string(),
            device_count,
            fresh_device_count,
            stale_device_count,
            never_seen_device_count,
            coverage_fraction: if device_count == 0 {
                0.0
            } else {
                fresh_device_count as f64 / device_count as f64
            },
        }
    }

    fn zone_geometry(zone_id: &str) -> SoilZoneGeometry {
        SoilZoneGeometry {
            zone_id: zone_id.to_string(),
            crs: "EPSG:4326".to_string(),
            polygon: vec![vec![
                GeoJsonCoordinate {
                    lon: -121.50,
                    lat: 38.58,
                },
                GeoJsonCoordinate {
                    lon: -121.49,
                    lat: 38.58,
                },
                GeoJsonCoordinate {
                    lon: -121.49,
                    lat: 38.59,
                },
                GeoJsonCoordinate {
                    lon: -121.50,
                    lat: 38.58,
                },
            ]],
        }
    }

    fn calibration_profile(
        metric: GatewayReadingMetric,
        scale: f64,
        offset: f64,
        valid_min: f64,
        valid_max: f64,
    ) -> CalibrationProfile {
        CalibrationProfile {
            profile_ref: "calibration:soil-probe-001:v1".to_string(),
            metric,
            scale,
            offset,
            valid_min,
            valid_max,
            method_version: "linear-v1".to_string(),
        }
    }

    fn validated_window<const N: usize>(values: [f64; N]) -> Vec<super::ValidatedSoilReading> {
        values
            .into_iter()
            .enumerate()
            .map(|(index, raw_value)| {
                let mut raw = valid_gateway_record();
                raw.payload_id = format!("payload-{index:03}");
                raw.raw_value = raw_value;
                raw.gateway_ts = format!("2026-06-12T10:00:0{index}Z");
                let reading = build_geolocated_soil_reading(&valid_device(), raw)
                    .expect("reading should geolocate");
                validate_and_calibrate_reading(
                    reading,
                    calibration_profile(
                        GatewayReadingMetric::SoilMoisturePercent,
                        1.0,
                        0.0,
                        0.0,
                        100.0,
                    ),
                )
                .expect("reading should validate")
            })
            .collect()
    }

    fn stuck_window_config() -> StuckSensorWindowConfig {
        StuckSensorWindowConfig {
            min_samples: 4,
            variance_threshold: 0.001,
            range_threshold: 0.05,
            valid_min: 0.0,
            valid_max: 100.0,
            rail_tolerance: 0.05,
            method_version: "stuck-window-v1".to_string(),
        }
    }

    fn sensor_health_snapshot(
        battery_voltage: Option<f64>,
        freshness_age_seconds: Option<u64>,
        link_status: SensorHealthLinkStatus,
    ) -> SensorHealthSnapshot {
        SensorHealthSnapshot {
            device_id: "soil-probe-001".to_string(),
            field_id: "field-001".to_string(),
            battery_voltage,
            freshness_age_seconds,
            link_status,
            evidence_ref: "soil-iot:payload-001".to_string(),
            evaluated_at: "2026-06-12T11:00:00Z".to_string(),
        }
    }

    fn sensor_health_thresholds() -> SensorHealthThresholds {
        SensorHealthThresholds {
            low_battery_voltage: 3.3,
            stale_after_seconds: 300,
            method_version: "soil-sensor-health-thresholds-v1".to_string(),
        }
    }

    fn zone_moisture_product(
        value: f64,
        freshness_age_seconds: u64,
        qa_flags: Vec<String>,
        evidence_refs: Vec<&str>,
    ) -> ZoneSoilMoistureProduct {
        ZoneSoilMoistureProduct {
            product_id: "zone-moisture-001".to_string(),
            field_id: "field-001".to_string(),
            zone_id: "zone-ne".to_string(),
            metric: GatewayReadingMetric::SoilMoisturePercent,
            value,
            freshness_age_seconds,
            qa_flags,
            evidence_refs: evidence_refs.into_iter().map(ToOwned::to_owned).collect(),
            generated_at: "2026-06-12T12:00:00Z".to_string(),
        }
    }

    fn zone_product_request(
        metric: GatewayReadingMetric,
        readings: Vec<ValidatedSoilReading>,
    ) -> ZoneSoilProductRequest {
        ZoneSoilProductRequest {
            product_id: None,
            field_id: "field-001".to_string(),
            zone_id: "zone-ne".to_string(),
            metric,
            readings,
            max_freshness_age_seconds: 300,
            generated_at: "2026-06-12T12:00:00Z".to_string(),
            method_version: "soil-zone-product-v1".to_string(),
        }
    }

    fn zone_soil_product_summary(
        mean_value: Option<f64>,
        freshness: ZoneSoilProductFreshness,
    ) -> ZoneSoilProductSummary {
        ZoneSoilProductSummary {
            product_id: "zone-soil-001".to_string(),
            field_id: "field-001".to_string(),
            zone_id: "zone-ne".to_string(),
            metric: GatewayReadingMetric::SoilMoisturePercent,
            mean_value,
            min_value: mean_value,
            max_value: mean_value,
            sample_count: usize::from(mean_value.is_some()),
            freshness,
            freshness_age_seconds: 120,
            method_version: "soil-zone-product-v1".to_string(),
            evidence_refs: vec!["soil-iot:payload-moisture-001".to_string()],
            excluded_payload_ids: Vec::new(),
            generated_at: "2026-06-12T12:00:00Z".to_string(),
        }
    }

    fn aerial_ndvi_product(
        field_id: &str,
        zone_id: &str,
        crs: &str,
        captured_at: &str,
        mean_value: f64,
    ) -> AerialIndexZoneProduct {
        AerialIndexZoneProduct {
            product_id: "ndvi-zone-001".to_string(),
            field_id: field_id.to_string(),
            zone_id: zone_id.to_string(),
            index_name: "ndvi".to_string(),
            mean_value,
            captured_at: captured_at.to_string(),
            crs: crs.to_string(),
            evidence_refs: vec!["imagery:ndvi-zone-001".to_string()],
        }
    }

    fn fusion_request(
        soil_product: ZoneSoilProductSummary,
        soil_crs: &str,
        aerial_product: AerialIndexZoneProduct,
    ) -> SoilAerialFusionRequest {
        SoilAerialFusionRequest {
            soil_product,
            soil_crs: soil_crs.to_string(),
            aerial_product,
            max_temporal_gap_seconds: 900,
            max_agreement_delta: 0.15,
            method_version: "soil-aerial-fusion-v1".to_string(),
        }
    }

    fn irrigation_trigger_config() -> IrrigationTriggerConfig {
        IrrigationTriggerConfig {
            low_moisture_threshold: 25.0,
            max_freshness_age_seconds: 300,
            method_version: "soil-irrigation-trigger-v1".to_string(),
        }
    }
}
