# Installing & operating AGBot

AGBot ships as one release artifact — the **geo_hub appliance image** — plus an
`agbot` CLI that runs it on a server and builds the native desktop apps on a Mac.

- **Linux server (headless):** hosts the geo_hub appliance = farmer portal (web)
  + web workspace/browse + APIs + satellite ingestion. No GUI.
- **Mac (ARM/Intel):** builds and runs the native desktop GUIs (`geo_viewer`,
  `flight_sim_cpp` viewer), pointed at the server over the network.
- **Any device (iPad / Android / desktop):** open the server URL and install the
  farmer PWA from the browser.

## One-line install

```sh
curl -fsSL https://raw.githubusercontent.com/vivasaayi/agbot/main/scripts/install.sh | sh
```

This clones the repo into `~/.agbot/repo` and links `agbot` onto your PATH.
Overrides: `AGBOT_HOME`, `AGBOT_REPO`, `AGBOT_REF`, `BIN_DIR`.

## Server role (Linux)

Requires Docker with the Compose plugin.

```sh
agbot up                 # pull the latest appliance image and start it
open http://localhost:8080/portal   # farmer PWA

agbot status             # container + /health status
agbot logs               # follow logs
agbot upgrade            # re-pull the newest image and restart (keeps data)
agbot down               # stop (named volumes retained)
```

State is persisted in two named Docker volumes so upgrades never lose data:

- `geo_hub_db`   → `/opt/agbot/db`   (SQLite database)
- `geo_hub_data` → `/opt/agbot/data` (ingested scenes / products)

### Disk retention

The pipeline regenerates monthly composites, climatologies, and phenology
products, superseding the prior version each time. By default the appliance
deletes a superseded product's artifact file as it is superseded
(`GEO_HUB__STORAGE__DELETE_SUPERSEDED_ARTIFACTS=true`) so rasters don't
accumulate unbounded on the data volume. Deletion is safety-guarded — only
files under the data root that no other product still references are removed.
Set `AGBOT_DELETE_SUPERSEDED=false` to keep every superseded artifact.

### Backup & restore

The `geo_hub` binary can snapshot and restore its SQLite database. Backup uses
`VACUUM INTO`, so it is a consistent snapshot safe to take while the server
runs; restore replaces the live file and should be done with the server stopped.

```sh
# Snapshot into the mounted db volume (safe while running):
docker compose -f docker-compose.prod.yml exec geo_hub \
  geo_hub backup /opt/agbot/db/backup-$(date +%F).db

# Restore from a snapshot (stop first, then start again):
agbot down
docker compose -f docker-compose.prod.yml run --rm geo_hub \
  geo_hub restore /opt/agbot/db/backup-2026-07-10.db
agbot up
```

Backup refuses to overwrite an existing destination; restore validates the
SQLite header before clobbering the live database and clears stale `-wal` /
`-shm` sidecars.

### Release channels

| Channel  | Image tag | Source                                        |
| -------- | --------- | --------------------------------------------- |
| `stable` | `:latest` | semver releases (tag push / workflow_dispatch)|
| `edge`   | `:edge`   | dated pre-release on every merge to `main`    |

```sh
agbot channel edge       # follow pre-releases
agbot upgrade --channel stable   # one-off upgrade on a specific channel
```

The image ref and port are configurable:

```sh
agbot config                          # show current settings
agbot config set port 9090            # expose on a different host port
agbot config set image_repo ghcr.io/<org>/geo-hub
```

### Admin access-code API (security)

The routes that mint, list, and revoke portal access codes
(`/api/admin/portal/access-codes*`) can forge a session for any account, so
they are gated by a static bearer token.

- **Disabled by default.** With no token configured the admin API returns
  `403 Forbidden` — fail-closed. First-run portal login is unaffected: the
  bootstrap access code is seeded directly into the database.
- **Enable it** by setting a strong random token before issuing codes to real
  users:

  ```sh
  export AGBOT_ADMIN_TOKEN="$(openssl rand -hex 32)"
  agbot up          # compose passes it through as GEO_HUB__SECURITY__ADMIN_TOKEN
  ```

  Then call the admin API with `Authorization: Bearer <token>`. A missing or
  wrong token is `401 Unauthorized`.

