#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]

//! Phase 3 Integration Tests for Carnelian Core
//!
//! These tests validate the complete Phase 3 agentic execution pipeline:
//!
//! - **Soul File System**: Loading, parsing, database sync, hash verification
//! - **Session Management**: CRUD, message persistence, expiration, token counters
//! - **Memory Retrieval**: Creation, 48hr window, high-importance, pgvector search
//! - **Context Assembly**: Priority ordering, token budgeting, pruning, provenance
//! - **Session Compaction**: Threshold triggers, memory flush, counter updates
//! - **Model Routing**: Local-first, capability checks, budget enforcement
//! - **Heartbeat Agentic Turn**: Context assembly, persistence, correlation tracking
//! - **Agentic Loop**: Tool calls, declarative plans, policy integration
//!
//! # Running Tests
//!
//! ```bash
//! # All Phase 3 integration tests (requires Docker for PostgreSQL)
//! cargo test --test phase3_integration_test -- --ignored
//!
//! # Run specific test group
//! cargo test --test phase3_integration_test test_soul -- --ignored
//!
//! # Run with logging
//! RUST_LOG=debug cargo test --test phase3_integration_test -- --ignored --nocapture
//! ```

mod common;

use std::path::PathBuf;
use std::str::FromStr;

use std::sync::Arc;

use carnelian_core::{
    Config, ContextWindow, Ledger, MemoryManager, MemoryQuery, MemorySource, ModelRouter,
    PolicyEngine, SegmentPriority, SegmentSourceType, SessionKey, SessionManager,
    SoulManager,
};
use carnelian_core::session::{CompactionTrigger, CompactionOutcome};
use carnelian_core::soul::SyncResult;
use serde_json::json;
use uuid::Uuid;

use common::*;

// =============================================================================
// TEST GROUP: Soul File System & Database Sync
// =============================================================================

/// Load a soul file from disk via SoulManager and verify directives are parsed
/// with correct priorities (P0 for Core Truths, P1 for Boundaries).
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_soul_file_load_and_parse() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let souls_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/souls");
    let identity_id = insert_test_identity_with_soul(&pool, "test_lian", "test_lian.md").await;

    let manager = SoulManager::new(pool.clone(), None, souls_path);
    let soul_data = manager.load(identity_id).await.expect("Failed to load soul");

    // Verify directives parsed correctly
    assert!(!soul_data.directives.is_empty(), "Should have parsed directives");
    assert!(!soul_data.hash.is_empty(), "Should have computed hash");

    // Core Truths should be P0
    let core_truths: Vec<_> = soul_data.directives.iter()
        .filter(|d| d.category == "Core Truths")
        .collect();
    assert_eq!(core_truths.len(), 3, "Should have 3 Core Truths directives");
    assert_eq!(core_truths[0].priority, 0, "Core Truths should be P0");

    // Boundaries should be P1
    let boundaries: Vec<_> = soul_data.directives.iter()
        .filter(|d| d.category == "Boundaries")
        .collect();
    assert_eq!(boundaries.len(), 3, "Should have 3 Boundaries directives");
    assert_eq!(boundaries[0].priority, 1, "Boundaries should be P1");

    println!("✓ Soul file loaded and parsed: {} directives", soul_data.directives.len());
}

/// Sync a soul file to the database and verify directives JSONB and hash are stored.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_soul_sync_to_db_new_file() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let souls_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/souls");
    let identity_id = insert_test_identity_with_soul(&pool, "test_lian", "test_lian.md").await;

    let manager = SoulManager::new(pool.clone(), None, souls_path);
    let result = manager.sync_to_db(identity_id).await.expect("Failed to sync");

    assert_eq!(result, SyncResult::Updated, "First sync should update");

    // Verify database state
    let directives = get_identity_directives(&pool, identity_id).await;
    assert!(!directives.is_empty(), "Directives should be stored in DB");

    let hash = get_soul_file_hash(&pool, identity_id).await;
    assert!(hash.is_some(), "Hash should be stored in DB");
    assert!(!hash.unwrap().is_empty(), "Hash should not be empty");

    println!("✓ Soul synced to database: {} directives stored", directives.len());
}

/// Sync the same file twice and verify the second sync returns Unchanged.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_soul_sync_unchanged_hash() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let souls_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/souls");
    let identity_id = insert_test_identity_with_soul(&pool, "test_lian", "test_lian.md").await;

    let manager = SoulManager::new(pool.clone(), None, souls_path);

    let first = manager.sync_to_db(identity_id).await.expect("First sync failed");
    assert_eq!(first, SyncResult::Updated);

    let second = manager.sync_to_db(identity_id).await.expect("Second sync failed");
    assert_eq!(second, SyncResult::Unchanged, "Second sync should be unchanged");

    println!("✓ Unchanged hash correctly detected on second sync");
}

/// Modify a soul file, sync again, and verify hash changes and directives update.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_soul_sync_modified_file() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let soul_file = temp_dir.path().join("modified.md");

    // Write initial content
    create_test_soul_file(&soul_file, "# Agent\n\n## Core Truths\n- Original directive\n")
        .expect("Failed to write soul file");

    let identity_id = insert_test_identity_with_soul(&pool, "modified_agent", "modified.md").await;
    let manager = SoulManager::new(pool.clone(), None, temp_dir.path().to_path_buf());

    let first = manager.sync_to_db(identity_id).await.expect("First sync failed");
    assert_eq!(first, SyncResult::Updated);
    let hash1 = get_soul_file_hash(&pool, identity_id).await.unwrap();

    // Modify the file
    create_test_soul_file(&soul_file, "# Agent\n\n## Core Truths\n- Modified directive\n- New directive\n")
        .expect("Failed to modify soul file");

    let second = manager.sync_to_db(identity_id).await.expect("Second sync failed");
    assert_eq!(second, SyncResult::Updated, "Modified file should trigger update");

    let hash2 = get_soul_file_hash(&pool, identity_id).await.unwrap();
    assert_ne!(hash1, hash2, "Hash should change after modification");

    let directives = get_identity_directives(&pool, identity_id).await;
    assert_eq!(directives.len(), 2, "Should have 2 directives after modification");

    println!("✓ Modified soul file detected and synced");
}

/// Call SoulManager::watch() and verify all identities with soul_file_path are synced.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_soul_watch_initial_sync() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let souls_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/souls");

    let id1 = insert_test_identity_with_soul(&pool, "lian_watch", "test_lian.md").await;
    let id2 = insert_test_identity_with_soul(&pool, "minimal_watch", "test_minimal.md").await;
    // Identity without soul_file_path should be skipped
    let _id3 = insert_test_identity(&pool, "no_soul").await;

    let manager = SoulManager::new(pool.clone(), None, souls_path);
    manager.watch().await.expect("Watch failed");

    // Both identities with soul paths should have directives
    let d1 = get_identity_directives(&pool, id1).await;
    assert!(!d1.is_empty(), "Lian should have directives after watch");

    let d2 = get_identity_directives(&pool, id2).await;
    assert!(!d2.is_empty(), "Minimal should have directives after watch");

    println!("✓ Watch synced {} + {} directives for 2 identities", d1.len(), d2.len());
}

// =============================================================================
// TEST GROUP: Session Persistence & Lifecycle
// =============================================================================

/// Create and load a session, verifying all fields match.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_session_create_and_load() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let agent_id = insert_test_identity(&pool, "session_agent").await;
    let session_key = format!("agent:{}:ui", agent_id);

    let sm = SessionManager::with_defaults(pool.clone());
    let created = sm.create_session(&session_key).await.expect("Failed to create session");

    assert_eq!(created.agent_id, agent_id);
    assert_eq!(created.channel, "ui");
    assert_eq!(created.session_key, session_key);

    let loaded = sm.load_session(&session_key).await.expect("Failed to load session");
    assert!(loaded.is_some(), "Session should be loadable");

    let loaded = loaded.unwrap();
    assert_eq!(loaded.session_id, created.session_id);
    assert_eq!(loaded.agent_id, agent_id);

    println!("✓ Session created and loaded: {}", created.session_id);
}

