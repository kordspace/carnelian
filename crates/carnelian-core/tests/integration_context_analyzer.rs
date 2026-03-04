//! Integration tests for ContextAnalyzer
//!
//! Tests conversation-to-task creation flow with real database

use carnelian_core::context_analyzer::{ActionItem, ContextAnalyzer};
use carnelian_magic::MantraTree;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[sqlx::test]
async fn test_analyze_and_create_tasks_integration(pool: PgPool) -> sqlx::Result<()> {
    let pool = Arc::new(pool);
    let mantra_tree = Arc::new(MantraTree::new(None));
    let analyzer = ContextAnalyzer::new(pool.clone(), mantra_tree);

    // Create a test session
    let session_id = Uuid::new_v4();
    let agent_id = Uuid::new_v4();

    // Insert test identity
    sqlx::query!(
        "INSERT INTO identities (identity_id, name, identity_type) VALUES ($1, 'Test Agent', 'core')",
        agent_id
    )
    .execute(pool.as_ref())
    .await?;

    // Insert test session
    sqlx::query!(
        "INSERT INTO sessions (session_id, session_key, agent_id) VALUES ($1, 'test-session', $2)",
        session_id,
        agent_id
    )
    .execute(pool.as_ref())
    .await?;

    // Insert test messages with action items
    sqlx::query!(
        "INSERT INTO session_messages (session_id, role, content) VALUES ($1, 'user', 'We need to implement OAuth2 authentication')",
        session_id
    )
    .execute(pool.as_ref())
    .await?;

    sqlx::query!(
        "INSERT INTO session_messages (session_id, role, content) VALUES ($1, 'user', 'TODO: Fix the login bug in production')",
        session_id
    )
    .execute(pool.as_ref())
    .await?;

    // Analyze session
    let action_items = analyzer
        .analyze_session(session_id, 10)
        .await
        .expect("Failed to analyze session");

    assert!(
        !action_items.is_empty(),
        "Should extract action items from messages"
    );

    // Create tasks from action items
    let created_count = analyzer
        .create_tasks_from_items(session_id, &action_items)
        .await
        .expect("Failed to create tasks");

    assert!(created_count > 0, "Should create at least one task");

    // Verify tasks were created in database
    let tasks = sqlx::query!(
        "SELECT task_id, title, state, correlation_id FROM tasks WHERE correlation_id = $1",
        session_id
    )
    .fetch_all(pool.as_ref())
    .await?;

    assert_eq!(tasks.len(), created_count, "Task count mismatch");
    assert!(
        tasks.iter().all(|t| t.state == "pending"),
        "All tasks should be pending"
    );
    assert!(
        tasks.iter().all(|t| t.correlation_id == Some(session_id)),
        "All tasks should have correct correlation_id"
    );

    Ok(())
}

#[sqlx::test]
async fn test_migration_18_19_smoke_check(pool: PgPool) -> sqlx::Result<()> {
    // Verify migration 18 (key_algorithm column) exists
    let key_algo_check = sqlx::query!(
        "SELECT column_name FROM information_schema.columns 
         WHERE table_name = 'config_store' AND column_name = 'key_algorithm'"
    )
    .fetch_optional(&pool)
    .await?;

    assert!(
        key_algo_check.is_some(),
        "Migration 18: key_algorithm column should exist"
    );

    // Verify migration 19 (skill_execution_log table) exists
    let exec_log_check = sqlx::query!(
        "SELECT table_name FROM information_schema.tables 
         WHERE table_name = 'skill_execution_log'"
    )
    .fetch_optional(&pool)
    .await?;

    assert!(
        exec_log_check.is_some(),
        "Migration 19: skill_execution_log table should exist"
    );

    Ok(())
}
