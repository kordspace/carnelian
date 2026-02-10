#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(unused_imports)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_precision_loss)]

//! Integration tests for the HTTP/WebSocket server

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::{Config, EventStream, Ledger, PolicyEngine, Scheduler, Server, WorkerManager};
use futures_util::{SinkExt, StreamExt};
use memory_stats::memory_stats;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Allocate a random available port and return it
async fn allocate_random_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to random port");
    let port = listener.local_addr().unwrap().port();
    drop(listener); // Release the port for the server to use
    port
}

/// Helper to wait for server to be ready
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

/// Create a lazy PolicyEngine for tests that don't need database access
fn create_test_policy_engine() -> Arc<PolicyEngine> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgresql://test:test@localhost:5432/test")
        .expect("Failed to create lazy pool");
    Arc::new(PolicyEngine::new(pool))
}

/// Create a lazy Scheduler for tests that don't need database access
fn create_test_scheduler(event_stream: Arc<EventStream>) -> Arc<tokio::sync::Mutex<Scheduler>> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgresql://test:test@localhost:5432/test")
        .expect("Failed to create lazy pool");
    let config = Arc::new(Config::default());
    let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
        config.clone(),
        event_stream.clone(),
    )));
    Arc::new(tokio::sync::Mutex::new(Scheduler::new(
        pool,
        event_stream,
        Duration::from_secs(3600),
        worker_manager,
        config,
    )))
}

/// Create a WorkerManager for tests
fn create_test_worker_manager(
    config: Arc<Config>,
    event_stream: Arc<EventStream>,
) -> Arc<tokio::sync::Mutex<WorkerManager>> {
    Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
        config,
        event_stream,
    )))
}

/// Create a lazy Ledger for tests that don't need database access
fn create_test_ledger() -> Arc<Ledger> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgresql://test:test@localhost:5432/test")
        .expect("Failed to create lazy pool");
    Arc::new(Ledger::new(pool))
}

#[tokio::test]
async fn test_websocket_event_streaming() {
    // Allocate a random available port
    let port = allocate_random_port().await;

    let mut config = Config::default();
    config.bind_address = "127.0.0.1".to_string();
    config.http_port = port;

    let event_stream = Arc::new(EventStream::new(100, 10));
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream.clone()),
    );

    // Spawn server in background
    let server_handle = tokio::spawn(async move {
        server.run().await.expect("Server failed to start");
    });

    // Wait for server to be ready with proper timeout
    assert!(
        wait_for_server(port, Duration::from_secs(5)).await,
        "Server failed to start within timeout"
    );

    // Connect WebSocket client
    let ws_url = format!("ws://127.0.0.1:{}/v1/events/ws", port);
    let (mut ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("Failed to connect WebSocket");

    // Publish test events
    for i in 0..5 {
        event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::Custom(format!("test_event_{}", i)),
            json!({"index": i}),
        ));
    }

    // Receive events (skip the RuntimeReady event)
    let mut received_count = 0;
    let timeout = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(msg) = ws_stream.next().await {
            if let Ok(Message::Text(text)) = msg {
                // Try to parse as EventEnvelope
                if let Ok(event) = serde_json::from_str::<EventEnvelope>(&text) {
                    if matches!(event.event_type, EventType::Custom(_)) {
                        received_count += 1;
                        if received_count >= 5 {
                            break;
                        }
                    }
                }
            }
        }
    });

    let _ = timeout.await;
    assert!(
        received_count >= 5,
        "Expected at least 5 events, got {}",
        received_count
    );

    // Clean up
    let _ = ws_stream.close(None).await;
    server_handle.abort();
}

#[tokio::test]
async fn test_websocket_backpressure() {
    // Allocate a random available port
    let port = allocate_random_port().await;

    let mut config = Config::default();
    config.bind_address = "127.0.0.1".to_string();
    config.http_port = port;

    // Small broadcast capacity to trigger lag
    let event_stream = Arc::new(EventStream::new(100, 5));
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream.clone()),
    );

    // Spawn server in background
    let server_handle = tokio::spawn(async move {
        server.run().await.expect("Server failed to start");
    });

    // Wait for server to be ready with proper timeout
    assert!(
        wait_for_server(port, Duration::from_secs(5)).await,
        "Server failed to start within timeout"
    );

    // Connect WebSocket client
    let ws_url = format!("ws://127.0.0.1:{}/v1/events/ws", port);
    let (mut ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("Failed to connect WebSocket");

    // Publish many events rapidly to trigger backpressure
    for i in 0..100 {
        event_stream.publish(EventEnvelope::new(
            EventLevel::Debug,
            EventType::Custom(format!("rapid_event_{}", i)),
            json!({"index": i}),
        ));
    }

    // Receive some events - we should get either events or lag notification
    let mut received_any = false;

    let timeout = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(msg) = ws_stream.next().await {
            if let Ok(Message::Text(_)) = msg {
                received_any = true;
                break;
            }
        }
    });

    let _ = timeout.await;

    // We should have received something
    assert!(received_any, "Should have received at least one message");

    // Clean up
    let _ = ws_stream.close(None).await;
    server_handle.abort();
}

