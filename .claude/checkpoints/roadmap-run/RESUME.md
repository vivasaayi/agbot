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
  (dependency-ordered, idempotent, retry-defer loop) + `geo_hub catalog register
  <dir>` CLI. RegisteredProduct is Serialize. 3 tests (filters, unknown-input
  400, sidecar dependency order). All prior suites green.

Track A 1->2->3->4->5(contract)->6->7(mapping)->8 done. Next: TA-09 (post_processor
L3 productization) or TA-05b/TA-07b wiring or Track B.

## Known-red (user decision: proceed, track separately)
~15 geo_hub products_api acceptance tests (farm/field CRUD, shapefile, geojson)
return 500 on main and every commit — PRE-EXISTING, unrelated to refactor. Not a
gate. Per-batch verification uses targeted tests + the batch's own test file.

## Next action
TA-09: post_processor L3 productization. `AnalysisJobRequest` gains
`input_product_ids`; each analysis module (ndvi trend, health, thermal anomaly,
LiDAR change, index anomaly/trend, zonal stats, zone delineation/priority) gains
`to_product_draft()` producing an L3 ProductRecordDraft whose `inputs` are the L2
catalog product ids (identity invariant!); confidence from HealthUncertaintyBand
where present; `zone_recommendations` stays a Recommendation but records L3
inputs in lineage. Integration test: L3 draft -> register -> trace_backward
reaches the satellite scene's L0. Needs TA-08 (done). TDD-first
(`post_processor` tests + a geo_hub trace test). Alternatives: TA-05b / TA-07b
wiring, or a Track B phase (TB-A2 catalog tree).

## Resume protocol
Read CLAUDE.md + this file + checkpoint.sqlite. Verify `git status --short`,
last commit, roadmap file unchanged. Continue from runs.next_action.
Checkpoint DB is source of truth; the world-sim-run-1 row is a prior unrelated run.
