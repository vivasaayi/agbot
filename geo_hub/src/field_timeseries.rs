//! Per-field satellite time-series extraction (batch S-3) and the
//! multi-source query/harmonization layer over it (batch S-4).
//!
//! Reduces a registered L2 index raster (single-band GeoTIFF with a nodata
//! sentinel for masked pixels) to five zonal statistics over its field scope
//! — mean, median, p10, p90, valid_fraction — and appends them to
//! `time_series_points` under the canonical `shared::timeseries_naming`
//! spellings (`field:{id}` / `sat.{index}.{stat}` / `product:{id}`).
//!
//! The append is idempotent: `time_series_points` has a
//! `(entity_ref, metric, t, source_ref)` primary key and this module writes
//! with `INSERT OR IGNORE`, so re-extracting the same product skips instead
//! of duplicating. The satellite derive paths call
//! [`extract_and_append_field_stats`] after L2 registration when the request
//! carries a field scope, logging and continuing on failure so a stats
//! problem never fails the derivation itself.

use std::collections::BTreeMap;

use serde::Serialize;
use shared::timeseries_naming::{
    field_entity_ref, product_source_ref, satellite_metric, ZonalStat, SOURCE_HLS, SOURCE_LANDSAT,
    SOURCE_MODIS, SOURCE_SENTINEL2,
};
use sqlx::Row;
use thiserror::Error;
use timeseries::{
    MetricDefinition, MetricKind, RollingBaselineConfig, SeasonalComparisonConfig,
    SeasonalComparisonTarget, SeriesPoint as EngineSeriesPoint, SeriesValue as EngineSeriesValue,
    TimeRange, TimeSeriesEngine, TimeSeriesError, ZonalTrendTarget,
};

use crate::catalog;
use crate::db::DbPool;
use shared::product_graph::ProductLevel;

