use std::path::Path;
use std::fs::File;
use std::io::{Write, BufWriter};
use serde_json;
use uuid::Uuid;
use crate::{FlightDataRecord, FlightSession, DataType};

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
        let file = File::create(output_path)?;
        let mut writer = BufWriter::new(file);
        
        // Write CSV header
        writeln!(writer, "timestamp,drone_id,data_type,position")?;
        
        for record in records {
            writeln!(
                writer,
                "{},{},{:?},{}",
                record.timestamp.timestamp(),
                record.drone_id,
                record.data_type,
                match &record.payload {
                    crate::DataPayload::Telemetry { position, .. } => format!("{},{},{}", position.0, position.1, position.2),
                    _ => ",,".to_string(),
                }
            )?;
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
    use chrono::Utc;
    use std::collections::HashMap;

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
            mission_id: Some(Uuid::new_v4()),
            drone_id: Uuid::new_v4(),
            start_time: Utc::now(),
            end_time: None,
            status: crate::SessionStatus::Completed,
            data_records: Vec::new(),
            summary: crate::SessionSummary::default(),
            tags: Vec::new(),
        };
        
        let records = vec![
            FlightDataRecord {
                id: Uuid::new_v4(),
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
                metadata: std::collections::HashMap::new(),
                file_path: None,
                size_bytes: 256,
            }
        ];
        
        let temp_path = std::env::temp_dir().join("test_export.json");
        let result = exporter.export_session(&session, &records, &temp_path).await;
        assert!(result.is_ok());
        
        // Clean up
        let _ = std::fs::remove_file(&temp_path);
    }
}
