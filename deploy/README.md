# AGBot environments

The Geo Hub appliance packages the farmer portal (`/portal`), remote web
workspace (`/workspace`), browse UI (`/browse`), APIs, and satellite worker in
one image. Rust crates are code boundaries, not individually deployed services.
Keep the monorepo while these surfaces share one runtime. `geo_viewer` is a
separate native Bevy application; the standalone mission-planner UI,
flight/sensor services and C++ simulator are not included in this appliance. They need their own
packaging and acceptance before claiming a full-platform deployment.

Geo Hub links shared, alerting, imagery, post-processing, interop,
orthomosaic, crop intelligence, copilot, compliance, fleet health, soil IoT,
time series, provenance, mission-planner, plugin SDK and raster I/O code as
libraries. Their presence in the image does not mean every feature is complete
or separately operable. Mission control, multi-drone control, sensor/data
collection, LiDAR, overlays, and the ground station have separate runtime or
hardware needs; inventory and validate those as additional deployment slices.

| Package boundary | Modules | Deployment status |
| --- | --- | --- |
| Geo Hub web appliance | `geo_hub`, `shared`, `imagery_processor`, `raster_io`, `post_processor`, `alerting`, `orthomosaic`, `crop_intelligence`, `copilot`, `compliance`, `fleet_health`, `soil_iot`, `timeseries`, `provenance`, `plugin_sdk`, `interop`, `mission_planner` | One server image with linked libraries, web assets, SQLite and artifact volumes; these libraries are not independently deployed services. |
| Desktop GIS | `geo_viewer`, `sensor_overlay_engine` | Native Bevy application with a portal-session launcher and separate Mac package; verify the target Mac and staging workflow before promotion. |
| Edge and flight | `mission_control`, `sensor_collector`, `imagery_processor`, `lidar_mapper`, `ground_station_ui` | The existing Dockerfile has a separate edge target; this environment adapter does not deploy or validate it. Hardware, serial access and flight safety require their own tests. |
| Mission UI and coordination | `mission_planner` frontend, `multi_drone_control`, `data_collector` | Outside this web appliance's deployed service boundary; identify the intended runtime and dependencies before adding them. |
| Simulation | `flight_sim_cpp` | Native C++ simulator, separate from the web appliance. |

Module membership in an image is not a claim that all module functionality is
complete or that hardware integrations have passed acceptance.

The default branch is `main`. Merge each reviewed feature into `main`, then
build staging from a clean checkout of that branch. This adapter follows the
organization contract in `~/work/.org/environments.md`. Install Docker Desktop
and the shared checkout on each machine. Do not merge or destroy another active
worktree as part of deployment.

## Feature worktree

Create one branch and worktree for each independent feature, then work and
test from that directory. For example, from the main checkout:

```sh
git worktree add .worktrees/field-filter -b feature/field-filter main
cd .worktrees/field-filter
```

From the feature worktree:

```sh
bash scripts/env worktree plan
bash scripts/env worktree up
bash scripts/env worktree status
bash scripts/test unit
bash scripts/test integration
bash scripts/test ui
bash scripts/env worktree stop
```

For a disposable server, the authenticated browser checks create a test field
and scouting note. Install Playwright and Chromium, then run:

```sh
AGBOT_TEST_DISPOSABLE=1 AGBOT_TEST_URL=http://127.0.0.1:PORT \
  AGBOT_TEST_ACCESS_CODE=YOUR_TEST_CODE bash scripts/test browser
```

If headless Chromium cannot create a WebGL context, add
`AGBOT_TEST_SOFTWARE_WEBGL=1` to use its software renderer for the workspace
and browse check.

Sign in at `/portal/` before opening `/workspace/` or `/browse`; those web
surfaces use the portal session for local API and map requests. The browse
map also depends on the external MapLibre CDN and OpenStreetMap service.

Docker assigns a loopback port; `up` prints the address. The port can change
when the container restarts, so check `bash scripts/env worktree status` again.
Each worktree has its
own SQLite and artifact volumes. To remove disposable data after work is done,
run `bash scripts/env worktree destroy` from that worktree before removing the
Git worktree. This permanently deletes that worktree's volumes.
After review and verification, merge the individual feature branch into
`main` from a clean main checkout. Build staging only after the merge, so its
image represents the combined revision rather than an individual worktree.

## Staging and promotion

After the adapter and feature changes have been merged, use a clean `main`
checkout on the Mac Studio:

```sh
bash scripts/build staging
bash scripts/deploy staging
bash scripts/test staging
docker image inspect "agbot:$(git rev-parse HEAD)" --format '{{.Id}} {{.Os}}/{{.Architecture}} {{index .Config.Labels "org.opencontainers.image.revision"}}'
```

The build records the clean source revision and immutable image ID in the
private staging state directory. A default staging deploy checks that receipt,
the image's revision label, and the current `main` revision still agree. If the
build failed, the tag was replaced, or `main` advanced, rebuild before the
default deploy. Starting a new build invalidates the previous receipt even if
the new build fails; the currently running service is not stopped. The deploy uses the verified image ID, so replacing the tag
between verification and startup cannot switch the artifact. An existing installation without a receipt also needs one build
with this adapter; its currently running service and volumes are preserved.
Passing `--image` explicitly is reserved for deliberate deployment of an already
tested artifact, such as rollback; the operator must verify its provenance.
Run `python3 scripts/test-release-integrity.py` to check the guard behavior.

