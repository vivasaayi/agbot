# Import/Export and Interop Adapters

Geospatial format interchange and the variable-rate-application (VRA) / prescription-map export that turns a finding into a file a sprayer, planter, or tractor executes — the bridge from insight (`09`/`23`) to action (`14`).

## Where We Are

- Greenfield-leaning, but partial export already exists. `09`'s `report_generator` scaffolds PDF/HTML/JSON/CSV/KML/Shapefile, `geo_hub` serves and imports GeoJSON/shapefile, and `geo_viewer` has exports — all scattered and unconsolidated.
- There is no single `interop` crate, no CRS-preserving round-trip guarantee, and no prescription/VRA export in a machine-executable format (ISO-XML / ISOBUS TaskData).
- Geospatial correctness dominates: every import and export must preserve CRS and extent, because a wrong overlay or a misaligned prescription is worse than none.

## Where We Should Be

- A consolidated `interop` crate that imports and exports Shapefile, KML/KMZ, GeoJSON, GeoPackage, and GeoTIFF with CRS preservation and reprojection, round-trip-tested.
- Field-boundary import (Shapefile/KML → `10` boundaries) with deterministic validation and reason-coded rejection of malformed or oblique-CRS files.
- Prescription / VRA export: management zones + per-zone rates → standard prescription formats (Shapefile, ISO-XML / ISOBUS TaskData) a machine executes — the bridge from finding to action, consumed by `14`.
- Platform interop connectors (John Deere Operations Center, Climate FieldView, Trimble) for boundary and prescription exchange, behind mockable external boundaries.
- Bulk import/export and migration, with import provenance recorded via `30`.

## Files

- `current-state.md`: maturity, what exists now (scattered partials), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.
- `stories.md`: per-capability vertical-slice stories.

## Build Order

1. Consolidate scattered export into one `interop` crate with a deterministic format-validation + CRS-reprojection pipeline.
2. Geospatial import/export round-trip (Shapefile, KML/KMZ, GeoJSON, GeoPackage, GeoTIFF) with CRS preservation.
3. Field-boundary import (Shapefile/KML → `10`) with validation and reason-coded rejection.
4. Prescription / VRA export (Shapefile + ISO-XML / ISOBUS TaskData) from `09`/`05` management zones.
5. Platform interop connectors (John Deere, FieldView, Trimble) behind mockable boundaries.
6. Bulk import/export and migration, with import provenance via `30`.

## Primary Crates

New crate `interop`, consolidating export logic from `09` (`report_generator`), `geo_hub` (GeoJSON/shapefile import/export), and `geo_viewer`. Builds on `07` (CRS/reprojection, boundaries), `10` (field boundaries), and `05`/`09` (management zones); produces prescriptions consumed by `14`; records import source via `30`.
