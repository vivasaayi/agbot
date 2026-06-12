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

#[cfg(test)]
mod tests {
    use super::{
        append_compliance_record_version, build_initial_compliance_record,
        refuse_in_place_mutation, AppendComplianceRecordVersionRequest, ComplianceRecordError,
        ComplianceRecordType, CreateComplianceRecordRequest,
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
}
