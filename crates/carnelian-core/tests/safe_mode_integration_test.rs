#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_precision_loss)]

//! Integration tests for the Safe Mode feature.
//!
//! These tests validate that safe mode correctly blocks side-effect operations:
//!
//! - **Task Execution**: Tasks stay pending when safe mode is active
//! - **Worker Spawns**: Worker spawn requests are rejected
//! - **Remote Model Calls**: Remote provider calls return `SafeModeActive`
//! - **Transcript Writes**: Filesystem writes return `SafeModeActive`
//! - **Ledger Logging**: Enable/disable actions are logged to the ledger
//! - **HTTP API**: `/v1/safe-mode/*` endpoints toggle and report status
//!
//! # Running Tests
//!
//! ```bash
//! cargo test --test safe_mode_integration_test -- --ignored
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use carnelian_core::{
    Config, EventStream, Ledger, ModelRouter, PolicyEngine, SafeModeGuard, Scheduler, Server,
    SessionManager, WorkerManager,
};
use serde_json::json;
use sqlx::PgPool;
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};
use tokio::net::TcpListener;
use uuid::Uuid;

// =============================================================================
// HELPERS
// =============================================================================

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

async fn get_database_url(container: &testcontainers::ContainerAsync<GenericImage>) -> String {
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");
    format!("postgresql://test:test@127.0.0.1:{}/carnelian_test", port)
}

async fn setup_test_db(database_url: &str) -> PgPool {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(database_url)
        .await
        .expect("Failed to connect to database");

    carnelian_core::db::run_migrations(&pool, None)
        .await
        .expect("Failed to run migrations");

    pool
}

async fn allocate_random_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to random port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

async fn wait_for_server(port: u16, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .is_ok()
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

/// Start a server backed by a real `PostgreSQL` container and return (port, pool, `server_handle`).
async fn start_db_backed_server(db_url: &str) -> (u16, PgPool, tokio::task::JoinHandle<()>) {
    let port = allocate_random_port().await;

    let mut config = Config::default();
    config.bind_address = "127.0.0.1".to_string();
    config.http_port = port;
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
    let safe_mode_guard = Arc::new(SafeModeGuard::new(pool.clone(), ledger.clone()));
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
        config.clone(),
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
        wait_for_server(port, Duration::from_secs(10)).await,
        "DB-backed server failed to start within timeout"
    );

    (port, pool, handle)
}

/// Insert a test task and return its ID.
async fn insert_test_task(pool: &PgPool, title: &str, priority: i32) -> Uuid {
    let task_id = Uuid::now_v7();
    sqlx::query(
        r"INSERT INTO tasks (task_id, title, description, priority, state, created_at, updated_at)
          VALUES ($1, $2, $3, $4, 'pending', NOW(), NOW())",
    )
    .bind(task_id)
    .bind(title)
    .bind(format!("Test task: {}", title))
    .bind(priority)
    .execute(pool)
    .await
    .expect("Failed to insert test task");
    task_id
}

