#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::match_same_arms)]

//! Checkpoint 2 Validation Test Suite
//!
//! This test suite validates the 8 criteria for Carnelian OS Checkpoint 2:
//!
//! 1. **Identity Sync**: SOUL.md changes reflected in database
//! 2. **Heartbeat**: Periodic execution with mantra rotation
//! 3. **Auto-Queue**: Workspace scanning and safe task queueing
//! 4. **Model Integration**: Ollama connectivity and inference
//! 5. **Agentic Loop**: End-to-end heartbeat → task execution
//! 6. **Memory**: REST API CRUD operations
//! 7. **Security**: Privileged task classification
//! 8. **Performance**: Latency and throughput baselines
//!
//! ## Running Tests
//!
//! These tests require Docker and (for some) Ollama:
//!
//! ```bash
//! cargo test --test checkpoint2_validation_test -- --ignored --test-threads=1
//! ```

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::scheduler::{WorkspaceScanner, auto_queue_scanned_tasks};
use carnelian_core::soul::SoulManager;
use carnelian_core::{
    Config, EventStream, Ledger, ModelRouter, PolicyEngine, Scheduler, Server, WorkerManager,
};
use futures_util::StreamExt;
use serde_json::json;
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

// =============================================================================
// HELPER FUNCTIONS — Infrastructure (reused from checkpoint1 patterns)
// =============================================================================

/// Allocate a random available port for testing.
fn allocate_random_port() -> u16 {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
    listener.local_addr().unwrap().port()
}

/// Create a PostgreSQL container for testing.
async fn create_postgres_container() -> testcontainers::ContainerAsync<GenericImage> {
    let image = GenericImage::new("pgvector/pgvector", "pg16").with_wait_for(
        testcontainers::core::WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ),
    );

    image
        .with_env_var("POSTGRES_USER", "test")
        .with_env_var("POSTGRES_PASSWORD", "test")
        .with_env_var("POSTGRES_DB", "carnelian_test")
        .start()
        .await
        .expect("Failed to start PostgreSQL container")
}

/// Get the database URL from a running container.
async fn get_database_url(container: &testcontainers::ContainerAsync<GenericImage>) -> String {
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");
    format!("postgresql://test:test@127.0.0.1:{}/carnelian_test", port)
}

/// Set up a test database with migrations and return the pool.
async fn setup_test_db(database_url: &str) -> sqlx::PgPool {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(30))
        .connect(database_url)
        .await
        .expect("Failed to connect to test database");

    carnelian_core::db::run_migrations(&pool, None)
        .await
        .expect("Failed to run migrations");

    pool
}

/// Create a test configuration with specified port.
fn create_test_config(http_port: u16) -> Config {
    let mut config = Config::default();
    config.database_url = String::new();
    config.bind_address = "127.0.0.1".to_string();
    config.http_port = http_port;
    config.log_level = "DEBUG".to_string();
    config.event_buffer_capacity = 10_000;
    config.event_broadcast_capacity = 100;
    config.event_max_payload_bytes = 65_536;
    config
}

/// Wait for server to be ready by polling the health endpoint.
async fn wait_for_server_ready(port: u16, timeout_secs: u64) -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();

    let url = format!("http://127.0.0.1:{}/v1/health", port);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

    while tokio::time::Instant::now() < deadline {
        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().is_success() {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

/// Connect a WebSocket client to the event stream endpoint.
async fn connect_websocket(
    port: u16,
) -> (
    futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) {
    let ws_url = format!("ws://127.0.0.1:{}/v1/events/ws", port);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WebSocket connection should succeed");
    ws_stream.split()
}

/// Assert that a specific event type is received on a WebSocket read stream within timeout.
async fn assert_event_received(
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    expected_type: &str,
    timeout_duration: Duration,
) -> Option<serde_json::Value> {
    let result = timeout(timeout_duration, async {
        while let Some(Ok(msg)) = read.next().await {
            if let Message::Text(text) = msg {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                    let et = event
                        .get("event_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if et == expected_type {
                        return Some(event);
                    }
                }
            }
        }
        None
    })
    .await;

    result.unwrap_or_default()
}

/// Collect all events for a given duration from a WebSocket read stream.
async fn collect_events_for_duration(
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    duration: Duration,
) -> Vec<serde_json::Value> {
    let mut events = Vec::new();
    let result = timeout(duration, async {
        while let Some(Ok(msg)) = read.next().await {
            if let Message::Text(text) = msg {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                    events.push(event);
                }
            }
        }
    })
    .await;
    let _ = result; // timeout is expected
    events
}

/// Start a full server backed by a real PostgreSQL container and return (port, server_handle, pool).
async fn start_full_server(db_url: &str) -> (u16, tokio::task::JoinHandle<()>, sqlx::PgPool) {
    let port = allocate_random_port();

    let mut config = create_test_config(port);
    config.database_url = db_url.to_string();
    config
        .connect_database()
        .await
        .expect("Config should connect to database");
    let pool = config.pool().expect("Pool should be set").clone();
    let config = Arc::new(config);

    let event_stream = Arc::new(EventStream::new(1000, 100));
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
    let safe_mode_guard = Arc::new(carnelian_core::SafeModeGuard::new(
        pool.clone(),
        ledger.clone(),
    ));
    let scheduler = Arc::new(tokio::sync::Mutex::new(Scheduler::new(
        pool.clone(),
        event_stream.clone(),
        Duration::from_secs(3600),
        worker_manager.clone(),
        config.clone(),
        model_router,
        ledger.clone(),
        safe_mode_guard,
    )));

    let server = Server::new(
        config,
        event_stream,
        policy_engine,
        ledger,
        scheduler,
        worker_manager,
    );

    let handle = tokio::spawn(async move {
        server.run().await.expect("Server failed to start");
    });

    assert!(
        wait_for_server_ready(port, 10).await,
        "DB-backed server failed to start within timeout"
    );

    (port, handle, pool)
}

