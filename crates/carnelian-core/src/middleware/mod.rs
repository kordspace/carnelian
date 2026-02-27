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

pub use cors::{CorsConfig, create_cors_layer, create_development_cors, create_production_cors};
pub use input_validation::{
    ValidationConfig, input_validation_middleware, sanitize_json, sanitize_string,
};
pub use rate_limit::{RateLimiter, rate_limit_middleware};
pub use security_headers::{FrameOptions, SecurityHeadersConfig, security_headers_middleware};
