//! Error types for Carnelian

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Worker error: {0}")]
    Worker(String),

    #[error("Security error: {0}")]
    Security(String),
}

pub type Result<T> = std::result::Result<T, Error>;
