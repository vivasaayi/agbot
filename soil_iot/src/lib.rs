use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
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

impl GeolocatedSoilReading {
    pub fn to_series_point(&self) -> SeriesPoint {
        SeriesPoint {
            entity_ref: format!("device:{}", self.device_id),
            metric: self.metric.as_str().to_string(),
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
    #[error("retired device {device_id} cannot leave retired status")]
    RetiredDeviceCannotTransition { device_id: String },
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
    #[error(
        "stuck-sensor window config must be finite with min_samples > 1 and valid_min <= valid_max"
    )]
    InvalidStuckWindowConfig,
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
        build_geolocated_soil_reading, build_soil_device_record, decode_gateway_payload,
        detect_stuck_sensor_window, ingest_gateway_readings, reading_rejection_for_device,
        transition_soil_device_status, validate_and_calibrate_reading, CalibrationProfile,
        GatewayIngestError, GatewayPayloadRejectionReason, GatewayReadingMetric,
        GatewayReadingRecord, GeoPosition, RawGatewayReading, ReadingGeolocationStatus,
        ReadingQaFlag, ReadingQaReason, ReadingRejectionReason, RegisterSoilDeviceRequest,
        SimulatedGateway, SoilDeviceStatus, SoilIotError, SoilSensorType, StuckSensorRail,
        StuckSensorWindowConfig,
    };
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
}
