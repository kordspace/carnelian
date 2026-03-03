//! Tests for MAGIC config nested/flat schema compatibility

#[cfg(test)]
mod tests {
    use crate::config::{Config, MagicConfig, QuantumOriginConfig};

    #[test]
    fn test_magic_config_nested_toml() {
        let toml = r#"
[magic]
enabled = true
entropy_timeout_ms = 5000
auto_suggest_skills = true

[magic.quantum_origin]
api_key = "test-key-123"
url = "https://test.quantinuum.com"

[magic.quantinuum]
device = "H2-1"

[magic.ibm_quantum]
token = "ibm-token-456"
backend = "ibm_kyoto"

[magic.cors_origins]
extra = ["https://example.com", "https://test.com"]
"#;

        let config: MagicConfig = toml::from_str(toml).expect("Failed to parse nested TOML");
        
        assert!(config.enabled);
        assert_eq!(config.entropy_timeout_ms, 5000);
        assert!(config.auto_suggest_skills);
        
        // Check nested quantum_origin
        let qo = config.quantum_origin.expect("quantum_origin should be Some");
        assert_eq!(qo.api_key, "test-key-123");
        assert_eq!(qo.url, "https://test.quantinuum.com");
        
        // Check nested quantinuum
        let qt = config.quantinuum.expect("quantinuum should be Some");
        assert_eq!(qt.device, "H2-1");
        
        // Check nested ibm_quantum
        let ibm = config.ibm_quantum.expect("ibm_quantum should be Some");
        assert_eq!(ibm.token, "ibm-token-456");
        assert_eq!(ibm.backend, "ibm_kyoto");
        
        // Check nested cors_origins
        let cors = config.cors_origins.expect("cors_origins should be Some");
        assert_eq!(cors.extra.len(), 2);
        assert_eq!(cors.extra[0], "https://example.com");
    }

    #[test]
    fn test_magic_config_flat_toml_backward_compat() {
        let toml = r#"
[magic]
enabled = true
quantum_origin_url = "https://legacy.quantinuum.com"
quantum_origin_api_key = "legacy-key-789"
quantinuum_enabled = true
quantinuum_device = "H1-1E"
qiskit_enabled = true
qiskit_backend = "ibm_brisbane"
entropy_timeout_ms = 3000
log_entropy_events = false
"#;

        let config: MagicConfig = toml::from_str(toml).expect("Failed to parse flat TOML");
        
        assert!(config.enabled);
        assert_eq!(config.quantum_origin_url, "https://legacy.quantinuum.com");
        assert_eq!(config.quantum_origin_api_key, "legacy-key-789");
        assert!(config.quantinuum_enabled);
        assert_eq!(config.quantinuum_device, "H1-1E");
        assert!(config.qiskit_enabled);
        assert_eq!(config.qiskit_backend, "ibm_brisbane");
        assert_eq!(config.entropy_timeout_ms, 3000);
        assert!(!config.log_entropy_events);
    }

    #[test]
    fn test_magic_config_mixed_nested_and_flat() {
        // Nested config takes precedence when both present
        let toml = r#"
[magic]
enabled = true
quantum_origin_url = "https://flat.example.com"
quantum_origin_api_key = "flat-key"

[magic.quantum_origin]
api_key = "nested-key"
url = "https://nested.example.com"
"#;

        let config: MagicConfig = toml::from_str(toml).expect("Failed to parse mixed TOML");
        
        // Nested should be populated
        let qo = config.quantum_origin.expect("quantum_origin should be Some");
        assert_eq!(qo.api_key, "nested-key");
        assert_eq!(qo.url, "https://nested.example.com");
        
        // Flat fields should also be populated (for backward compat)
        assert_eq!(config.quantum_origin_url, "https://flat.example.com");
        assert_eq!(config.quantum_origin_api_key, "flat-key");
    }

    #[test]
    fn test_magic_config_env_var_populates_nested() {
        use std::env;
        
        // Set env var
        env::set_var("CARNELIAN_QUANTUM_ORIGIN_API_KEY", "env-key-123");
        
        let toml = r#"
[magic]
enabled = true

[magic.quantum_origin]
url = "https://env-test.com"
"#;

        let mut config: Config = toml::from_str(&format!(
            r#"
database_url = "postgresql://test"
bind_address = "127.0.0.1"
port = 18789
{}
"#,
            toml
        ))
        .expect("Failed to parse config");
        
        // Apply env vars
        config.apply_env_overrides().expect("Failed to apply env overrides");
        
        // Check that nested config was populated from env var
        let qo = config.magic.quantum_origin.expect("quantum_origin should be Some");
        assert_eq!(qo.api_key, "env-key-123");
        assert_eq!(qo.url, "https://env-test.com");
        
        // Also check flat field for backward compat
        assert_eq!(config.magic.quantum_origin_api_key, "env-key-123");
        
        // Clean up
        env::remove_var("CARNELIAN_QUANTUM_ORIGIN_API_KEY");
    }

    #[test]
    fn test_magic_config_defaults() {
        let config = MagicConfig::default();
        
        assert!(!config.enabled);
        assert_eq!(config.entropy_timeout_ms, 5000);
        assert!(config.auto_suggest_skills);
        assert!(config.quantum_origin.is_none());
        assert!(config.quantinuum.is_none());
        assert!(config.ibm_quantum.is_none());
        assert!(config.cors_origins.is_none());
    }

    #[test]
    fn test_quantum_origin_config_defaults() {
        let config = QuantumOriginConfig::default();
        
        assert_eq!(config.api_key, "");
        assert_eq!(config.url, "https://origin.quantinuum.com");
    }
}
