//! Memory Retrieval and Management System
//!
//! This module provides database-backed memory storage with pgvector similarity
//! search for agent knowledge persistence. Memories are stored in PostgreSQL with
//! optional 1536-dimension vector embeddings for semantic retrieval.
//!
//! ## Architecture
//!
//! - **CRUD**: Create, read, update, and delete memories with full audit trail
//! - **Retrieval**: Flexible querying by identity, source, importance, and time range
//! - **Similarity Search**: pgvector cosine similarity for semantic memory lookup
//! - **Access Tracking**: Automatic `accessed_at` / `access_count` updates on retrieval
//!
//! ## "Today + Yesterday" Load Policy
//!
//! The `load_recent_memories()` method implements a temporal heuristic that loads
//! memories created within the last 48 hours (today + yesterday). This provides
//! recent conversational context without overwhelming the context window.
//!
//! ## Memory Sources
//!
//! Memories are categorised by origin:
//! - `conversation` — extracted from chat interactions
//! - `task` — derived from completed task outcomes
//! - `observation` — agent-initiated environmental observations
//! - `reflection` — synthesised insights from multiple memories
//!
//! ## Importance Scoring
//!
//! Each memory carries an importance score on a 0.0–1.0 scale:
//! - **0.0–0.3**: Low importance (routine, ephemeral)
//! - **0.3–0.7**: Medium importance (useful context)
//! - **0.7–1.0**: High importance (critical preferences, key facts)
//! - Memories with importance > 0.8 are considered "high-importance"
//!
//! ## pgvector Integration
//!
//! The `memories` table includes a `vector(1536)` column indexed with ivfflat
//! (cosine similarity). The `search_memories()` method uses the `<=>` operator
//! for efficient approximate nearest-neighbour search.
//!
//! ## Embedding Service
//!
//! Embedding generation is defined as a trait (`EmbeddingService`) for future
//! integration with an LLM gateway. A placeholder stub is provided until the
//! embedding service is available.
//!
//! ## Example
//!
//! ```ignore
//! let manager = MemoryManager::new(pool, Some(event_stream));
//!
//! // Create a memory
//! let memory = manager.create_memory(
//!     identity_id,
//!     "User prefers concise responses",
//!     Some("Communication preference".to_string()),
//!     MemorySource::Conversation,
//!     None, // embedding to be added later
//!     0.9,  // high importance
//! ).await?;
//!
//! // Load recent memories (today + yesterday)
//! let recent = manager.load_recent_memories(identity_id, 50).await?;
//!
//! // Search by similarity (when embeddings are available)
//! let query = MemorySearchQuery {
//!     embedding: query_embedding,
//!     identity_id,
//!     min_similarity: 0.75,
//!     limit: 10,
//!     sources: Some(vec![MemorySource::Conversation]),
//! };
//! let results = manager.search_memories(query).await?;
//! ```

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use pgvector::Vector;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgPool, Row};
use uuid::Uuid;

use carnelian_common::{Error, Result};
use carnelian_common::types::{EventEnvelope, EventLevel, EventType};

use crate::events::EventStream;

// =============================================================================
// MEMORY SOURCE
// =============================================================================

/// Origin category for a memory record.
///
/// Matches the CHECK constraint on `memories.source`:
/// `('conversation', 'task', 'observation', 'reflection')`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemorySource {
    Conversation,
    Task,
    Observation,
    Reflection,
}

impl fmt::Display for MemorySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conversation => write!(f, "conversation"),
            Self::Task => write!(f, "task"),
            Self::Observation => write!(f, "observation"),
            Self::Reflection => write!(f, "reflection"),
        }
    }
}

impl FromStr for MemorySource {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "conversation" => Ok(Self::Conversation),
            "task" => Ok(Self::Task),
            "observation" => Ok(Self::Observation),
            "reflection" => Ok(Self::Reflection),
            _ => Err(Error::Memory(format!(
                "Invalid memory source '{}': must be one of conversation, task, observation, reflection",
                s
            ))),
        }
    }
}

// =============================================================================
// MEMORY
// =============================================================================

/// A memory record matching the `memories` database table.
///
/// Memories store agent knowledge with optional vector embeddings for
/// semantic retrieval. Each memory tracks its importance, origin source,
/// and access patterns.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Memory {
    /// Unique memory identifier
    pub memory_id: Uuid,
    /// Agent identity this memory belongs to
    pub identity_id: Uuid,
    /// Full memory content text
    pub content: String,
    /// Optional short summary of the memory
    pub summary: Option<String>,
    /// Origin category (conversation, task, observation, reflection)
    pub source: String,
    /// Optional 1536-dimension embedding vector for similarity search
    #[sqlx(skip)]
    pub embedding: Option<Vec<f32>>,
    /// Importance score (0.0–1.0)
    pub importance: f32,
    /// When the memory was created
    pub created_at: DateTime<Utc>,
    /// When the memory was last accessed
    pub accessed_at: DateTime<Utc>,
    /// Number of times this memory has been retrieved
    pub access_count: i32,
}

impl Memory {
    /// Parse the source string into a typed `MemorySource`.
    #[must_use]
    pub fn source_type(&self) -> Option<MemorySource> {
        MemorySource::from_str(&self.source).ok()
    }

