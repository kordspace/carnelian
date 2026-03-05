//! Integration tests for session compaction and management
//!
//! These tests verify the full session compaction workflow including:
//! - Session compaction pipeline
//! - Memory flush during compaction
//! - Tool result pruning (soft-trim and hard-clear)
//! - Ledger event recording

use carnelian_common::Result;
use carnelian_core::config::Config;
use carnelian_core::ledger::Ledger;
use carnelian_core::session::{CompactionTrigger, Session, SessionManager};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Helper to get test database URL
fn get_test_db_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian_test".into())
}

/// Helper to create a test identity
async fn create_test_identity(pool: &PgPool, name: &str) -> Result<Uuid> {
    let identity_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO identities (identity_id, name, identity_type) VALUES ($1, $2, 'core')",
    )
    .bind(identity_id)
    .bind(name)
    .execute(pool)
    .await?;
    Ok(identity_id)
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_compact_session_full_flow() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let mut config = Config::default();
    let ledger = Ledger::new(pool.clone());
    
    // Set low context window to trigger compaction
    config.context_window_tokens = 200;
    config.context_reserve_percent = 10;

    // Create test identity
    let identity_id = create_test_identity(&pool, "test_compact_user").await?;

    // Create session manager
    let manager = SessionManager::new(pool.clone(), None, None, 24);
    let session = manager
        .create_session("agent:test:ui:group:compact_test")
        .await?;

    // Add many messages to exceed token limit
    for i in 0..20 {
        manager
            .append_message(
                session.session_id,
                if i % 2 == 0 { "user" } else { "assistant" },
                format!("Message {} with content to increase token count significantly", i),
                Some(15),
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
    }

    // Get token count before compaction
    let session_before: Session = sqlx::query_as(
        "SELECT * FROM sessions WHERE session_id = $1"
    )
    .bind(session.session_id)
    .fetch_one(&pool)
    .await?;
    
    let tokens_before: i64 = session_before
        .token_counters
        .get("total")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    // Run compaction
    let outcome = manager
        .compact_session(
            session.session_id,
            CompactionTrigger::ManualRequest,
            None,
            &config,
            Some(&ledger),
            false,
        )
        .await?;

    // Verify compaction occurred
    assert!(outcome.tokens_before > 0, "Should have tokens before compaction");
    assert!(
        outcome.tokens_after < outcome.tokens_before,
        "Token count should decrease after compaction"
    );

    // Verify compaction_count incremented
    let session_after: Session = sqlx::query_as(
        "SELECT * FROM sessions WHERE session_id = $1"
    )
    .bind(session.session_id)
    .fetch_one(&pool)
    .await?;
    
    assert_eq!(session_after.compaction_count, 1, "Compaction count should be 1");

    // Cleanup
    sqlx::query("DELETE FROM sessions WHERE session_id = $1")
        .bind(session.session_id)
        .execute(&pool)
        .await?;
    
    sqlx::query("DELETE FROM identities WHERE identity_id = $1")
        .bind(identity_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_memory_flush_zero_returns_nothing_to_store() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let manager = SessionManager::new(pool.clone(), None, None, 24);

    // Create test identity
    let identity_id = create_test_identity(&pool, "test_flush_zero_user").await?;

    // Create session with no meaningful content
    let session = manager
        .create_session("agent:test:ui:group:flush_zero_test")
        .await?;

    manager
        .append_message(
            session.session_id,
            "user",
            "hi".to_string(),
            Some(1),
            None,
            None,
            None,
            None,
            None,
        )
        .await?;

    // Trigger memory flush
    let count = manager
        .trigger_memory_flush(session.session_id, None, None)
        .await?;

    // Verify no memories were created (content too short/trivial)
    assert_eq!(count, 0, "Should not create memories for trivial content");

    // Cleanup
    sqlx::query("DELETE FROM sessions WHERE session_id = $1")
        .bind(session.session_id)
        .execute(&pool)
        .await?;
    
    sqlx::query("DELETE FROM identities WHERE identity_id = $1")
        .bind(identity_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_tool_result_soft_trim_updates_db() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let mut config = Config::default();
    config.tool_trim_threshold = 50; // Very low threshold to trigger trimming

    let manager = SessionManager::new(pool.clone(), None, None, 24);
    let session = manager
        .create_session("agent:test:ui:group:trim_test")
        .await?;

    // Add a large tool result message
    let large_content = "x".repeat(200); // 200 chars
    manager
        .append_message(
            session.session_id,
            "tool",
            large_content.clone(),
            Some(150), // Exceeds threshold
            None,
            None,
            None,
            None,
            None,
        )
        .await?;

    // Run tool result pruning
    let (trimmed_count, _cleared_count) = manager
        .prune_tool_results(session.session_id, &config)
        .await?;

    // Verify at least one message was trimmed
    assert!(trimmed_count > 0, "Should have trimmed at least one tool result");

    // Verify the message was actually trimmed in the database
    let messages = manager.load_messages(session.session_id, None, None).await?;
    let tool_msg = messages.iter().find(|m| m.role == "tool");
    
    if let Some(msg) = tool_msg {
        assert!(
            msg.content.len() < large_content.len(),
            "Tool result content should be trimmed"
        );
        assert!(
            msg.content.contains("..."),
            "Trimmed content should contain ellipsis"
        );
    }

    // Cleanup
    sqlx::query("DELETE FROM sessions WHERE session_id = $1")
        .bind(session.session_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_tool_result_hard_clear_deletes_old() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let mut config = Config::default();
    config.tool_clear_age_secs = 1; // Very short age threshold

    let manager = SessionManager::new(pool.clone(), None, None, 24);
    let session = manager
        .create_session("agent:test:ui:group:clear_test")
        .await?;

    // Add a tool result message
    let msg_id = manager
        .append_message(
            session.session_id,
            "tool",
            "Old tool result".to_string(),
            Some(10),
            None,
            None,
            None,
            None,
            None,
        )
        .await?;

    // Manually set the message timestamp to be old
    sqlx::query(
        "UPDATE session_messages SET ts = $1 WHERE message_id = $2",
    )
    .bind(Utc::now() - Duration::seconds(10))
    .bind(msg_id)
    .execute(&pool)
    .await?;

    // Run tool result pruning
    let (_trimmed_count, cleared_count) = manager
        .prune_tool_results(session.session_id, &config)
        .await?;

    // Verify at least one message was cleared
    assert!(cleared_count > 0, "Should have cleared at least one old tool result");

    // Verify the message was deleted
    let messages = manager.load_messages(session.session_id, None, None).await?;
    let tool_msg = messages.iter().find(|m| m.message_id == msg_id);
    assert!(tool_msg.is_none(), "Old tool result should be deleted");

    // Cleanup
    sqlx::query("DELETE FROM sessions WHERE session_id = $1")
        .bind(session.session_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_compaction_increments_count_and_recalculates_counters() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let mut config = Config::default();
    config.context_window_tokens = 150;

    let manager = SessionManager::new(pool.clone(), None, None, 24);
    let session = manager
        .create_session("agent:test:ui:group:counter_test")
        .await?;

    // Add messages
    for i in 0..10 {
        manager
            .append_message(
                session.session_id,
                if i % 2 == 0 { "user" } else { "assistant" },
                format!("Message {}", i),
                Some(10),
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
    }

    // Run compaction
    manager
        .compact_session(
            session.session_id,
            CompactionTrigger::ManualRequest,
            None,
            &config,
            None,
            false,
        )
        .await?;

    // Verify compaction count
    let session_after: Session = sqlx::query_as(
        "SELECT * FROM sessions WHERE session_id = $1"
    )
    .bind(session.session_id)
    .fetch_one(&pool)
    .await?;
    
    assert_eq!(session_after.compaction_count, 1);

    // Verify token counters were recalculated
    let total = session_after.token_counters.get("total").and_then(|v| v.as_i64()).unwrap_or(0);
    assert!(total > 0, "Token counters should be recalculated");

    // Cleanup
    sqlx::query("DELETE FROM sessions WHERE session_id = $1")
        .bind(session.session_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_compaction_ledger_event_recorded() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let mut config = Config::default();
    let ledger = Ledger::new(pool.clone());
    
    config.context_window_tokens = 150;

    let manager = SessionManager::new(pool.clone(), None, None, 24);
    let session = manager
        .create_session("agent:test:ui:group:ledger_compact_test")
        .await?;

    // Add messages
    for i in 0..10 {
        manager
            .append_message(
                session.session_id,
                if i % 2 == 0 { "user" } else { "assistant" },
                format!("Message {}", i),
                Some(10),
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
    }

    // Run compaction with ledger
    let correlation_id = Uuid::now_v7();
    manager
        .compact_session(
            session.session_id,
            CompactionTrigger::ManualRequest,
            Some(correlation_id),
            &config,
            Some(&ledger),
            false,
        )
        .await?;

    // Verify ledger event was recorded
    let events = sqlx::query_scalar::<_, String>(
        "SELECT action_type FROM ledger_events WHERE correlation_id = $1",
    )
    .bind(correlation_id)
    .fetch_all(&pool)
    .await?;

    assert!(!events.is_empty(), "Should have ledger events");
    assert!(
        events.iter().any(|e| e == "session.compacted"),
        "Should have session.compacted event"
    );

    // Cleanup
    sqlx::query("DELETE FROM sessions WHERE session_id = $1")
        .bind(session.session_id)
        .execute(&pool)
        .await?;
    
    sqlx::query("DELETE FROM ledger_events WHERE correlation_id = $1")
        .bind(correlation_id)
        .execute(&pool)
        .await?;

    Ok(())
}
