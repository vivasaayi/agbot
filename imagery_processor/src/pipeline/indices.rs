use anyhow::Context;
use image::GrayImage;
use shared::{error::AgroError, schemas::MultispectralImage, AgroResult};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tracing::{error, info};

use crate::{
    IndexBandRole, IndexBandValues, IndexPixelValue, IndexResultMeta, IndicesArgs, OutputFormat,
};

const NODATA_F32: f32 = -9999.0;

pub async fn run_indices(args: &IndicesArgs) -> AgroResult<()> {
    tokio::fs::create_dir_all(&args.output_dir).await?;

    let mut metadata_files = Vec::new();
    for entry in walkdir::WalkDir::new(&args.input_dir) {
        let entry = entry.context("walkdir")?;
        if entry.file_name().to_string_lossy().starts_with("metadata_")
            && entry.path().extension().map_or(false, |ext| ext == "json")
        {
            metadata_files.push(entry.path().to_path_buf());
        }
    }

    info!(count = metadata_files.len(), index = ?args.index, "Found metadata files");

    let mut failures = Vec::new();
    for mf in metadata_files {
        if let Err(e) = process_one(&mf, args).await {
            error!(file=%mf.display(), error=%e, "Failed processing");
            failures.push(format!("{}: {}", mf.display(), e));
        }
    }

    if !failures.is_empty() {
        return Err(AgroError::Processing(format!(
            "{} metadata file(s) failed: {}",
            failures.len(),
            failures.join("; ")
        )));
    }

    Ok(())
}

fn processing_error(message: impl Into<String>) -> AgroError {
    AgroError::Processing(message.into())
}

fn require_band_path<'a>(
    image: &'a MultispectralImage,
    band_name: &str,
    role: &str,
) -> AgroResult<&'a str> {
    image
        .file_paths
        .get(band_name)
        .map(String::as_str)
        .ok_or_else(|| processing_error(format!("{role} band '{band_name}' not found")))
}

fn load_luma_band(
    image: &MultispectralImage,
    band_name: &str,
    role: &str,
    expected_dimensions: (u32, u32),
) -> AgroResult<GrayImage> {
    let band_path = require_band_path(image, band_name, role)?;
    let band = image::open(band_path)
        .map_err(|e| processing_error(format!("Failed to load {role} band: {e}")))?
        .to_luma8();

    if band.dimensions() != expected_dimensions {
        return Err(processing_error(format!(
            "{role} band dimensions mismatch: expected {:?}, got {:?}",
            expected_dimensions,
            band.dimensions()
        )));
    }

    Ok(band)
}

fn valid_mask_at(mask_img: &Option<GrayImage>, x: u32, y: u32) -> bool {
    mask_img
        .as_ref()
        .map_or(true, |m| m.get_pixel(x, y)[0] != 0)
}

fn record_invalid_pixel(reasons: &mut BTreeMap<String, usize>, reason: &'static str) {
    *reasons.entry(reason.to_string()).or_insert(0) += 1;
}

fn calibrated_band_value(
    raw_value: f32,
    band_name: &str,
    calibration: &crate::io::RadiometricCalibrationEvidence,
) -> f32 {
    calibration
        .coefficients
        .get(band_name)
        .map(|coefficients| {
            (raw_value * coefficients.gain + coefficients.offset)
                .clamp(coefficients.output_min, coefficients.output_max)
        })
        .unwrap_or(raw_value)
}

#[derive(Debug, Clone)]
struct LoadedIndexBand {
    values: Vec<f32>,
    valid: Vec<bool>,
}

fn default_band_name(args: &IndicesArgs, role: IndexBandRole) -> String {
    if let Some(override_spec) = args
        .band_overrides
        .iter()
        .rev()
        .find(|override_spec| override_spec.role == role)
    {
        return override_spec.band_name.clone();
    }

    let preset_default = args
        .sensor
        .and_then(|preset| preset.default_band_for_role(role));

    match role {
        IndexBandRole::Blue => args
            .blue
            .clone()
            .or(preset_default.map(str::to_string))
            .unwrap_or_else(|| "Blue".to_string()),
        IndexBandRole::Green => args
            .green
            .clone()
            .or(preset_default.map(str::to_string))
            .unwrap_or_else(|| "Green".to_string()),
        IndexBandRole::Red => args
            .red
            .clone()
            .or(preset_default.map(str::to_string))
            .unwrap_or_else(|| "Red".to_string()),
        IndexBandRole::Nir => args
            .nir
            .clone()
            .or(preset_default.map(str::to_string))
            .unwrap_or_else(|| "NIR".to_string()),
        IndexBandRole::RedEdge => args
            .red_edge
            .clone()
            .or(preset_default.map(str::to_string))
            .unwrap_or_else(|| "RE".to_string()),
        IndexBandRole::Swir1 => args
            .swir1
            .clone()
            .or(preset_default.map(str::to_string))
            .unwrap_or_else(|| "SWIR1".to_string()),
        IndexBandRole::Swir2 => args
            .swir2
            .clone()
            .or(preset_default.map(str::to_string))
            .unwrap_or_else(|| "SWIR2".to_string()),
    }
}

