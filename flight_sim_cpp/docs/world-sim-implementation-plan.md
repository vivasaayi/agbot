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

### M1 — Extract the world compiler as a library (foundation)
**Why first:** everything else needs a real compiler API, not logic buried in `main()`.
- New `worldgen` compiler entrypoint: `compile_world(AoiSpec, SourceManifest, seed) → WorldArtifact`.
- Define `.agbworld` manifest struct: compiler version, AOI, source snapshot IDs +
  license, CRS/datum policy, tile index, quality metrics, global content hashes.
- Add per-`.agbscn`-tile provenance map (source dataset + license per layer).
- Move `world_demo_main.cpp` logic behind this API; `main` becomes a thin driver.
- **Gate 1 (World compile):** Lower Manhattan compiles from pinned snapshots to a
  stable `.agbworld` + tiles with reproducible content hashes and zero fatal errors.
- Tests: `worldgen_tests` — deterministic hash stability across two compiles.

### M2 — CRS/datum discipline
- Add ingest adapter for **EPSG:2263** (NY State Plane LI, feet) → canonical frame.
- Canonical metric frame **NAD83(2011)/UTM18N + NAVD88**; per-tile local ENU for runtime.
- Explicit datum tags on every elevation source; reject mixed orthometric/ellipsoidal.
- Tests: round-trip EPSG:2263 ↔ canonical within tolerance; datum-mismatch rejection.

### M3 — Authoritative terrain stack + no-silent-zero
- Ranked terrain sources: 3DEP 1 m DEM / NYC-NYS DEM (authoritative) → Terrarium (fallback).
- Fuse: height (DEM), surface residual (DSM−DEM), albedo (true ortho), semantic mask
  (6-inch land cover). DSM/landcover ingest into `terrain_engine`.
- Topobathy waterfront fusion; water polygons intentional (no resampled harbor holes).
- Every terrain tile carries one state: `authoritative` | `fallback` | `masked_water`
  | `missing`, with `fallback_reason` (`NO_SOURCE`/`DATUM_FAILURE`/`NODATA_STRIP`/`WATER_MASK_ONLY`).
  Extends existing `TerrainTileState`.
- **Gate 2 (Terrain):** compiled mesh vs source DEM RMSE/MAE/bias, fallback tiles
  reported explicitly with source lineage. Acceptance tighter than source error
  (resampling-only, well under the demo's current <5 m).

### M4 — Buildings LoD1 ranked height + hole preservation
- Ranked height stack: NYC-3D benchmark → Overture per-building → BES/planimetric →
  LiDAR DSM−DEM residual → floors×prior (flagged estimated). Extend `HeightResolver`.
- Anchor base elevation to real building grade (BES).
- Preserve courtyards/holes: replace earcut with **constrained Delaunay** (Shewchuk
  Triangle-class) for holed footprints; carry holes through `SceneSynthesis` (currently
  exterior-ring only).
- PLUTO used only as lot-level enrichment after footprint→lot association, never geometry.
- **Gate 3 (Buildings):** building count, footprint-area sum, height sanity vs
  authoritative NYC sources; spot-check roof detail against NYC-3D model.

### M5 — Deterministic render + sensor-graph unification
- Canonicalize vertex/material/instance ordering + quantization before hashing →
  deterministic scene hashes in `.agbworld`.
- Offscreen sensor path (RGB, semantic ID, linear depth, LiDAR rays) reads the **same**
  scene graph as the human renderer.
- **Gate 4 (Render):** non-blank offscreen outputs, min non-clear pixel ratio from
  canonical poses, stable scene hashes, depth/semantic consistency.
- *(Deferred sub-task, not blocking: bgfx-vs-Dawn backend + terrain clipmaps + 3D-Tiles
  city streaming. Decide backend at start of M5 — see §4.)*

### M6 — Autonomy evidence loop + nav gate
- Wire unified loop: offscreen RGB+depth+semantic+LiDAR → occupancy/voxel → costmap →
  global (A*/Hybrid-A*) → local (MPPI) → recovery. Components exist; wire them end-to-end.
- Keep Pure Pursuit / Stanley as interpretable baselines.
- **Gate 5 (Navigation):** delivery-robot scenario reaches goal collision-free; log
  path length, min clearance, replan count, recovery count, failure class, **plus
  time-to-first-plan, time-in-recovery, semantic/occupancy consistency**.

### M7 — Fixed-wing validation + weather/atmosphere
- Validate `FixedWingModel` against **NASA 6-DOF check cases**; adopt **AIAA S-119**
  variable naming for logged data.
- First-class acceptance: trimmed flight, coordinated turn, crosswind, climb/descent,
  stall entry/recovery, deterministic replay under a recorded weather preset.
- Weather preset schema (UTC, sun/moon ephemeris, visibility, cloud layers, wind
  ground/aloft, precip, temp/pressure, wetness). Deterministic first, live later.
- Atmosphere v1: analytic sky (Preetham) + aerial perspective + visibility haze.
  v2 Bruneton-class deferred. Night lighting from road hierarchy + POI scaffold.
- **Gate 6 (Flight dynamics):** deterministic replay under fixed weather/IC/inputs,
  checked against 6-DOF cases.

## 4. Decisions the user should make (not discoverable locally)

1. **Renderer backend for M5+:** bgfx (lowest-risk native migration, multi-backend) vs
   Dawn/WebGPU (browser parity, more infra to own). Recommendation: **bgfx** unless a
   browser client is a near-term product goal.
2. **Data acquisition:** 3DEP/NYC-DEM, DSM, 6-inch land cover, NYC-3D model, and
   Overture snapshots must be fetched + version-pinned. Confirm we may script these
   downloads and store snapshots (licenses: NYC Open Data terms, OSM/Overture ODbL vs
   CDLA — the report flags this as a real risk to resolve early).
3. **Scope of first vertical slice:** recommend Lower Manhattan (the demo's existing
   AOI, `40.700..40.740, -74.020..-73.980`) through Gates 1–3 before widening.

## 5. Immediate next step

Start **M1**: lift the compiler out of `world_demo_main.cpp` into a `worldgen`
library API with the `.agbworld` manifest + per-tile provenance, and stand up Gate 1
as a deterministic-hash test. This unblocks every later milestone and converts the
current demo into a reproducible build.
