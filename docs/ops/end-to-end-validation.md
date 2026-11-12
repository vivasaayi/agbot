# Hardware-free end-to-end acceptance

Run from the repository root (or the `codex/end-to-end` worktree). Requires Rust
1.93.1, CMake 3.24+, a C++20 compiler, Python 3, Node, and Playwright Chromium.
The runner limits builds to two jobs by default and uses a fresh temporary
runtime directory. Existing project containers, ports, and databases are not used.

Install browser tools in a separate directory, then run:

```sh
npm install --prefix /tmp/agbot-browser-tools playwright
PLAYWRIGHT_BROWSERS_PATH=/tmp/agbot-browser-tools/browsers \
  /tmp/agbot-browser-tools/node_modules/.bin/playwright install chromium
PLAYWRIGHT_MODULE=/tmp/agbot-browser-tools/node_modules/playwright \
PLAYWRIGHT_BROWSERS_PATH=/tmp/agbot-browser-tools/browsers \
  bash scripts/verify-e2e.sh
```

`CARGO_BIN`, `CMAKE_BIN`, and `CTEST_BIN` can point to isolated tool installations.
`AGBOT_BUILD_JOBS` overrides build concurrency. The runner prints the artifact
folder and retains source scans, maps, simulator evidence, and SQLite catalog.
It exits on the first failed stage. Each browser fixture binds to `127.0.0.1:0`,
so the OS chooses an available port, and stops its listener afterward.

The run covers:

- Canonical C++ simulator build and regression tests, then a seeded sample mission.
- Real LiDAR JSONL to collector-compatible files and local mapper products.
- World-space returns to geographic floating-point elevation GeoTIFF.
- Catalog registration with existing source lineage, plus geographic web tiling.
- Actual simulator raster selection/display in the authenticated workspace,
  with retained screenshots. The catalog layer is labeled as simulation.
- Landsat fixture discovery/derivation and advisor processing using real domain
  code; external source fetching uses local fixture rasters.
- Desktop and mobile farmer display of authenticated NDVI tiles, findings,
  historical trends, report generation and actual PDF download.
- Field onboarding and database backup/restore integration checks.
- Real browser field drawing/import and scouting, followed by stopping and
  restarting the native server and verifying the raster, field, and note remain.

The simulator origin in this recipe is read from its sample mission's home.
The geographic elevation product preserves simulator-local vertical coordinates;
it does not claim a real-world vertical datum. Simulation products and the
Landsat farmer fixture are separate evidence paths; the runner does not establish
that simulated LiDAR is a vegetation observation. Real hardware, live satellite
providers, clean-machine packaging, and hosted tenant isolation require separate
acceptance.

## Start an isolated native portal

After building `cargo build --locked -p geo_hub`, use a new data directory and
private access code. Keep the directory to retain the installation:
Run this example in Bash (`bash`), since its private prompt uses Bash's `read` options.

```sh
agbot_state=$(mktemp -d /tmp/agbot-portal.XXXXXX)
read -r -s -p 'Choose a private access code: ' agbot_code; echo
GEO_HUB__DATABASE_URL="sqlite://$agbot_state/hub.db?mode=rwc" \
GEO_HUB__DATA_ROOT="$agbot_state/products" \
GEO_HUB__BIND_ADDRESS=127.0.0.1:0 \
GEO_HUB__BOOTSTRAP__ACCESS_CODE="$agbot_code" \
GEO_HUB__BOOTSTRAP__SEED_DEMO=false \
GEO_HUB__SECURITY__REQUIRE_SESSION=true \
  cargo run --locked -p geo_hub
```

Use the actual address printed by `geo_hub listening`, followed by `/portal/`.
Stop with Ctrl-C. Restart with the same database/data directory and original
access code. Monitoring uses configured providers; this native startup command
alone does not configure or verify provider access. The acceptance runner uses
its own disposable, deterministic fixtures instead.