#[tokio::test]
async fn test_server_port_configuration() {
    // Allocate a random available port
    let port = allocate_random_port().await;

    let mut config = Config::default();
    config.bind_address = "127.0.0.1".to_string();
    config.http_port = port;

    let event_stream = Arc::new(EventStream::new(100, 10));
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream),
    );

    assert_eq!(server.port(), port);
}

#[tokio::test]
async fn test_event_json_serialization() {
    // Allocate a random available port
    let port = allocate_random_port().await;

    let mut config = Config::default();
    config.bind_address = "127.0.0.1".to_string();
    config.http_port = port;

    let event_stream = Arc::new(EventStream::new(100, 10));
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream.clone()),
    );

    // Spawn server in background
    let server_handle = tokio::spawn(async move {
        server.run().await.expect("Server failed to start");
    });

    // Wait for server to be ready with proper timeout
    assert!(
        wait_for_server(port, Duration::from_secs(5)).await,
        "Server failed to start within timeout"
    );

    // Connect WebSocket client
    let ws_url = format!("ws://127.0.0.1:{}/v1/events/ws", port);
    let (mut ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("Failed to connect WebSocket");

    // Publish a test event with specific payload
    let test_payload = json!({
        "task_id": "test-123",
        "status": "completed",
        "metadata": {
            "duration_ms": 1500,
            "worker": "worker-1"
        }
    });

    event_stream.publish(EventEnvelope::new(
        EventLevel::Info,
        EventType::TaskCompleted,
        test_payload.clone(),
    ));

    // Receive and verify JSON structure
    let timeout = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(msg) = ws_stream.next().await {
            if let Ok(Message::Text(text)) = msg {
                if let Ok(event) = serde_json::from_str::<EventEnvelope>(&text) {
                    if matches!(event.event_type, EventType::TaskCompleted) {
                        // Verify payload structure
                        assert_eq!(event.payload["task_id"], "test-123");
                        assert_eq!(event.payload["status"], "completed");
                        assert_eq!(event.payload["metadata"]["duration_ms"], 1500);
                        return true;
                    }
                }
            }
        }
        false
    });

    let result = timeout.await.unwrap_or(false);
    assert!(
        result,
        "Should have received and parsed TaskCompleted event"
    );

    // Clean up
    let _ = ws_stream.close(None).await;
    server_handle.abort();
}

// =============================================================================
// HELPER: PostgreSQL container for database-backed server tests
// =============================================================================

/// Create a PostgreSQL container for testing
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

/// Get the database URL from a running container
async fn get_database_url(container: &testcontainers::ContainerAsync<GenericImage>) -> String {
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");
    format!("postgresql://test:test@127.0.0.1:{}/carnelian_test", port)
}

/// Measure current physical memory usage in bytes.
/// Panics if memory stats are unavailable so acceptance criteria is always enforced.
fn measure_memory_usage() -> usize {
    memory_stats().map(|stats| stats.physical_mem).expect(
        "memory_stats() returned None — cannot enforce memory acceptance criteria on this platform",
    )
}

// =============================================================================
// LOAD TESTS
// =============================================================================

