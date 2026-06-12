use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceRecordType {
    AirspaceZone,
    AuthorizationDecision,
    ChemicalApplication,
    ComplianceReport,
    OperatorCertification,
    RemoteIdLog,
    FlightLog,
}

impl ComplianceRecordType {
    pub fn as_str(self) -> &'static str {
        match self {
            ComplianceRecordType::AirspaceZone => "airspace_zone",
            ComplianceRecordType::AuthorizationDecision => "authorization_decision",
            ComplianceRecordType::ChemicalApplication => "chemical_application",
            ComplianceRecordType::ComplianceReport => "compliance_report",
            ComplianceRecordType::OperatorCertification => "operator_certification",
            ComplianceRecordType::RemoteIdLog => "remote_id_log",
            ComplianceRecordType::FlightLog => "flight_log",
        }
    }
}

impl std::str::FromStr for ComplianceRecordType {
    type Err = ComplianceRecordError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "airspace_zone" => Ok(Self::AirspaceZone),
            "authorization_decision" => Ok(Self::AuthorizationDecision),
            "chemical_application" => Ok(Self::ChemicalApplication),
            "compliance_report" => Ok(Self::ComplianceReport),
            "operator_certification" => Ok(Self::OperatorCertification),
            "remote_id_log" => Ok(Self::RemoteIdLog),
            "flight_log" => Ok(Self::FlightLog),
            _ => Err(ComplianceRecordError::UnsupportedRecordType {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AirspaceZoneClass {
    Advisory,
    Controlled,
    NoFly,
    Restricted,
    TemporaryFlightRestriction,
}

impl AirspaceZoneClass {
    pub fn as_str(self) -> &'static str {
        match self {
            AirspaceZoneClass::Advisory => "advisory",
            AirspaceZoneClass::Controlled => "controlled",
            AirspaceZoneClass::NoFly => "no_fly",
            AirspaceZoneClass::Restricted => "restricted",
            AirspaceZoneClass::TemporaryFlightRestriction => "temporary_flight_restriction",
        }
    }
}

impl std::str::FromStr for AirspaceZoneClass {
    type Err = AirspaceZoneError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "advisory" => Ok(Self::Advisory),
            "controlled" => Ok(Self::Controlled),
            "no_fly" => Ok(Self::NoFly),
            "restricted" => Ok(Self::Restricted),
            "temporary_flight_restriction" | "tfr" => Ok(Self::TemporaryFlightRestriction),
            _ => Err(AirspaceZoneError::UnsupportedZoneClass {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AirspaceCoordinate {
    pub longitude: f64,
    pub latitude: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AirspaceZoneExtent {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AirspaceZoneIngestRequest {
    #[serde(default)]
    pub zone_id: Option<String>,
    pub zone_class: AirspaceZoneClass,
    #[serde(default)]
    pub crs: String,
    #[serde(default)]
    pub coordinates: Vec<AirspaceCoordinate>,
    #[serde(default)]
    pub effective_from: String,
    #[serde(default)]
    pub effective_to: Option<String>,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AirspaceZoneRecord {
    pub zone_id: String,
    pub zone_class: AirspaceZoneClass,
    pub crs: String,
    pub coordinates: Vec<AirspaceCoordinate>,
    pub extent: AirspaceZoneExtent,
    pub effective_from: String,
    pub effective_to: Option<String>,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CreateComplianceRecordRequest {
    #[serde(default)]
    pub record_id: Option<String>,
    pub record_type: ComplianceRecordType,
    #[serde(default)]
    pub org_id: String,
    #[serde(default)]
    pub field_id: String,
    #[serde(default)]
    pub flight_id: Option<String>,
    #[serde(default)]
    pub actor: String,
    #[serde(default)]
    pub provenance_ref: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AppendComplianceRecordVersionRequest {
    #[serde(default)]
    pub field_id: Option<String>,
    #[serde(default)]
    pub flight_id: Option<String>,
    #[serde(default)]
    pub actor: String,
    #[serde(default)]
    pub provenance_ref: String,
    #[serde(default)]
    pub change_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplianceRecord {
    pub record_id: String,
    pub version: u32,
    pub record_type: ComplianceRecordType,
    pub org_id: String,
    pub field_id: String,
    pub flight_id: Option<String>,
    pub created_at: String,
    pub actor: String,
    pub provenance_ref: String,
    pub prior_version: Option<u32>,
    pub change_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ComplianceRecordError {
    #[error("record_id cannot be empty")]
    EmptyRecordId,
    #[error("org_id cannot be empty")]
    EmptyOrgId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("actor cannot be empty")]
    EmptyActor,
    #[error("provenance_ref cannot be empty")]
    EmptyProvenanceRef,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("unsupported compliance record type {value}")]
    UnsupportedRecordType { value: String },
    #[error("compliance records are append-only; {action} must create a new version")]
    AppendOnlyMutationRefused { action: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AirspaceZoneError {
    #[error("zone_id cannot be empty")]
    EmptyZoneId,
    #[error("airspace zone CRS cannot be empty")]
    EmptyCrs,
    #[error("unsupported airspace zone CRS {value}; expected EPSG:4326")]
    UnsupportedCrs { value: String },
    #[error("airspace zone polygon must contain at least four coordinates including closure")]
    TooFewCoordinates,
    #[error("airspace zone polygon must be closed")]
    UnclosedPolygon,
    #[error("airspace coordinate is outside longitude/latitude bounds")]
    InvalidCoordinate,
    #[error("effective_from cannot be empty")]
    EmptyEffectiveFrom,
    #[error("source cannot be empty")]
    EmptySource,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("unsupported airspace zone class {value}")]
    UnsupportedZoneClass { value: String },
}

pub fn build_initial_compliance_record(
    request: CreateComplianceRecordRequest,
    generated_record_id: String,
    created_at: String,
) -> Result<ComplianceRecord, ComplianceRecordError> {
    let record_id = match normalize_optional_text(request.record_id) {
        Some(record_id) => record_id,
        None => normalize_required_text(generated_record_id, ComplianceRecordError::EmptyRecordId)?,
    };

    Ok(ComplianceRecord {
        record_id,
        version: 1,
        record_type: request.record_type,
        org_id: normalize_required_text(request.org_id, ComplianceRecordError::EmptyOrgId)?,
        field_id: normalize_required_text(request.field_id, ComplianceRecordError::EmptyFieldId)?,
        flight_id: normalize_optional_text(request.flight_id),
        created_at: normalize_required_text(created_at, ComplianceRecordError::EmptyCreatedAt)?,
        actor: normalize_required_text(request.actor, ComplianceRecordError::EmptyActor)?,
        provenance_ref: normalize_required_text(
            request.provenance_ref,
            ComplianceRecordError::EmptyProvenanceRef,
        )?,
        prior_version: None,
        change_reason: None,
    })
}

pub fn append_compliance_record_version(
    latest: &ComplianceRecord,
    request: AppendComplianceRecordVersionRequest,
    created_at: String,
) -> Result<ComplianceRecord, ComplianceRecordError> {
    let field_id =
        normalize_optional_text(request.field_id).unwrap_or_else(|| latest.field_id.clone());
    let flight_id = match normalize_optional_text(request.flight_id) {
        Some(flight_id) => Some(flight_id),
        None => latest.flight_id.clone(),
    };

    Ok(ComplianceRecord {
        record_id: latest.record_id.clone(),
        version: latest.version + 1,
        record_type: latest.record_type,
        org_id: latest.org_id.clone(),
        field_id,
        flight_id,
        created_at: normalize_required_text(created_at, ComplianceRecordError::EmptyCreatedAt)?,
        actor: normalize_required_text(request.actor, ComplianceRecordError::EmptyActor)?,
        provenance_ref: normalize_required_text(
            request.provenance_ref,
            ComplianceRecordError::EmptyProvenanceRef,
        )?,
        prior_version: Some(latest.version),
        change_reason: normalize_optional_text(request.change_reason),
    })
}

pub fn build_airspace_zone_record(
    request: AirspaceZoneIngestRequest,
    generated_zone_id: String,
    created_at: String,
) -> Result<AirspaceZoneRecord, AirspaceZoneError> {
    let zone_id = match normalize_optional_text(request.zone_id) {
        Some(zone_id) => zone_id,
        None => {
            normalize_required_airspace_text(generated_zone_id, AirspaceZoneError::EmptyZoneId)?
        }
    };
    let coordinates = validate_airspace_polygon(request.coordinates)?;
    let extent = compute_airspace_extent(&coordinates)?;

    Ok(AirspaceZoneRecord {
        zone_id,
        zone_class: request.zone_class,
        crs: normalize_airspace_crs(request.crs)?,
        coordinates,
        extent,
        effective_from: normalize_required_airspace_text(
            request.effective_from,
            AirspaceZoneError::EmptyEffectiveFrom,
        )?,
        effective_to: normalize_optional_text(request.effective_to),
        source: normalize_required_airspace_text(request.source, AirspaceZoneError::EmptySource)?,
        created_at: normalize_required_airspace_text(
            created_at,
            AirspaceZoneError::EmptyCreatedAt,
        )?,
    })
}

pub fn airspace_zone_contains_point(zone: &AirspaceZoneRecord, point: AirspaceCoordinate) -> bool {
    point_in_polygon(point, &zone.coordinates)
}

pub fn airspace_zone_intersects_polygon(
    zone: &AirspaceZoneRecord,
    polygon: &[AirspaceCoordinate],
) -> Result<bool, AirspaceZoneError> {
    let polygon = validate_airspace_polygon(polygon.to_vec())?;

    if polygon
        .iter()
        .copied()
        .any(|point| point_in_polygon(point, &zone.coordinates))
    {
        return Ok(true);
    }
    if zone
        .coordinates
        .iter()
        .copied()
        .any(|point| point_in_polygon(point, &polygon))
    {
        return Ok(true);
    }

    for zone_edge in zone.coordinates.windows(2) {
        for area_edge in polygon.windows(2) {
            if segments_intersect(zone_edge[0], zone_edge[1], area_edge[0], area_edge[1]) {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

pub fn airspace_zone_is_effective_at(zone: &AirspaceZoneRecord, at: Option<&str>) -> bool {
    let Some(at) = at.and_then(|value| normalize_optional_text(Some(value.to_string()))) else {
        return true;
    };

    if at.as_str() < zone.effective_from.as_str() {
        return false;
    }
    match &zone.effective_to {
        Some(effective_to) => at.as_str() <= effective_to.as_str(),
        None => true,
    }
}

pub fn refuse_in_place_mutation(action: &str) -> ComplianceRecordError {
    ComplianceRecordError::AppendOnlyMutationRefused {
        action: action.trim().to_string(),
    }
}

fn normalize_required_text(
    value: String,
    error: ComplianceRecordError,
) -> Result<String, ComplianceRecordError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_required_airspace_text(
    value: String,
    error: AirspaceZoneError,
) -> Result<String, AirspaceZoneError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_airspace_crs(value: String) -> Result<String, AirspaceZoneError> {
    let crs = normalize_required_airspace_text(value, AirspaceZoneError::EmptyCrs)?;
    if crs.eq_ignore_ascii_case("EPSG:4326") {
        Ok("EPSG:4326".to_string())
    } else {
        Err(AirspaceZoneError::UnsupportedCrs { value: crs })
    }
}

fn validate_airspace_polygon(
    coordinates: Vec<AirspaceCoordinate>,
) -> Result<Vec<AirspaceCoordinate>, AirspaceZoneError> {
    if coordinates.len() < 4 {
        return Err(AirspaceZoneError::TooFewCoordinates);
    }
    for coordinate in &coordinates {
        if !coordinate.longitude.is_finite()
            || !coordinate.latitude.is_finite()
            || coordinate.longitude < -180.0
            || coordinate.longitude > 180.0
            || coordinate.latitude < -90.0
            || coordinate.latitude > 90.0
        {
            return Err(AirspaceZoneError::InvalidCoordinate);
        }
    }
    let first = coordinates[0];
    let last = coordinates[coordinates.len() - 1];
    if !same_coordinate(first, last) {
        return Err(AirspaceZoneError::UnclosedPolygon);
    }

    Ok(coordinates)
}

fn compute_airspace_extent(
    coordinates: &[AirspaceCoordinate],
) -> Result<AirspaceZoneExtent, AirspaceZoneError> {
    let coordinates = validate_airspace_polygon(coordinates.to_vec())?;
    let mut extent = AirspaceZoneExtent {
        min_lon: f64::INFINITY,
        min_lat: f64::INFINITY,
        max_lon: f64::NEG_INFINITY,
        max_lat: f64::NEG_INFINITY,
    };

    for coordinate in coordinates {
        extent.min_lon = extent.min_lon.min(coordinate.longitude);
        extent.min_lat = extent.min_lat.min(coordinate.latitude);
        extent.max_lon = extent.max_lon.max(coordinate.longitude);
        extent.max_lat = extent.max_lat.max(coordinate.latitude);
    }

    Ok(extent)
}

fn point_in_polygon(point: AirspaceCoordinate, polygon: &[AirspaceCoordinate]) -> bool {
    let mut inside = false;
    for edge in polygon.windows(2) {
        let a = edge[0];
        let b = edge[1];
        if point_on_segment(point, a, b) {
            return true;
        }

        let crosses_latitude = (a.latitude > point.latitude) != (b.latitude > point.latitude);
        if crosses_latitude {
            let intersect_lon = (b.longitude - a.longitude) * (point.latitude - a.latitude)
                / (b.latitude - a.latitude)
                + a.longitude;
            if point.longitude < intersect_lon {
                inside = !inside;
            }
        }
    }
    inside
}

fn segments_intersect(
    a: AirspaceCoordinate,
    b: AirspaceCoordinate,
    c: AirspaceCoordinate,
    d: AirspaceCoordinate,
) -> bool {
    let o1 = orientation(a, b, c);
    let o2 = orientation(a, b, d);
    let o3 = orientation(c, d, a);
    let o4 = orientation(c, d, b);

    if nearly_zero(o1) && point_on_segment(c, a, b) {
        return true;
    }
    if nearly_zero(o2) && point_on_segment(d, a, b) {
        return true;
    }
    if nearly_zero(o3) && point_on_segment(a, c, d) {
        return true;
    }
    if nearly_zero(o4) && point_on_segment(b, c, d) {
        return true;
    }

    (o1 > 0.0) != (o2 > 0.0) && (o3 > 0.0) != (o4 > 0.0)
}

fn orientation(a: AirspaceCoordinate, b: AirspaceCoordinate, c: AirspaceCoordinate) -> f64 {
    (b.longitude - a.longitude) * (c.latitude - a.latitude)
        - (b.latitude - a.latitude) * (c.longitude - a.longitude)
}

fn point_on_segment(
    point: AirspaceCoordinate,
    a: AirspaceCoordinate,
    b: AirspaceCoordinate,
) -> bool {
    nearly_zero(orientation(a, b, point))
        && point.longitude >= a.longitude.min(b.longitude) - 1e-9
        && point.longitude <= a.longitude.max(b.longitude) + 1e-9
        && point.latitude >= a.latitude.min(b.latitude) - 1e-9
        && point.latitude <= a.latitude.max(b.latitude) + 1e-9
}

fn same_coordinate(a: AirspaceCoordinate, b: AirspaceCoordinate) -> bool {
    (a.longitude - b.longitude).abs() <= 1e-9 && (a.latitude - b.latitude).abs() <= 1e-9
}

fn nearly_zero(value: f64) -> bool {
    value.abs() <= 1e-12
}

#[cfg(test)]
mod tests {
    use super::{
        airspace_zone_contains_point, airspace_zone_intersects_polygon,
        append_compliance_record_version, build_airspace_zone_record,
        build_initial_compliance_record, refuse_in_place_mutation, AirspaceCoordinate,
        AirspaceZoneClass, AirspaceZoneError, AirspaceZoneIngestRequest,
        AppendComplianceRecordVersionRequest, ComplianceRecordError, ComplianceRecordType,
        CreateComplianceRecordRequest,
    };

    #[test]
    fn initial_compliance_record_has_stable_identity_and_provenance() {
        let record = build_initial_compliance_record(
            CreateComplianceRecordRequest {
                record_id: Some(" comp-rec-1 ".to_string()),
                record_type: ComplianceRecordType::ChemicalApplication,
                org_id: " org-alpha ".to_string(),
                field_id: " field-north ".to_string(),
                flight_id: Some(" flight-77 ".to_string()),
                actor: " compliance-officer-1 ".to_string(),
                provenance_ref: " provenance:compliance/comp-rec-1/v1 ".to_string(),
            },
            "generated-record".to_string(),
            " 2026-06-12T12:00:00Z ".to_string(),
        )
        .expect("record should be valid");

        assert_eq!(record.record_id, "comp-rec-1");
        assert_eq!(record.version, 1);
        assert_eq!(
            record.record_type,
            ComplianceRecordType::ChemicalApplication
        );
        assert_eq!(record.org_id, "org-alpha");
        assert_eq!(record.field_id, "field-north");
        assert_eq!(record.flight_id.as_deref(), Some("flight-77"));
        assert_eq!(record.actor, "compliance-officer-1");
        assert_eq!(record.provenance_ref, "provenance:compliance/comp-rec-1/v1");
        assert_eq!(record.prior_version, None);
    }

    #[test]
    fn append_only_change_creates_next_version_and_retains_prior() {
        let initial = build_initial_compliance_record(
            CreateComplianceRecordRequest {
                record_id: Some("comp-rec-1".to_string()),
                record_type: ComplianceRecordType::ChemicalApplication,
                org_id: "org-alpha".to_string(),
                field_id: "field-north".to_string(),
                flight_id: Some("flight-77".to_string()),
                actor: "compliance-officer-1".to_string(),
                provenance_ref: "provenance:compliance/comp-rec-1/v1".to_string(),
            },
            "generated-record".to_string(),
            "2026-06-12T12:00:00Z".to_string(),
        )
        .expect("initial record should be valid");

        let appended = append_compliance_record_version(
            &initial,
            AppendComplianceRecordVersionRequest {
                field_id: Some("field-south".to_string()),
                flight_id: None,
                actor: "compliance-officer-2".to_string(),
                provenance_ref: "provenance:compliance/comp-rec-1/v2".to_string(),
                change_reason: Some("corrected field linkage".to_string()),
            },
            "2026-06-12T13:00:00Z".to_string(),
        )
        .expect("append should be valid");

        assert_eq!(appended.record_id, initial.record_id);
        assert_eq!(appended.version, 2);
        assert_eq!(appended.prior_version, Some(1));
        assert_eq!(appended.field_id, "field-south");
        assert_eq!(appended.flight_id.as_deref(), Some("flight-77"));
        assert_eq!(initial.field_id, "field-north");
    }

    #[test]
    fn missing_provenance_is_rejected() {
        let error = build_initial_compliance_record(
            CreateComplianceRecordRequest {
                record_id: Some("comp-rec-1".to_string()),
                record_type: ComplianceRecordType::FlightLog,
                org_id: "org-alpha".to_string(),
                field_id: "field-north".to_string(),
                flight_id: None,
                actor: "compliance-officer-1".to_string(),
                provenance_ref: " ".to_string(),
            },
            "generated-record".to_string(),
            "2026-06-12T12:00:00Z".to_string(),
        )
        .expect_err("missing provenance should be rejected");

        assert_eq!(error, ComplianceRecordError::EmptyProvenanceRef);
    }

    #[test]
    fn in_place_mutation_is_refused() {
        let error = refuse_in_place_mutation("delete");

        assert_eq!(
            error,
            ComplianceRecordError::AppendOnlyMutationRefused {
                action: "delete".to_string()
            }
        );
    }

    #[test]
    fn airspace_zone_record_asserts_crs_and_extent() {
        let zone = build_airspace_zone_record(
            AirspaceZoneIngestRequest {
                zone_id: Some(" nfz-1 ".to_string()),
                zone_class: AirspaceZoneClass::NoFly,
                crs: " epsg:4326 ".to_string(),
                coordinates: square_zone(),
                effective_from: " 2026-06-01T00:00:00Z ".to_string(),
                effective_to: Some(" 2026-07-01T00:00:00Z ".to_string()),
                source: " faa-uasfm-2026-06 ".to_string(),
            },
            "generated-zone".to_string(),
            " 2026-06-12T12:00:00Z ".to_string(),
        )
        .expect("zone should be valid");

        assert_eq!(zone.zone_id, "nfz-1");
        assert_eq!(zone.zone_class, AirspaceZoneClass::NoFly);
        assert_eq!(zone.crs, "EPSG:4326");
        assert_eq!(zone.extent.min_lon, -96.70);
        assert_eq!(zone.extent.max_lat, 41.40);
        assert_eq!(zone.source, "faa-uasfm-2026-06");
    }

    #[test]
    fn airspace_zone_point_and_area_membership_are_deterministic() {
        let zone = build_airspace_zone_record(
            AirspaceZoneIngestRequest {
                zone_id: Some("nfz-1".to_string()),
                zone_class: AirspaceZoneClass::NoFly,
                crs: "EPSG:4326".to_string(),
                coordinates: square_zone(),
                effective_from: "2026-06-01T00:00:00Z".to_string(),
                effective_to: None,
                source: "faa-uasfm-2026-06".to_string(),
            },
            "generated-zone".to_string(),
            "2026-06-12T12:00:00Z".to_string(),
        )
        .expect("zone should be valid");

        assert!(airspace_zone_contains_point(
            &zone,
            AirspaceCoordinate {
                longitude: -96.45,
                latitude: 41.20
            }
        ));
        assert!(!airspace_zone_contains_point(
            &zone,
            AirspaceCoordinate {
                longitude: -97.00,
                latitude: 41.20
            }
        ));
        assert!(airspace_zone_intersects_polygon(
            &zone,
            &[
                AirspaceCoordinate {
                    longitude: -96.50,
                    latitude: 41.20,
                },
                AirspaceCoordinate {
                    longitude: -96.10,
                    latitude: 41.20,
                },
                AirspaceCoordinate {
                    longitude: -96.10,
                    latitude: 41.50,
                },
                AirspaceCoordinate {
                    longitude: -96.50,
                    latitude: 41.20,
                },
            ],
        )
        .expect("area query should be valid"));
    }

    #[test]
    fn airspace_zone_rejects_unsupported_crs() {
        let error = build_airspace_zone_record(
            AirspaceZoneIngestRequest {
                zone_id: Some("nfz-1".to_string()),
                zone_class: AirspaceZoneClass::NoFly,
                crs: "EPSG:3857".to_string(),
                coordinates: square_zone(),
                effective_from: "2026-06-01T00:00:00Z".to_string(),
                effective_to: None,
                source: "bad-crs".to_string(),
            },
            "generated-zone".to_string(),
            "2026-06-12T12:00:00Z".to_string(),
        )
        .expect_err("unsupported CRS should be rejected");

        assert_eq!(
            error,
            AirspaceZoneError::UnsupportedCrs {
                value: "EPSG:3857".to_string()
            }
        );
    }

    fn square_zone() -> Vec<AirspaceCoordinate> {
        vec![
            AirspaceCoordinate {
                longitude: -96.70,
                latitude: 41.10,
            },
            AirspaceCoordinate {
                longitude: -96.20,
                latitude: 41.10,
            },
            AirspaceCoordinate {
                longitude: -96.20,
                latitude: 41.40,
            },
            AirspaceCoordinate {
                longitude: -96.70,
                latitude: 41.40,
            },
            AirspaceCoordinate {
                longitude: -96.70,
                latitude: 41.10,
            },
        ]
    }
}
