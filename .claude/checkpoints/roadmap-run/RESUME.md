# Resume — field-intel-run-1

## CURRENT: satellite intelligence pipeline (2026-07)
Active plan + per-batch ledger: `docs/design/satellite-intelligence-pipeline.md`
(the ledger there is authoritative; batches 1-15 committed). Last: batch 15
(ae96756) — Otsu water-body extraction (post_processor/src/water_extent.rs +
geo_hub/src/water_extent_rasters.rs, POST /api/water-management/extent/derive,
binary-mask colormap; closes Phase 3 item 9's water half). Batch 14
(c5865b9) — dNBR burn-severity (post_processor/src/burn_severity.rs +
geo_hub/src/dnbr_rasters.rs, POST /api/change-detection/dnbr/derive, dnbr
diverging colormap). Batch 13 (224585d): Sen2Cor orchestration. Batch 12
(51e842b): CHIRPS dekads + fetcher. Batch 11 (a292632): SPI-N windows.
Batch 10 (ab688b0): phenology + land-cover. Batch 9 (b131e4e): CHIRPS + SPI.
Batch 8 (dde2610): climatology + VCI. Batch 7 (2bf6464): Web Mercator tiler.
Phases 1-3 (incl. water extent) + Sen2Cor + dNBR COMPLETE. NEXT: batch 16 =
remaining Phase 4 (crop-type ML feature export + inference seam — needs
WorldCover/WorldCereal bootstrap first; Sentinel-1 SAR water; HLS). Open: JP2 decode for sen2cor bands; LST
path for TCI/VHI; WorldCover bootstrap validation; /browse derive
affordances. Everything below is the earlier (completed) Track A/B refactor
history.

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

## Track B Phase A (web workspace) progress
- TB-A1 (47f6f10): /workspace static serving + shell + route-manifest test.
- TB-A2 (5a17861): catalog tree panel (farms->fields->scenes + scene detail);
  single-URL-file convention test added.
- TB-A3 (f6bc734): Leaflet 1.9.4 vendored (web/vendor/leaflet); map.js tile-layer
  factory over the product tile route; layers.js toggle/opacity per scene.
- TB-A4 (8be8440): annotations.js read/write/link (click-to-place point, delete,
  markers on map); map.js annotation-marker + captureNextClick helpers.
Web pattern: all backend URLs in web/js/api.js (enforced by tests); panels in
web/js/panels/; 5 workspace_static tests. Remaining Phase A: A5 recommendations+
reports+lineage, A6 provenance inspector, A7 compare mode. Then B/C/D/E.

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

## Track B Phase B (applications) progress
- TB-B1 (9e179cf): application_runs + findings with provenance (record_run).
- TB-B2 (2420580): crop_health_app pure composition in post_processor.
- TB-B3 (7ab05dc): crop_health run trigger + findings panel.
  Server: geo_hub/src/crop_health_run.rs maps post_processor CropHealthFinding
  -> ApplicationFinding, records via applications::record_run (new geo_hub->
  post_processor dep). Route POST /api/applications/crop-health/runs. Run inputs
  = dedup union of zones' cataloged L2/L3 ids; lineage per-finding to those.
  kind: declining_zone / unhealthy_zone / healthy_zone; severity from priority.
  tests/crop_health_run.rs 2 pass (two-scene declining+lineage, reject uncataloged).
  Web: web/js/panels/findings.js (list field findings + run-trigger form that
  resolves the field's L2 NDVI products as inputs); api.js fieldFindings +
  cropHealthRuns; app.js/index.html wired. workspace_static 5 green.

- TB-B4 (55727bd): water_priority application run + app picker.
  post_processor/src/water_priority_app.rs (pure, mirrors crop_health_app):
  mean soil moisture -> WaterStress bands, deficit(mm)+stress -> needs_irrigation
  (saturated never irrigates), area -> priority. geo_hub/src/water_priority_run.rs
  -> ApplicationFinding via record_run; route POST /api/applications/water-priority/runs
  (app_id water_priority). kind: water_deficit_zone / adequate_moisture_zone.
  tests/water_priority_run.rs 2 pass. Web: findings.js run trigger generalized to
  an app picker (APPS registry: productKind ndvi|soil_moisture, per-app zone
  fields); api.js waterPriorityRuns.

- TB-B5 (2f32978): anomaly-detection application run.
  post_processor/src/anomaly_app.rs (pure): per-zone absolute low/high thresholds
  then statistical band (mean ± multiplier·std over the run's zones), reusing
  ProductAnomalyReasonCode. geo_hub/src/anomaly_run.rs -> ApplicationFinding via
  record_run; route POST /api/applications/anomaly/runs (app_id anomaly_detection).
  kind: index_anomaly_zone (Track C alert input) / nominal_zone. tests 2 pass.
  Web: findings.js app picker anomaly_detection entry (index_value, ndvi kind);
  api.js anomalyRuns.

## Phase B COMPLETE (TB-B1..B5)
application_runs + findings with provenance, crop_health, water_priority, and
anomaly_detection applications — all pure post_processor compositions mapped to
governed geo_hub runs via applications::record_run, each finding lineage-traced
to its cataloged L2/L3 inputs. Web findings panel lists any field's findings +
an app-picker run trigger over all three apps. Pattern (copy for new apps):
post_processor/src/<app>_app.rs (pure) + geo_hub/src/<app>_run.rs (record_run
mapping) + route + tests/<app>_run.rs + an APPS entry in web/js/panels/findings.js.

## Track C (alerting) progress
- TB-C1 (1aacc92): alert evaluation over findings.
  geo_hub/src/alert_evaluation.rs screens list_field_findings -> alerts via
  alerting::evaluate_alert_rules; persists to new fired_alerts table with lineage
  alert->finding (ArtifactKind::Alert added to provenance). Default ruleset:
  index_anomaly_zone=critical, water_deficit_zone/declining_zone=warning.
  Idempotent (alert id + lineage keyed by finding+rule). Routes POST
  /api/fields/:field_id/alert-evaluation, GET /api/fields/:field_id/alerts.
  tests/alert_evaluation.rs 3 pass. Web: panels/alerts.js (list + evaluate
  trigger); api.js fieldAlerts/fieldAlertEvaluation.
  Reusable alerting fns not yet used: deduplicate_alert_stream,
  classify_alert_severity, route_alert_to_recipients, open/ack/resolve lifecycle.

- TB-C2 (cace70e): alert lifecycle.
  geo_hub/src/alert_lifecycle.rs persists fired->acknowledged->resolved per alert
  via alerting::{open_alert_lifecycle,acknowledge_alert,resolve_alert} (engine
  enforces order + idempotency). New alert_lifecycle table (state + transition
  log); alert_evaluation::get_fired_alert reconstructs FiredAlertRecord. Routes
  GET /api/alerts/:id/lifecycle, POST .../acknowledge, POST .../resolve
  { actor_id }. tests/alert_lifecycle.rs 4 pass. Web: alerts.js state tag +
  Ack/Resolve buttons; api.js alertLifecycle/alertAcknowledge/alertResolve.

- TB-C3 (f754a65): evidence-based severity classification.
  alert_evaluation classifies each fired alert via alerting::classify_alert_severity
  from finding metrics (anomaly |z_score| 1.5/2.5/4.0; water deficit mm 5/15/30;
  ndvi decline 0.05/0.10/0.20), overriding the static rule severity for downstream
  while retaining rule severity for audit. New alert_severity_classification table;
  StoredAlert.classified_severity (LEFT JOIN); GET /api/alerts/:id/severity.
  tests/alert_severity.rs 2 pass (35mm->emergency, z~2.0->warning). Web: alerts.js
  shows classified severity. Note: anomaly z is normalized so max |z| over n zones
  ~ sqrt(n-1); 5 zones caps at ~2.0 (warning) — emergency needs ~17 zones.

## TRACK C COMPLETE (TB-C1..C3)
Findings -> alerts (rule engine, lineage) -> governed lifecycle (open/ack/resolve)
-> evidence-based severity. All reuse the `alerting` crate; alerts trace
alert->finding->L2->L0 gap-free. Unused alerting fns remain for later: dedup,
routing (route_alert_to_recipients/evaluate_alert_preference), escalation
(evaluate_no_ack_escalation), delivery adapters.

## TRACK D (proposals) COMPLETE (TB-D1..D4)
- TB-D1 (21c29a2): geo_hub/src/proposal_queue.rs unified accept/reject queue;
  ArtifactKind::Proposal + provenance_store::get_lineage; proposals table; routes
  list/field/get/accept/reject; 3 tests. Proposals lineage-close to L0.
- TB-D2 (bdf45b5): copilot/src/advisor_rules.rs pure deterministic evaluators
  (evaluate_water_stress_rule, evaluate_pest_hotspot_rule) -> AdvisorProposal
  drafts; RemedyKind::action_category. 5 tests.
- TB-D3 (0377855): web/js/panels/proposals.js field proposal queue with
  accept/reject (reviewer identity); api.js proposal endpoints; catalog passes
  fieldId. workspace_static 5.
- TB-D4 (be8f70c): geo_hub/src/proposal_adapters.rs funnels advisor +
  CropClosedLoopProposal drafts into the queue (finding-sourced); accepted ->
  RecommendationRecord (author copilot-advisor). CropClosedLoopApprovalStatus
  extended (Approved/Rejected + parse). 8 tests.

## TRACK E (governed dispatch) COMPLETE (TB-E1..E4)
- TB-E1 (1d367b0): geo_hub/src/proposal_mission.rs draft_mission_for_proposal —
  accepted proposal -> inert MissionPlanDraft (action->MissionKind; desk work not
  flyable; dispatch_authorized always false). 5 tests.
- TB-E2 (5baee74): same module — dry_run_mission_dispatch + authorize_mission_
  dispatch. Separation of duties: operator != accepting reviewer; dispatch_
  authorized flips true only on distinct affirmative approval. 4 tests.
- TB-E3 (9a003a1): geo_hub/src/proposal_dispatch.rs governed_dispatch — refuse
  unless dispatch_authorized (no token/flag bypass), invoke mission_planner::
  dispatch_guarded_simulation_command, append Action lineage (action:<draft_id>
  <- source_proposal_id). geo_hub gains mission_planner dep. 3 unit + 2 e2e tests
  (tests/governed_dispatch.rs: accepted->draft->approve->guarded dispatch, Action
  traces to L0; unauthorized never dispatches).
- TB-E4 (d1d48e4): ground_station_ui/src/dispatch_advisory.rs read-only operator
  advisory (stages: awaiting_review/rejected/accepted_no_mission/awaiting_operator
  _approval/dispatch_authorized/blocked_operator_conflict). Advisory only — never
  dispatches; reflects separation-of-duties block. 6 tests.

## Track A wiring follow-ons
- TA-09b (8f84fa6): post_processor AnalysisResult::to_product_draft +
  ResultType::product_kind — analysis result self-describes as an L3 draft
  (delegates to l3_draft_from_request; identity invariant preserved). 2 tests.
- REMAINING (next batch, a coherent live-route wiring set):
  - TA-07b: the imagery sidecar API (product_sidecar::draft_from_evidence +
    write_product_sidecar) is DANGLING (no callers). Wire it into the live
    pipelines (indices.rs process_one ~line 549-596 builds reproducibility +
    out_path; call draft_from_evidence+write_product_sidecar before meta move).
    Design decision: how to derive L1 input refs + ProductScope in the CLI path
    (no catalog product ids there — derive from evidence.resolved_bands + image_id
    scene). Also masks/thermal/classify.
  - TA-05b: route live landsat/Sentinel ingest through commit_ingest.
  - TA-10b: Finding/Recommendation/Report lineage from live geo_hub create routes
    + lidar_mapper sidecars.
  Then full-workspace `cargo check` (use CARGO_INCREMENTAL=0; disk tight) +
  `just gis-test`.

## Disk note
Build ran target/ to 100% (No space left on device). Recovered by removing
target/debug/incremental (24G) — NOT cargo clean; built rlibs in target/debug/deps
preserved. Use CARGO_INCREMENTAL=0 on cargo runs to avoid regrowth (~18G free).

Per-batch gate: cargo test -p <crate> targeted + workspace_static; 15
products_api acceptance tests remain pre-existing known-red (183 pass).

## (historical) Phase A next action
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