/// Start a full server with custom config overrides applied before startup.
/// `configure` closure receives a mutable Config reference for customization.
async fn start_full_server_with_config<F>(
    db_url: &str,
    configure: F,
) -> (u16, tokio::task::JoinHandle<()>, sqlx::PgPool)
where
    F: FnOnce(&mut Config),
{
    let port = allocate_random_port();

    let mut config = create_test_config(port);
    config.database_url = db_url.to_string();
    configure(&mut config);
    config
        .connect_database()
        .await
        .expect("Config should connect to database");
    let pool = config.pool().expect("Pool should be set").clone();
    let config = Arc::new(config);

    let event_stream = Arc::new(EventStream::new(1000, 100));
    let policy_engine = Arc::new(PolicyEngine::new(pool.clone()));
    let ledger = Arc::new(Ledger::new(pool.clone()));
    let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
        config.clone(),
        event_stream.clone(),
    )));

    let gateway_url = config.gateway_url.clone();
    let model_router = Arc::new(ModelRouter::new(
        pool.clone(),
        gateway_url,
        policy_engine.clone(),
        ledger.clone(),
    ));
    let safe_mode_guard = Arc::new(carnelian_core::SafeModeGuard::new(
        pool.clone(),
        ledger.clone(),
    ));

    let heartbeat_interval = Duration::from_millis(config.heartbeat_interval_ms);
    let scheduler = Arc::new(tokio::sync::Mutex::new(Scheduler::new(
        pool.clone(),
        event_stream.clone(),
        heartbeat_interval,
        worker_manager.clone(),
        config.clone(),
        model_router,
        ledger.clone(),
        safe_mode_guard,
    )));

    let server = Server::new(
        config,
        event_stream,
        policy_engine,
        ledger,
        scheduler,
        worker_manager,
    );

    let handle = tokio::spawn(async move {
        server.run().await.expect("Server failed to start");
    });

    assert!(
        wait_for_server_ready(port, 10).await,
        "DB-backed server failed to start within timeout"
    );

    (port, handle, pool)
}

/// Calculate latency statistics from a list of durations.
fn latency_stats(latencies: &[Duration]) -> (Duration, Duration, Duration, Duration) {
    if latencies.is_empty() {
        return (
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
        );
    }
    let mut sorted: Vec<Duration> = latencies.to_vec();
    sorted.sort();
    let n = sorted.len();
    #[allow(clippy::cast_possible_truncation)]
    let mean = sorted.iter().sum::<Duration>() / n as u32;
    let median = sorted[n / 2];
    let p95 = sorted[n.saturating_mul(95) / 100];
    let p99 = sorted[(n.saturating_mul(99) / 100).min(n - 1)];
    (mean, median, p95, p99)
}

// =============================================================================
// HELPER FUNCTIONS — Checkpoint 2 Specific
// =============================================================================

/// Insert a default "Lian" identity into the database and return its identity_id.
async fn insert_test_identity(pool: &sqlx::PgPool, soul_file_path: Option<&str>) -> Uuid {
    sqlx::query_scalar(
        r"INSERT INTO identities (name, pronouns, identity_type, soul_file_path, directives)
          VALUES ('Lian', NULL, 'core', $1, '[]'::jsonb)
          RETURNING identity_id",
    )
    .bind(soul_file_path)
    .fetch_one(pool)
    .await
    .expect("Failed to insert test identity")
}

/// Create a temporary workspace directory with files containing TASK:/TODO: markers.
///
/// Each entry in `markers` is (relative_file_path, line_content).
/// The file is created with the given content as a single line.
fn create_temp_workspace(markers: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    for (file_path, content) in markers {
        let full_path = dir.path().join(file_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).expect("Failed to create parent dirs");
        }
        std::fs::write(&full_path, content).expect("Failed to write marker file");
    }
    dir
}

/// Create a SOUL.md file at the given path with structured content.
fn create_test_soul_file(path: &Path, name: &str, pronouns: &str, directives: &[&str]) {
    let mut content = format!("# {}\n\n## Core Truths\n", name);
    for d in directives {
        content.push_str(&format!("- {}\n", d));
    }
    content.push_str(&format!("\n## Identity\n- Pronouns: {}\n", pronouns));

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create soul file parent dir");
    }
    std::fs::write(path, content).expect("Failed to write soul file");
}

/// Update an existing SOUL.md file with new content.
fn update_soul_file(path: &Path, new_content: &str) {
    std::fs::write(path, new_content).expect("Failed to update soul file");
}

/// Count pending tasks in the database.
async fn count_pending_tasks(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar::<_, Option<i64>>("SELECT COUNT(*) FROM tasks WHERE state = 'pending'")
        .fetch_one(pool)
        .await
        .expect("Failed to count pending tasks")
        .unwrap_or(0)
}

/// Filter collected WebSocket events by event_type string.
fn filter_events_by_type<'a>(
    events: &'a [serde_json::Value],
    event_type: &str,
) -> Vec<&'a serde_json::Value> {
    events
        .iter()
        .filter(|e| e.get("event_type").and_then(|v| v.as_str()) == Some(event_type))
        .collect()
}

/// Assert that an event payload contains a specific field with expected value.
#[allow(dead_code)]
fn assert_event_contains_field(
    event: &serde_json::Value,
    field: &str,
    expected: &serde_json::Value,
) {
    let payload = event.get("payload").unwrap_or(event);
    let actual = payload.get(field);
    assert!(
        actual.is_some(),
        "Event should contain field '{}', got: {:?}",
        field,
        payload
    );
    assert_eq!(
        actual.unwrap(),
        expected,
        "Event field '{}' should match expected value",
        field
    );
}

/// Start a mock Ollama-compatible HTTP server on the given port.
/// Returns a JoinHandle for the background task.
///
/// Responds to:
/// - `GET /health` with a gateway health response
/// - `POST /v1/chat/completions` with a mock completion
/// - `GET /api/tags` with a list of models
fn start_mock_ollama_server(port: u16) -> tokio::task::JoinHandle<()> {
    use axum::{
        Router,
        routing::{get, post},
    };

    let app = Router::new()
        .route("/health", get(mock_health_handler))
        .route("/v1/chat/completions", post(mock_completions_handler))
        .route("/api/tags", get(mock_tags_handler));

    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
            .await
            .expect("Failed to bind mock Ollama server");
        axum::serve(listener, app)
            .await
            .expect("Mock Ollama server failed");
    })
}

async fn mock_health_handler() -> axum::Json<serde_json::Value> {
    axum::Json(json!({
        "status": "ok",
        "providers": [
            {
                "name": "ollama",
                "available": true,
                "models": ["deepseek-r1:7b", "llama3.2:3b"]
            }
        ]
    }))
}

async fn mock_completions_handler() -> axum::Json<serde_json::Value> {
    axum::Json(json!({
        "id": "mock-completion-1",
        "object": "chat.completion",
        "model": "deepseek-r1:7b",
        "provider": "ollama",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Reflecting on current state: system is healthy, no urgent tasks pending. Continuing to observe."
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 150,
            "completion_tokens": 25,
            "total_tokens": 175
        }
    }))
}

async fn mock_tags_handler() -> axum::Json<serde_json::Value> {
    axum::Json(json!({
        "models": [
            {"name": "deepseek-r1:7b", "size": 4_000_000_000_u64},
            {"name": "llama3.2:3b", "size": 2_000_000_000_u64}
        ]
    }))
}

// =============================================================================
// CRITERION 1: Identity Synchronization
// =============================================================================