/// Reason-coded failure of one extraction attempt. `NoFieldScope` is the
/// "nothing to do" case — callers walking many products treat it as a skip,
/// not a failure.
#[derive(Debug, Error)]
pub enum FieldTimeseriesError {
    #[error("product {0} not found in the catalog")]
    ProductNotFound(String),
    #[error(
        "product {product_id} is level {level}; only l2 index rasters feed the field time series"
    )]
    NotLevel2 { product_id: String, level: String },
    #[error("product {0} has no field scope; zonal stats have no entity to attach to")]
    NoFieldScope(String),
    #[error("product {0} has no raster artifact path")]
    NoArtifact(String),
    #[error("product {0} has no temporal_start; observations need a timestamp")]
    NoTemporalStart(String),
    #[error("product {0} raster has no valid pixels; value statistics are undefined")]
    NoValidPixels(String),
    #[error("failed to read raster {path}: {source}")]
    Raster {
        path: String,
        #[source]
        source: raster_io::RasterIoError,
    },
    #[error(transparent)]
    Catalog(#[from] crate::catalog::CatalogError),
    #[error("time-series engine rejected the series: {0}")]
    Engine(#[from] TimeSeriesError),
    #[error("time-series persistence failed: {0}")]
    Db(#[from] sqlx::Error),
}

/// What one extraction appended, plus the computed statistics (keyed by
/// [`ZonalStat::as_str`]) for callers that want to surface them.
#[derive(Debug, Clone)]
pub struct ExtractOutcome {
    pub product_id: String,
    pub points_appended: usize,
    pub points_skipped: usize,
    pub stats: BTreeMap<String, f64>,
}

/// Map a catalog `source_id` to a time-series source family constant.
///
/// `source_id` is the most reliable discriminator the derive paths persist
/// on the L2 row itself: `earth-search:sentinel-2-l2a`
/// (satellite_derivation), `sen2cor:l2a` (sen2cor_derive),
/// `usgs:{dataset}` with landsat dataset names (landsat_derive, inherited
/// from its L1 band products), and `hls-v2.0` (hls). Anything else maps to
/// `"unknown"`.
pub fn source_family(source_id: Option<&str>) -> &'static str {
    let Some(id) = source_id else {
        return "unknown";
    };
    let id = id.to_ascii_lowercase();
    if id.contains("sentinel-2") || id.contains("sentinel2") || id.starts_with("sen2cor") {
        SOURCE_SENTINEL2
    } else if id.starts_with("hls") {
        SOURCE_HLS
    } else if id.contains("landsat") {
        SOURCE_LANDSAT
    } else if id.contains("modis") {
        SOURCE_MODIS
    } else {
        "unknown"
    }
}

/// Percentile of pre-sorted values by linear interpolation between closest
/// ranks (the numpy/R-7 default): rank = q * (n - 1), interpolating between
/// the floor and ceil neighbors. Chosen over nearest-rank so small fields
/// (few valid pixels) do not quantize p10/p90 onto single pixels.
pub(crate) fn percentile_sorted(sorted: &[f64], q: f64) -> f64 {
    debug_assert!(!sorted.is_empty());
    let rank = q * (sorted.len() - 1) as f64;
    let low = rank.floor() as usize;
    let high = rank.ceil() as usize;
    if low == high {
        return sorted[low];
    }
    let frac = rank - low as f64;
    sorted[low] + frac * (sorted[high] - sorted[low])
}

/// Zonal statistics over the valid pixels of one raster band, in
/// [`ZonalStat::ALL`] order. A pixel is valid when it is finite and not the
/// nodata sentinel. Returns `None` when no pixel is valid
/// (mean/median/percentiles are undefined).
pub(crate) fn zonal_stats(values: &[f32], nodata: Option<f32>) -> Option<Vec<(ZonalStat, f64)>> {
    let mut valid: Vec<f64> = values
        .iter()
        .filter(|v| v.is_finite() && Some(**v) != nodata)
        .map(|v| f64::from(*v))
        .collect();
    if valid.is_empty() {
        return None;
    }
    valid.sort_by(|a, b| a.partial_cmp(b).expect("valid values are finite"));
    let mean = valid.iter().sum::<f64>() / valid.len() as f64;
    Some(vec![
        (ZonalStat::Mean, mean),
        (ZonalStat::Median, percentile_sorted(&valid, 0.5)),
        (ZonalStat::P10, percentile_sorted(&valid, 0.1)),
        (ZonalStat::P90, percentile_sorted(&valid, 0.9)),
        (
            ZonalStat::ValidFraction,
            valid.len() as f64 / values.len() as f64,
        ),
    ])
}

/// Extract the five zonal statistics from a field-scoped L2 index product
/// and append them to `time_series_points` (one row per stat, idempotent via
/// `INSERT OR IGNORE` on the `(entity_ref, metric, t, source_ref)` key).
///
/// - entity_ref: `field:{field_id}` from the product's field scope;
/// - metric: `sat.{product.kind}.{stat}` (the L2 kind is the index key,
///   e.g. `ndvi`);
/// - t: the product's `temporal_start` (scene acquisition time);
/// - source_ref: `product:{product_id}`;
/// - metadata: `{"source", "scene_id", "level"}` for downstream filtering.
pub async fn extract_and_append_field_stats(
    pool: &DbPool,
    product_id: &str,
) -> Result<ExtractOutcome, FieldTimeseriesError> {
    let product = catalog::get_product(pool, product_id)
        .await?
        .ok_or_else(|| FieldTimeseriesError::ProductNotFound(product_id.to_string()))?;
    if product.level != ProductLevel::L2 {
        return Err(FieldTimeseriesError::NotLevel2 {
            product_id: product.product_id,
            level: product.level.as_str().to_string(),
        });
    }
    let field_id = product
        .field_id
        .as_deref()
        .ok_or_else(|| FieldTimeseriesError::NoFieldScope(product.product_id.clone()))?;
    let path = product
        .path
        .as_deref()
        .ok_or_else(|| FieldTimeseriesError::NoArtifact(product.product_id.clone()))?;
    let observed_at = product
        .temporal_start
        .as_deref()
        .ok_or_else(|| FieldTimeseriesError::NoTemporalStart(product.product_id.clone()))?;

    // Same local single-band read path as the raster application pipelines
    // (drought/composite): full-band read with the GeoTIFF nodata tag as the
    // invalid sentinel (NaN pixels are invalid too).
    let raster_error = |source| FieldTimeseriesError::Raster {
        path: path.to_string(),
        source,
    };
    let mut reader = raster_io::GeoTiffReader::open(path).map_err(raster_error)?;
    let nodata = reader.info().nodata.map(|n| n as f32);
    let values = reader.read_band().map_err(raster_error)?.to_f32();
    let stats = zonal_stats(&values, nodata)
        .ok_or_else(|| FieldTimeseriesError::NoValidPixels(product.product_id.clone()))?;

    let entity_ref = field_entity_ref(field_id);
    let source_ref = product_source_ref(&product.product_id);
    let metadata_json = serde_json::json!({
        "source": source_family(product.source_id.as_deref()),
        "scene_id": product.scene_id,
        "level": "l2",
    })
    .to_string();
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let mut points_appended = 0;
    let mut points_skipped = 0;
    let mut out_stats = BTreeMap::new();
    for (stat, value) in &stats {
        let metric = satellite_metric(&product.kind, *stat);
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO time_series_points (
                entity_ref, metric, t, value_kind, scalar_value, source_ref,
                created_at, metadata_json
            )
            VALUES (?1, ?2, ?3, 'scalar', ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&entity_ref)
        .bind(&metric)
        .bind(observed_at)
        .bind(value)
        .bind(&source_ref)
        .bind(&created_at)
        .bind(&metadata_json)
        .execute(pool)
        .await?;
        if result.rows_affected() > 0 {
            points_appended += 1;
        } else {
            points_skipped += 1;
        }
        out_stats.insert(stat.as_str().to_string(), *value);
    }

    Ok(ExtractOutcome {
        product_id: product.product_id,
        points_appended,
        points_skipped,
        stats: out_stats,
    })
}

/// Best-effort hook for the satellite derive paths: run the extraction for a
/// freshly registered, field-scoped L2 product and log-and-continue on
/// failure — a stats problem must never fail the derivation that produced
/// the raster.
pub async fn append_field_stats_best_effort(pool: &DbPool, product_id: &str) {
    match extract_and_append_field_stats(pool, product_id).await {
        Ok(outcome) => {
            tracing::debug!(
                product_id = %outcome.product_id,
                appended = outcome.points_appended,
                skipped = outcome.points_skipped,
                "field time-series stats extracted"
            );
        }
        Err(err) => {
            tracing::warn!(
                product_id = %product_id,
                error = %err,
                "field time-series extraction failed; derivation result is unaffected"
            );
        }
    }
}

// --- Query and harmonization layer (batch S-4) --------------------------------

/// Fixed caveat attached to every harmonization entry: the adjustment is a
/// per-pair statistical alignment, not a physical cross-calibration.
pub const HARMONIZATION_CAVEAT: &str =
    "bandpass/BRDF differences are not corrected; merged view is for visual continuity and coarse trends";

/// Maximum |Δt| for a source observation to pair with a reference
/// observation when fitting the harmonization mapping.
const PAIR_WINDOW_SECONDS: i64 = 3 * 24 * 60 * 60;

/// One observation of a field metric, tagged with its origin source family
/// and the L2 product it was extracted from.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SeriesPointOut {
    pub t: String,
    pub value: f64,
    pub source: String,
    pub product_ref: String,
}

