use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// GPS coordinates
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GpsCoords {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoBounds {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    pub longitude: f64,
    pub latitude: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldBoundary {
    pub coordinates: Vec<GeoPoint>,
    #[serde(default)]
    pub crs: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidatedFieldBoundary {
    pub boundary: FieldBoundary,
    pub extent: GeoBounds,
    pub area_ha: f64,
}

pub const DEFAULT_RECORD_OWNER: &str = "unassigned";

fn default_record_owner() -> String {
    DEFAULT_RECORD_OWNER.to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FarmRecord {
    pub farm_id: String,
    #[serde(default = "default_record_owner")]
    pub org_id: String,
    #[serde(default = "default_record_owner")]
    pub owner: String,
    pub name: String,
    pub notes: Option<String>,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldRecord {
    #[serde(default)]
    pub farm_id: Option<String>,
    pub field_id: String,
    #[serde(default = "default_record_owner")]
    pub org_id: String,
    #[serde(default = "default_record_owner")]
    pub owner: String,
    pub name: String,
    #[serde(default)]
    pub area_ha: Option<f64>,
    pub crop: Option<String>,
    pub season: Option<String>,
    pub notes: Option<String>,
    pub boundary: FieldBoundary,
    pub extent: GeoBounds,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FarmFieldError {
    #[error("farm_id cannot be empty")]
    EmptyFarmId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("org_id cannot be empty")]
    EmptyOrgId,
    #[error("name cannot be empty")]
    EmptyName,
    #[error("field requires a farm_id: {field_id}")]
    MissingFarmId { field_id: String },
    #[error("farm not found: {farm_id}")]
    FarmNotFound { farm_id: String },
    #[error("farm {farm_id} belongs to {farm_org_id}, not {field_org_id}")]
    TenantBoundary {
        farm_id: String,
        farm_org_id: String,
        field_org_id: String,
    },
    #[error("invalid field boundary: {reason}")]
    BoundaryInvalid {
        reason: FieldBoundaryValidationError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FieldBoundaryValidationError {
    #[error("field boundary must declare a CRS")]
    MissingCrs,
    #[error("field boundary must contain a closed polygon ring")]
    TooFewCoordinates,
    #[error("field boundary contains invalid geographic coordinates")]
    InvalidCoordinate,
    #[error("field boundary polygon ring is not closed")]
    RingNotClosed,
    #[error("field boundary polygon self-intersects")]
    SelfIntersection,
    #[error("field boundary area is empty")]
    EmptyArea,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FarmFieldRegistry {
    farms: HashMap<String, FarmRecord>,
    fields: HashMap<String, FieldRecord>,
}

impl FarmFieldRegistry {
    pub fn insert_farm(&mut self, farm: FarmRecord) -> Result<FarmRecord, FarmFieldError> {
        let mut farm = normalize_farm_record(farm)?;
        farm.owner = farm.org_id.clone();
        self.farms.insert(farm.farm_id.clone(), farm.clone());
        Ok(farm)
    }

    pub fn insert_field(&mut self, field: FieldRecord) -> Result<FieldRecord, FarmFieldError> {
        let mut field = normalize_field_record(field)?;
        let farm_id = field
            .farm_id
            .clone()
            .ok_or_else(|| FarmFieldError::MissingFarmId {
                field_id: field.field_id.clone(),
            })?;
        let farm = self
            .farms
            .get(&farm_id)
            .ok_or_else(|| FarmFieldError::FarmNotFound {
                farm_id: farm_id.clone(),
            })?;
        if farm.org_id != field.org_id {
            return Err(FarmFieldError::TenantBoundary {
                farm_id,
                farm_org_id: farm.org_id.clone(),
                field_org_id: field.org_id,
            });
        }

        let validated = validate_field_boundary(&field.boundary)
            .map_err(|reason| FarmFieldError::BoundaryInvalid { reason })?;
        field.extent = validated.extent;
        field.area_ha = Some(validated.area_ha);
        field.owner = field.org_id.clone();
        self.fields.insert(field.field_id.clone(), field.clone());
        Ok(field)
    }

    pub fn farms_for_org(&self, org_id: &str) -> Vec<FarmRecord> {
        let mut farms = self
            .farms
            .values()
            .filter(|farm| farm.org_id == org_id)
            .cloned()
            .collect::<Vec<_>>();
        farms.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.farm_id.cmp(&right.farm_id))
        });
        farms
    }

    pub fn fields_for_org(&self, org_id: &str) -> Vec<FieldRecord> {
        let mut fields = self
            .fields
            .values()
            .filter(|field| field.org_id == org_id)
            .cloned()
            .collect::<Vec<_>>();
        fields.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.field_id.cmp(&right.field_id))
        });
        fields
    }

    pub fn farm_for_org(&self, org_id: &str, farm_id: &str) -> Option<FarmRecord> {
        self.farms
            .get(farm_id)
            .filter(|farm| farm.org_id == org_id)
            .cloned()
    }

    pub fn field_for_org(&self, org_id: &str, field_id: &str) -> Option<FieldRecord> {
        self.fields
            .get(field_id)
            .filter(|field| field.org_id == org_id)
            .cloned()
    }
}

fn normalize_farm_record(mut farm: FarmRecord) -> Result<FarmRecord, FarmFieldError> {
    farm.farm_id = normalize_farm_field_text(farm.farm_id).ok_or(FarmFieldError::EmptyFarmId)?;
    farm.org_id = normalize_farm_field_text(farm.org_id).ok_or(FarmFieldError::EmptyOrgId)?;
    farm.name = normalize_farm_field_text(farm.name).ok_or(FarmFieldError::EmptyName)?;
    Ok(farm)
}

fn normalize_field_record(mut field: FieldRecord) -> Result<FieldRecord, FarmFieldError> {
    field.field_id =
        normalize_farm_field_text(field.field_id).ok_or(FarmFieldError::EmptyFieldId)?;
    field.org_id = normalize_farm_field_text(field.org_id).ok_or(FarmFieldError::EmptyOrgId)?;
    field.name = normalize_farm_field_text(field.name).ok_or(FarmFieldError::EmptyName)?;
    field.farm_id = field.farm_id.and_then(normalize_farm_field_text);
    Ok(field)
}

fn normalize_farm_field_text(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

pub fn validate_field_boundary(
    boundary: &FieldBoundary,
) -> Result<ValidatedFieldBoundary, FieldBoundaryValidationError> {
    let crs = boundary
        .crs
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(FieldBoundaryValidationError::MissingCrs)?;
    let coordinates = &boundary.coordinates;
    if coordinates.len() < 4 {
        return Err(FieldBoundaryValidationError::TooFewCoordinates);
    }
    if coordinates.iter().any(|point| {
        !point.longitude.is_finite()
            || !point.latitude.is_finite()
            || point.longitude < -180.0
            || point.longitude > 180.0
            || point.latitude < -90.0
            || point.latitude > 90.0
    }) {
        return Err(FieldBoundaryValidationError::InvalidCoordinate);
    }
    if !points_approximately_equal(
        coordinates.first().expect("coordinates length checked"),
        coordinates.last().expect("coordinates length checked"),
    ) {
        return Err(FieldBoundaryValidationError::RingNotClosed);
    }
    if ring_self_intersects(coordinates) {
        return Err(FieldBoundaryValidationError::SelfIntersection);
    }

    let extent =
        bounds_from_points(coordinates).ok_or(FieldBoundaryValidationError::InvalidCoordinate)?;
    let area_ha = polygon_area_hectares(coordinates);
    if area_ha <= f64::EPSILON {
        return Err(FieldBoundaryValidationError::EmptyArea);
    }

    Ok(ValidatedFieldBoundary {
        boundary: FieldBoundary {
            coordinates: coordinates.clone(),
            crs: Some(crs.to_string()),
        },
        extent,
        area_ha,
    })
}

fn points_approximately_equal(left: &GeoPoint, right: &GeoPoint) -> bool {
    const EPSILON: f64 = 1e-9;
    (left.longitude - right.longitude).abs() <= EPSILON
        && (left.latitude - right.latitude).abs() <= EPSILON
}

fn ring_self_intersects(points: &[GeoPoint]) -> bool {
    let segment_count = points.len().saturating_sub(1);
    for left in 0..segment_count {
        for right in (left + 1)..segment_count {
            if segments_share_ring_vertex(left, right, segment_count) {
                continue;
            }
            if segments_intersect(
                &points[left],
                &points[left + 1],
                &points[right],
                &points[right + 1],
            ) {
                return true;
            }
        }
    }
    false
}

fn segments_share_ring_vertex(left: usize, right: usize, segment_count: usize) -> bool {
    left == right || left + 1 == right || (left == 0 && right + 1 == segment_count)
}

fn segments_intersect(a: &GeoPoint, b: &GeoPoint, c: &GeoPoint, d: &GeoPoint) -> bool {
    let o1 = orientation(a, b, c);
    let o2 = orientation(a, b, d);
    let o3 = orientation(c, d, a);
    let o4 = orientation(c, d, b);

    if orientation_sign(o1) != orientation_sign(o2) && orientation_sign(o3) != orientation_sign(o4)
    {
        return true;
    }

    (orientation_is_colinear(o1) && point_on_segment(a, c, b))
        || (orientation_is_colinear(o2) && point_on_segment(a, d, b))
        || (orientation_is_colinear(o3) && point_on_segment(c, a, d))
        || (orientation_is_colinear(o4) && point_on_segment(c, b, d))
}

fn orientation(a: &GeoPoint, b: &GeoPoint, c: &GeoPoint) -> f64 {
    (b.longitude - a.longitude) * (c.latitude - a.latitude)
        - (b.latitude - a.latitude) * (c.longitude - a.longitude)
}

fn orientation_sign(value: f64) -> i8 {
    if orientation_is_colinear(value) {
        0
    } else if value > 0.0 {
        1
    } else {
        -1
    }
}

fn orientation_is_colinear(value: f64) -> bool {
    value.abs() <= 1e-12
}

fn point_on_segment(start: &GeoPoint, point: &GeoPoint, end: &GeoPoint) -> bool {
    point.longitude >= start.longitude.min(end.longitude) - 1e-12
        && point.longitude <= start.longitude.max(end.longitude) + 1e-12
        && point.latitude >= start.latitude.min(end.latitude) - 1e-12
        && point.latitude <= start.latitude.max(end.latitude) + 1e-12
}

fn polygon_area_hectares(points: &[GeoPoint]) -> f64 {
    let mean_latitude =
        points.iter().map(|point| point.latitude).sum::<f64>() / points.len() as f64;
    let meters_per_degree_lat = 111_320.0;
    let meters_per_degree_lon = meters_per_degree_lat * mean_latitude.to_radians().cos();
    let area_m2 = points
        .windows(2)
        .map(|window| {
            let x1 = window[0].longitude * meters_per_degree_lon;
            let y1 = window[0].latitude * meters_per_degree_lat;
            let x2 = window[1].longitude * meters_per_degree_lon;
            let y2 = window[1].latitude * meters_per_degree_lat;
            x1 * y2 - x2 * y1
        })
        .sum::<f64>()
        .abs()
        * 0.5;
    area_m2 / 10_000.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnnotationGeometry {
    Point { coordinate: GeoPoint },
    Polygon { coordinates: Vec<GeoPoint> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationRecord {
    pub annotation_id: String,
    pub scene_id: String,
    pub field_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_id: Option<String>,
    pub label: String,
    pub note: Option<String>,
    pub severity: Option<String>,
    pub geometry: AnnotationGeometry,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationStatus {
    Open,
    Reviewed,
    Closed,
}

impl Default for RecommendationStatus {
    fn default() -> Self {
        Self::Open
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl Default for RecommendationPriority {
    fn default() -> Self {
        Self::Medium
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecommendationRecord {
    pub recommendation_id: String,
    pub scene_id: String,
    pub field_id: Option<String>,
    pub title: String,
    pub note: Option<String>,
    pub category: Option<String>,
    pub priority: RecommendationPriority,
    pub status: RecommendationStatus,
    #[serde(default)]
    pub annotation_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
    Html,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportRecord {
    pub report_id: String,
    pub scene_id: String,
    pub field_id: Option<String>,
    pub title: String,
    pub format: ReportFormat,
    pub artifact_path: String,
    pub download_url: String,
    pub annotation_count: usize,
    pub recommendation_count: usize,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RasterSpatialRef {
    #[serde(default)]
    pub georeferenced: bool,
    #[serde(default)]
    pub crs: Option<String>,
    #[serde(default)]
    pub bbox: Option<GeoBounds>,
    #[serde(default)]
    pub geo_transform: Option<[f64; 6]>,
    #[serde(default)]
    pub resolution: Option<RasterResolution>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RasterResolution {
    pub x: f64,
    pub y: f64,
}

pub const GEO_EXTENT_ASSERTION_TOLERANCE: f64 = 1.0e-9;
pub const RASTER_RESOLUTION_RELATIVE_TOLERANCE: f64 = 0.05;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RasterSpatialRefError {
    #[error("georeferencing missing spatial_ref")]
    MissingSpatialRef,
    #[error("georeferencing raster dimensions must be positive")]
    NonPositiveDimensions,
    #[error("georeferencing spatial_ref is not marked georeferenced")]
    NotGeoreferenced,
    #[error("georeferencing missing CRS")]
    MissingCrs,
    #[error("georeferencing missing extent bbox")]
    MissingBbox,
    #[error("georeferencing missing transform")]
    MissingTransform,
    #[error("georeferencing transform contains a non-finite value")]
    InvalidTransform,
    #[error("georeferencing requires positive resolution")]
    NonPositiveResolution,
    #[error(
        "georeferencing declared resolution {axis}={declared} differs from transform-derived {derived} beyond tolerance {tolerance}"
    )]
    ResolutionMismatch {
        axis: &'static str,
        declared: f64,
        derived: f64,
        tolerance: f64,
    },
    #[error(
        "georeferencing extent edge {edge}={actual} differs from transform-derived {expected} beyond GEO tolerance {tolerance}"
    )]
    ExtentMismatch {
        edge: &'static str,
        actual: f64,
        expected: f64,
        tolerance: f64,
    },
}

pub fn assert_raster_spatial_ref(
    spatial_ref: Option<&RasterSpatialRef>,
    width: u32,
    height: u32,
) -> Result<RasterSpatialRef, RasterSpatialRefError> {
    let spatial_ref = spatial_ref.ok_or(RasterSpatialRefError::MissingSpatialRef)?;
    if width == 0 || height == 0 {
        return Err(RasterSpatialRefError::NonPositiveDimensions);
    }
    if !spatial_ref.georeferenced {
        return Err(RasterSpatialRefError::NotGeoreferenced);
    }
    let crs = spatial_ref
        .crs
        .as_deref()
        .map(str::trim)
        .filter(|crs| !crs.is_empty())
        .ok_or(RasterSpatialRefError::MissingCrs)?;
    let bbox = spatial_ref
        .bbox
        .as_ref()
        .ok_or(RasterSpatialRefError::MissingBbox)?;
    let transform = spatial_ref
        .geo_transform
        .ok_or(RasterSpatialRefError::MissingTransform)?;
    if !transform.iter().all(|value| value.is_finite()) {
        return Err(RasterSpatialRefError::InvalidTransform);
    }

    let derived_resolution = transform_resolution(&transform)?;
    let resolution = match spatial_ref.resolution {
        Some(declared) => {
            validate_positive_resolution(declared)?;
            assert_resolution_matches("x", declared.x, derived_resolution.x)?;
            assert_resolution_matches("y", declared.y, derived_resolution.y)?;
            declared
        }
        None => derived_resolution,
    };

    let expected_bbox = transform_bbox(&transform, width, height);
    assert_extent_edge("min_lon", bbox.min_lon, expected_bbox.min_lon)?;
    assert_extent_edge("min_lat", bbox.min_lat, expected_bbox.min_lat)?;
    assert_extent_edge("max_lon", bbox.max_lon, expected_bbox.max_lon)?;
    assert_extent_edge("max_lat", bbox.max_lat, expected_bbox.max_lat)?;

    Ok(RasterSpatialRef {
        georeferenced: true,
        crs: Some(crs.to_string()),
        bbox: Some(bbox.clone()),
        geo_transform: Some(transform),
        resolution: Some(resolution),
    })
}

fn transform_resolution(transform: &[f64; 6]) -> Result<RasterResolution, RasterSpatialRefError> {
    let resolution = RasterResolution {
        x: transform[1].hypot(transform[4]),
        y: transform[2].hypot(transform[5]),
    };
    validate_positive_resolution(resolution)?;
    Ok(resolution)
}

fn validate_positive_resolution(resolution: RasterResolution) -> Result<(), RasterSpatialRefError> {
    if resolution.x.is_finite()
        && resolution.y.is_finite()
        && resolution.x > 0.0
        && resolution.y > 0.0
    {
        Ok(())
    } else {
        Err(RasterSpatialRefError::NonPositiveResolution)
    }
}

fn assert_resolution_matches(
    axis: &'static str,
    declared: f64,
    derived: f64,
) -> Result<(), RasterSpatialRefError> {
    let relative_delta = ((declared - derived) / derived).abs();
    if relative_delta <= RASTER_RESOLUTION_RELATIVE_TOLERANCE {
        Ok(())
    } else {
        Err(RasterSpatialRefError::ResolutionMismatch {
            axis,
            declared,
            derived,
            tolerance: RASTER_RESOLUTION_RELATIVE_TOLERANCE,
        })
    }
}

fn transform_bbox(transform: &[f64; 6], width: u32, height: u32) -> GeoBounds {
    let width = width as f64;
    let height = height as f64;
    let corners = [
        transform_point(transform, 0.0, 0.0),
        transform_point(transform, width, 0.0),
        transform_point(transform, 0.0, height),
        transform_point(transform, width, height),
    ];

    let mut min_lon = f64::INFINITY;
    let mut min_lat = f64::INFINITY;
    let mut max_lon = f64::NEG_INFINITY;
    let mut max_lat = f64::NEG_INFINITY;
    for (lon, lat) in corners {
        min_lon = min_lon.min(lon);
        min_lat = min_lat.min(lat);
        max_lon = max_lon.max(lon);
        max_lat = max_lat.max(lat);
    }

    GeoBounds {
        min_lon,
        min_lat,
        max_lon,
        max_lat,
    }
}

fn transform_point(transform: &[f64; 6], x: f64, y: f64) -> (f64, f64) {
    (
        transform[0] + transform[1] * x + transform[2] * y,
        transform[3] + transform[4] * x + transform[5] * y,
    )
}

fn assert_extent_edge(
    edge: &'static str,
    actual: f64,
    expected: f64,
) -> Result<(), RasterSpatialRefError> {
    if actual.is_finite()
        && expected.is_finite()
        && (actual - expected).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
    {
        Ok(())
    } else {
        Err(RasterSpatialRefError::ExtentMismatch {
            edge,
            actual,
            expected,
            tolerance: GEO_EXTENT_ASSERTION_TOLERANCE,
        })
    }
}

/// Telemetry data from flight controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Telemetry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub position: GpsCoords,
    pub battery_voltage: f32,
    pub battery_percentage: u8,
    pub armed: bool,
    pub mode: String,
    pub ground_speed: f32,
    pub air_speed: f32,
    pub heading: f32,
    pub altitude_relative: f32,
}

/// Mission waypoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Waypoint {
    pub sequence: u16,
    pub position: GpsCoords,
    pub command: u16,
    pub auto_continue: bool,
    pub param1: f32,
    pub param2: f32,
    pub param3: f32,
    pub param4: f32,
}

/// Complete mission definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    pub id: uuid::Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub waypoints: Vec<Waypoint>,
    pub home_position: GpsCoords,
}

/// LiDAR scan point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidarPoint {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub angle: f32,
    pub distance: f32,
    pub quality: u8,
}

/// LiDAR scan containing multiple points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidarScan {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub points: Vec<LidarPoint>,
    pub scan_id: uuid::Uuid,
}

/// Multispectral image metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub gps_position: Option<GpsCoords>,
    pub bands: Vec<String>,
    pub exposure_time: f32,
    pub gain: f32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub spatial_ref: Option<RasterSpatialRef>,
}

/// Captured multispectral image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultispectralImage {
    pub metadata: ImageMetadata,
    pub file_paths: HashMap<String, String>, // band_name -> file_path
    pub image_id: uuid::Uuid,
}

/// NDVI processing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NdviResult {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source_images: Vec<uuid::Uuid>,
    pub output_path: String,
    pub min_ndvi: f32,
    pub max_ndvi: f32,
    pub mean_ndvi: f32,
    pub vegetation_percentage: f32,
}

/// WebSocket message types for ground station communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    Telemetry {
        data: Telemetry,
    },
    MissionStatus {
        mission_id: uuid::Uuid,
        status: String,
    },
    LidarUpdate {
        scan: LidarScan,
    },
    ImageCaptured {
        image: MultispectralImage,
    },
    NdviProcessed {
        result: NdviResult,
    },
    SystemStatus {
        status: String,
        message: String,
    },
}

