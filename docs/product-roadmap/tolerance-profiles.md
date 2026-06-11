# Global Tolerance Profiles

Many stories across domains 02, 05, 06, 07, 08, 22, 28, and 32 use the phrase "within tolerance" without defining what that tolerance is. This document establishes the shared tolerance profiles that acceptance tests should reference by name rather than embedding magic constants inline.

When a story says "within tolerance," it must name which profile applies. If no profile fits, add one here.

---

## GEO — Geospatial Reprojection Tolerance

Applies to: coordinate round-trips, CRS reprojection, extent assertions, georeferenced overlay alignment.

| Metric | Tolerance | Notes |
| --- | --- | --- |
| Coordinate round-trip (lat/lon → projected → lat/lon) | ≤ 0.1 m | At the equator; tighter for RTK use cases |
| Pixel-corner drift after reprojection | ≤ 0.5 px at native resolution | Measured in the target CRS pixel grid |
| Extent boundary assertion | ≤ 1 m at each edge | For bounding-box assertions in acceptance tests |
| Tile alignment to terrain grid | ≤ 1 px at tile resolution | OSM/Terrarium z17 tiles |
| CRS authority code round-trip | exact match | EPSG/OGC authority + code must survive serialize/deserialize |

Profile name: `GEO`

---

## RASTER — Raster Product Tolerance

Applies to: NDVI and spectral index values, thermal products, DSM/DTM elevation grids, orthomosaic pixel values, raster transform round-trips.

| Metric | Tolerance | Notes |
| --- | --- | --- |
| Spectral index value (NDVI, NDRE, etc.) | ± 0.005 absolute | After reprojection to the test CRS; compare pixel-wise |
| Nodata mask agreement | exact pixel match | A pixel is nodata or it isn't; no fractional nodata |
| Raster transform (origin, pixel size, rotation) | ≤ 0.5 px drift | After affine round-trip at native resolution |
| Resolution assertion | ≤ 5 % relative deviation | Actual GSD vs. declared GSD in metadata |
| Elevation (DSM/DTM) agreement | ≤ 0.1 m | For synthetic test fixtures with known geometry |
| Band count and dtype | exact match | Schema assertion, not tolerance |

Profile name: `RASTER`

---

## CLOUD — Point Cloud Tolerance

Applies to: LiDAR raycast ranges, normals, density, and registration.

| Metric | Tolerance | Notes |
| --- | --- | --- |
| Range error (raycast vs. ground truth geometry) | ≤ 0.05 m | For simulated scenes with exact geometry |
| Normal direction error | ≤ 2° | Angle between computed and ground-truth normal |
| Point density agreement | ≤ 10 % relative | Actual pts/m² vs. expected for the scene |
| Registration residual (ICP or similar) | ≤ 0.1 m RMS | After alignment to reference cloud |
| Classification agreement | ≥ 95 % per-point accuracy | Against a labeled test fixture |

Profile name: `CLOUD`

---

## TELEM — Telemetry Tolerance

Applies to: golden-telemetry fixture comparisons, cross-build/cross-platform parity tests, trace diff assertions, replay round-trips.

| Metric | Tolerance | Notes |
| --- | --- | --- |
| Timestamp skew | ≤ 1 ms | Between recorded and replayed events at the same step |
| Position (lat/lon) | ≤ 0.5 m | For golden traces at GPS-scale precision |
| Altitude (barometric) | ≤ 0.3 m | Absolute deviation from golden fixture value |
| Velocity | ≤ 0.05 m/s | Per-axis, against golden fixture |
| Battery voltage | ≤ 0.01 V | Against golden fixture |
| Battery percentage | ≤ 0.5 % | Against golden fixture |
| Attitude (roll/pitch/yaw) | ≤ 0.1° | Against golden fixture |
| Deterministic runner: byte identity | exact | Two runs with the same seed must produce bit-identical JSONL |

Profile name: `TELEM`

---

## Usage in Stories

Reference a profile by name in the acceptance criterion rather than embedding a constant:

> "Given a field's DEM tiles, when terrain loads, then a known lat/lon round-trips to the correct elevation within **GEO** tolerance."

> "Given a seeded run, when the trace is compared to the golden fixture, then all fields agree within **TELEM** tolerance."

If a story needs a tighter tolerance for a specific field, state it explicitly alongside the profile name:

> "Position must agree within **TELEM** tolerance except altitude, which must agree within 0.05 m for RTK-grade missions."

---

## Affected Domains

| Domain | Profiles Used |
| --- | --- |
| 02 — Simulation and Digital Twin | TELEM, GEO, CLOUD |
| 05 — Imagery and Remote Sensing | RASTER, GEO |
| 06 — LiDAR Mapping and 3D | CLOUD, GEO |
| 07 — GIS and Geospatial Hub | GEO, RASTER |
| 08 — Geo Viewer and Visualization | GEO, RASTER |
| 22 — Orthomosaic and Photogrammetry | RASTER, GEO, CLOUD |
| 28 — Time-Series and Change Detection | RASTER, GEO |
| 32 — Import/Export and Interop | GEO, RASTER |
