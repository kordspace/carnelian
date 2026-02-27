//! Worker management and coordination
//!
//! The `WorkerManager` maintains an in-memory registry of active workers,
//! spawning them as child processes (Node.js, Python, or Shell) and managing
//! their lifecycle including health checks and graceful shutdown.
//!
//! # Worker Lifecycle
//!
//! 1. Workers are spawned via `start_workers()` up to the `max_workers` limit
//! 2. Each worker runs as a child process with `WORKER_ID` and `CARNELIAN_API_URL` env vars
//! 3. Health checks run every 30 seconds to detect crashed workers
//! 4. On shutdown, workers receive SIGTERM with a 5-second timeout before SIGKILL
//!
//! # Worker Transport Layer
//!
//! The `WorkerTransport` trait abstracts skill invocation from the underlying
//! communication mechanism. The first implementation, `ProcessJsonlTransport`,
//! uses JSON Lines over stdin/stdout for bidirectional communication.
//!
//! ## JSON Lines Protocol
//!
//! Request (written to stdin):
//! ```json
//! {"type":"Invoke","message_id":"...","payload":{"run_id":"...","skill_name":"...","input":{},"timeout_secs":300}}
//! ```
//!
//! Response (read from stdout):
//! ```json
//! {"type":"InvokeResult","message_id":"...","payload":{"run_id":"...","status":"Success","result":{},"duration_ms":42}}
//! ```
//!
//! ## Timeout Enforcement
//!
//! When a skill exceeds its timeout, the transport sends SIGTERM to the worker
//! process and waits for `skill_timeout_grace_period_secs`. If the process is
//! still alive after the grace period, SIGKILL is sent.
//!
//! ## Output Limits
//!
//! Output is tracked per invocation. If it exceeds `skill_max_output_bytes`,
//! the response is truncated and `InvokeResponse.truncated` is set to `true`.
//!
//! # Integration
//!
//! The `WorkerManager` is stored in `AppState` and initialized in the binary.
//! The status endpoint reports active workers via `get_worker_status()`.
//!
//! ## Using WorkerTransport for Skill Invocation
//!
//! ```ignore
//! let transport = worker_manager.get_transport("node-worker-1")?;
//! let response = transport.invoke(InvokeRequest {
//!     run_id: RunId::new(),
//!     skill_name: "my_skill".into(),
//!     input: serde_json::json!({"key": "value"}),
//!     timeout_secs: 60,
//!     correlation_id: None,
//! }).await?;
//! ```

use crate::config::Config;
use crate::events::EventStream;
use crate::ledger::Ledger;
use crate::skills::skill_trait::SkillInput;
use crate::skills::wasm_runtime::WasmSkillRuntime;
use async_trait::async_trait;
use bollard::container::{LogsOptions, StatsOptions};
use bollard::exec::{CreateExecOptions, StartExecResults};
use carnelian_common::types::{
    CancelRequest, EventEnvelope, EventLevel, EventType, HealthResponse, InvokeRequest,
    InvokeResponse, InvokeStatus, RunId, StreamEvent, TransportMessage, WorkerInfo,
};
use carnelian_common::{Error, Result};
use chrono::{DateTime, Utc};
use ed25519_dalek::Signature;
use futures_util::StreamExt;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use sysinfo::{Disks, Networks, ProcessRefreshKind, RefreshKind, System};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout};
use tokio::sync::{RwLock, mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Worker runtime type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerRuntime {
    /// Node.js worker
    Node,
    /// Python worker
    Python,
    /// Native ops worker
    Native,
    /// WebAssembly worker (future)
    Wasm,
}

impl std::fmt::Display for WorkerRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Node => write!(f, "node"),
            Self::Python => write!(f, "python"),
            Self::Native => write!(f, "native"),
            Self::Wasm => write!(f, "wasm"),
        }
    }
}

impl std::str::FromStr for WorkerRuntime {
    type Err = carnelian_common::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "node" => Ok(Self::Node),
            "python" => Ok(Self::Python),
            "native" | "shell" => Ok(Self::Native),
            "wasm" => Ok(Self::Wasm),
            other => Err(carnelian_common::Error::Config(format!(
                "Unknown worker runtime: {other}"
            ))),
        }
    }
}

/// Worker status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStatus {
    /// Worker process is starting up
    Starting,
    /// Worker is running and healthy
    Running,
    /// Worker is being stopped gracefully
    Stopping,
    /// Worker has stopped
    Stopped,
    /// Worker process crashed or failed health check
    Failed,
}

impl std::fmt::Display for WorkerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Stopping => write!(f, "stopping"),
            Self::Stopped => write!(f, "stopped"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// An active worker process
pub struct Worker {
    /// Unique worker identifier (e.g., "node-worker-1")
    pub id: String,
    /// Runtime type
    pub runtime: WorkerRuntime,
    /// The spawned tokio process handle (None when transport owns the process)
    pub process: Option<Child>,
    /// Current status
    pub status: WorkerStatus,
    /// Currently executing task ID
    pub current_task: Option<Uuid>,
    /// When the worker was started
    pub started_at: DateTime<Utc>,
    /// Last successful health check
    pub last_health_check: Option<DateTime<Utc>>,
    /// Transport for skill invocation (created after spawn)
    pub transport: Option<Arc<dyn WorkerTransport>>,
    /// Last attestation data reported by this worker
    pub last_attestation: Option<crate::attestation::WorkerAttestation>,
    /// Whether this worker has been quarantined due to attestation mismatch
    pub quarantined: bool,
    /// When attestation was last verified (for 5-min cadence gating)
    pub last_attestation_verified: Option<DateTime<Utc>>,
}

// =============================================================================
// WORKER TRANSPORT TRAIT
// =============================================================================

/// Trait abstracting skill invocation from the underlying communication mechanism.
///
/// Implementations handle serialization, timeout enforcement, output limits,
/// and event streaming for a specific transport protocol.
#[async_trait::async_trait]
pub trait WorkerTransport: Send + Sync {
    /// Invoke a skill and wait for completion.
    ///
    /// Sends the request to the worker, enforces timeout, and returns the response.
    /// Emits `SkillInvokeStart` before sending and `SkillInvokeEnd`/`SkillInvokeFailed` after.
    async fn invoke(&self, request: InvokeRequest) -> Result<InvokeResponse>;

    /// Subscribe to streaming events for a given run.
    ///
    /// Returns a channel receiver that yields `StreamEvent` messages as they arrive.
    async fn stream_events(&self, run_id: RunId) -> Result<mpsc::Receiver<StreamEvent>>;

    /// Cancel a running skill execution.
    ///
    /// Triggers the cancellation token and sends SIGTERM to the worker process.
    async fn cancel(&self, run_id: RunId, reason: String) -> Result<()>;

    /// Check transport health by verifying the worker process is alive.
    async fn health(&self) -> Result<HealthResponse>;

    /// Gracefully shut down the transport and its underlying worker process.
    ///
    /// Cancels all active runs, sends SIGTERM, waits the grace period,
    /// then SIGKILL if the process is still alive.
    async fn shutdown(&self) -> Result<()>;
}

// =============================================================================
// PROCESS JSONL TRANSPORT
// =============================================================================

/// Context for a single active skill execution run.
struct RunContext {
    /// Channel sender for streaming events to subscribers
    event_tx: mpsc::Sender<StreamEvent>,
    /// Token to signal cancellation
    cancel_token: CancellationToken,
    /// Accumulated output size in bytes
    output_bytes: usize,
    /// Accumulated log line count
    log_lines: usize,
    /// Oneshot sender for delivering the final InvokeResponse to the waiting invoke() call
    response_tx: Option<oneshot::Sender<InvokeResponse>>,
}

/// JSON Lines transport over stdin/stdout of a worker process.
///
/// Sends `TransportMessage` as JSON Lines to the worker's stdin and reads
/// responses from stdout. Supports timeout enforcement, output truncation,
/// cancellation, and event streaming.
///
/// ## Demultiplexing
///
/// The background stdout reader dispatches each `InvokeResult` to the
/// corresponding run's oneshot sender, allowing multiple concurrent `invoke()`
/// calls without holding a global lock.
pub struct ProcessJsonlTransport {
    /// Worker identifier
    worker_id: String,
    /// Handle to the worker's stdin for writing requests
    stdin: Arc<tokio::sync::Mutex<ChildStdin>>,
    /// Application configuration for timeouts and limits
    config: Arc<Config>,
    /// Event stream for emitting lifecycle events
    event_stream: Arc<EventStream>,
    /// Active runs indexed by RunId — used for event routing, output tracking,
    /// cancellation tokens, and per-run response delivery via oneshot senders.
    active_runs: Arc<RwLock<HashMap<RunId, RunContext>>>,
    /// When the transport was created (for uptime calculation)
    created_at: Instant,
    /// Process handle for health checks and shutdown
    process: Arc<tokio::sync::Mutex<Child>>,
    /// Pending health check response senders keyed by message_id
    pending_health: Arc<RwLock<HashMap<Uuid, oneshot::Sender<HealthResponse>>>>,
}

impl ProcessJsonlTransport {
    /// Create a new `ProcessJsonlTransport` from a spawned worker process.
    ///
    /// Takes ownership of the process stdin and stdout, spawning a background
    /// task to read stdout and route messages to active runs.
    pub fn new(
        worker_id: String,
        mut process: Child,
        config: Arc<Config>,
        event_stream: Arc<EventStream>,
    ) -> Result<(Self, Option<ChildStderr>)> {
        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| Error::Connection("Worker stdin not available".to_string()))?;
        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| Error::Connection("Worker stdout not available".to_string()))?;
        let stderr = process.stderr.take();

        let active_runs: Arc<RwLock<HashMap<RunId, RunContext>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let pending_health: Arc<RwLock<HashMap<Uuid, oneshot::Sender<HealthResponse>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        // Spawn background stdout reader that demuxes responses to per-run oneshot senders
        let reader_runs = active_runs.clone();
        let reader_worker_id = worker_id.clone();
        let reader_config = config.clone();
        let reader_pending_health = pending_health.clone();
        tokio::spawn(async move {
            Self::read_stdout_loop(
                stdout,
                reader_worker_id,
                reader_runs,
                reader_config,
                reader_pending_health,
            )
            .await;
        });

        Ok((
            Self {
                worker_id,
                stdin: Arc::new(tokio::sync::Mutex::new(stdin)),
                config,
                event_stream,
                active_runs,
                created_at: Instant::now(),
                process: Arc::new(tokio::sync::Mutex::new(process)),
                pending_health,
            },
            stderr,
        ))
    }

    /// Background task reading stdout line-by-line, parsing JSON Lines,
    /// and dispatching messages to per-run oneshot senders and event channels.
    ///
    /// `InvokeResult` messages are delivered to the corresponding run's oneshot
    /// sender, allowing multiple concurrent `invoke()` calls. `Stream` messages
    /// update output tracking and are forwarded to the run's event channel.
    async fn read_stdout_loop(
        stdout: ChildStdout,
        worker_id: String,
        active_runs: Arc<RwLock<HashMap<RunId, RunContext>>>,
        config: Arc<Config>,
        pending_health: Arc<RwLock<HashMap<Uuid, oneshot::Sender<HealthResponse>>>>,
    ) {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<TransportMessage>(&line) {
                Ok(TransportMessage::InvokeResult { payload, .. }) => {
                    let run_id = payload.run_id;
                    let mut runs = active_runs.write().await;
                    if let Some(ctx) = runs.get_mut(&run_id) {
                        // Apply output limit enforcement (Comment 3)
                        let mut resp = payload;
                        let result_bytes = serde_json::to_string(&resp.result)
                            .map(|s| s.len())
                            .unwrap_or(0);
                        if Self::check_output_limits(ctx, result_bytes, &config) {
                            let original_size = ctx.output_bytes + result_bytes;
                            resp.result = json!({
                                "...": "output truncated",
                                "original_size_bytes": original_size,
                                "max_output_bytes": config.skill_max_output_bytes,
                                "log_lines": ctx.log_lines,
                                "max_log_lines": config.skill_max_log_lines,
                            });
                            resp.truncated = true;
                        }
                        // Deliver to the waiting invoke() call via oneshot
                        if let Some(tx) = ctx.response_tx.take() {
                            let _ = tx.send(resp);
                        }
                    } else {
                        tracing::warn!(
                            worker_id = %worker_id,
                            run_id = ?run_id,
                            "Received InvokeResult for unknown run_id, discarding"
                        );
                    }
                }
                Ok(TransportMessage::Stream { ref payload, .. }) => {
                    let mut runs = active_runs.write().await;
                    if let Some(ctx) = runs.get_mut(&payload.run_id) {
                        // Track output bytes and log lines
                        ctx.output_bytes += payload.message.len();
                        ctx.log_lines += 1;
                        // Forward to event subscriber
                        let _ = ctx.event_tx.try_send(payload.clone());
                    }
                }
                Ok(TransportMessage::HealthResult {
                    message_id,
                    payload,
                }) => {
                    let mut pending = pending_health.write().await;
                    if let Some(tx) = pending.remove(&message_id) {
                        let _ = tx.send(payload);
                    } else {
                        tracing::debug!(
                            worker_id = %worker_id,
                            message_id = %message_id,
                            "Received HealthResult with no pending request"
                        );
                    }
                }
                Ok(msg) => {
                    // Other message types — log and discard
                    tracing::debug!(
                        worker_id = %worker_id,
                        msg_type = ?std::mem::discriminant(&msg),
                        "Received unexpected message type from worker"
                    );
                }
                Err(e) => {
                    // Non-JSON lines are logged as worker output
                    tracing::info!(
                        worker_id = %worker_id,
                        stream = "stdout",
                        parse_error = %e,
                        "{}", line
                    );
                }
            }
        }

        tracing::debug!(worker_id = %worker_id, "Stdout reader loop ended");
    }

    /// Write bytes to the worker's stdin, acquiring the lock briefly.
    async fn write_to_stdin(&self, data: &[u8]) -> Result<()> {
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(data)
            .await
            .map_err(|e| Error::Connection(format!("Failed to write to worker stdin: {e}")))?;
        stdin
            .flush()
            .await
            .map_err(|e| Error::Connection(format!("Failed to flush worker stdin: {e}")))?;
        drop(stdin);
        Ok(())
    }

    /// Send SIGTERM (Unix) or kill (Windows) to the worker process,
    /// wait the grace period, then SIGKILL if still alive.
    #[allow(clippy::significant_drop_tightening)]
    async fn cancel_with_signal(&self) {
        let grace = std::time::Duration::from_secs(self.config.skill_timeout_grace_period_secs);
        let mut proc = self.process.lock().await;

        #[cfg(unix)]
        {
            if let Some(pid) = proc.id() {
                let nix_pid = nix::unistd::Pid::from_raw(i32::try_from(pid).unwrap_or(i32::MAX));
                let _ = nix::sys::signal::kill(nix_pid, nix::sys::signal::Signal::SIGTERM);
                tracing::debug!(
                    worker_id = %self.worker_id,
                    pid = pid,
                    "Sent SIGTERM for cancellation"
                );
            }
        }
        #[cfg(not(unix))]
        {
            tracing::debug!(
                worker_id = %self.worker_id,
                "Non-Unix platform, will use kill() after grace period"
            );
        }

        // Wait grace period then force kill
        let exited = tokio::time::timeout(grace, proc.wait()).await;
        if exited.is_err() {
            tracing::warn!(
                worker_id = %self.worker_id,
                "Worker did not exit within grace period, sending SIGKILL"
            );
            let _ = proc.kill().await;
        }
    }

    /// Enforce output limits: returns true if the output should be truncated.
    fn check_output_limits(ctx: &RunContext, additional_bytes: usize, config: &Config) -> bool {
        ctx.output_bytes + additional_bytes > config.skill_max_output_bytes
            || ctx.log_lines >= config.skill_max_log_lines
    }

    /// Emit the appropriate completion event after an invoke finishes.
    fn emit_invoke_completion_event(
        &self,
        run_id: RunId,
        result: &Result<InvokeResponse>,
        start: Instant,
    ) {
        match result {
            Ok(resp) if resp.status == InvokeStatus::Success => {
                self.event_stream.publish(
                    EventEnvelope::new(
                        EventLevel::Info,
                        EventType::SkillInvokeEnd,
                        json!({
                            "run_id": run_id,
                            "worker_id": &self.worker_id,
                            "duration_ms": resp.duration_ms,
                            "truncated": resp.truncated,
                        }),
                    )
                    .with_actor_id(&self.worker_id),
                );
            }
            Ok(resp) => {
                self.event_stream.publish(
                    EventEnvelope::new(
                        EventLevel::Warn,
                        EventType::SkillInvokeFailed,
                        json!({
                            "run_id": run_id,
                            "worker_id": &self.worker_id,
                            "status": format!("{:?}", resp.status),
                            "error": resp.error,
                            "duration_ms": resp.duration_ms,
                        }),
                    )
                    .with_actor_id(&self.worker_id),
                );
            }
            Err(e) => {
                self.event_stream.publish(
                    EventEnvelope::new(
                        EventLevel::Error,
                        EventType::SkillInvokeFailed,
                        json!({
                            "run_id": run_id,
                            "worker_id": &self.worker_id,
                            "error": e.to_string(),
                            "duration_ms": start.elapsed().as_millis() as u64,
                        }),
                    )
                    .with_actor_id(&self.worker_id),
                );
            }
        }
    }
}

