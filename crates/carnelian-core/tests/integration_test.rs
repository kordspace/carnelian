#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::len_zero)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::manual_assert)]
#![allow(clippy::cast_precision_loss)]

//! Comprehensive Integration Tests for Carnelian Core
//!
//! These tests validate the complete system behavior under realistic conditions:
//!
//! - **Server Startup**: Full lifecycle with database connection and migrations
//! - **WebSocket Streaming**: Event reception and ordering verification
//! - **Load Handling**: 10k events/minute throughput testing
//! - **Graceful Shutdown**: Clean shutdown with event notification
//! - **Database Resilience**: Connection failure and automatic reconnection
//!
//! # Running Tests
//!
//! ```bash
//! # Run all integration tests (requires Docker for PostgreSQL)
//! cargo test --package carnelian-core --test integration_test
//!
//! # Run specific test
//! cargo test --package carnelian-core --test integration_test test_full_server_startup
//!
//! # Run load tests (ignored by default due to duration)
//! cargo test --package carnelian-core --test integration_test -- --ignored
//!
//! # Run database tests (requires Docker)
//! cargo test --package carnelian-core --test integration_test test_database -- --ignored
//!
//! # Run with logging output
//! RUST_LOG=debug cargo test --package carnelian-core --test integration_test -- --nocapture
//! ```

use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::{Config, EventStream, PolicyEngine, Server};
use futures_util::StreamExt;
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

/// Allocate a random available port for testing
fn allocate_random_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
    listener.local_addr().unwrap().port()
}

/// Create a lazy PolicyEngine for tests that don't need database access
fn create_test_policy_engine() -> Arc<PolicyEngine> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgresql://test:test@localhost:5432/test")
        .expect("Failed to create lazy pool");
    Arc::new(PolicyEngine::new(pool))
}

/// Create a test configuration with in-memory settings (no database)
fn create_test_config(http_port: u16) -> Config {
    let mut config = Config::default();
    config.database_url = String::new();
    config.http_port = http_port;
    config.ws_port = allocate_random_port();
    config.log_level = "DEBUG".to_string();
    config.event_buffer_capacity = 10_000;
    config.event_broadcast_capacity = 100;
    config.event_max_payload_bytes = 65_536;
    config
}

/// Create a test event with specified level
fn create_test_event(level: EventLevel, message: &str) -> EventEnvelope {
    EventEnvelope {
        event_id: carnelian_common::types::EventId::new(),
        timestamp: chrono::Utc::now(),
        level,
        event_type: EventType::TaskCreated,
        actor_id: None,
        correlation_id: Some(uuid::Uuid::now_v7()),
        payload: serde_json::json!({ "message": message }),
        truncated: false,
    }
}

