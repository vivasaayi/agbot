# Manhattan World Simulator — Implementation Plan

Derived from the research report (`Docs/flight-sim/r1.md`) reviewed against the
current `flight_sim_cpp` codebase. This plan is a **production-hardening
roadmap**, not a greenfield design: the demo pipeline
(`demo/world_demo_main.cpp`) already fuses DEM + detail, imports NYC footprints,
extrudes a batched city mesh, drapes OSM basemap, flies a Cessna Dubins circuit,
and routes a robot over the OSM road graph with terrain RMSE validation. The job
is to make that pipeline **deterministic, provenance-rich, authoritative-source-first,
and gated**.

## 1. Report review — verdict

The report is sound and well-matched to the repo. Adopt its three locking
principles verbatim:

1. **Official NYC geometry inside the AOI whenever it exists** (footprints, not PLUTO).
2. **Bare-earth DEM as terrain ground truth**; DSM/imagery are detail, not authority.
3. **One source-lineage manifest per tile** so every triangle traces to source + license.

Adaptations for our reality:

- **Don't rip out Terrarium.** The report calls it a "compatibility format," which
  matches the code — `GeoTerrain.hpp` already models `TerrainTileState::FlatFallback`
  and the demo already validates fused terrain RMSE against the DEM reference. Keep
  Terrarium as the global fallback; add 3DEP/NYC DEM as the authoritative AOI source
  *above* it in a ranked stack.
- **Renderer backend is not on the critical path.** The report's bgfx-vs-Dawn
  decision matters for browser parity, but the current OpenGL viewer + `.agbscn`
  scene-file boundary (`render/SceneFile.cpp`) is sufficient through Gate 4. Defer
  the backend swap; keep the `Renderer` interface clean (it already anticipates this).
- **Nav2/JSBSim are references, not dependencies.** We already have `MppiPlanner`,
  `HybridAStarPlanner`, `RoadGraphPlanner`, `FixedWingModel` + autopilot. Use the
  report's Nav2/JSBSim material as a validation-target and naming discipline, not as
  a port.

## 2. Current-state gap analysis (report area → repo state)

| Report area | Repo today | Gap to close |
|---|---|---|
| Deterministic compiler, `.agbworld` + `.agbscn` | `world_demo_main.cpp` (monolithic main), `render/SceneFile.cpp` writes `.agbscn` | No `.agbworld` manifest-of-manifests; no per-tile provenance; compiler logic lives in `main()` not a library |
| Source hierarchy (municipal→federal→open→AI) | `VectorImportExtractor` (NYC footprints), `RoadImport` (OSM), Terrarium DEM | No ranked multi-source fusion; no 3DEP/NYC-DEM ingest; no source-lineage record |
| CRS/datum policy | `GeoTerrain` = EPSG:4326 only; local ENU via `local_from_geo` | No EPSG:2263 ingest adapter; no NAD83(2011)/UTM18N+NAVD88 canonical metric frame; no datum-transform layer |
| Layered terrain (DEM/DSM/topobathy/landcover) | `terrain_engine` fusion (DEM + synthetic detail), `Validation` (RMSE/MAE/bias/NMAD) | No DSM residual, no topobathy waterfront fusion, no 6-inch land-cover semantic mask, no per-tile elevation-state |
| Buildings LoD1 ranked height stack | `HeightResolver` (attr > levels > default), `SceneMesh` extrusion | Only 3 height sources; no Overture join, no BES/LiDAR-residual, no NYC-3D benchmark; holes dropped at `SceneSynthesis` |
| Buildings LoD2 | none | Building-part decomposition, roof-shape, CDT for holes (earcut present, no CDT) |
| Renderer/streaming (3D Tiles/glTF, clipmaps) | OpenGL viewer, flat terrain grid, `.agbscn` | No terrain LOD/clipmap, no 3D-Tiles city streaming, no deterministic scene hash canonicalization |
| Sensors/autonomy | full `nav/` stack + LiDAR raycast + multispectral/RGB cameras | No unified evidence loop wiring RGB+depth+semantic+LiDAR→occupancy→costmap in one graph; nav gate metrics partial |
| Fixed-wing physics | `FixedWingModel` (478 lines) + autopilot + `cessna_tests` | Not validated against NASA 6-DOF check cases; no AIAA S-119 variable naming; stall/wind not first-class acceptance |
| Weather/atmosphere | steady wind vector, sensor presets | No weather preset schema, no sun ephemeris, no sky/atmosphere model, no night-lighting from road/POI scaffold |
| Validation gates | terrain RMSE + `--check` invariants + golden regression | 6 formal gates not codified as a suite tied to source truth |

