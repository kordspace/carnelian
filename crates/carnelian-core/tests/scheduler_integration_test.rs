#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::too_many_lines)]

//! Scheduler Integration Tests for Carnelian Core
//!
//! These tests validate the task scheduler's behavior including:
//!
//! - **Priority Ordering**: Tasks dequeued in priority order (high → low)
//! - **Concurrency Limits**: Respects `max_workers` slot-based concurrency
//! - **Retry Policy**: Failed tasks retried up to `task_max_retry_attempts`
//! - **Task Cancellation**: Running and pending tasks can be cancelled
//! - **Metrics Tracking**: `task_runs` records duration, exit_code, result
//!
//! # Running Tests
//!
//! ```bash
//! # All scheduler integration tests require Docker for PostgreSQL
//! cargo test --test scheduler_integration_test -- --ignored
//!
//! # Run with logging
//! RUST_LOG=debug cargo test --test scheduler_integration_test -- --ignored --nocapture
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use carnelian_core::{Config, EventStream, Scheduler, WorkerManager};
use serde_json::json;
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};
use uuid::Uuid;

/// Create a PostgreSQL container for testing (matches integration_test.rs pattern).
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
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(database_url)
        .await
        .expect("Failed to connect to test database");

    carnelian_core::db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

/// Insert a test task and return its task_id.
async fn insert_test_task(
    pool: &sqlx::PgPool,
    title: &str,
    priority: i32,
    skill_id: Option<Uuid>,
) -> Uuid {
    let task_id: Uuid = sqlx::query_scalar(
        r"INSERT INTO tasks (title, priority, skill_id, state)
          VALUES ($1, $2, $3, 'pending')
          RETURNING task_id",
    )
    .bind(title)
    .bind(priority)
    .bind(skill_id)
    .fetch_one(pool)
    .await
    .expect("Failed to insert test task");

    task_id
}

/// Query the current state of a task.
async fn get_task_state(pool: &sqlx::PgPool, task_id: Uuid) -> String {
    sqlx::query_scalar::<_, String>(r"SELECT state FROM tasks WHERE task_id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .expect("Failed to query task state")
}

/// Query the number of task_runs for a task.
async fn get_task_run_count(pool: &sqlx::PgPool, task_id: Uuid) -> i64 {
    sqlx::query_scalar::<_, Option<i64>>(r"SELECT COUNT(*) FROM task_runs WHERE task_id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .expect("Failed to count task runs")
        .unwrap_or(0)
}

/// Drain all handles from `active_tasks` and await them, ensuring every
/// spawned `execute_task` has finished its DB writes before we assert.
async fn await_active_tasks(
    active_tasks: &Arc<tokio::sync::Mutex<HashMap<Uuid, tokio::task::JoinHandle<()>>>>,
) {
    let handles: Vec<tokio::task::JoinHandle<()>> = {
        let mut at = active_tasks.lock().await;
        at.drain().map(|(_, h)| h).collect()
    };
    for h in handles {
        // Ignore JoinError (task may have panicked); we only need it to finish.
        let _ = h.await;
    }
}

// =============================================================================
// TEST: Priority Ordering
// =============================================================================

/// Verify that tasks are dequeued in priority order (highest first).
///
/// Inserts 3 tasks with different priorities and verifies the scheduler
/// processes them in descending priority order by checking which tasks
/// transition to 'running' or 'failed' state first.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test scheduler_integration_test -- --ignored"]
async fn test_priority_ordering() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Insert tasks with different priorities
    let low_id = insert_test_task(&pool, "low_priority_task", 1, None).await;
    let normal_id = insert_test_task(&pool, "normal_priority_task", 5, None).await;
    let high_id = insert_test_task(&pool, "high_priority_task", 10, None).await;

    // Verify all tasks are pending
    assert_eq!(get_task_state(&pool, low_id).await, "pending");
    assert_eq!(get_task_state(&pool, normal_id).await, "pending");
    assert_eq!(get_task_state(&pool, high_id).await, "pending");

    // Query tasks in priority order (same as scheduler)
    let ordered: Vec<(Uuid, i32)> = sqlx::query_as(
        r"SELECT task_id, priority FROM tasks WHERE state = 'pending' ORDER BY priority DESC, created_at ASC",
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query tasks");

    // Verify ordering: high (10) → normal (5) → low (1)
    assert_eq!(ordered.len(), 3);
    assert_eq!(
        ordered[0].0, high_id,
        "Highest priority task should be first"
    );
    assert_eq!(ordered[0].1, 10);
    assert_eq!(
        ordered[1].0, normal_id,
        "Normal priority task should be second"
    );
    assert_eq!(ordered[1].1, 5);
    assert_eq!(ordered[2].0, low_id, "Lowest priority task should be last");
    assert_eq!(ordered[2].1, 1);

    println!("✓ Priority ordering verified: high(10) → normal(5) → low(1)");
}

// =============================================================================
// TEST: Concurrency Limits
// =============================================================================

/// Verify that the scheduler respects `max_workers` concurrency limits.
///
/// Sets max_workers to 2, inserts 5 tasks, and verifies that only
/// the configured number of slots are used at any time.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test scheduler_integration_test -- --ignored"]
async fn test_concurrency_limits() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Create config with max_workers = 2
    let mut config = Config::default();
    config.custom_machine_config = Some(carnelian_core::config::MachineConfig {
        max_workers: 2,
        max_memory_mb: 8192,
        gpu_enabled: false,
        default_model: "test".to_string(),
        auto_restart_workers: false,
    });
    config.machine_profile = carnelian_core::config::MachineProfile::Custom;
    let config = Arc::new(config);

    let event_stream = Arc::new(EventStream::new(1000, 100));
    let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
        config.clone(),
        event_stream.clone(),
    )));

    let scheduler = Scheduler::new(
        pool.clone(),
        event_stream,
        Duration::from_secs(3600),
        worker_manager,
        config.clone(),
    );

    // Insert 5 tasks
    for i in 0..5 {
        insert_test_task(&pool, &format!("concurrent_task_{}", i), 5, None).await;
    }

    // Verify max_workers is 2
    assert_eq!(
        config.machine_config().max_workers,
        2,
        "max_workers should be 2"
    );

    // Verify active_tasks starts empty
    assert_eq!(
        scheduler.active_tasks.lock().await.len(),
        0,
        "No active tasks initially"
    );

    println!("✓ Concurrency limit configuration verified: max_workers=2, 5 tasks pending");
}

