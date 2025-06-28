use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::{FlightDataRecord as DataRecord, DataType, FlightSession as CollectionSession};

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
        let storage_path = self.get_record_path(record_id)?;
        
        if !storage_path.exists() {
            return Ok(None);
        }

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
            let record_path = session_path.join(
            format!("{}.json", record_id));
            // Note: This would need to load the actual record data
            // For now, we'll skip individual record storage in session export
            // Individual records are stored separately via store_data()
        }

        Ok(session_path)
    }

    /// List all stored sessions
    pub async fn list_sessions(&self, _drone_id: Option<uuid::Uuid>, _limit: Option<u32>) -> Result<Vec<crate::FlightSession>> {
        // TODO: Implement proper session listing with filtering
        Ok(Vec::new())
    }

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<StorageStatistics> {
        let mut stats = StorageStatistics::default();
        
        // Calculate total size and count files
        self.calculate_directory_stats(&self.config.base_path, &mut stats).await?;
        
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
        let filename = format!("{}_{}.json", record.data_type.to_string().to_lowercase(), record.id);
        
        Ok(self.config.base_path
            .join("records")
            .join(&date_path)
            .join(&filename))
    }

    fn get_record_path(&self, record_id: &Uuid) -> Result<PathBuf> {
        // For retrieval, we need to search for the record
        // This is a simplified implementation
        Ok(self.config.base_path
            .join("records")
            .join(format!("{}.json", record_id)))
    }

    fn get_session_path(&self, session_id: &Uuid) -> Result<PathBuf> {
        Ok(self.config.base_path
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
        let backup_path = self.config.base_path
            .join("backups")
            .join(format!("{}.backup", record.id));
        
        if let Some(parent) = backup_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::copy(original_path, &backup_path).await?;
        Ok(())
    }

    async fn calculate_directory_stats(&self, path: &Path, stats: &mut StorageStatistics) -> Result<()> {
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

    async fn scan_data_types(&self, path: &Path, breakdown: &mut HashMap<DataType, u32>) -> Result<()> {
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

    pub async fn store_data(&self, data: &crate::FlightDataRecord) -> Result<crate::FlightDataRecord> {
        let _storage_path = self.store_record(data).await?;
        Ok(data.clone())
    }

    pub async fn load_session(&self, _session_id: &Uuid) -> Result<Option<crate::FlightSession>> {
        // TODO: Implement session loading
        Ok(None)
    }

    pub async fn cleanup_before_date(&self, _cutoff_date: chrono::DateTime<chrono::Utc>) -> Result<u64> {
        // TODO: Implement cleanup functionality
        Ok(0)
    }

    pub async fn get_stats(&self) -> Result<crate::StorageStats> {
        // TODO: Implement stats gathering
        Ok(crate::StorageStats {
            total_records: 0,
            total_size_bytes: 0,
            sessions_count: 0,
            oldest_record: None,
            newest_record: None,
            data_type_breakdown: std::collections::HashMap::new(),
        })
    }

    pub async fn load_data(&self, _record_id: &uuid::Uuid) -> Result<Option<crate::FlightDataRecord>> {
        // TODO: Implement data loading
        Ok(None)
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
    use tempfile::tempdir;

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
            flight_id: Uuid::new_v4(),
            drone_id: Uuid::new_v4(),
            data_type: DataType::Image,
            timestamp: Utc::now(),
            payload: crate::DataPayload::Raw {
                format: "test".to_string(),
                schema: None,
                compression: None,
            },
            metadata: std::collections::HashMap::new(),
            file_path: None,
            size_bytes: 1024,
        };

        let path = engine.get_storage_path(&record).unwrap();
        assert!(path.to_string_lossy().contains("camera"));
        assert!(path.to_string_lossy().contains(&record.id.to_string()));
    }
}
