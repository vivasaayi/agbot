use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::schemas::GpsCoords;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub mod export;
pub mod indexing;
pub mod multispectral;
pub mod rplidar;
pub mod simulated_capture;
pub mod storage;

pub use export::{DataExporter, ExportFormat};
pub use indexing::{DataIndexer, IndexConfig, SearchQuery};
pub use multispectral::{
    multispectral_capture_to_record, validate_multispectral_capture, MultispectralBandCapture,
    MultispectralCaptureError, MultispectralCaptureManifest, MultispectralRecordError,
};
pub use rplidar::{
    lidar_scan_to_record, parse_rplidar_a3_measurements, LidarRecordError, RplidarParseError,
};
pub use simulated_capture::{
    flight_sim_cpp_lidar_observation_from_json, flight_sim_cpp_multispectral_observation_from_json,
    simulated_capture_frame_to_batch, SimulatedCaptureBatch, SimulatedCaptureError,
    SimulatedCaptureFrame, SimulatedCapturePath, SimulatedCapturePathStep,
    SimulatedSensorObservation,
};
pub use storage::{StorageConfig, StorageEngine};

/// Data collection and storage system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightDataRecord {
    pub id: Uuid,
    #[serde(default = "default_link_id")]
    pub session_id: Uuid,
    pub flight_id: Uuid,
    pub drone_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub data_type: DataType,
    pub payload: DataPayload,
    #[serde(default)]
    pub sensor_id: String,
    #[serde(default)]
    pub gps_coords: Option<GpsCoords>,
    #[serde(default)]
    pub calibration_ref: String,
    pub metadata: HashMap<String, String>,
    pub file_path: Option<PathBuf>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightDataProvenance {
    pub session_id: Uuid,
    pub sensor_id: String,
    pub gps_coords: Option<GpsCoords>,
    pub timestamp: Option<DateTime<Utc>>,
    pub calibration_ref: String,
}

impl FlightDataProvenance {
    pub fn complete(
        session_id: Uuid,
        sensor_id: String,
        gps_coords: GpsCoords,
        timestamp: DateTime<Utc>,
        calibration_ref: String,
    ) -> Self {
        Self {
            session_id,
            sensor_id,
            gps_coords: Some(gps_coords),
            timestamp: Some(timestamp),
            calibration_ref,
        }
    }

    pub fn validate(&self) -> std::result::Result<DateTime<Utc>, FlightDataProvenanceError> {
        if self.session_id == Uuid::nil() {
            return Err(FlightDataProvenanceError::MissingSessionId);
        }

        if self.sensor_id.trim().is_empty() {
            return Err(FlightDataProvenanceError::MissingSensorId);
        }

        let gps_coords = self
            .gps_coords
            .as_ref()
            .ok_or(FlightDataProvenanceError::MissingGpsCoords)?;
        if !gps_coords.latitude.is_finite()
            || !gps_coords.longitude.is_finite()
            || !gps_coords.altitude.is_finite()
            || !(-90.0..=90.0).contains(&gps_coords.latitude)
            || !(-180.0..=180.0).contains(&gps_coords.longitude)
        {
            return Err(FlightDataProvenanceError::InvalidGpsCoords);
        }

        let timestamp = self
            .timestamp
            .ok_or(FlightDataProvenanceError::MissingTimestamp)?;

        if self.calibration_ref.trim().is_empty() {
            return Err(FlightDataProvenanceError::MissingCalibrationRef);
        }

        Ok(timestamp)
    }
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum FlightDataProvenanceError {
    #[error("flight data record is missing session_id")]
    MissingSessionId,
    #[error("flight data record is missing sensor_id")]
    MissingSensorId,
    #[error("flight data record is missing gps coordinates")]
    MissingGpsCoords,
    #[error("flight data record has invalid gps coordinates")]
    InvalidGpsCoords,
    #[error("flight data record is missing timestamp")]
    MissingTimestamp,
    #[error("flight data record is missing calibration_ref")]
    MissingCalibrationRef,
    #[error("flight data record session {record_session_id} does not match collection session {expected_session_id}")]
    SessionMismatch {
        expected_session_id: Uuid,
        record_session_id: Uuid,
    },
}

impl FlightDataRecord {
    pub fn new(
        flight_id: Uuid,
        drone_id: Uuid,
        data_type: DataType,
        payload: DataPayload,
        provenance: FlightDataProvenance,
        size_bytes: u64,
    ) -> std::result::Result<Self, FlightDataProvenanceError> {
        let timestamp = provenance.validate()?;

        Ok(Self {
            id: Uuid::new_v4(),
            session_id: provenance.session_id,
            flight_id,
            drone_id,
            timestamp,
            data_type,
            payload,
            sensor_id: provenance.sensor_id,
            gps_coords: provenance.gps_coords,
            calibration_ref: provenance.calibration_ref,
            metadata: HashMap::new(),
            file_path: None,
            size_bytes,
        })
    }

    pub fn validate_provenance(&self) -> std::result::Result<(), FlightDataProvenanceError> {
        FlightDataProvenance {
            session_id: self.session_id,
            sensor_id: self.sensor_id.clone(),
            gps_coords: self.gps_coords.clone(),
            timestamp: Some(self.timestamp),
            calibration_ref: self.calibration_ref.clone(),
        }
        .validate()?;
        Ok(())
    }
}

pub(crate) fn prepare_record_for_storage(record: &FlightDataRecord) -> Result<FlightDataRecord> {
    let mut prepared = record.clone();
    apply_quality_mask(&mut prepared);
    prepared.metadata.remove(INTEGRITY_CHECKSUM_KEY);
    prepared.metadata.remove(INTEGRITY_VERIFIED_KEY);
    let checksum = record_integrity_checksum(&prepared)?;
    prepared
        .metadata
        .insert(INTEGRITY_CHECKSUM_KEY.to_string(), checksum);
    Ok(prepared)
}

pub(crate) fn verify_record_integrity(record: &FlightDataRecord) -> Result<FlightDataRecord> {
    let expected = record
        .metadata
        .get(INTEGRITY_CHECKSUM_KEY)
        .ok_or_else(|| anyhow::anyhow!("record {} is missing integrity checksum", record.id))?;
    let actual = record_integrity_checksum(record)?;
    if expected != &actual {
        anyhow::bail!(
            "record {} checksum mismatch: expected {}, computed {}",
            record.id,
            expected,
            actual
        );
    }

    let mut verified = record.clone();
    verified
        .metadata
        .insert(INTEGRITY_VERIFIED_KEY.to_string(), "true".to_string());
    Ok(verified)
}

fn record_integrity_checksum(record: &FlightDataRecord) -> Result<String> {
    let mut canonical = record.clone();
    canonical.metadata.clear();
    let encoded = serde_json::to_vec(&canonical)?;
    Ok(format!("{:016x}", fnv1a64(&encoded)))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn apply_quality_mask(record: &mut FlightDataRecord) {
    record.metadata.remove(QA_MASKED_KEY);
    record.metadata.remove(QA_REASON_KEY);

    let reason = match &record.payload {
        DataPayload::PointCloud { point_count, .. } if *point_count < 3 => {
            Some("sparse_point_cloud")
        }
        DataPayload::SensorData {
            sensor_type,
            values,
            ..
        } if sensor_type.eq_ignore_ascii_case("multispectral")
            && values
                .values()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value)) =>
        {
            Some("spectral_value_out_of_range")
        }
        _ => None,
    };

    if let Some(reason) = reason {
        record
            .metadata
            .insert(QA_MASKED_KEY.to_string(), "true".to_string());
        record
            .metadata
            .insert(QA_REASON_KEY.to_string(), reason.to_string());
    }
}

