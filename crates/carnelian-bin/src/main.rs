#![allow(clippy::uninlined_format_args)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::single_match_else)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::map_unwrap_or)]

//! 🔥 Carnelian OS CLI
//!
//! Command-line interface for the Carnelian local-first AI agent mainframe.
//!
//! # Commands
//!
//! - `carnelian start` - Start the orchestrator
//! - `carnelian stop` - Stop a running instance
//! - `carnelian status` - Query the status endpoint
//! - `carnelian migrate` - Run database migrations
//! - `carnelian logs` - Stream events from running instance

use std::path::PathBuf;
use std::time::Duration;
use sysinfo::System;

use carnelian_common::types::{CreateTaskRequest, CreateTaskResponse, EventEnvelope, EventLevel};
use clap::{Parser, Subcommand};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio_tungstenite::tungstenite::protocol::Message;
use uuid::Uuid;

use carnelian_core::{
    Config, EventStream, Ledger, ModelRouter, PolicyEngine, Scheduler, Server, WorkerManager,
};

use bollard::Docker;
use bollard::container::{
    Config as ContainerConfig, CreateContainerOptions, HostConfig, PortBinding,
    StartContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::models::{HostConfig as HostConfigModel, PortMap};
use futures_util::stream::TryStreamExt;

/// 🔥 Carnelian OS - Local-first AI agent mainframe
#[derive(Parser)]
#[command(name = "carnelian")]
#[command(version = carnelian_common::VERSION)]
#[command(about = "🔥 Carnelian OS - Local-first AI agent mainframe")]
#[command(after_help = "EXAMPLES:
  carnelian start                    Start the orchestrator
  carnelian start --log-level DEBUG  Start with debug logging
  carnelian status                   Check if running
  carnelian stop                     Stop gracefully
  carnelian migrate                  Run database migrations
  carnelian migrate --dry-run        Show pending migrations without applying
  carnelian logs                     Stream events from running instance
  carnelian task create \"My task\"           Create a new task
  carnelian task create \"Task\" --priority 10  Create high-priority task
  carnelian logs -f --level ERROR    Stream only ERROR events
  carnelian logs --url http://remote:18789  Connect to remote instance
  carnelian --database-url postgres://user:pass@host/db migrate  Use specific database")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to configuration file (default: machine.toml)
    #[arg(long, global = true, env = "CARNELIAN_CONFIG")]
    config: Option<PathBuf>,

    /// Override log level (ERROR, WARN, INFO, DEBUG, TRACE)
    #[arg(long, global = true, env = "LOG_LEVEL")]
    log_level: Option<String>,

    /// Override database URL (takes precedence over config file and environment)
    #[arg(long, global = true, env = "DATABASE_URL")]
    database_url: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Carnelian orchestrator
    Start,

    /// Stop a running Carnelian instance
    Stop,

    /// Query the status of a running instance
    Status {
        /// URL of the Carnelian server
        #[arg(long, env = "CARNELIAN_URL")]
        url: Option<String>,
    },

    /// Run database migrations
    Migrate {
        /// Show pending migrations without applying them
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },

    /// Stream events from a running Carnelian instance
    Logs {
        /// URL of the Carnelian server
        #[arg(long, env = "CARNELIAN_URL")]
        url: Option<String>,

        /// Keep connection open and stream events continuously
        #[arg(long, short = 'f')]
        follow: bool,

        /// Filter events by minimum level (ERROR, WARN, INFO, DEBUG, TRACE)
        #[arg(long)]
        level: Option<String>,

        /// Filter events by type (e.g., `TaskCreated`, `WorkerStarted`)
        #[arg(long)]
        event_type: Option<String>,
    },

    /// Skill management commands
    Skills {
        #[command(subcommand)]
        command: SkillsCommands,
    },

    /// Task management commands
    Task {
        #[command(subcommand)]
        command: TaskCommands,

        /// URL of the Carnelian server
        #[arg(long, env = "CARNELIAN_URL")]
        url: Option<String>,
    },

    /// Initialize Carnelian configuration (interactive setup wizard)
    Init {
        /// Skip all prompts, accept defaults
        #[arg(long, short = 'y')]
        non_interactive: bool,

        /// Overwrite existing machine.toml without asking
        #[arg(long)]
        force: bool,

        /// Skip already-completed steps (used by next phase)
        #[arg(long)]
        resume: bool,

        /// Path to an existing key file
        #[arg(long)]
        key_path: Option<PathBuf>,
    },

    /// Launch the desktop UI (or web UI with --web flag)
    Ui {
        /// Launch web UI instead of desktop
        #[arg(long)]
        web: bool,
    },

    /// Generate a new owner keypair
    Keygen {
        /// Output path for the key file (default: ~/.carnelian/owner.key)
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Key management commands
    Key {
        #[command(subcommand)]
        command: KeyCommands,
    },

    /// Migrate from Thummim project
    MigrateFromThummim {
        /// Path to Thummim project root
        #[arg(long)]
        path: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum SkillsCommands {
    /// Manually refresh skill registry (scan for new/updated/removed skills)
    Refresh,
}

#[derive(Subcommand)]
enum TaskCommands {
    /// Create a new task
    Create {
        /// Title for the task
        title: String,

        /// Optional description
        #[arg(long)]
        description: Option<String>,

        /// Optional skill ID (UUID) to execute
        #[arg(long)]
        skill_id: Option<String>,

        /// Task priority (higher = dequeued first)
        #[arg(long, default_value_t = 0)]
        priority: i32,
    },
}

#[derive(Subcommand)]
enum KeyCommands {
    /// Rotate the owner keypair
    Rotate,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Start => handle_start(cli.config, cli.log_level, cli.database_url).await,
        Commands::Stop => handle_stop().await,
        Commands::Status { url } => handle_status(&resolve_url(url)).await,
        Commands::Migrate { dry_run } => {
            handle_migrate(cli.config, cli.log_level, dry_run, cli.database_url).await
        }
        Commands::Logs {
            url,
            follow,
            level,
            event_type,
        } => handle_logs(&resolve_url(url), follow, level, event_type).await,
        Commands::Skills { command } => {
            handle_skills(command, cli.config, cli.log_level, cli.database_url).await
        }
        Commands::Task { command, url } => handle_task_command(command, &resolve_url(url)).await,
        Commands::Init { non_interactive, force, resume, key_path } => {
            handle_init(cli.config, cli.log_level, cli.database_url, non_interactive, force, resume, key_path).await
        }
        Commands::Ui { web } => handle_ui(web).await,
        Commands::Keygen { output } => handle_keygen(output).await,
        Commands::Key { command } => {
            handle_key(command, cli.config, cli.log_level, cli.database_url).await
        }
        Commands::MigrateFromThummim { path } => {
            handle_migrate_from_thummim(path, cli.config, cli.log_level, cli.database_url).await
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        if let carnelian_common::Error::ExitCode(code, _) = e {
            std::process::exit(code);
        }
        std::process::exit(1);
    }
}

/// Resolve the server URL from an explicit value, `CARNELIAN_HTTP_PORT` env var, or default.
fn resolve_url(explicit: Option<String>) -> String {
    if let Some(url) = explicit {
        return url;
    }
    if let Ok(port) = std::env::var("CARNELIAN_HTTP_PORT") {
        return format!("http://localhost:{}", port);
    }
    "http://localhost:18789".to_string()
}

/// Handle the `start` command - launch the orchestrator
async fn handle_start(
    config_path: Option<PathBuf>,
    log_level_override: Option<String>,
    database_url_override: Option<String>,
) -> carnelian_common::Result<()> {
    // Load configuration first (before tracing, since Config::load initializes tracing)
    let mut config = if let Some(path) = config_path {
        Config::load_from_file(&path)?
    } else {
        // Use default loading which handles machine.toml + env vars
        // But we need to do it without the tracing init first
        Config::load_from_file(std::path::Path::new("machine.toml")).unwrap_or_default()
    };

    // Apply environment overrides
    config.apply_env_overrides()?;

    // Override log level if specified via CLI
    if let Some(level) = log_level_override {
        config.log_level = level.to_uppercase();
    }

    // Initialize tracing
    carnelian_core::init_tracing(&config.log_level)?;

    // Log startup banner
    tracing::info!(
        version = carnelian_common::VERSION,
        "🔥 Carnelian OS starting..."
    );

    // Override database URL if specified via CLI (takes precedence over config and env)
    if let Some(url) = database_url_override {
        config.database_url = url;
    }

    // Validate configuration
    config.validate()?;

    // Connect to database
    tracing::info!("Connecting to database...");
    config.connect_database().await?;

    // Run migrations
    if let Ok(pool) = config.pool() {
        tracing::info!("Running database migrations...");
        carnelian_core::db::run_migrations(pool, None).await?;
    }

    // Load owner keypair
    config.load_owner_keypair()?;
    config.load_owner_keypair_from_db().await?;

    // Create event stream with configured capacity
    let event_stream = Arc::new(EventStream::with_max_payload(
        config.event_buffer_capacity,
        config.event_broadcast_capacity,
        config.event_max_payload_bytes,
    ));

    // Create policy engine with database pool
    let policy_engine = PolicyEngine::new(config.pool()?.clone());

    // Create audit ledger and verify chain integrity
    let ledger = Ledger::new(config.pool()?.clone());
    ledger.load_last_hash().await?;

    // Create session manager with safe mode guard (needed for factory and server)
    let safe_mode_guard = Arc::new(carnelian_core::SafeModeGuard::new(
        config.pool()?.clone(),
        ledger.clone(),
    ));
    let session_manager = Arc::new(
        carnelian_core::session::SessionManager::with_defaults(config.pool()?.clone())
            .with_safe_mode_guard(safe_mode_guard.clone()),
    );

    match ledger
        .verify_chain(config.owner_public_key.as_deref())
        .await
    {
        Ok(true) => {
            tracing::info!("Ledger chain verification passed");
        }
        Ok(false) => {
            tracing::error!("Ledger chain verification FAILED — tampered or corrupted audit trail");
            return Err(carnelian_common::Error::Config(
                "Ledger chain verification failed".to_string(),
            ));
        }
        Err(e) => {
            tracing::error!(error = %e, "Ledger chain verification error");
            return Err(e);
        }
    }

    let ledger = Arc::new(ledger);

    // Create worker manager
    let config_arc = Arc::new(config);
    let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
        config_arc.clone(),
        event_stream.clone(),
    )));

    // Create model router for LLM calls
    let policy_engine = Arc::new(policy_engine);
    let model_router = Arc::new(ModelRouter::new(
        config_arc.pool()?.clone(),
        config_arc.gateway_url.clone(),
        policy_engine.clone(),
        ledger.clone(),
    ));

    // Create scheduler with heartbeat interval from config, worker manager, and config
    let scheduler = Scheduler::new(
        config_arc.pool()?.clone(),
        event_stream.clone(),
        Duration::from_millis(config_arc.heartbeat_interval_ms),
        worker_manager.clone(),
        config_arc.clone(),
        model_router,
        ledger.clone(),
        safe_mode_guard.clone(),
    );

    // Create the adapter factory with all required dependencies
    let adapter_factory = Arc::new(carnelian_adapters::factory::DefaultAdapterFactory::new(
        config_arc.pool()?.clone(),
        session_manager.clone(),
        event_stream.clone(),
        policy_engine.clone(),
        0.8,   // spam_threshold - read from config or use sensible default
        3600,  // spam_ttl_secs
    ));

    // Create server with session manager and adapter factory wired in
    let server = Server::new(
        config_arc,
        event_stream,
        policy_engine,
        ledger,
        Arc::new(tokio::sync::Mutex::new(scheduler)),
        worker_manager,
    )
    .with_session_manager(session_manager)
    .with_adapter_factory(adapter_factory);

    // Write PID file only after all initialization succeeds
    // This prevents stale PID files if startup fails
    write_pid_file()?;

    tracing::info!("🔥 Carnelian OS ready");

    // Run server (blocks until shutdown signal)
    server.run().await?;

    // Cleanup PID file on graceful shutdown
    remove_pid_file();

    tracing::info!("🔥 Carnelian OS stopped");
    Ok(())
}

