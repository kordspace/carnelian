#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::too_many_lines)]

//! Integration tests for skill discovery module
//!
//! Tests manifest validation, checksum computation, database upsert,
//! directory scanning, event emission, and the refresh API endpoint.
//!
//! # Running Tests
//!
//! ```bash
//! cargo test --test skill_discovery_tests -- --ignored
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use carnelian_core::events::EventStream;
use carnelian_core::skills::{
    compute_manifest_checksum, validate_manifest, SkillDiscovery, SkillManifest,
};
use sqlx::PgPool;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};

/// Spin up a PostgreSQL container and return a connected pool.
async fn setup_postgres() -> (PgPool, testcontainers::ContainerAsync<GenericImage>) {
    let image = GenericImage::new("pgvector/pgvector", "pg16").with_wait_for(
        testcontainers::core::WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ),
    );

    let container = image
        .with_env_var("POSTGRES_USER", "test")
        .with_env_var("POSTGRES_PASSWORD", "test")
        .with_env_var("POSTGRES_DB", "carnelian_test")
        .start()
        .await
        .expect("Failed to start PostgreSQL container");

    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get mapped port");

    let url = format!("postgresql://test:test@127.0.0.1:{}/carnelian_test", port);

    // Retry connection with backoff
    let pool = {
        let mut last_err = None;
        let mut pool_opt = None;
        for _ in 0..30 {
            match sqlx::postgres::PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(Duration::from_secs(5))
                .connect(&url)
                .await
            {
                Ok(p) => {
                    pool_opt = Some(p);
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
        pool_opt.unwrap_or_else(|| panic!("Failed to connect to PostgreSQL: {:?}", last_err))
    };

    // Run migrations
    carnelian_core::db::run_migrations(&pool, None)
        .await
        .expect("Failed to run migrations");

    (pool, container)
}

/// Create a temporary registry directory with skill manifests.
fn create_temp_registry(skills: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    for (name, content) in skills {
        let skill_dir = dir.path().join(name);
        std::fs::create_dir_all(&skill_dir).expect("Failed to create skill dir");
        std::fs::write(skill_dir.join("skill.json"), content).expect("Failed to write skill.json");
    }
    dir
}

// =============================================================================
// MANIFEST VALIDATION TESTS
// =============================================================================

#[test]
fn test_validate_valid_manifest() {
    let manifest = SkillManifest {
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        runtime: "node".to_string(),
        version: "1.0.0".to_string(),
        capabilities_required: vec![],
        homepage: None,
        sandbox: None,
        openclaw_compat: None,
    };

    let errors = validate_manifest(&manifest);
    assert!(
        errors.is_empty(),
        "Valid manifest should have no errors: {:?}",
        errors
    );
}

#[test]
fn test_validate_missing_name() {
    let manifest = SkillManifest {
        name: String::new(),
        description: "A test skill".to_string(),
        runtime: "node".to_string(),
        version: "1.0.0".to_string(),
        capabilities_required: vec![],
        homepage: None,
        sandbox: None,
        openclaw_compat: None,
    };

    let errors = validate_manifest(&manifest);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field, "name");
}

#[test]
fn test_validate_missing_description() {
    let manifest = SkillManifest {
        name: "test-skill".to_string(),
        description: String::new(),
        runtime: "node".to_string(),
        version: "1.0.0".to_string(),
        capabilities_required: vec![],
        homepage: None,
        sandbox: None,
        openclaw_compat: None,
    };

    let errors = validate_manifest(&manifest);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field, "description");
}

#[test]
fn test_validate_invalid_runtime() {
    let manifest = SkillManifest {
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        runtime: "ruby".to_string(),
        version: "1.0.0".to_string(),
        capabilities_required: vec![],
        homepage: None,
        sandbox: None,
        openclaw_compat: None,
    };

    let errors = validate_manifest(&manifest);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field, "runtime");
    assert!(errors[0].message.contains("ruby"));
}

#[test]
fn test_validate_all_valid_runtimes() {
    for runtime in &["node", "python", "shell", "wasm"] {
        let manifest = SkillManifest {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            runtime: (*runtime).to_string(),
            version: "1.0.0".to_string(),
            capabilities_required: vec![],
            homepage: None,
            sandbox: None,
            openclaw_compat: None,
        };
        let errors = validate_manifest(&manifest);
        assert!(errors.is_empty(), "Runtime '{}' should be valid", runtime);
    }
}

