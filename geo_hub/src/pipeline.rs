//! Satellite pipeline job queue and subscriptions (batch S-6).
//!
//! A durable, SQLite-backed work queue that drives the satellite pipeline:
//! per-field dataset subscriptions produce `discover` jobs, discoveries fan
//! out into `derive` jobs, derivations trigger `l3_recompute` and `app_run`
//! jobs. Every job carries a deterministic `job_key`, so re-triggering the
//! same logical unit of work deduplicates instead of duplicating.
//!
//! Concurrency model: SQLite with WAL allows many readers plus one writer.
//! The queue assumes a **single worker process** claims jobs (the geo_hub
//! pipeline worker); enqueue is safe from request handlers because every
//! read-modify-write here runs inside one transaction and the connection has
//! a 5s busy timeout (see `db::connect_pool`).

use crate::db::DbPool;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use thiserror::Error;

/// Base retry backoff: a failed attempt is retried after
/// `BACKOFF_BASE_SECS * 2^attempts` seconds.
const BACKOFF_BASE_SECS: i64 = 60;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("pipeline job {0} not found")]
    JobNotFound(String),
    #[error("subscription {0} not found")]
    SubscriptionNotFound(String),
    #[error("job {job_id} is not retryable from status {status}")]
    NotRetryable { job_id: String, status: String },
    #[error("unknown enum value: {0}")]
    UnknownEnum(String),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Canonical timestamp format for queue columns: RFC 3339 UTC with second
/// precision and a `Z` suffix, so lexicographic comparison in SQL matches
/// chronological order.
pub fn format_ts(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}

// ---------------------------------------------------------------------------
// Job kinds, statuses, and payloads
// ---------------------------------------------------------------------------

/// The unit of pipeline work a job represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    /// Query the upstream catalog for new items covering a field.
    Discover,
    /// Derive one index raster from one catalog item for one field.
    Derive,
    /// Recompute an L3 monthly rollup after new derivations land.
    L3Recompute,
    /// Run a downstream application (drought watch, crop health, ...).
    AppRun,
    /// Expand a historical backfill request into discover jobs.
    BackfillEnumerate,
}

impl JobKind {
    pub fn as_str(self) -> &'static str {
        match self {
            JobKind::Discover => "discover",
            JobKind::Derive => "derive",
            JobKind::L3Recompute => "l3_recompute",
            JobKind::AppRun => "app_run",
            JobKind::BackfillEnumerate => "backfill_enumerate",
        }
    }

    pub fn parse(value: &str) -> Result<Self, PipelineError> {
        match value {
            "discover" => Ok(JobKind::Discover),
            "derive" => Ok(JobKind::Derive),
            "l3_recompute" => Ok(JobKind::L3Recompute),
            "app_run" => Ok(JobKind::AppRun),
            "backfill_enumerate" => Ok(JobKind::BackfillEnumerate),
            other => Err(PipelineError::UnknownEnum(other.to_string())),
        }
    }
}

/// Queue lifecycle state of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Dead,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Succeeded => "succeeded",
            JobStatus::Failed => "failed",
            JobStatus::Dead => "dead",
        }
    }

    pub fn parse(value: &str) -> Result<Self, PipelineError> {
        match value {
            "queued" => Ok(JobStatus::Queued),
            "running" => Ok(JobStatus::Running),
            "succeeded" => Ok(JobStatus::Succeeded),
            "failed" => Ok(JobStatus::Failed),
            "dead" => Ok(JobStatus::Dead),
            other => Err(PipelineError::UnknownEnum(other.to_string())),
        }
    }

    /// Terminal states: re-enqueueing the same job_key resets the job.
    fn is_terminal(self) -> bool {
        matches!(
            self,
            JobStatus::Succeeded | JobStatus::Failed | JobStatus::Dead
        )
    }
}

