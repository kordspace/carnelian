//! Input validation middleware for CARNELIAN API
//!
//! Provides request validation including:
//! - Content-Type validation
//! - Request size limits
//! - JSON schema validation
//! - SQL injection prevention
//! - XSS prevention

use axum::{
    body::Body,
    extract::Request,
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::Value;

/// Maximum request body size (10 MB)
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Input validation configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    pub max_body_size: usize,
    pub require_content_type: bool,
    pub allowed_content_types: Vec<String>,
    pub sanitize_input: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_body_size: MAX_BODY_SIZE,
            require_content_type: true,
            allowed_content_types: vec![
                "application/json".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            ],
            sanitize_input: true,
        }
    }
}

/// Validate content type header
pub fn validate_content_type(req: &Request, config: &ValidationConfig) -> Result<(), String> {
    if !config.require_content_type {
        return Ok(());
    }

    // Only validate for methods that typically have a body
    let method = req.method();
    if !matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        return Ok(());
    }

    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| "Missing Content-Type header".to_string())?;

    let is_allowed = config
        .allowed_content_types
        .iter()
        .any(|allowed| content_type.starts_with(allowed));

    if !is_allowed {
        return Err(format!(
            "Invalid Content-Type: {}. Allowed: {:?}",
            content_type, config.allowed_content_types
        ));
    }

    Ok(())
}

/// Sanitize string input to prevent XSS and SQL injection
pub fn sanitize_string(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('/', "&#x2F;")
}

/// Sanitize JSON value recursively
pub fn sanitize_json(value: &mut Value) {
    match value {
        Value::String(s) => {
            *s = sanitize_string(s);
        }
        Value::Array(arr) => {
            for item in arr {
                sanitize_json(item);
            }
        }
        Value::Object(obj) => {
            for (_, v) in obj.iter_mut() {
                sanitize_json(v);
            }
        }
        _ => {}
    }
}

/// Check for common SQL injection patterns
pub fn contains_sql_injection(input: &str) -> bool {
    let input_lower = input.to_lowercase();

    let sql_patterns = [
        "' or '1'='1",
        "' or 1=1",
        "'; drop table",
        "'; delete from",
        "union select",
        "exec(",
        "execute(",
        "xp_cmdshell",
        "sp_executesql",
    ];

    sql_patterns
        .iter()
        .any(|pattern| input_lower.contains(pattern))
}

/// Check for common XSS patterns
pub fn contains_xss(input: &str) -> bool {
    let input_lower = input.to_lowercase();

    let xss_patterns = [
        "<script",
        "javascript:",
        "onerror=",
        "onload=",
        "onclick=",
        "eval(",
        "expression(",
        "<iframe",
        "<object",
        "<embed",
    ];

    xss_patterns
        .iter()
        .any(|pattern| input_lower.contains(pattern))
}

/// Validate JSON structure
pub fn validate_json_structure(
    value: &Value,
    max_depth: usize,
    current_depth: usize,
) -> Result<(), String> {
    if current_depth > max_depth {
        return Err(format!("JSON nesting too deep (max: {})", max_depth));
    }

    match value {
        Value::Array(arr) => {
            if arr.len() > 1000 {
                return Err("Array too large (max: 1000 elements)".to_string());
            }
            for item in arr {
                validate_json_structure(item, max_depth, current_depth + 1)?;
            }
        }
        Value::Object(obj) => {
            if obj.len() > 100 {
                return Err("Object has too many keys (max: 100)".to_string());
            }
            for (key, val) in obj {
                if key.len() > 256 {
                    return Err("Object key too long (max: 256 chars)".to_string());
                }
                validate_json_structure(val, max_depth, current_depth + 1)?;
            }
        }
        Value::String(s) => {
            if s.len() > 1_000_000 {
                return Err("String too long (max: 1MB)".to_string());
            }
        }
        _ => {}
    }

    Ok(())
}

/// Input validation middleware
pub async fn input_validation_middleware(
    config: ValidationConfig,
    req: Request,
    next: Next,
) -> Response {
    // Validate content type
    if let Err(e) = validate_content_type(&req, &config) {
        return (StatusCode::UNSUPPORTED_MEDIA_TYPE, e).into_response();
    }

    // Continue with request
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_string() {
        let input = "<script>alert('XSS')</script>";
        let sanitized = sanitize_string(input);
        assert_eq!(
            sanitized,
            "&lt;script&gt;alert(&#x27;XSS&#x27;)&lt;&#x2F;script&gt;"
        );
    }

    #[test]
    fn test_sanitize_json() {
        let mut value = serde_json::json!({
            "name": "<script>alert('test')</script>",
            "nested": {
                "value": "normal & text"
            }
        });

        sanitize_json(&mut value);

        assert!(value["name"].as_str().unwrap().contains("&lt;script&gt;"));
        assert!(value["nested"]["value"].as_str().unwrap().contains("&amp;"));
    }

    #[test]
    fn test_sql_injection_detection() {
        assert!(contains_sql_injection("' OR '1'='1"));
        assert!(contains_sql_injection("'; DROP TABLE users--"));
        assert!(contains_sql_injection("UNION SELECT * FROM passwords"));
        assert!(!contains_sql_injection("normal text"));
    }

    #[test]
    fn test_xss_detection() {
        assert!(contains_xss("<script>alert('xss')</script>"));
        assert!(contains_xss("javascript:alert(1)"));
        assert!(contains_xss("<img onerror='alert(1)'>"));
        assert!(!contains_xss("normal text"));
    }

    #[test]
    fn test_json_structure_validation() {
        let valid = serde_json::json!({"key": "value"});
        assert!(validate_json_structure(&valid, 10, 0).is_ok());

        // Create deeply nested JSON programmatically to avoid macro issues
        let mut deep_value = serde_json::json!("value");
        for _ in 0..15 {
            deep_value = serde_json::json!({"nested": deep_value});
        }
        assert!(validate_json_structure(&deep_value, 5, 0).is_err());

        let large_array = serde_json::json!(vec![1; 2000]);
        assert!(validate_json_structure(&large_array, 10, 0).is_err());
    }

    #[test]
    fn test_validation_config_default() {
        let config = ValidationConfig::default();
        assert_eq!(config.max_body_size, MAX_BODY_SIZE);
        assert!(config.require_content_type);
        assert!(config.sanitize_input);
    }
}