#[test]
fn test_validate_multiple_errors() {
    let manifest = SkillManifest {
        name: String::new(),
        description: String::new(),
        runtime: "invalid".to_string(),
        version: "1.0.0".to_string(),
        capabilities_required: vec![],
        homepage: None,
        sandbox: None,
        openclaw_compat: None,
    };

    let errors = validate_manifest(&manifest);
    assert_eq!(
        errors.len(),
        3,
        "Should have 3 errors: name, description, runtime"
    );
}

// =============================================================================
// CHECKSUM TESTS
// =============================================================================

#[test]
fn test_checksum_deterministic() {
    let manifest = SkillManifest {
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        runtime: "node".to_string(),
        version: "1.0.0".to_string(),
        capabilities_required: vec!["fs.read".to_string()],
        homepage: None,
        sandbox: None,
        openclaw_compat: None,
    };

    let checksum1 = compute_manifest_checksum(&manifest);
    let checksum2 = compute_manifest_checksum(&manifest);
    assert_eq!(checksum1, checksum2, "Checksum should be deterministic");
    assert_eq!(checksum1.len(), 64, "blake3 hex should be 64 chars");
}

#[test]
fn test_checksum_changes_on_modification() {
    let mut manifest = SkillManifest {
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        runtime: "node".to_string(),
        version: "1.0.0".to_string(),
        capabilities_required: vec![],
        homepage: None,
        sandbox: None,
        openclaw_compat: None,
    };

    let checksum1 = compute_manifest_checksum(&manifest);
    manifest.version = "2.0.0".to_string();
    let checksum2 = compute_manifest_checksum(&manifest);
    assert_ne!(
        checksum1, checksum2,
        "Checksum should change when manifest changes"
    );
}

// =============================================================================
// MANIFEST PARSING TESTS
// =============================================================================

#[test]
fn test_parse_minimal_manifest() {
    let json = r#"{
        "name": "minimal",
        "description": "Minimal skill",
        "runtime": "shell"
    }"#;

    let manifest: SkillManifest = serde_json::from_str(json).expect("Should parse");
    assert_eq!(manifest.name, "minimal");
    assert_eq!(manifest.version, "0.1.0"); // default
    assert!(manifest.capabilities_required.is_empty());
    assert!(manifest.sandbox.is_none());
}

#[test]
fn test_parse_full_manifest() {
    let json = r#"{
        "name": "full-skill",
        "description": "Full skill with all fields",
        "runtime": "node",
        "version": "2.0.0",
        "capabilities_required": ["fs.read", "net.http"],
        "homepage": "https://example.com",
        "sandbox": {
            "mounts": [{"host": "/tmp", "container": "/workspace", "readonly": true}],
            "network": "restricted",
            "max_memory_mb": 512,
            "max_cpu_percent": 50,
            "env": {"NODE_ENV": "production"}
        },
        "openclaw_compat": {
            "emoji": "🔧",
            "requires": {"bins": ["curl"]},
            "tags": ["utility"]
        }
    }"#;

    let manifest: SkillManifest = serde_json::from_str(json).expect("Should parse");
    assert_eq!(manifest.name, "full-skill");
    assert_eq!(manifest.version, "2.0.0");
    assert_eq!(manifest.capabilities_required.len(), 2);

    let sandbox = manifest.sandbox.as_ref().unwrap();
    assert_eq!(sandbox.network, "restricted");
    assert_eq!(sandbox.max_memory_mb, 512);
    assert_eq!(sandbox.mounts.len(), 1);
    assert!(sandbox.mounts[0].readonly);

    let compat = manifest.openclaw_compat.as_ref().unwrap();
    assert_eq!(compat.emoji.as_deref(), Some("🔧"));
    assert_eq!(compat.tags, vec!["utility"]);
}

// =============================================================================
// DATABASE INTEGRATION TESTS (require Docker)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_discovery_insert_new_skill() {
    let (pool, _container) = setup_postgres().await;

    let registry = create_temp_registry(&[(
        "test-skill",
        r#"{"name":"test-skill","description":"A test","runtime":"shell","version":"1.0.0"}"#,
    )]);

    let discovery = SkillDiscovery::new(pool.clone(), None, registry.path().to_path_buf());
    let result = discovery.refresh().await.expect("Refresh should succeed");

    assert_eq!(result.discovered, 1, "Should discover 1 new skill");
    assert_eq!(result.updated, 0);
    assert_eq!(result.removed, 0);

    // Verify in database
    let count: i64 = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM skills WHERE name = 'test-skill'",
    )
    .fetch_one(&pool)
    .await
    .expect("Query should succeed")
    .unwrap_or(0);
    assert_eq!(count, 1, "Skill should be in database");
}

