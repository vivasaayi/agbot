# Farmer day-zero release gate

The release goal is a free application a farmer can install and use without an
agronomist or developer setting up their fields. Monetization is outside this
release. The existing GIS server, satellite worker, and farmer portal are the
implementation foundations; a separate viewer is not required for first use.

## Required journey

1. Install on a supported computer, start the application, and find the local
   browser address and private sign-in code. No Rust build or database editing.
2. Add a farm and field by drawing a boundary or importing a GeoJSON polygon;
   enter a field name, crop, and season. Ownership comes from the signed-in user.
3. Start satellite monitoring automatically. Show whether work is waiting,
   running, disabled, failed, or finished without suitable imagery.
4. Display an accepted, georeferenced vegetation layer with its acquisition
   date and quality information. Cloudy, missing, stale, or insufficient data
   must not become a healthy-field claim. Vegetation indices are observations,
   not diagnoses or treatment prescriptions.
5. Review trends and findings, record scouting notes, and download a report
   while signed in. Keep evidence linked to the right field and observation.
6. Restart and upgrade without losing fields, notes, or imagery. Export and
   restore a backup using documented instructions.

A sample farm, if offered, must be explicitly labeled and separate from real
farms. Immediate value can come from usable historical imagery; new satellite
acquisitions and cloud-free coverage cannot be promised for the first day.

## Acceptance environments

- Fresh installation and empty database, with authentication enabled.
- Existing installation with saved data and the original access code.
- Desktop and narrow/mobile browser layouts.
- Provider unavailable, no matching imagery, all-cloud imagery, expired session,
  interrupted processing, and restart while jobs are queued.
- Real Landsat processing through provider discovery, range reads, quality
  masks, field statistics, catalog publication, and farmer display.

API fixtures establish deterministic behavior. They do not replace an installed
application test or live imagery acceptance. Multi-tenant hosted deployment is a
separate gate; a session requirement alone does not establish tenant isolation.

## Current work and evidence

Implementation and validation for the 2026-09-22 batch are tracked in
`.codex/checkpoints/roadmap-run/checkpoint.sqlite` and `RESUME.md`.

Known remaining release concerns from the initial review:

- No farmer-facing standalone application download has been verified. The
  current delivery uses a repository installer and Docker appliance.
- The portal originally relied on an agronomist for field creation.
- Authenticated imagery and report downloads need browser verification.
- Catalog satellite products and legacy scene products are distinct; the
  farmer view must consume accepted catalog output rather than assume a legacy
  scene row exists.
- Live-provider coverage, restart recovery, backup/restore, and clean-machine
  installation still need release acceptance.
- Broad GIS/STAC tenant isolation has not been established. Keep the local
  appliance bound to loopback by default.

Do not mark the product farmer-ready until the full required journey passes on
a supported fresh installation and the results above are recorded.

## Implemented in the working tree (2026-09-22)

- Self-service field setup: draw or import a polygon, create/reuse an owned farm,
  and atomically save the field, Landsat subscription, and discovery job.
- Local startup: working installed CLI symlinks, private persisted first-login
  code, loopback binding, worker enabled, demo disabled, quality gate preserved.
- Farmer display: accepted field-scoped catalog imagery, authenticated tiles,
  acquisition date, pixel size and quality coverage, durable monitoring state,
  readable time-series chart, scouting notes, and authenticated report download.
- Report evidence is field/observation/quality scoped. Nominal observations are
  not converted into anomaly diagnoses; missing zone geometry is represented
  only by a validated whole-field boundary. PDFs draw a labeled schematic
  boundary, wrap text, and paginate rather than printing SVG markup.
- Satellite application area uses the validated geographic field boundary;
  geographic degrees are no longer treated as square metres.
- Release packaging produces a checksum-bearing launcher archive without a Git
  or Rust prerequisite. Docker and a published application image are still needed.
- Installer updates preserve local changes and refuse divergent history.

## Repeatable validation

Run the deterministic checks from the workspace root:

```sh
bash scripts/verify-delivery-pipeline.sh
node --test geo_hub/tests/js/portal_requests.test.mjs
cargo test -p geo_hub --test portal_onboarding --test portal_farms --test portal_reports --test portal_static --test landsat_advisor_pipeline
cargo test -p post_processor grower_report --lib
just gis-acceptance
```

