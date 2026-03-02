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
    MemoryCreated,
    MemoryUpdated,
    MemoryDeleted,
    MemorySearchPerformed,
    MemoryEmbeddingAdded,
    MemoryExported,
    MemoryImported,
    MemoryExportFailed,
    MemoryImportFailed,

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

    // Skill management
    SkillDiscovered,
    SkillUpdated,
    SkillRemoved,

    // Context assembly
    ContextAssembled,
    ContextPruned,
    ContextBudgetExceeded,

    // Soul management
    SoulUpdated,
    SoulLoadFailed,

    // System events
    RuntimeStart,
    RuntimeReady,
    RuntimeShutdown,
    ConfigLoaded,
    HeartbeatTick,
    HeartbeatOk,

    // Approval lifecycle
    ApprovalQueued,
    ApprovalApproved,
    ApprovalDenied,

    // Workspace scanning
    TaskAutoQueued,

    // Sub-agent lifecycle
    SubAgentCreated,
    SubAgentUpdated,
    SubAgentTerminated,
    SubAgentPaused,
    SubAgentResumed,

    // Workflow lifecycle
    WorkflowCreated,
    WorkflowUpdated,
    WorkflowDeleted,
    WorkflowExecutionStarted,
    WorkflowStepCompleted,
    WorkflowExecutionCompleted,
    WorkflowExecutionFailed,

    // Custom event type for extensibility
    Custom(String),
}

/// Event envelope containing metadata and payload
///
/// This is the primary structure for all events in the system.
/// Events are published to the event stream and distributed to subscribers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Attestation data reported by a worker during health checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerAttestationData {
    /// blake3 hash of the most recent ledger event the worker has seen
    pub last_ledger_head: String,
    /// Hash of the worker binary/script
    pub build_checksum: String,
    /// Configuration state identifier
    pub config_version: String,
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
    /// Attestation data (optional, for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation: Option<WorkerAttestationData>,
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
    /// Whether this log message contains sensitive data (encrypted at rest).
    #[serde(default)]
    pub sensitive: bool,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

// =============================================================================
// METRICS RESPONSE TYPES
// =============================================================================

/// Aggregated task latency statistics returned by `/v1/metrics`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    pub mean_ms: f64,
    pub median_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub sample_count: usize,
}

/// Full metrics snapshot returned by `GET /v1/metrics`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub task_latency: LatencyStats,
    pub event_throughput_per_sec: f64,
    pub event_stream_buffer_len: usize,
    pub event_stream_buffer_capacity: usize,
    pub event_stream_fill_percentage: f64,
    pub event_stream_total_received: usize,
    pub event_stream_total_stored: usize,
    pub event_stream_subscriber_count: usize,
    /// Average UI render duration in milliseconds (reported by the UI client).
    #[serde(default)]
    pub render_time_ms: f64,
    pub timestamp: DateTime<Utc>,
}

// =============================================================================
// APPROVAL QUEUE API TYPES
// =============================================================================

/// A single approval request in list / detail responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequestDetail {
    pub id: Uuid,
    pub action_type: String,
    pub payload: JsonValue,
    pub status: String,
    pub requested_by: Option<Uuid>,
    pub requested_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<Uuid>,
    pub correlation_id: Option<Uuid>,
}

/// Response body for `GET /v1/approvals`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListApprovalsResponse {
    pub approvals: Vec<ApprovalRequestDetail>,
}

/// Request body for `POST /v1/approvals/:id/approve` and `POST /v1/approvals/:id/deny`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalActionRequest {
    /// Hex-encoded Ed25519 signature (unused when server signs internally,
    /// but kept for future client-side signing).
    #[serde(default)]
    pub signature: String,
}

/// Response body after approving or denying an approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalActionResponse {
    pub approval_id: Uuid,
    pub status: String,
}

/// Request body for `POST /v1/approvals/batch`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchApprovalRequest {
    pub approval_ids: Vec<Uuid>,
    #[serde(default)]
    pub signature: String,
}

