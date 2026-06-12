use serde::Serialize;
use shared::schemas::WebSocketMessage;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRoute {
    Telemetry,
    MissionStatus,
    LidarUpdate,
    ImageCaptured,
    NdviProcessed,
    SystemStatus,
}

#[derive(Debug, Clone)]
pub struct DispatchedMessage {
    pub route: MessageRoute,
    pub message: WebSocketMessage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MissionStatusSnapshot {
    pub mission_id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemStatusSnapshot {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MessageDispatchState {
    pub latest_telemetry_mode: Option<String>,
    pub latest_telemetry_battery_percentage: Option<u8>,
    pub mission_statuses: Vec<MissionStatusSnapshot>,
    pub lidar_scan_point_counts: Vec<usize>,
    pub captured_image_ids: Vec<Uuid>,
    pub ndvi_means: Vec<f32>,
    pub system_statuses: Vec<SystemStatusSnapshot>,
    pub malformed_frames: u64,
}

pub type SharedMessageDispatchState = Arc<RwLock<MessageDispatchState>>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DispatchError {
    #[error("malformed websocket frame: {0}")]
    MalformedFrame(String),
}

impl MessageDispatchState {
    pub fn dispatch_frame(&mut self, frame: &str) -> Result<DispatchedMessage, DispatchError> {
        let message = serde_json::from_str::<WebSocketMessage>(frame).map_err(|err| {
            self.malformed_frames = self.malformed_frames.saturating_add(1);
            DispatchError::MalformedFrame(err.to_string())
        })?;
        let route = self.dispatch_message(&message);
        Ok(DispatchedMessage { route, message })
    }

    pub fn dispatch_message(&mut self, message: &WebSocketMessage) -> MessageRoute {
        match message {
            WebSocketMessage::Telemetry { data } => {
                self.latest_telemetry_mode = Some(data.mode.clone());
                self.latest_telemetry_battery_percentage = Some(data.battery_percentage);
                MessageRoute::Telemetry
            }
            WebSocketMessage::MissionStatus { mission_id, status } => {
                self.mission_statuses.push(MissionStatusSnapshot {
                    mission_id: *mission_id,
                    status: status.clone(),
                });
                MessageRoute::MissionStatus
            }
            WebSocketMessage::LidarUpdate { scan } => {
                self.lidar_scan_point_counts.push(scan.points.len());
                MessageRoute::LidarUpdate
            }
            WebSocketMessage::ImageCaptured { image } => {
                self.captured_image_ids.push(image.image_id);
                MessageRoute::ImageCaptured
            }
            WebSocketMessage::NdviProcessed { result } => {
                self.ndvi_means.push(result.mean_ndvi);
                MessageRoute::NdviProcessed
            }
            WebSocketMessage::SystemStatus { status, message } => {
                self.system_statuses.push(SystemStatusSnapshot {
                    status: status.clone(),
                    message: message.clone(),
                });
                MessageRoute::SystemStatus
            }
        }
    }
}

pub fn shared_message_dispatch_state() -> SharedMessageDispatchState {
    Arc::new(RwLock::new(MessageDispatchState::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::{
        GpsCoords, ImageMetadata, LidarPoint, LidarScan, MultispectralImage, NdviResult, Telemetry,
        WebSocketMessage,
    };
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn dispatches_all_websocket_variants_to_typed_routes() {
        let mission_id = Uuid::new_v4();
        let image_id = Uuid::new_v4();
        let mut state = MessageDispatchState::default();
        let cases = vec![
            (
                WebSocketMessage::Telemetry {
                    data: sample_telemetry("GUIDED", 72),
                },
                MessageRoute::Telemetry,
            ),
            (
                WebSocketMessage::MissionStatus {
                    mission_id,
                    status: "surveying".to_string(),
                },
                MessageRoute::MissionStatus,
            ),
            (
                WebSocketMessage::LidarUpdate {
                    scan: sample_lidar_scan(),
                },
                MessageRoute::LidarUpdate,
            ),
            (
                WebSocketMessage::ImageCaptured {
                    image: sample_image(image_id),
                },
                MessageRoute::ImageCaptured,
            ),
            (
                WebSocketMessage::NdviProcessed {
                    result: sample_ndvi(image_id),
                },
                MessageRoute::NdviProcessed,
            ),
            (
                WebSocketMessage::SystemStatus {
                    status: "warn".to_string(),
                    message: "wind increasing".to_string(),
                },
                MessageRoute::SystemStatus,
            ),
        ];

        for (message, route) in cases {
            let frame = serde_json::to_string(&message).unwrap();
            let dispatched = state.dispatch_frame(&frame).unwrap();
            assert_eq!(dispatched.route, route);
        }

        assert_eq!(state.latest_telemetry_mode.as_deref(), Some("GUIDED"));
        assert_eq!(state.latest_telemetry_battery_percentage, Some(72));
        assert_eq!(state.mission_statuses[0].mission_id, mission_id);
        assert_eq!(state.mission_statuses[0].status, "surveying");
        assert_eq!(state.lidar_scan_point_counts, vec![2]);
        assert_eq!(state.captured_image_ids, vec![image_id]);
        assert_eq!(state.ndvi_means, vec![0.42]);
        assert_eq!(state.system_statuses[0].status, "warn");
        assert_eq!(state.system_statuses[0].message, "wind increasing");
        assert_eq!(state.malformed_frames, 0);
    }

    #[test]
    fn malformed_frame_is_counted_and_preserves_prior_state() {
        let mut state = MessageDispatchState::default();
        let frame = serde_json::to_string(&WebSocketMessage::Telemetry {
            data: sample_telemetry("AUTO", 88),
        })
        .unwrap();
        state.dispatch_frame(&frame).unwrap();
        let before = state.clone();

        let error = state
            .dispatch_frame(r#"{"type":"Telemetry","data":{"mode":"broken"}}"#)
            .unwrap_err();

        assert!(matches!(error, DispatchError::MalformedFrame(_)));
        assert_eq!(state.malformed_frames, before.malformed_frames + 1);
        assert_eq!(state.latest_telemetry_mode, before.latest_telemetry_mode);
        assert_eq!(
            state.latest_telemetry_battery_percentage,
            before.latest_telemetry_battery_percentage
        );
        assert_eq!(state.mission_statuses, before.mission_statuses);
        assert_eq!(
            state.lidar_scan_point_counts,
            before.lidar_scan_point_counts
        );
        assert_eq!(state.captured_image_ids, before.captured_image_ids);
        assert_eq!(state.ndvi_means, before.ndvi_means);
        assert_eq!(state.system_statuses, before.system_statuses);
    }

    fn sample_telemetry(mode: &str, battery_percentage: u8) -> Telemetry {
        Telemetry {
            timestamp: timestamp(),
            position: GpsCoords {
                latitude: 42.0,
                longitude: -71.0,
                altitude: 120.0,
            },
            battery_voltage: 15.4,
            battery_percentage,
            armed: true,
            mode: mode.to_string(),
            ground_speed: 6.5,
            air_speed: 7.0,
            heading: 180.0,
            altitude_relative: 45.0,
        }
    }

    fn sample_lidar_scan() -> LidarScan {
        LidarScan {
            timestamp: timestamp(),
            points: vec![
                LidarPoint {
                    timestamp: timestamp(),
                    angle: 0.0,
                    distance: 2.0,
                    quality: 90,
                },
                LidarPoint {
                    timestamp: timestamp(),
                    angle: 1.0,
                    distance: 2.2,
                    quality: 92,
                },
            ],
            scan_id: Uuid::new_v4(),
        }
    }

    fn sample_image(image_id: Uuid) -> MultispectralImage {
        let mut file_paths = HashMap::new();
        file_paths.insert("red".to_string(), "red.tif".to_string());
        MultispectralImage {
            metadata: ImageMetadata {
                timestamp: timestamp(),
                gps_position: Some(GpsCoords {
                    latitude: 42.0,
                    longitude: -71.0,
                    altitude: 120.0,
                }),
                bands: vec!["red".to_string()],
                exposure_time: 1.0,
                gain: 1.0,
                width: 16,
                height: 16,
                spatial_ref: None,
            },
            file_paths,
            image_id,
        }
    }

    fn sample_ndvi(image_id: Uuid) -> NdviResult {
        NdviResult {
            timestamp: timestamp(),
            source_images: vec![image_id],
            output_path: "ndvi.tif".to_string(),
            min_ndvi: 0.1,
            max_ndvi: 0.8,
            mean_ndvi: 0.42,
            vegetation_percentage: 61.0,
        }
    }

    fn timestamp() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc)
    }
}
