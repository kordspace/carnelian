# Test Fixtures

This directory contains test fixtures for CARNELIAN testing.

## Structure

```
tests/fixtures/
├── wasm/           # WASM skill test fixtures
└── data/           # JSON/data test fixtures
```

## Usage

```rust
use std::fs;

// Load WASM fixture
let wasm_bytes = fs::read("tests/fixtures/wasm/sample_skill.wasm").unwrap();

// Load JSON fixture
let json_data = fs::read_to_string("tests/fixtures/data/sample_config.json").unwrap();
let config: Config = serde_json::from_str(&json_data).unwrap();
```

## Fixtures

### WASM Fixtures

- `sample_skill.wasm` - Minimal WASM skill for testing
- `echo_skill.wasm` - Echo skill for testing I/O
- `http_skill.wasm` - Skill with HTTP capability

### Data Fixtures

- `sample_config.json` - Configuration fixture
- `sample_memory.json` - Memory entry fixture
- `sample_workflow.json` - Workflow definition fixture
- `mock_llm_response.json` - Mock LLM responses

## Creating New Fixtures

1. Add files to appropriate subdirectory
2. Keep fixtures minimal and focused
3. Document what each fixture tests
4. Use descriptive names

## Best Practices

- Don't commit large files (>1MB)
- Generate fixtures programmatically when possible
- Use `.gitattributes` for binary files
- Document fixture schema in comments
