# Provenance and Audit Ledger: Detailed Stories

> Cross-cutting greenfield domain (M0 named): no code exists yet. This is the trust backbone, so every story is **deterministic and append-only by construction** — the explainability-and-trust pillar dominates every phase. Nothing here uses AI; hashing, chaining, content addressing, and manifest derivation must be inspectable and reproducible. The first slices land inside `09` and then thread outward into `04`/`05`/`06`/`22`/`23`/`28`; the first external consumers are copilot citations (`26`), carbon MRV (`19`), and compliance (`24`).

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M1 — Foundation

### STORY 30-01 · M1 · M · P0 — Lineage model and artifact identity
- **Story**: As `DSP`, I want every artifact to record its inputs, method, parameters, operator, and timestamp under a stable ID, so that any output can be traced to what produced it.
- **Deterministic / evidence**: persist `{artifact_id, kind, inputs[], method, parameters, operator, created_at}`; `inputs[]` reference upstream artifact IDs; record lineage for one `09` finding first.
- **Acceptance**:
  - Given a `09` finding computed from a product, when it is recorded, then a lineage row exists with the product ID in `inputs[]`, the method, parameters, operator, and timestamp.
  - Given a recorded lineage, when it is fetched by artifact ID, then all inputs and parameters round-trip unchanged.
  - Given a lineage record referencing an unknown input artifact ID, when it is written, then it is rejected with a reason code (no dangling lineage).
- **Tests**: unit (lineage round-trip), API contract (record/fetch), failure path (unknown input ID rejected).
- **Depends on**: `09` (finding artifact), `shared` schemas.

### STORY 30-02 · M1 · S · P1 — Actor identity and action attribution
- **Story**: As `PA`, I want every mutating action attributed to a known actor, so that the ledger always says who did it.
- **Deterministic / evidence**: resolve the actor from the request context; persist `{actor_id, actor_kind}` on every lineage and audit entry; reject actions with no resolvable actor.
- **Acceptance**:
  - Given an authenticated actor, when a mutating action occurs, then the action records that actor's ID and kind.
  - Given a request with no resolvable actor, when a mutating action is attempted, then it is refused and the refusal is itself audited.
- **Tests**: unit (actor resolution), API contract, failure path (unattributed action refused).
- **Depends on**: 30-01.

### STORY 30-03 · M1 · S · P1 — Ledger record listing and retrieval
- **Story**: As `AG`, I want to list and re-open lineage and audit records for an artifact, so that I can inspect a result's history.
- **Acceptance**: records are paginated and filterable by artifact, actor, and date; a record is retrievable by ID after restart.
- **Tests**: API contract (pagination + filters), fixture (seeded ledger).
- **Depends on**: 30-01.

---

## M3 — Explainable (the deterministic, tamper-evident core)

### STORY 30-04 · M3 · M · P0 — Content-addressed evidence store
- **Story**: As `DSP`, I want evidence objects stored by cryptographic digest, so that integrity is checkable and identical inputs deduplicate.
- **Deterministic / evidence**: hash an evidence object (raster ref + mask + counts, or finding evidence) with a fixed algorithm; address it by digest; store once per digest.
- **Acceptance**:
  - Given an evidence object, when it is stored, then it is addressed by its digest and retrievable by that digest.
  - Given two identical evidence objects, when both are stored, then they deduplicate to one entry with one digest.
  - Given a retrieved object whose bytes were altered, when its digest is recomputed, then it fails the integrity check with a reason code.
- **Tests**: unit (hashing + dedup), fixture (sample evidence), failure path (altered bytes fail integrity).
- **Depends on**: 30-01.

### STORY 30-05 · M3 · M · P0 — Append-only hash-chained audit log
- **Story**: As `PA`, I want every mutating action appended as a hash-linked entry, so that the action history is immutable and ordered.
- **Deterministic / evidence**: each entry stores `{seq, prev_hash, payload_hash, entry_hash, actor, ts}`; `entry_hash = H(prev_hash || payload_hash || …)`; the API exposes append and read but no update or delete.
- **Acceptance**:
  - Given a sequence of actions, when they are appended, then each entry links to the prior entry's hash and the chain is contiguous.
  - Given an attempt to update or delete an existing entry, when it is made, then it is refused (append-only).
- **Tests**: unit (chain linkage), API contract (append/read; no update/delete), failure path (update/delete refused).
- **Depends on**: 30-01, 30-02.

### STORY 30-06 · M3 · S · P0 — Tamper-evidence and chain verification
- **Story**: As `PA`, I want to verify the audit chain and detect any tampering, so that I can prove the log was not altered.
- **Deterministic / evidence**: walk the chain recomputing each `entry_hash`; report the first index where the recomputed hash diverges; verification is a pure function of the stored entries.
- **Acceptance**:
  - Given an intact chain, when it is verified, then verification passes and reports the verified length.
  - Given a chain with one edited or reordered entry, when it is verified, then verification fails and reports the breach index.
- **Tests**: unit (verify intact + detect edit + detect reorder), failure path (edited entry → breach index).
- **Depends on**: 30-05.

### STORY 30-07 · M3 · M · P0 — Backward provenance query ("what produced this?")
- **Story**: As `AG`, I want to trace a finding back to its source products and scene, so that I can defend where it came from.
- **Deterministic / evidence**: traverse `inputs[]` edges transitively from a finding to its root capture; return the lineage subgraph with methods and parameters at each node.
- **Acceptance**:
  - Given a finding, when traced backward, then the result includes its products, their source scene, and the capture session, each with method and parameters.
  - Given a finding whose lineage chain is incomplete (a missing input node), when traced, then the gap is reported explicitly (no fabricated path).
