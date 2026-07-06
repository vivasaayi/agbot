//! Pipeline worker loop (batch S-7).
//!
//! A **serial** background worker that drains the SQLite job queue built in
//! `crate::pipeline`: recover orphaned `running` jobs on startup, then on
//! every poll tick claim at most one ready job, execute it, and run the
//! subscription cadence pass (due subscriptions -> `discover` jobs).
//!
//! Batch scope: [`JobKind::Derive`] (S-7), [`JobKind::Discover`] (S-8), the
//! L3/application stage (S-9: a successful derive fans out into a monthly
//! [`JobKind::L3Recompute`] composite plus [`JobKind::AppRun`] jobs, whose
//! handlers run the composite/application cores in-process and then
//! evaluate field alerts), and [`JobKind::BackfillEnumerate`] (S-11: walk a
//! historical range in 90-day chunks, chaining one enumerate job per chunk;
//! see `crate::backfill`) all have handlers here.
//!
//! Everything the worker touches is injected through
//! [`PipelineWorkerContext`] — the DB pool, the hub config (data root +
//! `[pipeline]` cadence/politeness knobs), the COG store resolver, and the
//! STAC item fetcher/searcher — so tests drive [`run_one_tick`]
//! deterministically against in-memory fixtures with no network access.
//! `server::serve` spawns the loop when `pipeline.enabled` is set (S-8).
//!
//! `last_checked_at` ownership: the cadence pass touches the subscription
//! when it *enqueues* a discover job (so the subscription cannot re-fire on
//! every tick while the job waits in the queue), and the discover *handler*
//! touches it again after a successful STAC search (the authoritative record
//! of the last actual check). The small forward drift between the two
//! touches is covered by the subscription's `lookback_days` search margin.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use shared::schemas::GeoBounds;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::applications::ApplicationError;
use crate::backfill::{self, BACKFILL_CHUNK_DAYS, BACKFILL_PRIORITY};
use crate::catalog::{self, ProductFilter};
use crate::composite_rasters::{self, CompositeDeriveRequest, CompositeRasterError};
use crate::config::HubConfig;
use crate::db::DbPool;
use crate::earth_search::{self, EarthSearchItem};
use crate::pipeline::{
    self, AppRunPayload, BackfillEnumeratePayload, DerivePayload, DiscoverPayload, JobKind,
    L3RecomputePayload, PipelineError, PipelineJob,
};
use crate::satellite_derivation::{
    derive_satellite_index, index_kind_from_key, CogStoreResolver, DeriveRequest,
};
use shared::product_graph::ProductLevel;
use shared::timeseries_naming::{field_entity_ref, satellite_metric, ZonalStat};

/// Priority assigned to cadence-produced discover jobs. Zero keeps them
/// behind any operator-boosted work while still ahead of nothing by default.
const DISCOVER_PRIORITY: i64 = 0;

/// Page size for one discover STAC range search. A field-scale bbox over a
/// lookback window of days sees a handful of scenes; 50 leaves headroom.
const DISCOVER_SEARCH_LIMIT: usize = 50;

/// Boxed future alias so [`StacItemFetcher`] stays object-safe without an
/// `async-trait` dependency.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Failure fetching the STAC item a derive job references.
#[derive(Debug, thiserror::Error)]
pub enum ItemFetchError {
    /// The item does not exist upstream: a permanent, client-class failure.
    #[error("item {item_id} not found in collection {collection}")]
    NotFound { collection: String, item_id: String },
    /// Transient upstream/network failure: eligible for retry with backoff.
    #[error("item fetch failed: {0}")]
    Upstream(String),
    /// The fetcher cannot search this dataset at all (no upstream collection
    /// or the seam does not implement search): permanent, client-class.
    #[error("stac search unsupported: {0}")]
    SearchUnsupported(String),
}

impl ItemFetchError {
    fn is_client_error(&self) -> bool {
        matches!(
            self,
            ItemFetchError::NotFound { .. } | ItemFetchError::SearchUnsupported(_)
        )
    }
}

/// One discover-stage STAC range search: a WGS84 bbox (field-boundary
/// envelope), an ISO-8601 UTC time range, a cloud ceiling, and the
/// subscription's dataset key (`sentinel2` / `landsat` / `hls`).
#[derive(Debug, Clone, PartialEq)]
pub struct StacSearchQuery {
    /// `[min_lon, min_lat, max_lon, max_lat]`.
    pub bbox: [f64; 4],
    pub start_iso: String,
    pub end_iso: String,
    pub max_cloud_cover: f64,
    pub dataset: String,
}

/// Seam for resolving a derive payload's `(collection, item_id)` to the full
/// STAC item, and for the discover stage's range search. Production uses
/// [`EarthSearchItemFetcher`]; tests inject a fixture fetcher so the worker
/// never touches the network.
///
/// `search` has a default body returning
/// [`ItemFetchError::SearchUnsupported`], so fetch-only fixtures keep
/// compiling; any fetcher whose context executes discover jobs overrides it.
pub trait StacItemFetcher: Send + Sync {
    fn fetch_item<'a>(
        &'a self,
        collection: &'a str,
        item_id: &'a str,
    ) -> BoxFuture<'a, Result<EarthSearchItem, ItemFetchError>>;

    fn search<'a>(
        &'a self,
        query: &'a StacSearchQuery,
    ) -> BoxFuture<'a, Result<Vec<EarthSearchItem>, ItemFetchError>> {
        Box::pin(async move {
            Err(ItemFetchError::SearchUnsupported(format!(
                "this fetcher does not implement search (dataset {})",
                query.dataset
            )))
        })
    }
}

/// Production fetcher: live Earth Search item lookup. A `404` in the
/// upstream error text is classified as [`ItemFetchError::NotFound`] (the
/// item id is bad and retrying cannot help); everything else is transient.
pub struct EarthSearchItemFetcher;

