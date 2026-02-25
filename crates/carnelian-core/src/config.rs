//! Configuration management for 🔥 Carnelian OS
//!
//! This module provides a comprehensive configuration system with three-tier loading:
//! 1. Base configuration from `machine.toml` with machine profile settings
//! 2. Owner Ed25519 keypair from secure filesystem storage or database
//! 3. Environment variable overrides for runtime flexibility
//!
//! # Configuration Loading Sequence
//!
//! ```text
//! machine.toml → Environment Variables → Validation → Keypair Loading → Database Connection
//! ```
//!
//! # Example
//!
//! ```ignore
//! use carnelian_core::config::Config;
//!
//! // Load configuration with full initialization
//! let config = Config::load().await?;
//!
//! // Or load step by step
//! let mut config = Config::load_from_file(Path::new("machine.toml"))?;
//! config.apply_env_overrides()?;
//! config.validate()?;
//! config.load_owner_keypair()?;
//! config.connect_database().await?;
//! ```
//!
//! # Security Considerations
//!
//! - Owner keypairs should be stored with filesystem permissions `0600`
//! - `SigningKey` is never serialized and only kept in memory during runtime
//! - Sensitive values should use environment variables, not committed config files

use carnelian_common::{Error, Result};
use config::Config as ConfigBuilder;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

/// Decode base64 string to bytes.
fn base64_decode(input: &str) -> Result<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    fn char_to_val(c: u8) -> Option<u8> {
        ALPHABET.iter().position(|&x| x == c).map(|p| p as u8)
    }

    let input = input.as_bytes();
    let mut output = Vec::with_capacity(input.len() * 3 / 4);

    let mut buf = 0u32;
    let mut bits = 0;

    for &byte in input {
        if byte == b'=' {
            break;
        }
        if let Some(val) = char_to_val(byte) {
            buf = (buf << 6) | u32::from(val);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                output.push((buf >> bits) as u8);
                buf &= (1 << bits) - 1;
            }
        }
    }

    Ok(output)
}

/// Application configuration with optional database pool and owner keypair
///
/// # Fields
///
/// Configuration is loaded from multiple sources in order of precedence:
/// 1. Default values (lowest precedence)
/// 2. `machine.toml` configuration file
/// 3. Environment variables (highest precedence)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// PostgreSQL connection URL
    #[serde(default = "default_database_url")]
    pub database_url: String,

    /// HTTP API bind address (default: "0.0.0.0")
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    /// HTTP API port (default: 18789)
    ///
    /// This port serves both HTTP REST API and WebSocket connections.
    /// WebSocket endpoint: ws://host:port/v1/events/ws
    #[serde(default = "default_http_port")]
    pub http_port: u16,

    /// URL of the LLM Gateway service (default: "http://localhost:18790")
    #[serde(default = "default_gateway_url")]
    pub gateway_url: String,

    /// Ollama API endpoint URL
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,

    /// Machine profile determining resource limits
    #[serde(default)]
    pub machine_profile: MachineProfile,

    /// Logging level (ERROR, WARN, INFO, DEBUG, TRACE)
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Path to Ed25519 owner keypair file
    #[serde(default)]
    pub owner_keypair_path: Option<PathBuf>,

    /// Agent/display name for the AI assistant (default: "Lian")
    #[serde(default = "default_agent_name")]
    pub agent_name: String,

    /// Hex-encoded Ed25519 public key (32 bytes)
    #[serde(skip)]
    pub owner_public_key: Option<String>,

    /// Maximum database connections (default: 10)
    #[serde(default = "default_max_connections")]
    pub db_max_connections: u32,

    /// Database connection timeout in seconds (default: 30)
    #[serde(default = "default_connection_timeout")]
    pub db_connection_timeout_secs: u64,

    /// Database idle timeout in seconds (default: 600 = 10 minutes)
    #[serde(default = "default_idle_timeout")]
    pub db_idle_timeout_secs: u64,

    /// Custom machine configuration (only used when machine_profile is Custom)
    #[serde(default)]
    pub custom_machine_config: Option<MachineConfig>,

    /// Event buffer capacity (default: 10,000)
    #[serde(default = "default_event_buffer_capacity")]
    pub event_buffer_capacity: usize,

    /// Maximum event payload size in bytes (default: 65,536 = 64KB)
    #[serde(default = "default_event_max_payload_bytes")]
    pub event_max_payload_bytes: usize,

    /// Broadcast channel capacity for event distribution (default: 100)
    #[serde(default = "default_event_broadcast_capacity")]
    pub event_broadcast_capacity: usize,

    /// Heartbeat interval in milliseconds (default: 555,555ms ≈ 9.26 minutes)
    #[serde(default = "default_heartbeat_interval_ms")]
    pub heartbeat_interval_ms: u64,

    /// Skill execution timeout in seconds (default: 300 = 5 minutes)
    #[serde(default = "default_skill_timeout_secs")]
    pub skill_timeout_secs: u64,

    /// Grace period after SIGTERM before SIGKILL in seconds (default: 5)
    #[serde(default = "default_skill_timeout_grace_period_secs")]
    pub skill_timeout_grace_period_secs: u64,

    /// Maximum skill output size in bytes (default: 1,048,576 = 1MB)
    #[serde(default = "default_skill_max_output_bytes")]
    pub skill_max_output_bytes: usize,

    /// Maximum number of log lines per skill execution (default: 10,000)
    #[serde(default = "default_skill_max_log_lines")]
    pub skill_max_log_lines: usize,

    /// Path to skills registry directory (default: ./skills/registry)
    #[serde(default = "default_skills_registry_path")]
    pub skills_registry_path: PathBuf,

    /// Path to soul files directory (default: ./souls)
    #[serde(default = "default_souls_path")]
    pub souls_path: PathBuf,

    /// Default session expiry in hours (default: 24, 0 = never expires)
    #[serde(default = "default_session_expiry_hours")]
    pub session_default_expiry_hours: u32,

    /// Optional path for file-backed session transcripts (default: None)
    #[serde(default)]
    pub session_transcripts_path: Option<PathBuf>,

    /// Enable automatic file-backed transcript writing (default: false)
    #[serde(default)]
    pub session_enable_file_backup: bool,

    /// Default context window tokens (default: 32000)
    #[serde(default = "default_context_window_tokens")]
    pub context_window_tokens: usize,

    /// Context budget reserve percentage (default: 10)
    #[serde(default = "default_context_reserve_percent")]
    pub context_reserve_percent: u8,

    /// Tool result soft-trim threshold tokens (default: 2000)
    #[serde(default = "default_tool_trim_threshold")]
    pub tool_trim_threshold: usize,

    /// Tool result hard-clear age seconds (default: 3600)
    #[serde(default = "default_tool_clear_age_secs")]
    pub tool_clear_age_secs: i64,

    /// Maximum retry attempts for failed tasks (default: 3)
    #[serde(default = "default_task_max_retry_attempts")]
    pub task_max_retry_attempts: u32,

    /// Delay in seconds between task retry attempts (default: 5)
    #[serde(default = "default_task_retry_delay_secs")]
    pub task_retry_delay_secs: u64,

    /// Maximum tasks auto-queued per heartbeat from workspace scanning (default: 5)
    #[serde(default = "default_max_tasks_per_heartbeat")]
    pub max_tasks_per_heartbeat: usize,

    /// Workspace paths to scan for TASK:/TODO: markers during heartbeat (default: ["."])
    #[serde(default = "default_workspace_scan_paths")]
    pub workspace_scan_paths: Vec<PathBuf>,

    /// Whether the Telegram channel adapter is enabled (default: false)
    #[serde(default)]
    pub adapter_telegram_enabled: bool,

    /// Whether the Discord channel adapter is enabled (default: false)
    #[serde(default)]
    pub adapter_discord_enabled: bool,

    /// Spam detection threshold for channel adapters (default: 0.8)
    #[serde(default = "default_adapter_spam_threshold")]
    pub adapter_spam_threshold: f32,

    /// Allowed CORS origins for HTTP API and Web UI access.
    /// Default: localhost dev origins only. Set to allow remote browser access.
    #[serde(default = "default_cors_origins")]
    pub cors_origins: Vec<String>,

    /// Database pool (not serialized, initialized separately)
    #[serde(skip)]
    db_pool: Option<Arc<PgPool>>,

    /// Ed25519 signing key (not serialized, loaded separately)
    #[serde(skip)]
    owner_signing_key: Option<SigningKey>,
}