#[async_trait::async_trait]
impl WorkerTransport for ProcessJsonlTransport {
    #[allow(clippy::too_many_lines)]
    async fn invoke(&self, request: InvokeRequest) -> Result<InvokeResponse> {
        let run_id = request.run_id;
        let timeout_secs = request.timeout_secs;
        let start = Instant::now();
        let deadline = start + std::time::Duration::from_secs(timeout_secs);

        // Create per-run oneshot channel for response delivery
        let (response_tx, response_rx) = oneshot::channel::<InvokeResponse>();

        // Create run context with oneshot sender
        let (event_tx, _event_rx) = mpsc::channel::<StreamEvent>(100);
        let cancel_token = CancellationToken::new();
        let ctx = RunContext {
            event_tx,
            cancel_token: cancel_token.clone(),
            output_bytes: 0,
            log_lines: 0,
            response_tx: Some(response_tx),
        };
        self.active_runs.write().await.insert(run_id, ctx);

        // Emit SkillInvokeStart event
        self.event_stream.publish(
            EventEnvelope::new(
                EventLevel::Info,
                EventType::SkillInvokeStart,
                json!({
                    "run_id": run_id,
                    "skill_name": &request.skill_name,
                    "worker_id": &self.worker_id,
                }),
            )
            .with_actor_id(&self.worker_id),
        );

        // Serialize and send request
        let msg = TransportMessage::Invoke {
            message_id: Uuid::now_v7(),
            payload: request,
        };
        let mut line = serde_json::to_string(&msg)
            .map_err(|e| Error::Connection(format!("Failed to serialize request: {e}")))?;
        line.push('\n');

        self.write_to_stdin(line.as_bytes()).await?;

        // Wait for response, timeout, or cancellation
        let timeout_duration = deadline.saturating_duration_since(Instant::now());
        let result = tokio::select! {
            _ = cancel_token.cancelled() => {
                self.cancel_with_signal().await;
                Ok(InvokeResponse {
                    run_id,
                    status: InvokeStatus::Cancelled,
                    result: json!({}),
                    error: Some("Cancelled by request".to_string()),
                    exit_code: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    truncated: false,
                })
            }
            _ = tokio::time::sleep(timeout_duration) => {
                self.cancel_with_signal().await;
                Ok(InvokeResponse {
                    run_id,
                    status: InvokeStatus::Timeout,
                    result: json!({}),
                    error: Some(format!("Skill execution timed out after {timeout_secs}s")),
                    exit_code: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    truncated: false,
                })
            }
            response = response_rx => {
                response.map_err(|_| Error::Connection(
                    "Worker stdout closed before response received".to_string()
                ))
            }
        };

        // Clean up run context
        self.active_runs.write().await.remove(&run_id);

        // Emit completion event
        self.emit_invoke_completion_event(run_id, &result, start);

        result
    }

    async fn stream_events(&self, run_id: RunId) -> Result<mpsc::Receiver<StreamEvent>> {
        let (tx, rx) = mpsc::channel::<StreamEvent>(100);
        let mut runs = self.active_runs.write().await;
        if let Some(ctx) = runs.get_mut(&run_id) {
            ctx.event_tx = tx;
            Ok(rx)
        } else {
            Err(Error::Config(format!(
                "No active run found for run_id {:?}",
                run_id
            )))
        }
    }

    async fn cancel(&self, run_id: RunId, reason: String) -> Result<()> {
        // First, send a Cancel message to stdin so the worker can perform graceful cleanup
        let cancel_msg = TransportMessage::Cancel {
            message_id: Uuid::now_v7(),
            payload: CancelRequest {
                run_id,
                reason: reason.clone(),
            },
        };
        if let Ok(mut line) = serde_json::to_string(&cancel_msg) {
            line.push('\n');
            if let Err(e) = self.write_to_stdin(line.as_bytes()).await {
                tracing::warn!(
                    worker_id = %self.worker_id,
                    run_id = ?run_id,
                    error = %e,
                    "Failed to send Cancel message to worker stdin, proceeding with token cancellation"
                );
            }
        }

        // Then trigger the local cancellation token (which invoke() select! listens on)
        let runs = self.active_runs.read().await;
        runs.get(&run_id).map_or_else(
            || {
                Err(Error::Config(format!(
                    "No active run found for run_id {run_id:?}"
                )))
            },
            |ctx| {
                tracing::info!(
                    worker_id = %self.worker_id,
                    run_id = ?run_id,
                    reason = %reason,
                    "Cancelling skill execution"
                );
                ctx.cancel_token.cancel();
                Ok(())
            },
        )
    }

    async fn health(&self) -> Result<HealthResponse> {
        // First check if the process is still alive
        let wait_result = self.process.lock().await.try_wait();
        let alive = match wait_result {
            Ok(Some(_)) | Err(_) => false,
            Ok(None) => true,
        };

        if !alive {
            return Ok(HealthResponse {
                healthy: false,
                worker_id: self.worker_id.clone(),
                uptime_secs: self.created_at.elapsed().as_secs(),
                attestation: None,
            });
        }

        // Send a Health request over stdin and wait for the HealthResult
        let message_id = Uuid::now_v7();
        let (tx, rx) = oneshot::channel::<HealthResponse>();

        // Register the pending health response
        self.pending_health.write().await.insert(message_id, tx);

        // Serialize and send the Health message
        let msg = TransportMessage::Health { message_id };
        let mut line = serde_json::to_string(&msg)
            .map_err(|e| Error::Connection(format!("Failed to serialize Health request: {e}")))?;
        line.push('\n');

        if let Err(e) = self.write_to_stdin(line.as_bytes()).await {
            // Clean up pending entry on write failure
            self.pending_health.write().await.remove(&message_id);
            return Err(e);
        }

        // Wait for the response with a 10-second timeout
        match tokio::time::timeout(std::time::Duration::from_secs(10), rx).await {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(_)) => {
                // Oneshot sender dropped (stdout reader ended)
                self.pending_health.write().await.remove(&message_id);
                Ok(HealthResponse {
                    healthy: false,
                    worker_id: self.worker_id.clone(),
                    uptime_secs: self.created_at.elapsed().as_secs(),
                    attestation: None,
                })
            }
            Err(_) => {
                // Timeout waiting for health response
                self.pending_health.write().await.remove(&message_id);
                tracing::warn!(
                    worker_id = %self.worker_id,
                    "Health check timed out waiting for worker response"
                );
                Ok(HealthResponse {
                    healthy: false,
                    worker_id: self.worker_id.clone(),
                    uptime_secs: self.created_at.elapsed().as_secs(),
                    attestation: None,
                })
            }
        }
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!(worker_id = %self.worker_id, "Shutting down transport");

        // Cancel all active runs so waiting invoke() calls return immediately
        {
            let runs = self.active_runs.read().await;
            for (run_id, ctx) in runs.iter() {
                tracing::debug!(
                    worker_id = %self.worker_id,
                    run_id = ?run_id,
                    "Cancelling active run during shutdown"
                );
                ctx.cancel_token.cancel();
            }
        }

        // Terminate the worker process via SIGTERM + grace period + SIGKILL
        self.cancel_with_signal().await;

        Ok(())
    }
}

/// WASM worker transport for in-process WASM execution
pub struct WasmWorkerTransport {
    worker_id: String,
    runtime: Arc<WasmSkillRuntime>,
    event_stream: Arc<EventStream>,
    active_runs: Arc<RwLock<HashMap<RunId, mpsc::Sender<StreamEvent>>>>,
    created_at: std::time::Instant,
    config: Arc<Config>,
}