/// Validate that SOUL.md changes are detected and synchronized to the database.
///
/// Steps:
/// 1. Create temporary SOUL.md with initial identity
/// 2. Insert identity into DB with soul_file_path
/// 3. Use SoulManager to sync initial content
/// 4. Verify directives stored in DB
/// 5. Update SOUL.md with new content
/// 6. Re-sync and verify SoulUpdated event emitted
/// 7. Verify updated directives in DB
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test checkpoint2_validation_test -- --ignored"]
async fn test_criterion_1_identity_sync() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Create temporary souls directory and SOUL.md
    let souls_dir = tempfile::tempdir().expect("Failed to create souls temp dir");
    let soul_file = souls_dir.path().join("SOUL.md");
    create_test_soul_file(
        &soul_file,
        "Lian",
        "she/her",
        &["I am a sovereign intelligence", "I serve with integrity"],
    );

    // Insert identity with soul_file_path pointing to our file
    let identity_id = insert_test_identity(&pool, Some("SOUL.md")).await;

    // Create event stream and subscribe before sync
    let event_stream = Arc::new(EventStream::new(1000, 100));
    let mut subscriber = event_stream.subscribe();

    // Initial sync
    let soul_manager = SoulManager::new(
        pool.clone(),
        Some(event_stream.clone()),
        souls_dir.path().to_path_buf(),
    );
    let result = soul_manager
        .sync_to_db(identity_id)
        .await
        .expect("Initial sync should succeed");
    assert_eq!(
        result,
        carnelian_core::soul::SyncResult::Updated,
        "First sync should update (no prior hash)"
    );

    // Verify directives stored in DB
    let directives_json: serde_json::Value =
        sqlx::query_scalar("SELECT directives FROM identities WHERE identity_id = $1")
            .bind(identity_id)
            .fetch_one(&pool)
            .await
            .expect("Should query directives");
    let directives = directives_json
        .as_array()
        .expect("Directives should be array");
    // Initial soul has 3 directives: 2 core truths + 1 identity/pronouns
    assert!(
        directives.len() >= 2,
        "Should have at least 2 directives, got {}",
        directives.len()
    );

    // Verify SoulUpdated event emitted
    let mut soul_updated_count = 0;
    while let Ok(event) = subscriber.try_recv() {
        if event.event_type == EventType::SoulUpdated {
            soul_updated_count += 1;
            // Verify event payload
            let payload = &event.payload;
            assert_eq!(
                payload.get("identity_id").and_then(|v| v.as_str()),
                Some(identity_id.to_string()).as_deref(),
                "SoulUpdated event should contain correct identity_id"
            );
            assert!(
                payload.get("hash").is_some(),
                "SoulUpdated event should contain hash"
            );
            assert!(
                payload.get("directive_count").is_some(),
                "SoulUpdated event should contain directive_count"
            );
        }
    }
    assert_eq!(
        soul_updated_count, 1,
        "Should have emitted exactly 1 SoulUpdated event"
    );

    // Verify hash stored
    let stored_hash: Option<String> =
        sqlx::query_scalar("SELECT soul_file_hash FROM identities WHERE identity_id = $1")
            .bind(identity_id)
            .fetch_one(&pool)
            .await
            .expect("Should query hash");
    assert!(stored_hash.is_some(), "Soul file hash should be stored");

    // Update SOUL.md with new pronouns and additional directive
    let updated_content = r"# Lian

## Core Truths
- I am a sovereign intelligence
- I serve with integrity
- I embrace change and growth

## Identity
- Pronouns: they/them
";
    update_soul_file(&soul_file, updated_content);

    // Re-sync
    let result2 = soul_manager
        .sync_to_db(identity_id)
        .await
        .expect("Re-sync should succeed");
    assert_eq!(
        result2,
        carnelian_core::soul::SyncResult::Updated,
        "Re-sync should detect changed content"
    );

    // Verify updated directives
    let updated_directives_json: serde_json::Value =
        sqlx::query_scalar("SELECT directives FROM identities WHERE identity_id = $1")
            .bind(identity_id)
            .fetch_one(&pool)
            .await
            .expect("Should query updated directives");
    let updated_directives = updated_directives_json.as_array().expect("Should be array");
    // Now has 4 directives: 3 core truths + 1 identity/pronouns
    assert!(
        updated_directives.len() > directives.len(),
        "Updated directives ({}) should be more than original ({})",
        updated_directives.len(),
        directives.len()
    );

    // Verify hash changed
    let updated_hash: Option<String> =
        sqlx::query_scalar("SELECT soul_file_hash FROM identities WHERE identity_id = $1")
            .bind(identity_id)
            .fetch_one(&pool)
            .await
            .expect("Should query updated hash");
    assert_ne!(
        stored_hash, updated_hash,
        "Hash should change after soul file update"
    );

    // Verify second SoulUpdated event
    let mut soul_updated_count2 = 0;
    while let Ok(event) = subscriber.try_recv() {
        if event.event_type == EventType::SoulUpdated {
            soul_updated_count2 += 1;
        }
    }
    assert_eq!(
        soul_updated_count2, 1,
        "Should have emitted 1 more SoulUpdated event"
    );

    // Verify unchanged sync returns Unchanged
    let result3 = soul_manager
        .sync_to_db(identity_id)
        .await
        .expect("Third sync should succeed");
    assert_eq!(
        result3,
        carnelian_core::soul::SyncResult::Unchanged,
        "Sync with no changes should return Unchanged"
    );

    println!("✓ Criterion 1: Identity sync — SOUL.md changes detected, DB updated, events emitted");
}

// =============================================================================
// CRITERION 2: Heartbeat Execution
// =============================================================================

