use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const WGS84: &str = "EPSG:4326";
const WEB_MERCATOR: &str = "EPSG:3857";
const WEB_MERCATOR_RADIUS_METERS: f64 = 6_378_137.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    GeoJson,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteropRejectionReason {
    ParseError,
    MissingCrs,
    UnsupportedCrs { crs: String },
    UnsupportedGeometry { geometry_type: String },
    InvalidGeometry,
    EmptyFeatureCollection,
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
        validate_and_reproject_import, CrsTransform, ImportFormat, ImportPayload, InteropError,
        InteropRejectionReason, ReprojectedGeometry,
    };

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
}
