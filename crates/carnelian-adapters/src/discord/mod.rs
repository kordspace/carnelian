//! Discord bot adapter using `serenity`.
//!
//! Provides a Discord bot that integrates with Carnelian's session management,
//! event streaming, and capability-based security systems.

pub mod commands;
pub mod handlers;
pub mod pairing;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use serde_json::json;
use serenity::Client;
use serenity::all::{ChannelId, GatewayIntents, Http};
use sqlx::PgPool;
use tokio::sync::RwLock;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::EventStream;
use carnelian_core::policy::PolicyEngine;
use carnelian_core::session::SessionManager;

use crate::ChannelAdapter;
use crate::events;
use crate::rate_limiter::RateLimiter;
use crate::spam_detector::SpamDetector;
use crate::types::ChannelConfig;

/// Discord bot adapter.
///
/// Wraps a `serenity::Client` and integrates with Carnelian subsystems for
/// session management, rate limiting, spam detection, and capability checks.
pub struct DiscordAdapter {
    /// Channel configuration (token, trust level, etc.).
    config: ChannelConfig,
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
    /// Serenity HTTP client, populated after the client connects.
    /// Used by `send_message` to send outbound messages.
    http_client: Arc<RwLock<Option<Arc<Http>>>>,
}

impl DiscordAdapter {
    /// Create a new Discord adapter.
    ///
    /// The bot token is read from `config.bot_token`.
    ///
    /// # Errors
    ///
    /// Currently infallible, but returns `Result` for future extensibility.
    pub fn new(
        config: ChannelConfig,
        session_manager: Arc<SessionManager>,
        event_stream: Arc<EventStream>,
        policy_engine: Arc<PolicyEngine>,
        rate_limiter: Arc<RateLimiter>,
        spam_detector: Arc<SpamDetector>,
        db_pool: PgPool,
    ) -> anyhow::Result<Self> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        Ok(Self {
            config,
            session_manager,
            event_stream,
            policy_engine,
            rate_limiter,
            spam_detector,
            db_pool,
            running: Arc::new(AtomicBool::new(false)),
            shutdown_tx,
            shutdown_rx,
            http_client: Arc::new(RwLock::new(None)),
        })
    }

    /// Returns a reference to the channel configuration.
    #[must_use]
    pub const fn config(&self) -> &ChannelConfig {
        &self.config
    }
}

#[async_trait]
impl ChannelAdapter for DiscordAdapter {
    fn name(&self) -> &'static str {
        "discord"
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
                "channel_type": "discord",
                "channel_id": self.config.channel_id.to_string(),
            }),
        ));

        tracing::info!(
            channel_id = %self.config.channel_id,
            "Discord adapter started"
        );

        // Build the serenity event handler
        let handler = handlers::DiscordHandler {
            session_manager: self.session_manager.clone(),
            event_stream: self.event_stream.clone(),
            policy_engine: self.policy_engine.clone(),
            rate_limiter: self.rate_limiter.clone(),
            spam_detector: self.spam_detector.clone(),
            db_pool: self.db_pool.clone(),
            config: self.config.clone(),
        };

        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;

        let token = self.config.bot_token.clone();
        let running = self.running.clone();
        let event_stream = self.event_stream.clone();
        let mut shutdown_rx = self.shutdown_rx.clone();
        let http_client = self.http_client.clone();

        // Pre-create an Http client from the token so send_message works
        // even before the gateway connects.
        {
            let http = Arc::new(Http::new(&token));
            *http_client.write().await = Some(http);
        }

        tokio::spawn(async move {
            let client_result = Client::builder(&token, intents)
                .event_handler(handler)
                .await;

            match client_result {
                Ok(mut client) => {
                    // Update the stored Http handle with the one from the live client
                    // (it carries the authenticated session context).
                    {
                        *http_client.write().await = Some(client.http.clone());
                    }

                    tokio::select! {
                        result = client.start() => {
                            if let Err(e) = result {
                                tracing::error!(error = %e, "Discord client error");
                            }
                        }
                        _ = shutdown_rx.changed() => {
                            tracing::info!("Discord adapter received shutdown signal");
                            client.shard_manager.shutdown_all().await;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to build Discord client");
                }
            }

            // Clear the Http handle on shutdown
            *http_client.write().await = None;

            running.store(false, Ordering::SeqCst);

            // Emit disconnected event
            event_stream.publish(EventEnvelope::new(
                EventLevel::Info,
                EventType::Custom(events::CHANNEL_DISCONNECTED.to_string()),
                json!({
                    "channel_type": "discord",
                }),
            ));
        });

        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        let _ = self.shutdown_tx.send(true);
        tracing::info!(
            channel_id = %self.config.channel_id,
            "Discord adapter stop requested"
        );

        Ok(())
    }

    async fn send_message(&self, channel_user_id: &str, text: &str) -> anyhow::Result<()> {
        let http = self.http_client.read().await.clone().ok_or_else(|| {
            anyhow::anyhow!("Discord HTTP client not available — adapter may not be running")
        })?;

        let channel_id: u64 = channel_user_id
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid Discord channel ID: {e}"))?;

        ChannelId::new(channel_id)
            .say(&http, text)
            .await
            .map_err(|e| anyhow::anyhow!("Discord send failed: {e}"))?;

        // Emit message sent event
        self.event_stream.publish(EventEnvelope::new(
            EventLevel::Debug,
            EventType::Custom(events::CHANNEL_MESSAGE_SENT.to_string()),
            json!({
                "channel_type": "discord",
                "channel_user_id": channel_user_id,
            }),
        ));

        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}
