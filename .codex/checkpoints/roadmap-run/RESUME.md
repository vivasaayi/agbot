# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: a3c0e52f1429f1d062e9a95e76d95f44ba739ec5
- **Last implementation commit**: 0666e86 (`batch-20260617063000`)
- **Latest checkpoint commit**: pending checkpoint commit after 0666e86 (`batch-20260617063000`)
- **Current batch**: none
- **Completed feature rows**: 466 committed; 1 tests_passed; 2 skipped; 2 blocked; 27 pending in this run.
- **Blocker**: `18-10` payments/escrow is blocked pending external provider integration and compliance approval. No blocker for current batch.

## Latest verification

- `cargo test -p shared biodiversity_proxy` — pass
- `cargo test -p geo_hub biodiversity_proxy` — pass
- `cargo test -p geo_hub biodiversity_proxies` — pass
- `cargo check -p geo_hub` — pass
- `19-06` — committed as biodiversity imagery proxy with georeferenced heterogeneity/cover metrics, uncertainty, source layer evidence, stable hashes, persisted list/get API, and explicit no-signal results for degenerate imagery
- `cargo test -p shared soil_carbon` — pass
- `cargo test -p geo_hub soil_carbon` — pass
- `cargo check -p geo_hub` — pass
- `19-07` — committed as soil-carbon proxy with weighted index/biomass/practice evidence, explicit uncertainty band for computed outputs, stable hashes, persisted list/get API, and explicit unavailable results for insufficient evidence
- `cargo test -p shared sustainability_kpi --lib` — pass
- `cargo test -p geo_hub --test products_api sustainability_kpis_compute_get_and_list_with_stable_hash` — pass
- `cargo test -p geo_hub --test products_api sustainability_kpi_no_data_persists_without_current_value` — pass
- `cargo check -p geo_hub` — pass
- `19-08` — committed as sustainability KPI tracking with deterministic target status, no-data handling for missing source values, evidence-cited hashes, persisted list/get API, and real MRV KPI output-ref validation
- `cargo test -p shared sustainability_certification_pack --lib` — pass
- `cargo test -p geo_hub --test products_api sustainability_certification_pack` — pass
- `cargo check -p geo_hub` — pass
- `19-09` — verified as certification evidence packs with shared completeness gates, persisted Geo Hub create/get API coverage, evidence layer/audit/MRV bundle assertions, and missing-MRV refusal without write
- `cargo test -p geo_hub --test products_api sustainability_field_exports` — pass
- `cargo test -p shared sustainability_export --lib` — pass (0 matching tests; shared crate compiled with export structs)
- `cargo check -p geo_hub` — pass
- `19-10` — verified as field sustainability export/reporting with CSV row parity, GeoJSON CRS/extent features, PDF method/evidence citations, and valid empty-field artifacts
- `cargo test -p shared content_workflow --lib` — pass
- `cargo test -p geo_hub --test products_api content_workflow` — pass
- `cargo check -p geo_hub` — pass
- `20-02` — verified as authoring/editorial workflow with draft → review → publish transitions, audit rows with actor/timestamp, non-editor publish denial, skip-review refusal, and future scheduled publish staying in review
- `cargo test -p shared content_permissions --lib` — pass
- `cargo test -p geo_hub --test products_api content_permissions` — pass
- `cargo test -p geo_hub --test products_api content_workflow` — pass
- `cargo check -p geo_hub` — pass
- `20-03` — verified as CMS permission resolution from org-scoped role refs, editor publish capability, cross-org no-access resolution, and viewer workflow write denial with audit
- `cargo test -p shared content_search --lib` — pass
- `cargo test -p geo_hub --test products_api content_search` — pass
- `cargo check -p geo_hub` — pass
- `20-04` — verified as published-only content search with deterministic term ranking, org scoping, draft exclusion, result article links, and empty no-match results
- `cargo test -p shared content_taxonomy --lib` — pass
- `cargo test -p geo_hub --test products_api content_tags` — pass
- `cargo check -p geo_hub` — pass
- `20-05` — verified as controlled crop/region/topic taxonomy tagging, editor-confirmed AI suggestions only, tag-filtered content retrieval, and off-taxonomy rejection
- `cargo test -p shared content_portal_embed --lib` — pass
- `cargo test -p geo_hub --test products_api content_portal_embed` — pass
- `cargo test -p geo_hub --test products_api content_` — pass
- `cargo check -p geo_hub` — pass
- `20-06` — committed as read-only grower portal knowledge-base embedding with same-org `cms:viewer` visibility, published-only list/open routes, evidence refs, and 403/404 non-leakage for cross-org readers, drafts, and foreign content
- `cargo test -p shared content_engagement --lib` — pass
- `cargo test -p geo_hub --test products_api content_engagement` — pass
- `cargo test -p geo_hub --test products_api content_` — pass
- `cargo check -p geo_hub` — pass
- `20-07` — committed as content engagement analytics with persisted view/read/helpful-vote events, deterministic per-period summary rows, evidence refs, and zero summaries for published items with no activity

## Next action

- Select and claim the next pending feature after `20-07` content engagement analytics; next pending is `20-08` success-story publishing.
