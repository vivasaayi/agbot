use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitiveFeatureType {
    Water,
    Dwelling,
    OrganicField,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensitiveBufferFeature {
    pub feature_ref: String,
    pub feature_type: SensitiveFeatureType,
    pub geometry: ApplicationGeometry,
    pub required_buffer_m: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SprayBufferDecisionStatus {
    Compliant,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SprayBufferBlockReason {
    BufferBreach,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SprayBufferComplianceDecision {
    pub application_id: String,
    pub checked_at: String,
    pub status: SprayBufferDecisionStatus,
    pub reason_code: Option<SprayBufferBlockReason>,
    pub feature_ref: Option<String>,
    pub required_buffer_m: Option<f64>,
    pub actual_separation_m: Option<f64>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceCheckKind {
    Authorization,
    ReiPhi,
    SprayBuffer,
}

impl ComplianceCheckKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ComplianceCheckKind::Authorization => "authorization",
            ComplianceCheckKind::ReiPhi => "rei_phi",
            ComplianceCheckKind::SprayBuffer => "spray_buffer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceEvidenceInput {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceEvidenceRequest {
    pub check_id: String,
    pub check_kind: ComplianceCheckKind,
    pub rule_version: String,
    pub evaluated_at: String,
    pub decision_status: String,
    #[serde(default)]
    pub reason_code: Option<String>,
    #[serde(default)]
    pub input_refs: Vec<String>,
    #[serde(default)]
    pub raw_inputs: Vec<ComplianceEvidenceInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceEvidenceRecord {
    pub check_id: String,
    pub version: u32,
    pub check_kind: ComplianceCheckKind,
    pub rule_version: String,
    pub evaluated_at: String,
    pub decision_status: String,
    pub reason_code: Option<String>,
    pub input_refs: Vec<String>,
    pub raw_inputs: Vec<ComplianceEvidenceInput>,
    pub decision_hash: String,
    pub prior_decision_hash: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceAlertSeverity {
    Info,
    Warning,
    Critical,
}

impl ComplianceAlertSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            ComplianceAlertSeverity::Info => "info",
            ComplianceAlertSeverity::Warning => "warning",
            ComplianceAlertSeverity::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceAlertEvent {
    pub source_domain: String,
    pub event_type: String,
    pub subject_ref: String,
    pub severity_hint: ComplianceAlertSeverity,
    pub evidence_refs: Vec<String>,
    pub occurred_at: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceRecordSourceStatus {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceRecordSourceHealth {
    pub source_ref: String,
    pub status: ComplianceRecordSourceStatus,
    pub checked_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceDeadlineRecord {
    pub deadline_ref: String,
    pub due_at: String,
    pub evidence_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceAlertScheduleRequest {
    pub checked_at: String,
    pub lead_hours: i64,
    #[serde(default)]
    pub certifications: Vec<OperatorCertificationRecord>,
    #[serde(default)]
    pub clearance_windows: Vec<ReiPhiWindow>,
    #[serde(default)]
    pub filing_deadlines: Vec<ComplianceDeadlineRecord>,
    #[serde(default)]
    pub source_health: Vec<ComplianceRecordSourceHealth>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceRetentionClass {
    FlightSafety,
    ChemicalApplication,
    AuditEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceRecordStoragePlan {
    pub record_id: String,
    pub residency_tag: String,
    pub storage_region: String,
    pub retention_class: ComplianceRetentionClass,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceRetentionPolicyRule {
    pub residency_tag: String,
    pub retention_class: ComplianceRetentionClass,
    pub allowed_storage_regions: Vec<String>,
    pub min_retention_days: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompliancePolicyDecisionStatus {
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompliancePolicyBlockReason {
    ResidencyRegionMismatch,
    RetentionPeriodActive,
    MissingPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompliancePolicyDecision {
    pub record_id: String,
    pub checked_at: String,
    pub status: CompliancePolicyDecisionStatus,
    pub reason_code: Option<CompliancePolicyBlockReason>,
    pub residency_tag: String,
    pub storage_region: String,
    pub retention_class: ComplianceRetentionClass,
    pub min_retention_until: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceAuthorityFormat {
    FaaRemoteId,
    StatePesticideApplication,
}

impl ComplianceAuthorityFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FaaRemoteId => "faa_remote_id",
            Self::StatePesticideApplication => "state_pesticide_application",
        }
    }

    fn required_record_type(self) -> ComplianceRecordType {
        match self {
            Self::FaaRemoteId => ComplianceRecordType::RemoteIdLog,
            Self::StatePesticideApplication => ComplianceRecordType::ChemicalApplication,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplianceAuthorityExportRequest {
    pub authority_format: ComplianceAuthorityFormat,
    pub report: ComplianceAuditReport,
    pub generated_at: String,
    pub residency_tag: String,
    pub storage_region: String,
    pub retention_class: ComplianceRetentionClass,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplianceAuthorityExportArtifact {
    pub schema_version: String,
    pub authority_format: ComplianceAuthorityFormat,
    pub report_id: String,
    pub org_id: String,
    pub field_id: String,
    pub generated_at: String,
    pub residency_tag: String,
    pub storage_region: String,
    pub retention_class: ComplianceRetentionClass,
    pub content_type: String,
    pub file_name: String,
    pub included_record_ids: Vec<String>,
    pub provenance_refs: Vec<String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplianceAuthorityShareRequest {
    pub share_id: String,
    pub export: ComplianceAuthorityExportArtifact,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplianceAuthorityShareArtifact {
    pub share_id: String,
    pub report_id: String,
    pub authority_format: ComplianceAuthorityFormat,
    pub url_path: String,
    pub created_at: String,
    pub expires_at: String,
    #[serde(default)]
    pub revoked_at: Option<String>,
    pub residency_tag: String,
    pub storage_region: String,
    pub retention_class: ComplianceRetentionClass,
    pub revocable: bool,
    pub export: ComplianceAuthorityExportArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceRegulationAssistIntent {
    Summary,
    DraftFilingText,
    AuthorizeFlight,
    ClearViolation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplianceRuleCitation {
    pub rule_ref: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplianceRegulationAssistRequest {
    pub assist_id: String,
    pub intent: ComplianceRegulationAssistIntent,
    pub report: ComplianceAuditReport,
    pub generated_at: String,
    #[serde(default)]
    pub rule_citations: Vec<ComplianceRuleCitation>,
    pub feature_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplianceRegulationAssistOutput {
    pub assist_id: String,
    pub intent: ComplianceRegulationAssistIntent,
    pub report_id: String,
    pub generated_at: String,
    pub summary: String,
    pub draft_filing_text: String,
    pub uncertainty_flag: bool,
    pub uncertainty_reasons: Vec<String>,
    pub source_record_refs: Vec<String>,
    pub rule_refs: Vec<String>,
    pub can_authorize: bool,
    pub can_clear_violation: bool,
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
pub enum ComplianceAuthorityExportError {
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("residency_tag cannot be empty")]
    EmptyResidencyTag,
    #[error("storage_region cannot be empty")]
    EmptyStorageRegion,
    #[error("authority export requires at least one {record_type} record")]
    MissingAuthorityRecord { record_type: ComplianceRecordType },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ComplianceAuthorityShareError {
    #[error("share_id cannot be empty")]
    EmptyShareId,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("expires_at cannot be empty")]
    EmptyExpiresAt,
    #[error("revoked_at cannot be empty")]
    EmptyRevokedAt,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ComplianceRegulationAssistError {
    #[error("assist_id cannot be empty")]
    EmptyAssistId,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("rule citation ref cannot be empty")]
    EmptyRuleRef,
    #[error("rule citation title cannot be empty")]
    EmptyRuleTitle,
    #[error("regulation assist feature flag is disabled")]
    FeatureDisabled,
    #[error("regulation assist cannot authorize flights or clear violations; use the deterministic gate")]
    DeterministicGateRequired,
    #[error("regulation assist requires at least one rule citation")]
    EmptyRuleCitations,
    #[error("regulation assist requires source compliance records")]
    EmptySourceRecords,
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

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SprayBufferComplianceError {
    #[error("checked_at cannot be empty")]
    EmptyCheckedAt,
    #[error("sensitive feature_ref cannot be empty")]
    EmptyFeatureRef,
    #[error("required buffer must be finite and zero or greater")]
    InvalidRequiredBuffer,
    #[error("application geometry is invalid: {source}")]
    InvalidApplicationGeometry {
        #[source]
        source: ComplianceRecordError,
    },
    #[error("sensitive feature geometry is invalid: {source}")]
    InvalidFeatureGeometry {
        #[source]
        source: ComplianceRecordError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ComplianceEvidenceError {
    #[error("check_id cannot be empty")]
    EmptyCheckId,
    #[error("rule_version cannot be empty")]
    EmptyRuleVersion,
    #[error("evaluated_at cannot be empty")]
    EmptyEvaluatedAt,
    #[error("decision_status cannot be empty")]
    EmptyDecisionStatus,
    #[error("reason_code cannot be empty")]
    EmptyReasonCode,
    #[error("raw input key cannot be empty")]
    EmptyInputKey,
    #[error("raw input value cannot be empty")]
    EmptyInputValue,
    #[error("input_refs cannot contain an empty value")]
    EmptyInputRef,
    #[error("compliance evidence is append-only; cannot overwrite version {version}")]
    AppendOnlyOverwriteRefused { version: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ComplianceAlertScheduleError {
    #[error("checked_at cannot be empty")]
    EmptyCheckedAt,
    #[error("lead_hours must be zero or greater")]
    InvalidLeadHours,
    #[error("timestamp is not valid RFC3339")]
    InvalidTimestamp,
    #[error("source_ref cannot be empty")]
    EmptySourceRef,
    #[error("deadline_ref cannot be empty")]
    EmptyDeadlineRef,
    #[error("deadline evidence_ref cannot be empty")]
    EmptyDeadlineEvidenceRef,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompliancePolicyError {
    #[error("record_id cannot be empty")]
    EmptyRecordId,
    #[error("residency_tag cannot be empty")]
    EmptyResidencyTag,
    #[error("storage_region cannot be empty")]
    EmptyStorageRegion,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("checked_at cannot be empty")]
    EmptyCheckedAt,
    #[error("allowed_storage_regions cannot contain an empty value")]
    EmptyAllowedStorageRegion,
    #[error("min_retention_days must be zero or greater")]
    InvalidMinRetentionDays,
    #[error("timestamp is not valid RFC3339")]
    InvalidTimestamp,
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

pub fn build_compliance_authority_export(
    request: ComplianceAuthorityExportRequest,
) -> Result<ComplianceAuthorityExportArtifact, ComplianceAuthorityExportError> {
    let generated_at = normalize_authority_export_text(
        request.generated_at,
        ComplianceAuthorityExportError::EmptyGeneratedAt,
    )?;
    let residency_tag = normalize_authority_export_text(
        request.residency_tag,
        ComplianceAuthorityExportError::EmptyResidencyTag,
    )?;
    let storage_region = normalize_authority_export_text(
        request.storage_region,
        ComplianceAuthorityExportError::EmptyStorageRegion,
    )?;
    let required_record_type = request.authority_format.required_record_type();
    let records = request
        .report
        .records
        .iter()
        .filter(|record| record.record_type == required_record_type)
        .collect::<Vec<_>>();
    if records.is_empty() {
        return Err(ComplianceAuthorityExportError::MissingAuthorityRecord {
            record_type: required_record_type,
        });
    }

    let included_record_ids = records
        .iter()
        .map(|record| record.record_id.clone())
        .collect::<Vec<_>>();
    let payload_records = records
        .iter()
        .map(|record| {
            json!({
                "record_id": record.record_id,
                "version": record.version,
                "flight_id": record.flight_id,
                "provenance_ref": record.provenance_ref,
                "payload": record.payload,
            })
        })
        .collect::<Vec<_>>();
    let payload = match request.authority_format {
        ComplianceAuthorityFormat::FaaRemoteId => json!({
            "authority": "faa",
            "format": request.authority_format.as_str(),
            "report_id": request.report.report_id,
            "remote_id_logs": payload_records,
        }),
        ComplianceAuthorityFormat::StatePesticideApplication => json!({
            "authority": "state_pesticide_regulator",
            "format": request.authority_format.as_str(),
            "report_id": request.report.report_id,
            "chemical_applications": payload_records,
        }),
    };

    Ok(ComplianceAuthorityExportArtifact {
        schema_version: "compliance.authority_export.v1".to_string(),
        authority_format: request.authority_format,
        report_id: request.report.report_id.clone(),
        org_id: request.report.org_id.clone(),
        field_id: request.report.field_id.clone(),
        generated_at,
        residency_tag,
        storage_region,
        retention_class: request.retention_class,
        content_type: "application/json".to_string(),
        file_name: format!(
            "{}-{}.json",
            request.report.report_id,
            request.authority_format.as_str()
        ),
        included_record_ids,
        provenance_refs: request.report.provenance_refs.clone(),
        payload,
    })
}

pub fn build_compliance_authority_share(
    request: ComplianceAuthorityShareRequest,
) -> Result<ComplianceAuthorityShareArtifact, ComplianceAuthorityShareError> {
    let share_id = normalize_authority_share_text(
        request.share_id,
        ComplianceAuthorityShareError::EmptyShareId,
    )?;
    let created_at = normalize_authority_share_text(
        request.created_at,
        ComplianceAuthorityShareError::EmptyCreatedAt,
    )?;
    let expires_at = normalize_authority_share_text(
        request.expires_at,
        ComplianceAuthorityShareError::EmptyExpiresAt,
    )?;

    Ok(ComplianceAuthorityShareArtifact {
        url_path: format!("/api/compliance/authority-shares/{share_id}"),
        share_id,
        report_id: request.export.report_id.clone(),
        authority_format: request.export.authority_format,
        created_at,
        expires_at,
        revoked_at: None,
        residency_tag: request.export.residency_tag.clone(),
        storage_region: request.export.storage_region.clone(),
        retention_class: request.export.retention_class,
        revocable: true,
        export: request.export,
    })
}

pub fn revoke_compliance_authority_share(
    mut share: ComplianceAuthorityShareArtifact,
    revoked_at: String,
) -> Result<ComplianceAuthorityShareArtifact, ComplianceAuthorityShareError> {
    share.revoked_at = Some(normalize_authority_share_text(
        revoked_at,
        ComplianceAuthorityShareError::EmptyRevokedAt,
    )?);
    Ok(share)
}

pub fn build_compliance_regulation_assist(
    request: ComplianceRegulationAssistRequest,
) -> Result<ComplianceRegulationAssistOutput, ComplianceRegulationAssistError> {
    if !request.feature_enabled {
        return Err(ComplianceRegulationAssistError::FeatureDisabled);
    }
    match request.intent {
        ComplianceRegulationAssistIntent::Summary
        | ComplianceRegulationAssistIntent::DraftFilingText => {}
        ComplianceRegulationAssistIntent::AuthorizeFlight
        | ComplianceRegulationAssistIntent::ClearViolation => {
            return Err(ComplianceRegulationAssistError::DeterministicGateRequired);
        }
    }

    let assist_id = normalize_regulation_assist_text(
        request.assist_id,
        ComplianceRegulationAssistError::EmptyAssistId,
    )?;
    let generated_at = normalize_regulation_assist_text(
        request.generated_at,
        ComplianceRegulationAssistError::EmptyGeneratedAt,
    )?;
    if request.report.records.is_empty() {
        return Err(ComplianceRegulationAssistError::EmptySourceRecords);
    }
    if request.rule_citations.is_empty() {
        return Err(ComplianceRegulationAssistError::EmptyRuleCitations);
    }

    let mut rule_refs = Vec::new();
    let mut rule_titles = Vec::new();
    for citation in request.rule_citations {
        let rule_ref = normalize_regulation_assist_text(
            citation.rule_ref,
            ComplianceRegulationAssistError::EmptyRuleRef,
        )?;
        let title = normalize_regulation_assist_text(
            citation.title,
            ComplianceRegulationAssistError::EmptyRuleTitle,
        )?;
        rule_refs.push(rule_ref);
        rule_titles.push(title);
    }
    rule_refs.sort();
    rule_refs.dedup();
    rule_titles.sort();
    rule_titles.dedup();

    let mut source_record_refs = request
        .report
        .records
        .iter()
        .map(|record| format!("compliance_record:{}@v{}", record.record_id, record.version))
        .collect::<Vec<_>>();
    source_record_refs.sort();
    source_record_refs.dedup();

    let mut uncertainty_reasons = Vec::new();
    for mandatory in [
        ComplianceRecordType::RemoteIdLog,
        ComplianceRecordType::ChemicalApplication,
        ComplianceRecordType::OperatorCertification,
        ComplianceRecordType::AuthorizationDecision,
    ] {
        if !request
            .report
            .record_type_counts
            .contains_key(mandatory.as_str())
        {
            uncertainty_reasons.push(format!("missing_{}", mandatory.as_str()));
        }
    }
    if rule_refs.is_empty() {
        uncertainty_reasons.push("missing_rule_refs".to_string());
    }
    let uncertainty_flag = !uncertainty_reasons.is_empty();

    let record_types = request
        .report
        .record_type_counts
        .iter()
        .map(|(record_type, count)| format!("{record_type}:{count}"))
        .collect::<Vec<_>>()
        .join(", ");
    let summary = format!(
        "Compliance report {} for org {} field {} includes {} records ({}) and cites {} provenance refs. Deterministic gates remain authoritative.",
        request.report.report_id,
        request.report.org_id,
        request.report.field_id,
        request.report.record_count,
        record_types,
        request.report.provenance_refs.len()
    );
    let draft_filing_text = format!(
        "Draft filing for {}: submit records [{}] with rule citations [{}]. This draft does not authorize operations or clear violations.",
        request.report.field_id,
        source_record_refs.join(", "),
        rule_titles.join("; ")
    );

    Ok(ComplianceRegulationAssistOutput {
        assist_id,
        intent: request.intent,
        report_id: request.report.report_id,
        generated_at,
        summary,
        draft_filing_text,
        uncertainty_flag,
        uncertainty_reasons,
        source_record_refs,
        rule_refs,
        can_authorize: false,
        can_clear_violation: false,
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

pub fn evaluate_spray_buffer_compliance(
    application: &ChemicalApplicationRecord,
    sensitive_features: Vec<SensitiveBufferFeature>,
    checked_at: String,
) -> Result<SprayBufferComplianceDecision, SprayBufferComplianceError> {
    let checked_at =
        normalize_spray_buffer_text(checked_at, SprayBufferComplianceError::EmptyCheckedAt)?;
    let application_geometry = validate_application_geometry(application.geometry.clone())
        .map_err(|source| SprayBufferComplianceError::InvalidApplicationGeometry { source })?;
    let application_extent = extent_from_coordinates(&application_geometry.coordinates);

    let mut closest_breach = None::<(String, f64, f64)>;
    for feature in sensitive_features {
        let feature_ref = normalize_spray_buffer_text(
            feature.feature_ref,
            SprayBufferComplianceError::EmptyFeatureRef,
        )?;
        if !feature.required_buffer_m.is_finite() || feature.required_buffer_m < 0.0 {
            return Err(SprayBufferComplianceError::InvalidRequiredBuffer);
        }
        let feature_geometry = validate_application_geometry(feature.geometry)
            .map_err(|source| SprayBufferComplianceError::InvalidFeatureGeometry { source })?;
        let feature_extent = extent_from_coordinates(&feature_geometry.coordinates);
        let intersects = polygons_intersect(
            &application_geometry.coordinates,
            &feature_geometry.coordinates,
        );
        let separation_m = if intersects {
            0.0
        } else {
            extent_separation_m(application_extent, feature_extent)
        };
        if separation_m < feature.required_buffer_m {
            match &closest_breach {
                Some((_, _, current_actual)) if *current_actual <= separation_m => {}
                _ => {
                    closest_breach = Some((feature_ref, feature.required_buffer_m, separation_m));
                }
            }
        }
    }

    if let Some((feature_ref, required_buffer_m, actual_separation_m)) = closest_breach {
        Ok(SprayBufferComplianceDecision {
            application_id: application.application_id.clone(),
            checked_at,
            status: SprayBufferDecisionStatus::Blocked,
            reason_code: Some(SprayBufferBlockReason::BufferBreach),
            feature_ref: Some(feature_ref),
            required_buffer_m: Some(required_buffer_m),
            actual_separation_m: Some(actual_separation_m),
        })
    } else {
        Ok(SprayBufferComplianceDecision {
            application_id: application.application_id.clone(),
            checked_at,
            status: SprayBufferDecisionStatus::Compliant,
            reason_code: None,
            feature_ref: None,
            required_buffer_m: None,
            actual_separation_m: None,
        })
    }
}

pub fn build_compliance_evidence_record(
    request: ComplianceEvidenceRequest,
) -> Result<ComplianceEvidenceRecord, ComplianceEvidenceError> {
    build_compliance_evidence_record_version(request, 1, None)
}

pub fn append_compliance_evidence_record(
    latest: &ComplianceEvidenceRecord,
    request: ComplianceEvidenceRequest,
) -> Result<ComplianceEvidenceRecord, ComplianceEvidenceError> {
    build_compliance_evidence_record_version(
        request,
        latest.version + 1,
        Some(latest.decision_hash.clone()),
    )
}

pub fn refuse_compliance_evidence_overwrite(
    existing: &ComplianceEvidenceRecord,
) -> ComplianceEvidenceError {
    ComplianceEvidenceError::AppendOnlyOverwriteRefused {
        version: existing.version,
    }
}

pub fn schedule_compliance_alerts(
    request: ComplianceAlertScheduleRequest,
) -> Result<Vec<ComplianceAlertEvent>, ComplianceAlertScheduleError> {
    let checked_at_text = normalize_alert_text(
        request.checked_at,
        ComplianceAlertScheduleError::EmptyCheckedAt,
    )?;
    if request.lead_hours < 0 {
        return Err(ComplianceAlertScheduleError::InvalidLeadHours);
    }
    let checked_at = parse_alert_time(&checked_at_text)?;
    let lead_until = checked_at + Duration::hours(request.lead_hours);
    let mut events = Vec::new();

    for certification in request.certifications {
        let expires_at = parse_alert_time(&certification.expires_at)?;
        if expires_at >= checked_at && expires_at <= lead_until {
            events.push(compliance_alert_event(
                "compliance.cert_expiring",
                format!("operator_certification:{}", certification.cert_id),
                ComplianceAlertSeverity::Warning,
                vec![format!("certification:{}", certification.cert_id)],
                checked_at_text.clone(),
            ));
        }
    }

    for window in request.clearance_windows {
        if let Some(rei_clear_at) = &window.rei_clear_at {
            let clear_at = parse_alert_time(rei_clear_at)?;
            if clear_at >= checked_at && clear_at <= lead_until {
                events.push(compliance_alert_event(
                    "compliance.rei_clearance_due",
                    format!("application:{}", window.application_id),
                    ComplianceAlertSeverity::Info,
                    vec![window.source_application_ref.clone()],
                    checked_at_text.clone(),
                ));
            }
        }
        if let Some(phi_clear_at) = &window.phi_clear_at {
            let clear_at = parse_alert_time(phi_clear_at)?;
            if clear_at >= checked_at && clear_at <= lead_until {
                events.push(compliance_alert_event(
                    "compliance.phi_clearance_due",
                    format!("application:{}", window.application_id),
                    ComplianceAlertSeverity::Info,
                    vec![window.source_application_ref],
                    checked_at_text.clone(),
                ));
            }
        }
    }

    for deadline in request.filing_deadlines {
        let deadline_ref = normalize_alert_text(
            deadline.deadline_ref,
            ComplianceAlertScheduleError::EmptyDeadlineRef,
        )?;
        let evidence_ref = normalize_alert_text(
            deadline.evidence_ref,
            ComplianceAlertScheduleError::EmptyDeadlineEvidenceRef,
        )?;
        let due_at = parse_alert_time(&deadline.due_at)?;
        if due_at >= checked_at && due_at <= lead_until {
            events.push(compliance_alert_event(
                "compliance.deadline_due",
                format!("deadline:{deadline_ref}"),
                ComplianceAlertSeverity::Warning,
                vec![evidence_ref],
                checked_at_text.clone(),
            ));
        }
    }

    for source in request.source_health {
        let source_ref = normalize_alert_text(
            source.source_ref,
            ComplianceAlertScheduleError::EmptySourceRef,
        )?;
        parse_alert_time(&source.checked_at)?;
        if source.status == ComplianceRecordSourceStatus::Unavailable {
            events.push(compliance_alert_event(
                "compliance.source_unavailable",
                format!("source:{source_ref}"),
                ComplianceAlertSeverity::Critical,
                vec![format!("source:{source_ref}")],
                checked_at_text.clone(),
            ));
        }
    }

    events.sort_by(|left, right| {
        left.event_type
            .cmp(&right.event_type)
            .then_with(|| left.subject_ref.cmp(&right.subject_ref))
            .then_with(|| left.idempotency_key.cmp(&right.idempotency_key))
    });
    Ok(events)
}

pub fn evaluate_compliance_record_storage(
    plan: ComplianceRecordStoragePlan,
    policies: &[ComplianceRetentionPolicyRule],
    checked_at: String,
) -> Result<CompliancePolicyDecision, CompliancePolicyError> {
    let plan = normalize_storage_plan(plan)?;
    let checked_at = normalize_policy_text(checked_at, CompliancePolicyError::EmptyCheckedAt)?;
    parse_policy_time(&checked_at)?;
    let Some(policy) = find_retention_policy(&plan, policies)? else {
        return Ok(policy_decision(
            plan,
            checked_at,
            CompliancePolicyDecisionStatus::Blocked,
            Some(CompliancePolicyBlockReason::MissingPolicy),
            None,
        ));
    };
    let min_retention_until = min_retention_until(&plan.created_at, policy.min_retention_days)?;
    if !policy
        .allowed_storage_regions
        .iter()
        .any(|region| region == &plan.storage_region)
    {
        return Ok(policy_decision(
            plan,
            checked_at,
            CompliancePolicyDecisionStatus::Blocked,
            Some(CompliancePolicyBlockReason::ResidencyRegionMismatch),
            Some(min_retention_until),
        ));
    }

    Ok(policy_decision(
        plan,
        checked_at,
        CompliancePolicyDecisionStatus::Allowed,
        None,
        Some(min_retention_until),
    ))
}

pub fn evaluate_compliance_record_deletion(
    plan: ComplianceRecordStoragePlan,
    policies: &[ComplianceRetentionPolicyRule],
    checked_at: String,
) -> Result<CompliancePolicyDecision, CompliancePolicyError> {
    let plan = normalize_storage_plan(plan)?;
    let checked_at = normalize_policy_text(checked_at, CompliancePolicyError::EmptyCheckedAt)?;
    let checked_at_time = parse_policy_time(&checked_at)?;
    let Some(policy) = find_retention_policy(&plan, policies)? else {
        return Ok(policy_decision(
            plan,
            checked_at,
            CompliancePolicyDecisionStatus::Blocked,
            Some(CompliancePolicyBlockReason::MissingPolicy),
            None,
        ));
    };
    let min_retention_until = min_retention_until(&plan.created_at, policy.min_retention_days)?;
    let min_retention_time = parse_policy_time(&min_retention_until)?;
    if checked_at_time < min_retention_time {
        return Ok(policy_decision(
            plan,
            checked_at,
            CompliancePolicyDecisionStatus::Blocked,
            Some(CompliancePolicyBlockReason::RetentionPeriodActive),
            Some(min_retention_until),
        ));
    }

    Ok(policy_decision(
        plan,
        checked_at,
        CompliancePolicyDecisionStatus::Allowed,
        None,
        Some(min_retention_until),
    ))
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

fn normalize_authority_export_text(
    value: String,
    error: ComplianceAuthorityExportError,
) -> Result<String, ComplianceAuthorityExportError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_authority_share_text(
    value: String,
    error: ComplianceAuthorityShareError,
) -> Result<String, ComplianceAuthorityShareError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_regulation_assist_text(
    value: String,
    error: ComplianceRegulationAssistError,
) -> Result<String, ComplianceRegulationAssistError> {
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

fn extent_from_coordinates(coordinates: &[AirspaceCoordinate]) -> AirspaceZoneExtent {
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
    extent
}

fn polygons_intersect(left: &[AirspaceCoordinate], right: &[AirspaceCoordinate]) -> bool {
    if left
        .iter()
        .copied()
        .any(|point| point_in_polygon(point, right))
    {
        return true;
    }
    if right
        .iter()
        .copied()
        .any(|point| point_in_polygon(point, left))
    {
        return true;
    }
    for left_edge in left.windows(2) {
        for right_edge in right.windows(2) {
            if segments_intersect(left_edge[0], left_edge[1], right_edge[0], right_edge[1]) {
                return true;
            }
        }
    }
    false
}

fn extent_separation_m(left: AirspaceZoneExtent, right: AirspaceZoneExtent) -> f64 {
    let lon_gap = if left.max_lon < right.min_lon {
        right.min_lon - left.max_lon
    } else if right.max_lon < left.min_lon {
        left.min_lon - right.max_lon
    } else {
        0.0
    };
    let lat_gap = if left.max_lat < right.min_lat {
        right.min_lat - left.max_lat
    } else if right.max_lat < left.min_lat {
        left.min_lat - right.max_lat
    } else {
        0.0
    };
    let mean_lat_rad =
        ((left.min_lat + left.max_lat + right.min_lat + right.max_lat) / 4.0).to_radians();
    let meters_per_degree_lon = 111_320.0 * mean_lat_rad.cos().abs().max(0.01);
    let meters_per_degree_lat = 110_540.0;
    let dx = lon_gap * meters_per_degree_lon;
    let dy = lat_gap * meters_per_degree_lat;
    (dx * dx + dy * dy).sqrt()
}

fn normalize_spray_buffer_text(
    value: String,
    error: SprayBufferComplianceError,
) -> Result<String, SprayBufferComplianceError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn build_compliance_evidence_record_version(
    request: ComplianceEvidenceRequest,
    version: u32,
    prior_decision_hash: Option<String>,
) -> Result<ComplianceEvidenceRecord, ComplianceEvidenceError> {
    let check_id =
        normalize_evidence_text(request.check_id, ComplianceEvidenceError::EmptyCheckId)?;
    let rule_version = normalize_evidence_text(
        request.rule_version,
        ComplianceEvidenceError::EmptyRuleVersion,
    )?;
    let evaluated_at = normalize_evidence_text(
        request.evaluated_at,
        ComplianceEvidenceError::EmptyEvaluatedAt,
    )?;
    let decision_status = normalize_evidence_text(
        request.decision_status,
        ComplianceEvidenceError::EmptyDecisionStatus,
    )?;
    let reason_code = request
        .reason_code
        .map(|value| normalize_evidence_text(value, ComplianceEvidenceError::EmptyReasonCode))
        .transpose()?;
    let mut input_refs = Vec::new();
    for input_ref in request.input_refs {
        input_refs.push(normalize_evidence_text(
            input_ref,
            ComplianceEvidenceError::EmptyInputRef,
        )?);
    }
    input_refs.sort();
    input_refs.dedup();

    let mut raw_inputs = Vec::new();
    for input in request.raw_inputs {
        raw_inputs.push(ComplianceEvidenceInput {
            key: normalize_evidence_text(input.key, ComplianceEvidenceError::EmptyInputKey)?,
            value: normalize_evidence_text(input.value, ComplianceEvidenceError::EmptyInputValue)?,
        });
    }
    raw_inputs.sort_by(|left, right| {
        left.key
            .cmp(&right.key)
            .then_with(|| left.value.cmp(&right.value))
    });
    raw_inputs.dedup();

    let decision_hash = compliance_decision_hash(
        request.check_kind,
        &rule_version,
        &decision_status,
        reason_code.as_deref(),
        &input_refs,
        &raw_inputs,
    );

    Ok(ComplianceEvidenceRecord {
        check_id,
        version,
        check_kind: request.check_kind,
        rule_version,
        evaluated_at,
        decision_status,
        reason_code,
        input_refs,
        raw_inputs,
        decision_hash,
        prior_decision_hash,
    })
}

fn normalize_evidence_text(
    value: String,
    error: ComplianceEvidenceError,
) -> Result<String, ComplianceEvidenceError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn compliance_decision_hash(
    check_kind: ComplianceCheckKind,
    rule_version: &str,
    decision_status: &str,
    reason_code: Option<&str>,
    input_refs: &[String],
    raw_inputs: &[ComplianceEvidenceInput],
) -> String {
    let mut canonical = String::new();
    canonical.push_str("kind=");
    canonical.push_str(check_kind.as_str());
    canonical.push_str("\nrule_version=");
    canonical.push_str(rule_version);
    canonical.push_str("\ndecision_status=");
    canonical.push_str(decision_status);
    canonical.push_str("\nreason_code=");
    canonical.push_str(reason_code.unwrap_or(""));
    canonical.push_str("\ninput_refs=");
    for input_ref in input_refs {
        canonical.push_str(input_ref);
        canonical.push('\u{1f}');
    }
    canonical.push_str("\nraw_inputs=");
    for input in raw_inputs {
        canonical.push_str(&input.key);
        canonical.push('=');
        canonical.push_str(&input.value);
        canonical.push('\u{1f}');
    }
    format!("fnv1a64:{:016x}", fnv1a64(canonical.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn compliance_alert_event(
    event_type: &str,
    subject_ref: String,
    severity_hint: ComplianceAlertSeverity,
    mut evidence_refs: Vec<String>,
    occurred_at: String,
) -> ComplianceAlertEvent {
    evidence_refs.sort();
    evidence_refs.dedup();
    let idempotency_key = format!(
        "compliance:{}:{}:{}:{}",
        event_type,
        subject_ref,
        occurred_at,
        evidence_refs.join("|")
    );
    ComplianceAlertEvent {
        source_domain: "compliance".to_string(),
        event_type: event_type.to_string(),
        subject_ref,
        severity_hint,
        evidence_refs,
        occurred_at,
        idempotency_key,
    }
}

fn parse_alert_time(value: &str) -> Result<DateTime<Utc>, ComplianceAlertScheduleError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| ComplianceAlertScheduleError::InvalidTimestamp)
}

fn normalize_alert_text(
    value: String,
    error: ComplianceAlertScheduleError,
) -> Result<String, ComplianceAlertScheduleError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_storage_plan(
    plan: ComplianceRecordStoragePlan,
) -> Result<ComplianceRecordStoragePlan, CompliancePolicyError> {
    let record_id = normalize_policy_text(plan.record_id, CompliancePolicyError::EmptyRecordId)?;
    let residency_tag =
        normalize_policy_text(plan.residency_tag, CompliancePolicyError::EmptyResidencyTag)?;
    let storage_region = normalize_policy_text(
        plan.storage_region,
        CompliancePolicyError::EmptyStorageRegion,
    )?;
    let created_at = normalize_policy_text(plan.created_at, CompliancePolicyError::EmptyCreatedAt)?;
    parse_policy_time(&created_at)?;

    Ok(ComplianceRecordStoragePlan {
        record_id,
        residency_tag,
        storage_region,
        retention_class: plan.retention_class,
        created_at,
    })
}

fn find_retention_policy(
    plan: &ComplianceRecordStoragePlan,
    policies: &[ComplianceRetentionPolicyRule],
) -> Result<Option<ComplianceRetentionPolicyRule>, CompliancePolicyError> {
    let mut matching_policy = None;
    for policy in policies {
        let residency_tag = normalize_policy_text(
            policy.residency_tag.clone(),
            CompliancePolicyError::EmptyResidencyTag,
        )?;
        if policy.min_retention_days < 0 {
            return Err(CompliancePolicyError::InvalidMinRetentionDays);
        }
        let mut allowed_storage_regions = Vec::new();
        for region in &policy.allowed_storage_regions {
            allowed_storage_regions.push(normalize_policy_text(
                region.clone(),
                CompliancePolicyError::EmptyAllowedStorageRegion,
            )?);
        }
        if residency_tag == plan.residency_tag && policy.retention_class == plan.retention_class {
            matching_policy = Some(ComplianceRetentionPolicyRule {
                residency_tag,
                retention_class: policy.retention_class,
                allowed_storage_regions,
                min_retention_days: policy.min_retention_days,
            });
            break;
        }
    }
    Ok(matching_policy)
}

fn min_retention_until(created_at: &str, min_days: i64) -> Result<String, CompliancePolicyError> {
    Ok(format_rfc3339(
        parse_policy_time(created_at)? + Duration::days(min_days),
    ))
}

fn policy_decision(
    plan: ComplianceRecordStoragePlan,
    checked_at: String,
    status: CompliancePolicyDecisionStatus,
    reason_code: Option<CompliancePolicyBlockReason>,
    min_retention_until: Option<String>,
) -> CompliancePolicyDecision {
    CompliancePolicyDecision {
        record_id: plan.record_id,
        checked_at,
        status,
        reason_code,
        residency_tag: plan.residency_tag,
        storage_region: plan.storage_region,
        retention_class: plan.retention_class,
        min_retention_until,
    }
}

fn parse_policy_time(value: &str) -> Result<DateTime<Utc>, CompliancePolicyError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| CompliancePolicyError::InvalidTimestamp)
}

fn normalize_policy_text(
    value: String,
    error: CompliancePolicyError,
) -> Result<String, CompliancePolicyError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
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
        build_compliance_audit_report, build_compliance_authority_export,
        build_compliance_authority_share, build_compliance_evidence_record,
        build_compliance_regulation_assist, build_initial_compliance_record,
        build_operator_certification_record, check_operator_certification, compute_rei_phi_window,
        evaluate_compliance_record_deletion, evaluate_compliance_record_storage,
        evaluate_entry_harvest_clearance, evaluate_preflight_authorization,
        evaluate_spray_buffer_compliance, refuse_compliance_evidence_overwrite,
        refuse_in_place_mutation, revoke_compliance_authority_share, AirspaceCoordinate,
        AirspaceZoneClass, AirspaceZoneError, AirspaceZoneIngestRequest,
        AppendComplianceRecordVersionRequest, ApplicationGeometry, AuthorizationBlockReason,
        AuthorizationDecisionStatus, CertificationBlockReason, CertificationStatus,
        ChemicalApplicationRecord, ComplianceAlertScheduleRequest, ComplianceAlertSeverity,
        ComplianceAuditReportError, ComplianceAuditReportRequest, ComplianceAuthorityExportError,
        ComplianceAuthorityExportRequest, ComplianceAuthorityFormat,
        ComplianceAuthorityShareRequest, ComplianceCheckKind, ComplianceDeadlineRecord,
        ComplianceEvidenceError, ComplianceEvidenceInput, ComplianceEvidenceRequest,
        CompliancePolicyBlockReason, CompliancePolicyDecisionStatus, ComplianceRecordError,
        ComplianceRecordPayload, ComplianceRecordSourceHealth, ComplianceRecordSourceStatus,
        ComplianceRecordStoragePlan, ComplianceRecordType, ComplianceRegulationAssistError,
        ComplianceRegulationAssistIntent, ComplianceRegulationAssistRequest,
        ComplianceRetentionClass, ComplianceRetentionPolicyRule, ComplianceRuleCitation,
        CreateComplianceRecordRequest, EntryHarvestAction, EntryHarvestBlockReason,
        EntryHarvestDecisionStatus, IntervalWindowStatus, OperatorCertificationRegistrationRequest,
        PreflightAirspaceStatus, PreflightAuthorizationRequest, ProductLabelInterval,
        RemoteIdFlightLogRecord, RemoteIdTrackPoint, SensitiveBufferFeature, SensitiveFeatureType,
        SprayBufferBlockReason, SprayBufferComplianceError, SprayBufferDecisionStatus,
        TelemetryGapRecord,
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
    fn spray_buffer_compliance_allows_application_outside_required_buffers() {
        let decision = evaluate_spray_buffer_compliance(
            &chemical_application("app-1", application_square(0.0, 0.0, 0.001, 0.001)),
            vec![sensitive_feature(
                "water:creek-1",
                SensitiveFeatureType::Water,
                application_square(0.01, 0.0, 0.011, 0.001),
                25.0,
            )],
            "2026-06-14T20:45:00Z".to_string(),
        )
        .expect("buffer check should run");

        assert_eq!(decision.application_id, "app-1");
        assert_eq!(decision.status, SprayBufferDecisionStatus::Compliant);
        assert_eq!(decision.reason_code, None);
        assert_eq!(decision.feature_ref, None);
    }

    #[test]
    fn spray_buffer_compliance_blocks_water_buffer_breach_with_measured_separation() {
        let decision = evaluate_spray_buffer_compliance(
            &chemical_application("app-1", application_square(0.0, 0.0, 0.001, 0.001)),
            vec![sensitive_feature(
                "water:creek-1",
                SensitiveFeatureType::Water,
                application_square(0.0011, 0.0, 0.002, 0.001),
                25.0,
            )],
            "2026-06-14T20:45:00Z".to_string(),
        )
        .expect("buffer check should run");

        assert_eq!(decision.status, SprayBufferDecisionStatus::Blocked);
        assert_eq!(
            decision.reason_code,
            Some(SprayBufferBlockReason::BufferBreach)
        );
        assert_eq!(decision.feature_ref.as_deref(), Some("water:creek-1"));
        assert_eq!(decision.required_buffer_m, Some(25.0));
        assert!(
            decision.actual_separation_m.expect("actual separation") < 25.0,
            "separation should be inside required buffer"
        );
    }

    #[test]
    fn spray_buffer_compliance_refuses_feature_crs_mismatch() {
        let error = evaluate_spray_buffer_compliance(
            &chemical_application("app-1", application_square(0.0, 0.0, 0.001, 0.001)),
            vec![SensitiveBufferFeature {
                feature_ref: "water:creek-1".to_string(),
                feature_type: SensitiveFeatureType::Water,
                geometry: ApplicationGeometry {
                    crs: "EPSG:3857".to_string(),
                    coordinates: square(0.0011, 0.0, 0.002, 0.001),
                },
                required_buffer_m: 25.0,
            }],
            "2026-06-14T20:45:00Z".to_string(),
        )
        .expect_err("non-EPSG:4326 feature should be refused");

        assert!(matches!(
            error,
            SprayBufferComplianceError::InvalidFeatureGeometry { .. }
        ));
    }

    #[test]
    fn compliance_evidence_rerun_produces_identical_decision_hash() {
        let first = build_compliance_evidence_record(buffer_evidence_request("buffer.rules.v1"))
            .expect("evidence should build");
        let second = build_compliance_evidence_record(buffer_evidence_request("buffer.rules.v1"))
            .expect("rerun evidence should build");

        assert_eq!(first.decision_hash, second.decision_hash);
        assert_eq!(first.reason_code.as_deref(), Some("buffer_breach"));
        assert_eq!(first.raw_inputs[0].key, "actual_separation_m");
        assert_eq!(
            first.input_refs,
            vec!["application:app-1", "feature:water:creek-1"]
        );
    }

    #[test]
    fn compliance_evidence_rule_version_change_appends_without_overwriting_prior() {
        let first = build_compliance_evidence_record(buffer_evidence_request("buffer.rules.v1"))
            .expect("evidence should build");
        let second = super::append_compliance_evidence_record(
            &first,
            buffer_evidence_request("buffer.rules.v2"),
        )
        .expect("new rule version should append");

        assert_eq!(first.version, 1);
        assert_eq!(second.version, 2);
        assert_eq!(
            second.prior_decision_hash.as_deref(),
            Some(first.decision_hash.as_str())
        );
        assert_ne!(first.decision_hash, second.decision_hash);
        assert_eq!(first.rule_version, "buffer.rules.v1");
        assert_eq!(second.rule_version, "buffer.rules.v2");
    }

    #[test]
    fn compliance_evidence_refuses_in_place_overwrite() {
        let first = build_compliance_evidence_record(buffer_evidence_request("buffer.rules.v1"))
            .expect("evidence should build");
        let error = refuse_compliance_evidence_overwrite(&first);

        assert_eq!(
            error,
            ComplianceEvidenceError::AppendOnlyOverwriteRefused { version: 1 }
        );
    }

    #[test]
    fn compliance_alert_scheduler_emits_cert_expiry_with_evidence_ref() {
        let events = super::schedule_compliance_alerts(ComplianceAlertScheduleRequest {
            checked_at: "2026-06-14T12:00:00Z".to_string(),
            lead_hours: 48,
            certifications: vec![operator_cert(
                "cert-107-valid",
                "operator-17",
                "part-107",
                "2026-01-01T00:00:00Z",
                "2026-06-16T11:00:00Z",
            )],
            clearance_windows: Vec::new(),
            filing_deadlines: Vec::new(),
            source_health: Vec::new(),
        })
        .expect("scheduler should run");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].source_domain, "compliance");
        assert_eq!(events[0].event_type, "compliance.cert_expiring");
        assert_eq!(
            events[0].subject_ref,
            "operator_certification:cert-107-valid"
        );
        assert_eq!(events[0].severity_hint, ComplianceAlertSeverity::Warning);
        assert_eq!(
            events[0].evidence_refs,
            vec!["certification:cert-107-valid"]
        );
    }

    #[test]
    fn compliance_alert_scheduler_surfaces_unavailable_record_source() {
        let events = super::schedule_compliance_alerts(ComplianceAlertScheduleRequest {
            checked_at: "2026-06-14T12:00:00Z".to_string(),
            lead_hours: 24,
            certifications: Vec::new(),
            clearance_windows: Vec::new(),
            filing_deadlines: Vec::new(),
            source_health: vec![ComplianceRecordSourceHealth {
                source_ref: "state-pesticide-registry".to_string(),
                status: ComplianceRecordSourceStatus::Unavailable,
                checked_at: "2026-06-14T12:00:00Z".to_string(),
            }],
        })
        .expect("scheduler should run");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "compliance.source_unavailable");
        assert_eq!(events[0].subject_ref, "source:state-pesticide-registry");
        assert_eq!(events[0].severity_hint, ComplianceAlertSeverity::Critical);
        assert_eq!(
            events[0].evidence_refs,
            vec!["source:state-pesticide-registry"]
        );
    }

    #[test]
    fn compliance_alert_scheduler_ignores_deadlines_outside_lead_window() {
        let events = super::schedule_compliance_alerts(ComplianceAlertScheduleRequest {
            checked_at: "2026-06-14T12:00:00Z".to_string(),
            lead_hours: 24,
            certifications: Vec::new(),
            clearance_windows: Vec::new(),
            filing_deadlines: vec![ComplianceDeadlineRecord {
                deadline_ref: "state-filing-1".to_string(),
                due_at: "2026-06-17T12:00:00Z".to_string(),
                evidence_ref: "compliance:record:chem-app-1".to_string(),
            }],
            source_health: Vec::new(),
        })
        .expect("scheduler should run");

        assert!(events.is_empty());
    }

    #[test]
    fn compliance_policy_allows_record_storage_in_residency_region() {
        let decision = evaluate_compliance_record_storage(
            storage_plan(
                "record-1",
                "us",
                "us-east-1",
                ComplianceRetentionClass::AuditEvidence,
            ),
            &[retention_policy(
                "us",
                ComplianceRetentionClass::AuditEvidence,
                &["us-east-1"],
                365,
            )],
            "2026-06-14T12:00:00Z".to_string(),
        )
        .expect("storage policy should evaluate");

        assert_eq!(decision.status, CompliancePolicyDecisionStatus::Allowed);
        assert_eq!(decision.reason_code, None);
        assert_eq!(decision.residency_tag, "us");
        assert_eq!(decision.storage_region, "us-east-1");
        assert_eq!(
            decision.min_retention_until.as_deref(),
            Some("2027-06-14T12:00:00Z")
        );
    }

    #[test]
    fn compliance_policy_blocks_storage_in_wrong_residency_region() {
        let decision = evaluate_compliance_record_storage(
            storage_plan(
                "record-1",
                "us",
                "eu-central-1",
                ComplianceRetentionClass::AuditEvidence,
            ),
            &[retention_policy(
                "us",
                ComplianceRetentionClass::AuditEvidence,
                &["us-east-1"],
                365,
            )],
            "2026-06-14T12:00:00Z".to_string(),
        )
        .expect("storage policy should evaluate");

        assert_eq!(decision.status, CompliancePolicyDecisionStatus::Blocked);
        assert_eq!(
            decision.reason_code,
            Some(CompliancePolicyBlockReason::ResidencyRegionMismatch)
        );
    }

    #[test]
    fn compliance_policy_refuses_deletion_before_minimum_retention() {
        let decision = evaluate_compliance_record_deletion(
            storage_plan(
                "record-chem-app-1",
                "us",
                "us-east-1",
                ComplianceRetentionClass::ChemicalApplication,
            ),
            &[retention_policy(
                "us",
                ComplianceRetentionClass::ChemicalApplication,
                &["us-east-1"],
                30,
            )],
            "2026-06-20T12:00:00Z".to_string(),
        )
        .expect("deletion policy should evaluate");

        assert_eq!(decision.status, CompliancePolicyDecisionStatus::Blocked);
        assert_eq!(
            decision.reason_code,
            Some(CompliancePolicyBlockReason::RetentionPeriodActive)
        );
        assert_eq!(
            decision.min_retention_until.as_deref(),
            Some("2026-07-14T12:00:00Z")
        );
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

    #[test]
    fn authority_export_adapts_validated_report_to_specific_layout() {
        let report = complete_audit_report();

        let export = build_compliance_authority_export(ComplianceAuthorityExportRequest {
            authority_format: ComplianceAuthorityFormat::FaaRemoteId,
            report,
            generated_at: "2026-06-13T12:05:00Z".to_string(),
            residency_tag: "us".to_string(),
            storage_region: "us-east-1".to_string(),
            retention_class: ComplianceRetentionClass::AuditEvidence,
        })
        .expect("remote ID authority export should build from the report");

        assert_eq!(export.schema_version, "compliance.authority_export.v1");
        assert_eq!(
            export.authority_format,
            ComplianceAuthorityFormat::FaaRemoteId
        );
        assert_eq!(export.file_name, "report-field-north-faa_remote_id.json");
        assert_eq!(export.included_record_ids, vec!["remote-log-1".to_string()]);
        assert_eq!(export.residency_tag, "us");
        assert_eq!(
            export
                .payload
                .pointer("/remote_id_logs/0/record_id")
                .and_then(|value| value.as_str()),
            Some("remote-log-1")
        );
        assert!(export
            .provenance_refs
            .contains(&"provenance:remote-id/remote-log-1/v1".to_string()));
    }

    #[test]
    fn authority_export_refuses_format_without_required_record() {
        let report = build_compliance_audit_report(ComplianceAuditReportRequest {
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
            mandatory_record_types: vec![ComplianceRecordType::RemoteIdLog],
        })
        .expect("remote-only report should build");

        let error = build_compliance_authority_export(ComplianceAuthorityExportRequest {
            authority_format: ComplianceAuthorityFormat::StatePesticideApplication,
            report,
            generated_at: "2026-06-13T12:05:00Z".to_string(),
            residency_tag: "us".to_string(),
            storage_region: "us-east-1".to_string(),
            retention_class: ComplianceRetentionClass::AuditEvidence,
        })
        .expect_err("state pesticide export requires chemical applications");

        assert_eq!(
            error,
            ComplianceAuthorityExportError::MissingAuthorityRecord {
                record_type: ComplianceRecordType::ChemicalApplication
            }
        );
    }

    #[test]
    fn authority_share_is_bounded_and_revocable() {
        let export = build_compliance_authority_export(ComplianceAuthorityExportRequest {
            authority_format: ComplianceAuthorityFormat::StatePesticideApplication,
            report: complete_audit_report(),
            generated_at: "2026-06-13T12:05:00Z".to_string(),
            residency_tag: "us".to_string(),
            storage_region: "us-east-1".to_string(),
            retention_class: ComplianceRetentionClass::AuditEvidence,
        })
        .expect("authority export should build");

        let share = build_compliance_authority_share(ComplianceAuthorityShareRequest {
            share_id: "share-state-1".to_string(),
            export,
            created_at: "2026-06-13T12:10:00Z".to_string(),
            expires_at: "2026-06-20T12:10:00Z".to_string(),
        })
        .expect("share artifact should build");

        assert_eq!(
            share.url_path,
            "/api/compliance/authority-shares/share-state-1"
        );
        assert_eq!(share.residency_tag, "us");
        assert!(share.revocable);
        assert_eq!(share.revoked_at, None);

        let revoked = revoke_compliance_authority_share(share, "2026-06-14T12:10:00Z".to_string())
            .expect("share should revoke with audit timestamp");
        assert_eq!(revoked.revoked_at.as_deref(), Some("2026-06-14T12:10:00Z"));
    }

    #[test]
    fn regulation_assist_summarizes_records_with_citations_and_uncertainty() {
        let output = build_compliance_regulation_assist(ComplianceRegulationAssistRequest {
            assist_id: "assist-1".to_string(),
            intent: ComplianceRegulationAssistIntent::Summary,
            report: complete_audit_report(),
            generated_at: "2026-06-13T12:20:00Z".to_string(),
            rule_citations: vec![
                ComplianceRuleCitation {
                    rule_ref: "rule:faa:remote-id".to_string(),
                    title: "FAA Remote ID flight log submission".to_string(),
                },
                ComplianceRuleCitation {
                    rule_ref: "rule:state:pesticide-application".to_string(),
                    title: "State pesticide application filing".to_string(),
                },
            ],
            feature_enabled: true,
        })
        .expect("assist should summarize deterministic records");

        assert_eq!(output.assist_id, "assist-1");
        assert_eq!(output.report_id, "report-field-north");
        assert!(!output.can_authorize);
        assert!(!output.can_clear_violation);
        assert!(output
            .summary
            .contains("Deterministic gates remain authoritative"));
        assert!(output
            .draft_filing_text
            .contains("does not authorize operations or clear violations"));
        assert!(output
            .source_record_refs
            .contains(&"compliance_record:remote-log-1@v1".to_string()));
        assert!(output.rule_refs.contains(&"rule:faa:remote-id".to_string()));
        assert!(
            output.uncertainty_flag,
            "minimal report lacks all default mandatory record classes"
        );
        assert!(output
            .uncertainty_reasons
            .contains(&"missing_operator_certification".to_string()));
    }

    #[test]
    fn regulation_assist_refuses_authorization_or_clearance_attempts() {
        let error = build_compliance_regulation_assist(ComplianceRegulationAssistRequest {
            assist_id: "assist-denied".to_string(),
            intent: ComplianceRegulationAssistIntent::AuthorizeFlight,
            report: complete_audit_report(),
            generated_at: "2026-06-13T12:20:00Z".to_string(),
            rule_citations: vec![ComplianceRuleCitation {
                rule_ref: "rule:faa:remote-id".to_string(),
                title: "FAA Remote ID flight log submission".to_string(),
            }],
            feature_enabled: true,
        })
        .expect_err("assist must not authorize flights");

        assert_eq!(
            error,
            ComplianceRegulationAssistError::DeterministicGateRequired
        );
    }

    fn complete_audit_report() -> super::ComplianceAuditReport {
        build_compliance_audit_report(ComplianceAuditReportRequest {
            report_id: "report-field-north".to_string(),
            org_id: "org-alpha".to_string(),
            field_id: "field-north".to_string(),
            generated_at: "2026-06-13T12:00:00Z".to_string(),
            records: vec![
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
                    None,
                    "provenance:application/chem-app-1/v1",
                    Some(ComplianceRecordPayload::ChemicalApplication(
                        chemical_application(
                            "chem-app-1",
                            ApplicationGeometry {
                                crs: "EPSG:4326".to_string(),
                                coordinates: square_zone(),
                            },
                        ),
                    )),
                ),
            ],
            mandatory_record_types: vec![
                ComplianceRecordType::RemoteIdLog,
                ComplianceRecordType::ChemicalApplication,
            ],
        })
        .expect("complete audit report should build")
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

    fn chemical_application(
        application_id: &str,
        geometry: ApplicationGeometry,
    ) -> ChemicalApplicationRecord {
        ChemicalApplicationRecord {
            application_id: application_id.to_string(),
            product: "Example Herbicide".to_string(),
            epa_or_label_ref: "EPA-12345-LBL".to_string(),
            field_id: "field-north".to_string(),
            geometry,
            applied_at: "2026-06-12T13:00:00Z".to_string(),
            rate: 1.75,
            units: "L/ha".to_string(),
            operator_id: "operator-17".to_string(),
        }
    }

    fn sensitive_feature(
        feature_ref: &str,
        feature_type: SensitiveFeatureType,
        geometry: ApplicationGeometry,
        required_buffer_m: f64,
    ) -> SensitiveBufferFeature {
        SensitiveBufferFeature {
            feature_ref: feature_ref.to_string(),
            feature_type,
            geometry,
            required_buffer_m,
        }
    }

    fn buffer_evidence_request(rule_version: &str) -> ComplianceEvidenceRequest {
        ComplianceEvidenceRequest {
            check_id: "buffer-check:app-1".to_string(),
            check_kind: ComplianceCheckKind::SprayBuffer,
            rule_version: rule_version.to_string(),
            evaluated_at: "2026-06-14T21:03:00Z".to_string(),
            decision_status: "blocked".to_string(),
            reason_code: Some("buffer_breach".to_string()),
            input_refs: vec![
                "feature:water:creek-1".to_string(),
                "application:app-1".to_string(),
            ],
            raw_inputs: vec![
                ComplianceEvidenceInput {
                    key: "required_buffer_m".to_string(),
                    value: "25.000".to_string(),
                },
                ComplianceEvidenceInput {
                    key: "actual_separation_m".to_string(),
                    value: "11.132".to_string(),
                },
            ],
        }
    }

    fn storage_plan(
        record_id: &str,
        residency_tag: &str,
        storage_region: &str,
        retention_class: ComplianceRetentionClass,
    ) -> ComplianceRecordStoragePlan {
        ComplianceRecordStoragePlan {
            record_id: record_id.to_string(),
            residency_tag: residency_tag.to_string(),
            storage_region: storage_region.to_string(),
            retention_class,
            created_at: "2026-06-14T12:00:00Z".to_string(),
        }
    }

    fn retention_policy(
        residency_tag: &str,
        retention_class: ComplianceRetentionClass,
        regions: &[&str],
        min_retention_days: i64,
    ) -> ComplianceRetentionPolicyRule {
        ComplianceRetentionPolicyRule {
            residency_tag: residency_tag.to_string(),
            retention_class,
            allowed_storage_regions: regions.iter().map(|region| (*region).to_string()).collect(),
            min_retention_days,
        }
    }

    fn application_square(
        min_lon: f64,
        min_lat: f64,
        max_lon: f64,
        max_lat: f64,
    ) -> ApplicationGeometry {
        ApplicationGeometry {
            crs: "EPSG:4326".to_string(),
            coordinates: square(min_lon, min_lat, max_lon, max_lat),
        }
    }

    fn square(min_lon: f64, min_lat: f64, max_lon: f64, max_lat: f64) -> Vec<AirspaceCoordinate> {
        vec![
            AirspaceCoordinate {
                longitude: min_lon,
                latitude: min_lat,
            },
            AirspaceCoordinate {
                longitude: max_lon,
                latitude: min_lat,
            },
            AirspaceCoordinate {
                longitude: max_lon,
                latitude: max_lat,
            },
            AirspaceCoordinate {
                longitude: min_lon,
                latitude: max_lat,
            },
            AirspaceCoordinate {
                longitude: min_lon,
                latitude: min_lat,
            },
        ]
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
