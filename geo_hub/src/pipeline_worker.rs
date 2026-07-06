//! Pipeline worker loop (batch S-7).
//!
//! A **serial** background worker that drains the SQLite job queue built in
//! `crate::pipeline`: recover orphaned `running` jobs on startup, then on
//! every poll tick claim at most one ready job, execute it, and run the
//! subscription cadence pass (due subscriptions -> `discover` jobs).
//!
//! Batch scope: only [`JobKind::Derive`] has a handler here. `discover`,
//! `l3_recompute`, `app_run`, and `backfill_enumerate` handlers arrive in
//! batches S-8/S-9/S-11; until then those jobs dead-letter immediately with a
//! `handler not implemented` **client** error so they cannot retry-loop.
//!
//! Everything the worker touches is injected through
//! [`PipelineWorkerContext`] — the DB pool, the hub config (data root +
//! `[pipeline]` cadence/politeness knobs), the COG store resolver, and the
//! STAC item fetcher — so tests drive [`run_one_tick`] deterministically
//! against in-memory fixtures with no network access. The server wiring
//! (spawning the loop when `pipeline.enabled` is set) lands in batch S-8.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use shared::schemas::GeoBounds;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::config::HubConfig;
use crate::db::DbPool;
use crate::earth_search::{self, EarthSearchItem};
use crate::pipeline::{self, DerivePayload, DiscoverPayload, JobKind, PipelineError, PipelineJob};
use crate::satellite_derivation::{
    derive_satellite_index, index_kind_from_key, CogStoreResolver, DeriveRequest,
};

/// Priority assigned to cadence-produced discover jobs. Zero keeps them
/// behind any operator-boosted work while still ahead of nothing by default.
const DISCOVER_PRIORITY: i64 = 0;

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
}

impl ItemFetchError {
    fn is_client_error(&self) -> bool {
        matches!(self, ItemFetchError::NotFound { .. })
    }
}

/// Seam for resolving a derive payload's `(collection, item_id)` to the full
/// STAC item. Production uses [`EarthSearchItemFetcher`]; tests inject a
/// fixture fetcher so the worker never touches the network.
pub trait StacItemFetcher: Send + Sync {
    fn fetch_item<'a>(
        &'a self,
        collection: &'a str,
        item_id: &'a str,
    ) -> BoxFuture<'a, Result<EarthSearchItem, ItemFetchError>>;
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
        // Conservative politeness: every derive job is assumed to have hit
        // the imagery provider (item fetch and/or COG range reads).
        outcome.hit_provider = matches!(job.kind, JobKind::Derive);
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

async fn execute_job(ctx: &PipelineWorkerContext, job: &PipelineJob) -> JobRunResult {
    match job.kind {
        JobKind::Derive => execute_derive(ctx, job).await,
        // S-8 (discover, backfill_enumerate), S-9 (l3_recompute), and S-11
        // (app_run) bring these handlers; until then the jobs dead-letter as
        // client errors instead of burning retries.
        JobKind::Discover | JobKind::L3Recompute | JobKind::AppRun | JobKind::BackfillEnumerate => {
            client_failure(format!(
                "handler not implemented for job kind {} (arrives in a later batch)",
                job.kind.as_str()
            ))
        }
    }
}

/// Execute one derive job: decode the payload, resolve the STAC item, and
/// run the exact same derivation the `POST /api/satellite/derive` route uses
/// ([`derive_satellite_index`]), which registers L0/L1/L2 lineage and — with
/// the payload's field scope — appends per-field time-series stats (S-3).
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
        field_id: Some(payload.field_id),
        season_id: payload.season_id,
    };
    match derive_satellite_index(
        &ctx.pool,
        &ctx.config.data_root,
        ctx.cog_resolver.as_ref(),
        &request,
    )
    .await
    {
        Ok(_) => JobRunResult::Succeeded,
        Err(err) => JobRunResult::Failed {
            client_error: err.is_client_error(),
            error: err.to_string(),
        },
    }
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
    fn item_fetch_error_classifies_not_found_as_client() {
        let not_found = ItemFetchError::NotFound {
            collection: "sentinel-2-l2a".to_string(),
            item_id: "missing".to_string(),
        };
        assert!(not_found.is_client_error());
        assert!(!ItemFetchError::Upstream("timeout".to_string()).is_client_error());
    }
}