/// Response body for batch approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchApprovalResponse {
    pub approved: Vec<Uuid>,
    pub failed: Vec<Uuid>,
}

// =============================================================================
// CAPABILITY MANAGEMENT API TYPES
// =============================================================================

/// A single capability grant in list / detail responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityGrantDetail {
    pub grant_id: Uuid,
    pub subject_type: String,
    pub subject_id: String,
    pub capability_key: String,
    pub scope: Option<JsonValue>,
    pub constraints: Option<JsonValue>,
    pub approved_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Response body for `GET /v1/capabilities`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCapabilitiesResponse {
    pub grants: Vec<CapabilityGrantDetail>,
}

/// Request body for `POST /v1/capabilities`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantCapabilityRequest {
    pub subject_type: String,
    pub subject_id: String,
    pub capability_key: String,
    #[serde(default)]
    pub scope: Option<JsonValue>,
    #[serde(default)]
    pub constraints: Option<JsonValue>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

/// Response body after granting a capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantCapabilityResponse {
    pub grant_id: Uuid,
}

/// Response body after revoking a capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeCapabilityResponse {
    pub revoked: bool,
}

// =============================================================================
// MEMORY API TYPES
// =============================================================================

/// Request body for `POST /v1/memories`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMemoryRequest {
    pub identity_id: Uuid,
    pub content: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub source: String,
    pub importance: f32,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

/// Response body after creating a memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMemoryResponse {
    pub memory_id: Uuid,
    pub created_at: DateTime<Utc>,
}

/// Detail view of a memory record (excludes embedding).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDetail {
    pub memory_id: Uuid,
    pub identity_id: Uuid,
    pub content: String,
    pub summary: Option<String>,
    pub source: String,
    pub importance: f32,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub accessed_at: DateTime<Utc>,
    pub access_count: i32,
}

/// Response body for `GET /v1/memories`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMemoriesResponse {
    pub memories: Vec<MemoryDetail>,
}

/// Response body for `GET /v1/memories/{memory_id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMemoryResponse {
    pub memory: MemoryDetail,
}

// =============================================================================
// MEMORY PORTABILITY API TYPES
// =============================================================================

/// Request body for `POST /v1/memories/export`.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMemoryRequest {
    pub memory_ids: Vec<Uuid>,
    #[serde(default)]
    pub include_embedding: bool,
    #[serde(default)]
    pub topic_filter: Option<Vec<String>>,
    #[serde(default)]
    pub min_importance: Option<f32>,
    #[serde(default)]
    pub include_ledger_proof: bool,
    #[serde(default)]
    pub include_capabilities: bool,
    #[serde(default)]
    pub sign_export: bool,
}

/// Response body for `POST /v1/memories/export`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMemoryResponse {
    pub envelope_base64: String,
    pub memory_count: usize,
    pub signed: bool,
    pub export_timestamp: DateTime<Utc>,
}

/// Request body for `POST /v1/memories/import`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportMemoryRequest {
    pub envelope_base64: String,
    pub identity_id: Uuid,
    #[serde(default)]
    pub verify_signature: bool,
    #[serde(default)]
    pub public_key: Option<String>,
}

/// Single import result for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryImportResultApi {
    pub memory_id: Uuid,
    pub verified: bool,
    pub ledger_proof_valid: bool,
    pub warnings: Vec<String>,
}

/// Response body for `POST /v1/memories/import`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportMemoryResponse {
    pub results: Vec<MemoryImportResultApi>,
    pub successful_count: usize,
    pub failed_count: usize,
}

// =============================================================================
// HEARTBEAT API TYPES
// =============================================================================

/// A single heartbeat record from the `heartbeat_history` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRecord {
    pub heartbeat_id: Uuid,
    pub identity_id: Uuid,
    pub ts: DateTime<Utc>,
    pub mantra: Option<String>,
    pub tasks_queued: i32,
    pub status: String,
    pub duration_ms: Option<i32>,
}

