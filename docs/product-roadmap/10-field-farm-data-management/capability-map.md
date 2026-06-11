# Field, Farm, and Data Management: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (geospatial correctness, data quality, explainability, operability, agronomic value) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Field, Farm, and Data Management Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Organization and user model | missing | 7 | Create org, user, and assign membership |
| Roles and access control | missing | 6 | Enforce admin/advisor/operator/viewer roles |
| Tenant isolation | missing | 7 | Scope every read/write by organization |
| Farm and field entities | partial (schema only) | 8 | Persist farm/field as owned records |
| Field boundary management | partial (schema only) | 6 | Store and validate a field boundary |
| GeoJSON boundary import | missing | 6 | Import a field boundary from GeoJSON with CRS |
| Season and crop-plan history | missing | 7 | Link season and crop plan to a field |
| Scene and layer registry | partial (in `07`) | 8 | Own scene/layer records by field and season |
| Annotation persistence and audit | partial (schema only) | 6 | Persist annotations with author/change history |
| Recommendation persistence and audit | partial (schema only) | 6 | Persist recommendations with status lifecycle |
| Report and deliverable records | partial (schema only) | 5 | Persist report records and share links |
| Work order lifecycle | missing | 6 | Create a work order from a recommendation |

## Dependents

Domains `07` (GIS hub), `08` (viewer), and `09` (advisor) resolve field context, ownership, and traceability through this spine; it gates the whole advisor workflow.
