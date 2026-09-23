# Farmer first-use readiness — resume

- Run: `run-02-sim`
- Roadmap hash: `824257d0572b0997fc24491b3524053451a3989cf89de2381dd3ea1cd0536485` (unchanged)
- Base HEAD: `dedcff1`; last implementation commit: `6dc4485`.
- Current batch: none; `farmer-day-zero-20260922` committed as `6dc4485`.
- User goal: free farmer application usable from day zero; monetization deferred.
- User explicitly authorized sub-agents; three lanes used. The implementation is committed; no publication was requested.
- Features: SUP-FD-INSTALL, ONBOARD, BUNDLE, DISPLAY, BROWSER, PDF tests passed;
  SUP-FD-VERIFY implemented, full release gates still blocked.
- Completed: safe local launch/private code/quality defaults; atomic field onboarding;
  catalog-backed authenticated imagery/trends; monitoring status; scouting;
  report evidence/geometry/PDF/download; geographic area correction; launcher archive.
- Validation: workspace check+format, 8 GIS acceptance, focused Rust/Node/shell checks,
  shared323, Geo Hub lib241, viewer60, real desktop/mobile browser + live Landsat,
  PDF visual review, database backup/restore passed.
- Full validation blockers: terrain test needs missing native protobuf dylib;
  strict Clippy fails on existing unchanged shared/soil_iot lints. Stale sharing
  audit assertion corrected and targeted rerun passed. All remaining GIS targets
  except the terrain native test passed.
- Distribution blocker: stable image absent; edge image is old amd64 build.
  Current image/build, clean-machine and ARM64/Windows verification not completed.
- Evidence and remaining limitations: `docs/reviews/farmer-day-zero-readiness.md`.
- Logs/test DB/PDF: `/tmp/agbot-farmer-day-zero/`; temporary server stopped.
- Preview launcher: `/tmp/agbot-farmer-release/agbot-v0.0.0-day-zero-preview.tar.gz`.
- Next: resolve validation blockers, build the current appliance, and verify a
  clean supported-platform installation before publication. Preserve changes;
  consult SQLite events and git diff before continuing.