/// Response body for `GET /v1/heartbeats/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatStatusResponse {
    pub current_mantra: Option<String>,
    pub last_heartbeat_time: Option<DateTime<Utc>>,
    pub next_heartbeat_time: Option<DateTime<Utc>>,
    pub interval_ms: u64,
}

// =============================================================================
// IDENTITY API TYPES
// =============================================================================

/// Response body for `GET /v1/identity`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityResponse {
    pub identity_id: Uuid,
    pub name: String,
    pub pronouns: Option<String>,
    pub identity_type: String,
    pub soul_file_path: Option<String>,
    pub directive_count: usize,
    pub public_key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// =============================================================================
// PROVIDER API TYPES
// =============================================================================

/// Detail view of a model provider row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDetail {
    pub provider_id: Uuid,
    pub provider_type: String,
    pub name: String,
    pub enabled: bool,
    pub config: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Response body for `GET /v1/providers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListProvidersResponse {
    pub providers: Vec<ProviderDetail>,
}

/// Response body for `GET /v1/providers/ollama/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaStatusResponse {
    pub connected: bool,
    pub url: String,
    pub available_models: Vec<String>,
    pub error: Option<String>,
}

// =============================================================================
// SUB-AGENT TYPES
// =============================================================================

/// Detail record for a sub-agent, returned by list/get endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubAgentDetail {
    pub sub_agent_id: Uuid,
    pub parent_id: Uuid,
    pub created_by: Uuid,
    pub model_provider: Option<Uuid>,
    pub name: String,
    pub role: String,
    pub directives: Option<JsonValue>,
    pub ephemeral: bool,
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    pub terminated_at: Option<DateTime<Utc>>,
}

/// Response body for `GET /v1/sub-agents`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSubAgentsResponse {
    pub sub_agents: Vec<SubAgentDetail>,
}

/// Request body for `POST /v1/sub-agents`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSubAgentApiRequest {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub directives: Option<JsonValue>,
    #[serde(default)]
    pub model_provider: Option<Uuid>,
    #[serde(default)]
    pub ephemeral: bool,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default = "default_sub_agent_runtime")]
    pub runtime: String,
}

fn default_sub_agent_runtime() -> String {
    "node".to_string()
}

/// Request body for `PUT /v1/sub-agents/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSubAgentApiRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub directives: Option<JsonValue>,
    #[serde(default)]
    pub model_provider: Option<Uuid>,
    #[serde(default)]
    pub capabilities: Option<Vec<String>>,
}

/// Response body for `POST /v1/sub-agents` (creation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSubAgentResponse {
    pub sub_agent_id: Uuid,
    pub worker_id: Option<String>,
    pub worker_warning: Option<String>,
    pub name: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// Response body for pause/resume actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentActionResponse {
    pub status: String,
    #[serde(default)]
    pub worker_id: Option<String>,
    #[serde(default)]
    pub worker_warning: Option<String>,
}

// =============================================================================
// WORKFLOW TYPES
// =============================================================================

/// A single step in a workflow definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStepDef {
    pub step_id: String,
    pub skill_name: String,
    #[serde(default)]
    pub input_mapping: Option<JsonValue>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub condition: Option<JsonValue>,
    #[serde(default)]
    pub retry_policy: Option<WorkflowRetryPolicy>,
    #[serde(default)]
    pub continue_on_error: bool,
}

/// Retry policy for a workflow step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRetryPolicy {
    pub max_attempts: u32,
    #[serde(default = "default_retry_delay_secs")]
    pub delay_secs: u64,
}

const fn default_retry_delay_secs() -> u64 {
    5
}

/// Detail record for a workflow, returned by list/get endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowDetail {
    pub workflow_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Option<Uuid>,
    pub steps: Vec<WorkflowStepDef>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response for listing workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListWorkflowsResponse {
    pub workflows: Vec<WorkflowDetail>,
}