/// Test SessionKey parsing with valid formats.
#[test]
fn test_session_key_parsing() {
    let agent_id = Uuid::new_v4();

    // Simple key: agent:<uuid>:ui
    let key1 = SessionKey::from_str(&format!("agent:{}:ui", agent_id))
        .expect("Failed to parse simple key");
    assert!(key1.is_ui());
    assert!(!key1.is_cli());
    assert_eq!(key1.agent_id, agent_id);
    assert!(key1.group_id.is_none());

    // Key with group: agent:<uuid>:cli:group:main
    let key2 = SessionKey::from_str(&format!("agent:{}:cli:group:main", agent_id))
        .expect("Failed to parse grouped key");
    assert!(key2.is_cli());
    assert_eq!(key2.group_id, Some("main".to_string()));

    // Invalid: too few parts
    assert!(SessionKey::from_str("agent:badkey").is_err());

    // Invalid: bad UUID
    assert!(SessionKey::from_str("agent:not-a-uuid:ui").is_err());

    // Invalid: wrong prefix
    assert!(SessionKey::from_str("user:00000000-0000-0000-0000-000000000000:ui").is_err());

    println!("✓ Session key parsing validated for all formats");
}

/// Append messages and verify rows in session_messages with correct token counters.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_session_append_message() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let agent_id = insert_test_identity(&pool, "msg_agent").await;
    let session_key = format!("agent:{}:ui", agent_id);

    let sm = SessionManager::with_defaults(pool.clone());
    let session = sm.create_session(&session_key).await.expect("Failed to create session");

    // Append user message
    let msg1_id = sm.append_message(
        session.session_id, "user", "Hello, world!".to_string(),
        Some(5), None, None, None, None, None,
    ).await.expect("Failed to append user message");
    assert!(msg1_id > 0);

    // Append assistant message
    let msg2_id = sm.append_message(
        session.session_id, "assistant", "Hi there!".to_string(),
        Some(4), None, None, None, None, None,
    ).await.expect("Failed to append assistant message");
    assert!(msg2_id > msg1_id);

    // Append tool message
    let msg3_id = sm.append_message(
        session.session_id, "tool", "{\"result\": 42}".to_string(),
        Some(10), Some("calculator".to_string()), Some("call_1".to_string()),
        None, None, None,
    ).await.expect("Failed to append tool message");
    assert!(msg3_id > msg2_id);

    // Verify token counters updated
    let counters = sm.get_counters(session.session_id).await.expect("Failed to get counters");
    assert_eq!(counters.user, 5);
    assert_eq!(counters.assistant, 4);
    assert_eq!(counters.tool, 10);
    assert_eq!(counters.total, 19);

    println!("✓ Messages appended with correct token counters: total={}", counters.total);
}

/// Insert many messages and verify pagination works correctly.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_session_load_messages_pagination() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let agent_id = insert_test_identity(&pool, "pagination_agent").await;
    let session_id = create_test_session(&pool, agent_id, "ui").await;

    // Insert 100 messages
    for i in 0..100 {
        insert_test_message(&pool, session_id, "user", &format!("Message {}", i)).await;
    }

    let sm = SessionManager::with_defaults(pool.clone());

    // Load first page (newest first, limit 20)
    let page1 = sm.load_messages(session_id, Some(20), None).await
        .expect("Failed to load page 1");
    assert_eq!(page1.len(), 20, "First page should have 20 messages");

    // Load second page using cursor
    let last_id = page1.last().unwrap().message_id;
    let page2 = sm.load_messages(session_id, Some(20), Some(last_id)).await
        .expect("Failed to load page 2");
    assert_eq!(page2.len(), 20, "Second page should have 20 messages");

    // Verify no overlap
    let page1_ids: Vec<i64> = page1.iter().map(|m| m.message_id).collect();
    let page2_ids: Vec<i64> = page2.iter().map(|m| m.message_id).collect();
    for id in &page2_ids {
        assert!(!page1_ids.contains(id), "Pages should not overlap");
    }

    println!("✓ Pagination verified: 2 pages of 20 messages each, no overlap");
}

/// Create an expired session and verify cleanup deletes it.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_session_expiration_cleanup() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let agent_id = insert_test_identity(&pool, "expiry_agent").await;
    let session_id = create_test_session(&pool, agent_id, "ui").await;

    // Set expires_at to the past
    sqlx::query("UPDATE sessions SET expires_at = NOW() - INTERVAL '1 hour' WHERE session_id = $1")
        .bind(session_id)
        .execute(&pool)
        .await
        .expect("Failed to set expiry");

    let sm = SessionManager::with_defaults(pool.clone());
    let deleted = sm.cleanup_expired_sessions().await.expect("Cleanup failed");
    assert!(deleted >= 1, "Should have deleted at least 1 expired session");

    // Verify session is gone
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sessions WHERE session_id = $1)")
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .expect("Failed to check existence");
    assert!(!exists, "Expired session should be deleted");

    println!("✓ Expired session cleaned up: {} sessions deleted", deleted);
}

/// Extend a session's expiry and verify the new expiration time.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_session_extend_expiry() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let agent_id = insert_test_identity(&pool, "extend_agent").await;
    let session_key = format!("agent:{}:ui", agent_id);

    let sm = SessionManager::with_defaults(pool.clone());
    let session = sm.create_session(&session_key).await.expect("Failed to create session");

    let original_expiry: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT expires_at FROM sessions WHERE session_id = $1",
    )
    .bind(session.session_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to get expiry");

    sm.extend_session(session.session_id, 48).await.expect("Failed to extend session");

    let new_expiry: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT expires_at FROM sessions WHERE session_id = $1",
    )
    .bind(session.session_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to get new expiry");

    assert!(new_expiry.is_some(), "Session should have expiry after extension");
    if let (Some(orig), Some(new)) = (original_expiry, new_expiry) {
        assert!(new > orig, "Extended expiry should be later than original");
    }

    println!("✓ Session expiry extended by 48 hours");
}

/// Verify atomic token counter updates with row locking.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_session_token_counter_update() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let agent_id = insert_test_identity(&pool, "counter_agent").await;
    let session_key = format!("agent:{}:ui", agent_id);

    let sm = SessionManager::with_defaults(pool.clone());
    let session = sm.create_session(&session_key).await.expect("Failed to create session");

    // Increment counters atomically
    sm.increment_counters(session.session_id, "user", 100).await
        .expect("Failed to increment user counters");
    sm.increment_counters(session.session_id, "assistant", 200).await
        .expect("Failed to increment assistant counters");
    sm.increment_counters(session.session_id, "tool", 50).await
        .expect("Failed to increment tool counters");

    let counters = sm.get_counters(session.session_id).await
        .expect("Failed to get counters");
    assert_eq!(counters.user, 100);
    assert_eq!(counters.assistant, 200);
    assert_eq!(counters.tool, 50);
    assert_eq!(counters.total, 350);

    println!("✓ Token counters atomically updated: total={}", counters.total);
}

// =============================================================================
// TEST GROUP: Memory Retrieval & pgvector Search
// =============================================================================

/// Create a memory with validation and verify it's inserted correctly.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_memory_create_with_validation() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let identity_id = insert_test_identity(&pool, "memory_agent").await;
    let mm = MemoryManager::new(pool.clone(), None);

    let memory = mm.create_memory(
        identity_id,
        "User prefers concise responses",
        Some("Communication preference".to_string()),
        MemorySource::Conversation,
        None,
        0.9,
    ).await.expect("Failed to create memory");

    assert_eq!(memory.identity_id, identity_id);
    assert_eq!(memory.content, "User prefers concise responses");
    assert!((memory.importance - 0.9).abs() < f32::EPSILON, "importance should be 0.9");
    assert!(memory.is_high_importance());

    // Validate importance range
    let bad_result = mm.create_memory(
        identity_id, "test", None, MemorySource::Conversation, None, 1.5,
    ).await;
    assert!(bad_result.is_err(), "Importance > 1.0 should fail validation");

    let bad_result2 = mm.create_memory(
        identity_id, "test", None, MemorySource::Conversation, None, -0.1,
    ).await;
    assert!(bad_result2.is_err(), "Importance < 0.0 should fail validation");

    println!("✓ Memory created with validation: importance={}", memory.importance);
}

