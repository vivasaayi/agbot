use chrono::{DateTime, Utc};
use serde::Serialize;
use shared::schemas::{GpsCoords, Telemetry};

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
        let projected_points: Vec<ProjectedPoint> = samples
            .iter()
            .map(|sample| project_wgs84_to_web_mercator(&sample.gps_position()))
            .collect();

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
    pub overlay_assertions: Vec<OverlayAssertion>,
}

impl MapRenderState {
    pub fn from_flight_path(samples: &[FlightPathSample]) -> Self {
        let basemap = BasemapLayer::from_path(samples);
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

        Self {
            basemap,
            current_position,
            flight_path,
            overlay_assertions: vec![OverlayAssertion {
                overlay_id: telemetry_overlay.overlay_id,
                crs: telemetry_overlay.crs,
                accepted: true,
            }],
        }
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
