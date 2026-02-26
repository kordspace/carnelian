//! WhatsApp webhook handlers.
//!
//! Handles incoming webhook verification (Meta hub challenge) and message
//! payloads, applying rate limiting, spam detection, and capability checks
//! before forwarding to the session manager.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::EventStream;
use carnelian_core::policy::PolicyEngine;
use carnelian_core::session::SessionManager;

use crate::db as channel_db;
use crate::events;
use crate::rate_limiter::RateLimiter;
use crate::spam_detector::SpamDetector;
use crate::types::{TrustLevel, ChannelType};

use super::WhatsAppAdapter;

// =============================================================================
// DESERIALIZATION TYPES
// =============================================================================

/// Query parameters for webhook verification request.
#[derive(Debug, Deserialize)]
pub struct WhatsAppVerifyQuery {
    #[serde(rename = "hub.mode")]
    pub hub_mode: String,
    #[serde(rename = "hub.verify_token")]
    pub hub_verify_token: String,
    #[serde(rename = "hub.challenge")]
    pub hub_challenge: String,
}

/// Top-level webhook payload from Meta.
#[derive(Debug, Deserialize)]
pub struct WhatsAppWebhookPayload {
    pub object: String,
    pub entry: Vec<WhatsAppEntry>,
}

/// Individual entry in the webhook payload.
#[derive(Debug, Deserialize)]
pub struct WhatsAppEntry {
    pub changes: Vec<WhatsAppChange>,
}

/// Change object containing the value.
#[derive(Debug, Deserialize)]
pub struct WhatsAppChange {
    pub value: WhatsAppValue,
}

/// Value containing messages and metadata.
#[derive(Debug, Deserialize)]
pub struct WhatsAppValue {
    pub messages: Option<Vec<WhatsAppMessage>>,
    pub metadata: WhatsAppMetadata,
}

/// Individual WhatsApp message.
#[derive(Debug, Deserialize)]
pub struct WhatsAppMessage {
    pub id: String,
    pub from: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub text: Option<WhatsAppText>,
    pub timestamp: String,
}

/// Text content of a message.
#[derive(Debug, Deserialize)]
pub struct WhatsAppText {
    pub body: String,
}

/// Metadata about the WhatsApp business account.
#[derive(Debug, Deserialize)]
pub struct WhatsAppMetadata {
    pub phone_number_id: String,
    pub display_phone_number: String,
}

// =============================================================================
// WEBHOOK VERIFICATION
// =============================================================================

/// Handle webhook verification request from Meta.
///
/// Verifies the hub mode is "subscribe" and the verify token matches.
/// Returns the challenge string on success for the HTTP response.
pub fn handle_verification(
    query: &WhatsAppVerifyQuery,
    verify_token: &str,
    event_stream: &Arc<EventStream>,
) -> anyhow::Result<String> {
    if query.hub_mode != "subscribe" {
        anyhow::bail!(
            "Invalid hub.mode: expected 'subscribe', got '{}'",
            query.hub_mode
        );
    }

    if query.hub_verify_token != verify_token {
        anyhow::bail!(
            "Invalid verify token: expected '{}', got '{}'",
            verify_token,
            query.hub_verify_token
        );
    }

    // Emit webhook verified event
    event_stream.publish(EventEnvelope::new(
        EventLevel::Info,
        EventType::Custom(events::WHATSAPP_WEBHOOK_VERIFIED.to_string()),
        json!({
            "channel_type": "whatsapp",
        }),
    ));

    Ok(query.hub_challenge.clone())
}

// =============================================================================
// INBOUND MESSAGE HANDLING
// =============================================================================

