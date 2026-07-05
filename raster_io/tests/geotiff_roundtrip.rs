//! Network-free GeoTIFF round-trip tests: fixtures are written
//! programmatically with the crate's baseline writer and read back.

use raster_io::{
    write_geotiff_f32, write_geotiff_u16, write_geotiff_u8, GeoTiffReader, GeoTiffTags, RasterBand,
    RasterDtype, RasterIoError, RasterWindow,
};
use std::path::PathBuf;

const UTM_EPSG: u32 = 32643;
const UTM_TRANSFORM: [f64; 6] = [500_000.0, 10.0, 0.0, 4_300_000.0, 0.0, -10.0];

fn temp_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("raster_io_test_{}", uuid_like()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{nanos}_{:?}", std::thread::current().id()).replace(['(', ')', ' '], "")
}

fn utm_tags() -> GeoTiffTags {
    GeoTiffTags {
        epsg: Some(UTM_EPSG),
        geo_transform: Some(UTM_TRANSFORM),
        nodata: Some(0.0),
    }
}

/// 16x16 u16 band with known deterministic values; pixel (0,0) is nodata.
fn u16_fixture_values() -> Vec<u16> {
    (0..256u16)
        .map(|i| if i == 0 { 0 } else { 1000 + i })
        .collect()
}

#[test]
fn u16_geotiff_roundtrip_preserves_pixels_and_georeferencing() {
    let path = temp_path("band_u16.tif");
    let values = u16_fixture_values();
    write_geotiff_u16(&path, 16, 16, &values, &utm_tags()).expect("fixture writes");

    let mut reader = GeoTiffReader::open(&path).expect("fixture opens");
    let info = reader.info().clone();
    assert_eq!((info.width, info.height), (16, 16));
    assert_eq!(info.dtype, RasterDtype::U16);
    assert_eq!(info.epsg, Some(UTM_EPSG));
    assert_eq!(info.crs().as_deref(), Some("EPSG:32643"));
    assert_eq!(info.geo_transform, Some(UTM_TRANSFORM));
    assert_eq!(info.nodata, Some(0.0));

    let band = reader.read_band().expect("band reads");
    assert_eq!(band.dtype(), RasterDtype::U16);
    assert_eq!(band.len(), 256);
    assert_eq!(band.as_u16().unwrap(), values.as_slice());
    assert_eq!(band.value_as_f64(1), Some(1001.0));
}

#[test]
fn spatial_ref_matches_shared_contract() {
    let path = temp_path("band_spatial.tif");
    write_geotiff_u16(&path, 16, 16, &u16_fixture_values(), &utm_tags()).unwrap();

    let reader = GeoTiffReader::open(&path).unwrap();
    let spatial_ref = reader.spatial_ref().expect("spatial ref builds");

    assert!(spatial_ref.georeferenced);
    assert_eq!(spatial_ref.crs.as_deref(), Some("EPSG:32643"));
    assert_eq!(spatial_ref.geo_transform, Some(UTM_TRANSFORM));
    let bbox = spatial_ref.bbox.expect("bbox present");
    assert_eq!(bbox.min_lon, 500_000.0);
    assert_eq!(bbox.max_lon, 500_160.0);
    assert_eq!(bbox.max_lat, 4_300_000.0);
    assert_eq!(bbox.min_lat, 4_299_840.0);
    let resolution = spatial_ref.resolution.expect("resolution derived");
    assert_eq!((resolution.x, resolution.y), (10.0, 10.0));
}

#[test]
fn window_read_returns_expected_subrect_and_rejects_out_of_bounds() {
    let path = temp_path("band_window.tif");
    let values = u16_fixture_values();
    write_geotiff_u16(&path, 16, 16, &values, &utm_tags()).unwrap();

    let mut reader = GeoTiffReader::open(&path).unwrap();
    let window = RasterWindow {
        x: 2,
        y: 3,
        width: 4,
        height: 2,
    };
    let cropped = reader.read_window(window).expect("window reads");
    let expected: Vec<u16> = (0..2u16)
        .flat_map(|row| {
            let start = (3 + row) * 16 + 2;
            (start..start + 4).map(|i| 1000 + i)
        })
        .collect();
    assert_eq!(cropped, RasterBand::U16(expected));

    let out_of_bounds = reader.read_window(RasterWindow {
        x: 14,
        y: 0,
        width: 4,
        height: 1,
    });
    assert!(matches!(
        out_of_bounds,
        Err(RasterIoError::WindowOutOfBounds { .. })
    ));
}

#[test]
fn u8_and_f32_bands_roundtrip() {
    let u8_path = temp_path("band_u8.tif");
    let u8_values: Vec<u8> = (0..64).map(|i| (i % 12) as u8).collect();
    write_geotiff_u8(&u8_path, 8, 8, &u8_values, &utm_tags()).unwrap();
    let mut u8_reader = GeoTiffReader::open(&u8_path).unwrap();
    assert_eq!(u8_reader.info().dtype, RasterDtype::U8);
    assert_eq!(u8_reader.read_band().unwrap(), RasterBand::U8(u8_values));

    let f32_path = temp_path("band_f32.tif");
    let f32_values: Vec<f32> = (0..64).map(|i| i as f32 * 0.25 - 1.0).collect();
    write_geotiff_f32(&f32_path, 8, 8, &f32_values, &utm_tags()).unwrap();
    let mut f32_reader = GeoTiffReader::open(&f32_path).unwrap();
    assert_eq!(f32_reader.info().dtype, RasterDtype::F32);
    assert_eq!(f32_reader.info().nodata, Some(0.0));
    assert_eq!(f32_reader.read_band().unwrap(), RasterBand::F32(f32_values));
}

#[test]
fn geographic_epsg_and_missing_georeferencing_are_handled() {
    let geo_path = temp_path("band_geographic.tif");
    let tags = GeoTiffTags {
        epsg: Some(4326),
        geo_transform: Some([-74.1, 0.0001, 0.0, 40.8, 0.0, -0.0001]),
        nodata: None,
    };
    write_geotiff_u16(&geo_path, 4, 4, &[7u16; 16], &tags).unwrap();
    let reader = GeoTiffReader::open(&geo_path).unwrap();
    assert_eq!(reader.info().crs().as_deref(), Some("EPSG:4326"));
    assert_eq!(reader.info().nodata, None);
    assert!(reader.spatial_ref().is_ok());

    let bare_path = temp_path("band_bare.tif");
    write_geotiff_u16(&bare_path, 4, 4, &[7u16; 16], &GeoTiffTags::default()).unwrap();
    let bare = GeoTiffReader::open(&bare_path).unwrap();
    assert_eq!(bare.info().epsg, None);
    assert_eq!(bare.info().geo_transform, None);
    assert!(matches!(
        bare.spatial_ref(),
        Err(RasterIoError::MissingGeoreferencing { .. })
    ));
}
