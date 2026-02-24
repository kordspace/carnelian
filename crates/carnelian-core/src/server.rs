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
use base64::Engine as _;
use carnelian_common::Result;
use carnelian_common::types::{
    AgentXpResponse, AwardXpRequest, AwardXpResponse, CancelTaskRequest, CancelTaskResponse,
    ConfigureVoiceRequest, ConfigureVoiceResponse, CreateMemoryRequest, CreateMemoryResponse,
    CreateTaskRequest, CreateTaskResponse, CreateWorkflowRequest, EventEnvelope, EventLevel,
    EventType, ExecuteWorkflowRequest, ExportMemoryRequest, ExportMemoryResponse,
    GetMemoryResponse, HeartbeatRecord, HeartbeatStatusResponse, IdentityResponse,
    ImportMemoryRequest, ImportMemoryResponse, LeaderboardEntry, ListMemoriesResponse,
    ListProvidersResponse, ListRunsResponse, ListSkillsResponse, ListTasksResponse,
    ListVoicesResponse, ListWorkflowsParams, ListWorkflowsResponse, MemoryDetail,
    MemoryImportResultApi, OllamaStatusResponse, PaginatedRunLogsResponse, ProviderDetail,
    RunDetail, RunLogEntry, RunLogsQuery, SkillDetail, SkillMetricsDetail, SkillToggleResponse,
    TaskDetail, TestVoiceRequest, TestVoiceResponse, TopSkillsQuery, TopSkillsResponse,
    TranscribeVoiceRequest, TranscribeVoiceResponse, UpdateWorkflowRequest, XpEventDetail,
    XpHistoryQuery, XpHistoryResponse, XpLeaderboardResponse,
};
use futures_util::{SinkExt, StreamExt};
use http::{HeaderMap, Method, header};
use serde::{Deserialize, Serialize};
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

use std::str::FromStr;

use crate::ledger::Ledger;
use crate::memory::ChainAnchor;
use crate::memory::{MemoryManager, MemoryQuery, MemorySource};
use crate::metrics::MetricsCollector;
use crate::model_router::ModelRouter;
use crate::safe_mode::SafeModeGuard;
use crate::session::SessionManager;
use crate::sub_agent::{CreateSubAgentRequest, SubAgentManager, UpdateSubAgentRequest};
use crate::worker::{WorkerManager, WorkerRuntime};
use crate::{Config, EventStream, Scheduler, db, policy::PolicyEngine};

use carnelian_common::{ChannelAdapter, ChannelAdapterFactory};
use std::collections::HashMap;
use tokio::sync::RwLock;

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

/// Detailed health check response with extended diagnostics
#[derive(Debug, Serialize)]
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
    pub last_heartbeat_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Whether the scheduler is running
    pub scheduler_running: bool,
    /// Whether the worker manager is active
    pub worker_manager_active: bool,
    /// Number of active event stream subscribers
    pub event_stream_subscriber_count: usize,
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
    /// UUID of the core identity (Lian)
    pub identity_id: Option<uuid::Uuid>,
    /// App version string
    pub version: String,
    /// Current machine profile name
    pub machine_profile: String,
    /// Seconds since server started
    pub uptime_seconds: Option<u64>,
}

/// Worker information
#[derive(Debug, Serialize)]
pub struct WorkerInfo {
    /// Worker identifier
    pub id: String,
    /// Worker runtime type (e.g., "node", "python", "shell")
    pub runtime: String,
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
    /// Model router for LLM completion requests via the gateway
    pub model_router: Arc<ModelRouter>,
    /// Safe mode guard for toggling safe mode
    pub safe_mode_guard: Arc<SafeModeGuard>,
    /// Session manager for conversation persistence (wired with SafeModeGuard)
    pub session_manager: Arc<SessionManager>,
    /// Memory manager for agent knowledge persistence
    pub memory_manager: Arc<MemoryManager>,
    /// Sub-agent lifecycle manager
    pub sub_agent_manager: Arc<SubAgentManager>,
    /// Workflow execution engine
    pub workflow_engine: Arc<crate::workflow::WorkflowEngine>,
    /// XP manager for experience point tracking and progression
    pub xp_manager: Arc<crate::xp::XpManager>,
    /// Voice gateway for ElevenLabs speech-to-text and text-to-speech
    pub voice_gateway: Arc<crate::voice::VoiceGateway>,
    /// Skill Book for curated skill catalog and activation
    pub skill_book: Arc<crate::skill_book::SkillBook>,
    /// Active channel adapters keyed by session_id
    pub channel_adapters: Arc<RwLock<HashMap<Uuid, Arc<dyn ChannelAdapter>>>>,
    /// Factory for building channel adapters from configuration.
    /// Injected by the binary to avoid cyclic crate dependencies.
    /// Signature: (state, session_id, channel_type, channel_user_id, bot_token, trust_level, identity_id) -> Result<Arc<dyn ChannelAdapter>>
    pub channel_adapter_factory: Option<Arc<dyn ChannelAdapterFactory>>,
    /// Correlation ID counter for request tracing
    correlation_counter: Arc<AtomicU64>,
    /// Server start time for uptime calculation
    pub started_at: std::time::Instant,
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
        model_router: Arc<ModelRouter>,
        safe_mode_guard: Arc<SafeModeGuard>,
        session_manager: Arc<SessionManager>,
        memory_manager: Arc<MemoryManager>,
        sub_agent_manager: Arc<SubAgentManager>,
        workflow_engine: Arc<crate::workflow::WorkflowEngine>,
        xp_manager: Arc<crate::xp::XpManager>,
        voice_gateway: Arc<crate::voice::VoiceGateway>,
        skill_book: Arc<crate::skill_book::SkillBook>,
    ) -> Self {
        Self {
            config,
            event_stream,
            policy_engine,
            ledger,
            worker_manager,
            scheduler,
            metrics: Arc::new(MetricsCollector::new()),
            model_router,
            safe_mode_guard,
            session_manager,
            memory_manager,
            sub_agent_manager,
            workflow_engine,
            xp_manager,
            voice_gateway,
            skill_book,
            channel_adapters: Arc::new(RwLock::new(HashMap::new())),
            channel_adapter_factory: None,
            correlation_counter: Arc::new(AtomicU64::new(0)),
            started_at: std::time::Instant::now(),
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
        let safe_mode_guard = {
            let pool = self
                .config
                .pool()
                .expect("Database pool required for SafeModeGuard");
            Arc::new(SafeModeGuard::new(pool.clone(), self.ledger.clone()))
        };

        let model_router = {
            let pool = self
                .config
                .pool()
                .expect("Database pool required for ModelRouter");
            Arc::new(
                ModelRouter::new(
                    pool.clone(),
                    self.config.gateway_url.clone(),
                    self.policy_engine.clone(),
                    self.ledger.clone(),
                )
                .with_event_stream(self.event_stream.clone())
                .with_safe_mode_guard(safe_mode_guard.clone()),
            )
        };

        // Wire safe mode guard into worker manager
        {
            let mut wm = self.worker_manager.lock().await;
            wm.set_safe_mode_guard(safe_mode_guard.clone());
        }

        // Create session manager with safe mode guard wired in
        let session_manager = {
            let pool = self
                .config
                .pool()
                .expect("Database pool required for SessionManager");
            Arc::new(
                SessionManager::with_defaults(pool.clone())
                    .with_safe_mode_guard(safe_mode_guard.clone()),
            )
        };

        // Create memory manager for agent knowledge persistence
        let memory_manager = {
            let pool = self
                .config
                .pool()
                .expect("Database pool required for MemoryManager");
            let chain_anchor = Arc::new(crate::chain_anchor::LocalDbChainAnchor::new(pool.clone()));
            Arc::new(
                MemoryManager::new(pool.clone(), Some(self.event_stream.clone()))
                    .with_chain_anchor(chain_anchor),
            )
        };

        // Create sub-agent lifecycle manager
        let sub_agent_manager = {
            let pool = self
                .config
                .pool()
                .expect("Database pool required for SubAgentManager");
            Arc::new(SubAgentManager::new(
                pool.clone(),
                Some(self.event_stream.clone()),
            ))
        };

        // Create workflow execution engine
        let workflow_engine = {
            let pool = self
                .config
                .pool()
                .expect("Database pool required for WorkflowEngine");
            Arc::new(crate::workflow::WorkflowEngine::new(
                pool.clone(),
                Some(self.event_stream.clone()),
            ))
        };

        // Create XP manager for experience point tracking
        let xp_manager = {
            let pool = self
                .config
                .pool()
                .expect("Database pool required for XpManager");
            Arc::new(crate::xp::XpManager::new(pool.clone()))
        };

        // Create voice gateway for ElevenLabs integration
        let voice_gateway = {
            let pool = self
                .config
                .pool()
                .expect("Database pool required for VoiceGateway");
            Arc::new(crate::voice::VoiceGateway::new(pool.clone()))
        };

        // Create Skill Book for curated skill catalog
        let skill_book = {
            let skill_book_path = std::path::PathBuf::from("skills/skill-book");
            let registry_path = self.config.skills_registry_path.clone();
            Arc::new(crate::skill_book::SkillBook::new(
                skill_book_path,
                registry_path,
                self.config.clone(),
            ))
        };

        let state = AppState::new(
            self.config.clone(),
            self.event_stream.clone(),
            self.policy_engine.clone(),
            self.ledger.clone(),
            self.worker_manager.clone(),
            self.scheduler.clone(),
            model_router,
            safe_mode_guard,
            session_manager,
            memory_manager,
            sub_agent_manager,
            workflow_engine,
            xp_manager,
            voice_gateway,
            skill_book,
        );

        // Share the metrics collector and workflow engine with the scheduler
        {
            let mut scheduler = self.scheduler.lock().await;
            scheduler.set_metrics(state.metrics.clone());
            scheduler.set_workflow_engine(state.workflow_engine.clone());
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
        .route("/v1/health/detailed", get(detailed_health_handler))
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
        // Safe mode endpoints
        .route("/v1/safe-mode/status", get(safe_mode_status_handler))
        .route("/v1/safe-mode/enable", post(enable_safe_mode_handler))
        .route("/v1/safe-mode/disable", post(disable_safe_mode_handler))
        // Metrics endpoint
        .route("/v1/metrics", get(metrics_handler))
        // Approval endpoints
        .route("/v1/approvals", get(list_approvals_handler))
        .route("/v1/approvals/{id}/approve", post(approve_approval_handler))
        .route("/v1/approvals/{id}/deny", post(deny_approval_handler))
        .route("/v1/approvals/batch", post(batch_approve_handler))
        // Capability endpoints
        .route(
            "/v1/capabilities",
            get(list_capabilities_handler).post(grant_capability_handler),
        )
        .route(
            "/v1/capabilities/{id}",
            axum::routing::delete(revoke_capability_handler),
        )
        // Memory endpoints
        .route(
            "/v1/memories",
            post(create_memory_handler).get(list_memories_handler),
        )
        .route("/v1/memories/{memory_id}", get(get_memory_handler))
        .route("/v1/memories/export", post(export_memories_handler))
        .route("/v1/memories/import", post(import_memories_handler))
        // Heartbeat endpoints
        .route("/v1/heartbeats", get(list_heartbeats_handler))
        .route("/v1/heartbeats/status", get(heartbeat_status_handler))
        // Identity endpoints
        .route("/v1/identity", get(get_identity_handler))
        .route("/v1/identity/soul", get(get_soul_content_handler))
        // Provider endpoints
        .route("/v1/providers", get(list_providers_handler))
        .route("/v1/providers/ollama/status", get(ollama_status_handler))
        // Sub-agent endpoints
        .route(
            "/v1/sub-agents",
            post(create_sub_agent_handler).get(list_sub_agents_handler),
        )
        .route(
            "/v1/sub-agents/{id}",
            get(get_sub_agent_handler)
                .put(update_sub_agent_handler)
                .delete(delete_sub_agent_handler),
        )
        .route("/v1/sub-agents/{id}/pause", post(pause_sub_agent_handler))
        .route("/v1/sub-agents/{id}/resume", post(resume_sub_agent_handler))
        // Workflow endpoints
        .route(
            "/v1/workflows",
            post(create_workflow_handler).get(list_workflows_handler),
        )
        .route(
            "/v1/workflows/{id}",
            get(get_workflow_handler)
                .put(update_workflow_handler)
                .delete(delete_workflow_handler),
        )
        .route("/v1/workflows/{id}/execute", post(execute_workflow_handler))
        // Channel adapter endpoints
        .route(
            "/v1/channels",
            get(list_channels_handler).post(create_channel_handler),
        )
        .route(
            "/v1/channels/{id}",
            get(get_channel_handler)
                .put(update_channel_handler)
                .delete(delete_channel_handler),
        )
        .route("/v1/channels/{id}/pair", post(pair_channel_handler))
        // Gateway usage ingestion
        .route("/api/usage", post(ingest_usage_handler))
        // XP endpoints
        .route("/v1/xp/agents/{id}", get(get_agent_xp_handler))
        .route("/v1/xp/agents/{id}/history", get(get_xp_history_handler))
        .route("/v1/xp/leaderboard", get(get_xp_leaderboard_handler))
        .route("/v1/xp/skills/top", get(get_top_skills_handler))
        .route("/v1/xp/skills/{id}", get(get_skill_metrics_handler))
        .route("/v1/xp/award", post(award_xp_handler))
        // Voice endpoints
        .route("/v1/voice/configure", post(configure_voice_handler))
        .route("/v1/voice/test", post(test_voice_handler))
        .route("/v1/voice/voices", get(list_voices_handler))
        .route("/v1/voice/transcribe", post(transcribe_voice_handler))
        // Ledger anchor endpoints
        .route("/v1/ledger/anchor", post(publish_ledger_anchor_handler))
        .route("/v1/ledger/anchor/{id}", get(get_ledger_anchor_handler))
        .route(
            "/v1/ledger/anchor/{id}/verify",
            get(verify_ledger_anchor_handler),
        )
        // Ledger viewer endpoints
        .route("/v1/ledger/events", get(list_ledger_events_handler))
        .route("/v1/ledger/verify", get(verify_ledger_chain_handler))
        // Setup status endpoints
        .route("/v1/config/setup-status", get(get_setup_status_handler))
        .route(
            "/v1/config/setup-complete",
            post(post_setup_complete_handler),
        )
        // Skill Book endpoints
        .route("/v1/skill-book", get(list_skill_book_handler))
        .route("/v1/skill-book/:id", get(get_skill_book_entry_handler))
        .route("/v1/skill-book/:id/activate", post(activate_skill_handler))
        .route(
            "/v1/skill-book/:id/deactivate",
            delete(deactivate_skill_handler),
        )
        // API Key endpoint (localhost-only via middleware)
        .route("/v1/config/api-key", get(get_api_key_handler))
        // Revocation sync endpoint
        .route(
            "/v1/memory/revoked-grants",
            get(list_revoked_grants_handler),
        )
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

/// Detailed health check endpoint handler.
///
/// Returns extended health diagnostics including uptime, last heartbeat,
/// scheduler state, worker manager state, and event stream subscriber count.
async fn detailed_health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let db_healthy = state.check_database_health().await;
    let uptime_seconds = state.started_at.elapsed().as_secs();

    // Query last heartbeat timestamp from database
    let last_heartbeat_at: Option<chrono::DateTime<chrono::Utc>> =
        if let Ok(pool) = state.config.pool() {
            sqlx::query_scalar::<_, Option<chrono::DateTime<chrono::Utc>>>(
                "SELECT created_at FROM heartbeat_history ORDER BY created_at DESC LIMIT 1",
            )
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .flatten()
        } else {
            None
        };

    // Check scheduler running state
    let scheduler_running = {
        let scheduler = state.scheduler.lock().await;
        scheduler.is_running()
    };

    // Check worker manager active state
    let worker_manager_active = {
        let wm = state.worker_manager.lock().await;
        !wm.get_worker_status().await.is_empty()
    };

    let subscriber_count = state.event_stream.subscriber_count();

    let response = DetailedHealthResponse {
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
        uptime_seconds,
        last_heartbeat_at,
        scheduler_running,
        worker_manager_active,
        event_stream_subscriber_count: subscriber_count,
    };

    tracing::debug!(
        uptime_seconds = uptime_seconds,
        scheduler_running = scheduler_running,
        subscriber_count = subscriber_count,
        "Detailed health check completed"
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

    // Compute real queue depth from database (pending + running tasks)
    let queue_depth: u32 = if let Ok(pool) = state.config.pool() {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM tasks WHERE state IN ('pending', 'running')",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0)
        .try_into()
        .unwrap_or(0)
    } else {
        0 // TODO: return a meaningful estimate if pool is unavailable
    };

    // Look up the core identity
    let identity_id: Option<uuid::Uuid> = if let Ok(pool) = state.config.pool() {
        sqlx::query_scalar(
            r"SELECT identity_id FROM identities WHERE identity_type = 'core' LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let version = carnelian_common::VERSION.to_string();
    let machine_profile = format!("{:?}", state.config.machine_profile);
    let uptime_seconds = Some(state.started_at.elapsed().as_secs());

    let response = StatusResponse {
        workers,
        models: vec![],
        queue_depth,
        identity_id,
        version,
        machine_profile,
        uptime_seconds,
    };

    tracing::debug!(
        workers = response.workers.len(),
        models = response.models.len(),
        queue_depth = response.queue_depth,
        ?response.identity_id,
        version = %response.version,
        machine_profile = %response.machine_profile,
        uptime_seconds = ?response.uptime_seconds,
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

    // message is BYTEA after migration 0009; sensitive flags encrypted rows
    let rows: Vec<(
        i64,
        Uuid,
        chrono::DateTime<chrono::Utc>,
        String,
        Vec<u8>,
        Option<serde_json::Value>,
        bool,
        bool,
    )> = sqlx::query_as(
        r"SELECT log_id, run_id, ts, level, message, fields, truncated, sensitive
          FROM run_logs WHERE run_id = $1 ORDER BY ts ASC LIMIT $2 OFFSET $3",
    )
    .bind(run_id)
    .bind(i64::from(page_size))
    .bind(i64::from(offset))
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Build an EncryptionHelper if the owner signing key is available (for decrypting sensitive logs)
    let encryption_helper = state
        .config
        .owner_signing_key()
        .map(|sk| crate::encryption::EncryptionHelper::new(pool, sk));

    let mut logs: Vec<RunLogEntry> = Vec::with_capacity(rows.len());
    for (log_id, run_id, ts, level, message_bytes, fields, truncated, sensitive) in rows {
        let message = if sensitive {
            // Attempt decryption of sensitive message
            if let Some(ref helper) = encryption_helper {
                helper
                    .decrypt_text(&message_bytes)
                    .await
                    .unwrap_or_else(|_| "[encrypted — decryption failed]".to_string())
            } else {
                "[encrypted — no signing key available]".to_string()
            }
        } else {
            // Non-sensitive: raw UTF-8 bytes
            String::from_utf8(message_bytes).unwrap_or_else(|_| "[invalid UTF-8]".to_string())
        };
        logs.push(RunLogEntry {
            log_id,
            run_id,
            ts,
            level,
            message,
            fields,
            truncated,
            sensitive,
        });
    }

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
// RUN LOG HELPERS
// =============================================================================

/// Insert a run log entry, optionally encrypting the message if `sensitive` is true.
///
/// When `sensitive=true` and an `EncryptionHelper` is provided, the message is
/// encrypted before storage. Non-sensitive messages are stored as raw UTF-8 bytes
/// (the `message` column is BYTEA after migration 0009).
pub async fn insert_run_log(
    pool: &sqlx::PgPool,
    run_id: Uuid,
    level: &str,
    message: &str,
    sensitive: bool,
    encryption_helper: Option<&crate::encryption::EncryptionHelper>,
) -> carnelian_common::Result<()> {
    let message_bytes: Vec<u8> = if sensitive {
        if let Some(helper) = encryption_helper {
            helper.encrypt_text(message).await?
        } else {
            message.as_bytes().to_vec()
        }
    } else {
        message.as_bytes().to_vec()
    };

    sqlx::query("INSERT INTO run_logs (run_id, level, message, sensitive) VALUES ($1, $2, $3, $4)")
        .bind(run_id)
        .bind(level)
        .bind(&message_bytes)
        .bind(sensitive)
        .execute(pool)
        .await
        .map_err(carnelian_common::Error::Database)?;

    Ok(())
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
// SAFE MODE HANDLERS
// =============================================================================

/// Request body for enable/disable safe mode endpoints.
#[derive(Debug, Deserialize)]
struct SafeModeToggleRequest {
    /// Optional actor UUID performing the toggle.
    #[serde(default)]
    actor_id: Option<Uuid>,
}

/// GET `/v1/safe-mode/status` — query current safe mode state.
async fn safe_mode_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.safe_mode_guard.is_enabled().await {
        Ok(enabled) => (StatusCode::OK, Json(json!({"safe_mode": enabled}))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to query safe mode status");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to query safe mode: {}", e)})),
            )
                .into_response()
        }
    }
}

/// POST `/v1/safe-mode/enable` — enable safe mode.
async fn enable_safe_mode_handler(
    State(state): State<AppState>,
    Json(body): Json<SafeModeToggleRequest>,
) -> impl IntoResponse {
    let signing_key = state.config.owner_signing_key();
    match state
        .safe_mode_guard
        .enable(body.actor_id, signing_key)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({"safe_mode": true, "message": "Safe mode enabled"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to enable safe mode");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to enable safe mode: {}", e)})),
            )
                .into_response()
        }
    }
}

