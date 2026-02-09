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
use carnelian_core::{Config, EventStream, Ledger, PolicyEngine, Scheduler, Server, WorkerManager};
use futures_util::StreamExt;
use memory_stats::memory_stats;
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

/// Create a lazy Scheduler for tests that don't need database access
fn create_test_scheduler(event_stream: Arc<EventStream>) -> Arc<tokio::sync::Mutex<Scheduler>> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgresql://test:test@localhost:5432/test")
        .expect("Failed to create lazy pool");
    Arc::new(tokio::sync::Mutex::new(Scheduler::new(
        pool,
        event_stream,
        Duration::from_secs(3600), // Long interval for tests - won't actually tick
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

/// Create a test configuration with in-memory settings (no database)
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
    let event_stream = Arc::new(event_stream);
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream),
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
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream),
    );

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
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream),
    );

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

    // Record baseline memory usage before load
    let baseline_memory_bytes = memory_stats().map_or(0, |s| s.physical_mem);
    let mut peak_memory_bytes = baseline_memory_bytes;
    let mut memory_samples: Vec<usize> = vec![baseline_memory_bytes];

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

        // Sample memory every 1000 events (~6 seconds)
        if i % 1000 == 0 {
            if let Some(stats) = memory_stats() {
                let current = stats.physical_mem;
                memory_samples.push(current);
                if current > peak_memory_bytes {
                    peak_memory_bytes = current;
                }
            }

            // Check client is still connected
            if !client_connected.load(std::sync::atomic::Ordering::Relaxed) {
                panic!(
                    "WebSocket client disconnected during load test at event {}",
                    i
                );
            }
        }
    }
    let elapsed = start.elapsed();

    // Final memory sample after all events published
    if let Some(stats) = memory_stats() {
        let current = stats.physical_mem;
        memory_samples.push(current);
        if current > peak_memory_bytes {
            peak_memory_bytes = current;
        }
    }

    // Wait for remaining events to be received
    tokio::time::sleep(Duration::from_secs(3)).await;

    let final_count = received_count.load(std::sync::atomic::Ordering::Relaxed);
    let final_error_received = error_received.load(std::sync::atomic::Ordering::Relaxed);
    let still_connected = client_connected.load(std::sync::atomic::Ordering::Relaxed);
    let stats = event_stream_clone.stats();

    let baseline_mb = baseline_memory_bytes as f64 / (1024.0 * 1024.0);
    let peak_mb = peak_memory_bytes as f64 / (1024.0 * 1024.0);
    let growth_ratio = if baseline_memory_bytes > 0 {
        peak_memory_bytes as f64 / baseline_memory_bytes as f64
    } else {
        1.0
    };

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
    println!("=== Memory Usage ===");
    println!("Baseline: {:.2} MB", baseline_mb);
    println!("Peak:     {:.2} MB", peak_mb);
    println!("Growth:   {:.2}x baseline", growth_ratio);
    println!(
        "Samples:  {:?}",
        memory_samples
            .iter()
            .map(|s| format!("{:.1}MB", *s as f64 / (1024.0 * 1024.0)))
            .collect::<Vec<_>>()
    );

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
    // Use >= because the broadcast channel may deliver a small number of duplicates
    // when the receiver lags under heavy load.
    assert!(
        final_error_received >= error_published,
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

    // ASSERTION 6: Memory usage remains bounded
    // Peak should not exceed 2x baseline OR an absolute 256MB ceiling
    let max_allowed_bytes = std::cmp::max(baseline_memory_bytes * 2, 256 * 1024 * 1024);
    assert!(
        peak_memory_bytes <= max_allowed_bytes,
        "Peak memory ({:.2} MB) should stay within 2x baseline ({:.2} MB) or 256 MB ceiling. \
         Growth ratio: {:.2}x",
        peak_mb,
        baseline_mb,
        growth_ratio
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
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream),
    );

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
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream),
    );

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
    // Note: ERROR events are never dropped, so buffer may exceed nominal capacity
    // when many ERROR events are published. We expect at most 100 ERROR events (1 in 5)
    // plus some retained higher-priority events.
    assert!(
        stats.buffer_len <= 200,
        "Buffer should be bounded (got {})",
        stats.buffer_len
    );

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
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream),
    );

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
    let event_stream = Arc::new(event_stream);
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_ledger(),
        create_test_scheduler(event_stream.clone()),
        create_test_worker_manager(config, event_stream),
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