/// Machine-specific configuration settings
///
/// These settings are determined by the `MachineProfile` or can be
/// customized when using `MachineProfile::Custom`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineConfig {
    /// Maximum concurrent workers
    pub max_workers: u32,
    /// Memory limit in MB
    pub max_memory_mb: u64,
    /// Whether GPU is available
    pub gpu_enabled: bool,
    /// Default Ollama model
    pub default_model: String,
    /// Whether to automatically restart crashed workers
    #[serde(default = "default_auto_restart_workers")]
    pub auto_restart_workers: bool,
}

fn default_auto_restart_workers() -> bool {
    true
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            max_workers: 2,
            max_memory_mb: 8192,
            gpu_enabled: false,
            default_model: "deepseek-r1:7b".to_string(),
            auto_restart_workers: true,
        }
    }
}

fn default_database_url() -> String {
    "postgresql://carnelian:carnelian@localhost:5432/carnelian".to_string()
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}

fn default_http_port() -> u16 {
    18789
}

fn default_gateway_url() -> String {
    "http://localhost:18790".to_string()
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_log_level() -> String {
    "INFO".to_string()
}

fn default_max_connections() -> u32 {
    10
}

fn default_connection_timeout() -> u64 {
    30
}

fn default_idle_timeout() -> u64 {
    600
}

fn default_event_buffer_capacity() -> usize {
    10_000
}

fn default_event_max_payload_bytes() -> usize {
    65_536
}

fn default_event_broadcast_capacity() -> usize {
    100
}

fn default_heartbeat_interval_ms() -> u64 {
    555_555
}

fn default_skill_timeout_secs() -> u64 {
    300
}

fn default_skill_timeout_grace_period_secs() -> u64 {
    5
}

fn default_skill_max_output_bytes() -> usize {
    1_048_576
}

fn default_skill_max_log_lines() -> usize {
    10_000
}

fn default_task_max_retry_attempts() -> u32 {
    3
}

fn default_task_retry_delay_secs() -> u64 {
    5
}

fn default_max_tasks_per_heartbeat() -> usize {
    5
}

fn default_workspace_scan_paths() -> Vec<PathBuf> {
    vec![PathBuf::from(".")]
}

fn default_adapter_spam_threshold() -> f32 {
    0.8
}

fn default_cors_origins() -> Vec<String> {
    vec![
        "http://localhost:3000".to_string(),
        "http://localhost:5173".to_string(),
        "http://127.0.0.1:3000".to_string(),
        "http://127.0.0.1:5173".to_string(),
    ]
}

fn default_skills_registry_path() -> PathBuf {
    PathBuf::from("./skills/registry")
}

fn default_souls_path() -> PathBuf {
    PathBuf::from("./souls")
}

fn default_agent_name() -> String {
    "Lian".to_string()
}

fn default_session_expiry_hours() -> u32 {
    24
}

fn default_context_window_tokens() -> usize {
    32_000
}

fn default_context_reserve_percent() -> u8 {
    10
}

fn default_tool_trim_threshold() -> usize {
    2000
}

fn default_tool_clear_age_secs() -> i64 {
    3600
}

/// Machine profile determining resource limits and default model
///
/// # Variants
///
/// - `Standard`: Entry-level profile (8GB VRAM, 32GB RAM) - good for most tasks
/// - `Performance`: High-end profile (11GB+ VRAM, 64GB RAM) - for demanding workloads
/// - `Custom`: User-defined settings via `custom_machine_config`
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MachineProfile {
    #[default]
    Standard,
    Performance,
    Custom,
}

impl FromStr for MachineProfile {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "standard" | "thummim" => Ok(Self::Standard), // thummim for backward compatibility
            "performance" | "urim" => Ok(Self::Performance), // urim for backward compatibility
            "custom" => Ok(Self::Custom),
            _ => Err(Error::Config(format!(
                "Invalid machine profile '{}'. Valid values: standard, performance, custom",
                s
            ))),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: default_database_url(),
            bind_address: default_bind_address(),
            http_port: default_http_port(),
            gateway_url: default_gateway_url(),
            ollama_url: default_ollama_url(),
            machine_profile: MachineProfile::default(),
            log_level: default_log_level(),
            owner_keypair_path: None,
            agent_name: default_agent_name(),
            owner_public_key: None,
            db_max_connections: default_max_connections(),
            db_connection_timeout_secs: default_connection_timeout(),
            db_idle_timeout_secs: default_idle_timeout(),
            custom_machine_config: None,
            event_buffer_capacity: default_event_buffer_capacity(),
            event_max_payload_bytes: default_event_max_payload_bytes(),
            event_broadcast_capacity: default_event_broadcast_capacity(),
            heartbeat_interval_ms: default_heartbeat_interval_ms(),
            skill_timeout_secs: default_skill_timeout_secs(),
            skill_timeout_grace_period_secs: default_skill_timeout_grace_period_secs(),
            skill_max_output_bytes: default_skill_max_output_bytes(),
            skill_max_log_lines: default_skill_max_log_lines(),
            skills_registry_path: default_skills_registry_path(),
            souls_path: default_souls_path(),
            session_default_expiry_hours: default_session_expiry_hours(),
            session_transcripts_path: None,
            session_enable_file_backup: false,
            context_window_tokens: default_context_window_tokens(),
            context_reserve_percent: default_context_reserve_percent(),
            tool_trim_threshold: default_tool_trim_threshold(),
            tool_clear_age_secs: default_tool_clear_age_secs(),
            task_max_retry_attempts: default_task_max_retry_attempts(),
            task_retry_delay_secs: default_task_retry_delay_secs(),
            max_tasks_per_heartbeat: default_max_tasks_per_heartbeat(),
            workspace_scan_paths: default_workspace_scan_paths(),
            adapter_telegram_enabled: false,
            adapter_discord_enabled: false,
            adapter_spam_threshold: default_adapter_spam_threshold(),
            cors_origins: default_cors_origins(),
            db_pool: None,
            owner_signing_key: None,
        }
    }
}

