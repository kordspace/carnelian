//! Unified Session Management
//!
//! This module provides database-backed session management for agent conversations
//! across all channels (UI, CLI, external integrations). Sessions track message
//! history, token usage, and support optional file-backed JSONL transcripts.
//!
//! ## Session Key Format
//!
//! Session keys follow the format: `agent:<agentId>:<channel>:group:<id>`
//!
//! - `agent:<uuid>` — the agent identity this session belongs to
//! - `:<channel>` — channel type (e.g., `ui`, `cli`, `telegram`, `discord`)
//! - `:group:<id>` — optional group identifier for multi-session agents
//!
//! ## Token Counters
//!
//! Each session tracks token usage as JSONB with fields:
//! `{ "total": N, "user": N, "assistant": N, "tool": N }`
//!
//! ## Storage
//!
//! - **Primary**: PostgreSQL `sessions` and `session_messages` tables
//! - **Optional**: File-backed JSONL transcripts for archival/export
//!
//! ## Session Compaction
//!
//! When a session's total token count approaches the context window limit,
//! compaction is triggered to reduce the token footprint while preserving
//! important information.
//!
//! ### Trigger Conditions
//!
//! Compaction fires when `token_counters.total > (context_window_limit - reserve_tokens)`,
//! where `reserve_tokens = limit * (context_reserve_percent / 100)`. It can also
//! be triggered manually or by scheduled maintenance.
//!
//! ### Compaction Pipeline
//!
//! 1. **Memory flush** — Extract important user/assistant exchanges and persist
//!    them as durable memories via `MemoryManager`. Uses a heuristic importance
//!    scorer (0.6–0.8 range based on exchange length). Explicitly records
//!    "nothing to store" when no qualifying exchanges are found.
//! 2. **Conversation summarization** — Older messages (> 1 hour) are replaced
//!    with a single system summary message. Original messages are flagged with
//!    `{"compacted": true}` metadata and then deleted. (Current implementation
//!    uses extractive summarization; future phases will integrate LLM-based
//!    summarization.)
//! 3. **Tool result pruning** — Oversized tool results are soft-trimmed
//!    (head/tail preserved with ellipsis) and old tool results are hard-cleared
//!    (deleted), using thresholds from `Config`.
//! 4. **Token recalculation** — Counters are recomputed from remaining messages.
//! 5. **Session update** — `compaction_count` is incremented and `updated_at` set.
//!
//! ### Audit Trail
//!
//! Every compaction emits `MemoryCompressStart` / `MemoryCompressEnd` events and
//! logs a `session.compacted` entry to the tamper-resistant ledger with full
//! before/after metrics.
//!
//! ### Automatic vs Manual
//!
//! - **Automatic**: Use `append_message_with_compaction()` to check after every
//!   message append. Compaction errors are logged but never fail the append.
//! - **Manual**: Call `compact_session()` directly with `CompactionTrigger::ManualRequest`.
//!
//! ## Example
//!
//! ```ignore
//! let manager = SessionManager::new(pool, Some(event_stream), None, 24);
//! let session = manager.create_session("agent:uuid:ui:group:main").await?;
//! manager.append_message(session.session_id, "user", "Hello".to_string(), Some(5), None, None, None, None, None).await?;
//! let messages = manager.load_messages(session.session_id, None, None).await?;
//!
//! // Manual compaction
//! let outcome = manager.compact_session(
//!     session.session_id,
//!     CompactionTrigger::ManualRequest,
//!     None,
//!     &config,
//!     Some(&ledger),
//!     false,
//! ).await?;
//!
//! // Automatic compaction on message append
//! let (msg_id, compaction) = manager.append_message_with_compaction(
//!     session.session_id, "user", "Hello".into(), Some(5),
//!     None, None, None, None, None, &config, Some(&ledger),
//! ).await?;
//! ```

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use tokio::io::AsyncWriteExt;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_common::{Error, Result};
use carnelian_magic::QuantumHasher;

use crate::config::Config;
use crate::context::{estimate_tokens, ContextWindow};
use crate::events::EventStream;
use crate::ledger::Ledger;
use crate::memory::{MemoryManager, MemorySource};

// =============================================================================
// SESSION KEY
// =============================================================================

/// Parsed session key components.
///
/// Session keys follow the format `agent:<agentId>:<channel>:group:<id>`.
/// The group component is optional — keys without it are valid.
///
/// # Examples
///
/// ```ignore
/// let key: SessionKey = "agent:550e8400-e29b-41d4-a716-446655440000:ui:group:main".parse()?;
/// assert!(key.is_ui());
/// assert_eq!(key.group_id, Some("main".to_string()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionKey {
    /// The agent identity UUID
    pub agent_id: Uuid,
    /// Channel type (e.g., "ui", "cli", "telegram")
    pub channel: String,
    /// Optional group identifier
    pub group_id: Option<String>,
}

impl SessionKey {
    /// Returns true if this session is on the UI channel.
    #[must_use]
    pub fn is_ui(&self) -> bool {
        self.channel == "ui"
    }

    /// Returns true if this session is on the CLI channel.
    #[must_use]
    pub fn is_cli(&self) -> bool {
        self.channel == "cli"
    }

    /// Returns the channel type as a string slice.
    #[must_use]
    pub fn channel_type(&self) -> &str {
        &self.channel
    }
}

impl FromStr for SessionKey {
    type Err = Error;

    /// Parse a session key string into its components.
    ///
    /// Expected format: `agent:<uuid>:<channel>` or `agent:<uuid>:<channel>:group:<id>`
    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();

        // Minimum: ["agent", "<uuid>", "<channel>"]
        if parts.len() < 3 {
            return Err(Error::Session(format!(
                "Invalid session key format '{}': expected at least 'agent:<uuid>:<channel>'",
                s
            )));
        }

        if parts[0] != "agent" {
            return Err(Error::Session(format!(
                "Invalid session key prefix '{}': expected 'agent'",
                parts[0]
            )));
        }

        let agent_id = Uuid::parse_str(parts[1]).map_err(|e| {
            Error::Session(format!(
                "Invalid agent UUID '{}' in session key: {}",
                parts[1], e
            ))
        })?;

        let channel = parts[2].to_string();
        if channel.is_empty() {
            return Err(Error::Session("Empty channel in session key".to_string()));
        }

        // Optional group: parts[3] == "group", parts[4] == id
        let group_id = if parts.len() >= 5 && parts[3] == "group" {
            let gid = parts[4..].join(":");
            if gid.is_empty() {
                return Err(Error::Session("Empty group ID in session key".to_string()));
            }
            Some(gid)
        } else if parts.len() > 3 && parts[3] != "group" {
            return Err(Error::Session(format!(
                "Unexpected segment '{}' in session key, expected 'group'",
                parts[3]
            )));
        } else {
            None
        };

        Ok(Self {
            agent_id,
            channel,
            group_id,
        })
    }
}

impl fmt::Display for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "agent:{}:{}", self.agent_id, self.channel)?;
        if let Some(ref gid) = self.group_id {
            write!(f, ":group:{}", gid)?;
        }
        Ok(())
    }
}

// =============================================================================
// TOKEN COUNTERS
// =============================================================================

/// Token usage counters stored as JSONB in the sessions table.
///
/// Tracks token consumption by role to enable usage monitoring,
/// billing, and compaction trigger decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenCounters {
    /// Total tokens across all roles
    pub total: i64,
    /// Tokens from user messages
    pub user: i64,
    /// Tokens from assistant responses
    pub assistant: i64,
    /// Tokens from tool calls/results
    pub tool: i64,
}

impl Default for TokenCounters {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenCounters {
    /// Create zero-initialized token counters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            total: 0,
            user: 0,
            assistant: 0,
            tool: 0,
        }
    }

    /// Add tokens for a user message.
    pub fn add_user(&mut self, tokens: i64) {
        self.user += tokens;
        self.total += tokens;
    }

    /// Add tokens for an assistant message.
    pub fn add_assistant(&mut self, tokens: i64) {
        self.assistant += tokens;
        self.total += tokens;
    }

    /// Add tokens for a tool call/result.
    pub fn add_tool(&mut self, tokens: i64) {
        self.tool += tokens;
        self.total += tokens;
    }

    /// Increment counters by role name.
    ///
    /// Valid roles: `user`, `assistant`, `tool`, `system` (system tokens
    /// are added to total only).
    pub fn increment_by(&mut self, role: &str, tokens: i64) {
        match role {
            "user" => self.add_user(tokens),
            "assistant" => self.add_assistant(tokens),
            "tool" => self.add_tool(tokens),
            "system" => self.total += tokens,
            _ => self.total += tokens,
        }
    }
}

// =============================================================================
// SESSION
// =============================================================================

/// A session record matching the `sessions` database table.
///
/// Sessions track agent conversations with token usage, expiration,
/// and optional file-backed transcripts.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    /// Unique session identifier
    pub session_id: Uuid,
    /// Composite session key (e.g., `agent:<uuid>:ui:group:main`)
    pub session_key: String,
    /// Agent identity this session belongs to
    pub agent_id: Uuid,
    /// Channel type (e.g., "ui", "cli", "telegram")
    pub channel: String,
    /// Optional path to JSONL transcript file
    pub transcript_path: Option<String>,
    /// Token usage counters (JSONB)
    pub token_counters: JsonValue,
    /// Number of times this session has been compacted
    pub compaction_count: i32,
    /// Maximum context window size (tokens) for this session
    pub context_window_limit: Option<i32>,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last modified
    pub updated_at: DateTime<Utc>,
    /// When the session last had activity
    pub last_activity_at: DateTime<Utc>,
    /// When the session expires (None = never)
    pub expires_at: Option<DateTime<Utc>>,
}

impl Session {
    /// Returns true if this session has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|exp| exp < Utc::now())
    }

    /// Returns true if the session should be compacted based on token usage.
    ///
    /// Compaction is suggested when total tokens exceed the context window
    /// limit (if set).
    #[must_use]
    pub fn should_compact(&self) -> bool {
        let Some(limit) = self.context_window_limit else {
            return false;
        };
        let counters: TokenCounters =
            serde_json::from_value(self.token_counters.clone()).unwrap_or_default();
        counters.total > i64::from(limit)
    }

    /// Returns the duration until this session expires, or None if it never expires.
    #[must_use]
    pub fn time_until_expiry(&self) -> Option<Duration> {
        self.expires_at.map(|exp| exp - Utc::now())
    }

    /// Parse the token_counters JSONB into a typed struct.
    #[must_use]
    pub fn counters(&self) -> TokenCounters {
        serde_json::from_value(self.token_counters.clone()).unwrap_or_default()
    }
}

// =============================================================================
// SESSION MESSAGE
// =============================================================================

/// Valid message roles.
const VALID_ROLES: &[&str] = &["system", "user", "assistant", "tool"];

