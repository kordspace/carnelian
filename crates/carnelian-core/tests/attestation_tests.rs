//! Worker Attestation Integration Tests
//!
//! Verifies attestation verification, recording, quarantine workflow,
//! and database operations for the worker attestation system.

mod common;

use carnelian_core::attestation::{
    get_quarantined_workers, is_worker_quarantined, quarantine_worker, record_attestation,
    verify_attestation, WorkerAttestation,
};
use common::*;

// =============================================================================
// TEST 1: Attestation Verification Success
// =============================================================================

/// Verify that a worker with matching attestation values passes verification.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test attestation_tests -- --ignored"]
async fn test_attestation_verification_success() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let attestation = WorkerAttestation {
        worker_id: "test-worker-1".to_string(),
        last_ledger_head: "abc123".to_string(),
        build_checksum: "def456".to_string(),
        config_version: "v1".to_string(),
    };

    let result = verify_attestation(&pool, &attestation, "abc123", "def456", "v1")
        .await
        .unwrap();

    assert!(result.verified);
    assert!(result.mismatch_reason.is_none());
    assert_eq!(result.worker_id, "test-worker-1");
}

// =============================================================================
// TEST 2: Attestation Verification Mismatch (ledger head)
// =============================================================================

/// Verify that a worker with a mismatched ledger head fails verification.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test attestation_tests -- --ignored"]
async fn test_attestation_verification_ledger_head_mismatch() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let attestation = WorkerAttestation {
        worker_id: "test-worker-2".to_string(),
        last_ledger_head: "wrong_hash".to_string(),
        build_checksum: "def456".to_string(),
        config_version: "v1".to_string(),
    };

    let result = verify_attestation(&pool, &attestation, "abc123", "def456", "v1")
        .await
        .unwrap();

    assert!(!result.verified);
    assert!(result.mismatch_reason.is_some());
    let reason = result.mismatch_reason.unwrap();
    assert!(
        reason.contains("ledger_head mismatch"),
        "Expected ledger_head mismatch, got: {reason}"
    );
}

// =============================================================================
// TEST 3: Attestation Verification Multiple Mismatches
// =============================================================================

/// Verify that multiple mismatches are all reported.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test attestation_tests -- --ignored"]
async fn test_attestation_verification_multiple_mismatches() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let attestation = WorkerAttestation {
        worker_id: "test-worker-3".to_string(),
        last_ledger_head: "wrong_hash".to_string(),
        build_checksum: "wrong_checksum".to_string(),
        config_version: "wrong_version".to_string(),
    };

    let result = verify_attestation(&pool, &attestation, "abc123", "def456", "v1")
        .await
        .unwrap();

    assert!(!result.verified);
    let reason = result.mismatch_reason.unwrap();
    assert!(
        reason.contains("ledger_head mismatch"),
        "Missing ledger_head mismatch in: {reason}"
    );
    assert!(
        reason.contains("build_checksum mismatch"),
        "Missing build_checksum mismatch in: {reason}"
    );
    assert!(
        reason.contains("config_version mismatch"),
        "Missing config_version mismatch in: {reason}"
    );
}

// =============================================================================
// TEST 4: Record and Query Attestation
// =============================================================================

/// Verify that attestations can be recorded and queried.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test attestation_tests -- --ignored"]
async fn test_record_attestation() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let attestation = WorkerAttestation {
        worker_id: "test-worker-4".to_string(),
        last_ledger_head: "abc123".to_string(),
        build_checksum: "def456".to_string(),
        config_version: "v1".to_string(),
    };

    record_attestation(&pool, &attestation).await.unwrap();

    // Verify the record exists
    let row: (String, String, String) = sqlx::query_as(
        "SELECT last_ledger_head, build_checksum, config_version FROM worker_attestations WHERE worker_id = $1"
    )
    .bind("test-worker-4")
    .fetch_one(&pool)
    .await
    .expect("Should find attestation record");

    assert_eq!(row.0, "abc123");
    assert_eq!(row.1, "def456");
    assert_eq!(row.2, "v1");
}

// =============================================================================
// TEST 5: Quarantine Worker
// =============================================================================

/// Verify the full quarantine workflow: record, quarantine, check.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test attestation_tests -- --ignored"]
async fn test_quarantine_worker() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let attestation = WorkerAttestation {
        worker_id: "test-worker-5".to_string(),
        last_ledger_head: "abc123".to_string(),
        build_checksum: "def456".to_string(),
        config_version: "v1".to_string(),
    };

    // First record the attestation
    record_attestation(&pool, &attestation).await.unwrap();

    // Worker should not be quarantined yet
    let quarantined = is_worker_quarantined(&pool, "test-worker-5").await.unwrap();
    assert!(!quarantined);

    // Quarantine the worker
    quarantine_worker(&pool, "test-worker-5", "test mismatch")
        .await
        .unwrap();

    // Worker should now be quarantined
    let quarantined = is_worker_quarantined(&pool, "test-worker-5").await.unwrap();
    assert!(quarantined);

    // Should appear in quarantined workers list
    let quarantined_list = get_quarantined_workers(&pool).await.unwrap();
    assert!(quarantined_list.contains(&"test-worker-5".to_string()));
}

// =============================================================================
// TEST 6: Unknown Worker Not Quarantined
// =============================================================================

/// Verify that an unknown worker is not considered quarantined.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test attestation_tests -- --ignored"]
async fn test_unknown_worker_not_quarantined() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let quarantined = is_worker_quarantined(&pool, "nonexistent-worker")
        .await
        .unwrap();
    assert!(!quarantined);
}

// =============================================================================
// TEST 7: Attestation Upsert (record twice)
// =============================================================================

/// Verify that recording an attestation twice updates the existing record.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test attestation_tests -- --ignored"]
async fn test_attestation_upsert() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let attestation1 = WorkerAttestation {
        worker_id: "test-worker-7".to_string(),
        last_ledger_head: "hash_v1".to_string(),
        build_checksum: "checksum_v1".to_string(),
        config_version: "v1".to_string(),
    };

    record_attestation(&pool, &attestation1).await.unwrap();

    // Update with new values
    let attestation2 = WorkerAttestation {
        worker_id: "test-worker-7".to_string(),
        last_ledger_head: "hash_v2".to_string(),
        build_checksum: "checksum_v2".to_string(),
        config_version: "v2".to_string(),
    };

    record_attestation(&pool, &attestation2).await.unwrap();

    // Should have the updated values
    let row: (String, String, String) = sqlx::query_as(
        "SELECT last_ledger_head, build_checksum, config_version FROM worker_attestations WHERE worker_id = $1"
    )
    .bind("test-worker-7")
    .fetch_one(&pool)
    .await
    .expect("Should find attestation record");

    assert_eq!(row.0, "hash_v2");
    assert_eq!(row.1, "checksum_v2");
    assert_eq!(row.2, "v2");
}