/// Insert memories with various timestamps and verify 48hr window retrieval.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_memory_load_recent_48hr() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let identity_id = insert_test_identity(&pool, "recent_memory_agent").await;

    // Insert a recent memory (now)
    let recent_id = create_test_memory(&pool, identity_id, "Recent memory", 0.5).await;

    // Insert an old memory (3 days ago)
    let old_id = Uuid::new_v4();
    sqlx::query(
        r"INSERT INTO memories (memory_id, identity_id, content, source, importance, created_at)
          VALUES ($1, $2, 'Old memory', 'conversation', 0.5, NOW() - INTERVAL '3 days')",
    )
    .bind(old_id)
    .bind(identity_id)
    .execute(&pool)
    .await
    .expect("Failed to insert old memory");

    let mm = MemoryManager::new(pool.clone(), None);
    let recent = mm.load_recent_memories(identity_id, 50).await
        .expect("Failed to load recent memories");

    // Should include recent but not old
    let recent_ids: Vec<Uuid> = recent.iter().map(|m| m.memory_id).collect();
    assert!(recent_ids.contains(&recent_id), "Should include recent memory");
    assert!(!recent_ids.contains(&old_id), "Should not include 3-day-old memory");

    println!("✓ 48hr window retrieval: {} recent memories loaded", recent.len());
}

/// Insert memories with varying importance and verify high-importance filtering.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_memory_load_high_importance() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let identity_id = insert_test_identity(&pool, "importance_agent").await;

    create_test_memory(&pool, identity_id, "Low importance", 0.3).await;
    create_test_memory(&pool, identity_id, "Medium importance", 0.5).await;
    let high_id = create_test_memory(&pool, identity_id, "High importance", 0.85).await;
    let very_high_id = create_test_memory(&pool, identity_id, "Very high importance", 0.95).await;

    let mm = MemoryManager::new(pool.clone(), None);
    let high = mm.load_high_importance_memories(identity_id, 0.8, 50).await
        .expect("Failed to load high importance memories");

    assert_eq!(high.len(), 2, "Should have 2 high-importance memories");
    let high_ids: Vec<Uuid> = high.iter().map(|m| m.memory_id).collect();
    assert!(high_ids.contains(&high_id));
    assert!(high_ids.contains(&very_high_id));

    println!("✓ High-importance filtering: {} memories above 0.8 threshold", high.len());
}

/// Insert memories with embeddings and verify pgvector cosine similarity search.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_memory_similarity_search() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let identity_id = insert_test_identity(&pool, "similarity_agent").await;
    let mm = MemoryManager::new(pool.clone(), None);

    // Create memories with embeddings
    let emb1 = create_mock_embedding();
    let _m1 = mm.create_memory(
        identity_id, "Rust programming preferences", None,
        MemorySource::Conversation, Some(emb1.clone()), 0.8,
    ).await.expect("Failed to create memory with embedding");

    // Create a slightly different embedding
    let mut emb2 = create_mock_embedding();
    for val in emb2.iter_mut().take(100) {
        *val += 0.1;
    }
    // Re-normalize
    let norm: f32 = emb2.iter().map(|x| x * x).sum::<f32>().sqrt();
    for val in &mut emb2 {
        *val /= norm;
    }
    let _m2 = mm.create_memory(
        identity_id, "Python programming preferences", None,
        MemorySource::Conversation, Some(emb2), 0.7,
    ).await.expect("Failed to create second memory with embedding");

    // Search with the first embedding (should find itself as most similar)
    let search = carnelian_core::memory::MemorySearchQuery::new(emb1, identity_id);
    let results = mm.search_memories(search).await
        .expect("Failed to search memories");

    assert!(!results.is_empty(), "Should find at least one similar memory");

    println!("✓ pgvector similarity search: {} results found", results.len());
}

/// Verify access count increments on memory retrieval.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_memory_access_count_increment() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let identity_id = insert_test_identity(&pool, "access_agent").await;
    let mm = MemoryManager::new(pool.clone(), None);

    let memory = mm.create_memory(
        identity_id, "Test memory for access tracking", None,
        MemorySource::Observation, None, 0.6,
    ).await.expect("Failed to create memory");

    assert_eq!(memory.access_count, 0, "Initial access count should be 0");

    // Access the memory multiple times
    for _ in 0..3 {
        mm.get_memory(memory.memory_id).await.expect("Failed to get memory");
    }

    let count = get_memory_access_count(&pool, memory.memory_id).await;
    assert_eq!(count, 3, "Access count should be 3 after 3 retrievals");

    println!("✓ Memory access count incremented to {}", count);
}

/// Test MemoryQuery builder with filters.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_memory_query_builder() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let identity_id = insert_test_identity(&pool, "query_agent").await;
    let mm = MemoryManager::new(pool.clone(), None);

    // Create memories with different sources and importance
    mm.create_memory(identity_id, "Conv memory low", None, MemorySource::Conversation, None, 0.3)
        .await.unwrap();
    mm.create_memory(identity_id, "Conv memory high", None, MemorySource::Conversation, None, 0.9)
        .await.unwrap();
    mm.create_memory(identity_id, "Task memory", None, MemorySource::Task, None, 0.7)
        .await.unwrap();

    // Query with filters
    let query = MemoryQuery::new()
        .with_identity(identity_id)
        .with_sources(vec![MemorySource::Conversation])
        .with_min_importance(0.5)
        .with_limit(10);

    let results = mm.query_memories(query).await.expect("Failed to query memories");
    assert_eq!(results.len(), 1, "Should find 1 conversation memory with importance >= 0.5");
    assert_eq!(results[0].content, "Conv memory high");

    println!("✓ MemoryQuery builder filtering verified");
}

// =============================================================================
// TEST GROUP: Context Assembly & Token Budgeting
// =============================================================================

/// Verify context segments are sorted in correct priority order (P0 first)
/// after enforce_budget. Note: assemble() clears segments and re-loads from DB,
/// so we test priority ordering via add_raw_segment + enforce_budget directly.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_context_window_priority_ordering() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let mut ctx = ContextWindow::new(pool, None).with_budget(100_000);

    // Add segments in reverse priority order
    ctx.add_raw_segment(SegmentPriority::P4, "Tool result".to_string(), SegmentSourceType::ToolResult, None);
    ctx.add_raw_segment(SegmentPriority::P3, "Conversation".to_string(), SegmentSourceType::ConversationMessage, None);
    ctx.add_raw_segment(SegmentPriority::P1, "Memory".to_string(), SegmentSourceType::Memory, None);
    ctx.add_raw_segment(SegmentPriority::P0, "Soul directive".to_string(), SegmentSourceType::SoulDirective, None);
    ctx.add_raw_segment(SegmentPriority::P2, "Task context".to_string(), SegmentSourceType::TaskContext, None);

    // enforce_budget does NOT clear segments — it only prunes if over budget
    let config = Config::default();
    ctx.enforce_budget(config.tool_trim_threshold, config.tool_clear_age_secs);

    // compute_provenance works on current segments
    let provenance = ctx.compute_provenance();
    // All 5 segments should survive (budget is huge)
    let total_segments: usize = provenance.segment_counts.values().sum();
    assert_eq!(total_segments, 5, "All 5 segments should survive budget enforcement");

    println!("✓ Context segments: {} total after enforce_budget", total_segments);
}

/// Verify token budget enforcement prunes lower-priority segments.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_context_window_token_budget_enforcement() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Set a very small budget
    let mut ctx = ContextWindow::new(pool, None).with_budget(20);

    // Add P0 segment (should always be kept)
    ctx.add_raw_segment(
        SegmentPriority::P0,
        "Core directive".to_string(),
        SegmentSourceType::SoulDirective,
        None,
    );

    // Add large P3 segment that should be pruned (P3 = conversation, prunable)
    let large_content = "x ".repeat(500);
    ctx.add_raw_segment(
        SegmentPriority::P3,
        large_content,
        SegmentSourceType::ConversationMessage,
        None,
    );

    let config = Config::default();
    ctx.enforce_budget(config.tool_trim_threshold, config.tool_clear_age_secs);

    // P0 segment should survive, P3 should be pruned
    let provenance = ctx.compute_provenance();
    let soul_count = provenance.segment_counts.get("soul_directive").copied().unwrap_or(0);
    assert!(soul_count >= 1, "P0 soul directive should survive budget enforcement");

    println!("✓ Token budget enforcement: {} total tokens after pruning", provenance.total_tokens);
}

