#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::doc_markdown)]

//! Integration tests for configuration loading
//!
//! Tests cover:
//! - Loading configuration from TOML files
//! - Ed25519 keypair loading (PEM and raw formats)
//! - Sign/verify operations
//!
//! Note: Environment variable override tests are skipped in Rust 2024 edition
//! because env::set_var/remove_var are unsafe. The override logic is tested
//! via unit tests in config.rs.

use carnelian_core::config::{Config, MachineConfig, MachineProfile};
use std::path::PathBuf;

/// Get the path to test fixtures directory
fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn test_load_from_toml_file() {
    let config_path = fixtures_path().join("machine.toml");
    let config = Config::load_from_file(&config_path).expect("Failed to load config from TOML");

    assert_eq!(config.machine_profile, MachineProfile::Urim);
    assert_eq!(config.http_port, 19000);
    assert_eq!(
        config.database_url,
        "postgresql://test:test@localhost:5432/test_db"
    );
    assert_eq!(config.ollama_url, "http://localhost:11434");
    assert_eq!(config.log_level, "DEBUG");
    assert_eq!(config.db_max_connections, 5);
    assert_eq!(config.db_connection_timeout_secs, 15);
    assert_eq!(config.db_idle_timeout_secs, 300);
}

// Environment variable override tests removed - env::set_var is unsafe in Rust 2024.
// The override logic is tested via unit tests in config.rs.

#[test]
fn test_load_pem_keypair() {
    let key_path = fixtures_path().join("test_key.pem");

    let mut config = Config::default();
    config.owner_keypair_path = Some(key_path);

    config
        .load_owner_keypair()
        .expect("Failed to load PEM keypair");

    assert!(config.has_owner_keypair());
    assert!(config.owner_public_key.is_some());

    // Verify the public key is hex-encoded (64 chars for 32 bytes)
    let public_key = config.owner_public_key.as_ref().unwrap();
    assert_eq!(public_key.len(), 64);
}

#[test]
fn test_load_raw_keypair() {
    let key_path = fixtures_path().join("test_key_raw.bin");

    let mut config = Config::default();
    config.owner_keypair_path = Some(key_path);

    config
        .load_owner_keypair()
        .expect("Failed to load raw keypair");

    assert!(config.has_owner_keypair());
    assert!(config.owner_public_key.is_some());
}

#[test]
fn test_sign_and_verify() {
    let key_path = fixtures_path().join("test_key.pem");

    let mut config = Config::default();
    config.owner_keypair_path = Some(key_path);
    config.load_owner_keypair().expect("Failed to load keypair");

    let message = b"Hello, Carnelian OS!";

    // Sign the message
    let signature = config
        .sign_message(message)
        .expect("Failed to sign message");

    // Verify the signature
    let is_valid = config
        .verify_signature(message, &signature)
        .expect("Failed to verify");
    assert!(is_valid);

    // Verify with wrong message fails
    let wrong_message = b"Wrong message";
    let is_valid = config
        .verify_signature(wrong_message, &signature)
        .expect("Failed to verify");
    assert!(!is_valid);
}

#[test]
fn test_machine_config_profiles() {
    // Test Thummim profile (default)
    let config = Config::default();
    let machine = config.machine_config();
    assert_eq!(machine.max_workers, 4);
    assert_eq!(machine.max_memory_mb, 28672);
    assert!(machine.gpu_enabled);
    assert_eq!(machine.default_model, "deepseek-r1:7b");

    // Test Urim profile
    let mut config = Config::default();
    config.machine_profile = MachineProfile::Urim;
    let machine = config.machine_config();
    assert_eq!(machine.max_workers, 8);
    assert_eq!(machine.max_memory_mb, 57344);
    assert!(machine.gpu_enabled);
    assert_eq!(machine.default_model, "deepseek-r1:32b");

    // Test Custom profile with custom config
    let custom = MachineConfig {
        max_workers: 16,
        max_memory_mb: 131072,
        gpu_enabled: true,
        default_model: "llama3:70b".to_string(),
        auto_restart_workers: true,
    };
    let mut config = Config::default();
    config.machine_profile = MachineProfile::Custom;
    config.custom_machine_config = Some(custom);
    let machine = config.machine_config();
    assert_eq!(machine.max_workers, 16);
    assert_eq!(machine.max_memory_mb, 131072);
}

#[test]
fn test_validation_passes_for_valid_config() {
    let config = Config::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_validation_fails_for_invalid_port() {
    let mut config = Config::default();
    config.http_port = 80; // Below 1024
    assert!(config.validate().is_err());
}

#[test]
fn test_validation_fails_for_invalid_database_url() {
    let mut config = Config::default();
    config.database_url = "mysql://invalid".to_string();
    assert!(config.validate().is_err());
}

#[test]
fn test_validation_fails_for_invalid_log_level() {
    let mut config = Config::default();
    config.log_level = "VERBOSE".to_string();
    assert!(config.validate().is_err());
}

#[test]
fn test_missing_keypair_file_does_not_error() {
    let mut config = Config::default();
    config.owner_keypair_path = Some(PathBuf::from("/nonexistent/path/key.pem"));

    // Should not error, just warn
    assert!(config.load_owner_keypair().is_ok());
    assert!(!config.has_owner_keypair());
}

#[test]
fn test_sign_without_keypair_errors() {
    let config = Config::default();
    assert!(!config.has_owner_keypair());

    let result = config.sign_message(b"test");
    assert!(result.is_err());
}
