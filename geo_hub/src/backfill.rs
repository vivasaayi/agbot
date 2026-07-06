//! Resumable historical backfill runs (batch S-11).
//!
//! A backfill run asks the pipeline to derive indices for a field over a
//! long historical range (Landsat reaches back to 1982). The range is far
//! too large for one STAC search, so a run is walked in fixed 90-day chunks
//! by a chain of `backfill_enumerate` jobs: each execution searches one
//! chunk for one dataset, fans the results into priority `-10` derive jobs,
//! advances the per-dataset cursor stored on the run, and enqueues the next
//! link of the chain. Because the cursor is durable, a paused or restarted
//! run resumes exactly where it stopped instead of re-walking the range.
//!
//! This module owns the `backfill_runs` records and the pure chunk
//! arithmetic; the enumerate job handler lives in `crate::pipeline_worker`.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use thiserror::Error;

use crate::db::DbPool;
use crate::pipeline::{self, BackfillEnumeratePayload, EnqueueOutcome, JobKind, PipelineError};

/// Days covered by one enumerate chunk. Roughly one season: small enough
/// that a STAC page holds every scene, large enough that a 40-year range
/// stays under ~170 chunks per dataset.
pub const BACKFILL_CHUNK_DAYS: i64 = 90;

/// Priority for every job a backfill produces: behind all live pipeline
/// work (cadence discover and its derives run at 0, operator triggers
/// above), so a deep historical walk never starves fresh scenes.
pub const BACKFILL_PRIORITY: i64 = -10;

/// Datasets a backfill may walk. `hls` is deliberately excluded: it has no
/// Earth Search collection to range-search (`collection_for_dataset` maps it
/// to `None`), so a backfill could never enumerate it.
pub const BACKFILLABLE_DATASETS: [&str; 2] = ["landsat", "sentinel2"];

const DEFAULT_MAX_CLOUD_COVER: f64 = 70.0;

#[derive(Debug, Error)]
pub enum BackfillError {
    #[error("backfill run {0} not found")]
    NotFound(String),
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Pipeline(#[from] PipelineError),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// A stored backfill run. `cursor` maps each dataset to the next
/// unprocessed chunk start date; a dataset absent from the map has not been
/// started (its effective cursor is `start_date`), and a dataset whose
/// cursor reached `end_date` is complete.
#[derive(Debug, Clone, Serialize)]
pub struct BackfillRun {
    pub backfill_id: String,
    pub field_id: String,
    pub datasets: Vec<String>,
    pub indices: Vec<String>,
    pub start_date: String,
    pub end_date: String,
    pub max_cloud_cover: f64,
    pub status: String,
    pub cursor: BTreeMap<String, String>,
    pub scenes_discovered: i64,
    pub jobs_enqueued: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl BackfillRun {
    /// Effective cursor for one dataset: the stored value, or `start_date`
    /// for a dataset that has not been started.
    pub fn cursor_for(&self, dataset: &str) -> &str {
        self.cursor
            .get(dataset)
            .map(String::as_str)
            .unwrap_or(&self.start_date)
    }

    /// The first dataset (in requested order) with range left to walk, and
    /// its effective cursor. `None` means every dataset is complete.
    /// ISO dates compare lexicographically, so plain string comparison is
    /// chronological.
    pub fn next_incomplete_dataset(&self) -> Option<(&str, &str)> {
        self.datasets
            .iter()
            .map(|dataset| (dataset.as_str(), self.cursor_for(dataset)))
            .find(|(_, cursor)| *cursor < self.end_date.as_str())
    }

    /// Fingerprint identifying the next chunk the enumerate chain will
    /// process (`{dataset}:{cursor}`), or `"final"` when every dataset is
    /// complete. Each chunk gets a distinct enumerate `job_key`, so the
    /// queue's dedupe cannot swallow the next link of the chain.
    pub fn enumerate_fingerprint(&self) -> String {
        match self.next_incomplete_dataset() {
            Some((dataset, cursor)) => format!("{dataset}:{cursor}"),
            None => "final".to_string(),
        }
    }
}

/// Input for [`create_backfill_run`].
#[derive(Debug, Clone, Deserialize)]
pub struct CreateBackfillRequest {
    pub field_id: String,
    pub datasets: Vec<String>,
    pub indices: Vec<String>,
    /// Inclusive range start, `YYYY-MM-DD`.
    pub start_date: String,
    /// Exclusive range end, `YYYY-MM-DD`; must be after `start_date`.
    pub end_date: String,
    pub max_cloud_cover: Option<f64>,
}

fn parse_date(value: &str, label: &str) -> Result<NaiveDate, BackfillError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        BackfillError::Validation(format!("{label} must be a YYYY-MM-DD date: got {value:?}"))
    })
}

