//! Discord event handler implementing `serenity::EventHandler`.
//!
//! Handles incoming messages and applies rate limiting, spam detection,
//! and capability checks before forwarding to the session manager.

use std::sync::Arc;

use serde_json::json;
use serenity::all::{Context, EventHandler, Message, Ready};
use sqlx::PgPool;
use uuid::Uuid;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::policy::PolicyEngine;
use carnelian_core::session::SessionManager;
use carnelian_core::EventStream;

use crate::db as channel_db;
use crate::events;
use crate::rate_limiter::RateLimiter;
use crate::spam_detector::SpamDetector;
use crate::types::{ChannelConfig, TrustLevel};

/// Serenity event handler that bridges Discord events to Carnelian.
pub struct DiscordHandler {
    pub session_manager: Arc<SessionManager>,
    pub event_stream: Arc<EventStream>,
    pub policy_engine: Arc<PolicyEngine>,
    pub rate_limiter: Arc<RateLimiter>,
    pub spam_detector: Arc<SpamDetector>,
    pub db_pool: PgPool,
    pub config: ChannelConfig,
}

#[serenity::async_trait]
impl EventHandler for DiscordHandler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        tracing::info!(
            bot_name = %ready.user.name,
            "Discord bot connected"
        );

        // Register slash commands
        if let Err(e) = super::commands::register_commands(&_ctx) {
            tracing::warn!(error = %e, "Failed to register Discord slash commands");
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore bot messages
        if msg.author.bot {
            return;
        }

        let content = msg.content.clone();
        let channel_id = msg.channel_id.to_string();
        let user_id = msg.author.id.to_string();

        // Check for slash-style commands in message content
        if content.starts_with('!') {
            if let Err(e) = super::commands::handle_prefix_command(
                &ctx,
                &msg,
                &content,
                &self.db_pool,
                &self.event_stream,
                &self.policy_engine,
                self.config.identity_id,
            )
            .await
            {
                tracing::warn!(error = %e, "Discord command handler error");
            }
            return;
        }

        // 1. Load or create channel session
        let session = match channel_db::upsert_channel_session(
            &self.db_pool,
            "discord",
            &channel_id,
            TrustLevel::Untrusted.as_str(),
            self.config.identity_id,
            json!({
                "discord_user_id": user_id,
                "discord_guild_id": msg.guild_id.map(|g| g.to_string()),
                "discord_channel_name": msg.channel_id.to_string(),
            }),
        )
        .await
        {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to upsert Discord channel session");
                return;
            }
        };

        let trust_level = session.parsed_trust_level();

        // 2. Check rate limit
        if let Err(e) = self
            .rate_limiter
            .check_rate_limit("discord", &channel_id, trust_level)
        {
            tracing::warn!(%e, "Rate limit exceeded for Discord user");
            let _ = msg
                .channel_id
                .say(
                    &ctx.http,
                    "⏳ You're sending messages too quickly. Please slow down.",
                )
                .await;
            return;
        }

        // 3. Update spam score
        let spam_score = self
            .spam_detector
            .update_score("discord", &channel_id, &content);
        if self.spam_detector.is_spam(spam_score) {
            tracing::warn!(
                channel_id = %channel_id,
                spam_score = %spam_score,
                "Spam detected from Discord user"
            );
            return;
        }

        // 4. Check capability
        let has_receive = self
            .policy_engine
            .check_capability(
                "channel",
                &session.session_id.to_string(),
                "channel.message.receive",
                Some(&self.event_stream),
            )
            .await
            .unwrap_or(false);

        if !has_receive {
            let _ = msg
                .channel_id
                .say(
                    &ctx.http,
                    "🔒 This channel is not yet paired. Use `!pair` to connect.",
                )
                .await;
            return;
        }

        // 5. Create or resolve session in SessionManager
        // Use the persisted session_id as the stable identity to avoid key fragmentation.
        // Previously this used Uuid::now_v7() when identity_id was unset, generating a
        // new key per message and fragmenting conversation history.
        let stable_identity = self.config.identity_id.unwrap_or(session.session_id);
        let session_key = format!("agent:{stable_identity}:discord:group:{channel_id}");

        let conv_session = self.session_manager.create_session(&session_key).await;
        if let Ok(conv) = conv_session {
            if let Err(e) = self
                .session_manager
                .append_message(
                    conv.session_id,
                    "user",
                    content.clone(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .await
            {
                tracing::warn!(error = %e, "Failed to append Discord message to session");
            }
        }

        // 6. Emit event
        self.event_stream.publish(
            EventEnvelope::new(
                EventLevel::Info,
                EventType::Custom(events::CHANNEL_MESSAGE_RECEIVED.to_string()),
                json!({
                    "channel_type": "discord",
                    "channel_user_id": channel_id,
                    "discord_user_id": user_id,
                    "trust_level": trust_level.as_str(),
                    "message_length": content.len(),
                    "correlation_id": Uuid::now_v7().to_string(),
                }),
            )
            .with_actor_id(format!("discord:{channel_id}")),
        );

        // 7. Update last_seen_at
        let _ = channel_db::touch_channel_session(&self.db_pool, session.session_id).await;

        // Known limitation (v1.0.0): adapter acknowledges receipt only; full agentic loop
        // routing from channel messages deferred.
        let _ = msg.channel_id.say(&ctx.http, "✅ Message received.").await;
    }
}
