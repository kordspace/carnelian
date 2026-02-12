//! HTTP and WebSocket server for 🔥 Carnelian OS
//!
//! This module provides the Axum-based HTTP server with WebSocket support
//! for real-time event streaming to UI clients.

use axum::{
    Json, Router,
    extract::{Path, Query, State, WebSocketUpgrade, ws::Message},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use carnelian_common::Result;
use carnelian_common::types::{
    CancelTaskRequest, CancelTaskResponse, CreateTaskRequest, CreateTaskResponse, EventEnvelope,
    EventLevel, EventType, ListRunsResponse, ListSkillsResponse, ListTasksResponse,
    PaginatedRunLogsResponse, RunDetail, RunLogEntry, RunLogsQuery, SkillDetail,
    SkillToggleResponse, TaskDetail,
};
use futures_util::{SinkExt, StreamExt};
use http::{Method, header};
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    limit::RequestBodyLimitLayer,
    timeout::TimeoutLayer,
    trace::{DefaultOnResponse, MakeSpan, TraceLayer},
};
use tracing::{Level, Span};
use uuid::Uuid;

use crate::ledger::Ledger;
use crate::metrics::MetricsCollector;
use crate::worker::WorkerManager;
use crate::{Config, EventStream, Scheduler, db, policy::PolicyEngine};

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Overall health status: "healthy" or "degraded"
    pub status: String,
    /// Application version
    pub version: String,
    /// Database connection status: "connected" or "disconnected"
    pub database: String,
}

/// System status response
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    /// Active workers
    pub workers: Vec<WorkerInfo>,
    /// Available models
    pub models: Vec<String>,
    /// Number of tasks in queue
    pub queue_depth: u32,
}

/// Worker information
#[derive(Debug, Serialize)]
pub struct WorkerInfo {
    /// Worker identifier
    pub id: String,
    /// Worker status
    pub status: String,
    /// Currently executing task, if any
    pub current_task: Option<String>,
}

/// Custom span maker that generates correlation IDs for each request
#[derive(Clone)]
struct CorrelationIdMakeSpan;

impl<B> MakeSpan<B> for CorrelationIdMakeSpan {
    fn make_span(&mut self, request: &http::Request<B>) -> Span {
        let correlation_id = Uuid::now_v7();
        tracing::info_span!(
            "request",
            method = %request.method(),
            uri = %request.uri(),
            correlation_id = %correlation_id,
        )
    }
}

/// Shared application state for request handlers
#[derive(Clone)]
pub struct AppState {
    /// Application configuration with database pool
    pub config: Arc<Config>,
    /// Event stream for publishing and subscribing
    pub event_stream: Arc<EventStream>,
    /// Policy engine for capability-based security
    pub policy_engine: Arc<PolicyEngine>,
    /// Audit ledger for tamper-resistant logging
    pub ledger: Arc<Ledger>,
    /// Worker manager for process lifecycle
    pub worker_manager: Arc<tokio::sync::Mutex<WorkerManager>>,
    /// Task scheduler for creating/cancelling tasks
    pub scheduler: Arc<tokio::sync::Mutex<Scheduler>>,
    /// Performance metrics collector
    pub metrics: Arc<MetricsCollector>,
    /// Correlation ID counter for request tracing
    correlation_counter: Arc<AtomicU64>,
}

