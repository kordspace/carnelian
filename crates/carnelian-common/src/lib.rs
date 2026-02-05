//! Carnelian Common Types and Utilities
//!
//! Shared types, error handling, and utilities used across all crates.

pub mod error;
pub mod types;

pub use error::{Error, Result};

/// Carnelian version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
