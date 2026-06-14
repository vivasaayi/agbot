# Supply Chain & Marketplace: Detailed Stories

> Greenfield (M0): no code exists for this domain yet. It is the furthest module from the current codebase and is gated behind the core drone platform (`01`–`12`), the identity spine (`10`), the portal (`13`), and the advisor MVP (`09`) for demand signals. Stories are necessarily coarse and weighted to M1/M2 foundation; everything here is "build from scratch."

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI. Payments/escrow are an **external compliance boundary** — AGBot does not move money in-house.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `GR` grower, `OPS` operator, `PA` platform admin, `BUYER` produce buyer, `SUPPLIER` input supplier.

---

## M1 — Foundation

### STORY 18-01 · M1 · M · P1 — Org-scoped supplier/buyer accounts
- **Story**: As `PA`, I want supplier and buyer accounts created as org-scoped roles on the `10` identity model, so that every marketplace participant has a tenant-bound, audited identity.
- **Deterministic / evidence**: persist `{account_id, org_id, party_type(SUPPLIER|BUYER|GROWER), role_refs[], status, created_at}` layered on `10` org/roles; every account resolves to exactly one org; account lifecycle `Pending→Active→Suspended`.
- **Acceptance**:
  - Given a valid `10` org, when a supplier account is created, then it persists with `org_id`, `party_type`, and `Active` status and is listable within that org only.
  - Given a request to read an account from a different org, when the read runs, then it is denied (no cross-tenant join).
  - Given a create request with an unknown `org_id`, when it runs, then it fails with a clear `4xx` and no account row is written.
- **Tests**: API contract (create/list/suspend), authz (cross-tenant read denied), failure path (unknown org → 4xx).
- **Depends on**: `10` (org/roles/identity).

### STORY 18-02 · M1 · M · P2 — Marketplace catalog (inputs/produce)
- **Story**: As `SUPPLIER`, I want to persist catalog items for inputs and produce with category, unit, and owner, so that there is a structured basis for listings and orders.
- **Deterministic / evidence**: persist `{item_id, org_id, kind(INPUT|PRODUCE), category, name, unit_of_measure, owner_account_id, created_at}`; unit and category validated against a controlled vocabulary; item owned by one org.
- **Acceptance**:
  - Given an active supplier account, when a catalog item is created with a valid category and unit, then it persists scoped to the owner's org and is retrievable by ID.
  - Given a catalog item with an unrecognized unit of measure, when it is created, then it is rejected with a validation error and no row is written.
- **Tests**: unit (vocabulary validation), API contract (create/get/list), failure path (invalid unit → rejected).
- **Depends on**: 18-01, `10`.

### STORY 18-03 · M1 · S · P2 — Marketplace entry from portal (`13`)
- **Story**: As `GR`, I want a marketplace entry point in the grower portal, so that I can reach catalog, listings, and orders without a separate system.
- **Deterministic / evidence**: the portal (`13`) renders a marketplace navigation entry only for accounts whose `10` role grants marketplace access; the entry deep-links into the org-scoped marketplace surface.
- **Acceptance**:
  - Given a grower with marketplace access, when they open the portal, then the marketplace entry appears and links to their org's marketplace.
  - Given a user without marketplace access, when they open the portal, then the entry is absent and the marketplace route returns `403`.
- **Tests**: integration with `13` navigation, authz (no-access user denied), failure path (direct route hit without access → 403).
- **Depends on**: 18-01, `13` (portal), `10` (roles).

---

## M2 — Captured / Observable

### STORY 18-04 · M2 · S · P2 — Publish a listing from a catalog item
- **Story**: As `SUPPLIER`, I want to publish a listing from a catalog item with price, quantity, and availability window, so that buyers can discover what I offer.
- **Deterministic / evidence**: persist `{listing_id, item_id, org_id, price, currency, available_qty, window{from,to}, status(Draft|Published|Closed)}`; listing references one catalog item in the same org; published listings visible per tenant scope.
- **Acceptance**:
  - Given an owned catalog item, when a listing is published with a valid price and window, then it persists `Published` and appears in the org's listing feed.
  - Given a listing whose availability window ends before it starts, when it is published, then it is rejected with a validation error.