/// How one non-reference source was mapped onto the reference series.
/// `method` is `"least_squares"` (>= 8 overlap pairs), `"offset_only"`
/// (3-7 pairs), or `"none"` (< 3 pairs; identity mapping).
#[derive(Debug, Clone, Serialize)]
pub struct HarmonizationEntry {
    pub source: String,
    pub method: String,
    pub gain: f64,
    pub offset: f64,
    pub pair_count: usize,
    pub caveat: String,
}

/// Response of the per-field multi-source time-series query: raw series per
/// source family, a harmonized merged series, and the mapping evidence.
#[derive(Debug, Clone, Serialize)]
pub struct FieldSeriesResponse {
    pub field_id: String,
    pub metric: String,
    pub per_source: BTreeMap<String, Vec<SeriesPointOut>>,
    pub merged: Vec<SeriesPointOut>,
    pub harmonization: Vec<HarmonizationEntry>,
}

/// Parse an RFC 3339 timestamp to epoch seconds; `None` keeps unparsable
/// observations out of the pair matching (they still appear in the series).
fn epoch_seconds(t: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(t)
        .ok()
        .map(|dt| dt.timestamp())
}

/// Reference source for harmonization: `hls` when present (it is already a
/// cross-sensor harmonized product), else `landsat` (the longest-calibrated
/// archive), else the source with the most observations (ties resolve to
/// the alphabetically first source via the BTreeMap iteration order).
fn reference_source(per_source: &BTreeMap<String, Vec<SeriesPointOut>>) -> Option<String> {
    for preferred in [SOURCE_HLS, SOURCE_LANDSAT] {
        if per_source.contains_key(preferred) {
            return Some(preferred.to_string());
        }
    }
    per_source
        .iter()
        .max_by_key(|(_, points)| points.len())
        .map(|(source, _)| source.clone())
}

