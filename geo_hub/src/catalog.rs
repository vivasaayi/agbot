//! Product catalog registry (Track A batch 2).
//!
//! The catalog is the authoritative cross-source product graph. Producers hand
//! the catalog a [`ProductRecordDraft`] (from `shared::product_graph`); this
//! module derives the deterministic identity, validates the input graph, and
//! persists the node plus its input edges transactionally.
//!
//! Identity is `(kind, parameters_hash)` — deterministic over the algorithm,
//! parameters, and sorted inputs. Registration is therefore idempotent: the
//! same draft registered twice returns the same `product_id` and creates no
//! duplicate row. Scope (farm/field/season/scene, time, bbox) is descriptive
//! metadata, not identity.
//!
//! The provenance lineage write path is layered on top of `register_product`
//! in Track A batch 3; this batch establishes the catalog and its graph
//! invariants (mask-first ordering, unknown-input rejection, dedupe, filters).

use crate::db::DbPool;
use crate::provenance_store::{self, ProvenanceStoreError};
use provenance::{lineage_record_for_product_draft, ActorIdentity};
use shared::product_graph::{ProductLevel, ProductRecordDraft};
use sqlx::Row;
use std::str::FromStr;
use thiserror::Error;

/// Failure modes of catalog registration and lookup.
#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("input product {product_id} (role {role}) is not registered")]
    InputNotFound { product_id: String, role: String },
    #[error("quality mask {product_id} must be registered before the product that references it")]
    MaskNotRegistered { product_id: String },
    #[error("failed to serialize {what}: {source}")]
    Serialize {
        what: &'static str,
        source: serde_json::Error,
    },
    #[error("failed to deserialize {what}: {source}")]
    Deserialize {
        what: &'static str,
        source: serde_json::Error,
    },
    #[error("stored product {product_id} has an invalid level: {value}")]
    InvalidLevel { product_id: String, value: String },
    #[error("failed to persist provenance lineage: {0}")]
    Lineage(#[from] ProvenanceStoreError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// A catalog product row as persisted.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RegisteredProduct {
    pub product_id: String,
    pub level: ProductLevel,
    pub kind: String,
    pub algorithm_id: String,
    pub algorithm_version: String,
    pub parameters: serde_json::Value,
    pub parameters_hash: String,
    pub path: Option<String>,
    pub format: Option<String>,
    pub checksum_sha256: Option<String>,
    pub crs: Option<String>,
    /// `[min_x, min_y, max_x, max_y]` when the product is georeferenced.
    pub bbox: Option<[f64; 4]>,
    pub gsd_m_per_px: Option<f64>,
    pub temporal_start: Option<String>,
    pub temporal_end: Option<String>,
    pub farm_id: Option<String>,
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub scene_id: Option<String>,
    pub source_id: Option<String>,
    pub quality_mask_product_id: Option<String>,
    pub confidence: Option<f64>,
    pub confidence_method: Option<String>,
    pub status: String,
    pub superseded_by: Option<String>,
    pub provenance_id: Option<String>,
    pub created_at: String,
}

/// A directed edge in the product graph: `product_id` consumes
/// `input_product_id` in the given `role` (e.g. `band:nir`, `mask`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductInputEdge {
    pub input_product_id: String,
    pub role: String,
}

/// Filter for [`list_products`]. All set fields are ANDed together. `bbox`
/// (`[min_x, min_y, max_x, max_y]`) selects products whose bbox intersects it.
#[derive(Debug, Default, Clone)]
pub struct ProductFilter {
    pub farm_id: Option<String>,
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub scene_id: Option<String>,
    pub source_id: Option<String>,
    pub level: Option<ProductLevel>,
    pub kind: Option<String>,
    pub status: Option<String>,
    /// Lower bound: keep products whose `temporal_end >= temporal_start`.
    pub temporal_start: Option<String>,
    /// Upper bound: keep products whose `temporal_start <= temporal_end`.
    pub temporal_end: Option<String>,
    pub bbox: Option<[f64; 4]>,
}

