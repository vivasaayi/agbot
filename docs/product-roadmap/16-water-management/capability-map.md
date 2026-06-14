# Water Management: Capability Map

This map is service/domain-first. Each capability is intended (greenfield); none is implemented yet. Capabilities expand across the relevant pillars (with emphasis on agronomic value and data quality) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated. Every Primary First Slice is the M1 foundation step that makes the capability real.

## Water Management Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Soil-moisture data model (sensors + RS proxies) | missing (greenfield) | 8 | Persist a moisture reading linked to field/zone with source, freshness, and QA flag |
| Remote-sensing moisture proxies (NDWI/NDMI from `05`) | missing (greenfield) | 6 | Ingest one NDWI/NDMI layer from `05` as a zone moisture proxy |
| Evapotranspiration (ET) calculation | missing (greenfield) | 7 | Compute reference ET from `15` weather inputs, deterministic and cited |
| Zone-based water-need mapping (from `09`/`05` zones) | missing (greenfield) | 7 | Map water need onto management zones consumed from `09`/`05` |
| Irrigation scheduling engine | missing (greenfield) | 9 | Generate a per-zone water plan from moisture + ET evidence |
| Weather-input contract (from `15`) | missing (greenfield) | 5 | Define and validate the ET driver inputs contract with `15` |
| Irrigation hardware/valve control interface | missing (greenfield) | 8 | Dry-run a schedule against a valve adapter with audit |
| Water-use and savings reporting | missing (greenfield) | 6 | Report applied water vs. baseline per field and zone |
| Alerts and notifications (to `11`/`13`) | missing (greenfield) | 5 | Emit a low-moisture / over-irrigation alert to `13`/`11` |
| Per-field irrigation history | missing (greenfield) | 5 | Persist an auditable per-field irrigation event log |
