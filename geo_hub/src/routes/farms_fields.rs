//! Farms + fields CRUD / boundaries / imports route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers (decode_*, load_*, scene_exists, build_*_record) and domain
//! `*_error` mappers stay in the parent module and are reached via `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

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