/// Handle the `stop` command - send shutdown signal to running instance
async fn handle_stop() -> carnelian_common::Result<()> {
    let pid_path = get_pid_file_path()?;

    if !pid_path.exists() {
        println!("No running Carnelian instance found (PID file not present)");
        println!("Hint: Check with 'ps aux | grep carnelian' or 'pkill carnelian'");
        return Ok(());
    }

    let pid_str = std::fs::read_to_string(&pid_path)
        .map_err(|e| carnelian_common::Error::Config(format!("Failed to read PID file: {}", e)))?;

    let pid: u32 = pid_str
        .trim()
        .parse()
        .map_err(|e| carnelian_common::Error::Config(format!("Invalid PID in file: {}", e)))?;

    println!("Sending shutdown signal to Carnelian (PID: {})...", pid);

    // Send SIGTERM using shell command (avoids unsafe code)
    #[cfg(unix)]
    {
        let status = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .map_err(|e| {
                carnelian_common::Error::Config(format!("Failed to send signal: {}", e))
            })?;

        if !status.success() {
            println!("Process not found or permission denied. Removing stale PID file.");
            remove_pid_file();
            return Ok(());
        }
    }

    #[cfg(windows)]
    {
        let status = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string()])
            .status()
            .map_err(|e| {
                carnelian_common::Error::Config(format!("Failed to run taskkill: {}", e))
            })?;

        if !status.success() {
            println!("Process may not exist. Removing stale PID file.");
            remove_pid_file();
            return Ok(());
        }
    }

    // Wait for process to exit
    println!("Waiting for graceful shutdown...");
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(10);

    while start.elapsed() < timeout {
        if !is_process_running(pid) {
            println!("✓ Carnelian stopped gracefully");
            remove_pid_file();
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    println!("⚠ Process did not stop within 10 seconds");
    println!("You may need to manually terminate with: kill -9 {}", pid);
    Ok(())
}

/// Handle the `status` command - query the running instance
async fn handle_status(url: &str) -> carnelian_common::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| {
            carnelian_common::Error::Config(format!("Failed to create HTTP client: {}", e))
        })?;

    // Query health endpoint
    let health_url = format!("{}/v1/health", url);
    let health_result = client.get(&health_url).send().await;

    let (status, database) = match health_result {
        Ok(resp) if resp.status().is_success() => {
            let health: serde_json::Value = resp.json().await.unwrap_or_default();
            (
                health["status"].as_str().unwrap_or("unknown").to_string(),
                health["database"].as_str().unwrap_or("unknown").to_string(),
            )
        }
        Ok(resp) => {
            return Err(carnelian_common::Error::Config(format!(
                "Health check failed with status: {}",
                resp.status()
            )));
        }
        Err(e) => {
            if e.is_connect() {
                println!("🔥 Carnelian is not running");
                println!("   URL: {}", url);
                std::process::exit(1);
            }
            return Err(carnelian_common::Error::Config(format!(
                "Failed to connect: {}",
                e
            )));
        }
    };

    // Query status endpoint
    let status_url = format!("{}/v1/status", url);
    let status_result = client.get(&status_url).send().await;

    let (workers, models, queue_depth, workers_array) = match status_result {
        Ok(resp) if resp.status().is_success() => {
            let status_resp: serde_json::Value = resp.json().await.unwrap_or_default();
            (
                status_resp["workers"]
                    .as_array()
                    .map(|a| a.len())
                    .unwrap_or(0),
                status_resp["models"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
                status_resp["queue_depth"].as_u64().unwrap_or(0),
                status_resp["workers"].as_array().cloned(),
            )
        }
        _ => (0, vec![], 0, None),
    };

    // Display status
    println!("🔥 Carnelian Status");
    println!("   Version:     {}", carnelian_common::VERSION);
    println!("   Status:      {}", status);
    println!("   Database:    {}", database);
    println!("   Workers:     {} active", workers);

    // Print per-worker details
    if let Some(workers) = workers_array {
        for worker in workers {
            let runtime = worker["runtime"].as_str().unwrap_or("unknown");
            let worker_status = worker["status"].as_str().unwrap_or("unknown");
            let id = worker["id"].as_str().unwrap_or("unknown");
            println!("     • {:8} {:8} ({})", runtime, worker_status, id);
        }
    }

    println!("   Queue Depth: {}", queue_depth);
    if !models.is_empty() {
        println!("   Models:      {:?}", models);
    }

    Ok(())
}

