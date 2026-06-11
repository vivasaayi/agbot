# Provenance and Audit Ledger: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: lineage-record, audit-append, provenance-query, and evidence-pack-export routes or commands with pagination and audit IDs.
- Deterministic: hashing, hash-chaining, content addressing, manifest derivation, and chain verification computed without AI, with reason codes — this is the dominant pillar; the whole domain is deterministic and append-only.
- Geospatial: lineage and evidence preserve the product's CRS/extent references so a re-derived raster round-trips correctly.
- Explainability: every recorded action attributes an actor; every finding's lineage is traceable; tampering is detectable.
- Agronomic: evidence packs back a recommendation, an MRV claim, or a copilot citation that ties to a real field action.
- Tests: unit (hash/chain/manifest math), fixture (sample lineage graphs), API contract, and one failure path (broken chain / non-reproducible re-run).
- Operations: ledger health, retention enforcement, and a runbook.

## Category Epics

### EPIC-01: Lineage and Content-Addressed Evidence
- Goal: every artifact records its full lineage and its evidence is addressable and integrity-checkable.
- First release: a lineage model `{inputs[], method, parameters, operator, timestamp}` recorded for one `09` finding, plus a content-addressed evidence store keyed by digest.
- Expansion: thread lineage emission into `04`/`05`/`06`/`22`/`23`/`28` so products and findings carry it natively.
- Hardening: deduplication, large-evidence performance, and CRS/extent preservation through the evidence store.

### EPIC-02: Append-Only Audit Log and Tamper-Evidence
- Goal: an immutable, hash-chained record of who did what and when, with detectable tampering.
- First release: append-only audit entries linked by hash, with actor attribution on every mutating action.
- Expansion: chain verification that detects a broken, edited, or reordered chain and reports the breach point.
- Hardening: retention policy, audit-slice export, and negative-path tests (tampered chain fails verify).

### EPIC-03: Provenance Graph, Reproducibility, and Evidence Packs
- Goal: answer provenance queries, re-derive products, and export citable evidence.
- First release: backward and forward provenance queries over the lineage graph and a reproducibility manifest per product.
- Expansion: deterministic re-run with output-hash verification, and evidence packs for MRV (`19`), compliance (`24`), and copilot citation (`26`).
- Hardening: pack schema validation, retention/export round-trip tests, and a non-reproducible-re-run failure path.
