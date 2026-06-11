use chrono::{DateTime, Utc};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use shared::{config::AgroConfig, schemas::LidarScan, AgroResult};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::{error, info};
use uuid::Uuid;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarScanIngestRecord {
    pub path: String,
    pub scan_id: Uuid,
    pub captured_at: DateTime<Utc>,
    pub ingested_at: DateTime<Utc>,
    pub point_count: usize,
    pub angular_coverage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarScanIngestFailure {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarScanIngestSummary {
    pub loaded_count: usize,
    pub failed_count: usize,
    pub records: Vec<LidarScanIngestRecord>,
    pub failures: Vec<LidarScanIngestFailure>,
}

#[derive(Debug, Clone)]
pub struct LidarScanIngest {
    pub scans: Vec<LidarScan>,
    pub summary: LidarScanIngestSummary,
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
        Ok(Self {
            config: Arc::new(config),
        })
    }

    pub async fn process_directory(
        &self,
        input_dir: &PathBuf,
        output_dir: &PathBuf,
    ) -> AgroResult<()> {
        info!("Processing LiDAR scans in: {:?}", input_dir);

        let ingest = self.ingest_scans(input_dir, output_dir).await?;
        let all_scans = ingest.scans;

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

    pub async fn ingest_scans(
        &self,
        input_dir: &Path,
        output_dir: &Path,
    ) -> AgroResult<LidarScanIngest> {
        tokio::fs::create_dir_all(output_dir).await?;

        let scan_files = Self::scan_files(input_dir)?;
        info!("Found {} scan files to process", scan_files.len());

        let pb = ProgressBar::new(scan_files.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                )
                .expect("Invalid progress bar template")
                .progress_chars("#>-"),
        );

        use futures::stream::{self, StreamExt};

        let parallelism = 8;
        let mut load_stream = stream::iter(scan_files.into_iter().map(|path| async move {
            let loaded = Self::load_scan(&path).await;
            (path, loaded)
        }))
        .buffer_unordered(parallelism);

        let mut loaded_scans = Vec::new();
        let mut failures = Vec::new();
        while let Some((path, loaded)) = load_stream.next().await {
            pb.inc(1);
            let path_string = path.to_string_lossy().to_string();
            match loaded {
                Ok(scan) => {
                    let record = LidarScanIngestRecord {
                        path: path_string,
                        scan_id: scan.scan_id,
                        captured_at: scan.timestamp,
                        ingested_at: Utc::now(),
                        point_count: scan.points.len(),
                        angular_coverage: Self::angular_coverage_degrees(&scan),
                    };
                    loaded_scans.push((path, scan, record));
                }
                Err(e) => {
                    error!("Failed to load scan {:?}: {}", path, e);
                    failures.push(LidarScanIngestFailure {
                        path: path_string,
                        error: e.to_string(),
                    });
                }
            }
        }

        pb.finish_with_message("Scan loading complete");

        loaded_scans.sort_by(|(left, _, _), (right, _, _)| left.cmp(right));
        failures.sort_by(|left, right| left.path.cmp(&right.path));

        let records: Vec<_> = loaded_scans
            .iter()
            .map(|(_, _, record)| record.clone())
            .collect();
        let scans: Vec<_> = loaded_scans.into_iter().map(|(_, scan, _)| scan).collect();
        let summary = LidarScanIngestSummary {
            loaded_count: records.len(),
            failed_count: failures.len(),
            records,
            failures,
        };

        let summary_path = Self::scan_ingest_summary_path(output_dir);
        let content = serde_json::to_vec_pretty(&summary)?;
        tokio::fs::write(&summary_path, content).await?;
        info!(
            "Scans loaded: {}, failed: {}, summary: {:?}",
            summary.loaded_count, summary.failed_count, summary_path
        );

        if summary.loaded_count == 0 {
            return Err(shared::error::AgroError::Processing(
                "No LiDAR scans were processed successfully".into(),
            ));
        }

        Ok(LidarScanIngest { scans, summary })
    }

    fn scan_files(input_dir: &Path) -> AgroResult<Vec<PathBuf>> {
        let mut scan_files = Vec::new();
        for entry in walkdir::WalkDir::new(input_dir) {
            let entry = entry.map_err(|e| shared::error::AgroError::Io(e.into()))?;
            if entry.file_name().to_string_lossy().contains("scan_")
                && entry.path().extension().map_or(false, |ext| ext == "json")
            {
                scan_files.push(entry.path().to_path_buf());
            }
        }
        scan_files.sort();
        Ok(scan_files)
    }

    pub fn scan_ingest_summary_path(output_dir: &Path) -> PathBuf {
        output_dir.join("scan_ingest_summary.json")
    }

    pub fn angular_coverage_degrees(scan: &LidarScan) -> f32 {
        let mut angles: Vec<f32> = scan
            .points
            .iter()
            .filter_map(|point| {
                point
                    .angle
                    .is_finite()
                    .then(|| point.angle.rem_euclid(360.0))
            })
            .collect();

        if angles.len() < 2 {
            return 0.0;
        }

        angles.sort_by(f32::total_cmp);
        let largest_gap = angles
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .fold(0.0_f32, f32::max);
        let wrap_gap =
            angles.first().copied().unwrap_or(0.0) + 360.0 - angles.last().copied().unwrap_or(0.0);
        360.0 - largest_gap.max(wrap_gap)
    }

    async fn load_scan(scan_file: &Path) -> AgroResult<LidarScan> {
        let content = tokio::fs::read_to_string(scan_file).await?;
        let scan: LidarScan = serde_json::from_str(&content)?;
        Ok(scan)
    }

    pub fn create_occupancy_grid(
        &self,
        scans: &[LidarScan],
    ) -> AgroResult<HashMap<(i32, i32), GridCell>> {
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

    async fn save_grid_image(
        &self,
        grid: &HashMap<(i32, i32), GridCell>,
        output_dir: &PathBuf,
    ) -> AgroResult<()> {
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
        img.save(&output_path).map_err(|e| {
            shared::error::AgroError::Processing(format!("Failed to save grid image: {}", e))
        })?;

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

        info!(
            "Saved point cloud with {} points to: {:?}",
            points.len(),
            output_path
        );
        Ok(())
    }

    async fn save_obstacle_heatmap(
        &self,
        grid: &HashMap<(i32, i32), GridCell>,
        output_dir: &PathBuf,
    ) -> AgroResult<()> {
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
        let max_count = grid
            .values()
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
        img.save(&output_path).map_err(|e| {
            shared::error::AgroError::Processing(format!("Failed to save heatmap: {}", e))
        })?;

        info!("Saved obstacle heatmap to: {:?}", output_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shared::config::AgroConfig;
    use shared::schemas::{LidarPoint, LidarScan};
    use std::fs;
    use std::sync::Arc;
    use uuid::Uuid;

    fn test_mapper() -> LidarMapper {
        let config = AgroConfig::load().unwrap();
        LidarMapper {
            config: Arc::new(config),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("agbot_lidar_{name}_{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn scan_with_angles(
        scan_id: Uuid,
        captured_at: chrono::DateTime<Utc>,
        angles: &[f32],
    ) -> LidarScan {
        let points = angles
            .iter()
            .map(|angle| LidarPoint {
                timestamp: captured_at,
                angle: *angle,
                distance: 1500.0,
                quality: 30,
            })
            .collect();
        LidarScan {
            timestamp: captured_at,
            points,
            scan_id,
        }
    }

    #[test]
    fn test_create_occupancy_grid_obstacle() {
        // Single point within obstacle threshold should mark cell occupied
        let mapper = test_mapper();
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
        let mapper = test_mapper();
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

    #[test]
    fn scan_angular_coverage_uses_observed_angle_span() {
        let scan = scan_with_angles(Uuid::new_v4(), Utc::now(), &[270.0, 0.0, 180.0, 90.0]);
        assert_eq!(LidarMapper::angular_coverage_degrees(&scan), 270.0);

        let empty = scan_with_angles(Uuid::new_v4(), Utc::now(), &[]);
        assert_eq!(LidarMapper::angular_coverage_degrees(&empty), 0.0);

        let wraparound = scan_with_angles(Uuid::new_v4(), Utc::now(), &[350.0, 10.0]);
        assert_eq!(LidarMapper::angular_coverage_degrees(&wraparound), 20.0);
    }

    #[tokio::test]
    async fn ingest_scans_records_summary_and_skips_malformed() {
        let mapper = test_mapper();
        let input_dir = temp_dir("ingest_input");
        let output_dir = temp_dir("ingest_output");
        let captured_at = Utc::now();
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        let first = scan_with_angles(first_id, captured_at, &[0.0, 90.0, 180.0, 270.0]);
        let second = scan_with_angles(second_id, captured_at, &[15.0, 45.0, 75.0]);

        fs::write(
            input_dir.join("scan_first.json"),
            serde_json::to_string(&first).unwrap(),
        )
        .unwrap();
        fs::write(
            input_dir.join("scan_second.json"),
            serde_json::to_string(&second).unwrap(),
        )
        .unwrap();
        fs::write(input_dir.join("scan_bad.json"), "{not valid json").unwrap();

        let ingest = mapper.ingest_scans(&input_dir, &output_dir).await.unwrap();

        assert_eq!(ingest.scans.len(), 2);
        assert_eq!(ingest.summary.loaded_count, 2);
        assert_eq!(ingest.summary.failed_count, 1);
        assert_eq!(ingest.summary.records.len(), 2);
        assert_eq!(ingest.summary.failures.len(), 1);
        assert!(ingest.summary.failures[0].path.ends_with("scan_bad.json"));
        assert!(!ingest.summary.failures[0].error.is_empty());

        let first_record = ingest
            .summary
            .records
            .iter()
            .find(|record| record.scan_id == first_id)
            .unwrap();
        assert_eq!(first_record.captured_at, captured_at);
        assert_eq!(first_record.point_count, 4);
        assert_eq!(first_record.angular_coverage, 270.0);
        assert!(first_record.ingested_at >= captured_at);

        let persisted_path = output_dir.join("scan_ingest_summary.json");
        assert!(persisted_path.exists());
        let persisted: LidarScanIngestSummary =
            serde_json::from_str(&fs::read_to_string(persisted_path).unwrap()).unwrap();
        assert_eq!(persisted, ingest.summary);
    }
}