/// Overlap pairs `(source_value, reference_value)`: walking the source
/// series in time order, each observation takes the nearest still-unused
/// reference observation within [`PAIR_WINDOW_SECONDS`]; each reference
/// observation is used at most once.
fn overlap_pairs(source: &[SeriesPointOut], reference: &[SeriesPointOut]) -> Vec<(f64, f64)> {
    let ref_times: Vec<Option<i64>> = reference.iter().map(|p| epoch_seconds(&p.t)).collect();
    let mut used = vec![false; reference.len()];
    let mut pairs = Vec::new();
    for point in source {
        let Some(t) = epoch_seconds(&point.t) else {
            continue;
        };
        let nearest = ref_times
            .iter()
            .enumerate()
            .filter_map(|(index, ref_t)| {
                let ref_t = (*ref_t)?;
                let distance = (ref_t - t).abs();
                (!used[index] && distance <= PAIR_WINDOW_SECONDS).then_some((index, distance))
            })
            .min_by_key(|(index, distance)| (*distance, *index));
        if let Some((index, _)) = nearest {
            used[index] = true;
            pairs.push((point.value, reference[index].value));
        }
    }
    pairs
}

/// Fit the source -> reference mapping from overlap pairs:
/// - >= 8 pairs: ordinary least squares `y = gain * x + offset`;
/// - 3-7 pairs: offset only (mean difference), gain 1;
/// - < 3 pairs: identity (`"none"`).
///
/// A degenerate least-squares design (all source values equal) falls back to
/// the offset-only mapping since the gain is unidentifiable.
fn fit_mapping(pairs: &[(f64, f64)]) -> (&'static str, f64, f64) {
    let n = pairs.len();
    if n < 3 {
        return ("none", 1.0, 0.0);
    }
    let mean_offset = pairs.iter().map(|(x, y)| y - x).sum::<f64>() / n as f64;
    if n < 8 {
        return ("offset_only", 1.0, mean_offset);
    }
    let mean_x = pairs.iter().map(|(x, _)| x).sum::<f64>() / n as f64;
    let mean_y = pairs.iter().map(|(_, y)| y).sum::<f64>() / n as f64;
    let var_x = pairs.iter().map(|(x, _)| (x - mean_x).powi(2)).sum::<f64>();
    if var_x <= 1e-12 {
        return ("offset_only", 1.0, mean_offset);
    }
    let cov = pairs
        .iter()
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum::<f64>();
    let gain = cov / var_x;
    ("least_squares", gain, mean_y - gain * mean_x)
}

/// Harmonize the per-source series onto the reference source and merge.
///
/// The per-source inputs stay untouched; the merged series holds the
/// reference points as-is plus each non-reference point mapped through its
/// fitted gain/offset, sorted by `(t, source)`, every point keeping its
/// origin source tag. One [`HarmonizationEntry`] is emitted per
/// non-reference source.
pub fn harmonize(
    per_source: &BTreeMap<String, Vec<SeriesPointOut>>,
) -> (Vec<SeriesPointOut>, Vec<HarmonizationEntry>) {
    let Some(reference) = reference_source(per_source) else {
        return (Vec::new(), Vec::new());
    };
    let reference_points = &per_source[&reference];
    let mut merged = reference_points.clone();
    let mut entries = Vec::new();
    for (source, points) in per_source {
        if *source == reference {
            continue;
        }
        let pairs = overlap_pairs(points, reference_points);
        let (method, gain, offset) = fit_mapping(&pairs);
        merged.extend(points.iter().map(|point| SeriesPointOut {
            value: gain * point.value + offset,
            ..point.clone()
        }));
        entries.push(HarmonizationEntry {
            source: source.clone(),
            method: method.to_string(),
            gain,
            offset,
            pair_count: pairs.len(),
            caveat: HARMONIZATION_CAVEAT.to_string(),
        });
    }
    merged.sort_by(|a, b| (&a.t, &a.source).cmp(&(&b.t, &b.source)));
    (merged, entries)
}

