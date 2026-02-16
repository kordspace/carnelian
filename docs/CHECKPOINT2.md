# Checkpoint 2 Validation Guide

This guide covers validating the 8 criteria for Checkpoint 2 of 🔥 Carnelian OS. It includes step-by-step manual instructions, expected results, a performance baseline table, 24-hour autonomous validation, and guidance for recording the demo video.

## Prerequisites

- **Docker services running** — `docker-compose ps` shows `carnelian-postgres` and `carnelian-ollama` as healthy
- **Carnelian built** — `cargo build --bin carnelian`
- **Database migrated** — `carnelian migrate`
- **Model downloaded** — `docker exec carnelian-ollama ollama pull deepseek-r1:7b`
- **Minimum 10GB free disk space** — for 24-hour log files
- **Clean database state** — fresh migration recommended for baseline metrics

## Quick Start

```bash
# Automated 24-hour validation
./scripts/checkpoint2-validation.sh

# With options
./scripts/checkpoint2-validation.sh --skip-build         # Skip cargo build
./scripts/checkpoint2-validation.sh --duration 3600       # 1-hour test run
./scripts/checkpoint2-validation.sh --keep-running        # Leave server running after validation
./scripts/checkpoint2-validation.sh --dry-run             # Print steps without executing

# Run integration tests (requires Docker)
cargo test --test checkpoint2_validation_test -- --ignored --test-threads=1

# Standalone metrics collection
./scripts/collect-metrics.sh --pid <SERVER_PID> --output logs/metrics.jsonl --loop 300
```

## Validation Workflow

```mermaid
flowchart TD
    Start([Start Validation]) --> Docker{Docker Running?}
    Docker -->|No| StartDocker[Start Docker Services]
    Docker -->|Yes| DiskCheck{10GB Free?}
    StartDocker --> DiskCheck
    DiskCheck -->|No| Abort[Abort - Free Disk Space]
    DiskCheck -->|Yes| Build[Build Carnelian]
    Build --> Migrate[Run Migrations]
    Migrate --> StartServer[Start Server with Log Rotation]
    StartServer --> C1[Criterion 1: Identity Sync]
    C1 --> C2[Criterion 2: Heartbeat Execution]
    C2 --> C3[Criterion 3: Workspace Auto-Queue]
    C3 --> C4[Criterion 4: Model Integration]
    C4 --> C5[Criterion 5: Agentic Loop Stability]
    C5 --> C6[Criterion 6: Memory Management]
    C6 --> C7[Criterion 7: Security]
    C7 --> C8[Criterion 8: Performance]
    C8 --> Report[Generate Report]
    Report --> End([Validation Complete])
```

---

## Criterion 1: Identity Synchronization

**Objective:** Validate SOUL.md changes sync to database within 5 seconds.

### Manual Steps

```bash
# Check current identity
curl http://localhost:18789/v1/identity | jq

# View soul content
curl http://localhost:18789/v1/identity/soul | jq

# Edit SOUL.md (update name, pronouns, or core_values)
# Wait 5 seconds, then re-query
curl http://localhost:18789/v1/identity | jq

# Verify in database
psql postgresql://carnelian:carnelian@localhost:5432/carnelian \
  -c "SELECT identity_id, name, soul_file_hash, updated_at FROM identities ORDER BY updated_at DESC LIMIT 1;"
```

### Expected Results

- Identity endpoint returns current name, pronouns, core_values
- After SOUL.md edit, `updated_at` changes within 5 seconds
- `soul_file_hash` updates to reflect new content
- `SoulUpdated` event emitted on the event stream

### Automated Test

```bash
cargo test --test checkpoint2_validation_test test_criterion_1_identity_sync -- --ignored
```

### Troubleshooting

| Issue | Solution |
|-------|----------|
| Identity not found | Ensure SOUL.md exists in configured `souls_path` |
| Hash not updating | Check file watcher: `carnelian logs -f --event-type SoulUpdated` |
| Sync delay > 5s | Verify file watcher is running, check for I/O contention |