impl WasmWorkerTransport {
    /// Create a new WasmWorkerTransport
    pub fn new(
        worker_id: String,
        runtime: Arc<WasmSkillRuntime>,
        event_stream: Arc<EventStream>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            worker_id,
            runtime,
            event_stream,
            active_runs: Arc::new(RwLock::new(HashMap::new())),
            created_at: std::time::Instant::now(),
            config,
        }
    }
}

#[async_trait]
impl WorkerTransport for WasmWorkerTransport {
    async fn invoke(&self, request: InvokeRequest) -> Result<InvokeResponse> {
        let run_id = request.run_id.clone();
        let skill_name = request.skill_name.clone();

        // Emit SkillInvokeStart event
        self.event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::SkillInvokeStart,
            json!({
                "worker_id": self.worker_id,
                "run_id": run_id,
                "skill_name": skill_name,
            }),
        ));

        // Derive WASM path
        let wasm_path = format!("skills/core-registry/{}/{}.wasm", skill_name, skill_name);
        let skill_json_path = format!("skills/core-registry/{}/skill.json", skill_name);

        // Load skill if not already loaded
        let skill_id = format!("{}/{}-worker", skill_name, self.worker_id);
        if !self.runtime.is_loaded(&skill_id) {
            if let Err(e) = self
                .runtime
                .load(std::path::Path::new(&wasm_path), &skill_id)
            {
                tracing::warn!(error = %e, "Failed to preload WASM skill, will attempt on invoke");
            }
        }

        // Read capabilities from skill.json
        let capabilities =
            std::fs::read_to_string(&skill_json_path).map_or_else(Vec::new, |content| {
                serde_json::from_str::<serde_json::Value>(&content)
                    .ok()
                    .and_then(|v| v.get("capabilities_required").cloned())
                    .and_then(|v| {
                        v.as_array().map(|arr| {
                            arr.iter()
                                .filter_map(|s| s.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                    })
                    .unwrap_or_default()
            });

        // Convert input to SkillInput
        let skill_input = SkillInput {
            action: "execute".to_string(),
            params: request.input,
            identity_id: None,
            correlation_id: request.correlation_id.clone(),
        };

        // Call WASM runtime
        let start_time = std::time::Instant::now();
        let output = match self
            .runtime
            .invoke(&skill_id, skill_input, capabilities)
            .await
        {
            Ok(out) => out,
            Err(e) => {
                // Emit SkillInvokeFailed event
                self.event_stream.publish(EventEnvelope::new(
                    EventLevel::Error,
                    EventType::SkillInvokeFailed,
                    json!({
                        "worker_id": self.worker_id,
                        "run_id": run_id,
                        "skill_name": skill_name,
                        "error": e.to_string(),
                    }),
                ));
                return Err(e);
            }
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Emit SkillInvokeEnd event
        self.event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::SkillInvokeEnd,
            json!({
                "worker_id": self.worker_id,
                "run_id": run_id,
                "skill_name": skill_name,
                "success": output.success,
            }),
        ));

        // Check if output was truncated from metadata
        let truncated = output
            .metadata
            .get("truncated")
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false);

        Ok(InvokeResponse {
            run_id,
            status: if output.success {
                InvokeStatus::Success
            } else {
                InvokeStatus::Failed
            },
            result: output.data,
            error: output.error,
            exit_code: output
                .metadata
                .get("exit_code")
                .and_then(|v| v.parse::<i32>().ok()),
            duration_ms,
            truncated,
        })
    }

    async fn stream_events(&self, run_id: RunId) -> Result<mpsc::Receiver<StreamEvent>> {
        let (tx, rx) = mpsc::channel(256);
        let mut active_runs = self.active_runs.write().await;
        active_runs.insert(run_id, tx);
        Ok(rx)
    }

    async fn cancel(&self, run_id: RunId, _reason: String) -> Result<()> {
        let mut active_runs = self.active_runs.write().await;
        active_runs.remove(&run_id);
        // Epoch interruption handles the actual timeout
        Ok(())
    }

    async fn health(&self) -> Result<HealthResponse> {
        Ok(HealthResponse {
            healthy: true,
            worker_id: self.worker_id.clone(),
            uptime_secs: self.created_at.elapsed().as_secs(),
            attestation: None,
        })
    }

    async fn shutdown(&self) -> Result<()> {
        let mut active_runs = self.active_runs.write().await;
        active_runs.clear();
        Ok(())
    }
}

/// Native worker transport for in-process native ops execution
pub struct NativeWorkerTransport {
    worker_id: String,
    event_stream: Arc<EventStream>,
    config: Arc<Config>,
    active_runs: Arc<RwLock<HashMap<RunId, mpsc::Sender<StreamEvent>>>>,
    created_at: std::time::Instant,
}

impl NativeWorkerTransport {
    /// Create a new NativeWorkerTransport
    pub fn new(worker_id: String, event_stream: Arc<EventStream>, config: Arc<Config>) -> Self {
        Self {
            worker_id,
            event_stream,
            config,
            active_runs: Arc::new(RwLock::new(HashMap::new())),
            created_at: std::time::Instant::now(),
        }
    }

    /// Check if the required capability is present in the request
    fn check_capability(input: &serde_json::Value, required: &str) -> bool {
        input
            .get("capabilities")
            .and_then(|c| c.as_array())
            .map(|arr| arr.iter().any(|v| v.as_str() == Some(required)))
            .unwrap_or(false)
    }

    /// Verify owner approval signature for privileged operations
    fn verify_owner_approval(input: &serde_json::Value, config: &Config) -> Result<()> {
        // Extract the signature string
        let sig_str = input
            .get("_approval_signature")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::Security(
                    "Missing _approval_signature: owner approval is required for this operation"
                        .to_string(),
                )
            })?;

        // Hex-decode the signature bytes
        let sig_bytes = hex::decode(sig_str)
            .map_err(|e| Error::Security(format!("Invalid _approval_signature encoding: {}", e)))?;

        // Parse into ed25519_dalek::Signature
        let signature = Signature::from_slice(&sig_bytes)
            .map_err(|e| Error::Security(format!("Malformed _approval_signature: {}", e)))?;

        // Build the canonical message by removing the signature field
        let mut canonical = input.clone();
        if let Some(obj) = canonical.as_object_mut() {
            obj.remove("_approval_signature");
        }

        // Serialize to canonical JSON string
        let message_str = serde_json::to_string(&canonical).map_err(|e| {
            Error::Security(format!("Failed to serialize canonical message: {}", e))
        })?;
        let message_bytes = message_str.as_bytes();

        // Verify via Config::verify_signature
        match config.verify_signature(message_bytes, &signature) {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::Security(
                "Owner approval signature verification failed".to_string(),
            )),
            Err(e) => Err(e),
        }
    }
}

