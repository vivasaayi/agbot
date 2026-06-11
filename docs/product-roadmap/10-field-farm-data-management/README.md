# Field, Farm, and Data Management

The product spine: organizations, users, farms, fields, boundaries, seasons, crop plans, scenes, layers, annotations, recommendations, reports, and work orders, with tenant-safe access and an audit trail.

## Where We Are

- `shared` provides `AgroConfig`, `RuntimeMode`, `AgroError`, logging, and schemas for Telemetry, LidarScan, ImageData, and partial field-facing records (FarmRecord, FieldRecord, FieldBoundary, AnnotationRecord, RecommendationRecord, ReportRecord).
- `data_collector` has file-based session/record storage (a provenance angle); `geo_hub` has shapefile/scene storage.
- The core PRD domain model and tenant-safe multi-org access are not implemented: no Organization, User, role model, Season, CropPlan, or WorkOrder, and no tenant isolation or audit trail.

## Where We Should Be

- One canonical domain model (Organization, User, Farm, Field, FieldBoundary, Season, CropPlan, Scene, Layer, Annotation, Recommendation, Report, WorkOrder) that `07`/`08`/`09` depend on for field context, ownership, and traceability.
- Tenant-safe organization isolation with roles (admin, advisor, operator, viewer).
- GeoJSON boundary import, season/crop-plan history, and a full audit trail for annotations and recommendations.

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.

## Build Order

1. Define Organization/User/role and tenant isolation in a new domain crate.
2. Add Farm/Field/FieldBoundary entities with create/list/link.
3. Import field boundaries from GeoJSON, asserting CRS.
4. Add Season and CropPlan history linked to fields.
5. Link Scene/Layer to field and season; expose products per scene.
6. Add the audit trail for annotations, recommendations, and work orders.

## Primary Crates

`shared` (domain contracts) plus a new domain/control-plane crate. Underpins `07` (scene/layer serving), `08` (viewer context), and `09` (recommendation/report persistence). Maps to milestone M1 (platform foundation) and M3 (collaboration).
