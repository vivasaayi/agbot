use crate::product_anomalies::ProductAnomaly;
use crate::zonal_statistics::ProductGrid;
use serde::{Deserialize, Serialize};
use shared::schemas::{assert_raster_spatial_ref, RasterSpatialRefError};
use std::collections::{BTreeSet, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnomalyZonePolygon {
    pub coordinates: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnomalyZone {
    pub zone_id: String,
    pub cell_indices: Vec<usize>,
    pub polygon: AnomalyZonePolygon,
    pub area_m2: f32,
    pub centroid: (f64, f64),
    pub crs: String,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ZoneDelineationError {
    #[error("product grid dimensions do not match values/mask lengths: expected {expected}, values {values}, mask {mask}")]
    DimensionMismatch {
        expected: usize,
        values: usize,
        mask: usize,
    },
    #[error("product grid spatial metadata is invalid: {reason}")]
    SpatialRef { reason: RasterSpatialRefError },
    #[error("anomaly index {index} is outside product grid with {total_pixel_count} pixels")]
    AnomalyOutOfBounds {
        index: usize,
        total_pixel_count: usize,
    },
}

pub fn delineate_anomaly_zones(
    grid: &ProductGrid,
    anomalies: &[ProductAnomaly],
) -> Result<Vec<AnomalyZone>, ZoneDelineationError> {
    let expected = grid.width as usize * grid.height as usize;
    if grid.values.len() != expected || grid.nodata_mask.len() != expected {
        return Err(ZoneDelineationError::DimensionMismatch {
            expected,
            values: grid.values.len(),
            mask: grid.nodata_mask.len(),
        });
    }
    let spatial_ref = assert_raster_spatial_ref(Some(&grid.spatial_ref), grid.width, grid.height)
        .map_err(|reason| ZoneDelineationError::SpatialRef { reason })?;
    let transform = spatial_ref
        .geo_transform
        .expect("asserted spatial ref always has transform");
    let resolution = spatial_ref
        .resolution
        .expect("asserted spatial ref always has resolution");
    let crs = spatial_ref
        .crs
        .expect("asserted spatial ref always has CRS");

    let mut flagged = BTreeSet::new();
    for anomaly in anomalies {
        if anomaly.index >= expected {
            return Err(ZoneDelineationError::AnomalyOutOfBounds {
                index: anomaly.index,
                total_pixel_count: expected,
            });
        }
        flagged.insert(anomaly.index);
    }

    let flagged_lookup = flagged.iter().copied().collect::<HashSet<_>>();
    let mut visited = HashSet::new();
    let mut zones = Vec::new();

    for start in flagged {
        if visited.contains(&start) {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        let mut component = Vec::new();
        visited.insert(start);

        while let Some(index) = queue.pop_front() {
            component.push(index);
            for neighbor in neighbors(index, grid.width, grid.height) {
                if flagged_lookup.contains(&neighbor) && visited.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }

        component.sort_unstable();
        let zone_number = zones.len() + 1;
        zones.push(zone_from_component(
            zone_number,
            grid.width,
            &transform,
            resolution.x * resolution.y,
            &crs,
            component,
        ));
    }

    Ok(zones)
}

fn neighbors(index: usize, width: u32, height: u32) -> Vec<usize> {
    let width = width as usize;
    let height = height as usize;
    let row = index / width;
    let col = index % width;
    let mut neighbors = Vec::with_capacity(4);
    if col > 0 {
        neighbors.push(index - 1);
    }
    if col + 1 < width {
        neighbors.push(index + 1);
    }
    if row > 0 {
        neighbors.push(index - width);
    }
    if row + 1 < height {
        neighbors.push(index + width);
    }
    neighbors
}

fn zone_from_component(
    zone_number: usize,
    width: u32,
    transform: &[f64; 6],
    pixel_area: f64,
    crs: &str,
    cell_indices: Vec<usize>,
) -> AnomalyZone {
    let width = width as usize;
    let mut min_row = usize::MAX;
    let mut max_row = 0;
    let mut min_col = usize::MAX;
    let mut max_col = 0;
    let mut centroid_x = 0.0;
    let mut centroid_y = 0.0;

    for index in &cell_indices {
        let row = index / width;
        let col = index % width;
        min_row = min_row.min(row);
        max_row = max_row.max(row);
        min_col = min_col.min(col);
        max_col = max_col.max(col);
        let center = transform_point(transform, col as f64 + 0.5, row as f64 + 0.5);
        centroid_x += center.0;
        centroid_y += center.1;
    }

    let count = cell_indices.len() as f64;
    let top_left = transform_point(transform, min_col as f64, min_row as f64);
    let top_right = transform_point(transform, (max_col + 1) as f64, min_row as f64);
    let bottom_right = transform_point(transform, (max_col + 1) as f64, (max_row + 1) as f64);
    let bottom_left = transform_point(transform, min_col as f64, (max_row + 1) as f64);

    AnomalyZone {
        zone_id: format!("zone-{zone_number}"),
        cell_indices,
        polygon: AnomalyZonePolygon {
            coordinates: vec![top_left, top_right, bottom_right, bottom_left, top_left],
        },
        area_m2: (count * pixel_area) as f32,
        centroid: (centroid_x / count, centroid_y / count),
        crs: crs.to_string(),
    }
}

fn transform_point(transform: &[f64; 6], col: f64, row: f64) -> (f64, f64) {
    (
        transform[0] + col * transform[1] + row * transform[2],
        transform[3] + col * transform[4] + row * transform[5],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product_anomalies::{ProductAnomaly, ProductAnomalyReasonCode};
    use crate::zonal_statistics::ProductGrid;
    use shared::schemas::{GeoBounds, RasterResolution, RasterSpatialRef};

    #[test]
    fn adjacent_flagged_cells_are_grouped_into_one_zone() {
        let grid = product_grid(3, 2);
        let anomalies = vec![anomaly(0), anomaly(1)];

        let zones = delineate_anomaly_zones(&grid, &anomalies).expect("zones compute");

        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0].zone_id, "zone-1");
        assert_eq!(zones[0].cell_indices, vec![0, 1]);
        assert_eq!(zones[0].area_m2, 200.0);
        assert_eq!(zones[0].centroid, (500010.0, 4500015.0));
        assert_eq!(zones[0].crs, "EPSG:32614");
        assert_eq!(
            zones[0].polygon.coordinates,
            vec![
                (500000.0, 4500020.0),
                (500020.0, 4500020.0),
                (500020.0, 4500010.0),
                (500000.0, 4500010.0),
                (500000.0, 4500020.0)
            ]
        );
    }

    #[test]
    fn separated_patches_emit_two_zones() {
        let grid = product_grid(3, 2);
        let anomalies = vec![anomaly(0), anomaly(5)];

        let zones = delineate_anomaly_zones(&grid, &anomalies).expect("zones compute");

        assert_eq!(zones.len(), 2);
        assert_eq!(zones[0].cell_indices, vec![0]);
        assert_eq!(zones[1].cell_indices, vec![5]);
        assert_eq!(zones[0].area_m2, 100.0);
        assert_eq!(zones[1].area_m2, 100.0);
    }

    #[test]
    fn single_cell_zone_has_closed_polygon_area_and_centroid() {
        let grid = product_grid(3, 2);
        let anomalies = vec![anomaly(4)];

        let zones = delineate_anomaly_zones(&grid, &anomalies).expect("zones compute");

        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0].cell_indices, vec![4]);
        assert_eq!(zones[0].area_m2, 100.0);
        assert_eq!(zones[0].centroid, (500015.0, 4500005.0));
        assert_eq!(
            zones[0].polygon.coordinates,
            vec![
                (500010.0, 4500010.0),
                (500020.0, 4500010.0),
                (500020.0, 4500000.0),
                (500010.0, 4500000.0),
                (500010.0, 4500010.0)
            ]
        );
    }

    fn anomaly(index: usize) -> ProductAnomaly {
        ProductAnomaly {
            index,
            row: 0,
            col: 0,
            value: 0.1,
            threshold: 0.2,
            reason_code: ProductAnomalyReasonCode::BelowAbsoluteThreshold,
        }
    }

    fn product_grid(width: u32, height: u32) -> ProductGrid {
        ProductGrid {
            width,
            height,
            values: vec![0.5; (width * height) as usize],
            nodata_mask: vec![false; (width * height) as usize],
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
