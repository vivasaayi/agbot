# Agronomy Copilot: Detailed Stories

> Greenfield domain (M0 named): no code exists yet. Every story below is **built from scratch** and is gated behind the advisor MVP (`09`) and the provenance/evidence ledger (`30`) — citations must resolve to real evidence objects before any answer ships. The **explainability/trust pillar dominates every phase**: the copilot may only assert a claim that cites a real evidence object, never replaces or precedes deterministic products, always surfaces uncertainty, refuses rather than speculates when ungrounded, and audits every turn via `30`. The LLM is a true external boundary behind a mockable interface (deterministic-evidence-grounded RAG); for model choice it references the latest Claude models (e.g. `claude-opus-4-8`) behind that interface, but the architecture — not a specific model — is the contract, and tests run against a deterministic double.

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without the LLM.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M1 — Foundation

### STORY 26-01 · M1 · M · P0 — Evidence-retrieval index
- **Story**: As `AG`, I want a field's products, findings, reports, and trends indexed for retrieval, so that the copilot has a concrete, citable evidence set to answer from.
- **Deterministic / evidence**: build an index of evidence items `{evidence_id, kind, field_id, scene/zone_ref, ledger_ref, summary}` where every `ledger_ref` resolves to a `30` evidence object; the index is built deterministically without the LLM.
- **Acceptance**:
  - Given a field's `09` findings, `05`/`06` products, and `28` trends, when the index builds, then each entry carries a resolvable `30` ledger ref.
  - Given an item whose `30` ledger ref does not resolve, when indexing runs, then it is excluded (or flagged), never indexed as citable.
- **Tests**: unit (index build + ledger resolution), fixture (seeded field evidence), failure path (unresolvable ref excluded).
- **Depends on**: `09`, `30`, `05`/`06`, `28`, `07`/`10`.

### STORY 26-02 · M1 · M · P0 — LLM boundary as a mockable interface
- **Story**: As `PA`, I want the LLM behind a single grounded-RAG interface with a deterministic test double, so that the platform is not coupled to one model and is testable without a live model.
- **Deterministic / evidence**: define `CopilotModel { answer(question, retrieved_evidence) -> {text, cited_evidence_ids[], confidence} }`; a deterministic double returns fixed answers for fixtures; the live adapter targets the latest Claude models (e.g. `claude-opus-4-8`) but the interface, not the model, is the contract; the interface version is recorded.
- **Acceptance**:
  - Given the test double, when the interface is called, then a deterministic answer with citation IDs and confidence is returned.
  - Given the live adapter is unavailable/times out, when called, then the failure is surfaced cleanly (no fabricated answer).
- **Tests**: unit (interface contract), integration (double swap), failure path (adapter timeout surfaced).
- **Depends on**: 26-01.

### STORY 26-03 · M1 · S · P1 — Conversation and turn identity
- **Story**: As `AG`, I want each conversation and turn to have a stable ID scoped to a field, so that turns are traceable and auditable.
- **Deterministic / evidence**: persist `{conversation_id, field_id, turn_id, role, created_at}`; a turn cannot exist without a field scope; IDs are stable across restart.
- **Acceptance**:
  - Given a field, when a conversation starts, then a conversation row is created scoped to that field.
  - Given a turn without a valid field scope, when created, then it is rejected.
- **Tests**: unit (identity + scope), API contract (start/list), failure path (no field scope rejected).
- **Depends on**: 26-01, `07`/`10`.

---

## M2 — Captured / Observable

### STORY 26-04 · M2 · M · P1 — Conversation audit log via `30`
- **Story**: As `DSP`, I want every Q&A turn (question, answer, cited evidence, interface version) persisted and audited via `30`, so that a conversation is as defensible as a report.
- **Deterministic / evidence**: each completed turn writes `{question, answer, cited_evidence_ids[], confidence, interface_version, ts}` to the `30` audit trail; the write happens deterministically regardless of the LLM content.
- **Acceptance**:
  - Given a completed turn, when it is finalized, then a `30` audit record is written with the question, answer, and cited evidence IDs.
  - Given the audit write fails, when a turn completes, then the answer is not returned as final (no un-audited answers).