    /// Returns true if this memory has a high importance score (> 0.8).
    #[must_use]
    pub fn is_high_importance(&self) -> bool {
        self.importance > 0.8
    }

    /// Returns true if this memory has an embedding vector.
    #[must_use]
    pub fn has_embedding(&self) -> bool {
        self.embedding.is_some()
    }
}

/// Validate that an importance score is within the allowed range.
fn validate_importance(importance: f32) -> Result<()> {
    if !(0.0..=1.0).contains(&importance) {
        return Err(Error::Memory(format!(
            "Importance must be between 0.0 and 1.0, got {}",
            importance
        )));
    }
    Ok(())
}

/// Validate that an embedding has the required 1536 dimensions.
fn validate_embedding_dimension(embedding: &[f32]) -> Result<()> {
    if embedding.len() != 1536 {
        return Err(Error::Memory(format!(
            "Embedding must have 1536 dimensions, got {}",
            embedding.len()
        )));
    }
    Ok(())
}

// =============================================================================
// MEMORY QUERY
// =============================================================================

/// Flexible query parameters for filtering memories.
///
/// Use the builder methods to construct a query incrementally.
///
/// # Example
///
/// ```ignore
/// let query = MemoryQuery::new()
///     .with_identity(agent_id)
///     .with_sources(vec![MemorySource::Conversation])
///     .with_min_importance(0.5)
///     .with_limit(25);
/// let memories = manager.query_memories(query).await?;
/// ```
#[derive(Debug, Clone, Default)]
pub struct MemoryQuery {
    /// Filter by agent identity
    pub identity_id: Option<Uuid>,
    /// Filter by one or more source categories
    pub sources: Option<Vec<MemorySource>>,
    /// Minimum importance threshold (inclusive)
    pub min_importance: Option<f32>,
    /// Only memories created at or after this timestamp
    pub since: Option<DateTime<Utc>>,
    /// Only memories created before this timestamp
    pub until: Option<DateTime<Utc>>,
    /// Maximum number of results (default: 50)
    pub limit: i64,
}

impl MemoryQuery {
    /// Create a new query with default limit of 50.
    #[must_use]
    pub fn new() -> Self {
        Self {
            limit: 50,
            ..Default::default()
        }
    }

    /// Filter by agent identity.
    #[must_use]
    pub fn with_identity(mut self, identity_id: Uuid) -> Self {
        self.identity_id = Some(identity_id);
        self
    }

    /// Filter by one or more source categories.
    #[must_use]
    pub fn with_sources(mut self, sources: Vec<MemorySource>) -> Self {
        self.sources = Some(sources);
        self
    }

    /// Set minimum importance threshold.
    #[must_use]
    pub fn with_min_importance(mut self, min: f32) -> Self {
        self.min_importance = Some(min);
        self
    }

    /// Only include memories created at or after this timestamp.
    #[must_use]
    pub fn with_since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    /// Only include memories created before this timestamp.
    #[must_use]
    pub fn with_until(mut self, until: DateTime<Utc>) -> Self {
        self.until = Some(until);
        self
    }

    /// Set the maximum number of results.
    #[must_use]
    pub fn with_limit(mut self, limit: i64) -> Self {
        self.limit = limit;
        self
    }
}

// =============================================================================
// MEMORY SEARCH QUERY
// =============================================================================

/// Parameters for pgvector cosine similarity search.
///
/// # Example
///
/// ```ignore
/// let search = MemorySearchQuery {
///     embedding: query_vec,
///     identity_id: agent_id,
///     min_similarity: 0.75,
///     limit: 10,
///     sources: None,
/// };
/// let results = manager.search_memories(search).await?;
/// ```
#[derive(Debug, Clone)]
pub struct MemorySearchQuery {
    /// Query embedding vector (must be 1536 dimensions)
    pub embedding: Vec<f32>,
    /// Agent identity to search within
    pub identity_id: Uuid,
    /// Minimum cosine similarity threshold (0.0–1.0, default: 0.7)
    pub min_similarity: f32,
    /// Maximum number of results (default: 20)
    pub limit: i64,
    /// Optional source filter
    pub sources: Option<Vec<MemorySource>>,
}

impl MemorySearchQuery {
    /// Create a search query with sensible defaults.
    #[must_use]
    pub fn new(embedding: Vec<f32>, identity_id: Uuid) -> Self {
        Self {
            embedding,
            identity_id,
            min_similarity: 0.7,
            limit: 20,
            sources: None,
        }
    }
}

// =============================================================================
// MEMORY STATS
// =============================================================================

/// Aggregate statistics for an agent's memories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Total number of memories
    pub total_count: i64,
    /// Average importance across all memories
    pub avg_importance: f64,
    /// Memory ID of the most-accessed memory (if any)
    pub most_accessed_id: Option<Uuid>,
    /// Highest access count
    pub most_accessed_count: Option<i32>,
    /// Count of memories per source category
    pub conversation_count: i64,
    pub task_count: i64,
    pub observation_count: i64,
    pub reflection_count: i64,
}

// =============================================================================
// EMBEDDING SERVICE TRAIT
// =============================================================================

