// Minimal unit/integration tests for index edge cases and thermal calibration math.

use image::{GrayImage, ImageBuffer, Luma};
use imagery_processor::{
    io::{
        geotiff_spatial_sidecar_path, write_geotiff_spatial_sidecar, BandIngestEvidence,
        CalibrationStatus, GeoTiffSpatialSidecar,
    },
    pipeline::{indices::run_indices, masks::run_masks},
    IndexKind, IndicesArgs, MasksArgs, OutputFormat, SensorPreset,
};
use serde_json::Value;
use shared::schemas::{assert_raster_spatial_ref, RasterSpatialRef};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn temp_test_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("agbot_{name}_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_gray_image(path: &Path, width: u32, height: u32, values: &[u8]) {
    assert_eq!(values.len(), (width * height) as usize);
    let mut image = GrayImage::new(width, height);
    for (index, value) in values.iter().enumerate() {
        let x = (index as u32) % width;
        let y = (index as u32) / width;
        image.put_pixel(x, y, Luma([*value]));
    }
    image.save(path).unwrap();
}

fn write_gray16_image(path: &Path, width: u32, height: u32, values: &[u16]) {
    assert_eq!(values.len(), (width * height) as usize);
    let mut image: ImageBuffer<Luma<u16>, Vec<u16>> = ImageBuffer::new(width, height);
    for (index, value) in values.iter().enumerate() {
        let x = (index as u32) % width;
        let y = (index as u32) / width;
        image.put_pixel(x, y, Luma([*value]));
    }
    image.save(path).unwrap();
}

fn valid_spatial_ref(width: u32, height: u32) -> Value {
    let origin_x = -74.1;
    let origin_y = 40.8;
    let pixel_x = 0.0001;
    let pixel_y = -0.0001;

    serde_json::json!({
        "georeferenced": true,
        "crs": "EPSG:4326",
        "bbox": {
            "min_lon": origin_x,
            "min_lat": origin_y + pixel_y * height as f64,
            "max_lon": origin_x + pixel_x * width as f64,
            "max_lat": origin_y
        },
        "geo_transform": [origin_x, pixel_x, 0.0, origin_y, 0.0, pixel_y]
    })
}

fn write_metadata(input_dir: &Path, width: u32, height: u32, bands: &[(&str, &Path)]) {
    write_metadata_with_spatial_ref(
        input_dir,
        width,
        height,
        bands,
        Some(valid_spatial_ref(width, height)),
    );
}

fn write_metadata_with_spatial_ref(
    input_dir: &Path,
    width: u32,
    height: u32,
    bands: &[(&str, &Path)],
    spatial_ref: Option<Value>,
) {
    let file_paths = bands
        .iter()
        .map(|(name, path)| {
            (
                (*name).to_string(),
                Value::String(path.to_string_lossy().to_string()),
            )
        })
        .collect::<serde_json::Map<String, Value>>();

    let mut image_metadata = serde_json::json!({
        "timestamp": "2026-01-01T00:00:00Z",
        "gps_position": null,
        "bands": bands.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        "exposure_time": 1.0,
        "gain": 1.0,
        "width": width,
        "height": height
    });
    if let Some(spatial_ref) = spatial_ref {
        image_metadata
            .as_object_mut()
            .unwrap()
            .insert("spatial_ref".to_string(), spatial_ref);
    }

    let metadata = serde_json::json!({
        "metadata": image_metadata,
        "file_paths": file_paths,
        "image_id": uuid::Uuid::new_v4()
    });

    fs::write(
        input_dir.join("metadata_test.json"),
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .unwrap();
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {actual} to be within GEO tolerance of {expected}"
    );
}

fn valid_asserted_spatial_ref(width: u32, height: u32) -> RasterSpatialRef {
    let spatial_ref: RasterSpatialRef = serde_json::from_value(valid_spatial_ref(width, height))
        .expect("spatial ref fixture should deserialize");
    assert_raster_spatial_ref(Some(&spatial_ref), width, height)
        .expect("spatial ref fixture should assert")
}