/// POST `/v1/safe-mode/disable` — disable safe mode.
async fn disable_safe_mode_handler(
    State(state): State<AppState>,
    Json(body): Json<SafeModeToggleRequest>,
) -> impl IntoResponse {
    let signing_key = state.config.owner_signing_key();
    match state
        .safe_mode_guard
        .disable(body.actor_id, signing_key)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({"safe_mode": false, "message": "Safe mode disabled"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to disable safe mode");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to disable safe mode: {}", e)})),
            )
                .into_response()
        }
    }
}

// =============================================================================
// USAGE INGESTION HANDLER
// =============================================================================

/// A single usage record sent by the gateway.
#[derive(Debug, Deserialize)]
struct UsageRecord {
    /// Provider name (must match `model_providers.name`).
    provider: String,
    /// ISO-8601 timestamp.
    #[serde(default)]
    timestamp: Option<String>,
    /// Model used (deserialized for logging; not stored in `usage_costs`).
    #[serde(default)]
    #[allow(dead_code)]
    model: String,
    /// Prompt / input tokens.
    #[serde(default)]
    tokens_in: i32,
    /// Completion / output tokens.
    #[serde(default)]
    tokens_out: i32,
    /// Estimated cost in USD.
    #[serde(default)]
    estimated_cost: f64,
    /// Optional correlation ID from the originating request.
    #[serde(default)]
    correlation_id: Option<String>,
}

/// Request body for `POST /api/usage`.
#[derive(Debug, Deserialize)]
struct IngestUsageRequest {
    records: Vec<UsageRecord>,
}

/// Ingest usage records from the gateway via `POST /api/usage`.
///
/// Resolves each record's `provider` name to a `provider_id` in `model_providers`,
/// then inserts a row into `usage_costs`. Unknown providers are skipped with a warning.
async fn ingest_usage_handler(
    State(state): State<AppState>,
    Json(body): Json<IngestUsageRequest>,
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

    if body.records.is_empty() {
        return (StatusCode::OK, Json(json!({"inserted": 0}))).into_response();
    }

    let mut inserted: u64 = 0;
    let mut skipped: u64 = 0;

    for record in &body.records {
        // Resolve provider name → provider_id
        let provider_id: Option<Uuid> =
            sqlx::query_scalar("SELECT provider_id FROM model_providers WHERE name = $1 LIMIT 1")
                .bind(&record.provider)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();

        let Some(provider_id) = provider_id else {
            tracing::warn!(
                provider = %record.provider,
                "Usage record skipped: unknown provider"
            );
            skipped += 1;
            continue;
        };

        // Parse the optional correlation_id as UUID
        let correlation_id: Option<Uuid> = record
            .correlation_id
            .as_deref()
            .and_then(|s| s.parse().ok());

        // Parse timestamp or default to now (handled by DB default)
        let ts: Option<chrono::DateTime<chrono::Utc>> =
            record.timestamp.as_deref().and_then(|s| s.parse().ok());

        let result = if let Some(ts) = ts {
            sqlx::query(
                r"INSERT INTO usage_costs (provider_id, ts, tokens_in, tokens_out, cost_estimate, correlation_id)
                  VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(provider_id)
            .bind(ts)
            .bind(record.tokens_in)
            .bind(record.tokens_out)
            .bind(record.estimated_cost)
            .bind(correlation_id)
            .execute(pool)
            .await
        } else {
            sqlx::query(
                r"INSERT INTO usage_costs (provider_id, tokens_in, tokens_out, cost_estimate, correlation_id)
                  VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(provider_id)
            .bind(record.tokens_in)
            .bind(record.tokens_out)
            .bind(record.estimated_cost)
            .bind(correlation_id)
            .execute(pool)
            .await
        };

        match result {
            Ok(_) => inserted += 1,
            Err(e) => {
                tracing::warn!(
                    provider = %record.provider,
                    error = %e,
                    "Failed to insert usage record"
                );
                skipped += 1;
            }
        }
    }

    tracing::debug!(
        inserted = inserted,
        skipped = skipped,
        total = body.records.len(),
        "Usage ingestion complete"
    );

    (
        StatusCode::OK,
        Json(json!({
            "inserted": inserted,
            "skipped": skipped,
        })),
    )
        .into_response()
}

// =============================================================================
// APPROVAL HELPERS
// =============================================================================

/// After an approval request has been marked as approved, execute the underlying
/// action (e.g. capability grant or revoke) via the `PolicyEngine`.
///
/// For action types that are not capability-related this is a no-op.
async fn execute_approved_action(
    approval_id: Uuid,
    approval_queue: &crate::approvals::ApprovalQueue,
    policy_engine: &crate::PolicyEngine,
    event_stream: &crate::EventStream,
    ledger: &crate::Ledger,
    signing_key: Option<&ed25519_dalek::SigningKey>,
) -> carnelian_common::Result<()> {
    let request = match approval_queue.get(approval_id).await? {
        Some(r) => r,
        None => {
            return Err(carnelian_common::Error::Security(format!(
                "Approval request not found: {}",
                approval_id
            )));
        }
    };

    match request.action_type.as_str() {
        "capability.grant" => {
            policy_engine
                .execute_approved_grant(
                    approval_id,
                    approval_queue,
                    Some(event_stream),
                    Some(ledger),
                    signing_key,
                )
                .await?;
        }
        "capability.revoke" => {
            policy_engine
                .execute_approved_revoke(
                    approval_id,
                    approval_queue,
                    Some(event_stream),
                    Some(ledger),
                    signing_key,
                )
                .await?;
        }
        _ => {
            // Non-capability action types (config.change, db.migration, etc.)
            // are approved but have no automatic execution path yet.
            tracing::debug!(
                approval_id = %approval_id,
                action_type = %request.action_type,
                "Approved action has no automatic execution path"
            );
        }
    }

    Ok(())
}

// =============================================================================
// APPROVAL HANDLERS
// =============================================================================

/// Query parameters for listing approvals.
#[derive(Debug, Deserialize)]
struct ListApprovalsQuery {
    #[serde(default = "default_approval_limit")]
    limit: i64,
    #[serde(default)]
    action_type: Option<String>,
}

const fn default_approval_limit() -> i64 {
    100
}

/// List pending approvals via `GET /v1/approvals`.
async fn list_approvals_handler(
    State(state): State<AppState>,
    Query(params): Query<ListApprovalsQuery>,
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

    let approval_queue = crate::approvals::ApprovalQueue::new(pool.clone());
    match approval_queue.list_pending(params.limit).await {
        Ok(requests) => {
            let approvals: Vec<carnelian_common::types::ApprovalRequestDetail> = requests
                .into_iter()
                .filter(|r| {
                    params
                        .action_type
                        .as_ref()
                        .is_none_or(|at| r.action_type == *at)
                })
                .map(|r| carnelian_common::types::ApprovalRequestDetail {
                    id: r.id,
                    action_type: r.action_type,
                    payload: r.payload,
                    status: r.status,
                    requested_by: r.requested_by,
                    requested_at: r.requested_at,
                    resolved_at: r.resolved_at,
                    resolved_by: r.resolved_by,
                    correlation_id: r.correlation_id,
                })
                .collect();

            (
                StatusCode::OK,
                Json(
                    serde_json::to_value(carnelian_common::types::ListApprovalsResponse {
                        approvals,
                    })
                    .unwrap_or_default(),
                ),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{}", e)})),
        )
            .into_response(),
    }
}

/// Approve a pending action via `POST /v1/approvals/:id/approve`.
async fn approve_approval_handler(
    State(state): State<AppState>,
    Path(approval_id): Path<Uuid>,
    Json(body): Json<carnelian_common::types::ApprovalActionRequest>,
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

    // Load owner signing key for cryptographic approval
    let signing_key = match state.config.owner_signing_key() {
        Some(sk) => sk,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "owner signing key not configured"})),
            )
                .into_response();
        }
    };

    // Validate client-provided signature against owner public key
    if body.signature.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "signature is required"})),
        )
            .into_response();
    }
    let public_key_hex = crate::crypto::public_key_from_signing_key(signing_key);
    match crate::crypto::verify_signature(
        &public_key_hex,
        approval_id.to_string().as_bytes(),
        &body.signature,
    ) {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "invalid signature"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("signature verification failed: {}", e)})),
            )
                .into_response();
        }
    }

    // Use system UUID as approver (future: extract from auth context)
    let approved_by = Uuid::nil();

    let approval_queue = crate::approvals::ApprovalQueue::new(pool.clone());
    match approval_queue
        .approve(approval_id, approved_by, signing_key, &state.ledger)
        .await
    {
        Ok(()) => {
            // Execute the underlying action based on the approval's action_type
            if let Err(e) = execute_approved_action(
                approval_id,
                &approval_queue,
                &state.policy_engine,
                &state.event_stream,
                &state.ledger,
                Some(signing_key),
            )
            .await
            {
                tracing::error!(
                    approval_id = %approval_id,
                    error = %e,
                    "Failed to execute approved action"
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Approved but execution failed: {}", e)})),
                )
                    .into_response();
            }

            // Publish WebSocket event only on successful execution
            state.event_stream.publish(EventEnvelope::new(
                EventLevel::Info,
                EventType::ApprovalApproved,
                json!({ "approval_id": approval_id }),
            ));

            (
                StatusCode::OK,
                Json(
                    serde_json::to_value(carnelian_common::types::ApprovalActionResponse {
                        approval_id,
                        status: "approved".to_string(),
                    })
                    .unwrap_or_default(),
                ),
            )
                .into_response()
        }
        Err(e) => {
            let status = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else if e.to_string().contains("already") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, Json(json!({"error": format!("{}", e)}))).into_response()
        }
    }
}

