use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub mod config;
pub mod error;
pub mod schemas;

/// Initialize logging for the application
pub fn init_logging() -> Result<()> {
    dotenvy::dotenv().ok();
    
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    Ok(())
}

/// Common result type used across the workspace
pub type AgroResult<T> = Result<T, error::AgroError>;

/// Runtime modes for the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
