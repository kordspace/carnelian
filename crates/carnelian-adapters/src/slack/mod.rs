//! Slack Events API adapter for 🔥 Carnelian OS.
//!
//! Provides a webhook-based Slack bot that integrates with Carnelian's
//! session management, event streaming, and capability-based security systems.
//! This adapter is webhook-driven and uses HMAC-SHA256 signature verification
//! for request authentication.

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

/// Slack Events API adapter.
///
/// Wraps a `reqwest::Client` and integrates with Carnelian subsystems for
/// session management, rate limiting, spam detection, and capability checks.
/// The adapter is webhook-driven; `start()` only manages the running flag.
/// Request signatures are verified using HMAC-SHA256 with the `signing_secret`.
pub struct SlackAdapter {
    /// Channel configuration (`bot_token` = Slack Bot OAuth token xoxb-...).
    config: ChannelConfig,
    /// Slack signing secret for HMAC-SHA256 request verification.
    signing_secret: String,
    /// HTTP client for outbound API calls.
    http_client: reqwest::Client,
    /// Session manager for conversation persistence.
    session_manager: Arc<SessionManager>,
    /// Event stream for lifecycle events.
    event_stream: Arc<EventStream>,
    /// Policy engine for capability validation.
    policy_engine: Arc<PolicyEngine>,
    /// Per-channel rate limiter.
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

impl SlackAdapter {
    /// Create a new Slack adapter.
    ///
    /// The bot token is read from `config.bot_token`.
    /// The `signing_secret` is passed directly (not from config).
    ///
    /// # Errors
    ///
    /// Currently infallible, but returns `Result` for future extensibility.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: ChannelConfig,
        signing_secret: String,
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
            signing_secret,
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

    /// Returns the signing secret for HMAC verification.
    #[must_use]
    pub fn signing_secret(&self) -> &str {
        &self.signing_secret
    }

    /// Returns a clone of the shutdown receiver for monitoring shutdown signals.
    #[must_use]
    pub fn shutdown_receiver(&self) -> tokio::sync::watch::Receiver<bool> {
        self.shutdown_rx.clone()
    }
}

#[async_trait]
impl ChannelAdapter for SlackAdapter {
    fn name(&self) -> &'static str {
        "slack"
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
                "channel_type": "slack",
                "channel_id": self.config.channel_id.to_string(),
            }),
        ));

        tracing::info!(
            channel_id = %self.config.channel_id,
            "Slack adapter started"
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
                "channel_type": "slack",
            }),
        ));

        tracing::info!(
            channel_id = %self.config.channel_id,
            "Slack adapter stopped"
        );

        Ok(())
    }

    async fn send_message(&self, channel_user_id: &str, text: &str) -> anyhow::Result<()> {
        let url = "https://slack.com/api/chat.postMessage";

        let body = json!({
            "channel": channel_user_id,
            "text": text
        });

        let response = self
            .http_client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.config.bot_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Slack API request failed: {e}"))?;

        // Slack returns HTTP 200 even on errors, check response.ok field
        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse Slack API response: {e}"))?;

        let ok = response_json
            .get("ok")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !ok {
            let error = response_json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!("Slack API returned error: {error}");
        }

        // Emit message sent event
        self.event_stream.publish(EventEnvelope::new(
            EventLevel::Debug,
            EventType::Custom(events::SLACK_MESSAGE_SENT.to_string()),
            json!({
                "channel_type": "slack",
                "channel_user_id": channel_user_id,
            }),
        ));

        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Handle webhook POST with inbound payload.
    ///
    /// Slack uses a single POST endpoint for both URL verification and events.
    /// Extracts timestamp and signature from headers, delegates to handlers::handle_event,
    /// and returns appropriate JSON response.
    async fn handle_webhook_post(
        &self,
        headers: std::collections::HashMap<String, String>,
        body: bytes::Bytes,
    ) -> anyhow::Result<serde_json::Value> {
        // Extract required headers
        let timestamp = headers
            .get("x-slack-request-timestamp")
            .ok_or_else(|| anyhow::anyhow!("Missing x-slack-request-timestamp header"))?;
        let signature = headers
            .get("x-slack-signature")
            .ok_or_else(|| anyhow::anyhow!("Missing x-slack-signature header"))?;

        // Process the event
        let response = handlers::handle_event(&body, timestamp, signature, self).await?;

        // Return appropriate JSON based on response type
        match response {
            handlers::SlackEventResponse::Challenge(challenge) => {
                Ok(serde_json::json!({"challenge": challenge}))
            }
            handlers::SlackEventResponse::Ok => Ok(serde_json::json!({})),
        }
    }
}
