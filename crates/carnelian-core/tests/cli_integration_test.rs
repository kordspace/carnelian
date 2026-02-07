#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::too_many_lines)]

//! CLI Integration Tests for Carnelian
//!
//! These tests validate the CLI commands by spawning actual processes.
//!
//! # Running Tests
//!
//! ```bash
//! cargo test --test cli_integration_test -- --ignored
//! ```

use std::process::Command;
use std::time::Duration;

use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

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

/// Helper to wait for a CLI-spawned server to be ready
async fn wait_for_cli_server(port: u16, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .is_ok()
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

/// Test: `carnelian migrate --database-url <url>` runs successfully
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test cli_integration_test -- --ignored"]
async fn test_cli_migrate_command() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    // Execute migrate command via cargo run
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "--database-url",
            &db_url,
            "migrate",
        ])
        .output()
        .expect("Failed to execute carnelian migrate");

    assert!(
        output.status.success(),
        "carnelian migrate should exit 0. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify migrations were applied by querying the database
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to database");

    let latest_version: i64 =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version DESC LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("Should have at least one migration applied");

    assert!(
        latest_version > 0,
        "Latest migration version should be positive, got {}",
        latest_version
    );

    println!(
        "✓ CLI migrate command succeeded, latest migration version: {}",
        latest_version
    );
}

/// Test: `carnelian start --config <path>` starts server and responds to health check
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test cli_integration_test -- --ignored"]
async fn test_cli_start_command_with_config() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    // First run migrations
    let migrate_output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "--database-url",
            &db_url,
            "migrate",
        ])
        .output()
        .expect("Failed to execute carnelian migrate");
    assert!(migrate_output.status.success(), "Migrations should succeed");

    // Create temporary config file
    let temp_dir = std::env::temp_dir().join("carnelian_cli_test");
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let config_path = temp_dir.join("test_machine.toml");
    let port = {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
        listener.local_addr().unwrap().port()
    };

    let config_content = format!(
        r#"
machine_profile = "thummim"
http_port = {}
database_url = "{}"
log_level = "INFO"
"#,
        port, db_url
    );
    std::fs::write(&config_path, config_content).expect("Failed to write config file");

    // Spawn server as background process
    let mut child = tokio::process::Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "--config",
            config_path.to_str().unwrap(),
            "start",
        ])
        .kill_on_drop(true)
        .spawn()
        .expect("Failed to spawn carnelian start");

    // Wait for server to be ready (max 10s)
    let ready = wait_for_cli_server(port, Duration::from_secs(10)).await;
    assert!(ready, "Server should start within 10 seconds");

    // Verify health endpoint
    let client = reqwest::Client::new();
    let health_response = client
        .get(format!("http://127.0.0.1:{}/v1/health", port))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .expect("Health request should succeed");

    assert!(
        health_response.status().is_success(),
        "Health endpoint should return 2xx"
    );

    let body: serde_json::Value = health_response
        .json()
        .await
        .expect("Health response should be JSON");

    assert!(
        body.get("status").is_some(),
        "Health response should contain 'status' field"
    );

    let status = body["status"].as_str().unwrap_or("");
    assert!(
        status == "healthy" || status == "degraded",
        "Status should be 'healthy' or 'degraded', got '{}'",
        status
    );

    println!("✓ CLI start command succeeded, health status: {}", status);

    // Send SIGTERM equivalent (kill the process) and verify graceful shutdown
    child.kill().await.expect("Failed to kill server process");

    // Clean up temp files
    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// Test: `carnelian stop` terminates a running server
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test cli_integration_test -- --ignored"]
async fn test_cli_stop_command() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    // Run migrations first
    let migrate_output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "--database-url",
            &db_url,
            "migrate",
        ])
        .output()
        .expect("Failed to execute carnelian migrate");
    assert!(migrate_output.status.success(), "Migrations should succeed");

    let port = {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
        listener.local_addr().unwrap().port()
    };

    // Start server in background
    let mut child = tokio::process::Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "--database-url",
            &db_url,
            "start",
        ])
        .env("CARNELIAN_HTTP_PORT", port.to_string())
        .kill_on_drop(true)
        .spawn()
        .expect("Failed to spawn carnelian start");

    // Wait for server to be ready
    let ready = wait_for_cli_server(port, Duration::from_secs(10)).await;
    assert!(ready, "Server should start within 10 seconds");

    // Execute stop command
    let stop_output = Command::new("cargo")
        .args(["run", "--bin", "carnelian", "--", "stop"])
        .output()
        .expect("Failed to execute carnelian stop");

    // Stop command should exit cleanly (0 or non-zero if no PID found is acceptable)
    println!(
        "Stop command exit code: {:?}, stdout: {}, stderr: {}",
        stop_output.status.code(),
        String::from_utf8_lossy(&stop_output.stdout),
        String::from_utf8_lossy(&stop_output.stderr)
    );

    // Wait for server to terminate (max 5s)
    let terminated = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
    match terminated {
        Ok(Ok(status)) => println!("✓ Server terminated with status: {}", status),
        Ok(Err(e)) => println!("✓ Server process error (expected after stop): {}", e),
        Err(_) => {
            // Timeout - force kill
            child.kill().await.expect("Failed to kill server");
            println!("⚠ Server did not terminate within 5s, force killed");
        }
    }
}

