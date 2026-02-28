#![allow(dead_code)]
//! Shared test helpers for Phase 3 integration tests.
//!
//! Provides `PostgreSQL` container setup, database initialization, and
//! helper functions for creating test data across all Phase 3 modules.

use std::path::Path;
use std::time::Duration;

use serde_json::json;
use sqlx::PgPool;
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};
use uuid::Uuid;

// =============================================================================
// POSTGRESQL CONTAINER
// =============================================================================

/// Create a `PostgreSQL` container with pgvector for testing.
pub async fn create_postgres_container() -> testcontainers::ContainerAsync<GenericImage> {
    let image = GenericImage::new("pgvector/pgvector", "pg16").with_wait_for(
        testcontainers::core::WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ),
    );

    image
        .with_env_var("POSTGRES_USER", "test")
        .with_env_var("POSTGRES_PASSWORD", "test")
        .with_env_var("POSTGRES_DB", "carnelian_test")
        .start()
        .await
        .expect("Failed to start PostgreSQL container")
}

/// Get the database URL from a running container.
pub async fn get_database_url(container: &testcontainers::ContainerAsync<GenericImage>) -> String {
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");
    format!("postgresql://test:test@127.0.0.1:{port}/carnelian_test")
}

/// Set up a test database with migrations and return the pool.
pub async fn setup_test_db(database_url: &str) -> PgPool {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(database_url)
        .await
        .expect("Failed to connect to test database");

    carnelian_core::db::run_migrations(&pool, None)
        .await
        .expect("Failed to run migrations");

    pool
}

// =============================================================================
// IDENTITY HELPERS
// =============================================================================

/// Insert a test identity and return its `identity_id`.
pub async fn insert_test_identity(pool: &PgPool, name: &str) -> Uuid {
    let identity_id = Uuid::new_v4();
    sqlx::query(
        r"INSERT INTO identities (identity_id, name, pronouns, identity_type, directives)
          VALUES ($1, $2, 'they/them', 'core', '[]'::jsonb)",
    )
    .bind(identity_id)
    .bind(name)
    .execute(pool)
    .await
    .expect("Failed to insert test identity");

    identity_id
}

/// Insert a test identity with a soul file path and return its `identity_id`.
pub async fn insert_test_identity_with_soul(pool: &PgPool, name: &str, soul_path: &str) -> Uuid {
    let identity_id = Uuid::new_v4();
    sqlx::query(
        r"INSERT INTO identities (identity_id, name, pronouns, identity_type, directives, soul_file_path)
          VALUES ($1, $2, 'they/them', 'core', '[]'::jsonb, $3)",
    )
    .bind(identity_id)
    .bind(name)
    .bind(soul_path)
    .execute(pool)
    .await
    .expect("Failed to insert test identity with soul");

    identity_id
}

// =============================================================================
// SESSION HELPERS
// =============================================================================

/// Create a test session and return its `session_id`.
pub async fn create_test_session(pool: &PgPool, agent_id: Uuid, channel: &str) -> Uuid {
    let session_id = Uuid::new_v4();
    let session_key = format!("agent:{agent_id}:{channel}");
    let counters = json!({"total": 0, "user": 0, "assistant": 0, "tool": 0});

    sqlx::query(
        r"INSERT INTO sessions (session_id, session_key, agent_id, channel, token_counters)
          VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(session_id)
    .bind(&session_key)
    .bind(agent_id)
    .bind(channel)
    .bind(&counters)
    .execute(pool)
    .await
    .expect("Failed to create test session");

    session_id
}

/// Insert a test message into a session.
pub async fn insert_test_message(
    pool: &PgPool,
    session_id: Uuid,
    role: &str,
    content: &str,
) -> i64 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let token_estimate = (content.len() / 4) as i32;
    sqlx::query_scalar::<_, i64>(
        r"INSERT INTO session_messages (session_id, role, content, token_estimate, metadata, tool_metadata)
          VALUES ($1, $2, $3, $4, '{}'::jsonb, '{}'::jsonb)
          RETURNING message_id",
    )
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(token_estimate)
    .fetch_one(pool)
    .await
    .expect("Failed to insert test message")
}

/// Get the token counters for a session.
pub async fn get_session_token_counters(pool: &PgPool, session_id: Uuid) -> serde_json::Value {
    sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT token_counters FROM sessions WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .expect("Failed to get session token counters")
}

// =============================================================================
// MEMORY HELPERS
// =============================================================================