fn resolved_index_band_names(args: &IndicesArgs) -> BTreeMap<String, String> {
    args.index
        .required_bands()
        .iter()
        .map(|role| (role.key().to_string(), default_band_name(args, *role)))
        .collect()
}

fn load_index_band(
    image: &MultispectralImage,
    band_name: &str,
    role: IndexBandRole,
    expected_dimensions: (u32, u32),
    calibration: &crate::io::RadiometricCalibrationEvidence,
) -> AgroResult<LoadedIndexBand> {
    #[cfg(feature = "gdal-io")]
    {
        let band_path = require_band_path(image, band_name, role.label())?;
        if band_path.ends_with(".tif") || band_path.ends_with(".tiff") {
            let (width, height, raw_values, nodata) =
                crate::io::gdal_util::read_first_band_as_f32(band_path).map_err(|err| {
                    processing_error(format!("GDAL read {} failed: {err}", role.label()))
                })?;
            if (width as u32, height as u32) != expected_dimensions {
                return Err(processing_error(format!(
                    "{} band dimensions mismatch: expected {:?}, got {:?}",
                    role.label(),
                    expected_dimensions,
                    (width as u32, height as u32)
                )));
            }

            let valid = raw_values
                .iter()
                .map(|value| nodata.map_or(true, |nodata| (*value as f64) != nodata))
                .collect::<Vec<_>>();
            let values = raw_values
                .into_iter()
                .map(|value| calibrated_band_value(value, band_name, calibration))
                .collect::<Vec<_>>();

            return Ok(LoadedIndexBand { values, valid });
        }
    }

    let band = load_luma_band(image, band_name, role.label(), expected_dimensions)?;
    let values = band
        .pixels()
        .map(|pixel| calibrated_band_value(pixel[0] as f32, band_name, calibration))
        .collect::<Vec<_>>();
    let valid = vec![true; values.len()];

    Ok(LoadedIndexBand { values, valid })
}