fn base_indices_args(input_dir: PathBuf, output_dir: PathBuf) -> IndicesArgs {
    IndicesArgs {
        input_dir,
        output_dir,
        index: IndexKind::Ndvi,
        red: None,
        nir: None,
        red_edge: None,
        green: None,
        blue: None,
        swir1: None,
        swir2: None,
        out_format: OutputFormat::Png,
        sensor: None,
        mask: None,
    }
}

fn base_masks_args(input_dir: PathBuf, output_dir: PathBuf) -> MasksArgs {
    MasksArgs {
        input_dir,
        output_dir,
        qa_band: "QA_PIXEL".to_string(),
        kinds: Vec::new(),
        out_format: OutputFormat::Png,
    }
}

fn read_result_meta(output_dir: &Path) -> Value {
    let path = fs::read_dir(output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("_result.json"))
        })
        .unwrap();
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn read_band_ingest_evidence(output_dir: &Path) -> BandIngestEvidence {
    let path = fs::read_dir(output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("band_ingest_") && name.ends_with(".json"))
        })
        .unwrap();
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn read_mask_evidence(output_dir: &Path) -> Value {
    let path = fs::read_dir(output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("mask_evidence_") && name.ends_with(".json"))
        })
        .unwrap();
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn ndvi_handles_zero_denominator() {
    // (n - r) / (n + r) when n = r = 0 should not panic and yield 0/NODATA per our logic
    let r = 0.0f32;
    let n = 0.0f32;
    let denom = n + r;
    let v = if denom.abs() > f32::EPSILON {
        (n - r) / denom
    } else {
        0.0
    };
    assert_eq!(v, 0.0);
}

#[test]
fn ndvi_basic_values() {
    // Simple sanity: n=1, r=0 -> 1; n=0, r=1 -> -1; n=r -> 0
    assert!(((1.0f32 - 0.0) / (1.0 + 0.0) - 1.0).abs() < 1e-6);
    assert!(((0.0f32 - 1.0) / (0.0 + 1.0) + 1.0).abs() < 1e-6);
    assert!(((0.5f32 - 0.5) / (0.5 + 0.5) - 0.0).abs() < 1e-6);
}

#[test]
fn thermal_bt_from_radiance() {
    // TB = K2 / ln(1 + K1/L)
    let (k1, k2) = (774.8853f32, 1321.0789f32);
    let l = 10.0f32; // radiance
    let tb = k2 / ((k1 / l).ln_1p());
    assert!(tb.is_finite() && tb > 0.0);
}

#[test]
fn emissivity_correction_sane() {
    // LST = TB / (1 + (lambda * TB / rho) * ln(eps))
    let tb = 300.0f64;
    let rho = 1.4388e-2f64;
    let lambda_um = 10.895f64;
    let lambda_m = lambda_um * 1e-6;
    let eps = 0.98f64;
    let lst_k = tb / (1.0 + (lambda_m * tb / rho) * eps.ln());
    // For eps < 1, LST should be slightly higher than TB
    assert!(lst_k > tb - 0.01);
}

#[tokio::test]
async fn ndvi_stats_use_valid_masked_pixels_only() {
    let root = temp_test_dir("ndvi_masked_stats");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    let nir_path = input_dir.join("nir.png");
    let mask_path = input_dir.join("mask.png");
    write_gray_image(&red_path, 2, 1, &[10, 10]);
    write_gray_image(&nir_path, 2, 1, &[30, 30]);
    write_gray_image(&mask_path, 2, 1, &[255, 0]);
    write_metadata(
        &input_dir,
        2,
        1,
        &[("Red", red_path.as_path()), ("NIR", nir_path.as_path())],
    );

    let mut args = base_indices_args(input_dir, output_dir.clone());
    args.mask = Some(mask_path);

    run_indices(&args).await.unwrap();

    let meta = read_result_meta(&output_dir);
    let mean = meta["mean"].as_f64().unwrap();
    let min = meta["min"].as_f64().unwrap();
    let max = meta["max"].as_f64().unwrap();
    let valid_pixel_count = meta["valid_pixel_count"].as_u64().unwrap();
    let masked_count = meta["invalid_pixel_reasons"]["masked"].as_u64().unwrap();
    assert!((mean - 0.5).abs() < 1e-6);
    assert!((min - 0.5).abs() < 1e-6);
    assert!((max - 0.5).abs() < 1e-6);
    assert_eq!(valid_pixel_count, 1);
    assert_eq!(masked_count, 1);
}