impl StacItemFetcher for EarthSearchItemFetcher {
    fn fetch_item<'a>(
        &'a self,
        collection: &'a str,
        item_id: &'a str,
    ) -> BoxFuture<'a, Result<EarthSearchItem, ItemFetchError>> {
        Box::pin(async move {
            earth_search::fetch_item(collection, item_id)
                .await
                .map_err(|err| {
                    let message = err.to_string();
                    if message.contains("404") {
                        ItemFetchError::NotFound {
                            collection: collection.to_string(),
                            item_id: item_id.to_string(),
                        }
                    } else {
                        ItemFetchError::Upstream(message)
                    }
                })
        })
    }

    fn search<'a>(
        &'a self,
        query: &'a StacSearchQuery,
    ) -> BoxFuture<'a, Result<Vec<EarthSearchItem>, ItemFetchError>> {
        Box::pin(async move {
            let collection =
                earth_search::collection_for_dataset(&query.dataset).ok_or_else(|| {
                    ItemFetchError::SearchUnsupported(format!(
                        "no Earth Search collection for dataset {}",
                        query.dataset
                    ))
                })?;
            earth_search::search_items_range(
                &[collection],
                query.bbox,
                &query.start_iso,
                &query.end_iso,
                query.max_cloud_cover,
                DISCOVER_SEARCH_LIMIT,
            )
            .await
            .map_err(|err| ItemFetchError::Upstream(err.to_string()))
        })
    }
}

/// Everything the worker loop needs, fully injected (no globals): the queue
/// database, the hub config carrying `data_root` and the `[pipeline]`
/// settings, and the two remote seams (COG store resolver, item fetcher).
#[derive(Clone)]
pub struct PipelineWorkerContext {
    pub pool: DbPool,
    pub config: Arc<HubConfig>,
    pub cog_resolver: Arc<dyn CogStoreResolver>,
    pub item_fetcher: Arc<dyn StacItemFetcher>,
}

/// Terminal result of executing one claimed job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRunResult {
    Succeeded,
    Failed { error: String, client_error: bool },
}

/// What one worker tick did, for tests and loop bookkeeping.
#[derive(Debug, Default)]
pub struct TickOutcome {
    /// Whether a ready job was claimed this tick.
    pub claimed: bool,
    pub job_id: Option<String>,
    pub kind: Option<JobKind>,
    pub result: Option<JobRunResult>,
    /// Discover jobs enqueued by the cadence pass this tick.
    pub discover_enqueued: usize,
    /// True when the executed job class talks to a remote provider; the loop
    /// then applies the `provider_min_delay_ms` politeness pause before the
    /// next claim.
    pub hit_provider: bool,
}

/// Run one worker tick: claim and execute at most one ready job, then run
/// the subscription cadence pass.
///
/// The claim deliberately runs **before** the cadence pass, so a discover
/// job enqueued by this tick stays observable as `queued` until the next
/// tick — single-stepping in tests is deterministic and a cadence burst
/// cannot starve the job it was scheduled after.
pub async fn run_one_tick(ctx: &PipelineWorkerContext) -> Result<TickOutcome, PipelineError> {
    let mut outcome = TickOutcome::default();
    let now = Utc::now();

    // 1. Claim and execute at most one ready job.
    if let Some(job) = pipeline::claim_next_job(&ctx.pool, now).await? {
        outcome.claimed = true;
        outcome.job_id = Some(job.job_id.clone());
        outcome.kind = Some(job.kind);
        // Conservative politeness: derive jobs hit the imagery provider
        // (item fetch and/or COG range reads); discover and backfill
        // enumerate jobs hit the STAC search endpoint.
        outcome.hit_provider = matches!(
            job.kind,
            JobKind::Derive | JobKind::Discover | JobKind::BackfillEnumerate
        );
        let result = execute_job(ctx, &job).await;
        match &result {
            JobRunResult::Succeeded => {
                pipeline::complete_job(&ctx.pool, &job.job_id, Utc::now()).await?;
            }
            JobRunResult::Failed {
                error,
                client_error,
            } => {
                pipeline::fail_job(&ctx.pool, &job, error, *client_error, Utc::now()).await?;
            }
        }
        outcome.result = Some(result);
    }

    // 2. Cadence pass: due subscriptions produce discover jobs (job_key
    //    dedupe makes re-enqueueing an already-pending discover a no-op).
    for subscription in pipeline::due_subscriptions(&ctx.pool, now).await? {
        let payload = serde_json::to_value(DiscoverPayload {
            field_id: subscription.field_id.clone(),
            dataset: subscription.dataset.clone(),
        })?;
        pipeline::enqueue_job(
            &ctx.pool,
            JobKind::Discover,
            &pipeline::discover_job_key(&subscription.field_id, &subscription.dataset),
            &payload,
            Some(&subscription.field_id),
            Some(&subscription.dataset),
            DISCOVER_PRIORITY,
            now,
            None,
            now,
        )
        .await?;
        pipeline::touch_subscription_checked(&ctx.pool, &subscription.subscription_id, now).await?;
        outcome.discover_enqueued += 1;
    }

    Ok(outcome)
}

/// Spawn the serial worker loop: reset orphaned `running` jobs once, then
/// tick every `pipeline.poll_interval_ms` until `shutdown_rx` flips to
/// `true` (or its sender drops). After a tick that touched a remote
/// provider, sleeps `pipeline.provider_min_delay_ms` before the next claim.
pub fn spawn_pipeline_worker(
    ctx: PipelineWorkerContext,
    mut shutdown_rx: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if *shutdown_rx.borrow() {
            return;
        }
        match pipeline::reset_orphaned_running_jobs(&ctx.pool).await {
            Ok(0) => {}
            Ok(reset) => tracing::info!("pipeline worker re-queued {reset} orphaned running jobs"),
            Err(err) => {
                tracing::error!("pipeline worker failed to reset orphaned running jobs: {err}");
            }
        }

        let poll = Duration::from_millis(ctx.config.pipeline.poll_interval_ms.max(1));
        let provider_delay = Duration::from_millis(ctx.config.pipeline.provider_min_delay_ms);
        let mut ticker = tokio::time::interval(poll);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                changed = shutdown_rx.changed() => {
                    if changed.is_err() || *shutdown_rx.borrow() {
                        tracing::info!("pipeline worker shutting down");
                        break;
                    }
                }
                _ = ticker.tick() => {
                    match run_one_tick(&ctx).await {
                        Ok(outcome) => {
                            if outcome.hit_provider && !provider_delay.is_zero() {
                                tokio::time::sleep(provider_delay).await;
                            }
                        }
                        Err(err) => {
                            tracing::warn!("pipeline worker tick failed: {err}");
                        }
                    }
                }
            }
        }
    })
}

// --- Job execution -----------------------------------------------------------

fn client_failure(error: String) -> JobRunResult {
    JobRunResult::Failed {
        error,
        client_error: true,
    }
}

fn transient_failure(error: String) -> JobRunResult {
    JobRunResult::Failed {
        error,
        client_error: false,
    }
}

