// Minimal unit/integration tests for index edge cases and thermal calibration math.

use image::{GrayImage, ImageBuffer, Luma};
use imagery_processor::{
    io::{
        file_output_hash, geotiff_spatial_sidecar_path, png_spatial_sidecar_path,
        write_geotiff_spatial_sidecar, write_png_spatial_sidecar, BandIngestEvidence,
        CalibrationStatus, GeoTiffSpatialSidecar, PngGeoreferenceStatus, PngSpatialSidecar,
    },
    pipeline::{
        classify::run_classify, indices::run_indices, masks::run_masks, thermal::run_thermal,
    },
    BandOverrideSpec, ClassifyArgs, IndexBandRole, IndexKind, IndicesArgs, MasksArgs, OutputFormat,
    SensorPreset, TemperatureUnit, ThermalArgs, ThermalProduct,
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
        band_overrides: Vec::new(),
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

fn base_thermal_args(input_dir: PathBuf, output_dir: PathBuf) -> ThermalArgs {
    ThermalArgs {
        input_dir,
        output_dir,
        thermal_band: "Thermal".to_string(),
        thermal_band2: None,
        ml: Some(1.0),
        al: Some(0.0),
        k1: Some(774.8853),
        k2: Some(1321.0789),
        emissivity: 1.0,
        lambda_um: 10.895,
        unit: TemperatureUnit::Kelvin,
        products: vec![ThermalProduct::Lst],
        split_window: false,
        emissivity_from_ndvi: false,
        ndvi_image: None,
        red: None,
        nir: None,
        out_format: OutputFormat::Png,
        mask: None,
    }
}

fn base_classify_args(input_image: PathBuf, output_path: PathBuf) -> ClassifyArgs {
    ClassifyArgs {
        input_image,
        output_path,
        threshold: None,
        kmeans: None,
        seed: 0,
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

fn read_thermal_meta(output_dir: &Path) -> Value {
    let path = fs::read_dir(output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("thermal_result_") && name.ends_with(".json"))
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
async fn thermal_lst_records_intermediate_stats_and_spatial_ref() {
    let root = temp_test_dir("thermal_lst_evidence");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let thermal_path = input_dir.join("thermal.png");
    write_gray16_image(&thermal_path, 2, 1, &[10, 20]);
    write_metadata(&input_dir, 2, 1, &[("Thermal", thermal_path.as_path())]);

    let args = base_thermal_args(input_dir, output_dir.clone());

    run_thermal(&args).await.unwrap();

    let meta = read_thermal_meta(&output_dir);
    let expected_bt_10 = (1321.0789_f32 / (774.8853_f32 / 10.0).ln_1p()) as f64;
    let expected_bt_20 = (1321.0789_f32 / (774.8853_f32 / 20.0).ln_1p()) as f64;
    assert_eq!(meta["unit"].as_str().unwrap(), "kelvin");
    assert_eq!(meta["valid_pixel_count"].as_u64().unwrap(), 2);
    assert_close(meta["radiance"]["min"].as_f64().unwrap(), 10.0);
    assert_close(meta["radiance"]["max"].as_f64().unwrap(), 20.0);
    assert_close(
        meta["brightness_temperature"]["min"].as_f64().unwrap(),
        expected_bt_10,
    );
    assert_close(
        meta["brightness_temperature"]["max"].as_f64().unwrap(),
        expected_bt_20,
    );
    assert_close(meta["lst"]["min"].as_f64().unwrap(), expected_bt_10);
    assert_close(meta["lst"]["max"].as_f64().unwrap(), expected_bt_20);
    assert_eq!(meta["emissivity"].as_f64().unwrap(), 1.0);
    assert_eq!(meta["thermal_coefficients"]["ml"].as_f64().unwrap(), 1.0);
    assert_eq!(meta["thermal_coefficients"]["al"].as_f64().unwrap(), 0.0);
    assert_eq!(meta["spatial_ref"]["crs"].as_str().unwrap(), "EPSG:4326");
    assert_close(
        meta["spatial_ref"]["resolution"]["x"].as_f64().unwrap(),
        0.0001,
    );
}

#[tokio::test]
async fn thermal_lst_uses_ndvi_image_emissivity_and_records_evidence() {
    let root = temp_test_dir("thermal_ndvi_emissivity");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let thermal_path = input_dir.join("thermal.png");
    let ndvi_path = input_dir.join("ndvi.png");
    write_gray16_image(&thermal_path, 2, 1, &[10, 20]);
    write_gray_image(&ndvi_path, 2, 1, &[0, 255]);
    write_metadata(&input_dir, 2, 1, &[("Thermal", thermal_path.as_path())]);

    let mut args = base_thermal_args(input_dir, output_dir.clone());
    args.emissivity_from_ndvi = true;
    args.ndvi_image = Some(ndvi_path);

    run_thermal(&args).await.unwrap();

    let meta = read_thermal_meta(&output_dir);
    assert_eq!(
        meta["emissivity_evidence"]["method"].as_str().unwrap(),
        "ndvi_thresholds"
    );
    assert_eq!(
        meta["emissivity_evidence"]["source"].as_str().unwrap(),
        "ndvi_image"
    );
    assert!((meta["emissivity_evidence"]["min"].as_f64().unwrap() - 0.97).abs() < 1e-6);
    assert!((meta["emissivity_evidence"]["max"].as_f64().unwrap() - 0.99).abs() < 1e-6);
    assert!((meta["emissivity"].as_f64().unwrap() - 0.98).abs() < 1e-6);
}

#[tokio::test]
async fn thermal_split_window_records_two_band_method() {
    let root = temp_test_dir("thermal_split_window");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let thermal_path = input_dir.join("thermal_b10.png");
    let thermal2_path = input_dir.join("thermal_b11.png");
    write_gray16_image(&thermal_path, 2, 1, &[10, 20]);
    write_gray16_image(&thermal2_path, 2, 1, &[8, 18]);
    write_metadata(
        &input_dir,
        2,
        1,
        &[
            ("B10", thermal_path.as_path()),
            ("B11", thermal2_path.as_path()),
        ],
    );

    let mut args = base_thermal_args(input_dir, output_dir.clone());
    args.thermal_band = "B10".to_string();
    args.thermal_band2 = Some("B11".to_string());
    args.split_window = true;

    run_thermal(&args).await.unwrap();

    let meta = read_thermal_meta(&output_dir);
    assert_eq!(
        meta["thermal_method"]["method"].as_str().unwrap(),
        "split_window"
    );
    assert_eq!(
        meta["thermal_method"]["primary_band"].as_str().unwrap(),
        "B10"
    );
    assert_eq!(
        meta["thermal_method"]["second_band"].as_str().unwrap(),
        "B11"
    );
    assert!(meta["thermal_method"]["fallback_reason"].is_null());
    assert_eq!(
        meta["thermal_method"]["split_window_delta"]["count"]
            .as_u64()
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn thermal_split_window_falls_back_when_second_band_missing() {
    let root = temp_test_dir("thermal_split_window_fallback");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let thermal_path = input_dir.join("thermal.png");
    write_gray16_image(&thermal_path, 2, 1, &[10, 20]);
    write_metadata(&input_dir, 2, 1, &[("Thermal", thermal_path.as_path())]);

    let mut args = base_thermal_args(input_dir, output_dir.clone());
    args.split_window = true;

    run_thermal(&args).await.unwrap();

    let meta = read_thermal_meta(&output_dir);
    assert_eq!(
        meta["thermal_method"]["method"].as_str().unwrap(),
        "single_channel"
    );
    assert_eq!(
        meta["thermal_method"]["fallback_reason"].as_str().unwrap(),
        "second thermal band not provided"
    );
}

#[tokio::test]
async fn thermal_errors_when_coefficients_are_missing() {
    let root = temp_test_dir("thermal_missing_coefficients");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let thermal_path = input_dir.join("thermal.png");
    write_gray16_image(&thermal_path, 1, 1, &[10]);
    write_metadata(&input_dir, 1, 1, &[("Thermal", thermal_path.as_path())]);

    let mut args = base_thermal_args(input_dir, output_dir);
    args.ml = None;

    let error = run_thermal(&args).await.unwrap_err().to_string();

    assert!(error.contains("thermal coefficient 'ml' is required"));
}

#[tokio::test]
async fn thermal_errors_when_tir_band_is_missing() {
    let root = temp_test_dir("thermal_missing_tir");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    write_gray_image(&red_path, 1, 1, &[10]);
    write_metadata(&input_dir, 1, 1, &[("Red", red_path.as_path())]);

    let args = base_thermal_args(input_dir, output_dir);

    let error = run_thermal(&args).await.unwrap_err().to_string();

    assert!(error.contains("Thermal band 'Thermal' not found"));
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
    assert_eq!(meta["statistics_outcome"].as_str().unwrap(), "Computed");
    assert_eq!(meta["total_pixel_count"].as_u64().unwrap(), 2);
    assert_eq!(meta["clear_pixel_count"].as_u64().unwrap(), 1);
    assert_eq!(valid_pixel_count, 1);
    assert!((meta["clear_pixel_coverage"].as_f64().unwrap() - 0.5).abs() < 1e-6);
    assert!((meta["valid_pixel_coverage"].as_f64().unwrap() - 0.5).abs() < 1e-6);
    assert_eq!(masked_count, 1);
}

#[tokio::test]
async fn ndvi_fully_masked_scene_records_no_clear_pixels() {
    let root = temp_test_dir("ndvi_no_clear_pixels");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    let nir_path = input_dir.join("nir.png");
    let mask_path = input_dir.join("mask.png");
    write_gray_image(&red_path, 2, 1, &[10, 10]);
    write_gray_image(&nir_path, 2, 1, &[30, 30]);
    write_gray_image(&mask_path, 2, 1, &[0, 0]);
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
    assert_eq!(
        meta["statistics_outcome"].as_str().unwrap(),
        "NoClearPixels"
    );
    assert_eq!(meta["total_pixel_count"].as_u64().unwrap(), 2);
    assert_eq!(meta["clear_pixel_count"].as_u64().unwrap(), 0);
    assert_eq!(meta["valid_pixel_count"].as_u64().unwrap(), 0);
    assert_eq!(meta["clear_pixel_coverage"].as_f64().unwrap(), 0.0);
    assert_eq!(meta["valid_pixel_coverage"].as_f64().unwrap(), 0.0);
    assert!(meta["mean"].is_null());
    assert_eq!(meta["invalid_pixel_reasons"]["masked"].as_u64().unwrap(), 2);
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
    assert_eq!(evidence.band_index_to_name.get(&2).unwrap(), "B05");
    assert!(evidence.resolved_bands.get("red_edge").is_none());
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
async fn indices_png_writes_matching_spatial_sidecar() {
    let root = temp_test_dir("indices_png_sidecar");
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
    let output_path = PathBuf::from(meta["output_path"].as_str().unwrap());
    let sidecar_path = png_spatial_sidecar_path(&output_path);
    let sidecar: PngSpatialSidecar =
        serde_json::from_str(&fs::read_to_string(sidecar_path).unwrap()).unwrap();
    let expected = valid_asserted_spatial_ref(2, 1);

    assert_eq!(sidecar.status, PngGeoreferenceStatus::Georeferenced);
    assert_eq!(sidecar.crs.as_deref(), Some("EPSG:4326"));
    assert_eq!(sidecar.bbox, expected.bbox);
    assert_eq!(sidecar.resolution, expected.resolution);
    assert_eq!(sidecar.geo_transform, expected.geo_transform);
}

#[tokio::test]
async fn indices_retain_product_provenance_and_deterministic_hash() {
    let root = temp_test_dir("indices_reproducibility_evidence");
    let input_dir = root.join("input");
    let first_output_dir = root.join("output_a");
    let second_output_dir = root.join("output_b");
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

    let mut first_args = base_indices_args(input_dir.clone(), first_output_dir.clone());
    first_args.mask = Some(mask_path.clone());
    let mut second_args = base_indices_args(input_dir, second_output_dir.clone());
    second_args.mask = Some(mask_path.clone());

    run_indices(&first_args).await.unwrap();
    run_indices(&second_args).await.unwrap();

    let first_meta = read_result_meta(&first_output_dir);
    let second_meta = read_result_meta(&second_output_dir);
    let provenance = &first_meta["reproducibility"];
    assert_eq!(provenance["source_image_ids"].as_array().unwrap().len(), 1);
    assert_eq!(provenance["method"].as_str().unwrap(), "index");
    assert_eq!(provenance["parameters"]["index"].as_str().unwrap(), "ndvi");
    assert_eq!(
        provenance["parameters"]["resolved_bands"]["red"]
            .as_str()
            .unwrap(),
        "Red"
    );
    assert_eq!(
        provenance["mask_ref"].as_str().unwrap(),
        mask_path.to_string_lossy()
    );
    assert_eq!(
        provenance["calibration"]["status"].as_str().unwrap(),
        "UncalibratedDn"
    );
    assert!((provenance["statistics"]["mean"].as_f64().unwrap() - 0.5).abs() < 1e-6);
    assert!(
        (provenance["coverage"]["valid_pixel_coverage"]
            .as_f64()
            .unwrap()
            - 0.5)
            .abs()
            < 1e-6
    );

    let first_hash = provenance["output_hashes"]["product"]["value"]
        .as_str()
        .unwrap();
    let second_hash = second_meta["reproducibility"]["output_hashes"]["product"]["value"]
        .as_str()
        .unwrap();
    let first_output = PathBuf::from(first_meta["output_path"].as_str().unwrap());
    let actual_hash = file_output_hash(&first_output).await.unwrap();
    assert_eq!(first_hash, second_hash);
    assert_eq!(first_hash, actual_hash.value);
}

#[tokio::test]
async fn classify_threshold_records_boundaries_counts_and_spatial_sidecar() {
    let root = temp_test_dir("classify_threshold_evidence");
    let input_path = root.join("ndvi.png");
    let output_path = root.join("classified.png");
    write_gray_image(&input_path, 3, 1, &[0, 128, 255]);
    write_png_spatial_sidecar(&input_path, Some(&valid_asserted_spatial_ref(3, 1)))
        .await
        .unwrap();

    let mut args = base_classify_args(input_path, output_path.clone());
    args.threshold = Some(0.0);

    run_classify(&args).await.unwrap();

    let meta: Value =
        serde_json::from_str(&fs::read_to_string(output_path.with_extension("json")).unwrap())
            .unwrap();
    assert_eq!(meta["method"].as_str().unwrap(), "threshold");
    assert_eq!(meta["total_pixel_count"].as_u64().unwrap(), 3);
    assert_eq!(
        meta["class_boundaries"][0]["label"].as_str().unwrap(),
        "below_threshold"
    );
    assert_eq!(
        meta["class_boundaries"][1]["label"].as_str().unwrap(),
        "above_or_equal_threshold"
    );
    assert_eq!(meta["class_counts"][0]["pixel_count"].as_u64().unwrap(), 1);
    assert_eq!(meta["class_counts"][1]["pixel_count"].as_u64().unwrap(), 2);

    let sidecar: PngSpatialSidecar =
        serde_json::from_str(&fs::read_to_string(png_spatial_sidecar_path(&output_path)).unwrap())
            .unwrap();
    assert_eq!(sidecar.status, PngGeoreferenceStatus::Georeferenced);
    assert_eq!(sidecar.crs.as_deref(), Some("EPSG:4326"));
}

#[tokio::test]
async fn classify_kmeans_is_deterministic_with_seed() {
    let root = temp_test_dir("classify_kmeans_seed");
    let input_path = root.join("ndvi.png");
    let first_output = root.join("classified_a.png");
    let second_output = root.join("classified_b.png");
    write_gray_image(&input_path, 4, 1, &[0, 0, 128, 255]);

    let mut first_args = base_classify_args(input_path.clone(), first_output.clone());
    first_args.kmeans = Some(3);
    first_args.seed = 42;
    let mut second_args = base_classify_args(input_path, second_output.clone());
    second_args.kmeans = Some(3);
    second_args.seed = 42;

    run_classify(&first_args).await.unwrap();
    run_classify(&second_args).await.unwrap();

    assert_eq!(
        fs::read(&first_output).unwrap(),
        fs::read(&second_output).unwrap()
    );
    let meta: Value =
        serde_json::from_str(&fs::read_to_string(first_output.with_extension("json")).unwrap())
            .unwrap();
    assert_eq!(meta["method"].as_str().unwrap(), "kmeans");
    assert_eq!(meta["seed"].as_u64().unwrap(), 42);
    assert_eq!(meta["class_centers"].as_array().unwrap().len(), 3);
    assert_eq!(meta["class_counts"].as_array().unwrap().len(), 3);
    assert_eq!(
        meta["class_counts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["pixel_count"].as_u64().unwrap())
            .sum::<u64>(),
        4
    );
}

#[tokio::test]
async fn classify_kmeans_single_value_raster_reports_one_effective_class() {
    let root = temp_test_dir("classify_kmeans_single_value");
    let input_path = root.join("ndvi.png");
    let output_path = root.join("classified.png");
    write_gray_image(&input_path, 4, 1, &[128, 128, 128, 128]);

    let mut args = base_classify_args(input_path, output_path.clone());
    args.kmeans = Some(3);
    args.seed = 7;

    run_classify(&args).await.unwrap();

    let meta: Value =
        serde_json::from_str(&fs::read_to_string(output_path.with_extension("json")).unwrap())
            .unwrap();
    assert_eq!(meta["method"].as_str().unwrap(), "kmeans");
    assert_eq!(meta["effective_class_count"].as_u64().unwrap(), 1);
    assert_eq!(
        meta["class_counts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["pixel_count"].as_u64().unwrap() > 0)
            .count(),
        1
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
async fn dji_preset_resolves_ndre_default_bands() {
    let root = temp_test_dir("dji_ndre_defaults");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    let nir_path = input_dir.join("nir.png");
    let red_edge_path = input_dir.join("re.png");
    write_gray_image(&red_path, 1, 1, &[20]);
    write_gray_image(&nir_path, 1, 1, &[60]);
    write_gray_image(&red_edge_path, 1, 1, &[30]);
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
    args.index = IndexKind::Ndre;

    run_indices(&args).await.unwrap();

    let evidence = read_band_ingest_evidence(&output_dir);
    assert_eq!(evidence.resolved_bands.get("red_edge").unwrap(), "RE");
    assert_eq!(evidence.resolved_bands.get("nir").unwrap(), "NIR");

    let meta = read_result_meta(&output_dir);
    assert!((meta["mean"].as_f64().unwrap() - (30.0 / 90.0)).abs() < 1e-6);
}

#[tokio::test]
async fn generic_band_override_takes_precedence_and_is_recorded() {
    let root = temp_test_dir("generic_band_override");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    let default_nir_path = input_dir.join("nir.png");
    let override_nir_path = input_dir.join("alt_nir.png");
    write_gray_image(&red_path, 1, 1, &[20]);
    write_gray_image(&default_nir_path, 1, 1, &[40]);
    write_gray_image(&override_nir_path, 1, 1, &[80]);
    write_metadata(
        &input_dir,
        1,
        1,
        &[
            ("Red", red_path.as_path()),
            ("NIR", default_nir_path.as_path()),
            ("AltNIR", override_nir_path.as_path()),
        ],
    );

    let mut args = base_indices_args(input_dir, output_dir.clone());
    args.band_overrides = vec![BandOverrideSpec::new(IndexBandRole::Nir, "AltNIR")];

    run_indices(&args).await.unwrap();

    let evidence = read_band_ingest_evidence(&output_dir);
    assert_eq!(evidence.resolved_bands.get("nir").unwrap(), "AltNIR");

    let meta = read_result_meta(&output_dir);
    assert!((meta["mean"].as_f64().unwrap() - (60.0 / 100.0)).abs() < 1e-6);
}

#[tokio::test]
async fn generic_band_override_to_unknown_band_is_rejected() {
    let root = temp_test_dir("generic_band_override_missing");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("red.png");
    let nir_path = input_dir.join("nir.png");
    write_gray_image(&red_path, 1, 1, &[20]);
    write_gray_image(&nir_path, 1, 1, &[80]);
    write_metadata(
        &input_dir,
        1,
        1,
        &[("Red", red_path.as_path()), ("NIR", nir_path.as_path())],
    );

    let mut args = base_indices_args(input_dir, output_dir);
    args.band_overrides = vec![BandOverrideSpec::new(IndexBandRole::Nir, "MissingNIR")];

    let error = run_indices(&args).await.unwrap_err().to_string();

    assert!(error.contains("required band 'MissingNIR'"));
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
    assert!(error.contains("required band 'Green'"));
}

#[tokio::test]
async fn mndwi_uses_only_declared_green_and_swir1_bands() {
    let root = temp_test_dir("mndwi_declared_bands");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let green_path = input_dir.join("green.png");
    let swir1_path = input_dir.join("swir1.png");
    write_gray_image(&green_path, 1, 1, &[40]);
    write_gray_image(&swir1_path, 1, 1, &[25]);
    write_metadata(
        &input_dir,
        1,
        1,
        &[
            ("Green", green_path.as_path()),
            ("SWIR1", swir1_path.as_path()),
        ],
    );

    let mut args = base_indices_args(input_dir, output_dir.clone());
    args.index = IndexKind::Mndwi;

    run_indices(&args).await.unwrap();

    let meta = read_result_meta(&output_dir);
    assert_eq!(meta["index"].as_str().unwrap(), "mndwi");
    assert_eq!(meta["valid_pixel_count"].as_u64().unwrap(), 1);
    assert!((meta["mean"].as_f64().unwrap() - (15.0 / 65.0)).abs() < 1e-6);
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