/// Create a test memory and return its `memory_id`.
pub async fn create_test_memory(
    pool: &PgPool,
    identity_id: Uuid,
    content: &str,
    importance: f32,
) -> Uuid {
    let memory_id = Uuid::new_v4();
    sqlx::query(
        r"INSERT INTO memories (memory_id, identity_id, content, source, importance)
          VALUES ($1, $2, $3, 'conversation', $4)",
    )
    .bind(memory_id)
    .bind(identity_id)
    .bind(content)
    .bind(importance)
    .execute(pool)
    .await
    .expect("Failed to create test memory");

    memory_id
}

/// Generate a mock 1536-dimension embedding vector for testing.
pub fn create_mock_embedding() -> Vec<f32> {
    let mut embedding = vec![0.0f32; 1536];
    // Create a simple pattern for deterministic testing
    for (i, val) in embedding.iter_mut().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let i_f32 = i as f32;
        *val = (i_f32 * 0.001).sin();
    }
    // Normalize to unit vector for cosine similarity
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for val in &mut embedding {
            *val /= norm;
        }
    }
    embedding
}

/// Get the access count for a memory.
pub async fn get_memory_access_count(pool: &PgPool, memory_id: Uuid) -> i32 {
    sqlx::query_scalar::<_, i32>("SELECT access_count FROM memories WHERE memory_id = $1")
        .bind(memory_id)
        .fetch_one(pool)
        .await
        .expect("Failed to get memory access count")
}

// =============================================================================
// MODEL PROVIDER HELPERS
// =============================================================================

/// Insert a test model provider and return its `provider_id`.
pub async fn insert_test_provider(pool: &PgPool, name: &str, provider_type: &str) -> Uuid {
    let provider_id = Uuid::new_v4();
    let budget = json!({"daily_usd": 10.0, "monthly_usd": 100.0});
    sqlx::query(
        "INSERT INTO model_providers (provider_id, name, provider_type, enabled, budget_limits) \
         VALUES ($1, $2, $3, true, $4)",
    )
    .bind(provider_id)
    .bind(name)
    .bind(provider_type)
    .bind(&budget)
    .execute(pool)
    .await
    .expect("Failed to insert test provider");

    provider_id
}

/// Insert a test usage record.
pub async fn insert_test_usage(pool: &PgPool, provider_id: Uuid, cost: f64) {
    sqlx::query(
        r"INSERT INTO usage_costs (usage_id, provider_id, tokens_in, tokens_out, cost_estimate)
          VALUES ($1, $2, 100, 200, $3)",
    )
    .bind(Uuid::new_v4())
    .bind(provider_id)
    .bind(cost)
    .execute(pool)
    .await
    .expect("Failed to insert test usage");
}

/// Get total usage cost for a provider.
pub async fn get_total_usage_cost(pool: &PgPool, provider_id: Uuid) -> f64 {
    sqlx::query_scalar::<_, Option<f64>>(
        "SELECT SUM(cost_estimate::float8) FROM usage_costs WHERE provider_id = $1",
    )
    .bind(provider_id)
    .fetch_one(pool)
    .await
    .expect("Failed to get total usage cost")
    .unwrap_or(0.0)
}

// =============================================================================
// SOUL FILE HELPERS
// =============================================================================

/// Write a test soul file to the given path.
pub fn create_test_soul_file(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

/// Query and deserialize directives for an identity.
pub async fn get_identity_directives(pool: &PgPool, identity_id: Uuid) -> Vec<serde_json::Value> {
    let directives_json: serde_json::Value =
        sqlx::query_scalar("SELECT directives FROM identities WHERE identity_id = $1")
            .bind(identity_id)
            .fetch_one(pool)
            .await
            .expect("Failed to get identity directives");

    match directives_json {
        serde_json::Value::Array(arr) => arr,
        _ => vec![],
    }
}

/// Get the soul file hash for an identity.
pub async fn get_soul_file_hash(pool: &PgPool, identity_id: Uuid) -> Option<String> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT soul_file_hash FROM identities WHERE identity_id = $1",
    )
    .bind(identity_id)
    .fetch_one(pool)
    .await
    .expect("Failed to get soul file hash")
}

/// Get the compaction count for a session.
pub async fn get_compaction_count(pool: &PgPool, session_id: Uuid) -> i32 {
    sqlx::query_scalar::<_, i32>("SELECT compaction_count FROM sessions WHERE session_id = $1")
        .bind(session_id)
        .fetch_one(pool)
        .await
        .expect("Failed to get compaction count")
}
