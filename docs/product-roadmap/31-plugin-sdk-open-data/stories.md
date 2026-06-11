# Plugin / Extension SDK and Open Data: Detailed Stories

> Greenfield domain (M0 named): no code exists yet. This domain runs third-party code inside the platform, so the **capability/permission boundary is a real security concern and dominates every phase**: no plugin executes outside the capabilities its manifest declares, and exceeding them is a tested, blocked failure path. The extension host, manifest validation, and version gating are all deterministic and inspectable. The first extension point lands in `05` (custom spectral index); the SDK and open-data publishing follow once the boundary is proven.

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `DEV` plugin author, `PA` platform admin, `AG` agronomist, `DSP` drone service provider, `OPS` operator.

---

## M1 — Foundation

### STORY 31-01 · M1 · S · P0 — Extension-point taxonomy
- **Story**: As `DEV`, I want a defined set of extension points with typed contracts, so that I know exactly what I can build and how it plugs in.
- **Deterministic / evidence**: define six extension-point kinds — index, processor, report template, map layer, alert rule, import/export adapter — each as a typed trait/contract in `shared`; the host enumerates them.
- **Acceptance**:
  - Given the host, when extension points are listed, then exactly the six kinds are returned with their contract signatures.
  - Given a plugin declaring an unknown extension-point kind, when registered, then it is rejected with an unknown-kind reason code.
- **Tests**: unit (taxonomy enumeration), API contract, failure path (unknown kind rejected).
- **Depends on**: `shared`.

### STORY 31-02 · M1 · M · P0 — Plugin manifest and registration
- **Story**: As `DEV`, I want to register a plugin from a manifest, so that the platform can discover and load my extension.
- **Deterministic / evidence**: validate a manifest `{plugin_id, name, version, kind, host_api_version, capabilities[], entrypoint}` against a schema; persist a registration record; validation is deterministic.
- **Acceptance**:
  - Given a well-formed manifest, when a plugin is registered, then a registration record is created with its kind, version, and declared capabilities.
  - Given a manifest missing required fields or with a malformed capability list, when registered, then it is rejected with field-level reason codes (no partial registration).
- **Tests**: unit (manifest schema validation), API contract (register/list), failure path (malformed manifest rejected).
- **Depends on**: 31-01.

### STORY 31-03 · M1 · S · P1 — Plugin listing, enable/disable, and audit
- **Story**: As `PA`, I want to list, enable, and disable plugins, so that I control what runs in my deployment.
- **Deterministic / evidence**: plugins have lifecycle `Registered→Enabled→Disabled`; every transition is audited via `30`; a disabled plugin cannot execute.
- **Acceptance**: plugins are paginated and filterable by kind/status; a disabled plugin's execution is refused; every enable/disable is audited with actor and timestamp.
- **Tests**: API contract (list/enable/disable), failure path (disabled plugin refused), audit assertion.
- **Depends on**: 31-02, `30`.

---

## M3 — Explainable (the capability boundary and version contract)

### STORY 31-04 · M3 · M · P0 — Capability / permission model
- **Story**: As `PA`, I want plugins restricted to the capabilities they declare, so that a plugin cannot reach data or actions it was not granted.
- **Deterministic / evidence**: each capability (e.g. `read:scene`, `write:report`, `net:none`) is enforced at the host boundary; a call requiring an undeclared capability is denied before it runs.
- **Acceptance**:
  - Given a plugin declaring `read:scene` only, when it reads a scene, then the call is permitted.
  - Given the same plugin attempting a network call or a `write:field`, when it runs, then the call is denied with a capability-violation reason code and the attempt is audited.
- **Tests**: unit (capability check matrix), integration (permitted vs denied), failure path (undeclared capability denied).
- **Depends on**: 31-02.

### STORY 31-05 · M3 · L · P0 — Sandboxed execution
- **Story**: As `OPS`, I want plugins to run in a sandbox bounded by their capabilities and resource limits, so that a faulty or hostile plugin cannot destabilize the host.
- **Deterministic / evidence**: execute the plugin in an isolated context where capability checks are enforced and resource limits (time, memory) are bounded; a violation or limit breach terminates the plugin cleanly.
- **Acceptance**:
  - Given a well-behaved plugin within its limits, when it runs, then it completes and returns its result.
  - Given a plugin that exceeds its time/memory limit or attempts an undeclared capability, when it runs, then it is terminated with a reason code and the host stays healthy (no crash, no leaked access).