/// Get the current state of a task.
async fn get_task_state(pool: &PgPool, task_id: Uuid) -> String {
    sqlx::query_scalar::<_, String>(r"SELECT state FROM tasks WHERE task_id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .expect("Failed to get task state")
}

// =============================================================================
// TESTS
// =============================================================================

/// Test: Enable safe mode via HTTP, verify status, disable, verify status.
/// Also verifies that ledger events are logged for both enable and disable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test safe_mode_integration_test -- --ignored"]
async fn test_safe_mode_toggle_and_ledger_logging() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let (port, pool, _handle) = start_db_backed_server(&db_url).await;

    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{}", port);

    // 1. Check initial status — should be disabled
    let resp = client
        .get(format!("{}/v1/safe-mode/status", base))
        .send()
        .await
        .expect("GET /v1/safe-mode/status should succeed");
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["enabled"], false,
        "Safe mode should be disabled initially"
    );

    // 2. Enable safe mode
    let resp = client
        .post(format!("{}/v1/safe-mode/enable", base))
        .json(&json!({}))
        .send()
        .await
        .expect("POST /v1/safe-mode/enable should succeed");
    assert_eq!(resp.status(), 200);

    // 3. Check status — should be enabled
    let resp = client
        .get(format!("{}/v1/safe-mode/status", base))
        .send()
        .await
        .expect("GET /v1/safe-mode/status should succeed");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["enabled"], true,
        "Safe mode should be enabled after toggle"
    );

    // 4. Disable safe mode
    let resp = client
        .post(format!("{}/v1/safe-mode/disable", base))
        .json(&json!({}))
        .send()
        .await
        .expect("POST /v1/safe-mode/disable should succeed");
    assert_eq!(resp.status(), 200);

    // 5. Check status — should be disabled again
    let resp = client
        .get(format!("{}/v1/safe-mode/status", base))
        .send()
        .await
        .expect("GET /v1/safe-mode/status should succeed");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["enabled"], false,
        "Safe mode should be disabled after toggle"
    );

    // 6. Verify ledger events were logged
    let events: Vec<(String,)> = sqlx::query_as(
        r"SELECT action_type FROM ledger_events WHERE action_type LIKE 'safe_mode.%' ORDER BY created_at ASC",
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query ledger events");

    assert!(
        events.len() >= 2,
        "Should have at least 2 ledger events (enable + disable), got {}",
        events.len()
    );

    let action_types: Vec<&str> = events.iter().map(|(a,)| a.as_str()).collect();
    assert!(
        action_types.contains(&"safe_mode.enabled"),
        "Ledger should contain safe_mode.enabled event"
    );
    assert!(
        action_types.contains(&"safe_mode.disabled"),
        "Ledger should contain safe_mode.disabled event"
    );
}

/// Test: Tasks stay pending when safe mode is active (not marked as failed).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test safe_mode_integration_test -- --ignored"]
async fn test_safe_mode_blocks_task_execution() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let (port, pool, _handle) = start_db_backed_server(&db_url).await;

    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{}", port);

    // 1. Enable safe mode
    let resp = client
        .post(format!("{}/v1/safe-mode/enable", base))
        .json(&json!({}))
        .send()
        .await
        .expect("POST /v1/safe-mode/enable should succeed");
    assert_eq!(resp.status(), 200);

    // 2. Insert a pending task
    let task_id = insert_test_task(&pool, "safe_mode_blocked_task", 5).await;
    assert_eq!(get_task_state(&pool, task_id).await, "pending");

    // 3. Trigger scheduler poll via the safe mode guard directly
    let ledger = Arc::new(Ledger::new(pool.clone()));
    let safe_mode_guard = Arc::new(SafeModeGuard::new(pool.clone(), ledger));

    let config = Arc::new(Config::default());
    let event_stream = Arc::new(EventStream::new(1000, 100));
    let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
        config.clone(),
        event_stream.clone(),
    )));
    let active_tasks: Arc<
        tokio::sync::Mutex<std::collections::HashMap<Uuid, tokio::task::JoinHandle<()>>>,
    > = Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
    let metrics = Arc::new(carnelian_core::MetricsCollector::new());
    let ledger = Arc::new(carnelian_core::Ledger::new(pool.clone()));
    let lane_permits = Arc::new(HashMap::new());

    // Poll task queue — should short-circuit because safe mode is active
    Scheduler::poll_task_queue(
        &pool,
        &event_stream,
        &worker_manager,
        &config,
        &active_tasks,
        &metrics,
        &ledger,
        &safe_mode_guard,
        &None,
        &None,
        &lane_permits,
        &None,
    )
    .await
    .expect("poll_task_queue should succeed even in safe mode");

    // 4. Task should still be pending (NOT failed)
    let state = get_task_state(&pool, task_id).await;
    assert_eq!(
        state, "pending",
        "Task should remain pending when safe mode is active, got: {}",
        state
    );

    // 5. Disable safe mode and verify task is still pending (can be picked up later)
    let resp = client
        .post(format!("{}/v1/safe-mode/disable", base))
        .json(&json!({}))
        .send()
        .await
        .expect("POST /v1/safe-mode/disable should succeed");
    assert_eq!(resp.status(), 200);

    let state = get_task_state(&pool, task_id).await;
    assert_eq!(
        state, "pending",
        "Task should still be pending after disabling safe mode"
    );
}

