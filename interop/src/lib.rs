use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use shared::schemas::{
    assert_raster_spatial_ref, validate_field_boundary, FarmFieldEntityStatus, FieldBoundary,
    FieldBoundaryValidationError, FieldRecord, GeoBounds, GeoPoint, RasterResolution,
    RasterSpatialRef, RasterSpatialRefError,
};
use std::collections::BTreeSet;

const WGS84: &str = "EPSG:4326";
const WEB_MERCATOR: &str = "EPSG:3857";
const WEB_MERCATOR_RADIUS_METERS: f64 = 6_378_137.0;
const GEOTIFF_METADATA_MAGIC: &[u8] = b"AGBOT-GEOTIFF-METADATA-V1\n";
const SHAPEFILE_HEADER_BYTES: usize = 100;
const ESRI_FILE_CODE: i32 = 9994;
const SHAPEFILE_VERSION: i32 = 1000;
const SHAPE_TYPE_POLYGON: i32 = 5;
const PRESCRIPTION_RATE_ATTRIBUTE: &str = "RATE";
const PRESCRIPTION_UNIT_ATTRIBUTE: &str = "UNIT";
const PRESCRIPTION_RATE_FIELD_WIDTH: u8 = 18;
const PRESCRIPTION_RATE_DECIMALS: u8 = 6;
const GEOMETRY_EPSILON: f64 = 1e-9;
const TASKDATA_FILENAME: &str = "TASKDATA.XML";

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldBoundaryImportRequest {
    pub payload: ImportPayload,
    pub target_crs: String,
    pub field_id: String,
    pub farm_id: Option<String>,
    pub org_id: String,
    pub owner: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldBoundaryImportReport {
    pub field: FieldRecord,
    pub source_filename: String,
    pub source_crs: String,
    pub target_crs: String,
    pub feature_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrescriptionField {
    pub field_id: String,
    pub crs: String,
    pub boundary: Vec<InteropCoordinate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrescriptionZone {
    pub zone_id: String,
    pub polygon: Vec<InteropCoordinate>,
    pub crs: String,
    pub rate: f64,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrescriptionShapefileRequest {
    pub prescription_id: String,
    pub field: PrescriptionField,
    pub zones: Vec<PrescriptionZone>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrescriptionShapefileFiles {
    pub shp: Vec<u8>,
    pub shx: Vec<u8>,
    pub dbf: Vec<u8>,
    pub prj: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingsShapefileFiles {
    pub shp: Vec<u8>,
    pub shx: Vec<u8>,
    pub dbf: Vec<u8>,
    pub prj: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingsExportFeature {
    pub finding_id: String,
    pub zone_id: String,
    pub reason: String,
    pub priority: String,
    pub area_m2: f64,
    pub centroid: InteropCoordinate,
    pub crs: String,
    pub polygon: Vec<InteropCoordinate>,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingsExportRequest {
    pub export_id: String,
    pub crs: String,
    pub findings: Vec<FindingsExportFeature>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingsGeoJsonExport {
    pub export_id: String,
    pub crs: String,
    pub feature_count: usize,
    pub exported_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingsShapefileExport {
    pub export_id: String,
    pub crs: String,
    pub feature_count: usize,
    pub extent: Option<InteropExtent>,
    pub files: FindingsShapefileFiles,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrescriptionShapefileReport {
    pub prescription_id: String,
    pub field_id: String,
    pub field_crs: String,
    pub extent: InteropExtent,
    pub zone_count: usize,
    pub rate_attribute: String,
    pub unit_attribute: String,
    pub files: PrescriptionShapefileFiles,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDataValidation {
    pub valid: bool,
    pub task_count: usize,
    pub zone_count: usize,
    pub product_count: usize,
    pub prescription_grid_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrescriptionTaskDataReport {
    pub prescription_id: String,
    pub field_id: String,
    pub field_crs: String,
    pub zone_count: usize,
    pub unit_designator: String,
    pub taskdata_xml: Vec<u8>,
    pub validation: TaskDataValidation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JohnDeerePrescriptionPushRequest {
    pub remote_field_id: String,
    pub prescription: PrescriptionShapefileRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JohnDeereRetryPolicy {
    pub max_attempts: usize,
    #[serde(default)]
    pub backoff_millis: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JohnDeereUploadPayload {
    pub remote_field_id: String,
    pub prescription_id: String,
    pub crs: String,
    pub unit_designator: String,
    pub zone_count: usize,
    pub rates: Vec<f64>,
    pub taskdata_xml: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemotePrescriptionReceipt {
    pub remote_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JohnDeereEndpointError {
    pub message: String,
}

impl JohnDeereEndpointError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait JohnDeereConnectorEndpoint {
    fn push_prescription(
        &mut self,
        payload: JohnDeereUploadPayload,
    ) -> Result<RemotePrescriptionReceipt, JohnDeereEndpointError>;

    fn pull_boundaries(&mut self) -> Result<Vec<JohnDeereBoundary>, JohnDeereEndpointError>;

    fn wait_backoff(&mut self, _millis: u64) {}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JohnDeerePrescriptionPushReport {
    pub remote_id: String,
    pub attempts: usize,
    pub backoff_millis: Vec<u64>,
    pub zone_count: usize,
    pub unit_designator: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JohnDeereBoundary {
    pub remote_field_id: String,
    pub name: String,
    pub crs: String,
    pub boundary: Vec<InteropCoordinate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JohnDeereMappedBoundary {
    pub remote_field_id: String,
    pub name: String,
    pub source_crs: String,
    pub target_crs: String,
    pub extent: InteropExtent,
    pub feature_count: usize,
    pub boundary: Vec<InteropCoordinate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JohnDeereBoundaryPullReport {
    pub boundaries: Vec<JohnDeereMappedBoundary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct NormalizedPrescription {
    prescription_id: String,
    field_id: String,
    field_crs: String,
    field_extent: InteropExtent,
    zones: Vec<NormalizedPrescriptionZone>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct NormalizedPrescriptionZone {
    zone_id: String,
    polygon: Vec<InteropCoordinate>,
    extent: InteropExtent,
    area: f64,
    rate: f64,
    unit: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldBoundaryRejectionReason {
    MissingCrs,
    TooFewCoordinates,
    InvalidCoordinate,
    RingNotClosed,
    SelfIntersection,
    EmptyArea,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrescriptionRejectionReason {
    EmptyPrescriptionId,
    EmptyFieldId,
    EmptyCrs,
    EmptyZoneSet,
    EmptyZoneId,
    EmptyUnit {
        zone_id: String,
    },
    InvalidRate {
        zone_id: String,
    },
    InvalidFieldGeometry,
    InvalidZoneGeometry {
        zone_id: String,
    },
    CrsMismatch {
        zone_id: String,
        expected_crs: String,
        actual_crs: String,
    },
    ZoneOutsideField {
        zone_id: String,
    },
    OverlappingZones {
        left_zone_id: String,
        right_zone_id: String,
    },
    ZoneCoverageGap,
    UnsupportedTaskDataUnit {
        zone_id: String,
        unit: String,
    },
    MixedTaskDataUnits {
        zone_id: String,
        expected_unit: String,
        actual_unit: String,
    },
    InvalidTaskDataSchema {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JohnDeereConnectorError {
    EmptyRemoteFieldId,
    EmptyRemoteId,
    EndpointFailed {
        attempts: usize,
        message: String,
    },
    UnsupportedPrescriptionCrs {
        crs: String,
    },
    UnsupportedBoundaryCrs {
        remote_field_id: String,
        crs: String,
    },
    InvalidBoundaryGeometry {
        remote_field_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteropRejectionReason {
    ParseError,
    MissingCrs,
    UnsupportedCrs {
        crs: String,
    },
    UnsupportedGeometry {
        geometry_type: String,
    },
    InvalidGeometry,
    EmptyFeatureCollection,
    LayerMissingCrs {
        layer_name: String,
    },
    MissingRasterTransform,
    InvalidRasterSpatialRef {
        reason: String,
    },
    InvalidRasterCells,
    InvalidFieldBoundary {
        reason: FieldBoundaryRejectionReason,
    },
    InvalidPrescription {
        reason: PrescriptionRejectionReason,
    },
    JohnDeereConnector {
        reason: JohnDeereConnectorError,
    },
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

pub fn export_findings_geojson(
    request: FindingsExportRequest,
) -> Result<FindingsGeoJsonExport, InteropError> {
    let normalized = normalize_findings_export_request(request)?;
    let features = normalized
        .findings
        .iter()
        .map(finding_geojson_feature)
        .collect::<Vec<_>>();
    let mut crs_properties = Map::new();
    crs_properties.insert("name".to_string(), Value::String(normalized.crs.clone()));
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
    let exported_bytes = serde_json::to_vec(&Value::Object(document))
        .map_err(|_| rejected(&normalized.export_id, InteropRejectionReason::ParseError))?;
    Ok(FindingsGeoJsonExport {
        export_id: normalized.export_id,
        crs: normalized.crs,
        feature_count: normalized.findings.len(),
        exported_bytes,
    })
}

pub fn export_findings_shapefile(
    request: FindingsExportRequest,
) -> Result<FindingsShapefileExport, InteropError> {
    let normalized = normalize_findings_export_request(request)?;
    let extent = findings_extent(&normalized.findings);
    let shape_extent = extent.unwrap_or(InteropExtent {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 0.0,
        max_y: 0.0,
    });
    let record_contents = normalized
        .findings
        .iter()
        .map(|finding| {
            let extent = extent_from_coordinates(&finding.polygon).expect("finding is normalized");
            polygon_shapefile_record_content(&finding.polygon, extent)
        })
        .collect::<Vec<_>>();
    let files = FindingsShapefileFiles {
        shp: write_shp_bytes(shape_extent, &record_contents),
        shx: write_shx_bytes(shape_extent, &record_contents),
        dbf: write_findings_dbf(&normalized.findings),
        prj: projection_wkt(&normalized.crs).into_bytes(),
    };
    Ok(FindingsShapefileExport {
        export_id: normalized.export_id,
        crs: normalized.crs,
        feature_count: normalized.findings.len(),
        extent,
        files,
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

pub fn import_field_boundary(
    request: FieldBoundaryImportRequest,
) -> Result<FieldBoundaryImportReport, InteropError> {
    let source_filename = normalized_filename(request.payload.filename.clone());
    let imported = validate_and_reproject_import(
        request.payload,
        CrsTransform {
            target_crs: request.target_crs,
        },
    )?;
    let first_feature = imported
        .features
        .first()
        .ok_or_else(|| rejected(&source_filename, InteropRejectionReason::InvalidGeometry))?;
    let ReprojectedGeometry::Polygon { rings } = &first_feature.geometry else {
        return Err(rejected(
            &source_filename,
            InteropRejectionReason::UnsupportedGeometry {
                geometry_type: reprojected_geometry_type(&first_feature.geometry).to_string(),
            },
        ));
    };
    let exterior = rings
        .first()
        .ok_or_else(|| rejected(&source_filename, InteropRejectionReason::InvalidGeometry))?;
    let boundary = FieldBoundary {
        coordinates: exterior
            .iter()
            .map(|coordinate| GeoPoint {
                longitude: coordinate.x,
                latitude: coordinate.y,
            })
            .collect(),
        crs: Some(imported.target_crs.clone()),
    };
    let validated = validate_field_boundary(&boundary).map_err(|reason| {
        rejected(
            &source_filename,
            InteropRejectionReason::InvalidFieldBoundary {
                reason: reason.into(),
            },
        )
    })?;
    let field = FieldRecord {
        farm_id: request.farm_id,
        field_id: request.field_id,
        org_id: request.org_id,
        owner: request.owner,
        name: request.name,
        area_ha: Some(validated.area_ha),
        crop: None,
        season: None,
        notes: Some(format!(
            "imported from {source_filename}; source_crs={}; target_crs={}",
            imported.source_crs, imported.target_crs
        )),
        boundary: validated.boundary,
        extent: validated.extent,
        status: FarmFieldEntityStatus::Active,
        created_at: request.created_at.clone(),
        updated_at: request.created_at,
    };

    Ok(FieldBoundaryImportReport {
        field,
        source_filename,
        source_crs: imported.source_crs,
        target_crs: imported.target_crs,
        feature_count: imported.feature_count,
    })
}

pub fn export_prescription_shapefile(
    request: PrescriptionShapefileRequest,
) -> Result<PrescriptionShapefileReport, InteropError> {
    let prescription = normalize_prescription_request(request)?;
    let files = write_prescription_shapefile_bundle(
        &prescription.field_crs,
        prescription.field_extent,
        &prescription.zones,
    );
    Ok(PrescriptionShapefileReport {
        prescription_id: prescription.prescription_id,
        field_id: prescription.field_id,
        field_crs: prescription.field_crs,
        extent: prescription.field_extent,
        zone_count: prescription.zones.len(),
        rate_attribute: PRESCRIPTION_RATE_ATTRIBUTE.to_string(),
        unit_attribute: PRESCRIPTION_UNIT_ATTRIBUTE.to_string(),
        files,
    })
}

pub fn export_prescription_taskdata(
    request: PrescriptionShapefileRequest,
) -> Result<PrescriptionTaskDataReport, InteropError> {
    let prescription = normalize_prescription_request(request)?;
    let unit_designator = taskdata_unit_designator(&prescription)?;
    let taskdata_xml = write_taskdata_xml(&prescription, &unit_designator).into_bytes();
    let validation = validate_taskdata_xml(&taskdata_xml)?;
    Ok(PrescriptionTaskDataReport {
        prescription_id: prescription.prescription_id,
        field_id: prescription.field_id,
        field_crs: prescription.field_crs,
        zone_count: prescription.zones.len(),
        unit_designator,
        taskdata_xml,
        validation,
    })
}

pub fn validate_taskdata_xml(bytes: &[u8]) -> Result<TaskDataValidation, InteropError> {
    let xml = std::str::from_utf8(bytes)
        .map_err(|_| taskdata_schema_rejected("TaskData XML is not valid UTF-8".to_string()))?;
    let root_start = xml
        .find("<ISO11783_TaskData")
        .ok_or_else(|| taskdata_schema_rejected("missing ISO11783_TaskData root".to_string()))?;
    if xml[..root_start].contains('<') && !xml[..root_start].trim_start().starts_with("<?xml") {
        return Err(taskdata_schema_rejected(
            "missing ISO11783_TaskData root".to_string(),
        ));
    }
    let root_end = xml
        .rfind("</ISO11783_TaskData>")
        .ok_or_else(|| taskdata_schema_rejected("missing ISO11783_TaskData close".to_string()))?;
    if root_end <= root_start {
        return Err(taskdata_schema_rejected(
            "malformed ISO11783_TaskData root".to_string(),
        ));
    }
    if xml[root_end + "</ISO11783_TaskData>".len()..]
        .trim()
        .contains('<')
    {
        return Err(taskdata_schema_rejected(
            "unexpected XML outside ISO11783_TaskData root".to_string(),
        ));
    }
    let root_tag = next_xml_tag(&xml[root_start..])
        .ok_or_else(|| taskdata_schema_rejected("malformed ISO11783_TaskData root".to_string()))?;
    if root_tag.name != "ISO11783_TaskData" || root_tag.closing || root_tag.self_closing {
        return Err(taskdata_schema_rejected(
            "missing ISO11783_TaskData root".to_string(),
        ));
    }
    let body_start = root_start + root_tag.end;
    let body = &xml[body_start..root_end];
    let root_attrs = parse_xml_attributes(root_tag.raw)?;
    required_attr(&root_attrs, "VersionMajor")?;
    required_attr(&root_attrs, "VersionMinor")?;

    let task_tags = collect_xml_tags(body, "TSK")?;
    if task_tags.len() != 1 {
        return Err(taskdata_schema_rejected("missing TSK task".to_string()));
    }
    let product_tags = collect_xml_tags(body, "PDT")?;
    if product_tags.len() != 1 {
        return Err(taskdata_schema_rejected("missing PDT product".to_string()));
    }
    let pgp_tags = collect_xml_tags(body, "PGP")?;
    if pgp_tags.len() != 1 {
        return Err(taskdata_schema_rejected(
            "missing PGP prescription grid".to_string(),
        ));
    }
    let field_tags = collect_xml_tags(body, "PFD")?;
    if field_tags.len() != 1 {
        return Err(taskdata_schema_rejected("missing PFD field".to_string()));
    }
    let zone_tags = collect_xml_tags(body, "TZN")?;
    if zone_tags.is_empty() {
        return Err(taskdata_schema_rejected("missing TZN zones".to_string()));
    }

    let product_attrs = parse_xml_attributes(product_tags[0].raw)?;
    let product_id = required_attr(&product_attrs, "A")?;
    let product_unit = required_attr(&product_attrs, "C")?;
    let pgp_attrs = parse_xml_attributes(pgp_tags[0].raw)?;
    if required_attr(&pgp_attrs, "B")? != product_id {
        return Err(taskdata_schema_rejected(
            "PGP product reference does not resolve".to_string(),
        ));
    }
    let pgp_unit = required_attr(&pgp_attrs, "C")?;
    if pgp_unit != product_unit {
        return Err(taskdata_schema_rejected(
            "inconsistent TaskData units".to_string(),
        ));
    }

    let field_attrs = parse_xml_attributes(field_tags[0].raw)?;
    let field_crs = required_attr(&field_attrs, "Crs")?;
    let task_attrs = parse_xml_attributes(task_tags[0].raw)?;
    let task_crs = required_attr(&task_attrs, "Crs")?;
    if task_crs != field_crs {
        return Err(taskdata_schema_rejected(
            "inconsistent CRS declarations".to_string(),
        ));
    }
    required_attr(&task_attrs, "A")?;
    required_attr(&task_attrs, "B")?;

    let mut zone_ids = BTreeSet::new();
    let mut zone_count = 0usize;
    for tag in &zone_tags {
        if tag.self_closing {
            return Err(taskdata_schema_rejected(
                "TZN zone must contain polygon points".to_string(),
            ));
        }
        let attrs = parse_xml_attributes(tag.raw)?;
        let zone_id = required_attr(&attrs, "A")?;
        if !zone_ids.insert(zone_id.to_string()) {
            return Err(taskdata_schema_rejected(
                "duplicate TZN zone id".to_string(),
            ));
        }
        required_attr(&attrs, "B")?;
        let rate = required_attr(&attrs, "C")?;
        if rate
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .is_none()
        {
            return Err(taskdata_schema_rejected("invalid TZN rate".to_string()));
        }
        let zone_unit = required_attr(&attrs, "D")?;
        if zone_unit != product_unit {
            return Err(taskdata_schema_rejected(
                "inconsistent TaskData units".to_string(),
            ));
        }
        let zone_crs = required_attr(&attrs, "Crs")?;
        if zone_crs != field_crs {
            return Err(taskdata_schema_rejected(
                "inconsistent CRS declarations".to_string(),
            ));
        }
        let zone_body = tag_body(body, tag, "TZN")?;
        let polygon_tags = collect_xml_tags(zone_body, "PLN")?;
        if polygon_tags.len() != 1 {
            return Err(taskdata_schema_rejected(
                "TZN zone must contain one PLN polygon".to_string(),
            ));
        }
        let point_tags = collect_xml_tags(zone_body, "PNT")?;
        if point_tags.len() < 4 {
            return Err(taskdata_schema_rejected(
                "TZN polygon has too few PNT points".to_string(),
            ));
        }
        for point_tag in point_tags {
            let point_attrs = parse_xml_attributes(point_tag.raw)?;
            required_attr(&point_attrs, "A")?;
            let x = required_attr(&point_attrs, "B")?;
            let y = required_attr(&point_attrs, "C")?;
            if x.parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .is_none()
                || y.parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .is_none()
            {
                return Err(taskdata_schema_rejected(
                    "invalid PNT coordinate".to_string(),
                ));
            }
        }
        zone_count += 1;
    }
    Ok(TaskDataValidation {
        valid: true,
        task_count: task_tags.len(),
        zone_count,
        product_count: product_tags.len(),
        prescription_grid_count: pgp_tags.len(),
    })
}

pub fn push_john_deere_prescription(
    endpoint: &mut impl JohnDeereConnectorEndpoint,
    request: JohnDeerePrescriptionPushRequest,
    retry_policy: JohnDeereRetryPolicy,
) -> Result<JohnDeerePrescriptionPushReport, InteropError> {
    let remote_field_id = normalize_prescription_text(&request.remote_field_id)
        .ok_or_else(|| john_deere_rejected(JohnDeereConnectorError::EmptyRemoteFieldId))?;
    let prescription = normalize_prescription_request(request.prescription)?;
    validate_john_deere_prescription_crs(&prescription.field_crs)?;
    let unit_designator = taskdata_unit_designator(&prescription)?;
    let taskdata_xml = write_taskdata_xml(&prescription, &unit_designator).into_bytes();
    validate_taskdata_xml(&taskdata_xml)?;
    let rates = prescription
        .zones
        .iter()
        .map(|zone| zone.rate)
        .collect::<Vec<_>>();
    let payload = JohnDeereUploadPayload {
        remote_field_id,
        prescription_id: prescription.prescription_id,
        crs: prescription.field_crs,
        unit_designator: unit_designator.clone(),
        zone_count: prescription.zones.len(),
        rates,
        taskdata_xml,
    };
    let max_attempts = retry_policy.max_attempts.max(1);
    let mut backoff_millis = Vec::new();
    let mut last_error = None::<JohnDeereEndpointError>;

    for attempt in 1..=max_attempts {
        match endpoint.push_prescription(payload.clone()) {
            Ok(receipt) => {
                let remote_id = normalize_prescription_text(&receipt.remote_id)
                    .ok_or_else(|| john_deere_rejected(JohnDeereConnectorError::EmptyRemoteId))?;
                return Ok(JohnDeerePrescriptionPushReport {
                    remote_id,
                    attempts: attempt,
                    backoff_millis,
                    zone_count: payload.zone_count,
                    unit_designator,
                });
            }
            Err(error) => {
                last_error = Some(error);
                if attempt < max_attempts {
                    let backoff = backoff_for_attempt(&retry_policy, attempt);
                    endpoint.wait_backoff(backoff);
                    backoff_millis.push(backoff);
                }
            }
        }
    }

    let message = last_error
        .map(|error| error.message)
        .unwrap_or_else(|| "endpoint failed".to_string());
    Err(john_deere_rejected(
        JohnDeereConnectorError::EndpointFailed {
            attempts: max_attempts,
            message,
        },
    ))
}

pub fn pull_john_deere_boundaries(
    endpoint: &mut impl JohnDeereConnectorEndpoint,
) -> Result<JohnDeereBoundaryPullReport, InteropError> {
    let boundaries = endpoint.pull_boundaries().map_err(|error| {
        john_deere_rejected(JohnDeereConnectorError::EndpointFailed {
            attempts: 1,
            message: error.message,
        })
    })?;
    let mut mapped = Vec::with_capacity(boundaries.len());
    for boundary in boundaries {
        mapped.push(map_john_deere_boundary(boundary)?);
    }
    Ok(JohnDeereBoundaryPullReport { boundaries: mapped })
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

fn finding_geojson_feature(finding: &FindingsExportFeature) -> Value {
    let mut properties = Map::new();
    properties.insert(
        "finding_id".to_string(),
        Value::String(finding.finding_id.clone()),
    );
    properties.insert(
        "zone_id".to_string(),
        Value::String(finding.zone_id.clone()),
    );
    properties.insert("reason".to_string(), Value::String(finding.reason.clone()));
    properties.insert(
        "priority".to_string(),
        Value::String(finding.priority.clone()),
    );
    properties.insert("area_m2".to_string(), Value::from(finding.area_m2));
    properties.insert("centroid_x".to_string(), Value::from(finding.centroid.x));
    properties.insert("centroid_y".to_string(), Value::from(finding.centroid.y));
    properties.insert("crs".to_string(), Value::String(finding.crs.clone()));
    properties.insert(
        "evidence_refs".to_string(),
        Value::Array(
            finding
                .evidence_refs
                .iter()
                .map(|value| Value::String(value.clone()))
                .collect(),
        ),
    );
    let mut geometry = Map::new();
    geometry.insert("type".to_string(), Value::String("Polygon".to_string()));
    geometry.insert(
        "coordinates".to_string(),
        Value::Array(vec![Value::Array(
            finding
                .polygon
                .iter()
                .map(|coordinate| coordinate_to_geojson(*coordinate))
                .collect(),
        )]),
    );
    let mut feature = Map::new();
    feature.insert("type".to_string(), Value::String("Feature".to_string()));
    feature.insert("id".to_string(), Value::String(finding.finding_id.clone()));
    feature.insert("geometry".to_string(), Value::Object(geometry));
    feature.insert("properties".to_string(), Value::Object(properties));
    Value::Object(feature)
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

fn normalize_findings_export_request(
    mut request: FindingsExportRequest,
) -> Result<FindingsExportRequest, InteropError> {
    let export_id = normalized_filename(request.export_id.clone());
    request.export_id = export_id.clone();
    request.crs = normalize_crs(&request.crs)
        .ok_or_else(|| rejected(&export_id, InteropRejectionReason::MissingCrs))?;
    reject_unsupported_crs(&export_id, &request.crs)?;
    for finding in &mut request.findings {
        finding.finding_id = normalize_optional_text(&finding.finding_id)
            .ok_or_else(|| rejected(&export_id, InteropRejectionReason::InvalidGeometry))?;
        finding.zone_id = normalize_optional_text(&finding.zone_id)
            .ok_or_else(|| rejected(&export_id, InteropRejectionReason::InvalidGeometry))?;
        finding.reason = normalize_optional_text(&finding.reason)
            .ok_or_else(|| rejected(&export_id, InteropRejectionReason::InvalidGeometry))?;
        finding.priority = normalize_optional_text(&finding.priority)
            .ok_or_else(|| rejected(&export_id, InteropRejectionReason::InvalidGeometry))?;
        finding.crs = normalize_crs(&finding.crs)
            .ok_or_else(|| rejected(&export_id, InteropRejectionReason::MissingCrs))?;
        if finding.crs != request.crs {
            return Err(rejected(
                &export_id,
                InteropRejectionReason::UnsupportedCrs {
                    crs: finding.crs.clone(),
                },
            ));
        }
        if !finding.area_m2.is_finite()
            || finding.area_m2 < 0.0
            || !finding.centroid.x.is_finite()
            || !finding.centroid.y.is_finite()
        {
            return Err(rejected(
                &export_id,
                InteropRejectionReason::InvalidGeometry,
            ));
        }
        finding.evidence_refs = finding
            .evidence_refs
            .iter()
            .filter_map(|value| normalize_optional_text(value))
            .collect();
        if finding.polygon.len() < 4
            || finding
                .polygon
                .iter()
                .any(|coordinate| !coordinate.x.is_finite() || !coordinate.y.is_finite())
            || finding.polygon.first() != finding.polygon.last()
            || extent_from_coordinates(&finding.polygon).is_none()
        {
            return Err(rejected(
                &export_id,
                InteropRejectionReason::InvalidGeometry,
            ));
        }
    }
    Ok(request)
}

fn findings_extent(findings: &[FindingsExportFeature]) -> Option<InteropExtent> {
    let mut builder = ExtentBuilder::default();
    for finding in findings {
        for coordinate in &finding.polygon {
            builder.observe(*coordinate);
        }
    }
    builder.finish()
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

fn reprojected_geometry_type(geometry: &ReprojectedGeometry) -> &'static str {
    match geometry {
        ReprojectedGeometry::Point { .. } => "Point",
        ReprojectedGeometry::LineString { .. } => "LineString",
        ReprojectedGeometry::Polygon { .. } => "Polygon",
    }
}

impl From<FieldBoundaryValidationError> for FieldBoundaryRejectionReason {
    fn from(value: FieldBoundaryValidationError) -> Self {
        match value {
            FieldBoundaryValidationError::MissingCrs => Self::MissingCrs,
            FieldBoundaryValidationError::TooFewCoordinates => Self::TooFewCoordinates,
            FieldBoundaryValidationError::InvalidCoordinate => Self::InvalidCoordinate,
            FieldBoundaryValidationError::RingNotClosed => Self::RingNotClosed,
            FieldBoundaryValidationError::SelfIntersection => Self::SelfIntersection,
            FieldBoundaryValidationError::EmptyArea => Self::EmptyArea,
        }
    }
}

fn normalize_prescription_request(
    request: PrescriptionShapefileRequest,
) -> Result<NormalizedPrescription, InteropError> {
    let filename = normalized_filename(request.prescription_id.clone());
    let prescription_id =
        normalize_prescription_text(&request.prescription_id).ok_or_else(|| {
            prescription_rejected(&filename, PrescriptionRejectionReason::EmptyPrescriptionId)
        })?;
    let field_id = normalize_prescription_text(&request.field.field_id).ok_or_else(|| {
        prescription_rejected(&filename, PrescriptionRejectionReason::EmptyFieldId)
    })?;
    let field_crs = normalize_crs(&request.field.crs)
        .ok_or_else(|| prescription_rejected(&filename, PrescriptionRejectionReason::EmptyCrs))?;
    let field_boundary = normalize_prescription_ring(&request.field.boundary).ok_or_else(|| {
        prescription_rejected(&filename, PrescriptionRejectionReason::InvalidFieldGeometry)
    })?;
    let field_extent = extent_from_coordinates(&field_boundary).ok_or_else(|| {
        prescription_rejected(&filename, PrescriptionRejectionReason::InvalidFieldGeometry)
    })?;
    let field_area = polygon_area(&field_boundary);
    if field_area <= GEOMETRY_EPSILON || ring_self_intersects_coordinates(&field_boundary) {
        return Err(prescription_rejected(
            &filename,
            PrescriptionRejectionReason::InvalidFieldGeometry,
        ));
    }
    if request.zones.is_empty() {
        return Err(prescription_rejected(
            &filename,
            PrescriptionRejectionReason::EmptyZoneSet,
        ));
    }

    let mut zones = Vec::with_capacity(request.zones.len());
    for zone in request.zones {
        zones.push(normalize_prescription_zone(
            &filename,
            zone,
            &field_crs,
            &field_boundary,
            field_extent,
        )?);
    }
    assert_prescription_zones_do_not_overlap(&filename, &zones)?;
    assert_prescription_zones_tile_field(&filename, field_area, &zones)?;

    Ok(NormalizedPrescription {
        prescription_id,
        field_id,
        field_crs,
        field_extent,
        zones,
    })
}

fn backoff_for_attempt(policy: &JohnDeereRetryPolicy, attempt: usize) -> u64 {
    policy
        .backoff_millis
        .get(attempt.saturating_sub(1))
        .copied()
        .or_else(|| policy.backoff_millis.last().copied())
        .unwrap_or(0)
}

fn validate_john_deere_prescription_crs(crs: &str) -> Result<(), InteropError> {
    if matches!(crs, WGS84 | WEB_MERCATOR) {
        return Ok(());
    }
    Err(john_deere_rejected(
        JohnDeereConnectorError::UnsupportedPrescriptionCrs {
            crs: crs.to_string(),
        },
    ))
}

fn map_john_deere_boundary(
    boundary: JohnDeereBoundary,
) -> Result<JohnDeereMappedBoundary, InteropError> {
    let remote_field_id = normalize_prescription_text(&boundary.remote_field_id)
        .ok_or_else(|| john_deere_rejected(JohnDeereConnectorError::EmptyRemoteFieldId))?;
    let source_crs = normalize_crs(&boundary.crs).ok_or_else(|| {
        john_deere_rejected(JohnDeereConnectorError::UnsupportedBoundaryCrs {
            remote_field_id: remote_field_id.clone(),
            crs: boundary.crs.clone(),
        })
    })?;
    if source_crs.to_ascii_uppercase().contains("OBLIQUE")
        || !matches!(source_crs.as_str(), WGS84 | WEB_MERCATOR)
    {
        return Err(john_deere_rejected(
            JohnDeereConnectorError::UnsupportedBoundaryCrs {
                remote_field_id,
                crs: boundary.crs,
            },
        ));
    }
    let ring = normalize_prescription_ring(&boundary.boundary).ok_or_else(|| {
        john_deere_rejected(JohnDeereConnectorError::InvalidBoundaryGeometry {
            remote_field_id: remote_field_id.clone(),
        })
    })?;
    if polygon_area(&ring) <= GEOMETRY_EPSILON || ring_self_intersects_coordinates(&ring) {
        return Err(john_deere_rejected(
            JohnDeereConnectorError::InvalidBoundaryGeometry {
                remote_field_id: remote_field_id.clone(),
            },
        ));
    }
    let extent = extent_from_coordinates(&ring).ok_or_else(|| {
        john_deere_rejected(JohnDeereConnectorError::InvalidBoundaryGeometry {
            remote_field_id: remote_field_id.clone(),
        })
    })?;

    Ok(JohnDeereMappedBoundary {
        remote_field_id,
        name: normalize_prescription_text(&boundary.name)
            .unwrap_or_else(|| "<unnamed>".to_string()),
        source_crs: source_crs.clone(),
        target_crs: source_crs,
        extent,
        feature_count: 1,
        boundary: ring,
    })
}

fn normalize_prescription_zone(
    filename: &str,
    zone: PrescriptionZone,
    field_crs: &str,
    field_boundary: &[InteropCoordinate],
    field_extent: InteropExtent,
) -> Result<NormalizedPrescriptionZone, InteropError> {
    let zone_id = normalize_prescription_text(&zone.zone_id)
        .ok_or_else(|| prescription_rejected(filename, PrescriptionRejectionReason::EmptyZoneId))?;
    let zone_crs = normalize_crs(&zone.crs)
        .ok_or_else(|| prescription_rejected(filename, PrescriptionRejectionReason::EmptyCrs))?;
    if zone_crs != field_crs {
        return Err(prescription_rejected(
            filename,
            PrescriptionRejectionReason::CrsMismatch {
                zone_id,
                expected_crs: field_crs.to_string(),
                actual_crs: zone_crs,
            },
        ));
    }
    let unit = normalize_prescription_text(&zone.unit).ok_or_else(|| {
        prescription_rejected(
            filename,
            PrescriptionRejectionReason::EmptyUnit {
                zone_id: zone_id.clone(),
            },
        )
    })?;
    if !zone.rate.is_finite() || zone.rate < 0.0 {
        return Err(prescription_rejected(
            filename,
            PrescriptionRejectionReason::InvalidRate {
                zone_id: zone_id.clone(),
            },
        ));
    }
    let polygon = normalize_prescription_ring(&zone.polygon).ok_or_else(|| {
        prescription_rejected(
            filename,
            PrescriptionRejectionReason::InvalidZoneGeometry {
                zone_id: zone_id.clone(),
            },
        )
    })?;
    let extent = extent_from_coordinates(&polygon).ok_or_else(|| {
        prescription_rejected(
            filename,
            PrescriptionRejectionReason::InvalidZoneGeometry {
                zone_id: zone_id.clone(),
            },
        )
    })?;
    let area = polygon_area(&polygon);
    if area <= GEOMETRY_EPSILON || ring_self_intersects_coordinates(&polygon) {
        return Err(prescription_rejected(
            filename,
            PrescriptionRejectionReason::InvalidZoneGeometry {
                zone_id: zone_id.clone(),
            },
        ));
    }
    if !rate_fits_dbf(zone.rate) {
        return Err(prescription_rejected(
            filename,
            PrescriptionRejectionReason::InvalidRate {
                zone_id: zone_id.clone(),
            },
        ));
    }
    if !extent_within(extent, field_extent)
        || !polygon_inside_or_on_polygon(&polygon, field_boundary)
    {
        return Err(prescription_rejected(
            filename,
            PrescriptionRejectionReason::ZoneOutsideField {
                zone_id: zone_id.clone(),
            },
        ));
    }

    Ok(NormalizedPrescriptionZone {
        zone_id,
        polygon,
        extent,
        area,
        rate: zone.rate,
        unit,
    })
}

fn assert_prescription_zones_do_not_overlap(
    filename: &str,
    zones: &[NormalizedPrescriptionZone],
) -> Result<(), InteropError> {
    for left_index in 0..zones.len() {
        for right_index in (left_index + 1)..zones.len() {
            let left = &zones[left_index];
            let right = &zones[right_index];
            if extents_have_positive_overlap(left.extent, right.extent)
                && (polygons_overlap(&left.polygon, &right.polygon)
                    || extent_overlap_center_inside_both(left, right))
            {
                return Err(prescription_rejected(
                    filename,
                    PrescriptionRejectionReason::OverlappingZones {
                        left_zone_id: left.zone_id.clone(),
                        right_zone_id: right.zone_id.clone(),
                    },
                ));
            }
        }
    }
    Ok(())
}

fn assert_prescription_zones_tile_field(
    filename: &str,
    field_area: f64,
    zones: &[NormalizedPrescriptionZone],
) -> Result<(), InteropError> {
    let zone_area = zones.iter().map(|zone| zone.area).sum::<f64>();
    let tolerance = GEOMETRY_EPSILON.max(field_area.abs() * 1e-9);
    if (zone_area - field_area).abs() > tolerance {
        return Err(prescription_rejected(
            filename,
            PrescriptionRejectionReason::ZoneCoverageGap,
        ));
    }
    Ok(())
}

fn write_prescription_shapefile_bundle(
    crs: &str,
    extent: InteropExtent,
    zones: &[NormalizedPrescriptionZone],
) -> PrescriptionShapefileFiles {
    let record_contents = zones
        .iter()
        .map(|zone| polygon_shapefile_record_content(&zone.polygon, zone.extent))
        .collect::<Vec<_>>();
    PrescriptionShapefileFiles {
        shp: write_shp_bytes(extent, &record_contents),
        shx: write_shx_bytes(extent, &record_contents),
        dbf: write_prescription_dbf(zones),
        prj: projection_wkt(crs).into_bytes(),
    }
}

fn write_shp_bytes(extent: InteropExtent, record_contents: &[Vec<u8>]) -> Vec<u8> {
    let file_len_bytes = SHAPEFILE_HEADER_BYTES
        + record_contents
            .iter()
            .map(|content| 8 + content.len())
            .sum::<usize>();
    let mut bytes = shapefile_header(extent, file_len_bytes);
    for (index, content) in record_contents.iter().enumerate() {
        bytes.extend_from_slice(&((index as i32) + 1).to_be_bytes());
        bytes.extend_from_slice(&((content.len() / 2) as i32).to_be_bytes());
        bytes.extend_from_slice(content);
    }
    bytes
}

fn write_shx_bytes(extent: InteropExtent, record_contents: &[Vec<u8>]) -> Vec<u8> {
    let file_len_bytes = SHAPEFILE_HEADER_BYTES + record_contents.len() * 8;
    let mut bytes = shapefile_header(extent, file_len_bytes);
    let mut record_offset_words = (SHAPEFILE_HEADER_BYTES / 2) as i32;
    for content in record_contents {
        let content_len_words = (content.len() / 2) as i32;
        bytes.extend_from_slice(&record_offset_words.to_be_bytes());
        bytes.extend_from_slice(&content_len_words.to_be_bytes());
        record_offset_words += 4 + content_len_words;
    }
    bytes
}

fn shapefile_header(extent: InteropExtent, file_len_bytes: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; SHAPEFILE_HEADER_BYTES];
    bytes[0..4].copy_from_slice(&ESRI_FILE_CODE.to_be_bytes());
    bytes[24..28].copy_from_slice(&((file_len_bytes / 2) as i32).to_be_bytes());
    bytes[28..32].copy_from_slice(&SHAPEFILE_VERSION.to_le_bytes());
    bytes[32..36].copy_from_slice(&SHAPE_TYPE_POLYGON.to_le_bytes());
    bytes[36..44].copy_from_slice(&extent.min_x.to_le_bytes());
    bytes[44..52].copy_from_slice(&extent.min_y.to_le_bytes());
    bytes[52..60].copy_from_slice(&extent.max_x.to_le_bytes());
    bytes[60..68].copy_from_slice(&extent.max_y.to_le_bytes());
    bytes
}

fn polygon_shapefile_record_content(
    polygon: &[InteropCoordinate],
    extent: InteropExtent,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(44 + polygon.len() * 16);
    bytes.extend_from_slice(&SHAPE_TYPE_POLYGON.to_le_bytes());
    bytes.extend_from_slice(&extent.min_x.to_le_bytes());
    bytes.extend_from_slice(&extent.min_y.to_le_bytes());
    bytes.extend_from_slice(&extent.max_x.to_le_bytes());
    bytes.extend_from_slice(&extent.max_y.to_le_bytes());
    bytes.extend_from_slice(&1i32.to_le_bytes());
    bytes.extend_from_slice(&(polygon.len() as i32).to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    for coordinate in polygon {
        bytes.extend_from_slice(&coordinate.x.to_le_bytes());
        bytes.extend_from_slice(&coordinate.y.to_le_bytes());
    }
    bytes
}

fn write_prescription_dbf(zones: &[NormalizedPrescriptionZone]) -> Vec<u8> {
    let fields = [
        DbfField::character("ZONE_ID", 32),
        DbfField::numeric(
            PRESCRIPTION_RATE_ATTRIBUTE,
            PRESCRIPTION_RATE_FIELD_WIDTH,
            PRESCRIPTION_RATE_DECIMALS,
        ),
        DbfField::character(PRESCRIPTION_UNIT_ATTRIBUTE, 16),
    ];
    let header_len = 32 + fields.len() * 32 + 1;
    let record_len = 1 + fields
        .iter()
        .map(|field| field.length as usize)
        .sum::<usize>();
    let mut bytes = vec![0u8; 32];
    bytes[0] = 0x03;
    bytes[1] = 126;
    bytes[2] = 6;
    bytes[3] = 13;
    bytes[4..8].copy_from_slice(&(zones.len() as u32).to_le_bytes());
    bytes[8..10].copy_from_slice(&(header_len as u16).to_le_bytes());
    bytes[10..12].copy_from_slice(&(record_len as u16).to_le_bytes());
    for field in &fields {
        bytes.extend_from_slice(&field.descriptor());
    }
    bytes.push(0x0D);

    for zone in zones {
        bytes.push(b' ');
        bytes.extend_from_slice(&dbf_character_value(&zone.zone_id, fields[0].length));
        bytes.extend_from_slice(&dbf_numeric_value(
            zone.rate,
            fields[1].length,
            fields[1].decimal_count,
        ));
        bytes.extend_from_slice(&dbf_character_value(&zone.unit, fields[2].length));
    }
    bytes.push(0x1A);
    bytes
}

fn write_findings_dbf(findings: &[FindingsExportFeature]) -> Vec<u8> {
    let fields = [
        DbfField::character("FINDING_ID", 40),
        DbfField::character("ZONE_ID", 32),
        DbfField::character("REASON", 32),
        DbfField::character("PRIORITY", 16),
        DbfField::character("CRS", 24),
    ];
    let header_len = 32 + fields.len() * 32 + 1;
    let record_len = 1 + fields
        .iter()
        .map(|field| field.length as usize)
        .sum::<usize>();
    let mut bytes = vec![0u8; 32];
    bytes[0] = 0x03;
    bytes[1] = 126;
    bytes[2] = 6;
    bytes[3] = 13;
    bytes[4..8].copy_from_slice(&(findings.len() as u32).to_le_bytes());
    bytes[8..10].copy_from_slice(&(header_len as u16).to_le_bytes());
    bytes[10..12].copy_from_slice(&(record_len as u16).to_le_bytes());
    for field in &fields {
        bytes.extend_from_slice(&field.descriptor());
    }
    bytes.push(0x0D);

    for finding in findings {
        bytes.push(b' ');
        bytes.extend_from_slice(&dbf_character_value(&finding.finding_id, fields[0].length));
        bytes.extend_from_slice(&dbf_character_value(&finding.zone_id, fields[1].length));
        bytes.extend_from_slice(&dbf_character_value(&finding.reason, fields[2].length));
        bytes.extend_from_slice(&dbf_character_value(&finding.priority, fields[3].length));
        bytes.extend_from_slice(&dbf_character_value(&finding.crs, fields[4].length));
    }
    bytes.push(0x1A);
    bytes
}

fn taskdata_unit_designator(prescription: &NormalizedPrescription) -> Result<String, InteropError> {
    let mut unit_designator = None::<String>;
    for zone in &prescription.zones {
        let mapped = taskdata_unit_mapping(&zone.unit).ok_or_else(|| {
            prescription_rejected(
                &prescription.prescription_id,
                PrescriptionRejectionReason::UnsupportedTaskDataUnit {
                    zone_id: zone.zone_id.clone(),
                    unit: zone.unit.clone(),
                },
            )
        })?;
        if let Some(expected) = &unit_designator {
            if expected != mapped {
                return Err(prescription_rejected(
                    &prescription.prescription_id,
                    PrescriptionRejectionReason::MixedTaskDataUnits {
                        zone_id: zone.zone_id.clone(),
                        expected_unit: expected.clone(),
                        actual_unit: mapped.to_string(),
                    },
                ));
            }
        } else {
            unit_designator = Some(mapped.to_string());
        }
    }
    Ok(unit_designator.expect("prescription has at least one zone"))
}

fn taskdata_unit_mapping(unit: &str) -> Option<&'static str> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "kg_ha" | "kg/ha" | "kg-ha" => Some("kg/ha"),
        "l_ha" | "l/ha" | "l-ha" => Some("l/ha"),
        "seed_ha" | "seeds_ha" | "seeds/ha" => Some("seeds/ha"),
        _ => None,
    }
}

fn write_taskdata_xml(prescription: &NormalizedPrescription, unit_designator: &str) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<ISO11783_TaskData VersionMajor=\"4\" VersionMinor=\"3\" ManagementSoftwareManufacturer=\"AGBot\" ManagementSoftwareVersion=\"interop.taskdata.v1\">\n");
    xml.push_str(&format!(
        "  <CTR A=\"CTR1\" B=\"{}\" />\n",
        escape_xml("AGBot")
    ));
    xml.push_str(&format!(
        "  <FRM A=\"FRM1\" B=\"{}\" />\n",
        escape_xml(&prescription.field_id)
    ));
    xml.push_str(&format!(
        "  <PFD A=\"PFD1\" B=\"{}\" Crs=\"{}\" MinX=\"{:.6}\" MinY=\"{:.6}\" MaxX=\"{:.6}\" MaxY=\"{:.6}\" />\n",
        escape_xml(&prescription.field_id),
        escape_xml(&prescription.field_crs),
        prescription.field_extent.min_x,
        prescription.field_extent.min_y,
        prescription.field_extent.max_x,
        prescription.field_extent.max_y
    ));
    xml.push_str(&format!(
        "  <PDT A=\"PDT1\" B=\"{}\" C=\"{}\" />\n",
        escape_xml("AGBot prescription product"),
        escape_xml(unit_designator)
    ));
    xml.push_str(&format!(
        "  <TSK A=\"TSK1\" B=\"{}\" C=\"CTR1\" D=\"FRM1\" E=\"PFD1\" Crs=\"{}\">\n",
        escape_xml(&prescription.prescription_id),
        escape_xml(&prescription.field_crs)
    ));
    xml.push_str(&format!(
        "  <PGP A=\"PGP1\" B=\"PDT1\" C=\"{}\" />\n",
        escape_xml(unit_designator)
    ));
    for (zone_index, zone) in prescription.zones.iter().enumerate() {
        let zone_ref = format!("TZN{}", zone_index + 1);
        xml.push_str(&format!(
            "    <TZN A=\"{}\" B=\"{}\" C=\"{:.6}\" D=\"{}\" Crs=\"{}\">\n",
            zone_ref,
            escape_xml(&zone.zone_id),
            zone.rate,
            escape_xml(unit_designator),
            escape_xml(&prescription.field_crs)
        ));
        xml.push_str(&format!(
            "      <PLN A=\"PLN{}\" B=\"polygon\">\n",
            zone_index + 1
        ));
        for (point_index, coordinate) in zone.polygon.iter().enumerate() {
            xml.push_str(&format!(
                "        <PNT A=\"PNT{}-{}\" B=\"{:.6}\" C=\"{:.6}\" />\n",
                zone_index + 1,
                point_index + 1,
                coordinate.x,
                coordinate.y
            ));
        }
        xml.push_str("      </PLN>\n");
        xml.push_str("    </TZN>\n");
    }
    xml.push_str("  </TSK>\n");
    xml.push_str("</ISO11783_TaskData>\n");
    xml
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn taskdata_schema_rejected(reason: String) -> InteropError {
    InteropError::Rejected {
        filename: TASKDATA_FILENAME.to_string(),
        reason: InteropRejectionReason::InvalidPrescription {
            reason: PrescriptionRejectionReason::InvalidTaskDataSchema { reason },
        },
    }
}

#[derive(Debug, Clone, Copy)]
struct XmlTag<'a> {
    name: &'a str,
    raw: &'a str,
    end: usize,
    closing: bool,
    self_closing: bool,
}

fn collect_xml_tags<'a>(xml: &'a str, name: &str) -> Result<Vec<XmlTag<'a>>, InteropError> {
    let mut tags = Vec::new();
    let mut offset = 0usize;
    while let Some(relative_start) = xml[offset..].find('<') {
        let start = offset + relative_start;
        let Some(tag) = next_xml_tag(&xml[start..]) else {
            return Err(taskdata_schema_rejected("malformed XML tag".to_string()));
        };
        let absolute = XmlTag {
            end: start + tag.end,
            ..tag
        };
        if absolute.name == name && !absolute.closing {
            tags.push(absolute);
        }
        offset = absolute.end;
    }
    Ok(tags)
}

fn next_xml_tag(xml: &str) -> Option<XmlTag<'_>> {
    let start = xml.find('<')?;
    let end = xml[start..].find('>')? + start + 1;
    let raw = &xml[start + 1..end - 1];
    if raw.starts_with('?') || raw.starts_with('!') {
        return Some(XmlTag {
            name: "",
            raw,
            end,
            closing: false,
            self_closing: raw.ends_with('/'),
        });
    }
    let trimmed = raw.trim();
    let closing = trimmed.starts_with('/');
    let tag_text = trimmed.trim_start_matches('/').trim_end_matches('/').trim();
    let name_end = tag_text.find(char::is_whitespace).unwrap_or(tag_text.len());
    let name = &tag_text[..name_end];
    Some(XmlTag {
        name,
        raw,
        end,
        closing,
        self_closing: trimmed.ends_with('/'),
    })
}

fn tag_body<'a>(xml: &'a str, tag: &XmlTag<'a>, name: &str) -> Result<&'a str, InteropError> {
    let close = format!("</{name}>");
    let close_start = xml[tag.end..]
        .find(&close)
        .map(|offset| tag.end + offset)
        .ok_or_else(|| taskdata_schema_rejected(format!("missing {name} close")))?;
    Ok(&xml[tag.end..close_start])
}

fn parse_xml_attributes(raw: &str) -> Result<Map<String, Value>, InteropError> {
    let tag_text = raw
        .trim()
        .trim_start_matches('/')
        .trim_end_matches('/')
        .trim();
    let mut cursor = tag_text.find(char::is_whitespace).unwrap_or(tag_text.len());
    let mut attrs = Map::new();
    while cursor < tag_text.len() {
        while cursor < tag_text.len() && tag_text.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= tag_text.len() {
            break;
        }
        let key_start = cursor;
        while cursor < tag_text.len()
            && tag_text.as_bytes()[cursor] != b'='
            && !tag_text.as_bytes()[cursor].is_ascii_whitespace()
        {
            cursor += 1;
        }
        let key = tag_text[key_start..cursor].trim();
        while cursor < tag_text.len() && tag_text.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= tag_text.len() || tag_text.as_bytes()[cursor] != b'=' {
            return Err(taskdata_schema_rejected(
                "malformed XML attribute".to_string(),
            ));
        }
        cursor += 1;
        while cursor < tag_text.len() && tag_text.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= tag_text.len() || tag_text.as_bytes()[cursor] != b'"' {
            return Err(taskdata_schema_rejected(
                "malformed XML attribute".to_string(),
            ));
        }
        cursor += 1;
        let value_start = cursor;
        let Some(relative_end) = tag_text[cursor..].find('"') else {
            return Err(taskdata_schema_rejected(
                "malformed XML attribute".to_string(),
            ));
        };
        cursor += relative_end;
        let value = unescape_xml(&tag_text[value_start..cursor]);
        cursor += 1;
        if attrs
            .insert(key.to_string(), Value::String(value))
            .is_some()
        {
            return Err(taskdata_schema_rejected(
                "duplicate XML attribute".to_string(),
            ));
        }
    }
    Ok(attrs)
}

fn required_attr<'a>(attrs: &'a Map<String, Value>, name: &str) -> Result<&'a str, InteropError> {
    attrs
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| taskdata_schema_rejected(format!("missing required {name} attribute")))
}

fn unescape_xml(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[derive(Debug, Clone, Copy)]
struct DbfField {
    name: &'static str,
    field_type: u8,
    length: u8,
    decimal_count: u8,
}

impl DbfField {
    fn character(name: &'static str, length: u8) -> Self {
        Self {
            name,
            field_type: b'C',
            length,
            decimal_count: 0,
        }
    }

    fn numeric(name: &'static str, length: u8, decimal_count: u8) -> Self {
        Self {
            name,
            field_type: b'N',
            length,
            decimal_count,
        }
    }

    fn descriptor(self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        let name = self.name.as_bytes();
        let name_len = name.len().min(11);
        bytes[0..name_len].copy_from_slice(&name[0..name_len]);
        bytes[11] = self.field_type;
        bytes[16] = self.length;
        bytes[17] = self.decimal_count;
        bytes
    }
}

fn dbf_character_value(value: &str, width: u8) -> Vec<u8> {
    let width = width as usize;
    let mut bytes = vec![b' '; width];
    let value = value.as_bytes();
    let len = value.len().min(width);
    bytes[0..len].copy_from_slice(&value[0..len]);
    bytes
}

fn dbf_numeric_value(value: f64, width: u8, decimals: u8) -> Vec<u8> {
    let width = width as usize;
    let decimals = decimals as usize;
    let rendered = format!(
        "{value:>width$.decimals$}",
        width = width,
        decimals = decimals
    );
    if rendered.len() > width {
        return vec![b'*'; width];
    }
    let mut bytes = vec![b' '; width];
    let start = width - rendered.len();
    bytes[start..].copy_from_slice(rendered.as_bytes());
    bytes
}

fn rate_fits_dbf(rate: f64) -> bool {
    format!(
        "{rate:.decimals$}",
        decimals = PRESCRIPTION_RATE_DECIMALS as usize
    )
    .len()
        <= PRESCRIPTION_RATE_FIELD_WIDTH as usize
}

fn projection_wkt(crs: &str) -> String {
    match crs {
        WGS84 => "GEOGCS[\"WGS 84\",DATUM[\"WGS_1984\",SPHEROID[\"WGS 84\",6378137,298.257223563]],PRIMEM[\"Greenwich\",0],UNIT[\"degree\",0.0174532925199433],AUTHORITY[\"EPSG\",\"4326\"]]".to_string(),
        WEB_MERCATOR => "PROJCS[\"WGS 84 / Pseudo-Mercator\",GEOGCS[\"WGS 84\",DATUM[\"WGS_1984\",SPHEROID[\"WGS 84\",6378137,298.257223563]],PRIMEM[\"Greenwich\",0],UNIT[\"degree\",0.0174532925199433]],PROJECTION[\"Mercator_1SP\"],PARAMETER[\"central_meridian\",0],PARAMETER[\"scale_factor\",1],PARAMETER[\"false_easting\",0],PARAMETER[\"false_northing\",0],UNIT[\"metre\",1],AUTHORITY[\"EPSG\",\"3857\"]]".to_string(),
        _ => epsg_utm_wkt(crs).unwrap_or_else(|| {
            let code = crs.strip_prefix("EPSG:").unwrap_or(crs);
            format!("PROJCS[\"{crs}\",AUTHORITY[\"EPSG\",\"{code}\"]]")
        }),
    }
}

fn epsg_utm_wkt(crs: &str) -> Option<String> {
    let code = crs.strip_prefix("EPSG:")?.parse::<i32>().ok()?;
    let (hemisphere, zone, false_northing) = if (32601..=32660).contains(&code) {
        ("N", code - 32600, 0)
    } else if (32701..=32760).contains(&code) {
        ("S", code - 32700, 10_000_000)
    } else {
        return None;
    };
    let central_meridian = zone * 6 - 183;
    Some(format!(
        "PROJCS[\"WGS 84 / UTM zone {zone}{hemisphere}\",GEOGCS[\"WGS 84\",DATUM[\"WGS_1984\",SPHEROID[\"WGS 84\",6378137,298.257223563]],PRIMEM[\"Greenwich\",0],UNIT[\"degree\",0.0174532925199433]],PROJECTION[\"Transverse_Mercator\"],PARAMETER[\"latitude_of_origin\",0],PARAMETER[\"central_meridian\",{central_meridian}],PARAMETER[\"scale_factor\",0.9996],PARAMETER[\"false_easting\",500000],PARAMETER[\"false_northing\",{false_northing}],UNIT[\"metre\",1],AUTHORITY[\"EPSG\",\"{code}\"]]"
    ))
}

fn normalize_prescription_text(value: &str) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn normalize_prescription_ring(points: &[InteropCoordinate]) -> Option<Vec<InteropCoordinate>> {
    if points.len() < 4
        || points
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return None;
    }
    if !same_coordinate(
        *points.first().expect("points length checked"),
        *points.last().expect("points length checked"),
    ) {
        return None;
    }
    Some(clockwise_ring(points))
}

fn extent_from_coordinates(points: &[InteropCoordinate]) -> Option<InteropExtent> {
    let mut builder = ExtentBuilder::default();
    for point in points {
        builder.observe(*point);
    }
    builder.finish()
}

fn extent_within(inner: InteropExtent, outer: InteropExtent) -> bool {
    inner.min_x >= outer.min_x - GEOMETRY_EPSILON
        && inner.min_y >= outer.min_y - GEOMETRY_EPSILON
        && inner.max_x <= outer.max_x + GEOMETRY_EPSILON
        && inner.max_y <= outer.max_y + GEOMETRY_EPSILON
}

fn extents_have_positive_overlap(left: InteropExtent, right: InteropExtent) -> bool {
    left.min_x < right.max_x - GEOMETRY_EPSILON
        && left.max_x > right.min_x + GEOMETRY_EPSILON
        && left.min_y < right.max_y - GEOMETRY_EPSILON
        && left.max_y > right.min_y + GEOMETRY_EPSILON
}

fn extent_overlap_center_inside_both(
    left: &NormalizedPrescriptionZone,
    right: &NormalizedPrescriptionZone,
) -> bool {
    let overlap = InteropCoordinate {
        x: (left.extent.min_x.max(right.extent.min_x) + left.extent.max_x.min(right.extent.max_x))
            * 0.5,
        y: (left.extent.min_y.max(right.extent.min_y) + left.extent.max_y.min(right.extent.max_y))
            * 0.5,
    };
    point_strictly_inside_polygon(overlap, &left.polygon)
        && point_strictly_inside_polygon(overlap, &right.polygon)
}

fn polygon_area(points: &[InteropCoordinate]) -> f64 {
    signed_polygon_area(points).abs()
}

fn signed_polygon_area(points: &[InteropCoordinate]) -> f64 {
    points
        .windows(2)
        .map(|window| window[0].x * window[1].y - window[1].x * window[0].y)
        .sum::<f64>()
        * 0.5
}

fn clockwise_ring(points: &[InteropCoordinate]) -> Vec<InteropCoordinate> {
    let mut ring = points
        .iter()
        .take(points.len().saturating_sub(1))
        .copied()
        .collect::<Vec<_>>();
    if signed_polygon_area(points) > 0.0 {
        ring.reverse();
    }
    if let Some(first) = ring.first().copied() {
        ring.push(first);
    }
    ring
}

fn polygons_overlap(left: &[InteropCoordinate], right: &[InteropCoordinate]) -> bool {
    left.iter()
        .take(left.len().saturating_sub(1))
        .any(|point| point_strictly_inside_polygon(*point, right))
        || right
            .iter()
            .take(right.len().saturating_sub(1))
            .any(|point| point_strictly_inside_polygon(*point, left))
        || point_strictly_inside_polygon(polygon_centroid(left), right)
        || point_strictly_inside_polygon(polygon_centroid(right), left)
        || rings_have_proper_intersection(left, right)
}

fn polygon_inside_or_on_polygon(inner: &[InteropCoordinate], outer: &[InteropCoordinate]) -> bool {
    inner
        .iter()
        .all(|coordinate| point_in_or_on_polygon(*coordinate, outer))
        && inner
            .windows(2)
            .all(|segment| segment_inside_or_on_polygon(segment[0], segment[1], outer))
}

fn segment_inside_or_on_polygon(
    start: InteropCoordinate,
    end: InteropCoordinate,
    polygon: &[InteropCoordinate],
) -> bool {
    let midpoint = InteropCoordinate {
        x: (start.x + end.x) * 0.5,
        y: (start.y + end.y) * 0.5,
    };
    point_in_or_on_polygon(midpoint, polygon)
        && !polygon
            .windows(2)
            .any(|segment| segments_properly_intersect(start, end, segment[0], segment[1]))
}

fn point_in_or_on_polygon(point: InteropCoordinate, polygon: &[InteropCoordinate]) -> bool {
    point_on_polygon_boundary(point, polygon) || point_strictly_inside_polygon(point, polygon)
}

fn point_strictly_inside_polygon(point: InteropCoordinate, polygon: &[InteropCoordinate]) -> bool {
    if point_on_polygon_boundary(point, polygon) {
        return false;
    }
    let mut inside = false;
    for segment in polygon.windows(2) {
        let a = segment[0];
        let b = segment[1];
        let crosses = (a.y > point.y) != (b.y > point.y);
        if crosses {
            let intersection_x = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
            if point.x < intersection_x {
                inside = !inside;
            }
        }
    }
    inside
}

fn point_on_polygon_boundary(point: InteropCoordinate, polygon: &[InteropCoordinate]) -> bool {
    polygon
        .windows(2)
        .any(|segment| point_on_segment_coordinates(segment[0], point, segment[1]))
}

fn polygon_centroid(points: &[InteropCoordinate]) -> InteropCoordinate {
    let unique = points.len().saturating_sub(1).max(1);
    let (sum_x, sum_y) = points
        .iter()
        .take(unique)
        .fold((0.0, 0.0), |(sum_x, sum_y), point| {
            (sum_x + point.x, sum_y + point.y)
        });
    InteropCoordinate {
        x: sum_x / unique as f64,
        y: sum_y / unique as f64,
    }
}

fn ring_self_intersects_coordinates(points: &[InteropCoordinate]) -> bool {
    let segment_count = points.len().saturating_sub(1);
    for left in 0..segment_count {
        for right in (left + 1)..segment_count {
            if segments_share_ring_vertex(left, right, segment_count) {
                continue;
            }
            if segments_intersect_coordinates(
                points[left],
                points[left + 1],
                points[right],
                points[right + 1],
            ) {
                return true;
            }
        }
    }
    false
}

fn rings_have_proper_intersection(left: &[InteropCoordinate], right: &[InteropCoordinate]) -> bool {
    for left_segment in left.windows(2) {
        for right_segment in right.windows(2) {
            if segments_properly_intersect(
                left_segment[0],
                left_segment[1],
                right_segment[0],
                right_segment[1],
            ) {
                return true;
            }
        }
    }
    false
}

fn segments_intersect_coordinates(
    a: InteropCoordinate,
    b: InteropCoordinate,
    c: InteropCoordinate,
    d: InteropCoordinate,
) -> bool {
    let o1 = orientation_coordinates(a, b, c);
    let o2 = orientation_coordinates(a, b, d);
    let o3 = orientation_coordinates(c, d, a);
    let o4 = orientation_coordinates(c, d, b);

    if orientation_sign(o1) != orientation_sign(o2) && orientation_sign(o3) != orientation_sign(o4)
    {
        return true;
    }

    (orientation_is_colinear(o1) && point_on_segment_coordinates(a, c, b))
        || (orientation_is_colinear(o2) && point_on_segment_coordinates(a, d, b))
        || (orientation_is_colinear(o3) && point_on_segment_coordinates(c, a, d))
        || (orientation_is_colinear(o4) && point_on_segment_coordinates(c, b, d))
}

fn segments_properly_intersect(
    a: InteropCoordinate,
    b: InteropCoordinate,
    c: InteropCoordinate,
    d: InteropCoordinate,
) -> bool {
    let o1 = orientation_coordinates(a, b, c);
    let o2 = orientation_coordinates(a, b, d);
    let o3 = orientation_coordinates(c, d, a);
    let o4 = orientation_coordinates(c, d, b);
    orientation_sign(o1) != 0
        && orientation_sign(o2) != 0
        && orientation_sign(o3) != 0
        && orientation_sign(o4) != 0
        && orientation_sign(o1) != orientation_sign(o2)
        && orientation_sign(o3) != orientation_sign(o4)
}

fn orientation_coordinates(
    a: InteropCoordinate,
    b: InteropCoordinate,
    c: InteropCoordinate,
) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
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
    value.abs() <= GEOMETRY_EPSILON
}

fn segments_share_ring_vertex(left: usize, right: usize, segment_count: usize) -> bool {
    left == right || left + 1 == right || (left == 0 && right + 1 == segment_count)
}

fn point_on_segment_coordinates(
    start: InteropCoordinate,
    point: InteropCoordinate,
    end: InteropCoordinate,
) -> bool {
    orientation_is_colinear(orientation_coordinates(start, end, point))
        && point.x >= start.x.min(end.x) - GEOMETRY_EPSILON
        && point.x <= start.x.max(end.x) + GEOMETRY_EPSILON
        && point.y >= start.y.min(end.y) - GEOMETRY_EPSILON
        && point.y <= start.y.max(end.y) + GEOMETRY_EPSILON
}

fn same_coordinate(left: InteropCoordinate, right: InteropCoordinate) -> bool {
    (left.x - right.x).abs() <= GEOMETRY_EPSILON && (left.y - right.y).abs() <= GEOMETRY_EPSILON
}

fn prescription_rejected(filename: &str, reason: PrescriptionRejectionReason) -> InteropError {
    rejected(
        filename,
        InteropRejectionReason::InvalidPrescription { reason },
    )
}

fn john_deere_rejected(reason: JohnDeereConnectorError) -> InteropError {
    rejected(
        "john-deere-operations-center",
        InteropRejectionReason::JohnDeereConnector { reason },
    )
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
    if crs.to_ascii_uppercase().contains("OBLIQUE")
        || !(matches!(crs, WGS84 | WEB_MERCATOR) || epsg_utm_wkt(crs).is_some())
    {
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

fn normalize_optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
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
        export_findings_geojson, export_findings_shapefile, export_prescription_shapefile,
        export_prescription_taskdata, export_raster_geotiff, import_field_boundary,
        pull_john_deere_boundaries, push_john_deere_prescription, reopen_raster_geotiff,
        round_trip_vector_layer, validate_and_reproject_import, validate_geopackage_layers,
        validate_taskdata_xml, CrsTransform, FieldBoundaryImportRequest,
        FieldBoundaryRejectionReason, FindingsExportFeature, FindingsExportRequest, ImportFormat,
        ImportPayload, InteropCoordinate, InteropError, InteropExtent, InteropRejectionReason,
        JohnDeereBoundary, JohnDeereConnectorEndpoint, JohnDeereConnectorError,
        JohnDeereEndpointError, JohnDeerePrescriptionPushRequest, JohnDeereRetryPolicy,
        JohnDeereUploadPayload, PrescriptionField, PrescriptionRejectionReason,
        PrescriptionShapefileRequest, PrescriptionZone, RasterProduct, RemotePrescriptionReceipt,
        ReprojectedGeometry,
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

    #[test]
    fn findings_geojson_export_preserves_crs_and_schema() {
        let report = export_findings_geojson(findings_request(vec![finding_feature("finding-1")]))
            .expect("findings GeoJSON should export");
        let document: serde_json::Value =
            serde_json::from_slice(&report.exported_bytes).expect("GeoJSON should parse");

        assert_eq!(report.crs, "EPSG:32614");
        assert_eq!(report.feature_count, 1);
        assert_eq!(document["type"], "FeatureCollection");
        assert_eq!(
            document
                .pointer("/crs/properties/name")
                .and_then(|value| value.as_str()),
            Some("EPSG:32614")
        );
        assert_eq!(
            document.pointer("/features/0/properties/finding_id"),
            Some(&serde_json::json!("finding-1"))
        );
        assert_eq!(
            document.pointer("/features/0/geometry/type"),
            Some(&serde_json::json!("Polygon"))
        );
    }

    #[test]
    fn findings_shapefile_export_writes_consistent_bundle() {
        let report =
            export_findings_shapefile(findings_request(vec![finding_feature("finding-1")]))
                .expect("findings shapefile should export");

        assert_eq!(report.crs, "EPSG:32614");
        assert_eq!(report.feature_count, 1);
        assert_eq!(report.extent.expect("extent").min_x, 500_000.0);
        assert_shapefile_files_consistent(
            &report.files.shp,
            &report.files.shx,
            &report.files.dbf,
            &report.files.prj,
            1,
        );
    }

    #[test]
    fn empty_findings_exports_are_valid_empty_geojson_and_shapefile() {
        let geojson =
            export_findings_geojson(findings_request(Vec::new())).expect("empty GeoJSON exports");
        let document: serde_json::Value =
            serde_json::from_slice(&geojson.exported_bytes).expect("GeoJSON should parse");
        assert_eq!(geojson.feature_count, 0);
        assert!(document["features"]
            .as_array()
            .expect("features")
            .is_empty());

        let shapefile = export_findings_shapefile(findings_request(Vec::new()))
            .expect("empty shapefile exports");
        assert_eq!(shapefile.feature_count, 0);
        assert_eq!(shapefile.extent, None);
        assert_shapefile_files_consistent(
            &shapefile.files.shp,
            &shapefile.files.shx,
            &shapefile.files.dbf,
            &shapefile.files.prj,
            0,
        );
    }

    #[test]
    fn field_boundary_import_creates_field_record_with_area_and_source_crs() {
        let report = import_field_boundary(FieldBoundaryImportRequest {
            payload: ImportPayload {
                format: ImportFormat::GeoJson,
                filename: "field-alpha-boundary.geojson".to_string(),
                bytes: valid_geojson("EPSG:4326").as_bytes().to_vec(),
            },
            target_crs: "EPSG:4326".to_string(),
            field_id: "field-alpha".to_string(),
            farm_id: Some("farm-alpha".to_string()),
            org_id: "org-alpha".to_string(),
            owner: "owner-alpha".to_string(),
            name: "North Block".to_string(),
            created_at: "2026-06-12T14:00:00Z".to_string(),
        })
        .expect("valid boundary should import");

        assert_eq!(report.source_filename, "field-alpha-boundary.geojson");
        assert_eq!(report.source_crs, "EPSG:4326");
        assert_eq!(report.target_crs, "EPSG:4326");
        assert_eq!(report.feature_count, 1);
        assert_eq!(report.field.field_id, "field-alpha");
        assert_eq!(report.field.farm_id.as_deref(), Some("farm-alpha"));
        assert_eq!(report.field.org_id, "org-alpha");
        assert_eq!(report.field.owner, "owner-alpha");
        assert_eq!(report.field.name, "North Block");
        assert_eq!(report.field.boundary.crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(
            report.field.boundary.coordinates.first(),
            report.field.boundary.coordinates.last()
        );
        assert!(report.field.area_ha.expect("area should be set") > 0.0);
        assert_eq!(report.field.extent.min_lon, -121.0);
        assert_eq!(report.field.extent.max_lon, -120.99);
    }

    #[test]
    fn field_boundary_import_rejects_self_intersecting_ring() {
        let error = import_field_boundary(FieldBoundaryImportRequest {
            payload: ImportPayload {
                format: ImportFormat::GeoJson,
                filename: "bowtie-boundary.geojson".to_string(),
                bytes: bowtie_geojson("EPSG:4326").as_bytes().to_vec(),
            },
            target_crs: "EPSG:4326".to_string(),
            field_id: "field-bowtie".to_string(),
            farm_id: None,
            org_id: "org-alpha".to_string(),
            owner: "owner-alpha".to_string(),
            name: "Invalid Bowtie".to_string(),
            created_at: "2026-06-12T14:00:00Z".to_string(),
        })
        .expect_err("self-intersecting boundary should be rejected");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "bowtie-boundary.geojson".to_string(),
                reason: InteropRejectionReason::InvalidFieldBoundary {
                    reason: FieldBoundaryRejectionReason::SelfIntersection
                }
            }
        );
    }

    #[test]
    fn prescription_shapefile_exports_zone_rates_in_field_crs() {
        let report = export_prescription_shapefile(prescription_request())
            .expect("aligned prescription zones should export");

        assert_eq!(report.prescription_id, "rx-alpha-2026");
        assert_eq!(report.field_id, "field-alpha");
        assert_eq!(report.field_crs, "EPSG:32614");
        assert_eq!(report.zone_count, 2);
        assert_eq!(report.rate_attribute, "RATE");
        assert_eq!(report.unit_attribute, "UNIT");
        assert_eq!(
            report.extent,
            InteropExtent {
                min_x: 500_000.0,
                min_y: 4_499_980.0,
                max_x: 500_020.0,
                max_y: 4_500_000.0,
            }
        );
        assert_eq!(&report.files.shp[0..4], &9994i32.to_be_bytes());
        assert_eq!(&report.files.shx[0..4], &9994i32.to_be_bytes());
        assert_shapefile_bundle_consistent(&report.files, 2);
        let first_polygon = first_shp_polygon(&report.files.shp);
        assert!(signed_area_for_test(&first_polygon) < 0.0);
        assert_eq!(dbf_record_count(&report.files.dbf), 2);
        let dbf_text = String::from_utf8_lossy(&report.files.dbf);
        assert!(dbf_text.contains("zone-west"));
        assert!(dbf_text.contains("zone-east"));
        assert!(dbf_text.contains("32.500000"));
        assert!(dbf_text.contains("12.250000"));
        assert!(dbf_text.contains("kg_ha"));
        let prj_text = String::from_utf8(report.files.prj).expect("prj should be utf8");
        assert!(prj_text.contains("32614"));
    }

    #[test]
    fn prescription_shapefile_refuses_overlapping_zones() {
        let mut request = prescription_request();
        request.zones[0].polygon = rectangle(500_000.0, 4_499_980.0, 500_014.0, 4_500_000.0);
        request.zones[1].polygon = rectangle(500_010.0, 4_499_980.0, 500_020.0, 4_500_000.0);

        let error =
            export_prescription_shapefile(request).expect_err("overlap should refuse export");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "rx-alpha-2026".to_string(),
                reason: InteropRejectionReason::InvalidPrescription {
                    reason: PrescriptionRejectionReason::OverlappingZones {
                        left_zone_id: "zone-west".to_string(),
                        right_zone_id: "zone-east".to_string(),
                    }
                }
            }
        );
    }

    #[test]
    fn prescription_shapefile_refuses_coverage_gap() {
        let mut request = prescription_request();
        request.zones.pop();

        let error = export_prescription_shapefile(request)
            .expect_err("zones that do not tile the field should refuse export");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "rx-alpha-2026".to_string(),
                reason: InteropRejectionReason::InvalidPrescription {
                    reason: PrescriptionRejectionReason::ZoneCoverageGap
                }
            }
        );
    }

    #[test]
    fn prescription_shapefile_refuses_zone_edge_that_leaves_concave_field() {
        let request = PrescriptionShapefileRequest {
            prescription_id: "rx-concave".to_string(),
            field: PrescriptionField {
                field_id: "field-concave".to_string(),
                crs: "EPSG:32614".to_string(),
                boundary: vec![
                    InteropCoordinate { x: 0.0, y: 4.0 },
                    InteropCoordinate { x: 2.0, y: 4.0 },
                    InteropCoordinate { x: 2.0, y: 2.0 },
                    InteropCoordinate { x: 4.0, y: 2.0 },
                    InteropCoordinate { x: 4.0, y: 0.0 },
                    InteropCoordinate { x: 0.0, y: 0.0 },
                    InteropCoordinate { x: 0.0, y: 4.0 },
                ],
            },
            zones: vec![PrescriptionZone {
                zone_id: "zone-diagonal".to_string(),
                polygon: vec![
                    InteropCoordinate { x: 0.0, y: 4.0 },
                    InteropCoordinate { x: 4.0, y: 2.0 },
                    InteropCoordinate { x: 4.0, y: 0.0 },
                    InteropCoordinate { x: 0.0, y: 0.0 },
                    InteropCoordinate { x: 0.0, y: 4.0 },
                ],
                crs: "EPSG:32614".to_string(),
                rate: 20.0,
                unit: "kg_ha".to_string(),
            }],
        };

        let error = export_prescription_shapefile(request)
            .expect_err("edge crossing outside concave field should refuse export");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "rx-concave".to_string(),
                reason: InteropRejectionReason::InvalidPrescription {
                    reason: PrescriptionRejectionReason::ZoneOutsideField {
                        zone_id: "zone-diagonal".to_string(),
                    }
                }
            }
        );
    }

    #[test]
    fn prescription_shapefile_refuses_rate_that_cannot_fit_dbf_field() {
        let mut request = prescription_request();
        request.zones[0].rate = 1_000_000_000_000.0;

        let error =
            export_prescription_shapefile(request).expect_err("oversized rate should be refused");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "rx-alpha-2026".to_string(),
                reason: InteropRejectionReason::InvalidPrescription {
                    reason: PrescriptionRejectionReason::InvalidRate {
                        zone_id: "zone-west".to_string(),
                    }
                }
            }
        );
    }

    #[test]
    fn prescription_shapefile_refuses_zone_crs_mismatch() {
        let mut request = prescription_request();
        request.zones[1].crs = "EPSG:4326".to_string();

        let error =
            export_prescription_shapefile(request).expect_err("CRS mismatch should refuse export");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "rx-alpha-2026".to_string(),
                reason: InteropRejectionReason::InvalidPrescription {
                    reason: PrescriptionRejectionReason::CrsMismatch {
                        zone_id: "zone-east".to_string(),
                        expected_crs: "EPSG:32614".to_string(),
                        actual_crs: "EPSG:4326".to_string(),
                    }
                }
            }
        );
    }

    #[test]
    fn prescription_taskdata_exports_schema_valid_isobus_xml() {
        let report = export_prescription_taskdata(prescription_request())
            .expect("aligned prescription zones should export as TaskData");

        assert_eq!(report.prescription_id, "rx-alpha-2026");
        assert_eq!(report.field_id, "field-alpha");
        assert_eq!(report.field_crs, "EPSG:32614");
        assert_eq!(report.zone_count, 2);
        assert_eq!(report.unit_designator, "kg/ha");
        assert!(report.validation.valid);
        assert_eq!(report.validation.task_count, 1);
        assert_eq!(report.validation.zone_count, 2);
        assert_eq!(report.validation.product_count, 1);
        assert_eq!(report.validation.prescription_grid_count, 1);
        let xml = String::from_utf8(report.taskdata_xml.clone()).expect("TaskData should be utf8");
        assert!(xml.contains("<ISO11783_TaskData"));
        assert!(xml.contains("<TSK "));
        assert!(xml.contains("<TZN "));
        assert!(xml.contains("<PDT "));
        assert!(xml.contains("<PGP "));
        assert!(xml.contains("EPSG:32614"));
        assert!(xml.contains("32.500000"));
        assert!(xml.contains("12.250000"));
        assert!(xml.contains("kg/ha"));

        let validation = validate_taskdata_xml(&report.taskdata_xml)
            .expect("emitted TaskData should validate independently");
        assert!(validation.valid);
        assert_eq!(validation.zone_count, 2);
    }

    #[test]
    fn prescription_taskdata_refuses_unit_without_isobus_mapping() {
        let mut request = prescription_request();
        request.zones[0].unit = "bushel_ac".to_string();

        let error =
            export_prescription_taskdata(request).expect_err("unsupported unit should refuse XML");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "rx-alpha-2026".to_string(),
                reason: InteropRejectionReason::InvalidPrescription {
                    reason: PrescriptionRejectionReason::UnsupportedTaskDataUnit {
                        zone_id: "zone-west".to_string(),
                        unit: "bushel_ac".to_string(),
                    }
                }
            }
        );
    }

    #[test]
    fn prescription_taskdata_refuses_zone_outside_field() {
        let mut request = prescription_request();
        request.zones[1].polygon = rectangle(500_010.0, 4_499_980.0, 500_030.0, 4_500_000.0);

        let error =
            export_prescription_taskdata(request).expect_err("out-of-field zone should refuse XML");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "rx-alpha-2026".to_string(),
                reason: InteropRejectionReason::InvalidPrescription {
                    reason: PrescriptionRejectionReason::ZoneOutsideField {
                        zone_id: "zone-east".to_string(),
                    }
                }
            }
        );
    }

    #[test]
    fn taskdata_schema_validation_rejects_missing_prescription_grid() {
        let report = export_prescription_taskdata(prescription_request())
            .expect("aligned prescription zones should export as TaskData");
        let mut xml = String::from_utf8(report.taskdata_xml).expect("TaskData should be utf8");
        xml = xml.replace("  <PGP A=\"PGP1\" B=\"PDT1\" C=\"kg/ha\" />\n", "");

        let error = validate_taskdata_xml(xml.as_bytes())
            .expect_err("missing PGP should fail schema validation");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "TASKDATA.XML".to_string(),
                reason: InteropRejectionReason::InvalidPrescription {
                    reason: PrescriptionRejectionReason::InvalidTaskDataSchema {
                        reason: "missing PGP prescription grid".to_string(),
                    }
                }
            }
        );
    }

    #[test]
    fn taskdata_schema_validation_rejects_malformed_or_fake_xml() {
        let error = validate_taskdata_xml(
            b"<not-taskdata><ISO11783_TaskData /><TSK /><TZN /><PDT /><PGP Crs=\"EPSG:32614\" />",
        )
        .expect_err("fake token document should fail validation");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "TASKDATA.XML".to_string(),
                reason: InteropRejectionReason::InvalidPrescription {
                    reason: PrescriptionRejectionReason::InvalidTaskDataSchema {
                        reason: "missing ISO11783_TaskData root".to_string(),
                    }
                }
            }
        );
    }

    #[test]
    fn taskdata_schema_validation_rejects_inconsistent_crs_or_units() {
        let report = export_prescription_taskdata(prescription_request())
            .expect("aligned prescription zones should export as TaskData");
        let mut xml =
            String::from_utf8(report.taskdata_xml.clone()).expect("TaskData should be utf8");
        xml = xml.replacen("Crs=\"EPSG:32614\"", "Crs=\"EPSG:4326\"", 1);

        let error = validate_taskdata_xml(xml.as_bytes())
            .expect_err("conflicting CRS should fail schema validation");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "TASKDATA.XML".to_string(),
                reason: InteropRejectionReason::InvalidPrescription {
                    reason: PrescriptionRejectionReason::InvalidTaskDataSchema {
                        reason: "inconsistent CRS declarations".to_string(),
                    }
                }
            }
        );

        let mut xml = String::from_utf8(report.taskdata_xml).expect("TaskData should be utf8");
        xml = xml.replacen("D=\"kg/ha\"", "D=\"l/ha\"", 1);
        let error = validate_taskdata_xml(xml.as_bytes())
            .expect_err("conflicting zone unit should fail schema validation");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "TASKDATA.XML".to_string(),
                reason: InteropRejectionReason::InvalidPrescription {
                    reason: PrescriptionRejectionReason::InvalidTaskDataSchema {
                        reason: "inconsistent TaskData units".to_string(),
                    }
                }
            }
        );
    }

    #[test]
    fn john_deere_connector_pushes_prescription_after_retry_with_mapping() {
        let mut endpoint = FakeJohnDeereEndpoint::new()
            .with_push_error("503 transient")
            .with_push_success("jd-rx-001");

        let report = push_john_deere_prescription(
            &mut endpoint,
            JohnDeerePrescriptionPushRequest {
                remote_field_id: "jd-field-alpha".to_string(),
                prescription: john_deere_prescription_request(),
            },
            JohnDeereRetryPolicy {
                max_attempts: 3,
                backoff_millis: vec![100, 250],
            },
        )
        .expect("connector should retry and upload");

        assert_eq!(report.remote_id, "jd-rx-001");
        assert_eq!(report.attempts, 2);
        assert_eq!(report.backoff_millis, vec![100]);
        assert_eq!(endpoint.backoffs, vec![100]);
        assert_eq!(endpoint.uploads.len(), 2);
        let payload = endpoint.uploads.last().expect("payload should be sent");
        assert_eq!(payload.remote_field_id, "jd-field-alpha");
        assert_eq!(payload.prescription_id, "rx-alpha-2026");
        assert_eq!(payload.crs, "EPSG:4326");
        assert_eq!(payload.unit_designator, "kg/ha");
        assert_eq!(payload.zone_count, 2);
        assert_eq!(payload.rates, vec![32.5, 12.25]);
        assert!(String::from_utf8(payload.taskdata_xml.clone())
            .expect("TaskData XML")
            .contains("<ISO11783_TaskData"));
    }

    #[test]
    fn john_deere_connector_surfaces_endpoint_failure_after_retries() {
        let mut endpoint = FakeJohnDeereEndpoint::new()
            .with_push_error("503 transient")
            .with_push_error("500 still down");

        let error = push_john_deere_prescription(
            &mut endpoint,
            JohnDeerePrescriptionPushRequest {
                remote_field_id: "jd-field-alpha".to_string(),
                prescription: john_deere_prescription_request(),
            },
            JohnDeereRetryPolicy {
                max_attempts: 2,
                backoff_millis: vec![50],
            },
        )
        .expect_err("exhausted endpoint errors should surface");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "john-deere-operations-center".to_string(),
                reason: InteropRejectionReason::JohnDeereConnector {
                    reason: JohnDeereConnectorError::EndpointFailed {
                        attempts: 2,
                        message: "500 still down".to_string(),
                    }
                }
            }
        );
        assert_eq!(endpoint.uploads.len(), 2);
        assert_eq!(endpoint.backoffs, vec![50]);
    }

    #[test]
    fn john_deere_connector_refuses_prescription_push_with_unsupported_crs() {
        let mut request = prescription_request();
        request.field.crs = "EPSG:32614".to_string();
        request.zones[0].crs = "EPSG:32614".to_string();
        request.zones[1].crs = "EPSG:32614".to_string();
        let mut endpoint = FakeJohnDeereEndpoint::new().with_push_success("jd-rx-001");

        let error = push_john_deere_prescription(
            &mut endpoint,
            JohnDeerePrescriptionPushRequest {
                remote_field_id: "jd-field-alpha".to_string(),
                prescription: request,
            },
            JohnDeereRetryPolicy {
                max_attempts: 1,
                backoff_millis: Vec::new(),
            },
        )
        .expect_err("unsupported JD push CRS should refuse before upload");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "john-deere-operations-center".to_string(),
                reason: InteropRejectionReason::JohnDeereConnector {
                    reason: JohnDeereConnectorError::UnsupportedPrescriptionCrs {
                        crs: "EPSG:32614".to_string(),
                    }
                }
            }
        );
        assert!(endpoint.uploads.is_empty());
    }

    #[test]
    fn john_deere_connector_pulls_boundaries_with_valid_crs_mapping() {
        let mut endpoint = FakeJohnDeereEndpoint::new().with_boundary(JohnDeereBoundary {
            remote_field_id: "jd-field-alpha".to_string(),
            name: "North Block".to_string(),
            crs: "EPSG:4326".to_string(),
            boundary: vec![
                InteropCoordinate {
                    x: -121.0,
                    y: 39.01,
                },
                InteropCoordinate {
                    x: -120.99,
                    y: 39.01,
                },
                InteropCoordinate {
                    x: -120.99,
                    y: 39.0,
                },
                InteropCoordinate { x: -121.0, y: 39.0 },
                InteropCoordinate {
                    x: -121.0,
                    y: 39.01,
                },
            ],
        });

        let report =
            pull_john_deere_boundaries(&mut endpoint).expect("valid JD boundary should map");

        assert_eq!(report.boundaries.len(), 1);
        assert_eq!(report.boundaries[0].remote_field_id, "jd-field-alpha");
        assert_eq!(report.boundaries[0].target_crs, "EPSG:4326");
        assert_eq!(report.boundaries[0].feature_count, 1);
        assert!(report.boundaries[0].extent.min_x < report.boundaries[0].extent.max_x);
    }

    #[test]
    fn john_deere_connector_refuses_boundary_with_unsupported_crs() {
        let mut endpoint = FakeJohnDeereEndpoint::new().with_boundary(JohnDeereBoundary {
            remote_field_id: "jd-field-oblique".to_string(),
            name: "Bad CRS".to_string(),
            crs: "OBLIQUE:LOCAL".to_string(),
            boundary: rectangle(0.0, 0.0, 1.0, 1.0),
        });

        let error = pull_john_deere_boundaries(&mut endpoint)
            .expect_err("unsupported CRS should refuse pulled boundary");

        assert_eq!(
            error,
            InteropError::Rejected {
                filename: "john-deere-operations-center".to_string(),
                reason: InteropRejectionReason::JohnDeereConnector {
                    reason: JohnDeereConnectorError::UnsupportedBoundaryCrs {
                        remote_field_id: "jd-field-oblique".to_string(),
                        crs: "OBLIQUE:LOCAL".to_string(),
                    }
                }
            }
        );
    }

    #[derive(Default)]
    struct FakeJohnDeereEndpoint {
        push_results: Vec<Result<RemotePrescriptionReceipt, JohnDeereEndpointError>>,
        boundaries: Vec<JohnDeereBoundary>,
        uploads: Vec<JohnDeereUploadPayload>,
        backoffs: Vec<u64>,
    }

    impl FakeJohnDeereEndpoint {
        fn new() -> Self {
            Self::default()
        }

        fn with_push_error(mut self, message: &str) -> Self {
            self.push_results
                .push(Err(JohnDeereEndpointError::new(message)));
            self
        }

        fn with_push_success(mut self, remote_id: &str) -> Self {
            self.push_results.push(Ok(RemotePrescriptionReceipt {
                remote_id: remote_id.to_string(),
            }));
            self
        }

        fn with_boundary(mut self, boundary: JohnDeereBoundary) -> Self {
            self.boundaries.push(boundary);
            self
        }
    }

    impl JohnDeereConnectorEndpoint for FakeJohnDeereEndpoint {
        fn push_prescription(
            &mut self,
            payload: JohnDeereUploadPayload,
        ) -> Result<RemotePrescriptionReceipt, JohnDeereEndpointError> {
            self.uploads.push(payload);
            if self.push_results.is_empty() {
                return Err(JohnDeereEndpointError::new("no fake response configured"));
            }
            self.push_results.remove(0)
        }

        fn pull_boundaries(&mut self) -> Result<Vec<JohnDeereBoundary>, JohnDeereEndpointError> {
            Ok(self.boundaries.clone())
        }

        fn wait_backoff(&mut self, millis: u64) {
            self.backoffs.push(millis);
        }
    }

    fn prescription_request() -> PrescriptionShapefileRequest {
        PrescriptionShapefileRequest {
            prescription_id: "rx-alpha-2026".to_string(),
            field: PrescriptionField {
                field_id: "field-alpha".to_string(),
                crs: "EPSG:32614".to_string(),
                boundary: rectangle(500_000.0, 4_499_980.0, 500_020.0, 4_500_000.0),
            },
            zones: vec![
                PrescriptionZone {
                    zone_id: "zone-west".to_string(),
                    polygon: rectangle(500_000.0, 4_499_980.0, 500_010.0, 4_500_000.0),
                    crs: "EPSG:32614".to_string(),
                    rate: 32.5,
                    unit: "kg_ha".to_string(),
                },
                PrescriptionZone {
                    zone_id: "zone-east".to_string(),
                    polygon: rectangle(500_010.0, 4_499_980.0, 500_020.0, 4_500_000.0),
                    crs: "EPSG:32614".to_string(),
                    rate: 12.25,
                    unit: "kg_ha".to_string(),
                },
            ],
        }
    }

    fn john_deere_prescription_request() -> PrescriptionShapefileRequest {
        PrescriptionShapefileRequest {
            prescription_id: "rx-alpha-2026".to_string(),
            field: PrescriptionField {
                field_id: "field-alpha".to_string(),
                crs: "EPSG:4326".to_string(),
                boundary: rectangle(-121.0, 39.0, -120.98, 39.02),
            },
            zones: vec![
                PrescriptionZone {
                    zone_id: "zone-west".to_string(),
                    polygon: rectangle(-121.0, 39.0, -120.99, 39.02),
                    crs: "EPSG:4326".to_string(),
                    rate: 32.5,
                    unit: "kg_ha".to_string(),
                },
                PrescriptionZone {
                    zone_id: "zone-east".to_string(),
                    polygon: rectangle(-120.99, 39.0, -120.98, 39.02),
                    crs: "EPSG:4326".to_string(),
                    rate: 12.25,
                    unit: "kg_ha".to_string(),
                },
            ],
        }
    }

    fn rectangle(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Vec<InteropCoordinate> {
        vec![
            InteropCoordinate { x: min_x, y: max_y },
            InteropCoordinate { x: max_x, y: max_y },
            InteropCoordinate { x: max_x, y: min_y },
            InteropCoordinate { x: min_x, y: min_y },
            InteropCoordinate { x: min_x, y: max_y },
        ]
    }

    fn dbf_record_count(bytes: &[u8]) -> u32 {
        u32::from_le_bytes(bytes[4..8].try_into().expect("dbf count bytes"))
    }

    fn assert_shapefile_files_consistent(
        shp: &[u8],
        shx: &[u8],
        dbf: &[u8],
        prj: &[u8],
        records: u32,
    ) {
        assert_eq!(be_i32(shp, 24) as usize * 2, shp.len());
        assert_eq!(be_i32(shx, 24) as usize * 2, shx.len());
        assert_eq!(le_i32(shp, 32), 5);
        assert_eq!(le_i32(shx, 32), 5);
        assert_eq!(shx.len(), 100 + records as usize * 8);
        assert_eq!(dbf_record_count(dbf), records);
        assert!(String::from_utf8_lossy(prj).contains("WGS") || !prj.is_empty());
        let dbf_header_len = le_u16(dbf, 8) as usize;
        let dbf_record_len = le_u16(dbf, 10) as usize;
        assert_eq!(
            dbf.len(),
            dbf_header_len + dbf_record_len * records as usize + 1
        );
    }

    fn assert_shapefile_bundle_consistent(files: &super::PrescriptionShapefileFiles, records: u32) {
        assert_eq!(be_i32(&files.shp, 24) as usize * 2, files.shp.len());
        assert_eq!(be_i32(&files.shx, 24) as usize * 2, files.shx.len());
        assert_eq!(le_i32(&files.shp, 32), 5);
        assert_eq!(le_i32(&files.shx, 32), 5);
        assert_eq!(files.shx.len(), 100 + records as usize * 8);
        assert_eq!(dbf_record_count(&files.dbf), records);
        let dbf_header_len = le_u16(&files.dbf, 8) as usize;
        let dbf_record_len = le_u16(&files.dbf, 10) as usize;
        assert_eq!(
            files.dbf.len(),
            dbf_header_len + dbf_record_len * records as usize + 1
        );

        let mut shp_offset_words = 50i32;
        for index in 0..records as usize {
            let shx_offset = 100 + index * 8;
            let record_offset = (shp_offset_words as usize) * 2;
            let content_length_words = be_i32(&files.shx, shx_offset + 4);
            assert_eq!(be_i32(&files.shx, shx_offset), shp_offset_words);
            assert_eq!(be_i32(&files.shp, record_offset), (index + 1) as i32);
            assert_eq!(be_i32(&files.shp, record_offset + 4), content_length_words);
            assert_eq!(le_i32(&files.shp, record_offset + 8), 5);
            shp_offset_words += 4 + content_length_words;
        }
        assert_eq!(shp_offset_words as usize * 2, files.shp.len());
    }

    fn findings_request(findings: Vec<FindingsExportFeature>) -> FindingsExportRequest {
        FindingsExportRequest {
            export_id: "findings-export".to_string(),
            crs: "EPSG:32614".to_string(),
            findings,
        }
    }

    fn finding_feature(finding_id: &str) -> FindingsExportFeature {
        FindingsExportFeature {
            finding_id: finding_id.to_string(),
            zone_id: "zone-1".to_string(),
            reason: "below_absolute_threshold".to_string(),
            priority: "high".to_string(),
            area_m2: 3_000.0,
            centroid: InteropCoordinate {
                x: 500_010.0,
                y: 4_500_015.0,
            },
            crs: "EPSG:32614".to_string(),
            polygon: vec![
                InteropCoordinate {
                    x: 500_000.0,
                    y: 4_500_020.0,
                },
                InteropCoordinate {
                    x: 500_020.0,
                    y: 4_500_020.0,
                },
                InteropCoordinate {
                    x: 500_020.0,
                    y: 4_500_010.0,
                },
                InteropCoordinate {
                    x: 500_000.0,
                    y: 4_500_010.0,
                },
                InteropCoordinate {
                    x: 500_000.0,
                    y: 4_500_020.0,
                },
            ],
            evidence_refs: vec!["zone:zone-1".to_string(), "layer:ndvi".to_string()],
        }
    }

    fn first_shp_polygon(bytes: &[u8]) -> Vec<InteropCoordinate> {
        let record = 108;
        let point_count = le_i32(bytes, record + 40) as usize;
        let points_offset = record + 48;
        (0..point_count)
            .map(|index| {
                let offset = points_offset + index * 16;
                InteropCoordinate {
                    x: le_f64(bytes, offset),
                    y: le_f64(bytes, offset + 8),
                }
            })
            .collect()
    }

    fn signed_area_for_test(points: &[InteropCoordinate]) -> f64 {
        points
            .windows(2)
            .map(|window| window[0].x * window[1].y - window[1].x * window[0].y)
            .sum::<f64>()
            * 0.5
    }

    fn be_i32(bytes: &[u8], offset: usize) -> i32 {
        i32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("be i32"))
    }

    fn le_i32(bytes: &[u8], offset: usize) -> i32 {
        i32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("le i32"))
    }

    fn le_u16(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("le u16"))
    }

    fn le_f64(bytes: &[u8], offset: usize) -> f64 {
        f64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("le f64"))
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

    fn bowtie_geojson(crs: &str) -> String {
        format!(
            r#"{{
                "type": "FeatureCollection",
                "crs": {{
                    "type": "name",
                    "properties": {{ "name": "{crs}" }}
                }},
                "features": [{{
                    "type": "Feature",
                    "properties": {{ "zone": "invalid" }},
                    "geometry": {{
                        "type": "Polygon",
                        "coordinates": [[
                            [-121.0000, 39.0000],
                            [-120.9900, 39.0100],
                            [-120.9900, 39.0000],
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
