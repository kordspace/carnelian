//! Telegram bot command definitions and dispatch.

use std::sync::Arc;

use sqlx::PgPool;
use teloxide::macros::BotCommands;
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands as BotCommandsTrait;

use carnelian_core::policy::PolicyEngine;
use carnelian_core::EventStream;

use crate::db as channel_db;
use crate::types::ChannelConfig;

/// Supported bot commands.
#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Available commands:")]
pub enum Command {
    #[command(description = "Start interacting with Carnelian")]
    Start,
    #[command(description = "Show available commands")]
    Help,
    #[command(description = "Pair this chat with Carnelian (use /pair or /pair <token>)")]
    Pair(String),
    #[command(description = "Show session status and trust level")]
    Status,
    #[command(description = "Unpair this chat from Carnelian")]
    Unpair,
}

/// Dispatch a parsed command to the appropriate handler.
///
/// # Errors
///
/// Returns an error if command execution fails.
pub async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    event_stream: Arc<EventStream>,
    policy_engine: Arc<PolicyEngine>,
    db_pool: PgPool,
    config: ChannelConfig,
) -> anyhow::Result<()> {
    match cmd {
        Command::Start => handle_start(&bot, &msg).await,
        Command::Help => handle_help(&bot, &msg).await,
        Command::Pair(args) => {
            super::pairing::handle_pair(
                &bot,
                &msg,
                &args,
                &db_pool,
                &event_stream,
                &policy_engine,
                config.identity_id,
            )
            .await
        }
        Command::Status => handle_status(&bot, &msg, &db_pool).await,
        Command::Unpair => handle_unpair(&bot, &msg, &db_pool, &event_stream).await,
    }
}

/// Handle `/start` — welcome message.
async fn handle_start(bot: &Bot, msg: &Message) -> anyhow::Result<()> {
    bot.send_message(
        msg.chat.id,
        "🔥 *Welcome to Carnelian OS*\n\n\
         I'm your AI agent interface. Use /pair to connect this chat.\n\n\
         Commands:\n\
         /pair — Pair this chat\n\
         /status — Show session info\n\
         /help — Show all commands\n\
         /unpair — Disconnect this chat",
    )
    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
    .await?;
    Ok(())
}

/// Handle `/help` — list available commands.
async fn handle_help(bot: &Bot, msg: &Message) -> anyhow::Result<()> {
    bot.send_message(msg.chat.id, Command::descriptions().to_string())
        .await?;
    Ok(())
}

/// Handle `/status` — show session info, trust level, message count.
async fn handle_status(bot: &Bot, msg: &Message, db_pool: &PgPool) -> anyhow::Result<()> {
    let chat_id = msg.chat.id.0.to_string();

    let session = channel_db::get_channel_session(db_pool, "telegram", &chat_id).await?;

    let text = match session {
        Some(s) => {
            let paired = s
                .metadata
                .get("pairing_status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            format!(
                "📊 *Session Status*\n\n\
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
        None => "❌ No session found. Use /pair to connect.".to_string(),
    };

    bot.send_message(msg.chat.id, text)
        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
        .await?;

    Ok(())
}

/// Handle `/unpair` — revoke channel session.
async fn handle_unpair(
    bot: &Bot,
    msg: &Message,
    db_pool: &PgPool,
    event_stream: &Arc<EventStream>,
) -> anyhow::Result<()> {
    let chat_id = msg.chat.id.0.to_string();

    let session = channel_db::get_channel_session(db_pool, "telegram", &chat_id).await?;

    match session {
        Some(s) => {
            channel_db::delete_channel_session(db_pool, s.session_id).await?;

            // Emit unpaired event
            event_stream.publish(
                carnelian_common::types::EventEnvelope::new(
                    carnelian_common::types::EventLevel::Info,
                    carnelian_common::types::EventType::Custom(
                        crate::events::CHANNEL_UNPAIRED.to_string(),
                    ),
                    serde_json::json!({
                        "channel_type": "telegram",
                        "channel_user_id": chat_id,
                        "session_id": s.session_id.to_string(),
                    }),
                )
                .with_actor_id(format!("telegram:{chat_id}")),
            );

            bot.send_message(msg.chat.id, "✅ Chat unpaired. Use /pair to reconnect.")
                .await?;
        }
        None => {
            bot.send_message(msg.chat.id, "ℹ️ This chat is not currently paired.")
                .await?;
        }
    }

    Ok(())
}
