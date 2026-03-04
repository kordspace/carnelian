//! Slack pairing flow for the `/carnelian pair` command.
//!
//! Implements a token-based pairing flow:
//! 1. User sends `/carnelian pair` → generate token, store pending session
//! 2. User sends `/carnelian pair <token>` → verify token, confirm pairing
//! 3. Grant capabilities based on trust level

use std::sync::Arc;

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::policy::PolicyEngine;
use carnelian_core::EventStream;

use crate::db as channel_db;
use crate::events;
use crate::types::{ChannelType, PairingRequest, TrustLevel};
use crate::ChannelAdapter;

use super::SlackAdapter;

/// Handle the `/carnelian pair` command.
///
/// Syntax:
/// - `/carnelian pair` — generate a new pairing token (default: conversational trust)
/// - `/carnelian pair <trust_level>` — generate token with requested trust level
/// - `/carnelian pair <token>` — verify and complete pairing
///
/// # Errors
///
/// Returns an error if database operations or message sending fails.
#[allow(clippy::too_many_arguments)]
pub async fn handle_pair(
    channel_id: &str,
    user_id: &str,
    args: &str,
    adapter: &SlackAdapter,
    db_pool: &PgPool,
    event_stream: &Arc<EventStream>,
    policy_engine: &Arc<PolicyEngine>,
    identity_id: Option<Uuid>,
) -> anyhow::Result<()> {
    let trimmed = args.trim();

    if trimmed.is_empty() {
        initiate_pairing(channel_id, user_id, adapter, db_pool, identity_id, None).await
    } else if let Ok(requested_trust) = trimmed.parse::<TrustLevel>() {
        // Argument is a trust level name — initiate with that level
        initiate_pairing(
            channel_id,
            user_id,
            adapter,
            db_pool,
            identity_id,
            Some(requested_trust),
        )
        .await
    } else {
        // Argument is a pairing token — verify and complete
        complete_pairing(
            channel_id,
            trimmed,
            adapter,
            db_pool,
            event_stream,
            policy_engine,
        )
        .await
    }
}

/// Initiate pairing: create a pending channel session with a pairing token.
async fn initiate_pairing(
    channel_id: &str,
    user_id: &str,
    adapter: &SlackAdapter,
    db_pool: &PgPool,
    identity_id: Option<Uuid>,
    requested_trust_level: Option<TrustLevel>,
) -> anyhow::Result<()> {
    let pairing = PairingRequest::new(
        ChannelType::Slack,
        channel_id.to_string(),
        requested_trust_level,
    );

    let metadata = json!({
        "pairing_token": pairing.token.to_string(),
        "pairing_status": "pending",
        "pairing_expires_at": pairing.expires_at.to_rfc3339(),
        "requested_trust_level": pairing.requested_trust_level.as_str(),
        "slack_user_id": user_id,
    });

    let _session = channel_db::upsert_channel_session(
        db_pool,
        "slack",
        channel_id,
        TrustLevel::Untrusted.as_str(),
        identity_id,
        metadata,
    )
    .await?;

    let response = format!(
        "🔗 *Pairing initiated*\n\n\
         Your pairing token: `{}`\n\n\
         To complete pairing, use: `/carnelian pair {}`\n\n\
         This token expires in 15 minutes.",
        pairing.token, pairing.token
    );

    adapter.send_message(channel_id, &response).await?;
    Ok(())
}

/// Complete pairing: verify the token and upgrade the session.
#[allow(clippy::too_many_lines)]
async fn complete_pairing(
    channel_id: &str,
    token_str: &str,
    adapter: &SlackAdapter,
    db_pool: &PgPool,
    event_stream: &Arc<EventStream>,
    policy_engine: &Arc<PolicyEngine>,
) -> anyhow::Result<()> {
    let Some(session) = channel_db::get_channel_session(db_pool, "slack", channel_id).await? else {
        adapter
            .send_message(
                channel_id,
                "❌ No pending pairing found. Use `/carnelian pair` to start.",
            )
            .await?;
        return Ok(());
    };

    // Verify token
    let stored_token = session
        .metadata
        .get("pairing_token")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if stored_token != token_str {
        adapter
            .send_message(channel_id, "❌ Invalid pairing token.")
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
            adapter
                .send_message(
                    channel_id,
                    "❌ Pairing token has expired. Use `/carnelian pair` to generate a new one.",
                )
                .await?;
            return Ok(());
        }
    }

    // Determine the requested trust level from metadata
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
        tracing::warn!(
            channel_id = %channel_id,
            session_id = %session.session_id,
            "Owner trust level granted via Slack pairing — owner verification not yet implemented"
        );
    }

    let metadata = json!({
        "pairing_status": "confirmed",
        "paired_at": chrono::Utc::now().to_rfc3339(),
        "trust_level": trust_level.as_str(),
        "requested_trust_level": requested_trust_str,
        "slack_user_id": session.metadata.get("slack_user_id").cloned(),
    });

    channel_db::update_channel_session(db_pool, session.session_id, trust_level.as_str(), metadata)
        .await?;

    // Grant capabilities
    for cap in trust_level.capabilities() {
        if let Err(e) = policy_engine
            .grant_capability(
                "channel",
                &session.session_id.to_string(),
                cap,
                None,
                None,
                None,
                None,
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
                "Failed to grant capability during Slack pairing"
            );
        }
    }

    // Emit pairing event
    event_stream.publish(
        EventEnvelope::new(
            EventLevel::Info,
            EventType::Custom(events::CHANNEL_PAIRED.to_string()),
            json!({
                "channel_type": "slack",
                "channel_user_id": channel_id,
                "session_id": session.session_id.to_string(),
                "trust_level": trust_level.as_str(),
            }),
        )
        .with_actor_id(format!("slack:{channel_id}")),
    );

    let caps_str = trust_level
        .capabilities()
        .iter()
        .map(|c| format!("`{c}`"))
        .collect::<Vec<_>>()
        .join(", ");

    let confirmation = format!(
        "✅ *Pairing complete!*\n\n\
         Trust level: `{}`\n\
         Capabilities: {}\n\n\
         You can now interact with Carnelian through this Slack channel.",
        trust_level.as_str(),
        caps_str
    );

    adapter.send_message(channel_id, &confirmation).await?;
    Ok(())
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pairing_request_defaults_to_conversational() {
        let req = PairingRequest::new(ChannelType::Slack, "C12345678".to_string(), None);

        assert_eq!(req.requested_trust_level, TrustLevel::Conversational);
        assert!(!req.is_expired());
    }
}
