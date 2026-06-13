use anyhow::Context;
use shared::{
    error::AgroError,
    schemas::{assert_raster_spatial_ref, MultispectralImage},
    AgroResult,
};
use std::path::PathBuf;
use tracing::{error, info};

use crate::{OutputFormat, TemperatureUnit, ThermalArgs};

const NDVI_SOIL_THRESHOLD: f32 = 0.2;
const NDVI_VEGETATION_THRESHOLD: f32 = 0.5;
const SOIL_EMISSIVITY: f32 = 0.97;
const VEGETATION_EMISSIVITY: f32 = 0.99;

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

#[derive(Debug, Clone)]
struct ThermalBandData {
    band_name: String,
    path: String,
    width: u32,
    height: u32,
    values: Vec<f32>,
    is_u16: bool,
}

impl ThermalBandData {
    fn value_at(&self, index: usize) -> f32 {
        self.values[index]
    }
}

#[derive(Debug, Clone)]
struct EmissivityPlan {
    method: &'static str,
    source: String,
    values: Vec<f32>,
    stats: RunningStats,
}

impl EmissivityPlan {
    fn constant(value: f32, pixel_count: usize) -> Self {
        let value = value.clamp(0.8, 1.0);
        let mut stats = RunningStats::empty();
        let values = vec![value; pixel_count];
        for _ in 0..pixel_count {
            stats.record(value);
        }
        Self {
            method: "constant",
            source: "thermal_args".to_string(),
            values,
            stats,
        }
    }

    fn from_values(source: String, values: Vec<f32>) -> Self {
        let mut stats = RunningStats::empty();
        for value in &values {
            stats.record(*value);
        }
        Self {
            method: "ndvi_thresholds",
            source,
            values,
            stats,
        }
    }