#[tokio::test]
#[ignore]
async fn test_discovery_update_existing_skill() {
    let (pool, _container) = setup_postgres().await;

    // First insert
    let registry = create_temp_registry(&[(
        "update-test",
        r#"{"name":"update-test","description":"Version 1","runtime":"node","version":"1.0.0"}"#,
    )]);

    let discovery = SkillDiscovery::new(pool.clone(), None, registry.path().to_path_buf());
    let r1 = discovery.refresh().await.expect("First refresh");
    assert_eq!(r1.discovered, 1);

    // Update the manifest
    let skill_dir = registry.path().join("update-test");
    std::fs::write(
        skill_dir.join("skill.json"),
        r#"{"name":"update-test","description":"Version 2","runtime":"node","version":"2.0.0"}"#,
    )
    .expect("Write updated manifest");

    let r2 = discovery.refresh().await.expect("Second refresh");
    assert_eq!(r2.discovered, 0, "Should not discover again");
    assert_eq!(r2.updated, 1, "Should update 1 skill");

    // Verify updated description in database
    let desc: Option<String> =
        sqlx::query_scalar("SELECT description FROM skills WHERE name = 'update-test'")
            .fetch_one(&pool)
            .await
            .expect("Query should succeed");
    assert_eq!(desc.as_deref(), Some("Version 2"));
}

#[tokio::test]
#[ignore]
async fn test_discovery_unchanged_skill_not_updated() {
    let (pool, _container) = setup_postgres().await;

    let registry = create_temp_registry(&[(
        "stable-skill",
        r#"{"name":"stable-skill","description":"Stable","runtime":"python","version":"1.0.0"}"#,
    )]);

    let discovery = SkillDiscovery::new(pool.clone(), None, registry.path().to_path_buf());

    let r1 = discovery.refresh().await.expect("First refresh");
    assert_eq!(r1.discovered, 1);

    // Refresh again without changes
    let r2 = discovery.refresh().await.expect("Second refresh");
    assert_eq!(r2.discovered, 0, "No new discoveries");
    assert_eq!(r2.updated, 0, "No updates (checksum unchanged)");
    assert_eq!(r2.removed, 0, "No removals");
}

#[tokio::test]
#[ignore]
async fn test_discovery_remove_stale_skill() {
    let (pool, _container) = setup_postgres().await;

    // Create registry with two skills
    let registry = create_temp_registry(&[
        (
            "keep-me",
            r#"{"name":"keep-me","description":"Keep","runtime":"shell","version":"1.0.0"}"#,
        ),
        (
            "remove-me",
            r#"{"name":"remove-me","description":"Remove","runtime":"shell","version":"1.0.0"}"#,
        ),
    ]);

    let discovery = SkillDiscovery::new(pool.clone(), None, registry.path().to_path_buf());
    let r1 = discovery.refresh().await.expect("First refresh");
    assert_eq!(r1.discovered, 2);

    // Remove one skill directory
    std::fs::remove_dir_all(registry.path().join("remove-me")).expect("Remove skill dir");

    let r2 = discovery.refresh().await.expect("Second refresh");
    assert_eq!(r2.removed, 1, "Should remove 1 stale skill");

    // Verify removal
    let count: i64 = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM skills WHERE name = 'remove-me'",
    )
    .fetch_one(&pool)
    .await
    .expect("Query should succeed")
    .unwrap_or(0);
    assert_eq!(count, 0, "Removed skill should not be in database");

    // Verify kept skill still exists
    let count: i64 =
        sqlx::query_scalar::<_, Option<i64>>("SELECT COUNT(*) FROM skills WHERE name = 'keep-me'")
            .fetch_one(&pool)
            .await
            .expect("Query should succeed")
            .unwrap_or(0);
    assert_eq!(count, 1, "Kept skill should still be in database");
}

#[tokio::test]
#[ignore]
async fn test_discovery_multiple_skills() {
    let (pool, _container) = setup_postgres().await;

    let registry = create_temp_registry(&[
        (
            "skill-a",
            r#"{"name":"skill-a","description":"Skill A","runtime":"node","version":"1.0.0"}"#,
        ),
        (
            "skill-b",
            r#"{"name":"skill-b","description":"Skill B","runtime":"python","version":"1.0.0"}"#,
        ),
        (
            "skill-c",
            r#"{"name":"skill-c","description":"Skill C","runtime":"shell","version":"1.0.0"}"#,
        ),
    ]);

    let discovery = SkillDiscovery::new(pool.clone(), None, registry.path().to_path_buf());
    let result = discovery.refresh().await.expect("Refresh should succeed");

    assert_eq!(result.discovered, 3, "Should discover 3 skills");

    let count: i64 = sqlx::query_scalar::<_, Option<i64>>("SELECT COUNT(*) FROM skills")
        .fetch_one(&pool)
        .await
        .expect("Query should succeed")
        .unwrap_or(0);
    assert_eq!(count, 3, "Database should have 3 skills");
}