Treat the token as a secret (it is not baked into the image). Until a full
operator-auth layer lands, keep the server behind a private network or a
TLS-terminating reverse proxy.

### Require a session on the API

By default the `/api/*` surface is open (single-user / trusted-LAN loop). Once
the portal is reachable beyond a network you control, require a valid portal
session on every API call:

```sh
export AGBOT_REQUIRE_SESSION=true   # GEO_HUB__SECURITY__REQUIRE_SESSION
agbot up
```

Anonymous calls then get `401 Unauthorized`. Health checks, the access-code
login endpoint, the static app shell / PWA / browse assets, public share links,
and the admin API (which uses its own token) stay reachable. Note: Leaflet
raster-tile requests do not send an Authorization header, so map tiles under
`/api/**/tiles/**` require the session too — front the browse/workspace UIs with
an authenticating proxy if you enable the gate. Request bodies are capped at
64 MiB (`GEO_HUB__SECURITY__MAX_BODY_BYTES`).

### Rate limiting

A coarse per-client-IP cap blunts login brute-force and runaway clients:

```sh
export AGBOT_RATE_LIMIT_PER_MIN=120   # GEO_HUB__SECURITY__RATE_LIMIT_PER_MIN
agbot up
```

`0` (default) disables it. Over the cap → `429 Too Many Requests` with
`Retry-After`. It buckets by the peer IP, so put the reverse proxy in
`X-Forwarded-For`-preserving mode and terminate it close to the app; for
anything finer than a single 60-second fixed window, rate-limit at the proxy.

### TLS via a reverse proxy

geo_hub serves plain HTTP and does not terminate TLS itself. In any exposed
deployment, front it with a TLS-terminating reverse proxy (Caddy, nginx,
Traefik) that:

- terminates HTTPS and proxies to `http://127.0.0.1:8080`,
- forwards the real client IP (`X-Forwarded-For`) so rate-limit buckets and
  logs are meaningful,
- optionally adds its own auth in front of the browse/workspace tile URLs that
  the in-app session gate cannot cover (see the tile caveat above).

Example Caddyfile:

```
agbot.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

### Logging

The appliance emits **structured JSON logs** by default
(`GEO_HUB__OBSERVABILITY__LOG_FORMAT=json`) — one JSON object per record, ready
for a log aggregator. `RUST_LOG` sets levels (default `info`) in either format.
For human-readable local runs set `AGBOT_LOG_FORMAT=text`.

### Health & readiness probes

Two endpoints, both public:

- `GET /health` — **liveness**: the process is up. Dependency-free, so it never
  flaps on a transient database hiccup. The container healthcheck uses this.
- `GET /ready` (alias `GET /readyz`) — **readiness**: returns `200 ready` only
  when the SQLite pool answers a query, else `503`. Point an orchestrator's
  readiness/traffic-gate at this so requests aren't routed before the database
  is reachable.

## Mac role (desktop GUIs)

Requires `just`, a Rust toolchain, and (for the sim) a C++/CMake toolchain.
Point the CLI at your server, then build+run locally:

```sh
agbot config set geo_hub_url http://<server-host>:8080
agbot viewer             # builds geo_viewer (release, self-contained) and runs it
agbot sim                # builds flight_sim_cpp and runs it headless with defaults
```

`agbot viewer` exports `GEO_HUB_URL` so the Bevy client connects to the server.
The desktop apps are **built locally on the Macs** — they are intentionally not
shipped through CI (which is Linux-only).

## The release loop

1. Merge to `main` → GitHub Actions builds and pushes the geo_hub image and cuts
   a **dated pre-release** (`vYYYY.MM.DD-<sha>`, `:edge`).
2. Promote with a semver tag (`git tag v1.2.3 && git push --tags`) or a manual
   `workflow_dispatch` → `:latest` + a full GitHub release.
3. On the server, `agbot upgrade` pulls the new image and restarts, preserving
   the data volumes.
4. On a Mac, `agbot viewer` / `agbot sim` build the native apps against the
   server.
