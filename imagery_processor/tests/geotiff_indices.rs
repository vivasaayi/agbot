//! End-to-end GeoTIFF band → calibrated NDVI pipeline test (network-free).
//!
//! Fixture band GeoTIFFs are written programmatically with `raster_io`,
//! carry hand-chosen Landsat C2 L2 SR DNs, EPSG:32643 georeferencing, and
//! nodata=0; the assertions check hand-computed reflectance-based NDVI,
//! georeferencing propagation into the result evidence and PNG spatial
//! sidecar, and nodata reason coding.

use imagery_processor::io::geotiff_ingest::{ingest_geotiff_index_bands, GeoTiffIngestError};
use imagery_processor::io::{CalibrationStatus, PngSpatialSidecar};
use imagery_processor::pipeline::calibration::SensorProfile;
use imagery_processor::pipeline::indices::run_indices;
use imagery_processor::{
    Cli, Commands, IndexKind, IndexResultMeta, IndicesArgs, OutputFormat, SensorProfileArg,
};
use raster_io::{write_geotiff_u16, GeoTiffTags};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const WIDTH: u32 = 4;
const HEIGHT: u32 = 4;
const UTM_TRANSFORM: [f64; 6] = [500_000.0, 10.0, 0.0, 4_300_000.0, 0.0, -10.0];
/// Landsat C2 L2 SR: refl = DN * 0.0000275 - 0.2.
const RED_DN: u16 = 10_000; // -> 0.075
const NIR_DN: u16 = 20_000; // -> 0.35
/// NDVI = (0.35 - 0.075) / (0.35 + 0.075).
const EXPECTED_NDVI: f32 = 0.275 / 0.425;

fn temp_test_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("agbot_{name}_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn utm_tags() -> GeoTiffTags {
    GeoTiffTags {
        epsg: Some(32643),
        geo_transform: Some(UTM_TRANSFORM),
        nodata: Some(0.0),
    }
}

/// Band DNs with pixel (0,0) set to the nodata/fill DN 0.
fn band_dns(value: u16) -> Vec<u16> {
    let mut dns = vec![value; (WIDTH * HEIGHT) as usize];
    dns[0] = 0;
    dns
}