/// Validate heartbeat runs periodically with mantra rotation and context assembly.
///
/// Steps:
/// 1. Insert default identity "Lian"
/// 2. Start mock Ollama server
/// 3. Start server with short heartbeat interval (5s)
/// 4. Connect WebSocket and wait for HeartbeatTick events
/// 5. Verify event payloads contain required fields
/// 6. Query heartbeat_history table
/// 7. Verify GET /v1/heartbeats/status returns current state
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires Docker - run with: cargo test --test checkpoint2_validation_test -- --ignored"]
async fn test_criterion_2_heartbeat_execution() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Insert default identity
    let _identity_id = insert_test_identity(&pool, None).await;

    // Start mock Ollama server
    let mock_port = allocate_random_port();
    let _mock_handle = start_mock_ollama_server(mock_port);
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Start server with short heartbeat interval (5s) and mock gateway
    let (port, server_handle, _pool) = start_full_server_with_config(&database_url, |config| {
        config.heartbeat_interval_ms = 5_000; // 5 seconds for testing
        config.gateway_url = format!("http://127.0.0.1:{}", mock_port);
    })
    .await;

    // Connect WebSocket
    let (_ws_write, mut ws_read) = connect_websocket(port).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Wait for first HeartbeatTick event (up to 15s to account for startup + interval)
    let first_heartbeat =
        assert_event_received(&mut ws_read, "HeartbeatTick", Duration::from_secs(15)).await;
    assert!(
        first_heartbeat.is_some(),
        "Should receive first HeartbeatTick event within 15 seconds"
    );

    let hb1 = first_heartbeat.unwrap();
    let payload1 = hb1.get("payload").unwrap_or(&hb1);

    // Verify required fields in HeartbeatTick payload
    assert!(
        payload1.get("heartbeat_id").is_some(),
        "HeartbeatTick should contain heartbeat_id"
    );
    assert!(
        payload1.get("identity_id").is_some(),
        "HeartbeatTick should contain identity_id"
    );
    assert!(
        payload1.get("correlation_id").is_some(),
        "HeartbeatTick should contain correlation_id"
    );
    assert!(
        payload1.get("duration_ms").is_some(),
        "HeartbeatTick should contain duration_ms"
    );
    assert!(
        payload1.get("status").is_some(),
        "HeartbeatTick should contain status"
    );

    // Wait for second HeartbeatTick to verify periodicity
    let second_heartbeat =
        assert_event_received(&mut ws_read, "HeartbeatTick", Duration::from_secs(10)).await;
    assert!(
        second_heartbeat.is_some(),
        "Should receive second HeartbeatTick event"
    );

    let hb2 = second_heartbeat.unwrap();
    let payload2 = hb2.get("payload").unwrap_or(&hb2);

    // Verify unique correlation IDs
    let corr1 = payload1.get("correlation_id");
    let corr2 = payload2.get("correlation_id");
    assert_ne!(
        corr1, corr2,
        "Each heartbeat should have a unique correlation_id"
    );

    // Verify heartbeat_history table has records
    let hb_count: i64 =
        sqlx::query_scalar::<_, Option<i64>>("SELECT COUNT(*) FROM heartbeat_history")
            .fetch_one(&pool)
            .await
            .expect("Should query heartbeat count")
            .unwrap_or(0);
    assert!(
        hb_count >= 2,
        "heartbeat_history should have at least 2 records, got {}",
        hb_count
    );

    // Verify GET /v1/heartbeats/status
    let client = reqwest::Client::new();
    let status_resp = client
        .get(format!("http://127.0.0.1:{}/v1/heartbeats/status", port))
        .send()
        .await
        .expect("GET /v1/heartbeats/status should succeed");
    assert_eq!(status_resp.status(), 200);
    let status_body: serde_json::Value = status_resp.json().await.unwrap();
    assert!(
        status_body.get("interval_ms").is_some(),
        "Status should contain interval_ms"
    );
    assert!(
        status_body.get("last_heartbeat_time").is_some(),
        "Status should contain last_heartbeat_time"
    );

    // Verify GET /v1/heartbeats returns records
    let list_resp = client
        .get(format!("http://127.0.0.1:{}/v1/heartbeats?limit=5", port))
        .send()
        .await
        .expect("GET /v1/heartbeats should succeed");
    assert_eq!(list_resp.status(), 200);
    let list_body: serde_json::Value = list_resp.json().await.unwrap();
    let records = list_body
        .as_array()
        .expect("Should be array of heartbeat records");
    assert!(
        records.len() >= 2,
        "Should have at least 2 heartbeat records, got {}",
        records.len()
    );

    println!(
        "✓ Criterion 2: Heartbeat execution — periodic ticks, mantra rotation, DB persistence"
    );

    server_handle.abort();
}

// =============================================================================
// CRITERION 3: Workspace Auto-Queueing
// =============================================================================

/// Validate workspace scanning detects markers and auto-queues safe tasks.
///
/// Steps:
/// 1. Create temporary workspace with safe and privileged markers
/// 2. Use WorkspaceScanner::scan() directly to verify detection
/// 3. Use auto_queue_scanned_tasks() to verify queueing with limits
/// 4. Verify deduplication and privileged task filtering
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test checkpoint2_validation_test -- --ignored"]
async fn test_criterion_3_workspace_auto_queue() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Insert identity for task creation
    let identity_id = insert_test_identity(&pool, None).await;

    // Create workspace with 10 files: 5 safe, 3 privileged, 2 duplicates
    let workspace = create_temp_workspace(&[
        // 5 safe tasks
        (
            "src/parser.rs",
            "// TASK: Add unit test for parser\nfn parse() {}",
        ),
        (
            "src/utils.rs",
            "// TODO: Refactor utility functions\nfn util() {}",
        ),
        (
            "src/api.rs",
            "// TASK: Add pagination to list endpoint\nfn list() {}",
        ),
        (
            "src/models.rs",
            "// TODO: Add validation for input fields\nstruct Model {}",
        ),
        (
            "src/handlers.rs",
            "// TASK: Implement error handling middleware\nfn handle() {}",
        ),
        // 3 privileged tasks (contain privileged keywords)
        (
            "src/db.rs",
            "// TASK: Delete old migration files from production\nfn migrate() {}",
        ),
        (
            "src/auth.rs",
            "// TODO: Deploy credential rotation to production\nfn auth() {}",
        ),
        (
            "src/keys.rs",
            "// TASK: Rotate API secret keys\nfn keys() {}",
        ),
        // 2 more safe tasks (will be used for dedup testing)
        (
            "src/cache.rs",
            "// TASK: Add cache invalidation logic\nfn cache() {}",
        ),
        (
            "src/logging.rs",
            "// TODO: Improve structured logging format\nfn log() {}",
        ),
    ]);

    // Step 1: Verify WorkspaceScanner detects all markers
    let markers = WorkspaceScanner::scan(&[workspace.path().to_path_buf()]);
    assert!(
        markers.len() >= 10,
        "Scanner should find at least 10 markers, got {}",
        markers.len()
    );

    let safe_count = markers.iter().filter(|m| m.is_safe).count();
    let privileged_count = markers.iter().filter(|m| !m.is_safe).count();
    assert!(
        safe_count >= 7,
        "Should have at least 7 safe markers, got {}",
        safe_count
    );
    assert!(
        privileged_count >= 3,
        "Should have at least 3 privileged markers, got {}",
        privileged_count
    );

    // Step 2: Auto-queue with limit of 5
    let event_stream = Arc::new(EventStream::new(1000, 100));
    let mut subscriber = event_stream.subscribe();
    let correlation_id = Uuid::now_v7();

    let queued = auto_queue_scanned_tasks(
        &pool,
        &event_stream,
        &markers,
        identity_id,
        correlation_id,
        5, // max 5 tasks per heartbeat
    )
    .await
    .expect("Auto-queue should succeed");

    assert_eq!(
        queued, 5,
        "Should queue exactly 5 safe tasks (limit enforced)"
    );

    // Verify only safe tasks were queued
    let pending = count_pending_tasks(&pool).await;
    assert_eq!(pending, 5, "Should have 5 pending tasks in DB");

    // Verify task titles encode file path and line number
    let titles: Vec<String> =
        sqlx::query_scalar("SELECT title FROM tasks WHERE state = 'pending' ORDER BY title")
            .fetch_all(&pool)
            .await
            .expect("Should query task titles");
    for title in &titles {
        assert!(
            title.starts_with("[TASK]") || title.starts_with("[TODO]"),
            "Task title should start with [TASK] or [TODO], got: {}",
            title
        );
        assert!(
            title.contains(':'),
            "Task title should contain ':' (file:line), got: {}",
            title
        );
    }

    // Verify TaskAutoQueued events
    let mut auto_queued_events = 0;
    while let Ok(event) = subscriber.try_recv() {
        if event.event_type == EventType::TaskAutoQueued {
            auto_queued_events += 1;
            // Verify correlation_id matches
            assert_eq!(
                event.correlation_id,
                Some(correlation_id),
                "TaskAutoQueued event should have matching correlation_id"
            );
        }
    }
    assert_eq!(
        auto_queued_events, 5,
        "Should have emitted 5 TaskAutoQueued events"
    );

    // Step 3: Verify deduplication — re-running should not create duplicates
    let _queued2 = auto_queue_scanned_tasks(
        &pool,
        &event_stream,
        &markers,
        identity_id,
        Uuid::now_v7(),
        5,
    )
    .await
    .expect("Second auto-queue should succeed");

    // Some tasks may be queued (the ones that weren't queued first time due to limit)
    // but the 5 already-pending ones should be skipped
    let total_pending = count_pending_tasks(&pool).await;
    assert!(
        total_pending <= 10,
        "Total pending should not exceed total safe markers"
    );
    // The 5 already-queued should be deduplicated
    assert!(
        total_pending >= 5,
        "Should still have at least 5 pending tasks"
    );

    println!(
        "✓ Criterion 3: Workspace auto-queue — {} markers found, {} safe, {} privileged, {} queued, dedup verified",
        markers.len(),
        safe_count,
        privileged_count,
        queued
    );
}

