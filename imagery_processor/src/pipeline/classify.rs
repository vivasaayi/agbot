use shared::AgroResult;
use std::path::Path;
use tracing::info;

use crate::ClassifyArgs;

fn gray_to_index(value: u8) -> f32 {
    (value as f32 / 255.0) * 2.0 - 1.0
}

fn gray_center_to_index(value: f32) -> f32 {
    (value / 255.0) * 2.0 - 1.0
}

fn seeded_initial_centers(k: usize, seed: u64) -> Vec<f32> {
    let spacing = 255.0 / k as f32;
    let mut state = seed ^ 0x9e37_79b9_7f4a_7c15;
    let mut centers = (0..k)
        .map(|i| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let jitter_unit = ((state >> 32) as f32 / u32::MAX as f32) - 0.5;
            let base = (i as f32 + 0.5) * spacing;
            (base + jitter_unit * spacing * 0.25).clamp(0.0, 255.0)
        })
        .collect::<Vec<_>>();
    centers.sort_by(|left, right| left.total_cmp(right));
    centers
}

fn threshold_class_boundaries(threshold: f32) -> serde_json::Value {
    serde_json::json!([
        {
            "class_id": 0,
            "label": "below_threshold",
            "min": -1.0,
            "max": threshold,
            "inclusive_min": true,
            "inclusive_max": false,
        },
        {
            "class_id": 1,
            "label": "above_or_equal_threshold",
            "min": threshold,
            "max": 1.0,
            "inclusive_min": true,
            "inclusive_max": true,
        }
    ])
}

fn threshold_class_counts(below: usize, above: usize) -> serde_json::Value {
    serde_json::json!([
        {
            "class_id": 0,
            "label": "below_threshold",
            "pixel_count": below,
        },
        {
            "class_id": 1,
            "label": "above_or_equal_threshold",
            "pixel_count": above,
        }
    ])
}

fn kmeans_class_counts(counts: &[usize], centers: &[f32]) -> serde_json::Value {
    serde_json::Value::Array(
        counts
            .iter()
            .enumerate()
            .map(|(class_id, pixel_count)| {
                serde_json::json!({
                    "class_id": class_id,
                    "label": format!("cluster_{class_id}"),
                    "pixel_count": pixel_count,
                    "center": gray_center_to_index(centers[class_id]),
                })
            })
            .collect(),
    )
}

fn kmeans_class_boundaries(centers: &[f32]) -> serde_json::Value {
    let mut boundaries = Vec::with_capacity(centers.len());
    for (class_id, center) in centers.iter().enumerate() {
        let lower = if class_id == 0 {
            -1.0
        } else {
            gray_center_to_index((centers[class_id - 1] + center) / 2.0)
        };
        let upper = if class_id + 1 == centers.len() {
            1.0
        } else {
            gray_center_to_index((center + centers[class_id + 1]) / 2.0)
        };
        boundaries.push(serde_json::json!({
            "class_id": class_id,
            "label": format!("cluster_{class_id}"),
            "min": lower,
            "max": upper,
            "center": gray_center_to_index(*center),
        }));
    }
    serde_json::Value::Array(boundaries)
}

async fn propagate_png_sidecar(input_image: &Path, output_path: &Path) -> AgroResult<String> {
    let input_sidecar = crate::io::png_spatial_sidecar_path(input_image);
    let output_sidecar = crate::io::png_spatial_sidecar_path(output_path);
    if tokio::fs::metadata(&input_sidecar).await.is_ok() {
        tokio::fs::copy(&input_sidecar, &output_sidecar).await?;
    } else {
        crate::io::write_png_spatial_sidecar(output_path, None).await?;
    }
    Ok(output_sidecar.to_string_lossy().to_string())
}

