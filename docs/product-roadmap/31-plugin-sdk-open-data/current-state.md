# Plugin / Extension SDK and Open Data: Current State and Target State

## Mission

Open AGBot to extension without opening it to harm: give NGOs, researchers, and partners a typed extension-point taxonomy, a validated plugin manifest, a capability/permission model with sandboxed execution, and a versioning contract — then let them publish open data (anonymized layers and indices) with license and attribution. Extensions add value; the capability boundary keeps them safe.

## Current Maturity

greenfield (M0 named): no implementation exists. There is no `plugin_sdk` crate, no manifest schema, no capability model, no sandbox, and no open-data catalog. The only way to extend the platform today is to edit the core crates directly, which is neither safe for third parties nor compatible with the open-source mission.

## What Exists Now

- Nothing is built for this domain. There is no extension host, plugin registry, capability enforcement, or open-data publishing.
- Adjacent surfaces that would become extension points (already partially real):
  - Domain `05` (imagery / remote sensing): spectral-index computation — the natural home for a custom-index extension point.
  - Domain `09` (`post_processor`): the analysis-processor pipeline and the report generator (PDF/HTML/JSON/CSV/KML/Shapefile) — extension points for custom processors and report templates.
  - Domain `08` (geo viewer): layer rendering — an extension point for custom map layers.
  - Domain `29` (alerting/notification): alert-rule evaluation — an extension point for custom alert rules.
  - Domain `32` (import/export interop): format adapters — an extension point for custom import/export adapters.
  - Domains `07`/`08`: layer export, the basis for publishing anonymized open-data layers.

## Gaps to Close

- No extension-point taxonomy: the six kinds (index / processor / report template / map layer / alert rule / import-export adapter) are not defined as stable contracts.
- No plugin manifest schema, no registration, and no deterministic manifest validation.
- No capability/permission model and no sandboxed execution: there is no boundary that restricts third-party code to declared capabilities.
- No versioning or compatibility contract: nothing gates an incompatible plugin from loading.
- No SDK crate, scaffolding, docs, or worked example plugins.
- No open-data catalog or publishing flow with license/attribution metadata, and no plugin registry/marketplace.

## Related Existing Surfaces

- Domain `05` (imagery / remote sensing): spectral-index computation — the custom-index extension point.
- Domain `09` (`post_processor`): analysis processors and the report generator — custom-processor and report-template extension points.
- Domain `08` (geo viewer): layer rendering — the custom map-layer extension point.
- Domain `29` (alerting/notification): alert-rule evaluation — the custom alert-rule extension point.
- Domain `32` (import/export interop): format adapters — the custom adapter extension point.
- Domain `30` (provenance/audit): records which plugin produced which artifact, for trust.

## Target Operating Model

- Six stable extension points (index, processor, report template, map layer, alert rule, import/export adapter), each a typed contract in `shared`.
- A plugin is registered from a validated manifest that declares its kind, version, and the exact capabilities it needs; an invalid manifest is rejected with reason codes.
- The capability/permission model is the security boundary: a plugin runs sandboxed and can only touch what it declared; an attempt to exceed its capabilities is denied and audited — a tested failure path.
- A versioning and compatibility contract gates plugins against the host API version; an incompatible plugin is refused, never loaded.
- The SDK ships with a scaffolder, docs, and two worked examples (a custom vegetation index and a report template) so a `DEV` can build a plugin without reading the core source.
- Open data is publishable: anonymized layers and indices are shared with license and attribution metadata through a catalog; a plugin registry/marketplace follows once the capability model and versioning are proven.
- Every plugin-produced artifact records its plugin identity and version via `30`, so extended outputs stay traceable.
