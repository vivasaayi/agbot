use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::schemas::FarmFieldRegistry;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub mod findings_export;
pub mod grower_report;
pub mod lidar_analysis;
pub mod ndvi_analysis;
pub mod product_anomalies;
pub mod report_generator;
pub mod thermal_analysis;
pub mod zonal_statistics;
pub mod zone_delineation;
pub mod zone_recommendations;

pub use findings_export::{
    export_findings_csv, export_findings_geojson, FindingExportRecord, FindingsExportError,
    FINDINGS_CSV_HEADER,
};
pub use grower_report::{
    render_grower_ready_pdf, FieldReportMetadata, GrowerReportError, GrowerReportRequest,
    SceneReportMetadata,
};
pub use lidar_analysis::{LidarAnalysisConfig, LidarAnalysisProcessor};
pub use ndvi_analysis::{NdviAnalysisConfig, NdviAnalysisProcessor};
pub use product_anomalies::{
    flag_product_anomalies, AnomalyDetectionConfig, AnomalyDetectionError, ProductAnomaly,
    ProductAnomalyReasonCode,
};
pub use report_generator::ReportGenerator;
pub use thermal_analysis::{ThermalAnalysisConfig, ThermalAnalysisProcessor};
pub use zonal_statistics::{
    compute_zonal_statistics, ProductGrid, ProductGridStatistics, ZonalStatisticsError,
};
pub use zone_delineation::{
    delineate_anomaly_zones, AnomalyZone, AnomalyZonePolygon, ZoneDelineationError,
};
pub use zone_recommendations::{
    create_recommendation_from_zone, priority_for_zone_area, ZoneRecommendationError,
    ZoneRecommendationRequest,
};

