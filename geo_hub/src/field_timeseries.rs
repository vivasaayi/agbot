//! Per-field satellite time-series extraction (batch S-3).
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

use shared::timeseries_naming::{
    field_entity_ref, product_source_ref, satellite_metric, ZonalStat, SOURCE_HLS, SOURCE_LANDSAT,
    SOURCE_MODIS, SOURCE_SENTINEL2,
};
use thiserror::Error;

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
fn percentile_sorted(sorted: &[f64], q: f64) -> f64 {
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
fn zonal_stats(values: &[f32], nodata: Option<f32>) -> Option<Vec<(ZonalStat, f64)>> {
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