## 3. Sequenced milestones

Each milestone is independently committable and ends with a passing gate. Build/test
via `just flight-sim-build` / `just flight-sim-test`. Milestones map to the report's
six acceptance gates.

### M1 — Extract the world compiler as a library (foundation) — ✅ DONE (commit 540cde2)
**Why first:** everything else needs a real compiler API, not logic buried in `main()`.
- New `worldgen` compiler entrypoint: `compile_world(AoiSpec, SourceManifest, seed) → WorldArtifact`.
- Define `.agbworld` manifest struct: compiler version, AOI, source snapshot IDs +
  license, CRS/datum policy, tile index, quality metrics, global content hashes.
- Add per-`.agbscn`-tile provenance map (source dataset + license per layer).
- Move `world_demo_main.cpp` logic behind this API; `main` becomes a thin driver.
- **Gate 1 (World compile):** Lower Manhattan compiles from pinned snapshots to a
  stable `.agbworld` + tiles with reproducible content hashes and zero fatal errors.
- Tests: `worldgen_tests` — deterministic hash stability across two compiles.

### M2 — CRS/datum discipline — ✅ DONE
- `worldgen/Crs.{hpp,cpp}`: EPSG:2263 (NY State Plane LI, US ft, Lambert Conformal
  Conic 2SP) and EPSG:26918 (UTM 18N, Transverse Mercator) ↔ WGS84 on GRS80.
- `VerticalDatum` enum + `vertical_datums_compatible` (orthometric vs ellipsoidal
  rejected; NAVD88 family compatible).
- `source_crs` param on `vector_import`: projected footprints normalized to WGS84
  at ingest; native CRS preserved in source provenance.
- Compiler rejects mixed vertical datums (`mixed_vertical_datum`) when buildings
  carry base elevations; manifest policy/sources record the resolved datum.
- UTM 18N adapter is in place for M3 (3DEP DEM arrives in UTM/NAVD88); the runtime
  frame remains per-tile local ENU (not yet routed through UTM — deferred to when
  authoritative metric sources land).
- Tests: 2263/UTM18N round-trip + analytic anchor + independent scale checks;
  end-to-end 2263 ingest; datum-mismatch rejection.

### M3 — Authoritative terrain stack + no-silent-zero — ✅ DONE
Multi-batch (needs external data; user authorized acquisition).

**Batch 1 — authoritative 3DEP DEM + Gate 2 — ✅ DONE**
- `terrain_engine/GeoTiff.{hpp,cpp}`: minimal uncompressed float32 GeoTIFF reader
  (tiled/stripped, LE/BE, ModelTiepoint/PixelScale georef, GDAL_NODATA) — targets
  the USGS 3DEP `exportImage` product, no GDAL dependency.
- `dem_fusion` gains `source="geotiff"`: authoritative bare-earth DEM resampled onto
  the AOI grid; cells outside coverage stay nodata (no-silent-zero); confidence 0 there.
- `fetch_3dep_dem.sh`: version-pinned USGS 3DEP fetcher → `data/terrain/manhattan_3dep_dem.tif`
  + provenance sidecar (request URL, UTC, sha256, NAVD88).
