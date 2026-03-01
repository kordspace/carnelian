//! Context Assembly Pipeline
//!
//! This module implements a deterministic, priority-based context assembly pipeline
//! for constructing model input from multiple data sources. Segments are loaded in
//! priority order (P0–P4), token-budgeted, and pruned when the assembled context
//! exceeds the model's context window.
//!
//! ## Priority-Based Segment Loading
//!
//! | Priority | Source | Always Included? | Prunable? | Trim Strategy |
//! |----------|--------|------------------|-----------|---------------|
//! | P0 | Soul Directives | Yes | No | Never pruned |
//! | P1 | Recent Memories (48hr + high importance) | Preferred | Yes (lowest importance first) | Drop by importance |
//! | P2 | Task Context (current task/run) | Yes | No | Never pruned |
//! | P3 | Conversation History | Preferred | Yes (oldest first) | Drop oldest chunks |
//! | P4 | Tool Results | Optional | Yes (oldest first) | Soft-trim then hard-clear |
//!
//! ## Token Budget Enforcement
//!
//! When the total token count exceeds the budget (set to 90% of the context window
//! to reserve headroom for the model response), segments are pruned in reverse
//! priority order:
//!
//! 1. Hard-clear old tool results (P4, age > threshold)
//! 2. Soft-trim oversized tool results (P4, head+tail strategy)
//! 3. Drop oldest conversation messages (P3)
//! 4. Drop lowest-importance memories (P1)
//! 5. P0 (soul directives) and P2 (task context) are never pruned
//!
//! ## Provenance Tracking
//!
//! Every assembled context bundle records which memory IDs, run IDs, and message
//! IDs contributed to the final output. A blake3 hash of the concatenated segments
//! provides a deterministic fingerprint for audit logging via the ledger.
//!
//! ## Soft-Trim Strategy
//!
//! Oversized tool results are trimmed using a head+tail approach: 60% of the
//! token budget is allocated to the head (beginning) and 40% to the tail (end),
//! with an ellipsis separator indicating the omitted middle section.
//!
//! ## Context Integrity Auditing
//!
//! Every model call should be preceded by a `"model.context.assembled"` ledger event
//! that records full provenance metadata. This creates an auditable chain:
//!
//! 1. **Context assembly**: `log_to_ledger()` or `log_context_integrity()` records
//!    the blake3 hash, memory IDs, run IDs, and message IDs that contributed to the
//!    context bundle.
//! 2. **Model call**: The model router logs `"model.call.request"` and
//!    `"model.call.response"` events with the same `correlation_id`.
//! 3. **Audit trail**: Query `ledger_events` by `correlation_id` to reconstruct
//!    exactly what context was sent to the model and what response was received.
//!
//! The `ContextProvenance` struct captures:
//! - **`memory_ids`**: Which memories were included (source attribution)
//! - **`run_ids`**: Which task runs contributed context (task lineage)
//! - **`message_ids`**: Which conversation messages were included (session history)
//! - **`context_bundle_hash`**: blake3 hash for tamper detection
//! - **`total_tokens`**: Estimated token count for budget verification
//! - **`segment_counts`**: Breakdown by source type for observability
//!
//! ## Example
//!
//! ```ignore
//! let mut ctx = ContextWindow::build_for_session(
//!     pool, Some(event_stream), session_id, Some(task_id), &config,
//! ).await?;
//!
//! let assembled = ctx.assemble(&config).await?;
//!
//! // Log context integrity before model call (links via correlation_id)
//! ctx.log_to_ledger(&ledger, correlation_id).await?;
//!
//! // Or use the convenience method to get provenance back:
//! let (event_id, provenance) = ctx.log_context_integrity(&ledger, correlation_id).await?;
//! ```

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Row};
use tiktoken_rs::CoreBPE;
use uuid::Uuid;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_common::{Error, Result};

use crate::config::Config;
use crate::events::EventStream;
use crate::ledger::Ledger;
use crate::memory::MemoryManager;
use crate::session::SessionMessage;
use crate::soul::SoulDirective;

// =============================================================================
// TOKEN ESTIMATION
// =============================================================================

/// Cached cl100k_base tokenizer (used by GPT-4, GPT-3.5-turbo, and many others).
/// Initialised once on first call and reused for the lifetime of the process.
static CL100K_TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();

/// Get or initialise the cl100k_base tokenizer.
fn cl100k_tokenizer() -> &'static CoreBPE {
    CL100K_TOKENIZER.get_or_init(|| {
        tiktoken_rs::cl100k_base().expect("Failed to initialise cl100k_base tokenizer")
    })
}

/// Estimate the number of tokens in `text` for the given model.
///
/// Uses tiktoken-rs with the cl100k_base encoding (GPT-4 / GPT-3.5-turbo family).
/// Falls back to a heuristic (`text.len() / 4`) for models whose tokenizer is
/// not available.
///
/// # Arguments
///
/// * `text` - The text to tokenize
/// * `model` - Model name (e.g., "gpt-4", "deepseek-r1", "llama3")
pub fn estimate_tokens(text: &str, model: &str) -> usize {
    // For known OpenAI-family models, use the exact tokenizer
    let lower = model.to_lowercase();
    if lower.contains("gpt-4")
        || lower.contains("gpt-3.5")
        || lower.contains("gpt-4o")
        || lower.contains("o1")
        || lower.contains("o3")
    {
        return cl100k_tokenizer().encode_ordinary(text).len();
    }

    // For other models, try cl100k as a reasonable approximation
    // cl100k is close enough for most modern LLMs
    if lower.contains("deepseek")
        || lower.contains("llama")
        || lower.contains("mistral")
        || lower.contains("qwen")
    {
        return cl100k_tokenizer().encode_ordinary(text).len();
    }

    // Ultimate fallback: ~4 characters per token heuristic
    text.len() / 4
}

/// Estimate token count for a `SessionMessage`, including role overhead,
/// content, tool_name, and metadata.
pub fn estimate_message_tokens(message: &SessionMessage, model: &str) -> usize {
    let mut text = String::with_capacity(message.content.len() + 64);

    // Role token overhead (approx 4 tokens for role + delimiters)
    text.push_str(&message.role);
    text.push_str(": ");
    text.push_str(&message.content);

    if let Some(ref tool_name) = message.tool_name {
        text.push_str("\ntool: ");
        text.push_str(tool_name);
    }

    // Metadata can contribute tokens if non-empty
    if message.metadata != json!({}) {
        let meta_str = message.metadata.to_string();
        text.push_str("\nmetadata: ");
        text.push_str(&meta_str);
    }

    // Add per-message overhead (role markers, separators) — ~4 tokens
    estimate_tokens(&text, model) + 4
}

// =============================================================================
// SEGMENT TYPES AND PRIORITIES
// =============================================================================

/// Priority level for context segments.
///
/// Lower numeric value = higher priority = loaded first and pruned last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SegmentPriority {
    /// Soul directives — always included, never pruned
    P0 = 0,
    /// Recent memories — high importance, pruned by importance
    P1 = 1,
    /// Task context — always included, never pruned
    P2 = 2,
    /// Conversation history — pruned oldest-first
    P3 = 3,
    /// Tool results — lowest priority, soft-trimmed then hard-cleared
    P4 = 4,
}

/// Origin type for a context segment, used in provenance tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SegmentSourceType {
    SoulDirective,
    Memory,
    TaskContext,
    ConversationMessage,
    ToolResult,
}

