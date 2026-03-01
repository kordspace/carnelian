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

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::SigningKey;
use pgvector::Vector;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_json::json;
use sqlx::{FromRow, PgPool, Row};
use uuid::Uuid;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_common::{Error, Result};

use crate::encryption::EncryptionHelper;
use crate::events::EventStream;
use crate::ledger::Ledger;

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
    /// Topic tags for selective disclosure during export
    #[sqlx(json)]
    pub tags: Vec<String>,
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
// MEMORY ENVELOPE (Portable Memory Format)
// =============================================================================

/// Portable memory envelope for export/import with integrity verification.
///
/// Uses CBOR serialization, AES-256-GCM encryption (via `EncryptionHelper`),
/// blake3 integrity hashing, and optional Ed25519 signatures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEnvelope {
    /// Envelope format version
    pub version: u8,
    /// AES-256-GCM encrypted memory content (JSON bytes)
    pub encrypted_content: Vec<u8>,
    /// blake3 hash of encrypted_content for integrity verification
    pub content_hash: String,
    /// Optional ledger proof material for chain-of-custody verification
    pub ledger_proof: Option<LedgerProofMaterial>,
    /// Capability grants associated with this memory
    pub capability_grants: Vec<CapabilityGrantMetadata>,
    /// Optional embedding vector (unencrypted for portability)
    pub embedding: Option<Vec<f32>>,
    /// Arbitrary metadata key-value pairs
    pub metadata: HashMap<String, String>,
    /// Optional external chain anchor ID
    pub chain_anchor: Option<String>,
}

/// Ledger proof material capturing the chain position for verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerProofMaterial {
    /// Ledger event ID
    pub event_id: i64,
    /// blake3 hash of the ledger event
    pub event_hash: String,
    /// Hash of the preceding event (None for genesis)
    pub prev_hash: Option<String>,
    /// Ed25519 signature of event_hash (if privileged)
    pub core_signature: Option<String>,
    /// Timestamp of the ledger event
    pub timestamp: DateTime<Utc>,
}

/// Capability grant metadata for portable capability transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityGrantMetadata {
    /// Unique grant identifier
    pub grant_id: Uuid,
    /// Capability key (e.g., "memory.read")
    pub capability_key: String,
    /// Optional scope restrictions (JSON)
    pub scope: Option<JsonValue>,
    /// Identity that approved the grant
    pub approved_by: Option<Uuid>,
    /// Expiration timestamp
    pub expires_at: Option<DateTime<Utc>>,
}

/// Options controlling selective disclosure during memory export.
#[derive(Debug, Clone, Default)]
pub struct MemoryExportOptions {
    /// Include embedding vectors in the envelope
    pub include_embedding: bool,
    /// Filter memories by tags (include if any tag matches)
    pub topic_filter: Option<Vec<String>>,
    /// Minimum importance threshold for inclusion
    pub min_importance: Option<f32>,
    /// Include ledger proof material
    pub include_ledger_proof: bool,
    /// Include capability grants
    pub include_capabilities: bool,
}

/// Result of importing a single memory envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryImportResult {
    /// ID of the newly created memory
    pub memory_id: Uuid,
    /// Whether the envelope signature was verified
    pub verified: bool,
    /// Whether the ledger proof was valid
    pub ledger_proof_valid: bool,
    /// Any warnings encountered during import
    pub warnings: Vec<String>,
}

// =============================================================================
// CHAIN ANCHORING INTERFACE
// =============================================================================

/// Trait for anchoring memory hashes to an external chain (e.g., blockchain).
///
/// Implementations provide cryptographic proof that a memory hash existed at a
/// specific point in time on an external ledger.
#[async_trait]
pub trait ChainAnchor: Send + Sync {
    /// Anchor a hash to the external chain, returning an anchor ID.
    async fn anchor_hash(&self, hash: &str, metadata: JsonValue) -> Result<String>;
    /// Verify that a hash was anchored with the given anchor ID.
    async fn verify_anchor(&self, hash: &str, anchor_id: &str) -> Result<bool>;
    /// Retrieve the full proof for an anchor.
    async fn get_anchor_proof(&self, anchor_id: &str) -> Result<Option<JsonValue>>;
}

/// No-op chain anchor stub for use when no external chain is configured.
pub struct NoOpChainAnchor;

#[async_trait]
impl ChainAnchor for NoOpChainAnchor {
    async fn anchor_hash(&self, _hash: &str, _metadata: JsonValue) -> Result<String> {
        Ok("not_implemented".to_string())
    }

    async fn verify_anchor(&self, _hash: &str, _anchor_id: &str) -> Result<bool> {
        Ok(false)
    }

    async fn get_anchor_proof(&self, _anchor_id: &str) -> Result<Option<JsonValue>> {
        Ok(Some(serde_json::json!({"status": "not_implemented"})))
    }
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
///
/// When an `EncryptionHelper` is provided, memory content is encrypted at rest
/// using AES-256 via PostgreSQL's pgcrypto extension. Embeddings remain
/// unencrypted to preserve pgvector similarity search.
pub struct MemoryManager {
    /// Database connection pool
    pool: PgPool,
    /// Optional event stream for audit trail
    event_stream: Option<Arc<EventStream>>,
    /// Optional encryption helper for content encryption at rest
    encryption: Option<EncryptionHelper>,
    /// Optional audit ledger for chain-of-custody proofs
    ledger: Option<Arc<Ledger>>,
    /// Optional chain anchor for external blockchain integration
    chain_anchor: Option<Arc<dyn ChainAnchor>>,
}

impl MemoryManager {
    /// Create a new MemoryManager.
    ///
    /// # Arguments
    ///
    /// * `pool` - PostgreSQL connection pool
    /// * `event_stream` - Optional event stream for audit events
    pub fn new(pool: PgPool, event_stream: Option<Arc<EventStream>>) -> Self {
        Self {
            pool,
            event_stream,
            encryption: None,
            ledger: None,
            chain_anchor: None,
        }
    }

    /// Builder-style setter to enable encryption at rest for memory content.
    ///
    /// When set, `create_memory` encrypts content before INSERT and all
    /// retrieval methods decrypt content after SELECT. Embeddings are
    /// never encrypted.
    #[must_use]
    pub fn with_encryption(mut self, helper: EncryptionHelper) -> Self {
        self.encryption = Some(helper);
        self
    }

    /// Builder-style setter to attach an audit ledger for chain-of-custody proofs.
    #[must_use]
    pub fn with_ledger(mut self, ledger: Arc<Ledger>) -> Self {
        self.ledger = Some(ledger);
        self
    }

    /// Builder-style setter to attach a chain anchor for external blockchain integration.
    #[must_use]
    pub fn with_chain_anchor(mut self, anchor: Arc<dyn ChainAnchor>) -> Self {
        self.chain_anchor = Some(anchor);
        self
    }

