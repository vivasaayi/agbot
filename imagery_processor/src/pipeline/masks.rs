use anyhow::Context;
use shared::{
    schemas::{assert_raster_spatial_ref, MultispectralImage},
    AgroResult,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tracing::{error, info};

use crate::{MaskKind, MasksArgs, OutputFormat, QaScheme};
use std::collections::BTreeSet;

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

// Sentinel-2 L2A scene classification layer (SCL) classes:
// 0 NoData, 1 Saturated/defective, 2 Dark/topographic shadow, 3 Cloud shadow,
// 4 Vegetation, 5 Not-vegetated, 6 Water, 7 Unclassified,
// 8 Cloud medium probability, 9 Cloud high probability, 10 Thin cirrus, 11 Snow/ice.
pub const SCL_DEFAULT_KEEP_CLASSES: &[u8] = &[4, 5, 6];
pub const SCL_REJECT_CLASSES: &[u8] = &[0, 1, 2, 3, 8, 9, 10, 11];

pub fn scl_class_key(class: u16) -> &'static str {
    match class {
        0 => "no_data",
        1 => "saturated_defective",
        2 => "dark_area",
        3 => "cloud_shadow",
        4 => "vegetation",
        5 => "not_vegetated",
        6 => "water",
        7 => "unclassified",
        8 => "cloud_medium_probability",
        9 => "cloud_high_probability",
        10 => "thin_cirrus",
        11 => "snow_ice",
        _ => "out_of_range",
    }
}

/// Configuration for SCL-based masking: which classes count as clear, and how
/// far the rejected (cloud/shadow/nodata) region is dilated before the clear
/// mask is derived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SclMaskConfig {
    pub keep_classes: BTreeSet<u8>,
    pub dilate_radius: u32,
}

impl Default for SclMaskConfig {
    fn default() -> Self {
        Self {
            keep_classes: SCL_DEFAULT_KEEP_CLASSES.iter().copied().collect(),
            dilate_radius: 1,
        }
    }
}

impl SclMaskConfig {
    pub fn from_masks_args(args: &MasksArgs) -> Self {
        let keep_classes = if args.scl_keep_classes.is_empty() {
            SCL_DEFAULT_KEEP_CLASSES.iter().copied().collect()
        } else {
            args.scl_keep_classes.iter().copied().collect()
        };
        Self {
            keep_classes,
            dilate_radius: args.scl_dilate_radius,
        }
    }

    /// Reject classes are the fixed SCL reject set minus any class the
    /// configuration explicitly keeps (e.g. permissive keep-sets).
    pub fn reject_classes(&self) -> BTreeSet<u8> {
        SCL_REJECT_CLASSES
            .iter()
            .copied()
            .filter(|class| !self.keep_classes.contains(class))
            .collect()
    }
}

/// Morphological box (Chebyshev) dilation of a boolean mask by `radius` pixels.
pub fn dilate_mask(mask: &[bool], width: u32, height: u32, radius: u32) -> Vec<bool> {
    if radius == 0 {
        return mask.to_vec();
    }
    let (width, height, radius) = (width as i64, height as i64, radius as i64);
    let mut dilated = vec![false; mask.len()];
    for y in 0..height {
        for x in 0..width {
            if !mask[(y * width + x) as usize] {
                continue;
            }
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx >= 0 && nx < width && ny >= 0 && ny < height {
                        dilated[(ny * width + nx) as usize] = true;
                    }
                }
            }
        }
    }
    dilated
}

/// Rejected pixels (cloud/shadow/nodata) for an SCL raster, before dilation.
pub fn scl_reject_mask(scl: &[u16], config: &SclMaskConfig) -> Vec<bool> {
    let reject_classes = config.reject_classes();
    scl.iter()
        .map(|class| u8::try_from(*class).map_or(true, |class| reject_classes.contains(&class)))
        .collect()
}

/// Per-kind SCL mask sharing the QA_PIXEL output contract. The clear mask is
/// the keep-set minus the dilated reject region; per-class masks are raw.
pub fn scl_kind_masks(
    scl: &[u16],
    width: u32,
    height: u32,
    config: &SclMaskConfig,
) -> BTreeMap<MaskKind, Vec<bool>> {
    let dilated_reject = dilate_mask(
        &scl_reject_mask(scl, config),
        width,
        height,
        config.dilate_radius,
    );

    let mut masks = BTreeMap::new();
    for kind in [
        MaskKind::Cloud,
        MaskKind::CloudShadow,
        MaskKind::Snow,
        MaskKind::Water,
        MaskKind::Clear,
    ] {
        let mask = scl
            .iter()
            .enumerate()
            .map(|(index, class)| match kind {
                MaskKind::Cloud => matches!(class, 8..=10),
                MaskKind::CloudShadow => *class == 3,
                MaskKind::Snow => *class == 11,
                MaskKind::Water => *class == 6,
                MaskKind::Clear => {
                    u8::try_from(*class).is_ok_and(|class| config.keep_classes.contains(&class))
                        && !dilated_reject[index]
                }
            })
            .collect();
        masks.insert(kind, mask);
    }
    masks
}