pub async fn run_classify(args: &ClassifyArgs) -> AgroResult<()> {
    tokio::fs::create_dir_all(
        args.output_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new(".")),
    )
    .await?;

    // Load single-channel PNG index image
    let img = image::open(&args.input_image)
        .map_err(|e| {
            shared::error::AgroError::Processing(format!("Failed to open input image: {}", e))
        })?
        .to_luma8();

    let (w, h) = img.dimensions();
    let total = (w as usize) * (h as usize);

    let mut out = image::GrayImage::new(w, h);
    let class_counts: Vec<usize>;
    let class_boundaries: serde_json::Value;
    let class_centers: serde_json::Value;
    let effective_class_count: usize;

    if let Some(th) = args.threshold {
        // Expect index in [-1, 1] mapped to [0,255] as in indices pipeline
        let mut below = 0usize;
        let mut above = 0usize;
        for (x, y, pix) in out.enumerate_pixels_mut() {
            let idx = gray_to_index(img.get_pixel(x, y)[0]);
            let mask = if idx >= th {
                above += 1;
                255u8
            } else {
                below += 1;
                0u8
            };
            *pix = image::Luma([mask]);
        }
        class_counts = vec![below, above];
        class_boundaries = threshold_class_boundaries(th);
        class_centers = serde_json::Value::Null;
        effective_class_count = class_counts.iter().filter(|count| **count > 0).count();
        info!("Classification by threshold done");
    } else if let Some(k) = args.kmeans {
        if k == 0 {
            return Err(shared::error::AgroError::Processing(
                "k-means requires at least one class".into(),
            ));
        }
        // Simple, naive k-means on grayscale values
        let mut centers = seeded_initial_centers(k, args.seed);
        let mut labels = vec![0usize; total];
        // Few iterations
        for _ in 0..8 {
            // Assign
            for y in 0..h {
                for x in 0..w {
                    let idx = (y as usize) * (w as usize) + (x as usize);
                    let v = img.get_pixel(x, y)[0] as f32;
                    let mut best = 0usize;
                    let mut bestd = f32::INFINITY;
                    for (ci, c) in centers.iter().enumerate() {
                        let d = (v - *c).abs();
                        if d < bestd {
                            bestd = d;
                            best = ci;
                        }
                    }
                    labels[idx] = best;
                }
            }
            // Update
            let mut sums = vec![0.0f64; k];
            let mut counts = vec![0usize; k];
            for y in 0..h {
                for x in 0..w {
                    let idx = (y as usize) * (w as usize) + (x as usize);
                    let v = img.get_pixel(x, y)[0] as f32;
                    let l = labels[idx];
                    sums[l] += v as f64;
                    counts[l] += 1;
                }
            }
            for ci in 0..k {
                if counts[ci] > 0 {
                    centers[ci] = (sums[ci] / counts[ci] as f64) as f32;
                }
            }
        }
        // Output label image: scale labels to 0..255
        for y in 0..h {
            for x in 0..w {
                let idx = (y as usize) * (w as usize) + (x as usize);
                let l = labels[idx] as f32;
                let gray = ((l / (k.saturating_sub(1).max(1) as f32)) * 255.0).round() as u8;
                out.put_pixel(x, y, image::Luma([gray]));
            }
        }
        let mut counts = vec![0usize; k];
        for label in labels {
            counts[label] += 1;
        }
        class_counts = counts;
        effective_class_count = class_counts.iter().filter(|count| **count > 0).count();
        class_centers = serde_json::Value::Array(
            centers
                .iter()
                .map(|center| serde_json::json!(gray_center_to_index(*center)))
                .collect(),
        );
        class_boundaries = kmeans_class_boundaries(&centers);
        info!("Classification by k-means done (k={})", k);
    } else {
        return Err(shared::error::AgroError::Processing(
            "Either --threshold or --kmeans must be provided".into(),
        ));
    }

    out.save(&args.output_path).map_err(|e| {
        shared::error::AgroError::Processing(format!("Failed to save classification image: {}", e))
    })?;
    let spatial_ref_sidecar = propagate_png_sidecar(&args.input_image, &args.output_path).await?;

    // Simple JSON stats: proportion of positive class for threshold, or cluster counts
    if let Some(th) = args.threshold {
        let count_pos = class_counts[1];
        let meta = serde_json::json!({
            "method": "threshold",
            "threshold": th,
            "positive_ratio": count_pos as f64 / total as f64,
            "total_pixel_count": total,
            "effective_class_count": effective_class_count,
            "class_boundaries": class_boundaries,
            "class_counts": threshold_class_counts(class_counts[0], class_counts[1]),
            "spatial_ref_sidecar": spatial_ref_sidecar,
        });
        let json_path = args.output_path.with_extension("json");
        tokio::fs::write(json_path, serde_json::to_string_pretty(&meta)?).await?;
    } else if let Some(k) = args.kmeans {
        let center_gray_values = centers_from_json(&class_centers)?;
        let meta = serde_json::json!({
            "method": "kmeans",
            "k": k,
            "seed": args.seed,
            "total_pixel_count": total,
            "effective_class_count": effective_class_count,
            "class_centers": class_centers,
            "class_boundaries": class_boundaries,
            "class_counts": kmeans_class_counts(&class_counts, &center_gray_values),
            "cluster_counts": class_counts,
            "spatial_ref_sidecar": spatial_ref_sidecar,
        });
        let json_path = args.output_path.with_extension("json");
        tokio::fs::write(json_path, serde_json::to_string_pretty(&meta)?).await?;
    }

    Ok(())
}

fn centers_from_json(value: &serde_json::Value) -> AgroResult<Vec<f32>> {
    value
        .as_array()
        .ok_or_else(|| shared::error::AgroError::Processing("missing k-means centers".into()))?
        .iter()
        .map(|value| {
            value
                .as_f64()
                .map(|center| (((center as f32) + 1.0) / 2.0) * 255.0)
                .ok_or_else(|| {
                    shared::error::AgroError::Processing("invalid k-means center".into())
                })
        })
        .collect()
}
