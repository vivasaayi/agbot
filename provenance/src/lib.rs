use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Capture,
    Scene,
    Product,
    Finding,
    Report,
    Action,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProvenanceParameters {
    value: serde_json::Value,
}

impl ProvenanceParameters {
    pub fn from_json(value: serde_json::Value) -> Self {
        Self { value }
    }

    pub fn as_json(&self) -> &serde_json::Value {
        &self.value
    }

    pub fn into_json(self) -> serde_json::Value {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineageRecord {
    pub artifact_id: String,
    pub kind: ArtifactKind,
    #[serde(default)]
    pub inputs: Vec<String>,
    pub method: String,
    pub parameters: ProvenanceParameters,
    pub operator: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LineageLedger {
    records: BTreeMap<String, LineageRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProvenanceError {
    #[error("artifact_id cannot be empty")]
    EmptyArtifactId,
    #[error("input artifact id cannot be empty for {artifact_id}")]
    EmptyInputArtifactId { artifact_id: String },
    #[error("method cannot be empty for {artifact_id}")]
    EmptyMethod { artifact_id: String },
    #[error("operator cannot be empty for {artifact_id}")]
    EmptyOperator { artifact_id: String },
    #[error("created_at cannot be empty for {artifact_id}")]
    EmptyCreatedAt { artifact_id: String },
    #[error("lineage already exists for artifact {artifact_id}")]
    DuplicateArtifactId { artifact_id: String },
    #[error("unknown input artifact {input_artifact_id} for {artifact_id}")]
    UnknownInputArtifact {
        artifact_id: String,
        input_artifact_id: String,
    },
}

impl LineageLedger {
    pub fn record_lineage(
        &mut self,
        record: LineageRecord,
    ) -> Result<LineageRecord, ProvenanceError> {
        let record = normalize_lineage_record(record)?;
        if self.records.contains_key(&record.artifact_id) {
            return Err(ProvenanceError::DuplicateArtifactId {
                artifact_id: record.artifact_id,
            });
        }

        for input_artifact_id in &record.inputs {
            if !self.records.contains_key(input_artifact_id) {
                return Err(ProvenanceError::UnknownInputArtifact {
                    artifact_id: record.artifact_id,
                    input_artifact_id: input_artifact_id.clone(),
                });
            }
        }

        self.records
            .insert(record.artifact_id.clone(), record.clone());
        Ok(record)
    }

    pub fn fetch_lineage(&self, artifact_id: &str) -> Option<LineageRecord> {
        let artifact_id = normalize_optional_text(artifact_id)?;
        self.records.get(&artifact_id).cloned()
    }

    pub fn artifact_count(&self) -> usize {
        self.records.len()
    }
}

fn normalize_lineage_record(mut record: LineageRecord) -> Result<LineageRecord, ProvenanceError> {
    record.artifact_id =
        normalize_required_text(record.artifact_id, ProvenanceError::EmptyArtifactId)?;
    record.inputs = record
        .inputs
        .into_iter()
        .map(|input| {
            normalize_required_text(
                input,
                ProvenanceError::EmptyInputArtifactId {
                    artifact_id: record.artifact_id.clone(),
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    record.method = normalize_required_text(
        record.method,
        ProvenanceError::EmptyMethod {
            artifact_id: record.artifact_id.clone(),
        },
    )?;
    record.operator = normalize_required_text(
        record.operator,
        ProvenanceError::EmptyOperator {
            artifact_id: record.artifact_id.clone(),
        },
    )?;
    record.created_at = normalize_required_text(
        record.created_at,
        ProvenanceError::EmptyCreatedAt {
            artifact_id: record.artifact_id.clone(),
        },
    )?;
    Ok(record)
}

fn normalize_required_text(
    value: String,
    error: ProvenanceError,
) -> Result<String, ProvenanceError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactKind, LineageLedger, LineageRecord, ProvenanceError, ProvenanceParameters,
    };

    #[test]
    fn records_finding_lineage_from_product() {
        let mut ledger = LineageLedger::default();
        seed_product(&mut ledger);

        let finding = ledger
            .record_lineage(finding_lineage())
            .expect("finding lineage should be recorded");

        assert_eq!(finding.artifact_id, "finding:09:stress-ne-zone");
        assert_eq!(finding.kind, ArtifactKind::Finding);
        assert_eq!(finding.inputs, vec!["product:ndvi:alpha-2026-06-12"]);
        assert_eq!(finding.method, "09.crop_stress_finding");
        assert_eq!(finding.operator, "operator:dsp-7");
        assert_eq!(finding.created_at, "2026-06-12T13:00:00Z");
    }

    #[test]
    fn fetch_lineage_round_trips_inputs_and_parameters() {
        let mut ledger = LineageLedger::default();
        seed_product(&mut ledger);
        let expected = ledger
            .record_lineage(finding_lineage())
            .expect("finding lineage should be recorded");

        let fetched = ledger
            .fetch_lineage("finding:09:stress-ne-zone")
            .expect("lineage should be fetchable");

        assert_eq!(fetched, expected);
        assert_eq!(fetched.inputs, vec!["product:ndvi:alpha-2026-06-12"]);
        assert_eq!(
            fetched.parameters,
            ProvenanceParameters::from_json(serde_json::json!({
                "index": "ndvi",
                "threshold": 0.42,
                "zone": "NE"
            }))
        );
    }

    #[test]
    fn rejects_lineage_with_unknown_input_artifact() {
        let mut ledger = LineageLedger::default();

        let error = ledger
            .record_lineage(finding_lineage())
            .expect_err("unknown product input should be rejected");

        assert_eq!(
            error,
            ProvenanceError::UnknownInputArtifact {
                artifact_id: "finding:09:stress-ne-zone".to_string(),
                input_artifact_id: "product:ndvi:alpha-2026-06-12".to_string()
            }
        );
        assert!(ledger.fetch_lineage("finding:09:stress-ne-zone").is_none());
    }

    #[test]
    fn lineage_record_serializes_parameters_as_payload() {
        let value = serde_json::to_value(finding_lineage()).expect("lineage should serialize");

        assert_eq!(
            value["parameters"],
            serde_json::json!({
                "index": "ndvi",
                "threshold": 0.42,
                "zone": "NE"
            })
        );
    }

    fn seed_product(ledger: &mut LineageLedger) {
        ledger
            .record_lineage(scene_lineage())
            .expect("scene lineage should be recorded");
        ledger
            .record_lineage(product_lineage())
            .expect("product lineage should be recorded");
    }

    fn scene_lineage() -> LineageRecord {
        LineageRecord {
            artifact_id: "scene:alpha-2026-06-12".to_string(),
            kind: ArtifactKind::Scene,
            inputs: Vec::new(),
            method: "04.capture_session_scene".to_string(),
            parameters: ProvenanceParameters::from_json(serde_json::json!({
                "flight_id": "flight:alpha-17",
                "sensor_set": "multispectral-rig-2"
            })),
            operator: "operator:dsp-7".to_string(),
            created_at: "2026-06-12T12:00:00Z".to_string(),
        }
    }

    fn product_lineage() -> LineageRecord {
        LineageRecord {
            artifact_id: "product:ndvi:alpha-2026-06-12".to_string(),
            kind: ArtifactKind::Product,
            inputs: vec!["scene:alpha-2026-06-12".to_string()],
            method: "05.ndvi_product".to_string(),
            parameters: ProvenanceParameters::from_json(serde_json::json!({
                "red_band": "B04",
                "nir_band": "B08"
            })),
            operator: "operator:dsp-7".to_string(),
            created_at: "2026-06-12T12:30:00Z".to_string(),
        }
    }

    fn finding_lineage() -> LineageRecord {
        LineageRecord {
            artifact_id: "finding:09:stress-ne-zone".to_string(),
            kind: ArtifactKind::Finding,
            inputs: vec!["product:ndvi:alpha-2026-06-12".to_string()],
            method: "09.crop_stress_finding".to_string(),
            parameters: ProvenanceParameters::from_json(serde_json::json!({
                "index": "ndvi",
                "threshold": 0.42,
                "zone": "NE"
            })),
            operator: "operator:dsp-7".to_string(),
            created_at: "2026-06-12T13:00:00Z".to_string(),
        }
    }
}
