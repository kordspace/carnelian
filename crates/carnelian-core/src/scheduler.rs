//! Task scheduler and heartbeat runner for 🔥 Carnelian OS
//!
//! The Scheduler manages background tasks including:
//! - **Heartbeat System**: Periodic heartbeats at configurable intervals (default 555,555ms ≈ 9.26 minutes)
//! - **Mantra Selection**: "First unknown, then random rotation" strategy for selecting mantras
//! - **Task Queue Polling**: Priority-based dequeuing with concurrency limits
//! - **Task Execution**: Skill invocation via `WorkerManager` transports with timeout enforcement
//! - **Retry Policy**: Configurable retry attempts with delay between failures
//! - **Task Cancellation**: Graceful cancellation of running tasks with cleanup
//!
//! # Heartbeat Interval
//!
//! The default interval of 555,555ms is configurable via:
//! - `heartbeat_interval_ms` in `machine.toml`
//! - `CARNELIAN_HEARTBEAT_INTERVAL_MS` environment variable
//!
//! # Mantra Selection Strategy
//!
//! Mantras are selected using a "first unknown, then random rotation" approach:
//! 1. Query previously used mantras for the identity
//! 2. If any mantras haven't been used yet, select the first unknown one
//! 3. Once all mantras have been used, randomly select from the full rotation
//!
//! # Task Queue Polling
//!
//! Tasks are dequeued from the `tasks` table ordered by priority (DESC) then
//! created_at (ASC). The scheduler respects the `max_workers` concurrency limit
//! from the machine profile, only dequeuing tasks when execution slots are available.
//!
//! # Retry Policy
//!
//! Failed tasks are retried up to `task_max_retry_attempts` times (default: 3).
//! Between retries, the task is reset to 'pending' after a configurable delay
//! (`task_retry_delay_secs`, default: 5s). Once retries are exhausted, the task
//! is permanently marked as 'failed'.
//!
//! # Task Cancellation
//!
//! Running tasks can be cancelled via `cancel_task()`. The scheduler sends a
//! cancellation signal to the worker transport, waits for the grace period,
//! then force-aborts if necessary.
//!
//! # Integration with Event Stream
//!
//! Each heartbeat emits a `HeartbeatTick` event containing:
//! - `heartbeat_id`: Database record ID
//! - `identity_id`: The identity performing the heartbeat (Lian)
//! - `mantra`: Selected mantra for this heartbeat
//! - `tasks_queued`: Number of pending tasks in the queue
//! - `duration_ms`: Time taken to execute the heartbeat
//!
//! Task lifecycle events are emitted for: `TaskStarted`, `TaskCompleted`,
//! `TaskFailed`, `TaskCancelled`, and `Custom("TaskRetryScheduled")`.
//!
//! # Graceful Shutdown
//!
//! The scheduler responds to shutdown signals and cleanly terminates the heartbeat loop.
//! Call `shutdown()` before stopping the server to ensure proper cleanup.

use crate::config::Config;
use crate::events::EventStream;
use crate::worker::WorkerManager;
use carnelian_common::types::{
    EventEnvelope, EventLevel, EventType, InvokeRequest, InvokeStatus, RunId,
};
use carnelian_common::{Error, Result};
use rand::seq::SliceRandom;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use uuid::Uuid;

/// Static list of mantras for the heartbeat system
const MANTRAS: &[&str] = &[
    "What wants to emerge?",
    "Be present and authentic",
    "Share a brief thought",
    "Notice what's alive",
    "Trust the process",
];

/// Background task scheduler managing heartbeats, task queue polling, and task execution.
///
/// The Scheduler runs as a background tokio task, periodically executing
/// heartbeats and polling the task queue for pending work. It manages
/// concurrency limits, retry policies, and task cancellation.
pub struct Scheduler {
    /// Database connection pool
    pool: PgPool,
    /// Event stream for publishing heartbeat and task lifecycle events
    event_stream: Arc<EventStream>,
    /// Interval between heartbeats
    heartbeat_interval: Duration,
    /// Shutdown signal sender
    shutdown_tx: Option<watch::Sender<bool>>,
    /// Worker manager for skill execution via transports
    worker_manager: Arc<tokio::sync::Mutex<WorkerManager>>,
    /// Application configuration for retry policy and concurrency limits
    config: Arc<Config>,
    /// Active task execution handles keyed by task_id for cancellation support
    pub active_tasks: Arc<tokio::sync::Mutex<HashMap<Uuid, tokio::task::JoinHandle<()>>>>,
}