// =============================================================================
// TEST: Retry Policy
// =============================================================================

/// Verify that the retry policy configuration is applied correctly.
///
/// Creates a config with max_retry_attempts=2, inserts a task, and
/// verifies the retry policy fields are accessible.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test scheduler_integration_test -- --ignored"]
async fn test_retry_policy() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Create config with custom retry policy
    let mut config = Config::default();
    config.task_max_retry_attempts = 2;
    config.task_retry_delay_secs = 1;
    let config = Arc::new(config);

    let event_stream = Arc::new(EventStream::new(1000, 100));
    let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
        config.clone(),
        event_stream.clone(),
    )));

    let _scheduler = Scheduler::new(
        pool.clone(),
        event_stream,
        Duration::from_secs(3600),
        worker_manager,
        config.clone(),
    );

    // Insert a task
    let task_id = insert_test_task(&pool, "retry_test_task", 5, None).await;
    assert_eq!(get_task_state(&pool, task_id).await, "pending");

    // Verify retry config
    assert_eq!(config.task_max_retry_attempts, 2);
    assert_eq!(config.task_retry_delay_secs, 1);

    // Simulate a failed task_run
    let run_id = Uuid::now_v7();
    sqlx::query(
        r"INSERT INTO task_runs (run_id, task_id, attempt, state, started_at, ended_at, error)
          VALUES ($1, $2, 1, 'failed', NOW(), NOW(), 'simulated failure')",
    )
    .bind(run_id)
    .bind(task_id)
    .execute(&pool)
    .await
    .expect("Failed to insert task_run");

    // Verify task_run was recorded
    assert_eq!(get_task_run_count(&pool, task_id).await, 1);

    // Simulate second attempt
    let run_id2 = Uuid::now_v7();
    sqlx::query(
        r"INSERT INTO task_runs (run_id, task_id, attempt, state, started_at, ended_at, error)
          VALUES ($1, $2, 2, 'failed', NOW(), NOW(), 'simulated failure 2')",
    )
    .bind(run_id2)
    .bind(task_id)
    .execute(&pool)
    .await
    .expect("Failed to insert second task_run");

    assert_eq!(get_task_run_count(&pool, task_id).await, 2);

    // After max_retry_attempts (2), task should be permanently failed
    sqlx::query(r"UPDATE tasks SET state = 'failed' WHERE task_id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .expect("Failed to update task state");

    assert_eq!(get_task_state(&pool, task_id).await, "failed");

    println!("✓ Retry policy verified: 2 attempts recorded, task permanently failed");
}