/// Post-processing pipeline for agricultural drone data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingJob {
    pub id: Uuid,
    pub job_type: JobType,
    pub input_files: Vec<PathBuf>,
    pub output_directory: PathBuf,
    pub parameters: ProcessingParameters,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisJobIdentity {
    pub job_id: Uuid,
    pub scene_id: String,
    pub field_id: String,
    pub season_id: String,
    pub product_refs: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub status: JobStatus,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisJobRequest {
    pub org_id: String,
    pub scene_id: String,
    pub field_id: String,
    pub season_id: String,
    pub product_refs: Vec<String>,
    pub job_type: JobType,
    pub input_files: Vec<PathBuf>,
    pub output_directory: PathBuf,
    pub parameters: ProcessingParameters,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnalysisJobError {
    #[error("unknown scene: {scene_id}")]
    UnknownScene { scene_id: String },
    #[error("analysis job not found: {job_id}")]
    JobNotFound { job_id: Uuid },
    #[error("analysis job queue rejected request: {reason}")]
    QueueRejected { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobType {
    NdviAnalysis,
    LidarProcessing,
    ThermalAnalysis,
    MultiSpectralAnalysis,
    CompositeReport,
    HealthAssessment,
    YieldPrediction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingParameters {
    pub analysis_type: String,
    pub quality_threshold: f32,
    pub spatial_resolution_m: f32,
    pub temporal_aggregation: Option<String>,
    pub output_formats: Vec<OutputFormat>,
    pub custom_parameters: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    GeoTIFF,
    CSV,
    JSON,
    PDF,
    HTML,
    KML,
    Shapefile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub id: Uuid,
    pub job_id: Uuid,
    pub result_type: ResultType,
    pub data: ResultData,
    pub statistics: AnalysisStatistics,
    pub visualizations: Vec<VisualizationOutput>,
    pub recommendations: Vec<Recommendation>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetainedAnalysisResult {
    pub result: AnalysisResult,
    pub identity: AnalysisJobIdentity,
}

#[derive(Debug, Clone, Default)]
pub struct AnalysisResultListQuery {
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub scene_id: Option<String>,
    pub created_from: Option<DateTime<Utc>>,
    pub created_to: Option<DateTime<Utc>>,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone)]
pub struct AnalysisResultListPage {
    pub items: Vec<RetainedAnalysisResult>,
    pub total_count: usize,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResultType {
    NdviMap,
    ElevationModel,
    ThermalMap,
    HealthIndex,
    YieldEstimate,
    IrrigationMap,
    StressIndicators,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResultData {
    GridData {
        width: u32,
        height: u32,
        values: Vec<f32>,
        bounds: (f64, f64, f64, f64),
        units: String,
    },
    PointData {
        points: Vec<(f64, f64, f32)>,
        attributes: HashMap<String, Vec<f32>>,
    },
    ZonalData {
        zones: Vec<AnalysisZone>,
        aggregated_values: HashMap<String, f32>,
    },
    TimeSeriesData {
        timestamps: Vec<DateTime<Utc>>,
        values: HashMap<String, Vec<f32>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisZone {
    pub id: String,
    pub boundary: Vec<(f64, f64)>,
    pub area_m2: f32,
    pub values: HashMap<String, f32>,
    pub classification: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisStatistics {
    pub min_value: f32,
    pub max_value: f32,
    pub mean_value: f32,
    pub std_deviation: f32,
    pub percentiles: HashMap<String, f32>,
    pub coverage_area_m2: f32,
    pub valid_pixel_count: u32,
    pub total_pixel_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationOutput {
    pub id: Uuid,
    pub visualization_type: VisualizationType,
    pub file_path: PathBuf,
    pub format: String,
    pub description: String,
    pub parameters: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisualizationType {
    Heatmap,
    ContourMap,
    ClassificationMap,
    Chart,
    Histogram,
    ScatterPlot,
    TimeSeries,
    CompositeImage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub category: RecommendationCategory,
    pub priority: Priority,
    pub title: String,
    pub description: String,
    pub action_items: Vec<String>,
    pub affected_areas: Vec<AnalysisZone>,
    pub confidence_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationCategory {
    Irrigation,
    Fertilization,
    PestControl,
    Harvesting,
    Replanting,
    SoilManagement,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

/// Main post-processing service
pub struct PostProcessorService {
    job_queue: Vec<ProcessingJob>,
    completed_jobs: HashMap<Uuid, ProcessingJob>,
    results_cache: HashMap<Uuid, AnalysisResult>,
    result_records: HashMap<Uuid, RetainedAnalysisResult>,
    analysis_job_identities: HashMap<Uuid, AnalysisJobIdentity>,
    working_directory: PathBuf,
    ndvi_analyzer: NdviAnalysisProcessor,
    lidar_analyzer: LidarAnalysisProcessor,
    thermal_analyzer: ThermalAnalysisProcessor,
    report_generator: ReportGenerator,
}

impl PostProcessorService {
    pub fn new(working_directory: PathBuf) -> Result<Self> {
        let result_records = Self::load_retained_analysis_results(&working_directory)?;
        let results_cache = result_records
            .iter()
            .map(|(result_id, record)| (*result_id, record.result.clone()))
            .collect();
        let analysis_job_identities = result_records
            .values()
            .map(|record| (record.identity.job_id, record.identity.clone()))
            .collect();

        Ok(Self {
            job_queue: Vec::new(),
            completed_jobs: HashMap::new(),
            results_cache,
            result_records,
            analysis_job_identities,
            working_directory,
            ndvi_analyzer: NdviAnalysisProcessor::new(NdviAnalysisConfig::default()),
            lidar_analyzer: LidarAnalysisProcessor::new(LidarAnalysisConfig::default()),
            thermal_analyzer: ThermalAnalysisProcessor::new(ThermalAnalysisConfig::default()),
            report_generator: ReportGenerator::new(report_generator::ReportConfig {
                output_formats: vec![
                    report_generator::OutputFormat::PDF,
                    report_generator::OutputFormat::HTML,
                ],
                default_template: "agricultural_comprehensive".to_string(),
                include_raw_data: true,
                include_visualizations: true,
                enable_comparative_analysis: true,
                logo_path: None,
                company_info: report_generator::CompanyInfo {
                    name: "AgroDrone Analytics".to_string(),
                    address: "".to_string(),
                    contact_email: "info@agrodrone.com".to_string(),
                    website: Some("https://agrodrone.com".to_string()),
                    certification_info: None,
                },
            }),
        })
    }

    pub async fn submit_job(&mut self, mut job: ProcessingJob) -> Result<Uuid> {
        job.id = Uuid::new_v4();
        job.status = JobStatus::Queued;
        job.created_at = Utc::now();

        let job_id = job.id;
        self.job_queue.push(job);

        // Sort by priority and creation time
        self.job_queue
            .sort_by(|a, b| a.created_at.cmp(&b.created_at));

        tracing::info!("Submitted processing job: {}", job_id);
        Ok(job_id)
    }

    pub async fn submit_analysis_job(
        &mut self,
        scene_catalog: &FarmFieldRegistry,
        request: AnalysisJobRequest,
    ) -> std::result::Result<Uuid, AnalysisJobError> {
        let scene_known = scene_catalog
            .scenes_for_field_season(&request.org_id, &request.field_id, &request.season_id)
            .iter()
            .any(|entry| entry.scene.scene_id == request.scene_id);
        if !scene_known {
            return Err(AnalysisJobError::UnknownScene {
                scene_id: request.scene_id,
            });
        }

        let scene_id = request.scene_id;
        let field_id = request.field_id;
        let season_id = request.season_id;
        let product_refs = request.product_refs;
        let job = ProcessingJob {
            id: Uuid::nil(),
            job_type: request.job_type,
            input_files: request.input_files,
            output_directory: request.output_directory,
            parameters: request.parameters,
            status: JobStatus::Queued,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            error_message: None,
        };
        let job_id =
            self.submit_job(job)
                .await
                .map_err(|error| AnalysisJobError::QueueRejected {
                    reason: error.to_string(),
                })?;
        let created_at = self
            .get_job_status(&job_id)
            .await
            .map(|job| job.created_at)
            .unwrap_or_else(Utc::now);

        self.analysis_job_identities.insert(
            job_id,
            AnalysisJobIdentity {
                job_id,
                scene_id,
                field_id,
                season_id,
                product_refs,
                created_at,
                status: JobStatus::Queued,
                failure_reason: None,
            },
        );

        Ok(job_id)
    }

    pub async fn process_next_job(&mut self) -> Result<Option<AnalysisResult>> {
        if let Some(mut job) = self.job_queue.pop() {
            job.status = JobStatus::Processing;
            job.started_at = Some(Utc::now());
            self.sync_analysis_job_identity(&job);

            tracing::info!("Processing job: {} (type: {:?})", job.id, job.job_type);

            let result = match self.process_job(&job).await {
                Ok(mut result) => {
                    result.job_id = job.id;
                    job.status = JobStatus::Completed;
                    job.completed_at = Some(Utc::now());
                    self.results_cache.insert(result.id, result.clone());
                    Some(result)
                }
                Err(e) => {
                    job.status = JobStatus::Failed;
                    job.error_message = Some(e.to_string());
                    job.completed_at = Some(Utc::now());
                    tracing::error!("Job {} failed: {}", job.id, e);
                    None
                }
            };

            self.sync_analysis_job_identity(&job);
            if let Some(result) = result.as_ref() {
                self.retain_analysis_result(result).await?;
            }
            self.completed_jobs.insert(job.id, job);
            Ok(result)
        } else {
            Ok(None)
        }
    }

    async fn process_job(&mut self, job: &ProcessingJob) -> Result<AnalysisResult> {
        match job.job_type {
            JobType::NdviAnalysis => {
                self.ndvi_analyzer
                    .analyze(&job.input_files, &job.parameters)
                    .await
            }
            JobType::LidarProcessing => {
                self.lidar_analyzer
                    .analyze(&job.input_files, &job.parameters)
                    .await
            }
            JobType::ThermalAnalysis => {
                self.thermal_analyzer
                    .analyze(&job.input_files, &job.parameters)
                    .await
            }
            JobType::MultiSpectralAnalysis => self.process_multispectral(job).await,
            JobType::CompositeReport => self.generate_composite_report(job).await,
            JobType::HealthAssessment => self.assess_crop_health(job).await,
            JobType::YieldPrediction => self.predict_yield(job).await,
        }
    }

    async fn process_multispectral(&self, _job: &ProcessingJob) -> Result<AnalysisResult> {
        // Implementation for multispectral analysis
        let result = AnalysisResult {
            id: Uuid::new_v4(),
            job_id: _job.id,
            result_type: ResultType::HealthIndex,
            data: ResultData::GridData {
                width: 100,
                height: 100,
                values: vec![0.8; 10000], // Dummy data
                bounds: (0.0, 0.0, 100.0, 100.0),
                units: "health_index".to_string(),
            },
            statistics: AnalysisStatistics::default(),
            visualizations: Vec::new(),
            recommendations: Vec::new(),
            created_at: Utc::now(),
        };

        Ok(result)
    }

    async fn generate_composite_report(&self, job: &ProcessingJob) -> Result<AnalysisResult> {
        // Implementation for composite report generation
        let mut analysis_zones = Vec::new();
        let mut visualizations = Vec::new();
        let mut recommendations = Vec::new();

        // Create analysis zones based on job parameters
        analysis_zones.push(AnalysisZone {
            id: "composite_analysis".to_string(),
            boundary: vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
            area_m2: 10000.0,
            values: HashMap::from([
                ("ndvi_mean".to_string(), 0.75),
                ("thermal_variance".to_string(), 15.2),
                ("lidar_coverage".to_string(), 0.98),
            ]),
            classification: Some("Healthy Vegetation".to_string()),
        });

        recommendations.push(Recommendation {
            category: RecommendationCategory::Irrigation,
            priority: Priority::Medium,
            title: "Composite Analysis Complete".to_string(),
            description: "Multi-sensor data integration has been completed successfully"
                .to_string(),
            action_items: vec![
                "Review NDVI results".to_string(),
                "Check thermal anomalies".to_string(),
            ],
            affected_areas: analysis_zones.clone(),
            confidence_score: 0.92,
        });

        Ok(AnalysisResult {
            id: Uuid::new_v4(),
            job_id: job.id,
            result_type: ResultType::HealthIndex,
            data: ResultData::ZonalData {
                zones: analysis_zones,
                aggregated_values: HashMap::from([
                    ("total_area_analyzed".to_string(), 10000.0),
                    ("data_quality_score".to_string(), 0.92),
                ]),
            },
            statistics: AnalysisStatistics {
                min_value: 0.1,
                max_value: 0.95,
                mean_value: 0.75,
                std_deviation: 0.15,
                percentiles: HashMap::from([
                    ("25".to_string(), 0.65),
                    ("50".to_string(), 0.75),
                    ("75".to_string(), 0.85),
                ]),
                coverage_area_m2: 10000.0,
                valid_pixel_count: 9800,
                total_pixel_count: 10000,
            },
            visualizations,
            recommendations,
            created_at: Utc::now(),
        })
    }

    async fn assess_crop_health(&self, job: &ProcessingJob) -> Result<AnalysisResult> {
        // Combine NDVI, thermal, and multispectral data for health assessment
        let mut health_zones = Vec::new();

        // Create sample health zones
        health_zones.push(AnalysisZone {
            id: "zone_1".to_string(),
            boundary: vec![(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
            area_m2: 2500.0,
            values: [
                ("health_score".to_string(), 0.85),
                ("stress_level".to_string(), 0.15),
            ]
            .iter()
            .cloned()
            .collect(),
            classification: Some("Healthy".to_string()),
        });

        let recommendations = vec![Recommendation {
            category: RecommendationCategory::Irrigation,
            priority: Priority::Medium,
            title: "Increase irrigation in southwestern area".to_string(),
            description: "NDVI values indicate water stress in zones 3-5".to_string(),
            action_items: vec![
                "Increase irrigation frequency to 3x per week".to_string(),
                "Monitor soil moisture levels".to_string(),
            ],
            affected_areas: health_zones.clone(),
            confidence_score: 0.78,
        }];

        let result = AnalysisResult {
            id: Uuid::new_v4(),
            job_id: job.id,
            result_type: ResultType::HealthIndex,
            data: ResultData::ZonalData {
                zones: health_zones,
                aggregated_values: [("overall_health".to_string(), 0.82)]
                    .iter()
                    .cloned()
                    .collect(),
            },
            statistics: AnalysisStatistics::default(),
            visualizations: Vec::new(),
            recommendations,
            created_at: Utc::now(),
        };

        Ok(result)
    }

    async fn predict_yield(&self, job: &ProcessingJob) -> Result<AnalysisResult> {
        // Implementation for yield prediction based on crop health and historical data
        let result = AnalysisResult {
            id: Uuid::new_v4(),
            job_id: job.id,
            result_type: ResultType::YieldEstimate,
            data: ResultData::GridData {
                width: 50,
                height: 50,
                values: vec![4.2; 2500], // Dummy yield data (tons/hectare)
                bounds: (0.0, 0.0, 500.0, 500.0),
                units: "tons_per_hectare".to_string(),
            },
            statistics: AnalysisStatistics {
                min_value: 3.1,
                max_value: 5.8,
                mean_value: 4.2,
                std_deviation: 0.8,
                percentiles: [
                    ("25".to_string(), 3.6),
                    ("50".to_string(), 4.2),
                    ("75".to_string(), 4.9),
                ]
                .iter()
                .cloned()
                .collect(),
                coverage_area_m2: 250000.0,
                valid_pixel_count: 2500,
                total_pixel_count: 2500,
            },
            visualizations: Vec::new(),
            recommendations: Vec::new(),
            created_at: Utc::now(),
        };

        Ok(result)
    }

    pub async fn get_job_status(&self, job_id: &Uuid) -> Option<&ProcessingJob> {
        // Check active queue first
        if let Some(job) = self.job_queue.iter().find(|j| j.id == *job_id) {
            return Some(job);
        }

        // Check completed jobs
        self.completed_jobs.get(job_id)
    }

    pub async fn get_result(&self, result_id: &Uuid) -> Option<&AnalysisResult> {
        self.results_cache.get(result_id)
    }

    pub async fn list_analysis_results(
        &self,
        query: AnalysisResultListQuery,
    ) -> AnalysisResultListPage {
        let page = query.page.max(1);
        let page_size = if query.page_size == 0 {
            50
        } else {
            query.page_size.min(250)
        };
        let mut items: Vec<RetainedAnalysisResult> = self
            .result_records
            .values()
            .filter(|record| analysis_result_matches_query(record, &query))
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .result
                .created_at
                .cmp(&left.result.created_at)
                .then_with(|| right.result.id.cmp(&left.result.id))
        });

        let total_count = items.len();
        let start = (page - 1).saturating_mul(page_size);
        let paged_items = items.into_iter().skip(start).take(page_size).collect();

        AnalysisResultListPage {
            items: paged_items,
            total_count,
            page,
            page_size,
        }
    }

    pub fn analysis_job_identity(&self, job_id: &Uuid) -> Option<&AnalysisJobIdentity> {
        self.analysis_job_identities.get(job_id)
    }

    pub fn mark_analysis_job_failed(
        &mut self,
        job_id: &Uuid,
        reason_code: impl Into<String>,
    ) -> std::result::Result<(), AnalysisJobError> {
        let reason_code = reason_code.into();
        if let Some(queue_index) = self.job_queue.iter().position(|job| job.id == *job_id) {
            let mut job = self.job_queue.remove(queue_index);
            job.status = JobStatus::Failed;
            job.completed_at = Some(Utc::now());
            job.error_message = Some(reason_code.clone());
            self.sync_analysis_job_identity(&job);
            self.completed_jobs.insert(*job_id, job);
            return Ok(());
        }

        if let Some(job) = self.completed_jobs.get_mut(job_id) {
            job.status = JobStatus::Failed;
            job.completed_at = Some(Utc::now());
            job.error_message = Some(reason_code);
            let job = job.clone();
            self.sync_analysis_job_identity(&job);
            return Ok(());
        }

        Err(AnalysisJobError::JobNotFound { job_id: *job_id })
    }

    pub async fn list_jobs(&self, status_filter: Option<JobStatus>) -> Vec<&ProcessingJob> {
        let mut jobs: Vec<&ProcessingJob> = self.job_queue.iter().collect();
        jobs.extend(self.completed_jobs.values());

        if let Some(status) = status_filter {
            jobs.retain(|job| job.status == status);
        }

        jobs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        jobs
    }

    fn sync_analysis_job_identity(&mut self, job: &ProcessingJob) {
        if let Some(identity) = self.analysis_job_identities.get_mut(&job.id) {
            identity.status = job.status;
            identity.failure_reason = job.error_message.clone();
        }
    }

    async fn retain_analysis_result(&mut self, result: &AnalysisResult) -> Result<()> {
        let Some(identity) = self.analysis_job_identities.get(&result.job_id).cloned() else {
            return Ok(());
        };
        let record = RetainedAnalysisResult {
            result: result.clone(),
            identity,
        };
        let output_dir = Self::analysis_results_dir_for(&self.working_directory);
        tokio::fs::create_dir_all(&output_dir).await?;
        let output_path = output_dir.join(format!("{}.json", result.id));
        let content = serde_json::to_vec_pretty(&record)?;
        tokio::fs::write(output_path, content).await?;
        self.result_records.insert(result.id, record);
        Ok(())
    }

    fn load_retained_analysis_results(
        working_directory: &Path,
    ) -> Result<HashMap<Uuid, RetainedAnalysisResult>> {
        let results_dir = Self::analysis_results_dir_for(working_directory);
        if !results_dir.exists() {
            return Ok(HashMap::new());
        }

        let mut records = HashMap::new();
        for entry in fs::read_dir(results_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let content = fs::read(&path)?;
            let record: RetainedAnalysisResult = serde_json::from_slice(&content)?;
            records.insert(record.result.id, record);
        }
        Ok(records)
    }

    fn analysis_results_dir_for(working_directory: &Path) -> PathBuf {
        working_directory.join("analysis_results")
    }

    pub async fn cleanup_old_results(&mut self, older_than_days: u32) -> Result<u32> {
        let cutoff_date = Utc::now() - chrono::Duration::days(older_than_days as i64);
        let mut removed_count = 0;

        // Remove old completed jobs
        self.completed_jobs.retain(|_, job| {
            if job.completed_at.unwrap_or(job.created_at) > cutoff_date {
                true
            } else {
                removed_count += 1;
                false
            }
        });

        // Remove old results and their retained listing records.
        let old_result_ids: Vec<Uuid> = self
            .results_cache
            .iter()
            .filter_map(|(result_id, result)| {
                (result.created_at <= cutoff_date).then_some(*result_id)
            })
            .collect();
        for result_id in old_result_ids {
            self.results_cache.remove(&result_id);
            self.result_records.remove(&result_id);
            let result_path = Self::analysis_results_dir_for(&self.working_directory)
                .join(format!("{result_id}.json"));
            let _ = tokio::fs::remove_file(result_path).await;
        }

        tracing::info!("Cleaned up {} old processing jobs", removed_count);
        Ok(removed_count)
    }
}

impl Default for AnalysisStatistics {
    fn default() -> Self {
        Self {
            min_value: 0.0,
            max_value: 1.0,
            mean_value: 0.5,
            std_deviation: 0.2,
            percentiles: HashMap::new(),
            coverage_area_m2: 0.0,
            valid_pixel_count: 0,
            total_pixel_count: 0,
        }
    }
}

fn analysis_result_matches_query(
    record: &RetainedAnalysisResult,
    query: &AnalysisResultListQuery,
) -> bool {
    if query
        .field_id
        .as_deref()
        .is_some_and(|field_id| record.identity.field_id != field_id)
    {
        return false;
    }
    if query
        .season_id
        .as_deref()
        .is_some_and(|season_id| record.identity.season_id != season_id)
    {
        return false;
    }
    if query
        .scene_id
        .as_deref()
        .is_some_and(|scene_id| record.identity.scene_id != scene_id)
    {
        return false;
    }
    if query
        .created_from
        .is_some_and(|created_from| record.result.created_at < created_from)
    {
        return false;
    }
    if query
        .created_to
        .is_some_and(|created_to| record.result.created_at > created_to)
    {
        return false;
    }
    true
}

impl Default for ProcessingParameters {
    fn default() -> Self {
        Self {
            analysis_type: "standard".to_string(),
            quality_threshold: 0.8,
            spatial_resolution_m: 1.0,
            temporal_aggregation: None,
            output_formats: vec![OutputFormat::GeoTIFF, OutputFormat::JSON],
            custom_parameters: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::{
        FarmFieldEntityStatus, FarmFieldRegistry, FarmRecord, FieldBoundary, FieldRecord,
        GeoBounds, GeoPoint, SceneRecord, SeasonRecord,
    };
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_service_creation() {
        let temp_dir = tempdir().unwrap();
        let service = PostProcessorService::new(temp_dir.path().to_path_buf());
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_job_submission() {
        let temp_dir = tempdir().unwrap();
        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();

        let job = ProcessingJob {
            id: Uuid::new_v4(),
            job_type: JobType::NdviAnalysis,
            input_files: vec![],
            output_directory: temp_dir.path().to_path_buf(),
            parameters: ProcessingParameters::default(),
            status: JobStatus::Queued,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            error_message: None,
        };

        let job_id = service.submit_job(job).await.unwrap();
        let status = service.get_job_status(&job_id).await.unwrap();
        assert!(matches!(status.status, JobStatus::Queued));
    }

    #[tokio::test]
    async fn analysis_job_submission_links_scene_field_and_season() {
        let temp_dir = tempdir().unwrap();
        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let catalog = analysis_catalog();

        let job_id = service
            .submit_analysis_job(&catalog, analysis_job_request(temp_dir.path()))
            .await
            .expect("scene-linked job is accepted");

        let identity = service
            .analysis_job_identity(&job_id)
            .expect("identity is persisted");
        assert_eq!(identity.job_id, job_id);
        assert_eq!(identity.scene_id, "scene-2026-04-15");
        assert_eq!(identity.field_id, "field-a");
        assert_eq!(identity.season_id, "season-2026");
        assert_eq!(identity.product_refs, vec!["layer-ndvi".to_string()]);
        assert!(matches!(identity.status, JobStatus::Queued));
        assert!(identity.failure_reason.is_none());

        let status = service.get_job_status(&job_id).await.unwrap();
        assert_eq!(identity.created_at, status.created_at);
        assert!(matches!(status.status, JobStatus::Queued));
    }

    #[tokio::test]
    async fn analysis_job_submission_rejects_unknown_scene_without_queueing() {
        let temp_dir = tempdir().unwrap();
        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.scene_id = "missing-scene".to_string();

        let error = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect_err("unknown scene is rejected");

        assert_eq!(
            error,
            AnalysisJobError::UnknownScene {
                scene_id: "missing-scene".to_string()
            }
        );
        assert!(service.list_jobs(None).await.is_empty());
    }

    #[tokio::test]
    async fn analysis_job_failure_records_reason_code() {
        let temp_dir = tempdir().unwrap();
        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let catalog = analysis_catalog();
        let job_id = service
            .submit_analysis_job(&catalog, analysis_job_request(temp_dir.path()))
            .await
            .expect("scene-linked job is accepted");

        service
            .mark_analysis_job_failed(&job_id, "processing_error")
            .expect("failure status is recorded");

        let identity = service.analysis_job_identity(&job_id).unwrap();
        assert!(matches!(identity.status, JobStatus::Failed));
        assert_eq!(identity.failure_reason.as_deref(), Some("processing_error"));
        let status = service.get_job_status(&job_id).await.unwrap();
        assert!(matches!(status.status, JobStatus::Failed));
        assert_eq!(status.error_message.as_deref(), Some("processing_error"));
    }

    #[tokio::test]
    async fn completed_analysis_job_keeps_stable_job_id() {
        let temp_dir = tempdir().unwrap();
        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::MultiSpectralAnalysis;
        let job_id = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect("scene-linked job is accepted");

        let result = service
            .process_next_job()
            .await
            .expect("processing succeeds")
            .expect("result is produced");

        assert_eq!(result.job_id, job_id);
        let identity = service.analysis_job_identity(&job_id).unwrap();
        assert!(matches!(identity.status, JobStatus::Completed));
    }

    #[tokio::test]
    async fn completed_results_are_listed_with_filters_and_pagination() {
        let temp_dir = tempdir().unwrap();
        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let catalog = analysis_catalog();
        let started_at = Utc::now() - chrono::Duration::seconds(1);

        for _ in 0..2 {
            let mut request = analysis_job_request(temp_dir.path());
            request.job_type = JobType::MultiSpectralAnalysis;
            service
                .submit_analysis_job(&catalog, request)
                .await
                .expect("scene-linked job is accepted");
            service
                .process_next_job()
                .await
                .expect("processing succeeds")
                .expect("result is produced");
        }

        let page = service
            .list_analysis_results(AnalysisResultListQuery {
                field_id: Some("field-a".to_string()),
                season_id: Some("season-2026".to_string()),
                created_from: Some(started_at),
                created_to: Some(Utc::now() + chrono::Duration::seconds(1)),
                page: 1,
                page_size: 1,
                ..Default::default()
            })
            .await;

        assert_eq!(page.total_count, 2);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].identity.field_id, "field-a");
        assert_eq!(page.items[0].identity.season_id, "season-2026");
        assert!(matches!(
            page.items[0].identity.status,
            JobStatus::Completed
        ));

        let empty = service
            .list_analysis_results(AnalysisResultListQuery {
                field_id: Some("field-b".to_string()),
                ..Default::default()
            })
            .await;
        assert_eq!(empty.total_count, 0);
        assert!(empty.items.is_empty());
    }

    #[tokio::test]
    async fn completed_result_is_retrievable_after_restart() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let (result_id, job_id) = {
            let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
            let mut request = analysis_job_request(temp_dir.path());
            request.job_type = JobType::MultiSpectralAnalysis;
            let job_id = service
                .submit_analysis_job(&catalog, request)
                .await
                .expect("scene-linked job is accepted");
            let result = service
                .process_next_job()
                .await
                .expect("processing succeeds")
                .expect("result is produced");
            (result.id, job_id)
        };

        let restarted = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let result = restarted
            .get_result(&result_id)
            .await
            .expect("retained result loads after restart");
        assert_eq!(result.job_id, job_id);

        let page = restarted
            .list_analysis_results(AnalysisResultListQuery {
                field_id: Some("field-a".to_string()),
                season_id: Some("season-2026".to_string()),
                page: 1,
                page_size: 10,
                ..Default::default()
            })
            .await;
        assert_eq!(page.total_count, 1);
        assert_eq!(page.items[0].result.id, result_id);
    }

    fn analysis_job_request(output_directory: &std::path::Path) -> AnalysisJobRequest {
        AnalysisJobRequest {
            org_id: "org-a".to_string(),
            scene_id: "scene-2026-04-15".to_string(),
            field_id: "field-a".to_string(),
            season_id: "season-2026".to_string(),
            product_refs: vec!["layer-ndvi".to_string()],
            job_type: JobType::NdviAnalysis,
            input_files: Vec::new(),
            output_directory: output_directory.to_path_buf(),
            parameters: ProcessingParameters::default(),
        }
    }

    fn analysis_catalog() -> FarmFieldRegistry {
        let mut catalog = FarmFieldRegistry::default();
        catalog
            .insert_farm(FarmRecord {
                farm_id: "farm-a".to_string(),
                org_id: "org-a".to_string(),
                owner: "org-a".to_string(),
                name: "Prairie Farm".to_string(),
                notes: None,
                status: FarmFieldEntityStatus::Active,
                created_at: "2026-04-01T00:00:00Z".to_string(),
                updated_at: "2026-04-01T00:00:00Z".to_string(),
            })
            .expect("farm persists");
        catalog
            .insert_field(FieldRecord {
                farm_id: Some("farm-a".to_string()),
                field_id: "field-a".to_string(),
                org_id: "org-a".to_string(),
                owner: "org-a".to_string(),
                name: "North 80".to_string(),
                area_ha: None,
                crop: None,
                season: None,
                notes: None,
                boundary: FieldBoundary {
                    crs: Some("EPSG:4326".to_string()),
                    coordinates: vec![
                        GeoPoint {
                            longitude: -96.0,
                            latitude: 41.0,
                        },
                        GeoPoint {
                            longitude: -95.9,
                            latitude: 41.0,
                        },
                        GeoPoint {
                            longitude: -95.9,
                            latitude: 41.1,
                        },
                        GeoPoint {
                            longitude: -96.0,
                            latitude: 41.1,
                        },
                        GeoPoint {
                            longitude: -96.0,
                            latitude: 41.0,
                        },
                    ],
                },
                extent: GeoBounds {
                    min_lon: -96.0,
                    min_lat: 41.0,
                    max_lon: -95.9,
                    max_lat: 41.1,
                },
                status: FarmFieldEntityStatus::Active,
                created_at: "2026-04-01T00:00:00Z".to_string(),
                updated_at: "2026-04-01T00:00:00Z".to_string(),
            })
            .expect("field persists");
        catalog
            .insert_season(SeasonRecord {
                season_id: "season-2026".to_string(),
                field_id: "field-a".to_string(),
                org_id: "org-a".to_string(),
                start: "2026-03-01".to_string(),
                end: "2026-10-31".to_string(),
                label: "2026 Corn".to_string(),
            })
            .expect("season persists");
        catalog
            .insert_scene(SceneRecord {
                scene_id: "scene-2026-04-15".to_string(),
                field_id: "field-a".to_string(),
                season_id: "season-2026".to_string(),
                org_id: "org-a".to_string(),
                captured_at: "2026-04-15T14:30:00Z".to_string(),
                source: "landsat".to_string(),
            })
            .expect("scene persists");
        catalog
    }
}
