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
