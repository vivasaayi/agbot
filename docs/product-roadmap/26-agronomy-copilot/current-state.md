# Agronomy Copilot: Current State and Target State

## Mission

Let an agronomist ask a field's data questions in natural language and get answers that are grounded in real, deterministic evidence: every claim cites a resolvable evidence object from the deterministic layer and the provenance ledger (`30`), uncertainty is always surfaced, the copilot refuses when it has no grounding, it drafts recommendations for advisor review rather than applying them, and every turn is audited — making "evidence before advice" conversational.

## Current Maturity

greenfield pending (M0 named): no implementation exists. There is no `copilot` crate, no evidence-retrieval index, no grounded Q&A, no refusal guardrail, and no conversation audit. The deterministic surfaces this copilot must cite are partially real, but nothing yet retrieves or grounds against them.

## What Exists Now

- Nothing is built for this domain. There is no LLM interface, retrieval index, citation resolver, or conversation store.
- Adjacent surfaces it would ground on (already partially real):
  - Domain `09` (post-flight advisor): the findings and recommendations the copilot summarizes and cites; it must never precede or replace these deterministic products.
  - Domain `30` (provenance/audit ledger): the evidence objects every citation must resolve to, and the audit trail every Q&A turn is written into — the critical dependency.
  - Domains `05`/`06` (imagery / LiDAR): the georeferenced products a question can be grounded against.
  - Domain `28` (time-series and change detection): the trends and ranked change events the copilot pulls from to explain "what changed since last flight?"
  - Domains `07`/`10` (GIS hub / field-farm-data): the field/farm/season context that scopes a conversation.

## Gaps to Close

- No evidence-retrieval index over a field's products, findings, reports, and trends that resolves to `30` evidence objects.
- No LLM boundary as a mockable interface; the model must be a true external boundary behind a deterministic-evidence-grounded RAG interface.
- No grounded Q&A that emits a claim only when it cites a resolvable evidence object.
- No refusal path: nothing today forces the copilot to decline rather than speculate when grounding is absent.
- No uncertainty surfacing on answers.
- No "explain this zone / finding / change" summarization that cites evidence (including `28` change context).
- No advisor-reviewed recommendation drafting; nothing prevents an AI suggestion from being treated as applied.
- No conversation audit log via `30` (question, answer, cited evidence per turn).
- No multi-turn field context, and no answer-with-citations export.
- No proactive/closed-loop advisory consistent with `28`'s approval-gated change hook.

## Related Existing Surfaces

- Domain `09` (post-flight advisor): findings/recommendations to summarize and cite; the deterministic products the copilot must never replace.
- Domain `30` (provenance/audit ledger): the evidence objects citations resolve to and the audit store for every Q&A turn — the critical dependency.
- Domain `28` (time-series and change detection): ranked change events and trends the copilot cites when explaining change.
- Domains `05`/`06` (imagery / LiDAR): georeferenced products to ground answers against.
- Domains `07`/`10` (GIS hub / field-farm-data): field/farm/season context that scopes the conversation.
- `docs/product-doctrine.md` ("What Not To Build"): "do not let AI yield/health claims replace or precede deterministic, inspectable products" — the rule this domain operationalizes.

## Target Operating Model

- Evidence before advice, conversationally: the copilot retrieves real evidence objects first and only asserts claims it can cite back to them; it never replaces or precedes deterministic products.
- Hard grounding rule: no claim without a resolvable `30` evidence object. When grounding is absent, the copilot refuses (a tested path) rather than speculating.
- Uncertainty is always surfaced: every answer carries a confidence/uncertainty marker, and low-confidence or partial-evidence answers say so.
- The LLM is a true external boundary behind a mockable interface (deterministic-evidence-grounded RAG); it references the latest Claude models (e.g. `claude-opus-4-8`) behind that interface, but the architecture, not a specific model, is the contract. Tests run against a deterministic test double.
- Advisor-in-the-loop: the copilot drafts recommendations for human review and never auto-applies them; drafted actions flow into the `09`/`10` recommendation model only after review.
- Full audit: every turn (question, answer, cited evidence, model/interface version) is persisted via `30`, so a conversation is as defensible and replayable as a report.
- Bounded proactivity last: a proactive advisory surfaces a cited finding and drafts an approval-gated action, consistent with `28`'s closed-loop change hook — never executed without human approval.
