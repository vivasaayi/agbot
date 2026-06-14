# Geo Viewer and Visualization

Render georeferenced layers and field boundaries on the correct field, annotate problem zones, and surface recommendations and reports inside one advisor surface.

## Where We Are

- `geo_viewer` is a Bevy app with a clean plugin architecture: network I/O, tile map, annotations, recommendations, reports, and UI.
- State exists for field catalog, scene manifest, tile rendering, map view control (zoom/pan/cursor), and annotation/recommendation/report overlays.
- The rendering and end-to-end workflow logic behind those plugins is thin; layer toggle, boundary overlay, and compare mode are not yet real.

## Where We Should Be

- The field GIS viewer surface of the advisor workflow: pick a field, load its scene, render the right layer on the right ground.
- Field boundary overlay, layer toggles, CRS/extent/resolution readout, and season-to-season compare mode.
- An end-to-end annotate -> recommend -> report UX that writes back through `07` and reads context from `10`.

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.

## Build Order

1. Render a layer for a selected scene with a CRS/extent/resolution readout.
2. Overlay the field boundary on the correct ground, asserting georeferencing.
3. Add layer toggle and a second product (NDVI/thermal) on the same field.
4. Implement point/polygon annotation create/edit with audit write-back to `07`.
5. Build recommendation create-from-annotation and report-result overlays.
6. Add season-to-season compare mode and saved views.

## Primary Crates

`geo_viewer`, with `sensor_overlay_engine` for colormaps and `shared` for schemas. Consumes layers from domain `07`, products from `05`/`06`, and field context from `10`.