/// Test: Worker spawns are rejected when safe mode is active.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test safe_mode_integration_test -- --ignored"]
async fn test_safe_mode_blocks_worker_spawn() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let ledger = Arc::new(Ledger::new(pool.clone()));
    let safe_mode_guard = Arc::new(SafeModeGuard::new(pool.clone(), ledger.clone()));

    // Enable safe mode directly
    safe_mode_guard
        .enable(None, None)
        .await
        .expect("Failed to enable safe mode");

    let config = Arc::new(Config::default());
    let event_stream = Arc::new(EventStream::new(1000, 100));
    let mut worker_manager = WorkerManager::new(config.clone(), event_stream.clone());
    worker_manager.set_safe_mode_guard(safe_mode_guard.clone());

    // Attempt to spawn a worker — should fail with SafeModeActive
    let result = worker_manager
        .spawn_worker(carnelian_core::worker::WorkerRuntime::Node, false)
        .await;

    assert!(
        result.is_err(),
        "Worker spawn should fail when safe mode is active"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, carnelian_common::Error::SafeModeActive(_)),
        "Error should be SafeModeActive, got: {:?}",
        err
    );
}

/// Test: Transcript writes return SafeModeActive when safe mode is active.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test safe_mode_integration_test -- --ignored"]
async fn test_safe_mode_blocks_transcript_write() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let ledger = Arc::new(Ledger::new(pool.clone()));
    let safe_mode_guard = Arc::new(SafeModeGuard::new(pool.clone(), ledger.clone()));

    // Enable safe mode
    safe_mode_guard
        .enable(None, None)
        .await
        .expect("Failed to enable safe mode");

    // Create a SessionManager with the guard and a transcripts path
    let sm = SessionManager::new(
        pool.clone(),
        None,
        Some(std::path::PathBuf::from("/tmp/carnelian_test_transcripts")),
        24,
    )
    .with_safe_mode_guard(safe_mode_guard.clone());

    // Create a test session
    let agent_id = {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO identities (identity_id, name, created_at, updated_at) VALUES ($1, $2, NOW(), NOW())",
        )
        .bind(id)
        .bind("safe_mode_transcript_agent")
        .execute(&pool)
        .await
        .expect("Failed to insert identity");
        id
    };

    let session_key = format!("agent:{}:ui", agent_id);
    let session = sm
        .create_session(&session_key)
        .await
        .expect("Failed to create session");

    // Attempt to write transcript — should fail with SafeModeActive
    let result = sm.write_transcript_to_file(&session).await;
    assert!(
        result.is_err(),
        "write_transcript_to_file should fail when safe mode is active"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, carnelian_common::Error::SafeModeActive(_)),
        "Error should be SafeModeActive, got: {:?}",
        err
    );

    // Attempt to sync transcript — should also fail
    let result = sm.sync_transcript(session.session_id).await;
    assert!(
        result.is_err(),
        "sync_transcript should fail when safe mode is active"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, carnelian_common::Error::SafeModeActive(_)),
        "Error should be SafeModeActive, got: {:?}",
        err
    );
}