The browser smoke creates test fields, subscriptions, and a scouting note. Use a
**disposable** server with authentication enabled and install Playwright/Chromium:

```sh
AGBOT_TEST_DISPOSABLE=1 \
AGBOT_TEST_URL=http://127.0.0.1:18080 \
AGBOT_TEST_ACCESS_CODE=your-test-code \
node scripts/test-farmer-browser.cjs
```

`PLAYWRIGHT_MODULE` can point to an existing Playwright installation. The smoke
checks drawing/import, mobile width, navigation, persisted scouting, and revoked
sessions. Real accepted imagery and PDFs also require a populated test field;
the Landsat integration fixture covers derivation through authenticated display
and download without substituting fake domain calculations.

## Release still blocked

As checked on 2026-09-22, `ghcr.io/vivasaayi/geo-hub:latest` returns
`manifest unknown`; `:edge` is an amd64 image from July 10 at revision `c512e066`.
Neither contains this working-tree implementation. Build, publish, and verify the
new appliance and attached launcher on a clean supported computer before telling
farmers to download it. Native ARM64/Apple Silicon and Windows delivery have not
been verified. Local cached older ARM64 builds are not evidence for this release.

Live processing also encountered a missing Landsat mirror QA asset (HTTP 404 for
an August 3 observation). Other imagery processed; failed work remains visible.
Broader regions, seasons, no-imagery cases, and intermittent connectivity still
need release acceptance. Existing persisted findings are not rewritten by the
area correction; historical outputs need deliberate regeneration if relied on.
The PDF uses standard ASCII fonts; arbitrary-script field names remain a known
limitation. Hosted multi-tenant isolation and advanced `/browse` authentication
are outside the verified farmer-portal flow.

## Validation results for this batch

Passed:

- `cargo check`; `cargo fmt --all -- --check`; `git diff --check`.
- `just gis-acceptance`: 8 acceptance tests, including first-field setup and
  real Landsat fixture processing through authenticated imagery/report delivery.
- Focused onboarding, portal, Landsat, monitoring/area, PDF, and Node request tests.
- `cargo test -p geo_viewer`: 60 passed; shared library: 323 passed;
  Geo Hub library: 241 passed, 1 pre-existing ignored.
- Delivery/installer/bundle behavioral tests and appliance contract checks.
- Fresh Chromium desktop and 390px mobile sessions: drawing/import, existing
  farm choice, saved scouting notes across reload, revoked-session redirect,
  repeated refresh, actual georeferenced tiles, PDF generation and download.
  No uncaught browser errors; no horizontal overflow.
- Live Landsat on the first Iowa test field: 14 scenes discovered, 7 usable NDVI
  products accepted, 6 rejected by quality checks, 1 missing provider QA asset.
  The view selected September 4 usable imagery and showed its 98% processed-image
  coverage and approximately 30m pixel size. Those are test-field observations,
  not a regional agronomic validation.
- Process restart retained fields/notes; native database backup and restore
  matched farm, field, activity, product, and report counts with integrity `ok`.
  This does not replace a clean-machine full volume restoration test.
- The actual browser-downloaded one-page PDF and five fixture pages were
  rendered with Poppler and visually reviewed for boundary geometry and layout.

Full regression is **not green**:

- `just gis-test` first exposed a stale report-sharing test expecting three
  audit records. It now verifies all four events, including denied access after
  revocation, and passes. All other tests in that invocation passed. The
  remaining integration targets were then run explicitly.
- `cargo test -p geo_hub --test terrain_derive`: 5 pass, 1 fails because the local
  ONNX runtime references missing Homebrew `libprotobuf-lite.35.1.0.dylib`.
  All other remaining integration targets pass.
- `cargo clippy -- -D warnings` fails on existing lint errors in unchanged
  `shared` and `soil_iot` code. No unrelated lint cleanup was performed.

Browser evidence, test logs, the disposable database, and the downloaded test PDF
are under `/tmp/agbot-farmer-day-zero/` on the validation machine. The temporary
server was stopped after testing. No release was published and no commit made.