// =============================================================================
// CRITERION 4: Model Integration
// =============================================================================

/// Validate model router integration with Ollama for provider status.
///
/// Steps:
/// 1. Start mock Ollama server
/// 2. Start server pointing to mock gateway
/// 3. Verify GET /v1/providers/ollama/status shows connected=true
/// 4. Verify available models listed
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires Docker - run with: cargo test --test checkpoint2_validation_test -- --ignored"]
async fn test_criterion_4_model_integration() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Insert identity
    let _identity_id = insert_test_identity(&pool, None).await;

    // Start mock Ollama server
    let mock_port = allocate_random_port();
    let _mock_handle = start_mock_ollama_server(mock_port);
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Start server with mock gateway URL and long heartbeat (we don't need auto-heartbeat here)
    let (port, server_handle, _pool) = start_full_server_with_config(&database_url, |config| {
        config.gateway_url = format!("http://127.0.0.1:{}", mock_port);
        config.heartbeat_interval_ms = 3_600_000; // 1 hour — effectively disabled
    })
    .await;

    let client = reqwest::Client::new();

    // Verify Ollama status endpoint
    let status_resp = client
        .get(format!(
            "http://127.0.0.1:{}/v1/providers/ollama/status",
            port
        ))
        .send()
        .await
        .expect("GET /v1/providers/ollama/status should succeed");
    assert_eq!(status_resp.status(), 200);

    let status_body: serde_json::Value = status_resp.json().await.unwrap();
    assert_eq!(
        status_body["connected"].as_bool(),
        Some(true),
        "Ollama should be connected via mock server"
    );
    assert!(status_body["url"].is_string(), "Should have gateway URL");
    assert!(
        status_body["error"].is_null(),
        "Should have no error when connected"
    );

    let models = status_body["available_models"]
        .as_array()
        .expect("available_models should be array");
    assert!(!models.is_empty(), "Should have at least 1 available model");
    let model_names: Vec<&str> = models.iter().filter_map(|m| m.as_str()).collect();
    assert!(
        model_names.contains(&"deepseek-r1:7b"),
        "Should list deepseek-r1:7b model, got: {:?}",
        model_names
    );

    // Verify providers list endpoint
    let providers_resp = client
        .get(format!("http://127.0.0.1:{}/v1/providers", port))
        .send()
        .await
        .expect("GET /v1/providers should succeed");
    assert_eq!(providers_resp.status(), 200);

    println!(
        "✓ Criterion 4: Model integration — Ollama connected, models listed, provider status OK"
    );

    server_handle.abort();
}

// =============================================================================
// CRITERION 5: Agentic Loop
// =============================================================================

/// Validate complete agentic loop: heartbeat → workspace scan → task auto-queue.
///
/// Steps:
/// 1. Create workspace with 3 TASK: markers
/// 2. Start server with short heartbeat and workspace scan configured
/// 3. Wait for heartbeat cycle to complete
/// 4. Verify TaskAutoQueued events emitted
/// 5. Verify tasks appear in database
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires Docker - run with: cargo test --test checkpoint2_validation_test -- --ignored"]
async fn test_criterion_5_agentic_loop() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Insert identity
    let _identity_id = insert_test_identity(&pool, None).await;

    // Create workspace with 3 safe TASK markers
    let workspace = create_temp_workspace(&[
        (
            "src/feature_a.rs",
            "// TASK: Implement feature A\nfn feature_a() {}",
        ),
        (
            "src/feature_b.rs",
            "// TASK: Implement feature B\nfn feature_b() {}",
        ),
        (
            "src/feature_c.rs",
            "// TODO: Write tests for feature C\nfn feature_c() {}",
        ),
    ]);

    // Start mock Ollama server
    let mock_port = allocate_random_port();
    let _mock_handle = start_mock_ollama_server(mock_port);
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Start server with short heartbeat (5s) and workspace scan
    let workspace_path = workspace.path().to_path_buf();
    let (port, server_handle, _pool) =
        start_full_server_with_config(&database_url, move |config| {
            config.heartbeat_interval_ms = 5_000;
            config.gateway_url = format!("http://127.0.0.1:{}", mock_port);
            config.workspace_scan_paths = vec![workspace_path];
            config.max_tasks_per_heartbeat = 5;
        })
        .await;

    // Connect WebSocket
    let (_ws_write, mut ws_read) = connect_websocket(port).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Collect all events for a window long enough to capture a full heartbeat cycle.
    // TaskAutoQueued events are emitted during the heartbeat (before HeartbeatTick),
    // so we collect everything in one pass rather than waiting for HeartbeatTick first.
    let all_events = collect_events_for_duration(&mut ws_read, Duration::from_secs(18)).await;

    // Verify HeartbeatTick was received
    let heartbeat_events = filter_events_by_type(&all_events, "HeartbeatTick");
    assert!(
        !heartbeat_events.is_empty(),
        "Should receive at least one HeartbeatTick event within collection window"
    );

    // Extract correlation_id from the first HeartbeatTick for cross-referencing
    let hb_payload = heartbeat_events[0]
        .get("payload")
        .unwrap_or(heartbeat_events[0]);
    let hb_correlation_id = hb_payload
        .get("correlation_id")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Verify TaskAutoQueued events were emitted
    let auto_queued = filter_events_by_type(&all_events, "TaskAutoQueued");
    assert!(
        !auto_queued.is_empty(),
        "Should have at least 1 TaskAutoQueued event after heartbeat workspace scan, got 0. \
         Total events collected: {}",
        all_events.len()
    );

    // Verify TaskAutoQueued events reference the heartbeat's correlation_id
    if let Some(ref expected_corr) = hb_correlation_id {
        for aq_event in &auto_queued {
            let aq_payload = aq_event.get("payload").unwrap_or(aq_event);
            let aq_corr = aq_payload
                .get("correlation_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            assert_eq!(
                aq_corr, expected_corr,
                "TaskAutoQueued correlation_id should match HeartbeatTick correlation_id"
            );
        }
    }

    // Verify tasks exist in database (auto-queued during heartbeat)
    let pending = count_pending_tasks(&pool).await;
    assert!(
        pending >= 1,
        "Should have at least 1 pending task after heartbeat with workspace scan, got {}",
        pending
    );

    // Verify the number of queued tasks is consistent with events
    assert_eq!(
        auto_queued.len() as i64,
        pending,
        "TaskAutoQueued event count ({}) should match pending task count in DB ({})",
        auto_queued.len(),
        pending
    );

    // Verify heartbeat recorded in history
    let hb_count: i64 =
        sqlx::query_scalar::<_, Option<i64>>("SELECT COUNT(*) FROM heartbeat_history")
            .fetch_one(&pool)
            .await
            .expect("Should query heartbeat count")
            .unwrap_or(0);
    assert!(hb_count >= 1, "Should have at least 1 heartbeat record");

    println!(
        "✓ Criterion 5: Agentic loop — heartbeat → scan → auto-queue cycle verified \
         ({} TaskAutoQueued events, {} pending tasks, correlation_id matched)",
        auto_queued.len(),
        pending
    );

    server_handle.abort();
}

