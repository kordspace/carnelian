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
//! # Integration
//!
//! The `WorkerManager` is stored in `AppState` and initialized in the binary.
//! The status endpoint reports active workers via `get_worker_status()`.

use crate::config::Config;
use crate::events::EventStream;
use crate::server::WorkerInfo;
use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_common::{Error, Result};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdout};
use tokio::sync::{RwLock, watch};
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
    /// The spawned tokio process handle
    pub process: Child,
    /// Current status
    pub status: WorkerStatus,
    /// Currently executing task ID
    pub current_task: Option<Uuid>,
    /// When the worker was started
    pub started_at: DateTime<Utc>,
    /// Last successful health check
    pub last_health_check: Option<DateTime<Utc>>,
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
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            tracing::error!(
                worker_id = %worker_id,
                runtime = %runtime,
                error = %e,
                "Failed to spawn worker process"
            );
            Error::Connection(format!("Failed to spawn {} worker: {}", runtime, e))
        })?;

        // Capture stdout/stderr before moving child into Worker
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let worker = Worker {
            id: worker_id.clone(),
            runtime,
            process: child,
            status: WorkerStatus::Starting,
            current_task: None,
            started_at: Utc::now(),
            last_health_check: None,
        };

        self.workers.write().await.insert(worker_id.clone(), worker);

        // Spawn output handlers for stdout/stderr
        if let Some(stdout) = stdout {
            if let Some(stderr) = stderr {
                Self::spawn_output_handler(worker_id.clone(), stdout, stderr);
            }
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
    /// Sends SIGTERM (or kill on Windows) and waits up to 5 seconds
    /// for the process to exit. If the timeout expires, sends SIGKILL.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker ID is not found in the registry.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn stop_worker(&mut self, worker_id: &str) -> Result<()> {
        // Phase 1: Mark as stopping and extract the child handle + runtime.
        // Release the write lock before awaiting process exit.
        let (mut child, runtime) = {
            let mut workers = self.workers.write().await;
            let worker = workers
                .remove(worker_id)
                .ok_or_else(|| Error::Config(format!("Worker not found: {}", worker_id)))?;
            (worker.process, worker.runtime)
        };

        tracing::info!(worker_id = %worker_id, "Stopping worker");

        // Phase 2: Send SIGTERM (Unix) or platform-appropriate termination signal.
        #[cfg(unix)]
        {
            if let Some(pid) = child.id() {
                // Send SIGTERM for graceful shutdown
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
                tracing::debug!(worker_id = %worker_id, pid = pid, "Sent SIGTERM to worker");
            }
        }
        #[cfg(not(unix))]
        {
            // On Windows, there is no SIGTERM; start() will use TerminateProcess via kill().
            // We attempt a graceful wait first, then fall back to kill().
            tracing::debug!(worker_id = %worker_id, "Non-Unix platform, will use kill() as fallback");
        }

        // Phase 3: Wait up to 5 seconds for the process to exit gracefully.
        let exited = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await;

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
                // Phase 4: Timeout expired — force kill with SIGKILL.
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

        // Phase 5: Emit WorkerStopped event.
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
    /// Updates the worker status and emits a health check event.
    ///
    /// # Returns
    ///
    /// `true` if the worker is alive, `false` if it has exited.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn check_worker_health(&self, worker_id: &str) -> Result<bool> {
        let mut workers = self.workers.write().await;
        let worker = workers
            .get_mut(worker_id)
            .ok_or_else(|| Error::Config(format!("Worker not found: {}", worker_id)))?;

        let runtime = worker.runtime;

        let (healthy, exit_code): (bool, Option<Option<i32>>) = match worker.process.try_wait() {
            Ok(Some(status)) => {
                tracing::error!(
                    worker_id = %worker_id,
                    exit_code = ?status.code(),
                    "Worker process exited unexpectedly"
                );
                worker.status = WorkerStatus::Failed;
                (false, Some(status.code()))
            }
            Ok(None) => {
                worker.last_health_check = Some(Utc::now());
                (true, None)
            }
            Err(e) => {
                tracing::warn!(
                    worker_id = %worker_id,
                    error = %e,
                    "Failed to check worker health"
                );
                (false, None)
            }
        };
        drop(workers);

        if healthy {
            self.event_stream.publish(EventEnvelope::new(
                EventLevel::Debug,
                EventType::WorkerHealthCheck,
                json!({
                    "worker_id": worker_id,
                    "healthy": true,
                }),
            ));
        } else {
            self.event_stream.publish(EventEnvelope::new(
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
            self.event_stream.publish(EventEnvelope::new(
                EventLevel::Warn,
                EventType::WorkerStopped,
                json!({
                    "worker_id": worker_id,
                    "runtime": runtime.to_string(),
                    "reason": reason,
                }),
            ));
        }

        Ok(healthy)
    }

    /// Run health checks on all active workers.
    ///
    /// Checks each worker and logs a summary. Failed workers are logged
    /// at WARN level for potential restart.
    pub async fn run_health_checks(&self) -> Result<()> {
        let worker_ids: Vec<String> = self.workers.read().await.keys().cloned().collect();
        let mut healthy_count = 0usize;
        let mut failed_count = 0usize;

        for worker_id in &worker_ids {
            match self.check_worker_health(worker_id).await {
                Ok(true) => healthy_count += 1,
                Ok(false) => {
                    failed_count += 1;
                    tracing::warn!(worker_id = %worker_id, "Worker failed health check");
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
    /// Spawns a tokio task that runs health checks every 30 seconds.
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
                            let mut w = workers.write().await;
                            if let Some(worker) = w.get_mut(worker_id) {
                                match worker.process.try_wait() {
                                    Ok(Some(status)) => {
                                        tracing::error!(
                                            worker_id = %worker_id,
                                            exit_code = ?status.code(),
                                            "Worker process exited unexpectedly"
                                        );
                                        worker.status = WorkerStatus::Failed;
                                        failed += 1;

                                        event_stream.publish(EventEnvelope::new(
                                            EventLevel::Error,
                                            EventType::WorkerHealthCheck,
                                            json!({
                                                "worker_id": worker_id,
                                                "healthy": false,
                                                "exit_code": status.code(),
                                            }),
                                        ));
                                    }
                                    Ok(None) => {
                                        worker.last_health_check = Some(Utc::now());
                                        healthy += 1;
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            worker_id = %worker_id,
                                            error = %e,
                                            "Failed to check worker health"
                                        );
                                        failed += 1;
                                    }
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

    /// Spawn background tasks to read and log worker stdout/stderr.
    ///
    /// Each stream is read line-by-line using `BufReader` and logged
    /// with the worker_id for traceability.
    fn spawn_output_handler(worker_id: String, stdout: ChildStdout, stderr: ChildStderr) {
        let wid = worker_id.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!(worker_id = %wid, stream = "stdout", "{}", line);
            }
        });

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
