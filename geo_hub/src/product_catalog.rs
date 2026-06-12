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
            spatial_ref_json = excluded.spatial_ref_json,
            source_image_ids_json = excluded.source_image_ids_json,
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
    })
}

fn required_linkage(
    scene_id: &str,
    value: Option<String>,
    field: &'static str,
) -> Result<String, ProductPublishError> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ProductPublishError::UnlinkedScene {
            scene_id: scene_id.to_string(),
            field,
        })
}
