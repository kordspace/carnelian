//! Error types for 🔥 Carnelian OS

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Worker error: {0}")]
    Worker(String),

    #[error("Security error: {0}")]
    Security(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Keypair error: {0}")]
    Keypair(String),

    #[error("Environment variable error: {0}")]
    Environment(String),

    #[error("Event error: {0}")]
    Event(String),

    #[error("Broadcast channel error: {0}")]
    Broadcast(String),
}

impl<T> From<tokio::sync::broadcast::error::SendError<T>> for Error {
    fn from(err: tokio::sync::broadcast::error::SendError<T>) -> Self {
        Self::Broadcast(format!("Failed to send event: {err}"))
    }
}

impl From<config::ConfigError> for Error {
    fn from(err: config::ConfigError) -> Self {
        Self::Config(err.to_string())
    }
}

impl From<ed25519_dalek::SignatureError> for Error {
    fn from(err: ed25519_dalek::SignatureError) -> Self {
        Self::Security(err.to_string())
    }
}

impl From<std::env::VarError> for Error {
    fn from(err: std::env::VarError) -> Self {
        Self::Environment(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
