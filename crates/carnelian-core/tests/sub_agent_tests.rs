//! Integration tests for sub-agent lifecycle management.
//!
//! These tests validate CRUD operations, capability validation,
//! pause/resume lifecycle, event emission, and soft delete behavior.
//!
//! Run with: `cargo test --test sub_agent_tests -- --ignored`
//! (requires a running `PostgreSQL` instance with the Carnelian schema applied)

use std::sync::Arc;

use carnelian_common::types::EventType;
use carnelian_core::events::EventStream;
use carnelian_core::ledger::Ledger;
use carnelian_core::policy::PolicyEngine;
use carnelian_core::sub_agent::{CreateSubAgentRequest, SubAgentManager, UpdateSubAgentRequest};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

/// Helper to connect to the test database.
async fn test_pool() -> PgPool {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("Failed to connect to database")
}

/// Helper to create a core identity for testing, returning its `identity_id`.
async fn ensure_core_identity(pool: &PgPool) -> Uuid {
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT identity_id FROM identities WHERE identity_type = 'core' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .unwrap();

    if let Some(id) = existing {
        return id;
    }

    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO identities (name, identity_type) VALUES ('TestCore', 'core') RETURNING identity_id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Helper to grant `sub_agent.create` capability to a parent identity.
async fn grant_create_capability(pool: &PgPool, parent_id: Uuid) {
    let _ = sqlx::query(
        "INSERT INTO capability_grants (subject_type, subject_id, capability_key) \
         VALUES ('identity', $1, 'sub_agent.create') \
         ON CONFLICT DO NOTHING",
    )
    .bind(parent_id.to_string())
    .execute(pool)
    .await;
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_create_sub_agent() {
    let pool = test_pool().await;
    let event_stream = Arc::new(EventStream::new(100, 10));
    let manager = SubAgentManager::new(pool.clone(), Some(event_stream.clone()));
    let policy_engine = PolicyEngine::new(pool.clone());
    let ledger = Ledger::new(pool.clone());

    let parent_id = ensure_core_identity(&pool).await;
    grant_create_capability(&pool, parent_id).await;

    let request = CreateSubAgentRequest {
        name: format!("TestAgent-{}", Uuid::new_v4()),
        role: "code_review".to_string(),
        directives: Some(json!(["Review code for security"])),
        model_provider: None,
        ephemeral: false,
        capabilities: vec![],
        parent_id: None,
        runtime: "node".to_string(),
    };

    let sub_agent = manager
        .create_sub_agent(parent_id, parent_id, request.clone(), &policy_engine, Some(&ledger))
        .await
        .expect("Should create sub-agent");

    assert_eq!(sub_agent.name, request.name);
    assert_eq!(sub_agent.role, "code_review");
    assert_eq!(sub_agent.parent_id, parent_id);
    assert!(sub_agent.terminated_at.is_none());

    // Cleanup
    let _ = sqlx::query("DELETE FROM sub_agents WHERE sub_agent_id = $1")
        .bind(sub_agent.sub_agent_id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM identities WHERE identity_id = $1")
        .bind(sub_agent.sub_agent_id)
        .execute(&pool)
        .await;
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_create_sub_agent_without_capability_fails() {
    let pool = test_pool().await;
    let event_stream = Arc::new(EventStream::new(100, 10));
    let manager = SubAgentManager::new(pool.clone(), Some(event_stream.clone()));
    let policy_engine = PolicyEngine::new(pool.clone());

    // Use a random identity that has no capabilities
    let fake_parent = Uuid::new_v4();

    let request = CreateSubAgentRequest {
        name: "ShouldFail".to_string(),
        role: "test".to_string(),
        directives: None,
        model_provider: None,
        ephemeral: false,
        capabilities: vec![],
        parent_id: None,
        runtime: "node".to_string(),
    };

    let result = manager
        .create_sub_agent(fake_parent, fake_parent, request, &policy_engine, None)
        .await;

    assert!(result.is_err(), "Should fail without sub_agent.create capability");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("sub_agent.create"),
        "Error should mention missing capability: {err_msg}"
    );
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_get_sub_agent() {
    let pool = test_pool().await;
    let event_stream = Arc::new(EventStream::new(100, 10));
    let manager = SubAgentManager::new(pool.clone(), Some(event_stream.clone()));
    let policy_engine = PolicyEngine::new(pool.clone());
    let ledger = Ledger::new(pool.clone());

    let parent_id = ensure_core_identity(&pool).await;
    grant_create_capability(&pool, parent_id).await;

    let request = CreateSubAgentRequest {
        name: format!("GetTest-{}", Uuid::new_v4()),
        role: "test".to_string(),
        directives: None,
        model_provider: None,
        ephemeral: true,
        capabilities: vec![],
        parent_id: None,
        runtime: "node".to_string(),
    };

    let created = manager
        .create_sub_agent(parent_id, parent_id, request, &policy_engine, Some(&ledger))
        .await
        .unwrap();

    let fetched = manager
        .get_sub_agent(created.sub_agent_id)
        .await
        .unwrap()
        .expect("Should find sub-agent");

    assert_eq!(fetched.sub_agent_id, created.sub_agent_id);
    assert_eq!(fetched.name, created.name);
    assert!(fetched.ephemeral);

    // Not found case
    let missing = manager.get_sub_agent(Uuid::new_v4()).await.unwrap();
    assert!(missing.is_none());

    // Cleanup
    let _ = sqlx::query("DELETE FROM sub_agents WHERE sub_agent_id = $1")
        .bind(created.sub_agent_id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM identities WHERE identity_id = $1")
        .bind(created.sub_agent_id)
        .execute(&pool)
        .await;
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_list_sub_agents_with_filters() {
    let pool = test_pool().await;
    let event_stream = Arc::new(EventStream::new(100, 10));
    let manager = SubAgentManager::new(pool.clone(), Some(event_stream.clone()));
    let policy_engine = PolicyEngine::new(pool.clone());
    let ledger = Ledger::new(pool.clone());

    let parent_id = ensure_core_identity(&pool).await;
    grant_create_capability(&pool, parent_id).await;

    // Create two sub-agents
    let a1 = manager
        .create_sub_agent(
            parent_id,
            parent_id,
            CreateSubAgentRequest {
                name: format!("ListA-{}", Uuid::new_v4()),
                role: "a".to_string(),
                directives: None,
                model_provider: None,
                ephemeral: false,
                capabilities: vec![],
                parent_id: None,
                runtime: "node".to_string(),
            },
            &policy_engine,
            Some(&ledger),
        )
        .await
        .unwrap();

    let a2 = manager
        .create_sub_agent(
            parent_id,
            parent_id,
            CreateSubAgentRequest {
                name: format!("ListB-{}", Uuid::new_v4()),
                role: "b".to_string(),
                directives: None,
                model_provider: None,
                ephemeral: false,
                capabilities: vec![],
                parent_id: None,
                runtime: "node".to_string(),
            },
            &policy_engine,
            Some(&ledger),
        )
        .await
        .unwrap();

    // Terminate one
    manager.delete_sub_agent(a2.sub_agent_id).await.unwrap();

    // List without terminated
    let active = manager.list_sub_agents(Some(parent_id), false).await.unwrap();
    assert!(
        active.iter().any(|a| a.sub_agent_id == a1.sub_agent_id),
        "Active list should contain a1"
    );
    assert!(
        !active.iter().any(|a| a.sub_agent_id == a2.sub_agent_id),
        "Active list should not contain terminated a2"
    );

    // List with terminated
    let all = manager.list_sub_agents(Some(parent_id), true).await.unwrap();
    assert!(
        all.iter().any(|a| a.sub_agent_id == a2.sub_agent_id),
        "Full list should contain terminated a2"
    );

    // Cleanup
    for id in [a1.sub_agent_id, a2.sub_agent_id] {
        let _ = sqlx::query("DELETE FROM sub_agents WHERE sub_agent_id = $1")
            .bind(id)
            .execute(&pool)
            .await;
        let _ = sqlx::query("DELETE FROM identities WHERE identity_id = $1")
            .bind(id)
            .execute(&pool)
            .await;
    }
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_update_sub_agent() {
    let pool = test_pool().await;
    let event_stream = Arc::new(EventStream::new(100, 10));
    let manager = SubAgentManager::new(pool.clone(), Some(event_stream.clone()));
    let policy_engine = PolicyEngine::new(pool.clone());
    let ledger = Ledger::new(pool.clone());

    let parent_id = ensure_core_identity(&pool).await;
    grant_create_capability(&pool, parent_id).await;

    let created = manager
        .create_sub_agent(
            parent_id,
            parent_id,
            CreateSubAgentRequest {
                name: format!("UpdateTest-{}", Uuid::new_v4()),
                role: "original".to_string(),
                directives: None,
                model_provider: None,
                ephemeral: false,
                capabilities: vec![],
                parent_id: None,
                runtime: "node".to_string(),
            },
            &policy_engine,
            Some(&ledger),
        )
        .await
        .unwrap();

    let updated = manager
        .update_sub_agent(
            created.sub_agent_id,
            UpdateSubAgentRequest {
                name: Some("RenamedAgent".to_string()),
                role: Some("updated_role".to_string()),
                directives: Some(json!(["new directive"])),
                model_provider: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.name, "RenamedAgent");
    assert_eq!(updated.role, "updated_role");

    // Cleanup
    let _ = sqlx::query("DELETE FROM sub_agents WHERE sub_agent_id = $1")
        .bind(created.sub_agent_id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM identities WHERE identity_id = $1")
        .bind(created.sub_agent_id)
        .execute(&pool)
        .await;
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_pause_and_resume_sub_agent() {
    let pool = test_pool().await;
    let event_stream = Arc::new(EventStream::new(100, 10));
    let manager = SubAgentManager::new(pool.clone(), Some(event_stream.clone()));
    let policy_engine = PolicyEngine::new(pool.clone());
    let ledger = Ledger::new(pool.clone());

    let parent_id = ensure_core_identity(&pool).await;
    grant_create_capability(&pool, parent_id).await;

    let created = manager
        .create_sub_agent(
            parent_id,
            parent_id,
            CreateSubAgentRequest {
                name: format!("PauseTest-{}", Uuid::new_v4()),
                role: "test".to_string(),
                directives: Some(json!({"key": "value"})),
                model_provider: None,
                ephemeral: false,
                capabilities: vec![],
                parent_id: None,
                runtime: "node".to_string(),
            },
            &policy_engine,
            Some(&ledger),
        )
        .await
        .unwrap();

    // Pause
    manager.pause_sub_agent(created.sub_agent_id).await.unwrap();
    let paused = manager.get_sub_agent(created.sub_agent_id).await.unwrap().unwrap();
    let directives = paused.directives.unwrap();
    assert_eq!(directives["_paused"], json!(true), "Should have _paused flag");

    // Resume
    manager.resume_sub_agent(created.sub_agent_id).await.unwrap();
    let resumed = manager.get_sub_agent(created.sub_agent_id).await.unwrap().unwrap();
    let directives = resumed.directives.unwrap();
    assert!(directives.get("_paused").is_none(), "Should not have _paused flag after resume");

    // Cleanup
    let _ = sqlx::query("DELETE FROM sub_agents WHERE sub_agent_id = $1")
        .bind(created.sub_agent_id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM identities WHERE identity_id = $1")
        .bind(created.sub_agent_id)
        .execute(&pool)
        .await;
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_soft_delete_sub_agent() {
    let pool = test_pool().await;
    let event_stream = Arc::new(EventStream::new(100, 10));
    let manager = SubAgentManager::new(pool.clone(), Some(event_stream.clone()));
    let policy_engine = PolicyEngine::new(pool.clone());
    let ledger = Ledger::new(pool.clone());

    let parent_id = ensure_core_identity(&pool).await;
    grant_create_capability(&pool, parent_id).await;

    let created = manager
        .create_sub_agent(
            parent_id,
            parent_id,
            CreateSubAgentRequest {
                name: format!("DeleteTest-{}", Uuid::new_v4()),
                role: "test".to_string(),
                directives: None,
                model_provider: None,
                ephemeral: false,
                capabilities: vec![],
                parent_id: None,
                runtime: "node".to_string(),
            },
            &policy_engine,
            Some(&ledger),
        )
        .await
        .unwrap();

    // First delete should succeed
    let deleted = manager.delete_sub_agent(created.sub_agent_id).await.unwrap();
    assert!(deleted, "First delete should return true");

    // Verify terminated_at is set
    let terminated = manager.get_sub_agent(created.sub_agent_id).await.unwrap().unwrap();
    assert!(terminated.terminated_at.is_some(), "terminated_at should be set");

    // Second delete should return false (already terminated)
    let deleted_again = manager.delete_sub_agent(created.sub_agent_id).await.unwrap();
    assert!(!deleted_again, "Second delete should return false");

    // Delete non-existent
    let missing = manager.delete_sub_agent(Uuid::new_v4()).await.unwrap();
    assert!(!missing, "Deleting non-existent should return false");

    // Cleanup
    let _ = sqlx::query("DELETE FROM sub_agents WHERE sub_agent_id = $1")
        .bind(created.sub_agent_id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM identities WHERE identity_id = $1")
        .bind(created.sub_agent_id)
        .execute(&pool)
        .await;
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_event_emission() {
    let pool = test_pool().await;
    let event_stream = Arc::new(EventStream::new(1000, 100));
    let manager = SubAgentManager::new(pool.clone(), Some(event_stream.clone()));
    let policy_engine = PolicyEngine::new(pool.clone());
    let ledger = Ledger::new(pool.clone());

    let parent_id = ensure_core_identity(&pool).await;
    grant_create_capability(&pool, parent_id).await;

    // Subscribe before creating
    let mut rx = event_stream.subscribe();

    let created = manager
        .create_sub_agent(
            parent_id,
            parent_id,
            CreateSubAgentRequest {
                name: format!("EventTest-{}", Uuid::new_v4()),
                role: "test".to_string(),
                directives: None,
                model_provider: None,
                ephemeral: false,
                capabilities: vec![],
                parent_id: None,
                runtime: "node".to_string(),
            },
            &policy_engine,
            Some(&ledger),
        )
        .await
        .unwrap();

    // Collect events
    let mut events = Vec::new();
    while let Ok(env) = rx.try_recv() {
        events.push(env);
    }

    let created_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.event_type, EventType::SubAgentCreated))
        .collect();
    assert!(
        !created_events.is_empty(),
        "Should have emitted SubAgentCreated event"
    );
    assert_eq!(
        created_events[0].payload["sub_agent_id"],
        json!(created.sub_agent_id)
    );

    // Cleanup
    let _ = sqlx::query("DELETE FROM sub_agents WHERE sub_agent_id = $1")
        .bind(created.sub_agent_id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM identities WHERE identity_id = $1")
        .bind(created.sub_agent_id)
        .execute(&pool)
        .await;
}
