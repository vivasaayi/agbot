# geo_hub

Axum service for ingesting scene metadata and serving geospatial products to clients like `geo_viewer`.

## Current status

- ✅ Health endpoints: `/health`, `/ready`
- ✅ Scene listing endpoint: `GET /api/scenes`
- ✅ Product endpoint: `GET /api/scenes/:scene_id/products/:kind`
- ✅ File-backed serving from `data_root/scenes/<scene_id>/products/<kind>/`
- ✅ Fallback generation for supported derived products (currently `ndvi`)

## Running

```bash
cargo run -p geo_hub
```

Useful environment overrides:

- `GEO_HUB__BIND_ADDRESS=127.0.0.1:8080`
- `GEO_HUB__DATA_ROOT=/absolute/path/to/data/geo_hub`
- `GEO_HUB__DATABASE_URL=sqlite://geo_hub.db?mode=rwc`
- `GEO_HUB__TERRAIN__COMPILER_PATH=/absolute/path/to/agbot_terrain_compile`

## Elevation to simulator terrain

Provider DEM/DSM files enter the same catalog and GIS path as satellite
imagery. List the supported source profiles and ingest a server-local
GeoTIFF/COG:

```bash
curl "http://127.0.0.1:8080/api/ingest/elevation/sources"

curl -X POST "http://127.0.0.1:8080/api/ingest/elevation" \
  -H "content-type: application/json" \
  -d '{
    "profile_id": "copernicus_dem_glo30",
    "scene_id": "cop-dem-example",
    "artifact_path": "/absolute/path/to/cop-dem.tif",
    "acquired_at": "2026-07-01T00:00:00Z"
  }'
```

The ingest response contains the L1 elevation product ID and GIS tile URL.
After building `flight_sim_cpp`, derive the traceable L3 simulator package:

```bash
curl -X POST "http://127.0.0.1:8080/api/terrain/derive" \
  -H "content-type: application/json" \
  -d '{
    "elevation_product_id": "<L1 product ID>",
    "resolution": 256,
    "target_gsd_m": 30,
    "expected_vertical_datum": "EGM2008",
    "seed": 7
  }'
```

The result is a cataloged `sim_terrain_package` backed by `.agbworld`,
`.agbscn`, and validation artifacts. The first bridge accepts EPSG:4326
elevation only and rejects missing, changed, or datum-mismatched source
evidence.

## File-backed contract for quick local testing

Place product files here:

```text
<data_root>/
  scenes/
    <scene_id>/
      products/
        ndvi/
          output.png
```

Then request:

```bash
curl -i "http://127.0.0.1:8080/api/scenes/<scene_id>/products/ndvi"
```

If a local file is missing, `geo_hub` attempts to generate the product from ingested scene metadata when available.

## Boundary import strategy

`geo_hub` uses a native Rust shapefile reader for field-boundary import. The current strategy is deliberate:

- no GDAL/OGR system dependency in the default path
- import from a local `.shp` file path via `POST /api/fields/import/shapefile`
- only polygon shapefiles are accepted
- only single-ring field boundaries are accepted
- coordinates must already be geographic lon/lat in `EPSG:4326`

Example request:

```bash
curl -X POST "http://127.0.0.1:8080/api/fields/import/shapefile" \
  -H "content-type: application/json" \
  -d '{
    "path": "/absolute/path/to/field_boundary.shp",
    "name_prefix": "North 80",
    "crop": "corn",
    "season": "2026"
  }'
```

Current limits:

- multipart polygons and holes are rejected with a `400`
- projected shapefiles are rejected; reproject to `EPSG:4326` first
- DBF attribute mapping is not implemented yet; naming comes from the request or file stem

## KML decision

KML is intentionally deferred. GeoJSON and polygon shapefiles cover the initial advisor workflow, and KML would add another import surface before the farm and recommendation workflows are fully stabilized.
