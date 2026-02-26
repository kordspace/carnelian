//! Channel adapter trait and shared types.
//!
//! Defines the `ChannelAdapter` and `ChannelAdapterFactory` traits used by
//! both `carnelian-core` (to hold adapters in `AppState`) and
//! `carnelian-adapters` (to implement them).
//! Living in `carnelian-common` breaks the cyclic dependency.

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use uuid::Uuid;

/// Trait implemented by all channel adapters (Telegram, Discord, etc.).
///
/// Provides a uniform interface for starting, stopping, and sending messages
/// through any supported channel.
#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// Human-readable name of the adapter (e.g., "telegram", "discord").
    fn name(&self) -> &str;

    /// Start the adapter, connecting to the bot API and polling for updates.
    async fn start(&self) -> anyhow::Result<()>;

    /// Stop the adapter gracefully, disconnecting from the bot API.
    async fn stop(&self) -> anyhow::Result<()>;

    /// Send a text message to a specific channel user.
    async fn send_message(&self, channel_user_id: &str, text: &str) -> anyhow::Result<()>;

    /// Returns `true` if the adapter is currently running.
    fn is_running(&self) -> bool;

    /// Handle webhook verification request (GET /v1/webhooks/{channel}).
    ///
    /// Default implementation returns an error indicating webhook not supported.
    /// Used for hub-verification flows like Meta's WhatsApp webhook verification.
    async fn handle_webhook_verify(
        &self,
        _params: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<String> {
        anyhow::bail!("webhook verification not supported")
    }

    /// Handle webhook POST request with inbound payload.
    ///
    /// Default implementation returns an error indicating webhook not supported.
    /// Returns a JSON body to echo back (e.g., `{"challenge": "..."}` for Slack
    /// URL verification, or `{}` for a plain 200 OK).
    async fn handle_webhook_post(
        &self,
        _headers: std::collections::HashMap<String, String>,
        _body: Bytes,
    ) -> anyhow::Result<serde_json::Value> {
        anyhow::bail!("webhook POST not supported")
    }
}

/// Factory trait for building channel adapters from configuration.
///
/// Injected into `AppState` by the binary so `carnelian-core` can construct
/// adapters without depending on `carnelian-adapters` directly.
#[async_trait]
pub trait ChannelAdapterFactory: Send + Sync {
    /// Build a channel adapter for the given parameters.
    ///
    /// The factory is responsible for:
    /// - Validating `channel_type`
    /// - Storing the bot token in `config_store`
    /// - Constructing the appropriate adapter (Telegram/Discord)
    /// - **Not** starting the adapter (caller will call `.start()`)
    async fn build(
        &self,
        session_id: Uuid,
        channel_type: &str,
        channel_user_id: &str,
        bot_token: &str,
        trust_level: &str,
        identity_id: Option<Uuid>,
    ) -> anyhow::Result<Arc<dyn ChannelAdapter>>;

    /// Delete stored credentials for a channel.
    async fn delete_credentials(
        &self,
        channel_type: &str,
        channel_user_id: &str,
    ) -> anyhow::Result<()>;
}
