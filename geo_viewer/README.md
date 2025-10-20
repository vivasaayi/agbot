# geo_viewer

Bevy-powered desktop client for browsing geospatial products served by `geo_hub`.

## Current status

- ✅ Bevy + egui application scaffold
- ✅ Placeholder UI for layer selection and zoom control
- 🚧 Pending: tile fetching, map rendering, backend integration

## Running

```bash
cargo run -p geo_viewer
```

The window currently renders debug gizmos to verify the render loop. Future iterations will stream tiles and analytics overlays from the hub service.
