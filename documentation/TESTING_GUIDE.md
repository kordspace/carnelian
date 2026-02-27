# CARNELIAN Testing Guide

Complete guide for running tests, benchmarks, and load tests for CARNELIAN.

---

## Table of Contents

- [Quick Start](#quick-start)
- [Unit Tests](#unit-tests)
- [Integration Tests](#integration-tests)
- [Performance Benchmarks](#performance-benchmarks)
- [Load Testing](#load-testing)
- [Test Coverage](#test-coverage)
- [CI/CD Integration](#cicd-integration)

---

## Quick Start

### Prerequisites

```bash
# Install Rust and Cargo (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install k6 for load testing
# macOS
brew install k6

# Linux
sudo apt-key adv --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
echo "deb https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
sudo apt-get update
sudo apt-get install k6

# Set up test database
export TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/carnelian_test"
```

### Run All Tests

```bash
# Run all unit and integration tests
cargo test --all

# Run with output
cargo test --all -- --nocapture

# Run specific test
cargo test test_create_memory
```

---

## Unit Tests

Unit tests cover individual components in isolation.

### Memory Layer Tests

```bash
# Run all memory tests
cargo test --test lib memory_tests

# Run specific memory test
cargo test test_vector_similarity_search
```

**Coverage:**
- Memory creation and retrieval
- Vector similarity search
- Memory updates and deletion
- Tag-based filtering
- Pagination
- Identity-based queries

### Skill Execution Tests

```bash
# Run all skill execution tests
cargo test --test lib skill_execution_tests
```

**Coverage:**
- WASM runtime creation
- Skill loading
- Skill input/output validation
- Capability enforcement
- Timeout configuration

### XP System Tests

```bash
# Run all XP system tests
cargo test --test lib xp_system_tests
```

**Coverage:**
- XP award and tracking
- Leaderboard generation
- XP history queries
- Source-based filtering
- Pagination

---

## Integration Tests

Integration tests verify interactions between components.

### API Integration Tests

```bash
# Run all API tests
cargo test --test lib api_tests

# Run with logging
RUST_LOG=debug cargo test --test lib api_tests -- --nocapture
```

**Coverage:**
- Health endpoint
- Skill execution API
- Memory CRUD operations
- XP award API
- Workflow execution
- Authentication/authorization
- CORS headers
- Error responses

### Database Integration Tests

```bash
# Run all database tests
cargo test --test lib database_tests
```

**Coverage:**
- Connection pool management
- Transaction commit/rollback
- Concurrent operations
- Vector similarity queries
- Foreign key constraints
- Cascade deletes
- Index usage

---

## Performance Benchmarks

Benchmarks measure performance of critical operations.

### Memory Benchmarks

```bash
# Run memory benchmarks
cargo bench --bench memory_benchmarks

# Run specific benchmark
cargo bench --bench memory_benchmarks vector_search
```

**Benchmarks:**
- Vector similarity search (10, 50, 100 results)
- Memory creation
- Memory listing with pagination
- Concurrent memory creation

### Skill Benchmarks

```bash
# Run skill benchmarks
cargo bench --bench skill_benchmarks
```

**Benchmarks:**
- WASM runtime creation
- Skill input serialization
- Skill input deserialization

### Viewing Benchmark Results

```bash
# Results are saved to target/criterion/
# View HTML report
open target/criterion/report/index.html
```

---

## Load Testing

Load tests simulate real-world traffic patterns.

### Running Load Tests

```bash
# Start CARNELIAN server
docker-compose up -d

# Run load test
k6 run tests/performance/load_test.js

# Run with custom parameters
k6 run --vus 100 --duration 5m tests/performance/load_test.js

# Run with environment variables
BASE_URL=http://localhost:8080 API_KEY=your_key k6 run tests/performance/load_test.js
```

### Load Test Stages

1. **Warm up** (30s): Ramp to 10 users
2. **Load test** (1m): Ramp to 50 users
3. **Sustained load** (2m): Stay at 50 users
4. **Stress test** (1m): Ramp to 100 users
5. **Peak load** (1m): Stay at 100 users
6. **Cool down** (30s): Ramp down to 0

### Performance Thresholds

| Metric | Threshold |
|--------|-----------|
| HTTP request duration (p95) | < 500ms |
| HTTP request failure rate | < 1% |
| Error rate | < 5% |
| Skill execution (p95) | < 1000ms |
| Memory query (p95) | < 200ms |

### Viewing Load Test Results

```bash
# k6 outputs results to console
# For detailed analysis, use k6 Cloud or Grafana

# Export results to JSON
k6 run --out json=results.json tests/performance/load_test.js

# Send metrics to InfluxDB
k6 run --out influxdb=http://localhost:8086/k6 tests/performance/load_test.js
```

---

## Test Coverage

### Generating Coverage Reports

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage

# View report
open coverage/index.html
```

### Coverage Targets

| Component | Target | Current |
|-----------|--------|---------|
| Memory Layer | 80% | ⏳ TBD |
| Skill Execution | 80% | ⏳ TBD |
| XP System | 80% | ⏳ TBD |
| API Endpoints | 80% | ⏳ TBD |
| Database Layer | 70% | ⏳ TBD |
| **Overall** | **80%** | **⏳ TBD** |

---

## CI/CD Integration

### GitHub Actions

Tests run automatically on:
- Push to `main` branch
- Pull requests
- Nightly builds

```yaml
# .github/workflows/test.yml
name: Tests

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: cargo test --all
      - name: Run benchmarks
        run: cargo bench --no-run
```

### Pre-commit Hooks

```bash
# Install pre-commit hooks
cp scripts/pre-commit .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

---

## Test Organization

```
tests/
├── lib.rs                      # Test suite entry point
├── helpers/
│   └── mod.rs                  # Test utilities and helpers
├── unit/
│   ├── mod.rs
│   ├── memory_tests.rs         # Memory layer unit tests
│   ├── skill_execution_tests.rs # Skill execution unit tests
│   └── xp_system_tests.rs      # XP system unit tests
├── integration/
│   ├── mod.rs
│   ├── api_tests.rs            # API integration tests
│   └── database_tests.rs       # Database integration tests
└── performance/
    └── load_test.js            # k6 load testing script

benches/
├── memory_benchmarks.rs        # Memory performance benchmarks
└── skill_benchmarks.rs         # Skill performance benchmarks
```

---

## Best Practices

### Writing Tests

1. **Use descriptive names**: `test_vector_similarity_search_returns_closest_matches`
2. **Follow AAA pattern**: Arrange, Act, Assert
3. **Clean up after tests**: Use `cleanup_test_db()` helper
4. **Use test helpers**: Leverage `test_uuid()`, `test_embedding()`, etc.
5. **Test edge cases**: Empty inputs, large datasets, concurrent access

### Running Tests Efficiently

```bash
# Run tests in parallel (default)
cargo test

# Run tests sequentially (for debugging)
cargo test -- --test-threads=1

# Run only fast tests
cargo test --lib

# Skip slow integration tests
cargo test --lib --bins

# Watch mode (requires cargo-watch)
cargo watch -x test
```

### Debugging Failed Tests

```bash
# Run with full output
cargo test -- --nocapture

# Run with logging
RUST_LOG=debug cargo test -- --nocapture

# Run specific test with backtrace
RUST_BACKTRACE=1 cargo test test_name -- --nocapture

# Run with GDB
rust-gdb --args target/debug/deps/test_binary test_name
```

---

## Troubleshooting

### Database Connection Issues

```bash
# Ensure PostgreSQL is running
docker-compose up -d postgres

# Check connection
psql $TEST_DATABASE_URL -c "SELECT 1"

# Reset test database
dropdb carnelian_test
createdb carnelian_test
```

### WASM Skill Tests

```bash
# Build test WASM skills
cd skills/test-skill
cargo build --target wasm32-wasi --release
cp target/wasm32-wasi/release/test_skill.wasm ../../tests/fixtures/
```

### Load Test Failures

```bash
# Check server is running
curl http://localhost:8080/health

# Verify API key
export API_KEY=your_actual_key

# Reduce load
k6 run --vus 10 --duration 1m tests/performance/load_test.js
```

---

## Next Steps

1. **Achieve 80% test coverage** across all components
2. **Add E2E tests** with Playwright
3. **Set up continuous benchmarking** to track performance over time
4. **Integrate with Grafana** for real-time test metrics
5. **Add property-based testing** with proptest

---

## Resources

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [k6 Documentation](https://k6.io/docs/)
- [Criterion.rs Guide](https://bheisler.github.io/criterion.rs/book/)
- [CARNELIAN Implementation Roadmap](IMPLEMENTATION_ROADMAP.md)
- [CARNELIAN Pre-Deployment Review](PRE_DEPLOYMENT_REVIEW.md)

---

**Last Updated:** 2026-02-26  
**Status:** ✅ Test infrastructure complete, ready for execution
