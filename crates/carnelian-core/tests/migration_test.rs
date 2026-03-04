#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_precision_loss)]

//! Migration Integration Tests for Carnelian
//!
//! Comprehensive schema verification for Phase 1 delta tables, schema fixes,
//! seed data, and constraint enforcement.
//!
//! # Running Tests
//!
//! ```bash
//! cargo test --test migration_test -- --ignored
//! ```

use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};

/// Create a PostgreSQL container for testing
async fn create_postgres_container() -> testcontainers::ContainerAsync<GenericImage> {
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

/// Get the database URL from a running container
async fn get_database_url(container: &testcontainers::ContainerAsync<GenericImage>) -> String {
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");
    format!("postgresql://test:test@127.0.0.1:{}/carnelian_test", port)
}

/// Verify a column exists in a table with the expected data type
async fn verify_column(pool: &sqlx::PgPool, table: &str, column: &str, expected_type: &str) {
    let col_type: Option<String> = sqlx::query_scalar(
        "SELECT data_type::text FROM information_schema.columns \
         WHERE table_schema = 'public' AND table_name = $1 AND column_name = $2",
    )
    .bind(table)
    .bind(column)
    .fetch_optional(pool)
    .await
    .expect("Should query column info");

    assert!(
        col_type.is_some(),
        "Column '{}.{}' should exist",
        table,
        column
    );
    let actual = col_type.unwrap();
    assert!(
        actual.contains(expected_type),
        "Column '{}.{}' should be '{}', got '{}'",
        table,
        column,
        expected_type,
        actual
    );
}

/// Verify a table exists in the public schema
async fn verify_table_exists(pool: &sqlx::PgPool, table: &str) {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_name = $1)",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .expect("Should query table existence");

    assert!(exists, "Table '{}' should exist", table);
}

/// Verify an index exists on a table
async fn verify_index_exists(pool: &sqlx::PgPool, table: &str, index: &str) {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_indexes \
         WHERE schemaname = 'public' AND tablename = $1 AND indexname = $2)",
    )
    .bind(table)
    .bind(index)
    .fetch_one(pool)
    .await
    .expect("Should query index existence");

    assert!(
        exists,
        "Index '{}' on table '{}' should exist",
        index, table
    );
}

/// Verify LZ4 compression is set on a column
async fn verify_column_compression(pool: &sqlx::PgPool, table: &str, column: &str) -> bool {
    let result: Option<String> = sqlx::query_scalar(
        "SELECT attcompression::text FROM pg_attribute \
         WHERE attrelid = $1::regclass AND attname = $2",
    )
    .bind(table)
    .bind(column)
    .fetch_optional(pool)
    .await
    .expect("Failed to query compression");

    result.as_deref() == Some("l")
}

/// Connect to database and run all migrations
async fn setup_pool(db_url: &str) -> sqlx::PgPool {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(db_url)
        .await
        .expect("Failed to connect to database");

    carnelian_core::db::run_migrations(&pool, None)
        .await
        .expect("Migrations should succeed");

    pool
}