/// Deny a pending action via `POST /v1/approvals/:id/deny`.
async fn deny_approval_handler(
    State(state): State<AppState>,
    Path(approval_id): Path<Uuid>,
    Json(body): Json<carnelian_common::types::ApprovalActionRequest>,
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

    let signing_key = match state.config.owner_signing_key() {
        Some(sk) => sk,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "owner signing key not configured"})),
            )
                .into_response();
        }
    };

    // Validate client-provided signature against owner public key
    if body.signature.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "signature is required"})),
        )
            .into_response();
    }
    let public_key_hex = crate::crypto::public_key_from_signing_key(signing_key);
    match crate::crypto::verify_signature(
        &public_key_hex,
        approval_id.to_string().as_bytes(),
        &body.signature,
    ) {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "invalid signature"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("signature verification failed: {}", e)})),
            )
                .into_response();
        }
    }

    let denied_by = Uuid::nil();

    let approval_queue = crate::approvals::ApprovalQueue::new(pool.clone());
    match approval_queue
        .deny(approval_id, denied_by, signing_key, &state.ledger)
        .await
    {
        Ok(()) => {
            state.event_stream.publish(EventEnvelope::new(
                EventLevel::Info,
                EventType::ApprovalDenied,
                json!({ "approval_id": approval_id }),
            ));

            (
                StatusCode::OK,
                Json(
                    serde_json::to_value(carnelian_common::types::ApprovalActionResponse {
                        approval_id,
                        status: "denied".to_string(),
                    })
                    .unwrap_or_default(),
                ),
            )
                .into_response()
        }
        Err(e) => {
            let status = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else if e.to_string().contains("already") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, Json(json!({"error": format!("{}", e)}))).into_response()
        }
    }
}

/// Batch approve pending actions via `POST /v1/approvals/batch`.
async fn batch_approve_handler(
    State(state): State<AppState>,
    Json(body): Json<carnelian_common::types::BatchApprovalRequest>,
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

    let signing_key = match state.config.owner_signing_key() {
        Some(sk) => sk,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "owner signing key not configured"})),
            )
                .into_response();
        }
    };

    // Validate client-provided signature against owner public key.
    // For batch, the signature is verified against the concatenated sorted IDs.
    if body.signature.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "signature is required"})),
        )
            .into_response();
    }
    let public_key_hex = crate::crypto::public_key_from_signing_key(signing_key);
    {
        let mut sorted_ids = body.approval_ids.clone();
        sorted_ids.sort();
        let message = sorted_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        match crate::crypto::verify_signature(&public_key_hex, message.as_bytes(), &body.signature)
        {
            Ok(true) => {}
            Ok(false) => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "invalid signature"})),
                )
                    .into_response();
            }
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": format!("signature verification failed: {}", e)})),
                )
                    .into_response();
            }
        }
    }

    let approved_by = Uuid::nil();
    let all_ids = body.approval_ids.clone();

    let approval_queue = crate::approvals::ApprovalQueue::new(pool.clone());
    match approval_queue
        .batch_approve(body.approval_ids, approved_by, signing_key, &state.ledger)
        .await
    {
        Ok(approved) => {
            let mut executed: Vec<Uuid> = Vec::new();
            let mut failed: Vec<Uuid> = all_ids
                .iter()
                .filter(|id| !approved.contains(id))
                .copied()
                .collect();

            // Execute the underlying action for each approved item
            for id in &approved {
                match execute_approved_action(
                    *id,
                    &approval_queue,
                    &state.policy_engine,
                    &state.event_stream,
                    &state.ledger,
                    Some(signing_key),
                )
                .await
                {
                    Ok(()) => {
                        executed.push(*id);
                        // Publish event only on successful execution
                        state.event_stream.publish(EventEnvelope::new(
                            EventLevel::Info,
                            EventType::ApprovalApproved,
                            json!({ "approval_id": id }),
                        ));
                    }
                    Err(e) => {
                        tracing::error!(
                            approval_id = %id,
                            error = %e,
                            "Failed to execute approved action in batch"
                        );
                        failed.push(*id);
                    }
                }
            }

            (
                StatusCode::OK,
                Json(
                    serde_json::to_value(carnelian_common::types::BatchApprovalResponse {
                        approved: executed,
                        failed,
                    })
                    .unwrap_or_default(),
                ),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{}", e)})),
        )
            .into_response(),
    }
}

// =============================================================================
// CAPABILITY HANDLERS
// =============================================================================

/// Query parameters for listing capabilities.
#[derive(Debug, Deserialize)]
struct ListCapabilitiesQuery {
    #[serde(default)]
    subject_type: Option<String>,
    #[serde(default)]
    subject_id: Option<String>,
}

/// List capability grants via `GET /v1/capabilities`.
async fn list_capabilities_handler(
    State(state): State<AppState>,
    Query(params): Query<ListCapabilitiesQuery>,
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

    // If both subject_type and subject_id are provided, use filtered query
    let grants = if let (Some(st), Some(si)) = (&params.subject_type, &params.subject_id) {
        state.policy_engine.list_grants_for_subject(st, si).await
    } else {
        // List all grants (with optional subject_type filter)

        if let Some(ref st) = params.subject_type {
            sqlx::query_as::<_, crate::policy::CapabilityGrant>(
                r"SELECT grant_id, subject_type, subject_id, capability_key, scope, constraints,
                         approved_by, created_at, expires_at
                  FROM capability_grants
                  WHERE subject_type = $1 AND (expires_at IS NULL OR expires_at > NOW())
                  ORDER BY created_at DESC LIMIT 200",
            )
            .bind(st)
            .fetch_all(pool)
            .await
            .map_err(carnelian_common::Error::Database)
        } else {
            sqlx::query_as::<_, crate::policy::CapabilityGrant>(
                r"SELECT grant_id, subject_type, subject_id, capability_key, scope, constraints,
                         approved_by, created_at, expires_at
                  FROM capability_grants
                  WHERE (expires_at IS NULL OR expires_at > NOW())
                  ORDER BY created_at DESC LIMIT 200",
            )
            .fetch_all(pool)
            .await
            .map_err(carnelian_common::Error::Database)
        }
    };

    match grants {
        Ok(rows) => {
            let details: Vec<carnelian_common::types::CapabilityGrantDetail> = rows
                .into_iter()
                .map(|g| carnelian_common::types::CapabilityGrantDetail {
                    grant_id: g.grant_id,
                    subject_type: g.subject_type,
                    subject_id: g.subject_id,
                    capability_key: g.capability_key,
                    scope: g.scope,
                    constraints: g.constraints,
                    approved_by: g.approved_by,
                    created_at: g.created_at,
                    expires_at: g.expires_at,
                })
                .collect();

            (
                StatusCode::OK,
                Json(
                    serde_json::to_value(carnelian_common::types::ListCapabilitiesResponse {
                        grants: details,
                    })
                    .unwrap_or_default(),
                ),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{}", e)})),
        )
            .into_response(),
    }
}

/// Grant a capability via `POST /v1/capabilities`.
///
/// If the action requires approval, returns 202 Accepted with the approval_id.
async fn grant_capability_handler(
    State(state): State<AppState>,
    Json(body): Json<carnelian_common::types::GrantCapabilityRequest>,
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

    let signing_key = state.config.owner_signing_key();
    let approval_queue = crate::approvals::ApprovalQueue::new(pool.clone());

    match state
        .policy_engine
        .grant_capability(
            &body.subject_type,
            &body.subject_id,
            &body.capability_key,
            body.scope,
            body.constraints,
            None, // approved_by
            body.expires_at,
            Some(&state.event_stream),
            Some(&state.ledger),
            signing_key,
            Some(&approval_queue),
        )
        .await
    {
        Ok(grant_id) => (
            StatusCode::CREATED,
            Json(
                serde_json::to_value(carnelian_common::types::GrantCapabilityResponse { grant_id })
                    .unwrap_or_default(),
            ),
        )
            .into_response(),
        Err(carnelian_common::Error::ApprovalRequired(approval_id)) => {
            // Publish queued event
            state.event_stream.publish(EventEnvelope::new(
                EventLevel::Info,
                EventType::ApprovalQueued,
                json!({
                    "approval_id": approval_id,
                    "action_type": "capability.grant",
                }),
            ));

            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "approval_id": approval_id,
                    "message": "Capability grant queued for approval"
                })),
            )
                .into_response()
        }
        Err(e) => {
            let status = if e.to_string().contains("Invalid capability key") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, Json(json!({"error": format!("{}", e)}))).into_response()
        }
    }
}

/// Revoke a capability via `DELETE /v1/capabilities/:id`.
async fn revoke_capability_handler(
    State(state): State<AppState>,
    Path(grant_id): Path<Uuid>,
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

    let signing_key = state.config.owner_signing_key();
    let approval_queue = crate::approvals::ApprovalQueue::new(pool.clone());

    match state
        .policy_engine
        .revoke_capability(
            grant_id,
            None, // revoked_by
            Some(&state.event_stream),
            Some(&state.ledger),
            signing_key,
            Some(&approval_queue),
        )
        .await
    {
        Ok(revoked) => (
            StatusCode::OK,
            Json(
                serde_json::to_value(carnelian_common::types::RevokeCapabilityResponse { revoked })
                    .unwrap_or_default(),
            ),
        )
            .into_response(),
        Err(carnelian_common::Error::ApprovalRequired(approval_id)) => {
            state.event_stream.publish(EventEnvelope::new(
                EventLevel::Info,
                EventType::ApprovalQueued,
                json!({
                    "approval_id": approval_id,
                    "action_type": "capability.revoke",
                }),
            ));

            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "approval_id": approval_id,
                    "message": "Capability revocation queued for approval"
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{}", e)})),
        )
            .into_response(),
    }
}

// =============================================================================
// MEMORY HANDLERS
// =============================================================================

/// Query parameters for `GET /v1/memories`.
#[derive(Debug, Deserialize)]
struct ListMemoriesQuery {
    identity_id: Option<Uuid>,
    source: Option<String>,
    min_importance: Option<f32>,
    #[serde(default = "default_memory_limit")]
    limit: i64,
}

fn default_memory_limit() -> i64 {
    50
}

