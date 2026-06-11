# Field, Farm, and Data Management: Current State and Target State

## Mission

Be the product spine: a canonical, tenant-safe domain model that gives every scene, layer, finding, and report a field, an owner, a season, and a traceable history, so the advisor workflow has context and accountability.

## Current Maturity

greenfield pending: per the PRD feature matrix, organizations/users, farms/fields/boundaries, season history, work orders, and tenant isolation are missing. Some field-facing record types and provenance storage exist in `shared`, `data_collector`, and `geo_hub`, but the control-plane domain model is not implemented.

## What Exists Now

- `shared` config and runtime: `AgroConfig`, `RuntimeMode`, `AgroError`/`AgroResult`, structured logging (`shared/src/config.rs`, `error.rs`, `lib.rs`).
- `shared` schemas: Telemetry, LidarScan, WebSocketMessage, ImageData, plus field-facing records FarmRecord, FieldRecord, FieldBoundary, AnnotationRecord, RecommendationRecord, ReportRecord, GeoBounds, GeoPoint (`shared/src/schemas.rs`).
- File-based session/record storage in `data_collector` (provenance angle).
- Shapefile/scene storage in `geo_hub`.

## Gaps to Close

- No Organization, User, or role model (admin, advisor, operator, viewer) and no tenant-safe data boundaries.
- No Farm/Field/FieldBoundary entities as first-class, persisted, owned records (only loose schema structs).
- No GeoJSON boundary import (PRD MVP requirement); only shapefile storage exists in `geo_hub`.
- No Season or CropPlan history linked to fields.
- No canonical Scene/Layer entities owned by a field and season with a product catalog.
- No WorkOrder entity or task lifecycle.
- No audit trail for annotations, recommendations, or work orders, despite NFR requirements.
- No tenant isolation enforced on any read/write path.

## Source Modules Reviewed

- `shared/src/lib.rs`, `config.rs`, `error.rs`, `schemas.rs` (FarmRecord, FieldRecord, FieldBoundary, AnnotationRecord, RecommendationRecord, ReportRecord, GeoBounds, GeoPoint)
- `data_collector` (file-based session/record storage)
- `geo_hub` (shapefile/scene storage)
- `../../reference/product-requirements.md` (domain model and feature matrix)

## Target Operating Model

- A new domain/control-plane crate owns the canonical model: Organization, User, Farm, Field, FieldBoundary, Season, CropPlan, Scene, Layer, Annotation, Recommendation, Report, WorkOrder.
- Tenant-safe isolation: every entity belongs to an organization, and every read/write is scoped by role and org.
- Boundaries import from GeoJSON (MVP) with CRS asserted; shapefile/KML later.
- Season and crop-plan history make field-over-time repeatability possible, not disconnected reports.
- Scenes and layers are owned by a field and season and expose their product catalog to `07`/`08`.
- A full audit trail records author and change history for annotations, recommendations, and work orders.
- `07`, `08`, and `09` resolve all field context, ownership, and traceability through this spine.