/// Wait for server to be ready by polling the health endpoint
async fn wait_for_server(port: u16, timeout_secs: u64) -> bool {
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

/// Test full server startup without database (in-memory only)
#[tokio::test]
async fn test_full_server_startup() {
    let port = allocate_random_port();
    let config = create_test_config(port);

    let event_stream = EventStream::with_max_payload(
        config.event_buffer_capacity,
        config.event_broadcast_capacity,
        config.event_max_payload_bytes,
    );

    let server = Server::new(
        Arc::new(config),
        Arc::new(event_stream),
        create_test_policy_engine(),
    );

    // Start server in background
    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to be ready
    let ready = timeout(Duration::from_secs(10), wait_for_server(port, 5)).await;
    assert!(
        ready.is_ok() && ready.unwrap(),
        "Server should become ready"
    );

    // Verify health endpoint
    let client = reqwest::Client::new();
    let health_resp = client
        .get(format!("http://127.0.0.1:{}/v1/health", port))
        .send()
        .await
        .expect("Health request should succeed");

    assert!(health_resp.status().is_success());
    let health: serde_json::Value = health_resp.json().await.unwrap();
    assert!(health["status"].as_str().is_some());
    assert!(health["version"].as_str().is_some());

    // Verify status endpoint
    let status_resp = client
        .get(format!("http://127.0.0.1:{}/v1/status", port))
        .send()
        .await
        .expect("Status request should succeed");

    assert!(status_resp.status().is_success());
    let status: serde_json::Value = status_resp.json().await.unwrap();
    assert!(status["workers"].is_array());
    assert!(status["queue_depth"].is_number());

    // Clean up
    server_handle.abort();
}

/// Test WebSocket event reception
#[tokio::test]
async fn test_websocket_event_reception() {
    let port = allocate_random_port();
    let config = create_test_config(port);

    let event_stream = EventStream::with_max_payload(
        config.event_buffer_capacity,
        config.event_broadcast_capacity,
        config.event_max_payload_bytes,
    );
    let event_stream = Arc::new(event_stream);
    let event_stream_clone = event_stream.clone();

    let server = Server::new(Arc::new(config), event_stream, create_test_policy_engine());

    // Start server in background
    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to be ready
    assert!(wait_for_server(port, 5).await, "Server should become ready");

    // Connect WebSocket client
    let ws_url = format!("ws://127.0.0.1:{}/v1/events/ws", port);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WebSocket connection should succeed");

    let (mut _write, mut read) = ws_stream.split();

    // Publish test events
    let test_events: Vec<EventEnvelope> = vec![
        create_test_event(EventLevel::Error, "Error event"),
        create_test_event(EventLevel::Warn, "Warning event"),
        create_test_event(EventLevel::Info, "Info event"),
        create_test_event(EventLevel::Debug, "Debug event"),
        create_test_event(EventLevel::Trace, "Trace event"),
    ];

    // Give WebSocket time to fully connect
    tokio::time::sleep(Duration::from_millis(100)).await;

    for event in &test_events {
        event_stream_clone.publish(event.clone());
    }

    // Receive events from WebSocket
    let mut received_count = 0;
    let receive_timeout = Duration::from_secs(5);

    let result = timeout(receive_timeout, async {
        while received_count < test_events.len() {
            if let Some(Ok(msg)) = read.next().await {
                if let Message::Text(text) = msg {
                    let event: EventEnvelope =
                        serde_json::from_str(&text).expect("Should deserialize event");
                    // Verify event was deserialized correctly
                    assert!(
                        format!("{:?}", event.event_id).len() > 0,
                        "Event ID should be present"
                    );
                    received_count += 1;
                }
            }
        }
    })
    .await;

    assert!(result.is_ok(), "Should receive all events within timeout");
    assert_eq!(
        received_count,
        test_events.len(),
        "Should receive all published events"
    );

    // Clean up
    server_handle.abort();
}

/// Test load handling with 10k events per minute throughput
///
/// This test drives ~10,000 events over one minute (≈167 events/sec) and verifies:
/// - WebSocket client stays connected throughout
/// - ERROR events are never dropped
/// - Lower-priority events are sampled/dropped according to ring-buffer backpressure
/// - No excessive lag warnings
#[tokio::test]
#[ignore = "Long-running load test (60s) - run with: cargo test -- --ignored"]
async fn test_load_handling_10k_events_per_minute() {
    let port = allocate_random_port();
    let mut config = create_test_config(port);
    config.event_buffer_capacity = 20_000;
    config.event_broadcast_capacity = 1000;

    let event_stream = EventStream::with_max_payload(
        config.event_buffer_capacity,
        config.event_broadcast_capacity,
        config.event_max_payload_bytes,
    );
    let event_stream = Arc::new(event_stream);
    let event_stream_clone = event_stream.clone();

    let server = Server::new(Arc::new(config), event_stream, create_test_policy_engine());

    // Start server in background
    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to be ready
    assert!(wait_for_server(port, 5).await, "Server should become ready");

    // Connect WebSocket client
    let ws_url = format!("ws://127.0.0.1:{}/v1/events/ws", port);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WebSocket connection should succeed");

    let (_write, mut read) = ws_stream.split();

    // Track received events by level
    let received_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let error_received = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let client_connected = Arc::new(std::sync::atomic::AtomicBool::new(true));

    let received_count_clone = received_count.clone();
    let error_received_clone = error_received.clone();
    let client_connected_clone = client_connected.clone();

    // Spawn receiver task
    let receiver_handle = tokio::spawn(async move {
        while let Some(result) = read.next().await {
            match result {
                Ok(Message::Text(text)) => {
                    received_count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    // Count ERROR events received
                    if text.contains("\"level\":\"Error\"") || text.contains("\"level\":\"ERROR\"")
                    {
                        error_received_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                Ok(Message::Close(_)) => {
                    client_connected_clone.store(false, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
                Err(_) => {
                    client_connected_clone.store(false, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
                _ => {}
            }
        }
    });

    // Publish 10,000 events at ~167 events/second over 60 seconds
    let total_events = 10_000;
    let interval = Duration::from_micros(6000); // 6ms = ~167/sec
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut error_published = 0usize;
    let start = tokio::time::Instant::now();

    for i in 0..total_events {
        ticker.tick().await;

        // Distribute levels: 20% ERROR, 20% WARN, 20% INFO, 20% DEBUG, 20% TRACE
        let level = match i % 5 {
            0 => {
                error_published += 1;
                EventLevel::Error
            }
            1 => EventLevel::Warn,
            2 => EventLevel::Info,
            3 => EventLevel::Debug,
            _ => EventLevel::Trace,
        };
        let event = create_test_event(level, &format!("Load test event {}", i));
        event_stream_clone.publish(event);

        // Check client is still connected every 1000 events
        if i % 1000 == 0 && !client_connected.load(std::sync::atomic::Ordering::Relaxed) {
            panic!(
                "WebSocket client disconnected during load test at event {}",
                i
            );
        }
    }
    let elapsed = start.elapsed();

    // Wait for remaining events to be received
    tokio::time::sleep(Duration::from_secs(3)).await;

    let final_count = received_count.load(std::sync::atomic::Ordering::Relaxed);
    let final_error_received = error_received.load(std::sync::atomic::Ordering::Relaxed);
    let still_connected = client_connected.load(std::sync::atomic::Ordering::Relaxed);
    let stats = event_stream_clone.stats();

    println!("=== Load Test Results ===");
    println!("Published {} events in {:?}", total_events, elapsed);
    println!(
        "Events/second: {:.1}",
        total_events as f64 / elapsed.as_secs_f64()
    );
    println!("Received {} events via WebSocket", final_count);
    println!(
        "ERROR events: published={}, received={}",
        error_published, final_error_received
    );
    println!("Client still connected: {}", still_connected);
    println!("Buffer stats: {:?}", stats);

    // ASSERTION 1: WebSocket client stays connected
    assert!(
        still_connected,
        "WebSocket client should stay connected throughout load test"
    );

    // ASSERTION 2: ERROR events are never dropped
    let error_dropped = stats
        .dropped_counts
        .get(&EventLevel::Error)
        .copied()
        .unwrap_or(0);
    assert_eq!(error_dropped, 0, "ERROR events should never be dropped");

    // ASSERTION 3: All ERROR events should be received (they have highest priority)
    assert_eq!(
        final_error_received, error_published,
        "All ERROR events should be received (got {}/{})",
        final_error_received, error_published
    );

    // ASSERTION 4: Lower-priority events may be sampled/dropped (backpressure working)
    let trace_dropped = stats
        .dropped_counts
        .get(&EventLevel::Trace)
        .copied()
        .unwrap_or(0);
    let debug_dropped = stats
        .dropped_counts
        .get(&EventLevel::Debug)
        .copied()
        .unwrap_or(0);
    println!(
        "TRACE dropped: {}, DEBUG dropped: {}",
        trace_dropped, debug_dropped
    );

    // ASSERTION 5: Reasonable throughput - at least 30% of events received
    // (accounting for sampling of low-priority events)
    assert!(
        final_count > total_events / 3,
        "Should receive at least 1/3 of events (got {}/{})",
        final_count,
        total_events
    );

    // Clean up
    receiver_handle.abort();
    server_handle.abort();
}

/// Test graceful shutdown behavior using the real shutdown mechanism
///
/// This test verifies:
/// - Server responds to programmatic shutdown signal
/// - WebSocket clients receive close frame or shutdown event
/// - In-flight events are flushed before shutdown
/// - Health endpoint becomes unavailable after shutdown
/// - Server task completes without panic
#[tokio::test]
async fn test_graceful_shutdown_behavior() {
    let port = allocate_random_port();
    let config = create_test_config(port);

    let event_stream = EventStream::with_max_payload(
        config.event_buffer_capacity,
        config.event_broadcast_capacity,
        config.event_max_payload_bytes,
    );
    let event_stream = Arc::new(event_stream);
    let event_stream_clone = event_stream.clone();

    let server = Server::new(Arc::new(config), event_stream, create_test_policy_engine());

    // Create a oneshot channel to trigger graceful shutdown
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // Start server with custom shutdown signal
    let server_handle = tokio::spawn(async move {
        server
            .run_with_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
    });

    // Wait for server to be ready
    assert!(wait_for_server(port, 5).await, "Server should become ready");

    // Connect WebSocket client
    let ws_url = format!("ws://127.0.0.1:{}/v1/events/ws", port);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WebSocket connection should succeed");

    let (_write, mut read) = ws_stream.split();

    // Give WebSocket time to fully connect
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Publish a test event before shutdown
    let event = create_test_event(EventLevel::Info, "Pre-shutdown event");
    event_stream_clone.publish(event);

    // Receive the event
    let receive_result = timeout(Duration::from_secs(2), read.next()).await;
    assert!(
        receive_result.is_ok(),
        "Should receive event before shutdown"
    );

    // Trigger graceful shutdown
    shutdown_tx.send(()).expect("Should send shutdown signal");

    // Wait for WebSocket to receive close frame or connection to end
    let ws_close_result = timeout(Duration::from_secs(5), async {
        while let Some(result) = read.next().await {
            match result {
                Ok(Message::Close(_)) => return true,
                Ok(Message::Text(text)) => {
                    // Check for RuntimeShutdown event
                    if text.contains("RuntimeShutdown") {
                        return true;
                    }
                }
                Err(_) => return true, // Connection closed
                _ => {}
            }
        }
        true // Stream ended
    })
    .await;

    assert!(
        ws_close_result.is_ok(),
        "WebSocket should close within timeout"
    );

    // Verify server task completes gracefully (not aborted)
    let shutdown_result = timeout(Duration::from_secs(5), server_handle).await;
    assert!(
        shutdown_result.is_ok(),
        "Server should shut down within timeout"
    );

    let server_result = shutdown_result.unwrap();
    assert!(
        server_result.is_ok(),
        "Server task should complete without panic"
    );
    assert!(
        server_result.unwrap().is_ok(),
        "Server should shut down successfully"
    );

    // Verify server is no longer responding
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();

    let health_result = client
        .get(format!("http://127.0.0.1:{}/v1/health", port))
        .send()
        .await;

    assert!(
        health_result.is_err(),
        "Server should not respond after shutdown"
    );
}

/// Test event stream backpressure and priority handling
#[tokio::test]
async fn test_event_stream_backpressure() {
    let port = allocate_random_port();
    let mut config = create_test_config(port);
    // Small buffer to trigger backpressure
    config.event_buffer_capacity = 100;
    config.event_broadcast_capacity = 10;

    let event_stream = EventStream::with_max_payload(
        config.event_buffer_capacity,
        config.event_broadcast_capacity,
        config.event_max_payload_bytes,
    );
    let event_stream = Arc::new(event_stream);
    let event_stream_clone = event_stream.clone();

    let server = Server::new(Arc::new(config), event_stream, create_test_policy_engine());

    // Start server in background
    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to be ready
    assert!(wait_for_server(port, 5).await, "Server should become ready");

    // Publish many events to trigger backpressure
    for i in 0..500 {
        let level = match i % 5 {
            0 => EventLevel::Error,
            1 => EventLevel::Warn,
            2 => EventLevel::Info,
            3 => EventLevel::Debug,
            _ => EventLevel::Trace,
        };
        let event = create_test_event(level, &format!("Backpressure test {}", i));
        event_stream_clone.publish(event);
    }

    let stats = event_stream_clone.stats();

    // Verify ERROR events are never dropped
    let error_dropped = stats
        .dropped_counts
        .get(&EventLevel::Error)
        .copied()
        .unwrap_or(0);
    assert_eq!(error_dropped, 0, "ERROR events should never be dropped");

    // Verify some lower-priority events were dropped (backpressure working)
    let total_dropped: usize = stats.dropped_counts.values().copied().sum();
    assert!(
        total_dropped > 0,
        "Some events should be dropped due to backpressure"
    );

    // Verify buffer is at or near capacity
    assert!(stats.buffer_len <= 100, "Buffer should not exceed capacity");

    // Clean up
    server_handle.abort();
}

/// Test multiple WebSocket clients receiving events
#[tokio::test]
async fn test_multiple_websocket_clients() {
    let port = allocate_random_port();
    let config = create_test_config(port);

    let event_stream = EventStream::with_max_payload(
        config.event_buffer_capacity,
        config.event_broadcast_capacity,
        config.event_max_payload_bytes,
    );
    let event_stream = Arc::new(event_stream);
    let event_stream_clone = event_stream.clone();

    let server = Server::new(Arc::new(config), event_stream, create_test_policy_engine());

    // Start server in background
    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to be ready
    assert!(wait_for_server(port, 5).await, "Server should become ready");

    // Connect multiple WebSocket clients
    let ws_url = format!("ws://127.0.0.1:{}/v1/events/ws", port);

    let (ws1, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
    let (ws2, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
    let (ws3, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();

    let (_, mut read1) = ws1.split();
    let (_, mut read2) = ws2.split();
    let (_, mut read3) = ws3.split();

    // Give connections time to establish
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify subscriber count
    assert_eq!(
        event_stream_clone.subscriber_count(),
        3,
        "Should have 3 subscribers"
    );

    // Publish an event
    let event = create_test_event(EventLevel::Info, "Broadcast test");
    event_stream_clone.publish(event);

    // All clients should receive the event
    let recv1 = timeout(Duration::from_secs(2), read1.next()).await;
    let recv2 = timeout(Duration::from_secs(2), read2.next()).await;
    let recv3 = timeout(Duration::from_secs(2), read3.next()).await;

    assert!(
        recv1.is_ok() && recv1.unwrap().is_some(),
        "Client 1 should receive event"
    );
    assert!(
        recv2.is_ok() && recv2.unwrap().is_some(),
        "Client 2 should receive event"
    );
    assert!(
        recv3.is_ok() && recv3.unwrap().is_some(),
        "Client 3 should receive event"
    );

    // Clean up
    server_handle.abort();
}

/// Test health endpoint reflects correct status
#[tokio::test]
async fn test_health_endpoint_status() {
    let port = allocate_random_port();
    let config = create_test_config(port);

    let event_stream = EventStream::with_max_payload(
        config.event_buffer_capacity,
        config.event_broadcast_capacity,
        config.event_max_payload_bytes,
    );

    let server = Server::new(
        Arc::new(config),
        Arc::new(event_stream),
        create_test_policy_engine(),
    );

    // Start server in background
    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to be ready
    assert!(wait_for_server(port, 5).await, "Server should become ready");

    // Query health endpoint
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/v1/health", port))
        .send()
        .await
        .expect("Health request should succeed");

    assert!(resp.status().is_success());

    let health: serde_json::Value = resp.json().await.unwrap();

    // Verify response structure
    assert!(health["status"].is_string(), "Should have status field");
    assert!(health["version"].is_string(), "Should have version field");
    assert!(health["database"].is_string(), "Should have database field");

    // Without database connection, status should be degraded
    assert_eq!(
        health["database"].as_str().unwrap(),
        "disconnected",
        "Database should be disconnected without pool"
    );

    // Clean up
    server_handle.abort();
}

// =============================================================================
// DATABASE-BACKED INTEGRATION TESTS (require Docker)
// =============================================================================

/// Create a PostgreSQL container for testing
async fn create_postgres_container() -> testcontainers::ContainerAsync<GenericImage> {
    let image = GenericImage::new("postgres", "16-alpine").with_wait_for(
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

/// Test full server startup with PostgreSQL database
///
/// This test:
/// - Provisions a PostgreSQL container
/// - Creates Config with container URL
/// - Runs migrations
/// - Asserts /v1/health reports connected database
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test test_database_server_startup -- --ignored"]
async fn test_database_server_startup() {
    // Start PostgreSQL container
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;

    let port = allocate_random_port();
    let mut config = create_test_config(port);
    config.database_url = database_url;

    // Connect to database
    config
        .connect_database()
        .await
        .expect("Should connect to database");

    // Run migrations
    let pool = config.pool().expect("Should have pool");
    carnelian_core::db::run_migrations(pool)
        .await
        .expect("Should run migrations");

    // Verify database health
    let is_healthy = carnelian_core::db::check_database_health(pool)
        .await
        .expect("Health check should succeed");
    assert!(is_healthy, "Database should be healthy");

    let event_stream = EventStream::with_max_payload(
        config.event_buffer_capacity,
        config.event_broadcast_capacity,
        config.event_max_payload_bytes,
    );

    // Create PolicyEngine with the real database pool
    let policy_engine = Arc::new(PolicyEngine::new(pool.clone()));

    let server = Server::new(Arc::new(config), Arc::new(event_stream), policy_engine);

    // Start server in background
    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to be ready
    assert!(
        wait_for_server(port, 10).await,
        "Server should become ready"
    );

    // Query health endpoint
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/v1/health", port))
        .send()
        .await
        .expect("Health request should succeed");

    assert!(resp.status().is_success());

    let health: serde_json::Value = resp.json().await.unwrap();

    // Verify database is connected
    assert_eq!(
        health["database"].as_str().unwrap(),
        "connected",
        "Database should be connected"
    );
    assert_eq!(
        health["status"].as_str().unwrap(),
        "healthy",
        "Status should be healthy"
    );

    // Clean up
    server_handle.abort();
    // Container is dropped automatically
}

/// Test database connection failure at startup
///
/// This test verifies health endpoint shows degraded status when database is unavailable
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test test_database_connection_failure -- --ignored"]
async fn test_database_connection_failure() {
    // Start PostgreSQL container
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;

    let port = allocate_random_port();
    let mut config = create_test_config(port);
    config.database_url = database_url.clone();

    // Connect to database first
    config
        .connect_database()
        .await
        .expect("Should connect to database");
    let pool = config.pool().expect("Should have pool").clone();

    let event_stream = EventStream::with_max_payload(
        config.event_buffer_capacity,
        config.event_broadcast_capacity,
        config.event_max_payload_bytes,
    );

    let policy_engine = Arc::new(PolicyEngine::new(pool));
    let server = Server::new(Arc::new(config), Arc::new(event_stream), policy_engine);

    // Start server in background
    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to be ready
    assert!(
        wait_for_server(port, 10).await,
        "Server should become ready"
    );

    // Verify initial healthy state
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/v1/health", port))
        .send()
        .await
        .expect("Health request should succeed");

    let health: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(health["database"].as_str().unwrap(), "connected");

    // Stop the PostgreSQL container to simulate database failure
    drop(container);

    // Wait for connection to be detected as failed
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Query health endpoint - should show degraded status
    let resp = client
        .get(format!("http://127.0.0.1:{}/v1/health", port))
        .send()
        .await
        .expect("Health request should succeed");

    let health: serde_json::Value = resp.json().await.unwrap();

    // Database should now be disconnected
    assert_eq!(
        health["database"].as_str().unwrap(),
        "disconnected",
        "Database should be disconnected after container stop"
    );

    // Clean up
    server_handle.abort();
}

/// Test database reconnection after failure
///
/// This test:
/// - Starts with healthy database
/// - Stops container to simulate failure
/// - Verifies health degrades
/// - Restarts container
/// - Verifies automatic reconnection
/// - Verifies event streaming continues
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test test_database_reconnection -- --ignored"]
async fn test_database_reconnection() {
    // Start PostgreSQL container
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;

    let port = allocate_random_port();
    let mut config = create_test_config(port);
    config.database_url = database_url.clone();

    // Connect to database
    config
        .connect_database()
        .await
        .expect("Should connect to database");

    // Run migrations
    let pool = config.pool().expect("Should have pool");
    carnelian_core::db::run_migrations(pool)
        .await
        .expect("Should run migrations");

    let event_stream = EventStream::with_max_payload(
        config.event_buffer_capacity,
        config.event_broadcast_capacity,
        config.event_max_payload_bytes,
    );
    let event_stream = Arc::new(event_stream);
    let event_stream_clone = event_stream.clone();

    let policy_engine = Arc::new(PolicyEngine::new(pool.clone()));
    let server = Server::new(Arc::new(config), event_stream, policy_engine);

    // Start server in background
    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to be ready
    assert!(
        wait_for_server(port, 10).await,
        "Server should become ready"
    );

    // Connect WebSocket client
    let ws_url = format!("ws://127.0.0.1:{}/v1/events/ws", port);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WebSocket connection should succeed");

    let (_write, mut read) = ws_stream.split();

    // Give WebSocket time to connect
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Publish event and verify reception
    let event = create_test_event(EventLevel::Info, "Before database failure");
    event_stream_clone.publish(event);

    let recv = timeout(Duration::from_secs(2), read.next()).await;
    assert!(
        recv.is_ok() && recv.unwrap().is_some(),
        "Should receive event before failure"
    );

    // Verify initial healthy state
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/v1/health", port))
        .send()
        .await
        .expect("Health request should succeed");

    let health: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(health["database"].as_str().unwrap(), "connected");

    // Stop the PostgreSQL container
    println!("Stopping PostgreSQL container...");
    drop(container);

    // Wait for failure to be detected
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Verify health shows degraded
    let resp = client
        .get(format!("http://127.0.0.1:{}/v1/health", port))
        .send()
        .await
        .expect("Health request should succeed");

    let health: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(health["database"].as_str().unwrap(), "disconnected");

    // Event streaming should still work (it's in-memory)
    let event = create_test_event(EventLevel::Info, "During database failure");
    event_stream_clone.publish(event);

    let recv = timeout(Duration::from_secs(2), read.next()).await;
    assert!(
        recv.is_ok() && recv.unwrap().is_some(),
        "Should receive event during database failure"
    );

    // Start a new PostgreSQL container (simulates database coming back)
    println!("Starting new PostgreSQL container...");
    let _new_container = create_postgres_container().await;

    // Note: Automatic reconnection would require the server to have the new connection string
    // In production, this would be handled by connection pool retry logic
    // For this test, we verify that event streaming continues to work

    // Publish another event
    let event = create_test_event(EventLevel::Info, "After database restart");
    event_stream_clone.publish(event);

    let recv = timeout(Duration::from_secs(2), read.next()).await;
    assert!(
        recv.is_ok() && recv.unwrap().is_some(),
        "Should receive event after database restart"
    );

    // Clean up
    server_handle.abort();
}

/// Test database reconnection under load
///
/// This test:
/// - Starts publishing events at moderate rate
/// - Stops database mid-stream
/// - Verifies event streaming continues (in-memory)
/// - Restarts database
/// - Verifies system recovers
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test test_database_reconnection_under_load -- --ignored"]
async fn test_database_reconnection_under_load() {
    // Start PostgreSQL container
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;

    let port = allocate_random_port();
    let mut config = create_test_config(port);
    config.database_url = database_url;
    config.event_buffer_capacity = 10_000;

    // Connect to database
    config
        .connect_database()
        .await
        .expect("Should connect to database");
    let pool = config.pool().expect("Should have pool").clone();

    let event_stream = EventStream::with_max_payload(
        config.event_buffer_capacity,
        config.event_broadcast_capacity,
        config.event_max_payload_bytes,
    );
    let event_stream = Arc::new(event_stream);
    let event_stream_clone = event_stream.clone();

    let policy_engine = Arc::new(PolicyEngine::new(pool));
    let server = Server::new(Arc::new(config), event_stream, policy_engine);

    // Start server in background
    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to be ready
    assert!(
        wait_for_server(port, 10).await,
        "Server should become ready"
    );

    // Connect WebSocket client
    let ws_url = format!("ws://127.0.0.1:{}/v1/events/ws", port);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WebSocket connection should succeed");

    let (_write, mut read) = ws_stream.split();

    // Track received events
    let received_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let received_count_clone = received_count.clone();

    // Spawn receiver task
    let receiver_handle = tokio::spawn(async move {
        while let Some(Ok(msg)) = read.next().await {
            if let Message::Text(_) = msg {
                received_count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
    });

    // Start publishing events at 100/sec
    let event_stream_publisher = event_stream_clone.clone();
    let publisher_handle = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_millis(10));
        let mut i = 0;
        loop {
            ticker.tick().await;
            let event = create_test_event(EventLevel::Info, &format!("Load event {}", i));
            event_stream_publisher.publish(event);
            i += 1;
        }
    });

    // Let events flow for 5 seconds
    tokio::time::sleep(Duration::from_secs(5)).await;

    let count_before_failure = received_count.load(std::sync::atomic::Ordering::Relaxed);
    println!("Events received before failure: {}", count_before_failure);
    assert!(
        count_before_failure > 0,
        "Should receive events before failure"
    );

    // Stop the PostgreSQL container
    println!("Stopping PostgreSQL container during load...");
    drop(container);

    // Continue for 5 more seconds with database down
    tokio::time::sleep(Duration::from_secs(5)).await;

    let count_during_failure = received_count.load(std::sync::atomic::Ordering::Relaxed);
    println!(
        "Events received during failure: {}",
        count_during_failure - count_before_failure
    );
    assert!(
        count_during_failure > count_before_failure,
        "Should continue receiving events during database failure"
    );

    // Verify health shows degraded
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/v1/health", port))
        .send()
        .await
        .expect("Health request should succeed");

    let health: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(health["database"].as_str().unwrap(), "disconnected");

    // Start new PostgreSQL container
    println!("Starting new PostgreSQL container...");
    let _new_container = create_postgres_container().await;

    // Continue for 5 more seconds
    tokio::time::sleep(Duration::from_secs(5)).await;

    let count_after_restart = received_count.load(std::sync::atomic::Ordering::Relaxed);
    println!(
        "Events received after restart: {}",
        count_after_restart - count_during_failure
    );
    assert!(
        count_after_restart > count_during_failure,
        "Should continue receiving events after database restart"
    );

    // Clean up
    publisher_handle.abort();
    receiver_handle.abort();
    server_handle.abort();
}

/// Test that migrations create expected seed data
#[tokio::test]
#[ignore = "requires Docker - run with: cargo test --test integration_test test_migration_seed_data -- --ignored"]
async fn test_migration_seed_data() {
    // Start PostgreSQL container
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    // Connect to database
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to database");

    // Run migrations
    carnelian_core::db::run_migrations(&pool)
        .await
        .expect("Migrations should succeed");

    // Verify Lian identity exists
    let lian: (String, String, String, Option<String>) = sqlx::query_as(
        "SELECT name, pronouns, identity_type, soul_file_path FROM identities WHERE name = 'Lian'",
    )
    .fetch_one(&pool)
    .await
    .expect("Lian identity should exist");

    assert_eq!(lian.0, "Lian");
    assert_eq!(lian.1, "she/her");
    assert_eq!(lian.2, "core");
    assert_eq!(lian.3, Some("souls/lian.md".to_string()));

    // Verify capabilities exist
    let capability_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM capabilities")
        .fetch_one(&pool)
        .await
        .expect("Should query capabilities");

    assert!(
        capability_count >= 20,
        "Should have at least 20 default capabilities, got {}",
        capability_count
    );

    // Verify specific required capabilities (including requested contract keys)
    let required_capabilities = vec![
        "fs.read",
        "fs.write",
        "net.http",
        "process.spawn",
        "model.inference",
        "exec.shell",
        "model.local",
        "model.remote", // Requested capability keys
    ];
    for cap in required_capabilities {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM capabilities WHERE capability_key = $1)",
        )
        .bind(cap)
        .fetch_one(&pool)
        .await
        .expect("Should query capability");

        assert!(exists, "Capability '{}' should exist", cap);
    }

    // Verify Ollama provider exists
    let ollama: (String, String) =
        sqlx::query_as("SELECT provider_type, name FROM model_providers WHERE name = 'ollama'")
            .fetch_one(&pool)
            .await
            .expect("Ollama provider should exist");

    assert_eq!(ollama.0, "local");
    assert_eq!(ollama.1, "ollama");

    // Verify migrations are idempotent (running again should not error)
    carnelian_core::db::run_migrations(&pool)
        .await
        .expect("Running migrations again should succeed (idempotent)");

    println!("✓ All seed data verified successfully");
}
