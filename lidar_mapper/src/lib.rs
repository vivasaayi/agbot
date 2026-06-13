use chrono::{DateTime, Utc};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use nalgebra::{Matrix3, SymmetricEigen, Vector3};
use serde::{Deserialize, Serialize};
use shared::{
    config::AgroConfig,
    schemas::{
        assert_raster_spatial_ref, GeoBounds, LidarPoint, LidarScan, RasterResolution,
        RasterSpatialRef,
    },
    AgroResult,
};
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

pub const OCCUPANCY_GRID_LOCAL_CRS: &str = "LOCAL_LIDAR_METERS";
const OUTLIER_DISTANCE_EPSILON_METERS: f64 = 1.0e-9;
const GRID_COORDINATE_EPSILON: f64 = 1.0e-6;
const DEFAULT_LIDAR_COVERAGE_FLOOR: f32 = 0.80;
const POINT_CLOUD_FRAME_CRS_NOTE: &str =
    "LOCAL_LIDAR_METERS: x/y are derived from polar LiDAR angle and distance in meters; z=0 for 2D scans";

#[derive(Debug, Clone)]
pub struct LidarOccupancyGrid {
    pub cells: HashMap<(i32, i32), GridCell>,
    pub spatial_ref: RasterSpatialRef,
    pub resolution: RasterResolution,
    pub evidence: LidarOccupancyGridEvidence,
    pub width: u32,
    pub height: u32,
    pub min_grid_x: i32,
    pub min_grid_y: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LidarOccupancyGridEvidence {
    pub distance_threshold_m: f32,
    pub quality_threshold: u8,
    pub occupancy_threshold: f32,
    pub flip_y: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LidarCoverageStatus {
    Pass,
    LowCoverage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarCellDensity {
    pub grid_x: i32,
    pub grid_y: i32,
    pub points: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarCoverageDensityEvidence {
    pub width: u32,
    pub height: u32,
    pub min_grid_x: i32,
    pub min_grid_y: i32,
    pub total_cells: usize,
    pub covered_cells: usize,
    pub total_points: usize,
    pub max_points_per_cell: usize,
    pub mean_points_per_cell: f32,
    pub covered_cell_fraction: f32,
    pub coverage_floor: f32,
    pub status: LidarCoverageStatus,
    pub cell_densities: Vec<LidarCellDensity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarObstacleHeatmapEvidence {
    pub occupancy: LidarOccupancyGridEvidence,
    pub max_obstacle_count: usize,
    pub spatial_ref: RasterSpatialRef,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarPointCloudProvenance {
    pub scan_ids: Vec<Uuid>,
    pub captured_at: Vec<DateTime<Utc>>,
    pub point_count: usize,
    pub frame_crs_note: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LidarOutlierRemovalParams {
    pub k_neighbors: usize,
    pub stddev_multiplier: f32,
}

impl Default for LidarOutlierRemovalParams {
    fn default() -> Self {
        Self {
            k_neighbors: 8,
            stddev_multiplier: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarOutlierRemovalEvidence {
    pub points_in: usize,
    pub points_removed: usize,
    pub points_out: usize,
    pub params: LidarOutlierRemovalParams,
    pub mean_distance_threshold: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct CleanedLidarScans {
    pub scans: Vec<LidarScan>,
    pub evidence: LidarOutlierRemovalEvidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LidarPoint3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl LidarPoint3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    fn as_vector(self) -> Vector3<f64> {
        Vector3::new(self.x, self.y, self.z)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct SurfaceNormal {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl SurfaceNormal {
    fn from_vector(vector: Vector3<f64>) -> Self {
        Self {
            x: vector.x,
            y: vector.y,
            z: vector.z,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LidarNormalEstimationParams {
    pub k_neighbors: usize,
}

impl Default for LidarNormalEstimationParams {
    fn default() -> Self {
        Self { k_neighbors: 8 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarNormalEstimate {
    pub point_index: usize,
    pub neighbor_count: usize,
    pub normal: Option<SurfaceNormal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarNormalEstimationEvidence {
    pub points_in: usize,
    pub neighborhood_size: usize,
    pub normals_defined: usize,
    pub normals_undefined: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarNormalEstimationResult {
    pub estimates: Vec<LidarNormalEstimate>,
    pub evidence: LidarNormalEstimationEvidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LidarPointClass {
    Ground,
    NonGround,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LidarGroundSegmentationParams {
    pub max_ground_tilt_degrees: f64,
    pub max_ground_height_m: f64,
}

impl Default for LidarGroundSegmentationParams {
    fn default() -> Self {
        Self {
            max_ground_tilt_degrees: 30.0,
            max_ground_height_m: 0.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarPointClassification {
    pub point_index: usize,
    pub class: LidarPointClass,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarGroundSegmentationEvidence {
    pub points_in: usize,
    pub ground_count: usize,
    pub non_ground_count: usize,
    pub params: LidarGroundSegmentationParams,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarGroundSegmentationResult {
    pub classifications: Vec<LidarPointClassification>,
    pub evidence: LidarGroundSegmentationEvidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LidarObjectClusteringParams {
    pub cluster_distance_m: f64,
    pub min_cluster_size: usize,
}

impl Default for LidarObjectClusteringParams {
    fn default() -> Self {
        Self {
            cluster_distance_m: 1.0,
            min_cluster_size: 3,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LidarClusterBoundingBox {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarObjectCluster {
    pub id: usize,
    pub point_indices: Vec<usize>,
    pub point_count: usize,
    pub bbox: LidarClusterBoundingBox,
    pub centroid: LidarPoint3,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarObjectClusteringEvidence {
    pub points_in: usize,
    pub non_ground_points: usize,
    pub clusters_emitted: usize,
    pub noise_points: usize,
    pub params: LidarObjectClusteringParams,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarObjectClusteringResult {
    pub clusters: Vec<LidarObjectCluster>,
    pub evidence: LidarObjectClusteringEvidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LidarElevationRasterParams {
    pub resolution_m: f64,
    pub nodata: f32,
}

impl Default for LidarElevationRasterParams {
    fn default() -> Self {
        Self {
            resolution_m: 1.0,
            nodata: -9999.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarElevationRaster {
    pub kind: String,
    pub width: u32,
    pub height: u32,
    pub nodata: f32,
    pub values: Vec<f32>,
    pub spatial_ref: RasterSpatialRef,
}

impl LidarElevationRaster {
    pub fn value_at(&self, x: u32, y: u32) -> Option<f32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.values.get((y * self.width + x) as usize).copied()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarElevationEvidence {
    pub points_in: usize,
    pub ground_points: usize,
    pub dsm_valid_cells: usize,
    pub dtm_valid_cells: usize,
    pub params: LidarElevationRasterParams,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LidarElevationProducts {
    pub dsm: LidarElevationRaster,
    pub dtm: LidarElevationRaster,
    pub evidence: LidarElevationEvidence,
}

#[derive(Debug, Clone, Copy)]
struct IndexedLidarPoint {
    scan_index: usize,
    point_index: usize,
    x: f64,
    y: f64,
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
        let cleaned = self.clean_scans(&ingest.scans)?;
        self.save_outlier_removal_evidence(&cleaned.evidence, output_dir)
            .await?;
        let all_scans = cleaned.scans;

        // Create occupancy grid
        let grid = self.build_occupancy_grid(&all_scans)?;
        self.save_occupancy_spatial_ref(&grid, output_dir).await?;
        self.save_occupancy_grid_evidence(&grid.evidence, output_dir)
            .await?;
        let coverage_evidence =
            self.coverage_density_evidence(&grid, DEFAULT_LIDAR_COVERAGE_FLOOR)?;
        self.save_coverage_density_evidence(&coverage_evidence, output_dir)
            .await?;

        // Save grid as image
        self.save_grid_image(&grid.cells, output_dir).await?;

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

    pub fn clean_scans(&self, scans: &[LidarScan]) -> AgroResult<CleanedLidarScans> {
        self.remove_statistical_outliers(scans, LidarOutlierRemovalParams::default())
    }

    pub fn remove_statistical_outliers(
        &self,
        scans: &[LidarScan],
        params: LidarOutlierRemovalParams,
    ) -> AgroResult<CleanedLidarScans> {
        Self::validate_outlier_params(params)?;
        let indexed_points = Self::indexed_lidar_points(scans);
        let points_in = indexed_points.len();

        if points_in <= params.k_neighbors || points_in < 2 {
            return Ok(Self::unchanged_cleaned_scans(
                scans, params, points_in, None,
            ));
        }

        let mean_distances = Self::mean_neighbor_distances(&indexed_points, params.k_neighbors);
        let mean = mean_distances.iter().sum::<f64>() / mean_distances.len() as f64;
        let variance = mean_distances
            .iter()
            .map(|distance| {
                let delta = distance - mean;
                delta * delta
            })
            .sum::<f64>()
            / mean_distances.len() as f64;
        let threshold = mean + params.stddev_multiplier as f64 * variance.sqrt();

        let mut keep_points: Vec<Vec<bool>> = scans
            .iter()
            .map(|scan| vec![true; scan.points.len()])
            .collect();
        let mut points_removed = 0;
        for (point, mean_distance) in indexed_points.iter().zip(mean_distances.iter()) {
            if *mean_distance > threshold + OUTLIER_DISTANCE_EPSILON_METERS {
                keep_points[point.scan_index][point.point_index] = false;
                points_removed += 1;
            }
        }

        let cleaned_scans = scans
            .iter()
            .enumerate()
            .map(|(scan_index, scan)| {
                let mut cleaned = scan.clone();
                cleaned.points = scan
                    .points
                    .iter()
                    .enumerate()
                    .filter_map(|(point_index, point)| {
                        keep_points[scan_index][point_index].then(|| point.clone())
                    })
                    .collect();
                cleaned
            })
            .collect();

        Ok(CleanedLidarScans {
            scans: cleaned_scans,
            evidence: LidarOutlierRemovalEvidence {
                points_in,
                points_removed,
                points_out: points_in - points_removed,
                params,
                mean_distance_threshold: Some(threshold),
            },
        })
    }

    fn validate_outlier_params(params: LidarOutlierRemovalParams) -> AgroResult<()> {
        if params.k_neighbors == 0 {
            return Err(shared::error::AgroError::Processing(
                "LiDAR outlier removal requires at least one neighbor".into(),
            ));
        }
        if !params.stddev_multiplier.is_finite() || params.stddev_multiplier < 0.0 {
            return Err(shared::error::AgroError::Processing(
                "LiDAR outlier removal requires a non-negative stddev multiplier".into(),
            ));
        }
        Ok(())
    }

    fn unchanged_cleaned_scans(
        scans: &[LidarScan],
        params: LidarOutlierRemovalParams,
        points_in: usize,
        mean_distance_threshold: Option<f64>,
    ) -> CleanedLidarScans {
        CleanedLidarScans {
            scans: scans.to_vec(),
            evidence: LidarOutlierRemovalEvidence {
                points_in,
                points_removed: 0,
                points_out: points_in,
                params,
                mean_distance_threshold,
            },
        }
    }

    fn indexed_lidar_points(scans: &[LidarScan]) -> Vec<IndexedLidarPoint> {
        scans
            .iter()
            .enumerate()
            .flat_map(|(scan_index, scan)| {
                scan.points
                    .iter()
                    .enumerate()
                    .map(move |(point_index, point)| {
                        let (x, y) = Self::lidar_point_xy(point);
                        IndexedLidarPoint {
                            scan_index,
                            point_index,
                            x,
                            y,
                        }
                    })
            })
            .collect()
    }

    fn lidar_point_xy(point: &LidarPoint) -> (f64, f64) {
        let angle_rad = (point.angle as f64).to_radians();
        let distance_m = point.distance as f64 / 1000.0;
        (distance_m * angle_rad.cos(), distance_m * angle_rad.sin())
    }

    fn mean_neighbor_distances(points: &[IndexedLidarPoint], k_neighbors: usize) -> Vec<f64> {
        points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let mut distances: Vec<f64> = points
                    .iter()
                    .enumerate()
                    .filter_map(|(other_index, other)| {
                        (other_index != index).then(|| {
                            let dx = point.x - other.x;
                            let dy = point.y - other.y;
                            dx.hypot(dy)
                        })
                    })
                    .collect();
                distances.sort_by(f64::total_cmp);
                distances.iter().take(k_neighbors).sum::<f64>() / k_neighbors as f64
            })
            .collect()
    }

    async fn save_outlier_removal_evidence(
        &self,
        evidence: &LidarOutlierRemovalEvidence,
        output_dir: &PathBuf,
    ) -> AgroResult<()> {
        let output_path = output_dir.join("lidar_outlier_removal_evidence.json");
        let content = serde_json::to_vec_pretty(evidence)?;
        tokio::fs::write(&output_path, content).await?;
        info!("Saved LiDAR outlier removal evidence to: {:?}", output_path);
        Ok(())
    }

    pub fn estimate_surface_normals(
        &self,
        points: &[LidarPoint3],
        params: LidarNormalEstimationParams,
    ) -> AgroResult<LidarNormalEstimationResult> {
        Self::validate_normal_params(params)?;

        let mut estimates = Vec::with_capacity(points.len());
        let mut normals_defined = 0;
        let mut normals_undefined = 0;
        for point_index in 0..points.len() {
            let neighbor_indices =
                Self::nearest_point_indices(points, point_index, params.k_neighbors);
            if neighbor_indices.len() < params.k_neighbors {
                estimates.push(LidarNormalEstimate {
                    point_index,
                    neighbor_count: neighbor_indices.len(),
                    normal: None,
                });
                normals_undefined += 1;
                continue;
            }

            let neighbors: Vec<_> = neighbor_indices
                .iter()
                .map(|index| points[*index])
                .collect();
            let normal = Self::estimate_normal_from_neighbors(&neighbors);
            if normal.is_some() {
                normals_defined += 1;
            } else {
                normals_undefined += 1;
            }
            estimates.push(LidarNormalEstimate {
                point_index,
                neighbor_count: neighbor_indices.len(),
                normal,
            });
        }

        Ok(LidarNormalEstimationResult {
            estimates,
            evidence: LidarNormalEstimationEvidence {
                points_in: points.len(),
                neighborhood_size: params.k_neighbors,
                normals_defined,
                normals_undefined,
            },
        })
    }

    fn validate_normal_params(params: LidarNormalEstimationParams) -> AgroResult<()> {
        if params.k_neighbors == 0 {
            return Err(shared::error::AgroError::Processing(
                "LiDAR normal estimation requires at least one neighbor".into(),
            ));
        }
        Ok(())
    }

    fn nearest_point_indices(
        points: &[LidarPoint3],
        point_index: usize,
        k_neighbors: usize,
    ) -> Vec<usize> {
        let point = points[point_index];
        let mut distances: Vec<(usize, f64)> = points
            .iter()
            .enumerate()
            .filter_map(|(other_index, other)| {
                (other_index != point_index).then(|| {
                    let dx = point.x - other.x;
                    let dy = point.y - other.y;
                    let dz = point.z - other.z;
                    (other_index, dx.hypot(dy).hypot(dz))
                })
            })
            .collect();
        distances.sort_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        distances
            .into_iter()
            .take(k_neighbors)
            .map(|(index, _)| index)
            .collect()
    }

    fn estimate_normal_from_neighbors(neighbors: &[LidarPoint3]) -> Option<SurfaceNormal> {
        if neighbors.len() < 3 {
            return None;
        }

        let centroid = neighbors
            .iter()
            .map(|point| point.as_vector())
            .fold(Vector3::zeros(), |acc, point| acc + point)
            / neighbors.len() as f64;
        let covariance = neighbors.iter().fold(Matrix3::zeros(), |acc, point| {
            let centered = point.as_vector() - centroid;
            acc + centered * centered.transpose()
        }) / neighbors.len() as f64;

        let eigen = SymmetricEigen::new(covariance);
        let min_index = (0..3)
            .min_by(|left, right| eigen.eigenvalues[*left].total_cmp(&eigen.eigenvalues[*right]))
            .unwrap_or(0);
        let vector = eigen.eigenvectors.column(min_index).into_owned();
        let norm = vector.norm();
        if norm <= OUTLIER_DISTANCE_EPSILON_METERS || !norm.is_finite() {
            return None;
        }

        let mut normal = vector / norm;
        if normal.z < 0.0
            || (normal.z.abs() <= OUTLIER_DISTANCE_EPSILON_METERS
                && (normal.y < 0.0
                    || (normal.y.abs() <= OUTLIER_DISTANCE_EPSILON_METERS && normal.x < 0.0)))
        {
            normal = -normal;
        }
        Some(SurfaceNormal::from_vector(normal))
    }

    pub fn segment_ground_points(
        &self,
        points: &[LidarPoint3],
        normals: &[LidarNormalEstimate],
        params: LidarGroundSegmentationParams,
    ) -> AgroResult<LidarGroundSegmentationResult> {
        Self::validate_ground_segmentation_params(params)?;
        if points.len() != normals.len() {
            return Err(shared::error::AgroError::Processing(format!(
                "LiDAR ground segmentation expected {} normals for {} points",
                points.len(),
                normals.len()
            )));
        }

        let min_ground_normal_z = params.max_ground_tilt_degrees.to_radians().cos();
        let mut ground_count = 0;
        let mut classifications = Vec::with_capacity(points.len());
        for (point_index, (point, estimate)) in points.iter().zip(normals.iter()).enumerate() {
            if estimate.point_index != point_index {
                return Err(shared::error::AgroError::Processing(format!(
                    "LiDAR ground segmentation normal index mismatch at point {point_index}"
                )));
            }
            let is_ground = estimate
                .normal
                .is_some_and(|normal| normal.z.is_finite() && normal.z >= min_ground_normal_z)
                && point.z.is_finite()
                && point.z <= params.max_ground_height_m;
            let class = if is_ground {
                ground_count += 1;
                LidarPointClass::Ground
            } else {
                LidarPointClass::NonGround
            };
            classifications.push(LidarPointClassification { point_index, class });
        }

        if ground_count == 0 {
            return Err(shared::error::AgroError::Processing(
                "LiDAR ground segmentation found no ground surface".into(),
            ));
        }

        let non_ground_count = points.len() - ground_count;
        Ok(LidarGroundSegmentationResult {
            classifications,
            evidence: LidarGroundSegmentationEvidence {
                points_in: points.len(),
                ground_count,
                non_ground_count,
                params,
            },
        })
    }

    fn validate_ground_segmentation_params(
        params: LidarGroundSegmentationParams,
    ) -> AgroResult<()> {
        if !params.max_ground_tilt_degrees.is_finite()
            || !(0.0..=90.0).contains(&params.max_ground_tilt_degrees)
        {
            return Err(shared::error::AgroError::Processing(
                "LiDAR ground segmentation requires a ground tilt in [0, 90] degrees".into(),
            ));
        }
        if !params.max_ground_height_m.is_finite() {
            return Err(shared::error::AgroError::Processing(
                "LiDAR ground segmentation requires a finite ground height".into(),
            ));
        }
        Ok(())
    }

    pub fn cluster_non_ground_objects(
        &self,
        points: &[LidarPoint3],
        classifications: &[LidarPointClassification],
        params: LidarObjectClusteringParams,
    ) -> AgroResult<LidarObjectClusteringResult> {
        Self::validate_object_clustering_params(params)?;
        if points.len() != classifications.len() {
            return Err(shared::error::AgroError::Processing(format!(
                "LiDAR object clustering expected {} classifications for {} points",
                points.len(),
                classifications.len()
            )));
        }
        for point in points {
            if !point.x.is_finite() || !point.y.is_finite() || !point.z.is_finite() {
                return Err(shared::error::AgroError::Processing(
                    "LiDAR object clustering requires finite point coordinates".into(),
                ));
            }
        }

        let mut non_ground_indices = Vec::new();
        for (expected_index, classification) in classifications.iter().enumerate() {
            if classification.point_index != expected_index {
                return Err(shared::error::AgroError::Processing(format!(
                    "LiDAR object clustering classification index mismatch at point {expected_index}"
                )));
            }
            if classification.class == LidarPointClass::NonGround {
                non_ground_indices.push(expected_index);
            }
        }

        let mut visited = vec![false; points.len()];
        let mut clusters = Vec::new();
        let mut noise_points = 0usize;

        for &seed_index in &non_ground_indices {
            if visited[seed_index] {
                continue;
            }

            let mut queue = vec![seed_index];
            let mut cursor = 0usize;
            visited[seed_index] = true;
            while cursor < queue.len() {
                let current_index = queue[cursor];
                cursor += 1;
                for &candidate_index in &non_ground_indices {
                    if visited[candidate_index] {
                        continue;
                    }
                    if Self::point_distance(points[current_index], points[candidate_index])
                        <= params.cluster_distance_m
                    {
                        visited[candidate_index] = true;
                        queue.push(candidate_index);
                    }
                }
            }

            queue.sort_unstable();
            if queue.len() >= params.min_cluster_size {
                clusters.push(Self::build_object_cluster(clusters.len(), &queue, points)?);
            } else {
                noise_points += queue.len();
            }
        }

        Ok(LidarObjectClusteringResult {
            evidence: LidarObjectClusteringEvidence {
                points_in: points.len(),
                non_ground_points: non_ground_indices.len(),
                clusters_emitted: clusters.len(),
                noise_points,
                params,
            },
            clusters,
        })
    }

    fn validate_object_clustering_params(params: LidarObjectClusteringParams) -> AgroResult<()> {
        if !params.cluster_distance_m.is_finite() || params.cluster_distance_m <= 0.0 {
            return Err(shared::error::AgroError::Processing(
                "LiDAR object clustering requires a positive finite cluster distance".into(),
            ));
        }
        if params.min_cluster_size == 0 {
            return Err(shared::error::AgroError::Processing(
                "LiDAR object clustering requires min_cluster_size >= 1".into(),
            ));
        }
        Ok(())
    }

    fn build_object_cluster(
        id: usize,
        point_indices: &[usize],
        points: &[LidarPoint3],
    ) -> AgroResult<LidarObjectCluster> {
        let first_index = *point_indices.first().ok_or_else(|| {
            shared::error::AgroError::Processing(
                "LiDAR object clustering cannot build an empty cluster".into(),
            )
        })?;
        let first = points[first_index];
        let mut bbox = LidarClusterBoundingBox {
            min_x: first.x,
            min_y: first.y,
            min_z: first.z,
            max_x: first.x,
            max_y: first.y,
            max_z: first.z,
        };
        let mut sum = Vector3::zeros();
        for &point_index in point_indices {
            let point = points[point_index];
            bbox.min_x = bbox.min_x.min(point.x);
            bbox.min_y = bbox.min_y.min(point.y);
            bbox.min_z = bbox.min_z.min(point.z);
            bbox.max_x = bbox.max_x.max(point.x);
            bbox.max_y = bbox.max_y.max(point.y);
            bbox.max_z = bbox.max_z.max(point.z);
            sum += point.as_vector();
        }
        let centroid_vector = sum / point_indices.len() as f64;

        Ok(LidarObjectCluster {
            id,
            point_indices: point_indices.to_vec(),
            point_count: point_indices.len(),
            bbox,
            centroid: LidarPoint3::new(centroid_vector.x, centroid_vector.y, centroid_vector.z),
        })
    }

    fn point_distance(left: LidarPoint3, right: LidarPoint3) -> f64 {
        let dx = left.x - right.x;
        let dy = left.y - right.y;
        let dz = left.z - right.z;
        dx.hypot(dy).hypot(dz)
    }

    pub fn build_elevation_products(
        &self,
        points: &[LidarPoint3],
        classifications: &[LidarPointClassification],
        params: LidarElevationRasterParams,
    ) -> AgroResult<LidarElevationProducts> {
        Self::validate_elevation_params(params)?;
        if points.is_empty() {
            return Err(shared::error::AgroError::Processing(
                "LiDAR elevation products require at least one point".into(),
            ));
        }
        if points.len() != classifications.len() {
            return Err(shared::error::AgroError::Processing(format!(
                "LiDAR elevation products expected {} classifications for {} points",
                points.len(),
                classifications.len()
            )));
        }

        let cells = Self::elevation_cell_bounds(points, params.resolution_m)?;
        let (min_grid_x, min_grid_y, width, height) = cells;
        let spatial_ref = Self::elevation_spatial_ref(
            min_grid_x,
            min_grid_y,
            width,
            height,
            params.resolution_m,
        )?;
        let cell_count = (width * height) as usize;
        let mut dsm = vec![None; cell_count];
        let mut dtm = vec![None; cell_count];
        let mut ground_points = 0;

        for (point_index, (point, classification)) in
            points.iter().zip(classifications.iter()).enumerate()
        {
            if classification.point_index != point_index {
                return Err(shared::error::AgroError::Processing(format!(
                    "LiDAR elevation classification index mismatch at point {point_index}"
                )));
            }
            let x = (point.x / params.resolution_m).floor() as i32;
            let y = (point.y / params.resolution_m).floor() as i32;
            let raster_index = ((y - min_grid_y) as u32 * width + (x - min_grid_x) as u32) as usize;
            dsm[raster_index] = Some(
                dsm[raster_index]
                    .map(|current: f64| current.max(point.z))
                    .unwrap_or(point.z),
            );
            if classification.class == LidarPointClass::Ground {
                ground_points += 1;
                dtm[raster_index] = Some(
                    dtm[raster_index]
                        .map(|current: f64| current.min(point.z))
                        .unwrap_or(point.z),
                );
            }
        }

        let dsm_valid_cells = dsm.iter().filter(|value| value.is_some()).count();
        let dtm_valid_cells = dtm.iter().filter(|value| value.is_some()).count();
        let dsm_values = Self::materialize_elevation_values(dsm, params.nodata);
        let dtm_values = Self::materialize_elevation_values(dtm, params.nodata);

        Ok(LidarElevationProducts {
            dsm: LidarElevationRaster {
                kind: "dsm".to_string(),
                width,
                height,
                nodata: params.nodata,
                values: dsm_values,
                spatial_ref: spatial_ref.clone(),
            },
            dtm: LidarElevationRaster {
                kind: "dtm".to_string(),
                width,
                height,
                nodata: params.nodata,
                values: dtm_values,
                spatial_ref,
            },
            evidence: LidarElevationEvidence {
                points_in: points.len(),
                ground_points,
                dsm_valid_cells,
                dtm_valid_cells,
                params,
            },
        })
    }

    fn validate_elevation_params(params: LidarElevationRasterParams) -> AgroResult<()> {
        if !params.resolution_m.is_finite() || params.resolution_m <= 0.0 {
            return Err(shared::error::AgroError::Processing(
                "LiDAR elevation rasterization requires a positive resolution".into(),
            ));
        }
        if !params.nodata.is_finite() {
            return Err(shared::error::AgroError::Processing(
                "LiDAR elevation rasterization requires a finite nodata value".into(),
            ));
        }
        Ok(())
    }

    fn elevation_cell_bounds(
        points: &[LidarPoint3],
        resolution_m: f64,
    ) -> AgroResult<(i32, i32, u32, u32)> {
        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        let mut min_y = i32::MAX;
        let mut max_y = i32::MIN;
        for point in points {
            if !point.x.is_finite() || !point.y.is_finite() || !point.z.is_finite() {
                return Err(shared::error::AgroError::Processing(
                    "LiDAR elevation rasterization requires finite point coordinates".into(),
                ));
            }
            let x = (point.x / resolution_m).floor() as i32;
            let y = (point.y / resolution_m).floor() as i32;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        Ok((
            min_x,
            min_y,
            (max_x - min_x + 1) as u32,
            (max_y - min_y + 1) as u32,
        ))
    }

    fn elevation_spatial_ref(
        min_grid_x: i32,
        min_grid_y: i32,
        width: u32,
        height: u32,
        resolution_m: f64,
    ) -> AgroResult<RasterSpatialRef> {
        let origin_x = min_grid_x as f64 * resolution_m;
        let origin_y = min_grid_y as f64 * resolution_m;
        let spatial_ref = RasterSpatialRef {
            georeferenced: true,
            crs: Some(OCCUPANCY_GRID_LOCAL_CRS.to_string()),
            bbox: Some(GeoBounds {
                min_lon: origin_x,
                min_lat: origin_y,
                max_lon: origin_x + width as f64 * resolution_m,
                max_lat: origin_y + height as f64 * resolution_m,
            }),
            geo_transform: Some([origin_x, resolution_m, 0.0, origin_y, 0.0, resolution_m]),
            resolution: Some(RasterResolution {
                x: resolution_m,
                y: resolution_m,
            }),
        };

        assert_raster_spatial_ref(Some(&spatial_ref), width, height).map_err(|e| {
            shared::error::AgroError::Processing(format!(
                "LiDAR elevation spatial reference assertion failed: {e}"
            ))
        })
    }

    fn materialize_elevation_values(values: Vec<Option<f64>>, nodata: f32) -> Vec<f32> {
        values
            .into_iter()
            .map(|value| value.map(|z| z as f32).unwrap_or(nodata))
            .collect()
    }

    pub fn create_occupancy_grid(
        &self,
        scans: &[LidarScan],
    ) -> AgroResult<HashMap<(i32, i32), GridCell>> {
        Ok(self.build_occupancy_grid(scans)?.cells)
    }

    pub fn build_occupancy_grid(&self, scans: &[LidarScan]) -> AgroResult<LidarOccupancyGrid> {
        let resolution = self.config.processing.lidar_grid_resolution;
        let spatial_resolution = Self::assert_positive_grid_resolution(resolution)?;
        let evidence = self.occupancy_grid_evidence()?;
        let mut grid: HashMap<(i32, i32), GridCell> = HashMap::new();

        info!("Creating occupancy grid with resolution: {} m", resolution);

        for scan in scans {
            for point in &scan.points {
                // Convert polar to cartesian coordinates
                let angle_rad = point.angle.to_radians();
                let distance_m = point.distance / 1000.0; // Convert mm to m

                let x = distance_m * angle_rad.cos();
                let mut y = distance_m * angle_rad.sin();
                if evidence.flip_y {
                    y = -y;
                }

                // Convert to grid coordinates
                let grid_x = Self::grid_coordinate(x as f64, resolution);
                let grid_y = Self::grid_coordinate(y as f64, resolution);

                let cell = grid.entry((grid_x, grid_y)).or_default();
                cell.total_observations += 1;

                // Count as obstacle if within threshold
                if distance_m < evidence.distance_threshold_m
                    && (point.quality as u8) > evidence.quality_threshold
                {
                    cell.obstacle_count += 1;
                }
            }
        }

        // Determine final occupancy based on threshold
        for cell in grid.values_mut() {
            if cell.total_observations > 0 {
                let ratio = cell.obstacle_count as f32 / cell.total_observations as f32;
                cell.occupied = ratio > evidence.occupancy_threshold;
            }
        }
        info!("Generated occupancy grid with {} cells", grid.len());
        let (min_grid_x, min_grid_y, width, height) = Self::occupancy_grid_dimensions(&grid);
        let spatial_ref =
            Self::occupancy_grid_spatial_ref(min_grid_x, min_grid_y, width, height, resolution)?;
        Ok(LidarOccupancyGrid {
            cells: grid,
            spatial_ref,
            resolution: spatial_resolution,
            evidence,
            width,
            height,
            min_grid_x,
            min_grid_y,
        })
    }

    fn occupancy_grid_evidence(&self) -> AgroResult<LidarOccupancyGridEvidence> {
        let distance_threshold_m = self.config.processing.lidar_obstacle_distance_threshold;
        if !distance_threshold_m.is_finite() || distance_threshold_m <= 0.0 {
            return Err(shared::error::AgroError::Processing(
                "LiDAR occupancy grid requires a positive distance threshold".into(),
            ));
        }

        let occupancy_threshold = self.config.processing.lidar_occupancy_threshold;
        if !occupancy_threshold.is_finite() || !(0.0..=1.0).contains(&occupancy_threshold) {
            return Err(shared::error::AgroError::Processing(
                "LiDAR occupancy threshold must be in the range [0, 1]".into(),
            ));
        }

        Ok(LidarOccupancyGridEvidence {
            distance_threshold_m,
            quality_threshold: self.config.processing.lidar_quality_threshold,
            occupancy_threshold,
            flip_y: self.config.processing.lidar_image_flip_y,
        })
    }

    fn grid_coordinate(value_m: f64, resolution_m: f32) -> i32 {
        let scaled = value_m / resolution_m as f64;
        let nearest = scaled.round();
        if (scaled - nearest).abs() <= GRID_COORDINATE_EPSILON {
            nearest as i32
        } else {
            scaled.floor() as i32
        }
    }

    fn assert_positive_grid_resolution(resolution: f32) -> AgroResult<RasterResolution> {
        if resolution.is_finite() && resolution > 0.0 {
            Ok(RasterResolution {
                x: resolution as f64,
                y: resolution as f64,
            })
        } else {
            Err(shared::error::AgroError::Processing(
                "LiDAR occupancy grid requires a positive resolution".into(),
            ))
        }
    }

    fn occupancy_grid_dimensions(grid: &HashMap<(i32, i32), GridCell>) -> (i32, i32, u32, u32) {
        if grid.is_empty() {
            return (0, 0, 1, 1);
        }

        let min_x = grid.keys().map(|(x, _)| *x).min().unwrap_or(0);
        let max_x = grid.keys().map(|(x, _)| *x).max().unwrap_or(0);
        let min_y = grid.keys().map(|(_, y)| *y).min().unwrap_or(0);
        let max_y = grid.keys().map(|(_, y)| *y).max().unwrap_or(0);

        (
            min_x,
            min_y,
            (max_x - min_x + 1) as u32,
            (max_y - min_y + 1) as u32,
        )
    }

    fn occupancy_grid_spatial_ref(
        min_grid_x: i32,
        min_grid_y: i32,
        width: u32,
        height: u32,
        resolution: f32,
    ) -> AgroResult<RasterSpatialRef> {
        let resolution = Self::assert_positive_grid_resolution(resolution)?;
        let origin_x = min_grid_x as f64 * resolution.x;
        let origin_y = min_grid_y as f64 * resolution.y;
        let bbox = GeoBounds {
            min_lon: origin_x,
            min_lat: origin_y,
            max_lon: origin_x + width as f64 * resolution.x,
            max_lat: origin_y + height as f64 * resolution.y,
        };
        let spatial_ref = RasterSpatialRef {
            georeferenced: true,
            crs: Some(OCCUPANCY_GRID_LOCAL_CRS.to_string()),
            bbox: Some(bbox),
            geo_transform: Some([origin_x, resolution.x, 0.0, origin_y, 0.0, resolution.y]),
            resolution: Some(resolution),
        };

        assert_raster_spatial_ref(Some(&spatial_ref), width, height).map_err(|e| {
            shared::error::AgroError::Processing(format!(
                "LiDAR occupancy grid spatial reference assertion failed: {e}"
            ))
        })
    }

    async fn save_occupancy_spatial_ref(
        &self,
        grid: &LidarOccupancyGrid,
        output_dir: &PathBuf,
    ) -> AgroResult<()> {
        let output_path = output_dir.join("occupancy_grid_spatial_ref.json");
        let content = serde_json::to_vec_pretty(&grid.spatial_ref)?;
        tokio::fs::write(&output_path, content).await?;
        info!(
            "Saved occupancy grid spatial reference to: {:?}",
            output_path
        );
        Ok(())
    }

    async fn save_occupancy_grid_evidence(
        &self,
        evidence: &LidarOccupancyGridEvidence,
        output_dir: &PathBuf,
    ) -> AgroResult<()> {
        let output_path = output_dir.join("lidar_occupancy_grid_evidence.json");
        let content = serde_json::to_vec_pretty(evidence)?;
        tokio::fs::write(&output_path, content).await?;
        info!("Saved LiDAR occupancy grid evidence to: {:?}", output_path);
        Ok(())
    }

    pub fn coverage_density_evidence(
        &self,
        grid: &LidarOccupancyGrid,
        coverage_floor: f32,
    ) -> AgroResult<LidarCoverageDensityEvidence> {
        let coverage_floor = Self::normalize_coverage_floor(coverage_floor);
        let width = grid.width.max(1);
        let height = grid.height.max(1);
        let total_cells = (width * height) as usize;
        let mut covered_cells = 0usize;
        let mut total_points = 0usize;
        let mut max_points_per_cell = 0usize;
        let mut cell_densities = Vec::with_capacity(total_cells);

        for y_offset in 0..height {
            for x_offset in 0..width {
                let grid_x = grid.min_grid_x + x_offset as i32;
                let grid_y = grid.min_grid_y + y_offset as i32;
                let points = grid
                    .cells
                    .get(&(grid_x, grid_y))
                    .map(|cell| cell.total_observations)
                    .unwrap_or(0);
                if points > 0 {
                    covered_cells += 1;
                }
                total_points += points;
                max_points_per_cell = max_points_per_cell.max(points);
                cell_densities.push(LidarCellDensity {
                    grid_x,
                    grid_y,
                    points,
                });
            }
        }

        let covered_cell_fraction = if total_cells == 0 {
            0.0
        } else {
            covered_cells as f32 / total_cells as f32
        };
        let mean_points_per_cell = if total_cells == 0 {
            0.0
        } else {
            total_points as f32 / total_cells as f32
        };
        let status = if covered_cell_fraction >= coverage_floor {
            LidarCoverageStatus::Pass
        } else {
            LidarCoverageStatus::LowCoverage
        };

        Ok(LidarCoverageDensityEvidence {
            width,
            height,
            min_grid_x: grid.min_grid_x,
            min_grid_y: grid.min_grid_y,
            total_cells,
            covered_cells,
            total_points,
            max_points_per_cell,
            mean_points_per_cell,
            covered_cell_fraction,
            coverage_floor,
            status,
            cell_densities,
        })
    }

    fn normalize_coverage_floor(coverage_floor: f32) -> f32 {
        if coverage_floor.is_finite() {
            coverage_floor.clamp(0.0, 1.0)
        } else {
            DEFAULT_LIDAR_COVERAGE_FLOOR
        }
    }

    async fn save_coverage_density_evidence(
        &self,
        evidence: &LidarCoverageDensityEvidence,
        output_dir: &PathBuf,
    ) -> AgroResult<()> {
        let output_path = output_dir.join("lidar_coverage_density_evidence.json");
        let content = serde_json::to_vec_pretty(evidence)?;
        tokio::fs::write(&output_path, content).await?;
        info!(
            "Saved LiDAR coverage density evidence to: {:?}",
            output_path
        );
        Ok(())
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
        let provenance = Self::point_cloud_provenance(scans);

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
        self.save_point_cloud_provenance(&provenance, output_dir)
            .await?;

        info!(
            "Saved point cloud with {} points to: {:?}",
            points.len(),
            output_path
        );
        Ok(())
    }

    fn point_cloud_provenance(scans: &[LidarScan]) -> LidarPointCloudProvenance {
        LidarPointCloudProvenance {
            scan_ids: scans.iter().map(|scan| scan.scan_id).collect(),
            captured_at: scans.iter().map(|scan| scan.timestamp).collect(),
            point_count: scans.iter().map(|scan| scan.points.len()).sum(),
            frame_crs_note: POINT_CLOUD_FRAME_CRS_NOTE.to_string(),
        }
    }

    async fn save_point_cloud_provenance(
        &self,
        provenance: &LidarPointCloudProvenance,
        output_dir: &PathBuf,
    ) -> AgroResult<()> {
        let output_path = output_dir.join("point_cloud_provenance.json");
        let content = serde_json::to_vec_pretty(provenance)?;
        tokio::fs::write(&output_path, content).await?;
        info!("Saved point cloud provenance to: {:?}", output_path);
        Ok(())
    }

    async fn save_obstacle_heatmap(
        &self,
        grid: &LidarOccupancyGrid,
        output_dir: &PathBuf,
    ) -> AgroResult<()> {
        let width = grid.width.max(1);
        let height = grid.height.max(1);
        let max_obstacle_count = grid
            .cells
            .values()
            .map(|cell| cell.obstacle_count)
            .max()
            .unwrap_or(0);
        let low_color = Self::obstacle_heatmap_color(0, max_obstacle_count);
        let mut img = image::ImageBuffer::from_pixel(width, height, image::Rgb(low_color));

        for ((grid_x, grid_y), cell) in &grid.cells {
            let pixel_x = (*grid_x - grid.min_grid_x) as u32;
            let py = (*grid_y - grid.min_grid_y) as u32;
            let pixel_y = if grid.evidence.flip_y {
                height - 1 - py
            } else {
                py
            };

            let color = Self::obstacle_heatmap_color(cell.obstacle_count, max_obstacle_count);

            if pixel_x < width && pixel_y < height {
                img.put_pixel(pixel_x, pixel_y, image::Rgb(color));
            }
        }

        let output_path = output_dir.join("obstacle_heatmap.png");
        img.save(&output_path).map_err(|e| {
            shared::error::AgroError::Processing(format!("Failed to save heatmap: {}", e))
        })?;

        info!("Saved obstacle heatmap to: {:?}", output_path);
        self.save_obstacle_heatmap_evidence(
            &Self::obstacle_heatmap_evidence(grid, max_obstacle_count),
            output_dir,
        )
        .await?;
        Ok(())
    }

    fn obstacle_heatmap_color(obstacle_count: usize, max_obstacle_count: usize) -> [u8; 3] {
        if max_obstacle_count == 0 {
            return [0, 0, 255];
        }

        let intensity = (obstacle_count as f32 / max_obstacle_count as f32 * 255.0).round() as u8;
        [intensity, 0, 255u8.saturating_sub(intensity)]
    }

    fn obstacle_heatmap_evidence(
        grid: &LidarOccupancyGrid,
        max_obstacle_count: usize,
    ) -> LidarObstacleHeatmapEvidence {
        LidarObstacleHeatmapEvidence {
            occupancy: grid.evidence,
            max_obstacle_count,
            spatial_ref: grid.spatial_ref.clone(),
            width: grid.width,
            height: grid.height,
        }
    }

    async fn save_obstacle_heatmap_evidence(
        &self,
        evidence: &LidarObstacleHeatmapEvidence,
        output_dir: &PathBuf,
    ) -> AgroResult<()> {
        let output_path = output_dir.join("obstacle_heatmap_evidence.json");
        let content = serde_json::to_vec_pretty(evidence)?;
        tokio::fs::write(&output_path, content).await?;
        info!("Saved obstacle heatmap evidence to: {:?}", output_path);
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

    fn test_mapper_with_resolution(resolution: f32) -> LidarMapper {
        let mut config = AgroConfig::load().unwrap();
        config.processing.lidar_grid_resolution = resolution;
        LidarMapper {
            config: Arc::new(config),
        }
    }

    fn test_mapper_with_occupancy_controls(
        resolution: f32,
        distance_threshold_m: f32,
        quality_threshold: u8,
        occupancy_threshold: f32,
        flip_y: bool,
    ) -> LidarMapper {
        let mut config = AgroConfig::load().unwrap();
        config.processing.lidar_grid_resolution = resolution;
        config.processing.lidar_obstacle_distance_threshold = distance_threshold_m;
        config.processing.lidar_quality_threshold = quality_threshold;
        config.processing.lidar_occupancy_threshold = occupancy_threshold;
        config.processing.lidar_image_flip_y = flip_y;
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

    fn point(angle: f32, distance: f32) -> LidarPoint {
        LidarPoint {
            timestamp: Utc::now(),
            angle,
            distance,
            quality: 30,
        }
    }

    fn pcd_data_lines(content: &str) -> Vec<&str> {
        content
            .lines()
            .skip_while(|line| *line != "DATA ascii")
            .skip(1)
            .filter(|line| !line.trim().is_empty())
            .collect()
    }

    fn heatmap_test_grid(
        mapper: &LidarMapper,
        cells: Vec<((i32, i32), GridCell)>,
        width: u32,
        height: u32,
    ) -> LidarOccupancyGrid {
        LidarOccupancyGrid {
            cells: cells.into_iter().collect(),
            spatial_ref: LidarMapper::occupancy_grid_spatial_ref(0, 0, width, height, 1.0).unwrap(),
            resolution: RasterResolution { x: 1.0, y: 1.0 },
            evidence: mapper.occupancy_grid_evidence().unwrap(),
            width,
            height,
            min_grid_x: 0,
            min_grid_y: 0,
        }
    }

    fn normal_estimate(point_index: usize, z: f64) -> LidarNormalEstimate {
        LidarNormalEstimate {
            point_index,
            neighbor_count: 4,
            normal: Some(SurfaceNormal { x: 0.0, y: 0.0, z }),
        }
    }

    fn classification(point_index: usize, class: LidarPointClass) -> LidarPointClassification {
        LidarPointClassification { point_index, class }
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

    #[test]
    fn build_occupancy_grid_asserts_spatial_ref_from_cell_extent() {
        let mapper = test_mapper_with_resolution(1.0);
        let scan = LidarScan {
            timestamp: Utc::now(),
            points: vec![point(0.0, 1000.0), point(90.0, 2000.0)],
            scan_id: Uuid::new_v4(),
        };

        let product = mapper.build_occupancy_grid(&[scan]).unwrap();

        assert_eq!(product.cells.len(), 2);
        assert_eq!(product.width, 2);
        assert_eq!(product.height, 3);
        assert_eq!(product.min_grid_x, 0);
        assert_eq!(product.min_grid_y, 0);
        assert_eq!(
            product.spatial_ref.crs.as_deref(),
            Some("LOCAL_LIDAR_METERS")
        );
        assert_eq!(
            product.spatial_ref.resolution,
            Some(RasterResolution { x: 1.0, y: 1.0 })
        );
        assert_eq!(
            product.spatial_ref.bbox,
            Some(GeoBounds {
                min_lon: 0.0,
                min_lat: 0.0,
                max_lon: 2.0,
                max_lat: 3.0,
            })
        );
        let asserted =
            assert_raster_spatial_ref(Some(&product.spatial_ref), product.width, product.height)
                .unwrap();
        assert_eq!(asserted, product.spatial_ref);

        let first_cell = product.cells.get(&(1, 0)).unwrap();
        assert_eq!(first_cell.total_observations, 1);
        assert_eq!(first_cell.obstacle_count, 1);
    }

    #[test]
    fn build_occupancy_grid_records_thresholds_and_threshold_controls_occupancy() {
        let scan = LidarScan {
            timestamp: Utc::now(),
            points: vec![point(0.0, 1000.0), point(0.0, 1800.0)],
            scan_id: Uuid::new_v4(),
        };
        let low_threshold_mapper = test_mapper_with_occupancy_controls(10.0, 1.5, 0, 0.25, false);
        let high_threshold_mapper = test_mapper_with_occupancy_controls(10.0, 1.5, 0, 0.75, false);

        let low_threshold_grid = low_threshold_mapper
            .build_occupancy_grid(std::slice::from_ref(&scan))
            .unwrap();
        let high_threshold_grid = high_threshold_mapper.build_occupancy_grid(&[scan]).unwrap();

        assert!(low_threshold_grid.cells.get(&(0, 0)).unwrap().occupied);
        assert!(!high_threshold_grid.cells.get(&(0, 0)).unwrap().occupied);
        assert_eq!(low_threshold_grid.evidence.distance_threshold_m, 1.5);
        assert_eq!(low_threshold_grid.evidence.quality_threshold, 0);
        assert_eq!(low_threshold_grid.evidence.occupancy_threshold, 0.25);
        assert!(!low_threshold_grid.evidence.flip_y);
    }

    #[tokio::test]
    async fn save_occupancy_grid_evidence_persists_thresholds_and_flip() {
        let mapper = test_mapper_with_occupancy_controls(1.0, 2.5, 42, 0.75, true);
        let output_dir = temp_dir("occupancy_evidence");
        let evidence = mapper.occupancy_grid_evidence().unwrap();

        mapper
            .save_occupancy_grid_evidence(&evidence, &output_dir)
            .await
            .unwrap();

        let persisted_path = output_dir.join("lidar_occupancy_grid_evidence.json");
        assert!(persisted_path.exists());
        let persisted: LidarOccupancyGridEvidence =
            serde_json::from_str(&fs::read_to_string(persisted_path).unwrap()).unwrap();
        assert_eq!(persisted, evidence);
    }

    #[test]
    fn coverage_density_records_full_scan_grid() {
        let mapper = test_mapper();
        let grid = heatmap_test_grid(
            &mapper,
            vec![
                (
                    (0, 0),
                    GridCell {
                        occupied: false,
                        obstacle_count: 0,
                        total_observations: 2,
                    },
                ),
                (
                    (1, 0),
                    GridCell {
                        occupied: false,
                        obstacle_count: 0,
                        total_observations: 3,
                    },
                ),
                (
                    (0, 1),
                    GridCell {
                        occupied: false,
                        obstacle_count: 0,
                        total_observations: 1,
                    },
                ),
                (
                    (1, 1),
                    GridCell {
                        occupied: true,
                        obstacle_count: 1,
                        total_observations: 4,
                    },
                ),
            ],
            2,
            2,
        );

        let evidence = mapper.coverage_density_evidence(&grid, 0.75).unwrap();

        assert_eq!(evidence.status, LidarCoverageStatus::Pass);
        assert_eq!(evidence.width, 2);
        assert_eq!(evidence.height, 2);
        assert_eq!(evidence.total_cells, 4);
        assert_eq!(evidence.covered_cells, 4);
        assert_eq!(evidence.total_points, 10);
        assert_eq!(evidence.max_points_per_cell, 4);
        assert_eq!(evidence.coverage_floor, 0.75);
        assert!((evidence.covered_cell_fraction - 1.0).abs() < f32::EPSILON);
        assert!((evidence.mean_points_per_cell - 2.5).abs() < f32::EPSILON);
        assert_eq!(evidence.cell_densities.len(), 4);
        assert_eq!(evidence.cell_densities[0].points, 2);
        assert_eq!(evidence.cell_densities[3].points, 4);
    }

    #[tokio::test]
    async fn coverage_density_flags_and_persists_sparse_grid() {
        let mapper = test_mapper();
        let output_dir = temp_dir("coverage_density_evidence");
        let grid = heatmap_test_grid(
            &mapper,
            vec![(
                (0, 0),
                GridCell {
                    occupied: false,
                    obstacle_count: 0,
                    total_observations: 1,
                },
            )],
            3,
            3,
        );

        let evidence = mapper.coverage_density_evidence(&grid, 0.5).unwrap();

        assert_eq!(evidence.status, LidarCoverageStatus::LowCoverage);
        assert_eq!(evidence.total_cells, 9);
        assert_eq!(evidence.covered_cells, 1);
        assert_eq!(evidence.total_points, 1);
        assert!((evidence.covered_cell_fraction - (1.0 / 9.0)).abs() < 1e-6);
        assert!(evidence.cell_densities.iter().any(|cell| cell.points == 0));

        mapper
            .save_coverage_density_evidence(&evidence, &output_dir)
            .await
            .unwrap();
        let persisted_path = output_dir.join("lidar_coverage_density_evidence.json");
        let persisted: LidarCoverageDensityEvidence =
            serde_json::from_str(&fs::read_to_string(persisted_path).unwrap()).unwrap();
        assert_eq!(persisted, evidence);
    }

    #[tokio::test]
    async fn save_point_cloud_writes_valid_pcd_and_provenance() {
        let mapper = test_mapper();
        let output_dir = temp_dir("point_cloud_export");
        let captured_at = Utc::now();
        let scan_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        let second_captured_at = captured_at + chrono::Duration::seconds(10);
        let scans = vec![
            LidarScan {
                timestamp: captured_at,
                points: vec![point(0.0, 1000.0), point(0.0, 2000.0)],
                scan_id,
            },
            LidarScan {
                timestamp: second_captured_at,
                points: vec![point(0.0, 3000.0)],
                scan_id: second_id,
            },
        ];

        mapper.save_point_cloud(&scans, &output_dir).await.unwrap();

        let content = fs::read_to_string(output_dir.join("point_cloud.pcd")).unwrap();
        assert!(content.starts_with("# .PCD v0.7 - Point Cloud Data file format\n"));
        assert!(content.contains("VERSION 0.7\n"));
        assert!(content.contains("FIELDS x y z\n"));
        assert!(content.contains("SIZE 4 4 4\n"));
        assert!(content.contains("TYPE F F F\n"));
        assert!(content.contains("COUNT 1 1 1\n"));
        assert!(content.contains("WIDTH 3\n"));
        assert!(content.contains("HEIGHT 1\n"));
        assert!(content.contains("POINTS 3\n"));
        assert_eq!(
            pcd_data_lines(&content),
            vec![
                "1.000 0.000 0.000",
                "2.000 0.000 0.000",
                "3.000 0.000 0.000"
            ]
        );

        let provenance: LidarPointCloudProvenance = serde_json::from_str(
            &fs::read_to_string(output_dir.join("point_cloud_provenance.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(provenance.scan_ids, vec![scan_id, second_id]);
        assert_eq!(
            provenance.captured_at,
            vec![captured_at, second_captured_at]
        );
        assert_eq!(provenance.point_count, 3);
        assert!(provenance.frame_crs_note.contains(OCCUPANCY_GRID_LOCAL_CRS));
        assert!(provenance.frame_crs_note.contains("z=0"));
    }

    #[tokio::test]
    async fn save_point_cloud_writes_valid_empty_pcd_and_provenance() {
        let mapper = test_mapper();
        let output_dir = temp_dir("point_cloud_empty_export");

        mapper.save_point_cloud(&[], &output_dir).await.unwrap();

        let content = fs::read_to_string(output_dir.join("point_cloud.pcd")).unwrap();
        assert!(content.contains("WIDTH 0\n"));
        assert!(content.contains("POINTS 0\n"));
        assert_eq!(pcd_data_lines(&content), Vec::<&str>::new());

        let provenance: LidarPointCloudProvenance = serde_json::from_str(
            &fs::read_to_string(output_dir.join("point_cloud_provenance.json")).unwrap(),
        )
        .unwrap();
        assert!(provenance.scan_ids.is_empty());
        assert!(provenance.captured_at.is_empty());
        assert_eq!(provenance.point_count, 0);
        assert!(provenance.frame_crs_note.contains(OCCUPANCY_GRID_LOCAL_CRS));
    }

    #[tokio::test]
    async fn save_obstacle_heatmap_maps_density_and_records_thresholds() {
        let mapper = test_mapper_with_occupancy_controls(1.0, 2.5, 42, 0.75, false);
        let output_dir = temp_dir("heatmap_export");
        let grid = heatmap_test_grid(
            &mapper,
            vec![
                (
                    (0, 0),
                    GridCell {
                        occupied: false,
                        obstacle_count: 0,
                        total_observations: 4,
                    },
                ),
                (
                    (1, 0),
                    GridCell {
                        occupied: true,
                        obstacle_count: 4,
                        total_observations: 4,
                    },
                ),
            ],
            2,
            1,
        );

        mapper
            .save_obstacle_heatmap(&grid, &output_dir)
            .await
            .unwrap();

        let image = image::open(output_dir.join("obstacle_heatmap.png"))
            .unwrap()
            .to_rgb8();
        assert_eq!(image.dimensions(), (2, 1));
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 255]);
        assert_eq!(image.get_pixel(1, 0).0, [255, 0, 0]);

        let evidence: LidarObstacleHeatmapEvidence = serde_json::from_str(
            &fs::read_to_string(output_dir.join("obstacle_heatmap_evidence.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(evidence.occupancy, grid.evidence);
        assert_eq!(evidence.max_obstacle_count, 4);
        assert_eq!(evidence.width, 2);
        assert_eq!(evidence.height, 1);
        assert_eq!(evidence.spatial_ref.bbox, grid.spatial_ref.bbox);
    }

    #[tokio::test]
    async fn save_obstacle_heatmap_empty_grid_is_uniform_low() {
        let mapper = test_mapper_with_occupancy_controls(1.0, 5.0, 0, 0.5, false);
        let output_dir = temp_dir("heatmap_empty_export");
        let grid = heatmap_test_grid(&mapper, vec![], 1, 1);

        mapper
            .save_obstacle_heatmap(&grid, &output_dir)
            .await
            .unwrap();

        let image = image::open(output_dir.join("obstacle_heatmap.png"))
            .unwrap()
            .to_rgb8();
        assert_eq!(image.dimensions(), (1, 1));
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 255]);

        let evidence: LidarObstacleHeatmapEvidence = serde_json::from_str(
            &fs::read_to_string(output_dir.join("obstacle_heatmap_evidence.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(evidence.max_obstacle_count, 0);
        assert_eq!(evidence.occupancy, grid.evidence);
    }

    #[test]
    fn build_occupancy_grid_flips_y_coordinates_and_extent_consistently() {
        let scan = LidarScan {
            timestamp: Utc::now(),
            points: vec![point(90.0, 1000.0)],
            scan_id: Uuid::new_v4(),
        };
        let unflipped = test_mapper_with_occupancy_controls(1.0, 5.0, 0, 0.5, false)
            .build_occupancy_grid(std::slice::from_ref(&scan))
            .unwrap();
        let flipped = test_mapper_with_occupancy_controls(1.0, 5.0, 0, 0.5, true)
            .build_occupancy_grid(&[scan])
            .unwrap();

        assert!(unflipped.cells.contains_key(&(0, 1)));
        assert_eq!(unflipped.min_grid_y, 1);
        assert_eq!(
            unflipped.spatial_ref.bbox,
            Some(GeoBounds {
                min_lon: 0.0,
                min_lat: 1.0,
                max_lon: 1.0,
                max_lat: 2.0,
            })
        );

        assert!(flipped.cells.contains_key(&(0, -1)));
        assert_eq!(flipped.min_grid_y, -1);
        assert!(flipped.evidence.flip_y);
        assert_eq!(
            flipped.spatial_ref.bbox,
            Some(GeoBounds {
                min_lon: 0.0,
                min_lat: -1.0,
                max_lon: 1.0,
                max_lat: 0.0,
            })
        );
    }

    #[test]
    fn build_occupancy_grid_rejects_non_positive_resolution() {
        for resolution in [0.0, -1.0] {
            let mapper = test_mapper_with_resolution(resolution);
            let err = mapper.build_occupancy_grid(&[]).unwrap_err();
            assert!(err.to_string().contains("positive resolution"));
        }
    }

    #[test]
    fn build_occupancy_grid_rejects_out_of_range_occupancy_threshold() {
        for threshold in [-0.1, 1.1, f32::NAN] {
            let mapper = test_mapper_with_occupancy_controls(1.0, 5.0, 0, threshold, false);
            let err = mapper.build_occupancy_grid(&[]).unwrap_err();
            assert!(err.to_string().contains("occupancy threshold"));
        }
    }

    #[test]
    fn remove_statistical_outliers_records_removed_points() {
        let mapper = test_mapper();
        let params = LidarOutlierRemovalParams {
            k_neighbors: 2,
            stddev_multiplier: 1.0,
        };
        let scan = LidarScan {
            timestamp: Utc::now(),
            points: vec![
                point(0.0, 1000.0),
                point(5.0, 1000.0),
                point(355.0, 1000.0),
                point(10.0, 1100.0),
                point(0.0, 50_000.0),
            ],
            scan_id: Uuid::new_v4(),
        };

        let cleaned = mapper.remove_statistical_outliers(&[scan], params).unwrap();

        assert_eq!(cleaned.evidence.points_in, 5);
        assert_eq!(cleaned.evidence.points_removed, 1);
        assert_eq!(cleaned.evidence.points_out, 4);
        assert_eq!(cleaned.evidence.params, params);
        assert!(cleaned.evidence.mean_distance_threshold.is_some());
        assert_eq!(cleaned.scans[0].points.len(), 4);
        assert!(cleaned.scans[0]
            .points
            .iter()
            .all(|point| point.distance < 50_000.0));
    }

    #[test]
    fn remove_statistical_outliers_keeps_clean_cloud() {
        let mapper = test_mapper();
        let params = LidarOutlierRemovalParams {
            k_neighbors: 2,
            stddev_multiplier: 1.0,
        };
        let scan = LidarScan {
            timestamp: Utc::now(),
            points: vec![
                point(0.0, 1000.0),
                point(90.0, 1000.0),
                point(180.0, 1000.0),
                point(270.0, 1000.0),
            ],
            scan_id: Uuid::new_v4(),
        };

        let cleaned = mapper.remove_statistical_outliers(&[scan], params).unwrap();

        assert_eq!(cleaned.evidence.points_in, 4);
        assert_eq!(cleaned.evidence.points_removed, 0);
        assert_eq!(cleaned.evidence.points_out, 4);
        assert_eq!(cleaned.scans[0].points.len(), 4);
    }

    #[test]
    fn remove_statistical_outliers_handles_degenerate_cloud_without_crash() {
        let mapper = test_mapper();
        let params = LidarOutlierRemovalParams {
            k_neighbors: 8,
            stddev_multiplier: 1.0,
        };
        let scan = LidarScan {
            timestamp: Utc::now(),
            points: vec![point(0.0, 1000.0)],
            scan_id: Uuid::new_v4(),
        };

        let cleaned = mapper.remove_statistical_outliers(&[scan], params).unwrap();

        assert_eq!(cleaned.evidence.points_in, 1);
        assert_eq!(cleaned.evidence.points_removed, 0);
        assert_eq!(cleaned.evidence.points_out, 1);
        assert_eq!(cleaned.evidence.mean_distance_threshold, None);
        assert_eq!(cleaned.scans[0].points.len(), 1);
    }

    #[test]
    fn estimate_surface_normals_for_planar_patch() {
        let mapper = test_mapper();
        let params = LidarNormalEstimationParams { k_neighbors: 4 };
        let points = vec![
            LidarPoint3::new(-1.0, -1.0, 2.0),
            LidarPoint3::new(0.0, -1.0, 2.0),
            LidarPoint3::new(1.0, -1.0, 2.0),
            LidarPoint3::new(-1.0, 0.0, 2.0),
            LidarPoint3::new(0.0, 0.0, 2.0),
            LidarPoint3::new(1.0, 0.0, 2.0),
            LidarPoint3::new(-1.0, 1.0, 2.0),
            LidarPoint3::new(0.0, 1.0, 2.0),
            LidarPoint3::new(1.0, 1.0, 2.0),
        ];

        let result = mapper.estimate_surface_normals(&points, params).unwrap();

        assert_eq!(result.evidence.points_in, points.len());
        assert_eq!(result.evidence.neighborhood_size, 4);
        assert_eq!(result.evidence.normals_defined, points.len());
        assert_eq!(result.evidence.normals_undefined, 0);
        for estimate in &result.estimates {
            let normal = estimate.normal.unwrap();
            assert!(normal.x.abs() <= 1.0e-6);
            assert!(normal.y.abs() <= 1.0e-6);
            assert!((normal.z - 1.0).abs() <= 1.0e-6);
        }
    }

    #[test]
    fn estimate_surface_normals_marks_insufficient_neighbors_undefined() {
        let mapper = test_mapper();
        let params = LidarNormalEstimationParams { k_neighbors: 3 };
        let points = vec![
            LidarPoint3::new(0.0, 0.0, 0.0),
            LidarPoint3::new(1.0, 0.0, 0.0),
        ];

        let result = mapper.estimate_surface_normals(&points, params).unwrap();

        assert_eq!(result.evidence.points_in, 2);
        assert_eq!(result.evidence.neighborhood_size, 3);
        assert_eq!(result.evidence.normals_defined, 0);
        assert_eq!(result.evidence.normals_undefined, 2);
        assert!(result
            .estimates
            .iter()
            .all(|estimate| estimate.normal.is_none()));
    }

    #[test]
    fn segment_ground_points_classifies_sloped_terrain_and_canopy() {
        let mapper = test_mapper();
        let params = LidarGroundSegmentationParams {
            max_ground_tilt_degrees: 30.0,
            max_ground_height_m: 0.35,
        };
        let points = vec![
            LidarPoint3::new(-1.0, -1.0, 0.0),
            LidarPoint3::new(0.0, -1.0, 0.05),
            LidarPoint3::new(1.0, -1.0, 0.1),
            LidarPoint3::new(0.0, 0.0, 0.15),
            LidarPoint3::new(0.0, 0.0, 2.0),
            LidarPoint3::new(1.0, 1.0, 2.3),
        ];
        let normals = vec![
            normal_estimate(0, 0.98),
            normal_estimate(1, 0.98),
            normal_estimate(2, 0.98),
            normal_estimate(3, 0.98),
            normal_estimate(4, 0.98),
            normal_estimate(5, 0.4),
        ];

        let result = mapper
            .segment_ground_points(&points, &normals, params)
            .unwrap();

        assert_eq!(result.evidence.points_in, 6);
        assert_eq!(result.evidence.ground_count, 4);
        assert_eq!(result.evidence.non_ground_count, 2);
        assert_eq!(result.evidence.params, params);
        let classes: Vec<_> = result
            .classifications
            .iter()
            .map(|classification| classification.class)
            .collect();
        assert_eq!(
            classes,
            vec![
                LidarPointClass::Ground,
                LidarPointClass::Ground,
                LidarPointClass::Ground,
                LidarPointClass::Ground,
                LidarPointClass::NonGround,
                LidarPointClass::NonGround,
            ]
        );
    }

    #[test]
    fn segment_ground_points_reports_no_ground_surface() {
        let mapper = test_mapper();
        let params = LidarGroundSegmentationParams {
            max_ground_tilt_degrees: 30.0,
            max_ground_height_m: 0.35,
        };
        let points = vec![
            LidarPoint3::new(0.0, 0.0, 2.0),
            LidarPoint3::new(1.0, 0.0, 2.2),
            LidarPoint3::new(0.0, 1.0, 2.4),
        ];
        let normals = vec![
            normal_estimate(0, 0.98),
            normal_estimate(1, 0.98),
            normal_estimate(2, 0.98),
        ];

        let err = mapper
            .segment_ground_points(&points, &normals, params)
            .unwrap_err();

        assert!(err.to_string().contains("no ground surface"));
    }

    #[test]
    fn cluster_non_ground_points_finds_two_objects_with_bbox_centroids() {
        let mapper = test_mapper();
        let points = vec![
            LidarPoint3::new(0.0, 0.0, 1.0),
            LidarPoint3::new(0.2, 0.0, 1.2),
            LidarPoint3::new(0.0, 0.2, 1.1),
            LidarPoint3::new(5.0, 5.0, 2.0),
            LidarPoint3::new(5.2, 5.0, 2.1),
            LidarPoint3::new(5.0, 5.2, 2.2),
            LidarPoint3::new(10.0, 10.0, 0.0),
        ];
        let classifications = vec![
            classification(0, LidarPointClass::NonGround),
            classification(1, LidarPointClass::NonGround),
            classification(2, LidarPointClass::NonGround),
            classification(3, LidarPointClass::NonGround),
            classification(4, LidarPointClass::NonGround),
            classification(5, LidarPointClass::NonGround),
            classification(6, LidarPointClass::Ground),
        ];
        let params = LidarObjectClusteringParams {
            cluster_distance_m: 0.5,
            min_cluster_size: 2,
        };

        let result = mapper
            .cluster_non_ground_objects(&points, &classifications, params)
            .unwrap();

        assert_eq!(result.clusters.len(), 2);
        assert_eq!(result.evidence.points_in, 7);
        assert_eq!(result.evidence.non_ground_points, 6);
        assert_eq!(result.evidence.noise_points, 0);
        assert_eq!(result.evidence.params, params);

        let first = &result.clusters[0];
        assert_eq!(first.id, 0);
        assert_eq!(first.point_count, 3);
        assert_eq!(first.point_indices, vec![0, 1, 2]);
        assert!((first.centroid.x - (0.2 / 3.0)).abs() <= 1.0e-9);
        assert!((first.centroid.y - (0.2 / 3.0)).abs() <= 1.0e-9);
        assert!((first.centroid.z - 1.1).abs() <= 1.0e-9);
        assert_eq!(first.bbox.min_x, 0.0);
        assert_eq!(first.bbox.max_x, 0.2);
        assert_eq!(first.bbox.min_z, 1.0);
        assert_eq!(first.bbox.max_z, 1.2);

        let second = &result.clusters[1];
        assert_eq!(second.id, 1);
        assert_eq!(second.point_count, 3);
        assert_eq!(second.point_indices, vec![3, 4, 5]);
        assert!((second.centroid.x - (15.2 / 3.0)).abs() <= 1.0e-9);
        assert!((second.centroid.y - (15.2 / 3.0)).abs() <= 1.0e-9);
    }

    #[test]
    fn cluster_non_ground_points_drops_subthreshold_noise() {
        let mapper = test_mapper();
        let points = vec![
            LidarPoint3::new(0.0, 0.0, 1.0),
            LidarPoint3::new(0.2, 0.0, 1.0),
            LidarPoint3::new(5.0, 5.0, 2.0),
        ];
        let classifications = vec![
            classification(0, LidarPointClass::NonGround),
            classification(1, LidarPointClass::NonGround),
            classification(2, LidarPointClass::NonGround),
        ];
        let params = LidarObjectClusteringParams {
            cluster_distance_m: 0.5,
            min_cluster_size: 2,
        };

        let result = mapper
            .cluster_non_ground_objects(&points, &classifications, params)
            .unwrap();

        assert_eq!(result.clusters.len(), 1);
        assert_eq!(result.clusters[0].point_indices, vec![0, 1]);
        assert_eq!(result.evidence.noise_points, 1);
        assert_eq!(result.evidence.clusters_emitted, 1);
    }

    #[test]
    fn build_elevation_products_rasterizes_dsm_dtm_with_asserted_spatial_ref() {
        let mapper = test_mapper();
        let params = LidarElevationRasterParams {
            resolution_m: 1.0,
            nodata: -9999.0,
        };
        let points = vec![
            LidarPoint3::new(0.1, 0.1, 1.0),
            LidarPoint3::new(0.1, 0.1, 2.0),
            LidarPoint3::new(1.2, 0.1, 1.5),
            LidarPoint3::new(0.1, 1.2, 3.0),
        ];
        let classifications = vec![
            classification(0, LidarPointClass::Ground),
            classification(1, LidarPointClass::NonGround),
            classification(2, LidarPointClass::Ground),
            classification(3, LidarPointClass::NonGround),
        ];

        let products = mapper
            .build_elevation_products(&points, &classifications, params)
            .unwrap();

        assert_eq!(products.dsm.width, 2);
        assert_eq!(products.dsm.height, 2);
        assert_eq!(products.dsm.value_at(0, 0), Some(2.0));
        assert_eq!(products.dsm.value_at(1, 0), Some(1.5));
        assert_eq!(products.dsm.value_at(0, 1), Some(3.0));
        assert_eq!(products.dsm.value_at(1, 1), Some(params.nodata));
        assert_eq!(products.dtm.value_at(0, 0), Some(1.0));
        assert_eq!(products.dtm.value_at(1, 0), Some(1.5));

        let asserted = assert_raster_spatial_ref(Some(&products.dsm.spatial_ref), 2, 2).unwrap();
        assert_eq!(asserted, products.dsm.spatial_ref);
        assert_eq!(products.dtm.spatial_ref, products.dsm.spatial_ref);
        assert_eq!(products.evidence.ground_points, 2);
        assert_eq!(products.evidence.dsm_valid_cells, 3);
        assert_eq!(products.evidence.dtm_valid_cells, 2);

        let roundtrip: LidarElevationProducts =
            serde_json::from_str(&serde_json::to_string(&products).unwrap()).unwrap();
        assert_eq!(roundtrip, products);
    }

    #[test]
    fn build_elevation_products_keeps_dtm_nodata_without_ground_returns() {
        let mapper = test_mapper();
        let params = LidarElevationRasterParams {
            resolution_m: 1.0,
            nodata: -9999.0,
        };
        let points = vec![
            LidarPoint3::new(0.1, 0.1, 0.2),
            LidarPoint3::new(1.1, 0.1, 2.5),
        ];
        let classifications = vec![
            classification(0, LidarPointClass::Ground),
            classification(1, LidarPointClass::NonGround),
        ];

        let products = mapper
            .build_elevation_products(&points, &classifications, params)
            .unwrap();

        assert_eq!(products.dsm.value_at(1, 0), Some(2.5));
        assert_eq!(products.dtm.value_at(1, 0), Some(params.nodata));
        assert_ne!(products.dtm.value_at(1, 0), Some(0.0));
    }
}
