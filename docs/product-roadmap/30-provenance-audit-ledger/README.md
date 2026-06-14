# Provenance and Audit Ledger

The capture→product→finding→report→action lineage chain: a cross-cutting trust backbone that records what produced every artifact, lets it be re-derived, and proves it was not tampered with.

## Where We Are

- Not started / cross-cutting greenfield (M0 named). No `provenance`/`ledger` crate exists; lineage is implicit and scattered across the deterministic-product domains.
- The pieces it would bind together are partially real: `04` sessions, `05`/`06`/`22` products, `09`/`23`/`28` findings, and `09` reports each carry their own IDs but no shared lineage or hash-chained audit trail.
- This is the "explainability and trust" pillar made real: it underpins the agronomy copilot (`26`, whose citations resolve to ledger evidence objects), carbon MRV (`19`), and supplies audit records to compliance (`24`).

## Where We Should Be

- Every artifact records its inputs, method, parameters, operator, and timestamp in a shared lineage model that threads through `04`/`05`/`06`/`09`/`22`/`23`/`28`.
- Evidence is content-addressed and the action log is append-only and hash-chained, so tampering is detectable.
- A provenance graph answers "what produced this finding?" and "what did this scene affect?" deterministically.
- A reproducibility manifest re-derives a product from its recorded inputs and parameters; a deterministic re-run yields an identical output (verified by hash).
- Evidence packs are exportable for MRV (`19`), compliance (`24`), and copilot citation (`26`), under a retention policy.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.
- `stories.md`: per-capability vertical-slice stories.

## Build Order

1. Lineage model: every artifact records `{inputs[], method, parameters, operator, timestamp}` with a stable artifact ID, threaded into one deterministic-product domain first (`09`).
2. Content-addressed evidence store: hash an evidence object and address it by digest.
3. Append-only, hash-chained audit log of actions (who did what, when) with tamper-evidence.
4. Provenance graph queries: forward ("what did this scene affect?") and backward ("what produced this finding?").
5. Reproducibility manifest and deterministic re-run with output-hash verification.
6. Evidence packs and retention/export for MRV (`19`), compliance (`24`), and copilot (`26`).

## Primary Crates

New crate `provenance` (or a `provenance` module in `shared`), threaded through `04`, `05`, `06`, `09`, `22`, `23`, and `28`. Consumes artifact IDs from those domains and supplies evidence objects/audit records to `19` (MRV), `24` (compliance), and `26` (copilot citations). `shared` holds the lineage and audit schemas.
