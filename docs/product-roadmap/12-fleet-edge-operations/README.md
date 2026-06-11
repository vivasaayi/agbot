# Fleet and Edge Operations

Keep the platform deployable, observable, and operable in the field on Jetson/Raspberry Pi-class edge hardware: enroll drones, track their health, and distribute config to a fleet.

## Where We Are

- One config model (`AgroConfig`) loads server/storage/processing/MAVLink/GPS settings from TOML/env via `shared/src/config.rs`, with a `RuntimeMode` (Simulation/Flight) and `init_logging` tracing in `shared/src/lib.rs`.
- Multi-stage `Dockerfile` builds the runtime binaries; `docker-compose.yml` brings up Postgres and pgAdmin; `dev-start.sh` runs the sim stack locally.
- `Cross.toml` + `justfile` `arm`/`arm64` recipes cross-compile for aarch64 (Jetson) and armv7 (Raspberry Pi); systemd service examples live in the repo README.

## Where We Should Be

- A device registry: each drone/edge node enrolled with a stable identity, capabilities, and runtime mode.
- Fleet health and maintenance tracking, centralized observability (metrics/tracing beyond logs), and alerting.
- Config and software distribution (OTA) to enrolled nodes, with secrets management and per-node resource budgeting.

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.

## Build Order

1. Define device/drone identity and an enrollment record.
2. Report per-node health and runtime mode back to a registry.
3. Add centralized metrics/tracing and a minimal alert path.
4. Distribute config to enrolled nodes (signed, versioned).
5. Add secrets management and per-node resource budgeting.
6. Add OTA software/config rollout with staged release and rollback.

## Primary Crates

`shared` (`config.rs`, `lib.rs`) for config, runtime mode, and logging, plus the deployment surface: `Dockerfile`, `Cross.toml`, `docker-compose.yml`, `justfile`, `dev-start.sh`, `demo-terrain.sh`. Operates the binaries built across all domains; closely tied to the live console in domain `11` for operator-facing alerts.
