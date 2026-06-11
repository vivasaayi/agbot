# Plugin / Extension SDK and Open Data

Let NGOs, researchers, and partners extend AGBot — custom indices, analysis processors, report templates, map layers, alert rules, and import/export adapters — under a sandboxed capability model, and publish open data. The open-source mission, made extensible and safe.

## Where We Are

- Not started / greenfield (M0 named). No `plugin_sdk` crate exists; extensions can only be added by editing the core crates.
- The extension points it would expose are partially real surfaces: custom spectral index (`05`), custom processor / report template (`09`), custom map layer (`08`), custom alert rule (`29`), and custom import/export adapter (`32`).
- Letting third-party code run inside the platform makes this a real security boundary: plugins must declare and be restricted to capabilities, never exceed them.

## Where We Should Be

- A clear extension-point taxonomy (index, processor, report template, map layer, alert rule, import/export adapter) with a stable plugin manifest and registration.
- A capability/permission model with sandboxed execution: a plugin can only do what its manifest declares; exceeding its capabilities is a tested, blocked failure path.
- A versioning and compatibility contract that gates incompatible plugins instead of loading them.
- The SDK itself with scaffolding, docs, and worked example plugins (a custom vegetation index; a report template).
- An open-data catalog and publishing flow (share anonymized layers/indices with license and attribution metadata), and — later — a plugin registry/marketplace.

## Files

- `current-state.md`: maturity, what exists now (nothing; extension-point surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.
- `stories.md`: per-capability vertical-slice stories.

## Build Order

1. Extension-point taxonomy and a versioned plugin manifest schema with deterministic validation.
2. Plugin registration and discovery against one extension point (custom spectral index in `05`).
3. Capability/permission model and sandboxed execution that blocks undeclared capabilities.
4. Versioning and compatibility gating (refuse incompatible plugins, never load them).
5. The SDK crate, scaffolding, docs, and two example plugins (custom index; report template).
6. Open-data catalog and publishing with license/attribution; plugin registry/marketplace later.

## Primary Crates

New crate `plugin_sdk` (host runtime, manifest, capability model, sandbox) with the trait/extension-point contracts in `shared`. Extension points are wired into `05` (custom index), `09` (custom processor / report template), `08` (custom map layer), `29` (custom alert rule), and `32` (custom adapter). Open-data publishing draws on `07`/`08` layer export.
