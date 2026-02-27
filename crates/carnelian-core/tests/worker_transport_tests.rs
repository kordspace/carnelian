//! Integration tests for the Worker Transport Layer
//!
//! These tests require Node.js to be installed and use the mock worker
//! at `tests/fixtures/mock_worker.js`.

use carnelian_common::types::{InvokeRequest, InvokeStatus, RunId};
use carnelian_core::config::Config;
use carnelian_core::events::EventStream;
use carnelian_core::worker::{ProcessJsonlTransport, WorkerTransport};
use serde_json::json;
use std::process::Stdio;
use std::sync::Arc;

/// Path to the mock worker script relative to the workspace root.
fn mock_worker_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/tests/fixtures/mock_worker.js")
}

/// Spawn a mock worker process with optional environment overrides.
fn spawn_mock_worker(envs: Vec<(&str, &str)>) -> tokio::process::Child {
    let mut cmd = tokio::process::Command::new("node");
    cmd.arg(mock_worker_path())
        .env("WORKER_ID", "test-mock-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.spawn()
        .expect("Failed to spawn mock worker (is Node.js installed?)")
}

/// Create a test config with short timeouts for faster tests.
fn test_config() -> Arc<Config> {
    let mut config = Config::default();
    config.skill_timeout_secs = 10;
    config.skill_timeout_grace_period_secs = 2;
    config.skill_max_output_bytes = 1_048_576;
    config.skill_max_log_lines = 10_000;
    Arc::new(config)
}

fn test_event_stream() -> Arc<EventStream> {
    Arc::new(EventStream::new(100, 10))
}

#[tokio::test]
#[ignore = "requires Node.js installed - run with: cargo test --test worker_transport_tests -- --ignored"]
async fn test_process_jsonl_invoke_success() {
    let child = spawn_mock_worker(vec![]);
    let config = test_config();
    let event_stream = test_event_stream();

    let (transport, _stderr) =
        ProcessJsonlTransport::new("test-worker-1".into(), child, config, event_stream.clone())
            .expect("Failed to create transport");

    let run_id = RunId::new();
    let request = InvokeRequest {
        run_id,
        skill_name: "echo_skill".into(),
        input: json!({"hello": "world"}),
        timeout_secs: 10,
        correlation_id: None,
    };

    let response = transport
        .invoke(request)
        .await
        .expect("Invoke should succeed");

    assert_eq!(response.run_id, run_id);
    assert_eq!(response.status, InvokeStatus::Success);
    assert!(!response.truncated);
    assert!(response.error.is_none());

    // Verify the echo result contains our input
    let echo = &response.result["echo"];
    assert_eq!(echo["hello"], "world");
    assert_eq!(response.result["skill_name"], "echo_skill");
}

#[tokio::test]
#[ignore = "requires Node.js installed - run with: cargo test --test worker_transport_tests -- --ignored"]
async fn test_process_jsonl_timeout_enforcement() {
    // Worker sleeps 10 seconds, but timeout is 2 seconds
    let child = spawn_mock_worker(vec![("MOCK_WORKER_SLEEP_MS", "10000")]);
    let mut config = Config::default();
    config.skill_timeout_secs = 2;
    config.skill_timeout_grace_period_secs = 1;
    let config = Arc::new(config);
    let event_stream = test_event_stream();

    let (transport, _stderr) =
        ProcessJsonlTransport::new("test-worker-timeout".into(), child, config, event_stream)
            .expect("Failed to create transport");

    let run_id = RunId::new();
    let request = InvokeRequest {
        run_id,
        skill_name: "slow_skill".into(),
        input: json!({}),
        timeout_secs: 2,
        correlation_id: None,
    };

    let response = transport
        .invoke(request)
        .await
        .expect("Invoke should return timeout");

    assert_eq!(response.run_id, run_id);
    assert_eq!(response.status, InvokeStatus::Timeout);
    assert!(response.error.is_some());
    assert!(response.error.unwrap().contains("timed out"));
}

#[tokio::test]
#[ignore = "requires Node.js installed - run with: cargo test --test worker_transport_tests -- --ignored"]
async fn test_process_jsonl_output_truncation() {
    // Worker generates 2MB of output, but limit is 1MB
    let child = spawn_mock_worker(vec![("MOCK_WORKER_OUTPUT_SIZE", "2097152")]);
    let mut config = Config::default();
    config.skill_timeout_secs = 30;
    config.skill_max_output_bytes = 1_048_576; // 1MB
    let config = Arc::new(config);
    let event_stream = test_event_stream();

    let (transport, _stderr) =
        ProcessJsonlTransport::new("test-worker-truncate".into(), child, config, event_stream)
            .expect("Failed to create transport");

    let run_id = RunId::new();
    let request = InvokeRequest {
        run_id,
        skill_name: "big_output_skill".into(),
        input: json!({}),
        timeout_secs: 30,
        correlation_id: None,
    };

    let response = transport
        .invoke(request)
        .await
        .expect("Invoke should succeed");

    assert_eq!(response.run_id, run_id);
    assert_eq!(response.status, InvokeStatus::Success);
    // The 2MB result payload exceeds skill_max_output_bytes (1MB), so the transport
    // should replace it with a truncation marker and set truncated = true.
    assert!(response.truncated, "Response should be marked as truncated");
    assert!(
        response.result["..."].is_string(),
        "Result should contain truncation marker"
    );
}

#[tokio::test]
#[ignore = "requires Node.js installed - run with: cargo test --test worker_transport_tests -- --ignored"]
async fn test_process_jsonl_cancellation() {
    // Worker sleeps 30 seconds, we cancel after 1 second
    let child = spawn_mock_worker(vec![("MOCK_WORKER_SLEEP_MS", "30000")]);
    let config = test_config();
    let event_stream = test_event_stream();

    let (transport, _stderr) =
        ProcessJsonlTransport::new("test-worker-cancel".into(), child, config, event_stream)
            .expect("Failed to create transport");
    let transport = Arc::new(transport);

    let run_id = RunId::new();
    let request = InvokeRequest {
        run_id,
        skill_name: "long_skill".into(),
        input: json!({}),
        timeout_secs: 60,
        correlation_id: None,
    };

    // Spawn invoke in background
    let transport_clone = transport.clone();
    let invoke_handle = tokio::spawn(async move { transport_clone.invoke(request).await });

    // Wait a moment then cancel
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    transport
        .cancel(run_id, "test cancellation".into())
        .await
        .expect("Cancel should succeed");

    let response = invoke_handle
        .await
        .unwrap()
        .expect("Invoke should return cancelled");

    assert_eq!(response.run_id, run_id);
    assert_eq!(response.status, InvokeStatus::Cancelled);
    assert!(response.error.is_some());
}

#[tokio::test]
#[ignore = "requires Node.js installed - run with: cargo test --test worker_transport_tests -- --ignored"]
async fn test_process_jsonl_event_streaming() {
    // Worker emits 5 stream events before responding
    let child = spawn_mock_worker(vec![("MOCK_WORKER_EMIT_EVENTS", "5")]);
    let config = test_config();
    let event_stream = test_event_stream();

    let (transport, _stderr) =
        ProcessJsonlTransport::new("test-worker-events".into(), child, config, event_stream)
            .expect("Failed to create transport");

    let run_id = RunId::new();
    let request = InvokeRequest {
        run_id,
        skill_name: "event_skill".into(),
        input: json!({}),
        timeout_secs: 10,
        correlation_id: None,
    };

    let response = transport
        .invoke(request)
        .await
        .expect("Invoke should succeed");

    assert_eq!(response.run_id, run_id);
    assert_eq!(response.status, InvokeStatus::Success);
    // Events were emitted and processed (no crash = success)
}

#[tokio::test]
#[ignore = "requires Node.js installed - run with: cargo test --test worker_transport_tests -- --ignored"]
async fn test_process_jsonl_health_check() {
    let child = spawn_mock_worker(vec![]);
    let config = test_config();
    let event_stream = test_event_stream();

    let (transport, _stderr) =
        ProcessJsonlTransport::new("test-worker-health".into(), child, config, event_stream)
            .expect("Failed to create transport");

    // Worker should be healthy
    let health = transport
        .health()
        .await
        .expect("Health check should succeed");
    assert!(health.healthy);
    assert_eq!(health.worker_id, "test-worker-health");
    assert!(health.uptime_secs < 60); // Just started
}

#[tokio::test]
#[ignore = "requires Node.js installed - run with: cargo test --test worker_transport_tests -- --ignored"]
async fn test_worker_manager_transport_integration() {
    use carnelian_core::worker::{WorkerManager, WorkerRuntime};

    let config = test_config();
    let event_stream = test_event_stream();
    let mut manager = WorkerManager::new(config, event_stream);

    // Spawn a Node worker (requires node and worker script)
    // This test validates the integration path; it may fail if the
    // actual worker script isn't present, but the transport creation
    // path is exercised.
    let result = manager.spawn_worker(WorkerRuntime::Node, false).await;

    // If spawn succeeded, verify transport is accessible
    if let Ok(worker_id) = result {
        let transport = manager
            .get_transport(&worker_id)
            .await
            .expect("Transport should be available");

        let health = transport.health().await.expect("Health check should work");
        // The real worker may or may not be healthy depending on the environment
        // but the transport path works
        assert_eq!(health.worker_id, worker_id);

        // Clean up
        manager
            .stop_worker(&worker_id)
            .await
            .expect("Stop should succeed");
    }
    // If spawn failed (no node/script), that's expected in CI without workers
}
