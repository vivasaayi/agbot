# Import/Export and Interop Adapters: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (geospatial correctness first, then agronomic value, explainability/trust, data quality, operability) and the workstreams in `release-plan.md`. Partial export already exists but is scattered across `09`, `geo_hub`, and `geo_viewer`; the first slices consolidate it into one `interop` crate with a CRS-preserving pipeline. Geospatial correctness dominates: a round-trip that loses CRS or extent, or a misaligned prescription, is worse than no export at all. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Import/Export and Interop Adapters Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Format-validation + CRS-reprojection pipeline | scattered partial | 9 | Validate + reproject one import deterministically |
| Vector import/export (Shapefile/KML/GeoJSON/GeoPackage) | scattered partial | 9 | Round-trip Shapefile/GeoJSON with CRS preserved |
| Raster import/export (GeoTIFF) | partial | 6 | Export a GeoTIFF with asserted CRS/extent/resolution |
| Field-boundary import (-> `10`) | partial | 7 | Import a Shapefile/KML boundary into `10` with validation |
| Prescription / VRA export (Shapefile) | missing | 8 | Export zones+rates as a prescription Shapefile |
| Prescription / VRA export (ISO-XML / ISOBUS TaskData) | missing (greenfield) | 8 | Export a machine-executable ISO-XML TaskData |
| John Deere Operations Center connector | missing (greenfield) | 6 | Exchange boundaries/prescriptions (mockable) |
| Climate FieldView connector | missing (greenfield) | 5 | Exchange boundaries/prescriptions (mockable) |
| Trimble connector | missing (greenfield) | 4 | Exchange boundaries/prescriptions (mockable) |
| Bulk import/export and migration | missing (greenfield) | 6 | Bulk-import many boundaries with a per-row report |
| Provenance-on-import (via `30`) | missing (greenfield) | 5 | Record import source/format/CRS as lineage |
| Export catalog and report generator consolidation (`09`) | scattered partial | 6 | Route `09` report export through `interop` |