fn run_from_row(row: &SqliteRow) -> Result<BackfillRun, BackfillError> {
    let datasets_json: String = row.get("datasets_json");
    let indices_json: String = row.get("indices_json");
    let cursor_json: Option<String> = row.get("cursor_json");
    let cursor = match cursor_json {
        Some(json) => serde_json::from_str(&json)?,
        None => BTreeMap::new(),
    };
    Ok(BackfillRun {
        backfill_id: row.get("backfill_id"),
        field_id: row.get("field_id"),
        datasets: serde_json::from_str(&datasets_json)?,
        indices: serde_json::from_str(&indices_json)?,
        start_date: row.get("start_date"),
        end_date: row.get("end_date"),
        max_cloud_cover: row.get("max_cloud_cover"),
        status: row.get("status"),
        cursor,
        scenes_discovered: row.get("scenes_discovered"),
        jobs_enqueued: row.get("jobs_enqueued"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

/// Validate and persist a new backfill run (status `running`, empty cursor).
pub async fn create_backfill_run(
    pool: &DbPool,
    request: &CreateBackfillRequest,
    now: DateTime<Utc>,
) -> Result<BackfillRun, BackfillError> {
    if request.datasets.is_empty() {
        return Err(BackfillError::Validation(
            "datasets must be a non-empty list".to_string(),
        ));
    }
    for dataset in &request.datasets {
        if !BACKFILLABLE_DATASETS.contains(&dataset.as_str()) {
            let reason = if dataset == "hls" {
                "hls has no searchable upstream collection (HLS products \
                 register through /api/ingest/hls)"
            } else {
                "unknown dataset"
            };
            return Err(BackfillError::Validation(format!(
                "dataset {dataset:?} cannot be backfilled ({reason}); \
                 supported datasets: {}",
                BACKFILLABLE_DATASETS.join(", ")
            )));
        }
    }
    if request.indices.is_empty() || request.indices.iter().any(|index| index.trim().is_empty()) {
        return Err(BackfillError::Validation(
            "indices must be a non-empty list of index keys".to_string(),
        ));
    }
    let start = parse_date(&request.start_date, "start")?;
    let end = parse_date(&request.end_date, "end")?;
    if start >= end {
        return Err(BackfillError::Validation(format!(
            "start must be before end: got {start} .. {end}"
        )));
    }
    let max_cloud_cover = request.max_cloud_cover.unwrap_or(DEFAULT_MAX_CLOUD_COVER);
    if !(0.0..=100.0).contains(&max_cloud_cover) {
        return Err(BackfillError::Validation(
            "max_cloud_cover must be within 0..=100".to_string(),
        ));
    }

    let backfill_id = format!("backfill:{}", uuid::Uuid::new_v4());
    let now_ts = pipeline::format_ts(now);
    sqlx::query(
        r#"
        INSERT INTO backfill_runs
            (backfill_id, field_id, datasets_json, indices_json, start_date,
             end_date, max_cloud_cover, status, cursor_json,
             scenes_discovered, jobs_enqueued, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, 'running', NULL, 0, 0, ?, ?)
        "#,
    )
    .bind(&backfill_id)
    .bind(&request.field_id)
    .bind(serde_json::to_string(&request.datasets)?)
    .bind(serde_json::to_string(&request.indices)?)
    .bind(&request.start_date)
    .bind(&request.end_date)
    .bind(max_cloud_cover)
    .bind(&now_ts)
    .bind(&now_ts)
    .execute(pool)
    .await?;

    get_backfill_run(pool, &backfill_id)
        .await?
        .ok_or(BackfillError::NotFound(backfill_id))
}

/// Look up one run by id.
pub async fn get_backfill_run(
    pool: &DbPool,
    backfill_id: &str,
) -> Result<Option<BackfillRun>, BackfillError> {
    let row = sqlx::query("SELECT * FROM backfill_runs WHERE backfill_id = ?")
        .bind(backfill_id)
        .fetch_optional(pool)
        .await?;
    row.as_ref().map(run_from_row).transpose()
}

/// List a field's runs, newest first.
pub async fn list_backfill_runs(
    pool: &DbPool,
    field_id: &str,
) -> Result<Vec<BackfillRun>, BackfillError> {
    let rows = sqlx::query(
        "SELECT * FROM backfill_runs WHERE field_id = ? ORDER BY created_at DESC, backfill_id DESC",
    )
    .bind(field_id)
    .fetch_all(pool)
    .await?;
    rows.iter().map(run_from_row).collect()
}

/// Move one dataset's cursor to `next_date` (the just-processed chunk's
/// end). Read-modify-write of `cursor_json` in one transaction; safe under
/// the queue's single-writer worker assumption.
pub async fn advance_cursor(
    pool: &DbPool,
    backfill_id: &str,
    dataset: &str,
    next_date: &str,
    now: DateTime<Utc>,
) -> Result<(), BackfillError> {
    let mut tx = pool.begin().await?;
    let cursor_json: Option<Option<String>> =
        sqlx::query_scalar("SELECT cursor_json FROM backfill_runs WHERE backfill_id = ?")
            .bind(backfill_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(cursor_json) = cursor_json else {
        return Err(BackfillError::NotFound(backfill_id.to_string()));
    };
    let mut cursor: BTreeMap<String, String> = match cursor_json {
        Some(json) => serde_json::from_str(&json)?,
        None => BTreeMap::new(),
    };
    cursor.insert(dataset.to_string(), next_date.to_string());
    sqlx::query("UPDATE backfill_runs SET cursor_json = ?, updated_at = ? WHERE backfill_id = ?")
        .bind(serde_json::to_string(&cursor)?)
        .bind(pipeline::format_ts(now))
        .bind(backfill_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Add one chunk's enumeration results to the run counters.
pub async fn add_progress_counts(
    pool: &DbPool,
    backfill_id: &str,
    scenes_discovered: i64,
    jobs_enqueued: i64,
    now: DateTime<Utc>,
) -> Result<(), BackfillError> {
    let result = sqlx::query(
        r#"
        UPDATE backfill_runs
        SET scenes_discovered = scenes_discovered + ?,
            jobs_enqueued = jobs_enqueued + ?, updated_at = ?
        WHERE backfill_id = ?
        "#,
    )
    .bind(scenes_discovered)
    .bind(jobs_enqueued)
    .bind(pipeline::format_ts(now))
    .bind(backfill_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(BackfillError::NotFound(backfill_id.to_string()));
    }
    Ok(())
}

/// Statuses a run may hold.
pub const BACKFILL_STATUSES: [&str; 4] = ["running", "paused", "completed", "failed"];

/// Set a run's status (`running` / `paused` / `completed` / `failed`).
pub async fn set_status(
    pool: &DbPool,
    backfill_id: &str,
    status: &str,
    now: DateTime<Utc>,
) -> Result<(), BackfillError> {
    if !BACKFILL_STATUSES.contains(&status) {
        return Err(BackfillError::Validation(format!(
            "status must be one of {}: got {status}",
            BACKFILL_STATUSES.join(", ")
        )));
    }
    let result =
        sqlx::query("UPDATE backfill_runs SET status = ?, updated_at = ? WHERE backfill_id = ?")
            .bind(status)
            .bind(pipeline::format_ts(now))
            .bind(backfill_id)
            .execute(pool)
            .await?;
    if result.rows_affected() == 0 {
        return Err(BackfillError::NotFound(backfill_id.to_string()));
    }
    Ok(())
}

/// Pipeline-job counts for one run, grouped by queue status.
#[derive(Debug, Clone, Serialize)]
pub struct BackfillProgress {
    pub by_status: BTreeMap<String, i64>,
    pub total: i64,
}

/// Count the run's pipeline jobs by status (uses the
/// `idx_pipeline_jobs_backfill` index).
pub async fn progress(pool: &DbPool, backfill_id: &str) -> Result<BackfillProgress, BackfillError> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, COUNT(*) FROM pipeline_jobs WHERE backfill_id = ? GROUP BY status",
    )
    .bind(backfill_id)
    .fetch_all(pool)
    .await?;
    let by_status: BTreeMap<String, i64> = rows.into_iter().collect();
    let total = by_status.values().sum();
    Ok(BackfillProgress { by_status, total })
}

/// Enqueue the run's next enumerate job under its current chunk
/// fingerprint. Used by the create route (first link), the resume route
/// (restart the parked chain), and the worker (chain the next chunk).
pub async fn enqueue_enumerate_job(
    pool: &DbPool,
    run: &BackfillRun,
    run_after: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<EnqueueOutcome, BackfillError> {
    let payload = serde_json::to_value(BackfillEnumeratePayload {
        backfill_id: run.backfill_id.clone(),
    })?;
    let job_key = pipeline::backfill_enum_job_key(&run.backfill_id, &run.enumerate_fingerprint());
    Ok(pipeline::enqueue_job(
        pool,
        JobKind::BackfillEnumerate,
        &job_key,
        &payload,
        Some(&run.field_id),
        None,
        BACKFILL_PRIORITY,
        run_after,
        Some(&run.backfill_id),
        now,
    )
    .await?)
}

// ---------------------------------------------------------------------------
// Pure chunk arithmetic
// ---------------------------------------------------------------------------

/// The next `[chunk_start, chunk_end)` window of at most `chunk_days` days:
/// starts at `cursor_or_start`, capped at `end`. `None` when the range is
/// exhausted (or the inputs are not valid `YYYY-MM-DD` dates / a
/// non-positive chunk size).
pub fn next_chunk(cursor_or_start: &str, end: &str, chunk_days: i64) -> Option<(String, String)> {
    let start = NaiveDate::parse_from_str(cursor_or_start, "%Y-%m-%d").ok()?;
    let end = NaiveDate::parse_from_str(end, "%Y-%m-%d").ok()?;
    if start >= end || chunk_days <= 0 {
        return None;
    }
    let chunk_end = (start + chrono::Duration::days(chunk_days)).min(end);
    Some((start.to_string(), chunk_end.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_chunk_walks_an_exact_multiple_of_the_chunk_size() {
        // 180 days = exactly two 90-day chunks.
        let (s1, e1) = next_chunk("2024-01-01", "2024-06-29", 90).unwrap();
        assert_eq!((s1.as_str(), e1.as_str()), ("2024-01-01", "2024-03-31"));
        let (s2, e2) = next_chunk(&e1, "2024-06-29", 90).unwrap();
        assert_eq!((s2.as_str(), e2.as_str()), ("2024-03-31", "2024-06-29"));
        assert_eq!(next_chunk(&e2, "2024-06-29", 90), None, "range exhausted");
    }

    #[test]
    fn next_chunk_caps_the_partial_tail_at_end() {
        let (s1, e1) = next_chunk("2024-01-01", "2024-04-01", 90).unwrap();
        assert_eq!((s1.as_str(), e1.as_str()), ("2024-01-01", "2024-03-31"));
        let (s2, e2) = next_chunk(&e1, "2024-04-01", 90).unwrap();
        assert_eq!((s2.as_str(), e2.as_str()), ("2024-03-31", "2024-04-01"));
        assert_eq!(next_chunk(&e2, "2024-04-01", 90), None);
    }

    #[test]
    fn next_chunk_empty_or_inverted_range_yields_none() {
        assert_eq!(next_chunk("2024-01-01", "2024-01-01", 90), None);
        assert_eq!(next_chunk("2024-04-01", "2024-01-01", 90), None);
    }

    #[test]
    fn next_chunk_rejects_garbage_inputs() {
        assert_eq!(next_chunk("junk", "2024-01-01", 90), None);
        assert_eq!(next_chunk("2024-01-01", "junk", 90), None);
        assert_eq!(next_chunk("2024-01-01", "2024-04-01", 0), None);
        assert_eq!(next_chunk("2024-01-01", "2024-04-01", -7), None);
    }

    fn run_fixture(datasets: &[&str], cursor: &[(&str, &str)]) -> BackfillRun {
        BackfillRun {
            backfill_id: "backfill:test".to_string(),
            field_id: "field-1".to_string(),
            datasets: datasets.iter().map(|s| s.to_string()).collect(),
            indices: vec!["ndvi".to_string()],
            start_date: "2024-01-01".to_string(),
            end_date: "2024-04-01".to_string(),
            max_cloud_cover: 70.0,
            status: "running".to_string(),
            cursor: cursor
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            scenes_discovered: 0,
            jobs_enqueued: 0,
            created_at: "2026-07-01T00:00:00Z".to_string(),
            updated_at: "2026-07-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn next_incomplete_dataset_walks_in_requested_order() {
        let fresh = run_fixture(&["sentinel2", "landsat"], &[]);
        assert_eq!(
            fresh.next_incomplete_dataset(),
            Some(("sentinel2", "2024-01-01")),
            "unstarted dataset uses start_date"
        );

        let mid = run_fixture(&["sentinel2", "landsat"], &[("sentinel2", "2024-03-31")]);
        assert_eq!(
            mid.next_incomplete_dataset(),
            Some(("sentinel2", "2024-03-31"))
        );

        let second = run_fixture(&["sentinel2", "landsat"], &[("sentinel2", "2024-04-01")]);
        assert_eq!(
            second.next_incomplete_dataset(),
            Some(("landsat", "2024-01-01")),
            "first dataset complete -> second starts"
        );

        let done = run_fixture(
            &["sentinel2", "landsat"],
            &[("sentinel2", "2024-04-01"), ("landsat", "2024-04-01")],
        );
        assert_eq!(done.next_incomplete_dataset(), None);
        assert_eq!(done.enumerate_fingerprint(), "final");
        assert_eq!(mid.enumerate_fingerprint(), "sentinel2:2024-03-31");
    }
}