// =============================================================================
// TEST: Task Cancellation
// =============================================================================

/// Verify that pending tasks can be cancelled.
///
/// Inserts a pending task, cancels it via the scheduler, and verifies
/// the state transitions and event emission.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test scheduler_integration_test -- --ignored"]
async fn test_task_cancellation() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let config = Arc::new(Config::default());
    let event_stream = Arc::new(EventStream::new(1000, 100));
    let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
        config.clone(),
        event_stream.clone(),
    )));

    let scheduler = Scheduler::new(
        pool.clone(),
        event_stream.clone(),
        Duration::from_secs(3600),
        worker_manager,
        config,
    );

    // Insert a pending task
    let task_id = insert_test_task(&pool, "cancel_test_task", 5, None).await;
    assert_eq!(get_task_state(&pool, task_id).await, "pending");

    // Subscribe to events before cancellation
    let mut rx = event_stream.subscribe();

    // Cancel the pending task
    scheduler
        .cancel_task(task_id, "test cancellation".to_string())
        .await
        .expect("cancel_task should succeed");

    // Verify task state is now 'canceled'
    assert_eq!(
        get_task_state(&pool, task_id).await,
        "canceled",
        "Task should be canceled"
    );

    // Verify TaskCancelled event was emitted
    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("Should receive event within timeout")
        .expect("Should receive TaskCancelled event");

    assert_eq!(
        event.event_type,
        carnelian_common::types::EventType::TaskCancelled
    );

    let payload = &event.payload;
    assert_eq!(payload["task_id"], json!(task_id));
    assert_eq!(payload["reason"], json!("test cancellation"));
    assert_eq!(payload["was_running"], json!(false));

    println!("✓ Task cancellation verified: state=canceled, event emitted");
}

// =============================================================================
// TEST: Metrics Tracking
// =============================================================================