fn qa_mask_reason(record: &FlightDataRecord) -> Option<&str> {
    if record.metadata.get(QA_MASKED_KEY).map(String::as_str) == Some("true") {
        record.metadata.get(QA_REASON_KEY).map(String::as_str)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DataType {
    Telemetry,
    SensorReading,
    Image,
    Video,
    LidarScan,
    MultispectralImage,
    ThermalImage,
    GPSTrack,
    FlightLog,
    MissionPlan,
    WeatherData,
    SystemLog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataPayload {
    Telemetry {
        position: (f64, f64, f32),
        velocity: (f32, f32, f32),
        orientation: (f32, f32, f32),
        battery_level: f32,
        signal_strength: f32,
    },
    SensorData {
        sensor_type: String,
        values: HashMap<String, f64>,
        calibration_info: Option<String>,
    },
    MediaFile {
        file_type: String,
        dimensions: Option<(u32, u32)>,
        duration_seconds: Option<f32>,
        compression: Option<String>,
    },
    PointCloud {
        point_count: u32,
        bounds: ((f32, f32, f32), (f32, f32, f32)),
        format: String,
        has_color: bool,
        has_intensity: bool,
    },
    TrackLog {
        waypoint_count: u32,
        total_distance_m: f32,
        duration_seconds: f32,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    },
    Raw {
        format: String,
        schema: Option<String>,
        compression: Option<String>,
    },
}

fn default_link_id() -> Uuid {
    Uuid::nil()
}

fn default_owner_id() -> String {
    "unassigned".to_string()
}

const INTEGRITY_CHECKSUM_KEY: &str = "integrity_checksum";
const INTEGRITY_VERIFIED_KEY: &str = "integrity_verified";
const QA_MASKED_KEY: &str = "qa_masked";
const QA_REASON_KEY: &str = "qa_reason";
const DEFAULT_CAPTURE_FRESHNESS_THRESHOLD_SECONDS: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSessionRequest {
    pub flight_id: Uuid,
    pub field_id: Uuid,
    pub scene_id: Uuid,
    pub drone_id: Uuid,
    pub owner_id: String,
    pub mission_id: Option<Uuid>,
    pub tags: Vec<String>,
}

impl CaptureSessionRequest {
    pub fn new(
        flight_id: Uuid,
        field_id: Uuid,
        scene_id: Uuid,
        drone_id: Uuid,
        owner_id: String,
    ) -> Self {
        Self {
            flight_id,
            field_id,
            scene_id,
            drone_id,
            owner_id,
            mission_id: None,
            tags: Vec::new(),
        }
    }

    pub fn with_mission_id(mut self, mission_id: Uuid) -> Self {
        self.mission_id = Some(mission_id);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureLinkageReference {
    pub flight_id: Uuid,
    pub field_id: Uuid,
    pub scene_id: Uuid,
}

impl CaptureLinkageReference {
    pub fn new(flight_id: Uuid, field_id: Uuid, scene_id: Uuid) -> Self {
        Self {
            flight_id,
            field_id,
            scene_id,
        }
    }

    pub fn from_request(request: &CaptureSessionRequest) -> Self {
        Self::new(request.flight_id, request.field_id, request.scene_id)
    }
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum CaptureLinkageError {
    #[error("capture session references unknown flight_id: {flight_id}")]
    UnknownFlight { flight_id: Uuid },
    #[error("capture session references unknown field_id: {field_id}")]
    UnknownField { field_id: Uuid },
    #[error("capture session references unknown scene_id: {scene_id}")]
    UnknownScene { scene_id: Uuid },
    #[error(
        "capture session scene {scene_id} belongs to field {expected_field_id}, not {field_id}"
    )]
    SceneFieldMismatch {
        scene_id: Uuid,
        expected_field_id: Uuid,
        field_id: Uuid,
    },
}

#[derive(Debug, Clone, Default)]
struct CaptureLinkageCatalog {
    flights: HashSet<Uuid>,
    fields: HashSet<Uuid>,
    scenes_by_field: HashMap<Uuid, Uuid>,
}

impl CaptureLinkageCatalog {
    fn register(
        &mut self,
        reference: CaptureLinkageReference,
    ) -> std::result::Result<(), CaptureLinkageError> {
        if let Some(expected_field_id) = self.scenes_by_field.get(&reference.scene_id) {
            if *expected_field_id != reference.field_id {
                return Err(CaptureLinkageError::SceneFieldMismatch {
                    scene_id: reference.scene_id,
                    expected_field_id: *expected_field_id,
                    field_id: reference.field_id,
                });
            }
        }

        self.flights.insert(reference.flight_id);
        self.fields.insert(reference.field_id);
        self.scenes_by_field
            .insert(reference.scene_id, reference.field_id);
        Ok(())
    }

    fn validate(
        &self,
        reference: CaptureLinkageReference,
    ) -> std::result::Result<(), CaptureLinkageError> {
        if !self.flights.contains(&reference.flight_id) {
            return Err(CaptureLinkageError::UnknownFlight {
                flight_id: reference.flight_id,
            });
        }

        if !self.fields.contains(&reference.field_id) {
            return Err(CaptureLinkageError::UnknownField {
                field_id: reference.field_id,
            });
        }

        let expected_field_id = self.scenes_by_field.get(&reference.scene_id).ok_or(
            CaptureLinkageError::UnknownScene {
                scene_id: reference.scene_id,
            },
        )?;
        if *expected_field_id != reference.field_id {
            return Err(CaptureLinkageError::SceneFieldMismatch {
                scene_id: reference.scene_id,
                expected_field_id: *expected_field_id,
                field_id: reference.field_id,
            });
        }

        Ok(())
    }
}

/// Flight session containing related data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightSession {
    pub id: Uuid,
    #[serde(default = "default_link_id")]
    pub flight_id: Uuid,
    #[serde(default = "default_link_id")]
    pub field_id: Uuid,
    #[serde(default = "default_link_id")]
    pub scene_id: Uuid,
    #[serde(default = "default_owner_id")]
    pub owner_id: String,
    pub mission_id: Option<Uuid>,
    pub drone_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub status: SessionStatus,
    pub data_records: Vec<Uuid>,
    pub summary: SessionSummary,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionStatus {
    Started,
    Collecting,
    Ended,
    Failed,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SessionLifecycleError {
    #[error("capture session not found: {session_id}")]
    SessionNotFound { session_id: Uuid },
    #[error("invalid capture session transition for {session_id}: {from:?} -> {to:?}")]
    InvalidStatusTransition {
        session_id: Uuid,
        from: SessionStatus,
        to: SessionStatus,
    },
}

impl FlightSession {
    pub fn new(request: CaptureSessionRequest) -> Self {
        Self {
            id: Uuid::new_v4(),
            flight_id: request.flight_id,
            field_id: request.field_id,
            scene_id: request.scene_id,
            owner_id: request.owner_id,
            mission_id: request.mission_id,
            drone_id: request.drone_id,
            start_time: Utc::now(),
            end_time: None,
            status: SessionStatus::Started,
            data_records: Vec::new(),
            summary: SessionSummary::default(),
            tags: request.tags,
        }
    }

    pub fn transition_status(
        &mut self,
        next_status: SessionStatus,
    ) -> std::result::Result<(), SessionLifecycleError> {
        if self.status == next_status {
            return Ok(());
        }

        if !Self::is_valid_transition(self.status, next_status) {
            return Err(SessionLifecycleError::InvalidStatusTransition {
                session_id: self.id,
                from: self.status,
                to: next_status,
            });
        }

        self.status = next_status;
        Ok(())
    }

    fn is_valid_transition(from: SessionStatus, to: SessionStatus) -> bool {
        matches!(
            (from, to),
            (SessionStatus::Started, SessionStatus::Collecting)
                | (SessionStatus::Started, SessionStatus::Ended)
                | (SessionStatus::Started, SessionStatus::Failed)
                | (SessionStatus::Collecting, SessionStatus::Ended)
                | (SessionStatus::Collecting, SessionStatus::Failed)
        )
    }

    fn record_successful_capture(&mut self, record: &FlightDataRecord, now: DateTime<Utc>) {
        let last_record_at = self
            .summary
            .freshness
            .last_record_at
            .map_or(record.timestamp, |current| current.max(record.timestamp));
        self.summary.freshness.last_record_at = Some(last_record_at);
        self.summary.capture_health.record_success();
        self.refresh_capture_quality(now);
    }

    fn record_collection_failure(&mut self, failure: CollectionFailure, now: DateTime<Utc>) {
        self.summary.collection_failures.push(failure);
        self.refresh_capture_quality(now);
    }

    fn record_capture_reader_error(
        &mut self,
        error: &CaptureReaderError,
        retry_backoff_ms: Option<u64>,
        exhausted: bool,
    ) {
        self.summary
            .capture_health
            .record_reader_error(error, retry_backoff_ms, exhausted);
    }

    fn refresh_capture_quality(&mut self, now: DateTime<Utc>) {
        let threshold = chrono::Duration::seconds(DEFAULT_CAPTURE_FRESHNESS_THRESHOLD_SECONDS);
        self.summary.freshness = CaptureFreshness::from_last_record(
            self.summary.freshness.last_record_at,
            now,
            threshold,
        );
        self.summary.coverage = CaptureCoverage::from_counts(
            self.summary.record_count,
            self.summary.collection_failures.len() as u32,
        );
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FreshnessStatus {
    NoData,
    Fresh,
    Stale,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureFreshness {
    pub last_record_at: Option<DateTime<Utc>>,
    pub age_seconds: Option<i64>,
    pub status: FreshnessStatus,
}

impl CaptureFreshness {
    fn from_last_record(
        last_record_at: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
        stale_after: chrono::Duration,
    ) -> Self {
        match last_record_at {
            Some(last_record_at) => {
                let age_seconds = now
                    .signed_duration_since(last_record_at)
                    .num_seconds()
                    .max(0);
                let status = if age_seconds <= stale_after.num_seconds() {
                    FreshnessStatus::Fresh
                } else {
                    FreshnessStatus::Stale
                };

                Self {
                    last_record_at: Some(last_record_at),
                    age_seconds: Some(age_seconds),
                    status,
                }
            }
            None => Self::default(),
        }
    }
}

impl Default for CaptureFreshness {
    fn default() -> Self {
        Self {
            last_record_at: None,
            age_seconds: None,
            status: FreshnessStatus::NoData,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CaptureCoverageStatus {
    Unknown,
    Complete,
    Partial,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureCoverage {
    pub successful_records: u32,
    pub failed_observations: u32,
    pub expected_observations: u32,
    pub captured_fraction: f32,
    pub status: CaptureCoverageStatus,
}

impl CaptureCoverage {
    fn from_counts(successful_records: u32, failed_observations: u32) -> Self {
        let expected_observations = successful_records + failed_observations;
        if expected_observations == 0 {
            return Self::default();
        }

        let captured_fraction = successful_records as f32 / expected_observations as f32;
        let status = if failed_observations == 0 {
            CaptureCoverageStatus::Complete
        } else {
            CaptureCoverageStatus::Partial
        };

        Self {
            successful_records,
            failed_observations,
            expected_observations,
            captured_fraction,
            status,
        }
    }
}

impl Default for CaptureCoverage {
    fn default() -> Self {
        Self {
            successful_records: 0,
            failed_observations: 0,
            expected_observations: 0,
            captured_fraction: 0.0,
            status: CaptureCoverageStatus::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CaptureHealthStatus {
    Unknown,
    Healthy,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureHealth {
    pub successful_reads: u32,
    pub transient_errors: u32,
    pub persistent_errors: u32,
    pub retry_attempts: u32,
    pub operator_alerts: u32,
    pub success_rate: f32,
    pub last_error: Option<String>,
    pub last_backoff_ms: Option<u64>,
    pub status: CaptureHealthStatus,
}

impl CaptureHealth {
    fn record_success(&mut self) {
        self.successful_reads += 1;
        self.recalculate();
    }

    fn record_reader_error(
        &mut self,
        error: &CaptureReaderError,
        retry_backoff_ms: Option<u64>,
        exhausted: bool,
    ) {
        self.transient_errors += 1;
        self.last_error = Some(error.message.clone());

        if let Some(backoff_ms) = retry_backoff_ms {
            self.retry_attempts += 1;
            self.last_backoff_ms = Some(backoff_ms);
        }

        if exhausted {
            self.persistent_errors += 1;
            self.operator_alerts += 1;
        }

        self.recalculate();
    }

    fn recalculate(&mut self) {
        let total_observations =
            self.successful_reads + self.transient_errors + self.persistent_errors;
        self.success_rate = if total_observations == 0 {
            0.0
        } else {
            self.successful_reads as f32 / total_observations as f32
        };

        self.status = if self.persistent_errors > 0 {
            CaptureHealthStatus::Failed
        } else if self.transient_errors > 0 {
            CaptureHealthStatus::Degraded
        } else if self.successful_reads > 0 {
            CaptureHealthStatus::Healthy
        } else {
            CaptureHealthStatus::Unknown
        };
    }
}

impl Default for CaptureHealth {
    fn default() -> Self {
        Self {
            successful_reads: 0,
            transient_errors: 0,
            persistent_errors: 0,
            retry_attempts: 0,
            operator_alerts: 0,
            success_rate: 0.0,
            last_error: None,
            last_backoff_ms: None,
            status: CaptureHealthStatus::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetryBackoffPolicy {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub multiplier: u64,
}

impl RetryBackoffPolicy {
    pub fn new(max_retries: u32, initial_backoff_ms: u64, multiplier: u64) -> Self {
        Self {
            max_retries,
            initial_backoff_ms,
            multiplier: multiplier.max(1),
        }
    }

    fn backoff_ms_for_retry(&self, retry_index: u32) -> u64 {
        let mut backoff_ms = self.initial_backoff_ms;
        for _ in 0..retry_index {
            backoff_ms = backoff_ms.saturating_mul(self.multiplier);
        }
        backoff_ms
    }
}

impl Default for RetryBackoffPolicy {
    fn default() -> Self {
        Self::new(3, 100, 2)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureReaderError {
    pub sensor_id: String,
    pub data_type: DataType,
    pub occurred_at: DateTime<Utc>,
    pub message: String,
}

impl CaptureReaderError {
    pub fn transient(
        sensor_id: impl Into<String>,
        data_type: DataType,
        occurred_at: DateTime<Utc>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            sensor_id: sensor_id.into(),
            data_type,
            occurred_at,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureRetryReport {
    pub attempts: u32,
    pub backoff_schedule_ms: Vec<u64>,
    pub recovered: bool,
    pub failure: Option<CollectionFailure>,
    pub health: CaptureHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CollectionFailureKind {
    SensorDropout,
    MalformedFrame,
    MissingBand,
    ReaderError,
    QualityMasked,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CollectionFailure {
    pub id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub sensor_id: String,
    pub data_type: DataType,
    pub kind: CollectionFailureKind,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionFailureRequest {
    pub occurred_at: Option<DateTime<Utc>>,
    pub sensor_id: String,
    pub data_type: DataType,
    pub kind: CollectionFailureKind,
    pub message: String,
}

impl CollectionFailureRequest {
    fn into_failure(self) -> CollectionFailure {
        CollectionFailure {
            id: Uuid::new_v4(),
            occurred_at: self.occurred_at.unwrap_or_else(Utc::now),
            sensor_id: self.sensor_id,
            data_type: self.data_type,
            kind: self.kind,
            message: self.message,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub total_data_size_bytes: u64,
    pub record_count: u32,
    pub data_types: HashMap<DataType, u32>,
    pub flight_duration_seconds: f32,
    pub distance_covered_m: f32,
    pub area_covered_m2: f32,
    pub battery_consumed_percent: f32,
    #[serde(default)]
    pub freshness: CaptureFreshness,
    #[serde(default)]
    pub coverage: CaptureCoverage,
    #[serde(default)]
    pub collection_failures: Vec<CollectionFailure>,
    #[serde(default)]
    pub capture_health: CaptureHealth,
    #[serde(default)]
    pub aggregate_evidence: SessionAggregateEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionAggregateStatus {
    FromTelemetryTrack,
    NoTelemetryTrack,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionAggregateEvidence {
    pub status: SessionAggregateStatus,
    pub telemetry_record_ids: Vec<Uuid>,
    pub sample_count: usize,
}

impl Default for SessionAggregateEvidence {
    fn default() -> Self {
        Self {
            status: SessionAggregateStatus::NoTelemetryTrack,
            telemetry_record_ids: Vec::new(),
            sample_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureSessionListFilter {
    pub field_id: Option<Uuid>,
    pub flight_id: Option<Uuid>,
    pub started_after: Option<DateTime<Utc>>,
    pub started_before: Option<DateTime<Utc>>,
    pub offset: usize,
    pub limit: usize,
}

impl Default for CaptureSessionListFilter {
    fn default() -> Self {
        Self {
            field_id: None,
            flight_id: None,
            started_after: None,
            started_before: None,
            offset: 0,
            limit: 50,
        }
    }
}

impl CaptureSessionListFilter {
    fn normalized_limit(&self) -> usize {
        if self.limit == 0 {
            50
        } else {
            self.limit.min(500)
        }
    }

    fn matches(&self, session: &FlightSession) -> bool {
        if let Some(field_id) = self.field_id {
            if session.field_id != field_id {
                return false;
            }
        }

        if let Some(flight_id) = self.flight_id {
            if session.flight_id != flight_id {
                return false;
            }
        }

        if let Some(started_after) = self.started_after {
            if session.start_time < started_after {
                return false;
            }
        }

        if let Some(started_before) = self.started_before {
            if session.start_time > started_before {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureSessionListPage {
    pub items: Vec<CaptureSessionListItem>,
    pub total_count: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureSessionListItem {
    pub session_id: Uuid,
    pub flight_id: Uuid,
    pub field_id: Uuid,
    pub scene_id: Uuid,
    pub drone_id: Uuid,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub record_count: u32,
    pub failure_count: u32,
    pub freshness: CaptureFreshness,
    pub coverage: CaptureCoverage,
    pub aggregate_evidence: SessionAggregateEvidence,
    pub capture_health: CaptureHealth,
    pub qa: CaptureQaSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureQaSummary {
    pub passed: bool,
    pub masked_records: u32,
    pub reasons: Vec<String>,
}

impl CaptureQaSummary {
    fn from_summary(summary: &SessionSummary) -> Self {
        let mut reasons = summary
            .collection_failures
            .iter()
            .filter(|failure| failure.kind == CollectionFailureKind::QualityMasked)
            .map(|failure| failure.message.clone())
            .collect::<Vec<_>>();
        reasons.sort();
        reasons.dedup();

        let masked_records = summary
            .collection_failures
            .iter()
            .filter(|failure| failure.kind == CollectionFailureKind::QualityMasked)
            .count() as u32;

        Self {
            passed: masked_records == 0,
            masked_records,
            reasons,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReflyGapReason {
    CoverageGap,
    LowQualityMask,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReflyGapGeometry {
    pub field_id: Uuid,
    pub geometry_type: String,
    pub coordinates: Vec<GpsCoords>,
    pub area_fraction: f32,
    pub source_record_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureReflyRecommendation {
    pub session_id: Uuid,
    pub field_id: Uuid,
    pub linked_mission_id: Option<Uuid>,
    pub recommended: bool,
    pub advisory_only: bool,
    pub gate: String,
    pub reasons: Vec<ReflyGapReason>,
    pub gap_geometry: Option<ReflyGapGeometry>,
}

fn gap_geometry_for(
    field_id: Uuid,
    records: &[FlightDataRecord],
    area_fraction: f32,
) -> ReflyGapGeometry {
    let gps_points = records
        .iter()
        .filter_map(|record| record.gps_coords.clone())
        .collect::<Vec<_>>();

    if gps_points.is_empty() {
        return ReflyGapGeometry {
            field_id,
            geometry_type: "field_relative_unobserved".to_string(),
            coordinates: Vec::new(),
            area_fraction,
            source_record_count: 0,
        };
    }

    let mut min_latitude = f64::INFINITY;
    let mut max_latitude = f64::NEG_INFINITY;
    let mut min_longitude = f64::INFINITY;
    let mut max_longitude = f64::NEG_INFINITY;
    let mut altitude_sum = 0.0;
    for point in &gps_points {
        min_latitude = min_latitude.min(point.latitude);
        max_latitude = max_latitude.max(point.latitude);
        min_longitude = min_longitude.min(point.longitude);
        max_longitude = max_longitude.max(point.longitude);
        altitude_sum += point.altitude;
    }

    let pad = 0.0001 + f64::from(area_fraction.clamp(0.0, 1.0)) * 0.001;
    let altitude = altitude_sum / gps_points.len() as f64;
    let coordinates = vec![
        GpsCoords {
            latitude: min_latitude - pad,
            longitude: min_longitude - pad,
            altitude,
        },
        GpsCoords {
            latitude: min_latitude - pad,
            longitude: max_longitude + pad,
            altitude,
        },
        GpsCoords {
            latitude: max_latitude + pad,
            longitude: max_longitude + pad,
            altitude,
        },
        GpsCoords {
            latitude: max_latitude + pad,
            longitude: min_longitude - pad,
            altitude,
        },
        GpsCoords {
            latitude: min_latitude - pad,
            longitude: min_longitude - pad,
            altitude,
        },
    ];

    ReflyGapGeometry {
        field_id,
        geometry_type: "geo_bbox".to_string(),
        coordinates,
        area_fraction,
        source_record_count: gps_points.len() as u32,
    }
}

pub fn analyze_capture_refly(
    session: &FlightSession,
    records: &[FlightDataRecord],
) -> CaptureReflyRecommendation {
    let quality_masked_count = session
        .summary
        .collection_failures
        .iter()
        .filter(|failure| failure.kind == CollectionFailureKind::QualityMasked)
        .count() as u32;

    let has_coverage_gap = matches!(
        session.summary.coverage.status,
        CaptureCoverageStatus::Partial
    ) || session.summary.coverage.failed_observations > 0
        || (session.summary.coverage.expected_observations > 0
            && session.summary.coverage.captured_fraction < 0.999);
    let has_quality_gap = quality_masked_count > 0;

    let mut reasons = Vec::new();
    if has_coverage_gap {
        reasons.push(ReflyGapReason::CoverageGap);
    }
    if has_quality_gap {
        reasons.push(ReflyGapReason::LowQualityMask);
    }

    let expected = session.summary.coverage.expected_observations.max(1);
    let uncovered_fraction = if session.summary.coverage.expected_observations == 0 {
        0.0
    } else {
        (1.0 - session.summary.coverage.captured_fraction).clamp(0.0, 1.0)
    };
    let quality_fraction = quality_masked_count as f32 / expected as f32;
    let area_fraction = uncovered_fraction.max(quality_fraction).clamp(0.0, 1.0);
    let gap_geometry = if reasons.is_empty() {
        None
    } else {
        Some(gap_geometry_for(session.field_id, records, area_fraction))
    };

    CaptureReflyRecommendation {
        session_id: session.id,
        field_id: session.field_id,
        linked_mission_id: session.mission_id,
        recommended: !reasons.is_empty() && session.mission_id.is_some(),
        advisory_only: true,
        gate: if session.mission_id.is_some() {
            "advisory_mission_linked".to_string()
        } else {
            "mission_link_required".to_string()
        },
        reasons,
        gap_geometry,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSessionInspection {
    pub session: CaptureSessionListItem,
    pub summary: SessionSummary,
    pub failures: Vec<CollectionFailure>,
}

/// Main data collector service
pub struct DataCollectorService {
    storage: StorageEngine,
    active_sessions: HashMap<Uuid, FlightSession>,
    linkage_catalog: CaptureLinkageCatalog,
    indexer: DataIndexer,
    auto_export: bool,
    retention_days: u32,
}

impl DataCollectorService {
    pub fn new(data_root: PathBuf) -> Result<Self> {
        let storage_config = StorageConfig {
            base_path: data_root.clone(),
            max_file_size_mb: 100,
            compression_enabled: true,
            encryption_enabled: false,
            backup_enabled: false,
            retention_days: 365,
        };
        let storage = StorageEngine::new(storage_config)?;

        let index_config = IndexConfig {
            index_path: data_root.join("index"),
            spatial_resolution: 100.0,
            temporal_resolution_hours: 1,
            enable_spatial_index: true,
            enable_temporal_index: true,
            enable_type_index: true,
            spatial_grid_size: 0.001,
            temporal_bucket_size: chrono::Duration::minutes(5),
        };
        let indexer = DataIndexer::new(index_config);

        Ok(Self {
            storage,
            active_sessions: HashMap::new(),
            linkage_catalog: CaptureLinkageCatalog::default(),
            indexer,
            auto_export: false,
            retention_days: 365,
        })
    }

    pub fn register_capture_linkage(
        &mut self,
        reference: CaptureLinkageReference,
    ) -> std::result::Result<(), CaptureLinkageError> {
        self.linkage_catalog.register(reference)
    }

    pub fn validate_capture_linkage(
        &self,
        request: &CaptureSessionRequest,
    ) -> std::result::Result<(), CaptureLinkageError> {
        self.linkage_catalog
            .validate(CaptureLinkageReference::from_request(request))
    }

    pub async fn start_session(
        &mut self,
        drone_id: Uuid,
        mission_id: Option<Uuid>,
    ) -> Result<Uuid> {
        let flight_id = mission_id.unwrap_or_else(Uuid::nil);
        let mut request = CaptureSessionRequest::new(
            flight_id,
            Uuid::nil(),
            Uuid::nil(),
            drone_id,
            default_owner_id(),
        );
        request.mission_id = mission_id;

        self.register_capture_linkage(CaptureLinkageReference::from_request(&request))?;
        self.start_capture_session(request).await
    }

    pub async fn start_capture_session(&mut self, request: CaptureSessionRequest) -> Result<Uuid> {
        self.validate_capture_linkage(&request)?;
        let session = FlightSession::new(request);

        let session_id = session.id;

        self.storage.store_session(&session).await?;
        self.active_sessions.insert(session_id, session);

        tracing::info!("Started data collection session: {}", session_id);
        Ok(session_id)
    }

    pub async fn end_session(&mut self, session_id: &Uuid) -> Result<FlightSession> {
        let current_status = self
            .active_sessions
            .get(session_id)
            .ok_or(SessionLifecycleError::SessionNotFound {
                session_id: *session_id,
            })?
            .status;

        if !FlightSession::is_valid_transition(current_status, SessionStatus::Ended)
            && current_status != SessionStatus::Ended
        {
            return Err(SessionLifecycleError::InvalidStatusTransition {
                session_id: *session_id,
                from: current_status,
                to: SessionStatus::Ended,
            }
            .into());
        }

        let mut session = self.active_sessions.remove(session_id).ok_or(
            SessionLifecycleError::SessionNotFound {
                session_id: *session_id,
            },
        )?;
        session.transition_status(SessionStatus::Ended)?;
        session.end_time = Some(Utc::now());

        // Calculate final summary
        session.summary = self.calculate_session_summary(&session).await?;

        // Store session metadata
        self.storage.store_session(&session).await?;

        // Update index
        self.indexer.index_session(&session).await?;

        tracing::info!("Ended data collection session: {}", session_id);
        Ok(session)
    }

    pub async fn fail_session(&mut self, session_id: &Uuid) -> Result<FlightSession> {
        let current_status = self
            .active_sessions
            .get(session_id)
            .ok_or(SessionLifecycleError::SessionNotFound {
                session_id: *session_id,
            })?
            .status;

        if !FlightSession::is_valid_transition(current_status, SessionStatus::Failed)
            && current_status != SessionStatus::Failed
        {
            return Err(SessionLifecycleError::InvalidStatusTransition {
                session_id: *session_id,
                from: current_status,
                to: SessionStatus::Failed,
            }
            .into());
        }

        let mut session = self.active_sessions.remove(session_id).ok_or(
            SessionLifecycleError::SessionNotFound {
                session_id: *session_id,
            },
        )?;
        session.transition_status(SessionStatus::Failed)?;
        session.end_time = Some(Utc::now());

        self.storage.store_session(&session).await?;

        tracing::warn!("Failed data collection session: {}", session_id);
        Ok(session)
    }

    pub async fn record_collection_failure(
        &mut self,
        session_id: &Uuid,
        failure: CollectionFailureRequest,
    ) -> Result<CollectionFailure> {
        let failure = failure.into_failure();

        let session_snapshot = {
            let session = self.active_sessions.get_mut(session_id).ok_or(
                SessionLifecycleError::SessionNotFound {
                    session_id: *session_id,
                },
            )?;

            match session.status {
                SessionStatus::Started => session.transition_status(SessionStatus::Collecting)?,
                SessionStatus::Collecting => {}
                SessionStatus::Ended | SessionStatus::Failed => {
                    return Err(SessionLifecycleError::InvalidStatusTransition {
                        session_id: *session_id,
                        from: session.status,
                        to: SessionStatus::Collecting,
                    }
                    .into());
                }
            }

            session.record_collection_failure(failure.clone(), Utc::now());
            session.clone()
        };

        self.storage.store_session(&session_snapshot).await?;

        Ok(failure)
    }

    pub async fn collect_reader_capture_with_retry(
        &mut self,
        session_id: &Uuid,
        policy: RetryBackoffPolicy,
        attempts: impl IntoIterator<Item = std::result::Result<FlightDataRecord, CaptureReaderError>>,
    ) -> Result<CaptureRetryReport> {
        let mut attempt_count = 0u32;
        let mut backoff_schedule_ms = Vec::new();
        let mut saw_error = false;

        for attempt in attempts {
            attempt_count += 1;
            match attempt {
                Ok(record) => {
                    self.collect_data(session_id, record).await?;
                    return Ok(CaptureRetryReport {
                        attempts: attempt_count,
                        backoff_schedule_ms,
                        recovered: saw_error,
                        failure: None,
                        health: self.capture_health_snapshot(session_id).await?,
                    });
                }
                Err(error) => {
                    saw_error = true;
                    let retry_index = backoff_schedule_ms.len() as u32;
                    let can_retry = retry_index < policy.max_retries;
                    let retry_backoff_ms =
                        can_retry.then(|| policy.backoff_ms_for_retry(retry_index));

                    self.record_capture_reader_error(
                        session_id,
                        &error,
                        retry_backoff_ms,
                        !can_retry,
                    )
                    .await?;

                    if let Some(backoff_ms) = retry_backoff_ms {
                        backoff_schedule_ms.push(backoff_ms);
                    } else {
                        let failure = self
                            .record_collection_failure(
                                session_id,
                                CollectionFailureRequest {
                                    occurred_at: Some(error.occurred_at),
                                    sensor_id: error.sensor_id,
                                    data_type: error.data_type,
                                    kind: CollectionFailureKind::ReaderError,
                                    message: format!(
                                        "retry bound exhausted after {attempt_count} attempts: {}",
                                        error.message
                                    ),
                                },
                            )
                            .await?;

                        return Ok(CaptureRetryReport {
                            attempts: attempt_count,
                            backoff_schedule_ms,
                            recovered: false,
                            failure: Some(failure),
                            health: self.capture_health_snapshot(session_id).await?,
                        });
                    }
                }
            }
        }

        if saw_error {
            anyhow::bail!("capture reader attempts ended before retry recovery or exhaustion");
        }

        anyhow::bail!("capture retry requires at least one reader attempt");
    }

    async fn record_capture_reader_error(
        &mut self,
        session_id: &Uuid,
        error: &CaptureReaderError,
        retry_backoff_ms: Option<u64>,
        exhausted: bool,
    ) -> Result<CaptureHealth> {
        let session_snapshot = {
            let session = self.active_sessions.get_mut(session_id).ok_or(
                SessionLifecycleError::SessionNotFound {
                    session_id: *session_id,
                },
            )?;

            match session.status {
                SessionStatus::Started => session.transition_status(SessionStatus::Collecting)?,
                SessionStatus::Collecting => {}
                SessionStatus::Ended | SessionStatus::Failed => {
                    return Err(SessionLifecycleError::InvalidStatusTransition {
                        session_id: *session_id,
                        from: session.status,
                        to: SessionStatus::Collecting,
                    }
                    .into());
                }
            }

            session.record_capture_reader_error(error, retry_backoff_ms, exhausted);
            session.clone()
        };

        self.storage.store_session(&session_snapshot).await?;
        Ok(session_snapshot.summary.capture_health)
    }

    async fn capture_health_snapshot(&self, session_id: &Uuid) -> Result<CaptureHealth> {
        self.get_session(session_id)
            .await?
            .map(|session| session.summary.capture_health)
            .ok_or_else(|| {
                SessionLifecycleError::SessionNotFound {
                    session_id: *session_id,
                }
                .into()
            })
    }

    pub async fn collect_data(&mut self, session_id: &Uuid, data: FlightDataRecord) -> Result<()> {
        data.validate_provenance()?;
        if data.session_id != *session_id {
            return Err(FlightDataProvenanceError::SessionMismatch {
                expected_session_id: *session_id,
                record_session_id: data.session_id,
            }
            .into());
        }

        let current_status = self
            .active_sessions
            .get(session_id)
            .ok_or(SessionLifecycleError::SessionNotFound {
                session_id: *session_id,
            })?
            .status;

        let next_status = match current_status {
            SessionStatus::Started => SessionStatus::Collecting,
            SessionStatus::Collecting => SessionStatus::Collecting,
            _ => {
                return Err(SessionLifecycleError::InvalidStatusTransition {
                    session_id: *session_id,
                    from: current_status,
                    to: SessionStatus::Collecting,
                }
                .into());
            }
        };

        // Store the data
        let stored_data = self.storage.store_data(&data).await?;

        // Update session
        let session_snapshot = {
            let session = self.active_sessions.get_mut(session_id).ok_or(
                SessionLifecycleError::SessionNotFound {
                    session_id: *session_id,
                },
            )?;
            session.transition_status(next_status)?;
            session.data_records.push(stored_data.id);
            session.summary.record_count += 1;
            session.summary.total_data_size_bytes += stored_data.size_bytes;

            // Update data type counts
            *session
                .summary
                .data_types
                .entry(stored_data.data_type.clone())
                .or_insert(0) += 1;
            if let Some(reason) = qa_mask_reason(&stored_data) {
                session.record_collection_failure(
                    CollectionFailure {
                        id: Uuid::new_v4(),
                        occurred_at: stored_data.timestamp,
                        sensor_id: stored_data.sensor_id.clone(),
                        data_type: stored_data.data_type.clone(),
                        kind: CollectionFailureKind::QualityMasked,
                        message: format!("QA masked record {}: {}", stored_data.id, reason),
                    },
                    Utc::now(),
                );
                let failed_observations = session.summary.collection_failures.len() as u32;
                let successful_records = session.summary.record_count.saturating_sub(1);
                session.summary.coverage =
                    CaptureCoverage::from_counts(successful_records, failed_observations);
            } else {
                session.record_successful_capture(&stored_data, Utc::now());
            }
            session.clone()
        };

        self.storage.store_session(&session_snapshot).await?;

        // Update index
        self.indexer.index_record(&stored_data);

        Ok(())
    }

    pub async fn collect_simulated_capture_frame(
        &mut self,
        frame: SimulatedCaptureFrame,
    ) -> Result<SimulatedCaptureBatch> {
        let session_id = frame.session_id;
        let batch = simulated_capture_frame_to_batch(frame)?;

        for record in batch.records.iter().cloned() {
            self.collect_data(&session_id, record).await?;
        }

        for failure in batch.failures.iter().cloned() {
            self.record_collection_failure(&session_id, failure).await?;
        }

        Ok(batch)
    }

    pub async fn collect_simulated_capture_path(
        &mut self,
        path: SimulatedCapturePath,
    ) -> Result<SimulatedCaptureBatch> {
        let mut aggregate = SimulatedCaptureBatch {
            records: Vec::new(),
            failures: Vec::new(),
        };

        for step in path.steps {
            match step {
                SimulatedCapturePathStep::Fix {
                    observed_at,
                    position,
                    observations,
                } => {
                    let frame = SimulatedCaptureFrame {
                        session_id: path.session_id,
                        flight_id: path.flight_id,
                        drone_id: path.drone_id,
                        simulation_mission_id: path.simulation_mission_id,
                        observed_at,
                        position,
                        observations,
                    };
                    let batch = self.collect_simulated_capture_frame(frame).await?;
                    aggregate.records.extend(batch.records);
                    aggregate.failures.extend(batch.failures);
                }
                SimulatedCapturePathStep::Gap {
                    started_at,
                    ended_at,
                    sensor_id,
                    data_type,
                    message,
                } => {
                    let failure = CollectionFailureRequest {
                        occurred_at: Some(started_at),
                        sensor_id,
                        data_type,
                        kind: CollectionFailureKind::SensorDropout,
                        message: format!("{message}; gap_end={}", ended_at.to_rfc3339()),
                    };
                    self.record_collection_failure(&path.session_id, failure.clone())
                        .await?;
                    aggregate.failures.push(failure);
                }
            }
        }

        Ok(aggregate)
    }

    pub async fn get_session(&self, session_id: &Uuid) -> Result<Option<FlightSession>> {
        if let Some(session) = self.active_sessions.get(session_id) {
            Ok(Some(session.clone()))
        } else {
            self.storage.load_session(session_id).await
        }
    }

    pub async fn list_sessions(
        &self,
        drone_id: Option<Uuid>,
        limit: Option<u32>,
    ) -> Result<Vec<FlightSession>> {
        let mut sessions = self.storage.list_sessions(drone_id, limit).await?;

        // Add active sessions
        for session in self.active_sessions.values() {
            if drone_id.is_none() || drone_id == Some(session.drone_id) {
                sessions.push(session.clone());
            }
        }

        // Sort by start time (newest first)
        sessions.sort_by(|a, b| b.start_time.cmp(&a.start_time));

        Ok(sessions)
    }

    pub async fn list_capture_sessions(
        &self,
        filter: CaptureSessionListFilter,
    ) -> Result<CaptureSessionListPage> {
        let mut sessions = self.sessions_for_inspection().await?;
        sessions.retain(|session| filter.matches(session));

        let total_count = sessions.len();
        let offset = filter.offset.min(total_count);
        let limit = filter.normalized_limit();
        let items = sessions
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|session| capture_session_list_item(&session))
            .collect::<Vec<_>>();
        let has_more = offset + items.len() < total_count;

        Ok(CaptureSessionListPage {
            items,
            total_count,
            offset,
            limit,
            has_more,
        })
    }

    pub async fn inspect_capture_session(
        &self,
        session_id: &Uuid,
    ) -> Result<CaptureSessionInspection> {
        let session =
            self.get_session(session_id)
                .await?
                .ok_or(SessionLifecycleError::SessionNotFound {
                    session_id: *session_id,
                })?;

        Ok(CaptureSessionInspection {
            session: capture_session_list_item(&session),
            summary: session.summary.clone(),
            failures: session.summary.collection_failures.clone(),
        })
    }

    pub async fn recommend_capture_refly(
        &self,
        session_id: &Uuid,
    ) -> Result<CaptureReflyRecommendation> {
        let session =
            self.get_session(session_id)
                .await?
                .ok_or(SessionLifecycleError::SessionNotFound {
                    session_id: *session_id,
                })?;
        let mut records = Vec::new();
        for record_id in &session.data_records {
            if let Some(record) = self.storage.load_data(record_id).await? {
                records.push(record);
            }
        }

        Ok(analyze_capture_refly(&session, &records))
    }

    async fn sessions_for_inspection(&self) -> Result<Vec<FlightSession>> {
        let mut sessions_by_id = self
            .storage
            .list_sessions(None, None)
            .await?
            .into_iter()
            .map(|session| (session.id, session))
            .collect::<HashMap<_, _>>();

        for session in self.active_sessions.values() {
            sessions_by_id.insert(session.id, session.clone());
        }

        let mut sessions = sessions_by_id.into_values().collect::<Vec<_>>();
        sessions.sort_by(|left, right| {
            right
                .start_time
                .cmp(&left.start_time)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(sessions)
    }

    pub async fn search_data(&self, query: SearchQuery) -> Result<Vec<FlightDataRecord>> {
        let persisted_records = self.storage.load_all_data().await?;
        if persisted_records.is_empty() {
            return self.indexer.search(query).await;
        }

        Ok(DataIndexer::filter_records(&persisted_records, &query))
    }

    pub async fn export_session(
        &self,
        session_id: &Uuid,
        format: ExportFormat,
        output_path: &Path,
    ) -> Result<()> {
        let session = self
            .get_session(session_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        let export_config = export::ExportConfig {
            format,
            include_metadata: true,
            compress: false,
        };
        let exporter = DataExporter::new(export_config);

        let mut session_records = Vec::with_capacity(session.data_records.len());
        for record_id in &session.data_records {
            let record = self.storage.load_data(record_id).await?.ok_or_else(|| {
                anyhow::anyhow!(
                    "record {} listed in session {} was not found",
                    record_id,
                    session.id
                )
            })?;
            session_records.push(record);
        }

        match exporter
            .export_session(&session, &session_records, output_path)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Export failed: {}", e)),
        }
    }

    pub async fn cleanup_old_data(&mut self) -> Result<u32> {
        let cutoff_date = Utc::now() - chrono::Duration::days(self.retention_days as i64);
        let removed_count = self.storage.cleanup_before_date(cutoff_date).await?;

        // Rebuild index after cleanup
        let persisted_records = self.storage.load_all_data().await?;
        self.indexer
            .rebuild_from_records(&persisted_records)
            .await?;

        tracing::info!("Cleaned up {} old data records", removed_count);
        Ok(removed_count as u32)
    }

    pub async fn get_storage_stats(&self) -> Result<StorageStats> {
        self.storage.get_stats().await
    }

    async fn calculate_session_summary(&self, session: &FlightSession) -> Result<SessionSummary> {
        let mut summary = SessionSummary {
            freshness: session.summary.freshness.clone(),
            coverage: session.summary.coverage.clone(),
            collection_failures: session.summary.collection_failures.clone(),
            capture_health: session.summary.capture_health.clone(),
            ..SessionSummary::default()
        };
        let mut loaded_record_count = 0u32;

        // Calculate from stored records
        for record_id in &session.data_records {
            if let Some(record) = self.storage.load_data(record_id).await? {
                loaded_record_count += 1;
                summary.record_count += 1;
                summary.total_data_size_bytes += record.size_bytes;
                *summary.data_types.entry(record.data_type).or_insert(0) += 1;
            }
        }

        if loaded_record_count == 0 {
            summary.record_count = session.summary.record_count;
            summary.total_data_size_bytes = session.summary.total_data_size_bytes;
            summary.data_types = session.summary.data_types.clone();
        }

        // Calculate flight metrics from telemetry data
        let telemetry_records = self.get_telemetry_for_session(session).await?;
        if !telemetry_records.is_empty() {
            summary.flight_duration_seconds = self.calculate_flight_duration(&telemetry_records);
            summary.distance_covered_m = self.calculate_distance_covered(&telemetry_records);
            summary.area_covered_m2 = self.calculate_area_covered(&telemetry_records);
            summary.battery_consumed_percent =
                self.calculate_battery_consumption(&telemetry_records);
            summary.aggregate_evidence = SessionAggregateEvidence {
                status: SessionAggregateStatus::FromTelemetryTrack,
                telemetry_record_ids: telemetry_record_ids(&telemetry_records),
                sample_count: telemetry_records.len(),
            };
        } else {
            summary.aggregate_evidence = SessionAggregateEvidence::default();
        }

        Ok(summary)
    }

    async fn get_telemetry_for_session(
        &self,
        session: &FlightSession,
    ) -> Result<Vec<FlightDataRecord>> {
        let mut records = Vec::new();
        for record_id in &session.data_records {
            if let Some(record) = self.storage.load_data(record_id).await? {
                if record.data_type == DataType::Telemetry
                    && matches!(record.payload, DataPayload::Telemetry { .. })
                {
                    records.push(record);
                }
            }
        }
        records.sort_by(|left, right| {
            left.timestamp
                .cmp(&right.timestamp)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(records)
    }

    fn calculate_flight_duration(&self, records: &[FlightDataRecord]) -> f32 {
        let Some(first) = records.first() else {
            return 0.0;
        };
        let Some(last) = records.last() else {
            return 0.0;
        };
        last.timestamp
            .signed_duration_since(first.timestamp)
            .num_milliseconds()
            .max(0) as f32
            / 1000.0
    }

    fn calculate_distance_covered(&self, records: &[FlightDataRecord]) -> f32 {
        telemetry_samples(records)
            .windows(2)
            .map(|pair| distance_between_samples(pair[0], pair[1]))
            .sum::<f64>() as f32
    }

    fn calculate_area_covered(&self, records: &[FlightDataRecord]) -> f32 {
        let samples = telemetry_samples(records);
        if samples.len() < 3 {
            return 0.0;
        }
        let points = local_track_points(&samples);
        polygon_area_m2(&convex_hull(points)) as f32
    }

    fn calculate_battery_consumption(&self, records: &[FlightDataRecord]) -> f32 {
        let samples = telemetry_samples(records);
        let Some(first) = samples.first() else {
            return 0.0;
        };
        let Some(last) = samples.last() else {
            return 0.0;
        };
        ((first.battery_level - last.battery_level).max(0.0) * 100.0) as f32
    }
}

fn capture_session_list_item(session: &FlightSession) -> CaptureSessionListItem {
    CaptureSessionListItem {
        session_id: session.id,
        flight_id: session.flight_id,
        field_id: session.field_id,
        scene_id: session.scene_id,
        drone_id: session.drone_id,
        status: session.status,
        started_at: session.start_time,
        ended_at: session.end_time,
        record_count: session.summary.record_count,
        failure_count: session.summary.collection_failures.len() as u32,
        freshness: session.summary.freshness.clone(),
        coverage: session.summary.coverage.clone(),
        aggregate_evidence: session.summary.aggregate_evidence.clone(),
        capture_health: session.summary.capture_health.clone(),
        qa: CaptureQaSummary::from_summary(&session.summary),
    }
}

#[derive(Debug, Clone, Copy)]
struct TelemetryAggregateSample {
    latitude: f64,
    longitude: f64,
    altitude_m: f64,
    battery_level: f64,
}

fn telemetry_record_ids(records: &[FlightDataRecord]) -> Vec<Uuid> {
    records.iter().map(|record| record.id).collect()
}

fn telemetry_samples(records: &[FlightDataRecord]) -> Vec<TelemetryAggregateSample> {
    records.iter().filter_map(telemetry_sample).collect()
}

fn telemetry_sample(record: &FlightDataRecord) -> Option<TelemetryAggregateSample> {
    match record.payload {
        DataPayload::Telemetry {
            position,
            battery_level,
            ..
        } => Some(TelemetryAggregateSample {
            latitude: position.0,
            longitude: position.1,
            altitude_m: f64::from(position.2),
            battery_level: f64::from(battery_level),
        }),
        _ => None,
    }
}

fn distance_between_samples(
    left: TelemetryAggregateSample,
    right: TelemetryAggregateSample,
) -> f64 {
    let horizontal = haversine_distance_m(
        left.latitude,
        left.longitude,
        right.latitude,
        right.longitude,
    );
    let altitude_delta = right.altitude_m - left.altitude_m;
    horizontal.hypot(altitude_delta)
}

fn haversine_distance_m(
    left_latitude: f64,
    left_longitude: f64,
    right_latitude: f64,
    right_longitude: f64,
) -> f64 {
    let radius_m = 6_371_000.0;
    let left_lat = left_latitude.to_radians();
    let right_lat = right_latitude.to_radians();
    let delta_lat = (right_latitude - left_latitude).to_radians();
    let delta_lon = (right_longitude - left_longitude).to_radians();
    let a = (delta_lat / 2.0).sin().powi(2)
        + left_lat.cos() * right_lat.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    radius_m * c
}

fn local_track_points(samples: &[TelemetryAggregateSample]) -> Vec<(f64, f64)> {
    let Some(origin) = samples.first() else {
        return Vec::new();
    };
    let meters_per_degree_lat = 111_320.0;
    let meters_per_degree_lon = meters_per_degree_lat * origin.latitude.to_radians().cos();
    samples
        .iter()
        .map(|sample| {
            (
                (sample.longitude - origin.longitude) * meters_per_degree_lon,
                (sample.latitude - origin.latitude) * meters_per_degree_lat,
            )
        })
        .collect()
}

fn convex_hull(mut points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    if points.len() <= 2 {
        return points;
    }
    points.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.1
                    .partial_cmp(&right.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    points.dedup_by(|left, right| {
        (left.0 - right.0).abs() <= f64::EPSILON && (left.1 - right.1).abs() <= f64::EPSILON
    });

    let mut lower = Vec::new();
    for point in &points {
        while lower.len() >= 2
            && cross(lower[lower.len() - 2], lower[lower.len() - 1], *point) <= 0.0
        {
            lower.pop();
        }
        lower.push(*point);
    }

    let mut upper = Vec::new();
    for point in points.iter().rev() {
        while upper.len() >= 2
            && cross(upper[upper.len() - 2], upper[upper.len() - 1], *point) <= 0.0
        {
            upper.pop();
        }
        upper.push(*point);
    }

    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn cross(origin: (f64, f64), left: (f64, f64), right: (f64, f64)) -> f64 {
    (left.0 - origin.0) * (right.1 - origin.1) - (left.1 - origin.1) * (right.0 - origin.0)
}

fn polygon_area_m2(points: &[(f64, f64)]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let twice_area = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(left, right)| left.0 * right.1 - right.0 * left.1)
        .sum::<f64>();
    (twice_area / 2.0).abs()
}

impl Default for SessionSummary {
    fn default() -> Self {
        Self {
            total_data_size_bytes: 0,
            record_count: 0,
            data_types: HashMap::new(),
            flight_duration_seconds: 0.0,
            distance_covered_m: 0.0,
            area_covered_m2: 0.0,
            battery_consumed_percent: 0.0,
            freshness: CaptureFreshness::default(),
            coverage: CaptureCoverage::default(),
            collection_failures: Vec::new(),
            capture_health: CaptureHealth::default(),
            aggregate_evidence: SessionAggregateEvidence::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_size_bytes: u64,
    pub total_records: u32,
    pub sessions_count: u32,
    pub oldest_record: Option<DateTime<Utc>>,
    pub newest_record: Option<DateTime<Utc>>,
    pub data_type_breakdown: HashMap<DataType, u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::tempdir;

    fn capture_request() -> CaptureSessionRequest {
        CaptureSessionRequest::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "grower-ops".to_string(),
        )
    }

    fn gps_coords() -> GpsCoords {
        GpsCoords {
            latitude: 40.0,
            longitude: -105.0,
            altitude: 30.0,
        }
    }

    fn gps_coords_at(latitude: f64, longitude: f64, altitude: f64) -> GpsCoords {
        GpsCoords {
            latitude,
            longitude,
            altitude,
        }
    }

    fn provenance(session: &FlightSession) -> FlightDataProvenance {
        FlightDataProvenance::complete(
            session.id,
            "sensor-rgb-01".to_string(),
            gps_coords(),
            Utc::now(),
            "calibration-2026-06".to_string(),
        )
    }

    fn telemetry_record(session: &FlightSession) -> FlightDataRecord {
        FlightDataRecord::new(
            session.flight_id,
            session.drone_id,
            DataType::Telemetry,
            DataPayload::Telemetry {
                position: (40.0, -105.0, 30.0),
                velocity: (1.0, 0.0, 0.0),
                orientation: (0.0, 0.0, 0.0),
                battery_level: 0.9,
                signal_strength: 0.95,
            },
            provenance(session),
            256,
        )
        .unwrap()
    }

    fn telemetry_record_at(
        session: &FlightSession,
        timestamp: DateTime<Utc>,
        latitude: f64,
        longitude: f64,
        battery_level: f32,
    ) -> FlightDataRecord {
        FlightDataRecord::new(
            session.flight_id,
            session.drone_id,
            DataType::Telemetry,
            DataPayload::Telemetry {
                position: (latitude, longitude, 30.0),
                velocity: (1.0, 0.0, 0.0),
                orientation: (0.0, 0.0, 0.0),
                battery_level,
                signal_strength: 0.95,
            },
            FlightDataProvenance::complete(
                session.id,
                "telemetry-track-01".to_string(),
                gps_coords_at(latitude, longitude, 30.0),
                timestamp,
                "telemetry-calibration-v1".to_string(),
            ),
            256,
        )
        .unwrap()
    }

    fn sparse_point_cloud_record(session: &FlightSession) -> FlightDataRecord {
        FlightDataRecord::new(
            session.flight_id,
            session.drone_id,
            DataType::LidarScan,
            DataPayload::PointCloud {
                point_count: 1,
                bounds: ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)),
                format: "ply".to_string(),
                has_color: false,
                has_intensity: true,
            },
            FlightDataProvenance::complete(
                session.id,
                "lidar-qa-01".to_string(),
                gps_coords(),
                Utc::now(),
                "lidar-calibration-v1".to_string(),
            ),
            128,
        )
        .unwrap()
    }

    fn all_data_types() -> Vec<DataType> {
        vec![
            DataType::Telemetry,
            DataType::SensorReading,
            DataType::Image,
            DataType::Video,
            DataType::LidarScan,
            DataType::MultispectralImage,
            DataType::ThermalImage,
            DataType::GPSTrack,
            DataType::FlightLog,
            DataType::MissionPlan,
            DataType::WeatherData,
            DataType::SystemLog,
        ]
    }

    fn all_payloads() -> Vec<DataPayload> {
        let now = Utc::now();
        vec![
            DataPayload::Telemetry {
                position: (40.0, -105.0, 30.0),
                velocity: (1.0, 0.0, 0.0),
                orientation: (0.0, 0.0, 0.0),
                battery_level: 0.9,
                signal_strength: 0.95,
            },
            DataPayload::SensorData {
                sensor_type: "multispectral".to_string(),
                values: HashMap::from([("nir".to_string(), 0.72)]),
                calibration_info: Some("calibration-2026-06".to_string()),
            },
            DataPayload::MediaFile {
                file_type: "image/tiff".to_string(),
                dimensions: Some((1024, 1024)),
                duration_seconds: None,
                compression: Some("none".to_string()),
            },
            DataPayload::PointCloud {
                point_count: 42,
                bounds: ((0.0, 0.0, 0.0), (1.0, 1.0, 1.0)),
                format: "ply".to_string(),
                has_color: false,
                has_intensity: true,
            },
            DataPayload::TrackLog {
                waypoint_count: 2,
                total_distance_m: 10.0,
                duration_seconds: 5.0,
                start_time: now,
                end_time: now + chrono::Duration::seconds(5),
            },
            DataPayload::Raw {
                format: "json".to_string(),
                schema: Some("agbot.raw.v1".to_string()),
                compression: None,
            },
        ]
    }

    fn simulated_capture_frame(session: &FlightSession) -> SimulatedCaptureFrame {
        SimulatedCaptureFrame {
            session_id: session.id,
            flight_id: session.flight_id,
            drone_id: session.drone_id,
            simulation_mission_id: session.mission_id.unwrap_or(session.flight_id),
            observed_at: Utc::now(),
            position: gps_coords(),
            observations: vec![
                SimulatedSensorObservation::Telemetry {
                    sensor_id: "sim-telemetry".to_string(),
                    calibration_ref: "sim-telemetry-v1".to_string(),
                    velocity: (1.0, 0.0, 0.0),
                    orientation: (0.0, 0.0, 0.0),
                    battery_level: 0.88,
                    signal_strength: 0.99,
                },
                SimulatedSensorObservation::Failure {
                    sensor_id: "sim-multispectral".to_string(),
                    data_type: DataType::MultispectralImage,
                    kind: CollectionFailureKind::SensorDropout,
                    message: "simulated sensor dropout".to_string(),
                },
            ],
        }
    }

    fn telemetry_observation(sensor_id: &str) -> SimulatedSensorObservation {
        SimulatedSensorObservation::Telemetry {
            sensor_id: sensor_id.to_string(),
            calibration_ref: "sim-telemetry-v1".to_string(),
            velocity: (1.0, 0.0, 0.0),
            orientation: (0.0, 0.0, 0.0),
            battery_level: 0.88,
            signal_strength: 0.99,
        }
    }

    async fn start_linked_capture_session(
        service: &mut DataCollectorService,
        request: CaptureSessionRequest,
    ) -> Uuid {
        service
            .register_capture_linkage(CaptureLinkageReference::from_request(&request))
            .unwrap();
        service.start_capture_session(request).await.unwrap()
    }

    #[tokio::test]
    async fn test_service_creation() {
        let temp_dir = tempdir().unwrap();
        let service = DataCollectorService::new(temp_dir.path().to_path_buf());
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_session_lifecycle() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();

        let request = capture_request();
        let drone_id = request.drone_id;
        let session_id = start_linked_capture_session(&mut service, request).await;

        let session = service.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.drone_id, drone_id);
        assert_eq!(session.status, SessionStatus::Started);

        let ended_session = service.end_session(&session_id).await.unwrap();
        assert_eq!(ended_session.status, SessionStatus::Ended);
        assert!(ended_session.end_time.is_some());
    }

    #[tokio::test]
    async fn test_start_capture_session_persists_linkage_identity() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let request = capture_request();
        let flight_id = request.flight_id;
        let field_id = request.field_id;
        let scene_id = request.scene_id;
        let owner_id = request.owner_id.clone();

        let session_id = start_linked_capture_session(&mut service, request).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();

        assert_eq!(session.id, session_id);
        assert_eq!(session.flight_id, flight_id);
        assert_eq!(session.field_id, field_id);
        assert_eq!(session.scene_id, scene_id);
        assert_eq!(session.owner_id, owner_id);
        assert_eq!(session.status, SessionStatus::Started);
        assert!(session.end_time.is_none());

        let stored_path = temp_dir
            .path()
            .join("sessions")
            .join(session_id.to_string())
            .join("session.json");
        let stored_json = tokio::fs::read(&stored_path).await.unwrap();
        let stored_session: FlightSession = serde_json::from_slice(&stored_json).unwrap();
        assert_eq!(stored_session.flight_id, flight_id);
        assert_eq!(stored_session.status, SessionStatus::Started);
    }

    #[tokio::test]
    async fn test_start_capture_session_validates_registered_linkage() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let request = capture_request();
        let expected_linkage = CaptureLinkageReference::from_request(&request);

        service
            .register_capture_linkage(expected_linkage)
            .expect("fixture linkage is registered");
        let session_id = service.start_capture_session(request).await.unwrap();
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let record = telemetry_record(&session);

        service.collect_data(&session_id, record).await.unwrap();

        let session = service.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.status, SessionStatus::Collecting);
        assert_eq!(session.flight_id, expected_linkage.flight_id);
        assert_eq!(session.field_id, expected_linkage.field_id);
        assert_eq!(session.scene_id, expected_linkage.scene_id);
    }

    #[tokio::test]
    async fn test_start_capture_session_rejects_unknown_flight_linkage() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let mut request = capture_request();
        service
            .register_capture_linkage(CaptureLinkageReference::from_request(&request))
            .unwrap();
        let unknown_flight_id = Uuid::new_v4();
        request.flight_id = unknown_flight_id;

        let err = service.start_capture_session(request).await.unwrap_err();
        let linkage_error = err.downcast_ref::<CaptureLinkageError>().unwrap();

        assert_eq!(
            linkage_error,
            &CaptureLinkageError::UnknownFlight {
                flight_id: unknown_flight_id
            }
        );
        assert!(service.list_sessions(None, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_collect_data_transitions_started_session_to_collecting() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();

        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let record = telemetry_record(&session);
        let record_id = record.id;
        let record_timestamp = record.timestamp;

        service.collect_data(&session_id, record).await.unwrap();

        let session = service.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.status, SessionStatus::Collecting);
        assert_eq!(session.summary.record_count, 1);
        assert_eq!(session.data_records.len(), 1);

        let stored_path = temp_dir
            .path()
            .join("records")
            .join(record_timestamp.format("%Y/%m/%d").to_string())
            .join(format!("telemetry_{}.json", record_id));
        let stored_json = tokio::fs::read(&stored_path).await.unwrap();
        let stored_record: FlightDataRecord = serde_json::from_slice(&stored_json).unwrap();
        assert_eq!(stored_record.session_id, session.id);
        assert_eq!(stored_record.sensor_id, "sensor-rgb-01");
        assert_eq!(stored_record.gps_coords, Some(gps_coords()));
        assert_eq!(stored_record.calibration_ref, "calibration-2026-06");
    }

    #[tokio::test]
    async fn session_summary_aggregates_are_derived_from_telemetry_track() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let base_time = Utc.timestamp_opt(1_800_000_000, 0).unwrap();
        let records = vec![
            telemetry_record_at(&session, base_time, 40.0, -105.0, 0.90),
            telemetry_record_at(
                &session,
                base_time + chrono::Duration::seconds(10),
                40.0,
                -104.999,
                0.82,
            ),
            telemetry_record_at(
                &session,
                base_time + chrono::Duration::seconds(20),
                40.001,
                -104.999,
                0.75,
            ),
        ];
        let expected_record_ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
        for record in records {
            service.collect_data(&session_id, record).await.unwrap();
        }

        let ended = service.end_session(&session_id).await.unwrap();

        assert_eq!(ended.summary.flight_duration_seconds, 20.0);
        assert!(ended.summary.distance_covered_m > 190.0);
        assert!(ended.summary.area_covered_m2 > 4_000.0);
        assert!((ended.summary.battery_consumed_percent - 15.0).abs() < 0.001);
        assert_eq!(
            ended.summary.aggregate_evidence.status,
            SessionAggregateStatus::FromTelemetryTrack
        );
        assert_eq!(ended.summary.aggregate_evidence.sample_count, 3);
        assert_eq!(
            ended.summary.aggregate_evidence.telemetry_record_ids,
            expected_record_ids
        );
    }

    #[tokio::test]
    async fn session_summary_without_telemetry_records_explicit_no_track() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;

        let ended = service.end_session(&session_id).await.unwrap();

        assert_eq!(ended.summary.flight_duration_seconds, 0.0);
        assert_eq!(ended.summary.distance_covered_m, 0.0);
        assert_eq!(ended.summary.area_covered_m2, 0.0);
        assert_eq!(ended.summary.battery_consumed_percent, 0.0);
        assert_eq!(
            ended.summary.aggregate_evidence.status,
            SessionAggregateStatus::NoTelemetryTrack
        );
        assert!(ended
            .summary
            .aggregate_evidence
            .telemetry_record_ids
            .is_empty());
    }

    #[tokio::test]
    async fn stored_record_checksum_verifies_and_detects_tamper() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let record = telemetry_record(&session);
        let record_id = record.id;
        let record_timestamp = record.timestamp;

        service.collect_data(&session_id, record).await.unwrap();

        let loaded = service
            .storage
            .load_data(&record_id)
            .await
            .unwrap()
            .unwrap();
        assert!(loaded.metadata.contains_key("integrity_checksum"));
        assert_eq!(
            loaded
                .metadata
                .get("integrity_verified")
                .map(String::as_str),
            Some("true")
        );

        let stored_path = temp_dir
            .path()
            .join("records")
            .join(record_timestamp.format("%Y/%m/%d").to_string())
            .join(format!("telemetry_{}.json", record_id));
        let mut tampered: FlightDataRecord =
            serde_json::from_slice(&tokio::fs::read(&stored_path).await.unwrap()).unwrap();
        tampered.size_bytes += 1;
        tokio::fs::write(&stored_path, serde_json::to_vec_pretty(&tampered).unwrap())
            .await
            .unwrap();

        let err = service.storage.load_data(&record_id).await.unwrap_err();
        assert!(err.to_string().contains("checksum mismatch"));
    }

    #[tokio::test]
    async fn sparse_point_cloud_is_qa_masked_and_excluded_from_coverage() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let record = sparse_point_cloud_record(&session);
        let record_id = record.id;

        service.collect_data(&session_id, record).await.unwrap();

        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let loaded = service
            .storage
            .load_data(&record_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.metadata.get("qa_masked").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            loaded.metadata.get("qa_reason").map(String::as_str),
            Some("sparse_point_cloud")
        );
        assert_eq!(session.summary.collection_failures.len(), 1);
        assert_eq!(session.summary.coverage.successful_records, 0);
        assert_eq!(session.summary.coverage.failed_observations, 1);
        assert_eq!(session.summary.coverage.captured_fraction, 0.0);
    }

    #[tokio::test]
    async fn capture_gap_recommends_advisory_refly_tied_to_field_and_mission() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let mission_id = Uuid::new_v4();
        let request = capture_request().with_mission_id(mission_id);
        let field_id = request.field_id;
        let session_id = start_linked_capture_session(&mut service, request).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let first = telemetry_record_at(
            &session,
            Utc.timestamp_opt(1_800_300_000, 0).unwrap(),
            40.0000,
            -105.0000,
            0.9,
        );
        let second = telemetry_record_at(
            &session,
            Utc.timestamp_opt(1_800_300_010, 0).unwrap(),
            40.0010,
            -104.9990,
            0.9,
        );

        service.collect_data(&session_id, first).await.unwrap();
        service.collect_data(&session_id, second).await.unwrap();
        service
            .record_collection_failure(
                &session_id,
                CollectionFailureRequest {
                    occurred_at: Some(Utc.timestamp_opt(1_800_300_020, 0).unwrap()),
                    sensor_id: "multispectral-front".to_string(),
                    data_type: DataType::MultispectralImage,
                    kind: CollectionFailureKind::QualityMasked,
                    message: "cloud_shadow_masked".to_string(),
                },
            )
            .await
            .unwrap();

        let recommendation = service.recommend_capture_refly(&session_id).await.unwrap();

        assert!(recommendation.recommended);
        assert!(recommendation.advisory_only);
        assert_eq!(recommendation.field_id, field_id);
        assert_eq!(recommendation.linked_mission_id, Some(mission_id));
        assert_eq!(recommendation.gate, "advisory_mission_linked");
        assert!(recommendation
            .reasons
            .contains(&ReflyGapReason::CoverageGap));
        assert!(recommendation
            .reasons
            .contains(&ReflyGapReason::LowQualityMask));
        let geometry = recommendation.gap_geometry.expect("gap geometry emitted");
        assert_eq!(geometry.field_id, field_id);
        assert_eq!(geometry.geometry_type, "geo_bbox");
        assert_eq!(geometry.coordinates.len(), 5);
        assert_eq!(geometry.coordinates.first(), geometry.coordinates.last());
        assert_eq!(geometry.source_record_count, 2);
        assert!((geometry.area_fraction - (1.0 / 3.0)).abs() < 0.001);
    }

    #[tokio::test]
    async fn full_high_quality_capture_does_not_recommend_refly() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(
            &mut service,
            capture_request().with_mission_id(Uuid::new_v4()),
        )
        .await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();

        service
            .collect_data(
                &session_id,
                telemetry_record_at(
                    &session,
                    Utc.timestamp_opt(1_800_300_000, 0).unwrap(),
                    40.0000,
                    -105.0000,
                    0.9,
                ),
            )
            .await
            .unwrap();

        let recommendation = service.recommend_capture_refly(&session_id).await.unwrap();

        assert!(!recommendation.recommended);
        assert!(recommendation.advisory_only);
        assert!(recommendation.reasons.is_empty());
        assert!(recommendation.gap_geometry.is_none());
    }

    #[tokio::test]
    async fn simulated_capture_frame_persists_records_and_failures() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();

        let request = capture_request().with_mission_id(Uuid::new_v4());
        let session_id = start_linked_capture_session(&mut service, request).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();

        let batch = service
            .collect_simulated_capture_frame(simulated_capture_frame(&session))
            .await
            .expect("simulated frame persists");

        assert_eq!(batch.records.len(), 1);
        assert_eq!(batch.failures.len(), 1);

        let stored_session = service.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(stored_session.status, SessionStatus::Collecting);
        assert_eq!(stored_session.summary.record_count, 1);
        assert_eq!(stored_session.summary.collection_failures.len(), 1);
        assert_eq!(
            stored_session.summary.coverage.status,
            CaptureCoverageStatus::Partial
        );
        assert_eq!(
            stored_session
                .summary
                .data_types
                .get(&DataType::Telemetry)
                .copied(),
            Some(1)
        );
    }

    #[tokio::test]
    async fn simulated_flight_path_georeferences_records_to_path_points() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();

        let request = capture_request().with_mission_id(Uuid::new_v4());
        let session_id = start_linked_capture_session(&mut service, request).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let first_time = Utc.timestamp_opt(1_800_100_000, 0).unwrap();
        let second_time = Utc.timestamp_opt(1_800_100_030, 0).unwrap();
        let first_position = gps_coords_at(37.0, -122.0, 101.0);
        let second_position = gps_coords_at(37.0002, -122.0004, 103.5);

        let batch = service
            .collect_simulated_capture_path(SimulatedCapturePath {
                session_id: session.id,
                flight_id: session.flight_id,
                drone_id: session.drone_id,
                simulation_mission_id: session.mission_id.unwrap(),
                steps: vec![
                    SimulatedCapturePathStep::Fix {
                        observed_at: first_time,
                        position: first_position.clone(),
                        observations: vec![telemetry_observation("sim-telemetry-front")],
                    },
                    SimulatedCapturePathStep::Fix {
                        observed_at: second_time,
                        position: second_position.clone(),
                        observations: vec![telemetry_observation("sim-telemetry-front")],
                    },
                ],
            })
            .await
            .expect("path capture persists");

        assert_eq!(batch.records.len(), 2);
        assert!(batch.failures.is_empty());
        assert_eq!(batch.records[0].timestamp, first_time);
        assert_eq!(batch.records[0].gps_coords, Some(first_position));
        assert_eq!(batch.records[1].timestamp, second_time);
        assert_eq!(batch.records[1].gps_coords, Some(second_position));
        assert!(batch
            .records
            .iter()
            .all(|record| record.validate_provenance().is_ok()));

        let stored_session = service.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(stored_session.summary.record_count, 2);
        assert_eq!(
            stored_session.summary.coverage.status,
            CaptureCoverageStatus::Complete
        );
    }

    #[tokio::test]
    async fn simulated_flight_path_gap_becomes_coverage_hole_without_interpolation() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();

        let request = capture_request().with_mission_id(Uuid::new_v4());
        let session_id = start_linked_capture_session(&mut service, request).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let first_time = Utc.timestamp_opt(1_800_200_000, 0).unwrap();
        let gap_start = Utc.timestamp_opt(1_800_200_010, 0).unwrap();
        let gap_end = Utc.timestamp_opt(1_800_200_020, 0).unwrap();
        let second_time = Utc.timestamp_opt(1_800_200_030, 0).unwrap();

        let batch = service
            .collect_simulated_capture_path(SimulatedCapturePath {
                session_id: session.id,
                flight_id: session.flight_id,
                drone_id: session.drone_id,
                simulation_mission_id: session.mission_id.unwrap(),
                steps: vec![
                    SimulatedCapturePathStep::Fix {
                        observed_at: first_time,
                        position: gps_coords_at(37.0, -122.0, 101.0),
                        observations: vec![telemetry_observation("sim-telemetry-front")],
                    },
                    SimulatedCapturePathStep::Gap {
                        started_at: gap_start,
                        ended_at: gap_end,
                        sensor_id: "sim-telemetry-front".to_string(),
                        data_type: DataType::Telemetry,
                        message: "flight path gap: no telemetry fix".to_string(),
                    },
                    SimulatedCapturePathStep::Fix {
                        observed_at: second_time,
                        position: gps_coords_at(37.0003, -122.0005, 102.0),
                        observations: vec![telemetry_observation("sim-telemetry-front")],
                    },
                ],
            })
            .await
            .expect("path gap records coverage hole");

        assert_eq!(batch.records.len(), 2);
        assert_eq!(batch.failures.len(), 1);
        assert_eq!(batch.failures[0].occurred_at, Some(gap_start));
        assert_eq!(batch.failures[0].kind, CollectionFailureKind::SensorDropout);
        assert!(batch.failures[0].message.contains("flight path gap"));
        assert!(!batch
            .records
            .iter()
            .any(|record| record.timestamp == gap_start));
        assert!(!batch
            .records
            .iter()
            .any(|record| record.timestamp == gap_end));

        let stored_session = service.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(stored_session.summary.record_count, 2);
        assert_eq!(stored_session.summary.coverage.successful_records, 2);
        assert_eq!(stored_session.summary.coverage.failed_observations, 1);
        assert_eq!(stored_session.summary.coverage.expected_observations, 3);
        assert_eq!(
            stored_session.summary.coverage.status,
            CaptureCoverageStatus::Partial
        );
        assert!((stored_session.summary.coverage.captured_fraction - (2.0 / 3.0)).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_capture_quality_tracks_freshness_and_full_coverage() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();

        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let record = telemetry_record(&session);
        let record_timestamp = record.timestamp;

        service.collect_data(&session_id, record).await.unwrap();

        let session = service.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(
            session.summary.freshness.last_record_at,
            Some(record_timestamp)
        );
        assert_eq!(session.summary.freshness.status, FreshnessStatus::Fresh);
        assert!(session.summary.freshness.age_seconds.unwrap() >= 0);
        assert_eq!(session.summary.coverage.successful_records, 1);
        assert_eq!(session.summary.coverage.failed_observations, 0);
        assert_eq!(session.summary.coverage.expected_observations, 1);
        assert_eq!(
            session.summary.coverage.status,
            CaptureCoverageStatus::Complete
        );
        assert!((session.summary.coverage.captured_fraction - 1.0).abs() < f32::EPSILON);
        assert!(session.summary.collection_failures.is_empty());
    }

    #[tokio::test]
    async fn test_stale_capture_freshness_is_flagged() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();

        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let mut record = telemetry_record(&session);
        record.timestamp = Utc::now() - chrono::Duration::seconds(120);

        service.collect_data(&session_id, record).await.unwrap();

        let session = service.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.summary.freshness.status, FreshnessStatus::Stale);
        assert!(session.summary.freshness.age_seconds.unwrap() >= 120);
    }

    #[tokio::test]
    async fn test_collection_failure_reduces_coverage_and_persists() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();

        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let record = telemetry_record(&session);

        service.collect_data(&session_id, record).await.unwrap();
        let failure = service
            .record_collection_failure(
                &session_id,
                CollectionFailureRequest {
                    occurred_at: Some(Utc::now()),
                    sensor_id: "lidar-a3".to_string(),
                    data_type: DataType::LidarScan,
                    kind: CollectionFailureKind::SensorDropout,
                    message: "serial frame dropped mid-flight".to_string(),
                },
            )
            .await
            .unwrap();

        let session = service.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.status, SessionStatus::Collecting);
        assert_eq!(session.summary.collection_failures, vec![failure.clone()]);
        assert_eq!(session.summary.coverage.successful_records, 1);
        assert_eq!(session.summary.coverage.failed_observations, 1);
        assert_eq!(session.summary.coverage.expected_observations, 2);
        assert_eq!(
            session.summary.coverage.status,
            CaptureCoverageStatus::Partial
        );
        assert!((session.summary.coverage.captured_fraction - 0.5).abs() < f32::EPSILON);

        let stored_path = temp_dir
            .path()
            .join("sessions")
            .join(session_id.to_string())
            .join("session.json");
        let stored_json = tokio::fs::read(&stored_path).await.unwrap();
        let stored_session: FlightSession = serde_json::from_slice(&stored_json).unwrap();
        assert_eq!(stored_session.summary.collection_failures, vec![failure]);
        assert_eq!(
            stored_session.summary.coverage.status,
            CaptureCoverageStatus::Partial
        );
    }

    #[tokio::test]
    async fn transient_reader_error_retries_with_backoff_and_persists_health() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();

        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let record = telemetry_record(&session);
        let policy = RetryBackoffPolicy::new(2, 50, 2);
        let error_time = Utc.timestamp_opt(1_800_300_000, 0).unwrap();

        let report = service
            .collect_reader_capture_with_retry(
                &session_id,
                policy,
                vec![
                    Err(CaptureReaderError::transient(
                        "sensor-rgb-01",
                        DataType::Telemetry,
                        error_time,
                        "serial timeout",
                    )),
                    Ok(record),
                ],
            )
            .await
            .unwrap();

        assert_eq!(report.attempts, 2);
        assert_eq!(report.backoff_schedule_ms, vec![50]);
        assert!(report.recovered);
        assert!(report.failure.is_none());
        assert_eq!(report.health.successful_reads, 1);
        assert_eq!(report.health.transient_errors, 1);
        assert_eq!(report.health.retry_attempts, 1);
        assert_eq!(report.health.persistent_errors, 0);
        assert_eq!(report.health.operator_alerts, 0);
        assert_eq!(report.health.status, CaptureHealthStatus::Degraded);
        assert!((report.health.success_rate - 0.5).abs() < f32::EPSILON);

        let stored_session = service.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(stored_session.summary.record_count, 1);
        assert_eq!(stored_session.summary.capture_health, report.health);
        assert!(stored_session.summary.collection_failures.is_empty());

        let stored_path = temp_dir
            .path()
            .join("sessions")
            .join(session_id.to_string())
            .join("session.json");
        let stored_json = tokio::fs::read(&stored_path).await.unwrap();
        let stored_session: FlightSession = serde_json::from_slice(&stored_json).unwrap();
        assert_eq!(
            stored_session.summary.capture_health.status,
            CaptureHealthStatus::Degraded
        );
        assert_eq!(stored_session.summary.capture_health.transient_errors, 1);
    }

    #[tokio::test]
    async fn persistent_reader_errors_exhaust_retry_bound_and_record_failure() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();

        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let policy = RetryBackoffPolicy::new(2, 25, 2);
        let first_error = Utc.timestamp_opt(1_800_400_000, 0).unwrap();
        let second_error = Utc.timestamp_opt(1_800_400_001, 0).unwrap();
        let third_error = Utc.timestamp_opt(1_800_400_002, 0).unwrap();

        let report = service
            .collect_reader_capture_with_retry(
                &session_id,
                policy,
                vec![
                    Err(CaptureReaderError::transient(
                        "rplidar-a3-front",
                        DataType::LidarScan,
                        first_error,
                        "serial timeout 1",
                    )),
                    Err(CaptureReaderError::transient(
                        "rplidar-a3-front",
                        DataType::LidarScan,
                        second_error,
                        "serial timeout 2",
                    )),
                    Err(CaptureReaderError::transient(
                        "rplidar-a3-front",
                        DataType::LidarScan,
                        third_error,
                        "serial timeout 3",
                    )),
                ],
            )
            .await
            .unwrap();

        let failure = report.failure.as_ref().expect("retry exhaustion escalates");
        assert_eq!(report.attempts, 3);
        assert_eq!(report.backoff_schedule_ms, vec![25, 50]);
        assert!(!report.recovered);
        assert_eq!(failure.kind, CollectionFailureKind::ReaderError);
        assert_eq!(failure.sensor_id, "rplidar-a3-front");
        assert_eq!(failure.data_type, DataType::LidarScan);
        assert!(failure.message.contains("retry bound exhausted"));
        assert!(failure.message.contains("serial timeout 3"));
        assert_eq!(report.health.successful_reads, 0);
        assert_eq!(report.health.transient_errors, 3);
        assert_eq!(report.health.retry_attempts, 2);
        assert_eq!(report.health.persistent_errors, 1);
        assert_eq!(report.health.operator_alerts, 1);
        assert_eq!(report.health.status, CaptureHealthStatus::Failed);
        assert!((report.health.success_rate - 0.0).abs() < f32::EPSILON);

        let stored_session = service.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(stored_session.summary.record_count, 0);
        assert_eq!(
            stored_session.summary.collection_failures,
            vec![failure.clone()]
        );
        assert_eq!(
            stored_session.summary.coverage.status,
            CaptureCoverageStatus::Partial
        );
        assert_eq!(stored_session.summary.coverage.failed_observations, 1);
        assert_eq!(
            stored_session.summary.capture_health.status,
            CaptureHealthStatus::Failed
        );
        assert_eq!(stored_session.summary.capture_health.operator_alerts, 1);
    }

    #[tokio::test]
    async fn test_search_data_returns_persisted_records_after_restart() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let record = telemetry_record(&session);
        let record_id = record.id;
        let timestamp = record.timestamp;

        service.collect_data(&session_id, record).await.unwrap();
        drop(service);

        let restarted = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let results = restarted
            .search_data(SearchQuery {
                time_range: Some((
                    timestamp - chrono::Duration::minutes(1),
                    timestamp + chrono::Duration::minutes(1),
                )),
                spatial_bounds: Some((39.9, -105.1, 40.1, -104.9)),
                data_types: Some(vec![DataType::Telemetry]),
                drone_ids: Some(vec![session.drone_id]),
                limit: Some(10),
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, record_id);
    }

    #[tokio::test]
    async fn test_capture_session_listing_paginates_filters_and_surfaces_quality() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let request = capture_request();
        let window_start = Utc::now() - chrono::Duration::minutes(1);

        let first_session_id = start_linked_capture_session(&mut service, request.clone()).await;
        let first_session = service
            .get_session(&first_session_id)
            .await
            .unwrap()
            .unwrap();
        service
            .collect_data(&first_session_id, telemetry_record(&first_session))
            .await
            .unwrap();
        service.end_session(&first_session_id).await.unwrap();

        let second_session_id = start_linked_capture_session(&mut service, request.clone()).await;
        let second_session = service
            .get_session(&second_session_id)
            .await
            .unwrap()
            .unwrap();
        let second_started_at = Utc::now();
        service
            .collect_data(
                &second_session_id,
                telemetry_record_at(&second_session, second_started_at, 40.0, -105.0, 0.9),
            )
            .await
            .unwrap();
        service
            .collect_data(
                &second_session_id,
                telemetry_record_at(
                    &second_session,
                    second_started_at + chrono::Duration::seconds(10),
                    40.001,
                    -104.999,
                    0.88,
                ),
            )
            .await
            .unwrap();
        service.end_session(&second_session_id).await.unwrap();

        let page = service
            .list_capture_sessions(CaptureSessionListFilter {
                field_id: Some(request.field_id),
                flight_id: Some(request.flight_id),
                started_after: Some(window_start),
                started_before: Some(Utc::now() + chrono::Duration::minutes(1)),
                offset: 0,
                limit: 1,
            })
            .await
            .unwrap();

        assert_eq!(page.total_count, 2);
        assert_eq!(page.items.len(), 1);
        assert!(page.has_more);
        assert_eq!(page.items[0].session_id, second_session_id);
        assert_eq!(page.items[0].field_id, request.field_id);
        assert_eq!(page.items[0].flight_id, request.flight_id);
        assert_eq!(page.items[0].freshness.status, FreshnessStatus::Fresh);
        assert_eq!(
            page.items[0].coverage.status,
            CaptureCoverageStatus::Complete
        );
        assert_eq!(
            page.items[0].aggregate_evidence.status,
            SessionAggregateStatus::FromTelemetryTrack
        );
        assert_eq!(page.items[0].qa.masked_records, 0);
        assert!(page.items[0].qa.passed);
    }

    #[tokio::test]
    async fn test_capture_session_inspection_surfaces_failed_capture_evidence() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();

        service
            .collect_data(&session_id, telemetry_record(&session))
            .await
            .unwrap();
        let failure = service
            .record_collection_failure(
                &session_id,
                CollectionFailureRequest {
                    occurred_at: Some(Utc::now()),
                    sensor_id: "lidar-a3".to_string(),
                    data_type: DataType::LidarScan,
                    kind: CollectionFailureKind::SensorDropout,
                    message: "serial frame dropped mid-flight".to_string(),
                },
            )
            .await
            .unwrap();
        service.fail_session(&session_id).await.unwrap();

        let inspection = service.inspect_capture_session(&session_id).await.unwrap();

        assert_eq!(inspection.session.session_id, session_id);
        assert_eq!(inspection.session.status, SessionStatus::Failed);
        assert_eq!(
            inspection.session.coverage.status,
            CaptureCoverageStatus::Partial
        );
        assert_eq!(inspection.session.coverage.failed_observations, 1);
        assert_eq!(inspection.failures, vec![failure]);
        assert_eq!(inspection.summary.collection_failures, inspection.failures);
    }

    #[tokio::test]
    async fn test_export_session_json_loads_real_records() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let record = telemetry_record(&session);
        let record_id = record.id;

        service.collect_data(&session_id, record).await.unwrap();

        let output_path = temp_dir.path().join("session-export.json");
        service
            .export_session(&session_id, ExportFormat::Json, &output_path)
            .await
            .unwrap();

        let exported_json = tokio::fs::read(&output_path).await.unwrap();
        let exported: serde_json::Value = serde_json::from_slice(&exported_json).unwrap();
        let records = exported["records"].as_array().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["id"].as_str().unwrap(), record_id.to_string());
        assert_eq!(
            records[0]["session_id"].as_str().unwrap(),
            session_id.to_string()
        );
        assert_eq!(records[0]["sensor_id"].as_str().unwrap(), "sensor-rgb-01");
        assert_eq!(
            records[0]["calibration_ref"].as_str().unwrap(),
            "calibration-2026-06"
        );
    }

    #[tokio::test]
    async fn test_export_session_csv_loads_real_records() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let record = telemetry_record(&session);
        let record_id = record.id;

        service.collect_data(&session_id, record).await.unwrap();

        let output_path = temp_dir.path().join("session-export.csv");
        service
            .export_session(&session_id, ExportFormat::Csv, &output_path)
            .await
            .unwrap();

        let exported = tokio::fs::read_to_string(&output_path).await.unwrap();
        let lines = exported.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("record_id,session_id"));
        assert!(lines[1].contains(&record_id.to_string()));
        assert!(lines[1].contains("sensor-rgb-01"));
        assert!(lines[1].contains("calibration-2026-06"));
    }

    #[tokio::test]
    async fn test_export_session_json_allows_empty_session() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;

        let output_path = temp_dir.path().join("empty-session-export.json");
        service
            .export_session(&session_id, ExportFormat::Json, &output_path)
            .await
            .unwrap();

        let exported_json = tokio::fs::read(&output_path).await.unwrap();
        let exported: serde_json::Value = serde_json::from_slice(&exported_json).unwrap();
        assert_eq!(exported["records"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_export_session_kml_preserves_crs_extent_and_coordinate() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let first = telemetry_record_at(
            &session,
            Utc.with_ymd_and_hms(2026, 6, 11, 12, 0, 0).unwrap(),
            40.0,
            -105.0,
            0.9,
        );
        let second = telemetry_record_at(
            &session,
            Utc.with_ymd_and_hms(2026, 6, 11, 12, 0, 5).unwrap(),
            40.002,
            -104.998,
            0.89,
        );

        service.collect_data(&session_id, first).await.unwrap();
        service.collect_data(&session_id, second).await.unwrap();

        let output_path = temp_dir.path().join("session-export.kml");
        service
            .export_session(&session_id, ExportFormat::Kml, &output_path)
            .await
            .unwrap();

        let exported = tokio::fs::read_to_string(&output_path).await.unwrap();
        assert!(exported.contains("agbot:crs=\"EPSG:4326\""));
        assert!(exported.contains("agbot:min_lat=\"40\""));
        assert!(exported.contains("agbot:max_lat=\"40.002\""));
        assert!(exported.contains("agbot:min_lon=\"-105\""));
        assert!(exported.contains("agbot:max_lon=\"-104.998\""));
        assert!(exported.contains("agbot:resolution_m=\""));
        assert!(!exported.contains("agbot:resolution_m=\"0\""));
        assert!(exported.contains("agbot:record_count=\"2\""));
        assert!(exported.contains("<coordinates>-105,40,30</coordinates>"));
    }

    #[tokio::test]
    async fn test_export_session_gated_formats_return_clean_error() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;

        for format in [ExportFormat::Parquet, ExportFormat::HDF5] {
            let output_path = temp_dir.path().join(format!("{format:?}.bin"));
            let err = service
                .export_session(&session_id, format, &output_path)
                .await
                .unwrap_err();
            assert!(err.to_string().contains("not enabled"));
        }
    }

    #[tokio::test]
    async fn test_record_provenance_is_required_for_all_types_and_payloads() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();

        for data_type in all_data_types() {
            for payload in all_payloads() {
                let record = FlightDataRecord::new(
                    session.flight_id,
                    session.drone_id,
                    data_type.clone(),
                    payload,
                    provenance(&session),
                    128,
                )
                .unwrap();

                assert_eq!(record.session_id, session.id);
                assert_eq!(record.sensor_id, "sensor-rgb-01");
                assert_eq!(record.gps_coords, Some(gps_coords()));
                assert_eq!(record.calibration_ref, "calibration-2026-06");
                record.validate_provenance().unwrap();
            }
        }
    }

    #[tokio::test]
    async fn test_missing_gps_or_timestamp_is_rejected_as_provenance_error() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();

        let mut missing_gps = provenance(&session);
        missing_gps.gps_coords = None;
        let gps_err = FlightDataRecord::new(
            session.flight_id,
            session.drone_id,
            DataType::Telemetry,
            DataPayload::Raw {
                format: "json".to_string(),
                schema: None,
                compression: None,
            },
            missing_gps,
            128,
        )
        .unwrap_err();
        assert_eq!(gps_err, FlightDataProvenanceError::MissingGpsCoords);

        let mut missing_timestamp = provenance(&session);
        missing_timestamp.timestamp = None;
        let timestamp_err = FlightDataRecord::new(
            session.flight_id,
            session.drone_id,
            DataType::Telemetry,
            DataPayload::Raw {
                format: "json".to_string(),
                schema: None,
                compression: None,
            },
            missing_timestamp,
            128,
        )
        .unwrap_err();
        assert_eq!(timestamp_err, FlightDataProvenanceError::MissingTimestamp);
    }

    #[tokio::test]
    async fn test_collect_data_rejects_incomplete_provenance_record() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        let mut record = telemetry_record(&session);
        record.gps_coords = None;

        let err = service.collect_data(&session_id, record).await.unwrap_err();
        let provenance_error = err.downcast_ref::<FlightDataProvenanceError>().unwrap();

        assert_eq!(
            provenance_error,
            &FlightDataProvenanceError::MissingGpsCoords
        );
    }

    #[tokio::test]
    async fn test_fail_session_transitions_to_failed_terminal_state() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = start_linked_capture_session(&mut service, capture_request()).await;

        let failed_session = service.fail_session(&session_id).await.unwrap();

        assert_eq!(failed_session.status, SessionStatus::Failed);
        assert!(failed_session.end_time.is_some());
        assert!(service.get_session(&session_id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_collect_before_start_is_rejected_with_state_error() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let fake_session = FlightSession {
            id: Uuid::new_v4(),
            flight_id: Uuid::new_v4(),
            field_id: Uuid::new_v4(),
            scene_id: Uuid::new_v4(),
            owner_id: "grower-ops".to_string(),
            mission_id: None,
            drone_id: Uuid::new_v4(),
            start_time: Utc::now(),
            end_time: None,
            status: SessionStatus::Started,
            data_records: Vec::new(),
            summary: SessionSummary::default(),
            tags: Vec::new(),
        };
        let record = telemetry_record(&fake_session);

        let err = service
            .collect_data(&fake_session.id, record)
            .await
            .unwrap_err();
        let lifecycle_error = err.downcast_ref::<SessionLifecycleError>().unwrap();

        assert!(matches!(
            lifecycle_error,
            SessionLifecycleError::SessionNotFound { session_id }
                if *session_id == fake_session.id
        ));
    }
}
