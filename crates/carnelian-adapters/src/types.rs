//! Core channel types for adapter integrations.
//!
//! These types map directly to the `channel_sessions` database table and
//! provide the foundation for trust-level classification, pairing flows,
//! and channel configuration.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use uuid::Uuid;

// =============================================================================
// CHANNEL TYPE
// =============================================================================

/// Supported channel types matching the `channel_type` CHECK constraint
/// in the `channel_sessions` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    Telegram,
    Discord,
    Whatsapp,
    Slack,
    Ui,
}

impl ChannelType {
    /// Returns the database string representation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Telegram => "telegram",
            Self::Discord => "discord",
            Self::Whatsapp => "whatsapp",
            Self::Slack => "slack",
            Self::Ui => "ui",
        }
    }
}

impl std::fmt::Display for ChannelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ChannelType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "telegram" => Ok(Self::Telegram),
            "discord" => Ok(Self::Discord),
            "whatsapp" => Ok(Self::Whatsapp),
            "slack" => Ok(Self::Slack),
            "ui" => Ok(Self::Ui),
            other => Err(format!("Unknown channel type: {other}")),
        }
    }
}

// =============================================================================
// TRUST LEVEL
// =============================================================================

/// Trust levels matching the `trust_level` CHECK constraint in the
/// `channel_sessions` table.
///
/// | Level          | Description                                    |
/// |----------------|------------------------------------------------|
/// | `Untrusted`    | Read-only, rate-limited, 24h session expiry    |
/// | `Conversational` | Can send/receive, moderate rate limits       |
/// | `Owner`        | Full access, elevated capabilities             |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrustLevel {
    Conversational,
    Untrusted,
    Owner,
}

impl TrustLevel {
    /// Returns the database string representation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Conversational => "conversational",
            Self::Untrusted => "untrusted",
            Self::Owner => "owner",
        }
    }

    /// Returns the rate limit (messages per minute) for this trust level.
    #[must_use]
    pub fn rate_limit_per_minute(&self) -> u32 {
        match self {
            Self::Untrusted => 5,
            Self::Conversational => 30,
            Self::Owner => 100,
        }
    }

    /// Returns the context window token limit for this trust level.
    #[must_use]
    pub fn context_window_tokens(&self) -> usize {
        match self {
            Self::Untrusted => 4_000,
            Self::Conversational => 16_000,
            Self::Owner => 128_000,
        }
    }

    /// Returns the capabilities granted at this trust level.
    #[must_use]
    pub fn capabilities(&self) -> &'static [&'static str] {
        match self {
            Self::Untrusted => &["channel.message.receive"],
            Self::Conversational => &["channel.message.receive", "channel.message.send"],
            Self::Owner => &[
                "channel.message.receive",
                "channel.message.send",
                "task.create",
                "skill.execute",
                "config.read",
            ],
        }
    }
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TrustLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "conversational" => Ok(Self::Conversational),
            "untrusted" => Ok(Self::Untrusted),
            "owner" => Ok(Self::Owner),
            other => Err(format!("Unknown trust level: {other}")),
        }
    }
}

// =============================================================================
// CHANNEL SESSION
// =============================================================================

/// A channel session row from the `channel_sessions` table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChannelSession {
    pub session_id: Uuid,
    pub channel_type: String,
    pub channel_user_id: String,
    pub trust_level: String,
    pub identity_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub metadata: JsonValue,
}

impl ChannelSession {
    /// Parse the stored `trust_level` string into a `TrustLevel` enum.
    #[must_use]
    pub fn parsed_trust_level(&self) -> TrustLevel {
        self.trust_level
            .parse()
            .unwrap_or(TrustLevel::Untrusted)
    }

    /// Parse the stored `channel_type` string into a `ChannelType` enum.
    #[must_use]
    pub fn parsed_channel_type(&self) -> Option<ChannelType> {
        self.channel_type.parse().ok()
    }
}

// =============================================================================
// CHANNEL CONFIG
// =============================================================================

/// Configuration for a channel adapter instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Unique identifier for this channel configuration.
    pub channel_id: Uuid,
    /// Type of channel (telegram, discord, etc.).
    pub channel_type: ChannelType,
    /// Bot token for authenticating with the channel API.
    /// Stored encrypted in the database; decrypted at runtime.
    pub bot_token: String,
    /// Default trust level for new sessions on this channel.
    pub default_trust_level: TrustLevel,
    /// Whether this channel adapter is enabled.
    pub enabled: bool,
    /// Optional identity ID to associate with sessions on this channel.
    pub identity_id: Option<Uuid>,
}

// =============================================================================
// PAIRING REQUEST
// =============================================================================

/// Represents a pending pairing request initiated by the `/pair` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRequest {
    /// Unique pairing token (UUID).
    pub token: Uuid,
    /// Channel type of the requesting user.
    pub channel_type: ChannelType,
    /// Channel-specific user identifier.
    pub channel_user_id: String,
    /// Requested trust level (may be upgraded during confirmation).
    pub requested_trust_level: TrustLevel,
    /// When the pairing request was created.
    pub created_at: DateTime<Utc>,
    /// When the pairing request expires (default: 15 minutes).
    pub expires_at: DateTime<Utc>,
}

impl PairingRequest {
    /// Create a new pairing request with a 15-minute expiry.
    ///
    /// If `requested_trust_level` is `None`, defaults to `Conversational`.
    #[must_use]
    pub fn new(
        channel_type: ChannelType,
        channel_user_id: String,
        requested_trust_level: Option<TrustLevel>,
    ) -> Self {
        let now = Utc::now();
        Self {
            token: Uuid::now_v7(),
            channel_type,
            channel_user_id,
            requested_trust_level: requested_trust_level.unwrap_or(TrustLevel::Conversational),
            created_at: now,
            expires_at: now + chrono::Duration::minutes(15),
        }
    }

    /// Returns `true` if this pairing request has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}
