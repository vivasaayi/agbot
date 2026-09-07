//! Application runs (Track B phase B1).
//!
//! An application run is a governed analysis over cataloged L2/L3 products that
//! emits findings with provenance. Inputs must be registered catalog products of
//! level L2 or L3 (never raw files); every run writes lineage under a
//! `SystemService` actor so its findings trace back to source.

use crate::catalog;
use crate::db::DbPool;
use crate::provenance_store::{self, ProvenanceStoreError};
use provenance::{ActorIdentity, ArtifactKind, LineageRecord, ProvenanceParameters};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared::product_graph::ProductLevel;
use thiserror::Error;

/// A finding emitted by an application run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationFinding {
    pub kind: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub zone_geometry: Option<serde_json::Value>,
    #[serde(default)]
    pub metrics: Option<serde_json::Value>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

/// Request to record an application run.
#[derive(Debug, Clone, Deserialize)]
pub struct ApplicationRunRequest {
    #[serde(default)]
    pub org_id: Option<String>,
    pub field_id: String,
    /// Cataloged L2/L3 product ids this run consumed.
    pub input_product_ids: Vec<String>,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub findings: Vec<ApplicationFinding>,
}

/// A recorded application run.
#[derive(Debug, Clone, Serialize)]
pub struct ApplicationRunRecord {
    pub run_id: String,
    pub app_id: String,
    pub field_id: String,
    pub input_product_ids: Vec<String>,
    pub params_digest: String,
    pub status: String,
    pub output_finding_ids: Vec<String>,
    pub created_at: String,
}