async fn execute_job(ctx: &PipelineWorkerContext, job: &PipelineJob) -> JobRunResult {
    match job.kind {
        JobKind::Derive => execute_derive(ctx, job).await,
        JobKind::Discover => execute_discover(ctx, job).await,
        JobKind::L3Recompute => execute_l3_recompute(ctx, job).await,
        JobKind::AppRun => execute_app_run(ctx, job).await,
        JobKind::BackfillEnumerate => execute_backfill_enumerate(ctx, job).await,
    }
}

/// Execute one link of a backfill's enumerate chain (S-11): search one
/// 90-day chunk of the first incomplete dataset, fan `item x index` out
/// into priority [`BACKFILL_PRIORITY`] derive jobs tagged with the run id,
/// advance the run's cursor to the chunk end, and enqueue the next link.
///
/// - A run that is not `running` completes the job as a parked no-op; the
///   resume route re-enqueues the chain under the same cursor fingerprint.
/// - A search failure fails the job (retry with backoff for transient
///   upstream errors) **without** advancing the cursor, so the retry
///   re-walks the same chunk.
/// - When every dataset's cursor reaches `end_date`, the run is marked
///   `completed` and the chain stops.
async fn execute_backfill_enumerate(
    ctx: &PipelineWorkerContext,
    job: &PipelineJob,
) -> JobRunResult {
    let payload: BackfillEnumeratePayload = match serde_json::from_str(&job.payload_json) {
        Ok(payload) => payload,
        Err(err) => return client_failure(format!("invalid backfill_enumerate payload: {err}")),
    };
    let run = match backfill::get_backfill_run(&ctx.pool, &payload.backfill_id).await {
        Ok(Some(run)) => run,
        Ok(None) => {
            return client_failure(format!("backfill run {} not found", payload.backfill_id))
        }
        Err(err) => return transient_failure(err.to_string()),
    };
    if run.status != "running" {
        tracing::info!(
            backfill_id = %run.backfill_id,
            status = %run.status,
            "backfill enumerate: run not running; parking as no-op"
        );
        return JobRunResult::Succeeded;
    }

    let Some((dataset, cursor)) = run.next_incomplete_dataset() else {
        // Every dataset already reached end_date: close the run out.
        if let Err(err) =
            backfill::set_status(&ctx.pool, &run.backfill_id, "completed", Utc::now()).await
        {
            return transient_failure(err.to_string());
        }
        return JobRunResult::Succeeded;
    };
    let Some((chunk_start, chunk_end)) =
        backfill::next_chunk(cursor, &run.end_date, BACKFILL_CHUNK_DAYS)
    else {
        // Unreachable while next_incomplete_dataset holds, unless the stored
        // dates are corrupt — that cannot be fixed by retrying.
        return client_failure(format!(
            "backfill run {} has an unchunkable range {cursor} .. {}",
            run.backfill_id, run.end_date
        ));
    };

    // Field boundary -> WGS84 search envelope + derive AOI (same contract
    // as the discover handler).
    let boundary_json: Option<(String,)> =
        match sqlx::query_as("SELECT boundary_json FROM fields WHERE field_id = ?")
            .bind(&run.field_id)
            .fetch_optional(&ctx.pool)
            .await
        {
            Ok(row) => row,
            Err(err) => return transient_failure(err.to_string()),
        };
    let Some((boundary_json,)) = boundary_json else {
        return client_failure(format!("field {} not found", run.field_id));
    };
    let boundary: serde_json::Value = match serde_json::from_str(&boundary_json) {
        Ok(value) => value,
        Err(err) => {
            return client_failure(format!(
                "field {} boundary_json is not valid JSON: {err}",
                run.field_id
            ))
        }
    };
    let bbox = match aoi_bounds_from_geojson(&boundary) {
        Ok(bounds) => bounds,
        Err(message) => {
            return client_failure(format!(
                "field {} boundary is not a usable AOI: {message}",
                run.field_id
            ))
        }
    };

    // Search the chunk as a half-open window [start, end): the next chunk
    // starts at this chunk's end date, and derive job_key dedupe absorbs
    // any boundary-instant overlap.
    let query = StacSearchQuery {
        bbox: [bbox.min_lon, bbox.min_lat, bbox.max_lon, bbox.max_lat],
        start_iso: format!("{chunk_start}T00:00:00Z"),
        end_iso: format!("{chunk_end}T00:00:00Z"),
        max_cloud_cover: run.max_cloud_cover,
        dataset: dataset.to_string(),
    };
    let items = match ctx.item_fetcher.search(&query).await {
        Ok(items) => items,
        Err(err) => {
            return JobRunResult::Failed {
                client_error: err.is_client_error(),
                error: err.to_string(),
            }
        }
    };

    // Fan out: item x index -> derive job at backfill priority. Terminal
    // succeeded derives are skipped outright (re-enqueue would reset them
    // and re-derive the product); queued/running rows deduplicate.
    let mut enqueued = 0_i64;
    for item in &items {
        let collection = item
            .collection
            .clone()
            .or_else(|| earth_search::collection_for_dataset(dataset).map(String::from))
            .unwrap_or_else(|| dataset.to_string());
        for index in &run.indices {
            let job_key = pipeline::derive_job_key(dataset, &item.id, index, &run.field_id);
            match pipeline::find_job_by_key(&ctx.pool, &job_key).await {
                Ok(Some(existing)) if existing.status == pipeline::JobStatus::Succeeded => {
                    continue;
                }
                Ok(_) => {}
                Err(err) => return transient_failure(err.to_string()),
            }
            let derive_payload = match serde_json::to_value(DerivePayload {
                dataset: dataset.to_string(),
                collection: collection.clone(),
                item_id: item.id.clone(),
                index: index.clone(),
                field_id: run.field_id.clone(),
                season_id: None,
                aoi_geojson: boundary.clone(),
            }) {
                Ok(value) => value,
                Err(err) => return client_failure(format!("derive payload: {err}")),
            };
            let now = Utc::now();
            match pipeline::enqueue_job(
                &ctx.pool,
                JobKind::Derive,
                &job_key,
                &derive_payload,
                Some(&run.field_id),
                Some(dataset),
                BACKFILL_PRIORITY,
                now,
                Some(&run.backfill_id),
                now,
            )
            .await
            {
                Ok(pipeline::EnqueueOutcome::Enqueued) => enqueued += 1,
                Ok(pipeline::EnqueueOutcome::Deduplicated) => {}
                Err(err) => return transient_failure(err.to_string()),
            }
        }
    }

    // Durable progress: cursor to chunk end, counters bumped. Only after
    // this point is the chunk considered done.
    let now = Utc::now();
    let dataset = dataset.to_string();
    if let Err(err) =
        backfill::advance_cursor(&ctx.pool, &run.backfill_id, &dataset, &chunk_end, now).await
    {
        return transient_failure(err.to_string());
    }
    if let Err(err) = backfill::add_progress_counts(
        &ctx.pool,
        &run.backfill_id,
        items.len() as i64,
        enqueued,
        now,
    )
    .await
    {
        return transient_failure(err.to_string());
    }

    // Chain or complete, from the freshly advanced cursor.
    let updated = match backfill::get_backfill_run(&ctx.pool, &run.backfill_id).await {
        Ok(Some(updated)) => updated,
        Ok(None) => {
            return client_failure(format!(
                "backfill run {} vanished mid-enumerate",
                run.backfill_id
            ))
        }
        Err(err) => return transient_failure(err.to_string()),
    };
    if updated.next_incomplete_dataset().is_none() {
        if let Err(err) =
            backfill::set_status(&ctx.pool, &updated.backfill_id, "completed", Utc::now()).await
        {
            return transient_failure(err.to_string());
        }
        tracing::info!(
            backfill_id = %updated.backfill_id,
            scenes_discovered = updated.scenes_discovered,
            jobs_enqueued = updated.jobs_enqueued,
            "backfill run completed"
        );
        return JobRunResult::Succeeded;
    }
    let delay = ChronoDuration::milliseconds(ctx.config.pipeline.provider_min_delay_ms as i64);
    let now = Utc::now();
    if let Err(err) = backfill::enqueue_enumerate_job(&ctx.pool, &updated, now + delay, now).await {
        return transient_failure(format!(
            "chunk {chunk_start}..{chunk_end} ({dataset}) enumerated but chaining failed: {err}"
        ));
    }
    tracing::info!(
        backfill_id = %updated.backfill_id,
        dataset = %dataset,
        chunk_start = %chunk_start,
        chunk_end = %chunk_end,
        items = items.len(),
        enqueued,
        "backfill chunk enumerated"
    );
    JobRunResult::Succeeded
}