/// Request body for `POST /v1/workflows`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkflowRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub steps: Vec<WorkflowStepDef>,
}

/// Request body for `PUT /v1/workflows/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWorkflowRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub steps: Option<Vec<WorkflowStepDef>>,
}

/// Request body for `POST /v1/workflows/{id}/execute`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteWorkflowRequest {
    #[serde(default = "default_empty_json")]
    pub input: JsonValue,
    #[serde(default)]
    pub correlation_id: Option<Uuid>,
}

fn default_empty_json() -> JsonValue {
    JsonValue::Object(serde_json::Map::new())
}

/// Query parameters for listing workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListWorkflowsParams {
    #[serde(default)]
    pub enabled_only: bool,
}

/// Result of a single workflow step execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResultDetail {
    pub step_id: String,
    pub skill_name: String,
    pub status: String,
    pub output: Option<JsonValue>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Response body for workflow execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionResponse {
    pub workflow_id: Uuid,
    pub workflow_name: String,
    pub status: String,
    pub steps: Vec<StepResultDetail>,
    pub total_duration_ms: u64,
    pub successful_steps: usize,
    pub failed_steps: usize,
    pub execution_summary: String,
    #[serde(default)]
    pub correlation_id: Option<Uuid>,
}

// =============================================================================
// CHANNEL API TYPES
// =============================================================================

/// Detail view of a channel session, returned by list/get endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelDetail {
    pub session_id: Uuid,
    pub channel_type: String,
    pub channel_user_id: String,
    pub trust_level: String,
    pub identity_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub metadata: JsonValue,
    #[serde(default)]
    pub adapter_running: bool,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub last_message_text: Option<String>,
    #[serde(default)]
    pub last_message_at: Option<DateTime<Utc>>,
}

/// Response body for `GET /v1/channels`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListChannelsResponse {
    pub channels: Vec<ChannelDetail>,
}

/// Request body for `POST /v1/channels`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannelApiRequest {
    pub channel_type: String,
    pub channel_user_id: String,
    #[serde(default)]
    pub bot_token: Option<String>,
    #[serde(default = "default_trust_level")]
    pub trust_level: String,
    #[serde(default)]
    pub identity_id: Option<Uuid>,
    #[serde(default = "default_empty_json")]
    pub metadata: JsonValue,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_trust_level() -> String {
    "conversational".to_string()
}

const fn default_enabled() -> bool {
    true
}

/// Response body after creating a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannelResponse {
    pub session_id: Uuid,
    pub channel_type: String,
    pub channel_user_id: String,
    pub status: String,
}

/// Request body for `PUT /v1/channels/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChannelApiRequest {
    #[serde(default)]
    pub trust_level: Option<String>,
    #[serde(default)]
    pub bot_token: Option<String>,
    #[serde(default)]
    pub metadata: Option<JsonValue>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

/// Request body for `POST /v1/channels/{id}/pair`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairChannelApiRequest {
    #[serde(default)]
    pub trust_level: Option<String>,
}

/// Response body after initiating pairing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairChannelResponse {
    pub pairing_token: Uuid,
    pub expires_at: String,
    #[serde(default)]
    pub requested_trust_level: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
}

// =============================================================================
// XP API TYPES
// =============================================================================

/// Response body for `GET /v1/xp/agents/:id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentXpResponse {
    pub identity_id: Uuid,
    pub total_xp: i64,
    pub level: i32,
    pub xp_to_next_level: i64,
    pub progress_pct: f64,
    pub milestone_feature: Option<String>,
}

/// A single XP event row from `xp_events`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XpEventDetail {
    pub event_id: i64,
    pub source: String,
    pub xp_amount: i32,
    pub task_id: Option<Uuid>,
    pub skill_id: Option<Uuid>,
    pub ledger_event_id: Option<i64>,
    pub metadata: JsonValue,
    pub created_at: DateTime<Utc>,
}