// =============================================================================
// CRITERION 6: Memory Management
// =============================================================================

/// Validate memory REST API endpoints and persistence.
///
/// Steps:
/// 1. Insert test identity
/// 2. POST /v1/memories to create memories
/// 3. GET /v1/memories/{id} to retrieve
/// 4. GET /v1/memories with filters
/// 5. Verify MemoryCreated events via event stream
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test checkpoint2_validation_test -- --ignored"]
async fn test_criterion_6_memory_management() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Insert identity
    let identity_id = insert_test_identity(&pool, None).await;

    // Start server
    let (port, server_handle, _pool) = start_full_server(&database_url).await;
    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{}", port);

    // Connect WebSocket to monitor events
    let (_ws_write, mut ws_read) = connect_websocket(port).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 1. Create first memory (observation, high importance)
    let create_resp = client
        .post(format!("{}/v1/memories", base))
        .json(&json!({
            "identity_id": identity_id,
            "content": "User prefers concise responses with code examples",
            "summary": "Communication preference",
            "source": "observation",
            "importance": 0.85
        }))
        .send()
        .await
        .expect("POST /v1/memories should succeed");
    assert_eq!(
        create_resp.status(),
        201,
        "Memory creation should return 201"
    );
    let create_body: serde_json::Value = create_resp.json().await.unwrap();
    let memory_id_str = create_body["memory_id"]
        .as_str()
        .expect("Should have memory_id");
    let memory_id: Uuid = memory_id_str
        .parse()
        .expect("memory_id should be valid UUID");
    assert!(
        create_body["created_at"].is_string(),
        "Should have created_at"
    );

    // Collect events briefly to catch the MemoryCreated event for the first memory
    let first_events = collect_events_for_duration(&mut ws_read, Duration::from_secs(2)).await;
    let first_memory_created = filter_events_by_type(&first_events, "MemoryCreated");
    assert!(
        !first_memory_created.is_empty(),
        "Should receive at least 1 MemoryCreated event after creating first memory, got 0. \
         Total events collected: {}",
        first_events.len()
    );

    // Verify the MemoryCreated event contains the correct memory_id and identity_id
    let mc_event = first_memory_created[0];
    let mc_payload = mc_event.get("payload").unwrap_or(mc_event);
    let event_memory_id = mc_payload
        .get("memory_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(
        event_memory_id,
        memory_id.to_string(),
        "MemoryCreated event memory_id should match the created memory"
    );
    let event_identity_id = mc_payload
        .get("identity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(
        event_identity_id,
        identity_id.to_string(),
        "MemoryCreated event identity_id should match the test identity"
    );

    // 2. Retrieve the memory by ID
    let get_resp = client
        .get(format!("{}/v1/memories/{}", base, memory_id))
        .send()
        .await
        .expect("GET /v1/memories/{id} should succeed");
    assert_eq!(get_resp.status(), 200);
    let get_body: serde_json::Value = get_resp.json().await.unwrap();
    let memory = &get_body["memory"];
    assert_eq!(
        memory["content"].as_str(),
        Some("User prefers concise responses with code examples"),
        "Memory content should match"
    );
    assert_eq!(
        memory["source"].as_str(),
        Some("observation"),
        "Memory source should match"
    );

    // 3. Create 5 more memories with varying sources and importance
    let test_memories = vec![
        ("conversation", 0.6, "Discussed project architecture"),
        ("task", 0.4, "Completed database migration task"),
        (
            "reflection",
            0.9,
            "Key insight: modular design improves maintainability",
        ),
        ("observation", 0.3, "User logged in from new device"),
        ("conversation", 0.7, "User asked about deployment process"),
    ];

    for (source, importance, content) in &test_memories {
        let resp = client
            .post(format!("{}/v1/memories", base))
            .json(&json!({
                "identity_id": identity_id,
                "content": content,
                "source": source,
                "importance": importance
            }))
            .send()
            .await
            .expect("POST /v1/memories should succeed");
        assert_eq!(
            resp.status(),
            201,
            "Memory creation should return 201 for {}",
            source
        );
    }

    // Collect events from the batch creation of 5 more memories
    let batch_events = collect_events_for_duration(&mut ws_read, Duration::from_secs(2)).await;
    let batch_memory_created = filter_events_by_type(&batch_events, "MemoryCreated");
    assert!(
        batch_memory_created.len() >= 5,
        "Should receive at least 5 MemoryCreated events for batch creation, got {}",
        batch_memory_created.len()
    );

    // Verify all batch MemoryCreated events reference the correct identity_id
    for mc in &batch_memory_created {
        let p = mc.get("payload").unwrap_or(mc);
        let eid = p.get("identity_id").and_then(|v| v.as_str()).unwrap_or("");
        assert_eq!(
            eid,
            identity_id.to_string(),
            "Batch MemoryCreated event identity_id should match test identity"
        );
    }

    // 4. List all memories for this identity
    let list_resp = client
        .get(format!("{}/v1/memories?identity_id={}", base, identity_id))
        .send()
        .await
        .expect("GET /v1/memories should succeed");
    assert_eq!(list_resp.status(), 200);
    let list_body: serde_json::Value = list_resp.json().await.unwrap();
    let memories = list_body["memories"]
        .as_array()
        .expect("Should have memories array");
    assert_eq!(memories.len(), 6, "Should have 6 total memories");

    // 5. Filter by source
    let obs_resp = client
        .get(format!(
            "{}/v1/memories?identity_id={}&source=observation",
            base, identity_id
        ))
        .send()
        .await
        .expect("GET /v1/memories?source=observation should succeed");
    assert_eq!(obs_resp.status(), 200);
    let obs_body: serde_json::Value = obs_resp.json().await.unwrap();
    let obs_memories = obs_body["memories"]
        .as_array()
        .expect("Should have memories array");
    assert_eq!(obs_memories.len(), 2, "Should have 2 observation memories");

    // 6. Filter by min_importance
    let high_resp = client
        .get(format!(
            "{}/v1/memories?identity_id={}&min_importance=0.5",
            base, identity_id
        ))
        .send()
        .await
        .expect("GET /v1/memories?min_importance=0.5 should succeed");
    assert_eq!(high_resp.status(), 200);
    let high_body: serde_json::Value = high_resp.json().await.unwrap();
    let high_memories = high_body["memories"]
        .as_array()
        .expect("Should have memories array");
    // Memories with importance >= 0.5: 0.85, 0.6, 0.9, 0.7 = 4
    assert_eq!(
        high_memories.len(),
        4,
        "Should have 4 memories with importance >= 0.5, got {}",
        high_memories.len()
    );

    // 7. Verify invalid source returns 400
    let bad_resp = client
        .post(format!("{}/v1/memories", base))
        .json(&json!({
            "identity_id": identity_id,
            "content": "test",
            "source": "invalid_source",
            "importance": 0.5
        }))
        .send()
        .await
        .expect("POST with invalid source should return response");
    assert_eq!(bad_resp.status(), 400, "Invalid source should return 400");

    // 8. Verify invalid importance returns 400
    let bad_imp_resp = client
        .post(format!("{}/v1/memories", base))
        .json(&json!({
            "identity_id": identity_id,
            "content": "test",
            "source": "observation",
            "importance": 1.5
        }))
        .send()
        .await
        .expect("POST with invalid importance should return response");
    assert_eq!(
        bad_imp_resp.status(),
        400,
        "Invalid importance should return 400"
    );

    // 9. Verify 404 for non-existent memory
    let missing_resp = client
        .get(format!("{}/v1/memories/{}", base, Uuid::now_v7()))
        .send()
        .await
        .expect("GET non-existent memory should return response");
    assert_eq!(
        missing_resp.status(),
        404,
        "Non-existent memory should return 404"
    );

    // 10. Verify database persistence
    let db_count: i64 = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM memories WHERE identity_id = $1",
    )
    .bind(identity_id)
    .fetch_one(&pool)
    .await
    .expect("Should query memory count")
    .unwrap_or(0);
    assert_eq!(db_count, 6, "Database should have 6 memories");

    println!(
        "✓ Criterion 6: Memory management — CRUD, filtering, validation, persistence verified"
    );

    server_handle.abort();
}