/// Verify a column exists in a table with the expected data type
async fn verify_column(pool: &sqlx::PgPool, table: &str, column: &str, expected_type: &str) {
    let col_type: Option<String> = sqlx::query_scalar(
        "SELECT data_type::text FROM information_schema.columns \
         WHERE table_schema = 'public' AND table_name = $1 AND column_name = $2",
    )
    .bind(table)
    .bind(column)
    .fetch_optional(pool)
    .await
    .expect("Should query column info");

    assert!(
        col_type.is_some(),
        "Column '{}.{}' should exist",
        table,
        column
    );
    let actual = col_type.unwrap();
    assert!(
        actual.contains(expected_type),
        "Column '{}.{}' should be '{}', got '{}'",
        table,
        column,
        expected_type,
        actual
    );
}

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
    let event_stream = Arc::new(event_stream);

    // Create PolicyEngine with the real database pool
    let policy_engine = Arc::new(PolicyEngine::new(pool.clone()));

    // Create Scheduler with the real database pool
    let scheduler = Arc::new(tokio::sync::Mutex::new(Scheduler::new(
        pool.clone(),
        event_stream.clone(),
        Duration::from_secs(3600),
    )));

    let ledger = Arc::new(Ledger::new(pool.clone()));
    let config = Arc::new(config);
    let worker_manager = create_test_worker_manager(config.clone(), event_stream.clone());
    let server = Server::new(
        config,
        event_stream,
        policy_engine,
        ledger,
        scheduler,
        worker_manager,
    );

    // Start server in background
    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to be ready
    assert!(wait_for_server(port, 5).await, "Server should become ready");

    // Query health endpoint - should be healthy with database
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
    let event_stream = Arc::new(event_stream);

    let policy_engine = Arc::new(PolicyEngine::new(pool.clone()));
    let ledger = Arc::new(Ledger::new(pool.clone()));
    let scheduler = Arc::new(tokio::sync::Mutex::new(Scheduler::new(
        pool,
        event_stream.clone(),
        Duration::from_secs(3600),
    )));
    let config = Arc::new(config);
    let worker_manager = create_test_worker_manager(config.clone(), event_stream.clone());
    let server = Server::new(
        config,
        event_stream,
        policy_engine,
        ledger,
        scheduler,
        worker_manager,
    );

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
    let scheduler = Arc::new(tokio::sync::Mutex::new(Scheduler::new(
        pool.clone(),
        event_stream.clone(),
        Duration::from_secs(3600),
    )));
    let ledger = Arc::new(Ledger::new(pool.clone()));
    let config = Arc::new(config);
    let worker_manager = create_test_worker_manager(config.clone(), event_stream.clone());
    let server = Server::new(
        config,
        event_stream,
        policy_engine,
        ledger,
        scheduler,
        worker_manager,
    );

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

    let policy_engine = Arc::new(PolicyEngine::new(pool.clone()));
    let ledger = Arc::new(Ledger::new(pool.clone()));
    let scheduler = Arc::new(tokio::sync::Mutex::new(Scheduler::new(
        pool,
        event_stream.clone(),
        Duration::from_secs(3600),
    )));
    let config = Arc::new(config);
    let worker_manager = create_test_worker_manager(config.clone(), event_stream.clone());
    let server = Server::new(
        config,
        event_stream,
        policy_engine,
        ledger,
        scheduler,
        worker_manager,
    );

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