---

## Criterion 2: Heartbeat Execution

**Objective:** Validate heartbeat runs every ~9.26 minutes for 24 hours.

### Manual Steps

```bash
# Monitor heartbeat events in real-time
carnelian logs -f --event-type HeartbeatTick

# Check heartbeat history via API
curl http://localhost:18789/v1/heartbeats | jq

# Check heartbeat status
curl http://localhost:18789/v1/heartbeats/status | jq

# Query heartbeat count from database
psql postgresql://carnelian:carnelian@localhost:5432/carnelian \
  -c "SELECT COUNT(*), MIN(created_at), MAX(created_at) FROM heartbeat_history;"

# Check mantra diversity
psql postgresql://carnelian:carnelian@localhost:5432/carnelian \
  -c "SELECT mantra, COUNT(*) FROM heartbeat_history GROUP BY mantra ORDER BY COUNT(*) DESC;"
```

### Expected Results

- ~155 heartbeats in 24 hours (at 9.26 min intervals)
- No duplicate mantras in consecutive heartbeats
- Each heartbeat has a non-empty `mantra` field
- `HeartbeatTick` and `HeartbeatOk` events emitted per cycle
- Heartbeat records persisted in `heartbeat_history` table

### Automated Test

```bash
cargo test --test checkpoint2_validation_test test_criterion_2_heartbeat_execution -- --ignored
```

### Troubleshooting

| Issue | Solution |
|-------|----------|
| No heartbeats | Verify scheduler started: `curl http://localhost:18789/v1/health/detailed` |
| Heartbeat fails | Check Ollama connectivity: `curl http://localhost:11434/api/tags` |
| Interval drift | Check system load, verify `heartbeat_interval_ms` in config |

---

## Criterion 3: Workspace Auto-Queueing

**Objective:** Validate that `TASK:` and `TODO:` markers are automatically discovered and queued during heartbeat.

### Manual Steps

1. Create test file with markers:
   ```bash
   echo "// TODO: Test safe task" > test.rs
   echo "// TASK: Deploy to production" >> test.rs
   ```

2. Configure workspace scanning:
   ```toml
   # machine.toml
   max_tasks_per_heartbeat = 5
   workspace_scan_paths = ["."]
   ```

3. Trigger heartbeat:
   ```bash
   # Wait for next heartbeat or restart server
   carnelian start
   ```

4. Verify tasks queued:
   ```bash
   curl http://localhost:18789/v1/tasks | jq '.[] | select(.title | contains("TODO"))'
   ```

### Expected Results

- Safe task (`Test safe task`) appears in task queue
- Privileged task (`Deploy to production`) is NOT queued
- `TaskAutoQueued` event emitted with file path and line number
- Task title format: `[TODO] test.rs:1`

### Automated Tests

Run the unit tests (no Docker required):

```bash
cargo test --package carnelian-core --lib scheduler::tests
```

Expected: 15+ tests pass, including:
- `test_classify_safe_tasks`
- `test_classify_privileged_tasks`
- `test_strip_comment_prefix`
- `test_scan_temp_directory`
- `test_scan_respects_limit`
- `test_scan_skips_excluded_dirs`
- `test_scan_empty_description_skipped`
- `test_scan_empty_workspace_paths`
- `test_scan_nonexistent_path`
- `test_scan_zero_limit`
- `test_scan_unicode_markers`
- `test_scan_binary_extension_skipped`
- `test_scan_multiple_paths`

Run the integration test (requires Docker for PostgreSQL):

```bash
cargo test --test checkpoint2_validation_test test_criterion_3_workspace_auto_queue -- --ignored
```

### Configuration Reference

| Setting | Default | Env Variable | Description |
|---------|---------|-------------|-------------|
| `max_tasks_per_heartbeat` | `5` | `CARNELIAN_MAX_TASKS_PER_HEARTBEAT` | Max tasks auto-queued per heartbeat (0 = disabled) |
| `workspace_scan_paths` | `["."]` | `CARNELIAN_WORKSPACE_SCAN_PATHS` | Comma-separated paths to scan |

