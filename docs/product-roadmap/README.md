# AGBot Product Roadmap

Generated on 2026-06-11 from the current repository shape, the AGBot vision/mission, the Field Intelligence Suite product requirements (`../reference/product-requirements.md`), and the earlier milestone roadmap (`../archive/milestones-roadmap.md`).

AGBot (AgroDrone) should become an end-to-end agricultural drone platform: fly missions safely, capture LiDAR and multispectral data, derive trustworthy remote-sensing products, build 3D maps, serve geospatial layers, and turn all of it into agronomist-ready findings, recommendations, and reports. This is not just imagery processing; it is flight, capture, geospatial correctness, analysis, explainability, and a defensible field-to-decision workflow.

The near-term commercial wedge is the **Field Intelligence Suite**: the scene-to-report advisor workflow (domains 05, 07, 08, 09, 10). The flight, simulation, and coordination domains (01, 02, 03, 04, 06) are the capture-and-autonomy foundation that feeds it. Domains 11 and 12 keep the platform operable in the field.

## Roadmap Domains

Domains `01`–`12` are the **buildable core platform**, grounded in existing crates. Domains `13`–`21` are **greenfield product-vision modules** sourced from [`../reference/product-summary.md`](../reference/product-summary.md); they have no code yet (M0 named) and are sequenced after the core platform.

### Core Platform Domains (grounded in existing crates)

| Folder | Domain | Primary Crates | Maturity |
| --- | --- | --- | --- |
| `01-flight-mission-control` | Flight and Mission Control | `mission_control`, `mission_planner` | strong partial: mission CRUD, waypoints, optimization, and DB exist; MAVLink command handling and telemetry are early and untested |
| `02-simulation-digital-twin` | Simulation and Digital Twin | `drone_simulator`, `simulator`, `flight_sim_cpp` | medium partial: physics, multi-drone events, Bevy globe, and a C++ viewer with OSM/elevation terrain exist; 3D terrain and GIS integration are in progress |
| `03-multi-drone-coordination` | Multi-Drone Coordination | `multi_drone_control` | early partial: strong type model, geofence/altitude safety checks; formation optimization and collision avoidance algorithms are stubbed |
| `04-sensor-acquisition` | Sensor Acquisition and Data Capture | `sensor_collector`, `data_collector` | medium partial: hardware/sim abstraction, session lifecycle, storage and indexing exist; real-hardware paths untested, some exports and aggregates stubbed |
| `05-imagery-remote-sensing` | Imagery and Remote Sensing | `imagery_processor`, `sensor_overlay_engine` | early-to-strong partial: 12 spectral indices, sensor presets, thermal/mask CLI, and overlay colormaps defined; index/thermal/classify pipelines are scaffolded |
| `06-lidar-mapping-3d` | LiDAR Mapping and 3D Reconstruction | `lidar_mapper`, `sensor_overlay_engine` | strong partial: occupancy grids, heatmaps, PCD export, and tests exist; outlier removal, normals, and clustering are missing |
| `07-gis-geospatial-hub` | GIS and Geospatial Hub | `geo_hub`, `shared` | strong partial: axum server, routes, spatial DB, and Landsat client exist; CRS/extent contracts and real ingestion need hardening |
| `08-geo-viewer-visualization` | Geo Viewer and Visualization | `geo_viewer`, `sensor_overlay_engine` | early partial: Bevy plugin architecture for map, annotations, recommendations, and reports; rendering and workflow logic are thin |
| `09-postflight-analytics-advisor` | Post-Flight Analytics and Advisor | `post_processor` | medium partial: job queue, statistics, recommendations, and report scaffolding exist; analysis algorithms and report encoders are simplified/dummy |
| `10-field-farm-data-management` | Field, Farm, and Data Management | `shared`, new domain crate | greenfield pending: the Organization/Farm/Field/Boundary/Scene/Layer/Annotation/Recommendation/Report domain model is mostly missing per the PRD |
| `11-ground-station-ui` | Ground Station UI | `ground_station_ui` | early partial: WebSocket client and message dispatch work; web and CLI surfaces are frameworks with minimal interactivity |
| `12-fleet-edge-operations` | Fleet and Edge Operations | `shared`, deployment | early partial: config, runtime modes, logging, Docker, and ARM cross-compile exist; enrollment, OTA, health, and observability depth are missing |

### Adjacent Product Vision Domains (greenfield, M0)

These are aspirational modules from `../reference/product-summary.md` with no implementation yet. They are documented at M0 depth and gated behind the advisor MVP and the core platform (`01`–`12`).

