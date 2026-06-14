# Field, Farm, and Data Management: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: CRUD routes or commands with pagination, role checks, and audit events.
- Identity: stable IDs and org/field/season linkage on every entity.
- Tenant safety: every read/write scoped by organization and role.
- Geospatial: boundaries assert CRS and extent; GeoJSON round-trips.
- Audit: author and change history on annotations, recommendations, and work orders.
- Tests: unit (validation), fixture (GeoJSON boundary), API contract, and one failure path (cross-tenant access denied).
- Operations: runtime mode, migrations, and a runbook.

## Category Epics

### EPIC-01: Tenant-Safe Identity
- Goal: organizations and users with roles and enforced isolation.
- First release: Organization/User/membership model and admin/advisor/operator/viewer roles.
- Expansion: tenant isolation on every read/write path.
- Hardening: permission tests, cross-tenant denial paths, and audit of access.

### EPIC-02: Field Spine and Boundaries
- Goal: farms, fields, and boundaries as first-class owned records.
- First release: Farm/Field/FieldBoundary entities with create/list/link.
- Expansion: GeoJSON boundary import with CRS assertion and Season/CropPlan history.
- Hardening: boundary validation, shapefile/KML import, and field-over-time history.

### EPIC-03: Scene, Layer, and Work Order Traceability
- Goal: scenes, layers, findings, and tasks are owned, linked, and audited.
- First release: Scene/Layer registry owned by field and season, exposed to `07`/`08`.
- Expansion: annotation/recommendation/report persistence with author and change history.
- Hardening: WorkOrder lifecycle from recommendation to completion with full audit trail.
