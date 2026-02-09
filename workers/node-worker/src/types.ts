/**
 * Shared type definitions for the Carnelian Node Worker.
 *
 * These types mirror the Rust definitions in carnelian-common/src/types.rs
 * and must stay in sync with the TransportMessage enum and its payloads.
 */

// =============================================================================
// INVOKE REQUEST / RESPONSE
// =============================================================================

/** Request to invoke a skill on a worker */
export interface InvokeRequest {
  /** Unique identifier for this execution run (UUID v7) */
  run_id: string;
  /** Name of the skill to invoke */
  skill_name: string;
  /** Input payload for the skill */
  input: unknown;
  /** Timeout in seconds for this invocation */
  timeout_secs: number;
  /** Correlation ID for request tracing */
  correlation_id: string | null;
}

/** Request to cancel a running skill execution */
export interface CancelRequest {
  /** Run ID of the execution to cancel */
  run_id: string;
  /** Reason for cancellation */
  reason: string;
}

/** Status of a skill invocation */
export type InvokeStatus = "Success" | "Failed" | "Timeout" | "Cancelled";

/** Response from a skill invocation */
export interface InvokeResponse {
  /** Run ID this response corresponds to */
  run_id: string;
  /** Outcome status */
  status: InvokeStatus;
  /** Result payload (empty object on failure) */
  result: unknown;
  /** Error message if status is not Success */
  error: string | null;
  /** Process exit code if available */
  exit_code: number | null;
  /** Execution duration in milliseconds */
  duration_ms: number;
  /** Whether the output was truncated due to size limits */
  truncated: boolean;
}

// =============================================================================
// STREAM EVENTS
// =============================================================================

/** Type of stream event emitted during skill execution */
export type StreamEventType = "Log" | "Progress" | "Artifact";

/** Log level for stream events */
export type EventLevel = "Trace" | "Debug" | "Info" | "Warn" | "Error";

/** A streaming event emitted during skill execution */
export interface StreamEvent {
  /** Run ID this event belongs to */
  run_id: string;
  /** Type of stream event */
  event_type: StreamEventType;
  /** When the event was emitted (ISO 8601) */
  timestamp: string;
  /** Log level (relevant for Log events) */
  level: EventLevel | null;
  /** Human-readable message */
  message: string;
  /** Additional structured fields */
  fields: Record<string, unknown>;
}

// =============================================================================
// HEALTH CHECK
// =============================================================================

/** Health check response from a worker transport */
export interface HealthResponse {
  /** Whether the worker is healthy */
  healthy: boolean;
  /** Worker identifier */
  worker_id: string;
  /** Uptime in seconds */
  uptime_secs: number;
}

// =============================================================================
// TRANSPORT MESSAGE ENVELOPE
// =============================================================================

/**
 * Envelope for all transport messages, enabling request/response correlation.
 *
 * Uses discriminated union on `type` field, matching Rust's
 * `#[serde(tag = "type")]` attribute on `TransportMessage`.
 */
export type TransportMessage =
  | { type: "Invoke"; message_id: string; payload: InvokeRequest }
  | { type: "Cancel"; message_id: string; payload: CancelRequest }
  | { type: "Health"; message_id: string }
  | { type: "InvokeResult"; message_id: string; payload: InvokeResponse }
  | { type: "Stream"; message_id: string; payload: StreamEvent }
  | { type: "HealthResult"; message_id: string; payload: HealthResponse };

// =============================================================================
// EXECUTION CONTEXT
// =============================================================================

/** Context for a running skill execution */
export interface ExecutionContext {
  /** Run ID for this execution */
  runId: string;
  /** Skill name being executed */
  skillName: string;
  /** Start time of execution */
  startTime: number;
  /** Timeout deadline (epoch ms) */
  timeoutDeadline: number;
  /** AbortController for cancellation */
  abortController: AbortController;
  /** Correlation ID for tracing */
  correlationId: string | null;
  /** Total output bytes tracked */
  outputBytes: number;
  /** Whether output has been truncated */
  truncated: boolean;
}
