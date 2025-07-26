use clap::Parser;
use shared::{
    config::AgroConfig,
    schemas::LidarScan,
    AgroResult,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tracing::{info, warn, error};
use indicatif::{ProgressBar, ProgressStyle};
// use futures::stream::{self, StreamExt};

#[derive(Parser, Debug)]
#[command(name = "lidar_mapper")]
#[command(about = "LiDAR Mapper for point cloud processing")]
pub struct Args {
    #[arg(long, help = "Input directory containing LiDAR scan files")]
    pub input_dir: PathBuf,
    
    #[arg(long, help = "Output directory for maps and visualizations")]
    pub output_dir: PathBuf,
    
    #[arg(long, help = "Override obstacle distance threshold (m)")]
    pub distance_threshold: Option<f32>,
    #[arg(long, help = "Override quality threshold")]
    pub quality_threshold: Option<u8>,
    #[arg(long, help = "Override occupancy threshold (0.0-1.0)")]
    pub occupancy_threshold: Option<f32>,
    #[arg(long, help = "Flip Y axis in output images")]
    pub flip_y: Option<bool>,
}

pub struct LidarMapper {
    config: Arc<AgroConfig>,
}

#[derive(Debug, Clone)]
pub struct GridCell {
    pub occupied: bool,
    pub obstacle_count: usize,
    pub total_observations: usize,
}

impl Default for GridCell {
    fn default() -> Self {
        Self {
            occupied: false,
            obstacle_count: 0,
            total_observations: 0,
        }
    }
}

impl LidarMapper {
    /// Create a new mapper, loading config and applying CLI overrides
    pub async fn new(args: &Args) -> AgroResult<Self> {
        // Load base config
        let mut config = AgroConfig::load()?;
        // Apply CLI overrides
        if let Some(d) = args.distance_threshold {
            config.processing.lidar_obstacle_distance_threshold = d;
        }
        if let Some(q) = args.quality_threshold {
            config.processing.lidar_quality_threshold = q;
        }
        if let Some(o) = args.occupancy_threshold {
            config.processing.lidar_occupancy_threshold = o;
        }
        if let Some(f) = args.flip_y {
            config.processing.lidar_image_flip_y = f;
        }
        Ok(Self { config: Arc::new(config) })
    }

    pub async fn process_directory(&self, input_dir: &PathBuf, output_dir: &PathBuf) -> AgroResult<()> {
        info!("Processing LiDAR scans in: {:?}", input_dir);
        
        tokio::fs::create_dir_all(output_dir).await?;

        // Find all scan JSON files
        let mut scan_files = Vec::new();
        for entry in walkdir::WalkDir::new(input_dir) {
            let entry = entry.map_err(|e| shared::error::AgroError::Io(e.into()))?;
            if entry.file_name().to_string_lossy().contains("scan_") &&
               entry.path().extension().map_or(false, |ext| ext == "json") {
                scan_files.push(entry.path().to_path_buf());
            }
        }

        info!("Found {} scan files to process", scan_files.len());
        // Initialize progress bar for loading scans
        let pb = ProgressBar::new(scan_files.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .expect("Invalid progress bar template")
                .progress_chars("#>-"),
        );
        
        use futures::stream::{self, StreamExt};

        // Load all scans in parallel, track successes/failures
        let parallelism = 8;
        let mut load_stream = stream::iter(scan_files.into_iter().map(|path| {
            async move {
                let content = tokio::fs::read_to_string(&path).await
                    .map_err(|e| shared::error::AgroError::Io(e.into()))?;
                let scan: LidarScan = serde_json::from_str(&content)
                    .map_err(|e| shared::error::AgroError::Processing(e.to_string()))?;
                Ok::<(PathBuf, LidarScan), shared::error::AgroError>((path, scan))
            }
        }))
        .buffer_unordered(parallelism);

        let mut all_scans = Vec::new();
        let mut loaded = 0;
        let mut failed = 0;
        while let Some(res) = load_stream.next().await {
            // Update progress
            pb.inc(1);
            match res {
                Ok((_path, scan)) => { all_scans.push(scan); loaded += 1; }
                Err(e) => { error!("Failed to load scan: {}", e); failed += 1; }
            }
        }
        // Finish progress bar
        pb.finish_with_message("Scan loading complete");
        info!("Scans loaded: {}, failed: {}", loaded, failed);
        if loaded == 0 {
            return Err(shared::error::AgroError::Processing(
                "No LiDAR scans were processed successfully".into(),
            ).into());
        }

        // Create occupancy grid
        let grid = self.create_occupancy_grid(&all_scans)?;
        
        // Save grid as image
        self.save_grid_image(&grid, output_dir).await?;
        
        // Save point cloud
        self.save_point_cloud(&all_scans, output_dir).await?;
        
        // Generate obstacle heatmap
        self.save_obstacle_heatmap(&grid, output_dir).await?;

        info!("LiDAR mapping completed");
        Ok(())
    }

    async fn load_scan(&self, scan_file: &PathBuf) -> AgroResult<LidarScan> {
        let content = tokio::fs::read_to_string(scan_file).await?;
        let scan: LidarScan = serde_json::from_str(&content)?;
        Ok(scan)
    }

    pub fn create_occupancy_grid(&self, scans: &[LidarScan]) -> AgroResult<HashMap<(i32, i32), GridCell>> {
        let resolution = self.config.processing.lidar_grid_resolution;
        let dist_thresh = self.config.processing.lidar_obstacle_distance_threshold;
        let qual_thresh = self.config.processing.lidar_quality_threshold;
        let mut grid: HashMap<(i32, i32), GridCell> = HashMap::new();
        
        info!("Creating occupancy grid with resolution: {} m", resolution);

        for scan in scans {
            for point in &scan.points {
                // Convert polar to cartesian coordinates
                let angle_rad = point.angle.to_radians();
                let distance_m = point.distance / 1000.0; // Convert mm to m
                
                let x = distance_m * angle_rad.cos();
                let y = distance_m * angle_rad.sin();
                
                // Convert to grid coordinates
                let grid_x = (x / resolution) as i32;
                let grid_y = (y / resolution) as i32;
                
                let cell = grid.entry((grid_x, grid_y)).or_default();
                cell.total_observations += 1;
                
                // Count as obstacle if within threshold
                if distance_m < dist_thresh && (point.quality as u8) > qual_thresh {
                    cell.obstacle_count += 1;
                }
            }
        }

        // Determine final occupancy based on threshold
        let occ_thresh = self.config.processing.lidar_occupancy_threshold;
        for cell in grid.values_mut() {
            if cell.total_observations > 0 {
                let ratio = cell.obstacle_count as f32 / cell.total_observations as f32;
                cell.occupied = ratio > occ_thresh;
            }
        }
        info!("Generated occupancy grid with {} cells", grid.len());
        Ok(grid)
    }

    async fn save_grid_image(&self, grid: &HashMap<(i32, i32), GridCell>, output_dir: &PathBuf) -> AgroResult<()> {
        // Find grid bounds
        let min_x = grid.keys().map(|(x, _)| *x).min().unwrap_or(0);
        let max_x = grid.keys().map(|(x, _)| *x).max().unwrap_or(0);
        let min_y = grid.keys().map(|(_, y)| *y).min().unwrap_or(0);
        let max_y = grid.keys().map(|(_, y)| *y).max().unwrap_or(0);

        let width = (max_x - min_x + 1) as u32;
        let height = (max_y - min_y + 1) as u32;

        let mut img = image::ImageBuffer::new(width, height);
        let flip_y = self.config.processing.lidar_image_flip_y;

        for ((grid_x, grid_y), cell) in grid {
            let pixel_x = (*grid_x - min_x) as u32;
            let py = (*grid_y - min_y) as u32;
            let pixel_y = if flip_y { height - 1 - py } else { py };

            let color = if cell.occupied {
                [0u8, 0u8, 0u8] // Black for obstacles
            } else if cell.total_observations > 0 {
                [255u8, 255u8, 255u8] // White for free space
            } else {
                [128u8, 128u8, 128u8] // Gray for unknown
            };

            if pixel_x < width && pixel_y < height {
                img.put_pixel(pixel_x, pixel_y, image::Rgb(color));
            }
        }

        let output_path = output_dir.join("occupancy_grid.png");
        img.save(&output_path)
            .map_err(|e| shared::error::AgroError::Processing(format!("Failed to save grid image: {}", e)))?;

        info!("Saved occupancy grid to: {:?}", output_path);
        Ok(())
    }

    async fn save_point_cloud(&self, scans: &[LidarScan], output_dir: &PathBuf) -> AgroResult<()> {
        let mut points = Vec::new();

        for scan in scans {
            for point in &scan.points {
                let angle_rad = point.angle.to_radians();
                let distance_m = point.distance / 1000.0;
                
                let x = distance_m * angle_rad.cos();
                let y = distance_m * angle_rad.sin();
                let z = 0.0; // 2D LiDAR, so Z is always 0
                
                points.push(format!("{:.3} {:.3} {:.3}", x, y, z));
            }
        }

        let pcd_content = format!(
            "# .PCD v0.7 - Point Cloud Data file format\n\
             VERSION 0.7\n\
             FIELDS x y z\n\
             SIZE 4 4 4\n\
             TYPE F F F\n\
             COUNT 1 1 1\n\
             WIDTH {}\n\
             HEIGHT 1\n\
             VIEWPOINT 0 0 0 1 0 0 0\n\
             POINTS {}\n\
             DATA ascii\n\
             {}",
            points.len(),
            points.len(),
            points.join("\n")
        );

        let output_path = output_dir.join("point_cloud.pcd");
        tokio::fs::write(&output_path, pcd_content).await?;

        info!("Saved point cloud with {} points to: {:?}", points.len(), output_path);
        Ok(())
    }

    async fn save_obstacle_heatmap(&self, grid: &HashMap<(i32, i32), GridCell>, output_dir: &PathBuf) -> AgroResult<()> {
        // Find grid bounds
        let min_x = grid.keys().map(|(x, _)| *x).min().unwrap_or(0);
        let max_x = grid.keys().map(|(x, _)| *x).max().unwrap_or(0);
        let min_y = grid.keys().map(|(_, y)| *y).min().unwrap_or(0);
        let max_y = grid.keys().map(|(_, y)| *y).max().unwrap_or(0);

        let width = (max_x - min_x + 1) as u32;
        let height = (max_y - min_y + 1) as u32;

        let mut img = image::ImageBuffer::new(width, height);
        let flip_y = self.config.processing.lidar_image_flip_y;

        // Find max obstacle count for normalization
        let max_count = grid.values()
            .map(|cell| cell.obstacle_count)
            .max()
            .unwrap_or(1);

        for ((grid_x, grid_y), cell) in grid {
            let pixel_x = (*grid_x - min_x) as u32;
            let py = (*grid_y - min_y) as u32;
            let pixel_y = if flip_y { height - 1 - py } else { py };

            // Create heat map based on obstacle density
            let intensity = (cell.obstacle_count as f32 / max_count as f32 * 255.0) as u8;
            let color = [intensity, 0u8, 255u8 - intensity]; // Blue to red gradient

            if pixel_x < width && pixel_y < height {
                img.put_pixel(pixel_x, pixel_y, image::Rgb(color));
            }
        }

        let output_path = output_dir.join("obstacle_heatmap.png");
        img.save(&output_path)
            .map_err(|e| shared::error::AgroError::Processing(format!("Failed to save heatmap: {}", e)))?;

        info!("Saved obstacle heatmap to: {:?}", output_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::config::AgroConfig;
    use shared::schemas::{LidarScan, LidarPoint};
    use uuid::Uuid;
    use chrono::Utc;
    use std::sync::Arc;

    #[test]
    fn test_create_occupancy_grid_obstacle() {
        // Single point within obstacle threshold should mark cell occupied
        let config = AgroConfig::load().unwrap();
        let mapper = LidarMapper { config: Arc::new(config) };
        let point = LidarPoint {
            timestamp: Utc::now(),
            angle: 0.0,
            distance: 1000.0, // 1m < default threshold 5m
            quality: 30,
        };
        let scan = LidarScan {
            timestamp: Utc::now(),
            points: vec![point],
            scan_id: Uuid::new_v4(),
        };
        let grid = mapper.create_occupancy_grid(&[scan]).unwrap();
        assert_eq!(grid.len(), 1);
        let cell = grid.values().next().unwrap();
        assert!(cell.occupied, "Cell should be marked occupied");
    }

    #[test]
    fn test_create_occupancy_grid_free() {
        // Single point beyond obstacle threshold should mark cell free
        let config = AgroConfig::load().unwrap();
        let mapper = LidarMapper { config: Arc::new(config) };
        let point = LidarPoint {
            timestamp: Utc::now(),
            angle: std::f32::consts::FRAC_PI_2,
            distance: 10000.0, // 10m > default threshold 5m
            quality: 30,
        };
        let scan = LidarScan {
            timestamp: Utc::now(),
            points: vec![point],
            scan_id: Uuid::new_v4(),
        };
        let grid = mapper.create_occupancy_grid(&[scan]).unwrap();
        assert_eq!(grid.len(), 1);
        let cell = grid.values().next().unwrap();
        assert!(!cell.occupied, "Cell should be marked free");
    }
}
