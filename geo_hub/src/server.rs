use crate::pipeline_worker::{
    spawn_pipeline_worker, EarthSearchItemFetcher, PipelineWorkerContext,
};
use crate::satellite_derivation::UrlCogResolver;
use crate::{config::HubConfig, routes, state::AppState};
use anyhow::Result;
use axum::{
    extract::DefaultBodyLimit,
    middleware::from_fn_with_state,
    routing::{delete, get, patch, post, put},
    Router,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tower_http::services::ServeDir;
use tracing::{info, warn};

async fn health_handler() -> &'static str {
    "ok"
}

async fn ready_handler() -> &'static str {
    "ready"
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .nest_service(
            "/workspace",
            ServeDir::new(state.config.workspace_web_dir()),
        )
        .nest_service(
            "/portal",
            ServeDir::new(state.config.workspace_web_dir().join("portal")),
        )
        .route("/", get(routes::mobile_app))
        .route("/app", get(routes::mobile_app))
        .route("/browse", get(routes::browse_app))
        .route("/browse/app.js", get(routes::browse_app_js))
        .route("/browse/style.css", get(routes::browse_style_css))
        .route(
            "/api/mobile/scenes/search",
            post(routes::mobile_search_scenes),
        )
        .route("/api/mobile/analyze", post(routes::mobile_analyze))
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/api/ingest/health", get(routes::get_ingest_health))
        .route(
            "/api/ingest/drone-session",
            post(routes::ingest_drone_session),
        )
        .route("/api/ingest/hls/register", post(routes::register_hls))
        .route(
            "/api/ingest/sen2cor/ndvi/derive",
            post(routes::derive_sen2cor_index_route),
        )
        .route(
            "/api/ingest/sen2cor/index/derive",
            post(routes::derive_sen2cor_index_route),
        )
        .route(
            "/api/ingest/landsat/derive",
            post(routes::derive_landsat_product_route),
        )
        .route("/api/catalog/sources", get(routes::list_catalog_sources))
        .route(
            "/api/catalog/products",
            get(routes::list_catalog_products).post(routes::register_catalog_product),
        )
        .route(
            "/api/catalog/products/:product_id",
            get(routes::get_catalog_product),
        )
        .route(
            "/api/catalog/products/:product_id/tiles/:z/:x/:y.png",
            get(routes::catalog_product_web_tile),
        )
        .route("/api/satellite/derive", post(routes::satellite_derive))
        .route(
            "/api/fields/:field_id/modis/ingest",
            post(routes::ingest_field_modis),
        )
        .route(
            "/api/fields/:field_id/timeseries",
            get(routes::get_field_timeseries),
        )
        .route(
            "/api/fields/:field_id/timeseries/metrics",
            get(routes::get_field_timeseries_metrics),
        )
        .route(
            "/api/fields/:field_id/timeseries/summary",
            get(routes::get_field_timeseries_summary),
        )
        .route("/api/stac", get(routes::stac_landing_page))
        .route("/api/stac/conformance", get(routes::stac_conformance))
        .route("/api/stac/collections", get(routes::stac_list_collections))
        .route(
            "/api/stac/collections/:collection_id",
            get(routes::stac_get_collection),
        )
        .route(
            "/api/stac/collections/:collection_id/items",
            get(routes::stac_list_collection_items),
        )
        .route(
            "/api/stac/collections/:collection_id/items/:item_id",
            get(routes::stac_get_collection_item),
        )
        .route(
            "/api/stac/search",
            get(routes::stac_search_get).post(routes::stac_search_post),
        )
        .route(
            "/api/applications/:app_id/runs",
            post(routes::create_application_run),
        )
        .route(
            "/api/application-runs/:run_id",
            get(routes::get_application_run),
        )
        .route(
            "/api/fields/:field_id/findings",
            get(routes::list_field_findings),
        )
        .route(
            "/api/applications/crop-health/runs",
            post(routes::run_crop_health_app),
        )
        .route(
            "/api/applications/water-priority/runs",
            post(routes::run_water_priority_app),
        )
        .route(
            "/api/applications/anomaly/runs",
            post(routes::run_anomaly_app),
        )
        .route(
            "/api/applications/drought-watch/runs",
            post(routes::run_drought_watch_app),
        )
        .route(
            "/api/applications/water-balance-watch/runs",
            post(routes::run_water_balance_watch_app),
        )
        .route(
            "/api/fields/:field_id/alert-evaluation",
            post(routes::evaluate_field_alerts),
        )
        .route(
            "/api/fields/:field_id/alerts",
            get(routes::list_field_alerts),
        )
        .route(
            "/api/alerts/:alert_id/severity",
            get(routes::get_alert_severity_classification),
        )
        .route(
            "/api/alerts/:alert_id/lifecycle",
            get(routes::get_alert_lifecycle),
        )
        .route(
            "/api/alerts/:alert_id/acknowledge",
            post(routes::acknowledge_alert),
        )
        .route("/api/alerts/:alert_id/resolve", post(routes::resolve_alert))
        .route("/api/proposals", post(routes::create_proposal))
        .route(
            "/api/fields/:field_id/proposals",
            get(routes::list_field_proposals),
        )
        .route("/api/proposals/:proposal_id", get(routes::get_proposal))
        .route(
            "/api/proposals/:proposal_id/accept",
            post(routes::accept_proposal),
        )
        .route(
            "/api/proposals/:proposal_id/reject",
            post(routes::reject_proposal),
        )
        .route(
            "/api/farms",
            get(routes::list_farms).post(routes::create_farm),
        )
        .route(
            "/api/farms/:farm_id",
            get(routes::get_farm)
                .put(routes::update_farm)
                .delete(routes::delete_farm),
        )
        .route("/api/farms/:farm_id/fields", get(routes::list_farm_fields))
        .route(
            "/api/farms/:farm_id/fields/history",
            get(routes::list_farm_field_history),
        )
        .route(
            "/api/fields",
            get(routes::list_fields).post(routes::create_field),
        )
        .route(
            "/api/fleet/nodes",
            get(routes::list_fleet_nodes).post(routes::enroll_fleet_node),
        )
        .route("/api/fleet/nodes/enroll", post(routes::enroll_fleet_node))
        .route("/api/fleet/nodes/:node_id", get(routes::get_fleet_node))
        .route(
            "/api/tractors",
            get(routes::list_tractors).post(routes::register_tractor),
        )
        .route("/api/tractors/:tractor_id", get(routes::get_tractor))
        .route(
            "/api/tractors/:tractor_id/fleet-health",
            post(routes::ingest_tractor_fleet_health),
        )
        .route(
            "/api/tractors/:tractor_id/motion-commands/validate",
            post(routes::validate_tractor_motion_command),
        )
        .route(
            "/api/weather/forecasts",
            get(routes::list_weather_forecasts),
        )
        .route(
            "/api/weather/forecasts/pull",
            post(routes::pull_weather_forecast),
        )
        .route(
            "/api/weather/fetch-failures",
            get(routes::list_weather_fetch_failures),
        )
        .route(
            "/api/orthomosaic/frame-sets",
            get(routes::list_orthomosaic_frame_sets).post(routes::ingest_orthomosaic_frame_set),
        )
        .route(
            "/api/orthomosaic/reconstructions",
            post(routes::submit_orthomosaic_reconstruction),
        )
        .route(
            "/api/orthomosaic/reconstructions/:recon_id",
            get(routes::get_orthomosaic_reconstruction),
        )
        .route(
            "/api/orthomosaic/reconstructions/:recon_id/status",
            put(routes::update_orthomosaic_reconstruction_status),
        )
        .route(
            "/api/orthomosaic/reconstructions/:recon_id/handoff",
            post(routes::handoff_orthomosaic_tiles),
        )
        .route(
            "/api/orthomosaic/products/:scene_id/:kind/publish-gate",
            post(routes::apply_orthomosaic_publish_gate),
        )
        .route(
            "/api/copilot/conversations",
            get(routes::list_copilot_conversations)
                .post(routes::start_copilot_conversation_handler),
        )
        .route(
            "/api/copilot/conversations/:conversation_id/turns",
            get(routes::list_copilot_turns).post(routes::create_copilot_turn_handler),
        )
        .route(
            "/api/crop-intelligence/models",
            get(routes::list_crop_models).post(routes::register_crop_model),
        )
        .route(
            "/api/crop-intelligence/inference-runs",
            post(routes::submit_crop_inference_run),
        )
        .route(
            "/api/crop-intelligence/inference-runs/:run_id",
            get(routes::get_crop_inference_run),
        )
        .route(
            "/api/crop-intelligence/inference-runs/:run_id/status",
            put(routes::update_crop_inference_run_status),
        )
        .route(
            "/api/crop-intelligence/inference-runs/:run_id/progress",
            get(routes::list_crop_inference_run_progress)
                .post(routes::record_crop_inference_run_progress),
        )
        .route(
            "/api/crop-intelligence/inference-runs/:run_id/stall-check",
            post(routes::check_crop_inference_run_stall),
        )
        .route(
            "/api/crop-intelligence/inference-runs/:run_id/result",
            get(routes::get_crop_inference_run_result),
        )
        .route(
            "/api/crop-intelligence/detections/:detection_id/verification",
            post(routes::verify_crop_detection),
        )
        .route(
            "/api/crop-intelligence/detections/:detection_id/finding-promotion/validate",
            post(routes::validate_crop_detection_finding_promotion),
        )
        .route(
            "/api/scenes/:scene_id/crop-intelligence/detections/:detection_id/findings",
            post(routes::emit_crop_detection_finding),
        )
        .route(
            "/api/crop-intelligence/closed-loop-proposals",
            post(routes::create_crop_closed_loop_proposal),
        )
        .route(
            "/api/crop-intelligence/closed-loop-proposals/:proposal_id",
            get(routes::get_crop_closed_loop_proposal),
        )
        .route(
            "/api/crop-intelligence/inference-requests/validate",
            post(routes::validate_crop_model_for_inference),
        )
        .route(
            "/api/compliance/records",
            get(routes::list_compliance_records).post(routes::create_compliance_record),
        )
        .route(
            "/api/compliance/records/:record_id",
            delete(routes::refuse_delete_compliance_record),
        )
        .route(
            "/api/compliance/reports/export",
            post(routes::export_compliance_audit_report),
        )
        .route(
            "/api/compliance/reports/authority-export",
            post(routes::export_compliance_authority_report),
        )
        .route(
            "/api/compliance/reports/authority-shares",
            post(routes::create_compliance_authority_share),
        )
        .route(
            "/api/compliance/authority-shares/:share_id",
            get(routes::get_compliance_authority_share),
        )
        .route(
            "/api/compliance/authority-shares/:share_id/revoke",
            post(routes::revoke_compliance_authority_share_route),
        )
        .route(
            "/api/compliance/regulation-assist",
            post(routes::run_compliance_regulation_assist),
        )
        .route(
            "/api/compliance/records/:record_id/versions",
            post(routes::append_compliance_record_version_route),
        )
        .route(
            "/api/compliance/airspace-zones",
            get(routes::list_airspace_zones).post(routes::ingest_airspace_zone),
        )
        .route(
            "/api/compliance/airspace-zones/query",
            get(routes::query_airspace_zones_for_point),
        )
        .route(
            "/api/fleet-health/components",
            get(routes::list_fleet_components).post(routes::register_fleet_component),
        )
        .route(
            "/api/fleet-health/components/:component_id/history",
            get(routes::get_fleet_component_history),
        )
        .route(
            "/api/fleet-health/components/:component_id/install",
            post(routes::install_fleet_component_route),
        )
        .route(
            "/api/fleet-health/duty-accruals",
            post(routes::accrue_fleet_component_duty),
        )
        .route(
            "/api/fleet-health/health-indicators",
            get(routes::list_fleet_health_indicators)
                .post(routes::derive_fleet_health_indicators_route),
        )
        .route(
            "/api/fleet-health/ota-rollouts/evaluate",
            post(routes::evaluate_ota_rollout_route),
        )
        .route(
            "/api/fleet-health/ota-rollouts/control",
            post(routes::apply_rollout_control_route),
        )
        .route(
            "/api/soil-iot/devices",
            get(routes::list_soil_iot_devices).post(routes::register_soil_iot_device),
        )
        .route(
            "/api/soil-iot/devices/:device_id/config-pushes",
            get(routes::list_soil_iot_config_pushes).post(routes::record_soil_iot_config_push),
        )
        .route(
            "/api/soil-iot/devices/:device_id/config-pushes/:push_id/status",
            put(routes::update_soil_iot_config_push_status),
        )
        .route(
            "/api/soil-iot/readings",
            post(routes::ingest_soil_iot_reading),
        )
        .route(
            "/api/water-management/moisture-readings",
            get(routes::list_soil_moisture_readings).post(routes::ingest_soil_moisture_reading),
        )
        .route(
            "/api/water-management/moisture-reading-rejections",
            get(routes::list_soil_moisture_rejections),
        )
        .route(
            "/api/drought-management/indices",
            get(routes::list_drought_indices),
        )
        .route(
            "/api/drought-management/indices/compute",
            post(routes::compute_drought_index_route),
        )
        .route(
            "/api/drought-management/rasters",
            get(routes::list_drought_rasters_route),
        )
        .route(
            "/api/drought-management/rasters/derive",
            post(routes::derive_drought_raster_route),
        )
        .route(
            "/api/drought-management/chirps/register",
            post(routes::register_chirps_route),
        )
        .route(
            "/api/drought-management/chirps/fetch",
            post(routes::fetch_chirps_route),
        )
        .route(
            "/api/drought-management/spi/derive",
            post(routes::derive_spi_raster_route),
        )
        .route(
            "/api/drought-management/vhi/derive",
            post(routes::derive_vhi_raster_route),
        )
        .route("/api/thermal/lst/derive", post(routes::derive_lst_route))
        .route(
            "/api/water-management/et/derive",
            post(routes::derive_et_fraction_route),
        )
        .route(
            "/api/composites/derive",
            post(routes::derive_composite_route),
        )
        .route("/api/composites", get(routes::list_composites_route))
        .route(
            "/api/change-detection/dnbr/derive",
            post(routes::derive_dnbr_route),
        )
        .route("/api/change-detection/dnbr", get(routes::list_dnbr_route))
        .route(
            "/api/water-management/jrc/register",
            post(routes::register_jrc_route),
        )
        .route(
            "/api/water-management/sentinel1/register",
            post(routes::register_sentinel1_route),
        )
        .route(
            "/api/water-management/balance/derive",
            post(routes::derive_water_balance_route),
        )
        .route(
            "/api/water-management/seasonality/derive",
            post(routes::derive_water_seasonality_route),
        )
        .route(
            "/api/water-management/extent/derive",
            post(routes::derive_water_extent_route),
        )
        .route(
            "/api/water-management/extent",
            get(routes::list_water_extent_route),
        )
        .route(
            "/api/landcover/derive",
            post(routes::derive_landcover_route),
        )
        .route("/api/landcover/rasters", get(routes::list_landcover_route))
        .route(
            "/api/landcover/reference/register",
            post(routes::register_landcover_reference_route),
        )
        .route(
            "/api/landcover/validate",
            post(routes::validate_landcover_route),
        )
        .route(
            "/api/landcover/ml/classify",
            post(routes::classify_landcover_ml_route),
        )
        .route(
            "/api/marketplace/accounts",
            get(routes::list_marketplace_accounts).post(routes::create_marketplace_account),
        )
        .route(
            "/api/marketplace/accounts/:account_id",
            get(routes::get_marketplace_account),
        )
        .route(
            "/api/marketplace/accounts/:account_id/status",
            post(routes::update_marketplace_account_status),
        )
        .route(
            "/api/marketplace/catalog/items",
            get(routes::list_marketplace_catalog_items)
                .post(routes::create_marketplace_catalog_item),
        )
        .route(
            "/api/marketplace/catalog/items/:item_id",
            get(routes::get_marketplace_catalog_item),
        )
        .route(
            "/api/portal/marketplace-entry",
            get(routes::get_marketplace_portal_entry),
        )
        .route("/api/portal/login", post(routes::portal_login))
        .route("/api/portal/logout", post(routes::portal_logout))
        .route("/api/portal/me", get(routes::portal_me))
        .route("/api/portal/farms", get(routes::portal_list_farms))
        .route("/api/portal/fields", get(routes::portal_list_fields))
        .route(
            "/api/portal/fields/:field_id/overview",
            get(routes::portal_field_overview),
        )
        .route("/api/portal/reports", get(routes::portal_list_reports))
        .route(
            "/api/portal/reports/:report_id/read",
            post(routes::portal_mark_report_read),
        )
        .route(
            "/api/portal/reports/:report_id/download",
            get(routes::portal_download_report),
        )
        .route(
            "/api/portal/fields/:field_id/grower-report",
            post(routes::portal_generate_grower_report),
        )
        .route(
            "/api/portal/recommendations/:recommendation_id/status",
            put(routes::portal_update_recommendation_status),
        )
        .route(
            "/api/portal/fields/:field_id/activities",
            get(routes::portal_list_activities).post(routes::portal_create_activity),
        )
        .route(
            "/api/portal/fields/:field_id/activities/summary",
            get(routes::portal_field_activity_summary),
        )
        .route(
            "/api/portal/activities/:activity_id",
            put(routes::portal_update_activity).delete(routes::portal_delete_activity),
        )
        .route("/api/portal/alerts", get(routes::portal_list_alerts))
        .route(
            "/api/portal/notifications/summary",
            get(routes::portal_notifications_summary),
        )
        .route(
            "/api/admin/portal/access-codes",
            get(routes::list_portal_access_codes).post(routes::issue_portal_access_code),
        )
        .route(
            "/api/admin/portal/access-codes/:code_id/revoke",
            post(routes::revoke_portal_access_code),
        )
        .route(
            "/api/marketplace/listings",
            get(routes::list_marketplace_listings).post(routes::publish_marketplace_listing),
        )
        .route(
            "/api/marketplace/listings/:listing_id",
            get(routes::get_marketplace_listing),
        )
        .route(
            "/api/marketplace/listings/:listing_id/close",
            post(routes::close_marketplace_listing),
        )
        .route(
            "/api/marketplace/inventory",
            get(routes::list_marketplace_inventory).post(routes::upsert_marketplace_inventory),
        )
        .route(
            "/api/marketplace/inventory/:inventory_id",
            get(routes::get_marketplace_inventory),
        )
        .route(
            "/api/marketplace/inventory/:inventory_id/reserve",
            post(routes::reserve_marketplace_inventory_endpoint),
        )
        .route(
            "/api/marketplace/inventory/:inventory_id/fulfill",
            post(routes::fulfill_marketplace_inventory_endpoint),
        )
        .route(
            "/api/marketplace/inventory/:inventory_id/release",
            post(routes::release_marketplace_inventory_endpoint),
        )
        .route(
            "/api/marketplace/orders",
            get(routes::list_marketplace_orders).post(routes::place_marketplace_order),
        )
        .route(
            "/api/marketplace/orders/:order_id",
            get(routes::get_marketplace_order),
        )
        .route(
            "/api/marketplace/orders/:order_id/transition",
            post(routes::transition_marketplace_order),
        )
        .route(
            "/api/marketplace/orders/:order_id/audits",
            get(routes::list_marketplace_order_audits),
        )
        .route(
            "/api/marketplace/fulfillments",
            get(routes::list_marketplace_fulfillments).post(routes::create_marketplace_fulfillment),
        )
        .route(
            "/api/marketplace/fulfillments/:fulfillment_id",
            get(routes::get_marketplace_fulfillment),
        )
        .route(
            "/api/marketplace/fulfillments/:fulfillment_id/transition",
            post(routes::transition_marketplace_fulfillment),
        )
        .route(
            "/api/marketplace/fulfillments/:fulfillment_id/audits",
            get(routes::list_marketplace_fulfillment_audits),
        )
        .route(
            "/api/marketplace/ratings",
            get(routes::list_marketplace_ratings).post(routes::create_marketplace_rating),
        )
        .route(
            "/api/marketplace/ratings/accounts/:account_id/aggregate",
            get(routes::get_marketplace_rating_aggregate),
        )
        .route(
            "/api/marketplace/demand-forecasts",
            get(routes::list_marketplace_demand_forecasts)
                .post(routes::create_marketplace_demand_forecast),
        )
        .route(
            "/api/marketplace/demand-forecasts/:forecast_id",
            get(routes::get_marketplace_demand_forecast),
        )
        .route(
            "/api/marketplace/reports/org",
            get(routes::get_marketplace_org_report),
        )
        .route(
            "/api/sustainability/records",
            get(routes::list_sustainability_records).post(routes::create_sustainability_record),
        )
        .route(
            "/api/sustainability/records/:record_id",
            get(routes::get_sustainability_record),
        )
        .route(
            "/api/sustainability/carbon-footprints",
            get(routes::list_carbon_footprints).post(routes::create_carbon_footprint),
        )
        .route(
            "/api/sustainability/carbon-footprints/:footprint_id",
            get(routes::get_carbon_footprint),
        )
        .route(
            "/api/sustainability/biomass-estimates",
            get(routes::list_biomass_estimates).post(routes::create_biomass_estimate),
        )
        .route(
            "/api/sustainability/biomass-estimates/:estimate_id",
            get(routes::get_biomass_estimate),
        )
        .route(
            "/api/sustainability/baselines",
            get(routes::list_sustainability_baselines)
                .post(routes::create_sustainability_baseline_record),
        )
        .route(
            "/api/sustainability/comparisons",
            get(routes::list_sustainability_comparisons)
                .post(routes::create_sustainability_comparison),
        )
        .route(
            "/api/sustainability/comparisons/:comparison_id",
            get(routes::get_sustainability_comparison),
        )
        .route(
            "/api/sustainability/mrv-trails",
            get(routes::list_sustainability_mrv_trails)
                .post(routes::create_sustainability_mrv_trail_record),
        )
        .route(
            "/api/sustainability/mrv-trails/:trail_id",
            get(routes::get_sustainability_mrv_trail),
        )
        .route(
            "/api/sustainability/biodiversity-proxies",
            get(routes::list_biodiversity_proxies).post(routes::create_biodiversity_proxy),
        )
        .route(
            "/api/sustainability/biodiversity-proxies/:proxy_id",
            get(routes::get_biodiversity_proxy),
        )
        .route(
            "/api/sustainability/soil-carbon-proxies",
            get(routes::list_soil_carbon_proxies).post(routes::create_soil_carbon_proxy),
        )
        .route(
            "/api/sustainability/soil-carbon-proxies/:proxy_id",
            get(routes::get_soil_carbon_proxy),
        )
        .route(
            "/api/sustainability/kpis",
            get(routes::list_sustainability_kpis).post(routes::create_sustainability_kpi),
        )
        .route(
            "/api/sustainability/kpis/:kpi_id",
            get(routes::get_sustainability_kpi),
        )
        .route(
            "/api/sustainability/certification-packs",
            post(routes::create_sustainability_certification_pack),
        )
        .route(
            "/api/sustainability/certification-packs/:pack_id",
            get(routes::get_sustainability_certification_pack),
        )
        .route(
            "/api/sustainability/exports/field/:field_id/summary.csv",
            get(routes::export_sustainability_field_csv),
        )
        .route(
            "/api/sustainability/exports/field/:field_id/summary.geojson",
            get(routes::export_sustainability_field_geojson),
        )
        .route(
            "/api/sustainability/exports/field/:field_id/summary.pdf",
            get(routes::export_sustainability_field_pdf),
        )
        .route(
            "/api/content/items",
            get(routes::list_content_items).post(routes::create_content_item),
        )
        .route(
            "/api/content/success-stories",
            post(routes::create_success_story_item),
        )
        .route(
            "/api/content/success-stories/:content_id",
            get(routes::get_success_story_item),
        )
        .route(
            "/api/content/community-contributions",
            post(routes::create_community_contribution_route),
        )
        .route(
            "/api/content/community-contributions/:contribution_id/moderation",
            post(routes::moderate_community_contribution_route),
        )
        .route(
            "/api/content/items/:content_id",
            get(routes::get_content_item),
        )
        .route(
            "/api/content/items/:content_id/versions",
            post(routes::append_content_item_version),
        )
        .route(
            "/api/content/items/:content_id/workflow",
            post(routes::transition_content_item_workflow),
        )
        .route(
            "/api/content/items/:content_id/tags",
            post(routes::apply_content_item_tags),
        )
        .route(
            "/api/content/items/:content_id/engagement-events",
            post(routes::create_content_engagement_event_route),
        )
        .route(
            "/api/content/items/:content_id/engagement",
            get(routes::get_content_engagement_summary),
        )
        .route(
            "/api/content/items/:content_id/locales",
            post(routes::create_content_locale_variant_route),
        )
        .route(
            "/api/content/items/:content_id/localized",
            get(routes::get_localized_content_item),
        )
        .route(
            "/api/content/permissions/resolve",
            get(routes::resolve_content_permissions_route),
        )
        .route("/api/content/search", get(routes::search_content_items))
        .route("/api/content/tags", get(routes::list_content_items_by_tag))
        .route(
            "/api/portal/knowledge-base",
            get(routes::list_portal_knowledge_base),
        )
        .route(
            "/api/portal/knowledge-base/:content_id",
            get(routes::get_portal_knowledge_base_item),
        )
        .route(
            "/api/collaboration/channels",
            get(routes::list_collaboration_channels).post(routes::create_collaboration_channel),
        )
        .route(
            "/api/collaboration/channels/:channel_id",
            get(routes::get_collaboration_channel),
        )
        .route(
            "/api/collaboration/channels/:channel_id/messages",
            post(routes::post_collaboration_message),
        )
        .route(
            "/api/collaboration/channels/:channel_id/presence",
            get(routes::list_collaboration_presence)
                .post(routes::update_collaboration_presence_route),
        )
        .route(
            "/api/collaboration/channels/:channel_id/notifications",
            post(routes::create_collaboration_notifications),
        )
        .route(
            "/api/collaboration/channels/:channel_id/emergency-alerts",
            post(routes::raise_collaboration_emergency_alert_route),
        )
        .route(
            "/api/collaboration/emergency-alerts/:alert_id",
            post(routes::transition_collaboration_emergency_alert_route),
        )
        .route(
            "/api/collaboration/sessions",
            post(routes::record_collaboration_session_route),
        )
        .route(
            "/api/collaboration/sessions/:session_id/replay",
            get(routes::replay_collaboration_session_route),
        )
        .route(
            "/api/collaboration/sessions/:session_id/annotations",
            get(routes::list_collaboration_session_annotations_route)
                .post(routes::create_collaboration_session_annotation_route),
        )
        .route(
            "/api/collaboration/operator-console/feed",
            get(routes::collaboration_operator_console_feed_route),
        )
        .route(
            "/api/collaboration/portal/feed",
            get(routes::collaboration_portal_feed_route),
        )
        .route(
            "/api/collaboration/portal/streams/:stream_id",
            get(routes::collaboration_portal_stream_route),
        )
        .route(
            "/api/collaboration/portal/alerts/:alert_id",
            get(routes::collaboration_portal_alert_route),
        )
        .route(
            "/api/collaboration/mission-plans",
            post(routes::create_collaboration_mission_plan_route),
        )
        .route(
            "/api/collaboration/mission-plans/:plan_id/edits",
            post(routes::edit_collaboration_mission_plan_route),
        )
        .route(
            "/api/collaboration/mission-plans/:plan_id/dispatch",
            post(routes::dispatch_collaboration_mission_plan_route),
        )
        .route(
            "/api/collaboration/streams",
            post(routes::start_collaboration_stream_route),
        )
        .route(
            "/api/collaboration/streams/:stream_id/frames",
            get(routes::list_collaboration_stream_frames)
                .post(routes::relay_collaboration_stream_frame_route),
        )
        .route(
            "/api/collaboration/permissions/resolve",
            get(routes::resolve_collaboration_permissions_route),
        )
        .route(
            "/api/collaboration/actions/authorize",
            post(routes::authorize_collaboration_action_route),
        )
        .route(
            "/api/time-series/points",
            get(routes::list_time_series_points),
        )
        .route(
            "/api/provenance/lineage",
            get(routes::list_provenance_lineage_records),
        )
        .route(
            "/api/provenance/lineage/:artifact_id",
            get(routes::get_provenance_lineage_record),
        )
        .route(
            "/api/provenance/trace/:artifact_id",
            get(routes::get_provenance_trace),
        )
        .route(
            "/api/provenance/audit",
            get(routes::list_provenance_audit_entries),
        )
        .route(
            "/api/provenance/audit/:entry_hash",
            get(routes::get_provenance_audit_entry),
        )
        .route(
            "/api/plugins",
            get(routes::list_plugins).post(routes::register_plugin),
        )
        .route(
            "/api/plugins/:plugin_id/status",
            put(routes::update_plugin_status),
        )
        .route(
            "/api/plugins/:plugin_id/execute",
            post(routes::execute_plugin),
        )
        .route(
            "/api/alerting/fired-alerts",
            get(routes::list_fired_alerts).post(routes::store_fired_alert),
        )
        .route(
            "/api/alerting/fired-alerts/:alert_id",
            get(routes::get_fired_alert),
        )
        .route(
            "/api/alerting/rules",
            get(routes::list_alert_rules).post(routes::create_alert_rule),
        )
        .route(
            "/api/alerting/rules/:rule_id",
            get(routes::get_alert_rule_versions).put(routes::update_alert_rule),
        )
        .route(
            "/api/alerting/rules/:rule_id/status",
            put(routes::update_alert_rule_status),
        )
        .route(
            "/api/alerting/rules/:rule_id/subscriptions",
            get(routes::list_alert_rule_subscriptions).post(routes::create_alert_rule_subscription),
        )
        .route(
            "/api/fields/export/geojson",
            get(routes::export_fields_geojson),
        )
        .route(
            "/api/fields/import/geojson",
            post(routes::import_fields_geojson),
        )
        .route(
            "/api/fields/import/shapefile",
            post(routes::import_fields_shapefile),
        )
        .route("/api/fields/boundaries", get(routes::list_field_boundaries))
        .route("/api/fields/:field_id", get(routes::get_field))
        .route(
            "/api/fields/:field_id/farm/:farm_id",
            put(routes::link_field_to_farm),
        )
        .route(
            "/api/fields/:field_id/scenes",
            get(routes::list_field_scenes),
        )
        .route(
            "/api/fields/:field_id/scene-refresh-advisories",
            get(routes::list_field_scene_refresh_advisories),
        )
        .route(
            "/api/fields/:field_id/scene-change-advisories",
            get(routes::list_field_scene_change_advisories),
        )
        .route("/api/scenes", get(routes::list_scenes))
        .route("/api/layers", get(routes::list_layers))
        .route("/api/open-data/layers", get(routes::list_open_data_layers))
        .route(
            "/api/layers/:scene_id/:kind",
            get(routes::get_layer_metadata),
        )
        .route(
            "/api/layers/:scene_id/:kind/open-data",
            post(routes::publish_open_data_layer),
        )
        .route(
            "/api/layers/:scene_id/:kind/export/geotiff",
            get(routes::export_layer_geotiff),
        )
        .route("/api/scenes/:scene_id", get(routes::get_scene))
        .route("/api/scenes/:scene_id/audit", get(routes::get_scene_audit))
        .route(
            "/api/scenes/:scene_id/annotations",
            get(routes::list_scene_annotations).post(routes::create_scene_annotation),
        )
        .route(
            "/api/scenes/:scene_id/annotations/:annotation_id",
            put(routes::update_scene_annotation).delete(routes::delete_scene_annotation),
        )
        .route(
            "/api/scenes/:scene_id/recommendations",
            get(routes::list_scene_recommendations).post(routes::create_scene_recommendation),
        )
        .route(
            "/api/scenes/:scene_id/recommendations/:recommendation_id",
            get(routes::get_scene_recommendation)
                .put(routes::update_scene_recommendation)
                .delete(routes::delete_scene_recommendation),
        )
        .route(
            "/api/scenes/:scene_id/reports",
            get(routes::list_scene_reports).post(routes::generate_scene_report),
        )
        .route(
            "/api/scenes/:scene_id/reports/:report_id/lineage",
            get(routes::get_scene_report_lineage),
        )
        .route(
            "/api/scenes/:scene_id/reports/:report_id",
            get(routes::download_scene_report),
        )
        .route(
            "/api/scenes/:scene_id/reports/:report_id/shares",
            post(routes::create_report_share),
        )
        .route(
            "/api/scenes/:scene_id/reports/:report_id/shares/:share_token",
            delete(routes::revoke_report_share),
        )
        .route(
            "/api/report-shares/:share_token",
            get(routes::download_shared_report),
        )
        .route(
            "/api/fields/:field_id/exports/records.csv",
            get(routes::export_field_records_csv),
        )
        .route(
            "/api/fields/:field_id/exports/records.geojson",
            get(routes::export_field_records_geojson),
        )
        .route(
            "/api/scenes/:scene_id/exports/annotations.csv",
            get(routes::export_scene_annotations_csv),
        )
        .route(
            "/api/scenes/:scene_id/exports/recommendations.csv",
            get(routes::export_scene_recommendations_csv),
        )
        .route(
            "/api/scenes/:scene_id/exports/annotations.geojson",
            get(routes::export_scene_annotations_geojson),
        )
        .route(
            "/api/scenes/:scene_id/exports/recommendations.geojson",
            get(routes::export_scene_recommendations_geojson),
        )
        .route(
            "/api/scenes/:scene_id/field/:field_id",
            put(routes::link_scene_to_field),
        )
        .route(
            "/api/scenes/:scene_id/products/:kind",
            get(routes::stream_product),
        )
        .route(
            "/api/scenes/:scene_id/products/:kind/tiles/:z/:x/:y.png",
            get(routes::stream_product_tile),
        )
        // Satellite pipeline (batch S-8): subscriptions, manual trigger,
        // job queue inspection and retry.
        .route(
            "/api/fields/:field_id/subscriptions",
            get(routes::list_field_subscriptions).post(routes::upsert_field_subscription),
        )
        .route(
            "/api/subscriptions/:subscription_id",
            patch(routes::patch_subscription_status),
        )
        // Historical backfill (batch S-11): resumable 1982+ range walks.
        .route(
            "/api/fields/:field_id/backfill",
            post(routes::start_field_backfill),
        )
        .route(
            "/api/fields/:field_id/backfills",
            get(routes::list_field_backfills),
        )
        .route("/api/backfills/:backfill_id", get(routes::get_backfill))
        .route(
            "/api/backfills/:backfill_id/pause",
            post(routes::pause_backfill),
        )
        .route(
            "/api/backfills/:backfill_id/resume",
            post(routes::resume_backfill),
        )
        .route("/api/pipeline/run", post(routes::run_pipeline_now))
        .route("/api/pipeline/jobs", get(routes::list_pipeline_jobs))
        .route("/api/pipeline/jobs/:job_id", get(routes::get_pipeline_job))
        .route(
            "/api/pipeline/jobs/:job_id/retry",
            post(routes::retry_pipeline_job),
        )
        // Global request-security layers. The require-session gate is a no-op
        // unless `security.require_session` is set; the body limit always
        // applies. `from_fn_with_state` bakes in the state, so these wrap the
        // fully-stated router.
        .layer(from_fn_with_state(
            state.clone(),
            crate::security::require_session_mw,
        ))
        .layer(DefaultBodyLimit::max(state.config.security.max_body_bytes))
        .with_state(state)
}

