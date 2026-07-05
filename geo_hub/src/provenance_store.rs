//! Provenance ledger persistence and tracing (Track A batch 3).
//!
//! Producers construct evidence and lineage records; geo_hub is the only writer
//! of the ledger, and it writes at pipeline choke points (catalog product
//! registration, and — in later batches — scene ingest, finding/recommendation/
//! report creation). This module owns the SQL for `provenance_lineage_records`
//! and hydrates a [`LineageLedger`] on demand to answer backward/forward trace
//! queries with gap detection.
//!
//! The ledger table already existed (`db.rs`); this batch adds the write path
//! and the trace API that the workspace provenance inspector consumes.

use crate::db::DbPool;
use provenance::{
    ActorIdentity, BackwardProvenanceTrace, ForwardProvenanceTrace, LineageLedger, LineageRecord,
    ProvenanceParameters,
};
use serde::Serialize;
use sqlx::{Executor, Row, Sqlite};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProvenanceStoreError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("failed to serialize {what}: {source}")]
    Serialize {
        what: &'static str,
        source: serde_json::Error,
    },
    #[error("failed to decode {what}: {source}")]
    Decode {
        what: &'static str,
        source: serde_json::Error,
    },
    #[error("provenance ledger error: {0}")]
    Ledger(String),
}

/// Append a lineage record to the ledger. Idempotent on `artifact_id`
/// (`INSERT OR IGNORE`), so re-registering an already-registered product does
/// not duplicate its lineage. Accepts any sqlx executor so callers can write it
/// inside the same transaction as the domain write (see
/// [`crate::catalog::register_product`]).
pub async fn append_lineage<'e, E>(
    executor: E,
    record: &LineageRecord,
) -> Result<(), ProvenanceStoreError>
where
    E: Executor<'e, Database = Sqlite>,
{
    let inputs_json = serde_json::to_string(&record.inputs).map_err(|source| {
        ProvenanceStoreError::Serialize {
            what: "lineage inputs",
            source,
        }
    })?;
    let parameters_json = serde_json::to_string(record.parameters.as_json()).map_err(|source| {
        ProvenanceStoreError::Serialize {
            what: "lineage parameters",
            source,
        }
    })?;
    let kind = enum_to_db(&record.kind, "artifact kind")?;
    let actor_kind = enum_to_db(&record.actor.actor_kind, "actor kind")?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO provenance_lineage_records (
            artifact_id, kind, inputs_json, method, parameters_json, operator,
            actor_id, actor_kind, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&record.artifact_id)
    .bind(&kind)
    .bind(&inputs_json)
    .bind(&record.method)
    .bind(&parameters_json)
    .bind(&record.operator)
    .bind(&record.actor.actor_id)
    .bind(&actor_kind)
    .bind(&record.created_at)
    .execute(executor)
    .await?;
    Ok(())
}

/// Load every persisted lineage record.
pub async fn load_all_lineage(pool: &DbPool) -> Result<Vec<LineageRecord>, ProvenanceStoreError> {
    let rows = sqlx::query(
        r#"
        SELECT artifact_id, kind, inputs_json, method, parameters_json, operator,
               actor_id, actor_kind, created_at
        FROM provenance_lineage_records
        "#,
    )
    .fetch_all(pool)
    .await?;
    rows.iter().map(decode_lineage_record).collect()
}

/// Load a single lineage record by artifact id, if one exists. Used to validate
/// that a referenced artifact is a known, lineage-tracked entity.
pub async fn get_lineage(
    pool: &DbPool,
    artifact_id: &str,
) -> Result<Option<LineageRecord>, ProvenanceStoreError> {
    let Some(row) = sqlx::query(
        r#"
        SELECT artifact_id, kind, inputs_json, method, parameters_json, operator,
               actor_id, actor_kind, created_at
        FROM provenance_lineage_records
        WHERE artifact_id = ?
        "#,
    )
    .bind(artifact_id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(decode_lineage_record(&row)?))
}

/// Backward trace from `artifact_id` down to its L0 sources, with any missing
/// intermediate records reported as [`provenance::LineageGap`]s.
pub async fn trace_backward(
    pool: &DbPool,
    artifact_id: &str,
) -> Result<BackwardProvenanceTrace, ProvenanceStoreError> {
    let ledger = hydrate(pool).await?;
    ledger
        .trace_backward(artifact_id)
        .map_err(|err| ProvenanceStoreError::Ledger(format!("{err:?}")))
}

/// Forward trace: every record downstream of `artifact_id`.
pub async fn trace_forward(
    pool: &DbPool,
    artifact_id: &str,
) -> Result<ForwardProvenanceTrace, ProvenanceStoreError> {
    let ledger = hydrate(pool).await?;
    ledger
        .trace_forward(artifact_id)
        .map_err(|err| ProvenanceStoreError::Ledger(format!("{err:?}")))
}

async fn hydrate(pool: &DbPool) -> Result<LineageLedger, ProvenanceStoreError> {
    let records = load_all_lineage(pool).await?;
    LineageLedger::from_persisted_records(records)
        .map_err(|err| ProvenanceStoreError::Ledger(format!("{err:?}")))
}

fn decode_lineage_record(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<LineageRecord, ProvenanceStoreError> {
    let inputs_json: String = row.get("inputs_json");
    let inputs = serde_json::from_str::<Vec<String>>(&inputs_json).map_err(|source| {
        ProvenanceStoreError::Decode {
            what: "lineage inputs_json",
            source,
        }
    })?;
    let parameters_json: String = row.get("parameters_json");
    let parameters =
        serde_json::from_str::<serde_json::Value>(&parameters_json).map_err(|source| {
            ProvenanceStoreError::Decode {
                what: "lineage parameters_json",
                source,
            }
        })?;

    Ok(LineageRecord {
        artifact_id: row.get("artifact_id"),
        kind: db_to_enum(row.get::<String, _>("kind"), "artifact kind")?,
        inputs,
        method: row.get("method"),
        parameters: ProvenanceParameters::from_json(parameters),
        operator: row.get("operator"),
        actor: ActorIdentity {
            actor_id: row.get("actor_id"),
            actor_kind: db_to_enum(row.get::<String, _>("actor_kind"), "actor kind")?,
        },
        created_at: row.get("created_at"),
    })
}

/// Serialize a snake_case serde enum to its DB string form.
fn enum_to_db<T: Serialize>(value: &T, what: &'static str) -> Result<String, ProvenanceStoreError> {
    match serde_json::to_value(value)
        .map_err(|source| ProvenanceStoreError::Serialize { what, source })?
    {
        serde_json::Value::String(text) => Ok(text),
        other => Ok(other.to_string()),
    }
}

/// Parse a DB enum string back into its typed form.
fn db_to_enum<T: serde::de::DeserializeOwned>(
    value: String,
    what: &'static str,
) -> Result<T, ProvenanceStoreError> {
    serde_json::from_value(serde_json::Value::String(value))
        .map_err(|source| ProvenanceStoreError::Decode { what, source })
}
