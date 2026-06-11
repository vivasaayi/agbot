use anyhow::Context;
use image::GrayImage;
use shared::{error::AgroError, schemas::MultispectralImage, AgroResult};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tracing::{error, info};

use crate::{IndexKind, IndexResultMeta, IndicesArgs, OutputFormat};

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

async fn process_one(metadata_file: &PathBuf, args: &IndicesArgs) -> AgroResult<()> {
    let overrides = crate::io::BandOverrides::from_indices_args(args);
    let ingest = crate::io::ingest_multispectral_image(metadata_file, args.sensor, &overrides)
        .await
        .map_err(|err| processing_error(err.to_string()))?;
    crate::io::write_band_ingest_evidence(&args.output_dir, &ingest.evidence)
        .await
        .map_err(|err| processing_error(err.to_string()))?;

    let image: MultispectralImage = ingest.image;

    let (def_red, def_nir, def_re) = args
        .sensor
        .map(|p| p.default_bands())
        .unwrap_or((None, None, None));
    let red_name = ingest
        .evidence
        .resolved_bands
        .get("red")
        .cloned()
        .unwrap_or_else(|| {
            args.red
                .clone()
                .or(def_red.map(|s| s.to_string()))
                .unwrap_or_else(|| "Red".to_string())
        });
    let nir_name = ingest
        .evidence
        .resolved_bands
        .get("nir")
        .cloned()
        .unwrap_or_else(|| {
            args.nir
                .clone()
                .or(def_nir.map(|s| s.to_string()))
                .unwrap_or_else(|| "NIR".to_string())
        });
    let re_name = ingest
        .evidence
        .resolved_bands
        .get("red_edge")
        .cloned()
        .unwrap_or_else(|| {
            args.red_edge
                .clone()
                .or(def_re.map(|s| s.to_string()))
                .unwrap_or_else(|| "RE".to_string())
        });
    let green_name = args.green.clone().unwrap_or_else(|| "Green".to_string());
    let blue_name = args.blue.clone().unwrap_or_else(|| "Blue".to_string());
    let swir1_name = args.swir1.clone().unwrap_or_else(|| "SWIR1".to_string());
    let swir2_name = args.swir2.clone().unwrap_or_else(|| "SWIR2".to_string());

    // Locate band files
    let red_path = require_band_path(&image, &red_name, "Red")?;
    let nir_path = require_band_path(&image, &nir_name, "NIR")?;

    // Load bands (prefer GDAL for TIFFs when enabled)
    #[cfg(feature = "gdal-io")]
    let use_gdal = red_path.ends_with(".tif")
        || red_path.ends_with(".tiff")
        || nir_path.ends_with(".tif")
        || nir_path.ends_with(".tiff");
    #[cfg(not(feature = "gdal-io"))]
    let use_gdal = false;

    let (red_img, nir_img, red_f32_opt, nir_f32_opt, nd_red, nd_nir) = if use_gdal {
        #[cfg(feature = "gdal-io")]
        {
            let (rw, rh, red_buf, r_nd) = crate::io::gdal_util::read_first_band_as_f32(red_path)
                .map_err(|e| processing_error(format!("GDAL read red failed: {}", e)))?;
            let (nw, nh, nir_buf0, n_nd0) = crate::io::gdal_util::read_first_band_as_f32(nir_path)
                .map_err(|e| processing_error(format!("GDAL read nir failed: {}", e)))?;
            let (nw, nh, nir_buf, n_nd) = if rw != nw || rh != nh {
                // Resample NIR to red dimensions
                let (_w, _h, buf, nd) =
                    crate::io::gdal_util::read_first_band_as_f32_resampled(nir_path, rw, rh)
                        .map_err(|e| {
                            processing_error(format!("GDAL resample nir failed: {}", e))
                        })?;
                (rw, rh, buf, nd)
            } else {
                (nw, nh, nir_buf0, n_nd0)
            };
            // Normalize to 0..255 u8 quicklook for now
            let mut red_img = image::GrayImage::new(rw as u32, rh as u32);
            let mut nir_img = image::GrayImage::new(nw as u32, nh as u32);
            for y in 0..rh {
                for x in 0..rw {
                    let i = y * rw + x;
                    let r = red_buf[i];
                    let n = nir_buf[i];
                    let r8 = (r.max(0.0).min(65535.0) / 65535.0 * 255.0) as u8;
                    let n8 = (n.max(0.0).min(65535.0) / 65535.0 * 255.0) as u8;
                    red_img.put_pixel(x as u32, y as u32, image::Luma([r8]));
                    nir_img.put_pixel(x as u32, y as u32, image::Luma([n8]));
                }
            }
            (
                red_img,
                nir_img,
                Some(red_buf),
                Some(nir_buf),
                r_nd.map(|v| v as f32),
                n_nd.map(|v| v as f32),
            )
        }
        #[cfg(not(feature = "gdal-io"))]
        {
            unreachable!()
        }
    } else {
        let red_img = image::open(red_path)
            .map_err(|e| processing_error(format!("Failed to load Red band: {}", e)))?
            .to_luma8();
        let nir_img = image::open(nir_path)
            .map_err(|e| processing_error(format!("Failed to load NIR band: {}", e)))?
            .to_luma8();
        (
            red_img,
            nir_img,
            None::<Vec<f32>>,
            None::<Vec<f32>>,
            None::<f32>,
            None::<f32>,
        )
    };

    if red_img.dimensions() != nir_img.dimensions() {
        return Err(processing_error("Band dimensions mismatch"));
    }

    let (width, height) = red_img.dimensions();
    let mut out = image::ImageBuffer::new(width, height);
    // Optional f32 buffer for float output when GDAL used
    let have_f32 = red_f32_opt.is_some() && nir_f32_opt.is_some();
    let mut out_f32: Option<Vec<f32>> = if have_f32 {
        Some(vec![NODATA_F32; (width * height) as usize])
    } else {
        None
    };

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

    match args.index {
        IndexKind::Ndvi => {
            for (x, y, pix) in out.enumerate_pixels_mut() {
                let valid_mask = valid_mask_at(&mask_img, x, y);
                let (r, n, valid_data) =
                    if let (Some(ref rbuf), Some(ref nbuf)) = (&red_f32_opt, &nir_f32_opt) {
                        let i = (y * width + x) as usize;
                        let r = rbuf[i];
                        let n = nbuf[i];
                        let nodata_ok = match nd_red {
                            Some(nd) => r != nd,
                            None => true,
                        } && match nd_nir {
                            Some(nd) => n != nd,
                            None => true,
                        };
                        (
                            calibrated_band_value(
                                r,
                                &red_name,
                                &ingest.evidence.radiometric_calibration,
                            ),
                            calibrated_band_value(
                                n,
                                &nir_name,
                                &ingest.evidence.radiometric_calibration,
                            ),
                            nodata_ok,
                        )
                    } else {
                        (
                            calibrated_band_value(
                                red_img.get_pixel(x, y)[0] as f32,
                                &red_name,
                                &ingest.evidence.radiometric_calibration,
                            ),
                            calibrated_band_value(
                                nir_img.get_pixel(x, y)[0] as f32,
                                &nir_name,
                                &ingest.evidence.radiometric_calibration,
                            ),
                            true,
                        )
                    };
                let denom = n + r;
                let mut write_val = NODATA_F32;
                if !valid_mask {
                    record_invalid_pixel(&mut invalid_pixel_reasons, "masked");
                } else if !valid_data {
                    record_invalid_pixel(&mut invalid_pixel_reasons, "nodata");
                } else if denom.abs() <= f32::EPSILON {
                    record_invalid_pixel(&mut invalid_pixel_reasons, "divide_by_zero");
                } else {
                    let v = ((n - r) / denom).clamp(-1.0, 1.0);
                    write_val = v;
                    stats_min = stats_min.min(v);
                    stats_max = stats_max.max(v);
                    stats_sum += v as f64;
                    stats_count += 1;
                }
                if let Some(ref mut f32buf) = out_f32 {
                    f32buf[(y * width + x) as usize] = write_val;
                }
                let vis = if write_val.is_finite() {
                    ((write_val.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
                } else {
                    0
                };
                *pix = image::Luma([vis]);
            }
        }
        IndexKind::Ndre => {
            let re_path = require_band_path(&image, &re_name, "Red-edge")?;
            let re_img = if use_gdal {
                #[cfg(feature = "gdal-io")]
                {
                    let (rw, rh, re_buf, _nd) =
                        crate::io::gdal_util::read_first_band_as_f32(re_path).map_err(|e| {
                            processing_error(format!("GDAL read red-edge failed: {}", e))
                        })?;
                    let mut re_img = image::GrayImage::new(rw as u32, rh as u32);
                    for y in 0..rh {
                        for x in 0..rw {
                            let i = y * rw + x;
                            let v = (re_buf[i].max(0.0).min(65535.0) / 65535.0 * 255.0) as u8;
                            re_img.put_pixel(x as u32, y as u32, image::Luma([v]));
                        }
                    }
                    re_img
                }
                #[cfg(not(feature = "gdal-io"))]
                {
                    unreachable!()
                }
            } else {
                image::open(re_path)
                    .map_err(|e| processing_error(format!("Failed to load Red-edge band: {}", e)))?
                    .to_luma8()
            };
            if re_img.dimensions() != red_img.dimensions() {
                return Err(processing_error("Red-edge band dimensions mismatch"));
            }
            for (x, y, pix) in out.enumerate_pixels_mut() {
                let valid_mask = valid_mask_at(&mask_img, x, y);
                let (re, n, valid_data) =
                    if let (Some(ref _rbuf), Some(ref nbuf)) = (&red_f32_opt, &nir_f32_opt) {
                        let i = (y * width + x) as usize;
                        // re_img is u8; use it as proxy unless we also GDAL-read RE band; for simplicity use re_img u8
                        let re = re_img.get_pixel(x, y)[0] as f32;
                        let n = nbuf[i];
                        let nodata_ok = match nd_nir {
                            Some(nd) => n != nd,
                            None => true,
                        };
                        (
                            calibrated_band_value(
                                re,
                                &re_name,
                                &ingest.evidence.radiometric_calibration,
                            ),
                            calibrated_band_value(
                                n,
                                &nir_name,
                                &ingest.evidence.radiometric_calibration,
                            ),
                            nodata_ok,
                        )
                    } else {
                        (
                            calibrated_band_value(
                                re_img.get_pixel(x, y)[0] as f32,
                                &re_name,
                                &ingest.evidence.radiometric_calibration,
                            ),
                            calibrated_band_value(
                                nir_img.get_pixel(x, y)[0] as f32,
                                &nir_name,
                                &ingest.evidence.radiometric_calibration,
                            ),
                            true,
                        )
                    };
                let denom = n + re;
                let mut write_val = NODATA_F32;
                if !valid_mask {
                    record_invalid_pixel(&mut invalid_pixel_reasons, "masked");
                } else if !valid_data {
                    record_invalid_pixel(&mut invalid_pixel_reasons, "nodata");
                } else if denom.abs() <= f32::EPSILON {
                    record_invalid_pixel(&mut invalid_pixel_reasons, "divide_by_zero");
                } else {
                    let v = ((n - re) / denom).clamp(-1.0, 1.0);
                    write_val = v;
                    stats_min = stats_min.min(v);
                    stats_max = stats_max.max(v);
                    stats_sum += v as f64;
                    stats_count += 1;
                }
                if let Some(ref mut f32buf) = out_f32 {
                    f32buf[(y * width + x) as usize] = write_val;
                }
                let vis = if write_val.is_finite() {
                    ((write_val.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
                } else {
                    0
                };
                *pix = image::Luma([vis]);
            }
        }
        IndexKind::Evi => {
            // EVI = 2.5 * (NIR - Red) / (NIR + 6*Red - 7.5*Blue + 1)
            let blue_img = load_luma_band(&image, &blue_name, "Blue", (width, height))?;
            for (x, y, pix) in out.enumerate_pixels_mut() {
                let valid_mask = valid_mask_at(&mask_img, x, y);
                let r = if let Some(ref rbuf) = red_f32_opt {
                    rbuf[(y * width + x) as usize]
                } else {
                    red_img.get_pixel(x, y)[0] as f32
                };
                let n = if let Some(ref nbuf) = nir_f32_opt {
                    nbuf[(y * width + x) as usize]
                } else {
                    nir_img.get_pixel(x, y)[0] as f32
                };
                let b = blue_img.get_pixel(x, y)[0] as f32;
                let denom = n + 6.0 * r - 7.5 * b + 1.0;
                let mut write_val = NODATA_F32;
                let nodata_ok = match nd_red {
                    Some(nd) => r != nd,
                    None => true,
                } && match nd_nir {
                    Some(nd) => n != nd,
                    None => true,
                };
                if valid_mask && nodata_ok && denom.abs() > f32::EPSILON {
                    let v = 2.5 * (n - r) / denom;
                    write_val = v;
                    stats_min = stats_min.min(v);
                    stats_max = stats_max.max(v);
                    stats_sum += v as f64;
                    stats_count += 1;
                }
                if let Some(ref mut f32buf) = out_f32 {
                    f32buf[(y * width + x) as usize] = write_val;
                }
                let vis = if write_val.is_finite() {
                    ((write_val.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
                } else {
                    0
                };
                *pix = image::Luma([vis]);
            }
        }
        IndexKind::Vari => {
            // VARI = (G - R) / (G + R - B)
            let g_img = load_luma_band(&image, &green_name, "Green", (width, height))?;
            let b_img = load_luma_band(&image, &blue_name, "Blue", (width, height))?;
            for (x, y, pix) in out.enumerate_pixels_mut() {
                let r = red_img.get_pixel(x, y)[0] as f32;
                let g = g_img.get_pixel(x, y)[0] as f32;
                let b = b_img.get_pixel(x, y)[0] as f32;
                let denom = g + r - b;
                let mut write_val = NODATA_F32;
                if valid_mask_at(&mask_img, x, y) && denom.abs() > f32::EPSILON {
                    let v = (g - r) / denom;
                    write_val = v;
                    stats_min = stats_min.min(v);
                    stats_max = stats_max.max(v);
                    stats_sum += v as f64;
                    stats_count += 1;
                }
                if let Some(ref mut f32buf) = out_f32 {
                    f32buf[(y * width + x) as usize] = write_val;
                }
                *pix = image::Luma([if write_val.is_finite() {
                    ((write_val.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
                } else {
                    0
                }]);
            }
        }
        IndexKind::Gndvi => {
            // GNDVI = (NIR - G) / (NIR + G)
            let g_img = load_luma_band(&image, &green_name, "Green", (width, height))?;
            for (x, y, pix) in out.enumerate_pixels_mut() {
                let g = g_img.get_pixel(x, y)[0] as f32;
                let n = nir_img.get_pixel(x, y)[0] as f32;
                let denom = n + g;
                let mut write_val = NODATA_F32;
                if valid_mask_at(&mask_img, x, y) && denom.abs() > f32::EPSILON {
                    let v = (n - g) / denom;
                    write_val = v;
                    stats_min = stats_min.min(v);
                    stats_max = stats_max.max(v);
                    stats_sum += v as f64;
                    stats_count += 1;
                }
                if let Some(ref mut f32buf) = out_f32 {
                    f32buf[(y * width + x) as usize] = write_val;
                }
                *pix = image::Luma([if write_val.is_finite() {
                    ((write_val.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
                } else {
                    0
                }]);
            }
        }
        IndexKind::Ndwi => {
            // NDWI (McFeeters) = (G - NIR) / (G + NIR)
            let g_img = load_luma_band(&image, &green_name, "Green", (width, height))?;
            for (x, y, pix) in out.enumerate_pixels_mut() {
                let g = g_img.get_pixel(x, y)[0] as f32;
                let n = nir_img.get_pixel(x, y)[0] as f32;
                let denom = g + n;
                let mut write_val = NODATA_F32;
                if valid_mask_at(&mask_img, x, y) && denom.abs() > f32::EPSILON {
                    let v = (g - n) / denom;
                    write_val = v;
                    stats_min = stats_min.min(v);
                    stats_max = stats_max.max(v);
                    stats_sum += v as f64;
                    stats_count += 1;
                }
                if let Some(ref mut f32buf) = out_f32 {
                    f32buf[(y * width + x) as usize] = write_val;
                }
                *pix = image::Luma([if write_val.is_finite() {
                    ((write_val.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
                } else {
                    0
                }]);
            }
        }
        IndexKind::Mndwi => {
            // MNDWI (Xu) = (G - SWIR1) / (G + SWIR1)
            let g_img = load_luma_band(&image, &green_name, "Green", (width, height))?;
            let s_img = load_luma_band(&image, &swir1_name, "SWIR1", (width, height))?;
            for (x, y, pix) in out.enumerate_pixels_mut() {
                let g = g_img.get_pixel(x, y)[0] as f32;
                let s = s_img.get_pixel(x, y)[0] as f32;
                let denom = g + s;
                let mut write_val = NODATA_F32;
                if valid_mask_at(&mask_img, x, y) && denom.abs() > f32::EPSILON {
                    let v = (g - s) / denom;
                    write_val = v;
                    stats_min = stats_min.min(v);
                    stats_max = stats_max.max(v);
                    stats_sum += v as f64;
                    stats_count += 1;
                }
                if let Some(ref mut f32buf) = out_f32 {
                    f32buf[(y * width + x) as usize] = write_val;
                }
                *pix = image::Luma([if write_val.is_finite() {
                    ((write_val.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
                } else {
                    0
                }]);
            }
        }
        IndexKind::Msavi => {
            // MSAVI = (2*NIR + 1 - sqrt((2*NIR + 1)^2 - 8*(NIR - R))) / 2
            for (x, y, pix) in out.enumerate_pixels_mut() {
                let r = red_img.get_pixel(x, y)[0] as f32;
                let n = nir_img.get_pixel(x, y)[0] as f32;
                let term = (2.0 * n + 1.0) * (2.0 * n + 1.0) - 8.0 * (n - r);
                let mut write_val = NODATA_F32;
                if valid_mask_at(&mask_img, x, y) && term >= 0.0 {
                    let v = (2.0 * n + 1.0 - term.sqrt()) * 0.5;
                    write_val = v;
                    stats_min = stats_min.min(v);
                    stats_max = stats_max.max(v);
                    stats_sum += v as f64;
                    stats_count += 1;
                }
                if let Some(ref mut f32buf) = out_f32 {
                    f32buf[(y * width + x) as usize] = write_val;
                }
                *pix = image::Luma([if write_val.is_finite() {
                    ((write_val.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
                } else {
                    0
                }]);
            }
        }
        IndexKind::Nbr => {
            // NBR = (NIR - SWIR2) / (NIR + SWIR2)
            let s2_img = load_luma_band(&image, &swir2_name, "SWIR2", (width, height))?;
            for (x, y, pix) in out.enumerate_pixels_mut() {
                let n = nir_img.get_pixel(x, y)[0] as f32;
                let s2 = s2_img.get_pixel(x, y)[0] as f32;
                let denom = n + s2;
                let mut write_val = NODATA_F32;
                if valid_mask_at(&mask_img, x, y) && denom.abs() > f32::EPSILON {
                    let v = (n - s2) / denom;
                    write_val = v;
                    stats_min = stats_min.min(v);
                    stats_max = stats_max.max(v);
                    stats_sum += v as f64;
                    stats_count += 1;
                }
                if let Some(ref mut f32buf) = out_f32 {
                    f32buf[(y * width + x) as usize] = write_val;
                }
                *pix = image::Luma([if write_val.is_finite() {
                    ((write_val.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
                } else {
                    0
                }]);
            }
        }
        IndexKind::Ndmi => {
            // NDMI = (NIR - SWIR1) / (NIR + SWIR1)
            let s1_img = load_luma_band(&image, &swir1_name, "SWIR1", (width, height))?;
            for (x, y, pix) in out.enumerate_pixels_mut() {
                let n = nir_img.get_pixel(x, y)[0] as f32;
                let s1 = s1_img.get_pixel(x, y)[0] as f32;
                let denom = n + s1;
                let mut write_val = NODATA_F32;
                if valid_mask_at(&mask_img, x, y) && denom.abs() > f32::EPSILON {
                    let v = (n - s1) / denom;
                    write_val = v;
                    stats_min = stats_min.min(v);
                    stats_max = stats_max.max(v);
                    stats_sum += v as f64;
                    stats_count += 1;
                }
                if let Some(ref mut f32buf) = out_f32 {
                    f32buf[(y * width + x) as usize] = write_val;
                }
                *pix = image::Luma([if write_val.is_finite() {
                    ((write_val.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
                } else {
                    0
                }]);
            }
        }
        IndexKind::Evi2 => {
            // EVI2 = 2.5 * (NIR - R) / (NIR + 2.4*R + 1)
            for (x, y, pix) in out.enumerate_pixels_mut() {
                let r = red_img.get_pixel(x, y)[0] as f32;
                let n = nir_img.get_pixel(x, y)[0] as f32;
                let denom = n + 2.4 * r + 1.0;
                let mut write_val = NODATA_F32;
                if valid_mask_at(&mask_img, x, y) && denom.abs() > f32::EPSILON {
                    let v = 2.5 * (n - r) / denom;
                    write_val = v;
                    stats_min = stats_min.min(v);
                    stats_max = stats_max.max(v);
                    stats_sum += v as f64;
                    stats_count += 1;
                }
                if let Some(ref mut f32buf) = out_f32 {
                    f32buf[(y * width + x) as usize] = write_val;
                }
                *pix = image::Luma([if write_val.is_finite() {
                    ((write_val.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
                } else {
                    0
                }]);
            }
        }
        IndexKind::Savi => {
            // SAVI = (1+L)*(NIR-Red)/(NIR+Red+L), L typically 0.5
            let l = 0.5f32;
            for (x, y, pix) in out.enumerate_pixels_mut() {
                let valid_mask = valid_mask_at(&mask_img, x, y);
                let r = if let Some(ref rbuf) = red_f32_opt {
                    rbuf[(y * width + x) as usize]
                } else {
                    red_img.get_pixel(x, y)[0] as f32
                };
                let n = if let Some(ref nbuf) = nir_f32_opt {
                    nbuf[(y * width + x) as usize]
                } else {
                    nir_img.get_pixel(x, y)[0] as f32
                };
                let denom = n + r + l;
                let mut write_val = NODATA_F32;
                let nodata_ok = match nd_red {
                    Some(nd) => r != nd,
                    None => true,
                } && match nd_nir {
                    Some(nd) => n != nd,
                    None => true,
                };
                if valid_mask && nodata_ok && denom.abs() > f32::EPSILON {
                    let v = (1.0 + l) * (n - r) / denom;
                    write_val = v;
                    stats_min = stats_min.min(v);
                    stats_max = stats_max.max(v);
                    stats_sum += v as f64;
                    stats_count += 1;
                }
                if let Some(ref mut f32buf) = out_f32 {
                    f32buf[(y * width + x) as usize] = write_val;
                }
                let vis = if write_val.is_finite() {
                    ((write_val.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
                } else {
                    0
                };
                *pix = image::Luma([vis]);
            }
        }
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
                // Try to copy georeferencing from one of the source bands
                let src_ref = image
                    .file_paths
                    .get(&nir_name)
                    .or_else(|| image.file_paths.get(&red_name));
                if let Some(src) = src_ref {
                    let _ = crate::io::gdal_util::copy_geo_from(src, p.to_string_lossy().as_ref());
                }
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
        radiometric_calibration: ingest.evidence.radiometric_calibration.clone(),
        spatial_ref: ingest.evidence.spatial_ref.clone(),
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