/// Query parameters for paginated XP history requests.
#[derive(Debug, Clone, Deserialize)]
pub struct XpHistoryQuery {
    #[serde(default = "default_xp_page")]
    pub page: u32,
    #[serde(default = "default_xp_page_size")]
    pub page_size: u32,
}

impl XpHistoryQuery {
    /// Hard upper-bound on `page_size` to prevent unbounded queries.
    pub const MAX_PAGE_SIZE: u32 = 500;
}

const fn default_xp_page() -> u32 {
    1
}
const fn default_xp_page_size() -> u32 {
    50
}

/// Paginated response for `GET /v1/xp/agents/:id/history`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XpHistoryResponse {
    pub events: Vec<XpEventDetail>,
    pub page: u32,
    pub page_size: u32,
    pub total: i64,
}

/// A single entry on the XP leaderboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: i64,
    pub identity_id: Uuid,
    pub name: String,
    pub total_xp: i64,
    pub level: i32,
}

/// Response body for `GET /v1/xp/leaderboard`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XpLeaderboardResponse {
    pub entries: Vec<LeaderboardEntry>,
}

/// Detail view of skill metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetricsDetail {
    pub skill_id: Uuid,
    pub skill_name: String,
    pub usage_count: i32,
    pub success_count: i32,
    pub failure_count: i32,
    pub success_rate: f64,
    pub avg_duration_ms: i32,
    pub total_xp_earned: i64,
    pub skill_level: i32,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Query parameters for top skills endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct TopSkillsQuery {
    #[serde(default = "default_top_skills_limit")]
    pub limit: i64,
}

const fn default_top_skills_limit() -> i64 {
    10
}

/// Response body for `GET /v1/xp/skills/top`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopSkillsResponse {
    pub skills: Vec<SkillMetricsDetail>,
}

/// Request body for `POST /v1/xp/award`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwardXpRequest {
    pub identity_id: Uuid,
    pub xp_amount: i32,
    pub source: String,
    #[serde(default)]
    pub task_id: Option<Uuid>,
    #[serde(default)]
    pub skill_id: Option<Uuid>,
    #[serde(default)]
    pub ledger_event_id: Option<i64>,
    #[serde(default)]
    pub metadata: Option<JsonValue>,
    pub signature: String,
}

/// Response body for `POST /v1/xp/award`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwardXpResponse {
    pub identity_id: Uuid,
    pub xp_awarded: i32,
    pub new_total_xp: i64,
    pub level_up: Option<i32>,
}

// =============================================================================
// VOICE API TYPES
// =============================================================================

/// A single voice entry from `ElevenLabs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceInfo {
    pub voice_id: String,
    pub name: String,
    pub description: Option<String>,
    pub preview_url: Option<String>,
    pub labels: std::collections::HashMap<String, String>,
}

/// Response body for `GET /v1/voice/voices`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListVoicesResponse {
    pub voices: Vec<VoiceInfo>,
}

/// Request body for `POST /v1/voice/configure`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigureVoiceRequest {
    pub api_key: String,
    pub default_voice_id: Option<String>,
    pub identity_id: Option<Uuid>,
}

/// Response body for `POST /v1/voice/configure`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigureVoiceResponse {
    pub success: bool,
    pub message: String,
}

/// Request body for `POST /v1/voice/test`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestVoiceRequest {
    pub text: String,
    pub voice_id: String,
}

/// Response body for `POST /v1/voice/test`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestVoiceResponse {
    pub audio_base64: String,
    pub content_type: String,
}

/// Request body for `POST /v1/voice/transcribe`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscribeVoiceRequest {
    pub audio_base64: String,
}

/// Response body for `POST /v1/voice/transcribe`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscribeVoiceResponse {
    pub text: String,
}

// =============================================================================
// LEDGER ANCHOR API TYPES
// =============================================================================

/// Request body for `POST /v1/ledger/anchor`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishAnchorRequest {
    pub from_event_id: i64,
    pub to_event_id: i64,
}

