# Engineering Breakdown

## Purpose
Translate the product strategy into concrete build work by module.

## Priority legend
- P0: required to make the product credible
- P1: required to make the product sellable
- P2: required to make the product repeatable for teams
- P3: scale and expansion work

## Cross-cutting epics
### Epic A: Domain model and contracts
Milestone:
- Milestone 1

Priority:
- P0

Tasks:
- define `Organization`, `User`, `Farm`, `Field`, `FieldBoundary`, `Season`, `Scene`, `Layer`, `Annotation`, `Recommendation`, `Report`, `WorkOrder`
- define shared IDs and timestamps
- define status enums and validation rules
- add serde tests and backward-compatibility coverage

Done when:
- domain contracts support all MVP workflows without ad hoc types

### Epic B: Acceptance testing
Milestones:
- Milestone 1 through 4

Priority:
- P0 through P2

Tasks:
- define acceptance scenarios for each milestone
- keep fixture data for fields, scenes, and reports
- add workflow-level regression tests

Done when:
- each milestone has one user-facing end-to-end test path

## Module breakdown

### `shared`
#### Milestone 1
Priority:
- P0

Tasks:
- add farm and field domain contracts
- add boundary geometry contract
- add organization and role enums
- finalize raster spatial metadata contracts

#### Milestone 2
Priority:
- P1

Tasks:
- add annotation and recommendation contracts
- add report metadata contract
- add export payload contracts

#### Milestone 3
Priority:
- P2

Tasks:
- add work-order contract
- add audit-log event contract
- add field history summary models

#### Milestone 4
Priority:
- P3

Tasks:
- add prescription and management zone contracts
- add time-series summary models
- add anomaly candidate contracts

### `geo_hub`
#### Milestone 1
Priority:
- P0

Tasks:
- add farms and fields tables
- add field-boundary storage and APIs
- link scenes to fields
- expose field-scoped scene endpoints
- persist and return CRS, extent, transform metadata

#### Milestone 2
Priority:
- P1

Tasks:
- add annotation storage and CRUD APIs
- add recommendation storage and CRUD APIs
- add report metadata and artifact APIs

#### Milestone 3
Priority:
- P2

Tasks:
- add organization-aware authorization
- add work-order APIs
- add field history and season queries
- add report archive endpoints

#### Milestone 4
Priority:
- P3

Tasks:
- add time-series APIs
- add prescription endpoints
- add anomaly candidate APIs
- add cache and background processing hooks

### `geo_viewer`
#### Milestone 1
Priority:
- P0

Tasks:
- refactor into small plugins: `ui`, `network`, `render`, `overlays`
- render field boundaries
- support field and scene selection
- show field metadata and scene metadata together

#### Milestone 2
Priority:
- P1

Tasks:
- implement point and polygon annotations
- add recommendation creation workflow
- add report-preview or export initiation flow
- improve layer review UX

#### Milestone 3
Priority:
- P2

Tasks:
- add task and field history views
- add multi-user and role-aware UI states
- add client-facing report and archive navigation

#### Milestone 4
Priority:
- P3

Tasks:
- add time-series compare UI
- add management-zone and prescription views
- move toward tiled viewport for larger scenes
- improve performance and caching behavior

### `imagery_processor`
#### Milestone 1
Priority:
- P0

Tasks:
- maintain deterministic outputs and tests for NDVI and thermal
- support spatial metadata propagation in produced artifacts

#### Milestone 2
Priority:
- P1

Tasks:
- generate stable outputs consumed by advisor workflow
- add anomaly-support pre-processing hooks if lightweight

#### Milestone 3
Priority:
- P2

Tasks:
- support field-history-friendly output metadata
- improve pipeline repeatability and output versioning

#### Milestone 4
Priority:
- P3

Tasks:
- add anomaly candidate layers
- add time-series summary outputs
- support prescription-generation helpers where appropriate
- improve large-raster processing behavior

### `mission_planner`
#### Milestone 1
Priority:
- P1

Tasks:
- evaluate current mission domain against future work-order model
- avoid coupling field work orders directly to drone mission-only abstractions

#### Milestone 2
Priority:
- P1

Tasks:
- start recommendation-to-task model alignment
- define whether recommendation workflow stays here or moves to a new operations module

#### Milestone 3
Priority:
- P2

Tasks:
- implement work orders and assignments if retained in this crate
- expose status transitions and history

#### Milestone 4
Priority:
- P3

Tasks:
- integrate prescriptions or execution handoff if product direction requires it

## Suggested execution order
1. `shared` domain contracts
2. `geo_hub` farm, field, and boundary APIs
3. `geo_viewer` field overlay and field-scene workflow
4. `geo_hub` annotation and recommendation APIs
5. `geo_viewer` annotation workflow
6. report export path
7. roles and work orders
8. time-series, prescriptions, and scale work

## First implementation batch
If you want the most productive immediate batch, do this first:
- shared farm and field contracts
- geo_hub field CRUD and scene linking
- geo_viewer field boundary overlay
- geo_hub field-scoped scene manifest endpoints
- acceptance test: field plus scene plus layer rendering

That batch sets up every later milestone.