#[tokio::test]
async fn indices_persist_sentinel2_band_ingest_evidence() {
    let root = temp_test_dir("sentinel2_ingest_evidence");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("b04.png");
    let nir_path = input_dir.join("b08.png");
    let red_edge_path = input_dir.join("b05.png");
    write_gray_image(&red_path, 2, 1, &[10, 10]);
    write_gray_image(&nir_path, 2, 1, &[30, 30]);
    write_gray_image(&red_edge_path, 2, 1, &[20, 20]);
    write_metadata(
        &input_dir,
        2,
        1,
        &[
            ("B04", red_path.as_path()),
            ("B08", nir_path.as_path()),
            ("B05", red_edge_path.as_path()),
        ],
    );

    let mut args = base_indices_args(input_dir, output_dir.clone());
    args.sensor = Some(SensorPreset::Sentinel2);

    run_indices(&args).await.unwrap();

    let evidence = read_band_ingest_evidence(&output_dir);
    assert_eq!(evidence.sensor.as_deref(), Some("sentinel2"));
    assert_eq!(evidence.width, 2);
    assert_eq!(evidence.height, 1);
    assert_eq!(evidence.resolved_bands.get("red").unwrap(), "B04");
    assert_eq!(evidence.resolved_bands.get("nir").unwrap(), "B08");
    assert_eq!(evidence.resolved_bands.get("red_edge").unwrap(), "B05");
}

#[tokio::test]
async fn indices_persist_asserted_spatial_ref() {
    let root = temp_test_dir("indices_spatial_ref");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    let nir_path = input_dir.join("nir.png");
    write_gray_image(&red_path, 2, 1, &[10, 20]);
    write_gray_image(&nir_path, 2, 1, &[30, 40]);
    write_metadata(
        &input_dir,
        2,
        1,
        &[("Red", red_path.as_path()), ("NIR", nir_path.as_path())],
    );

    let args = base_indices_args(input_dir, output_dir.clone());

    run_indices(&args).await.unwrap();

    let meta = read_result_meta(&output_dir);
    let spatial_ref = &meta["spatial_ref"];
    assert_eq!(spatial_ref["crs"].as_str().unwrap(), "EPSG:4326");
    assert_eq!(spatial_ref["georeferenced"].as_bool().unwrap(), true);
    assert_close(spatial_ref["resolution"]["x"].as_f64().unwrap(), 0.0001);
    assert_close(spatial_ref["resolution"]["y"].as_f64().unwrap(), 0.0001);
    assert_close(spatial_ref["bbox"]["min_lon"].as_f64().unwrap(), -74.1);
    assert_close(spatial_ref["bbox"]["min_lat"].as_f64().unwrap(), 40.7999);
    assert_close(spatial_ref["bbox"]["max_lon"].as_f64().unwrap(), -74.0998);
    assert_close(spatial_ref["bbox"]["max_lat"].as_f64().unwrap(), 40.8);
    assert_eq!(
        spatial_ref["geo_transform"]
            .as_array()
            .expect("transform should be recorded")
            .len(),
        6
    );
}