impl std::fmt::Display for SegmentSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SoulDirective => write!(f, "soul_directive"),
            Self::Memory => write!(f, "memory"),
            Self::TaskContext => write!(f, "task_context"),
            Self::ConversationMessage => write!(f, "conversation_message"),
            Self::ToolResult => write!(f, "tool_result"),
        }
    }
}

/// A single segment of assembled context.
///
/// Each segment carries its content, token estimate, priority, and provenance
/// metadata so the assembly pipeline can sort, prune, and audit the final bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSegment {
    /// Priority level (P0 = highest)
    pub priority: SegmentPriority,
    /// Assembled text content for this segment
    pub content: String,
    /// Estimated token count
    pub token_estimate: usize,
    /// Origin type for provenance
    pub source_type: SegmentSourceType,
    /// Source identifier (memory_id, run_id, message_id as UUID)
    pub source_id: Option<Uuid>,
    /// Additional provenance metadata (e.g., message_id as i64, tool_call_id)
    pub metadata: serde_json::Value,
    /// Insertion order index for stable sorting within the same priority
    pub insertion_order: usize,
}

// =============================================================================
// CONTEXT PROVENANCE
// =============================================================================

/// Provenance record for an assembled context bundle.
///
/// Captures which data sources contributed to the context, enabling audit
/// trails and debugging of model behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextProvenance {
    /// Memory IDs included in the context
    pub memory_ids: Vec<Uuid>,
    /// Task run IDs included in the context
    pub run_ids: Vec<Uuid>,
    /// Message IDs included in the context
    pub message_ids: Vec<i64>,
    /// blake3 hash of the concatenated segment contents (canonical order)
    pub context_bundle_hash: String,
    /// Total estimated tokens in the assembled context
    pub total_tokens: usize,
    /// Count of segments by source type
    pub segment_counts: HashMap<String, usize>,
}

// =============================================================================
// CONTEXT WINDOW
// =============================================================================

/// Priority-based context assembly pipeline.
///
/// Loads segments from multiple data sources (soul directives, memories, tasks,
/// conversation history, tool results), enforces a token budget, and produces
/// a single context string ready for model consumption.
pub struct ContextWindow {
    /// Database connection pool
    pool: PgPool,
    /// Optional event stream for observability
    event_stream: Option<Arc<EventStream>>,
    /// Assembled segments
    segments: Vec<ContextSegment>,
    /// Current total token count across all segments
    total_tokens: usize,
    /// Token budget (target ceiling after pruning)
    budget_tokens: usize,
    /// Model name for tokenizer selection
    model: String,
    /// Provider name (e.g., "ollama", "openai")
    provider: String,
    /// Session ID (if building for a session)
    session_id: Option<Uuid>,
    /// Task ID (if building for a task)
    task_id: Option<Uuid>,
    /// Identity ID resolved from the session
    identity_id: Option<Uuid>,
}

impl ContextWindow {
    /// Create a new `ContextWindow` with default settings.
    ///
    /// # Arguments
    ///
    /// * `pool` - PostgreSQL connection pool
    /// * `event_stream` - Optional event stream for audit events
    pub fn new(pool: PgPool, event_stream: Option<Arc<EventStream>>) -> Self {
        Self {
            pool,
            event_stream,
            segments: Vec::new(),
            total_tokens: 0,
            budget_tokens: 32_000,
            model: "deepseek-r1:7b".to_string(),
            provider: "ollama".to_string(),
            session_id: None,
            task_id: None,
            identity_id: None,
        }
    }

    /// Apply configuration from a `Config` instance.
    ///
    /// Sets `budget_tokens` from `context_window_tokens` and `context_reserve_percent`.
    /// Tool trim/clear thresholds are read directly from `&Config` in `assemble()`.
    #[must_use]
    pub fn with_config(mut self, config: &Config) -> Self {
        let reserve_fraction = f64::from(config.context_reserve_percent) / 100.0;
        self.budget_tokens =
            (config.context_window_tokens as f64 * (1.0 - reserve_fraction)) as usize;
        self
    }

    /// Set the token budget.
    #[must_use]
    pub fn with_budget(mut self, tokens: usize) -> Self {
        self.budget_tokens = tokens;
        self
    }

    /// Set the model and provider for tokenizer selection.
    #[must_use]
    pub fn with_model(mut self, provider: &str, model: &str) -> Self {
        self.provider = provider.to_string();
        self.model = model.to_string();
        self
    }

    /// Associate a session with this context window.
    #[must_use]
    pub fn for_session(mut self, session_id: Uuid) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Associate a task with this context window.
    #[must_use]
    pub fn for_task(mut self, task_id: Uuid) -> Self {
        self.task_id = Some(task_id);
        self
    }

    /// Add a pre-built segment to the context window.
    ///
    /// Use this when you need full control over the segment metadata (e.g.,
    /// setting `message_id` or `run_id` in metadata for provenance tracking).
    /// The segment's `insertion_order` is set to the current segment count and
    /// `total_tokens` is updated automatically.
    pub fn add_segment(&mut self, mut segment: ContextSegment) {
        segment.insertion_order = self.segments.len();
        self.total_tokens += segment.token_estimate;
        self.segments.push(segment);
    }

    /// Add a raw segment with the given priority, content, source type, and optional source ID.
    ///
    /// Token estimation is computed automatically using the configured model tokenizer.
    pub fn add_raw_segment(
        &mut self,
        priority: SegmentPriority,
        content: String,
        source_type: SegmentSourceType,
        source_id: Option<Uuid>,
    ) {
        let tokens = estimate_tokens(&content, &self.model);
        let idx = self.segments.len();
        self.segments.push(ContextSegment {
            priority,
            content,
            token_estimate: tokens,
            source_type,
            source_id,
            metadata: serde_json::json!({}),
            insertion_order: idx,
        });
        self.total_tokens += tokens;
    }

    // =========================================================================
    // P0: SOUL DIRECTIVES
    // =========================================================================

    /// Load soul directives for the given identity (P0 — never pruned).
    ///
    /// Queries the `identities.directives` JSONB column, deserializes into
    /// `Vec<SoulDirective>`, sorts by priority, and adds each as a P0 segment.
    pub async fn load_soul_directives(&mut self, identity_id: Uuid) -> Result<()> {
        let row = sqlx::query("SELECT directives FROM identities WHERE identity_id = $1")
            .bind(identity_id)
            .fetch_optional(&self.pool)
            .await?;

        let Some(row) = row else {
            tracing::warn!(identity_id = %identity_id, "No identity found for soul directives");
            return Ok(());
        };

        let directives_json: serde_json::Value = row.get("directives");
        let mut directives: Vec<SoulDirective> =
            serde_json::from_value(directives_json).unwrap_or_default();

        // Sort by priority (0 = highest)
        directives.sort_by_key(|d| d.priority);

        for directive in &directives {
            let content = format!("## {}\n- {}", directive.category, directive.content);
            let tokens = estimate_tokens(&content, &self.model);

            let idx = self.segments.len();
            self.segments.push(ContextSegment {
                priority: SegmentPriority::P0,
                content,
                token_estimate: tokens,
                source_type: SegmentSourceType::SoulDirective,
                source_id: None,
                metadata: json!({
                    "category": directive.category,
                    "priority": directive.priority,
                }),
                insertion_order: idx,
            });
            self.total_tokens += tokens;
        }

        tracing::debug!(
            identity_id = %identity_id,
            directive_count = directives.len(),
            "Loaded soul directives (P0)"
        );

        Ok(())
    }