/// Execute one discover job: search the upstream STAC catalog for new items
/// covering the field's boundary within the subscription's lookback window,
/// and fan each `item x subscribed index` out into a derive job.
///
/// Skip semantics: a derive `job_key` whose row already **succeeded** is
/// skipped outright (plain re-enqueue would reset the terminal row and
/// re-derive the product); anything still queued/running deduplicates via
/// the normal `enqueue_job` no-op. After a successful search the handler
/// touches the subscription's `last_checked_at` (see the module docs for the
/// touch-ownership contract with the cadence pass).
async fn execute_discover(ctx: &PipelineWorkerContext, job: &PipelineJob) -> JobRunResult {
    let payload: DiscoverPayload = match serde_json::from_str(&job.payload_json) {
        Ok(payload) => payload,
        Err(err) => return client_failure(format!("invalid discover payload: {err}")),
    };

    let subscription =
        match pipeline::find_subscription(&ctx.pool, &payload.field_id, &payload.dataset).await {
            Ok(Some(subscription)) => subscription,
            Ok(None) => {
                return client_failure(format!(
                    "no subscription for field {} dataset {}",
                    payload.field_id, payload.dataset
                ))
            }
            Err(err) => return transient_failure(err.to_string()),
        };

    // Field boundary -> WGS84 search envelope + derive AOI.
    let boundary_json: Option<(String,)> =
        match sqlx::query_as("SELECT boundary_json FROM fields WHERE field_id = ?")
            .bind(&payload.field_id)
            .fetch_optional(&ctx.pool)
            .await
        {
            Ok(row) => row,
            Err(err) => return transient_failure(err.to_string()),
        };
    let Some((boundary_json,)) = boundary_json else {
        return client_failure(format!("field {} not found", payload.field_id));
    };
    let boundary: serde_json::Value = match serde_json::from_str(&boundary_json) {
        Ok(value) => value,
        Err(err) => {
            return client_failure(format!(
                "field {} boundary_json is not valid JSON: {err}",
                payload.field_id
            ))
        }
    };
    let bbox = match aoi_bounds_from_geojson(&boundary) {
        Ok(bounds) => bounds,
        Err(message) => {
            return client_failure(format!(
                "field {} boundary is not a usable AOI: {message}",
                payload.field_id
            ))
        }
    };

    // Search window: [anchor - lookback_days, now], where the anchor is the
    // last successful check (or now for a never-checked subscription). The
    // lookback margin absorbs late-published scenes and touch drift.
    let now = Utc::now();
    let anchor = subscription
        .last_checked_at
        .as_deref()
        .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
        .map(|ts| ts.with_timezone(&Utc))
        .unwrap_or(now);
    let start = anchor - ChronoDuration::days(subscription.lookback_days.max(0));
    let query = StacSearchQuery {
        bbox: [bbox.min_lon, bbox.min_lat, bbox.max_lon, bbox.max_lat],
        start_iso: pipeline::format_ts(start),
        end_iso: pipeline::format_ts(now),
        max_cloud_cover: subscription.max_cloud_cover,
        dataset: payload.dataset.clone(),
    };
    let items = match ctx.item_fetcher.search(&query).await {
        Ok(items) => items,
        Err(err) => {
            return JobRunResult::Failed {
                client_error: err.is_client_error(),
                error: err.to_string(),
            }
        }
    };

    // Fan out: item x subscribed index -> derive job.
    let mut enqueued = 0_usize;
    let mut deduplicated = 0_usize;
    let mut skipped_done = 0_usize;
    for item in &items {
        let collection = item
            .collection
            .clone()
            .or_else(|| earth_search::collection_for_dataset(&payload.dataset).map(String::from))
            .unwrap_or_else(|| payload.dataset.clone());
        for index in &subscription.indices {
            let job_key =
                pipeline::derive_job_key(&payload.dataset, &item.id, index, &payload.field_id);
            match pipeline::find_job_by_key(&ctx.pool, &job_key).await {
                Ok(Some(existing)) if existing.status == pipeline::JobStatus::Succeeded => {
                    skipped_done += 1;
                    continue;
                }
                Ok(_) => {}
                Err(err) => return transient_failure(err.to_string()),
            }
            let derive_payload = match serde_json::to_value(DerivePayload {
                dataset: payload.dataset.clone(),
                collection: collection.clone(),
                item_id: item.id.clone(),
                index: index.clone(),
                field_id: payload.field_id.clone(),
                season_id: None,
                aoi_geojson: boundary.clone(),
            }) {
                Ok(value) => value,
                Err(err) => return client_failure(format!("derive payload: {err}")),
            };
            let outcome = pipeline::enqueue_job(
                &ctx.pool,
                JobKind::Derive,
                &job_key,
                &derive_payload,
                Some(&payload.field_id),
                Some(&payload.dataset),
                0,
                now,
                job.backfill_id.as_deref(),
                now,
            )
            .await;
            match outcome {
                Ok(pipeline::EnqueueOutcome::Enqueued) => enqueued += 1,
                Ok(pipeline::EnqueueOutcome::Deduplicated) => deduplicated += 1,
                Err(err) => return transient_failure(err.to_string()),
            }
        }
    }

    // Authoritative check record (module docs: cadence touches at enqueue,
    // the handler touches after the search actually ran).
    if let Err(err) =
        pipeline::touch_subscription_checked(&ctx.pool, &subscription.subscription_id, now).await
    {
        return transient_failure(err.to_string());
    }

    // `complete_job` stores only the status, so the summary goes to the log.
    tracing::info!(
        field_id = %payload.field_id,
        dataset = %payload.dataset,
        items = items.len(),
        enqueued,
        deduplicated,
        skipped_done,
        "discover fan-out complete"
    );
    JobRunResult::Succeeded
}