/// Register a producer draft into the catalog and return its deterministic
/// `product_id`.
///
/// Enforces:
/// - **unknown-input rejection**: every declared input must already be
///   registered ([`CatalogError::InputNotFound`]);
/// - **mask-first ordering**: a referenced `quality_mask` must already be
///   registered ([`CatalogError::MaskNotRegistered`]);
/// - **dedupe**: identity is `(kind, parameters_hash)`; re-registering an
///   identical draft is a no-op that returns the existing id.
///
/// The product row, all its input edges (declared inputs plus the mask edge),
/// and its provenance lineage record are written in a single transaction, so a
/// registered product always has a traceable lineage.
///
/// This form attributes the lineage to a default catalog `SystemService`
/// actor; use [`register_product_with_actor`] to attribute a specific producer.
pub async fn register_product(
    pool: &DbPool,
    draft: &ProductRecordDraft,
    created_at: &str,
) -> Result<String, CatalogError> {
    register_product_with_actor(
        pool,
        draft,
        &ActorIdentity::system("geo_hub:catalog"),
        created_at,
    )
    .await
}

/// Register a product attributing its lineage to `actor`. See
/// [`register_product`] for the enforced invariants.
pub async fn register_product_with_actor(
    pool: &DbPool,
    draft: &ProductRecordDraft,
    actor: &ActorIdentity,
    created_at: &str,
) -> Result<String, CatalogError> {
    let product_id = draft.product_id();
    let parameters_hash = draft.parameters_hash();

    let mut tx = pool.begin().await?;

    // Idempotent dedupe on identity.
    if let Some(row) = sqlx::query(
        "SELECT product_id FROM catalog_products WHERE kind = ? AND parameters_hash = ?",
    )
    .bind(&draft.kind)
    .bind(&parameters_hash)
    .fetch_optional(&mut *tx)
    .await?
    {
        let existing: String = row.get("product_id");
        tx.commit().await?;
        return Ok(existing);
    }

    // Mask-first ordering: the referenced mask must already exist.
    if let Some(mask) = &draft.quality_mask {
        if !product_exists(&mut tx, &mask.product_id).await? {
            return Err(CatalogError::MaskNotRegistered {
                product_id: mask.product_id.clone(),
            });
        }
    }

    // Unknown-input rejection: every declared input must already exist.
    for input in &draft.inputs {
        if !product_exists(&mut tx, &input.product_id).await? {
            return Err(CatalogError::InputNotFound {
                product_id: input.product_id.clone(),
                role: input.role.clone(),
            });
        }
    }

    let parameters_json =
        serde_json::to_string(&draft.parameters).map_err(|source| CatalogError::Serialize {
            what: "parameters",
            source,
        })?;
    let spatial_ref_json = match &draft.spatial_ref {
        Some(sr) => Some(
            serde_json::to_string(sr).map_err(|source| CatalogError::Serialize {
                what: "spatial_ref",
                source,
            })?,
        ),
        None => None,
    };
    let quality_summary_json = match &draft.quality_summary {
        Some(qs) => Some(
            serde_json::to_string(qs).map_err(|source| CatalogError::Serialize {
                what: "quality_summary",
                source,
            })?,
        ),
        None => None,
    };

    let (crs, bbox) = match &draft.spatial_ref {
        Some(sr) => (
            sr.crs.clone(),
            sr.bbox
                .as_ref()
                .map(|b| (b.min_lon, b.min_lat, b.max_lon, b.max_lat)),
        ),
        None => (None, None),
    };
    let (bbox_min_x, bbox_min_y, bbox_max_x, bbox_max_y) = match bbox {
        Some((a, b, c, d)) => (Some(a), Some(b), Some(c), Some(d)),
        None => (None, None, None, None),
    };

    let (path, format, checksum) = match &draft.artifact {
        Some(a) => (
            Some(a.path.clone()),
            Some(a.format.clone()),
            a.checksum_sha256.clone(),
        ),
        None => (None, None, None),
    };
    let quality_mask_product_id = draft.quality_mask.as_ref().map(|m| m.product_id.clone());

    sqlx::query(
        r#"
        INSERT INTO catalog_products (
            product_id, level, kind, algorithm_id, algorithm_version,
            parameters_json, parameters_hash, path, format, checksum_sha256,
            spatial_ref_json, crs, bbox_min_x, bbox_min_y, bbox_max_x, bbox_max_y,
            gsd_m_per_px, temporal_start, temporal_end, farm_id, field_id,
            season_id, scene_id, source_id, quality_mask_product_id, confidence,
            confidence_method, quality_summary_json, status, provenance_id, created_at
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?, ?, 'registered', ?, ?
        )
        "#,
    )
    .bind(&product_id)
    .bind(draft.level.as_str())
    .bind(&draft.kind)
    .bind(&draft.algorithm_id)
    .bind(&draft.algorithm_version)
    .bind(&parameters_json)
    .bind(&parameters_hash)
    .bind(&path)
    .bind(&format)
    .bind(&checksum)
    .bind(&spatial_ref_json)
    .bind(&crs)
    .bind(bbox_min_x)
    .bind(bbox_min_y)
    .bind(bbox_max_x)
    .bind(bbox_max_y)
    .bind(draft.gsd_m_per_px)
    .bind(&draft.scope.temporal_start)
    .bind(&draft.scope.temporal_end)
    .bind(&draft.scope.farm_id)
    .bind(&draft.scope.field_id)
    .bind(&draft.scope.season_id)
    .bind(&draft.scope.scene_id)
    .bind(&draft.source_id)
    .bind(&quality_mask_product_id)
    .bind(draft.confidence)
    .bind(&draft.confidence_method)
    .bind(&quality_summary_json)
    .bind(&product_id) // provenance_id: the lineage artifact id equals the product id
    .bind(created_at)
    .execute(&mut *tx)
    .await?;

    // Input edges: declared inputs plus the quality-mask edge.
    for input in &draft.inputs {
        insert_edge(&mut tx, &product_id, &input.product_id, &input.role).await?;
    }
    if let Some(mask) = &draft.quality_mask {
        insert_edge(&mut tx, &product_id, &mask.product_id, &mask.role).await?;
    }

    // Provenance lineage, written in the same transaction as the domain row so a
    // registered product is never left without a traceable lineage.
    let lineage = lineage_record_for_product_draft(draft, actor.clone(), created_at);
    provenance_store::append_lineage(&mut *tx, &lineage).await?;

    tx.commit().await?;
    Ok(product_id)
}