/// Response body for `POST /v1/ledger/anchor`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishAnchorResponse {
    pub anchor_id: String,
    pub merkle_root: String,
    pub event_count: i64,
}

/// Response body for `GET /v1/ledger/anchor/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorProofResponse {
    pub anchor_id: String,
    pub hash: String,
    pub ledger_event_from: i64,
    pub ledger_event_to: i64,
    pub published_at: String,
    pub metadata: serde_json::Value,
    pub verified: bool,
}

/// Response body for `GET /v1/ledger/anchor/{id}/verify`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyAnchorResponse {
    pub anchor_id: String,
    pub hash: String,
    pub verified: bool,
}

// =============================================================================
// LEDGER VIEWER API TYPES
// =============================================================================

/// Single ledger event detail for the ledger viewer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEventDetail {
    pub event_id: i64,
    pub timestamp: String,
    pub actor_id: String,
    pub action_type: String,
    pub payload_hash: String,
    pub event_hash: String,
    pub signature: Option<String>,
}

/// Response body for `GET /v1/ledger/events`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListLedgerEventsResponse {
    pub events: Vec<LedgerEventDetail>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
}

/// Response body for `GET /v1/ledger/verify`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerVerifyResponse {
    pub intact: bool,
    pub event_count: u64,
    pub first_event_id: Option<i64>,
    pub last_event_id: Option<i64>,
}

// =============================================================================
// SETUP STATUS API TYPES
// =============================================================================

/// Response body for `GET /v1/config/setup-status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupStatusResponse {
    pub setup_complete: bool,
    pub machine_toml_exists: bool,
}

/// Request body for `POST /v1/config/setup-complete`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupCompleteRequest {}

/// Response body for `POST /v1/config/setup-complete`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupCompleteResponse {
    pub success: bool,
}

// =============================================================================
// SKILL BOOK API TYPES
// =============================================================================

/// Configuration field required for a skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfigField {
    pub key: String,
    pub label: String,
    pub secret: bool,
}

/// Single skill entry in the Skill Book catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBookEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub runtime: String,
    pub version: String,
    pub required_config: Vec<SkillConfigField>,
    pub activated: bool,
}

/// Skill Book catalog response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBookCatalog {
    pub skills: Vec<SkillBookEntry>,
    pub categories: Vec<String>,
}

/// Request to activate a skill from the Skill Book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivateSkillRequest {
    pub config: std::collections::HashMap<String, String>,
}

/// Response from skill activation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivateSkillResponse {
    pub skill_id: String,
    pub activated: bool,
}

/// Response from skill deactivation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeactivateSkillResponse {
    pub skill_id: String,
    pub deactivated: bool,
}

// =============================================================================
// HEALTH & STATUS RESPONSE TYPES
// =============================================================================

/// Worker information included in the system status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    /// Worker identifier
    pub id: String,
    /// Worker runtime type (e.g., "node", "python")
    pub runtime: String,
    /// Worker status
    pub status: String,
    /// Currently executing task, if any
    pub current_task: Option<String>,
}

/// Response body for `GET /v1/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    /// Active workers
    pub workers: Vec<WorkerInfo>,
    /// Available models
    pub models: Vec<String>,
    /// Number of tasks in queue
    pub queue_depth: u32,
    /// UUID of the core identity
    pub identity_id: Option<Uuid>,
    /// App version string
    pub version: String,
    /// Current machine profile name
    pub machine_profile: String,
    /// Seconds since server started
    pub uptime_seconds: Option<u64>,
}

/// Response body for `GET /v1/health/detailed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailedHealthResponse {
    /// Overall health status: "healthy" or "degraded"
    pub status: String,
    /// Application version
    pub version: String,
    /// Database connection status: "connected" or "disconnected"
    pub database: String,
    /// Seconds since server start
    pub uptime_seconds: u64,
    /// Timestamp of the last heartbeat, if any
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    /// Whether the scheduler is running
    pub scheduler_running: bool,
    /// Whether the worker manager is active
    pub worker_manager_active: bool,
    /// Number of active event stream subscribers
    pub event_stream_subscriber_count: usize,
}

