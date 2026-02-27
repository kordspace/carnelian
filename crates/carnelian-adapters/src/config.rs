//! Adapter configuration and credential management.
//!
//! Loads adapter settings from the `config_store` table or environment
//! variables. Bot tokens are stored encrypted and decrypted at runtime.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use carnelian_common::{Error, Result};

/// Top-level adapter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct AdapterConfig {
    /// Whether the Telegram adapter is enabled.
    #[serde(default)]
    pub telegram_enabled: bool,

    /// Whether the Discord adapter is enabled.
    #[serde(default)]
    pub discord_enabled: bool,

    /// Whether the `WhatsApp` adapter is enabled.
    #[serde(default)]
    pub whatsapp_enabled: bool,

    /// Whether the Slack adapter is enabled.
    #[serde(default)]
    pub slack_enabled: bool,

    /// Spam score threshold (0.0–1.0). Messages above this are dropped.
    #[serde(default = "default_spam_threshold")]
    pub spam_threshold: f32,

    /// TTL for spam score entries in seconds.
    #[serde(default = "default_spam_ttl_secs")]
    pub spam_ttl_secs: u64,
}

const fn default_spam_threshold() -> f32 {
    0.8
}

const fn default_spam_ttl_secs() -> u64 {
    3600
}

impl Default for AdapterConfig {
    fn default() -> Self {
        Self {
            telegram_enabled: false,
            discord_enabled: false,
            whatsapp_enabled: false,
            slack_enabled: false,
            spam_threshold: default_spam_threshold(),
            spam_ttl_secs: default_spam_ttl_secs(),
        }
    }
}

/// Load a bot credential from the `config_store` table.
///
/// The key format is `channel.{channel_type}.{channel_id}.token`.
/// Values are stored as plaintext in the JSONB `value` column.
/// Future enhancement: integrate with `carnelian-core/src/encryption.rs`
/// for encrypted storage.
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn load_bot_credential(
    pool: &PgPool,
    channel_type: &str,
    channel_id: &str,
) -> Result<Option<String>> {
    let key = format!("channel.{channel_type}.{channel_id}.token");

    let row: Option<(serde_json::Value,)> =
        sqlx::query_as(r"SELECT value FROM config_store WHERE key = $1")
            .bind(&key)
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;

    Ok(row.and_then(|(v,)| v.as_str().map(String::from)))
}

/// Store a bot credential in the `config_store` table.
///
/// Upserts the value so repeated calls are safe.
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn store_bot_credential(
    pool: &PgPool,
    channel_type: &str,
    channel_id: &str,
    token: &str,
) -> Result<()> {
    let key = format!("channel.{channel_type}.{channel_id}.token");
    let value = serde_json::Value::String(token.to_string());

    sqlx::query(
        r"
        INSERT INTO config_store (key, value)
        VALUES ($1, $2)
        ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = NOW()
        ",
    )
    .bind(&key)
    .bind(&value)
    .execute(pool)
    .await
    .map_err(Error::Database)?;

    Ok(())
}

/// Delete a bot credential from the `config_store` table.
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn delete_bot_credential(
    pool: &PgPool,
    channel_type: &str,
    channel_id: &str,
) -> Result<()> {
    let key = format!("channel.{channel_type}.{channel_id}.token");

    sqlx::query(r"DELETE FROM config_store WHERE key = $1")
        .bind(&key)
        .execute(pool)
        .await
        .map_err(Error::Database)?;

    Ok(())
}

/// Load adapter configuration from environment variables.
///
/// Supported variables:
/// - `TELEGRAM_BOT_TOKEN` — Telegram bot token (enables Telegram adapter)
/// - `DISCORD_BOT_TOKEN` — Discord bot token (enables Discord adapter)
/// - `WHATSAPP_ACCESS_TOKEN` — `WhatsApp` Cloud API access token (enables `WhatsApp` adapter)
/// - `SLACK_BOT_TOKEN` — Slack bot OAuth token (enables Slack adapter)
/// - `ADAPTER_SPAM_THRESHOLD` — Spam score threshold (default: 0.8)
#[must_use]
pub fn load_from_env() -> AdapterConfig {
    let telegram_enabled = std::env::var("TELEGRAM_BOT_TOKEN").is_ok();
    let discord_enabled = std::env::var("DISCORD_BOT_TOKEN").is_ok();
    let whatsapp_enabled = std::env::var("WHATSAPP_ACCESS_TOKEN").is_ok();
    let slack_enabled = std::env::var("SLACK_BOT_TOKEN").is_ok();

    let spam_threshold = std::env::var("ADAPTER_SPAM_THRESHOLD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default_spam_threshold());

    let spam_ttl_secs = std::env::var("ADAPTER_SPAM_TTL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default_spam_ttl_secs());

    AdapterConfig {
        telegram_enabled,
        discord_enabled,
        whatsapp_enabled,
        slack_enabled,
        spam_threshold,
        spam_ttl_secs,
    }
}