/// Handle inbound webhook payload from WhatsApp.
///
/// Processes each message in the payload, applying rate limiting,
/// spam detection, capability checks, and session management.
pub async fn handle_inbound(
    payload: &WhatsAppWebhookPayload,
    adapter: &WhatsAppAdapter,
) -> anyhow::Result<()> {
    for entry in &payload.entry {
        for change in &entry.changes {
            if let Some(messages) = &change.value.messages {
                for message in messages {
                    if let Err(e) = process_message(message, adapter).await {
                        tracing::warn!(
                            error = %e,
                            message_id = %message.id,
                            from = %message.from,
                            "Failed to process WhatsApp message"
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

/// Process a single WhatsApp message.
async fn process_message(
    message: &WhatsAppMessage,
    adapter: &WhatsAppAdapter,
) -> anyhow::Result<()> {
    // Only process text messages
    let text = match &message.text {
        Some(t) => &t.body,
        None => {
            tracing::debug!(
                message_id = %message.id,
                message_type = %message.type_,
                "Skipping non-text WhatsApp message"
            );
            return Ok(());
        }
    };

    let from = &message.from;
    let correlation_id = Uuid::new_v4();

    // Check for command prefix
    if text.starts_with('/') {
        super::commands::dispatch_command(
            text,
            from,
            adapter,
            &adapter.db_pool,
            &adapter.event_stream,
            &adapter.policy_engine,
        )
        .await?;
        return Ok(());
    }

    // 1. Session upsert
    let metadata = json!({
        "whatsapp_message_id": message.id,
        "whatsapp_timestamp": message.timestamp,
        "whatsapp_phone_number_id": adapter.phone_number_id(),
    });

    let session = channel_db::upsert_channel_session(
        &adapter.db_pool,
        "whatsapp",
        from,
        TrustLevel::Untrusted.as_str(),
        adapter.config().identity_id,
        metadata,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to upsert channel session: {e}"))?;

    let trust_level = session.parsed_trust_level();

    // 2. Rate limit check (synchronous, no await)
    if let Err(e) = adapter
        .rate_limiter
        .check_rate_limit("whatsapp", from, trust_level)
    {
        tracing::warn!(
            error = %e,
            channel_user_id = %from,
            "Rate limit exceeded"
        );
        adapter
            .send_message(from, "⏳ Rate limit exceeded. Please wait a moment before sending more messages.")
            .await?;
        return Ok(());
    }

    // 3. Spam detection (synchronous, no await)
    let spam_score = adapter
        .spam_detector
        .update_score("whatsapp", from, text);
    if adapter.spam_detector.is_spam(spam_score) {
        tracing::info!(
            channel_user_id = %from,
            spam_score = %spam_score,
            "Spam detected, dropping message"
        );
        return Ok(());
    }

    // 4. Capability check
    let has_receive = adapter
        .policy_engine
        .check_capability(
            "channel",
            &session.session_id.to_string(),
            "channel.message.receive",
            Some(&adapter.event_stream),
        )
        .await
        .unwrap_or(false);

    if !has_receive {
        tracing::warn!(
            channel_user_id = %from,
            "Capability check failed"
        );
        adapter
            .send_message(from, "🔒 You don't have permission to send messages. Use /pair to connect.")
            .await?;
        return Ok(());
    }

    // 5. Session manager integration
    let stable_identity = adapter
        .config()
        .identity_id
        .unwrap_or(session.session_id);

    let session_key = format!("agent:{stable_identity}:whatsapp:user:{from}");

    // Create session if needed and append message
    let conv_session = adapter
        .session_manager
        .create_session(&session_key)
        .await;

    if let Ok(conv) = conv_session {
        if let Err(e) = adapter
            .session_manager
            .append_message(
                conv.session_id,
                "user",
                text.to_string(),
                Some(correlation_id),
                None,
                None,
                None,
                None,
                None,
            )
            .await
        {
            tracing::warn!(error = %e, "Failed to append WhatsApp message to session");
        }
    }

    // 6. Emit message received event
    adapter.event_stream.publish(EventEnvelope::new(
        EventLevel::Info,
        EventType::Custom(events::WHATSAPP_MESSAGE_RECEIVED.to_string()),
        json!({
            "channel_type": "whatsapp",
            "channel_user_id": from,
            "trust_level": trust_level.as_str(),
            "message_length": text.len(),
            "correlation_id": correlation_id.to_string(),
        }),
    ));

    // 7. Touch session
    channel_db::touch_channel_session(&adapter.db_pool, session.session_id)
        .await
        .ok();

    Ok(())
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_verification_success() {
        let event_stream = Arc::new(EventStream::new());

        let query = WhatsAppVerifyQuery {
            hub_mode: "subscribe".to_string(),
            hub_verify_token: "my_verify_token".to_string(),
            hub_challenge: "abc123".to_string(),
        };

        let result = handle_verification(&query, "my_verify_token", &event_stream);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "abc123");
    }

    #[test]
    fn test_webhook_verification_wrong_token() {
        let event_stream = Arc::new(EventStream::new());

        let query = WhatsAppVerifyQuery {
            hub_mode: "subscribe".to_string(),
            hub_verify_token: "wrong_token".to_string(),
            hub_challenge: "abc123".to_string(),
        };

        let result = handle_verification(&query, "my_verify_token", &event_stream);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid verify token"));
    }
}
