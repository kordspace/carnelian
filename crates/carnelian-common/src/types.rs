//! Common types used throughout 🔥 Carnelian OS

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

// =============================================================================
// IDENTIFIERS
// =============================================================================

/// Unique identifier for tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for skill executions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId(pub Uuid);

impl RunId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for RunId {
    fn default() -> Self {
        Self::new()
    }
}

/// Task execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Capability grant for worker execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub id: Uuid,
    pub scope: String,
    pub permissions: Vec<String>,
    pub created_at: DateTime<Utc>,
}

// =============================================================================
// EVENT STREAMING
// =============================================================================

/// Unique identifier for events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub Uuid);

impl EventId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

/// Event severity level (matches logging levels)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum EventLevel {
    /// Critical errors that require immediate attention
    Error = 0,
    /// Warning conditions that may indicate problems
    Warn = 1,
    /// Informational messages about normal operation
    Info = 2,
    /// Debug information for development
    Debug = 3,
    /// Detailed trace information (very verbose)
    Trace = 4,
}

impl EventLevel {
    /// Get the priority weight for backpressure decisions.
    /// Lower values = higher priority (never dropped).
    #[must_use]
    pub const fn priority(&self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warn => 1,
            Self::Info => 2,
            Self::Debug => 3,
            Self::Trace => 4,
        }
    }
}

/// Event type categories for the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    // Task lifecycle
    TaskCreated,
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    TaskCancelled,

    // Worker lifecycle
    WorkerStarted,
    WorkerStopped,
    WorkerHealthCheck,

    // Skill execution
    SkillInvokeStart,
    SkillInvokeEnd,
    SkillInvokeFailed,

    // Memory operations
    MemoryFetchStart,
    MemoryFetchEnd,
    MemoryCompressStart,
    MemoryCompressEnd,
    MemoryWriteStart,
    MemoryWriteEnd,

    // Gateway operations
    GatewayRequestStart,
    GatewayRequestEnd,
    GatewayRateLimited,

    // Database operations
    DbQueryStart,
    DbQueryEnd,
    DbTransactionBegin,
    DbTransactionCommit,
    DbTransactionRollback,

    // Security events
    CapabilityGranted,
    CapabilityDenied,
    CapabilityRevoked,

    // System events
    RuntimeStart,
    RuntimeReady,
    RuntimeShutdown,
    ConfigLoaded,
    HeartbeatTick,

    // Custom event type for extensibility
    Custom(String),
}

/// Event envelope containing metadata and payload
///
/// This is the primary structure for all events in the system.
/// Events are published to the event stream and distributed to subscribers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Unique identifier for this event
    pub event_id: EventId,

    /// When the event occurred
    pub timestamp: DateTime<Utc>,

    /// Severity level of the event
    pub level: EventLevel,

    /// Type/category of the event
    pub event_type: EventType,

    /// Actor that generated the event (worker ID, task ID, etc.)
    pub actor_id: Option<String>,

    /// Correlation ID for request tracing across services
    pub correlation_id: Option<Uuid>,

    /// Event payload (JSON data)
    pub payload: JsonValue,

    /// Whether the payload was truncated due to size limits
    pub truncated: bool,
}

impl EventEnvelope {
    /// Maximum payload size in bytes (64KB)
    pub const MAX_PAYLOAD_BYTES: usize = 65_536;

    /// Create a new event envelope with the given level, type, and payload.
    #[must_use]
    pub fn new(level: EventLevel, event_type: EventType, payload: JsonValue) -> Self {
        Self {
            event_id: EventId::new(),
            timestamp: Utc::now(),
            level,
            event_type,
            actor_id: None,
            correlation_id: None,
            payload,
            truncated: false,
        }
    }

    /// Set the actor ID for this event.
    #[must_use]
    pub fn with_actor_id(mut self, actor_id: impl Into<String>) -> Self {
        self.actor_id = Some(actor_id.into());
        self
    }

    /// Set the correlation ID for request tracing.
    #[must_use]
    pub const fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    /// Truncate the payload if it exceeds the maximum size.
    ///
    /// Returns `true` if truncation occurred.
    pub fn truncate_payload_if_needed(&mut self) -> bool {
        if let Ok(serialized) = serde_json::to_string(&self.payload) {
            if serialized.len() > Self::MAX_PAYLOAD_BYTES {
                self.payload = serde_json::json!({
                    "...": "payload truncated at 64KB",
                    "original_size_bytes": serialized.len()
                });
                self.truncated = true;
                return true;
            }
        }
        false
    }

    /// Check if this event should be retained based on priority.
    /// ERROR events are always retained.
    #[must_use]
    pub const fn is_critical(&self) -> bool {
        matches!(self.level, EventLevel::Error)
    }
}

// =============================================================================
// WORKER TRANSPORT PROTOCOL
// =============================================================================

/// Request to invoke a skill on a worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeRequest {
    /// Unique identifier for this execution run
    pub run_id: RunId,
    /// Name of the skill to invoke
    pub skill_name: String,
    /// Input payload for the skill
    pub input: JsonValue,
    /// Timeout in seconds for this invocation
    pub timeout_secs: u64,
    /// Correlation ID for request tracing
    pub correlation_id: Option<Uuid>,
}

/// Request to cancel a running skill execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelRequest {
    /// Run ID of the execution to cancel
    pub run_id: RunId,
    /// Reason for cancellation
    pub reason: String,
}

/// Health check request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthRequest;

/// Status of a skill invocation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvokeStatus {
    /// Skill completed successfully
    Success,
    /// Skill execution failed
    Failed,
    /// Skill execution timed out
    Timeout,
    /// Skill execution was cancelled
    Cancelled,
}