/// Create a new memory via `POST /v1/memories`.
async fn create_memory_handler(
    State(state): State<AppState>,
    Json(body): Json<CreateMemoryRequest>,
) -> impl IntoResponse {
    // Validate source
    let source = match MemorySource::from_str(&body.source) {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid source: must be one of conversation, task, observation, reflection"})),
            )
                .into_response();
        }
    };

    // Validate importance range
    if !(0.0..=1.0).contains(&body.importance) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Importance must be between 0.0 and 1.0"})),
        )
            .into_response();
    }

    match state
        .memory_manager
        .create_memory(
            body.identity_id,
            &body.content,
            body.summary,
            source,
            None, // embedding not provided via REST
            body.importance,
            body.tags,
        )
        .await
    {
        Ok(memory) => (
            StatusCode::CREATED,
            Json(json!(CreateMemoryResponse {
                memory_id: memory.memory_id,
                created_at: memory.created_at,
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

/// List memories with optional filtering via `GET /v1/memories`.
async fn list_memories_handler(
    State(state): State<AppState>,
    Query(params): Query<ListMemoriesQuery>,
) -> impl IntoResponse {
    let limit = params.limit.clamp(1, 200);
    let mut query = MemoryQuery::new().with_limit(limit);

    if let Some(id) = params.identity_id {
        query = query.with_identity(id);
    }

    if let Some(ref source_str) = params.source {
        match MemorySource::from_str(source_str) {
            Ok(s) => {
                query = query.with_sources(vec![s]);
            }
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Invalid source: must be one of conversation, task, observation, reflection"})),
                )
                    .into_response();
            }
        }
    }

    if let Some(min_imp) = params.min_importance {
        query = query.with_min_importance(min_imp);
    }

    match state.memory_manager.query_memories(query).await {
        Ok(memories) => {
            let details: Vec<MemoryDetail> = memories
                .into_iter()
                .map(|m| MemoryDetail {
                    memory_id: m.memory_id,
                    identity_id: m.identity_id,
                    content: m.content,
                    summary: m.summary,
                    source: m.source,
                    importance: m.importance,
                    tags: m.tags,
                    created_at: m.created_at,
                    accessed_at: m.accessed_at,
                    access_count: m.access_count,
                })
                .collect();
            (
                StatusCode::OK,
                Json(json!(ListMemoriesResponse { memories: details })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

/// Retrieve a single memory via `GET /v1/memories/{memory_id}`.
async fn get_memory_handler(
    State(state): State<AppState>,
    Path(memory_id): Path<Uuid>,
) -> impl IntoResponse {
    match state.memory_manager.get_memory(memory_id).await {
        Ok(Some(memory)) => {
            let detail = MemoryDetail {
                memory_id: memory.memory_id,
                identity_id: memory.identity_id,
                content: memory.content,
                summary: memory.summary,
                source: memory.source,
                importance: memory.importance,
                tags: memory.tags,
                created_at: memory.created_at,
                accessed_at: memory.accessed_at,
                access_count: memory.access_count,
            };
            (
                StatusCode::OK,
                Json(json!(GetMemoryResponse { memory: detail })),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Memory not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

// =============================================================================
// MEMORY PORTABILITY HANDLERS
// =============================================================================

/// Export memories as a signed, encrypted CBOR envelope via `POST /v1/memories/export`.
async fn export_memories_handler(
    State(state): State<AppState>,
    Json(body): Json<ExportMemoryRequest>,
) -> impl IntoResponse {
    use crate::memory::MemoryExportOptions;
    use base64::Engine;

    if body.memory_ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "memory_ids must not be empty"})),
        )
            .into_response();
    }

    let options = MemoryExportOptions {
        include_embedding: body.include_embedding,
        topic_filter: body.topic_filter,
        min_importance: body.min_importance,
        include_ledger_proof: body.include_ledger_proof,
        include_capabilities: body.include_capabilities,
    };

    let signing_key = if body.sign_export {
        state.config.owner_signing_key()
    } else {
        None
    };

    let memory_count = body.memory_ids.len();

    match state
        .memory_manager
        .export_memories_batch(&body.memory_ids, &options, signing_key)
        .await
    {
        Ok(cbor_bytes) => {
            let envelope_base64 = base64::engine::general_purpose::STANDARD.encode(&cbor_bytes);
            (
                StatusCode::OK,
                Json(json!(ExportMemoryResponse {
                    envelope_base64,
                    memory_count,
                    signed: body.sign_export,
                    export_timestamp: chrono::Utc::now(),
                })),
            )
                .into_response()
        }
        Err(e) => {
            state
                .event_stream
                .publish(carnelian_common::types::EventEnvelope::new(
                    EventLevel::Error,
                    EventType::MemoryExportFailed,
                    json!({"error": format!("{e}")}),
                ));
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("{e}")})),
            )
                .into_response()
        }
    }
}

/// Import memories from a CBOR envelope via `POST /v1/memories/import`.
async fn import_memories_handler(
    State(state): State<AppState>,
    Json(body): Json<ImportMemoryRequest>,
) -> impl IntoResponse {
    use base64::Engine;

    let envelope_bytes =
        match base64::engine::general_purpose::STANDARD.decode(&body.envelope_base64) {
            Ok(b) => b,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": format!("Invalid base64: {e}")})),
                )
                    .into_response();
            }
        };

    let public_key = if body.verify_signature {
        match &body.public_key {
            Some(pk) => Some(pk.as_str()),
            None => state.config.owner_public_key.as_deref(),
        }
    } else {
        None
    };

    match state
        .memory_manager
        .import_memories_batch(
            &envelope_bytes,
            body.identity_id,
            body.verify_signature,
            public_key,
        )
        .await
    {
        Ok(results) => {
            let successful_count = results
                .iter()
                .filter(|r| r.memory_id != uuid::Uuid::nil())
                .count();
            let failed_count = results.len() - successful_count;
            let api_results: Vec<MemoryImportResultApi> = results
                .into_iter()
                .map(|r| MemoryImportResultApi {
                    memory_id: r.memory_id,
                    verified: r.verified,
                    ledger_proof_valid: r.ledger_proof_valid,
                    warnings: r.warnings,
                })
                .collect();
            (
                StatusCode::OK,
                Json(json!(ImportMemoryResponse {
                    results: api_results,
                    successful_count,
                    failed_count,
                })),
            )
                .into_response()
        }
        Err(e) => {
            state
                .event_stream
                .publish(carnelian_common::types::EventEnvelope::new(
                    EventLevel::Error,
                    EventType::MemoryImportFailed,
                    json!({"error": format!("{e}")}),
                ));
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("{e}")})),
            )
                .into_response()
        }
    }
}

// =============================================================================
// HEARTBEAT HANDLERS
// =============================================================================

/// Query parameters for `GET /v1/heartbeats`.
#[derive(Debug, Deserialize)]
struct ListHeartbeatsQuery {
    #[serde(default = "default_heartbeat_limit")]
    limit: i64,
}

fn default_heartbeat_limit() -> i64 {
    10
}

/// List recent heartbeat records via `GET /v1/heartbeats`.
async fn list_heartbeats_handler(
    State(state): State<AppState>,
    Query(params): Query<ListHeartbeatsQuery>,
) -> impl IntoResponse {
    let limit = params.limit.clamp(1, 100);
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("{e}")})),
            )
                .into_response();
        }
    };

    let rows: std::result::Result<
        Vec<(
            Uuid,
            Uuid,
            chrono::DateTime<chrono::Utc>,
            Option<String>,
            i32,
            String,
            Option<i32>,
        )>,
        _,
    > = sqlx::query_as(
        r"SELECT heartbeat_id, identity_id, ts, mantra, tasks_queued, status, duration_ms
          FROM heartbeat_history
          ORDER BY ts DESC
          LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await;

    match rows {
        Ok(rows) => {
            let records: Vec<HeartbeatRecord> = rows
                .into_iter()
                .map(
                    |(heartbeat_id, identity_id, ts, mantra, tasks_queued, status, duration_ms)| {
                        HeartbeatRecord {
                            heartbeat_id,
                            identity_id,
                            ts,
                            mantra,
                            tasks_queued,
                            status,
                            duration_ms,
                        }
                    },
                )
                .collect();
            (StatusCode::OK, Json(json!(records))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

/// Get current heartbeat status via `GET /v1/heartbeats/status`.
async fn heartbeat_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("{e}")})),
            )
                .into_response();
        }
    };

    let row: std::result::Result<Option<(Option<String>, chrono::DateTime<chrono::Utc>)>, _> =
        sqlx::query_as(r"SELECT mantra, ts FROM heartbeat_history ORDER BY ts DESC LIMIT 1")
            .fetch_optional(pool)
            .await;

    match row {
        Ok(Some((mantra, last_ts))) => {
            let interval_ms = state.config.heartbeat_interval_ms;
            let next_ts = last_ts + chrono::Duration::milliseconds(interval_ms as i64);
            (
                StatusCode::OK,
                Json(json!(HeartbeatStatusResponse {
                    current_mantra: mantra,
                    last_heartbeat_time: Some(last_ts),
                    next_heartbeat_time: Some(next_ts),
                    interval_ms,
                })),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::OK,
            Json(json!(HeartbeatStatusResponse {
                current_mantra: None,
                last_heartbeat_time: None,
                next_heartbeat_time: None,
                interval_ms: state.config.heartbeat_interval_ms,
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

// =============================================================================
// IDENTITY HANDLERS
// =============================================================================

/// Get core identity information via `GET /v1/identity`.
async fn get_identity_handler(State(state): State<AppState>) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("{e}")})),
            )
                .into_response();
        }
    };

    let row: std::result::Result<Option<(Uuid, String, Option<String>, String, Option<String>, serde_json::Value, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>, _> = sqlx::query_as(
        r"SELECT identity_id, name, pronouns, identity_type, soul_file_path, directives, created_at, updated_at
          FROM identities
          WHERE identity_type = 'core'
          LIMIT 1",
    )
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some((
            identity_id,
            name,
            pronouns,
            identity_type,
            soul_file_path,
            directives,
            created_at,
            updated_at,
        ))) => {
            let directive_count = directives.as_array().map_or(0, |a| a.len());
            (
                StatusCode::OK,
                Json(json!(IdentityResponse {
                    identity_id,
                    name,
                    pronouns,
                    identity_type,
                    soul_file_path,
                    directive_count,
                    created_at,
                    updated_at,
                })),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Core identity not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

/// Get SOUL.md content via `GET /v1/identity/soul`.
async fn get_soul_content_handler(State(state): State<AppState>) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("{e}")})),
            )
                .into_response();
        }
    };

    // Get core identity's soul_file_path
    let soul_path: std::result::Result<Option<Option<String>>, _> = sqlx::query_scalar(
        r"SELECT soul_file_path FROM identities WHERE identity_type = 'core' LIMIT 1",
    )
    .fetch_optional(pool)
    .await;

    let rel_path = match soul_path {
        Ok(Some(Some(p))) => p,
        Ok(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Core identity or soul file path not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("{e}")})),
            )
                .into_response();
        }
    };

    let full_path = state.config.souls_path.join(&rel_path);
    match tokio::fs::read_to_string(&full_path).await {
        Ok(content) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            content,
        )
            .into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Failed to read soul file: {e}")})),
        )
            .into_response(),
    }
}

// =============================================================================
// PROVIDER HANDLERS
// =============================================================================

/// List all model providers via `GET /v1/providers`.
async fn list_providers_handler(State(state): State<AppState>) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("{e}")})),
            )
                .into_response();
        }
    };

    let rows: std::result::Result<
        Vec<(
            Uuid,
            String,
            String,
            bool,
            serde_json::Value,
            chrono::DateTime<chrono::Utc>,
        )>,
        _,
    > = sqlx::query_as(
        r"SELECT provider_id, provider_type, name, enabled, config, created_at
          FROM model_providers
          ORDER BY provider_type ASC, name ASC",
    )
    .fetch_all(pool)
    .await;

    match rows {
        Ok(rows) => {
            let providers: Vec<ProviderDetail> = rows
                .into_iter()
                .map(
                    |(provider_id, provider_type, name, enabled, config, created_at)| {
                        ProviderDetail {
                            provider_id,
                            provider_type,
                            name,
                            enabled,
                            config,
                            created_at,
                        }
                    },
                )
                .collect();
            (
                StatusCode::OK,
                Json(json!(ListProvidersResponse { providers })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

/// Get Ollama connection status via `GET /v1/providers/ollama/status`.
async fn ollama_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let gateway_url = format!("{}/health", state.model_router.gateway_url());
    let http_client = reqwest::Client::new();

    match http_client
        .get(&gateway_url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(health) => {
                let mut connected = false;
                let mut models = Vec::new();

                if let Some(providers) = health.get("providers").and_then(|v| v.as_array()) {
                    for p in providers {
                        let available = p
                            .get("available")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        if available {
                            connected = true;
                            if let Some(m) = p.get("models").and_then(|v| v.as_array()) {
                                for model in m {
                                    if let Some(name) = model.as_str() {
                                        models.push(name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }

                (
                    StatusCode::OK,
                    Json(json!(OllamaStatusResponse {
                        connected,
                        url: state.model_router.gateway_url().to_string(),
                        available_models: models,
                        error: None,
                    })),
                )
                    .into_response()
            }
            Err(e) => (
                StatusCode::OK,
                Json(json!(OllamaStatusResponse {
                    connected: false,
                    url: state.model_router.gateway_url().to_string(),
                    available_models: vec![],
                    error: Some(format!("Invalid health response: {e}")),
                })),
            )
                .into_response(),
        },
        Ok(resp) => (
            StatusCode::OK,
            Json(json!(OllamaStatusResponse {
                connected: false,
                url: state.model_router.gateway_url().to_string(),
                available_models: vec![],
                error: Some(format!("Gateway returned status {}", resp.status())),
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::OK,
            Json(json!(OllamaStatusResponse {
                connected: false,
                url: state.model_router.gateway_url().to_string(),
                available_models: vec![],
                error: Some(format!("Gateway unreachable: {e}")),
            })),
        )
            .into_response(),
    }
}

// =============================================================================
// SUB-AGENT HANDLERS
// =============================================================================

/// Query parameters for listing sub-agents.
#[derive(Debug, Deserialize)]
struct ListSubAgentsParams {
    /// Filter by parent identity ID
    parent_id: Option<Uuid>,
    /// Include terminated sub-agents (default: false)
    #[serde(default)]
    include_terminated: bool,
}

/// Extract and validate the caller identity from the `X-Identity-Id` header.
///
/// Returns the caller's `identity_id` if the header is present and the identity
/// exists in the database. Returns a 403 response otherwise.
async fn resolve_caller_identity(
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
) -> std::result::Result<Uuid, axum::response::Response> {
    let header_val = headers
        .get("X-Identity-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "Missing X-Identity-Id header"})),
            )
                .into_response()
        })?;

    let caller_id = Uuid::from_str(header_val).map_err(|_| {
        (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Invalid X-Identity-Id: not a valid UUID"})),
        )
            .into_response()
    })?;

    // Verify the identity exists in the database
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM identities WHERE identity_id = $1)")
            .bind(caller_id)
            .fetch_one(pool)
            .await
            .unwrap_or(false);

    if !exists {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": format!("Identity {caller_id} not found")})),
        )
            .into_response());
    }

    Ok(caller_id)
}

/// Spawn a sub-agent worker process, returning the worker ID or an error response.
///
/// Assembles an `IdentityPack`, parses the runtime from the sub-agent's directives,
/// and calls `WorkerManager::spawn_sub_agent_worker`.
async fn spawn_worker_for_sub_agent(
    state: &AppState,
    sub_agent: &crate::sub_agent::SubAgent,
) -> std::result::Result<String, axum::response::Response> {
    let identity_pack = state
        .sub_agent_manager
        .build_identity_pack(sub_agent)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to build identity pack: {e}")})),
            )
                .into_response()
        })?;

    let runtime_str = crate::sub_agent::SubAgentManager::extract_runtime(sub_agent);
    let runtime: WorkerRuntime = runtime_str.parse().map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response()
    })?;

    let mut wm = state.worker_manager.lock().await;
    let worker_id = wm
        .spawn_sub_agent_worker(sub_agent.sub_agent_id, runtime, identity_pack)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to spawn worker: {e}")})),
            )
                .into_response()
        })?;

    Ok(worker_id)
}

