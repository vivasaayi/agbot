# Implementation Sequencing

This sequencing reconciles the domain roadmap with the earlier milestone plan in `../archive/milestones-roadmap.md`. The existing milestones (M1 Platform foundation, M2 Advisor MVP, M3 Collaboration and operations, M4 Precision ag and scale) remain the delivery spine; the phases below say which domains carry each milestone.

## Phase 0: Product Spine

Shared contracts that every domain depends on. Mostly lives in `shared` and a new domain crate.

- Define the core domain model: Organization, User, Farm, Field, FieldBoundary, Season, CropPlan, Scene, Layer, Annotation, Recommendation, Report, WorkOrder (domain `10`).
- Establish geospatial primitives: CRS, extent, resolution, transform, and georeferenced product metadata (domains `07`, `10`).
- Establish capture provenance: flight session, sensor stream, and scene→field→season linkage (domains `04`, `07`).
- Pick and document the authoritative storage backend and serving API shape (see confirmation questions in `requirements-rigor.md`).
- Add the acceptance-test harness extended from today's `just gis-test` / `just gis-acceptance`.

## Phase 1: Advisor MVP Vertical (Milestones M1 → M2)

The first sellable workflow: scene → field → layer → annotation → recommendation → report.

- **Domain 10** (M1 foundation): farms, fields, boundaries, scenes, and layer catalog with tenant-safe data.
- **Domain 07** (M1→M3): scene ingest, raster metadata correctness, and a layer-serving API the viewer can trust.
- **Domain 05** (M3): make NDVI/thermal/mask pipelines real and georeferenced, not just CLI argument parsing.
- **Domain 08** (M4): render layers and boundaries on the correct field, with annotations and recommendation overlays.
- **Domain 09** (M3→M4): turn deterministic products into anomalies, findings, recommendations, and a real PDF/CSV/GeoJSON report.

Exit: a pilot user can create a field, ingest a scene, view a layer on it, annotate a zone, create a recommendation, and export/share a report.

## Phase 2: Capture and Autonomy Foundation (parallel to Phase 1)

Make the data that feeds the advisor workflow real and trustworthy.

- **Domain 01**: finish MAVLink command handling and live telemetry; harden mission CRUD/optimization.
- **Domain 04**: prove real LiDAR/camera capture paths and finish session recording, indexing, and exports.
- **Domain 06**: extend occupancy/heatmaps with filtering and 3D reconstruction outputs the viewer can consume.
- **Domain 02**: keep the simulator/digital twin as the regression and planning surface for 01–04.

## Phase 3: Collaboration and Operations (Milestone M3)

- **Domain 10**: organizations, roles, assignments, work orders, status tracking, and field/season history.
- **Domain 11**: a real operator ground station (web + CLU) with live mission and capture status.
- **Domain 12**: drone enrollment, configuration, health, deployment, and edge/ARM operations.

Exit: a small advisory team can manage multiple farms and maintain field history without losing traceability.

## Phase 4: Precision Ag Expansion and Scale (Milestone M4)

- **Domain 09 + 05**: time-series comparison, prescriptions, management zones, and anomaly detection.
- **Domain 03**: real swarm coordination, formation control, and collision avoidance for multi-drone coverage.
- **Domain 06 + 07**: large-raster and tile performance hardening, dense point-cloud scale.
- **Domain 12**: alerts, fleet health, and enterprise operations support.

Exit: the platform supports repeated, comparative analysis at scale and operational outputs beyond static reports.

## Phase 5: Adjacent Product Vision (Later Horizon — Greenfield)

Domains `13`–`21` are greenfield product-vision modules from `../reference/product-summary.md` with no code today. They are documented at M0 depth and should not start until the core platform (`01`–`12`) and the advisor MVP are real. Likely sequencing within this horizon, by dependency and revenue proximity:

- **First wave (closest to existing value):** `15` Weather Advisory and `13` Farmers Portal — both build directly on the advisor workflow and field/farm model (`09`, `10`) and turn existing outputs into grower-facing value.
- **Second wave (precision-ag analytics extensions):** `16` Water Management, `17` Drought Management, and `19` Carbon and Sustainability — these extend the imagery/LiDAR/analytics domains (`05`, `06`, `09`) into specialized agronomic products.
- **Third wave (new platforms and surfaces):** `14` Autonomous Tractor (a new ground-vehicle platform reusing flight mission/safety patterns), `21` Real-time Collaboration (new live-video and messaging infrastructure), `20` Content Management (largely decoupled), and `18` Supply Chain and Marketplace (furthest from the codebase, with external payment/compliance boundaries).

Each should earn its place with user validation before implementation; none should pull engineering away from the advisor MVP.

## Cross-Domain Dependency Logic

- Phase 0 spine blocks everything: without the domain model and geospatial primitives, no overlay or report is trustworthy.
- Domain `10` (field/farm/data) and `07` (GIS hub) gate the entire advisor workflow.
- Domain `05` (imagery) and `06` (LiDAR) depend on `04` (capture) for real inputs, but can develop against fixtures and the `02` simulator in parallel.
- Domain `08` (viewer) depends on `07` serving correct, georeferenced layers.
- Domain `09` (advisor) is the first revenue milestone and should consume products from `05`/`06` and context from `10`.
- Domain `03` (swarm) should start only after single-drone flight (`01`) and safety guardrails are reliable.
- Domain `12` (fleet/edge) should harden once at least one full capture→report workflow runs on target hardware.

## Resourcing Logic

For a single primary engineer, treat Phase 0 and Phase 1 (the advisor vertical) as the near-term priority; gate Phases 3–4 behind pilot validation. If a second engineer joins, split backend contracts/processing (domains 05, 07, 09, 10) from flight/simulation/viewer UX (domains 01, 02, 08, 11).

## Delivery Philosophy

- Each milestone must end in a user-visible workflow, not only infrastructure.
- Acceptance tests are added at each milestone boundary.
- Geospatial correctness and safety guardrails are part of done, not follow-ups.
- A domain is not complete if the workflow only works for developers.
</content>
