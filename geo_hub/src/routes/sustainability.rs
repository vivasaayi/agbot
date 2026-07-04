//! Sustainability / ESG route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and the domain `*_error` mappers are reached from the parent
//! module via `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

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

pub async fn create_carbon_footprint(
    State(state): State<AppState>,
    Json(request): Json<CarbonFootprintComputeRequest>,
) -> AppResult<Json<CarbonFootprintResult>> {
    let requested_record_id =
        normalize_optional_text(Some(request.record_id.clone())).ok_or_else(|| {
            AppError::BadRequest("carbon footprint record_id is required".to_string())
        })?;
    let record = load_sustainability_record(&state, &requested_record_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "sustainability record {requested_record_id} is required for carbon footprint"
            ))
        })?;
    let result = compute_carbon_footprint(
        request,
        format!("carbon-footprint-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(carbon_footprint_error)?;
    if result.operation_id != record.operation_id {
        return Err(AppError::BadRequest(format!(
            "carbon footprint operation_id {} does not match sustainability record operation_id {}",
            result.operation_id, record.operation_id
        )));
    }
    insert_carbon_footprint_result(&state, &result).await?;

    Ok(Json(result))
}

pub async fn list_carbon_footprints(
    Query(query): Query<CarbonFootprintListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<CarbonFootprintResult>>> {
    let record_id = normalize_optional_text(query.record_id).ok_or_else(|| {
        AppError::BadRequest(
            "record_id query parameter is required for carbon footprints".to_string(),
        )
    })?;
    let operation_id = normalize_optional_text(query.operation_id);
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT footprint_id, record_id, operation_id, value_co2e, inputs_json,
               factor_set_version, factors_json, evidence_refs_json, status, result_hash,
               computed_at
        FROM carbon_footprints
        WHERE record_id = ?1
          AND (?2 IS NULL OR operation_id = ?2)
          AND (?3 IS NULL OR status = ?3)
        ORDER BY computed_at ASC, footprint_id ASC
        "#,
    )
    .bind(record_id)
    .bind(operation_id)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_carbon_footprint_result(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_carbon_footprint(
    Path(footprint_id): Path<String>,
    Query(query): Query<CarbonFootprintScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<CarbonFootprintResult>> {
    let record_id = normalize_optional_text(query.record_id)
        .ok_or_else(|| AppError::BadRequest("record_id query parameter is required".to_string()))?;
    let footprint = load_carbon_footprint_result(&state, &footprint_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if footprint.record_id != record_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(footprint))
}

pub async fn create_biomass_estimate(
    State(state): State<AppState>,
    Json(request): Json<BiomassEstimateRequest>,
) -> AppResult<Json<BiomassEstimateResult>> {
    let requested_record_id = normalize_optional_text(Some(request.record_id.clone()))
        .ok_or_else(|| AppError::BadRequest("biomass record_id is required".to_string()))?;
    load_sustainability_record(&state, &requested_record_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "sustainability record {requested_record_id} is required for biomass estimate"
            ))
        })?;
    let result = estimate_biomass(
        request,
        format!("biomass-estimate-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(biomass_estimate_error)?;
    insert_biomass_estimate_result(&state, &result).await?;

    Ok(Json(result))
}

pub async fn list_biomass_estimates(
    Query(query): Query<BiomassEstimateListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<BiomassEstimateResult>>> {
    let record_id = normalize_optional_text(query.record_id).ok_or_else(|| {
        AppError::BadRequest(
            "record_id query parameter is required for biomass estimates".to_string(),
        )
    })?;
    let rows = sqlx::query(
        r#"
        SELECT estimate_id, record_id, biomass_value, area, crs, extent_json,
               resolution_json, source_layer_refs_json, method_version, result_hash, computed_at
        FROM biomass_estimates
        WHERE record_id = ?1
        ORDER BY computed_at ASC, estimate_id ASC
        "#,
    )
    .bind(record_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_biomass_estimate_result(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_biomass_estimate(
    Path(estimate_id): Path<String>,
    Query(query): Query<BiomassEstimateScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<BiomassEstimateResult>> {
    let record_id = normalize_optional_text(query.record_id)
        .ok_or_else(|| AppError::BadRequest("record_id query parameter is required".to_string()))?;
    let estimate = load_biomass_estimate_result(&state, &estimate_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if estimate.record_id != record_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(estimate))
}

pub async fn create_sustainability_baseline_record(
    State(state): State<AppState>,
    Json(request): Json<SustainabilityBaselineCreateRequest>,
) -> AppResult<Json<SustainabilityBaselineRecord>> {
    let source_record = load_sustainability_record(&state, &request.source_record_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "sustainability record {} is required for baseline",
                request.source_record_id
            ))
        })?;
    if source_record.field_id != request.field_id
        || source_record.season_id != request.season_id
        || source_record.metric_type != request.metric_type
    {
        return Err(AppError::BadRequest(
            "baseline source record does not match requested field, season, and metric".to_string(),
        ));
    }
    let baseline = create_sustainability_baseline(
        request,
        format!("sustainability-baseline-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(sustainability_baseline_error)?;
    insert_sustainability_baseline(&state, &baseline).await?;

    Ok(Json(baseline))
}

pub async fn list_sustainability_baselines(
    Query(query): Query<SustainabilityBaselineListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SustainabilityBaselineRecord>>> {
    let field_id = normalize_optional_text(query.field_id).ok_or_else(|| {
        AppError::BadRequest(
            "field_id query parameter is required for sustainability baselines".to_string(),
        )
    })?;
    let metric_type = query
        .metric_type
        .map(|metric_type| metric_type.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT baseline_id, field_id, season_id, metric_type, metric_value, source_record_id,
               method_version, evidence_refs_json, created_at
        FROM sustainability_baselines
        WHERE field_id = ?1
          AND (?2 IS NULL OR metric_type = ?2)
        ORDER BY season_id ASC, metric_type ASC, baseline_id ASC
        "#,
    )
    .bind(field_id)
    .bind(metric_type)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_sustainability_baseline(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn create_sustainability_comparison(
    State(state): State<AppState>,
    Json(request): Json<SustainabilityComparisonRequest>,
) -> AppResult<Json<SustainabilityComparisonResult>> {
    let current_record = load_sustainability_record(&state, &request.current_source_record_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "sustainability record {} is required for comparison",
                request.current_source_record_id
            ))
        })?;
    if current_record.field_id != request.field_id
        || current_record.season_id != request.current_season_id
        || current_record.metric_type != request.metric_type
    {
        return Err(AppError::BadRequest(
            "current source record does not match requested field, season, and metric".to_string(),
        ));
    }
    let baseline = load_sustainability_baseline_for_metric(
        &state,
        &request.field_id,
        &request.baseline_season_id,
        request.metric_type,
    )
    .await?;
    let result = compare_sustainability_baseline(
        baseline.as_ref(),
        request,
        format!("sustainability-comparison-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(sustainability_baseline_error)?;
    insert_sustainability_comparison(&state, &result).await?;

    Ok(Json(result))
}

pub async fn list_sustainability_comparisons(
    Query(query): Query<SustainabilityComparisonListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SustainabilityComparisonResult>>> {
    let field_id = normalize_optional_text(query.field_id).ok_or_else(|| {
        AppError::BadRequest(
            "field_id query parameter is required for sustainability comparisons".to_string(),
        )
    })?;
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT comparison_id, field_id, baseline_season_id, current_season_id, metric_type,
               baseline_value, current_value, delta, trend, status, baseline_source_record_id,
               current_source_record_id, evidence_refs_json, method_version, result_hash,
               compared_at
        FROM sustainability_comparisons
        WHERE field_id = ?1
          AND (?2 IS NULL OR status = ?2)
        ORDER BY compared_at ASC, comparison_id ASC
        "#,
    )
    .bind(field_id)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_sustainability_comparison(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_sustainability_comparison(
    Path(comparison_id): Path<String>,
    Query(query): Query<SustainabilityComparisonScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<SustainabilityComparisonResult>> {
    let field_id = normalize_optional_text(query.field_id)
        .ok_or_else(|| AppError::BadRequest("field_id query parameter is required".to_string()))?;
    let comparison = load_sustainability_comparison(&state, &comparison_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if comparison.field_id != field_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(comparison))
}

pub async fn create_sustainability_mrv_trail_record(
    State(state): State<AppState>,
    Json(request): Json<SustainabilityMrvTrailCreateRequest>,
) -> AppResult<Json<SustainabilityMrvTrail>> {
    validate_sustainability_mrv_output_ref(&state, request.output_kind, &request.output_ref)
        .await?;
    let trail = create_sustainability_mrv_trail(
        request,
        format!("sustainability-mrv-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(sustainability_mrv_trail_error)?;
    insert_sustainability_mrv_trail(&state, &trail).await?;

    Ok(Json(trail))
}

pub async fn list_sustainability_mrv_trails(
    Query(query): Query<SustainabilityMrvTrailListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SustainabilityMrvTrail>>> {
    let output_ref = normalize_optional_text(query.output_ref);
    let output_kind = query.output_kind.map(|kind| kind.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT trail_id, output_ref, output_kind, input_layer_refs_json, method,
               method_version, crs, extent_json, parameters_json, audit_id, result_hash,
               rederived_result_hash, certification_ready, created_at
        FROM sustainability_mrv_trails
        WHERE (?1 IS NULL OR output_ref = ?1)
          AND (?2 IS NULL OR output_kind = ?2)
        ORDER BY created_at ASC, trail_id ASC
        "#,
    )
    .bind(output_ref)
    .bind(output_kind)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_sustainability_mrv_trail(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_sustainability_mrv_trail(
    Path(trail_id): Path<String>,
    Query(query): Query<SustainabilityMrvTrailScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<SustainabilityMrvTrail>> {
    let trail = load_sustainability_mrv_trail(&state, &trail_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if let Some(output_ref) = normalize_optional_text(query.output_ref) {
        if trail.output_ref != output_ref {
            return Err(AppError::NotFound);
        }
    }

    Ok(Json(trail))
}

pub async fn create_biodiversity_proxy(
    State(state): State<AppState>,
    Json(request): Json<BiodiversityProxyRequest>,
) -> AppResult<Json<BiodiversityProxyResult>> {
    let result = compute_biodiversity_proxy(
        request,
        format!("biodiversity-proxy-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(biodiversity_proxy_error)?;
    insert_biodiversity_proxy_result(&state, &result).await?;

    Ok(Json(result))
}

pub async fn list_biodiversity_proxies(
    Query(query): Query<BiodiversityProxyListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<BiodiversityProxyResult>>> {
    let field_id = normalize_optional_text(query.field_id).ok_or_else(|| {
        AppError::BadRequest(
            "field_id query parameter is required for biodiversity proxies".to_string(),
        )
    })?;
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT proxy_id, field_id, heterogeneity_score, cover_fraction, uncertainty, status,
               crs, extent_json, source_layer_refs_json, method_version, result_hash, computed_at
        FROM biodiversity_proxies
        WHERE field_id = ?1
          AND (?2 IS NULL OR status = ?2)
        ORDER BY computed_at ASC, proxy_id ASC
        "#,
    )
    .bind(field_id)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_biodiversity_proxy_result(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_biodiversity_proxy(
    Path(proxy_id): Path<String>,
    Query(query): Query<BiodiversityProxyScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<BiodiversityProxyResult>> {
    let field_id = normalize_optional_text(query.field_id)
        .ok_or_else(|| AppError::BadRequest("field_id query parameter is required".to_string()))?;
    let proxy = load_biodiversity_proxy_result(&state, &proxy_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if proxy.field_id != field_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(proxy))
}

pub async fn create_soil_carbon_proxy(
    State(state): State<AppState>,
    Json(request): Json<SoilCarbonProxyRequest>,
) -> AppResult<Json<SoilCarbonProxyResult>> {
    let result = compute_soil_carbon_proxy(
        request,
        format!("soil-carbon-proxy-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(soil_carbon_proxy_error)?;
    insert_soil_carbon_proxy_result(&state, &result).await?;

    Ok(Json(result))
}

pub async fn list_soil_carbon_proxies(
    Query(query): Query<SoilCarbonProxyListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SoilCarbonProxyResult>>> {
    let field_id = normalize_optional_text(query.field_id).ok_or_else(|| {
        AppError::BadRequest(
            "field_id query parameter is required for soil-carbon proxies".to_string(),
        )
    })?;
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT proxy_id, record_id, field_id, proxy_value, uncertainty_low, uncertainty_high,
               status, evidence_refs_json, method_version, result_hash, computed_at
        FROM soil_carbon_proxies
        WHERE field_id = ?1
          AND (?2 IS NULL OR status = ?2)
        ORDER BY computed_at ASC, proxy_id ASC
        "#,
    )
    .bind(field_id)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_soil_carbon_proxy_result(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_soil_carbon_proxy(
    Path(proxy_id): Path<String>,
    Query(query): Query<SoilCarbonProxyScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<SoilCarbonProxyResult>> {
    let field_id = normalize_optional_text(query.field_id)
        .ok_or_else(|| AppError::BadRequest("field_id query parameter is required".to_string()))?;
    let proxy = load_soil_carbon_proxy_result(&state, &proxy_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if proxy.field_id != field_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(proxy))
}

pub async fn create_sustainability_kpi(
    State(state): State<AppState>,
    Json(request): Json<SustainabilityKpiTrackingRequest>,
) -> AppResult<Json<SustainabilityKpiTrackingResult>> {
    let result = compute_sustainability_kpi(
        request,
        format!("sustainability-kpi-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(sustainability_kpi_error)?;
    insert_sustainability_kpi_result(&state, &result).await?;

    Ok(Json(result))
}

pub async fn list_sustainability_kpis(
    Query(query): Query<SustainabilityKpiListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SustainabilityKpiTrackingResult>>> {
    let field_id = normalize_optional_text(query.field_id).ok_or_else(|| {
        AppError::BadRequest(
            "field_id query parameter is required for sustainability KPIs".to_string(),
        )
    })?;
    let season_id = normalize_optional_text(query.season_id);
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT kpi_id, field_id, season_id, metric_ref, current_value, target_value,
               direction, at_risk_fraction, status, evidence_refs_json, method_version,
               result_hash, computed_at
        FROM sustainability_kpis
        WHERE field_id = ?1
          AND (?2 IS NULL OR season_id = ?2)
          AND (?3 IS NULL OR status = ?3)
        ORDER BY computed_at ASC, kpi_id ASC
        "#,
    )
    .bind(field_id)
    .bind(season_id)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_sustainability_kpi_result(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_sustainability_kpi(
    Path(kpi_id): Path<String>,
    Query(query): Query<SustainabilityKpiScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<SustainabilityKpiTrackingResult>> {
    let field_id = normalize_optional_text(query.field_id)
        .ok_or_else(|| AppError::BadRequest("field_id query parameter is required".to_string()))?;
    let kpi = load_sustainability_kpi_result(&state, &kpi_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if kpi.field_id != field_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(kpi))
}

pub async fn create_sustainability_certification_pack(
    State(state): State<AppState>,
    Json(mut request): Json<SustainabilityCertificationEvidencePackRequest>,
) -> AppResult<Json<SustainabilityCertificationEvidencePack>> {
    let claimed_output_refs = request
        .claimed_output_refs
        .iter()
        .filter_map(|value| normalize_optional_text(Some(value.clone())))
        .collect::<Vec<_>>();
    let (outputs, mrv_trails, evidence_layer_refs) =
        assemble_sustainability_certification_pack_inputs(&state, &claimed_output_refs).await?;
    request.outputs = outputs;
    request.mrv_trails = mrv_trails;
    request.evidence_layer_refs.extend(evidence_layer_refs);

    let pack = build_sustainability_certification_evidence_pack(
        request,
        format!("sustainability-certification-pack-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(sustainability_certification_pack_error)?;
    insert_sustainability_certification_pack(&state, &pack).await?;

    Ok(Json(pack))
}

pub async fn get_sustainability_certification_pack(
    Path(pack_id): Path<String>,
    Query(query): Query<SustainabilityCertificationPackScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<SustainabilityCertificationEvidencePack>> {
    let pack = load_sustainability_certification_pack(&state, &pack_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if let Some(claim_id) = normalize_optional_text(query.claim_id) {
        if pack.claim_id != claim_id {
            return Err(AppError::NotFound);
        }
    }

    Ok(Json(pack))
}

pub async fn export_sustainability_field_csv(
    Path(field_id): Path<String>,
    Query(query): Query<SustainabilityExportQuery>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let summary =
        load_sustainability_field_export_summary(&state, &field_id, query.season_id).await?;
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "record_type",
            "record_id",
            "field_id",
            "season_id",
            "metric_ref",
            "value",
            "unit",
            "status",
            "crs",
            "extent_json",
            "method_version",
            "evidence_refs",
            "result_hash",
            "computed_at",
        ])
        .map_err(|err| AppError::Anyhow(err.into()))?;
    for item in &summary.items {
        writer
            .write_record(vec![
                item.record_type.clone(),
                item.record_id.clone(),
                item.field_id.clone(),
                item.season_id.clone().unwrap_or_default(),
                item.metric_ref.clone(),
                item.value
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                item.unit.clone(),
                item.status.clone(),
                item.crs.clone().unwrap_or_default(),
                item.extent
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(|err| AppError::Anyhow(err.into()))?
                    .unwrap_or_default(),
                item.method_version.clone(),
                item.evidence_refs.join("|"),
                item.result_hash.clone(),
                item.computed_at.clone(),
            ])
            .map_err(|err| AppError::Anyhow(err.into()))?;
    }
    let csv_bytes = writer
        .into_inner()
        .map_err(|err| AppError::Anyhow(err.into_error().into()))?;

    response_with_bytes(
        csv_bytes,
        "text/csv; charset=utf-8",
        "sustainability-summary.csv",
    )
}

pub async fn export_sustainability_field_geojson(
    Path(field_id): Path<String>,
    Query(query): Query<SustainabilityExportQuery>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let summary =
        load_sustainability_field_export_summary(&state, &field_id, query.season_id).await?;
    let features = summary
        .items
        .iter()
        .map(sustainability_export_feature)
        .collect::<AppResult<Vec<_>>>()?;
    let mut geojson = feature_collection_with_crs(features, &summary.crs);
    if let GeoJson::FeatureCollection(collection) = &mut geojson {
        let mut members = collection.foreign_members.take().unwrap_or_default();
        members.insert(
            "field_id".to_string(),
            serde_json::Value::String(summary.field_id),
        );
        if let Some(season_id) = summary.season_id {
            members.insert(
                "season_id".to_string(),
                serde_json::Value::String(season_id),
            );
        }
        members.insert(
            "record_count".to_string(),
            serde_json::Value::from(summary.record_count as u64),
        );
        members.insert("empty".to_string(), serde_json::Value::Bool(summary.empty));
        members.insert(
            "generated_at".to_string(),
            serde_json::Value::String(summary.generated_at),
        );
        collection.foreign_members = Some(members);
    }

    response_with_bytes(
        serde_json::to_vec(&geojson).map_err(|err| AppError::Anyhow(err.into()))?,
        "application/geo+json",
        "sustainability-summary.geojson",
    )
}

pub async fn export_sustainability_field_pdf(
    Path(field_id): Path<String>,
    Query(query): Query<SustainabilityExportQuery>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let summary =
        load_sustainability_field_export_summary(&state, &field_id, query.season_id).await?;
    response_with_bytes(
        sustainability_summary_pdf_bytes(&summary),
        "application/pdf",
        "sustainability-summary.pdf",
    )
}