async fn product_exists(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    product_id: &str,
) -> Result<bool, CatalogError> {
    let found = sqlx::query("SELECT 1 FROM catalog_products WHERE product_id = ?")
        .bind(product_id)
        .fetch_optional(&mut **tx)
        .await?;
    Ok(found.is_some())
}

async fn insert_edge(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    product_id: &str,
    input_product_id: &str,
    role: &str,
) -> Result<(), CatalogError> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO catalog_product_inputs (product_id, input_product_id, role)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(product_id)
    .bind(input_product_id)
    .bind(role)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Look up a single product by id.
pub async fn get_product(
    pool: &DbPool,
    product_id: &str,
) -> Result<Option<RegisteredProduct>, CatalogError> {
    let row = sqlx::query("SELECT * FROM catalog_products WHERE product_id = ?")
        .bind(product_id)
        .fetch_optional(pool)
        .await?;
    row.map(row_to_product).transpose()
}

/// List products matching `filter`, most recent first.
pub async fn list_products(
    pool: &DbPool,
    filter: &ProductFilter,
) -> Result<Vec<RegisteredProduct>, CatalogError> {
    let mut sql = String::from("SELECT * FROM catalog_products WHERE 1 = 1");
    // Bind values are appended in the same order the placeholders are pushed.
    if filter.farm_id.is_some() {
        sql.push_str(" AND farm_id = ?");
    }
    if filter.field_id.is_some() {
        sql.push_str(" AND field_id = ?");
    }
    if filter.season_id.is_some() {
        sql.push_str(" AND season_id = ?");
    }
    if filter.scene_id.is_some() {
        sql.push_str(" AND scene_id = ?");
    }
    if filter.source_id.is_some() {
        sql.push_str(" AND source_id = ?");
    }
    if filter.level.is_some() {
        sql.push_str(" AND level = ?");
    }
    if filter.kind.is_some() {
        sql.push_str(" AND kind = ?");
    }
    if filter.status.is_some() {
        sql.push_str(" AND status = ?");
    }
    if filter.temporal_start.is_some() {
        sql.push_str(" AND temporal_end >= ?");
    }
    if filter.temporal_end.is_some() {
        sql.push_str(" AND temporal_start <= ?");
    }
    if filter.bbox.is_some() {
        // Intersection of two axis-aligned boxes.
        sql.push_str(
            " AND bbox_min_x IS NOT NULL AND bbox_min_x <= ? AND bbox_max_x >= ? \
             AND bbox_min_y <= ? AND bbox_max_y >= ?",
        );
    }
    sql.push_str(" ORDER BY created_at DESC, product_id ASC");

    let mut query = sqlx::query(&sql);
    if let Some(v) = &filter.farm_id {
        query = query.bind(v);
    }
    if let Some(v) = &filter.field_id {
        query = query.bind(v);
    }
    if let Some(v) = &filter.season_id {
        query = query.bind(v);
    }
    if let Some(v) = &filter.scene_id {
        query = query.bind(v);
    }
    if let Some(v) = &filter.source_id {
        query = query.bind(v);
    }
    if let Some(v) = &filter.level {
        query = query.bind(v.as_str());
    }
    if let Some(v) = &filter.kind {
        query = query.bind(v);
    }
    if let Some(v) = &filter.status {
        query = query.bind(v);
    }
    if let Some(v) = &filter.temporal_start {
        query = query.bind(v);
    }
    if let Some(v) = &filter.temporal_end {
        query = query.bind(v);
    }
    if let Some([min_x, min_y, max_x, max_y]) = &filter.bbox {
        // product.min_x <= filter.max_x AND product.max_x >= filter.min_x, etc.
        query = query.bind(max_x).bind(min_x).bind(max_y).bind(min_y);
    }

    let rows = query.fetch_all(pool).await?;
    rows.into_iter().map(row_to_product).collect()
}

