use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{DataType, FlightDataRecord as DataRecord, FlightSession as CollectionSession};

/// Storage engine for collected data
#[derive(Debug, Clone)]
pub struct StorageEngine {
    pub config: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub base_path: PathBuf,
    pub max_file_size_mb: u64,
    pub compression_enabled: bool,
    pub encryption_enabled: bool,
    pub backup_enabled: bool,
    pub retention_days: u32,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("./data"),
            max_file_size_mb: 100,
            compression_enabled: true,
            encryption_enabled: false,
            backup_enabled: true,
            retention_days: 365,
        }
    }
}

impl StorageEngine {
    pub fn new(config: StorageConfig) -> Result<Self> {
        // TODO: Add validation logic if needed
        Ok(Self { config })
    }

    /// Store a data record to persistent storage
    pub async fn store_record(&self, record: &DataRecord) -> Result<PathBuf> {
        let storage_path = self.get_storage_path(record)?;

        // Ensure directory exists
        if let Some(parent) = storage_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Serialize and write the record
        let data = if self.config.compression_enabled {
            self.compress_data(record).await?
        } else {
            serde_json::to_vec_pretty(record)?
        };

        fs::write(&storage_path, data).await?;

        // Create backup if enabled
        if self.config.backup_enabled {
            self.create_backup(&storage_path, record).await?;
        }

        Ok(storage_path)
    }

    /// Retrieve a data record by ID
    pub async fn retrieve_record(&self, record_id: &Uuid) -> Result<Option<DataRecord>> {
        let direct_path = self.get_record_path(record_id)?;
        let storage_path = if direct_path.exists() {
            Some(direct_path)
        } else {
            self.find_record_path(record_id).await?
        };

        let Some(storage_path) = storage_path else {
            return Ok(None);
        };

        let data = fs::read(&storage_path).await?;

        let record = if self.config.compression_enabled {
            self.decompress_data(&data).await?
        } else {
            serde_json::from_slice(&data)?
        };

        Ok(Some(record))
    }

    /// Store a complete collection session
    pub async fn store_session(&self, session: &CollectionSession) -> Result<PathBuf> {
        let session_path = self.get_session_path(&session.id)?;

        // Create session directory
        fs::create_dir_all(&session_path).await?;

        // Store session metadata
        let metadata_path = session_path.join("session.json");
        let metadata = serde_json::to_vec_pretty(session)?;
        fs::write(&metadata_path, metadata).await?;

        // Store individual records
        for record_id in &session.data_records {
            let _record_path = session_path.join(format!("{}.json", record_id));
            // Note: This would need to load the actual record data
            // For now, we'll skip individual record storage in session export
            // Individual records are stored separately via store_data()
        }

        Ok(session_path)
    }

    /// List all stored sessions
    pub async fn list_sessions(
        &self,
        drone_id: Option<uuid::Uuid>,
        limit: Option<u32>,
    ) -> Result<Vec<crate::FlightSession>> {
        let mut sessions = Vec::new();

        for metadata_path in self.session_metadata_paths()? {
            let metadata = fs::read(&metadata_path).await?;
            let session: CollectionSession = serde_json::from_slice(&metadata)?;
            if drone_id.is_none() || drone_id == Some(session.drone_id) {
                sessions.push(session);
            }
        }

        sessions.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        if let Some(limit) = limit {
            sessions.truncate(limit as usize);
        }

        Ok(sessions)
    }

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<StorageStatistics> {
        let mut stats = StorageStatistics::default();

        // Calculate total size and count files
        self.calculate_directory_stats(&self.config.base_path, &mut stats)
            .await?;

        // Get data type breakdown
        stats.data_type_breakdown = self.get_data_type_breakdown().await?;

        Ok(stats)
    }

    /// Clean up old data based on retention policy
    pub async fn cleanup_old_data(&self) -> Result<u64> {
        let cutoff_date = Utc::now() - chrono::Duration::days(self.config.retention_days as i64);
        let mut cleaned_bytes = 0u64;

        let sessions = self.list_sessions(None, None).await?;

        for session in sessions {
            let session_path = self.get_session_path(&session.id)?;
            let metadata_path = session_path.join("session.json");

            if metadata_path.exists() {
                let metadata = fs::read(&metadata_path).await?;
                if let Ok(session) = serde_json::from_slice::<CollectionSession>(&metadata) {
                    if session.start_time < cutoff_date {
                        let size = self.get_directory_size(&session_path).await?;
                        fs::remove_dir_all(&session_path).await?;
                        cleaned_bytes += size;
                    }
                }
            }
        }

        Ok(cleaned_bytes)
    }

