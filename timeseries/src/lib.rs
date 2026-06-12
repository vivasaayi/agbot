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
    pub t: String,
    pub value: SeriesValue,
    pub source_ref: String,
    pub created_at: String,
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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimeSeriesStore {
    points: BTreeMap<SeriesKey, SeriesPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimeSeriesError {
    #[error("entity_ref cannot be empty")]
    EmptyEntityRef,
    #[error("metric cannot be empty")]
    EmptyMetric,
    #[error("timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("source_ref cannot be empty")]
    EmptySourceRef,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    pub fn append(&mut self, point: SeriesPoint) -> Result<(), TimeSeriesError> {
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
        align_raster_pair, compute_aligned_raster_change, guard_coregisterable_pair,
        AlignedRasterGrid, AlignmentGuardConfig, AlignmentGuardProof, AlignmentRefusalReason,
        GeoExtent, RasterAlignmentConfig, RasterAlignmentEvidence, RasterChangeConfig,
        RasterResolution, RasterSeriesValue, SeriesPoint, SeriesQuery, SeriesValue, TimeRange,
        TimeSeriesEngine, TimeSeriesError, TimeSeriesStore,
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
            .append(scalar_point(
                "field:alpha",
                "ndvi_mean",
                "2026-06-10T10:00:00Z",
                0.68,
            ))
            .expect("first point should append");
        engine
            .append(scalar_point(
                "field:alpha",
                "ndvi_mean",
                "2026-06-12T10:00:00Z",
                0.72,
            ))
            .expect("second point should append");
        engine
            .append(scalar_point(
                "field:alpha",
                "soil_moisture",
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

    fn scalar_point(entity_ref: &str, metric: &str, t: &str, value: f64) -> SeriesPoint {
        SeriesPoint {
            entity_ref: entity_ref.to_string(),
            metric: metric.to_string(),
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
