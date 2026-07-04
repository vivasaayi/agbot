//! Scenes / layers / tiles / products / open-data route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers (decode_*, load_*, scene_exists, build_*_record) and domain
//! `*_error` mappers stay in the parent module and are reached via `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

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
        if !uncertainty
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