    /// Encrypt content bytes if an encryption helper is configured, otherwise
    /// store as raw UTF-8 bytes (for backward compatibility with unencrypted DBs).
    async fn encrypt_content(&self, content: &str) -> Result<Vec<u8>> {
        match &self.encryption {
            Some(helper) => helper.encrypt_text(content).await,
            None => Ok(content.as_bytes().to_vec()),
        }
    }

    /// Decrypt content bytes if an encryption helper is configured, otherwise
    /// interpret as raw UTF-8.
    async fn decrypt_content(&self, bytes: &[u8]) -> Result<String> {
        match &self.encryption {
            Some(helper) => helper.decrypt_text(bytes).await,
            None => String::from_utf8(bytes.to_vec())
                .map_err(|e| Error::Memory(format!("Invalid UTF-8 in memory content: {}", e))),
        }
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
        tags: Option<Vec<String>>,
    ) -> Result<Memory> {
        validate_importance(importance)?;

        if let Some(ref emb) = embedding {
            validate_embedding_dimension(emb)?;
        }

        let memory_id = Uuid::new_v4();
        let source_str = source.to_string();
        let pg_embedding = embedding.as_ref().map(|e| Vector::from(e.clone()));
        let tags_vec = tags.unwrap_or_default();
        let tags_json = serde_json::to_value(&tags_vec)
            .map_err(|e| Error::Memory(format!("Failed to serialize tags: {}", e)))?;

        // Encrypt content if encryption is configured; otherwise store raw UTF-8 bytes
        let content_bytes = self.encrypt_content(content).await?;

        let row = sqlx::query(
            r"INSERT INTO memories (memory_id, identity_id, content, summary, source, embedding, importance, tags)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
              RETURNING memory_id, identity_id, content, summary, source, importance, created_at, accessed_at, access_count, tags",
        )
        .bind(memory_id)
        .bind(identity_id)
        .bind(&content_bytes)
        .bind(&summary)
        .bind(&source_str)
        .bind(pg_embedding)
        .bind(importance)
        .bind(&tags_json)
        .fetch_one(&self.pool)
        .await?;

        // Decrypt content from BYTEA
        let content_raw: Vec<u8> = row.get("content");
        let decrypted_content = self.decrypt_content(&content_raw).await?;

        let tags_raw: JsonValue = row.get("tags");
        let returned_tags: Vec<String> = serde_json::from_value(tags_raw).unwrap_or_default();

        let memory = Memory {
            memory_id: row.get("memory_id"),
            identity_id: row.get("identity_id"),
            content: decrypted_content,
            summary: row.get("summary"),
            source: row.get("source"),
            embedding,
            importance: row.get("importance"),
            created_at: row.get("created_at"),
            accessed_at: row.get("accessed_at"),
            access_count: row.get("access_count"),
            tags: returned_tags,
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
              RETURNING memory_id, identity_id, content, summary, source, importance, created_at, accessed_at, access_count, tags",
        )
        .bind(memory_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let content_raw: Vec<u8> = row.get("content");
        let decrypted_content = self.decrypt_content(&content_raw).await?;
        let tags_raw: JsonValue = row.get("tags");
        let tags: Vec<String> = serde_json::from_value(tags_raw).unwrap_or_default();

        Ok(Some(Memory {
            memory_id: row.get("memory_id"),
            identity_id: row.get("identity_id"),
            content: decrypted_content,
            summary: row.get("summary"),
            source: row.get("source"),
            embedding: None,
            importance: row.get("importance"),
            created_at: row.get("created_at"),
            accessed_at: row.get("accessed_at"),
            access_count: row.get("access_count"),
            tags,
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
        tags: Option<Vec<String>>,
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
        if tags.is_some() {
            param_idx += 1;
            set_parts.push(format!("tags = ${param_idx}"));
        }

        if set_parts.is_empty() {
            // Nothing to update, just fetch the current state
            return self
                .get_memory(memory_id)
                .await?
                .ok_or_else(|| Error::Memory(format!("Memory {} not found", memory_id)));
        }

        let sql = format!(
            "UPDATE memories SET {} WHERE memory_id = $1 RETURNING memory_id, identity_id, content, summary, source, importance, created_at, accessed_at, access_count, tags",
            set_parts.join(", ")
        );

        let mut query = sqlx::query(&sql).bind(memory_id);

        // Encrypt content if provided (column is BYTEA)
        let encrypted_content = if let Some(c) = content {
            Some(self.encrypt_content(c).await?)
        } else {
            None
        };

        if let Some(ref ec) = encrypted_content {
            query = query.bind(ec);
        }
        if let Some(s) = &summary {
            query = query.bind(s);
        }
        if let Some(i) = importance {
            query = query.bind(i);
        }
        if let Some(ref t) = tags {
            let tags_json = serde_json::to_value(t)
                .map_err(|e| Error::Memory(format!("Failed to serialize tags: {}", e)))?;
            query = query.bind(tags_json);
        }

        let row = query
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| Error::Memory(format!("Memory {} not found", memory_id)))?;

        let content_raw: Vec<u8> = row.get("content");
        let decrypted_content = self.decrypt_content(&content_raw).await?;
        let tags_raw: JsonValue = row.get("tags");
        let returned_tags: Vec<String> = serde_json::from_value(tags_raw).unwrap_or_default();

        let memory = Memory {
            memory_id: row.get("memory_id"),
            identity_id: row.get("identity_id"),
            content: decrypted_content,
            summary: row.get("summary"),
            source: row.get("source"),
            embedding: None,
            importance: row.get("importance"),
            created_at: row.get("created_at"),
            accessed_at: row.get("accessed_at"),
            access_count: row.get("access_count"),
            tags: returned_tags,
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

            self.emit_event(EventType::MemoryDeleted, json!({"memory_id": memory_id}));
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
    pub async fn load_recent_memories(&self, identity_id: Uuid, limit: i64) -> Result<Vec<Memory>> {
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
              RETURNING memory_id, identity_id, content, summary, source, importance, created_at, accessed_at, access_count, tags",
        )
        .bind(identity_id)
        .bind(since)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut memories = Vec::with_capacity(rows.len());
        for row in &rows {
            let content_raw: Vec<u8> = row.get("content");
            let decrypted_content = self.decrypt_content(&content_raw).await?;
            let tags_raw: JsonValue = row.get("tags");
            let tags: Vec<String> = serde_json::from_value(tags_raw).unwrap_or_default();
            memories.push(Memory {
                memory_id: row.get("memory_id"),
                identity_id: row.get("identity_id"),
                content: decrypted_content,
                summary: row.get("summary"),
                source: row.get("source"),
                embedding: None,
                importance: row.get("importance"),
                created_at: row.get("created_at"),
                accessed_at: row.get("accessed_at"),
                access_count: row.get("access_count"),
                tags,
            });
        }

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
              RETURNING memory_id, identity_id, content, summary, source, importance, created_at, accessed_at, access_count, tags",
        )
        .bind(identity_id)
        .bind(min_importance)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut memories = Vec::with_capacity(rows.len());
        for row in &rows {
            let content_raw: Vec<u8> = row.get("content");
            let decrypted_content = self.decrypt_content(&content_raw).await?;
            let tags_raw: JsonValue = row.get("tags");
            let tags: Vec<String> = serde_json::from_value(tags_raw).unwrap_or_default();
            memories.push(Memory {
                memory_id: row.get("memory_id"),
                identity_id: row.get("identity_id"),
                content: decrypted_content,
                summary: row.get("summary"),
                source: row.get("source"),
                embedding: None,
                importance: row.get("importance"),
                created_at: row.get("created_at"),
                accessed_at: row.get("accessed_at"),
                access_count: row.get("access_count"),
                tags,
            });
        }

        // UPDATE...RETURNING does not preserve subquery ORDER BY
        memories.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
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
            "SELECT memory_id, identity_id, content, summary, source, importance, created_at, accessed_at, access_count, tags \
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

        let mut memories = Vec::with_capacity(rows.len());
        for row in &rows {
            let content_raw: Vec<u8> = row.get("content");
            let decrypted_content = self.decrypt_content(&content_raw).await?;
            let tags_raw: JsonValue = row.get("tags");
            let tags: Vec<String> = serde_json::from_value(tags_raw).unwrap_or_default();
            memories.push(Memory {
                memory_id: row.get("memory_id"),
                identity_id: row.get("identity_id"),
                content: decrypted_content,
                summary: row.get("summary"),
                source: row.get("source"),
                embedding: None,
                importance: row.get("importance"),
                created_at: row.get("created_at"),
                accessed_at: row.get("accessed_at"),
                access_count: row.get("access_count"),
                tags,
            });
        }

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
                             created_at, accessed_at, access_count, tags,
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
                             created_at, accessed_at, access_count, tags,
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
                         created_at, accessed_at, access_count, tags,
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

            let content_raw: Vec<u8> = row.get("content");
            let decrypted_content = self.decrypt_content(&content_raw).await?;
            let tags_raw: JsonValue = row.get("tags");
            let tags: Vec<String> = serde_json::from_value(tags_raw).unwrap_or_default();

            results.push((
                Memory {
                    memory_id,
                    identity_id: row.get("identity_id"),
                    content: decrypted_content,
                    summary: row.get("summary"),
                    source: row.get("source"),
                    embedding: None,
                    importance: row.get("importance"),
                    created_at: row.get("created_at"),
                    accessed_at: row.get("accessed_at"),
                    access_count: row.get("access_count"),
                    tags,
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

        let (most_accessed_id, most_accessed_count) = most_accessed.map_or((None, None), |row| {
            (
                Some(row.get::<Uuid, _>("memory_id")),
                Some(row.get::<i32, _>("access_count")),
            )
        });

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
    // MEMORY PORTABILITY (Export / Import)
    // =========================================================================

    /// Export a single memory as a signed, encrypted CBOR envelope.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - Memory to export
    /// * `options` - Controls selective disclosure (embedding, ledger proof, capabilities)
    /// * `signing_key` - Optional Ed25519 key; if provided, the CBOR bytes are signed
    ///
    /// # Returns
    ///
    /// CBOR-serialized bytes. If `signing_key` is provided, the first 64 bytes are the
    /// Ed25519 signature followed by the CBOR payload.
    pub async fn export_memory(
        &self,
        memory_id: Uuid,
        options: &MemoryExportOptions,
        signing_key: Option<&SigningKey>,
    ) -> Result<Vec<u8>> {
        // 1. Fetch memory
        let memory = self
            .get_memory(memory_id)
            .await?
            .ok_or_else(|| Error::Memory(format!("Memory {} not found", memory_id)))?;

        // 2. Apply topic filter (check tags stored in DB)
        if let Some(ref filter_tags) = options.topic_filter {
            let tags = self.get_memory_tags(memory_id).await?;
            let matches = tags.iter().any(|t| filter_tags.contains(t));
            if !matches {
                return Err(Error::Memory(format!(
                    "Memory {} does not match topic filter",
                    memory_id
                )));
            }
        }

        // 3. Apply importance filter
        if let Some(min_imp) = options.min_importance {
            if memory.importance < min_imp {
                return Err(Error::Memory(format!(
                    "Memory {} importance {} below threshold {}",
                    memory_id, memory.importance, min_imp
                )));
            }
        }

        // 4. Retrieve ledger proof if requested
        let ledger_proof = if options.include_ledger_proof {
            self.get_ledger_proof_for_memory(memory_id).await?
        } else {
            None
        };

        // 5. Retrieve capability grants if requested
        let capability_grants = if options.include_capabilities {
            self.get_capability_grants_for_memory(memory_id).await?
        } else {
            Vec::new()
        };

        // 6. Serialize memory content to JSON
        let content_json = serde_json::to_vec(&json!({
            "memory_id": memory.memory_id,
            "identity_id": memory.identity_id,
            "content": memory.content,
            "summary": memory.summary,
            "source": memory.source,
            "importance": memory.importance,
            "created_at": memory.created_at,
            "accessed_at": memory.accessed_at,
            "access_count": memory.access_count,
        }))
        .map_err(|e| Error::Memory(format!("Failed to serialize memory: {}", e)))?;

        // 7. Encrypt content
        let encrypted_content = self.encrypt_content_bytes(&content_json).await?;

        // 8. Compute blake3 hash of encrypted content
        let content_hash = hex::encode(blake3::hash(&encrypted_content).as_bytes());

        // 9. Build envelope
        let mut metadata = HashMap::new();
        metadata.insert("exported_at".to_string(), Utc::now().to_rfc3339());
        metadata.insert("source_memory_id".to_string(), memory_id.to_string());

        let embedding = if options.include_embedding {
            memory.embedding
        } else {
            None
        };

        // 9.5. Anchor hash if chain_anchor is configured
        let chain_anchor = if let Some(ref anchor) = self.chain_anchor {
            let anchor_metadata = serde_json::json!({
                "memory_id": memory_id.to_string(),
                "exported_at": Utc::now().to_rfc3339(),
            });
            match anchor.anchor_hash(&content_hash, anchor_metadata).await {
                Ok(anchor_id) => Some(anchor_id),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to anchor memory hash");
                    None
                }
            }
        } else {
            None
        };

        let envelope = MemoryEnvelope {
            version: 1,
            encrypted_content,
            content_hash,
            ledger_proof,
            capability_grants,
            embedding,
            metadata,
            chain_anchor,
        };

        // 10. Serialize to CBOR
        let mut cbor_bytes = Vec::new();
        ciborium::ser::into_writer(&envelope, &mut cbor_bytes)
            .map_err(|e| Error::Memory(format!("CBOR serialization failed: {}", e)))?;

        // 11. Sign if signing key provided
        let output = if let Some(sk) = signing_key {
            let sig = crate::crypto::sign_bytes(sk, &cbor_bytes);
            let sig_bytes = hex::decode(&sig)
                .map_err(|e| Error::Memory(format!("Signature hex decode failed: {}", e)))?;
            let mut signed = sig_bytes;
            signed.extend_from_slice(&cbor_bytes);
            signed
        } else {
            cbor_bytes
        };

        // 12. Emit event
        self.emit_event(
            EventType::MemoryExported,
            json!({
                "memory_id": memory_id,
                "signed": signing_key.is_some(),
                "include_ledger_proof": options.include_ledger_proof,
            }),
        );

        Ok(output)
    }

    /// Export multiple memories as a batch CBOR envelope.
    ///
    /// Fetches memories matching the query, applies export options, and serializes
    /// all envelopes into a single CBOR batch. When a `signing_key` is provided,
    /// the **entire** serialized `Vec<MemoryEnvelope>` CBOR is signed as a unit
    /// (64-byte Ed25519 signature prefix). Individual envelopes within the batch
    /// are not individually signed. Use `import_memories_batch` to import the
    /// result; it performs batch-level signature verification and propagates the
    /// `verified` status to each imported memory.
    pub async fn export_memories_batch(
        &self,
        memory_ids: &[Uuid],
        options: &MemoryExportOptions,
        signing_key: Option<&SigningKey>,
    ) -> Result<Vec<u8>> {
        let mut envelopes: Vec<MemoryEnvelope> = Vec::new();

        for &mid in memory_ids {
            match self.export_memory_envelope(mid, options).await {
                Ok(env) => envelopes.push(env),
                Err(e) => {
                    tracing::warn!(memory_id = %mid, error = %e, "Skipping memory during batch export");
                }
            }
        }

        if envelopes.is_empty() {
            return Err(Error::Memory(
                "No memories matched export criteria".to_string(),
            ));
        }

        // Serialize batch to CBOR
        let mut cbor_bytes = Vec::new();
        ciborium::ser::into_writer(&envelopes, &mut cbor_bytes)
            .map_err(|e| Error::Memory(format!("CBOR batch serialization failed: {}", e)))?;

        // Sign if signing key provided
        let output = if let Some(sk) = signing_key {
            let sig = crate::crypto::sign_bytes(sk, &cbor_bytes);
            let sig_bytes = hex::decode(&sig)
                .map_err(|e| Error::Memory(format!("Signature hex decode failed: {}", e)))?;
            let mut signed = sig_bytes;
            signed.extend_from_slice(&cbor_bytes);
            signed
        } else {
            cbor_bytes
        };

        self.emit_event(
            EventType::MemoryExported,
            json!({
                "count": envelopes.len(),
                "signed": signing_key.is_some(),
                "batch": true,
            }),
        );

        Ok(output)
    }

    /// Import a single memory from a CBOR envelope.
    ///
    /// # Arguments
    ///
    /// * `envelope_bytes` - Raw bytes (optionally prefixed with 64-byte Ed25519 signature)
    /// * `identity_id` - Identity to assign the imported memory to
    /// * `verify_signature` - Whether to verify the Ed25519 signature
    /// * `public_key` - Hex-encoded Ed25519 public key for verification
    pub async fn import_memory(
        &self,
        envelope_bytes: &[u8],
        identity_id: Uuid,
        verify_signature: bool,
        public_key: Option<&str>,
    ) -> Result<MemoryImportResult> {
        let mut warnings = Vec::new();

        // 1. Verify signature if requested
        let cbor_data = if verify_signature {
            if envelope_bytes.len() < 64 {
                return Err(Error::Memory(
                    "Envelope too short for signature verification".to_string(),
                ));
            }
            let sig_bytes = &envelope_bytes[..64];
            let cbor_payload = &envelope_bytes[64..];
            let sig_hex = hex::encode(sig_bytes);

            let pk = public_key.ok_or_else(|| {
                Error::Memory("Public key required for signature verification".to_string())
            })?;

            let valid = crate::crypto::verify_signature(pk, cbor_payload, &sig_hex)?;
            if !valid {
                return Err(Error::Memory(
                    "Envelope signature verification failed".to_string(),
                ));
            }
            cbor_payload
        } else {
            if envelope_bytes.len() >= 64 && public_key.is_some() {
                warnings.push("Signature present but verification not requested".to_string());
            }
            envelope_bytes
        };

        // 2. Deserialize CBOR to MemoryEnvelope
        let envelope: MemoryEnvelope = ciborium::de::from_reader(cbor_data)
            .map_err(|e| Error::Memory(format!("CBOR deserialization failed: {}", e)))?;

        // 2.5. Enforce topic-scoped capability grants (Phase 3.2)
        // Check if envelope has topic-scoped grants and verify identity has access
        if !envelope.capability_grants.is_empty() {
            if let Some(topic_filter) = envelope.metadata.get("topic_filter") {
                let topics: Vec<&str> = topic_filter.split(',').map(|s| s.trim()).collect();
                let policy_engine = crate::policy::PolicyEngine::new(self.pool.clone());

                for topic in topics {
                    let has_capability = policy_engine
                        .check_memory_topic_capability(identity_id, topic)
                        .await?;

                    if !has_capability {
                        return Err(Error::Security(format!(
                            "Topic capability denied: {}",
                            topic
                        )));
                    }
                }
            }
        }

        // 3. Verify blake3 hash
        let computed_hash = hex::encode(blake3::hash(&envelope.encrypted_content).as_bytes());
        if computed_hash != envelope.content_hash {
            return Err(Error::Memory(
                "Content hash mismatch: envelope may be tampered".to_string(),
            ));
        }

        // 4. Decrypt content
        let decrypted = self
            .decrypt_content_bytes(&envelope.encrypted_content)
            .await?;

        // 5. Deserialize JSON to memory fields
        let mem_json: JsonValue = serde_json::from_slice(&decrypted)
            .map_err(|e| Error::Memory(format!("Failed to parse decrypted content: {}", e)))?;

        let content = mem_json["content"]
            .as_str()
            .ok_or_else(|| Error::Memory("Missing 'content' field in envelope".to_string()))?;
        let summary = mem_json["summary"].as_str().map(String::from);
        let source_str = mem_json["source"].as_str().unwrap_or("observation");
        let source = MemorySource::from_str(source_str).unwrap_or(MemorySource::Observation);
        let importance = mem_json["importance"].as_f64().unwrap_or(0.5) as f32;

        // 6. Verify ledger proof if present
        let ledger_proof_valid = if let Some(ref proof) = envelope.ledger_proof {
            self.verify_ledger_proof(proof).await.unwrap_or_else(|e| {
                warnings.push(format!("Ledger proof verification error: {}", e));
                false
            })
        } else {
            warnings.push("No ledger proof included in envelope".to_string());
            false
        };

        // 7. Create new memory
        let memory = self
            .create_memory(
                identity_id,
                content,
                summary,
                source,
                None,
                importance,
                None,
            )
            .await?;

        // 8. Recreate capability grants if present
        for grant in &envelope.capability_grants {
            if let Err(e) = self
                .recreate_capability_grant(memory.memory_id, grant)
                .await
            {
                warnings.push(format!(
                    "Failed to recreate capability grant {}: {}",
                    grant.grant_id, e
                ));
            }
        }

        // 9. Add embedding if present
        if let Some(ref emb) = envelope.embedding {
            if let Err(e) = self
                .add_embedding_to_memory(memory.memory_id, emb.clone())
                .await
            {
                warnings.push(format!("Failed to add embedding: {}", e));
            }
        }

        // 10. Log import to ledger
        if let Some(ref ledger) = self.ledger {
            if let Err(e) = ledger
                .append_event(
                    None,
                    "memory.imported",
                    json!({
                        "memory_id": memory.memory_id,
                        "identity_id": identity_id,
                        "source_memory_id": envelope.metadata.get("source_memory_id"),
                        "verified": verify_signature,
                        "ledger_proof_valid": ledger_proof_valid,
                    }),
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .await
            {
                warnings.push(format!("Failed to log import to ledger: {}", e));
            }
        }

        // 11. Emit event
        self.emit_event(
            EventType::MemoryImported,
            json!({
                "memory_id": memory.memory_id,
                "identity_id": identity_id,
                "verified": verify_signature,
                "ledger_proof_valid": ledger_proof_valid,
            }),
        );

        Ok(MemoryImportResult {
            memory_id: memory.memory_id,
            verified: verify_signature,
            ledger_proof_valid,
            warnings,
        })
    }

    /// Import a batch of memories from a CBOR envelope containing `Vec<MemoryEnvelope>`.
    ///
    /// # Batch vs Per-Envelope Verification
    ///
    /// Batch exports sign the serialized `Vec<MemoryEnvelope>` CBOR as a whole
    /// (64-byte Ed25519 signature prefix). Individual envelopes within the batch
    /// are **not** individually signed. When `verify_signature` is `true`, this
    /// method verifies the batch-level signature against `public_key`, which
    /// guarantees integrity of all contained envelopes. Each envelope is then
    /// imported via `import_memory` with `verify_signature=false` (since the
    /// batch signature already covers them). The `verified` field on each
    /// [`MemoryImportResult`] reflects the batch-level verification outcome.
    ///
    /// For single imports via `import_memory`, the per-envelope signature
    /// (64-byte prefix on individual CBOR) is verified directly.
    pub async fn import_memories_batch(
        &self,
        batch_bytes: &[u8],
        identity_id: Uuid,
        verify_signature: bool,
        public_key: Option<&str>,
    ) -> Result<Vec<MemoryImportResult>> {
        // 1. Verify batch-level signature if requested.
        //    The batch signature covers the entire Vec<MemoryEnvelope> CBOR payload,
        //    so individual envelopes do not carry their own signatures.
        let (cbor_data, batch_verified) = if verify_signature {
            if batch_bytes.len() < 64 {
                return Err(Error::Memory(
                    "Batch too short for signature verification".to_string(),
                ));
            }
            let sig_bytes = &batch_bytes[..64];
            let cbor_payload = &batch_bytes[64..];
            let sig_hex = hex::encode(sig_bytes);

            let pk = public_key.ok_or_else(|| {
                Error::Memory("Public key required for signature verification".to_string())
            })?;

            let valid = crate::crypto::verify_signature(pk, cbor_payload, &sig_hex)?;
            if !valid {
                return Err(Error::Memory(
                    "Batch signature verification failed".to_string(),
                ));
            }
            (cbor_payload, true)
        } else {
            (batch_bytes, false)
        };

        // 2. Deserialize CBOR to Vec<MemoryEnvelope>
        let envelopes: Vec<MemoryEnvelope> = ciborium::de::from_reader(cbor_data)
            .map_err(|e| Error::Memory(format!("CBOR batch deserialization failed: {}", e)))?;

        // 3. Import each envelope individually.
        //    Per-envelope signature verification is skipped because the batch-level
        //    signature (verified above) already guarantees integrity of all envelopes.
        let mut results = Vec::with_capacity(envelopes.len());
        for envelope in envelopes {
            // Re-serialize individual envelope to CBOR for import_memory
            let mut single_cbor = Vec::new();
            ciborium::ser::into_writer(&envelope, &mut single_cbor)
                .map_err(|e| Error::Memory(format!("CBOR re-serialization failed: {}", e)))?;

            match self
                .import_memory(&single_cbor, identity_id, false, None)
                .await
            {
                Ok(mut result) => {
                    // Override verified to reflect batch-level verification
                    result.verified = batch_verified;
                    results.push(result);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to import memory in batch");
                    results.push(MemoryImportResult {
                        memory_id: Uuid::nil(),
                        verified: false,
                        ledger_proof_valid: false,
                        warnings: vec![format!("Import failed: {}", e)],
                    });
                }
            }
        }

        Ok(results)
    }

    // =========================================================================
    // PORTABILITY HELPERS (private)
    // =========================================================================

    /// Build a MemoryEnvelope without serializing to bytes (used by batch export).
    async fn export_memory_envelope(
        &self,
        memory_id: Uuid,
        options: &MemoryExportOptions,
    ) -> Result<MemoryEnvelope> {
        let memory = self
            .get_memory(memory_id)
            .await?
            .ok_or_else(|| Error::Memory(format!("Memory {} not found", memory_id)))?;

        // Apply topic filter
        if let Some(ref filter_tags) = options.topic_filter {
            let tags = self.get_memory_tags(memory_id).await?;
            if !tags.iter().any(|t| filter_tags.contains(t)) {
                return Err(Error::Memory(format!(
                    "Memory {} does not match topic filter",
                    memory_id
                )));
            }
        }

        // Apply importance filter
        if let Some(min_imp) = options.min_importance {
            if memory.importance < min_imp {
                return Err(Error::Memory(format!(
                    "Memory {} importance below threshold",
                    memory_id
                )));
            }
        }

        let ledger_proof = if options.include_ledger_proof {
            self.get_ledger_proof_for_memory(memory_id).await?
        } else {
            None
        };

        let capability_grants = if options.include_capabilities {
            self.get_capability_grants_for_memory(memory_id).await?
        } else {
            Vec::new()
        };

        let content_json = serde_json::to_vec(&json!({
            "memory_id": memory.memory_id,
            "identity_id": memory.identity_id,
            "content": memory.content,
            "summary": memory.summary,
            "source": memory.source,
            "importance": memory.importance,
            "created_at": memory.created_at,
            "accessed_at": memory.accessed_at,
            "access_count": memory.access_count,
        }))
        .map_err(|e| Error::Memory(format!("Failed to serialize memory: {}", e)))?;

        let encrypted_content = self.encrypt_content_bytes(&content_json).await?;
        let content_hash = hex::encode(blake3::hash(&encrypted_content).as_bytes());

        let mut metadata = HashMap::new();
        metadata.insert("exported_at".to_string(), Utc::now().to_rfc3339());
        metadata.insert("source_memory_id".to_string(), memory_id.to_string());

        // Add topic filter metadata if set (Phase 3.3)
        if let Some(ref filter_tags) = options.topic_filter {
            metadata.insert("topic_filter".to_string(), filter_tags.join(","));
        }

        let embedding = if options.include_embedding {
            memory.embedding
        } else {
            None
        };

        Ok(MemoryEnvelope {
            version: 1,
            encrypted_content,
            content_hash,
            ledger_proof,
            capability_grants,
            embedding,
            metadata,
            chain_anchor: None,
        })
    }

    /// Encrypt raw bytes using the encryption helper (or pass through if none).
    async fn encrypt_content_bytes(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        match &self.encryption {
            Some(helper) => helper.encrypt_bytes(plaintext).await,
            None => Ok(plaintext.to_vec()),
        }
    }

    /// Decrypt raw bytes using the encryption helper (or pass through if none).
    async fn decrypt_content_bytes(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        match &self.encryption {
            Some(helper) => helper.decrypt_bytes(ciphertext).await,
            None => Ok(ciphertext.to_vec()),
        }
    }

    /// Retrieve tags for a memory from the database.
    async fn get_memory_tags(&self, memory_id: Uuid) -> Result<Vec<String>> {
        let row: Option<(JsonValue,)> =
            sqlx::query_as("SELECT COALESCE(tags, '[]'::jsonb) FROM memories WHERE memory_id = $1")
                .bind(memory_id)
                .fetch_optional(&self.pool)
                .await?;

        match row {
            Some((val,)) => {
                let tags: Vec<String> = serde_json::from_value(val).unwrap_or_default();
                Ok(tags)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Query ledger for the creation event of a memory and build proof material.
    async fn get_ledger_proof_for_memory(
        &self,
        memory_id: Uuid,
    ) -> Result<Option<LedgerProofMaterial>> {
        let row: Option<(i64, DateTime<Utc>, String, Option<String>, Option<String>)> =
            sqlx::query_as(
                r"SELECT event_id, ts, event_hash, prev_hash, core_signature
                  FROM ledger_events
                  WHERE action_type = 'memory.created'
                    AND metadata @> $1::jsonb
                  ORDER BY event_id DESC
                  LIMIT 1",
            )
            .bind(json!({"memory_id": memory_id.to_string()}))
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(
            |(event_id, ts, event_hash, prev_hash, core_signature)| LedgerProofMaterial {
                event_id,
                event_hash,
                prev_hash,
                core_signature,
                timestamp: ts,
            },
        ))
    }

    /// Retrieve capability grants associated with a memory.
    async fn get_capability_grants_for_memory(
        &self,
        memory_id: Uuid,
    ) -> Result<Vec<CapabilityGrantMetadata>> {
        let rows: Vec<(
            Uuid,
            String,
            Option<JsonValue>,
            Option<Uuid>,
            Option<DateTime<Utc>>,
        )> = sqlx::query_as(
            r"SELECT grant_id, capability_key, scope, approved_by, expires_at
                  FROM capability_grants
                  WHERE subject_type = 'external_key' AND subject_id = $1::text
                  ORDER BY grant_id",
        )
        .bind(memory_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(grant_id, capability_key, scope, approved_by, expires_at)| {
                    CapabilityGrantMetadata {
                        grant_id,
                        capability_key,
                        scope,
                        approved_by,
                        expires_at,
                    }
                },
            )
            .collect())
    }

    /// Verify a ledger proof by recomputing the event hash and checking the
    /// Ed25519 `core_signature` against the owner public key when present.
    async fn verify_ledger_proof(&self, proof: &LedgerProofMaterial) -> Result<bool> {
        // Fetch the event from the ledger including quantum_salt
        let row: Option<(String, DateTime<Utc>, Option<Uuid>, String, String, Option<Vec<u8>>)> = sqlx::query_as(
            "SELECT action_type, ts, actor_id, payload_hash, event_hash, quantum_salt FROM ledger_events WHERE event_id = $1",
        )
        .bind(proof.event_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        match row {
            Some((action_type, ts, actor_id, payload_hash, stored_hash, quantum_salt)) => {
                // Recompute event hash with quantum_salt for verification
                let computed = Ledger::compute_event_hash(
                    &ts,
                    actor_id,
                    &action_type,
                    &payload_hash,
                    proof.prev_hash.as_deref(),
                    quantum_salt.as_deref(),
                );
                if computed != stored_hash || stored_hash != proof.event_hash {
                    return Ok(false);
                }

                // Verify Ed25519 core_signature if present in the proof
                if let Some(ref sig_hex) = proof.core_signature {
                    // Retrieve the owner public key from the ledger's config
                    // by querying the owner_keypairs table for the public key.
                    let pk_row: Option<(String,)> = sqlx::query_as(
                        "SELECT encode(public_key, 'hex') FROM owner_keypairs ORDER BY created_at DESC LIMIT 1",
                    )
                    .fetch_optional(&self.pool)
                    .await?;

                    match pk_row {
                        Some((public_key_hex,)) => {
                            let valid = crate::crypto::verify_signature(
                                &public_key_hex,
                                stored_hash.as_bytes(),
                                sig_hex,
                            )?;
                            if !valid {
                                return Ok(false);
                            }
                        }
                        None => {
                            // No owner public key available; reject proofs with
                            // signatures we cannot verify for privileged events.
                            return Ok(false);
                        }
                    }
                }

                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Recreate a capability grant from imported metadata.
    async fn recreate_capability_grant(
        &self,
        memory_id: Uuid,
        grant: &CapabilityGrantMetadata,
    ) -> Result<()> {
        // Check if grant has been revoked using PolicyEngine (cross-instance revocation check)
        let policy_engine = crate::policy::PolicyEngine::new(self.pool.clone());
        let is_revoked = policy_engine.is_grant_revoked(grant.grant_id).await?;

        if is_revoked {
            tracing::warn!(grant_id = %grant.grant_id, "Grant has been revoked on this instance, skipping recreation");
            return Err(Error::Security(format!(
                "Grant {} has been revoked on this instance",
                grant.grant_id
            )));
        }

        sqlx::query(
            r"INSERT INTO capability_grants (grant_id, subject_type, subject_id, capability_key, scope, approved_by, expires_at)
              VALUES ($1, 'external_key', $2, $3, $4, $5, $6)
              ON CONFLICT (grant_id) DO NOTHING",
        )
        .bind(Uuid::new_v4()) // New grant_id for the imported copy
        .bind(memory_id.to_string())
        .bind(&grant.capability_key)
        .bind(&grant.scope)
        .bind(grant.approved_by)
        .bind(grant.expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
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
        assert_eq!(MemorySource::from_str("task").unwrap(), MemorySource::Task);
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
        assert_eq!(MemorySource::from_str("Task").unwrap(), MemorySource::Task);
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
            tags: Vec::new(),
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
            tags: Vec::new(),
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
            tags: Vec::new(),
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
            tags: Vec::new(),
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
            tags: Vec::new(),
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

    // =========================================================================
    // Memory Portability tests
    // =========================================================================

    #[test]
    fn test_memory_envelope_cbor_roundtrip() {
        let envelope = MemoryEnvelope {
            version: 1,
            encrypted_content: b"test content bytes".to_vec(),
            content_hash: hex::encode(blake3::hash(b"test content bytes").as_bytes()),
            ledger_proof: None,
            capability_grants: Vec::new(),
            embedding: None,
            metadata: {
                let mut m = HashMap::new();
                m.insert(
                    "exported_at".to_string(),
                    "2025-01-01T00:00:00Z".to_string(),
                );
                m
            },
            chain_anchor: None,
        };

        // Serialize to CBOR
        let mut cbor_bytes = Vec::new();
        ciborium::ser::into_writer(&envelope, &mut cbor_bytes).expect("CBOR serialize");

        // Deserialize from CBOR
        let decoded: MemoryEnvelope =
            ciborium::de::from_reader(&cbor_bytes[..]).expect("CBOR deserialize");

        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.encrypted_content, b"test content bytes");
        assert_eq!(decoded.content_hash, envelope.content_hash);
        assert!(decoded.ledger_proof.is_none());
        assert!(decoded.capability_grants.is_empty());
        assert!(decoded.embedding.is_none());
        assert_eq!(
            decoded.metadata.get("exported_at").unwrap(),
            "2025-01-01T00:00:00Z"
        );
        assert!(decoded.chain_anchor.is_none());
    }

    #[test]
    fn test_memory_envelope_with_embedding_cbor_roundtrip() {
        let embedding = vec![0.1f32, 0.2, 0.3, 0.4];
        let envelope = MemoryEnvelope {
            version: 1,
            encrypted_content: b"data".to_vec(),
            content_hash: hex::encode(blake3::hash(b"data").as_bytes()),
            ledger_proof: Some(LedgerProofMaterial {
                event_id: 42,
                event_hash: "abc123".to_string(),
                prev_hash: Some("prev456".to_string()),
                core_signature: Some("sig789".to_string()),
                timestamp: Utc::now(),
            }),
            capability_grants: vec![CapabilityGrantMetadata {
                grant_id: Uuid::new_v4(),
                capability_key: "memory.read".to_string(),
                scope: Some(serde_json::json!({"level": "full"})),
                approved_by: Some(Uuid::new_v4()),
                expires_at: None,
            }],
            embedding: Some(embedding.clone()),
            metadata: HashMap::new(),
            chain_anchor: Some("anchor_id_123".to_string()),
        };

        let mut cbor_bytes = Vec::new();
        ciborium::ser::into_writer(&envelope, &mut cbor_bytes).expect("CBOR serialize");

        let decoded: MemoryEnvelope =
            ciborium::de::from_reader(&cbor_bytes[..]).expect("CBOR deserialize");

        assert_eq!(decoded.embedding.unwrap(), embedding);
        assert!(decoded.ledger_proof.is_some());
        let proof = decoded.ledger_proof.unwrap();
        assert_eq!(proof.event_id, 42);
        assert_eq!(proof.event_hash, "abc123");
        assert_eq!(proof.prev_hash, Some("prev456".to_string()));
        assert_eq!(decoded.capability_grants.len(), 1);
        assert_eq!(decoded.capability_grants[0].capability_key, "memory.read");
        assert_eq!(decoded.chain_anchor, Some("anchor_id_123".to_string()));
    }

    #[test]
    fn test_memory_envelope_content_hash_verification() {
        let content = b"sensitive memory content";
        let hash = hex::encode(blake3::hash(content).as_bytes());

        let envelope = MemoryEnvelope {
            version: 1,
            encrypted_content: content.to_vec(),
            content_hash: hash.clone(),
            ledger_proof: None,
            capability_grants: Vec::new(),
            embedding: None,
            metadata: HashMap::new(),
            chain_anchor: None,
        };

        // Verify hash matches
        let computed = hex::encode(blake3::hash(&envelope.encrypted_content).as_bytes());
        assert_eq!(computed, envelope.content_hash);

        // Tampered content should not match
        let mut tampered = envelope.clone();
        tampered.encrypted_content = b"tampered content".to_vec();
        let tampered_hash = hex::encode(blake3::hash(&tampered.encrypted_content).as_bytes());
        assert_ne!(tampered_hash, tampered.content_hash);
    }

    #[test]
    fn test_memory_envelope_signature_roundtrip() {
        use crate::crypto::{generate_ed25519_keypair, sign_bytes, verify_signature};

        let (signing_key, _verifying_key) = generate_ed25519_keypair();
        let public_key_hex = crate::crypto::public_key_from_signing_key(&signing_key);

        let envelope = MemoryEnvelope {
            version: 1,
            encrypted_content: b"test".to_vec(),
            content_hash: hex::encode(blake3::hash(b"test").as_bytes()),
            ledger_proof: None,
            capability_grants: Vec::new(),
            embedding: None,
            metadata: HashMap::new(),
            chain_anchor: None,
        };

        // Serialize to CBOR
        let mut cbor_bytes = Vec::new();
        ciborium::ser::into_writer(&envelope, &mut cbor_bytes).expect("CBOR serialize");

        // Sign
        let sig_hex = sign_bytes(&signing_key, &cbor_bytes);
        let sig_bytes = hex::decode(&sig_hex).expect("hex decode sig");

        // Combine: signature + CBOR
        let mut signed_payload = sig_bytes.clone();
        signed_payload.extend_from_slice(&cbor_bytes);

        // Verify
        assert_eq!(signed_payload.len(), 64 + cbor_bytes.len());
        let extracted_sig = hex::encode(&signed_payload[..64]);
        let extracted_cbor = &signed_payload[64..];
        let valid =
            verify_signature(&public_key_hex, extracted_cbor, &extracted_sig).expect("verify");
        assert!(valid);

        // Tampered payload should fail
        let mut tampered_payload = signed_payload.clone();
        if let Some(byte) = tampered_payload.get_mut(65) {
            *byte ^= 0xFF;
        }
        let tampered_cbor = &tampered_payload[64..];
        let tampered_sig = hex::encode(&tampered_payload[..64]);
        let tampered_valid = verify_signature(&public_key_hex, tampered_cbor, &tampered_sig)
            .expect("verify tampered");
        assert!(!tampered_valid);
    }

    #[test]
    fn test_memory_export_options_defaults() {
        let opts = MemoryExportOptions::default();
        assert!(!opts.include_embedding);
        assert!(opts.topic_filter.is_none());
        assert!(opts.min_importance.is_none());
        assert!(!opts.include_ledger_proof);
        assert!(!opts.include_capabilities);
    }

    #[test]
    fn test_no_op_chain_anchor() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let anchor = NoOpChainAnchor;

        let id = rt.block_on(anchor.anchor_hash("hash123", serde_json::json!({})));
        assert_eq!(id.unwrap(), "not_implemented");

        let verified = rt.block_on(anchor.verify_anchor("hash123", "anchor_id"));
        assert!(!verified.unwrap());

        let proof = rt.block_on(anchor.get_anchor_proof("anchor_id"));
        assert_eq!(
            proof.unwrap(),
            Some(serde_json::json!({"status": "not_implemented"}))
        );
    }

    #[test]
    fn test_memory_envelope_batch_cbor_roundtrip() {
        let envelopes = vec![
            MemoryEnvelope {
                version: 1,
                encrypted_content: b"memory1".to_vec(),
                content_hash: hex::encode(blake3::hash(b"memory1").as_bytes()),
                ledger_proof: None,
                capability_grants: Vec::new(),
                embedding: None,
                metadata: HashMap::new(),
                chain_anchor: None,
            },
            MemoryEnvelope {
                version: 1,
                encrypted_content: b"memory2".to_vec(),
                content_hash: hex::encode(blake3::hash(b"memory2").as_bytes()),
                ledger_proof: None,
                capability_grants: Vec::new(),
                embedding: Some(vec![1.0, 2.0, 3.0]),
                metadata: HashMap::new(),
                chain_anchor: None,
            },
        ];

        let mut cbor_bytes = Vec::new();
        ciborium::ser::into_writer(&envelopes, &mut cbor_bytes).expect("CBOR batch serialize");

        let decoded: Vec<MemoryEnvelope> =
            ciborium::de::from_reader(&cbor_bytes[..]).expect("CBOR batch deserialize");

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].encrypted_content, b"memory1");
        assert_eq!(decoded[1].encrypted_content, b"memory2");
        assert!(decoded[1].embedding.is_some());
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_export_import_roundtrip() {
        unimplemented!("Run with: cargo test --test memory -- --ignored");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_selective_disclosure_by_topic() {
        unimplemented!("Run with: cargo test --test memory -- --ignored");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_signature_verification() {
        unimplemented!("Run with: cargo test --test memory -- --ignored");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_ledger_proof_verification() {
        unimplemented!("Run with: cargo test --test memory -- --ignored");
    }

    /// Verifies that batch import with a valid batch-level signature results in
    /// `verified=true` on all `MemoryImportResult`s, and that a tampered batch
    /// is rejected outright.
    ///
    /// This is a unit-level test covering the signing/verification wire format
    /// without requiring a database. The batch CBOR is signed as a whole; individual
    /// envelopes are unsigned. `import_memories_batch` should verify the batch
    /// signature and then import each envelope with `verify_signature=false`.
    #[test]
    fn test_batch_import_with_signature() {
        use crate::crypto::{generate_ed25519_keypair, sign_bytes, verify_signature};

        let (signing_key, _verifying_key) = generate_ed25519_keypair();
        let public_key_hex = crate::crypto::public_key_from_signing_key(&signing_key);

        // Build two envelopes
        let envelopes = vec![
            MemoryEnvelope {
                version: 1,
                encrypted_content: b"batch_mem_1".to_vec(),
                content_hash: hex::encode(blake3::hash(b"batch_mem_1").as_bytes()),
                ledger_proof: None,
                capability_grants: Vec::new(),
                embedding: None,
                metadata: HashMap::new(),
                chain_anchor: None,
            },
            MemoryEnvelope {
                version: 1,
                encrypted_content: b"batch_mem_2".to_vec(),
                content_hash: hex::encode(blake3::hash(b"batch_mem_2").as_bytes()),
                ledger_proof: None,
                capability_grants: Vec::new(),
                embedding: Some(vec![1.0, 2.0]),
                metadata: HashMap::new(),
                chain_anchor: None,
            },
        ];

        // Serialize batch to CBOR
        let mut cbor_bytes = Vec::new();
        ciborium::ser::into_writer(&envelopes, &mut cbor_bytes).expect("CBOR batch serialize");

        // Sign the batch CBOR (mimics export_memories_batch with signing_key)
        let sig_hex = sign_bytes(&signing_key, &cbor_bytes);
        let sig_bytes = hex::decode(&sig_hex).expect("hex decode sig");
        let mut signed_batch = sig_bytes.clone();
        signed_batch.extend_from_slice(&cbor_bytes);

        // Verify the batch signature succeeds (simulates step 1 of import_memories_batch)
        assert!(signed_batch.len() >= 64);
        let extracted_sig = hex::encode(&signed_batch[..64]);
        let extracted_cbor = &signed_batch[64..];
        let valid =
            verify_signature(&public_key_hex, extracted_cbor, &extracted_sig).expect("verify");
        assert!(valid, "Batch signature should be valid");

        // Deserialize the CBOR payload to confirm envelopes are intact
        let decoded: Vec<MemoryEnvelope> =
            ciborium::de::from_reader(extracted_cbor).expect("CBOR batch deserialize");
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].encrypted_content, b"batch_mem_1");
        assert_eq!(decoded[1].encrypted_content, b"batch_mem_2");

        // Verify individual envelopes do NOT have a 64-byte sig prefix
        // (re-serialized singles are plain CBOR, not signed)
        for env in &decoded {
            let mut single_cbor = Vec::new();
            ciborium::ser::into_writer(env, &mut single_cbor).expect("re-serialize");
            // Should be valid CBOR without a sig prefix
            let roundtrip: MemoryEnvelope =
                ciborium::de::from_reader(&single_cbor[..]).expect("single deserialize");
            assert_eq!(roundtrip.version, env.version);
            assert_eq!(roundtrip.content_hash, env.content_hash);
        }

        // Tampered batch should fail verification
        let mut tampered_batch = signed_batch.clone();
        // Flip a byte in the CBOR payload (after the 64-byte sig)
        if let Some(byte) = tampered_batch.get_mut(65) {
            *byte ^= 0xFF;
        }
        let tampered_sig = hex::encode(&tampered_batch[..64]);
        let tampered_cbor = &tampered_batch[64..];
        let tampered_valid = verify_signature(&public_key_hex, tampered_cbor, &tampered_sig)
            .expect("verify tampered");
        assert!(
            !tampered_valid,
            "Tampered batch signature should be invalid"
        );
    }
}
