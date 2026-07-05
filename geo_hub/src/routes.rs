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
    build_airspace_zone_record, build_compliance_audit_report, build_compliance_authority_export,
    build_compliance_authority_share, build_compliance_regulation_assist,
    build_initial_compliance_record, refuse_in_place_mutation, revoke_compliance_authority_share,
    AirspaceCoordinate, AirspaceZoneClass, AirspaceZoneError, AirspaceZoneIngestRequest,
    AirspaceZoneRecord, AppendComplianceRecordVersionRequest, ComplianceAuditReport,
    ComplianceAuditReportError, ComplianceAuditReportRequest, ComplianceAuthorityExportArtifact,
    ComplianceAuthorityExportError, ComplianceAuthorityExportRequest, ComplianceAuthorityFormat,
    ComplianceAuthorityShareArtifact, ComplianceAuthorityShareError,
    ComplianceAuthorityShareRequest, ComplianceRecord, ComplianceRecordError,
    ComplianceRecordPayload, ComplianceRecordType, ComplianceRegulationAssistError,
    ComplianceRegulationAssistIntent, ComplianceRegulationAssistOutput,
    ComplianceRegulationAssistRequest, ComplianceRetentionClass, ComplianceRuleCitation,
    CreateComplianceRecordRequest,
};
use copilot::{
    create_copilot_turn, start_copilot_conversation, CopilotConversationError,
    CopilotConversationRecord, CopilotConversationStartRequest, CopilotTurnCreateRequest,
    CopilotTurnRecord, CopilotTurnRole,
};
use crop_intelligence::{
    apply_detection_verification, assemble_detection_finding, build_crop_closed_loop_proposal,
    build_inference_run_progress_record, build_inference_run_record, build_model_version_record,
    detect_inference_run_stall, inference_run_progress_stream, transition_inference_run_status,
    validate_detection_finding_promotion, validate_model_reference, CropClosedLoopAction,
    CropClosedLoopApprovalStatus, CropClosedLoopFindingEvidence, CropClosedLoopProposal,
    CropClosedLoopProposalError, CropClosedLoopProposalRequest, CropDetectionCorrectionLabel,
    CropDetectionFindingError, CropDetectionFindingRecord, CropDetectionFindingRequest,
    CropDetectionVerificationAction, CropDetectionVerificationError,
    CropDetectionVerificationRecord, CropDetectionVerificationRequest, CropModelRegistryError,
    CropModelTask, DetectionVerificationState, DetectionZoneGeometry, FindingPromotionDecision,
    FindingPromotionError, FindingPromotionRequest, InferenceModelReference, InferenceRunError,
    InferenceRunProgressInput, InferenceRunProgressRecord, InferenceRunProgressStream,
    InferenceRunRecord, InferenceRunStallEvent, InferenceRunStatus, InferenceRunSubmissionRequest,
    ModelGateResponse, ModelVersionRecord, ModelVersionRegistrationRequest,
};
use fleet_health::{
    accrue_component_duty, apply_rollout_control, build_component_duty_accruals,
    build_component_record, component_event, derive_health_indicators, evaluate_ota_rollout,
    install_component, integrate_ground_vehicle_health, ComponentDutyAccrualRecord,
    DutyAccrualRequest, FleetComponentEventRecord, FleetComponentRecord, FleetComponentType,
    FleetHealthError, FleetHealthIndicator, FleetHealthIndicatorDerivation,
    FleetHealthIndicatorSample, GroundVehicleHealthIngestRequest, GroundVehicleHealthIntegration,
    HealthIndicatorFreshness, HealthTelemetryGap, InstallComponentRequest, OtaRolloutDecision,
    OtaRolloutRequest, RegisterComponentRequest, RolloutControlDecision, RolloutControlRequest,
    ServiceHistoryEntry, TelemetryHealthIndicatorRequest,
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
    aggregate_content_engagement, aggregate_marketplace_ratings, append_content_version,
    apply_collaboration_mission_edit, apply_content_taxonomy_tags, assemble_marketplace_org_report,
    assert_raster_spatial_ref, authorize_collaboration_action, bind_fleet_node_identity,
    bounds_coverage_fraction, bounds_from_points, build_collaboration_channel,
    build_collaboration_message, build_collaboration_notifications,
    build_collaboration_operator_console_feed, build_content_portal_embed,
    build_marketplace_account_record, build_marketplace_catalog_item_record,
    build_marketplace_inventory_record, build_marketplace_portal_entry,
    build_soil_moisture_reading, build_sustainability_certification_evidence_pack,
    build_sustainability_record, build_tractor_record, close_marketplace_listing_record,
    compare_sustainability_baseline, compute_biodiversity_proxy, compute_carbon_footprint,
    compute_drought_index, compute_marketplace_demand_forecast, compute_soil_carbon_proxy,
    compute_sustainability_kpi, content_portal_embed_item, create_collaboration_mission_plan,
    create_community_contribution, create_content_engagement_event, create_content_locale_variant,
    create_marketplace_fulfillment_record, create_marketplace_rating_record,
    create_success_story_content, create_sustainability_baseline, create_sustainability_mrv_trail,
    create_versioned_content, estimate_biomass, evaluate_collaboration_mission_dispatch,
    expire_lapsed_collaboration_presence, fulfill_marketplace_inventory,
    link_collaboration_session_annotation, moderate_community_contribution,
    normalize_weather_provider_forecast, parse_biodiversity_proxy_status,
    parse_carbon_footprint_status, parse_content_contribution_status,
    parse_content_engagement_event_type, parse_content_status, parse_content_type,
    parse_drought_index_type, parse_marketplace_account_status, parse_marketplace_catalog_category,
    parse_marketplace_catalog_item_kind, parse_marketplace_demand_forecast_status,
    parse_marketplace_fulfillment_status, parse_marketplace_listing_status,
    parse_marketplace_order_status, parse_marketplace_party_type,
    parse_marketplace_unit_of_measure, parse_soil_carbon_proxy_status, parse_soil_moisture_qa_flag,
    parse_soil_moisture_rejection_reason, parse_sustainability_comparison_status,
    parse_sustainability_kpi_direction, parse_sustainability_kpi_status,
    parse_sustainability_metric_type, parse_sustainability_mrv_output_kind,
    parse_sustainability_trend, place_marketplace_order_record, prepare_open_data_publication,
    publish_marketplace_listing_record, raise_collaboration_emergency_alert,
    record_collaboration_session, relay_collaboration_stream_frame, release_marketplace_inventory,
    reserve_marketplace_inventory, resolve_content_permissions, resolve_localized_content,
    search_published_content, soil_moisture_rejection_reason_for_error,
    soil_moisture_rejection_record, start_collaboration_stream,
    transition_collaboration_emergency_alert, transition_content_workflow,
    transition_marketplace_account_status, transition_marketplace_fulfillment_status,
    transition_marketplace_order_status, update_collaboration_presence, validate_field_boundary,
    weather_fetch_failure_record, AnnotationGeometry, AnnotationRecord, BiodiversityProxyError,
    BiodiversityProxyRequest, BiodiversityProxyResult, BiodiversityProxyStatus,
    BiomassEstimateError, BiomassEstimateRequest, BiomassEstimateResult, CarbonEmissionFactor,
    CarbonFootprintComputeRequest, CarbonFootprintError, CarbonFootprintInput,
    CarbonFootprintResult, CarbonFootprintStatus, CollaborationAction,
    CollaborationActionAuthorizeRequest, CollaborationChannelCreateRequest,
    CollaborationChannelRecord, CollaborationChannelThread, CollaborationEmergencyAlertAuditRecord,
    CollaborationEmergencyAlertCreateRequest, CollaborationEmergencyAlertRaiseResult,
    CollaborationEmergencyAlertRecord, CollaborationEmergencyAlertSource,
    CollaborationEmergencyAlertState, CollaborationEmergencyAlertTransitionRequest,
    CollaborationEmergencyAlertTransitionResult, CollaborationError, CollaborationLiveStreamRecord,
    CollaborationMessageCreateRequest, CollaborationMessageRecord,
    CollaborationMissionDispatchAuditRecord, CollaborationMissionDispatchRequest,
    CollaborationMissionDispatchResult, CollaborationMissionEditAuditRecord,
    CollaborationMissionEditDecision, CollaborationMissionEditResult,
    CollaborationMissionPlanCreateRequest, CollaborationMissionPlanRecord,
    CollaborationMissionWaypoint, CollaborationMissionWaypointEditRequest,
    CollaborationNotificationEventRequest, CollaborationNotificationRecord,
    CollaborationOperatorConsoleFeed, CollaborationPermissionDecision,
    CollaborationPermissionResolveRequest, CollaborationPermissionSet, CollaborationPortalFeed,
    CollaborationPresenceRecord, CollaborationPresenceState, CollaborationPresenceUpdateRequest,
    CollaborationSessionAnnotationLinkRecord, CollaborationSessionAnnotationLinkRequest,
    CollaborationSessionAnnotationRecord, CollaborationSessionEventKind,
    CollaborationSessionEventRecord, CollaborationSessionRecord, CollaborationSessionRecordRequest,
    CollaborationSessionReplay, CollaborationStreamFrameRecord,
    CollaborationStreamFrameRelayRequest, CollaborationStreamRelayResult,
    CollaborationStreamStartRequest, CollaborationStreamState,
    ContentCommunityContributionCreateRequest, ContentCommunityContributionRecord,
    ContentContributionModerationAuditRecord, ContentContributionModerationRequest,
    ContentContributionModerationResult, ContentCreateRequest, ContentEditRequest,
    ContentEngagementEventCreateRequest, ContentEngagementEventRecord, ContentEngagementSummary,
    ContentError, ContentLocaleVariantCreateRequest, ContentLocaleVariantRecord,
    ContentLocalizedRecord, ContentPermissionResolveRequest, ContentPermissionSet,
    ContentPortalEmbed, ContentPortalEmbedItem, ContentPortalEmbedRequest, ContentRecord,
    ContentSearchDocument, ContentSearchRequest, ContentSearchResult, ContentStatus,
    ContentSuccessStoryCreateRequest, ContentSuccessStoryRecord, ContentTagApplyRequest,
    ContentTagRecord, ContentTaxonomyKind, ContentType, ContentVersionRecord,
    ContentWorkflowAction, ContentWorkflowAuditRecord, ContentWorkflowTransitionRequest,
    ContentWorkflowTransitionResult, DroughtIndexComputeRequest, DroughtIndexError,
    DroughtIndexPeriod, DroughtIndexRecord, DroughtIndexType, FarmFieldEntityStatus,
    FarmFieldListPage, FarmFieldListQuery, FarmRecord, FieldBoundary, FieldBoundaryRecord,
    FieldRecord, FleetNodeEnrollmentError, FleetNodeEnrollmentRequest, FleetNodeKind,
    FleetNodeRecord, FleetNodeRuntimeMode, FleetNodeStatus, GeoBounds, GeoPoint, GpsCoords,
    ImageMetadata, MarketplaceAccountCreateRequest, MarketplaceAccountError,
    MarketplaceAccountRecord, MarketplaceAccountStatus, MarketplaceCatalogCategory,
    MarketplaceCatalogError, MarketplaceCatalogItemCreateRequest, MarketplaceCatalogItemKind,
    MarketplaceCatalogItemRecord, MarketplaceDemandForecastError, MarketplaceDemandForecastRecord,
    MarketplaceDemandForecastRequest, MarketplaceDemandUncertaintyBand,
    MarketplaceFulfillmentAuditRecord, MarketplaceFulfillmentCreateRequest,
    MarketplaceFulfillmentError, MarketplaceFulfillmentRecord, MarketplaceFulfillmentStatus,
    MarketplaceInventoryError, MarketplaceInventoryRecord, MarketplaceInventoryUpsertRequest,
    MarketplaceListingError, MarketplaceListingPublishRequest, MarketplaceListingRecord,
    MarketplaceListingStatus, MarketplaceOrderAuditRecord, MarketplaceOrderCreateRequest,
    MarketplaceOrderError, MarketplaceOrderRecord, MarketplaceOrderStatus, MarketplaceOrgReport,
    MarketplaceOrgReportRequest, MarketplacePartyType, MarketplacePortalEntry,
    MarketplacePortalEntryError, MarketplaceRatingAggregate, MarketplaceRatingCreateRequest,
    MarketplaceRatingError, MarketplaceRatingRecord, MarketplaceReportError,
    MarketplaceReportPeriod, MultispectralImage, OpenDataPublication, OpenDataPublishError,
    OpenDataPublishRequest, RasterResolution, RasterSpatialRef, RecommendationPriority,
    RecommendationRecord, RecommendationStatus, ReportFormat, ReportRecord, ReportVisibility,
    SoilCarbonProxyError, SoilCarbonProxyRequest, SoilCarbonProxyResult, SoilCarbonProxyStatus,
    SoilCarbonUncertaintyBand, SoilMoistureReadingError, SoilMoistureReadingRecord,
    SoilMoistureReadingRequest, SoilMoistureRejectionReason, SoilMoistureRejectionRecord,
    SustainabilityBaselineCreateRequest, SustainabilityBaselineError, SustainabilityBaselineRecord,
    SustainabilityCertificationEvidencePack, SustainabilityCertificationEvidencePackError,
    SustainabilityCertificationEvidencePackRequest, SustainabilityCertificationOutputItem,
    SustainabilityComparisonRequest, SustainabilityComparisonResult,
    SustainabilityComparisonStatus, SustainabilityExportItem, SustainabilityFieldExportSummary,
    SustainabilityKpiError, SustainabilityKpiStatus, SustainabilityKpiTrackingRequest,
    SustainabilityKpiTrackingResult, SustainabilityMetricType, SustainabilityMrvOutputKind,
    SustainabilityMrvTrail, SustainabilityMrvTrailCreateRequest, SustainabilityMrvTrailError,
    SustainabilityRecord, SustainabilityRecordCreateRequest, SustainabilityRecordError,
    SustainabilityRecordLinkage, TractorCommandAuditDecision, TractorCommandAuditRecord,
    TractorCommandRejection, TractorCommandRejectionReason, TractorImplementRef,
    TractorLifecycleStatus, TractorMotionCommandRequest, TractorRecord, TractorRegistrationRequest,
    TractorRegistryError, VersionedContentRecord, VersionedSuccessStoryContentRecord,
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

/// Shared HTTP error mapping for application-run failures. Kept in the parent
/// module because both `applications` and `alerts` submodules use it (via
/// `super::application_error`).
fn application_error(err: crate::applications::ApplicationError) -> AppError {
    use crate::applications::ApplicationError;
    match err {
        ApplicationError::InputNotFound(_) | ApplicationError::InputNotL2OrL3 { .. } => {
            AppError::BadRequest(err.to_string())
        }
        other => AppError::Anyhow(Error::new(other)),
    }
}

/// Shared coordinate validation used by both the mobile and weather submodules.
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

// Route groups extracted into cohesive submodules (they reach this module's
// shared helpers and domain `*_error` mappers as descendants via `use super::*`).
// Glob `pub use` re-exports keep `routes::<handler>` resolving for server.rs.
mod alert_rules;
mod alerts;
mod applications;
mod browse;
mod catalog;
mod collaboration;
mod compliance_routes;
mod content;
mod copilot_routes;
mod crop_intelligence_routes;
mod drought_raster_routes;
mod farms_fields;
mod field_io;
mod fleet;
mod fleet_health_routes;
mod ingestion;
mod marketplace;
mod mobile;
mod orthomosaic_routes;
mod plugins;
mod product_tiles;
mod proposals;
mod provenance_routes;
mod satellite;
mod scenes_layers;
mod soil_iot_routes;
mod stac;
mod sustainability;
mod weather;
mod workspace;
pub use alert_rules::*;
pub use alerts::*;
pub use applications::*;
pub use browse::*;
pub use catalog::*;
pub use collaboration::*;
pub use compliance_routes::*;
pub use content::*;
pub use copilot_routes::*;
pub use crop_intelligence_routes::*;
pub use drought_raster_routes::*;
pub use farms_fields::*;
pub use field_io::*;
pub use fleet::*;
pub use fleet_health_routes::*;
pub use ingestion::*;
pub use marketplace::*;
pub use mobile::*;
pub use orthomosaic_routes::*;
pub use plugins::*;
pub use product_tiles::*;
pub use proposals::*;
pub use provenance_routes::*;
pub use satellite::*;
pub use scenes_layers::*;
pub use soil_iot_routes::*;
pub use stac::*;
pub use sustainability::*;
pub use weather::*;
pub use workspace::*;

const TILE_SIZE: u32 = 256;
const DEFAULT_LAYER_STALE_AFTER_DAYS: i64 = 14;
const MOBILE_APP_HTML: &str = include_str!("mobile_app.html");

fn default_connection_active() -> bool {
    true
}

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
pub struct CollaborationSessionAnnotationCreateRequest {
    pub actor_id: String,
    pub scene_id: String,
    pub stream_id: String,
    #[serde(default = "default_connection_active")]
    pub connection_active: bool,
    pub annotation: CreateAnnotationRequest,
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
pub struct MarketplaceFulfillmentListQuery {
    pub org_id: Option<String>,
    pub order_ref: Option<String>,
    pub status: Option<MarketplaceFulfillmentStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceFulfillmentScopeQuery {
    pub org_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceFulfillmentTransitionRequest {
    pub org_id: String,
    pub actor_id: String,
    pub status: MarketplaceFulfillmentStatus,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceRatingListQuery {
    pub org_id: Option<String>,
    pub order_ref: Option<String>,
    pub ratee_account_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceRatingAggregateQuery {
    pub org_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceDemandForecastListQuery {
    pub org_id: Option<String>,
    pub field_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceDemandForecastScopeQuery {
    pub org_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceReportQuery {
    pub org_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
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
pub struct CarbonFootprintListQuery {
    pub record_id: Option<String>,
    pub operation_id: Option<String>,
    pub status: Option<CarbonFootprintStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CarbonFootprintScopeQuery {
    pub record_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BiomassEstimateListQuery {
    pub record_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BiomassEstimateScopeQuery {
    pub record_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SustainabilityBaselineListQuery {
    pub field_id: Option<String>,
    pub metric_type: Option<SustainabilityMetricType>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SustainabilityComparisonListQuery {
    pub field_id: Option<String>,
    pub status: Option<SustainabilityComparisonStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SustainabilityComparisonScopeQuery {
    pub field_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SustainabilityMrvTrailListQuery {
    pub output_ref: Option<String>,
    pub output_kind: Option<SustainabilityMrvOutputKind>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SustainabilityMrvTrailScopeQuery {
    pub output_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BiodiversityProxyListQuery {
    pub field_id: Option<String>,
    pub status: Option<BiodiversityProxyStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BiodiversityProxyScopeQuery {
    pub field_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoilCarbonProxyListQuery {
    pub field_id: Option<String>,
    pub status: Option<SoilCarbonProxyStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoilCarbonProxyScopeQuery {
    pub field_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SustainabilityKpiListQuery {
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub status: Option<SustainabilityKpiStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SustainabilityKpiScopeQuery {
    pub field_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SustainabilityCertificationPackScopeQuery {
    pub claim_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SustainabilityExportQuery {
    pub season_id: Option<String>,
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
    pub actor_org_id: Option<String>,
    pub role_refs: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentPermissionQuery {
    pub org_id: String,
    pub actor_org_id: String,
    #[serde(default)]
    pub role_refs: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentSearchQuery {
    pub org_id: String,
    pub q: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentPortalEmbedQuery {
    pub org_id: String,
    pub actor_org_id: String,
    #[serde(default)]
    pub role_refs: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentTagFilterQuery {
    pub org_id: String,
    pub kind: ContentTaxonomyKind,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentEngagementSummaryQuery {
    pub org_id: String,
    pub period: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentLocalizedQuery {
    pub org_id: String,
    pub locale: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CollaborationChannelListQuery {
    pub org_id: Option<String>,
    pub field_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CollaborationScopeQuery {
    pub org_id: Option<String>,
    pub actor_org_id: Option<String>,
    pub actor_id: Option<String>,
    #[serde(default)]
    pub role_refs: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CollaborationPermissionQuery {
    pub org_id: String,
    pub actor_org_id: String,
    #[serde(default)]
    pub role_refs: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CollaborationPresenceListQuery {
    pub org_id: String,
    #[serde(default)]
    pub stale_before: Option<String>,
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
pub struct CropInferenceStallCheckRequest {
    pub detected_at: String,
    pub stall_window_seconds: u64,
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
pub struct ComplianceAuthorityExportApiRequest {
    pub authority_format: ComplianceAuthorityFormat,
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
    #[serde(default)]
    pub residency_tag: String,
    #[serde(default)]
    pub storage_region: String,
    pub retention_class: ComplianceRetentionClass,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComplianceAuthorityShareApiRequest {
    #[serde(flatten)]
    pub export_request: ComplianceAuthorityExportApiRequest,
    #[serde(default)]
    pub share_id: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub expires_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComplianceAuthorityShareRevokeRequest {
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComplianceRegulationAssistApiRequest {
    pub intent: ComplianceRegulationAssistIntent,
    #[serde(default)]
    pub assist_id: Option<String>,
    #[serde(default)]
    pub org_id: String,
    #[serde(default)]
    pub field_id: String,
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub mandatory_record_types: Vec<ComplianceRecordType>,
    #[serde(default)]
    pub rule_citations: Vec<ComplianceRuleCitation>,
    #[serde(default)]
    pub feature_enabled: bool,
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

fn marketplace_fulfillment_error(error: MarketplaceFulfillmentError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn marketplace_rating_error(error: MarketplaceRatingError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn marketplace_demand_forecast_error(error: MarketplaceDemandForecastError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn marketplace_report_error(error: MarketplaceReportError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn sustainability_record_error(error: SustainabilityRecordError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn carbon_footprint_error(error: CarbonFootprintError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn biomass_estimate_error(error: BiomassEstimateError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn sustainability_baseline_error(error: SustainabilityBaselineError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn sustainability_mrv_trail_error(error: SustainabilityMrvTrailError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn biodiversity_proxy_error(error: BiodiversityProxyError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn soil_carbon_proxy_error(error: SoilCarbonProxyError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn sustainability_kpi_error(error: SustainabilityKpiError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn sustainability_certification_pack_error(
    error: SustainabilityCertificationEvidencePackError,
) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn content_error(error: ContentError) -> AppError {
    if matches!(
        error,
        ContentError::PublishRequiresEditor | ContentError::AccessDenied { .. }
    ) {
        AppError::Forbidden(error.to_string())
    } else {
        AppError::BadRequest(error.to_string())
    }
}

fn collaboration_error(error: CollaborationError) -> AppError {
    if matches!(error, CollaborationError::AccessDenied { .. }) {
        AppError::Forbidden(error.to_string())
    } else {
        AppError::BadRequest(error.to_string())
    }
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

fn crop_closed_loop_proposal_error(error: CropClosedLoopProposalError) -> AppError {
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

fn compliance_authority_export_error(error: ComplianceAuthorityExportError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn compliance_authority_share_error(error: ComplianceAuthorityShareError) -> AppError {
    AppError::BadRequest(error.to_string())
}

fn compliance_regulation_assist_error(error: ComplianceRegulationAssistError) -> AppError {
    if matches!(
        error,
        ComplianceRegulationAssistError::DeterministicGateRequired
            | ComplianceRegulationAssistError::FeatureDisabled
    ) {
        AppError::Forbidden(error.to_string())
    } else {
        AppError::BadRequest(error.to_string())
    }
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
    // Read tolerates whatever the (lenient) create path accepted: `build_field_record`
    // requires only >= 3 in-range coordinates and does not require a CRS or a closed
    // ring. Re-running the strict `validate_field_boundary` here would 500 on those
    // loosely-specified-but-accepted boundaries, so instead compute the extent
    // directly from the stored coordinates, mirroring the create path.
    let extent = bounds_from_points(&boundary.coordinates).unwrap_or(GeoBounds {
        min_lon: 0.0,
        min_lat: 0.0,
        max_lon: 0.0,
        max_lat: 0.0,
    });
    // Compute the area with the same lenient shoelace the strict validator uses,
    // so consumers (e.g. demand-forecast field evidence) still get a real area.
    let area_ha = shared::schemas::polygon_area_hectares(&boundary.coordinates);

    Ok(FieldRecord {
        farm_id: row.get("farm_id"),
        field_id: row.get("field_id"),
        org_id: row.get("owner"),
        owner: row.get("owner"),
        name: row.get("name"),
        area_ha: Some(area_ha),
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

fn decode_marketplace_fulfillment_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<MarketplaceFulfillmentRecord> {
    Ok(MarketplaceFulfillmentRecord {
        fulfillment_id: row.get("fulfillment_id"),
        order_ref: row.get("order_ref"),
        org_id: row.get("org_id"),
        carrier_ref: row.get("carrier_ref"),
        tracking_ref: row.get("tracking_ref"),
        status: parse_marketplace_fulfillment_status(&row.get::<String, _>("status"))
            .map_err(marketplace_fulfillment_error)?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_marketplace_fulfillment_audit_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<MarketplaceFulfillmentAuditRecord> {
    let from_status = row
        .get::<Option<String>, _>("from_status")
        .map(|status| {
            parse_marketplace_fulfillment_status(&status).map_err(marketplace_fulfillment_error)
        })
        .transpose()?;
    Ok(MarketplaceFulfillmentAuditRecord {
        audit_id: row.get("audit_id"),
        fulfillment_id: row.get("fulfillment_id"),
        from_status,
        to_status: parse_marketplace_fulfillment_status(&row.get::<String, _>("to_status"))
            .map_err(marketplace_fulfillment_error)?,
        actor_id: row.get("actor_id"),
        occurred_at: row.get("occurred_at"),
    })
}

fn decode_marketplace_rating_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<MarketplaceRatingRecord> {
    Ok(MarketplaceRatingRecord {
        rating_id: row.get("rating_id"),
        order_ref: row.get("order_ref"),
        rater_account_id: row.get("rater_account_id"),
        ratee_account_id: row.get("ratee_account_id"),
        score: row.get("score"),
        comment: row.get("comment"),
        org_scope: row.get("org_scope"),
        created_at: row.get("created_at"),
    })
}

fn decode_marketplace_demand_forecast_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<MarketplaceDemandForecastRecord> {
    let evidence_refs = serde_json::from_str::<Vec<String>>(
        &row.get::<String, _>("evidence_refs_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(
            Error::new(err).context("failed to decode demand forecast evidence_refs_json"),
        )
    })?;
    let uncertainty_low: Option<f64> = row.get("uncertainty_low");
    let uncertainty_high: Option<f64> = row.get("uncertainty_high");
    let uncertainty_band = uncertainty_low
        .zip(uncertainty_high)
        .map(|(low, high)| MarketplaceDemandUncertaintyBand { low, high });
    Ok(MarketplaceDemandForecastRecord {
        forecast_id: row.get("forecast_id"),
        org_id: row.get("org_id"),
        field_id: row.get("field_id"),
        item_kind: parse_marketplace_catalog_item_kind(&row.get::<String, _>("item_kind"))
            .map_err(marketplace_catalog_error)?,
        horizon: row.get("horizon"),
        value: row.get("value"),
        evidence_refs,
        status: parse_marketplace_demand_forecast_status(&row.get::<String, _>("status"))
            .map_err(marketplace_demand_forecast_error)?,
        uncertainty_band,
        method: row.get("method"),
        created_at: row.get("created_at"),
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

fn decode_carbon_footprint_result(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<CarbonFootprintResult> {
    let inputs =
        serde_json::from_str::<Vec<CarbonFootprintInput>>(&row.get::<String, _>("inputs_json"))
            .map_err(|err| {
                AppError::Anyhow(
                    Error::new(err).context("failed to decode carbon footprint inputs_json"),
                )
            })?;
    let factors =
        serde_json::from_str::<Vec<CarbonEmissionFactor>>(&row.get::<String, _>("factors_json"))
            .map_err(|err| {
                AppError::Anyhow(
                    Error::new(err).context("failed to decode carbon footprint factors_json"),
                )
            })?;
    let evidence_refs = serde_json::from_str::<Vec<String>>(
        &row.get::<String, _>("evidence_refs_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(
            Error::new(err).context("failed to decode carbon footprint evidence_refs_json"),
        )
    })?;

    Ok(CarbonFootprintResult {
        footprint_id: row.get("footprint_id"),
        record_id: row.get("record_id"),
        operation_id: row.get("operation_id"),
        value_co2e: row.get("value_co2e"),
        inputs,
        factor_set_version: row.get("factor_set_version"),
        factors,
        evidence_refs,
        status: parse_carbon_footprint_status(&row.get::<String, _>("status"))
            .map_err(carbon_footprint_error)?,
        result_hash: row.get("result_hash"),
        computed_at: row.get("computed_at"),
    })
}

fn decode_biomass_estimate_result(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<BiomassEstimateResult> {
    let extent =
        serde_json::from_str::<GeoBounds>(&row.get::<String, _>("extent_json")).map_err(|err| {
            AppError::Anyhow(Error::new(err).context("failed to decode biomass extent_json"))
        })?;
    let resolution =
        serde_json::from_str::<RasterResolution>(&row.get::<String, _>("resolution_json"))
            .map_err(|err| {
                AppError::Anyhow(
                    Error::new(err).context("failed to decode biomass resolution_json"),
                )
            })?;
    let source_layer_refs =
        serde_json::from_str::<Vec<String>>(&row.get::<String, _>("source_layer_refs_json"))
            .map_err(|err| {
                AppError::Anyhow(
                    Error::new(err).context("failed to decode biomass source_layer_refs_json"),
                )
            })?;

    Ok(BiomassEstimateResult {
        estimate_id: row.get("estimate_id"),
        record_id: row.get("record_id"),
        biomass_value: row.get("biomass_value"),
        area: row.get("area"),
        crs: row.get("crs"),
        extent,
        resolution,
        source_layer_refs,
        method_version: row.get("method_version"),
        result_hash: row.get("result_hash"),
        computed_at: row.get("computed_at"),
    })
}

fn decode_sustainability_baseline(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<SustainabilityBaselineRecord> {
    let evidence_refs = serde_json::from_str::<Vec<String>>(
        &row.get::<String, _>("evidence_refs_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(
            Error::new(err).context("failed to decode sustainability baseline evidence_refs_json"),
        )
    })?;

    Ok(SustainabilityBaselineRecord {
        baseline_id: row.get("baseline_id"),
        field_id: row.get("field_id"),
        season_id: row.get("season_id"),
        metric_type: parse_sustainability_metric_type(&row.get::<String, _>("metric_type"))
            .map_err(sustainability_record_error)?,
        metric_value: row.get("metric_value"),
        source_record_id: row.get("source_record_id"),
        method_version: row.get("method_version"),
        evidence_refs,
        created_at: row.get("created_at"),
    })
}

fn decode_sustainability_comparison(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<SustainabilityComparisonResult> {
    let evidence_refs = serde_json::from_str::<Vec<String>>(
        &row.get::<String, _>("evidence_refs_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(
            Error::new(err)
                .context("failed to decode sustainability comparison evidence_refs_json"),
        )
    })?;

    Ok(SustainabilityComparisonResult {
        comparison_id: row.get("comparison_id"),
        field_id: row.get("field_id"),
        baseline_season_id: row.get("baseline_season_id"),
        current_season_id: row.get("current_season_id"),
        metric_type: parse_sustainability_metric_type(&row.get::<String, _>("metric_type"))
            .map_err(sustainability_record_error)?,
        baseline_value: row.get("baseline_value"),
        current_value: row.get("current_value"),
        delta: row.get("delta"),
        trend: parse_sustainability_trend(&row.get::<String, _>("trend"))
            .map_err(sustainability_baseline_error)?,
        status: parse_sustainability_comparison_status(&row.get::<String, _>("status"))
            .map_err(sustainability_baseline_error)?,
        baseline_source_record_id: row.get("baseline_source_record_id"),
        current_source_record_id: row.get("current_source_record_id"),
        evidence_refs,
        method_version: row.get("method_version"),
        result_hash: row.get("result_hash"),
        compared_at: row.get("compared_at"),
    })
}

fn decode_sustainability_mrv_trail(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<SustainabilityMrvTrail> {
    let input_layer_refs =
        serde_json::from_str::<Vec<String>>(&row.get::<String, _>("input_layer_refs_json"))
            .map_err(|err| {
                AppError::Anyhow(
                    Error::new(err).context("failed to decode MRV input_layer_refs_json"),
                )
            })?;
    let extent =
        serde_json::from_str::<GeoBounds>(&row.get::<String, _>("extent_json")).map_err(|err| {
            AppError::Anyhow(Error::new(err).context("failed to decode MRV extent_json"))
        })?;
    let parameters =
        serde_json::from_str::<BTreeMap<String, String>>(&row.get::<String, _>("parameters_json"))
            .map_err(|err| {
                AppError::Anyhow(Error::new(err).context("failed to decode MRV parameters_json"))
            })?;

    Ok(SustainabilityMrvTrail {
        trail_id: row.get("trail_id"),
        output_ref: row.get("output_ref"),
        output_kind: parse_sustainability_mrv_output_kind(&row.get::<String, _>("output_kind"))
            .map_err(sustainability_mrv_trail_error)?,
        input_layer_refs,
        method: row.get("method"),
        method_version: row.get("method_version"),
        crs: row.get("crs"),
        extent,
        parameters,
        audit_id: row.get("audit_id"),
        result_hash: row.get("result_hash"),
        rederived_result_hash: row.get("rederived_result_hash"),
        certification_ready: row.get::<i64, _>("certification_ready") != 0,
        created_at: row.get("created_at"),
    })
}

fn decode_biodiversity_proxy_result(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<BiodiversityProxyResult> {
    let extent =
        serde_json::from_str::<GeoBounds>(&row.get::<String, _>("extent_json")).map_err(|err| {
            AppError::Anyhow(Error::new(err).context("failed to decode biodiversity extent_json"))
        })?;
    let source_layer_refs =
        serde_json::from_str::<Vec<String>>(&row.get::<String, _>("source_layer_refs_json"))
            .map_err(|err| {
                AppError::Anyhow(
                    Error::new(err).context("failed to decode biodiversity source_layer_refs_json"),
                )
            })?;

    Ok(BiodiversityProxyResult {
        proxy_id: row.get("proxy_id"),
        field_id: row.get("field_id"),
        heterogeneity_score: row.get("heterogeneity_score"),
        cover_fraction: row.get("cover_fraction"),
        uncertainty: row.get("uncertainty"),
        status: parse_biodiversity_proxy_status(&row.get::<String, _>("status"))
            .map_err(biodiversity_proxy_error)?,
        crs: row.get("crs"),
        extent,
        source_layer_refs,
        method_version: row.get("method_version"),
        result_hash: row.get("result_hash"),
        computed_at: row.get("computed_at"),
    })
}

fn decode_soil_carbon_proxy_result(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<SoilCarbonProxyResult> {
    let evidence_refs = serde_json::from_str::<Vec<String>>(
        &row.get::<String, _>("evidence_refs_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode soil-carbon evidence_refs_json"))
    })?;
    let uncertainty_low: Option<f64> = row.get("uncertainty_low");
    let uncertainty_high: Option<f64> = row.get("uncertainty_high");
    let uncertainty_band = match (uncertainty_low, uncertainty_high) {
        (Some(low), Some(high)) => Some(SoilCarbonUncertaintyBand { low, high }),
        _ => None,
    };

    Ok(SoilCarbonProxyResult {
        proxy_id: row.get("proxy_id"),
        record_id: row.get("record_id"),
        field_id: row.get("field_id"),
        proxy_value: row.get("proxy_value"),
        uncertainty_band,
        status: parse_soil_carbon_proxy_status(&row.get::<String, _>("status"))
            .map_err(soil_carbon_proxy_error)?,
        evidence_refs,
        method_version: row.get("method_version"),
        result_hash: row.get("result_hash"),
        computed_at: row.get("computed_at"),
    })
}

fn decode_sustainability_kpi_result(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<SustainabilityKpiTrackingResult> {
    let evidence_refs = serde_json::from_str::<Vec<String>>(
        &row.get::<String, _>("evidence_refs_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(
            Error::new(err).context("failed to decode sustainability KPI evidence_refs_json"),
        )
    })?;

    Ok(SustainabilityKpiTrackingResult {
        kpi_id: row.get("kpi_id"),
        field_id: row.get("field_id"),
        season_id: row.get("season_id"),
        metric_ref: row.get("metric_ref"),
        current_value: row.get("current_value"),
        target_value: row.get("target_value"),
        direction: parse_sustainability_kpi_direction(&row.get::<String, _>("direction"))
            .map_err(sustainability_kpi_error)?,
        at_risk_fraction: row.get("at_risk_fraction"),
        status: parse_sustainability_kpi_status(&row.get::<String, _>("status"))
            .map_err(sustainability_kpi_error)?,
        evidence_refs,
        method_version: row.get("method_version"),
        result_hash: row.get("result_hash"),
        computed_at: row.get("computed_at"),
    })
}

fn decode_sustainability_certification_pack(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<SustainabilityCertificationEvidencePack> {
    let claimed_output_refs =
        serde_json::from_str::<Vec<String>>(&row.get::<String, _>("claimed_output_refs_json"))
            .map_err(|err| {
                AppError::Anyhow(
                    Error::new(err)
                        .context("failed to decode certification pack claimed_output_refs_json"),
                )
            })?;
    let outputs = serde_json::from_str::<Vec<SustainabilityCertificationOutputItem>>(
        &row.get::<String, _>("outputs_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode certification outputs_json"))
    })?;
    let evidence_layer_refs =
        serde_json::from_str::<Vec<String>>(&row.get::<String, _>("evidence_layer_refs_json"))
            .map_err(|err| {
                AppError::Anyhow(
                    Error::new(err)
                        .context("failed to decode certification pack evidence_layer_refs_json"),
                )
            })?;
    let mrv_trails = serde_json::from_str::<Vec<SustainabilityMrvTrail>>(
        &row.get::<String, _>("mrv_trails_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(
            Error::new(err).context("failed to decode certification pack mrv_trails_json"),
        )
    })?;
    let audit_ids = serde_json::from_str::<Vec<String>>(&row.get::<String, _>("audit_ids_json"))
        .map_err(|err| {
            AppError::Anyhow(
                Error::new(err).context("failed to decode certification pack audit_ids_json"),
            )
        })?;

    Ok(SustainabilityCertificationEvidencePack {
        pack_id: row.get("pack_id"),
        claim_id: row.get("claim_id"),
        claim_type: row.get("claim_type"),
        field_id: row.get("field_id"),
        season_id: row.get("season_id"),
        claimed_output_refs,
        outputs,
        evidence_layer_refs,
        mrv_trails,
        audit_ids,
        result_hash: row.get("result_hash"),
        pack_hash: row.get("pack_hash"),
        method_version: row.get("method_version"),
        created_at: row.get("created_at"),
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

fn decode_success_story_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<ContentSuccessStoryRecord> {
    let metrics = serde_json::from_str::<Vec<shared::schemas::ContentSuccessMetric>>(
        &row.get::<String, _>("metrics_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode success story metrics_json"))
    })?;
    Ok(ContentSuccessStoryRecord {
        content_id: row.get("content_id"),
        grower: row.get("grower"),
        crop: row.get("crop"),
        region: row.get("region"),
        outcome_summary: row.get("outcome_summary"),
        metrics,
    })
}

fn decode_community_contribution_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<ContentCommunityContributionRecord> {
    Ok(ContentCommunityContributionRecord {
        contribution_id: row.get("contribution_id"),
        org_id: row.get("org_id"),
        contributor_id: row.get("contributor_id"),
        content_type: parse_content_type(&row.get::<String, _>("content_type"))
            .map_err(content_error)?,
        body: row.get("body"),
        status: parse_content_contribution_status(&row.get::<String, _>("status"))
            .map_err(content_error)?,
        content_id: row.get("content_id"),
        submitted_at: row.get("submitted_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_content_locale_variant_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<ContentLocaleVariantRecord> {
    Ok(ContentLocaleVariantRecord {
        content_id: row.get("content_id"),
        locale: row.get("locale"),
        version_id: row.get("version_id"),
        body: row.get("body"),
        status: parse_content_status(&row.get::<String, _>("status")).map_err(content_error)?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_content_engagement_event_record(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<ContentEngagementEventRecord> {
    Ok(ContentEngagementEventRecord {
        event_id: row.get("event_id"),
        content_id: row.get("content_id"),
        org_id: row.get("org_id"),
        event_type: parse_content_engagement_event_type(&row.get::<String, _>("event_type"))
            .map_err(content_error)?,
        actor_id: row.get("actor_id"),
        period: row.get("period"),
        occurred_at: row.get("occurred_at"),
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

async fn upsert_fleet_component(state: &AppState, record: &FleetComponentRecord) -> AppResult<()> {
    let service_history_json = serde_json::to_string(&record.service_history)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO fleet_components (
            component_id, component_type, serial, airframe_id, installed_at, removed_at,
            service_history_json, flight_hours, cycles, duty_score, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        ON CONFLICT(component_id) DO UPDATE SET
            component_type = excluded.component_type,
            serial = excluded.serial,
            airframe_id = excluded.airframe_id,
            installed_at = excluded.installed_at,
            removed_at = excluded.removed_at,
            service_history_json = excluded.service_history_json,
            flight_hours = excluded.flight_hours,
            cycles = excluded.cycles,
            duty_score = excluded.duty_score,
            updated_at = excluded.updated_at
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

async fn insert_marketplace_fulfillment_record(
    state: &AppState,
    record: &MarketplaceFulfillmentRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO marketplace_fulfillments (
            fulfillment_id, order_ref, org_id, carrier_ref, tracking_ref,
            status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&record.fulfillment_id)
    .bind(&record.order_ref)
    .bind(&record.org_id)
    .bind(&record.carrier_ref)
    .bind(&record.tracking_ref)
    .bind(record.status.as_str())
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn update_marketplace_fulfillment_record(
    state: &AppState,
    record: &MarketplaceFulfillmentRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE marketplace_fulfillments
        SET status = ?2, updated_at = ?3
        WHERE fulfillment_id = ?1
        "#,
    )
    .bind(&record.fulfillment_id)
    .bind(record.status.as_str())
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_marketplace_fulfillment(
    state: &AppState,
    fulfillment_id: &str,
) -> AppResult<Option<MarketplaceFulfillmentRecord>> {
    let row = sqlx::query(
        r#"
        SELECT fulfillment_id, order_ref, org_id, carrier_ref, tracking_ref,
               status, created_at, updated_at
        FROM marketplace_fulfillments
        WHERE fulfillment_id = ?1
        "#,
    )
    .bind(fulfillment_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_marketplace_fulfillment_record(&row))
        .transpose()
}

async fn insert_marketplace_fulfillment_audit_record(
    state: &AppState,
    record: &MarketplaceFulfillmentAuditRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO marketplace_fulfillment_audits (
            audit_id, fulfillment_id, from_status, to_status, actor_id, occurred_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&record.audit_id)
    .bind(&record.fulfillment_id)
    .bind(record.from_status.map(|status| status.as_str().to_string()))
    .bind(record.to_status.as_str())
    .bind(&record.actor_id)
    .bind(&record.occurred_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn marketplace_order_participants(
    state: &AppState,
    order: &MarketplaceOrderRecord,
) -> AppResult<Vec<String>> {
    let listing = load_marketplace_listing(state, &order.listing_ref)
        .await?
        .ok_or(AppError::NotFound)?;
    let item = load_marketplace_catalog_item(state, &listing.item_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(vec![order.buyer_account_id.clone(), item.owner_account_id])
}

async fn insert_marketplace_rating_record(
    state: &AppState,
    record: &MarketplaceRatingRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO marketplace_ratings (
            rating_id, order_ref, rater_account_id, ratee_account_id,
            score, comment, org_scope, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&record.rating_id)
    .bind(&record.order_ref)
    .bind(&record.rater_account_id)
    .bind(&record.ratee_account_id)
    .bind(record.score)
    .bind(&record.comment)
    .bind(&record.org_scope)
    .bind(&record.created_at)
    .execute(&state.pool)
    .await
    .map_err(|err| {
        if err.to_string().contains("UNIQUE constraint failed") {
            AppError::BadRequest(format!(
                "marketplace rating already exists for order {} and rater {}",
                record.order_ref, record.rater_account_id
            ))
        } else {
            AppError::Anyhow(err.into())
        }
    })?;

    Ok(())
}

async fn load_marketplace_ratings_for_order(
    state: &AppState,
    order_ref: &str,
) -> AppResult<Vec<MarketplaceRatingRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT rating_id, order_ref, rater_account_id, ratee_account_id,
               score, comment, org_scope, created_at
        FROM marketplace_ratings
        WHERE order_ref = ?1
        ORDER BY created_at ASC, rating_id ASC
        "#,
    )
    .bind(order_ref)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_rating_record(&row))
        .collect()
}

async fn load_marketplace_ratings_for_ratee(
    state: &AppState,
    ratee_account_id: &str,
    org_scope: &str,
) -> AppResult<Vec<MarketplaceRatingRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT rating_id, order_ref, rater_account_id, ratee_account_id,
               score, comment, org_scope, created_at
        FROM marketplace_ratings
        WHERE ratee_account_id = ?1 AND org_scope = ?2
        ORDER BY created_at ASC, rating_id ASC
        "#,
    )
    .bind(ratee_account_id)
    .bind(org_scope)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_rating_record(&row))
        .collect()
}

async fn load_marketplace_demand_evidence_refs(
    state: &AppState,
    field_id: &str,
) -> AppResult<Vec<String>> {
    let rows = sqlx::query(
        r#"
        SELECT product_id, scene_id, kind
        FROM products
        WHERE field_id = ?1
          AND kind IN ('yield', 'yield_map', 'health', 'ndvi', 'biomass')
        ORDER BY scene_id ASC, kind ASC, COALESCE(product_id, '') ASC
        "#,
    )
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let product_id: Option<String> = row.get("product_id");
            let scene_id: String = row.get("scene_id");
            let kind: String = row.get("kind");
            product_id
                .filter(|id| !id.trim().is_empty())
                .map(|id| format!("product:{id}"))
                .unwrap_or_else(|| format!("product:{scene_id}:{kind}"))
        })
        .collect())
}

async fn insert_marketplace_demand_forecast_record(
    state: &AppState,
    record: &MarketplaceDemandForecastRecord,
) -> AppResult<()> {
    let evidence_refs_json =
        serde_json::to_string(&record.evidence_refs).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO marketplace_demand_forecasts (
            forecast_id, org_id, field_id, item_kind, horizon, value,
            evidence_refs_json, status, uncertainty_low, uncertainty_high,
            method, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
    )
    .bind(&record.forecast_id)
    .bind(&record.org_id)
    .bind(&record.field_id)
    .bind(record.item_kind.as_str())
    .bind(&record.horizon)
    .bind(record.value)
    .bind(evidence_refs_json)
    .bind(record.status.as_str())
    .bind(record.uncertainty_band.as_ref().map(|band| band.low))
    .bind(record.uncertainty_band.as_ref().map(|band| band.high))
    .bind(&record.method)
    .bind(&record.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_marketplace_demand_forecast(
    state: &AppState,
    forecast_id: &str,
) -> AppResult<Option<MarketplaceDemandForecastRecord>> {
    let row = sqlx::query(
        r#"
        SELECT forecast_id, org_id, field_id, item_kind, horizon, value,
               evidence_refs_json, status, uncertainty_low, uncertainty_high,
               method, created_at
        FROM marketplace_demand_forecasts
        WHERE forecast_id = ?1
        "#,
    )
    .bind(forecast_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_marketplace_demand_forecast_record(&row))
        .transpose()
}

async fn load_marketplace_orders_for_org_period(
    state: &AppState,
    org_id: &str,
    from: &str,
    to: &str,
) -> AppResult<Vec<MarketplaceOrderRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT order_id, org_id, listing_ref, buyer_account_id, qty, line_total,
               status, created_at, updated_at
        FROM marketplace_orders
        WHERE org_id = ?1
          AND created_at >= ?2
          AND created_at <= ?3
        ORDER BY created_at ASC, order_id ASC
        "#,
    )
    .bind(org_id)
    .bind(from)
    .bind(to)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_order_record(&row))
        .collect()
}

async fn load_marketplace_listings_for_org(
    state: &AppState,
    org_id: &str,
) -> AppResult<Vec<MarketplaceListingRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT listing_id, item_id, org_id, price, currency, available_qty,
               window_from, window_to, status, created_at, updated_at
        FROM marketplace_listings
        WHERE org_id = ?1
        ORDER BY created_at ASC, listing_id ASC
        "#,
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_listing_record(&row))
        .collect()
}

async fn load_marketplace_inventory_for_org(
    state: &AppState,
    org_id: &str,
) -> AppResult<Vec<MarketplaceInventoryRecord>> {
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
        .collect()
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

async fn insert_carbon_footprint_result(
    state: &AppState,
    result: &CarbonFootprintResult,
) -> AppResult<()> {
    let inputs_json =
        serde_json::to_string(&result.inputs).map_err(|err| AppError::Anyhow(err.into()))?;
    let factors_json =
        serde_json::to_string(&result.factors).map_err(|err| AppError::Anyhow(err.into()))?;
    let evidence_refs_json =
        serde_json::to_string(&result.evidence_refs).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO carbon_footprints (
            footprint_id, record_id, operation_id, value_co2e, inputs_json,
            factor_set_version, factors_json, evidence_refs_json, status, result_hash,
            computed_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(&result.footprint_id)
    .bind(&result.record_id)
    .bind(&result.operation_id)
    .bind(result.value_co2e)
    .bind(inputs_json)
    .bind(&result.factor_set_version)
    .bind(factors_json)
    .bind(evidence_refs_json)
    .bind(result.status.as_str())
    .bind(&result.result_hash)
    .bind(&result.computed_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_carbon_footprint_result(
    state: &AppState,
    footprint_id: &str,
) -> AppResult<Option<CarbonFootprintResult>> {
    let row = sqlx::query(
        r#"
        SELECT footprint_id, record_id, operation_id, value_co2e, inputs_json,
               factor_set_version, factors_json, evidence_refs_json, status, result_hash,
               computed_at
        FROM carbon_footprints
        WHERE footprint_id = ?1
        "#,
    )
    .bind(footprint_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_carbon_footprint_result(&row))
        .transpose()
}

async fn insert_biomass_estimate_result(
    state: &AppState,
    result: &BiomassEstimateResult,
) -> AppResult<()> {
    let extent_json =
        serde_json::to_string(&result.extent).map_err(|err| AppError::Anyhow(err.into()))?;
    let resolution_json =
        serde_json::to_string(&result.resolution).map_err(|err| AppError::Anyhow(err.into()))?;
    let source_layer_refs_json = serde_json::to_string(&result.source_layer_refs)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO biomass_estimates (
            estimate_id, record_id, biomass_value, area, crs, extent_json,
            resolution_json, source_layer_refs_json, method_version, result_hash, computed_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(&result.estimate_id)
    .bind(&result.record_id)
    .bind(result.biomass_value)
    .bind(result.area)
    .bind(&result.crs)
    .bind(extent_json)
    .bind(resolution_json)
    .bind(source_layer_refs_json)
    .bind(&result.method_version)
    .bind(&result.result_hash)
    .bind(&result.computed_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_biomass_estimate_result(
    state: &AppState,
    estimate_id: &str,
) -> AppResult<Option<BiomassEstimateResult>> {
    let row = sqlx::query(
        r#"
        SELECT estimate_id, record_id, biomass_value, area, crs, extent_json,
               resolution_json, source_layer_refs_json, method_version, result_hash, computed_at
        FROM biomass_estimates
        WHERE estimate_id = ?1
        "#,
    )
    .bind(estimate_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_biomass_estimate_result(&row))
        .transpose()
}

async fn insert_sustainability_baseline(
    state: &AppState,
    baseline: &SustainabilityBaselineRecord,
) -> AppResult<()> {
    let evidence_refs_json = serde_json::to_string(&baseline.evidence_refs)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO sustainability_baselines (
            baseline_id, field_id, season_id, metric_type, metric_value, source_record_id,
            method_version, evidence_refs_json, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(&baseline.baseline_id)
    .bind(&baseline.field_id)
    .bind(&baseline.season_id)
    .bind(baseline.metric_type.as_str())
    .bind(baseline.metric_value)
    .bind(&baseline.source_record_id)
    .bind(&baseline.method_version)
    .bind(evidence_refs_json)
    .bind(&baseline.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_sustainability_baseline_for_metric(
    state: &AppState,
    field_id: &str,
    season_id: &str,
    metric_type: SustainabilityMetricType,
) -> AppResult<Option<SustainabilityBaselineRecord>> {
    let row = sqlx::query(
        r#"
        SELECT baseline_id, field_id, season_id, metric_type, metric_value, source_record_id,
               method_version, evidence_refs_json, created_at
        FROM sustainability_baselines
        WHERE field_id = ?1
          AND season_id = ?2
          AND metric_type = ?3
        ORDER BY created_at DESC, baseline_id DESC
        LIMIT 1
        "#,
    )
    .bind(field_id)
    .bind(season_id)
    .bind(metric_type.as_str())
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_sustainability_baseline(&row))
        .transpose()
}

async fn insert_sustainability_comparison(
    state: &AppState,
    comparison: &SustainabilityComparisonResult,
) -> AppResult<()> {
    let evidence_refs_json = serde_json::to_string(&comparison.evidence_refs)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO sustainability_comparisons (
            comparison_id, field_id, baseline_season_id, current_season_id, metric_type,
            baseline_value, current_value, delta, trend, status, baseline_source_record_id,
            current_source_record_id, evidence_refs_json, method_version, result_hash,
            compared_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
        "#,
    )
    .bind(&comparison.comparison_id)
    .bind(&comparison.field_id)
    .bind(&comparison.baseline_season_id)
    .bind(&comparison.current_season_id)
    .bind(comparison.metric_type.as_str())
    .bind(comparison.baseline_value)
    .bind(comparison.current_value)
    .bind(comparison.delta)
    .bind(comparison.trend.as_str())
    .bind(comparison.status.as_str())
    .bind(&comparison.baseline_source_record_id)
    .bind(&comparison.current_source_record_id)
    .bind(evidence_refs_json)
    .bind(&comparison.method_version)
    .bind(&comparison.result_hash)
    .bind(&comparison.compared_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_sustainability_comparison(
    state: &AppState,
    comparison_id: &str,
) -> AppResult<Option<SustainabilityComparisonResult>> {
    let row = sqlx::query(
        r#"
        SELECT comparison_id, field_id, baseline_season_id, current_season_id, metric_type,
               baseline_value, current_value, delta, trend, status, baseline_source_record_id,
               current_source_record_id, evidence_refs_json, method_version, result_hash,
               compared_at
        FROM sustainability_comparisons
        WHERE comparison_id = ?1
        "#,
    )
    .bind(comparison_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_sustainability_comparison(&row))
        .transpose()
}

async fn validate_sustainability_mrv_output_ref(
    state: &AppState,
    output_kind: SustainabilityMrvOutputKind,
    output_ref: &str,
) -> AppResult<()> {
    let exists: i64 = match output_kind {
        SustainabilityMrvOutputKind::CarbonFootprint => {
            sqlx::query_scalar("SELECT COUNT(*) FROM carbon_footprints WHERE footprint_id = ?1")
                .bind(output_ref)
                .fetch_one(&state.pool)
                .await
                .map_err(Error::from)?
        }
        SustainabilityMrvOutputKind::BiomassEstimate => {
            sqlx::query_scalar("SELECT COUNT(*) FROM biomass_estimates WHERE estimate_id = ?1")
                .bind(output_ref)
                .fetch_one(&state.pool)
                .await
                .map_err(Error::from)?
        }
        SustainabilityMrvOutputKind::SustainabilityKpi => {
            sqlx::query_scalar("SELECT COUNT(*) FROM sustainability_kpis WHERE kpi_id = ?1")
                .bind(output_ref)
                .fetch_one(&state.pool)
                .await
                .map_err(Error::from)?
        }
    };
    if exists == 0 {
        return Err(AppError::BadRequest(format!(
            "MRV output_ref {output_ref} does not exist for {}",
            output_kind.as_str()
        )));
    }
    Ok(())
}

async fn insert_sustainability_mrv_trail(
    state: &AppState,
    trail: &SustainabilityMrvTrail,
) -> AppResult<()> {
    let input_layer_refs_json = serde_json::to_string(&trail.input_layer_refs)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let extent_json =
        serde_json::to_string(&trail.extent).map_err(|err| AppError::Anyhow(err.into()))?;
    let parameters_json =
        serde_json::to_string(&trail.parameters).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO sustainability_mrv_trails (
            trail_id, output_ref, output_kind, input_layer_refs_json, method, method_version,
            crs, extent_json, parameters_json, audit_id, result_hash, rederived_result_hash,
            certification_ready, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        "#,
    )
    .bind(&trail.trail_id)
    .bind(&trail.output_ref)
    .bind(trail.output_kind.as_str())
    .bind(input_layer_refs_json)
    .bind(&trail.method)
    .bind(&trail.method_version)
    .bind(&trail.crs)
    .bind(extent_json)
    .bind(parameters_json)
    .bind(&trail.audit_id)
    .bind(&trail.result_hash)
    .bind(&trail.rederived_result_hash)
    .bind(if trail.certification_ready {
        1_i64
    } else {
        0_i64
    })
    .bind(&trail.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_sustainability_mrv_trail(
    state: &AppState,
    trail_id: &str,
) -> AppResult<Option<SustainabilityMrvTrail>> {
    let row = sqlx::query(
        r#"
        SELECT trail_id, output_ref, output_kind, input_layer_refs_json, method,
               method_version, crs, extent_json, parameters_json, audit_id, result_hash,
               rederived_result_hash, certification_ready, created_at
        FROM sustainability_mrv_trails
        WHERE trail_id = ?1
        "#,
    )
    .bind(trail_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_sustainability_mrv_trail(&row))
        .transpose()
}

async fn insert_biodiversity_proxy_result(
    state: &AppState,
    result: &BiodiversityProxyResult,
) -> AppResult<()> {
    let extent_json =
        serde_json::to_string(&result.extent).map_err(|err| AppError::Anyhow(err.into()))?;
    let source_layer_refs_json = serde_json::to_string(&result.source_layer_refs)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO biodiversity_proxies (
            proxy_id, field_id, heterogeneity_score, cover_fraction, uncertainty, status,
            crs, extent_json, source_layer_refs_json, method_version, result_hash, computed_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
    )
    .bind(&result.proxy_id)
    .bind(&result.field_id)
    .bind(result.heterogeneity_score)
    .bind(result.cover_fraction)
    .bind(result.uncertainty)
    .bind(result.status.as_str())
    .bind(&result.crs)
    .bind(extent_json)
    .bind(source_layer_refs_json)
    .bind(&result.method_version)
    .bind(&result.result_hash)
    .bind(&result.computed_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_biodiversity_proxy_result(
    state: &AppState,
    proxy_id: &str,
) -> AppResult<Option<BiodiversityProxyResult>> {
    let row = sqlx::query(
        r#"
        SELECT proxy_id, field_id, heterogeneity_score, cover_fraction, uncertainty, status,
               crs, extent_json, source_layer_refs_json, method_version, result_hash, computed_at
        FROM biodiversity_proxies
        WHERE proxy_id = ?1
        "#,
    )
    .bind(proxy_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_biodiversity_proxy_result(&row))
        .transpose()
}

async fn insert_soil_carbon_proxy_result(
    state: &AppState,
    result: &SoilCarbonProxyResult,
) -> AppResult<()> {
    let evidence_refs_json =
        serde_json::to_string(&result.evidence_refs).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO soil_carbon_proxies (
            proxy_id, record_id, field_id, proxy_value, uncertainty_low, uncertainty_high, status,
            evidence_refs_json, method_version, result_hash, computed_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(&result.proxy_id)
    .bind(&result.record_id)
    .bind(&result.field_id)
    .bind(result.proxy_value)
    .bind(result.uncertainty_band.as_ref().map(|band| band.low))
    .bind(result.uncertainty_band.as_ref().map(|band| band.high))
    .bind(result.status.as_str())
    .bind(evidence_refs_json)
    .bind(&result.method_version)
    .bind(&result.result_hash)
    .bind(&result.computed_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_soil_carbon_proxy_result(
    state: &AppState,
    proxy_id: &str,
) -> AppResult<Option<SoilCarbonProxyResult>> {
    let row = sqlx::query(
        r#"
        SELECT proxy_id, record_id, field_id, proxy_value, uncertainty_low, uncertainty_high,
               status, evidence_refs_json, method_version, result_hash, computed_at
        FROM soil_carbon_proxies
        WHERE proxy_id = ?1
        "#,
    )
    .bind(proxy_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_soil_carbon_proxy_result(&row))
        .transpose()
}

async fn insert_sustainability_kpi_result(
    state: &AppState,
    result: &SustainabilityKpiTrackingResult,
) -> AppResult<()> {
    let evidence_refs_json =
        serde_json::to_string(&result.evidence_refs).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO sustainability_kpis (
            kpi_id, field_id, season_id, metric_ref, current_value, target_value, direction,
            at_risk_fraction, status, evidence_refs_json, method_version, result_hash, computed_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        "#,
    )
    .bind(&result.kpi_id)
    .bind(&result.field_id)
    .bind(&result.season_id)
    .bind(&result.metric_ref)
    .bind(result.current_value)
    .bind(result.target_value)
    .bind(result.direction.as_str())
    .bind(result.at_risk_fraction)
    .bind(result.status.as_str())
    .bind(evidence_refs_json)
    .bind(&result.method_version)
    .bind(&result.result_hash)
    .bind(&result.computed_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_sustainability_kpi_result(
    state: &AppState,
    kpi_id: &str,
) -> AppResult<Option<SustainabilityKpiTrackingResult>> {
    let row = sqlx::query(
        r#"
        SELECT kpi_id, field_id, season_id, metric_ref, current_value, target_value,
               direction, at_risk_fraction, status, evidence_refs_json, method_version,
               result_hash, computed_at
        FROM sustainability_kpis
        WHERE kpi_id = ?1
        "#,
    )
    .bind(kpi_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_sustainability_kpi_result(&row))
        .transpose()
}

async fn assemble_sustainability_certification_pack_inputs(
    state: &AppState,
    claimed_output_refs: &[String],
) -> AppResult<(
    Vec<SustainabilityCertificationOutputItem>,
    Vec<SustainabilityMrvTrail>,
    Vec<String>,
)> {
    let mut outputs = Vec::new();
    let mut trails = Vec::new();
    let mut evidence_layer_refs = BTreeSet::new();

    for output_ref in claimed_output_refs {
        let trail = load_latest_sustainability_mrv_trail_for_output(state, output_ref)
            .await?
            .ok_or_else(|| {
                sustainability_certification_pack_error(
                    SustainabilityCertificationEvidencePackError::MissingMrvTrail {
                        output_ref: output_ref.clone(),
                    },
                )
            })?;
        evidence_layer_refs.extend(trail.input_layer_refs.iter().cloned());
        outputs.push(load_sustainability_certification_output_item(state, &trail).await?);
        trails.push(trail);
    }

    Ok((
        outputs,
        trails,
        evidence_layer_refs.into_iter().collect::<Vec<_>>(),
    ))
}

async fn load_sustainability_field_export_summary(
    state: &AppState,
    field_id: &str,
    season_id: Option<String>,
) -> AppResult<SustainabilityFieldExportSummary> {
    let field = load_field(state, field_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let season_id = normalize_optional_text(season_id);
    let mut items = Vec::new();
    items.extend(
        load_sustainability_carbon_export_items(state, field_id, season_id.as_deref()).await?,
    );
    items.extend(
        load_sustainability_biomass_export_items(state, field_id, season_id.as_deref()).await?,
    );
    if season_id.is_none() {
        items.extend(load_sustainability_biodiversity_export_items(state, field_id).await?);
        items.extend(load_sustainability_soil_carbon_export_items(state, field_id).await?);
    }
    items
        .extend(load_sustainability_kpi_export_items(state, field_id, season_id.as_deref()).await?);
    items.sort_by(|left, right| {
        left.computed_at
            .cmp(&right.computed_at)
            .then_with(|| left.record_type.cmp(&right.record_type))
            .then_with(|| left.record_id.cmp(&right.record_id))
    });
    let crs = items
        .iter()
        .find_map(|item| item.crs.clone())
        .unwrap_or_else(|| field_record_crs(&field));
    let record_count = items.len();

    Ok(SustainabilityFieldExportSummary {
        field_id: field.field_id,
        season_id,
        crs,
        record_count,
        empty: record_count == 0,
        items,
        generated_at: current_record_timestamp(),
    })
}

async fn load_sustainability_carbon_export_items(
    state: &AppState,
    field_id: &str,
    season_id: Option<&str>,
) -> AppResult<Vec<SustainabilityExportItem>> {
    let rows = sqlx::query(
        r#"
        SELECT cf.footprint_id, cf.record_id, cf.operation_id, cf.value_co2e, cf.inputs_json,
               cf.factor_set_version, cf.factors_json, cf.evidence_refs_json, cf.status,
               cf.result_hash, cf.computed_at, sr.field_id, sr.season_id
        FROM carbon_footprints cf
        JOIN sustainability_records sr ON sr.record_id = cf.record_id
        WHERE sr.field_id = ?1
          AND (?2 IS NULL OR sr.season_id = ?2)
        ORDER BY cf.computed_at ASC, cf.footprint_id ASC
        "#,
    )
    .bind(field_id)
    .bind(season_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| {
            let field_id: String = row.get("field_id");
            let season_id: String = row.get("season_id");
            let footprint = decode_carbon_footprint_result(&row)?;
            Ok(SustainabilityExportItem {
                record_type: "carbon_footprint".to_string(),
                record_id: footprint.footprint_id,
                field_id,
                season_id: Some(season_id),
                metric_ref: format!("record:{}", footprint.record_id),
                value: footprint.value_co2e,
                unit: "kg_co2e".to_string(),
                status: footprint.status.as_str().to_string(),
                crs: None,
                extent: None,
                method_version: footprint.factor_set_version,
                evidence_refs: footprint.evidence_refs,
                result_hash: footprint.result_hash,
                computed_at: footprint.computed_at,
            })
        })
        .collect()
}

async fn load_sustainability_biomass_export_items(
    state: &AppState,
    field_id: &str,
    season_id: Option<&str>,
) -> AppResult<Vec<SustainabilityExportItem>> {
    let rows = sqlx::query(
        r#"
        SELECT be.estimate_id, be.record_id, be.biomass_value, be.area, be.crs, be.extent_json,
               be.resolution_json, be.source_layer_refs_json, be.method_version, be.result_hash,
               be.computed_at, sr.field_id, sr.season_id
        FROM biomass_estimates be
        JOIN sustainability_records sr ON sr.record_id = be.record_id
        WHERE sr.field_id = ?1
          AND (?2 IS NULL OR sr.season_id = ?2)
        ORDER BY be.computed_at ASC, be.estimate_id ASC
        "#,
    )
    .bind(field_id)
    .bind(season_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| {
            let field_id: String = row.get("field_id");
            let season_id: String = row.get("season_id");
            let estimate = decode_biomass_estimate_result(&row)?;
            Ok(SustainabilityExportItem {
                record_type: "biomass_estimate".to_string(),
                record_id: estimate.estimate_id,
                field_id,
                season_id: Some(season_id),
                metric_ref: format!("record:{}", estimate.record_id),
                value: Some(estimate.biomass_value),
                unit: "biomass_index".to_string(),
                status: "computed".to_string(),
                crs: Some(estimate.crs),
                extent: Some(estimate.extent),
                method_version: estimate.method_version,
                evidence_refs: estimate.source_layer_refs,
                result_hash: estimate.result_hash,
                computed_at: estimate.computed_at,
            })
        })
        .collect()
}

async fn load_sustainability_biodiversity_export_items(
    state: &AppState,
    field_id: &str,
) -> AppResult<Vec<SustainabilityExportItem>> {
    let rows = sqlx::query(
        r#"
        SELECT proxy_id, field_id, heterogeneity_score, cover_fraction, uncertainty, status, crs,
               extent_json, source_layer_refs_json, method_version, result_hash, computed_at
        FROM biodiversity_proxies
        WHERE field_id = ?1
        ORDER BY computed_at ASC, proxy_id ASC
        "#,
    )
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| {
            let proxy = decode_biodiversity_proxy_result(&row)?;
            Ok(SustainabilityExportItem {
                record_type: "biodiversity_proxy".to_string(),
                record_id: proxy.proxy_id,
                field_id: proxy.field_id,
                season_id: None,
                metric_ref: "biodiversity:cover_fraction".to_string(),
                value: proxy.cover_fraction,
                unit: "fraction".to_string(),
                status: proxy.status.as_str().to_string(),
                crs: Some(proxy.crs),
                extent: Some(proxy.extent),
                method_version: proxy.method_version,
                evidence_refs: proxy.source_layer_refs,
                result_hash: proxy.result_hash,
                computed_at: proxy.computed_at,
            })
        })
        .collect()
}

async fn load_sustainability_soil_carbon_export_items(
    state: &AppState,
    field_id: &str,
) -> AppResult<Vec<SustainabilityExportItem>> {
    let rows = sqlx::query(
        r#"
        SELECT proxy_id, record_id, field_id, proxy_value, uncertainty_low, uncertainty_high,
               status, evidence_refs_json, method_version, result_hash, computed_at
        FROM soil_carbon_proxies
        WHERE field_id = ?1
        ORDER BY computed_at ASC, proxy_id ASC
        "#,
    )
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| {
            let proxy = decode_soil_carbon_proxy_result(&row)?;
            Ok(SustainabilityExportItem {
                record_type: "soil_carbon_proxy".to_string(),
                record_id: proxy.proxy_id,
                field_id: proxy.field_id,
                season_id: None,
                metric_ref: format!("record:{}", proxy.record_id),
                value: proxy.proxy_value,
                unit: "soil_carbon_proxy".to_string(),
                status: proxy.status.as_str().to_string(),
                crs: None,
                extent: None,
                method_version: proxy.method_version,
                evidence_refs: proxy.evidence_refs,
                result_hash: proxy.result_hash,
                computed_at: proxy.computed_at,
            })
        })
        .collect()
}

async fn load_sustainability_kpi_export_items(
    state: &AppState,
    field_id: &str,
    season_id: Option<&str>,
) -> AppResult<Vec<SustainabilityExportItem>> {
    let rows = sqlx::query(
        r#"
        SELECT kpi_id, field_id, season_id, metric_ref, current_value, target_value,
               direction, at_risk_fraction, status, evidence_refs_json, method_version,
               result_hash, computed_at
        FROM sustainability_kpis
        WHERE field_id = ?1
          AND (?2 IS NULL OR season_id = ?2)
        ORDER BY computed_at ASC, kpi_id ASC
        "#,
    )
    .bind(field_id)
    .bind(season_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| {
            let kpi = decode_sustainability_kpi_result(&row)?;
            Ok(SustainabilityExportItem {
                record_type: "sustainability_kpi".to_string(),
                record_id: kpi.kpi_id,
                field_id: kpi.field_id,
                season_id: Some(kpi.season_id),
                metric_ref: kpi.metric_ref,
                value: kpi.current_value,
                unit: "kpi_value".to_string(),
                status: kpi.status.as_str().to_string(),
                crs: None,
                extent: None,
                method_version: kpi.method_version,
                evidence_refs: kpi.evidence_refs,
                result_hash: kpi.result_hash,
                computed_at: kpi.computed_at,
            })
        })
        .collect()
}

fn sustainability_export_feature(item: &SustainabilityExportItem) -> AppResult<Feature> {
    let mut properties = serde_json::Map::new();
    properties.insert(
        "record_type".to_string(),
        serde_json::Value::String(item.record_type.clone()),
    );
    properties.insert(
        "record_id".to_string(),
        serde_json::Value::String(item.record_id.clone()),
    );
    properties.insert(
        "field_id".to_string(),
        serde_json::Value::String(item.field_id.clone()),
    );
    if let Some(season_id) = &item.season_id {
        properties.insert(
            "season_id".to_string(),
            serde_json::Value::String(season_id.clone()),
        );
    }
    properties.insert(
        "metric_ref".to_string(),
        serde_json::Value::String(item.metric_ref.clone()),
    );
    if let Some(value) = item.value {
        properties.insert("value".to_string(), serde_json::Value::from(value));
    }
    properties.insert(
        "unit".to_string(),
        serde_json::Value::String(item.unit.clone()),
    );
    properties.insert(
        "status".to_string(),
        serde_json::Value::String(item.status.clone()),
    );
    if let Some(crs) = &item.crs {
        properties.insert("crs".to_string(), serde_json::Value::String(crs.clone()));
    }
    properties.insert(
        "method_version".to_string(),
        serde_json::Value::String(item.method_version.clone()),
    );
    properties.insert(
        "evidence_refs".to_string(),
        serde_json::to_value(&item.evidence_refs).map_err(|err| AppError::Anyhow(err.into()))?,
    );
    properties.insert(
        "result_hash".to_string(),
        serde_json::Value::String(item.result_hash.clone()),
    );
    properties.insert(
        "computed_at".to_string(),
        serde_json::Value::String(item.computed_at.clone()),
    );

    let geometry = item.extent.as_ref().map(|extent| {
        Geometry::new(GeoJsonValue::Polygon(vec![vec![
            vec![extent.min_lon, extent.min_lat],
            vec![extent.max_lon, extent.min_lat],
            vec![extent.max_lon, extent.max_lat],
            vec![extent.min_lon, extent.max_lat],
            vec![extent.min_lon, extent.min_lat],
        ]]))
    });

    Ok(Feature {
        bbox: None,
        geometry,
        id: Some(GeoJsonId::String(item.record_id.clone())),
        properties: Some(properties),
        foreign_members: None,
    })
}

fn sustainability_summary_pdf_bytes(summary: &SustainabilityFieldExportSummary) -> Vec<u8> {
    let mut lines = vec![
        "AGBot Sustainability Summary".to_string(),
        format!("field_id: {}", summary.field_id),
        format!(
            "season_id: {}",
            summary.season_id.as_deref().unwrap_or("all")
        ),
        format!("record_count: {}", summary.record_count),
        format!("generated_at: {}", summary.generated_at),
    ];
    if summary.items.is_empty() {
        lines.push("empty: true".to_string());
        lines.push("No sustainability records were available for this field scope.".to_string());
    }
    for item in &summary.items {
        lines.push(format!(
            "{} {} value={} unit={} status={} method_version={} evidence_refs={} result_hash={}",
            item.record_type,
            item.record_id,
            item.value
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            item.unit,
            item.status,
            item.method_version,
            item.evidence_refs.join("|"),
            item.result_hash
        ));
    }
    simple_pdf_bytes(&lines)
}

fn simple_pdf_bytes(lines: &[String]) -> Vec<u8> {
    let mut stream = String::from("BT\n/F1 10 Tf\n50 760 Td\n");
    for (index, line) in lines.iter().take(42).enumerate() {
        if index > 0 {
            stream.push_str("0 -16 Td\n");
        }
        stream.push('(');
        stream.push_str(&pdf_escape_text(line));
        stream.push_str(") Tj\n");
    }
    stream.push_str("ET\n");
    let objects = [
        "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_string(),
        "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_string(),
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n".to_string(),
        "4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_string(),
        format!(
            "5 0 obj\n<< /Length {} >>\nstream\n{}endstream\nendobj\n",
            stream.len(),
            stream
        ),
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = vec![0_usize];
    for object in &objects {
        offsets.push(pdf.len());
        pdf.push_str(object);
    }
    let xref_offset = pdf.len();
    pdf.push_str("xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n"
    ));
    pdf.into_bytes()
}

fn pdf_escape_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '(' => "\\(".to_string(),
            ')' => "\\)".to_string(),
            '\\' => "\\\\".to_string(),
            ch if ch.is_ascii_control() => " ".to_string(),
            ch => ch.to_string(),
        })
        .collect::<String>()
}

async fn load_latest_sustainability_mrv_trail_for_output(
    state: &AppState,
    output_ref: &str,
) -> AppResult<Option<SustainabilityMrvTrail>> {
    let row = sqlx::query(
        r#"
        SELECT trail_id, output_ref, output_kind, input_layer_refs_json, method,
               method_version, crs, extent_json, parameters_json, audit_id, result_hash,
               rederived_result_hash, certification_ready, created_at
        FROM sustainability_mrv_trails
        WHERE output_ref = ?1
        ORDER BY created_at DESC, trail_id DESC
        LIMIT 1
        "#,
    )
    .bind(output_ref)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_sustainability_mrv_trail(&row))
        .transpose()
}

async fn load_sustainability_certification_output_item(
    state: &AppState,
    trail: &SustainabilityMrvTrail,
) -> AppResult<SustainabilityCertificationOutputItem> {
    match trail.output_kind {
        SustainabilityMrvOutputKind::CarbonFootprint => {
            let output = load_carbon_footprint_result(state, &trail.output_ref)
                .await?
                .ok_or_else(|| {
                    sustainability_certification_pack_error(
                        SustainabilityCertificationEvidencePackError::MissingClaimedOutput {
                            output_ref: trail.output_ref.clone(),
                        },
                    )
                })?;
            Ok(SustainabilityCertificationOutputItem {
                output_ref: output.footprint_id,
                output_kind: SustainabilityMrvOutputKind::CarbonFootprint,
                value: output.value_co2e,
                unit: Some("kg_co2e".to_string()),
                method_version: output.factor_set_version,
                result_hash: output.result_hash,
            })
        }
        SustainabilityMrvOutputKind::BiomassEstimate => {
            let output = load_biomass_estimate_result(state, &trail.output_ref)
                .await?
                .ok_or_else(|| {
                    sustainability_certification_pack_error(
                        SustainabilityCertificationEvidencePackError::MissingClaimedOutput {
                            output_ref: trail.output_ref.clone(),
                        },
                    )
                })?;
            Ok(SustainabilityCertificationOutputItem {
                output_ref: output.estimate_id,
                output_kind: SustainabilityMrvOutputKind::BiomassEstimate,
                value: Some(output.biomass_value),
                unit: Some("biomass_index".to_string()),
                method_version: output.method_version,
                result_hash: output.result_hash,
            })
        }
        SustainabilityMrvOutputKind::SustainabilityKpi => {
            let output = load_sustainability_kpi_result(state, &trail.output_ref)
                .await?
                .ok_or_else(|| {
                    sustainability_certification_pack_error(
                        SustainabilityCertificationEvidencePackError::MissingClaimedOutput {
                            output_ref: trail.output_ref.clone(),
                        },
                    )
                })?;
            Ok(SustainabilityCertificationOutputItem {
                output_ref: output.kpi_id,
                output_kind: SustainabilityMrvOutputKind::SustainabilityKpi,
                value: output.current_value,
                unit: Some("kpi_value".to_string()),
                method_version: output.method_version,
                result_hash: output.result_hash,
            })
        }
    }
}

async fn insert_sustainability_certification_pack(
    state: &AppState,
    pack: &SustainabilityCertificationEvidencePack,
) -> AppResult<()> {
    let claimed_output_refs_json = serde_json::to_string(&pack.claimed_output_refs)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let outputs_json =
        serde_json::to_string(&pack.outputs).map_err(|err| AppError::Anyhow(err.into()))?;
    let evidence_layer_refs_json = serde_json::to_string(&pack.evidence_layer_refs)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let mrv_trails_json =
        serde_json::to_string(&pack.mrv_trails).map_err(|err| AppError::Anyhow(err.into()))?;
    let audit_ids_json =
        serde_json::to_string(&pack.audit_ids).map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO sustainability_certification_packs (
            pack_id, claim_id, claim_type, field_id, season_id, claimed_output_refs_json,
            outputs_json, evidence_layer_refs_json, mrv_trails_json, audit_ids_json,
            result_hash, pack_hash, method_version, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        "#,
    )
    .bind(&pack.pack_id)
    .bind(&pack.claim_id)
    .bind(&pack.claim_type)
    .bind(&pack.field_id)
    .bind(&pack.season_id)
    .bind(claimed_output_refs_json)
    .bind(outputs_json)
    .bind(evidence_layer_refs_json)
    .bind(mrv_trails_json)
    .bind(audit_ids_json)
    .bind(&pack.result_hash)
    .bind(&pack.pack_hash)
    .bind(&pack.method_version)
    .bind(&pack.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_sustainability_certification_pack(
    state: &AppState,
    pack_id: &str,
) -> AppResult<Option<SustainabilityCertificationEvidencePack>> {
    let row = sqlx::query(
        r#"
        SELECT pack_id, claim_id, claim_type, field_id, season_id, claimed_output_refs_json,
               outputs_json, evidence_layer_refs_json, mrv_trails_json, audit_ids_json,
               result_hash, pack_hash, method_version, created_at
        FROM sustainability_certification_packs
        WHERE pack_id = ?1
        "#,
    )
    .bind(pack_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_sustainability_certification_pack(&row))
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

async fn assert_content_workflow_permission(
    query: &ContentItemScopeQuery,
    content: &ContentRecord,
    request: &ContentWorkflowTransitionRequest,
    state: &AppState,
) -> AppResult<()> {
    let Some(actor_org_id) = normalize_optional_text(query.actor_org_id.clone()) else {
        return Ok(());
    };
    let Some(role_refs) = query.role_refs.clone() else {
        return Ok(());
    };
    let permissions = resolve_content_permissions(ContentPermissionResolveRequest {
        org_id: content.org_id.clone(),
        actor_org_id,
        role_refs: parse_role_refs(Some(role_refs)),
    })
    .map_err(content_error)?;
    let (allowed, permission) = match request.action {
        ContentWorkflowAction::SubmitForReview => (permissions.can_author, "can_author"),
        ContentWorkflowAction::Publish => (permissions.can_publish, "can_publish"),
        ContentWorkflowAction::Reject | ContentWorkflowAction::Unpublish => {
            (permissions.can_moderate, "can_moderate")
        }
    };
    if allowed {
        return Ok(());
    }

    insert_content_workflow_denial_audit(state, content, request).await?;
    Err(AppError::Forbidden(
        ContentError::AccessDenied { permission }.to_string(),
    ))
}

fn parse_role_refs(value: Option<String>) -> Vec<String> {
    value
        .into_iter()
        .flat_map(|roles| {
            roles
                .split(',')
                .map(str::trim)
                .filter(|role| !role.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}

async fn assert_collaboration_action_permission(
    state: &AppState,
    org_id: &str,
    actor_org_id: Option<String>,
    actor_id: Option<String>,
    role_refs: Option<String>,
    action: CollaborationAction,
    channel_id: Option<&str>,
) -> AppResult<()> {
    let Some(actor_org_id) = normalize_optional_text(actor_org_id) else {
        return Ok(());
    };
    let Some(role_refs) = role_refs else {
        return Ok(());
    };
    let actor_id = normalize_optional_text(actor_id).unwrap_or_else(|| "unknown".to_string());
    let decision = authorize_collaboration_action(
        CollaborationPermissionResolveRequest {
            org_id: org_id.to_string(),
            actor_org_id,
            role_refs: parse_role_refs(Some(role_refs)),
        },
        action,
    )
    .map_err(collaboration_error)?;
    insert_collaboration_permission_audit(state, &decision, &actor_id, channel_id).await?;
    if decision.allowed {
        return Ok(());
    }

    Err(AppError::Forbidden(
        CollaborationError::AccessDenied {
            permission: decision.action.permission_name(),
        }
        .to_string(),
    ))
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

async fn insert_success_story_item_with_version(
    state: &AppState,
    content: &ContentRecord,
    version: &ContentVersionRecord,
    success_story: &ContentSuccessStoryRecord,
) -> AppResult<()> {
    let metrics_json = serde_json::to_string(&success_story.metrics)
        .map_err(|err| AppError::Anyhow(err.into()))?;
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
    sqlx::query(
        r#"
        INSERT INTO cms_success_stories (
            content_id, grower, crop, region, outcome_summary, metrics_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&success_story.content_id)
    .bind(&success_story.grower)
    .bind(&success_story.crop)
    .bind(&success_story.region)
    .bind(&success_story.outcome_summary)
    .bind(metrics_json)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;
    tx.commit().await.map_err(Error::from)?;

    Ok(())
}

async fn insert_community_contribution(
    state: &AppState,
    contribution: &ContentCommunityContributionRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO cms_community_contributions (
            contribution_id, org_id, contributor_id, content_type, body, status, content_id,
            submitted_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(&contribution.contribution_id)
    .bind(&contribution.org_id)
    .bind(&contribution.contributor_id)
    .bind(contribution.content_type.as_str())
    .bind(&contribution.body)
    .bind(contribution.status.as_str())
    .bind(&contribution.content_id)
    .bind(&contribution.submitted_at)
    .bind(&contribution.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn persist_community_moderation_result(
    state: &AppState,
    result: &ContentContributionModerationResult,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    sqlx::query(
        r#"
        UPDATE cms_community_contributions
        SET status = ?2, content_id = ?3, updated_at = ?4
        WHERE contribution_id = ?1
        "#,
    )
    .bind(&result.contribution.contribution_id)
    .bind(result.contribution.status.as_str())
    .bind(&result.contribution.content_id)
    .bind(&result.contribution.updated_at)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;
    insert_community_moderation_audit_in_tx(&mut tx, &result.audit).await?;
    if let Some(content) = &result.content {
        let version = content.versions.first().ok_or_else(|| {
            AppError::BadRequest("approved contribution missing version".to_string())
        })?;
        sqlx::query(
            r#"
            INSERT INTO cms_contents (
                content_id, content_type, author_id, org_id, status, current_version,
                created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(&content.content.content_id)
        .bind(content.content.content_type.as_str())
        .bind(&content.content.author_id)
        .bind(&content.content.org_id)
        .bind(content.content.status.as_str())
        .bind(&content.content.current_version)
        .bind(&content.content.created_at)
        .bind(&content.content.updated_at)
        .execute(&mut *tx)
        .await
        .map_err(Error::from)?;
        insert_content_version_in_tx(&mut tx, version).await?;
    }
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

async fn persist_content_workflow_transition(
    state: &AppState,
    transition: &ContentWorkflowTransitionResult,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    sqlx::query(
        r#"
        UPDATE cms_contents
        SET status = ?2, updated_at = ?3
        WHERE content_id = ?1
        "#,
    )
    .bind(&transition.content.content_id)
    .bind(transition.content.status.as_str())
    .bind(&transition.content.updated_at)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;
    insert_content_workflow_audit_in_tx(&mut tx, &transition.audit).await?;
    tx.commit().await.map_err(Error::from)?;

    Ok(())
}

async fn insert_content_workflow_denial_audit(
    state: &AppState,
    content: &ContentRecord,
    request: &ContentWorkflowTransitionRequest,
) -> AppResult<()> {
    let audit = ContentWorkflowAuditRecord {
        audit_id: format!("content-workflow-denied-{}", Uuid::new_v4()),
        content_id: content.content_id.clone(),
        action: request.action,
        from_status: content.status,
        to_status: content.status,
        actor_id: normalize_optional_text(Some(request.actor_id.clone()))
            .unwrap_or_else(|| "unknown".to_string()),
        actor_role: request.actor_role,
        occurred_at: current_record_timestamp(),
        scheduled_effective_at: request.scheduled_effective_at.clone(),
    };
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    insert_content_workflow_audit_in_tx(&mut tx, &audit).await?;
    tx.commit().await.map_err(Error::from)?;

    Ok(())
}

async fn insert_content_tags(state: &AppState, tags: &[ContentTagRecord]) -> AppResult<()> {
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    for tag in tags {
        sqlx::query(
            r#"
            INSERT INTO cms_content_tags (content_id, kind, value, source, applied_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(content_id, kind, value) DO UPDATE SET
                source = excluded.source,
                applied_at = excluded.applied_at
            "#,
        )
        .bind(&tag.content_id)
        .bind(tag.kind.as_str())
        .bind(&tag.value)
        .bind(&tag.source)
        .bind(&tag.applied_at)
        .execute(&mut *tx)
        .await
        .map_err(Error::from)?;
    }
    tx.commit().await.map_err(Error::from)?;

    Ok(())
}

async fn insert_content_engagement_event(
    state: &AppState,
    event: &ContentEngagementEventRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO cms_content_engagement_events (
            event_id, content_id, org_id, event_type, actor_id, period, occurred_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&event.event_id)
    .bind(&event.content_id)
    .bind(&event.org_id)
    .bind(event.event_type.as_str())
    .bind(&event.actor_id)
    .bind(&event.period)
    .bind(&event.occurred_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn upsert_content_locale_variant(
    state: &AppState,
    variant: &ContentLocaleVariantRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO cms_content_locale_variants (
            content_id, locale, version_id, body, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(content_id, locale) DO UPDATE SET
            version_id = excluded.version_id,
            body = excluded.body,
            status = excluded.status,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&variant.content_id)
    .bind(&variant.locale)
    .bind(&variant.version_id)
    .bind(&variant.body)
    .bind(variant.status.as_str())
    .bind(&variant.created_at)
    .bind(&variant.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn upsert_content_engagement_summary(
    state: &AppState,
    summary: &ContentEngagementSummary,
) -> AppResult<()> {
    let evidence_refs_json = serde_json::to_string(&summary.evidence_refs)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO cms_content_engagement_summaries (
            content_id, org_id, period, views, reads, helpful_votes, event_count,
            evidence_refs_json, computed_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(content_id, org_id, period) DO UPDATE SET
            views = excluded.views,
            reads = excluded.reads,
            helpful_votes = excluded.helpful_votes,
            event_count = excluded.event_count,
            evidence_refs_json = excluded.evidence_refs_json,
            computed_at = excluded.computed_at
        "#,
    )
    .bind(&summary.content_id)
    .bind(&summary.org_id)
    .bind(&summary.period)
    .bind(summary.views as i64)
    .bind(summary.reads as i64)
    .bind(summary.helpful_votes as i64)
    .bind(summary.event_count as i64)
    .bind(evidence_refs_json)
    .bind(&summary.computed_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_content_workflow_audit_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    audit: &ContentWorkflowAuditRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO cms_content_workflow_audits (
            audit_id, content_id, action, from_status, to_status, actor_id, actor_role,
            occurred_at, scheduled_effective_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(&audit.audit_id)
    .bind(&audit.content_id)
    .bind(audit.action.as_str())
    .bind(audit.from_status.as_str())
    .bind(audit.to_status.as_str())
    .bind(&audit.actor_id)
    .bind(audit.actor_role.as_str())
    .bind(&audit.occurred_at)
    .bind(&audit.scheduled_effective_at)
    .execute(&mut **tx)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn insert_community_moderation_audit_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    audit: &ContentContributionModerationAuditRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO cms_community_contribution_audits (
            audit_id, contribution_id, action, from_status, to_status, moderator_id,
            occurred_at, reason
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&audit.audit_id)
    .bind(&audit.contribution_id)
    .bind(audit.action.as_str())
    .bind(audit.from_status.as_str())
    .bind(audit.to_status.as_str())
    .bind(&audit.moderator_id)
    .bind(&audit.occurred_at)
    .bind(&audit.reason)
    .execute(&mut **tx)
    .await
    .map_err(Error::from)?;

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

async fn load_success_story_record(
    state: &AppState,
    content_id: &str,
) -> AppResult<Option<ContentSuccessStoryRecord>> {
    let row = sqlx::query(
        r#"
        SELECT content_id, grower, crop, region, outcome_summary, metrics_json
        FROM cms_success_stories
        WHERE content_id = ?1
        "#,
    )
    .bind(content_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_success_story_record(&row)).transpose()
}

async fn load_community_contribution(
    state: &AppState,
    contribution_id: &str,
) -> AppResult<Option<ContentCommunityContributionRecord>> {
    let row = sqlx::query(
        r#"
        SELECT contribution_id, org_id, contributor_id, content_type, body, status,
               content_id, submitted_at, updated_at
        FROM cms_community_contributions
        WHERE contribution_id = ?1
        "#,
    )
    .bind(contribution_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_community_contribution_record(&row))
        .transpose()
}

async fn load_content_search_documents(
    state: &AppState,
    org_id: &str,
) -> AppResult<Vec<ContentSearchDocument>> {
    let rows = sqlx::query(
        r#"
        SELECT c.content_id, c.content_type, c.author_id, c.org_id, c.status,
               c.current_version, c.created_at, c.updated_at, v.body AS current_body
        FROM cms_contents c
        JOIN cms_content_versions v ON v.version_id = c.current_version
        WHERE c.org_id = ?1
        ORDER BY c.content_id ASC
        "#,
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| {
            let content = decode_content_record(&row)?;
            Ok(ContentSearchDocument {
                content,
                current_body: row.get("current_body"),
            })
        })
        .collect()
}

async fn load_content_portal_document(
    state: &AppState,
    content_id: &str,
) -> AppResult<Option<ContentSearchDocument>> {
    let row = sqlx::query(
        r#"
        SELECT c.content_id, c.content_type, c.author_id, c.org_id, c.status,
               c.current_version, c.created_at, c.updated_at, v.body AS current_body
        FROM cms_contents c
        JOIN cms_content_versions v ON v.version_id = c.current_version
        WHERE c.content_id = ?1
        "#,
    )
    .bind(content_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| {
        let content = decode_content_record(&row)?;
        Ok(ContentSearchDocument {
            content,
            current_body: row.get("current_body"),
        })
    })
    .transpose()
}

async fn load_content_engagement_events(
    state: &AppState,
    content_id: &str,
    org_id: &str,
    period: &str,
) -> AppResult<Vec<ContentEngagementEventRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT event_id, content_id, org_id, event_type, actor_id, period, occurred_at
        FROM cms_content_engagement_events
        WHERE content_id = ?1
          AND org_id = ?2
          AND period = ?3
        ORDER BY occurred_at ASC, event_id ASC
        "#,
    )
    .bind(content_id)
    .bind(org_id)
    .bind(period)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_content_engagement_event_record(&row))
        .collect()
}

async fn load_content_locale_variant(
    state: &AppState,
    content_id: &str,
    locale: &str,
) -> AppResult<Option<ContentLocaleVariantRecord>> {
    let normalized_locale = locale.trim().replace('_', "-").to_ascii_lowercase();
    let row = sqlx::query(
        r#"
        SELECT content_id, locale, version_id, body, status, created_at, updated_at
        FROM cms_content_locale_variants
        WHERE content_id = ?1
          AND locale = ?2
        "#,
    )
    .bind(content_id)
    .bind(normalized_locale)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_content_locale_variant_record(&row))
        .transpose()
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

async fn insert_collaboration_permission_audit(
    state: &AppState,
    decision: &CollaborationPermissionDecision,
    actor_id: &str,
    channel_id: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO collab_permission_audits (
            audit_id, org_id, actor_org_id, actor_id, action, permission,
            allowed, reason_code, channel_id, occurred_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(format!("collab-permission-audit-{}", Uuid::new_v4()))
    .bind(&decision.permissions.org_id)
    .bind(&decision.permissions.actor_org_id)
    .bind(actor_id)
    .bind(format!("{:?}", decision.action).to_ascii_lowercase())
    .bind(decision.action.permission_name())
    .bind(if decision.allowed { 1_i64 } else { 0_i64 })
    .bind(&decision.reason_code)
    .bind(channel_id)
    .bind(current_record_timestamp())
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn upsert_collaboration_presence(
    state: &AppState,
    record: &CollaborationPresenceRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO collab_presence (
            org_id, channel_id, account_id, state, last_seen, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(channel_id, account_id) DO UPDATE SET org_id = excluded.org_id,
                                                          state = excluded.state,
                                                          last_seen = excluded.last_seen,
                                                          updated_at = excluded.updated_at
        "#,
    )
    .bind(&record.org_id)
    .bind(&record.channel_id)
    .bind(&record.account_id)
    .bind(record.state.as_str())
    .bind(&record.last_seen)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn upsert_collaboration_presence_records(
    state: &AppState,
    records: &[CollaborationPresenceRecord],
) -> AppResult<()> {
    for record in records {
        upsert_collaboration_presence(state, record).await?;
    }
    Ok(())
}

async fn load_collaboration_presence_records(
    state: &AppState,
    channel_id: &str,
) -> AppResult<Vec<CollaborationPresenceRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT org_id, channel_id, account_id, state, last_seen, updated_at
        FROM collab_presence
        WHERE channel_id = ?1
        ORDER BY account_id ASC
        "#,
    )
    .bind(channel_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| {
            Ok(CollaborationPresenceRecord {
                org_id: row.get("org_id"),
                channel_id: row.get("channel_id"),
                account_id: row.get("account_id"),
                state: parse_collaboration_presence_state(&row.get::<String, _>("state"))?,
                last_seen: row.get("last_seen"),
                updated_at: row.get("updated_at"),
            })
        })
        .collect()
}

async fn insert_collaboration_notifications(
    state: &AppState,
    notifications: &[CollaborationNotificationRecord],
) -> AppResult<()> {
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    for notification in notifications {
        sqlx::query(
            r#"
            INSERT INTO collab_notifications (
                notification_id, event_id, org_id, channel_id, recipient_account_id,
                event_type, source_ref, body, delivery_state, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(&notification.notification_id)
        .bind(&notification.event_id)
        .bind(&notification.org_id)
        .bind(&notification.channel_id)
        .bind(&notification.recipient_account_id)
        .bind(&notification.event_type)
        .bind(&notification.source_ref)
        .bind(&notification.body)
        .bind(notification.delivery_state.as_str())
        .bind(&notification.created_at)
        .execute(&mut *tx)
        .await
        .map_err(Error::from)?;
    }
    tx.commit().await.map_err(Error::from)?;

    Ok(())
}

fn parse_collaboration_presence_state(value: &str) -> AppResult<CollaborationPresenceState> {
    match value {
        "online" => Ok(CollaborationPresenceState::Online),
        "away" => Ok(CollaborationPresenceState::Away),
        "offline" => Ok(CollaborationPresenceState::Offline),
        _ => Err(AppError::BadRequest(format!(
            "unsupported collaboration presence state {value}"
        ))),
    }
}

async fn persist_collaboration_session_replay(
    state: &AppState,
    replay: &CollaborationSessionReplay,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    sqlx::query(
        r#"
        INSERT INTO collab_sessions (
            session_id, org_id, created_at, event_count, has_explicit_gap
        )
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(&replay.session.session_id)
    .bind(&replay.session.org_id)
    .bind(&replay.session.created_at)
    .bind(replay.session.event_count as i64)
    .bind(if replay.session.has_explicit_gap {
        1_i64
    } else {
        0_i64
    })
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;

    for event in &replay.events {
        sqlx::query(
            r#"
            INSERT INTO collab_session_events (
                event_id, session_id, org_id, kind, occurred_at, actor_id, subject_ref, note
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(&event.event_id)
        .bind(&event.session_id)
        .bind(&event.org_id)
        .bind(event.kind.as_str())
        .bind(&event.occurred_at)
        .bind(&event.actor_id)
        .bind(&event.subject_ref)
        .bind(&event.note)
        .execute(&mut *tx)
        .await
        .map_err(Error::from)?;
    }

    tx.commit().await.map_err(Error::from)?;
    Ok(())
}

async fn load_collaboration_session_replay(
    state: &AppState,
    session_id: &str,
) -> AppResult<Option<CollaborationSessionReplay>> {
    let row = sqlx::query(
        r#"
        SELECT session_id, org_id, created_at, event_count, has_explicit_gap
        FROM collab_sessions
        WHERE session_id = ?1
        "#,
    )
    .bind(session_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let session = CollaborationSessionRecord {
        session_id: row.get("session_id"),
        org_id: row.get("org_id"),
        created_at: row.get("created_at"),
        event_count: row.get::<i64, _>("event_count") as u64,
        has_explicit_gap: row.get::<i64, _>("has_explicit_gap") != 0,
    };
    let rows = sqlx::query(
        r#"
        SELECT event_id, session_id, org_id, kind, occurred_at, actor_id, subject_ref, note
        FROM collab_session_events
        WHERE session_id = ?1
        ORDER BY occurred_at ASC, kind ASC, event_id ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;
    let events = rows
        .into_iter()
        .map(|row| {
            Ok(CollaborationSessionEventRecord {
                event_id: row.get("event_id"),
                session_id: row.get("session_id"),
                org_id: row.get("org_id"),
                kind: parse_collaboration_session_event_kind(&row.get::<String, _>("kind"))?,
                occurred_at: row.get("occurred_at"),
                actor_id: row.get("actor_id"),
                subject_ref: row.get("subject_ref"),
                note: row.get("note"),
            })
        })
        .collect::<AppResult<Vec<_>>>()?;

    Ok(Some(CollaborationSessionReplay { session, events }))
}

fn parse_collaboration_session_event_kind(value: &str) -> AppResult<CollaborationSessionEventKind> {
    match value {
        "stream_frame" => Ok(CollaborationSessionEventKind::StreamFrame),
        "stream_gap" => Ok(CollaborationSessionEventKind::StreamGap),
        "alert" => Ok(CollaborationSessionEventKind::Alert),
        "mission_edit" => Ok(CollaborationSessionEventKind::MissionEdit),
        "annotation" => Ok(CollaborationSessionEventKind::Annotation),
        _ => Err(AppError::BadRequest(format!(
            "unsupported collaboration session event kind {value}"
        ))),
    }
}

async fn persist_collaboration_session_annotation(
    state: &AppState,
    link: &CollaborationSessionAnnotationLinkRecord,
    annotation: &AnnotationRecord,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    sqlx::query(
        r#"
        INSERT INTO annotations (
            annotation_id, scene_id, field_id, author, crs, audit_id, label, note, severity,
            geometry_json, created_at, updated_at
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
    .bind(serde_json::to_string(&annotation.geometry).map_err(|err| AppError::Anyhow(err.into()))?)
    .bind(&annotation.created_at)
    .bind(&annotation.updated_at)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;

    sqlx::query(
        r#"
        INSERT INTO collab_session_annotations (
            link_id, session_id, org_id, scene_id, annotation_id, actor_id,
            visible, recoverable, occurred_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(&link.link_id)
    .bind(&link.session_id)
    .bind(&link.org_id)
    .bind(&link.scene_id)
    .bind(&link.annotation_id)
    .bind(&link.actor_id)
    .bind(if link.visible { 1_i64 } else { 0_i64 })
    .bind(if link.recoverable { 1_i64 } else { 0_i64 })
    .bind(&link.occurred_at)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;

    sqlx::query(
        r#"
        INSERT INTO collab_session_events (
            event_id, session_id, org_id, kind, occurred_at, actor_id, subject_ref, note
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(format!("{}:event", link.link_id))
    .bind(&link.session_id)
    .bind(&link.org_id)
    .bind(CollaborationSessionEventKind::Annotation.as_str())
    .bind(&link.occurred_at)
    .bind(&link.actor_id)
    .bind(format!("annotation:{}", link.annotation_id))
    .bind(if link.recoverable {
        "connection_lost_annotation_persisted"
    } else {
        "annotation_persisted"
    })
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;

    sqlx::query(
        r#"
        UPDATE collab_sessions
        SET event_count = event_count + 1,
            has_explicit_gap = CASE WHEN ?1 = 1 THEN 1 ELSE has_explicit_gap END
        WHERE session_id = ?2
        "#,
    )
    .bind(if link.recoverable { 1_i64 } else { 0_i64 })
    .bind(&link.session_id)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;

    tx.commit().await.map_err(Error::from)?;
    Ok(())
}

async fn load_collaboration_session_annotations(
    state: &AppState,
    session_id: &str,
    org_id: &str,
) -> AppResult<Vec<CollaborationSessionAnnotationRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT
            csa.link_id, csa.session_id, csa.org_id, csa.scene_id AS link_scene_id,
            csa.annotation_id AS link_annotation_id, csa.actor_id, csa.visible,
            csa.recoverable, csa.occurred_at,
            a.annotation_id, a.scene_id, a.field_id, a.author, a.crs, a.audit_id,
            a.label, a.note, a.severity, a.geometry_json, a.created_at, a.updated_at
        FROM collab_session_annotations csa
        INNER JOIN annotations a ON a.annotation_id = csa.annotation_id
        WHERE csa.session_id = ?1
          AND csa.org_id = ?2
          AND csa.visible = 1
        ORDER BY csa.occurred_at ASC, csa.link_id ASC
        "#,
    )
    .bind(session_id)
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| {
            Ok(CollaborationSessionAnnotationRecord {
                link: decode_collaboration_session_annotation_link(&row),
                annotation: decode_annotation_record(&row)?,
            })
        })
        .collect()
}

fn decode_collaboration_session_annotation_link(
    row: &sqlx::sqlite::SqliteRow,
) -> CollaborationSessionAnnotationLinkRecord {
    CollaborationSessionAnnotationLinkRecord {
        link_id: row.get("link_id"),
        session_id: row.get("session_id"),
        org_id: row.get("org_id"),
        scene_id: row.get("link_scene_id"),
        annotation_id: row.get("link_annotation_id"),
        actor_id: row.get("actor_id"),
        visible: row.get::<i64, _>("visible") != 0,
        recoverable: row.get::<i64, _>("recoverable") != 0,
        occurred_at: row.get("occurred_at"),
    }
}

async fn load_collaboration_operator_console_streams(
    state: &AppState,
    org_id: &str,
) -> AppResult<Vec<CollaborationLiveStreamRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT stream_id, org_id, mission_ref, source_ref, state, latency_budget_ms,
               started_at, updated_at, evidence_refs_json
        FROM collab_streams
        WHERE org_id = ?1
        ORDER BY updated_at DESC, stream_id ASC
        "#,
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_collaboration_stream(&row))
        .collect()
}

async fn load_collaboration_operator_console_active_alerts(
    state: &AppState,
    org_id: &str,
) -> AppResult<Vec<CollaborationEmergencyAlertRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT alert_id, org_id, channel_id, source, severity, trigger_ref,
               body, state, raised_at, updated_at
        FROM collab_emergency_alerts
        WHERE org_id = ?1
          AND state IN ('raised', 'acknowledged')
        ORDER BY updated_at DESC, alert_id ASC
        "#,
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_collaboration_emergency_alert(&row))
        .collect()
}

async fn upsert_collaboration_mission_plan(
    state: &AppState,
    plan: &CollaborationMissionPlanRecord,
) -> AppResult<()> {
    let waypoints_json =
        serde_json::to_string(&plan.waypoints).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO collab_mission_plans (
            plan_id, org_id, mission_ref, version, waypoints_json, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(plan_id) DO UPDATE SET org_id = excluded.org_id,
                                             mission_ref = excluded.mission_ref,
                                             version = excluded.version,
                                             waypoints_json = excluded.waypoints_json,
                                             updated_at = excluded.updated_at
        "#,
    )
    .bind(&plan.plan_id)
    .bind(&plan.org_id)
    .bind(&plan.mission_ref)
    .bind(plan.version as i64)
    .bind(waypoints_json)
    .bind(&plan.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    Ok(())
}

async fn persist_collaboration_mission_edit_result(
    state: &AppState,
    result: &CollaborationMissionEditResult,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    if result.audit.decision == CollaborationMissionEditDecision::Accepted {
        let waypoints_json = serde_json::to_string(&result.plan.waypoints)
            .map_err(|err| AppError::Anyhow(err.into()))?;
        sqlx::query(
            r#"
            UPDATE collab_mission_plans
            SET version = ?1, waypoints_json = ?2, updated_at = ?3
            WHERE plan_id = ?4
            "#,
        )
        .bind(result.plan.version as i64)
        .bind(waypoints_json)
        .bind(&result.plan.updated_at)
        .bind(&result.plan.plan_id)
        .execute(&mut *tx)
        .await
        .map_err(Error::from)?;
    }
    insert_collaboration_mission_edit_audit_query(&mut tx, &result.audit).await?;
    tx.commit().await.map_err(Error::from)?;
    Ok(())
}

async fn insert_collaboration_mission_edit_audit_query(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    audit: &CollaborationMissionEditAuditRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO collab_mission_edit_audits (
            audit_id, plan_id, mission_ref, actor_id, waypoint_id, base_version,
            resulting_version, decision, reason_code, occurred_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(&audit.audit_id)
    .bind(&audit.plan_id)
    .bind(&audit.mission_ref)
    .bind(&audit.actor_id)
    .bind(&audit.waypoint_id)
    .bind(audit.base_version as i64)
    .bind(audit.resulting_version as i64)
    .bind(audit.decision.as_str())
    .bind(&audit.reason_code)
    .bind(&audit.occurred_at)
    .execute(&mut **tx)
    .await
    .map_err(Error::from)?;
    Ok(())
}

async fn insert_collaboration_mission_dispatch_audit(
    state: &AppState,
    audit: &CollaborationMissionDispatchAuditRecord,
) -> AppResult<()> {
    let blocking_guardrails_json = serde_json::to_string(&audit.blocking_guardrails)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO collab_mission_dispatch_audits (
            audit_id, plan_id, mission_ref, actor_id, version, allowed,
            blocking_guardrails_json, occurred_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&audit.audit_id)
    .bind(&audit.plan_id)
    .bind(&audit.mission_ref)
    .bind(&audit.actor_id)
    .bind(audit.version as i64)
    .bind(if audit.allowed { 1_i64 } else { 0_i64 })
    .bind(blocking_guardrails_json)
    .bind(&audit.occurred_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    Ok(())
}

async fn load_collaboration_mission_plan(
    state: &AppState,
    plan_id: &str,
) -> AppResult<Option<CollaborationMissionPlanRecord>> {
    let row = sqlx::query(
        r#"
        SELECT plan_id, org_id, mission_ref, version, waypoints_json, updated_at
        FROM collab_mission_plans
        WHERE plan_id = ?1
        "#,
    )
    .bind(plan_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;
    row.map(|row| decode_collaboration_mission_plan(&row))
        .transpose()
}

fn decode_collaboration_mission_plan(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<CollaborationMissionPlanRecord> {
    let waypoints = serde_json::from_str::<Vec<CollaborationMissionWaypoint>>(
        &row.get::<String, _>("waypoints_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode collab mission waypoints_json"))
    })?;
    Ok(CollaborationMissionPlanRecord {
        plan_id: row.get("plan_id"),
        org_id: row.get("org_id"),
        mission_ref: row.get("mission_ref"),
        version: row.get::<i64, _>("version") as u64,
        waypoints,
        updated_at: row.get("updated_at"),
    })
}

async fn persist_collaboration_emergency_alert_raise(
    state: &AppState,
    result: &CollaborationEmergencyAlertRaiseResult,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    insert_collaboration_emergency_alert_query(&mut tx, &result.alert).await?;
    for delivery in &result.deliveries {
        sqlx::query(
            r#"
            INSERT INTO collab_alert_deliveries (
                delivery_id, alert_id, org_id, channel_id, recipient_account_id,
                delivery_state, retry_count, last_attempt_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(&delivery.delivery_id)
        .bind(&delivery.alert_id)
        .bind(&delivery.org_id)
        .bind(&delivery.channel_id)
        .bind(&delivery.recipient_account_id)
        .bind(delivery.delivery_state.as_str())
        .bind(delivery.retry_count as i64)
        .bind(&delivery.last_attempt_at)
        .execute(&mut *tx)
        .await
        .map_err(Error::from)?;
    }
    insert_collaboration_emergency_alert_audit_query(&mut tx, &result.audit).await?;
    tx.commit().await.map_err(Error::from)?;
    Ok(())
}

async fn persist_collaboration_emergency_alert_transition(
    state: &AppState,
    result: &CollaborationEmergencyAlertTransitionResult,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    sqlx::query(
        r#"
        UPDATE collab_emergency_alerts
        SET state = ?1, updated_at = ?2
        WHERE alert_id = ?3
        "#,
    )
    .bind(result.alert.state.as_str())
    .bind(&result.alert.updated_at)
    .bind(&result.alert.alert_id)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;
    insert_collaboration_emergency_alert_audit_query(&mut tx, &result.audit).await?;
    tx.commit().await.map_err(Error::from)?;
    Ok(())
}

async fn insert_collaboration_emergency_alert_query(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    alert: &CollaborationEmergencyAlertRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO collab_emergency_alerts (
            alert_id, org_id, channel_id, source, severity, trigger_ref,
            body, state, raised_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(&alert.alert_id)
    .bind(&alert.org_id)
    .bind(&alert.channel_id)
    .bind(alert.source.as_str())
    .bind(&alert.severity)
    .bind(&alert.trigger_ref)
    .bind(&alert.body)
    .bind(alert.state.as_str())
    .bind(&alert.raised_at)
    .bind(&alert.updated_at)
    .execute(&mut **tx)
    .await
    .map_err(Error::from)?;
    Ok(())
}

async fn insert_collaboration_emergency_alert_audit_query(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    audit: &CollaborationEmergencyAlertAuditRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO collab_alert_audits (
            audit_id, alert_id, action, actor_id, from_state, to_state, occurred_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&audit.audit_id)
    .bind(&audit.alert_id)
    .bind(&audit.action)
    .bind(&audit.actor_id)
    .bind(audit.from_state.as_str())
    .bind(audit.to_state.as_str())
    .bind(&audit.occurred_at)
    .execute(&mut **tx)
    .await
    .map_err(Error::from)?;
    Ok(())
}

async fn load_collaboration_emergency_alert(
    state: &AppState,
    alert_id: &str,
) -> AppResult<Option<CollaborationEmergencyAlertRecord>> {
    let row = sqlx::query(
        r#"
        SELECT alert_id, org_id, channel_id, source, severity, trigger_ref,
               body, state, raised_at, updated_at
        FROM collab_emergency_alerts
        WHERE alert_id = ?1
        "#,
    )
    .bind(alert_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_collaboration_emergency_alert(&row))
        .transpose()
}

fn decode_collaboration_emergency_alert(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<CollaborationEmergencyAlertRecord> {
    Ok(CollaborationEmergencyAlertRecord {
        alert_id: row.get("alert_id"),
        org_id: row.get("org_id"),
        channel_id: row.get("channel_id"),
        source: parse_collaboration_emergency_alert_source(&row.get::<String, _>("source"))?,
        severity: row.get("severity"),
        trigger_ref: row.get("trigger_ref"),
        body: row.get("body"),
        state: parse_collaboration_emergency_alert_state(&row.get::<String, _>("state"))?,
        raised_at: row.get("raised_at"),
        updated_at: row.get("updated_at"),
    })
}

fn parse_collaboration_emergency_alert_source(
    value: &str,
) -> AppResult<CollaborationEmergencyAlertSource> {
    match value {
        "01" => Ok(CollaborationEmergencyAlertSource::Safety01),
        "12" => Ok(CollaborationEmergencyAlertSource::Fleet12),
        _ => Err(AppError::BadRequest(format!(
            "unsupported collaboration emergency alert source {value}"
        ))),
    }
}

fn parse_collaboration_emergency_alert_state(
    value: &str,
) -> AppResult<CollaborationEmergencyAlertState> {
    match value {
        "raised" => Ok(CollaborationEmergencyAlertState::Raised),
        "acknowledged" => Ok(CollaborationEmergencyAlertState::Acknowledged),
        "resolved" => Ok(CollaborationEmergencyAlertState::Resolved),
        _ => Err(AppError::BadRequest(format!(
            "unsupported collaboration emergency alert state {value}"
        ))),
    }
}

async fn insert_collaboration_stream(
    state: &AppState,
    stream: &CollaborationLiveStreamRecord,
) -> AppResult<()> {
    let evidence_refs_json =
        serde_json::to_string(&stream.evidence_refs).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO collab_streams (
            stream_id, org_id, mission_ref, source_ref, state, latency_budget_ms,
            started_at, updated_at, evidence_refs_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(&stream.stream_id)
    .bind(&stream.org_id)
    .bind(&stream.mission_ref)
    .bind(&stream.source_ref)
    .bind(stream.state.as_str())
    .bind(stream.latency_budget_ms as i64)
    .bind(&stream.started_at)
    .bind(&stream.updated_at)
    .bind(evidence_refs_json)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn persist_collaboration_stream_relay_result(
    state: &AppState,
    result: &CollaborationStreamRelayResult,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    sqlx::query(
        r#"
        UPDATE collab_streams
        SET state = ?1, updated_at = ?2
        WHERE stream_id = ?3
        "#,
    )
    .bind(result.stream.state.as_str())
    .bind(&result.stream.updated_at)
    .bind(&result.stream.stream_id)
    .execute(&mut *tx)
    .await
    .map_err(Error::from)?;

    if let Some(frame) = &result.frame {
        sqlx::query(
            r#"
            INSERT INTO collab_stream_frames (
                frame_id, stream_id, org_id, sequence, captured_at, relayed_at,
                latency_ms, payload_ref, encoded_ref, relay_ref, view_ref, dropped
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
        )
        .bind(&frame.frame_id)
        .bind(&frame.stream_id)
        .bind(&frame.org_id)
        .bind(frame.sequence as i64)
        .bind(&frame.captured_at)
        .bind(&frame.relayed_at)
        .bind(frame.latency_ms as i64)
        .bind(&frame.payload_ref)
        .bind(&frame.encoded_ref)
        .bind(&frame.relay_ref)
        .bind(&frame.view_ref)
        .bind(if frame.dropped { 1_i64 } else { 0_i64 })
        .execute(&mut *tx)
        .await
        .map_err(Error::from)?;
    }

    tx.commit().await.map_err(Error::from)?;
    Ok(())
}

async fn load_collaboration_stream(
    state: &AppState,
    stream_id: &str,
) -> AppResult<Option<CollaborationLiveStreamRecord>> {
    let row = sqlx::query(
        r#"
        SELECT stream_id, org_id, mission_ref, source_ref, state, latency_budget_ms,
               started_at, updated_at, evidence_refs_json
        FROM collab_streams
        WHERE stream_id = ?1
        "#,
    )
    .bind(stream_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_collaboration_stream(&row)).transpose()
}

async fn load_collaboration_stream_frames(
    state: &AppState,
    stream_id: &str,
) -> AppResult<Vec<CollaborationStreamFrameRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT frame_id, stream_id, org_id, sequence, captured_at, relayed_at,
               latency_ms, payload_ref, encoded_ref, relay_ref, view_ref, dropped
        FROM collab_stream_frames
        WHERE stream_id = ?1
        ORDER BY sequence ASC
        "#,
    )
    .bind(stream_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_collaboration_stream_frame(&row))
        .collect()
}

async fn next_collaboration_stream_sequence(state: &AppState, stream_id: &str) -> AppResult<u64> {
    let max_sequence: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT MAX(sequence)
        FROM collab_stream_frames
        WHERE stream_id = ?1
        "#,
    )
    .bind(stream_id)
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;
    Ok(max_sequence.unwrap_or(0) as u64 + 1)
}

fn decode_collaboration_stream(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<CollaborationLiveStreamRecord> {
    let evidence_refs = serde_json::from_str::<Vec<String>>(
        &row.get::<String, _>("evidence_refs_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode stream evidence_refs_json"))
    })?;
    Ok(CollaborationLiveStreamRecord {
        stream_id: row.get("stream_id"),
        org_id: row.get("org_id"),
        mission_ref: row.get("mission_ref"),
        source_ref: row.get("source_ref"),
        state: parse_collaboration_stream_state(&row.get::<String, _>("state"))?,
        latency_budget_ms: row.get::<i64, _>("latency_budget_ms") as u64,
        started_at: row.get("started_at"),
        updated_at: row.get("updated_at"),
        evidence_refs,
    })
}

fn decode_collaboration_stream_frame(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<CollaborationStreamFrameRecord> {
    Ok(CollaborationStreamFrameRecord {
        frame_id: row.get("frame_id"),
        stream_id: row.get("stream_id"),
        org_id: row.get("org_id"),
        sequence: row.get::<i64, _>("sequence") as u64,
        captured_at: row.get("captured_at"),
        relayed_at: row.get("relayed_at"),
        latency_ms: row.get::<i64, _>("latency_ms") as u64,
        payload_ref: row.get("payload_ref"),
        encoded_ref: row.get("encoded_ref"),
        relay_ref: row.get("relay_ref"),
        view_ref: row.get("view_ref"),
        dropped: row.get::<i64, _>("dropped") != 0,
    })
}

fn parse_collaboration_stream_state(value: &str) -> AppResult<CollaborationStreamState> {
    match value {
        "starting" => Ok(CollaborationStreamState::Starting),
        "live" => Ok(CollaborationStreamState::Live),
        "reconnecting" => Ok(CollaborationStreamState::Reconnecting),
        "ended" => Ok(CollaborationStreamState::Ended),
        _ => Err(AppError::BadRequest(format!(
            "unsupported collaboration stream state {value}"
        ))),
    }
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

async fn insert_crop_inference_progress(
    state: &AppState,
    progress: &InferenceRunProgressRecord,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO crop_inference_progress (
            progress_id, run_id, tiles_total, tiles_done, coverage_fraction, stage, observed_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&progress.progress_id)
    .bind(&progress.run_id)
    .bind(progress.tiles_total as i64)
    .bind(progress.tiles_done as i64)
    .bind(progress.coverage_fraction)
    .bind(&progress.stage)
    .bind(&progress.observed_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    Ok(())
}

async fn load_crop_inference_progress(
    state: &AppState,
    run_id: &str,
) -> AppResult<Vec<InferenceRunProgressRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT progress_id, run_id, tiles_total, tiles_done, coverage_fraction, stage, observed_at
        FROM crop_inference_progress
        WHERE run_id = ?1
        ORDER BY observed_at ASC, progress_id ASC
        "#,
    )
    .bind(run_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(rows
        .into_iter()
        .map(|row| InferenceRunProgressRecord {
            progress_id: row.get("progress_id"),
            run_id: row.get("run_id"),
            tiles_total: row.get::<i64, _>("tiles_total") as u64,
            tiles_done: row.get::<i64, _>("tiles_done") as u64,
            coverage_fraction: row.get("coverage_fraction"),
            stage: row.get("stage"),
            observed_at: row.get("observed_at"),
        })
        .collect())
}

async fn insert_crop_inference_stall_event(
    state: &AppState,
    event: &InferenceRunStallEvent,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO crop_inference_stall_events (
            stall_id, run_id, last_progress_at, detected_at, stall_window_seconds, flagged
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&event.stall_id)
    .bind(&event.run_id)
    .bind(&event.last_progress_at)
    .bind(&event.detected_at)
    .bind(event.stall_window_seconds as i64)
    .bind(if event.flagged { 1_i64 } else { 0_i64 })
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

async fn persist_crop_closed_loop_proposal(
    state: &AppState,
    proposal: &CropClosedLoopProposal,
) -> AppResult<()> {
    let action_json =
        serde_json::to_string(&proposal.action).map_err(|err| AppError::Anyhow(err.into()))?;
    let refly_area_json = proposal
        .refly_area
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let findings_json =
        serde_json::to_string(&proposal.findings).map_err(|err| AppError::Anyhow(err.into()))?;
    let evidence_refs_json = serde_json::to_string(&proposal.evidence_refs)
        .map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO crop_closed_loop_proposals (
            proposal_id, action, field_id, requested_by, created_at, approval_status,
            approval_required, dispatch_authorized, confidence_floor, refly_area_json,
            treatment_prescription_ref, findings_json, evidence_refs_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        "#,
    )
    .bind(&proposal.proposal_id)
    .bind(action_json)
    .bind(&proposal.field_id)
    .bind(&proposal.requested_by)
    .bind(&proposal.created_at)
    .bind(proposal.approval_status.as_str())
    .bind(proposal.approval_required)
    .bind(proposal.dispatch_authorized)
    .bind(proposal.confidence_floor)
    .bind(refly_area_json)
    .bind(&proposal.treatment_prescription_ref)
    .bind(findings_json)
    .bind(evidence_refs_json)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_crop_closed_loop_proposal(
    state: &AppState,
    proposal_id: &str,
) -> AppResult<Option<CropClosedLoopProposal>> {
    let row = sqlx::query(
        r#"
        SELECT proposal_id, action, field_id, requested_by, created_at, approval_status,
               approval_required, dispatch_authorized, confidence_floor, refly_area_json,
               treatment_prescription_ref, findings_json, evidence_refs_json
        FROM crop_closed_loop_proposals
        WHERE proposal_id = ?1
        "#,
    )
    .bind(proposal_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_crop_closed_loop_proposal(&row))
        .transpose()
}

fn decode_crop_closed_loop_proposal(
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<CropClosedLoopProposal> {
    let action = serde_json::from_str::<CropClosedLoopAction>(&row.get::<String, _>("action"))
        .map_err(|err| {
            AppError::Anyhow(
                Error::new(err).context("failed to decode crop closed-loop proposal action"),
            )
        })?;
    let approval_status = {
        let raw = row.get::<String, _>("approval_status");
        CropClosedLoopApprovalStatus::parse(&raw).ok_or_else(|| {
            AppError::Anyhow(Error::msg(format!(
                "unknown crop closed-loop approval status {raw}"
            )))
        })?
    };
    let refly_area = row
        .get::<Option<String>, _>("refly_area_json")
        .map(|json| serde_json::from_str::<DetectionZoneGeometry>(&json))
        .transpose()
        .map_err(|err| {
            AppError::Anyhow(
                Error::new(err).context("failed to decode crop closed-loop refly_area_json"),
            )
        })?;
    let findings = serde_json::from_str::<Vec<CropClosedLoopFindingEvidence>>(
        &row.get::<String, _>("findings_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(Error::new(err).context("failed to decode crop closed-loop findings_json"))
    })?;
    let evidence_refs = serde_json::from_str::<Vec<String>>(
        &row.get::<String, _>("evidence_refs_json"),
    )
    .map_err(|err| {
        AppError::Anyhow(
            Error::new(err).context("failed to decode crop closed-loop evidence_refs_json"),
        )
    })?;

    Ok(CropClosedLoopProposal {
        proposal_id: row.get("proposal_id"),
        action,
        field_id: row.get("field_id"),
        requested_by: row.get("requested_by"),
        created_at: row.get("created_at"),
        approval_status,
        approval_required: row.get("approval_required"),
        dispatch_authorized: row.get("dispatch_authorized"),
        confidence_floor: row.get("confidence_floor"),
        refly_area,
        treatment_prescription_ref: row.get("treatment_prescription_ref"),
        findings,
        evidence_refs,
    })
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

async fn build_compliance_authority_export_from_api(
    state: &AppState,
    request: ComplianceAuthorityExportApiRequest,
) -> AppResult<ComplianceAuthorityExportArtifact> {
    let records =
        load_compliance_records_for_report(state, &request.org_id, &request.field_id).await?;
    let mandatory_record_types = if request.mandatory_record_types.is_empty() {
        default_compliance_report_mandatory_types()
    } else {
        request.mandatory_record_types
    };
    let generated_at = request
        .generated_at
        .unwrap_or_else(current_record_timestamp);
    let report = build_compliance_audit_report(ComplianceAuditReportRequest {
        report_id: request
            .report_id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("compliance-report-{}", Uuid::new_v4())),
        org_id: request.org_id,
        field_id: request.field_id,
        generated_at: generated_at.clone(),
        records,
        mandatory_record_types,
    })
    .map_err(compliance_audit_report_error)?;

    build_compliance_authority_export(ComplianceAuthorityExportRequest {
        authority_format: request.authority_format,
        report,
        generated_at,
        residency_tag: request.residency_tag,
        storage_region: request.storage_region,
        retention_class: request.retention_class,
    })
    .map_err(compliance_authority_export_error)
}

fn compliance_authority_export_id(export: &ComplianceAuthorityExportArtifact) -> String {
    format!("{}:{}", export.report_id, export.authority_format.as_str())
}

async fn persist_compliance_authority_export(
    state: &AppState,
    export: &ComplianceAuthorityExportArtifact,
) -> AppResult<()> {
    let export_json = serde_json::to_string(export).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO compliance_authority_exports (
            export_id, report_id, authority_format, org_id, field_id, generated_at,
            residency_tag, storage_region, retention_class, export_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(export_id) DO UPDATE SET
            generated_at = excluded.generated_at,
            residency_tag = excluded.residency_tag,
            storage_region = excluded.storage_region,
            retention_class = excluded.retention_class,
            export_json = excluded.export_json
        "#,
    )
    .bind(compliance_authority_export_id(export))
    .bind(&export.report_id)
    .bind(export.authority_format.as_str())
    .bind(&export.org_id)
    .bind(&export.field_id)
    .bind(&export.generated_at)
    .bind(&export.residency_tag)
    .bind(&export.storage_region)
    .bind(format!("{:?}", export.retention_class))
    .bind(export_json)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn persist_compliance_authority_share(
    state: &AppState,
    share: &ComplianceAuthorityShareArtifact,
) -> AppResult<()> {
    let share_json = serde_json::to_string(share).map_err(|err| AppError::Anyhow(err.into()))?;
    sqlx::query(
        r#"
        INSERT INTO compliance_authority_shares (
            share_id, export_id, report_id, authority_format, created_at, expires_at, revoked_at,
            residency_tag, storage_region, retention_class, share_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(share_id) DO UPDATE SET
            revoked_at = excluded.revoked_at,
            share_json = excluded.share_json
        "#,
    )
    .bind(&share.share_id)
    .bind(compliance_authority_export_id(&share.export))
    .bind(&share.report_id)
    .bind(share.authority_format.as_str())
    .bind(&share.created_at)
    .bind(&share.expires_at)
    .bind(&share.revoked_at)
    .bind(&share.residency_tag)
    .bind(&share.storage_region)
    .bind(format!("{:?}", share.retention_class))
    .bind(share_json)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
}

async fn load_compliance_authority_share(
    state: &AppState,
    share_id: &str,
) -> AppResult<Option<ComplianceAuthorityShareArtifact>> {
    let share_json = sqlx::query_scalar::<_, String>(
        "SELECT share_json FROM compliance_authority_shares WHERE share_id = ?1",
    )
    .bind(share_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    share_json
        .map(|json| {
            serde_json::from_str::<ComplianceAuthorityShareArtifact>(&json).map_err(|err| {
                AppError::Anyhow(
                    Error::new(err).context("failed to decode compliance authority share_json"),
                )
            })
        })
        .transpose()
}

async fn audit_compliance_authority_share_event(
    state: &AppState,
    share: &ComplianceAuthorityShareArtifact,
    event_type: &str,
    actor: Option<&str>,
    created_at: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO compliance_authority_share_events (
            share_id, event_type, actor, created_at, details
        )
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(&share.share_id)
    .bind(event_type)
    .bind(actor)
    .bind(created_at.unwrap_or(share.created_at.as_str()))
    .bind(format!(
        "{} share for report {}",
        share.authority_format.as_str(),
        share.report_id
    ))
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(())
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

#[allow(clippy::too_many_arguments)]
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
