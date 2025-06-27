use clap::Parser;
use shared::{
    config::AgroConfig,
    schemas::{MultispectralImage, NdviResult},
    AgroResult,
};
use std::{path::PathBuf, sync::Arc};
use tracing::{info, error};

#[derive(Parser, Debug)]
#[command(name = "ndvi_processor")]
#[command(about = "NDVI Processor for multispectral images")]
pub struct Args {
    #[arg(long, help = "Input directory containing multispectral images")]
    pub input_dir: PathBuf,
    
    #[arg(long, help = "Output directory for NDVI results")]
    pub output_dir: PathBuf,
}

pub struct NdviProcessor {
    #[allow(dead_code)]
    config: Arc<AgroConfig>,
}

impl NdviProcessor {
    pub async fn new() -> AgroResult<Self> {
        let config = Arc::new(AgroConfig::load()?);
        Ok(Self { config })
    }

    pub async fn process_directory(&self, input_dir: &PathBuf, output_dir: &PathBuf) -> AgroResult<()> {
        info!("Processing NDVI for images in: {:?}", input_dir);
        
        tokio::fs::create_dir_all(output_dir).await?;

        // Find all metadata JSON files
        let mut metadata_files = Vec::new();
        for entry in walkdir::WalkDir::new(input_dir) {
            let entry = entry.map_err(|e| shared::error::AgroError::Io(e.into()))?;
            if entry.file_name().to_string_lossy().starts_with("metadata_") &&
               entry.path().extension().map_or(false, |ext| ext == "json") {
                metadata_files.push(entry.path().to_path_buf());
            }
        }

        info!("Found {} metadata files to process", metadata_files.len());

        for metadata_file in metadata_files {
            match self.process_image(&metadata_file, output_dir).await {
                Ok(result) => {
                    info!("Processed NDVI: {} (mean: {:.3})", result.output_path, result.mean_ndvi);
                }
                Err(e) => {
                    error!("Failed to process {}: {}", metadata_file.display(), e);
                }
            }
        }

        Ok(())
    }

    async fn process_image(&self, metadata_file: &PathBuf, output_dir: &PathBuf) -> AgroResult<NdviResult> {
        // Load metadata
        let metadata_content = tokio::fs::read_to_string(metadata_file).await?;
        let image: MultispectralImage = serde_json::from_str(&metadata_content)?;

        // Find Red and NIR bands
        let red_path = image.file_paths.get("Red")
            .ok_or_else(|| shared::error::AgroError::Processing("Red band not found".to_string()))?;
        let nir_path = image.file_paths.get("NIR")
            .ok_or_else(|| shared::error::AgroError::Processing("NIR band not found".to_string()))?;

        // Load images
        let red_img = image::open(red_path)
            .map_err(|e| shared::error::AgroError::Processing(format!("Failed to load Red band: {}", e)))?
            .to_rgb8();
        
        let nir_img = image::open(nir_path)
            .map_err(|e| shared::error::AgroError::Processing(format!("Failed to load NIR band: {}", e)))?
            .to_rgb8();

        if red_img.dimensions() != nir_img.dimensions() {
            return Err(shared::error::AgroError::Processing(
                "Red and NIR images have different dimensions".to_string()
            ));
        }

        // Calculate NDVI
        let (width, height) = red_img.dimensions();
        let mut ndvi_img = image::ImageBuffer::new(width, height);
        let mut ndvi_values = Vec::new();

        for (x, y, pixel) in ndvi_img.enumerate_pixels_mut() {
            let red_pixel = red_img.get_pixel(x, y);
            let nir_pixel = nir_img.get_pixel(x, y);

            // Use red channel for simplicity (in practice, you might average RGB or use specific channels)
            let red = red_pixel[0] as f32;
            let nir = nir_pixel[0] as f32;

            // Calculate NDVI: (NIR - Red) / (NIR + Red)
            let ndvi = if (nir + red) > 0.0 {
                (nir - red) / (nir + red)
            } else {
                0.0
            };

            ndvi_values.push(ndvi);

            // Convert NDVI (-1 to 1) to grayscale (0 to 255)
            let gray_value = ((ndvi + 1.0) * 127.5) as u8;
            *pixel = image::Luma([gray_value]);
        }

        // Calculate statistics
        let min_ndvi = ndvi_values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_ndvi = ndvi_values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let mean_ndvi = ndvi_values.iter().sum::<f32>() / ndvi_values.len() as f32;
        
        // Calculate vegetation percentage (NDVI > 0.3 is often considered vegetation)
        let vegetation_pixels = ndvi_values.iter().filter(|&&v| v > 0.3).count();
        let vegetation_percentage = (vegetation_pixels as f32 / ndvi_values.len() as f32) * 100.0;

        // Save NDVI image
        let output_filename = format!(
            "ndvi_{}_{}.png",
            image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
            image.image_id
        );
        let output_path = output_dir.join(&output_filename);

        ndvi_img.save(&output_path)
            .map_err(|e| shared::error::AgroError::Processing(format!("Failed to save NDVI image: {}", e)))?;

        // Create result
        let result = NdviResult {
            timestamp: chrono::Utc::now(),
            source_images: vec![image.image_id],
            output_path: output_path.to_string_lossy().to_string(),
            min_ndvi,
            max_ndvi,
            mean_ndvi,
            vegetation_percentage,
        };

        // Save result metadata
        let result_filename = format!(
            "ndvi_result_{}_{}.json",
            image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
            image.image_id
        );
        let result_path = output_dir.join(result_filename);
        let result_json = serde_json::to_string_pretty(&result)?;
        tokio::fs::write(result_path, result_json).await?;

        Ok(result)
    }
}
