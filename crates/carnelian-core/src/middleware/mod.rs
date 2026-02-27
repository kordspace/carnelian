//! Middleware modules for CARNELIAN
//!
//! This module contains middleware for:
//! - Rate limiting
//! - CORS configuration
//! - Security headers

pub mod rate_limit;
pub mod cors;

pub use rate_limit::{RateLimiter, rate_limit_middleware};
pub use cors::{create_cors_layer, create_production_cors, create_development_cors, CorsConfig};
