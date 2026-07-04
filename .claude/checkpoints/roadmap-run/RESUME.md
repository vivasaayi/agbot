# Resume — field-intel-run-1

Refactor: AGBot → layered field-intelligence pipeline.
Plan: /Users/rajanpanneerselvam/Docs/AGBOT/refactor.md (reviewed + expanded 2026-07-03).

- Branch: field-intelligence-pipeline. Main: main.
- Two tracks (see plan §"Implementation Sequencing"):
  - Track A (data backbone): TA-01 … TA-10.
  - Track B (consumption): Phase A web (TB-A1..A7), B apps, C alerting, D proposals, E dispatch.

## Progress
- TA-01 committed (250e8dd): `shared/src/product_graph.rs` contract types + tests.
- TB-A1 committed (47f6f10): `/workspace` static serving via ServeDir; web shell +
  api.js + app.js; workspace_web_root config; route-manifest test (3 pass).
- TA-02 committed (5bf8306): catalog schema (catalog_sources/products/
  product_inputs/provenance_evidence, additive) + `geo_hub/src/catalog.rs`
  registry (register_product, get/list/trace_inputs/supersede);
  `tests/catalog_registry.rs` 9 pass. Identity = (kind, parameters_hash); scope
  is NOT identity (pinned by test).
- TA-03 committed (d417463): `geo_hub/src/provenance_store.rs` (append_lineage
  in-tx, load_all, trace_backward/forward). register_product writes lineage in
  the same transaction (SystemService default) + stamps provenance_id.
  GET /api/provenance/trace/:artifact_id. `tests/provenance_ledger.rs` 5 pass.
- TA-04 committed (2688b97): legacy `products` -> `catalog_products` bridge in
  `geo_hub/src/product_catalog.rs`. `backfill_products_to_catalog` (idempotent,
  leaves `products` intact); `publish_product`/`publish_georeferenced_product`
  dual-write. `legacy_product_draft` folds scene_id into parameters so two
  scenes' same kind stay distinct (L2, no input graph). `tests/catalog_backfill.rs`
  3 pass. catalog_registry(9)/provenance_ledger(5) green; products_api 183 pass /
  15 pre-existing known-red (unchanged).

- TA-05 committed (12df53e): `geo_hub/src/ingest_contract.rs` — `NormalizedIngest`
  + `commit_ingest(pool, ingest, actor, created_at)`: registers source
  (catalog_sources), upserts scene, registers L0-then-L1 catalog products with
  lineage. Idempotent; source_kind validated. `tests/ingest_contract.rs` 4 pass
  (source+scene+products, L1->L0 trace gap-free, idempotent, two-scene distinct).
  **Split:** live landsat/Sentinel call-site rewiring deferred to TA-05b.

- TA-06 committed (557e204): drone-session ingest via commit_ingest.
  `shared/src/drone_ingest.rs` (DroneIngestManifest + validate);
  `data_collector::export_ingest_manifest` (session records+checksums+health ->
  manifest); `geo_hub::ingest_contract::commit_drone_ingest` (validate, duplicate
  rejection, scene + L0 captures via commit_ingest); route
  `POST /api/ingest/drone-session`. 8 new tests (shared 4, data_collector 1,
  geo_hub route 3). All prior suites green; data_collector lib 57.

- TA-07 committed (f9cf06f): `imagery_processor/src/product_sidecar.rs` maps
  ProductReproducibilityEvidence -> ProductRecordDraft, writes
  `*.product_record.json` sidecars. Honors identity invariant (draft inputs =
  L1 band refs). 4 tests (golden map, identity, digest match, mask-before-index);
  imagery lib 19 green. **Split:** live run_* pipeline wiring -> TA-07b.

- TA-08 committed (4b50225): catalog register/read API + sidecar CLI.
  POST/GET `/api/catalog/products` (+ `/:id`) with farm/field/season/scene/
  source/level/kind/status/temporal/bbox filters; `catalog::register_sidecar_dir`
  (dependency-ordered, idempotent) + `geo_hub catalog register <dir>` CLI.
  RegisteredProduct is Serialize. 3 tests.
- TA-09 committed (3f91cd6): `post_processor/src/l3_product.rs` (to_l3_draft /
  l3_draft_from_request; L3 inputs = L2 catalog ids; confidence_from_uncertainty).
  `geo_hub/tests/l0_to_l3_trace.rs`: L0->L1->L2->L3 register + trace_backward
  reaches L0. 4 tests. Per-module wiring -> TA-09b.
- TA-10 committed (b6855d1): `ingest_contract::register_source_stub` (weather/
  iot/equipment sources); `geo_hub/tests/downstream_lineage.rs`: Report traces
  L0->L1->L2->L3->Finding->Recommendation->Report gap-free (7 records). 2 tests.
  Recommendation/report live-route lineage + lidar sidecars -> TA-10b.

## TRACK A DATA BACKBONE COMPLETE (TA-01..TA-10)
The declared Phase-0 blocker is done: product graph contract, catalog schema +
registry, provenance ledger write path + trace API, legacy dual-write bridge,
normalized ingest (satellite contract + drone route), imagery L1/L2 sidecar
mapping, catalog register/read API + CLI, L3 productization, downstream lineage
closure + source stubs. Every layer TDD-tested; full L0->Report trace gap-free.

Open Track A wiring follow-ons (mechanical, non-blocking, split out to keep the
retry/pipeline hot paths stable): TA-05b (route live landsat/Sentinel through
commit_ingest), TA-07b (call write_product_sidecar in live run_indices/masks/
thermal), TA-09b (per-analysis-module to_product_draft call sites), TA-10b
(Finding/Recommendation/Report lineage from the live create routes + lidar
sidecars).

## Known-red (user decision: proceed, track separately)
~15 geo_hub products_api acceptance tests (farm/field CRUD, shapefile, geojson)
return 500 on main and every commit — PRE-EXISTING, unrelated to refactor. Not a
gate. Per-batch verification uses targeted tests + the batch's own test file.

## Next action
Track B (consumption layers) — Track A backbone is complete. Start Phase A (web
workspace), which is independent and reuses existing geo_hub endpoints + the new
`GET /api/catalog/products`:
- TB-A2: catalog tree panel (farms -> fields -> scenes) + scene detail in
  `geo_hub/web/js/panels/catalog.js`, wired through `api.js`. Route-manifest test
  asserts api.js URL literals are all registered (extend batch-A1 test; also
  forbid non-api.js panel files from hardcoding `/api/` literals).
- Then TB-A3 map/tile layers, A4 annotations, A5 recommendations/reports+lineage,
  A6 provenance inspector (uses TA-03 trace API), A7 compare mode.
- Then Phase B (application_runs + crop_health_app), C (alert_evaluation),
  D (copilot proposals + unified queue), E (governed dispatch: wire
  dispatch_collaboration_mission_plan_route -> guarded_dispatch).
Web is static no-bundler HTML+ESM+vendored Leaflet under `geo_hub/web/`, served
at `/workspace` (TB-A1 done). Tests are Rust-side (workspace_static + route
manifest). Per-batch: cargo test -p <crate> targeted, then just gis-test.
NOTE known-red: ~15 geo_hub products_api acceptance tests fail pre-existing
(farm/field CRUD, shapefile, geojson) — not a gate; 183 others pass.

## Resume protocol
Read CLAUDE.md + this file + checkpoint.sqlite. Verify `git status --short`,
last commit, roadmap file unchanged. Continue from runs.next_action.
Checkpoint DB is source of truth; the world-sim-run-1 row is a prior unrelated run.
