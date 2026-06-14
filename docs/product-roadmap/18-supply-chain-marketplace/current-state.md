# Supply Chain & Marketplace: Current State and Target State

## Mission

Connect farmers to input suppliers and produce buyers in a trustworthy, org-scoped marketplace: catalog, listings, orders, procurement, inventory, and logistics, with demand forecasting that draws on drone-derived yield and field data, and payments handled through a compliant external boundary.

## Current Maturity

greenfield pending (M0 named): no implementation exists; product-vision module from `docs/reference/product-summary.md` (#18 Supply Chain & Marketplace). This is the furthest module from the current codebase: AGBot is a drone capture, geospatial, and advisor platform, and a multi-sided commerce marketplace shares almost no existing scaffolding beyond the identity spine.

## What Exists Now

Nothing is built for this domain. The following adjacent surfaces are real or in progress (themselves partly greenfield) and are what this module would build on:

- The org/role/identity model intended in the field-and-data spine (`10`), the basis for supplier/buyer/grower accounts and tenancy.
- The grower portal (`13`, also greenfield), the intended front door and marketplace entry point.
- Advisor yield and crop-health outputs in `09` (`post_processor`), the intended source of demand-forecast signals.
- Field and farm data in `10`, the intended source of demand context (acreage, crops, seasons).

No catalog, order, inventory, logistics, payment, or forecasting capability exists in any crate today.

## Gaps to Close

- No supplier, buyer, or marketplace-account model layered on `10` org/roles.
- No catalog of inputs or produce, and no listings.
- No order or procurement workflow.
- No inventory tracking.
- No demand forecasting consuming `09` yield/health or `10` field data.
- No logistics or fulfillment integration.
- No payments or escrow integration, and no compliance scoping (KYC/AML, PCI-DSS, tax, regional commerce law).
- No ratings/trust model and no marketplace reporting.
- No marketplace entry point wired from the portal (`13`).

## Related Existing Surfaces

- `10` field, farm, and data management (greenfield): org/role/identity model and field/farm data.
- `13` farmers portal (greenfield): intended front door and marketplace entry.
- `09` post-flight analytics and advisor (`post_processor`): yield/crop-health demand signals.
- `docs/reference/product-summary.md` (#18 Supply Chain & Marketplace): source description.

## Target Operating Model

- Suppliers, buyers, and growers are org-scoped accounts with roles and tenancy resolved through `10`; every listing, order, and transaction is owned and audited.
- A catalog of inputs and produce backs listings, orders, and a procurement workflow, with inventory tracking and logistics/fulfillment as integration boundaries.
- Demand forecasting consumes yield/health signals from `09` and field data from `10`, with the evidence behind a forecast inspectable.
- Payments and escrow run through a compliant external provider; AGBot does not move money in-house, and compliance (KYC/AML, PCI-DSS, tax, regional commerce regulation) is scoped before any transactional release.
- Ratings and trust signals protect counterparties, and reporting closes the loop; growers enter the marketplace through the portal (`13`).
