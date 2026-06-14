# Provenance and Audit Ledger: Release Plan

## Shipment Strategy

Ship in maturity order, but because this is a cross-cutting trust backbone the deterministic core lands early and is treated as P0. The lineage model and content-addressed evidence store come first (M1/M3), then the append-only hash-chained audit log and tamper-evidence (M3), then provenance-graph queries, reproducibility, and evidence packs (M4), then retention and broad threading into all product domains as a continuing M4/M5 effort. The explainability-and-trust pillar leads every phase: the domain only earns its place if outputs are defensible and tampering is detectable. The first consumers — copilot (`26`) citations, carbon MRV (`19`), and compliance (`24`) — pull the evidence-pack work forward.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 12 |
| M2 captured | 8 |
| M3 explainable | 26 |
| M4 interactive | 24 |
| M5 autonomous-assist | 13 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 30 |
| P1 | 33 |
| P2 | 20 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 14 |
| M | 41 |
| S | 28 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | Lineage model (inputs/method/params/operator/timestamp) | explainability | identity |
| M3 explainable | M | Content-addressed evidence store | explainability | evaluator |
| M3 explainable | M | Append-only hash-chained audit log | explainability | evaluator |
| M3 explainable | S | Tamper-evidence and chain verification | explainability | evaluator |
| M3 explainable | M | Backward provenance query | explainability | evaluator |
| M4 interactive | M | Reproducibility manifest | data quality | evaluator |
| M4 interactive | M | Deterministic re-run and output-hash verification | data quality | evaluator |
| M4 interactive | M | Evidence packs (MRV/compliance/copilot citation) | explainability | export |

## Execution Rules

- The audit log is append-only; no slice may expose an update or delete on a recorded action — mutation is modeled as a new entry, never an edit.
- Every mutating action across the threaded domains must attribute a known actor; an unattributed action is rejected and itself audited.
- Content addressing and hash-chaining are deterministic and tested; a broken chain or a digest mismatch must fail verification with a reason code and a breach point.
- A reproducibility manifest must capture exactly the inputs and parameters needed to re-derive a product; a deterministic re-run must yield an identical output hash or the slice fails.
- Evidence packs for `19`/`24`/`26` must resolve every citation to a stored evidence object; a citation that cannot be resolved is an error, not a silent omission.
- CRS and extent references must be preserved through the evidence store and any re-run so a re-derived raster round-trips correctly.
