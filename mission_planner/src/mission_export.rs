use crate::{Mission, MissionTelemetrySample};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use uuid::Uuid;

const MISSION_EXPORT_CRS: &str = "EPSG:4326";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionExportRequest {
    pub capture_provenance_ref: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MissionExportExtent {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionExport {
    pub mission_id: Uuid,
    pub crs: String,
    pub waypoint_count: usize,
    pub telemetry_feature_count: usize,
    pub telemetry_sample_count: usize,
    pub telemetry_extent: Option<MissionExportExtent>,
    pub plan_csv: String,
    pub telemetry_geojson: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionExportValidation {
    pub valid: bool,
    pub crs: String,
    pub waypoint_count: usize,
    pub telemetry_feature_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionExportErrorCode {
    TelemetryMissionMismatch,
    InvalidGeoJsonSchema,
    InvalidPlanCsvSchema,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionExportError {
    pub code: MissionExportErrorCode,
    pub message: String,
}

impl fmt::Display for MissionExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for MissionExportError {}

pub fn export_mission_plan_and_telemetry(
    mission: &Mission,
    telemetry: &[MissionTelemetrySample],
    request: MissionExportRequest,
) -> Result<MissionExport, MissionExportError> {
    if let Some(sample) = telemetry
        .iter()
        .find(|sample| sample.mission_id != mission.id)
    {
        return Err(MissionExportError {
            code: MissionExportErrorCode::TelemetryMissionMismatch,
            message: format!(
                "telemetry sample for mission {} cannot be exported with mission {}",
                sample.mission_id, mission.id
            ),
        });
    }

    let plan_csv = mission_plan_csv(mission);
    let telemetry_geojson = telemetry_track_geojson(mission, telemetry, request);
    let telemetry_feature_count = telemetry_geojson["features"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let telemetry_extent = telemetry_extent(telemetry);
    let export = MissionExport {
        mission_id: mission.id,
        crs: MISSION_EXPORT_CRS.to_string(),
        waypoint_count: mission.waypoints.len(),
        telemetry_feature_count,
        telemetry_sample_count: telemetry.len(),
        telemetry_extent,
        plan_csv,
        telemetry_geojson,
    };
    validate_mission_export_schema(&export)?;
    Ok(export)
}

pub fn validate_mission_export_schema(
    export: &MissionExport,
) -> Result<MissionExportValidation, MissionExportError> {
    if !export.plan_csv.starts_with(
        "sequence,waypoint_id,longitude,latitude,altitude_m,waypoint_type,speed_ms,heading_degrees,arrival_time,crs\n",
    ) {
        return Err(MissionExportError {
            code: MissionExportErrorCode::InvalidPlanCsvSchema,
            message: "mission plan CSV header is invalid".to_string(),
        });
    }
    if export.telemetry_geojson["type"] != "FeatureCollection" {
        return Err(invalid_geojson(
            "telemetry export must be a FeatureCollection",
        ));
    }
    if export.telemetry_geojson["crs"]["properties"]["name"] != MISSION_EXPORT_CRS {
        return Err(invalid_geojson("telemetry export CRS is not EPSG:4326"));
    }
    let Some(features) = export.telemetry_geojson["features"].as_array() else {
        return Err(invalid_geojson(
            "telemetry export features must be an array",
        ));
    };
    for feature in features {
        if feature["type"] != "Feature" || feature["geometry"]["type"] != "LineString" {
            return Err(invalid_geojson("telemetry feature must be a LineString"));
        }
        let Some(coordinates) = feature["geometry"]["coordinates"].as_array() else {
            return Err(invalid_geojson("telemetry coordinates must be an array"));
        };
        if coordinates.is_empty() {
            return Err(invalid_geojson(
                "non-empty telemetry feature has no coordinates",
            ));
        }
    }
    Ok(MissionExportValidation {
        valid: true,
        crs: MISSION_EXPORT_CRS.to_string(),
        waypoint_count: export.waypoint_count,
        telemetry_feature_count: features.len(),
    })
}

fn mission_plan_csv(mission: &Mission) -> String {
    let mut csv = String::from(
        "sequence,waypoint_id,longitude,latitude,altitude_m,waypoint_type,speed_ms,heading_degrees,arrival_time,crs\n",
    );
    for (index, waypoint) in mission.waypoints.iter().enumerate() {
        csv.push_str(&format!(
            "{},{},{:.7},{:.7},{:.3},{},{},{},{},{}\n",
            index + 1,
            waypoint.id,
            waypoint.position.x(),
            waypoint.position.y(),
            waypoint.altitude_m,
            csv_escape(&format!("{:?}", waypoint.waypoint_type)),
            waypoint
                .speed_ms
                .map(|speed| format!("{speed:.3}"))
                .unwrap_or_default(),
            waypoint
                .heading_degrees
                .map(|heading| format!("{heading:.3}"))
                .unwrap_or_default(),
            waypoint
                .arrival_time
                .map(|arrival| arrival.to_rfc3339())
                .unwrap_or_default(),
            MISSION_EXPORT_CRS
        ));
    }
    csv
}

fn telemetry_track_geojson(
    mission: &Mission,
    telemetry: &[MissionTelemetrySample],
    request: MissionExportRequest,
) -> Value {
    let mut samples = telemetry.to_vec();
    samples.sort_by_key(|sample| sample.telemetry.timestamp);
    let coordinates = samples
        .iter()
        .map(|sample| {
            json!([
                sample.telemetry.position.longitude,
                sample.telemetry.position.latitude,
                sample.telemetry.position.altitude
            ])
        })
        .collect::<Vec<_>>();
    let features = if coordinates.is_empty() {
        Vec::new()
    } else {
        vec![json!({
            "type": "Feature",
            "properties": {
                "mission_id": mission.id,
                "sample_count": samples.len(),
                "started_at": samples.first().map(|sample| sample.telemetry.timestamp.to_rfc3339()),
                "ended_at": samples.last().map(|sample| sample.telemetry.timestamp.to_rfc3339()),
                "drone_ids": drone_ids(&samples),
                "capture_provenance_ref": request.capture_provenance_ref,
            },
            "geometry": {
                "type": "LineString",
                "coordinates": coordinates,
            }
        })]
    };
    json!({
        "type": "FeatureCollection",
        "crs": {
            "type": "name",
            "properties": {
                "name": MISSION_EXPORT_CRS,
            }
        },
        "bbox": telemetry_extent(telemetry).map(|extent| {
            vec![extent.min_lon, extent.min_lat, extent.max_lon, extent.max_lat]
        }),
        "features": features,
    })
}

fn telemetry_extent(telemetry: &[MissionTelemetrySample]) -> Option<MissionExportExtent> {
    let mut points = telemetry.iter();
    let first = points.next()?;
    let mut extent = MissionExportExtent {
        min_lon: first.telemetry.position.longitude,
        min_lat: first.telemetry.position.latitude,
        max_lon: first.telemetry.position.longitude,
        max_lat: first.telemetry.position.latitude,
    };
    for sample in points {
        extent.min_lon = extent.min_lon.min(sample.telemetry.position.longitude);
        extent.min_lat = extent.min_lat.min(sample.telemetry.position.latitude);
        extent.max_lon = extent.max_lon.max(sample.telemetry.position.longitude);
        extent.max_lat = extent.max_lat.max(sample.telemetry.position.latitude);
    }
    Some(extent)
}

fn drone_ids(samples: &[MissionTelemetrySample]) -> Vec<String> {
    let mut ids = samples
        .iter()
        .map(|sample| sample.drone_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn invalid_geojson(message: &str) -> MissionExportError {
    MissionExportError {
        code: MissionExportErrorCode::InvalidGeoJsonSchema,
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, MissionStatus, Waypoint, WaypointType, WeatherConstraints};
    use chrono::{TimeZone, Utc};
    use geo::{LineString, Point, Polygon};
    use shared::schemas::{GpsCoords, Telemetry};

    #[test]
    fn completed_mission_exports_plan_csv_and_geojson_track() {
        let mission = sample_mission();
        let telemetry = vec![
            telemetry_sample(mission.id, "drone-1", 102, 41.002, -96.004),
            telemetry_sample(mission.id, "drone-1", 100, 41.000, -96.000),
            telemetry_sample(mission.id, "drone-1", 101, 41.001, -96.002),
        ];

        let export = export_mission_plan_and_telemetry(
            &mission,
            &telemetry,
            MissionExportRequest {
                capture_provenance_ref: Some("capture:session-alpha".to_string()),
            },
        )
        .expect("completed mission should export");

        assert_eq!(export.mission_id, mission.id);
        assert_eq!(export.crs, "EPSG:4326");
        assert_eq!(export.waypoint_count, 2);
        assert_eq!(export.telemetry_feature_count, 1);
        assert_eq!(export.telemetry_sample_count, 3);
        assert!(export.plan_csv.contains("sequence,waypoint_id,longitude"));
        assert!(export.plan_csv.contains("EPSG:4326"));
        assert_eq!(export.telemetry_geojson["type"], "FeatureCollection");
        assert_eq!(
            export.telemetry_geojson["crs"]["properties"]["name"],
            "EPSG:4326"
        );
        let coordinates = export.telemetry_geojson["features"][0]["geometry"]["coordinates"]
            .as_array()
            .expect("coordinates");
        assert_eq!(coordinates.len(), 3);
        assert_eq!(coordinates[0], json!([-96.0, 41.0, 400.0]));
        assert_eq!(
            export.telemetry_geojson["features"][0]["properties"]["capture_provenance_ref"],
            "capture:session-alpha"
        );
        assert_eq!(
            export.telemetry_extent,
            Some(MissionExportExtent {
                min_lon: -96.004,
                min_lat: 41.0,
                max_lon: -96.0,
                max_lat: 41.002,
            })
        );
    }

    #[test]
    fn mission_export_with_no_telemetry_produces_valid_empty_feature_collection() {
        let mission = sample_mission();

        let export = export_mission_plan_and_telemetry(
            &mission,
            &[],
            MissionExportRequest {
                capture_provenance_ref: None,
            },
        )
        .expect("empty telemetry should still export");

        assert_eq!(export.telemetry_feature_count, 0);
        assert_eq!(export.telemetry_sample_count, 0);
        assert_eq!(export.telemetry_extent, None);
        assert!(export.telemetry_geojson["features"]
            .as_array()
            .expect("features")
            .is_empty());
        assert!(
            validate_mission_export_schema(&export)
                .expect("empty telemetry export schema is valid")
                .valid
        );
    }

    #[test]
    fn mission_export_rejects_telemetry_from_another_mission() {
        let mission = sample_mission();
        let other_mission_id = Uuid::new_v4();
        let error = export_mission_plan_and_telemetry(
            &mission,
            &[telemetry_sample(
                other_mission_id,
                "drone-1",
                100,
                41.0,
                -96.0,
            )],
            MissionExportRequest {
                capture_provenance_ref: None,
            },
        )
        .expect_err("mixed mission telemetry should reject");

        assert_eq!(error.code, MissionExportErrorCode::TelemetryMissionMismatch);
    }

    fn sample_mission() -> Mission {
        let id = Uuid::new_v4();
        Mission {
            id,
            name: "North Block".to_string(),
            description: "export fixture".to_string(),
            created_at: Utc.timestamp_opt(90, 0).unwrap(),
            updated_at: Utc.timestamp_opt(95, 0).unwrap(),
            version: 1,
            field_id: "field-alpha".to_string(),
            season_id: "season-2026".to_string(),
            session_id: Some("session-alpha".to_string()),
            owner_id: "owner-alpha".to_string(),
            status: MissionStatus::Completed,
            area_of_interest: Polygon::new(
                LineString::from(vec![
                    (-96.01, 41.00),
                    (-96.00, 41.00),
                    (-96.00, 41.01),
                    (-96.01, 41.01),
                    (-96.01, 41.00),
                ]),
                Vec::new(),
            ),
            waypoints: vec![
                Waypoint {
                    id: Uuid::new_v4(),
                    position: Point::new(-96.0, 41.0),
                    altitude_m: 40.0,
                    waypoint_type: WaypointType::Takeoff,
                    actions: vec![Action::SetSpeed { speed_ms: 6.0 }],
                    arrival_time: Some(Utc.timestamp_opt(100, 0).unwrap()),
                    speed_ms: Some(6.0),
                    heading_degrees: Some(90.0),
                },
                Waypoint {
                    id: Uuid::new_v4(),
                    position: Point::new(-96.004, 41.002),
                    altitude_m: 42.0,
                    waypoint_type: WaypointType::Landing,
                    actions: Vec::new(),
                    arrival_time: Some(Utc.timestamp_opt(110, 0).unwrap()),
                    speed_ms: Some(5.0),
                    heading_degrees: Some(180.0),
                },
            ],
            flight_paths: Vec::new(),
            estimated_duration_minutes: 8,
            estimated_battery_usage: 22.0,
            weather_constraints: WeatherConstraints {
                max_wind_speed_ms: 10.0,
                max_precipitation_mm: 0.0,
                min_visibility_m: 2_000.0,
                temperature_range_celsius: (5.0, 35.0),
            },
            metadata: Default::default(),
        }
    }

    fn telemetry_sample(
        mission_id: Uuid,
        drone_id: &str,
        timestamp_seconds: i64,
        latitude: f64,
        longitude: f64,
    ) -> MissionTelemetrySample {
        MissionTelemetrySample {
            mission_id,
            drone_id: drone_id.to_string(),
            telemetry: Telemetry {
                timestamp: Utc.timestamp_opt(timestamp_seconds, 0).unwrap(),
                position: GpsCoords {
                    latitude,
                    longitude,
                    altitude: 400.0,
                },
                battery_voltage: 15.8,
                battery_percentage: 82,
                armed: true,
                mode: "AUTO".to_string(),
                ground_speed: 6.0,
                air_speed: 6.5,
                heading: 90.0,
                altitude_relative: 40.0,
            },
        }
    }
}
