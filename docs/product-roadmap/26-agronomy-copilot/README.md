# Agronomy Copilot

Natural-language Q&A over a field's data that only asserts claims it can cite back to a real evidence object: "evidence before advice" made conversational, and a platform differentiator.

## Where We Are

- Not started / greenfield (M0 named). No `copilot` crate exists; there is no evidence-retrieval index, no grounded Q&A, and no conversation audit.
- The surfaces it would ground on are partially real: `09` produces findings/recommendations, `05`/`06`/`28` produce products and trends, and `07`/`10` hold field context. The provenance/evidence ledger this copilot must cite (domain `30`) is the critical dependency that makes citations resolvable.
- The doctrine is explicit: AI must never replace or precede deterministic, inspectable products, and any AI output must cite its evidence layer and flag uncertainty. This domain is where that rule becomes a conversation.

## Where We Should Be

- A copilot that answers "why is the NE zone stressed?" or "what changed since last flight?" by retrieving and citing real evidence objects from the deterministic layer and the provenance ledger (`30`) — and refuses when no grounding evidence exists.
- Every claim cites a resolvable evidence object; uncertainty is always surfaced; the copilot drafts recommendations for advisor review and never auto-applies them.
- Every Q&A turn (question, answer, cited evidence) is persisted and audited via `30`, so a conversation is as defensible as a report.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P1 slices.
- `stories.md`: detailed vertical-slice stories.

## Build Order

1. Evidence-retrieval index over a field's products, findings, reports, and trends, resolving to `30` ledger evidence objects.
2. The LLM boundary as a mockable interface (deterministic-evidence-grounded RAG), with no claim emitted without a cited evidence object.
3. Grounded Q&A with mandatory citations and a tested refusal path when no evidence exists.
4. "Explain this zone / finding / change" summarization that cites evidence (pulling change context from `28`).
5. Conversation audit log via `30` and multi-turn field context.
6. Advisor-reviewed recommendation drafting and answer-with-citations export; then the M5 proactive, approval-gated advisory.

## Primary Crates

New crate `copilot` with the LLM behind a mockable interface, `shared` for schemas. Grounds on `09` (findings/recommendations), `30` (provenance/evidence ledger — citations resolve here), `07`/`10` (field context), and `05`/`06`/`28` (products and trends). Model/provider choice is deployment configuration recorded as `model_provider`, `model_id`, and `model_version`; the roadmap contract is the versioned boundary, deterministic test double, evidence citations, refusal behavior, and audit trail.
