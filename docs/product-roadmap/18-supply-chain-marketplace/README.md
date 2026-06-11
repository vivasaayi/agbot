# Supply Chain & Marketplace

A digital marketplace connecting farmers to input suppliers and buyers: catalog, listings, orders, procurement, inventory, and logistics, with demand forecasting that draws on drone-derived yield and field data.

## Where We Are

- Not started / vision only. This is a greenfield product-vision module sourced from `docs/reference/product-summary.md` (#18 Supply Chain & Marketplace); no code exists.
- This is the furthest module from the current codebase. AGBot today is a drone capture, geospatial, and advisor platform; a multi-sided marketplace with procurement, inventory, logistics, and payments is a distinct product surface with little existing scaffolding to build on.
- The spine it would consume is partially real: supplier/buyer/grower accounts and roles come from the field/farm/org spine (`10`), the grower entry point is the portal (`13`), and demand-forecast signals (yield/health) come from the advisor (`09`) and field data (`10`).

## Where We Should Be

- Suppliers, buyers, and growers transact as org-scoped accounts (roles and tenancy from `10`), entering the marketplace through the grower portal (`13`).
- A catalog of inputs and produce supports listings, orders, and a procurement workflow, with inventory tracking and logistics/fulfillment.
- Demand forecasting consumes yield and crop-health signals from `09` and field data from `10` to anticipate input and produce needs.
- Ratings and trust signals support safe counterparties, and reporting closes the loop.

## External Boundaries and Compliance

- Payments and escrow are an external boundary: AGBot integrates a third-party payment/escrow provider rather than building money movement in-house. This carries compliance obligations (KYC/AML, PCI-DSS, tax, and regional commerce regulation) that must be scoped before any transactional release.
- Logistics fulfillment likely integrates external carriers; treat it as an integration boundary, not an owned capability.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P1 slices.

## Build Order

1. Supplier/buyer/grower accounts and roles, resolved through `10`.
2. Marketplace catalog of inputs and produce.
3. Listings and orders with a procurement workflow.
4. Inventory tracking.
5. Demand forecasting consuming yield/health from `09` and field data from `10`.
6. Logistics/fulfillment, ratings/trust, payments/escrow via an external boundary, and reporting; marketplace entry from the portal (`13`).

## Primary Crates

Planned `marketplace` crate (a marketplace backend, catalog/order store, and forecasting service). Builds on domains `10` (orgs/identity, supplier/buyer/grower accounts), `13` (portal front door), and `09` (yield/demand signals). Payments/escrow and logistics are external integrations with compliance requirements.
