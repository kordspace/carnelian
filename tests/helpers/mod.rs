//! Test helpers and utilities for CARNELIAN test suite
//!
//! This module provides common test utilities including:
//! - Test database pool creation
//! - Mock data generators
//! - Test server setup
//! - Cleanup utilities

use carnelian_common::{Error, Result};
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::sync::Once;
use uuid::Uuid;

static INIT: Once = Once::new();

/// Initialize test environment (run once)
pub fn init_test_env() {
    INIT.call_once(|| {
        env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Debug)
            .try_init()
            .ok();
    });
}

/// Create a test database pool
pub async fn create_test_pool() -> PgPool {
    init_test_env();
    
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/carnelian_test".to_string());
    
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to create test database pool")
}

/// Run database migrations for tests
pub async fn run_test_migrations(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| Error::Database(format!("Migration failed: {}", e)))?;
    Ok(())
}

/// Clean up test database (truncate all tables)
pub async fn cleanup_test_db(pool: &PgPool) -> Result<()> {
    sqlx::query("TRUNCATE TABLE memories, memory_embeddings, skills, skill_executions, xp_events, xp_ledger, elixirs, elixir_versions, elixir_usage, elixir_drafts, workflows, workflow_executions, tasks CASCADE")
        .execute(pool)
        .await
        .map_err(|e| Error::Database(format!("Cleanup failed: {}", e)))?;
    Ok(())
}

/// Generate test UUID
pub fn test_uuid() -> Uuid {
    Uuid::new_v4()
}

/// Generate test identity ID
pub fn test_identity_id() -> Uuid {
    Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
}

/// Generate test correlation ID
pub fn test_correlation_id() -> Uuid {
    Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()
}

/// Generate test embedding vector (1536 dimensions for OpenAI)
pub fn test_embedding() -> Vec<f32> {
    vec![0.1; 1536]
}

/// Generate test embedding with specific pattern
pub fn test_embedding_with_pattern(pattern: f32) -> Vec<f32> {
    vec![pattern; 1536]
}

/// Create test memory request
pub fn test_memory_request() -> serde_json::Value {
    json!({
        "content": "Test memory content",
        "metadata": {
            "source": "test",
            "type": "unit_test"
        },
        "tags": ["test", "unit"]
    })
}

/// Create test skill input
pub fn test_skill_input() -> serde_json::Value {
    json!({
        "action": "execute",
        "params": {
            "input": "test input"
        }
    })
}

/// Create test workflow definition
pub fn test_workflow_definition() -> serde_json::Value {
    json!({
        "name": "Test Workflow",
        "description": "Test workflow for unit tests",
        "steps": [
            {
                "skill": "test-skill",
                "input": {"key": "value"}
            }
        ]
    })
}

/// Wait for async operation with timeout
pub async fn wait_for<F, T>(mut check: F, timeout_secs: u64) -> Result<T>
where
    F: FnMut() -> Option<T>,
{
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_secs);
    
    loop {
        if let Some(result) = check() {
            return Ok(result);
        }
        
        if start.elapsed() > timeout {
            return Err(Error::Worker("Timeout waiting for condition".to_string()));
        }
        
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_uuid_generation() {
        let uuid1 = test_uuid();
        let uuid2 = test_uuid();
        assert_ne!(uuid1, uuid2);
    }
    
    #[test]
    fn test_identity_id_consistent() {
        let id1 = test_identity_id();
        let id2 = test_identity_id();
        assert_eq!(id1, id2);
    }
    
    #[test]
    fn test_embedding_generation() {
        let embedding = test_embedding();
        assert_eq!(embedding.len(), 1536);
        assert_eq!(embedding[0], 0.1);
    }
}