impl Config {
    /// Load configuration with full initialization sequence.
    ///
    /// This method orchestrates the complete loading sequence:
    /// 1. Load base configuration from `machine.toml` (or use defaults)
    /// 2. Apply environment variable overrides
    /// 3. Validate configuration
    /// 4. Load owner keypair (if configured)
    /// 5. Connect to database
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Configuration validation fails
    /// - Database connection fails
    /// - Keypair loading fails (if configured)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = Config::load().await?;
    /// ```
    pub async fn load() -> Result<Self> {
        let mut config = Self::load_from_file(Path::new("machine.toml")).unwrap_or_else(|e| {
            // Can't log yet - tracing not initialized
            eprintln!("Failed to load machine.toml: {}. Using defaults.", e);
            Self::default()
        });

        config.apply_env_overrides()?;

        // Initialize tracing early so all subsequent operations can log
        crate::init_tracing(&config.log_level)?;

        tracing::info!(config_file = "machine.toml", "Configuration loaded");
        tracing::debug!("Applied environment variable overrides");

        config.validate()?;
        tracing::debug!("Configuration validated successfully");

        config.load_owner_keypair()?;
        config.connect_database().await?;
        config.load_owner_keypair_from_db().await?;

        tracing::info!(
            http_port = config.http_port,
            machine_profile = ?config.machine_profile,
            "Configuration ready"
        );

        Ok(config)
    }