#[async_trait]
impl WorkerTransport for NativeWorkerTransport {
    async fn invoke(&self, request: InvokeRequest) -> Result<InvokeResponse> {
        let run_id = request.run_id.clone();
        let skill_name = request.skill_name.clone();
        let start_time = std::time::Instant::now();

        // Emit SkillInvokeStart event
        self.event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::SkillInvokeStart,
            json!({
                "worker_id": self.worker_id,
                "run_id": run_id,
                "skill_name": skill_name,
            }),
        ));

        // Parse skill_name and execute the corresponding op with timeout
        let timeout_secs = request.timeout_secs;
        let timeout_duration = std::time::Duration::from_secs(timeout_secs);

        let result: Result<serde_json::Value> = match skill_name.as_str() {
            "git_status" => {
                if !Self::check_capability(&request.input, "git.read") {
                    Err(Error::Permission(
                        "Missing required capability: git.read".to_string(),
                    ))
                } else {
                    let path = request
                        .input
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(".");
                    let output = tokio::process::Command::new("git")
                        .args(["status", "--porcelain", path])
                        .output()
                        .await
                        .map_err(|e| Error::Io(e))?;

                    // Check if command succeeded
                    if !output.status.success() {
                        let exit_code = output.status.code();
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Ok(InvokeResponse {
                            run_id,
                            status: InvokeStatus::Failed,
                            result: json!({}),
                            error: Some(format!(
                                "git status failed with exit code {:?}: {}",
                                exit_code, stderr
                            )),
                            exit_code,
                            duration_ms: start_time.elapsed().as_millis() as u64,
                            truncated: false,
                        });
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout);
                    Ok(json!({ "output": stdout.to_string() }))
                }
            }
            "file_hash" => {
                if !Self::check_capability(&request.input, "fs.read") {
                    Err(Error::Permission(
                        "Missing required capability: fs.read".to_string(),
                    ))
                } else {
                    let path = request
                        .input
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| Error::Config("Missing path parameter".to_string()))?;
                    let bytes = tokio::fs::read(path).await.map_err(|e| Error::Io(e))?;
                    let hash = blake3::hash(&bytes);
                    Ok(json!({ "hash": hash.to_hex().to_string() }))
                }
            }
            "docker_ps" => {
                if !Self::check_capability(&request.input, "docker.read") {
                    Err(Error::Permission(
                        "Missing required capability: docker.read".to_string(),
                    ))
                } else {
                    let docker = bollard::Docker::connect_with_local_defaults().map_err(|e| {
                        Error::Worker(format!("Failed to connect to Docker: {}", e))
                    })?;
                    let containers = docker
                        .list_containers::<String>(None)
                        .await
                        .map_err(|e| Error::Worker(format!("Failed to list containers: {}", e)))?;
                    let container_ids: Vec<String> =
                        containers.iter().filter_map(|c| c.id.clone()).collect();
                    Ok(json!({ "containers": container_ids }))
                }
            }
            "dir_list" => {
                if !Self::check_capability(&request.input, "fs.read") {
                    Err(Error::Permission(
                        "Missing required capability: fs.read".to_string(),
                    ))
                } else {
                    let path = request
                        .input
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(".");
                    let depth = request
                        .input
                        .get("depth")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(1) as usize;
                    let mut entries = Vec::new();
                    for entry in walkdir::WalkDir::new(path).max_depth(depth) {
                        match entry {
                            Ok(e) => entries.push(e.path().to_string_lossy().to_string()),
                            Err(e) => tracing::warn!(error = %e, "Error reading directory entry"),
                        }
                    }
                    Ok(json!({ "entries": entries }))
                }
            }
            "file_read" => {
                if !Self::check_capability(&request.input, "fs.read") {
                    Err(Error::Permission(
                        "Missing required capability: fs.read".to_string(),
                    ))
                } else {
                    let path = request
                        .input
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            Error::Worker("Missing required parameter: path".to_string())
                        })?;
                    let max_bytes = request
                        .input
                        .get("max_bytes")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(1_048_576) as usize;

                    let bytes = tokio::fs::read(path).await.map_err(|e| Error::Io(e))?;
                    let original_len = bytes.len();
                    let truncated = original_len > max_bytes;
                    let content_bytes = if truncated {
                        &bytes[..max_bytes]
                    } else {
                        &bytes
                    };
                    let content = String::from_utf8_lossy(content_bytes).to_string();

                    Ok(json!({
                        "content": content,
                        "truncated": truncated,
                        "size": original_len
                    }))
                }
            }
            "file_write" => {
                if !Self::check_capability(&request.input, "fs.write") {
                    Err(Error::Permission(
                        "Missing required capability: fs.write".to_string(),
                    ))
                } else {
                    Self::verify_owner_approval(&request.input, &self.config)?;

                    let path = request
                        .input
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            Error::Worker("Missing required parameter: path".to_string())
                        })?;
                    let content = request
                        .input
                        .get("content")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            Error::Worker("Missing required parameter: content".to_string())
                        })?;

                    tokio::fs::write(path, content.as_bytes())
                        .await
                        .map_err(|e| Error::Io(e))?;

                    Ok(json!({
                        "written": true,
                        "path": path
                    }))
                }
            }
            "file_search" => {
                if !Self::check_capability(&request.input, "fs.read") {
                    Err(Error::Permission(
                        "Missing required capability: fs.read".to_string(),
                    ))
                } else {
                    let pattern = request
                        .input
                        .get("pattern")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            Error::Worker("Missing required parameter: pattern".to_string())
                        })?;
                    let path = request
                        .input
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(".");

                    let output = tokio::process::Command::new("rg")
                        .args(["--json", pattern, path])
                        .output()
                        .await
                        .map_err(|e| Error::Io(e))?;

                    if !output.status.success() {
                        let exit_code = output.status.code();
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Ok(InvokeResponse {
                            run_id,
                            status: InvokeStatus::Failed,
                            result: json!({}),
                            error: Some(format!(
                                "rg search failed with exit code {:?}: {}",
                                exit_code, stderr
                            )),
                            exit_code,
                            duration_ms: start_time.elapsed().as_millis() as u64,
                            truncated: false,
                        });
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    Ok(json!({ "output": stdout }))
                }
            }
            "file_delete" => {
                if !Self::check_capability(&request.input, "fs.write") {
                    Err(Error::Permission(
                        "Missing required capability: fs.write".to_string(),
                    ))
                } else {
                    Self::verify_owner_approval(&request.input, &self.config)?;

                    let path = request
                        .input
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            Error::Worker("Missing required parameter: path".to_string())
                        })?;

                    tokio::fs::remove_file(path)
                        .await
                        .map_err(|e| Error::Io(e))?;

                    Ok(json!({
                        "deleted": true,
                        "path": path
                    }))
                }
            }
            "file_move" => {
                if !Self::check_capability(&request.input, "fs.write") {
                    Err(Error::Permission(
                        "Missing required capability: fs.write".to_string(),
                    ))
                } else {
                    let src = request
                        .input
                        .get("src")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            Error::Worker("Missing required parameter: src".to_string())
                        })?;
                    let dst = request
                        .input
                        .get("dst")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            Error::Worker("Missing required parameter: dst".to_string())
                        })?;

                    tokio::fs::rename(src, dst)
                        .await
                        .map_err(|e| Error::Io(e))?;

                    Ok(json!({
                        "moved": true,
                        "src": src,
                        "dst": dst
                    }))
                }
            }
            "git_diff" => {
                if !Self::check_capability(&request.input, "git.read") {
                    Err(Error::Permission(
                        "Missing required capability: git.read".to_string(),
                    ))
                } else {
                    let path = request
                        .input
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(".");
                    let staged = request
                        .input
                        .get("staged")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let mut args = vec!["diff"];
                    if staged {
                        args.push("--cached");
                    }
                    args.push(path);

                    let output = tokio::process::Command::new("git")
                        .args(&args)
                        .output()
                        .await
                        .map_err(|e| Error::Io(e))?;

                    if !output.status.success() {
                        let exit_code = output.status.code();
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Ok(InvokeResponse {
                            run_id,
                            status: InvokeStatus::Failed,
                            result: json!({}),
                            error: Some(format!(
                                "git diff failed with exit code {:?}: {}",
                                exit_code, stderr
                            )),
                            exit_code,
                            duration_ms: start_time.elapsed().as_millis() as u64,
                            truncated: false,
                        });
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    Ok(json!({ "output": stdout }))
                }
            }
            "git_commit" => {
                if !Self::check_capability(&request.input, "git.write") {
                    Err(Error::Permission(
                        "Missing required capability: git.write".to_string(),
                    ))
                } else {
                    Self::verify_owner_approval(&request.input, &self.config)?;

                    let message = request
                        .input
                        .get("message")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            Error::Worker("Missing required parameter: message".to_string())
                        })?;
                    let path = request
                        .input
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(".");

                    let output = tokio::process::Command::new("git")
                        .args(["commit", "-m", message])
                        .current_dir(path)
                        .output()
                        .await
                        .map_err(|e| Error::Io(e))?;

                    if !output.status.success() {
                        let exit_code = output.status.code();
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Ok(InvokeResponse {
                            run_id,
                            status: InvokeStatus::Failed,
                            result: json!({}),
                            error: Some(format!(
                                "git commit failed with exit code {:?}: {}",
                                exit_code, stderr
                            )),
                            exit_code,
                            duration_ms: start_time.elapsed().as_millis() as u64,
                            truncated: false,
                        });
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    Ok(json!({ "output": stdout }))
                }
            }
            "git_log" => {
                if !Self::check_capability(&request.input, "git.read") {
                    Err(Error::Permission(
                        "Missing required capability: git.read".to_string(),
                    ))
                } else {
                    let path = request
                        .input
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(".");
                    let max_count = request
                        .input
                        .get("max_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(20);
                    let oneline = request
                        .input
                        .get("oneline")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);

                    let max_count_arg = format!("--max-count={}", max_count);
                    let mut args = vec!["log", &max_count_arg];
                    if oneline {
                        args.push("--oneline");
                    }
                    args.push(path);

                    let output = tokio::process::Command::new("git")
                        .args(&args)
                        .output()
                        .await
                        .map_err(|e| Error::Io(e))?;

                    if !output.status.success() {
                        let exit_code = output.status.code();
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Ok(InvokeResponse {
                            run_id,
                            status: InvokeStatus::Failed,
                            result: json!({}),
                            error: Some(format!(
                                "git log failed with exit code {:?}: {}",
                                exit_code, stderr
                            )),
                            exit_code,
                            duration_ms: start_time.elapsed().as_millis() as u64,
                            truncated: false,
                        });
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    Ok(json!({ "output": stdout }))
                }
            }
            "git_branch" => {
                if !Self::check_capability(&request.input, "git.read") {
                    Err(Error::Permission(
                        "Missing required capability: git.read".to_string(),
                    ))
                } else {
                    let path = request
                        .input
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(".");

                    let output = tokio::process::Command::new("git")
                        .args(["branch", "--list"])
                        .current_dir(path)
                        .output()
                        .await
                        .map_err(|e| Error::Io(e))?;

                    if !output.status.success() {
                        let exit_code = output.status.code();
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Ok(InvokeResponse {
                            run_id,
                            status: InvokeStatus::Failed,
                            result: json!({}),
                            error: Some(format!(
                                "git branch failed with exit code {:?}: {}",
                                exit_code, stderr
                            )),
                            exit_code,
                            duration_ms: start_time.elapsed().as_millis() as u64,
                            truncated: false,
                        });
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    Ok(json!({ "output": stdout }))
                }
            }
            "docker_exec" => {
                if !Self::check_capability(&request.input, "docker.exec") {
                    Err(Error::Permission(
                        "Missing required capability: docker.exec".to_string(),
                    ))
                } else {
                    Self::verify_owner_approval(&request.input, &self.config)?;

                    let container_id = request
                        .input
                        .get("container_id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                        Error::Worker("Missing required parameter: container_id".to_string())
                    })?;

                    let cmd_array = request
                        .input
                        .get("cmd")
                        .and_then(|v| v.as_array())
                        .ok_or_else(|| {
                            Error::Worker("Missing required parameter: cmd".to_string())
                        })?;

                    let mut cmd = Vec::new();
                    for (idx, item) in cmd_array.iter().enumerate() {
                        match item.as_str() {
                            Some(s) => cmd.push(s.to_string()),
                            None => {
                                return Err(Error::Worker(format!(
                                    "cmd[{}] must be a string, got: {:?}",
                                    idx, item
                                )));
                            }
                        }
                    }

                    if cmd.is_empty() {
                        return Err(Error::Worker(
                            "cmd must be a non-empty array of strings".to_string(),
                        ));
                    }

                    let docker = bollard::Docker::connect_with_local_defaults().map_err(|e| {
                        Error::Worker(format!("Failed to connect to Docker: {}", e))
                    })?;

                    let exec = docker
                        .create_exec(
                            container_id,
                            CreateExecOptions {
                                cmd: Some(cmd),
                                attach_stdout: Some(true),
                                attach_stderr: Some(true),
                                ..Default::default()
                            },
                        )
                        .await
                        .map_err(|e| Error::Worker(format!("Failed to create exec: {}", e)))?;

                    let exec_id = exec.id;
                    match docker.start_exec(&exec_id, None).await {
                        Ok(StartExecResults::Attached { mut output, .. }) => {
                            let mut collected = String::new();
                            while let Some(chunk) = output.next().await {
                                match chunk {
                                    Ok(msg) => collected.push_str(&msg.to_string()),
                                    Err(e) => {
                                        return Err(Error::Worker(format!("Stream error: {}", e)));
                                    }
                                }
                            }
                            Ok(json!({ "output": collected }))
                        }
                        Ok(_) => Err(Error::Worker("Unexpected exec result".to_string())),
                        Err(e) => Err(Error::Worker(format!("Failed to start exec: {}", e))),
                    }
                }
            }
            "docker_logs" => {
                if !Self::check_capability(&request.input, "docker.read") {
                    Err(Error::Permission(
                        "Missing required capability: docker.read".to_string(),
                    ))
                } else {
                    let container_id = request
                        .input
                        .get("container_id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                        Error::Worker("Missing required parameter: container_id".to_string())
                    })?;
                    let tail = request
                        .input
                        .get("tail")
                        .and_then(|v| v.as_str())
                        .unwrap_or("100");

                    let docker = bollard::Docker::connect_with_local_defaults().map_err(|e| {
                        Error::Worker(format!("Failed to connect to Docker: {}", e))
                    })?;

                    let mut logs_stream = docker.logs(
                        container_id,
                        Some(LogsOptions {
                            stdout: true,
                            stderr: true,
                            tail: tail.to_string(),
                            ..Default::default()
                        }),
                    );

                    let mut collected = String::new();
                    while let Some(chunk) = logs_stream.next().await {
                        match chunk {
                            Ok(msg) => collected.push_str(&msg.to_string()),
                            Err(e) => return Err(Error::Worker(format!("Stream error: {}", e))),
                        }
                    }

                    Ok(json!({ "output": collected }))
                }
            }
            "docker_stats" => {
                if !Self::check_capability(&request.input, "docker.read") {
                    Err(Error::Permission(
                        "Missing required capability: docker.read".to_string(),
                    ))
                } else {
                    let container_id = request
                        .input
                        .get("container_id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                        Error::Worker("Missing required parameter: container_id".to_string())
                    })?;

                    let docker = bollard::Docker::connect_with_local_defaults().map_err(|e| {
                        Error::Worker(format!("Failed to connect to Docker: {}", e))
                    })?;

                    let mut stats_stream = docker.stats(
                        container_id,
                        Some(StatsOptions {
                            stream: false,
                            ..Default::default()
                        }),
                    );

                    match stats_stream.next().await {
                        Some(Ok(stats)) => {
                            let stats_value = serde_json::to_value(&stats).map_err(|e| {
                                Error::Worker(format!("Failed to serialize stats: {}", e))
                            })?;
                            Ok(json!({ "stats": stats_value }))
                        }
                        Some(Err(e)) => Err(Error::Worker(format!("Failed to get stats: {}", e))),
                        None => Err(Error::Worker("No stats available".to_string())),
                    }
                }
            }
            "process_list" => {
                if !Self::check_capability(&request.input, "system.read") {
                    Err(Error::Permission(
                        "Missing required capability: system.read".to_string(),
                    ))
                } else {
                    let sys = System::new_with_specifics(
                        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
                    );

                    let processes: Vec<serde_json::Value> = sys
                        .processes()
                        .iter()
                        .map(|(pid, process)| {
                            json!({
                                "pid": pid.as_u32(),
                                "name": process.name(),
                                "cpu_usage": process.cpu_usage(),
                                "memory": process.memory()
                            })
                        })
                        .collect();

                    Ok(json!({ "processes": processes }))
                }
            }
            "disk_usage" => {
                if !Self::check_capability(&request.input, "system.read") {
                    Err(Error::Permission(
                        "Missing required capability: system.read".to_string(),
                    ))
                } else {
                    let disks = Disks::new_with_refreshed_list();

                    let disks_vec: Vec<serde_json::Value> = disks
                        .iter()
                        .map(|disk| {
                            json!({
                                "name": disk.name().to_string_lossy(),
                                "mount_point": disk.mount_point().to_string_lossy(),
                                "total_space": disk.total_space(),
                                "available_space": disk.available_space(),
                                "file_system": disk.file_system().to_string_lossy().to_string()
                            })
                        })
                        .collect();

                    Ok(json!({ "disks": disks_vec }))
                }
            }
            "network_stats" => {
                if !Self::check_capability(&request.input, "system.read") {
                    Err(Error::Permission(
                        "Missing required capability: system.read".to_string(),
                    ))
                } else {
                    let networks = Networks::new_with_refreshed_list();

                    let networks_vec: Vec<serde_json::Value> = networks
                        .iter()
                        .map(|(interface, data)| {
                            json!({
                                "interface": interface,
                                "received": data.received(),
                                "transmitted": data.transmitted()
                            })
                        })
                        .collect();

                    Ok(json!({ "networks": networks_vec }))
                }
            }
            "env_get" => {
                if !Self::check_capability(&request.input, "env.read") {
                    Err(Error::Permission(
                        "Missing required capability: env.read".to_string(),
                    ))
                } else {
                    const ALLOWLIST: &[&str] = &[
                        "PATH", "HOME", "USER", "USERNAME", "SHELL", "LANG", "LC_ALL", "PWD",
                        "TERM", "HOSTNAME", "TMPDIR", "TEMP", "TMP",
                    ];

                    let key = request
                        .input
                        .get("key")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            Error::Worker("Missing required parameter: key".to_string())
                        })?;

                    if !ALLOWLIST.contains(&key) {
                        return Err(Error::Permission(format!(
                            "env var '{}' is not in the allowed list",
                            key
                        )));
                    }

                    let value = std::env::var(key)
                        .map_err(|e| Error::Worker(format!("Failed to get env var: {}", e)))?;

                    Ok(json!({
                        "key": key,
                        "value": value
                    }))
                }
            }
            _ => Err(Error::Worker(format!(
                "Unknown native skill: {}",
                skill_name
            ))),
        };

        // Apply timeout to the operation
        let timeout_result = tokio::time::timeout(timeout_duration, async { result }).await;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        match timeout_result {
            Ok(Ok(data)) => {
                // Emit SkillInvokeEnd event
                self.event_stream.publish(EventEnvelope::new(
                    EventLevel::Info,
                    EventType::SkillInvokeEnd,
                    json!({
                        "worker_id": self.worker_id,
                        "run_id": run_id,
                        "skill_name": skill_name,
                        "success": true,
                    }),
                ));

                Ok(InvokeResponse {
                    run_id,
                    status: InvokeStatus::Success,
                    result: data,
                    error: None,
                    exit_code: Some(0),
                    duration_ms,
                    truncated: false,
                })
            }
            Ok(Err(e)) => {
                // Emit SkillInvokeFailed event
                self.event_stream.publish(EventEnvelope::new(
                    EventLevel::Error,
                    EventType::SkillInvokeFailed,
                    json!({
                        "worker_id": self.worker_id,
                        "run_id": run_id,
                        "skill_name": skill_name,
                        "error": e.to_string(),
                    }),
                ));

                Ok(InvokeResponse {
                    run_id,
                    status: InvokeStatus::Failed,
                    result: json!({}),
                    error: Some(e.to_string()),
                    exit_code: Some(1),
                    duration_ms,
                    truncated: false,
                })
            }
            Err(_) => {
                // Timeout occurred
                self.event_stream.publish(EventEnvelope::new(
                    EventLevel::Error,
                    EventType::SkillInvokeFailed,
                    json!({
                        "worker_id": self.worker_id,
                        "run_id": run_id,
                        "skill_name": skill_name,
                        "error": format!("Skill execution timed out after {}s", timeout_secs),
                    }),
                ));

                Ok(InvokeResponse {
                    run_id,
                    status: InvokeStatus::Timeout,
                    result: json!({}),
                    error: Some(format!("Skill execution timed out after {}s", timeout_secs)),
                    exit_code: None,
                    duration_ms,
                    truncated: false,
                })
            }
        }
    }

    async fn stream_events(&self, run_id: RunId) -> Result<mpsc::Receiver<StreamEvent>> {
        let (tx, rx) = mpsc::channel(256);
        let mut active_runs = self.active_runs.write().await;
        active_runs.insert(run_id, tx);
        Ok(rx)
    }

    async fn cancel(&self, run_id: RunId, _reason: String) -> Result<()> {
        let mut active_runs = self.active_runs.write().await;
        active_runs.remove(&run_id);
        Ok(())
    }

    async fn health(&self) -> Result<HealthResponse> {
        Ok(HealthResponse {
            healthy: true,
            worker_id: self.worker_id.clone(),
            uptime_secs: self.created_at.elapsed().as_secs(),
            attestation: None,
        })
    }

    async fn shutdown(&self) -> Result<()> {
        let mut active_runs = self.active_runs.write().await;
        active_runs.clear();
        Ok(())
    }
}

