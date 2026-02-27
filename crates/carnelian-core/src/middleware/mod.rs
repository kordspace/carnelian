//! Middleware modules for CARNELIAN
//!
//! This module contains middleware for:
//! - Rate limiting
//! - CORS configuration
//! - Security headers
//! - Input validation

pub mod rate_limit;
pub mod cors;
pub mod security_headers;
pub mod input_validation;

pub use rate_limit::{RateLimiter, rate_limit_middleware};
pub use cors::{create_cors_layer, create_production_cors, create_development_cors, CorsConfig};
pub use security_headers::{security_headers_middleware, SecurityHeadersConfig, FrameOptions};
pub use input_validation::{input_validation_middleware, ValidationConfig, sanitize_string, sanitize_json};