/// Test: 10k events/minute throughput via WebSocket
#[tokio::test]
#[ignore = "Long-running load test (60s) - run with: cargo test --test server_integration_test test_websocket_load -- --ignored"]
async fn test_websocket_load_10k_events_per_minute() {
    let port = allocate_random_port().await;

    let mut config = Config::default();
    config.bind_address = "127.0.0.1".to_string();
    config.http_port = port;

    // High capacity per acceptance criteria
    let event_stream = Arc::new(EventStream::new(50_000, 4096));
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream.clone()),
    );

    let server_handle = tokio::spawn(async move {
        server.run().await.expect("Server failed to start");
    });

    assert!(
        wait_for_server(port, Duration::from_secs(5)).await,
        "Server failed to start within timeout"
    );

    // Connect WebSocket client
    let ws_url = format!("ws://127.0.0.1:{}/v1/events/ws", port);
    let (mut ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("Failed to connect WebSocket");

    let baseline_mem = measure_memory_usage();
    println!("Baseline memory: {}MB", baseline_mem / (1024 * 1024));

    // Publish 10,000 events over 60 seconds (~166 events/sec)
    let stream_clone = event_stream.clone();
    let publisher = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_micros(6000)); // ~166/sec
        for i in 0..10_000u32 {
            interval.tick().await;
            stream_clone.publish(EventEnvelope::new(
                EventLevel::Info,
                EventType::Custom(format!("load_event_{}", i)),
                json!({"index": i}),
            ));
        }
    });

    // Track received event count
    let mut received_count = 0u32;
    let receive_timeout = tokio::time::timeout(Duration::from_secs(75), async {
        while let Some(msg) = ws_stream.next().await {
            if let Ok(Message::Text(text)) = msg {
                if let Ok(event) = serde_json::from_str::<EventEnvelope>(&text) {
                    if matches!(event.event_type, EventType::Custom(_)) {
                        received_count += 1;
                        if received_count >= 9_500 {
                            break;
                        }
                    }
                }
            }
        }
    });

    let _ = receive_timeout.await;
    publisher.await.expect("Publisher task should complete");

    assert!(
        received_count >= 9_500,
        "Should receive at least 9,500 of 10,000 events (5% loss tolerance), got {}",
        received_count
    );

    // Measure memory growth
    let final_mem = measure_memory_usage();
    let delta_mb = (final_mem.saturating_sub(baseline_mem)) / (1024 * 1024);
    assert!(
        delta_mb < 100,
        "Memory growth should be bounded (<100MB), grew {}MB",
        delta_mb
    );
    println!("Memory growth: {}MB", delta_mb);

    println!(
        "✓ 10k events/min load test passed: received {}/10000 events",
        received_count
    );

    let _ = ws_stream.close(None).await;
    server_handle.abort();
}

/// Test: bounded memory under flood scenario
#[tokio::test]
#[ignore = "Long-running load test - run with: cargo test --test server_integration_test test_websocket_bounded -- --ignored"]
async fn test_websocket_bounded_memory_under_load() {
    let port = allocate_random_port().await;

    let mut config = Config::default();
    config.bind_address = "127.0.0.1".to_string();
    config.http_port = port;

    // Default capacity to test backpressure
    let event_stream = Arc::new(EventStream::new(10_000, 1024));
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream.clone()),
    );

    let server_handle = tokio::spawn(async move {
        server.run().await.expect("Server failed to start");
    });

    assert!(
        wait_for_server(port, Duration::from_secs(5)).await,
        "Server failed to start within timeout"
    );

    let baseline_mem = measure_memory_usage();
    let mut max_mem: usize = baseline_mem;

    // Publish 50,000 events rapidly (flood scenario)
    for i in 0..50_000u32 {
        event_stream.publish(EventEnvelope::new(
            EventLevel::Debug,
            EventType::Custom(format!("flood_event_{}", i)),
            json!({"index": i}),
        ));

        // Sample memory every 1000 events
        if i % 1000 == 0 {
            let current = measure_memory_usage();
            if current > max_mem {
                max_mem = current;
            }
        }
    }

    // Verify memory plateaus
    let growth_factor = if baseline_mem > 0 {
        max_mem as f64 / baseline_mem as f64
    } else {
        1.0
    };
    assert!(
        growth_factor < 2.0,
        "Memory should stay within 2x baseline. Baseline: {}MB, Max: {}MB, Factor: {:.2}x",
        baseline_mem / (1024 * 1024),
        max_mem / (1024 * 1024),
        growth_factor
    );
    println!(
        "✓ Bounded memory test passed: baseline {}MB, max {}MB, factor {:.2}x",
        baseline_mem / (1024 * 1024),
        max_mem / (1024 * 1024),
        growth_factor
    );

    server_handle.abort();
}

