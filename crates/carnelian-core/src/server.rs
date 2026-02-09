//! HTTP and WebSocket server for 🔥 Carnelian OS
//!
//! This module provides the Axum-based HTTP server with WebSocket support
//! for real-time event streaming to UI clients.

use axum::{
    Json, Router,
    extract::{State, WebSocketUpgrade, ws::Message},
    response::IntoResponse,
    routing::{get, post},
};
use carnelian_common::Result;
use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
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
    ) -> Self {
        Self {
            config,
            event_stream,
            policy_engine,
            ledger,
            worker_manager,
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
        );
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

    let data = body.get("data").cloned().unwrap_or(json!({}));
    state
        .event_stream
        .publish(EventEnvelope::new(level, event_type, data));

    Json(json!({"status": "ok"}))
}

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
        let ledger = Arc::new(Ledger::new(pool));
        let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
            config.clone(),
            event_stream.clone(),
        )));
        AppState::new(config, event_stream, policy_engine, ledger, worker_manager)
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