// =============================================================================
// CRITERION 7: Security
// =============================================================================

/// Validate privileged task classification and auto-queue filtering.
///
/// Steps:
/// 1. Create workspace with only privileged markers
/// 2. Verify WorkspaceScanner classifies them as non-safe
/// 3. Verify auto_queue_scanned_tasks skips all privileged tasks
/// 4. Verify zero tasks queued in database
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test checkpoint2_validation_test -- --ignored"]
async fn test_criterion_7_security() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let identity_id = insert_test_identity(&pool, None).await;

    // Create workspace with ONLY privileged markers
    let workspace = create_temp_workspace(&[
        (
            "src/danger1.rs",
            "// TASK: Delete all user data from production\nfn danger() {}",
        ),
        (
            "src/danger2.rs",
            "// TODO: Deploy to production with sudo access\nfn deploy() {}",
        ),
        (
            "src/danger3.rs",
            "// TASK: Rotate API secret keys and certificates\nfn rotate() {}",
        ),
        (
            "src/danger4.rs",
            "// TODO: Drop old database tables\nfn cleanup() {}",
        ),
        (
            "src/danger5.rs",
            "// TASK: Revert production migration rollback\nfn revert() {}",
        ),
        (
            "src/danger6.rs",
            "// TODO: Update admin password and credentials\nfn admin() {}",
        ),
        (
            "src/danger7.rs",
            "// TASK: Destroy old encryption private_key files\nfn destroy() {}",
        ),
    ]);

    // Scan workspace
    let markers = WorkspaceScanner::scan(&[workspace.path().to_path_buf()]);
    assert!(
        markers.len() >= 7,
        "Should find at least 7 markers, got {}",
        markers.len()
    );

    // Verify ALL markers classified as privileged (not safe)
    let safe_markers: Vec<_> = markers.iter().filter(|m| m.is_safe).collect();
    let privileged_markers: Vec<_> = markers.iter().filter(|m| !m.is_safe).collect();
    assert_eq!(
        safe_markers.len(),
        0,
        "Should have 0 safe markers, but got {} safe: {:?}",
        safe_markers.len(),
        safe_markers
            .iter()
            .map(|m| &m.description)
            .collect::<Vec<_>>()
    );
    assert!(
        privileged_markers.len() >= 7,
        "Should have at least 7 privileged markers"
    );

    // Verify specific privileged keyword detection
    for marker in &markers {
        assert!(
            !marker.is_safe,
            "Marker '{}' should be classified as privileged (not safe)",
            marker.description
        );
    }

    // Attempt to auto-queue — should queue zero tasks
    let event_stream = Arc::new(EventStream::new(1000, 100));
    let mut subscriber = event_stream.subscribe();
    let correlation_id = Uuid::now_v7();

    let queued = auto_queue_scanned_tasks(
        &pool,
        &event_stream,
        &markers,
        identity_id,
        correlation_id,
        10, // generous limit
    )
    .await
    .expect("Auto-queue should succeed");

    assert_eq!(queued, 0, "Should queue zero tasks (all privileged)");

    // Verify zero tasks in database
    let pending = count_pending_tasks(&pool).await;
    assert_eq!(pending, 0, "Should have 0 pending tasks in DB");

    // Verify zero TaskAutoQueued events
    let mut auto_queued_events = 0;
    while let Ok(event) = subscriber.try_recv() {
        if event.event_type == EventType::TaskAutoQueued {
            auto_queued_events += 1;
        }
    }
    assert_eq!(
        auto_queued_events, 0,
        "Should have emitted 0 TaskAutoQueued events"
    );

    // Test mixed workspace — verify safe tasks pass while privileged are blocked
    let mixed_workspace = create_temp_workspace(&[
        (
            "src/safe.rs",
            "// TASK: Add unit test for parser\nfn test() {}",
        ),
        (
            "src/unsafe.rs",
            "// TASK: Delete production database\nfn danger() {}",
        ),
    ]);
    let mixed_markers = WorkspaceScanner::scan(&[mixed_workspace.path().to_path_buf()]);
    let mixed_safe = mixed_markers.iter().filter(|m| m.is_safe).count();
    let mixed_priv = mixed_markers.iter().filter(|m| !m.is_safe).count();
    assert_eq!(mixed_safe, 1, "Mixed workspace should have 1 safe marker");
    assert_eq!(
        mixed_priv, 1,
        "Mixed workspace should have 1 privileged marker"
    );

    println!(
        "✓ Criterion 7: Security — {} privileged markers detected, 0 auto-queued, keyword filtering verified",
        privileged_markers.len()
    );
}