/// Query one field metric across all satellite sources, grouped by the
/// `"source"` tag the extraction stored in `metadata_json` (rows without a
/// tag group under `"unknown"`). `start`/`end` bound `t` inclusively
/// (RFC 3339 strings compare lexicographically). With a `source` filter the
/// merged series is that raw series and no harmonization is fitted;
/// otherwise the merged series is harmonized via [`harmonize`].
pub async fn query_field_series(
    pool: &DbPool,
    field_id: &str,
    metric: &str,
    start: Option<&str>,
    end: Option<&str>,
    source: Option<&str>,
) -> Result<FieldSeriesResponse, FieldTimeseriesError> {
    let rows = sqlx::query(
        r#"
        SELECT t, scalar_value, source_ref, metadata_json
        FROM time_series_points
        WHERE entity_ref = ?1 AND metric = ?2
          AND value_kind = 'scalar' AND scalar_value IS NOT NULL
          AND (?3 IS NULL OR t >= ?3)
          AND (?4 IS NULL OR t <= ?4)
        ORDER BY t, source_ref
        "#,
    )
    .bind(field_entity_ref(field_id))
    .bind(metric)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await?;

    let mut per_source: BTreeMap<String, Vec<SeriesPointOut>> = BTreeMap::new();
    for row in rows {
        let point_source = row
            .get::<Option<String>, _>("metadata_json")
            .as_deref()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
            .and_then(|metadata| metadata["source"].as_str().map(str::to_string))
            .unwrap_or_else(|| "unknown".to_string());
        if source.is_some_and(|wanted| wanted != point_source) {
            continue;
        }
        per_source
            .entry(point_source.clone())
            .or_default()
            .push(SeriesPointOut {
                t: row.get("t"),
                value: row.get("scalar_value"),
                source: point_source,
                product_ref: row.get("source_ref"),
            });
    }

    let (merged, harmonization) = if source.is_some() {
        // Single-source view: nothing to harmonize against.
        let merged = per_source.values().next().cloned().unwrap_or_default();
        (merged, Vec::new())
    } else {
        harmonize(&per_source)
    };

    Ok(FieldSeriesResponse {
        field_id: field_id.to_string(),
        metric: metric.to_string(),
        per_source,
        merged,
        harmonization,
    })
}

/// Distinct metrics recorded for a field, sorted; the discovery companion
/// to [`query_field_series`].
pub async fn list_field_metrics(
    pool: &DbPool,
    field_id: &str,
) -> Result<Vec<String>, FieldTimeseriesError> {
    let metrics = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT metric FROM time_series_points WHERE entity_ref = ?1 ORDER BY metric",
    )
    .bind(field_entity_ref(field_id))
    .fetch_all(pool)
    .await?;
    Ok(metrics)
}

// --- Summary layer (batch S-5) -------------------------------------------------

/// Rolling-baseline window: the anomaly verdict compares the latest
/// observation against the mean of the trailing five before it. Fixed in v1;
/// no query knob.
const ROLLING_WINDOW_POINTS: usize = 5;

/// Deviation (|latest - baseline_mean|) at or beyond which the latest
/// observation is flagged anomalous. Sized for the [-1, 1] vegetation-index
/// metrics this series carries. Fixed in v1.
const ROLLING_ANOMALY_BAND: f64 = 0.15;

/// Prior-years comparison: an observation from an earlier year counts as
/// "same season" when its day-of-year is within this many days of the latest
/// observation's. Fixed in v1.
const SEASONAL_DOY_TOLERANCE: u32 = 15;

/// At least one prior-year observation is required before a comparison is
/// reported; with none the block is omitted rather than fabricated.
const SEASONAL_MIN_POINTS: usize = 1;

/// `time_series_points` does not persist units, so the in-memory engine
/// series is registered with this placeholder unit.
const SUMMARY_UNIT: &str = "unitless";
const SUMMARY_CADENCE: &str = "per_scene";

/// Latest-vs-recent-history verdict from the engine's rolling baseline:
/// the latest observation against the mean of the trailing
/// [`ROLLING_WINDOW_POINTS`] observations before it.
#[derive(Debug, Clone, Serialize)]
pub struct AnomalySummary {
    pub latest_t: String,
    pub latest_value: f64,
    pub baseline_mean: f64,
    /// `latest_value - baseline_mean`.
    pub deviation: f64,
    /// `|deviation| >= anomaly_band`.
    pub is_anomalous: bool,
    pub baseline_points: usize,
    pub anomaly_band: f64,
}

/// Latest observation vs the same-day-of-year window of prior years, from
/// the engine's seasonal comparison.
#[derive(Debug, Clone, Serialize)]
pub struct PriorYearsComparison {
    pub current_t: String,
    pub current_value: f64,
    pub prior_point_count: usize,
    pub seasonal_mean: f64,
    /// `current_value - seasonal_mean`.
    pub delta_from_seasonal_mean: f64,
    pub day_of_year_tolerance: u32,
}

/// Per-season statistics. v1 simplification: a "season" is the calendar year
/// of `t` (multi-season climates and southern-hemisphere wrap-around are not
/// modeled).
#[derive(Debug, Clone, Serialize)]
pub struct YearStats {
    pub year: i32,
    pub count: usize,
    pub mean: f64,
    pub max: f64,
    /// Timestamp of the seasonal peak (first occurrence on ties).
    pub max_t: String,
}

