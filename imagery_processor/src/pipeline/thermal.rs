use anyhow::Context;
use shared::{
    error::AgroError,
    schemas::{assert_raster_spatial_ref, MultispectralImage},
    AgroResult,
};
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

    let mut failures = Vec::new();
    for mf in metadata_files {
        if let Err(e) = process_one(&mf, args).await {
            error!(file=%mf.display(), error=%e, "Failed thermal processing");
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

#[derive(Debug, Clone, Copy)]
struct RunningStats {
    min: f32,
    max: f32,
    sum: f64,
    count: usize,
}

impl RunningStats {
    fn empty() -> Self {
        Self {
            min: f32::INFINITY,
            max: f32::NEG_INFINITY,
            sum: 0.0,
            count: 0,
        }
    }

    fn record(&mut self, value: f32) {
        if value.is_finite() {
            self.min = self.min.min(value);
            self.max = self.max.max(value);
            self.sum += value as f64;
            self.count += 1;
        }
    }

    fn mean(self) -> f32 {
        if self.count > 0 {
            (self.sum / self.count as f64) as f32
        } else {
            f32::NAN
        }
    }

    fn min_value(self) -> f32 {
        if self.count > 0 {
            self.min
        } else {
            f32::NAN
        }
    }

    fn max_value(self) -> f32 {
        if self.count > 0 {
            self.max
        } else {
            f32::NAN
        }
    }

    fn with_offset(self, offset: f32) -> Self {
        if self.count > 0 {
            Self {
                min: self.min + offset,
                max: self.max + offset,
                sum: self.sum + offset as f64 * self.count as f64,
                count: self.count,
            }
        } else {
            self
        }
    }
}

fn processing_error(message: impl Into<String>) -> AgroError {
    AgroError::Processing(message.into())
}

fn require_thermal_coefficient(name: &'static str, value: Option<f32>) -> AgroResult<f32> {
    value.ok_or_else(|| processing_error(format!("thermal coefficient '{name}' is required")))
}

fn stats_json(stats: RunningStats) -> serde_json::Value {
    serde_json::json!({
        "min": stats.min_value(),
        "max": stats.max_value(),
        "mean": stats.mean(),
        "count": stats.count,
    })
}

async fn process_one(metadata_file: &PathBuf, args: &ThermalArgs) -> AgroResult<()> {
    let metadata_content = tokio::fs::read_to_string(metadata_file).await?;
    let image: MultispectralImage = serde_json::from_str(&metadata_content)?;
    let spatial_ref = assert_raster_spatial_ref(
        image.metadata.spatial_ref.as_ref(),
        image.metadata.width,
        image.metadata.height,
    )
    .map_err(|err| processing_error(err.to_string()))?;

    let t_path = image.file_paths.get(&args.thermal_band).ok_or_else(|| {
        processing_error(format!("Thermal band '{}' not found", args.thermal_band))
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
    if (w, h) != (image.metadata.width, image.metadata.height) {
        return Err(processing_error(format!(
            "Thermal band dimensions mismatch: expected {}x{}, got {}x{}",
            image.metadata.width, image.metadata.height, w, h
        )));
    }
    let mut out_vis = image::GrayImage::new(w, h);

    let ml = require_thermal_coefficient("ml", args.ml)?;
    let al = require_thermal_coefficient("al", args.al)?;
    let k1 = require_thermal_coefficient("k1", args.k1)?;
    let k2 = require_thermal_coefficient("k2", args.k2)?;
    let eps = args.emissivity.clamp(0.8, 1.0);
    let lambda = if is_landsat_b11 {
        12.00
    } else if is_landsat_b10 {
        10.895
    } else {
        args.lambda_um
    }; // micrometers

    // Precompute emissivity correction term using Planck's law approximation
    // LST = TB / (1 + (lambda * TB / rho) * ln(eps))
    // where rho = h*c/sigma ≈ 1.4388e-2 m*K; since lambda in um, convert: lambda_m = lambda_um * 1e-6
    let rho = 1.4388e-2f64; // m*K
    let lambda_m = (lambda as f64) * 1e-6;
    let mut radiance_stats = RunningStats::empty();
    let mut bt_stats = RunningStats::empty();
    let mut lst_stats = RunningStats::empty();

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
            radiance_stats.record(l);
            // Brightness temperature from radiance using K1, K2: TB = K2 / ln(K1/L + 1)
            let tb = if l > 0.0 {
                k2 / ((k1 / l).ln_1p())
            } else {
                f32::NAN
            };
            bt_stats.record(tb);
            // Emissivity correction to LST
            let tb_f64 = tb as f64;
            let lst_k = (tb_f64 / (1.0 + (lambda_m * tb_f64 / rho) * (eps as f64).ln())) as f32;
            if lst_k.is_finite() {
                lst_stats.record(lst_k);
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
                crate::io::gdal_util::apply_spatial_ref(p.to_string_lossy().as_ref(), &spatial_ref)
                    .map_err(|e| {
                        processing_error(format!("Apply GeoTIFF spatial reference failed: {}", e))
                    })?;
                crate::io::write_geotiff_spatial_sidecar(&p, &spatial_ref).await?;
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
    let lst_output_stats = match args.unit {
        TemperatureUnit::Kelvin => lst_stats,
        TemperatureUnit::Celsius => lst_stats.with_offset(-273.15),
    };

    let meta = serde_json::json!({
        "timestamp": chrono::Utc::now(),
        "source_images": [image.image_id],
        "output_path": out_path.to_string_lossy(),
        "unit": format!("{:?}", args.unit).to_lowercase(),
        "min": lst_output_stats.min_value(),
        "max": lst_output_stats.max_value(),
        "mean": lst_output_stats.mean(),
        "valid_pixel_count": valid_count,
        "radiance": stats_json(radiance_stats),
        "brightness_temperature": stats_json(bt_stats),
        "lst": stats_json(lst_output_stats),
        "emissivity": eps,
        "thermal_coefficients": {
            "ml": ml,
            "al": al,
            "k1": k1,
            "k2": k2,
            "lambda_um": lambda,
        },
        "spatial_ref": spatial_ref,
    });

    let meta_path = args.output_dir.join(format!(
        "thermal_result_{}_{}.json",
        image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
        image.image_id
    ));
    tokio::fs::write(meta_path, serde_json::to_string_pretty(&meta)?).await?;

    Ok(())
}
