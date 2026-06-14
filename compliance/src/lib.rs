use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

impl std::fmt::Display for ComplianceRecordType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComplianceRecordPayload {
    RemoteIdFlightLog(RemoteIdFlightLogRecord),
    ChemicalApplication(ChemicalApplicationRecord),
}

impl ComplianceRecordPayload {
    fn payload_type(&self) -> &'static str {
        match self {
            ComplianceRecordPayload::RemoteIdFlightLog(_) => "remote_id_flight_log",
            ComplianceRecordPayload::ChemicalApplication(_) => "chemical_application",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteIdFlightLogRecord {
    pub flight_id: String,
    pub operator_id: String,
    pub aircraft_id: String,
    pub started_at: String,
    pub ended_at: String,
    #[serde(default)]
    pub track: Vec<RemoteIdTrackPoint>,
    #[serde(default)]
    pub telemetry_gaps: Vec<TelemetryGapRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteIdTrackPoint {
    pub observed_at: String,
    pub longitude: f64,
    pub latitude: f64,
    pub altitude_m: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryGapRecord {
    pub started_at: String,
    pub ended_at: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChemicalApplicationRecord {
    pub application_id: String,
    pub product: String,
    pub epa_or_label_ref: String,
    pub field_id: String,
    pub geometry: ApplicationGeometry,
    pub applied_at: String,
    pub rate: f64,
    pub units: String,
    pub operator_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApplicationGeometry {
    pub crs: String,
    #[serde(default)]
    pub coordinates: Vec<AirspaceCoordinate>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OperatorCertificationRegistrationRequest {
    #[serde(default)]
    pub cert_id: Option<String>,
    #[serde(default)]
    pub operator_id: String,
    #[serde(default)]
    pub cert_type: String,
    #[serde(default)]
    pub issued_at: String,
    #[serde(default)]
    pub expires_at: String,
    #[serde(default)]
    pub authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorCertificationRecord {
    pub cert_id: String,
    pub operator_id: String,
    pub cert_type: String,
    pub issued_at: String,
    pub expires_at: String,
    pub authority: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CertificationStatus {
    Valid,
    Expired,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CertificationBlockReason {
    ExpiredCertification,
    MissingCertification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificationCheckResult {
    pub operator_id: String,
    pub required_cert_type: String,
    pub checked_at: String,
    pub status: CertificationStatus,
    pub cert_id: Option<String>,
    pub expires_at: Option<String>,
    pub block_flight: bool,
    pub reason_code: Option<CertificationBlockReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightAirspaceStatus {
    Fresh,
    Missing,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationDecisionStatus {
    Permitted,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationBlockReason {
    NoFlyZoneIntersection,
    MissingAirspaceData,
    StaleAirspaceData,
    MissingCertification,
    ExpiredCertification,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreflightAuthorizationRequest {
    pub flight_id: String,
    pub operator_id: String,
    pub required_cert_type: String,
    pub planned_at: String,
    #[serde(default)]
    pub planned_area: Vec<AirspaceCoordinate>,
    pub airspace_status: PreflightAirspaceStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreflightAuthorizationDecision {
    pub flight_id: String,
    pub operator_id: String,
    pub checked_at: String,
    pub status: AuthorizationDecisionStatus,
    pub reason_code: Option<AuthorizationBlockReason>,
    pub zone_ref: Option<String>,
    pub cert_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProductLabelInterval {
    pub label_ref: String,
    pub rei_hours: Option<i64>,
    pub phi_days: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntervalWindowStatus {
    Known,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReiPhiWindow {
    pub application_id: String,
    pub label_ref: String,
    pub source_application_ref: String,
    pub applied_at: String,
    pub rei_status: IntervalWindowStatus,
    pub phi_status: IntervalWindowStatus,
    pub rei_clear_at: Option<String>,
    pub phi_clear_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryHarvestAction {
    ReEntry,
    Harvest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryHarvestDecisionStatus {
    Cleared,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryHarvestBlockReason {
    ReiActive,
    PhiActive,
    UnknownRei,
    UnknownPhi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryHarvestDecision {
    pub application_id: String,
    pub action: EntryHarvestAction,
    pub checked_at: String,
    pub status: EntryHarvestDecisionStatus,
    pub reason_code: Option<EntryHarvestBlockReason>,
    pub clear_at: Option<String>,
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
    #[serde(default)]
    pub payload: Option<ComplianceRecordPayload>,
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
    #[serde(default)]
    pub payload: Option<ComplianceRecordPayload>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<ComplianceRecordPayload>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplianceAuditReportRequest {
    pub report_id: String,
    pub org_id: String,
    pub field_id: String,
    pub generated_at: String,
    #[serde(default)]
    pub records: Vec<ComplianceRecord>,
    #[serde(default)]
    pub mandatory_record_types: Vec<ComplianceRecordType>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplianceAuditReport {
    pub schema_version: String,
    pub report_id: String,
    pub org_id: String,
    pub field_id: String,
    pub generated_at: String,
    pub record_count: usize,
    pub record_type_counts: BTreeMap<String, usize>,
    pub provenance_refs: Vec<String>,
    pub records: Vec<ComplianceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComplianceAuditReportError {
    EmptyReportId,
    EmptyOrgId,
    EmptyFieldId,
    EmptyGeneratedAt,
    EmptyRecords,
    MissingMandatoryRecords {
        missing: Vec<ComplianceRecordType>,
    },
    RecordScopeMismatch {
        record_id: String,
        expected_org_id: String,
        actual_org_id: String,
        expected_field_id: String,
        actual_field_id: String,
    },
}

impl std::fmt::Display for ComplianceAuditReportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComplianceAuditReportError::EmptyReportId => formatter.write_str("report_id cannot be empty"),
            ComplianceAuditReportError::EmptyOrgId => formatter.write_str("org_id cannot be empty"),
            ComplianceAuditReportError::EmptyFieldId => formatter.write_str("field_id cannot be empty"),
            ComplianceAuditReportError::EmptyGeneratedAt => formatter.write_str("generated_at cannot be empty"),
            ComplianceAuditReportError::EmptyRecords => {
                formatter.write_str("compliance audit report requires at least one record")
            }
            ComplianceAuditReportError::MissingMandatoryRecords { missing } => {
                let missing = missing
                    .iter()
                    .map(|record_type| record_type.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(formatter, "missing mandatory compliance records: {missing}")
            }
            ComplianceAuditReportError::RecordScopeMismatch {
                record_id,
                expected_org_id,
                actual_org_id,
                expected_field_id,
                actual_field_id,
            } => write!(
                formatter,
                "record {record_id} scope mismatch: expected org {expected_org_id}/field {expected_field_id}, got org {actual_org_id}/field {actual_field_id}"
            ),
        }
    }
}

impl std::error::Error for ComplianceAuditReportError {}

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
    #[error("{record_type} compliance records require a typed payload")]
    MissingTypedPayload { record_type: ComplianceRecordType },
    #[error("{record_type} compliance records do not accept payload type {payload_type}")]
    PayloadTypeMismatch {
        record_type: ComplianceRecordType,
        payload_type: String,
    },
    #[error("flight_id cannot be empty")]
    EmptyFlightId,
    #[error("operator_id cannot be empty")]
    EmptyOperatorId,
    #[error("aircraft_id cannot be empty")]
    EmptyAircraftId,
    #[error("remote ID flight log track cannot be empty")]
    EmptyRemoteIdTrack,
    #[error("remote ID track point timestamp cannot be empty")]
    EmptyTrackTimestamp,
    #[error("remote ID track point has invalid coordinate")]
    InvalidTrackCoordinate,
    #[error("remote ID track point has invalid altitude")]
    InvalidTrackAltitude,
    #[error("telemetry gap timestamp cannot be empty")]
    EmptyTelemetryGapTimestamp,
    #[error("telemetry gap reason cannot be empty")]
    EmptyTelemetryGapReason,
    #[error("{start_field} must be at or before {end_field}")]
    InvalidTimeRange {
        start_field: &'static str,
        end_field: &'static str,
    },
    #[error("request flight_id {request_flight_id} does not match payload flight_id {payload_flight_id}")]
    FlightIdMismatch {
        request_flight_id: String,
        payload_flight_id: String,
    },
    #[error("application_id cannot be empty")]
    EmptyApplicationId,
    #[error("product cannot be empty")]
    EmptyProduct,
    #[error("epa_or_label_ref cannot be empty")]
    EmptyEpaOrLabelRef,
    #[error("applied_at cannot be empty")]
    EmptyAppliedAt,
    #[error("application rate must be greater than zero")]
    InvalidApplicationRate,
    #[error("application units cannot be empty")]
    EmptyApplicationUnits,
    #[error("application geometry CRS cannot be empty")]
    EmptyApplicationGeometryCrs,
    #[error("unsupported application geometry CRS {value}; expected EPSG:4326")]
    UnsupportedApplicationGeometryCrs { value: String },
    #[error(
        "application geometry polygon must contain at least four coordinates including closure"
    )]
    InvalidApplicationGeometry,
    #[error("request field_id {request_field_id} does not match application field_id {payload_field_id}")]
    ApplicationFieldMismatch {
        request_field_id: String,
        payload_field_id: String,
    },
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OperatorCertificationError {
    #[error("cert_id cannot be empty")]
    EmptyCertId,
    #[error("operator_id cannot be empty")]
    EmptyOperatorId,
    #[error("cert_type cannot be empty")]
    EmptyCertType,
    #[error("issued_at cannot be empty")]
    EmptyIssuedAt,
    #[error("expires_at cannot be empty")]
    EmptyExpiresAt,
    #[error("authority cannot be empty")]
    EmptyAuthority,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("checked_at cannot be empty")]
    EmptyCheckedAt,
    #[error("{start_field} must be at or before {end_field}")]
    InvalidTimeRange {
        start_field: &'static str,
        end_field: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PreflightAuthorizationError {
    #[error("flight_id cannot be empty")]
    EmptyFlightId,
    #[error("operator_id cannot be empty")]
    EmptyOperatorId,
    #[error("required_cert_type cannot be empty")]
    EmptyRequiredCertType,
    #[error("planned_at cannot be empty")]
    EmptyPlannedAt,
    #[error("planned flight area is invalid: {source}")]
    InvalidFlightArea {
        #[source]
        source: AirspaceZoneError,
    },
    #[error("operator certification check failed: {source}")]
    Certification {
        #[source]
        source: OperatorCertificationError,
    },
    #[error("airspace intersection check failed: {source}")]
    Airspace {
        #[source]
        source: AirspaceZoneError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReiPhiError {
    #[error("label_ref cannot be empty")]
    EmptyLabelRef,
    #[error("checked_at cannot be empty")]
    EmptyCheckedAt,
    #[error("applied_at is not a valid RFC3339 timestamp")]
    InvalidAppliedAt,
    #[error("checked_at is not a valid RFC3339 timestamp")]
    InvalidCheckedAt,
    #[error("REI hours must be zero or greater")]
    InvalidReiHours,
    #[error("PHI days must be zero or greater")]
    InvalidPhiDays,
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

    let org_id = normalize_required_text(request.org_id, ComplianceRecordError::EmptyOrgId)?;
    let field_id = normalize_required_text(request.field_id, ComplianceRecordError::EmptyFieldId)?;
    let request_flight_id = normalize_optional_text(request.flight_id);
    let (flight_id, payload) = validate_compliance_payload(
        request.record_type,
        &field_id,
        request_flight_id,
        request.payload,
    )?;

    Ok(ComplianceRecord {
        record_id,
        version: 1,
        record_type: request.record_type,
        org_id,
        field_id,
        flight_id,
        created_at: normalize_required_text(created_at, ComplianceRecordError::EmptyCreatedAt)?,
        actor: normalize_required_text(request.actor, ComplianceRecordError::EmptyActor)?,
        provenance_ref: normalize_required_text(
            request.provenance_ref,
            ComplianceRecordError::EmptyProvenanceRef,
        )?,
        prior_version: None,
        change_reason: None,
        payload,
    })
}

pub fn append_compliance_record_version(
    latest: &ComplianceRecord,
    request: AppendComplianceRecordVersionRequest,
    created_at: String,
) -> Result<ComplianceRecord, ComplianceRecordError> {
    let field_id =
        normalize_optional_text(request.field_id).unwrap_or_else(|| latest.field_id.clone());
    let request_flight_id = match normalize_optional_text(request.flight_id) {
        Some(flight_id) => Some(flight_id),
        None => latest.flight_id.clone(),
    };
    let request_payload = request.payload.or_else(|| latest.payload.clone());
    let (flight_id, payload) = validate_compliance_payload(
        latest.record_type,
        &field_id,
        request_flight_id,
        request_payload,
    )?;

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
        payload,
    })
}

pub fn build_compliance_audit_report(
    request: ComplianceAuditReportRequest,
) -> Result<ComplianceAuditReport, ComplianceAuditReportError> {
    let report_id = normalize_required_report_text(
        request.report_id,
        ComplianceAuditReportError::EmptyReportId,
    )?;
    let org_id =
        normalize_required_report_text(request.org_id, ComplianceAuditReportError::EmptyOrgId)?;
    let field_id =
        normalize_required_report_text(request.field_id, ComplianceAuditReportError::EmptyFieldId)?;
    let generated_at = normalize_required_report_text(
        request.generated_at,
        ComplianceAuditReportError::EmptyGeneratedAt,
    )?;
    if request.records.is_empty() {
        return Err(ComplianceAuditReportError::EmptyRecords);
    }

    let present_types = request
        .records
        .iter()
        .map(|record| record.record_type)
        .collect::<BTreeSet<_>>();
    let missing = request
        .mandatory_record_types
        .iter()
        .copied()
        .filter(|record_type| !present_types.contains(record_type))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(ComplianceAuditReportError::MissingMandatoryRecords { missing });
    }

    let mut record_type_counts = BTreeMap::new();
    let mut provenance_refs = BTreeSet::new();
    let mut records = request.records;
    for record in &records {
        if record.org_id != org_id || record.field_id != field_id {
            return Err(ComplianceAuditReportError::RecordScopeMismatch {
                record_id: record.record_id.clone(),
                expected_org_id: org_id.clone(),
                actual_org_id: record.org_id.clone(),
                expected_field_id: field_id.clone(),
                actual_field_id: record.field_id.clone(),
            });
        }
        *record_type_counts
            .entry(record.record_type.as_str().to_string())
            .or_insert(0) += 1;
        provenance_refs.insert(record.provenance_ref.clone());
    }
    records.sort_by(|left, right| {
        left.record_type
            .as_str()
            .cmp(right.record_type.as_str())
            .then_with(|| left.record_id.cmp(&right.record_id))
            .then_with(|| left.version.cmp(&right.version))
    });

    Ok(ComplianceAuditReport {
        schema_version: "compliance.audit_report.v1".to_string(),
        report_id,
        org_id,
        field_id,
        generated_at,
        record_count: records.len(),
        record_type_counts,
        provenance_refs: provenance_refs.into_iter().collect(),
        records,
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

pub fn build_operator_certification_record(
    request: OperatorCertificationRegistrationRequest,
    generated_cert_id: String,
    created_at: String,
) -> Result<OperatorCertificationRecord, OperatorCertificationError> {
    let cert_id = match normalize_optional_text(request.cert_id) {
        Some(cert_id) => cert_id,
        None => normalize_required_operator_text(
            generated_cert_id,
            OperatorCertificationError::EmptyCertId,
        )?,
    };
    let operator_id = normalize_required_operator_text(
        request.operator_id,
        OperatorCertificationError::EmptyOperatorId,
    )?;
    let cert_type = normalize_required_operator_text(
        request.cert_type,
        OperatorCertificationError::EmptyCertType,
    )?;
    let issued_at = normalize_required_operator_text(
        request.issued_at,
        OperatorCertificationError::EmptyIssuedAt,
    )?;
    let expires_at = normalize_required_operator_text(
        request.expires_at,
        OperatorCertificationError::EmptyExpiresAt,
    )?;
    ensure_operator_time_range(&issued_at, &expires_at, "issued_at", "expires_at")?;

    Ok(OperatorCertificationRecord {
        cert_id,
        operator_id,
        cert_type,
        issued_at,
        expires_at,
        authority: normalize_required_operator_text(
            request.authority,
            OperatorCertificationError::EmptyAuthority,
        )?,
        created_at: normalize_required_operator_text(
            created_at,
            OperatorCertificationError::EmptyCreatedAt,
        )?,
    })
}

pub fn check_operator_certification(
    operator_id: String,
    required_cert_type: String,
    checked_at: String,
    certifications: &[OperatorCertificationRecord],
) -> Result<CertificationCheckResult, OperatorCertificationError> {
    let operator_id =
        normalize_required_operator_text(operator_id, OperatorCertificationError::EmptyOperatorId)?;
    let required_cert_type = normalize_required_operator_text(
        required_cert_type,
        OperatorCertificationError::EmptyCertType,
    )?;
    let checked_at =
        normalize_required_operator_text(checked_at, OperatorCertificationError::EmptyCheckedAt)?;

    let best_match = certifications
        .iter()
        .filter(|certification| {
            certification.operator_id == operator_id
                && certification.cert_type == required_cert_type
                && certification.issued_at <= checked_at
        })
        .max_by(|left, right| left.expires_at.cmp(&right.expires_at));

    let Some(certification) = best_match else {
        return Ok(CertificationCheckResult {
            operator_id,
            required_cert_type,
            checked_at,
            status: CertificationStatus::Missing,
            cert_id: None,
            expires_at: None,
            block_flight: true,
            reason_code: Some(CertificationBlockReason::MissingCertification),
        });
    };

    if certification.expires_at < checked_at {
        Ok(CertificationCheckResult {
            operator_id,
            required_cert_type,
            checked_at,
            status: CertificationStatus::Expired,
            cert_id: Some(certification.cert_id.clone()),
            expires_at: Some(certification.expires_at.clone()),
            block_flight: true,
            reason_code: Some(CertificationBlockReason::ExpiredCertification),
        })
    } else {
        Ok(CertificationCheckResult {
            operator_id,
            required_cert_type,
            checked_at,
            status: CertificationStatus::Valid,
            cert_id: Some(certification.cert_id.clone()),
            expires_at: Some(certification.expires_at.clone()),
            block_flight: false,
            reason_code: None,
        })
    }
}

pub fn evaluate_preflight_authorization(
    request: PreflightAuthorizationRequest,
    airspace_zones: &[AirspaceZoneRecord],
    certifications: &[OperatorCertificationRecord],
) -> Result<PreflightAuthorizationDecision, PreflightAuthorizationError> {
    let flight_id = normalize_required_preflight_text(
        request.flight_id,
        PreflightAuthorizationError::EmptyFlightId,
    )?;
    let operator_id = normalize_required_preflight_text(
        request.operator_id,
        PreflightAuthorizationError::EmptyOperatorId,
    )?;
    let required_cert_type = normalize_required_preflight_text(
        request.required_cert_type,
        PreflightAuthorizationError::EmptyRequiredCertType,
    )?;
    let planned_at = normalize_required_preflight_text(
        request.planned_at,
        PreflightAuthorizationError::EmptyPlannedAt,
    )?;
    let planned_area = validate_airspace_polygon(request.planned_area)
        .map_err(|source| PreflightAuthorizationError::InvalidFlightArea { source })?;

    match request.airspace_status {
        PreflightAirspaceStatus::Missing => {
            return Ok(blocked_authorization(
                flight_id,
                operator_id,
                planned_at,
                AuthorizationBlockReason::MissingAirspaceData,
                None,
                None,
            ));
        }
        PreflightAirspaceStatus::Stale => {
            return Ok(blocked_authorization(
                flight_id,
                operator_id,
                planned_at,
                AuthorizationBlockReason::StaleAirspaceData,
                None,
                None,
            ));
        }
        PreflightAirspaceStatus::Fresh => {}
    }

    let cert_check = check_operator_certification(
        operator_id.clone(),
        required_cert_type,
        planned_at.clone(),
        certifications,
    )
    .map_err(|source| PreflightAuthorizationError::Certification { source })?;
    if cert_check.block_flight {
        let reason_code = match cert_check.reason_code {
            Some(CertificationBlockReason::ExpiredCertification) => {
                AuthorizationBlockReason::ExpiredCertification
            }
            Some(CertificationBlockReason::MissingCertification) | None => {
                AuthorizationBlockReason::MissingCertification
            }
        };
        return Ok(blocked_authorization(
            flight_id,
            operator_id,
            planned_at,
            reason_code,
            None,
            cert_check.cert_id,
        ));
    }

    for zone in airspace_zones {
        if !hard_blocking_zone(zone) || !airspace_zone_is_effective_at(zone, Some(&planned_at)) {
            continue;
        }
        let intersects = airspace_zone_intersects_polygon(zone, &planned_area)
            .map_err(|source| PreflightAuthorizationError::Airspace { source })?;
        if intersects {
            return Ok(blocked_authorization(
                flight_id,
                operator_id,
                planned_at,
                AuthorizationBlockReason::NoFlyZoneIntersection,
                Some(zone.zone_id.clone()),
                cert_check.cert_id,
            ));
        }
    }

    Ok(PreflightAuthorizationDecision {
        flight_id,
        operator_id,
        checked_at: planned_at,
        status: AuthorizationDecisionStatus::Permitted,
        reason_code: None,
        zone_ref: None,
        cert_ref: cert_check.cert_id,
    })
}

pub fn compute_rei_phi_window(
    application: &ChemicalApplicationRecord,
    label: ProductLabelInterval,
) -> Result<ReiPhiWindow, ReiPhiError> {
    let label_ref = normalize_required_rei_text(label.label_ref, ReiPhiError::EmptyLabelRef)?;
    if label.rei_hours.is_some_and(|hours| hours < 0) {
        return Err(ReiPhiError::InvalidReiHours);
    }
    if label.phi_days.is_some_and(|days| days < 0) {
        return Err(ReiPhiError::InvalidPhiDays);
    }
    let applied_at = parse_rfc3339_utc(&application.applied_at, ReiPhiError::InvalidAppliedAt)?;
    let rei_clear_at = label
        .rei_hours
        .map(|hours| format_rfc3339(applied_at + Duration::hours(hours)));
    let phi_clear_at = label
        .phi_days
        .map(|days| format_rfc3339(applied_at + Duration::days(days)));

    Ok(ReiPhiWindow {
        application_id: application.application_id.clone(),
        label_ref,
        source_application_ref: application.application_id.clone(),
        applied_at: format_rfc3339(applied_at),
        rei_status: if rei_clear_at.is_some() {
            IntervalWindowStatus::Known
        } else {
            IntervalWindowStatus::Unknown
        },
        phi_status: if phi_clear_at.is_some() {
            IntervalWindowStatus::Known
        } else {
            IntervalWindowStatus::Unknown
        },
        rei_clear_at,
        phi_clear_at,
    })
}

pub fn evaluate_entry_harvest_clearance(
    window: &ReiPhiWindow,
    action: EntryHarvestAction,
    checked_at: String,
) -> Result<EntryHarvestDecision, ReiPhiError> {
    let checked_at_text = normalize_required_rei_text(checked_at, ReiPhiError::EmptyCheckedAt)?;
    let checked_at = parse_rfc3339_utc(&checked_at_text, ReiPhiError::InvalidCheckedAt)?;
    let (clear_at, unknown_reason, active_reason) = match action {
        EntryHarvestAction::ReEntry => (
            window.rei_clear_at.as_deref(),
            EntryHarvestBlockReason::UnknownRei,
            EntryHarvestBlockReason::ReiActive,
        ),
        EntryHarvestAction::Harvest => (
            window.phi_clear_at.as_deref(),
            EntryHarvestBlockReason::UnknownPhi,
            EntryHarvestBlockReason::PhiActive,
        ),
    };

    let Some(clear_at) = clear_at else {
        return Ok(entry_harvest_blocked(
            window,
            action,
            format_rfc3339(checked_at),
            unknown_reason,
            None,
        ));
    };
    let clear_at_time = parse_rfc3339_utc(clear_at, ReiPhiError::InvalidAppliedAt)?;
    if checked_at < clear_at_time {
        Ok(entry_harvest_blocked(
            window,
            action,
            format_rfc3339(checked_at),
            active_reason,
            Some(clear_at.to_string()),
        ))
    } else {
        Ok(EntryHarvestDecision {
            application_id: window.application_id.clone(),
            action,
            checked_at: format_rfc3339(checked_at),
            status: EntryHarvestDecisionStatus::Cleared,
            reason_code: None,
            clear_at: Some(clear_at.to_string()),
        })
    }
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

fn validate_compliance_payload(
    record_type: ComplianceRecordType,
    field_id: &str,
    request_flight_id: Option<String>,
    payload: Option<ComplianceRecordPayload>,
) -> Result<(Option<String>, Option<ComplianceRecordPayload>), ComplianceRecordError> {
    match record_type {
        ComplianceRecordType::RemoteIdLog | ComplianceRecordType::FlightLog => {
            let payload =
                payload.ok_or(ComplianceRecordError::MissingTypedPayload { record_type })?;
            match payload {
                ComplianceRecordPayload::RemoteIdFlightLog(log) => {
                    let log = validate_remote_id_flight_log(log)?;
                    let flight_id = match request_flight_id {
                        Some(request_flight_id) if request_flight_id == log.flight_id => {
                            Some(request_flight_id)
                        }
                        Some(request_flight_id) => {
                            return Err(ComplianceRecordError::FlightIdMismatch {
                                request_flight_id,
                                payload_flight_id: log.flight_id,
                            });
                        }
                        None => Some(log.flight_id.clone()),
                    };
                    Ok((
                        flight_id,
                        Some(ComplianceRecordPayload::RemoteIdFlightLog(log)),
                    ))
                }
                other => Err(ComplianceRecordError::PayloadTypeMismatch {
                    record_type,
                    payload_type: other.payload_type().to_string(),
                }),
            }
        }
        ComplianceRecordType::ChemicalApplication => {
            let payload =
                payload.ok_or(ComplianceRecordError::MissingTypedPayload { record_type })?;
            match payload {
                ComplianceRecordPayload::ChemicalApplication(application) => {
                    let application = validate_chemical_application(application)?;
                    if application.field_id != field_id {
                        return Err(ComplianceRecordError::ApplicationFieldMismatch {
                            request_field_id: field_id.to_string(),
                            payload_field_id: application.field_id,
                        });
                    }
                    Ok((
                        request_flight_id,
                        Some(ComplianceRecordPayload::ChemicalApplication(application)),
                    ))
                }
                other => Err(ComplianceRecordError::PayloadTypeMismatch {
                    record_type,
                    payload_type: other.payload_type().to_string(),
                }),
            }
        }
        _ => match payload {
            Some(payload) => Err(ComplianceRecordError::PayloadTypeMismatch {
                record_type,
                payload_type: payload.payload_type().to_string(),
            }),
            None => Ok((request_flight_id, None)),
        },
    }
}

fn validate_remote_id_flight_log(
    log: RemoteIdFlightLogRecord,
) -> Result<RemoteIdFlightLogRecord, ComplianceRecordError> {
    let flight_id = normalize_required_text(log.flight_id, ComplianceRecordError::EmptyFlightId)?;
    let operator_id =
        normalize_required_text(log.operator_id, ComplianceRecordError::EmptyOperatorId)?;
    let aircraft_id =
        normalize_required_text(log.aircraft_id, ComplianceRecordError::EmptyAircraftId)?;
    let started_at =
        normalize_required_text(log.started_at, ComplianceRecordError::EmptyCreatedAt)?;
    let ended_at = normalize_required_text(log.ended_at, ComplianceRecordError::EmptyCreatedAt)?;
    ensure_time_range(&started_at, &ended_at, "started_at", "ended_at")?;

    if log.track.is_empty() {
        return Err(ComplianceRecordError::EmptyRemoteIdTrack);
    }
    let track = log
        .track
        .into_iter()
        .map(validate_remote_id_track_point)
        .collect::<Result<Vec<_>, _>>()?;
    let telemetry_gaps = log
        .telemetry_gaps
        .into_iter()
        .map(validate_telemetry_gap)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(RemoteIdFlightLogRecord {
        flight_id,
        operator_id,
        aircraft_id,
        started_at,
        ended_at,
        track,
        telemetry_gaps,
    })
}

fn validate_remote_id_track_point(
    point: RemoteIdTrackPoint,
) -> Result<RemoteIdTrackPoint, ComplianceRecordError> {
    let observed_at = normalize_required_text(
        point.observed_at,
        ComplianceRecordError::EmptyTrackTimestamp,
    )?;
    if !point.longitude.is_finite()
        || !point.latitude.is_finite()
        || point.longitude < -180.0
        || point.longitude > 180.0
        || point.latitude < -90.0
        || point.latitude > 90.0
    {
        return Err(ComplianceRecordError::InvalidTrackCoordinate);
    }
    if !point.altitude_m.is_finite() {
        return Err(ComplianceRecordError::InvalidTrackAltitude);
    }

    Ok(RemoteIdTrackPoint {
        observed_at,
        longitude: point.longitude,
        latitude: point.latitude,
        altitude_m: point.altitude_m,
    })
}

fn validate_telemetry_gap(
    gap: TelemetryGapRecord,
) -> Result<TelemetryGapRecord, ComplianceRecordError> {
    let started_at = normalize_required_text(
        gap.started_at,
        ComplianceRecordError::EmptyTelemetryGapTimestamp,
    )?;
    let ended_at = normalize_required_text(
        gap.ended_at,
        ComplianceRecordError::EmptyTelemetryGapTimestamp,
    )?;
    let reason =
        normalize_required_text(gap.reason, ComplianceRecordError::EmptyTelemetryGapReason)?;
    ensure_time_range(
        &started_at,
        &ended_at,
        "telemetry_gap.started_at",
        "telemetry_gap.ended_at",
    )?;

    Ok(TelemetryGapRecord {
        started_at,
        ended_at,
        reason,
    })
}

fn validate_chemical_application(
    application: ChemicalApplicationRecord,
) -> Result<ChemicalApplicationRecord, ComplianceRecordError> {
    let application_id = normalize_required_text(
        application.application_id,
        ComplianceRecordError::EmptyApplicationId,
    )?;
    let product =
        normalize_required_text(application.product, ComplianceRecordError::EmptyProduct)?;
    let epa_or_label_ref = normalize_required_text(
        application.epa_or_label_ref,
        ComplianceRecordError::EmptyEpaOrLabelRef,
    )?;
    let field_id =
        normalize_required_text(application.field_id, ComplianceRecordError::EmptyFieldId)?;
    let applied_at = normalize_required_text(
        application.applied_at,
        ComplianceRecordError::EmptyAppliedAt,
    )?;
    let units = normalize_required_text(
        application.units,
        ComplianceRecordError::EmptyApplicationUnits,
    )?;
    let operator_id = normalize_required_text(
        application.operator_id,
        ComplianceRecordError::EmptyOperatorId,
    )?;
    if !application.rate.is_finite() || application.rate <= 0.0 {
        return Err(ComplianceRecordError::InvalidApplicationRate);
    }
    let geometry = validate_application_geometry(application.geometry)?;

    Ok(ChemicalApplicationRecord {
        application_id,
        product,
        epa_or_label_ref,
        field_id,
        geometry,
        applied_at,
        rate: application.rate,
        units,
        operator_id,
    })
}

fn validate_application_geometry(
    geometry: ApplicationGeometry,
) -> Result<ApplicationGeometry, ComplianceRecordError> {
    let crs = normalize_required_text(
        geometry.crs,
        ComplianceRecordError::EmptyApplicationGeometryCrs,
    )?;
    if !crs.eq_ignore_ascii_case("EPSG:4326") {
        return Err(ComplianceRecordError::UnsupportedApplicationGeometryCrs { value: crs });
    }
    let coordinates = validate_application_polygon(geometry.coordinates)?;

    Ok(ApplicationGeometry {
        crs: "EPSG:4326".to_string(),
        coordinates,
    })
}

fn validate_application_polygon(
    coordinates: Vec<AirspaceCoordinate>,
) -> Result<Vec<AirspaceCoordinate>, ComplianceRecordError> {
    if coordinates.len() < 4 {
        return Err(ComplianceRecordError::InvalidApplicationGeometry);
    }
    for coordinate in &coordinates {
        if !coordinate.longitude.is_finite()
            || !coordinate.latitude.is_finite()
            || coordinate.longitude < -180.0
            || coordinate.longitude > 180.0
            || coordinate.latitude < -90.0
            || coordinate.latitude > 90.0
        {
            return Err(ComplianceRecordError::InvalidApplicationGeometry);
        }
    }
    let first = coordinates[0];
    let last = coordinates[coordinates.len() - 1];
    if !same_coordinate(first, last) {
        return Err(ComplianceRecordError::InvalidApplicationGeometry);
    }

    Ok(coordinates)
}

fn ensure_time_range(
    started_at: &str,
    ended_at: &str,
    start_field: &'static str,
    end_field: &'static str,
) -> Result<(), ComplianceRecordError> {
    if started_at > ended_at {
        Err(ComplianceRecordError::InvalidTimeRange {
            start_field,
            end_field,
        })
    } else {
        Ok(())
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

fn normalize_required_report_text(
    value: String,
    error: ComplianceAuditReportError,
) -> Result<String, ComplianceAuditReportError> {
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

fn normalize_required_operator_text(
    value: String,
    error: OperatorCertificationError,
) -> Result<String, OperatorCertificationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn ensure_operator_time_range(
    started_at: &str,
    ended_at: &str,
    start_field: &'static str,
    end_field: &'static str,
) -> Result<(), OperatorCertificationError> {
    if started_at > ended_at {
        Err(OperatorCertificationError::InvalidTimeRange {
            start_field,
            end_field,
        })
    } else {
        Ok(())
    }
}

fn normalize_required_preflight_text(
    value: String,
    error: PreflightAuthorizationError,
) -> Result<String, PreflightAuthorizationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn blocked_authorization(
    flight_id: String,
    operator_id: String,
    checked_at: String,
    reason_code: AuthorizationBlockReason,
    zone_ref: Option<String>,
    cert_ref: Option<String>,
) -> PreflightAuthorizationDecision {
    PreflightAuthorizationDecision {
        flight_id,
        operator_id,
        checked_at,
        status: AuthorizationDecisionStatus::Blocked,
        reason_code: Some(reason_code),
        zone_ref,
        cert_ref,
    }
}

fn hard_blocking_zone(zone: &AirspaceZoneRecord) -> bool {
    matches!(
        zone.zone_class,
        AirspaceZoneClass::NoFly
            | AirspaceZoneClass::Restricted
            | AirspaceZoneClass::TemporaryFlightRestriction
    )
}

fn normalize_required_rei_text(value: String, error: ReiPhiError) -> Result<String, ReiPhiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn parse_rfc3339_utc(value: &str, error: ReiPhiError) -> Result<DateTime<Utc>, ReiPhiError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| error)
}

fn format_rfc3339(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn entry_harvest_blocked(
    window: &ReiPhiWindow,
    action: EntryHarvestAction,
    checked_at: String,
    reason_code: EntryHarvestBlockReason,
    clear_at: Option<String>,
) -> EntryHarvestDecision {
    EntryHarvestDecision {
        application_id: window.application_id.clone(),
        action,
        checked_at,
        status: EntryHarvestDecisionStatus::Blocked,
        reason_code: Some(reason_code),
        clear_at,
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
        build_compliance_audit_report, build_initial_compliance_record,
        build_operator_certification_record, check_operator_certification, compute_rei_phi_window,
        evaluate_entry_harvest_clearance, evaluate_preflight_authorization,
        refuse_in_place_mutation, AirspaceCoordinate, AirspaceZoneClass, AirspaceZoneError,
        AirspaceZoneIngestRequest, AppendComplianceRecordVersionRequest, ApplicationGeometry,
        AuthorizationBlockReason, AuthorizationDecisionStatus, CertificationBlockReason,
        CertificationStatus, ChemicalApplicationRecord, ComplianceAuditReportError,
        ComplianceAuditReportRequest, ComplianceRecordError, ComplianceRecordPayload,
        ComplianceRecordType, CreateComplianceRecordRequest, EntryHarvestAction,
        EntryHarvestBlockReason, EntryHarvestDecisionStatus, IntervalWindowStatus,
        OperatorCertificationRegistrationRequest, PreflightAirspaceStatus,
        PreflightAuthorizationRequest, ProductLabelInterval, RemoteIdFlightLogRecord,
        RemoteIdTrackPoint, TelemetryGapRecord,
    };

    #[test]
    fn initial_compliance_record_has_stable_identity_and_provenance() {
        let record = build_initial_compliance_record(
            CreateComplianceRecordRequest {
                record_id: Some(" comp-rec-1 ".to_string()),
                record_type: ComplianceRecordType::ComplianceReport,
                org_id: " org-alpha ".to_string(),
                field_id: " field-north ".to_string(),
                flight_id: Some(" flight-77 ".to_string()),
                actor: " compliance-officer-1 ".to_string(),
                provenance_ref: " provenance:compliance/comp-rec-1/v1 ".to_string(),
                payload: None,
            },
            "generated-record".to_string(),
            " 2026-06-12T12:00:00Z ".to_string(),
        )
        .expect("record should be valid");

        assert_eq!(record.record_id, "comp-rec-1");
        assert_eq!(record.version, 1);
        assert_eq!(record.record_type, ComplianceRecordType::ComplianceReport);
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
                record_type: ComplianceRecordType::ComplianceReport,
                org_id: "org-alpha".to_string(),
                field_id: "field-north".to_string(),
                flight_id: Some("flight-77".to_string()),
                actor: "compliance-officer-1".to_string(),
                provenance_ref: "provenance:compliance/comp-rec-1/v1".to_string(),
                payload: None,
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
                payload: None,
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
                record_type: ComplianceRecordType::ComplianceReport,
                org_id: "org-alpha".to_string(),
                field_id: "field-north".to_string(),
                flight_id: None,
                actor: "compliance-officer-1".to_string(),
                provenance_ref: " ".to_string(),
                payload: None,
            },
            "generated-record".to_string(),
            "2026-06-12T12:00:00Z".to_string(),
        )
        .expect_err("missing provenance should be rejected");

        assert_eq!(error, ComplianceRecordError::EmptyProvenanceRef);
    }

    #[test]
    fn operator_certification_valid_at_flight_time_allows_flight_input() {
        let cert = operator_cert(
            "cert-107-valid",
            "operator-17",
            "part-107",
            "2026-01-01T00:00:00Z",
            "2026-12-31T23:59:59Z",
        );

        let result = check_operator_certification(
            " operator-17 ".to_string(),
            " part-107 ".to_string(),
            "2026-06-12T12:00:00Z".to_string(),
            &[cert],
        )
        .expect("certification check should run");

        assert_eq!(result.status, CertificationStatus::Valid);
        assert_eq!(result.cert_id.as_deref(), Some("cert-107-valid"));
        assert!(!result.block_flight);
        assert_eq!(result.reason_code, None);
    }

    #[test]
    fn operator_certification_expired_blocks_flight_input() {
        let cert = operator_cert(
            "cert-107-expired",
            "operator-17",
            "part-107",
            "2025-01-01T00:00:00Z",
            "2026-01-01T00:00:00Z",
        );

        let result = check_operator_certification(
            "operator-17".to_string(),
            "part-107".to_string(),
            "2026-06-12T12:00:00Z".to_string(),
            &[cert],
        )
        .expect("certification check should run");

        assert_eq!(result.status, CertificationStatus::Expired);
        assert!(result.block_flight);
        assert_eq!(
            result.reason_code,
            Some(CertificationBlockReason::ExpiredCertification)
        );
    }

    #[test]
    fn operator_certification_missing_blocks_flight_input() {
        let result = check_operator_certification(
            "operator-17".to_string(),
            "part-107".to_string(),
            "2026-06-12T12:00:00Z".to_string(),
            &[],
        )
        .expect("certification check should run");

        assert_eq!(result.status, CertificationStatus::Missing);
        assert!(result.block_flight);
        assert_eq!(
            result.reason_code,
            Some(CertificationBlockReason::MissingCertification)
        );
    }

    #[test]
    fn preflight_authorization_permits_clear_flight_with_valid_cert() {
        let cert = operator_cert(
            "cert-107-valid",
            "operator-17",
            "part-107",
            "2026-01-01T00:00:00Z",
            "2026-12-31T23:59:59Z",
        );

        let decision = evaluate_preflight_authorization(
            preflight_request(PreflightAirspaceStatus::Fresh, clear_flight_area()),
            &[],
            &[cert],
        )
        .expect("authorization should evaluate");

        assert_eq!(decision.status, AuthorizationDecisionStatus::Permitted);
        assert_eq!(decision.reason_code, None);
        assert_eq!(decision.cert_ref.as_deref(), Some("cert-107-valid"));
    }

    #[test]
    fn preflight_authorization_blocks_active_no_fly_intersection() {
        let cert = operator_cert(
            "cert-107-valid",
            "operator-17",
            "part-107",
            "2026-01-01T00:00:00Z",
            "2026-12-31T23:59:59Z",
        );
        let zone = build_airspace_zone_record(
            base_zone_request(),
            "generated-zone".to_string(),
            "2026-06-12T12:00:00Z".to_string(),
        )
        .expect("zone should be valid");

        let decision = evaluate_preflight_authorization(
            preflight_request(PreflightAirspaceStatus::Fresh, intersecting_flight_area()),
            &[zone],
            &[cert],
        )
        .expect("authorization should evaluate");

        assert_eq!(decision.status, AuthorizationDecisionStatus::Blocked);
        assert_eq!(
            decision.reason_code,
            Some(AuthorizationBlockReason::NoFlyZoneIntersection)
        );
        assert_eq!(decision.zone_ref.as_deref(), Some("zone-1"));
    }

    #[test]
    fn preflight_authorization_denies_on_missing_airspace_data() {
        let cert = operator_cert(
            "cert-107-valid",
            "operator-17",
            "part-107",
            "2026-01-01T00:00:00Z",
            "2026-12-31T23:59:59Z",
        );

        let decision = evaluate_preflight_authorization(
            preflight_request(PreflightAirspaceStatus::Missing, clear_flight_area()),
            &[],
            &[cert],
        )
        .expect("authorization should evaluate");

        assert_eq!(decision.status, AuthorizationDecisionStatus::Blocked);
        assert_eq!(
            decision.reason_code,
            Some(AuthorizationBlockReason::MissingAirspaceData)
        );
    }

    #[test]
    fn preflight_authorization_denies_on_stale_airspace_data() {
        let cert = operator_cert(
            "cert-107-valid",
            "operator-17",
            "part-107",
            "2026-01-01T00:00:00Z",
            "2026-12-31T23:59:59Z",
        );

        let decision = evaluate_preflight_authorization(
            preflight_request(PreflightAirspaceStatus::Stale, clear_flight_area()),
            &[],
            &[cert],
        )
        .expect("authorization should evaluate");

        assert_eq!(decision.status, AuthorizationDecisionStatus::Blocked);
        assert_eq!(
            decision.reason_code,
            Some(AuthorizationBlockReason::StaleAirspaceData)
        );
    }

    #[test]
    fn rei_phi_window_computes_clearance_times_from_label() {
        let window = compute_rei_phi_window(
            &application_record(),
            ProductLabelInterval {
                label_ref: "EPA-12345-LBL".to_string(),
                rei_hours: Some(12),
                phi_days: Some(7),
            },
        )
        .expect("window should compute");

        assert_eq!(window.application_id, "chem-app-1");
        assert_eq!(window.label_ref, "EPA-12345-LBL");
        assert_eq!(window.rei_status, IntervalWindowStatus::Known);
        assert_eq!(window.phi_status, IntervalWindowStatus::Known);
        assert_eq!(window.rei_clear_at.as_deref(), Some("2026-06-13T01:00:00Z"));
        assert_eq!(window.phi_clear_at.as_deref(), Some("2026-06-19T13:00:00Z"));
    }

    #[test]
    fn rei_gate_blocks_reentry_before_clearance() {
        let window = compute_rei_phi_window(
            &application_record(),
            ProductLabelInterval {
                label_ref: "EPA-12345-LBL".to_string(),
                rei_hours: Some(12),
                phi_days: Some(7),
            },
        )
        .expect("window should compute");

        let decision = evaluate_entry_harvest_clearance(
            &window,
            EntryHarvestAction::ReEntry,
            "2026-06-12T18:00:00Z".to_string(),
        )
        .expect("clearance should evaluate");

        assert_eq!(decision.status, EntryHarvestDecisionStatus::Blocked);
        assert_eq!(
            decision.reason_code,
            Some(EntryHarvestBlockReason::ReiActive)
        );
        assert_eq!(decision.clear_at.as_deref(), Some("2026-06-13T01:00:00Z"));
    }

    #[test]
    fn missing_label_interval_marks_unknown_and_blocks_clearance() {
        let window = compute_rei_phi_window(
            &application_record(),
            ProductLabelInterval {
                label_ref: "EPA-12345-LBL".to_string(),
                rei_hours: None,
                phi_days: None,
            },
        )
        .expect("window should compute with unknown intervals");

        assert_eq!(window.rei_status, IntervalWindowStatus::Unknown);
        assert_eq!(window.phi_status, IntervalWindowStatus::Unknown);
        assert_eq!(window.rei_clear_at, None);
        assert_eq!(window.phi_clear_at, None);

        let decision = evaluate_entry_harvest_clearance(
            &window,
            EntryHarvestAction::Harvest,
            "2026-06-20T12:00:00Z".to_string(),
        )
        .expect("clearance should evaluate");

        assert_eq!(decision.status, EntryHarvestDecisionStatus::Blocked);
        assert_eq!(
            decision.reason_code,
            Some(EntryHarvestBlockReason::UnknownPhi)
        );
    }

    #[test]
    fn remote_id_flight_log_preserves_operator_aircraft_track_and_explicit_gap() {
        let record = build_initial_compliance_record(
            CreateComplianceRecordRequest {
                record_id: Some("remote-log-1".to_string()),
                record_type: ComplianceRecordType::RemoteIdLog,
                org_id: "org-alpha".to_string(),
                field_id: "field-north".to_string(),
                flight_id: None,
                actor: "operator-17".to_string(),
                provenance_ref: "provenance:remote-id/remote-log-1/v1".to_string(),
                payload: Some(ComplianceRecordPayload::RemoteIdFlightLog(
                    RemoteIdFlightLogRecord {
                        flight_id: "flight-77".to_string(),
                        operator_id: "operator-17".to_string(),
                        aircraft_id: "aircraft-ag-9".to_string(),
                        started_at: "2026-06-12T12:00:00Z".to_string(),
                        ended_at: "2026-06-12T12:18:00Z".to_string(),
                        track: vec![
                            RemoteIdTrackPoint {
                                observed_at: "2026-06-12T12:02:00Z".to_string(),
                                longitude: -96.61,
                                latitude: 41.21,
                                altitude_m: 118.0,
                            },
                            RemoteIdTrackPoint {
                                observed_at: "2026-06-12T12:10:00Z".to_string(),
                                longitude: -96.58,
                                latitude: 41.24,
                                altitude_m: 116.0,
                            },
                        ],
                        telemetry_gaps: vec![TelemetryGapRecord {
                            started_at: "2026-06-12T12:04:00Z".to_string(),
                            ended_at: "2026-06-12T12:08:00Z".to_string(),
                            reason: "remote-id-broadcast-dropout".to_string(),
                        }],
                    },
                )),
            },
            "generated-record".to_string(),
            "2026-06-12T12:19:00Z".to_string(),
        )
        .expect("remote id log should be valid");

        assert_eq!(record.flight_id.as_deref(), Some("flight-77"));
        let payload = record
            .payload
            .as_ref()
            .expect("typed payload should be retained");
        match payload {
            ComplianceRecordPayload::RemoteIdFlightLog(log) => {
                assert_eq!(log.operator_id, "operator-17");
                assert_eq!(log.aircraft_id, "aircraft-ag-9");
                assert_eq!(log.track.len(), 2);
                assert_eq!(log.telemetry_gaps.len(), 1);
                assert_eq!(log.telemetry_gaps[0].reason, "remote-id-broadcast-dropout");
            }
            ComplianceRecordPayload::ChemicalApplication(_) => {
                panic!("remote id log should not become a chemical application payload")
            }
        }
    }

    #[test]
    fn chemical_application_requires_product_rate_and_crs_geometry() {
        let valid = build_initial_compliance_record(
            CreateComplianceRecordRequest {
                record_id: Some("chem-app-1".to_string()),
                record_type: ComplianceRecordType::ChemicalApplication,
                org_id: "org-alpha".to_string(),
                field_id: "field-north".to_string(),
                flight_id: Some("flight-77".to_string()),
                actor: "operator-17".to_string(),
                provenance_ref: "provenance:application/chem-app-1/v1".to_string(),
                payload: Some(ComplianceRecordPayload::ChemicalApplication(
                    ChemicalApplicationRecord {
                        application_id: "chem-app-1".to_string(),
                        product: "Example Herbicide".to_string(),
                        epa_or_label_ref: "EPA-12345-LBL".to_string(),
                        field_id: "field-north".to_string(),
                        geometry: ApplicationGeometry {
                            crs: "EPSG:4326".to_string(),
                            coordinates: square_zone(),
                        },
                        applied_at: "2026-06-12T13:00:00Z".to_string(),
                        rate: 1.75,
                        units: "L/ha".to_string(),
                        operator_id: "operator-17".to_string(),
                    },
                )),
            },
            "generated-record".to_string(),
            "2026-06-12T13:01:00Z".to_string(),
        )
        .expect("complete chemical application should be valid");

        assert!(matches!(
            valid.payload,
            Some(ComplianceRecordPayload::ChemicalApplication(_))
        ));

        let missing_product = build_initial_compliance_record(
            CreateComplianceRecordRequest {
                record_id: Some("chem-app-2".to_string()),
                record_type: ComplianceRecordType::ChemicalApplication,
                org_id: "org-alpha".to_string(),
                field_id: "field-north".to_string(),
                flight_id: None,
                actor: "operator-17".to_string(),
                provenance_ref: "provenance:application/chem-app-2/v1".to_string(),
                payload: Some(ComplianceRecordPayload::ChemicalApplication(
                    ChemicalApplicationRecord {
                        application_id: "chem-app-2".to_string(),
                        product: " ".to_string(),
                        epa_or_label_ref: "EPA-12345-LBL".to_string(),
                        field_id: "field-north".to_string(),
                        geometry: ApplicationGeometry {
                            crs: "EPSG:4326".to_string(),
                            coordinates: square_zone(),
                        },
                        applied_at: "2026-06-12T13:00:00Z".to_string(),
                        rate: 1.75,
                        units: "L/ha".to_string(),
                        operator_id: "operator-17".to_string(),
                    },
                )),
            },
            "generated-record".to_string(),
            "2026-06-12T13:01:00Z".to_string(),
        )
        .expect_err("missing product should be rejected");
        assert_eq!(missing_product, ComplianceRecordError::EmptyProduct);

        let missing_rate = build_initial_compliance_record(
            CreateComplianceRecordRequest {
                record_id: Some("chem-app-3".to_string()),
                record_type: ComplianceRecordType::ChemicalApplication,
                org_id: "org-alpha".to_string(),
                field_id: "field-north".to_string(),
                flight_id: None,
                actor: "operator-17".to_string(),
                provenance_ref: "provenance:application/chem-app-3/v1".to_string(),
                payload: Some(ComplianceRecordPayload::ChemicalApplication(
                    ChemicalApplicationRecord {
                        application_id: "chem-app-3".to_string(),
                        product: "Example Herbicide".to_string(),
                        epa_or_label_ref: "EPA-12345-LBL".to_string(),
                        field_id: "field-north".to_string(),
                        geometry: ApplicationGeometry {
                            crs: "EPSG:4326".to_string(),
                            coordinates: square_zone(),
                        },
                        applied_at: "2026-06-12T13:00:00Z".to_string(),
                        rate: 0.0,
                        units: "L/ha".to_string(),
                        operator_id: "operator-17".to_string(),
                    },
                )),
            },
            "generated-record".to_string(),
            "2026-06-12T13:01:00Z".to_string(),
        )
        .expect_err("missing rate should be rejected");
        assert_eq!(missing_rate, ComplianceRecordError::InvalidApplicationRate);
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

    #[test]
    fn audit_report_includes_required_records_and_provenance() {
        let records = vec![
            compliance_record(
                "remote-log-1",
                ComplianceRecordType::RemoteIdLog,
                Some("flight-77"),
                "provenance:remote-id/remote-log-1/v1",
                Some(ComplianceRecordPayload::RemoteIdFlightLog(remote_id_log())),
            ),
            compliance_record(
                "chem-app-1",
                ComplianceRecordType::ChemicalApplication,
                Some("flight-77"),
                "provenance:application/chem-app-1/v1",
                Some(ComplianceRecordPayload::ChemicalApplication(
                    application_record(),
                )),
            ),
            compliance_record(
                "cert-operator-17",
                ComplianceRecordType::OperatorCertification,
                None,
                "provenance:cert/operator-17/v1",
                None,
            ),
            compliance_record(
                "auth-flight-77",
                ComplianceRecordType::AuthorizationDecision,
                Some("flight-77"),
                "provenance:authorization/flight-77/v1",
                None,
            ),
        ];

        let report = build_compliance_audit_report(ComplianceAuditReportRequest {
            report_id: "report-field-north".to_string(),
            org_id: "org-alpha".to_string(),
            field_id: "field-north".to_string(),
            generated_at: "2026-06-13T12:00:00Z".to_string(),
            records,
            mandatory_record_types: vec![
                ComplianceRecordType::RemoteIdLog,
                ComplianceRecordType::ChemicalApplication,
                ComplianceRecordType::OperatorCertification,
                ComplianceRecordType::AuthorizationDecision,
            ],
        })
        .expect("complete record set should produce an audit report");

        assert_eq!(report.schema_version, "compliance.audit_report.v1");
        assert_eq!(report.report_id, "report-field-north");
        assert_eq!(report.record_count, 4);
        assert_eq!(
            report.record_type_counts.get("remote_id_log").copied(),
            Some(1)
        );
        assert!(report
            .provenance_refs
            .contains(&"provenance:application/chem-app-1/v1".to_string()));
        assert_eq!(report.records[0].org_id, "org-alpha");
        assert_eq!(report.records[0].field_id, "field-north");
    }

    #[test]
    fn audit_report_rejects_missing_mandatory_records() {
        let error = build_compliance_audit_report(ComplianceAuditReportRequest {
            report_id: "report-field-north".to_string(),
            org_id: "org-alpha".to_string(),
            field_id: "field-north".to_string(),
            generated_at: "2026-06-13T12:00:00Z".to_string(),
            records: vec![compliance_record(
                "remote-log-1",
                ComplianceRecordType::RemoteIdLog,
                Some("flight-77"),
                "provenance:remote-id/remote-log-1/v1",
                Some(ComplianceRecordPayload::RemoteIdFlightLog(remote_id_log())),
            )],
            mandatory_record_types: vec![
                ComplianceRecordType::RemoteIdLog,
                ComplianceRecordType::ChemicalApplication,
            ],
        })
        .expect_err("missing mandatory records should fail export");

        assert_eq!(
            error,
            ComplianceAuditReportError::MissingMandatoryRecords {
                missing: vec![ComplianceRecordType::ChemicalApplication]
            }
        );
    }

    fn compliance_record(
        record_id: &str,
        record_type: ComplianceRecordType,
        flight_id: Option<&str>,
        provenance_ref: &str,
        payload: Option<ComplianceRecordPayload>,
    ) -> super::ComplianceRecord {
        build_initial_compliance_record(
            CreateComplianceRecordRequest {
                record_id: Some(record_id.to_string()),
                record_type,
                org_id: "org-alpha".to_string(),
                field_id: "field-north".to_string(),
                flight_id: flight_id.map(ToOwned::to_owned),
                actor: "compliance-officer-1".to_string(),
                provenance_ref: provenance_ref.to_string(),
                payload,
            },
            "generated-record".to_string(),
            "2026-06-13T12:00:00Z".to_string(),
        )
        .expect("compliance record should be valid")
    }

    fn remote_id_log() -> RemoteIdFlightLogRecord {
        RemoteIdFlightLogRecord {
            flight_id: "flight-77".to_string(),
            operator_id: "operator-17".to_string(),
            aircraft_id: "aircraft-ag-9".to_string(),
            started_at: "2026-06-12T12:00:00Z".to_string(),
            ended_at: "2026-06-12T12:18:00Z".to_string(),
            track: vec![RemoteIdTrackPoint {
                observed_at: "2026-06-12T12:02:00Z".to_string(),
                longitude: -96.61,
                latitude: 41.21,
                altitude_m: 118.0,
            }],
            telemetry_gaps: vec![TelemetryGapRecord {
                started_at: "2026-06-12T12:04:00Z".to_string(),
                ended_at: "2026-06-12T12:08:00Z".to_string(),
                reason: "remote-id-broadcast-dropout".to_string(),
            }],
        }
    }

    fn operator_cert(
        cert_id: &str,
        operator_id: &str,
        cert_type: &str,
        issued_at: &str,
        expires_at: &str,
    ) -> super::OperatorCertificationRecord {
        build_operator_certification_record(
            OperatorCertificationRegistrationRequest {
                cert_id: Some(cert_id.to_string()),
                operator_id: operator_id.to_string(),
                cert_type: cert_type.to_string(),
                issued_at: issued_at.to_string(),
                expires_at: expires_at.to_string(),
                authority: "FAA".to_string(),
            },
            "generated-cert".to_string(),
            "2026-06-12T12:00:00Z".to_string(),
        )
        .expect("operator cert should be valid")
    }

    fn application_record() -> ChemicalApplicationRecord {
        ChemicalApplicationRecord {
            application_id: "chem-app-1".to_string(),
            product: "Example Herbicide".to_string(),
            epa_or_label_ref: "EPA-12345-LBL".to_string(),
            field_id: "field-north".to_string(),
            geometry: ApplicationGeometry {
                crs: "EPSG:4326".to_string(),
                coordinates: square_zone(),
            },
            applied_at: "2026-06-12T13:00:00Z".to_string(),
            rate: 1.75,
            units: "L/ha".to_string(),
            operator_id: "operator-17".to_string(),
        }
    }

    fn preflight_request(
        airspace_status: PreflightAirspaceStatus,
        planned_area: Vec<AirspaceCoordinate>,
    ) -> PreflightAuthorizationRequest {
        PreflightAuthorizationRequest {
            flight_id: "flight-77".to_string(),
            operator_id: "operator-17".to_string(),
            required_cert_type: "part-107".to_string(),
            planned_at: "2026-06-12T12:00:00Z".to_string(),
            planned_area,
            airspace_status,
        }
    }

    fn base_zone_request() -> AirspaceZoneIngestRequest {
        AirspaceZoneIngestRequest {
            zone_id: Some("zone-1".to_string()),
            zone_class: AirspaceZoneClass::NoFly,
            crs: "EPSG:4326".to_string(),
            coordinates: square_zone(),
            effective_from: "2026-06-01T00:00:00Z".to_string(),
            effective_to: None,
            source: "faa-uasfm-2026-06".to_string(),
        }
    }

    fn clear_flight_area() -> Vec<AirspaceCoordinate> {
        vec![
            AirspaceCoordinate {
                longitude: -97.20,
                latitude: 41.00,
            },
            AirspaceCoordinate {
                longitude: -97.10,
                latitude: 41.00,
            },
            AirspaceCoordinate {
                longitude: -97.10,
                latitude: 41.10,
            },
            AirspaceCoordinate {
                longitude: -97.20,
                latitude: 41.00,
            },
        ]
    }

    fn intersecting_flight_area() -> Vec<AirspaceCoordinate> {
        vec![
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
        ]
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