/// Create a sub-agent via `POST /v1/sub-agents`.
///
/// Resolves the caller identity from the `X-Identity-Id` header, validates
/// the caller has `sub_agent.create` capability, creates the identity and
/// sub_agent records in a transaction, grants requested capabilities, and
/// spawns a worker process for the new sub-agent.
async fn create_sub_agent_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateSubAgentRequest>,
) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": format!("{e}")})),
            )
                .into_response();
        }
    };

    // Resolve the authenticated caller identity from the request header
    let caller_id = match resolve_caller_identity(&headers, pool).await {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // Determine the parent identity: use body.parent_id if provided, else caller
    let parent_id = body.parent_id.unwrap_or(caller_id);

    // If an explicit parent_id was provided, verify the caller IS that identity
    if body.parent_id.is_some() && parent_id != caller_id {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Caller identity does not match the requested parent_id"})),
        )
            .into_response();
    }

    let sub_agent = match state
        .sub_agent_manager
        .create_sub_agent(
            parent_id,
            caller_id,
            body,
            &state.policy_engine,
            Some(&state.ledger),
        )
        .await
    {
        Ok(sa) => sa,
        Err(carnelian_common::Error::Security(msg)) => {
            return (StatusCode::FORBIDDEN, Json(json!({"error": msg}))).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("{e}")})),
            )
                .into_response();
        }
    };

    // Spawn a worker process for the newly created sub-agent
    let worker_id = match spawn_worker_for_sub_agent(&state, &sub_agent).await {
        Ok(wid) => Some(wid),
        Err(resp) => {
            tracing::warn!(
                sub_agent_id = %sub_agent.sub_agent_id,
                "Sub-agent created but worker spawn failed; returning creation result with warning"
            );
            // Return the sub-agent with a spawn warning rather than failing the whole request
            let mut payload = serde_json::to_value(&sub_agent).unwrap_or(json!({}));
            if let Some(obj) = payload.as_object_mut() {
                let _ = resp; // consume the error response
                obj.insert(
                    "worker_warning".to_string(),
                    json!("Worker spawn failed; sub-agent created but not running"),
                );
            }
            return (StatusCode::CREATED, Json(payload)).into_response();
        }
    };

    let mut payload = serde_json::to_value(&sub_agent).unwrap_or(json!({}));
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("worker_id".to_string(), json!(worker_id));
    }
    (StatusCode::CREATED, Json(payload)).into_response()
}

