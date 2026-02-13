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

### Phase 3 Agentic Engine Tests (40+ tests)

End-to-end tests for the Phase 3 agentic execution pipeline:

- **Soul**: file loading, parsing, DB sync, hash verification
- **Session**: CRUD, messages, expiration, token counters, persistence across restart (drop pool + reconnect)
- **Memory**: creation, 48hr window, high-importance, pgvector similarity search, MemoryQuery builder
- **Context**: P0–P4 loaders (soul directives, memories, task context, conversation history, tool results), priority ordering, token budget enforcement, over-budget pruning cascade (P4→P3), provenance with memory/message/tool IDs
- **Compaction**: full pipeline via `compact_session` (memory flush, summarization, tool pruning, counter recompute, ledger event), `check_and_compact_if_needed` threshold trigger
- **Model Routing**: `ModelRouter::complete` budget exceeded error, within-budget gateway attempt, ledger event emission
- **Heartbeat**: agentic turn pipeline (context assembly, model call, heartbeat_history persistence, ledger event), correlation ID end-to-end
- **Agentic Loop**: session persistence, correlation ID propagation, policy engine capability checks, ledger audit events
- **Cross-Module**: soul→memory→context, session lifecycle, session restart persistence

```bash
# All Phase 3 tests (requires Docker)
cargo test --test phase3_integration_test -- --ignored

# Run specific test group
cargo test --test phase3_integration_test test_soul -- --ignored
cargo test --test phase3_integration_test test_session -- --ignored
cargo test --test phase3_integration_test test_memory -- --ignored
cargo test --test phase3_integration_test test_context -- --ignored
cargo test --test phase3_integration_test test_compaction -- --ignored
cargo test --test phase3_integration_test test_model_router -- --ignored
cargo test --test phase3_integration_test test_heartbeat -- --ignored
cargo test --test phase3_integration_test test_end_to_end -- --ignored

# SessionKey parsing (no Docker)
cargo test --test phase3_integration_test test_session_key_parsing
```

### Skill Discovery Tests (18 tests, mixed)

Manifest validation (no Docker): valid/invalid manifests, runtime validation, checksum determinism, JSON parsing. Database integration (Docker): insert, update, unchanged detection, stale removal, multi-skill scan, invalid manifest skipping, event emission, empty/nonexistent registry, checksum storage, capabilities storage.

```bash
# Unit tests only (no Docker)
cargo test --test skill_discovery_tests

# Integration tests (requires Docker)
cargo test --test skill_discovery_tests -- --ignored
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
