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

For another host, transfer the exact image through a registry or Docker
save/load, verify its identity and architecture, then deploy there. Keep the
previous image and backups for recovery; rolling back an image does not undo a
database migration. Both endpoints bind to loopback by default. Remote access
requires a separately configured and secured access path; do not expose the
service publicly merely by changing the bind address.

For access from another device on the same tailnet, Tailscale Serve can provide
HTTPS while the appliance stays on loopback. First check `tailscale serve status`
for existing services and the tailnet's access policy. The Mac Studio checked
on 2026-11-12 reported that Serve is disabled on the tailnet; an administrator
must enable it before this path works. For example, on the Mac Studio, after
staging is running and verified (use the Tailscale app's CLI path if it is not
on `PATH`):

```sh
tailscale serve --bg --https=20080 http://127.0.0.1:20080
tailscale serve status
```

Open the HTTPS address reported by Tailscale from an authorized tailnet device,
then sign in through the portal. This is not automatically enabled by the
adapter; remote access and the tailnet policy must be verified on the target
devices. Avoid `tailscale serve reset`, which removes other services too.

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
