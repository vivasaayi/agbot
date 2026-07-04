//! Alert lifecycle (Track C phase C2).
//!
//! Persists the governed fired->acknowledged->resolved state machine per fired
//! alert, driving each transition through the shared `alerting` lifecycle engine
//! (`open_alert_lifecycle` / `acknowledge_alert` / `resolve_alert`). The engine
//! enforces the legal transition order and idempotency; this module only loads,
//! saves, and exposes the record. Transitions are appended to an audit log on
//! the record.

use crate::alert_evaluation::{self, AlertEvaluationError};
use crate::db::DbPool;
use alerting::{
    acknowledge_alert, open_alert_lifecycle, resolve_alert, AlertLifecycleAction,
    AlertLifecycleRecord, AlertLifecycleState, AlertingError,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AlertLifecycleError {
    #[error("alert {0} not found")]
    AlertNotFound(String),
    #[error("unknown lifecycle state: {0}")]
    UnknownState(String),
    #[error(transparent)]
    Alerting(#[from] AlertingError),
    #[error(transparent)]
    Evaluation(#[from] AlertEvaluationError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("failed to serialize {what}: {source}")]
    Serialize {
        what: &'static str,
        source: serde_json::Error,
    },
}

/// The transition an operator requests on an alert's lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleTransition {
    Acknowledge,
    Resolve,
}

/// The lifecycle state's persisted string form (matches the enum's serde
/// snake_case representation). `AlertLifecycleState` exposes neither `as_str`
/// nor `FromStr`, so the mapping lives here.
fn state_to_str(state: AlertLifecycleState) -> &'static str {
    match state {
        AlertLifecycleState::Fired => "fired",
        AlertLifecycleState::Acknowledged => "acknowledged",
        AlertLifecycleState::Resolved => "resolved",
        AlertLifecycleState::AutoResolved => "auto_resolved",
    }
}

fn state_from_str(value: &str) -> Result<AlertLifecycleState, AlertLifecycleError> {
    match value {
        "fired" => Ok(AlertLifecycleState::Fired),
        "acknowledged" => Ok(AlertLifecycleState::Acknowledged),
        "resolved" => Ok(AlertLifecycleState::Resolved),
        "auto_resolved" => Ok(AlertLifecycleState::AutoResolved),
        other => Err(AlertLifecycleError::UnknownState(other.to_string())),
    }
}

/// Load a persisted lifecycle record, or `None` if the alert has none yet.
pub async fn load_lifecycle(
    pool: &DbPool,
    alert_id: &str,
) -> Result<Option<AlertLifecycleRecord>, AlertLifecycleError> {
    use sqlx::Row;
    let Some(row) = sqlx::query("SELECT * FROM alert_lifecycle WHERE alert_id = ?")
        .bind(alert_id)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(None);
    };
    let state = state_from_str(&row.get::<String, _>("state"))?;
    let transitions =
        serde_json::from_str(&row.get::<String, _>("transitions_json")).map_err(|source| {
            AlertLifecycleError::Serialize {
                what: "transitions",
                source,
            }
        })?;
    Ok(Some(AlertLifecycleRecord {
        alert_id: row.get("alert_id"),
        source_event_ref: row.get("source_event_ref"),
        state,
        fired_at: row.get("fired_at"),
        transitions,
    }))
}

/// Fetch a lifecycle record, opening one from the fired alert if none exists yet.
pub async fn get_or_open_lifecycle(
    pool: &DbPool,
    alert_id: &str,
    updated_at: &str,
) -> Result<AlertLifecycleRecord, AlertLifecycleError> {
    if let Some(record) = load_lifecycle(pool, alert_id).await? {
        return Ok(record);
    }
    let fired = alert_evaluation::get_fired_alert(pool, alert_id)
        .await?
        .ok_or_else(|| AlertLifecycleError::AlertNotFound(alert_id.to_string()))?;
    let record = open_alert_lifecycle(&fired)?;
    save_lifecycle(pool, &record, updated_at).await?;
    Ok(record)
}

async fn save_lifecycle(
    pool: &DbPool,
    record: &AlertLifecycleRecord,
    updated_at: &str,
) -> Result<(), AlertLifecycleError> {
    let transitions_json = serde_json::to_string(&record.transitions).map_err(|source| {
        AlertLifecycleError::Serialize {
            what: "transitions",
            source,
        }
    })?;
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO alert_lifecycle
            (alert_id, source_event_ref, state, fired_at, transitions_json, updated_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&record.alert_id)
    .bind(&record.source_event_ref)
    .bind(state_to_str(record.state))
    .bind(&record.fired_at)
    .bind(transitions_json)
    .bind(updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Apply a lifecycle transition to an alert: open the lifecycle if needed, run
/// the requested transition through the `alerting` engine (which enforces legal
/// order + idempotency), persist, and return the resulting action.
pub async fn transition(
    pool: &DbPool,
    alert_id: &str,
    transition: LifecycleTransition,
    actor_id: &str,
    at: &str,
) -> Result<AlertLifecycleAction, AlertLifecycleError> {
    let mut record = get_or_open_lifecycle(pool, alert_id, at).await?;
    let action = match transition {
        LifecycleTransition::Acknowledge => {
            acknowledge_alert(&mut record, actor_id.to_string(), at.to_string())?
        }
        LifecycleTransition::Resolve => {
            resolve_alert(&mut record, actor_id.to_string(), at.to_string())?
        }
    };
    save_lifecycle(pool, &record, at).await?;
    Ok(action)
}
