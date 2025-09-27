use std::path::PathBuf;
use anyhow::Context;
use shared::{schemas::MultispectralImage, AgroResult};
use tracing::{info, error};

use crate::{MasksArgs, MaskKind, OutputFormat};

// Landsat Collection 2 QA_PIXEL bits (subset):
// bit 0: Fill; 1: Dilated Cloud; 2: Cirrus; 3: Cloud; 4: Cloud Shadow; 5: Snow; 7: Water
fn qa_pixel_flags(v: u16) -> (bool, bool, bool, bool, bool) {
    let cloud = (v & (1 << 3)) != 0 || (v & (1 << 1)) != 0; // cloud or dilated cloud
    let cloud_shadow = (v & (1 << 4)) != 0;
    let snow = (v & (1 << 5)) != 0;
    let water = (v & (1 << 7)) != 0;
    let clear = !cloud && !cloud_shadow && !snow; // ignore water in clear definition
    (cloud, cloud_shadow, snow, water, clear)
}

pub async fn run_masks(args: &MasksArgs) -> AgroResult<()> {
    tokio::fs::create_dir_all(&args.output_dir).await?;

    let mut metadata_files = Vec::new();
    for entry in walkdir::WalkDir::new(&args.input_dir) {
        let entry = entry.context("walkdir")?;
        if entry.file_name().to_string_lossy().starts_with("metadata_") &&
           entry.path().extension().map_or(false, |ext| ext == "json") {
            metadata_files.push(entry.path().to_path_buf());
        }
    }

    info!(count = metadata_files.len(), "Found metadata files for masks");

    for mf in metadata_files {
        if let Err(e) = process_one(&mf, args).await {
            error!(file=%mf.display(), error=%e, "Failed masks processing");
        }
    }

    Ok(())
}

async fn process_one(metadata_file: &PathBuf, args: &MasksArgs) -> AgroResult<()> {
    let metadata_content = tokio::fs::read_to_string(metadata_file).await?;
    let image: MultispectralImage = serde_json::from_str(&metadata_content)?;

    let qa_path = image.file_paths.get(&args.qa_band)
        .ok_or_else(|| shared::error::AgroError::Processing(format!("QA band '{}' not found", args.qa_band)))?;

    // Read QA as u16; if TIFF+GDAL, use GDAL, otherwise image crate fallback
    #[cfg(feature = "gdal-io")]
    let use_gdal = qa_path.to_lowercase().ends_with(".tif") || qa_path.to_lowercase().ends_with(".tiff");
    #[cfg(not(feature = "gdal-io"))]
    let use_gdal = false;

    let (w, h, qa_u16): (u32, u32, Vec<u16>) = if use_gdal {
        #[cfg(feature = "gdal-io")]
        {
            let (w, h, buf_f32, _nd) = crate::io::gdal_util::read_first_band_as_f32(qa_path)
                .map_err(|e| shared::error::AgroError::Processing(format!("GDAL read QA failed: {}", e)))?;
            let mut v = vec![0u16; w*h];
            for i in 0..(w*h) { v[i] = buf_f32[i] as u16; }
            (w as u32, h as u32, v)
        }
        #[cfg(not(feature = "gdal-io"))]
        { unreachable!() }
    } else {
        let dyn_img = image::open(qa_path)
            .map_err(|e| shared::error::AgroError::Processing(format!("Failed to load QA image: {}", e)))?;
        let g = dyn_img.to_luma16();
        let (w, h) = g.dimensions();
        let mut v = vec![0u16; (w*h) as usize];
        for y in 0..h { for x in 0..w { v[(y*w+x) as usize] = g.get_pixel(x,y)[0]; }}
        (w, h, v)
    };

    let kinds = if args.kinds.is_empty() {
        vec![MaskKind::Cloud, MaskKind::CloudShadow, MaskKind::Snow, MaskKind::Water, MaskKind::Clear]
    } else { args.kinds.clone() };

    for kind in kinds {
        let mut mask = image::GrayImage::new(w, h);
        for y in 0..h { for x in 0..w {
            let v = qa_u16[(y*w + x) as usize];
            let (cloud, shadow, snow, water, clear) = qa_pixel_flags(v);
            let on = match kind {
                MaskKind::Cloud => cloud,
                MaskKind::CloudShadow => shadow,
                MaskKind::Snow => snow,
                MaskKind::Water => water,
                MaskKind::Clear => clear,
            };
            mask.put_pixel(x, y, image::Luma([if on { 255 } else { 0 }]));
        }}

        let name = format!(
            "mask_{}_{}_{:?}",
            image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
            image.image_id,
            kind
        ).to_lowercase();

        match args.out_format {
            OutputFormat::Png => {
                let p = args.output_dir.join(format!("{}.png", name));
                mask.save(&p)
                    .map_err(|e| shared::error::AgroError::Processing(format!("Save mask failed: {}", e)))?;
            }
            OutputFormat::Geotiff => {
                #[cfg(feature = "gdal-io")]
                {
                    let p = args.output_dir.join(format!("{}.tif", name));
                    crate::io::gdal_util::write_u8_geotiff_basic(p.to_string_lossy().as_ref(), mask.as_raw(), w as usize, h as usize)
                        .map_err(|e| shared::error::AgroError::Processing(format!("Create GeoTIFF failed: {}", e)))?;
                    let _ = crate::io::gdal_util::copy_geo_from(qa_path, p.to_string_lossy().as_ref());
                }
                #[cfg(not(feature = "gdal-io"))]
                {
                    return Err(shared::error::AgroError::Processing("Geotiff output requested but gdal-io feature is not enabled".into()));
                }
            }
        }
    }

    Ok(())
}
