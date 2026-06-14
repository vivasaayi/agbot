use crate::error::{AppError, AppResult};
use shared::schemas::GeoPoint;
use std::path::Path;

const SHAPEFILE_HEADER_BYTES: usize = 100;
const ESRI_FILE_CODE: i32 = 9994;
const SHAPE_TYPE_NULL: i32 = 0;
const SHAPE_TYPE_POINT: i32 = 1;
const SHAPE_TYPE_POLYLINE: i32 = 3;
const SHAPE_TYPE_POLYGON: i32 = 5;
const SHAPE_TYPE_MULTIPOINT: i32 = 8;
const SHAPE_TYPE_POINT_Z: i32 = 11;
const SHAPE_TYPE_POLYLINE_Z: i32 = 13;
const SHAPE_TYPE_POLYGON_Z: i32 = 15;
const SHAPE_TYPE_MULTIPOINT_Z: i32 = 18;
const SHAPE_TYPE_POINT_M: i32 = 21;
const SHAPE_TYPE_POLYLINE_M: i32 = 23;
const SHAPE_TYPE_POLYGON_M: i32 = 25;
const SHAPE_TYPE_MULTIPOINT_M: i32 = 28;
const SHAPE_TYPE_MULTIPATCH: i32 = 31;

#[derive(Debug, Clone, PartialEq)]
pub struct PolygonShapeRecord {
    pub record_index: usize,
    pub coordinates: Vec<GeoPoint>,
}

pub fn parse_polygon_records(path: &Path, bytes: &[u8]) -> AppResult<Vec<PolygonShapeRecord>> {
    if bytes.len() < SHAPEFILE_HEADER_BYTES {
        return Err(AppError::BadRequest(format!(
            "shapefile {} is truncated",
            path.display()
        )));
    }

    let file_code = read_be_i32(bytes, 0)?;
    if file_code != ESRI_FILE_CODE {
        return Err(AppError::BadRequest(format!(
            "file {} is not a valid ESRI shapefile",
            path.display()
        )));
    }

    let shape_type = read_le_i32(bytes, 32)?;
    validate_polygon_shape_type(shape_type, path)?;

    let mut offset = SHAPEFILE_HEADER_BYTES;
    let mut records = Vec::new();
    while offset < bytes.len() {
        if bytes.len().saturating_sub(offset) < 8 {
            return Err(AppError::BadRequest(format!(
                "shapefile {} has a truncated record header",
                path.display()
            )));
        }

        let content_length_words = read_be_i32(bytes, offset + 4)?;
        if content_length_words < 0 {
            return Err(AppError::BadRequest(format!(
                "shapefile {} has a negative record length",
                path.display()
            )));
        }
        let content_length_bytes = content_length_words as usize * 2;
        offset += 8;

        if bytes.len().saturating_sub(offset) < content_length_bytes {
            return Err(AppError::BadRequest(format!(
                "shapefile {} has a truncated record body",
                path.display()
            )));
        }

        let record = &bytes[offset..offset + content_length_bytes];
        offset += content_length_bytes;

        let record_shape_type = read_le_i32(record, 0)?;
        if record_shape_type == SHAPE_TYPE_NULL {
            continue;
        }
        validate_polygon_shape_type(record_shape_type, path)?;

        records.push(parse_polygon_record(path, records.len(), record)?);
    }

    if records.is_empty() {
        return Err(AppError::BadRequest(format!(
            "shapefile {} does not contain any polygon records",
            path.display()
        )));
    }

    Ok(records)
}