impl Scheduler {
    /// Create a new Scheduler instance.
    ///
    /// # Arguments
    ///
    /// * `pool` - Database connection pool for heartbeat logging and task queries
    /// * `event_stream` - Event stream for publishing heartbeat and task lifecycle events
    /// * `heartbeat_interval` - Duration between heartbeat ticks
    /// * `worker_manager` - Worker manager for skill execution via transports
    /// * `config` - Application configuration for retry policy and concurrency limits
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::time::Duration;
    /// let scheduler = Scheduler::new(
    ///     pool, event_stream, Duration::from_millis(555_555),
    ///     worker_manager, config,
    /// );
    /// ```
    pub fn new(
        pool: PgPool,
        event_stream: Arc<EventStream>,
        heartbeat_interval: Duration,
        worker_manager: Arc<tokio::sync::Mutex<WorkerManager>>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            pool,
            event_stream,
            heartbeat_interval,
            shutdown_tx: None,
            worker_manager,
            config,
            active_tasks: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Start the scheduler background task.
    ///
    /// This method spawns a background tokio task that runs the heartbeat loop
    /// at the configured interval. The method returns immediately (non-blocking).
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after spawning the background task.
    ///
    /// # Errors
    ///
    /// This method does not return errors directly, but the background task
    /// will log errors if heartbeat execution fails.
    #[allow(clippy::unused_async)]
    pub async fn start(&mut self) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        self.shutdown_tx = Some(shutdown_tx);

        let pool = self.pool.clone();
        let event_stream = self.event_stream.clone();
        let interval = self.heartbeat_interval;
        let worker_manager = self.worker_manager.clone();
        let config = self.config.clone();
        let active_tasks = self.active_tasks.clone();

        tokio::spawn(async move {
            Self::run_heartbeat_loop(
                pool,
                event_stream,
                interval,
                shutdown_rx,
                worker_manager,
                config,
                active_tasks,
            )
            .await;
        });

        tracing::info!(
            heartbeat_interval_ms = interval.as_millis() as u64,
            "Scheduler started"
        );

        Ok(())
    }

