# Provenance and Audit Ledger: Current State and Target State

## Mission

Be the trust backbone of the platform: record the lineage of every artifact (capture → product → finding → report → action), store evidence content-addressed, keep an append-only hash-chained log of who did what and when, answer provenance queries in both directions, and let any product be re-derived and proven untampered — so every output is defensible, citable, and auditable.

## Current Maturity

cross-cutting greenfield (M0 named): no implementation exists. There is no `provenance`/`ledger` crate or `shared` module that records lineage, content-addresses evidence, or maintains a hash-chained audit log. Each deterministic-product domain carries its own IDs, but there is no shared lineage chain, no tamper-evidence, and no reproducibility manifest.

## What Exists Now

- Nothing is built for this domain. There is no lineage model, evidence store, audit log, provenance graph, or reproducibility manifest.
- Adjacent surfaces it would thread through and bind (already partially real):
  - Domain `04` (sensor acquisition): capture sessions and sensor/source metadata — the head of the lineage chain.
  - Domains `05`/`06`/`22` (imagery / LiDAR / orthomosaic): deterministic products with statistics and parameters that the lineage model must record as inputs and methods.
  - Domains `09`/`23`/`28` (advisor / pest-disease / change-detection): findings whose evidence and reason codes the ledger must capture and make citable.
  - Domain `09` (`post_processor`): `AnalysisResult` and the report generator — artifacts that need a reproducibility manifest and an evidence pack.
  - Domains `19`/`24`/`26` (carbon MRV / compliance / copilot): consumers that need exportable evidence packs and audit records (copilot citations must resolve to ledger evidence objects).

## Gaps to Close

- No shared lineage model: artifacts do not record `{inputs[], method, parameters, operator, timestamp}` in a common, queryable form.
- No content-addressed evidence store: evidence is not hashed or addressed by digest, so identical inputs are not deduplicated and integrity cannot be checked.
- No append-only, hash-chained audit log of mutating actions; no tamper-evidence and no chain verification.
- No provenance graph: neither "what produced this finding?" (backward) nor "what did this scene affect?" (forward) can be answered.
- No reproducibility manifest and no deterministic re-run that proves an identical output by hash.
- No evidence packs for MRV (`19`), compliance (`24`), or copilot citation (`26`); copilot answers cannot yet resolve a citation to a ledger evidence object.
- No retention policy or ledger export, and no consistent actor/operator attribution across domains.

## Related Existing Surfaces

- Domain `04` (sensor acquisition): capture sessions and source metadata — the lineage chain head.
- Domains `05`/`06`/`22` (imagery / LiDAR / orthomosaic): deterministic products and their parameters to record as inputs/methods.
- Domains `09`/`23`/`28` (advisor / pest-disease / change-detection): findings, evidence, and reason codes to capture and make citable.
- Domains `19`/`24`/`26` (carbon MRV / compliance / copilot): evidence-pack and audit-record consumers.
- `shared/src/schemas.rs`: where the lineage, evidence, and audit schemas would live.

## Target Operating Model

- Every artifact (session, product, finding, report, action) carries a stable ID and a recorded lineage `{inputs[], method, parameters, operator, timestamp}` in a shared model.
- Evidence is content-addressed by cryptographic digest; identical inputs deduplicate, and any byte change is detectable.
- A single append-only, hash-chained audit log records every mutating action (who, what, when); a broken or reordered chain fails verification — tampering is detectable, not assumed-absent.
- A provenance graph answers backward ("what produced this finding?") and forward ("what did this scene affect?") queries deterministically.
- A reproducibility manifest records exactly the inputs and parameters needed to re-derive a product; a deterministic re-run yields a byte-identical output, verified by output hash.
- Evidence packs bundle an artifact's lineage and evidence for MRV (`19`), compliance (`24`), and copilot citation (`26`); a retention policy governs what is kept and an export emits an auditable slice.
- Provenance is threaded into the product domains rather than bolted on: `04`/`05`/`06`/`09`/`22`/`23`/`28` emit lineage as a first-class output, with tests on the math (hashing, chaining) and at least one tamper failure path.
