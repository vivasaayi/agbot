use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const EVIDENCE_DIGEST_ALGORITHM: &str = "sha256";

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceObject {
    pub evidence_kind: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredEvidence {
    pub digest: String,
    pub algorithm: String,
    pub object: EvidenceObject,
    pub canonical_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceIntegrityProof {
    pub digest: String,
    pub algorithm: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LineageLedger {
    records: BTreeMap<String, LineageRecord>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EvidenceStore {
    objects: BTreeMap<String, StoredEvidence>,
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
    #[error("evidence_kind cannot be empty")]
    EmptyEvidenceKind,
    #[error("evidence digest cannot be empty")]
    EmptyEvidenceDigest,
    #[error("lineage already exists for artifact {artifact_id}")]
    DuplicateArtifactId { artifact_id: String },
    #[error("unknown input artifact {input_artifact_id} for {artifact_id}")]
    UnknownInputArtifact {
        artifact_id: String,
        input_artifact_id: String,
    },
    #[error("unknown evidence digest {digest}")]
    UnknownEvidenceDigest { digest: String },
    #[error("evidence digest mismatch expected {expected_digest} actual {actual_digest}")]
    EvidenceDigestMismatch {
        expected_digest: String,
        actual_digest: String,
    },
    #[error("evidence serialization failed: {details}")]
    EvidenceSerializationFailed { details: String },
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

impl EvidenceStore {
    pub fn store_evidence(
        &mut self,
        object: EvidenceObject,
    ) -> Result<StoredEvidence, ProvenanceError> {
        let object = normalize_evidence_object(object)?;
        let canonical_bytes = canonical_evidence_bytes(&object)?;
        let digest = evidence_digest_for_bytes(&canonical_bytes);

        if let Some(existing) = self.objects.get(&digest) {
            return Ok(existing.clone());
        }

        let stored = StoredEvidence {
            digest: digest.clone(),
            algorithm: EVIDENCE_DIGEST_ALGORITHM.to_string(),
            object,
            canonical_bytes,
        };
        self.objects.insert(digest, stored.clone());
        Ok(stored)
    }

    pub fn fetch_evidence(&self, digest: &str) -> Option<StoredEvidence> {
        let digest = normalize_optional_text(digest)?;
        self.objects.get(&digest).cloned()
    }

    pub fn verify_evidence_bytes(
        &self,
        digest: &str,
        bytes: &[u8],
    ) -> Result<EvidenceIntegrityProof, ProvenanceError> {
        let expected_digest =
            normalize_required_text(digest.to_string(), ProvenanceError::EmptyEvidenceDigest)?;
        if !self.objects.contains_key(&expected_digest) {
            return Err(ProvenanceError::UnknownEvidenceDigest {
                digest: expected_digest,
            });
        }

        let actual_digest = evidence_digest_for_bytes(bytes);
        if actual_digest != expected_digest {
            return Err(ProvenanceError::EvidenceDigestMismatch {
                expected_digest,
                actual_digest,
            });
        }

        Ok(EvidenceIntegrityProof {
            digest: actual_digest,
            algorithm: EVIDENCE_DIGEST_ALGORITHM.to_string(),
            byte_len: bytes.len(),
        })
    }

    pub fn evidence_count(&self) -> usize {
        self.objects.len()
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

fn normalize_evidence_object(
    mut object: EvidenceObject,
) -> Result<EvidenceObject, ProvenanceError> {
    object.evidence_kind =
        normalize_required_text(object.evidence_kind, ProvenanceError::EmptyEvidenceKind)?;
    Ok(object)
}

fn canonical_evidence_bytes(object: &EvidenceObject) -> Result<Vec<u8>, ProvenanceError> {
    serde_json::to_vec(object).map_err(|error| ProvenanceError::EvidenceSerializationFailed {
        details: error.to_string(),
    })
}

fn evidence_digest_for_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!(
        "{}:{}",
        EVIDENCE_DIGEST_ALGORITHM,
        lowercase_hex(digest.as_slice())
    )
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    value
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
        ArtifactKind, EvidenceObject, EvidenceStore, LineageLedger, LineageRecord, ProvenanceError,
        ProvenanceParameters,
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

    #[test]
    fn evidence_store_addresses_object_by_digest_and_retrieves_it() {
        let mut store = EvidenceStore::default();

        let stored = store
            .store_evidence(sample_evidence())
            .expect("evidence should be stored by digest");

        assert_eq!(stored.algorithm, "sha256");
        assert!(stored.digest.starts_with("sha256:"));
        assert_eq!(store.evidence_count(), 1);
        assert_eq!(store.fetch_evidence(&stored.digest), Some(stored.clone()));
        let proof = store
            .verify_evidence_bytes(&stored.digest, &stored.canonical_bytes)
            .expect("stored bytes should verify");
        assert_eq!(proof.digest, stored.digest);
        assert_eq!(proof.byte_len, stored.canonical_bytes.len());
    }

    #[test]
    fn evidence_store_deduplicates_identical_objects() {
        let mut store = EvidenceStore::default();

        let first = store
            .store_evidence(sample_evidence())
            .expect("first evidence should store");
        let second = store
            .store_evidence(sample_evidence())
            .expect("duplicate evidence should deduplicate");

        assert_eq!(first.digest, second.digest);
        assert_eq!(store.evidence_count(), 1);
    }

    #[test]
    fn altered_evidence_bytes_fail_integrity_check_with_reason() {
        let mut store = EvidenceStore::default();
        let stored = store
            .store_evidence(sample_evidence())
            .expect("evidence should store");
        let mut altered_bytes = stored.canonical_bytes.clone();
        altered_bytes.push(b'\n');

        let error = store
            .verify_evidence_bytes(&stored.digest, &altered_bytes)
            .expect_err("altered bytes should fail digest verification");

        match error {
            ProvenanceError::EvidenceDigestMismatch {
                expected_digest,
                actual_digest,
            } => {
                assert_eq!(expected_digest, stored.digest);
                assert_ne!(actual_digest, expected_digest);
            }
            other => panic!("expected digest mismatch, got {other:?}"),
        }
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

    fn sample_evidence() -> EvidenceObject {
        EvidenceObject {
            evidence_kind: "finding_evidence".to_string(),
            payload: serde_json::json!({
                "finding_id": "finding:09:stress-ne-zone",
                "raster_ref": "raster:ndvi:alpha-2026-06-12",
                "mask_ref": "mask:stress-ne-zone",
                "counts": {
                    "pixels_flagged": 1842,
                    "pixels_total": 12000
                }
            }),
        }
    }
}
