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
#![allow(clippy::too_many_lines)]
#![allow(clippy::needless_raw_string_hashes)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(clippy::use_self)]
#![allow(clippy::manual_clamp)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::unnecessary_map_or)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::significant_drop_in_scrutinee)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unused_self)]
#![allow(clippy::unused_async)]
#![allow(clippy::unnested_or_patterns)]

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
pub mod chain_anchor;
pub mod config;
pub mod context;
pub mod context_analyzer;
pub mod crypto;
pub mod db;
pub mod elixir;
pub mod encryption;
pub mod events;
pub mod ledger;
pub mod memory;
pub mod metrics;
pub mod middleware;
pub mod model_router;
pub mod policy;
pub mod providers;
pub mod safe_mode;
pub mod scheduler;
pub mod secrets;
pub mod server;
pub mod session;
pub mod skill_book;
pub mod skills;
pub mod soul;
pub mod sub_agent;
pub mod voice;
pub mod worker;
pub mod workflow;
pub mod xp;

use std::env;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
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
pub use elixir::ElixirManager;
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
pub use sub_agent::{
    CreateSubAgentRequest, IdentityPack, SubAgent, SubAgentManager, UpdateSubAgentRequest,
};
pub use voice::VoiceGateway;
pub use worker::WorkerManager;
pub use workflow::WorkflowEngine;
pub use xp::XpManager;

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

    // Check for file-based logging via CARNELIAN_LOG_FILE
    let log_file = env::var("CARNELIAN_LOG_FILE").ok();
    let max_files = env::var("CARNELIAN_LOG_MAX_FILES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(5);

    // Rotation frequency: "hourly" (default, bounds per-file size for long runs) or "daily"
    // Known limitation (v1.0.0): hourly rotation used as a file-size guard; true size-based
    // rotation requires switching to `rolling-file` crate, deferred. tracing-appender does
    // not support native size-based rotation; hourly rotation prevents unbounded file growth
    // during long validation runs.
    let rotation = match env::var("CARNELIAN_LOG_ROTATION")
        .unwrap_or_else(|_| "hourly".to_string())
        .to_lowercase()
        .as_str()
    {
        "daily" => tracing_appender::rolling::Rotation::DAILY,
        "minutely" => tracing_appender::rolling::Rotation::MINUTELY,
        _ => tracing_appender::rolling::Rotation::HOURLY,
    };

    // Build EnvFilter with provided log level as default, allow RUST_LOG overrides
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    if let Some(ref log_path) = log_file {
        // File-based logging: JSON to rotating file + pretty to stdout
        let path = std::path::Path::new(log_path);
        let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let prefix = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("carnelian.log");

        // Time-based rotation, keeping up to max_files rotated logs
        let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
            .rotation(rotation)
            .filename_prefix(prefix)
            .max_log_files(max_files)
            .build(dir)
            .map_err(|e| Error::Config(format!("Failed to create log file appender: {e}")))?;

        let file_layer = fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(true)
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_writer(file_appender)
            .with_filter(EnvFilter::new(log_level));

        let stdout_layer = fmt::layer()
            .pretty()
            .with_ansi(true)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_span_events(FmtSpan::CLOSE)
            .with_filter(env_filter);

        tracing_subscriber::registry()
            .with(file_layer)
            .with(stdout_layer)
            .try_init()
            .map_err(|e| Error::Config(format!("Failed to initialize tracing: {e}")))?;
    } else if is_production {
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
        log_file = log_file.as_deref().unwrap_or("stdout"),
        "🔥 Carnelian tracing initialized"
    );

    Ok(())
}
