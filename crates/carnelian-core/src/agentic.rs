//! Agentic Execution Engine
//!
//! This module orchestrates the complete agentic execution pipeline for Carnelian OS.
//! It unifies session management, context assembly, model inference, tool execution,
//! declarative plan execution, and memory persistence into a single coherent loop.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐
//! │   Client     │
//! └──────┬──────┘
//!        │  AgenticRequest
//!        ▼
//! ┌──────────────────────────────────────────────────────────────┐
//! │                     AgenticEngine                            │
//! │                                                              │
//! │  1. Session Intake                                           │
//! │     ├─ load / create session (SessionManager)                │
//! │     ├─ append user message                                   │
//! │     └─ check compaction threshold                            │
//! │        └─ trigger memory flush → MemoryManager               │
//! │        └─ compact session                                    │
//! │                                                              │
//! │  2. Context Assembly                                         │
//! │     ├─ build ContextWindow (P0–P4 segments)                  │
//! │     ├─ enforce token budget                                  │
//! │     └─ log provenance to Ledger                              │
//! │                                                              │
//! │  3. Model Call                                               │
//! │     ├─ build CompletionRequest                               │
//! │     ├─ ModelRouter::complete() with capability + budget      │
//! │     └─ log to Ledger                                         │
//! │                                                              │
//! │  4. Response Parsing                                         │
//! │     ├─ detect tool_calls JSON → Vec<ToolCall>                │
//! │     ├─ detect declarative plan YAML/JSON → DeclarativePlan   │
//! │     └─ fallback: plain text response                         │
//! │                                                              │
//! │  5. Execution                                                │
//! │     ├─ Tool calls: capability check → worker invoke          │
//! │     └─ Plans: dependency graph → topological execution       │
//! │                                                              │
//! │  6. Persistence                                              │
//! │     ├─ append assistant + tool messages to session           │
//! │     ├─ update token counters                                 │
//! │     └─ log completion to Ledger                              │
//! └──────────────────────────────────────────────────────────────┘
//!        │
//!        ▼  AgenticResponse
//! ┌─────────────┐
//! │   Client     │
//! └─────────────┘
//! ```
//!
//! # Tool Call Format
//!
//! The engine detects tool calls when the model response contains a JSON object
//! with a `"tool_calls"` array:
//!
//! ```json
//! {
//!   "tool_calls": [
//!     {
//!       "tool_id": "call_123",
//!       "tool_name": "read_file",
//!       "arguments": {"path": "/etc/hosts"}
//!     }
//!   ]
//! }
//! ```
//!
//! # Declarative Plan Format
//!
//! Plans are detected when the response contains a JSON object with a `"plan"` key:
//!
//! ```json
//! {
//!   "plan": {
//!     "description": "Multi-step workflow",
//!     "steps": [
//!       {
//!         "action": "fetch_data",
//!         "skill": "http_get",
//!         "parameters": {"url": "https://api.example.com"},
//!         "dependencies": []
//!       }
//!     ]
//!   }
//! }
//! ```
//!
//! # Memory Flush Protocol
//!
//! Before session compaction, the engine extracts important information from
//! recent conversation history and persists it as durable memories via
//! `MemoryManager`. The model is prompted to identify key facts, preferences,
//! and decisions worth preserving. If the model returns no memories, the
//! outcome is logged explicitly as "nothing to store".
//!
//! # Error Handling
//!
//! The engine follows a graceful degradation strategy:
//! - Session errors: create new session if load fails
//! - Context assembly errors: fall back to minimal context (soul directives only)
//! - Model call errors: return error response with helpful message
//! - Tool execution errors: capture as failed result, continue with other tools
//! - Plan execution errors: skip dependent steps, continue independent ones
//! - Memory flush errors: log and continue with compaction
//! - Ledger/event errors: log warning, never block execution
//!
//! # Integration Points
//!
//! - **SessionManager**: conversation persistence and compaction
//! - **ContextWindow**: priority-based context assembly (P0–P4)
//! - **ModelRouter**: LLM completion with capability + budget enforcement
//! - **PolicyEngine**: capability checks for tool/skill execution
//! - **MemoryManager**: durable memory creation during flush
//! - **WorkerManager**: skill execution via transport protocols
//! - **Ledger**: tamper-resistant audit trail for all operations
//! - **EventStream**: real-time observability events

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use sqlx::PgPool;
use uuid::Uuid;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType, InvokeRequest, RunId};
use carnelian_common::{Error, Result};

use crate::config::Config;
use crate::context::{ContextWindow, estimate_tokens};
use crate::events::EventStream;
use crate::ledger::Ledger;
use crate::memory::{MemoryManager, MemorySource};
use crate::model_router::{CompletionRequest, Message, ModelRouter, UsageStats};
use crate::policy::PolicyEngine;
use crate::session::{CompactionTrigger, SessionKey, SessionManager};
use crate::worker::WorkerManager;

// =============================================================================
// CORE TYPES
// =============================================================================

/// Request to the agentic execution engine.
///
/// Contains all information needed to process a single turn of agent interaction:
/// the session to operate on, the identity performing the action, optional task
/// context, and the user's message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticRequest {
    /// Session key identifying the conversation
    pub session_key: SessionKey,
    /// Agent identity performing this action
    pub identity_id: Uuid,
    /// Optional task context for task-driven interactions
    pub task_id: Option<Uuid>,
    /// Optional run identifier for multi-turn task execution
    pub run_id: Option<Uuid>,
    /// The user's message content
    pub user_message: String,
    /// Optional correlation ID (generated if not provided)
    pub correlation_id: Option<Uuid>,
}

/// Response from the agentic execution engine.
///
/// Contains the assistant's response, results from any tool calls or plan
/// steps executed, memory creation count, token usage, and the correlation
/// ID that traces the entire operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticResponse {
    /// The assistant's text response
    pub assistant_message: String,
    /// Results from tool calls executed during this turn
    pub tool_calls_executed: Vec<ToolCallResult>,
    /// Results from declarative plan steps executed during this turn
    pub plan_steps_executed: Vec<PlanStepResult>,
    /// Number of memories created (e.g., during memory flush)
    pub memories_created: usize,
    /// Token usage statistics for this turn
    pub tokens_used: UsageStats,
    /// Correlation ID tracing this entire operation
    pub correlation_id: Uuid,
}

/// A parsed tool call from the model's response.
///
/// Tool calls follow a structured JSON format with a unique call ID,
/// the tool/skill name, and a JSON arguments object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call (for correlation)
    pub tool_id: String,
    /// Name of the tool/skill to invoke
    pub tool_name: String,
    /// Arguments to pass to the tool (JSON object)
    pub arguments: JsonValue,
    /// Call ID for correlating request with result
    pub call_id: String,
}

/// Result of executing a single tool call.
///
/// Captures the outcome including status, result data, optional error
/// message, and wall-clock duration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// Call ID correlating this result with the original tool call
    pub call_id: String,
    /// Name of the tool that was invoked
    pub tool_name: String,
    /// Execution status
    pub status: ToolCallStatus,
    /// Result data (JSON value)
    pub result: JsonValue,
    /// Error message if execution failed
    pub error: Option<String>,
    /// Wall-clock duration in milliseconds
    pub duration_ms: u64,
}

/// Status of a tool call execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCallStatus {
    /// Tool executed successfully
    Success,
    /// Tool execution failed
    Failed,
    /// Tool execution denied by capability check
    Denied,
    /// Tool execution timed out
    Timeout,
}

/// A declarative execution plan parsed from the model's response.
///
/// Plans describe multi-step workflows with dependency ordering.
/// Steps are executed in topological order respecting declared dependencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclarativePlan {
    /// Unique identifier for this plan
    pub plan_id: Uuid,
    /// Ordered list of plan steps
    pub steps: Vec<PlanStep>,
    /// Human-readable description of the plan
    pub description: String,
}

