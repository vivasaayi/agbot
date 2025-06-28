use std::collections::HashMap;
use uuid::Uuid;
use crate::{FlightDataRecord, DataType};

#[derive(Debug, Clone)]
pub struct IndexConfig {
    pub index_path: std::path::PathBuf,
    pub spatial_resolution: f64,
    pub temporal_resolution_hours: u32,
    pub enable_spatial_index: bool,
    pub enable_temporal_index: bool,
    pub enable_type_index: bool,
    pub spatial_grid_size: f64,
    pub temporal_bucket_size: chrono::Duration,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            index_path: std::path::PathBuf::from("./index"),
            spatial_resolution: 100.0,
            temporal_resolution_hours: 1,
            enable_spatial_index: true,
            enable_temporal_index: true,
            enable_type_index: true,
            spatial_grid_size: 0.001, // ~100m at equator
            temporal_bucket_size: chrono::Duration::minutes(5),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub time_range: Option<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>,
    pub spatial_bounds: Option<(f64, f64, f64, f64)>, // min_lat, min_lon, max_lat, max_lon
    pub data_types: Option<Vec<DataType>>,
    pub drone_ids: Option<Vec<Uuid>>,
    pub limit: Option<usize>,
}

#[derive(Debug)]
pub struct DataIndexer {
    config: IndexConfig,
    spatial_index: HashMap<SpatialKey, Vec<Uuid>>,
    temporal_index: HashMap<TemporalKey, Vec<Uuid>>,
    type_index: HashMap<DataType, Vec<Uuid>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SpatialKey {
    lat_bucket: i64,
    lon_bucket: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TemporalKey {
    timestamp_bucket: i64,
}

impl DataIndexer {
    pub fn new(config: IndexConfig) -> Self {
        Self {
            config,
            spatial_index: HashMap::new(),
            temporal_index: HashMap::new(),
            type_index: HashMap::new(),
        }
    }

    pub fn index_record(&mut self, record: &FlightDataRecord) {
        if self.config.enable_spatial_index {
            let spatial_key = self.get_spatial_key(&record.payload);
            self.spatial_index
                .entry(spatial_key)
                .or_default()
                .push(record.id);
        }

        if self.config.enable_temporal_index {
            let temporal_key = self.get_temporal_key(record.timestamp);
            self.temporal_index
                .entry(temporal_key)
                .or_default()
                .push(record.id);
        }

        if self.config.enable_type_index {
            self.type_index
                .entry(record.data_type.clone())
                .or_default()
                .push(record.id);
        }
    }

    pub fn find_by_location(
        &self,
        lat: f64,
        lon: f64,
        radius_deg: f64,
    ) -> Vec<Uuid> {
        if !self.config.enable_spatial_index {
            return Vec::new();
        }

        let mut results = Vec::new();
        let grid_size = self.config.spatial_grid_size;
        let bucket_radius = (radius_deg / grid_size).ceil() as i64;

        let center_lat_bucket = (lat / grid_size).floor() as i64;
        let center_lon_bucket = (lon / grid_size).floor() as i64;

        for lat_offset in -bucket_radius..=bucket_radius {
            for lon_offset in -bucket_radius..=bucket_radius {
                let spatial_key = SpatialKey {
                    lat_bucket: center_lat_bucket + lat_offset,
                    lon_bucket: center_lon_bucket + lon_offset,
                };

                if let Some(record_ids) = self.spatial_index.get(&spatial_key) {
                    results.extend(record_ids.iter().cloned());
                }
            }
        }

        results
    }

    pub fn find_by_time_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Vec<Uuid> {
        if !self.config.enable_temporal_index {
            return Vec::new();
        }

        let mut results = Vec::new();
        let bucket_size = self.config.temporal_bucket_size.num_seconds();

        let start_bucket = start.timestamp() / bucket_size;
        let end_bucket = end.timestamp() / bucket_size;

        for bucket in start_bucket..=end_bucket {
            let temporal_key = TemporalKey {
                timestamp_bucket: bucket,
            };

            if let Some(record_ids) = self.temporal_index.get(&temporal_key) {
                results.extend(record_ids.iter().cloned());
            }
        }

        results
    }

    pub fn find_by_type(&self, data_type: &DataType) -> Vec<Uuid> {
        if !self.config.enable_type_index {
            return Vec::new();
        }

        self.type_index
            .get(data_type)
            .map(|ids| ids.clone())
            .unwrap_or_default()
    }

    pub fn rebuild_indices(&mut self, records: &[FlightDataRecord]) {
        self.spatial_index.clear();
        self.temporal_index.clear();
        self.type_index.clear();

        for record in records {
            self.index_record(record);
        }
    }

    fn get_spatial_key(&self, payload: &crate::DataPayload) -> SpatialKey {
        let grid_size = self.config.spatial_grid_size;
        match payload {
            crate::DataPayload::Telemetry { position, .. } => {
                SpatialKey {
                    lat_bucket: (position.0 / grid_size).floor() as i64,
                    lon_bucket: (position.1 / grid_size).floor() as i64,
                }
            }
            _ => SpatialKey {
                lat_bucket: 0,
                lon_bucket: 0,
            }
        }
    }

    fn get_temporal_key(&self, timestamp: chrono::DateTime<chrono::Utc>) -> TemporalKey {
        let bucket_size = self.config.temporal_bucket_size.num_seconds();
        TemporalKey {
            timestamp_bucket: timestamp.timestamp() / bucket_size,
        }
    }

    pub fn get_statistics(&self) -> IndexStatistics {
        IndexStatistics {
            spatial_buckets: self.spatial_index.len(),
            temporal_buckets: self.temporal_index.len(),
            type_buckets: self.type_index.len(),
            total_indexed_records: self.spatial_index.values().map(|v| v.len()).sum(),
        }
    }

    pub async fn index_session(&mut self, _session: &crate::FlightSession) -> anyhow::Result<()> {
        // TODO: Implement session indexing
        Ok(())
    }

    pub async fn search(&self, _query: SearchQuery) -> anyhow::Result<Vec<crate::FlightDataRecord>> {
        // TODO: Implement search functionality
        Ok(Vec::new())
    }

    pub async fn rebuild(&mut self) -> anyhow::Result<()> {
        // TODO: Implement index rebuilding
        self.spatial_index.clear();
        self.temporal_index.clear();
        self.type_index.clear();
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct IndexStatistics {
    pub spatial_buckets: usize,
    pub temporal_buckets: usize,
    pub type_buckets: usize,
    pub total_indexed_records: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    #[test]
    fn test_spatial_indexing() {
        let config = IndexConfig::default();
        let mut indexer = DataIndexer::new(config);

        let record = FlightDataRecord {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            drone_id: Uuid::new_v4(),
            flight_id: Uuid::new_v4(),
            data_type: DataType::Telemetry,
            payload: crate::DataPayload::Telemetry {
                position: (40.7128, -74.0060, 100.0), // NYC coordinates
                velocity: (1.0, 0.0, 0.0),
                orientation: (0.0, 0.0, 0.0),
                battery_level: 0.8,
                signal_strength: 0.9,
            },
            metadata: HashMap::new(),
            file_path: None,
            size_bytes: 0,
        };

        indexer.index_record(&record);

        let found = indexer.find_by_location(40.7128, -74.0060, 0.001);
        assert!(!found.is_empty());
        assert!(found.contains(&record.id));
    }

    #[test]
    fn test_temporal_indexing() {
        let config = IndexConfig::default();
        let mut indexer = DataIndexer::new(config);

        let now = Utc::now();
        let record = FlightDataRecord {
            id: Uuid::new_v4(),
            timestamp: now,
            drone_id: Uuid::new_v4(),
            flight_id: Uuid::new_v4(),
            data_type: DataType::Telemetry,
            payload: crate::DataPayload::Telemetry {
                position: (0.0, 0.0, 0.0),
                velocity: (1.0, 0.0, 0.0),
                orientation: (0.0, 0.0, 0.0),
                battery_level: 0.8,
                signal_strength: 0.9,
            },
            metadata: HashMap::new(),
            file_path: None,
            size_bytes: 0,
        };

        indexer.index_record(&record);

        let found = indexer.find_by_time_range(
            now - chrono::Duration::minutes(1),
            now + chrono::Duration::minutes(1),
        );
        assert!(!found.is_empty());
        assert!(found.contains(&record.id));
    }
}