async fn process_one(metadata_file: &PathBuf, args: &IndicesArgs) -> AgroResult<()> {
    let image = crate::io::load_multispectral_metadata(metadata_file)
        .await
        .map_err(|err| processing_error(err.to_string()))?;
    let resolved_bands = resolved_index_band_names(args);
    let evidence = crate::io::resolve_band_ingest_evidence_for_resolved_bands(
        &image,
        args.sensor,
        resolved_bands,
    )
    .map_err(|err| processing_error(err.to_string()))?;
    crate::io::write_band_ingest_evidence(&args.output_dir, &evidence)
        .await
        .map_err(|err| processing_error(err.to_string()))?;

    let (width, height) = (image.metadata.width, image.metadata.height);
    let mut loaded_bands = BTreeMap::new();
    for role in args.index.required_bands() {
        let band_name = evidence
            .resolved_bands
            .get(role.key())
            .ok_or_else(|| {
                processing_error(format!(
                    "{:?} required {} band was not resolved",
                    args.index,
                    role.label()
                ))
            })?
            .clone();
        let band = load_index_band(
            &image,
            &band_name,
            *role,
            (width, height),
            &evidence.radiometric_calibration,
        )?;
        loaded_bands.insert(*role, band);
    }

    let mut out = image::ImageBuffer::new(width, height);
    let mut out_f32: Option<Vec<f32>> = Some(vec![NODATA_F32; (width * height) as usize]);

    let mut stats_min = f32::INFINITY;
    let mut stats_max = f32::NEG_INFINITY;
    let mut stats_sum = 0.0f64;
    let mut stats_count = 0usize;
    let mut invalid_pixel_reasons = BTreeMap::new();

    // Optional mask: non-zero means valid pixel
    let mask_img = if let Some(mask_path) = &args.mask {
        let mask = image::open(mask_path)
            .map_err(|e| processing_error(format!("Failed to load mask: {}", e)))?
            .to_luma8();
        if mask.dimensions() != (width, height) {
            return Err(processing_error(format!(
                "Mask dimensions mismatch: expected {:?}, got {:?}",
                (width, height),
                mask.dimensions()
            )));
        }
        Some(mask)
    } else {
        None
    };

    let (display_min, display_max) = args.index.expected_value_range();
    for (x, y, pix) in out.enumerate_pixels_mut() {
        let index = (y * width + x) as usize;
        let mut values = IndexBandValues::default();
        let mut valid_data = true;
        for role in args.index.required_bands() {
            let band = loaded_bands.get(role).ok_or_else(|| {
                processing_error(format!(
                    "{:?} required {} band was not loaded",
                    args.index,
                    role.label()
                ))
            })?;
            if !band.valid[index] {
                valid_data = false;
            }
            values.insert(*role, band.values[index]);
        }

        let mut write_val = NODATA_F32;
        if !valid_mask_at(&mask_img, x, y) {
            record_invalid_pixel(&mut invalid_pixel_reasons, "masked");
        } else if !valid_data {
            record_invalid_pixel(&mut invalid_pixel_reasons, "nodata");
        } else {
            match args
                .index
                .compute_value(&values)
                .map_err(|err| processing_error(err.to_string()))?
            {
                IndexPixelValue::Valid(value) => {
                    write_val = value;
                    stats_min = stats_min.min(value);
                    stats_max = stats_max.max(value);
                    stats_sum += value as f64;
                    stats_count += 1;
                }
                IndexPixelValue::Invalid { reason } => {
                    record_invalid_pixel(&mut invalid_pixel_reasons, reason);
                }
            }
        }

        if let Some(ref mut f32buf) = out_f32 {
            f32buf[index] = write_val;
        }
        let vis = if write_val.is_finite() {
            let scaled = (write_val.clamp(display_min, display_max) - display_min)
                / (display_max - display_min);
            (scaled * 255.0).round() as u8
        } else {
            0
        };
        *pix = image::Luma([vis]);
    }

    let (min, max, mean) = if stats_count > 0 {
        (
            stats_min,
            stats_max,
            (stats_sum / stats_count as f64) as f32,
        )
    } else {
        (f32::NAN, f32::NAN, f32::NAN)
    };

    let out_path = match args.out_format {
        OutputFormat::Png => {
            let out_name = format!(
                "{}_{}_{}.png",
                image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
                image.image_id,
                format!("{:?}", args.index).to_lowercase()
            );
            let p = args.output_dir.join(out_name);
            out.save(&p).map_err(|e| {
                shared::error::AgroError::Processing(format!("Failed to save index image: {}", e))
            })?;
            p
        }
        OutputFormat::Geotiff => {
            #[cfg(feature = "gdal-io")]
            {
                let out_name = format!(
                    "{}_{}_{}.tif",
                    image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
                    image.image_id,
                    format!("{:?}", args.index).to_lowercase()
                );
                let p = args.output_dir.join(out_name);
                let (w, h) = out.dimensions();
                if let Some(ref fbuf) = out_f32 {
                    crate::io::gdal_util::write_f32_geotiff(
                        p.to_string_lossy().as_ref(),
                        fbuf,
                        w as usize,
                        h as usize,
                        Some(NODATA_F32 as f64),
                    )
                    .map_err(|e| {
                        shared::error::AgroError::Processing(format!(
                            "Create GeoTIFF failed: {}",
                            e
                        ))
                    })?;
                } else {
                    crate::io::gdal_util::write_u8_geotiff_basic(
                        p.to_string_lossy().as_ref(),
                        out.as_raw(),
                        w as usize,
                        h as usize,
                    )
                    .map_err(|e| {
                        shared::error::AgroError::Processing(format!(
                            "Create GeoTIFF failed: {}",
                            e
                        ))
                    })?;
                }
                crate::io::gdal_util::apply_spatial_ref(
                    p.to_string_lossy().as_ref(),
                    &ingest.evidence.spatial_ref,
                )
                .map_err(|e| {
                    shared::error::AgroError::Processing(format!(
                        "Apply GeoTIFF spatial reference failed: {}",
                        e
                    ))
                })?;
                crate::io::write_geotiff_spatial_sidecar(&p, &ingest.evidence.spatial_ref).await?;
                p
            }
            #[cfg(not(feature = "gdal-io"))]
            {
                return Err(shared::error::AgroError::Processing(
                    "Geotiff output requested but gdal-io feature is not enabled".into(),
                ));
            }
        }
    };

    let meta = IndexResultMeta {
        timestamp: chrono::Utc::now(),
        source_images: vec![image.image_id],
        output_path: out_path.to_string_lossy().to_string(),
        index: format!("{:?}", args.index).to_lowercase(),
        min,
        max,
        mean,
        valid_pixel_count: stats_count,
        invalid_pixel_reasons,
        radiometric_calibration: evidence.radiometric_calibration.clone(),
        spatial_ref: evidence.spatial_ref.clone(),
    };

    let meta_name = format!(
        "{}_{}_{}_result.json",
        image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
        image.image_id,
        format!("{:?}", args.index).to_lowercase()
    );
    let meta_path = args.output_dir.join(meta_name);
    tokio::fs::write(meta_path, serde_json::to_string_pretty(&meta)?).await?;

    Ok(())
}
