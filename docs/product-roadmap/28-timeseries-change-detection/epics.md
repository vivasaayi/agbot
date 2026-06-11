# Time-Series and Change Detection: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: time-series append/query routes or commands and change-detection job submit/status/result, with pagination, freshness, and audit IDs.
- Deterministic: alignment/co-registration, per-pixel and zonal change, baseline/seasonality, and change-event ranking computed without AI, with reason codes and raw evidence retained.
- Geospatial: every two-date comparison asserts CRS, extent, and resolution and proves co-registration; change masks and zones round-trip as GeoTIFF/GeoJSON in the correct CRS.
- Explainability: every change output cites its evidence layer (the two source series/scenes and the alignment proof); forecast/gap-fill flags uncertainty.
- Reusability: the engine is generic across `(entity, metric, time)`; consumer domains (`09`/`15`/`16`/`17`/`19`/`25`/`27`) plug in without forking it.
- Tests: unit (delta/slope/baseline/alignment math), fixture (two-date scene pairs, multi-date series), API contract, and one failure path (uncoregistered pair refused).
- Operations: job health, retry/backoff, large-raster/long-series performance budget, and a runbook.

## Category Epics

### EPIC-01: Generic Time-Series Engine
- Goal: a reusable store and API for scalar and raster series keyed by `(entity, metric, time)`.
- First release: the `timeseries` store (scalar + raster) and an append/query API, with one real consumer (`09` vegetation trend) ported onto it.
- Expansion: zonal trend/slope, rolling baseline, and season-over-season comparison; additional scalar consumers (`15`/`16`/`17`/`25`/`27`).
- Hardening: long-series performance, retention/compaction, and determinism tests (same inputs → same series and trend).

### EPIC-02: Co-Registration-Gated Change Detection
- Goal: deterministic two-date change with a hard co-registration guard — no change map without alignment proof.
- First release: temporal alignment/co-registration onto a common grid/CRS/resolution, an alignment QA guard that refuses non-co-registerable pairs, and per-pixel delta + threshold change masks.
- Expansion: normalized change, zonal change vs baseline, and ranked change events with retained evidence and reason codes.
- Hardening: large-raster performance, negative-path coverage (CRS/extent/resolution mismatch, partial overlap), and evidence reproducibility.

### EPIC-03: Consumers, Export, and Bounded Autonomy
- Goal: turn series and change into shareable products and an approval-gated action.
- First release: time-series CSV, change-mask GeoTIFF, and change-zone GeoJSON export, plus the compare-view feed (aligned pair + change mask) to `08`.
- Expansion: forecast/gap-fill with an uncertainty band, and the remaining consumer integrations (`19` carbon stock, `25` RUL trend).
- Hardening: the closed-loop hook — a significant detected change auto-proposes an approval-gated re-fly/treatment mission (`09`→`01`/`14`) — gated behind reliable deterministic change and tested refusal paths.
