//! Telegram pairing flow for the `/pair` command.
//!
//! Implements a token-based pairing flow:
//! 1. User sends `/pair` → generate token, store pending session
//! 2. User sends `/pair <token>` → verify token, confirm pairing
//! 3. Grant capabilities based on trust level

use std::sync::Arc;

use serde_json::json;
use sqlx::PgPool;
use teloxide::prelude::*;
use uuid::Uuid;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::EventStream;
use carnelian_core::policy::PolicyEngine;

use crate::db as channel_db;
use crate::events;
use crate::types::{PairingRequest, TrustLevel};

/// Handle the `/pair` command.
///
/// Syntax:
/// - `/pair` — generate a new pairing token (default: conversational trust)
/// - `/pair <trust_level>` — generate token with requested trust level
/// - `/pair <token>` — verify and complete pairing
///
/// # Errors
///
/// Returns an error if database operations or message sending fails.
pub async fn handle_pair(
    bot: &Bot,
    msg: &Message,
    args: &str,
    db_pool: &PgPool,
    event_stream: &Arc<EventStream>,
    policy_engine: &Arc<PolicyEngine>,
    identity_id: Option<Uuid>,
) -> anyhow::Result<()> {
    let chat_id = msg.chat.id.0.to_string();
    let trimmed = args.trim();

    if trimmed.is_empty() {
        // Generate new pairing token with default trust level
        initiate_pairing(bot, msg, &chat_id, db_pool, identity_id, None).await
    } else if let Ok(requested_trust) = trimmed.parse::<TrustLevel>() {
        // Argument is a trust level name — initiate with that level
        initiate_pairing(
            bot,
            msg,
            &chat_id,
            db_pool,
            identity_id,
            Some(requested_trust),
        )
        .await
    } else {
        // Argument is a pairing token — verify and complete
        complete_pairing(
            bot,
            msg,
            &chat_id,
            trimmed,
            db_pool,
            event_stream,
            policy_engine,
        )
        .await
    }
}

/// Initiate pairing: create a pending channel session with a pairing token
/// stored in the metadata.
async fn initiate_pairing(
    bot: &Bot,
    msg: &Message,
    chat_id: &str,
    db_pool: &PgPool,
    identity_id: Option<Uuid>,
    requested_trust_level: Option<TrustLevel>,
) -> anyhow::Result<()> {
    let pairing = PairingRequest::new(
        crate::types::ChannelType::Telegram,
        chat_id.to_string(),
        requested_trust_level,
    );

    let metadata = json!({
        "pairing_token": pairing.token.to_string(),
        "pairing_status": "pending",
        "pairing_expires_at": pairing.expires_at.to_rfc3339(),
        "requested_trust_level": pairing.requested_trust_level.as_str(),
    });

    // Upsert the channel session with pending pairing metadata
    let _session = channel_db::upsert_channel_session(
        db_pool,
        "telegram",
        chat_id,
        TrustLevel::Untrusted.as_str(),
        identity_id,
        metadata,
    )
    .await?;

    let response = format!(
        "🔗 *Pairing initiated*\n\n\
         Your pairing token:\n`{}`\n\n\
         To complete pairing, use:\n`/pair {}`\n\n\
         This token expires in 15 minutes.",
        pairing.token, pairing.token
    );

    bot.send_message(msg.chat.id, response)
        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
        .await?;

    Ok(())
}

/// Complete pairing: verify the token and upgrade the session.
async fn complete_pairing(
    bot: &Bot,
    msg: &Message,
    chat_id: &str,
    token_str: &str,
    db_pool: &PgPool,
    event_stream: &Arc<EventStream>,
    policy_engine: &Arc<PolicyEngine>,
) -> anyhow::Result<()> {
    // Look up the existing session
    let Some(session) = channel_db::get_channel_session(db_pool, "telegram", chat_id).await? else {
        bot.send_message(
            msg.chat.id,
            "❌ No pending pairing found. Use /pair to start.",
        )
        .await?;
        return Ok(());
    };

    // Verify the token from metadata
    let stored_token = session
        .metadata
        .get("pairing_token")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if stored_token != token_str {
        bot.send_message(msg.chat.id, "❌ Invalid pairing token.")
            .await?;
        return Ok(());
    }

    // Check expiry
    let expires_str = session
        .metadata
        .get("pairing_expires_at")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if let Ok(expires_at) = chrono::DateTime::parse_from_rfc3339(expires_str) {
        if chrono::Utc::now() > expires_at {
            bot.send_message(
                msg.chat.id,
                "❌ Pairing token has expired. Use /pair to generate a new one.",
            )
            .await?;
            return Ok(());
        }
    }

    // Determine the requested trust level from metadata (stored during initiation)
    let requested_trust_str = session
        .metadata
        .get("requested_trust_level")
        .and_then(|v| v.as_str())
        .unwrap_or("conversational");

    let trust_level = requested_trust_str
        .parse::<TrustLevel>()
        .unwrap_or(TrustLevel::Conversational);

    // Owner trust level requires additional verification
    if trust_level == TrustLevel::Owner {
        // TODO: Implement owner verification (e.g., check against a pre-configured
        // owner list, require a secondary confirmation code, or verify via an
        // out-of-band channel). For now, owner pairing is allowed but logged.
        tracing::warn!(
            chat_id = %chat_id,
            session_id = %session.session_id,
            "Owner trust level granted via pairing — owner verification not yet implemented"
        );
    }

    let metadata = json!({
        "pairing_status": "confirmed",
        "paired_at": chrono::Utc::now().to_rfc3339(),
        "trust_level": trust_level.as_str(),
        "requested_trust_level": requested_trust_str,
    });

    channel_db::update_channel_session(db_pool, session.session_id, trust_level.as_str(), metadata)
        .await?;

    // Grant capabilities based on trust level
    for cap in trust_level.capabilities() {
        if let Err(e) = policy_engine
            .grant_capability(
                "channel",
                &session.session_id.to_string(),
                cap,
                None,
                None,
                None,
                None, // No expiry for conversational
                Some(event_stream),
                None,
                None,
                None,
            )
            .await
        {
            tracing::warn!(
                error = %e,
                capability = %cap,
                "Failed to grant capability during pairing"
            );
        }
    }

    // Emit pairing event
    event_stream.publish(
        EventEnvelope::new(
            EventLevel::Info,
            EventType::Custom(events::CHANNEL_PAIRED.to_string()),
            json!({
                "channel_type": "telegram",
                "channel_user_id": chat_id,
                "session_id": session.session_id.to_string(),
                "trust_level": trust_level.as_str(),
            }),
        )
        .with_actor_id(format!("telegram:{chat_id}")),
    );

    bot.send_message(
        msg.chat.id,
        format!(
            "✅ *Pairing complete!*\n\n\
             Trust level: `{}`\n\
             Capabilities: {}\n\n\
             You can now interact with Carnelian through this chat.",
            trust_level.as_str(),
            trust_level
                .capabilities()
                .iter()
                .map(|c| format!("`{c}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )
    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
    .await?;

    Ok(())
}
