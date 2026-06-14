# geo_viewer

Bevy desktop client for browsing geospatial products served by `geo_hub`.

## Current status

- ✅ Bevy + egui app with side panel controls
- ✅ Scene input + on-demand product loading
- ✅ Fetches image bytes from `geo_hub` and renders as texture sprite
- ✅ Zoom control for loaded product images

## Running

```bash
cargo run -p geo_viewer
```

Optional environment variables:

- `GEO_HUB_URL=http://127.0.0.1:8080`
- `GEO_VIEWER_SCENE_ID=<scene_id>`

## Backend contract

`geo_viewer` requests:

```text
GET /api/scenes/<scene_id>/products/ndvi
```

The endpoint should return image bytes (PNG recommended).

## Quick local flow

1. Run `geo_hub`.
2. Ensure a file exists at `<data_root>/scenes/<scene_id>/products/ndvi/*.png`.
3. Launch `geo_viewer` and enter the same `<scene_id>`.
4. Click `Load NDVI`.
