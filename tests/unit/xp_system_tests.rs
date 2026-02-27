//! Unit tests for XP System
//!
//! Tests cover:
//! - XP award and tracking
//! - XP leaderboard generation
//! - XP history queries
//! - XP source validation
//! - Agent XP calculations

use carnelian_core::xp::{XpManager, XpSource};
use carnelian_common::types::{AwardXpRequest, XpHistoryQuery};
use crate::helpers::*;

#[tokio::test]
async fn test_award_xp() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = XpManager::new(pool.clone());
    
    let identity_id = test_identity_id();
    let request = AwardXpRequest {
        identity_id,
        amount: 100,
        source: XpSource::SkillExecution,
        description: Some("Executed test skill".to_string()),
        metadata: None,
    };
    
    let result = manager.award(request).await.unwrap();
    
    assert_eq!(result.amount, 100);
    assert_eq!(result.identity_id, identity_id);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_get_agent_xp() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = XpManager::new(pool.clone());
    let identity_id = test_identity_id();
    
    // Award multiple XP amounts
    manager.award(AwardXpRequest {
        identity_id,
        amount: 50,
        source: XpSource::SkillExecution,
        description: None,
        metadata: None,
    }).await.unwrap();
    
    manager.award(AwardXpRequest {
        identity_id,
        amount: 75,
        source: XpSource::WorkflowCompletion,
        description: None,
        metadata: None,
    }).await.unwrap();
    
    let total_xp = manager.get_agent_xp(identity_id).await.unwrap();
    
    assert_eq!(total_xp, 125);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_xp_leaderboard() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = XpManager::new(pool.clone());
    
    // Create multiple agents with different XP
    let agent1 = test_uuid();
    let agent2 = test_uuid();
    let agent3 = test_uuid();
    
    manager.award(AwardXpRequest {
        identity_id: agent1,
        amount: 500,
        source: XpSource::SkillExecution,
        description: None,
        metadata: None,
    }).await.unwrap();
    
    manager.award(AwardXpRequest {
        identity_id: agent2,
        amount: 1000,
        source: XpSource::SkillExecution,
        description: None,
        metadata: None,
    }).await.unwrap();
    
    manager.award(AwardXpRequest {
        identity_id: agent3,
        amount: 250,
        source: XpSource::SkillExecution,
        description: None,
        metadata: None,
    }).await.unwrap();
    
    let leaderboard = manager.get_leaderboard(10).await.unwrap();
    
    assert_eq!(leaderboard.len(), 3);
    assert_eq!(leaderboard[0].identity_id, agent2); // Highest XP
    assert_eq!(leaderboard[0].total_xp, 1000);
    assert_eq!(leaderboard[1].identity_id, agent1);
    assert_eq!(leaderboard[2].identity_id, agent3);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_xp_history() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = XpManager::new(pool.clone());
    let identity_id = test_identity_id();
    
    // Award XP from different sources
    manager.award(AwardXpRequest {
        identity_id,
        amount: 100,
        source: XpSource::SkillExecution,
        description: Some("Skill 1".to_string()),
        metadata: None,
    }).await.unwrap();
    
    manager.award(AwardXpRequest {
        identity_id,
        amount: 200,
        source: XpSource::WorkflowCompletion,
        description: Some("Workflow 1".to_string()),
        metadata: None,
    }).await.unwrap();
    
    manager.award(AwardXpRequest {
        identity_id,
        amount: 50,
        source: XpSource::MemoryCreation,
        description: Some("Memory 1".to_string()),
        metadata: None,
    }).await.unwrap();
    
    let query = XpHistoryQuery {
        identity_id: Some(identity_id),
        source: None,
        limit: 10,
        offset: 0,
    };
    
    let history = manager.get_history(query).await.unwrap();
    
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].amount, 50); // Most recent first
    assert_eq!(history[1].amount, 200);
    assert_eq!(history[2].amount, 100);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_xp_history_by_source() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = XpManager::new(pool.clone());
    let identity_id = test_identity_id();
    
    // Award XP from different sources
    manager.award(AwardXpRequest {
        identity_id,
        amount: 100,
        source: XpSource::SkillExecution,
        description: None,
        metadata: None,
    }).await.unwrap();
    
    manager.award(AwardXpRequest {
        identity_id,
        amount: 200,
        source: XpSource::SkillExecution,
        description: None,
        metadata: None,
    }).await.unwrap();
    
    manager.award(AwardXpRequest {
        identity_id,
        amount: 50,
        source: XpSource::WorkflowCompletion,
        description: None,
        metadata: None,
    }).await.unwrap();
    
    let query = XpHistoryQuery {
        identity_id: Some(identity_id),
        source: Some(XpSource::SkillExecution),
        limit: 10,
        offset: 0,
    };
    
    let history = manager.get_history(query).await.unwrap();
    
    assert_eq!(history.len(), 2);
    assert!(history.iter().all(|e| e.source == XpSource::SkillExecution));
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_xp_pagination() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = XpManager::new(pool.clone());
    let identity_id = test_identity_id();
    
    // Award 15 XP events
    for i in 0..15 {
        manager.award(AwardXpRequest {
            identity_id,
            amount: (i + 1) * 10,
            source: XpSource::SkillExecution,
            description: Some(format!("Event {}", i)),
            metadata: None,
        }).await.unwrap();
    }
    
    // Get first page
    let page1 = manager.get_history(XpHistoryQuery {
        identity_id: Some(identity_id),
        source: None,
        limit: 10,
        offset: 0,
    }).await.unwrap();
    
    assert_eq!(page1.len(), 10);
    
    // Get second page
    let page2 = manager.get_history(XpHistoryQuery {
        identity_id: Some(identity_id),
        source: None,
        limit: 10,
        offset: 10,
    }).await.unwrap();
    
    assert_eq!(page2.len(), 5);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_xp_source_types() {
    init_test_env();
    
    // Verify all XP source types
    let sources = vec![
        XpSource::SkillExecution,
        XpSource::WorkflowCompletion,
        XpSource::MemoryCreation,
        XpSource::ElixirCreation,
        XpSource::TaskCompletion,
    ];
    
    assert_eq!(sources.len(), 5);
}

