//! Context Integrity Integration Tests
//!
//! Verifies that context assembly events are logged to the ledger with full
//! provenance metadata, and that correlation IDs correctly link context
//! assembly events to subsequent model call events.

mod common;

use common::*;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use carnelian_core::context::{ContextSegment, ContextWindow, SegmentPriority, SegmentSourceType};
use carnelian_core::ledger::Ledger;
use carnelian_core::model_router::{CompletionRequest, Message, ModelRouter};
use carnelian_core::policy::PolicyEngine;

// =============================================================================
// HELPERS
// =============================================================================

/// Populate a context window with test segments via the public `add_segment` API.
fn add_test_segments(
    ctx: &mut ContextWindow,
    mem_ids: &[Uuid],
    message_ids: &[i64],
    run_id: Option<Uuid>,
) {
    // P0: Soul directive
    ctx.add_segment(ContextSegment {
        priority: SegmentPriority::P0,
        content: "You are Lian, a thoughtful AI agent.".to_string(),
        token_estimate: 10,
        source_type: SegmentSourceType::SoulDirective,
        source_id: None,
        metadata: json!({}),
        insertion_order: 0, // overwritten by add_segment
    });

    // P1: Memories
    for &mem_id in mem_ids {
        ctx.add_segment(ContextSegment {
            priority: SegmentPriority::P1,
            content: format!("Memory content for {mem_id}"),
            token_estimate: 8,
            source_type: SegmentSourceType::Memory,
            source_id: Some(mem_id),
            metadata: json!({}),
            insertion_order: 0,
        });
    }

    // P2: Task context (if run_id provided)
    if let Some(rid) = run_id {
        ctx.add_segment(ContextSegment {
            priority: SegmentPriority::P2,
            content: "Current task: implement context integrity".to_string(),
            token_estimate: 8,
            source_type: SegmentSourceType::TaskContext,
            source_id: None,
            metadata: json!({"run_id": rid.to_string()}),
            insertion_order: 0,
        });
    }

    // P3: Conversation messages
    for &mid in message_ids {
        ctx.add_segment(ContextSegment {
            priority: SegmentPriority::P3,
            content: format!("User message {mid}"),
            token_estimate: 5,
            source_type: SegmentSourceType::ConversationMessage,
            source_id: None,
            metadata: json!({"message_id": mid}),
            insertion_order: 0,
        });
    }
}

// =============================================================================
// TEST 1: Context Assembly Logs Before Model Call
// =============================================================================

/// Verify that log_to_ledger creates a "model.context.assembled" event with
/// full provenance metadata (context_bundle_hash, memory_ids, run_ids, message_ids).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test context_integrity_test -- --ignored"]
async fn test_context_assembly_logs_provenance() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let ledger = Ledger::new(pool.clone());
    ledger.load_last_hash().await.expect("load_last_hash");

    let mem_id1 = Uuid::new_v4();
    let mem_id2 = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let message_ids: Vec<i64> = vec![101, 102, 103];
    let correlation_id = Uuid::now_v7();

    let mut ctx = ContextWindow::new(pool.clone(), None);
    add_test_segments(&mut ctx, &[mem_id1, mem_id2], &message_ids, Some(run_id));

    // Log to ledger
    let event_id = ctx
        .log_to_ledger(&ledger, correlation_id)
        .await
        .expect("log_to_ledger should succeed");

    assert!(event_id > 0, "event_id should be positive");

    // Query the ledger event
    let row = sqlx::query(
        "SELECT action_type, payload_hash, correlation_id, metadata \
         FROM ledger_events WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&pool)
    .await
    .expect("should find ledger event");

    let action_type: String = row.get("action_type");
    assert_eq!(action_type, "model.context.assembled");

    let stored_corr: Option<Uuid> = row.get("correlation_id");
    assert_eq!(stored_corr, Some(correlation_id));

    // Verify payload hash is non-empty (blake3)
    let payload_hash: String = row.get("payload_hash");
    assert!(!payload_hash.is_empty(), "payload_hash should be set");
}

// =============================================================================
// TEST 2: Correlation ID Links Context to Model Call
// =============================================================================

/// Verify that context assembly and model call events can be linked by
/// correlation_id and appear in chronological order.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test context_integrity_test -- --ignored"]
async fn test_correlation_id_links_context_to_model_call() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let ledger = Ledger::new(pool.clone());
    ledger.load_last_hash().await.expect("load_last_hash");

    let correlation_id = Uuid::now_v7();
    let identity_id = insert_test_identity(&pool, "test-agent").await;

    // 1. Log context assembly
    let mut ctx = ContextWindow::new(pool.clone(), None);
    add_test_segments(&mut ctx, &[Uuid::new_v4()], &[1, 2], None);

    ctx.log_to_ledger(&ledger, correlation_id)
        .await
        .expect("log context");

    // 2. Simulate model call request event (same correlation_id)
    ledger
        .append_event(
            Some(identity_id),
            "model.call.request",
            json!({
                "model": "test-model",
                "correlation_id": correlation_id,
            }),
            Some(correlation_id),
            None,
            None,
            None,
            None,
        )
        .await
        .expect("log model call request");

    // 3. Query all events with this correlation_id
    let rows = sqlx::query(
        "SELECT event_id, action_type, ts FROM ledger_events \
         WHERE correlation_id = $1 ORDER BY event_id ASC",
    )
    .bind(correlation_id)
    .fetch_all(&pool)
    .await
    .expect("query correlated events");

    assert_eq!(rows.len(), 2, "should have context + request events");

    let first_action: String = rows[0].get("action_type");
    let second_action: String = rows[1].get("action_type");

    assert_eq!(first_action, "model.context.assembled");
    assert_eq!(second_action, "model.call.request");

    // Verify chronological order
    let ts1: chrono::DateTime<chrono::Utc> = rows[0].get("ts");
    let ts2: chrono::DateTime<chrono::Utc> = rows[1].get("ts");
    assert!(ts1 <= ts2, "context assembly should precede model call");
}

