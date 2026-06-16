# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: 2193164 (`batch-20260616000100`)
- **Latest checkpoint commit**: this checkpoint commit after 2193164 (`batch-20260616000100`)
- **Current batch**: none
- **Completed feature rows**: 458 committed; 1 tests_passed; 2 skipped; 2 blocked; 35 pending in this run.
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

## Next action

- Select and claim the next pending feature after `19-08` sustainability KPI tracking; next pending is `19-09`.