// =============================================================================
// PHASE 1 DELTA: Sessions
// =============================================================================

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test migration_test -- --ignored"]
async fn test_phase1_delta_sessions_tables() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_pool(&db_url).await;

    // Verify sessions table exists with expected columns
    verify_table_exists(&pool, "sessions").await;
    verify_column(&pool, "sessions", "session_id", "uuid").await;
    verify_column(&pool, "sessions", "session_key", "text").await;
    verify_column(&pool, "sessions", "agent_id", "uuid").await;
    verify_column(&pool, "sessions", "channel", "text").await;
    verify_column(&pool, "sessions", "transcript_path", "text").await;
    verify_column(&pool, "sessions", "token_counters", "jsonb").await;
    verify_column(&pool, "sessions", "compaction_count", "integer").await;
    verify_column(&pool, "sessions", "expires_at", "timestamp").await;

    // Verify indexes
    verify_index_exists(&pool, "sessions", "idx_sessions_agent").await;
    verify_index_exists(&pool, "sessions", "idx_sessions_channel").await;
    verify_index_exists(&pool, "sessions", "idx_sessions_key").await;
    verify_index_exists(&pool, "sessions", "idx_sessions_activity").await;

    // Verify session_messages table
    verify_table_exists(&pool, "session_messages").await;
    verify_column(&pool, "session_messages", "message_id", "bigint").await;
    verify_column(&pool, "session_messages", "session_id", "uuid").await;
    verify_column(&pool, "session_messages", "role", "text").await;
    verify_column(&pool, "session_messages", "content", "text").await;
    verify_column(&pool, "session_messages", "tool_metadata", "jsonb").await;
    verify_column(&pool, "session_messages", "correlation_id", "uuid").await;

    // Verify foreign key cascade: insert session, then delete, messages should cascade
    let lian_id: uuid::Uuid =
        sqlx::query_scalar("SELECT identity_id FROM identities WHERE name = 'Lian' LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("Lian should exist");

    let session_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO sessions (session_key, agent_id) VALUES ('test_cascade_key', $1) RETURNING session_id",
    )
    .bind(lian_id)
    .fetch_one(&pool)
    .await
    .expect("Should insert test session");

    sqlx::query(
        "INSERT INTO session_messages (session_id, role, content) VALUES ($1, 'user', 'test message')",
    )
    .bind(session_id)
    .execute(&pool)
    .await
    .expect("Should insert test message");

    // Delete session - messages should cascade
    sqlx::query("DELETE FROM sessions WHERE session_id = $1")
        .bind(session_id)
        .execute(&pool)
        .await
        .expect("Should delete session");

    let orphan_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM session_messages WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(&pool)
            .await
            .expect("Should query orphan messages");

    assert_eq!(
        orphan_count, 0,
        "Messages should be cascade-deleted with session"
    );

    println!("✓ Phase 1 delta sessions tables verified");
}

// =============================================================================
// PHASE 1 DELTA: Skill Versions
// =============================================================================

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test migration_test -- --ignored"]
async fn test_phase1_delta_skill_versions() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_pool(&db_url).await;

    verify_table_exists(&pool, "skill_versions").await;
    verify_column(&pool, "skill_versions", "version_id", "uuid").await;
    verify_column(&pool, "skill_versions", "skill_id", "uuid").await;
    verify_column(&pool, "skill_versions", "version", "text").await;
    verify_column(&pool, "skill_versions", "manifest", "jsonb").await;
    verify_column(&pool, "skill_versions", "checksum", "text").await;
    verify_column(&pool, "skill_versions", "signature", "text").await;

    // Verify UNIQUE constraint on (skill_id, version)
    // Insert a test skill first
    let skill_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO skills (name, runtime) VALUES ('test_versioned_skill', 'node') RETURNING skill_id",
    )
    .fetch_one(&pool)
    .await
    .expect("Should insert test skill");

    sqlx::query(
        "INSERT INTO skill_versions (skill_id, version, manifest, checksum) VALUES ($1, '1.0.0', '{}'::jsonb, 'abc123')",
    )
    .bind(skill_id)
    .execute(&pool)
    .await
    .expect("Should insert first version");

    // Duplicate (skill_id, version) should fail
    let duplicate = sqlx::query(
        "INSERT INTO skill_versions (skill_id, version, manifest, checksum) VALUES ($1, '1.0.0', '{}'::jsonb, 'def456')",
    )
    .bind(skill_id)
    .execute(&pool)
    .await;

    assert!(
        duplicate.is_err(),
        "Duplicate (skill_id, version) should be rejected by UNIQUE constraint"
    );

    // Verify foreign key cascade
    sqlx::query("DELETE FROM skills WHERE skill_id = $1")
        .bind(skill_id)
        .execute(&pool)
        .await
        .expect("Should delete skill");

    let orphan_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM skill_versions WHERE skill_id = $1")
            .bind(skill_id)
            .fetch_one(&pool)
            .await
            .expect("Should query orphan versions");

    assert_eq!(orphan_count, 0, "Skill versions should be cascade-deleted");

    println!("✓ Phase 1 delta skill versions verified");
}