/// List sub-agents via `GET /v1/sub-agents`.
async fn list_sub_agents_handler(
    State(state): State<AppState>,
    Query(params): Query<ListSubAgentsParams>,
) -> impl IntoResponse {
    match state
        .sub_agent_manager
        .list_sub_agents(params.parent_id, params.include_terminated)
        .await
    {
        Ok(agents) => (StatusCode::OK, Json(json!({"sub_agents": agents}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

/// Get a sub-agent by ID via `GET /v1/sub-agents/{id}`.
async fn get_sub_agent_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.sub_agent_manager.get_sub_agent(id).await {
        Ok(Some(agent)) => (StatusCode::OK, Json(json!(agent))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Sub-agent not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

/// Update a sub-agent via `PUT /v1/sub-agents/{id}`.
async fn update_sub_agent_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateSubAgentRequest>,
) -> impl IntoResponse {
    match state.sub_agent_manager.update_sub_agent(id, body).await {
        Ok(agent) => (StatusCode::OK, Json(json!(agent))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

/// Delete (soft-terminate) a sub-agent via `DELETE /v1/sub-agents/{id}`.
///
/// Stops the worker process if one is running, then soft-deletes the record.
async fn delete_sub_agent_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // Stop worker if running
    let worker_id = format!("sub-agent-{id}");
    {
        let mut wm = state.worker_manager.lock().await;
        let _ = wm.stop_worker(&worker_id).await;
    }

    match state.sub_agent_manager.delete_sub_agent(id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Sub-agent not found or already terminated"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

/// Pause a sub-agent via `POST /v1/sub-agents/{id}/pause`.
///
/// Stops the worker process and sets the `_paused` flag in directives.
async fn pause_sub_agent_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // Stop worker if running
    let worker_id = format!("sub-agent-{id}");
    {
        let mut wm = state.worker_manager.lock().await;
        let _ = wm.stop_worker(&worker_id).await;
    }

    match state.sub_agent_manager.pause_sub_agent(id).await {
        Ok(()) => (StatusCode::OK, Json(json!({"status": "paused"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

/// Resume a sub-agent via `POST /v1/sub-agents/{id}/resume`.
///
/// Removes the `_paused` flag from directives and spawns a new worker process
/// using the runtime stored in the sub-agent's directives.
async fn resume_sub_agent_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // First resume the record (remove _paused flag)
    if let Err(e) = state.sub_agent_manager.resume_sub_agent(id).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response();
    }

    // Fetch the updated sub-agent to build the identity pack
    let sub_agent = match state.sub_agent_manager.get_sub_agent(id).await {
        Ok(Some(sa)) => sa,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Sub-agent not found after resume"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("{e}")})),
            )
                .into_response();
        }
    };

    // Spawn a worker for the resumed sub-agent
    match spawn_worker_for_sub_agent(&state, &sub_agent).await {
        Ok(worker_id) => (
            StatusCode::OK,
            Json(json!({"status": "resumed", "worker_id": worker_id})),
        )
            .into_response(),
        Err(_) => {
            // Record was resumed but worker failed to spawn
            (
                StatusCode::OK,
                Json(json!({
                    "status": "resumed",
                    "worker_warning": "Worker spawn failed; sub-agent resumed but not running"
                })),
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

// =============================================================================
// WORKFLOW HANDLERS
// =============================================================================

/// Create a workflow via `POST /v1/workflows`.
async fn create_workflow_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateWorkflowRequest>,
) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database pool unavailable"})),
            )
                .into_response();
        }
    };
    let caller_id = match resolve_caller_identity(&headers, pool).await {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if body.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Workflow name is required"})),
        )
            .into_response();
    }

    if body.steps.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Workflow must have at least one step"})),
        )
            .into_response();
    }

    match state
        .workflow_engine
        .create_workflow(body.name, body.description, body.steps, caller_id)
        .await
    {
        Ok(workflow) => (StatusCode::CREATED, Json(json!(workflow.to_detail()))).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// List workflows via `GET /v1/workflows`.
async fn list_workflows_handler(
    State(state): State<AppState>,
    Query(params): Query<ListWorkflowsParams>,
) -> impl IntoResponse {
    match state
        .workflow_engine
        .list_workflows(params.enabled_only)
        .await
    {
        Ok(workflows) => {
            let details: Vec<_> = workflows.iter().map(|w| w.to_detail()).collect();
            (
                StatusCode::OK,
                Json(json!(ListWorkflowsResponse { workflows: details })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get a workflow via `GET /v1/workflows/{id}`.
async fn get_workflow_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.workflow_engine.get_workflow(id).await {
        Ok(Some(workflow)) => (StatusCode::OK, Json(json!(workflow.to_detail()))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Workflow {id} not found")})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Update a workflow via `PUT /v1/workflows/{id}`.
async fn update_workflow_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateWorkflowRequest>,
) -> impl IntoResponse {
    match state
        .workflow_engine
        .update_workflow(id, body.name, body.description, body.steps)
        .await
    {
        Ok(workflow) => (StatusCode::OK, Json(json!(workflow.to_detail()))).into_response(),
        Err(e) => {
            let status = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::BAD_REQUEST
            };
            (status, Json(json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Delete a workflow via `DELETE /v1/workflows/{id}`.
async fn delete_workflow_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.workflow_engine.delete_workflow(id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Workflow {id} not found or already disabled")})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Execute a workflow via `POST /v1/workflows/{id}/execute`.
async fn execute_workflow_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ExecuteWorkflowRequest>,
) -> impl IntoResponse {
    match state
        .workflow_engine
        .execute_workflow(id, body.input, body.correlation_id)
        .await
    {
        Ok(result) => (StatusCode::OK, Json(json!(result))).into_response(),
        Err(e) => {
            let status = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else if e.to_string().contains("disabled") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, Json(json!({"error": e.to_string()}))).into_response()
        }
    }
}

// =============================================================================
// CHANNEL ADAPTER ENDPOINTS
// =============================================================================

/// Channel detail returned by channel API endpoints.
#[derive(Debug, Serialize, Deserialize)]
struct ChannelDetail {
    session_id: Uuid,
    channel_type: String,
    channel_user_id: String,
    trust_level: String,
    identity_id: Option<Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_seen_at: chrono::DateTime<chrono::Utc>,
    metadata: serde_json::Value,
    /// Whether an adapter is currently running for this channel.
    #[serde(skip_deserializing)]
    adapter_running: bool,
}

/// Request body for `POST /v1/channels`.
#[derive(Debug, Deserialize)]
struct CreateChannelRequest {
    channel_type: String,
    channel_user_id: String,
    /// Bot API token for the channel (Telegram bot token, Discord bot token, etc.).
    bot_token: Option<String>,
    trust_level: Option<String>,
    identity_id: Option<Uuid>,
    metadata: Option<serde_json::Value>,
    /// Whether to start the adapter immediately (default: true).
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

/// Request body for `PUT /v1/channels/{id}`.
#[derive(Debug, Deserialize)]
struct UpdateChannelRequest {
    trust_level: Option<String>,
    /// If provided, update the bot token and restart the adapter.
    bot_token: Option<String>,
    metadata: Option<serde_json::Value>,
}

/// Request body for `POST /v1/channels/{id}/pair`.
#[derive(Debug, Deserialize)]
struct PairChannelRequest {
    trust_level: Option<String>,
}

/// Query parameters for `GET /v1/channels`.
#[derive(Debug, Deserialize)]
struct ListChannelsQuery {
    channel_type: Option<String>,
}

/// Build and start a channel adapter for the given session, storing it in the adapters map.
///
/// Delegates to the `ChannelAdapterFactory` injected into `AppState`.
/// The factory is responsible for validating `channel_type`, storing the bot
/// token, and constructing the appropriate adapter.
async fn build_and_start_adapter(
    state: &AppState,
    session_id: Uuid,
    channel_type: &str,
    channel_user_id: &str,
    bot_token: &str,
    trust_level: &str,
    identity_id: Option<Uuid>,
) -> std::result::Result<(), String> {
    let factory = state
        .channel_adapter_factory
        .as_ref()
        .ok_or_else(|| "No channel adapter factory configured".to_string())?;

    let adapter = factory
        .build(
            session_id,
            channel_type,
            channel_user_id,
            bot_token,
            trust_level,
            identity_id,
        )
        .await
        .map_err(|e| format!("Failed to build adapter: {e}"))?;

    // Start the adapter
    adapter
        .start()
        .await
        .map_err(|e| format!("Failed to start adapter: {e}"))?;

    // Track it in the adapters map
    state
        .channel_adapters
        .write()
        .await
        .insert(session_id, adapter);

    Ok(())
}

/// List all channel sessions via `GET /v1/channels`.
async fn list_channels_handler(
    State(state): State<AppState>,
    Query(params): Query<ListChannelsQuery>,
) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let rows = if let Some(ref ct) = params.channel_type {
        sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                String,
                String,
                Option<Uuid>,
                chrono::DateTime<chrono::Utc>,
                chrono::DateTime<chrono::Utc>,
                serde_json::Value,
            ),
        >(
            r"SELECT session_id, channel_type, channel_user_id, trust_level,
                     identity_id, created_at, last_seen_at, metadata
              FROM channel_sessions WHERE channel_type = $1
              ORDER BY last_seen_at DESC",
        )
        .bind(ct)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                String,
                String,
                Option<Uuid>,
                chrono::DateTime<chrono::Utc>,
                chrono::DateTime<chrono::Utc>,
                serde_json::Value,
            ),
        >(
            r"SELECT session_id, channel_type, channel_user_id, trust_level,
                     identity_id, created_at, last_seen_at, metadata
              FROM channel_sessions
              ORDER BY last_seen_at DESC",
        )
        .fetch_all(pool)
        .await
    };

    match rows {
        Ok(rows) => {
            let adapters = state.channel_adapters.read().await;
            let channels: Vec<ChannelDetail> = rows
                .into_iter()
                .map(
                    |(
                        session_id,
                        channel_type,
                        channel_user_id,
                        trust_level,
                        identity_id,
                        created_at,
                        last_seen_at,
                        metadata,
                    )| {
                        let adapter_running =
                            adapters.get(&session_id).map_or(false, |a| a.is_running());
                        ChannelDetail {
                            session_id,
                            channel_type,
                            channel_user_id,
                            trust_level,
                            identity_id,
                            created_at,
                            last_seen_at,
                            metadata,
                            adapter_running,
                        }
                    },
                )
                .collect();
            (StatusCode::OK, Json(json!({"channels": channels}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Create a new channel session via `POST /v1/channels`.
///
/// Validates `channel_type`, inserts the session row, stores the bot token
/// in `config_store`, builds the corresponding adapter, starts it, and
/// tracks it in `AppState::channel_adapters`.
async fn create_channel_handler(
    State(state): State<AppState>,
    Json(body): Json<CreateChannelRequest>,
) -> impl IntoResponse {
    // Validate channel_type early
    let valid_types = ["telegram", "discord", "whatsapp", "slack", "ui"];
    if !valid_types.contains(&body.channel_type.to_lowercase().as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("Invalid channel_type '{}'. Must be one of: telegram, discord, whatsapp, slack, ui", body.channel_type)})),
        )
            .into_response();
    }

    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let trust_level = body.trust_level.as_deref().unwrap_or("untrusted");
    let metadata = body.metadata.unwrap_or(json!({}));

    // Insert the channel session row
    let result = sqlx::query_scalar::<_, Uuid>(
        r"INSERT INTO channel_sessions
            (channel_type, channel_user_id, trust_level, identity_id, metadata)
          VALUES ($1, $2, $3, $4, $5)
          RETURNING session_id",
    )
    .bind(&body.channel_type)
    .bind(&body.channel_user_id)
    .bind(trust_level)
    .bind(body.identity_id)
    .bind(&metadata)
    .fetch_one(pool)
    .await;

    match result {
        Ok(session_id) => {
            state.event_stream.publish(EventEnvelope::new(
                EventLevel::Info,
                EventType::Custom("ChannelCreated".to_string()),
                json!({
                    "session_id": session_id,
                    "channel_type": body.channel_type,
                    "channel_user_id": body.channel_user_id,
                    "trust_level": trust_level,
                }),
            ));

            // If a bot token was provided and the channel is enabled, build and start the adapter
            let mut adapter_status = "created_no_adapter";
            if let Some(ref token) = body.bot_token {
                if body.enabled {
                    match build_and_start_adapter(
                        &state,
                        session_id,
                        &body.channel_type,
                        &body.channel_user_id,
                        token,
                        trust_level,
                        body.identity_id,
                    )
                    .await
                    {
                        Ok(()) => {
                            adapter_status = "running";
                            tracing::info!(
                                session_id = %session_id,
                                channel_type = %body.channel_type,
                                "Channel adapter started"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                session_id = %session_id,
                                error = %e,
                                "Channel created but adapter failed to start"
                            );
                            adapter_status = "adapter_start_failed";
                        }
                    }
                } else {
                    // Store the token via raw SQL (adapter disabled)
                    let key = format!(
                        "channel.{}.{}.token",
                        body.channel_type, body.channel_user_id
                    );
                    let value = serde_json::Value::String(token.to_string());
                    let _ = sqlx::query(
                        r"INSERT INTO config_store (key, value) VALUES ($1, $2)
                          ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = NOW()",
                    )
                    .bind(&key)
                    .bind(&value)
                    .execute(pool)
                    .await;
                    adapter_status = "created_disabled";
                }
            }

            (
                StatusCode::CREATED,
                Json(json!({
                    "session_id": session_id,
                    "channel_type": body.channel_type,
                    "channel_user_id": body.channel_user_id,
                    "status": adapter_status,
                })),
            )
                .into_response()
        }
        Err(e) => {
            let status = if e.to_string().contains("duplicate key")
                || e.to_string().contains("unique constraint")
            {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };
            (status, Json(json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Get a single channel session via `GET /v1/channels/{id}`.
async fn get_channel_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let result = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            String,
            Option<Uuid>,
            chrono::DateTime<chrono::Utc>,
            chrono::DateTime<chrono::Utc>,
            serde_json::Value,
        ),
    >(
        r"SELECT session_id, channel_type, channel_user_id, trust_level,
                 identity_id, created_at, last_seen_at, metadata
          FROM channel_sessions WHERE session_id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;

    match result {
        Ok(Some((
            session_id,
            channel_type,
            channel_user_id,
            trust_level,
            identity_id,
            created_at,
            last_seen_at,
            metadata,
        ))) => {
            let adapter_running = state
                .channel_adapters
                .read()
                .await
                .get(&session_id)
                .map_or(false, |a| a.is_running());
            let detail = ChannelDetail {
                session_id,
                channel_type,
                channel_user_id,
                trust_level,
                identity_id,
                created_at,
                last_seen_at,
                metadata,
                adapter_running,
            };
            (StatusCode::OK, Json(json!(detail))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Channel session {id} not found")})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Update a channel session via `PUT /v1/channels/{id}`.
///
/// If `bot_token` is provided, the existing adapter is stopped, credentials
/// are updated, and a new adapter is built and started.
async fn update_channel_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateChannelRequest>,
) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    // Build dynamic update query
    let mut updates = Vec::new();
    let mut param_idx = 2_u32; // $1 is session_id

    if body.trust_level.is_some() {
        updates.push(format!("trust_level = ${param_idx}"));
        param_idx += 1;
    }
    if body.metadata.is_some() {
        updates.push(format!("metadata = ${param_idx}"));
        let _ = param_idx; // suppress unused_assignments
    }

    if updates.is_empty() && body.bot_token.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "No fields to update"})),
        )
            .into_response();
    }

    // Apply DB updates if any fields changed
    if !updates.is_empty() {
        updates.push("last_seen_at = NOW()".to_string());
        let set_clause = updates.join(", ");
        let query_str = format!(
            "UPDATE channel_sessions SET {set_clause} WHERE session_id = $1 RETURNING session_id"
        );

        let mut query = sqlx::query_scalar::<_, Uuid>(&query_str).bind(id);

        if let Some(ref tl) = body.trust_level {
            query = query.bind(tl);
        }
        if let Some(ref md) = body.metadata {
            query = query.bind(md);
        }

        match query.fetch_optional(pool).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": format!("Channel session {id} not found")})),
                )
                    .into_response();
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e.to_string()})),
                )
                    .into_response();
            }
        }
    }

    // If bot_token changed, restart the adapter
    let mut adapter_restarted = false;
    if let Some(ref new_token) = body.bot_token {
        // Look up channel_type and channel_user_id from DB
        let row = sqlx::query_as::<_, (String, String, String, Option<Uuid>)>(
            "SELECT channel_type, channel_user_id, trust_level, identity_id FROM channel_sessions WHERE session_id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await;

        if let Ok(Some((channel_type, channel_user_id, trust_level, identity_id))) = row {
            // Stop existing adapter if running
            if let Some(adapter) = state.channel_adapters.write().await.remove(&id) {
                let _ = adapter.stop().await;
            }

            // Build and start new adapter with updated token
            match build_and_start_adapter(
                &state,
                id,
                &channel_type,
                &channel_user_id,
                new_token,
                &trust_level,
                identity_id,
            )
            .await
            {
                Ok(()) => {
                    adapter_restarted = true;
                    tracing::info!(session_id = %id, "Channel adapter restarted with new token");
                }
                Err(e) => {
                    tracing::warn!(session_id = %id, error = %e, "Failed to restart adapter");
                }
            }
        }
    }

    state.event_stream.publish(EventEnvelope::new(
        EventLevel::Info,
        EventType::Custom("ChannelUpdated".to_string()),
        json!({
            "session_id": id,
            "trust_level": body.trust_level,
            "adapter_restarted": adapter_restarted,
        }),
    ));
    (
        StatusCode::OK,
        Json(json!({
            "session_id": id,
            "status": "updated",
            "adapter_restarted": adapter_restarted,
        })),
    )
        .into_response()
}

/// Delete a channel session via `DELETE /v1/channels/{id}`.
///
/// Stops the running adapter, removes credentials from `config_store`,
/// and deletes the session row.
async fn delete_channel_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    // Look up channel info before deletion so we can clean up credentials
    let channel_info = sqlx::query_as::<_, (String, String)>(
        "SELECT channel_type, channel_user_id FROM channel_sessions WHERE session_id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;

    // Stop the adapter if running
    if let Some(adapter) = state.channel_adapters.write().await.remove(&id) {
        if let Err(e) = adapter.stop().await {
            tracing::warn!(session_id = %id, error = %e, "Error stopping adapter during delete");
        }
    }

    // Remove credentials from config_store
    if let Ok(Some((ref channel_type, ref channel_user_id))) = channel_info {
        if let Some(ref factory) = state.channel_adapter_factory {
            let _ = factory
                .delete_credentials(channel_type, channel_user_id)
                .await;
        } else {
            // Fallback: delete directly via SQL
            let key = format!("channel.{channel_type}.{channel_user_id}.token");
            let _ = sqlx::query("DELETE FROM config_store WHERE key = $1")
                .bind(&key)
                .execute(pool)
                .await;
        }
    }

    // Delete the session row
    match sqlx::query("DELETE FROM channel_sessions WHERE session_id = $1")
        .bind(id)
        .execute(pool)
        .await
    {
        Ok(result) => {
            if result.rows_affected() > 0 {
                state.event_stream.publish(EventEnvelope::new(
                    EventLevel::Info,
                    EventType::Custom("ChannelDeleted".to_string()),
                    json!({"session_id": id}),
                ));
                StatusCode::NO_CONTENT.into_response()
            } else {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": format!("Channel session {id} not found")})),
                )
                    .into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Initiate pairing for a channel session via `POST /v1/channels/{id}/pair`.
///
/// Generates a pairing token and stores it in the session metadata.
async fn pair_channel_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<PairChannelRequest>,
) -> impl IntoResponse {
    let pool = match state.config.pool() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    // Verify session exists
    let exists = sqlx::query_scalar::<_, Uuid>(
        "SELECT session_id FROM channel_sessions WHERE session_id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;

    match exists {
        Ok(Some(_)) => {
            let trust_level = body.trust_level.as_deref().unwrap_or("conversational");

            // Validate the requested trust level
            let valid_trust_levels = ["untrusted", "conversational", "owner"];
            if !valid_trust_levels.contains(&trust_level) {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": format!(
                        "Invalid trust_level '{}'. Must be one of: untrusted, conversational, owner",
                        trust_level
                    )})),
                )
                    .into_response();
            }

            let pairing_token = Uuid::now_v7();
            let expires_at = chrono::Utc::now() + chrono::Duration::minutes(15);

            let metadata = json!({
                "pairing_token": pairing_token.to_string(),
                "pairing_status": "pending",
                "pairing_expires_at": expires_at.to_rfc3339(),
                "requested_trust_level": trust_level,
            });

            let _ = sqlx::query(
                "UPDATE channel_sessions SET metadata = $2, last_seen_at = NOW() WHERE session_id = $1",
            )
            .bind(id)
            .bind(&metadata)
            .execute(pool)
            .await;

            (
                StatusCode::OK,
                Json(json!({
                    "pairing_token": pairing_token,
                    "expires_at": expires_at.to_rfc3339(),
                    "requested_trust_level": trust_level,
                    "instructions": format!(
                        "Use the pairing token to complete pairing: /pair {}",
                        pairing_token
                    ),
                })),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Channel session {id} not found")})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// =============================================================================
// XP HANDLERS
// =============================================================================

/// Get agent XP details via `GET /v1/xp/agents/:id`.
async fn get_agent_xp_handler(
    State(state): State<AppState>,
    Path(identity_id): Path<Uuid>,
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

    // Fetch agent_xp row
    let row: Option<(i64, i32, i64)> = match sqlx::query_as(
        "SELECT total_xp, level, xp_to_next_level FROM agent_xp WHERE identity_id = $1",
    )
    .bind(identity_id)
    .fetch_optional(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let (total_xp, level, xp_to_next_level) = match row {
        Some(r) => r,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "agent XP record not found"})),
            )
                .into_response();
        }
    };

    // Compute progress_pct from level_progression
    let progress_pct = if level >= 99 {
        100.0
    } else {
        // Current level lower bound
        let lower: Option<i64> = match sqlx::query_scalar(
            "SELECT total_xp_required FROM level_progression WHERE level = $1",
        )
        .bind(level)
        .fetch_optional(pool)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("failed to query level_progression lower bound: {}", e)})),
                )
                    .into_response();
            }
        };

        // Next level upper bound
        let upper: Option<i64> = match sqlx::query_scalar(
            "SELECT total_xp_required FROM level_progression WHERE level = $1",
        )
        .bind(level + 1)
        .fetch_optional(pool)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("failed to query level_progression upper bound: {}", e)})),
                )
                    .into_response();
            }
        };

        match (lower, upper) {
            (Some(lo), Some(hi)) if hi > lo => {
                ((total_xp - lo) as f64 / (hi - lo) as f64 * 100.0).clamp(0.0, 100.0)
            }
            _ => 0.0,
        }
    };

    // Fetch milestone_feature for current level
    let milestone_feature: Option<String> = match sqlx::query_scalar(
        "SELECT milestone_feature FROM level_progression WHERE level = $1",
    )
    .bind(level)
    .fetch_optional(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("failed to query milestone_feature: {}", e)})),
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        Json(
            serde_json::to_value(AgentXpResponse {
                identity_id,
                total_xp,
                level,
                xp_to_next_level,
                progress_pct,
                milestone_feature,
            })
            .unwrap_or_default(),
        ),
    )
        .into_response()
}