/// Trait for generating vector embeddings from text.
///
/// This trait defines the interface for embedding generation that will be
/// implemented in a future phase when the LLM Gateway Service is available.
///
/// # Future Implementation
///
/// The concrete implementation will call an embedding model (e.g., OpenAI
/// text-embedding-3-small or a local model via Ollama) to produce 1536-dimension
/// vectors for semantic similarity search.
// TODO: Implement concrete EmbeddingService when LLM Gateway Service phase begins
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    /// Generate a 1536-dimension embedding vector from text.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to embed
    ///
    /// # Returns
    ///
    /// A 1536-element `Vec<f32>` representing the text embedding.
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>>;
}

// =============================================================================
// MEMORY MANAGER
// =============================================================================

/// Manages memory lifecycle, retrieval, and similarity search.
///
/// Follows the established manager pattern (see `SoulManager`, `SessionManager`)
/// with database-backed persistence and optional event stream integration.
pub struct MemoryManager {
    /// Database connection pool
    pool: PgPool,
    /// Optional event stream for audit trail
    event_stream: Option<Arc<EventStream>>,
}

impl MemoryManager {
    /// Create a new MemoryManager.
    ///
    /// # Arguments
    ///
    /// * `pool` - PostgreSQL connection pool
    /// * `event_stream` - Optional event stream for audit events
    pub fn new(pool: PgPool, event_stream: Option<Arc<EventStream>>) -> Self {
        Self { pool, event_stream }
    }

    // =========================================================================
    // CRUD OPERATIONS
    // =========================================================================

    /// Create a new memory record.
    ///
    /// Validates the importance range, inserts the memory into the database,
    /// and emits a `MemoryCreated` event.
    ///
    /// # Arguments
    ///
    /// * `identity_id` - Agent identity this memory belongs to
    /// * `content` - Full memory content text
    /// * `summary` - Optional short summary
    /// * `source` - Origin category
    /// * `embedding` - Optional 1536-dimension vector (validated if provided)
    /// * `importance` - Importance score (0.0–1.0)
    ///
    /// # Errors
    ///
    /// Returns an error if importance is out of range, embedding has wrong
    /// dimensions, or the database insert fails.
    pub async fn create_memory(
        &self,
        identity_id: Uuid,
        content: &str,
        summary: Option<String>,
        source: MemorySource,
        embedding: Option<Vec<f32>>,
        importance: f32,
    ) -> Result<Memory> {
        validate_importance(importance)?;

        if let Some(ref emb) = embedding {
            validate_embedding_dimension(emb)?;
        }

        let memory_id = Uuid::new_v4();
        let source_str = source.to_string();
        let pg_embedding = embedding.as_ref().map(|e| Vector::from(e.clone()));

        let row = sqlx::query(
            r"INSERT INTO memories (memory_id, identity_id, content, summary, source, embedding, importance)
              VALUES ($1, $2, $3, $4, $5, $6, $7)
              RETURNING memory_id, identity_id, content, summary, source, importance, created_at, accessed_at, access_count",
        )
        .bind(memory_id)
        .bind(identity_id)
        .bind(content)
        .bind(&summary)
        .bind(&source_str)
        .bind(pg_embedding)
        .bind(importance)
        .fetch_one(&self.pool)
        .await?;

        let memory = Memory {
            memory_id: row.get("memory_id"),
            identity_id: row.get("identity_id"),
            content: row.get("content"),
            summary: row.get("summary"),
            source: row.get("source"),
            embedding,
            importance: row.get("importance"),
            created_at: row.get("created_at"),
            accessed_at: row.get("accessed_at"),
            access_count: row.get("access_count"),
        };

        tracing::info!(
            memory_id = %memory_id,
            identity_id = %identity_id,
            source = %source_str,
            importance = importance,
            "Memory created"
        );

        self.emit_event(
            EventType::MemoryCreated,
            json!({
                "memory_id": memory_id,
                "identity_id": identity_id,
                "source": source_str,
                "importance": importance,
            }),
        );

        Ok(memory)
    }