/// Test: multiple WebSocket clients receive broadcast events
#[tokio::test]
#[ignore = "Load test - run with: cargo test --test server_integration_test test_websocket_multiple_clients_broadcast -- --ignored"]
async fn test_websocket_multiple_clients_broadcast() {
    let port = allocate_random_port().await;

    let mut config = Config::default();
    config.bind_address = "127.0.0.1".to_string();
    config.http_port = port;

    let event_stream = Arc::new(EventStream::new(10_000, 1024));
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream.clone()),
    );

    let server_handle = tokio::spawn(async move {
        server.run().await.expect("Server failed to start");
    });

    assert!(
        wait_for_server(port, Duration::from_secs(5)).await,
        "Server failed to start within timeout"
    );

    // Connect 10 WebSocket clients simultaneously
    let ws_url = format!("ws://127.0.0.1:{}/v1/events/ws", port);
    let mut clients = Vec::new();
    for _ in 0..10 {
        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .expect("Failed to connect WebSocket client");
        clients.push(ws_stream);
    }

    // Give clients time to register
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Publish 1,000 events
    for i in 0..1_000u32 {
        event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::Custom(format!("broadcast_event_{}", i)),
            json!({"index": i}),
        ));
    }

    // Read all clients concurrently to avoid sequential timeout starvation
    let mut handles = Vec::new();
    for (i, mut client) in clients.into_iter().enumerate() {
        handles.push(tokio::spawn(async move {
            let mut count = 0u32;
            let result = tokio::time::timeout(Duration::from_secs(5), async {
                while let Some(msg) = client.next().await {
                    if let Ok(Message::Text(text)) = msg {
                        if let Ok(event) = serde_json::from_str::<EventEnvelope>(&text) {
                            if matches!(event.event_type, EventType::Custom(_)) {
                                count += 1;
                                if count >= 900 {
                                    break;
                                }
                            }
                        }
                    }
                }
            });
            let _ = result.await;
            let _ = client.close(None).await;
            (i, count)
        }));
    }

    let mut client_counts = vec![0u32; 10];
    for handle in handles {
        let (i, count) = handle.await.expect("Client reader task should complete");
        client_counts[i] = count;
    }

    // All clients should have received a substantial number of events
    for (i, count) in client_counts.iter().enumerate() {
        assert!(
            *count >= 500,
            "Client {} should receive at least 500 events, got {}",
            i,
            count
        );
    }

    println!(
        "✓ Multi-client broadcast test passed: client counts {:?}",
        client_counts
    );

    server_handle.abort();
}

/// Test: capability grants with TEXT subject_id via database
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test server_integration_test test_capability_grants -- --ignored"]
async fn test_capability_grants_text_subject_id() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("Failed to connect to database");

    carnelian_core::db::run_migrations(&pool)
        .await
        .expect("Migrations should succeed");

    // Insert capability grant with external subject_id format
    let result = sqlx::query(
        "INSERT INTO capability_grants (subject_type, subject_id, capability_key) \
         VALUES ('channel', 'telegram:12345', 'fs.read')",
    )
    .execute(&pool)
    .await;

    assert!(
        result.is_ok(),
        "TEXT subject_id 'telegram:12345' should be accepted. Error: {:?}",
        result.err()
    );

    // Verify stored correctly
    let stored: String = sqlx::query_scalar(
        "SELECT subject_id FROM capability_grants WHERE subject_id = 'telegram:12345'",
    )
    .fetch_one(&pool)
    .await
    .expect("Should find stored grant");

    assert_eq!(
        stored, "telegram:12345",
        "Subject ID should be stored as TEXT"
    );

    // Insert grant with subject_type = 'external_key'
    let result_ext = sqlx::query(
        "INSERT INTO capability_grants (subject_type, subject_id, capability_key) \
         VALUES ('external_key', 'api-key-abc123', 'net.http')",
    )
    .execute(&pool)
    .await;

    assert!(
        result_ext.is_ok(),
        "subject_type 'external_key' should be accepted. Error: {:?}",
        result_ext.err()
    );

    println!("✓ Capability grants with TEXT subject_id verified");
}