/// Get the path to the PID file
fn get_pid_file_path() -> carnelian_common::Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| {
            carnelian_common::Error::Config("Could not determine home directory".to_string())
        })?;

    Ok(PathBuf::from(home).join(".carnelian").join("carnelian.pid"))
}

/// Write the current process ID to the PID file
fn write_pid_file() -> carnelian_common::Result<()> {
    let pid_path = get_pid_file_path()?;

    if let Some(parent) = pid_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            carnelian_common::Error::Config(format!("Failed to create PID directory: {}", e))
        })?;
    }

    std::fs::write(&pid_path, std::process::id().to_string())
        .map_err(|e| carnelian_common::Error::Config(format!("Failed to write PID file: {}", e)))?;

    tracing::debug!(pid_file = ?pid_path, pid = std::process::id(), "PID file written");
    Ok(())
}

/// Remove the PID file
fn remove_pid_file() {
    if let Ok(pid_path) = get_pid_file_path() {
        let _ = std::fs::remove_file(pid_path);
    }
}

/// Check if a process is running
fn is_process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // Use kill -0 to check if process exists (sends no signal, just checks)
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
}

/// Handle the `migrate` command - run database migrations
async fn handle_migrate(
    config_path: Option<PathBuf>,
    log_level_override: Option<String>,
    dry_run: bool,
    database_url_override: Option<String>,
) -> carnelian_common::Result<()> {
    // Load configuration
    let mut config = if let Some(path) = config_path {
        Config::load_from_file(&path)?
    } else {
        Config::load_from_file(std::path::Path::new("machine.toml")).unwrap_or_default()
    };

    // Apply environment overrides
    config.apply_env_overrides()?;

    // Override log level if specified via CLI
    if let Some(level) = log_level_override {
        config.log_level = level.to_uppercase();
    }

    // Override database URL if specified via CLI (takes precedence over config and env)
    if let Some(url) = database_url_override {
        config.database_url = url;
    }

    // Initialize tracing
    carnelian_core::init_tracing(&config.log_level)?;

    tracing::info!("🔥 Carnelian migrate starting...");

    // Connect to database
    tracing::info!("Connecting to database...");
    config.connect_database().await?;

    let pool = config.pool()?;

    // Load embedded migrations from db/migrations relative to workspace root
    let migrator = sqlx::migrate!("../../db/migrations");

    if dry_run {
        tracing::info!("Dry-run mode: checking pending migrations...");

        // Get applied migration versions from database
        let applied_versions: std::collections::HashSet<i64> =
            sqlx::query_scalar("SELECT version FROM _sqlx_migrations")
                .fetch_all(pool)
                .await
                .unwrap_or_default()
                .into_iter()
                .collect();

        // Compute pending migrations by diffing embedded vs applied
        let mut pending: Vec<_> = migrator
            .iter()
            .filter(|m| !applied_versions.contains(&m.version))
            .collect();
        pending.sort_by_key(|m| m.version);

        // Also show applied migrations for context
        let mut applied: Vec<_> = migrator
            .iter()
            .filter(|m| applied_versions.contains(&m.version))
            .collect();
        applied.sort_by_key(|m| m.version);

        if !applied.is_empty() {
            println!("Applied migrations:");
            for m in &applied {
                println!("  ✓ V{}: {}", m.version, m.description);
            }
            println!();
        }

        if pending.is_empty() {
            println!("No pending migrations. Database is up to date.");
        } else {
            println!("Pending migrations ({}):", pending.len());
            for m in &pending {
                println!("  → V{}: {}", m.version, m.description);
            }
        }

        println!("\nDry-run complete. No changes were made.");
    } else {
        // Run migrations
        tracing::info!("Running database migrations...");
        carnelian_core::db::run_migrations(pool, None).await?;

        println!("✓ Migrations completed successfully");
    }

    Ok(())
}

/// Handle the `skills` subcommands
async fn handle_skills(
    command: SkillsCommands,
    config_path: Option<PathBuf>,
    log_level_override: Option<String>,
    database_url_override: Option<String>,
) -> carnelian_common::Result<()> {
    match command {
        SkillsCommands::Refresh => {
            // Load configuration
            let mut config = if let Some(path) = config_path {
                Config::load_from_file(&path)?
            } else {
                Config::load_from_file(std::path::Path::new("machine.toml")).unwrap_or_default()
            };

            config.apply_env_overrides()?;

            if let Some(level) = log_level_override {
                config.log_level = level.to_uppercase();
            }

            if let Some(url) = database_url_override {
                config.database_url = url;
            }

            carnelian_core::init_tracing(&config.log_level)?;

            tracing::info!("🔥 Carnelian skills refresh starting...");

            config.connect_database().await?;
            let pool = config.pool()?.clone();

            // Run migrations to ensure schema is up to date
            carnelian_core::db::run_migrations(&pool, None).await?;

            let discovery = carnelian_core::SkillDiscovery::new(
                pool,
                None, // No event stream for CLI
                config.skills_registry_path.clone(),
            );

            let result = discovery.refresh().await?;

            println!("🔥 Skill Registry Refresh Complete");
            println!("   Discovered: {}", result.discovered);
            println!("   Updated:    {}", result.updated);
            println!("   Removed:    {}", result.removed);

            Ok(())
        }
    }
}

/// Handle the `task` subcommands
async fn handle_task_command(command: TaskCommands, url: &str) -> carnelian_common::Result<()> {
    match command {
        TaskCommands::Create {
            title,
            description,
            skill_id,
            priority,
        } => {
            // Parse skill_id if provided
            let parsed_skill_id = if let Some(ref sid) = skill_id {
                Some(Uuid::parse_str(sid).map_err(|_| {
                    carnelian_common::Error::Config(format!(
                        "Invalid skill ID format. Expected UUID (e.g., 550e8400-e29b-41d4-a716-446655440000), got: {}",
                        sid
                    ))
                })?)
            } else {
                None
            };

            let request = CreateTaskRequest {
                title,
                description,
                skill_id: parsed_skill_id,
                priority,
                requires_approval: false,
            };

            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|e| {
                    carnelian_common::Error::Config(format!("Failed to create HTTP client: {}", e))
                })?;

            let resp = client
                .post(format!("{}/v1/tasks", url.trim_end_matches('/')))
                .json(&request)
                .send()
                .await
                .map_err(|e| {
                    if e.is_connect() {
                        carnelian_common::Error::Connection(format!(
                            "Failed to connect to Carnelian at {}. Is it running?",
                            url
                        ))
                    } else {
                        carnelian_common::Error::Connection(format!(
                            "Request to {} failed: {}",
                            url, e
                        ))
                    }
                })?;

            if resp.status() == reqwest::StatusCode::CREATED {
                let body: CreateTaskResponse = resp.json().await.map_err(|e| {
                    carnelian_common::Error::Config(format!("Failed to parse response: {}", e))
                })?;
                println!("\u{2713} Task created successfully");
                println!("   Task ID:     {}", body.task_id);
                println!("   State:       {}", body.state);
                println!("   Created At:  {}", body.created_at);
            } else {
                let status = resp.status();
                let body: serde_json::Value = resp.json().await.unwrap_or_default();
                let error_msg = body["error"].as_str().unwrap_or("unknown error");
                return Err(carnelian_common::Error::Config(format!(
                    "Failed to create task (HTTP {}): {}",
                    status, error_msg
                )));
            }

            Ok(())
        }
    }
}