// =============================================================================
// ELIXIR API TYPES
// =============================================================================

/// Full detail view of a single elixir, returned by GET /v1/elixirs/:id and list endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElixirDetail {
    pub elixir_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub elixir_type: String,
    pub icon: String,
    pub created_by: Option<Uuid>,
    pub skill_id: Option<Uuid>,
    pub dataset: JsonValue,
    pub size_bytes: i64,
    pub version: i32,
    pub quality_score: f32,
    pub security_integrity_hash: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single version history entry from `elixir_versions`.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ElixirVersion {
    pub version_id: Uuid,
    pub elixir_id: Uuid,
    pub version_number: i32,
    pub dataset: JsonValue,
    pub change_description: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// A single auto-generated draft proposal from `elixir_drafts`.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ElixirDraft {
    pub draft_id: Uuid,
    pub skill_id: Uuid,
    pub proposed_name: String,
    pub proposed_description: Option<String>,
    pub dataset: JsonValue,
    pub auto_created_reason: Option<String>,
    pub status: String,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Request body for POST /v1/elixirs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateElixirRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub elixir_type: String,
    #[serde(default)]
    pub skill_id: Option<Uuid>,
    pub dataset: JsonValue,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub created_by: Option<Uuid>,
}

/// Query parameters for GET /v1/elixirs.
#[derive(Debug, Clone, Deserialize)]
pub struct ListElixirsQuery {
    pub elixir_type: Option<String>,
    pub skill_id: Option<Uuid>,
    pub active: Option<bool>,
    #[serde(default = "default_elixir_page")]
    pub page: u32,
    #[serde(default = "default_elixir_page_size")]
    pub page_size: u32,
}

const fn default_elixir_page() -> u32 {
    1
}

const fn default_elixir_page_size() -> u32 {
    50
}

/// Response body for GET /v1/elixirs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListElixirsResponse {
    pub elixirs: Vec<ElixirDetail>,
    pub page: u32,
    pub page_size: u32,
    pub total: i64,
}

/// Response body for GET /v1/elixirs/drafts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListElixirDraftsResponse {
    pub drafts: Vec<ElixirDraft>,
    pub total: i64,
}

/// Response body for GET /v1/elixirs/search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElixirSearchResponse {
    pub results: Vec<ElixirDetail>,
    pub query: String,
    pub total: i64,
}

/// Response body for POST /v1/elixirs/drafts/:id/approve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveDraftResponse {
    pub draft_id: Uuid,
    pub elixir_id: Uuid,
    pub approved: bool,
}

/// Response body for POST /v1/elixirs/drafts/:id/reject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectDraftResponse {
    pub draft_id: Uuid,
    pub rejected: bool,
}

// =============================================================================
// MAGIC API TYPES
// =============================================================================

use std::collections::HashMap;

/// A single mantra category with entry count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MantraCategory {
    pub category_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub base_weight: i32,
    pub cooldown_beats: i32,
    pub enabled: bool,
    pub entry_count: i64,
}

/// Response body for GET /v1/magic/mantras.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMantraCategoriesResponse {
    pub categories: Vec<MantraCategory>,
}

/// A single mantra entry detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MantraEntryDetail {
    pub entry_id: Uuid,
    pub category_id: Uuid,
    pub text: String,
    pub use_count: i32,
    pub enabled: bool,
    pub elixir_id: Option<Uuid>,
}

/// Response body for GET /v1/magic/mantras/{category_id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMantraEntriesResponse {
    pub entries: Vec<MantraEntryDetail>,
    pub category_id: Uuid,
}

/// Request body for POST /v1/magic/mantras.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMantraEntryRequest {
    pub text: String,
    pub elixir_id: Option<Uuid>,
}

