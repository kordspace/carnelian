# Integration Tests

## Prerequisites

- **Docker** must be running (PostgreSQL containers are spun up via `testcontainers`)
- Rust toolchain matching `rust-version` in `Cargo.toml`

## Quick Tests (no Docker required)

```bash
cargo test --package carnelian-core
```

## Test Suites

### CLI Tests (7 tests)

Validates `carnelian migrate`, `start`, `stop`, `status`, `logs --follow`, global flags, and error handling.

```bash
cargo test --test cli_integration_test -- --ignored
```

### Integration Tests (7 tests)

Core infrastructure tests: database connection/reconnection, server startup, heartbeat timing, migration seed data, and load handling (10k events/min).

```bash
cargo test --test integration_test -- --ignored
```

### Migration Tests (12 tests)

Verifies Phase 1 delta schema (sessions, skill_versions, workflows, sub_agents, XP, elixirs), seed data, schema fixes (pronouns, subject_id TEXT, subject_type enum, LZ4 compression), and migration idempotency.

```bash
cargo test --test migration_test -- --ignored
```

### Scheduler Tests (7 tests)

Validates task queue polling, priority-based dequeuing, concurrency limits, retry policies, task cancellation, metrics tracking, and concurrency controls.

```bash
cargo test --test scheduler_integration_test -- --ignored
```

### Server Tests (8 tests)

HTTP API and WebSocket tests: task lifecycle endpoints, skill management, run/log pagination, capability grants with text subject IDs, LZ4 compression verification, WebSocket load (10k events/min), bounded memory, and multi-client broadcast.

```bash
cargo test --test server_integration_test -- --ignored
```

### Worker Transport Tests (7 tests)

Validates the JSONL-based worker transport protocol: invoke/success flow, health checks, event streaming, output truncation, cancellation, timeout enforcement, and worker manager integration.

```bash
cargo test --test worker_transport_tests -- --ignored
```

### Config Tests (11 tests, no Docker)

Unit tests for configuration loading, validation, and machine profile resolution.

```bash
cargo test --test config_tests
```

### Logging Tests (11 tests, no Docker)

Unit tests for structured logging conventions, log level filtering, and output formatting.

```bash
cargo test --test logging_test
```

## All Integration Tests

```bash
cargo test --workspace -- --ignored
```

## Local CI Check

Run the local CI script to catch issues before pushing:

```bash
# Quick checks (fmt, clippy, unit tests, doc-tests)
./scripts/ci-local.sh

# Full checks including integration tests (requires Docker)
./scripts/ci-local.sh --full
```
