#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(unused_imports)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::redundant_clone)]

//! Integration tests for the HTTP/WebSocket server

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::{Config, EventStream, PolicyEngine, Scheduler, Server};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
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
    Arc::new(tokio::sync::Mutex::new(Scheduler::new(
        pool,
        event_stream,
        Duration::from_secs(3600),
    )))
}

#[tokio::test]
async fn test_websocket_event_streaming() {
    // Allocate a random available port
    let port = allocate_random_port().await;

    let mut config = Config::default();
    config.http_port = port;

    let event_stream = Arc::new(EventStream::new(100, 10));
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_scheduler(event_stream.clone()),
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
    config.http_port = port;

    // Small broadcast capacity to trigger lag
    let event_stream = Arc::new(EventStream::new(100, 5));
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_scheduler(event_stream.clone()),
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
    config.http_port = port;

    let event_stream = Arc::new(EventStream::new(100, 10));
    let config = Arc::new(config);

    let server = Server::new(config.clone(), event_stream.clone(), create_test_policy_engine(), create_test_scheduler(event_stream));

    assert_eq!(server.port(), port);
}

#[tokio::test]
async fn test_event_json_serialization() {
    // Allocate a random available port
    let port = allocate_random_port().await;

    let mut config = Config::default();
    config.http_port = port;

    let event_stream = Arc::new(EventStream::new(100, 10));
    let config = Arc::new(config);

    let server = Server::new(
        config.clone(),
        event_stream.clone(),
        create_test_policy_engine(),
        create_test_scheduler(event_stream.clone()),
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
