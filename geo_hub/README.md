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