    // =========================================================================
    // P1: RECENT MEMORIES
    // =========================================================================

    /// Load recent and high-importance memories for the given identity (P1).
    ///
    /// Combines the "today + yesterday" (48hr) window with high-importance
    /// memories (importance > 0.8), deduplicates by memory_id, and takes
    /// the top `limit` results sorted by importance then access time.
    pub async fn load_recent_memories(&mut self, identity_id: Uuid, limit: usize) -> Result<()> {
        let manager = MemoryManager::new(self.pool.clone(), None);

        // Load both recent (48hr) and high-importance memories
        #[allow(clippy::cast_possible_wrap)]
        let limit_i64 = limit as i64;
        let recent = manager.load_recent_memories(identity_id, limit_i64).await?;
        let important = manager
            .load_high_importance_memories(identity_id, 0.8, limit_i64)
            .await?;

        // Deduplicate by memory_id
        let mut seen = std::collections::HashSet::new();
        let mut combined = Vec::new();

        for mem in recent.into_iter().chain(important) {
            if seen.insert(mem.memory_id) {
                combined.push(mem);
            }
        }

        // Sort by importance DESC, then accessed_at DESC
        combined.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.accessed_at.cmp(&a.accessed_at))
        });

        // Take top `limit`
        combined.truncate(limit);

        for mem in &combined {
            let source_label = mem
                .source_type()
                .map_or_else(|| mem.source.clone(), |s| s.to_string());
            let content = format!("[Memory from {}] {}", source_label, mem.content);
            let tokens = estimate_tokens(&content, &self.model);

            let idx = self.segments.len();
            self.segments.push(ContextSegment {
                priority: SegmentPriority::P1,
                content,
                token_estimate: tokens,
                source_type: SegmentSourceType::Memory,
                source_id: Some(mem.memory_id),
                metadata: json!({
                    "importance": mem.importance,
                    "source": mem.source,
                    "created_at": mem.created_at.to_rfc3339(),
                }),
                insertion_order: idx,
            });
            self.total_tokens += tokens;
        }

        tracing::debug!(
            identity_id = %identity_id,
            memory_count = combined.len(),
            "Loaded recent memories (P1)"
        );

        Ok(())
    }

    // =========================================================================
    // P2: TASK CONTEXT
    // =========================================================================

    /// Load task context for the given task (P2 — never pruned).
    ///
    /// Queries the `tasks` table for task details and `task_runs` for the
    /// most recent run, formatting them as structured context.
    pub async fn load_task_context(&mut self, task_id: Uuid) -> Result<()> {
        // Load task details
        let task_row =
            sqlx::query("SELECT task_id, title, description, state FROM tasks WHERE task_id = $1")
                .bind(task_id)
                .fetch_optional(&self.pool)
                .await?;

        let Some(task_row) = task_row else {
            tracing::warn!(task_id = %task_id, "No task found for context loading");
            return Ok(());
        };

        let title: String = task_row.get("title");
        let description: Option<String> = task_row.get("description");
        let state: String = task_row.get("state");

        let mut content = format!(
            "## Current Task\nTitle: {}\nDescription: {}\nState: {}",
            title,
            description.as_deref().unwrap_or("(none)"),
            state,
        );

        // Load most recent run
        let run_row = sqlx::query(
            r"SELECT run_id, state, result, error
              FROM task_runs
              WHERE task_id = $1
              ORDER BY started_at DESC
              LIMIT 1",
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await?;

        let mut run_id: Option<Uuid> = None;

        if let Some(run) = run_row {
            let rid: Uuid = run.get("run_id");
            let run_state: String = run.get("state");
            let result: Option<String> = run.get("result");
            let error: Option<String> = run.get("error");
            run_id = Some(rid);

            content.push_str(&format!("\n\n## Latest Run\nState: {}", run_state));
            if let Some(ref r) = result {
                content.push_str(&format!("\nResult: {}", r));
            }
            if let Some(ref e) = error {
                content.push_str(&format!("\nError: {}", e));
            }
        }

        let tokens = estimate_tokens(&content, &self.model);

        let idx = self.segments.len();
        self.segments.push(ContextSegment {
            priority: SegmentPriority::P2,
            content,
            token_estimate: tokens,
            source_type: SegmentSourceType::TaskContext,
            source_id: Some(task_id),
            metadata: json!({
                "task_id": task_id,
                "run_id": run_id,
                "state": state,
            }),
            insertion_order: idx,
        });
        self.total_tokens += tokens;

        tracing::debug!(task_id = %task_id, "Loaded task context (P2)");

        Ok(())
    }

    // =========================================================================
    // P3: CONVERSATION HISTORY
    // =========================================================================

    /// Load conversation history for the given session (P3).
    ///
    /// Loads the most recent messages (excluding tool-role messages, which are
    /// loaded separately as P4), ordered chronologically (oldest first).
    pub async fn load_conversation_history(
        &mut self,
        session_id: Uuid,
        limit: usize,
    ) -> Result<()> {
        // Load messages in reverse order (newest first), then reverse for chronological
        let messages: Vec<SessionMessage> = sqlx::query_as(
            r"SELECT * FROM session_messages
              WHERE session_id = $1 AND role != 'tool'
              ORDER BY message_id DESC
              LIMIT $2",
        )
        .bind(session_id)
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await?;

        // Reverse to chronological order (oldest first)
        for msg in messages.iter().rev() {
            let content = format!("{}: {}", msg.role, msg.content);

            // Use stored token_estimate if available, otherwise estimate
            let tokens = msg
                .token_estimate
                .map_or_else(|| estimate_message_tokens(msg, &self.model), |t| t as usize);

            // Store message_id as i64 in metadata (not as UUID source_id)
            let idx = self.segments.len();
            self.segments.push(ContextSegment {
                priority: SegmentPriority::P3,
                content,
                token_estimate: tokens,
                source_type: SegmentSourceType::ConversationMessage,
                source_id: None,
                metadata: json!({
                    "message_id": msg.message_id,
                    "role": msg.role,
                    "ts": msg.ts.to_rfc3339(),
                }),
                insertion_order: idx,
            });
            self.total_tokens += tokens;
        }

        tracing::debug!(
            session_id = %session_id,
            message_count = messages.len(),
            "Loaded conversation history (P3)"
        );

        Ok(())
    }

    // =========================================================================
    // P4: TOOL RESULTS
    // =========================================================================

    /// Load tool results for the given session (P4 — lowest priority, prunable).
    ///
    /// Queries `session_messages` for tool-role messages, ordered newest first.
    pub async fn load_tool_results(&mut self, session_id: Uuid, limit: usize) -> Result<()> {
        let messages: Vec<SessionMessage> = sqlx::query_as(
            r"SELECT * FROM session_messages
              WHERE session_id = $1 AND role = 'tool'
              ORDER BY message_id DESC
              LIMIT $2",
        )
        .bind(session_id)
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await?;

        // Reverse to chronological order
        for msg in messages.iter().rev() {
            let tool_label = msg.tool_name.as_deref().unwrap_or("unknown");
            let content = format!("[Tool: {}] {}", tool_label, msg.content);
            let tokens = estimate_tokens(&content, &self.model);

            let idx = self.segments.len();
            self.segments.push(ContextSegment {
                priority: SegmentPriority::P4,
                content,
                token_estimate: tokens,
                source_type: SegmentSourceType::ToolResult,
                source_id: None,
                metadata: json!({
                    "message_id": msg.message_id,
                    "tool_name": tool_label,
                    "tool_call_id": msg.tool_call_id,
                    "ts": msg.ts.to_rfc3339(),
                }),
                insertion_order: idx,
            });
            self.total_tokens += tokens;
        }

        tracing::debug!(
            session_id = %session_id,
            tool_result_count = messages.len(),
            "Loaded tool results (P4)"
        );

        Ok(())
    }

    // =========================================================================
    // SOFT-TRIM FOR OVERSIZED TOOL RESULTS
    // =========================================================================

    /// Soft-trim a tool result to fit within `max_tokens`.
    ///
    /// Preserves the head (60%) and tail (40%) of the content, inserting an
    /// ellipsis separator indicating the omitted middle section.
    pub fn soft_trim_tool_result(content: &str, max_tokens: usize, model: &str) -> String {
        let current_tokens = estimate_tokens(content, model);
        if current_tokens <= max_tokens {
            return content.to_string();
        }

        // Approximate character budget from token budget
        // Use the ratio of current chars to current tokens
        let chars_per_token = if current_tokens > 0 {
            content.len() as f64 / current_tokens as f64
        } else {
            4.0
        };

        let target_chars = (max_tokens as f64 * chars_per_token) as usize;
        let head_chars = (target_chars as f64 * 0.6) as usize;
        let tail_chars = (target_chars as f64 * 0.4) as usize;

        // Ensure we don't exceed content length
        let head_chars = head_chars.min(content.len());
        let tail_start = content.len().saturating_sub(tail_chars);

        // Find safe split points (avoid splitting mid-character for UTF-8)
        let head_end = content
            .char_indices()
            .take_while(|(i, _)| *i < head_chars)
            .last()
            .map_or(0, |(i, c)| i + c.len_utf8());

        let tail_begin = content
            .char_indices()
            .find(|(i, _)| *i >= tail_start)
            .map_or(content.len(), |(i, _)| i);

        let trimmed_tokens = current_tokens.saturating_sub(max_tokens);
        let separator = format!("\n\n[... {} tokens omitted ...]\n\n", trimmed_tokens);

        format!(
            "{}{}{}",
            &content[..head_end],
            separator,
            &content[tail_begin..],
        )
    }

    // =========================================================================
    // HARD-CLEAR FOR OLD TOOL RESULTS
    // =========================================================================

    /// Remove tool result segments older than `age_threshold_secs`.
    ///
    /// Iterates through P4 segments, checks the message timestamp from metadata,
    /// and removes segments that exceed the age threshold.
    fn hard_clear_old_tool_results(&mut self, age_threshold_secs: i64) {
        let now = Utc::now();
        let mut removed_count = 0usize;
        let mut removed_tokens = 0usize;

        self.segments.retain(|seg| {
            if seg.priority != SegmentPriority::P4 {
                return true;
            }

            // Parse timestamp from metadata
            if let Some(ts_str) = seg.metadata.get("ts").and_then(|v| v.as_str()) {
                if let Ok(ts) = ts_str.parse::<DateTime<Utc>>() {
                    let age_secs = (now - ts).num_seconds();
                    if age_secs > age_threshold_secs {
                        removed_count += 1;
                        removed_tokens += seg.token_estimate;
                        return false;
                    }
                }
            }

            true
        });

        if removed_count > 0 {
            self.total_tokens = self.total_tokens.saturating_sub(removed_tokens);
            tracing::debug!(
                removed_count,
                removed_tokens,
                "Hard-cleared old tool results"
            );
        }
    }

    // =========================================================================
    // TOKEN BUDGET ENFORCEMENT
    // =========================================================================

    /// Enforce the token budget by pruning segments in reverse priority order.
    ///
    /// Pruning strategy (applied until budget is met):
    /// 1. Hard-clear old tool results (P4, age > threshold)
    /// 2. Soft-trim oversized tool results (P4, > threshold tokens)
    /// 3. Drop oldest conversation messages (P3)
    /// 4. Drop lowest-importance memories (P1)
    /// 5. Never drop soul directives (P0) or task context (P2)
    ///
    /// The reserve headroom is already factored into `budget_tokens` (set via
    /// `Config::context_reserve_percent`), so this method targets `budget_tokens`
    /// directly.
    #[allow(clippy::too_many_lines)]
    pub fn enforce_budget(&mut self, tool_trim_threshold: usize, tool_clear_age_secs: i64) {
        let target = self.budget_tokens;

        if self.total_tokens <= target {
            return;
        }

        // Step 1: Hard-clear old tool results
        self.hard_clear_old_tool_results(tool_clear_age_secs);
        if self.total_tokens <= target {
            return;
        }

        // Step 2: Soft-trim oversized tool results
        let max_tool_tokens = tool_trim_threshold;
        let model = self.model.clone();
        for seg in &mut self.segments {
            if seg.priority == SegmentPriority::P4 && seg.token_estimate > max_tool_tokens {
                let trimmed = Self::soft_trim_tool_result(&seg.content, max_tool_tokens, &model);
                let new_tokens = estimate_tokens(&trimmed, &model);
                let saved = seg.token_estimate.saturating_sub(new_tokens);
                seg.content = trimmed;
                seg.token_estimate = new_tokens;
                self.total_tokens = self.total_tokens.saturating_sub(saved);
            }
        }
        if self.total_tokens <= target {
            return;
        }

        // Step 3: Drop oldest conversation messages (P3)
        // Sort P3 segments by message_id ascending (oldest first), drop from front
        while self.total_tokens > target {
            // Find the oldest P3 segment
            let oldest_p3_idx = self
                .segments
                .iter()
                .enumerate()
                .filter(|(_, s)| s.priority == SegmentPriority::P3)
                .min_by_key(|(_, s)| {
                    s.metadata
                        .get("message_id")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(i64::MAX)
                })
                .map(|(i, _)| i);

            let Some(idx) = oldest_p3_idx else {
                break;
            };

            let removed = self.segments.remove(idx);
            self.total_tokens = self.total_tokens.saturating_sub(removed.token_estimate);

            self.emit_event(
                EventType::ContextPruned,
                json!({
                    "pruned_type": "conversation_message",
                    "message_id": removed.metadata.get("message_id"),
                    "tokens_freed": removed.token_estimate,
                }),
            );
        }

        if self.total_tokens <= target {
            return;
        }

        // Step 4: Drop lowest-importance memories (P1)
        while self.total_tokens > target {
            // Find the lowest-importance P1 segment
            let lowest_p1_idx = self
                .segments
                .iter()
                .enumerate()
                .filter(|(_, s)| s.priority == SegmentPriority::P1)
                .min_by(|(_, a), (_, b)| {
                    let imp_a = a
                        .metadata
                        .get("importance")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let imp_b = b
                        .metadata
                        .get("importance")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    imp_a
                        .partial_cmp(&imp_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i);

            let Some(idx) = lowest_p1_idx else {
                break;
            };

            let removed = self.segments.remove(idx);
            self.total_tokens = self.total_tokens.saturating_sub(removed.token_estimate);

            self.emit_event(
                EventType::ContextPruned,
                json!({
                    "pruned_type": "memory",
                    "memory_id": removed.source_id,
                    "importance": removed.metadata.get("importance"),
                    "tokens_freed": removed.token_estimate,
                }),
            );
        }

        // If still over budget, emit warning
        if self.total_tokens > target {
            tracing::warn!(
                total_tokens = self.total_tokens,
                budget = self.budget_tokens,
                target,
                "Context budget exceeded even after pruning"
            );

            self.emit_event(
                EventType::ContextBudgetExceeded,
                json!({
                    "total_tokens": self.total_tokens,
                    "budget_tokens": self.budget_tokens,
                    "target_tokens": target,
                }),
            );
        }
    }

    // =========================================================================
    // PROVENANCE TRACKING
    // =========================================================================

    /// Compute provenance metadata for the current segment set.
    ///
    /// Collects source IDs by type, computes a blake3 hash of the concatenated
    /// segment contents, and returns a `ContextProvenance` struct for audit logging.
    pub fn compute_provenance(&self) -> ContextProvenance {
        let mut memory_ids = Vec::new();
        let mut run_ids = Vec::new();
        let mut message_ids = Vec::new();
        let mut segment_counts: HashMap<String, usize> = HashMap::new();

        // Concatenate all content for hashing (in segment order)
        let mut hash_input = String::new();

        for seg in &self.segments {
            // Count by source type
            *segment_counts
                .entry(seg.source_type.to_string())
                .or_insert(0) += 1;

            // Collect source IDs
            match seg.source_type {
                SegmentSourceType::Memory => {
                    if let Some(id) = seg.source_id {
                        memory_ids.push(id);
                    }
                }
                SegmentSourceType::TaskContext => {
                    if let Some(id) = seg.metadata.get("run_id").and_then(|v| v.as_str()) {
                        if let Ok(uid) = Uuid::parse_str(id) {
                            run_ids.push(uid);
                        }
                    }
                }
                SegmentSourceType::ConversationMessage | SegmentSourceType::ToolResult => {
                    if let Some(mid) = seg.metadata.get("message_id").and_then(|v| v.as_i64()) {
                        message_ids.push(mid);
                    }
                }
                SegmentSourceType::SoulDirective => {}
            }

            hash_input.push_str(&seg.content);
            hash_input.push('\n');
        }

        let context_bundle_hash = hex::encode(blake3::hash(hash_input.as_bytes()).as_bytes());

        ContextProvenance {
            memory_ids,
            run_ids,
            message_ids,
            context_bundle_hash,
            total_tokens: self.total_tokens,
            segment_counts,
        }
    }

    // =========================================================================
    // CONTEXT BUNDLE ASSEMBLY
    // =========================================================================

    /// Assemble the full context string from all loaded segments.
    ///
    /// Reads `tool_trim_threshold` and `tool_clear_age_secs` from the supplied
    /// `Config`. Calls all load methods in priority order, applies soft-trim to
    /// oversized tool results, enforces the token budget, and concatenates
    /// segments with separators.
    ///
    /// Instances are mutable; for repeated use, call `assemble()` each time
    /// (auto-resets) or create fresh via `build_for_session()`.
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration supplying trim/clear thresholds
    pub async fn assemble(&mut self, config: &Config) -> Result<String> {
        let identity_id = self.identity_id;
        let session_id = self.session_id;
        let task_id = self.task_id;

        // Clear prior segment state so repeated calls don't accumulate duplicates.
        // Cached fields (budget_tokens, model, session_id, identity_id, task_id) remain set.
        self.segments.clear();
        self.total_tokens = 0;

        // 1. Load soul directives (P0)
        if let Some(iid) = identity_id {
            self.load_soul_directives(iid).await?;
        }

        // 2. Load recent memories (P1)
        if let Some(iid) = identity_id {
            self.load_recent_memories(iid, 10).await?;
        }

        // 3. Load task context (P2) if task_id provided
        if let Some(tid) = task_id {
            self.load_task_context(tid).await?;
        }

        // 4. Load conversation history (P3)
        if let Some(sid) = session_id {
            self.load_conversation_history(sid, 50).await?;
        }

        // 5. Load tool results (P4)
        if let Some(sid) = session_id {
            self.load_tool_results(sid, 20).await?;
        }

        // 6. Enforce budget (prune if needed) using config thresholds
        self.enforce_budget(config.tool_trim_threshold, config.tool_clear_age_secs);

        // 7. Sort by (priority, insertion_order) for deterministic chronological order within groups
        self.segments
            .sort_by_key(|s| (s.priority, s.insertion_order));

        // 8. Concatenate with separators
        let mut output = String::new();
        let mut current_priority = None;

        for seg in &self.segments {
            if current_priority != Some(seg.priority) {
                if !output.is_empty() {
                    output.push_str("\n\n---\n\n");
                }
                current_priority = Some(seg.priority);
            } else {
                output.push_str("\n\n");
            }
            output.push_str(&seg.content);
        }

        self.emit_event(
            EventType::ContextAssembled,
            json!({
                "total_tokens": self.total_tokens,
                "budget_tokens": self.budget_tokens,
                "segment_count": self.segments.len(),
                "session_id": session_id,
                "task_id": task_id,
            }),
        );

        Ok(output)
    }

    // =========================================================================
    // PRE-SEND AUDIT LOGGING
    // =========================================================================

    /// Log the assembled context to the audit ledger.
    ///
    /// Records provenance metadata (memory IDs, run IDs, message IDs, bundle hash)
    /// as a ledger event for post-hoc auditing of model inputs.
    pub async fn log_to_ledger(&self, ledger: &Ledger, correlation_id: Uuid) -> Result<i64> {
        tracing::debug!(
            segments = self.segments.len(),
            total_tokens = self.total_tokens,
            correlation_id = %correlation_id,
            "Computing context provenance for ledger"
        );

        let provenance = self.compute_provenance();

        let payload = json!({
            "action": "model.context.assembled",
            "session_id": self.session_id.map(|id| id.to_string()),
            "task_id": self.task_id.map(|id| id.to_string()),
            "context_bundle_hash": provenance.context_bundle_hash,
            "total_tokens": provenance.total_tokens,
            "segment_counts": provenance.segment_counts,
            "memory_ids": provenance.memory_ids,
            "run_ids": provenance.run_ids,
            "message_ids": provenance.message_ids,
        });

        let metadata = json!({
            "context_bundle_hash": provenance.context_bundle_hash,
            "memory_ids": provenance.memory_ids,
            "run_ids": provenance.run_ids,
            "message_ids": provenance.message_ids,
            "session_id": self.session_id.map(|id| id.to_string()),
            "total_tokens": provenance.total_tokens,
            "segment_counts": provenance.segment_counts,
        });

        let event_id = ledger
            .append_event(
                None,
                "model.context.assembled",
                payload,
                Some(correlation_id),
                None,
                Some(metadata),
                None,
                None,
            )
            .await?;

        tracing::info!(
            event_id,
            correlation_id = %correlation_id,
            total_tokens = provenance.total_tokens,
            "Context assembly logged to ledger"
        );

        Ok(event_id)
    }

    /// Compute provenance and log context integrity to the ledger in one step.
    ///
    /// This is a convenience wrapper around `compute_provenance` and
    /// `log_to_ledger` that returns both the ledger `event_id` and the
    /// full [`ContextProvenance`] record. Use this when the caller needs
    /// the provenance metadata (e.g., for correlation with downstream events)
    /// in addition to the ledger audit trail.
    ///
    /// # Arguments
    ///
    /// * `ledger` - Tamper-resistant audit ledger
    /// * `correlation_id` - UUID linking this event to subsequent model call events
    ///
    /// # Returns
    ///
    /// `(event_id, ContextProvenance)` on success.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let (event_id, provenance) = ctx.log_context_integrity(&ledger, correlation_id).await?;
    /// println!("Logged event {} with hash {}", event_id, provenance.context_bundle_hash);
    /// ```
    pub async fn log_context_integrity(
        &self,
        ledger: &Ledger,
        correlation_id: Uuid,
    ) -> Result<(i64, ContextProvenance)> {
        tracing::debug!(
            segments = self.segments.len(),
            total_tokens = self.total_tokens,
            correlation_id = %correlation_id,
            "Computing context provenance for integrity log"
        );

        let provenance = self.compute_provenance();

        let payload = json!({
            "action": "model.context.assembled",
            "session_id": self.session_id.map(|id| id.to_string()),
            "task_id": self.task_id.map(|id| id.to_string()),
            "context_bundle_hash": provenance.context_bundle_hash,
            "total_tokens": provenance.total_tokens,
            "segment_counts": provenance.segment_counts,
            "memory_ids": provenance.memory_ids,
            "run_ids": provenance.run_ids,
            "message_ids": provenance.message_ids,
        });

        let metadata = json!({
            "context_bundle_hash": provenance.context_bundle_hash,
            "memory_ids": provenance.memory_ids,
            "run_ids": provenance.run_ids,
            "message_ids": provenance.message_ids,
            "session_id": self.session_id.map(|id| id.to_string()),
            "total_tokens": provenance.total_tokens,
            "segment_counts": provenance.segment_counts,
        });

        let event_id = ledger
            .append_event(
                None,
                "model.context.assembled",
                payload,
                Some(correlation_id),
                None,
                Some(metadata),
                None,
                None,
            )
            .await?;

        tracing::info!(
            event_id,
            correlation_id = %correlation_id,
            context_bundle_hash = %provenance.context_bundle_hash,
            total_tokens = provenance.total_tokens,
            "Context integrity logged to ledger"
        );

        Ok((event_id, provenance))
    }

    // =========================================================================
    // CONTEXT WINDOW LIMIT RESOLUTION
    // =========================================================================

    /// Resolve the effective context window limit for a session.
    ///
    /// Fallback hierarchy:
    /// 1. Session-specific limit (`sessions.context_window_limit`)
    /// 2. Model-specific limit (from `model_providers.config` JSONB)
    /// 3. Provider default (128000 for Ollama)
    /// 4. Hard-coded default: 32000 tokens
    pub async fn resolve_context_window_limit(&self, session_id: Uuid) -> Result<usize> {
        // 1. Check session-specific limit
        let session_row =
            sqlx::query("SELECT context_window_limit FROM sessions WHERE session_id = $1")
                .bind(session_id)
                .fetch_optional(&self.pool)
                .await?;

        if let Some(row) = session_row {
            let limit: Option<i32> = row.get("context_window_limit");
            if let Some(l) = limit {
                return Ok(l as usize);
            }
        }

        // 2. Check model_providers for model-specific config
        let provider_row = sqlx::query(
            r"SELECT config FROM model_providers
              WHERE provider_type = $1
              LIMIT 1",
        )
        .bind(&self.provider)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = provider_row {
            let config: serde_json::Value = row.get("config");
            if let Some(limit) = config.get("context_window").and_then(|v| v.as_i64()) {
                return Ok(limit as usize);
            }
        }

        // 3. Provider defaults
        let default = match self.provider.as_str() {
            "ollama" => 128_000,
            "openai" => 128_000,
            _ => 32_000,
        };

        Ok(default)
    }

    // =========================================================================
    // SESSION INTEGRATION
    // =========================================================================

    /// Build a fully configured `ContextWindow` for a session.
    ///
    /// Loads session details, resolves the context window limit, applies
    /// `Config` for budget reserve percentage and tool trim/clear thresholds,
    /// and returns a builder ready for `assemble()`.
    ///
    /// # Arguments
    ///
    /// * `pool` - PostgreSQL connection pool
    /// * `event_stream` - Optional event stream
    /// * `session_id` - Session to build context for
    /// * `task_id` - Optional task for P2 context
    /// * `config` - Application configuration for context window settings
    pub async fn build_for_session(
        pool: PgPool,
        event_stream: Option<Arc<EventStream>>,
        session_id: Uuid,
        task_id: Option<Uuid>,
        config: &Config,
    ) -> Result<Self> {
        // Load session to get agent_id
        let session_row = sqlx::query(
            "SELECT agent_id, context_window_limit FROM sessions WHERE session_id = $1",
        )
        .bind(session_id)
        .fetch_optional(&pool)
        .await?;

        let Some(session_row) = session_row else {
            return Err(Error::Session(format!("Session {} not found", session_id)));
        };

        let agent_id: Uuid = session_row.get("agent_id");

        let mut ctx = Self::new(pool, event_stream)
            .with_config(config)
            .for_session(session_id);

        if let Some(tid) = task_id {
            ctx = ctx.for_task(tid);
        }

        ctx.identity_id = Some(agent_id);

        // Resolve context window limit; override the config-derived budget using
        // the session/provider-specific limit combined with the config reserve %.
        let limit = ctx.resolve_context_window_limit(session_id).await?;
        let reserve_fraction = f64::from(config.context_reserve_percent) / 100.0;
        ctx.budget_tokens = (limit as f64 * (1.0 - reserve_fraction)) as usize;

        tracing::debug!(
            session_id = %session_id,
            agent_id = %agent_id,
            context_limit = limit,
            budget_tokens = ctx.budget_tokens,
            tool_trim_threshold = config.tool_trim_threshold,
            tool_clear_age_secs = config.tool_clear_age_secs,
            "Context window configured for session"
        );

        Ok(ctx)
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
    // Token estimation tests
    // =========================================================================

    #[test]
    fn test_estimate_tokens_gpt4() {
        let text = "Hello, world! This is a test of the token estimation function.";
        let tokens = estimate_tokens(text, "gpt-4");
        // cl100k_base should give a reasonable count (roughly 13-15 tokens)
        assert!(tokens > 5, "Expected > 5 tokens, got {}", tokens);
        assert!(tokens < 30, "Expected < 30 tokens, got {}", tokens);
    }

    #[test]
    fn test_estimate_tokens_deepseek() {
        let text = "Hello, world! This is a test of the token estimation function.";
        let tokens = estimate_tokens(text, "deepseek-r1:7b");
        assert!(tokens > 5, "Expected > 5 tokens, got {}", tokens);
        assert!(tokens < 30, "Expected < 30 tokens, got {}", tokens);
    }

    #[test]
    fn test_estimate_tokens_unknown_model_fallback() {
        let text = "a]b]c]d]"; // 8 chars → ~2 tokens with /4 heuristic
        let tokens = estimate_tokens(text, "totally-unknown-model-xyz");
        assert_eq!(tokens, 2, "Fallback should be len/4 = 2");
    }

    #[test]
    fn test_estimate_tokens_empty_string() {
        let tokens = estimate_tokens("", "gpt-4");
        assert_eq!(tokens, 0);
    }

    #[test]
    fn test_estimate_message_tokens() {
        let msg = SessionMessage {
            message_id: 1,
            session_id: Uuid::new_v4(),
            ts: Utc::now(),
            role: "user".to_string(),
            content: "Hello, how are you?".to_string(),
            tool_name: None,
            tool_call_id: None,
            correlation_id: None,
            token_estimate: None,
            metadata: json!({}),
            tool_metadata: json!({}),
        };
        let tokens = estimate_message_tokens(&msg, "gpt-4");
        // Should include role overhead + content tokens + 4 per-message overhead
        assert!(tokens > 5, "Expected > 5 tokens, got {}", tokens);
        assert!(tokens < 30, "Expected < 30 tokens, got {}", tokens);
    }

    // =========================================================================
    // Segment priority ordering tests
    // =========================================================================

    #[test]
    fn test_segment_priority_ordering() {
        assert!(SegmentPriority::P0 < SegmentPriority::P1);
        assert!(SegmentPriority::P1 < SegmentPriority::P2);
        assert!(SegmentPriority::P2 < SegmentPriority::P3);
        assert!(SegmentPriority::P3 < SegmentPriority::P4);
    }

    #[test]
    fn test_segment_priority_sort() {
        let mut priorities = vec![
            SegmentPriority::P4,
            SegmentPriority::P0,
            SegmentPriority::P2,
            SegmentPriority::P1,
            SegmentPriority::P3,
        ];
        priorities.sort();
        assert_eq!(
            priorities,
            vec![
                SegmentPriority::P0,
                SegmentPriority::P1,
                SegmentPriority::P2,
                SegmentPriority::P3,
                SegmentPriority::P4,
            ]
        );
    }

    // =========================================================================
    // Soft-trim tests
    // =========================================================================

    #[test]
    fn test_soft_trim_short_content_unchanged() {
        let content = "Short content";
        let result = ContextWindow::soft_trim_tool_result(content, 1000, "gpt-4");
        assert_eq!(result, content);
    }

    #[test]
    fn test_soft_trim_preserves_head_and_tail() {
        // Create a long string that will exceed the token limit
        let content: String = (0..500).fold(String::new(), |mut acc, i| {
            use std::fmt::Write;
            let _ = write!(acc, "word{i} ");
            acc
        });
        let result = ContextWindow::soft_trim_tool_result(&content, 50, "gpt-4");

        // Should contain the separator
        assert!(
            result.contains("[..."),
            "Trimmed result should contain ellipsis separator"
        );
        assert!(
            result.contains("tokens omitted"),
            "Trimmed result should indicate omitted tokens"
        );

        // Head should be from the beginning
        assert!(
            result.starts_with("word0"),
            "Trimmed result should start with beginning of content"
        );
    }

    // =========================================================================
    // Hard-clear tests
    // =========================================================================

    #[tokio::test]
    async fn test_hard_clear_removes_old_segments() {
        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut ctx = ContextWindow::new(pool, None);

        let old_ts = (Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
        let recent_ts = Utc::now().to_rfc3339();

        ctx.segments.push(ContextSegment {
            priority: SegmentPriority::P4,
            content: "old tool result".to_string(),
            token_estimate: 10,
            source_type: SegmentSourceType::ToolResult,
            source_id: None,
            metadata: json!({"ts": old_ts}),
            insertion_order: 0,
        });
        ctx.segments.push(ContextSegment {
            priority: SegmentPriority::P4,
            content: "recent tool result".to_string(),
            token_estimate: 10,
            source_type: SegmentSourceType::ToolResult,
            source_id: None,
            metadata: json!({"ts": recent_ts}),
            insertion_order: 1,
        });
        ctx.total_tokens = 20;

        ctx.hard_clear_old_tool_results(3600); // 1 hour threshold

        assert_eq!(ctx.segments.len(), 1);
        assert_eq!(ctx.segments[0].content, "recent tool result");
        assert_eq!(ctx.total_tokens, 10);
    }

    #[tokio::test]
    async fn test_hard_clear_preserves_non_p4_segments() {
        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut ctx = ContextWindow::new(pool, None);

        let old_ts = (Utc::now() - chrono::Duration::hours(2)).to_rfc3339();

        ctx.segments.push(ContextSegment {
            priority: SegmentPriority::P0,
            content: "soul directive".to_string(),
            token_estimate: 10,
            source_type: SegmentSourceType::SoulDirective,
            source_id: None,
            metadata: json!({"ts": old_ts}),
            insertion_order: 0,
        });
        ctx.total_tokens = 10;

        ctx.hard_clear_old_tool_results(3600);

        assert_eq!(ctx.segments.len(), 1, "P0 segments should not be cleared");
    }

    // =========================================================================
    // Budget enforcement tests
    // =========================================================================

    #[tokio::test]
    async fn test_budget_enforcement_no_pruning_needed() {
        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut ctx = ContextWindow::new(pool, None).with_budget(1000);

        ctx.segments.push(ContextSegment {
            priority: SegmentPriority::P0,
            content: "soul".to_string(),
            token_estimate: 10,
            source_type: SegmentSourceType::SoulDirective,
            source_id: None,
            metadata: json!({}),
            insertion_order: 0,
        });
        ctx.total_tokens = 10;

        ctx.enforce_budget(2000, 3600);

        assert_eq!(ctx.segments.len(), 1, "No segments should be pruned");
    }

    #[tokio::test]
    async fn test_budget_enforcement_prunes_p3_before_p1() {
        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut ctx = ContextWindow::new(pool, None).with_budget(100);

        // P0 — never pruned
        ctx.segments.push(ContextSegment {
            priority: SegmentPriority::P0,
            content: "soul".to_string(),
            token_estimate: 30,
            source_type: SegmentSourceType::SoulDirective,
            source_id: None,
            metadata: json!({}),
            insertion_order: 0,
        });

        // P1 — pruned after P3
        ctx.segments.push(ContextSegment {
            priority: SegmentPriority::P1,
            content: "memory".to_string(),
            token_estimate: 30,
            source_type: SegmentSourceType::Memory,
            source_id: Some(Uuid::new_v4()),
            metadata: json!({"importance": 0.5}),
            insertion_order: 1,
        });

        // P3 — pruned first (after P4)
        ctx.segments.push(ContextSegment {
            priority: SegmentPriority::P3,
            content: "message".to_string(),
            token_estimate: 40,
            source_type: SegmentSourceType::ConversationMessage,
            source_id: None,
            metadata: json!({"message_id": 1}),
            insertion_order: 2,
        });

        ctx.total_tokens = 100;

        // Budget = 100, target = budget_tokens = 100. Total = 100 <= 100, no pruning.
        // Use budget=99 so total=100 > 99 triggers pruning.
        ctx.budget_tokens = 99;
        ctx.enforce_budget(2000, 3600);

        assert_eq!(
            ctx.segments.len(),
            2,
            "P3 should be pruned, P0 and P1 remain"
        );
        assert!(
            ctx.segments
                .iter()
                .all(|s| s.priority != SegmentPriority::P3),
            "No P3 segments should remain"
        );
    }

    // =========================================================================
    // Provenance tests
    // =========================================================================

    #[tokio::test]
    async fn test_provenance_computation() {
        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut ctx = ContextWindow::new(pool, None);

        let mem_id = Uuid::new_v4();

        ctx.segments.push(ContextSegment {
            priority: SegmentPriority::P0,
            content: "soul directive".to_string(),
            token_estimate: 5,
            source_type: SegmentSourceType::SoulDirective,
            source_id: None,
            metadata: json!({}),
            insertion_order: 0,
        });
        ctx.segments.push(ContextSegment {
            priority: SegmentPriority::P1,
            content: "memory content".to_string(),
            token_estimate: 10,
            source_type: SegmentSourceType::Memory,
            source_id: Some(mem_id),
            metadata: json!({}),
            insertion_order: 1,
        });
        ctx.segments.push(ContextSegment {
            priority: SegmentPriority::P3,
            content: "user: hello".to_string(),
            token_estimate: 5,
            source_type: SegmentSourceType::ConversationMessage,
            source_id: None,
            metadata: json!({"message_id": 42}),
            insertion_order: 2,
        });
        ctx.total_tokens = 20;

        let prov = ctx.compute_provenance();

        assert_eq!(prov.memory_ids, vec![mem_id]);
        assert_eq!(prov.message_ids, vec![42]);
        assert_eq!(prov.total_tokens, 20);
        assert_eq!(
            prov.context_bundle_hash.len(),
            64,
            "blake3 hex should be 64 chars"
        );
        assert_eq!(*prov.segment_counts.get("soul_directive").unwrap_or(&0), 1);
        assert_eq!(*prov.segment_counts.get("memory").unwrap_or(&0), 1);
        assert_eq!(
            *prov
                .segment_counts
                .get("conversation_message")
                .unwrap_or(&0),
            1
        );
    }

    #[tokio::test]
    async fn test_provenance_hash_deterministic() {
        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut ctx1 = ContextWindow::new(pool.clone(), None);
        let pool2 = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut ctx2 = ContextWindow::new(pool2, None);

        let seg = ContextSegment {
            priority: SegmentPriority::P0,
            content: "same content".to_string(),
            token_estimate: 5,
            source_type: SegmentSourceType::SoulDirective,
            source_id: None,
            metadata: json!({}),
            insertion_order: 0,
        };

        ctx1.segments.push(seg.clone());
        ctx2.segments.push(seg);

        let prov1 = ctx1.compute_provenance();
        let prov2 = ctx2.compute_provenance();

        assert_eq!(
            prov1.context_bundle_hash, prov2.context_bundle_hash,
            "Same content should produce same hash"
        );
    }

    // =========================================================================
    // Assemble reset tests (Comment 1)
    // =========================================================================

    #[tokio::test]
    async fn test_assemble_resets_state_on_repeated_calls() {
        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut ctx = ContextWindow::new(pool, None).with_budget(10_000);

        // Manually push segments to simulate a first "load"
        ctx.segments.push(ContextSegment {
            priority: SegmentPriority::P0,
            content: "soul".to_string(),
            token_estimate: 100,
            source_type: SegmentSourceType::SoulDirective,
            source_id: None,
            metadata: json!({}),
            insertion_order: 0,
        });
        ctx.total_tokens = 100;

        assert_eq!(ctx.segments.len(), 1);
        assert_eq!(ctx.total_tokens, 100);

        // Simulate what assemble() does at the top: clear state
        // (We can't call assemble() directly without a DB, but we can verify
        // the reset logic that runs before any load methods.)
        ctx.segments.clear();
        ctx.total_tokens = 0;

        // Cached builder fields must survive the reset
        assert_eq!(ctx.budget_tokens, 10_000);
        assert!(ctx.session_id.is_none()); // was never set
        assert!(ctx.identity_id.is_none());
        assert_eq!(ctx.segments.len(), 0);
        assert_eq!(ctx.total_tokens, 0);
    }

    // =========================================================================
    // Insertion order tests (Comment 2)
    // =========================================================================

    #[tokio::test]
    async fn test_insertion_order_preserves_chronological_within_priority() {
        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut ctx = ContextWindow::new(pool, None);

        // Push P3 segments in chronological order (msg 10, 20, 30)
        for (i, mid) in [10i64, 20, 30].iter().enumerate() {
            ctx.segments.push(ContextSegment {
                priority: SegmentPriority::P3,
                content: format!("msg-{mid}"),
                token_estimate: 5,
                source_type: SegmentSourceType::ConversationMessage,
                source_id: None,
                metadata: json!({"message_id": mid}),
                insertion_order: i,
            });
        }

        // Push a P0 segment that was inserted later (index 3)
        ctx.segments.push(ContextSegment {
            priority: SegmentPriority::P0,
            content: "soul".to_string(),
            token_estimate: 5,
            source_type: SegmentSourceType::SoulDirective,
            source_id: None,
            metadata: json!({}),
            insertion_order: 3,
        });

        // Sort by (priority, insertion_order)
        ctx.segments
            .sort_by_key(|s| (s.priority, s.insertion_order));

        // P0 should come first, then P3 in original chronological order
        assert_eq!(ctx.segments[0].content, "soul");
        assert_eq!(ctx.segments[1].content, "msg-10");
        assert_eq!(ctx.segments[2].content, "msg-20");
        assert_eq!(ctx.segments[3].content, "msg-30");
    }

    // =========================================================================
    // Integration tests (require database)
    // =========================================================================

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_context_assembly_integration() {
        unimplemented!("Run with: cargo test --test context -- --ignored");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_build_for_session_integration() {
        unimplemented!("Run with: cargo test --test context -- --ignored");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_resolve_context_window_limit_integration() {
        unimplemented!("Run with: cargo test --test context -- --ignored");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_log_to_ledger_integration() {
        unimplemented!("Run with: cargo test --test context_integrity_test -- --ignored");
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_log_context_integrity_integration() {
        unimplemented!("Run with: cargo test --test context_integrity_test -- --ignored");
    }

    #[tokio::test]
    async fn test_add_segment_updates_tokens_and_order() {
        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut ctx = ContextWindow::new(pool, None);

        assert_eq!(ctx.total_tokens, 0);
        assert_eq!(ctx.segments.len(), 0);

        ctx.add_segment(ContextSegment {
            priority: SegmentPriority::P0,
            content: "soul directive".to_string(),
            token_estimate: 10,
            source_type: SegmentSourceType::SoulDirective,
            source_id: None,
            metadata: json!({}),
            insertion_order: 999, // should be overwritten to 0
        });

        assert_eq!(ctx.segments.len(), 1);
        assert_eq!(ctx.total_tokens, 10);
        assert_eq!(ctx.segments[0].insertion_order, 0);

        ctx.add_segment(ContextSegment {
            priority: SegmentPriority::P1,
            content: "memory".to_string(),
            token_estimate: 5,
            source_type: SegmentSourceType::Memory,
            source_id: Some(Uuid::new_v4()),
            metadata: json!({}),
            insertion_order: 999, // should be overwritten to 1
        });

        assert_eq!(ctx.segments.len(), 2);
        assert_eq!(ctx.total_tokens, 15);
        assert_eq!(ctx.segments[1].insertion_order, 1);
    }

    #[tokio::test]
    async fn test_add_segment_preserves_metadata() {
        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut ctx = ContextWindow::new(pool, None);
        let run_id = Uuid::new_v4();

        ctx.add_segment(ContextSegment {
            priority: SegmentPriority::P2,
            content: "task context".to_string(),
            token_estimate: 8,
            source_type: SegmentSourceType::TaskContext,
            source_id: None,
            metadata: json!({"run_id": run_id.to_string()}),
            insertion_order: 0,
        });

        let prov = ctx.compute_provenance();
        assert_eq!(prov.run_ids.len(), 1);
        assert_eq!(prov.run_ids[0], run_id);
    }
}
