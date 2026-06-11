# Farmers Portal: Current State and Target State

## Mission

Be the grower's front door: a web dashboard and mobile app that turn the platform's scenes, layers, findings, and reports into a clear, owned, scoped view a farmer can act on, distinct from the agronomist viewer (`08`) and the operator console (`11`).

## Current Maturity

greenfield pending (M0 named): no implementation exists; this domain is a product-vision module from `product-summary.md` (#3 Farmers Portal). Nothing in the repository implements a grower-facing dashboard, report inbox, recommendation tracking, or mobile app.

## What Exists Now

- Nothing is built for this domain. There is no portal crate, web app, mobile app, or grower-facing API.
- Adjacent surfaces it would build on (already partially real):
  - Domain `10` (field/farm/data + org/roles): the canonical Organization/Farm/Field/Boundary model and access control the portal must resolve every view through. Itself greenfield-pending, so this domain is gated on it.
  - Domain `09` (post-flight analytics and advisor): the advisor reports and recommendations the report inbox consumes.
  - Domain `07` (GIS and geospatial hub): the layer/scene services the field overview reads for maps and overlays.
  - Domain `08` (geo viewer and visualization): the Bevy report/recommendation rendering patterns the portal can reuse or embed.

## Gaps to Close

- No grower identity surface or session scoped by org and role (depends on `10`).
- No field/farm overview that aggregates the latest findings per field.
- No report inbox that lists and opens advisor reports from `09`.
- No recommendation tracking (acknowledged / in progress / done) or status history.
- No notification or alert feed for new reports, recommendations, or risk events.
- No mobile app shell or offline/field-friendly read path.
- No marketplace entry point (`18`) or community/knowledge feed (`20`).
- No grower-scoped data export and sharing, audited through the `10` access model.

## Related Existing Surfaces

- Domain `10` (field/farm/data, org/roles): field/farm/boundary identity and tenant-safe access — the spine every portal view resolves through.
- Domain `09` (advisor): report and recommendation records the inbox consumes.
- Domain `07` (GIS hub): layer/scene APIs for the field map and overlays.
- Domain `08` (geo viewer): report/recommendation rendering patterns to reuse.
- `docs/reference/product-summary.md` (#3 Farmers Portal): the source description for this module.

## Target Operating Model

- A grower signs in and the portal resolves their organization, role, farms, and fields through the `10` spine; no view crosses a tenant boundary.
- A field/farm overview aggregates the latest scene, layer, and finding per field, reading `07`/`10` read-only.
- A report inbox consumes advisor reports from `09`, with recommendation tracking and an audited status lifecycle.
- A notification feed alerts on new reports, recommendations, and (later) weather/drought risk events from `15`/`17`.
- A mobile app shares the same overview, report, and notification APIs for in-field use.
- Marketplace (`18`) and community (`20`) appear as scoped entry points, and growers can export/share their own data under the `10` audit trail.
