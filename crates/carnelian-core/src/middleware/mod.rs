//! Middleware modules for CARNELIAN
//!
//! This module contains middleware for:
//! - Rate limiting
//! - CORS configuration
//! - Security headers
//! - Input validation

pub mod cors;
pub mod input_validation;
pub mod rate_limit;
pub mod security_headers;

pub use cors::{create_cors_layer, create_development_cors, create_production_cors, CorsConfig};
pub use input_validation::{
    input_validation_middleware, sanitize_json, sanitize_string, ValidationConfig,
};
pub use rate_limit::{rate_limit_middleware, RateLimiter};
pub use security_headers::{security_headers_middleware, FrameOptions, SecurityHeadersConfig};