/// Execute one derive job: decode the payload, resolve the STAC item, and
/// run the exact same derivation the `POST /api/satellite/derive` route uses
/// ([`derive_satellite_index`]), which registers L0/L1/L2 lineage and — with
/// the payload's field scope — appends per-field time-series stats (S-3).
/// A successful derive then fans out the S-9 follow-ups: the month's
/// `l3_recompute` (debounced by [`pipeline::l3_job_key`]) and one `app_run`
/// per applicable application.
async fn execute_derive(ctx: &PipelineWorkerContext, job: &PipelineJob) -> JobRunResult {
    let payload: DerivePayload = match serde_json::from_str(&job.payload_json) {
        Ok(payload) => payload,
        Err(err) => return client_failure(format!("invalid derive payload: {err}")),
    };
    let index = match index_kind_from_key(&payload.index) {
        Ok(index) => index,
        Err(err) => return client_failure(err.to_string()),
    };
    let aoi = match aoi_bounds_from_geojson(&payload.aoi_geojson) {
        Ok(aoi) => aoi,
        Err(message) => return client_failure(format!("invalid aoi_geojson: {message}")),
    };
    let item = match ctx
        .item_fetcher
        .fetch_item(&payload.collection, &payload.item_id)
        .await
    {
        Ok(item) => item,
        Err(err) => {
            return JobRunResult::Failed {
                client_error: err.is_client_error(),
                error: err.to_string(),
            }
        }
    };

    let request = DeriveRequest {
        item,
        aoi,
        index,
        field_id: Some(payload.field_id.clone()),
        season_id: payload.season_id.clone(),
    };
    let outcome = match derive_satellite_index(
        &ctx.pool,
        &ctx.config.data_root,
        ctx.cog_resolver.as_ref(),
        &request,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(err) => {
            return JobRunResult::Failed {
                client_error: err.is_client_error(),
                error: err.to_string(),
            }
        }
    };

    // Post-derive hook (S-9): the registration is durable, so a follow-up
    // enqueue failure is transient — the retry re-runs the (idempotent,
    // content-addressed) derive and re-attempts the fan-out.
    match enqueue_post_derive_jobs(ctx, &payload, &outcome.product_id).await {
        Ok(()) => JobRunResult::Succeeded,
        Err(err) => transient_failure(format!(
            "derive succeeded (product {}) but follow-up enqueue failed: {err}",
            outcome.product_id
        )),
    }
}

// --- S-9: post-derive fan-out, L3 recompute, and application runs -------------

/// The applications an index feeds, v1: a fixed mapping — `ndvi` drives the
/// crop-health app, and every index feeds the anomaly screen. Per-field,
/// subscription-level application configuration is future work; when it
/// lands, this becomes a lookup on the field's subscription instead.
pub fn apps_for_index(index: &str) -> Vec<&'static str> {
    match index {
        "ndvi" => vec![crate::crop_health_run::APP_ID, crate::anomaly_run::APP_ID],
        _ => vec![crate::anomaly_run::APP_ID],
    }
}

/// After a successful derive, enqueue the month's L3 composite recompute
/// (the `l3:` job key debounces a burst of same-month derives into one
/// rollup) and one app-run job per applicable application, dated by the L2
/// product's `temporal_start`.
async fn enqueue_post_derive_jobs(
    ctx: &PipelineWorkerContext,
    payload: &DerivePayload,
    product_id: &str,
) -> Result<(), String> {
    let product = catalog::get_product(&ctx.pool, product_id)
        .await
        .map_err(|err| format!("catalog lookup failed: {err}"))?;
    let temporal_start = product.as_ref().and_then(|p| p.temporal_start.clone());
    let Some(temporal_start) = temporal_start.filter(|ts| ts.len() >= 10) else {
        tracing::warn!(
            product_id = %product_id,
            "derived product has no usable temporal_start; skipping L3/app fan-out"
        );
        return Ok(());
    };
    let date = &temporal_start[..10];
    let month = &temporal_start[..7];
    let now = Utc::now();

    let l3_payload = serde_json::to_value(L3RecomputePayload {
        field_id: payload.field_id.clone(),
        dataset: payload.dataset.clone(),
        index: payload.index.clone(),
        month: month.to_string(),
    })
    .map_err(|err| format!("l3_recompute payload: {err}"))?;
    pipeline::enqueue_job(
        &ctx.pool,
        JobKind::L3Recompute,
        &pipeline::l3_job_key(&payload.field_id, &payload.dataset, &payload.index, month),
        &l3_payload,
        Some(&payload.field_id),
        Some(&payload.dataset),
        0,
        now,
        None,
        now,
    )
    .await
    .map_err(|err| format!("l3_recompute enqueue: {err}"))?;

    for app_id in apps_for_index(&payload.index) {
        let app_payload = serde_json::to_value(AppRunPayload {
            app_id: app_id.to_string(),
            field_id: payload.field_id.clone(),
            date: date.to_string(),
        })
        .map_err(|err| format!("app_run payload: {err}"))?;
        pipeline::enqueue_job(
            &ctx.pool,
            JobKind::AppRun,
            &pipeline::app_job_key(app_id, &payload.field_id, date),
            &app_payload,
            Some(&payload.field_id),
            None,
            0,
            now,
            None,
            now,
        )
        .await
        .map_err(|err| format!("app_run enqueue for {app_id}: {err}"))?;
    }
    Ok(())
}

