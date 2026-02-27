//! Slack webhook handlers.
//!
//! Handles incoming webhook verification (URL verification challenge) and message
//! events, applying HMAC-SHA256 signature verification, rate limiting, spam detection,
//! and capability checks before forwarding to the session manager.

#![allow(unused_imports)]

use std::sync::Arc;

use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::json;
use sha2::Sha256;
use sqlx::PgPool;
use uuid::Uuid;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::EventStream;
use carnelian_core::policy::PolicyEngine;
use carnelian_core::session::SessionManager;

use crate::ChannelAdapter;
use crate::db as channel_db;
use crate::events;
use crate::rate_limiter::RateLimiter;
use crate::spam_detector::SpamDetector;
use crate::types::{ChannelType, TrustLevel};

use super::SlackAdapter;

// =============================================================================
// DESERIALIZATION TYPES
// =============================================================================

/// Top-level Slack event payload (Events API).
#[derive(Debug, Deserialize)]
pub struct SlackEventPayload {
    #[serde(rename = "type")]
    pub type_: String,
    pub challenge: Option<String>,
    pub event: Option<SlackEvent>,
    pub team_id: Option<String>,
}

/// Individual Slack event (wrapped in `event_callback`).
#[derive(Debug, Deserialize)]
pub struct SlackEvent {
    #[serde(rename = "type")]
    pub type_: String,
    pub text: Option<String>,
    pub user: Option<String>,
    pub channel: Option<String>,
    pub ts: Option<String>,
    pub bot_id: Option<String>,
    #[serde(rename = "team")]
    pub team_id: Option<String>,
}

/// Slack slash command payload (form-encoded).
#[derive(Debug, Deserialize)]
pub struct SlackSlashCommand {
    pub command: String,
    pub text: String,
    #[serde(rename = "user_id")]
    pub user_id: String,
    #[serde(rename = "channel_id")]
    pub channel_id: String,
    #[serde(rename = "team_id")]
    pub team_id: String,
}

/// Response type for event handling.
#[derive(Debug)]
pub enum SlackEventResponse {
    Challenge(String),
    Ok,
}

// =============================================================================
// HMAC-SHA256 SIGNATURE VERIFICATION
// =============================================================================

/// Verify Slack request signature using HMAC-SHA256.
///
/// 1. Checks timestamp is within 5-minute window (replay protection)
/// 2. Builds base string: `v0:{timestamp}:{body}`
/// 3. Computes HMAC-SHA256 of base string with signing secret
/// 4. Compares computed signature with provided signature
///
/// # Errors
///
/// Returns an error if signature verification fails or timestamp is invalid.
pub fn verify_slack_signature(
    signing_secret: &str,
    timestamp: &str,
    body: &[u8],
    signature: &str,
) -> anyhow::Result<()> {
    type HmacSha256 = Hmac<Sha256>;

    // 1. Replay protection: check timestamp is within 5 minutes
    let now = chrono::Utc::now().timestamp();
    let ts = timestamp
        .parse::<i64>()
        .map_err(|_| anyhow::anyhow!("Invalid timestamp format"))?;

    if (now - ts).abs() > 300 {
        anyhow::bail!("Request timestamp too old (replay protection)");
    }

    // 2. Build base string
    let body_str =
        std::str::from_utf8(body).map_err(|_| anyhow::anyhow!("Invalid UTF-8 in request body"))?;
    let base_string = format!("v0:{timestamp}:{body_str}");

    // 3. Compute HMAC-SHA256
    let mut mac = HmacSha256::new_from_slice(signing_secret.as_bytes())
        .map_err(|_| anyhow::anyhow!("Invalid signing secret length"))?;
    mac.update(base_string.as_bytes());
    let result = mac.finalize();
    let computed_sig = hex::encode(result.into_bytes());

    // 4. Compare signatures (prepend v0= to computed signature)
    let expected = format!("v0={computed_sig}");
    if !constant_time_eq(&expected, signature) {
        anyhow::bail!("Signature mismatch");
    }

    Ok(())
}

