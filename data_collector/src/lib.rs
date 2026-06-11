use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::schemas::GpsCoords;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub mod export;
pub mod indexing;
pub mod multispectral;
pub mod rplidar;
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
        self.refresh_capture_quality(now);
    }

    fn record_collection_failure(&mut self, failure: CollectionFailure, now: DateTime<Utc>) {
        self.summary.collection_failures.push(failure);
        self.refresh_capture_quality(now);
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CollectionFailureKind {
    SensorDropout,
    MalformedFrame,
    MissingBand,
    ReaderError,
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
}

/// Main data collector service
pub struct DataCollectorService {
    storage: StorageEngine,
    active_sessions: HashMap<Uuid, FlightSession>,
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
            indexer,
            auto_export: false,
            retention_days: 365,
        })
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

        self.start_capture_session(request).await
    }

    pub async fn start_capture_session(&mut self, request: CaptureSessionRequest) -> Result<Uuid> {
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
            session.record_successful_capture(&stored_data, Utc::now());
            session.clone()
        };

        self.storage.store_session(&session_snapshot).await?;

        // Update index
        self.indexer.index_record(&stored_data);

        Ok(())
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
        let telemetry_records = self.get_telemetry_for_session(&session.id).await?;
        if !telemetry_records.is_empty() {
            summary.flight_duration_seconds = self.calculate_flight_duration(&telemetry_records);
            summary.distance_covered_m = self.calculate_distance_covered(&telemetry_records);
            summary.area_covered_m2 = self.calculate_area_covered(&telemetry_records);
            summary.battery_consumed_percent =
                self.calculate_battery_consumption(&telemetry_records);
        }

        Ok(summary)
    }

    async fn get_telemetry_for_session(&self, _session_id: &Uuid) -> Result<Vec<FlightDataRecord>> {
        // Implementation would query telemetry records for the session
        Ok(Vec::new())
    }

    fn calculate_flight_duration(&self, _records: &[FlightDataRecord]) -> f32 {
        // Calculate based on first and last telemetry timestamps
        0.0
    }

    fn calculate_distance_covered(&self, _records: &[FlightDataRecord]) -> f32 {
        // Sum distances between consecutive GPS positions
        0.0
    }

    fn calculate_area_covered(&self, _records: &[FlightDataRecord]) -> f32 {
        // Calculate area of convex hull of flight path
        0.0
    }

    fn calculate_battery_consumption(&self, _records: &[FlightDataRecord]) -> f32 {
        // Calculate based on first and last battery readings
        0.0
    }
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
        let session_id = service.start_capture_session(request).await.unwrap();

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

        let session_id = service.start_capture_session(request).await.unwrap();
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
    async fn test_collect_data_transitions_started_session_to_collecting() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();

        let session_id = service
            .start_capture_session(capture_request())
            .await
            .unwrap();
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
    async fn test_capture_quality_tracks_freshness_and_full_coverage() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();

        let session_id = service
            .start_capture_session(capture_request())
            .await
            .unwrap();
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

        let session_id = service
            .start_capture_session(capture_request())
            .await
            .unwrap();
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

        let session_id = service
            .start_capture_session(capture_request())
            .await
            .unwrap();
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
    async fn test_search_data_returns_persisted_records_after_restart() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = service
            .start_capture_session(capture_request())
            .await
            .unwrap();
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
    async fn test_export_session_json_loads_real_records() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = service
            .start_capture_session(capture_request())
            .await
            .unwrap();
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
        let session_id = service
            .start_capture_session(capture_request())
            .await
            .unwrap();
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
        let session_id = service
            .start_capture_session(capture_request())
            .await
            .unwrap();

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
    async fn test_record_provenance_is_required_for_all_types_and_payloads() {
        let temp_dir = tempdir().unwrap();
        let mut service = DataCollectorService::new(temp_dir.path().to_path_buf()).unwrap();
        let session_id = service
            .start_capture_session(capture_request())
            .await
            .unwrap();
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
        let session_id = service
            .start_capture_session(capture_request())
            .await
            .unwrap();
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
        let session_id = service
            .start_capture_session(capture_request())
            .await
            .unwrap();
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
        let session_id = service
            .start_capture_session(capture_request())
            .await
            .unwrap();

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
