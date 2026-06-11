# Import/Export and Interop Adapters: Current State and Target State

## Mission

Be the bridge from insight to action: interchange geospatial formats with CRS-preserving fidelity, import field boundaries into the field model, and export management zones and per-zone rates as a machine-executable prescription a sprayer, planter, or tractor runs — so a finding in `09`/`23` becomes an executable file in `14`, and every import/export round-trips with provable geospatial correctness.

## Current Maturity

greenfield-leaning with scattered partials: export logic exists in three places but is unconsolidated and has no CRS-preservation guarantee. `09`'s `report_generator` targets PDF/HTML/JSON/CSV/KML/Shapefile but its encoders are TODO scaffolding; `geo_hub` imports and exports GeoJSON/shapefile; `geo_viewer` has exports. There is no `interop` crate, no round-trip CRS test, and no prescription/VRA export in a machine-executable format.

## What Exists Now

- `09` (`post_processor/src/report_generator.rs`): a `ReportGenerator` targeting PDF/HTML/JSON/CSV/KML/Shapefile, but the format-specific encoders are unimplemented TODOs.
- `geo_hub` (`src/{routes.rs,shapefile.rs,db.rs}`): GeoJSON/shapefile import and CSV/GeoJSON export routes, plus shapefile storage in the spatial DB.
- `geo_viewer`: export surfaces in the viewer.
- `07` (GIS hub): the CRS/extent/resolution contracts and reprojection groundwork that an import/export pipeline must reuse.
- `10` (field/farm data): the field-boundary model that imported boundaries must land in.
- `05`/`09`: management zones and recommendations that a prescription map is built from.

## Gaps to Close

- No consolidated `interop` crate: export logic is duplicated and inconsistent across `09`, `geo_hub`, and `geo_viewer`.
- No deterministic format-validation + CRS-reprojection pipeline: imports are not uniformly validated, reprojected, or rejected with reason codes.
- No guaranteed CRS/extent round-trip: an export can silently drop or mangle the coordinate reference, producing a wrong overlay.
- No GeoPackage or proper GeoTIFF import/export with asserted spatial metadata.
- No field-boundary import path that validates geometry and lands a boundary in `10` (rejecting malformed or oblique-CRS files cleanly).
- No prescription / VRA export at all — neither prescription Shapefile nor ISO-XML / ISOBUS TaskData — so findings cannot yet drive a machine (`14`).
- No platform connectors (John Deere Operations Center, Climate FieldView, Trimble) for boundary/prescription exchange.
- No bulk import/export or migration, and no import provenance recorded via `30`.

## Related Existing Surfaces

- Domain `09` (`post_processor`): the report generator scaffold and its PDF/CSV/KML/Shapefile targets to consolidate.
- Domain `07` (GIS hub): CRS/extent/resolution contracts and reprojection to reuse for the import/export pipeline.
- Domain `10` (field/farm data): the field-boundary model that imported boundaries land in.
- Domains `05`/`09` (imagery / advisor): management zones and per-zone rates that become a prescription map.
- Domain `14` (autonomous tractor): the consumer that executes an exported prescription.
- Domain `30` (provenance/audit): records import source, format, and CRS as lineage.

## Target Operating Model

- One `interop` crate owns all import/export, with a single deterministic pipeline: validate the file, assert/reproject its CRS, and reject malformed or oblique-CRS input with reason codes.
- Vector (Shapefile, KML/KMZ, GeoJSON, GeoPackage) and raster (GeoTIFF) round-trip with CRS and extent preserved and asserted — a round-trip test proves no coordinate drift.
- Field-boundary import lands a validated boundary in `10`; geometry errors and unsupported CRS are clean, tested rejections, never silent.
- Prescription / VRA export turns `05`/`09` management zones + per-zone rates into a prescription Shapefile and a machine-executable ISO-XML / ISOBUS TaskData that `14` executes — the bridge from finding to action.
- Platform connectors (John Deere Operations Center, Climate FieldView, Trimble) exchange boundaries and prescriptions behind mockable external boundaries, simulation-first.
- Bulk import/export and migration produce a per-row report; every import records its source, format, and CRS as provenance via `30`.
- Every import/export asserts CRS/extent and has at least the happy path and one malformed/oblique-CRS failure path tested — a wrong overlay or a misaligned prescription is treated as worse than none.
