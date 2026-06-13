use anyhow::Context;
use shared::{
    schemas::{assert_raster_spatial_ref, MultispectralImage},
    AgroResult,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tracing::{error, info};

use crate::{MaskKind, MasksArgs, OutputFormat};

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
        if entry.file_name().to_string_lossy().starts_with("metadata_")
            && entry.path().extension().map_or(false, |ext| ext == "json")
        {
            metadata_files.push(entry.path().to_path_buf());
        }
    }

    info!(
        count = metadata_files.len(),
        "Found metadata files for masks"
    );

    let mut failures = Vec::new();
    for mf in metadata_files {
        if let Err(e) = process_one(&mf, args).await {
            error!(file=%mf.display(), error=%e, "Failed masks processing");
            failures.push(format!("{}: {}", mf.display(), e));
        }
    }

    if !failures.is_empty() {
        return Err(shared::error::AgroError::Processing(format!(
            "{} metadata file(s) failed: {}",
            failures.len(),
            failures.join("; ")
        )));
    }

    Ok(())
}

#[derive(Debug, serde::Serialize)]
struct MaskEvidence {
    image_id: uuid::Uuid,
    qa_band: String,
    width: u32,
    height: u32,
    class_counts: BTreeMap<String, u32>,
    outputs: BTreeMap<String, String>,
    reproducibility: crate::io::ProductReproducibilityEvidence,
}