### Privileged Keywords

Tasks containing any of these keywords are classified as privileged and **not** auto-queued:

`delete`, `drop`, `migrate`, `deploy`, `production`, `credential`, `secret`, `key rotation`, `sudo`, `admin`, `root`, `destroy`, `truncate`, `revert`, `rollback`, `security`, `permission`, `privilege`, `password`, `token`, `api_key`, `private_key`, `certificate`, `encryption`, `decrypt`

### Scannable File Extensions

`rs`, `py`, `ts`, `js`, `tsx`, `jsx`, `go`, `java`, `c`, `cpp`, `h`, `hpp`, `rb`, `sh`, `bash`, `zsh`, `toml`, `yaml`, `yml`, `json`, `md`, `txt`

### Excluded Directories

`target`, `node_modules`, `.git`, `__pycache__`, `dist`, `build`, `vendor`, `.*` (hidden)

### Troubleshooting

| Issue | Solution |
|-------|----------|
| No tasks queued | Check `max_tasks_per_heartbeat > 0`, verify file is in scan path |
| Privileged task queued | Review `PRIVILEGED_KEYWORDS` in `scheduler.rs` |
| Duplicate tasks | Deduplication should prevent this; check database state |
| Scan too slow | Reduce `workspace_scan_paths` scope or lower `max_tasks_per_heartbeat` |
| File not scanned | Verify extension is in `SCANNABLE_EXTENSIONS` and file < 256 KB |

---

## Criterion 4: Model Integration

**Objective:** Validate Ollama model calls during heartbeat.

### Manual Steps

```bash
# Check Ollama connectivity
curl http://localhost:18789/v1/providers/ollama/status | jq

# List providers
curl http://localhost:18789/v1/providers | jq

# Verify model availability
docker exec carnelian-ollama ollama list

# Check heartbeat records for model responses
psql postgresql://carnelian:carnelian@localhost:5432/carnelian \
  -c "SELECT heartbeat_id, mantra, status, reason FROM heartbeat_history ORDER BY created_at DESC LIMIT 5;"
```

### Expected Results

- Ollama status shows `"connected": true`
- At least one model listed (e.g., `deepseek-r1:7b`)
- Each heartbeat has a non-empty `reason` field (model reflection)
- No `model_unavailable` errors in heartbeat history

### Automated Test

```bash
cargo test --test checkpoint2_validation_test test_criterion_4_model_integration -- --ignored
```

### Troubleshooting

| Issue | Solution |
|-------|----------|
| Ollama not connected | Verify container: `docker inspect carnelian-ollama` |
| No models available | Download model: `docker exec carnelian-ollama ollama pull deepseek-r1:7b` |
| Model timeout | Check GPU availability, increase timeout in config |

---

## Criterion 5: Agentic Loop Stability

**Objective:** Validate 24-hour continuous operation without crashes.

### Manual Steps

```bash
# Start 24-hour validation
./scripts/checkpoint2-validation.sh --duration 86400

# Or monitor manually
carnelian start
# In another terminal:
./scripts/collect-metrics.sh --pid $(cat ~/.carnelian/carnelian.pid) --output logs/metrics.jsonl --loop 300

# Check for errors
carnelian logs -f --level ERROR

# Monitor detailed health
watch -n 60 'curl -s http://localhost:18789/v1/health/detailed | jq'
```

### Expected Results

- 100% uptime (no crashes)
- No memory leaks (RSS growth < 100MB over 24h)
- No deadlocks (all health checks pass)
- Heartbeat → workspace scan → auto-queue cycle runs continuously
- `TaskAutoQueued` events reference heartbeat `correlation_id`

### Automated Test

```bash
cargo test --test checkpoint2_validation_test test_criterion_5_agentic_loop -- --ignored
```

