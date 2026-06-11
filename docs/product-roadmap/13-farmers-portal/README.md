# Farmers Portal

The grower-facing front door: a web dashboard and mobile app that give a farmer centralized access to their farm data, advisor reports, recommendations, marketplace, and community knowledge.

## Where We Are

- Not started / vision only. This is a greenfield product-vision module sourced from `docs/reference/product-summary.md` (#3 Farmers Portal); no code exists.
- The platform spine it consumes is partially real: the field/farm/org domain model (`10`), the advisor reports it surfaces (`09`), GIS layers (`07`), and the agronomist viewer (`08`).
- This is distinct from the agronomist viewer (`08`) and the operator console (`11`): those are expert/operations surfaces; the portal is the grower's home.

## Where We Should Be

- A grower signs in (scoped by org and role from `10`) and sees their farms, fields, and the latest findings at a glance.
- A report inbox surfaces advisor reports from `09` with recommendation tracking (acknowledged, in progress, done) and notifications.
- A mobile app delivers the same overview, alerts, and report viewing in the field, plus entry points to the marketplace (`18`) and community/knowledge feed (`20`).
- Growers can export and share their own data, scoped and audited by the `10` access model.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P1 slices.

## Build Order

1. Grower identity and org/role access, resolved through `10`.
2. Field/farm overview that reads the `10` spine and `07` layers read-only.
3. Report inbox that consumes advisor reports from `09`.
4. Recommendation tracking and a notification/alert feed.
5. Mobile app shell sharing the same overview and report APIs.
6. Marketplace (`18`) and community (`20`) entry points, plus data export/sharing.

## Primary Crates

Planned `portal` crate (a portal backend-for-frontend plus web/mobile clients). Builds on domains `10` (field/farm/data, org/roles), `09` (advisor reports), `07` (GIS layers), `08` (viewer patterns), and later `18`/`20`.
