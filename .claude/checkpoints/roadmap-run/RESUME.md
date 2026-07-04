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

Track A 1->2->3->4->5(contract) done. Next: TA-05b (satellite call-site wiring)
or TA-06 (drone ingest) or a Track B phase.

## Known-red (user decision: proceed, track separately)
~15 geo_hub products_api acceptance tests (farm/field CRUD, shapefile, geojson)
return 500 on main and every commit — PRE-EXISTING, unrelated to refactor. Not a
gate. Per-batch verification uses targeted tests + the batch's own test file.

## Next action
TA-05b (or TA-06 / Track B). The ingest contract (`commit_ingest`) landed in
TA-05; TA-05b routes the *live* `landsat.rs` + Sentinel-2 STAC ingest call sites
through it: build a `NormalizedIngest` from the satellite scene (SR/L2A = L1,
raw downloads = L0, cloud cover -> quality/scene), call `commit_ingest`
additively so the existing retry state machine stays untouched and green. Mock
M2M/STAC fixtures. TDD-first. Alternatively proceed TA-06 (drone session ingest:
`export_ingest_manifest()` -> `POST /api/ingest/drone-session` via commit_ingest)
or a Track B phase (TB-A2 catalog tree) as a parallel lane.

## Resume protocol
Read CLAUDE.md + this file + checkpoint.sqlite. Verify `git status --short`,
last commit, roadmap file unchanged. Continue from runs.next_action.
Checkpoint DB is source of truth; the world-sim-run-1 row is a prior unrelated run.
