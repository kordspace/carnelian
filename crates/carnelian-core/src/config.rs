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
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
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

    /// HTTP API port (default: 18789)
    #[serde(default = "default_http_port")]
    pub http_port: u16,

    /// WebSocket port (default: 18790)
    #[serde(default = "default_ws_port")]
    pub ws_port: u16,

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
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            max_workers: 2,
            max_memory_mb: 8192,
            gpu_enabled: false,
            default_model: "deepseek-r1:7b".to_string(),
        }
    }
}

fn default_database_url() -> String {
    "postgresql://carnelian:carnelian@localhost:5432/carnelian".to_string()
}

fn default_http_port() -> u16 {
    18789
}

fn default_ws_port() -> u16 {
    18790
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

/// Machine profile determining resource limits and default model
///
/// # Variants
///
/// - `Thummim`: RTX 2080 Super (8GB VRAM), 32GB RAM - constrained profile
/// - `Urim`: RTX 2080 Ti (11GB VRAM), 64GB RAM - high-end profile
/// - `Custom`: User-defined settings via `custom_machine_config`
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum MachineProfile {
    #[default]
    Thummim,
    Urim,
    Custom,
}

impl FromStr for MachineProfile {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "thummim" => Ok(Self::Thummim),
            "urim" => Ok(Self::Urim),
            "custom" => Ok(Self::Custom),
            _ => Err(Error::Config(format!(
                "Invalid machine profile '{}'. Valid values: thummim, urim, custom",
                s
            ))),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: default_database_url(),
            http_port: default_http_port(),
            ws_port: default_ws_port(),
            ollama_url: default_ollama_url(),
            machine_profile: MachineProfile::default(),
            log_level: default_log_level(),
            owner_keypair_path: None,
            owner_public_key: None,
            db_max_connections: default_max_connections(),
            db_connection_timeout_secs: default_connection_timeout(),
            db_idle_timeout_secs: default_idle_timeout(),
            custom_machine_config: None,
            event_buffer_capacity: default_event_buffer_capacity(),
            event_max_payload_bytes: default_event_max_payload_bytes(),
            event_broadcast_capacity: default_event_broadcast_capacity(),
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

        tracing::info!(
            config_file = "machine.toml",
            "Configuration loaded"
        );
        tracing::debug!("Applied environment variable overrides");

        config.validate()?;
        tracing::debug!("Configuration validated successfully");

        config.load_owner_keypair()?;
        config.connect_database().await?;
        config.load_owner_keypair_from_db().await?;

        tracing::info!(
            http_port = config.http_port,
            ws_port = config.ws_port,
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
    /// - `CARNELIAN_WS_PORT` → `ws_port`
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
            self.http_port = port
                .parse()
                .map_err(|_| Error::Config(format!("Invalid CARNELIAN_HTTP_PORT value: {}", port)))?;
        }

        if let Ok(port) = std::env::var("CARNELIAN_WS_PORT") {
            self.ws_port = port
                .parse()
                .map_err(|_| Error::Config(format!("Invalid CARNELIAN_WS_PORT value: {}", port)))?;
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

        if let Ok(path) = std::env::var("CARNELIAN_OWNER_KEYPAIR_PATH") {
            self.owner_keypair_path = Some(PathBuf::from(path));
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

        if self.ws_port < 1024 {
            return Err(Error::Config(format!(
                "ws_port must be >= 1024, got {}",
                self.ws_port
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

    /// Load owner keypair from database config_store table.
    ///
    /// This should be called after database connection is established.
    /// Looks for key `owner_keypair` in the `config_store` table.
    ///
    /// # Errors
    ///
    /// Returns an error if database query fails or keypair data is invalid.
    pub async fn load_owner_keypair_from_db(&mut self) -> Result<()> {
        if self.has_owner_keypair() {
            return Ok(()); // Already loaded from file
        }

        let pool = match self.pool() {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!(
                    "Owner keypair not configured. Some features requiring signed authority will be unavailable."
                );
                return Ok(());
            }
        };

        // Query config_store for owner_keypair
        let row: Option<(Vec<u8>, bool)> = sqlx::query_as(
            "SELECT value_blob, encrypted FROM config_store WHERE key = 'owner_keypair'"
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| Error::Database(e))?;

        if let Some((key_data, encrypted)) = row {
            let key_bytes = if encrypted {
                // Decrypt using passphrase from environment
                Self::decrypt_keypair(&key_data)?
            } else {
                key_data
            };

            if let Some(signing_key) = Self::parse_ed25519_key(&key_bytes)? {
                let verifying_key = signing_key.verifying_key();
                self.owner_public_key = Some(hex::encode(verifying_key.as_bytes()));
                self.owner_signing_key = Some(signing_key);

                tracing::info!(
                    "Owner keypair loaded from database. Public key: {}",
                    self.owner_public_key.as_ref().unwrap()
                );
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

    /// Check if owner keypair is loaded.
    #[must_use]
    pub fn has_owner_keypair(&self) -> bool {
        self.owner_signing_key.is_some()
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
    /// | Thummim | 4 | 28GB | Yes | deepseek-r1:7b |
    /// | Urim | 8 | 56GB | Yes | deepseek-r1:32b |
    /// | Custom | 2 | 8GB | No | deepseek-r1:7b |
    #[must_use]
    pub fn machine_config(&self) -> MachineConfig {
        match self.machine_profile {
            MachineProfile::Thummim => MachineConfig {
                max_workers: 4,
                max_memory_mb: 28672, // 28GB, leaving 4GB for system
                gpu_enabled: true,
                default_model: "deepseek-r1:7b".to_string(),
            },
            MachineProfile::Urim => MachineConfig {
                max_workers: 8,
                max_memory_mb: 57344, // 56GB, leaving 8GB for system
                gpu_enabled: true,
                default_model: "deepseek-r1:32b".to_string(),
            },
            MachineProfile::Custom => self
                .custom_machine_config
                .clone()
                .unwrap_or_default(),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.http_port, 18789);
        assert_eq!(config.ws_port, 18790);
        assert_eq!(config.log_level, "INFO");
        assert_eq!(config.machine_profile, MachineProfile::Thummim);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_machine_profile_parsing() {
        assert_eq!(
            MachineProfile::from_str("thummim").unwrap(),
            MachineProfile::Thummim
        );
        assert_eq!(
            MachineProfile::from_str("THUMMIM").unwrap(),
            MachineProfile::Thummim
        );
        assert_eq!(
            MachineProfile::from_str("urim").unwrap(),
            MachineProfile::Urim
        );
        assert_eq!(
            MachineProfile::from_str("Urim").unwrap(),
            MachineProfile::Urim
        );
        assert_eq!(
            MachineProfile::from_str("custom").unwrap(),
            MachineProfile::Custom
        );
        assert!(MachineProfile::from_str("invalid").is_err());
    }

    #[test]
    fn test_machine_config_thummim() {
        let config = Config {
            machine_profile: MachineProfile::Thummim,
            ..Default::default()
        };
        let machine = config.machine_config();
        assert_eq!(machine.max_workers, 4);
        assert_eq!(machine.max_memory_mb, 28672);
        assert!(machine.gpu_enabled);
        assert_eq!(machine.default_model, "deepseek-r1:7b");
    }

    #[test]
    fn test_machine_config_urim() {
        let config = Config {
            machine_profile: MachineProfile::Urim,
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

        config.http_port = 18789;
        config.ws_port = 80; // Below 1024
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