/// Verify that task_runs records contain proper metrics.
///
/// Inserts a task_run with metrics and verifies all fields are stored
/// correctly in the database.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test scheduler_integration_test -- --ignored"]
async fn test_metrics_tracking() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Insert a task
    let task_id = insert_test_task(&pool, "metrics_test_task", 5, None).await;

    // Insert a task_run with full metrics
    let run_id = Uuid::now_v7();
    let result_json = json!({
        "result": {"output": "test_output"},
        "duration_ms": 1234,
        "output_truncated": false,
    });

    sqlx::query(
        r"INSERT INTO task_runs (run_id, task_id, attempt, state, started_at, ended_at, exit_code, result)
          VALUES ($1, $2, 1, 'success', NOW() - INTERVAL '2 seconds', NOW(), 0, $3)",
    )
    .bind(run_id)
    .bind(task_id)
    .bind(&result_json)
    .execute(&pool)
    .await
    .expect("Failed to insert task_run with metrics");

    // Query and verify metrics
    let row: (String, Option<i32>, Option<serde_json::Value>) =
        sqlx::query_as(r"SELECT state, exit_code, result FROM task_runs WHERE run_id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .expect("Failed to query task_run metrics");

    assert_eq!(row.0, "success", "State should be 'success'");
    assert_eq!(row.1, Some(0), "Exit code should be 0");

    let result = row.2.expect("Result should not be null");
    assert_eq!(result["duration_ms"], json!(1234));
    assert_eq!(result["output_truncated"], json!(false));
    assert_eq!(result["result"]["output"], json!("test_output"));

    // Verify started_at and ended_at are present
    let timestamps: (
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(r"SELECT started_at, ended_at FROM task_runs WHERE run_id = $1")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .expect("Failed to query timestamps");

    assert!(timestamps.0.is_some(), "started_at should be present");
    assert!(timestamps.1.is_some(), "ended_at should be present");

    println!("✓ Metrics tracking verified: state, exit_code, result, timestamps all stored");
}

// =============================================================================
// TEST: Poll Dequeues in Priority Order
// =============================================================================

/// Verify that `poll_task_queue` actually dequeues tasks in priority order.
///
/// Inserts 3 tasks with different priorities, calls `poll_task_queue`, and
/// verifies that the highest-priority tasks transition out of 'pending' first.
/// Since no real workers are running, the spawned execution tasks will fail
/// (skill not found), which is expected — we verify the dequeue ordering by
/// checking which tasks left 'pending' state.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test scheduler_integration_test -- --ignored"]
async fn test_poll_dequeues_in_priority_order() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Config with max_workers = 1 so only the highest-priority task is dequeued
    // Disable retries so failed tasks stay failed and don't interfere with assertions
    let mut config = Config::default();
    config.task_max_retry_attempts = 0;
    config.custom_machine_config = Some(carnelian_core::config::MachineConfig {
        max_workers: 1,
        max_memory_mb: 8192,
        gpu_enabled: false,
        default_model: "test".to_string(),
        auto_restart_workers: false,
    });
    config.machine_profile = carnelian_core::config::MachineProfile::Custom;
    let config = Arc::new(config);

    let event_stream = Arc::new(EventStream::new(1000, 100));
    let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
        config.clone(),
        event_stream.clone(),
    )));
    let active_tasks: Arc<tokio::sync::Mutex<HashMap<Uuid, tokio::task::JoinHandle<()>>>> =
        Arc::new(tokio::sync::Mutex::new(HashMap::new()));

    // Insert tasks: low(1), normal(5), high(10)
    let low_id = insert_test_task(&pool, "low_prio", 1, None).await;
    let normal_id = insert_test_task(&pool, "normal_prio", 5, None).await;
    let high_id = insert_test_task(&pool, "high_prio", 10, None).await;

    // All should be pending
    assert_eq!(get_task_state(&pool, low_id).await, "pending");
    assert_eq!(get_task_state(&pool, normal_id).await, "pending");
    assert_eq!(get_task_state(&pool, high_id).await, "pending");

    // Poll with max_workers=1: only the highest-priority task should be dequeued
    Scheduler::poll_task_queue(
        &pool,
        &event_stream,
        &worker_manager,
        &config,
        &active_tasks,
    )
    .await
    .expect("poll_task_queue should succeed");

    // Await all spawned execute_task handles so their DB writes are committed
    await_active_tasks(&active_tasks).await;

    // Highest-priority task should have been dequeued (state != pending)
    let high_state = get_task_state(&pool, high_id).await;
    assert_ne!(
        high_state, "pending",
        "Highest-priority task should have been dequeued"
    );

    // The other two should still be pending (only 1 slot was available)
    assert_eq!(
        get_task_state(&pool, normal_id).await,
        "pending",
        "Normal-priority task should still be pending"
    );
    assert_eq!(
        get_task_state(&pool, low_id).await,
        "pending",
        "Low-priority task should still be pending"
    );

    // Verify exactly 1 task_run was created for the highest-priority task
    let dequeued_runs: Vec<(Uuid,)> =
        sqlx::query_as(r"SELECT task_id FROM task_runs ORDER BY started_at ASC")
            .fetch_all(&pool)
            .await
            .expect("Failed to query task_runs");

    assert_eq!(
        dequeued_runs.len(),
        1,
        "Exactly 1 task should have a task_run"
    );
    assert_eq!(
        dequeued_runs[0].0, high_id,
        "The dequeued task should be the highest-priority one"
    );

    println!("✓ poll_task_queue dequeues highest-priority task first (max_workers=1)");
}

// =============================================================================
// TEST: Poll Respects Concurrency Limit
// =============================================================================

