use chrono::{DateTime, Utc};
use serde::Serialize;
use shared::schemas::{GpsCoords, Mission, Telemetry};
use uuid::Uuid;

pub const WGS84_CRS: &str = "EPSG:4326";
pub const WEB_MERCATOR_CRS: &str = "EPSG:3857";
pub const DEFAULT_FLIGHT_PATH_LIMIT: usize = 2_000;

const EARTH_RADIUS_M: f64 = 6_378_137.0;
const MAX_WEB_MERCATOR_LAT: f64 = 85.051_128_78;
const DEFAULT_MAP_WIDTH_PX: u32 = 900;
const DEFAULT_MAP_HEIGHT_PX: u32 = 520;
const MIN_BASEMAP_SPAN_M: f64 = 250.0;
const BASEMAP_PADDING_FRACTION: f64 = 0.20;
const EXTENT_TOLERANCE_M: f64 = 0.01;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FlightPathSample {
    pub timestamp: DateTime<Utc>,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
}

impl FlightPathSample {
    pub fn from_telemetry(telemetry: &Telemetry) -> Self {
        Self {
            timestamp: telemetry.timestamp,
            latitude: telemetry.position.latitude,
            longitude: telemetry.position.longitude,
            altitude_m: telemetry.position.altitude,
        }
    }

