# Farmers Portal: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: portal backend route, persistence, auth scoped to org/role (via `10`), pagination, and audit events.
- Access: every read/write is tenant-safe and resolved through the `10` org/role model; no view crosses a tenant boundary.
- Deterministic: overview aggregation and recommendation-status logic that runs without AI; AI summaries (if any) cite the `09` report they came from.
- Data: read-only consumption of `07` layers, `09` reports, and `10` field context with freshness shown to the grower.
- UI: web dashboard and mobile shell for overview, report inbox, and notifications.
- Tests: unit (aggregation/status logic), fixture (sample report/field payloads), API contract, and one failure path (unauthorized cross-tenant read denied).
- Operations: feature flag, read-path health, and a runbook.

## Category Epics

### EPIC-01: Grower Identity and Home
- Goal: a grower signs in and sees a scoped home of their farms and fields.
- First release: org/role-scoped sign-in (via `10`), grower dashboard, and field/farm overview.
- Expansion: saved views, preferences, and per-field latest-finding aggregation.
- Hardening: tenant-isolation tests and audited access.

### EPIC-02: Report Inbox and Recommendation Tracking
- Goal: a grower reads advisor reports and tracks recommendations to done.
- First release: report inbox consuming `09` reports, plus a recommendation status lifecycle.
- Expansion: notification/alert feed for new reports and recommendations.
- Hardening: audit trail on status changes and export of a report/recommendation summary.

### EPIC-03: Mobile, Marketplace, and Community
- Goal: extend the portal into the field and into the wider Aruvi surfaces.
- First release: mobile app shell sharing the overview/report APIs.
- Expansion: marketplace entry point (`18`) and community/knowledge feed (`20`).
- Hardening: grower-scoped data export/sharing under the `10` audit model.
