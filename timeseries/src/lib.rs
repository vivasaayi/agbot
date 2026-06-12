use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RasterSeriesValue {
    pub raster_ref: String,
    pub crs: Option<String>,
    pub extent: Option<GeoExtent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoExtent {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
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
    #[error("duplicate time-series point for {entity_ref}/{metric} at {t}")]
    DuplicateSeriesPoint {
        entity_ref: String,
        metric: String,
        t: String,
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
    })
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
        GeoExtent, RasterSeriesValue, SeriesPoint, SeriesQuery, SeriesValue, TimeRange,
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
}
