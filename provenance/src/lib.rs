use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const EVIDENCE_DIGEST_ALGORITHM: &str = "sha256";
const SYSTEM_AUDIT_ACTOR_ID: &str = "system:provenance-ledger";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    Operator,
    Agronomist,
    DroneServiceProvider,
    PlatformAdmin,
    SystemService,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorIdentity {
    pub actor_id: String,
    pub actor_kind: ActorKind,
}

impl ActorIdentity {
    pub fn system(actor_id: &str) -> Self {
        Self {
            actor_id: actor_id.to_string(),
            actor_kind: ActorKind::SystemService,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionContext {
    pub actor_id: Option<String>,
    pub actor_kind: Option<ActorKind>,
}

impl ActionContext {
    pub fn new(actor_id: Option<String>, actor_kind: Option<ActorKind>) -> Self {
        Self {
            actor_id,
            actor_kind,
        }
    }

    pub fn resolve_actor(&self, action_ref: &str) -> Result<ActorIdentity, ProvenanceError> {
        let action_ref =
            normalize_required_text(action_ref.to_string(), ProvenanceError::EmptyActionRef)?;
        let Some(actor_id) = self
            .actor_id
            .clone()
            .and_then(normalize_optional_text_owned)
        else {
            return Err(ProvenanceError::UnattributedAction { action_ref });
        };
        let Some(actor_kind) = self.actor_kind else {
            return Err(ProvenanceError::UnattributedAction { action_ref });
        };
        normalize_actor_identity(ActorIdentity {
            actor_id,
            actor_kind,
        })
    }
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
    pub actor: ActorIdentity,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackwardProvenanceTrace {
    pub target_artifact_id: String,
    pub records: Vec<LineageRecord>,
    pub gaps: Vec<LineageGap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageGap {
    pub missing_artifact_id: String,
    pub referenced_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditAction {
    pub action_ref: String,
    pub action_kind: String,
    pub artifact_ref: Option<String>,
    pub payload: ProvenanceParameters,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Accepted,
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditRefusalReason {
    UnattributedAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditChainBreachReason {
    SequenceMismatch,
    PreviousHashMismatch,
    PayloadHashMismatch,
    EntryHashMismatch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub seq: u64,
    pub prev_hash: Option<String>,
    pub payload_hash: String,
    pub entry_hash: String,
    pub actor: ActorIdentity,
    pub ts: String,
    pub action: AuditAction,
    pub outcome: AuditOutcome,
    pub refusal_reason: Option<AuditRefusalReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditChainVerification {
    pub verified: bool,
    pub verified_len: usize,
    pub breach_index: Option<usize>,
    pub reason: Option<AuditChainBreachReason>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LineageLedger {
    records: BTreeMap<String, LineageRecord>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EvidenceStore {
    objects: BTreeMap<String, StoredEvidence>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AuditLedger {
    entries: Vec<AuditEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProvenanceError {
    #[error("artifact_id cannot be empty")]
    EmptyArtifactId,
    #[error("actor_id cannot be empty")]
    EmptyActorId,
    #[error("input artifact id cannot be empty for {artifact_id}")]
    EmptyInputArtifactId { artifact_id: String },
    #[error("method cannot be empty for {artifact_id}")]
    EmptyMethod { artifact_id: String },
    #[error("operator cannot be empty for {artifact_id}")]
    EmptyOperator { artifact_id: String },
    #[error("created_at cannot be empty for {artifact_id}")]
    EmptyCreatedAt { artifact_id: String },
    #[error("action_ref cannot be empty")]
    EmptyActionRef,
    #[error("action_kind cannot be empty for {action_ref}")]
    EmptyActionKind { action_ref: String },
    #[error("action timestamp cannot be empty for {action_ref}")]
    EmptyActionTimestamp { action_ref: String },
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
    #[error("mutating action {action_ref} has no resolvable actor")]
    UnattributedAction { action_ref: String },
    #[error("audit log is append-only: refused {attempted_operation} for {action_ref}")]
    AppendOnlyAuditLog {
        action_ref: String,
        attempted_operation: String,
    },
    #[error("evidence serialization failed: {details}")]
    EvidenceSerializationFailed { details: String },
    #[error("audit serialization failed: {details}")]
    AuditSerializationFailed { details: String },
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

    pub fn trace_backward(
        &self,
        artifact_id: &str,
    ) -> Result<BackwardProvenanceTrace, ProvenanceError> {
        let target_artifact_id =
            normalize_required_text(artifact_id.to_string(), ProvenanceError::EmptyArtifactId)?;
        let mut trace = BackwardProvenanceTrace {
            target_artifact_id: target_artifact_id.clone(),
            records: Vec::new(),
            gaps: Vec::new(),
        };
        let mut visited = BTreeSet::new();
        self.collect_backward_lineage(&target_artifact_id, None, &mut visited, &mut trace);
        Ok(trace)
    }

    fn collect_backward_lineage(
        &self,
        artifact_id: &str,
        referenced_by: Option<String>,
        visited: &mut BTreeSet<String>,
        trace: &mut BackwardProvenanceTrace,
    ) {
        if !visited.insert(artifact_id.to_string()) {
            return;
        }

        let Some(record) = self.records.get(artifact_id) else {
            trace.gaps.push(LineageGap {
                missing_artifact_id: artifact_id.to_string(),
                referenced_by,
            });
            return;
        };

        trace.records.push(record.clone());
        for input_artifact_id in &record.inputs {
            self.collect_backward_lineage(
                input_artifact_id,
                Some(record.artifact_id.clone()),
                visited,
                trace,
            );
        }
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

impl AuditLedger {
    pub fn append_action(
        &mut self,
        actor: ActorIdentity,
        action: AuditAction,
    ) -> Result<AuditEntry, ProvenanceError> {
        let ts = action.occurred_at.clone();
        self.append_entry(actor, action, &ts, AuditOutcome::Accepted, None)
    }

    pub fn append_action_from_context(
        &mut self,
        context: ActionContext,
        action: AuditAction,
        ts: &str,
    ) -> Result<AuditEntry, ProvenanceError> {
        let action = normalize_audit_action(action)?;
        match context.resolve_actor(&action.action_ref) {
            Ok(actor) => self.append_entry(actor, action, ts, AuditOutcome::Accepted, None),
            Err(ProvenanceError::UnattributedAction { action_ref }) => {
                self.append_entry(
                    ActorIdentity::system(SYSTEM_AUDIT_ACTOR_ID),
                    action,
                    ts,
                    AuditOutcome::Refused,
                    Some(AuditRefusalReason::UnattributedAction),
                )?;
                Err(ProvenanceError::UnattributedAction { action_ref })
            }
            Err(error) => Err(error),
        }
    }

    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    pub fn verify_chain(&self) -> AuditChainVerification {
        verify_audit_chain(&self.entries)
    }

    pub fn replace_entry(
        &mut self,
        _seq: u64,
        replacement: AuditEntry,
    ) -> Result<(), ProvenanceError> {
        Err(ProvenanceError::AppendOnlyAuditLog {
            action_ref: replacement.action.action_ref,
            attempted_operation: "update".to_string(),
        })
    }

    pub fn delete_entry(&mut self, seq: u64) -> Result<(), ProvenanceError> {
        let action_ref = self
            .entries
            .iter()
            .find(|entry| entry.seq == seq)
            .map(|entry| entry.action.action_ref.clone())
            .unwrap_or_else(|| format!("seq:{seq}"));
        Err(ProvenanceError::AppendOnlyAuditLog {
            action_ref,
            attempted_operation: "delete".to_string(),
        })
    }

    fn append_entry(
        &mut self,
        actor: ActorIdentity,
        action: AuditAction,
        ts: &str,
        outcome: AuditOutcome,
        refusal_reason: Option<AuditRefusalReason>,
    ) -> Result<AuditEntry, ProvenanceError> {
        let actor = normalize_actor_identity(actor)?;
        let action = normalize_audit_action(action)?;
        let ts = normalize_required_text(
            ts.to_string(),
            ProvenanceError::EmptyActionTimestamp {
                action_ref: action.action_ref.clone(),
            },
        )?;
        let seq = self.entries.len() as u64 + 1;
        let prev_hash = self.entries.last().map(|entry| entry.entry_hash.clone());
        let payload_hash = audit_payload_hash(&action)?;
        let entry_hash = audit_entry_hash(
            seq,
            &prev_hash,
            &payload_hash,
            &actor,
            &ts,
            outcome,
            refusal_reason,
        )?;
        let entry = AuditEntry {
            seq,
            prev_hash,
            payload_hash,
            entry_hash,
            actor,
            ts,
            action,
            outcome,
            refusal_reason,
        };
        self.entries.push(entry.clone());
        Ok(entry)
    }
}

pub fn verify_audit_chain(entries: &[AuditEntry]) -> AuditChainVerification {
    for (index, entry) in entries.iter().enumerate() {
        let expected_seq = index as u64 + 1;
        if entry.seq != expected_seq {
            return audit_chain_breach(index, AuditChainBreachReason::SequenceMismatch);
        }

        let expected_prev_hash = if index == 0 {
            None
        } else {
            Some(entries[index - 1].entry_hash.clone())
        };
        if entry.prev_hash != expected_prev_hash {
            return audit_chain_breach(index, AuditChainBreachReason::PreviousHashMismatch);
        }

        let Ok(expected_payload_hash) = audit_payload_hash(&entry.action) else {
            return audit_chain_breach(index, AuditChainBreachReason::PayloadHashMismatch);
        };
        if entry.payload_hash != expected_payload_hash {
            return audit_chain_breach(index, AuditChainBreachReason::PayloadHashMismatch);
        }

        let Ok(expected_entry_hash) = audit_entry_hash(
            entry.seq,
            &entry.prev_hash,
            &entry.payload_hash,
            &entry.actor,
            &entry.ts,
            entry.outcome,
            entry.refusal_reason,
        ) else {
            return audit_chain_breach(index, AuditChainBreachReason::EntryHashMismatch);
        };
        if entry.entry_hash != expected_entry_hash {
            return audit_chain_breach(index, AuditChainBreachReason::EntryHashMismatch);
        }
    }

    AuditChainVerification {
        verified: true,
        verified_len: entries.len(),
        breach_index: None,
        reason: None,
    }
}

fn audit_chain_breach(index: usize, reason: AuditChainBreachReason) -> AuditChainVerification {
    AuditChainVerification {
        verified: false,
        verified_len: index,
        breach_index: Some(index),
        reason: Some(reason),
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
    record.actor = normalize_actor_identity(record.actor)?;
    record.created_at = normalize_required_text(
        record.created_at,
        ProvenanceError::EmptyCreatedAt {
            artifact_id: record.artifact_id.clone(),
        },
    )?;
    Ok(record)
}

fn normalize_actor_identity(mut actor: ActorIdentity) -> Result<ActorIdentity, ProvenanceError> {
    actor.actor_id = normalize_required_text(actor.actor_id, ProvenanceError::EmptyActorId)?;
    Ok(actor)
}

fn normalize_audit_action(mut action: AuditAction) -> Result<AuditAction, ProvenanceError> {
    action.action_ref =
        normalize_required_text(action.action_ref, ProvenanceError::EmptyActionRef)?;
    action.action_kind = normalize_required_text(
        action.action_kind,
        ProvenanceError::EmptyActionKind {
            action_ref: action.action_ref.clone(),
        },
    )?;
    action.artifact_ref = action.artifact_ref.and_then(normalize_optional_text_owned);
    action.occurred_at = normalize_required_text(
        action.occurred_at,
        ProvenanceError::EmptyActionTimestamp {
            action_ref: action.action_ref.clone(),
        },
    )?;
    Ok(action)
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
    digest_for_bytes(EVIDENCE_DIGEST_ALGORITHM, bytes)
}

fn audit_payload_hash(action: &AuditAction) -> Result<String, ProvenanceError> {
    let bytes =
        serde_json::to_vec(action).map_err(|error| ProvenanceError::AuditSerializationFailed {
            details: error.to_string(),
        })?;
    Ok(digest_for_bytes(EVIDENCE_DIGEST_ALGORITHM, &bytes))
}

#[derive(Serialize)]
struct AuditEntryHashMaterial<'a> {
    seq: u64,
    prev_hash: &'a Option<String>,
    payload_hash: &'a str,
    actor: &'a ActorIdentity,
    ts: &'a str,
    outcome: AuditOutcome,
    refusal_reason: Option<AuditRefusalReason>,
}

fn audit_entry_hash(
    seq: u64,
    prev_hash: &Option<String>,
    payload_hash: &str,
    actor: &ActorIdentity,
    ts: &str,
    outcome: AuditOutcome,
    refusal_reason: Option<AuditRefusalReason>,
) -> Result<String, ProvenanceError> {
    let material = AuditEntryHashMaterial {
        seq,
        prev_hash,
        payload_hash,
        actor,
        ts,
        outcome,
        refusal_reason,
    };
    let bytes = serde_json::to_vec(&material).map_err(|error| {
        ProvenanceError::AuditSerializationFailed {
            details: error.to_string(),
        }
    })?;
    Ok(digest_for_bytes(EVIDENCE_DIGEST_ALGORITHM, &bytes))
}

fn digest_for_bytes(algorithm: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!("{}:{}", algorithm, lowercase_hex(digest.as_slice()))
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

fn normalize_optional_text_owned(value: String) -> Option<String> {
    normalize_optional_text(&value)
}

#[cfg(test)]
mod tests {
    use super::{
        verify_audit_chain, ActionContext, ActorIdentity, ActorKind, ArtifactKind, AuditAction,
        AuditChainBreachReason, AuditLedger, AuditOutcome, AuditRefusalReason, EvidenceObject,
        EvidenceStore, LineageLedger, LineageRecord, ProvenanceError, ProvenanceParameters,
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
        assert_eq!(finding.actor, sample_actor());
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
    fn lineage_requires_formal_actor_identity() {
        let mut ledger = LineageLedger::default();
        seed_product(&mut ledger);
        let mut finding = finding_lineage();
        finding.actor.actor_id = " ".to_string();

        let error = ledger
            .record_lineage(finding)
            .expect_err("lineage without actor identity should be rejected");

        assert_eq!(error, ProvenanceError::EmptyActorId);
        assert!(ledger.fetch_lineage("finding:09:stress-ne-zone").is_none());
    }

    #[test]
    fn actor_context_appends_action_and_audits_missing_actor_refusal() {
        let mut audit = AuditLedger::default();

        let accepted = audit
            .append_action_from_context(
                ActionContext::new(
                    Some("operator:dsp-7".to_string()),
                    Some(ActorKind::Operator),
                ),
                sample_audit_action("action:field-boundary:update"),
                "2026-06-12T13:05:00Z",
            )
            .expect("authenticated actor should append audit entry");

        assert_eq!(accepted.actor, sample_actor());
        assert_eq!(accepted.outcome, AuditOutcome::Accepted);

        let error = audit
            .append_action_from_context(
                ActionContext::new(None, None),
                sample_audit_action("action:unattributed:update"),
                "2026-06-12T13:06:00Z",
            )
            .expect_err("missing actor should refuse action");

        assert_eq!(
            error,
            ProvenanceError::UnattributedAction {
                action_ref: "action:unattributed:update".to_string()
            }
        );
        let entries = audit.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].outcome, AuditOutcome::Refused);
        assert_eq!(
            entries[1].refusal_reason,
            Some(AuditRefusalReason::UnattributedAction)
        );
        assert_eq!(
            entries[1].actor,
            ActorIdentity::system("system:provenance-ledger")
        );
    }

    #[test]
    fn audit_log_is_append_only_and_hash_chained() {
        let mut audit = AuditLedger::default();
        let first = audit
            .append_action(sample_actor(), sample_audit_action("action:mission:create"))
            .expect("first action should append");
        let second = audit
            .append_action(
                sample_actor(),
                sample_audit_action("action:mission:approve"),
            )
            .expect("second action should append");

        assert_eq!(first.seq, 1);
        assert_eq!(first.prev_hash, None);
        assert_eq!(second.seq, 2);
        assert_eq!(second.prev_hash, Some(first.entry_hash.clone()));
        assert!(first.payload_hash.starts_with("sha256:"));
        assert!(second.entry_hash.starts_with("sha256:"));

        let verification = audit.verify_chain();
        assert!(verification.verified);
        assert_eq!(verification.verified_len, 2);
        assert_eq!(verification.breach_index, None);

        let error = audit
            .replace_entry(first.seq, first)
            .expect_err("audit entries should not be updateable");
        assert_eq!(
            error,
            ProvenanceError::AppendOnlyAuditLog {
                action_ref: "action:mission:create".to_string(),
                attempted_operation: "update".to_string()
            }
        );
    }

    #[test]
    fn audit_chain_verification_detects_edited_or_reordered_entries() {
        let mut audit = AuditLedger::default();
        audit
            .append_action(sample_actor(), sample_audit_action("action:mission:create"))
            .expect("first action should append");
        audit
            .append_action(
                sample_actor(),
                sample_audit_action("action:mission:approve"),
            )
            .expect("second action should append");

        let mut edited = audit.entries().to_vec();
        edited[1].action.payload = ProvenanceParameters::from_json(serde_json::json!({
            "field_id": "field:alpha",
            "approved": false
        }));
        let edited_verification = verify_audit_chain(&edited);
        assert!(!edited_verification.verified);
        assert_eq!(edited_verification.breach_index, Some(1));
        assert_eq!(
            edited_verification.reason,
            Some(AuditChainBreachReason::PayloadHashMismatch)
        );

        let mut reordered = audit.entries().to_vec();
        reordered.swap(0, 1);
        let reordered_verification = verify_audit_chain(&reordered);
        assert!(!reordered_verification.verified);
        assert_eq!(reordered_verification.breach_index, Some(0));
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

    #[test]
    fn backward_provenance_trace_includes_product_scene_and_capture() {
        let mut ledger = LineageLedger::default();
        seed_capture_graph(&mut ledger);

        let trace = ledger
            .trace_backward("finding:09:stress-ne-zone")
            .expect("backward trace should run");

        let artifact_ids = trace
            .records
            .iter()
            .map(|record| record.artifact_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            artifact_ids,
            vec![
                "finding:09:stress-ne-zone",
                "product:ndvi:alpha-2026-06-12",
                "scene:alpha-2026-06-12",
                "capture:alpha-2026-06-12"
            ]
        );
        assert!(trace.gaps.is_empty());
        assert_eq!(trace.records[1].method, "05.ndvi_product");
        assert_eq!(trace.records[2].method, "04.capture_session_scene");
        assert_eq!(trace.records[3].method, "04.capture_session");
    }

    #[test]
    fn backward_provenance_trace_reports_missing_input_gap() {
        let mut ledger = LineageLedger::default();
        let finding = finding_lineage();
        ledger.records.insert(finding.artifact_id.clone(), finding);

        let trace = ledger
            .trace_backward("finding:09:stress-ne-zone")
            .expect("incomplete trace should still return evidence");

        assert_eq!(trace.records.len(), 1);
        assert_eq!(trace.gaps.len(), 1);
        assert_eq!(
            trace.gaps[0].missing_artifact_id,
            "product:ndvi:alpha-2026-06-12"
        );
        assert_eq!(
            trace.gaps[0].referenced_by,
            Some("finding:09:stress-ne-zone".to_string())
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

    fn seed_capture_graph(ledger: &mut LineageLedger) {
        ledger
            .record_lineage(capture_lineage())
            .expect("capture lineage should be recorded");
        ledger
            .record_lineage(scene_lineage_with_capture())
            .expect("scene lineage should be recorded");
        ledger
            .record_lineage(product_lineage())
            .expect("product lineage should be recorded");
        ledger
            .record_lineage(finding_lineage())
            .expect("finding lineage should be recorded");
    }

    fn capture_lineage() -> LineageRecord {
        LineageRecord {
            artifact_id: "capture:alpha-2026-06-12".to_string(),
            kind: ArtifactKind::Capture,
            inputs: Vec::new(),
            method: "04.capture_session".to_string(),
            parameters: ProvenanceParameters::from_json(serde_json::json!({
                "flight_id": "flight:alpha-17",
                "platform": "agrodrone-7"
            })),
            operator: "operator:dsp-7".to_string(),
            actor: sample_actor(),
            created_at: "2026-06-12T11:45:00Z".to_string(),
        }
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
            actor: sample_actor(),
            created_at: "2026-06-12T12:00:00Z".to_string(),
        }
    }

    fn scene_lineage_with_capture() -> LineageRecord {
        LineageRecord {
            inputs: vec!["capture:alpha-2026-06-12".to_string()],
            ..scene_lineage()
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
            actor: sample_actor(),
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
            actor: sample_actor(),
            created_at: "2026-06-12T13:00:00Z".to_string(),
        }
    }

    fn sample_actor() -> ActorIdentity {
        ActorIdentity {
            actor_id: "operator:dsp-7".to_string(),
            actor_kind: ActorKind::Operator,
        }
    }

    fn sample_audit_action(action_ref: &str) -> AuditAction {
        AuditAction {
            action_ref: action_ref.to_string(),
            action_kind: "mission_mutation".to_string(),
            artifact_ref: Some("mission:alpha-17".to_string()),
            payload: ProvenanceParameters::from_json(serde_json::json!({
                "field_id": "field:alpha",
                "approved": true
            })),
            occurred_at: "2026-06-12T13:05:00Z".to_string(),
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
