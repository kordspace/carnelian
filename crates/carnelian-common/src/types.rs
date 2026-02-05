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
