use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RasterSeriesValue {
    pub raster_ref: String,
    pub crs: Option<String>,
    pub extent: Option<GeoExtent>,
    #[serde(default)]
    pub resolution: Option<RasterResolution>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoExtent {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RasterResolution {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SeriesValue {
    Scalar { value: f64 },
    Raster(RasterSeriesValue),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesPoint {
    pub entity_ref: String,
    pub metric: String,
    pub unit: String,
    pub t: String,
    pub value: SeriesValue,
    pub source_ref: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    Scalar,
    Raster,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricDefinition {
    pub metric: String,
    pub unit: String,
    pub kind: MetricKind,
    pub expected_cadence: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RasterAlignmentConfig {
    pub target_resolution_x: f64,
    pub target_resolution_y: f64,
    pub minimum_overlap_ratio: f64,
    pub resampling_method: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentRefusalReason {
    EntityMismatch,
    MetricMismatch,
    NotRasterPoint,
    MissingCrs,
    MissingExtent,
    MissingResolution,
    CrsMismatch,
    InsufficientOverlap,
    ResolutionMismatch,
    InvalidGuardConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RasterGridTransform {
    pub origin_x: f64,
    pub origin_y: f64,
    pub pixel_width: f64,
    pub pixel_height: f64,
    pub grid_columns: u32,
    pub grid_rows: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RasterAlignmentEvidence {
    pub alignment_ref: String,
    pub entity_ref: String,
    pub metric: String,
    pub earlier_t: String,
    pub later_t: String,
    pub earlier_raster_ref: String,
    pub later_raster_ref: String,
    pub earlier_source_ref: String,
    pub later_source_ref: String,
    pub aligned_earlier_ref: String,
    pub aligned_later_ref: String,
    pub target_crs: String,
    pub source_earlier_extent: GeoExtent,
    pub source_later_extent: GeoExtent,
    pub source_earlier_resolution: RasterResolution,
    pub source_later_resolution: RasterResolution,
    pub aligned_extent: GeoExtent,
    pub target_resolution_x: f64,
    pub target_resolution_y: f64,
    pub grid_columns: u32,
    pub grid_rows: u32,
    pub transform: RasterGridTransform,
    pub resampling_method: String,
    pub overlap_ratio_basis_points: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentGuardConfig {
    pub minimum_overlap_ratio: f64,
    pub resolution_tolerance: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentGuardProof {
    pub alignment_proof_ref: String,
    pub entity_ref: String,
    pub metric: String,
    pub earlier_t: String,
    pub later_t: String,
    pub earlier_raster_ref: String,
    pub later_raster_ref: String,
    pub target_crs: String,
    pub overlap_ratio_basis_points: u32,
    pub earlier_resolution: RasterResolution,
    pub later_resolution: RasterResolution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignmentGuardRefusal {
    pub reason_code: AlignmentRefusalReason,
    pub mismatch_detail: String,
    pub earlier_raster_ref: Option<String>,
    pub later_raster_ref: Option<String>,
    pub alignment_proof_ref: Option<String>,
    pub change_job_blocked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignedRasterGrid {
    pub raster_ref: String,
    pub alignment_ref: String,
    pub crs: String,
    pub extent: GeoExtent,
    pub resolution: RasterResolution,
    pub grid_columns: u32,
    pub grid_rows: u32,
    pub values: Vec<Option<f64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RasterChangeConfig {
    pub absolute_threshold: f64,
    pub method_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RasterChangeResult {
    pub delta_raster_ref: String,
    pub mask_raster_ref: String,
    pub alignment_ref: String,
    pub alignment_proof_ref: String,
    pub crs: String,
    pub extent: GeoExtent,
    pub resolution: RasterResolution,
    pub grid_columns: u32,
    pub grid_rows: u32,
    pub absolute_threshold: f64,
    pub method_version: String,
    pub delta_values: Vec<Option<f64>>,
    pub change_mask: Vec<bool>,
    pub changed_cell_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesProductIngest {
    pub entity_ref: String,
    pub metric: String,
    pub unit: String,
    pub source_ref: String,
    pub product_ref: String,
    pub product_date: String,
    pub finalized_at: String,
    pub value: SeriesValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeriesConflictResolution {
    KeepExisting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeriesIngestConflict {
    pub entity_ref: String,
    pub metric: String,
    pub t: String,
    pub existing_source_ref: String,
    pub incoming_source_ref: String,
    pub resolution: SeriesConflictResolution,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesIngestOutcome {
    pub point: SeriesPoint,
    pub conflict: Option<SeriesIngestConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZonalTrendTarget {
    pub entity_ref: String,
    pub metric: String,
    pub zone_ref: String,
    pub zone_crs: String,
    pub range: TimeRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ZonalTrendConfig {
    pub min_points: usize,
    pub flat_slope_epsilon: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Flat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZonalTrendResult {
    pub entity_ref: String,
    pub metric: String,
    pub unit: String,
    pub zone_ref: String,
    pub zone_crs: String,
    pub slope_per_day: f64,
    pub intercept: f64,
    pub fit_r_squared: f64,
    pub direction: TrendDirection,
    pub points_used: Vec<SeriesPoint>,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RollingBaselineConfig {
    pub window_points: usize,
    pub anomaly_band: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollingBaselineResult {
    pub entity_ref: String,
    pub metric: String,
    pub unit: String,
    pub zone_ref: String,
    pub zone_crs: String,
    pub baseline_mean: f64,
    pub latest_value: f64,
    pub delta_from_baseline: f64,
    pub anomaly: bool,
    pub baseline_window: Vec<SeriesPoint>,
    pub latest_point: SeriesPoint,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeasonalComparisonTarget {
    pub entity_ref: String,
    pub metric: String,
    pub zone_ref: String,
    pub zone_crs: String,
    pub current_t: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeasonalComparisonConfig {
    pub min_seasonal_points: usize,
    pub day_of_year_tolerance: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeasonalComparisonResult {
    pub entity_ref: String,
    pub metric: String,
    pub unit: String,
    pub zone_ref: String,
    pub zone_crs: String,
    pub current_point: SeriesPoint,
    pub seasonal_points: Vec<SeriesPoint>,
    pub seasonal_mean: f64,
    pub delta_from_seasonal_baseline: f64,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeEventDirection {
    Dropped,
    Increased,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeEventReasonCode {
    BaselineDrop,
    BaselineSpike,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChangeEventConfig {
    pub magnitude_threshold: f64,
    pub min_changed_cells: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeEvent {
    pub zone_ref: String,
    pub metric: String,
    pub magnitude: f64,
    pub direction: ChangeEventDirection,
    pub since_date: String,
    pub reason_code: ChangeEventReasonCode,
    pub changed_cell_count: u32,
    pub severity_score: f64,
    pub evidence_refs: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeEventDerivationInput {
    pub change: RasterChangeResult,
    pub trend: ZonalTrendResult,
    pub baseline: RollingBaselineResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompareViewFeed {
    pub schema_version: String,
    pub entity_ref: String,
    pub metric: String,
    pub alignment_ref: String,
    pub alignment_proof_ref: String,
    pub alignment_proof: AlignmentGuardProof,
    pub shared_view: CompareSharedView,
    pub earlier: CompareViewLayer,
    pub later: CompareViewLayer,
    pub change_mask: CompareViewChangeMask,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompareSharedView {
    pub crs: String,
    pub extent: GeoExtent,
    pub resolution: RasterResolution,
    pub grid_columns: u32,
    pub grid_rows: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompareViewLayer {
    pub raster_ref: String,
    pub source_ref: String,
    pub t: String,
    pub crs: String,
    pub extent: GeoExtent,
    pub resolution: RasterResolution,
    pub grid_columns: u32,
    pub grid_rows: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompareViewChangeMask {
    pub delta_raster_ref: String,
    pub mask_raster_ref: String,
    pub changed_cell_count: u32,
    pub absolute_threshold: f64,
    pub method_version: String,
    pub change_mask: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompareViewRefusal {
    pub schema_version: String,
    pub reason_code: AlignmentRefusalReason,
    pub mismatch_detail: String,
    pub earlier_raster_ref: Option<String>,
    pub later_raster_ref: Option<String>,
    pub alignment_proof_ref: Option<String>,
    pub no_misaligned_panes: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesCsvExport {
    pub content_type: String,
    pub schema_version: String,
    pub csv: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeMaskGeoTiffMetadata {
    pub mask_raster_ref: String,
    pub alignment_ref: String,
    pub alignment_proof_ref: String,
    pub crs: String,
    pub extent: GeoExtent,
    pub resolution: RasterResolution,
    pub grid_columns: u32,
    pub grid_rows: u32,
    pub changed_cell_count: u32,
    pub absolute_threshold: f64,
    pub method_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeMaskGeoTiffExport {
    pub content_type: String,
    pub schema_version: String,
    pub metadata: ChangeMaskGeoTiffMetadata,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeZonePolygon {
    pub crs: String,
    pub rings: Vec<Vec<[f64; 2]>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeZoneExportFeature {
    pub event: ChangeEvent,
    pub geometry: ChangeZonePolygon,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeZonesGeoJsonExport {
    pub content_type: String,
    pub schema_version: String,
    pub feature_collection: ChangeZoneFeatureCollection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeZoneFeatureCollection {
    pub geojson_type: String,
    pub crs: String,
    pub features: Vec<ChangeZoneGeoJsonFeature>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeZoneGeoJsonFeature {
    pub geojson_type: String,
    pub geometry: ChangeZoneGeoJsonGeometry,
    pub properties: ChangeZoneGeoJsonProperties,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeZoneGeoJsonGeometry {
    pub geojson_type: String,
    pub coordinates: Vec<Vec<[f64; 2]>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeZoneGeoJsonProperties {
    pub zone_ref: String,
    pub metric: String,
    pub magnitude: f64,
    pub direction: ChangeEventDirection,
    pub since_date: String,
    pub reason_code: ChangeEventReasonCode,
    pub changed_cell_count: u32,
    pub severity_score: f64,
    pub evidence_refs: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimeSeriesStore {
    points: BTreeMap<SeriesKey, SeriesPoint>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetricRegistry {
    definitions: BTreeMap<String, MetricDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimeSeriesError {
    #[error("entity_ref cannot be empty")]
    EmptyEntityRef,
    #[error("metric cannot be empty")]
    EmptyMetric,
    #[error("unit cannot be empty")]
    EmptyUnit,
    #[error("expected cadence cannot be empty for {metric}")]
    EmptyExpectedCadence { metric: String },
    #[error("timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("source_ref cannot be empty")]
    EmptySourceRef,
    #[error("product_ref cannot be empty")]
    EmptyProductRef,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("metric already registered: {metric}")]
    DuplicateMetricDefinition { metric: String },
    #[error("unknown metric: {metric}")]
    UnknownMetric { metric: String },
    #[error("metric {metric} unit mismatch: expected {expected_unit}, got {actual_unit}")]
    MetricUnitMismatch {
        metric: String,
        expected_unit: String,
        actual_unit: String,
    },
    #[error("metric {metric} kind mismatch")]
    MetricKindMismatch {
        metric: String,
        expected_kind: MetricKind,
        actual_kind: MetricKind,
    },
    #[error("scalar value must be finite")]
    InvalidScalarValue,
    #[error("raster_ref cannot be empty")]
    EmptyRasterRef,
    #[error("raster extent must be finite and ordered")]
    InvalidExtent,
    #[error("raster resolution must be finite and positive")]
    InvalidRasterResolution,
    #[error("duplicate time-series point for {entity_ref}/{metric} at {t}")]
    DuplicateSeriesPoint {
        entity_ref: String,
        metric: String,
        t: String,
    },
    #[error("alignment_ref cannot be empty")]
    EmptyAlignmentRef,
    #[error("resampling_method cannot be empty")]
    EmptyResamplingMethod,
    #[error(
        "raster alignment config must be finite with positive resolution and overlap in [0, 1]"
    )]
    InvalidAlignmentConfig,
    #[error("raster alignment requires raster series points")]
    AlignmentRequiresRasterPoint,
    #[error("raster alignment requires CRS on both raster points")]
    MissingRasterCrs,
    #[error("raster alignment requires extent on both raster points")]
    MissingRasterExtent,
    #[error("raster alignment requires resolution on both raster points")]
    MissingRasterResolution,
    #[error("raster alignment requires one entity and metric")]
    AlignmentSeriesMismatch,
    #[error("raster CRS mismatch: {earlier_crs} vs {later_crs}")]
    AlignmentCrsMismatch {
        earlier_crs: String,
        later_crs: String,
    },
    #[error("insufficient raster overlap: observed {observed_overlap_basis_points}bp below required {minimum_overlap_basis_points}bp")]
    InsufficientOverlap {
        reason_code: AlignmentRefusalReason,
        observed_overlap_basis_points: u32,
        minimum_overlap_basis_points: u32,
    },
    #[error("aligned grid must contain at least one cell")]
    InvalidAlignedGrid,
    #[error("delta_raster_ref cannot be empty")]
    EmptyDeltaRasterRef,
    #[error("mask_raster_ref cannot be empty")]
    EmptyMaskRasterRef,
    #[error("change method_version cannot be empty")]
    EmptyChangeMethodVersion,
    #[error("raster change config must be finite with a non-negative threshold")]
    InvalidChangeConfig,
    #[error("raster change inputs must match alignment evidence and proof")]
    ChangeAlignmentMismatch,
    #[error("aligned raster grid cell count does not match dimensions")]
    InvalidRasterCellCount,
    #[error("aligned raster grid values must be finite when present")]
    InvalidRasterCellValue,
    #[error("zone_ref cannot be empty")]
    EmptyZoneRef,
    #[error("zone_crs cannot be empty")]
    EmptyZoneCrs,
    #[error("trend config must require at least two points with finite non-negative flat epsilon")]
    InvalidTrendConfig,
    #[error("trend requires scalar points for {entity_ref}/{metric}")]
    TrendRequiresScalarPoint { entity_ref: String, metric: String },
    #[error("insufficient trend history for {entity_ref}/{metric}: observed {observed_points}, required {required_points}")]
    InsufficientTrendHistory {
        entity_ref: String,
        metric: String,
        observed_points: usize,
        required_points: usize,
    },
    #[error("invalid trend timestamp for {timestamp}")]
    InvalidTrendTimestamp { timestamp: String },
    #[error("baseline config must require at least one window point with finite non-negative anomaly band")]
    InvalidBaselineConfig,
    #[error("insufficient baseline history for {entity_ref}/{metric}: observed {observed_points}, required {required_points}")]
    InsufficientBaselineHistory {
        entity_ref: String,
        metric: String,
        observed_points: usize,
        required_points: usize,
    },
    #[error("no seasonal baseline for {entity_ref}/{metric} at {current_t}: observed {observed_points}, required {required_points}")]
    NoSeasonalBaseline {
        entity_ref: String,
        metric: String,
        current_t: String,
        observed_points: usize,
        required_points: usize,
    },
    #[error("change event config must have finite non-negative threshold")]
    InvalidChangeEventConfig,
    #[error("export CRS cannot be empty")]
    EmptyExportCrs,
    #[error("change zone export CRS mismatch: expected {expected_crs}, got {actual_crs}")]
    ChangeZoneCrsMismatch {
        expected_crs: String,
        actual_crs: String,
    },
    #[error("change zone geometry must contain finite closed polygon rings")]
    InvalidChangeZoneGeometry,
}

impl TimeSeriesStore {
    pub fn append(&mut self, point: SeriesPoint) -> Result<(), TimeSeriesError> {
        let point = normalize_point(point)?;
        let key = SeriesKey::from_point(&point);
        if self.points.contains_key(&key) {
            return Err(TimeSeriesError::DuplicateSeriesPoint {
                entity_ref: key.entity_ref,
                metric: key.metric,
                t: key.t,
            });
        }
        self.points.insert(key, point);
        Ok(())
    }

    pub fn query(&self, entity_ref: &str, metric: &str, range: TimeRange) -> Vec<SeriesPoint> {
        self.points
            .iter()
            .filter(|(key, _)| key.entity_ref == entity_ref && key.metric == metric)
            .filter(|(key, _)| range.contains(&key.t))
            .map(|(_, point)| point.clone())
            .collect()
    }

    fn get(&self, entity_ref: &str, metric: &str, t: &str) -> Option<&SeriesPoint> {
        self.points.get(&SeriesKey {
            entity_ref: entity_ref.to_string(),
            metric: metric.to_string(),
            t: t.to_string(),
        })
    }

    fn list_metrics(&self, entity_ref: &str) -> Vec<String> {
        self.points
            .keys()
            .filter(|key| key.entity_ref == entity_ref)
            .map(|key| key.metric.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}

impl MetricRegistry {
    pub fn register(
        &mut self,
        definition: MetricDefinition,
    ) -> Result<MetricDefinition, TimeSeriesError> {
        let definition = normalize_metric_definition(definition)?;
        if self.definitions.contains_key(&definition.metric) {
            return Err(TimeSeriesError::DuplicateMetricDefinition {
                metric: definition.metric,
            });
        }
        self.definitions
            .insert(definition.metric.clone(), definition.clone());
        Ok(definition)
    }

    pub fn get(&self, metric: &str) -> Option<&MetricDefinition> {
        self.definitions.get(metric)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: Option<String>,
    pub end: Option<String>,
}

impl TimeRange {
    fn contains(&self, t: &str) -> bool {
        self.start.as_deref().map_or(true, |start| t >= start)
            && self.end.as_deref().map_or(true, |end| t <= end)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimeSeriesEngine {
    store: TimeSeriesStore,
    metric_registry: MetricRegistry,
    ingest_conflicts: Vec<SeriesIngestConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesQuery {
    pub entity_ref: String,
    pub metric: String,
    pub range: TimeRange,
    pub limit: Option<usize>,
    pub cursor: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesQueryPage {
    pub points: Vec<SeriesPoint>,
    pub next_cursor: Option<usize>,
    pub no_series: bool,
}

impl TimeSeriesEngine {
    pub fn register_metric(
        &mut self,
        definition: MetricDefinition,
    ) -> Result<MetricDefinition, TimeSeriesError> {
        self.metric_registry.register(definition)
    }

    pub fn append(&mut self, point: SeriesPoint) -> Result<(), TimeSeriesError> {
        let point = normalize_point(point)?;
        self.validate_point_metric(&point)?;
        self.store.append(point)
    }

    pub fn query(&self, query: SeriesQuery) -> SeriesQueryPage {
        let points = self
            .store
            .query(&query.entity_ref, &query.metric, query.range);
        let no_series = points.is_empty();
        let start = query.cursor.unwrap_or(0).min(points.len());
        let limit = query.limit.unwrap_or(points.len()).max(1);
        let end = (start + limit).min(points.len());
        let next_cursor = (end < points.len()).then_some(end);

        SeriesQueryPage {
            points: points[start..end].to_vec(),
            next_cursor,
            no_series,
        }
    }

    pub fn list_metrics(&self, entity_ref: &str) -> Vec<String> {
        self.store.list_metrics(entity_ref)
    }

    pub fn ingest_product(
        &mut self,
        ingest: SeriesProductIngest,
    ) -> Result<SeriesIngestOutcome, TimeSeriesError> {
        let ingest = normalize_product_ingest(ingest)?;
        let point = SeriesPoint {
            entity_ref: ingest.entity_ref,
            metric: ingest.metric,
            unit: ingest.unit,
            t: ingest.product_date,
            value: ingest.value,
            source_ref: ingest.source_ref,
            created_at: ingest.finalized_at,
        };
        let point = normalize_point(point)?;
        self.validate_point_metric(&point)?;

        if let Some(existing) = self.store.get(&point.entity_ref, &point.metric, &point.t) {
            let conflict = SeriesIngestConflict {
                entity_ref: point.entity_ref.clone(),
                metric: point.metric.clone(),
                t: point.t.clone(),
                existing_source_ref: existing.source_ref.clone(),
                incoming_source_ref: point.source_ref.clone(),
                resolution: SeriesConflictResolution::KeepExisting,
            };
            self.ingest_conflicts.push(conflict.clone());
            return Ok(SeriesIngestOutcome {
                point: existing.clone(),
                conflict: Some(conflict),
            });
        }

        self.store.append(point.clone())?;
        Ok(SeriesIngestOutcome {
            point,
            conflict: None,
        })
    }

    pub fn ingest_conflicts(&self) -> &[SeriesIngestConflict] {
        &self.ingest_conflicts
    }

    pub fn compute_zonal_trend(
        &self,
        target: ZonalTrendTarget,
        config: ZonalTrendConfig,
    ) -> Result<ZonalTrendResult, TimeSeriesError> {
        let target = normalize_zonal_trend_target(target)?;
        let config = normalize_zonal_trend_config(config)?;
        let definition = self.metric_registry.get(&target.metric).ok_or_else(|| {
            TimeSeriesError::UnknownMetric {
                metric: target.metric.clone(),
            }
        })?;
        if definition.kind != MetricKind::Scalar {
            return Err(TimeSeriesError::MetricKindMismatch {
                metric: target.metric,
                expected_kind: MetricKind::Scalar,
                actual_kind: definition.kind,
            });
        }

        let points = self
            .store
            .query(&target.entity_ref, &target.metric, target.range.clone());
        if points.len() < config.min_points {
            return Err(TimeSeriesError::InsufficientTrendHistory {
                entity_ref: target.entity_ref,
                metric: target.metric,
                observed_points: points.len(),
                required_points: config.min_points,
            });
        }

        let mut samples = Vec::with_capacity(points.len());
        for point in &points {
            let SeriesValue::Scalar { value } = point.value else {
                return Err(TimeSeriesError::TrendRequiresScalarPoint {
                    entity_ref: target.entity_ref,
                    metric: target.metric,
                });
            };
            samples.push((timestamp_day_index(&point.t)?, value));
        }

        let first_day = samples[0].0;
        let normalized_samples = samples
            .iter()
            .map(|(day, value)| ((*day - first_day) as f64, *value))
            .collect::<Vec<_>>();
        let (slope_per_day, intercept, fit_r_squared) = least_squares_trend(&normalized_samples)?;
        let direction = if slope_per_day.abs() <= config.flat_slope_epsilon {
            TrendDirection::Flat
        } else if slope_per_day > 0.0 {
            TrendDirection::Increasing
        } else {
            TrendDirection::Decreasing
        };
        let evidence_refs = points
            .iter()
            .map(|point| point.source_ref.clone())
            .collect::<Vec<_>>();

        Ok(ZonalTrendResult {
            entity_ref: target.entity_ref,
            metric: target.metric,
            unit: definition.unit.clone(),
            zone_ref: target.zone_ref,
            zone_crs: target.zone_crs,
            slope_per_day,
            intercept,
            fit_r_squared,
            direction,
            points_used: points,
            evidence_refs,
        })
    }

    pub fn compute_rolling_baseline(
        &self,
        target: ZonalTrendTarget,
        config: RollingBaselineConfig,
    ) -> Result<RollingBaselineResult, TimeSeriesError> {
        let target = normalize_zonal_trend_target(target)?;
        let config = normalize_rolling_baseline_config(config)?;
        let unit = self.scalar_metric_unit(&target.metric)?;
        let points = self
            .store
            .query(&target.entity_ref, &target.metric, target.range.clone());
        let required_points = config.window_points + 1;
        if points.len() < required_points {
            return Err(TimeSeriesError::InsufficientBaselineHistory {
                entity_ref: target.entity_ref,
                metric: target.metric,
                observed_points: points.len(),
                required_points,
            });
        }

        let latest_point = points.last().cloned().expect("length checked");
        let latest_value = scalar_value_from_point(&latest_point)?;
        let baseline_start = points.len() - 1 - config.window_points;
        let baseline_window = points[baseline_start..points.len() - 1].to_vec();
        let baseline_values = baseline_window
            .iter()
            .map(scalar_value_from_point)
            .collect::<Result<Vec<_>, _>>()?;
        let baseline_mean = mean(&baseline_values);
        let delta_from_baseline = latest_value - baseline_mean;
        let anomaly = delta_from_baseline.abs() >= config.anomaly_band;
        let mut evidence_refs = baseline_window
            .iter()
            .map(|point| point.source_ref.clone())
            .collect::<Vec<_>>();
        evidence_refs.push(latest_point.source_ref.clone());

        Ok(RollingBaselineResult {
            entity_ref: target.entity_ref,
            metric: target.metric,
            unit,
            zone_ref: target.zone_ref,
            zone_crs: target.zone_crs,
            baseline_mean,
            latest_value,
            delta_from_baseline,
            anomaly,
            baseline_window,
            latest_point,
            evidence_refs,
        })
    }

    pub fn compute_seasonal_comparison(
        &self,
        target: SeasonalComparisonTarget,
        config: SeasonalComparisonConfig,
    ) -> Result<SeasonalComparisonResult, TimeSeriesError> {
        let target = normalize_seasonal_comparison_target(target)?;
        let config = normalize_seasonal_comparison_config(config)?;
        let unit = self.scalar_metric_unit(&target.metric)?;
        let current_point = self
            .store
            .get(&target.entity_ref, &target.metric, &target.current_t)
            .cloned()
            .ok_or_else(|| TimeSeriesError::NoSeasonalBaseline {
                entity_ref: target.entity_ref.clone(),
                metric: target.metric.clone(),
                current_t: target.current_t.clone(),
                observed_points: 0,
                required_points: config.min_seasonal_points,
            })?;
        let current_value = scalar_value_from_point(&current_point)?;
        let (current_year, current_day_of_year) = timestamp_year_and_day(&target.current_t)?;
        let mut seasonal_points = Vec::new();
        for point in self
            .store
            .query(&target.entity_ref, &target.metric, TimeRange::default())
        {
            if point.t == target.current_t {
                continue;
            }
            let (year, day_of_year) = timestamp_year_and_day(&point.t)?;
            let same_season = year < current_year
                && current_day_of_year.abs_diff(day_of_year) <= config.day_of_year_tolerance;
            if same_season {
                seasonal_points.push(point);
            }
        }
        if seasonal_points.len() < config.min_seasonal_points {
            return Err(TimeSeriesError::NoSeasonalBaseline {
                entity_ref: target.entity_ref,
                metric: target.metric,
                current_t: target.current_t,
                observed_points: seasonal_points.len(),
                required_points: config.min_seasonal_points,
            });
        }
        let seasonal_values = seasonal_points
            .iter()
            .map(scalar_value_from_point)
            .collect::<Result<Vec<_>, _>>()?;
        let seasonal_mean = mean(&seasonal_values);
        let delta_from_seasonal_baseline = current_value - seasonal_mean;
        let mut evidence_refs = seasonal_points
            .iter()
            .map(|point| point.source_ref.clone())
            .collect::<Vec<_>>();
        evidence_refs.push(current_point.source_ref.clone());

        Ok(SeasonalComparisonResult {
            entity_ref: target.entity_ref,
            metric: target.metric,
            unit,
            zone_ref: target.zone_ref,
            zone_crs: target.zone_crs,
            current_point,
            seasonal_points,
            seasonal_mean,
            delta_from_seasonal_baseline,
            evidence_refs,
        })
    }

    fn scalar_metric_unit(&self, metric: &str) -> Result<String, TimeSeriesError> {
        let definition =
            self.metric_registry
                .get(metric)
                .ok_or_else(|| TimeSeriesError::UnknownMetric {
                    metric: metric.to_string(),
                })?;
        if definition.kind != MetricKind::Scalar {
            return Err(TimeSeriesError::MetricKindMismatch {
                metric: metric.to_string(),
                expected_kind: MetricKind::Scalar,
                actual_kind: definition.kind,
            });
        }
        Ok(definition.unit.clone())
    }

    fn validate_point_metric(&self, point: &SeriesPoint) -> Result<(), TimeSeriesError> {
        let definition = self.metric_registry.get(&point.metric).ok_or_else(|| {
            TimeSeriesError::UnknownMetric {
                metric: point.metric.clone(),
            }
        })?;
        if point.unit != definition.unit {
            return Err(TimeSeriesError::MetricUnitMismatch {
                metric: point.metric.clone(),
                expected_unit: definition.unit.clone(),
                actual_unit: point.unit.clone(),
            });
        }
        let actual_kind = metric_kind_for_value(&point.value);
        if actual_kind != definition.kind {
            return Err(TimeSeriesError::MetricKindMismatch {
                metric: point.metric.clone(),
                expected_kind: definition.kind,
                actual_kind,
            });
        }
        Ok(())
    }
}

pub fn derive_ranked_change_events(
    inputs: Vec<ChangeEventDerivationInput>,
    config: ChangeEventConfig,
) -> Result<Vec<ChangeEvent>, TimeSeriesError> {
    let config = normalize_change_event_config(config)?;
    let mut events = Vec::new();
    for input in inputs {
        if input.change.changed_cell_count < config.min_changed_cells {
            continue;
        }
        let magnitude = input.baseline.delta_from_baseline.abs();
        if magnitude < config.magnitude_threshold {
            continue;
        }

        let direction = if input.baseline.delta_from_baseline < 0.0 {
            ChangeEventDirection::Dropped
        } else {
            ChangeEventDirection::Increased
        };
        let reason_code = match direction {
            ChangeEventDirection::Dropped => ChangeEventReasonCode::BaselineDrop,
            ChangeEventDirection::Increased => ChangeEventReasonCode::BaselineSpike,
        };
        let since_date = input
            .trend
            .points_used
            .first()
            .map(|point| point.t.clone())
            .unwrap_or_else(|| input.baseline.latest_point.t.clone());
        let severity_score = magnitude * f64::from(input.change.changed_cell_count);
        let mut evidence_refs = Vec::new();
        push_unique(&mut evidence_refs, input.change.alignment_ref.clone());
        push_unique(&mut evidence_refs, input.change.alignment_proof_ref.clone());
        push_unique(&mut evidence_refs, input.change.delta_raster_ref.clone());
        push_unique(&mut evidence_refs, input.change.mask_raster_ref.clone());
        push_unique(&mut evidence_refs, input.baseline.zone_ref.clone());
        for reference in input.trend.evidence_refs {
            push_unique(&mut evidence_refs, reference);
        }
        for reference in input.baseline.evidence_refs {
            push_unique(&mut evidence_refs, reference);
        }

        let verb = match direction {
            ChangeEventDirection::Dropped => "dropped",
            ChangeEventDirection::Increased => "increased",
        };
        let summary = format!(
            "{} {verb} {:.2} in {} since {}",
            input.baseline.metric, magnitude, input.baseline.zone_ref, since_date
        );
        events.push(ChangeEvent {
            zone_ref: input.baseline.zone_ref,
            metric: input.baseline.metric,
            magnitude,
            direction,
            since_date,
            reason_code,
            changed_cell_count: input.change.changed_cell_count,
            severity_score,
            evidence_refs,
            summary,
        });
    }

    events.sort_by(|left, right| {
        right
            .severity_score
            .total_cmp(&left.severity_score)
            .then_with(|| right.magnitude.total_cmp(&left.magnitude))
            .then_with(|| left.zone_ref.cmp(&right.zone_ref))
    });
    Ok(events)
}

pub fn export_series_csv(points: &[SeriesPoint]) -> Result<SeriesCsvExport, TimeSeriesError> {
    let mut csv = "entity_ref,metric,t,unit,value,source_ref,created_at\n".to_string();
    for point in points {
        let point = normalize_point(point.clone())?;
        let value = match &point.value {
            SeriesValue::Scalar { value } => value.to_string(),
            SeriesValue::Raster(raster) => raster.raster_ref.clone(),
        };
        csv.push_str(&csv_row([
            point.entity_ref.as_str(),
            point.metric.as_str(),
            point.t.as_str(),
            point.unit.as_str(),
            value.as_str(),
            point.source_ref.as_str(),
            point.created_at.as_str(),
        ]));
    }

    Ok(SeriesCsvExport {
        content_type: "text/csv".to_string(),
        schema_version: "timeseries.series_csv.v1".to_string(),
        csv,
    })
}

pub fn export_change_mask_geotiff(
    change: &RasterChangeResult,
) -> Result<ChangeMaskGeoTiffExport, TimeSeriesError> {
    validate_raster_change_result(change)?;
    let metadata = ChangeMaskGeoTiffMetadata {
        mask_raster_ref: change.mask_raster_ref.clone(),
        alignment_ref: change.alignment_ref.clone(),
        alignment_proof_ref: change.alignment_proof_ref.clone(),
        crs: change.crs.clone(),
        extent: change.extent,
        resolution: change.resolution,
        grid_columns: change.grid_columns,
        grid_rows: change.grid_rows,
        changed_cell_count: change.changed_cell_count,
        absolute_threshold: change.absolute_threshold,
        method_version: change.method_version.clone(),
    };
    let metadata_line = format!(
        "mask_raster_ref={};alignment_ref={};alignment_proof_ref={};crs={};extent={},{},{},{};resolution={},{};grid={}x{};changed_cell_count={};threshold={};method={}\n",
        metadata.mask_raster_ref,
        metadata.alignment_ref,
        metadata.alignment_proof_ref,
        metadata.crs,
        metadata.extent.min_x,
        metadata.extent.min_y,
        metadata.extent.max_x,
        metadata.extent.max_y,
        metadata.resolution.x,
        metadata.resolution.y,
        metadata.grid_columns,
        metadata.grid_rows,
        metadata.changed_cell_count,
        metadata.absolute_threshold,
        metadata.method_version
    );
    let mut bytes = b"AGBOT_TIMESERIES_GEOTIFF_METADATA\n".to_vec();
    bytes.extend(metadata_line.as_bytes());

    Ok(ChangeMaskGeoTiffExport {
        content_type: "image/tiff".to_string(),
        schema_version: "timeseries.change_mask_geotiff.v1".to_string(),
        metadata,
        bytes,
    })
}

pub fn export_change_zones_geojson(
    features: Vec<ChangeZoneExportFeature>,
    crs: String,
) -> Result<ChangeZonesGeoJsonExport, TimeSeriesError> {
    let crs = normalize_required_text(crs, TimeSeriesError::EmptyExportCrs)?;
    let mut geojson_features = Vec::with_capacity(features.len());
    for feature in features {
        let geometry = normalize_change_zone_polygon(feature.geometry, &crs)?;
        let event = feature.event;
        geojson_features.push(ChangeZoneGeoJsonFeature {
            geojson_type: "Feature".to_string(),
            geometry: ChangeZoneGeoJsonGeometry {
                geojson_type: "Polygon".to_string(),
                coordinates: geometry.rings,
            },
            properties: ChangeZoneGeoJsonProperties {
                zone_ref: event.zone_ref,
                metric: event.metric,
                magnitude: event.magnitude,
                direction: event.direction,
                since_date: event.since_date,
                reason_code: event.reason_code,
                changed_cell_count: event.changed_cell_count,
                severity_score: event.severity_score,
                evidence_refs: event.evidence_refs,
                summary: event.summary,
            },
        });
    }

    Ok(ChangeZonesGeoJsonExport {
        content_type: "application/geo+json".to_string(),
        schema_version: "timeseries.change_zones_geojson.v1".to_string(),
        feature_collection: ChangeZoneFeatureCollection {
            geojson_type: "FeatureCollection".to_string(),
            crs,
            features: geojson_features,
        },
    })
}

pub fn build_compare_view_feed(
    proof: &AlignmentGuardProof,
    evidence: &RasterAlignmentEvidence,
    change: &RasterChangeResult,
) -> Result<CompareViewFeed, CompareViewRefusal> {
    validate_compare_view_inputs(proof, evidence, change)?;
    let shared_view = CompareSharedView {
        crs: evidence.target_crs.clone(),
        extent: evidence.aligned_extent,
        resolution: RasterResolution {
            x: evidence.target_resolution_x,
            y: evidence.target_resolution_y,
        },
        grid_columns: evidence.grid_columns,
        grid_rows: evidence.grid_rows,
    };

    Ok(CompareViewFeed {
        schema_version: "timeseries.compare_view_feed.v1".to_string(),
        entity_ref: evidence.entity_ref.clone(),
        metric: evidence.metric.clone(),
        alignment_ref: evidence.alignment_ref.clone(),
        alignment_proof_ref: proof.alignment_proof_ref.clone(),
        alignment_proof: proof.clone(),
        earlier: CompareViewLayer {
            raster_ref: evidence.aligned_earlier_ref.clone(),
            source_ref: evidence.earlier_source_ref.clone(),
            t: evidence.earlier_t.clone(),
            crs: shared_view.crs.clone(),
            extent: shared_view.extent,
            resolution: shared_view.resolution,
            grid_columns: shared_view.grid_columns,
            grid_rows: shared_view.grid_rows,
        },
        later: CompareViewLayer {
            raster_ref: evidence.aligned_later_ref.clone(),
            source_ref: evidence.later_source_ref.clone(),
            t: evidence.later_t.clone(),
            crs: shared_view.crs.clone(),
            extent: shared_view.extent,
            resolution: shared_view.resolution,
            grid_columns: shared_view.grid_columns,
            grid_rows: shared_view.grid_rows,
        },
        change_mask: CompareViewChangeMask {
            delta_raster_ref: change.delta_raster_ref.clone(),
            mask_raster_ref: change.mask_raster_ref.clone(),
            changed_cell_count: change.changed_cell_count,
            absolute_threshold: change.absolute_threshold,
            method_version: change.method_version.clone(),
            change_mask: change.change_mask.clone(),
        },
        shared_view,
    })
}

pub fn compare_view_refusal_from_guard(refusal: AlignmentGuardRefusal) -> CompareViewRefusal {
    CompareViewRefusal {
        schema_version: "timeseries.compare_view_refusal.v1".to_string(),
        reason_code: refusal.reason_code,
        mismatch_detail: refusal.mismatch_detail,
        earlier_raster_ref: refusal.earlier_raster_ref,
        later_raster_ref: refusal.later_raster_ref,
        alignment_proof_ref: refusal.alignment_proof_ref,
        no_misaligned_panes: true,
    }
}

pub fn align_raster_pair(
    earlier: &SeriesPoint,
    later: &SeriesPoint,
    config: RasterAlignmentConfig,
    generated_alignment_ref: String,
) -> Result<RasterAlignmentEvidence, TimeSeriesError> {
    let alignment_ref =
        normalize_required_text(generated_alignment_ref, TimeSeriesError::EmptyAlignmentRef)?;
    let config = normalize_alignment_config(config)?;
    let earlier = normalize_point(earlier.clone())?;
    let later = normalize_point(later.clone())?;
    if earlier.entity_ref != later.entity_ref || earlier.metric != later.metric {
        return Err(TimeSeriesError::AlignmentSeriesMismatch);
    }

    let earlier_raster = raster_alignment_input(&earlier)?;
    let later_raster = raster_alignment_input(&later)?;
    if earlier_raster.crs != later_raster.crs {
        return Err(TimeSeriesError::AlignmentCrsMismatch {
            earlier_crs: earlier_raster.crs,
            later_crs: later_raster.crs,
        });
    }

    let overlap = extent_intersection(earlier_raster.extent, later_raster.extent);
    let overlap_area = overlap.map_or(0.0, extent_area);
    let denominator = extent_area(earlier_raster.extent).min(extent_area(later_raster.extent));
    let observed_overlap_ratio = if denominator > 0.0 {
        overlap_area / denominator
    } else {
        0.0
    };
    let observed_overlap_basis_points = ratio_to_basis_points(observed_overlap_ratio);
    let minimum_overlap_basis_points = ratio_to_basis_points(config.minimum_overlap_ratio);
    if observed_overlap_basis_points < minimum_overlap_basis_points {
        return Err(TimeSeriesError::InsufficientOverlap {
            reason_code: AlignmentRefusalReason::InsufficientOverlap,
            observed_overlap_basis_points,
            minimum_overlap_basis_points,
        });
    }

    let overlap = overlap.ok_or(TimeSeriesError::InvalidAlignedGrid)?;
    let grid_columns = grid_cell_count(overlap.max_x - overlap.min_x, config.target_resolution_x)?;
    let grid_rows = grid_cell_count(overlap.max_y - overlap.min_y, config.target_resolution_y)?;
    let aligned_extent = GeoExtent {
        min_x: overlap.min_x,
        min_y: overlap.min_y,
        max_x: overlap.min_x + f64::from(grid_columns) * config.target_resolution_x,
        max_y: overlap.min_y + f64::from(grid_rows) * config.target_resolution_y,
    };
    let transform = RasterGridTransform {
        origin_x: aligned_extent.min_x,
        origin_y: aligned_extent.max_y,
        pixel_width: config.target_resolution_x,
        pixel_height: -config.target_resolution_y,
        grid_columns,
        grid_rows,
    };

    Ok(RasterAlignmentEvidence {
        aligned_earlier_ref: format!("{alignment_ref}:earlier"),
        aligned_later_ref: format!("{alignment_ref}:later"),
        alignment_ref,
        entity_ref: earlier.entity_ref,
        metric: earlier.metric,
        earlier_t: earlier.t,
        later_t: later.t,
        earlier_raster_ref: earlier_raster.raster_ref,
        later_raster_ref: later_raster.raster_ref,
        earlier_source_ref: earlier.source_ref,
        later_source_ref: later.source_ref,
        target_crs: earlier_raster.crs,
        source_earlier_extent: earlier_raster.extent,
        source_later_extent: later_raster.extent,
        source_earlier_resolution: earlier_raster.resolution,
        source_later_resolution: later_raster.resolution,
        aligned_extent,
        target_resolution_x: config.target_resolution_x,
        target_resolution_y: config.target_resolution_y,
        grid_columns,
        grid_rows,
        transform,
        resampling_method: config.resampling_method,
        overlap_ratio_basis_points: observed_overlap_basis_points,
    })
}

pub fn guard_coregisterable_pair(
    earlier: &SeriesPoint,
    later: &SeriesPoint,
    config: AlignmentGuardConfig,
    generated_alignment_proof_ref: String,
) -> Result<AlignmentGuardProof, AlignmentGuardRefusal> {
    let alignment_proof_ref = normalize_required_text(
        generated_alignment_proof_ref,
        TimeSeriesError::EmptyAlignmentRef,
    )
    .map_err(|error| {
        guard_refusal(
            AlignmentRefusalReason::InvalidGuardConfig,
            error.to_string(),
            raster_ref_from_point(earlier),
            raster_ref_from_point(later),
        )
    })?;
    let config = normalize_guard_config(config).map_err(|error| {
        guard_refusal(
            AlignmentRefusalReason::InvalidGuardConfig,
            error.to_string(),
            raster_ref_from_point(earlier),
            raster_ref_from_point(later),
        )
    })?;
    let earlier = normalize_point(earlier.clone()).map_err(|error| {
        guard_refusal(
            guard_reason_from_error(&error),
            error.to_string(),
            raster_ref_from_point(earlier),
            raster_ref_from_point(later),
        )
    })?;
    let later = normalize_point(later.clone()).map_err(|error| {
        guard_refusal(
            guard_reason_from_error(&error),
            error.to_string(),
            raster_ref_from_point(&earlier),
            raster_ref_from_point(later),
        )
    })?;
    let earlier_ref = raster_ref_from_point(&earlier);
    let later_ref = raster_ref_from_point(&later);

    if earlier.entity_ref != later.entity_ref {
        return Err(guard_refusal(
            AlignmentRefusalReason::EntityMismatch,
            format!(
                "entity mismatch: {} vs {}",
                earlier.entity_ref, later.entity_ref
            ),
            earlier_ref,
            later_ref,
        ));
    }
    if earlier.metric != later.metric {
        return Err(guard_refusal(
            AlignmentRefusalReason::MetricMismatch,
            format!("metric mismatch: {} vs {}", earlier.metric, later.metric),
            earlier_ref,
            later_ref,
        ));
    }

    let earlier_raster = raster_alignment_input(&earlier).map_err(|error| {
        guard_refusal(
            guard_reason_from_error(&error),
            error.to_string(),
            earlier_ref.clone(),
            later_ref.clone(),
        )
    })?;
    let later_raster = raster_alignment_input(&later).map_err(|error| {
        guard_refusal(
            guard_reason_from_error(&error),
            error.to_string(),
            earlier_ref.clone(),
            later_ref.clone(),
        )
    })?;
    if earlier_raster.crs != later_raster.crs {
        return Err(guard_refusal(
            AlignmentRefusalReason::CrsMismatch,
            format!(
                "CRS mismatch: {} vs {}",
                earlier_raster.crs, later_raster.crs
            ),
            Some(earlier_raster.raster_ref),
            Some(later_raster.raster_ref),
        ));
    }

    let observed_overlap_basis_points =
        overlap_ratio_basis_points(earlier_raster.extent, later_raster.extent);
    let minimum_overlap_basis_points = ratio_to_basis_points(config.minimum_overlap_ratio);
    if observed_overlap_basis_points < minimum_overlap_basis_points {
        return Err(guard_refusal(
            AlignmentRefusalReason::InsufficientOverlap,
            format!(
                "observed {observed_overlap_basis_points}bp below required {minimum_overlap_basis_points}bp"
            ),
            Some(earlier_raster.raster_ref),
            Some(later_raster.raster_ref),
        ));
    }

    if !resolution_compatible(
        earlier_raster.resolution,
        later_raster.resolution,
        config.resolution_tolerance,
    ) {
        return Err(guard_refusal(
            AlignmentRefusalReason::ResolutionMismatch,
            format!(
                "resolution mismatch: {}x{} vs {}x{} with tolerance {}",
                earlier_raster.resolution.x,
                earlier_raster.resolution.y,
                later_raster.resolution.x,
                later_raster.resolution.y,
                config.resolution_tolerance
            ),
            Some(earlier_raster.raster_ref),
            Some(later_raster.raster_ref),
        ));
    }

    Ok(AlignmentGuardProof {
        alignment_proof_ref,
        entity_ref: earlier.entity_ref,
        metric: earlier.metric,
        earlier_t: earlier.t,
        later_t: later.t,
        earlier_raster_ref: earlier_raster.raster_ref,
        later_raster_ref: later_raster.raster_ref,
        target_crs: earlier_raster.crs,
        overlap_ratio_basis_points: observed_overlap_basis_points,
        earlier_resolution: earlier_raster.resolution,
        later_resolution: later_raster.resolution,
    })
}

pub fn compute_aligned_raster_change(
    guard_proof: &AlignmentGuardProof,
    evidence: &RasterAlignmentEvidence,
    earlier: &AlignedRasterGrid,
    later: &AlignedRasterGrid,
    config: RasterChangeConfig,
    generated_delta_raster_ref: String,
    generated_mask_raster_ref: String,
) -> Result<RasterChangeResult, TimeSeriesError> {
    let delta_raster_ref = normalize_required_text(
        generated_delta_raster_ref,
        TimeSeriesError::EmptyDeltaRasterRef,
    )?;
    let mask_raster_ref = normalize_required_text(
        generated_mask_raster_ref,
        TimeSeriesError::EmptyMaskRasterRef,
    )?;
    let config = normalize_change_config(config)?;
    validate_change_alignment(guard_proof, evidence, earlier, later)?;
    validate_aligned_grid(earlier)?;
    validate_aligned_grid(later)?;

    let mut delta_values = Vec::with_capacity(earlier.values.len());
    let mut change_mask = Vec::with_capacity(earlier.values.len());
    let mut changed_cell_count = 0_u32;
    for (earlier_value, later_value) in earlier.values.iter().zip(&later.values) {
        match (earlier_value, later_value) {
            (Some(earlier_value), Some(later_value)) => {
                let delta = later_value - earlier_value;
                if !delta.is_finite() {
                    return Err(TimeSeriesError::InvalidRasterCellValue);
                }
                let changed = delta.abs() >= config.absolute_threshold;
                if changed {
                    changed_cell_count += 1;
                }
                delta_values.push(Some(delta));
                change_mask.push(changed);
            }
            _ => {
                delta_values.push(None);
                change_mask.push(false);
            }
        }
    }

    Ok(RasterChangeResult {
        delta_raster_ref,
        mask_raster_ref,
        alignment_ref: evidence.alignment_ref.clone(),
        alignment_proof_ref: guard_proof.alignment_proof_ref.clone(),
        crs: evidence.target_crs.clone(),
        extent: evidence.aligned_extent,
        resolution: RasterResolution {
            x: evidence.target_resolution_x,
            y: evidence.target_resolution_y,
        },
        grid_columns: evidence.grid_columns,
        grid_rows: evidence.grid_rows,
        absolute_threshold: config.absolute_threshold,
        method_version: config.method_version,
        delta_values,
        change_mask,
        changed_cell_count,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SeriesKey {
    entity_ref: String,
    metric: String,
    t: String,
}

impl SeriesKey {
    fn from_point(point: &SeriesPoint) -> Self {
        Self {
            entity_ref: point.entity_ref.clone(),
            metric: point.metric.clone(),
            t: point.t.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct RasterAlignmentInput {
    raster_ref: String,
    crs: String,
    extent: GeoExtent,
    resolution: RasterResolution,
}

fn raster_alignment_input(point: &SeriesPoint) -> Result<RasterAlignmentInput, TimeSeriesError> {
    let SeriesValue::Raster(raster) = &point.value else {
        return Err(TimeSeriesError::AlignmentRequiresRasterPoint);
    };

    Ok(RasterAlignmentInput {
        raster_ref: raster.raster_ref.clone(),
        crs: raster
            .crs
            .clone()
            .ok_or(TimeSeriesError::MissingRasterCrs)?,
        extent: raster.extent.ok_or(TimeSeriesError::MissingRasterExtent)?,
        resolution: raster
            .resolution
            .ok_or(TimeSeriesError::MissingRasterResolution)?,
    })
}

fn normalize_alignment_config(
    config: RasterAlignmentConfig,
) -> Result<RasterAlignmentConfig, TimeSeriesError> {
    let resampling_method = normalize_required_text(
        config.resampling_method,
        TimeSeriesError::EmptyResamplingMethod,
    )?;
    if !config.target_resolution_x.is_finite()
        || !config.target_resolution_y.is_finite()
        || !config.minimum_overlap_ratio.is_finite()
        || config.target_resolution_x <= 0.0
        || config.target_resolution_y <= 0.0
        || !(0.0..=1.0).contains(&config.minimum_overlap_ratio)
    {
        return Err(TimeSeriesError::InvalidAlignmentConfig);
    }

    Ok(RasterAlignmentConfig {
        target_resolution_x: config.target_resolution_x,
        target_resolution_y: config.target_resolution_y,
        minimum_overlap_ratio: config.minimum_overlap_ratio,
        resampling_method,
    })
}

fn normalize_guard_config(
    config: AlignmentGuardConfig,
) -> Result<AlignmentGuardConfig, TimeSeriesError> {
    if !config.minimum_overlap_ratio.is_finite()
        || !(0.0..=1.0).contains(&config.minimum_overlap_ratio)
        || !config.resolution_tolerance.is_finite()
        || config.resolution_tolerance < 0.0
    {
        return Err(TimeSeriesError::InvalidAlignmentConfig);
    }

    Ok(config)
}

fn normalize_change_config(
    config: RasterChangeConfig,
) -> Result<RasterChangeConfig, TimeSeriesError> {
    let method_version = normalize_required_text(
        config.method_version,
        TimeSeriesError::EmptyChangeMethodVersion,
    )?;
    if !config.absolute_threshold.is_finite() || config.absolute_threshold < 0.0 {
        return Err(TimeSeriesError::InvalidChangeConfig);
    }

    Ok(RasterChangeConfig {
        absolute_threshold: config.absolute_threshold,
        method_version,
    })
}

fn normalize_metric_definition(
    mut definition: MetricDefinition,
) -> Result<MetricDefinition, TimeSeriesError> {
    definition.metric = normalize_required_text(definition.metric, TimeSeriesError::EmptyMetric)?;
    definition.unit = normalize_required_text(definition.unit, TimeSeriesError::EmptyUnit)?;
    definition.expected_cadence = normalize_required_text(
        definition.expected_cadence,
        TimeSeriesError::EmptyExpectedCadence {
            metric: definition.metric.clone(),
        },
    )?;
    Ok(definition)
}

fn normalize_product_ingest(
    mut ingest: SeriesProductIngest,
) -> Result<SeriesProductIngest, TimeSeriesError> {
    ingest.entity_ref =
        normalize_required_text(ingest.entity_ref, TimeSeriesError::EmptyEntityRef)?;
    ingest.metric = normalize_required_text(ingest.metric, TimeSeriesError::EmptyMetric)?;
    ingest.unit = normalize_required_text(ingest.unit, TimeSeriesError::EmptyUnit)?;
    ingest.source_ref =
        normalize_required_text(ingest.source_ref, TimeSeriesError::EmptySourceRef)?;
    ingest.product_ref =
        normalize_required_text(ingest.product_ref, TimeSeriesError::EmptyProductRef)?;
    ingest.product_date =
        normalize_required_text(ingest.product_date, TimeSeriesError::EmptyTimestamp)?;
    ingest.finalized_at =
        normalize_required_text(ingest.finalized_at, TimeSeriesError::EmptyCreatedAt)?;
    ingest.value = match ingest.value {
        SeriesValue::Scalar { value } => {
            if !value.is_finite() {
                return Err(TimeSeriesError::InvalidScalarValue);
            }
            SeriesValue::Scalar { value }
        }
        SeriesValue::Raster(raster) => SeriesValue::Raster(normalize_raster_value(raster)?),
    };
    Ok(ingest)
}

fn normalize_zonal_trend_target(
    mut target: ZonalTrendTarget,
) -> Result<ZonalTrendTarget, TimeSeriesError> {
    target.entity_ref =
        normalize_required_text(target.entity_ref, TimeSeriesError::EmptyEntityRef)?;
    target.metric = normalize_required_text(target.metric, TimeSeriesError::EmptyMetric)?;
    target.zone_ref = normalize_required_text(target.zone_ref, TimeSeriesError::EmptyZoneRef)?;
    target.zone_crs = normalize_required_text(target.zone_crs, TimeSeriesError::EmptyZoneCrs)?;
    Ok(target)
}

fn normalize_zonal_trend_config(
    config: ZonalTrendConfig,
) -> Result<ZonalTrendConfig, TimeSeriesError> {
    if config.min_points < 2
        || !config.flat_slope_epsilon.is_finite()
        || config.flat_slope_epsilon < 0.0
    {
        return Err(TimeSeriesError::InvalidTrendConfig);
    }
    Ok(config)
}

fn normalize_rolling_baseline_config(
    config: RollingBaselineConfig,
) -> Result<RollingBaselineConfig, TimeSeriesError> {
    if config.window_points == 0 || !config.anomaly_band.is_finite() || config.anomaly_band < 0.0 {
        return Err(TimeSeriesError::InvalidBaselineConfig);
    }
    Ok(config)
}

fn normalize_seasonal_comparison_target(
    mut target: SeasonalComparisonTarget,
) -> Result<SeasonalComparisonTarget, TimeSeriesError> {
    target.entity_ref =
        normalize_required_text(target.entity_ref, TimeSeriesError::EmptyEntityRef)?;
    target.metric = normalize_required_text(target.metric, TimeSeriesError::EmptyMetric)?;
    target.zone_ref = normalize_required_text(target.zone_ref, TimeSeriesError::EmptyZoneRef)?;
    target.zone_crs = normalize_required_text(target.zone_crs, TimeSeriesError::EmptyZoneCrs)?;
    target.current_t = normalize_required_text(target.current_t, TimeSeriesError::EmptyTimestamp)?;
    Ok(target)
}

fn normalize_seasonal_comparison_config(
    config: SeasonalComparisonConfig,
) -> Result<SeasonalComparisonConfig, TimeSeriesError> {
    if config.min_seasonal_points == 0 {
        return Err(TimeSeriesError::InvalidBaselineConfig);
    }
    Ok(config)
}

fn normalize_change_event_config(
    config: ChangeEventConfig,
) -> Result<ChangeEventConfig, TimeSeriesError> {
    if !config.magnitude_threshold.is_finite() || config.magnitude_threshold < 0.0 {
        return Err(TimeSeriesError::InvalidChangeEventConfig);
    }
    Ok(config)
}

fn metric_kind_for_value(value: &SeriesValue) -> MetricKind {
    match value {
        SeriesValue::Scalar { .. } => MetricKind::Scalar,
        SeriesValue::Raster(_) => MetricKind::Raster,
    }
}

fn timestamp_day_index(timestamp: &str) -> Result<i64, TimeSeriesError> {
    let (year, month, day) = date_parts(timestamp)?;
    Ok(days_from_civil(year, month, day))
}

fn timestamp_year_and_day(timestamp: &str) -> Result<(i32, u32), TimeSeriesError> {
    let (year, month, day) = date_parts(timestamp)?;
    Ok((year, day_of_year(year, month, day)))
}

fn date_parts(timestamp: &str) -> Result<(i32, u32, u32), TimeSeriesError> {
    let invalid = || TimeSeriesError::InvalidTrendTimestamp {
        timestamp: timestamp.to_string(),
    };
    let date = timestamp.get(0..10).ok_or_else(invalid)?;
    let bytes = date.as_bytes();
    if bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
        return Err(invalid());
    }
    let year = date[0..4].parse::<i32>().map_err(|_| invalid())?;
    let month = date[5..7].parse::<u32>().map_err(|_| invalid())?;
    let day = date[8..10].parse::<u32>().map_err(|_| invalid())?;
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return Err(invalid());
    }

    Ok((year, month, day))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn day_of_year(year: i32, month: u32, day: u32) -> u32 {
    let days_before_month = (1..month)
        .map(|previous_month| days_in_month(year, previous_month))
        .sum::<u32>();
    days_before_month + day
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = month as i32;
    let day = day as i32;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    i64::from(era * 146_097 + day_of_era - 719_468)
}

fn least_squares_trend(samples: &[(f64, f64)]) -> Result<(f64, f64, f64), TimeSeriesError> {
    if samples.len() < 2 {
        return Err(TimeSeriesError::InvalidTrendConfig);
    }
    let n = samples.len() as f64;
    let sum_x = samples.iter().map(|(x, _)| *x).sum::<f64>();
    let sum_y = samples.iter().map(|(_, y)| *y).sum::<f64>();
    let sum_xx = samples.iter().map(|(x, _)| x * x).sum::<f64>();
    let sum_xy = samples.iter().map(|(x, y)| x * y).sum::<f64>();
    let denominator = n * sum_xx - sum_x * sum_x;
    if !denominator.is_finite() || denominator.abs() < f64::EPSILON {
        return Err(TimeSeriesError::InvalidTrendTimestamp {
            timestamp: "duplicate trend timestamps".to_string(),
        });
    }
    let slope = (n * sum_xy - sum_x * sum_y) / denominator;
    let intercept = (sum_y - slope * sum_x) / n;
    let mean_y = sum_y / n;
    let total_sum_squares = samples
        .iter()
        .map(|(_, y)| {
            let diff = y - mean_y;
            diff * diff
        })
        .sum::<f64>();
    let residual_sum_squares = samples
        .iter()
        .map(|(x, y)| {
            let predicted = slope * x + intercept;
            let diff = y - predicted;
            diff * diff
        })
        .sum::<f64>();
    let fit_r_squared = if total_sum_squares.abs() < f64::EPSILON {
        1.0
    } else {
        1.0 - residual_sum_squares / total_sum_squares
    };
    Ok((slope, intercept, fit_r_squared.clamp(0.0, 1.0)))
}

fn scalar_value_from_point(point: &SeriesPoint) -> Result<f64, TimeSeriesError> {
    match point.value {
        SeriesValue::Scalar { value } => Ok(value),
        SeriesValue::Raster(_) => Err(TimeSeriesError::TrendRequiresScalarPoint {
            entity_ref: point.entity_ref.clone(),
            metric: point.metric.clone(),
        }),
    }
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn csv_row<'a>(fields: impl IntoIterator<Item = &'a str>) -> String {
    let mut row = fields
        .into_iter()
        .map(csv_escape)
        .collect::<Vec<_>>()
        .join(",");
    row.push('\n');
    row
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn validate_raster_change_result(change: &RasterChangeResult) -> Result<(), TimeSeriesError> {
    normalize_required_text(
        change.mask_raster_ref.clone(),
        TimeSeriesError::EmptyMaskRasterRef,
    )?;
    normalize_required_text(
        change.alignment_ref.clone(),
        TimeSeriesError::EmptyAlignmentRef,
    )?;
    normalize_required_text(
        change.alignment_proof_ref.clone(),
        TimeSeriesError::EmptyAlignmentRef,
    )?;
    normalize_required_text(change.crs.clone(), TimeSeriesError::MissingRasterCrs)?;
    normalize_raster_resolution(change.resolution)?;
    if !change.absolute_threshold.is_finite() || change.absolute_threshold < 0.0 {
        return Err(TimeSeriesError::InvalidChangeConfig);
    }
    normalize_required_text(
        change.method_version.clone(),
        TimeSeriesError::EmptyChangeMethodVersion,
    )?;
    let expected_len = usize::try_from(change.grid_columns)
        .ok()
        .and_then(|columns| {
            usize::try_from(change.grid_rows)
                .ok()
                .and_then(|rows| columns.checked_mul(rows))
        })
        .ok_or(TimeSeriesError::InvalidRasterCellCount)?;
    if change.change_mask.len() != expected_len || change.delta_values.len() != expected_len {
        return Err(TimeSeriesError::InvalidRasterCellCount);
    }
    if change
        .delta_values
        .iter()
        .flatten()
        .any(|value| !value.is_finite())
    {
        return Err(TimeSeriesError::InvalidRasterCellValue);
    }
    Ok(())
}

fn validate_compare_view_inputs(
    proof: &AlignmentGuardProof,
    evidence: &RasterAlignmentEvidence,
    change: &RasterChangeResult,
) -> Result<(), CompareViewRefusal> {
    validate_raster_change_result(change)
        .map_err(|error| compare_view_input_refusal(proof, evidence, error.to_string()))?;
    normalize_required_text(
        change.delta_raster_ref.clone(),
        TimeSeriesError::EmptyDeltaRasterRef,
    )
    .map_err(|error| compare_view_input_refusal(proof, evidence, error.to_string()))?;

    let expected_resolution = RasterResolution {
        x: evidence.target_resolution_x,
        y: evidence.target_resolution_y,
    };
    let changed_cell_count = change
        .change_mask
        .iter()
        .filter(|changed| **changed)
        .count();
    let mut mismatches = Vec::new();

    if proof.entity_ref != evidence.entity_ref {
        mismatches.push(format!(
            "entity_ref proof={} evidence={}",
            proof.entity_ref, evidence.entity_ref
        ));
    }
    if proof.metric != evidence.metric {
        mismatches.push(format!(
            "metric proof={} evidence={}",
            proof.metric, evidence.metric
        ));
    }
    if proof.earlier_t != evidence.earlier_t || proof.later_t != evidence.later_t {
        mismatches.push(format!(
            "time pair proof={}/{} evidence={}/{}",
            proof.earlier_t, proof.later_t, evidence.earlier_t, evidence.later_t
        ));
    }
    if proof.earlier_raster_ref != evidence.earlier_raster_ref
        || proof.later_raster_ref != evidence.later_raster_ref
    {
        mismatches.push(format!(
            "source rasters proof={}/{} evidence={}/{}",
            proof.earlier_raster_ref,
            proof.later_raster_ref,
            evidence.earlier_raster_ref,
            evidence.later_raster_ref
        ));
    }
    if proof.target_crs != evidence.target_crs || evidence.target_crs != change.crs {
        mismatches.push(format!(
            "crs proof={} evidence={} change={}",
            proof.target_crs, evidence.target_crs, change.crs
        ));
    }
    if proof.alignment_proof_ref != change.alignment_proof_ref {
        mismatches.push(format!(
            "alignment_proof_ref proof={} change={}",
            proof.alignment_proof_ref, change.alignment_proof_ref
        ));
    }
    if evidence.alignment_ref != change.alignment_ref {
        mismatches.push(format!(
            "alignment_ref evidence={} change={}",
            evidence.alignment_ref, change.alignment_ref
        ));
    }
    if proof.overlap_ratio_basis_points != evidence.overlap_ratio_basis_points {
        mismatches.push(format!(
            "overlap proof={}bp evidence={}bp",
            proof.overlap_ratio_basis_points, evidence.overlap_ratio_basis_points
        ));
    }
    if proof.earlier_resolution != evidence.source_earlier_resolution
        || proof.later_resolution != evidence.source_later_resolution
    {
        mismatches.push("source resolution proof/evidence mismatch".to_string());
    }
    if evidence.aligned_extent != change.extent {
        mismatches.push("aligned_extent evidence/change mismatch".to_string());
    }
    if expected_resolution != change.resolution {
        mismatches.push("resolution evidence/change mismatch".to_string());
    }
    if evidence.grid_columns != change.grid_columns || evidence.grid_rows != change.grid_rows {
        mismatches.push(format!(
            "grid evidence={}x{} change={}x{}",
            evidence.grid_columns, evidence.grid_rows, change.grid_columns, change.grid_rows
        ));
    }
    if changed_cell_count != change.changed_cell_count as usize {
        mismatches.push(format!(
            "changed_cell_count mask={} metadata={}",
            changed_cell_count, change.changed_cell_count
        ));
    }

    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(compare_view_input_refusal(
            proof,
            evidence,
            format!(
                "compare-view inputs are not one co-registered grid: {}",
                mismatches.join("; ")
            ),
        ))
    }
}

fn compare_view_input_refusal(
    proof: &AlignmentGuardProof,
    evidence: &RasterAlignmentEvidence,
    mismatch_detail: String,
) -> CompareViewRefusal {
    CompareViewRefusal {
        schema_version: "timeseries.compare_view_refusal.v1".to_string(),
        reason_code: AlignmentRefusalReason::InvalidGuardConfig,
        mismatch_detail,
        earlier_raster_ref: Some(evidence.earlier_raster_ref.clone()),
        later_raster_ref: Some(evidence.later_raster_ref.clone()),
        alignment_proof_ref: Some(proof.alignment_proof_ref.clone()),
        no_misaligned_panes: true,
    }
}

fn normalize_change_zone_polygon(
    polygon: ChangeZonePolygon,
    expected_crs: &str,
) -> Result<ChangeZonePolygon, TimeSeriesError> {
    let actual_crs = normalize_required_text(polygon.crs, TimeSeriesError::EmptyZoneCrs)?;
    if actual_crs != expected_crs {
        return Err(TimeSeriesError::ChangeZoneCrsMismatch {
            expected_crs: expected_crs.to_string(),
            actual_crs,
        });
    }
    if polygon.rings.is_empty() || !polygon.rings.iter().all(valid_polygon_ring) {
        return Err(TimeSeriesError::InvalidChangeZoneGeometry);
    }

    Ok(ChangeZonePolygon {
        crs: expected_crs.to_string(),
        rings: polygon.rings,
    })
}

fn valid_polygon_ring(ring: &Vec<[f64; 2]>) -> bool {
    ring.len() >= 4
        && ring
            .iter()
            .all(|coordinate| coordinate[0].is_finite() && coordinate[1].is_finite())
        && ring.first() == ring.last()
}

fn validate_change_alignment(
    guard_proof: &AlignmentGuardProof,
    evidence: &RasterAlignmentEvidence,
    earlier: &AlignedRasterGrid,
    later: &AlignedRasterGrid,
) -> Result<(), TimeSeriesError> {
    let expected_resolution = RasterResolution {
        x: evidence.target_resolution_x,
        y: evidence.target_resolution_y,
    };
    let matches = guard_proof.target_crs == evidence.target_crs
        && earlier.raster_ref == evidence.aligned_earlier_ref
        && later.raster_ref == evidence.aligned_later_ref
        && earlier.alignment_ref == evidence.alignment_ref
        && later.alignment_ref == evidence.alignment_ref
        && earlier.crs == evidence.target_crs
        && later.crs == evidence.target_crs
        && earlier.extent == evidence.aligned_extent
        && later.extent == evidence.aligned_extent
        && earlier.resolution == expected_resolution
        && later.resolution == expected_resolution
        && earlier.grid_columns == evidence.grid_columns
        && later.grid_columns == evidence.grid_columns
        && earlier.grid_rows == evidence.grid_rows
        && later.grid_rows == evidence.grid_rows;

    if matches {
        Ok(())
    } else {
        Err(TimeSeriesError::ChangeAlignmentMismatch)
    }
}

fn validate_aligned_grid(grid: &AlignedRasterGrid) -> Result<(), TimeSeriesError> {
    let expected_len = usize::try_from(grid.grid_columns)
        .ok()
        .and_then(|columns| {
            usize::try_from(grid.grid_rows)
                .ok()
                .and_then(|rows| columns.checked_mul(rows))
        })
        .ok_or(TimeSeriesError::InvalidRasterCellCount)?;
    if grid.values.len() != expected_len {
        return Err(TimeSeriesError::InvalidRasterCellCount);
    }
    if grid.values.iter().flatten().any(|value| !value.is_finite()) {
        return Err(TimeSeriesError::InvalidRasterCellValue);
    }
    Ok(())
}

fn extent_intersection(a: GeoExtent, b: GeoExtent) -> Option<GeoExtent> {
    let intersection = GeoExtent {
        min_x: a.min_x.max(b.min_x),
        min_y: a.min_y.max(b.min_y),
        max_x: a.max_x.min(b.max_x),
        max_y: a.max_y.min(b.max_y),
    };
    (intersection.min_x < intersection.max_x && intersection.min_y < intersection.max_y)
        .then_some(intersection)
}

fn extent_area(extent: GeoExtent) -> f64 {
    (extent.max_x - extent.min_x) * (extent.max_y - extent.min_y)
}

fn ratio_to_basis_points(ratio: f64) -> u32 {
    (ratio.clamp(0.0, 1.0) * 10_000.0).round() as u32
}

fn overlap_ratio_basis_points(a: GeoExtent, b: GeoExtent) -> u32 {
    let overlap_area = extent_intersection(a, b).map_or(0.0, extent_area);
    let denominator = extent_area(a).min(extent_area(b));
    let ratio = if denominator > 0.0 {
        overlap_area / denominator
    } else {
        0.0
    };
    ratio_to_basis_points(ratio)
}

fn resolution_compatible(
    earlier: RasterResolution,
    later: RasterResolution,
    tolerance: f64,
) -> bool {
    (earlier.x - later.x).abs() <= tolerance && (earlier.y - later.y).abs() <= tolerance
}

fn grid_cell_count(distance: f64, resolution: f64) -> Result<u32, TimeSeriesError> {
    let cells = (distance / resolution).floor();
    if cells < 1.0 {
        Err(TimeSeriesError::InvalidAlignedGrid)
    } else {
        Ok(cells as u32)
    }
}

fn raster_ref_from_point(point: &SeriesPoint) -> Option<String> {
    match &point.value {
        SeriesValue::Raster(raster) => Some(raster.raster_ref.clone()),
        SeriesValue::Scalar { .. } => None,
    }
}

fn guard_refusal(
    reason_code: AlignmentRefusalReason,
    mismatch_detail: String,
    earlier_raster_ref: Option<String>,
    later_raster_ref: Option<String>,
) -> AlignmentGuardRefusal {
    AlignmentGuardRefusal {
        reason_code,
        mismatch_detail,
        earlier_raster_ref,
        later_raster_ref,
        alignment_proof_ref: None,
        change_job_blocked: true,
    }
}

fn guard_reason_from_error(error: &TimeSeriesError) -> AlignmentRefusalReason {
    match error {
        TimeSeriesError::AlignmentRequiresRasterPoint => AlignmentRefusalReason::NotRasterPoint,
        TimeSeriesError::MissingRasterCrs => AlignmentRefusalReason::MissingCrs,
        TimeSeriesError::MissingRasterExtent => AlignmentRefusalReason::MissingExtent,
        TimeSeriesError::MissingRasterResolution => AlignmentRefusalReason::MissingResolution,
        _ => AlignmentRefusalReason::InvalidGuardConfig,
    }
}

fn normalize_point(point: SeriesPoint) -> Result<SeriesPoint, TimeSeriesError> {
    let value = match point.value {
        SeriesValue::Scalar { value } => {
            if !value.is_finite() {
                return Err(TimeSeriesError::InvalidScalarValue);
            }
            SeriesValue::Scalar { value }
        }
        SeriesValue::Raster(raster) => SeriesValue::Raster(normalize_raster_value(raster)?),
    };

    Ok(SeriesPoint {
        entity_ref: normalize_required_text(point.entity_ref, TimeSeriesError::EmptyEntityRef)?,
        metric: normalize_required_text(point.metric, TimeSeriesError::EmptyMetric)?,
        unit: normalize_required_text(point.unit, TimeSeriesError::EmptyUnit)?,
        t: normalize_required_text(point.t, TimeSeriesError::EmptyTimestamp)?,
        value,
        source_ref: normalize_required_text(point.source_ref, TimeSeriesError::EmptySourceRef)?,
        created_at: normalize_required_text(point.created_at, TimeSeriesError::EmptyCreatedAt)?,
    })
}

fn normalize_raster_value(value: RasterSeriesValue) -> Result<RasterSeriesValue, TimeSeriesError> {
    if let Some(extent) = value.extent {
        if !extent.min_x.is_finite()
            || !extent.min_y.is_finite()
            || !extent.max_x.is_finite()
            || !extent.max_y.is_finite()
            || extent.min_x >= extent.max_x
            || extent.min_y >= extent.max_y
        {
            return Err(TimeSeriesError::InvalidExtent);
        }
    }

    Ok(RasterSeriesValue {
        raster_ref: normalize_required_text(value.raster_ref, TimeSeriesError::EmptyRasterRef)?,
        crs: normalize_optional_text(value.crs),
        extent: value.extent,
        resolution: value
            .resolution
            .map(normalize_raster_resolution)
            .transpose()?,
    })
}

fn normalize_raster_resolution(
    resolution: RasterResolution,
) -> Result<RasterResolution, TimeSeriesError> {
    if resolution.x.is_finite()
        && resolution.y.is_finite()
        && resolution.x > 0.0
        && resolution.y > 0.0
    {
        Ok(resolution)
    } else {
        Err(TimeSeriesError::InvalidRasterResolution)
    }
}

fn normalize_required_text(
    value: String,
    error: TimeSeriesError,
) -> Result<String, TimeSeriesError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        align_raster_pair, build_compare_view_feed, compare_view_refusal_from_guard,
        compute_aligned_raster_change, derive_ranked_change_events, export_change_mask_geotiff,
        export_change_zones_geojson, export_series_csv, guard_coregisterable_pair,
        AlignedRasterGrid, AlignmentGuardConfig, AlignmentGuardProof, AlignmentRefusalReason,
        ChangeEvent, ChangeEventConfig, ChangeEventDerivationInput, ChangeEventDirection,
        ChangeEventReasonCode, ChangeZoneExportFeature, ChangeZonePolygon, GeoExtent,
        MetricDefinition, MetricKind, RasterAlignmentConfig, RasterAlignmentEvidence,
        RasterChangeConfig, RasterChangeResult, RasterResolution, RasterSeriesValue,
        RollingBaselineConfig, SeasonalComparisonConfig, SeasonalComparisonTarget, SeriesPoint,
        SeriesProductIngest, SeriesQuery, SeriesValue, TimeRange, TimeSeriesEngine,
        TimeSeriesError, TimeSeriesStore, TrendDirection, ZonalTrendConfig, ZonalTrendTarget,
    };

    #[test]
    fn scalar_points_are_retrieved_in_time_order() {
        let mut store = TimeSeriesStore::default();
        store
            .append(scalar_point(
                "field:alpha",
                "ndvi_mean",
                "2026-06-12T10:00:00Z",
                0.72,
            ))
            .expect("first point should append");
        store
            .append(scalar_point(
                "field:alpha",
                "ndvi_mean",
                "2026-06-10T10:00:00Z",
                0.68,
            ))
            .expect("second point should append");

        let points = store.query("field:alpha", "ndvi_mean", TimeRange::default());

        assert_eq!(points.len(), 2);
        assert_eq!(points[0].t, "2026-06-10T10:00:00Z");
        assert_eq!(points[1].t, "2026-06-12T10:00:00Z");
    }

    #[test]
    fn mixed_scalar_and_raster_points_round_trip_with_spatial_metadata() {
        let mut store = TimeSeriesStore::default();
        store
            .append(scalar_point(
                "field:alpha",
                "ndvi_mean",
                "2026-06-10T10:00:00Z",
                0.68,
            ))
            .expect("scalar point should append");
        store
            .append(SeriesPoint {
                entity_ref: "field:alpha".to_string(),
                metric: "ndvi_raster".to_string(),
                unit: "index".to_string(),
                t: "2026-06-10T10:00:00Z".to_string(),
                value: SeriesValue::Raster(RasterSeriesValue {
                    raster_ref: "product:scene-001:ndvi".to_string(),
                    crs: Some("EPSG:4326".to_string()),
                    extent: Some(GeoExtent {
                        min_x: -121.5,
                        min_y: 38.5,
                        max_x: -121.4,
                        max_y: 38.6,
                    }),
                    resolution: Some(RasterResolution { x: 0.01, y: 0.01 }),
                }),
                source_ref: "scene:scene-001".to_string(),
                created_at: "2026-06-12T12:00:00Z".to_string(),
            })
            .expect("raster point should append");

        let rasters = store.query("field:alpha", "ndvi_raster", TimeRange::default());
        assert_eq!(rasters.len(), 1);
        match &rasters[0].value {
            SeriesValue::Raster(value) => {
                assert_eq!(value.raster_ref, "product:scene-001:ndvi");
                assert_eq!(value.crs.as_deref(), Some("EPSG:4326"));
                assert_eq!(
                    value.resolution,
                    Some(RasterResolution { x: 0.01, y: 0.01 })
                );
                assert_eq!(
                    value.extent,
                    Some(GeoExtent {
                        min_x: -121.5,
                        min_y: 38.5,
                        max_x: -121.4,
                        max_y: 38.6,
                    })
                );
            }
            SeriesValue::Scalar { .. } => panic!("expected raster point"),
        }
    }

    #[test]
    fn duplicate_entity_metric_timestamp_is_rejected() {
        let mut store = TimeSeriesStore::default();
        let point = scalar_point("field:alpha", "ndvi_mean", "2026-06-12T10:00:00Z", 0.72);
        store
            .append(point.clone())
            .expect("first point should append");
        let error = store
            .append(point)
            .expect_err("duplicate key should be rejected");

        assert_eq!(
            error,
            TimeSeriesError::DuplicateSeriesPoint {
                entity_ref: "field:alpha".to_string(),
                metric: "ndvi_mean".to_string(),
                t: "2026-06-12T10:00:00Z".to_string()
            }
        );
    }

    #[test]
    fn reusable_api_appends_queries_and_lists_metrics_with_pagination() {
        let mut engine = TimeSeriesEngine::default();
        engine
            .register_metric(metric_definition("ndvi_mean", "index", MetricKind::Scalar))
            .expect("ndvi metric should register");
        engine
            .register_metric(metric_definition(
                "soil_moisture",
                "percent",
                MetricKind::Scalar,
            ))
            .expect("soil metric should register");
        engine
            .append(scalar_point_with_unit(
                "field:alpha",
                "ndvi_mean",
                "index",
                "2026-06-10T10:00:00Z",
                0.68,
            ))
            .expect("first point should append");
        engine
            .append(scalar_point_with_unit(
                "field:alpha",
                "ndvi_mean",
                "index",
                "2026-06-12T10:00:00Z",
                0.72,
            ))
            .expect("second point should append");
        engine
            .append(scalar_point_with_unit(
                "field:alpha",
                "soil_moisture",
                "percent",
                "2026-06-12T11:00:00Z",
                34.0,
            ))
            .expect("third point should append");

        let first_page = engine.query(SeriesQuery {
            entity_ref: "field:alpha".to_string(),
            metric: "ndvi_mean".to_string(),
            range: TimeRange::default(),
            limit: Some(1),
            cursor: None,
        });
        assert!(!first_page.no_series);
        assert_eq!(first_page.points.len(), 1);
        assert_eq!(first_page.next_cursor, Some(1));

        let second_page = engine.query(SeriesQuery {
            entity_ref: "field:alpha".to_string(),
            metric: "ndvi_mean".to_string(),
            range: TimeRange::default(),
            limit: Some(1),
            cursor: first_page.next_cursor,
        });
        assert_eq!(second_page.points.len(), 1);
        assert_eq!(second_page.next_cursor, None);

        assert_eq!(
            engine.list_metrics("field:alpha"),
            vec!["ndvi_mean".to_string(), "soil_moisture".to_string()]
        );
    }

    #[test]
    fn reusable_api_unknown_metric_returns_empty_marker() {
        let engine = TimeSeriesEngine::default();
        let page = engine.query(SeriesQuery {
            entity_ref: "field:missing".to_string(),
            metric: "ndvi_mean".to_string(),
            range: TimeRange::default(),
            limit: Some(25),
            cursor: None,
        });

        assert!(page.no_series);
        assert!(page.points.is_empty());
        assert_eq!(page.next_cursor, None);
    }

    #[test]
    fn metric_registry_accepts_matching_points_and_rejects_unknown_or_unit_mismatch() {
        let mut engine = TimeSeriesEngine::default();
        engine
            .register_metric(metric_definition("ndvi_mean", "index", MetricKind::Scalar))
            .expect("metric should register");

        engine
            .append(scalar_point_with_unit(
                "field:alpha",
                "ndvi_mean",
                "index",
                "2026-06-10T10:00:00Z",
                0.68,
            ))
            .expect("registered unit should append");

        let unknown_error = engine
            .append(scalar_point_with_unit(
                "field:alpha",
                "soil_moisture",
                "percent",
                "2026-06-10T10:00:00Z",
                34.0,
            ))
            .expect_err("unknown metric should be refused");
        assert_eq!(
            unknown_error,
            TimeSeriesError::UnknownMetric {
                metric: "soil_moisture".to_string()
            }
        );

        let mismatch_error = engine
            .append(scalar_point_with_unit(
                "field:alpha",
                "ndvi_mean",
                "percent",
                "2026-06-12T10:00:00Z",
                72.0,
            ))
            .expect_err("unit mismatch should be refused");
        assert_eq!(
            mismatch_error,
            TimeSeriesError::MetricUnitMismatch {
                metric: "ndvi_mean".to_string(),
                expected_unit: "index".to_string(),
                actual_unit: "percent".to_string()
            }
        );
    }

    #[test]
    fn product_ingest_records_fresh_raster_point_and_duplicate_conflict() {
        let mut engine = TimeSeriesEngine::default();
        engine
            .register_metric(metric_definition(
                "ndvi_raster",
                "index",
                MetricKind::Raster,
            ))
            .expect("raster metric should register");

        let first = engine
            .ingest_product(sample_product_ingest(
                "scene:001",
                "product:scene-001:ndvi",
                "2026-06-10T10:00:00Z",
            ))
            .expect("first product should ingest");
        assert!(first.conflict.is_none());
        assert_eq!(first.point.t, "2026-06-10T10:00:00Z");
        assert_eq!(first.point.source_ref, "scene:001");
        match &first.point.value {
            SeriesValue::Raster(raster) => {
                assert_eq!(raster.crs.as_deref(), Some("EPSG:32610"));
                assert_eq!(raster.extent, Some(default_extent()));
            }
            SeriesValue::Scalar { .. } => panic!("expected raster ingest"),
        }

        let duplicate = engine
            .ingest_product(sample_product_ingest(
                "scene:002",
                "product:scene-002:ndvi",
                "2026-06-10T10:00:00Z",
            ))
            .expect("duplicate should record a deterministic conflict");
        let conflict = duplicate
            .conflict
            .expect("duplicate should report conflict");
        assert_eq!(conflict.existing_source_ref, "scene:001");
        assert_eq!(conflict.incoming_source_ref, "scene:002");
        assert_eq!(engine.ingest_conflicts().len(), 1);

        let stored = engine.query(SeriesQuery {
            entity_ref: "field:alpha".to_string(),
            metric: "ndvi_raster".to_string(),
            range: TimeRange::default(),
            limit: None,
            cursor: None,
        });
        assert_eq!(stored.points.len(), 1);
        assert_eq!(stored.points[0].source_ref, "scene:001");
    }

    #[test]
    fn zonal_trend_returns_slope_direction_fit_and_contributing_points() {
        let mut engine = TimeSeriesEngine::default();
        engine
            .register_metric(metric_definition("ndvi_mean", "index", MetricKind::Scalar))
            .expect("metric should register");
        for (date, value) in [
            ("2026-06-10T10:00:00Z", 0.60),
            ("2026-06-12T10:00:00Z", 0.70),
            ("2026-06-14T10:00:00Z", 0.80),
        ] {
            engine
                .append(scalar_point_with_unit(
                    "field:alpha",
                    "ndvi_mean",
                    "index",
                    date,
                    value,
                ))
                .expect("trend point should append");
        }

        let trend = engine
            .compute_zonal_trend(
                ZonalTrendTarget {
                    entity_ref: "field:alpha".to_string(),
                    metric: "ndvi_mean".to_string(),
                    zone_ref: "zone:NE".to_string(),
                    zone_crs: "EPSG:32610".to_string(),
                    range: TimeRange::default(),
                },
                ZonalTrendConfig {
                    min_points: 3,
                    flat_slope_epsilon: 0.001,
                },
            )
            .expect("three points should produce a trend");

        assert_eq!(trend.direction, TrendDirection::Increasing);
        assert!((trend.slope_per_day - 0.05).abs() < 0.000001);
        assert!(trend.fit_r_squared > 0.999);
        assert_eq!(trend.zone_ref, "zone:NE");
        assert_eq!(trend.zone_crs, "EPSG:32610");
        assert_eq!(trend.points_used.len(), 3);
        assert_eq!(trend.evidence_refs.len(), 3);
    }

    #[test]
    fn zonal_trend_refuses_insufficient_history() {
        let mut engine = TimeSeriesEngine::default();
        engine
            .register_metric(metric_definition("ndvi_mean", "index", MetricKind::Scalar))
            .expect("metric should register");
        engine
            .append(scalar_point_with_unit(
                "field:alpha",
                "ndvi_mean",
                "index",
                "2026-06-10T10:00:00Z",
                0.60,
            ))
            .expect("one point should append");

        let error = engine
            .compute_zonal_trend(
                ZonalTrendTarget {
                    entity_ref: "field:alpha".to_string(),
                    metric: "ndvi_mean".to_string(),
                    zone_ref: "zone:NE".to_string(),
                    zone_crs: "EPSG:32610".to_string(),
                    range: TimeRange::default(),
                },
                ZonalTrendConfig {
                    min_points: 3,
                    flat_slope_epsilon: 0.001,
                },
            )
            .expect_err("one point should be insufficient");

        assert_eq!(
            error,
            TimeSeriesError::InsufficientTrendHistory {
                entity_ref: "field:alpha".to_string(),
                metric: "ndvi_mean".to_string(),
                observed_points: 1,
                required_points: 3
            }
        );
    }

    #[test]
    fn rolling_and_seasonal_baselines_record_windows_and_deltas() {
        let engine = seeded_baseline_engine();

        let rolling = engine
            .compute_rolling_baseline(
                ZonalTrendTarget {
                    entity_ref: "field:alpha".to_string(),
                    metric: "ndvi_mean".to_string(),
                    zone_ref: "zone:NE".to_string(),
                    zone_crs: "EPSG:32610".to_string(),
                    range: TimeRange {
                        start: Some("2026-01-01T00:00:00Z".to_string()),
                        end: None,
                    },
                },
                RollingBaselineConfig {
                    window_points: 2,
                    anomaly_band: 0.10,
                },
            )
            .expect("rolling baseline should compute");

        assert_eq!(rolling.baseline_window.len(), 2);
        assert!((rolling.baseline_mean - 0.71).abs() < 0.000001);
        assert!((rolling.latest_value - 0.50).abs() < 0.000001);
        assert!((rolling.delta_from_baseline + 0.21).abs() < 0.000001);
        assert!(rolling.anomaly);

        let seasonal = engine
            .compute_seasonal_comparison(
                SeasonalComparisonTarget {
                    entity_ref: "field:alpha".to_string(),
                    metric: "ndvi_mean".to_string(),
                    zone_ref: "zone:NE".to_string(),
                    zone_crs: "EPSG:32610".to_string(),
                    current_t: "2026-06-14T10:00:00Z".to_string(),
                },
                SeasonalComparisonConfig {
                    min_seasonal_points: 2,
                    day_of_year_tolerance: 1,
                },
            )
            .expect("seasonal comparison should find prior seasons");

        assert_eq!(seasonal.seasonal_points.len(), 2);
        assert!((seasonal.seasonal_mean - 0.65).abs() < 0.000001);
        assert!((seasonal.delta_from_seasonal_baseline + 0.15).abs() < 0.000001);
    }

    #[test]
    fn seasonal_comparison_refuses_without_matching_history() {
        let mut engine = TimeSeriesEngine::default();
        engine
            .register_metric(metric_definition("ndvi_mean", "index", MetricKind::Scalar))
            .expect("metric should register");
        engine
            .append(scalar_point_with_unit(
                "field:alpha",
                "ndvi_mean",
                "index",
                "2026-06-14T10:00:00Z",
                0.50,
            ))
            .expect("current point should append");

        let error = engine
            .compute_seasonal_comparison(
                SeasonalComparisonTarget {
                    entity_ref: "field:alpha".to_string(),
                    metric: "ndvi_mean".to_string(),
                    zone_ref: "zone:NE".to_string(),
                    zone_crs: "EPSG:32610".to_string(),
                    current_t: "2026-06-14T10:00:00Z".to_string(),
                },
                SeasonalComparisonConfig {
                    min_seasonal_points: 1,
                    day_of_year_tolerance: 0,
                },
            )
            .expect_err("missing prior season should be refused");

        assert_eq!(
            error,
            TimeSeriesError::NoSeasonalBaseline {
                entity_ref: "field:alpha".to_string(),
                metric: "ndvi_mean".to_string(),
                current_t: "2026-06-14T10:00:00Z".to_string(),
                observed_points: 0,
                required_points: 1
            }
        );
    }

    #[test]
    fn ranked_change_events_cite_mask_trend_zone_and_baseline_evidence() {
        let engine = seeded_baseline_engine();
        let trend = engine
            .compute_zonal_trend(
                ZonalTrendTarget {
                    entity_ref: "field:alpha".to_string(),
                    metric: "ndvi_mean".to_string(),
                    zone_ref: "zone:NE".to_string(),
                    zone_crs: "EPSG:32610".to_string(),
                    range: TimeRange {
                        start: Some("2026-01-01T00:00:00Z".to_string()),
                        end: None,
                    },
                },
                ZonalTrendConfig {
                    min_points: 3,
                    flat_slope_epsilon: 0.001,
                },
            )
            .expect("trend should compute");
        let baseline = engine
            .compute_rolling_baseline(
                ZonalTrendTarget {
                    entity_ref: "field:alpha".to_string(),
                    metric: "ndvi_mean".to_string(),
                    zone_ref: "zone:NE".to_string(),
                    zone_crs: "EPSG:32610".to_string(),
                    range: TimeRange {
                        start: Some("2026-01-01T00:00:00Z".to_string()),
                        end: None,
                    },
                },
                RollingBaselineConfig {
                    window_points: 2,
                    anomaly_band: 0.10,
                },
            )
            .expect("baseline should compute");
        let change = sample_drop_change_result();

        let events = derive_ranked_change_events(
            vec![ChangeEventDerivationInput {
                change,
                trend,
                baseline,
            }],
            ChangeEventConfig {
                magnitude_threshold: 0.10,
                min_changed_cells: 1,
            },
        )
        .expect("change event derivation should run");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].zone_ref, "zone:NE");
        assert_eq!(events[0].direction, ChangeEventDirection::Dropped);
        assert_eq!(events[0].reason_code, ChangeEventReasonCode::BaselineDrop);
        assert!((events[0].magnitude - 0.21).abs() < 0.000001);
        assert!(events[0].summary.contains("dropped"));
        assert!(events[0]
            .evidence_refs
            .iter()
            .any(|reference| reference == "alignment:field-alpha:ndvi"));
        assert!(events[0]
            .evidence_refs
            .iter()
            .any(|reference| reference == "zone:NE"));
    }

    #[test]
    fn change_event_derivation_returns_zero_for_subthreshold_change() {
        let engine = seeded_baseline_engine();
        let trend = engine
            .compute_zonal_trend(
                trend_target_2026(),
                ZonalTrendConfig {
                    min_points: 3,
                    flat_slope_epsilon: 0.001,
                },
            )
            .expect("trend should compute");
        let baseline = engine
            .compute_rolling_baseline(
                trend_target_2026(),
                RollingBaselineConfig {
                    window_points: 2,
                    anomaly_band: 0.10,
                },
            )
            .expect("baseline should compute");

        let events = derive_ranked_change_events(
            vec![ChangeEventDerivationInput {
                change: sample_drop_change_result(),
                trend,
                baseline,
            }],
            ChangeEventConfig {
                magnitude_threshold: 0.50,
                min_changed_cells: 1,
            },
        )
        .expect("change event derivation should run");

        assert!(events.is_empty());
    }

    #[test]
    fn raster_alignment_records_shared_grid_and_evidence() {
        let earlier = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-10T10:00:00Z",
            "product:scene-001:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 10.0,
            },
        );
        let later = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-12T10:00:00Z",
            "product:scene-002:ndvi",
            GeoExtent {
                min_x: 2.0,
                min_y: 2.0,
                max_x: 10.0,
                max_y: 10.0,
            },
        );

        let evidence = align_raster_pair(
            &earlier,
            &later,
            alignment_config(1.0, 1.0, 0.75),
            "alignment:field-alpha:ndvi:2026-06-10:2026-06-12".to_string(),
        )
        .expect("compatible rasters should align");

        assert_eq!(
            evidence.alignment_ref,
            "alignment:field-alpha:ndvi:2026-06-10:2026-06-12"
        );
        assert_eq!(evidence.target_crs, "EPSG:32610");
        assert_eq!(
            evidence.aligned_extent,
            GeoExtent {
                min_x: 2.0,
                min_y: 2.0,
                max_x: 10.0,
                max_y: 10.0,
            }
        );
        assert_eq!(evidence.grid_columns, 8);
        assert_eq!(evidence.grid_rows, 8);
        assert_eq!(evidence.target_resolution_x, 1.0);
        assert_eq!(evidence.target_resolution_y, 1.0);
        assert_eq!(
            evidence.source_earlier_resolution,
            RasterResolution { x: 1.0, y: 1.0 }
        );
        assert_eq!(
            evidence.source_later_resolution,
            RasterResolution { x: 1.0, y: 1.0 }
        );
        assert_eq!(evidence.overlap_ratio_basis_points, 10_000);
        assert_eq!(evidence.resampling_method, "nearest");
        assert_eq!(evidence.transform.origin_x, 2.0);
        assert_eq!(evidence.transform.origin_y, 10.0);
        assert_eq!(evidence.earlier_raster_ref, "product:scene-001:ndvi");
        assert_eq!(evidence.later_raster_ref, "product:scene-002:ndvi");
        assert_eq!(
            evidence.earlier_source_ref,
            "source:field:alpha:ndvi_raster:2026-06-10T10:00:00Z"
        );
        assert_eq!(
            evidence.later_source_ref,
            "source:field:alpha:ndvi_raster:2026-06-12T10:00:00Z"
        );
        assert_eq!(
            evidence.aligned_earlier_ref,
            "alignment:field-alpha:ndvi:2026-06-10:2026-06-12:earlier"
        );
        assert_eq!(
            evidence.aligned_later_ref,
            "alignment:field-alpha:ndvi:2026-06-10:2026-06-12:later"
        );
    }

    #[test]
    fn raster_alignment_refuses_insufficient_overlap() {
        let earlier = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-10T10:00:00Z",
            "product:scene-001:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 10.0,
            },
        );
        let later = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-12T10:00:00Z",
            "product:scene-002:ndvi",
            GeoExtent {
                min_x: 9.5,
                min_y: 9.5,
                max_x: 12.0,
                max_y: 12.0,
            },
        );

        let error = align_raster_pair(
            &earlier,
            &later,
            alignment_config(0.25, 0.25, 0.50),
            "alignment:field-alpha:insufficient".to_string(),
        )
        .expect_err("insufficient overlap should refuse alignment");

        assert_eq!(
            error,
            TimeSeriesError::InsufficientOverlap {
                reason_code: AlignmentRefusalReason::InsufficientOverlap,
                observed_overlap_basis_points: 400,
                minimum_overlap_basis_points: 5000
            }
        );
    }

    #[test]
    fn raster_alignment_refuses_missing_resolution() {
        let earlier = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-10T10:00:00Z",
            "product:scene-001:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 10.0,
            },
        );
        let mut later = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-12T10:00:00Z",
            "product:scene-002:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 10.0,
            },
        );
        if let SeriesValue::Raster(value) = &mut later.value {
            value.resolution = None;
        }

        let error = align_raster_pair(
            &earlier,
            &later,
            alignment_config(1.0, 1.0, 0.50),
            "alignment:field-alpha:missing-resolution".to_string(),
        )
        .expect_err("missing resolution should refuse alignment");

        assert_eq!(error, TimeSeriesError::MissingRasterResolution);
    }

    #[test]
    fn alignment_guard_passes_coregisterable_pair_with_proof_ref() {
        let earlier = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-10T10:00:00Z",
            "product:scene-001:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 10.0,
            },
        );
        let later = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-12T10:00:00Z",
            "product:scene-002:ndvi",
            GeoExtent {
                min_x: 2.0,
                min_y: 2.0,
                max_x: 10.0,
                max_y: 10.0,
            },
        );

        let proof = guard_coregisterable_pair(
            &earlier,
            &later,
            guard_config(0.75, 0.0),
            "alignment-proof:field-alpha:ndvi".to_string(),
        )
        .expect("compatible pair should pass guard");

        assert_eq!(
            proof.alignment_proof_ref,
            "alignment-proof:field-alpha:ndvi"
        );
        assert_eq!(proof.target_crs, "EPSG:32610");
        assert_eq!(proof.overlap_ratio_basis_points, 10_000);
        assert_eq!(
            proof.earlier_resolution,
            RasterResolution { x: 1.0, y: 1.0 }
        );
        assert_eq!(proof.later_resolution, RasterResolution { x: 1.0, y: 1.0 });
    }

    #[test]
    fn alignment_guard_refuses_crs_mismatch_with_api_shape() {
        let earlier = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-10T10:00:00Z",
            "product:scene-001:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 10.0,
            },
        );
        let mut later = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-12T10:00:00Z",
            "product:scene-002:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 10.0,
            },
        );
        if let SeriesValue::Raster(value) = &mut later.value {
            value.crs = Some("EPSG:4326".to_string());
        }

        let refusal = guard_coregisterable_pair(
            &earlier,
            &later,
            guard_config(0.75, 0.0),
            "alignment-proof:field-alpha:ndvi".to_string(),
        )
        .expect_err("CRS mismatch should refuse guard");

        assert_eq!(refusal.reason_code, AlignmentRefusalReason::CrsMismatch);
        assert!(refusal.mismatch_detail.contains("EPSG:32610"));
        assert!(refusal.mismatch_detail.contains("EPSG:4326"));
        assert!(refusal.change_job_blocked);
        assert!(refusal.alignment_proof_ref.is_none());
    }

    #[test]
    fn alignment_guard_refuses_insufficient_overlap_with_detail() {
        let earlier = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-10T10:00:00Z",
            "product:scene-001:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 10.0,
            },
        );
        let later = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-12T10:00:00Z",
            "product:scene-002:ndvi",
            GeoExtent {
                min_x: 9.5,
                min_y: 9.5,
                max_x: 12.0,
                max_y: 12.0,
            },
        );

        let refusal = guard_coregisterable_pair(
            &earlier,
            &later,
            guard_config(0.50, 0.0),
            "alignment-proof:field-alpha:ndvi".to_string(),
        )
        .expect_err("insufficient overlap should refuse guard");

        assert_eq!(
            refusal.reason_code,
            AlignmentRefusalReason::InsufficientOverlap
        );
        assert!(refusal.mismatch_detail.contains("400bp"));
        assert!(refusal.mismatch_detail.contains("5000bp"));
        assert!(refusal.change_job_blocked);
    }

    #[test]
    fn alignment_guard_refuses_resolution_mismatch() {
        let earlier = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-10T10:00:00Z",
            "product:scene-001:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 10.0,
            },
        );
        let mut later = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-12T10:00:00Z",
            "product:scene-002:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 10.0,
            },
        );
        if let SeriesValue::Raster(value) = &mut later.value {
            value.resolution = Some(RasterResolution { x: 2.0, y: 2.0 });
        }

        let refusal = guard_coregisterable_pair(
            &earlier,
            &later,
            guard_config(0.75, 0.01),
            "alignment-proof:field-alpha:ndvi".to_string(),
        )
        .expect_err("resolution mismatch should refuse guard");

        assert_eq!(
            refusal.reason_code,
            AlignmentRefusalReason::ResolutionMismatch
        );
        assert!(refusal.mismatch_detail.contains("1"));
        assert!(refusal.mismatch_detail.contains("2"));
        assert!(refusal.change_job_blocked);
    }

    #[test]
    fn raster_change_computes_delta_and_threshold_mask_on_aligned_grid() {
        let (evidence, proof) = aligned_pair_evidence_and_proof();
        let earlier_grid = aligned_grid(
            &evidence,
            &evidence.aligned_earlier_ref,
            [0.25, 0.50, 0.75, 1.00],
        );
        let later_grid = aligned_grid(
            &evidence,
            &evidence.aligned_later_ref,
            [0.00, 1.00, 0.875, 0.50],
        );

        let change = compute_aligned_raster_change(
            &proof,
            &evidence,
            &earlier_grid,
            &later_grid,
            change_config(0.25),
            "change:field-alpha:delta".to_string(),
            "change:field-alpha:mask".to_string(),
        )
        .expect("aligned rasters should produce change outputs");

        assert_eq!(change.delta_raster_ref, "change:field-alpha:delta");
        assert_eq!(change.mask_raster_ref, "change:field-alpha:mask");
        assert_eq!(change.alignment_ref, evidence.alignment_ref);
        assert_eq!(change.crs, evidence.target_crs);
        assert_eq!(change.extent, evidence.aligned_extent);
        assert_eq!(change.resolution, RasterResolution { x: 1.0, y: 1.0 });
        assert_eq!(change.grid_columns, 2);
        assert_eq!(change.grid_rows, 2);
        assert_eq!(change.absolute_threshold, 0.25);
        assert_eq!(
            change.delta_values,
            vec![Some(-0.25), Some(0.50), Some(0.125), Some(-0.50)]
        );
        assert_eq!(change.change_mask, vec![true, true, false, true]);
        assert_eq!(change.changed_cell_count, 3);
    }

    #[test]
    fn raster_change_identical_scenes_emit_empty_mask() {
        let (evidence, proof) = aligned_pair_evidence_and_proof();
        let earlier_grid = aligned_grid(
            &evidence,
            &evidence.aligned_earlier_ref,
            [0.25, 0.50, 0.75, 1.00],
        );
        let later_grid = aligned_grid(
            &evidence,
            &evidence.aligned_later_ref,
            [0.25, 0.50, 0.75, 1.00],
        );

        let change = compute_aligned_raster_change(
            &proof,
            &evidence,
            &earlier_grid,
            &later_grid,
            change_config(0.01),
            "change:field-alpha:delta".to_string(),
            "change:field-alpha:mask".to_string(),
        )
        .expect("identical aligned rasters should still produce outputs");

        assert_eq!(
            change.delta_values,
            vec![Some(0.0), Some(0.0), Some(0.0), Some(0.0)]
        );
        assert_eq!(change.change_mask, vec![false, false, false, false]);
        assert_eq!(change.changed_cell_count, 0);
    }

    #[test]
    fn raster_change_is_refused_before_delta_when_guard_refuses_pair() {
        let earlier = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-10T10:00:00Z",
            "product:scene-001:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 2.0,
                max_y: 2.0,
            },
        );
        let mut later = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-12T10:00:00Z",
            "product:scene-002:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 2.0,
                max_y: 2.0,
            },
        );
        if let SeriesValue::Raster(value) = &mut later.value {
            value.crs = Some("EPSG:4326".to_string());
        }

        let refusal = guard_coregisterable_pair(
            &earlier,
            &later,
            guard_config(0.75, 0.0),
            "alignment-proof:field-alpha:ndvi".to_string(),
        )
        .expect_err("guard should refuse before change computation");

        assert_eq!(refusal.reason_code, AlignmentRefusalReason::CrsMismatch);
        assert!(refusal.change_job_blocked);
        assert!(refusal.alignment_proof_ref.is_none());
    }

    #[test]
    fn series_csv_export_carries_entity_metric_time_value_and_empty_header() {
        let export = export_series_csv(&[
            scalar_point("field:alpha", "ndvi_mean", "2026-06-10T10:00:00Z", 0.68),
            scalar_point("field:alpha", "ndvi_mean", "2026-06-12T10:00:00Z", 0.72),
        ])
        .expect("series CSV should export");

        assert_eq!(export.content_type, "text/csv");
        assert!(export
            .csv
            .starts_with("entity_ref,metric,t,unit,value,source_ref,created_at\n"));
        assert!(export.csv.contains(
            "field:alpha,ndvi_mean,2026-06-10T10:00:00Z,index,0.68,source:field:alpha:ndvi_mean:2026-06-10T10:00:00Z,2026-06-12T12:00:00Z"
        ));

        let empty = export_series_csv(&[]).expect("empty series CSV should still export");
        assert_eq!(
            empty.csv,
            "entity_ref,metric,t,unit,value,source_ref,created_at\n"
        );
    }

    #[test]
    fn change_mask_geotiff_export_preserves_aligned_grid_metadata() {
        let change = sample_drop_change_result();

        let export = export_change_mask_geotiff(&change).expect("change mask should export");

        assert_eq!(export.content_type, "image/tiff");
        assert_eq!(export.metadata.mask_raster_ref, "change:field-alpha:mask");
        assert_eq!(export.metadata.crs, "EPSG:32610");
        assert_eq!(export.metadata.extent, change.extent);
        assert_eq!(export.metadata.resolution, change.resolution);
        assert_eq!(export.metadata.grid_columns, change.grid_columns);
        assert_eq!(export.metadata.grid_rows, change.grid_rows);
        assert!(export
            .bytes
            .starts_with(b"AGBOT_TIMESERIES_GEOTIFF_METADATA\n"));
    }

    #[test]
    fn change_zone_geojson_export_preserves_crs_properties_and_empty_result() {
        let event = sample_change_event();
        let export = export_change_zones_geojson(
            vec![ChangeZoneExportFeature {
                event: event.clone(),
                geometry: sample_zone_polygon("EPSG:4326"),
            }],
            "EPSG:4326".to_string(),
        )
        .expect("change zones should export");

        assert_eq!(export.content_type, "application/geo+json");
        assert_eq!(export.feature_collection.geojson_type, "FeatureCollection");
        assert_eq!(export.feature_collection.crs, "EPSG:4326");
        assert_eq!(export.feature_collection.features.len(), 1);
        let feature = &export.feature_collection.features[0];
        assert_eq!(feature.geojson_type, "Feature");
        assert_eq!(feature.geometry.geojson_type, "Polygon");
        assert_eq!(feature.properties.zone_ref, "zone-ne");
        assert_eq!(feature.properties.magnitude, event.magnitude);
        assert_eq!(feature.properties.direction, ChangeEventDirection::Dropped);
        assert_eq!(
            feature.properties.reason_code,
            ChangeEventReasonCode::BaselineDrop
        );

        let empty = export_change_zones_geojson(Vec::new(), "EPSG:4326".to_string())
            .expect("empty change-zone export should be valid");
        assert_eq!(empty.feature_collection.features.len(), 0);
        assert_eq!(empty.feature_collection.crs, "EPSG:4326");
    }

    #[test]
    fn compare_view_feed_locks_aligned_pair_and_change_mask_to_shared_view() {
        let (evidence, proof) = aligned_pair_evidence_and_proof();
        let change = sample_drop_change_result();

        let feed =
            build_compare_view_feed(&proof, &evidence, &change).expect("compare feed should build");

        assert_eq!(feed.schema_version, "timeseries.compare_view_feed.v1");
        assert_eq!(feed.entity_ref, "field:alpha");
        assert_eq!(feed.metric, "ndvi_raster");
        assert_eq!(feed.alignment_ref, evidence.alignment_ref);
        assert_eq!(feed.alignment_proof_ref, proof.alignment_proof_ref);
        assert_eq!(feed.shared_view.crs, "EPSG:32610");
        assert_eq!(feed.shared_view.extent, evidence.aligned_extent);
        assert_eq!(feed.earlier.raster_ref, evidence.aligned_earlier_ref);
        assert_eq!(feed.later.raster_ref, evidence.aligned_later_ref);
        assert_eq!(feed.change_mask.mask_raster_ref, change.mask_raster_ref);
        assert_eq!(feed.change_mask.change_mask, change.change_mask);
        assert_eq!(feed.alignment_proof.target_crs, feed.shared_view.crs);
    }

    #[test]
    fn compare_view_feed_refuses_mismatched_change_grid_without_panes() {
        let (evidence, proof) = aligned_pair_evidence_and_proof();
        let mut change = sample_drop_change_result();
        change.crs = "EPSG:4326".to_string();

        let refusal = build_compare_view_feed(&proof, &evidence, &change)
            .expect_err("mismatched change grid should not build compare feed");

        assert_eq!(
            refusal.reason_code,
            AlignmentRefusalReason::InvalidGuardConfig
        );
        assert!(refusal.mismatch_detail.contains("crs"));
        assert_eq!(
            refusal.earlier_raster_ref,
            Some(evidence.earlier_raster_ref)
        );
        assert!(refusal.no_misaligned_panes);
    }

    #[test]
    fn compare_view_refusal_passes_uncoregistered_pair_mismatch_to_viewer() {
        let earlier = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-10T10:00:00Z",
            "product:scene-001:ndvi",
            default_extent(),
        );
        let mut later = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-12T10:00:00Z",
            "product:scene-002:ndvi",
            default_extent(),
        );
        if let SeriesValue::Raster(value) = &mut later.value {
            value.crs = Some("EPSG:4326".to_string());
        }

        let guard_refusal = guard_coregisterable_pair(
            &earlier,
            &later,
            guard_config(0.75, 0.0),
            "alignment-proof:field-alpha:ndvi".to_string(),
        )
        .expect_err("CRS mismatch should refuse guard");
        let viewer_refusal = compare_view_refusal_from_guard(guard_refusal);

        assert_eq!(
            viewer_refusal.reason_code,
            AlignmentRefusalReason::CrsMismatch
        );
        assert!(viewer_refusal.mismatch_detail.contains("EPSG:32610"));
        assert!(viewer_refusal.mismatch_detail.contains("EPSG:4326"));
        assert!(viewer_refusal.no_misaligned_panes);
    }

    fn scalar_point(entity_ref: &str, metric: &str, t: &str, value: f64) -> SeriesPoint {
        scalar_point_with_unit(entity_ref, metric, "index", t, value)
    }

    fn scalar_point_with_unit(
        entity_ref: &str,
        metric: &str,
        unit: &str,
        t: &str,
        value: f64,
    ) -> SeriesPoint {
        SeriesPoint {
            entity_ref: entity_ref.to_string(),
            metric: metric.to_string(),
            unit: unit.to_string(),
            t: t.to_string(),
            value: SeriesValue::Scalar { value },
            source_ref: format!("source:{entity_ref}:{metric}:{t}"),
            created_at: "2026-06-12T12:00:00Z".to_string(),
        }
    }

    fn raster_point(
        entity_ref: &str,
        metric: &str,
        t: &str,
        raster_ref: &str,
        extent: GeoExtent,
    ) -> SeriesPoint {
        SeriesPoint {
            entity_ref: entity_ref.to_string(),
            metric: metric.to_string(),
            unit: "index".to_string(),
            t: t.to_string(),
            value: SeriesValue::Raster(RasterSeriesValue {
                raster_ref: raster_ref.to_string(),
                crs: Some("EPSG:32610".to_string()),
                extent: Some(extent),
                resolution: Some(RasterResolution { x: 1.0, y: 1.0 }),
            }),
            source_ref: format!("source:{entity_ref}:{metric}:{t}"),
            created_at: "2026-06-12T12:00:00Z".to_string(),
        }
    }

    fn metric_definition(metric: &str, unit: &str, kind: MetricKind) -> MetricDefinition {
        MetricDefinition {
            metric: metric.to_string(),
            unit: unit.to_string(),
            kind,
            expected_cadence: "per_flight".to_string(),
        }
    }

    fn seeded_baseline_engine() -> TimeSeriesEngine {
        let mut engine = TimeSeriesEngine::default();
        engine
            .register_metric(metric_definition("ndvi_mean", "index", MetricKind::Scalar))
            .expect("metric should register");
        for (date, value) in [
            ("2024-06-14T10:00:00Z", 0.64),
            ("2025-06-14T10:00:00Z", 0.66),
            ("2026-06-10T10:00:00Z", 0.70),
            ("2026-06-12T10:00:00Z", 0.72),
            ("2026-06-14T10:00:00Z", 0.50),
        ] {
            engine
                .append(scalar_point_with_unit(
                    "field:alpha",
                    "ndvi_mean",
                    "index",
                    date,
                    value,
                ))
                .expect("baseline point should append");
        }
        engine
    }

    fn trend_target_2026() -> ZonalTrendTarget {
        ZonalTrendTarget {
            entity_ref: "field:alpha".to_string(),
            metric: "ndvi_mean".to_string(),
            zone_ref: "zone:NE".to_string(),
            zone_crs: "EPSG:32610".to_string(),
            range: TimeRange {
                start: Some("2026-01-01T00:00:00Z".to_string()),
                end: None,
            },
        }
    }

    fn sample_drop_change_result() -> RasterChangeResult {
        let (evidence, proof) = aligned_pair_evidence_and_proof();
        let earlier = aligned_grid(&evidence, &evidence.aligned_earlier_ref, [0.70; 4]);
        let later = aligned_grid(
            &evidence,
            &evidence.aligned_later_ref,
            [0.45, 0.48, 0.70, 0.70],
        );
        compute_aligned_raster_change(
            &proof,
            &evidence,
            &earlier,
            &later,
            change_config(0.10),
            "change:field-alpha:delta".to_string(),
            "change:field-alpha:mask".to_string(),
        )
        .expect("sample drop should produce change result")
    }

    fn sample_change_event() -> ChangeEvent {
        ChangeEvent {
            zone_ref: "zone-ne".to_string(),
            metric: "ndvi_mean".to_string(),
            magnitude: 0.18,
            direction: ChangeEventDirection::Dropped,
            since_date: "2026-06-01T10:00:00Z".to_string(),
            reason_code: ChangeEventReasonCode::BaselineDrop,
            changed_cell_count: 3,
            severity_score: 0.54,
            evidence_refs: vec![
                "alignment:alpha:ndvi".to_string(),
                "change:field-alpha:mask".to_string(),
            ],
            summary: "ndvi_mean dropped 0.18 in zone-ne since 2026-06-01T10:00:00Z".to_string(),
        }
    }

    fn sample_zone_polygon(crs: &str) -> ChangeZonePolygon {
        ChangeZonePolygon {
            crs: crs.to_string(),
            rings: vec![vec![
                [-121.50, 38.50],
                [-121.40, 38.50],
                [-121.40, 38.60],
                [-121.50, 38.60],
                [-121.50, 38.50],
            ]],
        }
    }

    fn sample_product_ingest(
        source_ref: &str,
        raster_ref: &str,
        product_date: &str,
    ) -> SeriesProductIngest {
        SeriesProductIngest {
            entity_ref: "field:alpha".to_string(),
            metric: "ndvi_raster".to_string(),
            unit: "index".to_string(),
            source_ref: source_ref.to_string(),
            product_ref: raster_ref.to_string(),
            product_date: product_date.to_string(),
            finalized_at: "2026-06-12T12:00:00Z".to_string(),
            value: SeriesValue::Raster(RasterSeriesValue {
                raster_ref: raster_ref.to_string(),
                crs: Some("EPSG:32610".to_string()),
                extent: Some(default_extent()),
                resolution: Some(RasterResolution { x: 1.0, y: 1.0 }),
            }),
        }
    }

    fn default_extent() -> GeoExtent {
        GeoExtent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 2.0,
            max_y: 2.0,
        }
    }

    fn alignment_config(
        target_resolution_x: f64,
        target_resolution_y: f64,
        minimum_overlap_ratio: f64,
    ) -> RasterAlignmentConfig {
        RasterAlignmentConfig {
            target_resolution_x,
            target_resolution_y,
            minimum_overlap_ratio,
            resampling_method: " nearest ".to_string(),
        }
    }

    fn guard_config(minimum_overlap_ratio: f64, resolution_tolerance: f64) -> AlignmentGuardConfig {
        AlignmentGuardConfig {
            minimum_overlap_ratio,
            resolution_tolerance,
        }
    }

    fn aligned_pair_evidence_and_proof() -> (RasterAlignmentEvidence, AlignmentGuardProof) {
        let earlier = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-10T10:00:00Z",
            "product:scene-001:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 2.0,
                max_y: 2.0,
            },
        );
        let later = raster_point(
            "field:alpha",
            "ndvi_raster",
            "2026-06-12T10:00:00Z",
            "product:scene-002:ndvi",
            GeoExtent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 2.0,
                max_y: 2.0,
            },
        );
        let evidence = align_raster_pair(
            &earlier,
            &later,
            alignment_config(1.0, 1.0, 1.0),
            "alignment:field-alpha:ndvi".to_string(),
        )
        .expect("aligned pair should produce evidence");
        let proof = guard_coregisterable_pair(
            &earlier,
            &later,
            guard_config(1.0, 0.0),
            "alignment-proof:field-alpha:ndvi".to_string(),
        )
        .expect("aligned pair should pass guard");
        (evidence, proof)
    }

    fn aligned_grid(
        evidence: &RasterAlignmentEvidence,
        raster_ref: &str,
        values: [f64; 4],
    ) -> AlignedRasterGrid {
        AlignedRasterGrid {
            raster_ref: raster_ref.to_string(),
            alignment_ref: evidence.alignment_ref.clone(),
            crs: evidence.target_crs.clone(),
            extent: evidence.aligned_extent,
            resolution: RasterResolution { x: 1.0, y: 1.0 },
            grid_columns: evidence.grid_columns,
            grid_rows: evidence.grid_rows,
            values: values.into_iter().map(Some).collect(),
        }
    }

    fn change_config(absolute_threshold: f64) -> RasterChangeConfig {
        RasterChangeConfig {
            absolute_threshold,
            method_version: "delta-mask-v1".to_string(),
        }
    }
}
