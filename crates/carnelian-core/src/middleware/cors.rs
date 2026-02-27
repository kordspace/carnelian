//! CORS middleware for CARNELIAN API
//!
//! Configures Cross-Origin Resource Sharing (CORS) policies.
//! Provides different configurations for development and production environments.

use axum::http::{HeaderName, HeaderValue, Method, header};
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};

/// CORS configuration
#[derive(Debug, Clone)]
pub struct CorsConfig {
    pub production: bool,
    pub allowed_origins: Vec<String>,
    pub max_age: Duration,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            production: false,
            allowed_origins: vec!["http://localhost:3000".to_string()],
            max_age: Duration::from_secs(3600),
        }
    }
}

/// Create CORS layer based on configuration
pub fn create_cors_layer(config: CorsConfig) -> CorsLayer {
    if config.production {
        // Strict CORS for production
        let mut layer = CorsLayer::new()
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::PATCH,
                Method::OPTIONS,
            ])
            .allow_headers([
                header::CONTENT_TYPE,
                header::AUTHORIZATION,
                header::ACCEPT,
                HeaderName::from_static("x-carnelian-key"),
            ])
            .max_age(config.max_age);

        // Add allowed origins
        for origin in config.allowed_origins {
            if let Ok(origin_value) = origin.parse::<HeaderValue>() {
                layer = layer.allow_origin(origin_value);
            }
        }

        layer
    } else {
        // Permissive CORS for development
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
            .max_age(config.max_age)
    }
}

/// Create production CORS layer with specific origins
pub fn create_production_cors(allowed_origins: Vec<String>) -> CorsLayer {
    create_cors_layer(CorsConfig {
        production: true,
        allowed_origins,
        max_age: Duration::from_secs(3600),
    })
}

/// Create development CORS layer (permissive)
pub fn create_development_cors() -> CorsLayer {
    create_cors_layer(CorsConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cors_config_default() {
        let config = CorsConfig::default();
        assert!(!config.production);
        assert_eq!(config.allowed_origins.len(), 1);
        assert_eq!(config.max_age, Duration::from_secs(3600));
    }

    #[test]
    fn test_create_development_cors() {
        let layer = create_development_cors();
        // Layer should be created successfully
        assert!(true);
    }

    #[test]
    fn test_create_production_cors() {
        let origins = vec![
            "https://example.com".to_string(),
            "https://app.example.com".to_string(),
        ];
        let layer = create_production_cors(origins);
        // Layer should be created successfully
        assert!(true);
    }

    #[test]
    fn test_cors_config_production() {
        let config = CorsConfig {
            production: true,
            allowed_origins: vec!["https://example.com".to_string()],
            max_age: Duration::from_secs(7200),
        };

        assert!(config.production);
        assert_eq!(config.max_age, Duration::from_secs(7200));
    }
}