| Folder | Domain | Primary Crates | Maturity |
| --- | --- | --- | --- |
| `13-farmers-portal` | Farmers Portal | new crate TBD; builds on `09`, `10`, `07`, `08` | greenfield (M0): grower-facing dashboard + mobile app; report inbox, recommendation tracking, marketplace/community entry |
| `14-autonomous-tractor` | Autonomous Tractor (AruviTrac) | new crate TBD; parallels `01`, `03`; builds on `07`, `10`, `05`, `09` | greenfield (M0): self-driving tractor with GPS guidance, implement and prescription-map execution; reuses flight mission/safety patterns |
| `15-weather-advisory` | Weather Advisory | new crate TBD; builds on `10`; feeds `01`, `14`, `16`, `17` | greenfield (M0): hyper-local per-field forecast, spray/flight windows, frost/heat/wind risk alerts |
| `16-water-management` | Water Management | new crate TBD; builds on `05`, `15`, `09`, `10`, `07` | greenfield (M0): soil-moisture, evapotranspiration, irrigation scheduling, and water-use reporting |
| `17-drought-management` | Drought Management | new crate TBD; builds on `05`, `07`, `15`, `16`, `09` | greenfield (M0): drought indices, satellite+weather fusion, early warning, and mitigation strategy |
| `18-supply-chain-marketplace` | Supply Chain and Marketplace | new crate TBD; builds on `10`, `13`, `09` | greenfield (M0): input/produce marketplace, procurement, inventory, and demand forecasting; furthest from current code |
| `19-carbon-sustainability` | Carbon and Sustainability | new crate TBD; builds on `05`, `06`, `07`, `09`, `10` | greenfield (M0): carbon footprint, biomass/biodiversity, MRV and certification evidence packs |
| `20-content-management` | Content Management | new crate TBD; builds on `13`, `10` | greenfield (M0): blog, knowledge base, and community content; most decoupled from the drone/geo stack |
| `21-realtime-collaboration` | Real-time Collaboration | new crate TBD; builds on `01`, `04`, `08`, `11`, `10`, `12` | greenfield (M0): team messaging, live drone video, collaborative mission planning, and emergency alerts |

## Foundational Docs

- `product-doctrine.md`: product positioning, the field-to-decision promise, product surfaces, and what not to build.
- `requirements-rigor.md`: definition of done, the M0–M5 maturity model, the seven product pillars, the acceptance bar, the vertical-slice contract, and open confirmation questions.
- `implementation-sequencing.md`: build phases mapped to the existing M1–M4 milestones and the cross-domain dependency logic.
- Every domain folder includes `current-state.md`, `capability-map.md`, `epics.md`, and `release-plan.md` (a curated, phased backlog table rather than a generated mega-CSV).

## Product Principles

- Evidence before advice: deterministic processing (indices, occupancy, statistics) must run and be inspectable before any AI summary or yield/health claim.
- Geospatial correctness is mandatory: CRS, extent, resolution, and georeferencing must be preserved end to end; a wrong overlay is worse than no overlay.
- Workflow over features: solve the scene-to-report advisor workflow end to end before broadening scope.
- Capture feeds decisions: flight, sensors, LiDAR, and imagery exist to produce field findings, not dashboards for their own sake.
- Safety is non-negotiable: every flight and multi-drone action needs geofence, altitude, no-fly-zone, and battery guardrails with dry-run and abort.
- Explainability over black-box AI: outputs must be defensible, cite their evidence layer, and flag uncertainty.
- Field relevance: every major capability should tie back to a real farm action (scout, treat, irrigate, re-fly, report).
- Edge-ready: the platform must run on Jetson/Raspberry Pi class hardware in simulation and flight modes, not only on a developer laptop.
- Production discipline: tests, observability, and auditability are part of the product, added during a milestone rather than after it.

## Source References

- Reference docs (`../reference/`): `vision-mission.md`, `product-summary.md`, `architecture-technical.md`, `gis-implementation-summary.md`, `terrain-viewer-guide.md`, plus the repository root `README.md`.
- Earlier milestone planning (`../archive/`): `milestones-roadmap.md`, `engineering-breakdown.md`, `execution-checklist.md`, and milestone briefs 01–04. The live product requirements doc is `../reference/product-requirements.md`.
- MAVLink protocol: https://mavlink.io/en/
- Sentinel-2 / Landsat band references for spectral indices: https://www.usgs.gov/landsat-missions and https://sentinels.copernicus.eu/web/sentinel/missions/sentinel-2
- NDVI and vegetation indices background: https://www.usgs.gov/special-topics/remote-sensing-phenology/science/ndvi-foundation-remote-sensing-phenology
- OGC geospatial standards (CRS, GeoTIFF, GeoJSON): https://www.ogc.org/standards/
</content>
