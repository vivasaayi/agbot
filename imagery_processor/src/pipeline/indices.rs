use anyhow::Context;
use image::GrayImage;
use shared::{error::AgroError, schemas::MultispectralImage, AgroResult};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tracing::{error, info};

use crate::{
    IndexBandRole, IndexBandValues, IndexPixelValue, IndexResultMeta, IndexStatisticsOutcome,
    IndicesArgs, OutputFormat,
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

#[derive(Debug, Clone, PartialEq)]
struct MaskedIndexStatistics {
    min: f32,
    max: f32,
    mean: f32,
    total_pixel_count: usize,
    clear_pixel_count: usize,
    valid_pixel_count: usize,
    clear_pixel_coverage: f32,
    valid_pixel_coverage: f32,
    outcome: IndexStatisticsOutcome,
    invalid_pixel_reasons: BTreeMap<String, usize>,
}

fn summarize_masked_index_values(
    index_values: &[IndexPixelValue],
    nodata_valid: &[bool],
    clear_mask: &[bool],
) -> AgroResult<MaskedIndexStatistics> {
    if index_values.len() != nodata_valid.len() || index_values.len() != clear_mask.len() {
        return Err(processing_error(format!(
            "masked index statistics length mismatch: values={}, nodata={}, mask={}",
            index_values.len(),
            nodata_valid.len(),
            clear_mask.len()
        )));
    }

    let total_pixel_count = index_values.len();
    let mut clear_pixel_count = 0usize;
    let mut valid_pixel_count = 0usize;
    let mut stats_min = f32::INFINITY;
    let mut stats_max = f32::NEG_INFINITY;
    let mut stats_sum = 0.0f64;
    let mut invalid_pixel_reasons = BTreeMap::new();

    for ((pixel_value, nodata_is_valid), is_clear) in
        index_values.iter().zip(nodata_valid).zip(clear_mask)
    {
        if !*is_clear {
            record_invalid_pixel(&mut invalid_pixel_reasons, "masked");
            continue;
        }
        clear_pixel_count += 1;

        if !*nodata_is_valid {
            record_invalid_pixel(&mut invalid_pixel_reasons, "nodata");
            continue;
        }

        match pixel_value {
            IndexPixelValue::Valid(value) if value.is_finite() => {
                stats_min = stats_min.min(*value);
                stats_max = stats_max.max(*value);
                stats_sum += *value as f64;
                valid_pixel_count += 1;
            }
            IndexPixelValue::Valid(_) => {
                record_invalid_pixel(&mut invalid_pixel_reasons, "non_finite");
            }
            IndexPixelValue::Invalid { reason } => {
                record_invalid_pixel(&mut invalid_pixel_reasons, reason);
            }
        }
    }

    let (min, max, mean) = if valid_pixel_count > 0 {
        (
            stats_min,
            stats_max,
            (stats_sum / valid_pixel_count as f64) as f32,
        )
    } else {
        (f32::NAN, f32::NAN, f32::NAN)
    };

    let denominator = total_pixel_count as f32;
    let clear_pixel_coverage = if total_pixel_count == 0 {
        0.0
    } else {
        clear_pixel_count as f32 / denominator
    };
    let valid_pixel_coverage = if total_pixel_count == 0 {
        0.0
    } else {
        valid_pixel_count as f32 / denominator
    };
    let outcome = if clear_pixel_count == 0 {
        IndexStatisticsOutcome::NoClearPixels
    } else if valid_pixel_count == 0 {
        IndexStatisticsOutcome::NoValidPixels
    } else {
        IndexStatisticsOutcome::Computed
    };

    Ok(MaskedIndexStatistics {
        min,
        max,
        mean,
        total_pixel_count,
        clear_pixel_count,
        valid_pixel_count,
        clear_pixel_coverage,
        valid_pixel_coverage,
        outcome,
        invalid_pixel_reasons,
    })
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
    let total_pixels = (width * height) as usize;
    let mut index_values = Vec::with_capacity(total_pixels);
    let mut nodata_valid = Vec::with_capacity(total_pixels);
    let mut clear_mask = Vec::with_capacity(total_pixels);

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

        let is_clear = valid_mask_at(&mask_img, x, y);
        clear_mask.push(is_clear);
        nodata_valid.push(valid_data);

        let pixel_value = if valid_data {
            args.index
                .compute_value(&values)
                .map_err(|err| processing_error(err.to_string()))?
        } else {
            IndexPixelValue::Invalid { reason: "nodata" }
        };
        index_values.push(pixel_value);

        let write_val = if is_clear && valid_data {
            pixel_value
                .value()
                .filter(|value| value.is_finite())
                .unwrap_or(NODATA_F32)
        } else {
            NODATA_F32
        };

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

    let stats = summarize_masked_index_values(&index_values, &nodata_valid, &clear_mask)?;

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
        min: stats.min,
        max: stats.max,
        mean: stats.mean,
        total_pixel_count: stats.total_pixel_count,
        clear_pixel_count: stats.clear_pixel_count,
        valid_pixel_count: stats.valid_pixel_count,
        clear_pixel_coverage: stats.clear_pixel_coverage,
        valid_pixel_coverage: stats.valid_pixel_coverage,
        statistics_outcome: stats.outcome,
        invalid_pixel_reasons: stats.invalid_pixel_reasons,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IndexStatisticsOutcome;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-5,
            "actual {actual} did not match expected {expected}"
        );
    }

    #[test]
    fn masked_index_statistics_exclude_cloud_and_nodata_pixels() {
        let stats = summarize_masked_index_values(
            &[
                IndexPixelValue::Valid(0.2),
                IndexPixelValue::Valid(0.9),
                IndexPixelValue::Valid(0.4),
                IndexPixelValue::Valid(0.8),
            ],
            &[true, true, false, true],
            &[true, false, true, false],
        )
        .expect("statistics compute");

        assert_eq!(stats.outcome, IndexStatisticsOutcome::Computed);
        assert_eq!(stats.total_pixel_count, 4);
        assert_eq!(stats.clear_pixel_count, 2);
        assert_eq!(stats.valid_pixel_count, 1);
        assert_close(stats.clear_pixel_coverage, 0.5);
        assert_close(stats.valid_pixel_coverage, 0.25);
        assert_close(stats.min, 0.2);
        assert_close(stats.max, 0.2);
        assert_close(stats.mean, 0.2);
        assert_eq!(stats.invalid_pixel_reasons.get("masked"), Some(&2));
        assert_eq!(stats.invalid_pixel_reasons.get("nodata"), Some(&1));
    }

    #[test]
    fn fully_clouded_index_statistics_report_no_clear_pixels() {
        let stats = summarize_masked_index_values(
            &[IndexPixelValue::Valid(0.2), IndexPixelValue::Valid(0.4)],
            &[true, true],
            &[false, false],
        )
        .expect("statistics compute");

        assert_eq!(stats.outcome, IndexStatisticsOutcome::NoClearPixels);
        assert_eq!(stats.total_pixel_count, 2);
        assert_eq!(stats.clear_pixel_count, 0);
        assert_eq!(stats.valid_pixel_count, 0);
        assert_eq!(stats.clear_pixel_coverage, 0.0);
        assert_eq!(stats.valid_pixel_coverage, 0.0);
        assert!(stats.min.is_nan());
        assert!(stats.max.is_nan());
        assert!(stats.mean.is_nan());
        assert_eq!(stats.invalid_pixel_reasons.get("masked"), Some(&2));
    }
}