// =============================================================================
// PHASE 1 DELTA: Workflows
// =============================================================================

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test migration_test -- --ignored"]
async fn test_phase1_delta_workflows() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_pool(&db_url).await;

    verify_table_exists(&pool, "workflows").await;
    verify_column(&pool, "workflows", "workflow_id", "uuid").await;
    verify_column(&pool, "workflows", "name", "text").await;
    verify_column(&pool, "workflows", "skill_chain", "jsonb").await;
    verify_column(&pool, "workflows", "enabled", "boolean").await;
    verify_column(&pool, "workflows", "created_by", "uuid").await;

    // Verify indexes
    verify_index_exists(&pool, "workflows", "idx_workflows_enabled").await;
    verify_index_exists(&pool, "workflows", "idx_workflows_created_by").await;

    // Insert test workflow with skill_chain JSON array
    sqlx::query(
        "INSERT INTO workflows (name, skill_chain) VALUES ('test_workflow', '[\"skill_a\", \"skill_b\"]'::jsonb)",
    )
    .execute(&pool)
    .await
    .expect("Should insert test workflow");

    // Verify UNIQUE constraint on name
    let duplicate = sqlx::query(
        "INSERT INTO workflows (name, skill_chain) VALUES ('test_workflow', '[]'::jsonb)",
    )
    .execute(&pool)
    .await;

    assert!(
        duplicate.is_err(),
        "Duplicate workflow name should be rejected by UNIQUE constraint"
    );

    println!("✓ Phase 1 delta workflows verified");
}

// =============================================================================
// PHASE 1 DELTA: Sub-Agents
// =============================================================================

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test migration_test -- --ignored"]
async fn test_phase1_delta_sub_agents() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_pool(&db_url).await;

    verify_table_exists(&pool, "sub_agents").await;
    verify_column(&pool, "sub_agents", "sub_agent_id", "uuid").await;
    verify_column(&pool, "sub_agents", "parent_id", "uuid").await;
    verify_column(&pool, "sub_agents", "created_by", "uuid").await;
    verify_column(&pool, "sub_agents", "name", "text").await;
    verify_column(&pool, "sub_agents", "role", "text").await;
    verify_column(&pool, "sub_agents", "directives", "jsonb").await;
    verify_column(&pool, "sub_agents", "ephemeral", "boolean").await;

    // Verify indexes
    verify_index_exists(&pool, "sub_agents", "idx_sub_agents_parent").await;
    verify_index_exists(&pool, "sub_agents", "idx_sub_agents_created_by").await;
    verify_index_exists(&pool, "sub_agents", "idx_sub_agents_active").await;
    verify_index_exists(&pool, "sub_agents", "idx_sub_agents_role").await;

    // Verify foreign keys: sub_agent_id references identities
    let lian_id: uuid::Uuid =
        sqlx::query_scalar("SELECT identity_id FROM identities WHERE name = 'Lian' LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("Lian should exist");

    // Create a sub-agent identity first
    let sub_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO identities (name, identity_type) VALUES ('test_sub_agent', 'sub_agent') RETURNING identity_id",
    )
    .fetch_one(&pool)
    .await
    .expect("Should insert sub-agent identity");

    sqlx::query(
        "INSERT INTO sub_agents (sub_agent_id, parent_id, created_by, name, role) \
         VALUES ($1, $2, $2, 'test_sub', 'researcher')",
    )
    .bind(sub_id)
    .bind(lian_id)
    .execute(&pool)
    .await
    .expect("Should insert sub-agent");

    // Verify partial index: active sub-agents (terminated_at IS NULL)
    let active_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sub_agents WHERE terminated_at IS NULL")
            .fetch_one(&pool)
            .await
            .expect("Should query active sub-agents");

    assert!(active_count >= 1, "Should have at least 1 active sub-agent");

    // Clean up
    sqlx::query("DELETE FROM sub_agents WHERE sub_agent_id = $1")
        .bind(sub_id)
        .execute(&pool)
        .await
        .expect("Should delete sub-agent");
    sqlx::query("DELETE FROM identities WHERE identity_id = $1")
        .bind(sub_id)
        .execute(&pool)
        .await
        .expect("Should delete sub-agent identity");

    println!("✓ Phase 1 delta sub-agents verified");
}