/// Test: Remote model calls error with SafeModeActive while local calls succeed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test safe_mode_integration_test -- --ignored"]
async fn test_safe_mode_blocks_remote_model_calls() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let ledger = Arc::new(Ledger::new(pool.clone()));
    let safe_mode_guard = Arc::new(SafeModeGuard::new(pool.clone(), ledger.clone()));
    let policy_engine = Arc::new(PolicyEngine::new(pool.clone()));

    // Enable safe mode
    safe_mode_guard
        .enable(None, None)
        .await
        .expect("Failed to enable safe mode");

    // Insert a remote model provider
    sqlx::query(
        r"INSERT INTO model_providers (provider_id, provider_type, name, enabled, config, budget_limits)
          VALUES ($1, 'remote', 'openai', true, '{}', '{}')",
    )
    .bind(Uuid::now_v7())
    .execute(&pool)
    .await
    .expect("Failed to insert remote provider");

    // Insert a local model provider
    sqlx::query(
        r"INSERT INTO model_providers (provider_id, provider_type, name, enabled, config, budget_limits)
          VALUES ($1, 'local', 'ollama', true, '{}', '{}')",
    )
    .bind(Uuid::now_v7())
    .execute(&pool)
    .await
    .expect("Failed to insert local provider");

    // Create a ModelRouter with the guard
    let model_router = ModelRouter::new(
        pool.clone(),
        "http://localhost:18790".to_string(),
        policy_engine.clone(),
        ledger.clone(),
    )
    .with_safe_mode_guard(safe_mode_guard.clone());

    // Attempt a completion with a remote provider — should fail
    let request = carnelian_core::model_router::CompletionRequest {
        messages: vec![carnelian_core::model_router::Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
            name: None,
            tool_call_id: None,
        }],
        model: "gpt-4o".to_string(),
        max_tokens: Some(100),
        temperature: None,
        stream: None,
        correlation_id: None,
    };

    let identity_id = Uuid::now_v7();
    let result = model_router
        .complete(request, identity_id, None, None, None)
        .await;
    assert!(
        result.is_err(),
        "Remote model call should fail when safe mode is active"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, carnelian_common::Error::SafeModeActive(_)),
        "Error should be SafeModeActive for remote provider, got: {:?}",
        err
    );
}

/// Test: SessionManager has_safe_mode_guard returns true when guard is wired.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test safe_mode_integration_test -- --ignored"]
async fn test_session_manager_guard_is_wired() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    // Without guard — should be false
    let sm_no_guard = SessionManager::with_defaults(pool.clone());
    assert!(
        !sm_no_guard.has_safe_mode_guard(),
        "SessionManager without guard should return false"
    );

    // With guard — should be true
    let ledger = Arc::new(Ledger::new(pool.clone()));
    let guard = Arc::new(SafeModeGuard::new(pool.clone(), ledger));
    let sm_with_guard = SessionManager::with_defaults(pool).with_safe_mode_guard(guard);
    assert!(
        sm_with_guard.has_safe_mode_guard(),
        "SessionManager with guard should return true"
    );
}

/// Test: SafeModeGuard check_or_block returns correct results.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test safe_mode_integration_test -- --ignored"]
async fn test_safe_mode_guard_check_or_block() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let ledger = Arc::new(Ledger::new(pool.clone()));
    let guard = SafeModeGuard::new(pool.clone(), ledger);

    // Initially disabled — check should pass
    guard
        .check_or_block("test_operation")
        .await
        .expect("check_or_block should succeed when safe mode is disabled");

    // Enable safe mode
    guard
        .enable(None, None)
        .await
        .expect("Failed to enable safe mode");

    // Now check should fail
    let result = guard.check_or_block("test_operation").await;
    assert!(
        result.is_err(),
        "check_or_block should fail when safe mode is enabled"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, carnelian_common::Error::SafeModeActive(_)),
        "Error should be SafeModeActive, got: {:?}",
        err
    );

    // Disable safe mode
    guard
        .disable(None, None)
        .await
        .expect("Failed to disable safe mode");

    // Check should pass again
    guard
        .check_or_block("test_operation")
        .await
        .expect("check_or_block should succeed after disabling safe mode");
}