### Metrics Monitoring

The validation script collects metrics every 5 minutes to `logs/metrics-24h.jsonl`. Monitor:
- `memory_rss_kb` — should be stable, not monotonically increasing
- `health.status` — should always be `"healthy"`
- `heartbeats_24h` — should increase by ~1 every 9.26 minutes

### Troubleshooting

| Issue | Solution |
|-------|----------|
| Server crashes | Check `logs/carnelian-24h.err`, verify Ollama connectivity |
| Heartbeat stops | Check scheduler logs, verify model availability |
| Memory leak | Monitor RSS growth, check event stream buffer size |
| Auto-queue not working | Verify `workspace_scan_paths` in `machine.toml` |

---

## Criterion 6: Memory Management

**Objective:** Validate memory API endpoints and storage.

### Manual Steps

```bash
# Create a memory
curl -X POST http://localhost:18789/v1/memories \
  -H "Content-Type: application/json" \
  -d '{"identity_id":"<IDENTITY_UUID>","content":"User prefers concise responses","source":"observation","importance":0.85}'

# List memories
curl "http://localhost:18789/v1/memories?identity_id=<IDENTITY_UUID>" | jq

# Filter by source
curl "http://localhost:18789/v1/memories?identity_id=<IDENTITY_UUID>&source=observation" | jq

# Filter by importance
curl "http://localhost:18789/v1/memories?identity_id=<IDENTITY_UUID>&min_importance=0.5" | jq

# Get specific memory
curl http://localhost:18789/v1/memories/<MEMORY_UUID> | jq
```

### Expected Results

- Memories created with 201 status
- `MemoryCreated` event emitted with correct `memory_id` and `identity_id`
- Memories retrievable by ID, filterable by source and importance
- Invalid source returns 400
- Invalid importance (outside 0.0–1.0) returns 400
- Non-existent memory returns 404
- Memories persisted in database with embeddings

### Automated Test

```bash
cargo test --test checkpoint2_validation_test test_criterion_6_memory_management -- --ignored
```

### Troubleshooting

| Issue | Solution |
|-------|----------|
| 400 on valid source | Valid sources: `observation`, `conversation`, `task`, `reflection` |
| No embeddings | Check pgvector extension: `SELECT * FROM pg_extension WHERE extname = 'vector';` |
| Memory not found | Verify identity_id exists in identities table |

---

## Criterion 7: Security & Approval Queue

**Objective:** Validate privileged tasks skip auto-queue.

### Manual Steps

```bash
# Create workspace with privileged markers
echo "// TASK: Delete production database" > test_priv.rs
echo "// TASK: Implement safe feature" >> test_priv.rs

# Wait for heartbeat scan, then check tasks
curl http://localhost:18789/v1/tasks | jq

# Verify privileged task was NOT queued
psql postgresql://carnelian:carnelian@localhost:5432/carnelian \
  -c "SELECT title, state FROM tasks WHERE title LIKE '%production%';"
```

### Expected Results

- Safe tasks (`Implement safe feature`) are queued
- Privileged tasks (`Delete production database`) are NOT queued
- All 24 privileged keywords are enforced (see list below)
- Privileged markers are logged but skipped

### Automated Test

```bash
cargo test --test checkpoint2_validation_test test_criterion_7_security -- --ignored
```

### Privileged Keywords (24)

`delete`, `drop`, `migrate`, `deploy`, `production`, `credential`, `secret`, `key rotation`, `sudo`, `admin`, `root`, `destroy`, `truncate`, `revert`, `rollback`, `security`, `permission`, `privilege`, `password`, `token`, `api_key`, `private_key`, `certificate`, `encryption`, `decrypt`

### Troubleshooting

| Issue | Solution |
|-------|----------|
| Privileged task queued | Review `PRIVILEGED_KEYWORDS` in `scheduler.rs` |
| Safe task not queued | Check `max_tasks_per_heartbeat > 0` |

---

## Criterion 8: Performance Baseline

