// Minimal unit/integration tests for index edge cases and thermal calibration math.

use image::{GrayImage, Luma};
use imagery_processor::{
    io::BandIngestEvidence, pipeline::indices::run_indices, IndexKind, IndicesArgs, OutputFormat,
    SensorPreset,
};
use serde_json::Value;
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

fn write_metadata(input_dir: &Path, width: u32, height: u32, bands: &[(&str, &Path)]) {
    let file_paths = bands
        .iter()
        .map(|(name, path)| {
            (
                (*name).to_string(),
                Value::String(path.to_string_lossy().to_string()),
            )
        })
        .collect::<serde_json::Map<String, Value>>();

    let metadata = serde_json::json!({
        "metadata": {
            "timestamp": "2026-01-01T00:00:00Z",
            "gps_position": null,
            "bands": bands.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            "exposure_time": 1.0,
            "gain": 1.0,
            "width": width,
            "height": height
        },
        "file_paths": file_paths,
        "image_id": uuid::Uuid::new_v4()
    });

    fs::write(
        input_dir.join("metadata_test.json"),
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .unwrap();
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
    assert!((mean - 0.5).abs() < 1e-6);
    assert!((min - 0.5).abs() < 1e-6);
    assert!((max - 0.5).abs() < 1e-6);
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
