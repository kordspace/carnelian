//! Integration tests for encryption at rest functionality.
//!
//! These tests require a running `PostgreSQL` instance with pgcrypto extension
//! and the Carnelian schema applied (including migration 0009).
//!
//! Run with: `cargo test --test encryption_tests -- --ignored`

mod common;

use carnelian_core::crypto::{
    derive_aes_storage_key, derive_encryption_key, generate_ed25519_keypair,
};
use carnelian_core::encryption::EncryptionHelper;
use carnelian_core::memory::{MemoryManager, MemorySource};
use carnelian_core::server::insert_run_log;
use common::*;
use uuid::Uuid;

// =============================================================================
// ENCRYPT / DECRYPT ROUND-TRIP TESTS
// =============================================================================

/// Test encrypt/decrypt round-trip for short text.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_encrypt_decrypt_text_short() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let (signing, _) = generate_ed25519_keypair();
    let helper = EncryptionHelper::new(&pool, &signing);

    let plaintext = "Hello 🔥";
    let ciphertext = helper.encrypt_text(plaintext).await.expect("encrypt");
    assert_ne!(ciphertext, plaintext.as_bytes(), "ciphertext must differ");

    let decrypted = helper.decrypt_text(&ciphertext).await.expect("decrypt");
    assert_eq!(decrypted, plaintext);
}

/// Test encrypt/decrypt round-trip for 1 KB text.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_encrypt_decrypt_text_1kb() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let (signing, _) = generate_ed25519_keypair();
    let helper = EncryptionHelper::new(&pool, &signing);

    let plaintext = "A".repeat(1024);
    let ciphertext = helper.encrypt_text(&plaintext).await.expect("encrypt");
    let decrypted = helper.decrypt_text(&ciphertext).await.expect("decrypt");
    assert_eq!(decrypted, plaintext);
}

/// Test encrypt/decrypt round-trip for 100 KB text.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_encrypt_decrypt_text_100kb() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let (signing, _) = generate_ed25519_keypair();
    let helper = EncryptionHelper::new(&pool, &signing);

    let plaintext = "B".repeat(100 * 1024);
    let ciphertext = helper.encrypt_text(&plaintext).await.expect("encrypt");
    let decrypted = helper.decrypt_text(&ciphertext).await.expect("decrypt");
    assert_eq!(decrypted, plaintext);
}

/// Test encrypt/decrypt round-trip for binary data.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_encrypt_decrypt_bytes_roundtrip() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let (signing, _) = generate_ed25519_keypair();
    let helper = EncryptionHelper::new(&pool, &signing);

    let data: Vec<u8> = (0..=255).collect();
    let ciphertext = helper.encrypt_bytes(&data).await.expect("encrypt");
    let decrypted = helper.decrypt_bytes(&ciphertext).await.expect("decrypt");
    assert_eq!(decrypted, data);
}

// =============================================================================
// DECRYPTION FAILURE TESTS
// =============================================================================

/// Test that decryption with the wrong key fails.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_decrypt_wrong_key_fails() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let (signing1, _) = generate_ed25519_keypair();
    let (signing2, _) = generate_ed25519_keypair();
    let helper1 = EncryptionHelper::new(&pool, &signing1);
    let helper2 = EncryptionHelper::new(&pool, &signing2);

    let ciphertext = helper1.encrypt_text("secret data").await.expect("encrypt");
    let result = helper2.decrypt_text(&ciphertext).await;
    assert!(result.is_err(), "decryption with wrong key must fail");
}

/// Test that decryption of corrupted data fails.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_decrypt_corrupted_data_fails() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let (signing, _) = generate_ed25519_keypair();
    let helper = EncryptionHelper::new(&pool, &signing);

    let corrupted = vec![0u8, 1, 2, 3, 4, 5];
    let result = helper.decrypt_bytes(&corrupted).await;
    assert!(result.is_err(), "decryption of corrupted data must fail");
}

// =============================================================================
// KEY DERIVATION TESTS
// =============================================================================

