#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::float_cmp)]
#![allow(clippy::ignored_unit_patterns)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::similar_names)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::fn_params_excessive_bools)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::single_match_else)]
#![allow(clippy::if_not_else)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::manual_let_else)]

//! 🔥 Carnelian OS Core Orchestrator
//!
//! The core orchestrator manages task scheduling, worker coordination,
//! capability-based security, event streaming, and local model inference.
//!
//! # Logging System
//!
//! Carnelian uses the `tracing` crate for structured logging with the following features:
//!
//! ## Log Levels
//!
//! - **ERROR**: Unrecoverable failures requiring immediate attention
//! - **WARN**: Degraded state or recoverable issues (e.g., database reconnection)
//! - **INFO**: Lifecycle events (startup, shutdown, configuration loaded)
//! - **DEBUG**: Detailed operational information (event storage, subscriptions)
//! - **TRACE**: Verbose debugging (sampling decisions, individual event processing)
//!
//! ## Environment-Based Formatting
//!
//! - **Production** (`CARNELIAN_ENV=production`): JSON output with full span context
//! - **Development** (default): Pretty-printed output with colors and line numbers
//!
//! ## Correlation IDs
//!
//! All HTTP requests receive a UUID v7 correlation ID via `CorrelationIdMakeSpan`.
//! Propagate correlation IDs through operations using spans:
//!
//! ```text
//! let span = tracing::info_span!("operation", correlation_id = %id);
//! let _guard = span.enter();
//! // All logs within this scope include correlation_id
//! ```
//!
//! ## Configuration
//!
//! | Variable | Description |
//! |----------|-------------|
//! | `LOG_LEVEL` | Default log level (ERROR, WARN, INFO, DEBUG, TRACE) |
//! | `RUST_LOG` | Per-module filtering (e.g., `carnelian_core=debug,sqlx=warn`) |
//! | `CARNELIAN_ENV` | Environment mode (`production` or `development`) |
//!
//! ## Structured Logging Best Practices
//!
//! Use structured fields instead of string interpolation:
//!
//! ```text
//! // Good: structured fields
//! tracing::info!(user_id = %id, action = "login", "User authenticated");
//!
//! // Avoid: string interpolation
//! tracing::info!("User {} authenticated with action login", id);
//! ```

pub mod agentic;
pub mod approvals;
pub mod attestation;
pub mod config;
pub mod context;
pub mod crypto;
pub mod db;
pub mod encryption;
pub mod events;
pub mod ledger;
pub mod memory;
pub mod metrics;
pub mod model_router;
pub mod policy;
pub mod safe_mode;
pub mod scheduler;
pub mod server;
pub mod session;
pub mod skills;
pub mod soul;
pub mod worker;

use std::env;
use tracing_subscriber::{
    EnvFilter, Layer,
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

pub use agentic::{
    AgenticEngine, AgenticRequest, AgenticResponse, DeclarativePlan, PlanStep, PlanStepResult,
    PlanStepStatus, ToolCall, ToolCallResult, ToolCallStatus,
};
pub use approvals::{ApprovalQueue, ApprovalRequest};
pub use carnelian_common::{Error, Result};
pub use config::Config;
pub use context::{
    ContextProvenance, ContextSegment, ContextWindow, SegmentPriority, SegmentSourceType,
};
pub use crypto::{generate_ed25519_keypair, sign_bytes, verify_signature};
pub use encryption::EncryptionHelper;
pub use events::{EventStream, EventStreamStats, PriorityRingBuffer};
pub use ledger::{Ledger, LedgerEvent};
pub use memory::{Memory, MemoryManager, MemoryQuery, MemorySource};
pub use metrics::MetricsCollector;
pub use model_router::{CompletionRequest, CompletionResponse, Message, ModelRouter, UsageStats};
pub use policy::{CapabilityGrant, PolicyEngine};
pub use safe_mode::SafeModeGuard;
pub use scheduler::Scheduler;
pub use server::{AppState, Server};
pub use session::{Session, SessionKey, SessionManager, SessionMessage, TokenCounters};
pub use skills::{SkillDiscovery, SkillManifest};
pub use soul::SoulManager;
pub use worker::WorkerManager;

/// Core orchestrator version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the global tracing subscriber with environment-based formatting.
///
/// # Arguments
///
/// * `log_level` - Default log level (ERROR, WARN, INFO, DEBUG, TRACE)
///
/// # Environment Variables
///
/// * `CARNELIAN_ENV` or `RUST_ENV` - Set to "production" for JSON output, otherwise pretty output
/// * `RUST_LOG` - Override per-module log levels (e.g., `carnelian_core=debug,sqlx=warn`)
///
/// # Errors
///
/// Returns an error if the global subscriber has already been initialized.
///
/// # Example
///
/// ```ignore
/// carnelian_core::init_tracing("INFO")?;
/// ```
pub fn init_tracing(log_level: &str) -> Result<()> {
    // Detect environment: production uses JSON, development uses pretty
    let is_production = env::var("CARNELIAN_ENV")
        .or_else(|_| env::var("RUST_ENV"))
        .map(|v| v.to_lowercase() == "production")
        .unwrap_or(false);

    // Build EnvFilter with provided log level as default, allow RUST_LOG overrides
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    if is_production {
        // Production: JSON output with full span context
        let json_layer = fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(true)
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_filter(env_filter);

        tracing_subscriber::registry()
            .with(json_layer)
            .try_init()
            .map_err(|e| Error::Config(format!("Failed to initialize tracing: {e}")))?;
    } else {
        // Development: pretty output with colors
        let pretty_layer = fmt::layer()
            .pretty()
            .with_ansi(true)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_span_events(FmtSpan::CLOSE)
            .with_filter(env_filter);

        tracing_subscriber::registry()
            .with(pretty_layer)
            .try_init()
            .map_err(|e| Error::Config(format!("Failed to initialize tracing: {e}")))?;
    }

    tracing::info!(
        version = VERSION,
        environment = if is_production {
            "production"
        } else {
            "development"
        },
        log_level = log_level,
        "🔥 Carnelian tracing initialized"
    );

    Ok(())
}
