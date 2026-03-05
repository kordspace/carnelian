//! Discord command registration and prefix-based command handling.

use std::sync::Arc;

use serenity::all::{Context, Message};
use sqlx::PgPool;
use uuid::Uuid;

use carnelian_core::policy::PolicyEngine;
use carnelian_core::EventStream;

use crate::db as channel_db;

/// Register Discord slash commands globally.
///
/// Note: Slash command registration can take up to an hour to propagate.
/// For development, use guild-specific registration instead.
///
/// # Errors
///
/// Currently infallible, but returns `Result` for future extensibility.
pub fn register_commands(ctx: &Context) -> anyhow::Result<()> {
    // For now, we use prefix commands (!pair, !status, etc.)
    // Slash command registration can be added later for production use.
    tracing::debug!("Discord slash command registration placeholder (using prefix commands)");
    let _ = ctx;
    Ok(())
}

/// Handle prefix-based commands (e.g., `!pair`, `!status`, `!help`, `!unpair`).
///
/// # Errors
///
/// Returns an error if command execution fails.
pub async fn handle_prefix_command(
    ctx: &Context,
    msg: &Message,
    content: &str,
    db_pool: &PgPool,
    event_stream: &Arc<EventStream>,
    policy_engine: &Arc<PolicyEngine>,
    identity_id: Option<Uuid>,
) -> anyhow::Result<()> {
    let parts: Vec<&str> = content.splitn(2, ' ').collect();
    let command = parts[0].to_lowercase();
    let args = parts.get(1).copied().unwrap_or("");

    match command.as_str() {
        "!start" | "!hello" => handle_start(ctx, msg).await,
        "!help" => handle_help(ctx, msg).await,
        "!pair" => {
            super::pairing::handle_pair(
                ctx,
                msg,
                args,
                db_pool,
                event_stream,
                policy_engine,
                identity_id,
            )
            .await
        }
        "!status" => handle_status(ctx, msg, db_pool).await,
        "!unpair" => handle_unpair(ctx, msg, db_pool, event_stream).await,
        _ => Ok(()), // Unknown command, ignore
    }
}

/// Handle `!start` — welcome message.
async fn handle_start(ctx: &Context, msg: &Message) -> anyhow::Result<()> {
    msg.channel_id
        .say(
            &ctx.http,
            "🔥 **Welcome to Carnelian OS**\n\n\
             I'm your AI agent interface. Use `!pair` to connect this channel.\n\n\
             Commands:\n\
             `!pair` — Pair this channel\n\
             `!status` — Show session info\n\
             `!help` — Show all commands\n\
             `!unpair` — Disconnect this channel",
        )
        .await?;
    Ok(())
}

/// Handle `!help` — list available commands.
async fn handle_help(ctx: &Context, msg: &Message) -> anyhow::Result<()> {
    msg.channel_id
        .say(
            &ctx.http,
            "📋 **Available Commands**\n\n\
             `!start` — Start interacting with Carnelian\n\
             `!pair` — Pair this channel (use `!pair` or `!pair <token>`)\n\
             `!status` — Show session status and trust level\n\
             `!unpair` — Unpair this channel from Carnelian\n\
             `!help` — Show this help message",
        )
        .await?;
    Ok(())
}

/// Handle `!status` — show session info.
async fn handle_status(ctx: &Context, msg: &Message, db_pool: &PgPool) -> anyhow::Result<()> {
    let channel_id = msg.channel_id.to_string();

    let session = channel_db::get_channel_session(db_pool, "discord", &channel_id).await?;

    let text = match session {
        Some(s) => {
            let paired = s
                .metadata
                .get("pairing_status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            format!(
                "📊 **Session Status**\n\n\
                 Session ID: `{}`\n\
                 Trust level: `{}`\n\
                 Pairing: `{}`\n\
                 Created: {}\n\
                 Last seen: {}",
                s.session_id,
                s.trust_level,
                paired,
                s.created_at.format("%Y-%m-%d %H:%M UTC"),
                s.last_seen_at.format("%Y-%m-%d %H:%M UTC"),
            )
        }
        None => "❌ No session found. Use `!pair` to connect.".to_string(),
    };

    msg.channel_id.say(&ctx.http, text).await?;
    Ok(())
}

/// Handle `!unpair` — revoke channel session.
async fn handle_unpair(
    ctx: &Context,
    msg: &Message,
    db_pool: &PgPool,
    event_stream: &Arc<EventStream>,
) -> anyhow::Result<()> {
    let channel_id = msg.channel_id.to_string();

    let session = channel_db::get_channel_session(db_pool, "discord", &channel_id).await?;

    match session {
        Some(s) => {
            channel_db::delete_channel_session(db_pool, s.session_id).await?;

            event_stream.publish(
                carnelian_common::types::EventEnvelope::new(
                    carnelian_common::types::EventLevel::Info,
                    carnelian_common::types::EventType::Custom(
                        crate::events::CHANNEL_UNPAIRED.to_string(),
                    ),
                    serde_json::json!({
                        "channel_type": "discord",
                        "channel_user_id": channel_id,
                        "session_id": s.session_id.to_string(),
                    }),
                )
                .with_actor_id(format!("discord:{channel_id}")),
            );

            msg.channel_id
                .say(&ctx.http, "✅ Channel unpaired. Use `!pair` to reconnect.")
                .await?;
        }
        None => {
            msg.channel_id
                .say(&ctx.http, "ℹ️ This channel is not currently paired.")
                .await?;
        }
    }

    Ok(())
}
