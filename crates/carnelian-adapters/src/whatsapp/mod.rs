//! `WhatsApp` Cloud API adapter for 🔥 Carnelian OS.
//!
//! Provides a webhook-based `WhatsApp` bot that integrates with Carnelian's
//! session management, event streaming, and capability-based security systems.
//! Unlike Telegram's polling loop, this adapter is driven by HTTP webhooks
//! registered in `carnelian-core`.

pub mod commands;
pub mod handlers;
pub mod pairing;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use serde_json::json;
use sqlx::PgPool;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::EventStream;
use carnelian_core::policy::PolicyEngine;
use carnelian_core::session::SessionManager;

use crate::ChannelAdapter;
use crate::events;
use crate::rate_limiter::RateLimiter;
use crate::spam_detector::SpamDetector;
use crate::types::ChannelConfig;

/// `WhatsApp` Cloud API adapter.
///
/// Wraps a `reqwest::Client` and integrates with Carnelian subsystems for
/// session management, rate limiting, spam detection, and capability checks.
/// The adapter is webhook-driven; `start()` only manages the running flag.
pub struct WhatsAppAdapter {
    /// Channel configuration (bot_token = Meta access token).
    config: ChannelConfig,
    /// `WhatsApp` phone number ID for Graph API URL construction.
    phone_number_id: String,
    /// Verification token for Meta hub challenge.
    verify_token: String,
    /// HTTP client for outbound API calls.
    http_client: reqwest::Client,
    /// Session manager for conversation persistence.
    session_manager: Arc<SessionManager>,
    /// Event stream for lifecycle events.
    event_stream: Arc<EventStream>,
    /// Policy engine for capability validation.
    policy_engine: Arc<PolicyEngine>,
    /// Per-user rate limiter.
    rate_limiter: Arc<RateLimiter>,
    /// Spam score tracker.
    spam_detector: Arc<SpamDetector>,
    /// Database connection pool.
    db_pool: PgPool,
    /// Whether the adapter is currently running.
    running: Arc<AtomicBool>,
    /// Shutdown signal sender.
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    /// Shutdown signal receiver (cloneable).
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
}

impl WhatsAppAdapter {
    /// Create a new `WhatsApp` adapter.
    ///
    /// The access token is read from `config.bot_token`.
    ///
    /// # Errors
    ///
    /// Currently infallible, but returns `Result` for future extensibility.
    pub fn new(
        config: ChannelConfig,
        phone_number_id: String,
        verify_token: String,
        session_manager: Arc<SessionManager>,
        event_stream: Arc<EventStream>,
        policy_engine: Arc<PolicyEngine>,
        rate_limiter: Arc<RateLimiter>,
        spam_detector: Arc<SpamDetector>,
        db_pool: PgPool,
    ) -> anyhow::Result<Self> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let http_client = reqwest::Client::new();

        Ok(Self {
            config,
            phone_number_id,
            verify_token,
            http_client,
            session_manager,
            event_stream,
            policy_engine,
            rate_limiter,
            spam_detector,
            db_pool,
            running: Arc::new(AtomicBool::new(false)),
            shutdown_tx,
            shutdown_rx,
        })
    }

    /// Returns a reference to the channel configuration.
    #[must_use]
    pub const fn config(&self) -> &ChannelConfig {
        &self.config
    }

    /// Returns the phone number ID.
    #[must_use]
    pub fn phone_number_id(&self) -> &str {
        &self.phone_number_id
    }

    /// Returns the verification token.
    #[must_use]
    pub fn verify_token(&self) -> &str {
        &self.verify_token
    }

    /// Returns a clone of the shutdown receiver for monitoring shutdown signals.
    #[must_use]
    pub fn shutdown_receiver(&self) -> tokio::sync::watch::Receiver<bool> {
        self.shutdown_rx.clone()
    }
}

#[async_trait]
impl ChannelAdapter for WhatsAppAdapter {
    fn name(&self) -> &'static str {
        "whatsapp"
    }

    async fn start(&self) -> anyhow::Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.running.store(true, Ordering::SeqCst);

        // Emit connected event
        self.event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::Custom(events::CHANNEL_CONNECTED.to_string()),
            json!({
                "channel_type": "whatsapp",
                "channel_id": self.config.channel_id.to_string(),
            }),
        ));

        tracing::info!(
            channel_id = %self.config.channel_id,
            phone_number_id = %self.phone_number_id,
            "WhatsApp adapter started"
        );

        // Webhook-driven: no polling loop. HTTP routes handle inbound traffic.

        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        let _ = self.shutdown_tx.send(true);
        self.running.store(false, Ordering::SeqCst);

        // Emit disconnected event
        self.event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::Custom(events::CHANNEL_DISCONNECTED.to_string()),
            json!({
                "channel_type": "whatsapp",
            }),
        ));

        tracing::info!(
            channel_id = %self.config.channel_id,
            "WhatsApp adapter stopped"
        );

        Ok(())
    }

    async fn send_message(&self, channel_user_id: &str, text: &str) -> anyhow::Result<()> {
        let url = format!(
            "https://graph.facebook.com/v18.0/{}/messages",
            self.phone_number_id
        );

        let body = json!({
            "messaging_product": "whatsapp",
            "to": channel_user_id,
            "type": "text",
            "text": {
                "body": text
            }
        });

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.bot_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("WhatsApp API request failed: {e}"))?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            anyhow::bail!("WhatsApp API returned {status}: {body_text}");
        }

        // Emit message sent event
        self.event_stream.publish(EventEnvelope::new(
            EventLevel::Debug,
            EventType::Custom(events::WHATSAPP_MESSAGE_SENT.to_string()),
            json!({
                "channel_type": "whatsapp",
                "channel_user_id": channel_user_id,
            }),
        ));

        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Handle webhook verification request (Meta hub challenge).
    ///
    /// Parses hub.mode, hub.verify_token, and hub.challenge from params,
    /// delegates to handlers::handle_verification, and returns the challenge.
    async fn handle_webhook_verify(
        &self,
        params: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<String> {
        let query = handlers::WhatsAppVerifyQuery {
            hub_mode: params.get("hub.mode").cloned().unwrap_or_default(),
            hub_verify_token: params.get("hub.verify_token").cloned().unwrap_or_default(),
            hub_challenge: params.get("hub.challenge").cloned().unwrap_or_default(),
        };

        handlers::handle_verification(&query, &self.verify_token, &self.event_stream)
    }

    /// Handle webhook POST with inbound payload.
    ///
    /// Deserializes body as WhatsAppWebhookPayload and processes it.
    /// Returns empty JSON object on success.
    async fn handle_webhook_post(
        &self,
        _headers: std::collections::HashMap<String, String>,
        body: bytes::Bytes,
    ) -> anyhow::Result<serde_json::Value> {
        let payload: handlers::WhatsAppWebhookPayload = serde_json::from_slice(&body)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize WhatsApp payload: {e}"))?;

        handlers::handle_inbound(&payload, self).await?;
        Ok(serde_json::json!({}))
    }
}
