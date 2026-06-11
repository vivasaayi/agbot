# Carbon and Sustainability Tracking: Release Plan

## Shipment Strategy

This is a greenfield (M0 named) domain, so the plan is weighted to the M1 foundation and M2 captured phases: first establish record identity and a deterministic carbon-footprint model attributed through `10`, then bring in the biomass/biodiversity evidence layers from `05`/`06`/`07`, then make outputs explainable and verifiable (M3 MRV trail), then interactive certification export (M4). Priority is mostly P2 (post-MVP) because the whole domain is sequenced after the core drone platform (`01`-`12`) and gated by the advisor MVP; only the foundational identity slice is P1.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 18 |
| M2 captured | 16 |
| M3 explainable | 14 |
| M4 interactive | 10 |
| M5 autonomous-assist | 2 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P1 | 6 |
| P2 | 44 |
| P3 | 10 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 8 |
| M | 30 |
| S | 22 |

## First P0/P1 Vertical Slices

| Phase | Size | Priority | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- | --- |
| M1 foundation | S | P1 | Sustainability record identity (via `10`) | explainability and trust | identity |
| M1 foundation | M | P2 | Carbon-footprint model | explainability and trust | evaluator |
| M2 captured | M | P2 | Biomass / canopy estimation (`06`/`05`) | geospatial correctness | capture |
| M2 captured | S | P2 | Baseline and time-series comparison | data quality | capture |
| M3 explainable | M | P2 | MRV evidence trail | explainability and trust | evaluator |
| M3 explainable | S | P2 | Biodiversity assessment from imagery | agronomic value | evaluator |
| M4 interactive | M | P2 | Certification evidence packs (via `09`) | explainability and trust | operations |

## Execution Rules

- Sequenced after the core drone platform (`01`-`12`) and gated by the advisor MVP; do not start before `10` (field/season identity) is real enough to attribute records to.
- Every output must run its deterministic carbon/biomass/KPI math and retain raw evidence before any AI summary; AI summaries cite their input layers and flag uncertainty.
- Every biomass/biodiversity output must assert CRS/extent through `07` and round-trip its georeferencing — a wrong georeference invalidates a certification claim.
- Every certification-facing output must carry a complete MRV evidence trail (inputs, method, version, georeference, audit) before it can be exported.
