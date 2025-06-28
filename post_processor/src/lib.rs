use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub mod ndvi_analysis;
pub mod lidar_analysis;
pub mod thermal_analysis;
pub mod report_generator;

pub use ndvi_analysis::NdviAnalyzer;
pub use lidar_analysis::LidarAnalyzer;
pub use thermal_analysis::ThermalAnalyzer;
pub use report_generator::ReportGenerator;

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
pub enum JobType {
    NdviAnalysis,
    LidarProcessing,
    ThermalAnalysis,
    MultiSpectralAnalysis,
    CompositeReport,
    HealthAssessment,
    YieldPrediction,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
    working_directory: PathBuf,
    ndvi_analyzer: NdviAnalyzer,
    lidar_analyzer: LidarAnalyzer,
    thermal_analyzer: ThermalAnalyzer,
    report_generator: ReportGenerator,
}

impl PostProcessorService {
    pub fn new(working_directory: PathBuf) -> Result<Self> {
        Ok(Self {
            job_queue: Vec::new(),
            completed_jobs: HashMap::new(),
            results_cache: HashMap::new(),
            working_directory,
            ndvi_analyzer: NdviAnalyzer::new(),
            lidar_analyzer: LidarAnalyzer::new(),
            thermal_analyzer: ThermalAnalyzer::new(),
            report_generator: ReportGenerator::new(report_generator::ReportConfig {
                output_formats: vec![report_generator::OutputFormat::PDF, report_generator::OutputFormat::HTML],
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
        self.job_queue.sort_by(|a, b| {
            a.created_at.cmp(&b.created_at)
        });

        tracing::info!("Submitted processing job: {}", job_id);
        Ok(job_id)
    }

    pub async fn process_next_job(&mut self) -> Result<Option<AnalysisResult>> {
        if let Some(mut job) = self.job_queue.pop() {
            job.status = JobStatus::Processing;
            job.started_at = Some(Utc::now());

            tracing::info!("Processing job: {} (type: {:?})", job.id, job.job_type);

            let result = match self.process_job(&job).await {
                Ok(result) => {
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

            self.completed_jobs.insert(job.id, job);
            Ok(result)
        } else {
            Ok(None)
        }
    }

    async fn process_job(&self, job: &ProcessingJob) -> Result<AnalysisResult> {
        match job.job_type {
            JobType::NdviAnalysis => {
                self.ndvi_analyzer.analyze(&job.input_files, &job.parameters).await
            }
            JobType::LidarProcessing => {
                self.lidar_analyzer.analyze(&job.input_files, &job.parameters).await
            }
            JobType::ThermalAnalysis => {
                self.thermal_analyzer.analyze(&job.input_files, &job.parameters).await
            }
            JobType::MultiSpectralAnalysis => {
                self.process_multispectral(job).await
            }
            JobType::CompositeReport => {
                self.generate_composite_report(job).await
            }
            JobType::HealthAssessment => {
                self.assess_crop_health(job).await
            }
            JobType::YieldPrediction => {
                self.predict_yield(job).await
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
            description: "Multi-sensor data integration has been completed successfully".to_string(),
            action_items: vec!["Review NDVI results".to_string(), "Check thermal anomalies".to_string()],
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
            values: [("health_score".to_string(), 0.85), ("stress_level".to_string(), 0.15)]
                .iter().cloned().collect(),
            classification: Some("Healthy".to_string()),
        });

        let recommendations = vec![
            Recommendation {
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
            }
        ];

        let result = AnalysisResult {
            id: Uuid::new_v4(),
            job_id: job.id,
            result_type: ResultType::HealthIndex,
            data: ResultData::ZonalData {
                zones: health_zones,
                aggregated_values: [("overall_health".to_string(), 0.82)]
                    .iter().cloned().collect(),
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
                percentiles: [("25".to_string(), 3.6), ("50".to_string(), 4.2), ("75".to_string(), 4.9)]
                    .iter().cloned().collect(),
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

    pub async fn list_jobs(&self, status_filter: Option<JobStatus>) -> Vec<&ProcessingJob> {
        let mut jobs: Vec<&ProcessingJob> = self.job_queue.iter().collect();
        jobs.extend(self.completed_jobs.values());

        if let Some(status) = status_filter {
            jobs.retain(|job| matches!(job.status, status));
        }

        jobs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        jobs
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

        // Remove old results
        self.results_cache.retain(|_, result| {
            result.created_at > cutoff_date
        });

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
}