/// Start the geo_hub HTTP server using configuration and resources.
pub async fn serve(config: HubConfig, pool: crate::db::DbPool) -> Result<()> {
    let addr: SocketAddr = config.bind_address.parse()?;
    let shared_config = Arc::new(config);
    let state = AppState {
        pool: pool.clone(),
        config: Arc::clone(&shared_config),
        scene_search_cache: Default::default(),
    };

    let router = build_router(state);

    // Satellite pipeline worker (batch S-8): a serial background loop that
    // drains the job queue and runs the subscription cadence pass. Off by
    // default (`[pipeline] enabled = false`), so tests/CI are unaffected.
    let (worker_shutdown_tx, worker_shutdown_rx) = watch::channel(false);
    let worker_handle = if shared_config.pipeline.enabled {
        info!("satellite pipeline worker enabled");
        let worker_ctx = PipelineWorkerContext {
            pool: pool.clone(),
            config: Arc::clone(&shared_config),
            cog_resolver: Arc::new(UrlCogResolver),
            item_fetcher: Arc::new(EarthSearchItemFetcher),
        };
        Some(spawn_pipeline_worker(worker_ctx, worker_shutdown_rx))
    } else {
        None
    };

    let listener = TcpListener::bind(addr).await?;
    info!(%addr, "geo_hub listening");

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    // The HTTP server drained after the shutdown signal; flip the worker's
    // watch channel and wait for the loop to exit before returning.
    let _ = worker_shutdown_tx.send(true);
    if let Some(handle) = worker_handle {
        if let Err(err) = handle.await {
            warn!(%err, "pipeline worker task join failed");
        }
    }

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            warn!(%err, "failed to install Ctrl+C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};

        if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
            sigterm.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    // Give upstream tasks a short window to finish
    tokio::time::sleep(Duration::from_millis(100)).await;
    info!("shutdown signal received");
}