/// Execute one L3 recompute: composite the field's registered same-index L2
/// series over the payload's month through the same core the
/// `POST /api/composites/derive` route uses ([`composite_rasters::derive_composite`]),
/// then supersede the previous registered composite for that
/// (field, index, month). A month with no field-scoped inputs completes as
/// a logged no-op — an empty month is a normal pipeline state, not a fault.
async fn execute_l3_recompute(ctx: &PipelineWorkerContext, job: &PipelineJob) -> JobRunResult {
    let payload: L3RecomputePayload = match serde_json::from_str(&job.payload_json) {
        Ok(payload) => payload,
        Err(err) => return client_failure(format!("invalid l3_recompute payload: {err}")),
    };
    let Some((start, end)) = month_window(&payload.month) else {
        return client_failure(format!(
            "l3_recompute month {:?} is not YYYY-MM",
            payload.month
        ));
    };

    // Field-scoped input pre-check: the composite core composites every
    // same-kind L2 on the grid, so gate on the field's own series first.
    let inputs = match catalog::list_products(
        &ctx.pool,
        &ProductFilter {
            field_id: Some(payload.field_id.clone()),
            kind: Some(payload.index.clone()),
            level: Some(ProductLevel::L2),
            status: Some("registered".to_string()),
            temporal_start: Some(format!("{start}T00:00:00Z")),
            temporal_end: Some(format!("{end}T23:59:59Z")),
            ..ProductFilter::default()
        },
    )
    .await
    {
        Ok(inputs) => inputs,
        Err(err) => return transient_failure(err.to_string()),
    };
    if inputs.is_empty() {
        tracing::info!(
            field_id = %payload.field_id,
            index = %payload.index,
            month = %payload.month,
            "l3 recompute: no field-scoped L2 inputs in month; completing as no-op"
        );
        return JobRunResult::Succeeded;
    }
    // Latest input's season labels the composite scope.
    let season_id = inputs
        .iter()
        .max_by(|a, b| a.temporal_start.cmp(&b.temporal_start))
        .and_then(|p| p.season_id.clone())
        .unwrap_or_default();

    let outcome = match composite_rasters::derive_composite(
        &ctx.pool,
        &ctx.config.data_root,
        &CompositeDeriveRequest {
            kind: payload.index.clone(),
            start: start.to_string(),
            end: end.to_string(),
            method: "median".to_string(),
            field_id: payload.field_id.clone(),
            season_id,
        },
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(CompositeRasterError::NoUsableObservations { skipped, .. }) => {
            tracing::info!(
                field_id = %payload.field_id,
                index = %payload.index,
                month = %payload.month,
                skipped,
                "l3 recompute: no usable observations; completing as no-op"
            );
            return JobRunResult::Succeeded;
        }
        Err(err) => {
            return JobRunResult::Failed {
                client_error: err.is_client_error(),
                error: err.to_string(),
            }
        }
    };

    if let Err(err) = supersede_previous_composites(
        ctx,
        &payload.field_id,
        &payload.index,
        &payload.month,
        &outcome.composite_product_id,
    )
    .await
    {
        return transient_failure(format!(
            "composite {} registered but supersede pass failed: {err}",
            outcome.composite_product_id
        ));
    }
    tracing::info!(
        field_id = %payload.field_id,
        index = %payload.index,
        month = %payload.month,
        composite_product_id = %outcome.composite_product_id,
        observations = outcome.observations_used.len(),
        "l3 monthly composite recomputed"
    );
    JobRunResult::Succeeded
}

/// `YYYY-MM` -> inclusive (first day, last day) of that month.
fn month_window(month: &str) -> Option<(chrono::NaiveDate, chrono::NaiveDate)> {
    let start = chrono::NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").ok()?;
    let end = start
        .checked_add_months(chrono::Months::new(1))?
        .pred_opt()?;
    Some((start, end))
}

/// Mark every other registered `temporal_composite` for the same
/// (field, index, month) as superseded by the freshly registered one, so the
/// catalog always exposes exactly one live monthly composite per series.
async fn supersede_previous_composites(
    ctx: &PipelineWorkerContext,
    field_id: &str,
    index: &str,
    month: &str,
    new_product_id: &str,
) -> Result<(), crate::catalog::CatalogError> {
    let registered =
        composite_rasters::list_composite_products(&ctx.pool, Some(field_id.to_string())).await?;
    for previous in registered {
        if previous.product_id == new_product_id {
            continue;
        }
        let same_month = previous
            .temporal_start
            .as_deref()
            .is_some_and(|ts| ts.starts_with(month));
        let same_index = previous.parameters["band_names"]
            .as_array()
            .is_some_and(|bands| bands.iter().any(|band| band == index));
        if same_month && same_index {
            catalog::supersede_product(&ctx.pool, &previous.product_id, new_product_id).await?;
        }
    }
    Ok(())
}

/// One observation of a field index series: acquisition time, field-mean
/// value, and the L2 product it was extracted from.
struct SeriesObservation {
    t: String,
    mean: f64,
    product_id: String,
}

/// The field's `sat.{index}.mean` observations at or before the run date,
/// oldest first, from the S-3 per-field time-series store.
async fn field_index_series(
    pool: &DbPool,
    field_id: &str,
    index: &str,
    date: &str,
) -> Result<Vec<SeriesObservation>, sqlx::Error> {
    let rows: Vec<(String, f64, String)> = sqlx::query_as(
        r#"
        SELECT t, scalar_value, source_ref FROM time_series_points
        WHERE entity_ref = ? AND metric = ? AND t <= ?
        ORDER BY t ASC
        "#,
    )
    .bind(field_entity_ref(field_id))
    .bind(satellite_metric(index, ZonalStat::Mean))
    .bind(format!("{date}T23:59:59Z"))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(t, mean, source_ref)| SeriesObservation {
            t,
            mean,
            product_id: source_ref
                .strip_prefix("product:")
                .unwrap_or(&source_ref)
                .to_string(),
        })
        .collect())
}

