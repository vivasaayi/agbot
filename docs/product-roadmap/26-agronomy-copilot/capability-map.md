# Agronomy Copilot: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (explainability and trust first, then agronomic value, data quality, geospatial correctness) and the workstreams in `release-plan.md`. Because this is a greenfield domain (M0 named), every capability's current source status is "missing (greenfield)", and the Primary First Slice describes the M1 foundation step. The explainability/trust pillar dominates: the copilot may only assert claims that cite a real evidence object, never replaces or precedes deterministic products, always surfaces uncertainty, refuses when ungrounded, and audits every turn via `30`. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Agronomy Copilot Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Evidence-retrieval index | missing (greenfield) | 8 | Index a field's products/findings/reports/trends to `30` refs |
| LLM boundary as a mockable interface | missing (greenfield) | 6 | Grounded-RAG interface with a deterministic test double |
| Grounded Q&A with mandatory citations | missing (greenfield) | 9 | Answer a question citing resolvable evidence objects |
| Refusal / no-evidence guardrail | missing (greenfield) | 6 | Refuse rather than speculate when no grounding exists |
| Uncertainty surfacing | missing (greenfield) | 5 | Every answer carries an uncertainty/confidence marker |
| Explain zone / finding / change (cites `28`) | missing (greenfield) | 7 | Summarize a finding/change citing its evidence |
| Advisor-reviewed recommendation drafting | missing (greenfield) | 6 | Draft a recommendation for review; never auto-apply |
| Conversation audit log via `30` | missing (greenfield) | 6 | Persist every {question, answer, citations} turn |
| Multi-turn field context | missing (greenfield) | 5 | Carry field/scene context across a conversation |
| Answer + citations export | missing (greenfield) | 4 | Export an answer with its resolvable citations |
| Proactive closed-loop advisory (approval-gated) | missing (greenfield) | 4 | Surface a cited finding + draft an approval-gated action |
