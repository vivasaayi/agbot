# LiDAR Mapping and 3D Reconstruction: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: command, input/output contract, thresholds, and product provenance.
- Geospatial: occupancy/elevation grids assert CRS, extent, and resolution and round-trip.
- Deterministic: occupancy, segmentation, and elevation math that runs without AI.
- Data quality: outlier removal and coverage/freshness tracking on ingest.
- UI: elevation/obstacle overlays rendered by domain `08`.
- Tests: unit (occupancy/segmentation/elevation math), fixture (captured `04` scans), and one failure path (sparse/empty scan).
- Operations: runtime mode, processing health, and a runbook.

## Category Epics

### EPIC-01: Scan Ingest and Occupancy Products
- Goal: LiDAR scans become a trustworthy occupancy grid and point cloud.
- First release: parallel ingest, configurable-resolution occupancy grid, and PCD export with asserted extent.
- Expansion: obstacle heatmap thresholds and freshness/coverage tracking.
- Hardening: georeferencing assertions and occupancy-logic test depth.

### EPIC-02: Point-Cloud Cleaning and Geometry
- Goal: raw clouds become clean, oriented geometry before any product is derived.
- First release: statistical outlier/noise removal.
- Expansion: normal estimation and ground/non-ground segmentation.
- Hardening: clustering/segmentation of obstacle and canopy objects.

### EPIC-03: Elevation and 3D Reconstruction
- Goal: cleaned clouds become georeferenced elevation and terrain products.
- First release: a DSM rasterized from the cleaned cloud with asserted CRS/extent.
- Expansion: DTM, canopy-height model (DSM − DTM), and meshing.
- Hardening: canopy-height handoff to the advisor (`09`) and terrain rendering in the viewer (`08`).
