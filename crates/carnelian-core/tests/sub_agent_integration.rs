//! Integration tests for sub-agent lifecycle management
//!
//! These tests verify the full sub-agent workflow including:
//! - Creating and retrieving sub-agents
//! - Listing sub-agents with filters
//! - Pausing and resuming sub-agents
//! - Soft-deleting sub-agents

use carnelian_common::Result;
use carnelian_core::policy::PolicyEngine;
use carnelian_core::sub_agent::{CreateSubAgentRequest, SubAgentManager};
use serde_json::json;
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
async fn test_create_and_retrieve_sub_agent() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let manager = SubAgentManager::new(pool.clone(), None);
    let policy_engine = PolicyEngine::new(pool.clone());

    // Create test identities
    let parent_id = create_test_identity(&pool, "test_parent").await?;
    let created_by = create_test_identity(&pool, "test_creator").await?;

    // Create sub-agent
    let request = CreateSubAgentRequest {
        name: "TestSubAgent".to_string(),
        role: "test_role".to_string(),
        parent_id: Some(parent_id),
        directives: Some(json!(["Test directive 1", "Test directive 2"])),
        model_provider: None,
        ephemeral: false,
        capabilities: vec![],
        runtime: "node".to_string(),
    };

    let sub_agent = manager
        .create_sub_agent(parent_id, created_by, request, &policy_engine, None)
        .await?;

    // Verify sub-agent was created
    assert_eq!(sub_agent.name, "TestSubAgent");
    assert_eq!(sub_agent.role, "test_role");
    assert_eq!(sub_agent.parent_id, parent_id);
    assert_eq!(sub_agent.created_by, created_by);
    assert!(!sub_agent.ephemeral);

    // Retrieve sub-agent
    let retrieved = manager.get_sub_agent(sub_agent.sub_agent_id).await?;
    assert!(retrieved.is_some(), "Sub-agent should be retrievable");
    
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.sub_agent_id, sub_agent.sub_agent_id);
    assert_eq!(retrieved.name, sub_agent.name);

    // Cleanup
    sqlx::query("DELETE FROM sub_agents WHERE sub_agent_id = $1")
        .bind(sub_agent.sub_agent_id)
        .execute(&pool)
        .await?;
    
    sqlx::query("DELETE FROM identities WHERE identity_id IN ($1, $2, $3)")
        .bind(parent_id)
        .bind(created_by)
        .bind(sub_agent.sub_agent_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_list_sub_agents_with_filters() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let manager = SubAgentManager::new(pool.clone(), None);
    let policy_engine = PolicyEngine::new(pool.clone());

    // Create test identities
    let parent1 = create_test_identity(&pool, "test_parent1").await?;
    let parent2 = create_test_identity(&pool, "test_parent2").await?;
    let creator = create_test_identity(&pool, "test_creator2").await?;

    // Create sub-agents with different parents
    let request1 = CreateSubAgentRequest {
        name: "SubAgent1".to_string(),
        role: "role1".to_string(),
        parent_id: Some(parent1),
        directives: None,
        model_provider: None,
        ephemeral: false,
        capabilities: vec![],
        runtime: "node".to_string(),
    };

    let sub1 = manager
        .create_sub_agent(parent1, creator, request1, &policy_engine, None)
        .await?;

    let request2 = CreateSubAgentRequest {
        name: "SubAgent2".to_string(),
        role: "role2".to_string(),
        parent_id: Some(parent2),
        directives: None,
        model_provider: None,
        ephemeral: false,
        capabilities: vec![],
        runtime: "python".to_string(),
    };

    let sub2 = manager
        .create_sub_agent(parent2, creator, request2, &policy_engine, None)
        .await?;

    // List all sub-agents
    let all = manager.list_sub_agents(None, false).await?;
    assert!(all.len() >= 2, "Should list at least 2 sub-agents");

    // List sub-agents by parent filter
    let parent1_subs = manager.list_sub_agents(Some(parent1), false).await?;
    assert_eq!(parent1_subs.len(), 1, "Should list 1 sub-agent for parent1");
    assert_eq!(parent1_subs[0].sub_agent_id, sub1.sub_agent_id);

    let parent2_subs = manager.list_sub_agents(Some(parent2), false).await?;
    assert_eq!(parent2_subs.len(), 1, "Should list 1 sub-agent for parent2");
    assert_eq!(parent2_subs[0].sub_agent_id, sub2.sub_agent_id);

    // Cleanup
    sqlx::query("DELETE FROM sub_agents WHERE sub_agent_id IN ($1, $2)")
        .bind(sub1.sub_agent_id)
        .bind(sub2.sub_agent_id)
        .execute(&pool)
        .await?;
    
    sqlx::query("DELETE FROM identities WHERE identity_id IN ($1, $2, $3, $4, $5)")
        .bind(parent1)
        .bind(parent2)
        .bind(creator)
        .bind(sub1.sub_agent_id)
        .bind(sub2.sub_agent_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_pause_and_resume_sub_agent() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let manager = SubAgentManager::new(pool.clone(), None);
    let policy_engine = PolicyEngine::new(pool.clone());

    // Create test identities
    let parent_id = create_test_identity(&pool, "test_parent3").await?;
    let created_by = create_test_identity(&pool, "test_creator3").await?;

    // Create sub-agent
    let request = CreateSubAgentRequest {
        name: "PausableAgent".to_string(),
        role: "pausable".to_string(),
        parent_id: Some(parent_id),
        directives: None,
        model_provider: None,
        ephemeral: false,
        capabilities: vec![],
        runtime: "node".to_string(),
    };

    let sub_agent = manager
        .create_sub_agent(parent_id, created_by, request, &policy_engine, None)
        .await?;

    // Pause the sub-agent
    manager.pause_sub_agent(sub_agent.sub_agent_id).await?;

    // Verify pause flag is set
    let paused = manager.get_sub_agent(sub_agent.sub_agent_id).await?.unwrap();
    let directives = paused.directives.unwrap();
    assert_eq!(
        directives.get("_paused"),
        Some(&json!(true)),
        "Sub-agent should be paused"
    );

    // Resume the sub-agent
    manager.resume_sub_agent(sub_agent.sub_agent_id).await?;

    // Verify pause flag is removed
    let resumed = manager.get_sub_agent(sub_agent.sub_agent_id).await?.unwrap();
    if let Some(directives) = resumed.directives {
        assert!(
            !directives.as_object().unwrap().contains_key("_paused"),
            "Pause flag should be removed"
        );
    }

    // Cleanup
    sqlx::query("DELETE FROM sub_agents WHERE sub_agent_id = $1")
        .bind(sub_agent.sub_agent_id)
        .execute(&pool)
        .await?;
    
    sqlx::query("DELETE FROM identities WHERE identity_id IN ($1, $2, $3)")
        .bind(parent_id)
        .bind(created_by)
        .bind(sub_agent.sub_agent_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_soft_delete_sub_agent() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let manager = SubAgentManager::new(pool.clone(), None);
    let policy_engine = PolicyEngine::new(pool.clone());

    // Create test identities
    let parent_id = create_test_identity(&pool, "test_parent4").await?;
    let created_by = create_test_identity(&pool, "test_creator4").await?;

    // Create sub-agent
    let request = CreateSubAgentRequest {
        name: "DeletableAgent".to_string(),
        role: "deletable".to_string(),
        parent_id: Some(parent_id),
        directives: None,
        model_provider: None,
        ephemeral: false,
        capabilities: vec![],
        runtime: "node".to_string(),
    };

    let sub_agent = manager
        .create_sub_agent(parent_id, created_by, request, &policy_engine, None)
        .await?;

    // Soft-delete the sub-agent
    let deleted = manager.delete_sub_agent(sub_agent.sub_agent_id).await?;
    assert!(deleted, "Sub-agent should be deleted");

    // Verify terminated_at is set
    let terminated = manager.get_sub_agent(sub_agent.sub_agent_id).await?.unwrap();
    assert!(
        terminated.terminated_at.is_some(),
        "Sub-agent should have terminated_at timestamp"
    );

    // Verify it doesn't appear in default listings (exclude_terminated=true)
    let active_list = manager.list_sub_agents(Some(parent_id), false).await?;
    assert!(
        !active_list.iter().any(|s| s.sub_agent_id == sub_agent.sub_agent_id),
        "Terminated sub-agent should not appear in active list"
    );

    // Verify it appears when including terminated
    let all_list = manager.list_sub_agents(Some(parent_id), true).await?;
    assert!(
        all_list.iter().any(|s| s.sub_agent_id == sub_agent.sub_agent_id),
        "Terminated sub-agent should appear when including terminated"
    );

    // Cleanup
    sqlx::query("DELETE FROM sub_agents WHERE sub_agent_id = $1")
        .bind(sub_agent.sub_agent_id)
        .execute(&pool)
        .await?;
    
    sqlx::query("DELETE FROM identities WHERE identity_id IN ($1, $2, $3)")
        .bind(parent_id)
        .bind(created_by)
        .bind(sub_agent.sub_agent_id)
        .execute(&pool)
        .await?;

    Ok(())
}