fn write_metadata(input_dir: &Path, bands: &[(&str, &Path)]) -> PathBuf {
    let file_paths = bands
        .iter()
        .map(|(name, path)| {
            (
                (*name).to_string(),
                serde_json::Value::String(path.to_string_lossy().to_string()),
            )
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();
    let metadata = serde_json::json!({
        "metadata": {
            "timestamp": "2026-01-01T00:00:00Z",
            "gps_position": null,
            "bands": bands.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            "exposure_time": 1.0,
            "gain": 1.0,
            "width": WIDTH,
            "height": HEIGHT,
            "spatial_ref": null
        },
        "file_paths": file_paths,
        "image_id": uuid::Uuid::new_v4()
    });
    let metadata_path = input_dir.join("metadata_scene.json");
    fs::write(
        &metadata_path,
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .unwrap();
    metadata_path
}

fn indices_args(input_dir: PathBuf, output_dir: PathBuf) -> IndicesArgs {
    IndicesArgs {
        input_dir,
        output_dir,
        index: IndexKind::Ndvi,
        red: Some("B4".to_string()),
        nir: Some("B5".to_string()),
        red_edge: None,
        green: None,
        blue: None,
        swir1: None,
        swir2: None,
        band_overrides: Vec::new(),
        out_format: OutputFormat::Png,
        sensor: None,
        sensor_profile: SensorProfileArg::LandsatC2L2Sr,
        mask: None,
    }
}

fn find_file(dir: &Path, suffix: &str) -> PathBuf {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.to_string_lossy().ends_with(suffix))
        .unwrap_or_else(|| panic!("no *{suffix} in {}", dir.display()))
}

#[tokio::test]
async fn geotiff_bands_produce_calibrated_ndvi_with_georeferencing_evidence() {
    let root = temp_test_dir("geotiff_ndvi");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("B4.tif");
    let nir_path = input_dir.join("B5.tif");
    write_geotiff_u16(&red_path, WIDTH, HEIGHT, &band_dns(RED_DN), &utm_tags()).unwrap();
    write_geotiff_u16(&nir_path, WIDTH, HEIGHT, &band_dns(NIR_DN), &utm_tags()).unwrap();
    write_metadata(&input_dir, &[("B4", &red_path), ("B5", &nir_path)]);

    run_indices(&indices_args(input_dir, output_dir.clone()))
        .await
        .expect("GeoTIFF NDVI pipeline should succeed");

    let meta_path = find_file(&output_dir, "_ndvi_result.json");
    let meta: IndexResultMeta =
        serde_json::from_str(&fs::read_to_string(&meta_path).unwrap()).unwrap();

    // Hand-computed reflectance NDVI on every valid pixel.
    assert!(
        (meta.mean - EXPECTED_NDVI).abs() < 1e-5,
        "mean {} != {EXPECTED_NDVI}",
        meta.mean
    );
    assert!((meta.min - EXPECTED_NDVI).abs() < 1e-5);
    assert!((meta.max - EXPECTED_NDVI).abs() < 1e-5);

    // The DN=0 pixel is invalid with the nodata reason code.
    assert_eq!(meta.total_pixel_count, 16);
    assert_eq!(meta.valid_pixel_count, 15);
    assert_eq!(meta.invalid_pixel_reasons.get("nodata"), Some(&1));

    // Georeferencing from the fixture GeoTIFF tags is carried into evidence.
    assert_eq!(meta.spatial_ref.crs.as_deref(), Some("EPSG:32643"));
    assert_eq!(meta.spatial_ref.geo_transform, Some(UTM_TRANSFORM));
    assert!(meta.spatial_ref.georeferenced);

    // Batch-1 calibration evidence records the applied Landsat coefficients.
    assert_eq!(
        meta.radiometric_calibration.status,
        CalibrationStatus::CalibratedReflectance
    );
    let b4 = meta.radiometric_calibration.coefficients.get("B4").unwrap();
    assert!((b4.gain - 0.0000275).abs() < 1e-10);
    assert!((b4.offset + 0.2).abs() < 1e-7);
    assert_eq!(
        meta.reproducibility.parameters["sensor_profile"],
        serde_json::json!("landsat-c2-l2-sr")
    );

    // The PNG spatial sidecar carries the same georeferencing.
    let sidecar_path = find_file(&output_dir, ".png.spatial_ref.json");
    let sidecar: PngSpatialSidecar =
        serde_json::from_str(&fs::read_to_string(&sidecar_path).unwrap()).unwrap();
    assert_eq!(sidecar.crs.as_deref(), Some("EPSG:32643"));
    assert_eq!(sidecar.geo_transform, Some(UTM_TRANSFORM));
}

#[tokio::test]
async fn sensor_profile_without_geotiff_inputs_is_rejected() {
    let root = temp_test_dir("geotiff_profile_png_reject");
    let input_dir = root.join("input");
    let output_dir = root.join("output");
    fs::create_dir_all(&input_dir).unwrap();

    let red_path = input_dir.join("B4.png");
    let nir_path = input_dir.join("B5.png");
    image::GrayImage::from_pixel(WIDTH, HEIGHT, image::Luma([10]))
        .save(&red_path)
        .unwrap();
    image::GrayImage::from_pixel(WIDTH, HEIGHT, image::Luma([30]))
        .save(&nir_path)
        .unwrap();
    write_metadata(&input_dir, &[("B4", &red_path), ("B5", &nir_path)]);

    let error = run_indices(&indices_args(input_dir, output_dir))
        .await
        .expect_err("PNG bands with --sensor-profile must be rejected");
    assert!(
        error.to_string().contains("requires GeoTIFF"),
        "unexpected error: {error}"
    );
}

#[test]
fn mismatched_band_georeferencing_is_rejected_with_typed_errors() {
    let root = temp_test_dir("geotiff_mismatch");
    fs::create_dir_all(&root).unwrap();

    let red_path = root.join("B4.tif");
    write_geotiff_u16(&red_path, WIDTH, HEIGHT, &band_dns(RED_DN), &utm_tags()).unwrap();

    // Shifted geotransform.
    let shifted_path = root.join("B5_shifted.tif");
    let mut shifted_tags = utm_tags();
    shifted_tags.geo_transform = Some([500_100.0, 10.0, 0.0, 4_300_000.0, 0.0, -10.0]);
    write_geotiff_u16(
        &shifted_path,
        WIDTH,
        HEIGHT,
        &band_dns(NIR_DN),
        &shifted_tags,
    )
    .unwrap();

    // Different CRS.
    let other_crs_path = root.join("B5_crs.tif");
    let mut other_crs_tags = utm_tags();
    other_crs_tags.epsg = Some(32644);
    write_geotiff_u16(
        &other_crs_path,
        WIDTH,
        HEIGHT,
        &band_dns(NIR_DN),
        &other_crs_tags,
    )
    .unwrap();

    let metadata_path = write_metadata(&root, &[("B4", &red_path), ("B5", &shifted_path)]);
    let image: shared::schemas::MultispectralImage =
        serde_json::from_str(&fs::read_to_string(&metadata_path).unwrap()).unwrap();
    let resolved = BTreeMap::from([
        ("red".to_string(), "B4".to_string()),
        ("nir".to_string(), "B5".to_string()),
    ]);

    let transform_error = ingest_geotiff_index_bands(
        &image,
        None,
        resolved.clone(),
        Some(SensorProfile::LandsatC2L2Sr),
    )
    .expect_err("shifted geotransform must be rejected");
    assert!(matches!(
        transform_error,
        GeoTiffIngestError::GeoTransformMismatch { .. }
    ));

    let mut crs_image = image;
    crs_image.file_paths.insert(
        "B5".to_string(),
        other_crs_path.to_string_lossy().to_string(),
    );
    let crs_error = ingest_geotiff_index_bands(&crs_image, None, resolved, None)
        .expect_err("differing CRS must be rejected");
    assert!(matches!(crs_error, GeoTiffIngestError::CrsMismatch { .. }));
}

#[test]
fn sensor_profile_cli_flag_parses_documented_values() {
    use clap::Parser;

    for (flag, expected) in [
        ("landsat-c2-l2-sr", SensorProfileArg::LandsatC2L2Sr),
        (
            "sentinel2-l2a-baseline-0400",
            SensorProfileArg::Sentinel2L2ABaseline0400,
        ),
        ("sentinel2-l2a-legacy", SensorProfileArg::Sentinel2L2ALegacy),
        ("none", SensorProfileArg::None),
    ] {
        let cli = Cli::try_parse_from([
            "imagery_processor",
            "indices",
            "--input-dir",
            "in",
            "--output-dir",
            "out",
            "--sensor-profile",
            flag,
        ])
        .unwrap_or_else(|err| panic!("--sensor-profile {flag} should parse: {err}"));
        match cli.command {
            Commands::Indices(args) => assert_eq!(args.sensor_profile, expected),
            other => panic!("expected indices command, got {other:?}"),
        }
    }
}