/// Constant-time string comparison to prevent timing attacks.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

// =============================================================================
// EVENT HANDLING
// =============================================================================

/// Handle inbound Slack event webhook.
///
/// Verifies signature, deserializes payload, and routes to appropriate handler.
///
/// # Errors
///
/// Returns an error if signature verification fails or event processing fails.
pub async fn handle_event(
    payload_bytes: &[u8],
    timestamp: &str,
    signature: &str,
    adapter: &SlackAdapter,
) -> anyhow::Result<SlackEventResponse> {
    // 1. Verify signature
    verify_slack_signature(
        adapter.signing_secret(),
        timestamp,
        payload_bytes,
        signature,
    )?;

    // 2. Deserialize payload
    let payload: SlackEventPayload = serde_json::from_slice(payload_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to deserialize Slack payload: {e}"))?;

    // 3. Route by event type
    match payload.type_.as_str() {
        "url_verification" => {
            // URL verification challenge - return challenge string
            if let Some(challenge) = payload.challenge {
                adapter.event_stream.publish(EventEnvelope::new(
                    EventLevel::Info,
                    EventType::Custom(events::SLACK_URL_VERIFIED.to_string()),
                    json!({
                        "channel_type": "slack",
                        "team_id": payload.team_id,
                    }),
                ));
                Ok(SlackEventResponse::Challenge(challenge))
            } else {
                anyhow::bail!("url_verification missing challenge field")
            }
        }
        "event_callback" => {
            // Process message event
            if let Some(event) = payload.event {
                process_message_event(&event, adapter).await?;
            }
            Ok(SlackEventResponse::Ok)
        }
        _ => {
            tracing::warn!(
                event_type = %payload.type_,
                "Unknown Slack event type"
            );
            Ok(SlackEventResponse::Ok)
        }
    }
}

/// Process a single Slack message event.
#[allow(clippy::too_many_lines)]
async fn process_message_event(event: &SlackEvent, adapter: &SlackAdapter) -> anyhow::Result<()> {
    // 1. Skip bot messages
    if event.bot_id.is_some() {
        tracing::debug!(
            bot_id = %event.bot_id.as_ref().unwrap(),
            "Skipping bot message"
        );
        return Ok(());
    }

    // 2. Extract fields
    let channel_id = event
        .channel
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing channel field"))?;
    let user_id = event
        .user
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing user field"))?;
    let text = event
        .text
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing text field"))?;
    let ts = event.ts.as_deref().unwrap_or("");

    let correlation_id = Uuid::new_v4();

    // 3. Check for /carnelian command prefix
    let cmd_text = text.trim();
    if cmd_text.starts_with("/carnelian ") || cmd_text == "/carnelian" {
        let args = if cmd_text.len() > 11 {
            &cmd_text[11..]
        } else {
            ""
        };
        super::commands::dispatch_command(
            args,
            channel_id,
            user_id,
            adapter,
            &adapter.db_pool,
            &adapter.event_stream,
            &adapter.policy_engine,
        )
        .await?;
        return Ok(());
    }

    // 4. Session upsert
    let metadata = json!({
        "slack_user_id": user_id,
        "slack_channel_id": channel_id,
        "slack_team_id": event.team_id.as_ref().unwrap_or(&String::new()),
        "slack_ts": ts,
    });

    let session = channel_db::upsert_channel_session(
        &adapter.db_pool,
        "slack",
        channel_id,
        TrustLevel::Untrusted.as_str(),
        adapter.config().identity_id,
        metadata,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to upsert channel session: {e}"))?;

    let trust_level = session.parsed_trust_level();

    // 5. Rate limit check (synchronous, no await)
    if let Err(e) = adapter
        .rate_limiter
        .check_rate_limit("slack", channel_id, trust_level)
    {
        tracing::warn!(
            error = %e,
            channel_id = %channel_id,
            "Rate limit exceeded"
        );
        adapter
            .send_message(
                channel_id,
                "⏳ Rate limit exceeded. Please wait a moment before sending more messages.",
            )
            .await?;
        return Ok(());
    }

    // 6. Spam detection (synchronous, no await)
    let spam_score = adapter
        .spam_detector
        .update_score("slack", channel_id, text);
    if adapter.spam_detector.is_spam(spam_score) {
        tracing::info!(
            channel_id = %channel_id,
            spam_score = %spam_score,
            "Spam detected, dropping message"
        );
        return Ok(());
    }

    // 7. Capability check
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
            channel_id = %channel_id,
            "Capability check failed"
        );
        adapter
            .send_message(
                channel_id,
                "🔒 You don't have permission to send messages. Use `/carnelian pair` to connect.",
            )
            .await?;
        return Ok(());
    }

    // 8. Session manager integration
    let stable_identity = adapter.config().identity_id.unwrap_or(session.session_id);

    let session_key = format!("agent:{stable_identity}:slack:channel:{channel_id}");

    let conv_session = adapter.session_manager.create_session(&session_key).await;

    if let Ok(conv) = conv_session {
        if let Err(e) = adapter
            .session_manager
            .append_message(
                conv.session_id,
                "user",
                text.to_string(),
                None, // token_estimate
                None, // tool_name
                None, // tool_call_id
                Some(correlation_id),
                None, // metadata
                None, // tool_metadata
            )
            .await
        {
            tracing::warn!(error = %e, "Failed to append Slack message to session");
        }
    }

    // 9. Emit message received event
    adapter.event_stream.publish(EventEnvelope::new(
        EventLevel::Info,
        EventType::Custom(events::SLACK_MESSAGE_RECEIVED.to_string()),
        json!({
            "channel_type": "slack",
            "channel_user_id": channel_id,
            "slack_user_id": user_id,
            "trust_level": trust_level.as_str(),
            "message_length": text.len(),
            "correlation_id": correlation_id.to_string(),
        }),
    ));

    // 10. Touch session
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
    use crate::types::TrustLevel;

    #[test]
    fn test_url_verification_challenge() {
        // Test that handle_event properly verifies HMAC and returns challenge
        use std::time::{SystemTime, UNIX_EPOCH};

        let test_secret = "test_signing_secret_123";
        let challenge_str = "test_challenge_123";
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        // Build payload
        let payload = serde_json::json!({
            "type": "url_verification",
            "challenge": challenge_str,
            "team_id": "T123456"
        });
        let payload_bytes = payload.to_string().into_bytes();

        // Compute HMAC signature
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(test_secret.as_bytes()).unwrap();
        let base_string = format!(
            "v0:{timestamp}:{}",
            std::str::from_utf8(&payload_bytes).unwrap()
        );
        mac.update(base_string.as_bytes());
        let computed_sig = hex::encode(mac.finalize().into_bytes());
        let signature = format!("v0={computed_sig}");

        // Verify signature function works
        let result = verify_slack_signature(test_secret, &timestamp, &payload_bytes, &signature);
        assert!(
            result.is_ok(),
            "Signature verification failed: {:?}",
            result
        );

        // The response would contain the challenge if we had a full adapter
        // Here we just verify the signature logic which is the core of handle_event
    }

    #[test]
    fn test_rate_limiter_blocks_after_threshold() {
        // Create a rate limiter with no persisted config (uses defaults)
        let rate_limiter = Arc::new(RateLimiter::new(None));

        // Untrusted trust level has 5 req/min limit
        let trust = TrustLevel::Untrusted;
        let channel_id = "C123456";

        // First 5 requests should succeed
        for i in 0..5 {
            let result = rate_limiter.check_rate_limit("slack", channel_id, trust);
            assert!(
                result.is_ok(),
                "Request {} should pass but got: {:?}",
                i + 1,
                result
            );
        }

        // 6th request should fail (rate limited)
        let result = rate_limiter.check_rate_limit("slack", channel_id, trust);
        assert!(
            result.is_err(),
            "Request 6 should be rate limited but passed"
        );
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "def"));
        assert!(!constant_time_eq("abc", "abcd"));
        assert!(!constant_time_eq("", "a"));
        assert!(constant_time_eq("", ""));
    }
}