/// Best-effort field area from the backing product's bbox (CRS units are
/// meters for the projected L2 grids this pipeline derives). `0.0` when the
/// product or bbox is unavailable — area only grades finding priority.
async fn product_area_m2(pool: &DbPool, product_id: &str) -> f32 {
    match catalog::get_product(pool, product_id).await {
        Ok(Some(product)) => product
            .bbox
            .map(|b| (((b[2] - b[0]) * (b[3] - b[1])).abs()) as f32)
            .unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Classify an application-core failure: unregistered/non-L2-L3 inputs and
/// serialization problems are permanent client errors; catalog/provenance/DB
/// failures are transient.
fn application_failure(err: ApplicationError) -> JobRunResult {
    let client_error = matches!(
        err,
        ApplicationError::InputNotFound(_)
            | ApplicationError::InputNotL2OrL3 { .. }
            | ApplicationError::Serialize { .. }
    );
    JobRunResult::Failed {
        error: err.to_string(),
        client_error,
    }
}

/// Number of trailing observations the anomaly screen uses as its
/// population. Bounded so a long-lived field cannot grow the run unbounded.
const ANOMALY_POPULATION_LIMIT: usize = 12;

/// Execute one app-run job: assemble the field's observation series into the
/// application core's zone inputs, record the governed run (findings +
/// lineage via `applications::record_run`), then run alert evaluation for
/// the field ([`crate::alert_evaluation::evaluate_field_alerts`]) so new
/// findings screen into fired alerts. Zero available inputs complete as a
/// logged no-op.
async fn execute_app_run(ctx: &PipelineWorkerContext, job: &PipelineJob) -> JobRunResult {
    let payload: AppRunPayload = match serde_json::from_str(&job.payload_json) {
        Ok(payload) => payload,
        Err(err) => return client_failure(format!("invalid app_run payload: {err}")),
    };
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    // v1 apps read the NDVI mean series; per-app index configuration arrives
    // with subscription-level app config (see `apps_for_index`).
    let series = match field_index_series(&ctx.pool, &payload.field_id, "ndvi", &payload.date).await
    {
        Ok(series) => series,
        Err(err) => return transient_failure(err.to_string()),
    };
    if series.is_empty() {
        tracing::info!(
            app_id = %payload.app_id,
            field_id = %payload.field_id,
            date = %payload.date,
            "app run: no field observations on or before date; completing as no-op"
        );
        return JobRunResult::Succeeded;
    }

    let run = match payload.app_id.as_str() {
        crate::crop_health_run::APP_ID => {
            run_crop_health_from_series(ctx, &payload, &series, &created_at).await
        }
        crate::anomaly_run::APP_ID => {
            run_anomaly_from_series(ctx, &payload, &series, &created_at).await
        }
        other => return client_failure(format!("unknown app_id {other:?}")),
    };
    let run = match run {
        Ok(run) => run,
        Err(err) => return application_failure(err),
    };

    // Alert hook: screen the field's findings (including this run's) into
    // fired alerts through the existing Track C evaluation. propose_action
    // stays off — alert evaluation alone never creates proposals.
    if let Err(err) = crate::alert_evaluation::evaluate_field_alerts(
        &ctx.pool,
        &payload.field_id,
        &crate::alert_evaluation::default_ruleset(),
        false,
        &created_at,
    )
    .await
    {
        return transient_failure(format!(
            "app run {} recorded but alert evaluation failed: {err}",
            run.run_id
        ));
    }
    tracing::info!(
        app_id = %payload.app_id,
        field_id = %payload.field_id,
        date = %payload.date,
        run_id = %run.run_id,
        findings = run.output_finding_ids.len(),
        "application run recorded and alerts evaluated"
    );
    JobRunResult::Succeeded
}

/// Crop health over the field series: one whole-field zone whose mean is the
/// latest observation and whose delta is the change from the previous one
/// (0 for a first observation), with both backing L2 products as inputs.
async fn run_crop_health_from_series(
    ctx: &PipelineWorkerContext,
    payload: &AppRunPayload,
    series: &[SeriesObservation],
    created_at: &str,
) -> Result<crate::applications::ApplicationRunRecord, ApplicationError> {
    let latest = series.last().expect("caller checked non-empty");
    let previous = series.len().checked_sub(2).map(|i| &series[i]);
    let mut input_product_ids = vec![latest.product_id.clone()];
    if let Some(previous) = previous {
        if previous.product_id != latest.product_id {
            input_product_ids.push(previous.product_id.clone());
        }
    }
    let zone = post_processor::crop_health_app::ZoneHealthInput {
        zone_id: field_entity_ref(&payload.field_id),
        mean_ndvi: latest.mean as f32,
        ndvi_delta: previous
            .map(|p| (latest.mean - p.mean) as f32)
            .unwrap_or(0.0),
        area_m2: product_area_m2(&ctx.pool, &latest.product_id).await,
        input_product_ids,
    };
    crate::crop_health_run::run(
        &ctx.pool,
        &crate::crop_health_run::CropHealthRunRequest {
            org_id: None,
            field_id: payload.field_id.clone(),
            zones: vec![zone],
            trend_epsilon: None,
            no_vegetation_threshold: None,
        },
        created_at,
    )
    .await
}

/// Anomaly screen over the field series: the population is the trailing
/// window of whole-field observations (one zone per observation), so the
/// statistical band flags an observation that departs from the field's own
/// recent history.
async fn run_anomaly_from_series(
    ctx: &PipelineWorkerContext,
    payload: &AppRunPayload,
    series: &[SeriesObservation],
    created_at: &str,
) -> Result<crate::applications::ApplicationRunRecord, ApplicationError> {
    let window = &series[series.len().saturating_sub(ANOMALY_POPULATION_LIMIT)..];
    let area_m2 = product_area_m2(
        &ctx.pool,
        &window.last().expect("caller checked non-empty").product_id,
    )
    .await;
    let zones: Vec<post_processor::anomaly_app::ZoneIndexInput> = window
        .iter()
        .map(|observation| post_processor::anomaly_app::ZoneIndexInput {
            zone_id: format!("obs:{}", observation.t),
            index_value: observation.mean as f32,
            area_m2,
            input_product_ids: vec![observation.product_id.clone()],
        })
        .collect();
    crate::anomaly_run::run(
        &ctx.pool,
        &crate::anomaly_run::AnomalyRunRequest {
            org_id: None,
            field_id: payload.field_id.clone(),
            zones,
            low_threshold: None,
            high_threshold: None,
            std_dev_multiplier: None,
        },
        created_at,
    )
    .await
}

// --- AOI decoding -------------------------------------------------------------

/// WGS84 envelope of a derive payload's `aoi_geojson`. Accepts a bare bbox
/// array `[min_lon, min_lat, max_lon, max_lat]`, a GeoJSON Feature (via its
/// `geometry`), an object carrying a `bbox`, or any geometry object whose
/// `coordinates` nest `[lon, lat, ...]` positions (Point through
/// MultiPolygon). Malformed AOIs are client errors: the job dead-letters.
pub fn aoi_bounds_from_geojson(value: &serde_json::Value) -> Result<GeoBounds, String> {
    if let Some(array) = value.as_array() {
        return bounds_from_bbox_array(array);
    }
    if let Some(object) = value.as_object() {
        if let Some(bbox) = object.get("bbox").and_then(serde_json::Value::as_array) {
            return bounds_from_bbox_array(bbox);
        }
        if let Some(geometry) = object.get("geometry") {
            return aoi_bounds_from_geojson(geometry);
        }
        if let Some(coordinates) = object.get("coordinates") {
            let mut bounds: Option<GeoBounds> = None;
            collect_position_envelope(coordinates, &mut bounds)?;
            return bounds.ok_or_else(|| "geometry has no coordinate positions".to_string());
        }
    }
    Err("expected a bbox array, a Feature, or a geometry with coordinates".to_string())
}

fn bounds_from_bbox_array(array: &[serde_json::Value]) -> Result<GeoBounds, String> {
    let values: Vec<f64> = array.iter().filter_map(serde_json::Value::as_f64).collect();
    if values.len() != 4 {
        return Err(format!(
            "bbox must be four numbers [min_lon, min_lat, max_lon, max_lat], got {array:?}"
        ));
    }
    Ok(GeoBounds {
        min_lon: values[0],
        min_lat: values[1],
        max_lon: values[2],
        max_lat: values[3],
    })
}

/// Recursively fold every `[lon, lat, ...]` position under `value` into the
/// running envelope. A position is an array whose first element is a number.
fn collect_position_envelope(
    value: &serde_json::Value,
    bounds: &mut Option<GeoBounds>,
) -> Result<(), String> {
    let array = value
        .as_array()
        .ok_or_else(|| format!("expected coordinate array, got {value}"))?;
    if array.is_empty() {
        return Ok(());
    }
    if array[0].is_number() {
        if array.len() < 2 {
            return Err(format!("position needs [lon, lat], got {value}"));
        }
        let lon = array[0]
            .as_f64()
            .ok_or_else(|| format!("non-finite longitude in {value}"))?;
        let lat = array[1]
            .as_f64()
            .ok_or_else(|| format!("non-finite latitude in {value}"))?;
        match bounds {
            None => {
                *bounds = Some(GeoBounds {
                    min_lon: lon,
                    max_lon: lon,
                    min_lat: lat,
                    max_lat: lat,
                });
            }
            Some(bounds) => {
                bounds.min_lon = bounds.min_lon.min(lon);
                bounds.max_lon = bounds.max_lon.max(lon);
                bounds.min_lat = bounds.min_lat.min(lat);
                bounds.max_lat = bounds.max_lat.max(lat);
            }
        }
        return Ok(());
    }
    for element in array {
        collect_position_envelope(element, bounds)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn aoi_bounds_accepts_bbox_array() {
        let bounds = aoi_bounds_from_geojson(&json!([76.64, 11.34, 76.65, 11.35])).unwrap();
        assert_eq!(bounds.min_lon, 76.64);
        assert_eq!(bounds.min_lat, 11.34);
        assert_eq!(bounds.max_lon, 76.65);
        assert_eq!(bounds.max_lat, 11.35);
    }

    #[test]
    fn aoi_bounds_envelopes_polygon_and_feature() {
        let polygon = json!({
            "type": "Polygon",
            "coordinates": [[
                [76.64, 11.34], [76.65, 11.34], [76.65, 11.35],
                [76.64, 11.35], [76.64, 11.34],
            ]],
        });
        let bounds = aoi_bounds_from_geojson(&polygon).unwrap();
        assert_eq!(
            (
                bounds.min_lon,
                bounds.min_lat,
                bounds.max_lon,
                bounds.max_lat
            ),
            (76.64, 11.34, 76.65, 11.35)
        );

        let feature = json!({ "type": "Feature", "properties": {}, "geometry": polygon });
        let via_feature = aoi_bounds_from_geojson(&feature).unwrap();
        assert_eq!(via_feature.min_lon, bounds.min_lon);
        assert_eq!(via_feature.max_lat, bounds.max_lat);
    }

    #[test]
    fn aoi_bounds_rejects_malformed_shapes() {
        assert!(aoi_bounds_from_geojson(&json!([1.0, 2.0])).is_err());
        assert!(aoi_bounds_from_geojson(&json!("not geojson")).is_err());
        assert!(aoi_bounds_from_geojson(&json!({ "type": "Polygon" })).is_err());
        assert!(
            aoi_bounds_from_geojson(&json!({ "type": "Polygon", "coordinates": [[[76.64]]] }))
                .is_err()
        );
        assert!(aoi_bounds_from_geojson(&json!({ "type": "Polygon", "coordinates": [] })).is_err());
    }

    #[test]
    fn apps_for_index_maps_ndvi_to_crop_health_and_everything_to_anomaly() {
        assert_eq!(
            apps_for_index("ndvi"),
            vec!["crop_health", "anomaly_detection"]
        );
        assert_eq!(apps_for_index("mndwi"), vec!["anomaly_detection"]);
        assert_eq!(apps_for_index("lst"), vec!["anomaly_detection"]);
    }

    #[test]
    fn month_window_covers_whole_month_and_rejects_garbage() {
        let (start, end) = month_window("2026-06").unwrap();
        assert_eq!(start.to_string(), "2026-06-01");
        assert_eq!(end.to_string(), "2026-06-30");
        let (start, end) = month_window("2025-12").unwrap();
        assert_eq!(start.to_string(), "2025-12-01");
        assert_eq!(end.to_string(), "2025-12-31");
        let (_, feb_end) = month_window("2024-02").unwrap();
        assert_eq!(feb_end.to_string(), "2024-02-29");
        assert!(month_window("2026-13").is_none());
        assert!(month_window("junk").is_none());
    }

    #[test]
    fn item_fetch_error_classifies_not_found_as_client() {
        let not_found = ItemFetchError::NotFound {
            collection: "sentinel-2-l2a".to_string(),
            item_id: "missing".to_string(),
        };
        assert!(not_found.is_client_error());
        assert!(!ItemFetchError::Upstream("timeout".to_string()).is_client_error());
    }
}