**Objective:** Measure 24-hour performance metrics.

### Manual Steps

```bash
# Run the performance baseline test
cargo test --test checkpoint2_validation_test test_criterion_8_performance -- --ignored --nocapture

# Or collect metrics during 24-hour run
./scripts/collect-metrics.sh --pid <PID> --output logs/metrics.jsonl
```

### Performance Baseline Metrics

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| Heartbeat Interval (avg) | 9.26 min ± 30s | ___ | ⬜ |
| Heartbeat Success Rate | > 99% | ___ | ⬜ |
| Task Auto-Queue Latency | < 10s | ___ | ⬜ |
| Memory Growth (24h) | < 100MB | ___ | ⬜ |
| Event Throughput | > 10 events/sec | ___ | ⬜ |
| API Response Time (P95) | < 500ms | ___ | ⬜ |
| Task Creation (P99) | < 2s | ___ | ⬜ |
| Memory API (P99) | < 500ms | ___ | ⬜ |

Fill in the **Actual** column from the test output or `logs/checkpoint2-report.md` and mark **Status** as ✅ (pass) or ❌ (fail).

### Automated Test

```bash
cargo test --test checkpoint2_validation_test test_criterion_8_performance -- --ignored
```

---

## Validation Checklist

- [ ] **Criterion 1: Identity Synchronization**
  - [ ] SOUL.md changes reflected in DB within 5 seconds
  - [ ] `SoulUpdated` event emitted
  - [ ] `soul_file_hash` updates correctly

- [ ] **Criterion 2: Heartbeat Execution**
  - [ ] ~155 heartbeats in 24 hours
  - [ ] No duplicate mantras in sequence
  - [ ] Heartbeat records persisted in `heartbeat_history`

- [ ] **Criterion 3: Workspace Auto-Queueing**
  - [ ] Safe tasks queued from `TASK:` / `TODO:` markers
  - [ ] Privileged tasks skipped
  - [ ] `TaskAutoQueued` events emitted with `correlation_id`

- [ ] **Criterion 4: Model Integration**
  - [ ] Ollama connected and models listed
  - [ ] All heartbeats have non-empty reflections
  - [ ] Provider status endpoint returns correct data

- [ ] **Criterion 5: Agentic Loop Stability**
  - [ ] No crashes, 100% uptime
  - [ ] No memory leaks (RSS stable)
  - [ ] Heartbeat → scan → auto-queue cycle verified

- [ ] **Criterion 6: Memory Management**
  - [ ] Memory CRUD via REST API
  - [ ] `MemoryCreated` events emitted with correct IDs
  - [ ] Filtering by source and importance works
  - [ ] Validation errors return 400

- [ ] **Criterion 7: Security & Approval Queue**
  - [ ] All 24 privileged keywords enforced
  - [ ] Privileged tasks not auto-queued
  - [ ] Safe tasks queued normally

- [ ] **Criterion 8: Performance Baseline**
  - [ ] All metrics within targets
  - [ ] Metrics documented in table above

---

## Log Rotation Configuration

Carnelian supports file-based logging with daily rotation via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `CARNELIAN_LOG_FILE` | (none — stdout only) | Path to log file; enables file + stdout dual output |
| `CARNELIAN_LOG_MAX_FILES` | `5` | Maximum number of rotated log files to keep |

Example:

```bash
CARNELIAN_LOG_FILE=logs/carnelian.log CARNELIAN_LOG_MAX_FILES=10 carnelian start
```

When `CARNELIAN_LOG_FILE` is set, logs are written in JSON format to the rotating file and in pretty format to stdout simultaneously.

---

## Architecture