    // Private helper methods

    fn get_storage_path(&self, record: &DataRecord) -> Result<PathBuf> {
        let date_path = record.timestamp.format("%Y/%m/%d").to_string();
        let filename = format!(
            "{}_{}.json",
            record.data_type.to_string().to_lowercase(),
            record.id
        );

        Ok(self
            .config
            .base_path
            .join("records")
            .join(&date_path)
            .join(&filename))
    }

    fn get_record_path(&self, record_id: &Uuid) -> Result<PathBuf> {
        // For retrieval, we need to search for the record
        // This is a simplified implementation
        Ok(self
            .config
            .base_path
            .join("records")
            .join(format!("{}.json", record_id)))
    }

    fn record_json_paths(&self) -> Result<Vec<PathBuf>> {
        Self::json_paths_under(&self.config.base_path.join("records"))
    }

    fn session_metadata_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        let sessions_path = self.config.base_path.join("sessions");
        if !sessions_path.exists() {
            return Ok(paths);
        }

        for entry in walkdir::WalkDir::new(sessions_path) {
            let entry = entry?;
            if entry.file_type().is_file() && entry.file_name() == "session.json" {
                paths.push(entry.path().to_path_buf());
            }
        }

        Ok(paths)
    }

    fn json_paths_under(root: &Path) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        if !root.exists() {
            return Ok(paths);
        }

        for entry in walkdir::WalkDir::new(root) {
            let entry = entry?;
            if entry.file_type().is_file()
                && entry.path().extension().map_or(false, |ext| ext == "json")
            {
                paths.push(entry.path().to_path_buf());
            }
        }

        Ok(paths)
    }

    async fn find_record_path(&self, record_id: &Uuid) -> Result<Option<PathBuf>> {
        let record_id = record_id.to_string();
        for path in self.record_json_paths()? {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .map_or(false, |name| name.contains(&record_id))
            {
                return Ok(Some(path));
            }
        }

        Ok(None)
    }

    fn get_session_path(&self, session_id: &Uuid) -> Result<PathBuf> {
        Ok(self
            .config
            .base_path
            .join("sessions")
            .join(session_id.to_string()))
    }

    async fn compress_data(&self, record: &DataRecord) -> Result<Vec<u8>> {
        // Simplified compression - in practice, use a proper compression library
        let json_data = serde_json::to_vec(record)?;
        Ok(json_data)
    }

    async fn decompress_data(&self, data: &[u8]) -> Result<DataRecord> {
        // Simplified decompression
        let record = serde_json::from_slice(data)?;
        Ok(record)
    }

    async fn create_backup(&self, original_path: &Path, record: &DataRecord) -> Result<()> {
        let backup_path = self
            .config
            .base_path
            .join("backups")
            .join(format!("{}.backup", record.id));

        if let Some(parent) = backup_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::copy(original_path, &backup_path).await?;
        Ok(())
    }

    async fn calculate_directory_stats(
        &self,
        path: &Path,
        stats: &mut StorageStatistics,
    ) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let mut entries = fs::read_dir(path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;

            if metadata.is_file() {
                stats.total_files += 1;
                stats.total_size_bytes += metadata.len();
            } else if metadata.is_dir() {
                Box::pin(self.calculate_directory_stats(&entry.path(), stats)).await?;
            }
        }

        Ok(())
    }

    async fn get_data_type_breakdown(&self) -> Result<HashMap<DataType, u32>> {
        let mut breakdown = HashMap::new();

        // Simplified implementation - scan all records
        let records_path = self.config.base_path.join("records");
        if records_path.exists() {
            self.scan_data_types(&records_path, &mut breakdown).await?;
        }

        Ok(breakdown)
    }

    async fn scan_data_types(
        &self,
        path: &Path,
        breakdown: &mut HashMap<DataType, u32>,
    ) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let mut entries = fs::read_dir(path).await?;

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "json" {
                        if let Ok(data) = fs::read(entry.path()).await {
                            if let Ok(record) = serde_json::from_slice::<DataRecord>(&data) {
                                *breakdown.entry(record.data_type).or_insert(0) += 1;
                            }
                        }
                    }
                }
            } else if entry.file_type().await?.is_dir() {
                Box::pin(self.scan_data_types(&entry.path(), breakdown)).await?;
            }
        }

        Ok(())
    }

    async fn get_directory_size(&self, path: &Path) -> Result<u64> {
        let mut total_size = 0u64;

        if !path.exists() {
            return Ok(0);
        }

        let mut entries = fs::read_dir(path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;

            if metadata.is_file() {
                total_size += metadata.len();
            } else if metadata.is_dir() {
                total_size += Box::pin(self.get_directory_size(&entry.path())).await?;
            }
        }

        Ok(total_size)
    }

    pub async fn store_data(
        &self,
        data: &crate::FlightDataRecord,
    ) -> Result<crate::FlightDataRecord> {
        let prepared = crate::prepare_record_for_storage(data)?;
        let _storage_path = self.store_record(&prepared).await?;
        Ok(prepared)
    }

    pub async fn load_session(&self, session_id: &Uuid) -> Result<Option<crate::FlightSession>> {
        let session_path = self.get_session_path(session_id)?.join("session.json");

        if !session_path.exists() {
            return Ok(None);
        }

        let metadata = fs::read(&session_path).await?;
        let session = serde_json::from_slice(&metadata)?;
        Ok(Some(session))
    }

    pub async fn cleanup_before_date(
        &self,
        cutoff_date: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64> {
        let sessions = self.list_sessions(None, None).await?;
        let expired_sessions = sessions
            .into_iter()
            .filter(|session| session.start_time < cutoff_date)
            .collect::<Vec<_>>();

        if let Some(active_session) = expired_sessions.iter().find(|session| {
            matches!(
                session.status,
                crate::SessionStatus::Started | crate::SessionStatus::Collecting
            )
        }) {
            return Err(anyhow!(
                "refusing retention cleanup for in-progress session {}",
                active_session.id
            ));
        }

        let mut cleaned_bytes = 0u64;
        for session in expired_sessions {
            let mut session_removed_bytes = 0u64;
            let mut removed_records = 0u32;

            for record_id in &session.data_records {
                if let Some(record_path) = self.find_record_path(record_id).await? {
                    let size = fs::metadata(&record_path).await?.len();
                    fs::remove_file(&record_path).await?;
                    session_removed_bytes += size;
                    removed_records += 1;
                }
            }

            let session_path = self.get_session_path(&session.id)?;
            if session_path.exists() {
                let session_dir_size = self.get_directory_size(&session_path).await?;
                fs::remove_dir_all(&session_path).await?;
                session_removed_bytes += session_dir_size;
            }

            self.append_retention_audit(
                &session,
                cutoff_date,
                removed_records,
                session_removed_bytes,
            )
            .await?;
            cleaned_bytes += session_removed_bytes;
        }

        Ok(cleaned_bytes)
    }

    pub async fn get_stats(&self) -> Result<crate::StorageStats> {
        let mut stats = crate::StorageStats {
            total_records: 0,
            total_size_bytes: 0,
            sessions_count: self.list_sessions(None, None).await?.len() as u32,
            oldest_record: None,
            newest_record: None,
            data_type_breakdown: std::collections::HashMap::new(),
        };

        for record_path in self.record_json_paths()? {
            let data = fs::read(&record_path).await?;
            let record: DataRecord = serde_json::from_slice(&data)?;
            let metadata = fs::metadata(&record_path).await?;

            stats.total_records += 1;
            stats.total_size_bytes += metadata.len();
            stats.oldest_record = Some(
                stats
                    .oldest_record
                    .map_or(record.timestamp, |oldest| oldest.min(record.timestamp)),
            );
            stats.newest_record = Some(
                stats
                    .newest_record
                    .map_or(record.timestamp, |newest| newest.max(record.timestamp)),
            );
            *stats
                .data_type_breakdown
                .entry(record.data_type)
                .or_insert(0) += 1;
        }

        Ok(stats)
    }

    pub async fn load_data(
        &self,
        record_id: &uuid::Uuid,
    ) -> Result<Option<crate::FlightDataRecord>> {
        self.retrieve_record(record_id)
            .await?
            .map(|record| crate::verify_record_integrity(&record))
            .transpose()
    }

    pub async fn load_all_data(&self) -> Result<Vec<crate::FlightDataRecord>> {
        let mut records = Vec::new();

        for record_path in self.record_json_paths()? {
            let data = fs::read(&record_path).await?;
            let record = serde_json::from_slice::<DataRecord>(&data)?;
            records.push(crate::verify_record_integrity(&record)?);
        }

        records.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then_with(|| a.id.as_bytes().cmp(b.id.as_bytes()))
        });

        Ok(records)
    }

    async fn append_retention_audit(
        &self,
        session: &CollectionSession,
        cutoff_date: DateTime<Utc>,
        removed_records: u32,
        removed_bytes: u64,
    ) -> Result<()> {
        let audit_path = self.config.base_path.join("audit").join("retention.jsonl");
        if let Some(parent) = audit_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let event = serde_json::json!({
            "event": "retention_cleanup",
            "session_id": session.id,
            "cutoff_date": cutoff_date,
            "session_start_time": session.start_time,
            "session_status": session.status,
            "removed_records": removed_records,
            "removed_bytes": removed_bytes,
        });
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(audit_path)
            .await?;
        file.write_all(event.to_string().as_bytes()).await?;
        file.write_all(b"\n").await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageStatistics {
    pub total_files: u64,
    pub total_size_bytes: u64,
    pub data_type_breakdown: HashMap<DataType, u32>,
    pub oldest_record: Option<DateTime<Utc>>,
    pub newest_record: Option<DateTime<Utc>>,
}

impl DataType {
    fn to_string(&self) -> &'static str {
        match self {
            DataType::Telemetry => "telemetry",
            DataType::SensorReading => "sensor",
            DataType::Image => "image",
            DataType::Video => "video",
            DataType::LidarScan => "lidar",
            DataType::MultispectralImage => "multispectral",
            DataType::ThermalImage => "thermal",
            DataType::GPSTrack => "gps",
            DataType::FlightLog => "flight_log",
            DataType::MissionPlan => "mission",
            DataType::WeatherData => "weather",
            DataType::SystemLog => "system",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::GpsCoords;
    use tempfile::tempdir;

    fn test_config(base_path: PathBuf) -> StorageConfig {
        StorageConfig {
            base_path,
            compression_enabled: false,
            backup_enabled: false,
            ..Default::default()
        }
    }

    fn test_session(
        drone_id: Uuid,
        status: crate::SessionStatus,
        start_time: DateTime<Utc>,
    ) -> CollectionSession {
        CollectionSession {
            id: Uuid::new_v4(),
            flight_id: Uuid::new_v4(),
            field_id: Uuid::new_v4(),
            scene_id: Uuid::new_v4(),
            owner_id: "grower-ops".to_string(),
            mission_id: Some(Uuid::new_v4()),
            drone_id,
            start_time,
            end_time: matches!(
                status,
                crate::SessionStatus::Ended | crate::SessionStatus::Failed
            )
            .then_some(start_time + chrono::Duration::minutes(10)),
            status,
            data_records: Vec::new(),
            summary: crate::SessionSummary::default(),
            tags: Vec::new(),
        }
    }

    fn test_record(session: &CollectionSession, timestamp: DateTime<Utc>) -> DataRecord {
        DataRecord {
            id: Uuid::new_v4(),
            session_id: session.id,
            flight_id: session.flight_id,
            drone_id: session.drone_id,
            data_type: DataType::Telemetry,
            timestamp,
            payload: crate::DataPayload::Telemetry {
                position: (40.0, -105.0, 30.0),
                velocity: (1.0, 0.0, 0.0),
                orientation: (0.0, 0.0, 0.0),
                battery_level: 0.8,
                signal_strength: 0.9,
            },
            sensor_id: "telemetry-01".to_string(),
            gps_coords: Some(GpsCoords {
                latitude: 40.0,
                longitude: -105.0,
                altitude: 30.0,
            }),
            calibration_ref: "calibration-2026-06".to_string(),
            metadata: std::collections::HashMap::new(),
            file_path: None,
            size_bytes: 256,
        }
    }

    #[tokio::test]
    async fn test_storage_engine_creation() {
        let config = StorageConfig::default();
        let engine = StorageEngine::new(config).unwrap();
        assert!(engine.config.base_path.to_string_lossy().contains("data"));
    }

    #[tokio::test]
    async fn test_storage_path_generation() {
        let config = StorageConfig {
            base_path: PathBuf::from("/tmp/test"),
            ..Default::default()
        };
        let engine = StorageEngine::new(config).unwrap();

        let record = crate::FlightDataRecord {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            flight_id: Uuid::new_v4(),
            drone_id: Uuid::new_v4(),
            data_type: DataType::Image,
            timestamp: Utc::now(),
            payload: crate::DataPayload::Raw {
                format: "test".to_string(),
                schema: None,
                compression: None,
            },
            sensor_id: "camera-rgb-01".to_string(),
            gps_coords: Some(GpsCoords {
                latitude: 40.0,
                longitude: -105.0,
                altitude: 30.0,
            }),
            calibration_ref: "calibration-2026-06".to_string(),
            metadata: std::collections::HashMap::new(),
            file_path: None,
            size_bytes: 1024,
        };

        let path = engine.get_storage_path(&record).unwrap();
        assert!(path.to_string_lossy().contains("image"));
        assert!(path.to_string_lossy().contains(&record.id.to_string()));
    }

    #[tokio::test]
    async fn test_storage_lists_loads_and_stats_persisted_records() {
        let temp_dir = tempdir().unwrap();
        let engine = StorageEngine::new(test_config(temp_dir.path().to_path_buf())).unwrap();
        let drone_id = Uuid::new_v4();
        let mut session = test_session(drone_id, crate::SessionStatus::Ended, Utc::now());
        let record = test_record(&session, Utc::now());
        session.data_records.push(record.id);

        engine.store_data(&record).await.unwrap();
        engine.store_session(&session).await.unwrap();

        let loaded_record = engine.load_data(&record.id).await.unwrap().unwrap();
        assert_eq!(loaded_record.id, record.id);

        let loaded_sessions = engine
            .list_sessions(Some(drone_id), Some(10))
            .await
            .unwrap();
        assert_eq!(loaded_sessions.len(), 1);
        assert_eq!(loaded_sessions[0].id, session.id);

        let stats = engine.get_stats().await.unwrap();
        assert_eq!(stats.total_records, 1);
        assert_eq!(stats.sessions_count, 1);
        assert!(stats.total_size_bytes > 0);
        assert_eq!(
            stats.data_type_breakdown.get(&DataType::Telemetry),
            Some(&1)
        );
        assert_eq!(stats.oldest_record, Some(record.timestamp));
        assert_eq!(stats.newest_record, Some(record.timestamp));
    }

    #[tokio::test]
    async fn test_cleanup_before_date_removes_old_completed_sessions_and_audits() {
        let temp_dir = tempdir().unwrap();
        let engine = StorageEngine::new(test_config(temp_dir.path().to_path_buf())).unwrap();
        let cutoff = Utc::now() - chrono::Duration::days(1);
        let old_time = cutoff - chrono::Duration::hours(1);
        let new_time = cutoff + chrono::Duration::hours(1);

        let mut old_session = test_session(Uuid::new_v4(), crate::SessionStatus::Ended, old_time);
        let old_record = test_record(&old_session, old_time);
        old_session.data_records.push(old_record.id);
        engine.store_data(&old_record).await.unwrap();
        engine.store_session(&old_session).await.unwrap();

        let mut new_session = test_session(Uuid::new_v4(), crate::SessionStatus::Ended, new_time);
        let new_record = test_record(&new_session, new_time);
        new_session.data_records.push(new_record.id);
        engine.store_data(&new_record).await.unwrap();
        engine.store_session(&new_session).await.unwrap();

        let removed_bytes = engine.cleanup_before_date(cutoff).await.unwrap();

        assert!(removed_bytes > 0);
        assert!(engine
            .load_session(&old_session.id)
            .await
            .unwrap()
            .is_none());
        assert!(engine.load_data(&old_record.id).await.unwrap().is_none());
        assert!(engine
            .load_session(&new_session.id)
            .await
            .unwrap()
            .is_some());
        assert!(engine.load_data(&new_record.id).await.unwrap().is_some());

        let audit_path = temp_dir.path().join("audit").join("retention.jsonl");
        let audit = tokio::fs::read_to_string(audit_path).await.unwrap();
        assert!(audit.contains(&old_session.id.to_string()));
        assert!(audit.contains("\"removed_records\":1"));
    }

    #[tokio::test]
    async fn test_cleanup_before_date_refuses_active_session() {
        let temp_dir = tempdir().unwrap();
        let engine = StorageEngine::new(test_config(temp_dir.path().to_path_buf())).unwrap();
        let cutoff = Utc::now() - chrono::Duration::days(1);
        let old_time = cutoff - chrono::Duration::hours(1);
        let mut active_session =
            test_session(Uuid::new_v4(), crate::SessionStatus::Collecting, old_time);
        let active_record = test_record(&active_session, old_time);
        active_session.data_records.push(active_record.id);
        engine.store_data(&active_record).await.unwrap();
        engine.store_session(&active_session).await.unwrap();

        let err = engine.cleanup_before_date(cutoff).await.unwrap_err();

        assert!(err.to_string().contains("in-progress"));
        assert!(engine
            .load_session(&active_session.id)
            .await
            .unwrap()
            .is_some());
        assert!(engine.load_data(&active_record.id).await.unwrap().is_some());
    }
}