fn parse_polygon_record(
    path: &Path,
    record_index: usize,
    record: &[u8],
) -> AppResult<PolygonShapeRecord> {
    if record.len() < 44 {
        return Err(AppError::BadRequest(format!(
            "polygon record {} in {} is truncated",
            record_index + 1,
            path.display()
        )));
    }

    let num_parts = read_le_i32(record, 36)?;
    let num_points = read_le_i32(record, 40)?;
    if num_parts != 1 {
        return Err(AppError::BadRequest(format!(
            "shapefile {} contains multipart polygons; only single-ring field boundaries are supported",
            path.display()
        )));
    }
    if num_points < 3 {
        return Err(AppError::BadRequest(format!(
            "polygon record {} in {} has fewer than three points",
            record_index + 1,
            path.display()
        )));
    }

    let parts_offset = 44;
    let points_offset = parts_offset + num_parts as usize * 4;
    let points_len = num_points as usize * 16;
    if record.len().saturating_sub(points_offset) < points_len {
        return Err(AppError::BadRequest(format!(
            "polygon record {} in {} is truncated",
            record_index + 1,
            path.display()
        )));
    }

    let first_part_index = read_le_i32(record, parts_offset)?;
    if first_part_index != 0 {
        return Err(AppError::BadRequest(format!(
            "shapefile {} contains an unsupported polygon part layout",
            path.display()
        )));
    }

    let mut coordinates = Vec::with_capacity(num_points as usize);
    for point_index in 0..num_points as usize {
        let point_offset = points_offset + point_index * 16;
        let longitude = read_le_f64(record, point_offset)?;
        let latitude = read_le_f64(record, point_offset + 8)?;
        if !is_valid_lon_lat(longitude, latitude) {
            return Err(AppError::BadRequest(format!(
                "shapefile {} must use geographic lon/lat coordinates in EPSG:4326",
                path.display()
            )));
        }
        coordinates.push(GeoPoint {
            longitude,
            latitude,
        });
    }

    if coordinates.first() == coordinates.last() {
        coordinates.pop();
    }
    if coordinates.len() < 3 {
        return Err(AppError::BadRequest(format!(
            "polygon record {} in {} collapses to fewer than three unique points",
            record_index + 1,
            path.display()
        )));
    }

    Ok(PolygonShapeRecord {
        record_index,
        coordinates,
    })
}

fn validate_polygon_shape_type(shape_type: i32, path: &Path) -> AppResult<()> {
    match shape_type {
        SHAPE_TYPE_POLYGON | SHAPE_TYPE_POLYGON_M | SHAPE_TYPE_POLYGON_Z => Ok(()),
        SHAPE_TYPE_NULL => Ok(()),
        SHAPE_TYPE_POINT | SHAPE_TYPE_POINT_M | SHAPE_TYPE_POINT_Z => Err(AppError::BadRequest(
            format!(
                "shapefile {} contains point geometry; only polygon field boundaries are supported",
                path.display()
            ),
        )),
        SHAPE_TYPE_POLYLINE | SHAPE_TYPE_POLYLINE_M | SHAPE_TYPE_POLYLINE_Z => Err(
            AppError::BadRequest(format!(
                "shapefile {} contains line geometry; only polygon field boundaries are supported",
                path.display()
            )),
        ),
        SHAPE_TYPE_MULTIPOINT | SHAPE_TYPE_MULTIPOINT_M | SHAPE_TYPE_MULTIPOINT_Z => Err(
            AppError::BadRequest(format!(
                "shapefile {} contains multipoint geometry; only polygon field boundaries are supported",
                path.display()
            )),
        ),
        SHAPE_TYPE_MULTIPATCH => Err(AppError::BadRequest(format!(
            "shapefile {} contains multipatch geometry; only polygon field boundaries are supported",
            path.display()
        ))),
        _ => Err(AppError::BadRequest(format!(
            "shapefile {} uses unsupported shape type {}",
            path.display(),
            shape_type
        ))),
    }
}

fn is_valid_lon_lat(longitude: f64, latitude: f64) -> bool {
    longitude.is_finite()
        && latitude.is_finite()
        && (-180.0..=180.0).contains(&longitude)
        && (-90.0..=90.0).contains(&latitude)
}

fn read_be_i32(bytes: &[u8], offset: usize) -> AppResult<i32> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| AppError::BadRequest("shapefile stream ended unexpectedly".to_string()))?;
    Ok(i32::from_be_bytes(
        slice.try_into().expect("slice length is checked"),
    ))
}