/// A stored finding with its identity + run linkage.
#[derive(Debug, Clone, Serialize)]
pub struct StoredFinding {
    pub finding_id: String,
    pub run_id: String,
    pub app_id: String,
    pub field_id: Option<String>,
    pub finding: ApplicationFinding,
    pub created_at: String,
}

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("input product {0} is not registered in the catalog")]
    InputNotFound(String),
    #[error("input product {product_id} is level {level}; applications only consume L2/L3")]
    InputNotL2OrL3 { product_id: String, level: String },
    #[error("input product {product_id} is level {level}; this application requires L3")]
    InputNotL3 { product_id: String, level: String },
    #[error("input product {product_id} belongs to field {actual_field_id:?}, not requested field {requested_field_id}")]
    InputFieldMismatch {
        product_id: String,
        requested_field_id: String,
        actual_field_id: Option<String>,
    },
    #[error("failed to serialize {what}: {source}")]
    Serialize {
        what: &'static str,
        source: serde_json::Error,
    },
    #[error(transparent)]
    Catalog(#[from] catalog::CatalogError),
    #[error(transparent)]
    Provenance(#[from] ProvenanceStoreError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

fn params_digest(app_id: &str, params: &serde_json::Value, inputs: &[String]) -> String {
    let mut sorted = inputs.to_vec();
    sorted.sort();
    let canonical = serde_json::json!({
        "app_id": app_id,
        "params": params,
        "inputs": sorted,
    });
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string().as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Record an application run: validate inputs are cataloged L2/L3 products,
/// persist the run + its findings, and write lineage so each finding traces to
/// its L2/L3 inputs. Attributed to a `SystemService` actor.
pub async fn record_run(
    pool: &DbPool,
    app_id: &str,
    request: &ApplicationRunRequest,
    created_at: &str,
) -> Result<ApplicationRunRecord, ApplicationError> {
    // Inputs must be cataloged L2/L3 products.
    for product_id in &request.input_product_ids {
        let product = catalog::get_product(pool, product_id)
            .await?
            .ok_or_else(|| ApplicationError::InputNotFound(product_id.clone()))?;
        if !matches!(product.level, ProductLevel::L2 | ProductLevel::L3) {
            return Err(ApplicationError::InputNotL2OrL3 {
                product_id: product_id.clone(),
                level: product.level.as_str().to_string(),
            });
        }
    }

    let digest = params_digest(app_id, &request.params, &request.input_product_ids);
    let run_id = format!("{app_id}:{}:{}", request.field_id, &digest[..12]);
    let actor = ActorIdentity::system(&format!("geo_hub:app:{app_id}"));

    let inputs_json = serde_json::to_string(&request.input_product_ids).map_err(|source| {
        ApplicationError::Serialize {
            what: "input_product_ids",
            source,
        }
    })?;
    let params_json =
        serde_json::to_string(&request.params).map_err(|source| ApplicationError::Serialize {
            what: "params",
            source,
        })?;

    let mut finding_ids = Vec::with_capacity(request.findings.len());
    for (index, finding) in request.findings.iter().enumerate() {
        let finding_id = format!("{run_id}:f{index}");
        finding_ids.push(finding_id.clone());

        let zone_json = optional_json(&finding.zone_geometry, "zone_geometry")?;
        let metrics_json = optional_json(&finding.metrics, "metrics")?;
        let evidence_json = serde_json::to_string(&finding.evidence_refs).map_err(|source| {
            ApplicationError::Serialize {
                what: "evidence_refs",
                source,
            }
        })?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO application_findings
                (finding_id, run_id, app_id, field_id, kind, severity, confidence,
                 zone_geometry_json, metrics_json, evidence_refs_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&finding_id)
        .bind(&run_id)
        .bind(app_id)
        .bind(&request.field_id)
        .bind(&finding.kind)
        .bind(&finding.severity)
        .bind(finding.confidence)
        .bind(zone_json)
        .bind(metrics_json)
        .bind(evidence_json)
        .bind(created_at)
        .execute(pool)
        .await?;

        // Lineage: the finding derives from the run's L2/L3 inputs.
        provenance_store::append_lineage(
            pool,
            &LineageRecord {
                artifact_id: finding_id,
                kind: ArtifactKind::Finding,
                inputs: request.input_product_ids.clone(),
                method: format!("application:{app_id}"),
                parameters: ProvenanceParameters::from_json(request.params.clone()),
                operator: format!("geo_hub:app:{app_id}"),
                actor: actor.clone(),
                created_at: created_at.to_string(),
            },
        )
        .await?;
    }

    let finding_ids_json =
        serde_json::to_string(&finding_ids).map_err(|source| ApplicationError::Serialize {
            what: "finding_ids",
            source,
        })?;

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO application_runs
            (run_id, app_id, org_id, field_id, input_product_ids_json, params_json,
             params_digest, status, output_finding_ids_json, output_recommendation_ids_json,
             provenance_id, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, 'completed', ?, '[]', ?, ?)
        "#,
    )
    .bind(&run_id)
    .bind(app_id)
    .bind(&request.org_id)
    .bind(&request.field_id)
    .bind(inputs_json)
    .bind(params_json)
    .bind(&digest)
    .bind(&finding_ids_json)
    .bind(&run_id)
    .bind(created_at)
    .execute(pool)
    .await?;

    Ok(ApplicationRunRecord {
        run_id,
        app_id: app_id.to_string(),
        field_id: request.field_id.clone(),
        input_product_ids: request.input_product_ids.clone(),
        params_digest: digest,
        status: "completed".to_string(),
        output_finding_ids: finding_ids,
        created_at: created_at.to_string(),
    })
}

fn optional_json(
    value: &Option<serde_json::Value>,
    what: &'static str,
) -> Result<Option<String>, ApplicationError> {
    match value {
        Some(v) => {
            Ok(Some(serde_json::to_string(v).map_err(|source| {
                ApplicationError::Serialize { what, source }
            })?))
        }
        None => Ok(None),
    }
}

/// Fetch a run by id.
pub async fn get_run(
    pool: &DbPool,
    run_id: &str,
) -> Result<Option<ApplicationRunRecord>, ApplicationError> {
    use sqlx::Row;
    let Some(row) = sqlx::query("SELECT * FROM application_runs WHERE run_id = ?")
        .bind(run_id)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(None);
    };
    let input_product_ids: Vec<String> =
        serde_json::from_str(&row.get::<String, _>("input_product_ids_json")).unwrap_or_default();
    let output_finding_ids: Vec<String> = row
        .get::<Option<String>, _>("output_finding_ids_json")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    Ok(Some(ApplicationRunRecord {
        run_id: row.get("run_id"),
        app_id: row.get("app_id"),
        field_id: row.get::<Option<String>, _>("field_id").unwrap_or_default(),
        input_product_ids,
        params_digest: row.get("params_digest"),
        status: row.get("status"),
        output_finding_ids,
        created_at: row.get("created_at"),
    }))
}

/// List findings for a field, most recent first.
pub async fn list_field_findings(
    pool: &DbPool,
    field_id: &str,
) -> Result<Vec<StoredFinding>, ApplicationError> {
    use sqlx::Row;
    let rows = sqlx::query(
        "SELECT * FROM application_findings WHERE field_id = ? ORDER BY created_at DESC",
    )
    .bind(field_id)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        out.push(StoredFinding {
            finding_id: row.get("finding_id"),
            run_id: row.get("run_id"),
            app_id: row.get("app_id"),
            field_id: row.get("field_id"),
            finding: ApplicationFinding {
                kind: row.get("kind"),
                severity: row.get("severity"),
                confidence: row.get("confidence"),
                zone_geometry: row
                    .get::<Option<String>, _>("zone_geometry_json")
                    .and_then(|s| serde_json::from_str(&s).ok()),
                metrics: row
                    .get::<Option<String>, _>("metrics_json")
                    .and_then(|s| serde_json::from_str(&s).ok()),
                evidence_refs: row
                    .get::<Option<String>, _>("evidence_refs_json")
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default(),
            },
            created_at: row.get("created_at"),
        });
    }
    Ok(out)
}
