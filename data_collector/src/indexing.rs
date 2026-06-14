use crate::{DataType, FlightDataRecord};
use std::collections::HashMap;
use uuid::Uuid;

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
    records: HashMap<Uuid, FlightDataRecord>,
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
            records: HashMap::new(),
            spatial_index: HashMap::new(),
            temporal_index: HashMap::new(),
            type_index: HashMap::new(),
        }
    }

    pub fn index_record(&mut self, record: &FlightDataRecord) {
        self.records.insert(record.id, record.clone());

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

    pub fn find_by_location(&self, lat: f64, lon: f64, radius_deg: f64) -> Vec<Uuid> {
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
        self.records.clear();
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
            crate::DataPayload::Telemetry { position, .. } => SpatialKey {
                lat_bucket: (position.0 / grid_size).floor() as i64,
                lon_bucket: (position.1 / grid_size).floor() as i64,
            },
            _ => SpatialKey {
                lat_bucket: 0,
                lon_bucket: 0,
            },
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
        Ok(())
    }

    pub async fn search(&self, query: SearchQuery) -> anyhow::Result<Vec<crate::FlightDataRecord>> {
        let records = self.records.values().cloned().collect::<Vec<_>>();
        Ok(Self::filter_records(&records, &query))
    }

    pub async fn rebuild(&mut self) -> anyhow::Result<()> {
        self.records.clear();
        self.spatial_index.clear();
        self.temporal_index.clear();
        self.type_index.clear();
        Ok(())
    }

    pub async fn rebuild_from_records(
        &mut self,
        records: &[FlightDataRecord],
    ) -> anyhow::Result<()> {
        self.rebuild_indices(records);
        Ok(())
    }

    pub fn filter_records(
        records: &[FlightDataRecord],
        query: &SearchQuery,
    ) -> Vec<FlightDataRecord> {
        let mut results = records
            .iter()
            .filter(|record| Self::record_matches_query(record, query))
            .cloned()
            .collect::<Vec<_>>();

        results.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then_with(|| a.id.as_bytes().cmp(b.id.as_bytes()))
        });
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        results
    }

    fn record_matches_query(record: &FlightDataRecord, query: &SearchQuery) -> bool {
        if let Some((start, end)) = &query.time_range {
            if record.timestamp < *start || record.timestamp > *end {
                return false;
            }
        }

        if let Some((min_lat, min_lon, max_lat, max_lon)) = query.spatial_bounds {
            let Some((lat, lon)) = Self::record_lat_lon(record) else {
                return false;
            };
            if lat < min_lat || lat > max_lat || lon < min_lon || lon > max_lon {
                return false;
            }
        }

        if let Some(data_types) = &query.data_types {
            if !data_types.contains(&record.data_type) {
                return false;
            }
        }

        if let Some(drone_ids) = &query.drone_ids {
            if !drone_ids.contains(&record.drone_id) {
                return false;
            }
        }

        true
    }

    fn record_lat_lon(record: &FlightDataRecord) -> Option<(f64, f64)> {
        if let Some(coords) = &record.gps_coords {
            return Some((coords.latitude, coords.longitude));
        }

        match &record.payload {
            crate::DataPayload::Telemetry { position, .. } => Some((position.0, position.1)),
            _ => None,
        }
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
    use shared::schemas::GpsCoords;
    use std::collections::HashMap;

    fn test_record(
        data_type: DataType,
        drone_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
        latitude: f64,
        longitude: f64,
    ) -> FlightDataRecord {
        FlightDataRecord {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            timestamp,
            drone_id,
            flight_id: Uuid::new_v4(),
            data_type,
            payload: crate::DataPayload::Telemetry {
                position: (latitude, longitude, 100.0),
                velocity: (1.0, 0.0, 0.0),
                orientation: (0.0, 0.0, 0.0),
                battery_level: 0.8,
                signal_strength: 0.9,
            },
            sensor_id: "telemetry-01".to_string(),
            gps_coords: Some(GpsCoords {
                latitude,
                longitude,
                altitude: 100.0,
            }),
            calibration_ref: "calibration-2026-06".to_string(),
            metadata: HashMap::new(),
            file_path: None,
            size_bytes: 128,
        }
    }

    #[test]
    fn test_spatial_indexing() {
        let config = IndexConfig::default();
        let mut indexer = DataIndexer::new(config);

        let record = FlightDataRecord {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
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
            sensor_id: "telemetry-01".to_string(),
            gps_coords: Some(GpsCoords {
                latitude: 40.7128,
                longitude: -74.0060,
                altitude: 100.0,
            }),
            calibration_ref: "calibration-2026-06".to_string(),
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
            session_id: Uuid::new_v4(),
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
            sensor_id: "telemetry-01".to_string(),
            gps_coords: Some(GpsCoords {
                latitude: 0.0,
                longitude: 0.0,
                altitude: 0.0,
            }),
            calibration_ref: "calibration-2026-06".to_string(),
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

    #[tokio::test]
    async fn search_filters_indexed_records_by_space_time_type_and_drone() {
        let config = IndexConfig::default();
        let mut indexer = DataIndexer::new(config);
        let drone_id = Uuid::new_v4();
        let now = Utc::now();
        let matching = test_record(DataType::Telemetry, drone_id, now, 40.0, -105.0);
        let outside_bounds = test_record(DataType::Telemetry, drone_id, now, 41.0, -106.0);
        let wrong_type = test_record(DataType::Image, drone_id, now, 40.0, -105.0);

        indexer.index_record(&matching);
        indexer.index_record(&outside_bounds);
        indexer.index_record(&wrong_type);

        let results = indexer
            .search(SearchQuery {
                time_range: Some((
                    now - chrono::Duration::minutes(1),
                    now + chrono::Duration::minutes(1),
                )),
                spatial_bounds: Some((39.9, -105.1, 40.1, -104.9)),
                data_types: Some(vec![DataType::Telemetry]),
                drone_ids: Some(vec![drone_id]),
                limit: Some(10),
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, matching.id);
    }

    #[tokio::test]
    async fn rebuild_from_records_recovers_stale_index() {
        let config = IndexConfig::default();
        let mut indexer = DataIndexer::new(config);
        let now = Utc::now();
        let stale = test_record(DataType::Image, Uuid::new_v4(), now, 0.0, 0.0);
        let rebuilt = test_record(DataType::Telemetry, Uuid::new_v4(), now, 40.0, -105.0);
        indexer.index_record(&stale);

        indexer
            .rebuild_from_records(&[rebuilt.clone()])
            .await
            .unwrap();

        let results = indexer
            .search(SearchQuery {
                time_range: None,
                spatial_bounds: None,
                data_types: Some(vec![DataType::Telemetry]),
                drone_ids: None,
                limit: None,
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, rebuilt.id);
    }
}