- **Tests**: unit (window/price validation), API contract (publish/close/list), failure path (inverted window → rejected).
- **Depends on**: 18-02 (catalog), `10`.

### STORY 18-05 · M2 · L · P2 — Orders and procurement workflow
- **Story**: As `BUYER`, I want to place and track an order against a listing through a state machine, so that procurement progresses through defensible, audited states.
- **Deterministic / evidence**: persist `{order_id, org_id, listing_ref, buyer_account_id, qty, line_total, status}`; status lifecycle `Placed→Confirmed→Fulfilled→Closed` with `Cancelled` from any pre-fulfilled state; every transition audited with actor and timestamp; line total computed deterministically from listing price × qty.
- **Acceptance**:
  - Given a published listing with sufficient quantity, when an order is placed, then it persists `Placed` with a computed line total and an audit entry.
  - Given an order in `Placed`, when it is confirmed then fulfilled, then each transition is recorded with actor and timestamp.
  - Given a transition that is not legal from the current state (e.g. `Closed→Confirmed`), when attempted, then it is rejected and the order state is unchanged.
- **Tests**: unit (state machine + line-total math), API contract (place/confirm/fulfil/cancel), failure path (illegal transition → rejected).
- **Depends on**: 18-04 (listings), 18-06 (inventory), `10`.

### STORY 18-06 · M2 · M · P2 — Inventory tracking
- **Story**: As `SUPPLIER`, I want stock levels tracked against listings and orders, so that I never oversell and quantities stay consistent.
- **Deterministic / evidence**: persist `{inventory_id, item_id, org_id, on_hand, reserved}`; placing an order reserves stock, fulfilling decrements on-hand, cancelling releases the reservation; invariant `reserved ≤ on_hand` enforced atomically.
- **Acceptance**:
  - Given inventory with on-hand stock, when an order is placed, then the ordered quantity is moved to `reserved` and `on_hand` is unchanged until fulfilment.
  - Given an order for more than available on-hand, when it is placed, then it is rejected and no reservation is made.
- **Tests**: unit (reserve/decrement/release invariants), concurrency test (no oversell under parallel orders), failure path (over-reserve → rejected).
- **Depends on**: 18-02 (catalog), 18-04 (listings).

---

## M3 — Explainable

### STORY 18-07 · M3 · M · P2 — Demand forecasting from `09`/`10`
- **Story**: As `AG`, I want input/produce demand forecast from `09` yield/health outputs and `10` field data, so that growers and suppliers can plan procurement against real evidence.
- **Deterministic / evidence**: forecast persists `{forecast_id, org_id, item_kind, horizon, value, evidence_refs[]}` citing the `09` yield/health products and `10` field acreage/crop/season it derived from; deterministic baseline (acreage × crop input rate, or yield × area) runs first; any AI-assisted forecast carries an uncertainty band and cites the same evidence.
- **Acceptance**:
  - Given a field with `09` yield outputs and `10` acreage, when a demand forecast runs, then it returns a value that cites its `09`/`10` evidence refs.
  - Given an AI-assisted forecast, when it is produced, then it includes an uncertainty band and never omits its evidence layers.
  - Given a field with no `09` signal and no `10` acreage, when a forecast is requested, then it returns "no basis" rather than a fabricated number.
- **Tests**: unit (deterministic baseline math), evidence test (refs present), failure path (no evidence → "no basis").
- **Depends on**: `09` (yield/health), `10` (field data), 18-02.

### STORY 18-08 · M3 · S · P2 — Per-org marketplace reporting
- **Story**: As `PA`, I want a per-org sales and procurement report with audit, so that an organization can close the loop on its marketplace activity.
- **Deterministic / evidence**: aggregate orders/listings/inventory deterministically into `{period, sales_total, procurement_total, order_counts_by_status}` scoped to one org; every figure traceable to its source order IDs.
- **Acceptance**:
  - Given a period with orders, when a report runs, then totals and per-status counts are computed and each figure links to its source orders.
  - Given a period with no activity, when a report runs, then it returns a valid empty report (zeros), not an error.
