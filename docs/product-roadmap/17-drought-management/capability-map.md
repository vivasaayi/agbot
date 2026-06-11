# Drought Management: Capability Map

This map is service/domain-first. Each capability is intended (greenfield); none is implemented yet. Capabilities expand across the relevant pillars (with emphasis on explainability and agronomic value) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated. Every Primary First Slice is the M1 foundation step that makes the capability real. Deterministic stress/index evidence always precedes any AI prediction.

## Drought Management Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Drought-index data model (SPI/SPEI-style) | missing (greenfield) | 8 | Persist a deterministic drought index linked to field/region with inputs cited |
| Vegetation-stress evidence (from `05`) | missing (greenfield) | 6 | Ingest one stress index from `05` as dated, georeferenced evidence |
| Satellite + weather data fusion | missing (greenfield) | 8 | Join Landsat (`07`) and weather (`15`) into one dated, georeferenced store |
| Historical baselines and seasonal trends | missing (greenfield) | 6 | Compute a per-field baseline and trend for one index |
| Per-field/region drought risk scoring | missing (greenfield) | 8 | Score deterministic risk from index + stress evidence, inspectable |
| AI drought forecast (evidence-gated) | missing (greenfield) | 6 | Forecast that cites its evidence layer and flags uncertainty |
| Early-warning and alerting | missing (greenfield) | 6 | Fire a threshold-crossing alert to `13`/`11` with evidence |
| Mitigation strategy recommendations (to `16`/`09`) | missing (greenfield) | 7 | Recommend an irrigation/advisor action tied to the risk score |
| Drought reporting | missing (greenfield) | 5 | Per-field/region drought report with evidence and trend |
| Per-field/region drought history | missing (greenfield) | 5 | Persist an auditable index/score/alert history |