/// Direct input edges of `product_id` (one hop). Recursive backward tracing is
/// provided by the provenance layer in Track A batch 3.
pub async fn trace_inputs(
    pool: &DbPool,
    product_id: &str,
) -> Result<Vec<ProductInputEdge>, CatalogError> {
    let rows = sqlx::query(
        "SELECT input_product_id, role FROM catalog_product_inputs WHERE product_id = ?",
    )
    .bind(product_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ProductInputEdge {
            input_product_id: row.get("input_product_id"),
            role: row.get("role"),
        })
        .collect())
}

/// Mark `old_product_id` as superseded by `new_product_id`.
pub async fn supersede_product(
    pool: &DbPool,
    old_product_id: &str,
    new_product_id: &str,
) -> Result<(), CatalogError> {
    sqlx::query(
        "UPDATE catalog_products SET status = 'superseded', superseded_by = ? WHERE product_id = ?",
    )
    .bind(new_product_id)
    .bind(old_product_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn row_to_product(row: sqlx::sqlite::SqliteRow) -> Result<RegisteredProduct, CatalogError> {
    let product_id: String = row.get("product_id");
    let level_str: String = row.get("level");
    let level = ProductLevel::from_str(&level_str).map_err(|_| CatalogError::InvalidLevel {
        product_id: product_id.clone(),
        value: level_str.clone(),
    })?;
    let parameters_json: String = row.get("parameters_json");
    let parameters =
        serde_json::from_str(&parameters_json).map_err(|source| CatalogError::Deserialize {
            what: "parameters",
            source,
        })?;

    let bbox = match (
        row.get::<Option<f64>, _>("bbox_min_x"),
        row.get::<Option<f64>, _>("bbox_min_y"),
        row.get::<Option<f64>, _>("bbox_max_x"),
        row.get::<Option<f64>, _>("bbox_max_y"),
    ) {
        (Some(a), Some(b), Some(c), Some(d)) => Some([a, b, c, d]),
        _ => None,
    };

    Ok(RegisteredProduct {
        product_id,
        level,
        kind: row.get("kind"),
        algorithm_id: row.get("algorithm_id"),
        algorithm_version: row.get("algorithm_version"),
        parameters,
        parameters_hash: row.get("parameters_hash"),
        path: row.get("path"),
        format: row.get("format"),
        checksum_sha256: row.get("checksum_sha256"),
        crs: row.get("crs"),
        bbox,
        gsd_m_per_px: row.get("gsd_m_per_px"),
        temporal_start: row.get("temporal_start"),
        temporal_end: row.get("temporal_end"),
        farm_id: row.get("farm_id"),
        field_id: row.get("field_id"),
        season_id: row.get("season_id"),
        scene_id: row.get("scene_id"),
        source_id: row.get("source_id"),
        quality_mask_product_id: row.get("quality_mask_product_id"),
        confidence: row.get("confidence"),
        confidence_method: row.get("confidence_method"),
        status: row.get("status"),
        superseded_by: row.get("superseded_by"),
        provenance_id: row.get("provenance_id"),
        created_at: row.get("created_at"),
    })
}

// --- Sidecar directory registration CLI (Track A batch 8) -------------------

/// Outcome of walking a directory of `*.product_record.json` sidecars and
/// registering each into the catalog.
#[derive(Debug, Default)]
pub struct CatalogRegisterReport {
    /// Catalog product ids registered (or already present).
    pub registered: Vec<String>,
    /// Sidecars that could not be registered: `(sidecar_path, reason)`.
    pub failed: Vec<(String, String)>,
}

/// Recursively collect `*.product_record.json` sidecar file paths under `dir`.
fn collect_sidecar_paths(dir: &std::path::Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".product_record.json"))
            {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn is_mask_kind(kind: &str) -> bool {
    kind.contains("mask")
}

/// Walk `dir` for `*.product_record.json` sidecars and register each draft into
/// the catalog. Registration order respects the dependency graph: lower levels
/// first, masks before the products that reference them, and any draft whose
/// inputs are not yet present is deferred and retried until the set converges.
/// Registration is idempotent, so re-running is safe.
pub async fn register_sidecar_dir(
    pool: &DbPool,
    dir: &std::path::Path,
    created_at: &str,
) -> Result<CatalogRegisterReport, CatalogError> {
    let mut report = CatalogRegisterReport::default();
    let mut pending: Vec<(std::path::PathBuf, ProductRecordDraft)> = Vec::new();
    for path in collect_sidecar_paths(dir).map_err(sqlx::Error::Io)? {
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<ProductRecordDraft>(&bytes) {
                Ok(draft) => pending.push((path, draft)),
                Err(err) => report
                    .failed
                    .push((path.to_string_lossy().to_string(), format!("parse: {err}"))),
            },
            Err(err) => report
                .failed
                .push((path.to_string_lossy().to_string(), format!("read: {err}"))),
        }
    }

    // First-pass ordering: level ascending, then masks before non-masks.
    pending.sort_by(|(_, a), (_, b)| {
        a.level
            .cmp(&b.level)
            .then_with(|| is_mask_kind(&b.kind).cmp(&is_mask_kind(&a.kind)))
    });

    // Retry loop: defer drafts whose inputs/mask are not yet registered until a
    // full pass makes no progress.
    loop {
        let mut progressed = false;
        let mut deferred = Vec::new();
        for (path, draft) in std::mem::take(&mut pending) {
            match register_product(pool, &draft, created_at).await {
                Ok(product_id) => {
                    report.registered.push(product_id);
                    progressed = true;
                }
                Err(CatalogError::InputNotFound { .. })
                | Err(CatalogError::MaskNotRegistered { .. }) => {
                    deferred.push((path, draft));
                }
                Err(err) => {
                    report
                        .failed
                        .push((path.to_string_lossy().to_string(), err.to_string()));
                    progressed = true;
                }
            }
        }
        pending = deferred;
        if !progressed || pending.is_empty() {
            break;
        }
    }
    for (path, _) in pending {
        report.failed.push((
            path.to_string_lossy().to_string(),
            "unresolved inputs or mask".to_string(),
        ));
    }
    Ok(report)
}