/// Response from a skill invocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeResponse {
    /// Run ID this response corresponds to
    pub run_id: RunId,
    /// Outcome status
    pub status: InvokeStatus,
    /// Result payload (empty object on failure)
    pub result: JsonValue,
    /// Error message if status is not Success
    pub error: Option<String>,
    /// Process exit code if available
    pub exit_code: Option<i32>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Whether the output was truncated due to size limits
    pub truncated: bool,
}

/// Type of stream event emitted during skill execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamEventType {
    /// Log output from the worker
    Log,
    /// Progress update (percentage, stage, etc.)
    Progress,
    /// Artifact produced during execution
    Artifact,
}

/// A streaming event emitted during skill execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    /// Run ID this event belongs to
    pub run_id: RunId,
    /// Type of stream event
    pub event_type: StreamEventType,
    /// When the event was emitted
    pub timestamp: DateTime<Utc>,
    /// Log level (relevant for Log events)
    pub level: Option<EventLevel>,
    /// Human-readable message
    pub message: String,
    /// Additional structured fields
    pub fields: JsonValue,
}

/// Health check response from a worker transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Whether the worker is healthy
    pub healthy: bool,
    /// Worker identifier
    pub worker_id: String,
    /// Uptime in seconds
    pub uptime_secs: u64,
}

// =============================================================================
// REST API REQUEST / RESPONSE TYPES
// =============================================================================

/// Request body for creating a new task via `POST /v1/tasks`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    /// Human-readable title for the task
    pub title: String,
    /// Optional longer description
    pub description: Option<String>,
    /// Optional skill to execute
    pub skill_id: Option<Uuid>,
    /// Priority (higher = dequeued first, default 0)
    #[serde(default)]
    pub priority: i32,
    /// Whether the task requires manual approval before execution
    #[serde(default)]
    pub requires_approval: bool,
}

/// Response body returned after creating a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskResponse {
    pub task_id: Uuid,
    pub state: String,
    pub created_at: DateTime<Utc>,
}

/// A single task in list / detail responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetail {
    pub task_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub skill_id: Option<Uuid>,
    pub state: String,
    pub priority: i32,
    pub requires_approval: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response body for `GET /v1/tasks`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListTasksResponse {
    pub tasks: Vec<TaskDetail>,
}

/// Request body for cancelling a task via `POST /v1/tasks/:id/cancel`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelTaskRequest {
    /// Human-readable reason for cancellation
    #[serde(default)]
    pub reason: String,
}

/// Response body after cancelling a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelTaskResponse {
    pub task_id: Uuid,
    pub state: String,
}

/// A single task run in detail responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunDetail {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub attempt: i32,
    pub worker_id: Option<String>,
    pub state: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub result: Option<JsonValue>,
    pub error: Option<String>,
}

/// Response body for `GET /v1/tasks/:id/runs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListRunsResponse {
    pub runs: Vec<RunDetail>,
}

/// A single log entry from `run_logs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunLogEntry {
    pub log_id: i64,
    pub run_id: Uuid,
    pub ts: DateTime<Utc>,
    pub level: String,
    pub message: String,
    pub fields: Option<JsonValue>,
    pub truncated: bool,
}

/// Paginated response for `GET /v1/runs/:id/logs`.
///
/// `page` and `page_size` mirror the query parameters; `page_size` is capped
/// at `MAX_PAGE_SIZE` (1000).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedRunLogsResponse {
    pub logs: Vec<RunLogEntry>,
    pub page: u32,
    pub page_size: u32,
    pub total: i64,
}

impl PaginatedRunLogsResponse {
    /// Hard upper-bound on `page_size` to prevent unbounded queries.
    pub const MAX_PAGE_SIZE: u32 = 1000;
}

/// Query parameters for paginated run-log requests.
#[derive(Debug, Clone, Deserialize)]
pub struct RunLogsQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

const fn default_page() -> u32 {
    1
}
const fn default_page_size() -> u32 {
    100
}

/// A single skill in list responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDetail {
    pub skill_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub runtime: String,
    pub enabled: bool,
    pub discovered_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response body for `GET /v1/skills`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSkillsResponse {
    pub skills: Vec<SkillDetail>,
}

/// Response body for `POST /v1/skills/:id/enable` and `POST /v1/skills/:id/disable`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillToggleResponse {
    pub skill_id: Uuid,
    pub enabled: bool,
}

/// Response body for `POST /v1/skills/refresh`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRefreshResponse {
    pub discovered: u32,
    pub updated: u32,
    pub removed: u32,
}

/// Cancellation reason enum for programmatic use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CancellationReason {
    /// User explicitly requested cancellation
    UserRequested,
    /// System timeout exceeded
    Timeout,
    /// Superseded by a newer task
    Superseded,
    /// Other / free-text reason
    Other(String),
}

// =============================================================================
// WORKER TRANSPORT PROTOCOL (continued)
// =============================================================================

/// Envelope for all transport messages, enabling request/response correlation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TransportMessage {
    /// Invoke a skill
    Invoke {
        message_id: Uuid,
        payload: InvokeRequest,
    },
    /// Cancel a running invocation
    Cancel {
        message_id: Uuid,
        payload: CancelRequest,
    },
    /// Health check request
    Health { message_id: Uuid },
    /// Invoke response
    InvokeResult {
        message_id: Uuid,
        payload: InvokeResponse,
    },
    /// Streaming event
    Stream {
        message_id: Uuid,
        payload: StreamEvent,
    },
    /// Health check response
    HealthResult {
        message_id: Uuid,
        payload: HealthResponse,
    },
}
