use crate::{FlightPathSample, MapRenderState, MissionOverlayInput, DEFAULT_FLIGHT_PATH_LIMIT};
use serde::Serialize;
use shared::schemas::{Telemetry, WebSocketMessage};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

pub const DEFAULT_CAPTURE_EVENT_LIMIT: usize = 100;
pub const DEFAULT_TELEMETRY_STALE_AFTER: Duration = Duration::from_secs(5);

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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TelemetryTileValues {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
    pub battery_voltage: f32,
    pub battery_percentage: u8,
    pub armed: bool,
    pub mode: String,
    pub ground_speed: f32,
    pub air_speed: f32,
    pub heading: f32,
    pub altitude_relative: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TelemetryTileSnapshot {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
    pub battery_voltage: f32,
    pub battery_percentage: u8,
    pub armed: bool,
    pub mode: String,
    pub ground_speed: f32,
    pub air_speed: f32,
    pub heading: f32,
    pub altitude_relative: f32,
    pub last_update_at: chrono::DateTime<chrono::Utc>,
    pub last_update_age_seconds: u64,
    pub stale: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryFreshnessState {
    NoData,
    Fresh,
    Stale,
}

impl std::fmt::Display for TelemetryFreshnessState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            TelemetryFreshnessState::NoData => "No data",
            TelemetryFreshnessState::Fresh => "Fresh",
            TelemetryFreshnessState::Stale => "Stale",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TelemetryFreshnessSnapshot {
    pub state: TelemetryFreshnessState,
    pub last_update_age_seconds: Option<u64>,
    pub stale_after_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureEventKind {
    Lidar,
    ImageCaptured,
    NdviProcessed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaptureEvent {
    pub event_type: CaptureEventKind,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub summary: String,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MessageDispatchState {
    pub latest_telemetry_mode: Option<String>,
    pub latest_telemetry_battery_percentage: Option<u8>,
    pub latest_telemetry: Option<TelemetryTileValues>,
    pub latest_telemetry_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub mission_statuses: Vec<MissionStatusSnapshot>,
    pub lidar_scan_point_counts: Vec<usize>,
    pub captured_image_ids: Vec<Uuid>,
    pub ndvi_means: Vec<f32>,
    pub capture_events: Vec<CaptureEvent>,
    pub flight_path: Vec<FlightPathSample>,
    pub mission_overlay: Option<MissionOverlayInput>,
    pub system_statuses: Vec<SystemStatusSnapshot>,
    pub malformed_frames: u64,
    #[serde(skip)]
    capture_event_limit: usize,
    #[serde(skip)]
    flight_path_limit: usize,
}

pub type SharedMessageDispatchState = Arc<RwLock<MessageDispatchState>>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DispatchError {
    #[error("malformed websocket frame: {0}")]
    MalformedFrame(String),
}

impl MessageDispatchState {
    pub fn with_capture_event_limit(capture_event_limit: usize) -> Self {
        Self {
            latest_telemetry_mode: None,
            latest_telemetry_battery_percentage: None,
            latest_telemetry: None,
            latest_telemetry_updated_at: None,
            mission_statuses: Vec::new(),
            lidar_scan_point_counts: Vec::new(),
            captured_image_ids: Vec::new(),
            ndvi_means: Vec::new(),
            capture_events: Vec::new(),
            flight_path: Vec::new(),
            mission_overlay: None,
            system_statuses: Vec::new(),
            malformed_frames: 0,
            capture_event_limit: capture_event_limit.max(1),
            flight_path_limit: DEFAULT_FLIGHT_PATH_LIMIT,
        }
    }

    pub fn dispatch_frame(&mut self, frame: &str) -> Result<DispatchedMessage, DispatchError> {
        let message = serde_json::from_str::<WebSocketMessage>(frame).map_err(|err| {
            self.malformed_frames = self.malformed_frames.saturating_add(1);
            DispatchError::MalformedFrame(err.to_string())
        })?;
        let route = self.dispatch_message(&message);
        Ok(DispatchedMessage { route, message })
    }

    pub fn dispatch_message(&mut self, message: &WebSocketMessage) -> MessageRoute {
        self.dispatch_message_at(message, chrono::Utc::now())
    }

    pub fn dispatch_message_at(
        &mut self,
        message: &WebSocketMessage,
        received_at: chrono::DateTime<chrono::Utc>,
    ) -> MessageRoute {
        match message {
            WebSocketMessage::Telemetry { data } => {
                self.latest_telemetry_mode = Some(data.mode.clone());
                self.latest_telemetry_battery_percentage = Some(data.battery_percentage);
                self.latest_telemetry = Some(TelemetryTileValues::from(data));
                self.latest_telemetry_updated_at = Some(received_at);
                self.append_flight_path_sample(FlightPathSample::from_telemetry(data));
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
                self.append_capture_event(CaptureEvent {
                    event_type: CaptureEventKind::Lidar,
                    timestamp: scan.timestamp,
                    summary: format!("LiDAR scan: {} points", scan.points.len()),
                });
                MessageRoute::LidarUpdate
            }
            WebSocketMessage::ImageCaptured { image } => {
                self.captured_image_ids.push(image.image_id);
                self.append_capture_event(CaptureEvent {
                    event_type: CaptureEventKind::ImageCaptured,
                    timestamp: image.metadata.timestamp,
                    summary: format!("Image captured: {}", image.image_id),
                });
                MessageRoute::ImageCaptured
            }
            WebSocketMessage::NdviProcessed { result } => {
                self.ndvi_means.push(result.mean_ndvi);
                self.append_capture_event(CaptureEvent {
                    event_type: CaptureEventKind::NdviProcessed,
                    timestamp: result.timestamp,
                    summary: format!(
                        "NDVI processed: mean {:.3}, vegetation {:.1}%",
                        result.mean_ndvi, result.vegetation_percentage
                    ),
                });
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

    pub fn telemetry_tile_snapshot_at(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        stale_after: Duration,
    ) -> Option<TelemetryTileSnapshot> {
        let telemetry = self.latest_telemetry.as_ref()?;
        let last_update_at = self.latest_telemetry_updated_at?;
        let age = age_seconds(last_update_at, now);
        Some(TelemetryTileSnapshot {
            latitude: telemetry.latitude,
            longitude: telemetry.longitude,
            altitude_m: telemetry.altitude_m,
            battery_voltage: telemetry.battery_voltage,
            battery_percentage: telemetry.battery_percentage,
            armed: telemetry.armed,
            mode: telemetry.mode.clone(),
            ground_speed: telemetry.ground_speed,
            air_speed: telemetry.air_speed,
            heading: telemetry.heading,
            altitude_relative: telemetry.altitude_relative,
            last_update_at,
            last_update_age_seconds: age,
            stale: age > stale_after.as_secs(),
        })
    }

    pub fn telemetry_freshness_at(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        stale_after: Duration,
    ) -> TelemetryFreshnessSnapshot {
        match self.telemetry_tile_snapshot_at(now, stale_after) {
            Some(snapshot) => TelemetryFreshnessSnapshot {
                state: if snapshot.stale {
                    TelemetryFreshnessState::Stale
                } else {
                    TelemetryFreshnessState::Fresh
                },
                last_update_age_seconds: Some(snapshot.last_update_age_seconds),
                stale_after_seconds: stale_after.as_secs(),
            },
            None => TelemetryFreshnessSnapshot {
                state: TelemetryFreshnessState::NoData,
                last_update_age_seconds: None,
                stale_after_seconds: stale_after.as_secs(),
            },
        }
    }

    pub fn telemetry_tile_snapshot(&self) -> Option<TelemetryTileSnapshot> {
        self.telemetry_tile_snapshot_at(chrono::Utc::now(), DEFAULT_TELEMETRY_STALE_AFTER)
    }

    pub fn telemetry_freshness(&self) -> TelemetryFreshnessSnapshot {
        self.telemetry_freshness_at(chrono::Utc::now(), DEFAULT_TELEMETRY_STALE_AFTER)
    }

    pub fn capture_events(&self, filter: Option<CaptureEventKind>) -> Vec<CaptureEvent> {
        self.capture_events
            .iter()
            .filter(|event| filter.map_or(true, |kind| event.event_type == kind))
            .cloned()
            .collect()
    }

    pub fn map_render_state(&self) -> MapRenderState {
        MapRenderState::from_flight_path_and_mission(
            &self.flight_path,
            self.mission_overlay.as_ref(),
        )
    }

    pub fn set_mission_overlay(&mut self, mission_overlay: MissionOverlayInput) {
        self.mission_overlay = Some(mission_overlay);
    }

    fn append_capture_event(&mut self, event: CaptureEvent) {
        self.capture_events.push(event);
        self.capture_events.sort_by(|left, right| {
            left.timestamp
                .cmp(&right.timestamp)
                .then(left.event_type.cmp(&right.event_type))
                .then(left.summary.cmp(&right.summary))
        });
        while self.capture_events.len() > self.capture_event_limit {
            self.capture_events.remove(0);
        }
    }

    fn append_flight_path_sample(&mut self, sample: FlightPathSample) {
        self.flight_path.push(sample);
        while self.flight_path.len() > self.flight_path_limit {
            self.flight_path.remove(0);
        }
    }
}

impl Default for MessageDispatchState {
    fn default() -> Self {
        Self::with_capture_event_limit(DEFAULT_CAPTURE_EVENT_LIMIT)
    }
}

impl From<&Telemetry> for TelemetryTileValues {
    fn from(telemetry: &Telemetry) -> Self {
        Self {
            latitude: telemetry.position.latitude,
            longitude: telemetry.position.longitude,
            altitude_m: telemetry.position.altitude,
            battery_voltage: telemetry.battery_voltage,
            battery_percentage: telemetry.battery_percentage,
            armed: telemetry.armed,
            mode: telemetry.mode.clone(),
            ground_speed: telemetry.ground_speed,
            air_speed: telemetry.air_speed,
            heading: telemetry.heading,
            altitude_relative: telemetry.altitude_relative,
        }
    }
}

fn age_seconds(
    last_update_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> u64 {
    now.signed_duration_since(last_update_at)
        .num_seconds()
        .max(0) as u64
}

pub fn shared_message_dispatch_state() -> SharedMessageDispatchState {
    Arc::new(RwLock::new(MessageDispatchState::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_overlay_matches_basemap, BasemapLayer, FlightPathSample, MapOverlayLayer,
        MapRenderError, MapRenderState, MissionOverlayInput, MissionPolygonInput,
        MissionWaypointInput, WEB_MERCATOR_CRS, WGS84_CRS,
    };
    use shared::schemas::{
        GpsCoords, ImageMetadata, LidarPoint, LidarScan, MultispectralImage, NdviResult, Telemetry,
        WebSocketMessage,
    };
    use std::{collections::HashMap, time::Duration};
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

    #[test]
    fn telemetry_snapshot_tracks_all_bound_tiles_and_freshness() {
        let mut state = MessageDispatchState::default();
        state.dispatch_message_at(
            &WebSocketMessage::Telemetry {
                data: sample_telemetry("GUIDED", 72),
            },
            timestamp_at("2026-01-01T00:00:10Z"),
        );

        let snapshot = state
            .telemetry_tile_snapshot_at(
                timestamp_at("2026-01-01T00:00:12Z"),
                Duration::from_secs(5),
            )
            .expect("telemetry tile snapshot should exist");

        assert_eq!(snapshot.latitude, 42.0);
        assert_eq!(snapshot.longitude, -71.0);
        assert_eq!(snapshot.altitude_m, 120.0);
        assert_eq!(snapshot.battery_percentage, 72);
        assert_eq!(snapshot.battery_voltage, 15.4);
        assert_eq!(snapshot.mode, "GUIDED");
        assert!(snapshot.armed);
        assert_eq!(snapshot.ground_speed, 6.5);
        assert_eq!(snapshot.air_speed, 7.0);
        assert_eq!(snapshot.heading, 180.0);
        assert_eq!(snapshot.altitude_relative, 45.0);
        assert_eq!(snapshot.last_update_age_seconds, 2);
        assert!(!snapshot.stale);
    }

    #[test]
    fn telemetry_health_marks_stale_when_gap_exceeds_threshold() {
        let mut state = MessageDispatchState::default();
        assert_eq!(
            state
                .telemetry_freshness_at(
                    timestamp_at("2026-01-01T00:00:12Z"),
                    Duration::from_secs(5)
                )
                .state,
            TelemetryFreshnessState::NoData
        );

        state.dispatch_message_at(
            &WebSocketMessage::Telemetry {
                data: sample_telemetry("AUTO", 88),
            },
            timestamp_at("2026-01-01T00:00:00Z"),
        );

        let freshness = state
            .telemetry_freshness_at(timestamp_at("2026-01-01T00:00:06Z"), Duration::from_secs(5));

        assert_eq!(freshness.state, TelemetryFreshnessState::Stale);
        assert_eq!(freshness.last_update_age_seconds, Some(6));
    }

    #[test]
    fn telemetry_updates_accumulate_map_path_and_project_latest_position() {
        let mut state = MessageDispatchState::default();
        let mut first = sample_telemetry("AUTO", 88);
        first.position.latitude = 42.0000;
        first.position.longitude = -71.0000;
        let mut second = sample_telemetry("AUTO", 87);
        second.timestamp = timestamp_at("2026-01-01T00:00:05Z");
        second.position.latitude = 42.0005;
        second.position.longitude = -71.0007;

        state.dispatch_message(&WebSocketMessage::Telemetry { data: first });
        state.dispatch_message(&WebSocketMessage::Telemetry { data: second });

        let map_state = state.map_render_state();
        assert_eq!(map_state.basemap.crs, WEB_MERCATOR_CRS);
        assert_eq!(map_state.flight_path.len(), 2);
        let marker = map_state
            .current_position
            .expect("latest telemetry should produce a drone marker");
        assert_eq!(marker.latitude, 42.0005);
        assert_eq!(marker.longitude, -71.0007);
        assert!(marker.x_px >= 0.0 && marker.x_px <= map_state.basemap.width_px as f64);
        assert!(marker.y_px >= 0.0 && marker.y_px <= map_state.basemap.height_px as f64);
    }

    #[test]
    fn telemetry_coordinate_projects_to_map_canvas() {
        let sample = FlightPathSample::from_telemetry(&sample_telemetry("GUIDED", 72));
        let map_state = MapRenderState::from_flight_path(&[sample]);
        let point = map_state
            .flight_path
            .first()
            .expect("single telemetry sample should produce one path point");

        assert_eq!(point.source_crs, WGS84_CRS);
        assert_eq!(point.map_crs, WEB_MERCATOR_CRS);
        assert!(point.x_m.is_finite());
        assert!(point.y_m.is_finite());
        assert!(point.x_px >= 0.0 && point.x_px <= map_state.basemap.width_px as f64);
        assert!(point.y_px >= 0.0 && point.y_px <= map_state.basemap.height_px as f64);
    }

    #[test]
    fn wrong_crs_overlay_is_refused_before_rendering() {
        let basemap = BasemapLayer::default();
        let overlay = MapOverlayLayer::new("ndvi-overlay", WGS84_CRS, basemap.extent.clone());

        let error = assert_overlay_matches_basemap(&basemap, &overlay).unwrap_err();

        assert!(matches!(
            error,
            MapRenderError::CrsMismatch {
                overlay_id,
                basemap_crs,
                overlay_crs
            } if overlay_id == "ndvi-overlay"
                && basemap_crs == WEB_MERCATOR_CRS
                && overlay_crs == WGS84_CRS
        ));
    }

    #[test]
    fn mission_overlay_projects_waypoints_geofence_and_no_fly_zones() {
        let mut state = MessageDispatchState::default();
        state.set_mission_overlay(sample_mission_overlay(
            Some(sample_geofence()),
            vec![sample_no_fly_zone()],
        ));
        let mut telemetry = sample_telemetry("AUTO", 88);
        telemetry.position.latitude = 42.0002;
        telemetry.position.longitude = -71.0002;
        state.dispatch_message(&WebSocketMessage::Telemetry { data: telemetry });

        let map_state = state.map_render_state();
        let overlay = map_state
            .mission_overlay
            .expect("mission geometry should render as an overlay");

        assert_eq!(overlay.waypoints.len(), 2);
        assert!(overlay
            .waypoints
            .iter()
            .all(|waypoint| waypoint.map_crs == WEB_MERCATOR_CRS));
        assert_eq!(
            overlay
                .geofence
                .as_ref()
                .expect("geofence should be rendered")
                .map_crs,
            WEB_MERCATOR_CRS
        );
        assert_eq!(overlay.no_fly_zones.len(), 1);
        assert!(overlay
            .no_fly_zones
            .iter()
            .all(|zone| zone.map_crs == WEB_MERCATOR_CRS));
        assert_eq!(
            map_state
                .geofence_breach
                .as_ref()
                .map(|breach| breach.outside),
            Some(false)
        );
    }

    #[test]
    fn mission_overlay_flags_drone_outside_geofence() {
        let mut state = MessageDispatchState::default();
        state.set_mission_overlay(sample_mission_overlay(Some(sample_geofence()), vec![]));
        let mut telemetry = sample_telemetry("AUTO", 88);
        telemetry.position.latitude = 42.0040;
        telemetry.position.longitude = -71.0040;
        state.dispatch_message(&WebSocketMessage::Telemetry { data: telemetry });

        let map_state = state.map_render_state();

        assert_eq!(
            map_state
                .geofence_breach
                .as_ref()
                .map(|breach| breach.outside),
            Some(true)
        );
    }

    #[test]
    fn mission_overlay_omits_missing_geofence_without_default_geometry() {
        let mut state = MessageDispatchState::default();
        state.set_mission_overlay(sample_mission_overlay(None, vec![]));
        state.dispatch_message(&WebSocketMessage::Telemetry {
            data: sample_telemetry("AUTO", 88),
        });

        let map_state = state.map_render_state();
        let overlay = map_state
            .mission_overlay
            .expect("waypoints should render even without a geofence");

        assert!(overlay.geofence.is_none());
        assert!(map_state.geofence_breach.is_none());
    }

    #[test]
    fn capture_timeline_orders_filters_and_evicts_oldest_events() {
        let image_id = Uuid::new_v4();
        let mut state = MessageDispatchState::with_capture_event_limit(2);
        let mut lidar = sample_lidar_scan();
        lidar.timestamp = timestamp_at("2026-01-01T00:00:20Z");
        let mut image = sample_image(image_id);
        image.metadata.timestamp = timestamp_at("2026-01-01T00:00:30Z");
        let mut ndvi = sample_ndvi(image_id);
        ndvi.timestamp = timestamp_at("2026-01-01T00:00:10Z");

        state.dispatch_message(&WebSocketMessage::ImageCaptured { image });
        state.dispatch_message(&WebSocketMessage::NdviProcessed { result: ndvi });
        state.dispatch_message(&WebSocketMessage::LidarUpdate { scan: lidar });

        let timeline = state.capture_events(None);
        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].event_type, CaptureEventKind::Lidar);
        assert_eq!(timeline[0].timestamp, timestamp_at("2026-01-01T00:00:20Z"));
        assert_eq!(timeline[1].event_type, CaptureEventKind::ImageCaptured);

        let lidar_events = state.capture_events(Some(CaptureEventKind::Lidar));
        assert_eq!(lidar_events.len(), 1);
        assert!(lidar_events[0].summary.contains("2 points"));
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

    fn sample_mission_overlay(
        geofence: Option<MissionPolygonInput>,
        no_fly_zones: Vec<MissionPolygonInput>,
    ) -> MissionOverlayInput {
        MissionOverlayInput {
            mission_id: Uuid::new_v4(),
            waypoints: vec![
                MissionWaypointInput {
                    sequence: 1,
                    position: GpsCoords {
                        latitude: 42.0001,
                        longitude: -71.0001,
                        altitude: 120.0,
                    },
                },
                MissionWaypointInput {
                    sequence: 2,
                    position: GpsCoords {
                        latitude: 42.0004,
                        longitude: -71.0004,
                        altitude: 122.0,
                    },
                },
            ],
            geofence,
            no_fly_zones,
        }
    }

    fn sample_geofence() -> MissionPolygonInput {
        MissionPolygonInput::wgs84(
            "field-geofence",
            vec![
                gps(41.9995, -71.0008),
                gps(42.0008, -71.0008),
                gps(42.0008, -70.9995),
                gps(41.9995, -70.9995),
                gps(41.9995, -71.0008),
            ],
        )
    }

    fn sample_no_fly_zone() -> MissionPolygonInput {
        MissionPolygonInput::wgs84(
            "pump-house",
            vec![
                gps(42.00025, -71.0003),
                gps(42.00035, -71.0003),
                gps(42.00035, -71.0002),
                gps(42.00025, -71.0002),
                gps(42.00025, -71.0003),
            ],
        )
    }

    fn gps(latitude: f64, longitude: f64) -> GpsCoords {
        GpsCoords {
            latitude,
            longitude,
            altitude: 0.0,
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
        timestamp_at("2026-01-01T00:00:00Z")
    }

    fn timestamp_at(value: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }
}