    fn gps_position(&self) -> GpsCoords {
        GpsCoords {
            latitude: self.latitude,
            longitude: self.longitude,
            altitude: self.altitude_m,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectedPoint {
    pub x_m: f64,
    pub y_m: f64,
    pub crs: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectedExtent {
    pub crs: String,
    pub min_x_m: f64,
    pub min_y_m: f64,
    pub max_x_m: f64,
    pub max_y_m: f64,
}

impl ProjectedExtent {
    fn default_web_mercator() -> Self {
        Self::centered(
            WEB_MERCATOR_CRS,
            0.0,
            0.0,
            MIN_BASEMAP_SPAN_M,
            MIN_BASEMAP_SPAN_M,
        )
    }

    fn centered(crs: &str, center_x_m: f64, center_y_m: f64, width_m: f64, height_m: f64) -> Self {
        Self {
            crs: crs.to_string(),
            min_x_m: center_x_m - width_m / 2.0,
            min_y_m: center_y_m - height_m / 2.0,
            max_x_m: center_x_m + width_m / 2.0,
            max_y_m: center_y_m + height_m / 2.0,
        }
    }

    fn from_points(points: &[ProjectedPoint]) -> Self {
        if points.is_empty() {
            return Self::default_web_mercator();
        }

        let mut extent = Self {
            crs: WEB_MERCATOR_CRS.to_string(),
            min_x_m: f64::INFINITY,
            min_y_m: f64::INFINITY,
            max_x_m: f64::NEG_INFINITY,
            max_y_m: f64::NEG_INFINITY,
        };

        for point in points {
            extent.min_x_m = extent.min_x_m.min(point.x_m);
            extent.min_y_m = extent.min_y_m.min(point.y_m);
            extent.max_x_m = extent.max_x_m.max(point.x_m);
            extent.max_y_m = extent.max_y_m.max(point.y_m);
        }

        extent.with_minimum_span_and_padding()
    }

    fn with_minimum_span_and_padding(mut self) -> Self {
        self.ensure_minimum_span();
        let padding_x = self.width_m() * BASEMAP_PADDING_FRACTION;
        let padding_y = self.height_m() * BASEMAP_PADDING_FRACTION;
        self.min_x_m -= padding_x;
        self.max_x_m += padding_x;
        self.min_y_m -= padding_y;
        self.max_y_m += padding_y;
        self
    }

    fn ensure_minimum_span(&mut self) {
        if self.width_m() < MIN_BASEMAP_SPAN_M {
            let center = (self.min_x_m + self.max_x_m) / 2.0;
            self.min_x_m = center - MIN_BASEMAP_SPAN_M / 2.0;
            self.max_x_m = center + MIN_BASEMAP_SPAN_M / 2.0;
        }

        if self.height_m() < MIN_BASEMAP_SPAN_M {
            let center = (self.min_y_m + self.max_y_m) / 2.0;
            self.min_y_m = center - MIN_BASEMAP_SPAN_M / 2.0;
            self.max_y_m = center + MIN_BASEMAP_SPAN_M / 2.0;
        }
    }

    fn width_m(&self) -> f64 {
        (self.max_x_m - self.min_x_m).max(1.0)
    }

    fn height_m(&self) -> f64 {
        (self.max_y_m - self.min_y_m).max(1.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BasemapLayer {
    pub layer_id: String,
    pub name: String,
    pub crs: String,
    pub extent: ProjectedExtent,
    pub width_px: u32,
    pub height_px: u32,
}

impl BasemapLayer {
    pub fn from_path(samples: &[FlightPathSample]) -> Self {
        Self::from_path_and_mission(samples, None)
    }

    pub fn from_path_and_mission(
        samples: &[FlightPathSample],
        mission_overlay: Option<&MissionOverlayInput>,
    ) -> Self {
        Self::from_path_mission_and_captures(samples, mission_overlay, &[])
    }

    pub fn from_path_mission_and_captures(
        samples: &[FlightPathSample],
        mission_overlay: Option<&MissionOverlayInput>,
        capture_events: &[CaptureEventInput],
    ) -> Self {
        let mut projected_points: Vec<ProjectedPoint> = samples
            .iter()
            .map(|sample| project_wgs84_to_web_mercator(&sample.gps_position()))
            .collect();

        if let Some(mission_overlay) = mission_overlay {
            projected_points.extend(mission_overlay.projected_source_points());
        }
        projected_points.extend(
            capture_events
                .iter()
                .filter_map(|event| event.position.as_ref())
                .map(project_wgs84_to_web_mercator),
        );

        Self {
            layer_id: "ground-station-basemap".to_string(),
            name: "Operational Web Mercator Grid".to_string(),
            crs: WEB_MERCATOR_CRS.to_string(),
            extent: ProjectedExtent::from_points(&projected_points),
            width_px: DEFAULT_MAP_WIDTH_PX,
            height_px: DEFAULT_MAP_HEIGHT_PX,
        }
    }
}

impl Default for BasemapLayer {
    fn default() -> Self {
        Self {
            layer_id: "ground-station-basemap".to_string(),
            name: "Operational Web Mercator Grid".to_string(),
            crs: WEB_MERCATOR_CRS.to_string(),
            extent: ProjectedExtent::default_web_mercator(),
            width_px: DEFAULT_MAP_WIDTH_PX,
            height_px: DEFAULT_MAP_HEIGHT_PX,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MissionOverlayInput {
    pub mission_id: Uuid,
    pub waypoints: Vec<MissionWaypointInput>,
    pub geofence: Option<MissionPolygonInput>,
    pub no_fly_zones: Vec<MissionPolygonInput>,
}

impl MissionOverlayInput {
    pub fn from_mission_without_safety_geometry(mission: &Mission) -> Self {
        Self {
            mission_id: mission.id,
            waypoints: mission
                .waypoints
                .iter()
                .map(|waypoint| MissionWaypointInput {
                    sequence: waypoint.sequence,
                    position: waypoint.position.clone(),
                })
                .collect(),
            geofence: None,
            no_fly_zones: Vec::new(),
        }
    }

    fn projected_source_points(&self) -> Vec<ProjectedPoint> {
        let mut points: Vec<ProjectedPoint> = self
            .waypoints
            .iter()
            .map(|waypoint| project_wgs84_to_web_mercator(&waypoint.position))
            .collect();

        if let Some(geofence) = &self.geofence {
            points.extend(geofence.projected_source_points());
        }
        for zone in &self.no_fly_zones {
            points.extend(zone.projected_source_points());
        }

        points
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MissionWaypointInput {
    pub sequence: u16,
    pub position: GpsCoords,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MissionPolygonInput {
    pub polygon_id: String,
    pub source_crs: String,
    pub vertices: Vec<GpsCoords>,
}

impl MissionPolygonInput {
    pub fn wgs84(polygon_id: impl Into<String>, vertices: Vec<GpsCoords>) -> Self {
        Self {
            polygon_id: polygon_id.into(),
            source_crs: WGS84_CRS.to_string(),
            vertices,
        }
    }

    fn projected_source_points(&self) -> Vec<ProjectedPoint> {
        if self.source_crs != WGS84_CRS {
            return Vec::new();
        }

        self.vertices
            .iter()
            .map(project_wgs84_to_web_mercator)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapOverlayLayer {
    pub overlay_id: String,
    pub crs: String,
    pub extent: ProjectedExtent,
}

impl MapOverlayLayer {
    pub fn new(
        overlay_id: impl Into<String>,
        crs: impl Into<String>,
        extent: ProjectedExtent,
    ) -> Self {
        Self {
            overlay_id: overlay_id.into(),
            crs: crs.into(),
            extent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapPathPoint {
    pub timestamp: DateTime<Utc>,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
    pub source_crs: String,
    pub map_crs: String,
    pub x_m: f64,
    pub y_m: f64,
    pub x_px: f64,
    pub y_px: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CaptureEventInput {
    pub capture_event_id: String,
    pub timeline_entry_id: String,
    pub captured_at: DateTime<Utc>,
    pub position: Option<GpsCoords>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CaptureMapMarker {
    pub capture_event_id: String,
    pub timeline_entry_id: String,
    pub captured_at: DateTime<Utc>,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
    pub source_crs: String,
    pub map_crs: String,
    pub x_m: f64,
    pub y_m: f64,
    pub x_px: f64,
    pub y_px: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MissionMapOverlay {
    pub mission_id: Uuid,
    pub waypoints: Vec<MissionWaypointOverlay>,
    pub geofence: Option<MissionPolygonOverlay>,
    pub no_fly_zones: Vec<MissionPolygonOverlay>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MissionWaypointOverlay {
    pub sequence: u16,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
    pub source_crs: String,
    pub map_crs: String,
    pub x_m: f64,
    pub y_m: f64,
    pub x_px: f64,
    pub y_px: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MissionPolygonOverlay {
    pub polygon_id: String,
    pub source_crs: String,
    pub map_crs: String,
    pub vertices: Vec<MissionPolygonVertex>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MissionPolygonVertex {
    pub latitude: f64,
    pub longitude: f64,
    pub x_m: f64,
    pub y_m: f64,
    pub x_px: f64,
    pub y_px: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GeofenceBreach {
    pub mission_id: Uuid,
    pub latitude: f64,
    pub longitude: f64,
    pub outside: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OverlayAssertion {
    pub overlay_id: String,
    pub crs: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapRenderState {
    pub basemap: BasemapLayer,
    pub current_position: Option<MapPathPoint>,
    pub flight_path: Vec<MapPathPoint>,
    pub capture_markers: Vec<CaptureMapMarker>,
    pub unmapped_capture_event_ids: Vec<String>,
    pub mission_overlay: Option<MissionMapOverlay>,
    pub geofence_breach: Option<GeofenceBreach>,
    pub overlay_assertions: Vec<OverlayAssertion>,
}

impl MapRenderState {
    pub fn from_flight_path(samples: &[FlightPathSample]) -> Self {
        Self::from_flight_path_and_mission(samples, None)
    }

    pub fn from_flight_path_and_mission(
        samples: &[FlightPathSample],
        mission_overlay: Option<&MissionOverlayInput>,
    ) -> Self {
        Self::from_flight_path_mission_and_captures(samples, mission_overlay, &[])
    }

    pub fn from_flight_path_mission_and_captures(
        samples: &[FlightPathSample],
        mission_overlay: Option<&MissionOverlayInput>,
        capture_events: &[CaptureEventInput],
    ) -> Self {
        let basemap =
            BasemapLayer::from_path_mission_and_captures(samples, mission_overlay, capture_events);
        let telemetry_overlay = MapOverlayLayer::new(
            "telemetry-flight-path",
            WEB_MERCATOR_CRS,
            basemap.extent.clone(),
        );
        assert_overlay_matches_basemap(&basemap, &telemetry_overlay)
            .expect("telemetry overlay is constructed in the basemap CRS and extent");

        let flight_path: Vec<MapPathPoint> = samples
            .iter()
            .map(|sample| MapPathPoint::from_sample(sample, &basemap))
            .collect();
        let current_position = flight_path.last().cloned();
        let capture_markers = capture_events
            .iter()
            .filter_map(|event| CaptureMapMarker::from_input(event, &basemap))
            .collect::<Vec<_>>();
        let unmapped_capture_event_ids = capture_events
            .iter()
            .filter(|event| event.position.is_none())
            .map(|event| event.capture_event_id.clone())
            .collect::<Vec<_>>();
        let rendered_mission_overlay =
            mission_overlay.map(|overlay| MissionMapOverlay::from_input(overlay, &basemap));
        let geofence_breach = rendered_mission_overlay.as_ref().and_then(|overlay| {
            let marker = current_position.as_ref()?;
            let geofence = mission_overlay?.geofence.as_ref()?;
            Some(GeofenceBreach {
                mission_id: overlay.mission_id,
                latitude: marker.latitude,
                longitude: marker.longitude,
                outside: !point_is_inside_or_on_boundary(
                    marker.latitude,
                    marker.longitude,
                    &geofence.vertices,
                ),
            })
        });

        let mut overlay_assertions = vec![OverlayAssertion {
            overlay_id: telemetry_overlay.overlay_id,
            crs: telemetry_overlay.crs,
            accepted: true,
        }];
        if let Some(overlay) = &rendered_mission_overlay {
            overlay_assertions.extend(overlay.assertions(&basemap));
        }
        if !capture_markers.is_empty() {
            overlay_assertions.push(assert_projected_overlay("capture-markers", &basemap));
        }

        Self {
            basemap,
            current_position,
            flight_path,
            capture_markers,
            unmapped_capture_event_ids,
            mission_overlay: rendered_mission_overlay,
            geofence_breach,
            overlay_assertions,
        }
    }
}

impl CaptureMapMarker {
    fn from_input(input: &CaptureEventInput, basemap: &BasemapLayer) -> Option<Self> {
        let position = input.position.as_ref()?;
        let projected = project_wgs84_to_web_mercator(position);
        let (x_px, y_px) = project_to_canvas(&projected, basemap);
        Some(Self {
            capture_event_id: input.capture_event_id.clone(),
            timeline_entry_id: input.timeline_entry_id.clone(),
            captured_at: input.captured_at,
            latitude: position.latitude,
            longitude: position.longitude,
            altitude_m: position.altitude,
            source_crs: WGS84_CRS.to_string(),
            map_crs: basemap.crs.clone(),
            x_m: projected.x_m,
            y_m: projected.y_m,
            x_px,
            y_px,
        })
    }
}

impl MapPathPoint {
    fn from_sample(sample: &FlightPathSample, basemap: &BasemapLayer) -> Self {
        let projected = project_wgs84_to_web_mercator(&sample.gps_position());
        let (x_px, y_px) = project_to_canvas(&projected, basemap);

        Self {
            timestamp: sample.timestamp,
            latitude: sample.latitude,
            longitude: sample.longitude,
            altitude_m: sample.altitude_m,
            source_crs: WGS84_CRS.to_string(),
            map_crs: basemap.crs.clone(),
            x_m: projected.x_m,
            y_m: projected.y_m,
            x_px,
            y_px,
        }
    }
}

impl MissionMapOverlay {
    fn from_input(input: &MissionOverlayInput, basemap: &BasemapLayer) -> Self {
        Self {
            mission_id: input.mission_id,
            waypoints: input
                .waypoints
                .iter()
                .map(|waypoint| MissionWaypointOverlay::from_input(waypoint, basemap))
                .collect(),
            geofence: input
                .geofence
                .as_ref()
                .and_then(|polygon| MissionPolygonOverlay::from_input(polygon, basemap)),
            no_fly_zones: input
                .no_fly_zones
                .iter()
                .filter_map(|polygon| MissionPolygonOverlay::from_input(polygon, basemap))
                .collect(),
        }
    }

    fn assertions(&self, basemap: &BasemapLayer) -> Vec<OverlayAssertion> {
        let mut assertions = Vec::new();
        if self.geofence.is_some() {
            assertions.push(assert_projected_overlay("mission-geofence", basemap));
        }
        assertions.extend(
            self.no_fly_zones
                .iter()
                .map(|zone| assert_projected_overlay(&zone.polygon_id, basemap)),
        );
        if !self.waypoints.is_empty() {
            assertions.push(assert_projected_overlay("mission-waypoints", basemap));
        }
        assertions
    }
}

impl MissionWaypointOverlay {
    fn from_input(input: &MissionWaypointInput, basemap: &BasemapLayer) -> Self {
        let projected = project_wgs84_to_web_mercator(&input.position);
        let (x_px, y_px) = project_to_canvas(&projected, basemap);
        Self {
            sequence: input.sequence,
            latitude: input.position.latitude,
            longitude: input.position.longitude,
            altitude_m: input.position.altitude,
            source_crs: WGS84_CRS.to_string(),
            map_crs: basemap.crs.clone(),
            x_m: projected.x_m,
            y_m: projected.y_m,
            x_px,
            y_px,
        }
    }
}

impl MissionPolygonOverlay {
    fn from_input(input: &MissionPolygonInput, basemap: &BasemapLayer) -> Option<Self> {
        if input.source_crs != WGS84_CRS || input.vertices.len() < 3 {
            return None;
        }

        Some(Self {
            polygon_id: input.polygon_id.clone(),
            source_crs: input.source_crs.clone(),
            map_crs: basemap.crs.clone(),
            vertices: input
                .vertices
                .iter()
                .map(|vertex| MissionPolygonVertex::from_gps(vertex, basemap))
                .collect(),
        })
    }
}

impl MissionPolygonVertex {
    fn from_gps(position: &GpsCoords, basemap: &BasemapLayer) -> Self {
        let projected = project_wgs84_to_web_mercator(position);
        let (x_px, y_px) = project_to_canvas(&projected, basemap);
        Self {
            latitude: position.latitude,
            longitude: position.longitude,
            x_m: projected.x_m,
            y_m: projected.y_m,
            x_px,
            y_px,
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MapRenderError {
    #[error("overlay {overlay_id} CRS {overlay_crs} does not match basemap CRS {basemap_crs}")]
    CrsMismatch {
        overlay_id: String,
        basemap_crs: String,
        overlay_crs: String,
    },
    #[error(
        "overlay {overlay_id} extent edge {edge} differs from basemap: expected {expected}, actual {actual}, tolerance {tolerance}"
    )]
    ExtentMismatch {
        overlay_id: String,
        edge: &'static str,
        expected: f64,
        actual: f64,
        tolerance: f64,
    },
}

pub fn assert_overlay_matches_basemap(
    basemap: &BasemapLayer,
    overlay: &MapOverlayLayer,
) -> Result<(), MapRenderError> {
    if overlay.crs != basemap.crs || overlay.extent.crs != basemap.crs {
        return Err(MapRenderError::CrsMismatch {
            overlay_id: overlay.overlay_id.clone(),
            basemap_crs: basemap.crs.clone(),
            overlay_crs: overlay.crs.clone(),
        });
    }

    assert_extent_edge(
        &overlay.overlay_id,
        "min_x_m",
        basemap.extent.min_x_m,
        overlay.extent.min_x_m,
    )?;
    assert_extent_edge(
        &overlay.overlay_id,
        "min_y_m",
        basemap.extent.min_y_m,
        overlay.extent.min_y_m,
    )?;
    assert_extent_edge(
        &overlay.overlay_id,
        "max_x_m",
        basemap.extent.max_x_m,
        overlay.extent.max_x_m,
    )?;
    assert_extent_edge(
        &overlay.overlay_id,
        "max_y_m",
        basemap.extent.max_y_m,
        overlay.extent.max_y_m,
    )?;

    Ok(())
}

pub fn project_wgs84_to_web_mercator(position: &GpsCoords) -> ProjectedPoint {
    let latitude = position
        .latitude
        .clamp(-MAX_WEB_MERCATOR_LAT, MAX_WEB_MERCATOR_LAT);
    let longitude = position.longitude.clamp(-180.0, 180.0);
    let lon_rad = longitude.to_radians();
    let lat_rad = latitude.to_radians();

    ProjectedPoint {
        x_m: EARTH_RADIUS_M * lon_rad,
        y_m: EARTH_RADIUS_M * ((std::f64::consts::FRAC_PI_4 + lat_rad / 2.0).tan()).ln(),
        crs: WEB_MERCATOR_CRS.to_string(),
    }
}

fn project_to_canvas(projected: &ProjectedPoint, basemap: &BasemapLayer) -> (f64, f64) {
    let extent = &basemap.extent;
    let x_ratio = (projected.x_m - extent.min_x_m) / extent.width_m();
    let y_ratio = (projected.y_m - extent.min_y_m) / extent.height_m();
    (
        x_ratio * basemap.width_px as f64,
        (1.0 - y_ratio) * basemap.height_px as f64,
    )
}

fn assert_projected_overlay(overlay_id: &str, basemap: &BasemapLayer) -> OverlayAssertion {
    let overlay = MapOverlayLayer::new(overlay_id, WEB_MERCATOR_CRS, basemap.extent.clone());
    let accepted = assert_overlay_matches_basemap(basemap, &overlay).is_ok();
    OverlayAssertion {
        overlay_id: overlay_id.to_string(),
        crs: WEB_MERCATOR_CRS.to_string(),
        accepted,
    }
}

fn point_is_inside_or_on_boundary(latitude: f64, longitude: f64, polygon: &[GpsCoords]) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let mut inside = false;
    let mut previous = polygon.last().expect("polygon has at least 3 vertices");
    for current in polygon {
        if point_on_segment(latitude, longitude, previous, current) {
            return true;
        }

        let yi = previous.latitude;
        let yj = current.latitude;
        let xi = previous.longitude;
        let xj = current.longitude;
        let crosses_latitude = (yi > latitude) != (yj > latitude);
        if crosses_latitude {
            let crossing_longitude = (xj - xi) * (latitude - yi) / (yj - yi) + xi;
            if longitude < crossing_longitude {
                inside = !inside;
            }
        }
        previous = current;
    }

    inside
}

fn point_on_segment(latitude: f64, longitude: f64, start: &GpsCoords, end: &GpsCoords) -> bool {
    const EPSILON: f64 = 1e-10;
    let cross = (longitude - start.longitude) * (end.latitude - start.latitude)
        - (latitude - start.latitude) * (end.longitude - start.longitude);
    if cross.abs() > EPSILON {
        return false;
    }

    let within_longitude = longitude >= start.longitude.min(end.longitude) - EPSILON
        && longitude <= start.longitude.max(end.longitude) + EPSILON;
    let within_latitude = latitude >= start.latitude.min(end.latitude) - EPSILON
        && latitude <= start.latitude.max(end.latitude) + EPSILON;
    within_longitude && within_latitude
}

fn assert_extent_edge(
    overlay_id: &str,
    edge: &'static str,
    expected: f64,
    actual: f64,
) -> Result<(), MapRenderError> {
    if (expected - actual).abs() <= EXTENT_TOLERANCE_M {
        Ok(())
    } else {
        Err(MapRenderError::ExtentMismatch {
            overlay_id: overlay_id.to_string(),
            edge,
            expected,
            actual,
            tolerance: EXTENT_TOLERANCE_M,
        })
    }
}
