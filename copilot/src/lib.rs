use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopilotAnswerRequest {
    pub question: String,
    pub retrieved_evidence: Vec<EvidenceIndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopilotAnswer {
    pub text: String,
    pub cited_evidence_ids: Vec<String>,
    pub confidence: f64,
    pub model_provider: String,
    pub model_id: String,
    pub model_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeterministicAnswerFixture {
    pub question: String,
    pub text: String,
    pub cited_evidence_ids: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeterministicCopilotModel {
    model_provider: String,
    model_id: String,
    model_version: String,
    fixtures: BTreeMap<String, DeterministicAnswerFixture>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnavailableCopilotModel {
    adapter_name: String,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CopilotModelError {
    #[error("question cannot be empty")]
    EmptyQuestion,
    #[error("answer text cannot be empty")]
    EmptyAnswerText,
    #[error("model_provider cannot be empty")]
    EmptyModelProvider,
    #[error("model_id cannot be empty")]
    EmptyModelId,
    #[error("model_version cannot be empty")]
    EmptyModelVersion,
    #[error("cited_evidence_ids cannot contain empty values")]
    EmptyCitation,
    #[error("confidence must be finite and between 0 and 1")]
    InvalidConfidence,
    #[error("no deterministic answer fixture matched question {question}")]
    FixtureNotFound { question: String },
    #[error("cited evidence {evidence_id} was not in retrieved evidence")]
    CitationNotRetrieved { evidence_id: String },
    #[error("copilot model adapter unavailable: {reason}")]
    AdapterUnavailable { reason: String },
}

pub trait CopilotModel {
    fn answer(&self, request: CopilotAnswerRequest) -> Result<CopilotAnswer, CopilotModelError>;
}

impl DeterministicCopilotModel {
    pub fn new(
        model_provider: String,
        model_id: String,
        model_version: String,
        fixtures: Vec<DeterministicAnswerFixture>,
    ) -> Result<Self, CopilotModelError> {
        let model_provider =
            normalize_model_text(model_provider, CopilotModelError::EmptyModelProvider)?;
        let model_id = normalize_model_text(model_id, CopilotModelError::EmptyModelId)?;
        let model_version =
            normalize_model_text(model_version, CopilotModelError::EmptyModelVersion)?;
        let mut normalized_fixtures = BTreeMap::new();

        for fixture in fixtures {
            let normalized = normalize_fixture(fixture)?;
            normalized_fixtures.insert(normalized.question.clone(), normalized);
        }

        Ok(Self {
            model_provider,
            model_id,
            model_version,
            fixtures: normalized_fixtures,
        })
    }
}

impl CopilotModel for DeterministicCopilotModel {
    fn answer(&self, request: CopilotAnswerRequest) -> Result<CopilotAnswer, CopilotModelError> {
        let question = normalize_model_text(request.question, CopilotModelError::EmptyQuestion)?;
        let fixture =
            self.fixtures
                .get(&question)
                .ok_or_else(|| CopilotModelError::FixtureNotFound {
                    question: question.clone(),
                })?;
        let retrieved_ids = request
            .retrieved_evidence
            .into_iter()
            .filter_map(|entry| normalize_text(entry.evidence_id))
            .collect::<BTreeSet<_>>();

        for evidence_id in &fixture.cited_evidence_ids {
            if !retrieved_ids.contains(evidence_id) {
                return Err(CopilotModelError::CitationNotRetrieved {
                    evidence_id: evidence_id.clone(),
                });
            }
        }

        Ok(CopilotAnswer {
            text: fixture.text.clone(),
            cited_evidence_ids: fixture.cited_evidence_ids.clone(),
            confidence: fixture.confidence,
            model_provider: self.model_provider.clone(),
            model_id: self.model_id.clone(),
            model_version: self.model_version.clone(),
        })
    }
}

impl UnavailableCopilotModel {
    pub fn new(adapter_name: String, reason: String) -> Self {
        Self {
            adapter_name: normalize_text(adapter_name).unwrap_or_else(|| "unknown".to_string()),
            reason: normalize_text(reason).unwrap_or_else(|| "unavailable".to_string()),
        }
    }
}

impl CopilotModel for UnavailableCopilotModel {
    fn answer(&self, _request: CopilotAnswerRequest) -> Result<CopilotAnswer, CopilotModelError> {
        Err(CopilotModelError::AdapterUnavailable {
            reason: format!("{}: {}", self.adapter_name, self.reason),
        })
    }
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

fn normalize_model_text(
    value: String,
    error: CopilotModelError,
) -> Result<String, CopilotModelError> {
    normalize_text(value).ok_or(error)
}

fn normalize_fixture(
    fixture: DeterministicAnswerFixture,
) -> Result<DeterministicAnswerFixture, CopilotModelError> {
    let question = normalize_model_text(fixture.question, CopilotModelError::EmptyQuestion)?;
    let text = normalize_model_text(fixture.text, CopilotModelError::EmptyAnswerText)?;
    validate_confidence(fixture.confidence)?;
    let cited_evidence_ids = fixture
        .cited_evidence_ids
        .into_iter()
        .map(|evidence_id| normalize_model_text(evidence_id, CopilotModelError::EmptyCitation))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DeterministicAnswerFixture {
        question,
        text,
        cited_evidence_ids,
        confidence: fixture.confidence,
    })
}

fn validate_confidence(confidence: f64) -> Result<(), CopilotModelError> {
    if confidence.is_finite() && (0.0..=1.0).contains(&confidence) {
        Ok(())
    } else {
        Err(CopilotModelError::InvalidConfidence)
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
        build_evidence_retrieval_index, CopilotAnswerRequest, CopilotIndexError, CopilotModel,
        CopilotModelError, DeterministicAnswerFixture, DeterministicCopilotModel,
        EvidenceCandidate, EvidenceIndexEntry, EvidenceKind, EvidenceRejectionReason,
        LedgerEvidenceResolver, UnavailableCopilotModel,
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

    #[test]
    fn deterministic_model_returns_fixture_answer_with_citations_and_version() {
        let model = DeterministicCopilotModel::new(
            "test-double".to_string(),
            "fixture-rag".to_string(),
            "2026-06-12".to_string(),
            vec![DeterministicAnswerFixture {
                question: "why is the northeast zone stressed?".to_string(),
                text: "The northeast zone is stressed because NDVI dropped below threshold."
                    .to_string(),
                cited_evidence_ids: vec!["evidence-ndvi-001".to_string()],
                confidence: 0.82,
            }],
        )
        .expect("fixture model should be valid");

        let answer = model
            .answer(CopilotAnswerRequest {
                question: " why is the northeast zone stressed? ".to_string(),
                retrieved_evidence: vec![retrieved_evidence("evidence-ndvi-001")],
            })
            .expect("fixture answer should be returned");

        assert_eq!(
            answer.text,
            "The northeast zone is stressed because NDVI dropped below threshold."
        );
        assert_eq!(answer.cited_evidence_ids, vec!["evidence-ndvi-001"]);
        assert_eq!(answer.confidence, 0.82);
        assert_eq!(answer.model_provider, "test-double");
        assert_eq!(answer.model_id, "fixture-rag");
        assert_eq!(answer.model_version, "2026-06-12");
    }

    #[test]
    fn deterministic_model_rejects_fixture_citation_not_in_retrieved_evidence() {
        let model = DeterministicCopilotModel::new(
            "test-double".to_string(),
            "fixture-rag".to_string(),
            "2026-06-12".to_string(),
            vec![DeterministicAnswerFixture {
                question: "what changed?".to_string(),
                text: "NDVI changed in the northeast zone.".to_string(),
                cited_evidence_ids: vec!["evidence-missing".to_string()],
                confidence: 0.7,
            }],
        )
        .expect("fixture model should be valid");

        let error = model
            .answer(CopilotAnswerRequest {
                question: "what changed?".to_string(),
                retrieved_evidence: vec![retrieved_evidence("evidence-ndvi-001")],
            })
            .expect_err("unretrieved citation should fail");

        assert_eq!(
            error,
            CopilotModelError::CitationNotRetrieved {
                evidence_id: "evidence-missing".to_string()
            }
        );
    }

    #[test]
    fn unavailable_model_surfaces_failure_without_fabricated_answer() {
        let model = UnavailableCopilotModel::new(
            "live-adapter".to_string(),
            "deployment model timed out".to_string(),
        );
        let error = model
            .answer(CopilotAnswerRequest {
                question: "why is the crop stressed?".to_string(),
                retrieved_evidence: vec![retrieved_evidence("evidence-ndvi-001")],
            })
            .expect_err("unavailable model should fail cleanly");

        assert_eq!(
            error,
            CopilotModelError::AdapterUnavailable {
                reason: "live-adapter: deployment model timed out".to_string()
            }
        );
    }

    fn retrieved_evidence(evidence_id: &str) -> EvidenceIndexEntry {
        EvidenceIndexEntry {
            evidence_id: evidence_id.to_string(),
            kind: EvidenceKind::ImageryProduct,
            field_id: "field-001".to_string(),
            scene_ref: Some("scene-2026-06-01".to_string()),
            zone_ref: Some("zone-ne".to_string()),
            ledger_ref: format!("ledger:30:{evidence_id}"),
            summary: "NDVI in the northeast zone dropped below threshold.".to_string(),
        }
    }
}