/// Test that key derivation is deterministic (same signing key → same AES key).
#[test]
fn test_key_derivation_deterministic() {
    let (signing, _) = generate_ed25519_keypair();
    let key1 = derive_aes_storage_key(&signing);
    let key2 = derive_aes_storage_key(&signing);
    assert_eq!(key1, key2, "same signing key must produce same AES key");
}

/// Test that different signing keys produce different AES keys.
#[test]
fn test_key_derivation_different_keys() {
    let (signing1, _) = generate_ed25519_keypair();
    let (signing2, _) = generate_ed25519_keypair();
    let key1 = derive_aes_storage_key(&signing1);
    let key2 = derive_aes_storage_key(&signing2);
    assert_ne!(key1, key2);
}

/// Test that AES storage key differs from keys derived with other contexts.
#[test]
fn test_key_derivation_context_separation() {
    let (signing, _) = generate_ed25519_keypair();
    let aes_key = derive_aes_storage_key(&signing);
    let other_key = derive_encryption_key(&signing, "carnelian-memory-encryption-v1");
    assert_ne!(aes_key, other_key);
}

// =============================================================================
// MEMORY ENCRYPTION TESTS
// =============================================================================

/// Test memory creation with encrypted content and unencrypted embedding.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_memory_encrypted_content_unencrypted_embedding() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    // Create an identity for the memory
    let identity_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO identities (identity_id, name, identity_type, pronouns) VALUES ($1, 'test', 'core', 'they/them')",
    )
    .bind(identity_id)
    .execute(&pool)
    .await
    .expect("insert identity");

    let (signing, _) = generate_ed25519_keypair();
    let helper = EncryptionHelper::new(&pool, &signing);

    let manager = MemoryManager::new(pool.clone(), None).with_encryption(helper.clone());

    let content = "User prefers concise responses";
    let memory = manager
        .create_memory(
            identity_id,
            content,
            Some("Communication preference".to_string()),
            MemorySource::Conversation,
            None,
            0.9,
        )
        .await
        .expect("create memory");

    // Returned content should be decrypted
    assert_eq!(memory.content, content);

    // Verify the raw database content is encrypted (not plaintext)
    let raw: (Vec<u8>,) = sqlx::query_as("SELECT content FROM memories WHERE memory_id = $1")
        .bind(memory.memory_id)
        .fetch_one(&pool)
        .await
        .expect("fetch raw");

    assert_ne!(
        raw.0,
        content.as_bytes(),
        "raw DB content must be encrypted, not plaintext"
    );

    // Verify retrieval decrypts correctly
    let retrieved = manager
        .get_memory(memory.memory_id)
        .await
        .expect("get memory")
        .expect("memory should exist");
    assert_eq!(retrieved.content, content);
}

/// Test that embeddings remain unencrypted for pgvector search.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_embeddings_remain_unencrypted() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let identity_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO identities (identity_id, name, identity_type, pronouns) VALUES ($1, 'test', 'core', 'they/them')",
    )
    .bind(identity_id)
    .execute(&pool)
    .await
    .expect("insert identity");

    let (signing, _) = generate_ed25519_keypair();
    let helper = EncryptionHelper::new(&pool, &signing);

    let manager = MemoryManager::new(pool.clone(), None).with_encryption(helper);

    // Create memory without embedding first, then add embedding
    let memory = manager
        .create_memory(
            identity_id,
            "test content for embedding",
            None,
            MemorySource::Observation,
            None,
            0.5,
        )
        .await
        .expect("create memory");

    let embedding = vec![0.1f32; 1536];
    manager
        .add_embedding_to_memory(memory.memory_id, embedding.clone())
        .await
        .expect("add embedding");

    // Verify embedding is stored as-is (not encrypted) by checking it's queryable via pgvector
    let row_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM memories WHERE memory_id = $1 AND embedding IS NOT NULL",
    )
    .bind(memory.memory_id)
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(row_count, 1, "embedding should be stored and queryable");
}

// =============================================================================
// CONFIG STORE ENCRYPTION TESTS
// =============================================================================