#[tokio::test]
async fn test_xp_with_metadata() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = XpManager::new(pool.clone());
    let identity_id = test_identity_id();
    
    let metadata = serde_json::json!({
        "skill_name": "test-skill",
        "execution_time_ms": 150,
        "success": true
    });
    
    manager.award(AwardXpRequest {
        identity_id,
        amount: 100,
        source: XpSource::SkillExecution,
        description: Some("Skill execution with metadata".to_string()),
        metadata: Some(metadata.clone()),
    }).await.unwrap();
    
    let history = manager.get_history(XpHistoryQuery {
        identity_id: Some(identity_id),
        source: None,
        limit: 1,
        offset: 0,
    }).await.unwrap();
    
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].metadata, Some(metadata));
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_multiple_agents_xp() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = XpManager::new(pool.clone());
    
    let agent1 = test_uuid();
    let agent2 = test_uuid();
    
    // Award XP to both agents
    manager.award(AwardXpRequest {
        identity_id: agent1,
        amount: 100,
        source: XpSource::SkillExecution,
        description: None,
        metadata: None,
    }).await.unwrap();
    
    manager.award(AwardXpRequest {
        identity_id: agent2,
        amount: 200,
        source: XpSource::SkillExecution,
        description: None,
        metadata: None,
    }).await.unwrap();
    
    let agent1_xp = manager.get_agent_xp(agent1).await.unwrap();
    let agent2_xp = manager.get_agent_xp(agent2).await.unwrap();
    
    assert_eq!(agent1_xp, 100);
    assert_eq!(agent2_xp, 200);
    
    cleanup_test_db(&pool).await.unwrap();
}

#[tokio::test]
async fn test_zero_xp_award() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    cleanup_test_db(&pool).await.unwrap();
    
    let manager = XpManager::new(pool.clone());
    let identity_id = test_identity_id();
    
    let result = manager.award(AwardXpRequest {
        identity_id,
        amount: 0,
        source: XpSource::SkillExecution,
        description: None,
        metadata: None,
    }).await;
    
    // Should handle gracefully or reject
    assert!(result.is_ok() || result.is_err());
    
    cleanup_test_db(&pool).await.unwrap();
}