fn count_scl_classes(scl: &[u16], clear_mask: &[bool]) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::new();
    for class in scl {
        *counts.entry(scl_class_key(*class).to_string()).or_insert(0) += 1;
    }
    counts.insert(
        "clear".to_string(),
        clear_mask.iter().filter(|clear| **clear).count() as u32,
    );
    counts
}

fn qa_pixel_kind_masks(qa: &[u16]) -> BTreeMap<MaskKind, Vec<bool>> {
    let mut masks: BTreeMap<MaskKind, Vec<bool>> = [
        MaskKind::Cloud,
        MaskKind::CloudShadow,
        MaskKind::Snow,
        MaskKind::Water,
        MaskKind::Clear,
    ]
    .into_iter()
    .map(|kind| (kind, Vec::with_capacity(qa.len())))
    .collect();
    for value in qa {
        let (cloud, shadow, snow, water, clear) = qa_pixel_flags(*value);
        for (kind, on) in [
            (MaskKind::Cloud, cloud),
            (MaskKind::CloudShadow, shadow),
            (MaskKind::Snow, snow),
            (MaskKind::Water, water),
            (MaskKind::Clear, clear),
        ] {
            masks.entry(kind).or_default().push(on);
        }
    }
    masks
}

pub fn resolved_qa_band(args: &MasksArgs) -> String {
    args.qa_band.clone().unwrap_or_else(|| {
        match args.qa_scheme {
            QaScheme::QaPixel => "QA_PIXEL",
            QaScheme::Scl => "SCL",
        }
        .to_string()
    })
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
    qa_scheme: String,
    width: u32,
    height: u32,
    class_counts: BTreeMap<String, u32>,
    outputs: BTreeMap<String, String>,
    reproducibility: crate::io::ProductReproducibilityEvidence,
}

