use crate::product_anomalies::{
    flag_product_anomalies, AnomalyDetectionConfig, AnomalyDetectionError, ProductAnomaly,
    ProductAnomalyReasonCode,
};
use crate::zonal_statistics::{compute_zonal_statistics, ProductGrid};
use crate::{delineate_anomaly_zones, AnomalyZone, ZonalStatisticsError, ZoneDelineationError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ThermalSpotRequest {
    pub product_ref: String,
    pub product_kind: String,
    pub acquired_at: DateTime<Utc>,
    pub grid: ProductGrid,
    pub low_threshold_c: Option<f32>,
    pub high_threshold_c: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThermalSpotSummary {
    pub product_ref: String,
    pub acquired_at: DateTime<Utc>,
    pub spots: Vec<ThermalSpot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThermalSpot {
    pub spot_type: ThermalSpotType,
    pub zone: AnomalyZone,
    pub mean_temperature_c: f32,
    pub threshold_c: f32,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThermalSpotType {
    Hotspot,
    Coldspot,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ThermalSpotError {
    #[error("{field} is required")]
    MissingField { field: &'static str },
    #[error("thermal product is unavailable for product kind {product_kind}")]
    ThermalProductUnavailable { product_kind: String },
    #[error("at least one thermal threshold is required")]
    MissingThreshold,
    #[error("thermal threshold must be finite")]
    InvalidThreshold,
    #[error("thermal statistics failed: {0}")]
    Statistics(#[from] ZonalStatisticsError),
    #[error("thermal anomaly detection failed: {0}")]
    Anomaly(#[from] AnomalyDetectionError),
    #[error("thermal zone delineation failed: {0}")]
    Zone(#[from] ZoneDelineationError),
}

pub fn detect_thermal_spots(
    request: ThermalSpotRequest,
) -> Result<ThermalSpotSummary, ThermalSpotError> {
    require_text(&request.product_ref, "product_ref")?;
    require_text(&request.product_kind, "product_kind")?;
    if !request
        .product_kind
        .to_ascii_lowercase()
        .contains("thermal")
    {
        return Err(ThermalSpotError::ThermalProductUnavailable {
            product_kind: request.product_kind,
        });
    }
    if request.low_threshold_c.is_none() && request.high_threshold_c.is_none() {
        return Err(ThermalSpotError::MissingThreshold);
    }
    if request
        .low_threshold_c
        .into_iter()
        .chain(request.high_threshold_c)
        .any(|threshold| !threshold.is_finite())
    {
        return Err(ThermalSpotError::InvalidThreshold);
    }

    let stats = compute_zonal_statistics(&request.grid)?;
    let anomalies = flag_product_anomalies(
        &request.grid,
        &stats,
        &AnomalyDetectionConfig {
            low_threshold: request.low_threshold_c,
            high_threshold: request.high_threshold_c,
            std_dev_multiplier: None,
        },
    )?;

    let mut spots = Vec::new();
    spots.extend(spots_for_type(
        &request.grid,
        &anomalies,
        ThermalSpotType::Hotspot,
        request.high_threshold_c,
        stats.statistics.max_value,
    )?);
    spots.extend(spots_for_type(
        &request.grid,
        &anomalies,
        ThermalSpotType::Coldspot,
        request.low_threshold_c,
        stats.statistics.min_value,
    )?);
    spots.sort_by(|left, right| {
        spot_rank(left.spot_type)
            .cmp(&spot_rank(right.spot_type))
            .then_with(|| left.zone.zone_id.cmp(&right.zone.zone_id))
    });

    Ok(ThermalSpotSummary {
        product_ref: request.product_ref,
        acquired_at: request.acquired_at,
        spots,
    })
}

fn require_text(value: &str, field: &'static str) -> Result<(), ThermalSpotError> {
    if value.trim().is_empty() {
        Err(ThermalSpotError::MissingField { field })
    } else {
        Ok(())
    }
}

fn spots_for_type(
    grid: &ProductGrid,
    anomalies: &[ProductAnomaly],
    spot_type: ThermalSpotType,
    threshold: Option<f32>,
    extreme_value: f32,
) -> Result<Vec<ThermalSpot>, ThermalSpotError> {
    let Some(threshold) = threshold else {
        return Ok(Vec::new());
    };
    let filtered: Vec<_> = anomalies
        .iter()
        .filter(|anomaly| anomaly_matches_type(anomaly.reason_code, spot_type))
        .cloned()
        .collect();
    let zones = delineate_anomaly_zones(grid, &filtered)?;

    Ok(zones
        .into_iter()
        .map(|zone| {
            let mean_temperature_c = mean_zone_temperature(grid, &zone);
            ThermalSpot {
                confidence: confidence_from_margin(
                    spot_type,
                    mean_temperature_c,
                    threshold,
                    extreme_value,
                ),
                spot_type,
                zone,
                mean_temperature_c,
                threshold_c: threshold,
            }
        })
        .collect())
}

fn anomaly_matches_type(reason: ProductAnomalyReasonCode, spot_type: ThermalSpotType) -> bool {
    matches!(
        (reason, spot_type),
        (
            ProductAnomalyReasonCode::AboveAbsoluteThreshold
                | ProductAnomalyReasonCode::AboveStatisticalBand,
            ThermalSpotType::Hotspot
        ) | (
            ProductAnomalyReasonCode::BelowAbsoluteThreshold
                | ProductAnomalyReasonCode::BelowStatisticalBand,
            ThermalSpotType::Coldspot
        )
    )
}

fn mean_zone_temperature(grid: &ProductGrid, zone: &AnomalyZone) -> f32 {
    let sum = zone
        .cell_indices
        .iter()
        .map(|index| grid.values[*index])
        .sum::<f32>();
    sum / zone.cell_indices.len() as f32
}

fn confidence_from_margin(
    spot_type: ThermalSpotType,
    mean_temperature: f32,
    threshold: f32,
    extreme_value: f32,
) -> f32 {
    let (margin, max_margin) = match spot_type {
        ThermalSpotType::Hotspot => (mean_temperature - threshold, extreme_value - threshold),
        ThermalSpotType::Coldspot => (threshold - mean_temperature, threshold - extreme_value),
    };
    if max_margin <= 0.0 {
        0.0
    } else {
        (margin / max_margin).clamp(0.0, 1.0)
    }
}

fn spot_rank(spot_type: ThermalSpotType) -> u8 {
    match spot_type {
        ThermalSpotType::Hotspot => 0,
        ThermalSpotType::Coldspot => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::{GeoBounds, RasterResolution, RasterSpatialRef};

    #[test]
    fn thermal_spots_carry_area_mean_temperature_and_confidence() {
        let summary = detect_thermal_spots(ThermalSpotRequest {
            product_ref: "thermal-2026-05-15".to_string(),
            product_kind: "thermal".to_string(),
            acquired_at: acquired_at(),
            grid: grid(vec![20.0, 31.0, 32.0, 10.0, 9.0, 20.0], 3, 2),
            low_threshold_c: Some(12.0),
            high_threshold_c: Some(30.0),
        })
        .expect("thermal spot detection should run");

        assert_eq!(summary.product_ref, "thermal-2026-05-15");
        assert_eq!(summary.spots.len(), 2);
        let hot = &summary.spots[0];
        assert_eq!(hot.spot_type, ThermalSpotType::Hotspot);
        assert_eq!(hot.zone.area_m2, 200.0);
        assert_eq!(hot.zone.cell_indices, vec![1, 2]);
        assert_eq!(hot.mean_temperature_c, 31.5);
        assert_eq!(hot.threshold_c, 30.0);
        assert!((hot.confidence - 0.75).abs() < 1.0e-6);

        let cold = &summary.spots[1];
        assert_eq!(cold.spot_type, ThermalSpotType::Coldspot);
        assert_eq!(cold.zone.area_m2, 200.0);
        assert_eq!(cold.zone.cell_indices, vec![3, 4]);
        assert_eq!(cold.mean_temperature_c, 9.5);
        assert_eq!(cold.threshold_c, 12.0);
        assert!((cold.confidence - (2.5 / 3.0)).abs() < 1.0e-6);
    }

    #[test]
    fn thermal_spots_are_cleanly_unavailable_without_thermal_product() {
        let error = detect_thermal_spots(ThermalSpotRequest {
            product_ref: "ndvi-2026-05-15".to_string(),
            product_kind: "ndvi".to_string(),
            acquired_at: acquired_at(),
            grid: grid(vec![0.2, 0.3, 0.4, 0.5], 2, 2),
            low_threshold_c: Some(12.0),
            high_threshold_c: Some(30.0),
        })
        .expect_err("non-thermal products should be unavailable");

        assert_eq!(
            error,
            ThermalSpotError::ThermalProductUnavailable {
                product_kind: "ndvi".to_string()
            }
        );
    }

    fn acquired_at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-05-15T00:00:00Z")
            .expect("valid time")
            .with_timezone(&Utc)
    }

    fn grid(values: Vec<f32>, width: u32, height: u32) -> ProductGrid {
        ProductGrid {
            width,
            height,
            nodata_mask: vec![false; values.len()],
            values,
            spatial_ref: RasterSpatialRef {
                georeferenced: true,
                crs: Some("EPSG:32614".to_string()),
                bbox: Some(GeoBounds {
                    min_lon: 500000.0,
                    min_lat: 4500000.0,
                    max_lon: 500000.0 + width as f64 * 10.0,
                    max_lat: 4500000.0 + height as f64 * 10.0,
                }),
                geo_transform: Some([
                    500000.0,
                    10.0,
                    0.0,
                    4500000.0 + height as f64 * 10.0,
                    0.0,
                    -10.0,
                ]),
                resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
            },
        }
    }
}
