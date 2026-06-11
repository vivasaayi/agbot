# Post-Flight Analytics and Advisor: Current State and Target State

## Mission

Turn deterministic remote-sensing products into anomalies, findings, recommendations, and grower-ready reports: reduce the time from scene captured to report delivered and recommended action created.

## Current Maturity

medium partial: `post_processor` has a real job queue, rich result statistics, a recommendation model, and a report-generator scaffold, but the core analysis algorithms are simplified or dummy and the report encoders are unimplemented (TODOs).

## What Exists Now

- `ProcessingJob` queue with status (Queued/Processing/Completed/Failed/Cancelled) and parameters (`post_processor/src/lib.rs`).
- Job types: NdviAnalysis, LidarProcessing, ThermalAnalysis, MultiSpectralAnalysis, CompositeReport, HealthAssessment, YieldPrediction.
- Processors: `NdviAnalysisProcessor`, `LidarAnalysisProcessor`, `ThermalAnalysisProcessor` with config types (`ndvi_analysis.rs`, `lidar_analysis.rs`, `thermal_analysis.rs`).
- `AnalysisResult` with `AnalysisStatistics` (min/max/mean/std/percentiles/coverage) and `ResultData` variants (GridData, PointData, ZonalData, TimeSeriesData).
- `Recommendation` records with priority and action items.
- `ReportGenerator` targeting PDF/HTML/JSON/CSV/KML/Shapefile (`report_generator.rs`).

## Gaps to Close

- `process_multispectral`, `assess_crop_health`, and `predict_yield` return placeholder data (`lib.rs:306/392/444`).
- `ReportGenerator` has TODOs for data collection, content generation, format-specific encoding, page count, quality scoring, and delivery (`report_generator.rs:528/544/559/572/577/586`).
- Anomaly detection and zone delineation are not real (thermal `TODO: Implement anomaly detection`, `thermal_analysis.rs:699`; thermal patterns empty).
- Confidence/quality scores are hardcoded constants (`ndvi_analysis.rs:309`, `lidar_analysis.rs:188`).
- No consumption of real georeferenced products from `05`/`06` or field/season context from `10`.
- Recommendations are not yet persisted into the `10` domain model or linked to annotations.
- No tests on the analysis math or report encoders.

## Source Modules Reviewed

- `post_processor/src/lib.rs` (job queue, JobType, AnalysisResult, ResultData, process/assess/predict stubs)
- `post_processor/src/ndvi_analysis.rs`, `thermal_analysis.rs`, `lidar_analysis.rs`
- `post_processor/src/report_generator.rs` (PDF/HTML/JSON/CSV/KML/Shapefile scaffold, TODOs)
- `shared/src/schemas.rs` (RecommendationRecord, ReportRecord, FieldRecord)

## Target Operating Model

- Evidence before advice: deterministic zonal statistics and anomaly flags run and are inspectable before any AI, health, or yield claim.
- Every finding ties to a georeferenced zone with retained raw evidence and a reason code.
- Recommendations carry priority, action category, status, and evidence linkage, persisted into `10` and surfaced through `08`.
- A real report generator emits PDF (farmer-friendly), CSV, and GeoJSON with field metadata, map views, findings, recommendations, and layer source details.
- Health and yield outputs are gated behind explicit uncertainty and never precede the deterministic products.
- Reproducible outputs: the same scene and parameters produce the same findings, with tests on the math and at least one failure path.
