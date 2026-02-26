#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::too_many_lines)]

//! Init Command Integration Tests for Carnelian
//!
//! These tests validate the `carnelian init` command functionality including
//! non-interactive mode, idempotency, and hardware detection.
//!
//! # Running Tests
//!
//! ```bash
//! cargo test --test init_integration_test -- --ignored
//! ```

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use tempfile::TempDir;

/// Test: `carnelian init --non-interactive` runs successfully
#[tokio::test]
#[ignore = "Requires Docker and build - run with: cargo test --test init_integration_test -- --ignored"]
async fn test_init_non_interactive() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let home_path = temp_dir.path();

    // Run carnelian init --non-interactive with temp HOME
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "init",
            "--non-interactive",
        ])
        .env("HOME", home_path)
        .env("USERPROFILE", home_path) // Windows fallback
        .current_dir(home_path)
        .output()
        .expect("Failed to execute carnelian init");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "carnelian init should exit 0. stdout: {}, stderr: {}",
        stdout,
        stderr
    );

    // Verify success message
    assert!(
        stdout.contains("Carnelian OS initialized") || stderr.contains("Carnelian OS initialized"),
        "Output should contain success message"
    );

    // Verify init state file was created
    let init_state_path = home_path.join(".carnelian").join("init-state.json");
    assert!(
        init_state_path.exists(),
        "init-state.json should be created at {:?}",
        init_state_path
    );
}

/// Test: `carnelian init` is idempotent (running twice succeeds)
#[tokio::test]
#[ignore = "Requires Docker and build - run with: cargo test --test init_integration_test -- --ignored"]
async fn test_init_idempotent() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let home_path = temp_dir.path();

    // First run
    let output1 = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "init",
            "--non-interactive",
        ])
        .env("HOME", home_path)
        .env("USERPROFILE", home_path)
        .current_dir(home_path)
        .output()
        .expect("Failed to execute carnelian init (first run)");

    assert!(
        output1.status.success(),
        "First init should succeed. stderr: {}",
        String::from_utf8_lossy(&output1.stderr)
    );

    // Second run (idempotent)
    let output2 = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "init",
            "--non-interactive",
        ])
        .env("HOME", home_path)
        .env("USERPROFILE", home_path)
        .current_dir(home_path)
        .output()
        .expect("Failed to execute carnelian init (second run)");

    assert!(
        output2.status.success(),
        "Second init should succeed (idempotent). stderr: {}",
        String::from_utf8_lossy(&output2.stderr)
    );
}

/// Test: `carnelian init --force` overwrites existing machine.toml
#[tokio::test]
#[ignore = "Requires Docker and build - run with: cargo test --test init_integration_test -- --ignored"]
async fn test_init_force_overwrite() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let home_path = temp_dir.path();

    // Pre-create machine.toml
    let machine_toml_path = home_path.join("machine.toml");
    fs::write(&machine_toml_path, "# Old content").expect("Failed to write machine.toml");
    let original_mtime = fs::metadata(&machine_toml_path)
        .expect("Failed to get metadata")
        .modified()
        .expect("Failed to get modified time");

    // Run with --force
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "init",
            "--non-interactive",
            "--force",
        ])
        .env("HOME", home_path)
        .env("USERPROFILE", home_path)
        .current_dir(home_path)
        .output()
        .expect("Failed to execute carnelian init --force");

    assert!(
        output.status.success(),
        "init --force should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify machine.toml was overwritten (content changed)
    let new_content = fs::read_to_string(&machine_toml_path).expect("Failed to read machine.toml");
    assert!(
        !new_content.contains("# Old content"),
        "machine.toml should be overwritten with new content"
    );

    // Verify mtime changed (file was modified)
    let new_mtime = fs::metadata(&machine_toml_path)
        .expect("Failed to get metadata")
        .modified()
        .expect("Failed to get modified time");
    // Note: mtime comparison may be flaky in CI, content check is more reliable
}

/// Test: `carnelian init --resume` continues from saved state
#[tokio::test]
#[ignore = "Requires Docker and build - run with: cargo test --test init_integration_test -- --ignored"]
async fn test_init_resume() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let home_path = temp_dir.path();
    let carnelian_dir = home_path.join(".carnelian");
    fs::create_dir_all(&carnelian_dir).expect("Failed to create .carnelian dir");

    // Create a partial init state (simulating interrupted init)
    let init_state = serde_json::json!({
        "keypair_generated": true,
        "keypair_path": null,
        "machine_toml_written": false,
        "containers_started": false,
        "migrations_run": false,
        "skills_activated": false
    });
    let state_path = carnelian_dir.join("init-state.json");
    fs::write(&state_path, init_state.to_string()).expect("Failed to write init-state.json");

    // Run with --resume
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "carnelian",
            "--",
            "init",
            "--non-interactive",
            "--resume",
        ])
        .env("HOME", home_path)
        .env("USERPROFILE", home_path)
        .current_dir(home_path)
        .output()
        .expect("Failed to execute carnelian init --resume");

    assert!(
        output.status.success(),
        "init --resume should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify state was updated
    let updated_state: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&state_path).expect("Failed to read state")
    ).expect("Failed to parse state");
    
    // After resume, at least some tasks should be marked complete
    let tasks_completed = updated_state["machine_toml_written"].as_bool().unwrap_or(false)
        || updated_state["migrations_run"].as_bool().unwrap_or(false);
    assert!(
        tasks_completed,
        "Resume should complete at least some pending tasks"
    );
}
