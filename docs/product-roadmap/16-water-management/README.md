# Water Management

Intelligent irrigation: monitor soil moisture, compute evapotranspiration, and schedule water by management zone so every field gets the right amount of water at the right time.

## Where We Are

- Not started / vision only. This is a greenfield product-vision module sourced from `docs/reference/product-summary.md` (#11 Water Management); no code exists.
- The platform spine it consumes is partially real: moisture-related indices (NDWI/NDMI) belong to imagery and remote sensing (`05`), weather and ET inputs come from weather advisory (`15`), management zones come from the advisor (`09`) and imagery (`05`), field identity comes from the field/farm spine (`10`), and geospatial correctness comes from the GIS hub (`07`).
- This is an agronomic-decision module: its job is to turn moisture evidence into a defensible irrigation action, not to be a sensor dashboard.

## Where We Should Be

- A field carries a soil-moisture data model that fuses ground sensors with remote-sensing proxies (NDWI/NDMI from `05`), each reading dated, located, and quality-flagged.
- Evapotranspiration is computed deterministically from weather inputs (`15`) before any recommendation, with the method and inputs cited.
- An irrigation scheduling engine produces a per-zone water plan (consuming management zones from `09`/`05`), with water-use and savings reporting and alerts routed to the portal (`13`) and operator surfaces (`11`).
- A hardware/valve control interface can execute or dry-run a schedule against irrigation equipment, gated by the same safety and audit discipline as flight actions.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P1 slices.

## Build Order

1. Soil-moisture data model: sensor + remote-sensing readings linked to field/zone via `10`/`07`, freshness and QA flagged.
2. Evapotranspiration calculation from weather inputs (`15`), deterministic and inspectable.
3. Zone-based water-need mapping that consumes management zones from `09`/`05`.
4. Irrigation scheduling engine producing a per-zone water plan with evidence.
5. Water-use and savings reporting plus alerts routed to `13`/`11`.
6. Irrigation hardware/valve control interface with dry-run, execute, and audit; per-field history.

## Primary Crates

New crate(s) TBD (a water-management/irrigation engine plus a moisture data store). Builds on domains `05` (moisture indices), `15` (weather/ET inputs), `09` (management zones), `10` (field identity), and `07` (geospatial correctness).
