use crate::db::DbPool;
use anyhow::{Context, Result};
use shared::schemas::{MultispectralImage, RasterSpatialRef};
use sqlx::Row;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct ProductPublication {
    pub product_id: String,
    pub scene_id: String,
    pub field_id: String,
    pub season_id: String,
    pub product_kind: String,
    pub spatial_ref: RasterSpatialRef,
    pub source_image_ids: Vec<String>,
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
    pub gsd_m_per_px: Option<f64>,
}

#[derive(Debug, Error)]
pub enum ProductPublishError {
    #[error("product kind cannot be empty")]
    EmptyProductKind,
    #[error("unlinked scene {scene_id} missing {field}")]
    UnlinkedScene {
        scene_id: String,
        field: &'static str,
    },
    #[error("scene {scene_id} has no product spatial_ref")]
    MissingSpatialRef { scene_id: String },
    #[error("scene {scene_id} has no source image ids")]
    MissingSourceImageIds { scene_id: String },
    #[error("product path cannot be empty")]
    EmptyProductPath,
    #[error("product dimensions must be positive")]
    InvalidDimensions,
    #[error("product gsd_m_per_px must be finite and positive")]
    InvalidGsd,
}

pub async fn publish_product(
    pool: &DbPool,
    scene_id: &str,
    kind: &str,
    product_path: &Path,
) -> Result<ProductPublication> {
    let context = load_publication_context(pool, scene_id, kind).await?;
    let spatial_ref_json = serde_json::to_string(&context.spatial_ref)
        .context("failed to encode product spatial_ref")?;
    let source_image_ids_json = serde_json::to_string(&context.source_image_ids)
        .context("failed to encode product source_image_ids")?;

    sqlx::query(
        r#"
        INSERT INTO products (
            product_id, scene_id, field_id, season_id, kind, path,
            spatial_ref_json, source_image_ids_json, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))
        ON CONFLICT(scene_id, kind) DO UPDATE SET
            product_id = excluded.product_id,
            field_id = excluded.field_id,
            season_id = excluded.season_id,
            path = excluded.path,
            width_px = NULL,
            height_px = NULL,
            gsd_m_per_px = NULL,
            spatial_ref_json = excluded.spatial_ref_json,
            source_image_ids_json = excluded.source_image_ids_json,
            publish_status = NULL,
            qa_report_ref = NULL,
            provenance_hash = NULL,
            downstream_consumers_json = NULL,
            created_at = datetime('now')
        "#,
    )
    .bind(&context.product_id)
    .bind(&context.scene_id)
    .bind(&context.field_id)
    .bind(&context.season_id)
    .bind(&context.product_kind)
    .bind(product_path.to_string_lossy().to_string())
    .bind(spatial_ref_json)
    .bind(source_image_ids_json)
    .execute(pool)
    .await?;

    Ok(context)
}