async fn process_one(metadata_file: &PathBuf, args: &MasksArgs) -> AgroResult<()> {
    let metadata_content = tokio::fs::read_to_string(metadata_file).await?;
    let image: MultispectralImage = serde_json::from_str(&metadata_content)?;

    let qa_band = resolved_qa_band(args);
    let qa_path = image.file_paths.get(&qa_band).ok_or_else(|| {
        shared::error::AgroError::Processing(format!("QA band '{qa_band}' not found"))
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

    let scl_config = SclMaskConfig::from_masks_args(args);
    let kind_masks = match args.qa_scheme {
        QaScheme::QaPixel => qa_pixel_kind_masks(&qa_u16),
        QaScheme::Scl => scl_kind_masks(&qa_u16, w, h, &scl_config),
    };
    let class_counts = match args.qa_scheme {
        QaScheme::QaPixel => count_qa_classes(&qa_u16),
        QaScheme::Scl => count_scl_classes(&qa_u16, &kind_masks[&MaskKind::Clear]),
    };
    let mut outputs = BTreeMap::new();

    for kind in kinds {
        let kind_mask = &kind_masks[&kind];
        let mut mask = image::GrayImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let on = kind_mask[(y * w + x) as usize];
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
            "qa_band": qa_band.clone(),
            "qa_scheme": format!("{:?}", args.qa_scheme).to_lowercase(),
            "scl_keep_classes": scl_config.keep_classes.iter().copied().collect::<Vec<u8>>(),
            "scl_reject_classes": scl_config.reject_classes().iter().copied().collect::<Vec<u8>>(),
            "scl_dilate_radius": scl_config.dilate_radius,
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

    // Emit an L2 product-record sidecar next to each mask output (Track A phase
    // 10b polish). Every mask derives from the same source QA band, so that band
    // is the identity-bearing input; the mask kind distinguishes the products.
    let mask_scene = image.image_id.to_string();
    let mask_timestamp = image.metadata.timestamp.to_rfc3339();
    for (kind, output_path) in &outputs {
        let sidecar_ctx = crate::product_sidecar::l2_sidecar_context(
            &mask_scene,
            &format!("qa_mask_{kind}"),
            env!("CARGO_PKG_VERSION"),
            vec![crate::product_sidecar::band_input_ref(
                &mask_scene,
                "qa",
                &qa_band,
            )],
            spatial_ref.clone(),
            &mask_timestamp,
            None,
            None,
        );
        let product_path = PathBuf::from(output_path);
        let sidecar_draft = crate::product_sidecar::draft_from_evidence(
            &reproducibility,
            &sidecar_ctx,
            &product_path,
        );
        crate::product_sidecar::write_product_sidecar(&product_path, &sidecar_draft).await?;
    }

    let evidence = MaskEvidence {
        image_id: image.image_id,
        qa_band,
        qa_scheme: format!("{:?}", args.qa_scheme).to_lowercase(),
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

#[cfg(test)]
mod scl_tests {
    use super::*;

    fn config(keep: &[u8], dilate_radius: u32) -> SclMaskConfig {
        SclMaskConfig {
            keep_classes: keep.iter().copied().collect(),
            dilate_radius,
        }
    }

    #[test]
    fn default_keep_set_marks_only_vegetation_bare_and_water_clear() {
        // One pixel per SCL class 0..=11, radius 0 so keep/reject is unmixed.
        let scl: Vec<u16> = (0..=11).collect();
        let masks = scl_kind_masks(&scl, 12, 1, &config(SCL_DEFAULT_KEEP_CLASSES, 0));

        let clear = &masks[&MaskKind::Clear];
        for (class, is_clear) in clear.iter().enumerate() {
            let expected = matches!(class, 4..=6);
            assert_eq!(
                *is_clear, expected,
                "SCL class {class} clear should be {expected}"
            );
        }

        let reject = scl_reject_mask(&scl, &config(SCL_DEFAULT_KEEP_CLASSES, 0));
        for (class, rejected) in reject.iter().enumerate() {
            let expected = matches!(class, 0 | 1 | 2 | 3 | 8 | 9 | 10 | 11);
            assert_eq!(
                *rejected, expected,
                "SCL class {class} rejected should be {expected}"
            );
        }
    }

    #[test]
    fn scl_class_7_is_neither_clear_nor_rejected_by_default() {
        let scl: Vec<u16> = vec![7];
        let cfg = config(SCL_DEFAULT_KEEP_CLASSES, 0);
        assert_eq!(scl_kind_masks(&scl, 1, 1, &cfg)[&MaskKind::Clear], [false]);
        assert_eq!(scl_reject_mask(&scl, &cfg), [false]);
    }

    #[test]
    fn permissive_keep_set_also_keeps_unclassified() {
        let scl: Vec<u16> = vec![4, 5, 6, 7, 8];
        let masks = scl_kind_masks(&scl, 5, 1, &config(&[4, 5, 6, 7], 0));

        assert_eq!(
            masks[&MaskKind::Clear],
            [true, true, true, true, false],
            "permissive keep-set must keep class 7 and still reject clouds"
        );
    }

    #[test]
    fn per_class_masks_map_scl_classes_to_shared_mask_kinds() {
        let scl: Vec<u16> = vec![8, 9, 10, 3, 11, 6, 4];
        let masks = scl_kind_masks(&scl, 7, 1, &config(SCL_DEFAULT_KEEP_CLASSES, 0));

        assert_eq!(
            masks[&MaskKind::Cloud],
            [true, true, true, false, false, false, false],
            "cloud mask must cover classes 8, 9, 10"
        );
        assert_eq!(
            masks[&MaskKind::CloudShadow],
            [false, false, false, true, false, false, false]
        );
        assert_eq!(
            masks[&MaskKind::Snow],
            [false, false, false, false, true, false, false]
        );
        assert_eq!(
            masks[&MaskKind::Water],
            [false, false, false, false, false, true, false]
        );
    }

    #[test]
    fn dilation_grows_reject_region_into_neighboring_clear_pixels() {
        // 5x5 vegetation raster with a single high-probability cloud at center.
        let mut scl = vec![4u16; 25];
        scl[12] = 9;

        let no_dilation = scl_kind_masks(&scl, 5, 5, &config(SCL_DEFAULT_KEEP_CLASSES, 0));
        assert_eq!(
            no_dilation[&MaskKind::Clear]
                .iter()
                .filter(|clear| **clear)
                .count(),
            24
        );

        let dilated = scl_kind_masks(&scl, 5, 5, &config(SCL_DEFAULT_KEEP_CLASSES, 1));
        let clear = &dilated[&MaskKind::Clear];
        assert_eq!(
            clear.iter().filter(|clear| **clear).count(),
            16,
            "radius-1 dilation must clear a 3x3 hole around the cloud pixel"
        );
        for y in 1..4usize {
            for x in 1..4usize {
                assert!(!clear[y * 5 + x], "pixel ({x},{y}) must be inside the hole");
            }
        }
        // Per-class cloud mask stays raw (undilated).
        assert_eq!(
            dilated[&MaskKind::Cloud]
                .iter()
                .filter(|cloud| **cloud)
                .count(),
            1
        );
    }

    #[test]
    fn dilate_mask_respects_radius_and_borders() {
        let mut mask = vec![false; 9];
        mask[0] = true; // corner

        let dilated = dilate_mask(&mask, 3, 3, 1);
        assert_eq!(
            dilated,
            [true, true, false, true, true, false, false, false, false]
        );

        let dilated_wide = dilate_mask(&mask, 3, 3, 2);
        assert_eq!(dilated_wide, [true; 9]);
    }

    #[test]
    fn scl_class_counts_include_dilation_aware_clear_count() {
        let scl: Vec<u16> = vec![4, 4, 9, 6, 0];
        let cfg = config(SCL_DEFAULT_KEEP_CLASSES, 1);
        let masks = scl_kind_masks(&scl, 5, 1, &cfg);
        let counts = count_scl_classes(&scl, &masks[&MaskKind::Clear]);

        assert_eq!(counts.get("vegetation"), Some(&2));
        assert_eq!(counts.get("cloud_high_probability"), Some(&1));
        assert_eq!(counts.get("water"), Some(&1));
        assert_eq!(counts.get("no_data"), Some(&1));
        // Cloud at idx 2 dilates over idx 1 and 3; nodata at idx 4 dilates over 3.
        assert_eq!(counts.get("clear"), Some(&1));
    }
}
