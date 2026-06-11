# Geo Viewer and Visualization: Current State and Target State

## Mission

Be the field GIS viewer of the advisor workflow: render georeferenced layers and boundaries on the correct field, let an advisor annotate problem zones, and present recommendations and reports without leaving the surface.

## Current Maturity

early partial: `geo_viewer` has a well-structured Bevy plugin architecture (network, map, annotations, recommendations, reports, UI) and rich state types, but the rendering and workflow logic behind the plugins is thin and opaque; the end-to-end advisor UX is not yet real.

## What Exists Now

- Bevy app with `EguiPlugin` and six composed plugins (`geo_viewer/src/app.rs`).
- Tile map rendering with `TileRenderState`, `TileFetchTasks`, presence tracking, and zoom/pan/cursor map control (`geo_viewer/src/plugins/map.rs`, `state.rs`).
- Scene manifest tracking with geospatial metadata (CRS, center, extent, georeferenced flag) and per-scene product list (`SceneManifest`, `SceneGeospatialMetadata` in `state.rs`).
- Field catalog state with farms, fields, season groups, and scene summaries; field history fetch and shapefile import scaffolding (`FieldCatalogState`, `FieldImportState`).
- Annotation overlay state with point/polygon draft modes, severity filters, and create/fetch/update/delete tasks (`plugins/annotations.rs`).
- Recommendation and report overlay state with create/fetch/update/delete and report-generate tasks (`plugins/recommendations.rs`, `reports.rs`).
- Network plugin issuing async I/O against `geo_hub` via `GEO_HUB_URL` (`plugins/network.rs`).

## Gaps to Close

- Layer rendering is tile-blitting only; no layer toggle, no second-product switching, and no colormap legend on screen.
- Field boundary overlay is not rendered on the map; georeferencing is tracked but not asserted or drawn.
- No compare mode (season-to-season or product-to-product) despite season-group state existing.
- The annotate -> recommend -> report UX is wired as tasks but the interactive workflow and write-back audit are incomplete.
- No CRS/extent/resolution on-screen readout to prove geospatial correctness to the operator.
- No saved views, export, or shareable deliverable from the viewer.
- Depends on `07` serving correct georeferenced layers; thin/opaque rendering hides whether the overlay is provably right.
- No tests on the rendering or workflow paths.

## Source Modules Reviewed

- `geo_viewer/src/app.rs`, `state.rs`, `main.rs`
- `geo_viewer/src/plugins/map.rs`, `annotations.rs`, `recommendations.rs`, `reports.rs`, `network.rs`, `ui.rs`
- `shared/src/schemas.rs` (FieldRecord, AnnotationRecord, RecommendationRecord, ReportRecord, GeoPoint, GpsCoords)
- `sensor_overlay_engine` (colormaps, consumed for layer styling)

## Target Operating Model

- One viewer surface keyed to a field and scene from `10`/`07`, with the active layer always provably georeferenced.
- Boundary overlay drawn on the correct ground, with an on-screen CRS, extent, resolution, and dimensions readout.
- Layer toggle across products (NDVI, thermal, source) and a compare mode across seasons or products.
- Interactive point/polygon annotation with author, severity, note, and timestamp, audited back through `07`.
- Recommendation create-from-annotation and report-result overlays that close the loop into `09`.
- Saved views and shareable exports so an advisor can hand a grower a deliverable.
- Workflow-first: the surface must reach a recommendation, never a dead-end raster view.