#[tokio::test]
#[ignore]
async fn test_discovery_invalid_manifest_skipped() {
    let (pool, _container) = setup_postgres().await;

    let registry = create_temp_registry(&[
        (
            "valid-skill",
            r#"{"name":"valid-skill","description":"Valid","runtime":"node","version":"1.0.0"}"#,
        ),
        (
            "invalid-skill",
            r#"{"name":"","description":"","runtime":"invalid"}"#,
        ),
        ("broken-json", r"{ this is not json }"),
    ]);

    let discovery = SkillDiscovery::new(pool.clone(), None, registry.path().to_path_buf());
    let result = discovery.refresh().await.expect("Refresh should succeed");

    assert_eq!(
        result.discovered, 1,
        "Only valid skill should be discovered"
    );
}

#[tokio::test]
#[ignore]
async fn test_discovery_event_emission() {
    let (pool, _container) = setup_postgres().await;

    let event_stream = Arc::new(EventStream::new(1000, 100));
    let mut subscriber = event_stream.subscribe();

    let registry = create_temp_registry(&[(
        "event-test",
        r#"{"name":"event-test","description":"Event test","runtime":"shell","version":"1.0.0"}"#,
    )]);

    let discovery = SkillDiscovery::new(
        pool.clone(),
        Some(event_stream.clone()),
        registry.path().to_path_buf(),
    );
    discovery.refresh().await.expect("Refresh should succeed");

    // Check that a SkillDiscovered event was emitted
    let event = tokio::time::timeout(Duration::from_secs(2), subscriber.recv())
        .await
        .expect("Should receive event within timeout")
        .expect("Should receive event");

    assert_eq!(
        event.event_type,
        carnelian_common::types::EventType::SkillDiscovered
    );
    assert_eq!(event.payload["name"], "event-test");
}

#[tokio::test]
#[ignore]
async fn test_discovery_empty_registry() {
    let (pool, _container) = setup_postgres().await;

    let registry = tempfile::tempdir().expect("Create temp dir");

    let discovery = SkillDiscovery::new(pool.clone(), None, registry.path().to_path_buf());
    let result = discovery.refresh().await.expect("Refresh should succeed");

    assert_eq!(result.discovered, 0);
    assert_eq!(result.updated, 0);
    assert_eq!(result.removed, 0);
}

#[tokio::test]
#[ignore]
async fn test_discovery_nonexistent_registry() {
    let (pool, _container) = setup_postgres().await;

    let discovery = SkillDiscovery::new(
        pool.clone(),
        None,
        PathBuf::from("/nonexistent/path/skills"),
    );
    let result = discovery
        .refresh()
        .await
        .expect("Refresh should succeed even with missing dir");

    assert_eq!(result.discovered, 0);
    assert_eq!(result.updated, 0);
    assert_eq!(result.removed, 0);
}

#[tokio::test]
#[ignore]
async fn test_discovery_checksum_stored_in_db() {
    let (pool, _container) = setup_postgres().await;

    let manifest_json = r#"{"name":"checksum-test","description":"Checksum test","runtime":"node","version":"1.0.0"}"#;

    let registry = create_temp_registry(&[("checksum-test", manifest_json)]);

    let discovery = SkillDiscovery::new(pool.clone(), None, registry.path().to_path_buf());
    discovery.refresh().await.expect("Refresh should succeed");

    // Verify checksum is stored
    let checksum: Option<String> =
        sqlx::query_scalar("SELECT checksum FROM skills WHERE name = 'checksum-test'")
            .fetch_one(&pool)
            .await
            .expect("Query should succeed");

    assert!(checksum.is_some(), "Checksum should be stored");
    assert_eq!(
        checksum.as_ref().unwrap().len(),
        64,
        "blake3 hex should be 64 chars"
    );
}

#[tokio::test]
#[ignore]
async fn test_discovery_capabilities_stored() {
    let (pool, _container) = setup_postgres().await;

    let registry = create_temp_registry(&[(
        "cap-test",
        r#"{"name":"cap-test","description":"Cap test","runtime":"node","version":"1.0.0","capabilities_required":["fs.read","net.http"]}"#,
    )]);

    let discovery = SkillDiscovery::new(pool.clone(), None, registry.path().to_path_buf());
    discovery.refresh().await.expect("Refresh should succeed");

    let caps: Vec<String> = sqlx::query_scalar(
        "SELECT unnest(capabilities_required) FROM skills WHERE name = 'cap-test'",
    )
    .fetch_all(&pool)
    .await
    .expect("Query should succeed");

    assert_eq!(caps.len(), 2);
    assert!(caps.contains(&"fs.read".to_string()));
    assert!(caps.contains(&"net.http".to_string()));
}

