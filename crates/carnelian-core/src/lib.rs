//! Carnelian OS Core Orchestrator
//!
//! The core orchestrator manages task scheduling, worker coordination,
//! capability-based security, event streaming, and local model inference.

pub mod config;
pub mod db;
pub mod ledger;
pub mod policy;
pub mod scheduler;
pub mod server;
pub mod worker;

pub use carnelian_common::{Error, Result};

/// Core orchestrator version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
