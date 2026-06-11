# Plugin / Extension SDK and Open Data: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (operability and explainability/trust first, then a security dimension for sandboxing, plus data quality and geospatial correctness for the open-data layers) and the workstreams in `release-plan.md`. Because this is a greenfield domain (M0 named), every capability's current source status is "missing (greenfield)", and the Primary First Slice describes the M1 foundation step. The capability/permission boundary is a real security concern: a plugin must never exceed what its manifest declares. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Plugin / Extension SDK and Open Data Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Extension-point taxonomy | missing (greenfield) | 7 | Define the six extension-point kinds and their contracts |
| Plugin manifest and registration | missing (greenfield) | 8 | Validate a manifest and register a plugin |
| Capability / permission model | missing (greenfield) | 8 | Declare capabilities; deny anything undeclared |
| Sandboxed execution | missing (greenfield) | 9 | Run a plugin restricted to its declared capabilities |
| Versioning and compatibility contract | missing (greenfield) | 7 | Gate a plugin against the host API version |
| Custom spectral index extension point (`05`) | missing (greenfield) | 7 | Register a custom index that runs in `05` |
| Custom processor / report template (`09`) | missing (greenfield) | 7 | Register a processor and a report template in `09` |
| Custom map layer extension point (`08`) | missing (greenfield) | 5 | Register a map layer rendered by `08` |
| Custom alert rule extension point (`29`) | missing (greenfield) | 5 | Register an alert rule evaluated by `29` |
| Custom import/export adapter (`32`) | missing (greenfield) | 5 | Register a format adapter used by `32` |
| SDK, scaffolding, docs, and examples | missing (greenfield) | 8 | Ship the SDK crate, scaffolder, and two examples |
| Open-data catalog and publishing | missing (greenfield) | 7 | Publish an anonymized layer with license/attribution |
| Plugin registry / marketplace | missing (greenfield) | 6 | Catalog and install plugins from a registry (later) |
