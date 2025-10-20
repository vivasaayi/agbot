# geo_hub

Foundational service for ingesting, indexing, and serving geospatial products across multiple satellites.

## Current status

- ✅ Axum-based HTTP server scaffold with `/health` and `/ready` endpoints
- 🚧 Future work: product catalog, tile service, analytics job scheduling

## Running

```bash
cargo run -p geo_hub
```

Override the bind address with `GEO_HUB_BIND=127.0.0.1:8090`.

## Next steps

1. Define configuration schema (ingest paths, cache directories)
2. Implement metadata ingestion and storage
3. Expose catalog search endpoints and tile streaming APIs
4. Integrate analytics pipelines (spectral indices, thermal, masks)
``