fn read_le_i32(bytes: &[u8], offset: usize) -> AppResult<i32> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| AppError::BadRequest("shapefile stream ended unexpectedly".to_string()))?;
    Ok(i32::from_le_bytes(
        slice.try_into().expect("slice length is checked"),
    ))
}

fn read_le_f64(bytes: &[u8], offset: usize) -> AppResult<f64> {
    let slice = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| AppError::BadRequest("shapefile stream ended unexpectedly".to_string()))?;
    Ok(f64::from_le_bytes(
        slice.try_into().expect("slice length is checked"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn polygon_shapefile_bytes(points: &[(f64, f64)]) -> Vec<u8> {
        let num_parts = 1i32;
        let num_points = points.len() as i32;
        let record_len = 4 + 32 + 4 + 4 + 4 + points.len() * 16;
        let file_len = SHAPEFILE_HEADER_BYTES + 8 + record_len;

        let x_min = points
            .iter()
            .map(|point| point.0)
            .fold(f64::INFINITY, f64::min);
        let x_max = points
            .iter()
            .map(|point| point.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let y_min = points
            .iter()
            .map(|point| point.1)
            .fold(f64::INFINITY, f64::min);
        let y_max = points
            .iter()
            .map(|point| point.1)
            .fold(f64::NEG_INFINITY, f64::max);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&ESRI_FILE_CODE.to_be_bytes());
        bytes.extend_from_slice(&[0u8; 20]);
        bytes.extend_from_slice(&((file_len / 2) as i32).to_be_bytes());
        bytes.extend_from_slice(&1000i32.to_le_bytes());
        bytes.extend_from_slice(&SHAPE_TYPE_POLYGON.to_le_bytes());
        bytes.extend_from_slice(&x_min.to_le_bytes());
        bytes.extend_from_slice(&y_min.to_le_bytes());
        bytes.extend_from_slice(&x_max.to_le_bytes());
        bytes.extend_from_slice(&y_max.to_le_bytes());
        bytes.extend_from_slice(&0f64.to_le_bytes());
        bytes.extend_from_slice(&0f64.to_le_bytes());
        bytes.extend_from_slice(&0f64.to_le_bytes());
        bytes.extend_from_slice(&0f64.to_le_bytes());
        bytes.extend_from_slice(&1i32.to_be_bytes());
        bytes.extend_from_slice(&((record_len / 2) as i32).to_be_bytes());
        bytes.extend_from_slice(&SHAPE_TYPE_POLYGON.to_le_bytes());
        bytes.extend_from_slice(&x_min.to_le_bytes());
        bytes.extend_from_slice(&y_min.to_le_bytes());
        bytes.extend_from_slice(&x_max.to_le_bytes());
        bytes.extend_from_slice(&y_max.to_le_bytes());
        bytes.extend_from_slice(&num_parts.to_le_bytes());
        bytes.extend_from_slice(&num_points.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        for (x, y) in points {
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn parse_polygon_records_accepts_single_ring_boundary() {
        let path = Path::new("field.shp");
        let bytes = polygon_shapefile_bytes(&[
            (-96.7, 41.1),
            (-96.2, 41.1),
            (-96.2, 41.4),
            (-96.7, 41.4),
            (-96.7, 41.1),
        ]);

        let records = parse_polygon_records(path, &bytes).expect("polygon shapefile should parse");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].coordinates.len(), 4);
    }

    #[test]
    fn parse_polygon_records_rejects_projected_coordinates() {
        let path = Path::new("projected.shp");
        let bytes = polygon_shapefile_bytes(&[
            (500_000.0, 4_500_000.0),
            (500_100.0, 4_500_000.0),
            (500_100.0, 4_500_100.0),
            (500_000.0, 4_500_000.0),
        ]);

        let err = parse_polygon_records(path, &bytes).expect_err("projected coords should fail");
        assert!(err
            .to_string()
            .contains("must use geographic lon/lat coordinates in EPSG:4326"));
    }
}
