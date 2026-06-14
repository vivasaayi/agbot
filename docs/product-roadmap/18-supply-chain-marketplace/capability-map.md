# Supply Chain & Marketplace: Capability Map

This map is service/domain-first. Each capability is intended (greenfield); none is implemented yet. This is the furthest domain from the current codebase. Capabilities expand across the relevant pillars (with emphasis on operability and trust) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated. Every Primary First Slice is the M1 foundation step that makes the capability real.

## Supply Chain & Marketplace Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Supplier and buyer accounts (on `10` org/roles) | missing (greenfield) | 7 | Create an org-scoped supplier/buyer account via `10` |
| Marketplace catalog (inputs/produce) | missing (greenfield) | 8 | Persist a catalog item with category, unit, and owner |
| Listings | missing (greenfield) | 6 | Publish a listing from a catalog item, scoped and audited |
| Orders and procurement workflow | missing (greenfield) | 9 | Place and track an order through a state machine |
| Inventory tracking | missing (greenfield) | 7 | Track stock levels against listings and orders |
| Demand forecasting (from `09`/`10`) | missing (greenfield) | 7 | Forecast input/produce demand from `09` yield + `10` field data |
| Logistics and fulfillment | missing (greenfield) | 6 | Record a fulfillment/shipment against an order (integration boundary) |
| Payments and escrow (external boundary) | missing (greenfield) | 7 | Integrate an external payment/escrow provider with compliance scoping |
| Ratings and trust | missing (greenfield) | 5 | Record a counterparty rating tied to a completed order |
| Marketplace entry from portal (`13`) | missing (greenfield) | 4 | Wire a marketplace entry point from the grower portal |
| Marketplace reporting | missing (greenfield) | 4 | Per-org sales/procurement report with audit |