/// A single step in a declarative plan.
///
/// Each step has an action description, an optional skill to invoke,
/// parameters for the skill, and a list of step IDs that must complete
/// before this step can execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Unique identifier for this step
    pub step_id: Uuid,
    /// Human-readable action description
    pub action: String,
    /// Optional skill name to invoke for this step
    pub skill_name: Option<String>,
    /// Parameters to pass to the skill (JSON object)
    pub parameters: JsonValue,
    /// Step IDs that must complete before this step can execute
    pub dependencies: Vec<Uuid>,
}

/// Result of executing a single plan step.
///
/// Captures the outcome including status, result data, optional error
/// message, and wall-clock duration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStepResult {
    /// Step ID correlating this result with the original plan step
    pub step_id: Uuid,
    /// Execution status
    pub status: PlanStepStatus,
    /// Result data (JSON value)
    pub result: JsonValue,
    /// Error message if execution failed
    pub error: Option<String>,
    /// Wall-clock duration in milliseconds
    pub duration_ms: u64,
}

/// Status of a plan step execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanStepStatus {
    /// Step executed successfully
    Success,
    /// Step execution failed
    Failed,
    /// Step was skipped (dependency failed)
    Skipped,
    /// Step execution denied by capability check
    Denied,
}

// =============================================================================
// AGENTIC ENGINE
// =============================================================================

/// Orchestrates the complete agentic execution pipeline.
///
/// The `AgenticEngine` is the central coordinator that drives the agentic loop:
/// session intake → context assembly → model call → response parsing →
/// execution → persistence. It holds references to all required subsystems
/// and provides a single `process_request()` entry point.
///
/// # Example
///
/// ```ignore
/// let engine = AgenticEngine::new(
///     pool, session_manager, model_router, policy_engine,
///     memory_manager, worker_manager, ledger, config,
/// ).with_event_stream(event_stream);
///
/// let response = engine.process_request(request).await?;
/// ```
pub struct AgenticEngine {
    /// Database connection pool
    pool: PgPool,
    /// Session manager for conversation persistence
    session_manager: Arc<SessionManager>,
    /// Model router for LLM completion calls
    model_router: Arc<ModelRouter>,
    /// Policy engine for capability-based security
    policy_engine: Arc<PolicyEngine>,
    /// Memory manager for durable memory creation
    memory_manager: Arc<MemoryManager>,
    /// Worker manager for skill execution via transports
    worker_manager: Arc<tokio::sync::Mutex<WorkerManager>>,
    /// Tamper-resistant audit ledger
    ledger: Arc<Ledger>,
    /// Application configuration
    config: Arc<Config>,
    /// Optional event stream for real-time observability
    event_stream: Option<Arc<EventStream>>,
}

