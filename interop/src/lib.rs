use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use shared::schemas::{
    assert_raster_spatial_ref, GeoBounds, RasterResolution, RasterSpatialRef, RasterSpatialRefError,
};

const WGS84: &str = "EPSG:4326";
const WEB_MERCATOR: &str = "EPSG:3857";
const WEB_MERCATOR_RADIUS_METERS: f64 = 6_378_137.0;
const GEOTIFF_METADATA_MAGIC: &[u8] = b"AGBOT-GEOTIFF-METADATA-V1\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    GeoJson,
    GeoPackage,
    GeoTiff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPayload {
    pub format: ImportFormat,
    pub filename: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrsTransform {
    pub target_crs: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InteropExtent {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InteropCoordinate {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ReprojectedGeometry {
    Point { coordinate: InteropCoordinate },
    LineString { coordinates: Vec<InteropCoordinate> },
    Polygon { rings: Vec<Vec<InteropCoordinate>> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReprojectedFeature {
    pub geometry: ReprojectedGeometry,
    pub properties: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteropImportResult {
    pub source_crs: String,
    pub target_crs: String,
    pub extent: InteropExtent,
    pub feature_count: usize,
    pub features: Vec<ReprojectedFeature>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorRoundTripReport {
    pub format: ImportFormat,
    pub source_crs: String,
    pub exported_crs: String,
    pub feature_count: usize,
    pub original_extent: InteropExtent,
    pub round_tripped_extent: InteropExtent,
    pub max_coordinate_drift: f64,
    pub exported_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeoPackageLayerReport {
    pub layer_name: String,
    pub crs: String,
    pub feature_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RasterProduct {
    pub product_id: String,
    pub width: u32,
    pub height: u32,
    pub spatial_ref: RasterSpatialRef,
    pub cells: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RasterGeoTiffReport {
    pub format: ImportFormat,
    pub product_id: String,
    pub crs: String,
    pub extent: GeoBounds,
    pub resolution: RasterResolution,
    pub transform: [f64; 6],
    pub width: u32,
    pub height: u32,
    pub exported_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct RasterGeoTiffEnvelope {
    format: ImportFormat,
    product: RasterProduct,
    crs: String,
    extent: GeoBounds,
    resolution: RasterResolution,
    transform: [f64; 6],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteropRejectionReason {
    ParseError,
    MissingCrs,
    UnsupportedCrs { crs: String },
    UnsupportedGeometry { geometry_type: String },
    InvalidGeometry,
    EmptyFeatureCollection,
    LayerMissingCrs { layer_name: String },
    MissingRasterTransform,
    InvalidRasterSpatialRef { reason: String },
    InvalidRasterCells,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum InteropError {
    #[error("import {filename} rejected: {reason:?}")]
    Rejected {
        filename: String,
        reason: InteropRejectionReason,
    },
}

pub fn validate_and_reproject_import(
    payload: ImportPayload,
    transform: CrsTransform,
) -> Result<InteropImportResult, InteropError> {
    let filename = normalized_filename(payload.filename);
    let target_crs = normalize_crs(&transform.target_crs).ok_or_else(|| {
        rejected(
            &filename,
            InteropRejectionReason::UnsupportedCrs {
                crs: transform.target_crs.clone(),
            },
        )
    })?;
    reject_unsupported_crs(&filename, &target_crs)?;

    match payload.format {
        ImportFormat::GeoJson => parse_geojson_and_reproject(&filename, &payload.bytes, target_crs),
        ImportFormat::GeoPackage | ImportFormat::GeoTiff => {
            Err(rejected(&filename, InteropRejectionReason::ParseError))
        }
    }
}

pub fn round_trip_vector_layer(
    payload: ImportPayload,
    transform: CrsTransform,
) -> Result<VectorRoundTripReport, InteropError> {
    let imported = validate_and_reproject_import(payload, transform)?;
    let exported_bytes = export_geojson(&imported)?;
    let round_tripped = validate_and_reproject_import(
        ImportPayload {
            format: ImportFormat::GeoJson,
            filename: "round-trip.geojson".to_string(),
            bytes: exported_bytes.clone(),
        },
        CrsTransform {
            target_crs: imported.target_crs.clone(),
        },
    )?;
    let max_coordinate_drift = extent_drift(imported.extent, round_tripped.extent);
    Ok(VectorRoundTripReport {
        format: ImportFormat::GeoJson,
        source_crs: imported.source_crs,
        exported_crs: imported.target_crs,
        feature_count: imported.feature_count,
        original_extent: imported.extent,
        round_tripped_extent: round_tripped.extent,
        max_coordinate_drift,
        exported_bytes,
    })
}

pub fn validate_geopackage_layers(
    payload: ImportPayload,
) -> Result<Vec<GeoPackageLayerReport>, InteropError> {
    let filename = normalized_filename(payload.filename);
    let document = serde_json::from_slice::<Value>(&payload.bytes)
        .map_err(|_| rejected(&filename, InteropRejectionReason::ParseError))?;
    let layers = document
        .get("layers")
        .and_then(Value::as_array)
        .ok_or_else(|| rejected(&filename, InteropRejectionReason::ParseError))?;
    let mut reports = Vec::with_capacity(layers.len());
    for layer in layers {
        let layer = layer
            .as_object()
            .ok_or_else(|| rejected(&filename, InteropRejectionReason::ParseError))?;
        let layer_name = layer
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or("<unnamed>")
            .to_string();
        let Some(crs) = layer
            .get("crs")
            .and_then(Value::as_str)
            .and_then(normalize_crs)
        else {
            return Err(rejected(
                &filename,
                InteropRejectionReason::LayerMissingCrs { layer_name },
            ));
        };
        reject_unsupported_crs(&filename, &crs)?;
        let feature_count = layer
            .get("feature_count")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0);
        reports.push(GeoPackageLayerReport {
            layer_name,
            crs,
            feature_count,
        });
    }
    Ok(reports)
}

pub fn export_raster_geotiff(product: RasterProduct) -> Result<RasterGeoTiffReport, InteropError> {
    let filename = normalized_filename(product.product_id.clone());
    let spatial_ref = validate_raster_product(&filename, &product)?;
    let crs = spatial_ref.crs.clone().ok_or_else(|| {
        rejected(
            &filename,
            InteropRejectionReason::InvalidRasterSpatialRef {
                reason: "asserted raster spatial ref missing CRS".to_string(),
            },
        )
    })?;
    let extent = spatial_ref.bbox.clone().ok_or_else(|| {
        rejected(
            &filename,
            InteropRejectionReason::InvalidRasterSpatialRef {
                reason: "asserted raster spatial ref missing extent".to_string(),
            },
        )
    })?;
    let resolution = spatial_ref.resolution.ok_or_else(|| {
        rejected(
            &filename,
            InteropRejectionReason::InvalidRasterSpatialRef {
                reason: "asserted raster spatial ref missing resolution".to_string(),
            },
        )
    })?;
    let transform = spatial_ref
        .geo_transform
        .ok_or_else(|| rejected(&filename, InteropRejectionReason::MissingRasterTransform))?;
    let product = RasterProduct {
        spatial_ref,
        ..product
    };
    let envelope = RasterGeoTiffEnvelope {
        format: ImportFormat::GeoTiff,
        product: product.clone(),
        crs: crs.clone(),
        extent: extent.clone(),
        resolution,
        transform,
    };
    let mut exported_bytes = GEOTIFF_METADATA_MAGIC.to_vec();
    exported_bytes.extend(
        serde_json::to_vec(&envelope)
            .map_err(|_| rejected(&filename, InteropRejectionReason::ParseError))?,
    );

    Ok(RasterGeoTiffReport {
        format: ImportFormat::GeoTiff,
        product_id: product.product_id,
        crs,
        extent,
        resolution,
        transform,
        width: product.width,
        height: product.height,
        exported_bytes,
    })
}

pub fn reopen_raster_geotiff(bytes: &[u8]) -> Result<RasterProduct, InteropError> {
    let payload = bytes
        .strip_prefix(GEOTIFF_METADATA_MAGIC)
        .ok_or_else(|| rejected("<geotiff>", InteropRejectionReason::ParseError))?;
    let envelope = serde_json::from_slice::<RasterGeoTiffEnvelope>(payload)
        .map_err(|_| rejected("<geotiff>", InteropRejectionReason::ParseError))?;
    if envelope.format != ImportFormat::GeoTiff {
        return Err(rejected("<geotiff>", InteropRejectionReason::ParseError));
    }
    let filename = normalized_filename(envelope.product.product_id.clone());
    let mut product = envelope.product;
    let spatial_ref = validate_raster_product(&filename, &product)?;
    product.spatial_ref = spatial_ref;
    Ok(product)
}

fn export_geojson(imported: &InteropImportResult) -> Result<Vec<u8>, InteropError> {
    let features = imported
        .features
        .iter()
        .map(|feature| {
            let mut feature_object = Map::new();
            feature_object.insert("type".to_string(), Value::String("Feature".to_string()));
            feature_object.insert(
                "properties".to_string(),
                Value::Object(feature.properties.clone()),
            );
            feature_object.insert(
                "geometry".to_string(),
                geometry_to_geojson(&feature.geometry),
            );
            Value::Object(feature_object)
        })
        .collect::<Vec<_>>();
    let mut crs_properties = Map::new();
    crs_properties.insert(
        "name".to_string(),
        Value::String(imported.target_crs.clone()),
    );
    let mut crs = Map::new();
    crs.insert("type".to_string(), Value::String("name".to_string()));
    crs.insert("properties".to_string(), Value::Object(crs_properties));
    let mut document = Map::new();
    document.insert(
        "type".to_string(),
        Value::String("FeatureCollection".to_string()),
    );
    document.insert("crs".to_string(), Value::Object(crs));
    document.insert("features".to_string(), Value::Array(features));
    serde_json::to_vec(&Value::Object(document))
        .map_err(|_| rejected("<export>", InteropRejectionReason::ParseError))
}

fn geometry_to_geojson(geometry: &ReprojectedGeometry) -> Value {
    let (geometry_type, coordinates) = match geometry {
        ReprojectedGeometry::Point { coordinate } => ("Point", coordinate_to_geojson(*coordinate)),
        ReprojectedGeometry::LineString { coordinates } => (
            "LineString",
            Value::Array(
                coordinates
                    .iter()
                    .map(|coordinate| coordinate_to_geojson(*coordinate))
                    .collect(),
            ),
        ),
        ReprojectedGeometry::Polygon { rings } => (
            "Polygon",
            Value::Array(
                rings
                    .iter()
                    .map(|ring| {
                        Value::Array(
                            ring.iter()
                                .map(|coordinate| coordinate_to_geojson(*coordinate))
                                .collect(),
                        )
                    })
                    .collect(),
            ),
        ),
    };
    let mut object = Map::new();
    object.insert("type".to_string(), Value::String(geometry_type.to_string()));
    object.insert("coordinates".to_string(), coordinates);
    Value::Object(object)
}

fn coordinate_to_geojson(coordinate: InteropCoordinate) -> Value {
    Value::Array(vec![Value::from(coordinate.x), Value::from(coordinate.y)])
}

fn extent_drift(left: InteropExtent, right: InteropExtent) -> f64 {
    [
        (left.min_x - right.min_x).abs(),
        (left.min_y - right.min_y).abs(),
        (left.max_x - right.max_x).abs(),
        (left.max_y - right.max_y).abs(),
    ]
    .into_iter()
    .fold(0.0, f64::max)
}

fn validate_raster_product(
    filename: &str,
    product: &RasterProduct,
) -> Result<RasterSpatialRef, InteropError> {
    let expected_cells = usize::try_from(product.width)
        .ok()
        .and_then(|width| {
            usize::try_from(product.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| rejected(filename, InteropRejectionReason::InvalidRasterCells))?;
    if product.cells.len() != expected_cells || product.cells.iter().any(|value| !value.is_finite())
    {
        return Err(rejected(
            filename,
            InteropRejectionReason::InvalidRasterCells,
        ));
    }
    assert_raster_spatial_ref(Some(&product.spatial_ref), product.width, product.height)
        .map_err(|error| raster_spatial_ref_rejection(filename, error))
}

fn raster_spatial_ref_rejection(filename: &str, error: RasterSpatialRefError) -> InteropError {
    match error {
        RasterSpatialRefError::MissingTransform => {
            rejected(filename, InteropRejectionReason::MissingRasterTransform)
        }
        other => rejected(
            filename,
            InteropRejectionReason::InvalidRasterSpatialRef {
                reason: other.to_string(),
            },
        ),
    }
}

fn parse_geojson_and_reproject(
    filename: &str,
    bytes: &[u8],
    target_crs: String,
) -> Result<InteropImportResult, InteropError> {
    let document = serde_json::from_slice::<Value>(bytes)
        .map_err(|_| rejected(filename, InteropRejectionReason::ParseError))?;
    let object = document
        .as_object()
        .ok_or_else(|| rejected(filename, InteropRejectionReason::ParseError))?;
    if object.get("type").and_then(Value::as_str) != Some("FeatureCollection") {
        return Err(rejected(filename, InteropRejectionReason::ParseError));
    }
    let source_crs = extract_geojson_crs(&document)
        .ok_or_else(|| rejected(filename, InteropRejectionReason::MissingCrs))?;
    reject_unsupported_crs(filename, &source_crs)?;
    let features = object
        .get("features")
        .and_then(Value::as_array)
        .ok_or_else(|| rejected(filename, InteropRejectionReason::ParseError))?;
    if features.is_empty() {
        return Err(rejected(
            filename,
            InteropRejectionReason::EmptyFeatureCollection,
        ));
    }

    let mut reprojected_features = Vec::with_capacity(features.len());
    let mut extent_builder = ExtentBuilder::default();
    for feature in features {
        let feature = feature
            .as_object()
            .ok_or_else(|| rejected(filename, InteropRejectionReason::ParseError))?;
        if feature.get("type").and_then(Value::as_str) != Some("Feature") {
            return Err(rejected(filename, InteropRejectionReason::ParseError));
        }
        let properties = feature
            .get("properties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let geometry_value = feature
            .get("geometry")
            .ok_or_else(|| rejected(filename, InteropRejectionReason::ParseError))?;
        let geometry = parse_reprojected_geometry(
            filename,
            geometry_value,
            &source_crs,
            &target_crs,
            &mut extent_builder,
        )?;
        reprojected_features.push(ReprojectedFeature {
            geometry,
            properties,
        });
    }

    let extent = extent_builder
        .finish()
        .ok_or_else(|| rejected(filename, InteropRejectionReason::InvalidGeometry))?;
    Ok(InteropImportResult {
        source_crs,
        target_crs,
        feature_count: reprojected_features.len(),
        extent,
        features: reprojected_features,
    })
}

fn parse_reprojected_geometry(
    filename: &str,
    geometry: &Value,
    source_crs: &str,
    target_crs: &str,
    extent_builder: &mut ExtentBuilder,
) -> Result<ReprojectedGeometry, InteropError> {
    let geometry = geometry
        .as_object()
        .ok_or_else(|| rejected(filename, InteropRejectionReason::ParseError))?;
    let geometry_type = geometry
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| rejected(filename, InteropRejectionReason::ParseError))?;
    let coordinates = geometry
        .get("coordinates")
        .ok_or_else(|| rejected(filename, InteropRejectionReason::ParseError))?;

    match geometry_type {
        "Point" => {
            let coordinate =
                reproject_json_coordinate(filename, coordinates, source_crs, target_crs)?;
            extent_builder.observe(coordinate);
            Ok(ReprojectedGeometry::Point { coordinate })
        }
        "LineString" => {
            let coordinates = coordinates
                .as_array()
                .ok_or_else(|| rejected(filename, InteropRejectionReason::ParseError))?
                .iter()
                .map(|coordinate| {
                    reproject_json_coordinate(filename, coordinate, source_crs, target_crs)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if coordinates.len() < 2 {
                return Err(rejected(filename, InteropRejectionReason::InvalidGeometry));
            }
            for coordinate in &coordinates {
                extent_builder.observe(*coordinate);
            }
            Ok(ReprojectedGeometry::LineString { coordinates })
        }
        "Polygon" => {
            let rings = coordinates
                .as_array()
                .ok_or_else(|| rejected(filename, InteropRejectionReason::ParseError))?
                .iter()
                .map(|ring| parse_polygon_ring(filename, ring, source_crs, target_crs))
                .collect::<Result<Vec<_>, _>>()?;
            if rings.is_empty() {
                return Err(rejected(filename, InteropRejectionReason::InvalidGeometry));
            }
            for coordinate in rings.iter().flatten() {
                extent_builder.observe(*coordinate);
            }
            Ok(ReprojectedGeometry::Polygon { rings })
        }
        other => Err(rejected(
            filename,
            InteropRejectionReason::UnsupportedGeometry {
                geometry_type: other.to_string(),
            },
        )),
    }
}

fn parse_polygon_ring(
    filename: &str,
    ring: &Value,
    source_crs: &str,
    target_crs: &str,
) -> Result<Vec<InteropCoordinate>, InteropError> {
    let coordinates = ring
        .as_array()
        .ok_or_else(|| rejected(filename, InteropRejectionReason::ParseError))?
        .iter()
        .map(|coordinate| reproject_json_coordinate(filename, coordinate, source_crs, target_crs))
        .collect::<Result<Vec<_>, _>>()?;
    if coordinates.len() < 4 || coordinates.first() != coordinates.last() {
        return Err(rejected(filename, InteropRejectionReason::InvalidGeometry));
    }
    Ok(coordinates)
}

fn reproject_json_coordinate(
    filename: &str,
    coordinate: &Value,
    source_crs: &str,
    target_crs: &str,
) -> Result<InteropCoordinate, InteropError> {
    let values = coordinate
        .as_array()
        .ok_or_else(|| rejected(filename, InteropRejectionReason::ParseError))?;
    let x = values
        .first()
        .and_then(Value::as_f64)
        .ok_or_else(|| rejected(filename, InteropRejectionReason::ParseError))?;
    let y = values
        .get(1)
        .and_then(Value::as_f64)
        .ok_or_else(|| rejected(filename, InteropRejectionReason::ParseError))?;
    if !x.is_finite() || !y.is_finite() {
        return Err(rejected(filename, InteropRejectionReason::InvalidGeometry));
    }
    reproject_coordinate(filename, InteropCoordinate { x, y }, source_crs, target_crs)
}

fn reproject_coordinate(
    filename: &str,
    coordinate: InteropCoordinate,
    source_crs: &str,
    target_crs: &str,
) -> Result<InteropCoordinate, InteropError> {
    if source_crs == target_crs {
        return Ok(coordinate);
    }
    match (source_crs, target_crs) {
        (WGS84, WEB_MERCATOR) => {
            let lon = coordinate.x;
            let lat = coordinate.y.clamp(-85.051_128_78, 85.051_128_78);
            let x = WEB_MERCATOR_RADIUS_METERS * lon.to_radians();
            let y = WEB_MERCATOR_RADIUS_METERS
                * (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0)
                    .tan()
                    .ln();
            Ok(InteropCoordinate { x, y })
        }
        (WEB_MERCATOR, WGS84) => {
            let lon = (coordinate.x / WEB_MERCATOR_RADIUS_METERS).to_degrees();
            let lat = (2.0 * (coordinate.y / WEB_MERCATOR_RADIUS_METERS).exp().atan()
                - std::f64::consts::FRAC_PI_2)
                .to_degrees();
            Ok(InteropCoordinate { x: lon, y: lat })
        }
        _ => Err(rejected(
            filename,
            InteropRejectionReason::UnsupportedCrs {
                crs: format!("{source_crs}->{target_crs}"),
            },
        )),
    }
}

fn extract_geojson_crs(document: &Value) -> Option<String> {
    document
        .get("crs")?
        .get("properties")?
        .get("name")?
        .as_str()
        .and_then(normalize_crs)
}

fn reject_unsupported_crs(filename: &str, crs: &str) -> Result<(), InteropError> {
    if crs.to_ascii_uppercase().contains("OBLIQUE") || !matches!(crs, WGS84 | WEB_MERCATOR) {
        return Err(rejected(
            filename,
            InteropRejectionReason::UnsupportedCrs {
                crs: crs.to_string(),
            },
        ));
    }
    Ok(())
}

fn normalize_crs(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_ascii_uppercase())
}

fn normalized_filename(filename: String) -> String {
    let trimmed = filename.trim();
    if trimmed.is_empty() {
        "<unnamed>".to_string()
    } else {
        trimmed.to_string()
    }
}

fn rejected(filename: &str, reason: InteropRejectionReason) -> InteropError {
    InteropError::Rejected {
        filename: filename.to_string(),
        reason,
    }
}

#[derive(Debug, Default)]
struct ExtentBuilder {
    min_x: Option<f64>,
    min_y: Option<f64>,
    max_x: Option<f64>,
    max_y: Option<f64>,
}

impl ExtentBuilder {
    fn observe(&mut self, coordinate: InteropCoordinate) {
        self.min_x = Some(
            self.min_x
                .map_or(coordinate.x, |value| value.min(coordinate.x)),
        );
        self.min_y = Some(
            self.min_y
                .map_or(coordinate.y, |value| value.min(coordinate.y)),
        );
        self.max_x = Some(
            self.max_x
                .map_or(coordinate.x, |value| value.max(coordinate.x)),
        );
        self.max_y = Some(
            self.max_y
                .map_or(coordinate.y, |value| value.max(coordinate.y)),
        );
    }

    fn finish(self) -> Option<InteropExtent> {
        Some(InteropExtent {
            min_x: self.min_x?,
            min_y: self.min_y?,
            max_x: self.max_x?,
            max_y: self.max_y?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        export_raster_geotiff, reopen_raster_geotiff, round_trip_vector_layer,
        validate_and_reproject_import, validate_geopackage_layers, CrsTransform, ImportFormat,
        ImportPayload, InteropError, InteropRejectionReason, RasterProduct, ReprojectedGeometry,
    };
    use shared::schemas::{GeoBounds, RasterResolution, RasterSpatialRef};

    #[test]
    fn validation_pipeline_reprojects_supported_geojson_and_reports_extent() {
        let result = validate_and_reproject_import(
            ImportPayload {
                format: ImportFormat::GeoJson,
                filename: "field-alpha.geojson".to_string(),
                bytes: valid_geojson("EPSG:4326").as_bytes().to_vec(),
            },
            CrsTransform {
                target_crs: "EPSG:3857".to_string(),
            },
        )
        .expect("valid GeoJSON should reproject");

        assert_eq!(result.source_crs, "EPSG:4326");
        assert_eq!(result.target_crs, "EPSG:3857");
        assert_eq!(result.feature_count, 1);
        assert!(result.extent.min_x < -13_460_000.0);
        assert!(result.extent.max_x < -13_460_000.0);
        assert!(result.extent.min_y > 4_600_000.0);
        assert!(result.extent.max_y > result.extent.min_y);
        assert_eq!(result.features[0].properties["zone"], "NE");
        match &result.features[0].geometry {
            ReprojectedGeometry::Polygon { rings } => {
                assert_eq!(rings.len(), 1);
                assert_eq!(rings[0].first(), rings[0].last());
            }
            other => panic!("expected polygon, got {other:?}"),
        }
    }

    #[test]
    fn validation_pipeline_rejects_oblique_or_unrecognized_crs() {
        let error = validate_and_reproject_import(
            ImportPayload {
                format: ImportFormat::GeoJson,
                filename: "oblique.geojson".to_string(),
                bytes: valid_geojson("OBLIQUE:LOCAL-GRID").as_bytes().to_vec(),
            },
            CrsTransform {
                target_crs: "EPSG:3857".to_string(),
            },
        )
        .expect_err("oblique CRS should be rejected");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "oblique.geojson".to_string(),
                reason: InteropRejectionReason::UnsupportedCrs {
                    crs: "OBLIQUE:LOCAL-GRID".to_string()
                }
            }
        );
    }

    #[test]
    fn validation_pipeline_rejects_malformed_input_without_partial_import() {
        let error = validate_and_reproject_import(
            ImportPayload {
                format: ImportFormat::GeoJson,
                filename: "broken.geojson".to_string(),
                bytes: br#"{"type":"FeatureCollection","features":["#.to_vec(),
            },
            CrsTransform {
                target_crs: "EPSG:3857".to_string(),
            },
        )
        .expect_err("malformed GeoJSON should be rejected");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "broken.geojson".to_string(),
                reason: InteropRejectionReason::ParseError
            }
        );
    }

    #[test]
    fn vector_round_trip_geojson_preserves_crs_extent_and_geometry() {
        let report = round_trip_vector_layer(
            ImportPayload {
                format: ImportFormat::GeoJson,
                filename: "field-alpha.geojson".to_string(),
                bytes: valid_geojson("EPSG:4326").as_bytes().to_vec(),
            },
            CrsTransform {
                target_crs: "EPSG:3857".to_string(),
            },
        )
        .expect("GeoJSON should round-trip");

        assert_eq!(report.format, ImportFormat::GeoJson);
        assert_eq!(report.source_crs, "EPSG:4326");
        assert_eq!(report.exported_crs, "EPSG:3857");
        assert_eq!(report.feature_count, 1);
        assert!(report.max_coordinate_drift <= 0.000001);
        assert!(std::str::from_utf8(&report.exported_bytes)
            .expect("export should be utf8")
            .contains("\"EPSG:3857\""));
    }

    #[test]
    fn geopackage_layer_without_declared_crs_is_flagged_not_assumed() {
        let error = validate_geopackage_layers(ImportPayload {
            format: ImportFormat::GeoPackage,
            filename: "multi-layer.gpkg.json".to_string(),
            bytes: br#"{
                "layers": [
                    { "name": "field-alpha", "crs": "EPSG:4326", "feature_count": 1 },
                    { "name": "legacy-layer", "feature_count": 2 }
                ]
            }"#
            .to_vec(),
        })
        .expect_err("undeclared CRS layer should be rejected");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "multi-layer.gpkg.json".to_string(),
                reason: InteropRejectionReason::LayerMissingCrs {
                    layer_name: "legacy-layer".to_string()
                }
            }
        );
    }

    #[test]
    fn geotiff_export_reopens_with_source_spatial_metadata() {
        let product = raster_product();

        let report = export_raster_geotiff(product.clone()).expect("GeoTIFF should export");

        assert_eq!(report.format, ImportFormat::GeoTiff);
        assert_eq!(report.product_id, "ndvi-alpha");
        assert_eq!(report.crs, "EPSG:32610");
        assert_eq!(report.extent, product.spatial_ref.bbox.clone().unwrap());
        assert_eq!(report.resolution, RasterResolution { x: 10.0, y: 10.0 });
        assert_eq!(
            report.transform,
            [500_000.0, 10.0, 0.0, 4_100_000.0, 0.0, -10.0]
        );
        assert!(report
            .exported_bytes
            .starts_with(b"AGBOT-GEOTIFF-METADATA-V1\n"));

        let reopened =
            reopen_raster_geotiff(&report.exported_bytes).expect("GeoTIFF should re-open");
        assert_eq!(reopened.product_id, product.product_id);
        assert_eq!(reopened.width, product.width);
        assert_eq!(reopened.height, product.height);
        assert_eq!(reopened.spatial_ref, product.spatial_ref);
        assert_eq!(reopened.cells, product.cells);
    }

    #[test]
    fn geotiff_export_rejects_missing_source_transform() {
        let mut product = raster_product();
        product.spatial_ref.geo_transform = None;

        let error = export_raster_geotiff(product)
            .expect_err("raster without transform should not export as GeoTIFF");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "ndvi-alpha".to_string(),
                reason: InteropRejectionReason::MissingRasterTransform
            }
        );
    }

    fn valid_geojson(crs: &str) -> String {
        format!(
            r#"{{
                "type": "FeatureCollection",
                "crs": {{
                    "type": "name",
                    "properties": {{ "name": "{crs}" }}
                }},
                "features": [{{
                    "type": "Feature",
                    "properties": {{ "zone": "NE" }},
                    "geometry": {{
                        "type": "Polygon",
                        "coordinates": [[
                            [-121.0000, 39.0000],
                            [-120.9900, 39.0000],
                            [-120.9900, 39.0100],
                            [-121.0000, 39.0100],
                            [-121.0000, 39.0000]
                        ]]
                    }}
                }}]
            }}"#
        )
    }

    fn raster_product() -> RasterProduct {
        RasterProduct {
            product_id: "ndvi-alpha".to_string(),
            width: 2,
            height: 2,
            spatial_ref: RasterSpatialRef {
                georeferenced: true,
                crs: Some("EPSG:32610".to_string()),
                bbox: Some(GeoBounds {
                    min_lon: 500_000.0,
                    min_lat: 4_099_980.0,
                    max_lon: 500_020.0,
                    max_lat: 4_100_000.0,
                }),
                geo_transform: Some([500_000.0, 10.0, 0.0, 4_100_000.0, 0.0, -10.0]),
                resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
            },
            cells: vec![0.12, 0.28, 0.42, 0.51],
        }
    }
}
