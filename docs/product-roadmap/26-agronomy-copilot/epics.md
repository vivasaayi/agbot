# Agronomy Copilot: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: ask/answer and conversation routes or commands with pagination, audit IDs, and resolvable citation refs.
- Grounding: every claim cites a real evidence object resolving to the `30` ledger; no claim is emitted without one — this is the dominant rule.
- Deterministic: the retrieval index, citation resolver, refusal guardrail, and audit logging run without the LLM; the LLM sits behind a mockable interface and is tested with a deterministic double.
- Explainability: uncertainty is always surfaced; the refusal path is a first-class, tested outcome; the copilot never precedes or replaces deterministic products.
- Agronomic: answers and drafted recommendations tie to a field, finding, zone, or change event — and drafts are advisor-reviewed, never auto-applied.
- Tests: unit (citation resolution, refusal guardrail, uncertainty), fixture (seeded evidence/conversations), API contract, and one failure path (ungrounded question refused).
- Operations: feature flag, LLM-interface health/timeout, retry/backoff, conversation audit via `30`, and a runbook.

## Category Epics

### EPIC-01: Grounded Retrieval and the LLM Boundary
- Goal: a copilot that can only speak about evidence it has retrieved and can cite.
- First release: an evidence-retrieval index over a field's products/findings/reports/trends (resolving to `30`), and the LLM behind a mockable, deterministic-evidence-grounded RAG interface.
- Expansion: grounded Q&A with mandatory citations and uncertainty surfacing.
- Hardening: the refusal/no-evidence guardrail with negative-path tests, and citation-resolution integrity against `30`.

### EPIC-02: Explanations, Conversation, and Audit
- Goal: useful, multi-turn, fully audited conversations grounded in evidence.
- First release: "explain this zone / finding / change" summarization that cites evidence (pulling change context from `28`), and a conversation audit log via `30`.
- Expansion: multi-turn field context and answer-with-citations export.
- Hardening: audit completeness (every turn persisted), replayability, and context-isolation tests (no cross-field leakage).

### EPIC-03: Advisor-Reviewed Drafting and Bounded Proactivity
- Goal: the copilot drafts actions for a human, never applies them.
- First release: advisor-reviewed recommendation drafting that writes into the `09`/`10` model only after review.
- Expansion: proactive surfacing of a cited finding with a drafted action.
- Hardening: the M5 closed-loop advisory stays approval-gated (consistent with `28`'s change hook), with tested "no approval → no action" paths.
