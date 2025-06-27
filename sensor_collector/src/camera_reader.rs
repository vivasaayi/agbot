use shared::{
    config::AgroConfig,
    schemas::{MultispectralImage, ImageMetadata, GpsCoords},
    AgroResult,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tracing::{info, error};

pub struct CameraReader {
    config: Arc<AgroConfig>,
    data_dir: PathBuf,
}

impl CameraReader {
    pub async fn new(config: Arc<AgroConfig>, data_dir: PathBuf) -> AgroResult<Self> {
        Ok(Self { config, data_dir })
    }

    pub async fn run(&self) -> AgroResult<()> {
        info!("Starting multispectral camera reader on {}", self.config.camera.device);

        let mut capture_interval = tokio::time::interval(
            std::time::Duration::from_millis(self.config.camera.capture_interval_ms)
        );

        loop {
            capture_interval.tick().await;

            match self.capture_image().await {
                Ok(image) => {
                    self.save_image(&image).await?;
                    info!("Captured multispectral image: {}", image.image_id);
                }
                Err(e) => {
                    error!("Failed to capture image: {}", e);
                }
            }
        }
    }

    async fn capture_image(&self) -> AgroResult<MultispectralImage> {
        let timestamp = chrono::Utc::now();
        let image_id = uuid::Uuid::new_v4();

        // Mock GPS position (in real implementation, get from MAVLink)
        let gps_position = Some(GpsCoords {
            latitude: self.config.gps.home_latitude + (rand::random::<f64>() - 0.5) * 0.001,
            longitude: self.config.gps.home_longitude + (rand::random::<f64>() - 0.5) * 0.001,
            altitude: self.config.gps.home_altitude,
        });

        let bands = vec!["Red".to_string(), "NIR".to_string(), "Green".to_string(), "Blue".to_string()];
        
        let metadata = ImageMetadata {
            timestamp,
            gps_position,
            bands: bands.clone(),
            exposure_time: self.config.camera.exposure_time,
            gain: self.config.camera.gain,
            width: 1280,
            height: 1024,
        };

        // Create placeholder image files for each band
        let mut file_paths = HashMap::new();
        let session_dir = self.data_dir.join(timestamp.format("%Y%m%d_%H%M%S").to_string());
        tokio::fs::create_dir_all(&session_dir).await?;

        for band in &bands {
            let filename = format!("{}_{}.tiff", image_id, band.to_lowercase());
            let filepath = session_dir.join(&filename);
            
            // Create a simple test image (in real implementation, capture from camera)
            self.create_test_image(&filepath, band).await?;
            
            file_paths.insert(band.clone(), filepath.to_string_lossy().to_string());
        }

        Ok(MultispectralImage {
            metadata,
            file_paths,
            image_id,
        })
    }

    async fn create_test_image(&self, filepath: &PathBuf, band: &str) -> AgroResult<()> {
        // Create a simple test image using the image crate
        let (width, height) = (1280u32, 1024u32);
        
        let color = match band {
            "Red" => [255u8, 0u8, 0u8],
            "NIR" => [128u8, 128u8, 128u8],
            "Green" => [0u8, 255u8, 0u8],
            "Blue" => [0u8, 0u8, 255u8],
            _ => [128u8, 128u8, 128u8],
        };

        let mut img = image::ImageBuffer::new(width, height);
        for (_, _, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Rgb(color);
        }

        img.save(filepath)
            .map_err(|e| shared::error::AgroError::Processing(format!("Failed to save image: {}", e)))?;

        Ok(())
    }

    async fn save_image(&self, image: &MultispectralImage) -> AgroResult<()> {
        let filename = format!(
            "metadata_{}_{}.json",
            image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
            image.image_id
        );
        let filepath = self.data_dir.join(filename);

        let json = serde_json::to_string_pretty(image)?;
        tokio::fs::write(filepath, json).await?;

        Ok(())
    }
}

pub struct SimulatedCameraReader {
    config: Arc<AgroConfig>,
    data_dir: PathBuf,
}

impl SimulatedCameraReader {
    pub fn new(config: Arc<AgroConfig>, data_dir: PathBuf) -> Self {
        Self { config, data_dir }
    }

    pub async fn run(&self) -> AgroResult<()> {
        info!("Starting simulated multispectral camera reader");

        let mut capture_interval = tokio::time::interval(
            std::time::Duration::from_millis(self.config.camera.capture_interval_ms)
        );

        loop {
            capture_interval.tick().await;

            let image = self.generate_simulated_image().await?;
            self.save_image(&image).await?;
            info!("Generated simulated multispectral image: {}", image.image_id);
        }
    }

    async fn generate_simulated_image(&self) -> AgroResult<MultispectralImage> {
        let timestamp = chrono::Utc::now();
        let image_id = uuid::Uuid::new_v4();

        // Simulated GPS position with slight variations
        let gps_position = Some(GpsCoords {
            latitude: self.config.gps.home_latitude + (rand::random::<f64>() - 0.5) * 0.01,
            longitude: self.config.gps.home_longitude + (rand::random::<f64>() - 0.5) * 0.01,
            altitude: self.config.gps.home_altitude + (rand::random::<f64>() - 0.5) * 50.0,
        });

        let bands = vec!["Red".to_string(), "NIR".to_string(), "Green".to_string(), "Blue".to_string()];
        
        let metadata = ImageMetadata {
            timestamp,
            gps_position,
            bands: bands.clone(),
            exposure_time: self.config.camera.exposure_time,
            gain: self.config.camera.gain,
            width: 1280,
            height: 1024,
        };

        // Create simulated image files for each band
        let mut file_paths = HashMap::new();
        let session_dir = self.data_dir.join(format!("sim_{}", timestamp.format("%Y%m%d_%H%M%S")));
        tokio::fs::create_dir_all(&session_dir).await?;

        for band in &bands {
            let filename = format!("sim_{}_{}.tiff", image_id, band.to_lowercase());
            let filepath = session_dir.join(&filename);
            
            self.create_simulated_band_image(&filepath, band).await?;
            
            file_paths.insert(band.clone(), filepath.to_string_lossy().to_string());
        }

        Ok(MultispectralImage {
            metadata,
            file_paths,
            image_id,
        })
    }

    async fn create_simulated_band_image(&self, filepath: &PathBuf, band: &str) -> AgroResult<()> {
        let (width, height) = (1280u32, 1024u32);
        let mut img = image::ImageBuffer::new(width, height);

        for (x, y, pixel) in img.enumerate_pixels_mut() {
            // Create a pattern that simulates vegetation and soil
            let vegetation_mask = ((x + y) % 50) < 25;
            
            let color = match band {
                "Red" => {
                    if vegetation_mask {
                        [50u8, 0u8, 0u8] // Low red for vegetation
                    } else {
                        [150u8, 0u8, 0u8] // Higher red for soil
                    }
                }
                "NIR" => {
                    if vegetation_mask {
                        [200u8, 200u8, 200u8] // High NIR for vegetation
                    } else {
                        [100u8, 100u8, 100u8] // Lower NIR for soil
                    }
                }
                "Green" => {
                    if vegetation_mask {
                        [0u8, 255u8, 0u8] // High green for vegetation
                    } else {
                        [0u8, 100u8, 0u8] // Lower green for soil
                    }
                }
                "Blue" => {
                    if vegetation_mask {
                        [0u8, 0u8, 50u8] // Low blue for vegetation
                    } else {
                        [0u8, 0u8, 120u8] // Higher blue for soil
                    }
                }
                _ => [128u8, 128u8, 128u8],
            };

            *pixel = image::Rgb(color);
        }

        img.save(filepath)
            .map_err(|e| shared::error::AgroError::Processing(format!("Failed to save simulated image: {}", e)))?;

        Ok(())
    }

    async fn save_image(&self, image: &MultispectralImage) -> AgroResult<()> {
        let filename = format!(
            "sim_metadata_{}_{}.json",
            image.metadata.timestamp.format("%Y%m%d_%H%M%S"),
            image.image_id
        );
        let filepath = self.data_dir.join(filename);

        let json = serde_json::to_string_pretty(image)?;
        tokio::fs::write(filepath, json).await?;

        Ok(())
    }
}