#[tokio::test]
async fn indices_reject_missing_spatial_ref() {
    let root = temp_test_dir("indices_missing_spatial_ref");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    let nir_path = input_dir.join("nir.png");
    write_gray_image(&red_path, 1, 1, &[10]);
    write_gray_image(&nir_path, 1, 1, &[30]);
    write_metadata_with_spatial_ref(
        &input_dir,
        1,
        1,
        &[("Red", red_path.as_path()), ("NIR", nir_path.as_path())],
        None,
    );

    let args = base_indices_args(input_dir, output_dir);

    let error = run_indices(&args).await.unwrap_err().to_string();
    assert!(error.contains("georeferencing"));
    assert!(error.contains("missing spatial_ref"));
}

#[tokio::test]
async fn indices_reject_zero_resolution_spatial_ref() {
    let root = temp_test_dir("indices_zero_resolution_spatial_ref");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    let nir_path = input_dir.join("nir.png");
    write_gray_image(&red_path, 1, 1, &[10]);
    write_gray_image(&nir_path, 1, 1, &[30]);
    write_metadata_with_spatial_ref(
        &input_dir,
        1,
        1,
        &[("Red", red_path.as_path()), ("NIR", nir_path.as_path())],
        Some(serde_json::json!({
            "georeferenced": true,
            "crs": "EPSG:4326",
            "bbox": {
                "min_lon": -74.1,
                "min_lat": 40.7999,
                "max_lon": -74.1,
                "max_lat": 40.8
            },
            "geo_transform": [-74.1, 0.0, 0.0, 40.8, 0.0, -0.0001]
        })),
    );

    let args = base_indices_args(input_dir, output_dir);

    let error = run_indices(&args).await.unwrap_err().to_string();
    assert!(error.contains("georeferencing"));
    assert!(error.contains("positive resolution"));
}

#[test]
fn geotiff_sidecar_records_asserted_transform() {
    let spatial_ref = valid_asserted_spatial_ref(2, 1);

    let sidecar = GeoTiffSpatialSidecar::from_spatial_ref(&spatial_ref).unwrap();
    let sidecar_path = geotiff_spatial_sidecar_path(Path::new("/tmp/product.tif"));

    assert_eq!(sidecar.format_version, 1);
    assert_eq!(sidecar.crs, "EPSG:4326");
    assert_eq!(
        sidecar.geo_transform,
        [-74.1, 0.0001, 0.0, 40.8, 0.0, -0.0001]
    );
    assert_eq!(sidecar.resolution.x, 0.0001);
    assert_eq!(sidecar.resolution.y, 0.0001);
    assert_eq!(sidecar.bbox.min_lon, -74.1);
    assert_close(sidecar.bbox.max_lon, -74.0998);
    assert!(sidecar_path.ends_with("product.tif.spatial_ref.json"));
}

#[tokio::test]
async fn geotiff_sidecar_write_round_trips_transform() {
    let root = temp_test_dir("geotiff_sidecar_write");
    let product_path = root.join("product.tif");
    let spatial_ref = valid_asserted_spatial_ref(2, 1);

    let sidecar_path = write_geotiff_spatial_sidecar(&product_path, &spatial_ref)
        .await
        .unwrap();
    let sidecar: GeoTiffSpatialSidecar =
        serde_json::from_str(&fs::read_to_string(sidecar_path).unwrap()).unwrap();

    assert_eq!(sidecar.crs, "EPSG:4326");
    assert_eq!(
        sidecar.geo_transform,
        [-74.1, 0.0001, 0.0, 40.8, 0.0, -0.0001]
    );
    assert_eq!(sidecar.resolution.x, 0.0001);
    assert_eq!(sidecar.resolution.y, 0.0001);
}

#[cfg(not(feature = "gdal-io"))]
#[tokio::test]
async fn indices_geotiff_fails_without_gdal_feature() {
    let root = temp_test_dir("indices_no_gdal_geotiff");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    let nir_path = input_dir.join("nir.png");
    write_gray_image(&red_path, 1, 1, &[10]);
    write_gray_image(&nir_path, 1, 1, &[30]);
    write_metadata(
        &input_dir,
        1,
        1,
        &[("Red", red_path.as_path()), ("NIR", nir_path.as_path())],
    );

    let mut args = base_indices_args(input_dir, output_dir.clone());
    args.out_format = OutputFormat::Geotiff;

    let error = run_indices(&args).await.unwrap_err().to_string();

    assert!(error.contains("gdal-io feature is not enabled"));
    let outputs = fs::read_dir(output_dir).unwrap().count();
    assert_eq!(outputs, 1, "only band ingest evidence should be written");
}

