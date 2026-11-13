# Hardware-free end-to-end verification — 2026-11-12

The requested local hardware-free workflow passes on this Apple Silicon Mac.
This records acceptance of the simulator/GIS workflow and the farmer satellite
fixture workflow, not release readiness or live flight/provider acceptance.

## Requirement audit

| Requirement | Current evidence |
| --- | --- |
| Isolated worktree | `codex/end-to-end` under `.worktrees/end-to-end`; changes uncommitted; main tracked files unchanged (the requested `.worktrees/` directory is untracked there). |
| Mission → simulator capture | Canonical C++ headless executable completed the sample field-loop mission with seed 42; 430 scans and retained telemetry/manifest/validation. |
| Capture → processing | Collector bridge exported 428 hit scans and accounted for 2 no-hit scans; mapper completed the full dataset. World-return processing preserved spatial coordinates and elevation in a floating GeoTIFF. |
| Processing → catalog | Local map registered 429 products; geographic map registered source and derived product, with 0 failed registrations. Input records resolve to content-bearing source files. |
| Catalog → interface | Actual mission-derived GeoTIFF selected in authenticated workspace; browser verified loaded, nontransparent raster pixels, and screenshots were visually inspected. An opaque overlay that hid imagery was removed. |
| Farmer setup | Browser verified sign-in, boundary draw/import validation, field setup, farm reuse, scouting, mobile width, and revoked-session handling. |
| Imagery/findings/trends/report | Real Landsat processing over local source-raster fixtures; named advisor jobs completed with findings. Chromium verified authenticated NDVI tiles, findings, historical trend SVG, generation and PDF download at widths 1280 and 390. |
| Restart persistence | Native server stopped and restarted on a new OS-assigned port; original code, catalog raster, saved field and scouting note verified in the browser. |
| Backup/restore | Three database backup/restore integration tests passed. Imagery source directories are retained separately; database snapshots alone are not a complete data-volume backup. |
| Repeatable commands | `bash scripts/verify-e2e.sh` passed from a fresh runtime directory. Prerequisites, overrides, startup and limitations are documented in `docs/ops/end-to-end-validation.md`. |
| Port/project isolation | All acceptance listeners bound to loopback port 0, all fixture state isolated, server children stopped; no Geo Hub listener remained after acceptance. |

## Final execution evidence

- Log: `/private/tmp/agbot-e2e-final-run.log`.
- Retained artifacts: `/var/folders/0z/w3y6h7fx77jf8kbq2l40gj2c0000gn/T/agbot-e2e.qjBAf7`.
- Workspace compile: `/private/tmp/agbot-e2e-final-check.log` (passed).
- C++ regression: 21 passed, 1 optional world-demo check skipped, 0 failed.
- Runner Rust suites: backup/restore 3, Landsat/advisor 3, LiDAR catalog 2,
  onboarding 3, workspace routes/static 6; all passed, including browser stages.
- Earlier focused collector/mapper suites passed (57 collector, 41 mapper,
  3 bridge tests); source-neighbour equivalence and invalid-input checks included.
- `git diff --check` passed. No commit, publication, deployment, or hardware
  command was performed.

## Explicit limits

The simulator's geographic conversion uses its mission-home origin and local
world convention. The runner reads the origin from the same mission file;
the standalone mapper CLI requires the operator to supply that origin correctly.
Elevation values retain simulator-local y; an absolute vertical datum is not
asserted. The simulator mission and farmer Landsat fixture are separate evidence
paths: simulated elevation is not presented as a vegetation observation.

Real hardware, live providers, clean-machine packaging and hosted tenant
isolation remain separate acceptance gates. These results do not supersede the
farmer release-readiness gate in `farmer-day-zero-readiness.md`. No production
or farmer-ready release claim is made.
