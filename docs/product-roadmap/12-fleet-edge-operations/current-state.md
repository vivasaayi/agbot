# Fleet and Edge Operations: Current State and Target State

## Mission

Keep AGBot deployable, observable, and operable in the field at fleet scale. Enroll drones and edge nodes with stable identities, track their health, distribute config and software safely, and run reliably on Jetson/Raspberry Pi-class hardware in both simulation and flight modes. This is the operability backbone under every other domain.

## Current Maturity

early partial: configuration, runtime modes, structured logging, container build, and ARM cross-compile exist and work, but deployment is documented rather than productized. There is no device registry, no fleet health, no OTA, and no centralized observability beyond logs.

## What Exists Now

- One config model, `AgroConfig`, loading runtime mode plus MAVLink/LiDAR/camera/storage/server/GPS/processing settings from env (and `.env` via `dotenvy`) with sane defaults (`shared/src/config.rs`).
- `RuntimeMode::Simulation | Flight` parsed from `RUNTIME_MODE`, defaulting to Simulation (`shared/src/lib.rs`).
- `init_logging` wiring `tracing_subscriber` with `RUST_LOG`/`EnvFilter` for structured logs across the workspace (`shared/src/lib.rs`).
- Multi-stage `Dockerfile` building `mission_control`, `sensor_collector`, `imagery_processor`, `lidar_mapper`, and `ground_station_ui` into a slim runtime image with a non-root user and exposed ports 3000/8080/8081.
- `docker-compose.yml` for Postgres, a test Postgres, and pgAdmin with healthchecks and a shared network.
- `Cross.toml` + `justfile` `arm`/`arm64` recipes cross-compiling for `aarch64-unknown-linux-gnu` (Jetson) and `armv7-unknown-linux-gnueabihf` (Raspberry Pi).
- `dev-start.sh` building and launching the simulation stack locally; `demo-terrain.sh` for a demo flow; systemd service examples documented in the repo README.

## Gaps to Close

- No drone/device enrollment or registry: nodes have no stable identity, capability record, or ownership linkage.
- No fleet health or maintenance tracking: no heartbeat, uptime, version, or component-status reporting.
- No config or software distribution (OTA): config is per-node env/TOML, edited by hand.
- No centralized observability: only local `tracing` logs, no metrics, distributed tracing, or aggregation.
- No alerting on node-down, low-battery-fleet-wide, disk-full, or processing-stall conditions.
- No secrets management: DB passwords and tokens live in plaintext compose/env.
- No edge resource budgeting: no CPU/memory/disk/thermal limits or backpressure for Jetson/Pi-class nodes.
- No deployment tests or rollout/rollback discipline.

## Source Modules Reviewed

- `shared/src/config.rs` (`AgroConfig` and sub-configs, `load()`)
- `shared/src/lib.rs` (`RuntimeMode`, `init_logging`, `AgroResult`)
- `Dockerfile` (multi-stage build, runtime image, exposed ports)
- `Cross.toml` (aarch64/armv7 pre-build deps), `justfile` (`arm`, `arm64`, `docker`, `dev`)
- `docker-compose.yml` (Postgres/pgAdmin services), `dev-start.sh`, `demo-terrain.sh`

## Target Operating Model

- A device registry where each drone/edge node enrolls with a stable ID, capabilities, owner, and runtime mode.
- Continuous fleet health: heartbeat, version, component status, and maintenance state, surfaced to operators (domain `11`).
- Centralized observability: metrics and tracing aggregated beyond per-node logs, with alerting on health and capacity thresholds.
- Signed, versioned config and software distribution (OTA) with staged rollout and rollback.
- Secrets managed, not committed; per-node resource budgets enforced on Jetson/Pi-class hardware.
- Simulation-first and edge-ready: every deployment path validated in `Simulation` mode before flight nodes are enrolled.
