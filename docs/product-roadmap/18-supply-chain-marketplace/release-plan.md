# Supply Chain & Marketplace: Release Plan

## Shipment Strategy

Greenfield (M0) and the furthest domain from the current codebase: nothing ships until org-scoped marketplace accounts and a catalog exist. Ship in maturity order, weighted to M1/M2: supplier/buyer accounts and the catalog come first (M1), then listings, orders, and inventory make the marketplace observable (M2), then demand forecasting and reporting make it explainable (M3), then fulfillment, payments/escrow, and trust make it transactional and interactive (M4). Any autonomous procurement (M5) is gated behind a reliable order/inventory loop and a compliant payments boundary.

This domain is sequenced AFTER the core drone platform (`01`–`12`) and is gated by the advisor MVP (`09`) for demand signals, plus the identity spine (`10`) and the portal (`13`) as prerequisites. Most rows are P2 (post-MVP); the single foundational identity slice (marketplace accounts on `10`) is P1.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 21 |
| M2 captured | 18 |
| M3 explainable | 11 |
| M4 interactive | 14 |
| M5 autonomous-assist | 6 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 0 |
| P1 | 7 |
| P2 | 63 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 9 |
| M | 33 |
| S | 28 |

## First P0/P1 Vertical Slices

| Phase | Size | Priority | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- | --- |
| M1 foundation | M | P1 | Supplier and buyer accounts (on `10` org/roles) | operability | identity |
| M1 foundation | M | P2 | Marketplace catalog (inputs/produce) | operability | identity |
| M2 captured | S | P2 | Listings | trust | identity |
| M2 captured | L | P2 | Orders and procurement workflow | operability | operations |
| M2 captured | M | P2 | Inventory tracking | operability | capture |
| M3 explainable | M | P2 | Demand forecasting (from `09`/`10`) | explainability | evaluator |
| M4 interactive | M | P2 | Payments and escrow (external boundary) | trust | operations |
| M4 interactive | S | P2 | Ratings and trust | trust | operations |

## Execution Rules

- Sequence this domain AFTER the core drone platform (`01`–`12`); it also depends on the identity spine (`10`) and portal (`13`) being real, and is gated by the advisor MVP (`09`) for demand signals.
- The foundational marketplace-account identity slice is the single P1; everything else is P2 (post-MVP).
- Every listing, order, and rating belongs to an organization; no read or write may cross a tenant boundary.
- Payments and escrow are an external boundary: AGBot does not move money in-house, and compliance (KYC/AML, PCI-DSS, tax, regional commerce regulation) must be scoped and approved before any transactional release.
- Demand forecasts must cite their `09`/`10` evidence; an AI-assisted forecast flags uncertainty.
- Do not start M5 autonomous procurement until the order/inventory loop and the compliant payments boundary are reliable.
