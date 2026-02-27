//! Unit tests for Elixir Manager
//!
//! Tests cover:
//! - Elixir creation and retrieval
//! - Elixir versioning
//! - Elixir usage tracking
//! - Auto-draft generation
//! - Elixir approval/rejection

use carnelian_core::elixir::ElixirManager;
use carnelian_common::types::CreateElixirRequest;
use serde_json::json;
use crate::helpers::*;

#[tokio::test]
async fn test_create_elixir() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = ElixirManager::new(pool.clone());
    
    let request = CreateElixirRequest {
        skill_name: "test-skill".to_string(),
        dataset: json!({
            "examples": [
                {"input": "test", "output": "result"}
            ]
        }),
        quality_score: 0.95,
        metadata: json!({"source": "test"}),
    };
    
    let elixir = manager.create(request).await.unwrap();
    
    assert_eq!(elixir.skill_name, "test-skill");
    assert_eq!(elixir.quality_score, 0.95);
    assert!(elixir.version > 0);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_get_elixir() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = ElixirManager::new(pool.clone());
    
    let created = manager.create(CreateElixirRequest {
        skill_name: "test-skill".to_string(),
        dataset: json!({"data": "test"}),
        quality_score: 0.9,
        metadata: json!({}),
    }).await.unwrap();
    
    let retrieved = manager.get(created.id).await.unwrap();
    
    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.skill_name, "test-skill");
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_list_elixirs() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = ElixirManager::new(pool.clone());
    
    // Create multiple elixirs
    for i in 0..5 {
        manager.create(CreateElixirRequest {
            skill_name: format!("skill-{}", i),
            dataset: json!({"index": i}),
            quality_score: 0.8 + (i as f64 * 0.02),
            metadata: json!({}),
        }).await.unwrap();
    }
    
    let elixirs = manager.list(None, 10, 0).await.unwrap();
    
    assert_eq!(elixirs.len(), 5);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_elixir_versioning() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = ElixirManager::new(pool.clone());
    
    // Create first version
    let v1 = manager.create(CreateElixirRequest {
        skill_name: "versioned-skill".to_string(),
        dataset: json!({"version": 1}),
        quality_score: 0.8,
        metadata: json!({}),
    }).await.unwrap();
    
    // Create second version (same skill)
    let v2 = manager.create(CreateElixirRequest {
        skill_name: "versioned-skill".to_string(),
        dataset: json!({"version": 2}),
        quality_score: 0.9,
        metadata: json!({}),
    }).await.unwrap();
    
    assert_eq!(v2.version, v1.version + 1);
    assert_eq!(v2.skill_name, v1.skill_name);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_elixir_usage_tracking() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = ElixirManager::new(pool.clone());
    
    let elixir = manager.create(CreateElixirRequest {
        skill_name: "tracked-skill".to_string(),
        dataset: json!({}),
        quality_score: 0.85,
        metadata: json!({}),
    }).await.unwrap();
    
    // Track usage
    manager.track_usage(elixir.id, true, 0.9).await.unwrap();
    manager.track_usage(elixir.id, true, 0.95).await.unwrap();
    manager.track_usage(elixir.id, false, 0.5).await.unwrap();
    
    let usage = manager.get_usage_stats(elixir.id).await.unwrap();
    
    assert_eq!(usage.total_uses, 3);
    assert_eq!(usage.successful_uses, 2);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_elixir_quality_filtering() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = ElixirManager::new(pool.clone());
    
    // Create elixirs with different quality scores
    manager.create(CreateElixirRequest {
        skill_name: "low-quality".to_string(),
        dataset: json!({}),
        quality_score: 0.5,
        metadata: json!({}),
    }).await.unwrap();
    
    manager.create(CreateElixirRequest {
        skill_name: "high-quality".to_string(),
        dataset: json!({}),
        quality_score: 0.95,
        metadata: json!({}),
    }).await.unwrap();
    
    // Filter by minimum quality
    let high_quality = manager.list(Some(0.9), 10, 0).await.unwrap();
    
    assert_eq!(high_quality.len(), 1);
    assert_eq!(high_quality[0].skill_name, "high-quality");
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_elixir_search() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = ElixirManager::new(pool.clone());
    
    manager.create(CreateElixirRequest {
        skill_name: "data-processing".to_string(),
        dataset: json!({}),
        quality_score: 0.9,
        metadata: json!({"category": "data"}),
    }).await.unwrap();
    
    manager.create(CreateElixirRequest {
        skill_name: "image-processing".to_string(),
        dataset: json!({}),
        quality_score: 0.85,
        metadata: json!({"category": "image"}),
    }).await.unwrap();
    
    let results = manager.search("processing").await.unwrap();
    
    assert_eq!(results.len(), 2);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_auto_draft_creation() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = ElixirManager::new(pool.clone());
    
    // Simulate skill with 100+ usages
    let skill_name = "popular-skill";
    
    // Check if auto-draft should be created
    let should_create = manager.should_create_auto_draft(skill_name).await.unwrap();
    
    // Initially should not create (no usage data)
    assert!(!should_create);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_elixir_draft_approval() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = ElixirManager::new(pool.clone());
    
    // Create a draft
    let draft = manager.create_draft(
        "draft-skill".to_string(),
        json!({"draft": "data"}),
        0.88,
    ).await.unwrap();
    
    // Approve the draft
    let elixir = manager.approve_draft(draft.id).await.unwrap();
    
    assert_eq!(elixir.skill_name, "draft-skill");
    assert_eq!(elixir.quality_score, 0.88);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_elixir_draft_rejection() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = ElixirManager::new(pool.clone());
    
    // Create a draft
    let draft = manager.create_draft(
        "rejected-skill".to_string(),
        json!({}),
        0.7,
    ).await.unwrap();
    
    // Reject the draft
    manager.reject_draft(draft.id, "Quality too low".to_string()).await.unwrap();
    
    // Draft should be marked as rejected
    let drafts = manager.list_drafts(10, 0).await.unwrap();
    let rejected = drafts.iter().find(|d| d.id == draft.id).unwrap();
    
    assert_eq!(rejected.status, "rejected");
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_elixir_pagination() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = ElixirManager::new(pool.clone());
    
    // Create 15 elixirs
    for i in 0..15 {
        manager.create(CreateElixirRequest {
            skill_name: format!("skill-{}", i),
            dataset: json!({}),
            quality_score: 0.8,
            metadata: json!({}),
        }).await.unwrap();
    }
    
    // Get first page
    let page1 = manager.list(None, 10, 0).await.unwrap();
    assert_eq!(page1.len(), 10);
    
    // Get second page
    let page2 = manager.list(None, 10, 10).await.unwrap();
    assert_eq!(page2.len(), 5);
    
    cleanup_test_db(&pool).await.unwrap();
}
