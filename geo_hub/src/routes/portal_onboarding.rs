//! Atomic, session-owned first-field setup for the farmer portal.
use super::*;
use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortalFieldDraft {
    pub farm_id: Option<String>,
    pub farm_name: Option<String>,
    pub name: String,
    pub crop: Option<String>,
    pub season: Option<String>,
    pub boundary: FieldBoundary,
}

pub async fn portal_create_field(
    identity: PortalIdentity,
    State(state): State<AppState>,
    Json(draft): Json<PortalFieldDraft>,
) -> AppResult<Json<serde_json::Value>> {
    if draft.boundary.crs.as_deref() != Some("EPSG:4326") {
        return Err(AppError::BadRequest("boundary must use EPSG:4326".into()));
    }
    let validated = shared::schemas::validate_field_boundary(&draft.boundary)
        .map_err(|err| AppError::BadRequest(err.to_string()))?;
    let new_farm = match (&draft.farm_id, &draft.farm_name) {
        (Some(id), None) => {
            portal::owned_farm(&state, &identity.org_id, id).await?;
            None
        }
        (None, Some(name)) => Some(build_farm_record(CreateFarmRequest {
            farm_id: None,
            org_id: Some(identity.org_id.clone()),
            owner: None,
            name: name.clone(),
            notes: None,
            status: None,
        })?),
        _ => {
            return Err(AppError::BadRequest(
                "choose an existing farm or name a new farm".into(),
            ))
        }
    };
    let farm_id = new_farm
        .as_ref()
        .map(|farm| farm.farm_id.clone())
        .or(draft.farm_id);
    let mut field = build_field_record(CreateFieldRequest {
        farm_id,
        field_id: None,
        org_id: Some(identity.org_id.clone()),
        owner: None,
        name: draft.name,
        crop: normalize_optional_text(draft.crop),
        season: normalize_optional_text(draft.season),
        notes: None,
        status: None,
        boundary: validated.boundary,
    })?;
    field.area_ha = Some(validated.area_ha);
    let now = &field.created_at;
    let mut tx = state.pool.begin().await.map_err(Error::from)?;
    if let Some(farm) = new_farm {
        sqlx::query("INSERT INTO farms (farm_id,owner,name,status,created_at,updated_at) VALUES (?,?,?,'active',?,?)")
            .bind(farm.farm_id).bind(&identity.org_id).bind(farm.name).bind(now).bind(now)
            .execute(&mut *tx).await.map_err(Error::from)?;
    } else {
        // Recheck inside the transaction so a deleted/reassigned farm cannot be linked.
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM farms WHERE farm_id=? AND owner=? AND status='active'",
        )
        .bind(&field.farm_id)
        .bind(&identity.org_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(Error::from)?;
        if exists != 1 {
            return Err(AppError::NotFound);
        }
    }
    sqlx::query("INSERT INTO fields (field_id,farm_id,owner,name,crop,season,boundary_json,status,created_at,updated_at) VALUES (?,?,?,?,?,?,?,'active',?,?)")
        .bind(&field.field_id).bind(&field.farm_id).bind(&identity.org_id).bind(&field.name)
        .bind(&field.crop).bind(&field.season).bind(serde_json::to_string(&field.boundary).map_err(Error::from)?)
        .bind(now).bind(now).execute(&mut *tx).await.map_err(Error::from)?;
    sqlx::query("INSERT INTO satellite_subscriptions (subscription_id,field_id,dataset,indices_json,cadence_hours,max_cloud_cover,lookback_days,status,created_at,updated_at) VALUES (?,?,'landsat','[\"ndvi\"]',24,60,90,'active',?,?)")
        .bind(format!("sub:{}:landsat", field.field_id)).bind(&field.field_id).bind(now).bind(now)
        .execute(&mut *tx).await.map_err(Error::from)?;
    let payload = serde_json::json!({"field_id":field.field_id,"dataset":"landsat"});
    sqlx::query("INSERT INTO pipeline_jobs (job_id,job_key,kind,field_id,dataset,payload_json,status,priority,run_after,created_at,updated_at) VALUES (?,?,'discover',?,'landsat',?,'queued',10,?,?,?)")
        .bind(format!("job:{}",Uuid::new_v4())).bind(crate::pipeline::discover_job_key(&field.field_id,"landsat"))
        .bind(&field.field_id).bind(payload.to_string()).bind(now).bind(now).bind(now)
        .execute(&mut *tx).await.map_err(Error::from)?;
    tx.commit().await.map_err(Error::from)?;
    Ok(Json(serde_json::json!({"field":field,"monitoring":{
        "dataset":"landsat","status":if state.config.pipeline.enabled {"queued"} else {"worker_disabled"}
    }})))
}

pub async fn portal_get_field(
    identity: PortalIdentity,
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<FieldRecord>> {
    portal::owned_field(&state, &identity.org_id, &field_id).await?;
    let field = load_field(&state, &field_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(field))
}
