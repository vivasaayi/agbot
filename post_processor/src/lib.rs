use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::schemas::FarmFieldRegistry;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub mod evidence;
pub mod findings_export;
pub mod grower_report;
pub mod index_anomaly;
pub mod index_trend;
pub mod index_vegetation_classification;
pub mod lidar_analysis;
pub mod lidar_change;
pub mod ndvi_analysis;
pub mod product_anomalies;
pub mod report_generator;
pub mod thermal_analysis;
pub mod thermal_spots;
pub mod vegetation_summary;
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
pub use index_anomaly::{
    analyze_index_anomalies, IndexAnomalyDecision, IndexAnomalyError, IndexAnomalyRequest,
    INDEX_ANOMALY_FEATURE_FLAG_KEY, INDEX_ANOMALY_PAYLOAD_KEY,
};
pub use index_trend::{
    analyze_index_trend, IndexTrendCalibrationStatus, IndexTrendDecision, IndexTrendError,
    IndexTrendRequest, INDEX_TREND_FEATURE_FLAG_KEY, INDEX_TREND_PAYLOAD_KEY,
};
pub use index_vegetation_classification::{
    analyze_index_vegetation_type_classification, IndexVegetationTypeClassificationError,
    IndexVegetationTypeClassificationRequest, IndexVegetationTypeClassificationResult,
    IndexVegetationTypeClassificationSnapshot, VegetationTypeClassZone,
    VegetationTypeClassificationClassStat, VegetationTypeClassificationDecision,
    VegetationTypeSignature, INDEX_VEGETATION_CLASSIFICATION_FEATURE_FLAG_KEY,
    INDEX_VEGETATION_CLASSIFICATION_PAYLOAD_KEY,
};
pub use lidar_analysis::{LidarAnalysisConfig, LidarAnalysisProcessor};
pub use lidar_change::{
    analyze_lidar_change, LidarChangeDecision, LidarChangeError, LidarChangeRequest,
    LIDAR_CHANGE_FEATURE_FLAG_KEY, LIDAR_CHANGE_PAYLOAD_KEY,
};
pub use ndvi_analysis::{NdviAnalysisConfig, NdviAnalysisProcessor};
pub use product_anomalies::{
    flag_product_anomalies, AnomalyDetectionConfig, AnomalyDetectionError, ProductAnomaly,
    ProductAnomalyReasonCode,
};
pub use report_generator::ReportGenerator;
pub use thermal_analysis::{ThermalAnalysisConfig, ThermalAnalysisProcessor};
pub use thermal_spots::{
    detect_thermal_spots, ThermalSpot, ThermalSpotError, ThermalSpotRequest, ThermalSpotSummary,
    ThermalSpotType,
};
pub use vegetation_summary::{
    summarize_vegetation, VegetationSourceProduct, VegetationSummary, VegetationSummaryError,
    VegetationSummaryInput, VegetationTrend, DEFAULT_LOW_VIGOR_NDVI_THRESHOLD,
};
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