/// Handle the `logs` command - stream events from a running instance
#[allow(clippy::too_many_lines, clippy::redundant_pub_crate)]
async fn handle_logs(
    url: &str,
    follow: bool,
    level_filter: Option<String>,
    event_type_filter: Option<String>,
) -> carnelian_common::Result<()> {
    // Parse level filter if provided
    let min_level = if let Some(ref level_str) = level_filter {
        Some(parse_event_level(level_str)?)
    } else {
        None
    };

    // Convert HTTP URL to WebSocket URL
    let ws_base = url
        .replace("http://", "ws://")
        .replace("https://", "wss://");
    let ws_url = format!("{}/v1/events/ws", ws_base.trim_end_matches('/'));

    println!(
        "🔥 Connecting to Carnelian at {}...",
        url.trim_end_matches('/')
    );

    // Establish WebSocket connection
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .map_err(|e| {
            carnelian_common::Error::Connection(format!(
                "Failed to connect to Carnelian at {}. Is it running?\n  Error: {}",
                url, e
            ))
        })?;

    println!("🔥 Connected — streaming events{}\n", {
        let mut filters = Vec::new();
        if let Some(ref l) = level_filter {
            filters.push(format!("level >= {}", l.to_uppercase()));
        }
        if let Some(ref t) = event_type_filter {
            filters.push(format!("type = {}", t));
        }
        if filters.is_empty() {
            String::new()
        } else {
            format!(" ({})", filters.join(", "))
        }
    });

    let (mut sender, mut receiver) = ws_stream.split();
    let mut event_count = 0usize;

    loop {
        tokio::select! {
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<EventEnvelope>(&text) {
                            Ok(event) => {
                                // Apply level filter
                                if let Some(ref min) = min_level {
                                    if (event.level as u8) > (*min as u8) {
                                        continue;
                                    }
                                }

                                // Apply event type filter (case-insensitive substring match)
                                if let Some(ref type_filter) = event_type_filter {
                                    let event_type_str = format!("{:?}", event.event_type);
                                    if !event_type_str
                                        .to_lowercase()
                                        .contains(&type_filter.to_lowercase())
                                    {
                                        continue;
                                    }
                                }

                                println!("{}", format_event(&event));
                                event_count += 1;

                                // If not following, exit after first event
                                if !follow {
                                    break;
                                }
                            }
                            Err(_) => {
                                // Not a valid EventEnvelope, print raw for debugging
                                eprintln!("  [raw] {}", text);
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if let Err(e) = sender.send(Message::Pong(data)).await {
                            eprintln!("Failed to send pong: {}", e);
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        println!("\n🔥 Server closed connection");
                        break;
                    }
                    Some(Err(e)) => {
                        return Err(carnelian_common::Error::Connection(format!(
                            "WebSocket error: {}",
                            e
                        )));
                    }
                    None => {
                        println!("\n🔥 Connection closed");
                        break;
                    }
                    _ => {}
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\n🔥 Disconnecting... ({} events received)", event_count);
                break;
            }
        }
    }

    Ok(())
}

/// Parse a string into an `EventLevel` (case-insensitive)
fn parse_event_level(s: &str) -> carnelian_common::Result<EventLevel> {
    match s.to_uppercase().as_str() {
        "ERROR" => Ok(EventLevel::Error),
        "WARN" => Ok(EventLevel::Warn),
        "INFO" => Ok(EventLevel::Info),
        "DEBUG" => Ok(EventLevel::Debug),
        "TRACE" => Ok(EventLevel::Trace),
        _ => Err(carnelian_common::Error::Config(format!(
            "Invalid log level '{}'. Valid levels: ERROR, WARN, INFO, DEBUG, TRACE",
            s
        ))),
    }
}

/// Format an `EventEnvelope` for terminal display with ANSI colors
fn format_event(event: &EventEnvelope) -> String {
    let ts = event.timestamp.format("%Y-%m-%d %H:%M:%S%.3f");

    let (level_str, color_code) = match event.level {
        EventLevel::Error => ("ERROR", "\x1b[31m"),
        EventLevel::Warn => ("WARN ", "\x1b[33m"),
        EventLevel::Info => ("INFO ", "\x1b[32m"),
        EventLevel::Debug => ("DEBUG", "\x1b[34m"),
        EventLevel::Trace => ("TRACE", "\x1b[90m"),
    };
    let reset = "\x1b[0m";

    let event_type = format!("{:?}", event.event_type);

    let mut meta_parts = Vec::new();
    if let Some(ref actor) = event.actor_id {
        meta_parts.push(format!("actor={}", actor));
    }
    if let Some(ref corr) = event.correlation_id {
        meta_parts.push(format!("correlation={}", corr));
    }
    let meta = if meta_parts.is_empty() {
        String::new()
    } else {
        format!(" {}", meta_parts.join(" "))
    };

    let payload = if event.payload.is_null() || event.payload == serde_json::json!({}) {
        String::new()
    } else {
        format!("\n  payload: {}", event.payload)
    };

    format!("{color_code}[{ts}] {level_str} {event_type}{meta}{payload}{reset}")
}

/// Handle the `init` command - Interactive setup wizard with Docker and hardware detection
async fn handle_init(
    _config_path: Option<PathBuf>,
    _log_level_override: Option<String>,
    _database_url_override: Option<String>,
    _non_interactive: bool,
    force: bool,
    _resume: bool,
    key_path: Option<PathBuf>,
) -> carnelian_common::Result<()> {
    use std::io::{Write, stdin, stdout};
    use sysinfo::{MemoryKind, RefreshKind, System};

    // Welcome banner
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║ 🔥 Carnelian OS Setup Wizard                              ║");
    println!(
        "║   Version {}                                           ║",
        carnelian_common::VERSION
    );
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Hardware detection with sysinfo
    println!("Detecting hardware...");
    let mut sys = System::new_with_specifics(RefreshKind::new().with_memory(MemoryKind::RAM));
    sys.refresh_all();

    let total_ram_gb = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    println!("  RAM: {:.1} GB", total_ram_gb);

    // Detect GPU VRAM (numeric parsing)
    let mut vram_gb: f64 = 0.0;

    // Try nvidia-smi first (returns MiB)
    if let Ok(output) = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(first_line) = stdout.lines().next() {
                if let Ok(vram_mib) = first_line.trim().parse::<f64>() {
                    vram_gb = vram_mib / 1024.0;
                }
            }
        }
    }

    // Fallback to rocm-smi for AMD GPUs
    if vram_gb == 0.0 {
        if let Ok(output) = std::process::Command::new("rocm-smi")
            .args(["--showmeminfo", "vram"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Parse lines like: "GPU[0] : VRAM used: 1234 MB, total: 16384 MB"
                for line in stdout.lines() {
                    if line.contains("total:") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        for (i, part) in parts.iter().enumerate() {
                            if *part == "total:" && i + 1 < parts.len() {
                                if let Ok(vram_mb) = parts[i + 1].parse::<f64>() {
                                    vram_gb = vram_mb / 1024.0;
                                    break;
                                }
                            }
                        }
                    }
                    if vram_gb > 0.0 {
                        break;
                    }
                }
            }
        }
    }

    if vram_gb > 0.0 {
        println!("  GPU VRAM: {:.1} GB", vram_gb);
    } else {
        println!("  GPU: Not detected (CPU inference only)");
    }
    println!();

    // Minimum hardware guard
    if total_ram_gb < 8.0 {
        return Err(carnelian_common::Error::ExitCode(
            2,
            "Hardware below minimum requirements (need ≥ 8 GB RAM)".to_string(),
        ));
    }

    // Prerequisite check - Docker via bollard
    print!("Checking Docker... ");
    stdout().flush().unwrap();

    let docker = match Docker::connect_with_local_defaults() {
        Ok(d) => {
            // Test connection
            match d.version().await {
                Ok(_) => {
                    println!("✓ OK");
                    Some(d)
                }
                Err(e) => {
                    println!("⚠ Docker connection failed: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            println!("⚠ Docker not available: {}", e);
            // Platform-specific Docker install instructions
            #[cfg(target_os = "windows")]
            {
                println!("  Please install Docker Desktop for Windows:");
                println!("    winget install Docker.DockerDesktop");
                println!("  Or visit: https://docs.docker.com/desktop/install/windows/");
            }
            #[cfg(target_os = "macos")]
            {
                println!("  Please install Docker Desktop for Mac:");
                println!("    brew install --cask docker");
                println!("  Or visit: https://docs.docker.com/desktop/install/mac/");
            }
            #[cfg(target_os = "linux")]
            {
                println!("  Please install Docker Engine:");
                println!("    curl -fsSL https://get.docker.com | sh");
                println!("  Or visit: https://docs.docker.com/engine/install/");
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                println!("  Please install Docker before continuing.");
                println!("  Visit: https://docs.docker.com/get-docker/");
            }
            return Err(carnelian_common::Error::ExitCode(
                1,
                "Docker not found".to_string(),
            ));
        }
    };

    // Smart profile suggestion based on hardware
    let suggested_profile = if total_ram_gb >= 48.0 && vram_gb >= 10.0 {
        "urim"
    } else if total_ram_gb >= 16.0 && vram_gb >= 6.0 {
        "thummim"
    } else {
        "custom"
    };

    println!();
    println!(
        "Suggested profile: {} (based on {:.1}GB RAM {:.1}GB VRAM)",
        suggested_profile,
        total_ram_gb,
        vram_gb
    );
    print!(
        "Select machine profile [urim/thummim/custom] (default: {}): ",
        suggested_profile
    );
    stdout().flush().unwrap();
    let mut profile = String::new();
    stdin().read_line(&mut profile).unwrap();
    let profile = profile.trim();
    let machine_profile = if profile.is_empty() {
        suggested_profile
    } else {
        profile
    };

    // Container setup if Docker is available
    let mut auto_setup_containers = false;
    if docker.is_some() {
        println!();
        print!("Auto-setup PostgreSQL and Ollama containers? [Y/n]: ");
        stdout().flush().unwrap();
        let mut setup = String::new();
        stdin().read_line(&mut setup).unwrap();
        auto_setup_containers = setup.trim().to_lowercase() != "n";
    }

    // Default ports and URLs
    let (postgres_port, ollama_port, http_port) = match machine_profile {
        "thummim" => (5432, 11434, 18789),
        "urim" => (5432, 11434, 18789),
        _ => (5432, 11434, 18789),
    };

    // Database URL
    print!(
        "Database URL [postgresql://carnelian:carnelian@localhost:{}/carnelian]: ",
        postgres_port
    );
    stdout().flush().unwrap();
    let mut db_url = String::new();
    stdin().read_line(&mut db_url).unwrap();
    let database_url = if db_url.trim().is_empty() {
        format!(
            "postgresql://carnelian:carnelian@localhost:{}/carnelian",
            postgres_port
        )
    } else {
        db_url.trim().to_string()
    };

    // Ollama URL
    print!("Ollama URL [http://localhost:{}]: ", ollama_port);
    stdout().flush().unwrap();
    let mut ollama_url = String::new();
    stdin().read_line(&mut ollama_url).unwrap();
    let ollama_url = if ollama_url.trim().is_empty() {
        format!("http://localhost:{}", ollama_port)
    } else {
        ollama_url.trim().to_string()
    };

    // HTTP port
    print!("HTTP port [{}]: ", http_port);
    stdout().flush().unwrap();
    let mut port_str = String::new();
    stdin().read_line(&mut port_str).unwrap();
    let http_port = if port_str.trim().is_empty() {
        http_port
    } else {
        port_str.trim().parse::<u16>().unwrap_or(http_port)
    };

    // Workspace paths
    print!("Workspace paths to scan [.] (comma-separated): ");
    stdout().flush().unwrap();
    let mut paths = String::new();
    stdin().read_line(&mut paths).unwrap();
    let workspace_paths = if paths.trim().is_empty() {
        vec![".".to_string()]
    } else {
        paths
            .trim()
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    };

    // Owner keypair
    let home_dir = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let default_keypair_path = PathBuf::from(&home_dir)
        .join(".carnelian")
        .join("owner.key");

    // Track the actual key path for machine.toml
    let mut actual_keypair_path: Option<PathBuf> = None;

    // Handle --key-path flag: skip interactive prompt and use provided path
    if let Some(ref provided_key_path) = key_path {
        if !provided_key_path.exists() {
            return Err(carnelian_common::Error::Config(format!(
                "Key file not found: {}",
                provided_key_path.display()
            )));
        }
        println!("✓ Using key from --key-path: {}", provided_key_path.display());
        actual_keypair_path = Some(provided_key_path.clone());
    } else {
        // Interactive keypair selection
        println!();
        print!("Generate new owner keypair? [Y/n]: ");
        stdout().flush().unwrap();
        let mut gen_key = String::new();
        stdin().read_line(&mut gen_key).unwrap();
        let gen_key = gen_key.trim().to_lowercase();

        if gen_key.is_empty() || gen_key == "y" {
            // Generate keypair
            let (public_key, private_key_bytes) = carnelian_core::crypto::generate_ed25519_keypair();

            // Create parent directories
            if let Some(parent) = default_keypair_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    carnelian_common::Error::Config(format!("Failed to create key directory: {}", e))
                })?;
            }

            // Write private key
            std::fs::write(&default_keypair_path, &private_key_bytes).map_err(|e| {
                carnelian_common::Error::Config(format!("Failed to write key file: {}", e))
            })?;

            // Set permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let permissions = std::fs::Permissions::from_mode(0o600);
                std::fs::set_permissions(&default_keypair_path, permissions).map_err(|e| {
                    carnelian_common::Error::Config(format!("Failed to set key permissions: {}", e))
                })?;
            }

            println!("✓ Generated new owner keypair");
            println!("  Public key (hex): {}", hex::encode(public_key.as_bytes()));
            println!("  Private key file: {}", default_keypair_path.display());

            actual_keypair_path = Some(default_keypair_path);
        } else {
            print!("Path to existing key file (or press Enter to skip): ");
            stdout().flush().unwrap();
            let mut key_path_input = String::new();
            stdin().read_line(&mut key_path_input).unwrap();
            let key_path_input = key_path_input.trim();
            if !key_path_input.is_empty() {
                let path = std::path::Path::new(key_path_input);
                if !path.exists() {
                    return Err(carnelian_common::Error::Config(format!(
                        "Key file not found: {}",
                        key_path_input
                    )));
                }
                println!("✓ Using existing key: {}", key_path_input);
                actual_keypair_path = Some(PathBuf::from(key_path_input));
            } else {
                println!("  (No key configured)");
            }
        }
    }

    // Write machine.toml
    let machine_toml_path = PathBuf::from("machine.toml");
    if machine_toml_path.exists() && !force {
        print!("\nmachine.toml already exists. Overwrite? [y/N]: ");
        stdout().flush().unwrap();
        let mut overwrite = String::new();
        stdin().read_line(&mut overwrite).unwrap();
        if overwrite.trim().to_lowercase() != "y" {
            println!("Skipped writing machine.toml");
        } else {
            write_machine_toml(
                &machine_toml_path,
                machine_profile,
                &database_url,
                &ollama_url,
                http_port,
                &workspace_paths,
                actual_keypair_path.as_ref(),
            )?;
        }
    } else {
        write_machine_toml(
            &machine_toml_path,
            machine_profile,
            &database_url,
            &ollama_url,
            http_port,
            &workspace_paths,
            actual_keypair_path.as_ref(),
        )?;
    }

    // Docker container setup
    if auto_setup_containers {
        println!();
        println!("Setting up Docker containers...");
        if let Some(ref docker) = docker {
            // Pull PostgreSQL image
            println!("  Pulling PostgreSQL image...");
            let pg_options = CreateImageOptions {
                from_image: "postgres",
                tag: "16-alpine",
                ..Default::default()
            };
            let mut pg_stream = docker.create_image(Some(pg_options), None, None);
            while let Some(progress) = pg_stream.try_next().await.ok().flatten() {
                if let Some(status) = progress.status {
                    println!("    {}", status);
                }
            }

            // Pull Ollama image
            println!("  Pulling Ollama image...");
            let ollama_options = CreateImageOptions {
                from_image: "ollama/ollama",
                tag: "latest",
                ..Default::default()
            };
            let mut ollama_stream = docker.create_image(Some(ollama_options), None, None);
            while let Some(progress) = ollama_stream.try_next().await.ok().flatten() {
                if let Some(status) = progress.status {
                    println!("    {}", status);
                }
            }

            println!("✓ Images pulled successfully");

            // Create and start PostgreSQL container
            println!("  Creating PostgreSQL container...");
            let pg_config = ContainerConfig {
                image: Some("postgres:16-alpine".to_string()),
                env: Some(vec![
                    "POSTGRES_USER=carnelian".to_string(),
                    "POSTGRES_PASSWORD=carnelian".to_string(),
                    "POSTGRES_DB=carnelian".to_string(),
                ]),
                host_config: Some(HostConfig {
                    port_bindings: Some({
                        let mut bindings = std::collections::HashMap::new();
                        bindings.insert(
                            "5432/tcp".to_string(),
                            Some(vec![PortBinding {
                                host_ip: Some("0.0.0.0".to_string()),
                                host_port: Some(format!("{}", postgres_port)),
                            }]),
                        );
                        bindings
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            };

            match docker
                .create_container(
                    Some(CreateContainerOptions {
                        name: "carnelian-postgres",
                        platform: None,
                    }),
                    pg_config,
                )
                .await
            {
                Ok(_) => {
                    println!("    ✓ PostgreSQL container created");
                    match docker
                        .start_container("carnelian-postgres", None::<StartContainerOptions>)
                        .await
                    {
                        Ok(_) => println!(
                            "    ✓ PostgreSQL container started on port {}",
                            postgres_port
                        ),
                        Err(e) => println!("    ⚠ Failed to start PostgreSQL container: {}", e),
                    }
                }
                Err(e) => println!("    ⚠ Failed to create PostgreSQL container: {}", e),
            }

            // Create and start Ollama container
            println!("  Creating Ollama container...");
            let ollama_config = ContainerConfig {
                image: Some("ollama/ollama:latest".to_string()),
                host_config: Some(HostConfig {
                    port_bindings: Some({
                        let mut bindings = std::collections::HashMap::new();
                        bindings.insert(
                            "11434/tcp".to_string(),
                            Some(vec![PortBinding {
                                host_ip: Some("0.0.0.0".to_string()),
                                host_port: Some(format!("{}", ollama_port)),
                            }]),
                        );
                        bindings
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            };

            match docker
                .create_container(
                    Some(CreateContainerOptions {
                        name: "carnelian-ollama",
                        platform: None,
                    }),
                    ollama_config,
                )
                .await
            {
                Ok(_) => {
                    println!("    ✓ Ollama container created");
                    match docker
                        .start_container("carnelian-ollama", None::<StartContainerOptions>)
                        .await
                    {
                        Ok(_) => println!("    ✓ Ollama container started on port {}", ollama_port),
                        Err(e) => println!("    ⚠ Failed to start Ollama container: {}", e),
                    }
                }
                Err(e) => println!("    ⚠ Failed to create Ollama container: {}", e),
            }

            // Wait for PostgreSQL to be ready
            println!("  Waiting for PostgreSQL to be ready...");
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            println!("    ✓ Containers should be ready");
        }
    }

    // Run migrations
    println!();
    print!("Run database migrations now? [Y/n]: ");
    stdout().flush().unwrap();
    let mut run_migs = String::new();
    stdin().read_line(&mut run_migs).unwrap();
    if run_migs.trim().to_lowercase() != "n" {
        // Create minimal config for migrations
        let mut config = Config::default();
        config.database_url = database_url.clone();

        println!("Connecting to database...");
        if let Err(e) = config.connect_database().await {
            println!("⚠ Failed to connect to database: {}", e);
            if auto_setup_containers {
                println!("  Make sure containers are started:");
                println!("    docker start carnelian-postgres");
            } else {
                println!("  Make sure PostgreSQL is running");
            }
        } else {
            println!("Running migrations...");
            if let Ok(pool) = config.pool() {
                match carnelian_core::db::run_migrations(pool, None).await {
                    Ok(_) => println!("✓ Migrations completed"),
                    Err(e) => {
                        return Err(carnelian_common::Error::ExitCode(
                            3,
                            format!("Migration failed: {}", e),
                        ));
                    }
                }
            }
        }
    }

    // Docker-compose up option (alternative to direct container management)
    if docker.is_some() && !auto_setup_containers {
        println!();
        print!("Start services with docker-compose? [Y/n]: ");
        stdout().flush().unwrap();
        let mut compose_up = String::new();
        stdin().read_line(&mut compose_up).unwrap();
        if compose_up.trim().to_lowercase() != "n" {
            println!("Starting services with docker-compose...");
            match std::process::Command::new("docker-compose")
                .args(["-f", "docker-compose.yml", "up", "-d", "carnelian-postgres", "carnelian-ollama"])
                .status()
            {
                Ok(status) if status.success() => {
                    println!("✓ Services started with docker-compose");
                }
                Ok(_) => {
                    println!("⚠ docker-compose up failed - you may need to run it manually");
                }
                Err(e) => {
                    println!("⚠ Failed to run docker-compose: {}", e);
                }
            }
        }
    }

    // Starter skills activation
    println!();
    println!("Available starter skills:");
    println!("  - file-analyzer: Analyze files and extract metadata");
    println!("  - code-review: Review code for quality and issues");
    println!("  - model-usage: Track and optimize AI model usage");
    print!("Activate starter skills? [Y/n]: ");
    stdout().flush().unwrap();
    let mut activate_skills = String::new();
    stdin().read_line(&mut activate_skills).unwrap();
    if activate_skills.trim().to_lowercase() != "n" {
        let starter_skills = vec!["file-analyzer", "code-review", "model-usage"];
        let skill_book_path = PathBuf::from("skills/node-registry");
        let registry_path = PathBuf::from("skills/registry");
        
        // Create registry directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&registry_path) {
            println!("⚠ Failed to create registry directory: {}", e);
        } else {
            for skill_id in starter_skills {
                let skill_src = skill_book_path.join(skill_id);
                let skill_dst = registry_path.join(skill_id);
                
                if skill_src.exists() {
                    match std::fs::create_dir_all(&skill_dst) {
                        Ok(_) => {
                            // Copy skill files
                            if let Ok(entries) = std::fs::read_dir(&skill_src) {
                                for entry in entries.flatten() {
                                    let src_path = entry.path();
                                    let dst_path = skill_dst.join(entry.file_name());
                                    if src_path.is_file() {
                                        let _ = std::fs::copy(&src_path, &dst_path);
                                    }
                                }
                            }
                            println!("  ✓ Activated {}", skill_id);
                        }
                        Err(e) => {
                            println!("  ⚠ Failed to activate {}: {}", skill_id, e);
                        }
                    }
                } else {
                    println!("  ℹ Skill {} not found in node-registry (skipped)", skill_id);
                }
            }
        }
    }

    // Success summary
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║ ✓ Carnelian OS initialized!                             ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Next steps:");
    println!("  1. Start Carnelian:   carnelian start");
    println!("  2. Launch UI:         carnelian ui");
    println!("  3. Check status:      carnelian status");
    println!();
    println!(
        "Configuration file: {}",
        machine_toml_path
            .canonicalize()
            .unwrap_or(machine_toml_path)
            .display()
    );

    Ok(())
}

/// Helper to write machine.toml
fn write_machine_toml(
    path: &PathBuf,
    profile: &str,
    database_url: &str,
    ollama_url: &str,
    http_port: u16,
    workspace_paths: &[String],
    owner_keypair_path: Option<&PathBuf>,
) -> carnelian_common::Result<()> {
    let workspace_paths_str = workspace_paths
        .iter()
        .map(|p| format!("\"{}\"", p))
        .collect::<Vec<_>>()
        .join(", ");

    let keypair_line = if let Some(key_path) = owner_keypair_path {
        format!("owner_keypair_path = \"{}\"\n", key_path.display())
    } else {
        String::new()
    };

    let content = format!(
        r#"# 🔥 Carnelian OS Machine Configuration
# Generated by carnelian init

machine_profile = "{}"
http_port = {}

# Database
database_url = "{}"
db_max_connections = 10
db_connection_timeout_secs = 30

# Ollama
ollama_url = "{}"

# Logging
log_level = "INFO"

# Workspace scanning
max_tasks_per_heartbeat = 5
workspace_scan_paths = [{}]

# Owner keypair (generated by init)
{}"#,
        profile, http_port, database_url, ollama_url, workspace_paths_str, keypair_line
    );

    std::fs::write(path, content).map_err(|e| {
        carnelian_common::Error::Config(format!("Failed to write machine.toml: {}", e))
    })?;

    println!("✓ Wrote {}", path.display());
    Ok(())
}

/// Handle the `keygen` command - Generate owner keypair
async fn handle_keygen(output: Option<PathBuf>) -> carnelian_common::Result<()> {
    use std::io::Write;

    // Generate keypair
    let (public_key, private_key_bytes) = carnelian_core::crypto::generate_ed25519_keypair();

    // Determine output path
    let output_path = output.unwrap_or_else(|| {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".carnelian").join("owner.key")
    });

    // Create parent directories
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            carnelian_common::Error::Config(format!("Failed to create directory: {}", e))
        })?;
    }

    // Write private key
    std::fs::write(&output_path, &private_key_bytes)
        .map_err(|e| carnelian_common::Error::Config(format!("Failed to write key file: {}", e)))?;

    // Set permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&output_path, permissions).map_err(|e| {
            carnelian_common::Error::Config(format!("Failed to set permissions: {}", e))
        })?;
    }

    println!("🔥 Generated Ed25519 keypair");
    println!(
        "   Public key (hex): {}",
        hex::encode(public_key.as_bytes())
    );
    println!(
        "   Private key file: {}",
        output_path
            .canonicalize()
            .unwrap_or(output_path.clone())
            .display()
    );
    println!();
    println!("Add to machine.toml:");
    println!("   owner_keypair_path = \"{}\"", output_path.display());

    Ok(())
}