/// A full pipeline job row.
#[derive(Debug, Clone, Serialize)]
pub struct PipelineJob {
    pub job_id: String,
    pub job_key: String,
    pub kind: JobKind,
    pub field_id: Option<String>,
    pub dataset: Option<String>,
    pub payload_json: String,
    pub status: JobStatus,
    pub priority: i64,
    pub attempts: i64,
    pub max_attempts: i64,
    pub run_after: String,
    pub backfill_id: Option<String>,
    pub parent_job_id: Option<String>,
    pub claimed_at: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Payload for a [`JobKind::Discover`] job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverPayload {
    pub field_id: String,
    pub dataset: String,
}

/// Payload for a [`JobKind::Derive`] job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivePayload {
    pub dataset: String,
    pub collection: String,
    pub item_id: String,
    pub index: String,
    pub field_id: String,
    #[serde(default)]
    pub season_id: Option<String>,
    pub aoi_geojson: serde_json::Value,
}

/// Payload for a [`JobKind::L3Recompute`] job.
///
/// `product` selects which L3 the recompute produces (batch S-12):
/// `monthly_composite` (default), `climatology`, `phenology`, or
/// `drought_stack`. Older `l3_recompute` payloads written before S-12 have no
/// `product` field and deserialize as `monthly_composite`, preserving the
/// original monthly-composite behaviour byte-for-byte.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L3RecomputePayload {
    pub field_id: String,
    pub dataset: String,
    pub index: String,
    /// Month bucket in `YYYY-MM` form (the season-end month for phenology).
    pub month: String,
    /// Which L3 to (re)compute: `monthly_composite` | `climatology` |
    /// `phenology` | `drought_stack`.
    #[serde(default = "default_l3_product")]
    pub product: String,
}

/// Default `product` for an [`L3RecomputePayload`] with no explicit selector.
pub fn default_l3_product() -> String {
    "monthly_composite".to_string()
}

/// Payload for a [`JobKind::AppRun`] job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRunPayload {
    pub app_id: String,
    pub field_id: String,
    /// Run date in `YYYY-MM-DD` form.
    pub date: String,
}

/// Payload for a [`JobKind::BackfillEnumerate`] job. Deliberately just the
/// run id: the durable `backfill_runs` row (range, datasets, cursor) is the
/// source of truth, so a re-executed job always reads the current cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillEnumeratePayload {
    pub backfill_id: String,
}

/// Result of an enqueue attempt against the deduplicating queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnqueueOutcome {
    Enqueued,
    Deduplicated,
}

// ---------------------------------------------------------------------------
// Deterministic job keys
// ---------------------------------------------------------------------------

/// Job key for discovering new catalog items for one field/dataset.
pub fn discover_job_key(field_id: &str, dataset: &str) -> String {
    format!("discover:{dataset}:{field_id}")
}

/// Job key for deriving one index from one catalog item for one field.
pub fn derive_job_key(dataset: &str, item_id: &str, index: &str, field_id: &str) -> String {
    format!("derive:{dataset}:{item_id}:{index}:{field_id}")
}

/// Job key for an L3 monthly composite rollup recompute (`month` is
/// `YYYY-MM`). This is the `monthly_composite` product's key; its format is
/// preserved from before S-12 so in-flight jobs keep deduplicating.
pub fn l3_job_key(field_id: &str, dataset: &str, index: &str, month: &str) -> String {
    format!("l3:{dataset}:{index}:{field_id}:{month}")
}

/// Job key for a non-composite L3 suite recompute (batch S-12:
/// `climatology` | `phenology` | `drought_stack`). The product is folded into
/// the index token (`{index}_{product}`) so dedupe stays distinct per
/// (field, product, bucket) while sharing the `l3:` debounce prefix. `bucket`
/// is a `YYYY-MM` month (composite/drought) or a season label (phenology).
///
/// `monthly_composite` intentionally does not route here — it keeps the
/// legacy [`l3_job_key`] format for byte-compatible dedupe.
pub fn l3_suite_job_key(
    field_id: &str,
    dataset: &str,
    index: &str,
    product: &str,
    bucket: &str,
) -> String {
    format!("l3:{dataset}:{index}_{product}:{field_id}:{bucket}")
}

/// Job key for a downstream application run (`date` is `YYYY-MM-DD`).
pub fn app_job_key(app_id: &str, field_id: &str, date: &str) -> String {
    format!("app:{app_id}:{field_id}:{date}")
}

/// Job key for one link of a backfill's enumerate chain. `fingerprint`
/// identifies the next chunk (`{dataset}:{cursor_date}`, or `final`), so
/// each link has a distinct key and dedupe cannot swallow the chain, while
/// a crashed link re-enqueued under the same cursor still deduplicates.
pub fn backfill_enum_job_key(backfill_id: &str, fingerprint: &str) -> String {
    format!("backfill_enum:{backfill_id}:{fingerprint}")
}

