//! Integration tests for memory management system
//!
//! These tests verify the full memory workflow including:
//! - Creating and retrieving memories
//! - Loading recent memories (today + yesterday)
//! - Searching memories by similarity
//! - Updating access counts
//! - Querying with filters

use carnelian_common::Result;
use carnelian_core::memory::{MemoryManager, MemorySource};
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
async fn test_create_and_retrieve_memory() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let manager = MemoryManager::new(pool.clone(), None);

    // Create test identity
    let identity_id = create_test_identity(&pool, "test_memory_user").await?;

    // Create a memory
    let memory = manager
        .create_memory(
            identity_id,
            "User prefers concise responses",
            Some("Communication preference".to_string()),
            MemorySource::Conversation,
            None, // no embedding
            0.9,  // high importance
            None, // no tags
        )
        .await?;

    // Verify memory was created
    assert_eq!(memory.identity_id, identity_id);
    assert_eq!(memory.content, "User prefers concise responses");
    assert!((memory.importance - 0.9).abs() < f32::EPSILON);

    // Retrieve the memory
    let retrieved = manager.get_memory(memory.memory_id).await?;
    assert!(retrieved.is_some(), "Memory should be retrievable");

    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.memory_id, memory.memory_id);
    assert_eq!(retrieved.content, memory.content);

    // Cleanup
    sqlx::query("DELETE FROM memories WHERE memory_id = $1")
        .bind(memory.memory_id)
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
async fn test_load_recent_memories_today_yesterday() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let manager = MemoryManager::new(pool.clone(), None);

    // Create test identity
    let identity_id = create_test_identity(&pool, "test_recent_user").await?;

    // Create several memories
    let memory1 = manager
        .create_memory(
            identity_id,
            "Recent memory 1",
            None,
            MemorySource::Conversation,
            None,
            0.7,
            None,
        )
        .await?;

    let memory2 = manager
        .create_memory(
            identity_id,
            "Recent memory 2",
            None,
            MemorySource::Conversation,
            None,
            0.8,
            None,
        )
        .await?;

    // Load recent memories (48hr window)
    let recent = manager.load_recent_memories(identity_id, 10).await?;

    // Verify memories were loaded
    assert!(recent.len() >= 2, "Should load at least 2 recent memories");

    let memory_ids: Vec<Uuid> = recent.iter().map(|m| m.memory_id).collect();
    assert!(memory_ids.contains(&memory1.memory_id));
    assert!(memory_ids.contains(&memory2.memory_id));

    // Cleanup
    sqlx::query("DELETE FROM memories WHERE identity_id = $1")
        .bind(identity_id)
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
async fn test_load_high_importance_memories() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let manager = MemoryManager::new(pool.clone(), None);

    // Create test identity
    let identity_id = create_test_identity(&pool, "test_importance_user").await?;

    // Create memories with different importance levels
    let _low = manager
        .create_memory(
            identity_id,
            "Low importance memory",
            None,
            MemorySource::Conversation,
            None,
            0.3,
            None,
        )
        .await?;

    let high = manager
        .create_memory(
            identity_id,
            "High importance memory",
            None,
            MemorySource::Conversation,
            None,
            0.9,
            None,
        )
        .await?;

    // Query for high-importance memories (>0.8)
    let high_importance = sqlx::query_scalar::<_, Uuid>(
        "SELECT memory_id FROM memories WHERE identity_id = $1 AND importance > 0.8 ORDER BY importance DESC",
    )
    .bind(identity_id)
    .fetch_all(&pool)
    .await?;

    // Verify only high-importance memory was returned
    assert_eq!(high_importance.len(), 1);
    assert_eq!(high_importance[0], high.memory_id);

    // Cleanup
    sqlx::query("DELETE FROM memories WHERE identity_id = $1")
        .bind(identity_id)
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
async fn test_update_access_count() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let manager = MemoryManager::new(pool.clone(), None);

    // Create test identity
    let identity_id = create_test_identity(&pool, "test_access_user").await?;

    // Create a memory
    let memory = manager
        .create_memory(
            identity_id,
            "Test access tracking",
            None,
            MemorySource::Conversation,
            None,
            0.5,
            None,
        )
        .await?;

    // Initial access count should be 0
    assert_eq!(memory.access_count, 0);

    // Update access count
    manager.update_access_count(memory.memory_id).await?;

    // Retrieve and verify access count was incremented
    let updated = manager.get_memory(memory.memory_id).await?.unwrap();
    assert_eq!(updated.access_count, 1);
    // accessed_at is updated automatically when access_count is incremented

    // Cleanup
    sqlx::query("DELETE FROM memories WHERE memory_id = $1")
        .bind(memory.memory_id)
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
async fn test_query_memories_with_filters() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let manager = MemoryManager::new(pool.clone(), None);

    // Create test identity
    let identity_id = create_test_identity(&pool, "test_filter_user").await?;

    // Create memories with different sources
    let conv_memory = manager
        .create_memory(
            identity_id,
            "Conversation memory",
            None,
            MemorySource::Conversation,
            None,
            0.5,
            None,
        )
        .await?;

    let task_memory = manager
        .create_memory(
            identity_id,
            "Task memory",
            None,
            MemorySource::Task,
            None,
            0.6,
            None,
        )
        .await?;

    // Query by source filter
    let conv_only = sqlx::query_scalar::<_, Uuid>(
        "SELECT memory_id FROM memories WHERE identity_id = $1 AND source = $2",
    )
    .bind(identity_id)
    .bind("conversation")
    .fetch_all(&pool)
    .await?;

    assert_eq!(conv_only.len(), 1);
    assert_eq!(conv_only[0], conv_memory.memory_id);

    // Query by importance filter
    let important = sqlx::query_scalar::<_, Uuid>(
        "SELECT memory_id FROM memories WHERE identity_id = $1 AND importance >= $2",
    )
    .bind(identity_id)
    .bind(0.6)
    .fetch_all(&pool)
    .await?;

    assert_eq!(important.len(), 1);
    assert_eq!(important[0], task_memory.memory_id);

    // Cleanup
    sqlx::query("DELETE FROM memories WHERE identity_id = $1")
        .bind(identity_id)
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
async fn test_memory_stats() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let manager = MemoryManager::new(pool.clone(), None);

    // Create test identity
    let identity_id = create_test_identity(&pool, "test_stats_user").await?;

    // Create multiple memories
    #[allow(clippy::cast_precision_loss)]
    for i in 0..5 {
        manager
            .create_memory(
                identity_id,
                &format!("Memory {i}"),
                None,
                MemorySource::Conversation,
                None,
                0.5 + (i as f32).mul_add(0.1, 0.0),
                None,
            )
            .await?;
    }

    // Get memory stats
    let count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM memories WHERE identity_id = $1")
            .bind(identity_id)
            .fetch_one(&pool)
            .await?;

    assert_eq!(count, 5, "Should have 5 memories");

    let avg_importance =
        sqlx::query_scalar::<_, f32>("SELECT AVG(importance) FROM memories WHERE identity_id = $1")
            .bind(identity_id)
            .fetch_one(&pool)
            .await?;

    assert!(
        avg_importance > 0.6 && avg_importance < 0.7,
        "Average importance should be around 0.65"
    );

    // Cleanup
    sqlx::query("DELETE FROM memories WHERE identity_id = $1")
        .bind(identity_id)
        .execute(&pool)
        .await?;

    sqlx::query("DELETE FROM identities WHERE identity_id = $1")
        .bind(identity_id)
        .execute(&pool)
        .await?;

    Ok(())
}