    /// Load configuration from a TOML file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the TOML configuration file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = Config::load_from_file(Path::new("machine.toml"))?;
    /// ```
    pub fn load_from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(Error::Config(format!(
                "Configuration file not found: {}",
                path.display()
            )));
        }

        let config = ConfigBuilder::builder()
            .add_source(config::File::from(path))
            .build()
            .map_err(|e| Error::Config(format!("Failed to load config: {}", e)))?;

        config
            .try_deserialize::<Self>()
            .map_err(|e| Error::Config(format!("Failed to parse config: {}", e)))
    }

    /// Apply environment variable overrides to the configuration.
    ///
    /// Environment variables take precedence over file-based configuration.
    /// This method checks for and applies the following variables:
    ///
    /// - `DATABASE_URL` → `database_url`
    /// - `PORT` or `CARNELIAN_HTTP_PORT` → `http_port`
    /// - `LOG_LEVEL` → `log_level` (simple levels only: ERROR, WARN, INFO, DEBUG, TRACE)
    /// - `MACHINE_PROFILE` → `machine_profile`
    /// - `CARNELIAN_OLLAMA_URL` → `ollama_url`
    ///
    /// Note: `RUST_LOG` is NOT processed here. It is read directly by
    /// `tracing_subscriber::EnvFilter` to support module-level filtering
    /// (e.g., `carnelian_core=debug,sqlx=warn`).
    ///
    /// # Errors
    ///
    /// Returns an error if an environment variable has an invalid value.
    #[allow(clippy::too_many_lines)]
    pub fn apply_env_overrides(&mut self) -> Result<()> {
        dotenvy::dotenv().ok();

        if let Ok(url) = std::env::var("DATABASE_URL") {
            self.database_url = url;
        }

        if let Ok(port) = std::env::var("PORT") {
            self.http_port = port
                .parse()
                .map_err(|_| Error::Config(format!("Invalid PORT value: {}", port)))?;
        } else if let Ok(port) = std::env::var("CARNELIAN_HTTP_PORT") {
            self.http_port = port.parse().map_err(|_| {
                Error::Config(format!("Invalid CARNELIAN_HTTP_PORT value: {}", port))
            })?;
        }

        // Only use LOG_LEVEL for simple log level override
        // RUST_LOG is left for tracing_subscriber::EnvFilter to handle directly
        if let Ok(level) = std::env::var("LOG_LEVEL") {
            self.log_level = level.to_uppercase();
        }

        if let Ok(profile) = std::env::var("MACHINE_PROFILE") {
            self.machine_profile = MachineProfile::from_str(&profile)?;
        }

        if let Ok(url) = std::env::var("CARNELIAN_OLLAMA_URL") {
            self.ollama_url = url;
        }

        if let Ok(url) = std::env::var("CARNELIAN_GATEWAY_URL") {
            self.gateway_url = url;
        }

        if let Ok(path) = std::env::var("CARNELIAN_OWNER_KEYPAIR_PATH") {
            self.owner_keypair_path = Some(PathBuf::from(path));
        }

        if let Ok(interval) = std::env::var("CARNELIAN_HEARTBEAT_INTERVAL_MS") {
            self.heartbeat_interval_ms = interval.parse().map_err(|_| {
                Error::Config(format!(
                    "Invalid CARNELIAN_HEARTBEAT_INTERVAL_MS value: {}",
                    interval
                ))
            })?;
        }

        if let Ok(val) = std::env::var("CARNELIAN_SKILL_TIMEOUT_SECS") {
            self.skill_timeout_secs = val.parse().map_err(|_| {
                Error::Config(format!(
                    "Invalid CARNELIAN_SKILL_TIMEOUT_SECS value: {}",
                    val
                ))
            })?;
        }

        if let Ok(val) = std::env::var("CARNELIAN_SKILL_TIMEOUT_GRACE_PERIOD_SECS") {
            self.skill_timeout_grace_period_secs = val.parse().map_err(|_| {
                Error::Config(format!(
                    "Invalid CARNELIAN_SKILL_TIMEOUT_GRACE_PERIOD_SECS value: {}",
                    val
                ))
            })?;
        }

        if let Ok(val) = std::env::var("CARNELIAN_SKILL_MAX_OUTPUT_BYTES") {
            self.skill_max_output_bytes = val.parse().map_err(|_| {
                Error::Config(format!(
                    "Invalid CARNELIAN_SKILL_MAX_OUTPUT_BYTES value: {}",
                    val
                ))
            })?;
        }

        if let Ok(val) = std::env::var("CARNELIAN_SKILL_MAX_LOG_LINES") {
            self.skill_max_log_lines = val.parse().map_err(|_| {
                Error::Config(format!(
                    "Invalid CARNELIAN_SKILL_MAX_LOG_LINES value: {}",
                    val
                ))
            })?;
        }

        if let Ok(val) = std::env::var("CARNELIAN_TASK_MAX_RETRY_ATTEMPTS") {
            self.task_max_retry_attempts = val.parse().map_err(|_| {
                Error::Config(format!(
                    "Invalid CARNELIAN_TASK_MAX_RETRY_ATTEMPTS value: {}",
                    val
                ))
            })?;
        }

        if let Ok(val) = std::env::var("CARNELIAN_TASK_RETRY_DELAY_SECS") {
            self.task_retry_delay_secs = val.parse().map_err(|_| {
                Error::Config(format!(
                    "Invalid CARNELIAN_TASK_RETRY_DELAY_SECS value: {}",
                    val
                ))
            })?;
        }

        if let Ok(val) = std::env::var("CARNELIAN_MAX_TASKS_PER_HEARTBEAT") {
            self.max_tasks_per_heartbeat = val.parse().map_err(|_| {
                Error::Config(format!(
                    "Invalid CARNELIAN_MAX_TASKS_PER_HEARTBEAT value: {}",
                    val
                ))
            })?;
        }

        if let Ok(val) = std::env::var("CARNELIAN_WORKSPACE_SCAN_PATHS") {
            self.workspace_scan_paths = val.split(',').map(|s| PathBuf::from(s.trim())).collect();
        }

        // CARNELIAN_SOULS_PATH — override path to soul files directory
        if let Ok(path) = std::env::var("CARNELIAN_SOULS_PATH") {
            self.souls_path = PathBuf::from(path);
        }

        // CARNELIAN_AGENT_NAME — custom agent/assistant display name
        if let Ok(name) = std::env::var("CARNELIAN_AGENT_NAME") {
            self.agent_name = name;
        }

        // CARNELIAN_CORS_ORIGINS — comma-separated list of allowed CORS origins
        if let Ok(val) = std::env::var("CARNELIAN_CORS_ORIGINS") {
            self.cors_origins = val.split(',').map(|s| s.trim().to_string()).collect();
        }

        // SESSION_EXPIRY_HOURS — default session TTL in hours (0 = never)
        if let Ok(val) = std::env::var("SESSION_EXPIRY_HOURS") {
            self.session_default_expiry_hours = val.parse().map_err(|_| {
                Error::Config(format!("Invalid SESSION_EXPIRY_HOURS value: {}", val))
            })?;
        }

        // SESSION_TRANSCRIPTS_PATH — directory for file-backed session transcripts
        if let Ok(path) = std::env::var("SESSION_TRANSCRIPTS_PATH") {
            self.session_transcripts_path = Some(PathBuf::from(path));
        }

        // CARNELIAN_CONTEXT_WINDOW_TOKENS — default context window size
        if let Ok(val) = std::env::var("CARNELIAN_CONTEXT_WINDOW_TOKENS") {
            self.context_window_tokens = val.parse().map_err(|_| {
                Error::Config(format!(
                    "Invalid CARNELIAN_CONTEXT_WINDOW_TOKENS value: {}",
                    val
                ))
            })?;
        }

        // CARNELIAN_CONTEXT_RESERVE_PERCENT — budget reserve percentage
        if let Ok(val) = std::env::var("CARNELIAN_CONTEXT_RESERVE_PERCENT") {
            self.context_reserve_percent = val.parse().map_err(|_| {
                Error::Config(format!(
                    "Invalid CARNELIAN_CONTEXT_RESERVE_PERCENT value: {}",
                    val
                ))
            })?;
        }

        // CARNELIAN_TOOL_TRIM_THRESHOLD — soft-trim threshold for tool results
        if let Ok(val) = std::env::var("CARNELIAN_TOOL_TRIM_THRESHOLD") {
            self.tool_trim_threshold = val.parse().map_err(|_| {
                Error::Config(format!(
                    "Invalid CARNELIAN_TOOL_TRIM_THRESHOLD value: {}",
                    val
                ))
            })?;
        }

        // CARNELIAN_TOOL_CLEAR_AGE_SECS — hard-clear age threshold for tool results
        if let Ok(val) = std::env::var("CARNELIAN_TOOL_CLEAR_AGE_SECS") {
            self.tool_clear_age_secs = val.parse().map_err(|_| {
                Error::Config(format!(
                    "Invalid CARNELIAN_TOOL_CLEAR_AGE_SECS value: {}",
                    val
                ))
            })?;
        }

        Ok(())
    }

    /// Validate configuration completeness and correctness.
    ///
    /// Checks that all required fields are present and have valid values:
    /// - Database URL is not empty and has valid PostgreSQL format
    /// - Ports are in valid range (1024-65535)
    /// - Log level is valid (ERROR, WARN, INFO, DEBUG, TRACE)
    /// - Ollama URL is not empty and has valid HTTP format
    /// - Database connection parameters are within reasonable bounds
    ///
    /// # Errors
    ///
    /// Returns `Error::Config` with a descriptive message for any validation failure.
    pub fn validate(&self) -> Result<()> {
        if self.database_url.is_empty() {
            return Err(Error::Config("database_url cannot be empty".to_string()));
        }

        if !self.database_url.starts_with("postgresql://")
            && !self.database_url.starts_with("postgres://")
        {
            return Err(Error::Config(
                "database_url must be a valid PostgreSQL URL".to_string(),
            ));
        }

        if self.http_port < 1024 {
            return Err(Error::Config(format!(
                "http_port must be >= 1024, got {}",
                self.http_port
            )));
        }

        // Validate log_level is a simple level (not a module filter)
        // RUST_LOG module filters are handled separately by EnvFilter
        let valid_log_levels = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];
        if !valid_log_levels.contains(&self.log_level.as_str()) {
            return Err(Error::Config(format!(
                "log_level must be one of {:?}, got '{}'. Use RUST_LOG for module-level filtering.",
                valid_log_levels, self.log_level
            )));
        }

        if self.ollama_url.is_empty() {
            return Err(Error::Config("ollama_url cannot be empty".to_string()));
        }

        if !self.ollama_url.starts_with("http://") && !self.ollama_url.starts_with("https://") {
            return Err(Error::Config(
                "ollama_url must be a valid HTTP URL".to_string(),
            ));
        }

        if self.db_max_connections == 0 || self.db_max_connections > 1000 {
            return Err(Error::Config(format!(
                "db_max_connections must be between 1 and 1000, got {}",
                self.db_max_connections
            )));
        }

        if self.db_connection_timeout_secs == 0 || self.db_connection_timeout_secs > 300 {
            return Err(Error::Config(format!(
                "db_connection_timeout_secs must be between 1 and 300, got {}",
                self.db_connection_timeout_secs
            )));
        }

        if self.db_idle_timeout_secs == 0 || self.db_idle_timeout_secs > 3600 {
            return Err(Error::Config(format!(
                "db_idle_timeout_secs must be between 1 and 3600, got {}",
                self.db_idle_timeout_secs
            )));
        }

        // Event stream validation
        if self.event_buffer_capacity < 100 || self.event_buffer_capacity > 1_000_000 {
            return Err(Error::Config(format!(
                "event_buffer_capacity must be between 100 and 1,000,000, got {}",
                self.event_buffer_capacity
            )));
        }

        if self.event_max_payload_bytes < 1024 || self.event_max_payload_bytes > 1_048_576 {
            return Err(Error::Config(format!(
                "event_max_payload_bytes must be between 1KB and 1MB, got {}",
                self.event_max_payload_bytes
            )));
        }

        if self.event_broadcast_capacity < 10 || self.event_broadcast_capacity > 10_000 {
            return Err(Error::Config(format!(
                "event_broadcast_capacity must be between 10 and 10,000, got {}",
                self.event_broadcast_capacity
            )));
        }

        // Context window / compaction validation
        if self.context_reserve_percent < 1 || self.context_reserve_percent > 50 {
            return Err(Error::Config(format!(
                "context_reserve_percent must be between 1 and 50, got {}",
                self.context_reserve_percent
            )));
        }

        if self.tool_trim_threshold == 0 || self.tool_trim_threshold >= self.context_window_tokens {
            return Err(Error::Config(format!(
                "tool_trim_threshold must be > 0 and < context_window_tokens ({}), got {}",
                self.context_window_tokens, self.tool_trim_threshold
            )));
        }

        if self.tool_clear_age_secs <= 0 {
            return Err(Error::Config(format!(
                "tool_clear_age_secs must be > 0, got {}",
                self.tool_clear_age_secs
            )));
        }

        // Workspace scanning validation
        if self.max_tasks_per_heartbeat > 100 {
            tracing::warn!(
                limit = self.max_tasks_per_heartbeat,
                "max_tasks_per_heartbeat is very high, may impact heartbeat performance"
            );
        }

        for path in &self.workspace_scan_paths {
            if !path.exists() {
                tracing::warn!(
                    path = %path.display(),
                    "Workspace scan path does not exist, will be skipped during scanning"
                );
            }
            if path.is_absolute() {
                tracing::warn!(
                    path = %path.display(),
                    "Absolute workspace scan paths are discouraged for security"
                );
            }
        }

        Ok(())
    }

    /// Load owner Ed25519 keypair from secure storage.
    ///
    /// Attempts to load the keypair from (in order):
    /// 1. Filesystem path specified in `owner_keypair_path`
    /// 2. Database `config_store` table (key: `owner_keypair`)
    ///
    /// Supports two key formats:
    /// - **PEM/PKCS8**: Generated by `openssl genpkey -algorithm ed25519`
    /// - **Raw 32-byte seed**: Direct binary seed file
    ///
    /// If neither source is configured, logs a warning and continues
    /// (keypair is optional for development).
    ///
    /// # Security
    ///
    /// - Keypair files should have permissions `0600` (read/write owner only)
    /// - The `SigningKey` is kept in memory only and never serialized
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Keypair file exists but cannot be read
    /// - Keypair data is invalid (wrong length or format)
    pub fn load_owner_keypair(&mut self) -> Result<()> {
        // Try loading from filesystem first
        if let Some(ref path) = self.owner_keypair_path {
            if path.exists() {
                tracing::info!("Loading owner keypair from: {}", path.display());
                let key_bytes = std::fs::read(path)?;

                if let Some(signing_key) = Self::parse_ed25519_key(&key_bytes)? {
                    let verifying_key = signing_key.verifying_key();
                    self.owner_public_key = Some(hex::encode(verifying_key.as_bytes()));
                    self.owner_signing_key = Some(signing_key);

                    tracing::info!(
                        "Owner keypair loaded from file. Public key: {}",
                        self.owner_public_key.as_ref().unwrap()
                    );
                    return Ok(());
                }
            } else {
                tracing::warn!(
                    "Owner keypair path configured but file not found: {}",
                    path.display()
                );
            }
        }

        // Database fallback is handled asynchronously in load_owner_keypair_from_db
        // which should be called after database connection is established
        tracing::warn!(
            "Owner keypair not loaded from file. Will attempt database fallback after connection."
        );
        Ok(())
    }

    /// Load owner keypair from database `config_store` table.
    ///
    /// This should be called after database connection is established.
    /// Looks for key `owner_keypair` in the `config_store` table.
    ///
    /// For non-encrypted keys, delegates to [`crypto::load_keypair_from_db`]
    /// which reads the raw 32-byte seed directly. Encrypted keys are decrypted
    /// in-place using `CARNELIAN_KEYPAIR_PASSPHRASE` before parsing.
    ///
    /// If no keypair is found in the database (and none was loaded from file),
    /// a new keypair is generated and persisted automatically so that the owner
    /// key is always available after first-run initialization.
    ///
    /// # Errors
    ///
    /// Returns an error if database query fails or keypair data is invalid.
    pub async fn load_owner_keypair_from_db(&mut self) -> Result<()> {
        if self.has_owner_keypair() {
            return Ok(()); // Already loaded from file
        }

        let pool = match self.pool() {
            Ok(p) => p.clone(),
            Err(_) => {
                tracing::warn!("Database not connected; cannot load or generate owner keypair.");
                return Ok(());
            }
        };

        // Check if the stored keypair is encrypted — if so, use the decrypt path
        let encrypted_flag: Option<bool> =
            sqlx::query_scalar("SELECT encrypted FROM config_store WHERE key = 'owner_keypair'")
                .fetch_optional(&pool)
                .await
                .map_err(Error::Database)?;

        match encrypted_flag {
            Some(true) => {
                // Encrypted path: fetch blob, decrypt, parse
                let (key_data,): (Vec<u8>,) = sqlx::query_as(
                    "SELECT value_blob FROM config_store WHERE key = 'owner_keypair'",
                )
                .fetch_one(&pool)
                .await
                .map_err(Error::Database)?;

                let key_bytes = Self::decrypt_keypair(&key_data)?;
                if let Some(signing_key) = Self::parse_ed25519_key(&key_bytes)? {
                    let public_hex = crate::crypto::public_key_from_signing_key(&signing_key);
                    tracing::info!(
                        public_key = %public_hex,
                        "Owner keypair loaded from database (encrypted)"
                    );
                    self.owner_public_key = Some(public_hex);
                    self.owner_signing_key = Some(signing_key);
                    return Ok(());
                }
            }
            Some(false) => {
                // Non-encrypted path: delegate to crypto module
                if let Some(signing_key) = crate::crypto::load_keypair_from_db(&pool).await? {
                    let public_hex = crate::crypto::public_key_from_signing_key(&signing_key);
                    tracing::info!(
                        public_key = %public_hex,
                        "Owner keypair loaded from database"
                    );
                    self.owner_public_key = Some(public_hex);
                    self.owner_signing_key = Some(signing_key);
                    return Ok(());
                }
            }
            None => {
                // No keypair row exists — generate and store on first run
                tracing::info!(
                    "No owner keypair found in file or database; generating new keypair on first run"
                );
                self.generate_and_store_owner_keypair(None).await?;
                return Ok(());
            }
        }

        tracing::warn!(
            "Owner keypair not configured. Some features requiring signed authority will be unavailable."
        );
        Ok(())
    }

    /// Parse Ed25519 key from either PEM/PKCS8 or raw 32-byte seed format.
    fn parse_ed25519_key(key_bytes: &[u8]) -> Result<Option<SigningKey>> {
        // Check if it's PEM format (starts with "-----BEGIN")
        if key_bytes.starts_with(b"-----BEGIN") {
            return Self::parse_pem_key(key_bytes);
        }

        // Try raw 32-byte seed
        if key_bytes.len() == 32 {
            let key_array: [u8; 32] = key_bytes
                .try_into()
                .map_err(|_| Error::Security("Failed to convert keypair bytes".to_string()))?;
            return Ok(Some(SigningKey::from_bytes(&key_array)));
        }

        // Try PKCS8 DER format (48 bytes for Ed25519)
        if key_bytes.len() == 48 {
            // PKCS8 Ed25519 private key: 16-byte header + 32-byte seed
            // The seed is at offset 16
            let seed = &key_bytes[16..48];
            let key_array: [u8; 32] = seed
                .try_into()
                .map_err(|_| Error::Security("Failed to extract seed from PKCS8".to_string()))?;
            return Ok(Some(SigningKey::from_bytes(&key_array)));
        }

        Err(Error::Security(format!(
            "Invalid keypair format: expected PEM, raw 32-byte seed, or PKCS8 DER (48 bytes), got {} bytes",
            key_bytes.len()
        )))
    }

    /// Parse PEM-encoded Ed25519 private key (PKCS8 format from OpenSSL).
    fn parse_pem_key(pem_bytes: &[u8]) -> Result<Option<SigningKey>> {
        let pem_str = std::str::from_utf8(pem_bytes)
            .map_err(|_| Error::Security("Invalid PEM: not valid UTF-8".to_string()))?;

        // Find the base64 content between BEGIN and END markers
        let start_marker = "-----BEGIN PRIVATE KEY-----";
        let end_marker = "-----END PRIVATE KEY-----";

        let start = pem_str
            .find(start_marker)
            .ok_or_else(|| Error::Security("Invalid PEM: missing BEGIN marker".to_string()))?
            + start_marker.len();

        let end = pem_str
            .find(end_marker)
            .ok_or_else(|| Error::Security("Invalid PEM: missing END marker".to_string()))?;

        // Extract and decode base64 content
        let base64_content: String = pem_str[start..end]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();

        // Decode base64 to get PKCS8 DER
        let der_bytes = base64_decode(&base64_content)?;

        // PKCS8 Ed25519 private key structure:
        // SEQUENCE {
        //   INTEGER 0
        //   SEQUENCE { OID 1.3.101.112 }  -- Ed25519 OID
        //   OCTET STRING { OCTET STRING { 32-byte seed } }
        // }
        // The 32-byte seed is typically at a fixed offset in the DER structure
        // For Ed25519 PKCS8, the structure is 48 bytes total with seed at offset 16

        if der_bytes.len() < 48 {
            return Err(Error::Security(format!(
                "Invalid PKCS8 DER: expected at least 48 bytes, got {}",
                der_bytes.len()
            )));
        }

        // Extract the 32-byte seed from PKCS8 structure
        // The seed is wrapped in an OCTET STRING at the end
        // For standard Ed25519 PKCS8, seed starts at byte 16
        let seed_start = der_bytes.len() - 32;
        let seed = &der_bytes[seed_start..];

        let key_array: [u8; 32] = seed
            .try_into()
            .map_err(|_| Error::Security("Failed to extract seed from PEM".to_string()))?;

        Ok(Some(SigningKey::from_bytes(&key_array)))
    }

    /// Decrypt keypair data using passphrase from environment.
    fn decrypt_keypair(encrypted_data: &[u8]) -> Result<Vec<u8>> {
        let passphrase = std::env::var("CARNELIAN_KEYPAIR_PASSPHRASE").map_err(|_| {
            Error::Security(
                "Encrypted keypair found but CARNELIAN_KEYPAIR_PASSPHRASE not set".to_string(),
            )
        })?;

        // Derive key using blake3
        let key = blake3::derive_key("carnelian-keypair-encryption", passphrase.as_bytes());

        // Simple XOR decryption (for demonstration - production should use proper AEAD)
        // The encrypted data format: nonce (32 bytes) + ciphertext
        if encrypted_data.len() < 64 {
            return Err(Error::Security("Encrypted data too short".to_string()));
        }

        let nonce = &encrypted_data[..32];
        let ciphertext = &encrypted_data[32..];

        // Derive decryption key from passphrase key and nonce
        let decrypt_key = blake3::derive_key("carnelian-decrypt", &[&key[..], nonce].concat());

        // XOR decrypt
        let plaintext: Vec<u8> = ciphertext
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ decrypt_key[i % 32])
            .collect();

        Ok(plaintext)
    }

    /// Get the OpenAI API key from environment variable.
    #[must_use]
    pub fn openai_api_key(&self) -> Option<String> {
        std::env::var("OPENAI_API_KEY").ok()
    }

    /// Get the Anthropic API key from environment variable.
    #[must_use]
    pub fn anthropic_api_key(&self) -> Option<String> {
        std::env::var("ANTHROPIC_API_KEY").ok()
    }

    /// Get the Fireworks API key from environment variable.
    #[must_use]
    pub fn fireworks_api_key(&self) -> Option<String> {
        std::env::var("FIREWORKS_API_KEY").ok()
    }

    /// Check if owner keypair is loaded.
    #[must_use]
    pub fn has_owner_keypair(&self) -> bool {
        self.owner_signing_key.is_some()
    }

    /// Get a reference to the owner signing key, if loaded.
    #[must_use]
    pub fn owner_signing_key(&self) -> Option<&SigningKey> {
        self.owner_signing_key.as_ref()
    }

    /// Generate a new Ed25519 owner keypair, store it in the database, and
    /// update the in-memory config fields.
    ///
    /// This is intended for first-run initialization when no keypair exists.
    /// The keypair is stored unencrypted by default; set `CARNELIAN_KEYPAIR_PASSPHRASE`
    /// and use the encrypted path for production deployments.
    ///
    /// If a `ledger` is provided, a `"keypair.generated"` event is logged.
    ///
    /// # Errors
    ///
    /// Returns an error if the database pool is not connected or the store fails.
    pub async fn generate_and_store_owner_keypair(
        &mut self,
        ledger: Option<&crate::ledger::Ledger>,
    ) -> Result<()> {
        let pool = self.pool()?.clone();

        let (signing_key, _verifying_key) = crate::crypto::generate_ed25519_keypair();
        crate::crypto::store_keypair_in_db(&pool, &signing_key, false).await?;

        let public_hex = crate::crypto::public_key_from_signing_key(&signing_key);
        tracing::info!(
            public_key = %public_hex,
            "Generated and stored new owner keypair"
        );

        self.owner_public_key = Some(public_hex);
        self.owner_signing_key = Some(signing_key);

        // Log keypair generation to ledger (not a privileged action itself,
        // but useful for audit trail)
        if let Some(ledger) = ledger {
            if let Err(e) = ledger
                .append_event(
                    None,
                    "keypair.generated",
                    serde_json::json!({
                        "public_key": self.owner_public_key,
                    }),
                    None,
                    None,
                    None,
                )
                .await
            {
                tracing::warn!(error = %e, "Failed to log keypair generation to ledger");
            }
        }

        Ok(())
    }

    /// Sign a message with the owner's Ed25519 key.
    ///
    /// # Arguments
    ///
    /// * `message` - The message bytes to sign
    ///
    /// # Errors
    ///
    /// Returns an error if the owner keypair is not loaded.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let signature = config.sign_message(b"Hello, world!")?;
    /// ```
    pub fn sign_message(&self, message: &[u8]) -> Result<Signature> {
        let signing_key = self
            .owner_signing_key
            .as_ref()
            .ok_or_else(|| Error::Security("Owner keypair not loaded".to_string()))?;

        Ok(signing_key.sign(message))
    }

    /// Verify a signature against the owner's public key.
    ///
    /// # Arguments
    ///
    /// * `message` - The original message bytes
    /// * `signature` - The signature to verify
    ///
    /// # Errors
    ///
    /// Returns an error if the owner keypair is not loaded.
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the signature is valid, `Ok(false)` otherwise.
    pub fn verify_signature(&self, message: &[u8], signature: &Signature) -> Result<bool> {
        let signing_key = self
            .owner_signing_key
            .as_ref()
            .ok_or_else(|| Error::Security("Owner keypair not loaded".to_string()))?;

        let verifying_key = signing_key.verifying_key();
        Ok(verifying_key.verify(message, signature).is_ok())
    }

    /// Get machine-specific configuration based on the current profile.
    ///
    /// Returns `MachineConfig` with settings appropriate for the machine profile:
    ///
    /// | Profile | Max Workers | Max Memory | GPU | Default Model |
    /// |---------|-------------|------------|-----|---------------|
    /// | Standard | 4 | 28GB | Yes | deepseek-r1:7b |
    /// | Performance | 8 | 56GB | Yes | deepseek-r1:32b |
    /// | Custom | 2 | 8GB | No | deepseek-r1:7b |
    #[must_use]
    pub fn machine_config(&self) -> MachineConfig {
        match self.machine_profile {
            MachineProfile::Standard => MachineConfig {
                max_workers: 4,
                max_memory_mb: 28672, // 28GB, leaving 4GB for system
                gpu_enabled: true,
                default_model: "deepseek-r1:7b".to_string(),
                auto_restart_workers: true,
            },
            MachineProfile::Performance => MachineConfig {
                max_workers: 8,
                max_memory_mb: 57344, // 56GB, leaving 8GB for system
                gpu_enabled: true,
                default_model: "deepseek-r1:32b".to_string(),
                auto_restart_workers: true,
            },
            MachineProfile::Custom => self.custom_machine_config.clone().unwrap_or_default(),
        }
    }

    /// Connect to the database and initialize the connection pool.
    ///
    /// # Errors
    /// Returns an error if the connection cannot be established.
    pub async fn connect_database(&mut self) -> Result<()> {
        tracing::info!("Connecting to database...");

        let pool = PgPoolOptions::new()
            .max_connections(self.db_max_connections)
            .acquire_timeout(Duration::from_secs(self.db_connection_timeout_secs))
            .idle_timeout(Duration::from_secs(self.db_idle_timeout_secs))
            .connect(&self.database_url)
            .await?;

        tracing::info!("Database connection pool established");
        self.db_pool = Some(Arc::new(pool));
        Ok(())
    }

    /// Get a reference to the database pool.
    ///
    /// # Errors
    /// Returns an error if the database is not connected.
    pub fn pool(&self) -> Result<&PgPool> {
        self.db_pool
            .as_ref()
            .map(|p| p.as_ref())
            .ok_or_else(|| Error::Connection("Database not connected".to_string()))
    }

    /// Get an Arc reference to the database pool for sharing across tasks.
    ///
    /// # Errors
    /// Returns an error if the database is not connected.
    pub fn pool_arc(&self) -> Result<Arc<PgPool>> {
        self.db_pool
            .clone()
            .ok_or_else(|| Error::Connection("Database not connected".to_string()))
    }

    /// Check if the database pool is initialized.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.db_pool.is_some()
    }

    /// Read a config value from `config_store`, transparently decrypting if needed.
    ///
    /// Fetches `encrypted`, `value_blob`, `value_text`, and `key_version` for the
    /// given key. When `encrypted` is `true`, decrypts `value_blob` using the
    /// provided `EncryptionHelper` and parses the result as JSON. When `false`,
    /// parses `value_text` as JSON. Returns `Ok(None)` if the key does not exist.
    ///
    /// # Arguments
    ///
    /// * `pool` - Database connection pool
    /// * `config_key` - The config key to look up
    /// * `encryption_helper` - Optional helper for decrypting encrypted entries
    ///
    /// # Returns
    ///
    /// `(serde_json::Value, key_version)` on success, or `None` if the key is absent.
    pub async fn read_config_value(
        pool: &PgPool,
        config_key: &str,
        encryption_helper: Option<&crate::encryption::EncryptionHelper>,
    ) -> Result<Option<(serde_json::Value, i32)>> {
        let row: Option<(bool, Option<Vec<u8>>, Option<String>, i32)> = sqlx::query_as(
            "SELECT encrypted, value_blob, value_text, key_version FROM config_store WHERE key = $1",
        )
        .bind(config_key)
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)?;

        let Some((encrypted, value_blob, value_text, key_version)) = row else {
            return Ok(None);
        };

        let json_str = if encrypted {
            // Encrypted path: decrypt value_blob
            let blob = value_blob.ok_or_else(|| {
                Error::Config(format!(
                    "Config key '{}' is marked encrypted but value_blob is NULL",
                    config_key
                ))
            })?;
            let helper = encryption_helper.ok_or_else(|| {
                Error::Config(format!(
                    "Config key '{}' is encrypted but no EncryptionHelper provided",
                    config_key
                ))
            })?;
            helper.decrypt_text(&blob).await?
        } else {
            // Plaintext path: use value_text
            value_text.ok_or_else(|| {
                Error::Config(format!(
                    "Config key '{}' has encrypted=false but value_text is NULL",
                    config_key
                ))
            })?
        };

        let parsed: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            Error::Config(format!(
                "Config key '{}': failed to parse JSON: {}",
                config_key, e
            ))
        })?;

        Ok(Some((parsed, key_version)))
    }

    /// Update a value in the `config_store` table.
    ///
    /// When an `approval_queue` is provided, the change is queued for approval
    /// and `Error::ApprovalRequired` is returned with the approval ID. When
    /// `None`, the change is applied immediately and logged to the ledger.
    pub async fn update_config_value(
        pool: &PgPool,
        config_key: &str,
        old_value: Option<&serde_json::Value>,
        new_value: &serde_json::Value,
        requested_by: Option<uuid::Uuid>,
        ledger: Option<&crate::ledger::Ledger>,
        owner_signing_key: Option<&ed25519_dalek::SigningKey>,
        approval_queue: Option<&crate::approvals::ApprovalQueue>,
    ) -> Result<()> {
        Self::update_config_value_encrypted(
            pool,
            config_key,
            old_value,
            new_value,
            requested_by,
            ledger,
            owner_signing_key,
            approval_queue,
            None,
            1,
        )
        .await
    }

    /// Update a value in the `config_store` table with optional encryption.
    ///
    /// When `encryption_helper` is `Some`, the serialized value is encrypted
    /// and stored in `value_blob` with `encrypted=true` and the supplied
    /// `key_version`. When `None`, behaves identically to `update_config_value`.
    ///
    /// # Arguments
    ///
    /// * `key_version` - Version of the encryption key used. Callers performing
    ///   key rotation should increment this value. Defaults to `1` when called
    ///   via `update_config_value`.
    pub async fn update_config_value_encrypted(
        pool: &PgPool,
        config_key: &str,
        old_value: Option<&serde_json::Value>,
        new_value: &serde_json::Value,
        requested_by: Option<uuid::Uuid>,
        ledger: Option<&crate::ledger::Ledger>,
        owner_signing_key: Option<&ed25519_dalek::SigningKey>,
        approval_queue: Option<&crate::approvals::ApprovalQueue>,
        encryption_helper: Option<&crate::encryption::EncryptionHelper>,
        key_version: i32,
    ) -> Result<()> {
        if let Some(queue) = approval_queue {
            let correlation_id = Some(uuid::Uuid::now_v7());
            let approval_id = Self::queue_config_change(
                queue,
                config_key,
                old_value,
                new_value,
                requested_by,
                correlation_id,
            )
            .await?;
            return Err(Error::ApprovalRequired(approval_id));
        }

        // Direct write — no approval required
        let value_text = serde_json::to_string(new_value)
            .map_err(|e| Error::Config(format!("Failed to serialize config value: {}", e)))?;

        if let Some(helper) = encryption_helper {
            // Encrypted path: encrypt value_text into value_blob
            let encrypted_blob = helper.encrypt_text(&value_text).await?;
            sqlx::query(
                r"INSERT INTO config_store (key, value_text, value_blob, encrypted, key_version, updated_at)
                  VALUES ($1, NULL, $2, true, $3, NOW())
                  ON CONFLICT (key) DO UPDATE SET value_text = NULL, value_blob = $2, encrypted = true, key_version = $3, updated_at = NOW()",
            )
            .bind(config_key)
            .bind(&encrypted_blob)
            .bind(key_version)
            .execute(pool)
            .await
            .map_err(Error::Database)?;
        } else {
            // Plaintext path — still persist key_version for consistency
            sqlx::query(
                r"INSERT INTO config_store (key, value_text, key_version, updated_at)
                  VALUES ($1, $2, $3, NOW())
                  ON CONFLICT (key) DO UPDATE SET value_text = $2, key_version = $3, updated_at = NOW()",
            )
            .bind(config_key)
            .bind(&value_text)
            .bind(key_version)
            .execute(pool)
            .await
            .map_err(Error::Database)?;
        }

        tracing::info!(
            config_key = %config_key,
            encrypted = encryption_helper.is_some(),
            key_version = key_version,
            "Config value updated"
        );

        if let Some(ledger) = ledger {
            if let Err(e) = ledger
                .log_config_change(
                    config_key,
                    old_value,
                    new_value,
                    requested_by,
                    owner_signing_key,
                )
                .await
            {
                tracing::warn!(error = %e, "Failed to log config change to ledger");
            }
        }

        Ok(())
    }

    /// Queue a config change for approval before execution.
    ///
    /// Creates an approval request containing the config key, old value, and
    /// new value. Returns the approval ID for tracking.
    pub async fn queue_config_change(
        approval_queue: &crate::approvals::ApprovalQueue,
        config_key: &str,
        old_value: Option<&serde_json::Value>,
        new_value: &serde_json::Value,
        requested_by: Option<uuid::Uuid>,
        correlation_id: Option<uuid::Uuid>,
    ) -> Result<uuid::Uuid> {
        let payload = serde_json::json!({
            "config_key": config_key,
            "old_value": old_value,
            "new_value": new_value,
        });
        approval_queue
            .queue_action("config.change", payload, requested_by, correlation_id)
            .await
    }

    /// Execute a previously approved config change.
    ///
    /// Fetches the approval request, verifies it is approved, extracts the
    /// config key and new value, updates `config_store`, and logs to the ledger.
    pub async fn execute_approved_config_change(
        pool: &PgPool,
        approval_id: uuid::Uuid,
        approval_queue: &crate::approvals::ApprovalQueue,
        ledger: &crate::ledger::Ledger,
        owner_signing_key: Option<&ed25519_dalek::SigningKey>,
    ) -> Result<()> {
        let request = approval_queue.get(approval_id).await?.ok_or_else(|| {
            Error::Security(format!("Approval request not found: {}", approval_id))
        })?;

        if request.status != "approved" {
            return Err(Error::Security(format!(
                "Approval request {} is not approved (status: {})",
                approval_id, request.status
            )));
        }

        let payload = &request.payload;
        let config_key = payload["config_key"]
            .as_str()
            .ok_or_else(|| Error::Security("Missing config_key in approval payload".to_string()))?;
        let new_value = &payload["new_value"];

        // Serialize new_value to string for config_store
        let value_text = serde_json::to_string(new_value)
            .map_err(|e| Error::Config(format!("Failed to serialize config value: {}", e)))?;

        sqlx::query(
            r"INSERT INTO config_store (key, value_text, updated_at)
              VALUES ($1, $2, NOW())
              ON CONFLICT (key) DO UPDATE SET value_text = $2, updated_at = NOW()",
        )
        .bind(config_key)
        .bind(&value_text)
        .execute(pool)
        .await
        .map_err(Error::Database)?;

        tracing::info!(
            config_key = %config_key,
            approval_id = %approval_id,
            "Approved config change applied"
        );

        let old_value = payload
            .get("old_value")
            .and_then(|v| if v.is_null() { None } else { Some(v) });

        ledger
            .log_config_change(config_key, old_value, new_value, None, owner_signing_key)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.http_port, 18789);
        assert_eq!(config.log_level, "INFO");
        assert_eq!(config.machine_profile, MachineProfile::Standard);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_machine_profile_parsing() {
        assert_eq!(
            MachineProfile::from_str("standard").unwrap(),
            MachineProfile::Standard
        );
        assert_eq!(
            MachineProfile::from_str("STANDARD").unwrap(),
            MachineProfile::Standard
        );
        // Backward compatibility
        assert_eq!(
            MachineProfile::from_str("thummim").unwrap(),
            MachineProfile::Standard
        );
        assert_eq!(
            MachineProfile::from_str("performance").unwrap(),
            MachineProfile::Performance
        );
        // Backward compatibility
        assert_eq!(
            MachineProfile::from_str("urim").unwrap(),
            MachineProfile::Performance
        );
        assert_eq!(
            MachineProfile::from_str("custom").unwrap(),
            MachineProfile::Custom
        );
        assert!(MachineProfile::from_str("invalid").is_err());
    }

    #[test]
    fn test_machine_config_standard() {
        let config = Config {
            machine_profile: MachineProfile::Standard,
            ..Default::default()
        };
        let machine = config.machine_config();
        assert_eq!(machine.max_workers, 4);
        assert_eq!(machine.max_memory_mb, 28672);
        assert!(machine.gpu_enabled);
        assert_eq!(machine.default_model, "deepseek-r1:7b");
    }

    #[test]
    fn test_machine_config_performance() {
        let config = Config {
            machine_profile: MachineProfile::Performance,
            ..Default::default()
        };
        let machine = config.machine_config();
        assert_eq!(machine.max_workers, 8);
        assert_eq!(machine.max_memory_mb, 57344);
        assert!(machine.gpu_enabled);
        assert_eq!(machine.default_model, "deepseek-r1:32b");
    }

    #[test]
    fn test_machine_config_custom() {
        let custom = MachineConfig {
            max_workers: 16,
            max_memory_mb: 131072,
            gpu_enabled: true,
            default_model: "llama3:70b".to_string(),
            auto_restart_workers: true,
        };
        let config = Config {
            machine_profile: MachineProfile::Custom,
            custom_machine_config: Some(custom.clone()),
            ..Default::default()
        };
        let machine = config.machine_config();
        assert_eq!(machine.max_workers, 16);
        assert_eq!(machine.max_memory_mb, 131072);
    }

    #[test]
    fn test_validation_invalid_port() {
        let mut config = Config::default();
        config.http_port = 80; // Below 1024
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_invalid_url() {
        let mut config = Config::default();
        config.database_url = "invalid-url".to_string();
        assert!(config.validate().is_err());

        config.database_url = default_database_url();
        config.ollama_url = "not-a-url".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_invalid_log_level() {
        let mut config = Config::default();
        config.log_level = "VERBOSE".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_invalid_db_params() {
        let mut config = Config::default();
        config.db_max_connections = 0;
        assert!(config.validate().is_err());

        config.db_max_connections = 10;
        config.db_connection_timeout_secs = 500;
        assert!(config.validate().is_err());

        config.db_connection_timeout_secs = 30;
        config.db_idle_timeout_secs = 5000;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_has_owner_keypair() {
        let config = Config::default();
        assert!(!config.has_owner_keypair());
    }
}
