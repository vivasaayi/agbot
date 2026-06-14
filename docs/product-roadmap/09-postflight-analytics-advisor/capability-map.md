# Post-Flight Analytics and Advisor: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (agronomic value, explainability, data quality, geospatial correctness, performance and scale) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Post-Flight Analytics and Advisor Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Processing job queue and lifecycle | strong partial | 7 | Job identity linked to scene/field/season |
| Deterministic zonal statistics | partial | 8 | Compute zonal stats on a real georeferenced product |
| Anomaly flagging | missing (stubbed) | 8 | Threshold/outlier flags with reason codes |
| Zone delineation | missing (stubbed) | 7 | Delineate anomalous zones with extent and area |
| NDVI / vegetation analysis | medium partial | 7 | Index stats and coverage tied to evidence |
| Thermal analysis | medium partial | 6 | Hotspot detection with confidence and area |
| Crop health assessment | dummy | 6 | Evidence-gated health index with uncertainty |
| Yield prediction | dummy | 5 | Bounded yield estimate behind uncertainty flag |
| Recommendation generation | partial | 8 | Priority-ranked recommendation from a zone |
| Report generation (PDF) | scaffold with unimplemented encoders | 8 | Farmer-friendly PDF with metadata and findings |
| Findings export (CSV/GeoJSON) | scaffold with unimplemented encoders | 6 | Export findings and zones as CSV/GeoJSON |
| Reproducibility and evidence retention | partial | 5 | Persist raw evidence and reason codes per finding |
