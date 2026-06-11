# Product Requirements Document

## Product name
Field Intelligence Suite

## Positioning
A field scouting and agronomy platform that turns drone and imagery data into maps, findings, recommendations, and farmer-ready reports.

## Product thesis
Agriculture users do not buy raw imagery. They buy faster decisions, better field visibility, and cleaner operational follow-through.

The platform should answer:
- What field needs attention?
- Where is the issue?
- How severe is it?
- What is the next action?
- Can I send a clear report to the grower fast?

## Target user
Primary users:
- agronomists
- independent crop consultants
- drone service providers
- small advisory teams and co-ops

Secondary users:
- growers receiving recommendations and reports
- operations managers reviewing tasks and field history

## Initial market
- United States
- row crops first
- corn and soybean first
- later expansion into cotton and specialty crops

## Problem statement
Current workflows are fragmented across image processors, GIS tools, spreadsheets, PDFs, messaging apps, and internal notes. That causes delay and inconsistency between data capture and agronomic action.

Users need a single workflow to:
- ingest scenes
- inspect layers on the correct field
- identify and annotate problem zones
- create recommendations or work orders
- share reports and deliverables to clients

## Product goal
Reduce time from `scene captured` to `report delivered` and `recommended action created`.

## Product principles
- Trust first: geospatial correctness and traceability are mandatory.
- Workflow over features: solve one advisor workflow end to end before broadening scope.
- Explainability over black-box AI: outputs must be defensible.
- Field relevance over GIS novelty: every major feature should tie to a real farm action.
- Production discipline: tests, observability, and auditability are part of the product.

## Core use case
Scout crop stress and generate an action report for a field.

Target outcome:
1. import or create field boundary
2. ingest a scene
3. render raster layers on the field
4. identify anomalous zones
5. annotate issue areas
6. create a recommendation
7. export and share a report

## User jobs to be done
### Agronomist
- I want to inspect a field from recent imagery and quickly deliver findings.
- I want to create recommendations tied to exact problem areas.
- I want field history and repeatability, not disconnected reports.

### Drone service provider
- I want to process and review scenes for multiple clients.
- I want a clean handoff from data collection to report delivery.
- I want deliverables that look professional and are easy to share.

### Grower
- I want a simple summary of what changed, where the issue is, and what I should do next.

## In-scope MVP workflow
- organizations and users
- farms and fields
- field boundaries
- scene ingest
- raster layer catalog
- NDVI and thermal layer viewing
- annotations
- recommendations
- PDF export
- basic sharing

## Out-of-scope MVP
- accounting
- payroll
- inventory management
- complete farm ERP
- broad equipment telematics integration
- advanced autonomous drone orchestration
- high-claim diagnostic AI

## Functional requirements
### 1. Organization and access
- Create organization
- Create users
- Assign user to organization
- Basic roles: admin, advisor, operator, viewer
- Tenant-safe data boundaries

### 2. Farm and field management
- Create farm
- Create field
- Store crop, season, planting date, notes
- Import field boundary from GeoJSON
- Support manual boundary creation later

### 3. Scene and layer management
- Ingest scene metadata and products
- Associate scene to field and season
- Track source, acquisition time, sensor, processing status
- Expose available products per scene

### 4. GIS and map interaction
- Display raster layers
- Overlay field boundaries
- Toggle layers
- Zoom and pan
- Show CRS, extent, resolution, dimensions
- Preserve geospatial correctness end to end

### 5. Analytics
- Support NDVI
- Support thermal outputs
- Support basic anomaly flagging
- Support future spectral and classification layers

### 6. Annotation workflow
- Create point annotation
- Create polygon annotation
- Add note, category, severity, and timestamp
- Save author and change history

### 7. Recommendation workflow
- Create recommendation from one or more annotations
- Assign action category
- Assign priority and status
- Track open, reviewed, completed, dismissed

### 8. Reporting and deliverables
- Generate PDF report
- Export findings as CSV and GeoJSON
- Include field metadata, map views, findings, recommendations, and layer source details

### 9. Sharing
- Share report link
- Role-based visibility inside an organization
- Support farmer-friendly report output without requiring full system access

## Non-functional requirements
- Correct CRS and raster extent handling
- Large raster stability
- Persistent metadata and reproducible outputs
- Role-safe access and organization isolation
- Audit trail for annotations and recommendations
- Structured logs and observability
- Deterministic test coverage for critical math and API flows
- Acceptance tests for key user workflow

## Domain model
Required domain entities:
- Organization
- User
- Farm
- Field
- FieldBoundary
- Season
- CropPlan
- Scene
- Layer
- Annotation
- Recommendation
- Report
- WorkOrder

## Module mapping to repo
- `shared`
  - domain contracts, schema stability, common types
- `geo_hub`
  - scenes, layers, metadata, field-scene relationships, serving API
- `geo_viewer`
  - advisor/operator UI, layer controls, annotation and review workflow
- `imagery_processor`
  - NDVI, thermal, masks, anomaly-support outputs
- `mission_planner`
  - missions, tasks, recommendations, work-order evolution

## Feature matrix
| Area | MVP | V2 | V3 | Current repo status |
|---|---|---|---|---|
| Organizations and users | Basic org and user model | team roles | billing and SSO | missing |
| Farms and fields | farm, field, boundary | season history | portfolio analytics | missing |
| Boundary import | GeoJSON | shapefile | KML and sync adapters | missing |
| Scene ingest | manual ingest | bulk ingest | automated pipelines | partial |
| Raster metadata | CRS, extent, dimensions | transform inspection | reprojection workflows | partial |
| Layer catalog | NDVI, thermal, source layers | time series layers | prescriptions and zones | partial |
| GIS viewer | pan, zoom, layer toggles | compare mode | full analysis tools | partial |
| Vector overlays | field boundaries | annotations | prescriptions | missing |
| Analytics | NDVI and thermal | anomaly detection | predictive analytics | partial |
| Recommendations | manual recommendation records | templates | approvals | missing |
| Reporting | PDF export | branded reporting | scheduled reports | missing |
| Sharing | link sharing | role-based client access | client portal | missing |
| Work orders | basic tasks | field execution tracking | integrated operations | missing |
| Mobile and offline | none | read-only mobile | offline scouting | missing |
| Observability | logs | metrics and tracing | SLOs and alerting | partial |
| Test coverage | unit and integration tests | acceptance tests | release quality gates | partial |

## Commercial model
The core product should not be priced at `$3/month`.

Recommended pricing posture:
- Free or trial
  - very limited fields and reports
- Starter
  - `$19-$29/month`
  - solo agronomist or early grower
- Professional
  - `$79-$149/month`
  - consultants and drone operators
- Business
  - quote or usage based
  - multi-user organizations, higher data volume, white-label or API use

A `$3/month` product can later exist only as a limited farmer-viewer tier.

## Success metrics
- time from ingest to report export
- number of reports created per active user per week
- fields managed per organization
- recommendation creation rate
- pilot-to-paid conversion
- 60-day retention
- repeat weekly usage by advisors

## Release definition for MVP
The first sellable release is complete when a pilot user can:
- create a field
- ingest a scene
- view a layer over that field
- annotate a problem zone
- create a recommendation
- export and share a report