pub fn bounds_from_points(points: &[GeoPoint]) -> Option<GeoBounds> {
    let mut iter = points.iter();
    let first = iter.next()?;

    let mut min_lon = first.longitude;
    let mut max_lon = first.longitude;
    let mut min_lat = first.latitude;
    let mut max_lat = first.latitude;

    for point in iter {
        min_lon = min_lon.min(point.longitude);
        max_lon = max_lon.max(point.longitude);
        min_lat = min_lat.min(point.latitude);
        max_lat = max_lat.max(point.latitude);
    }

    Some(GeoBounds {
        min_lon,
        min_lat,
        max_lon,
        max_lat,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        assert_raster_spatial_ref, bounds_from_points, validate_field_boundary, AnnotationGeometry,
        AnnotationRecord, FarmFieldError, FarmFieldRegistry, FarmRecord, FieldBoundary,
        FieldBoundaryValidationError, FieldRecord, GeoBounds, GeoPoint, MultispectralImage,
        RasterResolution, RasterSpatialRef, RasterSpatialRefError, RecommendationPriority,
        RecommendationRecord, RecommendationStatus, ReportFormat, ReportRecord,
    };

    #[test]
    fn multispectral_image_deserializes_without_spatial_ref() {
        let payload = serde_json::json!({
            "metadata": {
                "timestamp": "2025-01-01T00:00:00Z",
                "gps_position": null,
                "bands": ["B4", "B5"],
                "exposure_time": 1.0,
                "gain": 1.0,
                "width": 64,
                "height": 32
            },
            "file_paths": {
                "B4": "B4.tif",
                "B5": "B5.tif"
            },
            "image_id": "00000000-0000-0000-0000-000000000000"
        });

        let image: MultispectralImage =
            serde_json::from_value(payload).expect("legacy metadata should deserialize");

        assert_eq!(image.metadata.spatial_ref, None);
    }

    #[test]
    fn multispectral_image_deserializes_with_spatial_ref() {
        let payload = serde_json::json!({
            "metadata": {
                "timestamp": "2025-01-01T00:00:00Z",
                "gps_position": null,
                "bands": ["B4", "B5"],
                "exposure_time": 1.0,
                "gain": 1.0,
                "width": 64,
                "height": 32,
                "spatial_ref": {
                    "georeferenced": true,
                    "crs": "EPSG:4326",
                    "bbox": {
                        "min_lon": -74.1,
                        "min_lat": 40.6,
                        "max_lon": -73.9,
                        "max_lat": 40.8
                    },
                    "geo_transform": [-74.1, 0.0001, 0.0, 40.8, 0.0, -0.0001]
                }
            },
            "file_paths": {
                "B4": "B4.tif",
                "B5": "B5.tif"
            },
            "image_id": "00000000-0000-0000-0000-000000000000"
        });

        let image: MultispectralImage =
            serde_json::from_value(payload).expect("spatial metadata should deserialize");

        assert_eq!(
            image.metadata.spatial_ref,
            Some(RasterSpatialRef {
                georeferenced: true,
                crs: Some("EPSG:4326".to_string()),
                bbox: Some(super::GeoBounds {
                    min_lon: -74.1,
                    min_lat: 40.6,
                    max_lon: -73.9,
                    max_lat: 40.8,
                }),
                geo_transform: Some([-74.1, 0.0001, 0.0, 40.8, 0.0, -0.0001]),
                resolution: None,
            })
        );
    }

    #[test]
    fn raster_spatial_ref_asserts_extent_and_resolution() {
        let spatial_ref = RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:4326".to_string()),
            bbox: Some(GeoBounds {
                min_lon: -74.1,
                min_lat: 40.7998,
                max_lon: -74.0998,
                max_lat: 40.8,
            }),
            geo_transform: Some([-74.1, 0.0001, 0.0, 40.8, 0.0, -0.0001]),
            resolution: None,
        };

        let asserted =
            assert_raster_spatial_ref(Some(&spatial_ref), 2, 2).expect("spatial ref should assert");

        assert_eq!(asserted.crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(
            asserted.resolution,
            Some(RasterResolution {
                x: 0.0001,
                y: 0.0001
            })
        );
    }

    #[test]
    fn raster_spatial_ref_rejects_missing_crs() {
        let spatial_ref = RasterSpatialRef {
            georeferenced: true,
            crs: Some(" ".to_string()),
            bbox: Some(GeoBounds {
                min_lon: -74.1,
                min_lat: 40.7999,
                max_lon: -74.0999,
                max_lat: 40.8,
            }),
            geo_transform: Some([-74.1, 0.0001, 0.0, 40.8, 0.0, -0.0001]),
            resolution: None,
        };

        let error = assert_raster_spatial_ref(Some(&spatial_ref), 1, 1).unwrap_err();

        assert_eq!(error, RasterSpatialRefError::MissingCrs);
    }

    #[test]
    fn raster_spatial_ref_rejects_non_positive_resolution() {
        let spatial_ref = RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:4326".to_string()),
            bbox: Some(GeoBounds {
                min_lon: -74.1,
                min_lat: 40.7999,
                max_lon: -74.1,
                max_lat: 40.8,
            }),
            geo_transform: Some([-74.1, 0.0, 0.0, 40.8, 0.0, -0.0001]),
            resolution: None,
        };

        let error = assert_raster_spatial_ref(Some(&spatial_ref), 1, 1).unwrap_err();

        assert_eq!(error, RasterSpatialRefError::NonPositiveResolution);
    }

    #[test]
    fn bounds_from_points_computes_expected_bbox() {
        let bounds = bounds_from_points(&[
            GeoPoint {
                longitude: -96.5,
                latitude: 41.2,
            },
            GeoPoint {
                longitude: -96.2,
                latitude: 41.4,
            },
            GeoPoint {
                longitude: -96.7,
                latitude: 41.1,
            },
        ])
        .expect("bounds should exist");

        assert_eq!(
            bounds,
            GeoBounds {
                min_lon: -96.7,
                min_lat: 41.1,
                max_lon: -96.2,
                max_lat: 41.4,
            }
        );
    }

    #[test]
    fn field_record_round_trips_through_json() {
        let field = FieldRecord {
            farm_id: Some("farm-1".to_string()),
            field_id: "field-1".to_string(),
            org_id: "org-1".to_string(),
            owner: "org-1".to_string(),
            name: "North 80".to_string(),
            area_ha: Some(32.4),
            crop: Some("corn".to_string()),
            season: Some("2026".to_string()),
            notes: Some("pivot irrigation".to_string()),
            boundary: FieldBoundary {
                crs: Some("EPSG:4326".to_string()),
                coordinates: vec![
                    GeoPoint {
                        longitude: -96.5,
                        latitude: 41.2,
                    },
                    GeoPoint {
                        longitude: -96.2,
                        latitude: 41.2,
                    },
                    GeoPoint {
                        longitude: -96.2,
                        latitude: 41.4,
                    },
                ],
            },
            extent: GeoBounds {
                min_lon: -96.5,
                min_lat: 41.2,
                max_lon: -96.2,
                max_lat: 41.4,
            },
            created_at: "2026-04-01T00:00:00Z".to_string(),
        };

        let value = serde_json::to_value(&field).expect("field should serialize");
        let decoded: FieldRecord = serde_json::from_value(value).expect("field should deserialize");

        assert_eq!(decoded, field);
    }

    #[test]
    fn farm_field_registry_lists_records_under_org_only() {
        let mut registry = FarmFieldRegistry::default();
        let farm = FarmRecord {
            farm_id: "farm-a".to_string(),
            org_id: "org-a".to_string(),
            owner: "org-a".to_string(),
            name: "Prairie Farm".to_string(),
            notes: None,
            created_at: "2026-04-01T00:00:00Z".to_string(),
        };
        let field = FieldRecord {
            farm_id: Some(farm.farm_id.clone()),
            field_id: "field-a".to_string(),
            org_id: "org-a".to_string(),
            owner: "org-a".to_string(),
            name: "North 80".to_string(),
            area_ha: Some(32.4),
            crop: Some("corn".to_string()),
            season: Some("2026".to_string()),
            notes: None,
            boundary: test_boundary(),
            extent: test_extent(),
            created_at: "2026-04-01T00:00:00Z".to_string(),
        };

        registry.insert_farm(farm).expect("farm persists");
        registry.insert_field(field).expect("field persists");

        let farms = registry.farms_for_org("org-a");
        let fields = registry.fields_for_org("org-a");

        assert_eq!(farms.len(), 1);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].farm_id.as_deref(), Some("farm-a"));
        assert_eq!(fields[0].org_id, "org-a");
        assert!(registry.farms_for_org("org-b").is_empty());
        assert!(registry.fields_for_org("org-b").is_empty());
    }

    #[test]
    fn field_with_cross_org_farm_is_rejected_without_writing() {
        let mut registry = FarmFieldRegistry::default();
        registry
            .insert_farm(FarmRecord {
                farm_id: "farm-a".to_string(),
                org_id: "org-a".to_string(),
                owner: "org-a".to_string(),
                name: "Prairie Farm".to_string(),
                notes: None,
                created_at: "2026-04-01T00:00:00Z".to_string(),
            })
            .expect("farm persists");

        let error = registry
            .insert_field(FieldRecord {
                farm_id: Some("farm-a".to_string()),
                field_id: "field-b".to_string(),
                org_id: "org-b".to_string(),
                owner: "org-b".to_string(),
                name: "Other Org Field".to_string(),
                area_ha: None,
                crop: None,
                season: None,
                notes: None,
                boundary: test_boundary(),
                extent: test_extent(),
                created_at: "2026-04-01T00:00:00Z".to_string(),
            })
            .expect_err("cross-org farm link is rejected");

        assert_eq!(
            error,
            FarmFieldError::TenantBoundary {
                farm_id: "farm-a".to_string(),
                farm_org_id: "org-a".to_string(),
                field_org_id: "org-b".to_string()
            }
        );
        assert!(registry.fields_for_org("org-b").is_empty());
    }

    #[test]
    fn field_boundary_validation_computes_extent_area_and_preserves_crs() {
        let boundary = test_boundary();

        let validated = validate_field_boundary(&boundary).expect("boundary validates");

        assert_eq!(validated.boundary, boundary);
        assert_eq!(validated.extent, test_extent());
        assert!(validated.area_ha > 0.0);
    }

    #[test]
    fn field_registry_rejects_unclosed_boundary_without_writing() {
        let mut registry = FarmFieldRegistry::default();
        registry
            .insert_farm(FarmRecord {
                farm_id: "farm-a".to_string(),
                org_id: "org-a".to_string(),
                owner: "org-a".to_string(),
                name: "Prairie Farm".to_string(),
                notes: None,
                created_at: "2026-04-01T00:00:00Z".to_string(),
            })
            .expect("farm persists");

        let error = registry
            .insert_field(FieldRecord {
                farm_id: Some("farm-a".to_string()),
                field_id: "field-a".to_string(),
                org_id: "org-a".to_string(),
                owner: "org-a".to_string(),
                name: "North 80".to_string(),
                area_ha: None,
                crop: None,
                season: None,
                notes: None,
                boundary: unclosed_test_boundary(),
                extent: test_extent(),
                created_at: "2026-04-01T00:00:00Z".to_string(),
            })
            .expect_err("unclosed boundary is rejected");

        assert_eq!(
            error,
            FarmFieldError::BoundaryInvalid {
                reason: FieldBoundaryValidationError::RingNotClosed
            }
        );
        assert!(registry.fields_for_org("org-a").is_empty());
    }

    #[test]
    fn field_boundary_validation_rejects_self_intersection() {
        let boundary = FieldBoundary {
            crs: Some("EPSG:4326".to_string()),
            coordinates: vec![
                GeoPoint {
                    longitude: -96.5,
                    latitude: 41.2,
                },
                GeoPoint {
                    longitude: -96.2,
                    latitude: 41.4,
                },
                GeoPoint {
                    longitude: -96.2,
                    latitude: 41.2,
                },
                GeoPoint {
                    longitude: -96.5,
                    latitude: 41.4,
                },
                GeoPoint {
                    longitude: -96.5,
                    latitude: 41.2,
                },
            ],
        };

        let error = validate_field_boundary(&boundary).expect_err("bowtie ring is rejected");

        assert_eq!(error, FieldBoundaryValidationError::SelfIntersection);
    }

    fn test_boundary() -> FieldBoundary {
        FieldBoundary {
            crs: Some("EPSG:4326".to_string()),
            coordinates: vec![
                GeoPoint {
                    longitude: -96.5,
                    latitude: 41.2,
                },
                GeoPoint {
                    longitude: -96.2,
                    latitude: 41.2,
                },
                GeoPoint {
                    longitude: -96.2,
                    latitude: 41.4,
                },
                GeoPoint {
                    longitude: -96.5,
                    latitude: 41.4,
                },
                GeoPoint {
                    longitude: -96.5,
                    latitude: 41.2,
                },
            ],
        }
    }

    fn unclosed_test_boundary() -> FieldBoundary {
        FieldBoundary {
            crs: Some("EPSG:4326".to_string()),
            coordinates: vec![
                GeoPoint {
                    longitude: -96.5,
                    latitude: 41.2,
                },
                GeoPoint {
                    longitude: -96.2,
                    latitude: 41.2,
                },
                GeoPoint {
                    longitude: -96.2,
                    latitude: 41.4,
                },
                GeoPoint {
                    longitude: -96.5,
                    latitude: 41.4,
                },
            ],
        }
    }

    fn test_extent() -> GeoBounds {
        GeoBounds {
            min_lon: -96.5,
            min_lat: 41.2,
            max_lon: -96.2,
            max_lat: 41.4,
        }
    }

    #[test]
    fn annotation_record_round_trips_through_json() {
        let annotation = AnnotationRecord {
            annotation_id: "ann-1".to_string(),
            scene_id: "scene-1".to_string(),
            field_id: Some("field-1".to_string()),
            author: Some("operator-1".to_string()),
            crs: Some("EPSG:4326".to_string()),
            audit_id: Some("audit-ann-1".to_string()),
            label: "Water stress".to_string(),
            note: Some("Observed near pivot edge".to_string()),
            severity: Some("high".to_string()),
            geometry: AnnotationGeometry::Point {
                coordinate: GeoPoint {
                    longitude: -96.4,
                    latitude: 41.2,
                },
            },
            created_at: "2026-04-01T00:00:00Z".to_string(),
            updated_at: "2026-04-01T00:00:00Z".to_string(),
        };

        let value = serde_json::to_value(&annotation).expect("annotation should serialize");
        let decoded: AnnotationRecord =
            serde_json::from_value(value).expect("annotation should deserialize");

        assert_eq!(decoded, annotation);
    }

    #[test]
    fn recommendation_record_round_trips_through_json() {
        let recommendation = RecommendationRecord {
            recommendation_id: "rec-1".to_string(),
            scene_id: "scene-1".to_string(),
            field_id: Some("field-1".to_string()),
            title: "Inspect water stress zone".to_string(),
            note: Some("Check irrigation and re-scout in 48h".to_string()),
            category: Some("irrigation".to_string()),
            priority: RecommendationPriority::High,
            status: RecommendationStatus::Reviewed,
            annotation_ids: vec!["ann-1".to_string(), "ann-2".to_string()],
            created_at: "2026-04-19T00:00:00Z".to_string(),
            updated_at: "2026-04-19T01:00:00Z".to_string(),
        };

        let value = serde_json::to_value(&recommendation).expect("recommendation should serialize");
        let decoded: RecommendationRecord =
            serde_json::from_value(value).expect("recommendation should deserialize");

        assert_eq!(decoded, recommendation);
    }

    #[test]
    fn report_record_round_trips_through_json() {
        let report = ReportRecord {
            report_id: "report-1".to_string(),
            scene_id: "scene-1".to_string(),
            field_id: Some("field-1".to_string()),
            title: "Scene 1 agronomy report".to_string(),
            format: ReportFormat::Html,
            artifact_path: "/tmp/report-1.html".to_string(),
            download_url: "/api/scenes/scene-1/reports/report-1".to_string(),
            annotation_count: 3,
            recommendation_count: 2,
            created_at: "2026-04-19T02:00:00Z".to_string(),
        };

        let value = serde_json::to_value(&report).expect("report should serialize");
        let decoded: ReportRecord =
            serde_json::from_value(value).expect("report should deserialize");

        assert_eq!(decoded, report);
    }
}
