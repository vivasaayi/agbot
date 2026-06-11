use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub mod export;
pub mod indexing;
pub mod storage;

pub use export::{DataExporter, ExportFormat};
pub use indexing::{DataIndexer, IndexConfig, SearchQuery};
pub use storage::{StorageConfig, StorageEngine};

/// Data collection and storage system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightDataRecord {
    pub id: Uuid,
    pub flight_id: Uuid,
    pub drone_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub data_type: DataType,
    pub payload: DataPayload,
    pub metadata: HashMap<String, String>,
    pub file_path: Option<PathBuf>,
    pub size_bytes: u64,
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

    pub async fn collect_data(&mut self, session_id: &Uuid, data: FlightDataRecord) -> Result<()> {
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
        self.indexer.search(query).await
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

        // TODO: Load session data first
        let session_records = Vec::new(); // Placeholder

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
        self.indexer.rebuild().await?;

        tracing::info!("Cleaned up {} old data records", removed_count);
        Ok(removed_count as u32)
    }

    pub async fn get_storage_stats(&self) -> Result<StorageStats> {
        self.storage.get_stats().await
    }

    async fn calculate_session_summary(&self, session: &FlightSession) -> Result<SessionSummary> {
        let mut summary = SessionSummary::default();

        // Calculate from stored records
        for record_id in &session.data_records {
            if let Some(record) = self.storage.load_data(record_id).await? {
                summary.record_count += 1;
                summary.total_data_size_bytes += record.size_bytes;
                *summary.data_types.entry(record.data_type).or_insert(0) += 1;
            }
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

    fn telemetry_record(session: &FlightSession) -> FlightDataRecord {
        FlightDataRecord {
            id: Uuid::new_v4(),
            flight_id: session.flight_id,
            drone_id: session.drone_id,
            timestamp: Utc::now(),
            data_type: DataType::Telemetry,
            payload: DataPayload::Telemetry {
                position: (40.0, -105.0, 30.0),
                velocity: (1.0, 0.0, 0.0),
                orientation: (0.0, 0.0, 0.0),
                battery_level: 0.9,
                signal_strength: 0.95,
            },
            metadata: HashMap::new(),
            file_path: None,
            size_bytes: 256,
        }
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

        service.collect_data(&session_id, record).await.unwrap();

        let session = service.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.status, SessionStatus::Collecting);
        assert_eq!(session.summary.record_count, 1);
        assert_eq!(session.data_records.len(), 1);
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
