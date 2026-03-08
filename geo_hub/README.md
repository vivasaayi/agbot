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