    /// Retrieve a memory by ID.
    ///
    /// Updates `accessed_at` and increments `access_count` atomically on
    /// successful retrieval.
    ///
    /// Returns `None` if no memory exists with the given ID.
    pub async fn get_memory(&self, memory_id: Uuid) -> Result<Option<Memory>> {
        let row = sqlx::query(
            r"UPDATE memories
              SET accessed_at = NOW(), access_count = access_count + 1
              WHERE memory_id = $1
              RETURNING memory_id, identity_id, content, summary, source, importance, created_at, accessed_at, access_count",
        )
        .bind(memory_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        Ok(Some(Memory {
            memory_id: row.get("memory_id"),
            identity_id: row.get("identity_id"),
            content: row.get("content"),
            summary: row.get("summary"),
            source: row.get("source"),
            embedding: None,
            importance: row.get("importance"),
            created_at: row.get("created_at"),
            accessed_at: row.get("accessed_at"),
            access_count: row.get("access_count"),
        }))
    }

    /// Update mutable fields of an existing memory.
    ///
    /// Only provided fields are updated; `None` values are left unchanged.
    /// Emits a `MemoryUpdated` event on success.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - Memory to update
    /// * `content` - New content (if Some)
    /// * `summary` - New summary (if Some)
    /// * `importance` - New importance score (if Some, validated 0.0–1.0)
    pub async fn update_memory(
        &self,
        memory_id: Uuid,
        content: Option<&str>,
        summary: Option<Option<String>>,
        importance: Option<f32>,
    ) -> Result<Memory> {
        if let Some(imp) = importance {
            validate_importance(imp)?;
        }

        // Build dynamic SET clause
        let mut set_parts = Vec::new();
        let mut param_idx = 1u32;

        if content.is_some() {
            param_idx += 1;
            set_parts.push(format!("content = ${param_idx}"));
        }
        if summary.is_some() {
            param_idx += 1;
            set_parts.push(format!("summary = ${param_idx}"));
        }
        if importance.is_some() {
            param_idx += 1;
            set_parts.push(format!("importance = ${param_idx}"));
        }

        if set_parts.is_empty() {
            // Nothing to update, just fetch the current state
            return self.get_memory(memory_id).await?.ok_or_else(|| {
                Error::Memory(format!("Memory {} not found", memory_id))
            });
        }

        let sql = format!(
            "UPDATE memories SET {} WHERE memory_id = $1 RETURNING memory_id, identity_id, content, summary, source, importance, created_at, accessed_at, access_count",
            set_parts.join(", ")
        );

        let mut query = sqlx::query(&sql).bind(memory_id);

        if let Some(c) = content {
            query = query.bind(c);
        }
        if let Some(s) = &summary {
            query = query.bind(s);
        }
        if let Some(i) = importance {
            query = query.bind(i);
        }

        let row = query.fetch_optional(&self.pool).await?.ok_or_else(|| {
            Error::Memory(format!("Memory {} not found", memory_id))
        })?;

        let memory = Memory {
            memory_id: row.get("memory_id"),
            identity_id: row.get("identity_id"),
            content: row.get("content"),
            summary: row.get("summary"),
            source: row.get("source"),
            embedding: None,
            importance: row.get("importance"),
            created_at: row.get("created_at"),
            accessed_at: row.get("accessed_at"),
            access_count: row.get("access_count"),
        };

        tracing::info!(memory_id = %memory_id, "Memory updated");

        self.emit_event(
            EventType::MemoryUpdated,
            json!({
                "memory_id": memory_id,
                "identity_id": memory.identity_id,
            }),
        );

        Ok(memory)
    }

    /// Delete a memory by ID.
    ///
    /// Performs a hard delete. Emits a `MemoryDeleted` event on success.
    pub async fn delete_memory(&self, memory_id: Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM memories WHERE memory_id = $1")
            .bind(memory_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            tracing::warn!(memory_id = %memory_id, "Attempted to delete non-existent memory");
        } else {
            tracing::info!(memory_id = %memory_id, "Memory deleted");

            self.emit_event(
                EventType::MemoryDeleted,
                json!({"memory_id": memory_id}),
            );
        }

        Ok(())
    }

    // =========================================================================
    // MEMORY RETRIEVAL METHODS
    // =========================================================================

    /// Load recent memories using the "today + yesterday" heuristic.
    ///
    /// Retrieves memories created within the last 48 hours for the given agent,
    /// ordered by creation time (newest first). Updates access tracking for all
    /// returned memories in a single transaction.
    ///
    /// # Arguments
    ///
    /// * `identity_id` - Agent identity to load memories for
    /// * `limit` - Maximum number of memories to return
    pub async fn load_recent_memories(
        &self,
        identity_id: Uuid,
        limit: i64,
    ) -> Result<Vec<Memory>> {
        let since = Utc::now() - Duration::days(2);

        let rows = sqlx::query(
            r"UPDATE memories
              SET accessed_at = NOW(), access_count = access_count + 1
              WHERE memory_id IN (
                  SELECT memory_id FROM memories
                  WHERE identity_id = $1 AND created_at >= $2
                  ORDER BY created_at DESC
                  LIMIT $3
              )
              RETURNING memory_id, identity_id, content, summary, source, importance, created_at, accessed_at, access_count",
        )
        .bind(identity_id)
        .bind(since)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut memories: Vec<Memory> = rows
            .iter()
            .map(|row| Memory {
                memory_id: row.get("memory_id"),
                identity_id: row.get("identity_id"),
                content: row.get("content"),
                summary: row.get("summary"),
                source: row.get("source"),
                embedding: None,
                importance: row.get("importance"),
                created_at: row.get("created_at"),
                accessed_at: row.get("accessed_at"),
                access_count: row.get("access_count"),
            })
            .collect();

        // UPDATE...RETURNING does not preserve subquery ORDER BY
        memories.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(memories)
    }

    /// Load high-importance memories for an agent.
    ///
    /// Retrieves memories with importance above the given threshold, ordered
    /// by importance (descending) then creation time (newest first).
    ///
    /// # Arguments
    ///
    /// * `identity_id` - Agent identity
    /// * `min_importance` - Minimum importance threshold (default: 0.8)
    /// * `limit` - Maximum number of memories to return
    pub async fn load_high_importance_memories(
        &self,
        identity_id: Uuid,
        min_importance: f32,
        limit: i64,
    ) -> Result<Vec<Memory>> {
        let rows = sqlx::query(
            r"UPDATE memories
              SET accessed_at = NOW(), access_count = access_count + 1
              WHERE memory_id IN (
                  SELECT memory_id FROM memories
                  WHERE identity_id = $1 AND importance > $2
                  ORDER BY importance DESC, created_at DESC
                  LIMIT $3
              )
              RETURNING memory_id, identity_id, content, summary, source, importance, created_at, accessed_at, access_count",
        )
        .bind(identity_id)
        .bind(min_importance)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut memories: Vec<Memory> = rows
            .iter()
            .map(|row| Memory {
                memory_id: row.get("memory_id"),
                identity_id: row.get("identity_id"),
                content: row.get("content"),
                summary: row.get("summary"),
                source: row.get("source"),
                embedding: None,
                importance: row.get("importance"),
                created_at: row.get("created_at"),
                accessed_at: row.get("accessed_at"),
                access_count: row.get("access_count"),
            })
            .collect();

        // UPDATE...RETURNING does not preserve subquery ORDER BY
        memories.sort_by(|a, b| {
            b.importance.partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.created_at.cmp(&a.created_at))
        });

        Ok(memories)
    }