/// Response of `GET /api/fields/:field_id/timeseries/summary`.
///
/// `series_basis` documents which series the statistics were computed over:
/// `"merged"` (the harmonized multi-source series, so multi-source history
/// reads as one line) or `"single_source"` (a `source` filter was given; raw
/// values of that source only).
#[derive(Debug, Clone, Serialize)]
pub struct FieldSeriesSummary {
    pub field_id: String,
    pub metric: String,
    pub series_basis: String,
    pub observation_count: usize,
    pub first_t: Option<String>,
    pub last_t: Option<String>,
    pub per_year: Vec<YearStats>,
    pub anomaly: Option<AnomalySummary>,
    pub vs_prior_years: Option<PriorYearsComparison>,
}

/// Group observations by calendar year of `t` (v1 season rule) and reduce to
/// [`YearStats`]. Observations whose `t` does not start with a parsable
/// 4-digit year are left out of the per-year stats (they still count toward
/// `observation_count`).
fn per_year_stats(points: &[SeriesPointOut]) -> Vec<YearStats> {
    let mut by_year: BTreeMap<i32, Vec<&SeriesPointOut>> = BTreeMap::new();
    for point in points {
        let Some(year) = point.t.get(0..4).and_then(|y| y.parse::<i32>().ok()) else {
            continue;
        };
        by_year.entry(year).or_default().push(point);
    }
    by_year
        .into_iter()
        .map(|(year, year_points)| {
            let mean = year_points.iter().map(|p| p.value).sum::<f64>() / year_points.len() as f64;
            let mut peak = year_points[0];
            for point in &year_points[1..] {
                if point.value > peak.value {
                    peak = point;
                }
            }
            YearStats {
                year,
                count: year_points.len(),
                mean,
                max: peak.value,
                max_t: peak.t.clone(),
            }
        })
        .collect()
}