#[tokio::test]
async fn sentinel2_indices_record_radiometric_calibration_evidence() {
    let root = temp_test_dir("sentinel2_calibration_evidence");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("b04.png");
    let nir_path = input_dir.join("b08.png");
    let red_edge_path = input_dir.join("b05.png");
    write_gray_image(&red_path, 2, 1, &[64, 128]);
    write_gray_image(&nir_path, 2, 1, &[192, 224]);
    write_gray_image(&red_edge_path, 2, 1, &[80, 90]);
    write_metadata(
        &input_dir,
        2,
        1,
        &[
            ("B04", red_path.as_path()),
            ("B08", nir_path.as_path()),
            ("B05", red_edge_path.as_path()),
        ],
    );

    let mut args = base_indices_args(input_dir, output_dir.clone());
    args.sensor = Some(SensorPreset::Sentinel2);

    run_indices(&args).await.unwrap();

    let evidence = read_band_ingest_evidence(&output_dir);
    assert_eq!(
        evidence.radiometric_calibration.status,
        CalibrationStatus::CalibratedReflectance
    );
    let red_coefficients = evidence
        .radiometric_calibration
        .coefficients
        .get("B04")
        .unwrap();
    assert!((red_coefficients.gain - (1.0 / 255.0)).abs() < 1e-6);
    assert_eq!(red_coefficients.offset, 0.0);
    assert_eq!(red_coefficients.output_min, 0.0);
    assert_eq!(red_coefficients.output_max, 1.0);

    let meta = read_result_meta(&output_dir);
    assert_eq!(
        meta["radiometric_calibration"]["status"].as_str().unwrap(),
        "CalibratedReflectance"
    );
    assert_eq!(
        meta["radiometric_calibration"]["coefficients"]["B04"]["offset"]
            .as_f64()
            .unwrap(),
        0.0
    );
}

#[tokio::test]
async fn missing_calibration_coefficients_are_marked_uncalibrated_dn() {
    let root = temp_test_dir("dji_uncalibrated_dn");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    let nir_path = input_dir.join("nir.png");
    let red_edge_path = input_dir.join("re.png");
    write_gray_image(&red_path, 1, 1, &[64]);
    write_gray_image(&nir_path, 1, 1, &[192]);
    write_gray_image(&red_edge_path, 1, 1, &[80]);
    write_metadata(
        &input_dir,
        1,
        1,
        &[
            ("Red", red_path.as_path()),
            ("NIR", nir_path.as_path()),
            ("RE", red_edge_path.as_path()),
        ],
    );

    let mut args = base_indices_args(input_dir, output_dir.clone());
    args.sensor = Some(SensorPreset::DjiMultispectral);

    run_indices(&args).await.unwrap();

    let evidence = read_band_ingest_evidence(&output_dir);
    assert_eq!(
        evidence.radiometric_calibration.status,
        CalibrationStatus::UncalibratedDn
    );
    assert!(evidence.radiometric_calibration.coefficients.is_empty());

    let meta = read_result_meta(&output_dir);
    assert_eq!(
        meta["radiometric_calibration"]["status"].as_str().unwrap(),
        "UncalibratedDn"
    );
}