/// Test: LZ4 compression verification via database
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test server_integration_test test_lz4_compression -- --ignored"]
async fn test_lz4_compression_verification() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("Failed to connect to database");

    carnelian_core::db::run_migrations(&pool)
        .await
        .expect("Migrations should succeed");

    let lian_id: uuid::Uuid =
        sqlx::query_scalar("SELECT identity_id FROM identities WHERE name = 'Lian' LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("Lian should exist");

    // Insert large memory content (>8KB)
    let large_content = "A".repeat(10_000);
    let memory_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO memories (identity_id, content, source) VALUES ($1, $2, 'observation') RETURNING memory_id",
    )
    .bind(lian_id)
    .bind(&large_content)
    .fetch_one(&pool)
    .await
    .expect("Should insert large memory");

    // Update to trigger compression
    sqlx::query("UPDATE memories SET content = content WHERE memory_id = $1")
        .bind(memory_id)
        .execute(&pool)
        .await
        .expect("Should update memory");

    // Verify compression is active via pg_attribute
    let compression: Option<String> = sqlx::query_scalar(
        "SELECT attcompression::text FROM pg_attribute \
         WHERE attrelid = 'memories'::regclass AND attname = 'content'",
    )
    .fetch_optional(&pool)
    .await
    .expect("Should query compression");

    assert_eq!(
        compression.as_deref(),
        Some("l"),
        "memories.content should have LZ4 compression (attcompression = 'l')"
    );

    // Verify column size is reasonable
    let col_size: Option<i32> =
        sqlx::query_scalar("SELECT pg_column_size(content) FROM memories WHERE memory_id = $1")
            .bind(memory_id)
            .fetch_optional(&pool)
            .await
            .expect("Should query column size");

    if let Some(size) = col_size {
        println!("Column size for 10KB memories.content: {} bytes", size);
    }

    // -------------------------------------------------------------------------
    // Verify run_logs.message LZ4 compression
    // -------------------------------------------------------------------------
    let rl_compression: Option<String> = sqlx::query_scalar(
        "SELECT attcompression::text FROM pg_attribute \
         WHERE attrelid = 'run_logs'::regclass AND attname = 'message'",
    )
    .fetch_optional(&pool)
    .await
    .expect("Should query run_logs.message compression");

    assert_eq!(
        rl_compression.as_deref(),
        Some("l"),
        "run_logs.message should have LZ4 compression (attcompression = 'l')"
    );

    // Insert a representative row to confirm the column is writable under LZ4
    // First we need a task_run to reference
    let large_message = "B".repeat(10_000);
    let rl_insert = sqlx::query(
        "INSERT INTO run_logs (run_id, level, message) \
         SELECT r.run_id, 'info', $1 FROM task_runs r LIMIT 1",
    )
    .bind(&large_message)
    .execute(&pool)
    .await;

    // If no task_runs exist, create one via a task first
    if rl_insert.is_err()
        || rl_insert
            .as_ref()
            .map(sqlx::postgres::PgQueryResult::rows_affected)
            .unwrap_or(0)
            == 0
    {
        // Create minimal task + task_run so we can insert a run_log
        sqlx::query(
            "INSERT INTO tasks (identity_id, task_type, payload, status) \
             VALUES ($1, 'test', '{}'::jsonb, 'pending')",
        )
        .bind(lian_id)
        .execute(&pool)
        .await
        .expect("Should insert test task");

        sqlx::query(
            "INSERT INTO task_runs (task_id, worker_id, status) \
             SELECT t.task_id, 'test-worker', 'running' FROM tasks t LIMIT 1",
        )
        .execute(&pool)
        .await
        .expect("Should insert test task_run");

        sqlx::query(
            "INSERT INTO run_logs (run_id, level, message) \
             SELECT r.run_id, 'info', $1 FROM task_runs r LIMIT 1",
        )
        .bind(&large_message)
        .execute(&pool)
        .await
        .expect("Should insert run_log with large message");
    }

    println!("✓ run_logs.message LZ4 compression verified and writable");

    // -------------------------------------------------------------------------
    // Verify ledger_events.metadata LZ4 compression
    // -------------------------------------------------------------------------
    let le_compression: Option<String> = sqlx::query_scalar(
        "SELECT attcompression::text FROM pg_attribute \
         WHERE attrelid = 'ledger_events'::regclass AND attname = 'metadata'",
    )
    .fetch_optional(&pool)
    .await
    .expect("Should query ledger_events.metadata compression");

    assert_eq!(
        le_compression.as_deref(),
        Some("l"),
        "ledger_events.metadata should have LZ4 compression (attcompression = 'l')"
    );

    // Insert a representative row with large metadata to confirm writability
    let large_metadata = serde_json::json!({
        "details": "C".repeat(10_000),
        "context": "lz4_compression_test"
    });
    sqlx::query(
        "INSERT INTO ledger_events (actor_id, action_type, payload_hash, event_hash, prev_hash, metadata) \
         VALUES ($1, 'test.lz4_verify', 'hash1', 'hash2', 'hash0', $2)",
    )
    .bind(lian_id)
    .bind(&large_metadata)
    .execute(&pool)
    .await
    .expect("Should insert ledger_event with large metadata under LZ4 compression");

    // Verify the row was stored and is readable
    let stored_metadata: serde_json::Value = sqlx::query_scalar(
        "SELECT metadata FROM ledger_events WHERE action_type = 'test.lz4_verify' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("Should read back ledger_event metadata");

    assert_eq!(
        stored_metadata["context"], "lz4_compression_test",
        "Stored metadata should be readable after LZ4 compression"
    );

    println!("✓ ledger_events.metadata LZ4 compression verified and writable");
    println!("✓ Full LZ4 compression verification passed (memories, run_logs, ledger_events)");
}
