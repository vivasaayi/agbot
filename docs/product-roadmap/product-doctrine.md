# Product Doctrine

## Positioning

AGBot is a field-to-decision agricultural drone platform for agronomists, crop consultants, drone service providers, and the small advisory teams who serve growers. It spans the full chain that competing tools fragment: plan and fly the mission, capture LiDAR and multispectral data, derive trustworthy remote-sensing products, render them on the correct field, and produce defensible findings, recommendations, and reports.

Pix4D, DroneDeploy, and Sentera are useful reference points for imagery processing depth, but AGBot's differentiator is the **closed loop**:

- Plan and simulate the mission before flying it.
- Fly it safely with guardrails and live telemetry.
- Capture LiDAR and multispectral data with known provenance.
- Derive deterministic products (indices, occupancy, thermal, statistics) that preserve geospatial truth.
- Explain what the field needs, where, and how severely.
- Turn that into a recommendation, work order, and grower-ready report.
- Re-fly and compare over the season.

## The Field-to-Decision Promise

Every field and scene the platform touches must be able to answer:

- Which field is this, who owns it, what crop and season, and where exactly is its boundary?
- What was captured, when, by which sensor, and is the data trustworthy and fresh?
- What do the deterministic products say (NDVI/index values, thermal, elevation, occupancy) with correct CRS and extent?
- Where are the anomalous zones, and how confident are we?
- What is the recommended next action and who owns it?
- Can a grower receive a clear report fast, without needing the full system?
- How does this scene compare to the last flight of the same field?

## The Seven Product Pillars

Every domain should report posture across these pillars (defined in `requirements-rigor.md`):

- **Safety** — flight, geofence, altitude, no-fly-zone, battery, and collision guardrails.
- **Geospatial correctness** — CRS, extent, resolution, georeferencing, and traceability.
- **Data quality** — sensor freshness, calibration, coverage, and QA masks.
- **Agronomic value** — findings and recommendations tied to a real field action.
- **Performance and scale** — large rasters, dense point clouds, and edge compute budgets.
- **Operability** — observability, deployment, configuration, and edge/ARM readiness.
- **Explainability and trust** — defensible outputs, evidence citation, uncertainty, and audit.

## Product Surfaces

- **Mission cockpit**: plan, simulate, and dispatch missions; live telemetry, map, and abort controls.
- **Digital twin / simulator**: 3D globe, terrain, and flight playback for planning, training, and regression.
- **Capture manager**: flight sessions, sensor streams, storage, and data provenance.
- **Remote-sensing workbench**: indices, thermal, masks, classification, and overlays with correct georeferencing.
- **Field GIS viewer**: field/farm catalog, layer toggles, boundary overlays, annotations, and compare mode.
- **Advisor workspace**: anomalies, findings, recommendations, work orders, and report export.
- **Grower deliverable**: a shareable, farmer-friendly report that does not require system access.
- **Fleet console**: drone enrollment, health, configuration, deployment, and edge operations.

## What Not To Build

- Do not ship a raster viewer without field context, boundaries, and a path to a recommendation.
- Do not let AI yield/health claims replace or precede deterministic, inspectable products.
- Do not display an overlay whose CRS, extent, or resolution cannot be proven correct.
- Do not execute a flight or multi-drone maneuver without geofence, altitude, battery, and abort guardrails.
- Do not assume cloud or a developer laptop; simulation and flight must run on edge hardware.
- Do not build farm ERP scope (accounting, payroll, inventory) in the MVP; stay on the advisor workflow.
</content>
