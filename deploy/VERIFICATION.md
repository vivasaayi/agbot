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
imported. No stable deployment or production promotion was performed.