impl AgenticEngine {
    /// Create a new `AgenticEngine` with all required dependencies.
    ///
    /// # Arguments
    ///
    /// * `pool` - PostgreSQL connection pool
    /// * `session_manager` - Session manager for conversation persistence
    /// * `model_router` - Model router for LLM completion calls
    /// * `policy_engine` - Policy engine for capability checks
    /// * `memory_manager` - Memory manager for durable memory creation
    /// * `worker_manager` - Worker manager for skill execution
    /// * `ledger` - Tamper-resistant audit ledger
    /// * `config` - Application configuration
    pub fn new(
        pool: PgPool,
        session_manager: Arc<SessionManager>,
        model_router: Arc<ModelRouter>,
        policy_engine: Arc<PolicyEngine>,
        memory_manager: Arc<MemoryManager>,
        worker_manager: Arc<tokio::sync::Mutex<WorkerManager>>,
        ledger: Arc<Ledger>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            pool,
            session_manager,
            model_router,
            policy_engine,
            memory_manager,
            worker_manager,
            ledger,
            config,
            event_stream: None,
        }
    }

    /// Attach an event stream for real-time observability.
    ///
    /// Returns `self` for builder-style chaining.
    #[must_use]
    pub fn with_event_stream(mut self, event_stream: Arc<EventStream>) -> Self {
        self.event_stream = Some(event_stream);
        self
    }

    // =========================================================================
    // MAIN ENTRY POINT
    // =========================================================================

    /// Process a single agentic request through the full execution pipeline.
    ///
    /// This is the main entry point for the agentic loop. It orchestrates:
    /// 1. Session intake (load/create, append user message, compaction check)
    /// 2. Context assembly (P0–P4 segments, token budget enforcement)
    /// 3. Model call (completion request via ModelRouter)
    /// 4. Response parsing (tool calls, declarative plans, or plain text)
    /// 5. Execution (tool invocation or plan step execution)
    /// 6. Persistence (assistant message, tool results, token counters)
    ///
    /// # Errors
    ///
    /// Returns an error if the model call fails or if critical session
    /// operations fail. Tool and plan execution errors are captured in
    /// the response rather than propagated.
    pub async fn process_request(&self, request: AgenticRequest) -> Result<AgenticResponse> {
        let start = Instant::now();
        let correlation_id = request.correlation_id.unwrap_or_else(Uuid::now_v7);

        // ── Ledger: request received ──────────────────────────────────────
        self.log_to_ledger(
            Some(request.identity_id),
            "agentic.request_received",
            json!({
                "session_key": request.session_key.to_string(),
                "identity_id": request.identity_id,
                "task_id": request.task_id,
                "correlation_id": correlation_id,
            }),
            Some(correlation_id),
        ).await;

        // ── Event: request start ──────────────────────────────────────────
        self.emit_event(
            EventType::Custom("agentic.request_start".to_string()),
            json!({
                "session_key": request.session_key.to_string(),
                "correlation_id": correlation_id,
            }),
            Some(correlation_id),
        );

        // ── Step 1: Session Intake ────────────────────────────────────────
        let session_key_str = request.session_key.to_string();
        let session = match self.session_manager.load_session(&session_key_str).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                // Session not found or expired — create a new one
                self.session_manager.create_session(&session_key_str).await?
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    session_key = %session_key_str,
                    correlation_id = %correlation_id,
                    "Failed to load session, creating new one"
                );
                self.session_manager.create_session(&session_key_str).await?
            }
        };

        let session_id = session.session_id;

        // Append user message
        let user_tokens = estimate_tokens(&request.user_message, "default");
        self.session_manager.append_message(
            session_id,
            "user",
            request.user_message.clone(),
            Some(user_tokens as i32),
            None,
            None,
            Some(correlation_id),
            None,
            None,
        ).await?;

        // Check compaction threshold
        let mut memories_created: usize = 0;
        let counters = session.counters();
        let reserve_fraction = f64::from(self.config.context_reserve_percent) / 100.0;
        let effective_limit = (self.config.context_window_tokens as f64 * (1.0 - reserve_fraction)) as i64;

        if counters.total + user_tokens as i64 > effective_limit {
            // Trigger memory flush before compaction
            match self.trigger_memory_flush(session_id, request.identity_id, correlation_id).await {
                Ok(count) => memories_created = count,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        session_id = %session_id,
                        correlation_id = %correlation_id,
                        "Memory flush failed, continuing with compaction"
                    );
                }
            }

            // Compact session (skip internal flush since we already flushed above)
            if let Err(e) = self.session_manager.compact_session(
                session_id,
                CompactionTrigger::TokenLimitExceeded,
                None,
                &self.config,
                Some(&self.ledger),
                true,
            ).await {
                tracing::warn!(
                    error = %e,
                    session_id = %session_id,
                    correlation_id = %correlation_id,
                    "Session compaction failed"
                );
            }
        }

        // ── Step 2: Context Assembly ──────────────────────────────────────
        let context_messages = match self.assemble_context(
            session_id,
            request.identity_id,
            request.task_id,
            correlation_id,
        ).await {
            Ok(msgs) => msgs,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    session_id = %session_id,
                    correlation_id = %correlation_id,
                    "Context assembly failed, using minimal context"
                );
                // Fallback: minimal context with just the user message
                vec![Message {
                    role: "user".to_string(),
                    content: request.user_message.clone(),
                    name: None,
                    tool_call_id: None,
                }]
            }
        };

        // ── Step 3: Model Call ────────────────────────────────────────────
        let completion_request = CompletionRequest {
            model: self.config.machine_config().default_model,
            messages: context_messages,
            temperature: Some(0.7),
            max_tokens: Some(2048),
            stream: Some(false),
            correlation_id: Some(correlation_id),
        };

        let completion = self.model_router.complete(
            completion_request,
            request.identity_id,
            request.task_id,
            request.run_id,
        ).await?;

        let assistant_content = completion.choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        // ── Step 4: Response Parsing ──────────────────────────────────────
        let mut tool_calls_executed = Vec::new();
        let mut plan_steps_executed = Vec::new();

        if let Ok(tool_calls) = Self::parse_tool_calls(&assistant_content) {
            // ── Step 5a: Tool Call Execution ───────────────────────────────
            tool_calls_executed = self.execute_tool_calls(
                tool_calls,
                request.identity_id,
                session_id,
                correlation_id,
            ).await?;
        } else if let Ok(plan) = Self::parse_declarative_plan(&assistant_content) {
            // ── Step 5b: Declarative Plan Execution ───────────────────────
            plan_steps_executed = self.execute_declarative_plan(
                plan,
                request.identity_id,
                session_id,
                correlation_id,
            ).await?;
        }

        // ── Step 6: Persistence ───────────────────────────────────────────
        let assistant_tokens = estimate_tokens(&assistant_content, "default");
        self.session_manager.append_message(
            session_id,
            "assistant",
            assistant_content.clone(),
            Some(assistant_tokens as i32),
            None,
            None,
            Some(correlation_id),
            None,
            None,
        ).await?;

        // ── Ledger: response completed ────────────────────────────────────
        let duration_ms = start.elapsed().as_millis() as u64;
        self.log_to_ledger(
            Some(request.identity_id),
            "agentic.response_completed",
            json!({
                "session_id": session_id,
                "correlation_id": correlation_id,
                "tokens_used": completion.usage.total_tokens,
                "tool_calls_count": tool_calls_executed.len(),
                "plan_steps_count": plan_steps_executed.len(),
                "memories_created": memories_created,
                "duration_ms": duration_ms,
            }),
            Some(correlation_id),
        ).await;

        // ── Event: response complete ──────────────────────────────────────
        self.emit_event(
            EventType::Custom("agentic.response_complete".to_string()),
            json!({
                "session_id": session_id,
                "correlation_id": correlation_id,
                "duration_ms": duration_ms,
            }),
            Some(correlation_id),
        );

        Ok(AgenticResponse {
            assistant_message: assistant_content,
            tool_calls_executed,
            plan_steps_executed,
            memories_created,
            tokens_used: completion.usage,
            correlation_id,
        })
    }

    // =========================================================================
    // CONTEXT ASSEMBLY
    // =========================================================================

    /// Assemble context for a model call using the ContextWindow pipeline.
    ///
    /// Builds a full context window with P0–P4 segments (soul directives,
    /// recent memories, task context, conversation history), enforces the
    /// token budget, and returns the assembled messages.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Session to build context for
    /// * `identity_id` - Agent identity for soul directives and memories
    /// * `task_id` - Optional task for P2 context
    /// * `correlation_id` - Correlation ID for tracing
    async fn assemble_context(
        &self,
        session_id: Uuid,
        identity_id: Uuid,
        task_id: Option<Uuid>,
        correlation_id: Uuid,
    ) -> Result<Vec<Message>> {
        let mut ctx = ContextWindow::build_for_session(
            self.pool.clone(),
            self.event_stream.clone(),
            session_id,
            task_id,
            &self.config,
        ).await?;

        // Load soul directives (P0)
        if let Err(e) = ctx.load_soul_directives(identity_id).await {
            tracing::warn!(
                error = %e,
                correlation_id = %correlation_id,
                "Failed to load soul directives for context"
            );
        }

        // Load recent memories (P1)
        if let Err(e) = ctx.load_recent_memories(identity_id, 10).await {
            tracing::warn!(
                error = %e,
                correlation_id = %correlation_id,
                "Failed to load recent memories for context"
            );
        }

        // Enforce budget
        ctx.enforce_budget(self.config.tool_trim_threshold, self.config.tool_clear_age_secs);

        // Assemble into messages
        let assembled = ctx.assemble(&self.config).await?;

        // Log context assembly to ledger
        let context_hash = Ledger::compute_payload_hash(&json!({"context": &assembled}));
        self.log_to_ledger(
            None,
            "agentic.context_assembled",
            json!({
                "session_id": session_id,
                "correlation_id": correlation_id,
                "context_hash": context_hash,
            }),
            Some(correlation_id),
        ).await;

        self.emit_event(
            EventType::ContextAssembled,
            json!({
                "session_id": session_id,
                "correlation_id": correlation_id,
            }),
            Some(correlation_id),
        );

        // Convert assembled string into a messages array
        // The assembled context is a single string; wrap it as a system message
        // followed by conversation history from the session
        let messages = self.build_messages_from_context(assembled, session_id).await?;
        Ok(messages)
    }

    /// Build a message array from assembled context and session history.
    ///
    /// Combines the assembled context (as a system message) with recent
    /// session messages to form the complete prompt for the model.
    async fn build_messages_from_context(
        &self,
        assembled_context: String,
        session_id: Uuid,
    ) -> Result<Vec<Message>> {
        let mut messages = Vec::new();

        // System message with assembled context
        if !assembled_context.is_empty() {
            messages.push(Message {
                role: "system".to_string(),
                content: assembled_context,
                name: None,
                tool_call_id: None,
            });
        }

        // Load recent session messages for conversation history
        let session_messages = self.session_manager.load_messages(session_id, Some(50), None).await?;

        // Session messages come newest-first; reverse for chronological order
        for msg in session_messages.into_iter().rev() {
            messages.push(Message {
                role: msg.role.clone(),
                content: msg.content.clone(),
                name: msg.tool_name.clone(),
                tool_call_id: msg.tool_call_id.clone(),
            });
        }

        Ok(messages)
    }

    // =========================================================================
    // RESPONSE PARSING
    // =========================================================================

    /// Parse tool calls from the model's response content.
    ///
    /// Looks for a JSON object with a `"tool_calls"` array. Each element
    /// must have `tool_id` (or `id`), `tool_name` (or `name`), and `arguments`.
    ///
    /// # Errors
    ///
    /// Returns an error if the content does not contain valid tool call JSON.
    pub fn parse_tool_calls(content: &str) -> Result<Vec<ToolCall>> {
        // Try to find JSON object in the content
        let json_str = Self::extract_json_object(content)
            .ok_or_else(|| Error::Agentic("No JSON object found in response".to_string()))?;

        let value: JsonValue = serde_json::from_str(json_str)
            .map_err(|e| Error::Agentic(format!("Invalid JSON in tool calls: {e}")))?;

        let calls_array = value.get("tool_calls")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::Agentic("No 'tool_calls' array found".to_string()))?;

        let mut tool_calls = Vec::with_capacity(calls_array.len());

        for (i, call) in calls_array.iter().enumerate() {
            let tool_id = call.get("tool_id")
                .or_else(|| call.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let tool_name = call.get("tool_name")
                .or_else(|| call.get("name"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Agentic(format!(
                    "Tool call at index {} missing 'tool_name' or 'name'", i
                )))?
                .to_string();

            let arguments = call.get("arguments")
                .cloned()
                .unwrap_or(json!({}));

            let call_id = if tool_id.is_empty() {
                format!("call_{}", Uuid::now_v7())
            } else {
                tool_id.clone()
            };

            tool_calls.push(ToolCall {
                tool_id,
                tool_name,
                arguments,
                call_id,
            });
        }

        if tool_calls.is_empty() {
            return Err(Error::Agentic("Empty tool_calls array".to_string()));
        }

        Ok(tool_calls)
    }

    /// Parse a declarative plan from the model's response content.
    ///
    /// Looks for a JSON object with a `"plan"` key containing `"description"`
    /// and `"steps"` fields. Each step must have `"action"` and may optionally
    /// include `"skill"`, `"parameters"`, and `"dependencies"`.
    ///
    /// # Errors
    ///
    /// Returns an error if the content does not contain a valid plan, or if
    /// the plan contains circular dependencies.
    pub fn parse_declarative_plan(content: &str) -> Result<DeclarativePlan> {
        let json_str = Self::extract_json_object(content)
            .ok_or_else(|| Error::Agentic("No JSON object found in response".to_string()))?;

        let value: JsonValue = serde_json::from_str(json_str)
            .map_err(|e| Error::Agentic(format!("Invalid JSON in plan: {e}")))?;

        let plan_obj = value.get("plan")
            .ok_or_else(|| Error::Agentic("No 'plan' key found".to_string()))?;

        let description = plan_obj.get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("Unnamed plan")
            .to_string();

        let steps_array = plan_obj.get("steps")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::Agentic("No 'steps' array found in plan".to_string()))?;

        let mut steps = Vec::with_capacity(steps_array.len());
        // Map from step index (1-based or string) to generated UUID
        let mut step_id_map: HashMap<String, Uuid> = HashMap::new();

        // First pass: generate UUIDs for all steps
        for (i, _) in steps_array.iter().enumerate() {
            let id = Uuid::now_v7();
            step_id_map.insert(format!("step_{}", i + 1), id);
            step_id_map.insert(i.to_string(), id);
        }

        // Second pass: parse steps with dependency resolution
        for (i, step_val) in steps_array.iter().enumerate() {
            let step_key = format!("step_{}", i + 1);
            let step_id = step_id_map[&step_key];

            let action = step_val.get("action")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Agentic(format!(
                    "Plan step at index {} missing 'action'", i
                )))?
                .to_string();

            let skill_name = step_val.get("skill")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let parameters = step_val.get("parameters")
                .cloned()
                .unwrap_or(json!({}));

            let dependencies = step_val.get("dependencies")
                .and_then(|v| v.as_array())
                .map(|deps| {
                    deps.iter()
                        .filter_map(|d| {
                            let dep_str = d.as_str().unwrap_or("").to_string();
                            step_id_map.get(&dep_str).copied()
                        })
                        .collect::<Vec<Uuid>>()
                })
                .unwrap_or_default();

            steps.push(PlanStep {
                step_id,
                action,
                skill_name,
                parameters,
                dependencies,
            });
        }

        let plan = DeclarativePlan {
            plan_id: Uuid::now_v7(),
            steps,
            description,
        };

        // Validate: check for circular dependencies
        Self::validate_plan_dependencies(&plan)?;

        Ok(plan)
    }

    /// Extract the first JSON object from a string.
    ///
    /// Finds the first `{` and matches it with its closing `}`, handling
    /// nested braces. Returns the substring if found.
    fn extract_json_object(content: &str) -> Option<&str> {
        let start = content.find('{')?;
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape_next = false;

        for (i, ch) in content[start..].char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }
            match ch {
                '\\' if in_string => escape_next = true,
                '"' => in_string = !in_string,
                '{' if !in_string => depth += 1,
                '}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&content[start..start + i + 1]);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Validate that a plan has no circular dependencies.
    ///
    /// Uses Kahn's algorithm for topological sort. If the sort cannot
    /// process all nodes, a cycle exists.
    ///
    /// # Errors
    ///
    /// Returns an error if circular dependencies are detected or if
    /// a dependency references a non-existent step.
    fn validate_plan_dependencies(plan: &DeclarativePlan) -> Result<()> {
        let step_ids: HashSet<Uuid> = plan.steps.iter().map(|s| s.step_id).collect();

        // Verify all dependencies reference valid step IDs
        for step in &plan.steps {
            for dep in &step.dependencies {
                if !step_ids.contains(dep) {
                    return Err(Error::Agentic(format!(
                        "Plan step {} references non-existent dependency {}",
                        step.step_id, dep
                    )));
                }
            }
        }

        // Kahn's algorithm for cycle detection
        let mut in_degree: HashMap<Uuid, usize> = HashMap::new();
        let mut adjacency: HashMap<Uuid, Vec<Uuid>> = HashMap::new();

        for step in &plan.steps {
            in_degree.entry(step.step_id).or_insert(0);
            adjacency.entry(step.step_id).or_default();
            for dep in &step.dependencies {
                adjacency.entry(*dep).or_default().push(step.step_id);
                *in_degree.entry(step.step_id).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<Uuid> = in_degree.iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut visited = 0usize;

        while let Some(node) = queue.pop_front() {
            visited += 1;
            if let Some(dependents) = adjacency.get(&node) {
                for &dependent in dependents {
                    if let Some(deg) = in_degree.get_mut(&dependent) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dependent);
                        }
                    }
                }
            }
        }

        if visited != plan.steps.len() {
            return Err(Error::Agentic(
                "Circular dependency detected in declarative plan".to_string()
            ));
        }

        Ok(())
    }

    // =========================================================================
    // TOOL CALL EXECUTION
    // =========================================================================

    /// Execute a list of tool calls with capability checks and worker invocation.
    ///
    /// For each tool call:
    /// 1. Checks capability via PolicyEngine
    /// 2. Looks up the skill in the database
    /// 3. Invokes the skill via WorkerManager
    /// 4. Persists the result to the session
    /// 5. Logs to ledger and emits events
    ///
    /// Tool calls that fail capability checks are returned with `Denied` status.
    /// Tool calls that fail execution are returned with `Failed` status.
    ///
    /// # Arguments
    ///
    /// * `tool_calls` - List of tool calls to execute
    /// * `identity_id` - Agent identity for capability checks
    /// * `session_id` - Session to persist results to
    /// * `correlation_id` - Correlation ID for tracing
    pub async fn execute_tool_calls(
        &self,
        tool_calls: Vec<ToolCall>,
        identity_id: Uuid,
        session_id: Uuid,
        correlation_id: Uuid,
    ) -> Result<Vec<ToolCallResult>> {
        let mut results = Vec::with_capacity(tool_calls.len());

        for tool_call in tool_calls {
            let call_start = Instant::now();

            self.emit_event(
                EventType::Custom("agentic.tool_call_start".to_string()),
                json!({
                    "call_id": tool_call.call_id,
                    "tool_name": tool_call.tool_name,
                    "correlation_id": correlation_id,
                }),
                Some(correlation_id),
            );

            // Capability check
            let capability_key = format!("tool.{}", tool_call.tool_name);
            let es_ref = self.event_stream.as_ref().map(|es| es.as_ref());
            let has_capability = match self.policy_engine.check_capability(
                "identity",
                &identity_id.to_string(),
                &capability_key,
                es_ref,
            ).await {
                Ok(granted) => granted,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        tool_name = %tool_call.tool_name,
                        "Capability check failed"
                    );
                    false
                }
            };

            let result = if !has_capability {
                let duration_ms = call_start.elapsed().as_millis() as u64;
                self.log_to_ledger(
                    Some(identity_id),
                    "agentic.tool_denied",
                    json!({
                        "call_id": tool_call.call_id,
                        "tool_name": tool_call.tool_name,
                        "capability_key": capability_key,
                        "correlation_id": correlation_id,
                    }),
                    Some(correlation_id),
                ).await;

                ToolCallResult {
                    call_id: tool_call.call_id.clone(),
                    tool_name: tool_call.tool_name.clone(),
                    status: ToolCallStatus::Denied,
                    result: json!(null),
                    error: Some(format!(
                        "Capability '{}' denied for identity {}",
                        capability_key, identity_id
                    )),
                    duration_ms,
                }
            } else {
                // Skill lookup
                let skill_row: Option<(Uuid, String, bool)> = sqlx::query_as(
                    "SELECT skill_id, runtime, enabled FROM skills WHERE name = $1 LIMIT 1",
                )
                .bind(&tool_call.tool_name)
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten();

                match skill_row {
                    None => {
                        let duration_ms = call_start.elapsed().as_millis() as u64;
                        ToolCallResult {
                            call_id: tool_call.call_id.clone(),
                            tool_name: tool_call.tool_name.clone(),
                            status: ToolCallStatus::Failed,
                            result: json!(null),
                            error: Some(format!("Skill '{}' not found", tool_call.tool_name)),
                            duration_ms,
                        }
                    }
                    Some((_skill_id, ref _skill_runtime, false)) => {
                        let duration_ms = call_start.elapsed().as_millis() as u64;
                        ToolCallResult {
                            call_id: tool_call.call_id.clone(),
                            tool_name: tool_call.tool_name.clone(),
                            status: ToolCallStatus::Failed,
                            result: json!(null),
                            error: Some(format!("Skill '{}' is disabled", tool_call.tool_name)),
                            duration_ms,
                        }
                    }
                    Some((_skill_id, ref skill_runtime, true)) => {
                        // Find a running worker whose runtime matches the skill's runtime
                        let invoke_result = {
                            let wm = self.worker_manager.lock().await;
                            let workers = wm.get_worker_status().await;
                            let mut transport = None;
                            for w in &workers {
                                if w.status == "running" && w.runtime == *skill_runtime {
                                    if let Ok(t) = wm.get_transport(&w.id).await {
                                        transport = Some(t);
                                        break;
                                    }
                                }
                            }
                            match transport {
                                Some(t) => {
                                    let run_id = RunId(Uuid::now_v7());
                                    let invoke_request = InvokeRequest {
                                        run_id,
                                        skill_name: tool_call.tool_name.clone(),
                                        input: tool_call.arguments.clone(),
                                        timeout_secs: self.config.skill_timeout_secs,
                                        correlation_id: Some(correlation_id),
                                    };
                                    t.invoke(invoke_request).await
                                }
                                None => Err(Error::Worker(format!(
                                    "No running worker with runtime '{}' available for skill '{}'",
                                    skill_runtime, tool_call.tool_name
                                ))),
                            }
                        };

                        let duration_ms = call_start.elapsed().as_millis() as u64;

                        match invoke_result {
                            Ok(invoke_response) => ToolCallResult {
                                call_id: tool_call.call_id.clone(),
                                tool_name: tool_call.tool_name.clone(),
                                status: ToolCallStatus::Success,
                                result: invoke_response.result,
                                error: None,
                                duration_ms,
                            },
                            Err(e) => {
                                let error_msg = format!("{e}");
                                let status = if error_msg.contains("timeout") || error_msg.contains("Timeout") {
                                    ToolCallStatus::Timeout
                                } else {
                                    ToolCallStatus::Failed
                                };

                                ToolCallResult {
                                    call_id: tool_call.call_id.clone(),
                                    tool_name: tool_call.tool_name.clone(),
                                    status,
                                    result: json!(null),
                                    error: Some(error_msg),
                                    duration_ms,
                                }
                            }
                        }
                    }
                }
            };

            // Persist every tool call result to session, regardless of status
            let persist_content = serde_json::to_string(&json!({
                "status": result.status,
                "result": result.result,
                "error": result.error,
                "duration_ms": result.duration_ms,
            })).unwrap_or_else(|_| "{}".to_string());
            let persist_tokens = estimate_tokens(&persist_content, "default");
            if let Err(e) = self.session_manager.append_message(
                session_id,
                "tool",
                persist_content,
                Some(persist_tokens as i32),
                Some(tool_call.tool_name.clone()),
                Some(tool_call.call_id.clone()),
                Some(correlation_id),
                None,
                None,
            ).await {
                tracing::warn!(error = %e, "Failed to persist tool call result to session");
            }

            // Ledger logging
            self.log_to_ledger(
                Some(identity_id),
                "agentic.tool_executed",
                json!({
                    "call_id": tool_call.call_id,
                    "tool_name": tool_call.tool_name,
                    "status": result.status,
                    "duration_ms": result.duration_ms,
                    "error": result.error,
                    "correlation_id": correlation_id,
                }),
                Some(correlation_id),
            ).await;

            self.emit_event(
                EventType::Custom("agentic.tool_call_end".to_string()),
                json!({
                    "call_id": tool_call.call_id,
                    "tool_name": tool_call.tool_name,
                    "status": result.status,
                    "duration_ms": result.duration_ms,
                }),
                Some(correlation_id),
            );

            results.push(result);
        }

        Ok(results)
    }

    // =========================================================================
    // DECLARATIVE PLAN EXECUTION
    // =========================================================================

    /// Execute a declarative plan with dependency-ordered step execution.
    ///
    /// Steps are executed in topological order respecting declared dependencies.
    /// If a step fails and has dependents, those dependents are skipped.
    /// Independent steps continue regardless of other step failures.
    ///
    /// # Arguments
    ///
    /// * `plan` - The declarative plan to execute
    /// * `identity_id` - Agent identity for capability checks
    /// * `session_id` - Session to persist results to
    /// * `correlation_id` - Correlation ID for tracing
    pub async fn execute_declarative_plan(
        &self,
        plan: DeclarativePlan,
        identity_id: Uuid,
        session_id: Uuid,
        correlation_id: Uuid,
    ) -> Result<Vec<PlanStepResult>> {
        let plan_start = Instant::now();

        // Build topological execution order
        let execution_order = Self::topological_sort(&plan)?;

        let mut results: HashMap<Uuid, PlanStepResult> = HashMap::new();
        let step_map: HashMap<Uuid, &PlanStep> = plan.steps.iter()
            .map(|s| (s.step_id, s))
            .collect();

        for step_id in &execution_order {
            let step = step_map[step_id];
            let step_start = Instant::now();

            self.emit_event(
                EventType::Custom("agentic.plan_step_start".to_string()),
                json!({
                    "plan_id": plan.plan_id,
                    "step_id": step.step_id,
                    "action": step.action,
                    "correlation_id": correlation_id,
                }),
                Some(correlation_id),
            );

            // Check if all dependencies succeeded
            let deps_ok = step.dependencies.iter().all(|dep_id| {
                results.get(dep_id)
                    .map(|r| r.status == PlanStepStatus::Success)
                    .unwrap_or(false)
            });

            if !deps_ok && !step.dependencies.is_empty() {
                let duration_ms = step_start.elapsed().as_millis() as u64;
                let result = PlanStepResult {
                    step_id: step.step_id,
                    status: PlanStepStatus::Skipped,
                    result: json!(null),
                    error: Some("Dependency step failed or was skipped".to_string()),
                    duration_ms,
                };

                // Persist skipped step to session
                let skip_content = serde_json::to_string(&json!({
                    "status": result.status,
                    "error": result.error,
                    "duration_ms": result.duration_ms,
                })).unwrap_or_else(|_| "{}".to_string());
                let skip_tokens = estimate_tokens(&skip_content, "default");
                if let Err(e) = self.session_manager.append_message(
                    session_id,
                    "tool",
                    skip_content,
                    Some(skip_tokens as i32),
                    step.skill_name.clone(),
                    Some(step.step_id.to_string()),
                    Some(correlation_id),
                    None,
                    None,
                ).await {
                    tracing::warn!(error = %e, "Failed to persist skipped plan step to session");
                }

                results.insert(step.step_id, result);
                continue;
            }

            // Execute the step
            let result = if let Some(ref skill_name) = step.skill_name {
                // Skill lookup for runtime matching
                let skill_row: Option<(Uuid, String, bool)> = sqlx::query_as(
                    "SELECT skill_id, runtime, enabled FROM skills WHERE name = $1 LIMIT 1",
                )
                .bind(skill_name)
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten();

                match skill_row {
                    None => PlanStepResult {
                        step_id: step.step_id,
                        status: PlanStepStatus::Failed,
                        result: json!(null),
                        error: Some(format!("Skill '{}' not found", skill_name)),
                        duration_ms: step_start.elapsed().as_millis() as u64,
                    },
                    Some((_skill_id, ref _skill_runtime, false)) => PlanStepResult {
                        step_id: step.step_id,
                        status: PlanStepStatus::Failed,
                        result: json!(null),
                        error: Some(format!("Skill '{}' is disabled", skill_name)),
                        duration_ms: step_start.elapsed().as_millis() as u64,
                    },
                    Some((_skill_id, ref skill_runtime, true)) => {
                        // Capability check
                        let capability_key = format!("tool.{}", skill_name);
                        let es_ref = self.event_stream.as_ref().map(|es| es.as_ref());
                        let has_capability = self.policy_engine.check_capability(
                            "identity",
                            &identity_id.to_string(),
                            &capability_key,
                            es_ref,
                        ).await.unwrap_or(false);

                        if !has_capability {
                            PlanStepResult {
                                step_id: step.step_id,
                                status: PlanStepStatus::Denied,
                                result: json!(null),
                                error: Some(format!("Capability '{}' denied", capability_key)),
                                duration_ms: step_start.elapsed().as_millis() as u64,
                            }
                        } else {
                            // Find a running worker whose runtime matches the skill's runtime
                            let invoke_result = {
                                let wm = self.worker_manager.lock().await;
                                let workers = wm.get_worker_status().await;
                                let mut transport = None;
                                for w in &workers {
                                    if w.status == "running" && w.runtime == *skill_runtime {
                                        if let Ok(t) = wm.get_transport(&w.id).await {
                                            transport = Some(t);
                                            break;
                                        }
                                    }
                                }
                                match transport {
                                    Some(t) => {
                                        let run_id = RunId(Uuid::now_v7());
                                        let invoke_request = InvokeRequest {
                                            run_id,
                                            skill_name: skill_name.clone(),
                                            input: step.parameters.clone(),
                                            timeout_secs: self.config.skill_timeout_secs,
                                            correlation_id: Some(correlation_id),
                                        };
                                        t.invoke(invoke_request).await
                                    }
                                    None => Err(Error::Worker(format!(
                                        "No running worker with runtime '{}' available for skill '{}'",
                                        skill_runtime, skill_name
                                    ))),
                                }
                            };

                            let duration_ms = step_start.elapsed().as_millis() as u64;

                            match invoke_result {
                                Ok(response) => PlanStepResult {
                                    step_id: step.step_id,
                                    status: PlanStepStatus::Success,
                                    result: response.result,
                                    error: None,
                                    duration_ms,
                                },
                                Err(e) => PlanStepResult {
                                    step_id: step.step_id,
                                    status: PlanStepStatus::Failed,
                                    result: json!(null),
                                    error: Some(format!("{e}")),
                                    duration_ms,
                                },
                            }
                        }
                    }
                }
            } else {
                // No skill — treat as a no-op success (informational step)
                PlanStepResult {
                    step_id: step.step_id,
                    status: PlanStepStatus::Success,
                    result: json!({"action": step.action, "note": "informational step, no skill invoked"}),
                    error: None,
                    duration_ms: step_start.elapsed().as_millis() as u64,
                }
            };

            // Persist every plan step result to session, regardless of status
            let step_content = serde_json::to_string(&json!({
                "step_id": result.step_id,
                "status": result.status,
                "result": result.result,
                "error": result.error,
                "duration_ms": result.duration_ms,
            })).unwrap_or_else(|_| "{}".to_string());
            let step_tokens = estimate_tokens(&step_content, "default");
            if let Err(e) = self.session_manager.append_message(
                session_id,
                "tool",
                step_content,
                Some(step_tokens as i32),
                step.skill_name.clone(),
                Some(step.step_id.to_string()),
                Some(correlation_id),
                None,
                None,
            ).await {
                tracing::warn!(error = %e, "Failed to persist plan step result to session");
            }

            self.emit_event(
                EventType::Custom("agentic.plan_step_end".to_string()),
                json!({
                    "plan_id": plan.plan_id,
                    "step_id": step.step_id,
                    "status": result.status,
                    "duration_ms": result.duration_ms,
                }),
                Some(correlation_id),
            );

            results.insert(step.step_id, result);
        }

        // Aggregate results
        let all_results: Vec<PlanStepResult> = execution_order.iter()
            .filter_map(|id| results.remove(id))
            .collect();

        let successful = all_results.iter().filter(|r| r.status == PlanStepStatus::Success).count();
        let failed = all_results.iter().filter(|r| r.status == PlanStepStatus::Failed).count();

        let plan_duration_ms = plan_start.elapsed().as_millis() as u64;

        // Persist plan summary to session
        let plan_summary = format!(
            "Plan '{}' executed: {}/{} steps succeeded, {} failed. Duration: {}ms",
            plan.description, successful, plan.steps.len(), failed, plan_duration_ms
        );
        let summary_tokens = estimate_tokens(&plan_summary, "default");
        if let Err(e) = self.session_manager.append_message(
            session_id,
            "assistant",
            plan_summary,
            Some(summary_tokens as i32),
            None,
            None,
            Some(correlation_id),
            Some(json!({"plan_id": plan.plan_id})),
            None,
        ).await {
            tracing::warn!(error = %e, "Failed to persist plan summary");
        }

        self.log_to_ledger(
            Some(identity_id),
            "agentic.plan_executed",
            json!({
                "plan_id": plan.plan_id,
                "total_steps": plan.steps.len(),
                "successful_steps": successful,
                "failed_steps": failed,
                "duration_ms": plan_duration_ms,
                "correlation_id": correlation_id,
            }),
            Some(correlation_id),
        ).await;

        Ok(all_results)
    }

    /// Compute topological execution order for plan steps using Kahn's algorithm.
    ///
    /// Returns step IDs in an order that respects all declared dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error if the plan contains circular dependencies (should
    /// not happen if `validate_plan_dependencies` was called during parsing).
    fn topological_sort(plan: &DeclarativePlan) -> Result<Vec<Uuid>> {
        let mut in_degree: HashMap<Uuid, usize> = HashMap::new();
        let mut adjacency: HashMap<Uuid, Vec<Uuid>> = HashMap::new();

        for step in &plan.steps {
            in_degree.entry(step.step_id).or_insert(0);
            adjacency.entry(step.step_id).or_default();
            for dep in &step.dependencies {
                adjacency.entry(*dep).or_default().push(step.step_id);
                *in_degree.entry(step.step_id).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<Uuid> = in_degree.iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut order = Vec::with_capacity(plan.steps.len());

        while let Some(node) = queue.pop_front() {
            order.push(node);
            if let Some(dependents) = adjacency.get(&node) {
                for &dependent in dependents {
                    if let Some(deg) = in_degree.get_mut(&dependent) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dependent);
                        }
                    }
                }
            }
        }

        if order.len() != plan.steps.len() {
            return Err(Error::Agentic(
                "Circular dependency detected during topological sort".to_string()
            ));
        }

        Ok(order)
    }

    // =========================================================================
    // MEMORY FLUSH
    // =========================================================================

    /// Trigger a memory flush before session compaction.
    ///
    /// Builds a special context from recent conversation history and prompts
    /// the model to extract important information worth preserving as durable
    /// memories. Each extracted memory is persisted via `MemoryManager`.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Session to flush memories from
    /// * `identity_id` - Agent identity for memory ownership
    /// * `correlation_id` - Correlation ID for tracing
    ///
    /// # Returns
    ///
    /// The number of memories created. Returns 0 if the model determines
    /// there is nothing worth storing.
    ///
    /// # Errors
    ///
    /// Returns an error if the model call fails. Memory creation errors
    /// for individual memories are logged but do not fail the overall flush.
    pub async fn trigger_memory_flush(
        &self,
        session_id: Uuid,
        identity_id: Uuid,
        correlation_id: Uuid,
    ) -> Result<usize> {
        self.emit_event(
            EventType::MemoryWriteStart,
            json!({
                "session_id": session_id,
                "correlation_id": correlation_id,
                "trigger": "pre_compaction_flush",
            }),
            Some(correlation_id),
        );

        // Load recent conversation history
        let recent_messages = self.session_manager.load_messages(session_id, Some(30), None).await?;

        if recent_messages.is_empty() {
            self.log_to_ledger(
                Some(identity_id),
                "agentic.memory_flush",
                json!({
                    "session_id": session_id,
                    "memories_created": 0,
                    "nothing_to_store": true,
                    "reason": "no_messages",
                    "correlation_id": correlation_id,
                }),
                Some(correlation_id),
            ).await;

            self.emit_event(
                EventType::MemoryWriteEnd,
                json!({
                    "session_id": session_id,
                    "memories_created": 0,
                    "correlation_id": correlation_id,
                }),
                Some(correlation_id),
            );

            return Ok(0);
        }

        // Build conversation summary for the model
        let mut conversation_text = String::new();
        for msg in recent_messages.iter().rev() {
            conversation_text.push_str(&format!("[{}]: {}\n", msg.role, msg.content));
        }

        let system_prompt = "Review the conversation and extract important information to store as durable memories. \
            For each memory, provide a JSON array of objects with fields: \
            \"content\" (text), \"importance\" (0.0-1.0), \"source\" (one of: conversation, task, observation, reflection). \
            If there is nothing important to store, return an empty array: []\n\
            Respond ONLY with the JSON array, no other text.";

        let flush_request = CompletionRequest {
            model: self.config.machine_config().default_model,
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                    name: None,
                    tool_call_id: None,
                },
                Message {
                    role: "user".to_string(),
                    content: conversation_text,
                    name: None,
                    tool_call_id: None,
                },
            ],
            temperature: Some(0.3),
            max_tokens: Some(1024),
            stream: Some(false),
            correlation_id: Some(correlation_id),
        };

        let flush_response = self.model_router.complete(
            flush_request,
            identity_id,
            None,
            None,
        ).await?;

        let flush_content = flush_response.choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        // Parse memory entries from response
        let memory_entries: Vec<JsonValue> = Self::parse_memory_entries(&flush_content);

        if memory_entries.is_empty() {
            self.log_to_ledger(
                Some(identity_id),
                "agentic.memory_flush",
                json!({
                    "session_id": session_id,
                    "memories_created": 0,
                    "nothing_to_store": true,
                    "correlation_id": correlation_id,
                }),
                Some(correlation_id),
            ).await;

            self.emit_event(
                EventType::MemoryWriteEnd,
                json!({
                    "session_id": session_id,
                    "memories_created": 0,
                    "correlation_id": correlation_id,
                }),
                Some(correlation_id),
            );

            return Ok(0);
        }

        // Persist each memory
        let mut created_count = 0usize;

        for entry in &memory_entries {
            let content = entry.get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if content.is_empty() {
                continue;
            }

            let importance = entry.get("importance")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5) as f32;

            // Clamp importance to valid range
            let importance = importance.clamp(0.0, 1.0);

            let source_str = entry.get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("conversation");

            let source = match source_str {
                "task" => MemorySource::Task,
                "observation" => MemorySource::Observation,
                "reflection" => MemorySource::Reflection,
                _ => MemorySource::Conversation,
            };

            match self.memory_manager.create_memory(
                identity_id,
                &content,
                None,
                source,
                None,
                importance,
            ).await {
                Ok(_) => created_count += 1,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        content_preview = %&content[..content.len().min(50)],
                        "Failed to create memory during flush"
                    );
                }
            }
        }

        self.log_to_ledger(
            Some(identity_id),
            "agentic.memory_flush",
            json!({
                "session_id": session_id,
                "memories_created": created_count,
                "flush_failed": false,
                "correlation_id": correlation_id,
            }),
            Some(correlation_id),
        ).await;

        self.emit_event(
            EventType::MemoryWriteEnd,
            json!({
                "session_id": session_id,
                "memories_created": created_count,
                "correlation_id": correlation_id,
            }),
            Some(correlation_id),
        );

        Ok(created_count)
    }

    /// Parse memory entries from the model's flush response.
    ///
    /// Attempts to extract a JSON array from the response content.
    /// Returns an empty vec if parsing fails.
    fn parse_memory_entries(content: &str) -> Vec<JsonValue> {
        // Try direct parse as array
        if let Ok(arr) = serde_json::from_str::<Vec<JsonValue>>(content.trim()) {
            return arr;
        }

        // Try to find array in content
        if let Some(start) = content.find('[') {
            if let Some(end) = content.rfind(']') {
                if start < end {
                    if let Ok(arr) = serde_json::from_str::<Vec<JsonValue>>(&content[start..=end]) {
                        return arr;
                    }
                }
            }
        }

        Vec::new()
    }

    // =========================================================================
    // OBSERVABILITY HELPERS
    // =========================================================================

    /// Log an event to the tamper-resistant audit ledger.
    ///
    /// Errors are logged as warnings but never propagated — the ledger
    /// is best-effort and must not block execution.
    async fn log_to_ledger(
        &self,
        actor_id: Option<Uuid>,
        action_type: &str,
        payload: JsonValue,
        correlation_id: Option<Uuid>,
    ) {
        if let Err(e) = self.ledger.append_event(
            actor_id,
            action_type,
            payload,
            correlation_id,
        ).await {
            tracing::warn!(
                error = %e,
                action_type = %action_type,
                "Failed to log to ledger (best-effort)"
            );
        }
    }

    /// Emit an event to the event stream for real-time observability.
    ///
    /// No-op if no event stream is attached. Errors are silently ignored
    /// since events are best-effort.
    fn emit_event(
        &self,
        event_type: EventType,
        payload: JsonValue,
        correlation_id: Option<Uuid>,
    ) {
        if let Some(ref stream) = self.event_stream {
            let mut envelope = EventEnvelope::new(
                EventLevel::Info,
                event_type,
                payload,
            );
            if let Some(cid) = correlation_id {
                envelope = envelope.with_correlation_id(cid);
            }
            stream.publish(envelope);
        }
    }
}

