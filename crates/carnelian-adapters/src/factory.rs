//! Adapter factory for constructing channel adapters from configuration.
//!
//! The `DefaultAdapterFactory` implements `ChannelAdapterFactory` from
//! `carnelian-common`, enabling `carnelian-core` to construct adapters
//! without direct dependency on `carnelian-adapters`.
//!
//! ## Credential Encoding Convention
//!
//! Since `ChannelAdapterFactory::build()` carries a single `bot_token: &str`,
//! the following encoding convention is used for multi-field credentials:
//!
//! | `channel_type` | `bot_token` format |
//! |----------------|-------------------|
//! | `"telegram"` | plain token string |
//! | `"discord"` | plain token string |
//! | `"whatsapp"` | JSON: `{"access_token":"…","phone_number_id":"…","verify_token":"…"}` |
//! | `"slack"` | JSON: `{"bot_token":"…","signing_secret":"…"}` |

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use uuid::Uuid;

use carnelian_common::ChannelAdapter;
use carnelian_common::channel::ChannelAdapterFactory;
use carnelian_core::EventStream;
use carnelian_core::policy::PolicyEngine;
use carnelian_core::session::SessionManager;
use sqlx::PgPool;

use crate::config;
use crate::discord::DiscordAdapter;
use crate::rate_limiter::RateLimiter;
use crate::slack::SlackAdapter;
use crate::spam_detector::SpamDetector;
use crate::telegram::TelegramAdapter;
use crate::types::{ChannelConfig, ChannelType, TrustLevel};
use crate::whatsapp::WhatsAppAdapter;

/// Default factory for building channel adapters.
///
/// Holds shared dependencies (database pool, session manager, event stream,
/// policy engine, and spam configuration) needed by all adapter constructors.
pub struct DefaultAdapterFactory {
    /// Database connection pool for persistence.
    db_pool: PgPool,
    /// Session manager for conversation persistence.
    session_manager: Arc<SessionManager>,
    /// Event stream for lifecycle events.
    event_stream: Arc<EventStream>,
    /// Policy engine for capability validation.
    policy_engine: Arc<PolicyEngine>,
    /// Spam score threshold (0.0–1.0).
    spam_threshold: f32,
    /// TTL for spam score entries in seconds.
    spam_ttl_secs: u64,
}

impl DefaultAdapterFactory {
    /// Create a new adapter factory.
    pub const fn new(
        db_pool: PgPool,
        session_manager: Arc<SessionManager>,
        event_stream: Arc<EventStream>,
        policy_engine: Arc<PolicyEngine>,
        spam_threshold: f32,
        spam_ttl_secs: u64,
    ) -> Self {
        Self {
            db_pool,
            session_manager,
            event_stream,
            policy_engine,
            spam_threshold,
            spam_ttl_secs,
        }
    }
}