- Demo/compiler use 3DEP as the Gate 2 reference layer (Terrarium fallback when absent);
  manifest records `vertical_datum=NAVD88` + 3DEP-licensed source; mixed-datum guard active.
- **Gate 2:** compiled terrain RMSE/MAE/bias vs the authoritative DEM — measured
  **0.96 m RMSE / 0.61 m MAE / ~0 bias** on Lower Manhattan; asserted < 2 m.
- Tests: GeoTIFF reader pinned to an independent decode of a committed 128² fixture;
  `dem_fusion:geotiff` estimator path; Gate 2 assertions in `world_demo --check`.

**Batch 2 — per-tile elevation-state + no-silent-zero — ✅ DONE**
- `ElevationState` {authoritative, fallback, masked_water, missing} + `fallback_reason`
  on every tile; terrain cell / authoritative-cell / nodata-cell accounting in the
  manifest quality block; folded into `world_hash`.
- Missing elevation is never coerced to zero: it is counted and reason-coded
  (`NO_AUTHORITATIVE_SOURCE`, `NODATA_STRIP`, `NO_TERRAIN`).
- Tests: authoritative state from the 3DEP fixture (full coverage, no gaps);
  fallback state from synthetic terrain; demo asserts cell accounting + state class.

**Batch 3 — water masking + masked_water — ✅ DONE (water)**
- `terrain_engine/WaterMask`: sea-connected flood fill (4-neighbourhood from the grid
  boundary over sub-sea-level/nodata cells). Interior below-sea depressions are left as
  land — the "no invented harbor holes" rule. Compiler counts water cells and promotes a
  predominantly-water tile to `masked_water` (`WATER_MASK_ONLY`).
- Lower Manhattan: 3822/16384 cells (~23%) masked as the Hudson + East rivers.
- Tests: boundary-connected channel is water, interior pit is not, threshold sweeps.

**Batch 3 (DSM residual + land-cover semantic classes) — ✅ DONE (commit ec091a2)**
- `worldgen/TerrainSemantics`: `apply_dsm_measured_heights` samples a highest-hit
  DSM and the bare-earth ground at each footprint centroid; the residual populates
  the `measured` height tier (`height_source="measured"`, outranks the attribute).
  Out-of-coverage / implausible residuals fall through. Vertical-datum discipline:
  an ellipsoidal DSM against orthometric terrain is rejected (`mixed_vertical_datum_dsm`).
- `read_geotiff_categorical` generalizes the GeoTIFF reader to single-band integer
  sample formats (8/16/32-bit); the float DEM reader still rejects integers.
  `sample_landcover_histogram` tallies class ids over the terrain grid (nearest,
  class -1 = unknown) into a deterministic manifest histogram.
- Both adapters are optional (activate when the reprojected raster is present).
  DSM + land-cover source snapshots + tile provenance layers recorded.
- Ingest scripts `fetch_nyc_dsm.sh` / `fetch_nyc_landcover.sh` reproject the NYC
  DSM / 6-inch land cover to lon/lat GeoTIFFs (GDAL; nearest for categorical) with
  provenance sidecars.
- Tests: adapter unit tests, categorical + float GeoTIFF reader coverage (in-test
  writers), and an end-to-end compiler wiring test (applied count, measured-tier
  accounting, provenance, datum rejection).
- **Note:** the NYC DSM/land-cover snapshots are not yet fetched, so the demo
  compiles in fallback (measured 0, 100% attribute heights). Run the fetch scripts
  to activate the measured tier + land-cover histogram on Lower Manhattan.

### M4 — Buildings LoD1 ranked height + hole preservation — ✅ DONE
- `HeightResolver` extended into a ranked, provenance-tagged stack:
  **measured** (LiDAR DSM−DEM residual / photogrammetric column, metres) → **attribute**
  (height_roof) → **levels**×storey → **default**. `vector_import` gains `measured_attr`;
  the `measured` tier is wired and ready for the M3-batch-3 DSM residual to populate it.