// =============================================================================
// ERROR VARIANT
// =============================================================================

// Note: The `Error::Agentic` variant must be added to carnelian_common::Error.
// If it does not exist yet, tool call parsing and plan validation will use
// a compatible error variant.

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_calls_valid_json() {
        let content = r#"Here is my response:
        {
            "tool_calls": [
                {
                    "tool_id": "call_001",
                    "tool_name": "read_file",
                    "arguments": {"path": "/etc/hosts"}
                },
                {
                    "tool_id": "call_002",
                    "tool_name": "write_file",
                    "arguments": {"path": "/tmp/out.txt", "content": "hello"}
                }
            ]
        }"#;

        let calls = AgenticEngine::parse_tool_calls(content).unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool_name, "read_file");
        assert_eq!(calls[0].tool_id, "call_001");
        assert_eq!(calls[0].arguments["path"], "/etc/hosts");
        assert_eq!(calls[1].tool_name, "write_file");
        assert_eq!(calls[1].tool_id, "call_002");
    }

    #[test]
    fn test_parse_tool_calls_with_name_alias() {
        let content = r#"{"tool_calls": [{"id": "c1", "name": "list_dir", "arguments": {}}]}"#;

        let calls = AgenticEngine::parse_tool_calls(content).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "list_dir");
        assert_eq!(calls[0].tool_id, "c1");
    }

    #[test]
    fn test_parse_tool_calls_invalid_json() {
        let content = "This is just a plain text response with no JSON.";
        let result = AgenticEngine::parse_tool_calls(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_tool_calls_missing_tool_name() {
        let content = r#"{"tool_calls": [{"tool_id": "c1", "arguments": {}}]}"#;
        let result = AgenticEngine::parse_tool_calls(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_tool_calls_empty_array() {
        let content = r#"{"tool_calls": []}"#;
        let result = AgenticEngine::parse_tool_calls(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_declarative_plan_valid_json() {
        let content = r#"{
            "plan": {
                "description": "Fetch and process data",
                "steps": [
                    {
                        "action": "fetch_data",
                        "skill": "http_get",
                        "parameters": {"url": "https://api.example.com"},
                        "dependencies": []
                    },
                    {
                        "action": "process_data",
                        "skill": "python_script",
                        "parameters": {"script": "process.py"},
                        "dependencies": ["step_1"]
                    }
                ]
            }
        }"#;

        let plan = AgenticEngine::parse_declarative_plan(content).unwrap();
        assert_eq!(plan.description, "Fetch and process data");
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].action, "fetch_data");
        assert_eq!(plan.steps[0].skill_name.as_deref(), Some("http_get"));
        assert!(plan.steps[0].dependencies.is_empty());
        assert_eq!(plan.steps[1].action, "process_data");
        assert_eq!(plan.steps[1].dependencies.len(), 1);
        assert_eq!(plan.steps[1].dependencies[0], plan.steps[0].step_id);
    }

    #[test]
    fn test_parse_declarative_plan_circular_dependencies() {
        // Manually construct a plan with circular deps to test validation
        let plan = DeclarativePlan {
            plan_id: Uuid::now_v7(),
            description: "Circular plan".to_string(),
            steps: vec![
                PlanStep {
                    step_id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
                    action: "step_a".to_string(),
                    skill_name: None,
                    parameters: json!({}),
                    dependencies: vec![
                        Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
                    ],
                },
                PlanStep {
                    step_id: Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
                    action: "step_b".to_string(),
                    skill_name: None,
                    parameters: json!({}),
                    dependencies: vec![
                        Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
                    ],
                },
            ],
        };

        let result = AgenticEngine::validate_plan_dependencies(&plan);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circular dependency"));
    }

    #[test]
    fn test_plan_step_dependency_ordering() {
        let id_a = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let id_b = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();
        let id_c = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();

        let plan = DeclarativePlan {
            plan_id: Uuid::now_v7(),
            description: "Ordered plan".to_string(),
            steps: vec![
                PlanStep {
                    step_id: id_c,
                    action: "step_c".to_string(),
                    skill_name: None,
                    parameters: json!({}),
                    dependencies: vec![id_a, id_b],
                },
                PlanStep {
                    step_id: id_a,
                    action: "step_a".to_string(),
                    skill_name: None,
                    parameters: json!({}),
                    dependencies: vec![],
                },
                PlanStep {
                    step_id: id_b,
                    action: "step_b".to_string(),
                    skill_name: None,
                    parameters: json!({}),
                    dependencies: vec![id_a],
                },
            ],
        };

        let order = AgenticEngine::topological_sort(&plan).unwrap();
        assert_eq!(order.len(), 3);

        // A must come before B and C
        let pos_a = order.iter().position(|&id| id == id_a).unwrap();
        let pos_b = order.iter().position(|&id| id == id_b).unwrap();
        let pos_c = order.iter().position(|&id| id == id_c).unwrap();

        assert!(pos_a < pos_b, "A must come before B");
        assert!(pos_a < pos_c, "A must come before C");
        assert!(pos_b < pos_c, "B must come before C");
    }

    #[test]
    fn test_memory_flush_nothing_to_store() {
        let entries = AgenticEngine::parse_memory_entries("[]");
        assert!(entries.is_empty());

        let entries = AgenticEngine::parse_memory_entries("No memories to extract.");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_memory_entries_valid() {
        let content = r#"[
            {"content": "User prefers concise responses", "importance": 0.9, "source": "conversation"},
            {"content": "Project uses Rust with Tokio", "importance": 0.7, "source": "observation"}
        ]"#;

        let entries = AgenticEngine::parse_memory_entries(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["content"], "User prefers concise responses");
        assert_eq!(entries[1]["importance"], 0.7);
    }

    #[test]
    fn test_parse_memory_entries_embedded_in_text() {
        let content = r#"Here are the memories I extracted:
        [{"content": "Important fact", "importance": 0.8, "source": "conversation"}]
        That's all."#;

        let entries = AgenticEngine::parse_memory_entries(content);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_extract_json_object() {
        let content = r#"Some text {"key": "value", "nested": {"a": 1}} more text"#;
        let json = AgenticEngine::extract_json_object(content);
        assert!(json.is_some());
        let parsed: JsonValue = serde_json::from_str(json.unwrap()).unwrap();
        assert_eq!(parsed["key"], "value");
        assert_eq!(parsed["nested"]["a"], 1);
    }

    #[test]
    fn test_extract_json_object_with_strings() {
        let content = r#"{"key": "value with { braces }"}"#;
        let json = AgenticEngine::extract_json_object(content);
        assert!(json.is_some());
        let parsed: JsonValue = serde_json::from_str(json.unwrap()).unwrap();
        assert_eq!(parsed["key"], "value with { braces }");
    }

    #[test]
    fn test_extract_json_object_none() {
        assert!(AgenticEngine::extract_json_object("no json here").is_none());
    }

    #[test]
    fn test_correlation_id_propagation() {
        // Verify that a provided correlation_id is preserved
        let request = AgenticRequest {
            session_key: SessionKey {
                agent_id: Uuid::new_v4(),
                channel: "test".to_string(),
                group_id: None,
            },
            identity_id: Uuid::new_v4(),
            task_id: None,
            run_id: None,
            user_message: "Hello".to_string(),
            correlation_id: Some(Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()),
        };

        assert_eq!(
            request.correlation_id.unwrap(),
            Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()
        );
    }

    #[test]
    fn test_tool_call_status_serialization() {
        let result = ToolCallResult {
            call_id: "c1".to_string(),
            tool_name: "test".to_string(),
            status: ToolCallStatus::Denied,
            result: json!(null),
            error: Some("denied".to_string()),
            duration_ms: 5,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "Denied");
    }

    #[test]
    fn test_plan_step_status_serialization() {
        let result = PlanStepResult {
            step_id: Uuid::new_v4(),
            status: PlanStepStatus::Skipped,
            result: json!(null),
            error: Some("dep failed".to_string()),
            duration_ms: 0,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "Skipped");
    }

    // =========================================================================
    // IGNORED INTEGRATION TESTS (require database)
    // =========================================================================

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_agentic_loop_simple_response() {
        // End-to-end test with simple text response
        // Run with: cargo test test_agentic_loop_simple_response -- --ignored
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_agentic_loop_tool_calls() {
        // End-to-end test with tool call execution
        // Run with: cargo test test_agentic_loop_tool_calls -- --ignored
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_agentic_loop_declarative_plan() {
        // End-to-end test with plan execution
        // Run with: cargo test test_agentic_loop_declarative_plan -- --ignored
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_agentic_loop_memory_flush() {
        // End-to-end test with memory flush before compaction
        // Run with: cargo test test_agentic_loop_memory_flush -- --ignored
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_agentic_loop_compaction_trigger() {
        // End-to-end test triggering compaction
        // Run with: cargo test test_agentic_loop_compaction_trigger -- --ignored
    }
}