/// Get paginated XP history via `GET /v1/xp/agents/:id/history`.
async fn get_xp_history_handler(
    State(state): State<AppState>,
    Path(identity_id): Path<Uuid>,
    Query(params): Query<XpHistoryQuery>,
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
    let page_size = params.page_size.min(XpHistoryQuery::MAX_PAGE_SIZE).max(1);
    let offset = (page - 1) * page_size;

    // Total count
    let total: i64 =
        match sqlx::query_scalar("SELECT COUNT(*) FROM xp_events WHERE identity_id = $1")
            .bind(identity_id)
            .fetch_one(pool)
            .await
        {
            Ok(t) => t,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e.to_string()})),
                )
                    .into_response();
            }
        };

    // Fetch page
    let rows = match sqlx::query_as::<_, (i64, String, i32, Option<Uuid>, Option<Uuid>, Option<i64>, serde_json::Value, chrono::DateTime<chrono::Utc>)>(
        r"SELECT xp_event_id, source, xp_amount, task_id, skill_id, ledger_event_id, metadata, created_at
          FROM xp_events WHERE identity_id = $1
          ORDER BY created_at DESC
          LIMIT $2 OFFSET $3",
    )
    .bind(identity_id)
    .bind(i64::from(page_size))
    .bind(i64::from(offset))
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let events: Vec<XpEventDetail> = rows
        .into_iter()
        .map(
            |(
                event_id,
                source,
                xp_amount,
                task_id,
                skill_id,
                ledger_event_id,
                metadata,
                created_at,
            )| {
                XpEventDetail {
                    event_id,
                    source,
                    xp_amount,
                    task_id,
                    skill_id,
                    ledger_event_id,
                    metadata,
                    created_at,
                }
            },
        )
        .collect();

    (
        StatusCode::OK,
        Json(
            serde_json::to_value(XpHistoryResponse {
                events,
                page,
                page_size,
                total,
            })
            .unwrap_or_default(),
        ),
    )
        .into_response()
}

/// Get XP leaderboard via `GET /v1/xp/leaderboard`.
async fn get_xp_leaderboard_handler(State(state): State<AppState>) -> impl IntoResponse {
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

    let rows = match sqlx::query_as::<_, (i64, Uuid, String, i64, i32)>(
        r"SELECT ROW_NUMBER() OVER (ORDER BY ax.total_xp DESC) AS rank,
                 ax.identity_id, i.name, ax.total_xp, ax.level
          FROM agent_xp ax
          JOIN identities i ON i.identity_id = ax.identity_id
          ORDER BY ax.total_xp DESC",
    )
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let entries: Vec<LeaderboardEntry> = rows
        .into_iter()
        .map(
            |(rank, identity_id, name, total_xp, level)| LeaderboardEntry {
                rank,
                identity_id,
                name,
                total_xp,
                level,
            },
        )
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::to_value(XpLeaderboardResponse { entries }).unwrap_or_default()),
    )
        .into_response()
}

/// Get skill metrics via `GET /v1/xp/skills/:id`.
async fn get_skill_metrics_handler(
    State(state): State<AppState>,
    Path(skill_id): Path<Uuid>,
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

    let row = match sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            i32,
            i32,
            i32,
            i32,
            i64,
            i32,
            Option<chrono::DateTime<chrono::Utc>>,
        ),
    >(
        r"SELECT sm.skill_id, s.name AS skill_name,
                 sm.usage_count, sm.success_count, sm.failure_count,
                 sm.avg_duration_ms, sm.total_xp_earned, sm.skill_level, sm.last_used_at
          FROM skill_metrics sm
          JOIN skills s ON s.skill_id = sm.skill_id
          WHERE sm.skill_id = $1",
    )
    .bind(skill_id)
    .fetch_optional(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let (
        sid,
        skill_name,
        usage_count,
        success_count,
        failure_count,
        avg_duration_ms,
        total_xp_earned,
        skill_level,
        last_used_at,
    ) = match row {
        Some(r) => r,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "skill metrics not found"})),
            )
                .into_response();
        }
    };

    let success_rate = if usage_count > 0 {
        success_count as f64 / usage_count as f64
    } else {
        0.0
    };

    (
        StatusCode::OK,
        Json(
            serde_json::to_value(SkillMetricsDetail {
                skill_id: sid,
                skill_name,
                usage_count,
                success_count,
                failure_count,
                success_rate,
                avg_duration_ms,
                total_xp_earned,
                skill_level,
                last_used_at,
            })
            .unwrap_or_default(),
        ),
    )
        .into_response()
}

/// Get top skills by XP earned via `GET /v1/xp/skills/top`.
async fn get_top_skills_handler(
    State(state): State<AppState>,
    Query(params): Query<TopSkillsQuery>,
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

    let limit = params.limit.clamp(1, 100);

    let rows = match sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            i32,
            i32,
            i32,
            i32,
            i64,
            i32,
            Option<chrono::DateTime<chrono::Utc>>,
        ),
    >(
        r"SELECT sm.skill_id, s.name AS skill_name,
                 sm.usage_count, sm.success_count, sm.failure_count,
                 sm.avg_duration_ms, sm.total_xp_earned, sm.skill_level, sm.last_used_at
          FROM skill_metrics sm
          JOIN skills s ON s.skill_id = sm.skill_id
          ORDER BY sm.total_xp_earned DESC
          LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let skills: Vec<SkillMetricsDetail> = rows
        .into_iter()
        .map(
            |(
                skill_id,
                skill_name,
                usage_count,
                success_count,
                failure_count,
                avg_duration_ms,
                total_xp_earned,
                skill_level,
                last_used_at,
            )| {
                let success_rate = if usage_count > 0 {
                    success_count as f64 / usage_count as f64
                } else {
                    0.0
                };
                SkillMetricsDetail {
                    skill_id,
                    skill_name,
                    usage_count,
                    success_count,
                    failure_count,
                    success_rate,
                    avg_duration_ms,
                    total_xp_earned,
                    skill_level,
                    last_used_at,
                }
            },
        )
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::to_value(TopSkillsResponse { skills }).unwrap_or_default()),
    )
        .into_response()
}

/// Admin-gated XP award via `POST /v1/xp/award`.
async fn award_xp_handler(
    State(state): State<AppState>,
    Json(body): Json<AwardXpRequest>,
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

    // Load owner signing key for cryptographic verification
    let signing_key = match state.config.owner_signing_key() {
        Some(sk) => sk,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "owner signing key not configured"})),
            )
                .into_response();
        }
    };

    // Validate signature is present
    if body.signature.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "signature is required"})),
        )
            .into_response();
    }

    // Verify signature over canonical message "{identity_id}:{xp_amount}:{source}"
    let canonical_message = format!("{}:{}:{}", body.identity_id, body.xp_amount, body.source);
    let public_key_hex = crate::crypto::public_key_from_signing_key(signing_key);
    match crate::crypto::verify_signature(
        &public_key_hex,
        canonical_message.as_bytes(),
        &body.signature,
    ) {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "invalid signature"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("signature verification failed: {}", e)})),
            )
                .into_response();
        }
    }

    // Validate xp_amount > 0
    if body.xp_amount <= 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "xp_amount must be positive"})),
        )
            .into_response();
    }

    // Map body.source to the correct XpSource variant, validating required IDs
    let xp_source = match body.source.as_str() {
        "task_completion" => {
            let task_id = match body.task_id {
                Some(id) => id,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": "task_id is required for source 'task_completion'"})),
                    )
                        .into_response();
                }
            };
            crate::xp::XpSource::TaskCompletion {
                task_id,
                skill_id: body.skill_id,
            }
        }
        "ledger_signing" => {
            let ledger_event_id = match body.ledger_event_id {
                Some(id) => id,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": "ledger_event_id is required for source 'ledger_signing'"})),
                    )
                        .into_response();
                }
            };
            crate::xp::XpSource::LedgerSigning { ledger_event_id }
        }
        "skill_usage" => {
            let skill_id = match body.skill_id {
                Some(id) => id,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": "skill_id is required for source 'skill_usage'"})),
                    )
                        .into_response();
                }
            };
            crate::xp::XpSource::SkillUsage { skill_id }
        }
        "quality_bonus" => crate::xp::XpSource::QualityBonus,
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("unknown source '{}', allowed: [\"task_completion\", \"ledger_signing\", \"skill_usage\", \"quality_bonus\"]", other)})),
            )
                .into_response();
        }
    };

    // Ensure agent_xp row exists
    if let Err(e) = state.xp_manager.ensure_agent_xp(body.identity_id).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("failed to ensure agent XP: {}", e)})),
        )
            .into_response();
    }

    // Award XP using the mapped source variant
    let level_up = match state
        .xp_manager
        .award_xp(body.identity_id, xp_source, body.xp_amount, body.metadata)
        .await
    {
        Ok(lu) => lu,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("failed to award XP: {}", e)})),
            )
                .into_response();
        }
    };

    // Query updated total_xp
    let new_total_xp: i64 =
        match sqlx::query_scalar("SELECT total_xp FROM agent_xp WHERE identity_id = $1")
            .bind(body.identity_id)
            .fetch_one(pool)
            .await
        {
            Ok(t) => t,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e.to_string()})),
                )
                    .into_response();
            }
        };

    (
        StatusCode::OK,
        Json(
            serde_json::to_value(AwardXpResponse {
                identity_id: body.identity_id,
                xp_awarded: body.xp_amount,
                new_total_xp,
                level_up,
            })
            .unwrap_or_default(),
        ),
    )
        .into_response()
}

// =============================================================================
// VOICE HANDLERS
// =============================================================================

/// Configure voice settings via `POST /v1/voice/configure`.
async fn configure_voice_handler(
    State(state): State<AppState>,
    Json(req): Json<ConfigureVoiceRequest>,
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

    // Build EncryptionHelper if owner signing key is available
    let encryption_helper = state
        .config
        .owner_signing_key()
        .map(|sk| crate::encryption::EncryptionHelper::new(pool, sk));

    // Save the API key
    if let Err(e) = state
        .voice_gateway
        .save_api_key(&req.api_key, encryption_helper.as_ref())
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to save API key: {}", e)})),
        )
            .into_response();
    }

    // Optionally update identity voice config
    if let (Some(identity_id), Some(voice_id)) = (req.identity_id, &req.default_voice_id) {
        if let Err(e) = state
            .voice_gateway
            .update_identity_voice_config(
                identity_id,
                serde_json::json!({
                    "default_voice_id": voice_id,
                    "enabled": true
                }),
            )
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update identity voice config: {}", e)})),
            )
                .into_response();
        }
    }

    (
        StatusCode::OK,
        Json(
            serde_json::to_value(ConfigureVoiceResponse {
                success: true,
                message: "Voice configured".to_string(),
            })
            .unwrap_or_default(),
        ),
    )
        .into_response()
}

/// Test text-to-speech via `POST /v1/voice/test`.
async fn test_voice_handler(
    State(state): State<AppState>,
    Json(req): Json<TestVoiceRequest>,
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

    let encryption_helper = state
        .config
        .owner_signing_key()
        .map(|sk| crate::encryption::EncryptionHelper::new(pool, sk));

    let audio_bytes = match state
        .voice_gateway
        .text_to_speech(&req.text, &req.voice_id, encryption_helper.as_ref())
        .await
    {
        Ok(bytes) => bytes,
        Err(carnelian_common::Error::Security(_)) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "ElevenLabs API key is invalid or expired"})),
            )
                .into_response();
        }
        Err(carnelian_common::Error::RateLimitExceeded(msg)) => {
            return (StatusCode::TOO_MANY_REQUESTS, Json(json!({"error": msg}))).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&audio_bytes);

    (
        StatusCode::OK,
        Json(
            serde_json::to_value(TestVoiceResponse {
                audio_base64,
                content_type: "audio/mpeg".to_string(),
            })
            .unwrap_or_default(),
        ),
    )
        .into_response()
}

/// List available voices via `GET /v1/voice/voices`.
async fn list_voices_handler(State(state): State<AppState>) -> impl IntoResponse {
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

    let encryption_helper = state
        .config
        .owner_signing_key()
        .map(|sk| crate::encryption::EncryptionHelper::new(pool, sk));

    let voices = match state
        .voice_gateway
        .list_voices(encryption_helper.as_ref())
        .await
    {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        Json(serde_json::to_value(ListVoicesResponse { voices }).unwrap_or_default()),
    )
        .into_response()
}

/// Transcribe audio to text via `POST /v1/voice/transcribe`.
async fn transcribe_voice_handler(
    State(state): State<AppState>,
    Json(req): Json<TranscribeVoiceRequest>,
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

    let encryption_helper = state
        .config
        .owner_signing_key()
        .map(|sk| crate::encryption::EncryptionHelper::new(pool, sk));

    let audio_bytes = match base64::engine::general_purpose::STANDARD.decode(&req.audio_base64) {
        Ok(bytes) => bytes::Bytes::from(bytes),
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Invalid base64 audio: {}", e)})),
            )
                .into_response();
        }
    };

    let text = match state
        .voice_gateway
        .speech_to_text(audio_bytes, encryption_helper.as_ref())
        .await
    {
        Ok(t) => t,
        Err(carnelian_common::Error::Security(_)) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "ElevenLabs API key is invalid or expired"})),
            )
                .into_response();
        }
        Err(carnelian_common::Error::RateLimitExceeded(msg)) => {
            return (StatusCode::TOO_MANY_REQUESTS, Json(json!({"error": msg}))).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        Json(serde_json::to_value(TranscribeVoiceResponse { text }).unwrap_or_default()),
    )
        .into_response()
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
        let model_router = Arc::new(ModelRouter::new(
            pool.clone(),
            "http://localhost:18790".to_string(),
            policy_engine.clone(),
            ledger.clone(),
        ));
        let safe_mode_guard = Arc::new(SafeModeGuard::new(pool.clone(), ledger.clone()));
        let session_manager = Arc::new(
            SessionManager::with_defaults(pool.clone())
                .with_safe_mode_guard(safe_mode_guard.clone()),
        );
        let memory_manager = {
            let chain_anchor = Arc::new(crate::chain_anchor::LocalDbChainAnchor::new(pool.clone()));
            Arc::new(
                MemoryManager::new(pool.clone(), Some(event_stream.clone()))
                    .with_chain_anchor(chain_anchor),
            )
        };
        let sub_agent_manager = Arc::new(SubAgentManager::new(
            pool.clone(),
            Some(event_stream.clone()),
        ));
        let workflow_engine = Arc::new(crate::workflow::WorkflowEngine::new(
            pool.clone(),
            Some(event_stream.clone()),
        ));
        let scheduler = Arc::new(tokio::sync::Mutex::new(Scheduler::new(
            pool.clone(),
            event_stream.clone(),
            Duration::from_secs(3600),
            worker_manager.clone(),
            config.clone(),
            model_router.clone(),
            ledger.clone(),
            safe_mode_guard.clone(),
        )));
        let xp_manager = Arc::new(crate::xp::XpManager::new(pool.clone()));
        let voice_gateway = Arc::new(crate::voice::VoiceGateway::new(pool.clone()));
        AppState::new(
            config,
            event_stream,
            policy_engine,
            ledger,
            worker_manager,
            scheduler,
            model_router,
            safe_mode_guard,
            session_manager,
            memory_manager,
            sub_agent_manager,
            workflow_engine,
            xp_manager,
            voice_gateway,
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

// =============================================================================
// LEDGER ANCHOR HANDLERS
// =============================================================================

/// Request body for publishing a ledger anchor
#[derive(Debug, Deserialize)]
pub struct PublishAnchorRequest {
    pub from_event_id: i64,
    pub to_event_id: i64,
}

/// Response for publish anchor endpoint
#[derive(Debug, Serialize)]
pub struct PublishAnchorResponse {
    pub anchor_id: String,
    pub merkle_root: String,
    pub event_count: i64,
}

/// Publish a ledger slice anchor
async fn publish_ledger_anchor_handler(
    State(state): State<AppState>,
    Json(body): Json<PublishAnchorRequest>,
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

    // Create LocalDbChainAnchor
    let chain_anchor = crate::chain_anchor::LocalDbChainAnchor::new(pool.clone());

    // Get owner signing key from config
    let mut config_guard = state.config.clone();
    let owner_signing_key = config_guard.owner_signing_key().cloned();

    match state
        .ledger
        .publish_ledger_anchor(
            body.from_event_id,
            body.to_event_id,
            &chain_anchor,
            owner_signing_key.as_ref(),
        )
        .await
    {
        Ok(anchor_id) => {
            // Get the proof to return the merkle root
            match chain_anchor.get_anchor_proof(&anchor_id).await {
                Ok(Some(proof)) => {
                    // Extract merkle_root and event_count from nested metadata object
                    let metadata = proof.get("metadata").and_then(|m| m.as_object());
                    let merkle_root = metadata
                        .and_then(|m| m.get("merkle_root"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let event_count = metadata
                        .and_then(|m| m.get("event_count"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);

                    let response = PublishAnchorResponse {
                        anchor_id,
                        merkle_root,
                        event_count,
                    };
                    (
                        StatusCode::OK,
                        Json(serde_json::to_value(response).unwrap()),
                    )
                        .into_response()
                }
                _ => (StatusCode::OK, Json(json!({"anchor_id": anchor_id}))).into_response(),
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to publish ledger anchor");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to publish anchor: {}", e)})),
            )
                .into_response()
        }
    }
}

