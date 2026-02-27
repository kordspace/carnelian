//! Mock implementations for external services
//!
//! This module provides mock implementations of external services
//! for testing without making real API calls.

use std::collections::HashMap;
use serde_json::Value;

/// Mock Ollama server for testing LLM interactions
pub struct MockOllama {
    responses: HashMap<String, Value>,
    call_count: usize,
}

impl MockOllama {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            call_count: 0,
        }
    }

    /// Pre-configure a mock response for a specific prompt pattern
    pub fn with_response(mut self, pattern: &str, response: Value) -> Self {
        self.responses.insert(pattern.to_string(), response);
        self
    }

    /// Mock generate method
    pub async fn generate(&mut self, model: &str, prompt: &str) -> Result<Value, String> {
        self.call_count += 1;
        
        // Find matching response
        for (pattern, response) in &self.responses {
            if prompt.contains(pattern) {
                return Ok(response.clone());
            }
        }

        // Default mock response
        let default_response = serde_json::json!({
            "model": model,
            "response": "Mock response for: ".to_string() + prompt,
            "done": true
        });

        Ok(default_response)
    }

    pub fn call_count(&self) -> usize {
        self.call_count
    }
}

/// Mock PostgreSQL for database testing
pub struct MockPostgres {
    queries: Vec<String>,
    should_fail: bool,
}

impl MockPostgres {
    pub fn new() -> Self {
        Self {
            queries: Vec::new(),
            should_fail: false,
        }
    }

    pub fn will_fail(mut self) -> Self {
        self.should_fail = true;
        self
    }

    pub async fn query(&mut self, sql: &str) -> Result<Vec<Value>, String> {
        self.queries.push(sql.to_string());
        
        if self.should_fail {
            return Err("Mock database error".to_string());
        }

        Ok(vec![])
    }

    pub fn queries(&self) -> &[String] {
        &self.queries
    }
}

/// Mock HTTP client for external API testing
pub struct MockHttpClient {
    responses: HashMap<String, (u16, Value)>,
}

impl MockHttpClient {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    /// Configure response for a URL pattern
    pub fn with_response(mut self, url_pattern: &str, status: u16, body: Value) -> Self {
        self.responses.insert(url_pattern.to_string(), (status, body));
        self
    }

    pub async fn get(&self, url: &str) -> Result<(u16, Value), String> {
        for (pattern, (status, body)) in &self.responses {
            if url.contains(pattern) {
                return Ok((*status, body.clone()));
            }
        }

        Ok((200, serde_json::json!({"message": "Mock response"})))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_ollama() {
        let mut ollama = MockOllama::new()
            .with_response("hello", serde_json::json!({"response": "Hi there!" }));

        let response = ollama.generate("llama3.2", "hello world").await.unwrap();
        assert!(response.get("response").unwrap().as_str().unwrap().contains("Hi there!"));
    }

    #[tokio::test]
    async fn test_mock_postgres() {
        let mut db = MockPostgres::new();
        let result = db.query("SELECT * FROM test").await;
        assert!(result.is_ok());
        assert_eq!(db.queries().len(), 1);
    }
}