/// Test config_store encryption with encrypted=true flag.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_config_store_encrypted_value() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let (signing, _) = generate_ed25519_keypair();
    let helper = EncryptionHelper::new(&pool, &signing);

    let config_key = "test_secret_key";
    let value = serde_json::json!("super-secret-value");

    // Store encrypted with key_version=1
    carnelian_core::Config::update_config_value_encrypted(
        &pool,
        config_key,
        None,
        &value,
        None,
        None,
        None,
        None,
        Some(&helper),
        1,
    )
    .await
    .expect("store encrypted config");

    // Verify encrypted flag is set and value_blob is populated
    let row: (bool, Option<Vec<u8>>, Option<String>, i32) = sqlx::query_as(
        "SELECT encrypted, value_blob, value_text, key_version FROM config_store WHERE key = $1",
    )
    .bind(config_key)
    .fetch_one(&pool)
    .await
    .expect("fetch config row");

    assert!(row.0, "encrypted flag should be true");
    assert!(row.1.is_some(), "value_blob should be populated");
    assert!(
        row.2.is_none(),
        "value_text should be NULL for encrypted values"
    );
    assert_eq!(row.3, 1, "key_version should be 1");

    // Decrypt the blob and verify it matches
    let decrypted = helper
        .decrypt_text(row.1.as_ref().unwrap())
        .await
        .expect("decrypt config blob");
    let decrypted_value: serde_json::Value = serde_json::from_str(&decrypted).expect("parse JSON");
    assert_eq!(decrypted_value, value);
}

/// Test read_config_value round-trip for encrypted config entries.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_read_config_value_encrypted_roundtrip() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let (signing, _) = generate_ed25519_keypair();
    let helper = EncryptionHelper::new(&pool, &signing);

    let value = serde_json::json!({"secret": "data", "count": 42});

    // Write encrypted
    carnelian_core::Config::update_config_value_encrypted(
        &pool,
        "test_read_encrypted",
        None,
        &value,
        None,
        None,
        None,
        None,
        Some(&helper),
        1,
    )
    .await
    .expect("write encrypted");

    // Read back via read_config_value
    let (read_value, key_version) =
        carnelian_core::Config::read_config_value(&pool, "test_read_encrypted", Some(&helper))
            .await
            .expect("read")
            .expect("key should exist");

    assert_eq!(read_value, value, "round-trip value must match");
    assert_eq!(key_version, 1, "key_version should be 1");
}

/// Test read_config_value for plaintext config entries.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_read_config_value_plaintext_roundtrip() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let value = serde_json::json!({"enabled": true});

    // Write plaintext (no encryption helper)
    carnelian_core::Config::update_config_value(
        &pool,
        "test_read_plaintext",
        None,
        &value,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("write plaintext");

    // Read back without encryption helper
    let (read_value, key_version) =
        carnelian_core::Config::read_config_value(&pool, "test_read_plaintext", None)
            .await
            .expect("read")
            .expect("key should exist");

    assert_eq!(read_value, value, "round-trip value must match");
    assert_eq!(key_version, 1, "default key_version should be 1");
}

/// Test read_config_value returns None for missing keys.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_read_config_value_missing_key() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let result = carnelian_core::Config::read_config_value(&pool, "nonexistent_key_12345", None)
        .await
        .expect("read should not error");

    assert!(result.is_none(), "missing key should return None");
}

