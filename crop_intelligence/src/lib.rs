use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CropModelTask {
    StandCount,
    CanopyCover,
    DiseaseDetection,
    PestDetection,
    WeedMapping,
}

impl CropModelTask {
    pub fn as_str(self) -> &'static str {
        match self {
            CropModelTask::StandCount => "stand_count",
            CropModelTask::CanopyCover => "canopy_cover",
            CropModelTask::DiseaseDetection => "disease_detection",
            CropModelTask::PestDetection => "pest_detection",
            CropModelTask::WeedMapping => "weed_mapping",
        }
    }
}

impl std::str::FromStr for CropModelTask {
    type Err = CropModelRegistryError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "stand_count" => Ok(CropModelTask::StandCount),
            "canopy_cover" => Ok(CropModelTask::CanopyCover),
            "disease_detection" => Ok(CropModelTask::DiseaseDetection),
            "pest_detection" => Ok(CropModelTask::PestDetection),
            "weed_mapping" => Ok(CropModelTask::WeedMapping),
            _ => Err(CropModelRegistryError::UnsupportedTask {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ModelVersionRegistrationRequest {
    #[serde(default)]
    pub model_id: String,
    #[serde(default)]
    pub version: String,
    pub task: CropModelTask,
    #[serde(default)]
    pub training_set_ref: String,
    #[serde(default = "default_model_metrics")]
    pub metrics: serde_json::Value,
    #[serde(default)]
    pub provenance_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelVersionRecord {
    pub model_id: String,
    pub version: String,
    pub task: CropModelTask,
    pub training_set_ref: String,
    pub metrics: serde_json::Value,
    pub provenance_ref: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct InferenceModelReference {
    #[serde(default)]
    pub model_id: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelGateResponse {
    pub model_id: String,
    pub version: String,
    pub registered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CropModelRegistryError {
    #[error("model_id cannot be empty")]
    EmptyModelId,
    #[error("model version cannot be empty")]
    EmptyVersion,
    #[error("training_set_ref cannot be empty")]
    EmptyTrainingSetRef,
    #[error("provenance_ref cannot be empty")]
    EmptyProvenanceRef,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("metrics must be a non-empty JSON object")]
    InvalidMetrics,
    #[error("unsupported crop model task {value}")]
    UnsupportedTask { value: String },
    #[error("unregistered model {model_id}@{version}")]
    UnregisteredModel { model_id: String, version: String },
}

pub fn build_model_version_record(
    request: ModelVersionRegistrationRequest,
    created_at: String,
) -> Result<ModelVersionRecord, CropModelRegistryError> {
    let model_id = normalize_required_text(request.model_id, CropModelRegistryError::EmptyModelId)?;
    let version = normalize_required_text(request.version, CropModelRegistryError::EmptyVersion)?;
    let training_set_ref = normalize_required_text(
        request.training_set_ref,
        CropModelRegistryError::EmptyTrainingSetRef,
    )?;
    let provenance_ref = normalize_required_text(
        request.provenance_ref,
        CropModelRegistryError::EmptyProvenanceRef,
    )?;
    let created_at = normalize_required_text(created_at, CropModelRegistryError::EmptyCreatedAt)?;
    validate_metrics(&request.metrics)?;

    Ok(ModelVersionRecord {
        model_id,
        version,
        task: request.task,
        training_set_ref,
        metrics: request.metrics,
        provenance_ref,
        created_at,
    })
}

pub fn validate_model_reference(
    reference: InferenceModelReference,
    registered: bool,
) -> Result<ModelGateResponse, CropModelRegistryError> {
    let model_id =
        normalize_required_text(reference.model_id, CropModelRegistryError::EmptyModelId)?;
    let version = normalize_required_text(reference.version, CropModelRegistryError::EmptyVersion)?;

    if !registered {
        return Err(CropModelRegistryError::UnregisteredModel { model_id, version });
    }

    Ok(ModelGateResponse {
        model_id,
        version,
        registered,
    })
}

fn normalize_required_text(
    value: String,
    error: CropModelRegistryError,
) -> Result<String, CropModelRegistryError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_metrics(metrics: &serde_json::Value) -> Result<(), CropModelRegistryError> {
    match metrics.as_object() {
        Some(metrics) if !metrics.is_empty() => Ok(()),
        _ => Err(CropModelRegistryError::InvalidMetrics),
    }
}

fn default_model_metrics() -> serde_json::Value {
    serde_json::json!({})
}

#[cfg(test)]
mod tests {
    use super::{
        build_model_version_record, validate_model_reference, CropModelRegistryError,
        CropModelTask, InferenceModelReference, ModelVersionRegistrationRequest,
    };

    #[test]
    fn model_version_record_requires_versioned_provenance() {
        let record = build_model_version_record(
            ModelVersionRegistrationRequest {
                model_id: " lesion-detector ".to_string(),
                version: " 2026.06.1 ".to_string(),
                task: CropModelTask::DiseaseDetection,
                training_set_ref: " dataset:lesion-v3 ".to_string(),
                metrics: serde_json::json!({
                    "precision": 0.91,
                    "recall": 0.87,
                    "iou": 0.73
                }),
                provenance_ref: " provenance:model/lesion-detector/2026.06.1 ".to_string(),
            },
            " 2026-06-12T12:00:00Z ".to_string(),
        )
        .expect("model version should be valid");

        assert_eq!(record.model_id, "lesion-detector");
        assert_eq!(record.version, "2026.06.1");
        assert_eq!(record.task, CropModelTask::DiseaseDetection);
        assert_eq!(record.training_set_ref, "dataset:lesion-v3");
        assert_eq!(
            record
                .metrics
                .get("precision")
                .and_then(|value| value.as_f64()),
            Some(0.91)
        );
        assert_eq!(
            record.provenance_ref,
            "provenance:model/lesion-detector/2026.06.1"
        );
    }

    #[test]
    fn model_version_rejects_missing_metrics() {
        let error = build_model_version_record(
            ModelVersionRegistrationRequest {
                model_id: "lesion-detector".to_string(),
                version: "2026.06.1".to_string(),
                task: CropModelTask::DiseaseDetection,
                training_set_ref: "dataset:lesion-v3".to_string(),
                metrics: serde_json::json!({}),
                provenance_ref: "provenance:model/lesion-detector/2026.06.1".to_string(),
            },
            "2026-06-12T12:00:00Z".to_string(),
        )
        .expect_err("empty metrics should be rejected");

        assert_eq!(error, CropModelRegistryError::InvalidMetrics);
    }

    #[test]
    fn unregistered_model_reference_is_rejected() {
        let error = validate_model_reference(
            InferenceModelReference {
                model_id: "unknown-model".to_string(),
                version: "v0".to_string(),
            },
            false,
        )
        .expect_err("unknown model should be rejected");

        assert_eq!(
            error,
            CropModelRegistryError::UnregisteredModel {
                model_id: "unknown-model".to_string(),
                version: "v0".to_string()
            }
        );
    }
}
