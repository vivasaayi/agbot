# Carbon and Sustainability Tracking: Current State and Target State

## Mission

Turn the platform's existing drone and satellite evidence into defensible environmental outputs: a carbon footprint per field and operation, biomass and biodiversity assessments, and a measurement/reporting/verification (MRV) trail strong enough to support eco-farming certification claims.

## Current Maturity

greenfield pending (M0 named): no implementation exists; this domain is a product-vision module from `product-summary.md` (#21 Carbon & Sustainability Tracking). Nothing in the repository computes a carbon footprint, biomass estimate, biodiversity score, sustainability KPI, or certification evidence pack.

## What Exists Now

- Nothing is built for this domain. There is no sustainability crate, carbon model, MRV trail, or certification export.
- Adjacent surfaces it would build on (already partially real):
  - Domain `05` (imagery and remote sensing): 12 spectral indices and sensor presets — the vegetation signal for biomass and biodiversity proxies.
  - Domain `06` (LiDAR mapping and 3D): canopy height, occupancy grids, and heatmaps — the structural input for biomass and carbon-stock estimation.
  - Domain `07` (GIS and geospatial hub): the spatial DB and CRS/extent services every output must be georeferenced through.
  - Domain `09` (post-flight analytics and advisor): the report and statistics scaffolding the certification evidence packs would extend.
  - Domain `10` (field/farm/data + season): the field, season, and operation identity every carbon/biodiversity number must attribute to. Itself greenfield-pending, so this domain is gated on it.

## Gaps to Close

- No carbon/sustainability record type owned by a field, season, and operation, with provenance and method version.
- No deterministic carbon-footprint model (per operation and per field) from logged inputs.
- No biomass/canopy estimation that consumes `06` canopy height and `05` indices with asserted georeferencing.
- No soil-carbon proxy model or its uncertainty handling.
- No biodiversity assessment from imagery (habitat/heterogeneity/cover proxies).
- No sustainability KPI catalog or tracking against targets.
- No baseline plus time-series comparison across seasons.
- No MRV evidence trail (inputs, method, version, georeference, audit) that a third party can verify.
- No certification evidence-pack export.

## Related Existing Surfaces

- Domain `05` (imagery/indices): vegetation indices feeding biomass and biodiversity proxies.
- Domain `06` (LiDAR/canopy): canopy height and structure feeding biomass and carbon-stock estimates.
- Domain `07` (GIS hub): spatial DB and CRS/extent contracts for georeferencing every output.
- Domain `09` (advisor/reports): statistics and report scaffolding the evidence packs extend.
- Domain `10` (field/season identity): the field/season/operation spine every number attributes to.
- `docs/reference/product-summary.md` (#21 Carbon & Sustainability Tracking): the source description for this module.

## Target Operating Model

- A new sustainability/MRV crate owns carbon, biomass, biodiversity, and KPI records, each attributed to a field, season, and operation through the `10` spine.
- The carbon-footprint model runs deterministically from logged operation inputs first; any AI summary cites the deterministic evidence and flags uncertainty.
- Biomass and biodiversity outputs consume `06` canopy and `05` indices, assert CRS/extent through `07`, and round-trip their georeferencing — certification needs provably correct geometry.
- A baseline plus time-series comparison makes seasonal change defensible rather than a single snapshot.
- Every output carries an MRV evidence trail: input layers, method, version, georeference, and audit IDs — so a certifier can reproduce and verify the claim.
- Certification evidence packs export the full chain through `09`, with the explainability and geospatial-correctness pillars treated as non-negotiable.
