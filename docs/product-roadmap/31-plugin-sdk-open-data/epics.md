# Plugin / Extension SDK and Open Data: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: plugin register/list/enable/disable and open-data publish routes or commands with pagination and audit IDs.
- Deterministic: manifest validation, capability enforcement, and version gating computed without AI, with reason codes — these are the inspectable contracts that make extension safe.
- Security: sandboxed execution is a real boundary; a plugin can only touch declared capabilities, and exceeding them is denied and audited.
- Geospatial: custom indices and map layers preserve CRS/extent; published open-data layers carry correct georeferencing.
- Explainability/trust: every plugin-produced artifact records its plugin identity and version (via `30`); published data carries license and attribution.
- Tests: unit (manifest/capability/version logic), fixture (sample plugins), API contract, and one failure path (capability violation / version mismatch denied).
- Operations: plugin enable/disable flags, host-API version surface, and a runbook.

## Category Epics

### EPIC-01: Extension Host and Safe Execution
- Goal: a plugin can be registered, validated, and run without exceeding its declared capabilities.
- First release: the extension-point taxonomy, a validated plugin manifest, and registration against one extension point (custom spectral index in `05`).
- Expansion: the capability/permission model and sandboxed execution that blocks undeclared capabilities, plus version/compatibility gating.
- Hardening: capability-violation and version-mismatch negative-path tests, and per-plugin audit via `30`.

### EPIC-02: Extension Points and the SDK
- Goal: the six extension points are real and a DEV can build a plugin from the SDK alone.
- First release: extension points for custom processor / report template (`09`) and custom map layer (`08`).
- Expansion: extension points for custom alert rule (`29`) and custom import/export adapter (`32`); the SDK crate, scaffolder, and two worked examples.
- Hardening: example-plugin tests as living docs, and SDK compatibility tests across host versions.

### EPIC-03: Open Data and Registry
- Goal: extensions and data can be shared in line with the open-source mission.
- First release: an open-data catalog and publishing flow for anonymized layers/indices with license and attribution metadata.
- Expansion: a plugin registry/marketplace to catalog and install plugins.
- Hardening: license/attribution validation, anonymization checks, and registry install/version negative-path tests.
