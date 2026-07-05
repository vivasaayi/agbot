# ADR 0001 — Field Intelligence Pipeline Architecture

- **Status:** Accepted
- **Date:** 2026-07-04
- **Context:** Refactor of AGBot from a "pile of viewers, processors, and apps" into
  a layered field-intelligence pipeline centered on a cataloged, provenance-rich
  product graph. Plan: `/Users/rajanpanneerselvam/Docs/AGBOT/refactor.md`.
  Implemented on branch `field-intelligence-pipeline` (run `field-intel-run-1`).

This ADR records the load-bearing decisions and the trigger that would make us
revisit each. The pipeline layers are: source ingestion → catalog + provenance
ledger (Layer 1) → L0–L3 processing (Layer 3) → geo_hub APIs → web viewer
(Layer 2) → governed applications (Layer 4) → agent/copilot proposals (Layer 5)
→ approval-gated governed dispatch.

## Decision 1 — Storage stays on SQLite (`geo_hub.db` is the catalog authority)

`geo_hub.db` remains the single catalog + provenance authority. The deployment
target is single-node/edge; the additive `ensure_column`/`run_migrations` pattern
and the existing test suites already assume it.

- **Containment:** all catalog SQL lives behind `geo_hub/src/catalog.rs`,
  `product_catalog.rs`, `ingest_contract.rs`, and `provenance_store.rs`.