#[tokio::test]
async fn masks_persist_class_count_evidence() {
    let root = temp_test_dir("qa_mask_evidence");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let qa_path = input_dir.join("qa.png");
    write_gray16_image(&qa_path, 5, 1, &[1 << 3, 1 << 4, 1 << 5, 1 << 7, 0]);
    write_metadata(&input_dir, 5, 1, &[("QA_PIXEL", qa_path.as_path())]);

    let args = base_masks_args(input_dir, output_dir.clone());

    run_masks(&args).await.unwrap();

    let evidence = read_mask_evidence(&output_dir);
    assert_eq!(evidence["qa_band"].as_str().unwrap(), "QA_PIXEL");
    assert_eq!(evidence["class_counts"]["cloud"].as_u64().unwrap(), 1);
    assert_eq!(
        evidence["class_counts"]["cloud_shadow"].as_u64().unwrap(),
        1
    );
    assert_eq!(evidence["class_counts"]["snow"].as_u64().unwrap(), 1);
    assert_eq!(evidence["class_counts"]["water"].as_u64().unwrap(), 1);
    assert_eq!(evidence["class_counts"]["clear"].as_u64().unwrap(), 2);
    assert!(evidence["outputs"]["clear"]
        .as_str()
        .unwrap()
        .ends_with(".png"));
}

#[tokio::test]
async fn masks_error_when_qa_band_is_missing() {
    let root = temp_test_dir("qa_mask_missing_band");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    write_gray_image(&red_path, 1, 1, &[10]);
    write_metadata(&input_dir, 1, 1, &[("Red", red_path.as_path())]);

    let args = base_masks_args(input_dir, output_dir);

    let err = run_masks(&args).await.unwrap_err();
    assert!(err.to_string().contains("QA band 'QA_PIXEL' not found"));
}

#[tokio::test]
async fn ndvi_metadata_records_divide_by_zero_reason() {
    let root = temp_test_dir("ndvi_divide_by_zero_reason");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    let nir_path = input_dir.join("nir.png");
    write_gray_image(&red_path, 2, 1, &[0, 10]);
    write_gray_image(&nir_path, 2, 1, &[0, 30]);
    write_metadata(
        &input_dir,
        2,
        1,
        &[("Red", red_path.as_path()), ("NIR", nir_path.as_path())],
    );

    let args = base_indices_args(input_dir, output_dir.clone());

    run_indices(&args).await.unwrap();

    let meta = read_result_meta(&output_dir);
    assert_eq!(meta["valid_pixel_count"].as_u64().unwrap(), 1);
    assert_eq!(
        meta["invalid_pixel_reasons"]["divide_by_zero"]
            .as_u64()
            .unwrap(),
        1
    );
    assert!((meta["mean"].as_f64().unwrap() - 0.5).abs() < 1e-6);
}

#[tokio::test]
async fn indices_error_when_required_extra_band_is_missing() {
    let root = temp_test_dir("missing_extra_band");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    let nir_path = input_dir.join("nir.png");
    write_gray_image(&red_path, 1, 1, &[10]);
    write_gray_image(&nir_path, 1, 1, &[30]);
    write_metadata(
        &input_dir,
        1,
        1,
        &[("Red", red_path.as_path()), ("NIR", nir_path.as_path())],
    );

    let mut args = base_indices_args(input_dir, output_dir);
    args.index = IndexKind::Gndvi;

    let error = run_indices(&args).await.unwrap_err().to_string();
    assert!(error.contains("Green band"));
}

#[tokio::test]
async fn indices_error_when_mask_dimensions_do_not_match() {
    let root = temp_test_dir("mask_dimensions");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    let nir_path = input_dir.join("nir.png");
    let mask_path = input_dir.join("mask.png");
    write_gray_image(&red_path, 2, 1, &[10, 10]);
    write_gray_image(&nir_path, 2, 1, &[30, 30]);
    write_gray_image(&mask_path, 1, 1, &[255]);
    write_metadata(
        &input_dir,
        2,
        1,
        &[("Red", red_path.as_path()), ("NIR", nir_path.as_path())],
    );

    let mut args = base_indices_args(input_dir, output_dir);
    args.mask = Some(mask_path);

    let error = run_indices(&args).await.unwrap_err().to_string();
    assert!(error.contains("Mask dimensions mismatch"));
}