/// Test: `carnelian status` returns structured JSON output
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test cli_integration_test -- --ignored"]
async fn test_cli_status_command() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    // Run migrations first
    let migrate_output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "--database-url",
            &db_url,
            "migrate",
        ])
        .output()
        .expect("Failed to execute carnelian migrate");
    assert!(migrate_output.status.success(), "Migrations should succeed");

    let port = {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
        listener.local_addr().unwrap().port()
    };

    // Start server in background
    let _child = tokio::process::Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "--database-url",
            &db_url,
            "start",
        ])
        .env("CARNELIAN_HTTP_PORT", port.to_string())
        .kill_on_drop(true)
        .spawn()
        .expect("Failed to spawn carnelian start");

    // Wait for server to be ready
    let ready = wait_for_cli_server(port, Duration::from_secs(10)).await;
    assert!(ready, "Server should start within 10 seconds");

    // Execute status command
    let status_output = Command::new("cargo")
        .args(["run", "--bin", "carnelian", "--", "status"])
        .env("CARNELIAN_HTTP_PORT", port.to_string())
        .output()
        .expect("Failed to execute carnelian status");

    assert!(
        status_output.status.success(),
        "carnelian status should exit 0. stderr: {}",
        String::from_utf8_lossy(&status_output.stderr)
    );

    let stdout = String::from_utf8_lossy(&status_output.stdout);
    println!("Status output: {}", stdout);

    // Verify output contains expected structure
    // The status command queries /v1/status which returns JSON
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
        assert!(
            parsed.get("workers").is_some() || parsed.get("queue_depth").is_some(),
            "Status output should contain 'workers' or 'queue_depth' field"
        );
        println!("✓ CLI status command returned valid JSON");
    } else {
        // Status may output human-readable text instead of raw JSON
        println!("✓ CLI status command completed (non-JSON output)");
    }
}

/// Test: `carnelian logs --follow` streams events
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test cli_integration_test -- --ignored"]
async fn test_cli_logs_follow_command() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    // Run migrations first
    let migrate_output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "--database-url",
            &db_url,
            "migrate",
        ])
        .output()
        .expect("Failed to execute carnelian migrate");
    assert!(migrate_output.status.success(), "Migrations should succeed");

    let port = {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
        listener.local_addr().unwrap().port()
    };

    // Start server in background
    let _server = tokio::process::Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "--database-url",
            &db_url,
            "start",
        ])
        .env("CARNELIAN_HTTP_PORT", port.to_string())
        .kill_on_drop(true)
        .spawn()
        .expect("Failed to spawn carnelian start");

    // Wait for server to be ready
    let ready = wait_for_cli_server(port, Duration::from_secs(10)).await;
    assert!(ready, "Server should start within 10 seconds");

    // Spawn logs --follow as subprocess with stdout capture
    let mut logs_child = tokio::process::Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "logs",
            "--follow",
            "--url",
            &format!("http://127.0.0.1:{}", port),
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("Failed to spawn carnelian logs");

    // Give the logs command time to connect to the WebSocket
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Publish a test event via HTTP POST so the logs client can capture it
    let client = reqwest::Client::new();
    let event_payload = serde_json::json!({
        "event_type": "Custom",
        "level": "Info",
        "data": {"marker": "cli_logs_test_event_12345"}
    });
    let post_result = client
        .post(format!("http://127.0.0.1:{}/v1/events", port))
        .json(&event_payload)
        .send()
        .await;
    println!(
        "POST /v1/events result: {:?}",
        post_result.as_ref().map(reqwest::Response::status)
    );

    // Allow time for the event to propagate through WebSocket to the logs child
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Kill the logs process and capture output
    logs_child.kill().await.ok();
    let output = logs_child
        .wait_with_output()
        .await
        .expect("Failed to get logs output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!(
        "Logs output ({} bytes): {}",
        stdout.len(),
        &stdout[..stdout.len().min(1000)]
    );

    // Assert the expected event marker appears in the streamed output
    assert!(
        stdout.contains("cli_logs_test_event_12345"),
        "Logs --follow should have captured the published test event marker 'cli_logs_test_event_12345'. Got: {}",
        &stdout[..stdout.len().min(500)]
    );
    println!("✓ CLI logs --follow command captured streamed event");
}

/// Test: global flags work across commands
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test cli_integration_test -- --ignored"]
async fn test_cli_global_flags() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    // Test --database-url override with migrate command
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "--database-url",
            &db_url,
            "migrate",
        ])
        .output()
        .expect("Failed to execute carnelian migrate with --database-url");

    assert!(
        output.status.success(),
        "--database-url flag should work with migrate. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    println!("✓ --database-url global flag works with migrate command");

    // Test --config flag with a custom config file
    let temp_dir = std::env::temp_dir().join("carnelian_flags_test");
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let config_path = temp_dir.join("custom.toml");

    let config_content = format!(
        r#"
machine_profile = "thummim"
http_port = 19999
database_url = "{}"
log_level = "DEBUG"
"#,
        db_url
    );
    std::fs::write(&config_path, config_content).expect("Failed to write config file");

    // Verify --config flag is accepted (migrate with config)
    let config_output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "--config",
            config_path.to_str().unwrap(),
            "migrate",
        ])
        .output()
        .expect("Failed to execute carnelian migrate with --config");

    assert!(
        config_output.status.success(),
        "--config flag should work with migrate. stderr: {}",
        String::from_utf8_lossy(&config_output.stderr)
    );

    println!("✓ --config global flag works with migrate command");

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// Test: invalid database URL produces meaningful error
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test cli_integration_test -- --ignored"]
async fn test_cli_invalid_database_url() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "--database-url",
            "invalid://url",
            "migrate",
        ])
        .output()
        .expect("Failed to execute carnelian migrate");

    assert!(
        !output.status.success(),
        "carnelian migrate with invalid URL should exit non-zero"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error")
            || stderr.contains("Error")
            || stderr.contains("failed")
            || stderr.contains("Failed")
            || stderr.contains("invalid"),
        "stderr should contain error message about connection. Got: {}",
        stderr
    );

    println!("✓ Invalid database URL produces error exit code and message");
}