Staging is at `http://127.0.0.1:20080`. The staging smoke checks route responses;
it does not replace the authenticated farmer journey, real imagery, or backup
and restore acceptance. Record the commit, image ID, platform, test results,
and schema state before promotion. Startup applies the application's schema
initialization; there is no separately configured migration or demo seed hook.
Back up both SQLite and artifact volumes before an upgrade, and verify restore.

For a consistent cold staging backup, stop the service and archive both
project-scoped volumes into a new private directory on the Mac:

```sh
BACKUP_DIR="$HOME/agbot-backups/staging-$(date +%Y%m%d-%H%M%S)"
mkdir -m 700 -p "$BACKUP_DIR"
bash scripts/env staging stop
docker run --rm -v agbot-staging_database:/data:ro -v "$BACKUP_DIR:/backup" \
  debian:12-slim tar -C /data -czf /backup/database.tar.gz .
docker run --rm -v agbot-staging_artifacts:/data:ro -v "$BACKUP_DIR:/backup" \
  debian:12-slim tar -C /data -czf /backup/artifacts.tar.gz .
bash scripts/deploy staging
```

Check each command succeeds; if backup fails, restart staging and investigate
before upgrading. Back up the private environment file separately. Test restore
into **new disposable volumes** before relying on a backup. These archives are
not encrypted and need private storage. Production has different volume names;
do not substitute them into a restore command without a deliberate recovery
procedure. A 2026-11-12 disposable Docker check restored both a field and a
raster artifact into separate volumes; a clean-machine restore remains to be
verified.

Production uses a separate persistent database and data volume at loopback port
30080. Deploy the exact image ID tested in staging, available on the target
Docker host:

```sh
bash scripts/deploy production --image sha256:TESTED_IMAGE_ID
```

Production deploys require a full `sha256:` image ID or a registry reference
with an `@sha256:` digest. Mutable tags such as `latest` are rejected. Pinning
prevents later tag changes; the operator must still verify that this exact
platform artifact passed staging acceptance before promotion.

For another host, transfer the exact image through a registry or Docker
save/load, verify its identity and architecture, then deploy there. Keep the
previous image and backups for recovery; rolling back an image does not undo a
database migration. Both endpoints bind to loopback by default. Remote access
requires a separately configured and secured access path; do not expose the
service publicly merely by changing the bind address.

For access from another device on the same tailnet, Tailscale Serve can proxy
the appliance while it stays on loopback. Check `tailscale serve status` and
the tailnet's access policy before changing a listener. On 2026-11-12, the Mac
Studio's HTTP listener on port 20080 worked within the tailnet, but HTTPS
required an administrator to enable the tailnet setting. For example, with
staging running (use the Tailscale app's CLI path if it is not on `PATH`):

```sh
tailscale serve --bg --http=20080 http://127.0.0.1:20080
tailscale serve status
```

Tailscale encrypts traffic between tailnet devices, but this is still an HTTP
browser origin: browser features that require a secure context may not work,
and the native viewer refuses non-loopback HTTP. The HTTP portal and workspace
were checked from the Mac Studio; access from a second device still needs a
check. Prefer HTTPS once the tailnet administrator has enabled it. To add HTTPS
without taking over the existing HTTP port, use a separate port:

```sh
tailscale serve --bg --https=20443 http://127.0.0.1:20080
tailscale serve status
```

Open the HTTPS address reported by Tailscale from an authorized tailnet device,
then sign in through the portal and verify the viewer there. Once HTTPS works,
remove only the old listener with `tailscale serve --http=20080 off` if it is
no longer needed. Neither listener is managed by the product adapter. Avoid
`tailscale serve reset`, which removes other services too.

Credentials are generated per environment in the shared launcher's private
`~/.local/share/work-environments/<project>/environment.env` (or
`WORK_ENV_STATE_ROOT`). Its `POSTGRES_PASSWORD` field is used as the Geo Hub
bootstrap access code even though Geo Hub uses SQLite; the name comes from the
shared launcher. Protect the file and container logs: the current bootstrap
logs the initial access code. Existing appliance volumes are not imported.
For a newly created staging environment, show its code locally with:

```sh
awk -F= '$1 == "POSTGRES_PASSWORD" {print $2}' \
  "${WORK_ENV_STATE_ROOT:-$HOME/.local/share/work-environments}/agbot-staging/environment.env"
```

Back up this private environment file separately from the data volumes. The
bootstrap code is seeded on first initialization; replacing the file later does
not rotate the existing database's access code.

The native viewer can be configured with `GEO_HUB_URL` as described in
`geo_viewer/README.md`. Its launcher obtains a portal session, and
`scripts/package-viewer` produces a separate Mac archive. Build and test a
clean staging archive on the target-compatible Mac before promoting it.
