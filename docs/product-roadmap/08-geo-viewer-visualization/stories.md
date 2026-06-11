# Geo Viewer and Visualization: Detailed Stories

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI. No overlay may be drawn whose CRS/extent/resolution cannot be asserted against the `07` manifest; geospatial placement must be provable on screen.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

This domain is the **core advisor MVP surface**: the field GIS viewer that renders georeferenced layers and boundaries, lets an advisor annotate problem zones, and drives the annotate → recommend → report loop without leaving the screen. It consumes `05`/`06` products and `07` layers. Real crate: `geo_viewer` (Bevy `app.rs` with `EguiPlugin` + six plugins; `plugins/{map,annotations,recommendations,reports,network,ui}.rs`; `state.rs` with `TileRenderState`, `SceneManifest`, `SceneGeospatialMetadata`, `FieldCatalogState`, `FieldImportState`), network I/O against `geo_hub` via `GEO_HUB_URL`, `shared/src/schemas.rs` (`FieldRecord`, `AnnotationRecord`, `RecommendationRecord`, `ReportRecord`, `GeoPoint`, `GpsCoords`), `sensor_overlay_engine` colormaps.

---

## M1 — Foundation

### STORY 08-01 · M1 · S · P0 — Field/scene catalog and selection
- **Story**: As `AG`, I want to pick a field and scene from the `10`/`07` catalog, so that the viewer is always keyed to a known field with its owner and season.
- **Deterministic / evidence**: `FieldCatalogState` lists farms/fields/season-groups and per-scene summaries from `07`; selecting a field/scene records `{field_id, scene_id, season_id, owner}` in viewer state.
- **Acceptance**:
  - Given a populated catalog, when a field and scene are selected, then the viewer state holds field/scene/season/owner and shows the field context.
  - Given a scene whose field linkage is missing, when it is selected, then the viewer refuses with an "unlinked scene" message rather than showing a field-less raster.
- **Tests**: unit (selection state), integration (catalog fetch from `07`), failure path (unlinked scene refused).
- **Depends on**: `07` (catalog), `10` (field/owner/season).

### STORY 08-02 · M1 · S · P1 — Map navigation and cursor geolocation
- **Story**: As `AG`, I want zoom/pan with a live lon/lat cursor readout, so that I can navigate the field and read coordinates under the cursor.
- **Deterministic / evidence**: map control updates zoom/pan; the cursor screen position is reprojected to lon/lat via the active layer transform and shown live.
- **Acceptance**:
  - Given a placed layer, when the cursor moves, then the lon/lat readout updates and matches the layer transform at that pixel.
  - Given no active layer/transform, when the cursor moves, then the readout shows "no georeference" rather than a fabricated coordinate.
- **Tests**: unit (screen→lon/lat reprojection), failure path (no transform → no coordinate).
- **Depends on**: 08-01.

---

## M2 — Captured / Observable

### STORY 08-03 · M2 · M · P0 — Tile layer rendering with presence/missing states
- **Story**: As `AG`, I want a scene product rendered with explicit presence and missing states, so that I can tell a loading or absent layer from a real one.
- **Deterministic / evidence**: `TileRenderState`/`TileFetchTasks` fetch tiles from `07` with presence tracking; missing/failed tiles render a distinct state, not a blank that looks like data.
- **Acceptance**:
  - Given a scene with available tiles, when the layer renders, then present tiles draw and presence is tracked.
  - Given a scene with missing/failed tiles, when the layer renders, then those tiles show a distinct missing/error state rather than blank ground.
- **Tests**: unit (presence state machine), integration (tile fetch), failure path (failed tile → error state).
- **Depends on**: 08-01, `07` (layer serving).

### STORY 08-04 · M2 · S · P1 — Scene manifest and product list
- **Story**: As `AG`, I want the scene manifest with its geospatial metadata and product list loaded, so that I know which products (NDVI/thermal/source) exist for the scene.
- **Deterministic / evidence**: `SceneManifest`/`SceneGeospatialMetadata` (CRS, center, extent, georeferenced flag) and the per-scene product list are fetched and held in state.
- **Acceptance**:
  - Given a selected scene, when the manifest loads, then its CRS/center/extent/georeferenced flag and product list are available in state.
  - Given a manifest marked not-georeferenced, when it loads, then products are flagged unplaceable rather than offered for overlay.
- **Tests**: unit (manifest parse), failure path (not-georeferenced flag respected).
- **Depends on**: 08-01, `07`.

---

## M3 — Explainable (provable georeferencing — the trust foundation)

