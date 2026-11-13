# Environment adoption evidence

Checked on the Mac Studio on 2026-11-12. These are disposable validation results,
not a release or production deployment.

- The ARM64 appliance image built successfully from the development checkout.
- Two temporary Git worktrees used `scripts/env worktree up`, received distinct
  Compose projects, credentials and host ports, and stored different fields.
  Destroying one left the other and its field available.
- A staging-shaped stack retained a field across a stop and redeploy with the
  same image. Disposable staging and production stacks both ran the same local
  image ID with separate volumes.
- Cold archives of the database and artifact volumes were restored into fresh
  disposable volumes; a field and raster artifact survived.
- The farmer browser smoke passed first-field onboarding and scouting; a
  signed-in browser loaded workspace and browse APIs and a real workspace tile
  without forwarding a session to an external origin. `scripts/test ui` passed.
- In a Linux builder, `cargo test --locked -p geo_hub --test workspace_static`
  passed six tests; targeted acceptance tests across `products_api`,
  `portal_onboarding`, and `landsat_advisor_pipeline` passed eight tests.
  `cargo test --locked -p shared --lib` passed 323 tests and
  `cargo test --locked -p geo_viewer` passed 60 tests. These do not establish a
  native Mac viewer build or authenticated viewer workflow.

Still required for a release: build a clean `main` revision containing these
changes, exercise its authenticated workflows and data recovery on the
actual staging install, record the image and acceptance evidence, and promote
the same artifact. Native `geo_viewer` authentication and packaging are not
implemented. The Mac Studio's Tailscale Serve reported that it is disabled on
the tailnet, so remote HTTPS was not verified. Existing appliance data was not
imported. At that checkpoint, no stable deployment or production promotion had
been performed.

Staging was subsequently deployed on the Mac Studio at `127.0.0.1:20080`
from clean `main` commit `f26b780cea878fc546e0b567ff4615d6a01349e9`. Its
`linux/arm64` image ID was
`sha256:6bd3afd0422a1edf2db97537689e3317dc14863f7187ccabc174a2584b111c28`.
Health, portal authentication, and browser checks against a disposable instance
of that exact image passed. The operator record is kept in the private staging
environment state directory. Production promotion has not been performed.

## Native viewer feature worktree

Subsequent work in `feature/viewer-auth` added origin-scoped portal sessions,
a sign-in launcher, and a separate Mac package. The Linux viewer suite passed
64 tests, the launcher passed five Python tests, and an ARM64 macOS release
binary built successfully. The macOS release test suite also passed 64 tests.
The extracted native binary's headless connection check authenticated to live
staging through the packaged launcher, validated the identity response, and
rejected redirects, unauthorized responses, and malformed identity data in
tests. This was
a worktree artifact at the time of testing, not a clean `main` release. The Mac desktop
was locked, so the actual viewer window and rendering have not yet been checked.
Tailscale Serve remained disabled during this verification.
