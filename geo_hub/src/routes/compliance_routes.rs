//! Compliance / audit / airspace-zones route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and domain `*_error` mappers are reached from the parent via
//! `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

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

pub async fn export_compliance_authority_report(
    State(state): State<AppState>,
    Json(request): Json<ComplianceAuthorityExportApiRequest>,
) -> AppResult<Json<ComplianceAuthorityExportArtifact>> {
    let export = build_compliance_authority_export_from_api(&state, request).await?;
    persist_compliance_authority_export(&state, &export).await?;

    Ok(Json(export))
}

pub async fn create_compliance_authority_share(
    State(state): State<AppState>,
    Json(request): Json<ComplianceAuthorityShareApiRequest>,
) -> AppResult<Json<ComplianceAuthorityShareArtifact>> {
    let export = build_compliance_authority_export_from_api(&state, request.export_request).await?;
    persist_compliance_authority_export(&state, &export).await?;
    let created_at = request.created_at.unwrap_or_else(current_record_timestamp);
    let share = build_compliance_authority_share(ComplianceAuthorityShareRequest {
        share_id: request
            .share_id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("compliance-share-{}", Uuid::new_v4())),
        export,
        created_at,
        expires_at: request.expires_at,
    })
    .map_err(compliance_authority_share_error)?;
    persist_compliance_authority_share(&state, &share).await?;
    audit_compliance_authority_share_event(&state, &share, "share_created", None, None).await?;

    Ok(Json(share))
}

pub async fn get_compliance_authority_share(
    Path(share_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<ComplianceAuthorityExportArtifact>> {
    let share = load_compliance_authority_share(&state, &share_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if share.revoked_at.is_some() {
        return Err(AppError::Forbidden(
            "compliance authority share has been revoked".to_string(),
        ));
    }
    audit_compliance_authority_share_event(&state, &share, "share_accessed", None, None).await?;

    Ok(Json(share.export))
}

pub async fn revoke_compliance_authority_share_route(
    Path(share_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<ComplianceAuthorityShareRevokeRequest>,
) -> AppResult<Json<ComplianceAuthorityShareArtifact>> {
    let share = load_compliance_authority_share(&state, &share_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let revoked = revoke_compliance_authority_share(
        share,
        request.revoked_at.unwrap_or_else(current_record_timestamp),
    )
    .map_err(compliance_authority_share_error)?;
    persist_compliance_authority_share(&state, &revoked).await?;
    audit_compliance_authority_share_event(
        &state,
        &revoked,
        "share_revoked",
        request.actor.as_deref(),
        revoked.revoked_at.as_deref(),
    )
    .await?;

    Ok(Json(revoked))
}

pub async fn run_compliance_regulation_assist(
    State(state): State<AppState>,
    Json(request): Json<ComplianceRegulationAssistApiRequest>,
) -> AppResult<Json<ComplianceRegulationAssistOutput>> {
    let records =
        load_compliance_records_for_report(&state, &request.org_id, &request.field_id).await?;
    let mandatory_record_types = if request.mandatory_record_types.is_empty() {
        default_compliance_report_mandatory_types()
    } else {
        request.mandatory_record_types
    };
    let generated_at = request
        .generated_at
        .unwrap_or_else(current_record_timestamp);
    let report = build_compliance_audit_report(ComplianceAuditReportRequest {
        report_id: format!("compliance-assist-report-{}", Uuid::new_v4()),
        org_id: request.org_id,
        field_id: request.field_id,
        generated_at: generated_at.clone(),
        records,
        mandatory_record_types,
    })
    .map_err(compliance_audit_report_error)?;

    let output = build_compliance_regulation_assist(ComplianceRegulationAssistRequest {
        assist_id: request
            .assist_id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("compliance-assist-{}", Uuid::new_v4())),
        intent: request.intent,
        report,
        generated_at,
        rule_citations: request.rule_citations,
        feature_enabled: request.feature_enabled,
    })
    .map_err(compliance_regulation_assist_error)?;

    Ok(Json(output))
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
