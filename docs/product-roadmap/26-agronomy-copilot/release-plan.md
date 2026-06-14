# Agronomy Copilot: Release Plan

## Shipment Strategy

Ship in maturity order with the explainability/trust pillar leading every phase, because an ungrounded answer is worse than no answer. The evidence-retrieval index and the mockable LLM boundary come first (M1), then grounded conversation capture and audit (M2), then the deterministic trust core — mandatory citations, the refusal/no-evidence guardrail, and uncertainty surfacing (M3). Explanations, multi-turn context, and advisor-reviewed drafting land in M4. The proactive, approval-gated advisory (M5) is gated behind a reliable grounding/refusal core and stays consistent with `28`'s closed-loop change hook. The copilot is sequenced after the advisor MVP (`09`) and the provenance ledger (`30`): citations must resolve to real evidence objects before any answer ships. It never replaces or precedes deterministic products.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 14 |
| M2 captured | 10 |
| M3 explainable | 20 |
| M4 interactive | 14 |
| M5 autonomous-assist | 4 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 12 |
| P1 | 32 |
| P2 | 18 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 9 |
| M | 33 |
| S | 20 |

## First P0/P1 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | Evidence-retrieval index | explainability and trust | identity |
| M1 foundation | M | LLM boundary as a mockable interface | operability | interface |
| M3 explainable | M | Grounded Q&A with mandatory citations | explainability and trust | evaluator |
| M3 explainable | M | Refusal / no-evidence guardrail | explainability and trust | evaluator |
| M3 explainable | S | Uncertainty surfacing | explainability and trust | evaluator |
| M3 explainable | M | Explain zone / finding / change (cites `28`) | agronomic value | evaluator |
| M4 interactive | M | Advisor-reviewed recommendation drafting | agronomic value | interaction |
| M4 interactive | M | Conversation audit log via `30` | explainability and trust | audit |

## Execution Rules

- The copilot may only assert a claim that cites a real evidence object resolving to the `30` ledger; no claim without a resolvable citation, ever.
- The copilot never replaces or precedes deterministic, inspectable products; it summarizes and cites them.
- When no grounding evidence exists, the copilot must refuse rather than speculate; the refusal path is a tested, first-class outcome.
- Every answer must surface uncertainty; low-confidence or partial-evidence answers say so.
- The LLM is a true external boundary behind a mockable interface; all logic and tests run against a deterministic test double — domain logic is never mocked to pass.
- Every Q&A turn (question, answer, cited evidence, interface/model version) is persisted and audited via `30`.
- Drafted recommendations are advisor-reviewed and never auto-applied; the M5 proactive advisory stays approval-gated, consistent with `28`'s closed-loop hook.
