//! `WhatsApp` command parsing and dispatch.
//!
//! Commands are plain text parsed from message body (no bot framework).

use std::sync::Arc;

use sqlx::PgPool;

use carnelian_core::policy::PolicyEngine;
use carnelian_core::EventStream;

use crate::db as channel_db;
use crate::ChannelAdapter;

use super::WhatsAppAdapter;

/// Dispatch a command from the message body.
///
/// Returns `Ok(true)` if a command was recognized and dispatched,
/// `Ok(false)` if not a recognized command.
///
/// # Errors
///
/// Returns an error if command execution fails.
pub async fn dispatch_command(
    body: &str,
    from: &str,
    adapter: &WhatsAppAdapter,
    db_pool: &PgPool,
    event_stream: &Arc<EventStream>,
    policy_engine: &Arc<PolicyEngine>,
) -> anyhow::Result<bool> {
    let parts: Vec<&str> = body.splitn(2, ' ').collect();
    let command = parts[0].to_lowercase();
    let args = parts.get(1).copied().unwrap_or("");

    match command.as_str() {
        "/pair" => {
            super::pairing::handle_pair(
                from,
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
        "/status" => {
            handle_status(from, adapter, db_pool).await?;
            Ok(true)
        }
        "/help" => {
            handle_help(from, adapter).await?;
            Ok(true)
        }
        _ => Ok(false), // Not a recognized command
    }
}

/// Handle `/help` — list available commands.
async fn handle_help(from: &str, adapter: &WhatsAppAdapter) -> anyhow::Result<()> {
    let help_text = "📋 *Available Commands*

\
`/pair` — Pair this number with Carnelian
\
`/pair <trust_level>` — Pair with specific trust level
\
`/pair <token>` — Complete pairing with token
\
`/status` — Show session status and trust level
\
`/help` — Show this help message

\
Trust levels: untrusted, conversational, owner";

    adapter.send_message(from, help_text).await?;
    Ok(())
}

/// Handle `/status` — show session info.
async fn handle_status(
    from: &str,
    adapter: &WhatsAppAdapter,
    db_pool: &PgPool,
) -> anyhow::Result<()> {
    let session = channel_db::get_channel_session(db_pool, "whatsapp", from).await?;

    let text = match session {
        Some(s) => {
            let paired = s
                .metadata
                .get("pairing_status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            format!(
                "📊 *Session Status*

\
                 Session ID: `{}`
\
                 Trust level: `{}`
\
                 Pairing: `{}`
\
                 Created: {}
\
                 Last seen: {}",
                s.session_id,
                s.trust_level,
                paired,
                s.created_at.format("%Y-%m-%d %H:%M UTC"),
                s.last_seen_at.format("%Y-%m-%d %H:%M UTC"),
            )
        }
        None => "❌ No session found. Use `/pair` to connect.".to_string(),
    };

    adapter.send_message(from, &text).await?;
    Ok(())
}
