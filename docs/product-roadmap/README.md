# AGBot Product Roadmap

Generated on 2026-06-11 from the current repository shape, the AGBot vision/mission, the Field Intelligence Suite product requirements (`../reference/product-requirements.md`), and the earlier milestone roadmap (`../archive/milestones-roadmap.md`).

AGBot (AgroDrone) should become an end-to-end agricultural drone platform: fly missions safely, capture LiDAR and multispectral data, derive trustworthy remote-sensing products, build 3D maps, serve geospatial layers, and turn all of it into agronomist-ready findings, recommendations, and reports. This is not just imagery processing; it is flight, capture, geospatial correctness, analysis, explainability, and a defensible field-to-decision workflow.

The near-term commercial wedge is the **Field Intelligence Suite**: the scene-to-report advisor workflow (domains 05, 07, 08, 09, 10). The flight, simulation, and coordination domains (01, 02, 03, 04, 06) are the capture-and-autonomy foundation that feeds it. Domains 11 and 12 keep the platform operable in the field.

## Roadmap Domains

Domains `01`–`12` are the **buildable core platform**, grounded in existing crates. Domains `13`–`21` are **greenfield product-vision modules** sourced from [`../reference/product-summary.md`](../reference/product-summary.md); they have no code yet (M0 named) and are sequenced after the core platform. Domains `22`–`32` are **MVP-adjacent capability extensions and cross-cutting platform subsystems** added on 2026-06-11 to close gaps the original 21 left implicit — the orthomosaic primitive, computer-vision agronomy, a first-class time-series/change-detection engine, an evidence-grounded copilot, and the trust/interop backbones (provenance, alerting, plugin SDK, import/export). They are grouped by character rather than maturity: two are core-adjacent analysis extensions, five are new product domains, and four are cross-cutting subsystems.

### Core Platform Domains (grounded in existing crates)

| Folder | Domain | Primary Crates | Maturity |
| --- | --- | --- | --- |
| `01-flight-mission-control` | Flight and Mission Control | `mission_control`, `mission_planner` | strong partial: mission CRUD, waypoints, optimization, and DB exist; MAVLink command handling and telemetry are early and untested |
| `02-simulation-digital-twin` | Simulation and Digital Twin | `drone_simulator`, `flight_sim_cpp` | medium partial: physics, multi-drone events, and the canonical C++ simulator (globe navigation, OSM/elevation terrain, OpenGL viewer) exist; scene synthesis, ray-traced camera capture, and video/telemetry streaming are planned (the Rust/Bevy `simulator` crate was removed) |
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

### MVP-Adjacent Capability Extensions (added 2026-06-11)

These close gaps the original 21 domains left implicit. The first two are **core-adjacent analysis extensions** with real upstream crates feeding them; the rest are **new product domains** that monetize the capture/analysis stack.

| Folder | Domain | Primary Crates | Maturity |
| --- | --- | --- | --- |
| `22-orthomosaic-photogrammetry` | Orthomosaic and Photogrammetry | new `orthomosaic` crate; frames in from `04`; mosaic/DSM out to `05`, `06`, `07` | core-adjacent greenfield (M0/M1): the missing drone-ag primitive — stitch hundreds of frames into one georeferenced orthomosaic + DSM/DTM via SfM; capture exists upstream (`04`), no stitching pipeline yet |
| `23-pest-disease-weed-intelligence` | Pest, Disease and Weed Intelligence (CV) | new `crop_intelligence` crate; builds on `05`, `22`; findings to `09`; weed maps to `32` | greenfield (M0): CV-ML pest/disease/weed detection plus deterministic stand count, canopy cover, and growth-stage products; the highest-WTP agronomy output, evidence-gated before any model claim |
| `24-regulatory-compliance` | Regulatory and Compliance | new `compliance` crate; builds on `01`, `04`, `10`, `30` | greenfield (M0): airspace/NFZ authorization, Remote ID logging, operator-cert and pesticide/REI records, drift/buffer-zone checks, and audit-ready compliance reports |
| `25-predictive-maintenance-fleet-health` | Predictive Maintenance and Fleet Health | new `fleet_health` crate (overlaps `12`); builds on `01`, `12`, `28`, `29` | greenfield (M0): component/airframe health, telemetry-driven degradation detection, remaining-useful-life estimates, and a pre-flight readiness gate that blocks dispatch |
| `26-agronomy-copilot` | Agronomy Copilot (LLM) | new `copilot` crate; builds on `09`, `30`, `07`, `10`, `28` | greenfield (M0): evidence-grounded natural-language Q&A that cites the provenance ledger, refuses when ungrounded, and never precedes deterministic products — the differentiator |
| `27-soil-iot-sensor-network` | Soil and IoT Sensor Network | new `soil_iot` crate; reuses `04`; delegates series to `28`; feeds `16`, `29` | greenfield (M0): ground soil-moisture/EC/temperature and micro-weather sensors that ground-truth aerial indices; calibration, QA masks, and freshness before any product |

