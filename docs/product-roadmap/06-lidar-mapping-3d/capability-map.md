# LiDAR Mapping and 3D Reconstruction: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (data quality, geospatial correctness, agronomic value, performance/scale, explainability) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## LiDAR Mapping and 3D Reconstruction Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Scan ingest and parallel loading | strong partial | 6 | Ingest RPLIDAR scans with freshness and coverage |
| Occupancy grid construction | strong partial | 8 | Configurable-resolution grid with asserted extent |
| Point-cloud export (PCD) | strong partial | 5 | PCD v0.7 export with provenance metadata |
| Obstacle heatmap | strong partial | 5 | Blue → red obstacle heatmap with thresholds |
| Outlier and noise removal | missing | 7 | Statistical outlier removal before product derivation |
| Normal estimation | missing | 6 | Per-point normals for surface orientation |
| Ground/non-ground segmentation | missing | 6 | Separate ground plane from canopy and obstacles |
| Clustering and segmentation | missing | 7 | Cluster obstacle and canopy objects |
| DSM/DTM elevation products | missing | 8 | Rasterize cleaned cloud into a georeferenced DSM |
| Canopy-height model | missing | 6 | Derive canopy height (DSM − DTM) for the advisor |
| 3D reconstruction and meshing | missing | 7 | Mesh the cleaned cloud for terrain rendering |
| Elevation/obstacle overlays | strong partial | 6 | Publish elevation/intensity overlays to the viewer |
