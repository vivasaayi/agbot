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

### Authenticated appliance

Build the native viewer on the Mac with `cargo build --release -p geo_viewer`,
then run `python3 scripts/viewer` from the repository root. The launcher uses
`http://127.0.0.1:20080` by default, prompts for the portal access code without
echoing it, obtains a time-limited session, starts the viewer, and revokes the
session on normal exit. Geo Hub sessions can last up to 30 days if the launcher
is terminated before it can revoke them. Set `GEO_HUB_URL` to select another environment and
`AGBOT_VIEWER_BIN` to use a separately packaged executable. Remote URLs must use
HTTPS; HTTP is accepted only for loopback addresses.

The viewer sends the session only to `/api/` URLs on the configured Geo Hub
origin and does not follow authenticated redirects. External tile providers do
not receive the session. If the session expires while the viewer is open,
restart the launcher and sign in again. The native executable is separate from
the Docker appliance and must be built for the target Mac; this does not change
the staging image. Run `bash scripts/package-viewer worktree` to build a
versioned archive under that worktree's `out/viewer/`. Extract it and run
`./run-viewer`. For a release candidate, run `bash scripts/package-viewer staging`
from a clean checkout of `main`, test that archive with staging, then transfer
that same tested archive to the target Mac. The archive records source revision
and architecture in `manifest.json` and the command prints its SHA-256 checksum.
To verify authentication without opening a window, run
`python3 scripts/viewer --check-connection` or `./run-viewer --check-connection`
from an extracted archive.

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
