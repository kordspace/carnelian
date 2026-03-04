//! Telegram message and command handlers.
//!
//! Uses `teloxide::dispatching` to route incoming updates to the appropriate
//! handler functions, applying rate limiting, spam detection, and capability
//! checks before processing.

use std::sync::Arc;

use serde_json::json;
use sqlx::PgPool;
use teloxide::dispatching::{UpdateFilterExt, UpdateHandler};
use teloxide::prelude::*;
use teloxide::types::Update;
use uuid::Uuid;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::policy::PolicyEngine;
use carnelian_core::session::SessionManager;
use carnelian_core::EventStream;

use crate::events;
use crate::rate_limiter::RateLimiter;
use crate::spam_detector::SpamDetector;
use crate::types::ChannelConfig;
use crate::{db as channel_db, types::TrustLevel};

/// Build the teloxide update handler tree.
///
/// Routes:
/// - `/start`, `/help`, `/pair`, `/status`, `/unpair` → command handlers
/// - Text messages → `handle_message`
pub fn build_handler() -> UpdateHandler<anyhow::Error> {
    let command_handler = Update::filter_message()
        .filter_command::<super::commands::Command>()
        .endpoint(super::commands::handle_command);

    let message_handler = Update::filter_message().endpoint(handle_message);

    dptree::entry()
        .branch(command_handler)
        .branch(message_handler)
}

/// Process a single message from Telegram.
///
/// 1. Extract text content
/// 2. Look up or create channel session
/// 3. Check rate limits
/// 4. Check spam score
/// 5. Verify capabilities
/// 6. Forward to session manager
/// 7. Emit `ChannelMessageReceived` event
/// 8. Update `last_seen_at`
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
async fn handle_message(
    bot: Bot,
    msg: Message,
    session_manager: Arc<SessionManager>,
    event_stream: Arc<EventStream>,
    policy_engine: Arc<PolicyEngine>,
    rate_limiter: Arc<RateLimiter>,
    spam_detector: Arc<SpamDetector>,
    db_pool: PgPool,
    config: ChannelConfig,
) -> anyhow::Result<()> {
    let text = match msg.text() {
        Some(t) => t.to_string(),
        None => return Ok(()), // Ignore non-text messages for now
    };

    let chat_id = msg.chat.id.0.to_string();
    let user_id = msg
        .from
        .as_ref()
        .map_or_else(|| chat_id.clone(), |u| u.id.0.to_string());

    // 1. Load or create channel session
    let session = channel_db::upsert_channel_session(
        &db_pool,
        "telegram",
        &chat_id,
        TrustLevel::Untrusted.as_str(),
        config.identity_id,
        json!({"telegram_user_id": user_id}),
    )
    .await?;

    let trust_level = session.parsed_trust_level();

    // 2. Check rate limit
    if let Err(e) = rate_limiter.check_rate_limit("telegram", &chat_id, trust_level) {
        tracing::warn!(%e, "Rate limit exceeded for Telegram user");
        bot.send_message(
            msg.chat.id,
            "⏳ You're sending messages too quickly. Please slow down.",
        )
        .await?;
        return Ok(());
    }

    // 3. Update spam score
    let spam_score = spam_detector.update_score("telegram", &chat_id, &text);
    if spam_detector.is_spam(spam_score) {
        tracing::warn!(
            chat_id = %chat_id,
            spam_score = %spam_score,
            "Spam detected from Telegram user"
        );
        // Silently drop spam messages
        return Ok(());
    }

    // 4. Check capability
    let has_receive = policy_engine
        .check_capability(
            "channel",
            &session.session_id.to_string(),
            "channel.message.receive",
            Some(&event_stream),
        )
        .await
        .unwrap_or(false);

    if !has_receive {
        // Unpaired users without capabilities get a pairing prompt
        bot.send_message(
            msg.chat.id,
            "🔒 This channel is not yet paired. Use /pair to connect.",
        )
        .await?;
        return Ok(());
    }

    // 5. Create or resolve session in SessionManager
    // Use the persisted session_id as the stable identity to avoid key fragmentation.
    // Previously this used Uuid::now_v7() when identity_id was unset, generating a
    // new key per message and fragmenting conversation history.
    let stable_identity = config.identity_id.unwrap_or(session.session_id);
    let session_key = format!("agent:{stable_identity}:telegram:group:{chat_id}");

    // Ensure a conversation session exists
    let conv_session = session_manager.create_session(&session_key).await;
    if let Ok(conv) = conv_session {
        // 6. Append user message
        if let Err(e) = session_manager
            .append_message(
                conv.session_id,
                "user",
                text.clone(),
                None, // token count estimated later
                None,
                None,
                None,
                None,
                None,
            )
            .await
        {
            tracing::warn!(error = %e, "Failed to append Telegram message to session");
        }
    }

    // 7. Emit event
    event_stream.publish(
        EventEnvelope::new(
            EventLevel::Info,
            EventType::Custom(events::CHANNEL_MESSAGE_RECEIVED.to_string()),
            json!({
                "channel_type": "telegram",
                "channel_user_id": chat_id,
                "trust_level": trust_level.as_str(),
                "message_length": text.len(),
                "correlation_id": Uuid::now_v7().to_string(),
            }),
        )
        .with_actor_id(format!("telegram:{chat_id}")),
    );

    // 8. Update last_seen_at
    let _ = channel_db::touch_channel_session(&db_pool, session.session_id).await;

    // Known limitation (v1.0.0): adapter acknowledges receipt only; full agentic loop
    // routing from channel messages deferred.
    // For now, acknowledge receipt
    bot.send_message(msg.chat.id, "✅ Message received.")
        .await?;

    Ok(())
}