#[async_trait]
impl ChannelAdapterFactory for DefaultAdapterFactory {
    async fn build(
        &self,
        session_id: Uuid,
        channel_type: &str,
        channel_user_id: &str,
        bot_token: &str,
        trust_level: &str,
        identity_id: Option<Uuid>,
    ) -> anyhow::Result<Arc<dyn ChannelAdapter>> {
        // 1. Persist the credential blob
        config::store_bot_credential(&self.db_pool, channel_type, channel_user_id, bot_token)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to store bot credential: {e}"))?;

        // 2. Parse trust level (default to Conversational on failure)
        let parsed_trust_level = trust_level
            .parse::<TrustLevel>()
            .unwrap_or(TrustLevel::Conversational);

        // 3. Parse channel type
        let parsed_channel_type = channel_type
            .parse::<ChannelType>()
            .map_err(|_| anyhow::anyhow!("Unsupported channel type: {channel_type}"))?;

        // 4. Build ChannelConfig
        let mut channel_config = ChannelConfig {
            channel_id: session_id,
            channel_type: parsed_channel_type,
            bot_token: bot_token.to_string(),
            default_trust_level: parsed_trust_level,
            enabled: true,
            identity_id,
        };

        // 5. Construct RateLimiter
        let rate_limiter = Arc::new(RateLimiter::new(Some(self.event_stream.clone())));

        // 6. Construct SpamDetector
        let spam_detector = Arc::new(SpamDetector::new(
            self.spam_threshold,
            Duration::from_secs(self.spam_ttl_secs),
            Some(self.event_stream.clone()),
        ));

        // 7. Match on channel type and construct appropriate adapter
        let adapter: Arc<dyn ChannelAdapter> = match channel_type {
            "telegram" => {
                // Plain token - use as-is
                let adapter = TelegramAdapter::new(
                    channel_config,
                    self.session_manager.clone(),
                    self.event_stream.clone(),
                    self.policy_engine.clone(),
                    rate_limiter,
                    spam_detector,
                    self.db_pool.clone(),
                )?;
                Arc::new(adapter)
            }
            "discord" => {
                // Plain token - use as-is
                let adapter = DiscordAdapter::new(
                    channel_config,
                    self.session_manager.clone(),
                    self.event_stream.clone(),
                    self.policy_engine.clone(),
                    rate_limiter,
                    spam_detector,
                    self.db_pool.clone(),
                )?;
                Arc::new(adapter)
            }
            "whatsapp" => {
                // JSON-encoded: {"access_token":"...","phone_number_id":"...","verify_token":"..."}
                let parsed: serde_json::Value = serde_json::from_str(bot_token).map_err(|e| {
                    anyhow::anyhow!("Failed to parse WhatsApp credentials as JSON: {e}")
                })?;

                let access_token = parsed
                    .get("access_token")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Missing access_token in WhatsApp credentials")
                    })?;
                let phone_number_id = parsed
                    .get("phone_number_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Missing phone_number_id in WhatsApp credentials")
                    })?;
                let verify_token = parsed
                    .get("verify_token")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Missing verify_token in WhatsApp credentials")
                    })?;

                // Set the bot_token to the actual access token
                channel_config.bot_token = access_token.to_string();

                let adapter = WhatsAppAdapter::new(
                    channel_config,
                    phone_number_id.to_string(),
                    verify_token.to_string(),
                    self.session_manager.clone(),
                    self.event_stream.clone(),
                    self.policy_engine.clone(),
                    rate_limiter,
                    spam_detector,
                    self.db_pool.clone(),
                )?;
                Arc::new(adapter)
            }
            "slack" => {
                // JSON-encoded: {"bot_token":"...","signing_secret":"..."}
                let parsed: serde_json::Value = serde_json::from_str(bot_token).map_err(|e| {
                    anyhow::anyhow!("Failed to parse Slack credentials as JSON: {e}")
                })?;

                let bot_token_field = parsed
                    .get("bot_token")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing bot_token in Slack credentials"))?;
                let signing_secret = parsed
                    .get("signing_secret")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Missing signing_secret in Slack credentials")
                    })?;

                // Set the bot_token to the actual bot token
                channel_config.bot_token = bot_token_field.to_string();

                let adapter = SlackAdapter::new(
                    channel_config,
                    signing_secret.to_string(),
                    self.session_manager.clone(),
                    self.event_stream.clone(),
                    self.policy_engine.clone(),
                    rate_limiter,
                    spam_detector,
                    self.db_pool.clone(),
                )?;
                Arc::new(adapter)
            }
            _ => {
                anyhow::bail!("Unsupported channel type: {channel_type}");
            }
        };

        Ok(adapter)
    }

    async fn delete_credentials(
        &self,
        channel_type: &str,
        channel_user_id: &str,
    ) -> anyhow::Result<()> {
        config::delete_bot_credential(&self.db_pool, channel_type, channel_user_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete bot credential: {e}"))?;
        Ok(())
    }
}