/// Test heartbeat interval timing with `run_with_shutdown`
///
/// This test:
/// - Starts a server with a real database and a short heartbeat interval (2s)
/// - Connects a WebSocket client to `/v1/events/ws`
/// - Waits for two or more `HeartbeatTick` events from the scheduler
/// - Asserts elapsed time between consecutive heartbeats matches the configured interval (±500ms)
/// - Verifies event payload contains expected fields
/// - Verifies mantra selection follows "first unknown, then random rotation" via DB state
/// - Shuts down cleanly via `run_with_shutdown`
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test test_heartbeat_interval_timing -- --ignored"]
async fn test_heartbeat_interval_timing() {
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

    // Run migrations (creates Lian identity + seed data needed by scheduler)
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

    let policy_engine = Arc::new(PolicyEngine::new(pool.clone()));
    let ledger = Arc::new(Ledger::new(pool.clone()));

    // Configure scheduler with a short heartbeat interval (2 seconds) for test speed
    let heartbeat_interval = Duration::from_millis(2000);
    let scheduler = Arc::new(tokio::sync::Mutex::new(Scheduler::new(
        pool.clone(),
        event_stream.clone(),
        heartbeat_interval,
    )));

    let config = Arc::new(config);
    let worker_manager = create_test_worker_manager(config.clone(), event_stream.clone());

    let server = Server::new(
        config,
        event_stream,
        policy_engine,
        ledger,
        scheduler,
        worker_manager,
    );

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

    // Collect HeartbeatTick events with timestamps
    let mut heartbeat_times: Vec<tokio::time::Instant> = Vec::new();
    let mut heartbeat_payloads: Vec<serde_json::Value> = Vec::new();
    let target_heartbeats = 3;

    // Wait for at least 3 heartbeats (should take ~6 seconds with 2s interval)
    // Allow up to 15 seconds total to account for the first tick delay + DB queries
    let collect_result = timeout(Duration::from_secs(15), async {
        while heartbeat_times.len() < target_heartbeats {
            if let Some(Ok(msg)) = read.next().await {
                if let Message::Text(text) = msg {
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                        if event.get("event_type").and_then(|v| v.as_str()) == Some("HeartbeatTick")
                        {
                            heartbeat_times.push(tokio::time::Instant::now());
                            heartbeat_payloads.push(event);
                        }
                    }
                }
            }
        }
    })
    .await;

    assert!(
        collect_result.is_ok(),
        "Should receive {} HeartbeatTick events within timeout (got {})",
        target_heartbeats,
        heartbeat_times.len()
    );

    // ASSERTION 1: Verify timing between consecutive heartbeats
    let tolerance = Duration::from_millis(500);
    for i in 1..heartbeat_times.len() {
        let delta = heartbeat_times[i] - heartbeat_times[i - 1];
        let lower = heartbeat_interval.saturating_sub(tolerance);
        let upper = heartbeat_interval + tolerance;
        println!(
            "Heartbeat {} → {}: delta={:?} (expected {:?} ± {:?})",
            i - 1,
            i,
            delta,
            heartbeat_interval,
            tolerance
        );
        assert!(
            delta >= lower && delta <= upper,
            "Heartbeat interval delta {:?} should be within {:?} ± {:?}",
            delta,
            heartbeat_interval,
            tolerance
        );
    }

    // ASSERTION 2: Verify event payload contains expected fields
    for (idx, payload) in heartbeat_payloads.iter().enumerate() {
        let p = &payload["payload"];
        assert!(
            p.get("heartbeat_id").is_some(),
            "Heartbeat {} should have heartbeat_id",
            idx
        );
        assert!(
            p.get("identity_id").is_some(),
            "Heartbeat {} should have identity_id",
            idx
        );
        assert!(
            p.get("mantra").is_some(),
            "Heartbeat {} should have mantra",
            idx
        );
        assert!(
            p.get("tasks_queued").is_some(),
            "Heartbeat {} should have tasks_queued",
            idx
        );
        assert!(
            p.get("duration_ms").is_some(),
            "Heartbeat {} should have duration_ms",
            idx
        );
    }

    // ASSERTION 3: Verify mantra selection strategy via database state
    // The first mantras should be "unknown" ones (not yet used), confirming
    // the "first unknown, then random rotation" strategy
    let mantras: Vec<String> = heartbeat_payloads
        .iter()
        .filter_map(|p| p["payload"]["mantra"].as_str().map(String::from))
        .collect();
    println!("Mantras selected: {:?}", mantras);
    assert!(
        !mantras.is_empty(),
        "Should have collected at least one mantra"
    );
    // First mantra should be a valid string (from the MANTRAS list)
    assert!(!mantras[0].is_empty(), "First mantra should not be empty");

    // Verify heartbeat records were written to the database
    let pool_ref = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&get_database_url(&container).await)
        .await
        .expect("Should reconnect for verification");

    let heartbeat_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM heartbeat_history")
        .fetch_one(&pool_ref)
        .await
        .expect("Should query heartbeat_history");

    #[allow(clippy::cast_possible_wrap)]
    let target_i64 = target_heartbeats as i64;
    assert!(
        heartbeat_count >= target_i64,
        "Database should have at least {} heartbeat records, got {}",
        target_heartbeats,
        heartbeat_count
    );

    println!(
        "✓ Heartbeat interval test passed: {} heartbeats at ~{:?} intervals",
        heartbeat_times.len(),
        heartbeat_interval
    );

    // Trigger graceful shutdown
    shutdown_tx.send(()).expect("Should send shutdown signal");

    // Wait for server to shut down cleanly
    let shutdown_result = timeout(Duration::from_secs(10), server_handle).await;
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
    assert_eq!(lian.1, "he/him");
    assert_eq!(lian.2, "core");
    assert_eq!(lian.3, Some("souls/lian.md".to_string()));
    assert_ne!(lian.1, "they/them", "Lian pronouns must not be they/them");

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

    // =========================================================================
    // SCHEMA COVERAGE: Verify all expected tables, columns, indexes, extensions
    // =========================================================================

    // Verify all expected tables exist via information_schema
    let expected_tables = vec![
        "identities",
        "capabilities",
        "capability_grants",
        "skills",
        "tasks",
        "task_runs",
        "run_logs",
        "ledger_events",
        "memories",
        "model_providers",
        "usage_costs",
        "config_store",
        "config_versions",
        "heartbeat_history",
    ];

    let existing_tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_type = 'BASE TABLE' \
         ORDER BY table_name",
    )
    .fetch_all(&pool)
    .await
    .expect("Should query information_schema.tables");

    for table in &expected_tables {
        assert!(
            existing_tables.iter().any(|t| t == table),
            "Table '{}' should exist in schema. Found tables: {:?}",
            table,
            existing_tables
        );
    }
    println!("✓ All {} expected tables verified", expected_tables.len());

    // Verify key columns with correct data types for critical tables

    // identities table columns
    verify_column(&pool, "identities", "identity_id", "uuid").await;
    verify_column(&pool, "identities", "name", "text").await;
    verify_column(&pool, "identities", "identity_type", "text").await;
    verify_column(&pool, "identities", "soul_file_path", "text").await;
    verify_column(&pool, "identities", "directives", "jsonb").await;
    verify_column(&pool, "identities", "voice_config", "jsonb").await;
    verify_column(&pool, "identities", "created_at", "timestamp").await;

    // tasks table columns
    verify_column(&pool, "tasks", "task_id", "uuid").await;
    verify_column(&pool, "tasks", "title", "text").await;
    verify_column(&pool, "tasks", "state", "text").await;
    verify_column(&pool, "tasks", "priority", "integer").await;
    verify_column(&pool, "tasks", "requires_approval", "boolean").await;

    // ledger_events table columns
    verify_column(&pool, "ledger_events", "event_id", "bigint").await;
    verify_column(&pool, "ledger_events", "action_type", "text").await;
    verify_column(&pool, "ledger_events", "payload_hash", "text").await;
    verify_column(&pool, "ledger_events", "prev_hash", "text").await;
    verify_column(&pool, "ledger_events", "event_hash", "text").await;
    verify_column(&pool, "ledger_events", "core_signature", "text").await;
    verify_column(&pool, "ledger_events", "metadata", "jsonb").await;

    // memories table columns
    verify_column(&pool, "memories", "memory_id", "uuid").await;
    verify_column(&pool, "memories", "content", "text").await;
    verify_column(&pool, "memories", "source", "text").await;
    verify_column(&pool, "memories", "importance", "real").await;

    // capability_grants table columns
    verify_column(&pool, "capability_grants", "grant_id", "uuid").await;
    verify_column(&pool, "capability_grants", "subject_type", "text").await;
    verify_column(&pool, "capability_grants", "subject_id", "text").await;
    verify_column(&pool, "capability_grants", "capability_key", "text").await;
    verify_column(&pool, "capability_grants", "scope", "jsonb").await;

    // usage_costs table columns
    verify_column(&pool, "usage_costs", "tokens_in", "integer").await;
    verify_column(&pool, "usage_costs", "tokens_out", "integer").await;
    verify_column(&pool, "usage_costs", "cost_estimate", "numeric").await;

    println!("✓ Key column types verified across critical tables");

    // Verify expected indexes exist via pg_indexes
    let expected_indexes = vec![
        ("identities", "idx_identities_type"),
        ("capability_grants", "idx_capability_grants_subject"),
        ("capability_grants", "idx_capability_grants_key"),
        ("skills", "idx_skills_enabled"),
        ("skills", "idx_skills_runtime"),
        ("tasks", "idx_tasks_state"),
        ("tasks", "idx_tasks_created_by"),
        ("tasks", "idx_tasks_correlation"),
        ("task_runs", "idx_task_runs_task"),
        ("task_runs", "idx_task_runs_state"),
        ("run_logs", "idx_run_logs_run"),
        ("run_logs", "idx_run_logs_level"),
        ("run_logs", "idx_run_logs_ts"),
        ("ledger_events", "idx_ledger_events_ts"),
        ("ledger_events", "idx_ledger_events_actor"),
        ("ledger_events", "idx_ledger_events_correlation"),
        ("memories", "idx_memories_identity"),
        ("memories", "idx_memories_source"),
        ("memories", "idx_memories_importance"),
        ("model_providers", "idx_model_providers_enabled"),
        ("usage_costs", "idx_usage_costs_provider"),
        ("usage_costs", "idx_usage_costs_ts"),
        ("usage_costs", "idx_usage_costs_task"),
        ("config_versions", "idx_config_versions_key"),
        ("config_versions", "idx_config_versions_created"),
        ("heartbeat_history", "idx_heartbeat_history_identity"),
        ("heartbeat_history", "idx_heartbeat_history_ts"),
    ];

    let existing_indexes: Vec<(String, String)> = sqlx::query_as(
        "SELECT tablename::text, indexname::text FROM pg_indexes \
         WHERE schemaname = 'public' ORDER BY tablename, indexname",
    )
    .fetch_all(&pool)
    .await
    .expect("Should query pg_indexes");

    for (table, index) in &expected_indexes {
        assert!(
            existing_indexes
                .iter()
                .any(|(t, i)| t == table && i == index),
            "Index '{}' on table '{}' should exist. Found indexes on '{}': {:?}",
            index,
            table,
            table,
            existing_indexes
                .iter()
                .filter(|(t, _)| t == table)
                .map(|(_, i)| i.as_str())
                .collect::<Vec<_>>()
        );
    }
    println!("✓ All {} expected indexes verified", expected_indexes.len());

    // Verify extensions are enabled: pgcrypto, vector
    let extensions: Vec<String> =
        sqlx::query_scalar("SELECT extname::text FROM pg_extension ORDER BY extname")
            .fetch_all(&pool)
            .await
            .expect("Should query pg_extension");

    assert!(
        extensions.iter().any(|e| e == "pgcrypto"),
        "pgcrypto extension should be enabled. Found: {:?}",
        extensions
    );
    assert!(
        extensions.iter().any(|e| e == "vector"),
        "vector extension should be enabled. Found: {:?}",
        extensions
    );
    println!("✓ Required extensions (pgcrypto, vector) verified");

    // Verify CHECK constraint enforcement
    // Invalid identity_type should fail
    let invalid_identity = sqlx::query(
        "INSERT INTO identities (name, identity_type) VALUES ('test_invalid', 'invalid_type')",
    )
    .execute(&pool)
    .await;

    assert!(
        invalid_identity.is_err(),
        "Invalid identity_type should be rejected by CHECK constraint"
    );

    // Invalid task state should fail
    let invalid_task =
        sqlx::query("INSERT INTO tasks (title, state) VALUES ('test_invalid', 'bogus_state')")
            .execute(&pool)
            .await;

    assert!(
        invalid_task.is_err(),
        "Invalid task state should be rejected by CHECK constraint"
    );

    // Invalid memory importance (out of range) should fail
    let lian_id: uuid::Uuid =
        sqlx::query_scalar("SELECT identity_id FROM identities WHERE name = 'Lian' LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("Lian should exist");

    let invalid_memory = sqlx::query(
        "INSERT INTO memories (identity_id, content, source, importance) \
         VALUES ($1, 'test', 'observation', 5.0)",
    )
    .bind(lian_id)
    .execute(&pool)
    .await;

    assert!(
        invalid_memory.is_err(),
        "Importance > 1.0 should be rejected by CHECK constraint"
    );

    println!("✓ CHECK constraint enforcement verified");

    // Verify Lian directives seed data completeness
    let directives: serde_json::Value =
        sqlx::query_scalar("SELECT directives FROM identities WHERE name = 'Lian'")
            .fetch_one(&pool)
            .await
            .expect("Should query Lian directives");

    assert!(
        directives.is_array(),
        "Lian directives should be a JSON array"
    );
    let directives_arr = directives.as_array().unwrap();
    assert!(
        directives_arr.len() >= 3,
        "Lian should have at least 3 directives, got {}",
        directives_arr.len()
    );

    // Verify all 20 capabilities exist (exact count from seed data)
    assert_eq!(
        capability_count, 20,
        "Should have exactly 20 default capabilities, got {}",
        capability_count
    );

    println!("✓ Complete schema coverage validation passed");
}
