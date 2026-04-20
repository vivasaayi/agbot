# Milestone 1: Platform Foundation

## Goal
Build the domain model, GIS correctness, and platform interfaces required for the agriculture product to be trusted.

## Why this milestone matters
The product cannot become sellable until fields, scenes, and layers are modeled correctly and rendered on trustworthy geospatial coordinates.

## Scope
- shared domain contracts
- field and farm model
- scene-to-field linking
- geospatial metadata correctness
- viewer support for field overlays
- test scaffolding for platform workflow

## Workstreams
### 1. Domain model
Jobs:
- add contracts to `shared` for `Organization`, `User`, `Farm`, `Field`, `FieldBoundary`, `Season`, `SceneRef`, `LayerRef`, `AnnotationRef`, and `RecommendationRef`
- define identifiers, ownership rules, and timestamps
- document required fields and allowed nullability

Acceptance:
- shared contracts compile cleanly and serialize deterministically
- model is sufficient to express farm, field, scene, and boundary relationships

### 2. Persistence and API foundation
Jobs:
- extend `geo_hub` data model to store farms, fields, and field boundaries
- link scenes to fields
- expose CRUD APIs for farms and fields
- expose `GET /api/fields/:field_id` and `GET /api/fields/:field_id/scenes`

Acceptance:
- field records can be created and retrieved through API
- scenes can be associated with fields and returned through field-scoped endpoints

### 3. Geospatial correctness
Jobs:
- finalize raster metadata contract for CRS, extent, transform, and dimensions
- persist geospatial source metadata into scene metadata
- return field and scene extents through `geo_hub`
- document assumptions where source data lacks spatial reference

Acceptance:
- scene manifest includes trustworthy geospatial metadata when available
- field overlays and raster layers share a valid coordinate relationship

### 4. Viewer field overlays
Jobs:
- add field boundary overlay rendering in `geo_viewer`
- display field name, crop, season, and scene association
- support selection of a field and retrieval of related scenes

Acceptance:
- a field boundary can be rendered over a loaded scene and viewed in the UI

### 5. Testing and acceptance harness
Jobs:
- add unit tests for shared domain serialization
- add integration tests for field CRUD and field-scene association
- define one end-to-end acceptance test scenario:
  - create field
  - ingest scene
  - retrieve field and scene manifest
  - render field boundary with layer

Acceptance:
- tests pass in CI-focused commands
- acceptance scenario is documented and reproducible

## Deliverables
- shared domain types for farms and fields
- `geo_hub` APIs for farms and fields
- scene metadata with CRS and extent support
- `geo_viewer` boundary overlay support
- milestone acceptance test runbook

## Dependencies
- existing scene manifest and product-serving path in `geo_hub`
- current geospatial metadata contract work already added in `shared` and `geo_hub`

## Risks
- incomplete source georeferencing from ingest pipeline
- boundary coordinate format inconsistency
- Bevy-side complexity in overlay rendering

## Exit criteria
- product can model fields, not only raw scenes
- viewer can display a field boundary with a scene
- tests cover farm, field, and scene relationships