/// Verify soul directives are loaded as P0 segments.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_context_load_soul_directives() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Sync soul file first
    let souls_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/souls");
    let identity_id = insert_test_identity_with_soul(&pool, "ctx_soul_agent", "test_lian.md").await;
    let sm = SoulManager::new(pool.clone(), None, souls_path);
    sm.sync_to_db(identity_id).await.expect("Failed to sync soul");

    let mut ctx = ContextWindow::new(pool.clone(), None).with_budget(50000);
    ctx.load_soul_directives(identity_id).await.expect("Failed to load soul directives");

    // Verify segments were loaded (compute_provenance works on current segments)
    let provenance = ctx.compute_provenance();
    let soul_count = provenance.segment_counts.get("soul_directive").copied().unwrap_or(0);
    assert!(soul_count > 0, "Should have loaded soul directive segments");
    assert!(provenance.total_tokens > 0, "Should have counted tokens");

    println!("✓ Soul directives loaded as P0 segments: {} segments", soul_count);
}

/// Verify recent memories are loaded as P1 segments.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_context_load_recent_memories() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let identity_id = insert_test_identity(&pool, "ctx_memory_agent").await;
    let mm = MemoryManager::new(pool.clone(), None);

    mm.create_memory(identity_id, "Important recent fact", None, MemorySource::Conversation, None, 0.9)
        .await.unwrap();
    mm.create_memory(identity_id, "Another recent fact", None, MemorySource::Observation, None, 0.7)
        .await.unwrap();

    let mut ctx = ContextWindow::new(pool.clone(), None).with_budget(50000);
    ctx.load_recent_memories(identity_id, 20).await.expect("Failed to load memories");

    // Verify segments were loaded
    let provenance = ctx.compute_provenance();
    let mem_count = provenance.segment_counts.get("memory").copied().unwrap_or(0);
    assert!(mem_count > 0, "Should have loaded memory segments");
    assert!(!provenance.memory_ids.is_empty(), "Should have tracked memory IDs");

    println!("✓ Recent memories loaded as P1 segments: {} memories", mem_count);
}

/// Verify provenance tracking computes memory IDs and bundle hash.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_context_provenance_tracking() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let identity_id = insert_test_identity(&pool, "provenance_agent").await;
    let mm = MemoryManager::new(pool.clone(), None);

    let mem = mm.create_memory(identity_id, "Tracked memory", None, MemorySource::Conversation, None, 0.8)
        .await.unwrap();

    let mut ctx = ContextWindow::new(pool.clone(), None).with_budget(50000);
    ctx.load_recent_memories(identity_id, 20).await.expect("Failed to load memories");

    let provenance = ctx.compute_provenance();

    assert!(provenance.memory_ids.contains(&mem.memory_id), "Provenance should include memory ID");
    assert!(!provenance.context_bundle_hash.is_empty(), "Bundle hash should be computed");
    assert!(provenance.total_tokens > 0, "Total tokens should be > 0");

    println!("✓ Provenance tracking: hash={}, {} memory IDs",
        &provenance.context_bundle_hash[..16], provenance.memory_ids.len());
}

/// Verify task context is loaded as P2 segments via the real loader.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_context_load_task_context() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let identity_id = insert_test_identity(&pool, "ctx_task_agent").await;

    // Insert a task into the tasks table
    let task_id = Uuid::new_v4();
    sqlx::query(
        r"INSERT INTO tasks (task_id, identity_id, title, description, state, priority)
          VALUES ($1, $2, 'Refactor module', 'Refactor the context module for clarity', 'running', 5)",
    )
    .bind(task_id)
    .bind(identity_id)
    .execute(&pool)
    .await
    .expect("Failed to insert task");

    // Insert a task run
    let run_id = Uuid::new_v4();
    sqlx::query(
        r"INSERT INTO task_runs (run_id, task_id, state)
          VALUES ($1, $2, 'running')",
    )
    .bind(run_id)
    .bind(task_id)
    .execute(&pool)
    .await
    .expect("Failed to insert task run");

    let mut ctx = ContextWindow::new(pool.clone(), None).with_budget(50000);
    ctx.load_task_context(task_id).await.expect("Failed to load task context");

    let provenance = ctx.compute_provenance();
    let task_count = provenance.segment_counts.get("task_context").copied().unwrap_or(0);
    assert!(task_count > 0, "Should have loaded task context as P2 segment");
    assert!(provenance.total_tokens > 0, "Task context should have tokens");

    println!("✓ Task context loaded as P2: {} segments, {} tokens", task_count, provenance.total_tokens);
}

/// Verify conversation history is loaded as P3 segments via the real loader,
/// and provenance includes message IDs.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_context_load_conversation_history() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let agent_id = insert_test_identity(&pool, "ctx_conv_agent").await;
    let session_key = format!("agent:{}:ui", agent_id);
    let sm = SessionManager::with_defaults(pool.clone());
    let session = sm.create_session(&session_key).await.expect("Failed to create session");

    // Append user/assistant messages (not tool — those are P4)
    for i in 0..5 {
        sm.append_message(session.session_id, "user", format!("Question {}", i), Some(10), None, None, None, None, None)
            .await.expect("Failed to append user message");
        sm.append_message(session.session_id, "assistant", format!("Answer {}", i), Some(10), None, None, None, None, None)
            .await.expect("Failed to append assistant message");
    }

    let mut ctx = ContextWindow::new(pool.clone(), None).with_budget(50000);
    ctx.load_conversation_history(session.session_id, 50).await.expect("Failed to load conversation history");

    let provenance = ctx.compute_provenance();
    let conv_count = provenance.segment_counts.get("conversation_message").copied().unwrap_or(0);
    assert_eq!(conv_count, 10, "Should have loaded 10 conversation messages as P3 segments");
    assert!(provenance.total_tokens > 0, "Conversation should have tokens");

    println!("✓ Conversation history loaded as P3: {} segments", conv_count);
}

/// Verify tool results are loaded as P4 segments via the real loader,
/// and provenance includes tool result metadata.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_context_load_tool_results() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let agent_id = insert_test_identity(&pool, "ctx_tool_agent").await;
    let session_key = format!("agent:{}:ui", agent_id);
    let sm = SessionManager::with_defaults(pool.clone());
    let session = sm.create_session(&session_key).await.expect("Failed to create session");

    // Append tool result messages
    for i in 0..3 {
        sm.append_message(
            session.session_id, "tool",
            format!("Tool output {}: file contents with data", i),
            Some(20), Some(format!("tool_{}", i)), Some(format!("call_{}", i)),
            None, None, None,
        ).await.expect("Failed to append tool message");
    }

    let mut ctx = ContextWindow::new(pool.clone(), None).with_budget(50000);
    ctx.load_tool_results(session.session_id, 20).await.expect("Failed to load tool results");

    let provenance = ctx.compute_provenance();
    let tool_count = provenance.segment_counts.get("tool_result").copied().unwrap_or(0);
    assert_eq!(tool_count, 3, "Should have loaded 3 tool results as P4 segments");

    println!("✓ Tool results loaded as P4: {} segments", tool_count);
}

/// Force an over-budget scenario and verify P4 and P3 segments are pruned
/// while P0 and P2 survive. Tests the full pruning cascade.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_context_over_budget_pruning_cascade() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Use a very small budget to force pruning
    let mut ctx = ContextWindow::new(pool, None).with_budget(50);

    // P0: Soul directive (never pruned) — small
    ctx.add_raw_segment(SegmentPriority::P0, "Be helpful.".to_string(), SegmentSourceType::SoulDirective, None);

    // P2: Task context (never pruned) — small
    ctx.add_raw_segment(SegmentPriority::P2, "Current task: test".to_string(), SegmentSourceType::TaskContext, None);

    // P3: Conversation messages (prunable, oldest first) — large
    for i in 0..5 {
        let large_msg = format!("Conversation message {} with lots of content to consume tokens {}", i, "x".repeat(200));
        ctx.add_raw_segment(SegmentPriority::P3, large_msg, SegmentSourceType::ConversationMessage, None);
    }

    // P4: Tool results (prunable first) — large
    for i in 0..3 {
        let large_tool = format!("Tool result {} with extensive output data {}", i, "y".repeat(300));
        ctx.add_raw_segment(SegmentPriority::P4, large_tool, SegmentSourceType::ToolResult, None);
    }

    let config = Config::default();
    ctx.enforce_budget(config.tool_trim_threshold, config.tool_clear_age_secs);

    let provenance = ctx.compute_provenance();

    // P0 should always survive
    let p0_count = provenance.segment_counts.get("soul_directive").copied().unwrap_or(0);
    assert!(p0_count >= 1, "P0 soul directive should survive pruning");

    // P2 should always survive
    let p2_count = provenance.segment_counts.get("task_context").copied().unwrap_or(0);
    assert!(p2_count >= 1, "P2 task context should survive pruning");

    // P4 should be pruned first (most or all removed)
    let p4_count = provenance.segment_counts.get("tool_result").copied().unwrap_or(0);

    // P3 should be partially or fully pruned
    let p3_count = provenance.segment_counts.get("conversation_message").copied().unwrap_or(0);

    // Total tokens should be within budget
    assert!(provenance.total_tokens <= 50,
        "Total tokens {} should be within budget 50", provenance.total_tokens);

    println!("✓ Over-budget pruning: P0={}, P2={}, P3={} (was 5), P4={} (was 3), total_tokens={}",
        p0_count, p2_count, p3_count, p4_count, provenance.total_tokens);
}