    /// Query memories with flexible filters.
    ///
    /// Builds a dynamic SQL query based on the provided `MemoryQuery` filters.
    /// Results are ordered by `created_at DESC` and limited by `query.limit`.
    ///
    /// # Arguments
    ///
    /// * `query` - Filter parameters (see `MemoryQuery` builder methods)
    #[allow(clippy::too_many_lines)]
    pub async fn query_memories(&self, query: MemoryQuery) -> Result<Vec<Memory>> {
        let mut conditions = Vec::new();
        let mut bind_idx = 0u32;

        // Build WHERE conditions dynamically
        // We'll use a manual approach with format strings for the dynamic parts,
        // but bind values safely via sqlx.

        if query.identity_id.is_some() {
            bind_idx += 1;
            conditions.push(format!("identity_id = ${bind_idx}"));
        }

        if let Some(ref sources) = query.sources {
            if !sources.is_empty() {
                bind_idx += 1;
                conditions.push(format!("source = ANY(${bind_idx})"));
            }
        }

        if query.min_importance.is_some() {
            bind_idx += 1;
            conditions.push(format!("importance >= ${bind_idx}"));
        }

        if query.since.is_some() {
            bind_idx += 1;
            conditions.push(format!("created_at >= ${bind_idx}"));
        }

        if query.until.is_some() {
            bind_idx += 1;
            conditions.push(format!("created_at < ${bind_idx}"));
        }

        bind_idx += 1;
        let limit_idx = bind_idx;

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT memory_id, identity_id, content, summary, source, importance, created_at, accessed_at, access_count \
             FROM memories {where_clause} ORDER BY created_at DESC LIMIT ${limit_idx}"
        );

        let mut db_query = sqlx::query(&sql);

        if let Some(id) = query.identity_id {
            db_query = db_query.bind(id);
        }

        if let Some(ref sources) = query.sources {
            if !sources.is_empty() {
                let source_strings: Vec<String> = sources.iter().map(ToString::to_string).collect();
                db_query = db_query.bind(source_strings);
            }
        }

        if let Some(min_imp) = query.min_importance {
            db_query = db_query.bind(min_imp);
        }

        if let Some(since) = query.since {
            db_query = db_query.bind(since);
        }

        if let Some(until) = query.until {
            db_query = db_query.bind(until);
        }

        db_query = db_query.bind(query.limit);

        let rows = db_query.fetch_all(&self.pool).await?;

        let memories: Vec<Memory> = rows
            .iter()
            .map(|row| Memory {
                memory_id: row.get("memory_id"),
                identity_id: row.get("identity_id"),
                content: row.get("content"),
                summary: row.get("summary"),
                source: row.get("source"),
                embedding: None,
                importance: row.get("importance"),
                created_at: row.get("created_at"),
                accessed_at: row.get("accessed_at"),
                access_count: row.get("access_count"),
            })
            .collect();