pub async fn publish_georeferenced_product(
    pool: &DbPool,
    scene_id: &str,
    field_id: &str,
    season_id: &str,
    kind: &str,
    product_path: &Path,
    spatial_ref: &RasterSpatialRef,
    width_px: u32,
    height_px: u32,
    gsd_m_per_px: f64,
    source_image_ids: Vec<String>,
) -> Result<ProductPublication> {
    if product_path.as_os_str().is_empty() {
        return Err(ProductPublishError::EmptyProductPath.into());
    }
    if width_px == 0 || height_px == 0 {
        return Err(ProductPublishError::InvalidDimensions.into());
    }
    if !gsd_m_per_px.is_finite() || gsd_m_per_px <= 0.0 {
        return Err(ProductPublishError::InvalidGsd.into());
    }

    let scene_id = normalize_required(
        scene_id,
        ProductPublishError::UnlinkedScene {
            scene_id: scene_id.to_string(),
            field: "scene_id",
        },
    )?;
    let field_id = normalize_required(
        field_id,
        ProductPublishError::UnlinkedScene {
            scene_id: scene_id.clone(),
            field: "field_id",
        },
    )?;
    let season_id = normalize_required(
        season_id,
        ProductPublishError::UnlinkedScene {
            scene_id: scene_id.clone(),
            field: "season_id",
        },
    )?;
    let product_kind = kind.trim().to_ascii_lowercase();
    if product_kind.is_empty() {
        return Err(ProductPublishError::EmptyProductKind.into());
    }
    let source_image_ids = source_image_ids
        .into_iter()
        .filter_map(|value| normalize_optional(&value))
        .collect::<Vec<_>>();
    if source_image_ids.is_empty() {
        return Err(ProductPublishError::MissingSourceImageIds {
            scene_id: scene_id.clone(),
        }
        .into());
    }

    let spatial_ref_json =
        serde_json::to_string(spatial_ref).context("failed to encode product spatial_ref")?;
    let source_image_ids_json = serde_json::to_string(&source_image_ids)
        .context("failed to encode product source_image_ids")?;
    let product_id = format!("{scene_id}:{product_kind}");

    sqlx::query(
        r#"
        INSERT INTO products (
            product_id, scene_id, field_id, season_id, kind, path,
            width_px, height_px, gsd_m_per_px,
            spatial_ref_json, source_image_ids_json, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))
        ON CONFLICT(scene_id, kind) DO UPDATE SET
            product_id = excluded.product_id,
            field_id = excluded.field_id,
            season_id = excluded.season_id,
            path = excluded.path,
            width_px = excluded.width_px,
            height_px = excluded.height_px,
            gsd_m_per_px = excluded.gsd_m_per_px,
            spatial_ref_json = excluded.spatial_ref_json,
            source_image_ids_json = excluded.source_image_ids_json,
            publish_status = NULL,
            qa_report_ref = NULL,
            provenance_hash = NULL,
            downstream_consumers_json = NULL,
            created_at = datetime('now')
        "#,
    )
    .bind(&product_id)
    .bind(&scene_id)
    .bind(&field_id)
    .bind(&season_id)
    .bind(&product_kind)
    .bind(product_path.to_string_lossy().to_string())
    .bind(i64::from(width_px))
    .bind(i64::from(height_px))
    .bind(gsd_m_per_px)
    .bind(spatial_ref_json)
    .bind(source_image_ids_json)
    .execute(pool)
    .await?;

    Ok(ProductPublication {
        product_id,
        scene_id,
        field_id,
        season_id,
        product_kind,
        spatial_ref: spatial_ref.clone(),
        source_image_ids,
        width_px: Some(width_px),
        height_px: Some(height_px),
        gsd_m_per_px: Some(gsd_m_per_px),
    })
}

async fn load_publication_context(
    pool: &DbPool,
    scene_id: &str,
    kind: &str,
) -> Result<ProductPublication> {
    let row = sqlx::query(
        r#"
        SELECT s.scene_id, s.field_id, s.season_id, s.metadata_json, sr.spatial_ref_json
        FROM scenes s
        LEFT JOIN scene_spatial_refs sr ON sr.scene_id = s.scene_id
        WHERE s.scene_id = ?1
        "#,
    )
    .bind(scene_id)
    .fetch_one(pool)
    .await?;

    let scene_id: String = row.get("scene_id");
    let field_id = required_linkage(&scene_id, row.get("field_id"), "field_id")?;
    let season_id = required_linkage(&scene_id, row.get("season_id"), "season_id")?;
    let product_kind = kind.trim().to_ascii_lowercase();
    if product_kind.is_empty() {
        return Err(ProductPublishError::EmptyProductKind.into());
    }

    let metadata_json: String = row.get("metadata_json");
    let image: MultispectralImage =
        serde_json::from_str(&metadata_json).context("failed to decode scene metadata_json")?;
    let spatial_ref = match row.get::<Option<String>, _>("spatial_ref_json") {
        Some(spatial_ref_json) => serde_json::from_str::<RasterSpatialRef>(&spatial_ref_json)
            .context("failed to decode asserted product spatial_ref")?,
        None => image.metadata.spatial_ref.clone().ok_or_else(|| {
            ProductPublishError::MissingSpatialRef {
                scene_id: scene_id.clone(),
            }
        })?,
    };
    let source_image_ids = vec![image.image_id.to_string()];
    if source_image_ids.iter().any(|value| value.trim().is_empty()) {
        return Err(ProductPublishError::MissingSourceImageIds {
            scene_id: scene_id.clone(),
        }
        .into());
    }

    Ok(ProductPublication {
        product_id: format!("{scene_id}:{product_kind}"),
        scene_id,
        field_id,
        season_id,
        product_kind,
        spatial_ref,
        source_image_ids,
        width_px: None,
        height_px: None,
        gsd_m_per_px: None,
    })
}

fn required_linkage(
    scene_id: &str,
    value: Option<String>,
    field: &'static str,
) -> std::result::Result<String, ProductPublishError> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ProductPublishError::UnlinkedScene {
            scene_id: scene_id.to_string(),
            field,
        })
}

fn normalize_required(
    value: &str,
    error: ProductPublishError,
) -> std::result::Result<String, ProductPublishError> {
    normalize_optional(value).ok_or(error)
}

fn normalize_optional(value: &str) -> Option<String> {
    let trimmed = value.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}
