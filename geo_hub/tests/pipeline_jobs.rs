//! Integration tests for the SQLite-backed satellite pipeline job queue and
//! field subscriptions (batch S-6): dedup on job_key, terminal-state
//! re-enqueue, l3 debounce, priority/run_after claiming, exponential backoff,
//! client-error dead-lettering, orphan recovery, and subscription cadence.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use geo_hub::pipeline::{
    self, DiscoverPayload, EnqueueOutcome, JobKind, JobStatus, SubscriptionUpsert,
};
use geo_hub::{db, HubConfig};
use serde_json::json;
use tempfile::TempDir;

fn t0() -> DateTime<Utc> {
    "2026-06-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap()
}

async fn pool(tmp: &TempDir) -> Result<db::DbPool> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("pipeline.db").display()
        ),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    db::connect_pool(&config).await
}

fn discover_payload(field: &str, dataset: &str) -> serde_json::Value {
    serde_json::to_value(DiscoverPayload {
        field_id: field.to_string(),
        dataset: dataset.to_string(),
    })
    .unwrap()
}

async fn enqueue_discover(
    pool: &db::DbPool,
    field: &str,
    priority: i64,
    run_after: DateTime<Utc>,
) -> Result<EnqueueOutcome> {
    let outcome = pipeline::enqueue_job(
        pool,
        JobKind::Discover,
        &pipeline::discover_job_key(field, "sentinel-2-l2a"),
        &discover_payload(field, "sentinel-2-l2a"),
        Some(field),
        Some("sentinel-2-l2a"),
        priority,
        run_after,
        None,
        t0(),
    )
    .await?;
    Ok(outcome)
}

#[tokio::test]
async fn connect_pool_enables_wal_journal_mode() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let mode: (String,) = sqlx::query_as("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await?;
    assert_eq!(mode.0.to_lowercase(), "wal");
    Ok(())
}

#[tokio::test]
async fn enqueue_deduplicates_on_job_key() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let first = enqueue_discover(&pool, "field-1", 0, t0()).await?;
    let second = enqueue_discover(&pool, "field-1", 0, t0()).await?;

    assert_eq!(first, EnqueueOutcome::Enqueued);
    assert_eq!(second, EnqueueOutcome::Deduplicated);

    let jobs = pipeline::list_jobs(&pool, None).await?;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].kind, JobKind::Discover);
    assert_eq!(jobs[0].status, JobStatus::Queued);
    Ok(())
}

#[tokio::test]
async fn reenqueue_after_terminal_resets_job() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    enqueue_discover(&pool, "field-1", 0, t0()).await?;
    let job = pipeline::claim_next_job(&pool, t0()).await?.unwrap();
    pipeline::complete_job(&pool, &job.job_id, t0()).await?;

    let outcome = enqueue_discover(&pool, "field-1", 0, t0() + Duration::hours(1)).await?;
    assert_eq!(outcome, EnqueueOutcome::Enqueued);

    let jobs = pipeline::list_jobs(&pool, None).await?;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].status, JobStatus::Queued);
    assert_eq!(jobs[0].attempts, 0);
    assert_eq!(jobs[0].last_error, None);
    assert_eq!(
        jobs[0].run_after,
        pipeline::format_ts(t0() + Duration::hours(1))
    );
    Ok(())
}

#[tokio::test]
async fn l3_conflict_pushes_run_after() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let key = pipeline::l3_job_key("field-1", "sentinel-2-l2a", "ndvi", "2026-06");
    let payload = json!({
        "field_id": "field-1", "dataset": "sentinel-2-l2a",
        "index": "ndvi", "month": "2026-06"
    });
    let first = pipeline::enqueue_job(
        &pool,
        JobKind::L3Recompute,
        &key,
        &payload,
        Some("field-1"),
        Some("sentinel-2-l2a"),
        0,
        t0(),
        None,
        t0(),
    )
    .await?;
    let later = t0() + Duration::minutes(10);
    let second = pipeline::enqueue_job(
        &pool,
        JobKind::L3Recompute,
        &key,
        &payload,
        Some("field-1"),
        Some("sentinel-2-l2a"),
        0,
        later,
        None,
        later,
    )
    .await?;

    assert_eq!(first, EnqueueOutcome::Enqueued);
    assert_eq!(second, EnqueueOutcome::Deduplicated);

    let jobs = pipeline::list_jobs(&pool, None).await?;
    assert_eq!(jobs.len(), 1);
    // Debounced: the pending l3 job's run_after moved out to the later time.
    assert_eq!(jobs[0].run_after, pipeline::format_ts(later));

    // Non-l3 queued jobs are a plain no-op dedup: run_after stays put.
    enqueue_discover(&pool, "field-2", 0, t0()).await?;
    enqueue_discover(&pool, "field-2", 0, later).await?;
    let jobs = pipeline::list_jobs(&pool, Some("field-2")).await?;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].run_after, pipeline::format_ts(t0()));
    Ok(())
}

