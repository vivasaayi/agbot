# Milestones Roadmap

## Objective
Sequence platform delivery from technical foundation to a sellable agriculture product.

This roadmap assumes the product is built for agronomists, crop consultants, and drone service providers first.

## Milestone structure
- Milestone 1: Platform foundation
- Milestone 2: Advisor MVP
- Milestone 3: Collaboration and operations
- Milestone 4: Precision ag expansion and scale

## Milestone 1: Platform foundation
Goal:
- establish the domain model and geospatial correctness required for the product to be trusted

Primary outputs:
- organization, farm, field, boundary, scene, and layer domain contracts
- field-boundary aware APIs
- CRS, extent, and raster metadata correctness
- viewer boundary overlay support
- ingestion contract ready for real geospatial metadata
- baseline test coverage and acceptance harness

Exit criteria:
- a field can be created and linked to a scene
- a scene manifest returns trustworthy geospatial metadata
- field boundaries can be rendered in the viewer
- platform tests cover critical ingest and metadata paths

## Milestone 2: Advisor MVP
Goal:
- ship the first sellable workflow: ingest to report

Primary outputs:
- raster review workflow
- annotations
- recommendations
- report export
- basic sharing

Exit criteria:
- advisor can inspect a field, mark issues, create recommendation, and export report in one workflow
- output is usable by a grower or client without additional tools

## Milestone 3: Collaboration and operations
Goal:
- enable teams and recurring client work

Primary outputs:
- organizations, roles, assignments
- work orders and status tracking
- field history and season context
- client workflow and repeat project management

Exit criteria:
- small advisory team can manage multiple farms and maintain field history without losing traceability

## Milestone 4: Precision ag expansion and scale
Goal:
- broaden from reporting to operational agronomy and scalable delivery

Primary outputs:
- time-series comparison
- prescriptions and management zones
- anomaly detection support
- large-raster and tile performance hardening
- alerts and enterprise operations support

Exit criteria:
- platform supports repeated analysis at scale and can generate operational outputs beyond static reports

## Recommended timeline
This is an execution target, not a promise.

- Milestone 1: 4-6 weeks
- Milestone 2: 6-8 weeks
- Milestone 3: 6-8 weeks
- Milestone 4: 8-12 weeks

## Dependency logic
Milestone 1 blocks everything else because the product cannot be credible without:
- domain model
- field relationships
- geospatial correctness
- stable APIs

Milestone 2 is the first revenue milestone.

Milestone 3 should be started only after at least one complete advisor workflow exists.

Milestone 4 should start after pilot users confirm the core workflow is useful.

## Resourcing logic
If developed by one primary engineer:
- Milestone 1 and Milestone 2 should be treated as the near-term priority
- Milestone 3 and Milestone 4 should remain gated by user validation

If a second engineer is available:
- one can own backend contracts and processing
- one can own viewer UX and workflow systems

## Delivery philosophy
- each milestone must end in a user-visible workflow, not only infrastructure
- acceptance tests should be added at each milestone boundary
- docs and API contracts should be updated during the milestone, not after it
- no milestone is complete if the workflow only works for developers
