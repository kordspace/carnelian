//! Security headers middleware for CARNELIAN API
//!
//! Implements security best practices through HTTP headers:
//! - Content Security Policy (CSP)
//! - HTTP Strict Transport Security (HSTS)
//! - X-Frame-Options
//! - X-Content-Type-Options
//! - X-XSS-Protection
//! - Referrer-Policy

use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue, header},
    middleware::Next,
    response::Response,
};

/// Security headers configuration
#[derive(Debug, Clone)]
pub struct SecurityHeadersConfig {
    pub enable_hsts: bool,
    pub hsts_max_age: u32,
    pub enable_csp: bool,
    pub csp_policy: String,
    pub frame_options: FrameOptions,
}

#[derive(Debug, Clone)]
pub enum FrameOptions {
    Deny,
    SameOrigin,
    AllowFrom(String),
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            enable_hsts: true,
            hsts_max_age: 31536000, // 1 year
            enable_csp: true,
            csp_policy: "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self' data:; connect-src 'self'".to_string(),
            frame_options: FrameOptions::Deny,
        }
    }
}

impl SecurityHeadersConfig {
    /// Create production security headers configuration
    pub fn production() -> Self {
        Self {
            enable_hsts: true,
            hsts_max_age: 63072000, // 2 years
            enable_csp: true,
            csp_policy: "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' https:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'".to_string(),
            frame_options: FrameOptions::Deny,
        }
    }

    /// Create development security headers configuration (more permissive)
    pub fn development() -> Self {
        Self {
            enable_hsts: false,
            hsts_max_age: 0,
            enable_csp: true,
            csp_policy: "default-src 'self' 'unsafe-inline' 'unsafe-eval'; img-src 'self' data: https:; connect-src 'self' ws: wss:".to_string(),
            frame_options: FrameOptions::SameOrigin,
        }
    }
}

/// Security headers middleware
pub async fn security_headers_middleware(
    config: SecurityHeadersConfig,
    req: Request,
    next: Next,
) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();

    // HSTS - Force HTTPS
    if config.enable_hsts {
        let hsts_value = format!(
            "max-age={}; includeSubDomains; preload",
            config.hsts_max_age
        );
        if let Ok(value) = HeaderValue::from_str(&hsts_value) {
            headers.insert(header::STRICT_TRANSPORT_SECURITY, value);
        }
    }

    // CSP - Prevent XSS and injection attacks
    if config.enable_csp {
        if let Ok(value) = HeaderValue::from_str(&config.csp_policy) {
            headers.insert(header::CONTENT_SECURITY_POLICY, value);
        }
    }

    // X-Frame-Options - Prevent clickjacking
    let frame_value = match config.frame_options {
        FrameOptions::Deny => "DENY",
        FrameOptions::SameOrigin => "SAMEORIGIN",
        FrameOptions::AllowFrom(ref url) => {
            // Note: ALLOW-FROM is deprecated, use CSP frame-ancestors instead
            headers.insert(
                header::X_FRAME_OPTIONS,
                HeaderValue::from_str(&format!("ALLOW-FROM {}", url))
                    .unwrap_or_else(|_| HeaderValue::from_static("DENY")),
            );
            return response;
        }
    };
    headers.insert(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static(frame_value),
    );

    // X-Content-Type-Options - Prevent MIME sniffing
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );

    // X-XSS-Protection - Enable browser XSS protection
    headers.insert(
        header::X_XSS_PROTECTION,
        HeaderValue::from_static("1; mode=block"),
    );

    // Referrer-Policy - Control referrer information
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    // Permissions-Policy - Control browser features
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_config_default() {
        let config = SecurityHeadersConfig::default();
        assert!(config.enable_hsts);
        assert_eq!(config.hsts_max_age, 31536000);
        assert!(config.enable_csp);
    }

    #[test]
    fn test_security_config_production() {
        let config = SecurityHeadersConfig::production();
        assert!(config.enable_hsts);
        assert_eq!(config.hsts_max_age, 63072000);
        assert!(config.csp_policy.contains("frame-ancestors 'none'"));
    }

    #[test]
    fn test_security_config_development() {
        let config = SecurityHeadersConfig::development();
        assert!(!config.enable_hsts);
        assert!(config.csp_policy.contains("unsafe-inline"));
    }

    #[test]
    fn test_frame_options_variants() {
        let deny = FrameOptions::Deny;
        let same_origin = FrameOptions::SameOrigin;
        let allow_from = FrameOptions::AllowFrom("https://example.com".to_string());

        // Just verify they can be created
        assert!(matches!(deny, FrameOptions::Deny));
        assert!(matches!(same_origin, FrameOptions::SameOrigin));
        assert!(matches!(allow_from, FrameOptions::AllowFrom(_)));
    }
}