        Ok(memories)
    }

    // =========================================================================
    // PGVECTOR SIMILARITY SEARCH
    // =========================================================================

    /// Search memories by cosine similarity using pgvector.
    ///
    /// Uses the `<=>` cosine distance operator. pgvector returns distance
    /// (0 = identical, 2 = opposite), so `min_similarity` is converted to
    /// a maximum distance threshold: `max_distance = 1.0 - min_similarity`.
    ///
    /// Results include the similarity score (1.0 - distance) alongside each
    /// memory. Access tracking is updated for all returned memories.
    ///
    /// # Arguments
    ///
    /// * `search_query` - Search parameters including embedding, identity, and thresholds
    ///
    /// # Returns
    ///
    /// A vector of `(Memory, f32)` tuples where the `f32` is the cosine similarity score.
    #[allow(clippy::too_many_lines)]
    pub async fn search_memories(
        &self,
        search_query: MemorySearchQuery,
    ) -> Result<Vec<(Memory, f32)>> {
        validate_embedding_dimension(&search_query.embedding)?;

        let max_distance = 1.0 - search_query.min_similarity;
        let pg_embedding = Vector::from(search_query.embedding.clone());

        let rows = if let Some(ref sources) = search_query.sources {
            if !sources.is_empty() {
                let source_strings: Vec<String> = sources.iter().map(ToString::to_string).collect();
                sqlx::query(
                    r"SELECT memory_id, identity_id, content, summary, source, importance,
                             created_at, accessed_at, access_count,
                             (embedding <=> $1::vector) AS distance
                      FROM memories
                      WHERE identity_id = $2
                        AND embedding IS NOT NULL
                        AND (embedding <=> $1::vector) < $3
                        AND source = ANY($5)
                      ORDER BY embedding <=> $1::vector
                      LIMIT $4",
                )
                .bind(&pg_embedding)
                .bind(search_query.identity_id)
                .bind(max_distance)
                .bind(search_query.limit)
                .bind(&source_strings)
                .fetch_all(&self.pool)
                .await?
            } else {
                sqlx::query(
                    r"SELECT memory_id, identity_id, content, summary, source, importance,
                             created_at, accessed_at, access_count,
                             (embedding <=> $1::vector) AS distance
                      FROM memories
                      WHERE identity_id = $2
                        AND embedding IS NOT NULL
                        AND (embedding <=> $1::vector) < $3
                      ORDER BY embedding <=> $1::vector
                      LIMIT $4",
                )
                .bind(&pg_embedding)
                .bind(search_query.identity_id)
                .bind(max_distance)
                .bind(search_query.limit)
                .fetch_all(&self.pool)
                .await?
            }
        } else {
            sqlx::query(
                r"SELECT memory_id, identity_id, content, summary, source, importance,
                         created_at, accessed_at, access_count,
                         (embedding <=> $1::vector) AS distance
                  FROM memories
                  WHERE identity_id = $2
                    AND embedding IS NOT NULL
                    AND (embedding <=> $1::vector) < $3
                  ORDER BY embedding <=> $1::vector
                  LIMIT $4",
            )
            .bind(&pg_embedding)
            .bind(search_query.identity_id)
            .bind(max_distance)
            .bind(search_query.limit)
            .fetch_all(&self.pool)
            .await?
        };

        let mut results = Vec::with_capacity(rows.len());
        let mut memory_ids = Vec::with_capacity(rows.len());

        for row in &rows {
            let distance: f32 = row.get("distance");
            let similarity = 1.0 - distance;
            let memory_id: Uuid = row.get("memory_id");
            memory_ids.push(memory_id);

            results.push((
                Memory {
                    memory_id,
                    identity_id: row.get("identity_id"),
                    content: row.get("content"),
                    summary: row.get("summary"),
                    source: row.get("source"),
                    embedding: None,
                    importance: row.get("importance"),
                    created_at: row.get("created_at"),
                    accessed_at: row.get("accessed_at"),
                    access_count: row.get("access_count"),
                },
                similarity,
            ));
        }

        // Batch update access tracking for all results
        if !memory_ids.is_empty() {
            sqlx::query(
                "UPDATE memories SET accessed_at = NOW(), access_count = access_count + 1 WHERE memory_id = ANY($1)",
            )
            .bind(&memory_ids)
            .execute(&self.pool)
            .await?;
        }

        self.emit_event(
            EventType::MemorySearchPerformed,
            json!({
                "identity_id": search_query.identity_id,
                "result_count": results.len(),
                "min_similarity": search_query.min_similarity,
            }),
        );

        Ok(results)
    }

    // =========================================================================
    // EMBEDDING MANAGEMENT
    // =========================================================================

    /// Add or replace the embedding vector for an existing memory.
    ///
    /// Validates that the embedding has exactly 1536 dimensions before updating.
    /// Emits a `MemoryEmbeddingAdded` event on success.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - Memory to update
    /// * `embedding` - 1536-dimension vector
    pub async fn add_embedding_to_memory(
        &self,
        memory_id: Uuid,
        embedding: Vec<f32>,
    ) -> Result<()> {
        validate_embedding_dimension(&embedding)?;

        let pg_embedding = Vector::from(embedding);

        sqlx::query("UPDATE memories SET embedding = $1 WHERE memory_id = $2")
            .bind(&pg_embedding)
            .bind(memory_id)
            .execute(&self.pool)
            .await?;

        tracing::info!(memory_id = %memory_id, "Embedding added to memory");

        self.emit_event(
            EventType::MemoryEmbeddingAdded,
            json!({"memory_id": memory_id}),
        );

        Ok(())
    }

    /// Generate and add an embedding for a memory (stub).
    ///
    /// This method will be implemented when the embedding service is available.
    // TODO: Implement when LLM Gateway Service phase begins
    #[allow(clippy::unused_async)]
    pub async fn generate_and_add_embedding(&self, _memory_id: Uuid) -> Result<()> {
        Err(Error::Config(
            "Embedding service not yet implemented. This will be available in the LLM Gateway Service phase.".to_string(),
        ))
    }

    // =========================================================================
    // ACCESS TRACKING
    // =========================================================================

    /// Update access tracking for a single memory.
    ///
    /// Sets `accessed_at` to NOW() and increments `access_count` by 1.
    pub async fn update_access_count(&self, memory_id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE memories SET accessed_at = NOW(), access_count = access_count + 1 WHERE memory_id = $1",
        )
        .bind(memory_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get aggregate statistics for an agent's memories.
    ///
    /// Returns total count, average importance, most-accessed memory,
    /// and per-source distribution.
    pub async fn get_memory_stats(&self, identity_id: Uuid) -> Result<MemoryStats> {
        // Aggregate counts and importance
        let agg_row = sqlx::query(
            r"SELECT
                COUNT(*) AS total_count,
                COALESCE(AVG(importance::double precision), 0.0) AS avg_importance,
                COUNT(*) FILTER (WHERE source = 'conversation') AS conversation_count,
                COUNT(*) FILTER (WHERE source = 'task') AS task_count,
                COUNT(*) FILTER (WHERE source = 'observation') AS observation_count,
                COUNT(*) FILTER (WHERE source = 'reflection') AS reflection_count
              FROM memories
              WHERE identity_id = $1",
        )
        .bind(identity_id)
        .fetch_one(&self.pool)
        .await?;

        let total_count: i64 = agg_row.get("total_count");
        let avg_importance: f64 = agg_row.get("avg_importance");
        let conversation_count: i64 = agg_row.get("conversation_count");
        let task_count: i64 = agg_row.get("task_count");
        let observation_count: i64 = agg_row.get("observation_count");
        let reflection_count: i64 = agg_row.get("reflection_count");

        // Most accessed memory
        let most_accessed = sqlx::query(
            r"SELECT memory_id, access_count
              FROM memories
              WHERE identity_id = $1
              ORDER BY access_count DESC
              LIMIT 1",
        )
        .bind(identity_id)
        .fetch_optional(&self.pool)
        .await?;

        let (most_accessed_id, most_accessed_count) = most_accessed.map_or(
            (None, None),
            |row| (
                Some(row.get::<Uuid, _>("memory_id")),
                Some(row.get::<i32, _>("access_count")),
            ),
        );

        Ok(MemoryStats {
            total_count,
            avg_importance,
            most_accessed_id,
            most_accessed_count,
            conversation_count,
            task_count,
            observation_count,
            reflection_count,
        })
    }

    // =========================================================================
    // EVENT EMISSION
    // =========================================================================

    /// Emit an event to the event stream (if available).
    fn emit_event(&self, event_type: EventType, payload: serde_json::Value) {
        if let Some(ref es) = self.event_stream {
            es.publish(EventEnvelope::new(EventLevel::Info, event_type, payload));
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // MemorySource tests
    // =========================================================================

    #[test]
    fn test_memory_source_display() {
        assert_eq!(MemorySource::Conversation.to_string(), "conversation");
        assert_eq!(MemorySource::Task.to_string(), "task");
        assert_eq!(MemorySource::Observation.to_string(), "observation");
        assert_eq!(MemorySource::Reflection.to_string(), "reflection");
    }

    #[test]
    fn test_memory_source_from_str() {
        assert_eq!(
            MemorySource::from_str("conversation").unwrap(),
            MemorySource::Conversation
        );
        assert_eq!(
            MemorySource::from_str("task").unwrap(),
            MemorySource::Task
        );
        assert_eq!(
            MemorySource::from_str("observation").unwrap(),
            MemorySource::Observation
        );
        assert_eq!(
            MemorySource::from_str("reflection").unwrap(),
            MemorySource::Reflection
        );
    }

    #[test]
    fn test_memory_source_from_str_case_insensitive() {
        assert_eq!(
            MemorySource::from_str("CONVERSATION").unwrap(),
            MemorySource::Conversation
        );
        assert_eq!(
            MemorySource::from_str("Task").unwrap(),
            MemorySource::Task
        );
    }

    #[test]
    fn test_memory_source_from_str_invalid() {
        assert!(MemorySource::from_str("unknown").is_err());
        assert!(MemorySource::from_str("").is_err());
    }

    #[test]
    fn test_memory_source_roundtrip() {
        for source in &[
            MemorySource::Conversation,
            MemorySource::Task,
            MemorySource::Observation,
            MemorySource::Reflection,
        ] {
            let s = source.to_string();
            let parsed = MemorySource::from_str(&s).unwrap();
            assert_eq!(&parsed, source);
        }
    }

    #[test]
    fn test_memory_source_serialization() {
        let source = MemorySource::Conversation;
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, r#""conversation""#);

        let deserialized: MemorySource = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, source);
    }

    // =========================================================================
    // Importance validation tests
    // =========================================================================

    #[test]
    fn test_validate_importance_valid() {
        assert!(validate_importance(0.0).is_ok());
        assert!(validate_importance(0.5).is_ok());
        assert!(validate_importance(1.0).is_ok());
        assert!(validate_importance(0.001).is_ok());
        assert!(validate_importance(0.999).is_ok());
    }

    #[test]
    fn test_validate_importance_invalid() {
        assert!(validate_importance(-0.1).is_err());
        assert!(validate_importance(1.1).is_err());
        assert!(validate_importance(-1.0).is_err());
        assert!(validate_importance(2.0).is_err());
    }

    // =========================================================================
    // Embedding validation tests
    // =========================================================================

    #[test]
    fn test_validate_embedding_dimension_valid() {
        let embedding = vec![0.0f32; 1536];
        assert!(validate_embedding_dimension(&embedding).is_ok());
    }

    #[test]
    fn test_validate_embedding_dimension_too_short() {
        let embedding = vec![0.0f32; 100];
        assert!(validate_embedding_dimension(&embedding).is_err());
    }

    #[test]
    fn test_validate_embedding_dimension_too_long() {
        let embedding = vec![0.0f32; 2000];
        assert!(validate_embedding_dimension(&embedding).is_err());
    }

    #[test]
    fn test_validate_embedding_dimension_empty() {
        let embedding: Vec<f32> = vec![];
        assert!(validate_embedding_dimension(&embedding).is_err());
    }

    // =========================================================================
    // Memory helper tests
    // =========================================================================

    #[test]
    fn test_memory_is_high_importance() {
        let memory = Memory {
            memory_id: Uuid::new_v4(),
            identity_id: Uuid::new_v4(),
            content: "test".to_string(),
            summary: None,
            source: "conversation".to_string(),
            embedding: None,
            importance: 0.9,
            created_at: Utc::now(),
            accessed_at: Utc::now(),
            access_count: 0,
        };
        assert!(memory.is_high_importance());
    }

    #[test]
    fn test_memory_is_not_high_importance() {
        let memory = Memory {
            memory_id: Uuid::new_v4(),
            identity_id: Uuid::new_v4(),
            content: "test".to_string(),
            summary: None,
            source: "conversation".to_string(),
            embedding: None,
            importance: 0.5,
            created_at: Utc::now(),
            accessed_at: Utc::now(),
            access_count: 0,
        };
        assert!(!memory.is_high_importance());
    }

    #[test]
    fn test_memory_has_embedding() {
        let mut memory = Memory {
            memory_id: Uuid::new_v4(),
            identity_id: Uuid::new_v4(),
            content: "test".to_string(),
            summary: None,
            source: "conversation".to_string(),
            embedding: None,
            importance: 0.5,
            created_at: Utc::now(),
            accessed_at: Utc::now(),
            access_count: 0,
        };
        assert!(!memory.has_embedding());

        memory.embedding = Some(vec![0.0; 1536]);
        assert!(memory.has_embedding());
    }

    #[test]
    fn test_memory_source_type() {
        let memory = Memory {
            memory_id: Uuid::new_v4(),
            identity_id: Uuid::new_v4(),
            content: "test".to_string(),
            summary: None,
            source: "task".to_string(),
            embedding: None,
            importance: 0.5,
            created_at: Utc::now(),
            accessed_at: Utc::now(),
            access_count: 0,
        };
        assert_eq!(memory.source_type(), Some(MemorySource::Task));
    }

    #[test]
    fn test_memory_source_type_invalid() {
        let memory = Memory {
            memory_id: Uuid::new_v4(),
            identity_id: Uuid::new_v4(),
            content: "test".to_string(),
            summary: None,
            source: "invalid".to_string(),
            embedding: None,
            importance: 0.5,
            created_at: Utc::now(),
            accessed_at: Utc::now(),
            access_count: 0,
        };
        assert_eq!(memory.source_type(), None);
    }

    // =========================================================================
    // MemoryQuery builder tests
    // =========================================================================

    #[test]
    fn test_memory_query_defaults() {
        let query = MemoryQuery::new();
        assert_eq!(query.limit, 50);
        assert!(query.identity_id.is_none());
        assert!(query.sources.is_none());
        assert!(query.min_importance.is_none());
        assert!(query.since.is_none());
        assert!(query.until.is_none());
    }

    #[test]
    fn test_memory_query_builder() {
        let id = Uuid::new_v4();
        let since = Utc::now() - Duration::days(7);
        let query = MemoryQuery::new()
            .with_identity(id)
            .with_sources(vec![MemorySource::Conversation, MemorySource::Task])
            .with_min_importance(0.5)
            .with_since(since)
            .with_limit(25);

        assert_eq!(query.identity_id, Some(id));
        assert_eq!(
            query.sources,
            Some(vec![MemorySource::Conversation, MemorySource::Task])
        );
        assert_eq!(query.min_importance, Some(0.5));
        assert_eq!(query.since, Some(since));
        assert_eq!(query.limit, 25);
    }

    // =========================================================================
    // MemorySearchQuery tests
    // =========================================================================

    #[test]
    fn test_memory_search_query_defaults() {
        let id = Uuid::new_v4();
        let embedding = vec![0.0f32; 1536];
        let query = MemorySearchQuery::new(embedding.clone(), id);

        assert_eq!(query.identity_id, id);
        assert_eq!(query.embedding.len(), 1536);
        assert!((query.min_similarity - 0.7).abs() < f32::EPSILON);
        assert_eq!(query.limit, 20);
        assert!(query.sources.is_none());
    }

    // =========================================================================
    // Integration tests (require database)
    // =========================================================================

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_create_and_retrieve_memory() {
        // This test requires a running PostgreSQL instance with the schema applied
        unimplemented!("Run with: cargo test --test memory -- --ignored");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_load_recent_memories_today_yesterday() {
        unimplemented!("Run with: cargo test --test memory -- --ignored");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_load_high_importance_memories() {
        unimplemented!("Run with: cargo test --test memory -- --ignored");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_search_memories_similarity() {
        unimplemented!("Run with: cargo test --test memory -- --ignored");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_update_access_count() {
        unimplemented!("Run with: cargo test --test memory -- --ignored");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_query_memories_with_filters() {
        unimplemented!("Run with: cargo test --test memory -- --ignored");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_memory_stats() {
        unimplemented!("Run with: cargo test --test memory -- --ignored");
    }
}