- **Tests**: unit (aggregation math), API contract (report by period/org), failure path (empty period → valid empty report).
- **Depends on**: 18-05 (orders), 18-06 (inventory).

---

## M4 — Interactive

### STORY 18-09 · M4 · S · P2 — Logistics and fulfillment (integration boundary)
- **Story**: As `OPS`, I want to record a fulfillment/shipment against an order, so that delivery is tracked even though carriers are an external integration.
- **Deterministic / evidence**: persist `{fulfillment_id, order_ref, org_id, carrier_ref, tracking_ref, status(Pending|Shipped|Delivered|Failed)}`; fulfillment references one order in the same org; carrier/tracking treated as an opaque external integration boundary; status transitions audited.
- **Acceptance**:
  - Given a `Confirmed` order, when a shipment is recorded, then a fulfillment row persists linked to the order and the order can advance to `Fulfilled`.
  - Given a fulfillment whose order belongs to another org, when it is recorded, then it is rejected (no cross-tenant link).
- **Tests**: API contract (record/advance), authz (cross-tenant link denied), failure path (foreign order → rejected).
- **Depends on**: 18-05 (orders), `10`.

### STORY 18-10 · M4 · M · P2 — Payments and escrow (external compliance boundary)
- **Story**: As `BUYER`, I want order payment and escrow handled through a compliant external provider, so that funds move safely without AGBot taking custody of money.
- **Deterministic / evidence**: AGBot persists only `{payment_intent_id, order_ref, org_id, provider, provider_ref, state(Initiated|Held|Released|Refunded|Failed)}` and reconciles provider webhooks; **no money moves in-house**; KYC/AML, PCI-DSS, tax, and regional commerce compliance are scoped and approved before this slice is released; provider is the system of record for funds.
- **Acceptance**:
  - Given a confirmed order and an approved provider integration, when a payment intent is created, then AGBot stores the provider reference and reflects the provider's `Held` state without holding funds itself.
  - Given a provider webhook signaling release, when it is reconciled, then the order's payment state advances and the event is audited against the order.
  - Given a provider failure or rejected/expired intent, when reconciled, then the order is not marked paid and the failure is recorded with a reason code (never silently succeeds).
- **Tests**: contract test against a provider sandbox/mock, webhook reconciliation test, failure path (failed/expired intent → order not paid), compliance gate test (slice disabled until compliance approval flag set).
- **Depends on**: 18-05 (orders), `10`; external payment/escrow provider; compliance approval.

### STORY 18-11 · M4 · S · P2 — Ratings and trust
- **Story**: As `BUYER`, I want to record a counterparty rating tied to a completed order, so that trustworthy participants are visible and bad actors are surfaced.
- **Deterministic / evidence**: persist `{rating_id, order_ref, rater_account_id, ratee_account_id, score, comment, org_scope}`; a rating is permitted only against a `Closed`/`Fulfilled` order the rater participated in; one rating per party per order; aggregate score computed deterministically.
- **Acceptance**:
  - Given a fulfilled order the rater took part in, when they submit a rating, then it persists tied to the order and updates the ratee's aggregate score.
  - Given a rating attempt on an order the user did not participate in, when submitted, then it is rejected.
- **Tests**: unit (aggregate score, one-per-party), authz (non-participant denied), failure path (non-participant rating → rejected).
- **Depends on**: 18-05 (orders), 18-01.

---

## Coverage note

This file covers all 11 capabilities in `capability-map.md` with ~11 greenfield stories (≈1 per capability), weighted to M1/M2 foundation and almost entirely P2, matching `release-plan.md` (only the marketplace-account identity slice, 18-01, is P1; no P0). The curated counts in `release-plan.md` (≈70 rows) expand several of these into sibling slices when implemented (e.g. catalog import, listing variants, multi-leg logistics, refund/dispute flows, additional report cuts). Payments and escrow (18-10) are deliberately modeled as an **external compliance boundary**: AGBot never moves money in-house, and KYC/AML, PCI-DSS, tax, and regional commerce compliance must be scoped and approved before that slice is released.