/// Summarize one field metric: season (calendar-year) statistics computed in
/// plain code, plus the `timeseries` engine's rolling-baseline anomaly
/// verdict and same-DOY prior-years comparison for the latest observation.
///
/// Without a `source` filter the summary runs over the harmonized merged
/// series (`series_basis: "merged"`); with one, over that source's raw
/// series (`series_basis: "single_source"`).
///
/// The engine keys points by `(entity_ref, metric, t)`, so when two sources
/// observe the same timestamp in the merged series only the first (sorted by
/// source) feeds the engine computations; all observations still feed
/// `observation_count` and `per_year`.
///
/// `anomaly` is omitted when fewer than [`ROLLING_WINDOW_POINTS`] + 1
/// observations exist; `vs_prior_years` when no prior-year observation falls
/// within [`SEASONAL_DOY_TOLERANCE`] days of the latest observation's
/// day-of-year (or its timestamp has no parsable date).
pub async fn summarize_field_series(
    pool: &DbPool,
    field_id: &str,
    metric: &str,
    source: Option<&str>,
) -> Result<FieldSeriesSummary, FieldTimeseriesError> {
    let response = query_field_series(pool, field_id, metric, None, None, source).await?;
    let points = response.merged;
    let series_basis = if source.is_some() {
        "single_source"
    } else {
        "merged"
    };

    let mut summary = FieldSeriesSummary {
        field_id: field_id.to_string(),
        metric: metric.to_string(),
        series_basis: series_basis.to_string(),
        observation_count: points.len(),
        first_t: points.first().map(|p| p.t.clone()),
        last_t: points.last().map(|p| p.t.clone()),
        per_year: per_year_stats(&points),
        anomaly: None,
        vs_prior_years: None,
    };
    let Some(latest) = points.last() else {
        return Ok(summary);
    };

    let entity_ref = field_entity_ref(field_id);
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut engine = TimeSeriesEngine::default();
    engine.register_metric(MetricDefinition {
        metric: metric.to_string(),
        unit: SUMMARY_UNIT.to_string(),
        kind: MetricKind::Scalar,
        expected_cadence: SUMMARY_CADENCE.to_string(),
    })?;
    for point in &points {
        match engine.append(EngineSeriesPoint {
            entity_ref: entity_ref.clone(),
            metric: metric.to_string(),
            unit: SUMMARY_UNIT.to_string(),
            t: point.t.clone(),
            value: EngineSeriesValue::Scalar { value: point.value },
            source_ref: point.product_ref.clone(),
            created_at: created_at.clone(),
        }) {
            Ok(()) => {}
            // Two sources at the same timestamp: keep the first, skip the rest.
            Err(TimeSeriesError::DuplicateSeriesPoint { .. }) => {}
            Err(other) => return Err(other.into()),
        }
    }

    match engine.compute_rolling_baseline(
        ZonalTrendTarget {
            entity_ref: entity_ref.clone(),
            metric: metric.to_string(),
            // Field-level summary: the whole field is the "zone".
            zone_ref: entity_ref.clone(),
            zone_crs: "EPSG:4326".to_string(),
            range: TimeRange::default(),
        },
        RollingBaselineConfig {
            window_points: ROLLING_WINDOW_POINTS,
            anomaly_band: ROLLING_ANOMALY_BAND,
        },
    ) {
        Ok(result) => {
            summary.anomaly = Some(AnomalySummary {
                latest_t: result.latest_point.t.clone(),
                latest_value: result.latest_value,
                baseline_mean: result.baseline_mean,
                deviation: result.delta_from_baseline,
                is_anomalous: result.anomaly,
                baseline_points: result.baseline_window.len(),
                anomaly_band: ROLLING_ANOMALY_BAND,
            });
        }
        Err(TimeSeriesError::InsufficientBaselineHistory { .. }) => {}
        Err(other) => return Err(other.into()),
    }

    match engine.compute_seasonal_comparison(
        SeasonalComparisonTarget {
            entity_ref: entity_ref.clone(),
            metric: metric.to_string(),
            zone_ref: entity_ref,
            zone_crs: "EPSG:4326".to_string(),
            current_t: latest.t.clone(),
        },
        SeasonalComparisonConfig {
            min_seasonal_points: SEASONAL_MIN_POINTS,
            day_of_year_tolerance: SEASONAL_DOY_TOLERANCE,
        },
    ) {
        Ok(result) => {
            summary.vs_prior_years = Some(PriorYearsComparison {
                current_t: result.current_point.t.clone(),
                current_value: result.seasonal_mean + result.delta_from_seasonal_baseline,
                prior_point_count: result.seasonal_points.len(),
                seasonal_mean: result.seasonal_mean,
                delta_from_seasonal_mean: result.delta_from_seasonal_baseline,
                day_of_year_tolerance: SEASONAL_DOY_TOLERANCE,
            });
        }
        Err(TimeSeriesError::NoSeasonalBaseline { .. }) => {}
        // Non-date timestamps cannot anchor a day-of-year comparison: omit
        // the block instead of failing the whole summary.
        Err(TimeSeriesError::InvalidTrendTimestamp { .. }) => {}
        Err(other) => return Err(other.into()),
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_interpolates_linearly_between_ranks() {
        let sorted: Vec<f64> = (1..=14).map(f64::from).collect();
        // rank = q * 13: p10 -> 1.3 => 2 + 0.3, p50 -> 6.5, p90 -> 11.7.
        assert!((percentile_sorted(&sorted, 0.1) - 2.3).abs() < 1e-12);
        assert!((percentile_sorted(&sorted, 0.5) - 7.5).abs() < 1e-12);
        assert!((percentile_sorted(&sorted, 0.9) - 12.7).abs() < 1e-12);
        assert_eq!(percentile_sorted(&sorted, 0.0), 1.0);
        assert_eq!(percentile_sorted(&sorted, 1.0), 14.0);
        assert_eq!(percentile_sorted(&[42.0], 0.5), 42.0);
    }

    #[test]
    fn zonal_stats_ignore_nodata_and_nan() {
        let values = [1.0f32, 2.0, -9999.0, f32::NAN, 3.0, 4.0];
        let stats = zonal_stats(&values, Some(-9999.0)).expect("valid pixels present");
        let stat = |wanted: ZonalStat| {
            stats
                .iter()
                .find(|(stat, _)| *stat == wanted)
                .expect("stat present")
                .1
        };
        assert!((stat(ZonalStat::Mean) - 2.5).abs() < 1e-12);
        assert!((stat(ZonalStat::Median) - 2.5).abs() < 1e-12);
        assert!((stat(ZonalStat::ValidFraction) - 4.0 / 6.0).abs() < 1e-12);
        // All masked -> undefined.
        assert!(zonal_stats(&[f32::NAN, -9999.0], Some(-9999.0)).is_none());
    }

    fn point(t: &str, value: f64, source: &str) -> SeriesPointOut {
        SeriesPointOut {
            t: t.to_string(),
            value,
            source: source.to_string(),
            product_ref: format!("product:p-{source}-{t}"),
        }
    }

    #[test]
    fn reference_source_prefers_hls_then_landsat_then_largest() {
        let series = |source: &str, n: usize| {
            (
                source.to_string(),
                (0..n)
                    .map(|i| point(&format!("2026-01-{:02}T00:00:00Z", i + 1), 0.5, source))
                    .collect::<Vec<_>>(),
            )
        };
        let all: BTreeMap<_, _> = [
            series(SOURCE_HLS, 1),
            series(SOURCE_LANDSAT, 2),
            series(SOURCE_SENTINEL2, 9),
        ]
        .into_iter()
        .collect();
        assert_eq!(reference_source(&all).as_deref(), Some(SOURCE_HLS));

        let no_hls: BTreeMap<_, _> = [series(SOURCE_LANDSAT, 1), series(SOURCE_SENTINEL2, 9)]
            .into_iter()
            .collect();
        assert_eq!(reference_source(&no_hls).as_deref(), Some(SOURCE_LANDSAT));

        let neither: BTreeMap<_, _> = [series(SOURCE_MODIS, 3), series(SOURCE_SENTINEL2, 5)]
            .into_iter()
            .collect();
        assert_eq!(
            reference_source(&neither).as_deref(),
            Some(SOURCE_SENTINEL2),
            "largest series wins without hls/landsat"
        );
        assert_eq!(reference_source(&BTreeMap::new()), None);
    }

    #[test]
    fn overlap_pairs_take_nearest_reference_within_window_once() {
        let reference = vec![
            point("2026-01-10T00:00:00Z", 0.5, "hls"),
            point("2026-01-20T00:00:00Z", 0.6, "hls"),
        ];
        let source = vec![
            // 1 day from ref[0]: pairs with it.
            point("2026-01-09T00:00:00Z", 0.55, "sentinel2"),
            // 2 days from ref[0] (already used) and 8 days from ref[1]: no pair.
            point("2026-01-12T00:00:00Z", 0.57, "sentinel2"),
            // 3 days from ref[1]: inclusive window boundary pairs.
            point("2026-01-23T00:00:00Z", 0.65, "sentinel2"),
            // 4 days out: beyond the window.
            point("2026-01-27T00:00:00Z", 0.70, "sentinel2"),
        ];
        assert_eq!(
            overlap_pairs(&source, &reference),
            vec![(0.55, 0.5), (0.65, 0.6)]
        );
    }

    #[test]
    fn fit_mapping_ladder_matches_pair_count() {
        assert_eq!(fit_mapping(&[(0.1, 0.2), (0.2, 0.3)]), ("none", 1.0, 0.0));

        let (method, gain, offset) = fit_mapping(&[(0.1, 0.2), (0.2, 0.3), (0.3, 0.5)]);
        assert_eq!(method, "offset_only");
        assert_eq!(gain, 1.0);
        assert!((offset - 0.4 / 3.0).abs() < 1e-12);

        let pairs: Vec<(f64, f64)> = (0..8)
            .map(|i| {
                let x = 0.1 * f64::from(i);
                (x, 1.5 * x - 0.2)
            })
            .collect();
        let (method, gain, offset) = fit_mapping(&pairs);
        assert_eq!(method, "least_squares");
        assert!((gain - 1.5).abs() < 1e-12, "gain {gain}");
        assert!((offset + 0.2).abs() < 1e-12, "offset {offset}");

        // Degenerate design (constant x): gain is unidentifiable, fall back
        // to the mean offset.
        let flat: Vec<(f64, f64)> = (0..8).map(|i| (0.4, 0.5 + 0.01 * f64::from(i))).collect();
        let (method, gain, offset) = fit_mapping(&flat);
        assert_eq!(method, "offset_only");
        assert_eq!(gain, 1.0);
        assert!((offset - 0.135).abs() < 1e-12);
    }

    #[test]
    fn harmonize_single_source_is_identity() {
        let per_source: BTreeMap<_, _> = [(
            SOURCE_SENTINEL2.to_string(),
            vec![
                point("2026-01-01T00:00:00Z", 0.4, SOURCE_SENTINEL2),
                point("2026-01-05T00:00:00Z", 0.5, SOURCE_SENTINEL2),
            ],
        )]
        .into_iter()
        .collect();
        let (merged, entries) = harmonize(&per_source);
        assert_eq!(merged, per_source[SOURCE_SENTINEL2]);
        assert!(entries.is_empty(), "no non-reference sources");
    }

    #[test]
    fn source_family_maps_known_derive_source_ids() {
        assert_eq!(
            source_family(Some("earth-search:sentinel-2-l2a")),
            SOURCE_SENTINEL2
        );
        assert_eq!(source_family(Some("sen2cor:l2a")), SOURCE_SENTINEL2);
        assert_eq!(source_family(Some("usgs:landsat_ot_c2_l2")), SOURCE_LANDSAT);
        assert_eq!(source_family(Some("hls-v2.0")), SOURCE_HLS);
        assert_eq!(source_family(Some("nasa:modis_terra")), SOURCE_MODIS);
        assert_eq!(source_family(Some("drone-fleet")), "unknown");
        assert_eq!(source_family(None), "unknown");
    }
}