/// Handle the `key` command - Key management
async fn handle_key(
    command: KeyCommands,
    config_path: Option<PathBuf>,
    log_level_override: Option<String>,
    database_url_override: Option<String>,
) -> carnelian_common::Result<()> {
    match command {
        KeyCommands::Rotate => {
            handle_key_rotate(config_path, log_level_override, database_url_override).await
        }
    }
}

/// Handle the `key rotate` command - Rotate owner keypair
async fn handle_key_rotate(
    config_path: Option<PathBuf>,
    log_level_override: Option<String>,
    database_url_override: Option<String>,
) -> carnelian_common::Result<()> {
    use std::io::Write;

    // Load configuration
    let mut config = if let Some(path) = config_path {
        Config::load_from_file(&path)?
    } else {
        Config::load_from_file(std::path::Path::new("machine.toml")).unwrap_or_default()
    };

    config.apply_env_overrides()?;

    if let Some(level) = log_level_override {
        config.log_level = level.to_uppercase();
    }

    if let Some(url) = database_url_override {
        config.database_url = url;
    }

    carnelian_core::init_tracing(&config.log_level)?;

    tracing::info!("🔥 Key rotation starting...");

    // Connect to database
    config.connect_database().await?;

    // Load existing keypair (file first, then DB - mirrors main startup)
    if let Err(e) = config.load_owner_keypair() {
        tracing::debug!("Failed to load keypair from file: {}", e);
    }
    config.load_owner_keypair_from_db().await?;

    // Load owner keypair into a local variable first to avoid temporary lifetime issues
    let owner_keypair_opt = config.owner_signing_key();
    let old_keypair = owner_keypair_opt
        .as_ref()
        .ok_or_else(|| carnelian_common::Error::Security("Owner keypair not loaded".to_string()))?;
    let old_public_key = old_keypair.verifying_key();
    let old_public_key_hex = hex::encode(old_public_key.as_bytes());

    // Generate new keypair
    let (new_public_key, new_private_key_bytes) =
        carnelian_core::crypto::generate_ed25519_keypair();
    let new_public_key_hex = hex::encode(new_public_key.as_bytes());

    // Build rotation message
    let timestamp = chrono::Utc::now().timestamp();
    let rotation_message = format!("key_rotation:{}:{}", timestamp, new_public_key_hex);

    // Sign with old key
    let signature = config.sign_message(rotation_message.as_bytes())?;

    // Store new keypair in config_store as base64-encoded JSON
    let pool = config.pool()?.clone();
    let new_value = serde_json::json!({
        "seed_base64": base64::encode(&new_private_key_bytes)
    });

    Config::update_config_value(
        &pool,
        "owner_keypair",
        None, // old_value (we don't track the previous value in this context)
        &new_value,
        None,              // requested_by (no specific requester for CLI rotation)
        None,              // ledger (no ledger in CLI context)
        Some(old_keypair), // owner_signing_key for signing the ledger entry
        None,              // approval_queue (direct write, no approval needed for CLI)
    )
    .await?;

    // Determine key file path
    let keypair_path = config.owner_keypair_path.clone().unwrap_or_else(|| {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".carnelian").join("owner.key.new")
    });

    // Write new key file
    std::fs::write(&keypair_path, &new_private_key_bytes).map_err(|e| {
        carnelian_common::Error::Config(format!("Failed to write new key file: {}", e))
    })?;

    // Set permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&keypair_path, permissions).map_err(|e| {
            carnelian_common::Error::Config(format!("Failed to set permissions: {}", e))
        })?;
    }

    println!("🔥 Key rotation completed");
    println!("   Old public key: {}", old_public_key_hex);
    println!("   New public key: {}", new_public_key_hex);
    println!(
        "   Rotation signature: {}",
        hex::encode(signature.to_bytes())
    );
    println!("   New key file: {}", keypair_path.display());
    println!();
    println!("The new keypair has been stored in the database and written to disk.");

    Ok(())
}