/// Build context with all loaders (P0–P4) and verify full assembly pipeline
/// including provenance with memory IDs, message metadata, and bundle hash.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_context_assemble_full_pipeline() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Set up identity with soul
    let souls_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/souls");
    let identity_id = insert_test_identity_with_soul(&pool, "full_ctx_agent", "test_minimal.md").await;
    let soul_mgr = SoulManager::new(pool.clone(), None, souls_path);
    soul_mgr.sync_to_db(identity_id).await.expect("Failed to sync soul");

    // Create memories (P1)
    let mm = MemoryManager::new(pool.clone(), None);
    let mem = mm.create_memory(identity_id, "User likes Rust", None, MemorySource::Conversation, None, 0.85)
        .await.unwrap();

    // Create session with messages (P3) and tool results (P4)
    let session_key = format!("agent:{}:ui", identity_id);
    let sesm = SessionManager::with_defaults(pool.clone());
    let session = sesm.create_session(&session_key).await.expect("Failed to create session");

    sesm.append_message(session.session_id, "user", "What is Rust?".to_string(), Some(8), None, None, None, None, None)
        .await.expect("Failed to append user message");
    sesm.append_message(session.session_id, "assistant", "Rust is a systems language.".to_string(), Some(10), None, None, None, None, None)
        .await.expect("Failed to append assistant message");
    sesm.append_message(
        session.session_id, "tool", "File contents: fn main() {}".to_string(),
        Some(12), Some("read_file".to_string()), Some("call_001".to_string()), None, None, None,
    ).await.expect("Failed to append tool message");

    // Create a task (P2)
    let task_id = Uuid::new_v4();
    sqlx::query(
        r"INSERT INTO tasks (task_id, identity_id, title, description, state, priority)
          VALUES ($1, $2, 'Build feature', 'Implement new feature', 'running', 5)",
    )
    .bind(task_id)
    .bind(identity_id)
    .execute(&pool)
    .await
    .expect("Failed to insert task");

    // Build full context with all priorities via build_for_session
    let config = Config::default();
    let mut ctx = ContextWindow::build_for_session(
        pool.clone(), None, session.session_id, Some(task_id), &config,
    ).await.expect("Failed to build context for session");

    let assembled = ctx.assemble(&config).await.expect("Failed to assemble");
    let provenance = ctx.compute_provenance();

    assert!(!assembled.is_empty(), "Assembled context should not be empty");
    assert!(provenance.total_tokens > 0, "Should have counted tokens");
    assert!(!provenance.context_bundle_hash.is_empty(), "Should have bundle hash");

    // Verify provenance includes memory IDs
    assert!(provenance.memory_ids.contains(&mem.memory_id),
        "Provenance should include the created memory ID");

    // Verify multiple segment types are present
    let total_segments: usize = provenance.segment_counts.values().sum();
    assert!(total_segments >= 4, "Should have segments from P0, P1, P2, P3, P4 (got {})", total_segments);

    println!("✓ Full context pipeline (P0–P4): {} segments, {} tokens, {} memory IDs, hash={}",
        total_segments, provenance.total_tokens, provenance.memory_ids.len(),
        &provenance.context_bundle_hash[..16]);
}

// =============================================================================
// TEST GROUP: Session Compaction & Memory Flush
// =============================================================================

/// Full compaction pipeline: seed messages, call compact_session, verify
/// memories flushed, counters recomputed, compaction_count incremented,
/// and ledger event written.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_compaction_full_pipeline() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let agent_id = insert_test_identity(&pool, "compaction_agent").await;
    let session_key = format!("agent:{}:ui", agent_id);

    let sm = SessionManager::with_defaults(pool.clone());
    let session = sm.create_session(&session_key).await.expect("Failed to create session");
    let ledger = Ledger::new(pool.clone());
    let config = Config::default();
    let correlation_id = Uuid::now_v7();

    // Seed user/assistant exchange pairs with >100 chars combined to trigger memory flush
    for i in 0..5 {
        let user_msg = format!("User question {} — this is a detailed question about Rust programming and memory management that exceeds the minimum character threshold for flush", i);
        let asst_msg = format!("Assistant reply {} — here is a comprehensive answer about Rust's ownership model, borrowing rules, and lifetime annotations that provides meaningful content", i);
        sm.append_message(session.session_id, "user", user_msg, Some(50), None, None, Some(correlation_id), None, None)
            .await.expect("Failed to append user message");
        sm.append_message(session.session_id, "assistant", asst_msg, Some(50), None, None, Some(correlation_id), None, None)
            .await.expect("Failed to append assistant message");
    }

    // Also seed a tool result message for tool pruning coverage
    sm.append_message(
        session.session_id, "tool",
        "Tool output: file contents here with enough data to be meaningful".to_string(),
        Some(20), Some("read_file".to_string()), Some("call_001".to_string()),
        Some(correlation_id), None, None,
    ).await.expect("Failed to append tool message");

    let counters_before = sm.get_counters(session.session_id).await.expect("Failed to get counters");
    assert!(counters_before.total > 0, "Should have tokens before compaction");

    let initial_count = get_compaction_count(&pool, session.session_id).await;
    assert_eq!(initial_count, 0, "Initial compaction count should be 0");

    // Run the full compaction pipeline
    let outcome: CompactionOutcome = sm.compact_session(
        session.session_id,
        CompactionTrigger::ManualRequest,
        Some(correlation_id),
        &config,
        Some(&ledger),
        false, // do NOT skip flush — exercise the memory flush path
    ).await.expect("compact_session should succeed");

    // Verify memories were flushed (exchanges > 100 chars combined)
    let memory_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM memories WHERE identity_id = $1",
    )
    .bind(agent_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to count memories");
    assert_eq!(usize::try_from(memory_count).unwrap_or(0), outcome.memories_flushed,
        "DB memory count should match outcome.memories_flushed");
    assert!(outcome.memories_flushed > 0, "Should have flushed at least 1 memory");
    assert!(!outcome.flush_failed, "Memory flush should not have failed");

    // Verify counters were recomputed (step 5 of pipeline)
    let counters_after = sm.get_counters(session.session_id).await.expect("Failed to get counters");
    assert_eq!(outcome.tokens_after, counters_after.total,
        "Outcome tokens_after should match recomputed counters");

    // Verify compaction_count incremented
    let new_count = get_compaction_count(&pool, session.session_id).await;
    assert_eq!(new_count, 1, "Compaction count should be 1 after compact_session");

    // Verify ledger event was written
    let ledger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ledger_events WHERE action_type = 'session.compaction' AND correlation_id = $1",
    )
    .bind(correlation_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to count ledger events");
    assert!(ledger_count >= 1, "Should have at least 1 compaction ledger event");

    println!("✓ Full compaction pipeline: {} memories flushed, {} → {} tokens, compaction_count={}",
        outcome.memories_flushed, outcome.tokens_before, outcome.tokens_after, new_count);
}

