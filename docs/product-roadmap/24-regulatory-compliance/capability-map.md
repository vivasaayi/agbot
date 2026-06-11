# Regulatory and Compliance: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (safety first, then explainability and trust, geospatial correctness, data quality, operability) and the workstreams in `release-plan.md`. Because this is a greenfield domain (M0 named), every capability's current source status is "missing (greenfield)", and the Primary First Slice describes the M1 foundation step. The safety, explainability, and geospatial pillars dominate: a flight or application that violates a deterministic rule must be blocked, and every record must be defensible and append-only. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Regulatory and Compliance Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Airspace / no-fly-zone database | missing (greenfield) | 8 | Store airspace zones with CRS/extent (consumes `07`) |
| Pre-flight authorization checks | missing (greenfield) | 9 | Deterministic block on airspace/cert violation with reason codes |
| Remote ID and regulatory flight logging | missing (greenfield) | 7 | Append-only flight log authorities can be handed (`30`) |
| Operator certification / license registry | missing (greenfield) | 7 | Register a cert with expiry; block flight on expired/missing |
| Chemical / pesticide application records | missing (greenfield) | 9 | Record what/where/when/rate/operator per application |
| REI / pre-harvest interval (PHI) tracking | missing (greenfield) | 7 | Compute REI/PHI windows from an application record |
| Spray drift and buffer-zone compliance | missing (greenfield) | 8 | Assert buffers around sensitive areas/water (geospatial) |
| Data residency and retention policy controls | missing (greenfield) | 6 | Enforce retention/residency on compliance records |
| Audit-ready compliance reports and exports | missing (greenfield) | 8 | Emit a defensible report/export tied to `30` provenance |
| Compliance deadline and expiry alerting (via `29`) | missing (greenfield) | 5 | Raise an expiry/deadline alert through `29` |
| Compliance record identity and linkage | missing (greenfield) | 6 | Stable IDs linked to org/field/flight, append-only |