    fn value_at(&self, index: usize) -> f32 {
        self.values[index]
    }
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

fn load_gray_values(
    path: &str,
    expected_dimensions: (u32, u32),
    role: &str,
) -> AgroResult<(Vec<f32>, bool)> {
    let dyn_img = image::open(path)
        .map_err(|e| processing_error(format!("Failed to load {role} image: {e}")))?;
    let is_u16 = matches!(
        dyn_img.color(),
        image::ColorType::L16
            | image::ColorType::La16
            | image::ColorType::Rgb16
            | image::ColorType::Rgba16
    );

    if is_u16 {
        let image = dyn_img.to_luma16();
        if image.dimensions() != expected_dimensions {
            return Err(processing_error(format!(
                "{role} dimensions mismatch: expected {:?}, got {:?}",
                expected_dimensions,
                image.dimensions()
            )));
        }
        Ok((image.pixels().map(|pixel| pixel[0] as f32).collect(), true))
    } else {
        let image = dyn_img.to_luma8();
        if image.dimensions() != expected_dimensions {
            return Err(processing_error(format!(
                "{role} dimensions mismatch: expected {:?}, got {:?}",
                expected_dimensions,
                image.dimensions()
            )));
        }
        Ok((image.pixels().map(|pixel| pixel[0] as f32).collect(), false))
    }
}

fn load_thermal_band_data(
    image: &MultispectralImage,
    band_name: &str,
) -> AgroResult<ThermalBandData> {
    let path = require_band_path(image, band_name, "Thermal")?;
    let expected_dimensions = (image.metadata.width, image.metadata.height);
    let (values, is_u16) = load_gray_values(path, expected_dimensions, "Thermal band")?;
    Ok(ThermalBandData {
        band_name: band_name.to_string(),
        path: path.to_string(),
        width: expected_dimensions.0,
        height: expected_dimensions.1,
        values,
        is_u16,
    })
}

fn load_ndvi_image_emissivity(
    path: &PathBuf,
    expected_dimensions: (u32, u32),
) -> AgroResult<Vec<f32>> {
    let image = image::open(path)
        .map_err(|e| processing_error(format!("Failed to load NDVI image: {e}")))?
        .to_luma8();
    if image.dimensions() != expected_dimensions {
        return Err(processing_error(format!(
            "NDVI image dimensions mismatch: expected {:?}, got {:?}",
            expected_dimensions,
            image.dimensions()
        )));
    }

    Ok(image
        .pixels()
        .map(|pixel| {
            let ndvi = (pixel[0] as f32 / 255.0) * 2.0 - 1.0;
            emissivity_from_ndvi(ndvi)
        })
        .collect())
}

fn load_band_ndvi_emissivity(
    image: &MultispectralImage,
    red_band: &str,
    nir_band: &str,
) -> AgroResult<Vec<f32>> {
    let expected_dimensions = (image.metadata.width, image.metadata.height);
    let red_path = require_band_path(image, red_band, "Red")?;
    let nir_path = require_band_path(image, nir_band, "NIR")?;
    let (red, _) = load_gray_values(red_path, expected_dimensions, "Red band")?;
    let (nir, _) = load_gray_values(nir_path, expected_dimensions, "NIR band")?;

    Ok(red
        .iter()
        .zip(nir.iter())
        .map(|(red, nir)| {
            let denominator = red + nir;
            if denominator.abs() <= f32::EPSILON {
                emissivity_from_ndvi(f32::NAN)
            } else {
                emissivity_from_ndvi((nir - red) / denominator)
            }
        })
        .collect())
}

fn emissivity_from_ndvi(ndvi: f32) -> f32 {
    if !ndvi.is_finite() || ndvi <= NDVI_SOIL_THRESHOLD {
        SOIL_EMISSIVITY
    } else if ndvi >= NDVI_VEGETATION_THRESHOLD {
        VEGETATION_EMISSIVITY
    } else {
        let vegetation_fraction = ((ndvi - NDVI_SOIL_THRESHOLD)
            / (NDVI_VEGETATION_THRESHOLD - NDVI_SOIL_THRESHOLD))
            .powi(2);
        SOIL_EMISSIVITY + vegetation_fraction * (VEGETATION_EMISSIVITY - SOIL_EMISSIVITY)
    }
}

fn build_emissivity_plan(
    image: &MultispectralImage,
    args: &ThermalArgs,
    pixel_count: usize,
) -> AgroResult<EmissivityPlan> {
    if !args.emissivity_from_ndvi {
        return Ok(EmissivityPlan::constant(args.emissivity, pixel_count));
    }

    if let Some(ndvi_image) = &args.ndvi_image {
        return Ok(EmissivityPlan::from_values(
            "ndvi_image".to_string(),
            load_ndvi_image_emissivity(ndvi_image, (image.metadata.width, image.metadata.height))?,
        ));
    }

    let red = args.red.as_deref().ok_or_else(|| {
        processing_error("NDVI emissivity requested without --ndvi-image or --red/--nir bands")
    })?;
    let nir = args.nir.as_deref().ok_or_else(|| {
        processing_error("NDVI emissivity requested without --ndvi-image or --red/--nir bands")
    })?;
    Ok(EmissivityPlan::from_values(
        "computed_from_bands".to_string(),
        load_band_ndvi_emissivity(image, red, nir)?,
    ))
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

    let primary_band = load_thermal_band_data(&image, &args.thermal_band)?;
    let is_landsat_b10 = primary_band.band_name.eq_ignore_ascii_case("B10")
        || primary_band.path.to_lowercase().contains("_b10");
    let is_landsat_b11 = primary_band.band_name.eq_ignore_ascii_case("B11")
        || primary_band.path.to_lowercase().contains("_b11");
    let split_band = if args.split_window {
        args.thermal_band2
            .as_deref()
            .map(|band_name| load_thermal_band_data(&image, band_name))
            .transpose()?
    } else {
        None
    };
    let split_fallback_reason =
        (args.split_window && split_band.is_none()).then_some("second thermal band not provided");
    let thermal_method = if split_band.is_some() {
        "split_window"
    } else {
        "single_channel"
    };
    let (w, h) = (primary_band.width, primary_band.height);
    let mut out_vis = image::GrayImage::new(w, h);

    let ml = require_thermal_coefficient("ml", args.ml)?;
    let al = require_thermal_coefficient("al", args.al)?;
    let k1 = require_thermal_coefficient("k1", args.k1)?;
    let k2 = require_thermal_coefficient("k2", args.k2)?;
    let emissivity_plan = build_emissivity_plan(&image, args, (w as usize) * (h as usize))?;
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
    let mut split_delta_stats = RunningStats::empty();

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
            let index = (y as usize) * (w as usize) + (x as usize);
            if !mask_img
                .as_ref()
                .map_or(true, |m| m.get_pixel(x, y)[0] != 0)
            {
                out_vis.put_pixel(x, y, image::Luma([0]));
                continue;
            }
            let dn = primary_band.value_at(index);
            let dn_max = if primary_band.is_u16 { 65535.0 } else { 255.0 };
            // Radiance
            let l = ml * dn + al; // W/(m^2*sr*um)
            radiance_stats.record(l);
            // Brightness temperature from radiance using K1, K2: TB = K2 / ln(K1/L + 1)
            let tb = if l > 0.0 {
                k2 / ((k1 / l).ln_1p())
            } else {
                f32::NAN
            };
            let tb_for_lst = if let Some(split_band) = &split_band {
                let dn2 = split_band.value_at(index);
                let l2 = ml * dn2 + al;
                let tb2 = if l2 > 0.0 {
                    k2 / ((k1 / l2).ln_1p())
                } else {
                    f32::NAN
                };
                if tb.is_finite() && tb2.is_finite() {
                    split_delta_stats.record(tb - tb2);
                    (tb + tb2) / 2.0
                } else {
                    tb
                }
            } else {
                tb
            };
            bt_stats.record(tb_for_lst);
            // Emissivity correction to LST
            let eps = emissivity_plan.value_at(index);
            let tb_f64 = tb_for_lst as f64;
            let lst_k = (tb_f64 / (1.0 + (lambda_m * tb_f64 / rho) * (eps as f64).ln())) as f32;
            if lst_k.is_finite() {
                lst_stats.record(lst_k);
                valid_count += 1;
            }
            out_f32[index] = lst_k;
            // Visualization: scale to 0..255 around reasonable range
            let vis = if primary_band.is_u16 {
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
            crate::io::write_png_spatial_sidecar(&p, Some(&spatial_ref)).await?;
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
        "emissivity": emissivity_plan.stats.mean(),
        "emissivity_evidence": {
            "method": emissivity_plan.method,
            "source": emissivity_plan.source,
            "min": emissivity_plan.stats.min_value(),
            "max": emissivity_plan.stats.max_value(),
            "mean": emissivity_plan.stats.mean(),
            "count": emissivity_plan.stats.count,
            "ndvi_thresholds": {
                "soil": NDVI_SOIL_THRESHOLD,
                "vegetation": NDVI_VEGETATION_THRESHOLD,
                "soil_emissivity": SOIL_EMISSIVITY,
                "vegetation_emissivity": VEGETATION_EMISSIVITY,
            }
        },
        "thermal_method": {
            "method": thermal_method,
            "primary_band": args.thermal_band,
            "second_band": args.thermal_band2,
            "fallback_reason": split_fallback_reason,
            "split_window_delta": stats_json(split_delta_stats),
        },
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
