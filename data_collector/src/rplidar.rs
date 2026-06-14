use crate::{
    CollectionFailureKind, CollectionFailureRequest, DataPayload, DataType, FlightDataProvenance,
    FlightDataProvenanceError, FlightDataRecord,
};
use chrono::{DateTime, Utc};
use shared::schemas::{LidarPoint, LidarScan};
use uuid::Uuid;

const RPLIDAR_MEASUREMENT_NODE_BYTES: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RplidarParseError {
    #[error("RPLIDAR frame is empty")]
    EmptyFrame,
    #[error("RPLIDAR frame length {length} is not a multiple of 5-byte measurement nodes")]
    InvalidFrameLength { length: usize },
    #[error("RPLIDAR node {node_index} has invalid start/inverse-start flags")]
    InvalidSyncFlags { node_index: usize },
    #[error("RPLIDAR node {node_index} is missing the angle check bit")]
    InvalidAngleCheckBit { node_index: usize },
    #[error("RPLIDAR node {node_index} reported zero distance")]
    ZeroDistance { node_index: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum LidarRecordError {
    #[error("cannot convert an empty LiDAR scan into a capture record")]
    EmptyScan,
    #[error(transparent)]
    Provenance(#[from] FlightDataProvenanceError),
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
}

impl RplidarParseError {
    pub fn into_collection_failure(
        self,
        sensor_id: impl Into<String>,
        occurred_at: DateTime<Utc>,
    ) -> CollectionFailureRequest {
        CollectionFailureRequest {
            occurred_at: Some(occurred_at),
            sensor_id: sensor_id.into(),
            data_type: DataType::LidarScan,
            kind: CollectionFailureKind::MalformedFrame,
            message: self.to_string(),
        }
    }
}

pub fn parse_rplidar_a3_measurements(
    frame: &[u8],
    timestamp: DateTime<Utc>,
) -> Result<LidarScan, RplidarParseError> {
    if frame.is_empty() {
        return Err(RplidarParseError::EmptyFrame);
    }

    if frame.len() % RPLIDAR_MEASUREMENT_NODE_BYTES != 0 {
        return Err(RplidarParseError::InvalidFrameLength {
            length: frame.len(),
        });
    }

    let mut points = Vec::with_capacity(frame.len() / RPLIDAR_MEASUREMENT_NODE_BYTES);
    for (node_index, node) in frame
        .chunks_exact(RPLIDAR_MEASUREMENT_NODE_BYTES)
        .enumerate()
    {
        let start_flag = node[0] & 0x01 != 0;
        let inverse_start_flag = node[0] & 0x02 != 0;
        if start_flag == inverse_start_flag {
            return Err(RplidarParseError::InvalidSyncFlags { node_index });
        }

        let angle_q6_check = u16::from_le_bytes([node[1], node[2]]);
        if angle_q6_check & 0x01 == 0 {
            return Err(RplidarParseError::InvalidAngleCheckBit { node_index });
        }

        let distance_q2 = u16::from_le_bytes([node[3], node[4]]);
        if distance_q2 == 0 {
            return Err(RplidarParseError::ZeroDistance { node_index });
        }

        points.push(LidarPoint {
            timestamp,
            angle: ((angle_q6_check >> 1) as f32) / 64.0,
            distance: (distance_q2 as f32) / 4.0 / 1000.0,
            quality: node[0] >> 2,
        });
    }

    Ok(LidarScan {
        timestamp,
        points,
        scan_id: Uuid::new_v4(),
    })
}

pub fn lidar_scan_to_record(
    flight_id: Uuid,
    drone_id: Uuid,
    scan: &LidarScan,
    provenance: FlightDataProvenance,
) -> Result<FlightDataRecord, LidarRecordError> {
    if scan.points.is_empty() {
        return Err(LidarRecordError::EmptyScan);
    }

    let bounds = point_cloud_bounds(&scan.points);
    let size_bytes = serde_json::to_vec(scan)?.len() as u64;
    let mut record = FlightDataRecord::new(
        flight_id,
        drone_id,
        DataType::LidarScan,
        DataPayload::PointCloud {
            point_count: scan.points.len() as u32,
            bounds,
            format: "rplidar-a3-q2".to_string(),
            has_color: false,
            has_intensity: true,
        },
        provenance,
        size_bytes,
    )?;

    record
        .metadata
        .insert("scan_id".to_string(), scan.scan_id.to_string());
    record.metadata.insert(
        "schema".to_string(),
        "shared::schemas::LidarScan".to_string(),
    );
    record
        .metadata
        .insert("source".to_string(), "rplidar-a3".to_string());
    record
        .metadata
        .insert("distance_units".to_string(), "meters".to_string());
    Ok(record)
}

fn point_cloud_bounds(points: &[LidarPoint]) -> ((f32, f32, f32), (f32, f32, f32)) {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut min_z = 0.0f32;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut max_z = 0.0f32;

    for point in points {
        let angle_radians = point.angle.to_radians();
        let x = point.distance * angle_radians.cos();
        let y = point.distance * angle_radians.sin();
        let z = 0.0;

        min_x = min_x.min(x);
        min_y = min_y.min(y);
        min_z = min_z.min(z);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        max_z = max_z.max(z);
    }

    ((min_x, min_y, min_z), (max_x, max_y, max_z))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataPayload, DataType, FlightDataProvenance};
    use chrono::{TimeZone, Utc};
    use shared::schemas::GpsCoords;
    use uuid::Uuid;

    fn node(start: bool, quality: u8, angle_degrees: f32, distance_m: f32) -> [u8; 5] {
        let sync_quality = (quality << 2) | u8::from(start) | (u8::from(!start) << 1);
        let angle_q6_check = (((angle_degrees * 64.0).round() as u16) << 1) | 1;
        let distance_q2 = (distance_m * 1000.0 * 4.0).round() as u16;
        let angle = angle_q6_check.to_le_bytes();
        let distance = distance_q2.to_le_bytes();
        [sync_quality, angle[0], angle[1], distance[0], distance[1]]
    }

    fn provenance(session_id: Uuid) -> FlightDataProvenance {
        FlightDataProvenance::complete(
            session_id,
            "rplidar-a3-front".to_string(),
            GpsCoords {
                latitude: 41.0,
                longitude: -96.0,
                altitude: 402.0,
            },
            Utc.timestamp_opt(1000, 0).unwrap(),
            "rplidar-a3-cal-2026".to_string(),
        )
    }

    #[test]
    fn rplidar_frame_parses_to_shared_lidar_scan_shape() {
        let timestamp = Utc.timestamp_opt(1000, 0).unwrap();
        let bytes = [
            node(true, 32, 0.0, 2.0),
            node(false, 28, 90.0, 3.5),
            node(false, 25, 180.0, 1.25),
        ]
        .concat();

        let scan = parse_rplidar_a3_measurements(&bytes, timestamp).expect("frame parses");

        assert_eq!(scan.points.len(), 3);
        assert_eq!(scan.timestamp, timestamp);
        assert_eq!(scan.points[0].angle, 0.0);
        assert_eq!(scan.points[0].distance, 2.0);
        assert_eq!(scan.points[0].quality, 32);
        assert_eq!(scan.points[1].angle, 90.0);
        assert_eq!(scan.points[1].distance, 3.5);
        assert_eq!(scan.points[2].angle, 180.0);
    }

    #[test]
    fn rplidar_scan_converts_to_provenance_complete_record() {
        let timestamp = Utc.timestamp_opt(1000, 0).unwrap();
        let session_id = Uuid::new_v4();
        let flight_id = Uuid::new_v4();
        let drone_id = Uuid::new_v4();
        let bytes = [
            node(true, 32, 0.0, 2.0),
            node(false, 28, 90.0, 3.5),
            node(false, 25, 180.0, 1.25),
        ]
        .concat();
        let scan = parse_rplidar_a3_measurements(&bytes, timestamp).expect("frame parses");

        let record = lidar_scan_to_record(flight_id, drone_id, &scan, provenance(session_id))
            .expect("record builds");

        assert_eq!(record.session_id, session_id);
        assert_eq!(record.flight_id, flight_id);
        assert_eq!(record.drone_id, drone_id);
        assert_eq!(record.data_type, DataType::LidarScan);
        assert_eq!(record.sensor_id, "rplidar-a3-front");
        assert_eq!(record.calibration_ref, "rplidar-a3-cal-2026");
        assert_eq!(
            record.metadata.get("scan_id"),
            Some(&scan.scan_id.to_string())
        );
        match record.payload {
            DataPayload::PointCloud {
                point_count,
                bounds,
                format,
                has_intensity,
                ..
            } => {
                assert_eq!(point_count, 3);
                assert_eq!(format, "rplidar-a3-q2");
                assert!(has_intensity);
                assert!(bounds.0 .0 <= -1.25);
                assert!(bounds.1 .1 >= 3.5);
            }
            _ => panic!("expected point cloud payload"),
        }
    }

    #[test]
    fn malformed_rplidar_frame_maps_to_collection_failure() {
        let timestamp = Utc.timestamp_opt(1000, 0).unwrap();
        let error = parse_rplidar_a3_measurements(&[0x01, 0x02, 0x03], timestamp)
            .expect_err("short frame is malformed");

        let failure = error.into_collection_failure("rplidar-a3-front", timestamp);

        assert_eq!(failure.data_type, DataType::LidarScan);
        assert_eq!(failure.kind, crate::CollectionFailureKind::MalformedFrame);
        assert_eq!(failure.sensor_id, "rplidar-a3-front");
        assert!(failure.message.contains("5-byte"));
    }
}