- **Tests**: unit (limit enforcement), integration (sandbox isolation), failure path (runaway/violating plugin terminated, host survives).
- **Depends on**: 31-04.

### STORY 31-06 · M3 · M · P0 — Versioning and compatibility contract
- **Story**: As `PA`, I want plugins gated against the host API version, so that an incompatible plugin never loads.
- **Deterministic / evidence**: compare the manifest's `host_api_version` against the host's supported range using a deterministic compatibility rule; refuse anything outside the range.
- **Acceptance**:
  - Given a plugin compatible with the host API version, when loaded, then it loads.
  - Given a plugin built for an unsupported host API version, when loaded, then it is refused with a version-mismatch reason code (never loaded with degraded behavior).
- **Tests**: unit (compatibility rule incl. boundaries), API contract, failure path (incompatible version refused).
- **Depends on**: 31-02.

### STORY 31-07 · M3 · M · P1 — Custom spectral index extension point (`05`)
- **Story**: As `DEV`, I want to register a custom spectral index that `05` runs, so that I can compute an index the platform does not ship.
- **Deterministic / evidence**: the index plugin declares its input bands and formula; `05` invokes it under the sandbox; the output raster preserves the source CRS/extent/resolution.
- **Acceptance**:
  - Given a registered index plugin and a multispectral scene with the required bands, when `05` runs it, then the index raster is produced with the source CRS/extent and the plugin's identity recorded via `30`.
  - Given a scene missing a band the plugin requires, when invoked, then it fails with a missing-band reason code (no fabricated index).
- **Tests**: unit (index invocation), geospatial (CRS/extent preserved), failure path (missing band).
- **Depends on**: 31-04, 31-05, `05`, `30`.

### STORY 31-08 · M3 · M · P1 — Custom processor / report template (`09`)
- **Story**: As `DEV`, I want to register a custom analysis processor and a custom report template that `09` uses, so that partners can add analyses and branded deliverables.
- **Deterministic / evidence**: a processor plugin slots into the `post_processor` pipeline under the sandbox; a report-template plugin renders within the report generator; both record plugin identity/version via `30`.
- **Acceptance**:
  - Given a registered processor, when a `09` job selects it, then it runs sandboxed and its result is stored with plugin provenance.
  - Given a report-template plugin that requests data outside its declared capabilities, when rendered, then it is denied and the report falls back to a safe default (no unauthorized data in the deliverable).
- **Tests**: integration (`09` pipeline + report render), unit (template capability check), failure path (over-reach denied).
- **Depends on**: 31-04, 31-05, `09`, `30`.

---

## M4 — Interactive (the SDK, remaining extension points, and open data)

### STORY 31-09 · M4 · S · P1 — Custom map layer extension point (`08`)
- **Story**: As `DEV`, I want to register a custom map layer that `08` renders, so that I can visualize a derived layer in the viewer.
- **Deterministic / evidence**: the layer plugin declares its source and styling; `08` renders it only if its CRS/extent are asserted correct.
- **Acceptance**: a registered layer renders in `08` with correct CRS/extent; a layer whose CRS cannot be proven is not rendered (a wrong overlay is worse than none).
- **Tests**: integration (`08` render), geospatial (CRS assertion), failure path (unprovable CRS → not rendered).
- **Depends on**: 31-04, 31-05, `08`.

### STORY 31-10 · M4 · S · P1 — Custom alert rule extension point (`29`)
- **Story**: As `DEV`, I want to register a custom alert rule that `29` evaluates, so that partners can define their own alerting logic.
- **Deterministic / evidence**: the rule plugin is a pure evaluator over declared inputs; `29` runs it under the sandbox; the rule cannot send notifications itself — it only emits a finding `29` routes.
- **Acceptance**: a registered rule evaluates within `29` and emits a deterministic finding; a rule attempting to send a notification directly is denied (notification dispatch stays with `29`).
- **Tests**: integration (`29` evaluation), unit (rule purity), failure path (direct-dispatch attempt denied).
- **Depends on**: 31-04, 31-05, `29`.