// =============================================================================
// PHASE 1 DELTA: XP System
// =============================================================================

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test migration_test -- --ignored"]
async fn test_phase1_delta_xp_system() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_pool(&db_url).await;

    // Verify level_progression has 99 rows
    let level_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM level_progression")
        .fetch_one(&pool)
        .await
        .expect("Should query level count");

    assert_eq!(level_count, 99, "Should have exactly 99 levels");

    // Query level 1: xp_required = 0, total_xp_required = 0
    let level1: (i64, i64) = sqlx::query_as(
        "SELECT xp_required, total_xp_required FROM level_progression WHERE level = 1",
    )
    .fetch_one(&pool)
    .await
    .expect("Level 1 should exist");

    assert_eq!(level1.0, 0, "Level 1 xp_required should be 0");
    assert_eq!(level1.1, 0, "Level 1 total_xp_required should be 0");

    // Query level 5: verify milestone_feature = 'unlock_sub_agents'
    let level5_milestone: Option<String> =
        sqlx::query_scalar("SELECT milestone_feature FROM level_progression WHERE level = 5")
            .fetch_one(&pool)
            .await
            .expect("Level 5 should exist");

    assert_eq!(
        level5_milestone.as_deref(),
        Some("unlock_sub_agents"),
        "Level 5 milestone should be 'unlock_sub_agents'"
    );

    // Query level 99: verify total_xp_required = 3875297318 (exponent 1.172)
    let level99_total: i64 =
        sqlx::query_scalar("SELECT total_xp_required FROM level_progression WHERE level = 99")
            .fetch_one(&pool)
            .await
            .expect("Level 99 should exist");

    assert_eq!(
        level99_total, 3_875_297_318,
        "Level 99 total_xp_required should be 3875297318"
    );

    // Verify agent_xp table
    verify_table_exists(&pool, "agent_xp").await;
    verify_column(&pool, "agent_xp", "identity_id", "uuid").await;
    verify_column(&pool, "agent_xp", "total_xp", "bigint").await;
    verify_column(&pool, "agent_xp", "level", "integer").await;
    verify_column(&pool, "agent_xp", "xp_to_next_level", "bigint").await;

    // Verify skill_metrics table
    verify_table_exists(&pool, "skill_metrics").await;
    verify_column(&pool, "skill_metrics", "skill_id", "uuid").await;
    verify_column(&pool, "skill_metrics", "usage_count", "integer").await;
    verify_column(&pool, "skill_metrics", "total_xp_earned", "bigint").await;
    verify_column(&pool, "skill_metrics", "skill_level", "integer").await;

    // Verify xp_events table with CHECK constraint on source enum
    verify_table_exists(&pool, "xp_events").await;

    let valid_sources = [
        "task_completion",
        "ledger_signing",
        "skill_usage",
        "quality_bonus",
        "elixir_creation",
        "elixir_usage",
    ];

    let lian_id: uuid::Uuid =
        sqlx::query_scalar("SELECT identity_id FROM identities WHERE name = 'Lian' LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("Lian should exist");

    for source in &valid_sources {
        let result = sqlx::query(
            "INSERT INTO xp_events (identity_id, source, xp_amount) VALUES ($1, $2, 10)",
        )
        .bind(lian_id)
        .bind(source)
        .execute(&pool)
        .await;

        assert!(
            result.is_ok(),
            "XP event source '{}' should be accepted",
            source
        );
    }

    // Invalid source should fail
    let invalid_source = sqlx::query(
        "INSERT INTO xp_events (identity_id, source, xp_amount) VALUES ($1, 'invalid_source', 10)",
    )
    .bind(lian_id)
    .execute(&pool)
    .await;

    assert!(
        invalid_source.is_err(),
        "Invalid XP event source should be rejected by CHECK constraint"
    );

    println!("✓ Phase 1 delta XP system verified");
}

