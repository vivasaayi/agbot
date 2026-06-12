use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Finding,
    ImageryProduct,
    LidarProduct,
    Report,
    Trend,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceCandidate {
    pub evidence_id: String,
    pub kind: EvidenceKind,
    pub field_id: String,
    pub scene_ref: Option<String>,
    pub zone_ref: Option<String>,
    pub ledger_ref: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceIndexEntry {
    pub evidence_id: String,
    pub kind: EvidenceKind,
    pub field_id: String,
    pub scene_ref: Option<String>,
    pub zone_ref: Option<String>,
    pub ledger_ref: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRetrievalIndex {
    pub field_id: String,
    pub entries: Vec<EvidenceIndexEntry>,
    pub rejected_items: Vec<RejectedEvidenceItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRejectionReason {
    DuplicateEvidenceId,
    EmptyEvidenceId,
    EmptyLedgerRef,
    EmptySummary,
    FieldMismatch,
    UnresolvedLedgerRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RejectedEvidenceItem {
    pub evidence_id: Option<String>,
    pub ledger_ref: Option<String>,
    pub reason: EvidenceRejectionReason,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CopilotIndexError {
    #[error("field_id cannot be empty")]
    EmptyFieldId,
}

pub trait LedgerEvidenceResolver {
    fn resolves_ledger_ref(&self, ledger_ref: &str) -> bool;
}

pub fn build_evidence_retrieval_index(
    field_id: String,
    candidates: Vec<EvidenceCandidate>,
    resolver: &impl LedgerEvidenceResolver,
) -> Result<EvidenceRetrievalIndex, CopilotIndexError> {
    let field_id = normalize_required_text(field_id, CopilotIndexError::EmptyFieldId)?;
    let mut entries = Vec::new();
    let mut rejected_items = Vec::new();
    let mut seen_evidence_ids = BTreeSet::new();

    for candidate in candidates {
        let evidence_id = normalize_text(candidate.evidence_id);
        let ledger_ref = normalize_text(candidate.ledger_ref);

        let Some(evidence_id) = evidence_id else {
            rejected_items.push(rejected_item(
                None,
                ledger_ref,
                EvidenceRejectionReason::EmptyEvidenceId,
            ));
            continue;
        };

        if !seen_evidence_ids.insert(evidence_id.clone()) {
            rejected_items.push(rejected_item(
                Some(evidence_id),
                ledger_ref,
                EvidenceRejectionReason::DuplicateEvidenceId,
            ));
            continue;
        }

        let candidate_field_id = normalize_text(candidate.field_id);
        if candidate_field_id.as_deref() != Some(field_id.as_str()) {
            rejected_items.push(rejected_item(
                Some(evidence_id),
                ledger_ref,
                EvidenceRejectionReason::FieldMismatch,
            ));
            continue;
        }

        let Some(ledger_ref) = ledger_ref else {
            rejected_items.push(rejected_item(
                Some(evidence_id),
                None,
                EvidenceRejectionReason::EmptyLedgerRef,
            ));
            continue;
        };

        let Some(summary) = normalize_text(candidate.summary) else {
            rejected_items.push(rejected_item(
                Some(evidence_id),
                Some(ledger_ref),
                EvidenceRejectionReason::EmptySummary,
            ));
            continue;
        };

        if !resolver.resolves_ledger_ref(&ledger_ref) {
            rejected_items.push(rejected_item(
                Some(evidence_id),
                Some(ledger_ref),
                EvidenceRejectionReason::UnresolvedLedgerRef,
            ));
            continue;
        }

        entries.push(EvidenceIndexEntry {
            evidence_id,
            kind: candidate.kind,
            field_id: field_id.clone(),
            scene_ref: normalize_optional_text(candidate.scene_ref),
            zone_ref: normalize_optional_text(candidate.zone_ref),
            ledger_ref,
            summary,
        });
    }

    entries.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    rejected_items.sort_by(|left, right| {
        left.evidence_id
            .cmp(&right.evidence_id)
            .then(left.ledger_ref.cmp(&right.ledger_ref))
            .then((left.reason as u8).cmp(&(right.reason as u8)))
    });

    Ok(EvidenceRetrievalIndex {
        field_id,
        entries,
        rejected_items,
    })
}

fn rejected_item(
    evidence_id: Option<String>,
    ledger_ref: Option<String>,
    reason: EvidenceRejectionReason,
) -> RejectedEvidenceItem {
    RejectedEvidenceItem {
        evidence_id,
        ledger_ref,
        reason,
    }
}

fn normalize_required_text(
    value: String,
    error: CopilotIndexError,
) -> Result<String, CopilotIndexError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(normalize_text)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        build_evidence_retrieval_index, CopilotIndexError, EvidenceCandidate, EvidenceKind,
        EvidenceRejectionReason, LedgerEvidenceResolver,
    };

    struct FixtureLedger {
        refs: BTreeSet<String>,
    }

    impl LedgerEvidenceResolver for FixtureLedger {
        fn resolves_ledger_ref(&self, ledger_ref: &str) -> bool {
            self.refs.contains(ledger_ref)
        }
    }

    #[test]
    fn evidence_index_requires_resolvable_ledger_refs() {
        let ledger = FixtureLedger {
            refs: BTreeSet::from(["ledger:30:ndvi:001".to_string()]),
        };
        let index = build_evidence_retrieval_index(
            " field-001 ".to_string(),
            vec![
                EvidenceCandidate {
                    evidence_id: "evidence-ndvi-001".to_string(),
                    kind: EvidenceKind::ImageryProduct,
                    field_id: "field-001".to_string(),
                    scene_ref: Some("scene-2026-06-01".to_string()),
                    zone_ref: Some("zone-ne".to_string()),
                    ledger_ref: "ledger:30:ndvi:001".to_string(),
                    summary: "NDVI in the northeast zone dropped below 0.42.".to_string(),
                },
                EvidenceCandidate {
                    evidence_id: "evidence-trend-missing-ledger".to_string(),
                    kind: EvidenceKind::Trend,
                    field_id: "field-001".to_string(),
                    scene_ref: None,
                    zone_ref: Some("zone-ne".to_string()),
                    ledger_ref: "ledger:30:trend:missing".to_string(),
                    summary: "Unresolved trend should not be citable.".to_string(),
                },
            ],
            &ledger,
        )
        .expect("index should build");

        assert_eq!(index.field_id, "field-001");
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].evidence_id, "evidence-ndvi-001");
        assert_eq!(index.entries[0].ledger_ref, "ledger:30:ndvi:001");
        assert_eq!(index.rejected_items.len(), 1);
        assert_eq!(
            index.rejected_items[0].evidence_id.as_deref(),
            Some("evidence-trend-missing-ledger")
        );
        assert_eq!(
            index.rejected_items[0].reason,
            EvidenceRejectionReason::UnresolvedLedgerRef
        );
    }

    #[test]
    fn evidence_index_is_field_scoped_and_deterministically_ordered() {
        let ledger = FixtureLedger {
            refs: BTreeSet::from([
                "ledger:30:report:002".to_string(),
                "ledger:30:finding:001".to_string(),
            ]),
        };
        let index = build_evidence_retrieval_index(
            "field-001".to_string(),
            vec![
                EvidenceCandidate {
                    evidence_id: "evidence-report-002".to_string(),
                    kind: EvidenceKind::Report,
                    field_id: "field-001".to_string(),
                    scene_ref: None,
                    zone_ref: None,
                    ledger_ref: "ledger:30:report:002".to_string(),
                    summary: "Advisor report summarized northeast-zone stress.".to_string(),
                },
                EvidenceCandidate {
                    evidence_id: "evidence-other-field".to_string(),
                    kind: EvidenceKind::Finding,
                    field_id: "field-002".to_string(),
                    scene_ref: None,
                    zone_ref: None,
                    ledger_ref: "ledger:30:finding:001".to_string(),
                    summary: "Other-field finding must not enter this field index.".to_string(),
                },
                EvidenceCandidate {
                    evidence_id: "evidence-finding-001".to_string(),
                    kind: EvidenceKind::Finding,
                    field_id: "field-001".to_string(),
                    scene_ref: Some("scene-2026-06-01".to_string()),
                    zone_ref: Some("zone-ne".to_string()),
                    ledger_ref: "ledger:30:finding:001".to_string(),
                    summary: "Finding cites the same stressed northeast zone.".to_string(),
                },
            ],
            &ledger,
        )
        .expect("index should build");

        let evidence_ids = index
            .entries
            .iter()
            .map(|entry| entry.evidence_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            evidence_ids,
            vec!["evidence-finding-001", "evidence-report-002"]
        );
        assert_eq!(index.rejected_items.len(), 1);
        assert_eq!(
            index.rejected_items[0].reason,
            EvidenceRejectionReason::FieldMismatch
        );
    }

    #[test]
    fn evidence_index_rejects_empty_field_scope() {
        let ledger = FixtureLedger {
            refs: BTreeSet::new(),
        };
        let error = build_evidence_retrieval_index(" ".to_string(), Vec::new(), &ledger)
            .expect_err("empty field scope should be rejected");

        assert_eq!(error, CopilotIndexError::EmptyFieldId);
    }
}
