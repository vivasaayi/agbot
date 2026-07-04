# AGBot Layer Browser (`/browse`)

MapLibre GL JS web UI for browsing satellite/derived layers served by geo_hub.
This is the GIS browsing surface decided in
`docs/design/satellite-intelligence-pipeline.md` ("Viewer, decided 2026-07");
the Bevy `geo_viewer` is not extended for satellite browsing.

## Run

```sh
cargo run -p geo_hub          # serves on the configured bind address
# then open http://127.0.0.1:<port>/browse
```

The page is self-contained apart from two deliberate external dependencies:

- **MapLibre GL JS v4.7.1** from the unpkg CDN, pinned with SRI hashes in
  `index.html` (bump version + both `integrity` attributes together).
- **OSM raster basemap tiles** (`tile.openstreetmap.org`) with attribution.

Everything else (STAC catalog, product tiles, field boundaries) is fetched
same-origin from geo_hub, so no CORS configuration is required.

## What it expects from geo_hub

- `GET /api/stac/collections` and `GET /api/stac/search` — the internal STAC
  catalog. Seed products via `POST /api/catalog/products` (see
  `geo_hub/tests/stac_api.rs` for draft shapes). Items need an RFC3339
  `temporal_start` and a **WGS84** bbox to be displayable; projected-CRS items
  appear in the list but are marked "not displayable" (geo_hub has no
  reprojection yet).
- `GET /api/scenes/:scene_id/products/:kind/tiles/:z/:x/:y.png` — product
  tiles. **These tiles are scene-local, not Web Mercator**: zoom `z` splits the
  product image itself into `2^z x 2^z` equal pixel tiles. The UI therefore
  stitches all tiles at a fixed zoom (z=2, 1024x1024) into a canvas and places
  it as a MapLibre `image` source positioned by the STAC item bbox (north-up
  raster assumed). A global-TMS (Web Mercator) tiler is future work; when it
  exists, switch `app.js` to a `raster` source using the tile URL template.
- `GET /api/fields/export/geojson` — active field boundaries as a GeoJSON
  FeatureCollection, drawn as an orange line overlay with click popups.

## Files

- `index.html` / `style.css` / `app.js` — no npm, no build step; the files are
  embedded into the geo_hub binary via `include_str!`
  (`geo_hub/src/routes/browse.rs`) and served at `/browse`, `/browse/app.js`,
  `/browse/style.css`.