### STORY 08-05 · M3 · M · P0 — Georeferenced layer placement
- **Story**: As `OPS`, I want a layer placed on the correct ground only after its CRS/extent are asserted against the `07` manifest, so that no overlay is ever drawn in the wrong place.
- **Deterministic / evidence**: before drawing, assert the layer's CRS/extent/resolution match the `07` `SceneGeospatialMetadata`; place the layer by its transform so corner pixels map to the manifest corners within tolerance.
- **Acceptance**:
  - Given a layer whose CRS/extent match the manifest, when it is placed, then its corners align to the manifest extent within tolerance.
  - Given a layer whose CRS/extent disagree with the manifest, when placement is attempted, then it is refused with a mismatch error rather than drawn approximately.
- **Tests**: unit (corner alignment), geospatial (CRS/extent assertion vs manifest), failure path (mismatch refused).
- **Depends on**: 08-04, `07` (manifest), `05`/`06` (products).

### STORY 08-06 · M3 · S · P0 — CRS/extent/resolution readout
- **Story**: As `OPS`, I want CRS, extent, resolution, and dimensions shown on screen, so that I can prove the overlay's geospatial correctness to a grower.
- **Deterministic / evidence**: an on-screen readout shows the active layer's `{CRS, extent, resolution, width×height}` sourced from the manifest, updating on layer change.
- **Acceptance**:
  - Given a placed layer, when it is active, then the readout shows its CRS/extent/resolution/dimensions from the manifest.
  - Given a layer with incomplete metadata, when it is active, then the readout shows the missing fields explicitly rather than blanks that imply correctness.
- **Tests**: unit (readout values match manifest), failure path (incomplete metadata flagged).
- **Depends on**: 08-05.

### STORY 08-07 · M3 · M · P0 — Field boundary overlay
- **Story**: As `AG`, I want the field boundary drawn on the map, so that I can see the layer in the context of the actual field.
- **Deterministic / evidence**: fetch the `FieldRecord` boundary from `07`, reproject it into the active layer CRS, and draw it; assert the boundary's CRS is known before drawing.
- **Acceptance**:
  - Given a field with a boundary and a placed layer, when the boundary draws, then it is reprojected into the layer CRS and overlays the correct ground.
  - Given a boundary with an unknown CRS, when drawing is attempted, then it is refused rather than drawn assuming the layer CRS.
- **Tests**: unit (boundary reprojection), geospatial (boundary vs layer CRS), failure path (unknown boundary CRS refused).
- **Depends on**: 08-05, `07` (boundaries).

---

## M4 — Interactive (the annotate → recommend → report loop)

### STORY 08-08 · M4 · M · P0 — Layer toggle and product switching
- **Story**: As `AG`, I want to toggle NDVI/thermal/source on the same field, so that I can compare products without losing my place.
- **Deterministic / evidence**: switch the active product among the manifest's product list while preserving view; each switched layer is re-asserted against the manifest (08-05) before drawing; a colormap legend (from `sensor_overlay_engine`) is shown.
- **Acceptance**:
  - Given a scene with NDVI and thermal products, when the operator toggles, then the active layer switches in place with the correct legend, view preserved.
  - Given a product that fails its georeference assertion, when toggled to, then it is refused and the prior layer stays, rather than drawing the bad layer.
- **Tests**: unit (toggle state + legend), geospatial (re-assert on switch), failure path (bad product refused, prior retained).
- **Depends on**: 08-04, 08-05, `05`/`06` (products).

### STORY 08-09 · M4 · L · P0 — Point/polygon annotation workflow
- **Story**: As `AG`, I want to create and edit point and polygon annotations with severity and note, so that I can mark problem zones, audited back through `07`.
- **Deterministic / evidence**: `plugins/annotations.rs` point/polygon draft modes; on commit, geometry is captured in the layer CRS and written to `07` as an `AnnotationRecord` with `{author, severity, note, timestamp, audit_id}`.
- **Acceptance**:
  - Given a placed georeferenced layer, when a polygon annotation is committed, then it is written to `07` with author/severity/note/timestamp and an audit ID, in the correct CRS.
  - Given the write-back to `07` fails, when committing, then the annotation is held locally with an error state and is not silently lost or shown as saved.
- **Tests**: unit (draft→record, CRS capture), integration (write-back to `07`), failure path (write-back failure → local error state).
- **Depends on**: 08-05, 08-07, `07` (annotation records).

### STORY 08-10 · M4 · M · P0 — Recommendation create-from-annotation
- **Story**: As `AG`, I want to build a recommendation from selected annotations, so that a marked zone becomes an actionable next step in the report.
- **Deterministic / evidence**: `plugins/recommendations.rs` creates a `RecommendationRecord` referencing one or more annotation IDs with `{priority, action_category, status, evidence_refs[]}`, persisted via `07`/`09`.
- **Acceptance**:
  - Given selected annotations, when a recommendation is created, then it references those annotations and is stored with priority, category, and status.
  - Given a recommendation created from zero annotations, when committed, then it is rejected rather than producing a recommendation with no evidence.