```mermaid
sequenceDiagram
    participant Script as Validation Script
    participant Server as Carnelian Server
    participant DB as PostgreSQL
    participant Ollama as Ollama
    participant FS as Filesystem

    Script->>Server: Start with log rotation
    loop Every 9.26 minutes
        Server->>FS: Scan workspace for markers
        Server->>DB: Auto-queue safe tasks
        Server->>Ollama: Request reflection
        Ollama-->>Server: Return reflection
        Server->>DB: Store heartbeat
    end
    loop Every 60 seconds
        Script->>Server: GET /v1/health
        Server-->>Script: Health status
    end
    loop Every 5 minutes
        Script->>Server: GET /v1/metrics
        Server-->>Script: Metrics snapshot
        Script->>FS: Append to metrics-24h.jsonl
    end
    Note over Script: After 24 hours
    Script->>Server: SIGTERM (graceful shutdown)
    Script->>FS: Generate summary report
```

```
Heartbeat Cycle
  └─ run_heartbeat()
       ├─ Agentic planning (model call)
       ├─ Persist heartbeat to DB
       ├─ Emit HeartbeatTick / HeartbeatOk events
       └─ Workspace Scan & Auto-Queue
            ├─ WorkspaceScanner::scan(paths, limit)
            │    ├─ Walk directories (skip excluded)
            │    ├─ Filter by extension
            │    ├─ Parse TASK:/TODO: markers
            │    └─ Classify safe vs privileged
            └─ auto_queue_scanned_tasks()
                 ├─ Skip privileged markers
                 ├─ Deduplicate by title
                 ├─ INSERT INTO tasks
                 └─ Emit TaskAutoQueued event
```

---

## Known Limitations

- No approval queue UI (CLI-only approval workflow)
- No encryption-at-rest for task results (planned for Phase 3)
- No distributed tracing (single-node only)
- Worker attestation not fully implemented
- Memory compression strategy is basic (no LLM-based summarization yet)
- UI panels for heartbeat/identity/providers are basic (no real-time charts)
- Performance metrics vary by machine profile (Thummim vs Urim)
- GPU availability affects model inference speed
- On Windows, worker signal handling (SIGTERM) behaves differently than Linux/macOS

---

## Troubleshooting

### Common Issues

| Issue | Solution |
|-------|----------|
| Server crashes | Check `logs/carnelian-24h.err`, verify Ollama connectivity |
| Heartbeat stops | Check scheduler logs, verify model availability |
| Memory leak | Monitor RSS growth, check event stream buffer |
| Auto-queue not working | Verify `workspace_scan_paths` in `machine.toml` |
| Privileged task queued | Review `PRIVILEGED_KEYWORDS` in `scheduler.rs` |
| Identity not syncing | Check file watcher, verify `souls_path` in config |
| Ollama not connected | Verify container: `docker inspect carnelian-ollama` |
| Performance tests fail | Ensure Docker has sufficient resources, close other applications |

### Debug Commands

```bash
# Check server status
carnelian status

# Detailed health (includes uptime, scheduler state, subscriber count)
curl http://localhost:18789/v1/health/detailed | jq

# View all logs
carnelian logs -f

# View only errors
carnelian logs -f --level ERROR

# View heartbeat events
carnelian logs -f --event-type HeartbeatTick

# Check database connection
psql postgresql://carnelian:carnelian@localhost:5432/carnelian -c "SELECT COUNT(*) FROM heartbeat_history;"

# Verify Docker services
docker-compose ps
docker-compose logs carnelian-postgres
docker-compose logs carnelian-ollama

# Collect a single metrics snapshot
./scripts/collect-metrics.sh
```

---

## References

- **Integration Tests:** `crates/carnelian-core/tests/checkpoint2_validation_test.rs`
- **Validation Script:** `scripts/checkpoint2-validation.sh`
- **Metrics Script:** `scripts/collect-metrics.sh`
- **API Documentation:** [API.md](API.md)
- **Checkpoint 1 Guide:** [CHECKPOINT1.md](CHECKPOINT1.md)
- **CLI Documentation:** [README.md](../README.md)
- **Development Guide:** [DEVELOPMENT.md](DEVELOPMENT.md)
- **Docker Guide:** [DOCKER.md](DOCKER.md)