// =============================================================================
// REGRESSION: MISSING REGISTRY MUST NOT DELETE EXISTING SKILLS
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_missing_registry_does_not_delete_existing_skills() {
    let (pool, _container) = setup_postgres().await;

    // First, insert a skill via a valid registry
    let registry = create_temp_registry(&[(
        "survivor",
        r#"{"name":"survivor","description":"Should survive","runtime":"shell","version":"1.0.0"}"#,
    )]);

    let discovery = SkillDiscovery::new(pool.clone(), None, registry.path().to_path_buf());
    let r1 = discovery.refresh().await.expect("First refresh");
    assert_eq!(r1.discovered, 1);

    // Verify the skill is in the database
    let count: i64 =
        sqlx::query_scalar::<_, Option<i64>>("SELECT COUNT(*) FROM skills WHERE name = 'survivor'")
            .fetch_one(&pool)
            .await
            .expect("Query should succeed")
            .unwrap_or(0);
    assert_eq!(count, 1, "Skill should be in database after first refresh");

    // Now refresh with a nonexistent registry path
    let discovery_missing = SkillDiscovery::new(
        pool.clone(),
        None,
        PathBuf::from("/nonexistent/registry/path"),
    );
    let r2 = discovery_missing
        .refresh()
        .await
        .expect("Refresh with missing registry should succeed");

    assert_eq!(r2.discovered, 0);
    assert_eq!(r2.updated, 0);
    assert_eq!(
        r2.removed, 0,
        "Must NOT remove skills when registry is missing"
    );

    // Verify the skill is still in the database
    let count: i64 =
        sqlx::query_scalar::<_, Option<i64>>("SELECT COUNT(*) FROM skills WHERE name = 'survivor'")
            .fetch_one(&pool)
            .await
            .expect("Query should succeed")
            .unwrap_or(0);
    assert_eq!(
        count, 1,
        "Skill must still be in database after refresh with missing registry"
    );
}

// =============================================================================
// FILE WATCHER DEBOUNCE TEST
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_file_watcher_debounce_coalesces_rapid_edits() {
    use carnelian_core::skills::start_file_watcher;

    let (pool, _container) = setup_postgres().await;
    let event_stream = Arc::new(EventStream::new(1000, 100));
    let mut subscriber = event_stream.subscribe();

    // Create a registry with one skill
    let registry = tempfile::tempdir().expect("Create temp dir");
    let skill_dir = registry.path().join("debounce-skill");
    std::fs::create_dir_all(&skill_dir).expect("Create skill dir");
    std::fs::write(
        skill_dir.join("skill.json"),
        r#"{"name":"debounce-skill","description":"v1","runtime":"shell","version":"1.0.0"}"#,
    )
    .expect("Write initial manifest");

    // Start the file watcher
    let watcher_handle = start_file_watcher(
        pool.clone(),
        event_stream.clone(),
        registry.path().to_path_buf(),
    );

    // Give the watcher time to initialize and perform its first scan
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Drain any events from the initial watcher trigger
    while subscriber.try_recv().is_ok() {}

    // Perform multiple rapid edits within the 2-second debounce window
    for i in 0..5 {
        std::fs::write(
            skill_dir.join("skill.json"),
            format!(
                r#"{{"name":"debounce-skill","description":"rapid edit {}","runtime":"shell","version":"1.0.{i}"}}"#,
                i
            ),
        )
        .expect("Write rapid edit");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Wait for the debounce window (2s) plus processing time
    tokio::time::sleep(Duration::from_secs(4)).await;

    // Count how many SkillUpdated/SkillDiscovered events were emitted
    let mut event_count = 0u32;
    while let Ok(event) = subscriber.try_recv() {
        match event.event_type {
            carnelian_common::types::EventType::SkillDiscovered
            | carnelian_common::types::EventType::SkillUpdated => {
                event_count += 1;
            }
            _ => {}
        }
    }

    // The debouncer should coalesce the 5 rapid edits into at most 1 refresh
    assert!(
        event_count <= 1,
        "Expected at most 1 refresh event from debounced rapid edits, got {}",
        event_count
    );

    // Clean up
    watcher_handle.abort();
}