### STORY 31-11 · M4 · S · P1 — Custom import/export adapter extension point (`32`)
- **Story**: As `DEV`, I want to register a custom import/export adapter that `32` uses, so that I can support a format the platform does not ship.
- **Deterministic / evidence**: the adapter plugin declares the format and direction; `32` invokes it through the standard adapter contract; imports are still validated and reprojected by `32`'s deterministic pipeline.
- **Acceptance**: a registered adapter participates in a `32` import/export; an adapter that returns a layer with the wrong CRS is caught by `32`'s validation and rejected (the boundary is enforced by the host, not trusted to the plugin).
- **Tests**: integration (`32` round-trip), geospatial (CRS validation), failure path (wrong-CRS output rejected).
- **Depends on**: 31-04, 31-05, `32`.

### STORY 31-12 · M4 · M · P1 — SDK, scaffolding, docs, and example plugins
- **Story**: As `DEV`, I want an SDK with a scaffolder, docs, and worked examples, so that I can build a plugin without reading the core source.
- **Deterministic / evidence**: the SDK exposes the extension-point traits and a manifest builder; a scaffolder generates a plugin skeleton; two examples (a custom vegetation index, a report template) build and pass their own tests.
- **Acceptance**:
  - Given the scaffolder, when a DEV generates a plugin of a chosen kind, then a buildable skeleton with a valid manifest is produced.
  - Given the two example plugins, when their tests run, then both build, register, and execute under the sandbox successfully.
- **Tests**: build (scaffolded skeleton compiles), example-plugin tests (index + template), failure path (scaffold with an invalid kind errors).
- **Depends on**: 31-01, 31-07, 31-08.

### STORY 31-13 · M4 · M · P1 — Open-data catalog and publishing
- **Story**: As `AG`, I want to publish an anonymized layer or index with license and attribution, so that researchers and NGOs can reuse it in line with the open-source mission.
- **Deterministic / evidence**: publishing requires license + attribution metadata and runs an anonymization check (strips owner/field identifiers); published layers preserve CRS/extent; the catalog is queryable.
- **Acceptance**:
  - Given a layer with license and attribution metadata that passes the anonymization check, when published, then it appears in the open-data catalog with correct CRS/extent and its license.
  - Given a layer missing a license or failing anonymization, when published, then it is refused with the cited reason (no unlicensed or de-anonymizable data leaves the platform).
- **Tests**: API contract (publish/list), unit (anonymization + license check), failure path (missing license / failed anonymization refused).
- **Depends on**: 31-02, `07`/`08` layer export.

---

## M5 — Plugin Registry / Marketplace (later)

### STORY 31-14 · M5 · M · P2 — Plugin registry and install
- **Story**: As `PA`, I want to browse and install plugins from a registry, so that I can extend my deployment without building from source.
- **Deterministic / evidence**: the registry catalogs plugins with manifest, version, and capability declarations; install re-runs manifest validation, version gating, and capability review locally before enabling.
- **Acceptance**:
  - Given a registry entry compatible with the host, when installed, then it is validated, version-gated, and registered disabled-by-default pending an explicit enable.
  - Given a registry entry incompatible with the host API version, when installed, then it is refused before download/enable (the version contract is re-checked locally, not trusted from the registry).
- **Tests**: API contract (browse/install), unit (local re-validation), failure path (incompatible entry refused).
- **Depends on**: 31-02, 31-06.

---

## Coverage note

These 14 stories cover all 13 capabilities in `capability-map.md`. The breakdown carries a heavy M3 security core — capability model, sandboxed execution, and version gating — reflecting that **the capability boundary leads every phase** in `release-plan.md`: no plugin runs outside its declared capabilities, and that is the central tested failure path (31-04/31-05). The curated counts in `release-plan.md` (~74 rows) expand several of these (per-extension-point adapter variants, additional example plugins, registry signing/trust slices) into sibling stories when implemented.
