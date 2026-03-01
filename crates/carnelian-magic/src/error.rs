//! Error types for carnelian-magic

use thiserror::Error;

/// Error type for carnelian-magic operations
#[derive(Debug, Error)]
pub enum MagicError {
    /// No entropy provider could supply bytes
    #[error("Entropy unavailable: {0}")]
    EntropyUnavailable(String),

    /// A specific provider failed
    #[error("Provider error from {provider}: {message}")]
    ProviderError {
        provider: String,
        message: String,
    },

    /// Network failure
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// Python skill invocation failed
    #[error("Skill bridge error: {0}")]
    SkillBridgeError(String),

    /// JSON parsing failure
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for carnelian-magic operations
pub type Result<T> = std::result::Result<T, MagicError>;