- Base elevation already anchored to NYC grade via `base_elev_attr` (ground_elevation).
- Hole/courtyard preservation: earcut-with-holes already triangulates courtyards
  correctly (verified by the donut mesh test — 8 cap + 16 wall tris); constrained
  Delaunay is unnecessary for these footprints. Courtyards are counted in the manifest.
- **Gate 3 (Buildings):** manifest records building count, footprint-area sum,
  median/max height, courtyard count, and the height-source breakdown. On Lower
  Manhattan: 2198 buildings, median 17.1 m, 709,862 m² footprint, 17 courtyards,
  100% heights from the authoritative NYC height attribute. Asserted in `world_demo`.
- Deferred (need extra sources): Overture per-building join, NYC-3D benchmark
  spot-check, PLUTO lot enrichment.

### M5 — Deterministic render + sensor-graph unification — ✅ DONE
- `render/OffscreenRenderer`: a dependency-free CPU rasterizer (z-buffered,
  perspective-correct, deterministic iteration) that renders the **same**
  `RenderScene` the viewer shows into co-registered **RGB + linear-depth +
  semantic-id** buffers, plus a `frame_hash`.
- **Gate 4 (Render):** on the compiled Manhattan scene from a canonical aerial pose —
  52.8% non-blank coverage, byte-identical `frame_hash` across renders, and depth⇔semantic
  co-registration (every hit has finite positive depth). Asserted in `world_demo --check`.
- Unit tests: quad coverage/depth/colour/semantic, background sky, occlusion z-test,
  determinism.
- Deferred (non-blocking): bgfx/Dawn GPU backend, terrain clipmaps, 3D-Tiles streaming.
  The deterministic scene geometry hash already lives in `.agbworld` (tile.content_hash).

### M6 — Autonomy evidence loop + nav gate — 🚧 IN PROGRESS

**Batch 1 — city occupancy + Gate 5 (path-level) — ✅ DONE (commit 1adf6f4)**
- `nav/CityEvidence`: building footprints (the same geometry the sensor observes)
  rasterized into an occupancy costmap via even-odd scanline fill (holes/courtyards
  left free, street corridors traversable) + separable Chebyshev inflation.
- `run_evidence_loop`: A* global plan start→goal, arc-length-midpoint recovery probe
  (block midpoint → replan), nearest-free-cell goal tolerance (snap blocked
  endpoints), reason-coded failure (`start_blocked`/`goal_blocked`/`no_initial_plan`/
  `no_recovery_plan`).
- `EvidencePlanResult` reports length, euclidean, min clearance, replan/recovery
  counts, lethal-cell count, collision-free, failure class.
- **Gate 5:** robot routes ~3060 m collision-free over Lower Manhattan occupancy
  (2997 m euclidean), 3 m clearance, recovers once. Asserted in `world_demo --check`;
  unit tests in `nav/city_evidence_tests`.

**Batch 2 — sensor-derived occupancy + consistency — ✅ DONE (commit 5374273)**
- `occupancy_from_sensor_frame`: back-projects a co-registered depth+semantic
  offscreen frame into an XZ occupancy grid via the render camera basis (depth is
  eye-space metres along forward); hits above a height threshold mark obstacles,
  ground/terrain stays free. The costmap is now what the robot *perceives*.
- `occupancy_consistency`: precision/recall between the sensor-derived grid and the
  authoritative footprint grid (Chebyshev tolerance absorbs sub-cell error).
- `AStarPlanner.PlanResult.expanded` (nodes popped/closed) backs deterministic
  **time-to-first-plan** / **time-in-recovery** proxies on `EvidencePlanResult`.
- **Gate 5** renders a near-nadir sensor frame and asserts perceived occupancy
  agrees with footprints: Lower Manhattan **precision 1.0** (1926/1926 perceived
  obstacle cells are real buildings), recall 0.44 (single pose / occlusion /
  >25 m roofs), plus non-zero planner effort. Unit tests cover back-projection,
  the precision/recall metric, and effort reporting.