/// True when the job key belongs to an L3 recompute, which debounces instead
/// of plain no-op deduplication when re-enqueued while still queued.
fn is_l3_key(job_key: &str) -> bool {
    job_key.starts_with("l3:")
}

// ---------------------------------------------------------------------------
// Queue operations
// ---------------------------------------------------------------------------

fn job_from_row(row: &SqliteRow) -> Result<PipelineJob, PipelineError> {
    Ok(PipelineJob {
        job_id: row.get("job_id"),
        job_key: row.get("job_key"),
        kind: JobKind::parse(&row.get::<String, _>("kind"))?,
        field_id: row.get("field_id"),
        dataset: row.get("dataset"),
        payload_json: row.get("payload_json"),
        status: JobStatus::parse(&row.get::<String, _>("status"))?,
        priority: row.get("priority"),
        attempts: row.get("attempts"),
        max_attempts: row.get("max_attempts"),
        run_after: row.get("run_after"),
        backfill_id: row.get("backfill_id"),
        parent_job_id: row.get("parent_job_id"),
        claimed_at: row.get("claimed_at"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        last_error: row.get("last_error"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

/// Enqueue a job, deduplicating on `job_key`.
///
/// - No existing row: insert as `queued` -> [`EnqueueOutcome::Enqueued`].
/// - Existing row in a terminal state (succeeded / failed / dead): reset it to
///   `queued` with fresh payload, `attempts = 0`, the new `run_after`, and
///   cleared errors -> [`EnqueueOutcome::Enqueued`].
/// - Existing row still `queued` and the key is an `l3:*` debounce key: push
///   `run_after` out to the later of the existing and new values ->
///   [`EnqueueOutcome::Deduplicated`].
/// - Otherwise (`queued` non-l3, or `running`): no-op ->
///   [`EnqueueOutcome::Deduplicated`].
///
/// Implemented as SELECT + conditional UPDATE/INSERT inside one transaction;
/// safe under the module's single-writer assumption documented above.
#[allow(clippy::too_many_arguments)]
pub async fn enqueue_job(
    pool: &DbPool,
    kind: JobKind,
    job_key: &str,
    payload: &serde_json::Value,
    field_id: Option<&str>,
    dataset: Option<&str>,
    priority: i64,
    run_after: DateTime<Utc>,
    backfill_id: Option<&str>,
    now: DateTime<Utc>,
) -> Result<EnqueueOutcome, PipelineError> {
    let payload_json = serde_json::to_string(payload)?;
    let run_after_ts = format_ts(run_after);
    let now_ts = format_ts(now);

    let mut tx = pool.begin().await?;

    let existing =
        sqlx::query("SELECT job_id, status, run_after FROM pipeline_jobs WHERE job_key = ?")
            .bind(job_key)
            .fetch_optional(&mut *tx)
            .await?;

    let outcome = match existing {
        None => {
            let job_id = format!("job:{}", uuid::Uuid::new_v4());
            sqlx::query(
                r#"
                INSERT INTO pipeline_jobs
                    (job_id, job_key, kind, field_id, dataset, payload_json,
                     status, priority, attempts, max_attempts, run_after,
                     backfill_id, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, 'queued', ?, 0, 3, ?, ?, ?, ?)
                "#,
            )
            .bind(&job_id)
            .bind(job_key)
            .bind(kind.as_str())
            .bind(field_id)
            .bind(dataset)
            .bind(&payload_json)
            .bind(priority)
            .bind(&run_after_ts)
            .bind(backfill_id)
            .bind(&now_ts)
            .bind(&now_ts)
            .execute(&mut *tx)
            .await?;
            EnqueueOutcome::Enqueued
        }
        Some(row) => {
            let job_id: String = row.get("job_id");
            let status = JobStatus::parse(&row.get::<String, _>("status"))?;
            let existing_run_after: String = row.get("run_after");
            if status.is_terminal() {
                sqlx::query(
                    r#"
                    UPDATE pipeline_jobs
                    SET status = 'queued', payload_json = ?, priority = ?,
                        attempts = 0, run_after = ?, backfill_id = ?,
                        claimed_at = NULL, started_at = NULL,
                        finished_at = NULL, last_error = NULL, updated_at = ?
                    WHERE job_id = ?
                    "#,
                )
                .bind(&payload_json)
                .bind(priority)
                .bind(&run_after_ts)
                .bind(backfill_id)
                .bind(&now_ts)
                .bind(&job_id)
                .execute(&mut *tx)
                .await?;
                EnqueueOutcome::Enqueued
            } else if status == JobStatus::Queued
                && is_l3_key(job_key)
                && run_after_ts > existing_run_after
            {
                // Debounce: each new upstream event pushes the pending
                // recompute out, so a burst of derivations rolls up once.
                sqlx::query(
                    "UPDATE pipeline_jobs SET run_after = ?, updated_at = ? WHERE job_id = ?",
                )
                .bind(&run_after_ts)
                .bind(&now_ts)
                .bind(&job_id)
                .execute(&mut *tx)
                .await?;
                EnqueueOutcome::Deduplicated
            } else {
                EnqueueOutcome::Deduplicated
            }
        }
    };

    tx.commit().await?;
    Ok(outcome)
}

/// Claim the next ready job: highest priority first, then oldest. Marks the
/// job `running` and increments its attempt counter in one statement
/// (UPDATE ... RETURNING, supported by the bundled SQLite >= 3.35).
pub async fn claim_next_job(
    pool: &DbPool,
    now: DateTime<Utc>,
) -> Result<Option<PipelineJob>, PipelineError> {
    let now_ts = format_ts(now);
    let row = sqlx::query(
        r#"
        UPDATE pipeline_jobs
        SET status = 'running', claimed_at = ?, started_at = ?,
            attempts = attempts + 1, updated_at = ?
        WHERE job_id = (
            SELECT job_id FROM pipeline_jobs
            WHERE status = 'queued' AND run_after <= ?
            ORDER BY priority DESC, created_at ASC, job_id ASC
            LIMIT 1
        )
        RETURNING *
        "#,
    )
    .bind(&now_ts)
    .bind(&now_ts)
    .bind(&now_ts)
    .bind(&now_ts)
    .fetch_optional(pool)
    .await?;

    row.as_ref().map(job_from_row).transpose()
}

/// Mark a running job as succeeded.
pub async fn complete_job(
    pool: &DbPool,
    job_id: &str,
    now: DateTime<Utc>,
) -> Result<(), PipelineError> {
    let now_ts = format_ts(now);
    let result = sqlx::query(
        "UPDATE pipeline_jobs SET status = 'succeeded', finished_at = ?, updated_at = ? WHERE job_id = ?",
    )
    .bind(&now_ts)
    .bind(&now_ts)
    .bind(job_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(PipelineError::JobNotFound(job_id.to_string()));
    }
    Ok(())
}

/// Record a failed attempt.
///
/// - Client errors (bad request / permanent upstream rejection) go straight
///   to `dead`: retrying cannot help.
/// - Transient errors retry with exponential backoff
///   (`run_after = now + 60s * 2^attempts`) until `max_attempts` is reached,
///   then go `dead`.
pub async fn fail_job(
    pool: &DbPool,
    job: &PipelineJob,
    error_msg: &str,
    is_client_error: bool,
    now: DateTime<Utc>,
) -> Result<(), PipelineError> {
    let now_ts = format_ts(now);
    let retryable = !is_client_error && job.attempts < job.max_attempts;

    let result = if retryable {
        let backoff_secs = BACKOFF_BASE_SECS * 2_i64.pow(job.attempts.clamp(0, 30) as u32);
        let run_after_ts = format_ts(now + Duration::seconds(backoff_secs));
        sqlx::query(
            r#"
            UPDATE pipeline_jobs
            SET status = 'queued', run_after = ?, last_error = ?, updated_at = ?
            WHERE job_id = ?
            "#,
        )
        .bind(&run_after_ts)
        .bind(error_msg)
        .bind(&now_ts)
        .bind(&job.job_id)
        .execute(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            UPDATE pipeline_jobs
            SET status = 'dead', last_error = ?, finished_at = ?, updated_at = ?
            WHERE job_id = ?
            "#,
        )
        .bind(error_msg)
        .bind(&now_ts)
        .bind(&now_ts)
        .bind(&job.job_id)
        .execute(pool)
        .await?
    };
    if result.rows_affected() == 0 {
        return Err(PipelineError::JobNotFound(job.job_id.clone()));
    }
    Ok(())
}

/// Outcome of the startup orphan sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OrphanRecovery {
    /// Orphaned running jobs put back on the queue for another attempt.
    pub requeued: usize,
    /// Orphaned running jobs buried because they had already exhausted their
    /// attempts — a job that keeps taking the worker down mid-run is a poison
    /// pill, not a transient failure.
    pub dead_lettered: usize,
}

/// Recover jobs left `running` by a crashed or killed worker (called on worker
/// startup, before the poll loop).
///
/// [`claim_next_job`] charges an attempt up front, so a job found `running`
/// after a crash has already consumed one. This matters under
/// `panic = "abort"` (the release profile): a handler panic takes the whole
/// process down, the container restarts, and a naive re-queue would re-run the
/// same poison job and crash again — an unbounded crash loop that starves every
/// other job. So a job whose `attempts` have reached `max_attempts` is
/// dead-lettered here instead of re-queued; the rest are re-queued for a normal
/// retry, and the worker keeps draining.
pub async fn reset_orphaned_running_jobs(pool: &DbPool) -> Result<OrphanRecovery, PipelineError> {
    let now_ts = format_ts(Utc::now());

    // Bury poison jobs first (they are still `running`), then re-queue the rest.
    let dead = sqlx::query(
        r#"
        UPDATE pipeline_jobs
        SET status = 'dead',
            last_error = 'worker exited while running this job; attempts exhausted',
            finished_at = ?, updated_at = ?
        WHERE status = 'running' AND attempts >= max_attempts
        "#,
    )
    .bind(&now_ts)
    .bind(&now_ts)
    .execute(pool)
    .await?;

    let requeued = sqlx::query(
        r#"
        UPDATE pipeline_jobs
        SET status = 'queued', claimed_at = NULL, updated_at = ?
        WHERE status = 'running'
        "#,
    )
    .bind(&now_ts)
    .execute(pool)
    .await?;

    Ok(OrphanRecovery {
        requeued: requeued.rows_affected() as usize,
        dead_lettered: dead.rows_affected() as usize,
    })
}

/// List jobs, optionally filtered to one field, oldest first.
pub async fn list_jobs(
    pool: &DbPool,
    field_id: Option<&str>,
) -> Result<Vec<PipelineJob>, PipelineError> {
    let rows = match field_id {
        Some(field_id) => sqlx::query(
            "SELECT * FROM pipeline_jobs WHERE field_id = ? ORDER BY created_at ASC, job_key ASC",
        )
        .bind(field_id)
        .fetch_all(pool)
        .await?,
        None => {
            sqlx::query("SELECT * FROM pipeline_jobs ORDER BY created_at ASC, job_key ASC")
                .fetch_all(pool)
                .await?
        }
    };
    rows.iter().map(job_from_row).collect()
}

/// Look up one job by id.
pub async fn get_job(pool: &DbPool, job_id: &str) -> Result<Option<PipelineJob>, PipelineError> {
    let row = sqlx::query("SELECT * FROM pipeline_jobs WHERE job_id = ?")
        .bind(job_id)
        .fetch_optional(pool)
        .await?;
    row.as_ref().map(job_from_row).transpose()
}

/// Look up one job by its deterministic job key.
pub async fn find_job_by_key(
    pool: &DbPool,
    job_key: &str,
) -> Result<Option<PipelineJob>, PipelineError> {
    let row = sqlx::query("SELECT * FROM pipeline_jobs WHERE job_key = ?")
        .bind(job_key)
        .fetch_optional(pool)
        .await?;
    row.as_ref().map(job_from_row).transpose()
}

/// Filters for the job listing API.
#[derive(Debug, Clone, Default)]
pub struct JobFilter {
    pub field_id: Option<String>,
    pub status: Option<JobStatus>,
    pub limit: Option<i64>,
}

/// List jobs newest first for the API: optional field/status filters, with a
/// limit (default 100, clamped to 1..=500).
pub async fn list_jobs_filtered(
    pool: &DbPool,
    filter: &JobFilter,
) -> Result<Vec<PipelineJob>, PipelineError> {
    let limit = filter.limit.unwrap_or(100).clamp(1, 500);
    let mut sql = String::from("SELECT * FROM pipeline_jobs WHERE 1 = 1");
    if filter.field_id.is_some() {
        sql.push_str(" AND field_id = ?");
    }
    if filter.status.is_some() {
        sql.push_str(" AND status = ?");
    }
    sql.push_str(" ORDER BY created_at DESC, job_key DESC LIMIT ?");

    let mut query = sqlx::query(&sql);
    if let Some(field_id) = &filter.field_id {
        query = query.bind(field_id);
    }
    if let Some(status) = filter.status {
        query = query.bind(status.as_str());
    }
    let rows = query.bind(limit).fetch_all(pool).await?;
    rows.iter().map(job_from_row).collect()
}

/// Reset a `dead` or `failed` job back to `queued` for an immediate re-run
/// (operator retry). Attempts and errors are cleared, mirroring the terminal
/// re-enqueue reset in [`enqueue_job`].
pub async fn retry_job(
    pool: &DbPool,
    job_id: &str,
    now: DateTime<Utc>,
) -> Result<PipelineJob, PipelineError> {
    let job = get_job(pool, job_id)
        .await?
        .ok_or_else(|| PipelineError::JobNotFound(job_id.to_string()))?;
    if !matches!(job.status, JobStatus::Dead | JobStatus::Failed) {
        return Err(PipelineError::NotRetryable {
            job_id: job_id.to_string(),
            status: job.status.as_str().to_string(),
        });
    }
    let now_ts = format_ts(now);
    sqlx::query(
        r#"
        UPDATE pipeline_jobs
        SET status = 'queued', attempts = 0, run_after = ?,
            claimed_at = NULL, started_at = NULL, finished_at = NULL,
            last_error = NULL, updated_at = ?
        WHERE job_id = ?
        "#,
    )
    .bind(&now_ts)
    .bind(&now_ts)
    .bind(job_id)
    .execute(pool)
    .await?;
    get_job(pool, job_id)
        .await?
        .ok_or_else(|| PipelineError::JobNotFound(job_id.to_string()))
}

// ---------------------------------------------------------------------------
// Subscriptions
// ---------------------------------------------------------------------------

/// Desired state for one field/dataset subscription (upsert input).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionUpsert {
    pub field_id: String,
    pub dataset: String,
    pub indices: Vec<String>,
    pub cadence_hours: i64,
    pub max_cloud_cover: f64,
    pub lookback_days: i64,
}

/// A stored field/dataset subscription.
#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionRecord {
    pub subscription_id: String,
    pub field_id: String,
    pub dataset: String,
    pub indices: Vec<String>,
    pub cadence_hours: i64,
    pub max_cloud_cover: f64,
    pub lookback_days: i64,
    pub status: String,
    pub last_checked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn subscription_id_for(field_id: &str, dataset: &str) -> String {
    format!("sub:{dataset}:{field_id}")
}

fn subscription_from_row(row: &SqliteRow) -> Result<SubscriptionRecord, PipelineError> {
    let indices_json: String = row.get("indices_json");
    Ok(SubscriptionRecord {
        subscription_id: row.get("subscription_id"),
        field_id: row.get("field_id"),
        dataset: row.get("dataset"),
        indices: serde_json::from_str(&indices_json)?,
        cadence_hours: row.get("cadence_hours"),
        max_cloud_cover: row.get("max_cloud_cover"),
        lookback_days: row.get("lookback_days"),
        status: row.get("status"),
        last_checked_at: row.get("last_checked_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

/// Create or update the subscription for `(field_id, dataset)`. Updates keep
/// the subscription id, status, and `last_checked_at`; the watch parameters
/// (indices, cadence, cloud cover, lookback) take the new values.
pub async fn upsert_subscription(
    pool: &DbPool,
    upsert: &SubscriptionUpsert,
    now: DateTime<Utc>,
) -> Result<SubscriptionRecord, PipelineError> {
    let subscription_id = subscription_id_for(&upsert.field_id, &upsert.dataset);
    let indices_json = serde_json::to_string(&upsert.indices)?;
    let now_ts = format_ts(now);

    sqlx::query(
        r#"
        INSERT INTO satellite_subscriptions
            (subscription_id, field_id, dataset, indices_json, cadence_hours,
             max_cloud_cover, lookback_days, status, last_checked_at,
             created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, 'active', NULL, ?, ?)
        ON CONFLICT(field_id, dataset) DO UPDATE SET
            indices_json = excluded.indices_json,
            cadence_hours = excluded.cadence_hours,
            max_cloud_cover = excluded.max_cloud_cover,
            lookback_days = excluded.lookback_days,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&subscription_id)
    .bind(&upsert.field_id)
    .bind(&upsert.dataset)
    .bind(&indices_json)
    .bind(upsert.cadence_hours)
    .bind(upsert.max_cloud_cover)
    .bind(upsert.lookback_days)
    .bind(&now_ts)
    .bind(&now_ts)
    .execute(pool)
    .await?;

    let row =
        sqlx::query("SELECT * FROM satellite_subscriptions WHERE field_id = ? AND dataset = ?")
            .bind(&upsert.field_id)
            .bind(&upsert.dataset)
            .fetch_one(pool)
            .await?;
    subscription_from_row(&row)
}

/// List subscriptions, optionally scoped to one field.
pub async fn list_subscriptions(
    pool: &DbPool,
    field_id: Option<&str>,
) -> Result<Vec<SubscriptionRecord>, PipelineError> {
    let rows = match field_id {
        Some(field_id) => sqlx::query(
            "SELECT * FROM satellite_subscriptions WHERE field_id = ? ORDER BY subscription_id ASC",
        )
        .bind(field_id)
        .fetch_all(pool)
        .await?,
        None => {
            sqlx::query("SELECT * FROM satellite_subscriptions ORDER BY subscription_id ASC")
                .fetch_all(pool)
                .await?
        }
    };
    rows.iter().map(subscription_from_row).collect()
}

/// Look up the subscription for one `(field_id, dataset)` pair.
pub async fn find_subscription(
    pool: &DbPool,
    field_id: &str,
    dataset: &str,
) -> Result<Option<SubscriptionRecord>, PipelineError> {
    let row =
        sqlx::query("SELECT * FROM satellite_subscriptions WHERE field_id = ? AND dataset = ?")
            .bind(field_id)
            .bind(dataset)
            .fetch_optional(pool)
            .await?;
    row.as_ref().map(subscription_from_row).transpose()
}

/// Set a subscription's status (`active` / `paused`).
pub async fn set_subscription_status(
    pool: &DbPool,
    subscription_id: &str,
    status: &str,
    now: DateTime<Utc>,
) -> Result<(), PipelineError> {
    let result = sqlx::query(
        "UPDATE satellite_subscriptions SET status = ?, updated_at = ? WHERE subscription_id = ?",
    )
    .bind(status)
    .bind(format_ts(now))
    .bind(subscription_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(PipelineError::SubscriptionNotFound(
            subscription_id.to_string(),
        ));
    }
    Ok(())
}

/// Active subscriptions whose cadence has elapsed: never checked, or
/// `last_checked_at + cadence_hours < now`. The cadence arithmetic runs in
/// Rust on the fetched active rows, keeping the SQL trivial.
pub async fn due_subscriptions(
    pool: &DbPool,
    now: DateTime<Utc>,
) -> Result<Vec<SubscriptionRecord>, PipelineError> {
    let active = sqlx::query(
        "SELECT * FROM satellite_subscriptions WHERE status = 'active' ORDER BY subscription_id ASC",
    )
    .fetch_all(pool)
    .await?;

    let mut due = Vec::new();
    for row in &active {
        let record = subscription_from_row(row)?;
        let is_due = match &record.last_checked_at {
            None => true,
            Some(checked_ts) => match DateTime::parse_from_rfc3339(checked_ts) {
                Ok(checked) => {
                    checked.with_timezone(&Utc) + Duration::hours(record.cadence_hours) < now
                }
                // An unparseable timestamp should not silently starve the
                // subscription; treat it as due so the next check repairs it.
                Err(_) => true,
            },
        };
        if is_due {
            due.push(record);
        }
    }
    Ok(due)
}

/// Record that a subscription's discover check ran now.
pub async fn touch_subscription_checked(
    pool: &DbPool,
    subscription_id: &str,
    now: DateTime<Utc>,
) -> Result<(), PipelineError> {
    let now_ts = format_ts(now);
    let result = sqlx::query(
        "UPDATE satellite_subscriptions SET last_checked_at = ?, updated_at = ? WHERE subscription_id = ?",
    )
    .bind(&now_ts)
    .bind(&now_ts)
    .bind(subscription_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(PipelineError::SubscriptionNotFound(
            subscription_id.to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_job_key_is_deterministic_per_field_and_dataset() {
        assert_eq!(
            discover_job_key("field-1", "sentinel-2-l2a"),
            "discover:sentinel-2-l2a:field-1"
        );
    }

    #[test]
    fn derive_job_key_includes_item_index_and_field() {
        assert_eq!(
            derive_job_key("sentinel-2-l2a", "S2A_31TCJ_20260601", "ndvi", "field-1"),
            "derive:sentinel-2-l2a:S2A_31TCJ_20260601:ndvi:field-1"
        );
    }

    #[test]
    fn l3_job_key_buckets_by_month_and_is_debounceable() {
        let key = l3_job_key("field-1", "sentinel-2-l2a", "ndvi", "2026-06");
        assert_eq!(key, "l3:sentinel-2-l2a:ndvi:field-1:2026-06");
        assert!(is_l3_key(&key));
        assert!(!is_l3_key(&discover_job_key("field-1", "sentinel-2-l2a")));
    }

    #[test]
    fn l3_suite_job_key_varies_by_product_and_shares_debounce_prefix() {
        let clim = l3_suite_job_key(
            "field-1",
            "sentinel-2-l2a",
            "ndvi",
            "climatology",
            "2026-06",
        );
        let stack = l3_suite_job_key(
            "field-1",
            "sentinel-2-l2a",
            "ndvi",
            "drought_stack",
            "2026-06",
        );
        assert_eq!(clim, "l3:sentinel-2-l2a:ndvi_climatology:field-1:2026-06");
        assert_ne!(clim, stack, "product distinguishes the dedupe bucket");
        // Distinct from the monthly composite key for the same (field, month).
        assert_ne!(
            clim,
            l3_job_key("field-1", "sentinel-2-l2a", "ndvi", "2026-06")
        );
        // Still an l3 debounce key.
        assert!(is_l3_key(&clim));
        assert!(is_l3_key(&stack));
    }

    #[test]
    fn l3_payload_product_defaults_to_monthly_composite() {
        let payload: L3RecomputePayload = serde_json::from_str(
            r#"{"field_id":"f","dataset":"d","index":"ndvi","month":"2026-06"}"#,
        )
        .unwrap();
        assert_eq!(payload.product, "monthly_composite");
    }

    #[test]
    fn app_job_key_buckets_by_day() {
        assert_eq!(
            app_job_key("drought_watch", "field-1", "2026-06-01"),
            "app:drought_watch:field-1:2026-06-01"
        );
    }

    #[test]
    fn job_kind_round_trips_through_strings() {
        for kind in [
            JobKind::Discover,
            JobKind::Derive,
            JobKind::L3Recompute,
            JobKind::AppRun,
            JobKind::BackfillEnumerate,
        ] {
            assert_eq!(JobKind::parse(kind.as_str()).unwrap(), kind);
        }
        assert!(JobKind::parse("bogus").is_err());
    }

    #[test]
    fn job_status_round_trips_and_flags_terminal_states() {
        for status in [
            JobStatus::Queued,
            JobStatus::Running,
            JobStatus::Succeeded,
            JobStatus::Failed,
            JobStatus::Dead,
        ] {
            assert_eq!(JobStatus::parse(status.as_str()).unwrap(), status);
        }
        assert!(JobStatus::Succeeded.is_terminal());
        assert!(JobStatus::Failed.is_terminal());
        assert!(JobStatus::Dead.is_terminal());
        assert!(!JobStatus::Queued.is_terminal());
        assert!(!JobStatus::Running.is_terminal());
    }

    #[test]
    fn format_ts_emits_sortable_rfc3339_utc() {
        let at = "2026-06-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        assert_eq!(format_ts(at), "2026-06-01T00:00:00Z");
        let later = at + Duration::seconds(90);
        assert!(format_ts(later) > format_ts(at));
    }
}