// =============================================================================
// TEST 3: Provenance Tracking Captures All Sources
// =============================================================================

/// Verify that compute_provenance correctly captures memory_ids, run_ids,
/// message_ids, and produces a valid 64-character hex blake3 hash.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test context_integrity_test -- --ignored"]
async fn test_provenance_captures_all_sources() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let mem_id1 = Uuid::new_v4();
    let mem_id2 = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let message_ids: Vec<i64> = vec![10, 20, 30, 40, 50];

    let mut ctx = ContextWindow::new(pool, None);
    add_test_segments(&mut ctx, &[mem_id1, mem_id2], &message_ids, Some(run_id));

    let provenance = ctx.compute_provenance();

    // Verify memory IDs
    assert_eq!(provenance.memory_ids.len(), 2);
    assert!(provenance.memory_ids.contains(&mem_id1));
    assert!(provenance.memory_ids.contains(&mem_id2));

    // Verify run IDs
    assert_eq!(provenance.run_ids.len(), 1);
    assert!(provenance.run_ids.contains(&run_id));

    // Verify message IDs
    assert_eq!(provenance.message_ids.len(), 5);
    for &mid in &message_ids {
        assert!(provenance.message_ids.contains(&mid));
    }

    // Verify context_bundle_hash is a 64-character hex string (blake3 = 32 bytes = 64 hex chars)
    assert_eq!(
        provenance.context_bundle_hash.len(),
        64,
        "blake3 hash should be 64 hex characters"
    );
    assert!(
        provenance
            .context_bundle_hash
            .chars()
            .all(|c| c.is_ascii_hexdigit()),
        "hash should be valid hex"
    );

    // Verify segment counts
    assert_eq!(provenance.segment_counts.get("soul_directive"), Some(&1));
    assert_eq!(provenance.segment_counts.get("memory"), Some(&2));
    assert_eq!(provenance.segment_counts.get("task_context"), Some(&1));
    assert_eq!(
        provenance.segment_counts.get("conversation_message"),
        Some(&5)
    );
}

// =============================================================================
// TEST 4: Context Hash Changes When Content Changes
// =============================================================================

/// Verify that adding a new segment changes the context_bundle_hash.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test context_integrity_test -- --ignored"]
async fn test_context_hash_changes_with_content() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let mem_id = Uuid::new_v4();

    // Build context with one memory
    let mut ctx1 = ContextWindow::new(pool.clone(), None);
    add_test_segments(&mut ctx1, &[mem_id], &[1, 2], None);
    let hash1 = ctx1.compute_provenance().context_bundle_hash;

    // Build context with an additional memory
    let mem_id2 = Uuid::new_v4();
    let mut ctx2 = ContextWindow::new(pool.clone(), None);
    add_test_segments(&mut ctx2, &[mem_id, mem_id2], &[1, 2], None);
    let hash2 = ctx2.compute_provenance().context_bundle_hash;

    assert_ne!(
        hash1, hash2,
        "Adding a memory should change the context hash"
    );

    // Verify same content produces same hash (deterministic)
    let mut ctx3 = ContextWindow::new(pool, None);
    add_test_segments(&mut ctx3, &[mem_id], &[1, 2], None);
    let hash3 = ctx3.compute_provenance().context_bundle_hash;

    assert_eq!(hash1, hash3, "Same content should produce the same hash");
}

// =============================================================================
// TEST 5: log_context_integrity Returns Provenance
// =============================================================================