/// Response for get anchor endpoint
#[derive(Debug, Serialize)]
pub struct AnchorProofResponse {
    pub anchor_id: String,
    pub hash: String,
    pub ledger_event_from: i64,
    pub ledger_event_to: i64,
    pub published_at: String,
    pub metadata: serde_json::Value,
    pub verified: bool,
}

/// Get anchor proof by ID
async fn get_ledger_anchor_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
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

    let chain_anchor = crate::chain_anchor::LocalDbChainAnchor::new(pool.clone());

    match chain_anchor.get_anchor_proof(&id).await {
        Ok(Some(proof)) => {
            let response = AnchorProofResponse {
                anchor_id: proof
                    .get("anchor_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                hash: proof
                    .get("hash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                ledger_event_from: proof
                    .get("ledger_event_from")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                ledger_event_to: proof
                    .get("ledger_event_to")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                published_at: proof
                    .get("published_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                metadata: proof
                    .get("metadata")
                    .cloned()
                    .unwrap_or(serde_json::json!({})),
                verified: proof
                    .get("verified")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            };
            (
                StatusCode::OK,
                Json(serde_json::to_value(response).unwrap()),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Anchor not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get anchor proof");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get anchor: {}", e)})),
            )
                .into_response()
        }
    }
}

/// Verify a hash against a stored anchor
async fn verify_ledger_anchor_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let hash = match params.get("hash") {
        Some(h) => h,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Missing 'hash' query parameter"})),
            )
                .into_response();
        }
    };

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

    let chain_anchor = crate::chain_anchor::LocalDbChainAnchor::new(pool.clone());

    match chain_anchor.verify_anchor(&id, hash).await {
        Ok(matches) => (
            StatusCode::OK,
            Json(json!({
                "anchor_id": id,
                "hash": hash,
                "verified": matches,
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to verify anchor");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to verify anchor: {}", e)})),
            )
                .into_response()
        }
    }
}

/// Query parameters for list revoked grants endpoint
#[derive(Debug, Deserialize)]
pub struct ListRevokedGrantsQuery {
    /// ISO8601 timestamp to list revocations since
    pub since: Option<String>,
}

/// Response for revoked grants endpoint
#[derive(Debug, Serialize)]
pub struct RevokedGrantsResponse {
    /// List of revoked grants
    pub grants: Vec<RevokedGrantResponse>,
}

/// Individual revoked grant response
#[derive(Debug, Serialize)]
pub struct RevokedGrantResponse {
    pub grant_id: String,
    pub revoked_at: String,
    pub revoked_by: Option<String>,
    pub reason: Option<String>,
}

/// Query parameters for `GET /v1/ledger/events`.
#[derive(Debug, Deserialize)]
struct LedgerEventsQuery {
    #[serde(default = "default_ledger_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
    #[serde(default)]
    action_type: Option<String>,
    #[serde(default)]
    actor_id: Option<String>,
    #[serde(default)]
    from_ts: Option<String>,
    #[serde(default)]
    to_ts: Option<String>,
}

fn default_ledger_limit() -> i64 {
    50
}

/// List ledger events via `GET /v1/ledger/events`.
async fn list_ledger_events_handler(
    State(state): State<AppState>,
    Query(params): Query<LedgerEventsQuery>,
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

    let limit = params.limit.clamp(1, 100);
    let offset = params.offset.max(0);

    // Build the base query
    let mut sql = String::from(
        r"SELECT event_id, timestamp, actor_id, action_type, payload_hash, event_hash, signature 
          FROM ledger_events WHERE 1=1",
    );
    let mut binds: Vec<Box<dyn std::any::Any>> = Vec::new();

    // Add filters
    if let Some(ref action_type) = params.action_type {
        sql.push_str(" AND action_type = $");
        sql.push_str(&(binds.len() + 1).to_string());
        binds.push(Box::new(action_type.clone()));
    }
    if let Some(ref actor_id) = params.actor_id {
        sql.push_str(" AND actor_id = $");
        sql.push_str(&(binds.len() + 1).to_string());
        binds.push(Box::new(actor_id.clone()));
    }
    if let Some(ref from_ts) = params.from_ts {
        sql.push_str(" AND timestamp >= $");
        sql.push_str(&(binds.len() + 1).to_string());
        binds.push(Box::new(from_ts.clone()));
    }
    if let Some(ref to_ts) = params.to_ts {
        sql.push_str(" AND timestamp <= $");
        sql.push_str(&(binds.len() + 1).to_string());
        binds.push(Box::new(to_ts.clone()));
    }

    // Count total for pagination
    let count_sql = sql.replace(
        "SELECT event_id, timestamp, actor_id, action_type, payload_hash, event_hash, signature",
        "SELECT COUNT(*)",
    );

    let total: i64 = match sqlx::query_scalar(&count_sql).fetch_one(pool).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to count ledger events");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
                .into_response();
        }
    };

    // Add ordering and pagination
    sql.push_str(&format!(
        " ORDER BY event_id DESC LIMIT {} OFFSET {}",
        limit, offset
    ));

    // Execute query
    let rows: Vec<(
        i64,
        chrono::DateTime<chrono::Utc>,
        String,
        String,
        String,
        String,
        Option<String>,
    )> = match sqlx::query_as(&sql).fetch_all(pool).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to fetch ledger events");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
                .into_response();
        }
    };

    let events: Vec<carnelian_common::types::LedgerEventDetail> = rows
        .into_iter()
        .map(
            |(event_id, timestamp, actor_id, action_type, payload_hash, event_hash, signature)| {
                carnelian_common::types::LedgerEventDetail {
                    event_id,
                    timestamp: timestamp.to_rfc3339(),
                    actor_id,
                    action_type,
                    payload_hash,
                    event_hash,
                    signature,
                }
            },
        )
        .collect();

    let response = carnelian_common::types::ListLedgerEventsResponse {
        events,
        total,
        offset,
        limit,
    };

    (
        StatusCode::OK,
        Json(serde_json::to_value(response).unwrap_or_default()),
    )
        .into_response()
}

/// Verify ledger chain integrity via `GET /v1/ledger/verify`.
async fn verify_ledger_chain_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.ledger.verify_chain(None).await {
        Ok(intact) => {
            let (event_count, first_event_id, last_event_id) = if let Ok(pool) = state.config.pool()
            {
                let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ledger_events")
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0);
                let first: Option<i64> =
                    sqlx::query_scalar("SELECT MIN(event_id) FROM ledger_events")
                        .fetch_one(pool)
                        .await
                        .unwrap_or(None);
                let last: Option<i64> =
                    sqlx::query_scalar("SELECT MAX(event_id) FROM ledger_events")
                        .fetch_one(pool)
                        .await
                        .unwrap_or(None);
                (count as u64, first, last)
            } else {
                (0u64, None, None)
            };
            let response = carnelian_common::types::LedgerVerifyResponse {
                intact,
                event_count,
                first_event_id,
                last_event_id,
            };
            (
                StatusCode::OK,
                Json(serde_json::to_value(response).unwrap_or_default()),
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to verify ledger chain");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Verification failed: {}", e)})),
            )
                .into_response()
        }
    }
}

/// Get setup status via `GET /v1/config/setup-status`.
async fn get_setup_status_handler(State(state): State<AppState>) -> impl IntoResponse {
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

    // Check if setup_complete key exists in config_store
    let setup_complete: Option<String> =
        sqlx::query_scalar("SELECT value FROM config_store WHERE key = 'setup_complete'")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

    // Check if machine.toml exists
    let machine_toml_exists = std::path::Path::new("machine.toml").exists()
        || std::path::Path::new("/etc/carnelian/machine.toml").exists()
        || std::env::var("CARNELIAN_CONFIG_PATH").is_ok();

    let response = carnelian_common::types::SetupStatusResponse {
        setup_complete: setup_complete.is_some(),
        machine_toml_exists,
    };

    (
        StatusCode::OK,
        Json(serde_json::to_value(response).unwrap_or_default()),
    )
        .into_response()
}

/// Mark setup as complete via `POST /v1/config/setup-complete`.
async fn post_setup_complete_handler(State(state): State<AppState>) -> impl IntoResponse {
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

    // Insert setup_complete = true into config_store
    let result = sqlx::query(
        "INSERT INTO config_store (key, value) VALUES ('setup_complete', 'true') ON CONFLICT (key) DO UPDATE SET value = 'true'",
    )
    .execute(pool)
    .await;

    match result {
        Ok(_) => {
            let response = carnelian_common::types::SetupCompleteResponse { success: true };
            (
                StatusCode::OK,
                Json(serde_json::to_value(response).unwrap_or_default()),
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to mark setup complete");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update config: {}", e)})),
            )
                .into_response()
        }
    }
}

// =============================================================================
// SKILL BOOK HANDLERS
// =============================================================================

/// List all skills in the Skill Book catalog via `GET /v1/skill-book`.
async fn list_skill_book_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.skill_book.load_catalog() {
        Ok(catalog) => (
            StatusCode::OK,
            Json(serde_json::to_value(catalog).unwrap_or_default()),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to load Skill Book catalog");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to load catalog: {}", e)})),
            )
                .into_response()
        }
    }
}

/// Get a single Skill Book entry via `GET /v1/skill-book/:id`.
async fn get_skill_book_entry_handler(
    State(state): State<AppState>,
    Path(skill_id): Path<String>,
) -> impl IntoResponse {
    match state.skill_book.get_entry(&skill_id) {
        Ok(entry) => (
            StatusCode::OK,
            Json(serde_json::to_value(entry).unwrap_or_default()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Skill not found: {}", e)})),
        )
            .into_response(),
    }
}

/// Activate a skill from the Skill Book via `POST /v1/skill-book/:id/activate`.
async fn activate_skill_handler(
    State(state): State<AppState>,
    Path(skill_id): Path<String>,
    Json(body): Json<carnelian_common::types::ActivateSkillRequest>,
) -> impl IntoResponse {
    match state.skill_book.activate(&skill_id, body.config).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap_or_default()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("Failed to activate skill: {}", e)})),
        )
            .into_response(),
    }
}

/// Deactivate a skill via `DELETE /v1/skill-book/:id/deactivate`.
async fn deactivate_skill_handler(
    State(state): State<AppState>,
    Path(skill_id): Path<String>,
) -> impl IntoResponse {
    match state.skill_book.deactivate(&skill_id).await {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap_or_default()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("Failed to deactivate skill: {}", e)})),
        )
            .into_response(),
    }
}

// =============================================================================
// API KEY HANDLER (LOCALHOST-ONLY)
// =============================================================================

/// Get API key for remote clients via `GET /v1/config/api-key`.
/// Restricted to localhost-only access.
async fn get_api_key_handler(State(state): State<AppState>) -> impl IntoResponse {
    // Get owner signing key hash as API key
    let api_key = state
        .config
        .owner_signing_key()
        .as_ref()
        .map(|key| {
            let public_key = key.verifying_key();
            let key_bytes = public_key.as_bytes();
            format!("crn_{}", hex::encode(&key_bytes[..16]))
        })
        .unwrap_or_else(|| "not_configured".to_string());

    Json(json!({
        "api_key": api_key,
        "note": "Include this in the X-Carnelian-Key header for remote access"
    }))
}

/// Middleware layer that restricts access to localhost only.
pub async fn localhost_only<B>(
    req: axum::http::Request<B>,
    next: axum::middleware::Next<B>,
) -> impl IntoResponse {
    // Check if request is from localhost
    let remote_addr = req
        .extensions()
        .get::<std::net::SocketAddr>()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_default();

    let is_localhost =
        remote_addr == "127.0.0.1" || remote_addr == "::1" || remote_addr == "localhost";

    if !is_localhost {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "This endpoint is only accessible from localhost"
            })),
        )
            .into_response();
    }

    next.run(req).await
}