### Cross-Cutting Platform Subsystems (added 2026-06-11)

Reusable backbones that many domains should consume instead of each reinventing them. When built, they are **threaded through earlier domains rather than bolted on**.

| Folder | Domain | Primary Crates | Maturity |
| --- | --- | --- | --- |
| `28-timeseries-change-detection` | Time-Series and Change Detection | new `timeseries` crate; builds on `07`, `10`, `05`, `06`; consumed by `09`, `15`, `16`, `17`, `19`, `25`, `27` | thin-partial → promote (M0/M3): a first-class, reusable scalar+raster time-series and change-detection engine; "what changed since last flight" made trustworthy via mandatory co-registration before any change map |
| `29-alerting-notification` | Alerting and Notification | new `alerting` crate (in/alongside `shared`); fed by `15`, `17`, `24`, `25`, `27`, `09`; surfaced via `11`, `13` | greenfield (M0): one alert rule engine + multi-channel delivery backbone with severity, dedup/anti-storm, routing/escalation, and per-fire explainability |
| `30-provenance-audit-ledger` | Provenance and Audit Ledger | new `provenance` crate (in/alongside `shared`); threaded through `04`, `05`, `06`, `09`, `22`, `23`, `28` | greenfield (M0): append-only, hash-chained capture→product→finding→report lineage; reproducibility manifests and evidence packs that underpin the copilot (`26`), carbon MRV (`19`), and compliance (`24`) |
| `31-plugin-sdk-open-data` | Plugin SDK and Open Data | new `plugin_sdk` crate; extension points in `05`, `08`, `09`, `29`, `32` | greenfield (M0): a capability-sandboxed SDK for custom indices, processors, report templates, layers, and adapters, plus open-data publishing — serves the open-source mission |
| `32-import-export-interop` | Import/Export and Interop | new `interop` crate; consolidates `09`/`geo_hub`/`geo_viewer` exports; builds on `07`, `10`, `14`, `30` | greenfield-leaning (M0): Shapefile/KML/GeoPackage/GeoTIFF round-trip with CRS fidelity, boundary import, **VRA/prescription export** (the bridge from finding to machine action), and John Deere/FieldView/Trimble interop |

### Mission-Tier Horizons (moonshots)

Documented as direction, not scheduled work. They compose existing domains rather than adding folders:

- **Closed-loop autonomy** — a deterministic change (`28`) or cited finding (`26`) auto-proposes a targeted re-fly or treatment mission that executes only after approval, tying `09` → `01`/`14`. Seeded as the M5 stories in `28` and `26`; the literal "machines that make food" loop, always approval-gated.
- **Offline-first / decentralized deployment** — local-first sync and conflict resolution for disconnected villages, extending the edge posture of `12` and the adapter model of `32`.
- **Grow-pod / vertical-farming controller + federated learning** — a controlled-environment control plane and privacy-preserving cross-farm model improvement; genuinely new platform scope, deliberately left as a horizon, not a domain.

## Foundational Docs

- `product-doctrine.md`: product positioning, the field-to-decision promise, product surfaces, and what not to build.
- `requirements-rigor.md`: definition of done, the M0–M5 maturity model, the seven product pillars, the acceptance bar, the vertical-slice contract, and open confirmation questions.
- `implementation-sequencing.md`: build phases mapped to the existing M1–M4 milestones and the cross-domain dependency logic.
- Every domain folder includes `current-state.md`, `capability-map.md`, `epics.md`, `release-plan.md` (a curated, phased backlog table rather than a generated mega-CSV), and `stories.md` (the detailed, per-slice story breakdown).

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