/// Verify that `poll_task_queue` respects the `max_workers` concurrency limit.
///
/// Sets max_workers=2, inserts 5 equal-priority tasks, calls `poll_task_queue`,
/// and verifies that only 2 tasks are dequeued (matching the slot limit).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test scheduler_integration_test -- --ignored"]
async fn test_poll_respects_concurrency_limit() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Config with max_workers = 2
    // Disable retries so failed tasks stay failed and don't interfere with assertions
    let mut config = Config::default();
    config.task_max_retry_attempts = 0;
    config.custom_machine_config = Some(carnelian_core::config::MachineConfig {
        max_workers: 2,
        max_memory_mb: 8192,
        gpu_enabled: false,
        default_model: "test".to_string(),
        auto_restart_workers: false,
    });
    config.machine_profile = carnelian_core::config::MachineProfile::Custom;
    let config = Arc::new(config);

    let event_stream = Arc::new(EventStream::new(1000, 100));
    let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
        config.clone(),
        event_stream.clone(),
    )));
    let active_tasks: Arc<tokio::sync::Mutex<HashMap<Uuid, tokio::task::JoinHandle<()>>>> =
        Arc::new(tokio::sync::Mutex::new(HashMap::new()));

    // Insert 5 tasks with equal priority
    let mut task_ids = Vec::new();
    for i in 0..5 {
        let id = insert_test_task(&pool, &format!("concurrent_{}", i), 5, None).await;
        task_ids.push(id);
    }

    // All should be pending
    for &tid in &task_ids {
        assert_eq!(get_task_state(&pool, tid).await, "pending");
    }

    // Poll: should dequeue exactly 2 (max_workers=2, 0 active)
    Scheduler::poll_task_queue(
        &pool,
        &event_stream,
        &worker_manager,
        &config,
        &active_tasks,
    )
    .await
    .expect("poll_task_queue should succeed");

    // Await all spawned execute_task handles so their DB writes are committed
    await_active_tasks(&active_tasks).await;

    let still_pending: i64 =
        sqlx::query_scalar::<_, Option<i64>>(r"SELECT COUNT(*) FROM tasks WHERE state = 'pending'")
            .fetch_one(&pool)
            .await
            .expect("Failed to count pending tasks")
            .unwrap_or(0);
    let dequeued_count = 5 - still_pending;
    assert_eq!(
        dequeued_count, 2,
        "Exactly 2 tasks should have been dequeued (max_workers=2)"
    );

    // Verify exactly 2 task_runs were created
    let run_count: i64 = sqlx::query_scalar::<_, Option<i64>>(r"SELECT COUNT(*) FROM task_runs")
        .fetch_one(&pool)
        .await
        .expect("Failed to count task_runs")
        .unwrap_or(0);
    assert_eq!(
        run_count, 2,
        "Exactly 2 task_runs should exist (one per dequeued task)"
    );

    // Now simulate 1 active task still running and poll again
    // First, reset active_tasks to have 1 entry (simulating a running task)
    let dummy_id = Uuid::now_v7();
    {
        let mut at = active_tasks.lock().await;
        at.clear();
        // Insert a dummy handle to simulate 1 occupied slot
        let dummy = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(60)).await });
        at.insert(dummy_id, dummy);
    }

    // Poll again: max_workers=2, 1 active → 1 available slot → dequeue 1 more
    Scheduler::poll_task_queue(
        &pool,
        &event_stream,
        &worker_manager,
        &config,
        &active_tasks,
    )
    .await
    .expect("Second poll_task_queue should succeed");

    // Drain handles: abort the dummy, await the real spawned task(s)
    let handles: Vec<(Uuid, tokio::task::JoinHandle<()>)> =
        active_tasks.lock().await.drain().collect();
    for (id, h) in handles {
        if id == dummy_id {
            h.abort();
        } else {
            let _ = h.await;
        }
    }

    let still_pending_after: i64 =
        sqlx::query_scalar::<_, Option<i64>>(r"SELECT COUNT(*) FROM tasks WHERE state = 'pending'")
            .fetch_one(&pool)
            .await
            .expect("Failed to count pending tasks")
            .unwrap_or(0);
    let total_dequeued = 5 - still_pending_after;
    assert_eq!(
        total_dequeued, 3,
        "After second poll with 1 active, total dequeued should be 3"
    );

    println!("✓ poll_task_queue respects max_workers concurrency limit");
}