- **Tests**: unit (recommendation assembly), integration (persist + annotation refs), failure path (no-annotation recommendation rejected).
- **Depends on**: 08-09, `09` (recommendation entity), `07`.

### STORY 08-11 · M4 · M · P1 — Report result overlay
- **Story**: As `AG`, I want report findings and zones rendered on the field, so that I can review the advisor's output in geographic context before delivery.
- **Deterministic / evidence**: `plugins/reports.rs` fetches the `09` report's findings/zones and draws zone polygons in the layer CRS with their reason/priority; a report-generate task triggers `09`.
- **Acceptance**:
  - Given a completed report, when its overlay loads, then finding zones draw in the correct CRS with reason and priority labels.
  - Given a report whose zones reference a different CRS than the active layer, when overlaid, then zones are reprojected or the overlay is refused, never drawn misaligned.
- **Tests**: unit (zone draw + labels), geospatial (zone CRS vs layer), failure path (CRS mismatch reprojected/refused).
- **Depends on**: 08-08, `09` (reports).

### STORY 08-12 · M4 · M · P1 — Compare mode (season/product)
- **Story**: As `AG`, I want side-by-side or swipe comparison across two scenes or products, so that I can judge change between seasons or products.
- **Deterministic / evidence**: compare two layers using season-group state; both layers are independently asserted against their manifests (08-05) and locked to a shared view; the divider/swipe keeps both georeferenced.
- **Acceptance**:
  - Given two comparable scenes of one field, when compare mode opens, then both render in a shared, georeferenced view and pan/zoom together.
  - Given two scenes with incompatible CRS/extent that cannot share a view, when compare is attempted, then it is refused with a mismatch message rather than showing misaligned panes.
- **Tests**: unit (shared-view sync), geospatial (both layers asserted), failure path (incompatible scenes refused).
- **Depends on**: 08-05, 08-08.

### STORY 08-13 · M4 · S · P1 — Saved views and snapshot export
- **Story**: As `AG`, I want to persist a named view and export a snapshot, so that I can hand a grower a deliverable from the viewer.
- **Deterministic / evidence**: persist `{name, field_id, scene_id, active_product, camera, overlays}`; export a snapshot image plus the view metadata so it can be reopened.
- **Acceptance**:
  - Given a configured view, when it is saved and reopened, then the field/scene/product/camera/overlays are restored exactly.
  - Given a snapshot export of an ungeoreferenced state, when export runs, then the snapshot is clearly marked non-georeferenced rather than implying placement.
- **Tests**: unit (save→restore round-trip), failure path (ungeoreferenced snapshot marked).
- **Depends on**: 08-06, 08-08.

---

## M5 — Autonomous-Assist (gated)

### STORY 08-14 · M5 · S · P2 — Suggested annotation prompts from advisor findings
- **Story**: As `AG`, I want the viewer to suggest annotation zones from `09` anomaly findings with an uncertainty flag, so that I start from candidate problem areas instead of a blank field — once findings are trustworthy.
- **Deterministic / evidence**: render `09` finding zones as suggested (non-committed) annotations carrying their evidence and uncertainty; the operator must confirm before any write-back to `07`; feature-flagged.
- **Acceptance**:
  - Given trustworthy `09` findings, when the viewer loads, then suggested zones appear as uncommitted annotations citing their evidence and uncertainty.
  - Given findings without asserted georeferencing, when suggestions are requested, then they are unavailable (gated), never drawn or auto-committed.
- **Tests**: unit (suggestion render + confirm gate), gating test (ungeoreferenced findings → disabled), failure path (operator rejects → no write-back).
- **Depends on**: 08-09, `09` (findings), 08-05.

---

## Coverage note

These 14 stories cover all 12 capabilities in `capability-map.md`, ordered by phase and consistent with `release-plan.md` (M1 8 / M2 9 / M3 22 / M4 22 / M5 2; P0 30 / P1 23 / P2 10). The domain leans M3/M4 because the Bevy plugin scaffolding already exists: the work is making layers provably georeferenced (08-05) with an on-screen CRS/extent readout (08-06), drawing the field boundary (08-07), and delivering the interactive annotate → recommend → report loop (08-09 → 08-10 → 08-11) plus compare mode (08-12). No overlay is drawn whose CRS/extent/resolution cannot be asserted against the `07` manifest, and every annotation writes back through `07` with author/severity/timestamp/audit. The surface always offers a path to a recommendation — no dead-end raster views. The curated ~63 backlog rows expand several stories into siblings — per-geometry annotation editing in 08-09, swipe vs side-by-side variants in 08-12, and additional saved-view/export formats in 08-13 — when implemented.
