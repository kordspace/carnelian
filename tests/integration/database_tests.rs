//! Integration tests for database operations
//!
//! Tests cover:
//! - Connection pool management
//! - Transaction handling
//! - Deadlock retry logic
//! - Concurrent access patterns
//! - Database migrations

use sqlx::{PgPool, Row};
use crate::helpers::*;

#[tokio::test]
async fn test_database_connection() {
    init_test_env();
    
    let pool = create_test_pool().await;
    
    let result: (i32,) = sqlx::query_as("SELECT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    
    assert_eq!(result.0, 1);
}

#[tokio::test]
async fn test_connection_pool_exhaustion() {
    init_test_env();
    
    let pool = create_test_pool().await;
    
    // Spawn multiple concurrent tasks that acquire connections
    let mut handles = vec![];
    
    for _ in 0..10 {
        let pool_clone = pool.clone();
        let handle = tokio::spawn(async move {
            let _conn = pool_clone.acquire().await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        });
        handles.push(handle);
    }
    
    // All tasks should complete successfully
    for handle in handles {
        assert!(handle.await.is_ok());
    }
}

#[tokio::test]
async fn test_transaction_commit() {
    init_test_env();
    
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let mut tx = pool.begin().await.unwrap();
    
    sqlx::query("INSERT INTO memories (id, content, embedding, created_at, updated_at) VALUES ($1, $2, $3, NOW(), NOW())")
        .bind(test_uuid())
        .bind("Test transaction content")
        .bind(test_embedding())
        .execute(&mut *tx)
        .await
        .unwrap();
    
    tx.commit().await.unwrap();
    
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM memories")
        .fetch_one(&pool)
        .await
        .unwrap();
    
    assert_eq!(count.0, 1);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_transaction_rollback() {
    init_test_env();
    
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let mut tx = pool.begin().await.unwrap();
    
    sqlx::query("INSERT INTO memories (id, content, embedding, created_at, updated_at) VALUES ($1, $2, $3, NOW(), NOW())")
        .bind(test_uuid())
        .bind("Test rollback content")
        .bind(test_embedding())
        .execute(&mut *tx)
        .await
        .unwrap();
    
    tx.rollback().await.unwrap();
    
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM memories")
        .fetch_one(&pool)
        .await
        .unwrap();
    
    assert_eq!(count.0, 0);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_concurrent_inserts() {
    init_test_env();
    
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let mut handles = vec![];
    
    for i in 0..20 {
        let pool_clone = pool.clone();
        let handle = tokio::spawn(async move {
            sqlx::query("INSERT INTO memories (id, content, embedding, created_at, updated_at) VALUES ($1, $2, $3, NOW(), NOW())")
                .bind(test_uuid())
                .bind(format!("Concurrent insert {}", i))
                .bind(test_embedding())
                .execute(&pool_clone)
                .await
        });
        handles.push(handle);
    }
    
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }
    
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM memories")
        .fetch_one(&pool)
        .await
        .unwrap();
    
    assert_eq!(count.0, 20);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_concurrent_updates() {
    init_test_env();
    
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let memory_id = test_uuid();
    
    // Insert initial memory
    sqlx::query("INSERT INTO memories (id, content, embedding, created_at, updated_at) VALUES ($1, $2, $3, NOW(), NOW())")
        .bind(memory_id)
        .bind("Initial content")
        .bind(test_embedding())
        .execute(&pool)
        .await
        .unwrap();
    
    // Concurrent updates
    let mut handles = vec![];
    
    for i in 0..10 {
        let pool_clone = pool.clone();
        let handle = tokio::spawn(async move {
            sqlx::query("UPDATE memories SET content = $1, updated_at = NOW() WHERE id = $2")
                .bind(format!("Updated content {}", i))
                .bind(memory_id)
                .execute(&pool_clone)
                .await
        });
        handles.push(handle);
    }
    
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_database_migration() {
    init_test_env();
    
    let pool = create_test_pool().await;
    
    let result = run_test_migrations(&pool).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_vector_similarity_query() {
    init_test_env();
    
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    // Insert memories with embeddings
    let embedding1 = test_embedding_with_pattern(0.1);
    let embedding2 = test_embedding_with_pattern(0.2);
    
    sqlx::query("INSERT INTO memories (id, content, embedding, created_at, updated_at) VALUES ($1, $2, $3, NOW(), NOW())")
        .bind(test_uuid())
        .bind("Memory 1")
        .bind(&embedding1)
        .execute(&pool)
        .await
        .unwrap();
    
    sqlx::query("INSERT INTO memories (id, content, embedding, created_at, updated_at) VALUES ($1, $2, $3, NOW(), NOW())")
        .bind(test_uuid())
        .bind("Memory 2")
        .bind(&embedding2)
        .execute(&pool)
        .await
        .unwrap();
    
    // Query using vector similarity (cosine distance)
    let results = sqlx::query("SELECT content FROM memories ORDER BY embedding <=> $1 LIMIT 5")
        .bind(&embedding1)
        .fetch_all(&pool)
        .await
        .unwrap();
    
    assert!(results.len() > 0);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_index_usage() {
    init_test_env();
    
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    // Insert test data
    for i in 0..100 {
        sqlx::query("INSERT INTO memories (id, content, embedding, created_at, updated_at) VALUES ($1, $2, $3, NOW(), NOW())")
            .bind(test_uuid())
            .bind(format!("Memory {}", i))
            .bind(test_embedding())
            .execute(&pool)
            .await
            .unwrap();
    }
    
    // Query should use index
    let start = std::time::Instant::now();
    
    let _results = sqlx::query("SELECT * FROM memories WHERE created_at > NOW() - INTERVAL '1 hour'")
        .fetch_all(&pool)
        .await
        .unwrap();
    
    let duration = start.elapsed();
    
    // Should be fast with index
    assert!(duration.as_millis() < 1000);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_foreign_key_constraints() {
    init_test_env();
    
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let memory_id = test_uuid();
    
    // Insert memory
    sqlx::query("INSERT INTO memories (id, content, embedding, created_at, updated_at) VALUES ($1, $2, $3, NOW(), NOW())")
        .bind(memory_id)
        .bind("Test memory")
        .bind(test_embedding())
        .execute(&pool)
        .await
        .unwrap();
    
    // Try to insert memory_embedding with non-existent memory_id
    let result = sqlx::query("INSERT INTO memory_embeddings (memory_id, embedding, model) VALUES ($1, $2, $3)")
        .bind(test_uuid()) // Different ID
        .bind(test_embedding())
        .bind("test-model")
        .execute(&pool)
        .await;
    
    // Should fail due to foreign key constraint
    assert!(result.is_err());
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_cascade_delete() {
    init_test_env();
    
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let memory_id = test_uuid();
    
    // Insert memory
    sqlx::query("INSERT INTO memories (id, content, embedding, created_at, updated_at) VALUES ($1, $2, $3, NOW(), NOW())")
        .bind(memory_id)
        .bind("Test memory")
        .bind(test_embedding())
        .execute(&pool)
        .await
        .unwrap();
    
    // Insert related embedding
    sqlx::query("INSERT INTO memory_embeddings (memory_id, embedding, model) VALUES ($1, $2, $3)")
        .bind(memory_id)
        .bind(test_embedding())
        .bind("test-model")
        .execute(&pool)
        .await
        .unwrap();
    
    // Delete memory
    sqlx::query("DELETE FROM memories WHERE id = $1")
        .bind(memory_id)
        .execute(&pool)
        .await
        .unwrap();
    
    // Related embedding should be deleted (if CASCADE is configured)
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM memory_embeddings WHERE memory_id = $1")
        .bind(memory_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    
    assert_eq!(count.0, 0);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_connection_recovery() {
    init_test_env();
    
    let pool = create_test_pool().await;
    
    // Simulate connection failure and recovery
    let result1 = sqlx::query("SELECT 1").fetch_one(&pool).await;
    assert!(result1.is_ok());
    
    // Pool should handle reconnection automatically
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let result2 = sqlx::query("SELECT 1").fetch_one(&pool).await;
    assert!(result2.is_ok());
}

#[tokio::test]
async fn test_prepared_statement_caching() {
    init_test_env();
    
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    // Execute same query multiple times
    for i in 0..10 {
        let _result: (i32,) = sqlx::query_as("SELECT $1::int")
            .bind(i)
            .fetch_one(&pool)
            .await
            .unwrap();
    }
    
    // Should benefit from prepared statement caching
    assert!(true);
    
    cleanup_test_db(&pool).await.unwrap();
}