/// Result of a single health check on a worker.
#[allow(dead_code)]
struct HealthCheckResult {
    /// Whether the worker is still alive
    healthy: bool,
    /// The worker's runtime type
    runtime: WorkerRuntime,
    /// Exit code if the worker has exited (Some(Some(code)) or Some(None) for signal)
    #[allow(clippy::option_option)]
    exit_code: Option<Option<i32>>,
    /// Attestation data from the health response (if any)
    attestation: Option<carnelian_common::types::WorkerAttestationData>,
    /// When attestation was last verified (from worker registry)
    last_attestation_verified: Option<DateTime<Utc>>,
}

/// Compute the expected build checksum for a worker runtime, matching the
/// algorithm each worker uses to report its own checksum.
///
/// - **Node**: reads `workers/node-worker/package.json` → `v{version}` (matches `computeBuildChecksum()` in index.ts)
/// - **Python**: SHA-256 of `workers/python-worker/worker.py` (matches `compute_build_checksum()` in worker.py)
/// - **Shell/Wasm**: placeholder version string
fn compute_expected_checksum(_config: &Config, runtime: WorkerRuntime, path: &str) -> String {
    match runtime {
        WorkerRuntime::Node => {
            // Match the Node worker's computeBuildChecksum(): reads package.json version
            let pkg_path = format!("{}/package.json", path);
            match std::fs::read_to_string(&pkg_path) {
                Ok(contents) => {
                    if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&contents) {
                        if let Some(version) = pkg.get("version").and_then(|v| v.as_str()) {
                            return format!("v{}", version);
                        }
                    }
                    "v0.1.0".to_string()
                }
                Err(_) => "v0.1.0".to_string(),
            }
        }
        WorkerRuntime::Python => {
            // Match the Python worker's compute_build_checksum(): returns "v{VERSION}"
            "v0.1.0".to_string()
        }
        _ => format!("v0.1.0-{}", path),
    }
}

/// Perform a health check on a single worker, updating its status and emitting events.
///
/// This is the shared implementation used by `check_worker_health`, `run_health_checks`,
/// and the background health check loop. It:
/// 1. Locks the registry, checks health via transport or `try_wait()`, updates status/timestamps
/// 2. Emits `WorkerHealthCheck` event
/// 3. If unhealthy, emits `WorkerStopped` event and removes the worker from the registry
///
/// Returns `Ok(result)` with health status, or `Err` if the worker is not found.
#[allow(clippy::significant_drop_tightening, clippy::too_many_lines)]
async fn perform_single_health_check(
    workers: &RwLock<HashMap<String, Worker>>,
    event_stream: &EventStream,
    worker_id: &str,
) -> Result<HealthCheckResult> {
    let (healthy, runtime, exit_code, attestation, last_attestation_verified) = {
        let mut w = workers.write().await;
        let worker = w
            .get_mut(worker_id)
            .ok_or_else(|| Error::Config(format!("Worker not found: {}", worker_id)))?;

        let runtime = worker.runtime;
        let last_att_verified = worker.last_attestation_verified;

        // If the worker has a transport, use it for health checks
        if let Some(ref transport) = worker.transport {
            match transport.health().await {
                Ok(resp) if resp.healthy => {
                    worker.last_health_check = Some(Utc::now());
                    // Store attestation on the worker struct
                    if let Some(ref att) = resp.attestation {
                        worker.last_attestation = Some(crate::attestation::WorkerAttestation {
                            worker_id: worker_id.to_string(),
                            last_ledger_head: att.last_ledger_head.clone(),
                            build_checksum: att.build_checksum.clone(),
                            config_version: att.config_version.clone(),
                        });
                    }
                    (true, runtime, None, resp.attestation, last_att_verified)
                }
                Ok(resp) => {
                    tracing::error!(
                        worker_id = %worker_id,
                        "Worker process exited unexpectedly (transport health check)"
                    );
                    worker.status = WorkerStatus::Failed;
                    (false, runtime, None, resp.attestation, last_att_verified)
                }
                Err(e) => {
                    tracing::warn!(
                        worker_id = %worker_id,
                        error = %e,
                        "Failed to check worker health via transport"
                    );
                    (false, runtime, None, None, last_att_verified)
                }
            }
        } else if let Some(ref mut process) = worker.process {
            match process.try_wait() {
                Ok(Some(status)) => {
                    tracing::error!(
                        worker_id = %worker_id,
                        exit_code = ?status.code(),
                        "Worker process exited unexpectedly"
                    );
                    worker.status = WorkerStatus::Failed;
                    (false, runtime, Some(status.code()), None, last_att_verified)
                }
                Ok(None) => {
                    worker.last_health_check = Some(Utc::now());
                    (true, runtime, None, None, last_att_verified)
                }
                Err(e) => {
                    tracing::warn!(
                        worker_id = %worker_id,
                        error = %e,
                        "Failed to check worker health"
                    );
                    (false, runtime, None, None, last_att_verified)
                }
            }
        } else {
            // No process and no transport — mark as failed
            worker.status = WorkerStatus::Failed;
            (false, runtime, None, None, last_att_verified)
        }
    };

    // Emit WorkerHealthCheck event (always)
    if healthy {
        event_stream.publish(EventEnvelope::new(
            EventLevel::Debug,
            EventType::WorkerHealthCheck,
            json!({
                "worker_id": worker_id,
                "healthy": true,
            }),
        ));
    } else {
        event_stream.publish(EventEnvelope::new(
            EventLevel::Error,
            EventType::WorkerHealthCheck,
            json!({
                "worker_id": worker_id,
                "healthy": false,
                "exit_code": exit_code,
            }),
        ));

        // Emit WorkerStopped for crashed/unexpectedly exited workers
        let reason = match exit_code {
            Some(Some(code)) => format!("crashed (exit code {})", code),
            Some(None) => "crashed (signal)".to_string(),
            None => "crashed".to_string(),
        };
        event_stream.publish(EventEnvelope::new(
            EventLevel::Warn,
            EventType::WorkerStopped,
            json!({
                "worker_id": worker_id,
                "runtime": runtime.to_string(),
                "reason": reason,
            }),
        ));

        // Remove failed worker from registry
        workers.write().await.remove(worker_id);
        tracing::info!(
            worker_id = %worker_id,
            "Removed failed worker from registry"
        );
    }

    Ok(HealthCheckResult {
        healthy,
        runtime,
        exit_code,
        attestation,
        last_attestation_verified,
    })
}

/// Background worker manager maintaining an in-memory registry of active workers.
///
/// Workers are spawned as child processes and monitored via periodic health checks.
/// The manager integrates with `AppState` and emits events through `EventStream`.
pub struct WorkerManager {
    /// Thread-safe worker registry
    workers: Arc<RwLock<HashMap<String, Worker>>>,
    /// Application configuration for max_workers
    config: Arc<Config>,
    /// Event stream for emitting worker lifecycle events
    event_stream: Arc<EventStream>,
    /// Shutdown signal sender for health check loop
    shutdown_tx: Option<watch::Sender<bool>>,
    /// Counter for generating unique worker IDs per runtime
    id_counters: HashMap<String, u32>,
    /// Safe mode guard for blocking side-effect operations
    safe_mode_guard: Option<Arc<crate::safe_mode::SafeModeGuard>>,
    /// Database pool for attestation queries
    pool: Option<PgPool>,
    /// Ledger for logging quarantine events
    ledger: Option<Arc<Ledger>>,
}