- **Tests**: unit (graph traversal), fixture (multi-level lineage), failure path (missing node reported).
- **Depends on**: 30-01, 30-04.

### STORY 30-08 · M3 · S · P1 — Forward provenance query ("what did this affect?")
- **Story**: As `OPS`, I want to trace a scene forward to every finding, report, and action derived from it, so that I can assess the impact of re-processing or retracting it.
- **Deterministic / evidence**: traverse `inputs[]` edges in reverse from a scene to all downstream artifacts; return the affected-artifact set.
- **Acceptance**: a scene traces forward to all dependent findings/reports/actions; a scene with no downstream artifacts returns an empty set, not an error.
- **Tests**: unit (reverse traversal), fixture (fan-out graph), failure path (no-downstream → empty set).
- **Depends on**: 30-07.

### STORY 30-09 · M3 · M · P1 — Provenance threading into product domains
- **Story**: As `DSP`, I want `04`/`05`/`06`/`22`/`23`/`28` to emit lineage natively, so that provenance is captured at the source, not reconstructed.
- **Deterministic / evidence**: each product/finding emits `{inputs[], method, parameters}` into the ledger as part of its normal output; CRS/extent references are preserved in the lineage.
- **Acceptance**:
  - Given a product computed in `05`/`06`/`22`, when it completes, then it has emitted a lineage record referencing its source scene and parameters.
  - Given a product whose source scene reference is missing, when it completes, then lineage emission fails and the product is flagged (no untraceable product silently accepted).
- **Tests**: integration (per-domain emission), geospatial (CRS/extent preserved in lineage), failure path (missing source ref flagged).
- **Depends on**: 30-01, 30-04, `04`/`05`/`06`/`22`/`23`/`28`.

---

## M4 — Interactive (re-derivation and citable evidence)

### STORY 30-10 · M4 · M · P0 — Reproducibility manifest
- **Story**: As `DSP`, I want each product to record a manifest of exactly the inputs and parameters needed to re-derive it, so that the result is reproducible by anyone.
- **Deterministic / evidence**: persist `{product_id, input_digests[], method, method_version, parameters}`; the manifest references content-addressed inputs so they can be re-fetched exactly.
- **Acceptance**:
  - Given a completed product, when its manifest is read, then it lists every input digest, the method version, and all parameters.
  - Given a manifest whose referenced input digest is no longer present, when it is validated, then validation fails with the missing digest (not a partial re-derive).
- **Tests**: unit (manifest assembly), API contract, failure path (missing input digest fails validation).
- **Depends on**: 30-04, 30-09.

### STORY 30-11 · M4 · M · P0 — Deterministic re-run and output-hash verification
- **Story**: As `AG`, I want to re-derive a product from its manifest and confirm it matches the original, so that I can prove the output was not tampered with or silently changed.
- **Deterministic / evidence**: re-run the recorded method+version over the recorded inputs; hash the output and compare to the stored output hash.
- **Acceptance**:
  - Given a manifest and present inputs, when the product is re-derived, then the re-run output hash equals the stored output hash.
  - Given a method-version mismatch or an altered input, when re-derived, then the output hash differs and the re-run is flagged as non-reproducible with the cause.
- **Tests**: determinism (same manifest → same output hash), unit (mismatch detection), failure path (altered input → non-reproducible flagged).
- **Depends on**: 30-10.

### STORY 30-12 · M4 · M · P0 — Evidence packs for MRV / compliance / copilot
- **Story**: As `AG`, I want to export an artifact's lineage and evidence as a self-contained pack, so that MRV (`19`), compliance (`24`), and the copilot (`26`) can cite it.
- **Deterministic / evidence**: bundle `{artifact lineage subgraph, evidence objects by digest, audit entries, manifest}` into a schema-validated pack; every copilot citation must resolve to an evidence object in the pack.
- **Acceptance**:
  - Given a finding, when an evidence pack is exported, then it contains the backward-lineage subgraph, the evidence objects, the audit trail, and the manifest, and validates against the pack schema.
  - Given a citation that references an evidence object missing from the pack, when the pack is built, then the build fails with the unresolved citation (no pack with dangling citations).
- **Tests**: schema validation, integration (copilot citation resolves), failure path (unresolved citation fails build).
- **Depends on**: 30-07, 30-10, `19`/`24`/`26`.

### STORY 30-13 · M4 · S · P1 — Retention policy and ledger export
- **Story**: As `PA`, I want a retention policy and an auditable export of a ledger slice, so that I can keep what is required and hand off an audit trail.
- **Deterministic / evidence**: apply a retention rule (by age/kind/field) that may archive but never silently drops chain continuity; export a verifiable audit slice with its chain proof.
- **Acceptance**:
  - Given a retention rule, when it runs, then governed records are archived and the audit chain remains verifiable across the retained range.
  - Given an export request, when it runs, then the exported slice carries a chain proof that verifies independently; a slice that would break chain continuity is refused.
- **Tests**: unit (retention rule), API contract (export + verify), failure path (continuity-breaking export refused).
- **Depends on**: 30-05, 30-06.

---

## Coverage note

These 13 stories cover all 12 capabilities in `capability-map.md` (provenance threading 30-09 spans the multi-domain emission capability). The breakdown carries a deliberately heavy M3 deterministic core — content addressing, hash-chaining, tamper-evidence, and provenance queries — reflecting that **trust leads every phase** in `release-plan.md`. The curated counts in `release-plan.md` (~83 rows) expand several of these (per-domain provenance threading variants for `04`/`05`/`06`/`22`/`23`/`28`, additional evidence-pack and retention slices) into sibling stories when implemented.