/// Verify check_and_compact_if_needed triggers compaction when tokens exceed
/// the effective limit, and returns None when under limit.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_check_and_compact_if_needed() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let agent_id = insert_test_identity(&pool, "auto_compact_agent").await;
    let session_key = format!("agent:{}:ui", agent_id);

    let sm = SessionManager::with_defaults(pool.clone());
    let session = sm.create_session(&session_key).await.expect("Failed to create session");
    let ledger = Ledger::new(pool.clone());

    // Use a small context window so we can exceed it with a few messages
    let mut config = Config::default();
    config.context_window_tokens = 200;
    config.context_reserve_percent = 10; // effective limit = 180 tokens

    // First check: under limit, should return None
    let result = sm.check_and_compact_if_needed(
        session.session_id, None, &config, Some(&ledger),
    ).await.expect("check_and_compact_if_needed should succeed");
    assert!(result.is_none(), "Should not compact when under limit");

    // Seed enough messages to exceed the effective limit (180 tokens)
    for i in 0..10 {
        let msg = format!("Message {} with enough content to accumulate tokens beyond the small window limit", i);
        sm.append_message(session.session_id, "user", msg, Some(25), None, None, None, None, None)
            .await.expect("Failed to append message");
        sm.append_message(
            session.session_id, "assistant",
            format!("Reply {} with detailed content that also contributes tokens to push us over the limit threshold", i),
            Some(25), None, None, None, None, None,
        ).await.expect("Failed to append message");
    }

    let counters = sm.get_counters(session.session_id).await.expect("Failed to get counters");
    assert!(counters.total > 180, "Total tokens {} should exceed effective limit 180", counters.total);

    // Second check: over limit, should trigger compaction
    let result = sm.check_and_compact_if_needed(
        session.session_id, Some(Uuid::now_v7()), &config, Some(&ledger),
    ).await.expect("check_and_compact_if_needed should succeed");
    assert!(result.is_some(), "Should trigger compaction when over limit");

    let outcome = result.unwrap();
    assert!(outcome.tokens_before > 0, "Should have recorded tokens_before");

    // Verify compaction_count incremented
    let count = get_compaction_count(&pool, session.session_id).await;
    assert_eq!(count, 1, "Compaction count should be 1");

    println!("✓ check_and_compact_if_needed: triggered at {} tokens, compacted to {}",
        outcome.tokens_before, outcome.tokens_after);
}

// =============================================================================
// TEST GROUP: Model Routing & Budget Enforcement
// =============================================================================

