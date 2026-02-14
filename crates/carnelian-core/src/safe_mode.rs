//! Safe Mode Guard
//!
//! Provides a centralized guard mechanism that blocks side-effect operations
//! when safe mode is enabled. Safe mode state is stored in the `config_store`
//! table as a JSON boolean value under the key `"safe_mode"`.
//!
//! When enabled, the guard blocks:
//! - Task execution (scheduler)
//! - Worker process spawning
//! - Remote model calls (local/Ollama calls are allowed)
//! - Filesystem writes (session transcripts)
//!
//! Enable/disable operations are logged to the tamper-resistant audit ledger
//! with Ed25519 signatures when a signing key is available.

use std::sync::Arc;

use carnelian_common::{Error, Result};
use ed25519_dalek::SigningKey;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::encryption::EncryptionHelper;
use crate::ledger::Ledger;

/// Centralized guard that checks safe mode state before allowing side-effect operations.
pub struct SafeModeGuard {
    /// Database connection pool
    pool: PgPool,
    /// Audit ledger for tamper-resistant logging
    ledger: Arc<Ledger>,
    /// Optional encryption helper for reading encrypted config entries
    encryption: Option<EncryptionHelper>,
}

impl SafeModeGuard {
    /// Create a new `SafeModeGuard`.
    ///
    /// # Arguments
    ///
    /// * `pool` - Database connection pool for querying `config_store`
    /// * `ledger` - Audit ledger for logging enable/disable events
    pub fn new(pool: PgPool, ledger: Arc<Ledger>) -> Self {
        Self {
            pool,
            ledger,
            encryption: None,
        }
    }

    /// Builder-style setter to enable decryption of encrypted config entries.
    ///
    /// When set, `is_enabled()` can read safe mode state even if it was stored
    /// via `update_config_value_encrypted`.
    #[must_use]
    pub fn with_encryption(mut self, helper: EncryptionHelper) -> Self {
        self.encryption = Some(helper);
        self
    }

    /// Check whether safe mode is currently enabled.
    ///
    /// Uses [`Config::read_config_value`] to transparently handle both plaintext
    /// and encrypted entries in `config_store`. Returns `false` (disabled) if the
    /// key is not found or cannot be parsed.
    pub async fn is_enabled(&self) -> Result<bool> {
        let result =
            crate::Config::read_config_value(&self.pool, "safe_mode", self.encryption.as_ref())
                .await;

        match result {
            Ok(Some((value, _key_version))) => Ok(value["enabled"].as_bool().unwrap_or(false)),
            Ok(None) => Ok(false),
            Err(_) => {
                // Graceful fallback: if decryption or parsing fails, treat as disabled
                tracing::warn!("Failed to read safe_mode config, defaulting to disabled");
                Ok(false)
            }
        }
    }

    /// Enable safe mode.
    ///
    /// Sets `safe_mode = {"enabled": true}` in `config_store` and logs a
    /// `safe_mode.enabled` event to the audit ledger with an optional Ed25519
    /// signature.
    ///
    /// # Arguments
    ///
    /// * `actor_id` - Optional UUID of the actor enabling safe mode
    /// * `owner_signing_key` - Optional Ed25519 key for signing the ledger event
    pub async fn enable(
        &self,
        actor_id: Option<Uuid>,
        owner_signing_key: Option<&SigningKey>,
    ) -> Result<()> {
        let value = json!({"enabled": true});
        let value_text = serde_json::to_string(&value)
            .map_err(|e| Error::Config(format!("Failed to serialize safe mode value: {}", e)))?;

        sqlx::query(
            r"INSERT INTO config_store (key, value_text, updated_at)
               VALUES ('safe_mode', $1, NOW())
               ON CONFLICT (key) DO UPDATE SET value_text = $1, updated_at = NOW()",
        )
        .bind(&value_text)
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        tracing::warn!(actor_id = ?actor_id, "Safe mode ENABLED");

        self.ledger
            .append_event(
                actor_id,
                "safe_mode.enabled",
                json!({
                    "actor_id": actor_id,
                }),
                None,
                owner_signing_key,
                None,
            )
            .await?;

        Ok(())
    }

    /// Disable safe mode.
    ///
    /// Sets `safe_mode = {"enabled": false}` in `config_store` and logs a
    /// `safe_mode.disabled` event to the audit ledger with an optional Ed25519
    /// signature.
    ///
    /// # Arguments
    ///
    /// * `actor_id` - Optional UUID of the actor disabling safe mode
    /// * `owner_signing_key` - Optional Ed25519 key for signing the ledger event
    pub async fn disable(
        &self,
        actor_id: Option<Uuid>,
        owner_signing_key: Option<&SigningKey>,
    ) -> Result<()> {
        let value = json!({"enabled": false});
        let value_text = serde_json::to_string(&value)
            .map_err(|e| Error::Config(format!("Failed to serialize safe mode value: {}", e)))?;

        sqlx::query(
            r"INSERT INTO config_store (key, value_text, updated_at)
               VALUES ('safe_mode', $1, NOW())
               ON CONFLICT (key) DO UPDATE SET value_text = $1, updated_at = NOW()",
        )
        .bind(&value_text)
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        tracing::info!(actor_id = ?actor_id, "Safe mode DISABLED");

        self.ledger
            .append_event(
                actor_id,
                "safe_mode.disabled",
                json!({
                    "actor_id": actor_id,
                }),
                None,
                owner_signing_key,
                None,
            )
            .await?;

        Ok(())
    }

    /// Check whether the requested operation is allowed.
    ///
    /// Returns `Ok(())` if safe mode is disabled, or
    /// `Err(Error::SafeModeActive)` if safe mode is enabled.
    ///
    /// # Arguments
    ///
    /// * `operation` - Name of the operation being attempted (for error messages)
    pub async fn check_or_block(&self, operation: &str) -> Result<()> {
        if self.is_enabled().await? {
            tracing::warn!(
                operation = %operation,
                "Operation blocked by safe mode"
            );
            return Err(Error::SafeModeActive(format!(
                "operation '{}' is blocked while safe mode is active",
                operation
            )));
        }
        Ok(())
    }
}