    /// Shutdown the scheduler gracefully.
    ///
    /// Sends a shutdown signal to the background task and waits for it to terminate.
    #[allow(clippy::unused_async)]
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(true);
            tracing::info!("Scheduler shutdown signal sent");
        }
        Ok(())
    }

    /// Run the heartbeat loop until shutdown signal is received.
    async fn run_heartbeat_loop(
        pool: PgPool,
        event_stream: Arc<EventStream>,
        interval: Duration,
        mut shutdown_rx: watch::Receiver<bool>,
        worker_manager: Arc<tokio::sync::Mutex<WorkerManager>>,
        config: Arc<Config>,
        active_tasks: Arc<tokio::sync::Mutex<HashMap<Uuid, tokio::task::JoinHandle<()>>>>,
    ) {
        let mut ticker = tokio::time::interval(interval);
        // Skip the first immediate tick
        ticker.tick().await;

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(e) = Self::run_heartbeat(&pool, &event_stream).await {
                        tracing::warn!(error = %e, "Heartbeat execution failed");
                    }
                    // Poll task queue after heartbeat
                    if let Err(e) = Self::poll_task_queue(
                        &pool,
                        &event_stream,
                        &worker_manager,
                        &config,
                        &active_tasks,
                    ).await {
                        tracing::warn!(error = %e, "Task queue polling failed");
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("Scheduler received shutdown signal, stopping heartbeat loop");
                        break;
                    }
                }
            }
        }
    }

    /// Execute a single heartbeat cycle.
    ///
    /// This method:
    /// 1. Queries the database for the default identity (Lian)
    /// 2. Selects a mantra using the "first unknown, then random" strategy
    /// 3. Counts pending tasks in the queue
    /// 4. Logs the heartbeat to `heartbeat_history`
    /// 5. Emits a `HeartbeatTick` event
    async fn run_heartbeat(pool: &PgPool, event_stream: &EventStream) -> Result<()> {
        let start = std::time::Instant::now();

        // Query for default identity (Lian)
        let identity_id: Option<Uuid> = sqlx::query_scalar(
            r"SELECT identity_id FROM identities WHERE name = 'Lian' AND identity_type = 'core' LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)?;

        let identity_id = match identity_id {
            Some(id) => id,
            None => {
                tracing::error!("Default identity 'Lian' not found in database");
                return Err(Error::Database(sqlx::Error::RowNotFound));
            }
        };

        // Select mantra
        let mantra = Self::select_mantra(pool, identity_id).await?;

        // Count pending tasks
        let tasks_queued: i64 = sqlx::query_scalar::<_, Option<i64>>(
            r"SELECT COUNT(*) FROM tasks WHERE state = 'pending'",
        )
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?
        .unwrap_or(0);

        let duration_ms = start.elapsed().as_millis() as i32;

        // Insert heartbeat record
        let heartbeat_id: Uuid = sqlx::query_scalar(
            r"
            INSERT INTO heartbeat_history (identity_id, mantra, tasks_queued, status, duration_ms)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING heartbeat_id
            ",
        )
        .bind(identity_id)
        .bind(&mantra)
        .bind(tasks_queued as i32)
        .bind("ok")
        .bind(duration_ms)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;

        // Emit HeartbeatTick event
        event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::HeartbeatTick,
            json!({
                "heartbeat_id": heartbeat_id,
                "identity_id": identity_id,
                "mantra": mantra,
                "tasks_queued": tasks_queued,
                "duration_ms": duration_ms,
                "status": "ok"
            }),
        ));

        tracing::info!(
            heartbeat_id = %heartbeat_id,
            identity_id = %identity_id,
            mantra = ?mantra,
            tasks_queued = tasks_queued,
            duration_ms = duration_ms,
            "Heartbeat completed"
        );

        Ok(())
    }

    /// Select a mantra using "first unknown, then random rotation" strategy.
    ///
    /// # Strategy
    ///
    /// 1. Query previously used mantras for this identity
    /// 2. Find mantras not yet used (set difference)
    /// 3. If unknown mantras exist, return the first one
    /// 4. Otherwise, randomly select from the full rotation
    async fn select_mantra(pool: &PgPool, identity_id: Uuid) -> Result<Option<String>> {
        // Query used mantras
        let used_mantras: Vec<String> = sqlx::query_scalar(
            r"SELECT DISTINCT mantra FROM heartbeat_history WHERE identity_id = $1 AND mantra IS NOT NULL",
        )
        .bind(identity_id)
        .fetch_all(pool)
        .await
        .map_err(Error::Database)?;

        // Find unknown mantras (not yet used)
        let unknown: Vec<&str> = MANTRAS
            .iter()
            .copied()
            .filter(|m| !used_mantras.iter().any(|u| u == *m))
            .collect();

        if !unknown.is_empty() {
            // Return first unknown mantra
            return Ok(Some(unknown[0].to_string()));
        }

        // All mantras used, select randomly
        let mut rng = rand::thread_rng();
        Ok(MANTRAS.choose(&mut rng).map(|s| (*s).to_string()))
    }

    /// Poll the task queue for pending work and dispatch tasks to workers.
    ///
    /// Queries pending tasks ordered by priority (DESC) and created_at (ASC),
    /// checks available concurrency slots, and spawns execution handlers for
    /// each dequeued task.
    ///
    /// # Concurrency Model
    ///
    /// Uses a slot-based model: `available = max_workers - active_count`.
    /// Only dequeues up to the number of available slots.
    pub async fn poll_task_queue(
        pool: &PgPool,
        event_stream: &Arc<EventStream>,
        worker_manager: &Arc<tokio::sync::Mutex<WorkerManager>>,
        config: &Arc<Config>,
        active_tasks: &Arc<tokio::sync::Mutex<HashMap<Uuid, tokio::task::JoinHandle<()>>>>,
    ) -> Result<()> {
        // Check concurrency: count active tasks
        let active_count = active_tasks.lock().await.len();
        let max_workers = config.machine_config().max_workers as usize;

        if active_count >= max_workers {
            tracing::debug!(
                active_count = active_count,
                max_workers = max_workers,
                "All execution slots occupied, skipping dequeue"
            );
            return Ok(());
        }

        let available_slots = max_workers - active_count;

        // Query pending tasks ordered by priority DESC, created_at ASC
        let pending_tasks: Vec<(Uuid, Option<Uuid>, i32)> = sqlx::query_as(
            r"SELECT task_id, skill_id, priority FROM tasks WHERE state = 'pending' ORDER BY priority DESC, created_at ASC LIMIT $1",
        )
        .bind(i64::try_from(available_slots).unwrap_or(i64::MAX))
        .fetch_all(pool)
        .await
        .map_err(Error::Database)?;

        tracing::debug!(
            pending_count = pending_tasks.len(),
            available_slots = available_slots,
            "Polled task queue"
        );

        for (task_id, skill_id, priority) in pending_tasks {
            tracing::debug!(
                task_id = %task_id,
                skill_id = ?skill_id,
                priority = priority,
                "Dequeuing task for execution"
            );

            // Spawn async task execution handler
            let pool = pool.clone();
            let event_stream = event_stream.clone();
            let worker_manager = worker_manager.clone();
            let config = config.clone();
            let active_tasks_clone = active_tasks.clone();

            let handle = tokio::spawn(async move {
                if let Err(e) = Self::execute_task(
                    task_id,
                    skill_id,
                    &pool,
                    &event_stream,
                    &worker_manager,
                    &config,
                    &active_tasks_clone,
                )
                .await
                {
                    tracing::error!(
                        task_id = %task_id,
                        error = %e,
                        "Task execution failed with unhandled error"
                    );
                    // Ensure task is marked as failed on unhandled errors;
                    // persist the error message in description for diagnostics.
                    let error_msg = format!("execute_task error: {e}");
                    let _ = sqlx::query(
                        r"UPDATE tasks SET state = 'failed', description = $2, updated_at = NOW() WHERE task_id = $1",
                    )
                    .bind(task_id)
                    .bind(&error_msg)
                    .execute(&pool)
                    .await;
                }

                // Remove from active_tasks on completion
                active_tasks_clone.lock().await.remove(&task_id);
            });

            // Store handle for cancellation support
            active_tasks.lock().await.insert(task_id, handle);
        }

        Ok(())
    }

    /// Execute a single task: update state, invoke skill, track metrics, handle retries.
    ///
    /// # Lifecycle
    ///
    /// 1. Set task state to 'running'
    /// 2. Create `task_runs` record with attempt number
    /// 3. Emit `TaskStarted` event
    /// 4. Look up skill details and get worker transport
    /// 5. Invoke skill via transport
    /// 6. Update `task_runs` with result metrics
    /// 7. Update task state based on outcome
    /// 8. Handle retry logic on failure
    /// 9. Emit completion/failure event
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    async fn execute_task(
        task_id: Uuid,
        skill_id: Option<Uuid>,
        pool: &PgPool,
        event_stream: &Arc<EventStream>,
        worker_manager: &Arc<tokio::sync::Mutex<WorkerManager>>,
        config: &Arc<Config>,
        active_tasks: &Arc<tokio::sync::Mutex<HashMap<Uuid, tokio::task::JoinHandle<()>>>>,
    ) -> Result<()> {
        let exec_start = std::time::Instant::now();
        eprintln!("[execute_task] START task_id={task_id} skill_id={skill_id:?}");

        // Update task state to 'running'
        sqlx::query(r"UPDATE tasks SET state = 'running', updated_at = NOW() WHERE task_id = $1")
            .bind(task_id)
            .execute(pool)
            .await
            .map_err(|e| {
                eprintln!("[execute_task] FAIL at SET running: {e}");
                Error::Database(e)
            })?;
        eprintln!("[execute_task] OK set running");

        // Determine attempt number from existing task_runs
        let attempt: i64 = sqlx::query_scalar::<_, Option<i64>>(
            r"SELECT COALESCE(MAX(attempt), 0) FROM task_runs WHERE task_id = $1",
        )
        .bind(task_id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            eprintln!("[execute_task] FAIL at attempt query: {e}");
            Error::Database(e)
        })?
        .unwrap_or(0)
            + 1;
        eprintln!("[execute_task] OK attempt={attempt}");

        // Create task_run record (worker_id stores skill name for usage tracking)
        let run_id = RunId::new();
        sqlx::query(
            r"INSERT INTO task_runs (run_id, task_id, attempt, state, started_at, worker_id)
              VALUES ($1, $2, $3, 'running', NOW(), $4)",
        )
        .bind(run_id.0)
        .bind(task_id)
        .bind(attempt as i32)
        .bind(skill_id.map(|s| s.to_string()))
        .execute(pool)
        .await
        .map_err(|e| {
            eprintln!("[execute_task] FAIL at INSERT task_run: {e}");
            Error::Database(e)
        })?;
        eprintln!("[execute_task] OK task_run inserted run_id={}", run_id.0);

        // Emit TaskStarted event
        event_stream.publish(
            EventEnvelope::new(
                EventLevel::Info,
                EventType::TaskStarted,
                json!({
                    "task_id": task_id,
                    "skill_id": skill_id,
                    "attempt": attempt,
                    "run_id": run_id.0,
                }),
            )
            .with_actor_id(task_id.to_string()),
        );

        tracing::info!(
            task_id = %task_id,
            skill_id = ?skill_id,
            attempt = attempt,
            run_id = %run_id.0,
            "Task execution started"
        );

        // TODO: Phase 4 - Implement capability checking
        // For now, all skills are allowed to execute
        // Future: Check skill.capabilities_required against granted capabilities
        // Future: Verify capability constraints (scope, rate limits, etc.)

        // Get skill details
        let skill_info: Option<(String, String)> = if let Some(sid) = skill_id {
            sqlx::query_as(
                r"SELECT name, runtime FROM skills WHERE skill_id = $1 AND enabled = true",
            )
            .bind(sid)
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?
        } else {
            None
        };

        eprintln!("[execute_task] skill_info={skill_info:?}");
        let (skill_name, _runtime) = match skill_info {
            Some(info) => info,
            None => {
                let error_msg = format!("Skill not found or disabled: {:?}", skill_id);
                tracing::warn!(task_id = %task_id, "{}", error_msg);

                // Update task_run as failed
                Self::update_task_run_failed(pool, run_id, &error_msg, exec_start).await?;
                // Handle retry or permanent failure
                Self::handle_task_failure(
                    task_id,
                    pool,
                    event_stream,
                    config,
                    active_tasks,
                    attempt,
                    &error_msg,
                )
                .await?;
                return Ok(());
            }
        };

        // Get task description as input payload
        let task_description: Option<String> =
            sqlx::query_scalar(r"SELECT description FROM tasks WHERE task_id = $1")
                .bind(task_id)
                .fetch_optional(pool)
                .await
                .map_err(Error::Database)?;

        let input = task_description.map_or_else(|| json!({}), |desc| json!({"description": desc}));

        // Get a running worker's transport
        let transport = {
            let wm = worker_manager.lock().await;
            let workers = wm.get_worker_status().await;
            let mut found_transport = None;
            for w in &workers {
                if w.status == "running" {
                    match wm.get_transport(&w.id).await {
                        Ok(t) => {
                            found_transport = Some(t);
                            break;
                        }
                        Err(_) => continue,
                    }
                }
            }
            found_transport
        };
        eprintln!("[execute_task] transport found={}", transport.is_some());

        let transport = match transport {
            Some(t) => t,
            None => {
                let error_msg = "No running worker with transport available".to_string();
                tracing::warn!(task_id = %task_id, "{}", error_msg);

                Self::update_task_run_failed(pool, run_id, &error_msg, exec_start).await?;
                Self::handle_task_failure(
                    task_id,
                    pool,
                    event_stream,
                    config,
                    active_tasks,
                    attempt,
                    &error_msg,
                )
                .await?;
                return Ok(());
            }
        };

        // Invoke skill via transport
        let invoke_request = InvokeRequest {
            run_id,
            skill_name: skill_name.clone(),
            input,
            timeout_secs: config.skill_timeout_secs,
            correlation_id: None,
        };

        let response = transport.invoke(invoke_request).await;

        match response {
            Ok(resp) => {
                let duration_ms = exec_start.elapsed().as_millis() as i64;

                // Update task_run with result metrics
                let result_json = json!({
                    "result": resp.result,
                    "duration_ms": duration_ms,
                    "output_truncated": resp.truncated,
                });

                sqlx::query(
                    r"UPDATE task_runs SET state = $1, ended_at = NOW(), exit_code = $2, result = $3, worker_id = $4
                      WHERE run_id = $5",
                )
                .bind(match resp.status {
                    InvokeStatus::Success => "success",
                    InvokeStatus::Failed => "failed",
                    InvokeStatus::Timeout => "timeout",
                    InvokeStatus::Cancelled => "canceled",
                })
                .bind(resp.exit_code)
                .bind(&result_json)
                .bind(&skill_name)
                .bind(run_id.0)
                .execute(pool)
                .await
                .map_err(Error::Database)?;

                match resp.status {
                    InvokeStatus::Success => {
                        // Update task state to 'completed'
                        sqlx::query(
                            r"UPDATE tasks SET state = 'completed', updated_at = NOW() WHERE task_id = $1",
                        )
                        .bind(task_id)
                        .execute(pool)
                        .await
                        .map_err(Error::Database)?;

                        event_stream.publish(
                            EventEnvelope::new(
                                EventLevel::Info,
                                EventType::TaskCompleted,
                                json!({
                                    "task_id": task_id,
                                    "skill_name": skill_name,
                                    "duration_ms": duration_ms,
                                    "exit_code": resp.exit_code,
                                    "truncated": resp.truncated,
                                }),
                            )
                            .with_actor_id(task_id.to_string()),
                        );

                        tracing::info!(
                            task_id = %task_id,
                            skill_name = %skill_name,
                            duration_ms = duration_ms,
                            "Task completed successfully"
                        );
                    }
                    InvokeStatus::Failed | InvokeStatus::Timeout => {
                        let error_msg = resp
                            .error
                            .clone()
                            .unwrap_or_else(|| format!("Skill invocation {:?}", resp.status));

                        tracing::warn!(
                            task_id = %task_id,
                            skill_name = %skill_name,
                            status = ?resp.status,
                            error = %error_msg,
                            "Task execution failed"
                        );

                        // Persist error and skill info on the task_run record
                        let fail_result = json!({
                            "error": error_msg,
                            "skill_name": skill_name,
                            "status": format!("{:?}", resp.status),
                            "duration_ms": duration_ms,
                        });
                        let _ = sqlx::query(
                            r"UPDATE task_runs SET error = $1, result = $2, worker_id = $3
                              WHERE run_id = $4",
                        )
                        .bind(&error_msg)
                        .bind(&fail_result)
                        .bind(&skill_name)
                        .bind(run_id.0)
                        .execute(pool)
                        .await;

                        Self::handle_task_failure(
                            task_id,
                            pool,
                            event_stream,
                            config,
                            active_tasks,
                            attempt,
                            &error_msg,
                        )
                        .await?;
                    }
                    InvokeStatus::Cancelled => {
                        sqlx::query(
                            r"UPDATE tasks SET state = 'canceled', updated_at = NOW() WHERE task_id = $1",
                        )
                        .bind(task_id)
                        .execute(pool)
                        .await
                        .map_err(Error::Database)?;

                        event_stream.publish(
                            EventEnvelope::new(
                                EventLevel::Info,
                                EventType::TaskCancelled,
                                json!({
                                    "task_id": task_id,
                                    "reason": "cancelled_by_worker",
                                }),
                            )
                            .with_actor_id(task_id.to_string()),
                        );

                        tracing::info!(task_id = %task_id, "Task cancelled by worker");
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Transport invoke error: {}", e);
                tracing::error!(task_id = %task_id, error = %e, "Skill invocation failed");

                Self::update_task_run_failed(pool, run_id, &error_msg, exec_start).await?;
                Self::handle_task_failure(
                    task_id,
                    pool,
                    event_stream,
                    config,
                    active_tasks,
                    attempt,
                    &error_msg,
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Update a task_run record as failed with error details and duration.
    async fn update_task_run_failed(
        pool: &PgPool,
        run_id: RunId,
        error_msg: &str,
        exec_start: std::time::Instant,
    ) -> Result<()> {
        let duration_ms = exec_start.elapsed().as_millis() as i64;
        let result_json = json!({
            "error": error_msg,
            "duration_ms": duration_ms,
        });

        sqlx::query(
            r"UPDATE task_runs SET state = 'failed', ended_at = NOW(), result = $1, error = $2
              WHERE run_id = $3",
        )
        .bind(&result_json)
        .bind(error_msg)
        .bind(run_id.0)
        .execute(pool)
        .await
        .map_err(Error::Database)?;

        Ok(())
    }

    /// Handle task failure: apply retry policy or mark as permanently failed.
    ///
    /// If attempts < `task_max_retry_attempts`, immediately marks the task as
    /// 'retry_pending' and spawns a detached timer that resets it to 'pending'
    /// after the configured delay. This frees the worker slot immediately
    /// instead of sleeping inside the execution task.
    ///
    /// Once retries are exhausted, marks the task as permanently 'failed'.
    #[allow(clippy::too_many_arguments)]
    async fn handle_task_failure(
        task_id: Uuid,
        pool: &PgPool,
        event_stream: &Arc<EventStream>,
        config: &Arc<Config>,
        _active_tasks: &Arc<tokio::sync::Mutex<HashMap<Uuid, tokio::task::JoinHandle<()>>>>,
        attempt: i64,
        error_msg: &str,
    ) -> Result<()> {
        let max_attempts = i64::from(config.task_max_retry_attempts);

        if attempt < max_attempts {
            let retry_delay_secs = config.task_retry_delay_secs;

            tracing::info!(
                task_id = %task_id,
                attempt = attempt,
                max_attempts = max_attempts,
                retry_delay_secs = retry_delay_secs,
                "Scheduling task retry (slot released immediately)"
            );

            // Mark task as 'failed' immediately so it is not re-dequeued during the delay
            sqlx::query(
                r"UPDATE tasks SET state = 'failed', updated_at = NOW() WHERE task_id = $1",
            )
            .bind(task_id)
            .execute(pool)
            .await
            .map_err(Error::Database)?;

            // Emit retry-scheduled event now (before the detached delay)
            event_stream.publish(
                EventEnvelope::new(
                    EventLevel::Info,
                    EventType::Custom("TaskRetryScheduled".to_string()),
                    json!({
                        "task_id": task_id,
                        "attempt": attempt,
                        "max_attempts": max_attempts,
                        "retry_delay_secs": retry_delay_secs,
                        "error": error_msg,
                    }),
                )
                .with_actor_id(task_id.to_string()),
            );

            // Spawn a detached timer to reset the task to 'pending' after the delay.
            // This returns immediately, freeing the worker slot.
            let pool = pool.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(retry_delay_secs)).await;

                if let Err(e) = sqlx::query(
                    r"UPDATE tasks SET state = 'pending', updated_at = NOW() WHERE task_id = $1 AND state = 'failed'",
                )
                .bind(task_id)
                .execute(&pool)
                .await
                {
                    tracing::error!(
                        task_id = %task_id,
                        error = %e,
                        "Failed to reset task to pending after retry delay"
                    );
                } else {
                    tracing::debug!(
                        task_id = %task_id,
                        "Task reset to pending after retry delay"
                    );
                }
            });
        } else {
            tracing::warn!(
                task_id = %task_id,
                attempt = attempt,
                max_attempts = max_attempts,
                "Task retries exhausted, marking as permanently failed"
            );

            sqlx::query(
                r"UPDATE tasks SET state = 'failed', updated_at = NOW() WHERE task_id = $1",
            )
            .bind(task_id)
            .execute(pool)
            .await
            .map_err(Error::Database)?;

            event_stream.publish(
                EventEnvelope::new(
                    EventLevel::Warn,
                    EventType::TaskFailed,
                    json!({
                        "task_id": task_id,
                        "attempt": attempt,
                        "reason": "max_retries_exceeded",
                        "error": error_msg,
                    }),
                )
                .with_actor_id(task_id.to_string()),
            );
        }

        Ok(())
    }

    /// Cancel a running or pending task.
    ///
    /// If the task is currently running, sends a cancellation signal to the
    /// worker transport, aborts the execution handle, and updates state.
    /// If the task is pending, simply updates the state to 'canceled'.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to cancel
    /// * `reason` - Human-readable cancellation reason
    pub async fn cancel_task(&self, task_id: Uuid, reason: String) -> Result<()> {
        // Check if task is actively running
        let handle = self.active_tasks.lock().await.remove(&task_id);

        if let Some(handle) = handle {
            tracing::info!(
                task_id = %task_id,
                reason = %reason,
                "Cancelling running task"
            );

            // Get current run_id for this task
            let run_id: Option<Uuid> = sqlx::query_scalar(
                r"SELECT run_id FROM task_runs WHERE task_id = $1 AND state = 'running' ORDER BY attempt DESC LIMIT 1",
            )
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Error::Database)?;

            // Send cancellation to worker transport if we have a run_id
            if let Some(rid) = run_id {
                let wm = self.worker_manager.lock().await;
                let workers = wm.get_worker_status().await;
                for w in &workers {
                    if w.status == "running" {
                        if let Ok(transport) = wm.get_transport(&w.id).await {
                            let _ = transport.cancel(RunId(rid), reason.clone()).await;
                            break;
                        }
                    }
                }
            }

            // Wait grace period then force abort
            let grace = Duration::from_secs(self.config.skill_timeout_grace_period_secs);
            let abort_handle = handle.abort_handle();
            tokio::select! {
                _ = tokio::time::sleep(grace) => {
                    tracing::warn!(
                        task_id = %task_id,
                        "Task did not stop within grace period, force aborting"
                    );
                    abort_handle.abort();
                }
                _ = handle => {
                    tracing::debug!(task_id = %task_id, "Task handle completed after cancel signal");
                }
            }

            // Update task_run state
            if let Some(rid) = run_id {
                let _ = sqlx::query(
                    r"UPDATE task_runs SET state = 'canceled', ended_at = NOW() WHERE run_id = $1",
                )
                .bind(rid)
                .execute(&self.pool)
                .await;
            }

            // Update task state
            sqlx::query(
                r"UPDATE tasks SET state = 'canceled', updated_at = NOW() WHERE task_id = $1",
            )
            .bind(task_id)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

            self.event_stream.publish(
                EventEnvelope::new(
                    EventLevel::Info,
                    EventType::TaskCancelled,
                    json!({
                        "task_id": task_id,
                        "reason": reason,
                        "was_running": true,
                    }),
                )
                .with_actor_id(task_id.to_string()),
            );

            tracing::info!(task_id = %task_id, "Running task cancelled");
        } else {
            // Task is not actively running — may be pending
            tracing::info!(
                task_id = %task_id,
                reason = %reason,
                "Cancelling pending task"
            );

            sqlx::query(
                r"UPDATE tasks SET state = 'canceled', updated_at = NOW() WHERE task_id = $1 AND state = 'pending'",
            )
            .bind(task_id)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

            self.event_stream.publish(
                EventEnvelope::new(
                    EventLevel::Info,
                    EventType::TaskCancelled,
                    json!({
                        "task_id": task_id,
                        "reason": reason,
                        "was_running": false,
                    }),
                )
                .with_actor_id(task_id.to_string()),
            );

            tracing::info!(task_id = %task_id, "Pending task cancelled");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::WorkerManager;

    /// Helper to create a test scheduler with lazy pool (no real DB connection).
    fn create_test_scheduler() -> Scheduler {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgresql://test:test@localhost:5432/test")
            .expect("Failed to create lazy pool");
        let event_stream = Arc::new(EventStream::new(100, 10));
        let config = Arc::new(Config::default());
        let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
            config.clone(),
            event_stream.clone(),
        )));
        Scheduler::new(
            pool,
            event_stream,
            Duration::from_millis(1000),
            worker_manager,
            config,
        )
    }

    #[test]
    #[allow(clippy::len_zero)]
    fn test_mantras_defined() {
        assert!(MANTRAS.len() > 0, "Mantras list should not be empty");
        assert_eq!(MANTRAS.len(), 5, "Should have 5 mantras defined");
    }

    #[tokio::test]
    async fn test_scheduler_creation() {
        let scheduler = create_test_scheduler();

        assert_eq!(scheduler.heartbeat_interval, Duration::from_millis(1000));
        assert!(scheduler.shutdown_tx.is_none());
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_mantra_selection_unknown_first() {
        // This test requires a real database connection
        // Run with: cargo test test_mantra_selection_unknown_first -- --ignored
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_mantra_selection_random_rotation() {
        // This test requires a real database connection
        // Run with: cargo test test_mantra_selection_random_rotation -- --ignored
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_heartbeat_execution() {
        // This test requires a real database connection
        // Run with: cargo test test_heartbeat_execution -- --ignored
    }
}