- **Tests**: fixture (turn → audit record), API contract (audit retrieval), failure path (audit write failure blocks final answer).
- **Depends on**: 26-02, 26-03, `30`.

### STORY 26-05 · M2 · S · P1 — Multi-turn field context
- **Story**: As `AG`, I want the copilot to carry field/scene context across turns, so that follow-up questions ("and the NE zone?") resolve correctly.
- **Deterministic / evidence**: a deterministic context object carries `{field_id, active_scene, active_zone, last_evidence_ids}`; context is scoped to one field and never bleeds across fields/conversations.
- **Acceptance**:
  - Given a conversation about a field, when a follow-up omits the field, then the carried context resolves it to the same field.
  - Given a follow-up that would reference another field's evidence, when resolved, then it is isolated (no cross-field leakage).
- **Tests**: unit (context carry), integration (multi-turn), failure path (cross-field isolation).
- **Depends on**: 26-03.

---

## M3 — Explainable (the deterministic trust core)

### STORY 26-06 · M3 · M · P0 — Grounded Q&A with mandatory citations
- **Story**: As `AG`, I want answers that cite the specific evidence objects they rest on, so that I can verify every claim against the deterministic layer.
- **Deterministic / evidence**: an answer is accepted only if every claim carries ≥1 `cited_evidence_id` that resolves to a `30` evidence object in the retrieved set; a deterministic post-check strips/rejects any uncited claim before the answer is returned.
- **Acceptance**:
  - Given a question with relevant indexed evidence, when answered, then the answer cites resolvable evidence objects and the post-check passes.
  - Given an answer containing a claim with no resolvable citation, when post-checked, then that claim is rejected and the answer is not returned as grounded.
- **Tests**: unit (citation post-check), integration (double + index), failure path (uncited claim rejected).
- **Depends on**: 26-01, 26-02.

### STORY 26-07 · M3 · M · P0 — Refusal / no-evidence guardrail
- **Story**: As `AG`, I want the copilot to refuse rather than guess when it has no grounding, so that I can trust that an answer always rests on evidence.
- **Deterministic / evidence**: before answering, a deterministic check confirms the retrieval set is non-empty and relevant; if no grounding evidence exists, the copilot returns a structured refusal `{refused, reason=no_evidence}` and never calls the model for a speculative answer.
- **Acceptance**:
  - Given a question with no relevant indexed evidence, when asked, then the copilot refuses with `no_evidence` and offers what data would be needed.
  - Given a question whose only "evidence" fails to resolve in `30`, when asked, then it refuses (never speculates on unresolved refs).
- **Tests**: unit (refusal trigger), integration (empty index → refusal), failure path (unresolved-ref refusal).
- **Depends on**: 26-01, 26-06.

### STORY 26-08 · M3 · S · P0 — Uncertainty surfacing
- **Story**: As `AG`, I want every answer to carry a confidence/uncertainty marker, so that I do not over-trust a partial-evidence answer.
- **Deterministic / evidence**: confidence is derived deterministically from evidence coverage (how much of the claim set is cited, freshness of evidence via `28`/`30`); low coverage forces a "low confidence / partial evidence" marker regardless of model tone.
- **Acceptance**:
  - Given a fully cited answer with fresh evidence, when returned, then it carries a high-confidence marker.
  - Given an answer resting on stale or partial evidence, when returned, then it carries a low-confidence/partial marker (never silently confident).
- **Tests**: unit (confidence derivation), presentation (marker always present), failure path (stale evidence → low marker).
- **Depends on**: 26-06.

### STORY 26-09 · M3 · M · P1 — Explain this zone / finding / change (cites `28`)
- **Story**: As `AG`, I want to ask "explain the NE zone" or "what changed since last flight?" and get a summary that cites its evidence, so that I understand a finding or change without reading raw data.
- **Deterministic / evidence**: the copilot retrieves the finding (`09`), zone, and the ranked change event/trend (`28`), and summarizes them citing each source; the change explanation reuses `28`'s evidence (aligned pair, mask, event) rather than recomputing it.
- **Acceptance**:
  - Given a finding with a `28` change event, when asked to explain, then the summary cites the finding, the zone, and the change event's evidence.
  - Given a request to explain a change where `28` reports "no baseline," when asked, then the copilot says there is no comparable history (no invented change).
