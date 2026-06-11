# Carbon and Sustainability Tracking

Turn the platform's drone and satellite data into defensible environmental evidence: carbon footprint per field and operation, biomass and biodiversity assessment, and auditable sustainability reporting for eco-farming certifications.

## Where We Are

- Not started / vision only. This is a greenfield product-vision module sourced from `docs/reference/product-summary.md` (#21 Carbon & Sustainability Tracking); no code exists.
- The evidence layers it consumes are partially real: spectral indices (`05`), LiDAR canopy/biomass (`06`), the geospatial hub (`07`), and the analytics/report scaffolding (`09`). The field/season identity spine (`10`) it must attribute everything to is itself greenfield-pending.
- This is an evidence and reporting domain, not a sensing domain: it computes nothing the drone/satellite stack does not already capture; its value is correct attribution, baselining, and an auditable measurement/reporting/verification (MRV) trail.

## Where We Should Be

- Every field and operation carries a carbon-footprint estimate tied to its `10` identity and `09` evidence, with reason codes and the input layers cited.
- Biomass/canopy (from `06`) and vegetation indices (from `05`) feed defensible biomass and biodiversity assessments, georeferenced through `07`.
- A baseline plus time-series comparison shows change over seasons, and an MRV evidence trail makes every number traceable back to its source scene and method.
- Certification evidence packs export the full chain (inputs, method, version, georeference, audit) so a third party can verify a sustainability or eco-certification claim.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P1 slices.

## Build Order

1. Carbon/sustainability record identity, attributed to field/season/operation via `10`.
2. Carbon-footprint model per operation from logged inputs (deterministic, evidence-cited).
3. Biomass/canopy estimation consuming `06` canopy height and `05` indices, georeferenced via `07`.
4. Baseline plus time-series comparison and sustainability KPI tracking.
5. Biodiversity assessment from imagery and soil-carbon proxies.
6. MRV evidence trail and certification evidence-pack export (via `09`).

## Primary Crates

Planned `sustainability` crate (a sustainability/MRV service plus report encoders). Builds on domains `05` (indices), `06` (biomass/canopy), `07` (geospatial), `09` (analytics/reports), and `10` (field/season identity). Sequenced after the core drone platform (`01`-`12`) and gated by the advisor MVP.
