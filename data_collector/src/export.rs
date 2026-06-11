use crate::{FlightDataRecord, FlightSession};
use serde_json;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ExportConfig {
    pub format: ExportFormat,
    pub include_metadata: bool,
    pub compress: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Csv,
    Parquet,
    HDF5,
}

#[derive(Debug)]
pub struct DataExporter {
    config: ExportConfig,
}

impl DataExporter {
    pub fn new(config: ExportConfig) -> Self {
        Self { config }
    }

    pub async fn export_session(
        &self,
        session: &FlightSession,
        records: &[FlightDataRecord],
        output_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self.config.format {
            ExportFormat::Json => self.export_json(session, records, output_path).await,
            ExportFormat::Csv => self.export_csv(session, records, output_path).await,
            ExportFormat::Parquet => self.export_parquet(session, records, output_path).await,
            ExportFormat::HDF5 => self.export_hdf5(session, records, output_path).await,
        }
    }

    async fn export_json(
        &self,
        session: &FlightSession,
        records: &[FlightDataRecord],
        output_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let file = File::create(output_path)?;
        let mut writer = BufWriter::new(file);

        if self.config.include_metadata {
            let export_data = ExportData {
                session: session.clone(),
                records: records.to_vec(),
            };
            serde_json::to_writer_pretty(&mut writer, &export_data)?;
        } else {
            serde_json::to_writer_pretty(&mut writer, records)?;
        }

        writer.flush()?;
        Ok(())
    }

    async fn export_csv(
        &self,
        _session: &FlightSession,
        records: &[FlightDataRecord],
        output_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut writer = csv::Writer::from_path(output_path)?;
        writer.write_record([
            "record_id",
            "session_id",
            "timestamp",
            "flight_id",
            "drone_id",
            "data_type",
            "sensor_id",
            "gps_lat",
            "gps_lon",
            "gps_alt",
            "calibration_ref",
            "size_bytes",
            "position_lat",
            "position_lon",
            "position_alt",
        ])?;

        for record in records {
            let (gps_lat, gps_lon, gps_alt) = record
                .gps_coords
                .as_ref()
                .map(|coords| {
                    (
                        coords.latitude.to_string(),
                        coords.longitude.to_string(),
                        coords.altitude.to_string(),
                    )
                })
                .unwrap_or_else(|| (String::new(), String::new(), String::new()));
            let (position_lat, position_lon, position_alt) = match &record.payload {
                crate::DataPayload::Telemetry { position, .. } => (
                    position.0.to_string(),
                    position.1.to_string(),
                    position.2.to_string(),
                ),
                _ => (String::new(), String::new(), String::new()),
            };

            writer.write_record(vec![
                record.id.to_string(),
                record.session_id.to_string(),
                record.timestamp.to_rfc3339(),
                record.flight_id.to_string(),
                record.drone_id.to_string(),
                format!("{:?}", record.data_type),
                record.sensor_id.clone(),
                gps_lat,
                gps_lon,
                gps_alt,
                record.calibration_ref.clone(),
                record.size_bytes.to_string(),
                position_lat,
                position_lon,
                position_alt,
            ])?;
        }

        writer.flush()?;
        Ok(())
    }

    async fn export_parquet(
        &self,
        _session: &FlightSession,
        _records: &[FlightDataRecord],
        _output_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // TODO: Implement Parquet export using arrow/parquet crates
        unimplemented!("Parquet export not yet implemented")
    }

    async fn export_hdf5(
        &self,
        _session: &FlightSession,
        _records: &[FlightDataRecord],
        _output_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // TODO: Implement HDF5 export using hdf5 crate
        unimplemented!("HDF5 export not yet implemented")
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ExportData {
    session: FlightSession,
    records: Vec<FlightDataRecord>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataType;
    use chrono::Utc;
    use shared::schemas::GpsCoords;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_json_export() {
        let config = ExportConfig {
            format: ExportFormat::Json,
            include_metadata: true,
            compress: false,
        };
        let exporter = DataExporter::new(config);

        let session = FlightSession {
            id: Uuid::new_v4(),
            flight_id: Uuid::new_v4(),
            field_id: Uuid::new_v4(),
            scene_id: Uuid::new_v4(),
            owner_id: "grower-ops".to_string(),
            mission_id: Some(Uuid::new_v4()),
            drone_id: Uuid::new_v4(),
            start_time: Utc::now(),
            end_time: None,
            status: crate::SessionStatus::Ended,
            data_records: Vec::new(),
            summary: crate::SessionSummary::default(),
            tags: Vec::new(),
        };

        let records = vec![FlightDataRecord {
            id: Uuid::new_v4(),
            session_id: session.id,
            flight_id: Uuid::new_v4(),
            drone_id: Uuid::new_v4(),
            timestamp: Utc::now(),
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
            metadata: std::collections::HashMap::new(),
            file_path: None,
            size_bytes: 256,
        }];

        let temp_path = std::env::temp_dir().join("test_export.json");
        let result = exporter
            .export_session(&session, &records, &temp_path)
            .await;
        assert!(result.is_ok());

        // Clean up
        let _ = std::fs::remove_file(&temp_path);
    }
}