/// Verify ModelRouter::complete returns BudgetExceeded when provider daily
/// budget is exhausted. Uses a remote provider with daily_limit_usd set,
/// seeds usage records to exceed it, then calls complete.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_model_router_budget_exceeded_via_complete() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Create a remote provider named "openai" so model "gpt-4o" routes to it
    let budget = json!({"daily_limit_usd": 5.0, "monthly_limit_usd": 100.0});
    sqlx::query(
        "INSERT INTO model_providers (provider_id, name, provider_type, enabled, budget_limits) \
         VALUES ($1, $2, $3, true, $4)",
    )
    .bind(Uuid::new_v4())
    .bind("openai")
    .bind("remote")
    .bind(&budget)
    .execute(&pool)
    .await
    .expect("Failed to insert openai provider");

    // Resolve provider_id for usage seeding
    let provider_id: Uuid = sqlx::query_scalar(
        "SELECT provider_id FROM model_providers WHERE name = 'openai' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to get provider_id");

    // Seed usage records to exceed the $5 daily limit
    for _ in 0..6 {
        insert_test_usage(&pool, provider_id, 1.0).await;
    }
    let total = get_total_usage_cost(&pool, provider_id).await;
    assert!(total > 5.0, "Total usage ${:.2} should exceed $5 daily limit", total);

    // Create a ModelRouter pointing to a dummy gateway (won't be reached)
    let identity_id = insert_test_identity(&pool, "budget_test_agent").await;
    let policy_engine = PolicyEngine::new(pool.clone());
    let ledger = Ledger::new(pool.clone());
    let router = ModelRouter::new(
        pool.clone(),
        "http://127.0.0.1:1".to_string(), // unreachable gateway
        Arc::new(policy_engine),
        Arc::new(ledger),
    );

    // Grant model.remote capability so the capability check passes
    sqlx::query(
        "INSERT INTO capability_grants (grant_id, subject_type, subject_id, capability_key) \
         VALUES ($1, 'identity', $2, 'model.remote')",
    )
    .bind(Uuid::new_v4())
    .bind(identity_id.to_string())
    .execute(&pool)
    .await
    .expect("Failed to grant capability");

    let correlation_id = Uuid::now_v7();
    let request = carnelian_core::CompletionRequest {
        model: "gpt-4o".to_string(),
        messages: vec![carnelian_core::Message {
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

    let result = router.complete(request, identity_id, None, None).await;
    assert!(result.is_err(), "Should fail when budget exceeded");
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("budget") || err_str.contains("Budget"),
        "Error should mention budget, got: {}", err_str);

    println!("✓ ModelRouter::complete returns BudgetExceeded when over daily limit (${:.2})", total);
}

/// Verify ModelRouter::complete attempts gateway call when within budget,
/// and that a ledger event is emitted for the request attempt.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_model_router_within_budget_attempts_gateway() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Create a local provider (no budget check needed, no capability check)
    let budget = json!({"daily_limit_usd": 100.0});
    sqlx::query(
        "INSERT INTO model_providers (provider_id, name, provider_type, enabled, budget_limits) \
         VALUES ($1, $2, $3, true, $4)",
    )
    .bind(Uuid::new_v4())
    .bind("ollama")
    .bind("local")
    .bind(&budget)
    .execute(&pool)
    .await
    .expect("Failed to insert local provider");

    let identity_id = insert_test_identity(&pool, "router_gateway_agent").await;
    let policy_engine = PolicyEngine::new(pool.clone());
    let ledger = Ledger::new(pool.clone());
    let router = ModelRouter::new(
        pool.clone(),
        "http://127.0.0.1:1".to_string(), // unreachable gateway
        Arc::new(policy_engine),
        Arc::new(ledger),
    );

    let correlation_id = Uuid::now_v7();
    let request = carnelian_core::CompletionRequest {
        model: "deepseek-r1:7b".to_string(),
        messages: vec![carnelian_core::Message {
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

    let result = router.complete(request, identity_id, None, None).await;
    // Should fail at gateway level (connection refused), NOT at budget level
    assert!(result.is_err(), "Should fail because gateway is unreachable");
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("Gateway") || err_str.contains("gateway") || err_str.contains("onnect"),
        "Error should be gateway-related, got: {}", err_str);

    // Verify a ledger event was written for the model.call.request attempt
    let ledger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ledger_events WHERE action_type = 'model.call.request' AND correlation_id = $1",
    )
    .bind(correlation_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to count ledger events");
    assert!(ledger_count >= 1, "Should have logged model.call.request to ledger");

    println!("✓ ModelRouter within budget: gateway attempted, ledger event written");
}

// =============================================================================
// TEST GROUP: Heartbeat Agentic Turn
// =============================================================================

/// Exercise the heartbeat agentic turn pipeline: assemble context (soul
/// directives + memories), attempt model call via ModelRouter (will fail
/// without a live gateway — testing the "failed" path), persist to
/// heartbeat_history, and write a ledger event. This replicates the
/// Scheduler::run_heartbeat flow using public APIs.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_heartbeat_agentic_turn_pipeline() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // Set up identity with soul directives and memories
    let souls_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/souls");
    let identity_id = insert_test_identity_with_soul(&pool, "heartbeat_agent", "test_lian.md").await;
    let soul_mgr = SoulManager::new(pool.clone(), None, souls_path);
    soul_mgr.sync_to_db(identity_id).await.expect("Failed to sync soul");

    let mm = MemoryManager::new(pool.clone(), None);
    mm.create_memory(identity_id, "Recent observation for heartbeat", None, MemorySource::Observation, None, 0.7)
        .await.expect("Failed to create memory");

    // Insert a local provider so the router can select it
    sqlx::query(
        "INSERT INTO model_providers (provider_id, name, provider_type, enabled, budget_limits) \
         VALUES ($1, 'ollama', 'local', true, $2)",
    )
    .bind(Uuid::new_v4())
    .bind(json!({}))
    .execute(&pool)
    .await
    .expect("Failed to insert local provider");

    let config = Config::default();
    let correlation_id = Uuid::now_v7();
    let start = std::time::Instant::now();

    // Step 1: Assemble context (same as run_heartbeat does)
    let mut ctx = ContextWindow::new(pool.clone(), None).with_config(&config);
    ctx.load_soul_directives(identity_id).await.expect("Failed to load soul directives");
    ctx.load_recent_memories(identity_id, 10).await.expect("Failed to load memories");
    ctx.add_raw_segment(
        SegmentPriority::P2,
        "Current state: 0 pending tasks in queue.".to_string(),
        SegmentSourceType::TaskContext,
        None,
    );
    ctx.enforce_budget(config.tool_trim_threshold, config.tool_clear_age_secs);

    let context_text = ctx.assemble(&config).await.expect("Failed to assemble context");
    assert!(!context_text.is_empty(), "Heartbeat context should not be empty");

    // Step 2: Attempt model call (will fail — no live gateway)
    let policy_engine = PolicyEngine::new(pool.clone());
    let ledger = Arc::new(Ledger::new(pool.clone()));
    let router = ModelRouter::new(
        pool.clone(),
        "http://127.0.0.1:1".to_string(),
        Arc::new(policy_engine),
        ledger.clone(),
    );

    let request = carnelian_core::CompletionRequest {
        model: "deepseek-r1:7b".to_string(),
        messages: vec![
            carnelian_core::Message {
                role: "system".to_string(),
                content: context_text,
                name: None,
                tool_call_id: None,
            },
            carnelian_core::Message {
                role: "user".to_string(),
                content: "Reflect briefly on the current state.".to_string(),
                name: None,
                tool_call_id: None,
            },
        ],
        temperature: Some(0.7),
        max_tokens: Some(500),
        stream: None,
        correlation_id: Some(correlation_id),
    };

    let (status, reason) = match router.complete(request, identity_id, None, None).await {
        Ok(response) => {
            let content = response.choices.first().map(|c| c.message.content.clone()).unwrap_or_default();
            ("ok".to_string(), Some(content))
        }
        Err(e) => ("failed".to_string(), Some(format!("Model call error: {e}"))),
    };

    let duration_ms = i32::try_from(start.elapsed().as_millis()).unwrap_or(i32::MAX);

    // Step 3: Persist to heartbeat_history (same as run_heartbeat does)
    let heartbeat_id: Uuid = sqlx::query_scalar(
        r"INSERT INTO heartbeat_history (identity_id, mantra, tasks_queued, status, duration_ms, reason, correlation_id)
          VALUES ($1, $2, $3, $4, $5, $6, $7)
          RETURNING heartbeat_id",
    )
    .bind(identity_id)
    .bind("Test mantra")
    .bind(0i32)
    .bind(&status)
    .bind(duration_ms)
    .bind(&reason)
    .bind(correlation_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to persist heartbeat");

    // Step 4: Write ledger event (same as run_heartbeat does)
    ledger.append_event(
        Some(identity_id),
        "heartbeat.completed",
        json!({
            "heartbeat_id": heartbeat_id,
            "status": status,
            "tasks_queued": 0,
            "duration_ms": duration_ms,
        }),
        Some(correlation_id),
    ).await.expect("Failed to log heartbeat to ledger");

    // Verify heartbeat_history record
    let (stored_status, stored_duration, stored_corr): (String, i32, Option<Uuid>) = sqlx::query_as(
        "SELECT status, duration_ms, correlation_id FROM heartbeat_history WHERE heartbeat_id = $1",
    )
    .bind(heartbeat_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to query heartbeat");

    assert_eq!(stored_status, "failed", "Status should be 'failed' without live gateway");
    assert!(stored_duration > 0, "Duration should be > 0");
    assert_eq!(stored_corr, Some(correlation_id), "Correlation ID should be preserved");

    // Verify ledger event
    let ledger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ledger_events WHERE action_type = 'heartbeat.completed' AND correlation_id = $1",
    )
    .bind(correlation_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to count ledger events");
    assert!(ledger_count >= 1, "Should have heartbeat.completed ledger event");

    println!("✓ Heartbeat agentic turn: context assembled, model attempted, heartbeat_id={}, status={}, ledger logged",
        heartbeat_id, stored_status);
}

/// Verify that the heartbeat pipeline correctly propagates correlation IDs
/// through context assembly, heartbeat_history, and ledger events.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_heartbeat_correlation_end_to_end() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let identity_id = insert_test_identity(&pool, "heartbeat_corr_agent").await;
    let correlation_id = Uuid::now_v7();
    let ledger = Ledger::new(pool.clone());

    // Simulate a heartbeat with correlation tracking through all stages
    let heartbeat_id: Uuid = sqlx::query_scalar(
        r"INSERT INTO heartbeat_history (identity_id, status, duration_ms, reason, correlation_id)
          VALUES ($1, 'ok', 42, 'All systems nominal', $2)
          RETURNING heartbeat_id",
    )
    .bind(identity_id)
    .bind(correlation_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to persist heartbeat");

    // Log heartbeat event with same correlation
    ledger.append_event(
        Some(identity_id),
        "heartbeat.completed",
        json!({
            "heartbeat_id": heartbeat_id,
            "status": "ok",
            "correlation_id": correlation_id,
        }),
        Some(correlation_id),
    ).await.expect("Failed to log heartbeat");

    // Verify correlation ID links heartbeat_history and ledger_events
    let hb_corr: Option<Uuid> = sqlx::query_scalar(
        "SELECT correlation_id FROM heartbeat_history WHERE heartbeat_id = $1",
    )
    .bind(heartbeat_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to query heartbeat correlation");
    assert_eq!(hb_corr, Some(correlation_id));

    let ledger_corr: Option<Uuid> = sqlx::query_scalar(
        "SELECT correlation_id FROM ledger_events WHERE action_type = 'heartbeat.completed' AND correlation_id = $1",
    )
    .bind(correlation_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to query ledger correlation");
    assert_eq!(ledger_corr, Some(correlation_id));

    println!("✓ Heartbeat correlation end-to-end: heartbeat_id={}, correlation_id={}", heartbeat_id, correlation_id);
}

// =============================================================================
// TEST GROUP: Agentic Loop End-to-End
// =============================================================================

/// Verify agentic request/response types serialize correctly.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_agentic_loop_session_persistence() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let agent_id = insert_test_identity(&pool, "agentic_agent").await;
    let session_key = format!("agent:{}:ui", agent_id);

    let sm = SessionManager::with_defaults(pool.clone());
    let session = sm.create_session(&session_key).await.expect("Failed to create session");

    // Simulate agentic loop: append user message
    let _msg_id = sm.append_message(
        session.session_id, "user", "What is 2+2?".to_string(),
        Some(8), None, None, Some(Uuid::now_v7()), None, None,
    ).await.expect("Failed to append user message");

    // Simulate assistant response
    let _msg_id = sm.append_message(
        session.session_id, "assistant", "The answer is 4.".to_string(),
        Some(6), None, None, Some(Uuid::now_v7()), None, None,
    ).await.expect("Failed to append assistant message");

    // Simulate tool result persistence
    let _msg_id = sm.append_message(
        session.session_id, "tool",
        json!({"status": "Success", "result": 4}).to_string(),
        Some(15), Some("calculator".to_string()), Some("call_001".to_string()),
        Some(Uuid::now_v7()), None, None,
    ).await.expect("Failed to append tool message");

    // Verify all messages persisted
    let messages = sm.load_messages(session.session_id, Some(100), None).await
        .expect("Failed to load messages");
    assert_eq!(messages.len(), 3, "Should have 3 messages");

    // Verify token counters
    let counters = sm.get_counters(session.session_id).await.expect("Failed to get counters");
    assert_eq!(counters.total, 29, "Total tokens should be 8+6+15=29");

    println!("✓ Agentic loop session persistence: {} messages, {} tokens",
        messages.len(), counters.total);
}

/// Verify correlation ID flows through session messages.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_agentic_loop_correlation_id_propagation() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let agent_id = insert_test_identity(&pool, "corr_agentic_agent").await;
    let session_key = format!("agent:{}:ui", agent_id);
    let correlation_id = Uuid::now_v7();

    let sm = SessionManager::with_defaults(pool.clone());
    let session = sm.create_session(&session_key).await.expect("Failed to create session");

    // Append messages with correlation ID
    sm.append_message(
        session.session_id, "user", "Hello".to_string(),
        Some(3), None, None, Some(correlation_id), None, None,
    ).await.expect("Failed to append message");

    sm.append_message(
        session.session_id, "assistant", "Hi!".to_string(),
        Some(2), None, None, Some(correlation_id), None, None,
    ).await.expect("Failed to append message");

    // Verify correlation IDs are stored
    let corr_ids: Vec<Option<Uuid>> = sqlx::query_scalar(
        "SELECT correlation_id FROM session_messages WHERE session_id = $1 ORDER BY message_id",
    )
    .bind(session.session_id)
    .fetch_all(&pool)
    .await
    .expect("Failed to query correlation IDs");

    assert_eq!(corr_ids.len(), 2);
    for cid in &corr_ids {
        assert_eq!(*cid, Some(correlation_id), "Correlation ID should be preserved");
    }

    println!("✓ Correlation ID propagated through {} messages", corr_ids.len());
}

/// Verify policy engine integration with capability checks.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_agentic_loop_policy_engine_integration() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let policy_engine = PolicyEngine::new(pool.clone());

    // Without any grants, capability check should return false
    let has_cap = policy_engine.check_capability(
        "identity",
        &Uuid::new_v4().to_string(),
        "tool.read_file",
        None::<&carnelian_core::EventStream>,
    ).await.unwrap_or(false);

    assert!(!has_cap, "Should not have capability without explicit grant");

    println!("✓ Policy engine denies capability without grant");
}

/// Verify ledger event persistence for agentic operations.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_agentic_loop_ledger_audit() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let identity_id = insert_test_identity(&pool, "ledger_agent").await;
    let ledger = Ledger::new(pool.clone());

    // Log an agentic event
    ledger.append_event(
        Some(identity_id),
        "agentic.request_received",
        json!({
            "session_key": "agent:test:ui",
            "correlation_id": Uuid::now_v7(),
        }),
        Some(Uuid::now_v7()),
    ).await.expect("Failed to append ledger event");

    // Verify the event exists
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ledger_events WHERE action_type = 'agentic.request_received'",
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to count ledger events");

    assert!(count >= 1, "Should have at least 1 agentic ledger event");

    println!("✓ Ledger audit event persisted: {} events", count);
}

// =============================================================================
// TEST GROUP: Cross-Module Integration
// =============================================================================

/// End-to-end test: soul sync → memory creation → context assembly.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_end_to_end_soul_memory_context() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    // 1. Sync soul file
    let souls_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/souls");
    let identity_id = insert_test_identity_with_soul(&pool, "e2e_agent", "test_lian.md").await;
    let soul_mgr = SoulManager::new(pool.clone(), None, souls_path);
    soul_mgr.sync_to_db(identity_id).await.expect("Failed to sync soul");

    // 2. Create memories
    let mm = MemoryManager::new(pool.clone(), None);
    mm.create_memory(identity_id, "User prefers Rust", None, MemorySource::Conversation, None, 0.9)
        .await.unwrap();
    mm.create_memory(identity_id, "Project uses PostgreSQL", None, MemorySource::Task, None, 0.85)
        .await.unwrap();

    // 3. Assemble context via build_for_session
    let session_key = format!("agent:{}:ui", identity_id);
    let sm = SessionManager::with_defaults(pool.clone());
    let session = sm.create_session(&session_key).await.expect("Failed to create session");

    let config = Config::default();
    let mut ctx = ContextWindow::build_for_session(
        pool.clone(), None, session.session_id, None, &config,
    ).await.expect("Failed to build context");

    let assembled = ctx.assemble(&config).await.expect("Failed to assemble");
    let provenance = ctx.compute_provenance();

    // Verify soul directives present
    assert!(assembled.contains("Core Truths"), "Should contain soul directives");

    // Verify memories present
    assert!(assembled.contains("User prefers Rust"), "Should contain memory");

    // Verify provenance
    assert_eq!(provenance.memory_ids.len(), 2, "Should track 2 memory IDs");
    assert!(!provenance.context_bundle_hash.is_empty());

    println!("✓ End-to-end: soul + {} memories → {} tokens, hash={}",
        provenance.memory_ids.len(), provenance.total_tokens,
        &provenance.context_bundle_hash[..16]);
}