async fn process_one(metadata_file: &PathBuf, args: &MasksArgs) -> AgroResult<()> {
    let metadata_content = tokio::fs::read_to_string(metadata_file).await?;
    let image: MultispectralImage = serde_json::from_str(&metadata_content)?;

    let qa_path = image.file_paths.get(&args.qa_band).ok_or_else(|| {
        shared::error::AgroError::Processing(format!("QA band '{}' not found", args.qa_band))
    })?;

    // Read QA as u16; if TIFF+GDAL, use GDAL, otherwise image crate fallback
    #[cfg(feature = "gdal-io")]
    let use_gdal =
        qa_path.to_lowercase().ends_with(".tif") || qa_path.to_lowercase().ends_with(".tiff");
    #[cfg(not(feature = "gdal-io"))]
    let use_gdal = false;

    let (w, h, qa_u16): (u32, u32, Vec<u16>) = if use_gdal {
        #[cfg(feature = "gdal-io")]
        {
            let (w, h, buf_f32, _nd) = crate::io::gdal_util::read_first_band_as_f32(qa_path)
                .map_err(|e| {
                    shared::error::AgroError::Processing(format!("GDAL read QA failed: {}", e))
                })?;
            let mut v = vec![0u16; w * h];
            for i in 0..(w * h) {
                v[i] = buf_f32[i] as u16;
            }
            (w as u32, h as u32, v)
        }
        #[cfg(not(feature = "gdal-io"))]
        {
            unreachable!()
        }
    } else {
        let dyn_img = image::open(qa_path).map_err(|e| {
            shared::error::AgroError::Processing(format!("Failed to load QA image: {}", e))
        })?;
        let g = dyn_img.to_luma16();
        let (w, h) = g.dimensions();
        let mut v = vec![0u16; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                v[(y * w + x) as usize] = g.get_pixel(x, y)[0];
            }
        }
        (w, h, v)
    };
    let spatial_ref = assert_raster_spatial_ref(image.metadata.spatial_ref.as_ref(), w, h).ok();

    let kinds = if args.kinds.is_empty() {
        vec![
            MaskKind::Cloud,
            MaskKind::CloudShadow,
            MaskKind::Snow,
            MaskKind::Water,
            MaskKind::Clear,
        ]
    } else {
        args.kinds.clone()
    };
    let selected_kinds = kinds
        .iter()
        .map(|kind| mask_kind_key(*kind))
        .collect::<Vec<_>>();

    let class_counts = count_qa_classes(&qa_u16);
    let mut outputs = BTreeMap::new();

    for kind in kinds {
        let mut mask = image::GrayImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let v = qa_u16[(y * w + x) as usize];
                let (cloud, shadow, snow, water, clear) = qa_pixel_flags(v);
                let on = match kind {
                    MaskKind::Cloud => cloud,
                    MaskKind::CloudShadow => shadow,
                    MaskKind::Snow => snow,
                    MaskKind::Water => water,
                    MaskKind::Clear => clear,
                };
                mask.put_pixel(x, y, image::Luma([if on { 255 } else { 0 }]));
            }
        }

        let name = format!(
            "mask_{}_{}_{}",
            image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
            image.image_id,
            mask_kind_key(kind)
        )
        .to_lowercase();

        let output_path = match args.out_format {
            OutputFormat::Png => {
                let p = args.output_dir.join(format!("{}.png", name));
                mask.save(&p).map_err(|e| {
                    shared::error::AgroError::Processing(format!("Save mask failed: {}", e))
                })?;
                crate::io::write_png_spatial_sidecar(&p, spatial_ref.as_ref()).await?;
                p
            }
            OutputFormat::Geotiff => {
                #[cfg(feature = "gdal-io")]
                {
                    let p = args.output_dir.join(format!("{}.tif", name));
                    crate::io::gdal_util::write_u8_geotiff_basic(
                        p.to_string_lossy().as_ref(),
                        mask.as_raw(),
                        w as usize,
                        h as usize,
                    )
                    .map_err(|e| {
                        shared::error::AgroError::Processing(format!(
                            "Create GeoTIFF failed: {}",
                            e
                        ))
                    })?;
                    let _ =
                        crate::io::gdal_util::copy_geo_from(qa_path, p.to_string_lossy().as_ref());
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
        outputs.insert(
            mask_kind_key(kind).to_string(),
            output_path.to_string_lossy().to_string(),
        );
    }

    let mut output_hashes = BTreeMap::new();
    for (kind, output_path) in &outputs {
        output_hashes.insert(
            kind.clone(),
            crate::io::file_output_hash(PathBuf::from(output_path).as_path()).await?,
        );
    }
    let total_pixel_count = (w as usize) * (h as usize);
    let clear_pixel_count = class_counts.get("clear").copied().unwrap_or_default() as usize;
    let clear_pixel_coverage = if total_pixel_count == 0 {
        0.0
    } else {
        clear_pixel_count as f32 / total_pixel_count as f32
    };
    let reproducibility = crate::io::ProductReproducibilityEvidence::new(
        vec![image.image_id],
        "mask",
        serde_json::json!({
            "qa_band": args.qa_band.clone(),
            "out_format": format!("{:?}", args.out_format).to_lowercase(),
            "kinds": selected_kinds,
        }),
        None,
        None,
        serde_json::json!({
            "class_counts": class_counts.clone(),
        }),
        serde_json::json!({
            "total_pixel_count": total_pixel_count,
            "clear_pixel_count": clear_pixel_count,
            "clear_pixel_coverage": clear_pixel_coverage,
        }),
        output_hashes,
    );

    let evidence = MaskEvidence {
        image_id: image.image_id,
        qa_band: args.qa_band.clone(),
        width: w,
        height: h,
        class_counts,
        outputs,
        reproducibility,
    };
    let evidence_path = args
        .output_dir
        .join(format!("mask_evidence_{}.json", image.image_id));
    tokio::fs::write(evidence_path, serde_json::to_vec_pretty(&evidence)?).await?;

    Ok(())
}

fn count_qa_classes(qa_u16: &[u16]) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::from([
        ("cloud".to_string(), 0),
        ("cloud_shadow".to_string(), 0),
        ("snow".to_string(), 0),
        ("water".to_string(), 0),
        ("clear".to_string(), 0),
    ]);

    for value in qa_u16 {
        let (cloud, cloud_shadow, snow, water, clear) = qa_pixel_flags(*value);
        if cloud {
            *counts.get_mut("cloud").unwrap() += 1;
        }
        if cloud_shadow {
            *counts.get_mut("cloud_shadow").unwrap() += 1;
        }
        if snow {
            *counts.get_mut("snow").unwrap() += 1;
        }
        if water {
            *counts.get_mut("water").unwrap() += 1;
        }
        if clear {
            *counts.get_mut("clear").unwrap() += 1;
        }
    }

    counts
}

fn mask_kind_key(kind: MaskKind) -> &'static str {
    match kind {
        MaskKind::Cloud => "cloud",
        MaskKind::CloudShadow => "cloud_shadow",
        MaskKind::Snow => "snow",
        MaskKind::Water => "water",
        MaskKind::Clear => "clear",
    }
}
