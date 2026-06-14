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
    Kml,
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
            ExportFormat::Kml => self.export_kml(session, records, output_path).await,
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

    async fn export_kml(
        &self,
        session: &FlightSession,
        records: &[FlightDataRecord],
        output_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let file = File::create(output_path)?;
        let mut writer = BufWriter::new(file);
        let geo_records = georeferenced_records(records);
        let extent = GeospatialExtent::from_records(&geo_records);
        let resolution_m = geospatial_resolution_m(&geo_records);

        writeln!(writer, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")?;
        writeln!(
            writer,
            "<kml xmlns=\"http://www.opengis.net/kml/2.2\" xmlns:agbot=\"https://agbot.local/schema/export/1\">"
        )?;
        write!(
            writer,
            "  <Document agbot:session_id=\"{}\" agbot:field_id=\"{}\" agbot:crs=\"EPSG:4326\"",
            session.id, session.field_id
        )?;
        if let Some(extent) = &extent {
            write!(
                writer,
                " agbot:min_lat=\"{}\" agbot:max_lat=\"{}\" agbot:min_lon=\"{}\" agbot:max_lon=\"{}\" agbot:min_alt=\"{}\" agbot:max_alt=\"{}\"",
                extent.min_lat,
                extent.max_lat,
                extent.min_lon,
                extent.max_lon,
                extent.min_alt,
                extent.max_alt
            )?;
        }
        writeln!(
            writer,
            " agbot:resolution_m=\"{}\" agbot:record_count=\"{}\">",
            resolution_m,
            geo_records.len()
        )?;
        writeln!(
            writer,
            "    <name>{}</name>",
            xml_escape(&session.id.to_string())
        )?;

        if let Some(extent) = &extent {
            writeln!(writer, "    <Region>")?;
            writeln!(writer, "      <LatLonAltBox>")?;
            writeln!(writer, "        <north>{}</north>", extent.max_lat)?;
            writeln!(writer, "        <south>{}</south>", extent.min_lat)?;
            writeln!(writer, "        <east>{}</east>", extent.max_lon)?;
            writeln!(writer, "        <west>{}</west>", extent.min_lon)?;
            writeln!(
                writer,
                "        <minAltitude>{}</minAltitude>",
                extent.min_alt
            )?;
            writeln!(
                writer,
                "        <maxAltitude>{}</maxAltitude>",
                extent.max_alt
            )?;
            writeln!(writer, "      </LatLonAltBox>")?;
            writeln!(writer, "    </Region>")?;
        }

        for record in geo_records {
            writeln!(writer, "    <Placemark>")?;
            writeln!(
                writer,
                "      <name>{}</name>",
                xml_escape(&record.id.to_string())
            )?;
            writeln!(writer, "      <ExtendedData>")?;
            write_data_element(&mut writer, "record_id", &record.id.to_string())?;
            write_data_element(&mut writer, "session_id", &record.session_id.to_string())?;
            write_data_element(&mut writer, "data_type", &format!("{:?}", record.data_type))?;
            write_data_element(&mut writer, "sensor_id", &record.sensor_id)?;
            write_data_element(&mut writer, "timestamp", &record.timestamp.to_rfc3339())?;
            write_data_element(&mut writer, "calibration_ref", &record.calibration_ref)?;
            writeln!(writer, "      </ExtendedData>")?;
            if let Some(coords) = &record.gps_coords {
                writeln!(writer, "      <Point>")?;
                writeln!(
                    writer,
                    "        <coordinates>{},{},{}</coordinates>",
                    coords.longitude, coords.latitude, coords.altitude
                )?;
                writeln!(writer, "      </Point>")?;
            }
            writeln!(writer, "    </Placemark>")?;
        }

        writeln!(writer, "  </Document>")?;
        writeln!(writer, "</kml>")?;
        writer.flush()?;
        Ok(())
    }

    async fn export_parquet(
        &self,
        _session: &FlightSession,
        _records: &[FlightDataRecord],
        _output_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err(unsupported_export("Parquet"))
    }

    async fn export_hdf5(
        &self,
        _session: &FlightSession,
        _records: &[FlightDataRecord],
        _output_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err(unsupported_export("HDF5"))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ExportData {
    session: FlightSession,
    records: Vec<FlightDataRecord>,
}

#[derive(Debug, Clone)]
struct GeospatialExtent {
    min_lat: f64,
    max_lat: f64,
    min_lon: f64,
    max_lon: f64,
    min_alt: f64,
    max_alt: f64,
}

impl GeospatialExtent {
    fn from_records(records: &[FlightDataRecord]) -> Option<Self> {
        let mut coords = records
            .iter()
            .filter_map(|record| record.gps_coords.as_ref());
        let first = coords.next()?;
        let mut extent = Self {
            min_lat: first.latitude,
            max_lat: first.latitude,
            min_lon: first.longitude,
            max_lon: first.longitude,
            min_alt: first.altitude,
            max_alt: first.altitude,
        };

        for coord in coords {
            extent.min_lat = extent.min_lat.min(coord.latitude);
            extent.max_lat = extent.max_lat.max(coord.latitude);
            extent.min_lon = extent.min_lon.min(coord.longitude);
            extent.max_lon = extent.max_lon.max(coord.longitude);
            extent.min_alt = extent.min_alt.min(coord.altitude);
            extent.max_alt = extent.max_alt.max(coord.altitude);
        }

        Some(extent)
    }
}

fn georeferenced_records(records: &[FlightDataRecord]) -> Vec<FlightDataRecord> {
    let mut geo_records = records
        .iter()
        .filter(|record| record.gps_coords.is_some())
        .cloned()
        .collect::<Vec<_>>();
    geo_records.sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then_with(|| a.id.cmp(&b.id)));
    geo_records
}

fn geospatial_resolution_m(records: &[FlightDataRecord]) -> String {
    let coords = records
        .iter()
        .filter_map(|record| record.gps_coords.as_ref())
        .collect::<Vec<_>>();
    if coords.len() < 2 {
        return "0".to_string();
    }

    let mut min_distance = f64::INFINITY;
    for (index, left) in coords.iter().enumerate() {
        for right in coords.iter().skip(index + 1) {
            let distance = haversine_m(
                left.latitude,
                left.longitude,
                right.latitude,
                right.longitude,
            );
            if distance.is_finite() && distance < min_distance {
                min_distance = distance;
            }
        }
    }

    format!("{min_distance:.3}")
}

fn haversine_m(left_lat: f64, left_lon: f64, right_lat: f64, right_lon: f64) -> f64 {
    let lat1 = left_lat.to_radians();
    let lat2 = right_lat.to_radians();
    let d_lat = (right_lat - left_lat).to_radians();
    let d_lon = (right_lon - left_lon).to_radians();
    let a = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin().powi(2);
    6_371_000.0 * 2.0 * a.sqrt().atan2((1.0 - a).sqrt())
}

fn write_data_element(
    writer: &mut BufWriter<File>,
    name: &str,
    value: &str,
) -> std::io::Result<()> {
    writeln!(
        writer,
        "        <Data name=\"{}\"><value>{}</value></Data>",
        xml_escape(name),
        xml_escape(value)
    )
}

fn unsupported_export(format: &str) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("{format} export is not enabled in this build"),
    ))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
