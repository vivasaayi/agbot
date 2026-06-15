use crate::{
    error::{AppError, AppResult},
    ingest, landsat,
    product_catalog::{publish_georeferenced_product, ProductPublishError},
    shapefile,
    state::{AppState, SceneSearchCacheKey},
};
use alerting::{
    build_alert_rule_record, build_alert_rule_subscription, normalize_fired_alert_record,
    transition_alert_rule_status, version_alert_rule_record, AlertHistoryPage,
    AlertRuleAuditRecord, AlertRuleCreateRequest, AlertRuleRecord, AlertRuleStatus,
    AlertRuleStatusUpdateRequest, AlertRuleSubscriptionCreateRequest, AlertRuleSubscriptionRecord,
    AlertRuleUpdateRequest, AlertSeverityHint, AlertingError, FiredAlertRecord,
};
use anyhow::Error;
use axum::response::Html;
use axum::response::{IntoResponse, Response};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    Json,
};
use compliance::{
    airspace_zone_contains_point, airspace_zone_is_effective_at, append_compliance_record_version,
    build_airspace_zone_record, build_compliance_audit_report, build_initial_compliance_record,
    refuse_in_place_mutation, AirspaceCoordinate, AirspaceZoneClass, AirspaceZoneError,
    AirspaceZoneIngestRequest, AirspaceZoneRecord, AppendComplianceRecordVersionRequest,
    ComplianceAuditReport, ComplianceAuditReportError, ComplianceAuditReportRequest,
    ComplianceRecord, ComplianceRecordError, ComplianceRecordPayload, ComplianceRecordType,
    CreateComplianceRecordRequest,
};
use copilot::{
    create_copilot_turn, start_copilot_conversation, CopilotConversationError,
    CopilotConversationRecord, CopilotConversationStartRequest, CopilotTurnCreateRequest,
    CopilotTurnRecord, CopilotTurnRole,
};
use crop_intelligence::{
    apply_detection_verification, assemble_detection_finding, build_inference_run_record,
    build_model_version_record, transition_inference_run_status,
    validate_detection_finding_promotion, validate_model_reference, CropDetectionCorrectionLabel,
    CropDetectionFindingError, CropDetectionFindingRecord, CropDetectionFindingRequest,
    CropDetectionVerificationAction, CropDetectionVerificationError,
    CropDetectionVerificationRecord, CropDetectionVerificationRequest, CropModelRegistryError,
    CropModelTask, DetectionVerificationState, DetectionZoneGeometry, FindingPromotionDecision,
    FindingPromotionError, FindingPromotionRequest, InferenceModelReference, InferenceRunError,
    InferenceRunRecord, InferenceRunStatus, InferenceRunSubmissionRequest, ModelGateResponse,
    ModelVersionRecord, ModelVersionRegistrationRequest,
};
use fleet_health::{
    accrue_component_duty, apply_rollout_control, build_component_duty_accruals,
    build_component_record, component_event, derive_health_indicators, evaluate_ota_rollout,
    install_component, ComponentDutyAccrualRecord, DutyAccrualRequest, FleetComponentEventRecord,
    FleetComponentRecord, FleetComponentType, FleetHealthError, FleetHealthIndicator,
    FleetHealthIndicatorDerivation, FleetHealthIndicatorSample, HealthIndicatorFreshness,
    HealthTelemetryGap, InstallComponentRequest, OtaRolloutDecision, OtaRolloutRequest,
    RegisterComponentRequest, RolloutControlDecision, RolloutControlRequest, ServiceHistoryEntry,
    TelemetryHealthIndicatorRequest,
};
use geojson::{
    feature::Id as GeoJsonId, Feature, FeatureCollection, GeoJson, Geometry, Value as GeoJsonValue,
};
use image::{imageops::FilterType, DynamicImage, GrayImage, ImageBuffer, ImageFormat, Rgb};
use interop::{export_raster_geotiff, RasterProduct};
use orthomosaic::{
    build_frame_set_record, build_reconstruction_job, build_tiled_output_handoff,
    evaluate_mosaic_publish_gate, transition_reconstruction_status, FramePoseRecord,
    FrameSetIngestError, FrameSetIngestRequest, FrameSetRecord, MosaicPublishGateDecision,
    MosaicPublishGateError, MosaicPublishGateRequest, ReconstructionJobError,
    ReconstructionJobRecord, ReconstructionJobRequest, ReconstructionStatus, TiledOutputHandoff,
    TiledOutputHandoffError, TiledOutputHandoffRequest,
};
use plugin_sdk::{
    PluginExecutionLimits, PluginExecutionPlan, PluginHost, PluginLifecycleAuditRecord,
    PluginLifecycleError, PluginLifecycleStatus, PluginLifecycleTransitionRequest,
    PluginRegistrationError, PluginRegistrationRecord, RawPluginManifest, SandboxExecutionOutcome,
    SandboxExecutionStatus, SandboxTerminationReason,
};
use provenance::{
    ActorIdentity, ActorKind, ArtifactKind, AuditAction, AuditEntry, AuditLedger,
    AuditRefusalReason, BackwardProvenanceTrace, LineageLedger, LineageRecord,
    ProvenanceParameters,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use shared::plugin_extensions::ExtensionPointKind;
use shared::schemas::{
    append_content_version, assert_raster_spatial_ref, bind_fleet_node_identity,
    bounds_coverage_fraction, bounds_from_points, build_collaboration_channel,
    build_collaboration_message, build_marketplace_account_record,
    build_marketplace_catalog_item_record, build_marketplace_inventory_record,
    build_marketplace_portal_entry, build_soil_moisture_reading, build_sustainability_record,
    build_tractor_record, close_marketplace_listing_record, compute_drought_index,
    create_versioned_content, fulfill_marketplace_inventory, normalize_weather_provider_forecast,
    parse_content_status, parse_content_type, parse_drought_index_type,
    parse_marketplace_account_status, parse_marketplace_catalog_category,
    parse_marketplace_catalog_item_kind, parse_marketplace_listing_status,
    parse_marketplace_order_status, parse_marketplace_party_type,
    parse_marketplace_unit_of_measure, parse_soil_moisture_qa_flag,
    parse_soil_moisture_rejection_reason, parse_sustainability_metric_type,
    place_marketplace_order_record, prepare_open_data_publication,
    publish_marketplace_listing_record, release_marketplace_inventory,
    reserve_marketplace_inventory, soil_moisture_rejection_reason_for_error,
    soil_moisture_rejection_record, transition_marketplace_account_status,
    transition_marketplace_order_status, validate_field_boundary, weather_fetch_failure_record,
    AnnotationGeometry, AnnotationRecord, CollaborationChannelCreateRequest,
    CollaborationChannelRecord, CollaborationChannelThread, CollaborationError,
    CollaborationMessageCreateRequest, CollaborationMessageRecord, ContentCreateRequest,
    ContentEditRequest, ContentError, ContentRecord, ContentStatus, ContentType,
    ContentVersionRecord, DroughtIndexComputeRequest, DroughtIndexError, DroughtIndexPeriod,
    DroughtIndexRecord, DroughtIndexType, FarmFieldEntityStatus, FarmFieldListPage,
    FarmFieldListQuery, FarmRecord, FieldBoundary, FieldBoundaryRecord, FieldRecord,
    FleetNodeEnrollmentError, FleetNodeEnrollmentRequest, FleetNodeKind, FleetNodeRecord,
    FleetNodeRuntimeMode, FleetNodeStatus, GeoBounds, GeoPoint, GpsCoords, ImageMetadata,
    MarketplaceAccountCreateRequest, MarketplaceAccountError, MarketplaceAccountRecord,
    MarketplaceAccountStatus, MarketplaceCatalogCategory, MarketplaceCatalogError,
    MarketplaceCatalogItemCreateRequest, MarketplaceCatalogItemKind, MarketplaceCatalogItemRecord,
    MarketplaceInventoryError, MarketplaceInventoryRecord, MarketplaceInventoryUpsertRequest,
    MarketplaceListingError, MarketplaceListingPublishRequest, MarketplaceListingRecord,
    MarketplaceListingStatus, MarketplaceOrderAuditRecord, MarketplaceOrderCreateRequest,
    MarketplaceOrderError, MarketplaceOrderRecord, MarketplaceOrderStatus, MarketplacePartyType,
    MarketplacePortalEntry, MarketplacePortalEntryError, MultispectralImage, OpenDataPublication,
    OpenDataPublishError, OpenDataPublishRequest, RasterResolution, RasterSpatialRef,
    RecommendationPriority, RecommendationRecord, RecommendationStatus, ReportFormat, ReportRecord,
    ReportVisibility, SoilMoistureReadingError, SoilMoistureReadingRecord,
    SoilMoistureReadingRequest, SoilMoistureRejectionReason, SoilMoistureRejectionRecord,
    SustainabilityMetricType, SustainabilityRecord, SustainabilityRecordCreateRequest,
    SustainabilityRecordError, SustainabilityRecordLinkage, TractorCommandAuditDecision,
    TractorCommandAuditRecord, TractorCommandRejection, TractorCommandRejectionReason,
    TractorImplementRef, TractorLifecycleStatus, TractorMotionCommandRequest, TractorRecord,
    TractorRegistrationRequest, TractorRegistryError, VersionedContentRecord,
    WeatherFetchFailureRecord, WeatherForecastRecord, WeatherForecastVariables, WeatherIngestError,
    WeatherProviderForecastPoint, WeatherProviderForecastResponse, DEFAULT_RECORD_OWNER,
    GEO_EXTENT_ASSERTION_TOLERANCE,
};
use soil_iot::{
    build_geolocated_soil_reading, build_soil_config_push_record, build_soil_device_record,
    transition_soil_config_push_status, GatewayIngestError, GatewayReadingRecord, GeoPosition,
    GeolocatedSoilReading, RegisterSoilDeviceRequest, SoilDeviceConfigPushRecord,
    SoilDeviceConfigPushRequest, SoilDeviceConfigPushStatus, SoilDeviceConfigPushStatusUpdate,
    SoilDeviceRecord, SoilDeviceStatus, SoilIotError, SoilSensorType,
};
use sqlx::Row;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io::Cursor;
use std::io::ErrorKind;
use std::path::{Path as FsPath, PathBuf};
use std::time::SystemTime;
use timeseries::{SeriesPoint, SeriesValue};
use tokio::fs::File;
use tokio::fs::{self, DirEntry};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

const TILE_SIZE: u32 = 256;
const DEFAULT_LAYER_STALE_AFTER_DAYS: i64 = 14;
const MOBILE_APP_HTML: &str = include_str!("mobile_app.html");

#[derive(Debug, Serialize)]
pub struct SceneSummary {
    pub scene_id: String,
    pub owner: String,
    pub sensor: String,
    pub acquired_at: String,
    pub created_at: String,
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub linked_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SceneRefreshAdvisory {
    pub current_scene_id: String,
    pub candidate_scene_id: String,
    pub current_acquired_at: String,
    pub candidate_acquired_at: String,
    pub current_cloud_cover: Option<f64>,
    pub candidate_cloud_cover: Option<f64>,
    pub uncertainty: bool,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct SceneRefreshAdvisoriesResponse {
    pub advisory_enabled: bool,
    pub reason: Option<String>,
    pub advisories: Vec<SceneRefreshAdvisory>,
}

#[derive(Debug, Serialize)]
pub struct SceneChangeAdvisory {
    pub baseline_scene_id: String,
    pub comparison_scene_id: String,
    pub baseline_acquired_at: String,
    pub comparison_acquired_at: String,
    pub common_extent: Option<SceneExtent>,
    pub coverage_fraction: f64,
    pub change_score: f64,
    pub uncertainty_low: f64,
    pub uncertainty_high: f64,
    pub confidence: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct SceneChangeAdvisoriesResponse {
    pub advisory_enabled: bool,
    pub reason: Option<String>,
    pub advisories: Vec<SceneChangeAdvisory>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SceneDetail {
    pub scene_id: String,
    pub owner: Option<String>,
    pub sensor: Option<String>,
    pub acquired_at: Option<String>,
    pub created_at: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bands: Vec<String>,
    pub gps_position: Option<GpsCoords>,
    pub data_path: Option<String>,
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub linked_at: Option<String>,
    pub field: Option<FieldRecord>,
    pub ingest: Option<ingest::SceneIngestRecord>,
    pub geospatial: SceneGeospatialMetadata,
    pub available_products: Vec<ProductSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductSummary {
    pub product_id: Option<String>,
    pub kind: String,
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub filename: String,
    pub content_type: String,
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
    pub gsd_m_per_px: Option<f64>,
    pub spatial_ref: Option<RasterSpatialRef>,
    pub source_image_ids: Vec<String>,
    pub source_scan_ids: Vec<String>,
    pub publish_status: Option<String>,
    pub qa_report_ref: Option<String>,
    pub provenance_hash: Option<String>,
    pub downstream_consumers: Vec<String>,
    pub url_path: String,
    pub tile_url_template: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LayerListQuery {
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub product_kind: Option<String>,
    pub date: Option<String>,
    pub stale_after_days: Option<i64>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayerListResponse {
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub layers: Vec<LayerMetadata>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayerMetadata {
    pub layer_id: String,
    pub product_id: Option<String>,
    pub scene_id: String,
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub product_kind: String,
    pub dataset: String,
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
    pub gsd_m_per_px: Option<f64>,
    pub spatial_ref: RasterSpatialRef,
    pub source_image_ids: Vec<String>,
    pub source_scan_ids: Vec<String>,
    pub publish_status: Option<String>,
    pub qa_report_ref: Option<String>,
    pub provenance_hash: Option<String>,
    pub downstream_consumers: Vec<String>,
    pub freshness: LayerFreshness,
    pub source: String,
    pub url_path: String,
    pub tile_url_template: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayerFreshness {
    pub acquired_at: String,
    pub ingested_at: Option<String>,
    pub coverage_fraction: Option<f64>,
    pub stale_after_days: i64,
    pub age_days: Option<i64>,
    pub stale: bool,
    pub field_coverage_fraction: Option<f64>,
    pub field_coverage_status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenDataLayerPublishRequest {
    pub license: String,
    pub attribution: String,
    #[serde(default)]
    pub owner_identifier: Option<String>,
    #[serde(default)]
    pub field_identifier: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenDataCatalogResponse {
    pub layers: Vec<OpenDataLayerCatalogEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenDataLayerCatalogEntry {
    pub open_data_id: String,
    pub product_kind: String,
    pub license: String,
    pub attribution: String,
    pub anonymized: bool,
    pub spatial_ref: RasterSpatialRef,
    pub url_path: String,
    pub tile_url_template: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SceneAuditTrail {
    pub scene_id: String,
    pub ingest_attempts: Vec<ingest::SceneIngestAttemptRecord>,
    pub link_audits: Vec<SceneLinkAuditRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SceneLinkAuditRecord {
    pub audit_id: String,
    pub scene_id: String,
    pub mutation: String,
    pub previous_field_id: Option<String>,
    pub previous_season_id: Option<String>,
    pub new_field_id: String,
    pub new_season_id: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SceneGeospatialMetadata {
    pub georeferenced: bool,
    pub crs: Option<String>,
    pub center: Option<GpsCoords>,
    pub extent: Option<SceneExtent>,
    pub spatial_ref: Option<RasterSpatialRef>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SceneExtent {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

#[derive(Debug, Deserialize)]
pub struct CreateFieldRequest {
    pub farm_id: Option<String>,
    pub field_id: Option<String>,
    pub org_id: Option<String>,
    pub owner: Option<String>,
    pub name: String,
    pub crop: Option<String>,
    pub season: Option<String>,
    pub notes: Option<String>,
    pub status: Option<FarmFieldEntityStatus>,
    pub boundary: FieldBoundary,
}

#[derive(Debug, Deserialize)]
pub struct CreateFarmRequest {
    pub farm_id: Option<String>,
    pub org_id: Option<String>,
    pub owner: Option<String>,
    pub name: String,
    pub notes: Option<String>,
    pub status: Option<FarmFieldEntityStatus>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFarmRequest {
    pub name: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FarmFieldApiListQuery {
    pub org_id: Option<String>,
    pub owner: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    pub status: Option<FarmFieldEntityStatus>,
}

impl FarmFieldApiListQuery {
    fn org_filter(&self) -> Option<String> {
        normalize_optional_text(self.org_id.clone())
            .or_else(|| normalize_optional_text(self.owner.clone()))
    }

    fn list_query(&self) -> FarmFieldListQuery {
        FarmFieldListQuery {
            page: self.page,
            page_size: self.page_size,
            status: self.status,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateAnnotationRequest {
    pub annotation_id: Option<String>,
    pub field_id: Option<String>,
    pub author: Option<String>,
    pub crs: Option<String>,
    pub audit_id: Option<String>,
    pub label: String,
    pub note: Option<String>,
    pub severity: Option<String>,
    pub geometry: AnnotationGeometry,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAnnotationRequest {
    pub author: Option<String>,
    pub crs: Option<String>,
    pub audit_id: Option<String>,
    pub label: String,
    pub note: Option<String>,
    pub severity: Option<String>,
    pub geometry: AnnotationGeometry,
}

#[derive(Debug, Deserialize)]
pub struct CreateRecommendationRequest {
    pub recommendation_id: Option<String>,
    pub author_user_id: Option<String>,
    pub title: String,
    pub note: Option<String>,
    pub category: Option<String>,
    pub action_category: Option<String>,
    pub priority: Option<RecommendationPriority>,
    pub status: Option<RecommendationStatus>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub annotation_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRecommendationRequest {
    pub title: String,
    pub note: Option<String>,
    pub category: Option<String>,
    pub action_category: Option<String>,
    pub priority: RecommendationPriority,
    pub status: RecommendationStatus,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub annotation_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReportRequest {
    pub title: Option<String>,
    #[serde(default)]
    pub visibility: ReportVisibility,
}

#[derive(Debug, Deserialize)]
pub struct CreateReportShareRequest {
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportShareResponse {
    pub share_token: String,
    pub report_id: String,
    pub scene_id: String,
    pub url_path: String,
    pub expires_at: String,
    pub revoked_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
struct ReportShareRecord {
    share_token: String,
    report_id: String,
    scene_id: String,
    expires_at: String,
    revoked_at: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone)]
struct SharedReportRecord {
    share: ReportShareRecord,
    report: ReportRecord,
}

#[derive(Debug, Deserialize)]
pub struct ImportShapefileRequest {
    pub path: String,
    pub crs: Option<String>,
    pub name_prefix: Option<String>,
    pub farm_id: Option<String>,
    pub owner: Option<String>,
    pub crop: Option<String>,
    pub season: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FleetNodeListQuery {
    pub owner_org_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TractorListQuery {
    pub org_id: Option<String>,
    pub field_id: Option<String>,
    pub status: Option<TractorLifecycleStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TractorMotionCommandValidationRequest {
    pub command_id: Option<String>,
    pub command_type: String,
    pub requested_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PullWeatherForecastRequest {
    pub field_id: String,
    pub provider: String,
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default)]
    pub fetched_at: Option<String>,
    #[serde(default)]
    pub valid_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeatherForecastListQuery {
    pub field_id: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeatherFetchFailureListQuery {
    pub field_id: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FleetComponentListQuery {
    pub airframe_id: Option<String>,
    pub component_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FleetHealthIndicatorListQuery {
    pub component_id: Option<String>,
    pub indicator: Option<String>,
    pub freshness: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoilDeviceListQuery {
    pub org_id: Option<String>,
    pub field_id: Option<String>,
    pub zone_id: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoilMoistureReadingListQuery {
    pub field_id: Option<String>,
    pub zone_ref: Option<String>,
    pub source: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoilMoistureRejectionListQuery {
    pub field_id: Option<String>,
    pub reason: Option<SoilMoistureRejectionReason>,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DroughtIndexListQuery {
    pub field_or_region_ref: Option<String>,
    pub index_type: Option<DroughtIndexType>,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceAccountListQuery {
    pub org_id: Option<String>,
    pub party_type: Option<MarketplacePartyType>,
    pub status: Option<MarketplaceAccountStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceAccountScopeQuery {
    pub org_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceCatalogListQuery {
    pub org_id: Option<String>,
    pub kind: Option<MarketplaceCatalogItemKind>,
    pub category: Option<MarketplaceCatalogCategory>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceCatalogScopeQuery {
    pub org_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplacePortalEntryQuery {
    pub org_id: Option<String>,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceListingListQuery {
    pub org_id: Option<String>,
    pub status: Option<MarketplaceListingStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceListingScopeQuery {
    pub org_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceListingCloseRequest {
    pub org_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceInventoryListQuery {
    pub org_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceInventoryScopeQuery {
    pub org_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceInventoryAdjustmentRequest {
    pub org_id: String,
    pub qty: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceOrderListQuery {
    pub org_id: Option<String>,
    pub status: Option<MarketplaceOrderStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceOrderScopeQuery {
    pub org_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceOrderTransitionRequest {
    pub org_id: String,
    pub actor_id: String,
    pub status: MarketplaceOrderStatus,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceAccountStatusRequest {
    pub org_id: String,
    pub status: MarketplaceAccountStatus,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SustainabilityRecordListQuery {
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub metric_type: Option<SustainabilityMetricType>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SustainabilityRecordScopeQuery {
    pub field_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentItemListQuery {
    pub org_id: Option<String>,
    pub content_type: Option<ContentType>,
    pub status: Option<ContentStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentItemScopeQuery {
    pub org_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CollaborationChannelListQuery {
    pub org_id: Option<String>,
    pub field_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CollaborationScopeQuery {
    pub org_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimeSeriesPointListQuery {
    pub entity_ref: Option<String>,
    pub metric: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlertHistoryListQuery {
    pub source_domain: Option<String>,
    pub field_id: Option<String>,
    pub severity: Option<AlertSeverityHint>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlertRuleListQuery {
    pub status: Option<AlertRuleStatus>,
    pub event_type: Option<String>,
    pub include_versions: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProvenanceLineageListQuery {
    pub artifact_id: Option<String>,
    pub actor_id: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvenanceLineagePage {
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub records: Vec<LineageRecord>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProvenanceAuditListQuery {
    pub artifact_id: Option<String>,
    pub actor_id: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvenanceAuditPage {
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub entries: Vec<AuditEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginListQuery {
    pub kind: Option<ExtensionPointKind>,
    pub status: Option<PluginLifecycleStatus>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginRegistrationPage {
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub plugins: Vec<PluginRegistrationRecord>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginStatusUpdateRequest {
    pub status: PluginLifecycleStatus,
    pub actor_id: String,
    pub actor_kind: Option<ActorKind>,
    pub occurred_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginExecutionRequest {
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    pub estimated_runtime_ms: u64,
    pub estimated_memory_mb: u64,
    pub result: Option<String>,
    pub limits: Option<PluginExecutionLimits>,
    pub attempted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeSeriesPointResponse {
    pub entity_ref: String,
    pub metric: String,
    pub t: String,
    pub value: SeriesValue,
    pub source_ref: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrthomosaicFrameSetListQuery {
    pub scene_id: Option<String>,
    pub field_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateReconstructionStatusRequest {
    pub status: ReconstructionStatus,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CropModelListQuery {
    pub task: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CopilotConversationListQuery {
    pub field_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCropInferenceRunStatusRequest {
    pub status: InferenceRunStatus,
    #[serde(default)]
    pub failure_reason_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VerifyCropDetectionRequest {
    pub task: CropModelTask,
    pub label: String,
    pub confidence: f64,
    #[serde(default)]
    pub evidence_tile_refs: Vec<String>,
    pub zone_geometry: DetectionZoneGeometry,
    pub action: CropDetectionVerificationAction,
    pub actor: String,
    pub verified_at: String,
    #[serde(default)]
    pub corrected_label: Option<String>,
    #[serde(default)]
    pub corrected_geometry: Option<DetectionZoneGeometry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CropFindingPromotionValidationRequest {
    #[serde(default)]
    pub allow_unverified: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmitCropDetectionFindingRequest {
    pub finding_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub model_id: String,
    pub version: String,
    pub emitted_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComplianceRecordListQuery {
    pub record_id: Option<String>,
    pub record_type: Option<String>,
    pub org_id: Option<String>,
    pub field_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComplianceAuditReportExportRequest {
    #[serde(default)]
    pub report_id: Option<String>,
    #[serde(default)]
    pub org_id: String,
    #[serde(default)]
    pub field_id: String,
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub mandatory_record_types: Vec<ComplianceRecordType>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AirspaceZoneListQuery {
    pub zone_id: Option<String>,
    pub zone_class: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AirspaceZonePointQuery {
    pub longitude: f64,
    pub latitude: f64,
    pub at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldSeasonGroup {
    pub season: Option<String>,
    pub fields: Vec<FieldRecord>,
}

#[derive(Debug, Deserialize)]
pub struct MobileAnalyzeRequest {
    pub latitude: f64,
    pub longitude: f64,
    pub date: Option<String>,
    pub days: Option<u8>,
    pub products: Option<Vec<String>>,
    pub source: Option<String>,
    pub external_scene_id: Option<String>,
    pub selected_scene: Option<MobileSceneCandidate>,
    pub field_geometry: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct MobileSceneSearchRequest {
    pub latitude: f64,
    pub longitude: f64,
    pub date: Option<String>,
    pub days: Option<u8>,
    pub source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MobileSceneSearchResponse {
    pub scenes: Vec<MobileSceneCandidate>,
    pub search_days: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MobileSceneCandidate {
    pub external_scene_id: String,
    pub dataset: String,
    pub dataset_label: String,
    pub provider: String,
    pub collection: String,
    pub acquired_at: String,
    pub cloud_cover: Option<f64>,
    pub bbox: Option<GeoBounds>,
    pub resolution_m: f64,
    pub asset_count: usize,
}

#[derive(Debug, Serialize)]
pub struct MobileAnalyzeResponse {
    pub scene_id: String,
    pub external_scene_id: Option<String>,
    pub sensor: String,
    pub acquired_at: String,
    pub source: String,
    pub dataset: Option<String>,
    pub dataset_label: Option<String>,
    pub provider: Option<String>,
    pub collection: Option<String>,
    pub cloud_cover: Option<f64>,
    pub resolution_m: Option<f64>,
    pub asset_count: usize,
    pub search_days: u8,
    pub real_products_ready: bool,
    pub location: GpsCoords,
    pub extent: SceneExtent,
    pub products: Vec<MobileProduct>,
}

#[derive(Debug, Serialize)]
pub struct MobileProduct {
    pub kind: String,
    pub label: String,
    pub url_path: String,
    pub tile_url_template: String,
    pub stats: Option<serde_json::Value>,
}

pub async fn mobile_app() -> Html<&'static str> {
    Html(MOBILE_APP_HTML)
}

pub async fn get_ingest_health(
    State(state): State<AppState>,
) -> AppResult<Json<ingest::SceneIngestHealth>> {
    Ok(Json(ingest::load_ingest_health(&state.pool).await?))
}

pub async fn mobile_search_scenes(
    State(state): State<AppState>,
    Json(request): Json<MobileSceneSearchRequest>,
) -> AppResult<Json<MobileSceneSearchResponse>> {
    validate_lat_lon(request.latitude, request.longitude)?;

    let target_date = request
        .date
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| chrono::Utc::now().date_naive().to_string());
    let requested_days = request.days.unwrap_or(14).clamp(1, 30);
    let source_mode = normalize_source_mode(request.source.as_deref());
    let mut search_days = requested_days;
    let mut candidates = Vec::new();
    if source_mode == "sample" {
        return Ok(Json(MobileSceneSearchResponse {
            scenes: Vec::new(),
            search_days,
        }));
    }

    for window_days in expanded_landsat_windows(requested_days) {
        search_days = window_days;
        let cache_key = SceneSearchCacheKey::new(
            &source_mode,
            request.latitude,
            request.longitude,
            &target_date,
            window_days,
            5,
        );
        if let Some(found) = state.scene_search_cache.get(&cache_key) {
            if !found.is_empty() {
                candidates = found;
                break;
            }
            continue;
        }
        match landsat::search_scenes_for_source(
            &source_mode,
            request.latitude,
            request.longitude,
            &target_date,
            window_days,
            5,
        )
        .await
        {
            Ok(found) if !found.is_empty() => {
                state.scene_search_cache.store(cache_key, found.clone());
                candidates = found;
                break;
            }
            Ok(found) => {
                state.scene_search_cache.store(cache_key, found);
                continue;
            }
            Err(err) => {
                tracing::warn!(error = %err, "real satellite scene search failed");
                return Err(AppError::Anyhow(err));
            }
        }
    }

    Ok(Json(MobileSceneSearchResponse {
        scenes: candidates.into_iter().map(mobile_scene_candidate).collect(),
        search_days,
    }))
}

pub async fn mobile_analyze(
    State(state): State<AppState>,
    Json(request): Json<MobileAnalyzeRequest>,
) -> AppResult<Json<MobileAnalyzeResponse>> {
    validate_lat_lon(request.latitude, request.longitude)?;

    let products = normalize_mobile_products(request.products);
    let acquired_at = request
        .date
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| chrono::Utc::now().date_naive().to_string());
    let requested_days = request.days.unwrap_or(14).clamp(1, 30);
    let source_mode = request.source.as_deref();
    let source_mode = normalize_source_mode(source_mode);
    let field_geometry = normalize_field_geometry(request.field_geometry.as_ref())?;
    let mut search_days = requested_days;
    let selected_candidate = request
        .selected_scene
        .as_ref()
        .map(candidate_from_mobile_scene);
    if let (Some(selected_id), Some(candidate)) = (
        request.external_scene_id.as_deref(),
        selected_candidate.as_ref(),
    ) {
        if selected_id != candidate.item_id {
            return Err(AppError::BadRequest(
                "selected scene payload does not match selected scene id".to_string(),
            ));
        }
    }
    let landsat_candidate = if source_mode == "sample" {
        None
    } else if selected_candidate.is_some() {
        selected_candidate
    } else {
        let mut found = None;
        for window_days in expanded_landsat_windows(requested_days) {
            search_days = window_days;
            match landsat::search_best_scene_for_source(
                &source_mode,
                request.latitude,
                request.longitude,
                &acquired_at,
                window_days,
            )
            .await
            {
                Ok(Some(candidate)) => {
                    if request
                        .external_scene_id
                        .as_deref()
                        .is_some_and(|selected| selected != candidate.item_id)
                    {
                        match landsat::search_scenes_for_source(
                            &source_mode,
                            request.latitude,
                            request.longitude,
                            &acquired_at,
                            window_days,
                            10,
                        )
                        .await
                        {
                            Ok(candidates) => {
                                found = candidates.into_iter().find(|candidate| {
                                    request
                                        .external_scene_id
                                        .as_deref()
                                        .is_some_and(|selected| selected == candidate.item_id)
                                });
                                if found.is_some() {
                                    break;
                                }
                            }
                            Err(err) => {
                                tracing::warn!(error = %err, "selected satellite scene lookup failed");
                                break;
                            }
                        }
                    } else {
                        found = Some(candidate);
                        break;
                    }
                }
                Ok(None) => continue,
                Err(err) => {
                    tracing::warn!(error = %err, "real satellite scene search failed; using sample fallback");
                    break;
                }
            }
        }
        if request.external_scene_id.is_some() && found.is_none() {
            return Err(AppError::BadRequest(
                "selected satellite scene was not found for this location and date window"
                    .to_string(),
            ));
        }
        found
    };
    let scene_id = landsat_candidate
        .as_ref()
        .map(|candidate| cached_landsat_scene_id(candidate, request.latitude, request.longitude))
        .unwrap_or_else(|| {
            format!(
                "mobile_{:.5}_{:.5}_{}_{}d_{}",
                request.latitude,
                request.longitude,
                acquired_at.replace('-', ""),
                search_days,
                Uuid::new_v4().simple()
            )
            .replace('.', "p")
            .replace('-', "m")
        });

    let scene_dir = state.config.data_root.join("scenes").join(&scene_id);
    fs::create_dir_all(&scene_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;

    let extent = extent_around(request.latitude, request.longitude, 0.035);
    let image = if let Some(candidate) = &landsat_candidate {
        describe_real_landsat_scene(
            candidate,
            request.latitude,
            request.longitude,
            extent.clone(),
        )
    } else {
        write_synthetic_landsat_scene(
            &scene_dir,
            request.latitude,
            request.longitude,
            &acquired_at,
            extent.clone(),
        )
        .await?
    };
    let mut metadata_value = serde_json::to_value(&image).map_err(Error::from)?;
    if let Some(candidate) = &landsat_candidate {
        metadata_value["satellite_provider"] = serde_json::json!({
            "dataset": candidate.dataset,
            "dataset_label": candidate.dataset_label,
            "provider": candidate.provider,
            "collection": candidate.collection,
            "item_id": candidate.item_id,
            "acquired_at": candidate.acquired_at,
            "cloud_cover": candidate.cloud_cover,
            "resolution_m": candidate.resolution_m,
            "assets": candidate.assets,
        });
    }
    if let Some(geometry) = &field_geometry {
        metadata_value["field_geometry"] = geometry.clone();
    }
    let metadata_json = serde_json::to_string_pretty(&metadata_value).map_err(Error::from)?;
    fs::write(scene_dir.join("metadata_ingested.json"), &metadata_json)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;

    let synthetic_scene_field_id = (source_mode == "sample").then(|| "sample-mobile".to_string());
    let synthetic_scene_season_id = (source_mode == "sample").then(|| "sample".to_string());

    sqlx::query(
        r#"
        INSERT INTO scenes (scene_id, owner, sensor, acquired_at, data_path, metadata_json, cloud_cover, created_at, field_id, season_id)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(scene_id) DO UPDATE SET owner = excluded.owner,
                                          sensor = excluded.sensor,
                                          acquired_at = excluded.acquired_at,
                                          data_path = excluded.data_path,
                                          metadata_json = excluded.metadata_json,
                                          cloud_cover = excluded.cloud_cover,
                                          field_id = excluded.field_id,
                                          season_id = excluded.season_id
        "#,
    )
    .bind(&scene_id)
    .bind(DEFAULT_RECORD_OWNER)
    .bind(
        landsat_candidate
            .as_ref()
            .map(|candidate| format!("{}-stac-rendered-products", candidate.dataset))
            .unwrap_or_else(|| "landsat8-simulated".to_string()),
    )
    .bind(
        landsat_candidate
            .as_ref()
            .map(|candidate| candidate.acquired_at.clone())
            .unwrap_or_else(|| format!("{acquired_at}T00:00:00Z")),
    )
    .bind(scene_dir.to_string_lossy().to_string())
    .bind(&metadata_json)
    .bind(
        landsat_candidate
            .as_ref()
            .and_then(|candidate| candidate.cloud_cover)
            .unwrap_or(8.0f64),
    )
    .bind(current_record_timestamp())
    .bind(synthetic_scene_field_id)
    .bind(synthetic_scene_season_id)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut mobile_products = Vec::new();
    if let Some(candidate) = &landsat_candidate {
        let (rgb_path, rgb_stats) = create_real_landsat_product(
            &state,
            &scene_id,
            &scene_dir,
            candidate,
            "rgb",
            field_geometry.as_ref(),
        )
        .await?;
        mobile_products.push(mobile_product_from_kind(
            &scene_id,
            "rgb",
            rgb_stats.or(read_product_stats(&rgb_path).await?),
        ));

        for kind in products {
            let (product_path, request_stats) = create_real_landsat_product(
                &state,
                &scene_id,
                &scene_dir,
                candidate,
                &kind,
                field_geometry.as_ref(),
            )
            .await?;
            let stats = request_stats.or(read_product_stats(&product_path).await?);
            mobile_products.push(mobile_product_from_kind(&scene_id, &kind, stats));
        }
    } else {
        create_rgb_product(&state, &scene_id, &scene_dir).await?;
        mobile_products.push(mobile_product_from_kind(&scene_id, "rgb", None));

        for kind in products {
            let product_path = ingest::ensure_product(&state.pool, &scene_id, &kind)
                .await
                .map_err(AppError::Anyhow)?;
            let stats = read_product_stats(&product_path).await?;
            mobile_products.push(mobile_product_from_kind(&scene_id, &kind, stats));
        }
    }

    let response_acquired_at = landsat_candidate
        .as_ref()
        .map(|candidate| candidate.acquired_at.clone())
        .unwrap_or_else(|| acquired_at.clone());
    let source = match &landsat_candidate {
        Some(candidate) if search_days > requested_days => {
            format!(
                "real {} scene selected and rendered from {} after expanding search to {} days",
                candidate.dataset_label, candidate.provider, search_days
            )
        }
        Some(candidate) => {
            format!(
                "real {} scene selected and rendered from {}",
                candidate.dataset_label, candidate.provider
            )
        }
        None if source_mode == "sample" => {
            "backend-generated Landsat-style sample selected by user".to_string()
        }
        None => {
            "backend-generated Landsat-style sample; real Landsat search did not return a usable scene"
                .to_string()
        }
    };
    let asset_count = landsat_candidate
        .as_ref()
        .map(|candidate| candidate.asset_count)
        .unwrap_or(0);

    Ok(Json(MobileAnalyzeResponse {
        scene_id,
        external_scene_id: landsat_candidate
            .as_ref()
            .map(|candidate| candidate.item_id.clone()),
        sensor: if landsat_candidate.is_some() {
            landsat_candidate
                .as_ref()
                .map(|candidate| candidate.dataset_label.clone())
                .unwrap_or_else(|| "Satellite scene metadata".to_string())
        } else {
            "Landsat 8 sample backend".to_string()
        },
        acquired_at: response_acquired_at,
        source,
        dataset: landsat_candidate
            .as_ref()
            .map(|candidate| candidate.dataset.clone()),
        dataset_label: landsat_candidate
            .as_ref()
            .map(|candidate| candidate.dataset_label.clone()),
        provider: landsat_candidate
            .as_ref()
            .map(|candidate| candidate.provider.clone()),
        collection: landsat_candidate
            .as_ref()
            .map(|candidate| candidate.collection.clone()),
        cloud_cover: landsat_candidate
            .as_ref()
            .and_then(|candidate| candidate.cloud_cover),
        resolution_m: landsat_candidate
            .as_ref()
            .map(|candidate| candidate.resolution_m),
        asset_count,
        search_days,
        real_products_ready: landsat_candidate.is_some(),
        location: GpsCoords {
            latitude: request.latitude,
            longitude: request.longitude,
            altitude: 0.0,
        },
        extent,
        products: mobile_products,
    }))
}

fn validate_lat_lon(latitude: f64, longitude: f64) -> AppResult<()> {
    if !latitude.is_finite() || !longitude.is_finite() {
        return Err(AppError::BadRequest(
            "latitude and longitude must be finite numbers".to_string(),
        ));
    }
    if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
        return Err(AppError::BadRequest(
            "latitude or longitude outside valid range".to_string(),
        ));
    }
    Ok(())
}

fn normalize_field_geometry(
    geometry: Option<&serde_json::Value>,
) -> AppResult<Option<serde_json::Value>> {
    let Some(value) = geometry else {
        return Ok(None);
    };

    let geometry = if value.get("type").and_then(|item| item.as_str()) == Some("Feature") {
        value.get("geometry").ok_or_else(|| {
            AppError::BadRequest("field GeoJSON feature must include geometry".to_string())
        })?
    } else {
        value
    };
    let Some(geometry_type) = geometry.get("type").and_then(|item| item.as_str()) else {
        return Err(AppError::BadRequest(
            "field geometry must include a GeoJSON type".to_string(),
        ));
    };
    if !matches!(geometry_type, "Polygon" | "MultiPolygon") {
        return Err(AppError::BadRequest(
            "field geometry must be a Polygon or MultiPolygon".to_string(),
        ));
    }
    if geometry.get("coordinates").is_none() {
        return Err(AppError::BadRequest(
            "field geometry must include coordinates".to_string(),
        ));
    }

    Ok(Some(geometry.clone()))
}

fn normalize_source_mode(source: Option<&str>) -> String {
    match source.unwrap_or("auto").trim().to_lowercase().as_str() {
        "sample" => "sample".to_string(),
        "landsat" | "landsat8" | "landsat9" => "landsat".to_string(),
        "sentinel" | "sentinel2" | "sentinel-2" | "sentinel_2" => "sentinel2".to_string(),
        _ => "auto".to_string(),
    }
}

fn normalize_mobile_products(products: Option<Vec<String>>) -> Vec<String> {
    let requested = products.unwrap_or_else(|| {
        vec![
            "ndvi".to_string(),
            "ndmi".to_string(),
            "nbr".to_string(),
            "mndwi".to_string(),
            "evi2".to_string(),
        ]
    });

    let supported = [
        "ndvi", "ndre", "evi", "savi", "vari", "gndvi", "ndwi", "mndwi", "msavi", "nbr", "ndmi",
        "evi2",
    ];
    let mut normalized = Vec::new();
    for product in requested {
        let kind = product.trim().to_lowercase();
        if supported.contains(&kind.as_str()) && !normalized.contains(&kind) {
            normalized.push(kind);
        }
    }
    if normalized.is_empty() {
        normalized.push("ndvi".to_string());
    }
    normalized
}

fn expanded_landsat_windows(requested_days: u8) -> Vec<u8> {
    let mut windows = Vec::new();
    for window in [requested_days.clamp(1, 30), 14, 30] {
        if !windows.contains(&window) {
            windows.push(window);
        }
    }
    windows
}

fn mobile_scene_candidate(candidate: landsat::LandsatSceneCandidate) -> MobileSceneCandidate {
    MobileSceneCandidate {
        external_scene_id: candidate.item_id,
        dataset: candidate.dataset,
        dataset_label: candidate.dataset_label,
        provider: candidate.provider,
        collection: candidate.collection,
        acquired_at: candidate.acquired_at,
        cloud_cover: candidate.cloud_cover,
        bbox: candidate.bbox,
        resolution_m: candidate.resolution_m,
        asset_count: candidate.asset_count,
    }
}

fn candidate_from_mobile_scene(scene: &MobileSceneCandidate) -> landsat::LandsatSceneCandidate {
    landsat::LandsatSceneCandidate {
        dataset: normalize_source_mode(Some(&scene.dataset)),
        dataset_label: scene.dataset_label.clone(),
        provider: scene.provider.clone(),
        collection: scene.collection.clone(),
        item_id: scene.external_scene_id.clone(),
        acquired_at: scene.acquired_at.clone(),
        cloud_cover: scene.cloud_cover,
        bbox: scene.bbox.clone(),
        resolution_m: scene.resolution_m,
        asset_count: scene.asset_count,
        assets: BTreeMap::new(),
    }
}

fn cached_landsat_scene_id(
    candidate: &landsat::LandsatSceneCandidate,
    latitude: f64,
    longitude: f64,
) -> String {
    sanitize_scene_id(&format!(
        "{}_{}_{:.5}_{:.5}",
        candidate.dataset, candidate.item_id, latitude, longitude
    ))
}

fn sanitize_scene_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn extent_around(latitude: f64, longitude: f64, half_size_degrees: f64) -> SceneExtent {
    SceneExtent {
        min_lon: (longitude - half_size_degrees).clamp(-180.0, 180.0),
        min_lat: (latitude - half_size_degrees).clamp(-90.0, 90.0),
        max_lon: (longitude + half_size_degrees).clamp(-180.0, 180.0),
        max_lat: (latitude + half_size_degrees).clamp(-90.0, 90.0),
    }
}

fn raster_spatial_ref_for_extent(
    extent: &SceneExtent,
    width: u32,
    height: u32,
) -> RasterSpatialRef {
    let resolution_x = (extent.max_lon - extent.min_lon) / width as f64;
    let resolution_y = (extent.max_lat - extent.min_lat) / height as f64;

    RasterSpatialRef {
        georeferenced: true,
        crs: Some("EPSG:4326".to_string()),
        bbox: Some(GeoBounds {
            min_lon: extent.min_lon,
            min_lat: extent.min_lat,
            max_lon: extent.max_lon,
            max_lat: extent.max_lat,
        }),
        geo_transform: Some([
            extent.min_lon,
            resolution_x,
            0.0,
            extent.max_lat,
            0.0,
            -resolution_y,
        ]),
        resolution: Some(RasterResolution {
            x: resolution_x,
            y: resolution_y,
        }),
    }
}

async fn write_synthetic_landsat_scene(
    scene_dir: &FsPath,
    latitude: f64,
    longitude: f64,
    acquired_at: &str,
    extent: SceneExtent,
) -> AppResult<MultispectralImage> {
    let width = 512;
    let height = 512;
    let bands = synthetic_landsat_bands(width, height, latitude, longitude);
    let mut file_paths = BTreeMap::new();

    for (band_name, pixels) in bands {
        let path = scene_dir.join(format!("{band_name}.png"));
        let image = GrayImage::from_raw(width, height, pixels).ok_or_else(|| {
            AppError::Anyhow(anyhow::anyhow!(
                "failed to create synthetic band {band_name}"
            ))
        })?;
        image
            .save(&path)
            .map_err(|err| AppError::Anyhow(err.into()))?;
        file_paths.insert(band_name, path.to_string_lossy().to_string());
    }

    let timestamp = chrono::DateTime::parse_from_rfc3339(&format!("{acquired_at}T00:00:00Z"))
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    Ok(MultispectralImage {
        metadata: ImageMetadata {
            timestamp,
            gps_position: Some(GpsCoords {
                latitude,
                longitude,
                altitude: 0.0,
            }),
            bands: file_paths.keys().cloned().collect(),
            exposure_time: 1.0,
            gain: 1.0,
            width,
            height,
            spatial_ref: Some(raster_spatial_ref_for_extent(&extent, width, height)),
        },
        file_paths: file_paths.into_iter().collect(),
        image_id: Uuid::new_v4(),
    })
}

fn describe_real_landsat_scene(
    candidate: &landsat::LandsatSceneCandidate,
    latitude: f64,
    longitude: f64,
    extent: SceneExtent,
) -> MultispectralImage {
    let timestamp = chrono::DateTime::parse_from_rfc3339(&candidate.acquired_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    MultispectralImage {
        metadata: ImageMetadata {
            timestamp,
            gps_position: Some(GpsCoords {
                latitude,
                longitude,
                altitude: 0.0,
            }),
            bands: candidate.assets.keys().cloned().collect(),
            exposure_time: 1.0,
            gain: 1.0,
            width: 512,
            height: 512,
            spatial_ref: Some(raster_spatial_ref_for_extent(&extent, 512, 512)),
        },
        file_paths: candidate.assets.clone().into_iter().collect(),
        image_id: Uuid::new_v4(),
    }
}

fn synthetic_landsat_bands(
    width: u32,
    height: u32,
    latitude: f64,
    longitude: f64,
) -> Vec<(String, Vec<u8>)> {
    let mut b2 = Vec::with_capacity((width * height) as usize);
    let mut b3 = Vec::with_capacity((width * height) as usize);
    let mut b4 = Vec::with_capacity((width * height) as usize);
    let mut b5 = Vec::with_capacity((width * height) as usize);
    let mut b6 = Vec::with_capacity((width * height) as usize);
    let mut b7 = Vec::with_capacity((width * height) as usize);

    let lat_seed = latitude as f32;
    let lon_seed = longitude as f32;
    let location_phase = ((latitude * 0.37 + longitude * 0.19).sin() as f32) * 0.10;
    let field_scale_x = 3.0 + ((lat_seed * 1.91).sin().abs() * 5.0);
    let field_scale_y = 3.0 + ((lon_seed * 1.37).cos().abs() * 5.0);
    let row_angle = (lat_seed * 0.17 + lon_seed * 0.11).sin();
    let stress_cx = 0.18 + ((lat_seed * 0.73).sin().abs() * 0.64);
    let stress_cy = 0.18 + ((lon_seed * 0.67).cos().abs() * 0.64);
    let wet_cx = 0.15 + ((lat_seed * 0.41 + lon_seed * 0.23).cos().abs() * 0.70);
    let wet_cy = 0.15 + ((lat_seed * 0.29 - lon_seed * 0.31).sin().abs() * 0.70);
    let tint_r = ((lat_seed * 0.13).sin() * 0.045).clamp(-0.045, 0.045);
    let tint_g = ((lon_seed * 0.09).cos() * 0.045).clamp(-0.045, 0.045);
    let tint_b = (((lat_seed + lon_seed) * 0.07).sin() * 0.035).clamp(-0.035, 0.035);
    for y in 0..height {
        for x in 0..width {
            let nx = x as f32 / (width - 1) as f32;
            let ny = y as f32 / (height - 1) as f32;
            let rotated_x = (nx * row_angle.cos()) - (ny * row_angle.sin());
            let rotated_y = (nx * row_angle.sin()) + (ny * row_angle.cos());
            let irrigation = ((rotated_x * 22.0 + lat_seed as f32).sin()
                * (rotated_y * 17.0 + lon_seed as f32).cos())
            .max(0.0);
            let field_bands = (((nx * field_scale_x).floor() as i32
                + (ny * field_scale_y).floor() as i32)
                % 2) as f32;
            let stress_patch = gaussian(nx, ny, stress_cx, stress_cy, 0.10 + field_scale_x * 0.006);
            let wet_patch = gaussian(nx, ny, wet_cx, wet_cy, 0.11 + field_scale_y * 0.007);
            let diagonal = ((nx + ny + location_phase).fract() * 0.08).clamp(0.0, 0.08);
            let vegetation = (0.48 + irrigation * 0.24 + field_bands * 0.12 - stress_patch * 0.42
                + location_phase)
                .clamp(0.05, 0.95);
            let moisture = (0.35 + wet_patch * 0.42 - stress_patch * 0.16).clamp(0.05, 0.9);
            let soil = (1.0 - vegetation).clamp(0.0, 1.0);

            b2.push(to_u8(0.18 + soil * 0.10 + wet_patch * 0.06 + tint_b));
            b3.push(to_u8(
                0.24 + vegetation * 0.22 + wet_patch * 0.08 + tint_g + diagonal,
            ));
            b4.push(to_u8(
                0.18 + soil * 0.30 + stress_patch * 0.20 + tint_r + diagonal * 0.5,
            ));
            b5.push(to_u8(0.28 + vegetation * 0.58 - stress_patch * 0.24));
            b6.push(to_u8(
                0.22 + soil * 0.28 - moisture * 0.14 + stress_patch * 0.12,
            ));
            b7.push(to_u8(
                0.18 + soil * 0.35 - moisture * 0.08 + stress_patch * 0.18,
            ));
        }
    }

    vec![
        ("B2".to_string(), b2),
        ("B3".to_string(), b3),
        ("B4".to_string(), b4),
        ("B5".to_string(), b5),
        ("B6".to_string(), b6),
        ("B7".to_string(), b7),
    ]
}

fn gaussian(x: f32, y: f32, cx: f32, cy: f32, radius: f32) -> f32 {
    let dx = x - cx;
    let dy = y - cy;
    (-(dx * dx + dy * dy) / (2.0 * radius * radius)).exp()
}

fn to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

async fn create_rgb_product(state: &AppState, scene_id: &str, scene_dir: &FsPath) -> AppResult<()> {
    let red = image::open(scene_dir.join("B4.png"))
        .map_err(|err| AppError::Anyhow(err.into()))?
        .to_luma8();
    let green = image::open(scene_dir.join("B3.png"))
        .map_err(|err| AppError::Anyhow(err.into()))?
        .to_luma8();
    let blue = image::open(scene_dir.join("B2.png"))
        .map_err(|err| AppError::Anyhow(err.into()))?
        .to_luma8();
    let (width, height) = red.dimensions();
    let mut rgb = ImageBuffer::new(width, height);
    for y in 0..height {
        for x in 0..width {
            rgb.put_pixel(
                x,
                y,
                Rgb([
                    red.get_pixel(x, y)[0],
                    green.get_pixel(x, y)[0],
                    blue.get_pixel(x, y)[0],
                ]),
            );
        }
    }

    let product_dir = scene_dir.join("products").join("rgb");
    fs::create_dir_all(&product_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let product_path = product_dir.join("rgb.png");
    DynamicImage::ImageRgb8(rgb)
        .save(&product_path)
        .map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO products (scene_id, kind, path, created_at)
        VALUES (?1, 'rgb', ?2, datetime('now'))
        ON CONFLICT(scene_id, kind) DO UPDATE SET path = excluded.path,
                                                width_px = NULL,
                                                height_px = NULL,
                                                gsd_m_per_px = NULL,
                                                publish_status = NULL,
                                                qa_report_ref = NULL,
                                                provenance_hash = NULL,
                                                downstream_consumers_json = NULL,
                                                created_at = datetime('now')
        "#,
    )
    .bind(scene_id)
    .bind(product_path.to_string_lossy().to_string())
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn create_real_landsat_product(
    state: &AppState,
    scene_id: &str,
    scene_dir: &FsPath,
    candidate: &landsat::LandsatSceneCandidate,
    kind: &str,
    field_geometry: Option<&serde_json::Value>,
) -> AppResult<(PathBuf, Option<serde_json::Value>)> {
    let kind = kind.to_lowercase();
    let product_dir = scene_dir.join("products").join(&kind);
    fs::create_dir_all(&product_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;

    let product_path = product_dir.join(format!("{kind}.png"));
    if product_path.exists() {
        upsert_product_path(state, scene_id, &kind, &product_path).await?;
        let stats = if field_geometry.is_some() {
            landsat::product_statistics(candidate, &kind, field_geometry)
                .await
                .map_err(AppError::Anyhow)?
        } else {
            None
        };
        return Ok((product_path, stats));
    }

    let bytes = landsat::render_product_png(candidate, &kind)
        .await
        .map_err(AppError::Anyhow)?;
    image::load_from_memory(&bytes).map_err(|err| AppError::Anyhow(err.into()))?;
    fs::write(&product_path, bytes)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;

    let request_stats = landsat::product_statistics(candidate, &kind, field_geometry)
        .await
        .map_err(AppError::Anyhow)?;
    if field_geometry.is_none() {
        if let Some(mut stats) = request_stats.clone() {
            if let Some(object) = stats.as_object_mut() {
                object.insert(
                    "output_path".to_string(),
                    serde_json::Value::String(product_path.to_string_lossy().to_string()),
                );
                object.insert(
                    "timestamp".to_string(),
                    serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
                );
            }
            let stats_path = product_dir.join(format!("{kind}_result.json"));
            let stats_json = serde_json::to_string_pretty(&stats).map_err(Error::from)?;
            fs::write(stats_path, stats_json)
                .await
                .map_err(|err| AppError::Anyhow(err.into()))?;
        }
    }

    upsert_product_path(state, scene_id, &kind, &product_path).await?;

    Ok((product_path, request_stats))
}

async fn upsert_product_path(
    state: &AppState,
    scene_id: &str,
    kind: &str,
    product_path: &FsPath,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO products (scene_id, kind, path, created_at)
        VALUES (?1, ?2, ?3, datetime('now'))
        ON CONFLICT(scene_id, kind) DO UPDATE SET path = excluded.path,
                                                width_px = NULL,
                                                height_px = NULL,
                                                gsd_m_per_px = NULL,
                                                publish_status = NULL,
                                                qa_report_ref = NULL,
                                                provenance_hash = NULL,
                                                downstream_consumers_json = NULL,
                                                created_at = datetime('now')
        "#,
    )
    .bind(scene_id)
    .bind(kind)
    .bind(product_path.to_string_lossy().to_string())
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn read_product_stats(product_path: &FsPath) -> AppResult<Option<serde_json::Value>> {
    let Some(product_dir) = product_path.parent() else {
        return Ok(None);
    };
    let mut entries = fs::read_dir(product_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with("_result.json"))
        {
            let text = fs::read_to_string(path)
                .await
                .map_err(|err| AppError::Anyhow(err.into()))?;
            let stats = serde_json::from_str(&text).map_err(Error::from)?;
            return Ok(Some(stats));
        }
    }
    Ok(None)
}

fn mobile_product_from_kind(
    scene_id: &str,
    kind: &str,
    stats: Option<serde_json::Value>,
) -> MobileProduct {
    MobileProduct {
        kind: kind.to_string(),
        label: product_label(kind).to_string(),
        url_path: format!("/api/scenes/{scene_id}/products/{kind}"),
        tile_url_template: format!(
            "/api/scenes/{scene_id}/products/{kind}/tiles/{{z}}/{{x}}/{{y}}.png"
        ),
        stats,
    }
}

fn product_label(kind: &str) -> &'static str {
    match kind {
        "rgb" => "Natural Color",
        "ndvi" => "Vegetation Health (NDVI)",
        "ndmi" => "Crop Moisture (NDMI)",
        "nbr" => "Stress / Burn Index (NBR)",
        "mndwi" => "Water / Wet Areas (MNDWI)",
        "evi2" => "Enhanced Vegetation (EVI2)",
        "ndwi" => "Water Index (NDWI)",
        "savi" => "Soil Adjusted Vegetation (SAVI)",
        "gndvi" => "Green NDVI",
        "vari" => "Visible Atmospherically Resistant Index",
        "ndre" => "Red Edge Index (NDRE)",
        "msavi" => "Modified SAVI",
        _ => "Analysis Layer",
    }
}

pub async fn import_fields_geojson(
    State(state): State<AppState>,
    Json(payload): Json<GeoJson>,
) -> AppResult<Json<Vec<FieldRecord>>> {
    let fields = fields_from_geojson(payload)?;

    let fields = upsert_fields(&state, &fields).await?;

    Ok(Json(fields))
}

pub async fn import_fields_shapefile(
    State(state): State<AppState>,
    Json(payload): Json<ImportShapefileRequest>,
) -> AppResult<Json<Vec<FieldRecord>>> {
    let fields = fields_from_shapefile(payload).await?;

    let fields = upsert_fields(&state, &fields).await?;

    Ok(Json(fields))
}

async fn upsert_fields(state: &AppState, fields: &[FieldRecord]) -> AppResult<Vec<FieldRecord>> {
    let mut persisted = Vec::with_capacity(fields.len());
    for field in fields {
        let mut field = field.clone();
        field.owner = field_owner_for_farm(state, field.farm_id.as_deref(), &field.owner).await?;
        field.org_id = field.owner.clone();
        sqlx::query(
            r#"
            INSERT INTO fields (field_id, farm_id, owner, name, crop, season, notes, boundary_json, status, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(field_id) DO UPDATE SET
                farm_id = excluded.farm_id,
                owner = excluded.owner,
                name = excluded.name,
                crop = excluded.crop,
                season = excluded.season,
                notes = excluded.notes,
                boundary_json = excluded.boundary_json,
                status = excluded.status,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&field.field_id)
        .bind(&field.farm_id)
        .bind(&field.owner)
        .bind(&field.name)
        .bind(&field.crop)
        .bind(&field.season)
        .bind(&field.notes)
        .bind(serde_json::to_string(&field.boundary).map_err(|err| AppError::Anyhow(err.into()))?)
        .bind(field.status.as_str())
        .bind(&field.created_at)
        .bind(&field.updated_at)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;
        persisted.push(field);
    }

    Ok(persisted)
}

pub async fn enroll_fleet_node(
    State(state): State<AppState>,
    Json(request): Json<FleetNodeEnrollmentRequest>,
) -> AppResult<Json<FleetNodeRecord>> {
    let binding = bind_fleet_node_identity(
        request.clone(),
        None,
        Uuid::new_v4().to_string(),
        current_record_timestamp(),
    )
    .map_err(fleet_enrollment_error)?;
    let record = binding.record;
    let capabilities_json =
        serde_json::to_string(&record.capabilities).map_err(|err| AppError::Anyhow(err.into()))?;

    let result = sqlx::query(
        r#"
        INSERT OR IGNORE INTO fleet_nodes
            (node_id, hardware_id, kind, capabilities_json, owner_org_id, runtime_mode, enrolled_at, status)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&record.node_id)
    .bind(&record.hardware_id)
    .bind(record.kind.as_str())
    .bind(capabilities_json)
    .bind(&record.owner_org_id)
    .bind(record.runtime_mode.as_str())
    .bind(&record.enrolled_at)
    .bind(record.status.as_str())
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    if result.rows_affected() == 0 {
        let existing = load_fleet_node_by_hardware_id(&state, &record.hardware_id)
            .await?
            .ok_or_else(|| AppError::Anyhow(anyhow::anyhow!("fleet node conflict not found")))?;
        let binding =
            bind_fleet_node_identity(request, Some(existing), record.node_id, record.enrolled_at)
                .map_err(fleet_enrollment_error)?;
        return Ok(Json(binding.record));
    }

    Ok(Json(record))
}

pub async fn list_fleet_nodes(
    Query(query): Query<FleetNodeListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FleetNodeRecord>>> {
    let owner_org_id = normalize_optional_text(query.owner_org_id);
    let rows = if let Some(owner_org_id) = owner_org_id {
        sqlx::query(
            r#"
            SELECT node_id, hardware_id, kind, capabilities_json, owner_org_id, runtime_mode, enrolled_at, status
            FROM fleet_nodes
            WHERE owner_org_id = ?1
            ORDER BY enrolled_at DESC, node_id ASC
            "#,
        )
        .bind(owner_org_id)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?
    } else {
        sqlx::query(
            r#"
            SELECT node_id, hardware_id, kind, capabilities_json, owner_org_id, runtime_mode, enrolled_at, status
            FROM fleet_nodes
            ORDER BY enrolled_at DESC, node_id ASC
            "#,
        )
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?
    };

    rows.into_iter()
        .map(|row| decode_fleet_node_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_fleet_node(
    Path(node_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<FleetNodeRecord>> {
    let node = load_fleet_node(&state, &node_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(node))
}

pub async fn register_tractor(
    State(state): State<AppState>,
    Json(mut request): Json<TractorRegistrationRequest>,
) -> AppResult<Json<TractorRecord>> {
    if request
        .tractor_id
        .as_ref()
        .is_none_or(|tractor_id| tractor_id.trim().is_empty())
    {
        request.tractor_id = Some(Uuid::new_v4().to_string());
    }

    let field_id = normalize_optional_text(Some(request.field_id.clone()))
        .ok_or_else(|| AppError::BadRequest("tractor field_id is required".to_string()))?;
    let field = load_field(&state, &field_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("field {field_id} does not exist")))?;
    let record = build_tractor_record(request, &field, current_record_timestamp())
        .map_err(tractor_registry_error)?;
    if load_tractor(&state, &record.tractor_id).await?.is_some() {
        return Err(AppError::BadRequest(format!(
            "tractor {} is already registered",
            record.tractor_id
        )));
    }
    insert_tractor_record(&state, &record).await?;
    Ok(Json(record))
}

pub async fn list_tractors(
    Query(query): Query<TractorListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<TractorRecord>>> {
    let org_id = normalize_optional_text(query.org_id);
    let field_id = normalize_optional_text(query.field_id);
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT tractor_id, org_id, field_id, capabilities_json, implement_ref_json, status,
               registered_at, updated_at
        FROM tractor_vehicles
        WHERE (?1 IS NULL OR org_id = ?1)
          AND (?2 IS NULL OR field_id = ?2)
          AND (?3 IS NULL OR status = ?3)
        ORDER BY tractor_id ASC
        "#,
    )
    .bind(org_id)
    .bind(field_id)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_tractor_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_tractor(
    Path(tractor_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<TractorRecord>> {
    let tractor = load_tractor(&state, &tractor_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(tractor))
}

pub async fn validate_tractor_motion_command(
    Path(tractor_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<TractorMotionCommandValidationRequest>,
) -> AppResult<Response> {
    let command_type = normalize_optional_text(Some(request.command_type))
        .ok_or_else(|| AppError::BadRequest("tractor command_type is required".to_string()))?;
    let command = TractorMotionCommandRequest {
        command_id: normalize_optional_text(request.command_id),
        tractor_id: tractor_id.clone(),
        command_type,
        requested_by: normalize_optional_text(request.requested_by),
    };

    let Some(tractor) = load_tractor(&state, &tractor_id).await? else {
        let audit = build_tractor_command_audit(
            &command,
            None,
            TractorCommandRejectionReason::UnknownTractor,
        );
        insert_tractor_command_audit(&state, &audit).await?;
        let rejection = TractorCommandRejection {
            tractor_id,
            reason: TractorCommandRejectionReason::UnknownTractor,
            status: None,
            audit,
        };
        return Ok((tractor_rejection_status(&rejection), Json(rejection)).into_response());
    };

    if tractor.status == TractorLifecycleStatus::OutOfService {
        let audit = build_tractor_command_audit(
            &command,
            Some(&tractor),
            TractorCommandRejectionReason::TractorOutOfService,
        );
        insert_tractor_command_audit(&state, &audit).await?;
        let rejection = TractorCommandRejection {
            tractor_id,
            reason: TractorCommandRejectionReason::TractorOutOfService,
            status: Some(tractor.status),
            audit,
        };
        return Ok((tractor_rejection_status(&rejection), Json(rejection)).into_response());
    }

    Ok(Json(tractor).into_response())
}

pub async fn pull_weather_forecast(
    State(state): State<AppState>,
    Json(request): Json<PullWeatherForecastRequest>,
) -> AppResult<Response> {
    validate_lat_lon(request.latitude, request.longitude)?;
    let field_id = normalize_optional_text(Some(request.field_id))
        .ok_or_else(|| AppError::BadRequest("weather field_id is required".to_string()))?;
    load_field(&state, &field_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("field {field_id} does not exist")))?;
    let field_ref = canonical_weather_field_ref(&field_id);
    let provider = normalize_optional_text(Some(request.provider))
        .ok_or_else(|| AppError::BadRequest("weather provider is required".to_string()))?;
    let fetched_at =
        normalize_optional_text(request.fetched_at).unwrap_or_else(current_record_timestamp);

    if provider.eq_ignore_ascii_case("unreachable") {
        let failure = weather_fetch_failure_record(
            format!("weather-fetch-failure-{}", Uuid::new_v4()),
            field_ref,
            provider,
            fetched_at,
            "provider unreachable".to_string(),
        )
        .map_err(weather_ingest_error)?;
        insert_weather_fetch_failure(
            &state,
            &field_id,
            &failure,
            request.latitude,
            request.longitude,
            current_record_timestamp(),
        )
        .await?;
        return Ok((StatusCode::BAD_GATEWAY, Json(failure)).into_response());
    }

    let provider_response =
        sample_weather_provider_response(&provider, fetched_at, request.valid_time)?;
    let records = normalize_weather_provider_forecast(field_ref, provider_response)
        .map_err(weather_ingest_error)?;
    for record in &records {
        let created_at = current_record_timestamp();
        insert_weather_forecast_record(
            &state,
            &field_id,
            record,
            request.latitude,
            request.longitude,
            created_at.clone(),
        )
        .await?;
        insert_weather_time_series_points(&state, &field_id, record, created_at).await?;
    }

    Ok(Json(records).into_response())
}

pub async fn list_weather_forecasts(
    Query(query): Query<WeatherForecastListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<WeatherForecastRecord>>> {
    let field_id = normalize_optional_text(query.field_id);
    let source = normalize_optional_text(query.source);
    let rows = sqlx::query(
        r#"
        SELECT forecast_id, field_id, field_ref, valid_time, vars_json, source, fetched_at
        FROM weather_forecasts
        WHERE (?1 IS NULL OR field_id = ?1)
          AND (?2 IS NULL OR source = ?2)
        ORDER BY valid_time ASC, forecast_id ASC
        "#,
    )
    .bind(field_id)
    .bind(source)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_weather_forecast_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn list_weather_fetch_failures(
    Query(query): Query<WeatherFetchFailureListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<WeatherFetchFailureRecord>>> {
    let field_id = normalize_optional_text(query.field_id);
    let source = normalize_optional_text(query.source);
    let rows = sqlx::query(
        r#"
        SELECT failure_id, field_id, field_ref, source, fetched_at, reason
        FROM weather_fetch_failures
        WHERE (?1 IS NULL OR field_id = ?1)
          AND (?2 IS NULL OR source = ?2)
        ORDER BY fetched_at DESC, failure_id ASC
        "#,
    )
    .bind(field_id)
    .bind(source)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(
        rows.into_iter()
            .map(|row| WeatherFetchFailureRecord {
                failure_id: row.get("failure_id"),
                field_ref: row.get("field_ref"),
                source: row.get("source"),
                fetched_at: row.get("fetched_at"),
                reason: row.get("reason"),
            })
            .collect(),
    ))
}

pub async fn register_fleet_component(
    State(state): State<AppState>,
    Json(request): Json<RegisterComponentRequest>,
) -> AppResult<Json<FleetComponentRecord>> {
    let record = build_component_record(
        request,
        format!("fleet-component-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(fleet_health_error)?;

    if let Some(airframe_id) = &record.airframe_id {
        validate_enrolled_airframe(&state, airframe_id).await?;
    }

    insert_fleet_component(&state, &record).await?;
    append_fleet_component_event(
        &state,
        &component_event(
            &record.component_id,
            "registered",
            record.airframe_id.clone(),
            record.created_at.clone(),
            None,
            Some(format!("serial {}", record.serial)),
        )
        .map_err(fleet_health_error)?,
    )
    .await?;
    if let (Some(airframe_id), Some(installed_at)) = (&record.airframe_id, &record.installed_at) {
        append_fleet_component_event(
            &state,
            &component_event(
                &record.component_id,
                "installed",
                Some(airframe_id.clone()),
                installed_at.clone(),
                None,
                Some("initial install".to_string()),
            )
            .map_err(fleet_health_error)?,
        )
        .await?;
    }
    for service in &record.service_history {
        append_fleet_component_event(
            &state,
            &component_event(
                &record.component_id,
                "service_recorded",
                record.airframe_id.clone(),
                service.performed_at.clone(),
                Some(service.technician.clone()),
                Some(service.action.clone()),
            )
            .map_err(fleet_health_error)?,
        )
        .await?;
    }

    Ok(Json(record))
}

pub async fn list_fleet_components(
    Query(query): Query<FleetComponentListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FleetComponentRecord>>> {
    let airframe_id = normalize_optional_text(query.airframe_id);
    let component_type = normalize_optional_text(query.component_type)
        .map(parse_fleet_component_type)
        .transpose()?
        .map(|component_type| component_type.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT component_id, component_type, serial, airframe_id, installed_at, removed_at,
               service_history_json, flight_hours, cycles, duty_score, created_at, updated_at
        FROM fleet_components
        WHERE (?1 IS NULL OR airframe_id = ?1)
          AND (?2 IS NULL OR component_type = ?2)
        ORDER BY updated_at DESC, component_id ASC
        "#,
    )
    .bind(airframe_id)
    .bind(component_type)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_fleet_component_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_fleet_component_history(
    Path(component_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FleetComponentEventRecord>>> {
    load_fleet_component(&state, &component_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let rows = sqlx::query(
        r#"
        SELECT component_id, event_type, airframe_id, event_at, actor, details
        FROM fleet_component_events
        WHERE component_id = ?1
        ORDER BY event_at ASC, id ASC
        "#,
    )
    .bind(component_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_fleet_component_event(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn install_fleet_component_route(
    Path(component_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<InstallComponentRequest>,
) -> AppResult<Json<FleetComponentRecord>> {
    let component_id = normalize_optional_text(Some(component_id))
        .ok_or_else(|| AppError::BadRequest("component_id is required".to_string()))?;
    let existing = load_fleet_component(&state, &component_id)
        .await?
        .ok_or(AppError::NotFound)?;
    validate_enrolled_airframe(&state, request.airframe_id.trim()).await?;
    let attempted_airframe = request.airframe_id.trim().to_string();
    let attempted_at = normalize_optional_text(Some(request.installed_at.clone()))
        .unwrap_or_else(current_record_timestamp);
    let actor = request.actor.clone();

    let updated = match install_component(&existing, request, current_record_timestamp()) {
        Ok(updated) => updated,
        Err(FleetHealthError::AlreadyInstalled { .. }) => {
            append_fleet_component_event(
                &state,
                &component_event(
                    &component_id,
                    "double_install_rejected",
                    Some(attempted_airframe),
                    attempted_at,
                    actor,
                    Some("component already installed on another airframe".to_string()),
                )
                .map_err(fleet_health_error)?,
            )
            .await?;
            return Err(fleet_health_error(FleetHealthError::AlreadyInstalled {
                component_id: existing.component_id,
                airframe_id: existing.airframe_id.unwrap_or_default(),
            }));
        }
        Err(error) => return Err(fleet_health_error(error)),
    };

    update_fleet_component_install(&state, &updated).await?;
    append_fleet_component_event(
        &state,
        &component_event(
            &updated.component_id,
            "installed",
            updated.airframe_id.clone(),
            updated
                .installed_at
                .clone()
                .unwrap_or_else(current_record_timestamp),
            actor,
            Some("component installed".to_string()),
        )
        .map_err(fleet_health_error)?,
    )
    .await?;

    Ok(Json(updated))
}

pub async fn accrue_fleet_component_duty(
    State(state): State<AppState>,
    Json(request): Json<DutyAccrualRequest>,
) -> AppResult<Json<Vec<ComponentDutyAccrualRecord>>> {
    validate_enrolled_airframe(&state, request.airframe_id.trim()).await?;
    let components =
        load_active_fleet_components_for_airframe(&state, request.airframe_id.trim()).await?;
    let component_ids = components
        .iter()
        .map(|component| component.component_id.clone())
        .collect::<Vec<_>>();
    let accruals =
        build_component_duty_accruals(request, &component_ids).map_err(fleet_health_error)?;

    for accrual in &accruals {
        let inserted = insert_component_duty_accrual(&state, accrual).await?;
        if inserted {
            if let Some(component) = components
                .iter()
                .find(|component| component.component_id == accrual.component_id)
            {
                let updated = accrue_component_duty(component, accrual, current_record_timestamp())
                    .map_err(fleet_health_error)?;
                update_fleet_component_duty_totals(&state, &updated).await?;
                append_fleet_component_event(
                    &state,
                    &component_event(
                        &updated.component_id,
                        "duty_accrued",
                        updated.airframe_id.clone(),
                        accrual.accrued_at.clone(),
                        None,
                        Some(format!("session {}", accrual.session_id)),
                    )
                    .map_err(fleet_health_error)?,
                )
                .await?;
            }
        }
    }

    let session_id = accruals
        .first()
        .map(|accrual| accrual.session_id.clone())
        .unwrap_or_default();
    let airframe_id = accruals
        .first()
        .map(|accrual| accrual.airframe_id.clone())
        .unwrap_or_default();
    let persisted = if session_id.is_empty() {
        Vec::new()
    } else {
        load_component_duty_accruals_for_session(&state, &session_id, &airframe_id).await?
    };

    Ok(Json(persisted))
}

pub async fn derive_fleet_health_indicators_route(
    State(state): State<AppState>,
    Json(mut request): Json<TelemetryHealthIndicatorRequest>,
) -> AppResult<Json<FleetHealthIndicatorDerivation>> {
    let component_ids = request
        .samples
        .iter()
        .filter_map(|sample| normalize_optional_text(Some(sample.component_id.clone())))
        .chain(
            request
                .telemetry_gaps
                .iter()
                .filter_map(|gap| normalize_optional_text(Some(gap.component_id.clone()))),
        )
        .collect::<BTreeSet<_>>();
    let mut components = BTreeMap::new();
    for component_id in component_ids {
        let component = load_fleet_component(&state, &component_id)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("component {component_id} does not exist"))
            })?;
        components.insert(component.component_id.clone(), component);
    }
    for sample in &mut request.samples {
        if let Some(component_id) = normalize_optional_text(Some(sample.component_id.clone())) {
            if let Some(component) = components.get(&component_id) {
                sample.component_id = component.component_id.clone();
                sample.component_type = component.component_type;
            }
        }
    }
    for gap in &mut request.telemetry_gaps {
        if let Some(component_id) = normalize_optional_text(Some(gap.component_id.clone())) {
            if let Some(component) = components.get(&component_id) {
                gap.component_id = component.component_id.clone();
            }
        }
    }

    let derived = derive_health_indicators(request).map_err(fleet_health_error)?;
    for sample in &derived.samples {
        let airframe_id = components
            .get(&sample.component_id)
            .and_then(|component| component.airframe_id.as_deref());
        insert_fleet_health_indicator_sample(&state, sample, airframe_id).await?;
        insert_time_series_point(&state, sample).await?;
    }
    for gap in &derived.gaps {
        let airframe_id = components
            .get(&gap.component_id)
            .and_then(|component| component.airframe_id.as_deref());
        insert_fleet_health_telemetry_gap(&state, gap, airframe_id, &derived).await?;
    }

    Ok(Json(derived))
}

pub async fn list_fleet_health_indicators(
    Query(query): Query<FleetHealthIndicatorListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FleetHealthIndicatorSample>>> {
    let component_id = normalize_optional_text(query.component_id);
    let indicator = normalize_optional_text(query.indicator)
        .map(parse_fleet_health_indicator)
        .transpose()?
        .map(|indicator| indicator.as_str().to_string());
    let freshness = normalize_optional_text(query.freshness)
        .map(parse_health_indicator_freshness)
        .transpose()?
        .map(|freshness| freshness.as_str().to_string());

    let rows = sqlx::query(
        r#"
        SELECT component_id, indicator, value, ts, source_ref, freshness, created_at
        FROM fleet_health_indicator_samples
        WHERE (?1 IS NULL OR component_id = ?1)
          AND (?2 IS NULL OR indicator = ?2)
          AND (?3 IS NULL OR freshness = ?3)
        ORDER BY ts ASC, component_id ASC, indicator ASC
        "#,
    )
    .bind(component_id)
    .bind(indicator)
    .bind(freshness)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_fleet_health_indicator_sample(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn evaluate_ota_rollout_route(
    Json(request): Json<OtaRolloutRequest>,
) -> AppResult<Json<OtaRolloutDecision>> {
    evaluate_ota_rollout(request)
        .map(Json)
        .map_err(fleet_health_error)
}

pub async fn apply_rollout_control_route(
    Json(request): Json<RolloutControlRequest>,
) -> AppResult<Json<RolloutControlDecision>> {
    apply_rollout_control(request)
        .map(Json)
        .map_err(fleet_health_error)
}

pub async fn register_soil_iot_device(
    State(state): State<AppState>,
    Json(request): Json<RegisterSoilDeviceRequest>,
) -> AppResult<Json<SoilDeviceRecord>> {
    let record = build_soil_device_record(
        request,
        Uuid::new_v4().to_string(),
        current_record_timestamp(),
    )
    .map_err(soil_iot_error)?;

    insert_soil_iot_device(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_soil_iot_devices(
    Query(query): Query<SoilDeviceListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SoilDeviceRecord>>> {
    let org_id = normalize_optional_text(query.org_id);
    let field_id = normalize_optional_text(query.field_id);
    let zone_id = normalize_optional_text(query.zone_id);
    let status = normalize_optional_text(query.status)
        .map(parse_soil_device_status)
        .transpose()?
        .map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT device_id, org_id, field_id, zone_id, sensor_type, latitude, longitude, crs,
               calibration_profile_ref, status, created_at, updated_at
        FROM soil_iot_devices
        WHERE (?1 IS NULL OR org_id = ?1)
          AND (?2 IS NULL OR field_id = ?2)
          AND (?3 IS NULL OR zone_id = ?3)
          AND (?4 IS NULL OR status = ?4)
        ORDER BY updated_at DESC, device_id ASC
        "#,
    )
    .bind(org_id)
    .bind(field_id)
    .bind(zone_id)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_soil_iot_device(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn record_soil_iot_config_push(
    Path(device_id): Path<String>,
    State(state): State<AppState>,
    Json(mut request): Json<SoilDeviceConfigPushRequest>,
) -> AppResult<Json<SoilDeviceConfigPushRecord>> {
    let device_id = normalize_optional_text(Some(device_id))
        .ok_or_else(|| AppError::BadRequest("device_id cannot be empty".to_string()))?;
    if let Some(body_device_id) = normalize_optional_text(Some(request.device_id.clone())) {
        if body_device_id != device_id {
            return Err(AppError::BadRequest(format!(
                "request device_id {} does not match path device_id {}",
                body_device_id, device_id
            )));
        }
    }
    load_soil_iot_device(&state, &device_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("device {device_id} is not registered")))?;

    request.device_id = device_id;
    let record = build_soil_config_push_record(request, Uuid::new_v4().to_string())
        .map_err(soil_iot_error)?;
    insert_soil_iot_config_push(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_soil_iot_config_pushes(
    Path(device_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SoilDeviceConfigPushRecord>>> {
    let device_id = normalize_optional_text(Some(device_id))
        .ok_or_else(|| AppError::BadRequest("device_id cannot be empty".to_string()))?;
    load_soil_iot_device(&state, &device_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("device {device_id} is not registered")))?;

    let rows = sqlx::query(
        r#"
        SELECT push_id, device_id, config_version, pushed_at, push_status, failure_reason, updated_at
        FROM soil_iot_config_pushes
        WHERE device_id = ?1
        ORDER BY pushed_at ASC, push_id ASC
        "#,
    )
    .bind(&device_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_soil_iot_config_push(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn update_soil_iot_config_push_status(
    Path((device_id, push_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(mut request): Json<SoilDeviceConfigPushStatusUpdate>,
) -> AppResult<Json<SoilDeviceConfigPushRecord>> {
    let device_id = normalize_optional_text(Some(device_id))
        .ok_or_else(|| AppError::BadRequest("device_id cannot be empty".to_string()))?;
    let push_id = normalize_optional_text(Some(push_id))
        .ok_or_else(|| AppError::BadRequest("push_id cannot be empty".to_string()))?;
    let record = load_soil_iot_config_push(&state, &push_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("config push {push_id} is not registered")))?;
    if record.device_id != device_id {
        return Err(AppError::BadRequest(format!(
            "config push {} belongs to device {}",
            push_id, record.device_id
        )));
    }

    if normalize_optional_text(Some(request.updated_at.clone())).is_none() {
        request.updated_at = current_record_timestamp();
    }
    let updated = transition_soil_config_push_status(&record, request).map_err(soil_iot_error)?;
    update_soil_iot_config_push(&state, &updated).await?;

    Ok(Json(updated))
}

pub async fn ingest_soil_iot_reading(
    State(state): State<AppState>,
    Json(request): Json<GatewayReadingRecord>,
) -> AppResult<Json<GeolocatedSoilReading>> {
    let device = load_soil_iot_device(&state, &request.device_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!("device {} is not registered", request.device_id))
        })?;
    let reading = build_geolocated_soil_reading(&device, request).map_err(gateway_ingest_error)?;
    if !reading.excluded_from_geospatial_products {
        let metadata = soil_reading_time_series_metadata(&reading)?;
        insert_time_series_point_record(&state, &reading.to_series_point(), Some(metadata)).await?;
    }

    Ok(Json(reading))
}

pub async fn ingest_soil_moisture_reading(
    State(state): State<AppState>,
    Json(request): Json<SoilMoistureReadingRequest>,
) -> AppResult<Response> {
    let ingested_at = current_record_timestamp();
    let field_id = match normalize_optional_text(request.field_id.clone()) {
        Some(field_id) => field_id,
        None => {
            return reject_soil_moisture_reading(
                &state,
                &request,
                SoilMoistureRejectionReason::MissingFieldLinkage,
                ingested_at,
            )
            .await;
        }
    };

    let Some(field) = load_field(&state, &field_id).await? else {
        return reject_soil_moisture_reading(
            &state,
            &request,
            SoilMoistureRejectionReason::FieldNotFound,
            ingested_at,
        )
        .await;
    };

    let record = match build_soil_moisture_reading(
        request.clone(),
        &field,
        format!("water-moisture-{}", Uuid::new_v4()),
        ingested_at.clone(),
    ) {
        Ok(record) => record,
        Err(error) => {
            return reject_soil_moisture_reading(
                &state,
                &request,
                soil_moisture_rejection_reason_for_error(&error),
                ingested_at,
            )
            .await;
        }
    };

    insert_soil_moisture_reading(&state, &record).await?;
    insert_soil_moisture_time_series_point(&state, &record).await?;

    Ok(Json(record).into_response())
}

pub async fn list_soil_moisture_readings(
    Query(query): Query<SoilMoistureReadingListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SoilMoistureReadingRecord>>> {
    let field_id = normalize_optional_text(query.field_id);
    let zone_ref = normalize_optional_text(query.zone_ref);
    let source = normalize_optional_text(query.source);
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let rows = sqlx::query(
        r#"
        SELECT reading_id, field_id, zone_ref, value, source, captured_at, qa_flag, ingested_at
        FROM water_moisture_readings
        WHERE (?1 IS NULL OR field_id = ?1)
          AND (?2 IS NULL OR zone_ref = ?2)
          AND (?3 IS NULL OR source = ?3)
          AND (?4 IS NULL OR captured_at >= ?4)
          AND (?5 IS NULL OR captured_at <= ?5)
        ORDER BY captured_at ASC, reading_id ASC
        "#,
    )
    .bind(field_id)
    .bind(zone_ref)
    .bind(source)
    .bind(start)
    .bind(end)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_soil_moisture_reading(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn list_soil_moisture_rejections(
    Query(query): Query<SoilMoistureRejectionListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SoilMoistureRejectionRecord>>> {
    let field_id = normalize_optional_text(query.field_id);
    let reason = query.reason.map(|reason| reason.as_str().to_string());
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let rows = sqlx::query(
        r#"
        SELECT rejection_id, reading_id, field_id, zone_ref, source, captured_at, reason, rejected_at
        FROM water_moisture_reading_rejections
        WHERE (?1 IS NULL OR field_id = ?1)
          AND (?2 IS NULL OR reason = ?2)
          AND (?3 IS NULL OR rejected_at >= ?3)
          AND (?4 IS NULL OR rejected_at <= ?4)
        ORDER BY rejected_at ASC, rejection_id ASC
        "#,
    )
    .bind(field_id)
    .bind(reason)
    .bind(start)
    .bind(end)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_soil_moisture_rejection(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

async fn reject_soil_moisture_reading(
    state: &AppState,
    request: &SoilMoistureReadingRequest,
    reason: SoilMoistureRejectionReason,
    rejected_at: String,
) -> AppResult<Response> {
    let rejection = soil_moisture_rejection_record(
        format!("water-moisture-rejection-{}", Uuid::new_v4()),
        request,
        reason,
        rejected_at,
    )
    .map_err(soil_moisture_error)?;
    insert_soil_moisture_rejection(state, &rejection).await?;
    Ok((StatusCode::BAD_REQUEST, Json(rejection)).into_response())
}

pub async fn compute_drought_index_route(
    State(state): State<AppState>,
    Json(request): Json<DroughtIndexComputeRequest>,
) -> AppResult<Json<DroughtIndexRecord>> {
    validate_drought_scope_ref(&state, &request.field_or_region_ref).await?;
    let record = compute_drought_index(
        request,
        format!("drought-index-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(drought_index_error)?;

    insert_drought_index_record(&state, &record, current_record_timestamp()).await?;
    insert_drought_index_time_series_point(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_drought_indices(
    Query(query): Query<DroughtIndexListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<DroughtIndexRecord>>> {
    let field_or_region_ref = normalize_optional_text(query.field_or_region_ref);
    let index_type = query
        .index_type
        .map(|index_type| index_type.as_str().to_string());
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let rows = sqlx::query(
        r#"
        SELECT index_id, field_or_region_ref, index_type, value, period_start, period_end,
               accumulation_days, input_refs_json, method, computed_at
        FROM drought_indices
        WHERE (?1 IS NULL OR field_or_region_ref = ?1)
          AND (?2 IS NULL OR index_type = ?2)
          AND (?3 IS NULL OR period_end >= ?3)
          AND (?4 IS NULL OR period_start <= ?4)
        ORDER BY period_end ASC, index_id ASC
        "#,
    )
    .bind(field_or_region_ref)
    .bind(index_type)
    .bind(start)
    .bind(end)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_drought_index_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn create_marketplace_account(
    State(state): State<AppState>,
    Json(request): Json<MarketplaceAccountCreateRequest>,
) -> AppResult<Json<MarketplaceAccountRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id.clone()))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let org_exists = marketplace_org_exists(&state, &org_id).await?;
    let record = build_marketplace_account_record(
        request,
        org_exists,
        format!("marketplace-account-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(marketplace_account_error)?;
    insert_marketplace_account_record(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_marketplace_accounts(
    Query(query): Query<MarketplaceAccountListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceAccountRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace accounts".to_string(),
        )
    })?;
    let party_type = query
        .party_type
        .map(|party_type| party_type.as_str().to_string());
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT account_id, org_id, party_type, role_refs_json, status, created_at, updated_at
        FROM marketplace_accounts
        WHERE (?1 IS NULL OR org_id = ?1)
          AND (?2 IS NULL OR party_type = ?2)
          AND (?3 IS NULL OR status = ?3)
        ORDER BY created_at ASC, account_id ASC
        "#,
    )
    .bind(org_id)
    .bind(party_type)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_account_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_marketplace_account(
    Path(account_id): Path<String>,
    Query(query): Query<MarketplaceAccountScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceAccountRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let account = load_marketplace_account(&state, &account_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if account.org_id != org_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(account))
}

pub async fn update_marketplace_account_status(
    Path(account_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<MarketplaceAccountStatusRequest>,
) -> AppResult<Json<MarketplaceAccountRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let account = load_marketplace_account(&state, &account_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if account.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let updated =
        transition_marketplace_account_status(&account, request.status, current_record_timestamp())
            .map_err(marketplace_account_error)?;
    update_marketplace_account_record(&state, &updated).await?;

    Ok(Json(updated))
}

pub async fn create_marketplace_catalog_item(
    State(state): State<AppState>,
    Json(request): Json<MarketplaceCatalogItemCreateRequest>,
) -> AppResult<Json<MarketplaceCatalogItemRecord>> {
    let owner_account_id = normalize_optional_text(Some(request.owner_account_id.clone()))
        .ok_or_else(|| {
            AppError::BadRequest("marketplace owner_account_id is required".to_string())
        })?;
    let owner_account = load_marketplace_account(&state, &owner_account_id).await?;
    let record = build_marketplace_catalog_item_record(
        request,
        owner_account.as_ref(),
        format!("marketplace-item-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(marketplace_catalog_error)?;
    insert_marketplace_catalog_item_record(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_marketplace_catalog_items(
    Query(query): Query<MarketplaceCatalogListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceCatalogItemRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace catalog".to_string(),
        )
    })?;
    let kind = query.kind.map(|kind| kind.as_str().to_string());
    let category = query.category.map(|category| category.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT item_id, org_id, kind, category, name, unit_of_measure, owner_account_id, created_at
        FROM marketplace_catalog_items
        WHERE (?1 IS NULL OR org_id = ?1)
          AND (?2 IS NULL OR kind = ?2)
          AND (?3 IS NULL OR category = ?3)
        ORDER BY created_at ASC, item_id ASC
        "#,
    )
    .bind(org_id)
    .bind(kind)
    .bind(category)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_catalog_item_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_marketplace_catalog_item(
    Path(item_id): Path<String>,
    Query(query): Query<MarketplaceCatalogScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceCatalogItemRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let item = load_marketplace_catalog_item(&state, &item_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if item.org_id != org_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(item))
}

pub async fn get_marketplace_portal_entry(
    Query(query): Query<MarketplacePortalEntryQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplacePortalEntry>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let account_id = normalize_optional_text(query.account_id).ok_or_else(|| {
        AppError::BadRequest("account_id query parameter is required".to_string())
    })?;
    let account = load_marketplace_account(&state, &account_id).await?;
    let entry = build_marketplace_portal_entry(account.as_ref(), org_id)
        .map_err(marketplace_portal_entry_error)?;

    Ok(Json(entry))
}

pub async fn publish_marketplace_listing(
    State(state): State<AppState>,
    Json(request): Json<MarketplaceListingPublishRequest>,
) -> AppResult<Json<MarketplaceListingRecord>> {
    let item_id = normalize_optional_text(Some(request.item_id.clone()))
        .ok_or_else(|| AppError::BadRequest("marketplace item_id is required".to_string()))?;
    let catalog_item = load_marketplace_catalog_item(&state, &item_id).await?;
    let record = publish_marketplace_listing_record(
        request,
        catalog_item.as_ref(),
        format!("marketplace-listing-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(marketplace_listing_error)?;
    insert_marketplace_listing_record(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_marketplace_listings(
    Query(query): Query<MarketplaceListingListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceListingRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace listings".to_string(),
        )
    })?;
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT listing_id, item_id, org_id, price, currency, available_qty,
               window_from, window_to, status, created_at, updated_at
        FROM marketplace_listings
        WHERE (?1 IS NULL OR org_id = ?1)
          AND (?2 IS NULL OR status = ?2)
        ORDER BY created_at ASC, listing_id ASC
        "#,
    )
    .bind(org_id)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_listing_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_marketplace_listing(
    Path(listing_id): Path<String>,
    Query(query): Query<MarketplaceListingScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceListingRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let listing = load_marketplace_listing(&state, &listing_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if listing.org_id != org_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(listing))
}

pub async fn close_marketplace_listing(
    Path(listing_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<MarketplaceListingCloseRequest>,
) -> AppResult<Json<MarketplaceListingRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let listing = load_marketplace_listing(&state, &listing_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if listing.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let updated = close_marketplace_listing_record(&listing, current_record_timestamp())
        .map_err(marketplace_listing_error)?;
    update_marketplace_listing_record(&state, &updated).await?;

    Ok(Json(updated))
}

pub async fn upsert_marketplace_inventory(
    State(state): State<AppState>,
    Json(request): Json<MarketplaceInventoryUpsertRequest>,
) -> AppResult<Json<MarketplaceInventoryRecord>> {
    let item_id = normalize_optional_text(Some(request.item_id.clone()))
        .ok_or_else(|| AppError::BadRequest("marketplace item_id is required".to_string()))?;
    let catalog_item = load_marketplace_catalog_item(&state, &item_id).await?;
    let record = build_marketplace_inventory_record(
        request,
        catalog_item.as_ref(),
        format!("marketplace-inventory-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(marketplace_inventory_error)?;
    upsert_marketplace_inventory_record(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_marketplace_inventory(
    Query(query): Query<MarketplaceInventoryListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceInventoryRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace inventory".to_string(),
        )
    })?;
    let rows = sqlx::query(
        r#"
        SELECT inventory_id, item_id, org_id, on_hand, reserved, updated_at
        FROM marketplace_inventory
        WHERE org_id = ?1
        ORDER BY item_id ASC, inventory_id ASC
        "#,
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_inventory_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_marketplace_inventory(
    Path(inventory_id): Path<String>,
    Query(query): Query<MarketplaceInventoryScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceInventoryRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let inventory = load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if inventory.org_id != org_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(inventory))
}

pub async fn reserve_marketplace_inventory_endpoint(
    Path(inventory_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<MarketplaceInventoryAdjustmentRequest>,
) -> AppResult<Json<MarketplaceInventoryRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let inventory = load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if inventory.org_id != org_id {
        return Err(AppError::NotFound);
    }
    reserve_marketplace_inventory(&inventory, request.qty, current_record_timestamp())
        .map_err(marketplace_inventory_error)?;
    update_marketplace_inventory_reserve(&state, &inventory_id, &org_id, request.qty).await?;
    load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn fulfill_marketplace_inventory_endpoint(
    Path(inventory_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<MarketplaceInventoryAdjustmentRequest>,
) -> AppResult<Json<MarketplaceInventoryRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let inventory = load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if inventory.org_id != org_id {
        return Err(AppError::NotFound);
    }
    fulfill_marketplace_inventory(&inventory, request.qty, current_record_timestamp())
        .map_err(marketplace_inventory_error)?;
    update_marketplace_inventory_fulfill(&state, &inventory_id, &org_id, request.qty).await?;
    load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn release_marketplace_inventory_endpoint(
    Path(inventory_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<MarketplaceInventoryAdjustmentRequest>,
) -> AppResult<Json<MarketplaceInventoryRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let inventory = load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if inventory.org_id != org_id {
        return Err(AppError::NotFound);
    }
    release_marketplace_inventory(&inventory, request.qty, current_record_timestamp())
        .map_err(marketplace_inventory_error)?;
    update_marketplace_inventory_release(&state, &inventory_id, &org_id, request.qty).await?;
    load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn place_marketplace_order(
    State(state): State<AppState>,
    Json(request): Json<MarketplaceOrderCreateRequest>,
) -> AppResult<Json<MarketplaceOrderRecord>> {
    let listing_ref = normalize_optional_text(Some(request.listing_ref.clone()))
        .ok_or_else(|| AppError::BadRequest("marketplace listing_ref is required".to_string()))?;
    let buyer_account_id = normalize_optional_text(Some(request.buyer_account_id.clone()))
        .ok_or_else(|| {
            AppError::BadRequest("marketplace buyer_account_id is required".to_string())
        })?;
    let listing = load_marketplace_listing(&state, &listing_ref).await?;
    let buyer_account = load_marketplace_account(&state, &buyer_account_id).await?;
    let (order, audit) = place_marketplace_order_record(
        request,
        listing.as_ref(),
        buyer_account.as_ref(),
        format!("marketplace-order-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(marketplace_order_error)?;
    let listing = listing.ok_or(AppError::NotFound)?;
    let inventory = load_marketplace_inventory_by_item(&state, &listing.item_id, &order.org_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("marketplace inventory is required".to_string()))?;
    reserve_marketplace_inventory(&inventory, order.qty, current_record_timestamp())
        .map_err(marketplace_inventory_error)?;
    update_marketplace_inventory_reserve(&state, &inventory.inventory_id, &order.org_id, order.qty)
        .await?;
    insert_marketplace_order_record(&state, &order).await?;
    insert_marketplace_order_audit_record(&state, &audit).await?;

    Ok(Json(order))
}

pub async fn list_marketplace_orders(
    Query(query): Query<MarketplaceOrderListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceOrderRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace orders".to_string(),
        )
    })?;
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT order_id, org_id, listing_ref, buyer_account_id, qty, line_total,
               status, created_at, updated_at
        FROM marketplace_orders
        WHERE org_id = ?1
          AND (?2 IS NULL OR status = ?2)
        ORDER BY created_at ASC, order_id ASC
        "#,
    )
    .bind(org_id)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_order_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_marketplace_order(
    Path(order_id): Path<String>,
    Query(query): Query<MarketplaceOrderScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceOrderRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let order = load_marketplace_order(&state, &order_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if order.org_id != org_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(order))
}

pub async fn transition_marketplace_order(
    Path(order_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<MarketplaceOrderTransitionRequest>,
) -> AppResult<Json<MarketplaceOrderRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let order = load_marketplace_order(&state, &order_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if order.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let (updated, audit) = transition_marketplace_order_status(
        &order,
        request.status,
        request.actor_id,
        current_record_timestamp(),
    )
    .map_err(marketplace_order_error)?;
    if updated.status == MarketplaceOrderStatus::Fulfilled {
        let listing = load_marketplace_listing(&state, &order.listing_ref)
            .await?
            .ok_or(AppError::NotFound)?;
        let inventory = load_marketplace_inventory_by_item(&state, &listing.item_id, &order.org_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("marketplace inventory is required".to_string()))?;
        update_marketplace_inventory_fulfill(
            &state,
            &inventory.inventory_id,
            &order.org_id,
            order.qty,
        )
        .await?;
    } else if updated.status == MarketplaceOrderStatus::Cancelled {
        let listing = load_marketplace_listing(&state, &order.listing_ref)
            .await?
            .ok_or(AppError::NotFound)?;
        let inventory = load_marketplace_inventory_by_item(&state, &listing.item_id, &order.org_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("marketplace inventory is required".to_string()))?;
        update_marketplace_inventory_release(
            &state,
            &inventory.inventory_id,
            &order.org_id,
            order.qty,
        )
        .await?;
    }
    update_marketplace_order_record(&state, &updated).await?;
    insert_marketplace_order_audit_record(&state, &audit).await?;

    Ok(Json(updated))
}

pub async fn list_marketplace_order_audits(
    Path(order_id): Path<String>,
    Query(query): Query<MarketplaceOrderScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceOrderAuditRecord>>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let order = load_marketplace_order(&state, &order_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if order.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let rows = sqlx::query(
        r#"
        SELECT audit_id, order_id, from_status, to_status, actor_id, occurred_at
        FROM marketplace_order_audits
        WHERE order_id = ?1
        ORDER BY occurred_at ASC, audit_id ASC
        "#,
    )
    .bind(order_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_order_audit_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn create_sustainability_record(
    State(state): State<AppState>,
    Json(request): Json<SustainabilityRecordCreateRequest>,
) -> AppResult<Json<SustainabilityRecord>> {
    let field_id = normalize_optional_text(Some(request.field_id.clone())).unwrap_or_default();
    let linkage = if field_id.is_empty() {
        None
    } else {
        load_sustainability_record_linkage(&state, &field_id).await?
    };
    let record = build_sustainability_record(
        request,
        linkage,
        format!("sustainability-record-{}", Uuid::new_v4()),
        format!("sustainability-audit-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(sustainability_record_error)?;
    insert_sustainability_record(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_sustainability_records(
    Query(query): Query<SustainabilityRecordListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SustainabilityRecord>>> {
    let field_id = normalize_optional_text(query.field_id).ok_or_else(|| {
        AppError::BadRequest(
            "field_id query parameter is required for sustainability records".to_string(),
        )
    })?;
    let season_id = normalize_optional_text(query.season_id);
    let metric_type = query
        .metric_type
        .map(|metric_type| metric_type.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT record_id, field_id, season_id, operation_id, metric_type, method_version,
               created_at, audit_id
        FROM sustainability_records
        WHERE field_id = ?1
          AND (?2 IS NULL OR season_id = ?2)
          AND (?3 IS NULL OR metric_type = ?3)
        ORDER BY created_at ASC, record_id ASC
        "#,
    )
    .bind(field_id)
    .bind(season_id)
    .bind(metric_type)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_sustainability_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_sustainability_record(
    Path(record_id): Path<String>,
    Query(query): Query<SustainabilityRecordScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<SustainabilityRecord>> {
    let field_id = normalize_optional_text(query.field_id)
        .ok_or_else(|| AppError::BadRequest("field_id query parameter is required".to_string()))?;
    let record = load_sustainability_record(&state, &record_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if record.field_id != field_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(record))
}

pub async fn create_content_item(
    State(state): State<AppState>,
    Json(request): Json<ContentCreateRequest>,
) -> AppResult<Json<VersionedContentRecord>> {
    let (content, version) = create_versioned_content(
        request,
        format!("content-{}", Uuid::new_v4()),
        format!("content-version-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(content_error)?;
    insert_content_item_with_version(&state, &content, &version).await?;

    Ok(Json(VersionedContentRecord {
        content,
        versions: vec![version],
    }))
}

pub async fn append_content_item_version(
    Path(content_id): Path<String>,
    Query(query): Query<ContentItemScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<ContentEditRequest>,
) -> AppResult<Json<VersionedContentRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let content = load_content_record(&state, &content_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if content.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let (updated, version) = append_content_version(
        &content,
        request.body,
        format!("content-version-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(content_error)?;
    append_content_version_record(&state, &updated, &version).await?;

    load_versioned_content(&state, &updated.content_id, &org_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn get_content_item(
    Path(content_id): Path<String>,
    Query(query): Query<ContentItemScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<VersionedContentRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;

    load_versioned_content(&state, &content_id, &org_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn list_content_items(
    Query(query): Query<ContentItemListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<ContentRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest("org_id query parameter is required for content items".to_string())
    })?;
    let content_type = query
        .content_type
        .map(|content_type| content_type.as_str().to_string());
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT content_id, content_type, author_id, org_id, status, current_version,
               created_at, updated_at
        FROM cms_contents
        WHERE org_id = ?1
          AND (?2 IS NULL OR content_type = ?2)
          AND (?3 IS NULL OR status = ?3)
        ORDER BY created_at ASC, content_id ASC
        "#,
    )
    .bind(org_id)
    .bind(content_type)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_content_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn create_collaboration_channel(
    State(state): State<AppState>,
    Json(request): Json<CollaborationChannelCreateRequest>,
) -> AppResult<Json<CollaborationChannelRecord>> {
    let channel = build_collaboration_channel(
        request,
        format!("collab-channel-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;
    validate_collaboration_field_ref(&state, &channel.org_id, &channel.field_ref).await?;
    insert_collaboration_channel(&state, &channel).await?;

    Ok(Json(channel))
}

pub async fn list_collaboration_channels(
    Query(query): Query<CollaborationChannelListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<CollaborationChannelRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for collaboration channels".to_string(),
        )
    })?;
    let field_ref = normalize_optional_text(query.field_ref);
    let rows = sqlx::query(
        r#"
        SELECT channel_id, org_id, field_ref, member_account_ids_json, created_at
        FROM collab_channels
        WHERE org_id = ?1
          AND (?2 IS NULL OR field_ref = ?2)
        ORDER BY created_at ASC, channel_id ASC
        "#,
    )
    .bind(org_id)
    .bind(field_ref)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_collaboration_channel(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_collaboration_channel(
    Path(channel_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<CollaborationChannelThread>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;

    load_collaboration_thread(&state, &channel_id, &org_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn post_collaboration_message(
    Path(channel_id): Path<String>,
    Query(query): Query<CollaborationScopeQuery>,
    State(state): State<AppState>,
    Json(request): Json<CollaborationMessageCreateRequest>,
) -> AppResult<Json<CollaborationMessageRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let channel = load_collaboration_channel(&state, &channel_id).await?;
    let Some(channel) = channel else {
        return Err(AppError::BadRequest(format!(
            "collaboration channel {channel_id} does not exist"
        )));
    };
    if channel.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let message = build_collaboration_message(
        request,
        Some(&channel),
        format!("collab-message-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(collaboration_error)?;
    insert_collaboration_message(&state, &message, &channel.org_id).await?;

    Ok(Json(message))
}

pub async fn list_time_series_points(
    Query(query): Query<TimeSeriesPointListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<TimeSeriesPointResponse>>> {
    let entity_ref = normalize_optional_text(query.entity_ref)
        .ok_or_else(|| AppError::BadRequest("entity_ref is required".to_string()))?;
    let metric = normalize_optional_text(query.metric);
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let rows = sqlx::query(
        r#"
        SELECT entity_ref, metric, t, value_kind, scalar_value, source_ref, created_at, metadata_json
        FROM time_series_points
        WHERE entity_ref = ?1
          AND (?2 IS NULL OR metric = ?2)
          AND (?3 IS NULL OR t >= ?3)
          AND (?4 IS NULL OR t <= ?4)
        ORDER BY t ASC, metric ASC, source_ref ASC
        "#,
    )
    .bind(entity_ref)
    .bind(metric)
    .bind(start)
    .bind(end)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_time_series_point_response(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn list_provenance_lineage_records(
    Query(query): Query<ProvenanceLineageListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<ProvenanceLineagePage>> {
    let artifact_id = normalize_optional_text(query.artifact_id);
    let actor_id = normalize_optional_text(query.actor_id);
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM provenance_lineage_records
        WHERE (?1 IS NULL OR artifact_id = ?1)
          AND (?2 IS NULL OR actor_id = ?2)
          AND (?3 IS NULL OR created_at >= ?3)
          AND (?4 IS NULL OR created_at <= ?4)
        "#,
    )
    .bind(&artifact_id)
    .bind(&actor_id)
    .bind(&start)
    .bind(&end)
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    let rows = sqlx::query(
        r#"
        SELECT artifact_id, kind, inputs_json, method, parameters_json, operator, actor_id,
               actor_kind, created_at
        FROM provenance_lineage_records
        WHERE (?1 IS NULL OR artifact_id = ?1)
          AND (?2 IS NULL OR actor_id = ?2)
          AND (?3 IS NULL OR created_at >= ?3)
          AND (?4 IS NULL OR created_at <= ?4)
        ORDER BY created_at DESC, artifact_id ASC
        LIMIT ?5 OFFSET ?6
        "#,
    )
    .bind(artifact_id)
    .bind(actor_id)
    .bind(start)
    .bind(end)
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let records = rows
        .into_iter()
        .map(|row| decode_lineage_record(&row))
        .collect::<AppResult<Vec<_>>>()?;

    Ok(Json(ProvenanceLineagePage {
        page,
        page_size,
        total: total as usize,
        records,
    }))
}

pub async fn get_provenance_lineage_record(
    Path(artifact_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<LineageRecord>> {
    let artifact_id = normalize_optional_text(Some(artifact_id))
        .ok_or_else(|| AppError::BadRequest("artifact_id is required".to_string()))?;
    let row = sqlx::query(
        r#"
        SELECT artifact_id, kind, inputs_json, method, parameters_json, operator, actor_id,
               actor_kind, created_at
        FROM provenance_lineage_records
        WHERE artifact_id = ?1
        "#,
    )
    .bind(artifact_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_lineage_record(&row))
        .transpose()?
        .map(Json)
        .ok_or(AppError::NotFound)
}

pub async fn list_provenance_audit_entries(
    Query(query): Query<ProvenanceAuditListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<ProvenanceAuditPage>> {
    let artifact_id = normalize_optional_text(query.artifact_id);
    let actor_id = normalize_optional_text(query.actor_id);
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM provenance_audit_entries
        WHERE (?1 IS NULL OR artifact_ref = ?1)
          AND (?2 IS NULL OR actor_id = ?2)
          AND (?3 IS NULL OR ts >= ?3)
          AND (?4 IS NULL OR ts <= ?4)
        "#,
    )
    .bind(&artifact_id)
    .bind(&actor_id)
    .bind(&start)
    .bind(&end)
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    let rows = sqlx::query(
        r#"
        SELECT entry_hash, seq, prev_hash, payload_hash, actor_id, actor_kind, ts, action_ref,
               action_kind, artifact_ref, payload_json, occurred_at, outcome, refusal_reason
        FROM provenance_audit_entries
        WHERE (?1 IS NULL OR artifact_ref = ?1)
          AND (?2 IS NULL OR actor_id = ?2)
          AND (?3 IS NULL OR ts >= ?3)
          AND (?4 IS NULL OR ts <= ?4)
        ORDER BY ts DESC, seq DESC
        LIMIT ?5 OFFSET ?6
        "#,
    )
    .bind(artifact_id)
    .bind(actor_id)
    .bind(start)
    .bind(end)
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let entries = rows
        .into_iter()
        .map(|row| decode_audit_entry(&row))
        .collect::<AppResult<Vec<_>>>()?;

    Ok(Json(ProvenanceAuditPage {
        page,
        page_size,
        total: total as usize,
        entries,
    }))
}

pub async fn get_provenance_audit_entry(
    Path(entry_hash): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<AuditEntry>> {
    let entry_hash = normalize_optional_text(Some(entry_hash))
        .ok_or_else(|| AppError::BadRequest("entry_hash is required".to_string()))?;
    let row = sqlx::query(
        r#"
        SELECT entry_hash, seq, prev_hash, payload_hash, actor_id, actor_kind, ts, action_ref,
               action_kind, artifact_ref, payload_json, occurred_at, outcome, refusal_reason
        FROM provenance_audit_entries
        WHERE entry_hash = ?1
        "#,
    )
    .bind(entry_hash)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_audit_entry(&row))
        .transpose()?
        .map(Json)
        .ok_or(AppError::NotFound)
}

pub async fn register_plugin(
    State(state): State<AppState>,
    Json(manifest): Json<RawPluginManifest>,
) -> AppResult<Json<PluginRegistrationRecord>> {
    let mut host = PluginHost::default();
    let record = host
        .register_plugin(manifest)
        .map_err(plugin_registration_error)?;
    insert_plugin_registration(&state, &record, current_record_timestamp()).await?;
    Ok(Json(record))
}

pub async fn list_plugins(
    Query(query): Query<PluginListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<PluginRegistrationPage>> {
    let kind = query.kind.map(|kind| kind.as_str().to_string());
    let status = query.status.map(|status| status.as_str().to_string());
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM plugin_registrations
        WHERE (?1 IS NULL OR kind = ?1)
          AND (?2 IS NULL OR status = ?2)
        "#,
    )
    .bind(&kind)
    .bind(&status)
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    let rows = sqlx::query(
        r#"
        SELECT plugin_id, name, version, kind, host_api_version, capabilities_json, entrypoint,
               status
        FROM plugin_registrations
        WHERE (?1 IS NULL OR kind = ?1)
          AND (?2 IS NULL OR status = ?2)
        ORDER BY updated_at DESC, plugin_id ASC
        LIMIT ?3 OFFSET ?4
        "#,
    )
    .bind(kind)
    .bind(status)
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let plugins = rows
        .into_iter()
        .map(|row| decode_plugin_registration(&row))
        .collect::<AppResult<Vec<_>>>()?;

    Ok(Json(PluginRegistrationPage {
        page,
        page_size,
        total: total as usize,
        plugins,
    }))
}

pub async fn update_plugin_status(
    Path(plugin_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<PluginStatusUpdateRequest>,
) -> AppResult<Json<PluginRegistrationRecord>> {
    let plugin_id = normalize_optional_text(Some(plugin_id))
        .ok_or_else(|| AppError::BadRequest("plugin_id is required".to_string()))?;
    let current = load_plugin_registration(&state, &plugin_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let occurred_at =
        normalize_optional_text(request.occurred_at).unwrap_or_else(current_record_timestamp);
    let actor_kind = request.actor_kind.unwrap_or(ActorKind::PlatformAdmin);
    let mut host =
        PluginHost::with_registration_records(vec![current]).map_err(plugin_registration_error)?;
    let (updated, audit) = host
        .transition_plugin_status(
            &plugin_id,
            PluginLifecycleTransitionRequest {
                status: request.status,
                actor_id: request.actor_id,
                occurred_at,
            },
            format!("plugin-lifecycle-audit-{}", Uuid::new_v4()),
        )
        .map_err(plugin_lifecycle_error)?;

    update_plugin_registration_status(&state, &updated, &audit.occurred_at).await?;
    insert_plugin_lifecycle_audit(&state, &audit).await?;
    append_plugin_lifecycle_provenance_audit(&state, &audit, actor_kind).await?;

    Ok(Json(updated))
}

pub async fn execute_plugin(
    Path(plugin_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<PluginExecutionRequest>,
) -> AppResult<Json<SandboxExecutionOutcome>> {
    let plugin_id = normalize_optional_text(Some(plugin_id))
        .ok_or_else(|| AppError::BadRequest("plugin_id is required".to_string()))?;
    let current = load_plugin_registration(&state, &plugin_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut host =
        PluginHost::with_registration_records(vec![current]).map_err(plugin_registration_error)?;
    let limits = request.limits.unwrap_or(PluginExecutionLimits {
        max_runtime_ms: 1_000,
        max_memory_mb: 512,
    });
    let attempted_at =
        normalize_optional_text(request.attempted_at).unwrap_or_else(current_record_timestamp);
    let outcome = host.execute_sandboxed(
        PluginExecutionPlan {
            plugin_id: plugin_id.clone(),
            required_capabilities: request.required_capabilities,
            estimated_runtime_ms: request.estimated_runtime_ms,
            estimated_memory_mb: request.estimated_memory_mb,
            result: request
                .result
                .unwrap_or_else(|| "plugin execution complete".to_string()),
        },
        limits,
        &attempted_at,
    );
    if outcome.status == SandboxExecutionStatus::Terminated
        && outcome.termination_reason == Some(SandboxTerminationReason::PluginNotEnabled)
    {
        return Err(AppError::Forbidden(format!(
            "plugin {plugin_id} is not enabled"
        )));
    }

    Ok(Json(outcome))
}

pub async fn create_alert_rule(
    State(state): State<AppState>,
    Json(request): Json<AlertRuleCreateRequest>,
) -> AppResult<Json<AlertRuleRecord>> {
    let record = build_alert_rule_record(
        request,
        format!("alert-rule-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(alerting_error)?;
    insert_alert_rule_record(&state, &record).await?;
    Ok(Json(record))
}

pub async fn list_alert_rules(
    Query(query): Query<AlertRuleListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AlertRuleRecord>>> {
    let status = query.status.map(|status| status.as_str().to_string());
    let event_type = normalize_optional_text(query.event_type);
    let include_versions = query.include_versions.unwrap_or(false);
    let rows = if include_versions {
        sqlx::query(
            r#"
            SELECT rule_id, version, event_type, subject_ref, severity, channels_json, status,
                   created_at, updated_at
            FROM alert_rules
            WHERE (?1 IS NULL OR status = ?1)
              AND (?2 IS NULL OR event_type = ?2)
            ORDER BY rule_id ASC, version ASC
            "#,
        )
        .bind(&status)
        .bind(&event_type)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?
    } else {
        sqlx::query(
            r#"
            SELECT rules.rule_id, rules.version, rules.event_type, rules.subject_ref, rules.severity,
                   rules.channels_json, rules.status, rules.created_at, rules.updated_at
            FROM alert_rules AS rules
            JOIN (
                SELECT rule_id, MAX(version) AS version
                FROM alert_rules
                GROUP BY rule_id
            ) AS latest
              ON latest.rule_id = rules.rule_id AND latest.version = rules.version
            WHERE (?1 IS NULL OR rules.status = ?1)
              AND (?2 IS NULL OR rules.event_type = ?2)
            ORDER BY rules.updated_at DESC, rules.rule_id ASC
            "#,
        )
        .bind(&status)
        .bind(&event_type)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?
    };

    rows.into_iter()
        .map(|row| decode_alert_rule_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_alert_rule_versions(
    Path(rule_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AlertRuleRecord>>> {
    let rule_id = normalize_optional_text(Some(rule_id))
        .ok_or_else(|| AppError::BadRequest("rule_id is required".to_string()))?;
    let rows = sqlx::query(
        r#"
        SELECT rule_id, version, event_type, subject_ref, severity, channels_json, status,
               created_at, updated_at
        FROM alert_rules
        WHERE rule_id = ?1
        ORDER BY version ASC
        "#,
    )
    .bind(rule_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;
    if rows.is_empty() {
        return Err(AppError::NotFound);
    }

    rows.into_iter()
        .map(|row| decode_alert_rule_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn update_alert_rule(
    Path(rule_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<AlertRuleUpdateRequest>,
) -> AppResult<Json<AlertRuleRecord>> {
    let rule_id = normalize_optional_text(Some(rule_id))
        .ok_or_else(|| AppError::BadRequest("rule_id is required".to_string()))?;
    let current = load_latest_alert_rule(&state, &rule_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let updated = version_alert_rule_record(&current, request, current_record_timestamp())
        .map_err(alerting_error)?;
    insert_alert_rule_record(&state, &updated).await?;
    Ok(Json(updated))
}

pub async fn update_alert_rule_status(
    Path(rule_id): Path<String>,
    State(state): State<AppState>,
    Json(mut request): Json<AlertRuleStatusUpdateRequest>,
) -> AppResult<Json<AlertRuleRecord>> {
    let rule_id = normalize_optional_text(Some(rule_id))
        .ok_or_else(|| AppError::BadRequest("rule_id is required".to_string()))?;
    let current = load_latest_alert_rule(&state, &rule_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if normalize_optional_text(Some(request.occurred_at.clone())).is_none() {
        request.occurred_at = current_record_timestamp();
    }
    let (updated, audit) = transition_alert_rule_status(
        &current,
        request,
        format!("alert-rule-audit-{}", Uuid::new_v4()),
    )
    .map_err(alerting_error)?;
    insert_alert_rule_record(&state, &updated).await?;
    insert_alert_rule_audit(&state, &audit).await?;
    Ok(Json(updated))
}

pub async fn create_alert_rule_subscription(
    Path(rule_id): Path<String>,
    State(state): State<AppState>,
    Json(mut request): Json<AlertRuleSubscriptionCreateRequest>,
) -> AppResult<Json<AlertRuleSubscriptionRecord>> {
    let rule_id = normalize_optional_text(Some(rule_id))
        .ok_or_else(|| AppError::BadRequest("rule_id is required".to_string()))?;
    if let Some(body_rule_id) = normalize_optional_text(Some(request.rule_id.clone())) {
        if body_rule_id != rule_id {
            return Err(AppError::BadRequest(format!(
                "request rule_id {} does not match path rule_id {}",
                body_rule_id, rule_id
            )));
        }
    }
    let rule = load_latest_alert_rule(&state, &rule_id)
        .await?
        .ok_or(AppError::NotFound)?;
    request.rule_id = rule_id;
    let subscription = build_alert_rule_subscription(
        request,
        &rule,
        format!("alert-rule-subscription-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(alerting_error)?;
    insert_alert_rule_subscription(&state, &subscription).await?;
    Ok(Json(subscription))
}

pub async fn list_alert_rule_subscriptions(
    Path(rule_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AlertRuleSubscriptionRecord>>> {
    let rule_id = normalize_optional_text(Some(rule_id))
        .ok_or_else(|| AppError::BadRequest("rule_id is required".to_string()))?;
    load_latest_alert_rule(&state, &rule_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let rows = sqlx::query(
        r#"
        SELECT subscription_id, rule_id, recipient_id, recipient_role, channels_json, created_at
        FROM alert_rule_subscriptions
        WHERE rule_id = ?1
        ORDER BY created_at ASC, subscription_id ASC
        "#,
    )
    .bind(rule_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_alert_rule_subscription(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn store_fired_alert(
    State(state): State<AppState>,
    Json(record): Json<FiredAlertRecord>,
) -> AppResult<Json<FiredAlertRecord>> {
    let record = normalize_fired_alert_record(record).map_err(alerting_error)?;
    insert_fired_alert_record(&state, &record).await?;
    Ok(Json(record))
}

pub async fn list_fired_alerts(
    Query(query): Query<AlertHistoryListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<AlertHistoryPage>> {
    let source_domain = normalize_optional_text(query.source_domain);
    let field_id = normalize_optional_text(query.field_id);
    let severity = query.severity.map(|severity| severity.as_str().to_string());
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM alert_fired_alerts
        WHERE (?1 IS NULL OR source_domain = ?1)
          AND (?2 IS NULL OR field_id = ?2)
          AND (?3 IS NULL OR severity = ?3)
          AND (?4 IS NULL OR fired_at >= ?4)
          AND (?5 IS NULL OR fired_at <= ?5)
        "#,
    )
    .bind(&source_domain)
    .bind(&field_id)
    .bind(&severity)
    .bind(&start)
    .bind(&end)
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    let rows = sqlx::query(
        r#"
        SELECT alert_id, matched_rule_id, source_event_ref, source_domain, event_type, subject_ref,
               field_id, evidence_refs_json, severity, channels_json, fired_at, explanation
        FROM alert_fired_alerts
        WHERE (?1 IS NULL OR source_domain = ?1)
          AND (?2 IS NULL OR field_id = ?2)
          AND (?3 IS NULL OR severity = ?3)
          AND (?4 IS NULL OR fired_at >= ?4)
          AND (?5 IS NULL OR fired_at <= ?5)
        ORDER BY fired_at DESC, alert_id ASC
        LIMIT ?6 OFFSET ?7
        "#,
    )
    .bind(source_domain)
    .bind(field_id)
    .bind(severity)
    .bind(start)
    .bind(end)
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let alerts = rows
        .into_iter()
        .map(|row| decode_fired_alert_record(&row))
        .collect::<AppResult<Vec<_>>>()?;

    Ok(Json(AlertHistoryPage {
        page,
        page_size,
        total: total as usize,
        alerts,
    }))
}

pub async fn get_fired_alert(
    Path(alert_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<FiredAlertRecord>> {
    let alert_id = normalize_optional_text(Some(alert_id))
        .ok_or_else(|| AppError::BadRequest("alert_id is required".to_string()))?;
    let row = sqlx::query(
        r#"
        SELECT alert_id, matched_rule_id, source_event_ref, source_domain, event_type, subject_ref,
               field_id, evidence_refs_json, severity, channels_json, fired_at, explanation
        FROM alert_fired_alerts
        WHERE alert_id = ?1
        "#,
    )
    .bind(alert_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_fired_alert_record(&row))
        .transpose()?
        .map(Json)
        .ok_or(AppError::NotFound)
}

pub async fn ingest_orthomosaic_frame_set(
    State(state): State<AppState>,
    Json(request): Json<FrameSetIngestRequest>,
) -> AppResult<Json<FrameSetRecord>> {
    validate_orthomosaic_linkage(
        &state,
        &request.scene_id,
        &request.field_id,
        &request.season_id,
    )
    .await?;
    let record = build_frame_set_record(
        request,
        Uuid::new_v4().to_string(),
        current_record_timestamp(),
    )
    .map_err(orthomosaic_ingest_error)?;
    let frames_json =
        serde_json::to_string(&record.frames).map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO orthomosaic_frame_sets
            (frame_set_id, scene_id, field_id, season_id, frames_json, crs_hint, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&record.frame_set_id)
    .bind(&record.scene_id)
    .bind(&record.field_id)
    .bind(&record.season_id)
    .bind(frames_json)
    .bind(&record.crs_hint)
    .bind(&record.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(record))
}

pub async fn list_orthomosaic_frame_sets(
    Query(query): Query<OrthomosaicFrameSetListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FrameSetRecord>>> {
    let scene_id = normalize_optional_text(query.scene_id);
    let field_id = normalize_optional_text(query.field_id);
    let rows = match (scene_id, field_id) {
        (Some(scene_id), Some(field_id)) => sqlx::query(
            r#"
            SELECT frame_set_id, scene_id, field_id, season_id, frames_json, crs_hint, created_at
            FROM orthomosaic_frame_sets
            WHERE scene_id = ?1 AND field_id = ?2
            ORDER BY created_at DESC, frame_set_id ASC
            "#,
        )
        .bind(scene_id)
        .bind(field_id)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?,
        (Some(scene_id), None) => sqlx::query(
            r#"
            SELECT frame_set_id, scene_id, field_id, season_id, frames_json, crs_hint, created_at
            FROM orthomosaic_frame_sets
            WHERE scene_id = ?1
            ORDER BY created_at DESC, frame_set_id ASC
            "#,
        )
        .bind(scene_id)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?,
        (None, Some(field_id)) => sqlx::query(
            r#"
            SELECT frame_set_id, scene_id, field_id, season_id, frames_json, crs_hint, created_at
            FROM orthomosaic_frame_sets
            WHERE field_id = ?1
            ORDER BY created_at DESC, frame_set_id ASC
            "#,
        )
        .bind(field_id)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?,
        (None, None) => sqlx::query(
            r#"
            SELECT frame_set_id, scene_id, field_id, season_id, frames_json, crs_hint, created_at
            FROM orthomosaic_frame_sets
            ORDER BY created_at DESC, frame_set_id ASC
            "#,
        )
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?,
    };

    rows.into_iter()
        .map(|row| decode_orthomosaic_frame_set_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn submit_orthomosaic_reconstruction(
    State(state): State<AppState>,
    Json(request): Json<ReconstructionJobRequest>,
) -> AppResult<Json<ReconstructionJobRecord>> {
    let record = build_reconstruction_job(
        request,
        Uuid::new_v4().to_string(),
        current_record_timestamp(),
    )
    .map_err(reconstruction_job_error)?;
    if !orthomosaic_frame_set_exists(&state, &record.frame_set_id).await? {
        return Err(AppError::BadRequest(format!(
            "frame_set_id {} does not exist",
            record.frame_set_id
        )));
    }
    let params_json =
        serde_json::to_string(&record.params).map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO orthomosaic_reconstructions
            (recon_id, frame_set_id, params_json, status, failure_reason, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&record.recon_id)
    .bind(&record.frame_set_id)
    .bind(params_json)
    .bind(record.status.as_str())
    .bind(&record.failure_reason)
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(record))
}

pub async fn get_orthomosaic_reconstruction(
    Path(recon_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<ReconstructionJobRecord>> {
    let record = load_orthomosaic_reconstruction(&state, &recon_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(record))
}

pub async fn update_orthomosaic_reconstruction_status(
    Path(recon_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<UpdateReconstructionStatusRequest>,
) -> AppResult<Json<ReconstructionJobRecord>> {
    let record = load_orthomosaic_reconstruction(&state, &recon_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let updated = transition_reconstruction_status(
        record,
        request.status,
        request.failure_reason,
        current_record_timestamp(),
    )
    .map_err(reconstruction_job_error)?;

    sqlx::query(
        r#"
        UPDATE orthomosaic_reconstructions
        SET status = ?2, failure_reason = ?3, updated_at = ?4
        WHERE recon_id = ?1
        "#,
    )
    .bind(&updated.recon_id)
    .bind(updated.status.as_str())
    .bind(&updated.failure_reason)
    .bind(&updated.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(updated))
}

pub async fn handoff_orthomosaic_tiles(
    Path(recon_id): Path<String>,
    State(state): State<AppState>,
    Json(mut request): Json<TiledOutputHandoffRequest>,
) -> AppResult<Json<TiledOutputHandoff>> {
    let record = load_orthomosaic_reconstruction(&state, &recon_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if record.status != ReconstructionStatus::Completed {
        return Err(AppError::BadRequest(format!(
            "reconstruction {recon_id} must be completed before tiled handoff"
        )));
    }
    let frame_set = load_orthomosaic_frame_set(&state, &record.frame_set_id)
        .await?
        .ok_or(AppError::NotFound)?;
    request.recon_id = recon_id;
    if normalize_optional_text(Some(request.scene_id.clone())).as_deref()
        != Some(frame_set.scene_id.as_str())
    {
        return Err(AppError::BadRequest(format!(
            "handoff scene_id must match frame set scene_id {}",
            frame_set.scene_id
        )));
    }

    let handoff = build_tiled_output_handoff(request).map_err(tiled_output_handoff_error)?;
    if handoff.recon_id != record.recon_id {
        return Err(AppError::BadRequest(format!(
            "handoff recon_id must match reconstruction {}",
            record.recon_id
        )));
    }

    for layer in &handoff.layers {
        let product_path = PathBuf::from(&layer.uri);
        let exists = fs::try_exists(&product_path)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
        if !exists {
            return Err(AppError::BadRequest(format!(
                "product {} output path does not exist: {}",
                layer.product_kind,
                product_path.display()
            )));
        }
        publish_georeferenced_product(
            &state.pool,
            &handoff.scene_id,
            &frame_set.field_id,
            &frame_set.season_id,
            &layer.product_kind,
            &product_path,
            &layer.spatial_ref,
            layer.width_px,
            layer.height_px,
            layer.gsd_m_per_px,
            handoff.source_image_ids.clone(),
            handoff.source_image_ids.clone(),
        )
        .await
        .map_err(|err| {
            if is_product_publish_error(&err) {
                AppError::BadRequest(err.to_string())
            } else {
                AppError::Anyhow(err)
            }
        })?;
    }

    Ok(Json(handoff))
}

pub async fn apply_orthomosaic_publish_gate(
    Path((scene_id, kind)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(request): Json<MosaicPublishGateRequest>,
) -> AppResult<Json<MosaicPublishGateDecision>> {
    let scene_id = normalize_optional_text(Some(scene_id))
        .ok_or_else(|| AppError::BadRequest("scene_id is required".to_string()))?;
    let kind = normalize_optional_text(Some(kind))
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| AppError::BadRequest("product kind is required".to_string()))?;
    let product_exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM products WHERE scene_id = ?1 AND lower(kind) = lower(?2)",
    )
    .bind(&scene_id)
    .bind(&kind)
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;
    if product_exists == 0 {
        return Err(AppError::NotFound);
    }

    let decision = evaluate_mosaic_publish_gate(request).map_err(mosaic_publish_gate_error)?;
    if decision.scene_id != scene_id || decision.product_kind != kind {
        return Err(AppError::BadRequest(format!(
            "publish gate request must target product {scene_id}:{kind}"
        )));
    }
    let downstream_consumers_json = serde_json::to_string(&decision.downstream_consumers)
        .map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        UPDATE products
        SET publish_status = ?3,
            qa_report_ref = ?4,
            provenance_hash = ?5,
            downstream_consumers_json = ?6
        WHERE scene_id = ?1 AND lower(kind) = lower(?2)
        "#,
    )
    .bind(&scene_id)
    .bind(&kind)
    .bind(decision.status.as_str())
    .bind(&decision.qa_report_ref)
    .bind(&decision.provenance_hash)
    .bind(downstream_consumers_json)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(decision))
}

pub async fn start_copilot_conversation_handler(
    State(state): State<AppState>,
    Json(request): Json<CopilotConversationStartRequest>,
) -> AppResult<Json<CopilotConversationRecord>> {
    let conversation = start_copilot_conversation(
        request,
        format!("copilot-conversation-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(copilot_conversation_error)?;
    assert_copilot_field_exists(&state, &conversation.field_id).await?;
    insert_copilot_conversation(&state, &conversation).await?;

    Ok(Json(conversation))
}

pub async fn list_copilot_conversations(
    Query(query): Query<CopilotConversationListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<CopilotConversationRecord>>> {
    let field_id = normalize_optional_text(query.field_id)
        .ok_or_else(|| AppError::BadRequest("field_id query parameter is required".to_string()))?;
    assert_copilot_field_exists(&state, &field_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT conversation_id, field_id, created_at
        FROM copilot_conversations
        WHERE field_id = ?1
        ORDER BY created_at ASC, conversation_id ASC
        "#,
    )
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_copilot_conversation(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn create_copilot_turn_handler(
    Path(conversation_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CopilotTurnCreateRequest>,
) -> AppResult<Json<CopilotTurnRecord>> {
    let conversation = load_copilot_conversation(&state, &conversation_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let turn = create_copilot_turn(
        &conversation,
        request,
        format!("copilot-turn-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(copilot_conversation_error)?;
    insert_copilot_turn(&state, &turn).await?;

    Ok(Json(turn))
}

pub async fn list_copilot_turns(
    Path(conversation_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<CopilotTurnRecord>>> {
    load_copilot_conversation(&state, &conversation_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let rows = sqlx::query(
        r#"
        SELECT conversation_id, field_id, turn_id, role, created_at
        FROM copilot_turns
        WHERE conversation_id = ?1
        ORDER BY created_at ASC, rowid ASC
        "#,
    )
    .bind(conversation_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_copilot_turn(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn register_crop_model(
    State(state): State<AppState>,
    Json(request): Json<ModelVersionRegistrationRequest>,
) -> AppResult<Json<ModelVersionRecord>> {
    let record = build_model_version_record(request, current_record_timestamp())
        .map_err(crop_model_registry_error)?;
    let metrics_json =
        serde_json::to_string(&record.metrics).map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO crop_models
            (model_id, version, task, training_set_ref, metrics_json, provenance_ref, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&record.model_id)
    .bind(&record.version)
    .bind(record.task.as_str())
    .bind(&record.training_set_ref)
    .bind(metrics_json)
    .bind(&record.provenance_ref)
    .bind(&record.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(record))
}

pub async fn list_crop_models(
    Query(query): Query<CropModelListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<ModelVersionRecord>>> {
    let task = normalize_optional_text(query.task);
    let rows = if let Some(task) = task {
        let task = parse_crop_model_task(task)?;
        sqlx::query(
            r#"
            SELECT model_id, version, task, training_set_ref, metrics_json, provenance_ref, created_at
            FROM crop_models
            WHERE task = ?1
            ORDER BY created_at DESC, model_id ASC, version ASC
            "#,
        )
        .bind(task.as_str())
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?
    } else {
        sqlx::query(
            r#"
            SELECT model_id, version, task, training_set_ref, metrics_json, provenance_ref, created_at
            FROM crop_models
            ORDER BY created_at DESC, model_id ASC, version ASC
            "#,
        )
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?
    };

    rows.into_iter()
        .map(|row| decode_crop_model_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn validate_crop_model_for_inference(
    State(state): State<AppState>,
    Json(reference): Json<InferenceModelReference>,
) -> AppResult<Json<ModelGateResponse>> {
    let model_id = reference.model_id.trim().to_string();
    let version = reference.version.trim().to_string();
    let registered = crop_model_exists(&state, &model_id, &version).await?;
    match validate_model_reference(reference, registered) {
        Ok(response) => Ok(Json(response)),
        Err(CropModelRegistryError::UnregisteredModel { model_id, version }) => {
            audit_crop_model_event(
                &state,
                &model_id,
                &version,
                "unregistered_model_rejected",
                Some("inference request rejected because model version is not registered"),
            )
            .await?;
            Err(AppError::BadRequest(format!(
                "unregistered model {model_id}@{version}"
            )))
        }
        Err(error) => Err(crop_model_registry_error(error)),
    }
}

pub async fn submit_crop_inference_run(
    State(state): State<AppState>,
    Json(request): Json<InferenceRunSubmissionRequest>,
) -> AppResult<Json<InferenceRunRecord>> {
    let model_registered = if let Some(model) = request.model.as_ref() {
        Some(crop_model_exists(&state, model.model_id.trim(), model.version.trim()).await?)
    } else {
        None
    };
    let record = match build_inference_run_record(
        request,
        format!("crop-inference-run-{}", Uuid::new_v4()),
        current_record_timestamp(),
        model_registered,
    ) {
        Ok(record) => record,
        Err(InferenceRunError::ModelGate {
            source: CropModelRegistryError::UnregisteredModel { model_id, version },
        }) => {
            audit_crop_model_event(
                &state,
                &model_id,
                &version,
                "unregistered_model_rejected",
                Some("inference run rejected because model version is not registered"),
            )
            .await?;
            return Err(AppError::BadRequest(format!(
                "unregistered model {model_id}@{version}"
            )));
        }
        Err(error) => return Err(crop_inference_run_error(error)),
    };
    if !crop_inference_mosaic_is_published(
        &state,
        &record.mosaic_ref,
        &record.field_id,
        &record.season_id,
    )
    .await?
    {
        return Err(AppError::BadRequest(format!(
            "mosaic {} is not published and provenance-gated for field {} season {}",
            record.mosaic_ref, record.field_id, record.season_id
        )));
    }
    insert_crop_inference_run(&state, &record).await?;

    Ok(Json(record))
}

pub async fn get_crop_inference_run(
    Path(run_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<InferenceRunRecord>> {
    load_crop_inference_run(&state, &run_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn get_crop_inference_run_result(
    Path(run_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<InferenceRunRecord>> {
    let record = load_crop_inference_run(&state, &run_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if record.status != InferenceRunStatus::Completed {
        return Err(AppError::BadRequest(format!(
            "inference run {run_id} has not completed"
        )));
    }

    Ok(Json(record))
}

pub async fn update_crop_inference_run_status(
    Path(run_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<UpdateCropInferenceRunStatusRequest>,
) -> AppResult<Json<InferenceRunRecord>> {
    let record = load_crop_inference_run(&state, &run_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let updated = transition_inference_run_status(
        record,
        request.status,
        request.failure_reason_code,
        current_record_timestamp(),
    )
    .map_err(crop_inference_run_error)?;
    update_crop_inference_run(&state, &updated).await?;

    Ok(Json(updated))
}

pub async fn verify_crop_detection(
    Path(detection_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<VerifyCropDetectionRequest>,
) -> AppResult<Json<CropDetectionVerificationRecord>> {
    let record = apply_detection_verification(CropDetectionVerificationRequest {
        detection_id,
        task: request.task,
        label: request.label,
        confidence: request.confidence,
        evidence_tile_refs: request.evidence_tile_refs,
        zone_geometry: request.zone_geometry,
        action: request.action,
        actor: request.actor,
        verified_at: request.verified_at,
        corrected_label: request.corrected_label,
        corrected_geometry: request.corrected_geometry,
    })
    .map_err(crop_detection_verification_error)?;

    persist_crop_detection_verification(&state, &record).await?;

    Ok(Json(record))
}

pub async fn validate_crop_detection_finding_promotion(
    Path(detection_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CropFindingPromotionValidationRequest>,
) -> AppResult<Json<FindingPromotionDecision>> {
    let verification_state = load_crop_detection_verification_state(&state, &detection_id)
        .await?
        .unwrap_or_default();
    let decision = validate_detection_finding_promotion(FindingPromotionRequest {
        detection_id,
        verification_state,
        allow_unverified: request.allow_unverified,
    })
    .map_err(finding_promotion_error)?;

    Ok(Json(decision))
}

pub async fn emit_crop_detection_finding(
    Path((scene_id, detection_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(request): Json<EmitCropDetectionFindingRequest>,
) -> AppResult<Json<RecommendationRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }
    let field_id = load_scene_field_id(&state, &scene_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "scene {scene_id} must be linked to a field before emitting crop findings"
            ))
        })?;
    let detection = load_crop_detection_verification_record(&state, &detection_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "verified detection {detection_id} was not found for finding emission"
            ))
        })?;
    let finding = assemble_detection_finding(CropDetectionFindingRequest {
        finding_id: request.finding_id,
        field_id,
        zone_id: request.zone_id,
        detection,
        model: InferenceModelReference {
            model_id: request.model_id,
            version: request.version,
        },
        emitted_at: request.emitted_at,
    })
    .map_err(crop_detection_finding_error)?;

    let annotation = annotation_from_crop_detection_finding(&scene_id, &finding)?;
    let recommendation =
        recommendation_from_crop_detection_finding(&scene_id, &finding, &annotation);
    persist_crop_detection_finding_recommendation(&state, &annotation, &recommendation).await?;

    Ok(Json(recommendation))
}

pub async fn create_compliance_record(
    State(state): State<AppState>,
    Json(request): Json<CreateComplianceRecordRequest>,
) -> AppResult<Json<ComplianceRecord>> {
    let record = build_initial_compliance_record(
        request,
        format!("compliance-record-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(compliance_record_error)?;

    assert_field_owned_by_org(&state, &record.org_id, &record.field_id).await?;
    insert_compliance_record(&state, &record).await?;
    audit_compliance_record_event(
        &state,
        &record.record_id,
        "record_created",
        Some(&record.actor),
        Some("initial compliance record version created"),
    )
    .await?;

    Ok(Json(record))
}

pub async fn list_compliance_records(
    Query(query): Query<ComplianceRecordListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<ComplianceRecord>>> {
    let record_id = normalize_optional_text(query.record_id);
    let org_id = normalize_optional_text(query.org_id);
    let field_id = normalize_optional_text(query.field_id);
    let record_type = normalize_optional_text(query.record_type)
        .map(parse_compliance_record_type)
        .transpose()?
        .map(|record_type| record_type.as_str().to_string());

    let rows = sqlx::query(
        r#"
        SELECT record_id, version, record_type, org_id, field_id, flight_id, created_at,
               actor, provenance_ref, prior_version, change_reason, payload_json
        FROM compliance_records
        WHERE (?1 IS NULL OR record_id = ?1)
          AND (?2 IS NULL OR record_type = ?2)
          AND (?3 IS NULL OR org_id = ?3)
          AND (?4 IS NULL OR field_id = ?4)
        ORDER BY record_id ASC, version ASC
        "#,
    )
    .bind(record_id)
    .bind(record_type)
    .bind(org_id)
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_compliance_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn append_compliance_record_version_route(
    Path(record_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<AppendComplianceRecordVersionRequest>,
) -> AppResult<Json<ComplianceRecord>> {
    let record_id = normalize_optional_text(Some(record_id))
        .ok_or_else(|| AppError::BadRequest("record_id is required".to_string()))?;
    let latest = load_latest_compliance_record(&state, &record_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let record = append_compliance_record_version(&latest, request, current_record_timestamp())
        .map_err(compliance_record_error)?;

    assert_field_owned_by_org(&state, &record.org_id, &record.field_id).await?;
    insert_compliance_record(&state, &record).await?;
    audit_compliance_record_event(
        &state,
        &record.record_id,
        "version_appended",
        Some(&record.actor),
        record.change_reason.as_deref(),
    )
    .await?;

    Ok(Json(record))
}

pub async fn refuse_delete_compliance_record(
    Path(record_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<StatusCode> {
    let record_id = normalize_optional_text(Some(record_id))
        .ok_or_else(|| AppError::BadRequest("record_id is required".to_string()))?;
    let latest = load_latest_compliance_record(&state, &record_id)
        .await?
        .ok_or(AppError::NotFound)?;
    audit_compliance_record_event(
        &state,
        &record_id,
        "delete_refused",
        Some(&latest.actor),
        Some("delete refused because compliance records are append-only"),
    )
    .await?;

    Err(compliance_record_error(refuse_in_place_mutation("delete")))
}

pub async fn export_compliance_audit_report(
    State(state): State<AppState>,
    Json(request): Json<ComplianceAuditReportExportRequest>,
) -> AppResult<Json<ComplianceAuditReport>> {
    let records =
        load_compliance_records_for_report(&state, &request.org_id, &request.field_id).await?;
    let mandatory_record_types = if request.mandatory_record_types.is_empty() {
        default_compliance_report_mandatory_types()
    } else {
        request.mandatory_record_types
    };
    let report = build_compliance_audit_report(ComplianceAuditReportRequest {
        report_id: request
            .report_id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("compliance-report-{}", Uuid::new_v4())),
        org_id: request.org_id,
        field_id: request.field_id,
        generated_at: request
            .generated_at
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(current_record_timestamp),
        records,
        mandatory_record_types,
    })
    .map_err(compliance_audit_report_error)?;

    Ok(Json(report))
}

pub async fn ingest_airspace_zone(
    State(state): State<AppState>,
    Json(request): Json<AirspaceZoneIngestRequest>,
) -> AppResult<Json<AirspaceZoneRecord>> {
    let record = build_airspace_zone_record(
        request,
        format!("airspace-zone-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(airspace_zone_error)?;
    insert_airspace_zone(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_airspace_zones(
    Query(query): Query<AirspaceZoneListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AirspaceZoneRecord>>> {
    let zone_id = normalize_optional_text(query.zone_id);
    let zone_class = normalize_optional_text(query.zone_class)
        .map(parse_airspace_zone_class)
        .transpose()?
        .map(|zone_class| zone_class.as_str().to_string());

    let rows = sqlx::query(
        r#"
        SELECT zone_id, zone_class, crs, geometry_json, min_lon, min_lat, max_lon, max_lat,
               effective_from, effective_to, source, created_at
        FROM compliance_airspace_zones
        WHERE (?1 IS NULL OR zone_id = ?1)
          AND (?2 IS NULL OR zone_class = ?2)
        ORDER BY zone_id ASC
        "#,
    )
    .bind(zone_id)
    .bind(zone_class)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_airspace_zone(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn query_airspace_zones_for_point(
    Query(query): Query<AirspaceZonePointQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AirspaceZoneRecord>>> {
    let point = validate_airspace_query_point(query.longitude, query.latitude)?;
    let at = normalize_optional_text(query.at);
    let rows = sqlx::query(
        r#"
        SELECT zone_id, zone_class, crs, geometry_json, min_lon, min_lat, max_lon, max_lat,
               effective_from, effective_to, source, created_at
        FROM compliance_airspace_zones
        WHERE min_lon <= ?1
          AND max_lon >= ?1
          AND min_lat <= ?2
          AND max_lat >= ?2
        ORDER BY zone_id ASC
        "#,
    )
    .bind(point.longitude)
    .bind(point.latitude)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let zones = rows
        .into_iter()
        .map(|row| decode_airspace_zone(&row))
        .collect::<AppResult<Vec<_>>>()?
        .into_iter()
        .filter(|zone| airspace_zone_is_effective_at(zone, at.as_deref()))
        .filter(|zone| airspace_zone_contains_point(zone, point))
        .collect::<Vec<_>>();

    Ok(Json(zones))
}

fn farm_field_page_window(
    query: &FarmFieldListQuery,
) -> (FarmFieldEntityStatus, usize, usize, i64, i64) {
    let status = query.status.unwrap_or_default();
    let page = query.normalized_page();
    let page_size = query.normalized_page_size();
    let offset = page.saturating_sub(1).saturating_mul(page_size);
    (
        status,
        page,
        page_size,
        i64::try_from(page_size).unwrap_or(i64::MAX),
        i64::try_from(offset).unwrap_or(i64::MAX),
    )
}

fn farm_field_list_page<T>(
    items: Vec<T>,
    total_count: i64,
    page: usize,
    page_size: usize,
) -> FarmFieldListPage<T> {
    FarmFieldListPage {
        items,
        total_count: usize::try_from(total_count).unwrap_or(usize::MAX),
        page,
        page_size,
    }
}

pub async fn list_farms(
    Query(query): Query<FarmFieldApiListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<FarmFieldListPage<FarmRecord>>> {
    let org_filter = query.org_filter();
    let list_query = query.list_query();
    let (status, page, page_size, limit, offset) = farm_field_page_window(&list_query);

    let total_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM farms WHERE (?1 IS NULL OR owner = ?1) AND status = ?2",
    )
    .bind(&org_filter)
    .bind(status.as_str())
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    let rows = sqlx::query(
        r#"
        SELECT farm_id, owner, name, notes, status, created_at,
               COALESCE(NULLIF(updated_at, ''), created_at) AS updated_at
        FROM farms
        WHERE (?1 IS NULL OR owner = ?1) AND status = ?2
        ORDER BY name ASC, farm_id ASC
        LIMIT ?3 OFFSET ?4
        "#,
    )
    .bind(&org_filter)
    .bind(status.as_str())
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let farms = rows
        .into_iter()
        .map(|row| decode_farm_record(&row))
        .collect::<Vec<_>>();

    Ok(Json(farm_field_list_page(
        farms,
        total_count,
        page,
        page_size,
    )))
}

pub async fn create_farm(
    State(state): State<AppState>,
    Json(request): Json<CreateFarmRequest>,
) -> AppResult<Json<FarmRecord>> {
    let farm = build_farm_record(request)?;

    sqlx::query(
        r#"
        INSERT INTO farms (farm_id, owner, name, notes, status, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&farm.farm_id)
    .bind(&farm.owner)
    .bind(&farm.name)
    .bind(&farm.notes)
    .bind(farm.status.as_str())
    .bind(&farm.created_at)
    .bind(&farm.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(farm))
}

pub async fn get_farm(
    Path(farm_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<FarmRecord>> {
    let farm = load_farm(&state, &farm_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(farm))
}

pub async fn update_farm(
    Path(farm_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<UpdateFarmRequest>,
) -> AppResult<Json<FarmRecord>> {
    let mut farm = load_farm(&state, &farm_id)
        .await?
        .ok_or(AppError::NotFound)?;
    farm.name = normalize_farm_name(request.name)?;
    farm.notes = normalize_optional_text(request.notes);
    farm.updated_at = current_record_timestamp();

    sqlx::query(
        r#"
        UPDATE farms
        SET name = ?2, notes = ?3, updated_at = ?4
        WHERE farm_id = ?1
        "#,
    )
    .bind(&farm.farm_id)
    .bind(&farm.name)
    .bind(&farm.notes)
    .bind(&farm.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(farm))
}

pub async fn delete_farm(
    Path(farm_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<StatusCode> {
    if load_farm(&state, &farm_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    let updated_at = current_record_timestamp();
    sqlx::query("UPDATE fields SET farm_id = NULL, updated_at = ?2 WHERE farm_id = ?1")
        .bind(&farm_id)
        .bind(&updated_at)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;
    sqlx::query("DELETE FROM farms WHERE farm_id = ?1")
        .bind(&farm_id)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_farm_fields(
    Path(farm_id): Path<String>,
    Query(query): Query<FarmFieldApiListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<FarmFieldListPage<FieldRecord>>> {
    if load_farm(&state, &farm_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    let org_filter = query.org_filter();
    let list_query = query.list_query();
    let (status, page, page_size, limit, offset) = farm_field_page_window(&list_query);
    let total_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM fields WHERE farm_id = ?1 AND (?2 IS NULL OR owner = ?2) AND status = ?3",
    )
    .bind(&farm_id)
    .bind(&org_filter)
    .bind(status.as_str())
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    let rows = sqlx::query(
        r#"
        SELECT field_id, farm_id, owner, name, crop, season, notes, boundary_json, status,
               created_at, COALESCE(NULLIF(updated_at, ''), created_at) AS updated_at
        FROM fields
        WHERE farm_id = ?1 AND (?2 IS NULL OR owner = ?2) AND status = ?3
        ORDER BY COALESCE(season, '') DESC, name ASC, field_id ASC
        LIMIT ?4 OFFSET ?5
        "#,
    )
    .bind(&farm_id)
    .bind(&org_filter)
    .bind(status.as_str())
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut fields = Vec::with_capacity(rows.len());
    for row in rows {
        fields.push(decode_field_record(&row)?);
    }

    Ok(Json(farm_field_list_page(
        fields,
        total_count,
        page,
        page_size,
    )))
}

pub async fn list_farm_field_history(
    Path(farm_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FieldSeasonGroup>>> {
    if load_farm(&state, &farm_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    let rows = sqlx::query(
        r#"
        SELECT field_id, farm_id, owner, name, crop, season, notes, boundary_json, status,
               created_at, COALESCE(NULLIF(updated_at, ''), created_at) AS updated_at
        FROM fields
        WHERE farm_id = ?1 AND status = 'active'
        ORDER BY COALESCE(season, '') DESC, name ASC, field_id ASC
        "#,
    )
    .bind(&farm_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut fields = Vec::with_capacity(rows.len());
    for row in rows {
        fields.push(decode_field_record(&row)?);
    }
    Ok(Json(group_fields_by_season(fields)))
}

pub async fn list_fields(
    Query(query): Query<FarmFieldApiListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<FarmFieldListPage<FieldRecord>>> {
    let org_filter = query.org_filter();
    let list_query = query.list_query();
    let (status, page, page_size, limit, offset) = farm_field_page_window(&list_query);
    let total_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM fields WHERE (?1 IS NULL OR owner = ?1) AND status = ?2",
    )
    .bind(&org_filter)
    .bind(status.as_str())
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    let rows = sqlx::query(
        r#"
        SELECT field_id, farm_id, owner, name, crop, season, notes, boundary_json, status,
               created_at, COALESCE(NULLIF(updated_at, ''), created_at) AS updated_at
        FROM fields
        WHERE (?1 IS NULL OR owner = ?1) AND status = ?2
        ORDER BY name ASC, field_id ASC
        LIMIT ?3 OFFSET ?4
        "#,
    )
    .bind(&org_filter)
    .bind(status.as_str())
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut fields = Vec::with_capacity(rows.len());
    for row in rows {
        fields.push(decode_field_record(&row)?);
    }

    Ok(Json(farm_field_list_page(
        fields,
        total_count,
        page,
        page_size,
    )))
}

pub async fn list_field_boundaries(
    Query(query): Query<FarmFieldApiListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<FarmFieldListPage<FieldBoundaryRecord>>> {
    let org_filter = query.org_filter();
    let list_query = query.list_query();
    let (status, page, page_size, limit, offset) = farm_field_page_window(&list_query);
    let total_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM fields WHERE (?1 IS NULL OR owner = ?1) AND status = ?2",
    )
    .bind(&org_filter)
    .bind(status.as_str())
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    let rows = sqlx::query(
        r#"
        SELECT field_id, farm_id, owner, name, crop, season, notes, boundary_json, status,
               created_at, COALESCE(NULLIF(updated_at, ''), created_at) AS updated_at
        FROM fields
        WHERE (?1 IS NULL OR owner = ?1) AND status = ?2
        ORDER BY name ASC, field_id ASC
        LIMIT ?3 OFFSET ?4
        "#,
    )
    .bind(&org_filter)
    .bind(status.as_str())
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut boundaries = Vec::with_capacity(rows.len());
    for row in rows {
        boundaries.push(field_boundary_record_from_field(decode_field_record(&row)?));
    }

    Ok(Json(farm_field_list_page(
        boundaries,
        total_count,
        page,
        page_size,
    )))
}

pub async fn export_fields_geojson(State(state): State<AppState>) -> AppResult<Json<GeoJson>> {
    let rows = sqlx::query(
        r#"
        SELECT field_id, farm_id, owner, name, crop, season, notes, boundary_json, status,
               created_at, COALESCE(NULLIF(updated_at, ''), created_at) AS updated_at
        FROM fields
        WHERE status = 'active'
        ORDER BY name ASC, field_id ASC
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut fields = Vec::with_capacity(rows.len());
    for row in rows {
        fields.push(decode_field_record(&row)?);
    }

    Ok(Json(geojson_from_fields(fields)))
}

pub async fn list_scene_annotations(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AnnotationRecord>>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let rows = sqlx::query(
        r#"
        SELECT annotation_id, scene_id, field_id, author, crs, audit_id, label, note, severity, geometry_json, created_at, updated_at
        FROM annotations
        WHERE scene_id = ?1
        ORDER BY created_at ASC
        "#,
    )
    .bind(&scene_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut annotations = Vec::with_capacity(rows.len());
    for row in rows {
        annotations.push(decode_annotation_record(&row)?);
    }

    Ok(Json(annotations))
}

pub async fn create_scene_annotation(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CreateAnnotationRequest>,
) -> AppResult<Json<AnnotationRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let annotation = build_annotation_record(&state, &scene_id, request).await?;
    sqlx::query(
        r#"
        INSERT INTO annotations (
            annotation_id, scene_id, field_id, author, crs, audit_id, label, note, severity, geometry_json, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
    )
    .bind(&annotation.annotation_id)
    .bind(&annotation.scene_id)
    .bind(&annotation.field_id)
    .bind(&annotation.author)
    .bind(&annotation.crs)
    .bind(&annotation.audit_id)
    .bind(&annotation.label)
    .bind(&annotation.note)
    .bind(&annotation.severity)
    .bind(
        serde_json::to_string(&annotation.geometry).map_err(|err| AppError::Anyhow(err.into()))?,
    )
    .bind(&annotation.created_at)
    .bind(&annotation.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(annotation))
}

pub async fn update_scene_annotation(
    Path((scene_id, annotation_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(request): Json<UpdateAnnotationRequest>,
) -> AppResult<Json<AnnotationRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let existing = load_annotation(&state, &scene_id, &annotation_id)
        .await?
        .ok_or(AppError::NotFound)?;
    validate_annotation_geometry(&request.geometry)?;

    let label = normalize_annotation_label(request.label)?;
    let author = normalize_optional_text(request.author).or(existing.author);
    let crs = normalize_optional_text(request.crs).or(existing.crs);
    let audit_id = normalize_optional_text(request.audit_id).or(existing.audit_id);
    let updated = AnnotationRecord {
        annotation_id: annotation_id.clone(),
        scene_id: scene_id.clone(),
        field_id: load_scene_field_id(&state, &scene_id).await?,
        author,
        crs,
        audit_id,
        label,
        note: normalize_optional_text(request.note),
        severity: normalize_optional_text(request.severity),
        geometry: request.geometry,
        created_at: existing.created_at,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    let result = sqlx::query(
        r#"
        UPDATE annotations
        SET field_id = ?1, author = ?2, crs = ?3, audit_id = ?4, label = ?5, note = ?6, severity = ?7, geometry_json = ?8, updated_at = ?9
        WHERE annotation_id = ?10 AND scene_id = ?11
        "#,
    )
    .bind(&updated.field_id)
    .bind(&updated.author)
    .bind(&updated.crs)
    .bind(&updated.audit_id)
    .bind(&updated.label)
    .bind(&updated.note)
    .bind(&updated.severity)
    .bind(serde_json::to_string(&updated.geometry).map_err(|err| AppError::Anyhow(err.into()))?)
    .bind(&updated.updated_at)
    .bind(&updated.annotation_id)
    .bind(&updated.scene_id)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(updated))
}

pub async fn delete_scene_annotation(
    Path((scene_id, annotation_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<StatusCode> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let result = sqlx::query("DELETE FROM annotations WHERE annotation_id = ?1 AND scene_id = ?2")
        .bind(&annotation_id)
        .bind(&scene_id)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_scene_recommendations(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<RecommendationRecord>>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let rows = sqlx::query(
        r#"
        SELECT recommendation_id, scene_id, field_id, title, note, category, priority, status, evidence_refs_json, created_at, updated_at
        FROM recommendations
        WHERE scene_id = ?1
        ORDER BY created_at DESC
        "#,
    )
    .bind(&scene_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut recommendations = Vec::with_capacity(rows.len());
    for row in rows {
        recommendations.push(decode_recommendation_record(&state, &row).await?);
    }

    Ok(Json(recommendations))
}

pub async fn get_scene_recommendation(
    Path((scene_id, recommendation_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Json<RecommendationRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let recommendation = load_recommendation(&state, &scene_id, &recommendation_id)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(recommendation))
}

pub async fn create_scene_recommendation(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CreateRecommendationRequest>,
) -> AppResult<Json<RecommendationRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let recommendation = build_recommendation_record(&state, &scene_id, request).await?;
    sqlx::query(
        r#"
        INSERT INTO recommendations (
            recommendation_id, scene_id, field_id, title, note, category, priority, status, evidence_refs_json, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(&recommendation.recommendation_id)
    .bind(&recommendation.scene_id)
    .bind(&recommendation.field_id)
    .bind(&recommendation.title)
    .bind(&recommendation.note)
    .bind(&recommendation.category)
    .bind(recommendation_priority_str(recommendation.priority))
    .bind(recommendation_status_str(recommendation.status))
    .bind(
        serde_json::to_string(&recommendation.evidence_refs)
            .map_err(|err| AppError::Anyhow(err.into()))?,
    )
    .bind(&recommendation.created_at)
    .bind(&recommendation.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    persist_recommendation_annotations(
        &state,
        &recommendation.recommendation_id,
        &recommendation.annotation_ids,
    )
    .await?;

    Ok(Json(recommendation))
}

pub async fn update_scene_recommendation(
    Path((scene_id, recommendation_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(request): Json<UpdateRecommendationRequest>,
) -> AppResult<Json<RecommendationRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let existing = load_recommendation(&state, &scene_id, &recommendation_id)
        .await?
        .ok_or(AppError::NotFound)?;
    validate_recommendation_annotation_ids(&state, &scene_id, &request.annotation_ids).await?;
    let explicit_evidence_refs = if request.evidence_refs.is_empty() {
        existing.evidence_refs.clone()
    } else {
        request.evidence_refs
    };

    let updated = RecommendationRecord {
        recommendation_id: recommendation_id.clone(),
        scene_id: scene_id.clone(),
        field_id: load_scene_field_id(&state, &scene_id).await?,
        org_id: existing.org_id,
        author_user_id: existing.author_user_id,
        title: normalize_recommendation_title(request.title)?,
        note: normalize_optional_text(request.note),
        category: normalize_optional_text(request.category),
        action_category: normalize_optional_text(request.action_category)
            .unwrap_or(existing.action_category),
        priority: request.priority,
        status: request.status,
        evidence_refs: combine_text_values(
            recommendation_evidence_from_annotations(&request.annotation_ids),
            explicit_evidence_refs,
        ),
        annotation_ids: request.annotation_ids,
        created_at: existing.created_at,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    let result = sqlx::query(
        r#"
        UPDATE recommendations
        SET field_id = ?1, title = ?2, note = ?3, category = ?4, priority = ?5, status = ?6, evidence_refs_json = ?7, updated_at = ?8
        WHERE recommendation_id = ?9 AND scene_id = ?10
        "#,
    )
    .bind(&updated.field_id)
    .bind(&updated.title)
    .bind(&updated.note)
    .bind(&updated.category)
    .bind(recommendation_priority_str(updated.priority))
    .bind(recommendation_status_str(updated.status))
    .bind(
        serde_json::to_string(&updated.evidence_refs)
            .map_err(|err| AppError::Anyhow(err.into()))?,
    )
    .bind(&updated.updated_at)
    .bind(&updated.recommendation_id)
    .bind(&updated.scene_id)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    persist_recommendation_annotations(&state, &updated.recommendation_id, &updated.annotation_ids)
        .await?;

    Ok(Json(updated))
}

pub async fn delete_scene_recommendation(
    Path((scene_id, recommendation_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<StatusCode> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let result =
        sqlx::query("DELETE FROM recommendations WHERE recommendation_id = ?1 AND scene_id = ?2")
            .bind(&recommendation_id)
            .bind(&scene_id)
            .execute(&state.pool)
            .await
            .map_err(Error::from)?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_scene_reports(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<ReportRecord>>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let rows = sqlx::query(
        r#"
        SELECT report_id, scene_id, field_id, title, format, path, visibility, annotation_count, recommendation_count, created_at
        FROM reports
        WHERE scene_id = ?1
        ORDER BY created_at DESC
        "#,
    )
    .bind(&scene_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut reports = Vec::with_capacity(rows.len());
    for row in rows {
        reports.push(decode_report_record(&row)?);
    }

    Ok(Json(reports))
}

pub async fn generate_scene_report(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CreateReportRequest>,
) -> AppResult<Json<ReportRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let report = build_scene_report(&state, &scene_id, request.title, request.visibility).await?;
    sqlx::query(
        r#"
        INSERT INTO reports (
            report_id, scene_id, field_id, title, format, path, visibility, annotation_count, recommendation_count, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(&report.report_id)
    .bind(&report.scene_id)
    .bind(&report.field_id)
    .bind(&report.title)
    .bind(report_format_str(report.format))
    .bind(&report.artifact_path)
    .bind(report_visibility_str(report.visibility))
    .bind(report.annotation_count as i64)
    .bind(report.recommendation_count as i64)
    .bind(&report.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(report))
}

pub async fn download_scene_report(
    Path((scene_id, report_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let report = load_report(&state, &scene_id, &report_id)
        .await?
        .ok_or(AppError::NotFound)?;
    report_file_response(&report).await
}

pub async fn get_scene_report_lineage(
    Path((scene_id, report_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Json<BackwardProvenanceTrace>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let report = load_report(&state, &scene_id, &report_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let records = build_report_lineage_records(&state, &report).await?;
    let ledger = LineageLedger::from_persisted_records(records)
        .map_err(|err| AppError::Anyhow(Error::new(err)))?;
    let trace = ledger
        .trace_backward(&report_artifact_ref(&report.report_id))
        .map_err(|err| AppError::Anyhow(Error::new(err)))?;

    Ok(Json(trace))
}

pub async fn create_report_share(
    Path((scene_id, report_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(request): Json<CreateReportShareRequest>,
) -> AppResult<Json<ReportShareResponse>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let report = load_report(&state, &scene_id, &report_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if report.visibility != ReportVisibility::Shared {
        return Err(AppError::BadRequest(
            "org-only report cannot be shared".to_string(),
        ));
    }

    let now = current_record_timestamp();
    let share = ReportShareRecord {
        share_token: Uuid::new_v4().to_string(),
        report_id,
        scene_id,
        expires_at: normalize_share_expires_at(request.expires_at)?,
        revoked_at: None,
        created_at: now,
    };

    sqlx::query(
        r#"
        INSERT INTO report_shares (share_token, report_id, scene_id, expires_at, revoked_at, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&share.share_token)
    .bind(&share.report_id)
    .bind(&share.scene_id)
    .bind(&share.expires_at)
    .bind(&share.revoked_at)
    .bind(&share.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    audit_report_share_event(&state, &share, "share_created", None).await?;

    Ok(Json(report_share_response(&share)))
}

pub async fn revoke_report_share(
    Path((scene_id, report_id, share_token)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> AppResult<StatusCode> {
    let revoked_at = current_record_timestamp();
    let result = sqlx::query(
        r#"
        UPDATE report_shares
        SET revoked_at = COALESCE(revoked_at, ?1)
        WHERE scene_id = ?2 AND report_id = ?3 AND share_token = ?4
        "#,
    )
    .bind(&revoked_at)
    .bind(&scene_id)
    .bind(&report_id)
    .bind(&share_token)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    let share = load_report_share(&state, &share_token)
        .await?
        .ok_or(AppError::NotFound)?;
    audit_report_share_event(&state, &share, "share_revoked", None).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn download_shared_report(
    Path(share_token): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let share = load_report_share_with_report(&state, &share_token)
        .await?
        .ok_or(AppError::NotFound)?;

    if share.share.revoked_at.is_some() {
        return Err(AppError::Forbidden(
            "report share link has been revoked".to_string(),
        ));
    }
    if share_expired(&share.share.expires_at)? {
        return Err(AppError::Forbidden(
            "report share link has expired".to_string(),
        ));
    }
    if share.report.visibility != ReportVisibility::Shared {
        return Err(AppError::Forbidden(
            "report is not publicly shareable".to_string(),
        ));
    }

    audit_report_share_event(&state, &share.share, "share_accessed", None).await?;
    report_file_response(&share.report).await
}

pub async fn export_scene_annotations_csv(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let annotations = load_scene_annotation_records(&state, &scene_id).await?;
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "annotation_id",
            "scene_id",
            "field_id",
            "author",
            "crs",
            "audit_id",
            "label",
            "severity",
            "note",
            "geometry_type",
            "geometry_json",
            "created_at",
            "updated_at",
        ])
        .map_err(|err| AppError::Anyhow(err.into()))?;
    for annotation in annotations {
        let geometry_type = annotation_geometry_type(&annotation.geometry).to_string();
        let geometry_json = serde_json::to_string(&annotation.geometry)
            .map_err(|err| AppError::Anyhow(err.into()))?;
        writer
            .write_record(vec![
                annotation.annotation_id,
                annotation.scene_id,
                annotation.field_id.unwrap_or_default(),
                annotation.author.unwrap_or_default(),
                annotation.crs.unwrap_or_default(),
                annotation.audit_id.unwrap_or_default(),
                annotation.label,
                annotation.severity.unwrap_or_default(),
                annotation.note.unwrap_or_default(),
                geometry_type,
                geometry_json,
                annotation.created_at,
                annotation.updated_at,
            ])
            .map_err(|err| AppError::Anyhow(err.into()))?;
    }
    let csv_bytes = writer
        .into_inner()
        .map_err(|err| AppError::Anyhow(err.into_error().into()))?;

    response_with_bytes(csv_bytes, "text/csv; charset=utf-8", "annotations.csv")
}

pub async fn export_scene_recommendations_csv(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let recommendations = load_scene_recommendation_records(&state, &scene_id).await?;
    let annotations = load_scene_annotation_records(&state, &scene_id).await?;
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "recommendation_id",
            "scene_id",
            "field_id",
            "org_id",
            "author_user_id",
            "title",
            "category",
            "action_category",
            "priority",
            "status",
            "evidence_refs",
            "annotation_ids",
            "note",
            "created_at",
            "updated_at",
        ])
        .map_err(|err| AppError::Anyhow(err.into()))?;
    for recommendation in recommendations {
        let export_field_id = recommendation_export_field_id(&recommendation, &annotations);
        writer
            .write_record(vec![
                recommendation.recommendation_id,
                recommendation.scene_id,
                export_field_id.unwrap_or_default(),
                recommendation.org_id,
                recommendation.author_user_id,
                recommendation.title,
                recommendation.category.unwrap_or_default(),
                recommendation.action_category,
                recommendation_priority_str(recommendation.priority).to_string(),
                recommendation_status_str(recommendation.status).to_string(),
                recommendation.evidence_refs.join("|"),
                recommendation.annotation_ids.join("|"),
                recommendation.note.unwrap_or_default(),
                recommendation.created_at,
                recommendation.updated_at,
            ])
            .map_err(|err| AppError::Anyhow(err.into()))?;
    }
    let csv_bytes = writer
        .into_inner()
        .map_err(|err| AppError::Anyhow(err.into_error().into()))?;

    response_with_bytes(csv_bytes, "text/csv; charset=utf-8", "recommendations.csv")
}

pub async fn export_scene_annotations_geojson(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let annotations = load_scene_annotation_records(&state, &scene_id).await?;
    let crs = collection_crs_from_annotations(&annotations)?;
    let geojson = feature_collection_with_crs(
        annotations
            .iter()
            .map(feature_from_annotation)
            .collect::<AppResult<Vec<_>>>()?,
        &crs,
    );

    response_with_bytes(
        serde_json::to_vec(&geojson).map_err(|err| AppError::Anyhow(err.into()))?,
        "application/geo+json",
        "annotations.geojson",
    )
}

pub async fn export_scene_recommendations_geojson(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let recommendations = load_scene_recommendation_records(&state, &scene_id).await?;
    let annotations = load_scene_annotation_records(&state, &scene_id).await?;
    let crs = collection_crs_from_annotations(&annotations)?;
    let mut features = Vec::new();
    for recommendation in &recommendations {
        features.extend(recommendation_features(recommendation, &annotations)?);
    }

    let geojson = feature_collection_with_crs(features, &crs);

    response_with_bytes(
        serde_json::to_vec(&geojson).map_err(|err| AppError::Anyhow(err.into()))?,
        "application/geo+json",
        "recommendations.geojson",
    )
}

pub async fn export_field_records_csv(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let field = load_field(&state, &field_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let annotations = load_field_annotation_records(&state, &field_id).await?;
    let recommendations = load_field_recommendation_records(&state, &field_id).await?;
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "record_type",
            "record_id",
            "scene_id",
            "field_id",
            "crs",
            "title",
            "label",
            "status",
            "priority",
            "evidence_refs",
            "annotation_ids",
            "geometry_type",
            "geometry_json",
            "created_at",
            "updated_at",
        ])
        .map_err(|err| AppError::Anyhow(err.into()))?;

    for annotation in &annotations {
        let geometry_type = annotation_geometry_type(&annotation.geometry).to_string();
        let geometry_json = serde_json::to_string(&annotation.geometry)
            .map_err(|err| AppError::Anyhow(err.into()))?;
        writer
            .write_record(vec![
                "annotation".to_string(),
                annotation.annotation_id.clone(),
                annotation.scene_id.clone(),
                annotation
                    .field_id
                    .clone()
                    .unwrap_or_else(|| field.field_id.clone()),
                annotation
                    .crs
                    .clone()
                    .unwrap_or_else(|| field_record_crs(&field)),
                String::new(),
                annotation.label.clone(),
                String::new(),
                annotation.severity.clone().unwrap_or_default(),
                String::new(),
                String::new(),
                geometry_type,
                geometry_json,
                annotation.created_at.clone(),
                annotation.updated_at.clone(),
            ])
            .map_err(|err| AppError::Anyhow(err.into()))?;
    }

    for recommendation in recommendations {
        writer
            .write_record(vec![
                "recommendation".to_string(),
                recommendation.recommendation_id,
                recommendation.scene_id,
                recommendation
                    .field_id
                    .unwrap_or_else(|| field.field_id.clone()),
                field_record_crs(&field),
                recommendation.title,
                String::new(),
                recommendation_status_str(recommendation.status).to_string(),
                recommendation_priority_str(recommendation.priority).to_string(),
                recommendation.evidence_refs.join("|"),
                recommendation.annotation_ids.join("|"),
                String::new(),
                String::new(),
                recommendation.created_at,
                recommendation.updated_at,
            ])
            .map_err(|err| AppError::Anyhow(err.into()))?;
    }

    let csv_bytes = writer
        .into_inner()
        .map_err(|err| AppError::Anyhow(err.into_error().into()))?;

    response_with_bytes(csv_bytes, "text/csv; charset=utf-8", "field-records.csv")
}

pub async fn export_field_records_geojson(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let field = load_field(&state, &field_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let field_crs = field_record_crs(&field);
    let annotations = load_field_annotation_records(&state, &field_id).await?;
    assert_field_bundle_annotation_crs(&annotations, &field_crs)?;
    let recommendations = load_field_recommendation_records(&state, &field_id).await?;

    let mut features = vec![feature_from_field(field.clone())];
    features.extend(
        annotations
            .iter()
            .map(feature_from_annotation)
            .collect::<AppResult<Vec<_>>>()?,
    );
    for recommendation in &recommendations {
        features.extend(recommendation_features(recommendation, &annotations)?);
    }

    let geojson = feature_collection_with_crs(features, &field_crs);

    response_with_bytes(
        serde_json::to_vec(&geojson).map_err(|err| AppError::Anyhow(err.into()))?,
        "application/geo+json",
        "field-records.geojson",
    )
}

pub async fn create_field(
    State(state): State<AppState>,
    Json(request): Json<CreateFieldRequest>,
) -> AppResult<Json<FieldRecord>> {
    let mut field = build_field_record(request)?;
    field.owner = field_owner_for_farm(&state, field.farm_id.as_deref(), &field.owner).await?;
    field.org_id = field.owner.clone();

    sqlx::query(
        r#"
        INSERT INTO fields (field_id, farm_id, owner, name, crop, season, notes, boundary_json, status, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(&field.field_id)
    .bind(&field.farm_id)
    .bind(&field.owner)
    .bind(&field.name)
    .bind(&field.crop)
    .bind(&field.season)
    .bind(&field.notes)
    .bind(serde_json::to_string(&field.boundary).map_err(|err| AppError::Anyhow(err.into()))?)
    .bind(field.status.as_str())
    .bind(&field.created_at)
    .bind(&field.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(field))
}

pub async fn get_field(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<FieldRecord>> {
    let field = load_field(&state, &field_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(field))
}

pub async fn link_field_to_farm(
    Path((field_id, farm_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Json<FieldRecord>> {
    let mut field = load_field(&state, &field_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let farm = load_farm(&state, &farm_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let updated_at = current_record_timestamp();
    sqlx::query("UPDATE fields SET farm_id = ?2, owner = ?3, updated_at = ?4 WHERE field_id = ?1")
        .bind(&field_id)
        .bind(&farm_id)
        .bind(&farm.owner)
        .bind(&updated_at)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;

    field.farm_id = Some(farm_id);
    field.owner = farm.owner.clone();
    field.org_id = farm.owner;
    field.updated_at = updated_at;
    Ok(Json(field))
}

pub async fn list_field_scenes(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SceneSummary>>> {
    if load_field(&state, &field_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    let rows = sqlx::query(
        "SELECT scene_id, owner, sensor, acquired_at, created_at, field_id, season_id, linked_at FROM scenes WHERE field_id = ?1 ORDER BY acquired_at DESC",
    )
    .bind(&field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let scenes = rows
        .into_iter()
        .map(|row| SceneSummary {
            scene_id: row.get("scene_id"),
            owner: row.get("owner"),
            sensor: row.get("sensor"),
            acquired_at: row.get("acquired_at"),
            created_at: row.get("created_at"),
            field_id: row.get("field_id"),
            season_id: row.get("season_id"),
            linked_at: row.get("linked_at"),
        })
        .collect();

    Ok(Json(scenes))
}

pub async fn list_field_scene_refresh_advisories(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<SceneRefreshAdvisoriesResponse>> {
    if load_field(&state, &field_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    let current_scene_row = sqlx::query(
        "SELECT scene_id, owner, sensor, acquired_at, data_path, metadata_json, cloud_cover, field_id, season_id, linked_at FROM scenes WHERE field_id = ?1 ORDER BY acquired_at DESC LIMIT 1",
    )
    .bind(&field_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;
    let Some(current_scene_row) = current_scene_row else {
        return Ok(Json(SceneRefreshAdvisoriesResponse {
            advisory_enabled: false,
            reason: Some("no-linked-scene".to_string()),
            advisories: Vec::new(),
        }));
    };

    let current_scene_id: String = current_scene_row.get("scene_id");
    let current_acquired_at: String = current_scene_row.get("acquired_at");
    let current_cloud_cover: Option<f64> = current_scene_row.get("cloud_cover");
    let current_season_id: Option<String> = current_scene_row.get("season_id");
    let current_data_path: String = current_scene_row.get("data_path");
    let current_scene_dir = FsPath::new(&current_data_path);
    let current_metadata = load_scene_metadata(Some(&current_scene_row), current_scene_dir).await?;
    let current_asserted_spatial_ref =
        ingest::load_scene_spatial_ref(&state.pool, &current_scene_id).await?;
    if let Err(error) = assert_scene_spatial_ref_integrity(
        current_metadata.as_ref(),
        current_asserted_spatial_ref.as_ref(),
    ) {
        return Ok(Json(SceneRefreshAdvisoriesResponse {
            advisory_enabled: false,
            reason: Some(format!("advisory-gated: {error}")),
            advisories: Vec::new(),
        }));
    }

    let current_acquired_at_ts = parse_acquired_at(&current_acquired_at)
        .ok_or_else(|| AppError::BadRequest("current scene acquired_at is invalid".to_string()))?;

    let candidate_rows = sqlx::query(
        "SELECT scene_id, owner, sensor, acquired_at, data_path, metadata_json, cloud_cover, season_id FROM scenes WHERE scene_id != ?1 AND acquired_at > ?2 AND (?3 IS NULL OR season_id = ?3) ORDER BY acquired_at DESC",
    )
    .bind(&current_scene_id)
    .bind(&current_acquired_at)
    .bind(current_season_id.clone())
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut advisories = Vec::new();
    for row in candidate_rows {
        let candidate_scene_id: String = row.get("scene_id");
        let candidate_acquired_at: String = row.get("acquired_at");
        let candidate_cloud_cover: Option<f64> = row.get("cloud_cover");
        let candidate_data_path: String = row.get("data_path");
        let candidate_scene_dir = FsPath::new(&candidate_data_path);
        let candidate_metadata = load_scene_metadata(Some(&row), candidate_scene_dir).await?;
        let candidate_asserted_spatial_ref =
            ingest::load_scene_spatial_ref(&state.pool, &candidate_scene_id).await?;

        let candidate_acquired_at_ts = match parse_acquired_at(&candidate_acquired_at) {
            Some(ts) if ts > current_acquired_at_ts => ts,
            _ => continue,
        };

        let (is_lower_cloud, cloud_is_uncertain) =
            is_lower_cloud(current_cloud_cover, candidate_cloud_cover);
        if !is_lower_cloud {
            continue;
        }

        let mut uncertainty = cloud_is_uncertain;
        if uncertainty == false
            && !is_scene_spatially_consistent(
                current_asserted_spatial_ref.as_ref(),
                current_metadata.as_ref(),
                candidate_asserted_spatial_ref.as_ref(),
                candidate_metadata.as_ref(),
            )
        {
            uncertainty = true;
        }

        advisories.push(SceneRefreshAdvisory {
            current_scene_id: current_scene_id.clone(),
            candidate_scene_id,
            current_acquired_at: current_acquired_at_ts.to_rfc3339(),
            candidate_acquired_at: candidate_acquired_at_ts.to_rfc3339(),
            current_cloud_cover,
            candidate_cloud_cover,
            uncertainty,
            reason: if uncertainty {
                "temporal/fidelity confidence reduced".to_string()
            } else {
                "fresher-lower-cloud".to_string()
            },
        });
    }

    Ok(Json(SceneRefreshAdvisoriesResponse {
        advisory_enabled: true,
        reason: None,
        advisories,
    }))
}

pub async fn list_field_scene_change_advisories(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<SceneChangeAdvisoriesResponse>> {
    if load_field(&state, &field_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    let rows = sqlx::query(
        "SELECT scene_id, acquired_at, data_path, metadata_json, cloud_cover FROM scenes WHERE field_id = ?1 ORDER BY acquired_at DESC LIMIT 2",
    )
    .bind(&field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    if rows.len() < 2 {
        return Ok(Json(SceneChangeAdvisoriesResponse {
            advisory_enabled: true,
            reason: Some("single-linked-scene: no comparison available".to_string()),
            advisories: Vec::new(),
        }));
    }

    let comparison_scene_id: String = rows[0].get("scene_id");
    let comparison_acquired_at: String = rows[0].get("acquired_at");
    let comparison_data_path: String = rows[0].get("data_path");
    let comparison_cloud_cover: Option<f64> = rows[0].get("cloud_cover");
    let comparison_metadata =
        load_scene_metadata(Some(&rows[0]), FsPath::new(&comparison_data_path)).await?;
    let comparison_spatial_ref =
        ingest::load_scene_spatial_ref(&state.pool, &comparison_scene_id).await?;

    let baseline_scene_id: String = rows[1].get("scene_id");
    let baseline_acquired_at: String = rows[1].get("acquired_at");
    let baseline_data_path: String = rows[1].get("data_path");
    let baseline_cloud_cover: Option<f64> = rows[1].get("cloud_cover");
    let baseline_metadata =
        load_scene_metadata(Some(&rows[1]), FsPath::new(&baseline_data_path)).await?;
    let baseline_spatial_ref =
        ingest::load_scene_spatial_ref(&state.pool, &baseline_scene_id).await?;

    let baseline_extent =
        scene_extent_for_link(baseline_metadata.as_ref(), baseline_spatial_ref.as_ref());
    let comparison_extent = scene_extent_for_link(
        comparison_metadata.as_ref(),
        comparison_spatial_ref.as_ref(),
    );

    let comparable = baseline_spatial_ref
        .as_ref()
        .zip(comparison_spatial_ref.as_ref())
        .is_some_and(|(baseline, comparison)| {
            assert_scene_spatial_ref_integrity(baseline_metadata.as_ref(), Some(baseline)).is_ok()
                && assert_scene_spatial_ref_integrity(
                    comparison_metadata.as_ref(),
                    Some(comparison),
                )
                .is_ok()
                && assert_spatial_refs_equivalent(baseline, comparison).is_ok()
        });

    let common_extent = baseline_extent
        .as_ref()
        .zip(comparison_extent.as_ref())
        .and_then(|(baseline, comparison)| common_scene_extent(baseline, comparison));
    let coverage_fraction = common_extent
        .as_ref()
        .zip(baseline_extent.as_ref())
        .map(|(common, baseline)| {
            let baseline_area = scene_extent_area(baseline);
            if baseline_area <= f64::EPSILON {
                0.0
            } else {
                (scene_extent_area(common) / baseline_area).clamp(0.0, 1.0)
            }
        })
        .unwrap_or(0.0);

    let (change_score, uncertainty_low, uncertainty_high, reason, confidence) = if comparable {
        let score = coarse_scene_change_score(baseline_cloud_cover, comparison_cloud_cover);
        let uncertainty = if baseline_cloud_cover.is_some() && comparison_cloud_cover.is_some() {
            0.05
        } else {
            0.25
        };
        (
            score,
            (score - uncertainty).max(0.0),
            (score + uncertainty).min(1.0),
            "aligned-common-extent".to_string(),
            if uncertainty <= 0.05 { "medium" } else { "low" }.to_string(),
        )
    } else {
        (
            0.0,
            0.0,
            1.0,
            "spatial-ref-mismatch: change unavailable without comparable CRS/extent/resolution"
                .to_string(),
            "low".to_string(),
        )
    };

    Ok(Json(SceneChangeAdvisoriesResponse {
        advisory_enabled: true,
        reason: None,
        advisories: vec![SceneChangeAdvisory {
            baseline_scene_id,
            comparison_scene_id,
            baseline_acquired_at,
            comparison_acquired_at,
            common_extent: if comparable { common_extent } else { None },
            coverage_fraction: if comparable { coverage_fraction } else { 0.0 },
            change_score,
            uncertainty_low,
            uncertainty_high,
            confidence,
            reason,
        }],
    }))
}

pub async fn link_scene_to_field(
    Path((scene_id, field_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Json<SceneDetail>> {
    let field = load_field(&state, &field_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let season_id = season_id_for_linked_field(&field)?;

    let scene_row = sqlx::query(
        "SELECT scene_id, owner, sensor, acquired_at, data_path, metadata_json, created_at, field_id, season_id, linked_at FROM scenes WHERE scene_id = ?1",
    )
    .bind(&scene_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?
    .ok_or(AppError::NotFound)?;

    let scene_dir = state.config.data_root.join("scenes").join(&scene_id);
    let metadata = load_scene_metadata(Some(&scene_row), &scene_dir).await?;
    let asserted_spatial_ref = ingest::load_scene_spatial_ref(&state.pool, &scene_id).await?;
    assert_scene_spatial_ref_integrity(metadata.as_ref(), asserted_spatial_ref.as_ref())?;
    let scene_extent = scene_extent_for_link(metadata.as_ref(), asserted_spatial_ref.as_ref())
        .ok_or_else(|| {
            AppError::BadRequest(
                "scene-field-season linkage requires a georeferenced scene extent".to_string(),
            )
        })?;

    if !scene_extent_intersects_bounds(&scene_extent, &field.extent) {
        return Err(AppError::BadRequest(
            "no-overlap: scene extent does not intersect field boundary".to_string(),
        ));
    }

    let previous_field_id = scene_row.get::<Option<String>, _>("field_id");
    let previous_season_id = scene_row.get::<Option<String>, _>("season_id");
    let linked_at = current_record_timestamp();
    let updated = sqlx::query(
        "UPDATE scenes SET field_id = ?1, season_id = ?2, linked_at = ?3 WHERE scene_id = ?4",
    )
    .bind(&field_id)
    .bind(&season_id)
    .bind(&linked_at)
    .bind(&scene_id)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    insert_scene_link_audit(
        &state,
        &scene_id,
        previous_field_id.as_deref(),
        previous_season_id.as_deref(),
        &field_id,
        &season_id,
        &linked_at,
    )
    .await?;

    get_scene(Path(scene_id), State(state)).await
}

pub async fn list_scenes(State(state): State<AppState>) -> AppResult<Json<Vec<SceneSummary>>> {
    let rows =
        sqlx::query(
            "SELECT scene_id, owner, sensor, acquired_at, created_at, field_id, season_id, linked_at FROM scenes ORDER BY acquired_at DESC",
        )
            .fetch_all(&state.pool)
            .await
            .map_err(Error::from)?;

    let scenes = rows
        .into_iter()
        .map(|row| SceneSummary {
            scene_id: row.get("scene_id"),
            owner: row.get("owner"),
            sensor: row.get("sensor"),
            acquired_at: row.get("acquired_at"),
            created_at: row.get("created_at"),
            field_id: row.get("field_id"),
            season_id: row.get("season_id"),
            linked_at: row.get("linked_at"),
        })
        .collect();

    Ok(Json(scenes))
}

pub async fn get_scene(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<SceneDetail>> {
    let scene_row = sqlx::query(
        "SELECT scene_id, owner, sensor, acquired_at, data_path, metadata_json, created_at, field_id, season_id, linked_at FROM scenes WHERE scene_id = ?1",
    )
            .bind(&scene_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(Error::from)?;

    let scene_dir = state.config.data_root.join("scenes").join(&scene_id);
    let has_scene_dir = fs::try_exists(&scene_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;

    if scene_row.is_none() && !has_scene_dir {
        return Err(AppError::NotFound);
    }

    let metadata = load_scene_metadata(scene_row.as_ref(), &scene_dir).await?;
    let field = load_scene_field(&state, scene_row.as_ref()).await?;
    let ingest = ingest::load_ingest_record(&state.pool, &scene_id).await?;
    let asserted_spatial_ref = ingest::load_scene_spatial_ref(&state.pool, &scene_id).await?;
    assert_scene_spatial_ref_integrity(metadata.as_ref(), asserted_spatial_ref.as_ref())?;
    let available_products = collect_scene_products(&state, &scene_id).await?;

    Ok(Json(SceneDetail {
        scene_id,
        owner: scene_row.as_ref().map(|row| row.get("owner")),
        sensor: scene_row.as_ref().map(|row| row.get("sensor")),
        acquired_at: scene_row.as_ref().map(|row| row.get("acquired_at")),
        created_at: scene_row.as_ref().map(|row| row.get("created_at")),
        width: metadata.as_ref().map(|image| image.metadata.width),
        height: metadata.as_ref().map(|image| image.metadata.height),
        bands: metadata
            .as_ref()
            .map(|image| image.metadata.bands.clone())
            .unwrap_or_default(),
        gps_position: metadata
            .as_ref()
            .and_then(|image| image.metadata.gps_position.clone()),
        data_path: scene_row.as_ref().map(|row| row.get("data_path")),
        field_id: scene_row
            .as_ref()
            .and_then(|row| row.get::<Option<String>, _>("field_id")),
        season_id: scene_row
            .as_ref()
            .and_then(|row| row.get::<Option<String>, _>("season_id")),
        linked_at: scene_row
            .as_ref()
            .and_then(|row| row.get::<Option<String>, _>("linked_at")),
        field,
        ingest,
        geospatial: build_geospatial_metadata_with_asserted(
            metadata.as_ref(),
            asserted_spatial_ref.as_ref(),
        ),
        available_products,
    }))
}

pub async fn list_layers(
    Query(query): Query<LayerListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<LayerListResponse>> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 100);
    let stale_after_days = normalized_stale_after_days(query.stale_after_days);
    let rows = load_layer_rows(&state).await?;
    let mut layers = Vec::new();

    for row in rows {
        if !layer_row_matches_query(&row, &query) {
            continue;
        }
        if let Some(layer) = layer_from_row(&row, false, stale_after_days).await? {
            layers.push(layer);
        }
    }

    let total = layers.len();
    let start = page.saturating_sub(1).saturating_mul(page_size);
    let layers = layers.into_iter().skip(start).take(page_size).collect();

    Ok(Json(LayerListResponse {
        page,
        page_size,
        total,
        layers,
    }))
}

pub async fn get_layer_metadata(
    Path((scene_id, kind)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Json<LayerMetadata>> {
    let row = load_layer_row(&state, &scene_id, &kind)
        .await?
        .ok_or(AppError::NotFound)?;
    let layer = layer_from_row(&row, true, DEFAULT_LAYER_STALE_AFTER_DAYS)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(layer))
}

pub async fn publish_open_data_layer(
    Path((scene_id, kind)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(request): Json<OpenDataLayerPublishRequest>,
) -> AppResult<Json<OpenDataLayerCatalogEntry>> {
    let row = load_layer_row(&state, &scene_id, &kind)
        .await?
        .ok_or(AppError::NotFound)?;
    let layer = layer_from_row(&row, true, DEFAULT_LAYER_STALE_AFTER_DAYS)
        .await?
        .ok_or(AppError::NotFound)?;
    let source_layer_ref = layer.layer_id.clone();
    let generated_open_data_id = format!("open-data:{}:{}", layer.scene_id, layer.product_kind);
    let publication = prepare_open_data_publication(
        OpenDataPublishRequest {
            source_layer_ref,
            license: request.license,
            attribution: request.attribution,
            owner_identifier: request.owner_identifier,
            field_identifier: request.field_identifier,
        },
        generated_open_data_id,
    )
    .map_err(open_data_publish_error)?;

    sqlx::query(
        r#"
        UPDATE products
        SET open_data_license = ?3,
            open_data_attribution = ?4,
            open_data_anonymized = 1,
            open_data_refusal_reason = NULL,
            open_data_published_at = datetime('now')
        WHERE scene_id = ?1 AND lower(kind) = lower(?2)
        "#,
    )
    .bind(&scene_id)
    .bind(&kind)
    .bind(&publication.license)
    .bind(&publication.attribution)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(open_data_catalog_entry_from_layer(
        &layer,
        &publication,
        None,
    )))
}

pub async fn list_open_data_layers(
    State(state): State<AppState>,
) -> AppResult<Json<OpenDataCatalogResponse>> {
    let rows = load_layer_rows(&state).await?;
    let mut layers = Vec::new();
    for row in rows {
        if let Some(layer) = layer_from_row(&row, false, DEFAULT_LAYER_STALE_AFTER_DAYS).await? {
            let license = row.get::<Option<String>, _>("open_data_license");
            let attribution = row.get::<Option<String>, _>("open_data_attribution");
            let anonymized = row.get::<Option<i64>, _>("open_data_anonymized") == Some(1);
            if let (Some(license), Some(attribution), true) = (license, attribution, anonymized) {
                let publication = OpenDataPublication {
                    open_data_id: format!("open-data:{}:{}", layer.scene_id, layer.product_kind),
                    source_layer_ref: layer.layer_id.clone(),
                    license,
                    attribution,
                    anonymized,
                };
                layers.push(open_data_catalog_entry_from_layer(
                    &layer,
                    &publication,
                    row.get("open_data_published_at"),
                ));
            }
        }
    }

    Ok(Json(OpenDataCatalogResponse { layers }))
}

pub async fn get_scene_audit(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<SceneAuditTrail>> {
    let scene_exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM scenes WHERE scene_id = ?1")
        .bind(&scene_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(Error::from)?;
    let ingest_attempts = ingest::load_ingest_attempts(&state.pool, &scene_id).await?;
    let link_audits = load_scene_link_audits(&state, &scene_id).await?;

    if scene_exists.is_none() && ingest_attempts.is_empty() && link_audits.is_empty() {
        return Err(AppError::NotFound);
    }

    Ok(Json(SceneAuditTrail {
        scene_id,
        ingest_attempts,
        link_audits,
    }))
}

async fn load_scene_link_audits(
    state: &AppState,
    scene_id: &str,
) -> AppResult<Vec<SceneLinkAuditRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT audit_id, scene_id, mutation, previous_field_id, previous_season_id,
               new_field_id, new_season_id, occurred_at
        FROM scene_link_audits
        WHERE scene_id = ?1
        ORDER BY occurred_at ASC, audit_id ASC
        "#,
    )
    .bind(scene_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(rows
        .into_iter()
        .map(|row| SceneLinkAuditRecord {
            audit_id: row.get("audit_id"),
            scene_id: row.get("scene_id"),
            mutation: row.get("mutation"),
            previous_field_id: row.get("previous_field_id"),
            previous_season_id: row.get("previous_season_id"),
            new_field_id: row.get("new_field_id"),
            new_season_id: row.get("new_season_id"),
            occurred_at: row.get("occurred_at"),
        })
        .collect())
}

async fn insert_scene_link_audit(
    state: &AppState,
    scene_id: &str,
    previous_field_id: Option<&str>,
    previous_season_id: Option<&str>,
    new_field_id: &str,
    new_season_id: &str,
    occurred_at: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO scene_link_audits (
            audit_id, scene_id, mutation, previous_field_id, previous_season_id,
            new_field_id, new_season_id, occurred_at
        )
        VALUES (?1, ?2, 'link_scene_to_field', ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(format!("scene-link-audit-{}", Uuid::new_v4()))
    .bind(scene_id)
    .bind(previous_field_id)
    .bind(previous_season_id)
    .bind(new_field_id)
    .bind(new_season_id)
    .bind(occurred_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    Ok(())
}

pub async fn export_layer_geotiff(
    Path((scene_id, kind)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let row = load_layer_row(&state, &scene_id, &kind)
        .await?
        .ok_or(AppError::NotFound)?;
    let layer = layer_from_row(&row, true, DEFAULT_LAYER_STALE_AFTER_DAYS)
        .await?
        .ok_or(AppError::NotFound)?;
    let metadata_json: String = row.get("metadata_json");
    let image = serde_json::from_str::<MultispectralImage>(&metadata_json).map_err(|err| {
        AppError::Anyhow(
            Error::new(err).context("failed to decode layer scene metadata_json from database"),
        )
    })?;
    let width = layer.width_px.unwrap_or(image.metadata.width);
    let height = layer.height_px.unwrap_or(image.metadata.height);
    let cell_count = raster_cell_count(width, height)?;
    let report = export_raster_geotiff(RasterProduct {
        product_id: layer.layer_id.clone(),
        width,
        height,
        spatial_ref: layer.spatial_ref,
        cells: vec![0.0; cell_count],
    })
    .map_err(|err| AppError::BadRequest(err.to_string()))?;

    response_with_bytes(
        report.exported_bytes,
        "image/tiff",
        &format!("{}-{}.tif", scene_id, layer.product_kind),
    )
}

fn raster_cell_count(width: u32, height: u32) -> AppResult<usize> {
    usize::try_from(u64::from(width) * u64::from(height)).map_err(|_| {
        AppError::BadRequest("raster dimensions are too large for GeoTIFF export".to_string())
    })
}

pub async fn stream_product(
    Path((scene_id, kind)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    assert_scene_product_spatial_integrity(&state, &scene_id).await?;
    let product_path = resolve_product_path(&state, &scene_id, &kind).await?;

    let file = File::open(&product_path)
        .await
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => AppError::NotFound,
            _ => AppError::Anyhow(error.into()),
        })?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let content_type = content_type_for_path(&product_path);

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));

    if let Some(filename) = product_path.file_name().and_then(|name| name.to_str()) {
        if let Ok(value) = HeaderValue::from_str(&format!("inline; filename=\"{}\"", filename)) {
            headers.insert(header::CONTENT_DISPOSITION, value);
        }
    }

    Ok((headers, body).into_response())
}

pub async fn stream_product_tile(
    Path((scene_id, kind, z, x, y_segment)): Path<(String, String, u8, u32, String)>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let y = y_segment
        .strip_suffix(".png")
        .ok_or_else(|| AppError::BadRequest("tile requests must end with .png".to_string()))?
        .parse::<u32>()
        .map_err(|_| AppError::BadRequest("invalid tile y coordinate".to_string()))?;
    assert_scene_product_spatial_integrity(&state, &scene_id).await?;
    let product_path = resolve_product_path(&state, &scene_id, &kind).await?;
    let tile_path = tile_cache_path(&state, &scene_id, &kind, &product_path, z, x, y).await?;

    if !fs::try_exists(&tile_path)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        let source_path = product_path.clone();
        let tile_bytes =
            tokio::task::spawn_blocking(move || generate_tile_bytes(&source_path, z, x, y))
                .await
                .map_err(|err| AppError::Anyhow(err.into()))??;

        if let Some(parent) = tile_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|err| AppError::Anyhow(err.into()))?;
        }
        fs::write(&tile_path, tile_bytes)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
    }

    let file = File::open(&tile_path)
        .await
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => AppError::NotFound,
            _ => AppError::Anyhow(error.into()),
        })?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );

    Ok((headers, body).into_response())
}

async fn resolve_product_path(state: &AppState, scene_id: &str, kind: &str) -> AppResult<PathBuf> {
    if let Some(path) = find_product_file_on_disk(state, scene_id, kind).await? {
        return Ok(path);
    }

    match ingest::ensure_product(&state.pool, scene_id, kind).await {
        Ok(path) => Ok(path),
        Err(err) if is_missing_scene_error(&err) => Err(AppError::NotFound),
        Err(err) if is_product_publish_error(&err) => Err(AppError::BadRequest(err.to_string())),
        Err(err) => Err(AppError::Anyhow(err)),
    }
}

async fn find_product_file_on_disk(
    state: &AppState,
    scene_id: &str,
    kind: &str,
) -> AppResult<Option<PathBuf>> {
    let product_dir = state
        .config
        .data_root
        .join("scenes")
        .join(scene_id)
        .join("products")
        .join(kind);

    if !fs::try_exists(&product_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        return Ok(None);
    }

    let mut entries = fs::read_dir(&product_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;

    select_preferred_product_path(&mut entries).await
}

async fn tile_cache_path(
    state: &AppState,
    scene_id: &str,
    kind: &str,
    product_path: &FsPath,
    z: u8,
    x: u32,
    y: u32,
) -> AppResult<PathBuf> {
    // On-demand tiles are cached under a source fingerprint so regenerated products
    // naturally miss the old cache path without needing synchronous cleanup work.
    let fingerprint = product_cache_fingerprint(product_path).await?;
    Ok(state
        .config
        .data_root
        .join("scenes")
        .join(scene_id)
        .join("tile_cache")
        .join(kind)
        .join(fingerprint)
        .join(z.to_string())
        .join(x.to_string())
        .join(format!("{y}.png")))
}

async fn product_cache_fingerprint(path: &FsPath) -> AppResult<String> {
    let metadata = fs::metadata(path)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let modified_epoch = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|value| value.as_secs())
        .unwrap_or_default();

    Ok(format!("{}-{}", metadata.len(), modified_epoch))
}

fn generate_tile_bytes(product_path: &FsPath, z: u8, x: u32, y: u32) -> AppResult<Vec<u8>> {
    let tiles_per_axis = 1_u32
        .checked_shl(z as u32)
        .ok_or_else(|| AppError::BadRequest("unsupported zoom level".to_string()))?;
    if x >= tiles_per_axis || y >= tiles_per_axis {
        return Err(AppError::NotFound);
    }

    let image = image::open(product_path).map_err(|err| AppError::Anyhow(err.into()))?;
    let rgba = image.to_rgba8();
    let source_width = rgba.width().max(1);
    let source_height = rgba.height().max(1);

    let x0 = (((x as f64) / (tiles_per_axis as f64)) * source_width as f64).floor() as u32;
    let y0 = (((y as f64) / (tiles_per_axis as f64)) * source_height as f64).floor() as u32;
    let x1 = ((((x + 1) as f64) / (tiles_per_axis as f64)) * source_width as f64).ceil() as u32;
    let y1 = ((((y + 1) as f64) / (tiles_per_axis as f64)) * source_height as f64).ceil() as u32;

    let crop_width = x1
        .saturating_sub(x0)
        .clamp(1, source_width.saturating_sub(x0).max(1));
    let crop_height = y1
        .saturating_sub(y0)
        .clamp(1, source_height.saturating_sub(y0).max(1));

    let cropped = image::imageops::crop_imm(&rgba, x0, y0, crop_width, crop_height).to_image();
    let resized = image::imageops::resize(&cropped, TILE_SIZE, TILE_SIZE, FilterType::Triangle);
    let tile = DynamicImage::ImageRgba8(resized);

    let mut cursor = Cursor::new(Vec::new());
    tile.write_to(&mut cursor, ImageFormat::Png)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    Ok(cursor.into_inner())
}

async fn collect_scene_products(
    state: &AppState,
    scene_id: &str,
) -> AppResult<Vec<ProductSummary>> {
    let mut products = BTreeMap::new();
    let scene_products_dir = state
        .config
        .data_root
        .join("scenes")
        .join(scene_id)
        .join("products");

    if fs::try_exists(&scene_products_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        let mut kind_dirs = fs::read_dir(&scene_products_dir)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;

        while let Some(entry) = kind_dirs
            .next_entry()
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?
        {
            let file_type = entry
                .file_type()
                .await
                .map_err(|err| AppError::Anyhow(err.into()))?;
            if !file_type.is_dir() {
                continue;
            }

            let kind = entry.file_name().to_string_lossy().to_string();
            let mut entries = fs::read_dir(entry.path())
                .await
                .map_err(|err| AppError::Anyhow(err.into()))?;

            if let Some(path) = select_preferred_product_path(&mut entries).await? {
                products.insert(kind.clone(), build_product_summary(scene_id, &kind, &path));
            }
        }
    }

    let rows = sqlx::query(
        r#"
        SELECT product_id, field_id, season_id, kind, path, width_px, height_px, gsd_m_per_px,
               spatial_ref_json, source_image_ids_json, source_scan_ids_json,
               publish_status, qa_report_ref, provenance_hash, downstream_consumers_json
        FROM products
        WHERE scene_id = ?1
        "#,
    )
    .bind(scene_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    for row in rows {
        let kind: String = row.get("kind");
        let path = PathBuf::from(row.get::<String, _>("path"));
        let exists = fs::try_exists(&path)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
        if !exists {
            continue;
        }
        products.insert(
            kind.clone(),
            product_summary_from_row(scene_id, &row, &path)?,
        );
    }

    Ok(products.into_values().collect())
}

async fn load_layer_rows(state: &AppState) -> AppResult<Vec<sqlx::sqlite::SqliteRow>> {
    let rows = sqlx::query(
        r#"
        SELECT
            p.product_id,
            p.kind,
            p.path,
            p.field_id AS product_field_id,
            p.season_id AS product_season_id,
            p.width_px AS product_width_px,
            p.height_px AS product_height_px,
            p.gsd_m_per_px AS product_gsd_m_per_px,
            p.spatial_ref_json AS product_spatial_ref_json,
            p.source_image_ids_json,
            p.source_scan_ids_json,
            p.publish_status,
            p.qa_report_ref,
            p.provenance_hash,
            p.downstream_consumers_json,
            p.open_data_license,
            p.open_data_attribution,
            p.open_data_anonymized,
            p.open_data_refusal_reason,
            p.open_data_published_at,
            s.scene_id,
            s.sensor,
            s.acquired_at,
            s.metadata_json,
            s.field_id,
            s.season_id,
            i.ingested_at,
            i.coverage_fraction,
            i.source_path AS ingest_source_path,
            sr.spatial_ref_json AS scene_spatial_ref_json,
            f.boundary_json AS field_boundary_json
        FROM products p
        JOIN scenes s ON s.scene_id = p.scene_id
        LEFT JOIN scene_ingests i ON i.scene_id = s.scene_id
        LEFT JOIN scene_spatial_refs sr ON sr.scene_id = s.scene_id
        LEFT JOIN fields f ON f.field_id = COALESCE(p.field_id, s.field_id)
        ORDER BY s.acquired_at DESC, p.kind ASC
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;
    Ok(rows)
}

async fn load_layer_row(
    state: &AppState,
    scene_id: &str,
    kind: &str,
) -> AppResult<Option<sqlx::sqlite::SqliteRow>> {
    let row = sqlx::query(
        r#"
        SELECT
            p.product_id,
            p.kind,
            p.path,
            p.field_id AS product_field_id,
            p.season_id AS product_season_id,
            p.width_px AS product_width_px,
            p.height_px AS product_height_px,
            p.gsd_m_per_px AS product_gsd_m_per_px,
            p.spatial_ref_json AS product_spatial_ref_json,
            p.source_image_ids_json,
            p.source_scan_ids_json,
            p.publish_status,
            p.qa_report_ref,
            p.provenance_hash,
            p.downstream_consumers_json,
            p.open_data_license,
            p.open_data_attribution,
            p.open_data_anonymized,
            p.open_data_refusal_reason,
            p.open_data_published_at,
            s.scene_id,
            s.sensor,
            s.acquired_at,
            s.metadata_json,
            s.field_id,
            s.season_id,
            i.ingested_at,
            i.coverage_fraction,
            i.source_path AS ingest_source_path,
            sr.spatial_ref_json AS scene_spatial_ref_json,
            f.boundary_json AS field_boundary_json
        FROM products p
        JOIN scenes s ON s.scene_id = p.scene_id
        LEFT JOIN scene_ingests i ON i.scene_id = s.scene_id
        LEFT JOIN scene_spatial_refs sr ON sr.scene_id = s.scene_id
        LEFT JOIN fields f ON f.field_id = COALESCE(p.field_id, s.field_id)
        WHERE p.scene_id = ?1 AND lower(p.kind) = lower(?2)
        "#,
    )
    .bind(scene_id)
    .bind(kind.trim())
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;
    Ok(row)
}

fn layer_row_matches_query(row: &sqlx::sqlite::SqliteRow, query: &LayerListQuery) -> bool {
    if !optional_filter_matches(
        row.get::<Option<String>, _>("field_id"),
        query.field_id.as_ref(),
    ) {
        return false;
    }
    if !optional_filter_matches(
        row.get::<Option<String>, _>("season_id"),
        query.season_id.as_ref(),
    ) {
        return false;
    }
    if let Some(kind) = query.product_kind.as_ref().and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then_some(trimmed.to_ascii_lowercase())
    }) {
        let row_kind: String = row.get("kind");
        if row_kind.to_ascii_lowercase() != kind {
            return false;
        }
    }
    if let Some(date) = query.date.as_ref().and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    }) {
        let acquired_at: String = row.get("acquired_at");
        if !acquired_at.starts_with(date) {
            return false;
        }
    }

    true
}

fn optional_filter_matches(row_value: Option<String>, filter: Option<&String>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    let filter = filter.trim();
    if filter.is_empty() {
        return true;
    }
    row_value.as_deref() == Some(filter)
}

fn open_data_publish_error(error: OpenDataPublishError) -> AppError {
    match error {
        OpenDataPublishError::Refused { reason } => {
            AppError::BadRequest(format!("open_data_refused:{reason:?}").to_ascii_lowercase())
        }
    }
}

fn open_data_catalog_entry_from_layer(
    layer: &LayerMetadata,
    publication: &OpenDataPublication,
    published_at: Option<String>,
) -> OpenDataLayerCatalogEntry {
    OpenDataLayerCatalogEntry {
        open_data_id: publication.open_data_id.clone(),
        product_kind: layer.product_kind.clone(),
        license: publication.license.clone(),
        attribution: publication.attribution.clone(),
        anonymized: publication.anonymized,
        spatial_ref: layer.spatial_ref.clone(),
        url_path: layer.url_path.clone(),
        tile_url_template: layer.tile_url_template.clone(),
        published_at,
    }
}

async fn layer_from_row(
    row: &sqlx::sqlite::SqliteRow,
    strict: bool,
    stale_after_days: i64,
) -> AppResult<Option<LayerMetadata>> {
    let product_path = PathBuf::from(row.get::<String, _>("path"));
    if !fs::try_exists(&product_path)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        return if strict {
            Err(AppError::NotFound)
        } else {
            Ok(None)
        };
    }

    let scene_id: String = row.get("scene_id");
    let product_kind: String = row.get("kind");
    let dataset: String = row.get("sensor");
    let metadata_json: String = row.get("metadata_json");
    let image = serde_json::from_str::<MultispectralImage>(&metadata_json).map_err(|err| {
        AppError::Anyhow(
            Error::new(err).context("failed to decode layer scene metadata_json from database"),
        )
    })?;
    let width_px = optional_u32(row.get("product_width_px"))?;
    let height_px = optional_u32(row.get("product_height_px"))?;
    let gsd_m_per_px = row.get("product_gsd_m_per_px");
    let spatial_ref = match row.get::<Option<String>, _>("product_spatial_ref_json") {
        Some(spatial_ref_json) => {
            let decoded =
                serde_json::from_str::<RasterSpatialRef>(&spatial_ref_json).map_err(|err| {
                    AppError::Anyhow(Error::new(err).context("failed to decode layer spatial_ref"))
                })?;
            match (width_px, height_px) {
                (Some(width), Some(height)) => {
                    assert_raster_spatial_ref(Some(&decoded), width, height)
                        .map_err(|err| AppError::BadRequest(format!("metadata-integrity: {err}")))?
                }
                _ => decoded,
            }
        }
        None => {
            let Some(spatial_ref_json) = row.get::<Option<String>, _>("scene_spatial_ref_json")
            else {
                return if strict {
                    Err(AppError::BadRequest(format!(
                        "metadata-integrity: layer {scene_id}:{product_kind} has no asserted spatial_ref"
                    )))
                } else {
                    Ok(None)
                };
            };
            let spatial_ref =
                serde_json::from_str::<RasterSpatialRef>(&spatial_ref_json).map_err(|err| {
                    AppError::Anyhow(Error::new(err).context("failed to decode layer spatial_ref"))
                })?;
            if let Err(err) = assert_scene_spatial_ref_integrity(Some(&image), Some(&spatial_ref)) {
                return if strict { Err(err) } else { Ok(None) };
            }
            spatial_ref
        }
    };

    let source = row
        .get::<Option<String>, _>("ingest_source_path")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| row.get("sensor"));
    let url_path = format!("/api/scenes/{scene_id}/products/{product_kind}");
    let field_id = row
        .get::<Option<String>, _>("product_field_id")
        .or_else(|| row.get::<Option<String>, _>("field_id"));
    let season_id = row
        .get::<Option<String>, _>("product_season_id")
        .or_else(|| row.get::<Option<String>, _>("season_id"));
    let product_id = row
        .get::<Option<String>, _>("product_id")
        .filter(|value| !value.trim().is_empty());
    let acquired_at = row.get::<String, _>("acquired_at");
    let (field_coverage_fraction, field_coverage_status) = layer_field_coverage(row, &spatial_ref)?;

    Ok(Some(LayerMetadata {
        layer_id: format!("{scene_id}:{product_kind}"),
        product_id,
        scene_id,
        field_id,
        season_id,
        product_kind,
        dataset,
        width_px,
        height_px,
        gsd_m_per_px,
        spatial_ref,
        source_image_ids: decode_source_image_ids(row.get("source_image_ids_json"))?,
        source_scan_ids: decode_source_scan_ids(row.get("source_scan_ids_json"))?,
        publish_status: row.get("publish_status"),
        qa_report_ref: row.get("qa_report_ref"),
        provenance_hash: row.get("provenance_hash"),
        downstream_consumers: decode_downstream_consumers(row.get("downstream_consumers_json"))?,
        freshness: layer_freshness(
            acquired_at,
            row.get("ingested_at"),
            row.get("coverage_fraction"),
            stale_after_days,
            field_coverage_fraction,
            field_coverage_status,
        ),
        source,
        tile_url_template: format!("{url_path}/tiles/{{z}}/{{x}}/{{y}}.png"),
        url_path,
    }))
}

fn normalized_stale_after_days(value: Option<i64>) -> i64 {
    value
        .unwrap_or(DEFAULT_LAYER_STALE_AFTER_DAYS)
        .clamp(0, 3650)
}

fn layer_freshness(
    acquired_at: String,
    ingested_at: Option<String>,
    coverage_fraction: Option<f64>,
    stale_after_days: i64,
    field_coverage_fraction: Option<f64>,
    field_coverage_status: Option<String>,
) -> LayerFreshness {
    let age_days = chrono::DateTime::parse_from_rfc3339(&acquired_at)
        .ok()
        .map(|acquired| {
            chrono::Utc::now()
                .signed_duration_since(acquired.with_timezone(&chrono::Utc))
                .num_days()
        });
    let stale = age_days.is_some_and(|age| age > stale_after_days);

    LayerFreshness {
        acquired_at,
        ingested_at,
        coverage_fraction,
        stale_after_days,
        age_days,
        stale,
        field_coverage_fraction,
        field_coverage_status,
    }
}

fn layer_field_coverage(
    row: &sqlx::sqlite::SqliteRow,
    spatial_ref: &RasterSpatialRef,
) -> AppResult<(Option<f64>, Option<String>)> {
    let Some(boundary_json) = row.get::<Option<String>, _>("field_boundary_json") else {
        return Ok((None, None));
    };
    let Some(layer_bounds) = spatial_ref.bbox.as_ref() else {
        return Ok((None, None));
    };
    let boundary = serde_json::from_str::<FieldBoundary>(&boundary_json).map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode field boundary_json"))
    })?;
    let validated = match validate_field_boundary(&boundary) {
        Ok(validated) => validated,
        Err(_) => return Ok((None, Some("invalid_boundary".to_string()))),
    };

    let Some(layer_crs) = spatial_ref.crs.as_deref() else {
        return Ok((None, Some("missing_crs".to_string())));
    };

    if boundary
        .crs
        .as_deref()
        .map(str::trim)
        .filter(|crs| !crs.is_empty())
        != Some(layer_crs)
    {
        return Ok((None, Some("crs_mismatch".to_string())));
    }

    let fraction = bounds_coverage_fraction(&validated.extent, layer_bounds);
    let status = if fraction == 0.0 {
        "no_coverage"
    } else if fraction >= 0.999_999 {
        "full"
    } else {
        "partial"
    };

    Ok((Some(fraction), Some(status.to_string())))
}

async fn is_supported_product_file(entry: &DirEntry) -> AppResult<bool> {
    let file_type = entry
        .file_type()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    if !file_type.is_file() {
        return Ok(false);
    }

    let extension = entry
        .path()
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());
    Ok(matches!(
        extension.as_deref(),
        Some("png") | Some("jpg") | Some("jpeg") | Some("tif") | Some("tiff")
    ))
}

fn is_missing_scene_error(err: &anyhow::Error) -> bool {
    err.chain().any(|source| {
        source
            .downcast_ref::<sqlx::Error>()
            .is_some_and(|sqlx_err| matches!(sqlx_err, sqlx::Error::RowNotFound))
    })
}

fn is_product_publish_error(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|source| source.downcast_ref::<ProductPublishError>().is_some())
}

fn content_type_for_path(path: &FsPath) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("tif") | Some("tiff") => "image/tiff",
        _ => "application/octet-stream",
    }
}

fn is_png(path: &FsPath) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
}

async fn select_preferred_product_path(entries: &mut fs::ReadDir) -> AppResult<Option<PathBuf>> {
    let mut selected: Option<PathBuf> = None;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        if !is_supported_product_file(&entry).await? {
            continue;
        }

        let path = entry.path();
        match &selected {
            None => selected = Some(path),
            Some(current) => {
                if is_png(&path) && !is_png(current) {
                    selected = Some(path);
                }
            }
        }
    }

    Ok(selected)
}

fn build_product_summary(scene_id: &str, kind: &str, path: &FsPath) -> ProductSummary {
    ProductSummary {
        product_id: None,
        kind: kind.to_string(),
        field_id: None,
        season_id: None,
        filename: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string(),
        content_type: content_type_for_path(path).to_string(),
        width_px: None,
        height_px: None,
        gsd_m_per_px: None,
        spatial_ref: None,
        source_image_ids: Vec::new(),
        source_scan_ids: Vec::new(),
        publish_status: None,
        qa_report_ref: None,
        provenance_hash: None,
        downstream_consumers: Vec::new(),
        url_path: format!("/api/scenes/{scene_id}/products/{kind}"),
        tile_url_template: format!(
            "/api/scenes/{scene_id}/products/{kind}/tiles/{{z}}/{{x}}/{{y}}.png"
        ),
    }
}

fn product_summary_from_row(
    scene_id: &str,
    row: &sqlx::sqlite::SqliteRow,
    path: &FsPath,
) -> AppResult<ProductSummary> {
    let kind: String = row.get("kind");
    let spatial_ref = row
        .get::<Option<String>, _>("spatial_ref_json")
        .map(|json| {
            serde_json::from_str::<RasterSpatialRef>(&json).map_err(|err| {
                AppError::Anyhow(
                    Error::new(err).context("failed to decode product spatial_ref_json"),
                )
            })
        })
        .transpose()?;

    Ok(ProductSummary {
        product_id: row
            .get::<Option<String>, _>("product_id")
            .filter(|value| !value.trim().is_empty()),
        kind: kind.clone(),
        field_id: row
            .get::<Option<String>, _>("field_id")
            .filter(|value| !value.trim().is_empty()),
        season_id: row
            .get::<Option<String>, _>("season_id")
            .filter(|value| !value.trim().is_empty()),
        filename: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string(),
        content_type: content_type_for_path(path).to_string(),
        width_px: optional_u32(row.get("width_px"))?,
        height_px: optional_u32(row.get("height_px"))?,
        gsd_m_per_px: row.get("gsd_m_per_px"),
        spatial_ref,
        source_image_ids: decode_source_image_ids(row.get("source_image_ids_json"))?,
        source_scan_ids: decode_source_scan_ids(row.get("source_scan_ids_json"))?,
        publish_status: row.get("publish_status"),
        qa_report_ref: row.get("qa_report_ref"),
        provenance_hash: row.get("provenance_hash"),
        downstream_consumers: decode_downstream_consumers(row.get("downstream_consumers_json"))?,
        url_path: format!("/api/scenes/{scene_id}/products/{kind}"),
        tile_url_template: format!(
            "/api/scenes/{scene_id}/products/{kind}/tiles/{{z}}/{{x}}/{{y}}.png"
        ),
    })
}

fn decode_source_image_ids(value: Option<String>) -> AppResult<Vec<String>> {
    let Some(json) = value.filter(|value| !value.trim().is_empty()) else {
        return Ok(Vec::new());
    };
    serde_json::from_str::<Vec<String>>(&json).map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode product source_image_ids_json"))
    })
}

fn decode_source_scan_ids(value: Option<String>) -> AppResult<Vec<String>> {
    let Some(json) = value.filter(|value| !value.trim().is_empty()) else {
        return Ok(Vec::new());
    };
    serde_json::from_str::<Vec<String>>(&json).map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode product source_scan_ids_json"))
    })
}

fn decode_downstream_consumers(value: Option<String>) -> AppResult<Vec<String>> {
    let Some(json) = value.filter(|value| !value.trim().is_empty()) else {
        return Ok(Vec::new());
    };
    serde_json::from_str::<Vec<String>>(&json).map_err(|err| {
        AppError::Anyhow(
            Error::new(err).context("failed to decode product downstream_consumers_json"),
        )
    })
}

fn optional_u32(value: Option<i64>) -> AppResult<Option<u32>> {
    value
        .map(|value| {
            u32::try_from(value).map_err(|_| {
                AppError::BadRequest("product raster dimensions are invalid".to_string())
            })
        })
        .transpose()
}

async fn assert_scene_product_spatial_integrity(state: &AppState, scene_id: &str) -> AppResult<()> {
    let scene_row =
        sqlx::query("SELECT scene_id, metadata_json, field_id FROM scenes WHERE scene_id = ?1")
            .bind(scene_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(Error::from)?;
    let Some(scene_row) = scene_row else {
        return Ok(());
    };
    let scene_dir = state.config.data_root.join("scenes").join(scene_id);
    let metadata = load_scene_metadata(Some(&scene_row), &scene_dir).await?;
    let asserted_spatial_ref = ingest::load_scene_spatial_ref(&state.pool, scene_id).await?;
    assert_scene_spatial_ref_integrity(metadata.as_ref(), asserted_spatial_ref.as_ref())
}

fn assert_scene_spatial_ref_integrity(
    metadata: Option<&MultispectralImage>,
    asserted_spatial_ref: Option<&RasterSpatialRef>,
) -> AppResult<()> {
    let (Some(image), Some(asserted_spatial_ref)) = (metadata, asserted_spatial_ref) else {
        return Ok(());
    };
    let metadata_spatial_ref = assert_raster_spatial_ref(
        image.metadata.spatial_ref.as_ref(),
        image.metadata.width,
        image.metadata.height,
    )
    .map_err(|err| AppError::BadRequest(format!("metadata-integrity: {err}")))?;
    assert_spatial_refs_equivalent(&metadata_spatial_ref, asserted_spatial_ref)
}

fn assert_spatial_refs_equivalent(
    metadata_spatial_ref: &RasterSpatialRef,
    asserted_spatial_ref: &RasterSpatialRef,
) -> AppResult<()> {
    if metadata_spatial_ref.crs != asserted_spatial_ref.crs {
        return Err(metadata_integrity_mismatch("CRS"));
    }
    match (
        metadata_spatial_ref.bbox.as_ref(),
        asserted_spatial_ref.bbox.as_ref(),
    ) {
        (Some(left), Some(right)) => {
            assert_close("min_lon", left.min_lon, right.min_lon)?;
            assert_close("min_lat", left.min_lat, right.min_lat)?;
            assert_close("max_lon", left.max_lon, right.max_lon)?;
            assert_close("max_lat", left.max_lat, right.max_lat)?;
        }
        _ => return Err(metadata_integrity_mismatch("extent bbox")),
    }
    match (
        metadata_spatial_ref.resolution,
        asserted_spatial_ref.resolution,
    ) {
        (Some(left), Some(right)) => {
            assert_close("resolution.x", left.x, right.x)?;
            assert_close("resolution.y", left.y, right.y)?;
        }
        _ => return Err(metadata_integrity_mismatch("resolution")),
    }
    match (
        metadata_spatial_ref.geo_transform,
        asserted_spatial_ref.geo_transform,
    ) {
        (Some(left), Some(right)) => {
            for (index, (left, right)) in left.iter().zip(right.iter()).enumerate() {
                assert_close(&format!("geo_transform[{index}]"), *left, *right)?;
            }
        }
        _ => return Err(metadata_integrity_mismatch("transform")),
    }
    Ok(())
}

fn assert_close(label: &str, left: f64, right: f64) -> AppResult<()> {
    if (left - right).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE {
        Ok(())
    } else {
        Err(metadata_integrity_mismatch(label))
    }
}

fn metadata_integrity_mismatch(label: &str) -> AppError {
    AppError::BadRequest(format!(
        "metadata-integrity: persisted spatial_ref does not match scene metadata at {label}"
    ))
}

fn build_geospatial_metadata(metadata: Option<&MultispectralImage>) -> SceneGeospatialMetadata {
    build_geospatial_metadata_with_asserted(metadata, None)
}

fn season_id_for_linked_field(field: &FieldRecord) -> AppResult<String> {
    field
        .season
        .as_deref()
        .map(str::trim)
        .filter(|season| !season.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            AppError::BadRequest(
                "scene-field-season linkage requires the field to have a season".to_string(),
            )
        })
}

fn scene_extent_for_link(
    metadata: Option<&MultispectralImage>,
    asserted_spatial_ref: Option<&RasterSpatialRef>,
) -> Option<SceneExtent> {
    build_geospatial_metadata_with_asserted(metadata, asserted_spatial_ref).extent
}

fn scene_extent_intersects_bounds(scene_extent: &SceneExtent, field_bounds: &GeoBounds) -> bool {
    scene_extent.min_lon <= field_bounds.max_lon
        && scene_extent.max_lon >= field_bounds.min_lon
        && scene_extent.min_lat <= field_bounds.max_lat
        && scene_extent.max_lat >= field_bounds.min_lat
}

fn build_geospatial_metadata_with_asserted(
    metadata: Option<&MultispectralImage>,
    asserted_spatial_ref: Option<&RasterSpatialRef>,
) -> SceneGeospatialMetadata {
    let spatial_ref = asserted_spatial_ref
        .or_else(|| metadata.and_then(|image| image.metadata.spatial_ref.as_ref()));
    let extent = spatial_ref.and_then(|spatial| {
        spatial.bbox.as_ref().map(|bbox| SceneExtent {
            min_lon: bbox.min_lon,
            min_lat: bbox.min_lat,
            max_lon: bbox.max_lon,
            max_lat: bbox.max_lat,
        })
    });
    let center = extent.as_ref().map(|bbox| GpsCoords {
        latitude: (bbox.min_lat + bbox.max_lat) / 2.0,
        longitude: (bbox.min_lon + bbox.max_lon) / 2.0,
        altitude: metadata
            .and_then(|image| image.metadata.gps_position.as_ref())
            .map(|gps| gps.altitude)
            .unwrap_or(0.0),
    });

    SceneGeospatialMetadata {
        georeferenced: spatial_ref.is_some_and(|spatial| spatial.georeferenced),
        crs: spatial_ref.and_then(|spatial| spatial.crs.clone()),
        center: center.or_else(|| metadata.and_then(|image| image.metadata.gps_position.clone())),
        extent,
        spatial_ref: spatial_ref.cloned(),
    }
}

fn build_field_record(mut request: CreateFieldRequest) -> AppResult<FieldRecord> {
    let field_id = request
        .field_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("field name is required".to_string()));
    }
    let org_id = normalize_org_id(request.org_id.take(), request.owner.take());
    request.boundary.crs = request.boundary.crs.as_deref().and_then(normalize_crs_text);
    if request.boundary.coordinates.len() < 3 {
        return Err(AppError::BadRequest(
            "field boundary must contain at least three coordinates".to_string(),
        ));
    }
    if request.boundary.coordinates.iter().any(|point| {
        !point.longitude.is_finite()
            || !point.latitude.is_finite()
            || point.longitude < -180.0
            || point.longitude > 180.0
            || point.latitude < -90.0
            || point.latitude > 90.0
    }) {
        return Err(AppError::BadRequest(
            "field boundary contains invalid geographic coordinates".to_string(),
        ));
    }

    let extent = bounds_from_points(&request.boundary.coordinates).ok_or_else(|| {
        AppError::BadRequest("field boundary must contain valid coordinates".to_string())
    })?;

    let created_at = current_record_timestamp();
    Ok(FieldRecord {
        farm_id: request.farm_id,
        field_id,
        org_id: org_id.clone(),
        owner: org_id,
        name,
        area_ha: None,
        crop: request.crop,
        season: request.season,
        notes: request.notes,
        boundary: request.boundary,
        extent,
        status: request.status.unwrap_or_default(),
        created_at: created_at.clone(),
        updated_at: created_at,
    })
}

fn build_farm_record(request: CreateFarmRequest) -> AppResult<FarmRecord> {
    let farm_id = request
        .farm_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let org_id = normalize_org_id(request.org_id, request.owner);
    let created_at = current_record_timestamp();
    Ok(FarmRecord {
        farm_id,
        org_id: org_id.clone(),
        owner: org_id,
        name: normalize_farm_name(request.name)?,
        notes: normalize_optional_text(request.notes),
        status: request.status.unwrap_or_default(),
        created_at: created_at.clone(),
        updated_at: created_at,
    })
}

async fn build_annotation_record(
    state: &AppState,
    scene_id: &str,
    request: CreateAnnotationRequest,
) -> AppResult<AnnotationRecord> {
    let annotation_id = request
        .annotation_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let label = normalize_annotation_label(request.label)?;
    validate_annotation_geometry(&request.geometry)?;
    let field_id =
        normalize_optional_text(request.field_id).or(load_scene_field_id(state, scene_id).await?);
    let audit_id = normalize_optional_text(request.audit_id)
        .unwrap_or_else(|| format!("annotation-audit-{}", Uuid::new_v4()));

    let timestamp = chrono::Utc::now().to_rfc3339();
    Ok(AnnotationRecord {
        annotation_id,
        scene_id: scene_id.to_string(),
        field_id,
        author: normalize_optional_text(request.author),
        crs: normalize_optional_text(request.crs),
        audit_id: Some(audit_id),
        label,
        note: normalize_optional_text(request.note),
        severity: normalize_optional_text(request.severity),
        geometry: request.geometry,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    })
}

async fn build_recommendation_record(
    state: &AppState,
    scene_id: &str,
    request: CreateRecommendationRequest,
) -> AppResult<RecommendationRecord> {
    validate_recommendation_annotation_ids(state, scene_id, &request.annotation_ids).await?;

    let recommendation_id = request
        .recommendation_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = chrono::Utc::now().to_rfc3339();
    let category = normalize_optional_text(request.category);
    let action_category = normalize_optional_text(request.action_category)
        .or_else(|| category.clone())
        .unwrap_or_else(|| "general".to_string());
    let evidence_refs = combine_text_values(
        recommendation_evidence_from_annotations(&request.annotation_ids),
        request.evidence_refs,
    );

    Ok(RecommendationRecord {
        recommendation_id,
        scene_id: scene_id.to_string(),
        field_id: load_scene_field_id(state, scene_id).await?,
        org_id: DEFAULT_RECORD_OWNER.to_string(),
        author_user_id: normalize_optional_text(request.author_user_id)
            .unwrap_or_else(|| DEFAULT_RECORD_OWNER.to_string()),
        title: normalize_recommendation_title(request.title)?,
        note: normalize_optional_text(request.note),
        category,
        action_category,
        priority: request.priority.unwrap_or_default(),
        status: request.status.unwrap_or_default(),
        evidence_refs,
        annotation_ids: request.annotation_ids,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    })
}

async fn build_scene_report(
    state: &AppState,
    scene_id: &str,
    title: Option<String>,
    visibility: ReportVisibility,
) -> AppResult<ReportRecord> {
    let scene_row = sqlx::query(
        "SELECT scene_id, sensor, acquired_at, data_path, metadata_json, field_id FROM scenes WHERE scene_id = ?1",
    )
    .bind(scene_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;
    let scene_dir = state.config.data_root.join("scenes").join(scene_id);
    let metadata = load_scene_metadata(scene_row.as_ref(), &scene_dir).await?;
    let field = load_scene_field(state, scene_row.as_ref()).await?;
    let geospatial = build_geospatial_metadata(metadata.as_ref());
    let annotations = load_scene_annotation_records(state, scene_id).await?;
    let recommendations = load_scene_recommendation_records(state, scene_id).await?;
    let report_id = Uuid::new_v4().to_string();
    let report_title = title
        .and_then(|value| {
            let trimmed = value.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .unwrap_or_else(|| format!("Scene {} field intelligence report", scene_id));
    let report_dir = state.config.data_root.join("reports").join(scene_id);
    fs::create_dir_all(&report_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let artifact_path = report_dir.join(format!("{report_id}.html"));
    let html = render_scene_report_html(
        scene_id,
        scene_row.as_ref().map(|row| row.get("sensor")),
        scene_row.as_ref().map(|row| row.get("acquired_at")),
        metadata.as_ref(),
        field.as_ref(),
        &geospatial,
        &annotations,
        &recommendations,
        &report_title,
    );
    fs::write(&artifact_path, html)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let artifact_uri = artifact_path.to_string_lossy().to_string();
    let mut source_refs = vec![format!("scene:{scene_id}")];
    source_refs.extend(
        annotations
            .iter()
            .map(|annotation| format!("annotation:{}", annotation.annotation_id)),
    );
    source_refs.extend(
        recommendations
            .iter()
            .map(|recommendation| format!("recommendation:{}", recommendation.recommendation_id)),
    );

    Ok(ReportRecord {
        report_id: report_id.clone(),
        scene_id: scene_id.to_string(),
        field_id: field.as_ref().map(|field| field.field_id.clone()),
        season_id: None,
        org_id: field
            .as_ref()
            .map(|field| field.org_id.clone())
            .unwrap_or_else(|| DEFAULT_RECORD_OWNER.to_string()),
        generated_by: DEFAULT_RECORD_OWNER.to_string(),
        source_refs,
        title: report_title,
        format: ReportFormat::Html,
        artifact_path: artifact_uri.clone(),
        artifact_uri,
        download_url: format!("/api/scenes/{scene_id}/reports/{report_id}"),
        visibility,
        annotation_count: annotations.len(),
        recommendation_count: recommendations.len(),
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn normalize_annotation_label(label: String) -> AppResult<String> {
    let label = label.trim().to_string();
    if label.is_empty() {
        return Err(AppError::BadRequest(
            "annotation label is required".to_string(),
        ));
    }
    Ok(label)
}

fn fleet_enrollment_error(error: FleetNodeEnrollmentError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn tractor_registry_error(error: TractorRegistryError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn tractor_rejection_status(rejection: &TractorCommandRejection) -> StatusCode {
    StatusCode::from_u16(rejection.status_code()).unwrap_or(StatusCode::BAD_REQUEST)
}

fn weather_ingest_error(error: WeatherIngestError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn canonical_weather_field_ref(field_id: &str) -> String {
    format!("field:{field_id}")
}

fn parse_fleet_node_kind(value: String) -> AppResult<FleetNodeKind> {
    value.parse::<FleetNodeKind>().map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode fleet node kind"))
    })
}

fn parse_fleet_node_runtime_mode(value: String) -> AppResult<FleetNodeRuntimeMode> {
    value.parse::<FleetNodeRuntimeMode>().map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode fleet node runtime_mode"))
    })
}

fn parse_fleet_node_status(value: String) -> AppResult<FleetNodeStatus> {
    value.parse::<FleetNodeStatus>().map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode fleet node status"))
    })
}

fn parse_tractor_lifecycle_status(value: String) -> AppResult<TractorLifecycleStatus> {
    value
        .parse::<TractorLifecycleStatus>()
        .map_err(|err| AppError::Anyhow(Error::new(err).context("failed to decode tractor status")))
}

fn sample_weather_provider_response(
    provider: &str,
    fetched_at: String,
    valid_time: Option<String>,
) -> AppResult<WeatherProviderForecastResponse> {
    if !provider.eq_ignore_ascii_case("sample") {
        return Err(AppError::BadRequest(format!(
            "unsupported weather provider {provider}"
        )));
    }
    let valid_time = normalize_optional_text(valid_time)
        .unwrap_or_else(|| (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339());

    Ok(WeatherProviderForecastResponse {
        source: "sample".to_string(),
        fetched_at,
        points: vec![WeatherProviderForecastPoint {
            valid_time,
            temperature_celsius: 22.0,
            wind_speed_mps: 4.5,
            precipitation_mm: 0.2,
            humidity_percent: 63.0,
            radiation_w_m2: 710.0,
        }],
    })
}

fn fleet_health_error(error: FleetHealthError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn parse_fleet_component_type(value: String) -> AppResult<FleetComponentType> {
    value
        .parse::<FleetComponentType>()
        .map_err(fleet_health_error)
}

fn parse_fleet_health_indicator(value: String) -> AppResult<FleetHealthIndicator> {
    value
        .parse::<FleetHealthIndicator>()
        .map_err(fleet_health_error)
}

fn parse_health_indicator_freshness(value: String) -> AppResult<HealthIndicatorFreshness> {
    value
        .parse::<HealthIndicatorFreshness>()
        .map_err(fleet_health_error)
}

fn soil_iot_error(error: SoilIotError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn gateway_ingest_error(error: GatewayIngestError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn soil_moisture_error(error: SoilMoistureReadingError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn drought_index_error(error: DroughtIndexError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn marketplace_account_error(error: MarketplaceAccountError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn marketplace_catalog_error(error: MarketplaceCatalogError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn marketplace_portal_entry_error(error: MarketplacePortalEntryError) -> AppError {
    match error {
        MarketplacePortalEntryError::MissingAccount
        | MarketplacePortalEntryError::OrgMismatch { .. }
        | MarketplacePortalEntryError::AccountNotActive { .. }
        | MarketplacePortalEntryError::MissingMarketplaceRole { .. } => {
            AppError::Forbidden(error.to_string())
        }
        MarketplacePortalEntryError::EmptyOrgId => AppError::BadRequest(error.to_string()),
    }
}

fn marketplace_listing_error(error: MarketplaceListingError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn marketplace_inventory_error(error: MarketplaceInventoryError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn marketplace_order_error(error: MarketplaceOrderError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn sustainability_record_error(error: SustainabilityRecordError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn content_error(error: ContentError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn collaboration_error(error: CollaborationError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn parse_soil_sensor_type(value: String) -> AppResult<SoilSensorType> {
    value.parse::<SoilSensorType>().map_err(soil_iot_error)
}

fn parse_soil_device_status(value: String) -> AppResult<SoilDeviceStatus> {
    value.parse::<SoilDeviceStatus>().map_err(soil_iot_error)
}

fn parse_soil_config_push_status(value: String) -> AppResult<SoilDeviceConfigPushStatus> {
    value
        .parse::<SoilDeviceConfigPushStatus>()
        .map_err(soil_iot_error)
}

fn orthomosaic_ingest_error(error: FrameSetIngestError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn reconstruction_job_error(error: ReconstructionJobError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn tiled_output_handoff_error(error: TiledOutputHandoffError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn mosaic_publish_gate_error(error: MosaicPublishGateError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn copilot_conversation_error(error: CopilotConversationError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn parse_reconstruction_status(value: String) -> AppResult<ReconstructionStatus> {
    value.parse::<ReconstructionStatus>().map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode reconstruction status"))
    })
}

fn crop_model_registry_error(error: CropModelRegistryError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn crop_inference_run_error(error: InferenceRunError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn crop_detection_verification_error(error: CropDetectionVerificationError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn finding_promotion_error(error: FindingPromotionError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn crop_detection_finding_error(error: CropDetectionFindingError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn parse_crop_model_task(value: String) -> AppResult<CropModelTask> {
    value
        .parse::<CropModelTask>()
        .map_err(crop_model_registry_error)
}

fn parse_copilot_turn_role(value: String) -> AppResult<CopilotTurnRole> {
    value
        .parse::<CopilotTurnRole>()
        .map_err(copilot_conversation_error)
}

fn parse_detection_verification_state(value: String) -> AppResult<DetectionVerificationState> {
    value
        .parse::<DetectionVerificationState>()
        .map_err(crop_detection_verification_error)
}

fn compliance_record_error(error: ComplianceRecordError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn compliance_audit_report_error(error: ComplianceAuditReportError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn parse_compliance_record_type(value: String) -> AppResult<ComplianceRecordType> {
    value
        .parse::<ComplianceRecordType>()
        .map_err(compliance_record_error)
}

fn alerting_error(error: AlertingError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn plugin_registration_error(error: PluginRegistrationError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn plugin_lifecycle_error(error: PluginLifecycleError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn airspace_zone_error(error: AirspaceZoneError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn parse_airspace_zone_class(value: String) -> AppResult<AirspaceZoneClass> {
    value
        .parse::<AirspaceZoneClass>()
        .map_err(airspace_zone_error)
}

fn normalize_farm_name(name: String) -> AppResult<String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("farm name is required".to_string()));
    }
    Ok(name)
}

fn normalize_recommendation_title(title: String) -> AppResult<String> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::BadRequest(
            "recommendation title is required".to_string(),
        ));
    }
    Ok(title)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn combine_text_values(first: Vec<String>, second: Vec<String>) -> Vec<String> {
    let mut combined = Vec::new();
    let mut seen = BTreeSet::new();
    for value in first.into_iter().chain(second.into_iter()) {
        let Some(value) = normalize_optional_text(Some(value)) else {
            continue;
        };
        if seen.insert(value.clone()) {
            combined.push(value);
        }
    }
    combined
}

fn recommendation_evidence_from_annotations(annotation_ids: &[String]) -> Vec<String> {
    annotation_ids
        .iter()
        .filter_map(|annotation_id| normalize_optional_text(Some(annotation_id.clone())))
        .map(|annotation_id| format!("annotation:{}", annotation_id))
        .collect::<Vec<_>>()
}

fn normalize_org_id(org_id: Option<String>, owner: Option<String>) -> String {
    normalize_optional_text(org_id)
        .or_else(|| normalize_optional_text(owner))
        .unwrap_or_else(|| DEFAULT_RECORD_OWNER.to_string())
}

fn current_record_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn fields_from_geojson(geojson: GeoJson) -> AppResult<Vec<FieldRecord>> {
    match geojson {
        GeoJson::FeatureCollection(collection) => collection
            .features
            .into_iter()
            .enumerate()
            .map(|(index, feature)| build_field_from_feature(feature, index))
            .collect(),
        GeoJson::Feature(feature) => Ok(vec![build_field_from_feature(feature, 0)?]),
        GeoJson::Geometry(geometry) => Ok(vec![build_field_from_geometry(geometry, None, 0)?]),
    }
}

async fn fields_from_shapefile(request: ImportShapefileRequest) -> AppResult<Vec<FieldRecord>> {
    let path = PathBuf::from(request.path.trim());
    if path.as_os_str().is_empty() {
        return Err(AppError::BadRequest(
            "shapefile path is required".to_string(),
        ));
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(|ext| !ext.eq_ignore_ascii_case("shp"))
        .unwrap_or(true)
    {
        return Err(AppError::BadRequest(
            "shapefile import currently requires a .shp path".to_string(),
        ));
    }

    let bytes = fs::read(&path).await.map_err(|err| {
        AppError::BadRequest(format!(
            "failed to read shapefile {}: {err}",
            path.display()
        ))
    })?;
    let source_crs = resolve_shapefile_crs(&path, request.crs.as_deref()).await?;
    let shapes = shapefile::parse_polygon_records(&path, &bytes)?;
    let base_name = request
        .name_prefix
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "Imported Field".to_string());
    let single_shape = shapes.len() == 1;

    shapes
        .into_iter()
        .map(|shape| {
            let shape_name = if single_shape {
                base_name.clone()
            } else {
                format!("{} {}", base_name, shape.record_index + 1)
            };
            build_field_record(CreateFieldRequest {
                farm_id: request.farm_id.clone(),
                field_id: None,
                org_id: request.owner.clone(),
                owner: request.owner.clone(),
                name: shape_name,
                crop: request.crop.clone(),
                season: request.season.clone(),
                notes: request.notes.clone(),
                status: None,
                boundary: FieldBoundary {
                    coordinates: shape.coordinates,
                    crs: Some(source_crs.clone()),
                },
            })
        })
        .collect()
}

async fn resolve_shapefile_crs(path: &FsPath, supplied_crs: Option<&str>) -> AppResult<String> {
    if let Some(crs) = supplied_crs.and_then(normalize_crs_text) {
        return require_supported_boundary_crs(path, crs);
    }

    let prj_path = path.with_extension("prj");
    let prj_text = fs::read_to_string(&prj_path).await.map_err(|err| {
        if err.kind() == ErrorKind::NotFound {
            AppError::BadRequest(format!(
                "missing CRS for shapefile {}; provide a .prj file or crs in the import request",
                path.display()
            ))
        } else {
            AppError::BadRequest(format!(
                "failed to read shapefile CRS {}: {err}",
                prj_path.display()
            ))
        }
    })?;
    let crs = normalize_crs_text(&prj_text).ok_or_else(|| {
        AppError::BadRequest(format!(
            "missing CRS for shapefile {}; .prj is empty",
            path.display()
        ))
    })?;
    require_supported_boundary_crs(path, crs)
}

fn require_supported_boundary_crs(path: &FsPath, crs: String) -> AppResult<String> {
    if crs == "EPSG:4326" {
        Ok(crs)
    } else {
        Err(AppError::BadRequest(format!(
            "shapefile {} CRS {crs} is not supported; import currently requires EPSG:4326 lon/lat coordinates",
            path.display()
        )))
    }
}

fn group_fields_by_season(fields: Vec<FieldRecord>) -> Vec<FieldSeasonGroup> {
    let mut grouped: BTreeMap<Option<String>, Vec<FieldRecord>> = BTreeMap::new();
    for field in fields {
        grouped.entry(field.season.clone()).or_default().push(field);
    }

    grouped
        .into_iter()
        .rev()
        .map(|(season, fields)| FieldSeasonGroup { season, fields })
        .collect()
}

fn geojson_from_fields(fields: Vec<FieldRecord>) -> GeoJson {
    GeoJson::FeatureCollection(FeatureCollection {
        bbox: None,
        foreign_members: None,
        features: fields.into_iter().map(feature_from_field).collect(),
    })
}

fn response_with_bytes(bytes: Vec<u8>, content_type: &str, filename: &str) -> AppResult<Response> {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type).map_err(|err| AppError::Anyhow(err.into()))?,
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .map_err(|err| AppError::Anyhow(err.into()))?,
    );

    Ok((headers, Body::from(bytes)).into_response())
}

fn feature_from_field(field: FieldRecord) -> Feature {
    let mut ring: Vec<Vec<f64>> = field
        .boundary
        .coordinates
        .iter()
        .map(|point| vec![point.longitude, point.latitude])
        .collect();
    if let Some(first) = ring.first().cloned() {
        if ring.last() != Some(&first) {
            ring.push(first);
        }
    }

    let mut properties = serde_json::Map::new();
    properties.insert(
        "field_id".to_string(),
        serde_json::Value::String(field.field_id.clone()),
    );
    properties.insert(
        "owner".to_string(),
        serde_json::Value::String(field.owner.clone()),
    );
    properties.insert(
        "org_id".to_string(),
        serde_json::Value::String(field.org_id.clone()),
    );
    properties.insert(
        "created_at".to_string(),
        serde_json::Value::String(field.created_at.clone()),
    );
    if let Some(farm_id) = field.farm_id {
        properties.insert("farm_id".to_string(), serde_json::Value::String(farm_id));
    }
    if let Some(area_ha) = field.area_ha {
        properties.insert("area_ha".to_string(), serde_json::Value::from(area_ha));
    }
    properties.insert("name".to_string(), serde_json::Value::String(field.name));
    if let Some(crs) = field.boundary.crs.as_ref() {
        properties.insert("crs".to_string(), serde_json::Value::String(crs.clone()));
    }
    if let Some(crop) = field.crop {
        properties.insert("crop".to_string(), serde_json::Value::String(crop));
    }
    if let Some(season) = field.season {
        properties.insert("season".to_string(), serde_json::Value::String(season));
    }
    if let Some(notes) = field.notes {
        properties.insert("notes".to_string(), serde_json::Value::String(notes));
    }

    Feature {
        bbox: None,
        geometry: Some(Geometry::new(GeoJsonValue::Polygon(vec![ring]))),
        id: Some(GeoJsonId::String(field.field_id)),
        properties: Some(properties),
        foreign_members: None,
    }
}

fn feature_from_annotation(annotation: &AnnotationRecord) -> AppResult<Feature> {
    let mut properties = serde_json::Map::new();
    properties.insert(
        "annotation_id".to_string(),
        serde_json::Value::String(annotation.annotation_id.clone()),
    );
    properties.insert(
        "scene_id".to_string(),
        serde_json::Value::String(annotation.scene_id.clone()),
    );
    if let Some(field_id) = annotation.field_id.as_ref() {
        properties.insert(
            "field_id".to_string(),
            serde_json::Value::String(field_id.clone()),
        );
    }
    if let Some(author) = annotation.author.as_ref() {
        properties.insert(
            "author".to_string(),
            serde_json::Value::String(author.clone()),
        );
    }
    if let Some(crs) = annotation.crs.as_ref() {
        properties.insert("crs".to_string(), serde_json::Value::String(crs.clone()));
    }
    if let Some(audit_id) = annotation.audit_id.as_ref() {
        properties.insert(
            "audit_id".to_string(),
            serde_json::Value::String(audit_id.clone()),
        );
    }
    properties.insert(
        "label".to_string(),
        serde_json::Value::String(annotation.label.clone()),
    );
    properties.insert(
        "geometry_type".to_string(),
        serde_json::Value::String(annotation_geometry_type(&annotation.geometry).to_string()),
    );
    if let Some(severity) = annotation.severity.as_ref() {
        properties.insert(
            "severity".to_string(),
            serde_json::Value::String(severity.clone()),
        );
    }
    if let Some(note) = annotation.note.as_ref() {
        properties.insert("note".to_string(), serde_json::Value::String(note.clone()));
    }
    properties.insert(
        "created_at".to_string(),
        serde_json::Value::String(annotation.created_at.clone()),
    );
    properties.insert(
        "updated_at".to_string(),
        serde_json::Value::String(annotation.updated_at.clone()),
    );

    Ok(Feature {
        bbox: None,
        geometry: Some(geometry_from_annotation(&annotation.geometry)?),
        id: Some(GeoJsonId::String(annotation.annotation_id.clone())),
        properties: Some(properties),
        foreign_members: None,
    })
}

fn recommendation_features(
    recommendation: &RecommendationRecord,
    annotations: &[AnnotationRecord],
) -> AppResult<Vec<Feature>> {
    if recommendation.annotation_ids.is_empty() {
        let mut properties = serde_json::Map::new();
        populate_recommendation_properties(&mut properties, recommendation);
        return Ok(vec![Feature {
            bbox: None,
            geometry: None,
            id: Some(GeoJsonId::String(recommendation.recommendation_id.clone())),
            properties: Some(properties),
            foreign_members: None,
        }]);
    }

    let mut features = Vec::new();
    for annotation_id in &recommendation.annotation_ids {
        if let Some(annotation) = annotations
            .iter()
            .find(|annotation| annotation.annotation_id == *annotation_id)
        {
            let mut properties = serde_json::Map::new();
            populate_recommendation_properties(&mut properties, recommendation);
            properties.insert(
                "annotation_id".to_string(),
                serde_json::Value::String(annotation.annotation_id.clone()),
            );
            if !properties.contains_key("field_id") {
                if let Some(field_id) = annotation.field_id.as_ref() {
                    properties.insert(
                        "field_id".to_string(),
                        serde_json::Value::String(field_id.clone()),
                    );
                }
            }
            if let Some(crs) = annotation.crs.as_ref() {
                properties.insert("crs".to_string(), serde_json::Value::String(crs.clone()));
            }
            features.push(Feature {
                bbox: None,
                geometry: Some(geometry_from_annotation(&annotation.geometry)?),
                id: Some(GeoJsonId::String(format!(
                    "{}:{}",
                    recommendation.recommendation_id, annotation.annotation_id
                ))),
                properties: Some(properties),
                foreign_members: None,
            });
        }
    }

    Ok(features)
}

fn recommendation_export_field_id(
    recommendation: &RecommendationRecord,
    annotations: &[AnnotationRecord],
) -> Option<String> {
    if let Some(field_id) = recommendation
        .field_id
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        return Some(field_id.clone());
    }

    let mut linked_field_ids = BTreeSet::new();
    for annotation_id in &recommendation.annotation_ids {
        if let Some(field_id) = annotations
            .iter()
            .find(|annotation| annotation.annotation_id == *annotation_id)
            .and_then(|annotation| annotation.field_id.as_ref())
            .filter(|value| !value.trim().is_empty())
        {
            linked_field_ids.insert(field_id.clone());
        }
    }

    if linked_field_ids.len() == 1 {
        linked_field_ids.into_iter().next()
    } else {
        None
    }
}

fn populate_recommendation_properties(
    properties: &mut serde_json::Map<String, serde_json::Value>,
    recommendation: &RecommendationRecord,
) {
    properties.insert(
        "recommendation_id".to_string(),
        serde_json::Value::String(recommendation.recommendation_id.clone()),
    );
    properties.insert(
        "scene_id".to_string(),
        serde_json::Value::String(recommendation.scene_id.clone()),
    );
    if let Some(field_id) = recommendation.field_id.as_ref() {
        properties.insert(
            "field_id".to_string(),
            serde_json::Value::String(field_id.clone()),
        );
    }
    properties.insert(
        "org_id".to_string(),
        serde_json::Value::String(recommendation.org_id.clone()),
    );
    properties.insert(
        "author_user_id".to_string(),
        serde_json::Value::String(recommendation.author_user_id.clone()),
    );
    properties.insert(
        "title".to_string(),
        serde_json::Value::String(recommendation.title.clone()),
    );
    properties.insert(
        "priority".to_string(),
        serde_json::Value::String(recommendation_priority_str(recommendation.priority).to_string()),
    );
    properties.insert(
        "status".to_string(),
        serde_json::Value::String(recommendation_status_str(recommendation.status).to_string()),
    );
    properties.insert(
        "action_category".to_string(),
        serde_json::Value::String(recommendation.action_category.clone()),
    );
    properties.insert(
        "evidence_refs".to_string(),
        serde_json::Value::Array(
            recommendation
                .evidence_refs
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        ),
    );
    properties.insert(
        "annotation_ids".to_string(),
        serde_json::Value::Array(
            recommendation
                .annotation_ids
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        ),
    );
    if let Some(category) = recommendation.category.as_ref() {
        properties.insert(
            "category".to_string(),
            serde_json::Value::String(category.clone()),
        );
    }
    if let Some(note) = recommendation.note.as_ref() {
        properties.insert("note".to_string(), serde_json::Value::String(note.clone()));
    }
    properties.insert(
        "created_at".to_string(),
        serde_json::Value::String(recommendation.created_at.clone()),
    );
    properties.insert(
        "updated_at".to_string(),
        serde_json::Value::String(recommendation.updated_at.clone()),
    );
}

fn feature_collection_with_crs(features: Vec<Feature>, crs: &str) -> GeoJson {
    let mut crs_properties = serde_json::Map::new();
    crs_properties.insert(
        "name".to_string(),
        serde_json::Value::String(crs.to_string()),
    );
    let mut crs_object = serde_json::Map::new();
    crs_object.insert(
        "type".to_string(),
        serde_json::Value::String("name".to_string()),
    );
    crs_object.insert(
        "properties".to_string(),
        serde_json::Value::Object(crs_properties),
    );
    let mut foreign_members = serde_json::Map::new();
    foreign_members.insert("crs".to_string(), serde_json::Value::Object(crs_object));

    GeoJson::FeatureCollection(FeatureCollection {
        bbox: None,
        foreign_members: Some(foreign_members),
        features,
    })
}

fn collection_crs_from_annotations(annotations: &[AnnotationRecord]) -> AppResult<String> {
    let mut collection_crs = None;
    for annotation in annotations {
        let Some(raw_crs) = annotation
            .crs
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let normalized = normalize_geojson_crs(Some(raw_crs.to_string()))?;
        if let Some(existing) = collection_crs.as_ref() {
            if existing != &normalized {
                return Err(AppError::BadRequest(
                    "GeoJSON export requires a single annotation CRS".to_string(),
                ));
            }
        } else {
            collection_crs = Some(normalized);
        }
    }

    Ok(collection_crs.unwrap_or_else(|| "EPSG:4326".to_string()))
}

fn field_record_crs(field: &FieldRecord) -> String {
    field
        .boundary
        .crs
        .clone()
        .unwrap_or_else(|| "EPSG:4326".to_string())
}

fn assert_field_bundle_annotation_crs(
    annotations: &[AnnotationRecord],
    field_crs: &str,
) -> AppResult<()> {
    let field_crs = normalize_geojson_crs(Some(field_crs.to_string()))?;
    for annotation in annotations {
        let annotation_crs = annotation
            .crs
            .as_ref()
            .map(|crs| normalize_geojson_crs(Some(crs.clone())))
            .transpose()?
            .unwrap_or_else(|| field_crs.clone());
        if annotation_crs != field_crs {
            return Err(AppError::BadRequest(format!(
                "field export requires annotation {} CRS {} to match field CRS {}",
                annotation.annotation_id, annotation_crs, field_crs
            )));
        }
    }
    Ok(())
}

fn annotation_geometry_type(geometry: &AnnotationGeometry) -> &'static str {
    match geometry {
        AnnotationGeometry::Point { .. } => "point",
        AnnotationGeometry::Polygon { .. } => "polygon",
    }
}

fn geometry_from_annotation(geometry: &AnnotationGeometry) -> AppResult<Geometry> {
    Ok(match geometry {
        AnnotationGeometry::Point { coordinate } => Geometry::new(GeoJsonValue::Point(vec![
            coordinate.longitude,
            coordinate.latitude,
        ])),
        AnnotationGeometry::Polygon { coordinates } => {
            let mut ring = coordinates
                .iter()
                .map(|coordinate| vec![coordinate.longitude, coordinate.latitude])
                .collect::<Vec<_>>();
            if let Some(first) = ring.first().cloned() {
                ring.push(first);
            }
            Geometry::new(GeoJsonValue::Polygon(vec![ring]))
        }
    })
}

fn validate_annotation_geometry(geometry: &AnnotationGeometry) -> AppResult<()> {
    match geometry {
        AnnotationGeometry::Point { coordinate } => {
            validate_geo_point(coordinate)?;
        }
        AnnotationGeometry::Polygon { coordinates } => {
            if coordinates.len() < 3 {
                return Err(AppError::BadRequest(
                    "polygon annotation must contain at least three coordinates".to_string(),
                ));
            }
            for coordinate in coordinates {
                validate_geo_point(coordinate)?;
            }
        }
    }
    Ok(())
}

fn validate_geo_point(point: &GeoPoint) -> AppResult<()> {
    if !point.longitude.is_finite()
        || !point.latitude.is_finite()
        || point.longitude < -180.0
        || point.longitude > 180.0
        || point.latitude < -90.0
        || point.latitude > 90.0
    {
        return Err(AppError::BadRequest(
            "annotation geometry contains invalid geographic coordinates".to_string(),
        ));
    }

    Ok(())
}

fn build_field_from_feature(feature: geojson::Feature, index: usize) -> AppResult<FieldRecord> {
    let geojson::Feature {
        geometry,
        id,
        properties,
        ..
    } = feature;
    let geometry = geometry
        .ok_or_else(|| AppError::BadRequest("GeoJSON feature is missing geometry".to_string()))?;
    let properties = properties.unwrap_or_default();

    let field_id = property_string(&properties, "field_id")
        .or_else(|| property_string(&properties, "id"))
        .or_else(|| id.as_ref().and_then(geojson_id_to_string));
    let name = property_string(&properties, "name")
        .or_else(|| property_string(&properties, "field_name"))
        .unwrap_or_else(|| format!("Imported Field {}", index + 1));
    let crs =
        property_string(&properties, "crs").or_else(|| property_string(&properties, "source_crs"));

    build_field_from_geometry(
        geometry,
        Some(CreateFieldRequest {
            farm_id: None,
            field_id,
            org_id: property_string(&properties, "org_id"),
            owner: property_string(&properties, "owner"),
            name,
            crop: property_string(&properties, "crop"),
            season: property_string(&properties, "season"),
            notes: property_string(&properties, "notes"),
            status: None,
            boundary: FieldBoundary {
                coordinates: Vec::new(),
                crs,
            },
        }),
        index,
    )
}

fn build_field_from_geometry(
    geometry: Geometry,
    template: Option<CreateFieldRequest>,
    index: usize,
) -> AppResult<FieldRecord> {
    let mut boundary = boundary_from_geometry(geometry)?;
    let template = template.unwrap_or(CreateFieldRequest {
        farm_id: None,
        field_id: None,
        org_id: None,
        owner: None,
        name: format!("Imported Field {}", index + 1),
        crop: None,
        season: None,
        notes: None,
        status: None,
        boundary: FieldBoundary {
            coordinates: Vec::new(),
            crs: None,
        },
    });
    boundary.crs = Some(normalize_geojson_crs(template.boundary.crs.clone())?);
    validate_field_boundary(&boundary)
        .map_err(|err| AppError::BadRequest(format!("invalid GeoJSON field boundary: {err}")))?;

    build_field_record(CreateFieldRequest {
        farm_id: template.farm_id,
        field_id: template.field_id,
        org_id: template.org_id,
        owner: template.owner,
        name: template.name,
        crop: template.crop,
        season: template.season,
        notes: template.notes,
        status: template.status,
        boundary,
    })
}

fn boundary_from_geometry(geometry: Geometry) -> AppResult<FieldBoundary> {
    match geometry.value {
        GeoJsonValue::Polygon(rings) => {
            let exterior = rings.into_iter().next().ok_or_else(|| {
                AppError::BadRequest(
                    "GeoJSON polygon does not contain an exterior ring".to_string(),
                )
            })?;
            boundary_from_ring(exterior)
        }
        GeoJsonValue::MultiPolygon(polygons) => {
            let exterior = polygons
                .into_iter()
                .max_by_key(|polygon| polygon.first().map_or(0, Vec::len))
                .and_then(|polygon| polygon.into_iter().next())
                .ok_or_else(|| {
                    AppError::BadRequest(
                        "GeoJSON multipolygon does not contain a usable exterior ring".to_string(),
                    )
                })?;
            boundary_from_ring(exterior)
        }
        _ => Err(AppError::BadRequest(
            "only Polygon and MultiPolygon GeoJSON geometries are supported".to_string(),
        )),
    }
}

fn boundary_from_ring(ring: Vec<Vec<f64>>) -> AppResult<FieldBoundary> {
    let mut coordinates = Vec::with_capacity(ring.len());
    for position in ring {
        if position.len() < 2 {
            return Err(AppError::BadRequest(
                "GeoJSON polygon coordinates must contain longitude and latitude".to_string(),
            ));
        }
        coordinates.push(GeoPoint {
            longitude: position[0],
            latitude: position[1],
        });
    }

    Ok(FieldBoundary {
        coordinates,
        crs: None,
    })
}

fn normalize_geojson_crs(value: Option<String>) -> AppResult<String> {
    let Some(value) = value else {
        return Ok("EPSG:4326".to_string());
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok("EPSG:4326".to_string());
    }
    let upper = trimmed.to_ascii_uppercase();
    if upper == "EPSG:4326"
        || upper == "CRS84"
        || upper.contains("OGC:1.3:CRS84")
        || upper.contains("WGS 84")
        || upper.contains("WGS_1984")
    {
        return Ok("EPSG:4326".to_string());
    }

    Err(AppError::BadRequest(format!(
        "unsupported GeoJSON CRS: {trimmed}"
    )))
}

fn normalize_crs_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let upper = trimmed.to_ascii_uppercase();
    if upper.contains("EPSG:4326")
        || upper.contains("\"EPSG\",\"4326\"")
        || ((upper.contains("GEOGCS") || upper.contains("GEOGCRS"))
            && !upper.contains("PROJCS")
            && !upper.contains("PROJCRS")
            && (upper.contains("WGS 84") || upper.contains("WGS_1984")))
    {
        Some("EPSG:4326".to_string())
    } else {
        Some(trimmed.to_string())
    }
}

fn property_string(
    properties: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    properties.get(key).and_then(|value| match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    })
}

fn geojson_id_to_string(id: &GeoJsonId) -> Option<String> {
    match id {
        GeoJsonId::String(text) => Some(text.clone()),
        GeoJsonId::Number(number) => Some(number.to_string()),
    }
}

fn decode_field_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<FieldRecord> {
    let boundary_json: String = row.get("boundary_json");
    let boundary = serde_json::from_str::<FieldBoundary>(&boundary_json).map_err(|err| {
        AppError::Anyhow(anyhow::Error::new(err).context("failed to decode field boundary_json"))
    })?;
    let extent = bounds_from_points(&boundary.coordinates).ok_or_else(|| {
        AppError::Anyhow(anyhow::anyhow!(
            "field boundary does not contain any coordinates"
        ))
    })?;

    Ok(FieldRecord {
        farm_id: row.get("farm_id"),
        field_id: row.get("field_id"),
        org_id: row.get("owner"),
        owner: row.get("owner"),
        name: row.get("name"),
        area_ha: None,
        crop: row.get("crop"),
        season: row.get("season"),
        notes: row.get("notes"),
        boundary,
        extent,
        status: decode_farm_field_status(row.get("status")),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_farm_record(row: &sqlx::sqlite::SqliteRow) -> FarmRecord {
    FarmRecord {
        farm_id: row.get("farm_id"),
        org_id: row.get("owner"),
        owner: row.get("owner"),
        name: row.get("name"),
        notes: row.get("notes"),
        status: decode_farm_field_status(row.get("status")),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn decode_farm_field_status(value: String) -> FarmFieldEntityStatus {
    match value.trim() {
        "archived" => FarmFieldEntityStatus::Archived,
        _ => FarmFieldEntityStatus::Active,
    }
}

fn field_boundary_record_from_field(field: FieldRecord) -> FieldBoundaryRecord {
    FieldBoundaryRecord {
        field_id: field.field_id,
        farm_id: field.farm_id,
        org_id: field.org_id,
        owner: field.owner,
        name: field.name,
        boundary: field.boundary,
        extent: field.extent,
        area_ha: field.area_ha,
        status: field.status,
        created_at: field.created_at,
        updated_at: field.updated_at,
    }
}

fn decode_fleet_node_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<FleetNodeRecord> {
    let capabilities_json: String = row.get("capabilities_json");
    let capabilities = serde_json::from_str::<Vec<String>>(&capabilities_json).map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode fleet node capabilities_json"))
    })?;
    let kind = parse_fleet_node_kind(row.get::<String, _>("kind"))?;
    let runtime_mode = parse_fleet_node_runtime_mode(row.get::<String, _>("runtime_mode"))?;
    let status = parse_fleet_node_status(row.get::<String, _>("status"))?;

    Ok(FleetNodeRecord {
        node_id: row.get("node_id"),
        hardware_id: row.get("hardware_id"),
        kind,
        capabilities,
        owner_org_id: row.get("owner_org_id"),
        runtime_mode,
        enrolled_at: row.get("enrolled_at"),
        status,
    })
}

fn decode_tractor_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<TractorRecord> {
    let capabilities_json: String = row.get("capabilities_json");
    let capabilities = serde_json::from_str::<Vec<String>>(&capabilities_json).map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode tractor capabilities_json"))
    })?;
    let implement_ref_json: String = row.get("implement_ref_json");
    let implement_ref =
        serde_json::from_str::<TractorImplementRef>(&implement_ref_json).map_err(|err| {
            AppError::Anyhow(Error::new(err).context("failed to decode tractor implement_ref_json"))
        })?;

    Ok(TractorRecord {
        tractor_id: row.get("tractor_id"),
        org_id: row.get("org_id"),
        field_id: row.get("field_id"),
        capabilities,
        implement_ref,
        status: parse_tractor_lifecycle_status(row.get::<String, _>("status"))?,
        registered_at: row.get("registered_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_weather_forecast_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<WeatherForecastRecord> {
    let vars_json: String = row.get("vars_json");
    let vars = serde_json::from_str::<WeatherForecastVariables>(&vars_json).map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode weather vars_json"))
    })?;
    Ok(WeatherForecastRecord {
        forecast_id: row.get("forecast_id"),
        field_ref: row.get("field_ref"),
        valid_time: row.get("valid_time"),
        vars,
        source: row.get("source"),
        fetched_at: row.get("fetched_at"),
    })
}

fn decode_fleet_component_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<FleetComponentRecord> {
    let service_history_json: String = row.get("service_history_json");
    let service_history = serde_json::from_str::<Vec<ServiceHistoryEntry>>(&service_history_json)
        .map_err(|err| {
        AppError::Anyhow(
            Error::new(err).context("failed to decode fleet component service_history_json"),
        )
    })?;

    Ok(FleetComponentRecord {
        component_id: row.get("component_id"),
        component_type: parse_fleet_component_type(row.get::<String, _>("component_type"))?,
        serial: row.get("serial"),
        airframe_id: row.get("airframe_id"),
        installed_at: row.get("installed_at"),
        removed_at: row.get("removed_at"),
        service_history,
        flight_hours: row.get("flight_hours"),
        cycles: row.get::<i64, _>("cycles") as u32,
        duty_score: row.get("duty_score"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_component_duty_accrual(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<ComponentDutyAccrualRecord> {
    Ok(ComponentDutyAccrualRecord {
        session_id: row.get("session_id"),
        component_id: row.get("component_id"),
        airframe_id: row.get("airframe_id"),
        flight_hours: row.get("flight_hours"),
        cycles: row.get::<i64, _>("cycles") as u32,
        duty_score: row.get("duty_score"),
        accrued_at: row.get("accrued_at"),
    })
}

fn decode_fleet_health_indicator_sample(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<FleetHealthIndicatorSample> {
    Ok(FleetHealthIndicatorSample {
        component_id: row.get("component_id"),
        indicator: parse_fleet_health_indicator(row.get::<String, _>("indicator"))?,
        value: row.get("value"),
        ts: row.get("ts"),
        source_ref: row.get("source_ref"),
        created_at: row.get("created_at"),
        freshness: parse_health_indicator_freshness(row.get::<String, _>("freshness"))?,
    })
}

fn decode_time_series_point_response(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<TimeSeriesPointResponse> {
    let value_kind: String = row.get("value_kind");
    let value = match value_kind.as_str() {
        "scalar" => SeriesValue::Scalar {
            value: row.get("scalar_value"),
        },
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported time-series value kind {other}"
            )));
        }
    };
    let metadata_json: Option<String> = row.get("metadata_json");
    let metadata = metadata_json
        .map(|metadata_json| serde_json::from_str::<serde_json::Value>(&metadata_json))
        .transpose()
        .map_err(|err| {
            AppError::Anyhow(Error::new(err).context("failed to decode time-series metadata_json"))
        })?;

    Ok(TimeSeriesPointResponse {
        entity_ref: row.get("entity_ref"),
        metric: row.get("metric"),
        t: row.get("t"),
        value,
        source_ref: row.get("source_ref"),
        created_at: row.get("created_at"),
        metadata,
    })
}

fn decode_fired_alert_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<FiredAlertRecord> {
    let evidence_refs = serde_json::from_str::<Vec<String>>(
        &row.get::<String, _>("evidence_refs_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode alert evidence_refs_json"))
    })?;
    let channels = serde_json::from_str::<Vec<String>>(&row.get::<String, _>("channels_json"))
        .map_err(|err| {
            AppError::Anyhow(Error::new(err).context("failed to decode alert channels_json"))
        })?;
    let severity = row
        .get::<String, _>("severity")
        .parse::<AlertSeverityHint>()
        .map_err(alerting_error)?;

    Ok(FiredAlertRecord {
        alert_id: row.get("alert_id"),
        matched_rule_id: row.get("matched_rule_id"),
        source_event_ref: row.get("source_event_ref"),
        source_domain: row.get("source_domain"),
        event_type: row.get("event_type"),
        subject_ref: row.get("subject_ref"),
        field_id: row.get("field_id"),
        evidence_refs,
        severity,
        channels,
        fired_at: row.get("fired_at"),
        explanation: row.get("explanation"),
    })
}

fn decode_alert_rule_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<AlertRuleRecord> {
    let channels = serde_json::from_str::<Vec<String>>(&row.get::<String, _>("channels_json"))
        .map_err(|err| {
            AppError::Anyhow(Error::new(err).context("failed to decode alert rule channels_json"))
        })?;
    let severity = row
        .get::<String, _>("severity")
        .parse::<AlertSeverityHint>()
        .map_err(alerting_error)?;
    let status = row
        .get::<String, _>("status")
        .parse::<AlertRuleStatus>()
        .map_err(alerting_error)?;
    let version: i64 = row.get("version");

    Ok(AlertRuleRecord {
        rule_id: row.get("rule_id"),
        version: u32::try_from(version).map_err(|err| AppError::Anyhow(err.into()))?,
        event_type: row.get("event_type"),
        subject_ref: row.get("subject_ref"),
        severity,
        channels,
        status,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_alert_rule_subscription(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<AlertRuleSubscriptionRecord> {
    let channels = serde_json::from_str::<Vec<String>>(&row.get::<String, _>("channels_json"))
        .map_err(|err| {
            AppError::Anyhow(
                Error::new(err).context("failed to decode alert subscription channels_json"),
            )
        })?;

    Ok(AlertRuleSubscriptionRecord {
        subscription_id: row.get("subscription_id"),
        rule_id: row.get("rule_id"),
        recipient_id: row.get("recipient_id"),
        recipient_role: row.get("recipient_role"),
        channels,
        created_at: row.get("created_at"),
    })
}

fn decode_lineage_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<LineageRecord> {
    let inputs = serde_json::from_str::<Vec<String>>(&row.get::<String, _>("inputs_json"))
        .map_err(|err| {
            AppError::Anyhow(
                Error::new(err).context("failed to decode provenance lineage inputs_json"),
            )
        })?;
    let parameters =
        serde_json::from_str::<serde_json::Value>(&row.get::<String, _>("parameters_json"))
            .map_err(|err| {
                AppError::Anyhow(
                    Error::new(err).context("failed to decode provenance lineage parameters_json"),
                )
            })?;

    Ok(LineageRecord {
        artifact_id: row.get("artifact_id"),
        kind: decode_db_enum(row.get::<String, _>("kind"))?,
        inputs,
        method: row.get("method"),
        parameters: ProvenanceParameters::from_json(parameters),
        operator: row.get("operator"),
        actor: ActorIdentity {
            actor_id: row.get("actor_id"),
            actor_kind: decode_db_enum(row.get::<String, _>("actor_kind"))?,
        },
        created_at: row.get("created_at"),
    })
}

fn decode_audit_entry(row: &sqlx::sqlite::SqliteRow) -> AppResult<AuditEntry> {
    let payload = serde_json::from_str::<serde_json::Value>(&row.get::<String, _>("payload_json"))
        .map_err(|err| {
            AppError::Anyhow(
                Error::new(err).context("failed to decode provenance audit payload_json"),
            )
        })?;
    let seq: i64 = row.get("seq");
    let refusal_reason: Option<String> = row.get("refusal_reason");

    Ok(AuditEntry {
        seq: u64::try_from(seq).map_err(|err| AppError::Anyhow(err.into()))?,
        prev_hash: row.get("prev_hash"),
        payload_hash: row.get("payload_hash"),
        entry_hash: row.get("entry_hash"),
        actor: ActorIdentity {
            actor_id: row.get("actor_id"),
            actor_kind: decode_db_enum(row.get::<String, _>("actor_kind"))?,
        },
        ts: row.get("ts"),
        action: AuditAction {
            action_ref: row.get("action_ref"),
            action_kind: row.get("action_kind"),
            artifact_ref: row.get("artifact_ref"),
            payload: ProvenanceParameters::from_json(payload),
            occurred_at: row.get("occurred_at"),
        },
        outcome: decode_db_enum(row.get::<String, _>("outcome"))?,
        refusal_reason: refusal_reason
            .map(decode_db_enum::<AuditRefusalReason>)
            .transpose()?,
    })
}

fn decode_plugin_registration(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<PluginRegistrationRecord> {
    let capabilities = serde_json::from_str::<Vec<String>>(
        &row.get::<String, _>("capabilities_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode plugin capabilities_json"))
    })?;

    Ok(PluginRegistrationRecord {
        plugin_id: row.get("plugin_id"),
        name: row.get("name"),
        version: row.get("version"),
        kind: decode_db_enum::<ExtensionPointKind>(row.get("kind"))?,
        host_api_version: row.get("host_api_version"),
        capabilities,
        entrypoint: row.get("entrypoint"),
        status: decode_db_enum::<PluginLifecycleStatus>(row.get("status"))?,
    })
}

fn decode_db_enum<T>(value: String) -> AppResult<T>
where
    T: DeserializeOwned,
{
    serde_json::from_value(serde_json::Value::String(value))
        .map_err(|err| AppError::Anyhow(Error::new(err).context("failed to decode enum value")))
}

fn encode_db_enum<T>(value: T) -> AppResult<String>
where
    T: Serialize,
{
    match serde_json::to_value(value).map_err(|err| AppError::Anyhow(err.into()))? {
        serde_json::Value::String(value) => Ok(value),
        other => Err(AppError::Anyhow(
            Error::msg(format!("enum serialized as non-string value {other}"))
                .context("failed to encode enum value"),
        )),
    }
}

fn soil_reading_time_series_metadata(reading: &GeolocatedSoilReading) -> AppResult<String> {
    serde_json::to_string(&serde_json::json!({
        "payload_id": &reading.payload_id,
        "device_id": &reading.device_id,
        "field_id": &reading.field_id,
        "zone_id": &reading.zone_id,
        "position": &reading.position,
        "geolocation_status": reading.geolocation_status,
        "excluded_from_geospatial_products": reading.excluded_from_geospatial_products,
        "qa_flags": &reading.qa_flags,
    }))
    .map_err(|err| AppError::Anyhow(Error::new(err)))
}

fn decode_soil_moisture_reading(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<SoilMoistureReadingRecord> {
    Ok(SoilMoistureReadingRecord {
        reading_id: row.get("reading_id"),
        field_id: row.get("field_id"),
        zone_ref: row.get("zone_ref"),
        value: row.get("value"),
        source: row.get("source"),
        captured_at: row.get("captured_at"),
        qa_flag: parse_soil_moisture_qa_flag(&row.get::<String, _>("qa_flag"))
            .map_err(soil_moisture_error)?,
        ingested_at: row.get("ingested_at"),
    })
}

fn decode_soil_moisture_rejection(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<SoilMoistureRejectionRecord> {
    Ok(SoilMoistureRejectionRecord {
        rejection_id: row.get("rejection_id"),
        reading_id: row.get("reading_id"),
        field_id: row.get("field_id"),
        zone_ref: row.get("zone_ref"),
        source: row.get("source"),
        captured_at: row.get("captured_at"),
        reason: parse_soil_moisture_rejection_reason(&row.get::<String, _>("reason"))
            .map_err(soil_moisture_error)?,
        rejected_at: row.get("rejected_at"),
    })
}

fn decode_drought_index_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<DroughtIndexRecord> {
    let input_refs = serde_json::from_str::<Vec<String>>(&row.get::<String, _>("input_refs_json"))
        .map_err(|err| {
            AppError::Anyhow(Error::new(err).context("failed to decode drought input_refs_json"))
        })?;
    let accumulation_days = row
        .try_get::<Option<i64>, _>("accumulation_days")
        .map_err(Error::from)?
        .map(|days| days as u32);

    Ok(DroughtIndexRecord {
        index_id: row.get("index_id"),
        field_or_region_ref: row.get("field_or_region_ref"),
        index_type: parse_drought_index_type(&row.get::<String, _>("index_type"))
            .map_err(drought_index_error)?,
        value: row.get("value"),
        period: DroughtIndexPeriod {
            start: row.get("period_start"),
            end: row.get("period_end"),
            accumulation_days,
        },
        input_refs,
        method: row.get("method"),
        computed_at: row.get("computed_at"),
    })
}

fn decode_marketplace_account_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<MarketplaceAccountRecord> {
    let role_refs = serde_json::from_str::<Vec<String>>(&row.get::<String, _>("role_refs_json"))
        .map_err(|err| {
            AppError::Anyhow(Error::new(err).context("failed to decode marketplace role_refs_json"))
        })?;

    Ok(MarketplaceAccountRecord {
        account_id: row.get("account_id"),
        org_id: row.get("org_id"),
        party_type: parse_marketplace_party_type(&row.get::<String, _>("party_type"))
            .map_err(marketplace_account_error)?,
        role_refs,
        status: parse_marketplace_account_status(&row.get::<String, _>("status"))
            .map_err(marketplace_account_error)?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_marketplace_catalog_item_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<MarketplaceCatalogItemRecord> {
    Ok(MarketplaceCatalogItemRecord {
        item_id: row.get("item_id"),
        org_id: row.get("org_id"),
        kind: parse_marketplace_catalog_item_kind(&row.get::<String, _>("kind"))
            .map_err(marketplace_catalog_error)?,
        category: parse_marketplace_catalog_category(&row.get::<String, _>("category"))
            .map_err(marketplace_catalog_error)?,
        name: row.get("name"),
        unit_of_measure: parse_marketplace_unit_of_measure(
            &row.get::<String, _>("unit_of_measure"),
        )
        .map_err(marketplace_catalog_error)?,
        owner_account_id: row.get("owner_account_id"),
        created_at: row.get("created_at"),
    })
}

fn decode_marketplace_listing_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<MarketplaceListingRecord> {
    Ok(MarketplaceListingRecord {
        listing_id: row.get("listing_id"),
        item_id: row.get("item_id"),
        org_id: row.get("org_id"),
        price: row.get("price"),
        currency: row.get("currency"),
        available_qty: row.get("available_qty"),
        window: shared::schemas::MarketplaceAvailabilityWindow {
            from: row.get("window_from"),
            to: row.get("window_to"),
        },
        status: parse_marketplace_listing_status(&row.get::<String, _>("status"))
            .map_err(marketplace_listing_error)?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_marketplace_inventory_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<MarketplaceInventoryRecord> {
    Ok(MarketplaceInventoryRecord {
        inventory_id: row.get("inventory_id"),
        item_id: row.get("item_id"),
        org_id: row.get("org_id"),
        on_hand: row.get("on_hand"),
        reserved: row.get("reserved"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_marketplace_order_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<MarketplaceOrderRecord> {
    Ok(MarketplaceOrderRecord {
        order_id: row.get("order_id"),
        org_id: row.get("org_id"),
        listing_ref: row.get("listing_ref"),
        buyer_account_id: row.get("buyer_account_id"),
        qty: row.get("qty"),
        line_total: row.get("line_total"),
        status: parse_marketplace_order_status(&row.get::<String, _>("status"))
            .map_err(marketplace_order_error)?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_marketplace_order_audit_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<MarketplaceOrderAuditRecord> {
    let from_status = row
        .get::<Option<String>, _>("from_status")
        .map(|status| parse_marketplace_order_status(&status).map_err(marketplace_order_error))
        .transpose()?;
    Ok(MarketplaceOrderAuditRecord {
        audit_id: row.get("audit_id"),
        order_id: row.get("order_id"),
        from_status,
        to_status: parse_marketplace_order_status(&row.get::<String, _>("to_status"))
            .map_err(marketplace_order_error)?,
        actor_id: row.get("actor_id"),
        occurred_at: row.get("occurred_at"),
    })
}

fn decode_sustainability_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<SustainabilityRecord> {
    Ok(SustainabilityRecord {
        record_id: row.get("record_id"),
        field_id: row.get("field_id"),
        season_id: row.get("season_id"),
        operation_id: row.get("operation_id"),
        metric_type: parse_sustainability_metric_type(&row.get::<String, _>("metric_type"))
            .map_err(sustainability_record_error)?,
        method_version: row.get("method_version"),
        created_at: row.get("created_at"),
        audit_id: row.get("audit_id"),
    })
}

fn decode_content_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<ContentRecord> {
    Ok(ContentRecord {
        content_id: row.get("content_id"),
        content_type: parse_content_type(&row.get::<String, _>("content_type"))
            .map_err(content_error)?,
        author_id: row.get("author_id"),
        org_id: row.get("org_id"),
        status: parse_content_status(&row.get::<String, _>("status")).map_err(content_error)?,
        current_version: row.get("current_version"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_content_version_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<ContentVersionRecord> {
    Ok(ContentVersionRecord {
        version_id: row.get("version_id"),
        content_id: row.get("content_id"),
        body: row.get("body"),
        created_at: row.get("created_at"),
    })
}

fn decode_collaboration_channel(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<CollaborationChannelRecord> {
    let member_account_ids =
        serde_json::from_str::<Vec<String>>(&row.get::<String, _>("member_account_ids_json"))
            .map_err(|err| {
                AppError::Anyhow(
                    Error::new(err).context("failed to decode collab member_account_ids_json"),
                )
            })?;

    Ok(CollaborationChannelRecord {
        channel_id: row.get("channel_id"),
        org_id: row.get("org_id"),
        field_ref: row.get("field_ref"),
        member_account_ids,
        created_at: row.get("created_at"),
    })
}

fn decode_collaboration_message(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<CollaborationMessageRecord> {
    Ok(CollaborationMessageRecord {
        message_id: row.get("message_id"),
        channel_id: row.get("channel_id"),
        author_id: row.get("author_id"),
        body: row.get("body"),
        sent_at: row.get("sent_at"),
    })
}

fn decode_soil_iot_device(row: &sqlx::sqlite::SqliteRow) -> AppResult<SoilDeviceRecord> {
    Ok(SoilDeviceRecord {
        device_id: row.get("device_id"),
        org_id: row.get("org_id"),
        field_id: row.get("field_id"),
        zone_id: row.get("zone_id"),
        sensor_type: parse_soil_sensor_type(row.get::<String, _>("sensor_type"))?,
        position: GeoPosition {
            latitude: row.get("latitude"),
            longitude: row.get("longitude"),
            crs: row.get("crs"),
        },
        calibration_profile_ref: row.get("calibration_profile_ref"),
        status: parse_soil_device_status(row.get::<String, _>("status"))?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_soil_iot_config_push(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<SoilDeviceConfigPushRecord> {
    Ok(SoilDeviceConfigPushRecord {
        push_id: row.get("push_id"),
        device_id: row.get("device_id"),
        config_version: row.get("config_version"),
        pushed_at: row.get("pushed_at"),
        push_status: parse_soil_config_push_status(row.get::<String, _>("push_status"))?,
        failure_reason: row.get("failure_reason"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_fleet_component_event(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<FleetComponentEventRecord> {
    Ok(FleetComponentEventRecord {
        component_id: row.get("component_id"),
        event_type: row.get("event_type"),
        airframe_id: row.get("airframe_id"),
        event_at: row.get("event_at"),
        actor: row.get("actor"),
        details: row.get("details"),
    })
}

fn decode_orthomosaic_frame_set_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<FrameSetRecord> {
    let frames_json: String = row.get("frames_json");
    let frames = serde_json::from_str::<Vec<FramePoseRecord>>(&frames_json).map_err(|err| {
        AppError::Anyhow(
            Error::new(err).context("failed to decode orthomosaic frame set frames_json"),
        )
    })?;

    Ok(FrameSetRecord {
        frame_set_id: row.get("frame_set_id"),
        scene_id: row.get("scene_id"),
        field_id: row.get("field_id"),
        season_id: row.get("season_id"),
        frames,
        crs_hint: row.get("crs_hint"),
        created_at: row.get("created_at"),
    })
}

fn decode_orthomosaic_reconstruction_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<ReconstructionJobRecord> {
    let params_json: String = row.get("params_json");
    let params = serde_json::from_str::<serde_json::Value>(&params_json).map_err(|err| {
        AppError::Anyhow(
            Error::new(err).context("failed to decode orthomosaic reconstruction params_json"),
        )
    })?;

    Ok(ReconstructionJobRecord {
        recon_id: row.get("recon_id"),
        frame_set_id: row.get("frame_set_id"),
        params,
        status: parse_reconstruction_status(row.get::<String, _>("status"))?,
        failure_reason: row.get("failure_reason"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_crop_model_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<ModelVersionRecord> {
    let metrics_json: String = row.get("metrics_json");
    let metrics = serde_json::from_str::<serde_json::Value>(&metrics_json).map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode crop model metrics_json"))
    })?;

    Ok(ModelVersionRecord {
        model_id: row.get("model_id"),
        version: row.get("version"),
        task: parse_crop_model_task(row.get::<String, _>("task"))?,
        training_set_ref: row.get("training_set_ref"),
        metrics,
        provenance_ref: row.get("provenance_ref"),
        created_at: row.get("created_at"),
    })
}

fn decode_compliance_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<ComplianceRecord> {
    let version: i64 = row.get("version");
    let prior_version: Option<i64> = row.get("prior_version");

    Ok(ComplianceRecord {
        record_id: row.get("record_id"),
        version: version as u32,
        record_type: parse_compliance_record_type(row.get::<String, _>("record_type"))?,
        org_id: row.get("org_id"),
        field_id: row.get("field_id"),
        flight_id: row.get("flight_id"),
        created_at: row.get("created_at"),
        actor: row.get("actor"),
        provenance_ref: row.get("provenance_ref"),
        prior_version: prior_version.map(|version| version as u32),
        change_reason: row.get("change_reason"),
        payload: decode_compliance_payload(row)?,
    })
}

fn decode_compliance_payload(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<Option<ComplianceRecordPayload>> {
    let payload_json: Option<String> = row.get("payload_json");
    payload_json
        .map(|payload_json| {
            serde_json::from_str::<ComplianceRecordPayload>(&payload_json).map_err(|err| {
                AppError::Anyhow(
                    Error::new(err).context("failed to decode compliance payload_json"),
                )
            })
        })
        .transpose()
}

fn encode_compliance_payload(record: &ComplianceRecord) -> AppResult<Option<String>> {
    record
        .payload
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|err| AppError::Anyhow(Error::new(err)))
}

fn decode_airspace_zone(row: &sqlx::sqlite::SqliteRow) -> AppResult<AirspaceZoneRecord> {
    let geometry_json: String = row.get("geometry_json");
    let coordinates =
        serde_json::from_str::<Vec<AirspaceCoordinate>>(&geometry_json).map_err(|err| {
            AppError::Anyhow(
                Error::new(err).context("failed to decode airspace zone geometry_json"),
            )
        })?;

    Ok(AirspaceZoneRecord {
        zone_id: row.get("zone_id"),
        zone_class: parse_airspace_zone_class(row.get::<String, _>("zone_class"))?,
        crs: row.get("crs"),
        coordinates,
        extent: compliance::AirspaceZoneExtent {
            min_lon: row.get("min_lon"),
            min_lat: row.get("min_lat"),
            max_lon: row.get("max_lon"),
            max_lat: row.get("max_lat"),
        },
        effective_from: row.get("effective_from"),
        effective_to: row.get("effective_to"),
        source: row.get("source"),
        created_at: row.get("created_at"),
    })
}

fn decode_annotation_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<AnnotationRecord> {
    let geometry_json: String = row.get("geometry_json");
    let geometry = serde_json::from_str::<AnnotationGeometry>(&geometry_json).map_err(|err| {
        AppError::Anyhow(anyhow::Error::new(err).context("failed to decode annotation geometry"))
    })?;

    Ok(AnnotationRecord {
        annotation_id: row.get("annotation_id"),
        scene_id: row.get("scene_id"),
        field_id: row.get("field_id"),
        author: row.get("author"),
        crs: row.get("crs"),
        audit_id: row.get("audit_id"),
        label: row.get("label"),
        note: row.get("note"),
        severity: row.get("severity"),
        geometry,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

async fn decode_recommendation_record(
    state: &AppState,
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<RecommendationRecord> {
    let recommendation_id: String = row.get("recommendation_id");
    let annotation_ids = load_recommendation_annotation_ids(state, &recommendation_id).await?;
    let stored_evidence_refs = serde_json::from_str::<Vec<String>>(
        &row.get::<String, _>("evidence_refs_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(
            Error::new(err).context("failed to decode recommendation evidence_refs_json"),
        )
    })?;
    let category: Option<String> = row.get("category");
    Ok(RecommendationRecord {
        recommendation_id: recommendation_id.clone(),
        scene_id: row.get("scene_id"),
        field_id: row.get("field_id"),
        org_id: DEFAULT_RECORD_OWNER.to_string(),
        author_user_id: DEFAULT_RECORD_OWNER.to_string(),
        title: row.get("title"),
        note: row.get("note"),
        category: category.clone(),
        action_category: category
            .and_then(|value| normalize_optional_text(Some(value)))
            .unwrap_or_else(|| "general".to_string()),
        priority: parse_recommendation_priority(row.get("priority"))?,
        status: parse_recommendation_status(row.get("status"))?,
        evidence_refs: combine_text_values(
            recommendation_evidence_from_annotations(&annotation_ids),
            stored_evidence_refs,
        ),
        annotation_ids,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_report_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<ReportRecord> {
    let scene_id: String = row.get("scene_id");
    let report_id: String = row.get("report_id");
    let artifact_path: String = row.get("path");
    Ok(ReportRecord {
        report_id: report_id.clone(),
        scene_id: scene_id.clone(),
        field_id: row.get("field_id"),
        season_id: None,
        org_id: DEFAULT_RECORD_OWNER.to_string(),
        generated_by: DEFAULT_RECORD_OWNER.to_string(),
        source_refs: vec![format!("scene:{scene_id}")],
        title: row.get("title"),
        format: parse_report_format(row.get("format"))?,
        artifact_path: artifact_path.clone(),
        artifact_uri: artifact_path,
        download_url: format!("/api/scenes/{scene_id}/reports/{report_id}"),
        visibility: parse_report_visibility(row.get("visibility"))?,
        annotation_count: row.get::<i64, _>("annotation_count") as usize,
        recommendation_count: row.get::<i64, _>("recommendation_count") as usize,
        created_at: row.get("created_at"),
    })
}

fn decode_report_share_record(row: &sqlx::sqlite::SqliteRow) -> ReportShareRecord {
    ReportShareRecord {
        share_token: row.get("share_token"),
        report_id: row.get("share_report_id"),
        scene_id: row.get("share_scene_id"),
        expires_at: row.get("share_expires_at"),
        revoked_at: row.get("share_revoked_at"),
        created_at: row.get("share_created_at"),
    }
}

fn decode_shared_report_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<SharedReportRecord> {
    Ok(SharedReportRecord {
        share: decode_report_share_record(row),
        report: decode_report_record(row)?,
    })
}

async fn load_field(state: &AppState, field_id: &str) -> AppResult<Option<FieldRecord>> {
    let row = sqlx::query(
        r#"
        SELECT field_id, farm_id, owner, name, crop, season, notes, boundary_json, status,
               created_at, COALESCE(NULLIF(updated_at, ''), created_at) AS updated_at
        FROM fields
        WHERE field_id = ?1
        "#,
    )
    .bind(field_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_field_record(&row)).transpose()
}

async fn validate_drought_scope_ref(state: &AppState, field_or_region_ref: &str) -> AppResult<()> {
    let scope_ref = normalize_optional_text(Some(field_or_region_ref.to_string()))
        .ok_or_else(|| AppError::BadRequest("field_or_region_ref is required".to_string()))?;
    if let Some(field_id) = scope_ref.strip_prefix("field:") {
        let field_id = normalize_optional_text(Some(field_id.to_string()))
            .ok_or_else(|| AppError::BadRequest("field scope requires a field id".to_string()))?;
        load_field(state, &field_id)
            .await?
            .ok_or_else(|| AppError::BadRequest(format!("field {field_id} does not exist")))?;
        return Ok(());
    }
    if let Some(region_ref) = scope_ref.strip_prefix("region:") {
        normalize_optional_text(Some(region_ref.to_string())).ok_or_else(|| {
            AppError::BadRequest("region scope requires a non-empty region ref".to_string())
        })?;
        return Ok(());
    }

    Err(AppError::BadRequest(
        "field_or_region_ref must start with field: or region:".to_string(),
    ))
}

async fn load_farm(state: &AppState, farm_id: &str) -> AppResult<Option<FarmRecord>> {
    let row = sqlx::query(
        r#"
        SELECT farm_id, owner, name, notes, status, created_at,
               COALESCE(NULLIF(updated_at, ''), created_at) AS updated_at
        FROM farms
        WHERE farm_id = ?1
        "#,
    )
    .bind(farm_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(row.map(|row| decode_farm_record(&row)))
}

async fn load_fleet_node(state: &AppState, node_id: &str) -> AppResult<Option<FleetNodeRecord>> {
    let row = sqlx::query(
        r#"
        SELECT node_id, hardware_id, kind, capabilities_json, owner_org_id, runtime_mode, enrolled_at, status
        FROM fleet_nodes
        WHERE node_id = ?1
        "#,
    )
    .bind(node_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_fleet_node_record(&row)).transpose()
}

async fn load_fleet_node_by_hardware_id(
    state: &AppState,
    hardware_id: &str,
) -> AppResult<Option<FleetNodeRecord>> {
    let row = sqlx::query(
        r#"
        SELECT node_id, hardware_id, kind, capabilities_json, owner_org_id, runtime_mode, enrolled_at, status
        FROM fleet_nodes
        WHERE hardware_id = ?1
        "#,
    )
    .bind(hardware_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_fleet_node_record(&row)).transpose()
}

async fn load_tractor(state: &AppState, tractor_id: &str) -> AppResult<Option<TractorRecord>> {
    let row = sqlx::query(
        r#"
        SELECT tractor_id, org_id, field_id, capabilities_json, implement_ref_json, status,
               registered_at, updated_at
        FROM tractor_vehicles
        WHERE tractor_id = ?1
        "#,
    )
    .bind(tractor_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_tractor_record(&row)).transpose()
}

async fn insert_tractor_record(state: &AppState, record: &TractorRecord) -> AppResult<()> {
    let capabilities_json =
        serde_json::to_string(&record.capabilities).map_err(|err| AppError::Anyhow(err.into()))?;
    let implement_ref_json =
        serde_json::to_string(&record.implement_ref).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO tractor_vehicles (
            tractor_id, org_id, field_id, capabilities_json, implement_ref_json, status,
            registered_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&record.tractor_id)
    .bind(&record.org_id)
    .bind(&record.field_id)
    .bind(capabilities_json)
    .bind(implement_ref_json)
    .bind(record.status.as_str())
    .bind(&record.registered_at)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

fn build_tractor_command_audit(
    command: &TractorMotionCommandRequest,
    tractor: Option<&TractorRecord>,
    reason: TractorCommandRejectionReason,
) -> TractorCommandAuditRecord {
    TractorCommandAuditRecord {
        audit_id: format!("tractor-command-audit-{}", Uuid::new_v4()),
        command_id: command.command_id.clone(),
        tractor_id: command.tractor_id.clone(),
        org_id: tractor.map(|tractor| tractor.org_id.clone()),
        field_id: tractor.map(|tractor| tractor.field_id.clone()),
        command_type: command.command_type.clone(),
        requested_by: command.requested_by.clone(),
        decision: TractorCommandAuditDecision::Rejected,
        reason_code: reason.as_str().to_string(),
        at: current_record_timestamp(),
    }
}

async fn insert_tractor_command_audit(
    state: &AppState,
    audit: &TractorCommandAuditRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO tractor_command_audits (
            audit_id, command_id, tractor_id, org_id, field_id, command_type, requested_by,
            decision, reason_code, at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(&audit.audit_id)
    .bind(&audit.command_id)
    .bind(&audit.tractor_id)
    .bind(&audit.org_id)
    .bind(&audit.field_id)
    .bind(&audit.command_type)
    .bind(&audit.requested_by)
    .bind(match audit.decision {
        TractorCommandAuditDecision::Allowed => "allowed",
        TractorCommandAuditDecision::Rejected => "rejected",
    })
    .bind(&audit.reason_code)
    .bind(&audit.at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_weather_forecast_record(
    state: &AppState,
    field_id: &str,
    record: &WeatherForecastRecord,
    latitude: f64,
    longitude: f64,
    created_at: String,
) -> AppResult<()> {
    let vars_json =
        serde_json::to_string(&record.vars).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO weather_forecasts (
            forecast_id, field_id, field_ref, valid_time, vars_json, source, fetched_at,
            latitude, longitude, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(&record.forecast_id)
    .bind(field_id)
    .bind(&record.field_ref)
    .bind(&record.valid_time)
    .bind(vars_json)
    .bind(&record.source)
    .bind(&record.fetched_at)
    .bind(latitude)
    .bind(longitude)
    .bind(created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_weather_time_series_points(
    state: &AppState,
    field_id: &str,
    record: &WeatherForecastRecord,
    created_at: String,
) -> AppResult<()> {
    let metadata = serde_json::json!({
        "forecast_id": record.forecast_id,
        "field_id": field_id,
        "source": record.source,
        "fetched_at": record.fetched_at,
        "valid_time": record.valid_time
    })
    .to_string();

    for (metric, value) in [
        ("temperature_celsius", &record.vars.temperature_celsius),
        ("wind_speed_mps", &record.vars.wind_speed_mps),
        ("precipitation_mm", &record.vars.precipitation_mm),
        ("humidity_percent", &record.vars.humidity_percent),
        ("radiation_w_m2", &record.vars.radiation_w_m2),
    ] {
        insert_time_series_point_record(
            state,
            &SeriesPoint {
                entity_ref: record.field_ref.clone(),
                metric: metric.to_string(),
                unit: value.unit.clone(),
                t: record.valid_time.clone(),
                value: SeriesValue::Scalar { value: value.value },
                source_ref: record.forecast_id.clone(),
                created_at: created_at.clone(),
            },
            Some(metadata.clone()),
        )
        .await?;
    }

    Ok(())
}

async fn insert_weather_fetch_failure(
    state: &AppState,
    field_id: &str,
    failure: &WeatherFetchFailureRecord,
    latitude: f64,
    longitude: f64,
    created_at: String,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO weather_fetch_failures (
            failure_id, field_id, field_ref, source, fetched_at, reason, latitude, longitude, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(&failure.failure_id)
    .bind(field_id)
    .bind(&failure.field_ref)
    .bind(&failure.source)
    .bind(&failure.fetched_at)
    .bind(&failure.reason)
    .bind(latitude)
    .bind(longitude)
    .bind(created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn validate_enrolled_airframe(state: &AppState, airframe_id: &str) -> AppResult<()> {
    let airframe_id = normalize_optional_text(Some(airframe_id.to_string()))
        .ok_or_else(|| AppError::BadRequest("airframe_id is required".to_string()))?;
    let node = load_fleet_node(state, &airframe_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("airframe {airframe_id} is not enrolled")))?;
    if node.kind != FleetNodeKind::Drone {
        return Err(AppError::BadRequest(format!(
            "fleet node {airframe_id} is not an aircraft"
        )));
    }

    Ok(())
}

async fn insert_fleet_component(state: &AppState, record: &FleetComponentRecord) -> AppResult<()> {
    let service_history_json = serde_json::to_string(&record.service_history)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO fleet_components (
            component_id, component_type, serial, airframe_id, installed_at, removed_at,
            service_history_json, flight_hours, cycles, duty_score, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
    )
    .bind(&record.component_id)
    .bind(record.component_type.as_str())
    .bind(&record.serial)
    .bind(&record.airframe_id)
    .bind(&record.installed_at)
    .bind(&record.removed_at)
    .bind(service_history_json)
    .bind(record.flight_hours)
    .bind(i64::from(record.cycles))
    .bind(record.duty_score)
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn update_fleet_component_install(
    state: &AppState,
    record: &FleetComponentRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE fleet_components
        SET airframe_id = ?1, installed_at = ?2, removed_at = ?3, updated_at = ?4
        WHERE component_id = ?5
        "#,
    )
    .bind(&record.airframe_id)
    .bind(&record.installed_at)
    .bind(&record.removed_at)
    .bind(&record.updated_at)
    .bind(&record.component_id)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn update_fleet_component_duty_totals(
    state: &AppState,
    record: &FleetComponentRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE fleet_components
        SET flight_hours = ?1, cycles = ?2, duty_score = ?3, updated_at = ?4
        WHERE component_id = ?5
        "#,
    )
    .bind(record.flight_hours)
    .bind(i64::from(record.cycles))
    .bind(record.duty_score)
    .bind(&record.updated_at)
    .bind(&record.component_id)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_component_duty_accrual(
    state: &AppState,
    accrual: &ComponentDutyAccrualRecord,
) -> AppResult<bool> {
    let result = sqlx::query(
        r#"
        INSERT OR IGNORE INTO fleet_component_duty_accruals (
            session_id, component_id, airframe_id, flight_hours, cycles, duty_score, accrued_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&accrual.session_id)
    .bind(&accrual.component_id)
    .bind(&accrual.airframe_id)
    .bind(accrual.flight_hours)
    .bind(i64::from(accrual.cycles))
    .bind(accrual.duty_score)
    .bind(&accrual.accrued_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(result.rows_affected() > 0)
}

async fn insert_fleet_health_indicator_sample(
    state: &AppState,
    sample: &FleetHealthIndicatorSample,
    airframe_id: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO fleet_health_indicator_samples (
            component_id, airframe_id, indicator, value, ts, source_ref, freshness, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&sample.component_id)
    .bind(airframe_id)
    .bind(sample.indicator.as_str())
    .bind(sample.value)
    .bind(&sample.ts)
    .bind(&sample.source_ref)
    .bind(sample.freshness.as_str())
    .bind(&sample.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_time_series_point(
    state: &AppState,
    sample: &FleetHealthIndicatorSample,
) -> AppResult<()> {
    insert_time_series_point_record(state, &sample.to_series_point(), None).await
}

async fn insert_time_series_point_record(
    state: &AppState,
    point: &SeriesPoint,
    metadata_json: Option<String>,
) -> AppResult<()> {
    let scalar_value = match &point.value {
        SeriesValue::Scalar { value } => *value,
        SeriesValue::Raster(_) => {
            return Err(AppError::BadRequest(
                "only scalar time-series points are supported by geo_hub persistence".to_string(),
            ));
        }
    };

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO time_series_points (
            entity_ref, metric, t, value_kind, scalar_value, source_ref, created_at, metadata_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&point.entity_ref)
    .bind(&point.metric)
    .bind(&point.t)
    .bind("scalar")
    .bind(scalar_value)
    .bind(&point.source_ref)
    .bind(&point.created_at)
    .bind(metadata_json)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_fired_alert_record(state: &AppState, record: &FiredAlertRecord) -> AppResult<()> {
    let evidence_refs_json =
        serde_json::to_string(&record.evidence_refs).map_err(|err| AppError::Anyhow(err.into()))?;
    let channels_json =
        serde_json::to_string(&record.channels).map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO alert_fired_alerts (
            alert_id, matched_rule_id, source_event_ref, source_domain, event_type, subject_ref,
            field_id, evidence_refs_json, severity, channels_json, fired_at, explanation, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        "#,
    )
    .bind(&record.alert_id)
    .bind(&record.matched_rule_id)
    .bind(&record.source_event_ref)
    .bind(&record.source_domain)
    .bind(&record.event_type)
    .bind(&record.subject_ref)
    .bind(&record.field_id)
    .bind(evidence_refs_json)
    .bind(record.severity.as_str())
    .bind(channels_json)
    .bind(&record.fired_at)
    .bind(&record.explanation)
    .bind(current_record_timestamp())
    .execute(&state.pool)
    .await
    .map_err(|err| {
        if err.to_string().contains("UNIQUE constraint failed") {
            AppError::BadRequest(format!(
                "fired alert {} already exists and history is immutable",
                record.alert_id
            ))
        } else {
            AppError::Anyhow(err.into())
        }
    })?;

    Ok(())
}

async fn insert_plugin_registration(
    state: &AppState,
    record: &PluginRegistrationRecord,
    timestamp: String,
) -> AppResult<()> {
    let capabilities_json =
        serde_json::to_string(&record.capabilities).map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO plugin_registrations (
            plugin_id, name, version, kind, host_api_version, capabilities_json, entrypoint,
            status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
        "#,
    )
    .bind(&record.plugin_id)
    .bind(&record.name)
    .bind(&record.version)
    .bind(record.kind.as_str())
    .bind(&record.host_api_version)
    .bind(capabilities_json)
    .bind(&record.entrypoint)
    .bind(record.status.as_str())
    .bind(timestamp)
    .execute(&state.pool)
    .await
    .map_err(|err| {
        if err.to_string().contains("UNIQUE constraint failed") {
            AppError::BadRequest(format!("plugin {} is already registered", record.plugin_id))
        } else {
            AppError::Anyhow(err.into())
        }
    })?;

    Ok(())
}

async fn update_plugin_registration_status(
    state: &AppState,
    record: &PluginRegistrationRecord,
    updated_at: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE plugin_registrations
        SET status = ?2, updated_at = ?3
        WHERE plugin_id = ?1
        "#,
    )
    .bind(&record.plugin_id)
    .bind(record.status.as_str())
    .bind(updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_plugin_lifecycle_audit(
    state: &AppState,
    audit: &PluginLifecycleAuditRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO plugin_lifecycle_audits (
            audit_id, plugin_id, previous_status, new_status, actor_id, occurred_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&audit.audit_id)
    .bind(&audit.plugin_id)
    .bind(audit.previous_status.as_str())
    .bind(audit.new_status.as_str())
    .bind(&audit.actor_id)
    .bind(&audit.occurred_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn append_plugin_lifecycle_provenance_audit(
    state: &AppState,
    audit: &PluginLifecycleAuditRecord,
    actor_kind: ActorKind,
) -> AppResult<()> {
    let existing_entries = load_provenance_audit_entries_for_append(state).await?;
    let mut ledger = AuditLedger::from_entries(existing_entries)
        .map_err(|err| AppError::Anyhow(Error::new(err)))?;
    let entry = ledger
        .append_action(
            ActorIdentity {
                actor_id: audit.actor_id.clone(),
                actor_kind,
            },
            AuditAction {
                action_ref: audit.audit_id.clone(),
                action_kind: "plugin_lifecycle_transition".to_string(),
                artifact_ref: Some(format!("plugin:{}", audit.plugin_id)),
                payload: ProvenanceParameters::from_json(serde_json::json!({
                    "plugin_id": audit.plugin_id,
                    "previous_status": audit.previous_status,
                    "new_status": audit.new_status,
                })),
                occurred_at: audit.occurred_at.clone(),
            },
        )
        .map_err(|err| AppError::Anyhow(Error::new(err)))?;
    insert_provenance_audit_entry(state, &entry).await
}

async fn insert_provenance_audit_entry(state: &AppState, entry: &AuditEntry) -> AppResult<()> {
    let payload_json = serde_json::to_string(entry.action.payload.as_json())
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let actor_kind = encode_db_enum(entry.actor.actor_kind)?;
    let outcome = encode_db_enum(entry.outcome)?;
    let refusal_reason = entry.refusal_reason.map(encode_db_enum).transpose()?;

    sqlx::query(
        r#"
        INSERT INTO provenance_audit_entries (
            entry_hash, seq, prev_hash, payload_hash, actor_id, actor_kind, ts, action_ref,
            action_kind, artifact_ref, payload_json, occurred_at, outcome, refusal_reason
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        "#,
    )
    .bind(&entry.entry_hash)
    .bind(entry.seq as i64)
    .bind(&entry.prev_hash)
    .bind(&entry.payload_hash)
    .bind(&entry.actor.actor_id)
    .bind(actor_kind)
    .bind(&entry.ts)
    .bind(&entry.action.action_ref)
    .bind(&entry.action.action_kind)
    .bind(&entry.action.artifact_ref)
    .bind(payload_json)
    .bind(&entry.action.occurred_at)
    .bind(outcome)
    .bind(refusal_reason)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_alert_rule_record(state: &AppState, record: &AlertRuleRecord) -> AppResult<()> {
    let channels_json =
        serde_json::to_string(&record.channels).map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO alert_rules (
            rule_id, version, event_type, subject_ref, severity, channels_json, status,
            created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(&record.rule_id)
    .bind(record.version as i64)
    .bind(&record.event_type)
    .bind(&record.subject_ref)
    .bind(record.severity.as_str())
    .bind(channels_json)
    .bind(record.status.as_str())
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_alert_rule_audit(state: &AppState, audit: &AlertRuleAuditRecord) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO alert_rule_audits (
            audit_id, rule_id, version, previous_status, new_status, actor_id, occurred_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&audit.audit_id)
    .bind(&audit.rule_id)
    .bind(audit.version as i64)
    .bind(audit.previous_status.as_str())
    .bind(audit.new_status.as_str())
    .bind(&audit.actor_id)
    .bind(&audit.occurred_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_alert_rule_subscription(
    state: &AppState,
    subscription: &AlertRuleSubscriptionRecord,
) -> AppResult<()> {
    let channels_json = serde_json::to_string(&subscription.channels)
        .map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO alert_rule_subscriptions (
            subscription_id, rule_id, recipient_id, recipient_role, channels_json, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&subscription.subscription_id)
    .bind(&subscription.rule_id)
    .bind(&subscription.recipient_id)
    .bind(&subscription.recipient_role)
    .bind(channels_json)
    .bind(&subscription.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_fleet_health_telemetry_gap(
    state: &AppState,
    gap: &HealthTelemetryGap,
    airframe_id: Option<&str>,
    derived: &FleetHealthIndicatorDerivation,
) -> AppResult<()> {
    let sample = derived.samples.first().ok_or_else(|| {
        AppError::BadRequest("health indicator sample required before gap persistence".to_string())
    })?;
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO fleet_health_telemetry_gaps (
            component_id, airframe_id, started_at, ended_at, reason, source_ref, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&gap.component_id)
    .bind(airframe_id)
    .bind(&gap.started_at)
    .bind(&gap.ended_at)
    .bind(&gap.reason)
    .bind(&sample.source_ref)
    .bind(&sample.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_soil_iot_device(state: &AppState, record: &SoilDeviceRecord) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO soil_iot_devices (
            device_id, org_id, field_id, zone_id, sensor_type, latitude, longitude, crs,
            calibration_profile_ref, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
    )
    .bind(&record.device_id)
    .bind(&record.org_id)
    .bind(&record.field_id)
    .bind(&record.zone_id)
    .bind(record.sensor_type.as_str())
    .bind(record.position.latitude)
    .bind(record.position.longitude)
    .bind(&record.position.crs)
    .bind(&record.calibration_profile_ref)
    .bind(record.status.as_str())
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_soil_iot_config_push(
    state: &AppState,
    record: &SoilDeviceConfigPushRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO soil_iot_config_pushes (
            push_id, device_id, config_version, pushed_at, push_status, failure_reason, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&record.push_id)
    .bind(&record.device_id)
    .bind(&record.config_version)
    .bind(&record.pushed_at)
    .bind(record.push_status.as_str())
    .bind(&record.failure_reason)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn update_soil_iot_config_push(
    state: &AppState,
    record: &SoilDeviceConfigPushRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE soil_iot_config_pushes
        SET push_status = ?2, failure_reason = ?3, updated_at = ?4
        WHERE push_id = ?1
        "#,
    )
    .bind(&record.push_id)
    .bind(record.push_status.as_str())
    .bind(&record.failure_reason)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_soil_moisture_reading(
    state: &AppState,
    record: &SoilMoistureReadingRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO water_moisture_readings (
            reading_id, field_id, zone_ref, value, source, captured_at, qa_flag, ingested_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&record.reading_id)
    .bind(&record.field_id)
    .bind(&record.zone_ref)
    .bind(record.value)
    .bind(&record.source)
    .bind(&record.captured_at)
    .bind(record.qa_flag.as_str())
    .bind(&record.ingested_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_soil_moisture_rejection(
    state: &AppState,
    rejection: &SoilMoistureRejectionRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO water_moisture_reading_rejections (
            rejection_id, reading_id, field_id, zone_ref, source, captured_at, reason, rejected_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&rejection.rejection_id)
    .bind(&rejection.reading_id)
    .bind(&rejection.field_id)
    .bind(&rejection.zone_ref)
    .bind(&rejection.source)
    .bind(&rejection.captured_at)
    .bind(rejection.reason.as_str())
    .bind(&rejection.rejected_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_soil_moisture_time_series_point(
    state: &AppState,
    record: &SoilMoistureReadingRecord,
) -> AppResult<()> {
    let metadata = serde_json::json!({
        "reading_id": record.reading_id,
        "field_id": record.field_id,
        "zone_ref": record.zone_ref,
        "source": record.source,
        "captured_at": record.captured_at,
        "qa_flag": record.qa_flag.as_str()
    })
    .to_string();
    let point = SeriesPoint {
        entity_ref: format!("field:{}:zone:{}", record.field_id, record.zone_ref),
        metric: "soil_moisture_percent".to_string(),
        unit: "percent".to_string(),
        t: record.captured_at.clone(),
        value: SeriesValue::Scalar {
            value: record.value,
        },
        source_ref: record.reading_id.clone(),
        created_at: record.ingested_at.clone(),
    };

    insert_time_series_point_record(state, &point, Some(metadata)).await
}

async fn insert_drought_index_record(
    state: &AppState,
    record: &DroughtIndexRecord,
    created_at: String,
) -> AppResult<()> {
    let input_refs_json =
        serde_json::to_string(&record.input_refs).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO drought_indices (
            index_id, field_or_region_ref, index_type, value, period_start, period_end,
            accumulation_days, input_refs_json, method, computed_at, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(&record.index_id)
    .bind(&record.field_or_region_ref)
    .bind(record.index_type.as_str())
    .bind(record.value)
    .bind(&record.period.start)
    .bind(&record.period.end)
    .bind(record.period.accumulation_days.map(i64::from))
    .bind(input_refs_json)
    .bind(&record.method)
    .bind(&record.computed_at)
    .bind(created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_drought_index_time_series_point(
    state: &AppState,
    record: &DroughtIndexRecord,
) -> AppResult<()> {
    let metadata = serde_json::json!({
        "index_id": record.index_id,
        "field_or_region_ref": record.field_or_region_ref,
        "index_type": record.index_type.as_str(),
        "period": record.period,
        "input_refs": record.input_refs,
        "method": record.method,
        "computed_at": record.computed_at
    })
    .to_string();
    let point = SeriesPoint {
        entity_ref: record.field_or_region_ref.clone(),
        metric: format!("drought_{}", record.index_type.as_str()),
        unit: "z_score".to_string(),
        t: record.period.end.clone(),
        value: SeriesValue::Scalar {
            value: record.value,
        },
        source_ref: record.index_id.clone(),
        created_at: record.computed_at.clone(),
    };

    insert_time_series_point_record(state, &point, Some(metadata)).await
}

async fn insert_marketplace_account_record(
    state: &AppState,
    record: &MarketplaceAccountRecord,
) -> AppResult<()> {
    let role_refs_json =
        serde_json::to_string(&record.role_refs).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO marketplace_accounts (
            account_id, org_id, party_type, role_refs_json, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&record.account_id)
    .bind(&record.org_id)
    .bind(record.party_type.as_str())
    .bind(role_refs_json)
    .bind(record.status.as_str())
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn update_marketplace_account_record(
    state: &AppState,
    record: &MarketplaceAccountRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE marketplace_accounts
        SET status = ?2, updated_at = ?3
        WHERE account_id = ?1
        "#,
    )
    .bind(&record.account_id)
    .bind(record.status.as_str())
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_marketplace_catalog_item_record(
    state: &AppState,
    record: &MarketplaceCatalogItemRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO marketplace_catalog_items (
            item_id, org_id, kind, category, name, unit_of_measure, owner_account_id, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&record.item_id)
    .bind(&record.org_id)
    .bind(record.kind.as_str())
    .bind(record.category.as_str())
    .bind(&record.name)
    .bind(record.unit_of_measure.as_str())
    .bind(&record.owner_account_id)
    .bind(&record.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_marketplace_account(
    state: &AppState,
    account_id: &str,
) -> AppResult<Option<MarketplaceAccountRecord>> {
    let row = sqlx::query(
        r#"
        SELECT account_id, org_id, party_type, role_refs_json, status, created_at, updated_at
        FROM marketplace_accounts
        WHERE account_id = ?1
        "#,
    )
    .bind(account_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_marketplace_account_record(&row))
        .transpose()
}

async fn load_marketplace_catalog_item(
    state: &AppState,
    item_id: &str,
) -> AppResult<Option<MarketplaceCatalogItemRecord>> {
    let row = sqlx::query(
        r#"
        SELECT item_id, org_id, kind, category, name, unit_of_measure, owner_account_id, created_at
        FROM marketplace_catalog_items
        WHERE item_id = ?1
        "#,
    )
    .bind(item_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_marketplace_catalog_item_record(&row))
        .transpose()
}

async fn insert_marketplace_listing_record(
    state: &AppState,
    record: &MarketplaceListingRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO marketplace_listings (
            listing_id, item_id, org_id, price, currency, available_qty,
            window_from, window_to, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(&record.listing_id)
    .bind(&record.item_id)
    .bind(&record.org_id)
    .bind(record.price)
    .bind(&record.currency)
    .bind(record.available_qty)
    .bind(&record.window.from)
    .bind(&record.window.to)
    .bind(record.status.as_str())
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn update_marketplace_listing_record(
    state: &AppState,
    record: &MarketplaceListingRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE marketplace_listings
        SET status = ?2, updated_at = ?3
        WHERE listing_id = ?1
        "#,
    )
    .bind(&record.listing_id)
    .bind(record.status.as_str())
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_marketplace_listing(
    state: &AppState,
    listing_id: &str,
) -> AppResult<Option<MarketplaceListingRecord>> {
    let row = sqlx::query(
        r#"
        SELECT listing_id, item_id, org_id, price, currency, available_qty,
               window_from, window_to, status, created_at, updated_at
        FROM marketplace_listings
        WHERE listing_id = ?1
        "#,
    )
    .bind(listing_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_marketplace_listing_record(&row))
        .transpose()
}

async fn upsert_marketplace_inventory_record(
    state: &AppState,
    record: &MarketplaceInventoryRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO marketplace_inventory (
            inventory_id, item_id, org_id, on_hand, reserved, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(inventory_id) DO UPDATE SET
            item_id = excluded.item_id,
            org_id = excluded.org_id,
            on_hand = excluded.on_hand,
            reserved = excluded.reserved,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&record.inventory_id)
    .bind(&record.item_id)
    .bind(&record.org_id)
    .bind(record.on_hand)
    .bind(record.reserved)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_marketplace_inventory(
    state: &AppState,
    inventory_id: &str,
) -> AppResult<Option<MarketplaceInventoryRecord>> {
    let row = sqlx::query(
        r#"
        SELECT inventory_id, item_id, org_id, on_hand, reserved, updated_at
        FROM marketplace_inventory
        WHERE inventory_id = ?1
        "#,
    )
    .bind(inventory_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_marketplace_inventory_record(&row))
        .transpose()
}

async fn load_marketplace_inventory_by_item(
    state: &AppState,
    item_id: &str,
    org_id: &str,
) -> AppResult<Option<MarketplaceInventoryRecord>> {
    let row = sqlx::query(
        r#"
        SELECT inventory_id, item_id, org_id, on_hand, reserved, updated_at
        FROM marketplace_inventory
        WHERE item_id = ?1 AND org_id = ?2
        ORDER BY inventory_id ASC
        LIMIT 1
        "#,
    )
    .bind(item_id)
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_marketplace_inventory_record(&row))
        .transpose()
}

async fn update_marketplace_inventory_reserve(
    state: &AppState,
    inventory_id: &str,
    org_id: &str,
    qty: f64,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"
        UPDATE marketplace_inventory
        SET reserved = reserved + ?3, updated_at = ?4
        WHERE inventory_id = ?1
          AND org_id = ?2
          AND reserved + ?3 <= on_hand
        "#,
    )
    .bind(inventory_id)
    .bind(org_id)
    .bind(qty)
    .bind(current_record_timestamp())
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    if result.rows_affected() == 0 {
        return Err(marketplace_inventory_error(
            MarketplaceInventoryError::InsufficientAvailableQuantity,
        ));
    }
    Ok(())
}

async fn update_marketplace_inventory_fulfill(
    state: &AppState,
    inventory_id: &str,
    org_id: &str,
    qty: f64,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"
        UPDATE marketplace_inventory
        SET on_hand = on_hand - ?3, reserved = reserved - ?3, updated_at = ?4
        WHERE inventory_id = ?1
          AND org_id = ?2
          AND reserved >= ?3
        "#,
    )
    .bind(inventory_id)
    .bind(org_id)
    .bind(qty)
    .bind(current_record_timestamp())
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    if result.rows_affected() == 0 {
        return Err(marketplace_inventory_error(
            MarketplaceInventoryError::InsufficientReservedQuantity,
        ));
    }
    Ok(())
}

async fn update_marketplace_inventory_release(
    state: &AppState,
    inventory_id: &str,
    org_id: &str,
    qty: f64,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"
        UPDATE marketplace_inventory
        SET reserved = reserved - ?3, updated_at = ?4
        WHERE inventory_id = ?1
          AND org_id = ?2
          AND reserved >= ?3
        "#,
    )
    .bind(inventory_id)
    .bind(org_id)
    .bind(qty)
    .bind(current_record_timestamp())
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    if result.rows_affected() == 0 {
        return Err(marketplace_inventory_error(
            MarketplaceInventoryError::InsufficientReservedQuantity,
        ));
    }
    Ok(())
}

async fn insert_marketplace_order_record(
    state: &AppState,
    record: &MarketplaceOrderRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO marketplace_orders (
            order_id, org_id, listing_ref, buyer_account_id, qty, line_total,
            status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(&record.order_id)
    .bind(&record.org_id)
    .bind(&record.listing_ref)
    .bind(&record.buyer_account_id)
    .bind(record.qty)
    .bind(record.line_total)
    .bind(record.status.as_str())
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn update_marketplace_order_record(
    state: &AppState,
    record: &MarketplaceOrderRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE marketplace_orders
        SET status = ?2, updated_at = ?3
        WHERE order_id = ?1
        "#,
    )
    .bind(&record.order_id)
    .bind(record.status.as_str())
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_marketplace_order(
    state: &AppState,
    order_id: &str,
) -> AppResult<Option<MarketplaceOrderRecord>> {
    let row = sqlx::query(
        r#"
        SELECT order_id, org_id, listing_ref, buyer_account_id, qty, line_total,
               status, created_at, updated_at
        FROM marketplace_orders
        WHERE order_id = ?1
        "#,
    )
    .bind(order_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_marketplace_order_record(&row))
        .transpose()
}

async fn insert_marketplace_order_audit_record(
    state: &AppState,
    record: &MarketplaceOrderAuditRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO marketplace_order_audits (
            audit_id, order_id, from_status, to_status, actor_id, occurred_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&record.audit_id)
    .bind(&record.order_id)
    .bind(record.from_status.map(|status| status.as_str().to_string()))
    .bind(record.to_status.as_str())
    .bind(&record.actor_id)
    .bind(&record.occurred_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn marketplace_org_exists(state: &AppState, org_id: &str) -> AppResult<bool> {
    let exists: i64 = sqlx::query_scalar(
        r#"
        SELECT CASE
            WHEN EXISTS(SELECT 1 FROM farms WHERE owner = ?1)
              OR EXISTS(SELECT 1 FROM fields WHERE owner = ?1)
            THEN 1 ELSE 0
        END
        "#,
    )
    .bind(org_id)
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(exists != 0)
}

async fn insert_sustainability_record(
    state: &AppState,
    record: &SustainabilityRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO sustainability_records (
            record_id, field_id, season_id, operation_id, metric_type, method_version,
            created_at, audit_id
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&record.record_id)
    .bind(&record.field_id)
    .bind(&record.season_id)
    .bind(&record.operation_id)
    .bind(record.metric_type.as_str())
    .bind(&record.method_version)
    .bind(&record.created_at)
    .bind(&record.audit_id)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_sustainability_record(
    state: &AppState,
    record_id: &str,
) -> AppResult<Option<SustainabilityRecord>> {
    let row = sqlx::query(
        r#"
        SELECT record_id, field_id, season_id, operation_id, metric_type, method_version,
               created_at, audit_id
        FROM sustainability_records
        WHERE record_id = ?1
        "#,
    )
    .bind(record_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_sustainability_record(&row))
        .transpose()
}

async fn load_sustainability_record_linkage(
    state: &AppState,
    field_id: &str,
) -> AppResult<Option<SustainabilityRecordLinkage>> {
    let row = sqlx::query("SELECT field_id, season FROM fields WHERE field_id = ?1")
        .bind(field_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(Error::from)?;

    Ok(row.map(|row| SustainabilityRecordLinkage {
        field_id: row.get("field_id"),
        season_id: row.get("season"),
    }))
}

async fn insert_content_item_with_version(
    state: &AppState,
    content: &ContentRecord,
    version: &ContentVersionRecord,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    sqlx::query(
        r#"
        INSERT INTO cms_contents (
            content_id, content_type, author_id, org_id, status, current_version,
            created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&content.content_id)
    .bind(content.content_type.as_str())
    .bind(&content.author_id)
    .bind(&content.org_id)
    .bind(content.status.as_str())
    .bind(&content.current_version)
    .bind(&content.created_at)
    .bind(&content.updated_at)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;
    insert_content_version_in_tx(&mut tx, version).await?;
    tx.commit().await.map_err(Error::from)?;

    Ok(())
}

async fn append_content_version_record(
    state: &AppState,
    content: &ContentRecord,
    version: &ContentVersionRecord,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    insert_content_version_in_tx(&mut tx, version).await?;
    sqlx::query(
        r#"
        UPDATE cms_contents
        SET current_version = ?2, updated_at = ?3
        WHERE content_id = ?1
        "#,
    )
    .bind(&content.content_id)
    .bind(&content.current_version)
    .bind(&content.updated_at)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;
    tx.commit().await.map_err(Error::from)?;

    Ok(())
}

async fn insert_content_version_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    version: &ContentVersionRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO cms_content_versions (version_id, content_id, body, created_at)
        VALUES (?1, ?2, ?3, ?4)
        "#,
    )
    .bind(&version.version_id)
    .bind(&version.content_id)
    .bind(&version.body)
    .bind(&version.created_at)
    .execute(&mut **tx)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_content_record(
    state: &AppState,
    content_id: &str,
) -> AppResult<Option<ContentRecord>> {
    let row = sqlx::query(
        r#"
        SELECT content_id, content_type, author_id, org_id, status, current_version,
               created_at, updated_at
        FROM cms_contents
        WHERE content_id = ?1
        "#,
    )
    .bind(content_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_content_record(&row)).transpose()
}

async fn load_versioned_content(
    state: &AppState,
    content_id: &str,
    org_id: &str,
) -> AppResult<Option<VersionedContentRecord>> {
    let Some(content) = load_content_record(state, content_id).await? else {
        return Ok(None);
    };
    if content.org_id != org_id {
        return Ok(None);
    }
    let versions = load_content_versions(state, content_id).await?;

    Ok(Some(VersionedContentRecord { content, versions }))
}

async fn load_content_versions(
    state: &AppState,
    content_id: &str,
) -> AppResult<Vec<ContentVersionRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT version_id, content_id, body, created_at
        FROM cms_content_versions
        WHERE content_id = ?1
        ORDER BY rowid ASC
        "#,
    )
    .bind(content_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_content_version_record(&row))
        .collect()
}

async fn insert_collaboration_channel(
    state: &AppState,
    channel: &CollaborationChannelRecord,
) -> AppResult<()> {
    let member_account_ids_json = serde_json::to_string(&channel.member_account_ids)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO collab_channels (
            channel_id, org_id, field_ref, member_account_ids_json, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(&channel.channel_id)
    .bind(&channel.org_id)
    .bind(&channel.field_ref)
    .bind(member_account_ids_json)
    .bind(&channel.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_collaboration_message(
    state: &AppState,
    message: &CollaborationMessageRecord,
    org_id: &str,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    sqlx::query(
        r#"
        INSERT INTO collab_messages (message_id, channel_id, author_id, body, sent_at)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(&message.message_id)
    .bind(&message.channel_id)
    .bind(&message.author_id)
    .bind(&message.body)
    .bind(&message.sent_at)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;

    sqlx::query(
        r#"
        INSERT INTO collab_message_audits (
            audit_id, message_id, channel_id, org_id, actor_id, event_type, occurred_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(format!("collab-message-audit-{}", Uuid::new_v4()))
    .bind(&message.message_id)
    .bind(&message.channel_id)
    .bind(org_id)
    .bind(&message.author_id)
    .bind("message_posted")
    .bind(&message.sent_at)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;

    tx.commit().await.map_err(Error::from)?;

    Ok(())
}

async fn load_collaboration_channel(
    state: &AppState,
    channel_id: &str,
) -> AppResult<Option<CollaborationChannelRecord>> {
    let row = sqlx::query(
        r#"
        SELECT channel_id, org_id, field_ref, member_account_ids_json, created_at
        FROM collab_channels
        WHERE channel_id = ?1
        "#,
    )
    .bind(channel_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_collaboration_channel(&row))
        .transpose()
}

async fn load_collaboration_thread(
    state: &AppState,
    channel_id: &str,
    org_id: &str,
) -> AppResult<Option<CollaborationChannelThread>> {
    let Some(channel) = load_collaboration_channel(state, channel_id).await? else {
        return Ok(None);
    };
    if channel.org_id != org_id {
        return Ok(None);
    }
    let messages = load_collaboration_messages(state, channel_id).await?;

    Ok(Some(CollaborationChannelThread { channel, messages }))
}

async fn load_collaboration_messages(
    state: &AppState,
    channel_id: &str,
) -> AppResult<Vec<CollaborationMessageRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT message_id, channel_id, author_id, body, sent_at
        FROM collab_messages
        WHERE channel_id = ?1
        ORDER BY rowid ASC
        "#,
    )
    .bind(channel_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_collaboration_message(&row))
        .collect()
}

async fn load_latest_alert_rule(
    state: &AppState,
    rule_id: &str,
) -> AppResult<Option<AlertRuleRecord>> {
    let row = sqlx::query(
        r#"
        SELECT rule_id, version, event_type, subject_ref, severity, channels_json, status,
               created_at, updated_at
        FROM alert_rules
        WHERE rule_id = ?1
        ORDER BY version DESC
        LIMIT 1
        "#,
    )
    .bind(rule_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_alert_rule_record(&row)).transpose()
}

async fn load_plugin_registration(
    state: &AppState,
    plugin_id: &str,
) -> AppResult<Option<PluginRegistrationRecord>> {
    let row = sqlx::query(
        r#"
        SELECT plugin_id, name, version, kind, host_api_version, capabilities_json, entrypoint,
               status
        FROM plugin_registrations
        WHERE plugin_id = ?1
        "#,
    )
    .bind(plugin_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_plugin_registration(&row)).transpose()
}

async fn load_provenance_audit_entries_for_append(state: &AppState) -> AppResult<Vec<AuditEntry>> {
    let rows = sqlx::query(
        r#"
        SELECT entry_hash, seq, prev_hash, payload_hash, actor_id, actor_kind, ts, action_ref,
               action_kind, artifact_ref, payload_json, occurred_at, outcome, refusal_reason
        FROM provenance_audit_entries
        ORDER BY seq ASC
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_audit_entry(&row))
        .collect()
}

async fn validate_collaboration_field_ref(
    state: &AppState,
    org_id: &str,
    field_ref: &str,
) -> AppResult<()> {
    let Some(field_id) = field_ref.strip_prefix("field:") else {
        return Err(AppError::BadRequest(
            "collaboration field_ref must use field:<field_id>".to_string(),
        ));
    };
    let field_id = normalize_optional_text(Some(field_id.to_string()))
        .ok_or_else(|| AppError::BadRequest("collaboration field_ref is required".to_string()))?;

    assert_field_owned_by_org(state, org_id, &field_id).await
}

async fn load_soil_iot_device(
    state: &AppState,
    device_id: &str,
) -> AppResult<Option<SoilDeviceRecord>> {
    let row = sqlx::query(
        r#"
        SELECT device_id, org_id, field_id, zone_id, sensor_type, latitude, longitude, crs,
               calibration_profile_ref, status, created_at, updated_at
        FROM soil_iot_devices
        WHERE device_id = ?1
        "#,
    )
    .bind(device_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_soil_iot_device(&row)).transpose()
}

async fn load_soil_iot_config_push(
    state: &AppState,
    push_id: &str,
) -> AppResult<Option<SoilDeviceConfigPushRecord>> {
    let row = sqlx::query(
        r#"
        SELECT push_id, device_id, config_version, pushed_at, push_status, failure_reason, updated_at
        FROM soil_iot_config_pushes
        WHERE push_id = ?1
        "#,
    )
    .bind(push_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_soil_iot_config_push(&row)).transpose()
}

async fn append_fleet_component_event(
    state: &AppState,
    event: &FleetComponentEventRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO fleet_component_events
            (component_id, event_type, airframe_id, event_at, actor, details)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&event.component_id)
    .bind(&event.event_type)
    .bind(&event.airframe_id)
    .bind(&event.event_at)
    .bind(&event.actor)
    .bind(&event.details)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_fleet_component(
    state: &AppState,
    component_id: &str,
) -> AppResult<Option<FleetComponentRecord>> {
    sqlx::query(
        r#"
        SELECT component_id, component_type, serial, airframe_id, installed_at, removed_at,
               service_history_json, flight_hours, cycles, duty_score, created_at, updated_at
        FROM fleet_components
        WHERE component_id = ?1
        "#,
    )
    .bind(component_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?
    .map(|row| decode_fleet_component_record(&row))
    .transpose()
}

async fn load_active_fleet_components_for_airframe(
    state: &AppState,
    airframe_id: &str,
) -> AppResult<Vec<FleetComponentRecord>> {
    let airframe_id = normalize_optional_text(Some(airframe_id.to_string()))
        .ok_or_else(|| AppError::BadRequest("airframe_id is required".to_string()))?;
    let rows = sqlx::query(
        r#"
        SELECT component_id, component_type, serial, airframe_id, installed_at, removed_at,
               service_history_json, flight_hours, cycles, duty_score, created_at, updated_at
        FROM fleet_components
        WHERE airframe_id = ?1
          AND removed_at IS NULL
        ORDER BY component_id ASC
        "#,
    )
    .bind(airframe_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_fleet_component_record(&row))
        .collect()
}

async fn load_component_duty_accruals_for_session(
    state: &AppState,
    session_id: &str,
    airframe_id: &str,
) -> AppResult<Vec<ComponentDutyAccrualRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT session_id, component_id, airframe_id, flight_hours, cycles, duty_score, accrued_at
        FROM fleet_component_duty_accruals
        WHERE session_id = ?1
          AND airframe_id = ?2
        ORDER BY component_id ASC
        "#,
    )
    .bind(session_id)
    .bind(airframe_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_component_duty_accrual(&row))
        .collect()
}

async fn validate_orthomosaic_linkage(
    state: &AppState,
    scene_id: &str,
    field_id: &str,
    season_id: &str,
) -> AppResult<()> {
    let scene_id = normalize_optional_text(Some(scene_id.to_string()))
        .ok_or_else(|| AppError::BadRequest("scene_id is required".to_string()))?;
    let field_id = normalize_optional_text(Some(field_id.to_string()))
        .ok_or_else(|| AppError::BadRequest("field_id is required".to_string()))?;
    let season_id = normalize_optional_text(Some(season_id.to_string()))
        .ok_or_else(|| AppError::BadRequest("season_id is required".to_string()))?;
    let field = load_field(state, &field_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("field {field_id} does not exist")))?;
    if field
        .season
        .as_deref()
        .is_some_and(|field_season| field_season != season_id)
    {
        return Err(AppError::BadRequest(format!(
            "field {field_id} is linked to season {}, not {season_id}",
            field.season.unwrap_or_default()
        )));
    }

    let scene_row = sqlx::query("SELECT field_id, season_id FROM scenes WHERE scene_id = ?1")
        .bind(&scene_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(Error::from)?
        .ok_or_else(|| AppError::BadRequest(format!("scene {scene_id} does not exist")))?;
    let scene_field_id: Option<String> = scene_row.get("field_id");
    if scene_field_id
        .as_deref()
        .is_some_and(|scene_field_id| scene_field_id != field_id)
    {
        return Err(AppError::BadRequest(format!(
            "scene {scene_id} is linked to field {}, not {field_id}",
            scene_field_id.unwrap_or_default()
        )));
    }
    let scene_season_id: Option<String> = scene_row.get("season_id");
    if scene_season_id
        .as_deref()
        .is_some_and(|scene_season_id| scene_season_id != season_id)
    {
        return Err(AppError::BadRequest(format!(
            "scene {scene_id} is linked to season {}, not {season_id}",
            scene_season_id.unwrap_or_default()
        )));
    }

    Ok(())
}

async fn orthomosaic_frame_set_exists(state: &AppState, frame_set_id: &str) -> AppResult<bool> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM orthomosaic_frame_sets WHERE frame_set_id = ?1")
            .bind(frame_set_id)
            .fetch_one(&state.pool)
            .await
            .map_err(Error::from)?;

    Ok(count > 0)
}

async fn load_orthomosaic_frame_set(
    state: &AppState,
    frame_set_id: &str,
) -> AppResult<Option<FrameSetRecord>> {
    let row = sqlx::query(
        r#"
        SELECT frame_set_id, scene_id, field_id, season_id, frames_json, crs_hint, created_at
        FROM orthomosaic_frame_sets
        WHERE frame_set_id = ?1
        "#,
    )
    .bind(frame_set_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_orthomosaic_frame_set_record(&row))
        .transpose()
}

async fn load_orthomosaic_reconstruction(
    state: &AppState,
    recon_id: &str,
) -> AppResult<Option<ReconstructionJobRecord>> {
    let row = sqlx::query(
        r#"
        SELECT recon_id, frame_set_id, params_json, status, failure_reason, created_at, updated_at
        FROM orthomosaic_reconstructions
        WHERE recon_id = ?1
        "#,
    )
    .bind(recon_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_orthomosaic_reconstruction_record(&row))
        .transpose()
}

async fn assert_copilot_field_exists(state: &AppState, field_id: &str) -> AppResult<()> {
    if load_field(state, field_id).await?.is_some() {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "copilot field scope {field_id} does not exist"
        )))
    }
}

async fn insert_copilot_conversation(
    state: &AppState,
    conversation: &CopilotConversationRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO copilot_conversations (conversation_id, field_id, created_at)
        VALUES (?1, ?2, ?3)
        "#,
    )
    .bind(&conversation.conversation_id)
    .bind(&conversation.field_id)
    .bind(&conversation.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_copilot_conversation(
    state: &AppState,
    conversation_id: &str,
) -> AppResult<Option<CopilotConversationRecord>> {
    let row = sqlx::query(
        r#"
        SELECT conversation_id, field_id, created_at
        FROM copilot_conversations
        WHERE conversation_id = ?1
        "#,
    )
    .bind(conversation_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_copilot_conversation(&row)).transpose()
}

fn decode_copilot_conversation(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<CopilotConversationRecord> {
    Ok(CopilotConversationRecord {
        conversation_id: row.get("conversation_id"),
        field_id: row.get("field_id"),
        created_at: row.get("created_at"),
    })
}

async fn insert_copilot_turn(state: &AppState, turn: &CopilotTurnRecord) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO copilot_turns (turn_id, conversation_id, field_id, role, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(&turn.turn_id)
    .bind(&turn.conversation_id)
    .bind(&turn.field_id)
    .bind(turn.role.as_str())
    .bind(&turn.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

fn decode_copilot_turn(row: &sqlx::sqlite::SqliteRow) -> AppResult<CopilotTurnRecord> {
    Ok(CopilotTurnRecord {
        conversation_id: row.get("conversation_id"),
        field_id: row.get("field_id"),
        turn_id: row.get("turn_id"),
        role: parse_copilot_turn_role(row.get("role"))?,
        created_at: row.get("created_at"),
    })
}

async fn crop_inference_mosaic_is_published(
    state: &AppState,
    mosaic_ref: &str,
    field_id: &str,
    season_id: &str,
) -> AppResult<bool> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM products
        WHERE product_id = ?1
          AND field_id = ?2
          AND season_id = ?3
          AND lower(publish_status) = 'published'
          AND provenance_hash IS NOT NULL
          AND trim(provenance_hash) <> ''
        "#,
    )
    .bind(mosaic_ref)
    .bind(field_id)
    .bind(season_id)
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(count > 0)
}

async fn insert_crop_inference_run(state: &AppState, record: &InferenceRunRecord) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO crop_inference_runs (
            run_id, mosaic_ref, field_id, season_id, model_id, model_version,
            status, failure_reason_code, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(&record.run_id)
    .bind(&record.mosaic_ref)
    .bind(&record.field_id)
    .bind(&record.season_id)
    .bind(&record.model_id)
    .bind(&record.model_version)
    .bind(record.status.as_str())
    .bind(&record.failure_reason_code)
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn update_crop_inference_run(state: &AppState, record: &InferenceRunRecord) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE crop_inference_runs
        SET status = ?2,
            failure_reason_code = ?3,
            updated_at = ?4
        WHERE run_id = ?1
        "#,
    )
    .bind(&record.run_id)
    .bind(record.status.as_str())
    .bind(&record.failure_reason_code)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_crop_inference_run(
    state: &AppState,
    run_id: &str,
) -> AppResult<Option<InferenceRunRecord>> {
    let row = sqlx::query(
        r#"
        SELECT run_id, mosaic_ref, field_id, season_id, model_id, model_version,
               status, failure_reason_code, created_at, updated_at
        FROM crop_inference_runs
        WHERE run_id = ?1
        "#,
    )
    .bind(run_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_crop_inference_run(&row)).transpose()
}

fn decode_crop_inference_run(row: &sqlx::sqlite::SqliteRow) -> AppResult<InferenceRunRecord> {
    let status = row
        .get::<String, _>("status")
        .parse::<InferenceRunStatus>()
        .map_err(crop_inference_run_error)?;

    Ok(InferenceRunRecord {
        run_id: row.get("run_id"),
        mosaic_ref: row.get("mosaic_ref"),
        field_id: row.get("field_id"),
        season_id: row.get("season_id"),
        model_id: row.get("model_id"),
        model_version: row.get("model_version"),
        status,
        failure_reason_code: row.get("failure_reason_code"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

async fn crop_model_exists(state: &AppState, model_id: &str, version: &str) -> AppResult<bool> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM crop_models WHERE model_id = ?1 AND version = ?2")
            .bind(model_id)
            .bind(version)
            .fetch_one(&state.pool)
            .await
            .map_err(Error::from)?;

    Ok(count > 0)
}

async fn audit_crop_model_event(
    state: &AppState,
    model_id: &str,
    version: &str,
    event_type: &str,
    details: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO crop_model_events (model_id, version, event_type, created_at, details)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(model_id)
    .bind(version)
    .bind(event_type)
    .bind(current_record_timestamp())
    .bind(details)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn persist_crop_detection_verification(
    state: &AppState,
    record: &CropDetectionVerificationRecord,
) -> AppResult<()> {
    let evidence_tile_refs_json = serde_json::to_string(&record.evidence_tile_refs)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let zone_geometry_json =
        serde_json::to_string(&record.zone_geometry).map_err(|err| AppError::Anyhow(err.into()))?;
    let corrected_geometry_json = record
        .corrected_geometry
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let correction_label_json = record
        .correction_label
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|err| AppError::Anyhow(err.into()))?;

    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    sqlx::query(
        r#"
        INSERT INTO crop_detection_verifications (
            detection_id, task, label, confidence, evidence_tile_refs_json,
            zone_geometry_json, verification_state, actor, verified_at,
            corrected_label, corrected_geometry_json, correction_label_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        ON CONFLICT(detection_id) DO UPDATE SET
            task = excluded.task,
            label = excluded.label,
            confidence = excluded.confidence,
            evidence_tile_refs_json = excluded.evidence_tile_refs_json,
            zone_geometry_json = excluded.zone_geometry_json,
            verification_state = excluded.verification_state,
            actor = excluded.actor,
            verified_at = excluded.verified_at,
            corrected_label = excluded.corrected_label,
            corrected_geometry_json = excluded.corrected_geometry_json,
            correction_label_json = excluded.correction_label_json
        "#,
    )
    .bind(&record.detection_id)
    .bind(record.task.as_str())
    .bind(&record.label)
    .bind(record.confidence)
    .bind(&evidence_tile_refs_json)
    .bind(&zone_geometry_json)
    .bind(record.verification_state.as_str())
    .bind(&record.actor)
    .bind(&record.verified_at)
    .bind(&record.corrected_label)
    .bind(&corrected_geometry_json)
    .bind(&correction_label_json)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;

    sqlx::query("DELETE FROM crop_detection_correction_labels WHERE source_detection_id = ?1")
        .bind(&record.detection_id)
        .execute(&mut *tx)
        .await
        .map_err(Error::from)?;
    if let Some(label) = record.correction_label.as_ref() {
        persist_crop_detection_correction_label(&mut tx, label).await?;
    }

    tx.commit().await.map_err(Error::from)?;
    Ok(())
}

async fn persist_crop_detection_correction_label(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    label: &CropDetectionCorrectionLabel,
) -> AppResult<()> {
    let geometry_json =
        serde_json::to_string(&label.geometry).map_err(|err| AppError::Anyhow(err.into()))?;
    let evidence_tile_refs_json = serde_json::to_string(&label.evidence_tile_refs)
        .map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO crop_detection_correction_labels (
            label_id, source_detection_id, task, label, geometry_json,
            actor, created_at, evidence_tile_refs_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&label.label_id)
    .bind(&label.source_detection_id)
    .bind(label.task.as_str())
    .bind(&label.label)
    .bind(geometry_json)
    .bind(&label.actor)
    .bind(&label.created_at)
    .bind(evidence_tile_refs_json)
    .execute(&mut **tx)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_crop_detection_verification_state(
    state: &AppState,
    detection_id: &str,
) -> AppResult<Option<DetectionVerificationState>> {
    let state_value = sqlx::query_scalar::<_, String>(
        "SELECT verification_state FROM crop_detection_verifications WHERE detection_id = ?1",
    )
    .bind(detection_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    state_value
        .map(parse_detection_verification_state)
        .transpose()
}

async fn load_crop_detection_verification_record(
    state: &AppState,
    detection_id: &str,
) -> AppResult<Option<CropDetectionVerificationRecord>> {
    let row = sqlx::query(
        r#"
        SELECT detection_id, task, label, confidence, evidence_tile_refs_json,
               zone_geometry_json, verification_state, actor, verified_at,
               corrected_label, corrected_geometry_json, correction_label_json
        FROM crop_detection_verifications
        WHERE detection_id = ?1
        "#,
    )
    .bind(detection_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_crop_detection_verification_record(&row))
        .transpose()
}

fn decode_crop_detection_verification_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<CropDetectionVerificationRecord> {
    let evidence_tile_refs =
        serde_json::from_str::<Vec<String>>(&row.get::<String, _>("evidence_tile_refs_json"))
            .map_err(|err| {
                AppError::Anyhow(
                    Error::new(err)
                        .context("failed to decode crop detection evidence_tile_refs_json"),
                )
            })?;
    let zone_geometry =
        serde_json::from_str::<DetectionZoneGeometry>(&row.get::<String, _>("zone_geometry_json"))
            .map_err(|err| {
                AppError::Anyhow(
                    Error::new(err).context("failed to decode crop detection zone_geometry_json"),
                )
            })?;
    let corrected_geometry = row
        .get::<Option<String>, _>("corrected_geometry_json")
        .map(|json| serde_json::from_str::<DetectionZoneGeometry>(&json))
        .transpose()
        .map_err(|err| {
            AppError::Anyhow(
                Error::new(err).context("failed to decode crop detection corrected_geometry_json"),
            )
        })?;
    let correction_label = row
        .get::<Option<String>, _>("correction_label_json")
        .map(|json| serde_json::from_str::<CropDetectionCorrectionLabel>(&json))
        .transpose()
        .map_err(|err| {
            AppError::Anyhow(
                Error::new(err).context("failed to decode crop detection correction_label_json"),
            )
        })?;

    Ok(CropDetectionVerificationRecord {
        detection_id: row.get("detection_id"),
        task: parse_crop_model_task(row.get::<String, _>("task"))?,
        label: row.get("label"),
        confidence: row.get("confidence"),
        evidence_tile_refs,
        zone_geometry,
        verification_state: parse_detection_verification_state(
            row.get::<String, _>("verification_state"),
        )?,
        actor: row.get("actor"),
        verified_at: row.get("verified_at"),
        corrected_label: row.get("corrected_label"),
        corrected_geometry,
        correction_label,
    })
}

fn annotation_from_crop_detection_finding(
    scene_id: &str,
    finding: &CropDetectionFindingRecord,
) -> AppResult<AnnotationRecord> {
    let geometry = annotation_geometry_from_detection(&finding.zone_geometry);
    validate_annotation_geometry(&geometry)?;
    Ok(AnnotationRecord {
        annotation_id: format!("{}-zone", finding.finding_id),
        scene_id: scene_id.to_string(),
        field_id: Some(finding.field_id.clone()),
        author: Some("crop_intelligence".to_string()),
        crs: Some(finding.zone_geometry.crs.clone()),
        audit_id: Some(format!("crop-finding:{}", finding.finding_id)),
        label: finding.label.clone(),
        note: Some(format!(
            "{} finding from detection {} with confidence {:.2}",
            finding.finding_type.as_str(),
            finding.detection_id,
            finding.confidence
        )),
        severity: Some("medium".to_string()),
        geometry,
        created_at: finding.emitted_at.clone(),
        updated_at: finding.emitted_at.clone(),
    })
}

fn annotation_geometry_from_detection(geometry: &DetectionZoneGeometry) -> AnnotationGeometry {
    let bbox = &geometry.bbox;
    AnnotationGeometry::Polygon {
        coordinates: vec![
            GeoPoint {
                longitude: bbox.min_lon,
                latitude: bbox.min_lat,
            },
            GeoPoint {
                longitude: bbox.max_lon,
                latitude: bbox.min_lat,
            },
            GeoPoint {
                longitude: bbox.max_lon,
                latitude: bbox.max_lat,
            },
            GeoPoint {
                longitude: bbox.min_lon,
                latitude: bbox.max_lat,
            },
            GeoPoint {
                longitude: bbox.min_lon,
                latitude: bbox.min_lat,
            },
        ],
    }
}

fn recommendation_from_crop_detection_finding(
    scene_id: &str,
    finding: &CropDetectionFindingRecord,
    annotation: &AnnotationRecord,
) -> RecommendationRecord {
    let annotation_ids = vec![annotation.annotation_id.clone()];
    let evidence_refs = combine_text_values(
        recommendation_evidence_from_annotations(&annotation_ids),
        finding.evidence_refs.clone(),
    );

    RecommendationRecord {
        recommendation_id: finding.finding_id.clone(),
        scene_id: scene_id.to_string(),
        field_id: Some(finding.field_id.clone()),
        org_id: DEFAULT_RECORD_OWNER.to_string(),
        author_user_id: "crop_intelligence".to_string(),
        title: format!("Crop intelligence finding: {}", finding.label),
        note: Some(format!(
            "Detection {} confidence {:.2}; model {}@{}; verification {}.",
            finding.detection_id,
            finding.confidence,
            finding.model_version.model_id,
            finding.model_version.version,
            finding.verification_state.as_str()
        )),
        category: Some("crop_intelligence_finding".to_string()),
        action_category: "crop_intelligence_finding".to_string(),
        priority: RecommendationPriority::Medium,
        status: RecommendationStatus::Open,
        evidence_refs,
        annotation_ids,
        created_at: finding.emitted_at.clone(),
        updated_at: finding.emitted_at.clone(),
    }
}

async fn persist_crop_detection_finding_recommendation(
    state: &AppState,
    annotation: &AnnotationRecord,
    recommendation: &RecommendationRecord,
) -> AppResult<()> {
    let geometry_json =
        serde_json::to_string(&annotation.geometry).map_err(|err| AppError::Anyhow(err.into()))?;
    let evidence_refs_json = serde_json::to_string(&recommendation.evidence_refs)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let mut tx = state.pool.begin().await.map_err(Error::from)?;

    sqlx::query(
        r#"
        INSERT INTO annotations (
            annotation_id, scene_id, field_id, author, crs, audit_id, label,
            note, severity, geometry_json, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        ON CONFLICT(annotation_id) DO UPDATE SET
            scene_id = excluded.scene_id,
            field_id = excluded.field_id,
            author = excluded.author,
            crs = excluded.crs,
            audit_id = excluded.audit_id,
            label = excluded.label,
            note = excluded.note,
            severity = excluded.severity,
            geometry_json = excluded.geometry_json,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&annotation.annotation_id)
    .bind(&annotation.scene_id)
    .bind(&annotation.field_id)
    .bind(&annotation.author)
    .bind(&annotation.crs)
    .bind(&annotation.audit_id)
    .bind(&annotation.label)
    .bind(&annotation.note)
    .bind(&annotation.severity)
    .bind(geometry_json)
    .bind(&annotation.created_at)
    .bind(&annotation.updated_at)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;

    sqlx::query(
        r#"
        INSERT INTO recommendations (
            recommendation_id, scene_id, field_id, title, note, category, priority,
            status, evidence_refs_json, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(recommendation_id) DO UPDATE SET
            scene_id = excluded.scene_id,
            field_id = excluded.field_id,
            title = excluded.title,
            note = excluded.note,
            category = excluded.category,
            priority = excluded.priority,
            status = excluded.status,
            evidence_refs_json = excluded.evidence_refs_json,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&recommendation.recommendation_id)
    .bind(&recommendation.scene_id)
    .bind(&recommendation.field_id)
    .bind(&recommendation.title)
    .bind(&recommendation.note)
    .bind(&recommendation.category)
    .bind(recommendation_priority_str(recommendation.priority))
    .bind(recommendation_status_str(recommendation.status))
    .bind(evidence_refs_json)
    .bind(&recommendation.created_at)
    .bind(&recommendation.updated_at)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;

    sqlx::query("DELETE FROM recommendation_annotations WHERE recommendation_id = ?1")
        .bind(&recommendation.recommendation_id)
        .execute(&mut *tx)
        .await
        .map_err(Error::from)?;
    for annotation_id in &recommendation.annotation_ids {
        sqlx::query(
            r#"
            INSERT INTO recommendation_annotations (recommendation_id, annotation_id)
            VALUES (?1, ?2)
            "#,
        )
        .bind(&recommendation.recommendation_id)
        .bind(annotation_id)
        .execute(&mut *tx)
        .await
        .map_err(Error::from)?;
    }

    tx.commit().await.map_err(Error::from)?;
    Ok(())
}

async fn assert_field_owned_by_org(
    state: &AppState,
    org_id: &str,
    field_id: &str,
) -> AppResult<()> {
    let owner: Option<String> = sqlx::query_scalar("SELECT owner FROM fields WHERE field_id = ?1")
        .bind(field_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(Error::from)?;

    match owner {
        Some(owner) if owner == org_id => Ok(()),
        Some(owner) => Err(AppError::BadRequest(format!(
            "field {field_id} belongs to org {owner}, not {org_id}"
        ))),
        None => Err(AppError::BadRequest(format!(
            "field {field_id} does not exist"
        ))),
    }
}

async fn insert_compliance_record(state: &AppState, record: &ComplianceRecord) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO compliance_records (
            record_id, version, record_type, org_id, field_id, flight_id, created_at,
            actor, provenance_ref, prior_version, change_reason, payload_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
    )
    .bind(&record.record_id)
    .bind(i64::from(record.version))
    .bind(record.record_type.as_str())
    .bind(&record.org_id)
    .bind(&record.field_id)
    .bind(&record.flight_id)
    .bind(&record.created_at)
    .bind(&record.actor)
    .bind(&record.provenance_ref)
    .bind(record.prior_version.map(i64::from))
    .bind(&record.change_reason)
    .bind(encode_compliance_payload(record)?)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_latest_compliance_record(
    state: &AppState,
    record_id: &str,
) -> AppResult<Option<ComplianceRecord>> {
    sqlx::query(
        r#"
        SELECT record_id, version, record_type, org_id, field_id, flight_id, created_at,
               actor, provenance_ref, prior_version, change_reason, payload_json
        FROM compliance_records
        WHERE record_id = ?1
        ORDER BY version DESC
        LIMIT 1
        "#,
    )
    .bind(record_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?
    .map(|row| decode_compliance_record(&row))
    .transpose()
}

async fn load_compliance_records_for_report(
    state: &AppState,
    org_id: &str,
    field_id: &str,
) -> AppResult<Vec<ComplianceRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT record_id, version, record_type, org_id, field_id, flight_id, created_at, actor, provenance_ref, prior_version, change_reason, payload_json
        FROM compliance_records
        WHERE org_id = ?1 AND field_id = ?2
        ORDER BY record_type ASC, record_id ASC, version ASC
        "#,
    )
    .bind(org_id)
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_compliance_record(&row))
        .collect()
}

fn default_compliance_report_mandatory_types() -> Vec<ComplianceRecordType> {
    vec![
        ComplianceRecordType::RemoteIdLog,
        ComplianceRecordType::ChemicalApplication,
        ComplianceRecordType::OperatorCertification,
        ComplianceRecordType::AuthorizationDecision,
    ]
}

async fn audit_compliance_record_event(
    state: &AppState,
    record_id: &str,
    event_type: &str,
    actor: Option<&str>,
    details: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO compliance_record_events (record_id, event_type, actor, created_at, details)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(record_id)
    .bind(event_type)
    .bind(actor)
    .bind(current_record_timestamp())
    .bind(details)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_airspace_zone(state: &AppState, record: &AirspaceZoneRecord) -> AppResult<()> {
    let geometry_json =
        serde_json::to_string(&record.coordinates).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO compliance_airspace_zones (
            zone_id, zone_class, crs, geometry_json, min_lon, min_lat, max_lon, max_lat,
            effective_from, effective_to, source, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
    )
    .bind(&record.zone_id)
    .bind(record.zone_class.as_str())
    .bind(&record.crs)
    .bind(geometry_json)
    .bind(record.extent.min_lon)
    .bind(record.extent.min_lat)
    .bind(record.extent.max_lon)
    .bind(record.extent.max_lat)
    .bind(&record.effective_from)
    .bind(&record.effective_to)
    .bind(&record.source)
    .bind(&record.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

fn validate_airspace_query_point(longitude: f64, latitude: f64) -> AppResult<AirspaceCoordinate> {
    if !longitude.is_finite()
        || !latitude.is_finite()
        || !(-180.0..=180.0).contains(&longitude)
        || !(-90.0..=90.0).contains(&latitude)
    {
        return Err(AppError::BadRequest(
            "airspace point query requires valid longitude/latitude".to_string(),
        ));
    }

    Ok(AirspaceCoordinate {
        longitude,
        latitude,
    })
}

async fn field_owner_for_farm(
    state: &AppState,
    farm_id: Option<&str>,
    requested_owner: &str,
) -> AppResult<String> {
    if let Some(farm_id) = farm_id {
        let farm = load_farm(state, farm_id)
            .await?
            .ok_or_else(|| AppError::BadRequest(format!("farm {} does not exist", farm_id)))?;
        return Ok(farm.owner);
    }
    let owner = requested_owner.trim();
    Ok(if owner.is_empty() {
        DEFAULT_RECORD_OWNER.to_string()
    } else {
        owner.to_string()
    })
}

async fn load_scene_field(
    state: &AppState,
    scene_row: Option<&sqlx::sqlite::SqliteRow>,
) -> AppResult<Option<FieldRecord>> {
    let Some(field_id) = scene_row.and_then(|row| row.get::<Option<String>, _>("field_id")) else {
        return Ok(None);
    };

    load_field(state, &field_id).await
}

async fn load_annotation(
    state: &AppState,
    scene_id: &str,
    annotation_id: &str,
) -> AppResult<Option<AnnotationRecord>> {
    let row = sqlx::query(
        r#"
        SELECT annotation_id, scene_id, field_id, author, crs, audit_id, label, note, severity, geometry_json, created_at, updated_at
        FROM annotations
        WHERE scene_id = ?1 AND annotation_id = ?2
        "#,
    )
    .bind(scene_id)
    .bind(annotation_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_annotation_record(&row)).transpose()
}

async fn load_recommendation(
    state: &AppState,
    scene_id: &str,
    recommendation_id: &str,
) -> AppResult<Option<RecommendationRecord>> {
    let row = sqlx::query(
        r#"
        SELECT recommendation_id, scene_id, field_id, title, note, category, priority, status, evidence_refs_json, created_at, updated_at
        FROM recommendations
        WHERE scene_id = ?1 AND recommendation_id = ?2
        "#,
    )
    .bind(scene_id)
    .bind(recommendation_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    match row {
        Some(row) => Ok(Some(decode_recommendation_record(state, &row).await?)),
        None => Ok(None),
    }
}

async fn load_report(
    state: &AppState,
    scene_id: &str,
    report_id: &str,
) -> AppResult<Option<ReportRecord>> {
    let row = sqlx::query(
        r#"
        SELECT report_id, scene_id, field_id, title, format, path, visibility, annotation_count, recommendation_count, created_at
        FROM reports
        WHERE scene_id = ?1 AND report_id = ?2
        "#,
    )
    .bind(scene_id)
    .bind(report_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_report_record(&row)).transpose()
}

async fn build_report_lineage_records(
    state: &AppState,
    report: &ReportRecord,
) -> AppResult<Vec<LineageRecord>> {
    let mut records = load_all_provenance_lineage_records(state).await?;
    let mut seen = records
        .iter()
        .map(|record| record.artifact_id.clone())
        .collect::<BTreeSet<_>>();

    push_lineage_record_if_absent(
        &mut records,
        &mut seen,
        LineageRecord {
            artifact_id: scene_artifact_ref(&report.scene_id),
            kind: ArtifactKind::Scene,
            inputs: Vec::new(),
            method: "10.scene_registry".to_string(),
            parameters: ProvenanceParameters::from_json(serde_json::json!({
                "scene_id": &report.scene_id,
                "field_id": &report.field_id,
            })),
            operator: report.generated_by.clone(),
            actor: ActorIdentity::system("geo_hub"),
            created_at: report.created_at.clone(),
        },
    );

    let annotations = load_scene_annotation_records(state, &report.scene_id).await?;
    for annotation in &annotations {
        push_lineage_record_if_absent(
            &mut records,
            &mut seen,
            LineageRecord {
                artifact_id: annotation_artifact_ref(&annotation.annotation_id),
                kind: ArtifactKind::Annotation,
                inputs: vec![scene_artifact_ref(&annotation.scene_id)],
                method: "10.annotation_persistence".to_string(),
                parameters: ProvenanceParameters::from_json(serde_json::json!({
                    "field_id": &annotation.field_id,
                    "label": &annotation.label,
                    "severity": &annotation.severity,
                    "crs": &annotation.crs,
                    "audit_id": &annotation.audit_id,
                })),
                operator: annotation
                    .author
                    .clone()
                    .unwrap_or_else(|| report.generated_by.clone()),
                actor: ActorIdentity::system("geo_hub"),
                created_at: annotation.created_at.clone(),
            },
        );
    }

    let recommendations = load_scene_recommendation_records(state, &report.scene_id).await?;
    for recommendation in &recommendations {
        let annotation_inputs =
            load_recommendation_annotation_ids(state, &recommendation.recommendation_id)
                .await?
                .into_iter()
                .map(|annotation_id| annotation_artifact_ref(&annotation_id));
        let inputs = unique_lineage_inputs(
            annotation_inputs
                .chain(recommendation.evidence_refs.iter().cloned())
                .collect::<Vec<_>>(),
        );
        push_lineage_record_if_absent(
            &mut records,
            &mut seen,
            LineageRecord {
                artifact_id: recommendation_artifact_ref(&recommendation.recommendation_id),
                kind: ArtifactKind::Recommendation,
                inputs,
                method: "10.recommendation_lifecycle".to_string(),
                parameters: ProvenanceParameters::from_json(serde_json::json!({
                    "field_id": &recommendation.field_id,
                    "title": &recommendation.title,
                    "category": &recommendation.category,
                    "priority": recommendation.priority,
                    "status": recommendation.status,
                })),
                operator: recommendation.author_user_id.clone(),
                actor: ActorIdentity::system("geo_hub"),
                created_at: recommendation.created_at.clone(),
            },
        );
    }

    let report_inputs = unique_lineage_inputs(
        std::iter::once(scene_artifact_ref(&report.scene_id))
            .chain(
                annotations
                    .iter()
                    .map(|annotation| annotation_artifact_ref(&annotation.annotation_id)),
            )
            .chain(recommendations.iter().map(|recommendation| {
                recommendation_artifact_ref(&recommendation.recommendation_id)
            }))
            .chain(report.source_refs.iter().cloned())
            .collect::<Vec<_>>(),
    );
    push_lineage_record_if_absent(
        &mut records,
        &mut seen,
        LineageRecord {
            artifact_id: report_artifact_ref(&report.report_id),
            kind: ArtifactKind::Report,
            inputs: report_inputs,
            method: "10.report_deliverable".to_string(),
            parameters: ProvenanceParameters::from_json(serde_json::json!({
                "scene_id": &report.scene_id,
                "field_id": &report.field_id,
                "season_id": &report.season_id,
                "title": &report.title,
                "artifact_uri": &report.artifact_uri,
                "annotation_count": report.annotation_count,
                "recommendation_count": report.recommendation_count,
            })),
            operator: report.generated_by.clone(),
            actor: ActorIdentity::system("geo_hub"),
            created_at: report.created_at.clone(),
        },
    );

    Ok(records)
}

async fn load_all_provenance_lineage_records(state: &AppState) -> AppResult<Vec<LineageRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT artifact_id, kind, inputs_json, method, parameters_json, operator, actor_id,
               actor_kind, created_at
        FROM provenance_lineage_records
        ORDER BY created_at ASC, artifact_id ASC
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_lineage_record(&row))
        .collect()
}

fn push_lineage_record_if_absent(
    records: &mut Vec<LineageRecord>,
    seen: &mut BTreeSet<String>,
    record: LineageRecord,
) {
    if seen.insert(record.artifact_id.clone()) {
        records.push(record);
    }
}

fn unique_lineage_inputs(inputs: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    inputs
        .into_iter()
        .filter_map(|input| {
            let input = input.trim();
            (!input.is_empty()).then(|| input.to_string())
        })
        .filter(|input| seen.insert(input.clone()))
        .collect()
}

fn scene_artifact_ref(scene_id: &str) -> String {
    format!("scene:{scene_id}")
}

fn annotation_artifact_ref(annotation_id: &str) -> String {
    format!("annotation:{annotation_id}")
}

fn recommendation_artifact_ref(recommendation_id: &str) -> String {
    format!("recommendation:{recommendation_id}")
}

fn report_artifact_ref(report_id: &str) -> String {
    format!("report:{report_id}")
}

async fn load_report_share(
    state: &AppState,
    share_token: &str,
) -> AppResult<Option<ReportShareRecord>> {
    let row = sqlx::query(
        r#"
        SELECT share_token,
               report_id AS share_report_id,
               scene_id AS share_scene_id,
               expires_at AS share_expires_at,
               revoked_at AS share_revoked_at,
               created_at AS share_created_at
        FROM report_shares
        WHERE share_token = ?1
        "#,
    )
    .bind(share_token)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(row.map(|row| decode_report_share_record(&row)))
}

async fn load_report_share_with_report(
    state: &AppState,
    share_token: &str,
) -> AppResult<Option<SharedReportRecord>> {
    let row = sqlx::query(
        r#"
        SELECT s.share_token,
               s.report_id AS share_report_id,
               s.scene_id AS share_scene_id,
               s.expires_at AS share_expires_at,
               s.revoked_at AS share_revoked_at,
               s.created_at AS share_created_at,
               r.report_id,
               r.scene_id,
               r.field_id,
               r.title,
               r.format,
               r.path,
               r.visibility,
               r.annotation_count,
               r.recommendation_count,
               r.created_at
        FROM report_shares s
        JOIN reports r ON r.report_id = s.report_id AND r.scene_id = s.scene_id
        WHERE s.share_token = ?1
        "#,
    )
    .bind(share_token)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_shared_report_record(&row)).transpose()
}

async fn audit_report_share_event(
    state: &AppState,
    share: &ReportShareRecord,
    event_type: &str,
    details: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO report_share_events (share_token, report_id, scene_id, event_type, created_at, details)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&share.share_token)
    .bind(&share.report_id)
    .bind(&share.scene_id)
    .bind(event_type)
    .bind(current_record_timestamp())
    .bind(details)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_recommendation_annotation_ids(
    state: &AppState,
    recommendation_id: &str,
) -> AppResult<Vec<String>> {
    let rows = sqlx::query(
        r#"
        SELECT annotation_id
        FROM recommendation_annotations
        WHERE recommendation_id = ?1
        ORDER BY annotation_id ASC
        "#,
    )
    .bind(recommendation_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("annotation_id"))
        .collect())
}

async fn load_scene_annotation_records(
    state: &AppState,
    scene_id: &str,
) -> AppResult<Vec<AnnotationRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT annotation_id, scene_id, field_id, author, crs, audit_id, label, note, severity, geometry_json, created_at, updated_at
        FROM annotations
        WHERE scene_id = ?1
        ORDER BY created_at ASC
        "#,
    )
    .bind(scene_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut annotations = Vec::with_capacity(rows.len());
    for row in rows {
        annotations.push(decode_annotation_record(&row)?);
    }

    Ok(annotations)
}

async fn load_scene_recommendation_records(
    state: &AppState,
    scene_id: &str,
) -> AppResult<Vec<RecommendationRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT recommendation_id, scene_id, field_id, title, note, category, priority, status, evidence_refs_json, created_at, updated_at
        FROM recommendations
        WHERE scene_id = ?1
        ORDER BY created_at DESC
        "#,
    )
    .bind(scene_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut recommendations = Vec::with_capacity(rows.len());
    for row in rows {
        recommendations.push(decode_recommendation_record(state, &row).await?);
    }

    Ok(recommendations)
}

async fn load_field_annotation_records(
    state: &AppState,
    field_id: &str,
) -> AppResult<Vec<AnnotationRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT annotation_id, scene_id, field_id, author, crs, audit_id, label, note, severity, geometry_json, created_at, updated_at
        FROM annotations
        WHERE field_id = ?1
           OR scene_id IN (SELECT scene_id FROM scenes WHERE field_id = ?1)
        ORDER BY scene_id ASC, created_at ASC, annotation_id ASC
        "#,
    )
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut annotations = Vec::with_capacity(rows.len());
    for row in rows {
        annotations.push(decode_annotation_record(&row)?);
    }

    Ok(annotations)
}

async fn load_field_recommendation_records(
    state: &AppState,
    field_id: &str,
) -> AppResult<Vec<RecommendationRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT recommendation_id, scene_id, field_id, title, note, category, priority, status, evidence_refs_json, created_at, updated_at
        FROM recommendations
        WHERE field_id = ?1
           OR scene_id IN (SELECT scene_id FROM scenes WHERE field_id = ?1)
        ORDER BY scene_id ASC, created_at ASC, recommendation_id ASC
        "#,
    )
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut recommendations = Vec::with_capacity(rows.len());
    for row in rows {
        recommendations.push(decode_recommendation_record(state, &row).await?);
    }

    Ok(recommendations)
}

async fn load_scene_field_id(state: &AppState, scene_id: &str) -> AppResult<Option<String>> {
    Ok(
        sqlx::query("SELECT field_id FROM scenes WHERE scene_id = ?1")
            .bind(scene_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(Error::from)?
            .and_then(|row| row.get::<Option<String>, _>("field_id")),
    )
}

async fn validate_recommendation_annotation_ids(
    state: &AppState,
    scene_id: &str,
    annotation_ids: &[String],
) -> AppResult<()> {
    if annotation_ids.is_empty() {
        return Err(AppError::BadRequest(
            "recommendation requires at least one annotation".to_string(),
        ));
    }

    for annotation_id in annotation_ids {
        let annotation_id = annotation_id.trim();
        if annotation_id.is_empty() {
            return Err(AppError::BadRequest(
                "recommendation annotation links cannot be empty".to_string(),
            ));
        }
        if load_annotation(state, scene_id, annotation_id)
            .await?
            .is_none()
        {
            return Err(AppError::BadRequest(format!(
                "annotation {} does not exist on this scene",
                annotation_id
            )));
        }
    }

    Ok(())
}

async fn persist_recommendation_annotations(
    state: &AppState,
    recommendation_id: &str,
    annotation_ids: &[String],
) -> AppResult<()> {
    sqlx::query("DELETE FROM recommendation_annotations WHERE recommendation_id = ?1")
        .bind(recommendation_id)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;

    for annotation_id in annotation_ids {
        sqlx::query(
            r#"
            INSERT INTO recommendation_annotations (recommendation_id, annotation_id)
            VALUES (?1, ?2)
            "#,
        )
        .bind(recommendation_id)
        .bind(annotation_id)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;
    }

    Ok(())
}

fn recommendation_priority_str(priority: RecommendationPriority) -> &'static str {
    match priority {
        RecommendationPriority::Low => "low",
        RecommendationPriority::Medium => "medium",
        RecommendationPriority::High => "high",
        RecommendationPriority::Critical => "critical",
    }
}

fn recommendation_status_str(status: RecommendationStatus) -> &'static str {
    match status {
        RecommendationStatus::Open => "open",
        RecommendationStatus::Reviewed => "reviewed",
        RecommendationStatus::Completed => "completed",
        RecommendationStatus::Dismissed => "dismissed",
        RecommendationStatus::Closed => "closed",
    }
}

fn parse_recommendation_priority(value: String) -> AppResult<RecommendationPriority> {
    match value.as_str() {
        "low" => Ok(RecommendationPriority::Low),
        "medium" => Ok(RecommendationPriority::Medium),
        "high" => Ok(RecommendationPriority::High),
        "critical" => Ok(RecommendationPriority::Critical),
        _ => Err(AppError::Anyhow(anyhow::anyhow!(
            "invalid recommendation priority {}",
            value
        ))),
    }
}

fn parse_recommendation_status(value: String) -> AppResult<RecommendationStatus> {
    match value.as_str() {
        "open" => Ok(RecommendationStatus::Open),
        "reviewed" => Ok(RecommendationStatus::Reviewed),
        "completed" => Ok(RecommendationStatus::Completed),
        "dismissed" => Ok(RecommendationStatus::Dismissed),
        "closed" => Ok(RecommendationStatus::Closed),
        _ => Err(AppError::Anyhow(anyhow::anyhow!(
            "invalid recommendation status {}",
            value
        ))),
    }
}

fn report_format_str(format: ReportFormat) -> &'static str {
    match format {
        ReportFormat::Html => "html",
    }
}

fn parse_report_format(value: String) -> AppResult<ReportFormat> {
    match value.as_str() {
        "html" => Ok(ReportFormat::Html),
        _ => Err(AppError::Anyhow(anyhow::anyhow!(
            "invalid report format {}",
            value
        ))),
    }
}

fn report_visibility_str(visibility: ReportVisibility) -> &'static str {
    match visibility {
        ReportVisibility::Org => "org",
        ReportVisibility::Shared => "shared",
    }
}

fn parse_report_visibility(value: String) -> AppResult<ReportVisibility> {
    match value.as_str() {
        "org" => Ok(ReportVisibility::Org),
        "shared" => Ok(ReportVisibility::Shared),
        _ => Err(AppError::BadRequest(format!(
            "invalid report visibility {}",
            value
        ))),
    }
}

fn normalize_share_expires_at(value: Option<String>) -> AppResult<String> {
    match normalize_optional_text(value) {
        Some(value) => parse_share_timestamp(&value).map(format_share_timestamp),
        None => Ok(format_share_timestamp(
            chrono::Utc::now() + chrono::Duration::days(7),
        )),
    }
}

fn share_expired(expires_at: &str) -> AppResult<bool> {
    Ok(parse_share_timestamp(expires_at)? <= chrono::Utc::now())
}

fn parse_share_timestamp(value: &str) -> AppResult<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| AppError::BadRequest(format!("invalid report share expiry {}", value)))
}

fn parse_acquired_at(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(ts.with_timezone(&chrono::Utc));
    }

    let date_only = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?;
    let midnight = chrono::NaiveTime::from_hms_opt(0, 0, 0)?;
    let naive = chrono::NaiveDateTime::new(date_only, midnight);
    Some(chrono::DateTime::from_naive_utc_and_offset(
        naive,
        chrono::Utc,
    ))
}

fn is_lower_cloud(
    current_cloud_cover: Option<f64>,
    candidate_cloud_cover: Option<f64>,
) -> (bool, bool) {
    match (current_cloud_cover, candidate_cloud_cover) {
        (Some(current), Some(candidate)) => (candidate < current, false),
        (None, Some(_)) => (true, true),
        _ => (false, false),
    }
}

fn common_scene_extent(left: &SceneExtent, right: &SceneExtent) -> Option<SceneExtent> {
    let min_lon = left.min_lon.max(right.min_lon);
    let min_lat = left.min_lat.max(right.min_lat);
    let max_lon = left.max_lon.min(right.max_lon);
    let max_lat = left.max_lat.min(right.max_lat);
    (min_lon < max_lon && min_lat < max_lat).then_some(SceneExtent {
        min_lon,
        min_lat,
        max_lon,
        max_lat,
    })
}

fn scene_extent_area(extent: &SceneExtent) -> f64 {
    let width = (extent.max_lon - extent.min_lon).max(0.0);
    let height = (extent.max_lat - extent.min_lat).max(0.0);
    width * height
}

fn coarse_scene_change_score(
    baseline_cloud_cover: Option<f64>,
    comparison_cloud_cover: Option<f64>,
) -> f64 {
    match (baseline_cloud_cover, comparison_cloud_cover) {
        (Some(baseline), Some(comparison)) => {
            ((comparison - baseline).abs() / 100.0).clamp(0.0, 1.0)
        }
        _ => 0.0,
    }
}

fn is_scene_spatially_consistent(
    current_asserted_spatial_ref: Option<&RasterSpatialRef>,
    _current_metadata: Option<&MultispectralImage>,
    candidate_asserted_spatial_ref: Option<&RasterSpatialRef>,
    candidate_metadata: Option<&MultispectralImage>,
) -> bool {
    let Some(current_asserted_spatial_ref) = current_asserted_spatial_ref else {
        return false;
    };
    let Some(candidate_asserted_spatial_ref) = candidate_asserted_spatial_ref else {
        return false;
    };

    if assert_scene_spatial_ref_integrity(candidate_metadata, Some(candidate_asserted_spatial_ref))
        .is_err()
    {
        return false;
    }

    assert_spatial_refs_equivalent(current_asserted_spatial_ref, candidate_asserted_spatial_ref)
        .is_ok()
}

fn format_share_timestamp(timestamp: chrono::DateTime<chrono::Utc>) -> String {
    timestamp.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn report_share_response(share: &ReportShareRecord) -> ReportShareResponse {
    ReportShareResponse {
        share_token: share.share_token.clone(),
        report_id: share.report_id.clone(),
        scene_id: share.scene_id.clone(),
        url_path: format!("/api/report-shares/{}", share.share_token),
        expires_at: share.expires_at.clone(),
        revoked_at: share.revoked_at.clone(),
        created_at: share.created_at.clone(),
    }
}

async fn report_file_response(report: &ReportRecord) -> AppResult<Response> {
    let report_path = PathBuf::from(&report.artifact_path);
    let file = File::open(&report_path)
        .await
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => AppError::NotFound,
            _ => AppError::Anyhow(error.into()),
        })?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    if let Some(filename) = report_path.file_name().and_then(|name| name.to_str()) {
        if let Ok(value) = HeaderValue::from_str(&format!("inline; filename=\"{}\"", filename)) {
            headers.insert(header::CONTENT_DISPOSITION, value);
        }
    }

    Ok((headers, body).into_response())
}

fn render_scene_report_html(
    scene_id: &str,
    sensor: Option<String>,
    acquired_at: Option<String>,
    metadata: Option<&MultispectralImage>,
    field: Option<&FieldRecord>,
    geospatial: &SceneGeospatialMetadata,
    annotations: &[AnnotationRecord],
    recommendations: &[RecommendationRecord],
    report_title: &str,
) -> String {
    let field_name = field
        .map(|field| field.name.clone())
        .unwrap_or_else(|| "Unlinked field".to_string());
    let map_svg = render_report_map_svg(field, geospatial, annotations, recommendations);
    let recommendations_html = recommendations
        .iter()
        .map(|recommendation| {
            format!(
                "<li><strong>{}</strong> [{} / {}]{}{} </li>",
                escape_html(&recommendation.title),
                recommendation_status_str(recommendation.status),
                recommendation_priority_str(recommendation.priority),
                recommendation
                    .category
                    .as_ref()
                    .map(|category| format!(" Category: {}.", escape_html(category)))
                    .unwrap_or_default(),
                recommendation
                    .note
                    .as_ref()
                    .map(|note| format!(" {}", escape_html(note)))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let annotations_html = annotations
        .iter()
        .map(|annotation| {
            format!(
                "<li><strong>{}</strong>{}{} </li>",
                escape_html(&annotation.label),
                annotation
                    .severity
                    .as_ref()
                    .map(|severity| format!(" [{}]", escape_html(severity)))
                    .unwrap_or_default(),
                annotation
                    .note
                    .as_ref()
                    .map(|note| format!(" {}", escape_html(note)))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>{title}</title>
  <style>
    body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; margin: 32px; color: #1a1f26; background: #f7f4ee; }}
    h1, h2 {{ margin-bottom: 8px; }}
    .meta {{ display: grid; grid-template-columns: repeat(2, minmax(240px, 1fr)); gap: 12px; margin-bottom: 24px; }}
    .card {{ background: #ffffff; border: 1px solid #d8d0c4; border-radius: 10px; padding: 16px; }}
    .map {{ margin: 24px 0; background: #ffffff; border: 1px solid #d8d0c4; border-radius: 10px; padding: 16px; }}
    ul {{ padding-left: 20px; }}
    .muted {{ color: #5b6572; }}
  </style>
</head>
<body>
  <h1>{title}</h1>
  <p class="muted">Scene {scene_id} • Field {field_name}</p>
  <div class="meta">
    <div class="card"><strong>Sensor</strong><div>{sensor}</div></div>
    <div class="card"><strong>Acquired</strong><div>{acquired_at}</div></div>
    <div class="card"><strong>Raster</strong><div>{width} × {height} px</div></div>
    <div class="card"><strong>Products</strong><div>{bands}</div></div>
    <div class="card"><strong>Annotations</strong><div>{annotation_count}</div></div>
    <div class="card"><strong>Recommendations</strong><div>{recommendation_count}</div></div>
  </div>
  <div class="map">
    <h2>Field Snapshot</h2>
    {map_svg}
  </div>
  <div class="card">
    <h2>Findings</h2>
    <ul>{annotations_html}</ul>
  </div>
  <div class="card" style="margin-top: 16px;">
    <h2>Recommendations</h2>
    <ul>{recommendations_html}</ul>
  </div>
</body>
</html>"#,
        title = escape_html(report_title),
        scene_id = escape_html(scene_id),
        field_name = escape_html(&field_name),
        sensor = escape_html(sensor.as_deref().unwrap_or("unknown")),
        acquired_at = escape_html(acquired_at.as_deref().unwrap_or("n/a")),
        width = metadata
            .map(|image| image.metadata.width)
            .unwrap_or_default(),
        height = metadata
            .map(|image| image.metadata.height)
            .unwrap_or_default(),
        bands = escape_html(
            &metadata
                .map(|image| image.metadata.bands.join(", "))
                .unwrap_or_else(|| "n/a".to_string())
        ),
        annotation_count = annotations.len(),
        recommendation_count = recommendations.len(),
        annotations_html = annotations_html,
        recommendations_html = recommendations_html,
        map_svg = map_svg,
    )
}

fn render_report_map_svg(
    field: Option<&FieldRecord>,
    geospatial: &SceneGeospatialMetadata,
    annotations: &[AnnotationRecord],
    recommendations: &[RecommendationRecord],
) -> String {
    let width = 820.0;
    let height = 360.0;
    let extent = geospatial.extent.clone().or_else(|| {
        field.map(|field| SceneExtent {
            min_lon: field.extent.min_lon,
            min_lat: field.extent.min_lat,
            max_lon: field.extent.max_lon,
            max_lat: field.extent.max_lat,
        })
    });

    let Some(extent) = extent else {
        return "<div class=\"muted\">No geospatial extent available for map preview.</div>"
            .to_string();
    };

    let mut svg = format!(
        "<svg viewBox=\"0 0 {width} {height}\" width=\"100%\" height=\"{height}\" xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"100%\" height=\"100%\" fill=\"#f4efe5\"/>"
    );

    if let Some(field) = field {
        let points = field
            .boundary
            .coordinates
            .iter()
            .map(|point| svg_project(point.longitude, point.latitude, &extent, width, height))
            .map(|(x, y)| format!("{x:.1},{y:.1}"))
            .collect::<Vec<_>>()
            .join(" ");
        svg.push_str(&format!(
            "<polygon points=\"{}\" fill=\"#e4d7b5\" stroke=\"#967433\" stroke-width=\"2\"/>",
            points
        ));
    }

    for annotation in annotations {
        match &annotation.geometry {
            AnnotationGeometry::Point { coordinate } => {
                let (x, y) = svg_project(
                    coordinate.longitude,
                    coordinate.latitude,
                    &extent,
                    width,
                    height,
                );
                svg.push_str(&format!(
                    "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"6\" fill=\"#c64242\" stroke=\"#ffffff\" stroke-width=\"2\"/>"
                ));
            }
            AnnotationGeometry::Polygon { coordinates } => {
                let points = coordinates
                    .iter()
                    .map(|point| {
                        svg_project(point.longitude, point.latitude, &extent, width, height)
                    })
                    .map(|(x, y)| format!("{x:.1},{y:.1}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                svg.push_str(&format!(
                    "<polygon points=\"{}\" fill=\"rgba(198,66,66,0.2)\" stroke=\"#c64242\" stroke-width=\"2\"/>",
                    points
                ));
            }
        }
    }

    for recommendation in recommendations {
        if recommendation.annotation_ids.is_empty() {
            continue;
        }
        svg.push_str(&format!(
            "<text x=\"16\" y=\"{}\" font-size=\"12\" fill=\"#1a1f26\">{} [{} / {}]</text>",
            22 + (recommendations
                .iter()
                .position(
                    |candidate| candidate.recommendation_id == recommendation.recommendation_id
                )
                .unwrap_or(0) as i32
                * 18),
            escape_html(&recommendation.title),
            recommendation_status_str(recommendation.status),
            recommendation_priority_str(recommendation.priority),
        ));
    }

    svg.push_str("</svg>");
    svg
}

fn svg_project(
    longitude: f64,
    latitude: f64,
    extent: &SceneExtent,
    width: f64,
    height: f64,
) -> (f64, f64) {
    let x = if (extent.max_lon - extent.min_lon).abs() <= f64::EPSILON {
        width / 2.0
    } else {
        ((longitude - extent.min_lon) / (extent.max_lon - extent.min_lon)) * width
    };
    let y = if (extent.max_lat - extent.min_lat).abs() <= f64::EPSILON {
        height / 2.0
    } else {
        (1.0 - ((latitude - extent.min_lat) / (extent.max_lat - extent.min_lat))) * height
    };
    (x.clamp(0.0, width), y.clamp(0.0, height))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

async fn scene_exists(state: &AppState, scene_id: &str) -> AppResult<bool> {
    let scene_in_db = sqlx::query("SELECT 1 FROM scenes WHERE scene_id = ?1 LIMIT 1")
        .bind(scene_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(Error::from)?
        .is_some();
    if scene_in_db {
        return Ok(true);
    }

    let scene_dir = state.config.data_root.join("scenes").join(scene_id);
    fs::try_exists(scene_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))
}

async fn load_scene_metadata(
    scene_row: Option<&sqlx::sqlite::SqliteRow>,
    scene_dir: &FsPath,
) -> AppResult<Option<MultispectralImage>> {
    if let Some(row) = scene_row {
        let metadata_json: String = row.get("metadata_json");
        let image = serde_json::from_str::<MultispectralImage>(&metadata_json).map_err(|err| {
            AppError::Anyhow(
                anyhow::Error::new(err)
                    .context("failed to decode scene metadata_json from database"),
            )
        })?;
        return Ok(Some(image));
    }

    let mut entries = match fs::read_dir(scene_dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(AppError::Anyhow(err.into())),
    };

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        let path = entry.path();
        let is_metadata = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "metadata_ingested.json" || name.starts_with("metadata_"))
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
        if !is_metadata {
            continue;
        }

        let metadata_json = fs::read_to_string(&path)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
        let image = serde_json::from_str::<MultispectralImage>(&metadata_json).map_err(|err| {
            AppError::Anyhow(anyhow::Error::new(err).context(format!(
                "failed to decode scene metadata at {}",
                path.display()
            )))
        })?;
        return Ok(Some(image));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{
        build_field_record, build_geospatial_metadata, build_product_summary,
        cached_landsat_scene_id, content_type_for_path, fields_from_geojson, geojson_from_fields,
        is_lower_cloud, is_missing_scene_error, is_png, normalize_field_geometry,
        scene_extent_intersects_bounds, AppError, CreateFieldRequest,
    };
    use crate::landsat;
    use geojson::{Feature, FeatureCollection, GeoJson, Geometry, Value as GeoJsonValue};
    use shared::schemas::{
        validate_field_boundary, FieldBoundary, GeoBounds, GeoPoint, GpsCoords, ImageMetadata,
        MultispectralImage, RasterResolution, RasterSpatialRef,
    };
    use std::collections::BTreeMap;
    use std::path::Path;
    use uuid::Uuid;

    #[test]
    fn content_type_detection_works() {
        assert_eq!(content_type_for_path(Path::new("tile.png")), "image/png");
        assert_eq!(content_type_for_path(Path::new("tile.JPG")), "image/jpeg");
        assert_eq!(content_type_for_path(Path::new("tile.tiff")), "image/tiff");
        assert_eq!(
            content_type_for_path(Path::new("tile.unknown")),
            "application/octet-stream"
        );
    }

    #[test]
    fn png_extension_detection_is_case_insensitive() {
        assert!(is_png(Path::new("x.png")));
        assert!(is_png(Path::new("x.PNG")));
        assert!(!is_png(Path::new("x.jpeg")));
    }

    #[test]
    fn row_not_found_errors_are_detected() {
        let err = anyhow::Error::new(sqlx::Error::RowNotFound);
        assert!(is_missing_scene_error(&err));
    }

    #[test]
    fn product_summary_contains_expected_url_and_filename() {
        let summary = build_product_summary("scene-1", "ndvi", Path::new("/tmp/output.png"));
        assert_eq!(summary.filename, "output.png");
        assert_eq!(summary.content_type, "image/png");
        assert_eq!(summary.url_path, "/api/scenes/scene-1/products/ndvi");
    }

    #[test]
    fn geospatial_metadata_uses_available_center_but_not_fake_extent() {
        let image = MultispectralImage {
            image_id: Uuid::nil(),
            metadata: ImageMetadata {
                timestamp: "2025-01-01T00:00:00Z"
                    .parse()
                    .expect("timestamp should parse"),
                gps_position: Some(GpsCoords {
                    latitude: 40.7128,
                    longitude: -74.0060,
                    altitude: 12.0,
                }),
                bands: vec!["B4".to_string(), "B5".to_string()],
                exposure_time: 1.0,
                gain: 1.0,
                width: 512,
                height: 256,
                spatial_ref: None,
            },
            file_paths: Default::default(),
        };

        let geospatial = build_geospatial_metadata(Some(&image));

        assert!(!geospatial.georeferenced);
        assert_eq!(geospatial.crs, None);
        assert_eq!(
            geospatial.center.as_ref().map(|gps| gps.latitude),
            Some(40.7128)
        );
        assert_eq!(geospatial.extent, None);
    }

    #[test]
    fn geospatial_metadata_defaults_when_no_metadata_exists() {
        let geospatial = build_geospatial_metadata(None);

        assert!(!geospatial.georeferenced);
        assert_eq!(geospatial.crs, None);
        assert!(geospatial.center.is_none());
        assert_eq!(geospatial.extent, None);
    }

    #[test]
    fn geospatial_metadata_prefers_bbox_when_available() {
        let image = MultispectralImage {
            image_id: Uuid::nil(),
            metadata: ImageMetadata {
                timestamp: "2025-01-01T00:00:00Z"
                    .parse()
                    .expect("timestamp should parse"),
                gps_position: Some(GpsCoords {
                    latitude: 1.0,
                    longitude: 2.0,
                    altitude: 3.0,
                }),
                bands: vec!["B4".to_string(), "B5".to_string()],
                exposure_time: 1.0,
                gain: 1.0,
                width: 512,
                height: 256,
                spatial_ref: Some(RasterSpatialRef {
                    georeferenced: true,
                    crs: Some("EPSG:4326".to_string()),
                    bbox: Some(GeoBounds {
                        min_lon: -74.1,
                        min_lat: 40.6,
                        max_lon: -73.9,
                        max_lat: 40.8,
                    }),
                    geo_transform: Some([-74.1, 0.000390625, 0.0, 40.8, 0.0, -0.00078125]),
                    resolution: Some(RasterResolution {
                        x: 0.000390625,
                        y: 0.00078125,
                    }),
                }),
            },
            file_paths: Default::default(),
        };

        let geospatial = build_geospatial_metadata(Some(&image));

        assert!(geospatial.georeferenced);
        assert_eq!(geospatial.crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(
            geospatial.center.as_ref().map(|gps| gps.latitude),
            Some(40.7)
        );
        assert_eq!(
            geospatial.center.as_ref().map(|gps| gps.longitude),
            Some(-74.0)
        );
        assert_eq!(
            geospatial.extent,
            Some(super::SceneExtent {
                min_lon: -74.1,
                min_lat: 40.6,
                max_lon: -73.9,
                max_lat: 40.8,
            })
        );
    }

    #[test]
    fn scene_extent_intersection_detects_overlap_and_gap() {
        let field_bounds = GeoBounds {
            min_lon: -96.7,
            min_lat: 41.1,
            max_lon: -96.2,
            max_lat: 41.4,
        };

        assert!(scene_extent_intersects_bounds(
            &super::SceneExtent {
                min_lon: -96.8,
                min_lat: 41.0,
                max_lon: -96.1,
                max_lat: 41.5,
            },
            &field_bounds,
        ));
        assert!(!scene_extent_intersects_bounds(
            &super::SceneExtent {
                min_lon: -90.8,
                min_lat: 35.0,
                max_lon: -90.1,
                max_lat: 35.5,
            },
            &field_bounds,
        ));
    }

    #[test]
    fn is_lower_cloud_flags_fresher_and_reduces_uncertainty_only_when_comparable() {
        assert_eq!(is_lower_cloud(Some(50.0), Some(25.0)), (true, false));
        assert_eq!(is_lower_cloud(Some(25.0), Some(50.0)), (false, false));
        assert_eq!(is_lower_cloud(None, Some(32.0)), (true, true));
        assert_eq!(is_lower_cloud(Some(25.0), None), (false, false));
        assert_eq!(is_lower_cloud(None, None), (false, false));
    }

    #[test]
    fn build_field_record_computes_extent_from_boundary() {
        let field = build_field_record(CreateFieldRequest {
            farm_id: None,
            field_id: Some("north-80".to_string()),
            org_id: None,
            owner: None,
            name: "North 80".to_string(),
            crop: Some("corn".to_string()),
            season: Some("2026".to_string()),
            notes: Some("test field".to_string()),
            status: None,
            boundary: FieldBoundary {
                crs: Some("EPSG:4326".to_string()),
                coordinates: vec![
                    GeoPoint {
                        longitude: -96.7,
                        latitude: 41.1,
                    },
                    GeoPoint {
                        longitude: -96.2,
                        latitude: 41.1,
                    },
                    GeoPoint {
                        longitude: -96.2,
                        latitude: 41.4,
                    },
                ],
            },
        })
        .expect("field should build");

        assert_eq!(field.field_id, "north-80");
        assert_eq!(field.name, "North 80");
        assert_eq!(
            field.extent,
            GeoBounds {
                min_lon: -96.7,
                min_lat: 41.1,
                max_lon: -96.2,
                max_lat: 41.4,
            }
        );
    }

    #[test]
    fn build_field_record_rejects_short_boundary() {
        let err = build_field_record(CreateFieldRequest {
            farm_id: None,
            field_id: None,
            org_id: None,
            owner: None,
            name: "Short boundary".to_string(),
            crop: None,
            season: None,
            notes: None,
            status: None,
            boundary: FieldBoundary {
                crs: None,
                coordinates: vec![
                    GeoPoint {
                        longitude: -96.7,
                        latitude: 41.1,
                    },
                    GeoPoint {
                        longitude: -96.2,
                        latitude: 41.1,
                    },
                ],
            },
        })
        .expect_err("boundary should be rejected");

        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn build_field_record_rejects_invalid_coordinate_ranges() {
        let err = build_field_record(CreateFieldRequest {
            farm_id: None,
            field_id: None,
            org_id: None,
            owner: None,
            name: "Bad coordinates".to_string(),
            crop: None,
            season: None,
            notes: None,
            status: None,
            boundary: FieldBoundary {
                crs: None,
                coordinates: vec![
                    GeoPoint {
                        longitude: -96.7,
                        latitude: 41.1,
                    },
                    GeoPoint {
                        longitude: 200.0,
                        latitude: 41.1,
                    },
                    GeoPoint {
                        longitude: -96.2,
                        latitude: 41.4,
                    },
                ],
            },
        })
        .expect_err("invalid coordinates should be rejected");

        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn geojson_import_defaults_crs_and_round_trips_closed_polygon() {
        let geojson = GeoJson::Feature(square_feature(None));

        let fields = fields_from_geojson(geojson).expect("field imports");

        assert_eq!(fields.len(), 1);
        let field = &fields[0];
        assert_eq!(field.boundary.crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(
            field.boundary.coordinates.first(),
            field.boundary.coordinates.last()
        );
        validate_field_boundary(&field.boundary).expect("imported boundary validates");
        let imported_ring_len = field.boundary.coordinates.len();

        let exported = geojson_from_fields(fields);
        let GeoJson::FeatureCollection(FeatureCollection { features, .. }) = exported else {
            panic!("fields export as feature collection");
        };
        let GeoJsonValue::Polygon(rings) = features[0]
            .geometry
            .as_ref()
            .expect("geometry exists")
            .value
            .clone()
        else {
            panic!("field exports as polygon");
        };

        assert_eq!(rings[0].first(), rings[0].last());
        assert_eq!(rings[0].len(), imported_ring_len);
        assert_eq!(
            features[0]
                .properties
                .as_ref()
                .and_then(|properties| properties.get("crs"))
                .and_then(|value| value.as_str()),
            Some("EPSG:4326")
        );
    }

    #[test]
    fn geojson_import_rejects_unsupported_crs() {
        let err = fields_from_geojson(GeoJson::Feature(square_feature(Some("EPSG:3857"))))
            .expect_err("unsupported CRS is rejected");

        assert!(matches!(err, AppError::BadRequest(_)));
        assert!(format!("{err}").contains("unsupported GeoJSON CRS"));
    }

    fn square_feature(crs: Option<&str>) -> Feature {
        let mut properties = serde_json::Map::new();
        properties.insert(
            "field_id".to_string(),
            serde_json::Value::String("geojson-field".to_string()),
        );
        properties.insert(
            "name".to_string(),
            serde_json::Value::String("GeoJSON Field".to_string()),
        );
        if let Some(crs) = crs {
            properties.insert(
                "crs".to_string(),
                serde_json::Value::String(crs.to_string()),
            );
        }

        Feature {
            bbox: None,
            geometry: Some(Geometry::new(GeoJsonValue::Polygon(vec![vec![
                vec![-96.5, 41.2],
                vec![-96.2, 41.2],
                vec![-96.2, 41.4],
                vec![-96.5, 41.4],
                vec![-96.5, 41.2],
            ]]))),
            id: None,
            properties: Some(properties),
            foreign_members: None,
        }
    }

    #[test]
    fn normalize_field_geometry_accepts_polygon_feature() {
        let feature = serde_json::json!({
            "type": "Feature",
            "properties": {},
            "geometry": {
                "type": "Polygon",
                "coordinates": [[
                    [-119.45, 36.74],
                    [-119.38, 36.74],
                    [-119.38, 36.81],
                    [-119.45, 36.74]
                ]]
            }
        });

        let geometry = normalize_field_geometry(Some(&feature))
            .expect("field geometry should be accepted")
            .expect("geometry should be returned");

        assert_eq!(
            geometry.get("type").and_then(|value| value.as_str()),
            Some("Polygon")
        );
    }

    #[test]
    fn normalize_field_geometry_rejects_points() {
        let point = serde_json::json!({
            "type": "Point",
            "coordinates": [-119.45, 36.74]
        });

        let err = normalize_field_geometry(Some(&point))
            .expect_err("point geometry should not be accepted as a field");

        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn cached_landsat_scene_id_is_stable_and_filesystem_safe() {
        let candidate = landsat::LandsatSceneCandidate {
            dataset: "landsat".to_string(),
            dataset_label: "Landsat 8/9 Collection 2".to_string(),
            provider: "Microsoft Planetary Computer".to_string(),
            collection: "landsat-c2-l2".to_string(),
            item_id: "LC09_L2SP_042034_20260601_02_T1".to_string(),
            acquired_at: "2026-06-01T18:32:58Z".to_string(),
            cloud_cover: Some(3.85),
            bbox: None,
            resolution_m: 30.0,
            asset_count: 7,
            assets: BTreeMap::new(),
        };

        let scene_id = cached_landsat_scene_id(&candidate, 36.7783, -119.4179);

        assert_eq!(
            scene_id,
            "landsat_lc09_l2sp_042034_20260601_02_t1_36_77830__119_41790"
        );
    }
}
