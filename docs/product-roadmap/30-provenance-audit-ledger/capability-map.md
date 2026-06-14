# Provenance and Audit Ledger: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (explainability and trust first, then data quality, operability, geospatial correctness, performance/scale) and the workstreams in `release-plan.md`. Because this is a cross-cutting greenfield domain (M0 named), every capability's current source status is "missing (greenfield)", and the Primary First Slice describes the M1 foundation step. Explainability and trust dominates: the whole domain exists to make outputs defensible, re-derivable, and tamper-evident. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Provenance and Audit Ledger Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Lineage model (inputs/method/params/operator/timestamp) | missing (greenfield) | 9 | Record full lineage for one `09` finding artifact |
| Content-addressed evidence store | missing (greenfield) | 8 | Hash an evidence object and address it by digest |
| Append-only hash-chained audit log | missing (greenfield) | 8 | Append who/what/when as a hash-linked entry |
| Tamper-evidence and chain verification | missing (greenfield) | 7 | Detect a broken hash chain on verify |
| Backward provenance query ("what produced this?") | missing (greenfield) | 7 | Trace a finding back to its source scene/products |
| Forward provenance query ("what did this affect?") | missing (greenfield) | 6 | Trace a scene forward to findings/reports/actions |
| Reproducibility manifest | missing (greenfield) | 7 | Record inputs+params needed to re-derive a product |
| Deterministic re-run and output-hash verification | missing (greenfield) | 6 | Re-run a product and assert identical output hash |
| Evidence packs (MRV/compliance/copilot citation) | missing (greenfield) | 7 | Bundle an artifact's lineage + evidence into a pack |
| Provenance threading into product domains | missing (greenfield) | 8 | Emit lineage from `04`/`05`/`06`/`09`/`22`/`23`/`28` |
| Retention policy and ledger export | missing (greenfield) | 5 | Apply a retention rule and export an audit slice |
| Operator/actor identity and action attribution | missing (greenfield) | 5 | Attribute every mutating action to a known actor |