impl AppState {
    /// Create new application state
    #[must_use]
    pub fn new(
        config: Arc<Config>,
        event_stream: Arc<EventStream>,
        policy_engine: Arc<PolicyEngine>,
        ledger: Arc<Ledger>,
        worker_manager: Arc<tokio::sync::Mutex<WorkerManager>>,
        scheduler: Arc<tokio::sync::Mutex<Scheduler>>,
    ) -> Self {
        Self {
            config,
            event_stream,
            policy_engine,
            ledger,
            worker_manager,
            scheduler,
            metrics: Arc::new(MetricsCollector::new()),
            correlation_counter: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Generate next correlation ID
    #[must_use]
    pub fn next_correlation_id(&self) -> u64 {
        self.correlation_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Check database health
    ///
    /// Returns `true` if database is reachable, `false` otherwise.
    /// Errors are logged at DEBUG level since health checks are routine.
    pub async fn check_database_health(&self) -> bool {
        match self.config.pool() {
            Ok(pool) => match db::check_database_health(pool).await {
                Ok(healthy) => healthy,
                Err(e) => {
                    tracing::debug!("Database health check failed: {}", e);
                    false
                }
            },
            Err(e) => {
                tracing::debug!("Database pool not available: {}", e);
                false
            }
        }
    }
}

/// HTTP and WebSocket server
pub struct Server {
    /// Application configuration
    config: Arc<Config>,
    /// Event stream for real-time updates
    event_stream: Arc<EventStream>,
    /// Policy engine for capability-based security
    policy_engine: Arc<PolicyEngine>,
    /// Audit ledger for tamper-resistant logging
    ledger: Arc<Ledger>,
    /// Background task scheduler
    scheduler: Arc<tokio::sync::Mutex<Scheduler>>,
    /// Worker process manager
    worker_manager: Arc<tokio::sync::Mutex<WorkerManager>>,
}

impl Server {
    /// Create a new server with the given configuration, event stream, policy engine, ledger, scheduler, and worker manager.
    #[must_use]
    pub fn new(
        config: Arc<Config>,
        event_stream: Arc<EventStream>,
        policy_engine: Arc<PolicyEngine>,
        ledger: Arc<Ledger>,
        scheduler: Arc<tokio::sync::Mutex<Scheduler>>,
        worker_manager: Arc<tokio::sync::Mutex<WorkerManager>>,
    ) -> Self {
        Self {
            config,
            event_stream,
            policy_engine,
            ledger,
            scheduler,
            worker_manager,
        }
    }

    /// Get the HTTP port from configuration.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.config.http_port
    }

    /// Run the server with graceful shutdown support.
    ///
    /// This method starts the HTTP server and listens for shutdown signals
    /// (SIGINT/SIGTERM). On shutdown, it waits for in-flight requests to
    /// complete before exiting.
    pub async fn run(&self) -> Result<()> {
        self.run_with_shutdown(shutdown_signal()).await
    }

    /// Run the server with a custom shutdown signal.
    ///
    /// This method is useful for testing graceful shutdown behavior
    /// without relying on OS signals.
    #[allow(clippy::too_many_lines)]
    pub async fn run_with_shutdown<F>(&self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let state = AppState::new(
            self.config.clone(),
            self.event_stream.clone(),
            self.policy_engine.clone(),
            self.ledger.clone(),
            self.worker_manager.clone(),
            self.scheduler.clone(),
        );

        // Share the metrics collector with the scheduler and event stream
        {
            let mut scheduler = self.scheduler.lock().await;
            scheduler.set_metrics(state.metrics.clone());
        }
        state.event_stream.set_metrics(state.metrics.clone());

        let router = build_router(state.clone());

        // Start the scheduler background task
        {
            let mut scheduler = self.scheduler.lock().await;
            if let Err(e) = scheduler.start().await {
                tracing::warn!(error = %e, "Failed to start scheduler, continuing without heartbeats");
            }
        }

        // Start worker processes
        {
            let mut worker_manager = self.worker_manager.lock().await;
            if let Err(e) = worker_manager.start_workers().await {
                tracing::warn!(error = %e, "Failed to start workers");
            }
        }

        // Initial skill discovery scan
        if let Ok(pool) = self.config.pool() {
            let discovery = crate::skills::SkillDiscovery::new(
                pool.clone(),
                Some(self.event_stream.clone()),
                self.config.skills_registry_path.clone(),
            );
            match discovery.refresh().await {
                Ok(r) => {
                    if r.discovered > 0 || r.updated > 0 || r.removed > 0 {
                        tracing::info!(
                            discovered = r.discovered,
                            updated = r.updated,
                            removed = r.removed,
                            "Initial skill discovery complete"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Initial skill discovery failed, continuing");
                }
            }
        }

        // Start file watcher for automatic skill discovery
        let skill_watcher_handle = self.config.pool().map_or_else(
            |_| None,
            |pool| {
                Some(crate::skills::start_file_watcher(
                    pool.clone(),
                    self.event_stream.clone(),
                    self.config.skills_registry_path.clone(),
                ))
            },
        );

        // Initial soul file sync
        if let Ok(pool) = self.config.pool() {
            let soul_manager = crate::soul::SoulManager::new(
                pool.clone(),
                Some(self.event_stream.clone()),
                self.config.souls_path.clone(),
            );
            if let Err(e) = soul_manager.watch().await {
                tracing::warn!(error = %e, "Initial soul file sync failed, continuing");
            }
        }

        // Start file watcher for automatic soul file sync
        let soul_watcher_handle = self.config.pool().map_or_else(
            |_| None,
            |pool| {
                Some(crate::soul::start_soul_watcher(
                    pool.clone(),
                    self.event_stream.clone(),
                    self.config.souls_path.clone(),
                ))
            },
        );

        let addr = format!("{}:{}", self.config.bind_address, self.config.http_port);
        let listener = TcpListener::bind(&addr).await.map_err(|e| {
            carnelian_common::Error::Connection(format!("Failed to bind to {}: {}", addr, e))
        })?;

        tracing::info!("🔥 Carnelian server listening on {}", addr);

        // Publish runtime ready event
        state.event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::RuntimeReady,
            json!({"port": self.config.http_port}),
        ));

        // Serve with graceful shutdown
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|e| carnelian_common::Error::Connection(format!("Server error: {}", e)))?;

        // Stop file watchers
        if let Some(handle) = skill_watcher_handle {
            handle.abort();
            tracing::debug!("Skill file watcher stopped");
        }
        if let Some(handle) = soul_watcher_handle {
            handle.abort();
            tracing::debug!("Soul file watcher stopped");
        }

        // Stop workers before scheduler shutdown
        {
            let mut worker_manager = self.worker_manager.lock().await;
            if let Err(e) = worker_manager.stop_all_workers().await {
                tracing::warn!(error = %e, "Failed to stop workers gracefully");
            }
        }

        // Shutdown the scheduler before publishing shutdown event
        {
            let mut scheduler = self.scheduler.lock().await;
            if let Err(e) = scheduler.shutdown().await {
                tracing::warn!(error = %e, "Failed to shutdown scheduler gracefully");
            }
        }

        // Publish shutdown event
        state.event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::RuntimeShutdown,
            json!({"reason": "graceful_shutdown"}),
        ));

