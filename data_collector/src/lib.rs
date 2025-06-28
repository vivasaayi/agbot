use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tokio::fs;

pub mod storage;
pub mod export;
pub mod indexing;

pub use storage::{StorageEngine, StorageConfig};
pub use export::{DataExporter, ExportFormat};
pub use indexing::{DataIndexer, IndexConfig, SearchQuery};

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

/// Flight session containing related data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightSession {
    pub id: Uuid,
    pub mission_id: Option<Uuid>,
    pub drone_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub status: SessionStatus,
    pub data_records: Vec<Uuid>,
    pub summary: SessionSummary,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionStatus {
    Active,
    Completed,
    Aborted,
    Failed,
    Processing,
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
    data_root: PathBuf,
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
            data_root,
            indexer,
            auto_export: false,
            retention_days: 365,
        })
    }

    pub async fn start_session(&mut self, drone_id: Uuid, mission_id: Option<Uuid>) -> Result<Uuid> {
        let session = FlightSession {
            id: Uuid::new_v4(),
            mission_id,
            drone_id,
            start_time: Utc::now(),
            end_time: None,
            status: SessionStatus::Active,
            data_records: Vec::new(),
            summary: SessionSummary::default(),
            tags: Vec::new(),
        };

        let session_id = session.id;
        self.active_sessions.insert(session_id, session);
        
        // Create session directory
        let session_dir = self.get_session_path(&session_id);
        fs::create_dir_all(&session_dir).await?;

        tracing::info!("Started data collection session: {}", session_id);
        Ok(session_id)
    }

    pub async fn end_session(&mut self, session_id: &Uuid) -> Result<FlightSession> {
        if let Some(mut session) = self.active_sessions.remove(session_id) {
            session.end_time = Some(Utc::now());
            session.status = SessionStatus::Completed;
            
            // Calculate final summary
            session.summary = self.calculate_session_summary(&session).await?;
            
            // Store session metadata
            self.storage.store_session(&session).await?;
            
            // Update index
            self.indexer.index_session(&session).await?;
            
            tracing::info!("Ended data collection session: {}", session_id);
            Ok(session)
        } else {
            Err(anyhow::anyhow!("Session not found: {}", session_id))
        }
    }

    pub async fn collect_data(&mut self, session_id: &Uuid, data: FlightDataRecord) -> Result<()> {
        // Store the data
        let stored_data = self.storage.store_data(&data).await?;
        
        // Update session
        if let Some(session) = self.active_sessions.get_mut(session_id) {
            session.data_records.push(stored_data.id);
            session.summary.record_count += 1;
            session.summary.total_data_size_bytes += stored_data.size_bytes;
            
            // Update data type counts
            *session.summary.data_types.entry(stored_data.data_type.clone()).or_insert(0) += 1;
        }

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

    pub async fn list_sessions(&self, drone_id: Option<Uuid>, limit: Option<u32>) -> Result<Vec<FlightSession>> {
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

    pub async fn export_session(&self, session_id: &Uuid, format: ExportFormat, output_path: &Path) -> Result<()> {
        let session = self.get_session(session_id).await?
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        let export_config = export::ExportConfig {
            format,
            include_metadata: true,
            compress: false,
        };
        let exporter = DataExporter::new(export_config);
        
        // TODO: Load session data first
        let session_records = Vec::new(); // Placeholder
        
        match exporter.export_session(&session, &session_records, output_path).await {
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

    fn get_session_path(&self, session_id: &Uuid) -> PathBuf {
        self.data_root.join("sessions").join(session_id.to_string())
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
            summary.battery_consumed_percent = self.calculate_battery_consumption(&telemetry_records);
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
        
        let drone_id = Uuid::new_v4();
        let session_id = service.start_session(drone_id, None).await.unwrap();
        
        let session = service.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.drone_id, drone_id);
        assert!(matches!(session.status, SessionStatus::Active));
        
        let ended_session = service.end_session(&session_id).await.unwrap();
        assert!(matches!(ended_session.status, SessionStatus::Completed));
    }
}
