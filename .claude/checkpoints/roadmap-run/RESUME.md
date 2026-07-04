# Resume — field-intel-run-1

Refactor: AGBot → layered field-intelligence pipeline.
Plan: /Users/rajanpanneerselvam/Docs/AGBOT/refactor.md (reviewed + expanded 2026-07-03).

- Branch: field-intelligence-pipeline. Main: main.
- Two tracks (see plan §"Implementation Sequencing"):
  - Track A (data backbone): TA-01 … TA-10.
  - Track B (consumption): Phase A web (TB-A1..A7), B apps, C alerting, D proposals, E dispatch.

## Progress
- TA-01 committed (250e8dd): `shared/src/product_graph.rs` contract types + tests.
- TB-A1 tests_passed (about to commit): `/workspace` static serving via ServeDir;
  `geo_hub/web/{index.html,js/api.js,js/app.js}`; `workspace_web_root` config +
  `workspace_web_dir()` resolver; route-manifest test `tests/workspace_static.rs`
  (3 tests pass); geo_hub clippy-clean.

## Next action
Commit TB-A1, then start TA-02 (catalog schema + registration):
`geo_hub/src/db.rs` additive migrations (catalog_sources, catalog_products,
catalog_product_inputs, provenance_evidence), `geo_hub/src/catalog.rs`
(register_product with mask-first ordering + unknown-input rejection + dedupe),
`geo_hub/tests/catalog_registry.rs`. TDD-first.

## Resume protocol
Read CLAUDE.md + this file + checkpoint.sqlite. Verify `git status --short`,
last commit, roadmap file unchanged. Continue from runs.next_action.
Checkpoint DB is source of truth; the world-sim-run-1 row is a prior unrelated run.
