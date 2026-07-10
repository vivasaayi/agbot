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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeriesFreshnessState {
    Fresh,
    Stale,
    NoBaseline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeriesCadenceHealthConfig {
    pub expected_cadence_days: u32,
    pub stale_after_days: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeriesGap {
    pub from_t: String,
    pub to_t: String,
    pub observed_gap_days: u32,
    pub expected_cadence_days: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeriesCadenceHealth {
    pub entity_ref: String,
    pub metric: String,
    pub evaluated_at: String,
    pub last_seen: Option<String>,
    pub age_days: Option<u32>,
    pub expected_cadence_days: u32,
    pub stale_after_days: u32,
    pub state: SeriesFreshnessState,
    pub point_count: usize,
    pub gap_count: usize,
    pub gaps: Vec<SeriesGap>,
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
    #[error("cadence health config requires expected_cadence_days and stale_after_days greater than zero")]
    InvalidCadenceHealthConfig,
    #[error("timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("source_ref cannot be empty")]
    EmptySourceRef,
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
        self.start.as_deref().is_none_or(|start| t >= start)
            && self.end.as_deref().is_none_or(|end| t <= end)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimeSeriesEngine {
    store: TimeSeriesStore,
    metric_registry: MetricRegistry,
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

pub fn evaluate_series_cadence_health(
    points: &[SeriesPoint],
    entity_ref: String,
    metric: String,
    evaluated_at: String,
    config: SeriesCadenceHealthConfig,
) -> Result<SeriesCadenceHealth, TimeSeriesError> {
    let entity_ref = normalize_required_text(entity_ref, TimeSeriesError::EmptyEntityRef)?;
    let metric = normalize_required_text(metric, TimeSeriesError::EmptyMetric)?;
    let evaluated_at = normalize_required_text(evaluated_at, TimeSeriesError::EmptyTimestamp)?;
    if config.expected_cadence_days == 0 || config.stale_after_days == 0 {
        return Err(TimeSeriesError::InvalidCadenceHealthConfig);
    }
    let evaluated_day = timestamp_day_index(&evaluated_at)?;
    let mut scoped = points
        .iter()
        .filter(|point| point.entity_ref == entity_ref && point.metric == metric)
        .cloned()
        .collect::<Vec<_>>();
    scoped.sort_by(|left, right| left.t.cmp(&right.t));

    if scoped.is_empty() {
        return Ok(SeriesCadenceHealth {
            entity_ref,
            metric,
            evaluated_at,
            last_seen: None,
            age_days: None,
            expected_cadence_days: config.expected_cadence_days,
            stale_after_days: config.stale_after_days,
            state: SeriesFreshnessState::NoBaseline,
            point_count: 0,
            gap_count: 0,
            gaps: Vec::new(),
        });
    }

    let mut gaps = Vec::new();
    for pair in scoped.windows(2) {
        let from = &pair[0];
        let to = &pair[1];
        let observed_gap_days =
            (timestamp_day_index(&to.t)? - timestamp_day_index(&from.t)?).max(0) as u32;
        if observed_gap_days > config.expected_cadence_days {
            gaps.push(SeriesGap {
                from_t: from.t.clone(),
                to_t: to.t.clone(),
                observed_gap_days,
                expected_cadence_days: config.expected_cadence_days,
            });
        }
    }

    let last_seen = scoped
        .last()
        .expect("non-empty scoped series checked above")
        .t
        .clone();
    let age_days = (evaluated_day - timestamp_day_index(&last_seen)?).max(0) as u32;
    let state = if age_days > config.stale_after_days {
        SeriesFreshnessState::Stale
    } else {
        SeriesFreshnessState::Fresh
    };

    Ok(SeriesCadenceHealth {
        entity_ref,
        metric,
        evaluated_at,
        last_seen: Some(last_seen),
        age_days: Some(age_days),
        expected_cadence_days: config.expected_cadence_days,
        stale_after_days: config.stale_after_days,
        state,
        point_count: scoped.len(),
        gap_count: gaps.len(),
        gaps,
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
        evaluate_series_cadence_health, GeoExtent, MetricDefinition, MetricKind, RasterResolution,
        RasterSeriesValue, RollingBaselineConfig, SeasonalComparisonConfig,
        SeasonalComparisonTarget, SeriesCadenceHealthConfig, SeriesFreshnessState, SeriesPoint,
        SeriesQuery, SeriesValue, TimeRange, TimeSeriesEngine, TimeSeriesError, TimeSeriesStore,
        TrendDirection, ZonalTrendConfig, ZonalTrendTarget,
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
    fn series_cadence_health_reports_freshness_and_gaps() {
        let points = vec![
            scalar_point("field:alpha", "ndvi_mean", "2026-06-01T10:00:00Z", 0.62),
            scalar_point("field:alpha", "ndvi_mean", "2026-06-03T10:00:00Z", 0.58),
            scalar_point("field:alpha", "ndvi_mean", "2026-06-04T10:00:00Z", 0.57),
        ];

        let health = evaluate_series_cadence_health(
            &points,
            "field:alpha".to_string(),
            "ndvi_mean".to_string(),
            "2026-06-05T10:00:00Z".to_string(),
            cadence_config(),
        )
        .expect("cadence health should evaluate");

        assert_eq!(health.state, SeriesFreshnessState::Fresh);
        assert_eq!(health.last_seen.as_deref(), Some("2026-06-04T10:00:00Z"));
        assert_eq!(health.age_days, Some(1));
        assert_eq!(health.point_count, 3);
        assert_eq!(health.gap_count, 1);
        assert_eq!(health.gaps[0].from_t, "2026-06-01T10:00:00Z");
        assert_eq!(health.gaps[0].to_t, "2026-06-03T10:00:00Z");
        assert_eq!(health.gaps[0].observed_gap_days, 2);
    }

    #[test]
    fn series_cadence_health_marks_stale_and_no_baseline() {
        let points = vec![scalar_point(
            "field:alpha",
            "ndvi_mean",
            "2026-06-01T10:00:00Z",
            0.62,
        )];
        let stale = evaluate_series_cadence_health(
            &points,
            "field:alpha".to_string(),
            "ndvi_mean".to_string(),
            "2026-06-05T10:00:00Z".to_string(),
            cadence_config(),
        )
        .expect("stale cadence health should evaluate");
        assert_eq!(stale.state, SeriesFreshnessState::Stale);
        assert_eq!(stale.age_days, Some(4));

        let empty = evaluate_series_cadence_health(
            &points,
            "field:alpha".to_string(),
            "soil_moisture".to_string(),
            "2026-06-05T10:00:00Z".to_string(),
            cadence_config(),
        )
        .expect("empty cadence health should evaluate");
        assert_eq!(empty.state, SeriesFreshnessState::NoBaseline);
        assert_eq!(empty.last_seen, None);
        assert_eq!(empty.point_count, 0);
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

    fn scalar_point(entity_ref: &str, metric: &str, t: &str, value: f64) -> SeriesPoint {
        scalar_point_with_unit(entity_ref, metric, "index", t, value)
    }

    fn cadence_config() -> SeriesCadenceHealthConfig {
        SeriesCadenceHealthConfig {
            expected_cadence_days: 1,
            stale_after_days: 2,
        }
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
}