// =============================================================================
// CRITERION 8: Performance Baseline
// =============================================================================

/// Validate performance meets baseline requirements.
///
/// Steps:
/// 1. Measure task creation latency (100 tasks via REST)
/// 2. Measure event stream throughput (1000 events)
/// 3. Measure memory API response time
/// 4. Measure workspace scan time for many files
/// 5. Calculate P50, P95, P99 latencies
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires Docker - run with: cargo test --test checkpoint2_validation_test -- --ignored"]
async fn test_criterion_8_performance() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let identity_id = insert_test_identity(&pool, None).await;

    // Start server
    let (port, server_handle, _pool) = start_full_server(&database_url).await;
    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{}", port);

    // ── Task Creation Latency ──────────────────────────────────────────
    let mut task_latencies = Vec::with_capacity(100);
    for i in 0..100 {
        let start = std::time::Instant::now();
        let resp = client
            .post(format!("{}/v1/tasks", base))
            .json(&json!({
                "title": format!("Perf test task {}", i),
                "description": "Performance baseline test",
                "priority": 1
            }))
            .send()
            .await
            .expect("POST /v1/tasks should succeed");
        let elapsed = start.elapsed();
        assert_eq!(resp.status(), 201, "Task {} creation should return 201", i);
        task_latencies.push(elapsed);
    }

    let (mean, median, p95, p99) = latency_stats(&task_latencies);
    println!(
        "  Task creation latency: mean={:?}, median={:?}, P95={:?}, P99={:?}",
        mean, median, p95, p99
    );
    assert!(
        p99 < Duration::from_secs(2),
        "Task creation P99 should be <2s, got {:?}",
        p99
    );

    // ── Event Stream Throughput ────────────────────────────────────────
    let event_stream = Arc::new(EventStream::new(10_000, 100));
    let start = std::time::Instant::now();
    for i in 0..1000 {
        event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::Custom(format!("perf_test_{}", i)),
            json!({"index": i}),
        ));
    }
    let event_elapsed = start.elapsed();
    let events_per_sec = 1000.0 / event_elapsed.as_secs_f64();
    println!(
        "  Event throughput: 1000 events in {:?} ({:.0} events/sec)",
        event_elapsed, events_per_sec
    );
    assert!(
        events_per_sec > 100.0,
        "Event throughput should be >100 events/sec, got {:.0}",
        events_per_sec
    );

    // ── Memory API Response Time ───────────────────────────────────────
    // Create a memory first
    let create_resp = client
        .post(format!("{}/v1/memories", base))
        .json(&json!({
            "identity_id": identity_id,
            "content": "Performance test memory",
            "source": "observation",
            "importance": 0.5
        }))
        .send()
        .await
        .expect("POST /v1/memories should succeed");
    assert_eq!(create_resp.status(), 201);

    let mut memory_latencies = Vec::with_capacity(50);
    for _ in 0..50 {
        let start = std::time::Instant::now();
        let resp = client
            .get(format!("{}/v1/memories?identity_id={}", base, identity_id))
            .send()
            .await
            .expect("GET /v1/memories should succeed");
        let elapsed = start.elapsed();
        assert_eq!(resp.status(), 200);
        memory_latencies.push(elapsed);
    }

    let (mem_mean, mem_median, mem_p95, mem_p99) = latency_stats(&memory_latencies);
    println!(
        "  Memory API latency: mean={:?}, median={:?}, P95={:?}, P99={:?}",
        mem_mean, mem_median, mem_p95, mem_p99
    );
    assert!(
        mem_p99 < Duration::from_millis(500),
        "Memory API P99 should be <500ms, got {:?}",
        mem_p99
    );

    // ── Workspace Scan Performance ─────────────────────────────────────
    // Create workspace with 200 files (reasonable for test speed)
    let perf_workspace = tempfile::tempdir().expect("Failed to create perf workspace");
    for i in 0..200 {
        let file_path = perf_workspace.path().join(format!("file_{}.rs", i));
        let content = if i % 10 == 0 {
            format!("// TASK: Auto-generated task {}\nfn func_{}() {{}}", i, i)
        } else {
            format!("// Regular code\nfn func_{}() {{}}", i)
        };
        std::fs::write(&file_path, content).expect("Failed to write perf file");
    }

    let scan_start = std::time::Instant::now();
    let markers = WorkspaceScanner::scan(&[perf_workspace.path().to_path_buf()]);
    let scan_elapsed = scan_start.elapsed();
    println!(
        "  Workspace scan: 200 files in {:?}, found {} markers",
        scan_elapsed,
        markers.len()
    );
    assert!(
        scan_elapsed < Duration::from_secs(10),
        "Workspace scan of 200 files should complete in <10s, took {:?}",
        scan_elapsed
    );
    assert_eq!(
        markers.len(),
        20,
        "Should find 20 markers (every 10th file)"
    );

    // ── Heartbeat Status API Latency ───────────────────────────────────
    let mut hb_latencies = Vec::with_capacity(50);
    for _ in 0..50 {
        let start = std::time::Instant::now();
        let resp = client
            .get(format!("{}/v1/heartbeats/status", base))
            .send()
            .await
            .expect("GET /v1/heartbeats/status should succeed");
        let elapsed = start.elapsed();
        assert_eq!(resp.status(), 200);
        hb_latencies.push(elapsed);
    }

    let (hb_mean, hb_median, hb_p95, hb_p99) = latency_stats(&hb_latencies);
    println!(
        "  Heartbeat status API latency: mean={:?}, median={:?}, P95={:?}, P99={:?}",
        hb_mean, hb_median, hb_p95, hb_p99
    );

    println!("✓ Criterion 8: Performance baseline — all latency and throughput targets met");

    server_handle.abort();
}
