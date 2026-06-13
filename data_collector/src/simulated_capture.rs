use crate::{
    lidar_scan_to_record, multispectral_capture_to_record, CollectionFailureKind,
    CollectionFailureRequest, DataPayload, DataType, FlightDataProvenance,
    FlightDataProvenanceError, FlightDataRecord, LidarRecordError, MultispectralBandCapture,
    MultispectralCaptureManifest, MultispectralRecordError,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::schemas::{GpsCoords, LidarPoint, LidarScan};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatedCaptureFrame {
    pub session_id: Uuid,
    pub flight_id: Uuid,
    pub drone_id: Uuid,
    pub simulation_mission_id: Uuid,
    pub observed_at: DateTime<Utc>,
    pub position: GpsCoords,
    pub observations: Vec<SimulatedSensorObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimulatedSensorObservation {
    Telemetry {
        sensor_id: String,
        calibration_ref: String,
        velocity: (f32, f32, f32),
        orientation: (f32, f32, f32),
        battery_level: f32,
        signal_strength: f32,
    },
    Lidar {
        sensor_id: String,
        calibration_ref: String,
        scan: LidarScan,
    },
    Multispectral {
        sensor_id: String,
        calibration_ref: String,
        manifest: MultispectralCaptureManifest,
    },
    Failure {
        sensor_id: String,
        data_type: DataType,
        kind: CollectionFailureKind,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatedCaptureBatch {
    pub records: Vec<FlightDataRecord>,
    pub failures: Vec<CollectionFailureRequest>,
}

#[derive(Debug, thiserror::Error)]
pub enum SimulatedCaptureError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Provenance(#[from] FlightDataProvenanceError),
    #[error(transparent)]
    Lidar(#[from] LidarRecordError),
    #[error(transparent)]
    Multispectral(#[from] MultispectralRecordError),
    #[error("flight_sim_cpp {sensor_type} output is invalid: {message}")]
    InvalidFlightSimCppOutput {
        sensor_type: &'static str,
        message: String,
    },
}

#[derive(Debug, Deserialize)]
struct FlightSimCppLidarScan {
    timestamp: DateTime<Utc>,
    points: Vec<FlightSimCppLidarPoint>,
    scan_id: Uuid,
    sensor_id: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct FlightSimCppLidarPoint {
    timestamp: DateTime<Utc>,
    angle: f32,
    range_m: f32,
    quality: u8,
}

#[derive(Debug, Deserialize)]
struct FlightSimCppMultispectralCapture {
    status: String,
    reason: String,
    bands: Vec<FlightSimCppMultispectralBand>,
}

#[derive(Debug, Deserialize)]
struct FlightSimCppMultispectralBand {
    name: String,
    width: u32,
    height: u32,
}

pub fn flight_sim_cpp_lidar_observation_from_json(
    json: &str,
    calibration_ref: impl Into<String>,
) -> Result<SimulatedSensorObservation, SimulatedCaptureError> {
    let scan: FlightSimCppLidarScan = serde_json::from_str(json)?;
    if scan.status != "ok" {
        return Ok(SimulatedSensorObservation::Failure {
            sensor_id: scan.sensor_id,
            data_type: DataType::LidarScan,
            kind: CollectionFailureKind::SensorDropout,
            message: format!("flight_sim_cpp lidar status {}", scan.status),
        });
    }
    if scan.points.is_empty() {
        return Err(SimulatedCaptureError::InvalidFlightSimCppOutput {
            sensor_type: "lidar",
            message: "ok scan contained no points".to_string(),
        });
    }

    Ok(SimulatedSensorObservation::Lidar {
        sensor_id: scan.sensor_id,
        calibration_ref: calibration_ref.into(),
        scan: LidarScan {
            timestamp: scan.timestamp,
            points: scan
                .points
                .into_iter()
                .map(|point| LidarPoint {
                    timestamp: point.timestamp,
                    angle: point.angle,
                    distance: point.range_m,
                    quality: point.quality,
                })
                .collect(),
            scan_id: scan.scan_id,
        },
    })
}

pub fn flight_sim_cpp_multispectral_observation_from_json(
    json: &str,
    sensor_id: impl Into<String>,
    calibration_ref: impl Into<String>,
) -> Result<SimulatedSensorObservation, SimulatedCaptureError> {
    let sensor_id = sensor_id.into();
    let capture: FlightSimCppMultispectralCapture = serde_json::from_str(json)?;
    if capture.status != "ok" {
        let message = if capture.reason.trim().is_empty() {
            format!("flight_sim_cpp multispectral status {}", capture.status)
        } else {
            format!(
                "flight_sim_cpp multispectral status {}: {}",
                capture.status, capture.reason
            )
        };
        return Ok(SimulatedSensorObservation::Failure {
            sensor_id,
            data_type: DataType::MultispectralImage,
            kind: CollectionFailureKind::SensorDropout,
            message,
        });
    }
    if capture.bands.is_empty() {
        return Err(SimulatedCaptureError::InvalidFlightSimCppOutput {
            sensor_type: "multispectral",
            message: "ok capture contained no bands".to_string(),
        });
    }

    let expected_bands = capture
        .bands
        .iter()
        .map(|band| band.name.clone())
        .collect::<Vec<_>>();
    let bands = capture
        .bands
        .into_iter()
        .map(|band| MultispectralBandCapture {
            file_path: PathBuf::from(format!("sim://flight_sim_cpp/{}.tif", band.name)),
            name: band.name,
            width: band.width,
            height: band.height,
            exposure_time_ms: 1,
            gain: 1.0,
        })
        .collect();

    Ok(SimulatedSensorObservation::Multispectral {
        sensor_id,
        calibration_ref: calibration_ref.into(),
        manifest: MultispectralCaptureManifest {
            expected_bands,
            bands,
        },
    })
}

pub fn simulated_capture_frame_to_batch(
    frame: SimulatedCaptureFrame,
) -> Result<SimulatedCaptureBatch, SimulatedCaptureError> {
    let mut records = Vec::new();
    let mut failures = Vec::new();
    let session_id = frame.session_id;
    let flight_id = frame.flight_id;
    let drone_id = frame.drone_id;
    let simulation_mission_id = frame.simulation_mission_id;
    let observed_at = frame.observed_at;
    let position = frame.position;

    for observation in frame.observations {
        match observation {
            SimulatedSensorObservation::Telemetry {
                sensor_id,
                calibration_ref,
                velocity,
                orientation,
                battery_level,
                signal_strength,
            } => {
                let provenance = provenance_for(
                    session_id,
                    &position,
                    observed_at,
                    sensor_id,
                    calibration_ref,
                );
                let mut record = FlightDataRecord::new(
                    flight_id,
                    drone_id,
                    DataType::Telemetry,
                    DataPayload::Telemetry {
                        position: (
                            position.latitude,
                            position.longitude,
                            position.altitude as f32,
                        ),
                        velocity,
                        orientation,
                        battery_level,
                        signal_strength,
                    },
                    provenance,
                    128,
                )?;
                tag_simulated_record(&mut record, simulation_mission_id);
                records.push(record);
            }
            SimulatedSensorObservation::Lidar {
                sensor_id,
                calibration_ref,
                scan,
            } => {
                let provenance = provenance_for(
                    session_id,
                    &position,
                    observed_at,
                    sensor_id,
                    calibration_ref,
                );
                let mut record = lidar_scan_to_record(flight_id, drone_id, &scan, provenance)?;
                tag_simulated_record(&mut record, simulation_mission_id);
                records.push(record);
            }
            SimulatedSensorObservation::Multispectral {
                sensor_id,
                calibration_ref,
                manifest,
            } => {
                let provenance = provenance_for(
                    session_id,
                    &position,
                    observed_at,
                    sensor_id,
                    calibration_ref,
                );
                let mut record =
                    multispectral_capture_to_record(flight_id, drone_id, &manifest, provenance)?;
                tag_simulated_record(&mut record, simulation_mission_id);
                records.push(record);
            }
            SimulatedSensorObservation::Failure {
                sensor_id,
                data_type,
                kind,
                message,
            } => failures.push(CollectionFailureRequest {
                occurred_at: Some(observed_at),
                sensor_id,
                data_type,
                kind,
                message,
            }),
        }
    }

    Ok(SimulatedCaptureBatch { records, failures })
}

fn provenance_for(
    session_id: Uuid,
    position: &GpsCoords,
    observed_at: DateTime<Utc>,
    sensor_id: String,
    calibration_ref: String,
) -> FlightDataProvenance {
    FlightDataProvenance::complete(
        session_id,
        sensor_id,
        position.clone(),
        observed_at,
        calibration_ref,
    )
}

fn tag_simulated_record(record: &mut FlightDataRecord, simulation_mission_id: Uuid) {
    record
        .metadata
        .insert("source".to_string(), "flight_sim_cpp".to_string());
    record
        .metadata
        .insert("runtime_mode".to_string(), "simulation".to_string());
    record.metadata.insert(
        "simulation_mission_id".to_string(),
        simulation_mission_id.to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MultispectralBandCapture, MultispectralCaptureManifest};
    use chrono::TimeZone;
    use shared::schemas::LidarPoint;
    use std::path::PathBuf;

    fn gps() -> GpsCoords {
        GpsCoords {
            latitude: 40.0,
            longitude: -96.0,
            altitude: 402.0,
        }
    }

    fn lidar_scan(timestamp: DateTime<Utc>) -> LidarScan {
        LidarScan {
            timestamp,
            points: vec![
                LidarPoint {
                    timestamp,
                    angle: 0.0,
                    distance: 4.0,
                    quality: 15,
                },
                LidarPoint {
                    timestamp,
                    angle: 90.0,
                    distance: 3.0,
                    quality: 14,
                },
            ],
            scan_id: Uuid::new_v4(),
        }
    }

    fn band(name: &str) -> MultispectralBandCapture {
        MultispectralBandCapture {
            name: name.to_string(),
            file_path: PathBuf::from(format!("sim://capture/{name}.tif")),
            width: 16,
            height: 12,
            exposure_time_ms: 8,
            gain: 1.0,
        }
    }

    fn multispectral_manifest() -> MultispectralCaptureManifest {
        MultispectralCaptureManifest {
            expected_bands: vec![
                "Red".to_string(),
                "Green".to_string(),
                "Blue".to_string(),
                "NIR".to_string(),
            ],
            bands: vec![band("Red"), band("Green"), band("Blue"), band("NIR")],
        }
    }

    fn frame_with(observations: Vec<SimulatedSensorObservation>) -> SimulatedCaptureFrame {
        SimulatedCaptureFrame {
            session_id: Uuid::new_v4(),
            flight_id: Uuid::new_v4(),
            drone_id: Uuid::new_v4(),
            simulation_mission_id: Uuid::new_v4(),
            observed_at: Utc.timestamp_opt(1_800_000_100, 0).unwrap(),
            position: gps(),
            observations,
        }
    }

    fn record_schema_without_origin(record: &FlightDataRecord) -> serde_json::Value {
        let mut value = serde_json::to_value(record).expect("record serializes");
        let object = value.as_object_mut().expect("record serializes to object");
        object.remove("id");
        if let Some(metadata) = object
            .get_mut("metadata")
            .and_then(serde_json::Value::as_object_mut)
        {
            metadata.remove("source");
            metadata.remove("runtime_mode");
            metadata.remove("simulation_mission_id");
        }
        value
    }

    #[test]
    fn simulated_adapter_matches_real_hardware_record_schema() {
        let timestamp = Utc.timestamp_opt(1_800_000_120, 0).unwrap();
        let scan = lidar_scan(timestamp);
        let manifest = multispectral_manifest();
        let frame = frame_with(vec![
            SimulatedSensorObservation::Lidar {
                sensor_id: "rplidar-a3-front".to_string(),
                calibration_ref: "rplidar-a3-cal-2026".to_string(),
                scan: scan.clone(),
            },
            SimulatedSensorObservation::Multispectral {
                sensor_id: "multispectral-front".to_string(),
                calibration_ref: "multispectral-cal-2026".to_string(),
                manifest: manifest.clone(),
            },
        ]);
        let session_id = frame.session_id;
        let flight_id = frame.flight_id;
        let drone_id = frame.drone_id;
        let simulation_mission_id = frame.simulation_mission_id;
        let observed_at = frame.observed_at;

        let direct_lidar = lidar_scan_to_record(
            flight_id,
            drone_id,
            &scan,
            FlightDataProvenance::complete(
                session_id,
                "rplidar-a3-front".to_string(),
                gps(),
                observed_at,
                "rplidar-a3-cal-2026".to_string(),
            ),
        )
        .expect("direct lidar record builds");
        let direct_multispectral = multispectral_capture_to_record(
            flight_id,
            drone_id,
            &manifest,
            FlightDataProvenance::complete(
                session_id,
                "multispectral-front".to_string(),
                gps(),
                observed_at,
                "multispectral-cal-2026".to_string(),
            ),
        )
        .expect("direct multispectral record builds");

        let batch = simulated_capture_frame_to_batch(frame).expect("sim frame converts");

        assert_eq!(batch.records.len(), 2);
        assert_eq!(
            record_schema_without_origin(&batch.records[0]),
            record_schema_without_origin(&direct_lidar)
        );
        assert_eq!(
            record_schema_without_origin(&batch.records[1]),
            record_schema_without_origin(&direct_multispectral)
        );
        assert!(batch
            .records
            .iter()
            .all(|record| record.metadata.get("simulation_mission_id")
                == Some(&simulation_mission_id.to_string())));
    }

    #[test]
    fn simulated_failure_uses_collection_failure_request_shape() {
        let frame = frame_with(vec![SimulatedSensorObservation::Failure {
            sensor_id: "rplidar-a3-front".to_string(),
            data_type: DataType::LidarScan,
            kind: CollectionFailureKind::MalformedFrame,
            message: "RPLIDAR frame is empty".to_string(),
        }]);
        let observed_at = frame.observed_at;

        let batch = simulated_capture_frame_to_batch(frame).expect("failure converts");

        assert_eq!(batch.failures.len(), 1);
        let failure = &batch.failures[0];
        assert_eq!(failure.occurred_at, Some(observed_at));
        assert_eq!(failure.sensor_id, "rplidar-a3-front");
        assert_eq!(failure.data_type, DataType::LidarScan);
        assert_eq!(failure.kind, CollectionFailureKind::MalformedFrame);
        assert_eq!(failure.message, "RPLIDAR frame is empty");
    }

    #[test]
    fn flight_sim_cpp_lidar_json_maps_range_m_into_shared_scan() {
        let json = r#"{
            "timestamp": "2027-01-15T10:30:00Z",
            "points": [
                {
                    "timestamp": "2027-01-15T10:30:00Z",
                    "angle": 12.5,
                    "range_m": 4.75,
                    "quality": 27
                }
            ],
            "scan_id": "11111111-1111-4111-8111-111111111111",
            "sensor_id": "sim-lidar-a3",
            "status": "ok"
        }"#;

        let observation = flight_sim_cpp_lidar_observation_from_json(json, "sim-lidar-v1")
            .expect("lidar observation parses");

        match observation {
            SimulatedSensorObservation::Lidar {
                sensor_id,
                calibration_ref,
                scan,
            } => {
                assert_eq!(sensor_id, "sim-lidar-a3");
                assert_eq!(calibration_ref, "sim-lidar-v1");
                assert_eq!(
                    scan.scan_id,
                    Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap()
                );
                assert_eq!(scan.points.len(), 1);
                assert_eq!(scan.points[0].angle, 12.5);
                assert_eq!(scan.points[0].distance, 4.75);
                assert_eq!(scan.points[0].quality, 27);
            }
            other => panic!("expected lidar observation, got {other:?}"),
        }
    }

    #[test]
    fn flight_sim_cpp_multispectral_json_maps_to_capture_manifest() {
        let json = r#"{
            "status": "ok",
            "reason": "",
            "bands": [
                { "name": "Red", "width": 16, "height": 12 },
                { "name": "Green", "width": 16, "height": 12 },
                { "name": "Blue", "width": 16, "height": 12 },
                { "name": "NIR", "width": 16, "height": 12 }
            ]
        }"#;

        let observation = flight_sim_cpp_multispectral_observation_from_json(
            json,
            "sim-multispectral",
            "sim-multispectral-v1",
        )
        .expect("multispectral observation parses");

        match observation {
            SimulatedSensorObservation::Multispectral {
                sensor_id,
                calibration_ref,
                manifest,
            } => {
                assert_eq!(sensor_id, "sim-multispectral");
                assert_eq!(calibration_ref, "sim-multispectral-v1");
                assert_eq!(manifest.expected_bands, ["Red", "Green", "Blue", "NIR"]);
                assert_eq!(manifest.bands.len(), 4);
                assert_eq!(
                    manifest.bands[0].file_path,
                    PathBuf::from("sim://flight_sim_cpp/Red.tif")
                );
                assert_eq!(manifest.bands[0].width, 16);
                assert_eq!(manifest.bands[0].height, 12);
                assert_eq!(manifest.bands[0].exposure_time_ms, 1);
                assert_eq!(manifest.bands[0].gain, 1.0);
            }
            other => panic!("expected multispectral observation, got {other:?}"),
        }
    }

    #[test]
    fn flight_sim_cpp_non_ok_sensor_json_maps_to_failure() {
        let lidar_json = r#"{
            "timestamp": "2027-01-15T10:30:00Z",
            "points": [],
            "scan_id": "11111111-1111-4111-8111-111111111111",
            "sensor_id": "sim-lidar-a3",
            "status": "no_hits"
        }"#;
        let multispectral_json = r#"{
            "status": "no_coverage",
            "reason": "mission footprint outside camera frustum",
            "bands": []
        }"#;

        let lidar_observation =
            flight_sim_cpp_lidar_observation_from_json(lidar_json, "sim-lidar-v1")
                .expect("lidar failure parses");
        let multispectral_observation = flight_sim_cpp_multispectral_observation_from_json(
            multispectral_json,
            "sim-multispectral",
            "sim-multispectral-v1",
        )
        .expect("multispectral failure parses");

        match lidar_observation {
            SimulatedSensorObservation::Failure {
                sensor_id,
                data_type,
                kind,
                message,
            } => {
                assert_eq!(sensor_id, "sim-lidar-a3");
                assert_eq!(data_type, DataType::LidarScan);
                assert_eq!(kind, CollectionFailureKind::SensorDropout);
                assert!(message.contains("no_hits"));
            }
            other => panic!("expected lidar failure, got {other:?}"),
        }
        match multispectral_observation {
            SimulatedSensorObservation::Failure {
                sensor_id,
                data_type,
                kind,
                message,
            } => {
                assert_eq!(sensor_id, "sim-multispectral");
                assert_eq!(data_type, DataType::MultispectralImage);
                assert_eq!(kind, CollectionFailureKind::SensorDropout);
                assert!(message.contains("mission footprint outside camera frustum"));
            }
            other => panic!("expected multispectral failure, got {other:?}"),
        }
    }

    #[test]
    fn simulated_sensor_frame_builds_provenance_complete_records() {
        let timestamp = Utc.timestamp_opt(1_800_000_100, 0).unwrap();
        let frame = frame_with(vec![
            SimulatedSensorObservation::Telemetry {
                sensor_id: "sim-telemetry".to_string(),
                calibration_ref: "sim-telemetry-v1".to_string(),
                velocity: (1.0, 0.0, 0.0),
                orientation: (0.0, 0.0, 0.1),
                battery_level: 0.88,
                signal_strength: 0.99,
            },
            SimulatedSensorObservation::Lidar {
                sensor_id: "sim-lidar-a3".to_string(),
                calibration_ref: "sim-lidar-v1".to_string(),
                scan: lidar_scan(timestamp),
            },
            SimulatedSensorObservation::Multispectral {
                sensor_id: "sim-multispectral".to_string(),
                calibration_ref: "sim-multispectral-v1".to_string(),
                manifest: multispectral_manifest(),
            },
        ]);
        let session_id = frame.session_id;
        let flight_id = frame.flight_id;
        let drone_id = frame.drone_id;
        let simulation_mission_id = frame.simulation_mission_id;

        let batch = simulated_capture_frame_to_batch(frame).expect("sim frame converts");

        assert!(batch.failures.is_empty());
        assert_eq!(batch.records.len(), 3);
        assert!(batch
            .records
            .iter()
            .all(|record| record.session_id == session_id
                && record.flight_id == flight_id
                && record.drone_id == drone_id
                && record.gps_coords == Some(gps())
                && record.validate_provenance().is_ok()
                && record.metadata.get("source") == Some(&"flight_sim_cpp".to_string())
                && record.metadata.get("runtime_mode") == Some(&"simulation".to_string())
                && record.metadata.get("simulation_mission_id")
                    == Some(&simulation_mission_id.to_string())));
        assert!(batch
            .records
            .iter()
            .any(|record| record.data_type == DataType::LidarScan));
        assert!(batch
            .records
            .iter()
            .any(|record| record.data_type == DataType::MultispectralImage
                && record.metadata.get("bands") == Some(&"Red,Green,Blue,NIR".to_string())));
    }

    #[test]
    fn simulated_sensor_failure_builds_collection_failure_request() {
        let frame = frame_with(vec![SimulatedSensorObservation::Failure {
            sensor_id: "sim-multispectral".to_string(),
            data_type: DataType::MultispectralImage,
            kind: CollectionFailureKind::SensorDropout,
            message: "simulated sensor dropout at frame 12".to_string(),
        }]);
        let observed_at = frame.observed_at;

        let batch = simulated_capture_frame_to_batch(frame).expect("failure converts");

        assert!(batch.records.is_empty());
        assert_eq!(batch.failures.len(), 1);
        assert_eq!(batch.failures[0].occurred_at, Some(observed_at));
        assert_eq!(batch.failures[0].sensor_id, "sim-multispectral");
        assert_eq!(batch.failures[0].data_type, DataType::MultispectralImage);
        assert_eq!(batch.failures[0].kind, CollectionFailureKind::SensorDropout);
        assert!(batch.failures[0].message.contains("dropout"));
    }
}
