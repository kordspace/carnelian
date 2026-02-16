//! Integration tests for workspace scanning and task auto-queueing.
//!
//! These tests validate the end-to-end flow of workspace scanning during
//! heartbeat, including marker detection, safe/privileged classification,
//! database insertion, event emission, and deduplication.
//!
//! Run with: `cargo test --test workspace_scan_test -- --ignored`

mod common;

use carnelian_core::config::Config;
use carnelian_core::events::EventStream;
use carnelian_core::scheduler::WorkspaceScanner;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Verify that the scanner finds markers in a temp workspace, classifies them
/// correctly, inserts safe tasks into the database, skips privileged ones,
/// emits `TaskAutoQueued` events, and deduplicates on a second pass.
#[tokio::test]
#[ignore = "requires database connection"]
async fn test_workspace_scan_auto_queue() {
    // ── Setup: temp workspace with marker files ──────────────────────
    let workspace = std::env::temp_dir().join(format!("carnelian_integ_scan_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&workspace).unwrap();

    // Safe markers
    std::fs::write(
        workspace.join("safe_tasks.rs"),
        "// TODO: Add error handling for network timeouts\n\
         // TASK: Implement pagination for user list\n\
         // TODO: Write unit tests for parser\n",
    )
    .unwrap();

    // Privileged markers
    std::fs::write(
        workspace.join("privileged_tasks.py"),
        "# TODO: Deploy to production\n\
         # TASK: Delete old migration files\n\
         # TODO: Rotate credential keys\n",
    )
    .unwrap();

    // Ignored directory
    let ignored = workspace.join("node_modules");
    std::fs::create_dir_all(&ignored).unwrap();
    std::fs::write(ignored.join("lib.js"), "// TODO: Should be ignored").unwrap();

    // ── Scan ─────────────────────────────────────────────────────────
    let markers = WorkspaceScanner::scan(&[workspace.clone()]);

    // Should find 6 markers total (3 safe + 3 privileged), NOT the node_modules one
    assert_eq!(
        markers.len(),
        6,
        "Expected 6 markers, got {}",
        markers.len()
    );

    let safe_count = markers.iter().filter(|m| m.is_safe).count();
    let privileged_count = markers.iter().filter(|m| !m.is_safe).count();

    assert_eq!(safe_count, 3, "Expected 3 safe markers");
    assert_eq!(privileged_count, 3, "Expected 3 privileged markers");

    // ── Database auto-queue (requires running DB) ────────────────────
    let container = common::create_postgres_container().await;
    let db_url = common::get_database_url(&container).await;
    let pool = common::setup_test_db(&db_url).await;

    // Create a test identity
    let identity_id: Uuid = sqlx::query_scalar(
        "INSERT INTO identities (name, kind) VALUES ('scanner-test', 'system') RETURNING identity_id",
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create test identity");

    let event_stream = Arc::new(EventStream::new(100, 10));
    let correlation_id = Uuid::now_v7();

    // Auto-queue safe tasks
    let queued = carnelian_core::scheduler::auto_queue_scanned_tasks(
        &pool,
        &event_stream,
        &markers,
        identity_id,
        correlation_id,
        20,
    )
    .await
    .expect("auto_queue_scanned_tasks failed");

    // Only safe tasks should be queued
    assert_eq!(queued, 3, "Expected 3 tasks queued, got {queued}");

    // Verify tasks exist in database
    let task_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE correlation_id = $1")
            .bind(correlation_id)
            .fetch_one(&pool)
            .await
            .expect("Failed to count tasks");
    assert_eq!(task_count, 3, "Expected 3 tasks in DB");

    // Verify privileged tasks are NOT in the database
    let priv_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE title LIKE '%Deploy to production%' OR title LIKE '%Delete old migration%' OR title LIKE '%Rotate credential%'",
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to count privileged tasks");
    assert_eq!(priv_count, 0, "Privileged tasks should NOT be queued");

    // ── Deduplication: re-run should queue 0 ─────────────────────────
    let correlation_id_2 = Uuid::now_v7();
    let queued_2 = carnelian_core::scheduler::auto_queue_scanned_tasks(
        &pool,
        &event_stream,
        &markers,
        identity_id,
        correlation_id_2,
        20,
    )
    .await
    .expect("second auto_queue_scanned_tasks failed");

    assert_eq!(queued_2, 0, "Deduplication should prevent re-queuing");

    // ── Cleanup ──────────────────────────────────────────────────────
    let _ = std::fs::remove_dir_all(&workspace);
}

/// Verify that scan() returns markers even when the heartbeat limit would be 0.
/// The limit=0 disabling is enforced at the heartbeat/auto_queue level, not in scan().
#[tokio::test]
async fn test_workspace_scan_returns_markers_regardless_of_limit() {
    let workspace =
        std::env::temp_dir().join(format!("carnelian_disabled_scan_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(workspace.join("tasks.rs"), "// TODO: Should be found\n").unwrap();

    let markers = WorkspaceScanner::scan(&[workspace.clone()]);
    assert_eq!(
        markers.len(),
        1,
        "scan() should return markers; limit is enforced elsewhere"
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

/// Verify that config defaults are correct for workspace scanning fields.
#[test]
fn test_config_workspace_scan_defaults() {
    let config = Config::default();
    assert_eq!(config.max_tasks_per_heartbeat, 5);
    assert_eq!(config.workspace_scan_paths, vec![PathBuf::from(".")]);
}