// =============================================================================
// PHASE 1 DELTA: Elixirs System
// =============================================================================

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test migration_test -- --ignored"]
async fn test_phase1_delta_elixirs_system() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_pool(&db_url).await;

    // Verify elixirs table
    verify_table_exists(&pool, "elixirs").await;
    verify_column(&pool, "elixirs", "elixir_id", "uuid").await;
    verify_column(&pool, "elixirs", "name", "text").await;
    verify_column(&pool, "elixirs", "elixir_type", "text").await;
    verify_column(&pool, "elixirs", "quality_score", "real").await;
    verify_column(&pool, "elixirs", "active", "boolean").await;

    // Verify CHECK constraint on elixir_type
    let valid_types = [
        "skill_backup",
        "domain_knowledge",
        "context_cache",
        "training_data",
    ];

    for (i, elixir_type) in valid_types.iter().enumerate() {
        let result = sqlx::query(
            "INSERT INTO elixirs (name, elixir_type, dataset) VALUES ($1, $2, '{}'::jsonb)",
        )
        .bind(format!("test_elixir_{}", i))
        .bind(elixir_type)
        .execute(&pool)
        .await;

        assert!(
            result.is_ok(),
            "Elixir type '{}' should be accepted. Error: {:?}",
            elixir_type,
            result.err()
        );
    }

    // Invalid elixir_type should fail
    let invalid_type = sqlx::query(
        "INSERT INTO elixirs (name, elixir_type, dataset) VALUES ('invalid_elixir', 'invalid_type', '{}'::jsonb)",
    )
    .execute(&pool)
    .await;

    assert!(
        invalid_type.is_err(),
        "Invalid elixir_type should be rejected by CHECK constraint"
    );

    // Verify quality_score CHECK constraint (0.0 - 100.0)
    let invalid_quality = sqlx::query(
        "INSERT INTO elixirs (name, elixir_type, dataset, quality_score) VALUES ('bad_quality', 'skill_backup', '{}'::jsonb, 150.0)",
    )
    .execute(&pool)
    .await;

    assert!(
        invalid_quality.is_err(),
        "Quality score > 100.0 should be rejected"
    );

    // Verify ivfflat index on embedding
    verify_index_exists(&pool, "elixirs", "idx_elixirs_embedding").await;

    // Verify elixir_versions table with UNIQUE constraint
    verify_table_exists(&pool, "elixir_versions").await;

    let elixir_id: uuid::Uuid =
        sqlx::query_scalar("SELECT elixir_id FROM elixirs WHERE name = 'test_elixir_0' LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("Test elixir should exist");

    sqlx::query(
        "INSERT INTO elixir_versions (elixir_id, version_number, dataset) VALUES ($1, 1, '{}'::jsonb)",
    )
    .bind(elixir_id)
    .execute(&pool)
    .await
    .expect("Should insert elixir version");

    let duplicate_version = sqlx::query(
        "INSERT INTO elixir_versions (elixir_id, version_number, dataset) VALUES ($1, 1, '{}'::jsonb)",
    )
    .bind(elixir_id)
    .execute(&pool)
    .await;

    assert!(
        duplicate_version.is_err(),
        "Duplicate (elixir_id, version_number) should be rejected"
    );

    // Verify elixir_usage table
    verify_table_exists(&pool, "elixir_usage").await;

    // Verify sub_agent_elixirs table with UNIQUE constraint
    verify_table_exists(&pool, "sub_agent_elixirs").await;

    // Verify elixir_drafts table with CHECK constraint on status
    verify_table_exists(&pool, "elixir_drafts").await;

    let skill_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO skills (name, runtime) VALUES ('draft_test_skill', 'node') RETURNING skill_id",
    )
    .fetch_one(&pool)
    .await
    .expect("Should insert test skill");

    for status in &["pending", "approved", "rejected"] {
        let result = sqlx::query(
            "INSERT INTO elixir_drafts (skill_id, proposed_name, dataset, status) VALUES ($1, $2, '{}'::jsonb, $3)",
        )
        .bind(skill_id)
        .bind(format!("draft_{}", status))
        .bind(status)
        .execute(&pool)
        .await;

        assert!(
            result.is_ok(),
            "Elixir draft status '{}' should be accepted",
            status
        );
    }

    let invalid_status = sqlx::query(
        "INSERT INTO elixir_drafts (skill_id, proposed_name, dataset, status) VALUES ($1, 'bad_draft', '{}'::jsonb, 'invalid')",
    )
    .bind(skill_id)
    .execute(&pool)
    .await;

    assert!(
        invalid_status.is_err(),
        "Invalid draft status should be rejected by CHECK constraint"
    );

    println!("✓ Phase 1 delta elixirs system verified");
}