/// Test key_version=2 for key rotation scenario.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_config_store_key_version_rotation() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    let (signing1, _) = generate_ed25519_keypair();
    let (signing2, _) = generate_ed25519_keypair();
    let helper1 = EncryptionHelper::new(&pool, &signing1);
    let helper2 = EncryptionHelper::new(&pool, &signing2);

    let config_key = "rotatable_secret";
    let value_v1 = serde_json::json!("secret-v1");
    let value_v2 = serde_json::json!("secret-v2");

    // Write with key_version=1
    carnelian_core::Config::update_config_value_encrypted(
        &pool,
        config_key,
        None,
        &value_v1,
        None,
        None,
        None,
        None,
        Some(&helper1),
        1,
    )
    .await
    .expect("write v1");

    // Verify key_version=1
    let (read_v1, kv1) =
        carnelian_core::Config::read_config_value(&pool, config_key, Some(&helper1))
            .await
            .expect("read v1")
            .expect("should exist");
    assert_eq!(read_v1, value_v1);
    assert_eq!(kv1, 1);

    // Simulate key rotation: re-encrypt with new key at version 2
    carnelian_core::Config::update_config_value_encrypted(
        &pool,
        config_key,
        Some(&value_v1),
        &value_v2,
        None,
        None,
        None,
        None,
        Some(&helper2),
        2,
    )
    .await
    .expect("write v2");

    // Verify key_version=2 and new value
    let (read_v2, kv2) =
        carnelian_core::Config::read_config_value(&pool, config_key, Some(&helper2))
            .await
            .expect("read v2")
            .expect("should exist");
    assert_eq!(read_v2, value_v2);
    assert_eq!(kv2, 2, "key_version should be updated to 2 after rotation");

    // Old key should fail to decrypt
    let old_read =
        carnelian_core::Config::read_config_value(&pool, config_key, Some(&helper1)).await;
    assert!(
        old_read.is_err(),
        "old key should fail to decrypt rotated value"
    );
}

// =============================================================================
// RUN LOGS SENSITIVE FLAG TESTS
// =============================================================================

/// Test run_logs with sensitive=true flag.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Docker - run with: cargo test --test encryption_tests -- --ignored"]
async fn test_run_logs_sensitive_flag() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;
    let pool = setup_test_db(&db_url).await;

    // Create prerequisite task and run
    let task_id = Uuid::new_v4();
    let identity_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO identities (identity_id, name, identity_type, pronouns) VALUES ($1, 'test', 'core', 'they/them')",
    )
    .bind(identity_id)
    .execute(&pool)
    .await
    .expect("insert identity");

    sqlx::query(
        "INSERT INTO tasks (task_id, identity_id, description, status, priority) VALUES ($1, $2, 'test', 'pending', 'medium')",
    )
    .bind(task_id)
    .bind(identity_id)
    .execute(&pool)
    .await
    .expect("insert task");

    let run_id = Uuid::new_v4();
    sqlx::query("INSERT INTO task_runs (run_id, task_id, status) VALUES ($1, $2, 'running')")
        .bind(run_id)
        .bind(task_id)
        .execute(&pool)
        .await
        .expect("insert run");

    let (signing, _) = generate_ed25519_keypair();
    let helper = EncryptionHelper::new(&pool, &signing);

    // Insert a sensitive log entry
    let secret_message = "API key: sk-1234567890";
    insert_run_log(&pool, run_id, "info", secret_message, true, Some(&helper))
        .await
        .expect("insert sensitive log");

    // Insert a non-sensitive log entry
    let normal_message = "Task started";
    insert_run_log(&pool, run_id, "info", normal_message, false, None)
        .await
        .expect("insert normal log");

    // Verify sensitive flag in database
    let rows: Vec<(Vec<u8>, bool)> = sqlx::query_as(
        "SELECT message, sensitive FROM run_logs WHERE run_id = $1 ORDER BY log_id ASC",
    )
    .bind(run_id)
    .fetch_all(&pool)
    .await
    .expect("fetch logs");

    assert_eq!(rows.len(), 2);

    // First row: sensitive=true, message is encrypted
    assert!(rows[0].1, "first log should be sensitive");
    assert_ne!(
        rows[0].0,
        secret_message.as_bytes(),
        "sensitive message must be encrypted in DB"
    );

    // Second row: sensitive=false, message is raw UTF-8
    assert!(!rows[1].1, "second log should not be sensitive");
    assert_eq!(
        String::from_utf8(rows[1].0.clone()).unwrap(),
        normal_message,
        "non-sensitive message should be plain UTF-8"
    );

    // Decrypt the sensitive message and verify
    let decrypted = helper
        .decrypt_text(&rows[0].0)
        .await
        .expect("decrypt sensitive log");
    assert_eq!(decrypted, secret_message);
}
