//! Unit tests for Memory Manager
//!
//! Tests cover:
//! - Memory creation and retrieval
//! - Vector similarity search
//! - Memory updates and deletion
//! - Tag-based filtering
//! - Metadata queries

use carnelian_core::memory::MemoryManager;
use carnelian_common::types::{CreateMemoryRequest, UpdateMemoryRequest};
use serde_json::json;
use crate::helpers::*;

#[tokio::test]
async fn test_create_memory() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = MemoryManager::new(pool.clone());
    
    let request = CreateMemoryRequest {
        content: "Test memory content for unit testing".to_string(),
        metadata: json!({
            "type": "test",
            "source": "unit_test",
            "priority": "high"
        }),
        tags: vec!["test".to_string(), "unit".to_string()],
        identity_id: Some(test_identity_id()),
    };
    
    let memory = manager.create(request).await.unwrap();
    
    assert_eq!(memory.content, "Test memory content for unit testing");
    assert_eq!(memory.tags.len(), 2);
    assert!(memory.tags.contains(&"test".to_string()));
    assert!(memory.embedding.len() > 0);
    assert_eq!(memory.identity_id, Some(test_identity_id()));
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_get_memory() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = MemoryManager::new(pool.clone());
    
    let created = manager.create(CreateMemoryRequest {
        content: "Memory to retrieve".to_string(),
        metadata: json!({}),
        tags: vec![],
        identity_id: None,
    }).await.unwrap();
    
    let retrieved = manager.get(created.id).await.unwrap();
    
    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.content, "Memory to retrieve");
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_vector_similarity_search() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = MemoryManager::new(pool.clone());
    
    // Create multiple memories with different content
    let mem1 = manager.create(CreateMemoryRequest {
        content: "Rust programming language is great for systems programming".to_string(),
        metadata: json!({"topic": "programming"}),
        tags: vec!["rust".to_string()],
        identity_id: None,
    }).await.unwrap();
    
    let mem2 = manager.create(CreateMemoryRequest {
        content: "Python is excellent for data science and machine learning".to_string(),
        metadata: json!({"topic": "programming"}),
        tags: vec!["python".to_string()],
        identity_id: None,
    }).await.unwrap();
    
    let mem3 = manager.create(CreateMemoryRequest {
        content: "Cooking pasta requires boiling water and salt".to_string(),
        metadata: json!({"topic": "cooking"}),
        tags: vec!["food".to_string()],
        identity_id: None,
    }).await.unwrap();
    
    // Search using first memory's embedding
    let results = manager.search_similar(&mem1.embedding, 5, None).await.unwrap();
    
    assert!(results.len() >= 1);
    assert_eq!(results[0].id, mem1.id); // Most similar to itself
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_update_memory() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = MemoryManager::new(pool.clone());
    
    let created = manager.create(CreateMemoryRequest {
        content: "Original content".to_string(),
        metadata: json!({"version": 1}),
        tags: vec!["v1".to_string()],
        identity_id: None,
    }).await.unwrap();
    
    let updated = manager.update(created.id, UpdateMemoryRequest {
        content: Some("Updated content".to_string()),
        metadata: Some(json!({"version": 2})),
        tags: Some(vec!["v2".to_string()]),
    }).await.unwrap();
    
    assert_eq!(updated.content, "Updated content");
    assert_eq!(updated.metadata["version"], 2);
    assert!(updated.tags.contains(&"v2".to_string()));
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_delete_memory() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = MemoryManager::new(pool.clone());
    
    let created = manager.create(CreateMemoryRequest {
        content: "Memory to delete".to_string(),
        metadata: json!({}),
        tags: vec![],
        identity_id: None,
    }).await.unwrap();
    
    manager.delete(created.id).await.unwrap();
    
    let result = manager.get(created.id).await;
    assert!(result.is_err());
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_list_memories_with_filters() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = MemoryManager::new(pool.clone());
    
    // Create memories with different tags
    manager.create(CreateMemoryRequest {
        content: "Memory 1".to_string(),
        metadata: json!({}),
        tags: vec!["important".to_string()],
        identity_id: None,
    }).await.unwrap();
    
    manager.create(CreateMemoryRequest {
        content: "Memory 2".to_string(),
        metadata: json!({}),
        tags: vec!["important".to_string(), "urgent".to_string()],
        identity_id: None,
    }).await.unwrap();
    
    manager.create(CreateMemoryRequest {
        content: "Memory 3".to_string(),
        metadata: json!({}),
        tags: vec!["normal".to_string()],
        identity_id: None,
    }).await.unwrap();
    
    // List with tag filter
    let important_memories = manager.list(Some(vec!["important".to_string()]), None, 10, 0).await.unwrap();
    
    assert_eq!(important_memories.len(), 2);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_memory_pagination() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = MemoryManager::new(pool.clone());
    
    // Create 15 memories
    for i in 0..15 {
        manager.create(CreateMemoryRequest {
            content: format!("Memory {}", i),
            metadata: json!({}),
            tags: vec![],
            identity_id: None,
        }).await.unwrap();
    }
    
    // Get first page
    let page1 = manager.list(None, None, 10, 0).await.unwrap();
    assert_eq!(page1.len(), 10);
    
    // Get second page
    let page2 = manager.list(None, None, 10, 10).await.unwrap();
    assert_eq!(page2.len(), 5);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_memory_with_identity() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = MemoryManager::new(pool.clone());
    
    let identity1 = test_identity_id();
    let identity2 = test_uuid();
    
    // Create memories for different identities
    manager.create(CreateMemoryRequest {
        content: "Identity 1 memory".to_string(),
        metadata: json!({}),
        tags: vec![],
        identity_id: Some(identity1),
    }).await.unwrap();
    
    manager.create(CreateMemoryRequest {
        content: "Identity 2 memory".to_string(),
        metadata: json!({}),
        tags: vec![],
        identity_id: Some(identity2),
    }).await.unwrap();
    
    // List memories for identity 1
    let identity1_memories = manager.list(None, Some(identity1), 10, 0).await.unwrap();
    
    assert_eq!(identity1_memories.len(), 1);
    assert_eq!(identity1_memories[0].content, "Identity 1 memory");
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_memory_metadata_query() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = MemoryManager::new(pool.clone());
    
    manager.create(CreateMemoryRequest {
        content: "High priority task".to_string(),
        metadata: json!({"priority": "high", "status": "pending"}),
        tags: vec![],
        identity_id: None,
    }).await.unwrap();
    
    manager.create(CreateMemoryRequest {
        content: "Low priority task".to_string(),
        metadata: json!({"priority": "low", "status": "completed"}),
        tags: vec![],
        identity_id: None,
    }).await.unwrap();
    
    // Query by metadata (implementation depends on MemoryManager API)
    let all_memories = manager.list(None, None, 10, 0).await.unwrap();
    
    let high_priority: Vec<_> = all_memories.iter()
        .filter(|m| m.metadata.get("priority").and_then(|v| v.as_str()) == Some("high"))
        .collect();
    
    assert_eq!(high_priority.len(), 1);
    
    cleanup_test_db(&pool).await.unwrap();
}
