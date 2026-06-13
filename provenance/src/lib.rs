use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const EVIDENCE_DIGEST_ALGORITHM: &str = "sha256";
const EVIDENCE_PACK_SCHEMA_VERSION: &str = "provenance.evidence_pack.v1";
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
pub struct ReproducibilityManifest {
    pub product_id: String,
    pub input_digests: Vec<String>,
    pub method: String,
    pub method_version: String,
    pub parameters: ProvenanceParameters,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibilityManifestValidation {
    pub product_id: String,
    pub input_count: usize,
    pub missing_digests: Vec<String>,
    pub valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibilityInputBytes {
    pub digest: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReproducibilityMismatchReason {
    MethodVersionMismatch,
    OutputHashMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibilityVerification {
    pub product_id: String,
    pub reproducible: bool,
    pub expected_method_version: String,
    pub actual_method_version: String,
    pub expected_output_hash: String,
    pub actual_output_hash: String,
    pub input_count: usize,
    pub reason: Option<ReproducibilityMismatchReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePackRequest {
    pub target_artifact_id: String,
    #[serde(default)]
    pub evidence_digests: Vec<String>,
    #[serde(default)]
    pub citation_digests: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidencePack {
    pub schema_version: String,
    pub target_artifact_id: String,
    pub lineage: BackwardProvenanceTrace,
    pub evidence_objects: Vec<StoredEvidence>,
    pub audit_entries: Vec<AuditEntry>,
    pub manifests: Vec<ReproducibilityManifest>,
    pub citation_digests: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePackValidation {
    pub valid: bool,
    pub schema_version: String,
    pub target_artifact_id: String,
    pub lineage_record_count: usize,
    pub evidence_count: usize,
    pub audit_entry_count: usize,
    pub manifest_count: usize,
    pub citation_count: usize,
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
pub struct ReproducibilityManifestStore {
    manifests: BTreeMap<String, ReproducibilityManifest>,
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
    #[error("duplicate evidence digest {digest} in evidence pack")]
    DuplicateEvidenceDigest { digest: String },
    #[error("unsupported evidence digest algorithm {algorithm}")]
    UnsupportedEvidenceAlgorithm { algorithm: String },
    #[error("method_version cannot be empty for {product_id}")]
    EmptyMethodVersion { product_id: String },
    #[error("manifest input digest cannot be empty for {product_id}")]
    EmptyManifestInputDigest { product_id: String },
    #[error("manifest input digest {digest} appears more than once for {product_id}")]
    DuplicateManifestInputDigest { product_id: String, digest: String },
    #[error("lineage already exists for artifact {artifact_id}")]
    DuplicateArtifactId { artifact_id: String },
    #[error("reproducibility manifest already exists for product {product_id}")]
    DuplicateManifestProductId { product_id: String },
    #[error("unknown reproducibility manifest for product {product_id}")]
    UnknownManifestProductId { product_id: String },
    #[error("unknown input artifact {input_artifact_id} for {artifact_id}")]
    UnknownInputArtifact {
        artifact_id: String,
        input_artifact_id: String,
    },
    #[error("unknown evidence digest {digest}")]
    UnknownEvidenceDigest { digest: String },
    #[error("manifest for product {product_id} references missing input digest {digest}")]
    MissingManifestInputDigest { product_id: String, digest: String },
    #[error("rerun input digest {digest} is not listed in manifest for {product_id}")]
    UnexpectedManifestInputDigest { product_id: String, digest: String },
    #[error("evidence pack for {target_artifact_id} has unresolved citation digest {digest}")]
    UnresolvedEvidencePackCitation {
        target_artifact_id: String,
        digest: String,
    },
    #[error(
        "evidence pack schema_version must be provenance.evidence_pack.v1, got {schema_version}"
    )]
    InvalidEvidencePackSchemaVersion { schema_version: String },
    #[error("evidence pack for {target_artifact_id} has lineage gap {missing_artifact_id}")]
    EvidencePackLineageGap {
        target_artifact_id: String,
        missing_artifact_id: String,
    },
    #[error("evidence pack audit chain invalid at {breach_index:?}: {reason:?}")]
    InvalidEvidencePackAuditChain {
        breach_index: Option<usize>,
        reason: Option<AuditChainBreachReason>,
    },
    #[error("reproducibility manifest requires product artifact {artifact_id}, got {kind:?}")]
    ManifestRequiresProduct {
        artifact_id: String,
        kind: ArtifactKind,
    },
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

impl ReproducibilityManifestStore {
    pub fn record_manifest(
        &mut self,
        manifest: ReproducibilityManifest,
    ) -> Result<ReproducibilityManifest, ProvenanceError> {
        let manifest = normalize_reproducibility_manifest(manifest)?;
        if self.manifests.contains_key(&manifest.product_id) {
            return Err(ProvenanceError::DuplicateManifestProductId {
                product_id: manifest.product_id,
            });
        }

        self.manifests
            .insert(manifest.product_id.clone(), manifest.clone());
        Ok(manifest)
    }

    pub fn fetch_manifest(&self, product_id: &str) -> Option<ReproducibilityManifest> {
        let product_id = normalize_optional_text(product_id)?;
        self.manifests.get(&product_id).cloned()
    }

    pub fn validate_manifest_inputs(
        &self,
        product_id: &str,
        evidence_store: &EvidenceStore,
    ) -> Result<ReproducibilityManifestValidation, ProvenanceError> {
        let product_id =
            normalize_required_text(product_id.to_string(), ProvenanceError::EmptyArtifactId)?;
        let manifest = self.manifests.get(&product_id).ok_or_else(|| {
            ProvenanceError::UnknownManifestProductId {
                product_id: product_id.clone(),
            }
        })?;

        let mut missing_digests = Vec::new();
        for digest in &manifest.input_digests {
            if evidence_store.fetch_evidence(digest).is_none() {
                missing_digests.push(digest.clone());
            }
        }

        if let Some(digest) = missing_digests.first() {
            return Err(ProvenanceError::MissingManifestInputDigest {
                product_id,
                digest: digest.clone(),
            });
        }

        Ok(ReproducibilityManifestValidation {
            product_id,
            input_count: manifest.input_digests.len(),
            missing_digests,
            valid: true,
        })
    }

    pub fn manifest_count(&self) -> usize {
        self.manifests.len()
    }
}

impl AuditLedger {
    pub fn from_entries(entries: Vec<AuditEntry>) -> Result<Self, ProvenanceError> {
        let verification = verify_audit_chain(&entries);
        if !verification.verified {
            return Err(ProvenanceError::InvalidEvidencePackAuditChain {
                breach_index: verification.breach_index,
                reason: verification.reason,
            });
        }
        Ok(Self { entries })
    }

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

pub fn build_reproducibility_manifest(
    product: &LineageRecord,
    input_digests: Vec<String>,
    method_version: String,
) -> Result<ReproducibilityManifest, ProvenanceError> {
    let product = normalize_lineage_record(product.clone())?;
    if product.kind != ArtifactKind::Product {
        return Err(ProvenanceError::ManifestRequiresProduct {
            artifact_id: product.artifact_id,
            kind: product.kind,
        });
    }

    normalize_reproducibility_manifest(ReproducibilityManifest {
        product_id: product.artifact_id,
        input_digests,
        method: product.method,
        method_version,
        parameters: product.parameters,
    })
}

pub fn output_hash_for_bytes(bytes: &[u8]) -> String {
    digest_for_bytes(EVIDENCE_DIGEST_ALGORITHM, bytes)
}

pub fn verify_reproducible_output(
    manifest: &ReproducibilityManifest,
    inputs: &[ReproducibilityInputBytes],
    actual_method_version: &str,
    rerun_output_bytes: &[u8],
    expected_output_hash: &str,
) -> Result<ReproducibilityVerification, ProvenanceError> {
    let manifest = normalize_reproducibility_manifest(manifest.clone())?;
    let actual_method_version = normalize_required_text(
        actual_method_version.to_string(),
        ProvenanceError::EmptyMethodVersion {
            product_id: manifest.product_id.clone(),
        },
    )?;
    let expected_output_hash = normalize_required_text(
        expected_output_hash.to_string(),
        ProvenanceError::EmptyEvidenceDigest,
    )?;
    validate_reproducibility_inputs(&manifest, inputs)?;
    let actual_output_hash = output_hash_for_bytes(rerun_output_bytes);

    let reason = if actual_method_version != manifest.method_version {
        Some(ReproducibilityMismatchReason::MethodVersionMismatch)
    } else if actual_output_hash != expected_output_hash {
        Some(ReproducibilityMismatchReason::OutputHashMismatch)
    } else {
        None
    };

    Ok(ReproducibilityVerification {
        product_id: manifest.product_id,
        reproducible: reason.is_none(),
        expected_method_version: manifest.method_version,
        actual_method_version,
        expected_output_hash,
        actual_output_hash,
        input_count: manifest.input_digests.len(),
        reason,
    })
}

pub fn build_evidence_pack(
    lineage_ledger: &LineageLedger,
    evidence_store: &EvidenceStore,
    audit_ledger: &AuditLedger,
    manifest_store: &ReproducibilityManifestStore,
    request: EvidencePackRequest,
) -> Result<EvidencePack, ProvenanceError> {
    let request = normalize_evidence_pack_request(request)?;
    let lineage = lineage_ledger.trace_backward(&request.target_artifact_id)?;
    if let Some(gap) = lineage.gaps.first() {
        return Err(ProvenanceError::EvidencePackLineageGap {
            target_artifact_id: request.target_artifact_id,
            missing_artifact_id: gap.missing_artifact_id.clone(),
        });
    }
    if !lineage
        .records
        .iter()
        .any(|record| record.artifact_id == request.target_artifact_id)
    {
        return Err(ProvenanceError::EvidencePackLineageGap {
            target_artifact_id: request.target_artifact_id.clone(),
            missing_artifact_id: request.target_artifact_id,
        });
    }
    let mut manifests = Vec::new();
    let mut evidence_digests = request
        .evidence_digests
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for record in &lineage.records {
        if let Some(manifest) = manifest_store.fetch_manifest(&record.artifact_id) {
            evidence_digests.extend(manifest.input_digests.iter().cloned());
            manifests.push(manifest);
        }
    }

    let mut evidence_objects = Vec::with_capacity(evidence_digests.len());
    for digest in evidence_digests {
        let evidence = evidence_store.fetch_evidence(&digest).ok_or_else(|| {
            ProvenanceError::UnknownEvidenceDigest {
                digest: digest.clone(),
            }
        })?;
        evidence_objects.push(evidence);
    }
    evidence_objects.sort_by(|left, right| left.digest.cmp(&right.digest));
    manifests.sort_by(|left, right| left.product_id.cmp(&right.product_id));

    let evidence_digest_set = evidence_objects
        .iter()
        .map(|evidence| evidence.digest.clone())
        .collect::<BTreeSet<_>>();
    for digest in &request.citation_digests {
        if !evidence_digest_set.contains(digest) {
            return Err(ProvenanceError::UnresolvedEvidencePackCitation {
                target_artifact_id: request.target_artifact_id.clone(),
                digest: digest.clone(),
            });
        }
    }

    let pack = EvidencePack {
        schema_version: EVIDENCE_PACK_SCHEMA_VERSION.to_string(),
        target_artifact_id: request.target_artifact_id,
        lineage,
        evidence_objects,
        audit_entries: audit_ledger.entries().to_vec(),
        manifests,
        citation_digests: request.citation_digests,
    };
    verify_evidence_pack_schema(&pack)?;
    Ok(pack)
}

pub fn verify_evidence_pack_schema(
    pack: &EvidencePack,
) -> Result<EvidencePackValidation, ProvenanceError> {
    let schema_version = normalize_required_text(
        pack.schema_version.clone(),
        ProvenanceError::EvidenceSerializationFailed {
            details: "evidence pack schema_version cannot be empty".to_string(),
        },
    )?;
    if schema_version != EVIDENCE_PACK_SCHEMA_VERSION {
        return Err(ProvenanceError::InvalidEvidencePackSchemaVersion { schema_version });
    }
    let target_artifact_id = normalize_required_text(
        pack.target_artifact_id.clone(),
        ProvenanceError::EmptyArtifactId,
    )?;
    if pack.lineage.target_artifact_id != target_artifact_id {
        return Err(ProvenanceError::UnknownInputArtifact {
            artifact_id: target_artifact_id,
            input_artifact_id: pack.lineage.target_artifact_id.clone(),
        });
    }
    if let Some(gap) = pack.lineage.gaps.first() {
        return Err(ProvenanceError::EvidencePackLineageGap {
            target_artifact_id: target_artifact_id.clone(),
            missing_artifact_id: gap.missing_artifact_id.clone(),
        });
    }
    if !pack
        .lineage
        .records
        .iter()
        .any(|record| record.artifact_id == target_artifact_id)
    {
        return Err(ProvenanceError::EvidencePackLineageGap {
            target_artifact_id: target_artifact_id.clone(),
            missing_artifact_id: target_artifact_id.clone(),
        });
    }

    let mut evidence_digests = BTreeSet::new();
    for evidence in &pack.evidence_objects {
        let digest = verify_stored_evidence(evidence)?;
        if !evidence_digests.insert(digest.clone()) {
            return Err(ProvenanceError::DuplicateEvidenceDigest { digest });
        }
    }

    let audit_verification = verify_audit_chain(&pack.audit_entries);
    if !audit_verification.verified {
        return Err(ProvenanceError::InvalidEvidencePackAuditChain {
            breach_index: audit_verification.breach_index,
            reason: audit_verification.reason,
        });
    }

    for manifest in &pack.manifests {
        let manifest = normalize_reproducibility_manifest(manifest.clone())?;
        for digest in &manifest.input_digests {
            if !evidence_digests.contains(digest) {
                return Err(ProvenanceError::MissingManifestInputDigest {
                    product_id: manifest.product_id,
                    digest: digest.clone(),
                });
            }
        }
    }
    for digest in &pack.citation_digests {
        let digest = normalize_required_text(digest.clone(), ProvenanceError::EmptyEvidenceDigest)?;
        if !evidence_digests.contains(&digest) {
            return Err(ProvenanceError::UnresolvedEvidencePackCitation {
                target_artifact_id: pack.target_artifact_id.clone(),
                digest,
            });
        }
    }

    Ok(EvidencePackValidation {
        valid: true,
        schema_version,
        target_artifact_id: pack.target_artifact_id.clone(),
        lineage_record_count: pack.lineage.records.len(),
        evidence_count: pack.evidence_objects.len(),
        audit_entry_count: pack.audit_entries.len(),
        manifest_count: pack.manifests.len(),
        citation_count: pack.citation_digests.len(),
    })
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

fn verify_stored_evidence(evidence: &StoredEvidence) -> Result<String, ProvenanceError> {
    let digest = normalize_required_text(
        evidence.digest.clone(),
        ProvenanceError::EmptyEvidenceDigest,
    )?;
    let algorithm = normalize_required_text(
        evidence.algorithm.clone(),
        ProvenanceError::UnsupportedEvidenceAlgorithm {
            algorithm: evidence.algorithm.clone(),
        },
    )?;
    if algorithm != EVIDENCE_DIGEST_ALGORITHM {
        return Err(ProvenanceError::UnsupportedEvidenceAlgorithm { algorithm });
    }

    let object = normalize_evidence_object(evidence.object.clone())?;
    let expected_canonical_bytes = canonical_evidence_bytes(&object)?;
    let object_digest = evidence_digest_for_bytes(&expected_canonical_bytes);
    if object_digest != digest || expected_canonical_bytes != evidence.canonical_bytes {
        return Err(ProvenanceError::EvidenceDigestMismatch {
            expected_digest: digest,
            actual_digest: object_digest,
        });
    }

    let byte_digest = evidence_digest_for_bytes(&evidence.canonical_bytes);
    if byte_digest != digest {
        return Err(ProvenanceError::EvidenceDigestMismatch {
            expected_digest: digest,
            actual_digest: byte_digest,
        });
    }

    Ok(digest)
}

fn normalize_reproducibility_manifest(
    mut manifest: ReproducibilityManifest,
) -> Result<ReproducibilityManifest, ProvenanceError> {
    manifest.product_id =
        normalize_required_text(manifest.product_id, ProvenanceError::EmptyArtifactId)?;
    manifest.input_digests = manifest
        .input_digests
        .into_iter()
        .map(|digest| {
            normalize_required_text(
                digest,
                ProvenanceError::EmptyManifestInputDigest {
                    product_id: manifest.product_id.clone(),
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    manifest.method = normalize_required_text(
        manifest.method,
        ProvenanceError::EmptyMethod {
            artifact_id: manifest.product_id.clone(),
        },
    )?;
    manifest.method_version = normalize_required_text(
        manifest.method_version,
        ProvenanceError::EmptyMethodVersion {
            product_id: manifest.product_id.clone(),
        },
    )?;
    Ok(manifest)
}

fn normalize_evidence_pack_request(
    mut request: EvidencePackRequest,
) -> Result<EvidencePackRequest, ProvenanceError> {
    request.target_artifact_id =
        normalize_required_text(request.target_artifact_id, ProvenanceError::EmptyArtifactId)?;
    request.evidence_digests = request
        .evidence_digests
        .into_iter()
        .map(|digest| normalize_required_text(digest, ProvenanceError::EmptyEvidenceDigest))
        .collect::<Result<Vec<_>, _>>()?;
    request.citation_digests = request
        .citation_digests
        .into_iter()
        .map(|digest| normalize_required_text(digest, ProvenanceError::EmptyEvidenceDigest))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(request)
}

fn validate_reproducibility_inputs(
    manifest: &ReproducibilityManifest,
    inputs: &[ReproducibilityInputBytes],
) -> Result<(), ProvenanceError> {
    let manifest = normalize_reproducibility_manifest(manifest.clone())?;
    let mut input_by_digest = BTreeMap::new();
    for input in inputs {
        let digest = normalize_required_text(
            input.digest.clone(),
            ProvenanceError::EmptyManifestInputDigest {
                product_id: manifest.product_id.clone(),
            },
        )?;
        if input_by_digest
            .insert(digest.clone(), input.bytes.clone())
            .is_some()
        {
            return Err(ProvenanceError::DuplicateManifestInputDigest {
                product_id: manifest.product_id.clone(),
                digest,
            });
        }
    }

    for digest in &manifest.input_digests {
        if !input_by_digest.contains_key(digest) {
            return Err(ProvenanceError::MissingManifestInputDigest {
                product_id: manifest.product_id.clone(),
                digest: digest.clone(),
            });
        }
    }

    for digest in input_by_digest.keys() {
        if !manifest.input_digests.contains(digest) {
            return Err(ProvenanceError::UnexpectedManifestInputDigest {
                product_id: manifest.product_id.clone(),
                digest: digest.clone(),
            });
        }
    }

    Ok(())
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
        build_evidence_pack, build_reproducibility_manifest, output_hash_for_bytes,
        verify_audit_chain, verify_evidence_pack_schema, verify_reproducible_output, ActionContext,
        ActorIdentity, ActorKind, ArtifactKind, AuditAction, AuditChainBreachReason, AuditLedger,
        AuditOutcome, AuditRefusalReason, EvidenceObject, EvidencePackRequest, EvidenceStore,
        LineageLedger, LineageRecord, ProvenanceError, ProvenanceParameters,
        ReproducibilityInputBytes, ReproducibilityManifestStore, ReproducibilityMismatchReason,
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

    #[test]
    fn reproducibility_manifest_lists_input_digests_method_version_and_parameters() {
        let mut evidence_store = EvidenceStore::default();
        let scene_evidence = evidence_store
            .store_evidence(scene_evidence())
            .expect("scene evidence should store");
        let calibration_evidence = evidence_store
            .store_evidence(calibration_evidence())
            .expect("calibration evidence should store");
        let product = product_lineage();

        let manifest = build_reproducibility_manifest(
            &product,
            vec![
                scene_evidence.digest.clone(),
                calibration_evidence.digest.clone(),
            ],
            "05.ndvi_product.v2".to_string(),
        )
        .expect("manifest should build for product lineage");

        assert_eq!(manifest.product_id, "product:ndvi:alpha-2026-06-12");
        assert_eq!(
            manifest.input_digests,
            vec![scene_evidence.digest, calibration_evidence.digest]
        );
        assert_eq!(manifest.method, "05.ndvi_product");
        assert_eq!(manifest.method_version, "05.ndvi_product.v2");
        assert_eq!(
            manifest.parameters,
            ProvenanceParameters::from_json(serde_json::json!({
                "red_band": "B04",
                "nir_band": "B08"
            }))
        );

        let mut manifest_store = ReproducibilityManifestStore::default();
        let stored = manifest_store
            .record_manifest(manifest.clone())
            .expect("manifest should persist");
        assert_eq!(stored, manifest);
        assert_eq!(
            manifest_store.fetch_manifest("product:ndvi:alpha-2026-06-12"),
            Some(manifest.clone())
        );

        let validation = manifest_store
            .validate_manifest_inputs("product:ndvi:alpha-2026-06-12", &evidence_store)
            .expect("all input digests should validate");
        assert!(validation.valid);
        assert_eq!(validation.input_count, 2);
        assert!(validation.missing_digests.is_empty());
    }

    #[test]
    fn reproducibility_manifest_validation_fails_on_missing_input_digest() {
        let mut evidence_store = EvidenceStore::default();
        let present = evidence_store
            .store_evidence(scene_evidence())
            .expect("scene evidence should store");
        let missing = "sha256:missing-input-digest".to_string();
        let manifest = build_reproducibility_manifest(
            &product_lineage(),
            vec![present.digest, missing.clone()],
            "05.ndvi_product.v2".to_string(),
        )
        .expect("manifest should build");
        let mut manifest_store = ReproducibilityManifestStore::default();
        manifest_store
            .record_manifest(manifest)
            .expect("manifest should persist");

        let error = manifest_store
            .validate_manifest_inputs("product:ndvi:alpha-2026-06-12", &evidence_store)
            .expect_err("missing input digest should block validation");

        assert_eq!(
            error,
            ProvenanceError::MissingManifestInputDigest {
                product_id: "product:ndvi:alpha-2026-06-12".to_string(),
                digest: missing
            }
        );
    }

    #[test]
    fn reproducibility_manifest_requires_product_lineage() {
        let error = build_reproducibility_manifest(
            &scene_lineage(),
            vec!["sha256:scene-input".to_string()],
            "04.capture_session_scene.v1".to_string(),
        )
        .expect_err("scene lineage should not get a product manifest");

        assert_eq!(
            error,
            ProvenanceError::ManifestRequiresProduct {
                artifact_id: "scene:alpha-2026-06-12".to_string(),
                kind: ArtifactKind::Scene,
            }
        );
    }

    #[test]
    fn reproducible_rerun_matches_expected_output_hash() {
        let manifest = sample_reproducibility_manifest();
        let inputs = sample_reproducibility_inputs(&manifest);
        let output_bytes = b"ndvi product bytes v2";
        let expected_hash = output_hash_for_bytes(output_bytes);

        let verification = verify_reproducible_output(
            &manifest,
            &inputs,
            "05.ndvi_product.v2",
            output_bytes,
            &expected_hash,
        )
        .expect("verification should run");

        assert!(verification.reproducible);
        assert_eq!(verification.expected_output_hash, expected_hash);
        assert_eq!(verification.actual_output_hash, expected_hash);
        assert_eq!(verification.reason, None);
        assert_eq!(verification.input_count, manifest.input_digests.len());
    }

    #[test]
    fn rerun_flags_method_version_mismatch() {
        let manifest = sample_reproducibility_manifest();
        let inputs = sample_reproducibility_inputs(&manifest);
        let output_bytes = b"ndvi product bytes v2";
        let expected_hash = output_hash_for_bytes(output_bytes);

        let verification = verify_reproducible_output(
            &manifest,
            &inputs,
            "05.ndvi_product.v3",
            output_bytes,
            &expected_hash,
        )
        .expect("verification should flag mismatch");

        assert!(!verification.reproducible);
        assert_eq!(
            verification.reason,
            Some(ReproducibilityMismatchReason::MethodVersionMismatch)
        );
        assert_eq!(verification.expected_method_version, "05.ndvi_product.v2");
        assert_eq!(verification.actual_method_version, "05.ndvi_product.v3");
        assert_eq!(verification.actual_output_hash, expected_hash);
    }

    #[test]
    fn rerun_flags_altered_input_hash_mismatch() {
        let manifest = sample_reproducibility_manifest();
        let inputs = sample_reproducibility_inputs(&manifest);
        let expected_hash = output_hash_for_bytes(b"ndvi product bytes v2");
        let altered_output_bytes = b"ndvi product bytes v2\n";

        let verification = verify_reproducible_output(
            &manifest,
            &inputs,
            "05.ndvi_product.v2",
            altered_output_bytes,
            &expected_hash,
        )
        .expect("verification should flag altered rerun output");

        assert!(!verification.reproducible);
        assert_eq!(
            verification.reason,
            Some(ReproducibilityMismatchReason::OutputHashMismatch)
        );
        assert_ne!(verification.actual_output_hash, expected_hash);
    }

    #[test]
    fn rerun_refuses_missing_manifest_input_digest() {
        let manifest = sample_reproducibility_manifest();
        let inputs = vec![sample_reproducibility_inputs(&manifest)[0].clone()];

        let error = verify_reproducible_output(
            &manifest,
            &inputs,
            "05.ndvi_product.v2",
            b"ndvi product bytes v2",
            "sha256:expected-output",
        )
        .expect_err("missing manifest input should refuse rerun");

        assert_eq!(
            error,
            ProvenanceError::MissingManifestInputDigest {
                product_id: "product:ndvi:alpha-2026-06-12".to_string(),
                digest: "sha256:calibration-input".to_string(),
            }
        );
    }

    #[test]
    fn rerun_rejects_duplicate_or_extra_input_digests() {
        let manifest = sample_reproducibility_manifest();
        let mut duplicate_inputs = sample_reproducibility_inputs(&manifest);
        duplicate_inputs.push(duplicate_inputs[0].clone());

        let duplicate_error = verify_reproducible_output(
            &manifest,
            &duplicate_inputs,
            "05.ndvi_product.v2",
            b"ndvi product bytes v2",
            "sha256:expected-output",
        )
        .expect_err("duplicate input digest should be rejected");

        assert_eq!(
            duplicate_error,
            ProvenanceError::DuplicateManifestInputDigest {
                product_id: "product:ndvi:alpha-2026-06-12".to_string(),
                digest: "sha256:scene-input".to_string(),
            }
        );

        let mut extra_inputs = sample_reproducibility_inputs(&manifest);
        extra_inputs.push(ReproducibilityInputBytes {
            digest: "sha256:unexpected".to_string(),
            bytes: b"unexpected bytes".to_vec(),
        });

        let extra_error = verify_reproducible_output(
            &manifest,
            &extra_inputs,
            "05.ndvi_product.v2",
            b"ndvi product bytes v2",
            "sha256:expected-output",
        )
        .expect_err("extra input digest should be rejected");

        assert_eq!(
            extra_error,
            ProvenanceError::UnexpectedManifestInputDigest {
                product_id: "product:ndvi:alpha-2026-06-12".to_string(),
                digest: "sha256:unexpected".to_string(),
            }
        );
    }

    #[test]
    fn evidence_pack_exports_lineage_evidence_audit_and_manifest_with_resolved_citations() {
        let mut ledger = LineageLedger::default();
        seed_capture_graph(&mut ledger);
        let mut evidence_store = EvidenceStore::default();
        let finding_evidence = evidence_store
            .store_evidence(sample_evidence())
            .expect("finding evidence should store");
        let scene_input = evidence_store
            .store_evidence(scene_evidence())
            .expect("scene input evidence should store");
        let calibration_input = evidence_store
            .store_evidence(calibration_evidence())
            .expect("calibration input evidence should store");
        let mut manifest_store = ReproducibilityManifestStore::default();
        manifest_store
            .record_manifest(
                build_reproducibility_manifest(
                    &product_lineage(),
                    vec![scene_input.digest.clone(), calibration_input.digest.clone()],
                    "05.ndvi_product.v2".to_string(),
                )
                .expect("manifest should build"),
            )
            .expect("manifest should persist");
        let mut audit = AuditLedger::default();
        audit
            .append_action(
                sample_actor(),
                audit_action_for_artifact(
                    "action:product:ndvi:derive",
                    "product:ndvi:alpha-2026-06-12",
                ),
            )
            .expect("product audit should append");
        audit
            .append_action(
                sample_actor(),
                audit_action_for_artifact(
                    "action:finding:stress:create",
                    "finding:09:stress-ne-zone",
                ),
            )
            .expect("finding audit should append");

        let pack = build_evidence_pack(
            &ledger,
            &evidence_store,
            &audit,
            &manifest_store,
            EvidencePackRequest {
                target_artifact_id: "finding:09:stress-ne-zone".to_string(),
                evidence_digests: vec![finding_evidence.digest.clone()],
                citation_digests: vec![finding_evidence.digest.clone(), scene_input.digest.clone()],
            },
        )
        .expect("evidence pack should build");

        assert_eq!(pack.schema_version, "provenance.evidence_pack.v1");
        assert_eq!(pack.target_artifact_id, "finding:09:stress-ne-zone");
        assert_eq!(pack.lineage.records.len(), 4);
        assert_eq!(pack.evidence_objects.len(), 3);
        assert_eq!(pack.audit_entries.len(), 2);
        assert_eq!(pack.manifests.len(), 1);
        assert_eq!(
            pack.citation_digests,
            vec![finding_evidence.digest, scene_input.digest]
        );
        let validation = verify_evidence_pack_schema(&pack).expect("pack schema should validate");
        assert!(validation.valid);
        assert_eq!(validation.evidence_count, 3);
        assert_eq!(validation.citation_count, 2);
    }

    #[test]
    fn evidence_pack_refuses_unresolved_copilot_citation() {
        let mut ledger = LineageLedger::default();
        seed_capture_graph(&mut ledger);
        let evidence_store = EvidenceStore::default();
        let audit = AuditLedger::default();
        let manifest_store = ReproducibilityManifestStore::default();

        let error = build_evidence_pack(
            &ledger,
            &evidence_store,
            &audit,
            &manifest_store,
            EvidencePackRequest {
                target_artifact_id: "finding:09:stress-ne-zone".to_string(),
                evidence_digests: Vec::new(),
                citation_digests: vec!["sha256:missing-citation".to_string()],
            },
        )
        .expect_err("dangling copilot citation should refuse pack");

        assert_eq!(
            error,
            ProvenanceError::UnresolvedEvidencePackCitation {
                target_artifact_id: "finding:09:stress-ne-zone".to_string(),
                digest: "sha256:missing-citation".to_string(),
            }
        );
    }

    #[test]
    fn evidence_pack_refuses_missing_target_lineage_gap() {
        let ledger = LineageLedger::default();
        let evidence_store = EvidenceStore::default();
        let audit = AuditLedger::default();
        let manifest_store = ReproducibilityManifestStore::default();

        let error = build_evidence_pack(
            &ledger,
            &evidence_store,
            &audit,
            &manifest_store,
            EvidencePackRequest {
                target_artifact_id: "finding:missing".to_string(),
                evidence_digests: Vec::new(),
                citation_digests: Vec::new(),
            },
        )
        .expect_err("pack should refuse missing target lineage");

        assert_eq!(
            error,
            ProvenanceError::EvidencePackLineageGap {
                target_artifact_id: "finding:missing".to_string(),
                missing_artifact_id: "finding:missing".to_string(),
            }
        );
    }

    #[test]
    fn evidence_pack_schema_rejects_bad_schema_tampered_evidence_and_bad_audit_chain() {
        let mut ledger = LineageLedger::default();
        seed_capture_graph(&mut ledger);
        let mut evidence_store = EvidenceStore::default();
        let finding_evidence = evidence_store
            .store_evidence(sample_evidence())
            .expect("finding evidence should store");
        let mut audit = AuditLedger::default();
        audit
            .append_action(
                sample_actor(),
                audit_action_for_artifact(
                    "action:finding:stress:create",
                    "finding:09:stress-ne-zone",
                ),
            )
            .expect("audit should append");
        let manifest_store = ReproducibilityManifestStore::default();
        let pack = build_evidence_pack(
            &ledger,
            &evidence_store,
            &audit,
            &manifest_store,
            EvidencePackRequest {
                target_artifact_id: "finding:09:stress-ne-zone".to_string(),
                evidence_digests: vec![finding_evidence.digest],
                citation_digests: Vec::new(),
            },
        )
        .expect("base pack should build");

        let mut bad_schema = pack.clone();
        bad_schema.schema_version = "bogus".to_string();
        assert_eq!(
            verify_evidence_pack_schema(&bad_schema).expect_err("schema version should be exact"),
            ProvenanceError::InvalidEvidencePackSchemaVersion {
                schema_version: "bogus".to_string(),
            }
        );

        let mut tampered_evidence = pack.clone();
        tampered_evidence.evidence_objects[0].object.payload = serde_json::json!({
            "tampered": true
        });
        assert!(matches!(
            verify_evidence_pack_schema(&tampered_evidence),
            Err(ProvenanceError::EvidenceDigestMismatch { .. })
        ));

        let mut bad_audit = pack;
        bad_audit.audit_entries[0].seq = 99;
        assert_eq!(
            verify_evidence_pack_schema(&bad_audit).expect_err("audit chain should validate"),
            ProvenanceError::InvalidEvidencePackAuditChain {
                breach_index: Some(0),
                reason: Some(AuditChainBreachReason::SequenceMismatch),
            }
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

    fn audit_action_for_artifact(action_ref: &str, artifact_ref: &str) -> AuditAction {
        AuditAction {
            action_ref: action_ref.to_string(),
            action_kind: "artifact_mutation".to_string(),
            artifact_ref: Some(artifact_ref.to_string()),
            payload: ProvenanceParameters::from_json(serde_json::json!({
                "artifact_ref": artifact_ref,
                "changed": true
            })),
            occurred_at: "2026-06-12T13:10:00Z".to_string(),
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

    fn scene_evidence() -> EvidenceObject {
        EvidenceObject {
            evidence_kind: "scene_input".to_string(),
            payload: serde_json::json!({
                "scene_ref": "scene:alpha-2026-06-12",
                "bands": ["B04", "B08"],
                "crs": "EPSG:32610"
            }),
        }
    }

    fn calibration_evidence() -> EvidenceObject {
        EvidenceObject {
            evidence_kind: "calibration_input".to_string(),
            payload: serde_json::json!({
                "calibration_ref": "calibration:multispectral-rig-2:2026-06",
                "panel_reflectance": 0.72
            }),
        }
    }

    fn sample_reproducibility_manifest() -> super::ReproducibilityManifest {
        build_reproducibility_manifest(
            &product_lineage(),
            vec![
                "sha256:scene-input".to_string(),
                "sha256:calibration-input".to_string(),
            ],
            "05.ndvi_product.v2".to_string(),
        )
        .expect("manifest should build")
    }

    fn sample_reproducibility_inputs(
        manifest: &super::ReproducibilityManifest,
    ) -> Vec<ReproducibilityInputBytes> {
        vec![
            ReproducibilityInputBytes {
                digest: manifest.input_digests[0].clone(),
                bytes: b"scene bytes v1".to_vec(),
            },
            ReproducibilityInputBytes {
                digest: manifest.input_digests[1].clone(),
                bytes: b"calibration bytes v1".to_vec(),
            },
        ]
    }
}
