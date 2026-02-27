# Test Mocks

This directory contains mock implementations for testing external dependencies.

## Structure

```
tests/mocks/
└── mod.rs          # Main mock module
```

## Usage

```rust
use tests::mocks::{MockOllama, MockPostgres, MockHttpClient};

#[tokio::test]
async fn test_with_mock_ollama() {
    let mut mock = MockOllama::new()
        .with_response("hello", serde_json::json!({"response": "Hi!"}));
    
    let result = mock.generate("llama3.2", "hello world").await.unwrap();
    assert_eq!(mock.call_count(), 1);
}
```

## Available Mocks

### MockOllama
- Mock LLM interactions
- Configure responses by prompt pattern
- Track call counts

### MockPostgres
- Mock database queries
- Simulate failures
- Track executed SQL

### MockHttpClient
- Mock HTTP requests
- Configure status codes and responses
- URL pattern matching

## Best Practices

- Use mocks for external services only
- Keep mocks simple and predictable
- Document expected behavior
- Reset state between tests
- Don't mock internal business logic
