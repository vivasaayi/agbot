use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgroError {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("MAVLink communication error: {0}")]
    Mavlink(String),
    
    #[error("Sensor error: {0}")]
    Sensor(String),
    
    #[error("Processing error: {0}")]
    Processing(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Hardware not available in simulation mode")]
    SimulationMode,
    
    #[error("Unknown error: {0}")]
    Other(#[from] anyhow::Error),
}
