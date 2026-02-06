# Integration Tests

## Prerequisites

- **Docker** must be running (PostgreSQL containers are spun up via `testcontainers`)
- Rust toolchain matching `rust-version` in `Cargo.toml`

## Quick Tests (no Docker required)

```bash
cargo test --package carnelian-core
```

## CLI Tests

Validates `carnelian migrate`, `start`, `stop`, `status`, `logs --follow`, global flags, and error handling.

```bash
cargo test --test cli_integration_test -- --ignored
```

## Migration Tests

Verifies Phase 1 delta schema (sessions, skill_versions, workflows, sub_agents, XP, elixirs), seed data, schema fixes (pronouns, subject_id TEXT, subject_type enum, LZ4 compression), and migration idempotency.

```bash
cargo test --test migration_test -- --ignored
```

## Server Tests

Includes WebSocket load tests (10k events/min, bounded memory, multi-client broadcast), capability grants, and LZ4 compression verification.

```bash
# All server integration tests
cargo test --test server_integration_test -- --ignored

# Individual tests
cargo test --test server_integration_test test_websocket_load -- --ignored
cargo test --test server_integration_test test_capability_grants -- --ignored
cargo test --test server_integration_test test_lz4_compression -- --ignored
```

## All Integration Tests

```bash
cargo test --workspace -- --ignored
```
