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
//! ## Example
//!
//! ```ignore
//! let manager = SessionManager::new(pool, Some(event_stream), None, 24);
//! let session = manager.create_session("agent:uuid:ui:group:main").await?;
//! manager.append_message(session.session_id, "user", "Hello".to_string(), Some(5), None, None, None).await?;
//! let messages = manager.load_messages(session.session_id, None, None).await?;
//! ```

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use tokio::io::AsyncWriteExt;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use carnelian_common::{Error, Result};
use carnelian_common::types::{EventEnvelope, EventLevel, EventType};

use crate::events::EventStream;
use crate::ledger::Ledger;

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
            return Err(Error::Session(
                "Empty channel in session key".to_string(),
            ));
        }

        // Optional group: parts[3] == "group", parts[4] == id
        let group_id = if parts.len() >= 5 && parts[3] == "group" {
            let gid = parts[4..].join(":");
            if gid.is_empty() {
                return Err(Error::Session(
                    "Empty group ID in session key".to_string(),
                ));
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
        self.expires_at
            .is_some_and(|exp| exp < Utc::now())
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
        }
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
        let session: Option<Session> = sqlx::query_as(
            "SELECT * FROM sessions WHERE session_key = $1",
        )
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
        let result = sqlx::query(
            "DELETE FROM session_messages WHERE session_id = $1 AND ts < $2",
        )
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
    pub async fn update_counters(
        &self,
        session_id: Uuid,
        counters: &TokenCounters,
    ) -> Result<()> {
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

        let mut counters: TokenCounters =
            serde_json::from_value(counters_json).unwrap_or_default();
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
        let counters_json: JsonValue = sqlx::query_scalar(
            "SELECT token_counters FROM sessions WHERE session_id = $1",
        )
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
        let result = sqlx::query(
            "DELETE FROM sessions WHERE expires_at IS NOT NULL AND expires_at < NOW()",
        )
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
    pub async fn extend_session(
        &self,
        session_id: Uuid,
        additional_hours: u32,
    ) -> Result<()> {
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
        let transcripts_dir = self.transcripts_path.as_ref().ok_or_else(|| {
            Error::Session("Transcripts path not configured".to_string())
        })?;

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
        let content = tokio::fs::read_to_string(transcript_path).await.map_err(|e| {
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
        let session: Session = sqlx::query_as(
            "SELECT * FROM sessions WHERE session_id = $1",
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await?;

        let transcript_path = session.transcript_path.as_ref().ok_or_else(|| {
            Error::Session(format!(
                "Session {} has no transcript_path set",
                session_id
            ))
        })?;

        // Load existing transcript to find last timestamp
        let existing = self.load_transcript_from_file(transcript_path).await.unwrap_or_default();
        let last_ts = existing.last().map(|m| m.ts);

        // Load new messages from database
        let new_messages: Vec<SessionMessage> = if let Some(since) = last_ts {
            self.load_messages_since(session_id, since).await?
        } else {
            sqlx::query_as(
                "SELECT * FROM session_messages WHERE session_id = $1 ORDER BY ts ASC",
            )
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
    // HELPER METHODS AND UTILITIES
    // =========================================================================

    /// Emit an event to the event stream (if available).
    fn emit_event(&self, event_type: EventType, payload: JsonValue) {
        if let Some(ref es) = self.event_stream {
            es.publish(EventEnvelope::new(EventLevel::Info, event_type, payload));
        }
    }

    /// List active (non-expired) sessions, optionally filtered by agent_id.
    pub async fn list_active_sessions(
        &self,
        agent_id: Option<Uuid>,
    ) -> Result<Vec<Session>> {
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

        let session: Session = sqlx::query_as(
            "SELECT * FROM sessions WHERE session_id = $1",
        )
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
                    "session_key": session.session_key,
                    "channel": session.channel,
                }),
                correlation_id,
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
                    "reason": reason,
                    "final_token_counters": final_counters,
                    "message_count": message_count,
                }),
                None,
            )
            .await
    }

    /// Log a compaction event to the audit ledger.
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
                    "before_counters": before_counters,
                    "after_counters": after_counters,
                    "messages_removed": messages_removed,
                }),
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
            serde_json::from_str(r#"{"total": 0, "user": 0, "assistant": 0, "tool": 0}"#)
                .unwrap();
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
}