        tracing::info!("🔥 Carnelian server shut down gracefully");
        Ok(())
    }
}

/// Allowed origins for CORS (local UI development)
const ALLOWED_ORIGINS: [&str; 4] = [
    "http://localhost:3000",
    "http://localhost:5173",
    "http://127.0.0.1:3000",
    "http://127.0.0.1:5173",
];

/// Build the Axum router with all routes and middleware.
#[allow(deprecated)] // TimeoutLayer::new is deprecated but simpler than with_status_code
fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health_handler))
        .route("/v1/status", get(status_handler))
        .route("/v1/events", post(publish_event_handler))
        .route("/v1/events/ws", get(ws_handler))
        // Task endpoints
        .route("/v1/tasks", post(create_task_handler))
        .route("/v1/tasks", get(list_tasks_handler))
        .route("/v1/tasks/{task_id}", get(get_task_handler))
        .route("/v1/tasks/{task_id}/cancel", post(cancel_task_handler))
        .route("/v1/tasks/{task_id}/runs", get(list_runs_handler))
        // Run endpoints
        .route("/v1/runs/{run_id}", get(get_run_handler))
        .route("/v1/runs/{run_id}/logs", get(get_run_logs_handler))
        // Skill endpoints
        .route("/v1/skills", get(list_skills_handler))
        .route("/v1/skills/{skill_id}/enable", post(enable_skill_handler))
        .route("/v1/skills/{skill_id}/disable", post(disable_skill_handler))
        .route("/v1/skills/refresh", post(refresh_skills_handler))
        // Metrics endpoint
        .route("/v1/metrics", get(metrics_handler))
        // 10MB request body limit
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024))
        // 30-second timeout
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        // Compression (gzip, brotli)
        .layer(CompressionLayer::new())
        // CORS restricted to local UI development origins
        .layer(
            CorsLayer::new()
                .allow_origin(
                    ALLOWED_ORIGINS
                        .iter()
                        .filter_map(|s| s.parse().ok())
                        .collect::<Vec<_>>(),
                )
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT]),
        )
        // Request tracing with correlation IDs (UUID v7)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(CorrelationIdMakeSpan)
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(state)
}

