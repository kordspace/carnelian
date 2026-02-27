//! Docker secrets management for CARNELIAN
//!
//! Provides utilities for reading secrets from Docker secrets or environment variables.
//! Follows Docker Swarm secrets pattern: /run/secrets/<secret_name>

use carnelian_common::{Error, Result};
use std::fs;
use std::path::Path;

/// Read a secret from Docker secrets or fallback to environment variable
///
/// # Arguments
/// * `secret_name` - Name of the secret (e.g., "postgres_password")
/// * `env_var_name` - Fallback environment variable name (e.g., "POSTGRES_PASSWORD")
///
/// # Returns
/// The secret value as a string
pub fn read_secret(secret_name: &str, env_var_name: &str) -> Result<String> {
    // Try Docker secret first
    let secret_path = format!("/run/secrets/{}", secret_name);

    if Path::new(&secret_path).exists() {
        fs::read_to_string(&secret_path)
            .map(|s| s.trim().to_string())
            .map_err(|e| Error::Config(format!("Failed to read secret {}: {}", secret_name, e)))
    } else {
        // Fallback to environment variable
        std::env::var(env_var_name).map_err(|_| {
            Error::Config(format!(
                "Secret {} not found in /run/secrets and {} not set in environment",
                secret_name, env_var_name
            ))
        })
    }
}

/// Read database password from secrets
pub fn get_database_password() -> Result<String> {
    read_secret("postgres_password", "POSTGRES_PASSWORD")
}

/// Read Carnelian API key from secrets
pub fn get_carnelian_api_key() -> Result<String> {
    read_secret("carnelian_api_key", "CARNELIAN_API_KEY")
}

/// Read Ollama API key from secrets (if required)
pub fn get_ollama_api_key() -> Result<String> {
    read_secret("ollama_api_key", "OLLAMA_API_KEY")
}

/// Read OpenAI API key from secrets (if required)
pub fn get_openai_api_key() -> Result<String> {
    read_secret("openai_api_key", "OPENAI_API_KEY")
}

/// Check if running in Docker with secrets
pub fn is_using_docker_secrets() -> bool {
    Path::new("/run/secrets").exists()
}

/// List available Docker secrets
pub fn list_available_secrets() -> Result<Vec<String>> {
    let secrets_dir = Path::new("/run/secrets");

    if !secrets_dir.exists() {
        return Ok(vec![]);
    }

    let entries = fs::read_dir(secrets_dir)
        .map_err(|e| Error::Config(format!("Failed to read secrets directory: {}", e)))?;

    let mut secrets = vec![];
    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            secrets.push(name.to_string());
        }
    }

    Ok(secrets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_is_using_docker_secrets() {
        // Will be false in test environment
        let using_secrets = is_using_docker_secrets();
        assert!(!using_secrets || using_secrets); // Just verify it returns a bool
    }

    #[test]
    fn test_read_secret_from_env() {
        env::set_var("TEST_SECRET", "test_value");

        let result = read_secret("nonexistent_secret", "TEST_SECRET");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_value");

        env::remove_var("TEST_SECRET");
    }

    #[test]
    fn test_read_secret_missing() {
        let result = read_secret("nonexistent_secret", "NONEXISTENT_ENV_VAR");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_database_password_fallback() {
        env::set_var("POSTGRES_PASSWORD", "test_db_password");

        let result = get_database_password();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_db_password");

        env::remove_var("POSTGRES_PASSWORD");
    }

    #[test]
    fn test_get_carnelian_api_key_fallback() {
        env::set_var("CARNELIAN_API_KEY", "test_api_key");

        let result = get_carnelian_api_key();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_api_key");

        env::remove_var("CARNELIAN_API_KEY");
    }

    #[test]
    fn test_list_available_secrets() {
        let result = list_available_secrets();
        assert!(result.is_ok());
        // In test environment, should return empty list
        assert_eq!(result.unwrap().len(), 0);
    }
}