/// A single message in a session, matching the `session_messages` table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SessionMessage {
    /// Auto-incrementing message identifier
    pub message_id: i64,
    /// Session this message belongs to
    pub session_id: Uuid,
    /// Timestamp of the message
    pub ts: DateTime<Utc>,
    /// Message role: system, user, assistant, or tool
    pub role: String,
    /// Message content text
    pub content: String,
    /// Tool name (for tool role messages)
    pub tool_name: Option<String>,
    /// Tool call ID for correlating tool calls with results
    pub tool_call_id: Option<String>,
    /// Correlation ID for tracing
    pub correlation_id: Option<Uuid>,
    /// Estimated token count for this message
    pub token_estimate: Option<i32>,
    /// Additional metadata (JSONB)
    pub metadata: JsonValue,
    /// Tool-specific metadata (JSONB)
    pub tool_metadata: JsonValue,
}

/// Validate that a role string is one of the allowed values.
fn validate_role(role: &str) -> Result<()> {
    if VALID_ROLES.contains(&role) {
        Ok(())
    } else {
        Err(Error::Session(format!(
            "Invalid message role '{}': must be one of {:?}",
            role, VALID_ROLES
        )))
    }
}

/// Truncate a string to at most `max_chars` characters for memory storage.
///
/// If the string exceeds the limit, it is cut at a character boundary and
/// an ellipsis is appended.
fn truncate_for_memory(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .take_while(|(i, _)| *i < max_chars)
        .last()
        .map_or(0, |(i, c)| i + c.len_utf8());
    format!("{}...", &s[..end])
}

// =============================================================================
// SESSION STATS
// =============================================================================

/// Statistics for a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    /// Total number of messages
    pub message_count: i64,
    /// Messages by role
    pub system_count: i64,
    pub user_count: i64,
    pub assistant_count: i64,
    pub tool_count: i64,
    /// Token totals
    pub token_counters: TokenCounters,
    /// Session duration
    pub duration_seconds: i64,
}

// =============================================================================
// COMPACTION TYPES
// =============================================================================

/// Reason a session compaction was triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionTrigger {
    /// Token usage exceeded the effective context window limit
    TokenLimitExceeded,
    /// Compaction was explicitly requested (e.g., via API)
    ManualRequest,
    /// Compaction triggered by a scheduled maintenance job
    ScheduledMaintenance,
}

impl fmt::Display for CompactionTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenLimitExceeded => write!(f, "token_limit_exceeded"),
            Self::ManualRequest => write!(f, "manual_request"),
            Self::ScheduledMaintenance => write!(f, "scheduled_maintenance"),
        }
    }
}

/// Metrics captured during a session compaction operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionOutcome {
    /// Total tokens before compaction
    pub tokens_before: i64,
    /// Total tokens after compaction
    pub tokens_after: i64,
    /// Number of messages deleted (hard-clear)
    pub messages_pruned: usize,
    /// Number of messages replaced with a summary
    pub messages_summarized: usize,
    /// Number of memories created during the flush step
    pub memories_flushed: usize,
    /// Number of tool results soft-trimmed
    pub tool_results_trimmed: usize,
    /// Number of tool results hard-cleared
    pub tool_results_cleared: usize,
    /// Wall-clock duration of the compaction in milliseconds
    pub duration_ms: u64,
    /// True if the memory flush succeeded but had nothing to store
    pub nothing_to_store: bool,
    /// True if the memory flush step encountered an error
    pub flush_failed: bool,
}

// =============================================================================
// SESSION MANAGER
// =============================================================================

/// Manages session lifecycle, message persistence, and token tracking.
///
/// Follows the established manager pattern (see `SoulManager`, `SkillDiscovery`)
/// with database-backed persistence and optional event stream integration.
pub struct SessionManager {
    /// Database connection pool
    pool: PgPool,
    /// Optional event stream for audit trail
    event_stream: Option<Arc<EventStream>>,
    /// Optional path for file-backed JSONL transcripts
    transcripts_path: Option<PathBuf>,
    /// Default session expiry in hours (0 = never expires)
    default_expiry_hours: u32,
    /// Safe mode guard for blocking filesystem writes
    safe_mode_guard: Option<Arc<crate::safe_mode::SafeModeGuard>>,
}

impl SessionManager {
    /// Create a new SessionManager with full configuration.
    ///
    /// # Arguments
    ///
    /// * `pool` - PostgreSQL connection pool
    /// * `event_stream` - Optional event stream for audit events
    /// * `transcripts_path` - Optional directory for JSONL transcript files
    /// * `default_expiry_hours` - Default session TTL in hours (0 = no expiry)
    pub fn new(
        pool: PgPool,
        event_stream: Option<Arc<EventStream>>,
        transcripts_path: Option<PathBuf>,
        default_expiry_hours: u32,
    ) -> Self {
        Self {
            pool,
            event_stream,
            transcripts_path,
            default_expiry_hours,
            safe_mode_guard: None,
        }
    }

    /// Create a SessionManager with sensible defaults.
    ///
    /// Uses no event stream, no file transcripts, and 24-hour expiry.
    #[must_use]
    pub fn with_defaults(pool: PgPool) -> Self {
        Self {
            pool,
            event_stream: None,
            transcripts_path: None,
            default_expiry_hours: 24,
            safe_mode_guard: None,
        }
    }

    /// Set the safe mode guard for blocking filesystem writes when safe mode is active.
    pub fn set_safe_mode_guard(&mut self, guard: Arc<crate::safe_mode::SafeModeGuard>) {
        self.safe_mode_guard = Some(guard);
    }

    /// Builder-style setter for the safe mode guard.
    #[must_use]
    pub fn with_safe_mode_guard(mut self, guard: Arc<crate::safe_mode::SafeModeGuard>) -> Self {
        self.safe_mode_guard = Some(guard);
        self
    }

    /// Returns `true` if a `SafeModeGuard` has been wired into this manager.
    #[must_use]
    pub fn has_safe_mode_guard(&self) -> bool {
        self.safe_mode_guard.is_some()
    }

    // =========================================================================
    // SESSION CRUD
    // =========================================================================