- **Revisit trigger:** multi-tenant hosting, or real polygon-join query load
  (spatial predicates the SQLite build can't serve efficiently). At that point,
  reassess PostGIS behind the same `catalog.rs` seam.

## Decision 2 — New catalog tables; the legacy `products` table becomes a serving projection

The legacy `products` table has `scene_id NOT NULL` + `UNIQUE(scene_id, kind)`, so
it structurally cannot represent L3 products (multi-scene, field/season-scoped) or
multiple parameterizations of the same kind. Rather than extend it, the refactor
adds first-class catalog tables and dual-writes during the transition.

- **New tables** (`geo_hub/src/db.rs` migrations): `catalog_sources`,
  `catalog_products`, `catalog_product_inputs`, `provenance_evidence`.
- `catalog_products.path`/`format`/`checksum_sha256` are **nullable** — L3
  aggregates and not-yet-materialized drafts have no single backing artifact.
- Legacy `products` is kept and back-filled (`product_catalog.rs`
  `backfill_products_to_catalog`, idempotent); legacy tile routes are unaffected.
- **Revisit trigger:** once no reader depends on `products`, drop the dual-write.

### Identity invariant (correction to the original plan)

Product identity is `UNIQUE(kind, parameters_hash)`, where `parameters_hash`
covers algorithm id/version + canonical parameters + **sorted inputs** only.
Scope (scene/field/season/time, bbox) is descriptive metadata, **not** identity.
Consequence for producers: two scenes yield two distinct products *only because
they list distinct L0/L1 input product ids*. An L2/L3 product that omits its
inputs collapses with every other same-parameter product. Pinned by
`geo_hub/tests/catalog_registry.rs::identity_ignores_scope_two_scenes_same_computation_collapse`
and honored by every sidecar emitter (`imagery_processor::product_sidecar`,
`lidar_mapper::product_sidecar`, `post_processor::l3_product`).

## Decision 3 — Provenance ledger lives in `geo_hub.db`; producers emit evidence, geo_hub persists

The `provenance_lineage_records` / `provenance_audit_entries` tables already
matched `LineageRecord` field-for-field; only the write path was missing.

- Producers construct evidence (`ProductReproducibilityEvidence`,
  `BandIngestEvidence`, `LidarProductReproducibilityEvidence`, analysis
  uncertainty) and embed it in `product_record.json` sidecars. They **never**
  write the ledger.
- geo_hub persists lineage transactionally at choke points:
  `catalog::register_product`, `ingest_contract::commit_ingest`, application runs,
  alert evaluation, proposal creation, recommendation creation, and governed
  dispatch (`provenance_store::append_lineage`).
- `GET /api/provenance/trace/:artifact_id` hydrates a `LineageLedger` per request.
  A backward trace from a Report closes Report → Recommendation → Finding → L3 →
  L2 → L1 → L0 gap-free; from a governed dispatch Action → proposal → finding → L0.

## Decision 4 — Quality masks and confidence are first-class

Quality masks are themselves catalog products (`kind = "qa_mask*"`) referenced by
`quality_mask_product_id`; masks are registered/emitted **before** the indices
that reference them. Confidence is a scalar column plus `confidence_method`
(e.g. L3 confidence derived from an analysis `HealthUncertaintyBand`). Producers
emit `product_record.json` sidecars (the same pattern as the existing
`spatial_ref.json` sidecars); a `geo_hub catalog register <dir>` CLI walks them
into the catalog.

## Decision 5 — No new application crate; governance lives next to the catalog

`post_processor` + `crop_intelligence` already hold the L3 analytics and detection
workflows. The missing piece was *governed execution + catalog emission*, which
belongs next to the catalog — module `geo_hub/src/applications.rs`, not a new
crate.

- Application runs consume only cataloged L2/L3 product refs; outputs are findings
  (+ optional recommendations); every run emits `SystemService` lineage.
- Apps are pure composition functions in `post_processor`
  (`crop_health_app.rs`, `water_priority_app.rs`, `anomaly_app.rs`) mapped to
  governed runs via `applications::record_run`.
- **Copilot is the agent engine, deterministic rules only** (`copilot::advisor_rules`,
  LLM-free); LLM explanation is a deferred optional layer.
- One **unified proposal queue** (`geo_hub/src/proposal_queue.rs`) over alerts /
  findings / recommendations; adapters funnel advisor drafts and
  `CropClosedLoopProposal`s into it (`proposal_adapters.rs`), and
  `CropClosedLoopApprovalStatus` was extended beyond its `Pending` stub.

## Decision 6 — Governed dispatch: separation of duties, no bypass

The approval-gated hand-off from an accepted proposal to flight, satisfying
roadmap M4 (approval-gated, audited), deliberately short of M5 unattended
autonomy.

- `mission_planner::proposal_mission::build_mission_plan_from_proposal` turns an
  approved proposal + field boundary into a real, waypoint-level survey mission
  (reusing `survey_template.rs`), tagged with the proposal id.
- `geo_hub::proposal_mission` is the governance draft + two-step approval gate:
  a dry-run, then `authorize_mission_dispatch` which enforces **separation of
  duties** — the operator must differ from the accepting reviewer.
- `geo_hub::proposal_dispatch::governed_dispatch` refuses unless the approval is
  authorized (no auth-token/flag bypass — authorization is carried by the approval
  value itself), invokes `mission_planner::dispatch_guarded_simulation_command`
  (safety halt, mandatory abort path, MAVLink audit), and records the flight
  `Action` on the ledger sourced from the proposal.
- **Invariants (tested):** no dispatch without a distinct reviewer + operator; no
  dispatch without authorization; every agent artifact carries `SystemService`
  lineage.

## Decision 7 — Web viewer is a static, no-bundler workspace served by geo_hub

Plain HTML + ES-module JS + **vendored** Leaflet 1.9.x under `geo_hub/web/`,
served via `tower-http ServeDir` at `/workspace`. No npm/bundler/Node in the
build (offline field deployments; raster XYZ tiles make MapLibre GL unnecessary).

- **Single-URL-file convention:** all backend URL literals live only in
  `web/js/api.js`, enforced by `geo_hub/tests/workspace_static.rs`
  (route-manifest test + a check that no other `web/js/**.js` contains an `/api/`
  literal), preventing UI/API drift without a JS test harness.

## Decision 8 — Bevy scope freeze

`geo_viewer` (Bevy + egui) is retained for **3D/LiDAR/terrain only**: point clouds,
3D DSM/terrain, 3D mission replay, high-rate sensor overlay. All 2D
browse/compare/annotate/review work is web-first in the `/workspace` viewer. The
existing egui `SuggestedAnnotation` accept/reject UI is retained but frozen.

## Known baseline: pre-existing red tests (not a merge gate)

~15 `geo_hub` `products_api` acceptance tests (farm/field CRUD, shapefile import,
geojson export, annotation/recommendation/report roundtrips) return 500 and fail
on `main` and at every commit back through the prior world-sim work — they predate
this refactor. 183 other `geo_hub` tests pass. Per-batch verification used targeted
`cargo test -p <crate> <test>` plus each batch's own test file; the acceptance
suite is tracked as known-red pending a separate triage.
