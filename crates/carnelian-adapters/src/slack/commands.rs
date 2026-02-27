//! Slack command parsing and dispatch.
//!
//! Handles slash command text from `/carnelian` invocations and provides
//! the response logic for pairing, status, and help commands.

use std::sync::Arc;

use sqlx::PgPool;

use carnelian_core::EventStream;
use carnelian_core::policy::PolicyEngine;

use crate::ChannelAdapter;
use crate::db as channel_db;

use super::SlackAdapter;

/// Dispatch a command from the message body.
///
/// The `body` parameter is the text after `/carnelian ` (e.g., "pair", "status").
/// Returns `Ok(true)` if a command was recognized and dispatched,
/// `Ok(false)` if not a recognized command.
pub async fn dispatch_command(
    body: &str,
    channel_id: &str,
    user_id: &str,
    adapter: &SlackAdapter,
    db_pool: &PgPool,
    event_stream: &Arc<EventStream>,
    policy_engine: &Arc<PolicyEngine>,
) -> anyhow::Result<bool> {
    let trimmed = body.trim();
    let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    let command = parts[0].to_lowercase();
    let args = parts.get(1).copied().unwrap_or("");

    match command.as_str() {
        "pair" | "/carnelian pair" => {
            super::pairing::handle_pair(
                channel_id,
                user_id,
                args,
                adapter,
                db_pool,
                event_stream,
                policy_engine,
                adapter.config().identity_id,
            )
            .await?;
            Ok(true)
        }
        "status" | "/carnelian status" => {
            handle_status(channel_id, adapter, db_pool).await?;
            Ok(true)
        }
        "help" | "/carnelian help" => {
            handle_help(channel_id, adapter).await?;
            Ok(true)
        }
        "" => {
            // Empty command after /carnelian - show help
            handle_help(channel_id, adapter).await?;
            Ok(true)
        }
        _ => Ok(false), // Not a recognized command
    }
}

/// Handle `help` — list available commands.
async fn handle_help(channel_id: &str, adapter: &SlackAdapter) -> anyhow::Result<()> {
    let help_text = "📋 *Carnelian Commands*\n\n\
`/carnelian pair` — Pair this channel with Carnelian\n\
`/carnelian pair <trust_level>` — Pair with specific trust level\n\
`/carnelian pair <token>` — Complete pairing with token\n\
`/carnelian status` — Show session status and trust level\n\
`/carnelian help` — Show this help message\n\n\
Trust levels: untrusted, conversational, owner";

    adapter.send_message(channel_id, help_text).await?;
    Ok(())
}

/// Handle `status` — show session info.
async fn handle_status(
    channel_id: &str,
    adapter: &SlackAdapter,
    db_pool: &PgPool,
) -> anyhow::Result<()> {
    let session = channel_db::get_channel_session(db_pool, "slack", channel_id).await?;

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
        None => "❌ No session found. Use `/carnelian pair` to connect.".to_string(),
    };

    adapter.send_message(channel_id, &text).await?;
    Ok(())
}