- **Tests**: integration (`09` + `28` retrieval), unit (citation assembly), failure path (no baseline → no invented change).
- **Depends on**: 26-06, `09`, `28`.

---

## M4 — Interactive

### STORY 26-10 · M4 · M · P0 — Advisor-reviewed recommendation drafting
- **Story**: As `AG`, I want the copilot to draft a recommendation I can review and edit, so that I get a head start without the AI ever applying an action on its own.
- **Deterministic / evidence**: a drafted recommendation is created in a `draft` state with `{cited_evidence_ids[], zone_ref}` and is inert; it writes into the `09`/`10` recommendation model only after an explicit advisor review/approval; an unreviewed draft is never treated as active.
- **Acceptance**:
  - Given a grounded answer, when the copilot drafts a recommendation, then it is stored as a reviewable draft citing its evidence, with no field action taken.
  - Given an unreviewed draft, when the system checks active recommendations, then the draft is excluded (never auto-applied).
- **Tests**: API contract (draft/review/approve), unit (draft inertness), failure path (unreviewed draft not active).
- **Depends on**: 26-06, 26-08, `09`/`10`.

### STORY 26-11 · M4 · S · P1 — Answer + citations export
- **Story**: As `DSP`, I want to export an answer together with its resolvable citations, so that a client can verify the basis of advice.
- **Deterministic / evidence**: export bundles `{question, answer, confidence, cited_evidence[]}` where each citation includes a resolvable `30` ledger ref; the export validates against a schema.
- **Acceptance**:
  - Given a grounded answer, when exported, then the bundle includes every citation with a resolvable ledger ref and validates.
  - Given an answer that was a refusal, when exported, then the export records the refusal and its reason (not an empty/fabricated answer).
- **Tests**: schema validation, unit (citation resolution in export), failure path (refusal exported faithfully).
- **Depends on**: 26-06, 26-07, 26-04.

---

## M5 — Autonomous-Assist (grounded, approval-gated)

### STORY 26-12 · M5 · M · P2 — Proactive closed-loop advisory (approval-gated)
- **Story**: As `AG`, I want the copilot to proactively surface a cited finding and draft an approval-gated action, so that important changes reach me with a next step — without anything executing on its own.
- **Deterministic / evidence**: when `28` emits a high-severity change event for a field, the copilot composes a proactive advisory citing that event's evidence and drafts an approval-gated action (consistent with `28-20`'s closed-loop hook); the advisory and draft are inert until a human approves; every proactive turn is audited via `30`.
- **Acceptance**:
  - Given a high-severity `28` change event, when the proactive advisory runs, then a cited advisory and an approval-gated draft action are produced and audited, and nothing executes.
  - Given the advisory's evidence cannot be resolved in `30`, when it runs, then no advisory is surfaced (no ungrounded proactive claim); and with no approval, no action is taken.
- **Tests**: integration (`28` event → advisory), unit (approval gate), failure path (unresolved evidence → no advisory; no approval → no action).
- **Depends on**: 26-07, 26-10, `28`, `30`, `09`/`01`.

---

## Coverage note

These 12 stories cover all 11 capabilities in `capability-map.md`. The breakdown is M1/M3-weighted, with a deliberately heavy M3 trust core — grounded Q&A with mandatory citations (26-06), the refusal/no-evidence guardrail (26-07), and uncertainty surfacing (26-08) — because the copilot's value rests entirely on never asserting an ungrounded claim. The LLM stays a true external boundary behind a mockable interface (26-02), and every turn is audited via `30` (26-04). Drafted recommendations are advisor-reviewed and never auto-applied (26-10), and the single M5 story (26-12) stays grounded and approval-gated, consistent with `28`'s closed-loop change hook. The curated counts in `release-plan.md` (≈62 rows) expand several of these (per-evidence-kind retrieval adapters, additional guardrail and uncertainty slices, and per-format export variants) into sibling stories when implemented.