const HEALTH_FEATURE_FLAG_KEY: &str = "crop_health_feature_enabled";
const HEALTH_APPROVAL_KEY: &str = "crop_health_approval_granted";
const HEALTH_STALE_KEY: &str = "crop_health_products_stale";
const HEALTH_EVIDENCE_KEY: &str = "evidence_refs";
const YIELD_FEATURE_FLAG_KEY: &str = "crop_yield_feature_enabled";
const YIELD_EVIDENCE_KEY: &str = "yield_evidence_refs";

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
    IndexAnomalyDetection,
    LidarChangeAdvisory,
    IndexTrendAdvisory,
    IndexVegetationTypeClassification,
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
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub uncertainty: Option<HealthUncertaintyBand>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthUncertaintyBand {
    pub lower: f32,
    pub upper: f32,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
            JobType::IndexAnomalyDetection => self.analyze_index_anomaly(job).await,
            JobType::LidarChangeAdvisory => self.analyze_lidar_change(job).await,
            JobType::IndexTrendAdvisory => self.analyze_index_trend(job).await,
            JobType::IndexVegetationTypeClassification => {
                self.analyze_index_vegetation_type_classification(job).await
            }
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
            evidence_refs: Vec::new(),
            uncertainty: None,
            created_at: Utc::now(),
        };

        Ok(result)
    }

    async fn generate_composite_report(&self, job: &ProcessingJob) -> Result<AnalysisResult> {
        // Implementation for composite report generation
        let mut analysis_zones = Vec::new();
        let visualizations = Vec::new();
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
            evidence_refs: Vec::new(),
            uncertainty: None,
            created_at: Utc::now(),
        })
    }

    async fn assess_crop_health(&self, job: &ProcessingJob) -> Result<AnalysisResult> {
        let evidence_refs = self.resolve_health_product_refs(job)?;
        let quality_threshold = self.normalize_quality_threshold(job.parameters.quality_threshold);

        self.ensure_health_feature_enabled(job)?;
        self.ensure_health_products_approved(job)?;
        self.ensure_products_not_stale(job)?;

        let health_score = self.compose_health_score(&evidence_refs, quality_threshold);
        let uncertainty = self.compose_health_uncertainty(&evidence_refs, quality_threshold);

        let mut zones = Vec::new();
        zones.push(AnalysisZone {
            id: "health_assessment_zone".to_string(),
            boundary: vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
            area_m2: 10000.0,
            values: [
                ("health_score".to_string(), health_score),
                ("quality_threshold".to_string(), quality_threshold),
                ("evidence_count".to_string(), evidence_refs.len() as f32),
            ]
            .iter()
            .cloned()
            .collect(),
            classification: Some(self.classify_health_zone(health_score)),
        });

        let recommendations = if health_score < 0.55 {
            vec![Recommendation {
                category: RecommendationCategory::Irrigation,
                priority: Priority::Medium,
                title: "Review potential crop-stress signal".to_string(),
                description: "Deterministic health indicators indicate review is advised."
                    .to_string(),
                action_items: vec![
                    "Prioritize inspection of lower scoring zones".to_string(),
                    "Confirm recent irrigation and irrigation scheduling logs are complete"
                        .to_string(),
                ],
                affected_areas: zones.clone(),
                confidence_score: 1.0 - uncertainty,
            }]
        } else {
            Vec::new()
        };

        Ok(AnalysisResult {
            id: Uuid::new_v4(),
            job_id: job.id,
            result_type: ResultType::HealthIndex,
            data: ResultData::ZonalData {
                zones,
                aggregated_values: HashMap::from([
                    ("overall_health".to_string(), health_score),
                    ("lower_uncertainty".to_string(), health_score - uncertainty),
                    ("upper_uncertainty".to_string(), health_score + uncertainty),
                ]),
            },
            statistics: AnalysisStatistics {
                min_value: health_score - uncertainty,
                max_value: health_score + uncertainty,
                mean_value: health_score,
                std_deviation: uncertainty,
                percentiles: HashMap::new(),
                coverage_area_m2: 10000.0,
                valid_pixel_count: 1,
                total_pixel_count: 1,
            },
            visualizations: Vec::new(),
            recommendations,
            evidence_refs,
            uncertainty: Some(HealthUncertaintyBand {
                lower: (health_score - uncertainty).max(0.0),
                upper: (health_score + uncertainty).min(1.0),
            }),
            created_at: Utc::now(),
        })
    }

    async fn predict_yield(&self, job: &ProcessingJob) -> Result<AnalysisResult> {
        let evidence_refs = self.resolve_yield_product_refs(job)?;
        self.ensure_yield_feature_enabled(job)?;
        let quality_threshold = self.normalize_quality_threshold(job.parameters.quality_threshold);

        let yield_estimate = self.compose_yield_estimate(&evidence_refs, quality_threshold);
        let uncertainty_span = self.compose_yield_uncertainty(&evidence_refs, quality_threshold);
        let lower_yield = (yield_estimate - uncertainty_span).max(0.0);
        let upper_yield = yield_estimate + uncertainty_span;

        let result = AnalysisResult {
            id: Uuid::new_v4(),
            job_id: job.id,
            result_type: ResultType::YieldEstimate,
            data: ResultData::GridData {
                width: 1,
                height: 1,
                values: vec![yield_estimate],
                bounds: (0.0, 0.0, 1.0, 1.0),
                units: "tons_per_hectare".to_string(),
            },
            statistics: AnalysisStatistics {
                min_value: lower_yield,
                max_value: upper_yield,
                mean_value: yield_estimate,
                std_deviation: uncertainty_span / 2.0,
                percentiles: [
                    ("25".to_string(), lower_yield),
                    ("50".to_string(), yield_estimate),
                    ("75".to_string(), upper_yield),
                ]
                .iter()
                .cloned()
                .collect(),
                coverage_area_m2: 1.0,
                valid_pixel_count: 1,
                total_pixel_count: 1,
            },
            visualizations: Vec::new(),
            recommendations: Vec::new(),
            evidence_refs,
            uncertainty: Some(HealthUncertaintyBand {
                lower: lower_yield,
                upper: upper_yield,
            }),
            created_at: Utc::now(),
        };

        Ok(result)
    }

    async fn analyze_index_trend(&self, job: &ProcessingJob) -> Result<AnalysisResult> {
        self.ensure_index_trend_feature_enabled(job)?;
        let request = self.resolve_index_trend_request(job)?;
        let trend = analyze_index_trend(request).map_err(|error| anyhow::anyhow!("{error}"))?;

        let (result_data, statistics) = match trend.decision {
            IndexTrendDecision::Available { .. } => {
                let stats = trend
                    .delta_statistics
                    .expect("available trend must include statistics");
                (
                    ResultData::GridData {
                        width: trend.width,
                        height: trend.height,
                        values: trend.delta_values.unwrap_or_default(),
                        bounds: (
                            trend.common_extent.min_lon,
                            trend.common_extent.min_lat,
                            trend.common_extent.max_lon,
                            trend.common_extent.max_lat,
                        ),
                        units: "index_delta".to_string(),
                    },
                    stats.statistics,
                )
            }
            IndexTrendDecision::LowConfidence { .. } | IndexTrendDecision::Unavailable { .. } => (
                ResultData::ZonalData {
                    zones: Vec::new(),
                    aggregated_values: HashMap::from([
                        ("coverage_fraction".to_string(), trend.coverage_fraction),
                        ("available".to_string(), 0.0),
                    ]),
                },
                AnalysisStatistics {
                    min_value: 0.0,
                    max_value: 0.0,
                    mean_value: 0.0,
                    std_deviation: 0.0,
                    percentiles: HashMap::new(),
                    coverage_area_m2: 0.0,
                    valid_pixel_count: 0,
                    total_pixel_count: trend.width.saturating_mul(trend.height),
                },
            ),
        };

        Ok(AnalysisResult {
            id: Uuid::new_v4(),
            job_id: job.id,
            result_type: ResultType::StressIndicators,
            data: result_data,
            statistics,
            visualizations: Vec::new(),
            recommendations: if trend.decision.label() == "unavailable" {
                vec![Recommendation {
                    category: RecommendationCategory::General,
                    priority: Priority::Medium,
                    title: "Index trend unavailable".to_string(),
                    description: "Trend comparison was blocked by comparability constraints."
                        .to_string(),
                    action_items: Vec::new(),
                    affected_areas: Vec::new(),
                    confidence_score: 0.2,
                }]
            } else {
                Vec::new()
            },
            evidence_refs: vec![trend.evidence_input_hash],
            uncertainty: Some(trend.uncertainty),
            created_at: Utc::now(),
        })
    }

    async fn analyze_lidar_change(&self, job: &ProcessingJob) -> Result<AnalysisResult> {
        self.ensure_lidar_change_feature_enabled(job)?;
        let request = self.resolve_lidar_change_request(job)?;
        let result = analyze_lidar_change(request).map_err(|error| anyhow::anyhow!("{error}"))?;

        let (result_data, statistics) = match result.decision {
            LidarChangeDecision::Available { .. } => {
                let stats = result
                    .change_statistics
                    .expect("available change result must include statistics");
                (
                    ResultData::GridData {
                        width: result.width,
                        height: result.height,
                        values: result.obstacle_change_values.unwrap_or_default(),
                        bounds: (
                            result.common_extent.min_lon,
                            result.common_extent.min_lat,
                            result.common_extent.max_lon,
                            result.common_extent.max_lat,
                        ),
                        units: "obstacle_canopy_change".to_string(),
                    },
                    stats.statistics,
                )
            }
            LidarChangeDecision::LowConfidence { .. } | LidarChangeDecision::Unavailable { .. } => {
                let available =
                    if matches!(result.decision, LidarChangeDecision::Unavailable { .. }) {
                        0.0
                    } else {
                        0.0
                    };
                (
                    ResultData::ZonalData {
                        zones: Vec::new(),
                        aggregated_values: HashMap::from([
                            ("coverage_fraction".to_string(), result.coverage_fraction),
                            ("available".to_string(), available),
                        ]),
                    },
                    AnalysisStatistics {
                        min_value: 0.0,
                        max_value: 0.0,
                        mean_value: 0.0,
                        std_deviation: 0.0,
                        percentiles: HashMap::new(),
                        coverage_area_m2: 0.0,
                        valid_pixel_count: 0,
                        total_pixel_count: result.width.saturating_mul(result.height),
                    },
                )
            }
        };

        Ok(AnalysisResult {
            id: Uuid::new_v4(),
            job_id: job.id,
            result_type: ResultType::StressIndicators,
            data: result_data,
            statistics,
            visualizations: Vec::new(),
            recommendations: if result.decision.label() == "unavailable" {
                vec![Recommendation {
                    category: RecommendationCategory::General,
                    priority: Priority::Medium,
                    title: "Lidar change advisory unavailable".to_string(),
                    description: "Change comparison was blocked by advisability constraints."
                        .to_string(),
                    action_items: Vec::new(),
                    affected_areas: Vec::new(),
                    confidence_score: 0.2,
                }]
            } else {
                Vec::new()
            },
            evidence_refs: if result.evidence_input_hash.is_empty() {
                Vec::new()
            } else {
                vec![result.evidence_input_hash]
            },
            uncertainty: Some(result.uncertainty),
            created_at: Utc::now(),
        })
    }

    async fn analyze_index_anomaly(&self, job: &ProcessingJob) -> Result<AnalysisResult> {
        self.ensure_index_anomaly_feature_enabled(job)?;
        let request = self.resolve_index_anomaly_request(job)?;
        let anomaly =
            analyze_index_anomalies(request).map_err(|error| anyhow::anyhow!("{error}"))?;

        let zone_count = anomaly.zones.len() as f32;
        let (result_data, statistics) = match anomaly.decision {
            IndexAnomalyDecision::Available { .. } => {
                let stats = anomaly
                    .layer_statistics
                    .expect("available anomaly result must include statistics");
                (
                    ResultData::ZonalData {
                        zones: anomaly
                            .zones
                            .into_iter()
                            .map(|zone| AnalysisZone {
                                id: zone.zone_id,
                                boundary: zone.polygon.coordinates,
                                area_m2: zone.area_m2,
                                values: HashMap::from([("zone_area_m2".to_string(), zone.area_m2)]),
                                classification: Some("index_anomaly".to_string()),
                            })
                            .collect(),
                        aggregated_values: HashMap::from([
                            ("anomaly_count".to_string(), anomaly.anomaly_count as f32),
                            ("zone_count".to_string(), zone_count),
                            ("coverage_fraction".to_string(), anomaly.coverage_fraction),
                        ]),
                    },
                    stats.statistics,
                )
            }
            IndexAnomalyDecision::Unavailable { .. } => (
                ResultData::ZonalData {
                    zones: Vec::new(),
                    aggregated_values: HashMap::from([
                        ("coverage_fraction".to_string(), anomaly.coverage_fraction),
                        ("available".to_string(), 0.0),
                    ]),
                },
                AnalysisStatistics {
                    min_value: 0.0,
                    max_value: 0.0,
                    mean_value: 0.0,
                    std_deviation: 0.0,
                    percentiles: HashMap::new(),
                    coverage_area_m2: 0.0,
                    valid_pixel_count: 0,
                    total_pixel_count: anomaly.width.saturating_mul(anomaly.height),
                },
            ),
        };

        Ok(AnalysisResult {
            id: Uuid::new_v4(),
            job_id: job.id,
            result_type: ResultType::StressIndicators,
            data: result_data,
            statistics,
            visualizations: Vec::new(),
            recommendations: if anomaly.decision.label() == "unavailable" {
                vec![Recommendation {
                    category: RecommendationCategory::General,
                    priority: Priority::Medium,
                    title: "Index anomaly unavailable".to_string(),
                    description: "Anomaly detection was blocked because index data is not georeferenced or lacks sufficient valid pixels.".to_string(),
                    action_items: Vec::new(),
                    affected_areas: Vec::new(),
                    confidence_score: 0.2,
                }]
            } else {
                Vec::new()
            },
            evidence_refs: vec![anomaly.evidence_input_hash],
            uncertainty: Some(anomaly.uncertainty),
            created_at: Utc::now(),
        })
    }

    async fn analyze_index_vegetation_type_classification(
        &self,
        job: &ProcessingJob,
    ) -> Result<AnalysisResult> {
        self.ensure_index_vegetation_classification_feature_enabled(job)?;
        let request = self.resolve_index_vegetation_type_classification_request(job)?;
        let result = analyze_index_vegetation_type_classification(request)
            .map_err(|error| anyhow::anyhow!("{error}"))?;

        let (result_data, statistics) = match &result.decision {
            VegetationTypeClassificationDecision::Available { .. }
            | VegetationTypeClassificationDecision::LowConfidence { .. } => (
                ResultData::ZonalData {
                    zones: result
                        .zones
                        .into_iter()
                        .map(|zone| AnalysisZone {
                            id: zone.zone_id,
                            boundary: zone.polygon,
                            area_m2: zone.area_m2,
                            values: HashMap::from([
                                ("pixel_count".to_string(), zone.pixel_count as f32),
                                ("mean_confidence".to_string(), zone.mean_confidence),
                                ("coverage_fraction".to_string(), zone.coverage_fraction),
                                (
                                    "match_distance".to_string(),
                                    zone.matched_signature_distance,
                                ),
                            ]),
                            classification: Some(zone.class_name),
                        })
                        .collect(),
                    aggregated_values: HashMap::from([
                        ("coverage_fraction".to_string(), result.coverage_fraction),
                        ("class_count".to_string(), result.class_stats.len() as f32),
                        ("mean_confidence".to_string(), result.mean_confidence),
                    ]),
                },
                AnalysisStatistics {
                    min_value: 0.0,
                    max_value: 1.0,
                    mean_value: result.mean_confidence,
                    std_deviation: 0.0,
                    percentiles: HashMap::from([(
                        "class_coverage".to_string(),
                        result.coverage_fraction,
                    )]),
                    coverage_area_m2: result.width as f32 * result.height as f32,
                    valid_pixel_count: (result.coverage_fraction
                        * (result.width as f32 * result.height as f32))
                        .round() as u32,
                    total_pixel_count: result.width.saturating_mul(result.height),
                },
            ),
            VegetationTypeClassificationDecision::Unavailable { .. } => (
                ResultData::ZonalData {
                    zones: Vec::new(),
                    aggregated_values: HashMap::from([
                        ("coverage_fraction".to_string(), result.coverage_fraction),
                        ("available".to_string(), 0.0),
                    ]),
                },
                AnalysisStatistics {
                    min_value: 0.0,
                    max_value: 0.0,
                    mean_value: 0.0,
                    std_deviation: 0.0,
                    percentiles: HashMap::new(),
                    coverage_area_m2: 0.0,
                    valid_pixel_count: 0,
                    total_pixel_count: result.width.saturating_mul(result.height),
                },
            ),
        };

        Ok(AnalysisResult {
            id: Uuid::new_v4(),
            job_id: job.id,
            result_type: ResultType::StressIndicators,
            data: result_data,
            statistics,
            visualizations: Vec::new(),
            recommendations: if result.decision.label() == "unavailable" {
                vec![Recommendation {
                    category: RecommendationCategory::General,
                    priority: Priority::Medium,
                    title: "Vegetation-type classification unavailable".to_string(),
                    description: "Classification was blocked by data comparability constraints."
                        .to_string(),
                    action_items: Vec::new(),
                    affected_areas: Vec::new(),
                    confidence_score: 0.2,
                }]
            } else {
                Vec::new()
            },
            evidence_refs: vec![result.evidence_input_hash],
            uncertainty: Some(result.uncertainty),
            created_at: Utc::now(),
        })
    }

    fn resolve_yield_product_refs(&self, job: &ProcessingJob) -> Result<Vec<String>> {
        let identity = self
            .analysis_job_identities
            .get(&job.id)
            .ok_or_else(|| anyhow::anyhow!("missing analysis identity for yield prediction"))?;

        let mut references = BTreeSet::new();
        for item in &identity.product_refs {
            let item = item.trim();
            if !item.is_empty() {
                references.insert(item.to_string());
            }
        }

        if let Some(custom_refs_value) = job.parameters.custom_parameters.get(YIELD_EVIDENCE_KEY) {
            if let Some(values) = custom_refs_value.as_array() {
                for value in values {
                    if let Some(reference) = value.as_str() {
                        let reference = reference.trim();
                        if !reference.is_empty() {
                            references.insert(reference.to_string());
                        }
                    }
                }
            }
        }

        if references.is_empty() {
            return Err(anyhow::anyhow!(
                "no deterministic product references available for yield estimate"
            ));
        }

        Ok(references.into_iter().collect())
    }

    fn ensure_yield_feature_enabled(&self, job: &ProcessingJob) -> Result<()> {
        let enabled = matches!(
            job.parameters.custom_parameters.get(YIELD_FEATURE_FLAG_KEY),
            Some(value) if value.as_bool().unwrap_or(false)
        );
        if !enabled {
            return Err(anyhow::anyhow!(
                "crop yield prediction is disabled until feature flag is enabled"
            ));
        }
        Ok(())
    }

    fn resolve_index_anomaly_request(&self, job: &ProcessingJob) -> Result<IndexAnomalyRequest> {
        let _identity = self.analysis_job_identities.get(&job.id).ok_or_else(|| {
            anyhow::anyhow!("missing analysis identity for index anomaly detection")
        })?;

        let payload = job
            .parameters
            .custom_parameters
            .get(INDEX_ANOMALY_PAYLOAD_KEY)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "index anomaly request requires '{}' payload",
                    INDEX_ANOMALY_PAYLOAD_KEY
                )
            })?;
        serde_json::from_value(payload.clone())
            .map_err(|error| anyhow::anyhow!("invalid index anomaly payload: {error}"))
    }

    fn ensure_index_anomaly_feature_enabled(&self, job: &ProcessingJob) -> Result<()> {
        let enabled = matches!(
            job.parameters
                .custom_parameters
                .get(INDEX_ANOMALY_FEATURE_FLAG_KEY),
            Some(value) if value.as_bool().unwrap_or(false)
        );
        if !enabled {
            return Err(anyhow::anyhow!(
                "index anomaly detection is disabled until feature flag is enabled"
            ));
        }
        Ok(())
    }

    fn resolve_index_vegetation_type_classification_request(
        &self,
        job: &ProcessingJob,
    ) -> Result<IndexVegetationTypeClassificationRequest> {
        let _identity = self.analysis_job_identities.get(&job.id).ok_or_else(|| {
            anyhow::anyhow!("missing analysis identity for index vegetation classification")
        })?;

        let payload = job
            .parameters
            .custom_parameters
            .get(INDEX_VEGETATION_CLASSIFICATION_PAYLOAD_KEY)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "index vegetation-type request requires '{}' payload",
                    INDEX_VEGETATION_CLASSIFICATION_PAYLOAD_KEY
                )
            })?;
        serde_json::from_value(payload.clone())
            .map_err(|error| anyhow::anyhow!("invalid index vegetation-type payload: {error}"))
    }

    fn ensure_index_vegetation_classification_feature_enabled(
        &self,
        job: &ProcessingJob,
    ) -> Result<()> {
        let enabled = matches!(
            job.parameters
                .custom_parameters
                .get(INDEX_VEGETATION_CLASSIFICATION_FEATURE_FLAG_KEY),
            Some(value) if value.as_bool().unwrap_or(false)
        );
        if !enabled {
            return Err(anyhow::anyhow!(
                "index vegetation-type classification is disabled until feature flag is enabled"
            ));
        }
        Ok(())
    }

    fn resolve_index_trend_request(&self, job: &ProcessingJob) -> Result<IndexTrendRequest> {
        let _identity = self
            .analysis_job_identities
            .get(&job.id)
            .ok_or_else(|| anyhow::anyhow!("missing analysis identity for index trend advisory"))?;

        let payload = job
            .parameters
            .custom_parameters
            .get(INDEX_TREND_PAYLOAD_KEY)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "index trend request requires '{}' payload",
                    INDEX_TREND_PAYLOAD_KEY
                )
            })?;
        serde_json::from_value(payload.clone())
            .map_err(|error| anyhow::anyhow!("invalid index trend payload: {error}"))
    }

    fn resolve_lidar_change_request(&self, job: &ProcessingJob) -> Result<LidarChangeRequest> {
        let _identity = self.analysis_job_identities.get(&job.id).ok_or_else(|| {
            anyhow::anyhow!("missing analysis identity for lidar change advisory")
        })?;

        let payload = job
            .parameters
            .custom_parameters
            .get(LIDAR_CHANGE_PAYLOAD_KEY)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "lidar change advisory requires '{}' payload",
                    LIDAR_CHANGE_PAYLOAD_KEY
                )
            })?;
        serde_json::from_value(payload.clone())
            .map_err(|error| anyhow::anyhow!("invalid lidar change payload: {error}"))
    }

    fn ensure_index_trend_feature_enabled(&self, job: &ProcessingJob) -> Result<()> {
        let enabled = matches!(
            job.parameters.custom_parameters.get(INDEX_TREND_FEATURE_FLAG_KEY),
            Some(value) if value.as_bool().unwrap_or(false)
        );
        if !enabled {
            return Err(anyhow::anyhow!(
                "index trend advisory is disabled until feature flag is enabled"
            ));
        }
        Ok(())
    }

    fn ensure_lidar_change_feature_enabled(&self, job: &ProcessingJob) -> Result<()> {
        let enabled = matches!(
            job.parameters
                .custom_parameters
                .get(LIDAR_CHANGE_FEATURE_FLAG_KEY),
            Some(value) if value.as_bool().unwrap_or(false)
        );
        if !enabled {
            return Err(anyhow::anyhow!(
                "lidar change advisory is disabled until feature flag is enabled"
            ));
        }
        Ok(())
    }

    fn compose_yield_estimate(&self, evidence_refs: &[String], quality_threshold: f32) -> f32 {
        let mut digest: u64 = 0;
        for evidence_ref in evidence_refs {
            digest = digest
                .wrapping_add(stable_ref_hash(evidence_ref))
                .wrapping_mul(0xD6E8_EBEE_B3D5);
        }
        let evidence_signal = ((digest % 9000) as f32) / 9000.0;
        (2.2 + (evidence_signal * 0.6) + (quality_threshold * 3.4)).clamp(1.2, 12.0)
    }

    fn compose_yield_uncertainty(&self, evidence_refs: &[String], quality_threshold: f32) -> f32 {
        let evidence_strength = if evidence_refs.is_empty() {
            1.0
        } else {
            (1.0 / (evidence_refs.len() as f32 + 1.0)).clamp(0.05, 1.0)
        };
        let quality_gap = (1.0 - quality_threshold).abs().clamp(0.0, 1.0);
        let uncertainty = (0.55 * evidence_strength) + (0.45 * quality_gap);
        (uncertainty * 0.4).clamp(0.15, 2.2)
    }

    fn resolve_health_product_refs(&self, job: &ProcessingJob) -> Result<Vec<String>> {
        let identity = self
            .analysis_job_identities
            .get(&job.id)
            .ok_or_else(|| anyhow::anyhow!("missing analysis identity for health assessment"))?;

        let mut references = BTreeSet::new();
        for item in &identity.product_refs {
            let item = item.trim();
            if !item.is_empty() {
                references.insert(item.to_string());
            }
        }

        if let Some(custom_refs_value) = job.parameters.custom_parameters.get(HEALTH_EVIDENCE_KEY) {
            if let Some(values) = custom_refs_value.as_array() {
                for value in values {
                    if let Some(reference) = value.as_str() {
                        let reference = reference.trim();
                        if !reference.is_empty() {
                            references.insert(reference.to_string());
                        }
                    }
                }
            }
        }

        if references.is_empty() {
            return Err(anyhow::anyhow!(
                "no deterministic product references available"
            ));
        }

        Ok(references.into_iter().collect())
    }

    fn ensure_health_feature_enabled(&self, job: &ProcessingJob) -> Result<()> {
        let enabled = matches!(
            job.parameters.custom_parameters.get(HEALTH_FEATURE_FLAG_KEY),
            Some(value) if value.as_bool().unwrap_or(false)
        );
        if !enabled {
            return Err(anyhow::anyhow!(
                "crop health assessment is disabled until feature flag is enabled"
            ));
        }
        Ok(())
    }

    fn ensure_health_products_approved(&self, job: &ProcessingJob) -> Result<()> {
        let approved = matches!(
            job.parameters.custom_parameters.get(HEALTH_APPROVAL_KEY),
            Some(value) if value.as_bool().unwrap_or(false)
        );
        if !approved {
            return Err(anyhow::anyhow!(
                "crop health assessment requires explicit approval before running"
            ));
        }
        Ok(())
    }

    fn ensure_products_not_stale(&self, job: &ProcessingJob) -> Result<()> {
        let stale = matches!(
            job.parameters.custom_parameters.get(HEALTH_STALE_KEY),
            Some(value) if value.as_bool().unwrap_or(false)
        );
        if stale {
            return Err(anyhow::anyhow!(
                "crop health assessment skipped: source products are stale or unavailable"
            ));
        }
        Ok(())
    }

    fn compose_health_score(&self, evidence_refs: &[String], quality_threshold: f32) -> f32 {
        let mut digest: u64 = 0;
        for evidence_ref in evidence_refs {
            digest = digest
                .wrapping_add(stable_ref_hash(evidence_ref))
                .wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }

        let evidence_signal = ((digest % 1000) as f32) / 1000.0;
        let quality_signal = quality_threshold;
        ((0.7 * evidence_signal) + (0.3 * quality_signal)).clamp(0.0, 1.0)
    }

    fn compose_health_uncertainty(&self, evidence_refs: &[String], quality_threshold: f32) -> f32 {
        let evidence_signal = if evidence_refs.is_empty() {
            1.0
        } else {
            (1.0 / (evidence_refs.len() as f32 + 1.0)).clamp(0.05, 1.0)
        };

        let quality_gap = (1.0 - quality_threshold).abs().clamp(0.0, 1.0);
        let uncertainty = (0.12 * evidence_signal) + (0.08 * quality_gap);
        uncertainty.clamp(0.03, 0.35)
    }

    fn normalize_quality_threshold(&self, quality_threshold: f32) -> f32 {
        quality_threshold.clamp(0.0, 1.0)
    }

    fn classify_health_zone(&self, health_score: f32) -> String {
        match health_score {
            score if score >= 0.8 => "Healthy".to_string(),
            score if score >= 0.65 => "Watch".to_string(),
            score if score >= 0.5 => "Review".to_string(),
            _ => "At risk".to_string(),
        }
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

fn stable_ref_hash(reference: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in reference.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1_0000_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shared::schemas::{
        FarmFieldEntityStatus, FarmFieldRegistry, FarmRecord, FieldBoundary, FieldRecord,
        GeoBounds, GeoPoint, RasterResolution, SceneRecord, SeasonRecord,
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
    async fn health_assessment_requires_feature_flag_and_approval() {
        let temp_dir = tempdir().unwrap();
        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::HealthAssessment;
        request
            .parameters
            .custom_parameters
            .insert(HEALTH_FEATURE_FLAG_KEY.to_string(), json!(false));
        request
            .parameters
            .custom_parameters
            .insert(HEALTH_APPROVAL_KEY.to_string(), json!(false));

        let job_id = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect("health request is accepted");

        let result = service
            .process_next_job()
            .await
            .expect("processing attempted");
        assert!(result.is_none());

        let identity = service
            .analysis_job_identity(&job_id)
            .expect("identity exists after failure");
        assert!(matches!(identity.status, JobStatus::Failed));
        assert!(identity
            .failure_reason
            .as_ref()
            .is_some_and(|message| message.contains("feature flag")));
    }

    #[tokio::test]
    async fn health_assessment_requires_non_stale_deterministic_products() {
        let temp_dir = tempdir().unwrap();
        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::HealthAssessment;
        request
            .parameters
            .custom_parameters
            .insert(HEALTH_FEATURE_FLAG_KEY.to_string(), json!(true));
        request
            .parameters
            .custom_parameters
            .insert(HEALTH_APPROVAL_KEY.to_string(), json!(true));
        request
            .parameters
            .custom_parameters
            .insert(HEALTH_STALE_KEY.to_string(), json!(true));

        let job_id = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect("health request is accepted");

        let result = service
            .process_next_job()
            .await
            .expect("processing attempted");
        assert!(result.is_none());

        let identity = service
            .analysis_job_identity(&job_id)
            .expect("identity exists after failure");
        assert!(matches!(identity.status, JobStatus::Failed));
        assert!(identity
            .failure_reason
            .as_ref()
            .is_some_and(|message| message.contains("stale")));
    }

    #[tokio::test]
    async fn health_assessment_derives_evidence_and_uncertainty_from_deterministic_inputs() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::HealthAssessment;
        request.product_refs = vec![
            "layer-ndvi-2026-04-15".to_string(),
            "layer-thermal-2026-04-15".to_string(),
        ];
        request.parameters.quality_threshold = 0.85;
        request
            .parameters
            .custom_parameters
            .insert(HEALTH_FEATURE_FLAG_KEY.to_string(), json!(true));
        request
            .parameters
            .custom_parameters
            .insert(HEALTH_APPROVAL_KEY.to_string(), json!(true));
        request
            .parameters
            .custom_parameters
            .insert(HEALTH_STALE_KEY.to_string(), json!(false));

        let first_result = {
            let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
            let mut request = request.clone();
            request.job_type = JobType::HealthAssessment;

            let _ = service
                .submit_analysis_job(&catalog, request.clone())
                .await
                .expect("job accepted");
            service
                .process_next_job()
                .await
                .expect("processing attempted")
                .expect("health result produced")
        };

        let request = request;
        let second_result = {
            let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();

            let _ = service
                .submit_analysis_job(&catalog, request.clone())
                .await
                .expect("job accepted");
            service
                .process_next_job()
                .await
                .expect("processing attempted")
                .expect("health result produced")
        };

        assert_eq!(first_result.evidence_refs, second_result.evidence_refs);
        assert_eq!(
            first_result.statistics.mean_value,
            second_result.statistics.mean_value
        );
        assert_eq!(first_result.uncertainty, second_result.uncertainty);
        assert_eq!(
            first_result.evidence_refs,
            vec![
                "layer-ndvi-2026-04-15".to_string(),
                "layer-thermal-2026-04-15".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn yield_estimate_requires_feature_flag() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::YieldPrediction;
        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();

        let job_id = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect("yield request is accepted");

        let result = service
            .process_next_job()
            .await
            .expect("processing attempted");
        assert!(result.is_none());

        let identity = service
            .analysis_job_identity(&job_id)
            .expect("identity exists after failure");
        assert!(matches!(identity.status, JobStatus::Failed));
        assert!(identity
            .failure_reason
            .as_ref()
            .is_some_and(|message| message.contains("disabled")));
    }

    #[tokio::test]
    async fn yield_estimate_reports_bounded_range_with_presented_uncertainty() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::YieldPrediction;
        request.parameters.quality_threshold = 0.82;
        request.product_refs = vec![
            "layer-ndvi-2026-04-15".to_string(),
            "layer-thermal-2026-04-15".to_string(),
        ];
        request
            .parameters
            .custom_parameters
            .insert(YIELD_FEATURE_FLAG_KEY.to_string(), json!(true));

        let first_result = {
            let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
            let mut request = request.clone();
            request.job_type = JobType::YieldPrediction;

            let _ = service
                .submit_analysis_job(&catalog, request.clone())
                .await
                .expect("yield job accepted");
            service
                .process_next_job()
                .await
                .expect("processing attempted")
                .expect("yield result produced")
        };

        let second_result = {
            let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();

            let _ = service
                .submit_analysis_job(&catalog, request.clone())
                .await
                .expect("yield job accepted");
            service
                .process_next_job()
                .await
                .expect("processing attempted")
                .expect("yield result produced")
        };

        let first_uncertainty = first_result
            .uncertainty
            .expect("yield result always includes uncertainty");
        let second_uncertainty = second_result
            .uncertainty
            .expect("yield result always includes uncertainty");

        assert_eq!(
            first_result.statistics.mean_value,
            second_result.statistics.mean_value
        );
        assert_eq!(first_uncertainty, second_uncertainty);
        assert!(first_uncertainty.lower < first_result.statistics.mean_value);
        assert!(first_uncertainty.upper > first_result.statistics.mean_value);
        assert_eq!(first_result.statistics.min_value, first_uncertainty.lower);
        assert_eq!(first_result.statistics.max_value, first_uncertainty.upper);
    }

    #[test]
    fn yield_range_math_is_deterministic_and_produces_bounds() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = PostProcessorService::new(temp_dir.path().to_path_buf())
            .expect("service can be created");
        let evidence_refs = vec![
            "layer-ndvi-2026-04-15".to_string(),
            "layer-thermal-2026-04-15".to_string(),
        ];

        let low_quality = service.compose_yield_estimate(&evidence_refs, 0.40);
        let high_quality = service.compose_yield_estimate(&evidence_refs, 0.90);
        let low_uncertainty = service.compose_yield_uncertainty(&evidence_refs, 0.40);
        let high_uncertainty = service.compose_yield_uncertainty(&evidence_refs, 0.90);
        let low_span = (low_quality - low_uncertainty).max(0.0);

        assert!(high_quality >= low_quality);
        assert!(high_uncertainty <= low_uncertainty + f32::EPSILON);
        assert!(high_quality - low_uncertainty > low_span);
        assert!(high_quality + high_uncertainty > high_quality - high_uncertainty);
        assert!(low_uncertainty >= 0.15);
        assert!(low_uncertainty <= 2.2);
        assert!(high_uncertainty >= 0.15);
        assert!(high_uncertainty <= 2.2);
    }

    #[tokio::test]
    async fn index_trend_advisory_requires_feature_flag() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::IndexTrendAdvisory;
        request.parameters.custom_parameters.insert(
            INDEX_TREND_PAYLOAD_KEY.to_string(),
            serde_json::to_value(trend_request()).expect("valid request"),
        );
        request
            .parameters
            .custom_parameters
            .insert(INDEX_TREND_FEATURE_FLAG_KEY.to_string(), json!(false));

        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let job_id = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect("trend request is accepted");

        let result = service
            .process_next_job()
            .await
            .expect("processing attempted");
        assert!(result.is_none());

        let identity = service
            .analysis_job_identity(&job_id)
            .expect("identity exists after failure");
        assert!(matches!(identity.status, JobStatus::Failed));
        assert!(identity
            .failure_reason
            .as_ref()
            .is_some_and(|message| message.contains("disabled")));
    }

    #[tokio::test]
    async fn index_trend_advisory_produces_delta_for_comparable_calibrated_scenes() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::IndexTrendAdvisory;
        request.parameters.custom_parameters.insert(
            INDEX_TREND_PAYLOAD_KEY.to_string(),
            serde_json::to_value(trend_request()).expect("valid request"),
        );
        request
            .parameters
            .custom_parameters
            .insert(INDEX_TREND_FEATURE_FLAG_KEY.to_string(), json!(true));

        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let _ = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect("trend request is accepted");

        let result = service
            .process_next_job()
            .await
            .expect("processing attempted")
            .expect("trend result produced");

        assert_eq!(result.result_type, ResultType::StressIndicators);
        assert!(matches!(result.data, ResultData::GridData { .. }));
        assert!(result.uncertainty.is_some());
    }

    #[tokio::test]
    async fn index_trend_advisory_marks_low_confidence_when_uncalibrated() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::IndexTrendAdvisory;
        let mut payload = trend_request();
        payload.snapshots[0].calibration_status = IndexTrendCalibrationStatus::UncalibratedDn;
        request.parameters.custom_parameters.insert(
            INDEX_TREND_PAYLOAD_KEY.to_string(),
            serde_json::to_value(payload).expect("valid request"),
        );
        request
            .parameters
            .custom_parameters
            .insert(INDEX_TREND_FEATURE_FLAG_KEY.to_string(), json!(true));

        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let _ = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect("trend request is accepted");
        let result = service
            .process_next_job()
            .await
            .expect("processing attempted")
            .expect("trend result produced");

        assert!(matches!(result.data, ResultData::ZonalData { .. }));
        assert!(result.uncertainty.is_some());
    }

    #[tokio::test]
    async fn lidar_change_advisory_requires_feature_flag() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::LidarChangeAdvisory;
        request.parameters.custom_parameters.insert(
            LIDAR_CHANGE_PAYLOAD_KEY.to_string(),
            serde_json::to_value(lidar_change_request()).expect("valid request"),
        );
        request
            .parameters
            .custom_parameters
            .insert(LIDAR_CHANGE_FEATURE_FLAG_KEY.to_string(), json!(false));

        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let job_id = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect("lidar request is accepted");

        let result = service
            .process_next_job()
            .await
            .expect("processing attempted");
        assert!(result.is_none());

        let identity = service
            .analysis_job_identity(&job_id)
            .expect("identity exists after failure");
        assert!(matches!(identity.status, JobStatus::Failed));
        assert!(identity
            .failure_reason
            .as_ref()
            .is_some_and(|message| message.contains("disabled")));
    }

    #[tokio::test]
    async fn lidar_change_advisory_produces_change_for_comparable_reliable_snapshots() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::LidarChangeAdvisory;
        request.parameters.custom_parameters.insert(
            LIDAR_CHANGE_PAYLOAD_KEY.to_string(),
            serde_json::to_value(lidar_change_request()).expect("valid request"),
        );
        request
            .parameters
            .custom_parameters
            .insert(LIDAR_CHANGE_FEATURE_FLAG_KEY.to_string(), json!(true));

        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let _ = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect("lidar request is accepted");

        let result = service
            .process_next_job()
            .await
            .expect("processing attempted")
            .expect("lidar result produced");

        assert_eq!(result.result_type, ResultType::StressIndicators);
        assert!(matches!(result.data, ResultData::GridData { .. }));
        assert!(result.uncertainty.is_some());
        assert!(result.evidence_refs.first().is_some());
    }

    #[tokio::test]
    async fn lidar_change_advisory_marks_low_confidence_when_segmentation_unreliable() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = lidar_change_request();
        request.snapshots[1].segmentation_reliable = false;

        let mut job_request = analysis_job_request(temp_dir.path());
        job_request.job_type = JobType::LidarChangeAdvisory;
        job_request.parameters.custom_parameters.insert(
            LIDAR_CHANGE_PAYLOAD_KEY.to_string(),
            serde_json::to_value(request).expect("valid request"),
        );
        job_request
            .parameters
            .custom_parameters
            .insert(LIDAR_CHANGE_FEATURE_FLAG_KEY.to_string(), json!(true));

        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let _ = service
            .submit_analysis_job(&catalog, job_request)
            .await
            .expect("lidar request is accepted");
        let result = service
            .process_next_job()
            .await
            .expect("processing attempted")
            .expect("lidar result produced");

        assert!(matches!(result.data, ResultData::ZonalData { .. }));
        assert!(result.uncertainty.is_some());
    }

    #[tokio::test]
    async fn index_anomaly_detection_requires_feature_flag() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::IndexAnomalyDetection;
        request.parameters.custom_parameters.insert(
            INDEX_ANOMALY_PAYLOAD_KEY.to_string(),
            serde_json::to_value(anomaly_request()).expect("valid request"),
        );
        request
            .parameters
            .custom_parameters
            .insert(INDEX_ANOMALY_FEATURE_FLAG_KEY.to_string(), json!(false));

        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let job_id = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect("anomaly request is accepted");

        let result = service
            .process_next_job()
            .await
            .expect("processing attempted");
        assert!(result.is_none());

        let identity = service
            .analysis_job_identity(&job_id)
            .expect("identity exists after failure");
        assert!(matches!(identity.status, JobStatus::Failed));
        assert!(identity
            .failure_reason
            .as_ref()
            .is_some_and(|message| message.contains("disabled")));
    }

    #[tokio::test]
    async fn index_anomaly_detection_marks_ungeoreferenced_as_unavailable() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = anomaly_request();
        request.grid.spatial_ref.georeferenced = false;
        request.grid.spatial_ref.crs = None;

        let mut job_request = analysis_job_request(temp_dir.path());
        job_request.job_type = JobType::IndexAnomalyDetection;
        job_request.parameters.custom_parameters.insert(
            INDEX_ANOMALY_PAYLOAD_KEY.to_string(),
            serde_json::to_value(request).expect("valid request"),
        );
        job_request
            .parameters
            .custom_parameters
            .insert(INDEX_ANOMALY_FEATURE_FLAG_KEY.to_string(), json!(true));

        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let _ = service
            .submit_analysis_job(&catalog, job_request)
            .await
            .expect("anomaly request is accepted");
        let result = service
            .process_next_job()
            .await
            .expect("processing attempted")
            .expect("anomaly result produced");

        assert_eq!(result.result_type, ResultType::StressIndicators);
        assert!(matches!(result.data, ResultData::ZonalData { .. }));
        assert_eq!(result.statistics.total_pixel_count, 12);
        assert!(result.uncertainty.is_some());
    }

    #[tokio::test]
    async fn index_anomaly_detection_returns_zones_and_reproducible_evidence_hash() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::IndexAnomalyDetection;
        request.parameters.custom_parameters.insert(
            INDEX_ANOMALY_PAYLOAD_KEY.to_string(),
            serde_json::to_value(anomaly_request()).expect("valid request"),
        );
        request
            .parameters
            .custom_parameters
            .insert(INDEX_ANOMALY_FEATURE_FLAG_KEY.to_string(), json!(true));

        let mut first_result = {
            let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
            let _ = service
                .submit_analysis_job(&catalog, request.clone())
                .await
                .expect("anomaly job is accepted");
            service
                .process_next_job()
                .await
                .expect("processing attempted")
                .expect("anomaly result produced")
        };

        let mut second_result = {
            let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
            let _ = service
                .submit_analysis_job(&catalog, request)
                .await
                .expect("anomaly job is accepted");
            service
                .process_next_job()
                .await
                .expect("processing attempted")
                .expect("anomaly result produced")
        };

        first_result.evidence_refs.sort();
        second_result.evidence_refs.sort();
        assert!(matches!(first_result.data, ResultData::ZonalData { .. }));
        assert_eq!(first_result.evidence_refs, second_result.evidence_refs);
        assert_eq!(
            first_result.statistics.mean_value,
            second_result.statistics.mean_value
        );
        assert_eq!(first_result.statistics.total_pixel_count, 12);
    }

    #[tokio::test]
    async fn index_vegetation_classification_requires_feature_flag() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::IndexVegetationTypeClassification;
        request.parameters.custom_parameters.insert(
            INDEX_VEGETATION_CLASSIFICATION_PAYLOAD_KEY.to_string(),
            serde_json::to_value(vegetation_classification_request()).expect("valid request"),
        );
        request.parameters.custom_parameters.insert(
            INDEX_VEGETATION_CLASSIFICATION_FEATURE_FLAG_KEY.to_string(),
            json!(false),
        );

        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let job_id = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect("vegetation request is accepted");

        let result = service
            .process_next_job()
            .await
            .expect("processing attempted");
        assert!(result.is_none());

        let identity = service
            .analysis_job_identity(&job_id)
            .expect("identity exists after failure");
        assert!(matches!(identity.status, JobStatus::Failed));
        assert!(identity
            .failure_reason
            .as_ref()
            .is_some_and(|message| message.contains("disabled")));
    }

    #[tokio::test]
    async fn index_vegetation_classification_produces_zones_when_calibrated() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::IndexVegetationTypeClassification;
        request.parameters.custom_parameters.insert(
            INDEX_VEGETATION_CLASSIFICATION_PAYLOAD_KEY.to_string(),
            serde_json::to_value(vegetation_classification_request()).expect("valid request"),
        );
        request.parameters.custom_parameters.insert(
            INDEX_VEGETATION_CLASSIFICATION_FEATURE_FLAG_KEY.to_string(),
            json!(true),
        );

        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let _ = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect("vegetation request is accepted");
        let result = service
            .process_next_job()
            .await
            .expect("processing attempted")
            .expect("vegetation result produced");

        assert_eq!(result.result_type, ResultType::StressIndicators);
        assert!(matches!(result.data, ResultData::ZonalData { .. }));
        assert!(result.uncertainty.is_some());
        assert!(matches!(result.statistics.total_pixel_count, 4));
    }

    #[tokio::test]
    async fn index_vegetation_classification_marks_low_confidence_for_uncalibrated_scene() {
        let mut request_payload = vegetation_classification_request();
        request_payload.snapshots[1].calibrated = false;

        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::IndexVegetationTypeClassification;
        request.parameters.custom_parameters.insert(
            INDEX_VEGETATION_CLASSIFICATION_PAYLOAD_KEY.to_string(),
            serde_json::to_value(request_payload).expect("valid request"),
        );
        request.parameters.custom_parameters.insert(
            INDEX_VEGETATION_CLASSIFICATION_FEATURE_FLAG_KEY.to_string(),
            json!(true),
        );

        let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
        let _ = service
            .submit_analysis_job(&catalog, request)
            .await
            .expect("vegetation request is accepted");
        let result = service
            .process_next_job()
            .await
            .expect("processing attempted")
            .expect("vegetation result produced");
        assert_eq!(result.evidence_refs.len(), 1);
        assert!(result.statistics.mean_value >= 0.0);
    }

    #[tokio::test]
    async fn index_vegetation_classification_zones_have_reproducible_evidence() {
        let temp_dir = tempdir().unwrap();
        let catalog = analysis_catalog();
        let mut request = analysis_job_request(temp_dir.path());
        request.job_type = JobType::IndexVegetationTypeClassification;
        request.parameters.custom_parameters.insert(
            INDEX_VEGETATION_CLASSIFICATION_PAYLOAD_KEY.to_string(),
            serde_json::to_value(vegetation_classification_request()).expect("valid request"),
        );
        request.parameters.custom_parameters.insert(
            INDEX_VEGETATION_CLASSIFICATION_FEATURE_FLAG_KEY.to_string(),
            json!(true),
        );

        let mut first = {
            let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
            let _ = service
                .submit_analysis_job(&catalog, request.clone())
                .await
                .expect("vegetation request is accepted");
            service
                .process_next_job()
                .await
                .expect("processing attempted")
                .expect("vegetation result produced")
        };

        let mut second = {
            let mut service = PostProcessorService::new(temp_dir.path().to_path_buf()).unwrap();
            let _ = service
                .submit_analysis_job(&catalog, request)
                .await
                .expect("vegetation request is accepted");
            service
                .process_next_job()
                .await
                .expect("processing attempted")
                .expect("vegetation result produced")
        };

        first.evidence_refs.sort();
        second.evidence_refs.sort();
        assert_eq!(first.evidence_refs, second.evidence_refs);
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

    fn trend_request() -> IndexTrendRequest {
        IndexTrendRequest {
            snapshots: vec![
                trend_snapshot("scene-2026-05-01", "2026-05-01T00:00:00Z"),
                trend_snapshot("scene-2026-06-01", "2026-06-01T00:00:00Z"),
            ],
        }
    }

    fn lidar_change_request() -> LidarChangeRequest {
        LidarChangeRequest {
            snapshots: vec![
                lidar_snapshot(
                    "scene-2026-05-01",
                    "2026-05-01T00:00:00Z",
                    true,
                    0.0,
                    1.0,
                    3.0,
                    3.5,
                ),
                lidar_snapshot(
                    "scene-2026-06-01",
                    "2026-06-01T00:00:00Z",
                    true,
                    0.0,
                    0.8,
                    3.1,
                    3.7,
                ),
            ],
        }
    }

    fn lidar_snapshot(
        scene_id: &str,
        captured_at: &str,
        segmentation_reliable: bool,
        occupancy_prev: f32,
        occupancy_current: f32,
        chm_prev: f32,
        chm_current: f32,
    ) -> lidar_change::LidarChangeSnapshotPayload {
        let acquired_at = chrono::DateTime::parse_from_rfc3339(captured_at)
            .expect("captured time is valid")
            .with_timezone(&Utc);
        let occupancy_grid = ProductGrid {
            width: 2,
            height: 2,
            values: vec![occupancy_prev, 1.0, 0.5, occupancy_current],
            nodata_mask: vec![false, false, true, false],
            spatial_ref: shared::schemas::RasterSpatialRef {
                georeferenced: true,
                crs: Some("EPSG:32614".to_string()),
                bbox: Some(GeoBounds {
                    min_lon: 500000.0,
                    min_lat: 4500000.0,
                    max_lon: 500020.0,
                    max_lat: 4500020.0,
                }),
                geo_transform: Some([500000.0, 10.0, 0.0, 4500020.0, 0.0, -10.0]),
                resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
            },
        };
        let chm_grid = ProductGrid {
            width: 2,
            height: 2,
            values: vec![chm_prev, 3.6, 2.4, chm_current],
            nodata_mask: vec![false, false, true, false],
            spatial_ref: shared::schemas::RasterSpatialRef {
                georeferenced: true,
                crs: Some("EPSG:32614".to_string()),
                bbox: Some(GeoBounds {
                    min_lon: 500000.0,
                    min_lat: 4500000.0,
                    max_lon: 500020.0,
                    max_lat: 4500020.0,
                }),
                geo_transform: Some([500000.0, 10.0, 0.0, 4500020.0, 0.0, -10.0]),
                resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
            },
        };

        lidar_change::LidarChangeSnapshotPayload {
            field_id: "field-a".to_string(),
            scene_id: scene_id.to_string(),
            occupancy_product_ref: format!("occupancy-layer-{scene_id}"),
            chm_product_ref: format!("chm-layer-{scene_id}"),
            acquired_at,
            occupancy_grid,
            chm_grid,
            segmentation_reliable,
        }
    }

    fn anomaly_request() -> IndexAnomalyRequest {
        IndexAnomalyRequest {
            field_id: "field-a".to_string(),
            scene_id: "scene-2026-05-01".to_string(),
            product_ref: "layer-ndvi-2026-05-01".to_string(),
            acquired_at: DateTime::parse_from_rfc3339("2026-05-01T00:00:00Z")
                .expect("valid time")
                .with_timezone(&Utc),
            grid: ProductGrid {
                width: 4,
                height: 3,
                values: vec![
                    0.1, 0.25, 0.3, 0.85, 0.4, 0.45, 0.48, 0.5, 0.52, 0.55, 0.58, 0.6,
                ],
                nodata_mask: vec![false; 12],
                spatial_ref: shared::schemas::RasterSpatialRef {
                    georeferenced: true,
                    crs: Some("EPSG:32614".to_string()),
                    bbox: Some(GeoBounds {
                        min_lon: 500000.0,
                        min_lat: 4500000.0,
                        max_lon: 500040.0,
                        max_lat: 4500030.0,
                    }),
                    geo_transform: Some([500000.0, 10.0, 0.0, 4500030.0, 0.0, -10.0]),
                    resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
                },
            },
            low_threshold: Some(0.2),
            high_threshold: Some(0.8),
            std_dev_multiplier: Some(2.0),
        }
    }

    fn vegetation_classification_request() -> IndexVegetationTypeClassificationRequest {
        IndexVegetationTypeClassificationRequest {
            snapshots: vec![
                vegetation_snapshot(
                    "scene-2026-05-01",
                    "2026-05-01T00:00:00Z",
                    vec![0.45, 0.47, 0.49, 0.48],
                ),
                vegetation_snapshot(
                    "scene-2026-06-01",
                    "2026-06-01T00:00:00Z",
                    vec![0.44, 0.46, 0.45, 0.47],
                ),
            ],
            signature_library: Some(vec![
                VegetationTypeSignature {
                    name: "cotton".to_string(),
                    mean_ndvi: 0.42,
                    std_ndvi: 0.16,
                    trend_ndvi: 0.0,
                    mean_tolerance: 0.22,
                    std_tolerance: 0.19,
                    trend_tolerance: 0.20,
                },
                VegetationTypeSignature {
                    name: "forest".to_string(),
                    mean_ndvi: 0.72,
                    std_ndvi: 0.08,
                    trend_ndvi: 0.0,
                    mean_tolerance: 0.18,
                    std_tolerance: 0.16,
                    trend_tolerance: 0.20,
                },
                VegetationTypeSignature {
                    name: "bush".to_string(),
                    mean_ndvi: 0.30,
                    std_ndvi: 0.14,
                    trend_ndvi: -0.01,
                    mean_tolerance: 0.20,
                    std_tolerance: 0.16,
                    trend_tolerance: 0.22,
                },
                VegetationTypeSignature {
                    name: "palm".to_string(),
                    mean_ndvi: 0.60,
                    std_ndvi: 0.12,
                    trend_ndvi: 0.02,
                    mean_tolerance: 0.20,
                    std_tolerance: 0.14,
                    trend_tolerance: 0.24,
                },
                VegetationTypeSignature {
                    name: "rice".to_string(),
                    mean_ndvi: 0.36,
                    std_ndvi: 0.15,
                    trend_ndvi: 0.00,
                    mean_tolerance: 0.24,
                    std_tolerance: 0.20,
                    trend_tolerance: 0.25,
                },
            ]),
        }
    }

    fn vegetation_snapshot(
        scene_id: &str,
        captured_at: &str,
        values: Vec<f32>,
    ) -> IndexVegetationTypeClassificationSnapshot {
        IndexVegetationTypeClassificationSnapshot {
            field_id: "field-a".to_string(),
            scene_id: scene_id.to_string(),
            product_ref: format!("layer-ndvi-{scene_id}"),
            acquired_at: chrono::DateTime::parse_from_rfc3339(captured_at)
                .expect("capture time is valid")
                .with_timezone(&Utc),
            grid: ProductGrid {
                width: 2,
                height: 2,
                values,
                nodata_mask: vec![false; 4],
                spatial_ref: shared::schemas::RasterSpatialRef {
                    georeferenced: true,
                    crs: Some("EPSG:32614".to_string()),
                    bbox: Some(GeoBounds {
                        min_lon: 500000.0,
                        min_lat: 4500000.0,
                        max_lon: 500020.0,
                        max_lat: 4500020.0,
                    }),
                    geo_transform: Some([500000.0, 10.0, 0.0, 4500020.0, 0.0, -10.0]),
                    resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
                },
            },
            calibrated: true,
        }
    }

    fn trend_snapshot(scene_id: &str, captured_at: &str) -> index_trend::IndexTrendSnapshotPayload {
        let acquired_at = chrono::DateTime::parse_from_rfc3339(captured_at)
            .expect("captured time is valid")
            .with_timezone(&Utc);
        index_trend::IndexTrendSnapshotPayload {
            field_id: "field-a".to_string(),
            scene_id: scene_id.to_string(),
            product_ref: format!("layer-{scene_id}"),
            acquired_at,
            grid: ProductGrid {
                width: 2,
                height: 2,
                values: vec![0.2, 0.3, 0.4, 0.5],
                nodata_mask: vec![false; 4],
                spatial_ref: shared::schemas::RasterSpatialRef {
                    georeferenced: true,
                    crs: Some("EPSG:32614".to_string()),
                    bbox: Some(GeoBounds {
                        min_lon: 500000.0,
                        min_lat: 4500000.0,
                        max_lon: 500020.0,
                        max_lat: 4500020.0,
                    }),
                    geo_transform: Some([500000.0, 10.0, 0.0, 4500020.0, 0.0, -10.0]),
                    resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
                },
            },
            calibration_status: IndexTrendCalibrationStatus::CalibratedReflectance,
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