#[tokio::test]
async fn claim_respects_run_after_and_priority() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    // Not yet due.
    enqueue_discover(&pool, "future", 100, t0() + Duration::hours(2)).await?;
    // Due, low priority.
    enqueue_discover(&pool, "low", 1, t0()).await?;
    // Due, high priority.
    enqueue_discover(&pool, "high", 5, t0()).await?;

    let first = pipeline::claim_next_job(&pool, t0()).await?.unwrap();
    assert_eq!(first.field_id.as_deref(), Some("high"));
    assert_eq!(first.status, JobStatus::Running);
    assert_eq!(first.attempts, 1);

    let second = pipeline::claim_next_job(&pool, t0()).await?.unwrap();
    assert_eq!(second.field_id.as_deref(), Some("low"));

    // The future job is not claimable yet.
    assert!(pipeline::claim_next_job(&pool, t0()).await?.is_none());

    // ... but becomes claimable once its run_after passes.
    let third = pipeline::claim_next_job(&pool, t0() + Duration::hours(3))
        .await?
        .unwrap();
    assert_eq!(third.field_id.as_deref(), Some("future"));
    Ok(())
}

#[tokio::test]
async fn failed_job_backs_off_exponentially() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    enqueue_discover(&pool, "field-1", 0, t0()).await?;

    // Attempt 1 fails: back off 60s * 2^1 = 120s.
    let job = pipeline::claim_next_job(&pool, t0()).await?.unwrap();
    assert_eq!(job.attempts, 1);
    pipeline::fail_job(&pool, &job, "provider timeout", false, t0()).await?;
    let jobs = pipeline::list_jobs(&pool, None).await?;
    assert_eq!(jobs[0].status, JobStatus::Queued);
    assert_eq!(
        jobs[0].run_after,
        pipeline::format_ts(t0() + Duration::seconds(120))
    );
    assert_eq!(jobs[0].last_error.as_deref(), Some("provider timeout"));

    // Attempt 2 fails: back off 60s * 2^2 = 240s.
    let later = t0() + Duration::seconds(120);
    let job = pipeline::claim_next_job(&pool, later).await?.unwrap();
    assert_eq!(job.attempts, 2);
    pipeline::fail_job(&pool, &job, "provider timeout", false, later).await?;
    let jobs = pipeline::list_jobs(&pool, None).await?;
    assert_eq!(jobs[0].status, JobStatus::Queued);
    assert_eq!(
        jobs[0].run_after,
        pipeline::format_ts(later + Duration::seconds(240))
    );

    // Attempt 3 (max_attempts = 3) fails: job goes dead.
    let final_now = later + Duration::seconds(240);
    let job = pipeline::claim_next_job(&pool, final_now).await?.unwrap();
    assert_eq!(job.attempts, 3);
    pipeline::fail_job(&pool, &job, "provider timeout", false, final_now).await?;
    let jobs = pipeline::list_jobs(&pool, None).await?;
    assert_eq!(jobs[0].status, JobStatus::Dead);
    assert!(jobs[0].finished_at.is_some());
    Ok(())
}

#[tokio::test]
async fn client_error_goes_dead() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    enqueue_discover(&pool, "field-1", 0, t0()).await?;
    let job = pipeline::claim_next_job(&pool, t0()).await?.unwrap();
    assert_eq!(job.attempts, 1);

    pipeline::fail_job(&pool, &job, "404 item not found", true, t0()).await?;

    let jobs = pipeline::list_jobs(&pool, None).await?;
    assert_eq!(jobs[0].status, JobStatus::Dead);
    assert_eq!(jobs[0].last_error.as_deref(), Some("404 item not found"));
    assert!(pipeline::claim_next_job(&pool, t0() + Duration::days(30))
        .await?
        .is_none());
    Ok(())
}

