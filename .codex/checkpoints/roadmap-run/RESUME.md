# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: a3c0e52f1429f1d062e9a95e76d95f44ba739ec5
- **Last implementation commit**: 99bf815 (`batch-20260617052656`)
- **Latest checkpoint commit**: this checkpoint commit after 99bf815 (`batch-20260617052656`)
- **Current batch**: none
- **Completed feature rows**: 461 committed; 1 tests_passed; 2 skipped; 2 blocked; 32 pending in this run.
- **Blocker**: `18-10` payments/escrow is blocked pending external provider integration and compliance approval.

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

## Next action

- Select and claim the next pending feature after `20-02` authoring/editorial workflow; next pending is `20-03`.