// =============================================================================
// PHASE 1 DELTA: Seed Data
// =============================================================================

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test migration_test -- --ignored"]
async fn test_phase1_delta_seed_data() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_pool(&db_url).await;

    // Verify Lian's XP record exists
    let lian_xp: (i64, i32, i64) = sqlx::query_as(
        "SELECT total_xp, level, xp_to_next_level FROM agent_xp \
         WHERE identity_id = (SELECT identity_id FROM identities WHERE name = 'Lian')",
    )
    .fetch_one(&pool)
    .await
    .expect("Lian's XP record should exist");

    assert_eq!(lian_xp.0, 0, "Lian total_xp should be 0");
    assert_eq!(lian_xp.1, 1, "Lian level should be 1");
    assert_eq!(lian_xp.2, 117, "Lian xp_to_next_level should be 117");

    // Verify machine profile configs in config_store
    let urim_config: serde_json::Value =
        sqlx::query_scalar("SELECT value FROM config_store WHERE key = 'machine_profile.urim'")
            .fetch_one(&pool)
            .await
            .expect("Urim config should exist");

    assert_eq!(
        urim_config["elixir_max_count"].as_i64(),
        Some(200),
        "Urim elixir_max_count should be 200"
    );
    assert_eq!(
        urim_config["elixir_max_size_mb"].as_i64(),
        Some(50),
        "Urim elixir_max_size_mb should be 50"
    );
    assert_eq!(
        urim_config["elixir_max_per_sub_agent"].as_i64(),
        Some(10),
        "Urim elixir_max_per_sub_agent should be 10"
    );

    let thummim_config: serde_json::Value =
        sqlx::query_scalar("SELECT value FROM config_store WHERE key = 'machine_profile.thummim'")
            .fetch_one(&pool)
            .await
            .expect("Thummim config should exist");

    assert_eq!(
        thummim_config["elixir_max_count"].as_i64(),
        Some(100),
        "Thummim elixir_max_count should be 100"
    );
    assert_eq!(
        thummim_config["elixir_max_size_mb"].as_i64(),
        Some(20),
        "Thummim elixir_max_size_mb should be 20"
    );
    assert_eq!(
        thummim_config["elixir_max_per_sub_agent"].as_i64(),
        Some(5),
        "Thummim elixir_max_per_sub_agent should be 5"
    );

    println!("✓ Phase 1 delta seed data verified");
}