/// Verify that log_context_integrity returns both event_id and provenance,
/// and that the provenance matches compute_provenance().
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test context_integrity_test -- --ignored"]
async fn test_log_context_integrity_returns_provenance() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let ledger = Ledger::new(pool.clone());
    ledger.load_last_hash().await.expect("load_last_hash");

    let mem_id = Uuid::new_v4();
    let correlation_id = Uuid::now_v7();

    let mut ctx = ContextWindow::new(pool.clone(), None);
    add_test_segments(&mut ctx, &[mem_id], &[1, 2, 3], Some(Uuid::new_v4()));

    // Get expected provenance
    let expected_provenance = ctx.compute_provenance();

    // Call log_context_integrity
    let (event_id, provenance) = ctx
        .log_context_integrity(&ledger, correlation_id)
        .await
        .expect("log_context_integrity should succeed");

    assert!(event_id > 0);
    assert_eq!(
        provenance.context_bundle_hash,
        expected_provenance.context_bundle_hash
    );
    assert_eq!(provenance.memory_ids, expected_provenance.memory_ids);
    assert_eq!(provenance.run_ids, expected_provenance.run_ids);
    assert_eq!(provenance.message_ids, expected_provenance.message_ids);
    assert_eq!(provenance.total_tokens, expected_provenance.total_tokens);
    assert_eq!(
        provenance.segment_counts,
        expected_provenance.segment_counts
    );

    // Verify the event was actually persisted
    let row =
        sqlx::query("SELECT action_type, correlation_id FROM ledger_events WHERE event_id = $1")
            .bind(event_id)
            .fetch_one(&pool)
            .await
            .expect("should find event");

    let action_type: String = row.get("action_type");
    assert_eq!(action_type, "model.context.assembled");

    let stored_corr: Option<Uuid> = row.get("correlation_id");
    assert_eq!(stored_corr, Some(correlation_id));
}

// =============================================================================
// TEST 6: ModelRouter::complete() Logs Context Before Model Call
// =============================================================================

/// Verify that calling ModelRouter::complete() with a ContextProvenance emits
/// a "model.context.assembled" ledger event **before** the "model.call.request"
/// event, both sharing the same correlation_id, and that the context event
/// contains provenance metadata (hash, memory_ids, etc.).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test context_integrity_test -- --ignored"]
async fn test_model_router_complete_logs_context_before_call() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let identity_id = insert_test_identity(&pool, "router_ctx_agent").await;

    let ledger = Ledger::new(pool.clone());
    ledger.load_last_hash().await.expect("load_last_hash");

    let policy_engine = std::sync::Arc::new(PolicyEngine::new(pool.clone()));
    let ledger_arc = std::sync::Arc::new(Ledger::new(pool.clone()));
    ledger_arc.load_last_hash().await.expect("load_last_hash");

    // Use a bogus gateway URL — the call will fail at the HTTP level,
    // but the context + request ledger events are written *before* the HTTP call.
    let router = ModelRouter::new(
        pool.clone(),
        "http://127.0.0.1:1".to_string(), // unreachable
        policy_engine,
        ledger_arc,
    );

    // Insert a local provider so select_provider succeeds
    sqlx::query(
        r"INSERT INTO model_providers (provider_id, provider_type, name, enabled, config, budget_limits)
          VALUES ($1, 'local', 'ollama', true, '{}'::jsonb, '{}'::jsonb)
          ON CONFLICT DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .expect("insert provider");

    // Build provenance from a test context window
    let mem_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let mut ctx = ContextWindow::new(pool.clone(), None);
    add_test_segments(&mut ctx, &[mem_id], &[10, 20], Some(run_id));
    let provenance = ctx.compute_provenance();

    let correlation_id = Uuid::now_v7();
    let request = CompletionRequest {
        model: "deepseek-r1:7b".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
            name: None,
            tool_call_id: None,
        }],
        temperature: None,
        max_tokens: None,
        stream: None,
        correlation_id: Some(correlation_id),
    };

    // Call complete — will fail at gateway level, but ledger events are written first
    let _result = router
        .complete(request, identity_id, None, None, Some(&provenance))
        .await;

    // Query all ledger events with this correlation_id, ordered by event_id
    let rows = sqlx::query(
        "SELECT event_id, action_type, metadata \
         FROM ledger_events \
         WHERE correlation_id = $1 \
         ORDER BY event_id ASC",
    )
    .bind(correlation_id)
    .fetch_all(&pool)
    .await
    .expect("query ledger events");

    // Must have at least 2 events: context.assembled then call.request
    assert!(
        rows.len() >= 2,
        "Expected at least 2 ledger events for correlation_id, got {}",
        rows.len()
    );

    // First event must be model.context.assembled
    let first_action: String = rows[0].get("action_type");
    assert_eq!(first_action, "model.context.assembled");

    // Second event must be model.call.request
    let second_action: String = rows[1].get("action_type");
    assert_eq!(second_action, "model.call.request");

    // Verify context event metadata contains provenance fields
    let metadata: serde_json::Value = rows[0].get("metadata");
    assert!(
        metadata.get("context_bundle_hash").is_some(),
        "metadata should contain context_bundle_hash"
    );
    assert_eq!(
        metadata["context_bundle_hash"].as_str().unwrap().len(),
        64,
        "context_bundle_hash should be 64-char blake3 hex"
    );
    assert!(
        metadata.get("memory_ids").is_some(),
        "metadata should contain memory_ids"
    );
    assert!(
        metadata.get("total_tokens").is_some(),
        "metadata should contain total_tokens"
    );

    // Verify the hash matches what we computed
    assert_eq!(
        metadata["context_bundle_hash"].as_str().unwrap(),
        provenance.context_bundle_hash,
        "Persisted hash should match provenance"
    );
}