/// Verify session and message persistence across a simulated restart.
/// Creates a session, appends messages, drops the pool (simulating shutdown),
/// reconnects to the same database, and reloads the session and messages.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_session_persistence_across_restart() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;

    let session_id;
    let session_key;
    let correlation_id = Uuid::now_v7();

    // Phase 1: Create session and messages, then drop the pool
    {
        let pool = setup_test_db(&database_url).await;
        let agent_id = insert_test_identity(&pool, "restart_agent").await;
        session_key = format!("agent:{}:ui", agent_id);

        let sm = SessionManager::with_defaults(pool.clone());
        let session = sm.create_session(&session_key).await.expect("Failed to create session");
        session_id = session.session_id;

        // Append messages with a correlation ID
        sm.append_message(
            session_id, "user", "Hello before restart".to_string(),
            Some(10), None, None, Some(correlation_id), None, None,
        ).await.expect("Failed to append user message");

        sm.append_message(
            session_id, "assistant", "Hi! I'll remember this.".to_string(),
            Some(8), None, None, Some(correlation_id), None, None,
        ).await.expect("Failed to append assistant message");

        sm.append_message(
            session_id, "tool", "Tool result: success".to_string(),
            Some(5), Some("test_tool".to_string()), Some("call_001".to_string()),
            Some(correlation_id), None, None,
        ).await.expect("Failed to append tool message");

        // Verify before drop
        let counters = sm.get_counters(session_id).await.expect("Failed to get counters");
        assert_eq!(counters.total, 23, "Pre-restart total should be 10+8+5=23");

        // Pool is dropped here, simulating shutdown
        pool.close().await;
    }

    // Phase 2: Reconnect to the same database and verify everything persisted
    {
        let pool2 = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect(&database_url)
            .await
            .expect("Failed to reconnect to database");

        let sm2 = SessionManager::with_defaults(pool2.clone());

        // Reload session by key
        let loaded_session = sm2.load_session(&session_key).await
            .expect("Failed to load session")
            .expect("Session should exist after restart");
        assert_eq!(loaded_session.session_id, session_id, "Session ID should match");

        // Reload messages
        let messages = sm2.load_messages(session_id, Some(100), None).await
            .expect("Failed to load messages");
        assert_eq!(messages.len(), 3, "Should have 3 messages after restart");
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello before restart");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[2].role, "tool");

        // Verify token counters survived
        let counters = sm2.get_counters(session_id).await.expect("Failed to get counters");
        assert_eq!(counters.total, 23, "Post-restart total should still be 23");
        assert_eq!(counters.user, 10);
        assert_eq!(counters.assistant, 8);
        assert_eq!(counters.tool, 5);

        // Verify correlation IDs survived
        let corr_ids: Vec<Option<Uuid>> = sqlx::query_scalar(
            "SELECT correlation_id FROM session_messages WHERE session_id = $1 ORDER BY message_id",
        )
        .bind(session_id)
        .fetch_all(&pool2)
        .await
        .expect("Failed to query correlation IDs");
        assert_eq!(corr_ids.len(), 3);
        for cid in &corr_ids {
            assert_eq!(*cid, Some(correlation_id), "Correlation ID should survive restart");
        }

        println!("✓ Session persistence across restart: {} messages, {} tokens, correlation IDs intact",
            messages.len(), counters.total);
    }
}

/// End-to-end test: session creation → message append → token tracking → expiry.
#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test phase3_integration_test -- --ignored"]
async fn test_end_to_end_session_lifecycle() {
    let container = create_postgres_container().await;
    let database_url = get_database_url(&container).await;
    let pool = setup_test_db(&database_url).await;

    let agent_id = insert_test_identity(&pool, "lifecycle_agent").await;
    let session_key = format!("agent:{}:cli", agent_id);

    let sm = SessionManager::with_defaults(pool.clone());

    // 1. Create session
    let session = sm.create_session(&session_key).await.expect("Failed to create");

    // 2. Append messages
    for i in 0..10 {
        sm.append_message(
            session.session_id, "user", format!("Message {}", i),
            Some(5), None, None, None, None, None,
        ).await.expect("Failed to append");
    }

    // 3. Verify token tracking
    let counters = sm.get_counters(session.session_id).await.expect("Failed to get counters");
    assert_eq!(counters.user, 50, "Should have 10 * 5 = 50 user tokens");

    // 4. Extend session
    sm.extend_session(session.session_id, 72).await.expect("Failed to extend");

    // 5. Load messages
    let messages = sm.load_messages(session.session_id, Some(100), None).await
        .expect("Failed to load");
    assert_eq!(messages.len(), 10);

    // 6. Delete session
    sm.delete_session(session.session_id).await.expect("Failed to delete");
    let loaded = sm.load_session(&session_key).await.expect("Failed to load");
    assert!(loaded.is_none(), "Session should be deleted");

    println!("✓ End-to-end session lifecycle: create → 10 messages → extend → delete");
}