// =============================================================================
// SCHEMA FIXES: Lian Pronouns
// =============================================================================

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test migration_test -- --ignored"]
async fn test_schema_fixes_lian_pronouns() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_pool(&db_url).await;

    let pronouns: String = sqlx::query_scalar(
        "SELECT pronouns FROM identities WHERE name = 'Lian' AND identity_type = 'core'",
    )
    .fetch_one(&pool)
    .await
    .expect("Lian should exist");

    assert_eq!(pronouns, "he/him", "Lian pronouns should be 'he/him'");
    assert_ne!(
        pronouns, "they/them",
        "Lian pronouns must not be 'they/them'"
    );
    assert_ne!(pronouns, "she/her", "Lian pronouns must not be 'she/her'");

    println!("✓ Schema fixes: Lian pronouns verified as 'he/him'");
}

// =============================================================================
// SCHEMA FIXES: subject_id TEXT type
// =============================================================================

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test migration_test -- --ignored"]
async fn test_schema_fixes_subject_id_text_type() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_pool(&db_url).await;

    // Verify column type is TEXT
    verify_column(&pool, "capability_grants", "subject_id", "text").await;

    // Insert test grant with UUID-format subject_id
    let result_uuid = sqlx::query(
        "INSERT INTO capability_grants (subject_type, subject_id, capability_key) \
         VALUES ('identity', '550e8400-e29b-41d4-a716-446655440000', 'fs.read')",
    )
    .execute(&pool)
    .await;

    assert!(
        result_uuid.is_ok(),
        "UUID-format subject_id should be accepted. Error: {:?}",
        result_uuid.err()
    );

    // Insert test grant with external format: telegram:12345
    let result_telegram = sqlx::query(
        "INSERT INTO capability_grants (subject_type, subject_id, capability_key) \
         VALUES ('external_key', 'telegram:12345', 'fs.read')",
    )
    .execute(&pool)
    .await;

    assert!(
        result_telegram.is_ok(),
        "External format 'telegram:12345' should be accepted. Error: {:?}",
        result_telegram.err()
    );

    // Insert test grant with external format: discord:user1234
    let result_discord = sqlx::query(
        "INSERT INTO capability_grants (subject_type, subject_id, capability_key) \
         VALUES ('channel', 'discord:user1234', 'net.http')",
    )
    .execute(&pool)
    .await;

    assert!(
        result_discord.is_ok(),
        "External format 'discord:user1234' should be accepted. Error: {:?}",
        result_discord.err()
    );

    // Attempt invalid format: email with @ should fail CHECK constraint
    let result_invalid = sqlx::query(
        "INSERT INTO capability_grants (subject_type, subject_id, capability_key) \
         VALUES ('identity', 'invalid@email.com', 'fs.read')",
    )
    .execute(&pool)
    .await;

    assert!(
        result_invalid.is_err(),
        "Invalid subject_id format 'invalid@email.com' should be rejected by CHECK constraint"
    );

    println!("✓ Schema fixes: subject_id TEXT type with CHECK constraint verified");
}

// =============================================================================
// SCHEMA FIXES: subject_type enum expansion
// =============================================================================

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test migration_test -- --ignored"]
async fn test_schema_fixes_subject_type_enum() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_pool(&db_url).await;

    // Test all valid subject_type values
    let valid_types = ["identity", "skill", "channel", "session", "external_key"];

    for (i, subject_type) in valid_types.iter().enumerate() {
        let subject_id = format!("test-subject-{}", i);
        let result = sqlx::query(
            "INSERT INTO capability_grants (subject_type, subject_id, capability_key) \
             VALUES ($1, $2, 'fs.read')",
        )
        .bind(subject_type)
        .bind(&subject_id)
        .execute(&pool)
        .await;

        assert!(
            result.is_ok(),
            "subject_type '{}' should be accepted. Error: {:?}",
            subject_type,
            result.err()
        );
    }

    // Invalid subject_type should fail
    let invalid_type = sqlx::query(
        "INSERT INTO capability_grants (subject_type, subject_id, capability_key) \
         VALUES ('invalid_type', 'test-invalid', 'fs.read')",
    )
    .execute(&pool)
    .await;

    assert!(
        invalid_type.is_err(),
        "Invalid subject_type 'invalid_type' should be rejected by CHECK constraint"
    );

    println!("✓ Schema fixes: subject_type enum expansion verified");
}