/// Handle the `ui` command - Launch desktop UI or serve web UI
async fn handle_ui(web: bool) -> carnelian_common::Result<()> {
    if web {
        // Serve web UI
        println!("🔥 Starting Carnelian Web UI server...");

        // Determine the web UI directory
        let web_dir = std::path::PathBuf::from("target/dx/carnelian-ui/release/web/public");

        if !web_dir.exists() {
            println!("⚠ Web UI directory not found: {}", web_dir.display());
            println!("  Building web UI...");

            // Attempt to build the web UI using dx
            let build_result = std::process::Command::new("dx")
                .args([
                    "build",
                    "--release",
                    "-p",
                    "carnelian-ui",
                    "--platform",
                    "web",
                ])
                .current_dir(".")
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status();

            match build_result {
                Ok(status) if status.success() => {
                    println!("✓ Web UI built successfully");
                }
                Ok(_) => {
                    println!("⚠ Failed to build web UI. Make sure Dioxus CLI (dx) is installed.");
                    println!("  Install with: cargo install dioxus-cli");
                    return Ok(());
                }
                Err(e) => {
                    println!("⚠ Failed to run dx command: {}", e);
                    println!("  Make sure Dioxus CLI (dx) is installed: cargo install dioxus-cli");
                    return Ok(());
                }
            }
        }

        // Serve the web UI
        let port = std::env::var("CARNELIAN_WEB_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(8080);

        let addr = format!("0.0.0.0:{}", port);

        println!("  Serving web UI from: {}", web_dir.display());
        println!("  Web UI available at: http://{}", addr);
        println!("  Press Ctrl+C to stop the server");

        // Use a simple static file server
        let serve_result = serve_web_ui(&web_dir, port).await;

        if let Err(e) = serve_result {
            println!("⚠ Failed to serve web UI: {}", e);
        }

        return Ok(());
    }

    // Desktop UI launch
    let ui_binary = if let Ok(exe_path) = std::env::current_exe() {
        let same_dir = exe_path
            .parent()
            .map(|p| p.join("carnelian-ui"))
            .unwrap_or_else(|| PathBuf::from("carnelian-ui"));

        #[cfg(windows)]
        let same_dir = same_dir.with_extension("exe");

        if same_dir.exists() {
            same_dir
        } else {
            // Fall back to PATH lookup
            PathBuf::from("carnelian-ui")
        }
    } else {
        PathBuf::from("carnelian-ui")
    };

    if !ui_binary.exists() && !which::which(&ui_binary).is_ok() {
        return Err(carnelian_common::Error::Config(
            "carnelian-ui binary not found. Build it with: cargo build --release -p carnelian-ui"
                .to_string(),
        ));
    }

    // Spawn the UI (detached)
    let mut cmd = std::process::Command::new(&ui_binary);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                // Detach from parent process group
                libc::setsid();
                Ok(())
            });
        }
    }

    let child = cmd
        .spawn()
        .map_err(|e| carnelian_common::Error::Config(format!("Failed to launch UI: {}", e)))?;

    println!("🔥 Carnelian UI launched (PID: {})", child.id());

    Ok(())
}