/// Health check endpoint handler.
///
/// Returns the overall health status of the system including database connectivity.
async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let db_healthy = state.check_database_health().await;

    let response = HealthResponse {
        status: if db_healthy {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
        version: carnelian_common::VERSION.to_string(),
        database: if db_healthy {
            "connected".to_string()
        } else {
            "disconnected".to_string()
        },
    };

    tracing::info!(
        status = %response.status,
        database = %response.database,
        version = %response.version,
        "Health check completed"
    );

    Json(response)
}

/// System status endpoint handler.
///
/// Returns current system status including workers, models, and queue depth.
/// Note: Workers and models will be populated when those systems are implemented.
async fn status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let workers = {
        let worker_manager = state.worker_manager.lock().await;
        worker_manager.get_worker_status().await
    };

    let response = StatusResponse {
        workers,
        models: vec![],
        queue_depth: 0,
    };

    tracing::debug!(
        workers = response.workers.len(),
        models = response.models.len(),
        queue_depth = response.queue_depth,
        "Status check completed"
    );

    Json(response)
}

/// Publish event handler — accepts JSON event payloads via HTTP POST.
///
/// Publishes the event to the EventStream so WebSocket subscribers receive it.
async fn publish_event_handler(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let event_type_str = body["event_type"].as_str().unwrap_or("Custom");
    let level_str = body["level"].as_str().unwrap_or("Info");

    let level = match level_str {
        "Error" | "ERROR" => EventLevel::Error,
        "Warn" | "WARN" => EventLevel::Warn,
        "Debug" | "DEBUG" => EventLevel::Debug,
        "Trace" | "TRACE" => EventLevel::Trace,
        _ => EventLevel::Info,
    };

    let event_type = match event_type_str {
        "TaskCreated" => EventType::TaskCreated,
        "TaskStarted" => EventType::TaskStarted,
        "TaskCompleted" => EventType::TaskCompleted,
        "TaskFailed" => EventType::TaskFailed,
        "WorkerStarted" => EventType::WorkerStarted,
        "WorkerStopped" => EventType::WorkerStopped,
        other => EventType::Custom(other.to_string()),
    };

    let data = body.get("data").cloned().unwrap_or_else(|| json!({}));
    state
        .event_stream
        .publish(EventEnvelope::new(level, event_type, data));

    Json(json!({"status": "ok"}))
}

// =============================================================================
// METRICS HANDLER
// =============================================================================

/// Metrics endpoint handler — returns aggregated performance metrics.
///
/// Returns task latency percentiles, event throughput, and stream stats.
async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    let event_stats = state.event_stream.stats();
    let snapshot = state.metrics.get_snapshot(&event_stats);

    // Convert to the common type for serialization
    let response = carnelian_common::types::MetricsSnapshot {
        task_latency: carnelian_common::types::LatencyStats {
            mean_ms: snapshot.task_latency.mean_ms,
            median_ms: snapshot.task_latency.median_ms,
            p50_ms: snapshot.task_latency.p50_ms,
            p95_ms: snapshot.task_latency.p95_ms,
            p99_ms: snapshot.task_latency.p99_ms,
            sample_count: snapshot.task_latency.sample_count,
        },
        event_throughput_per_sec: snapshot.event_throughput_per_sec,
        event_stream_buffer_len: snapshot.event_stream_buffer_len,
        event_stream_buffer_capacity: snapshot.event_stream_buffer_capacity,
        event_stream_fill_percentage: snapshot.event_stream_fill_percentage,
        event_stream_total_received: snapshot.event_stream_total_received,
        event_stream_total_stored: snapshot.event_stream_total_stored,
        event_stream_subscriber_count: snapshot.event_stream_subscriber_count,
        render_time_ms: snapshot.render_time_ms,
        timestamp: snapshot.timestamp,
    };

    Json(serde_json::to_value(response).unwrap_or_default())
}