**Batch 3 (remaining) — full local-control pipeline**
- Wire the end-to-end `NavigationPipeline`: global (A*/Hybrid-A*) → **local MPPI**
  controller → recovery/replan behavior, driving a robot state over the
  sensor-derived costmap (not just planning a path). Keep **Pure Pursuit /
  Stanley** as interpretable baselines.
- Fold LiDAR ray hits into the same occupancy grid alongside depth+semantic.
- Extend Gate 5 with executed-trajectory metrics (tracking error, controller
  smoothness) on top of the path-level metrics already gated.

### M7 — Fixed-wing validation + weather/atmosphere — ⬜ NOT STARTED
Greenfield (no weather/atmosphere/ephemeris module exists yet). Sequenced sub-steps:

**Batch 1 — flight-dynamics validation harness**
- Adopt **AIAA S-119** variable naming for the fixed-wing state/force log
  (`vehicles/FixedWingModel` already produces the state; add a named-channel logger).
- Encode **NASA 6-DOF check cases** as fixtures; add a `cessna_tests`-style suite that
  replays fixed IC + control inputs and compares trajectory channels within tolerance.
- First-class acceptance cases: trimmed flight, coordinated turn, crosswind response,
  climb/descent, **stall entry/recovery** (tie stall to critical AoA + load factor,
  not airspeed alone). Deterministic replay asserted.

**Batch 2 — deterministic weather presets**
- Weather-preset schema (UTC timestamp, sun/moon ephemeris inputs, visibility, cloud
  layers, wind ground+aloft, precip class/rate, temp/pressure, road/roof wetness).
- NOAA solar-position equations for deterministic sun/sky; presets first, live
  METAR/TAF assimilation deferred. Wire the preset wind into `FixedWingModel`.

**Batch 3 — atmosphere + night lighting (visual, non-blocking for Gate 6)**
- Atmosphere v1: analytic **Preetham** sky + aerial perspective + visibility haze.
  v2 Bruneton-class deferred. Night lighting generated from road hierarchy +
  intersection density + POI scaffold (not random bloom).

- **Gate 6 (Flight dynamics):** deterministic replay under fixed weather/IC/inputs,
  checked against the 6-DOF cases; stall/crosswind cases pass.

## 4. Decisions the user should make (not discoverable locally)

1. **Renderer backend for M5+:** bgfx (lowest-risk native migration, multi-backend) vs
   Dawn/WebGPU (browser parity, more infra to own). Recommendation: **bgfx** unless a
   browser client is a near-term product goal.
2. **Data acquisition:** 3DEP DEM is fetched + pinned. DSM + 6-inch land-cover
   ingest scripts exist (`fetch_nyc_dsm.sh` / `fetch_nyc_landcover.sh`, GDAL reproject)
   but the NYC snapshots are **not yet fetched** — the demo runs in fallback until they
   are. NYC-3D model and Overture snapshots still pending. Confirm we may store the
   reprojected snapshots (licenses: NYC Open Data terms; OSM/Overture ODbL vs CDLA —
   the report flags this as a real risk to resolve before ingesting Overture).
3. **Scope of first vertical slice:** recommend Lower Manhattan (the demo's existing
   AOI, `40.700..40.740, -74.020..-73.980`) through Gates 1–3 before widening.

## 5. Immediate next step

M1–M5, M3-batch-3, and M6 batches 1–2 are done and committed (sensor-derived
occupancy now cross-checks the compiled footprints at precision 1.0). Two tracks
remain: **M6 batch 3** — wire the full `NavigationPipeline` (global → MPPI local →
recovery) to *execute* a trajectory over the sensor costmap and fold in LiDAR hits;
and **M7** — fixed-wing 6-DOF validation → deterministic weather presets → atmosphere.
Recommend M7 next (larger, greenfield, unblocks the flight-dynamics gate), with M6
batch 3 as a parallel autonomy-depth track.
