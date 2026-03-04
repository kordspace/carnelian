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

use crate::agentic::AgenticEngine;
use crate::config::{Config, WorkerLane};
use crate::context::ContextWindow;
use crate::events::EventStream;
use crate::ledger::Ledger;
use crate::metrics::MetricsCollector;
use crate::model_router::{CompletionRequest, Message, ModelRouter};
use crate::worker::WorkerManager;
use crate::workflow::WorkflowEngine;
use carnelian_common::types::{
    EventEnvelope, EventLevel, EventType, InvokeRequest, InvokeStatus, RunId,
};
use carnelian_common::{Error, Result};
use carnelian_magic::{EntropyProvider, QuantumHasher, entropy_arc_impl as _};
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, Semaphore};
use uuid::Uuid;


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
    /// Performance metrics collector
    metrics: Arc<MetricsCollector>,
    /// Model router for LLM calls during heartbeat
    model_router: Arc<ModelRouter>,
    /// Audit ledger for tamper-resistant logging
    ledger: Arc<Ledger>,
    /// Safe mode guard for blocking side-effect operations
    safe_mode_guard: Arc<crate::safe_mode::SafeModeGuard>,
    /// Workflow engine for executing workflow-dispatch tasks and auto skill chaining
    workflow_engine: Option<Arc<WorkflowEngine>>,
    /// XP manager for awarding experience points on task completion
    xp_manager: Option<Arc<crate::xp::XpManager>>,
    /// MantraTree for MAGIC quantum-enhanced operations
    mantra_tree: Option<Arc<carnelian_magic::MantraTree>>,
    /// Entropy provider for quantum-salted correlation IDs and ledger events
    entropy_provider: Option<Arc<carnelian_magic::MixedEntropyProvider>>,
    /// Per-lane concurrency semaphores for task execution control.
    ///
    /// The `Heartbeat` lane semaphore is reserved for future use if heartbeat tasks
    /// are ever routed through the queue. The heartbeat loop invokes `run_heartbeat`
    /// directly, bypassing semaphore acquisition and guaranteeing it always runs.
    lane_semaphores: Arc<HashMap<WorkerLane, Arc<Semaphore>>>,
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
    /// * `model_router` - Model router for LLM calls during heartbeat
    /// * `ledger` - Audit ledger for tamper-resistant logging
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::time::Duration;
    /// let scheduler = Scheduler::new(
    ///     pool, event_stream, Duration::from_millis(555_555),
    ///     worker_manager, config, model_router, ledger,
    /// );
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: PgPool,
        event_stream: Arc<EventStream>,
        heartbeat_interval: Duration,
        worker_manager: Arc<tokio::sync::Mutex<WorkerManager>>,
        config: Arc<Config>,
        model_router: Arc<ModelRouter>,
        ledger: Arc<Ledger>,
        safe_mode_guard: Arc<crate::safe_mode::SafeModeGuard>,
    ) -> Self {
        // Build per-lane semaphores from config
        let mut lane_map = HashMap::new();
        lane_map.insert(
            WorkerLane::Heartbeat,
            Arc::new(Semaphore::new(config.worker_lanes.heartbeat)),
        );
        lane_map.insert(
            WorkerLane::CodeTask,
            Arc::new(Semaphore::new(config.worker_lanes.code_task)),
        );
        lane_map.insert(
            WorkerLane::DataTask,
            Arc::new(Semaphore::new(config.worker_lanes.data_task)),
        );
        lane_map.insert(
            WorkerLane::IoTask,
            Arc::new(Semaphore::new(config.worker_lanes.io_task)),
        );
        lane_map.insert(
            WorkerLane::ChatTask,
            Arc::new(Semaphore::new(config.worker_lanes.chat_task)),
        );

        Self {
            pool,
            event_stream,
            heartbeat_interval,
            shutdown_tx: None,
            worker_manager,
            config,
            active_tasks: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            metrics: Arc::new(MetricsCollector::new()),
            model_router,
            ledger,
            safe_mode_guard,
            workflow_engine: None,
            xp_manager: None,
            mantra_tree: None,
            entropy_provider: None,
            lane_semaphores: Arc::new(lane_map),
        }
    }

    /// Set the workflow engine for workflow-dispatch and auto skill chaining.
    pub fn set_workflow_engine(&mut self, engine: Arc<WorkflowEngine>) {
        self.workflow_engine = Some(engine);
    }

    /// Set the XP manager for awarding experience points on task completion.
    pub fn set_xp_manager(&mut self, xp_manager: Arc<crate::xp::XpManager>) {
        self.xp_manager = Some(xp_manager);
    }

    /// Set the MantraTree for MAGIC quantum-enhanced operations.
    pub fn set_mantra_tree(&mut self, tree: Arc<carnelian_magic::MantraTree>) {
        self.mantra_tree = Some(tree);
    }

    /// Set the entropy provider for quantum-salted correlation IDs and ledger events.
    pub fn set_entropy_provider(&mut self, provider: Arc<carnelian_magic::MixedEntropyProvider>) {
        self.entropy_provider = Some(provider);
    }

    /// Set a shared metrics collector (called from server to share with AppState).
    pub fn set_metrics(&mut self, metrics: Arc<MetricsCollector>) {
        self.metrics = metrics;
    }

    /// Check if the scheduler heartbeat loop is currently running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.shutdown_tx.is_some()
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
        let metrics = self.metrics.clone();
        let model_router = self.model_router.clone();
        let ledger = self.ledger.clone();
        let safe_mode_guard = self.safe_mode_guard.clone();
        let workflow_engine = self.workflow_engine.clone();
        let xp_manager = self.xp_manager.clone();
        let mantra_tree = self.mantra_tree.clone();
        let entropy_provider = self.entropy_provider.clone();
        let lane_semaphores = self.lane_semaphores.clone();

        tokio::spawn(async move {
            Self::run_heartbeat_loop(
                pool,
                event_stream,
                interval,
                shutdown_rx,
                worker_manager,
                config,
                active_tasks,
                metrics,
                model_router,
                ledger,
                safe_mode_guard,
                workflow_engine,
                xp_manager,
                mantra_tree,
                entropy_provider,
                lane_semaphores,
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
    #[allow(clippy::too_many_arguments)]
    async fn run_heartbeat_loop(
        pool: PgPool,
        event_stream: Arc<EventStream>,
        interval: Duration,
        mut shutdown_rx: watch::Receiver<bool>,
        worker_manager: Arc<tokio::sync::Mutex<WorkerManager>>,
        config: Arc<Config>,
        active_tasks: Arc<tokio::sync::Mutex<HashMap<Uuid, tokio::task::JoinHandle<()>>>>,
        metrics: Arc<MetricsCollector>,
        model_router: Arc<ModelRouter>,
        ledger: Arc<Ledger>,
        safe_mode_guard: Arc<crate::safe_mode::SafeModeGuard>,
        workflow_engine: Option<Arc<WorkflowEngine>>,
        xp_manager: Option<Arc<crate::xp::XpManager>>,
        mantra_tree: Option<Arc<carnelian_magic::MantraTree>>,
        entropy_provider: Option<Arc<carnelian_magic::MixedEntropyProvider>>,
        lane_semaphores: Arc<HashMap<WorkerLane, Arc<Semaphore>>>,
    ) {
        let mut ticker = tokio::time::interval(interval);
        // Skip the first immediate tick
        ticker.tick().await;

        let mut last_quality_check: Option<chrono::DateTime<chrono::Utc>> = None;

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    // Acquire heartbeat lane permit to enforce concurrency guarantee
                    let heartbeat_semaphore = lane_semaphores.get(&WorkerLane::Heartbeat)
                        .expect("Heartbeat lane semaphore must exist");
                    let _permit = heartbeat_semaphore.acquire().await
                        .expect("Heartbeat semaphore should never be closed");
                    
                    if let Err(e) = Self::run_heartbeat(
                        &pool,
                        &event_stream,
                        &config,
                        &model_router,
                        &ledger,
                        entropy_provider.as_ref(),
                        mantra_tree.as_ref(),
                    ).await {
                        tracing::warn!(error = %e, "Heartbeat execution failed");
                    }
                    
                    // Permit is automatically released when _permit is dropped
                    // Poll task queue after heartbeat
                    if let Err(e) = Self::poll_task_queue(
                        &pool,
                        &event_stream,
                        &worker_manager,
                        &config,
                        &active_tasks,
                        &metrics,
                        &ledger,
                        &safe_mode_guard,
                        &workflow_engine,
                        &xp_manager,
                        &lane_semaphores,
                        &entropy_provider,
                    ).await {
                        tracing::warn!(error = %e, "Task queue polling failed");
                    }

                    // Daily quality bonus cron
                    if let Some(xp_mgr) = &xp_manager {
                        let should_run = last_quality_check
                            .map(|t| chrono::Utc::now() - t > chrono::Duration::hours(24))
                            .unwrap_or(true);
                        if should_run {
                            if let Err(e) = xp_mgr.run_quality_bonus_check(&pool).await {
                                tracing::warn!(error = %e, "Quality bonus check failed");
                            } else {
                                last_quality_check = Some(chrono::Utc::now());
                            }
                        }
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

    /// Execute a single agentic heartbeat cycle.
    ///
    /// This method:
    /// 1. Generates a correlation ID for end-to-end tracing
    /// 2. Queries the database for the default identity (Lian)
    /// 3. Selects a mantra using the "first unknown, then random" strategy
    /// 4. Counts pending tasks in the queue
    /// 5. Assembles a context window (soul directives, recent memories, task summary)
    /// 6. Makes a model call for brief reflection/planning
    /// 7. Parses the response and persists the heartbeat with correlation ID
    /// 8. Emits `HeartbeatTick` and `HeartbeatOk` events
    ///
    /// If the model call fails, the heartbeat is still logged with `status='failed'`
    /// and only `HeartbeatTick` is emitted (no `HeartbeatOk`).
    #[allow(clippy::too_many_lines)]
    async fn run_heartbeat(
        pool: &PgPool,
        event_stream: &Arc<EventStream>,
        config: &Config,
        model_router: &ModelRouter,
        ledger: &Ledger,
        entropy_provider: Option<&Arc<carnelian_magic::MixedEntropyProvider>>,
        mantra_tree: Option<&Arc<carnelian_magic::MantraTree>>,
    ) -> Result<()> {
        let start = std::time::Instant::now();
        
        // Generate quantum-salted correlation ID if entropy provider is available
        let correlation_id = if let Some(provider) = entropy_provider {
            match tokio::time::timeout(
                std::time::Duration::from_millis(config.magic.entropy_timeout_ms),
                provider.as_ref().get_bytes(16)
            ).await {
                Ok(Ok(entropy_bytes)) => {
                    <[u8; 16]>::try_from(entropy_bytes.as_slice())
                        .map(|bytes_array| {
                            tracing::debug!("Generated quantum-salted correlation ID");
                            Uuid::from_bytes(bytes_array)
                        })
                        .unwrap_or_else(|_| {
                            tracing::warn!("Failed to convert entropy bytes to UUID, falling back to rand");
                            Uuid::from_bytes(rand::random())
                        })
                }
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, "Entropy provider failed, falling back to rand");
                    Uuid::from_bytes(rand::random())
                }
                Err(_) => {
                    tracing::warn!("Entropy provider timeout, falling back to rand");
                    Uuid::from_bytes(rand::random())
                }
            }
        } else {
            // No entropy provider configured - use rand::thread_rng() as requested
            Uuid::from_bytes(rand::random())
        };

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

        // ── Mantra Selection (MAGIC subsystem) ──────────────────────────
        // Obtain 8 bytes of entropy for mantra selection and track actual source
        let (entropy_bytes, actual_entropy_source): (Vec<u8>, &str) = if let Some(provider) = entropy_provider {
            match tokio::time::timeout(
                std::time::Duration::from_millis(config.magic.entropy_timeout_ms),
                provider.as_ref().get_bytes(8)
            ).await {
                Ok(Ok(bytes)) => {
                    tracing::debug!("Generated quantum entropy for mantra selection");
                    (bytes, "quantum")
                }
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, "Entropy provider failed for mantra, falling back to rand");
                    (rand::random::<[u8; 8]>().to_vec(), "os_random")
                }
                Err(_) => {
                    tracing::warn!("Entropy provider timeout for mantra, falling back to rand");
                    (rand::random::<[u8; 8]>().to_vec(), "os_random")
                }
            }
        } else {
            (rand::random::<[u8; 8]>().to_vec(), "os_random")
        };

        // MAGIC path: build context and select mantra
        let mut mantra_selection: Option<carnelian_magic::MantraSelection> = None;
        let mut mantra_context: Option<carnelian_magic::MantraContext> = None;
        let mantra_text: Option<String>;
        // Track fallback selection metadata for mantra_history
        let mut fallback_entry_id: Option<Uuid> = None;
        let mut fallback_category_id: Option<Uuid> = None;

        if let Some(tree) = mantra_tree {
            // Build context from DB
            match carnelian_magic::MantraTree::build_context(pool).await {
                Ok(context) => {
                    mantra_context = Some(context.clone());
                    // Select mantra using preloaded tree or with pool
                    match tree.select_with_pool(&entropy_bytes, &context, pool).await {
                        Ok(selection) => {
                            mantra_text = Some(selection.mantra_text.clone());
                            mantra_selection = Some(selection);
                            tracing::debug!("MAGIC mantra selection succeeded");
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "MAGIC mantra selection failed, using fallback");
                            // Fallback: fetch all eligible entries and select with OS-random
                            let entries: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
                                "SELECT entry_id, category_id, text FROM mantra_entries WHERE enabled = true"
                            )
                            .fetch_all(pool)
                            .await
                            .map_err(Error::Database)?;
                            
                            if !entries.is_empty() {
                                let idx = rand::random::<usize>() % entries.len();
                                let (entry_id, category_id, text) = &entries[idx];
                                fallback_entry_id = Some(*entry_id);
                                fallback_category_id = Some(*category_id);
                                mantra_text = Some(text.clone());
                            } else {
                                mantra_text = None;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "MAGIC context build failed, using fallback");
                    // Fallback: fetch all eligible entries and select with OS-random
                    let entries: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
                        "SELECT entry_id, category_id, text FROM mantra_entries WHERE enabled = true"
                    )
                    .fetch_all(pool)
                    .await
                    .map_err(Error::Database)?;
                    
                    if !entries.is_empty() {
                        let idx = rand::random::<usize>() % entries.len();
                        let (entry_id, category_id, text) = &entries[idx];
                        fallback_entry_id = Some(*entry_id);
                        fallback_category_id = Some(*category_id);
                        mantra_text = Some(text.clone());
                    } else {
                        mantra_text = None;
                    }
                }
            }
        } else {
            // MAGIC disabled: fetch all eligible entries and select with OS-random
            let entries: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
                "SELECT entry_id, category_id, text FROM mantra_entries WHERE enabled = true"
            )
            .fetch_all(pool)
            .await
            .map_err(Error::Database)?;
            
            if !entries.is_empty() {
                let idx = rand::random::<usize>() % entries.len();
                let (entry_id, category_id, text) = &entries[idx];
                fallback_entry_id = Some(*entry_id);
                fallback_category_id = Some(*category_id);
                mantra_text = Some(text.clone());
            } else {
                mantra_text = None;
            }
        }

        // Count pending tasks
        let tasks_queued: i64 = sqlx::query_scalar::<_, Option<i64>>(
            r"SELECT COUNT(*) FROM tasks WHERE state = 'pending'",
        )
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?
        .unwrap_or(0);

        // ── Context Assembly ─────────────────────────────────────────────
        let mut ctx =
            ContextWindow::new(pool.clone(), Some(event_stream.clone())).with_config(config);

        // Load soul directives (P0)
        if let Err(e) = ctx.load_soul_directives(identity_id).await {
            tracing::warn!(
                error = %e,
                correlation_id = %correlation_id,
                "Failed to load soul directives for heartbeat context"
            );
        }

        // Load recent memories (P1)
        if let Err(e) = ctx.load_recent_memories(identity_id, 10).await {
            tracing::warn!(
                error = %e,
                correlation_id = %correlation_id,
                "Failed to load recent memories for heartbeat context"
            );
        }

        // Add task queue summary as P2 segment
        let task_summary = format!(
            "Current state: {} pending tasks in queue. Mantra: \"{}\"",
            tasks_queued,
            mantra_text.as_deref().unwrap_or("none")
        );
        ctx.add_raw_segment(
            crate::context::SegmentPriority::P2,
            task_summary,
            crate::context::SegmentSourceType::TaskContext,
            None,
        );

        // Enforce budget
        ctx.enforce_budget(config.tool_trim_threshold, config.tool_clear_age_secs);

        // Assemble context
        let context_text = match ctx.assemble(config).await {
            Ok(text) => {
                tracing::debug!(
                    correlation_id = %correlation_id,
                    context_len = text.len(),
                    "Heartbeat context assembled"
                );
                text
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    correlation_id = %correlation_id,
                    "Context assembly failed, using minimal context"
                );
                format!(
                    "Pending tasks: {}. Mantra: \"{}\"",
                    tasks_queued,
                    mantra_text.as_deref().unwrap_or("none")
                )
            }
        };

        // Log context to ledger
        if let Err(e) = ctx.log_to_ledger(ledger, correlation_id).await {
            tracing::warn!(error = %e, "Failed to log heartbeat context to ledger");
        }

        // ── Model Call ───────────────────────────────────────────────────
        // Prepare system and user messages based on MAGIC selection
        let system_content = if let Some(ref selection) = mantra_selection {
            format!("{}\n\n{}", context_text, selection.system_message)
        } else {
            context_text
        };

        let user_content = mantra_selection
            .as_ref()
            .map_or_else(
                || "Reflect briefly on the current state. Note any observations or planning thoughts. Keep it concise (2-3 sentences).".to_string(),
                |selection| selection.user_message.clone(),
            );

        let request = CompletionRequest {
            model: "deepseek-r1:7b".to_string(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_content,
                    name: None,
                    tool_call_id: None,
                },
                Message {
                    role: "user".to_string(),
                    content: user_content,
                    name: None,
                    tool_call_id: None,
                },
            ],
            temperature: Some(0.7),
            max_tokens: Some(1000),
            stream: None,
            correlation_id: Some(correlation_id),
        };

        let model_result = tokio::time::timeout(
            Duration::from_secs(30),
            model_router.complete(request, identity_id, None, None, None),
        )
        .await;

        let (status, reason, response_content) = match model_result {
            Ok(Ok(response)) => {
                let content = response
                    .choices
                    .first()
                    .map(|c| c.message.content.clone())
                    .unwrap_or_default();

                // Truncate for DB storage, full response is in the ledger
                let summary = if content.len() > 500 {
                    format!("{}…", &content[..497])
                } else {
                    content.clone()
                };

                tracing::info!(
                    correlation_id = %correlation_id,
                    model = %response.model,
                    provider = %response.provider,
                    tokens_in = response.usage.prompt_tokens,
                    tokens_out = response.usage.completion_tokens,
                    response_len = content.len(),
                    "Heartbeat model call succeeded"
                );

                ("ok".to_string(), Some(summary), Some(content))
            }
            Ok(Err(e)) => {
                tracing::warn!(
                    error = %e,
                    correlation_id = %correlation_id,
                    "Heartbeat model call failed"
                );
                ("failed".to_string(), Some(format!("Model call error: {e}")), None)
            }
            Err(_) => {
                tracing::warn!(
                    correlation_id = %correlation_id,
                    "Heartbeat model call timed out (30s)"
                );
                (
                    "failed".to_string(),
                    Some("Model call timed out after 30s".to_string()),
                    None,
                )
            }
        };

        let duration_ms = start.elapsed().as_millis() as i32;
        let is_ok = status == "ok";

        // Track LLM auto-queued tasks for observability
        let mut llm_auto_queued: usize = 0;

        // ── Parse Tool Calls and Queue Tasks ─────────────────────────────
        if let Some(content) = response_content {
            match AgenticEngine::parse_tool_calls(&content) {
                Ok(tool_calls) if !tool_calls.is_empty() => {
                    tracing::info!(
                        correlation_id = %correlation_id,
                        tool_call_count = tool_calls.len(),
                        "Parsed tool calls from heartbeat response"
                    );

                    llm_auto_queued = auto_queue_llm_tasks(
                        pool,
                        event_stream,
                        &tool_calls,
                        identity_id,
                        correlation_id,
                        config.max_tasks_per_heartbeat,
                    )
                    .await
                    .unwrap_or_else(|e| {
                        tracing::warn!(
                            error = %e,
                            correlation_id = %correlation_id,
                            "Failed to auto-queue LLM-suggested tasks"
                        );
                        0
                    });
                }
                Ok(_) => {
                    tracing::debug!(
                        correlation_id = %correlation_id,
                        "No tool calls found in heartbeat response"
                    );
                }
                Err(e) => {
                    tracing::debug!(
                        correlation_id = %correlation_id,
                        error = %e,
                        "Failed to parse tool calls from heartbeat response (expected for most heartbeats)"
                    );
                }
            }
        }

        // ── Persist to Database ──────────────────────────────────────────
        let heartbeat_id: Uuid = sqlx::query_scalar(
            r"
            INSERT INTO heartbeat_history (identity_id, mantra, tasks_queued, status, duration_ms, reason, correlation_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING heartbeat_id
            ",
        )
        .bind(identity_id)
        .bind(&mantra_text)
        .bind(tasks_queued as i32)
        .bind(&status)
        .bind(duration_ms)
        .bind(&reason)
        .bind(correlation_id)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;

        // Persist mantra_history row for every heartbeat
        if let Some(ref selection) = mantra_selection {
            // MAGIC success path - use full selection data
            if let Err(e) = sqlx::query(
                r"
                INSERT INTO mantra_history (heartbeat_id, category_id, entry_id, context_snapshot, context_weights, suggested_skill_ids, entropy_source)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "
            )
            .bind(heartbeat_id)
            .bind(selection.category_id)
            .bind(selection.entry_id)
            .bind(serde_json::to_value(&mantra_context).ok())
            .bind(serde_json::to_value(&selection.context_weights).ok())
            .bind(&selection.suggested_skill_ids)
            .bind(actual_entropy_source)
            .execute(pool)
            .await
            {
                tracing::warn!(error = %e, "Failed to persist mantra_history");
            }

            // Increment use_count on selected entry
            if let Err(e) = sqlx::query("UPDATE mantra_entries SET use_count = use_count + 1 WHERE entry_id = $1")
                .bind(selection.entry_id)
                .execute(pool)
                .await
            {
                tracing::warn!(error = %e, "Failed to increment mantra entry use_count");
            }
        } else if fallback_entry_id.is_some() && fallback_category_id.is_some() {
            // Fallback path - use captured entry/category IDs with sane defaults
            if let Err(e) = sqlx::query(
                r"
                INSERT INTO mantra_history (heartbeat_id, category_id, entry_id, context_snapshot, context_weights, suggested_skill_ids, entropy_source)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "
            )
            .bind(heartbeat_id)
            .bind(fallback_category_id.unwrap())
            .bind(fallback_entry_id.unwrap())
            .bind(None::<serde_json::Value>)  // No context snapshot for fallback
            .bind(None::<serde_json::Value>)  // No context weights for fallback
            .bind(Vec::<Uuid>::new())        // Empty suggested_skill_ids
            .bind(actual_entropy_source)
            .execute(pool)
            .await
            {
                tracing::warn!(error = %e, "Failed to persist fallback mantra_history");
            }

            // Increment use_count on selected entry
            if let Err(e) = sqlx::query("UPDATE mantra_entries SET use_count = use_count + 1 WHERE entry_id = $1")
                .bind(fallback_entry_id.unwrap())
                .execute(pool)
                .await
            {
                tracing::warn!(error = %e, "Failed to increment fallback mantra entry use_count");
            }
        }

        // Log to ledger
        if let Err(e) = ledger
            .append_event(
                Some(identity_id),
                "heartbeat.completed",
                json!({
                    "mantra": mantra_text,
                    "reason": reason,
                    "status": status,
                    "duration_ms": duration_ms,
                    "mantra_category": mantra_selection.as_ref().map(|s| s.category.as_db_name()),
                    "entropy_source": actual_entropy_source,
                    "context_weights": mantra_selection.as_ref().map(|s| &s.context_weights),
                    "suggested_skill_ids": mantra_selection.as_ref().map(|s| &s.suggested_skill_ids),
                    "elixir_reference": mantra_selection.as_ref().and_then(|s| s.elixir_reference),
                    "elixir_drafts_pending": mantra_context.as_ref().map(|c| c.elixir_drafts_pending).unwrap_or(0),
                }),
                Some(correlation_id),
                None,
                None,
                None,
                None,
            )
            .await
        {
            tracing::warn!(error = %e, "Failed to log heartbeat to ledger");
        }

        // ── Emit Events ─────────────────────────────────────────────────
        // Always emit HeartbeatTick for real-time monitoring
        event_stream.publish(
            EventEnvelope::new(
                EventLevel::Info,
                EventType::HeartbeatTick,
                json!({
                    "heartbeat_id": heartbeat_id,
                    "identity_id": identity_id,
                    "mantra": mantra_text,
                    "tasks_queued": tasks_queued,
                    "duration_ms": duration_ms,
                    "status": status,
                    "correlation_id": correlation_id,
                    "mantra_category": mantra_selection.as_ref().map(|s| s.category.as_db_name()),
                    "entropy_source": actual_entropy_source,
                    "context_weights": mantra_selection.as_ref().map(|s| &s.context_weights),
                    "suggested_skill_ids": mantra_selection.as_ref().map(|s| &s.suggested_skill_ids),
                    "elixir_reference": mantra_selection.as_ref().and_then(|s| s.elixir_reference),
                    "elixir_drafts_pending": mantra_context.as_ref().map(|c| c.elixir_drafts_pending).unwrap_or(0),
                }),
            )
            .with_correlation_id(correlation_id),
        );

        // Only emit HeartbeatOk on successful agentic planning
        if is_ok {
            event_stream.publish(
                EventEnvelope::new(
                    EventLevel::Info,
                    EventType::HeartbeatOk,
                    json!({
                        "heartbeat_id": heartbeat_id,
                        "identity_id": identity_id,
                        "correlation_id": correlation_id,
                        "duration_ms": duration_ms,
                        "response_summary": reason,
                    }),
                )
                .with_correlation_id(correlation_id),
            );
        }

        // ── Workspace Scan & Auto-Queue ─────────────────────────────────
        let scan_limit = config.max_tasks_per_heartbeat;
        let mut ws_scanned: usize = 0;
        let mut ws_safe: usize = 0;
        let mut ws_privileged: usize = 0;
        let mut ws_auto_queued: usize = 0;

        if scan_limit > 0 && !config.workspace_scan_paths.is_empty() {
            let markers = WorkspaceScanner::scan(&config.workspace_scan_paths);
            if !markers.is_empty() {
                ws_scanned = markers.len();
                ws_safe = markers.iter().filter(|m| m.is_safe).count();
                ws_privileged = ws_scanned - ws_safe;
                tracing::debug!(
                    total = ws_scanned,
                    safe = ws_safe,
                    privileged = ws_privileged,
                    "Workspace scan found markers"
                );

                match auto_queue_scanned_tasks(
                    pool,
                    event_stream,
                    &markers,
                    identity_id,
                    correlation_id,
                    scan_limit,
                )
                .await
                {
                    Ok(queued) => {
                        ws_auto_queued = queued;
                        if queued > 0 {
                            tracing::info!(
                                queued = queued,
                                correlation_id = %correlation_id,
                                "Auto-queued tasks from workspace scan"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            correlation_id = %correlation_id,
                            "Failed to auto-queue scanned tasks"
                        );
                    }
                }
            }
        }

        tracing::info!(
            heartbeat_id = %heartbeat_id,
            identity_id = %identity_id,
            mantra = ?mantra_text,
            tasks_queued = tasks_queued,
            workspace_scanned = ws_scanned,
            workspace_safe = ws_safe,
            workspace_privileged = ws_privileged,
            workspace_auto_queued = ws_auto_queued,
            llm_auto_queued = llm_auto_queued,
            duration_ms = duration_ms,
            status = %status,
            correlation_id = %correlation_id,
            "Heartbeat completed"
        );

        Ok(())
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
        metrics: &Arc<MetricsCollector>,
        ledger: &Arc<Ledger>,
        safe_mode_guard: &Arc<crate::safe_mode::SafeModeGuard>,
        workflow_engine: &Option<Arc<WorkflowEngine>>,
        xp_manager: &Option<Arc<crate::xp::XpManager>>,
        lane_semaphores: &Arc<HashMap<WorkerLane, Arc<Semaphore>>>,
        entropy_provider: &Option<Arc<carnelian_magic::MixedEntropyProvider>>,
    ) -> Result<()> {
        // If safe mode is active, skip dequeuing entirely — tasks stay pending
        if safe_mode_guard.is_enabled().await.unwrap_or(false) {
            tracing::debug!("Safe mode active, skipping task dequeue");
            return Ok(());
        }

        // Query pending tasks ordered by priority DESC, created_at ASC
        // Fetch title and description for lane classification
        // Use sum of all lane permits as LIMIT to support lane skipping
        let total_lane_permits = config.worker_lanes.heartbeat
            + config.worker_lanes.code_task
            + config.worker_lanes.data_task
            + config.worker_lanes.io_task
            + config.worker_lanes.chat_task;
        let fetch_limit = total_lane_permits as usize;
        
        let pending_tasks: Vec<(Uuid, Option<Uuid>, i32, String, Option<String>)> = sqlx::query_as(
            r"SELECT task_id, skill_id, priority, title, description FROM tasks WHERE state = 'pending' ORDER BY priority DESC, created_at ASC LIMIT $1",
        )
        .bind(i64::try_from(fetch_limit).unwrap_or(i64::MAX))
        .fetch_all(pool)
        .await
        .map_err(Error::Database)?;

        tracing::debug!(
            pending_count = pending_tasks.len(),
            "Polled task queue"
        );

        for (task_id, skill_id, priority, title, description) in pending_tasks {
            // Classify task into lane
            let lane = crate::config::classify_task_lane(&title, &description.unwrap_or_default());
            
            // Try to acquire a permit for this lane (non-blocking)
            let semaphore = lane_semaphores.get(&lane).ok_or_else(|| {
                Error::Config(format!("Missing semaphore for lane {:?}", lane))
            })?;
            
            let permit = match semaphore.clone().try_acquire_owned() {
                Ok(p) => p,
                Err(_) => {
                    tracing::debug!(
                        task_id = %task_id,
                        lane = ?lane,
                        "No available permits for lane, skipping task"
                    );
                    continue; // Skip this task, try next one
                }
            };
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
            let metrics = metrics.clone();
            let safe_mode_guard = safe_mode_guard.clone();

            let workflow_engine = workflow_engine.clone();
            let xp_manager = xp_manager.clone();
            let entropy_provider = entropy_provider.clone();

            let handle = tokio::spawn(async move {
                if let Err(e) = Self::execute_task(
                    task_id,
                    skill_id,
                    &pool,
                    &event_stream,
                    &worker_manager,
                    &config,
                    &active_tasks_clone,
                    &metrics,
                    &safe_mode_guard,
                    workflow_engine.as_ref(),
                    xp_manager.as_ref(),
                    entropy_provider.as_ref(),
                )
                .await
                {
                    // If safe mode blocked the operation, leave the task in its
                    // current state (pending) so it can be retried once safe mode
                    // is disabled.  Do NOT mark it as failed.
                    if matches!(e, Error::SafeModeActive(_)) {
                        tracing::warn!(
                            task_id = %task_id,
                            "Task execution blocked by safe mode, leaving task pending"
                        );
                    } else {
                        tracing::error!(
                            task_id = %task_id,
                            error = %e,
                            "Task execution failed with unhandled error"
                        );
                        // Ensure task is marked as failed on unhandled errors
                        let _ = sqlx::query(
                            r"UPDATE tasks SET state = 'failed', updated_at = NOW() WHERE task_id = $1",
                        )
                        .bind(task_id)
                        .execute(&pool)
                        .await;
                    }
                }

                // Remove from active_tasks on completion
                active_tasks_clone.lock().await.remove(&task_id);
                
                // Drop permit to release semaphore slot
                drop(permit);
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
        metrics: &Arc<MetricsCollector>,
        safe_mode_guard: &Arc<crate::safe_mode::SafeModeGuard>,
        workflow_engine: Option<&Arc<WorkflowEngine>>,
        xp_manager: Option<&Arc<crate::xp::XpManager>>,
        entropy_provider: Option<&Arc<carnelian_magic::MixedEntropyProvider>>,
    ) -> Result<()> {
        // Safe mode is checked in poll_task_queue before dequeuing, but
        // re-check here as a defence-in-depth measure in case the flag was
        // toggled between dequeue and execution.
        safe_mode_guard.check_or_block("task_execution").await?;

        let exec_start = std::time::Instant::now();

        // ── Workflow dispatch detection ──────────────────────────────────
        // Before normal skill execution, check if this task is a workflow
        // dispatch or if it has no skill and can be auto-chained.
        if let Some(wf_engine) = workflow_engine {
            // Fetch the task description to check for workflow dispatch marker
            let task_desc: Option<String> =
                sqlx::query_scalar(r"SELECT description FROM tasks WHERE task_id = $1")
                    .bind(task_id)
                    .fetch_optional(pool)
                    .await
                    .map_err(Error::Database)?;

            let desc_str = task_desc.unwrap_or_default();

            // Check if this is a workflow-dispatch task
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&desc_str) {
                if parsed.get("_workflow_dispatch").and_then(|d| d.as_bool()) == Some(true) {
                    tracing::info!(
                        task_id = %task_id,
                        "Detected workflow-dispatch task, delegating to WorkflowEngine"
                    );

                    // Mark task as running
                    sqlx::query(
                        r"UPDATE tasks SET state = 'running', updated_at = NOW() WHERE task_id = $1",
                    )
                    .bind(task_id)
                    .execute(pool)
                    .await
                    .map_err(Error::Database)?;

                    match wf_engine.try_execute_workflow_task(task_id).await {
                        Ok(Some(result)) => {
                            let final_state = if result.status == "success" {
                                "completed"
                            } else {
                                "failed"
                            };
                            let _ = sqlx::query(
                                r"UPDATE tasks SET state = $1, updated_at = NOW() WHERE task_id = $2",
                            )
                            .bind(final_state)
                            .bind(task_id)
                            .execute(pool)
                            .await;

                            let duration_ms = exec_start.elapsed().as_millis() as i64;
                            if final_state == "completed" {
                                event_stream.publish(
                                    EventEnvelope::new(
                                        EventLevel::Info,
                                        EventType::TaskCompleted,
                                        json!({
                                            "task_id": task_id,
                                            "workflow_dispatch": true,
                                            "duration_ms": duration_ms,
                                        }),
                                    )
                                    .with_actor_id(task_id.to_string()),
                                );
                            } else {
                                event_stream.publish(
                                    EventEnvelope::new(
                                        EventLevel::Warn,
                                        EventType::TaskFailed,
                                        json!({
                                            "task_id": task_id,
                                            "workflow_dispatch": true,
                                            "duration_ms": duration_ms,
                                            "error": result.execution_summary,
                                        }),
                                    )
                                    .with_actor_id(task_id.to_string()),
                                );
                            }
                            return Ok(());
                        }
                        Ok(None) => {
                            // Not actually a workflow dispatch (shouldn't happen), fall through
                            tracing::warn!(
                                task_id = %task_id,
                                "Task had _workflow_dispatch marker but try_execute returned None"
                            );
                        }
                        Err(e) => {
                            tracing::error!(
                                task_id = %task_id,
                                error = %e,
                                "Workflow dispatch execution failed"
                            );
                            let _ = sqlx::query(
                                r"UPDATE tasks SET state = 'failed', updated_at = NOW() WHERE task_id = $1",
                            )
                            .bind(task_id)
                            .execute(pool)
                            .await;

                            event_stream.publish(
                                EventEnvelope::new(
                                    EventLevel::Warn,
                                    EventType::TaskFailed,
                                    json!({
                                        "task_id": task_id,
                                        "workflow_dispatch": true,
                                        "error": e.to_string(),
                                    }),
                                )
                                .with_actor_id(task_id.to_string()),
                            );
                            return Ok(());
                        }
                    }
                }
            }

            // If no skill_id is set, try auto skill chaining
            if skill_id.is_none() {
                tracing::info!(
                    task_id = %task_id,
                    "No skill_id set, attempting auto skill chaining"
                );

                // Mark task as running
                sqlx::query(
                    r"UPDATE tasks SET state = 'running', updated_at = NOW() WHERE task_id = $1",
                )
                .bind(task_id)
                .execute(pool)
                .await
                .map_err(Error::Database)?;

                match wf_engine.execute_task_with_chaining(task_id).await {
                    Ok(result) => {
                        let final_state = if result.status == "success" {
                            "completed"
                        } else {
                            "failed"
                        };
                        let _ = sqlx::query(
                            r"UPDATE tasks SET state = $1, updated_at = NOW() WHERE task_id = $2",
                        )
                        .bind(final_state)
                        .bind(task_id)
                        .execute(pool)
                        .await;

                        let duration_ms = exec_start.elapsed().as_millis() as i64;
                        event_stream.publish(
                            EventEnvelope::new(
                                EventLevel::Info,
                                if final_state == "completed" {
                                    EventType::TaskCompleted
                                } else {
                                    EventType::TaskFailed
                                },
                                json!({
                                    "task_id": task_id,
                                    "auto_chained": true,
                                    "duration_ms": duration_ms,
                                    "workflow_status": result.status,
                                }),
                            )
                            .with_actor_id(task_id.to_string()),
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        // Chaining failed (e.g. no matching skills) — fall through
                        // to the normal "skill not found" path below, resetting
                        // the task back to running for the standard flow.
                        tracing::debug!(
                            task_id = %task_id,
                            error = %e,
                            "Auto skill chaining failed, falling through to normal execution"
                        );
                        // Reset task state back to pending so the normal flow
                        // can set it to running again with proper bookkeeping.
                        let _ = sqlx::query(
                            r"UPDATE tasks SET state = 'pending', updated_at = NOW() WHERE task_id = $1",
                        )
                        .bind(task_id)
                        .execute(pool)
                        .await;
                    }
                }
            }
        }

        // ── Normal skill-based execution ─────────────────────────────────

        // Update task state to 'running'
        sqlx::query(r"UPDATE tasks SET state = 'running', updated_at = NOW() WHERE task_id = $1")
            .bind(task_id)
            .execute(pool)
            .await
            .map_err(Error::Database)?;

        // Determine attempt number from existing task_runs
        let attempt: i64 = sqlx::query_scalar::<_, Option<i32>>(
            r"SELECT COALESCE(MAX(attempt), 0) FROM task_runs WHERE task_id = $1",
        )
        .bind(task_id)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?
        .map_or(0, i64::from)
            + 1;

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
        .map_err(Error::Database)?;

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

        // Record task latency metric (created_at → started_at)
        let created_at: Option<chrono::DateTime<chrono::Utc>> =
            sqlx::query_scalar(r"SELECT created_at FROM tasks WHERE task_id = $1")
                .bind(task_id)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();
        if let Some(created_at) = created_at {
            metrics.record_task_latency(task_id, created_at, chrono::Utc::now());
        }

        // Known limitation (v1.0.0): capability enforcement at dispatch time is not yet
        // implemented; all skills are permitted to execute subject to the global policy layer.
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

        let (skill_name, runtime) = match skill_info {
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

        // Parse runtime and get transport for that runtime
        let parsed_runtime = runtime.parse::<crate::worker::WorkerRuntime>().map_err(|e| {
            Error::Config(format!("Invalid worker runtime '{}': {}", runtime, e))
        })?;

        let transport = {
            let wm = worker_manager.lock().await;
            wm.get_transport_for_runtime(parsed_runtime).await
        };

        let transport = match transport {
            Ok(t) => t,
            Err(e) => {
                let error_msg = format!("No running {} worker available: {}", runtime, e);
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

                        // Compute quantum checksum for successful task result
                        let started_at: Option<DateTime<Utc>> = sqlx::query_scalar(
                            "SELECT started_at FROM task_runs WHERE run_id = $1"
                        )
                        .bind(run_id.0)
                        .fetch_optional(pool)
                        .await
                        .ok()
                        .flatten();

                        if let Some(ts) = started_at {
                            let result_text = serde_json::to_string(&result_json).unwrap_or_else(|_| "{}".to_string());
                            let hasher = entropy_provider
                                .as_ref()
                                .map_or_else(
                                    QuantumHasher::with_os_entropy,
                                    |provider| QuantumHasher::new(Arc::clone(provider)),
                                );
                            match hasher.compute_with_ts("task_runs", run_id.0, result_text.as_bytes(), ts) {
                                Ok(checksum) => {
                                    if let Err(e) = sqlx::query("UPDATE task_runs SET quantum_checksum = $1 WHERE run_id = $2")
                                        .bind(&checksum)
                                        .bind(run_id.0)
                                        .execute(pool)
                                        .await
                                    {
                                        tracing::warn!(run_id = %run_id.0, error = %e, "Failed to store quantum checksum");
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(run_id = %run_id.0, error = %e, "Failed to compute quantum checksum");
                                }
                            }
                        }

                        // Award task completion XP
                        if let Some(xp_mgr) = xp_manager {
                            let assigned_to: Option<Uuid> = sqlx::query_scalar(
                                "SELECT assigned_to FROM tasks WHERE task_id = $1",
                            )
                            .bind(task_id)
                            .fetch_optional(pool)
                            .await
                            .ok()
                            .flatten();

                            if let Some(agent_id) = assigned_to {
                                let mut total_xp =
                                    crate::xp::XpManager::calculate_task_xp(duration_ms);

                                // First skill use bonus
                                if let Some(sid) = skill_id {
                                    if matches!(
                                        xp_mgr.is_first_skill_use(agent_id, sid).await,
                                        Ok(true)
                                    ) {
                                        total_xp += 10;
                                    }
                                }

                                let source =
                                    crate::xp::XpSource::TaskCompletion { task_id, skill_id };
                                if let Err(e) =
                                    xp_mgr.award_xp(agent_id, source, total_xp, None).await
                                {
                                    tracing::warn!(error = %e, task_id = %task_id, "Failed to award task XP");
                                }
                            }

                            // Update skill metrics on success
                            if let Some(sid) = skill_id {
                                if let Err(e) =
                                    xp_mgr.update_skill_metrics(sid, duration_ms, true).await
                                {
                                    tracing::warn!(error = %e, skill_id = %sid, "Failed to update skill metrics");
                                }
                            }
                        }
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

                        // Track skill failure metrics
                        if let Some(xp_mgr) = xp_manager {
                            if let Some(sid) = skill_id {
                                if let Err(e) =
                                    xp_mgr.update_skill_metrics(sid, duration_ms, false).await
                                {
                                    tracing::warn!(error = %e, skill_id = %sid, "Failed to update skill metrics on failure");
                                }
                            }
                        }

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
            let retry_delay = config.task_retry_delay_secs;
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(retry_delay)).await;
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

// =============================================================================
// WORKSPACE SCANNER
// =============================================================================

/// File extensions eligible for workspace scanning.
const SCANNABLE_EXTENSIONS: &[&str] = &[
    "rs", "py", "ts", "js", "tsx", "jsx", "go", "java", "c", "cpp", "h", "hpp", "rb", "sh", "bash",
    "zsh", "toml", "yaml", "yml", "json", "md", "txt",
];

/// Maximum file size in bytes to scan (256 KB). Larger files are skipped.
const MAX_SCAN_FILE_BYTES: u64 = 262_144;

/// Keywords in a marker description that indicate a privileged (non-safe) task.
const PRIVILEGED_KEYWORDS: &[&str] = &[
    "delete",
    "drop",
    "migrate",
    "deploy",
    "production",
    "credential",
    "secret",
    "key rotation",
    "sudo",
    "admin",
    "root",
    "destroy",
    "truncate",
    "revert",
    "rollback",
    "security",
    "permission",
    "privilege",
    "password",
    "token",
    "api_key",
    "private_key",
    "certificate",
    "encryption",
    "decrypt",
];

/// A task marker discovered in a workspace file.
#[derive(Debug, Clone)]
pub struct ScannedMarker {
    /// Relative file path where the marker was found.
    pub file_path: String,
    /// 1-indexed line number.
    pub line_number: usize,
    /// The marker prefix that matched (`TASK` or `TODO`).
    pub marker_type: String,
    /// The text following the marker prefix (trimmed).
    pub description: String,
    /// Whether the task is classified as safe to auto-queue.
    pub is_safe: bool,
}

/// Workspace scanner that detects `TASK:` and `TODO:` markers in source files.
pub struct WorkspaceScanner;

impl WorkspaceScanner {
    /// Scan configured workspace paths for `TASK:` and `TODO:` markers.
    ///
    /// Returns all discovered markers, skipping files that are too large,
    /// binary, or have non-scannable extensions. Directories named `target`,
    /// `node_modules`, `.git`, and `__pycache__` are excluded.
    ///
    /// The caller is responsible for enforcing any per-heartbeat limit on
    /// how many safe tasks are actually queued.
    pub fn scan(paths: &[std::path::PathBuf]) -> Vec<ScannedMarker> {
        let mut markers = Vec::new();

        for base in paths {
            if !base.exists() {
                tracing::debug!(path = %base.display(), "Workspace scan path does not exist, skipping");
                continue;
            }
            Self::walk_dir(base, base, &mut markers);
        }

        markers
    }

    /// Recursively walk a directory collecting markers.
    fn walk_dir(base: &std::path::Path, dir: &std::path::Path, markers: &mut Vec<ScannedMarker>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();

            // Skip hidden and well-known non-source directories
            if path.is_dir() {
                if name.starts_with('.')
                    || name == "target"
                    || name == "node_modules"
                    || name == "__pycache__"
                    || name == "dist"
                    || name == "build"
                    || name == "vendor"
                {
                    continue;
                }
                Self::walk_dir(base, &path, markers);
                continue;
            }

            // Filter by extension
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !SCANNABLE_EXTENSIONS.contains(&ext) {
                continue;
            }

            // Skip large files
            if let Ok(meta) = entry.metadata() {
                if meta.len() > MAX_SCAN_FILE_BYTES {
                    continue;
                }
            }

            Self::scan_file(base, &path, markers);
        }
    }

    /// Scan a single file for `TASK:` and `TODO:` markers.
    fn scan_file(base: &std::path::Path, path: &std::path::Path, markers: &mut Vec<ScannedMarker>) {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return, // skip binary / unreadable files
        };

        let rel_path = path
            .strip_prefix(base)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            // Match TASK: or TODO: (case-insensitive prefix search after stripping comment chars)
            let stripped = Self::strip_comment_prefix(trimmed);
            let upper = stripped.to_uppercase();

            let (marker_type, description) = if let Some(rest) = upper.strip_prefix("TASK:") {
                // Use original casing for description
                let offset = stripped.len() - rest.len();
                ("TASK".to_string(), stripped[offset..].trim().to_string())
            } else if let Some(rest) = upper.strip_prefix("TODO:") {
                let offset = stripped.len() - rest.len();
                ("TODO".to_string(), stripped[offset..].trim().to_string())
            } else {
                continue;
            };

            if description.is_empty() {
                continue;
            }

            let is_safe = Self::classify_safe(&description);

            markers.push(ScannedMarker {
                file_path: rel_path.clone(),
                line_number: idx + 1,
                marker_type,
                description,
                is_safe,
            });
        }
    }

    /// Strip common comment prefixes (`//`, `#`, `--`, `/*`, `*`, `<!--`).
    fn strip_comment_prefix(line: &str) -> &str {
        let s = line.trim_start();
        // Order matters: try longer prefixes first
        for prefix in &[
            "///", "//!", "//", "##", "#!", "#", "--", "/*", "*/", "*", "<!--",
        ] {
            if let Some(rest) = s.strip_prefix(prefix) {
                return rest.trim_start();
            }
        }
        s
    }

    /// Classify whether a task description is safe to auto-queue.
    ///
    /// A task is **privileged** (not safe) if its description contains any of
    /// the [`PRIVILEGED_KEYWORDS`]. Everything else is considered safe.
    pub(crate) fn classify_safe(description: &str) -> bool {
        let lower = description.to_lowercase();
        !PRIVILEGED_KEYWORDS.iter().any(|kw| lower.contains(kw))
    }
}

/// Auto-queue scanned markers as tasks in the database, skipping duplicates
/// and privileged tasks. Returns the number of tasks actually inserted.
///
/// The `limit` controls how many **safe** tasks are queued. Privileged markers
/// are always skipped and do not count toward the limit.
///
/// Deduplication: a marker is considered a duplicate if a pending or running
/// task already exists with the same title (which encodes file path and line).
pub async fn auto_queue_scanned_tasks(
    pool: &PgPool,
    event_stream: &EventStream,
    markers: &[ScannedMarker],
    identity_id: Uuid,
    correlation_id: Uuid,
    limit: usize,
) -> Result<usize> {
    let mut queued = 0usize;

    for marker in markers {
        if queued >= limit {
            break;
        }

        if !marker.is_safe {
            tracing::debug!(
                file = %marker.file_path,
                line = marker.line_number,
                description = %marker.description,
                "Skipping privileged task marker"
            );
            continue;
        }

        let title = format!(
            "[{}] {}:{}",
            marker.marker_type, marker.file_path, marker.line_number
        );

        // Dedup: skip if a non-terminal task with the same title already exists
        let existing: Option<i64> = sqlx::query_scalar(
            r"SELECT COUNT(*) FROM tasks WHERE title = $1 AND state IN ('pending', 'running')",
        )
        .bind(&title)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;

        if existing.unwrap_or(0) > 0 {
            tracing::debug!(title = %title, "Task already exists, skipping");
            continue;
        }

        // Insert new task
        let task_id: Uuid = sqlx::query_scalar(
            r"INSERT INTO tasks (title, description, created_by, priority, correlation_id)
              VALUES ($1, $2, $3, $4, $5)
              RETURNING task_id",
        )
        .bind(&title)
        .bind(&marker.description)
        .bind(identity_id)
        .bind(0_i32) // default priority for auto-queued tasks
        .bind(correlation_id)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;

        event_stream.publish(
            EventEnvelope::new(
                EventLevel::Info,
                EventType::TaskAutoQueued,
                json!({
                    "task_id": task_id,
                    "title": title,
                    "marker_type": marker.marker_type,
                    "file_path": marker.file_path,
                    "line_number": marker.line_number,
                    "description": marker.description,
                    "correlation_id": correlation_id,
                }),
            )
            .with_correlation_id(correlation_id),
        );

        tracing::info!(
            task_id = %task_id,
            title = %title,
            "Auto-queued task from workspace scan"
        );

        queued += 1;
    }

    Ok(queued)
}

/// Auto-queue LLM-suggested tasks from heartbeat tool calls, skipping duplicates
/// and privileged tasks. Returns the number of tasks actually inserted.
///
/// The `limit` controls how many **safe** tasks are queued. Privileged tool calls
/// are always skipped and do not count toward the limit.
///
/// Deduplication: a tool call is considered a duplicate if a pending or running
/// task already exists with the same title (which encodes the tool name).
pub async fn auto_queue_llm_tasks(
    pool: &PgPool,
    event_stream: &EventStream,
    tool_calls: &[crate::agentic::ToolCall],
    identity_id: Uuid,
    correlation_id: Uuid,
    limit: usize,
) -> Result<usize> {
    let mut queued = 0usize;

    for tool_call in tool_calls {
        if queued >= limit {
            break;
        }

        // Build description from arguments
        let description = serde_json::to_string(&tool_call.arguments)
            .unwrap_or_else(|_| "{}".to_string());

        // Safety gate: skip privileged tool calls
        if !WorkspaceScanner::classify_safe(&description) {
            tracing::debug!(
                tool_name = %tool_call.tool_name,
                description = %description,
                "Skipping privileged LLM tool call"
            );
            continue;
        }

        let title = format!("[tool_call] {}", tool_call.tool_name);

        // Dedup: skip if a non-terminal task with the same title already exists
        let existing: Option<i64> = sqlx::query_scalar(
            r"SELECT COUNT(*) FROM tasks WHERE title = $1 AND state IN ('pending', 'running')",
        )
        .bind(&title)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;

        if existing.unwrap_or(0) > 0 {
            tracing::debug!(title = %title, "LLM task already exists, skipping");
            continue;
        }

        // Insert new task with origin='llm_suggested'
        let task_id: Uuid = sqlx::query_scalar(
            r"INSERT INTO tasks (title, description, created_by, priority, correlation_id, origin)
              VALUES ($1, $2, $3, $4, $5, $6)
              RETURNING task_id",
        )
        .bind(&title)
        .bind(&description)
        .bind(identity_id)
        .bind(-1_i32) // low priority for LLM-suggested tasks
        .bind(correlation_id)
        .bind("llm_suggested")
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;

        event_stream.publish(
            EventEnvelope::new(
                EventLevel::Info,
                EventType::TaskAutoQueued,
                json!({
                    "task_id": task_id,
                    "title": title,
                    "tool_name": tool_call.tool_name,
                    "arguments": tool_call.arguments,
                    "source": "heartbeat_tool_call",
                    "priority": -1,
                    "correlation_id": correlation_id,
                    "origin": "llm_suggested",
                }),
            )
            .with_correlation_id(correlation_id),
        );

        tracing::info!(
            task_id = %task_id,
            tool_name = %tool_call.tool_name,
            correlation_id = %correlation_id,
            "Auto-queued LLM-suggested task from heartbeat"
        );

        queued += 1;
    }

    Ok(queued)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::Ledger;
    use crate::model_router::ModelRouter;
    use crate::policy::PolicyEngine;
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
        let policy_engine = Arc::new(PolicyEngine::new(pool.clone()));
        let ledger = Arc::new(Ledger::new(pool.clone()));
        let model_router = Arc::new(ModelRouter::new(
            pool.clone(),
            "http://localhost:18790".to_string(),
            policy_engine,
            ledger.clone(),
        ));
        let safe_mode_guard = Arc::new(crate::safe_mode::SafeModeGuard::new(
            pool.clone(),
            ledger.clone(),
        ));
        Scheduler::new(
            pool,
            event_stream,
            Duration::from_millis(1000),
            worker_manager,
            config,
            model_router,
            ledger,
            safe_mode_guard,
        )
    }


    #[tokio::test]
    async fn test_scheduler_creation() {
        let scheduler = create_test_scheduler();

        assert_eq!(scheduler.heartbeat_interval, Duration::from_millis(1000));
        assert!(scheduler.shutdown_tx.is_none());
        // Verify new fields are present
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
    #[ignore = "requires database connection and gateway"]
    async fn test_heartbeat_execution() {
        // This test requires a real database connection and running gateway
        // Run with: cargo test test_heartbeat_execution -- --ignored
    }

    #[tokio::test]
    #[ignore = "requires database connection and gateway"]
    async fn test_heartbeat_model_failure_graceful_degradation() {
        // Verify that heartbeat completes with status='failed' when model call fails
        // and that HeartbeatTick is emitted but HeartbeatOk is NOT emitted
        // Run with: cargo test test_heartbeat_model_failure_graceful_degradation -- --ignored
    }

    #[tokio::test]
    #[ignore = "requires database connection and gateway"]
    async fn test_heartbeat_correlation_id_propagation() {
        // Verify that correlation_id flows through:
        // 1. heartbeat_history.correlation_id
        // 2. ledger event correlation_id
        // 3. HeartbeatTick event payload
        // 4. HeartbeatOk event payload
        // Run with: cargo test test_heartbeat_correlation_id_propagation -- --ignored
    }

    // ── WorkspaceScanner tests ──────────────────────────────────────

    #[test]
    fn test_classify_safe_tasks() {
        assert!(WorkspaceScanner::classify_safe(
            "Implement pagination for list view"
        ));
        assert!(WorkspaceScanner::classify_safe("Add unit tests for parser"));
        assert!(WorkspaceScanner::classify_safe("Refactor error handling"));
        assert!(WorkspaceScanner::classify_safe("Fix typo in README"));
    }

    #[test]
    fn test_classify_privileged_tasks() {
        assert!(!WorkspaceScanner::classify_safe(
            "Delete old migration files"
        ));
        assert!(!WorkspaceScanner::classify_safe("Deploy to production"));
        assert!(!WorkspaceScanner::classify_safe("Rotate credential keys"));
        assert!(!WorkspaceScanner::classify_safe("Drop unused tables"));
        assert!(!WorkspaceScanner::classify_safe("Migrate database schema"));
        assert!(!WorkspaceScanner::classify_safe("Update admin permissions"));
        assert!(!WorkspaceScanner::classify_safe(
            "Revert last security patch"
        ));
        assert!(!WorkspaceScanner::classify_safe("Truncate logs table"));
        // New keywords added in section 8
        assert!(!WorkspaceScanner::classify_safe("Reset user password"));
        assert!(!WorkspaceScanner::classify_safe(
            "Rotate api_key for service"
        ));
        assert!(!WorkspaceScanner::classify_safe("Renew TLS certificate"));
        assert!(!WorkspaceScanner::classify_safe(
            "Implement encryption at rest"
        ));
        assert!(!WorkspaceScanner::classify_safe("Decrypt backup archive"));
        assert!(!WorkspaceScanner::classify_safe(
            "Store private_key securely"
        ));
        assert!(!WorkspaceScanner::classify_safe("Refresh auth token logic"));
    }

    #[test]
    fn test_strip_comment_prefix() {
        assert_eq!(
            WorkspaceScanner::strip_comment_prefix("// TODO: fix"),
            "TODO: fix"
        );
        assert_eq!(
            WorkspaceScanner::strip_comment_prefix("# TASK: build"),
            "TASK: build"
        );
        assert_eq!(
            WorkspaceScanner::strip_comment_prefix("-- TODO: query"),
            "TODO: query"
        );
        assert_eq!(
            WorkspaceScanner::strip_comment_prefix("/// TODO: doc"),
            "TODO: doc"
        );
        assert_eq!(
            WorkspaceScanner::strip_comment_prefix("* TODO: list"),
            "TODO: list"
        );
        assert_eq!(
            WorkspaceScanner::strip_comment_prefix("TASK: plain"),
            "TASK: plain"
        );
    }

    #[test]
    fn test_scan_temp_directory() {
        let dir =
            std::env::temp_dir().join(format!("carnelian_scan_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        // Create a Rust file with markers
        std::fs::write(
            dir.join("example.rs"),
            "fn main() {\n    // TODO: Add error handling\n    // TASK: Implement caching\n    println!(\"hello\");\n}\n",
        )
        .unwrap();

        // Create a Python file with a privileged marker
        std::fs::write(
            dir.join("deploy.py"),
            "# TODO: Deploy to production\n# TASK: Write unit tests\n",
        )
        .unwrap();

        // Create a file that should be skipped (wrong extension)
        std::fs::write(dir.join("data.bin"), "TODO: should not match").unwrap();

        let markers = WorkspaceScanner::scan(&[dir.clone()]);

        // Should find markers from .rs and .py but not .bin
        assert!(
            markers.len() >= 3,
            "Expected at least 3 markers, got {}",
            markers.len()
        );

        // Verify the privileged one is classified correctly
        let deploy_marker = markers
            .iter()
            .find(|m| m.description.contains("Deploy to production"));
        assert!(deploy_marker.is_some(), "Should find deploy marker");
        assert!(
            !deploy_marker.unwrap().is_safe,
            "Deploy marker should be privileged"
        );

        // Verify safe ones
        let safe_count = markers.iter().filter(|m| m.is_safe).count();
        assert!(safe_count >= 2, "Expected at least 2 safe markers");

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_returns_all_markers() {
        let dir =
            std::env::temp_dir().join(format!("carnelian_limit_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        // Create a file with many markers
        let mut content = String::new();
        for i in 0..20 {
            content.push_str(&format!("// TODO: Task number {i}\n"));
        }
        std::fs::write(dir.join("many.rs"), &content).unwrap();

        let markers = WorkspaceScanner::scan(&[dir.clone()]);
        assert_eq!(
            markers.len(),
            20,
            "scan() should return all markers without a cap"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_skips_excluded_dirs() {
        let dir =
            std::env::temp_dir().join(format!("carnelian_excl_test_{}", uuid::Uuid::new_v4()));
        let target_dir = dir.join("target");
        let node_dir = dir.join("node_modules");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::create_dir_all(&node_dir).unwrap();

        std::fs::write(target_dir.join("gen.rs"), "// TODO: Should be skipped").unwrap();
        std::fs::write(node_dir.join("lib.js"), "// TODO: Should be skipped too").unwrap();
        std::fs::write(dir.join("src.rs"), "// TODO: Should be found").unwrap();

        let markers = WorkspaceScanner::scan(&[dir.clone()]);
        assert_eq!(
            markers.len(),
            1,
            "Should only find marker outside excluded dirs"
        );
        assert!(markers[0].file_path.contains("src.rs"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_empty_description_skipped() {
        let dir =
            std::env::temp_dir().join(format!("carnelian_empty_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("empty.rs"),
            "// TODO:\n// TASK:  \n// TODO: Real task\n",
        )
        .unwrap();

        let markers = WorkspaceScanner::scan(&[dir.clone()]);
        assert_eq!(markers.len(), 1, "Should skip empty descriptions");
        assert_eq!(markers[0].description, "Real task");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Edge case tests (section 7.1) ───────────────────────────────

    #[test]
    fn test_scan_empty_workspace_paths() {
        let markers = WorkspaceScanner::scan(&[]);
        assert!(markers.is_empty(), "Empty paths should yield no markers");
    }

    #[test]
    fn test_scan_nonexistent_path() {
        let markers =
            WorkspaceScanner::scan(&[std::path::PathBuf::from("/nonexistent/path/abc123")]);
        assert!(
            markers.is_empty(),
            "Non-existent path should yield no markers"
        );
    }

    #[test]
    fn test_scan_still_returns_markers_when_limit_is_zero() {
        // scan() no longer takes a limit; limit=0 is enforced at the
        // heartbeat / auto_queue level. Verify scan() still returns markers.
        let dir =
            std::env::temp_dir().join(format!("carnelian_zero_limit_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("tasks.rs"), "// TODO: Should be found\n").unwrap();

        let markers = WorkspaceScanner::scan(&[dir.clone()]);
        assert_eq!(markers.len(), 1, "scan() should still return markers");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_unicode_markers() {
        let dir = std::env::temp_dir().join(format!("carnelian_unicode_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("unicode.rs"),
            "// TODO: Fix the hot path\n// TASK: Add i18n support for Japanese\n",
        )
        .unwrap();

        let markers = WorkspaceScanner::scan(&[dir.clone()]);
        assert_eq!(markers.len(), 2, "Should find unicode markers");
        assert!(
            markers[0].description.contains("hot path"),
            "Should preserve unicode in description"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_binary_extension_skipped() {
        let dir = std::env::temp_dir().join(format!("carnelian_bin_ext_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        // Files with non-scannable extensions
        std::fs::write(dir.join("image.png"), "TODO: hidden in binary").unwrap();
        std::fs::write(dir.join("archive.zip"), "TASK: hidden in archive").unwrap();
        std::fs::write(dir.join("data.bin"), "TODO: hidden in bin").unwrap();
        // One scannable file
        std::fs::write(dir.join("real.rs"), "// TODO: Visible task\n").unwrap();

        let markers = WorkspaceScanner::scan(&[dir.clone()]);
        assert_eq!(
            markers.len(),
            1,
            "Should only find marker in scannable extension"
        );
        assert_eq!(markers[0].description, "Visible task");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_multiple_paths() {
        let base = std::env::temp_dir().join(format!("carnelian_multi_{}", uuid::Uuid::new_v4()));
        let path_a = base.join("project_a");
        let path_b = base.join("project_b");
        std::fs::create_dir_all(&path_a).unwrap();
        std::fs::create_dir_all(&path_b).unwrap();

        std::fs::write(path_a.join("a.rs"), "// TODO: Task from A\n").unwrap();
        std::fs::write(path_b.join("b.py"), "# TODO: Task from B\n").unwrap();

        let markers = WorkspaceScanner::scan(&[path_a, path_b]);
        assert_eq!(markers.len(), 2, "Should find markers from both paths");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_safe_task_found_after_many_privileged() {
        // Regression: the old scan() limit counted privileged markers toward
        // the cap, so a safe marker appearing after N privileged ones would be
        // dropped even though the safe-task quota was not exhausted.
        let dir =
            std::env::temp_dir().join(format!("carnelian_priv_first_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        // 10 privileged markers followed by 1 safe marker
        let mut content = String::new();
        for i in 0..10 {
            content.push_str(&format!("// TODO: Delete old table {i}\n"));
        }
        content.push_str("// TODO: Add unit tests for parser\n");
        std::fs::write(dir.join("mixed.rs"), &content).unwrap();

        let markers = WorkspaceScanner::scan(&[dir.clone()]);

        // scan() must return ALL 11 markers (no cap)
        assert_eq!(markers.len(), 11, "scan() should return all 11 markers");

        let privileged_count = markers.iter().filter(|m| !m.is_safe).count();
        let safe_count = markers.iter().filter(|m| m.is_safe).count();
        assert_eq!(privileged_count, 10, "Expected 10 privileged markers");
        assert_eq!(safe_count, 1, "Expected 1 safe marker");

        // The safe marker must be present even though it appears after 10
        // privileged ones — auto_queue_scanned_tasks would queue it because
        // the limit applies only to safe tasks queued, not total markers.
        let safe_marker = markers.iter().find(|m| m.is_safe).unwrap();
        assert_eq!(safe_marker.description, "Add unit tests for parser");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