// =============================================================================
// TASK HANDLERS
// =============================================================================

/// Create a new task via `POST /v1/tasks`.
async fn create_task_handler(
    State(state): State<AppState>,
    Json(body): Json<CreateTaskRequest>,
) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "database unavailable"})),
            )
                .into_response();
        }
    };

    // Look up the default identity (Lian) for created_by
    let identity_id: Option<Uuid> = sqlx::query_scalar(
        r"SELECT identity_id FROM identities WHERE name = 'Lian' AND identity_type = 'core' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let row: Option<(Uuid, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r"INSERT INTO tasks (title, description, skill_id, priority, requires_approval, created_by, state)
          VALUES ($1, $2, $3, $4, $5, $6, 'pending')
          RETURNING task_id, state, created_at",
    )
    .bind(&body.title)
    .bind(&body.description)
    .bind(body.skill_id)
    .bind(body.priority)
    .bind(body.requires_approval)
    .bind(identity_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    match row {
        Some((task_id, task_state, created_at)) => {
            state.event_stream.publish(
                EventEnvelope::new(
                    EventLevel::Info,
                    EventType::TaskCreated,
                    json!({
                        "task_id": task_id,
                        "title": body.title,
                        "skill_id": body.skill_id,
                        "priority": body.priority,
                    }),
                )
                .with_actor_id(task_id.to_string()),
            );

            (
                StatusCode::CREATED,
                Json(
                    serde_json::to_value(CreateTaskResponse {
                        task_id,
                        state: task_state,
                        created_at,
                    })
                    .unwrap_or_default(),
                ),
            )
                .into_response()
        }
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "failed to create task"})),
        )
            .into_response(),
    }
}

/// List all tasks via `GET /v1/tasks`.
#[allow(clippy::type_complexity)]
async fn list_tasks_handler(State(state): State<AppState>) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "database unavailable"})),
            )
                .into_response();
        }
    };

    let rows: Vec<(
        Uuid,
        String,
        Option<String>,
        Option<Uuid>,
        String,
        i32,
        bool,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        r"SELECT task_id, title, description, skill_id, state, priority, requires_approval, created_at, updated_at
          FROM tasks ORDER BY created_at DESC LIMIT 200",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let tasks: Vec<TaskDetail> = rows
        .into_iter()
        .map(
            |(
                task_id,
                title,
                description,
                skill_id,
                task_state,
                priority,
                requires_approval,
                created_at,
                updated_at,
            )| {
                TaskDetail {
                    task_id,
                    title,
                    description,
                    skill_id,
                    state: task_state,
                    priority,
                    requires_approval,
                    created_at,
                    updated_at,
                }
            },
        )
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::to_value(ListTasksResponse { tasks }).unwrap_or_default()),
    )
        .into_response()
}

/// Get a single task via `GET /v1/tasks/:task_id`.
#[allow(clippy::type_complexity)]
async fn get_task_handler(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "database unavailable"})),
            )
                .into_response();
        }
    };

    let row: Option<(
        Uuid,
        String,
        Option<String>,
        Option<Uuid>,
        String,
        i32,
        bool,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        r"SELECT task_id, title, description, skill_id, state, priority, requires_approval, created_at, updated_at
          FROM tasks WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    match row {
        Some((
            task_id,
            title,
            description,
            skill_id,
            task_state,
            priority,
            requires_approval,
            created_at,
            updated_at,
        )) => (
            StatusCode::OK,
            Json(
                serde_json::to_value(TaskDetail {
                    task_id,
                    title,
                    description,
                    skill_id,
                    state: task_state,
                    priority,
                    requires_approval,
                    created_at,
                    updated_at,
                })
                .unwrap_or_default(),
            ),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "task not found"})),
        )
            .into_response(),
    }
}