#[tokio::test]
async fn orphaned_running_jobs_reset_to_queued() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    enqueue_discover(&pool, "field-1", 0, t0()).await?;
    enqueue_discover(&pool, "field-2", 0, t0()).await?;
    pipeline::claim_next_job(&pool, t0()).await?.unwrap();
    pipeline::claim_next_job(&pool, t0()).await?.unwrap();

    let recovery = pipeline::reset_orphaned_running_jobs(&pool).await?;
    assert_eq!(recovery.requeued, 2);
    assert_eq!(recovery.dead_lettered, 0);

    let jobs = pipeline::list_jobs(&pool, None).await?;
    assert!(jobs.iter().all(|job| job.status == JobStatus::Queued));
    Ok(())
}

/// A job that keeps taking the worker down mid-run (poison pill) must not be
/// re-queued forever: once its attempts reach the ceiling the orphan sweep
/// dead-letters it, and a healthy sibling still recovers so the worker drains.
#[tokio::test]
async fn orphaned_job_with_exhausted_attempts_is_dead_lettered() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    enqueue_discover(&pool, "field-poison", 0, t0()).await?;
    enqueue_discover(&pool, "field-healthy", 0, t0()).await?;
    let poison = pipeline::claim_next_job(&pool, t0()).await?.unwrap();
    pipeline::claim_next_job(&pool, t0()).await?.unwrap();

    // Simulate the poison job having crash-looped up to its attempt ceiling; a
    // mid-run process death leaves it `running`.
    sqlx::query("UPDATE pipeline_jobs SET attempts = max_attempts WHERE job_id = ?")
        .bind(&poison.job_id)
        .execute(&pool)
        .await?;

    let recovery = pipeline::reset_orphaned_running_jobs(&pool).await?;
    assert_eq!(recovery.dead_lettered, 1, "poison job buried");
    assert_eq!(recovery.requeued, 1, "healthy job re-queued");

    let jobs = pipeline::list_jobs(&pool, None).await?;
    let poison_row = jobs.iter().find(|job| job.job_id == poison.job_id).unwrap();
    assert_eq!(poison_row.status, JobStatus::Dead);
    assert!(poison_row
        .last_error
        .as_deref()
        .unwrap()
        .contains("attempts exhausted"));

    // The worker keeps draining: the healthy job is claimable, the poison one is not.
    let next = pipeline::claim_next_job(&pool, t0()).await?.unwrap();
    assert_ne!(next.job_id, poison.job_id);
    Ok(())
}

#[tokio::test]
async fn due_subscriptions_respects_cadence() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let sub = pipeline::upsert_subscription(
        &pool,
        &SubscriptionUpsert {
            field_id: "field-1".to_string(),
            dataset: "sentinel-2-l2a".to_string(),
            indices: vec!["ndvi".to_string(), "ndmi".to_string()],
            cadence_hours: 24,
            max_cloud_cover: 60.0,
            lookback_days: 14,
        },
        t0(),
    )
    .await?;
    assert_eq!(sub.status, "active");
    assert_eq!(sub.last_checked_at, None);

    // Never checked: due immediately.
    let due = pipeline::due_subscriptions(&pool, t0()).await?;
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].subscription_id, sub.subscription_id);

    // Just checked: not due one hour later.
    pipeline::touch_subscription_checked(&pool, &sub.subscription_id, t0()).await?;
    let due = pipeline::due_subscriptions(&pool, t0() + Duration::hours(1)).await?;
    assert!(due.is_empty());

    // Due again after the 24h cadence elapses.
    let due = pipeline::due_subscriptions(&pool, t0() + Duration::hours(25)).await?;
    assert_eq!(due.len(), 1);

    // Paused subscriptions are never due.
    pipeline::set_subscription_status(&pool, &sub.subscription_id, "paused", t0()).await?;
    let due = pipeline::due_subscriptions(&pool, t0() + Duration::hours(48)).await?;
    assert!(due.is_empty());

    // Upsert on the same (field, dataset) updates in place.
    let updated = pipeline::upsert_subscription(
        &pool,
        &SubscriptionUpsert {
            field_id: "field-1".to_string(),
            dataset: "sentinel-2-l2a".to_string(),
            indices: vec!["ndvi".to_string()],
            cadence_hours: 12,
            max_cloud_cover: 40.0,
            lookback_days: 7,
        },
        t0() + Duration::hours(48),
    )
    .await?;
    assert_eq!(updated.subscription_id, sub.subscription_id);
    assert_eq!(updated.cadence_hours, 12);
    let all = pipeline::list_subscriptions(&pool, Some("field-1")).await?;
    assert_eq!(all.len(), 1);
    Ok(())
}
