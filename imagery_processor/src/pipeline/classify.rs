use shared::AgroResult;
use tracing::info;

use crate::ClassifyArgs;

pub async fn run_classify(args: &ClassifyArgs) -> AgroResult<()> {
    tokio::fs::create_dir_all(args.output_path.parent().unwrap_or_else(|| std::path::Path::new("."))).await?;

    // Load single-channel PNG index image
    let img = image::open(&args.input_image)
        .map_err(|e| shared::error::AgroError::Processing(format!("Failed to open input image: {}", e)))?
        .to_luma8();

    let (w, h) = img.dimensions();

    let mut out = image::GrayImage::new(w, h);

    if let Some(th) = args.threshold {
        // Expect index in [-1, 1] mapped to [0,255] as in indices pipeline
        for (x, y, pix) in out.enumerate_pixels_mut() {
            let v = img.get_pixel(x, y)[0] as f32;
            let idx = (v / 255.0) * 2.0 - 1.0; // back to [-1, 1]
            let mask = if idx >= th { 255u8 } else { 0u8 };
            *pix = image::Luma([mask]);
        }
        info!("Classification by threshold done");
    } else if let Some(k) = args.kmeans {
        // Simple, naive k-means on grayscale values
        let centers_init: Vec<f32> = (0..k).map(|i| (i as f32 + 0.5) * (255.0 / k as f32)).collect();
        let mut centers = centers_init;
        let mut labels = vec![0usize; (w*h) as usize];
        // Few iterations
        for _ in 0..8 {
            // Assign
            for y in 0..h {
                for x in 0..w {
                    let idx = (y as usize) * (w as usize) + (x as usize);
                    let v = img.get_pixel(x, y)[0] as f32;
                    let mut best = 0usize; let mut bestd = f32::INFINITY;
                    for (ci, c) in centers.iter().enumerate() {
                        let d = (v - *c).abs();
                        if d < bestd { bestd = d; best = ci; }
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
                if counts[ci] > 0 { centers[ci] = (sums[ci] / counts[ci] as f64) as f32; }
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
        info!("Classification by k-means done (k={})", k);
    } else {
        return Err(shared::error::AgroError::Processing("Either --threshold or --kmeans must be provided".into()));
    }

    out.save(&args.output_path)
        .map_err(|e| shared::error::AgroError::Processing(format!("Failed to save classification image: {}", e)))?;

    // Simple JSON stats: proportion of positive class for threshold, or cluster counts
    if let Some(th) = args.threshold {
        let mut count_pos = 0usize;
        for (x, y, _) in out.enumerate_pixels() {
            if out.get_pixel(x, y)[0] == 255 { count_pos += 1; }
        }
        let total = (w as usize) * (h as usize);
        let meta = serde_json::json!({
            "method": "threshold",
            "threshold": th,
            "positive_ratio": count_pos as f64 / total as f64,
        });
        let json_path = args.output_path.with_extension("json");
        tokio::fs::write(json_path, serde_json::to_string_pretty(&meta)?).await?;
    } else if let Some(k) = args.kmeans {
        let mut counts = vec![0usize; k];
        for y in 0..h { for x in 0..w {
            let v = out.get_pixel(x, y)[0] as f32;
            let label = ((v as f32 / 255.0) * (k.saturating_sub(1).max(1) as f32)).round() as usize;
            counts[label] += 1;
        }}
        let meta = serde_json::json!({
            "method": "kmeans",
            "k": k,
            "cluster_counts": counts,
        });
        let json_path = args.output_path.with_extension("json");
        tokio::fs::write(json_path, serde_json::to_string_pretty(&meta)?).await?;
    }

    Ok(())
}