/// Cancel a task via `POST /v1/tasks/:task_id/cancel`.
async fn cancel_task_handler(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
    Json(body): Json<CancelTaskRequest>,
) -> impl IntoResponse {
    let reason = if body.reason.is_empty() {
        "cancelled via API".to_string()
    } else {
        body.reason
    };

    let scheduler = state.scheduler.lock().await;
    match scheduler.cancel_task(task_id, reason).await {
        Ok(()) => (
            StatusCode::OK,
            Json(
                serde_json::to_value(CancelTaskResponse {
                    task_id,
                    state: "canceled".to_string(),
                })
                .unwrap_or_default(),
            ),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{}", e)})),
        )
            .into_response(),
    }
}

/// List runs for a task via `GET /v1/tasks/:task_id/runs`.
#[allow(clippy::type_complexity)]
async fn list_runs_handler(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "database unavailable"})),
            )
                .into_response();
        }
    };

    let rows: Vec<(
        Uuid,
        Uuid,
        i32,
        Option<String>,
        String,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<i32>,
        Option<serde_json::Value>,
        Option<String>,
    )> = sqlx::query_as(
        r"SELECT run_id, task_id, attempt, worker_id, state, started_at, ended_at, exit_code, result, error
          FROM task_runs WHERE task_id = $1 ORDER BY attempt ASC",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let runs: Vec<RunDetail> = rows
        .into_iter()
        .map(
            |(
                run_id,
                task_id,
                attempt,
                worker_id,
                run_state,
                started_at,
                ended_at,
                exit_code,
                result,
                error,
            )| {
                RunDetail {
                    run_id,
                    task_id,
                    attempt,
                    worker_id,
                    state: run_state,
                    started_at,
                    ended_at,
                    exit_code,
                    result,
                    error,
                }
            },
        )
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::to_value(ListRunsResponse { runs }).unwrap_or_default()),
    )
        .into_response()
}

/// Get a single run by ID via `GET /v1/runs/:run_id`.
#[allow(clippy::type_complexity)]
async fn get_run_handler(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "database unavailable"})),
            )
                .into_response();
        }
    };

    let row: Option<(
        Uuid,
        Uuid,
        i32,
        Option<String>,
        String,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<i32>,
        Option<serde_json::Value>,
        Option<String>,
    )> = sqlx::query_as(
        r"SELECT run_id, task_id, attempt, worker_id, state, started_at, ended_at, exit_code, result, error
          FROM task_runs WHERE run_id = $1",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    match row {
        Some((
            run_id,
            task_id,
            attempt,
            worker_id,
            run_state,
            started_at,
            ended_at,
            exit_code,
            result,
            error,
        )) => {
            let detail = RunDetail {
                run_id,
                task_id,
                attempt,
                worker_id,
                state: run_state,
                started_at,
                ended_at,
                exit_code,
                result,
                error,
            };
            (
                StatusCode::OK,
                Json(serde_json::to_value(detail).unwrap_or_default()),
            )
                .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "run not found"})),
        )
            .into_response(),
    }
}

// =============================================================================
// RUN LOG HANDLERS
// =============================================================================

/// Get paginated logs for a run via `GET /v1/runs/:run_id/logs`.
#[allow(clippy::type_complexity)]
async fn get_run_logs_handler(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
    Query(params): Query<RunLogsQuery>,
) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "database unavailable"})),
            )
                .into_response();
        }
    };

    let page = params.page.max(1);
    let page_size = params
        .page_size
        .clamp(1, PaginatedRunLogsResponse::MAX_PAGE_SIZE);
    let offset = (page - 1) * page_size;

    let total: i64 =
        sqlx::query_scalar::<_, Option<i64>>(r"SELECT COUNT(*) FROM run_logs WHERE run_id = $1")
            .bind(run_id)
            .fetch_one(pool)
            .await
            .ok()
            .flatten()
            .unwrap_or(0);

    let rows: Vec<(
        i64,
        Uuid,
        chrono::DateTime<chrono::Utc>,
        String,
        String,
        Option<serde_json::Value>,
        bool,
    )> = sqlx::query_as(
        r"SELECT log_id, run_id, ts, level, message, fields, truncated
          FROM run_logs WHERE run_id = $1 ORDER BY ts ASC LIMIT $2 OFFSET $3",
    )
    .bind(run_id)
    .bind(i64::from(page_size))
    .bind(i64::from(offset))
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let logs: Vec<RunLogEntry> = rows
        .into_iter()
        .map(
            |(log_id, run_id, ts, level, message, fields, truncated)| RunLogEntry {
                log_id,
                run_id,
                ts,
                level,
                message,
                fields,
                truncated,
            },
        )
        .collect();

    (
        StatusCode::OK,
        Json(
            serde_json::to_value(PaginatedRunLogsResponse {
                logs,
                page,
                page_size,
                total,
            })
            .unwrap_or_default(),
        ),
    )
        .into_response()
}