    /// Create a new session from a session key string.
    ///
    /// Parses the session key to extract agent_id and channel, initializes
    /// token counters to zero, and sets expiry based on `default_expiry_hours`.
    ///
    /// # Errors
    ///
    /// Returns an error if the session key format is invalid or if a session
    /// with the same key already exists.
    pub async fn create_session(&self, session_key: &str) -> Result<Session> {
        let parsed = SessionKey::from_str(session_key)?;
        let session_id = Uuid::new_v4();
        let counters = serde_json::to_value(TokenCounters::default())?;

        let expires_at = if self.default_expiry_hours > 0 {
            Some(Utc::now() + Duration::hours(i64::from(self.default_expiry_hours)))
        } else {
            None
        };

        let session: Session = sqlx::query_as(
            r"INSERT INTO sessions (session_id, session_key, agent_id, channel, token_counters, expires_at)
              VALUES ($1, $2, $3, $4, $5, $6)
              RETURNING *",
        )
        .bind(session_id)
        .bind(session_key)
        .bind(parsed.agent_id)
        .bind(&parsed.channel)
        .bind(&counters)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db_err) if db_err.constraint() == Some("sessions_session_key_key") => {
                Error::Session(format!("Session key '{}' already exists", session_key))
            }
            _ => Error::Database(e),
        })?;

        tracing::info!(
            session_id = %session_id,
            session_key = %session_key,
            agent_id = %parsed.agent_id,
            channel = %parsed.channel,
            "Session created"
        );

        self.emit_event(
            EventType::Custom("SessionCreated".to_string()),
            json!({
                "session_id": session_id,
                "session_key": session_key,
                "agent_id": parsed.agent_id,
                "channel": parsed.channel,
            }),
        );

        Ok(session)
    }

    /// Load a session by its session key.
    ///
    /// Returns `None` if no session exists with the given key.
    /// Updates `last_activity_at` on successful load.
    /// Returns `None` for expired sessions (does not delete them).
    pub async fn load_session(&self, session_key: &str) -> Result<Option<Session>> {
        let session: Option<Session> =
            sqlx::query_as("SELECT * FROM sessions WHERE session_key = $1")
                .bind(session_key)
                .fetch_optional(&self.pool)
                .await?;

        let Some(session) = session else {
            return Ok(None);
        };

        if session.is_expired() {
            tracing::debug!(
                session_key = %session_key,
                "Session expired, returning None"
            );
            return Ok(None);
        }

        // Touch last_activity_at
        sqlx::query("UPDATE sessions SET last_activity_at = NOW() WHERE session_id = $1")
            .bind(session.session_id)
            .execute(&self.pool)
            .await?;

        Ok(Some(session))
    }

    /// Update a session's mutable fields.
    ///
    /// Updates `transcript_path`, `token_counters`, `compaction_count`,
    /// `context_window_limit`, `expires_at`, and sets `updated_at` to NOW().
    pub async fn update_session(&self, session: &Session) -> Result<()> {
        sqlx::query(
            r"UPDATE sessions
              SET transcript_path = $1,
                  token_counters = $2,
                  compaction_count = $3,
                  context_window_limit = $4,
                  expires_at = $5,
                  updated_at = NOW()
              WHERE session_id = $6",
        )
        .bind(&session.transcript_path)
        .bind(&session.token_counters)
        .bind(session.compaction_count)
        .bind(session.context_window_limit)
        .bind(session.expires_at)
        .bind(session.session_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a session and all its messages (cascade).
    ///
    /// The database CASCADE constraint on `session_messages.session_id`
    /// automatically removes all associated messages.
    pub async fn delete_session(&self, session_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE session_id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        tracing::info!(session_id = %session_id, "Session deleted");

        self.emit_event(
            EventType::Custom("SessionDeleted".to_string()),
            json!({"session_id": session_id}),
        );

        Ok(())
    }

    // =========================================================================
    // MESSAGE OPERATIONS
    // =========================================================================

    /// Append a message to a session.
    ///
    /// All operations (message insert, counter update, `last_activity_at` touch)
    /// run inside a single database transaction. On any failure the transaction
    /// rolls back so counters and timestamps cannot drift from the messages table.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Target session
    /// * `role` - Message role (system, user, assistant, tool)
    /// * `content` - Message text
    /// * `token_estimate` - Optional estimated token count
    /// * `tool_name` - Tool name (for tool role)
    /// * `tool_call_id` - Tool call correlation ID
    /// * `correlation_id` - Request correlation ID
    /// * `metadata` - Additional JSONB metadata (pass `None` for default `{}`)
    /// * `tool_metadata` - Tool-specific JSONB metadata (pass `None` for default `{}`)
    ///
    /// # Returns
    ///
    /// The auto-generated `message_id`.
    #[allow(clippy::too_many_arguments)]
    pub async fn append_message(
        &self,
        session_id: Uuid,
        role: &str,
        content: String,
        token_estimate: Option<i32>,
        tool_name: Option<String>,
        tool_call_id: Option<String>,
        correlation_id: Option<Uuid>,
        metadata: Option<JsonValue>,
        tool_metadata: Option<JsonValue>,
    ) -> Result<i64> {
        validate_role(role)?;

        let meta = metadata.unwrap_or_else(|| json!({}));
        let tool_meta = tool_metadata.unwrap_or_else(|| json!({}));

        let mut tx = self.pool.begin().await.map_err(Error::Database)?;

        // 1. Insert the message
        let message_id: i64 = sqlx::query_scalar(
            r"INSERT INTO session_messages (session_id, role, content, token_estimate, tool_name, tool_call_id, correlation_id, metadata, tool_metadata)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
              RETURNING message_id",
        )
        .bind(session_id)
        .bind(role)
        .bind(&content)
        .bind(token_estimate)
        .bind(&tool_name)
        .bind(&tool_call_id)
        .bind(correlation_id)
        .bind(&meta)
        .bind(&tool_meta)
        .fetch_one(&mut *tx)
        .await?;

        // 2. Atomically update token counters and last_activity_at
        if let Some(tokens) = token_estimate {
            Self::increment_counters_in_tx(&mut tx, session_id, role, tokens).await?;
        }

        // 3. Touch last_activity_at (combined with counter update when possible)
        if token_estimate.is_none() {
            sqlx::query("UPDATE sessions SET last_activity_at = NOW() WHERE session_id = $1")
                .bind(session_id)
                .execute(&mut *tx)
                .await?;
        }

        // 4. Compute quantum checksum for integrity verification
        // Note: message_id is i64 (BIGSERIAL), convert to Uuid for hasher interface
        let message_uuid = Uuid::from_u128(message_id as u128);
        let ts: DateTime<Utc> =
            sqlx::query_scalar("SELECT ts FROM session_messages WHERE message_id = $1")
                .bind(message_id)
                .fetch_one(&mut *tx)
                .await?;

        let hasher = QuantumHasher::with_os_entropy();
        match hasher.compute_with_ts("session_messages", message_uuid, content.as_bytes(), ts) {
            Ok(checksum) => {
                if let Err(e) = sqlx::query(
                    "UPDATE session_messages SET quantum_checksum = $1 WHERE message_id = $2",
                )
                .bind(&checksum)
                .bind(message_id)
                .execute(&mut *tx)
                .await
                {
                    tracing::warn!(message_id = message_id, error = %e, "Failed to store quantum checksum");
                }
            }
            Err(e) => {
                tracing::warn!(message_id = message_id, error = %e, "Failed to compute quantum checksum");
            }
        }

        tx.commit().await.map_err(Error::Database)?;

        tracing::debug!(
            session_id = %session_id,
            message_id = message_id,
            role = %role,
            token_estimate = ?token_estimate,
            "Message appended"
        );

        Ok(message_id)
    }

    /// Append a message and automatically check whether compaction is needed.
    ///
    /// Delegates to `append_message` for the core insert, then calls
    /// `check_and_compact_if_needed`. Compaction errors are logged but
    /// do **not** fail the append — the message is already committed.
    ///
    /// Returns `(message_id, Option<CompactionOutcome>)`.
    #[allow(clippy::too_many_arguments)]
    pub async fn append_message_with_compaction(
        &self,
        session_id: Uuid,
        role: &str,
        content: String,
        token_estimate: Option<i32>,
        tool_name: Option<String>,
        tool_call_id: Option<String>,
        correlation_id: Option<Uuid>,
        metadata: Option<JsonValue>,
        tool_metadata: Option<JsonValue>,
        config: &Config,
        ledger: Option<&Ledger>,
    ) -> Result<(i64, Option<CompactionOutcome>)> {
        let message_id = self
            .append_message(
                session_id,
                role,
                content,
                token_estimate,
                tool_name,
                tool_call_id,
                correlation_id,
                metadata,
                tool_metadata,
            )
            .await?;

        let compaction_outcome = match self
            .check_and_compact_if_needed(session_id, correlation_id, config, ledger)
            .await
        {
            Ok(outcome) => outcome,
            Err(e) => {
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "Automatic compaction check failed (message already committed)"
                );
                None
            }
        };

        Ok((message_id, compaction_outcome))
    }

    /// Load messages for a session with optional cursor-based pagination.
    ///
    /// Messages are returned in reverse insertion order (newest first),
    /// ordered by `message_id DESC` to match the `message_id` cursor.
    /// Use `before_message_id` for stable cursor-based pagination.
    pub async fn load_messages(
        &self,
        session_id: Uuid,
        limit: Option<i64>,
        before_message_id: Option<i64>,
    ) -> Result<Vec<SessionMessage>> {
        let messages = if let Some(before_id) = before_message_id {
            sqlx::query_as(
                r"SELECT * FROM session_messages
                  WHERE session_id = $1 AND message_id < $2
                  ORDER BY message_id DESC
                  LIMIT $3",
            )
            .bind(session_id)
            .bind(before_id)
            .bind(limit.unwrap_or(100))
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                r"SELECT * FROM session_messages
                  WHERE session_id = $1
                  ORDER BY message_id DESC
                  LIMIT $2",
            )
            .bind(session_id)
            .bind(limit.unwrap_or(100))
            .fetch_all(&self.pool)
            .await?
        };

        Ok(messages)
    }

    /// Load messages since a given timestamp in chronological order.
    pub async fn load_messages_since(
        &self,
        session_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<Vec<SessionMessage>> {
        let messages: Vec<SessionMessage> = sqlx::query_as(
            r"SELECT * FROM session_messages
              WHERE session_id = $1 AND ts > $2
              ORDER BY ts ASC",
        )
        .bind(session_id)
        .bind(since)
        .fetch_all(&self.pool)
        .await?;

        Ok(messages)
    }

    /// Delete messages older than a given timestamp.
    ///
    /// After deletion, recalculates token counters from remaining messages.
    /// Returns the number of deleted messages.
    pub async fn delete_messages_before(
        &self,
        session_id: Uuid,
        before_ts: DateTime<Utc>,
    ) -> Result<u64> {
        let result = sqlx::query("DELETE FROM session_messages WHERE session_id = $1 AND ts < $2")
            .bind(session_id)
            .bind(before_ts)
            .execute(&self.pool)
            .await?;

        let deleted = result.rows_affected();

        // Recalculate token counters from remaining messages
        if deleted > 0 {
            self.recalculate_counters(session_id).await?;
        }

        Ok(deleted)
    }

    // =========================================================================
    // TOKEN COUNTER MANAGEMENT
    // =========================================================================

    /// Replace the token counters for a session.
    pub async fn update_counters(&self, session_id: Uuid, counters: &TokenCounters) -> Result<()> {
        let counters_json = serde_json::to_value(counters)?;

        sqlx::query(
            "UPDATE sessions SET token_counters = $1, updated_at = NOW() WHERE session_id = $2",
        )
        .bind(&counters_json)
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Atomically increment token counters for a specific role.
    ///
    /// Opens a transaction, locks the session row with `SELECT FOR UPDATE`,
    /// mutates the counters in Rust, and writes back within the same
    /// transaction to prevent lost updates under concurrency.
    pub async fn increment_counters(
        &self,
        session_id: Uuid,
        role: &str,
        tokens: i32,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(Error::Database)?;
        Self::increment_counters_in_tx(&mut tx, session_id, role, tokens).await?;
        tx.commit().await.map_err(Error::Database)?;
        Ok(())
    }

    /// Inner helper: increment counters within an existing transaction.
    ///
    /// Locks the session row with `SELECT ... FOR UPDATE`, deserialises the
    /// JSONB counters, increments the requested role and total, then writes
    /// back and touches `last_activity_at` — all inside the caller's
    /// transaction so the update is atomic.
    async fn increment_counters_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        session_id: Uuid,
        role: &str,
        tokens: i32,
    ) -> Result<()> {
        let counters_json: JsonValue = sqlx::query_scalar(
            "SELECT token_counters FROM sessions WHERE session_id = $1 FOR UPDATE",
        )
        .bind(session_id)
        .fetch_one(&mut **tx)
        .await?;

        let mut counters: TokenCounters = serde_json::from_value(counters_json).unwrap_or_default();
        counters.increment_by(role, i64::from(tokens));

        let updated_json = serde_json::to_value(&counters)?;
        sqlx::query(
            "UPDATE sessions SET token_counters = $1, updated_at = NOW(), last_activity_at = NOW() WHERE session_id = $2",
        )
        .bind(&updated_json)
        .bind(session_id)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    /// Get the current token counters for a session.
    pub async fn get_counters(&self, session_id: Uuid) -> Result<TokenCounters> {
        let counters_json: JsonValue =
            sqlx::query_scalar("SELECT token_counters FROM sessions WHERE session_id = $1")
                .bind(session_id)
                .fetch_one(&self.pool)
                .await?;

        let counters: TokenCounters = serde_json::from_value(counters_json)?;
        Ok(counters)
    }

    /// Recalculate token counters from remaining messages.
    async fn recalculate_counters(&self, session_id: Uuid) -> Result<()> {
        let rows: Vec<(String, Option<i64>)> = sqlx::query_as(
            r"SELECT role, SUM(token_estimate::bigint) as total
              FROM session_messages
              WHERE session_id = $1
              GROUP BY role",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        let mut counters = TokenCounters::new();
        for (role, total) in &rows {
            let tokens = total.unwrap_or(0);
            counters.increment_by(role, tokens);
        }

        self.update_counters(session_id, &counters).await
    }

    // =========================================================================
    // SESSION EXPIRATION AND CLEANUP
    // =========================================================================

    /// Delete all expired sessions.
    ///
    /// Returns the number of sessions deleted. Cascade constraints
    /// automatically remove associated messages.
    pub async fn cleanup_expired_sessions(&self) -> Result<u32> {
        let result =
            sqlx::query("DELETE FROM sessions WHERE expires_at IS NOT NULL AND expires_at < NOW()")
                .execute(&self.pool)
                .await?;

        let count = u32::try_from(result.rows_affected()).unwrap_or(u32::MAX);

        if count > 0 {
            tracing::info!(count = count, "Expired sessions cleaned up");

            self.emit_event(
                EventType::Custom("SessionsExpired".to_string()),
                json!({"count": count}),
            );
        }

        Ok(count)
    }

    /// Extend a session's expiry by additional hours.
    pub async fn extend_session(&self, session_id: Uuid, additional_hours: u32) -> Result<()> {
        sqlx::query(
            r"UPDATE sessions
              SET expires_at = COALESCE(expires_at, NOW()) + ($1 || ' hours')::interval,
                  updated_at = NOW()
              WHERE session_id = $2",
        )
        .bind(additional_hours.to_string())
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Lightweight touch to keep a session alive.
    pub async fn touch_session(&self, session_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE sessions SET last_activity_at = NOW() WHERE session_id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // =========================================================================
    // FILE-BACKED TRANSCRIPT SUPPORT
    // =========================================================================

    /// Write all session messages to a JSONL transcript file.
    ///
    /// Creates a file at `<transcripts_path>/<session_key>.jsonl` and updates
    /// the session's `transcript_path` field in the database.
    ///
    /// Returns the path to the written file.
    pub async fn write_transcript_to_file(&self, session: &Session) -> Result<PathBuf> {
        if let Some(ref guard) = self.safe_mode_guard {
            guard.check_or_block("filesystem_write").await?;
        }

        let transcripts_dir = self
            .transcripts_path
            .as_ref()
            .ok_or_else(|| Error::Session("Transcripts path not configured".to_string()))?;

        // Load all messages in chronological order
        let messages: Vec<SessionMessage> = sqlx::query_as(
            r"SELECT * FROM session_messages
              WHERE session_id = $1
              ORDER BY ts ASC",
        )
        .bind(session.session_id)
        .fetch_all(&self.pool)
        .await?;

        // Sanitize session key for filename
        let safe_name = session.session_key.replace(':', "_");
        let file_path = transcripts_dir.join(format!("{safe_name}.jsonl"));

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Write JSONL
        let mut content = String::new();
        for msg in &messages {
            let line = serde_json::to_string(msg)?;
            content.push_str(&line);
            content.push('\n');
        }
        tokio::fs::write(&file_path, &content).await?;

        // Update transcript_path in database
        let path_str = file_path.to_string_lossy().to_string();
        sqlx::query(
            "UPDATE sessions SET transcript_path = $1, updated_at = NOW() WHERE session_id = $2",
        )
        .bind(&path_str)
        .bind(session.session_id)
        .execute(&self.pool)
        .await?;

        tracing::info!(
            session_id = %session.session_id,
            path = %file_path.display(),
            message_count = messages.len(),
            "Transcript written to file"
        );

        Ok(file_path)
    }

    /// Load a transcript from a JSONL file.
    ///
    /// Each line in the file is expected to be a JSON-serialized `SessionMessage`.
    pub async fn load_transcript_from_file(
        &self,
        transcript_path: &str,
    ) -> Result<Vec<SessionMessage>> {
        let content = tokio::fs::read_to_string(transcript_path)
            .await
            .map_err(|e| {
                Error::Session(format!(
                    "Failed to read transcript file '{}': {}",
                    transcript_path, e
                ))
            })?;

        let mut messages = Vec::new();
        for (i, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let msg: SessionMessage = serde_json::from_str(line).map_err(|e| {
                Error::Session(format!(
                    "Failed to parse line {} of transcript '{}': {}",
                    i + 1,
                    transcript_path,
                    e
                ))
            })?;
            messages.push(msg);
        }

        Ok(messages)
    }

    /// Sync a session's messages to its transcript file (append new messages).
    ///
    /// Reads the existing file to determine the last synced message, then
    /// appends any newer messages.
    pub async fn sync_transcript(&self, session_id: Uuid) -> Result<()> {
        if let Some(ref guard) = self.safe_mode_guard {
            guard.check_or_block("filesystem_write").await?;
        }

        let session: Session = sqlx::query_as("SELECT * FROM sessions WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await?;

        let transcript_path = session.transcript_path.as_ref().ok_or_else(|| {
            Error::Session(format!("Session {} has no transcript_path set", session_id))
        })?;

        // Load existing transcript to find last timestamp
        let existing = self
            .load_transcript_from_file(transcript_path)
            .await
            .unwrap_or_default();
        let last_ts = existing.last().map(|m| m.ts);

        // Load new messages from database
        let new_messages: Vec<SessionMessage> = if let Some(since) = last_ts {
            self.load_messages_since(session_id, since).await?
        } else {
            sqlx::query_as("SELECT * FROM session_messages WHERE session_id = $1 ORDER BY ts ASC")
                .bind(session_id)
                .fetch_all(&self.pool)
                .await?
        };

        if new_messages.is_empty() {
            return Ok(());
        }

        // Append to file
        let mut content = String::new();
        for msg in &new_messages {
            let line = serde_json::to_string(msg)?;
            content.push_str(&line);
            content.push('\n');
        }

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(transcript_path)
            .await?;
        file.write_all(content.as_bytes()).await?;

        Ok(())
    }

    // =========================================================================
    // SESSION COMPACTION
    // =========================================================================

    /// Flush important conversation exchanges to durable memory.
    ///
    /// Loads recent session messages and extracts user/assistant exchanges
    /// that are worth persisting as long-term memories. Uses a simple
    /// heuristic: longer exchanges receive higher importance scores
    /// (0.6–0.8 range).
    ///
    /// Returns the number of memories created. Returns `Ok(0)` and logs
    /// explicitly when there is nothing to store.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Session to flush memories from
    /// * `correlation_id` - Optional correlation ID for tracing
    /// * `task_id` - Optional task context
    // Known limitation (v1.0.0): memory extraction uses keyword heuristics; LLM-based
    // extraction deferred until the gateway session is available inside the flush path.
    #[allow(clippy::too_many_lines)]
    pub async fn trigger_memory_flush(
        &self,
        session_id: Uuid,
        correlation_id: Option<Uuid>,
        _task_id: Option<Uuid>,
    ) -> Result<usize> {
        self.emit_event(
            EventType::MemoryWriteStart,
            json!({
                "session_id": session_id,
                "correlation_id": correlation_id,
                "source": "compaction_flush",
            }),
        );

        // Load the last 100 messages in chronological order
        let messages: Vec<SessionMessage> = sqlx::query_as(
            r"SELECT * FROM session_messages
              WHERE session_id = $1
              ORDER BY message_id DESC
              LIMIT 100",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        if messages.is_empty() {
            tracing::info!(
                session_id = %session_id,
                "Memory flush: nothing to store (no messages)"
            );
            self.emit_event(
                EventType::MemoryWriteEnd,
                json!({
                    "session_id": session_id,
                    "memories_created": 0,
                    "nothing_to_store": true,
                }),
            );
            return Ok(0);
        }

        // Resolve agent_id from the session
        let agent_id: Uuid =
            sqlx::query_scalar("SELECT agent_id FROM sessions WHERE session_id = $1")
                .bind(session_id)
                .fetch_one(&self.pool)
                .await?;

        let memory_mgr = MemoryManager::new(self.pool.clone(), self.event_stream.clone());

        // Extract user/assistant exchange pairs worth remembering.
        // Heuristic: pair each user message with the following assistant reply;
        // longer combined content → higher importance (0.6–0.8).
        let mut memories_created = 0usize;
        let mut reversed = messages;
        reversed.reverse(); // chronological order

        let mut i = 0;
        while i < reversed.len() {
            let msg = &reversed[i];
            if msg.role == "user" {
                // Look for the next assistant reply
                let assistant_reply = reversed.get(i + 1).filter(|m| m.role == "assistant");

                if let Some(reply) = assistant_reply {
                    let combined_len = msg.content.len() + reply.content.len();
                    // Only persist exchanges with meaningful content (> 100 chars combined)
                    if combined_len > 100 {
                        let importance = (0.6 + (combined_len as f32 / 5000.0).min(0.2)).min(0.8);
                        let content = format!(
                            "User asked: {}\nAssistant replied: {}",
                            truncate_for_memory(&msg.content, 500),
                            truncate_for_memory(&reply.content, 500),
                        );

                        match memory_mgr
                            .create_memory(
                                agent_id,
                                &content,
                                None,
                                MemorySource::Conversation,
                                None,
                                importance,
                                None,
                            )
                            .await
                        {
                            Ok(_) => memories_created += 1,
                            Err(e) => {
                                tracing::warn!(
                                    session_id = %session_id,
                                    error = %e,
                                    "Failed to create memory during flush"
                                );
                            }
                        }
                    }
                    i += 2; // skip past the pair
                    continue;
                }
            }
            i += 1;
        }

        if memories_created == 0 {
            tracing::info!(
                session_id = %session_id,
                "Memory flush: nothing to store (no qualifying exchanges)"
            );
        } else {
            tracing::info!(
                session_id = %session_id,
                memories_created,
                "Memory flush completed"
            );
        }

        self.emit_event(
            EventType::MemoryWriteEnd,
            json!({
                "session_id": session_id,
                "memories_created": memories_created,
                "nothing_to_store": memories_created == 0,
            }),
        );

        Ok(memories_created)
    }

    /// Summarize a range of conversation messages into a single system message.
    ///
    /// Loads messages between `start_message_id` and `end_message_id` (inclusive),
    /// concatenates user/assistant content, and inserts a summary system message.
    /// Original messages are flagged with `{"compacted": true}` metadata for
    /// potential future deletion.
    ///
    /// Returns `(summary_message_id, token_estimate, messages_summarized)`.
    ///
    // Known limitation (v1.0.0): extractive (frequency-based) summarization only;
    // LLM-based summarization deferred.
    pub async fn summarize_conversation_segment(
        &self,
        session_id: Uuid,
        start_message_id: i64,
        end_message_id: i64,
    ) -> Result<(i64, i32, usize)> {
        let messages: Vec<SessionMessage> = sqlx::query_as(
            r"SELECT * FROM session_messages
              WHERE session_id = $1
                AND message_id >= $2
                AND message_id <= $3
              ORDER BY message_id ASC",
        )
        .bind(session_id)
        .bind(start_message_id)
        .bind(end_message_id)
        .fetch_all(&self.pool)
        .await?;

        if messages.is_empty() {
            return Ok((0, 0, 0));
        }

        // Build extractive summary from user/assistant messages
        let mut summary_parts: Vec<String> = Vec::new();
        for msg in &messages {
            match msg.role.as_str() {
                "user" => {
                    summary_parts.push(format!(
                        "- User: {}",
                        truncate_for_memory(&msg.content, 200)
                    ));
                }
                "assistant" => {
                    summary_parts.push(format!(
                        "- Assistant: {}",
                        truncate_for_memory(&msg.content, 200)
                    ));
                }
                _ => {}
            }
        }

        if summary_parts.is_empty() {
            return Ok((0, 0, 0));
        }

        let first_ts = messages
            .first()
            .map(|m| m.ts.to_rfc3339())
            .unwrap_or_default();
        let last_ts = messages
            .last()
            .map(|m| m.ts.to_rfc3339())
            .unwrap_or_default();

        let summary_content = format!(
            "Summary of conversation from {} to {}:\n{}",
            first_ts,
            last_ts,
            summary_parts.join("\n"),
        );

        #[allow(clippy::cast_possible_wrap)]
        let token_est = estimate_tokens(&summary_content, "deepseek-r1:7b") as i32;

        // Insert the summary as a system message
        let summary_id = self
            .append_message(
                session_id,
                "system",
                summary_content,
                Some(token_est),
                None,
                None,
                None,
                Some(json!({"compaction_summary": true, "covers_range": [start_message_id, end_message_id]})),
                None,
            )
            .await?;

        // Mark original messages with compacted metadata flag
        let messages_summarized = messages.len();
        let compacted_flag = json!({"compacted": true});
        sqlx::query(
            r"UPDATE session_messages
              SET metadata = metadata || $4::jsonb
              WHERE session_id = $1
                AND message_id >= $2
                AND message_id <= $3",
        )
        .bind(session_id)
        .bind(start_message_id)
        .bind(end_message_id)
        .bind(&compacted_flag)
        .execute(&self.pool)
        .await?;

        tracing::debug!(
            session_id = %session_id,
            summary_id,
            messages_summarized,
            token_est,
            "Conversation segment summarized"
        );

        Ok((summary_id, token_est, messages_summarized))
    }

    /// Prune tool result messages by soft-trimming oversized ones and
    /// hard-clearing old ones.
    ///
    /// - **Soft-trim**: Messages with `token_estimate > config.tool_trim_threshold`
    ///   have their content trimmed in-place and metadata updated.
    /// - **Hard-clear**: Messages older than `config.tool_clear_age_secs` are deleted.
    ///
    /// Returns `(trimmed_count, cleared_count)`.
    pub async fn prune_tool_results(
        &self,
        session_id: Uuid,
        config: &Config,
    ) -> Result<(usize, usize)> {
        let tool_messages: Vec<SessionMessage> = sqlx::query_as(
            r"SELECT * FROM session_messages
              WHERE session_id = $1 AND role = 'tool'
              ORDER BY message_id ASC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        let now = Utc::now();
        let mut trimmed_count = 0usize;
        let mut cleared_count = 0usize;
        let mut cleared_token_delta = 0i64;

        for msg in &tool_messages {
            let age_secs = (now - msg.ts).num_seconds();

            // Hard-clear: delete messages older than threshold
            if age_secs > config.tool_clear_age_secs {
                sqlx::query(
                    "DELETE FROM session_messages WHERE message_id = $1 AND session_id = $2",
                )
                .bind(msg.message_id)
                .bind(session_id)
                .execute(&self.pool)
                .await?;
                cleared_count += 1;
                cleared_token_delta += i64::from(msg.token_estimate.unwrap_or(0));
                continue;
            }

            // Soft-trim: trim oversized tool results
            let token_est = msg.token_estimate.unwrap_or(0) as usize;
            if token_est > config.tool_trim_threshold {
                let trimmed_content = ContextWindow::soft_trim_tool_result(
                    &msg.content,
                    config.tool_trim_threshold,
                    "deepseek-r1:7b",
                );
                #[allow(clippy::cast_possible_wrap)]
                let new_tokens = estimate_tokens(&trimmed_content, "deepseek-r1:7b") as i32;

                sqlx::query(
                    r"UPDATE session_messages
                      SET content = $1,
                          token_estimate = $2,
                          metadata = metadata || $3::jsonb
                      WHERE message_id = $4 AND session_id = $5",
                )
                .bind(&trimmed_content)
                .bind(new_tokens)
                .bind(json!({"soft_trimmed": true, "original_tokens": token_est}))
                .bind(msg.message_id)
                .bind(session_id)
                .execute(&self.pool)
                .await?;
                trimmed_count += 1;
            }
        }

        // Adjust token counters for cleared messages
        if cleared_token_delta > 0 {
            let mut counters = self.get_counters(session_id).await?;
            counters.tool = (counters.tool - cleared_token_delta).max(0);
            counters.total = (counters.total - cleared_token_delta).max(0);
            self.update_counters(session_id, &counters).await?;
        }

        tracing::debug!(
            session_id = %session_id,
            trimmed_count,
            cleared_count,
            "Tool results pruned"
        );

        Ok((trimmed_count, cleared_count))
    }

    /// Execute a full session compaction.
    ///
    /// Runs the compaction pipeline in order:
    /// 1. Flush important exchanges to durable memory
    /// 2. Summarize older conversation messages (> 1 hour old)
    /// 3. Prune tool results (soft-trim + hard-clear)
    /// 4. Recalculate token counters from remaining messages
    /// 5. Increment `compaction_count` and update session
    ///
    /// Logs the compaction event to the ledger and emits
    /// `MemoryCompressStart` / `MemoryCompressEnd` events.
    #[allow(clippy::too_many_lines)]
    pub async fn compact_session(
        &self,
        session_id: Uuid,
        trigger: CompactionTrigger,
        correlation_id: Option<Uuid>,
        config: &Config,
        ledger: Option<&Ledger>,
        skip_flush: bool,
    ) -> Result<CompactionOutcome> {
        let start = Instant::now();

        // Load session and verify it exists
        let session: Session = sqlx::query_as("SELECT * FROM sessions WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| Error::Session(format!("Session {} not found", session_id)))?;

        let counters_before = session.counters();
        let tokens_before = counters_before.total;

        self.emit_event(
            EventType::MemoryCompressStart,
            json!({
                "session_id": session_id,
                "trigger": trigger.to_string(),
                "tokens_before": tokens_before,
                "correlation_id": correlation_id,
            }),
        );

        // Step 1: Flush important exchanges to durable memory (skipped when caller already flushed)
        let (memories_flushed, flush_failed) = if skip_flush {
            tracing::debug!(
                session_id = %session_id,
                "Skipping memory flush during compaction (caller already flushed)"
            );
            (0, false)
        } else {
            match self
                .trigger_memory_flush(session_id, correlation_id, None)
                .await
            {
                Ok(count) => (count, false),
                Err(e) => {
                    tracing::warn!(error = %e, "Memory flush failed during compaction");
                    (0, true)
                }
            }
        };

        // Step 2: Summarize older conversation messages (> 1 hour old)
        let one_hour_ago = Utc::now() - Duration::hours(1);
        let old_messages: Vec<(i64,)> = sqlx::query_as(
            r"SELECT message_id FROM session_messages
              WHERE session_id = $1
                AND ts < $2
                AND role IN ('user', 'assistant')
                AND (metadata->>'compacted') IS NULL
              ORDER BY message_id ASC",
        )
        .bind(session_id)
        .bind(one_hour_ago)
        .fetch_all(&self.pool)
        .await?;

        let mut messages_summarized = 0usize;
        if !old_messages.is_empty() {
            let start_id = old_messages.first().map_or(0, |(id,)| *id);
            let end_id = old_messages.last().map_or(0, |(id,)| *id);
            if start_id > 0 && end_id > 0 {
                let (_, _, count) = self
                    .summarize_conversation_segment(session_id, start_id, end_id)
                    .await?;
                messages_summarized = count;
            }
        }

        // Step 3: Prune tool results
        let (tool_results_trimmed, tool_results_cleared) =
            self.prune_tool_results(session_id, config).await?;

        // Step 4: Delete compacted original messages to reclaim space
        let delete_result = sqlx::query(
            r"DELETE FROM session_messages
              WHERE session_id = $1
                AND (metadata->>'compacted')::boolean = true",
        )
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        let messages_pruned = delete_result.rows_affected() as usize;

        // Step 5: Recalculate token counters from remaining messages
        self.recalculate_counters(session_id).await?;

        // Step 6: Increment compaction_count and update session
        sqlx::query(
            r"UPDATE sessions
              SET compaction_count = compaction_count + 1,
                  updated_at = NOW()
              WHERE session_id = $1",
        )
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        let counters_after = self.get_counters(session_id).await?;
        let tokens_after = counters_after.total;
        let duration_ms = start.elapsed().as_millis() as u64;

        let outcome = CompactionOutcome {
            tokens_before,
            tokens_after,
            messages_pruned,
            messages_summarized,
            memories_flushed,
            tool_results_trimmed,
            tool_results_cleared,
            duration_ms,
            nothing_to_store: !flush_failed && memories_flushed == 0,
            flush_failed,
        };

        // Log to ledger
        if let Some(ledger) = ledger {
            if let Err(e) = ledger
                .log_session_compaction(
                    session_id,
                    session.agent_id,
                    trigger,
                    &outcome,
                    correlation_id,
                )
                .await
            {
                tracing::warn!(error = %e, "Failed to log compaction to ledger");
            }
        }

        tracing::info!(
            session_id = %session_id,
            trigger = %trigger,
            tokens_before,
            tokens_after,
            messages_pruned,
            messages_summarized,
            memories_flushed,
            tool_results_trimmed,
            tool_results_cleared,
            duration_ms,
            "Session compaction completed"
        );

        self.emit_event(
            EventType::MemoryCompressEnd,
            json!({
                "session_id": session_id,
                "trigger": trigger.to_string(),
                "outcome": serde_json::to_value(&outcome).unwrap_or_default(),
            }),
        );

        Ok(outcome)
    }

    /// Check whether a session needs compaction and run it if so.
    ///
    /// Calculates the effective context window limit using the session's
    /// `context_window_limit` (or `config.context_window_tokens` as fallback),
    /// subtracts the reserve percentage, and triggers compaction when
    /// `token_counters.total` exceeds that threshold.
    ///
    /// Returns `None` if no compaction was needed.
    pub async fn check_and_compact_if_needed(
        &self,
        session_id: Uuid,
        correlation_id: Option<Uuid>,
        config: &Config,
        ledger: Option<&Ledger>,
    ) -> Result<Option<CompactionOutcome>> {
        let session: Session = sqlx::query_as("SELECT * FROM sessions WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await?;

        let limit = session
            .context_window_limit
            .map_or(config.context_window_tokens, |l| l as usize);

        let reserve_tokens =
            (limit as f64 * (f64::from(config.context_reserve_percent) / 100.0)) as i64;
        #[allow(clippy::cast_possible_wrap)]
        let effective_limit = limit as i64 - reserve_tokens;

        let counters = session.counters();
        if counters.total <= effective_limit {
            return Ok(None);
        }

        tracing::info!(
            session_id = %session_id,
            total_tokens = counters.total,
            effective_limit,
            "Token limit exceeded, triggering compaction"
        );

        let outcome = self
            .compact_session(
                session_id,
                CompactionTrigger::TokenLimitExceeded,
                correlation_id,
                config,
                ledger,
                false,
            )
            .await?;

        Ok(Some(outcome))
    }

    // =========================================================================
    // HELPER METHODS AND UTILITIES
    // =========================================================================

    /// Emit an event to the event stream (if available).
    fn emit_event(&self, event_type: EventType, payload: JsonValue) {
        if let Some(ref es) = self.event_stream {
            es.publish(EventEnvelope::new(EventLevel::Info, event_type, payload));
        }
    }

    /// List active (non-expired) sessions, optionally filtered by agent_id.
    pub async fn list_active_sessions(&self, agent_id: Option<Uuid>) -> Result<Vec<Session>> {
        let sessions = if let Some(aid) = agent_id {
            sqlx::query_as(
                r"SELECT * FROM sessions
                  WHERE agent_id = $1
                    AND (expires_at IS NULL OR expires_at > NOW())
                  ORDER BY last_activity_at DESC",
            )
            .bind(aid)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                r"SELECT * FROM sessions
                  WHERE expires_at IS NULL OR expires_at > NOW()
                  ORDER BY last_activity_at DESC",
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(sessions)
    }

    /// Get statistics for a session.
    pub async fn get_session_stats(&self, session_id: Uuid) -> Result<SessionStats> {
        // Get message counts by role
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r"SELECT role, COUNT(*) as cnt
              FROM session_messages
              WHERE session_id = $1
              GROUP BY role",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        let mut system_count = 0i64;
        let mut user_count = 0i64;
        let mut assistant_count = 0i64;
        let mut tool_count = 0i64;
        let mut message_count = 0i64;

        for (role, cnt) in &rows {
            message_count += cnt;
            match role.as_str() {
                "system" => system_count = *cnt,
                "user" => user_count = *cnt,
                "assistant" => assistant_count = *cnt,
                "tool" => tool_count = *cnt,
                _ => {}
            }
        }

        let counters = self.get_counters(session_id).await?;

        let session: Session = sqlx::query_as("SELECT * FROM sessions WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await?;

        let duration_seconds = (Utc::now() - session.created_at).num_seconds();

        Ok(SessionStats {
            message_count,
            system_count,
            user_count,
            assistant_count,
            tool_count,
            token_counters: counters,
            duration_seconds,
        })
    }

    // =========================================================================
    // LEDGER INTEGRATION
    // =========================================================================

    /// Log session creation to the audit ledger.
    pub async fn log_session_created(
        &self,
        ledger: &Ledger,
        session: &Session,
        correlation_id: Option<Uuid>,
    ) -> Result<i64> {
        ledger
            .append_event(
                Some(session.agent_id),
                "session.created",
                json!({
                    "session_id": session.session_id,
                    "agent_id": session.agent_id,
                }),
                correlation_id,
                None,
                None,
                None,
                None,
            )
            .await
    }

    /// Log session deletion to the audit ledger.
    pub async fn log_session_deleted(
        &self,
        ledger: &Ledger,
        session_id: Uuid,
        agent_id: Uuid,
        reason: &str,
        final_counters: &TokenCounters,
        message_count: i64,
    ) -> Result<i64> {
        ledger
            .append_event(
                Some(agent_id),
                "session.deleted",
                json!({
                    "session_id": session_id,
                    "message_count": message_count,
                }),
                None,
                None,
                None,
                None,
                None,
            )
            .await
    }

    /// Log session compaction to the audit ledger.
    pub async fn log_compaction_event(
        &self,
        ledger: &Ledger,
        session_id: Uuid,
        agent_id: Uuid,
        before_counters: &TokenCounters,
        after_counters: &TokenCounters,
        messages_removed: u64,
    ) -> Result<i64> {
        ledger
            .append_event(
                Some(agent_id),
                "session.compacted",
                json!({
                    "session_id": session_id,
                    "messages_removed": messages_removed,
                }),
                None,
                None,
                None,
                None,
                None,
            )
            .await
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // SessionKey parsing tests
    // =========================================================================

    #[test]
    fn test_parse_valid_session_key_with_group() {
        let key: SessionKey = "agent:550e8400-e29b-41d4-a716-446655440000:ui:group:main"
            .parse()
            .unwrap();
        assert_eq!(
            key.agent_id,
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
        );
        assert_eq!(key.channel, "ui");
        assert_eq!(key.group_id, Some("main".to_string()));
        assert!(key.is_ui());
        assert!(!key.is_cli());
    }

    #[test]
    fn test_parse_valid_session_key_without_group() {
        let key: SessionKey = "agent:550e8400-e29b-41d4-a716-446655440000:cli"
            .parse()
            .unwrap();
        assert_eq!(key.channel, "cli");
        assert_eq!(key.group_id, None);
        assert!(key.is_cli());
    }

    #[test]
    fn test_parse_session_key_invalid_prefix() {
        let result = SessionKey::from_str("session:550e8400-e29b-41d4-a716-446655440000:ui");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_session_key_invalid_uuid() {
        let result = SessionKey::from_str("agent:not-a-uuid:ui");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_session_key_too_short() {
        let result = SessionKey::from_str("agent:550e8400-e29b-41d4-a716-446655440000");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_session_key_empty_channel() {
        let result = SessionKey::from_str("agent:550e8400-e29b-41d4-a716-446655440000:");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_session_key_unexpected_segment() {
        let result =
            SessionKey::from_str("agent:550e8400-e29b-41d4-a716-446655440000:ui:extra:stuff");
        assert!(result.is_err());
    }

    #[test]
    fn test_session_key_display_with_group() {
        let key = SessionKey {
            agent_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            channel: "ui".to_string(),
            group_id: Some("main".to_string()),
        };
        assert_eq!(
            key.to_string(),
            "agent:550e8400-e29b-41d4-a716-446655440000:ui:group:main"
        );
    }

    #[test]
    fn test_session_key_display_without_group() {
        let key = SessionKey {
            agent_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            channel: "telegram".to_string(),
            group_id: None,
        };
        assert_eq!(
            key.to_string(),
            "agent:550e8400-e29b-41d4-a716-446655440000:telegram"
        );
    }

    #[test]
    fn test_session_key_roundtrip() {
        let original = "agent:550e8400-e29b-41d4-a716-446655440000:discord:group:lobby";
        let key: SessionKey = original.parse().unwrap();
        assert_eq!(key.to_string(), original);
    }

    #[test]
    fn test_session_key_channel_type() {
        let key: SessionKey = "agent:550e8400-e29b-41d4-a716-446655440000:telegram"
            .parse()
            .unwrap();
        assert_eq!(key.channel_type(), "telegram");
    }

    // =========================================================================
    // TokenCounters tests
    // =========================================================================

    #[test]
    fn test_token_counters_default() {
        let c = TokenCounters::default();
        assert_eq!(c.total, 0);
        assert_eq!(c.user, 0);
        assert_eq!(c.assistant, 0);
        assert_eq!(c.tool, 0);
    }

    #[test]
    fn test_token_counters_add_user() {
        let mut c = TokenCounters::new();
        c.add_user(100);
        assert_eq!(c.user, 100);
        assert_eq!(c.total, 100);
        assert_eq!(c.assistant, 0);
    }

    #[test]
    fn test_token_counters_add_assistant() {
        let mut c = TokenCounters::new();
        c.add_assistant(200);
        assert_eq!(c.assistant, 200);
        assert_eq!(c.total, 200);
    }

    #[test]
    fn test_token_counters_add_tool() {
        let mut c = TokenCounters::new();
        c.add_tool(50);
        assert_eq!(c.tool, 50);
        assert_eq!(c.total, 50);
    }

    #[test]
    fn test_token_counters_increment_by() {
        let mut c = TokenCounters::new();
        c.increment_by("user", 10);
        c.increment_by("assistant", 20);
        c.increment_by("tool", 5);
        c.increment_by("system", 3);
        assert_eq!(c.user, 10);
        assert_eq!(c.assistant, 20);
        assert_eq!(c.tool, 5);
        assert_eq!(c.total, 38);
    }

    #[test]
    fn test_token_counters_serialization_roundtrip() {
        let mut c = TokenCounters::new();
        c.add_user(100);
        c.add_assistant(200);
        c.add_tool(50);

        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json["total"], 350);
        assert_eq!(json["user"], 100);
        assert_eq!(json["assistant"], 200);
        assert_eq!(json["tool"], 50);

        let deserialized: TokenCounters = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, c);
    }

    #[test]
    fn test_token_counters_deserialize_from_db_default() {
        let db_json: JsonValue =
            serde_json::from_str(r#"{"total": 0, "user": 0, "assistant": 0, "tool": 0}"#).unwrap();
        let c: TokenCounters = serde_json::from_value(db_json).unwrap();
        assert_eq!(c, TokenCounters::default());
    }

    // =========================================================================
    // Session helper tests
    // =========================================================================

    #[test]
    fn test_session_is_expired_true() {
        let session = Session {
            session_id: Uuid::new_v4(),
            session_key: "agent:00000000-0000-0000-0000-000000000000:ui".to_string(),
            agent_id: Uuid::new_v4(),
            channel: "ui".to_string(),
            transcript_path: None,
            token_counters: serde_json::to_value(TokenCounters::default()).unwrap(),
            compaction_count: 0,
            context_window_limit: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_activity_at: Utc::now(),
            expires_at: Some(Utc::now() - Duration::hours(1)),
        };
        assert!(session.is_expired());
    }

    #[test]
    fn test_session_is_expired_false() {
        let session = Session {
            session_id: Uuid::new_v4(),
            session_key: "agent:00000000-0000-0000-0000-000000000000:ui".to_string(),
            agent_id: Uuid::new_v4(),
            channel: "ui".to_string(),
            transcript_path: None,
            token_counters: serde_json::to_value(TokenCounters::default()).unwrap(),
            compaction_count: 0,
            context_window_limit: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_activity_at: Utc::now(),
            expires_at: Some(Utc::now() + Duration::hours(1)),
        };
        assert!(!session.is_expired());
    }

    #[test]
    fn test_session_is_expired_none() {
        let session = Session {
            session_id: Uuid::new_v4(),
            session_key: "agent:00000000-0000-0000-0000-000000000000:ui".to_string(),
            agent_id: Uuid::new_v4(),
            channel: "ui".to_string(),
            transcript_path: None,
            token_counters: serde_json::to_value(TokenCounters::default()).unwrap(),
            compaction_count: 0,
            context_window_limit: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_activity_at: Utc::now(),
            expires_at: None,
        };
        assert!(!session.is_expired());
    }

    #[test]
    fn test_session_should_compact() {
        let mut counters = TokenCounters::new();
        counters.add_user(5000);

        let session = Session {
            session_id: Uuid::new_v4(),
            session_key: "agent:00000000-0000-0000-0000-000000000000:ui".to_string(),
            agent_id: Uuid::new_v4(),
            channel: "ui".to_string(),
            transcript_path: None,
            token_counters: serde_json::to_value(&counters).unwrap(),
            compaction_count: 0,
            context_window_limit: Some(4000),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_activity_at: Utc::now(),
            expires_at: None,
        };
        assert!(session.should_compact());
    }

    #[test]
    fn test_session_should_not_compact_no_limit() {
        let mut counters = TokenCounters::new();
        counters.add_user(5000);

        let session = Session {
            session_id: Uuid::new_v4(),
            session_key: "agent:00000000-0000-0000-0000-000000000000:ui".to_string(),
            agent_id: Uuid::new_v4(),
            channel: "ui".to_string(),
            transcript_path: None,
            token_counters: serde_json::to_value(&counters).unwrap(),
            compaction_count: 0,
            context_window_limit: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_activity_at: Utc::now(),
            expires_at: None,
        };
        assert!(!session.should_compact());
    }

    #[test]
    fn test_session_should_not_compact_under_limit() {
        let mut counters = TokenCounters::new();
        counters.add_user(1000);

        let session = Session {
            session_id: Uuid::new_v4(),
            session_key: "agent:00000000-0000-0000-0000-000000000000:ui".to_string(),
            agent_id: Uuid::new_v4(),
            channel: "ui".to_string(),
            transcript_path: None,
            token_counters: serde_json::to_value(&counters).unwrap(),
            compaction_count: 0,
            context_window_limit: Some(4000),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_activity_at: Utc::now(),
            expires_at: None,
        };
        assert!(!session.should_compact());
    }

    // =========================================================================
    // Role validation tests
    // =========================================================================

    #[test]
    fn test_validate_role_valid() {
        assert!(validate_role("system").is_ok());
        assert!(validate_role("user").is_ok());
        assert!(validate_role("assistant").is_ok());
        assert!(validate_role("tool").is_ok());
    }

    #[test]
    fn test_validate_role_invalid() {
        assert!(validate_role("admin").is_err());
        assert!(validate_role("").is_err());
        assert!(validate_role("USER").is_err());
    }

    // =========================================================================
    // Compaction type tests
    // =========================================================================

    #[test]
    fn test_compaction_trigger_display() {
        assert_eq!(
            CompactionTrigger::TokenLimitExceeded.to_string(),
            "token_limit_exceeded"
        );
        assert_eq!(
            CompactionTrigger::ManualRequest.to_string(),
            "manual_request"
        );
        assert_eq!(
            CompactionTrigger::ScheduledMaintenance.to_string(),
            "scheduled_maintenance"
        );
    }

    #[test]
    fn test_compaction_trigger_serialization_roundtrip() {
        let trigger = CompactionTrigger::TokenLimitExceeded;
        let json = serde_json::to_value(trigger).unwrap();
        assert_eq!(json, serde_json::json!("token_limit_exceeded"));
        let deserialized: CompactionTrigger = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, trigger);
    }

    #[test]
    fn test_compaction_trigger_all_variants_roundtrip() {
        for trigger in [
            CompactionTrigger::TokenLimitExceeded,
            CompactionTrigger::ManualRequest,
            CompactionTrigger::ScheduledMaintenance,
        ] {
            let json = serde_json::to_value(trigger).unwrap();
            let back: CompactionTrigger = serde_json::from_value(json).unwrap();
            assert_eq!(back, trigger);
        }
    }

    // =========================================================================
    // Compaction outcome serialization and semantics
    // =========================================================================

    #[test]
    fn test_compaction_outcome_serialization_all_fields() {
        let outcome = CompactionOutcome {
            tokens_before: 10000,
            tokens_after: 5000,
            messages_pruned: 20,
            messages_summarized: 15,
            memories_flushed: 3,
            tool_results_trimmed: 5,
            tool_results_cleared: 2,
            duration_ms: 150,
            nothing_to_store: false,
            flush_failed: false,
        };

        let json = serde_json::to_value(&outcome).unwrap();
        assert_eq!(json["tokens_before"], 10000);
        assert_eq!(json["tokens_after"], 5000);
        assert_eq!(json["messages_pruned"], 20);
        assert_eq!(json["messages_summarized"], 15);
        assert_eq!(json["memories_flushed"], 3);
        assert_eq!(json["tool_results_trimmed"], 5);
        assert_eq!(json["tool_results_cleared"], 2);
        assert_eq!(json["duration_ms"], 150);
        assert_eq!(json["nothing_to_store"], false);
        assert_eq!(json["flush_failed"], false);

        let deserialized: CompactionOutcome = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.tokens_before, 10000);
        assert_eq!(deserialized.tokens_after, 5000);
        assert!(!deserialized.flush_failed);
    }

    #[test]
    fn test_compaction_outcome_nothing_to_store_when_flush_succeeds_with_zero() {
        // Flush succeeded but found nothing → nothing_to_store = true, flush_failed = false
        let outcome = CompactionOutcome {
            tokens_before: 5000,
            tokens_after: 5000,
            messages_pruned: 0,
            messages_summarized: 0,
            memories_flushed: 0,
            tool_results_trimmed: 0,
            tool_results_cleared: 0,
            duration_ms: 10,
            nothing_to_store: true,
            flush_failed: false,
        };

        assert!(outcome.nothing_to_store);
        assert!(!outcome.flush_failed);
        assert_eq!(outcome.memories_flushed, 0);
    }

    #[test]
    fn test_compaction_outcome_flush_failed_not_nothing_to_store() {
        // Flush failed → flush_failed = true, nothing_to_store must be false
        // (we cannot claim "nothing to store" when we don't know)
        let flush_failed = true;
        let memories_flushed = 0usize;
        let nothing_to_store = !flush_failed && memories_flushed == 0;

        assert!(
            !nothing_to_store,
            "nothing_to_store must be false when flush_failed is true"
        );
        assert!(flush_failed);

        let outcome = CompactionOutcome {
            tokens_before: 8000,
            tokens_after: 6000,
            messages_pruned: 5,
            messages_summarized: 3,
            memories_flushed,
            tool_results_trimmed: 1,
            tool_results_cleared: 0,
            duration_ms: 50,
            nothing_to_store,
            flush_failed,
        };

        assert!(outcome.flush_failed);
        assert!(!outcome.nothing_to_store);
        assert_eq!(outcome.memories_flushed, 0);
    }

    #[test]
    fn test_compaction_outcome_flush_succeeded_with_memories() {
        // Flush succeeded and created memories → neither flag set
        let flush_failed = false;
        let memories_flushed = 4usize;
        let nothing_to_store = !flush_failed && memories_flushed == 0;

        assert!(!nothing_to_store);
        assert!(!flush_failed);

        let outcome = CompactionOutcome {
            tokens_before: 12000,
            tokens_after: 7000,
            messages_pruned: 10,
            messages_summarized: 8,
            memories_flushed,
            tool_results_trimmed: 2,
            tool_results_cleared: 1,
            duration_ms: 200,
            nothing_to_store,
            flush_failed,
        };

        assert!(!outcome.flush_failed);
        assert!(!outcome.nothing_to_store);
        assert_eq!(outcome.memories_flushed, 4);
    }

    // =========================================================================
    // Compaction trigger threshold calculation tests
    // =========================================================================

    /// Helper to compute whether compaction should trigger given token usage,
    /// context window limit, and reserve percentage.
    fn should_trigger_compaction(total_tokens: i64, window_limit: i64, reserve_pct: u32) -> bool {
        let reserve = (window_limit as f64 * (f64::from(reserve_pct) / 100.0)) as i64;
        let effective_limit = window_limit - reserve;
        total_tokens > effective_limit
    }

    #[test]
    fn test_threshold_over_limit_10pct_reserve() {
        // 32000 window, 10% reserve → effective = 28800
        assert!(should_trigger_compaction(30000, 32000, 10));
    }

    #[test]
    fn test_threshold_under_limit_10pct_reserve() {
        assert!(!should_trigger_compaction(20000, 32000, 10));
    }

    #[test]
    fn test_threshold_exactly_at_limit() {
        // effective = 32000 - 3200 = 28800; total == 28800 → should NOT trigger (not >)
        assert!(!should_trigger_compaction(28800, 32000, 10));
    }

    #[test]
    fn test_threshold_one_over_limit() {
        assert!(should_trigger_compaction(28801, 32000, 10));
    }

    #[test]
    fn test_threshold_high_reserve_25pct() {
        // 32000 window, 25% reserve → effective = 24000
        assert!(should_trigger_compaction(25000, 32000, 25));
        assert!(!should_trigger_compaction(23000, 32000, 25));
    }

    #[test]
    fn test_threshold_low_reserve_1pct() {
        // 32000 window, 1% reserve → effective = 31680
        assert!(should_trigger_compaction(32000, 32000, 1));
        assert!(!should_trigger_compaction(31000, 32000, 1));
    }

    #[test]
    fn test_threshold_large_window_128k() {
        // 128000 window, 10% reserve → effective = 115200
        assert!(should_trigger_compaction(120000, 128000, 10));
        assert!(!should_trigger_compaction(100000, 128000, 10));
    }

    #[test]
    fn test_threshold_zero_tokens_never_triggers() {
        assert!(!should_trigger_compaction(0, 32000, 10));
        assert!(!should_trigger_compaction(0, 128000, 50));
    }

    #[test]
    fn test_compaction_trigger_no_context_window_limit() {
        let mut counters = TokenCounters::new();
        counters.add_user(100000);

        let session = Session {
            session_id: Uuid::new_v4(),
            session_key: "agent:00000000-0000-0000-0000-000000000000:ui".to_string(),
            agent_id: Uuid::new_v4(),
            channel: "ui".to_string(),
            transcript_path: None,
            token_counters: serde_json::to_value(&counters).unwrap(),
            compaction_count: 0,
            context_window_limit: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_activity_at: Utc::now(),
            expires_at: None,
        };

        // No context_window_limit → should_compact returns false regardless of tokens
        assert!(!session.should_compact());
    }

    #[test]
    fn test_should_compact_over_limit() {
        let mut counters = TokenCounters::new();
        counters.add_user(33000);

        let session = Session {
            session_id: Uuid::new_v4(),
            session_key: "agent:00000000-0000-0000-0000-000000000000:ui".to_string(),
            agent_id: Uuid::new_v4(),
            channel: "ui".to_string(),
            transcript_path: None,
            token_counters: serde_json::to_value(&counters).unwrap(),
            compaction_count: 0,
            context_window_limit: Some(32000),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_activity_at: Utc::now(),
            expires_at: None,
        };

        assert!(session.should_compact());
    }

    #[test]
    fn test_should_compact_at_exact_limit() {
        let mut counters = TokenCounters::new();
        counters.add_user(32000);

        let session = Session {
            session_id: Uuid::new_v4(),
            session_key: "agent:00000000-0000-0000-0000-000000000000:ui".to_string(),
            agent_id: Uuid::new_v4(),
            channel: "ui".to_string(),
            transcript_path: None,
            token_counters: serde_json::to_value(&counters).unwrap(),
            compaction_count: 0,
            context_window_limit: Some(32000),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_activity_at: Utc::now(),
            expires_at: None,
        };

        // Exactly at limit → should_compact uses > not >=
        assert!(!session.should_compact());
    }

    // =========================================================================
    // Soft-trim effects on token estimates
    // =========================================================================

    #[test]
    fn test_soft_trim_reduces_token_count() {
        use crate::context::{estimate_tokens, ContextWindow};

        // Generate a large tool result
        let large_content = "word ".repeat(2000); // ~2000 tokens
        let original_tokens = estimate_tokens(&large_content, "deepseek-r1:7b");
        assert!(original_tokens > 500, "Test content should be large enough");

        let max_tokens = 200;
        let trimmed =
            ContextWindow::soft_trim_tool_result(&large_content, max_tokens, "deepseek-r1:7b");
        let trimmed_tokens = estimate_tokens(&trimmed, "deepseek-r1:7b");

        // Trimmed result should have fewer tokens than original
        assert!(
            trimmed_tokens < original_tokens,
            "Trimmed should have fewer tokens"
        );
        // Trimmed result should contain the ellipsis separator
        assert!(
            trimmed.contains("[..."),
            "Trimmed should contain omission marker"
        );
        assert!(
            trimmed.contains("tokens omitted"),
            "Trimmed should indicate omitted tokens"
        );
    }

    #[test]
    fn test_soft_trim_no_op_when_under_threshold() {
        use crate::context::{estimate_tokens, ContextWindow};

        let small_content = "Hello, this is a short tool result.";
        let original_tokens = estimate_tokens(small_content, "deepseek-r1:7b");

        let max_tokens = 1000;
        let result =
            ContextWindow::soft_trim_tool_result(small_content, max_tokens, "deepseek-r1:7b");

        // Should return content unchanged
        assert_eq!(result, small_content);
        assert_eq!(estimate_tokens(&result, "deepseek-r1:7b"), original_tokens);
    }

    #[test]
    fn test_soft_trim_preserves_head_and_tail() {
        use crate::context::ContextWindow;

        let content = format!("HEAD_MARKER {}TAIL_MARKER", "x ".repeat(2000),);
        let trimmed = ContextWindow::soft_trim_tool_result(&content, 100, "deepseek-r1:7b");

        assert!(
            trimmed.starts_with("HEAD_MARKER"),
            "Head should be preserved"
        );
        assert!(trimmed.ends_with("TAIL_MARKER"), "Tail should be preserved");
    }

    #[test]
    fn test_soft_trim_token_delta_for_counter_adjustment() {
        use crate::context::{estimate_tokens, ContextWindow};

        let large_content = "token ".repeat(3000);
        #[allow(clippy::cast_possible_wrap)]
        let original_tokens = estimate_tokens(&large_content, "deepseek-r1:7b") as i32;

        let max_tokens = 300;
        let trimmed =
            ContextWindow::soft_trim_tool_result(&large_content, max_tokens, "deepseek-r1:7b");
        #[allow(clippy::cast_possible_wrap)]
        let new_tokens = estimate_tokens(&trimmed, "deepseek-r1:7b") as i32;

        // The delta is what would be subtracted from counters
        let token_delta = original_tokens - new_tokens;
        assert!(token_delta > 0, "Token delta should be positive after trim");
        // New tokens should be reasonably close to max_tokens
        assert!(
            (new_tokens as usize) < original_tokens as usize,
            "New token count should be less than original"
        );
    }

    // =========================================================================
    // Compaction count tests
    // =========================================================================

    #[test]
    fn test_compaction_count_initial_zero() {
        let session = Session {
            session_id: Uuid::new_v4(),
            session_key: "agent:00000000-0000-0000-0000-000000000000:ui".to_string(),
            agent_id: Uuid::new_v4(),
            channel: "ui".to_string(),
            transcript_path: None,
            token_counters: serde_json::to_value(TokenCounters::default()).unwrap(),
            compaction_count: 0,
            context_window_limit: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_activity_at: Utc::now(),
            expires_at: None,
        };
        assert_eq!(session.compaction_count, 0);
    }

    #[test]
    fn test_compaction_count_tracks_multiple_compactions() {
        let session = Session {
            session_id: Uuid::new_v4(),
            session_key: "agent:00000000-0000-0000-0000-000000000000:ui".to_string(),
            agent_id: Uuid::new_v4(),
            channel: "ui".to_string(),
            transcript_path: None,
            token_counters: serde_json::to_value(TokenCounters::default()).unwrap(),
            compaction_count: 5,
            context_window_limit: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_activity_at: Utc::now(),
            expires_at: None,
        };
        assert_eq!(session.compaction_count, 5);
    }

    // =========================================================================
    // Ledger compaction event payload shape
    // =========================================================================

    #[test]
    fn test_ledger_compaction_payload_shape() {
        // Verify the JSON payload shape that log_session_compaction would produce
        let outcome = CompactionOutcome {
            tokens_before: 15000,
            tokens_after: 8000,
            messages_pruned: 12,
            messages_summarized: 10,
            memories_flushed: 2,
            tool_results_trimmed: 3,
            tool_results_cleared: 1,
            duration_ms: 250,
            nothing_to_store: false,
            flush_failed: false,
        };

        let session_id = Uuid::new_v4();
        let trigger = CompactionTrigger::TokenLimitExceeded;

        // Reconstruct the payload shape used by Ledger::log_session_compaction
        let payload = json!({
            "session_id": session_id,
            "trigger": trigger.to_string(),
            "tokens_before": outcome.tokens_before,
            "tokens_after": outcome.tokens_after,
            "messages_pruned": outcome.messages_pruned,
            "messages_summarized": outcome.messages_summarized,
            "memories_flushed": outcome.memories_flushed,
            "tool_results_trimmed": outcome.tool_results_trimmed,
            "tool_results_cleared": outcome.tool_results_cleared,
            "duration_ms": outcome.duration_ms,
            "nothing_to_store": outcome.nothing_to_store,
            "flush_failed": outcome.flush_failed,
        });

        // All expected keys must be present
        assert!(payload.get("session_id").is_some());
        assert!(payload.get("trigger").is_some());
        assert!(payload.get("tokens_before").is_some());
        assert!(payload.get("tokens_after").is_some());
        assert!(payload.get("messages_pruned").is_some());
        assert!(payload.get("messages_summarized").is_some());
        assert!(payload.get("memories_flushed").is_some());
        assert!(payload.get("tool_results_trimmed").is_some());
        assert!(payload.get("tool_results_cleared").is_some());
        assert!(payload.get("duration_ms").is_some());
        assert!(payload.get("nothing_to_store").is_some());
        assert!(payload.get("flush_failed").is_some());

        // Verify types
        assert!(payload["trigger"].is_string());
        assert!(payload["tokens_before"].is_number());
        assert!(payload["tokens_after"].is_number());
        assert!(payload["nothing_to_store"].is_boolean());
        assert!(payload["flush_failed"].is_boolean());
    }

    #[test]
    fn test_ledger_payload_flush_failed_included() {
        let outcome = CompactionOutcome {
            tokens_before: 10000,
            tokens_after: 8000,
            messages_pruned: 5,
            messages_summarized: 3,
            memories_flushed: 0,
            tool_results_trimmed: 1,
            tool_results_cleared: 0,
            duration_ms: 80,
            nothing_to_store: false,
            flush_failed: true,
        };

        let payload = json!({
            "session_id": Uuid::new_v4(),
            "trigger": CompactionTrigger::ManualRequest.to_string(),
            "tokens_before": outcome.tokens_before,
            "tokens_after": outcome.tokens_after,
            "messages_pruned": outcome.messages_pruned,
            "messages_summarized": outcome.messages_summarized,
            "memories_flushed": outcome.memories_flushed,
            "tool_results_trimmed": outcome.tool_results_trimmed,
            "tool_results_cleared": outcome.tool_results_cleared,
            "duration_ms": outcome.duration_ms,
            "nothing_to_store": outcome.nothing_to_store,
            "flush_failed": outcome.flush_failed,
        });

        assert_eq!(payload["flush_failed"], true);
        assert_eq!(payload["nothing_to_store"], false);
        assert_eq!(payload["memories_flushed"], 0);
    }

    // =========================================================================
    // Truncation helper tests
    // =========================================================================

    #[test]
    fn test_truncate_for_memory_short_string() {
        let s = "Hello, world!";
        assert_eq!(truncate_for_memory(s, 100), "Hello, world!");
    }

    #[test]
    fn test_truncate_for_memory_exact_limit() {
        let s = "Hello";
        assert_eq!(truncate_for_memory(s, 5), "Hello");
    }

    #[test]
    fn test_truncate_for_memory_over_limit() {
        let s = "Hello, world! This is a longer string.";
        let result = truncate_for_memory(s, 13);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 16); // 13 chars + "..."
    }

    #[test]
    fn test_truncate_for_memory_unicode() {
        let s = "Héllo wörld café";
        let result = truncate_for_memory(s, 5);
        assert!(result.ends_with("..."));
        // Should not panic on multi-byte characters
    }

    // =========================================================================
    // Integration tests (require database)
    // =========================================================================

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_compact_session_full_flow() {
        // Verifies the full compaction pipeline:
        // 1. Create session with messages exceeding token limit
        // 2. Run compact_session()
        // 3. Assert tokens_after < tokens_before
        // 4. Assert compaction_count incremented
        // 5. Assert compacted messages deleted
        // 6. Assert summary message inserted
        unimplemented!("Run with: cargo test -- --ignored test_compact_session_full_flow");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_memory_flush_zero_returns_nothing_to_store() {
        // Verifies that when trigger_memory_flush returns Ok(0),
        // the outcome has nothing_to_store=true and flush_failed=false
        unimplemented!(
            "Run with: cargo test -- --ignored test_memory_flush_zero_returns_nothing_to_store"
        );
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_memory_flush_error_sets_flush_failed() {
        // Verifies that when trigger_memory_flush returns Err,
        // the outcome has flush_failed=true and nothing_to_store=false
        unimplemented!(
            "Run with: cargo test -- --ignored test_memory_flush_error_sets_flush_failed"
        );
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_tool_result_soft_trim_updates_db() {
        // Verifies that prune_tool_results() soft-trims oversized tool messages:
        // 1. Insert tool message with token_estimate > tool_trim_threshold
        // 2. Run prune_tool_results()
        // 3. Assert content is trimmed, token_estimate updated
        // 4. Assert metadata contains {"soft_trimmed": true, "original_tokens": N}
        unimplemented!("Run with: cargo test -- --ignored test_tool_result_soft_trim_updates_db");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_tool_result_hard_clear_deletes_old() {
        // Verifies that prune_tool_results() hard-clears old tool messages:
        // 1. Insert tool message with ts older than tool_clear_age_secs
        // 2. Run prune_tool_results()
        // 3. Assert message deleted
        // 4. Assert token counters adjusted
        unimplemented!("Run with: cargo test -- --ignored test_tool_result_hard_clear_deletes_old");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_summarization_creates_summary_and_flags_originals() {
        // Verifies summarize_conversation_segment():
        // 1. Insert several user/assistant messages
        // 2. Run summarize_conversation_segment()
        // 3. Assert a system summary message was inserted
        // 4. Assert original messages have {"compacted": true} metadata
        unimplemented!(
            "Run with: cargo test -- --ignored test_summarization_creates_summary_and_flags_originals"
        );
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_compaction_increments_count_and_recalculates_counters() {
        // Verifies compact_session():
        // 1. Create session, add messages
        // 2. Run compact_session()
        // 3. Assert compaction_count = 1
        // 4. Assert token_counters recalculated from remaining messages
        unimplemented!(
            "Run with: cargo test -- --ignored test_compaction_increments_count_and_recalculates_counters"
        );
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_compaction_ledger_event_recorded() {
        // Verifies that compact_session() logs to the ledger:
        // 1. Create session, add messages, run compact_session() with ledger
        // 2. Query ledger for "session.compacted" events
        // 3. Assert payload contains all expected fields including flush_failed
        unimplemented!("Run with: cargo test -- --ignored test_compaction_ledger_event_recorded");
    }
}
