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

use carnelian_common::types::{EventEnvelope, EventLevel};
use clap::{Parser, Subcommand};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio_tungstenite::tungstenite::protocol::Message;

use carnelian_core::{Config, EventStream, Ledger, PolicyEngine, Scheduler, Server, WorkerManager};

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
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

/// Resolve the server URL from an explicit value, CARNELIAN_HTTP_PORT env var, or default.
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
        carnelian_core::db::run_migrations(pool).await?;
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

    // Create scheduler with heartbeat interval from config
    let scheduler = Scheduler::new(
        config.pool()?.clone(),
        event_stream.clone(),
        Duration::from_millis(config.heartbeat_interval_ms),
    );

    // Create audit ledger and verify chain integrity
    let ledger = Ledger::new(config.pool()?.clone());
    ledger.load_last_hash().await?;

    match ledger.verify_chain().await {
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

    // Create server
    let server = Server::new(
        config_arc,
        event_stream,
        Arc::new(policy_engine),
        ledger,
        Arc::new(tokio::sync::Mutex::new(scheduler)),
        worker_manager,
    );

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

    let (workers, models, queue_depth) = match status_result {
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
            )
        }
        _ => (0, vec![], 0),
    };

    // Display status
    println!("🔥 Carnelian Status");
    println!("   Version:     {}", carnelian_common::VERSION);
    println!("   Status:      {}", status);
    println!("   Database:    {}", database);
    println!("   Workers:     {} active", workers);
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
        carnelian_core::db::run_migrations(pool).await?;

        println!("✓ Migrations completed successfully");
    }

    Ok(())
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