// =============================================================================
// SKILL HANDLERS
// =============================================================================

/// List all skills via `GET /v1/skills`.
#[allow(clippy::type_complexity)]
async fn list_skills_handler(State(state): State<AppState>) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "database unavailable"})),
            )
                .into_response();
        }
    };

    let rows: Vec<(
        Uuid,
        String,
        Option<String>,
        String,
        bool,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        r"SELECT skill_id, name, description, runtime, enabled, discovered_at, updated_at
          FROM skills ORDER BY name ASC",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let skills: Vec<SkillDetail> = rows
        .into_iter()
        .map(
            |(skill_id, name, description, runtime, enabled, discovered_at, updated_at)| {
                SkillDetail {
                    skill_id,
                    name,
                    description,
                    runtime,
                    enabled,
                    discovered_at,
                    updated_at,
                }
            },
        )
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::to_value(ListSkillsResponse { skills }).unwrap_or_default()),
    )
        .into_response()
}

/// Enable a skill via `POST /v1/skills/:skill_id/enable`.
async fn enable_skill_handler(
    State(state): State<AppState>,
    Path(skill_id): Path<Uuid>,
) -> impl IntoResponse {
    toggle_skill(state, skill_id, true).await
}

/// Disable a skill via `POST /v1/skills/:skill_id/disable`.
async fn disable_skill_handler(
    State(state): State<AppState>,
    Path(skill_id): Path<Uuid>,
) -> impl IntoResponse {
    toggle_skill(state, skill_id, false).await
}

/// Shared logic for enable/disable skill.
async fn toggle_skill(state: AppState, skill_id: Uuid, enabled: bool) -> axum::response::Response {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "database unavailable"})),
            )
                .into_response();
        }
    };

    let result =
        sqlx::query(r"UPDATE skills SET enabled = $1, updated_at = NOW() WHERE skill_id = $2")
            .bind(enabled)
            .bind(skill_id)
            .execute(pool)
            .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => (
            StatusCode::OK,
            Json(
                serde_json::to_value(SkillToggleResponse { skill_id, enabled }).unwrap_or_default(),
            ),
        )
            .into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "skill not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{}", e)})),
        )
            .into_response(),
    }
}

/// Trigger skill refresh via `POST /v1/skills/refresh`.
///
/// Scans the skills registry directory for new, updated, or removed skill
/// manifests and synchronizes the database accordingly.
async fn refresh_skills_handler(State(state): State<AppState>) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p.clone(),
        Err(e) => {
            tracing::error!(error = %e, "Database pool unavailable for skill refresh");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database unavailable"})),
            )
                .into_response();
        }
    };

    let discovery = crate::skills::SkillDiscovery::new(
        pool,
        Some(state.event_stream.clone()),
        state.config.skills_registry_path.clone(),
    );

    match discovery.refresh().await {
        Ok(result) => (
            StatusCode::OK,
            Json(serde_json::to_value(result).unwrap_or_default()),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Skill refresh failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Skill refresh failed: {}", e)})),
            )
                .into_response()
        }
    }
}

// =============================================================================
// WEBSOCKET HANDLERS
// =============================================================================

/// WebSocket upgrade handler for event streaming.
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

