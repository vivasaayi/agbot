//! Mobile app + scene search + analyze route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers (decode_*, load_*, scene_exists, build_*_record) and domain
//! `*_error` mappers stay in the parent module and are reached via `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn mobile_app() -> Html<&'static str> {
    Html(MOBILE_APP_HTML)
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

pub(crate) fn normalize_field_geometry(
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

pub(crate) fn cached_landsat_scene_id(
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
            let irrigation = ((rotated_x * 22.0 + lat_seed).sin()
                * (rotated_y * 17.0 + lon_seed).cos())
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
