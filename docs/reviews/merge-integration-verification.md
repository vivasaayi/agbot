# End-to-end branch integration review

This is an uncommitted merge prepared in the isolated `integration-review`
worktree. It combines `main` at `f26b780` with `codex/end-to-end` at `63712df`.
Neither the source worktree nor the deployed staging installation was changed.

The merge had conflicts in workspace `api.js` and `map.js`. The resolution
keeps authenticated scene and catalog tiles, the end-to-end branch's catalog
layer selection and footprint focus, and its removal of an opaque map overlay.
An updated JavaScript test covers authenticated requests for both tile types,
the initial map without an opaque rectangle, and catalog footprint focus.

Verification on 2026-11-12:

- `node --test geo_hub/tests/js/*.test.mjs` passed six tests; JavaScript syntax
  and `git diff --check` passed.
- In an isolated Linux builder with the merged source mounted, the Geo Hub
  `workspace_static`, `portal_onboarding`, `lidar_catalog_pipeline`, and
  `landsat_advisor_pipeline` integration targets passed. The
  `data_collector` `sim_lidar_bridge` target and `lidar_mapper` library tests
  also passed. Browser-only portions of the Landsat target were not enabled in
  that Rust run.
- A disposable authenticated Geo Hub served a generated, labeled simulation
  GeoTIFF registered through the real catalog API. Chromium loaded the merged
  workspace, selected the catalog layer, received a tile with the bearer
  session, and verified nontransparent rendered pixels and no page errors.
  This browser check used the `f26b780` Geo Hub server binary with merged web
  assets mounted read-only; it did not exercise the branch's server changes.

- `scripts/verify-e2e.sh` passed on the Mac with a fresh disposable state directory. The C++ suite had no failures and one skipped demo check; the simulator generated 430 LiDAR scans, the collector exported 428 scans with two empty, and Geo Hub registered 431 products. Browser checks covered farmer onboarding, scouting, an authenticated catalog raster, and persistence across restart. Five Rust integration targets passed 17 tests. Artifacts are in `/var/folders/0z/w3y6h7fx77jf8kbq2l40gj2c0000gn/T/agbot-e2e.8L5dWa`.
- `cargo check --locked` passed across the workspace on macOS with the isolated Rust toolchain.
- The first full run stopped because an inherited `RUST_LOG=warn` hid the server's startup log. The workspace browser harness now sets `RUST_LOG=geo_hub=info` for its child server; a fresh full run passed with the original inherited environment.

The branch supplies explicit migration and sample-data commands, but they are
not yet wired into the shared environment hooks: those hooks run before Compose
replaces an existing service, so an upgrade could migrate a live SQLite database.
The existing startup migration behavior is retained pending a safe lifecycle.
At the time of verification, the merge was uncommitted and had not been deployed.

## Combined viewer and release guard review

The then-uncommitted `feature/viewer-auth` and `feature/release-integrity` changes
were also applied to this isolated review worktree. The combined checkout
passed a workspace-wide `cargo check --locked`, 64 native macOS `geo_viewer`
tests, five viewer launcher tests, eight
release guard tests, and six workspace JavaScript tests. An existing packaged
macOS viewer binary from the viewer feature authenticated through the combined
launcher to a disposable Geo Hub server built from the merged server source;
its headless connection check succeeded. The package was built from the viewer
feature worktree, not from a clean combined revision. The Mac desktop was
locked, so no graphical viewer behavior was verified. At the time of this check, these additions were uncommitted review changes.
Neither staging nor the source worktrees were modified by the integration check.