/// Handle an established WebSocket connection.
async fn handle_websocket(socket: axum::extract::ws::WebSocket, state: AppState) {
    let connection_start = std::time::Instant::now();
    let subscriber_count = state.event_stream.subscriber_count();

    tracing::info!(
        subscriber_count = subscriber_count,
        "WebSocket connection established"
    );

    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.event_stream.subscribe();

    // Spawn task to forward events to WebSocket
    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Forward events from broadcast channel
                result = rx.recv() => {
                    match result {
                        Ok(event) => {
                            match serde_json::to_string(&event) {
                                Ok(json) => {
                                    if sender.send(Message::Text(json.into())).await.is_err() {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to serialize event: {}", e);
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                            // Notify client about dropped events
                            tracing::warn!(
                                dropped_count = count,
                                "WebSocket client lagged, events dropped"
                            );
                            let msg = json!({
                                "type": "events_dropped",
                                "count": count,
                                "message": "Client fell behind, some events were dropped"
                            });
                            if let Ok(json) = serde_json::to_string(&msg) {
                                let _ = sender.send(Message::Text(json.into())).await;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                // Heartbeat ping every 30 seconds
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    if sender.send(Message::Ping(vec![].into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Handle incoming messages (for close/pong)
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Close(_)) => {
                let duration = connection_start.elapsed();
                tracing::debug!(
                    duration_secs = duration.as_secs(),
                    "WebSocket client closed connection"
                );
                break;
            }
            Ok(Message::Pong(_)) => {
                // Client responded to ping
            }
            Err(e) => {
                let duration = connection_start.elapsed();
                tracing::debug!(
                    duration_secs = duration.as_secs(),
                    error = %e,
                    "WebSocket error"
                );
                break;
            }
            _ => {}
        }
    }

    // Abort send task when receiver closes
    send_task.abort();
}

/// Create a shutdown signal that listens for SIGINT and SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            tracing::info!("Received SIGINT, initiating graceful shutdown...");
        }
        () = terminate => {
            tracing::info!("Received SIGTERM, initiating graceful shutdown...");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    fn create_test_state() -> AppState {
        let config = Arc::new(Config::default());
        let event_stream = Arc::new(EventStream::new(100, 10));
        // Create a lazy pool that won't connect until used - tests that don't hit DB will work
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgresql://test:test@localhost:5432/test")
            .expect("Failed to create lazy pool");
        let policy_engine = Arc::new(PolicyEngine::new(pool.clone()));
        let ledger = Arc::new(Ledger::new(pool.clone()));
        let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
            config.clone(),
            event_stream.clone(),
        )));
        let scheduler = Arc::new(tokio::sync::Mutex::new(Scheduler::new(
            pool.clone(),
            event_stream.clone(),
            Duration::from_secs(3600),
            worker_manager.clone(),
            config.clone(),
        )));
        AppState::new(
            config,
            event_stream,
            policy_engine,
            ledger,
            worker_manager,
            scheduler,
        )
    }

    #[tokio::test]
    async fn test_health_endpoint_structure() {
        let state = create_test_state();
        let router = build_router(state);

        let request = Request::builder()
            .uri("/v1/health")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // Verify structure
        assert!(json.get("status").is_some());
        assert!(json.get("version").is_some());
        assert!(json.get("database").is_some());

        // Without database, should be degraded
        assert_eq!(json["status"], "degraded");
        assert_eq!(json["database"], "disconnected");
        assert_eq!(json["version"], carnelian_common::VERSION);
    }

    #[tokio::test]
    async fn test_status_endpoint_structure() {
        let state = create_test_state();
        let router = build_router(state);

        let request = Request::builder()
            .uri("/v1/status")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // Verify structure
        assert!(json.get("workers").is_some());
        assert!(json.get("models").is_some());
        assert!(json.get("queue_depth").is_some());

        // Verify placeholder values
        assert_eq!(json["workers"], serde_json::json!([]));
        assert_eq!(json["models"], serde_json::json!([]));
        assert_eq!(json["queue_depth"], 0);
    }

    #[tokio::test]
    async fn test_health_response_content_type() {
        let state = create_test_state();
        let router = build_router(state);

        let request = Request::builder()
            .uri("/v1/health")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok());

        assert!(content_type.is_some());
        assert!(content_type.unwrap().contains("application/json"));
    }

    #[tokio::test]
    async fn test_status_response_content_type() {
        let state = create_test_state();
        let router = build_router(state);

        let request = Request::builder()
            .uri("/v1/status")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok());

        assert!(content_type.is_some());
        assert!(content_type.unwrap().contains("application/json"));
    }
}