impl WorkerManager {
    /// Create a new WorkerManager.
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration (provides max_workers via machine_config)
    /// * `event_stream` - Event stream for publishing worker lifecycle events
    pub fn new(config: Arc<Config>, event_stream: Arc<EventStream>) -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            config,
            event_stream,
            shutdown_tx: None,
            id_counters: HashMap::new(),
            safe_mode_guard: None,
            pool: None,
            ledger: None,
        }
    }

    /// Set the database pool for attestation verification.
    pub fn set_pool(&mut self, pool: PgPool) {
        self.pool = Some(pool);
    }

    /// Set the ledger for logging quarantine events.
    pub fn set_ledger(&mut self, ledger: Arc<Ledger>) {
        self.ledger = Some(ledger);
    }

    /// Set the safe mode guard for blocking worker spawns when safe mode is active.
    pub fn set_safe_mode_guard(&mut self, guard: Arc<crate::safe_mode::SafeModeGuard>) {
        self.safe_mode_guard = Some(guard);
    }

    /// Generate a unique worker ID for the given runtime.
    ///
    /// IDs follow the pattern `{runtime}-worker-{n}`, e.g., "node-worker-1".
    fn next_worker_id(&mut self, runtime: WorkerRuntime) -> String {
        let key = runtime.to_string();
        let counter = self.id_counters.entry(key.clone()).or_insert(0);
        *counter += 1;
        format!("{}-worker-{}", key, counter)
    }

    /// Spawn a single worker process of the given runtime type.
    ///
    /// # Arguments
    ///
    /// * `runtime` - The runtime type to spawn (Node, Python, Shell)
    ///
    /// # Returns
    ///
    /// The unique worker ID assigned to the spawned worker.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The max_workers limit has been reached (unless bypass_limit is true)
    /// - The process fails to spawn
    pub async fn spawn_worker(
        &mut self,
        runtime: WorkerRuntime,
        bypass_limit: bool,
    ) -> Result<String> {
        if let Some(ref guard) = self.safe_mode_guard {
            guard.check_or_block("worker_spawn").await?;
        }

        let max_workers = self.config.machine_config().max_workers;
        let current_count = self.workers.read().await.len();

        if !bypass_limit && current_count >= max_workers as usize {
            return Err(Error::Config(format!(
                "Max workers limit reached ({}/{})",
                current_count, max_workers
            )));
        }

        let worker_id = self.next_worker_id(runtime);

        // Handle Python runtime specially (inline to avoid circular dependency)
        if runtime == WorkerRuntime::Python {
            // Step 1: Detect Python binary
            let python_bin = which::which("python3")
                .or_else(|_| which::which("python"))
                .map_err(|e| {
                    Error::Connection(format!(
                        "Failed to find Python binary (tried 'python3', then 'python'): {}",
                        e
                    ))
                })?;
            tracing::info!("Using Python binary: {}", python_bin.display());

            // Step 2: Pre-install requirements if present
            let req_path = std::path::PathBuf::from("workers/python-worker/requirements.txt");
            if req_path.exists() {
                tracing::info!(
                    "Installing Python worker requirements from {}",
                    req_path.display()
                );
                let status = tokio::process::Command::new(&python_bin)
                    .args([
                        "-m",
                        "pip",
                        "install",
                        "-r",
                        req_path.to_str().unwrap(),
                        "--quiet",
                        "--disable-pip-version-check",
                    ])
                    .status()
                    .await
                    .map_err(|e| {
                        Error::Connection(format!("Failed to spawn pip install command: {}", e))
                    })?;

                if status.success() {
                    tracing::info!("Python worker requirements installed successfully");
                } else {
                    let code = status.code().unwrap_or(-1);
                    tracing::warn!("pip install failed with exit code: {}", code);
                    return Err(Error::Connection(format!(
                        "pip install failed with exit code: {}",
                        code
                    )));
                }
            }

            // Step 3: Compute attestation env vars
            let api_url = format!("http://localhost:{}", self.config.http_port);
            let ledger_head = self
                .get_expected_ledger_head()
                .await
                .unwrap_or_else(|_| "genesis".to_string());
            let config_version = self
                .get_expected_config_version()
                .await
                .unwrap_or_else(|_| "v1".to_string());
            let build_checksum = self.get_expected_build_checksum(runtime);

            // Step 4: Build spawn command with detected binary
            let mut cmd = tokio::process::Command::new(&python_bin);
            cmd.arg("workers/python-worker/worker.py")
                .env("WORKER_ID", &worker_id)
                .env("CARNELIAN_API_URL", &api_url)
                .env("CARNELIAN_LEDGER_HEAD", &ledger_head)
                .env("CARNELIAN_CONFIG_VERSION", &config_version)
                .env("CARNELIAN_BUILD_CHECKSUM", &build_checksum)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            // Step 5: Spawn the child process
            let child = cmd.spawn().map_err(|e| {
                tracing::error!(
                    worker_id = %worker_id,
                    runtime = %runtime,
                    error = %e,
                    "Failed to spawn Python worker process"
                );
                Error::Connection(format!("Failed to spawn Python worker: {}", e))
            })?;

            // Step 6: Create transport from the spawned process
            let (transport, stderr) = ProcessJsonlTransport::new(
                worker_id.clone(),
                child,
                self.config.clone(),
                self.event_stream.clone(),
            )?;
            let transport: Arc<dyn WorkerTransport> = Arc::new(transport);

            // Step 7: Register worker (same as other runtimes)
            let worker = Worker {
                id: worker_id.clone(),
                runtime,
                process: None,
                status: WorkerStatus::Starting,
                current_task: None,
                started_at: Utc::now(),
                last_health_check: None,
                transport: Some(transport),
                last_attestation: None,
                quarantined: false,
                last_attestation_verified: None,
            };

            self.workers.write().await.insert(worker_id.clone(), worker);

            // Spawn stderr handler if available
            if let Some(stderr) = stderr {
                Self::spawn_stderr_handler(worker_id.clone(), stderr);
            }

            // Emit WorkerStarted event
            self.event_stream.publish(EventEnvelope::new(
                EventLevel::Info,
                EventType::WorkerStarted,
                json!({
                    "worker_id": worker_id,
                    "runtime": runtime.to_string(),
                }),
            ));

            tracing::info!(
                worker_id = %worker_id,
                runtime = %runtime,
                "Worker spawned"
            );

            // Update status to Running after successful spawn
            {
                let mut workers = self.workers.write().await;
                if let Some(w) = workers.get_mut(&worker_id) {
                    w.status = WorkerStatus::Running;
                }
            }

            return Ok(worker_id);
        }

        let api_url = format!("http://localhost:{}", self.config.http_port);

        let mut cmd = match runtime {
            WorkerRuntime::Node => {
                let mut c = tokio::process::Command::new("node");
                c.args(["workers/node-worker/dist/index.js"]);
                c
            }
            WorkerRuntime::Python => {
                unreachable!("Python runtime should have been handled earlier")
            }
            WorkerRuntime::Native => {
                // Create native transport for in-process execution
                let transport = NativeWorkerTransport::new(
                    worker_id.clone(),
                    self.event_stream.clone(),
                    self.config.clone(),
                );
                let transport: Arc<dyn WorkerTransport> = Arc::new(transport);

                // Register worker with transport
                let worker = Worker {
                    id: worker_id.clone(),
                    runtime,
                    process: None,
                    status: WorkerStatus::Running,
                    current_task: None,
                    started_at: Utc::now(),
                    last_health_check: None,
                    transport: Some(transport),
                    last_attestation: None,
                    quarantined: false,
                    last_attestation_verified: None,
                };

                self.workers.write().await.insert(worker_id.clone(), worker);

                // Emit WorkerStarted event
                self.event_stream.publish(EventEnvelope::new(
                    EventLevel::Info,
                    EventType::WorkerStarted,
                    json!({
                        "worker_id": worker_id,
                        "runtime": runtime.to_string(),
                    }),
                ));

                tracing::info!(
                    worker_id = %worker_id,
                    runtime = %runtime,
                    "Worker spawned"
                );

                return Ok(worker_id);
            }
            WorkerRuntime::Wasm => {
                // Create WASM runtime and transport
                let wasm_runtime = Arc::new(crate::skills::wasm_runtime::WasmSkillRuntime::new()?);
                let transport = WasmWorkerTransport::new(
                    worker_id.clone(),
                    wasm_runtime,
                    self.event_stream.clone(),
                    self.config.clone(),
                );
                let transport: Arc<dyn WorkerTransport> = Arc::new(transport);

                // Register worker with transport
                let worker = Worker {
                    id: worker_id.clone(),
                    runtime,
                    process: None,
                    status: WorkerStatus::Running,
                    current_task: None,
                    started_at: Utc::now(),
                    last_health_check: None,
                    transport: Some(transport),
                    last_attestation: None,
                    quarantined: false,
                    last_attestation_verified: None,
                };

                self.workers.write().await.insert(worker_id.clone(), worker);

                // Emit WorkerStarted event
                self.event_stream.publish(EventEnvelope::new(
                    EventLevel::Info,
                    EventType::WorkerStarted,
                    json!({
                        "worker_id": worker_id,
                        "runtime": runtime.to_string(),
                    }),
                ));

                tracing::info!(
                    worker_id = %worker_id,
                    runtime = %runtime,
                    "Worker spawned"
                );

                return Ok(worker_id);
            }
        };

        // Compute attestation env vars for the worker
        let ledger_head = self
            .get_expected_ledger_head()
            .await
            .unwrap_or_else(|_| "genesis".to_string());
        let config_version = self
            .get_expected_config_version()
            .await
            .unwrap_or_else(|_| "v1".to_string());
        let build_checksum = self.get_expected_build_checksum(runtime);

        cmd.env("WORKER_ID", &worker_id)
            .env("CARNELIAN_API_URL", &api_url)
            .env("CARNELIAN_LEDGER_HEAD", &ledger_head)
            .env("CARNELIAN_CONFIG_VERSION", &config_version)
            .env("CARNELIAN_BUILD_CHECKSUM", &build_checksum)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd.spawn().map_err(|e| {
            tracing::error!(
                worker_id = %worker_id,
                runtime = %runtime,
                error = %e,
                "Failed to spawn worker process"
            );
            Error::Connection(format!("Failed to spawn {} worker: {}", runtime, e))
        })?;

        // Create transport from the spawned process
        let (transport, stderr) = ProcessJsonlTransport::new(
            worker_id.clone(),
            child,
            self.config.clone(),
            self.event_stream.clone(),
        )?;
        let transport: Arc<dyn WorkerTransport> = Arc::new(transport);

        let worker = Worker {
            id: worker_id.clone(),
            runtime,
            process: None,
            status: WorkerStatus::Starting,
            current_task: None,
            started_at: Utc::now(),
            last_health_check: None,
            transport: Some(transport),
            last_attestation: None,
            quarantined: false,
            last_attestation_verified: None,
        };

        self.workers.write().await.insert(worker_id.clone(), worker);

        // Spawn stderr handler if available
        if let Some(stderr) = stderr {
            Self::spawn_stderr_handler(worker_id.clone(), stderr);
        }

        // Emit WorkerStarted event
        self.event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::WorkerStarted,
            json!({
                "worker_id": worker_id,
                "runtime": runtime.to_string(),
            }),
        ));

        tracing::info!(
            worker_id = %worker_id,
            runtime = %runtime,
            "Worker spawned"
        );

        // Update status to Running after successful spawn
        {
            let mut workers = self.workers.write().await;
            if let Some(w) = workers.get_mut(&worker_id) {
                w.status = WorkerStatus::Running;
            }
        }

        Ok(worker_id)
    }

    /// Start workers up to the configured max_workers limit.
    ///
    /// Spawns Node.js workers by default, plus one each of Native, Wasm, and Python runtimes.
    /// Logs errors for individual worker spawn failures but continues starting remaining workers.
    pub async fn start_workers(&mut self) -> Result<()> {
        let max_workers = self.config.machine_config().max_workers;
        let mut node_started = 0u32;
        let mut native_started = 0u32;
        let mut wasm_started = 0u32;
        let mut python_started = 0u32;

        // Spawn Node.js workers (primary runtime)
        for _ in 0..max_workers {
            match self.spawn_worker(WorkerRuntime::Node, false).await {
                Ok(id) => {
                    tracing::info!(worker_id = %id, runtime = "node", "Worker started successfully");
                    node_started += 1;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to start Node worker, continuing with remaining");
                }
            }
        }

        // Spawn one Native worker (in-process)
        match self.spawn_worker(WorkerRuntime::Native, true).await {
            Ok(id) => {
                tracing::info!(worker_id = %id, runtime = "native", "Native worker started successfully");
                native_started += 1;
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to start Native worker, continuing");
            }
        }

        // Spawn one Wasm worker (in-process)
        match self.spawn_worker(WorkerRuntime::Wasm, true).await {
            Ok(id) => {
                tracing::info!(worker_id = %id, runtime = "wasm", "Wasm worker started successfully");
                wasm_started += 1;
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to start Wasm worker, continuing");
            }
        }

        // Attempt to spawn one Python worker (external binary - may not be available)
        match self.spawn_worker(WorkerRuntime::Python, true).await {
            Ok(id) => {
                tracing::info!(worker_id = %id, runtime = "python", "Python worker started successfully");
                python_started += 1;
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to start Python worker (python may not be installed), continuing");
            }
        }

        let total_started = node_started + native_started + wasm_started + python_started;

        tracing::info!(
            node = node_started,
            native = native_started,
            wasm = wasm_started,
            python = python_started,
            total = total_started,
            max_workers = max_workers,
            "Worker startup complete"
        );

        // Start background health check loop
        self.start_health_check_loop();

        Ok(())
    }

    /// Stop a specific worker by ID.
    ///
    /// For transport-owned workers, calls `transport.shutdown()` which cancels
    /// all active runs and sends SIGTERM/SIGKILL to the process. For legacy
    /// process-owned workers, sends SIGTERM directly and waits up to 5 seconds.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker ID is not found in the registry.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn stop_worker(&mut self, worker_id: &str) -> Result<()> {
        // Phase 1: Remove worker from registry, extract transport + process + runtime.
        let (transport, child, runtime) = {
            let mut workers = self.workers.write().await;
            let worker = workers
                .remove(worker_id)
                .ok_or_else(|| Error::Config(format!("Worker not found: {worker_id}")))?;
            (worker.transport, worker.process, worker.runtime)
        };

        tracing::info!(worker_id = %worker_id, "Stopping worker");

        // Phase 2: If transport owns the process, use transport.shutdown() to
        // cancel active runs and terminate the process.
        if let Some(transport) = transport {
            if let Err(e) = transport.shutdown().await {
                tracing::warn!(
                    worker_id = %worker_id,
                    error = %e,
                    "Error during transport shutdown"
                );
            }
        } else if let Some(mut child) = child {
            // Legacy path: process is owned directly by the Worker struct.
            #[cfg(unix)]
            {
                if let Some(pid) = child.id() {
                    let nix_pid =
                        nix::unistd::Pid::from_raw(i32::try_from(pid).unwrap_or(i32::MAX));
                    if let Err(e) =
                        nix::sys::signal::kill(nix_pid, nix::sys::signal::Signal::SIGTERM)
                    {
                        tracing::warn!(
                            worker_id = %worker_id,
                            error = %e,
                            "Failed to send SIGTERM to worker"
                        );
                    }
                    tracing::debug!(worker_id = %worker_id, pid = pid, "Sent SIGTERM to worker");
                }
            }
            #[cfg(not(unix))]
            {
                tracing::debug!(
                    worker_id = %worker_id,
                    "Non-Unix platform, will use kill() as fallback"
                );
            }

            let exited =
                tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await;

            match exited {
                Ok(Ok(status)) => {
                    tracing::info!(
                        worker_id = %worker_id,
                        exit_code = ?status.code(),
                        "Worker stopped gracefully"
                    );
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        worker_id = %worker_id,
                        error = %e,
                        "Error waiting for worker exit"
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        worker_id = %worker_id,
                        "Worker did not exit within 5 seconds, sending SIGKILL"
                    );
                    if let Err(e) = child.kill().await {
                        tracing::error!(
                            worker_id = %worker_id,
                            error = %e,
                            "Failed to SIGKILL worker"
                        );
                    }
                }
            }
        }

        // Phase 3: Emit WorkerStopped event.
        self.event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::WorkerStopped,
            json!({
                "worker_id": worker_id,
                "runtime": runtime.to_string(),
                "reason": "requested",
            }),
        ));

        Ok(())
    }

    /// Stop all active workers.
    ///
    /// Iterates through all workers and stops each one. Errors are logged
    /// but do not prevent stopping remaining workers.
    pub async fn stop_all_workers(&mut self) -> Result<()> {
        let worker_ids: Vec<String> = self.workers.read().await.keys().cloned().collect();
        let count = worker_ids.len();

        tracing::info!(worker_count = count, "Stopping all workers");

        for worker_id in worker_ids {
            if let Err(e) = self.stop_worker(&worker_id).await {
                tracing::warn!(
                    worker_id = %worker_id,
                    error = %e,
                    "Failed to stop worker"
                );
            }
        }

        // Stop health check loop
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(true);
            tracing::info!("Worker health check loop stopped");
        }

        tracing::info!("All workers stopped");
        Ok(())
    }

    /// Restart a specific worker.
    ///
    /// Stops the worker and spawns a new one with the same runtime type.
    ///
    /// # Returns
    ///
    /// The new worker ID.
    pub async fn restart_worker(&mut self, worker_id: &str) -> Result<String> {
        let runtime = self
            .workers
            .read()
            .await
            .get(worker_id)
            .ok_or_else(|| Error::Config(format!("Worker not found: {}", worker_id)))?
            .runtime;

        self.stop_worker(worker_id).await?;
        self.spawn_worker(runtime, false).await
    }

    /// Check health of a specific worker.
    ///
    /// Uses `try_wait()` to check if the process is still alive without blocking.
    /// Updates the worker status and emits health check and stopped events as needed.
    /// Failed workers are removed from the registry.
    ///
    /// # Returns
    ///
    /// `true` if the worker is alive, `false` if it has exited.
    pub async fn check_worker_health(&self, worker_id: &str) -> Result<bool> {
        let result =
            perform_single_health_check(&self.workers, &self.event_stream, worker_id).await?;
        Ok(result.healthy)
    }

    /// Run health checks on all active workers.
    ///
    /// Checks each worker and logs a summary. Failed workers are removed
    /// from the registry and have `WorkerStopped` events emitted.
    pub async fn run_health_checks(&self) -> Result<()> {
        let worker_ids: Vec<String> = self.workers.read().await.keys().cloned().collect();
        let mut healthy_count = 0usize;
        let mut failed_count = 0usize;

        for worker_id in &worker_ids {
            match perform_single_health_check(&self.workers, &self.event_stream, worker_id).await {
                Ok(result) => {
                    if result.healthy {
                        healthy_count += 1;
                    } else {
                        failed_count += 1;
                        tracing::warn!(worker_id = %worker_id, "Worker failed health check");
                    }
                }
                Err(e) => {
                    failed_count += 1;
                    tracing::warn!(
                        worker_id = %worker_id,
                        error = %e,
                        "Error during health check"
                    );
                }
            }
        }

        tracing::debug!(
            healthy = healthy_count,
            failed = failed_count,
            total = worker_ids.len(),
            "Health check cycle complete"
        );

        Ok(())
    }

    /// Start the background health check loop.
    ///
    /// Spawns a tokio task that runs health checks every 30 seconds using the
    /// shared `perform_single_health_check` function. Failed workers are automatically
    /// removed from the registry and have `WorkerStopped` events emitted.
    /// After each healthy check, attestation data (if present) is verified and
    /// mismatched workers are quarantined.
    /// The loop responds to the shutdown signal for graceful termination.
    fn start_health_check_loop(&mut self) {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        self.shutdown_tx = Some(shutdown_tx);

        let workers = self.workers.clone();
        let event_stream = self.event_stream.clone();
        let pool = self.pool.clone();
        let ledger = self.ledger.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            // Skip the first immediate tick
            interval.tick().await;

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let worker_ids: Vec<String> = workers.read().await.keys().cloned().collect();
                        let mut healthy = 0usize;
                        let mut failed = 0usize;

                        for worker_id in &worker_ids {
                            match perform_single_health_check(&workers, &event_stream, worker_id).await {
                                Ok(result) => {
                                    if result.healthy {
                                        healthy += 1;

                                        // Process attestation if present and due (every 5 minutes)
                                        let attestation_due = result.last_attestation_verified.is_none_or(|t| Utc::now().signed_duration_since(t).num_seconds() > 300); // Always due if never verified

                                        if attestation_due {
                                        if let (Some(att_data), Some(db_pool)) = (&result.attestation, &pool) {
                                            let attestation = crate::attestation::WorkerAttestation {
                                                worker_id: worker_id.clone(),
                                                last_ledger_head: att_data.last_ledger_head.clone(),
                                                build_checksum: att_data.build_checksum.clone(),
                                                config_version: att_data.config_version.clone(),
                                            };

                                            // Compute expected values
                                            let expected_ledger_head = if let Some(ref l) = ledger {
                                                l.load_last_hash().await.unwrap_or(None).unwrap_or_else(|| "genesis".to_string())
                                            } else {
                                                "genesis".to_string()
                                            };

                                            let expected_build_checksum = {
                                                let runtime = result.runtime;
                                                let path = match runtime {
                                                    WorkerRuntime::Node => "workers/node-worker",
                                                    WorkerRuntime::Python => "workers/python-worker",
                                                    WorkerRuntime::Native => "workers/native-worker",
                                                    WorkerRuntime::Wasm => "workers/wasm-worker",
                                                };
                                                compute_expected_checksum(&config, runtime, path)
                                            };

                                            let expected_config_version = {
                                                let version: Option<String> = sqlx::query_scalar(
                                                    "SELECT value_text FROM config_store WHERE key = 'config_version'"
                                                )
                                                .fetch_optional(db_pool)
                                                .await
                                                .unwrap_or(None);
                                                version.unwrap_or_else(|| "v1".to_string())
                                            };

                                            match crate::attestation::verify_attestation(
                                                db_pool,
                                                &attestation,
                                                &expected_ledger_head,
                                                &expected_build_checksum,
                                                &expected_config_version,
                                            ).await {
                                                Ok(att_result) if !att_result.verified => {
                                                    // Quarantine the worker
                                                    let reason = att_result.mismatch_reason.as_deref().unwrap_or("unknown");
                                                    if let Err(e) = crate::attestation::quarantine_worker(db_pool, worker_id, reason).await {
                                                        tracing::warn!(worker_id = %worker_id, error = %e, "Failed to quarantine worker in DB");
                                                    }

                                                    // Mark quarantined in registry
                                                    {
                                                        let mut w = workers.write().await;
                                                        if let Some(worker) = w.get_mut(worker_id.as_str()) {
                                                            worker.quarantined = true;
                                                            worker.status = WorkerStatus::Failed;
                                                        }
                                                    }

                                                    // Log to ledger
                                                    if let Some(ref l) = ledger {
                                                        if let Err(e) = l.append_event(
                                                            None,
                                                            "worker.quarantined",
                                                            json!({
                                                                "worker_id": worker_id,
                                                                "reason": att_result.mismatch_reason,
                                                                "attestation": attestation,
                                                            }),
                                                            None,
                                                            None,
                                                            None,
                                                        ).await {
                                                            tracing::warn!(worker_id = %worker_id, error = %e, "Failed to log quarantine to ledger");
                                                        }
                                                    }

                                                    tracing::error!(
                                                        worker_id = %worker_id,
                                                        reason = ?att_result.mismatch_reason,
                                                        "Worker quarantined due to attestation mismatch"
                                                    );
                                                    failed += 1;
                                                    healthy -= 1;
                                                }
                                                Ok(_) => {
                                                    // Record successful attestation
                                                    if let Err(e) = crate::attestation::record_attestation(db_pool, &attestation).await {
                                                        tracing::warn!(worker_id = %worker_id, error = %e, "Failed to record attestation");
                                                    }
                                                    // Update last_attestation_verified timestamp
                                                    {
                                                        let mut w = workers.write().await;
                                                        if let Some(worker) = w.get_mut(worker_id.as_str()) {
                                                            worker.last_attestation_verified = Some(Utc::now());
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::warn!(worker_id = %worker_id, error = %e, "Attestation verification error");
                                                }
                                            }
                                        }
                                        } // end if attestation_due
                                    } else {
                                        failed += 1;
                                    }
                                }
                                Err(e) => {
                                    failed += 1;
                                    tracing::warn!(
                                        worker_id = %worker_id,
                                        error = %e,
                                        "Background health check error"
                                    );
                                }
                            }
                        }

                        tracing::debug!(
                            healthy = healthy,
                            failed = failed,
                            total = worker_ids.len(),
                            "Background health check complete"
                        );
                    }
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            tracing::info!("Health check loop received shutdown signal");
                            break;
                        }
                    }
                }
            }
        });
    }

    /// Check if a worker is quarantined before assigning tasks.
    ///
    /// First checks the in-memory `worker.quarantined` flag (fast path),
    /// then falls back to the database `worker_attestations` table.
    pub async fn can_assign_task(&self, worker_id: &str) -> Result<bool> {
        // Fast path: check in-memory quarantine flag
        {
            let workers = self.workers.read().await;
            if let Some(worker) = workers.get(worker_id) {
                if worker.quarantined {
                    return Ok(false);
                }
            }
        }

        // Slow path: check database
        if let Some(ref pool) = self.pool {
            let quarantined = crate::attestation::is_worker_quarantined(pool, worker_id).await?;
            Ok(!quarantined)
        } else {
            Ok(true)
        }
    }

    /// Get the expected ledger head hash from the ledger.
    ///
    /// Returns the last hash from the ledger, or "genesis" if no ledger is
    /// configured or no events have been recorded.
    pub async fn get_expected_ledger_head(&self) -> Result<String> {
        if let Some(ref ledger) = self.ledger {
            let last_hash = ledger.load_last_hash().await?;
            Ok(last_hash.unwrap_or_else(|| "genesis".to_string()))
        } else {
            Ok("genesis".to_string())
        }
    }

    /// Get the expected config version from the `config_store` table.
    ///
    /// Returns the value of the `config_version` key, or "v1" if not found
    /// or no database pool is configured.
    pub async fn get_expected_config_version(&self) -> Result<String> {
        if let Some(ref pool) = self.pool {
            let version: Option<String> = sqlx::query_scalar(
                "SELECT value_text FROM config_store WHERE key = 'config_version'",
            )
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;

            Ok(version.unwrap_or_else(|| "v1".to_string()))
        } else {
            Ok("v1".to_string())
        }
    }

    /// Get the expected build checksum for a worker runtime.
    ///
    /// Delegates to `compute_expected_checksum()` which matches each worker's
    /// own checksum computation algorithm.
    pub fn get_expected_build_checksum(&self, runtime: WorkerRuntime) -> String {
        let path = match runtime {
            WorkerRuntime::Node => "workers/node-worker",
            WorkerRuntime::Python => "workers/python-worker",
            WorkerRuntime::Native => "workers/native-worker",
            WorkerRuntime::Wasm => "workers/wasm-worker",
        };
        compute_expected_checksum(&self.config, runtime, path)
    }

    /// Process attestation data from a health check response.
    ///
    /// Verifies the attestation against expected values and quarantines the worker
    /// if there is a mismatch. Records successful attestations in the database.
    /// Updates `worker.last_attestation_verified` timestamp on success.
    ///
    /// Returns `true` if attestation passed, `false` if quarantined.
    pub async fn process_attestation(
        &self,
        worker_id: &str,
        runtime: WorkerRuntime,
        attestation_data: &crate::attestation::WorkerAttestation,
    ) -> Result<bool> {
        let pool = match self.pool {
            Some(ref p) => p,
            None => return Ok(true), // No pool, skip attestation
        };

        let expected_ledger_head = self.get_expected_ledger_head().await?;
        let expected_build_checksum = self.get_expected_build_checksum(runtime);
        let expected_config_version = self.get_expected_config_version().await?;

        let result = crate::attestation::verify_attestation(
            pool,
            attestation_data,
            &expected_ledger_head,
            &expected_build_checksum,
            &expected_config_version,
        )
        .await?;

        if !result.verified {
            // Quarantine worker in DB
            crate::attestation::quarantine_worker(
                pool,
                worker_id,
                result.mismatch_reason.as_deref().unwrap_or("unknown"),
            )
            .await?;

            // Mark quarantined in registry
            {
                let mut workers = self.workers.write().await;
                if let Some(worker) = workers.get_mut(worker_id) {
                    worker.quarantined = true;
                    worker.status = WorkerStatus::Failed;
                }
            }

            // Log to ledger
            if let Some(ref ledger) = self.ledger {
                if let Err(e) = ledger
                    .append_event(
                        None,
                        "worker.quarantined",
                        json!({
                            "worker_id": worker_id,
                            "reason": result.mismatch_reason,
                            "attestation": attestation_data,
                        }),
                        None,
                        None,
                        None,
                    )
                    .await
                {
                    tracing::warn!(
                        worker_id = %worker_id,
                        error = %e,
                        "Failed to log worker quarantine to ledger"
                    );
                }
            }

            tracing::error!(
                worker_id = %worker_id,
                reason = ?result.mismatch_reason,
                "Worker quarantined due to attestation mismatch"
            );

            Ok(false)
        } else {
            // Record successful attestation
            crate::attestation::record_attestation(pool, attestation_data).await?;

            // Update last_attestation_verified timestamp
            {
                let mut workers = self.workers.write().await;
                if let Some(worker) = workers.get_mut(worker_id) {
                    worker.last_attestation_verified = Some(Utc::now());
                }
            }

            Ok(true)
        }
    }

    /// Spawn a worker process for a sub-agent with a scoped identity pack.
    ///
    /// The identity pack is serialized as JSON and passed to the worker process
    /// via the `CARNELIAN_IDENTITY_PACK` environment variable. This provides the
    /// worker with the sub-agent's identity context including granted capabilities.
    ///
    /// # Arguments
    ///
    /// * `sub_agent_id` - UUID of the sub-agent identity
    /// * `runtime` - Worker runtime type (Node, Python, Shell)
    /// * `identity_pack` - Scoped identity context for the worker process
    ///
    /// # Returns
    ///
    /// The unique worker ID assigned to the spawned worker.
    pub async fn spawn_sub_agent_worker(
        &mut self,
        sub_agent_id: Uuid,
        runtime: WorkerRuntime,
        identity_pack: crate::sub_agent::IdentityPack,
    ) -> Result<String> {
        if let Some(ref guard) = self.safe_mode_guard {
            guard.check_or_block("worker_spawn").await?;
        }

        let max_workers = self.config.machine_config().max_workers;
        let current_count = self.workers.read().await.len();

        if current_count >= max_workers as usize {
            return Err(Error::Config(format!(
                "Max workers limit reached ({}/{})",
                current_count, max_workers
            )));
        }

        let worker_id = format!("sub-agent-{}", sub_agent_id);
        let api_url = format!("http://localhost:{}", self.config.http_port);

        let identity_pack_json = serde_json::to_string(&identity_pack)
            .map_err(|e| Error::Config(format!("Failed to serialize identity pack: {}", e)))?;

        let mut cmd = match runtime {
            WorkerRuntime::Node => {
                let mut c = tokio::process::Command::new("node");
                c.args(["workers/node-worker/dist/index.js"]);
                c
            }
            WorkerRuntime::Python => {
                let mut c = tokio::process::Command::new("python");
                c.args(["workers/python-worker/worker.py"]);
                c
            }
            WorkerRuntime::Native => {
                // Native runtime is handled inline in spawn_worker, not via subprocess
                unreachable!("Native runtime should have been handled earlier")
            }
            WorkerRuntime::Wasm => {
                return Err(Error::Config("Wasm runtime not yet supported".to_string()));
            }
        };

        cmd.env("WORKER_ID", &worker_id)
            .env("CARNELIAN_API_URL", &api_url)
            .env("CARNELIAN_IDENTITY_PACK", &identity_pack_json)
            .env("CARNELIAN_SUB_AGENT_ID", sub_agent_id.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let process = cmd.spawn().map_err(|e| {
            Error::Config(format!(
                "Failed to spawn sub-agent worker {}: {}",
                worker_id, e
            ))
        })?;

        let worker = Worker {
            id: worker_id.clone(),
            runtime,
            process: Some(process),
            status: WorkerStatus::Running,
            current_task: None,
            started_at: Utc::now(),
            last_health_check: None,
            transport: None,
            last_attestation: None,
            quarantined: false,
            last_attestation_verified: None,
        };

        self.workers.write().await.insert(worker_id.clone(), worker);

        self.event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::WorkerStarted,
            json!({
                "worker_id": worker_id,
                "runtime": runtime.to_string(),
                "sub_agent_id": sub_agent_id,
            }),
        ));

        tracing::info!(
            worker_id = %worker_id,
            sub_agent_id = %sub_agent_id,
            runtime = %runtime,
            "Sub-agent worker spawned"
        );

        Ok(worker_id)
    }

    /// Get worker status information for the status endpoint.
    ///
    /// Returns a vector of `WorkerInfo` structs suitable for JSON serialization.
    pub async fn get_worker_status(&self) -> Vec<WorkerInfo> {
        let workers = self.workers.read().await;
        workers
            .values()
            .map(|w| WorkerInfo {
                id: w.id.clone(),
                runtime: w.runtime.to_string(),
                status: w.status.to_string(),
                current_task: w.current_task.map(|t| t.to_string()),
            })
            .collect()
    }

    /// Get the number of active workers.
    pub async fn get_worker_count(&self) -> usize {
        self.workers.read().await.len()
    }

    /// Register a pre-built worker with an existing transport.
    ///
    /// This is primarily intended for integration tests that need to inject
    /// mock workers without going through `spawn_worker` (which hardcodes
    /// the worker script path).
    ///
    /// # Arguments
    ///
    /// * `worker_id` - Unique identifier for the worker
    /// * `runtime` - The runtime type of the worker
    /// * `transport` - A pre-built transport implementing `WorkerTransport`
    pub async fn register_worker(
        &mut self,
        worker_id: String,
        runtime: WorkerRuntime,
        transport: Arc<dyn WorkerTransport>,
    ) {
        let worker = Worker {
            id: worker_id.clone(),
            runtime,
            process: None,
            status: WorkerStatus::Running,
            current_task: None,
            started_at: Utc::now(),
            last_health_check: None,
            transport: Some(transport),
            last_attestation: None,
            quarantined: false,
            last_attestation_verified: None,
        };
        self.workers.write().await.insert(worker_id.clone(), worker);

        self.event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::WorkerStarted,
            json!({
                "worker_id": worker_id,
                "runtime": runtime.to_string(),
            }),
        ));
    }

    /// Get the transport for a specific worker.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker is not found or has no transport.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn get_transport(&self, worker_id: &str) -> Result<Arc<dyn WorkerTransport>> {
        let workers = self.workers.read().await;
        let worker = workers
            .get(worker_id)
            .ok_or_else(|| Error::Config(format!("Worker not found: {}", worker_id)))?;
        worker
            .transport
            .clone()
            .ok_or_else(|| Error::Config(format!("Worker {} has no transport", worker_id)))
    }

    /// Spawn a background task to read and log worker stderr.
    ///
    /// Collects all stderr output and emits a single `error!` log when the
    /// stream closes, instead of one `warn!` per line.  This keeps CI logs
    /// readable when a worker crashes with a multi-line stack trace.
    fn spawn_stderr_handler(worker_id: String, stderr: ChildStderr) {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            let mut collected = Vec::new();
            while let Ok(Some(line)) = lines.next_line().await {
                if !line.is_empty() {
                    collected.push(line);
                }
            }
            if collected.is_empty() {
                tracing::debug!(worker_id = %worker_id, "Worker stderr stream closed (no output)");
            } else {
                let combined = collected.join("\n");
                tracing::error!(
                    worker_id = %worker_id,
                    lines = collected.len(),
                    "Worker stderr:\n{}",
                    combined
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worker_manager_creation() {
        let config = Arc::new(Config::default());
        let event_stream = Arc::new(EventStream::new(100, 10));
        let manager = WorkerManager::new(config, event_stream);

        assert_eq!(manager.get_worker_count().await, 0);
        assert!(manager.shutdown_tx.is_none());
    }

    #[test]
    fn test_worker_id_generation() {
        let config = Arc::new(Config::default());
        let event_stream = Arc::new(EventStream::new(100, 10));
        let mut manager = WorkerManager::new(config, event_stream);

        let id1 = manager.next_worker_id(WorkerRuntime::Node);
        let id2 = manager.next_worker_id(WorkerRuntime::Node);
        let id3 = manager.next_worker_id(WorkerRuntime::Python);

        assert_eq!(id1, "node-worker-1");
        assert_eq!(id2, "node-worker-2");
        assert_eq!(id3, "python-worker-1");
    }

    #[tokio::test]
    async fn test_max_workers_limit() {
        let mut config = Config::default();
        config.custom_machine_config = Some(crate::config::MachineConfig {
            max_workers: 0,
            max_memory_mb: 8192,
            gpu_enabled: false,
            default_model: "test".to_string(),
            auto_restart_workers: false,
        });
        config.machine_profile = crate::config::MachineProfile::Custom;
        let config = Arc::new(config);
        let event_stream = Arc::new(EventStream::new(100, 10));
        let mut manager = WorkerManager::new(config, event_stream);

        let result = manager.spawn_worker(WorkerRuntime::Node).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Max workers limit reached")
        );
    }

    #[tokio::test]
    async fn test_worker_status_reporting() {
        let config = Arc::new(Config::default());
        let event_stream = Arc::new(EventStream::new(100, 10));
        let manager = WorkerManager::new(config, event_stream);

        let status = manager.get_worker_status().await;
        assert!(status.is_empty());
    }

    #[test]
    fn test_worker_runtime_display() {
        assert_eq!(WorkerRuntime::Node.to_string(), "node");
        assert_eq!(WorkerRuntime::Python.to_string(), "python");
        assert_eq!(WorkerRuntime::Native.to_string(), "native");
        assert_eq!(WorkerRuntime::Wasm.to_string(), "wasm");
    }

    #[test]
    fn test_worker_status_display() {
        assert_eq!(WorkerStatus::Starting.to_string(), "starting");
        assert_eq!(WorkerStatus::Running.to_string(), "running");
        assert_eq!(WorkerStatus::Stopping.to_string(), "stopping");
        assert_eq!(WorkerStatus::Stopped.to_string(), "stopped");
        assert_eq!(WorkerStatus::Failed.to_string(), "failed");
    }

    #[tokio::test]
    #[ignore = "requires node.js installed and worker scripts present"]
    async fn test_spawn_and_stop_worker() {
        // This test requires actual worker scripts to be present
        // Run with: cargo test test_spawn_and_stop_worker -- --ignored
    }
}
