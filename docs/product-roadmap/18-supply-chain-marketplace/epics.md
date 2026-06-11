# Supply Chain & Marketplace: Epic Breakdown

These epics are greenfield (M0): no code exists yet, and this is the furthest domain from the current codebase. Each is intended to ship as a vertical slice well after the core drone platform (`01`–`12`), the advisor MVP, and the portal (`13`) and identity spine (`10`) are real.

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: route or command, persistence, auth/tenant scope (via `10`), pagination, and audit events.
- Operability: account, catalog, order, and inventory lifecycles that are observable and recoverable.
- Trust: every listing, order, and rating is owned, scoped, and audited; counterparties are verifiable.
- Deterministic: order state machines, inventory math, and demand forecasts that run and are inspectable; any AI-assisted forecast cites its evidence.
- External boundaries: payments/escrow and logistics integrate third parties; compliance is scoped before transactional release.
- UI: catalog, listings, order tracking, inventory, and reporting (entered via `13`).
- Tests: unit (order/inventory/forecast logic), fixture (catalog/order data), API contract, and one failure path (payment decline / out-of-stock).
- Operations: feature flag or runtime mode, integration health, retry/backoff, and a runbook.

## Category Epics

### EPIC-01: Marketplace Identity and Catalog
- Goal: org-scoped supplier/buyer/grower accounts and a catalog of inputs and produce.
- First release: supplier/buyer accounts on `10` org/roles; catalog items with category, unit, and owner.
- Expansion: listings published from catalog items; marketplace entry from the portal (`13`).
- Hardening: ownership/audit, moderation, and tenant isolation across every read/write.

### EPIC-02: Orders, Inventory, and Demand
- Goal: a working procurement loop with inventory and demand forecasting.
- First release: orders through a state machine; inventory tracking against listings and orders.
- Expansion: demand forecasting from `09` yield/health and `10` field data, with evidence inspectable.
- Hardening: reconciliation, oversell protection, and forecast accuracy reporting.

### EPIC-03: Fulfillment, Payments, and Trust
- Goal: complete the transaction safely with logistics, compliant payments, and trust signals.
- First release: fulfillment/shipment records against orders (logistics integration boundary).
- Expansion: external payment/escrow integration with compliance scoping; ratings and trust.
- Hardening: dispute handling, payment reconciliation, audit, and rollback/disable controls.
