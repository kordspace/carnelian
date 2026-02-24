//! Telegram bot adapter using `teloxide`.
//!
//! Provides a full-featured Telegram bot that integrates with Carnelian's
//! session management, event streaming, and capability-based security.

pub mod commands;
pub mod handlers;
pub mod pairing;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use serde_json::json;
use sqlx::PgPool;
use teloxide::prelude::*;
use teloxide::types::ChatId;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::EventStream;
use carnelian_core::policy::PolicyEngine;
use carnelian_core::session::SessionManager;

use crate::ChannelAdapter;
use crate::events;
use crate::rate_limiter::RateLimiter;
use crate::spam_detector::SpamDetector;
use crate::types::ChannelConfig;

/// Telegram bot adapter.
///
/// Wraps a `teloxide::Bot` and integrates with Carnelian subsystems for
/// session management, rate limiting, spam detection, and capability checks.
pub struct TelegramAdapter {
    /// The teloxide bot instance.
    bot: Bot,
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
}

impl TelegramAdapter {
    /// Create a new Telegram adapter.
    ///
    /// The bot token is read from `config.bot_token`.
    pub fn new(
        config: ChannelConfig,
        session_manager: Arc<SessionManager>,
        event_stream: Arc<EventStream>,
        policy_engine: Arc<PolicyEngine>,
        rate_limiter: Arc<RateLimiter>,
        spam_detector: Arc<SpamDetector>,
        db_pool: PgPool,
    ) -> anyhow::Result<Self> {
        let bot = Bot::new(&config.bot_token);
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        Ok(Self {
            bot,
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
        })
    }

    /// Returns a reference to the underlying `teloxide::Bot`.
    #[must_use]
    pub fn bot(&self) -> &Bot {
        &self.bot
    }

    /// Returns a reference to the channel configuration.
    #[must_use]
    pub fn config(&self) -> &ChannelConfig {
        &self.config
    }
}

#[async_trait]
impl ChannelAdapter for TelegramAdapter {
    fn name(&self) -> &str {
        "telegram"
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
                "channel_type": "telegram",
                "channel_id": self.config.channel_id.to_string(),
            }),
        ));

        tracing::info!(
            channel_id = %self.config.channel_id,
            "Telegram adapter started"
        );

        // Spawn the polling loop in a background task
        let bot = self.bot.clone();
        let session_manager = self.session_manager.clone();
        let event_stream = self.event_stream.clone();
        let policy_engine = self.policy_engine.clone();
        let rate_limiter = self.rate_limiter.clone();
        let spam_detector = self.spam_detector.clone();
        let db_pool = self.db_pool.clone();
        let config = self.config.clone();
        let running = self.running.clone();
        let mut shutdown_rx = self.shutdown_rx.clone();

        tokio::spawn(async move {
            let handler = handlers::build_handler();

            let mut dispatcher = Dispatcher::builder(bot, handler)
                .dependencies(dptree::deps![
                    session_manager,
                    event_stream.clone(),
                    policy_engine,
                    rate_limiter,
                    spam_detector,
                    db_pool,
                    config
                ])
                .enable_ctrlc_handler()
                .build();

            tokio::select! {
                () = dispatcher.dispatch() => {
                    tracing::info!("Telegram dispatcher exited");
                }
                _ = shutdown_rx.changed() => {
                    tracing::info!("Telegram adapter received shutdown signal");
                    let _ = dispatcher.shutdown_token().shutdown();
                }
            }

            running.store(false, Ordering::SeqCst);

            // Emit disconnected event
            event_stream.publish(EventEnvelope::new(
                EventLevel::Info,
                EventType::Custom(events::CHANNEL_DISCONNECTED.to_string()),
                json!({
                    "channel_type": "telegram",
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
            "Telegram adapter stop requested"
        );

        Ok(())
    }

    async fn send_message(&self, channel_user_id: &str, text: &str) -> anyhow::Result<()> {
        let chat_id: i64 = channel_user_id
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid Telegram chat ID: {e}"))?;

        self.bot
            .send_message(ChatId(chat_id), text)
            .await
            .map_err(|e| anyhow::anyhow!("Telegram send failed: {e}"))?;

        // Emit message sent event
        self.event_stream.publish(EventEnvelope::new(
            EventLevel::Debug,
            EventType::Custom(events::CHANNEL_MESSAGE_SENT.to_string()),
            json!({
                "channel_type": "telegram",
                "channel_user_id": channel_user_id,
            }),
        ));

        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}