// =============================================================================
// SCHEMA FIXES: LZ4 Compression
// =============================================================================

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test migration_test -- --ignored"]
async fn test_schema_fixes_lz4_compression() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_pool(&db_url).await;

    // memories.content and run_logs.message are BYTEA after migration 0009
    // (encryption at rest) with default compression — LZ4 was intentionally
    // removed because encrypted output is incompressible.
    assert!(
        !verify_column_compression(&pool, "memories", "content").await,
        "memories.content should NOT have LZ4 compression (reset by migration 0009)"
    );

    assert!(
        !verify_column_compression(&pool, "run_logs", "message").await,
        "run_logs.message should NOT have LZ4 compression (reset by migration 0009)"
    );

    // Verify LZ4 compression on ledger_events.metadata (unchanged by migration 0009)
    assert!(
        verify_column_compression(&pool, "ledger_events", "metadata").await,
        "ledger_events.metadata should have LZ4 compression"
    );

    // Insert large BYTEA content (>8KB) to verify storage works
    let lian_id: uuid::Uuid =
        sqlx::query_scalar("SELECT identity_id FROM identities WHERE name = 'Lian' LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("Lian should exist");

    let large_content = "x".repeat(10_000); // 10KB of data
    let large_content_bytes = large_content.as_bytes().to_vec();
    let memory_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO memories (identity_id, content, source) VALUES ($1, $2, 'observation') RETURNING memory_id",
    )
    .bind(lian_id)
    .bind(&large_content_bytes)
    .fetch_one(&pool)
    .await
    .expect("Should insert large memory content");

    // Update to verify BYTEA column is writable
    sqlx::query("UPDATE memories SET content = content WHERE memory_id = $1")
        .bind(memory_id)
        .execute(&pool)
        .await
        .expect("Should update memory");

    println!(
        "✓ Schema fixes: compression verified (LZ4 on ledger_events, default on memories/run_logs)"
    );
}

// =============================================================================
// MIGRATION IDEMPOTENCY
// =============================================================================

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test migration_test -- --ignored"]
async fn test_migration_idempotency() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("Failed to connect to database");

    // Run migrations first time
    carnelian_core::db::run_migrations(&pool, None)
        .await
        .expect("First migration run should succeed");

    // Count seed data rows
    let lian_count_1: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM identities WHERE name = 'Lian'")
            .fetch_one(&pool)
            .await
            .expect("Should count Lian");

    let level_count_1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM level_progression")
        .fetch_one(&pool)
        .await
        .expect("Should count levels");

    let cap_count_1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM capabilities")
        .fetch_one(&pool)
        .await
        .expect("Should count capabilities");

    // Run migrations second time
    carnelian_core::db::run_migrations(&pool, None)
        .await
        .expect("Second migration run should succeed (idempotent)");

    // Verify row counts remain stable
    let lian_count_2: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM identities WHERE name = 'Lian'")
            .fetch_one(&pool)
            .await
            .expect("Should count Lian");

    let level_count_2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM level_progression")
        .fetch_one(&pool)
        .await
        .expect("Should count levels");

    let cap_count_2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM capabilities")
        .fetch_one(&pool)
        .await
        .expect("Should count capabilities");

    assert_eq!(
        lian_count_1, lian_count_2,
        "Lian count should be stable after re-run"
    );
    assert_eq!(
        level_count_1, level_count_2,
        "Level count should be stable after re-run"
    );
    assert_eq!(
        cap_count_1, cap_count_2,
        "Capability count should be stable after re-run"
    );

    println!("✓ Migration idempotency verified");
}
