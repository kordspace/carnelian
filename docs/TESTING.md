# CARNELIAN Testing Guide

This guide covers all testing approaches for CARNELIAN, from unit tests to E2E tests.

## Test Coverage Overview

CARNELIAN maintains comprehensive test coverage across multiple layers:

- **Unit Tests**: 262+ tests covering core functionality
- **Integration Tests**: Database, API, and worker integration
- **E2E Tests**: Playwright tests for desktop UI
- **Benchmark Tests**: Performance regression testing

## Running Tests

### Quick Start

```bash
# Run all unit tests
cargo test --workspace

# Run integration tests (requires Docker)
cargo test --workspace -- --ignored

# Run E2E tests
cd tests/e2e && npm test

# Run benchmarks
cargo bench
```

### Using Make

```bash
# Run all tests
make test

# Run only unit tests
make test-unit

# Run only integration tests
make test-integration

# Run E2E tests
make test-e2e

# Generate coverage report
make test-coverage
```

## Unit Tests

Unit tests are located in each crate's `src/` directory and test individual modules.

```bash
# Run all unit tests
cargo test --lib --workspace

# Run tests for specific crate
cargo test --package carnelian-core --lib

# Run specific test
cargo test --package carnelian-core --lib test_name

# Run with output
cargo test --lib -- --nocapture
```

### Test Organization

```
crates/carnelian-core/src/
├── scheduler.rs          # Scheduler tests
├── policy.rs             # Policy engine tests
├── ledger.rs             # Ledger tests
├── worker.rs             # Worker manager tests
├── memory.rs             # Memory manager tests
└── session.rs            # Session tests
```

## Integration Tests

Integration tests require Docker for PostgreSQL and are marked with `#[ignore]`.

### Setup

```bash
# Start Docker services
docker-compose up -d

# Wait for services to be ready
docker-compose ps
```

### Running Integration Tests

```bash
# Run all integration tests
cargo test --workspace -- --ignored

# Run specific integration test
cargo test --test production_validation_test -- --ignored

# Run with logging
RUST_LOG=debug cargo test --workspace -- --ignored --nocapture
```

### Integration Test Suites

| Test Suite | Description | Docker Required |
|------------|-------------|-----------------|
| `production_validation_test.rs` | Production readiness validation | Yes |
| `cli_integration_test.rs` | CLI command validation | Yes |
| `migration_test.rs` | Database migration tests | Yes |
| `scheduler_integration_test.rs` | Scheduler integration | Yes |
| `server_integration_test.rs` | HTTP API integration | Yes |
| `worker_transport_test.rs` | Worker communication | Yes |

## E2E Tests

End-to-end tests use Playwright to test the Dioxus desktop UI.

### Setup

```bash
cd tests/e2e
npm install
npx playwright install
```

### Running E2E Tests

```bash
# Run all E2E tests
npm test

# Run in headed mode (see browser)
npm run test:headed

# Run in debug mode
npm run test:debug

# Run in UI mode
npm run test:ui

# Run specific browser
npm run test:chromium
npm run test:firefox
npm run test:webkit
```

### E2E Test Structure

```
tests/e2e/
├── tests/
│   ├── dioxus-ui.spec.ts      # UI component tests
│   ├── api-integration.spec.ts # API integration tests
│   └── workflow.spec.ts        # Workflow tests
├── playwright.config.ts        # Playwright configuration
└── package.json                # Dependencies
```

## Coverage Reports

Generate test coverage reports using `tarpaulin`:

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate HTML coverage report
cargo tarpaulin --out Html --output-dir coverage

# Open report
open coverage/tarpaulin-report.html  # macOS
xdg-open coverage/tarpaulin-report.html  # Linux
start coverage/tarpaulin-report.html  # Windows
```

### Coverage Targets

- **Core modules**: >80% coverage
- **Critical paths**: >90% coverage
- **Integration tests**: All major workflows covered

## Benchmarks

Performance benchmarks ensure no regressions:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench skill_benchmarks

# Compare with baseline
cargo bench -- --save-baseline main
cargo bench -- --baseline main
```

### Benchmark Suites

- `skill_benchmarks.rs` - Skill execution performance
- `scheduler_benchmarks.rs` - Task scheduling performance
- `memory_benchmarks.rs` - Memory retrieval performance

## CI/CD Testing

### GitHub Actions

