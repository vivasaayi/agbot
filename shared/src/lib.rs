use anyhow::Result;
use serde::{Deserialize, Serialize};
// use uuid::Uuid; // uncomment when needed
// use chrono::{DateTime, Utc}; // uncomment when needed
// use nalgebra::{Point3, Vector3}; // uncomment when needed

pub mod config;
pub mod control_plane;
pub mod error;
pub mod fleet_alerts;
pub mod logging;
pub mod observability;
pub mod plugin_extensions;
pub mod resource_budget;
pub mod schemas;
pub mod secrets;
pub mod types;

pub use control_plane::*;
pub use fleet_alerts::*;
pub use logging::{
    active_logging_context, current_operation_span, init_logging, init_logging_with_context,
    logging_operation_span, with_correlation_id, LoggingContext, LoggingNodeIdSource,
};
pub use observability::*;
pub use resource_budget::*;
pub use secrets::*;
pub use types::*;

/// Common result type used across the workspace
pub type AgroResult<T> = Result<T, error::AgroError>;

/// Runtime modes for the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeMode {
    Simulation,
    Flight,
}

impl std::str::FromStr for RuntimeMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "SIMULATION" => Ok(RuntimeMode::Simulation),
            "FLIGHT" => Ok(RuntimeMode::Flight),
            _ => Err(anyhow::anyhow!("Invalid runtime mode: {}", s)),
        }
    }
}
