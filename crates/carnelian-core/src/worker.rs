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
use crate::server::WorkerInfo;
use carnelian_common::types::{
    CancelRequest, EventEnvelope, EventLevel, EventType, HealthResponse, InvokeRequest,
    InvokeResponse, InvokeStatus, RunId, StreamEvent, TransportMessage,
};
use carnelian_common::{Error, Result};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
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
    /// Shell worker
    Shell,
    /// WebAssembly worker (future)
    Wasm,
}

impl std::fmt::Display for WorkerRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Node => write!(f, "node"),
            Self::Python => write!(f, "python"),
            Self::Shell => write!(f, "shell"),
            Self::Wasm => write!(f, "wasm"),
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

        // Spawn background stdout reader that demuxes responses to per-run oneshot senders
        let reader_runs = active_runs.clone();
        let reader_worker_id = worker_id.clone();
        let reader_config = config.clone();
        tokio::spawn(async move {
            Self::read_stdout_loop(stdout, reader_worker_id, reader_runs, reader_config).await;
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
                Ok(msg) => {
                    // Other message types (HealthResult, etc.) — log and discard
                    tracing::debug!(
                        worker_id = %worker_id,
                        msg_type = ?std::mem::discriminant(&msg),
                        "Received non-invoke/stream message from worker"
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
        stdin.write_all(data).await.map_err(|e| {
            Error::Connection(format!("Failed to write to worker stdin: {e}"))
        })?;
        stdin.flush().await.map_err(|e| {
            Error::Connection(format!("Failed to flush worker stdin: {e}"))
        })?;
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
                let _ =
                    nix::sys::signal::kill(nix_pid, nix::sys::signal::Signal::SIGTERM);
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
        let wait_result = self.process.lock().await.try_wait();
        let healthy = match wait_result {
            Ok(Some(_)) | Err(_) => false,
            Ok(None) => true,
        };

        Ok(HealthResponse {
            healthy,
            worker_id: self.worker_id.clone(),
            uptime_secs: self.created_at.elapsed().as_secs(),
        })
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
    let (healthy, runtime, exit_code) = {
        let mut w = workers.write().await;
        let worker = w
            .get_mut(worker_id)
            .ok_or_else(|| Error::Config(format!("Worker not found: {}", worker_id)))?;

        let runtime = worker.runtime;

        // If the worker has a transport, use it for health checks
        if let Some(ref transport) = worker.transport {
            match transport.health().await {
                Ok(resp) if resp.healthy => {
                    worker.last_health_check = Some(Utc::now());
                    (true, runtime, None)
                }
                Ok(_) => {
                    tracing::error!(
                        worker_id = %worker_id,
                        "Worker process exited unexpectedly (transport health check)"
                    );
                    worker.status = WorkerStatus::Failed;
                    (false, runtime, None)
                }
                Err(e) => {
                    tracing::warn!(
                        worker_id = %worker_id,
                        error = %e,
                        "Failed to check worker health via transport"
                    );
                    (false, runtime, None)
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
                    (false, runtime, Some(status.code()))
                }
                Ok(None) => {
                    worker.last_health_check = Some(Utc::now());
                    (true, runtime, None)
                }
                Err(e) => {
                    tracing::warn!(
                        worker_id = %worker_id,
                        error = %e,
                        "Failed to check worker health"
                    );
                    (false, runtime, None)
                }
            }
        } else {
            // No process and no transport — mark as failed
            worker.status = WorkerStatus::Failed;
            (false, runtime, None)
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
        }
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
    /// - The max_workers limit has been reached
    /// - The process fails to spawn
    pub async fn spawn_worker(&mut self, runtime: WorkerRuntime) -> Result<String> {
        let max_workers = self.config.machine_config().max_workers;
        let current_count = self.workers.read().await.len();

        if current_count >= max_workers as usize {
            return Err(Error::Config(format!(
                "Max workers limit reached ({}/{})",
                current_count, max_workers
            )));
        }

        let worker_id = self.next_worker_id(runtime);
        let api_url = format!("http://localhost:{}", self.config.http_port);

        let mut cmd = match runtime {
            WorkerRuntime::Node => {
                let mut c = tokio::process::Command::new("node");
                c.args(["workers/node-worker/index.js"]);
                c
            }
            WorkerRuntime::Python => {
                let mut c = tokio::process::Command::new("python");
                c.args(["workers/python-worker/worker.py"]);
                c
            }
            WorkerRuntime::Shell => {
                let mut c = tokio::process::Command::new("bash");
                c.args(["workers/shell-worker/worker.sh"]);
                c
            }
            WorkerRuntime::Wasm => {
                return Err(Error::Config("Wasm runtime not yet supported".to_string()));
            }
        };

        cmd.env("WORKER_ID", &worker_id)
            .env("CARNELIAN_API_URL", &api_url)
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
    /// Spawns Node.js workers by default. Logs errors for individual
    /// worker spawn failures but continues starting remaining workers.
    pub async fn start_workers(&mut self) -> Result<()> {
        let max_workers = self.config.machine_config().max_workers;
        let mut started = 0u32;

        for _ in 0..max_workers {
            match self.spawn_worker(WorkerRuntime::Node).await {
                Ok(id) => {
                    tracing::info!(worker_id = %id, "Worker started successfully");
                    started += 1;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to start worker, continuing with remaining");
                }
            }
        }

        tracing::info!(
            total_started = started,
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
        self.spawn_worker(runtime).await
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
    /// The loop responds to the shutdown signal for graceful termination.
    fn start_health_check_loop(&mut self) {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        self.shutdown_tx = Some(shutdown_tx);

        let workers = self.workers.clone();
        let event_stream = self.event_stream.clone();

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

    /// Get worker status information for the status endpoint.
    ///
    /// Returns a vector of `WorkerInfo` structs suitable for JSON serialization.
    pub async fn get_worker_status(&self) -> Vec<WorkerInfo> {
        let workers = self.workers.read().await;
        workers
            .values()
            .map(|w| WorkerInfo {
                id: w.id.clone(),
                status: w.status.to_string(),
                current_task: w.current_task.map(|t| t.to_string()),
            })
            .collect()
    }

    /// Get the number of active workers.
    pub async fn get_worker_count(&self) -> usize {
        self.workers.read().await.len()
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
    fn spawn_stderr_handler(worker_id: String, stderr: ChildStderr) {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::warn!(worker_id = %worker_id, stream = "stderr", "{}", line);
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
        assert_eq!(WorkerRuntime::Shell.to_string(), "shell");
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