Tests run automatically on:
- Push to `main`
- Pull requests
- Scheduled nightly builds

### CI Test Matrix

```yaml
Strategy:
  - OS: ubuntu-latest, windows-latest, macos-latest
  - Rust: stable, beta
  - Features: default, full
```

### Local CI Checks

Run the same checks as CI locally:

```bash
# Quick checks (no Docker)
./scripts/ci-local.sh

# Full checks (with Docker)
./scripts/ci-local.sh --full
```

## Test Data

### Test Database

Integration tests use `testcontainers` to spin up PostgreSQL:

```rust
let container = create_postgres_container()
    .start()
    .await
    .expect("Failed to start container");
```

### Fixtures

Test fixtures are in `tests/fixtures/`:

```
tests/fixtures/
├── skills/          # Test skill manifests
├── configs/         # Test configurations
└── data/            # Test data files
```

## Writing Tests

### Unit Test Example

```rust
#[test]
fn test_scheduler_priority() {
    let scheduler = Scheduler::new();
    let task = Task::new("test", Priority::High);
    scheduler.enqueue(task);
    assert_eq!(scheduler.next().unwrap().priority, Priority::High);
}
```

### Integration Test Example

```rust
#[tokio::test]
#[ignore = "requires docker"]
async fn test_database_migration() {
    let container = create_postgres_container().start().await?;
    let pool = setup_test_db(&container).await?;
    
    // Test migrations
    let result = sqlx::query("SELECT 1").fetch_one(&pool).await;
    assert!(result.is_ok());
}
```

### E2E Test Example

```typescript
test('should create task', async ({ page }) => {
  await page.goto('http://localhost:8080/tasks');
  await page.click('button:has-text("New Task")');
  await page.fill('input[name="title"]', 'Test Task');
  await page.click('button:has-text("Create")');
  await expect(page.locator('text=Test Task')).toBeVisible();
});
```

## Test Best Practices

### General Guidelines

1. **Isolation**: Tests should not depend on each other
2. **Cleanup**: Always clean up resources (use `Drop` trait)
3. **Determinism**: Tests should be deterministic (no random data)
4. **Speed**: Keep unit tests fast (<100ms)
5. **Documentation**: Document complex test scenarios

### Naming Conventions

```rust
// Unit tests
#[test]
fn test_<module>_<scenario>() { }

// Integration tests
#[tokio::test]
#[ignore = "requires docker"]
async fn test_<feature>_<scenario>() { }
```

### Test Attributes

```rust
#[test]                           // Unit test
#[tokio::test]                    // Async test
#[ignore]                         // Skip by default
#[ignore = "reason"]              // Skip with reason
#[should_panic]                   // Expect panic
#[should_panic(expected = "msg")] // Expect specific panic
```

## Debugging Tests

### Enable Logging

```bash
RUST_LOG=debug cargo test -- --nocapture
```

### Run Single Test

```bash
cargo test test_name -- --exact --nocapture
```

### Debug with GDB/LLDB

```bash
rust-gdb target/debug/deps/test_binary
rust-lldb target/debug/deps/test_binary
```

## Continuous Testing

### Watch Mode

```bash
# Install cargo-watch
cargo install cargo-watch

# Run tests on file changes
cargo watch -x test
```

### Pre-commit Hooks

Tests run automatically via pre-commit hooks:

```bash
# Install hooks
./scripts/setup-hooks.sh

# Run manually
prek run --all-files
```

## Troubleshooting

### Common Issues

**Issue**: Tests fail with "database connection refused"
**Solution**: Ensure Docker is running: `docker-compose up -d`

**Issue**: E2E tests timeout
**Solution**: Increase timeout in `playwright.config.ts`

**Issue**: Coverage report incomplete
**Solution**: Run with `--all-features` flag

**Issue**: Flaky tests
**Solution**: Add proper wait conditions, avoid timing assumptions

## Resources

- [Rust Testing Documentation](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Playwright Documentation](https://playwright.dev/)
- [Testcontainers Documentation](https://github.com/testcontainers/testcontainers-rs)
- [Tarpaulin Documentation](https://github.com/xd009642/tarpaulin)

## Test Metrics

Current test metrics (updated automatically):

- **Total Tests**: 262+ unit tests, 61 integration tests
- **Coverage**: ~75% (target: 80%)
- **CI Pass Rate**: >95%
- **Average Test Duration**: <5 minutes (full suite)