/// Serve web UI static files
async fn serve_web_ui(web_dir: &std::path::Path, port: u16) -> carnelian_common::Result<()> {
    use axum::{
        Router,
        extract::Path,
        http::StatusCode,
        response::{Html, IntoResponse},
        routing::get,
    };
    use tokio::net::TcpListener;
    use tower_http::services::ServeDir;

    // Create router with static file serving
    let app = Router::new()
        .nest_service("/", ServeDir::new(web_dir))
        .fallback(|| async { (StatusCode::NOT_FOUND, "404 - Not Found") });

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await.map_err(|e| {
        carnelian_common::Error::Connection(format!("Failed to bind to {}: {}", addr, e))
    })?;

    println!("Web UI server listening on http://{}", addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| carnelian_common::Error::Connection(format!("Server error: {}", e)))?;

    Ok(())
}

/// Handle the `migrate-from-thummim` command - Migrate from Thummim project
async fn handle_migrate_from_thummim(
    path: Option<PathBuf>,
    config_path: Option<PathBuf>,
    log_level_override: Option<String>,
    database_url_override: Option<String>,
) -> carnelian_common::Result<()> {
    use std::io::Write;

    // Get Thummim path
    let thummim_path = if let Some(p) = path {
        p
    } else {
        print!("Path to Thummim project root: ");
        std::io::stdout().flush().unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let p = input.trim();
        if p.is_empty() {
            return Err(carnelian_common::Error::Config(
                "Thummim project path is required".to_string(),
            ));
        }
        PathBuf::from(p)
    };

    // Validate path
    if !thummim_path.exists() {
        return Err(carnelian_common::Error::Config(format!(
            "Path does not exist: {}",
            thummim_path.display()
        )));
    }

    let skills_dir = thummim_path.join("skills");
    if !skills_dir.exists() {
        return Err(carnelian_common::Error::Config(format!(
            "No skills/ directory found in {}",
            thummim_path.display()
        )));
    }

    // Load Carnelian config
    let mut config = if let Some(path) = config_path {
        Config::load_from_file(&path)?
    } else {
        Config::load_from_file(std::path::Path::new("machine.toml")).unwrap_or_default()
    };

    config.apply_env_overrides()?;

    if let Some(level) = log_level_override {
        config.log_level = level.to_uppercase();
    }

    if let Some(url) = database_url_override {
        config.database_url = url;
    }

    carnelian_core::init_tracing(&config.log_level)?;

    // Connect to database
    config.connect_database().await?;
    let pool = config.pool()?.clone();

    // Run migrations
    carnelian_core::db::run_migrations(&pool, None).await?;

    println!("🔥 Migrating from Thummim: {}", thummim_path.display());
    println!();

    // Track migration stats
    let mut skills_migrated = 0u32;
    let mut skills_skipped = 0u32;
    let mut skills_errored = 0u32;
    let mut tasks_imported = 0u32;
    let mut tasks_skipped = 0u32;

    // Migrate skills - walk skills/ directory
    let registry_path = config.skills_registry_path.clone();
    std::fs::create_dir_all(&registry_path).map_err(|e| {
        carnelian_common::Error::Config(format!("Failed to create registry: {}", e))
    })?;

    for entry in walkdir::WalkDir::new(&skills_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.file_name() == Some(std::ffi::OsStr::new("SKILL.md")) {
            let skill_dir = path.parent().unwrap();
            let skill_name = skill_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            // Read SKILL.md
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("  ⚠ Failed to read {}: {}", path.display(), e);
                    skills_errored += 1;
                    continue;
                }
            };

            // Parse YAML frontmatter (between --- delimiters)
            let mut name = skill_name.to_string();
            let mut description = format!("Migrated skill from Thummim: {}", skill_name);
            let mut runtime = "node";

            if let Some(frontmatter_start) = content.find("---") {
                if let Some(frontmatter_end) = content[frontmatter_start + 3..].find("---") {
                    let frontmatter =
                        &content[frontmatter_start + 3..frontmatter_start + 3 + frontmatter_end];

                    // Simple parsing - look for key: value patterns
                    for line in frontmatter.lines() {
                        if let Some((key, value)) = line.split_once(':') {
                            let key = key.trim();
                            let value = value.trim().trim_matches('\"').trim_matches('\'');
                            match key {
                                "name" => name = value.to_string(),
                                "description" => description = value.to_string(),
                                "runtime" => runtime = value,
                                _ => {}
                            }
                        }
                    }
                }
            }

            // Create Carnelian skill directory
            let carnelian_skill_dir = registry_path.join(&name);
            if carnelian_skill_dir.exists() {
                skills_skipped += 1;
                continue;
            }

            if let Err(e) = std::fs::create_dir(&carnelian_skill_dir) {
                eprintln!("  ⚠ Failed to create skill dir: {}", e);
                skills_errored += 1;
                continue;
            }

            // Write skill.json
            let skill_json = format!(
                r#"{{
  "name": "{}",
  "description": "{}",
  "runtime": "{}",
  "version": "1.0.0",
  "capabilities_required": [],
  "sandbox": {{
    "network": "disabled",
    "max_memory_mb": 128
  }}
}}"#,
                name,
                description.replace('"', "\\\""),
                runtime
            );

            if let Err(e) = std::fs::write(carnelian_skill_dir.join("skill.json"), skill_json) {
                eprintln!("  ⚠ Failed to write skill.json: {}", e);
                skills_errored += 1;
                continue;
            }

            // Copy SKILL.md
            if let Err(e) = std::fs::copy(path, carnelian_skill_dir.join("SKILL.md")) {
                eprintln!("  ⚠ Failed to copy SKILL.md: {}", e);
            }

            skills_migrated += 1;
        }
    }

    // Migrate tasks - read .agent/task-queue.json
    let task_queue_path = thummim_path.join(".agent").join("task-queue.json");
    if task_queue_path.exists() {
        let task_content = std::fs::read_to_string(&task_queue_path).map_err(|e| {
            carnelian_common::Error::Config(format!("Failed to read task-queue.json: {}", e))
        })?;

        let tasks: serde_json::Value = serde_json::from_str(&task_content).map_err(|e| {
            carnelian_common::Error::Config(format!("Failed to parse task-queue.json: {}", e))
        })?;

        if let Some(task_list) = tasks.as_array() {
            for task in task_list {
                let status = task["status"].as_str().unwrap_or("pending");

                // Only migrate pending and in_progress
                if status != "pending" && status != "in_progress" {
                    tasks_skipped += 1;
                    continue;
                }

                let description = task["description"].as_str().unwrap_or("");
                let priority_str = task["priority"].as_str().unwrap_or("medium");
                let priority = match priority_str {
                    "high" => 10,
                    "medium" => 5,
                    "low" => 1,
                    _ => 5,
                };

                // Insert into database
                let title = if description.len() > 255 {
                    &description[..255]
                } else {
                    description
                };

                let result = sqlx::query(
                    "INSERT INTO tasks (title, description, priority, state, created_at, updated_at) 
                     VALUES ($1, $2, $3, $4, NOW(), NOW()) 
                     RETURNING task_id"
                )
                .bind(title)
                .bind(description)
                .bind(priority)
                .bind("pending")
                .fetch_one(&pool)
                .await;

                match result {
                    Ok(_) => tasks_imported += 1,
                    Err(e) => {
                        eprintln!("  ⚠ Failed to import task: {}", e);
                    }
                }
            }
        }
    }

    // Refresh skills
    let discovery = carnelian_core::SkillDiscovery::new(
        pool.clone(),
        None,
        config.skills_registry_path.clone(),
    );
    let _ = discovery.refresh().await;

    // Print summary
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║ Migration Summary                                         ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!(
        "║ Skills migrated:  {:>3}                                    ║",
        skills_migrated
    );
    println!(
        "║ Skills skipped:   {:>3}  (already exist)                   ║",
        skills_skipped
    );
    println!(
        "║ Skills errored:   {:>3}                                    ║",
        skills_errored
    );
    println!(
        "║ Tasks imported:   {:>3}                                    ║",
        tasks_imported
    );
    println!(
        "║ Tasks skipped:    {:>3}  (completed)                       ║",
        tasks_skipped
    );
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Next: carnelian skills refresh");

    Ok(())
}
