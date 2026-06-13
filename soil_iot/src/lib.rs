use alerting::{AlertCandidateRecord, AlertEvent, AlertSeverityHint, AlertingError, SourceAdapter};
use serde::{Deserialize, Serialize};
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
    #[error("stuck-sensor method_version cannot be empty")]
    EmptyStuckWindowMethodVersion,
    #[error("sensor-health method_version cannot be empty")]
    EmptySensorHealthMethodVersion,
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
        decode_gateway_payload, detect_stuck_sensor_window, emit_sensor_health_alert_events,
        evaluate_irrigation_trigger, evaluate_sensor_health_snapshot, ingest_gateway_readings,
        reading_rejection_for_device, transition_soil_config_push_status,
        transition_soil_device_status, validate_and_calibrate_reading, CalibrationProfile,
        GatewayIngestError, GatewayPayloadRejectionReason, GatewayReadingMetric,
        GatewayReadingRecord, GeoPosition, IrrigationTriggerConfig,
        IrrigationTriggerSuppressionReason, RawGatewayReading, ReadingGeolocationStatus,
        ReadingQaFlag, ReadingQaReason, ReadingRejectionReason, RegisterSoilDeviceRequest,
        SensorHealthEventKind, SensorHealthLinkStatus, SensorHealthMonitorState,
        SensorHealthReasonCode, SensorHealthSnapshot, SensorHealthThresholds, SimulatedGateway,
        SoilDeviceConfigPushRequest, SoilDeviceConfigPushStatus, SoilDeviceConfigPushStatusUpdate,
        SoilDeviceStatus, SoilIotError, SoilSensorType, StuckSensorRail, StuckSensorWindowConfig,
        ZoneSoilMoistureProduct,
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

    fn irrigation_trigger_config() -> IrrigationTriggerConfig {
        IrrigationTriggerConfig {
            low_moisture_threshold: 25.0,
            max_freshness_age_seconds: 300,
            method_version: "soil-irrigation-trigger-v1".to_string(),
        }
    }
}
