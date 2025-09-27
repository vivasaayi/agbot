use anyhow::Context;
use shared::{schemas::MultispectralImage, AgroResult};
use std::path::PathBuf;
use tracing::{error, info};

use crate::{OutputFormat, TemperatureUnit, ThermalArgs};

// Thermal pipeline (Phase 4)
// DN -> radiance (L = ML*DN + AL) -> brightness temperature (using K1,K2) -> emissivity-corrected LST
pub async fn run_thermal(args: &ThermalArgs) -> AgroResult<()> {
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

    info!(count = metadata_files.len(), "Found metadata files");

    for mf in metadata_files {
        if let Err(e) = process_one(&mf, args).await {
            error!(file=%mf.display(), error=%e, "Failed thermal processing");
        }
    }

    Ok(())
}

async fn process_one(metadata_file: &PathBuf, args: &ThermalArgs) -> AgroResult<()> {
    let metadata_content = tokio::fs::read_to_string(metadata_file).await?;
    let image: MultispectralImage = serde_json::from_str(&metadata_content)?;

    let t_path = image.file_paths.get(&args.thermal_band).ok_or_else(|| {
        shared::error::AgroError::Processing(format!(
            "Thermal band '{}' not found",
            args.thermal_band
        ))
    })?;

    // Read thermal band as 16-bit if available; fall back to 8-bit
    // Heuristic sensor preset detection (basic): infer Landsat 8/9 Band 10 or Sentinel TIR-like
    let is_landsat_b10 =
        args.thermal_band.eq_ignore_ascii_case("B10") || t_path.to_lowercase().contains("_b10");
    let is_landsat_b11 =
        args.thermal_band.eq_ignore_ascii_case("B11") || t_path.to_lowercase().contains("_b11");
    let dyn_img = image::open(t_path).map_err(|e| {
        shared::error::AgroError::Processing(format!("Failed to load thermal band: {}", e))
    })?;
    let is_u16 = matches!(
        dyn_img.color(),
        image::ColorType::L16
            | image::ColorType::La16
            | image::ColorType::Rgb16
            | image::ColorType::Rgba16
    );
    let t_img_u16 = if is_u16 {
        Some(dyn_img.to_luma16())
    } else {
        None
    };
    let t_img_u8 = if is_u16 {
        None
    } else {
        Some(dyn_img.to_luma8())
    };
    let (w, h) = if let Some(ref im) = t_img_u16 {
        im.dimensions()
    } else {
        t_img_u8.as_ref().unwrap().dimensions()
    };
    let mut out_vis = image::GrayImage::new(w, h);

    // Configure calibration params
    // Defaults from Landsat 8/9 Collection 2 (typical example values; real scenes vary per metadata)
    let (ml_def, al_def, k1_def, k2_def, lambda_def) = if is_landsat_b11 {
        (0.0003342, 0.1, 480.8883, 1201.1442, 12.00) // rough example for B11
    } else if is_landsat_b10 {
        (0.0003342, 0.1, 774.8853, 1321.0789, 10.895)
    } else {
        // Generic defaults
        (
            args.ml.unwrap_or(0.0003342),
            args.al.unwrap_or(0.1),
            args.k1.unwrap_or(774.8853),
            args.k2.unwrap_or(1321.0789),
            args.lambda_um,
        )
    };
    // Use user overrides if provided, otherwise sensor defaults
    let ml = args.ml.unwrap_or(ml_def);
    let al = args.al.unwrap_or(al_def);
    let k1 = args.k1.unwrap_or(k1_def);
    let k2 = args.k2.unwrap_or(k2_def);
    let eps = args.emissivity.clamp(0.8, 1.0);
    let lambda = if is_landsat_b10 || is_landsat_b11 {
        lambda_def
    } else {
        args.lambda_um
    }; // micrometers

    // Precompute emissivity correction term using Planck's law approximation
    // LST = TB / (1 + (lambda * TB / rho) * ln(eps))
    // where rho = h*c/sigma ≈ 1.4388e-2 m*K; since lambda in um, convert: lambda_m = lambda_um * 1e-6
    let rho = 1.4388e-2f64; // m*K
    let lambda_m = (lambda as f64) * 1e-6;
    let mut min_t = f32::INFINITY;
    let mut max_t = f32::NEG_INFINITY;
    let mut sum_t = 0.0f64;

    // Optional mask: non-zero means valid pixel
    let mask_img = if let Some(mask_path) = &args.mask {
        Some(
            image::open(mask_path)
                .map_err(|e| {
                    shared::error::AgroError::Processing(format!("Failed to load mask: {}", e))
                })?
                .to_luma8(),
        )
    } else {
        None
    };

    // Float32 buffer for GeoTIFF
    let mut out_f32: Vec<f32> = vec![f32::NAN; (w as usize) * (h as usize)];
    let mut valid_count: usize = 0;

    for y in 0..h {
        for x in 0..w {
            if !mask_img
                .as_ref()
                .map_or(true, |m| m.get_pixel(x, y)[0] != 0)
            {
                out_vis.put_pixel(x, y, image::Luma([0]));
                continue;
            }
            let dn: f32 = if let Some(ref im16) = t_img_u16 {
                im16.get_pixel(x, y)[0] as f32
            } else {
                t_img_u8.as_ref().unwrap().get_pixel(x, y)[0] as f32
            };
            let dn_max = if t_img_u16.is_some() { 65535.0 } else { 255.0 };
            // Radiance
            let l = ml * dn + al; // W/(m^2*sr*um)
                                  // Brightness temperature from radiance using K1, K2: TB = K2 / ln(K1/L + 1)
            let tb = if l > 0.0 {
                k2 / ((k1 / l).ln_1p())
            } else {
                f32::NAN
            };
            // Emissivity correction to LST
            let tb_f64 = tb as f64;
            let lst_k = (tb_f64 / (1.0 + (lambda_m * tb_f64 / rho) * (eps as f64).ln())) as f32;
            if lst_k.is_finite() {
                min_t = min_t.min(lst_k);
                max_t = max_t.max(lst_k);
                sum_t += lst_k as f64;
                valid_count += 1;
            }
            out_f32[(y as usize) * (w as usize) + (x as usize)] = lst_k;
            // Visualization: scale to 0..255 around reasonable range
            let vis = if t_img_u16.is_some() {
                // assume 250..330K typical
                (((lst_k - 250.0) / 80.0) * 255.0).clamp(0.0, 255.0) as u8
            } else {
                // 8-bit likely already scaled 0..255
                (dn / dn_max * 255.0).clamp(0.0, 255.0) as u8
            };
            out_vis.put_pixel(x, y, image::Luma([vis]));
        }
    }

    let mean_t = if valid_count > 0 {
        (sum_t / valid_count as f64) as f32
    } else {
        f32::NAN
    };

    let out_path = match args.out_format {
        OutputFormat::Png => {
            let p = args.output_dir.join(format!(
                "thermal_{}_{}.png",
                image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
                image.image_id
            ));
            out_vis.save(&p).map_err(|e| {
                shared::error::AgroError::Processing(format!("Failed to save thermal image: {}", e))
            })?;
            p
        }
        OutputFormat::Geotiff => {
            #[cfg(feature = "gdal-io")]
            {
                let p = args.output_dir.join(format!(
                    "thermal_{}_{}.tif",
                    image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
                    image.image_id
                ));
                // Write float32 GeoTIFF with nodata as NaN or -9999
                crate::io::gdal_util::write_f32_geotiff(
                    p.to_string_lossy().as_ref(),
                    &out_f32,
                    w as usize,
                    h as usize,
                    Some(-9999.0),
                )
                .map_err(|e| {
                    shared::error::AgroError::Processing(format!("Create GeoTIFF failed: {}", e))
                })?;
                // Try to copy georeferencing from thermal source band
                let _ = crate::io::gdal_util::copy_geo_from(t_path, p.to_string_lossy().as_ref());
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

    // Unit conversion for summary (output GeoTIFF remains Kelvin internally)
    let (min_out, max_out, mean_out) = match args.unit {
        TemperatureUnit::Kelvin => (min_t, max_t, mean_t),
        TemperatureUnit::Celsius => (min_t - 273.15, max_t - 273.15, mean_t - 273.15),
    };

    let meta = serde_json::json!({
        "timestamp": chrono::Utc::now(),
        "source_images": [image.image_id],
        "output_path": out_path.to_string_lossy(),
        "unit": format!("{:?}", args.unit).to_lowercase(),
        "min": min_out,
        "max": max_out,
        "mean": mean_out,
        "emissivity": eps,
        "ml": ml,
        "al": al,
        "k1": k1,
        "k2": k2,
        "lambda_um": lambda,
    });

    let meta_path = args.output_dir.join(format!(
        "thermal_result_{}_{}.json",
        image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
        image.image_id
    ));
    tokio::fs::write(meta_path, serde_json::to_string_pretty(&meta)?).await?;

    Ok(())
}
