# Geo Viewer and Visualization: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: viewer reads/writes through `07` routes with audit IDs and pagination.
- Geospatial: CRS, extent, and resolution asserted before any overlay is drawn.
- Deterministic: boundary placement and annotation geometry computed without AI.
- Data quality: scene freshness, sensor, and product availability surfaced in the UI.
- UI: field/scene picker, layer toggles, boundary overlay, annotation tools, and report panel.
- Tests: unit (geo transforms, tile URL), fixture (scene manifest), UI/overlay, and one failure path (missing scene/tile).
- Operations: `GEO_HUB_URL` config, render/fetch health, and a runbook.

## Category Epics

### EPIC-01: Trustworthy Layer Rendering
- Goal: render a scene product on the correct ground with provable georeferencing.
- First release: tile rendering with presence states plus an on-screen CRS/extent/resolution readout.
- Expansion: layer toggle and product switching (NDVI, thermal, source).
- Hardening: colormap legends, large-raster stability, and missing-tile handling.

### EPIC-02: Field Context and Boundary Overlay
- Goal: every layer is shown in the context of its field, owner, and boundary.
- First release: field/scene catalog selection from `10` and field boundary overlay.
- Expansion: season grouping and season-to-season compare mode.
- Hardening: saved views, swipe/side-by-side compare, and export snapshots.

### EPIC-03: Annotate to Recommend to Report
- Goal: an advisor turns what they see into a recommendation and a report without leaving the surface.
- First release: point/polygon annotation create/edit with severity, note, and audit write-back to `07`.
- Expansion: recommendation create-from-annotation and report-result overlays from `09`.
- Hardening: change history, shareable deliverable export, and negative-path tests.