/// Request body for PATCH /v1/magic/mantras/{entry_id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMantraEntryRequest {
    pub text: Option<String>,
    pub enabled: Option<bool>,
    pub elixir_id: Option<Uuid>,
}

/// A single mantra history record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MantraHistoryRecord {
    pub history_id: Uuid,
    pub ts: DateTime<Utc>,
    pub category_id: Uuid,
    pub entry_id: Uuid,
    pub entropy_source: String,
    pub context_snapshot: Option<JsonValue>,
    pub context_weights: Option<JsonValue>,
    pub suggested_skill_ids: Vec<Uuid>,
    pub elixir_reference: Option<Uuid>,
}

/// Response body for GET /v1/magic/mantras/history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MantraHistoryResponse {
    pub history: Vec<MantraHistoryRecord>,
}

/// Response body for POST /v1/magic/mantras/simulate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MantraSimulateResponse {
    pub category: String,
    pub category_id: Uuid,
    pub entry_id: Uuid,
    pub mantra_text: String,
    pub system_message: String,
    pub user_message: String,
    pub entropy_source: String,
    pub selection_ts: DateTime<Utc>,
    pub suggested_skill_ids: Vec<Uuid>,
    pub elixir_reference: Option<Uuid>,
    pub context_weights: HashMap<String, i32>,
}

/// Request body for POST /v1/magic/entropy/sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropySampleRequest {
    pub bytes: usize,
}

/// Response body for POST /v1/magic/entropy/sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropySampleResponse {
    pub bytes: usize,
    pub hex: String,
    pub source: String,
}

/// A single entropy log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyLogEntry {
    pub log_id: Uuid,
    pub ts: DateTime<Utc>,
    pub source: String,
    pub bytes_requested: i32,
    pub quantum_available: bool,
    pub latency_ms: Option<i64>,
    pub correlation_id: Option<Uuid>,
}

/// Response body for GET /v1/magic/entropy/log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyLogResponse {
    pub entries: Vec<EntropyLogEntry>,
    pub limit: i64,
}

/// Response body for GET /v1/magic/config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicConfigResponse {
    pub enabled: bool,
    pub quantum_origin_url: String,
    pub quantum_origin_api_key: String,
    pub quantinuum_enabled: bool,
    pub quantinuum_device: String,
    pub quantinuum_n_bits: u32,
    pub qiskit_enabled: bool,
    pub qiskit_backend: String,
    pub entropy_timeout_ms: u64,
    pub entropy_mix_ratio: f64,
    pub log_entropy_events: bool,
    pub mantra_cooldown_beats: i32,
}

/// Request body for POST /v1/magic/config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicConfigUpdateRequest {
    pub quantum_origin_api_key: Option<String>,
    pub quantinuum_enabled: Option<bool>,
    pub qiskit_enabled: Option<bool>,
}

/// Request body for POST /v1/magic/auth/quantinuum/login.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantinuumLoginRequest {
    pub email: String,
    pub password: String,
}

/// Response body for POST /v1/magic/auth/quantinuum/login.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantinuumLoginResponse {
    pub message: String,
    pub expires_at: String,
}

/// Response body for POST /v1/magic/auth/quantinuum/refresh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantinuumRefreshResponse {
    pub message: String,
    pub token_expiry: String,
}

/// Quantinuum authentication status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantinuumAuthStatus {
    pub authenticated: bool,
    pub expiry: Option<String>,
}

/// Quantum Origin authentication status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumOriginAuthStatus {
    pub configured: bool,
}

/// Response body for GET /v1/magic/auth/status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicAuthStatusResponse {
    pub quantinuum: QuantinuumAuthStatus,
    pub quantum_origin: QuantumOriginAuthStatus,
}

/// Response body for POST /v1/magic/elixirs/rehash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicElixirsRehashResponse {
    pub message: String,
    pub rehashed: i64,
}
