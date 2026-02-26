//! WhatsApp pairing flow for the `/pair` command.
//!
//! Implements a token-based pairing flow:
//! 1. User sends `/pair` → generate token, store pending session
//! 2. User sends `/pair <token>` → verify token, confirm pairing
//! 3. Grant capabilities based on trust level

use std::sync::Arc;

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::EventStream;
use carnelian_core::policy::PolicyEngine;

use crate::db as channel_db;
use crate::events;
use crate::types::{PairingRequest, TrustLevel, ChannelType};

use super::WhatsAppAdapter;

/// Handle the `/pair` command.
///
/// Syntax:
/// - `/pair` — generate a new pairing token (default: conversational trust)
/// - `/pair <trust_level>` — generate token with requested trust level
/// - `/pair <token>` — verify and complete pairing
pub async fn handle_pair(
    from: &str,
    args: &str,
    adapter: &WhatsAppAdapter,
    db_pool: &PgPool,
    event_stream: &Arc<EventStream>,
    policy_engine: &Arc<PolicyEngine>,
    identity_id: Option<Uuid>,
) -> anyhow::Result<()> {
    let trimmed = args.trim();

    if trimmed.is_empty() {
        initiate_pairing(from, adapter, db_pool, identity_id, None).await
    } else if let Ok(requested_trust) = trimmed.parse::<TrustLevel>() {
        // Argument is a trust level name — initiate with that level
        initiate_pairing(from, adapter, db_pool, identity_id, Some(requested_trust)).await
    } else {
        // Argument is a pairing token — verify and complete
        complete_pairing(
            from,
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
    from: &str,
    adapter: &WhatsAppAdapter,
    db_pool: &PgPool,
    identity_id: Option<Uuid>,
    requested_trust_level: Option<TrustLevel>,
) -> anyhow::Result<()> {
    let pairing = PairingRequest::new(
        ChannelType::Whatsapp,
        from.to_string(),
        requested_trust_level,
    );

    let metadata = json!({
        "pairing_token": pairing.token.to_string(),
        "pairing_status": "pending",
        "pairing_expires_at": pairing.expires_at.to_rfc3339(),
        "requested_trust_level": pairing.requested_trust_level.as_str(),
        "whatsapp_phone_number_id": adapter.phone_number_id(),
    });

    let _session = channel_db::upsert_channel_session(
        db_pool,
        "whatsapp",
        from,
        TrustLevel::Untrusted.as_str(),
        identity_id,
        metadata,
    )
    .await?;

    let response = format!(
        "🔗 *Pairing initiated*

\
         Your pairing token: `{}`

\
         To complete pairing, use: `/pair {}`

\
         This token expires in 15 minutes.",
        pairing.token, pairing.token
    );

    adapter.send_message(from, &response).await?;
    Ok(())
}

/// Complete pairing: verify the token and upgrade the session.
async fn complete_pairing(
    from: &str,
    token_str: &str,
    adapter: &WhatsAppAdapter,
    db_pool: &PgPool,
    event_stream: &Arc<EventStream>,
    policy_engine: &Arc<PolicyEngine>,
) -> anyhow::Result<()> {
    let session = match channel_db::get_channel_session(db_pool, "whatsapp", from).await? {
        Some(s) => s,
        None => {
            adapter
                .send_message(from, "❌ No pending pairing found. Use `/pair` to start.")
                .await?;
            return Ok(());
        }
    };

    // Verify token
    let stored_token = session
        .metadata
        .get("pairing_token")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if stored_token != token_str {
        adapter
            .send_message(from, "❌ Invalid pairing token.")
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
                .send_message(from, "❌ Pairing token has expired. Use `/pair` to generate a new one.")
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
            channel_user_id = %from,
            session_id = %session.session_id,
            "Owner trust level granted via WhatsApp pairing — owner verification not yet implemented"
        );
    }

    let metadata = json!({
        "pairing_status": "confirmed",
        "paired_at": chrono::Utc::now().to_rfc3339(),
        "trust_level": trust_level.as_str(),
        "requested_trust_level": requested_trust_str,
        "whatsapp_phone_number_id": adapter.phone_number_id(),
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
                "Failed to grant capability during WhatsApp pairing"
            );
        }
    }

    // Emit pairing event
    event_stream.publish(
        EventEnvelope::new(
            EventLevel::Info,
            EventType::Custom(events::CHANNEL_PAIRED.to_string()),
            json!({
                "channel_type": "whatsapp",
                "channel_user_id": from,
                "session_id": session.session_id.to_string(),
                "trust_level": trust_level.as_str(),
            }),
        )
        .with_actor_id(format!("whatsapp:{from}")),
    );

    let caps_str = trust_level
        .capabilities()
        .iter()
        .map(|c| format!("`{c}`"))
        .collect::<Vec<_>>()
        .join(", ");

    let confirmation = format!(
        "✅ *Pairing complete!*

\
         Trust level: `{}`
\
         Capabilities: {}

\
         You can now interact with Carnelian through WhatsApp.",
        trust_level.as_str(),
        caps_str
    );

    adapter.send_message(from, &confirmation).await?;
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
        let req = PairingRequest::new(
            ChannelType::Whatsapp,
            "15551234567".to_string(),
            None,
        );

        assert_eq!(req.requested_trust_level, TrustLevel::Conversational);
        assert!(!req.is_expired());
    }
}
