# Checkpoint 1 Validation Guide

This guide covers validating the 8 criteria for Checkpoint 1 of 🔥 Carnelian OS. It includes step-by-step manual instructions, expected results, a performance baseline table, and guidance for recording the demo video.

## Prerequisites

- **Docker services running** — `docker-compose ps` shows `carnelian-postgres` and `carnelian-ollama` as healthy
- **Carnelian built** — `cargo build --bin carnelian`
- **Database migrated** — `carnelian migrate`
- **Model downloaded** — see [DOCKER.md](DOCKER.md) model download section

## Quick Start

```bash
# Automated validation (runs all criteria)
./scripts/checkpoint1-validation.sh

# Automated with options
./scripts/checkpoint1-validation.sh --skip-build      # Skip cargo build
./scripts/checkpoint1-validation.sh --keep-running     # Leave server running after validation
./scripts/checkpoint1-validation.sh --clean            # Remove test data after validation

# Manual validation — follow the sections below
```

## Validation Workflow

```mermaid
flowchart TD
    Start([Start Validation]) --> Docker{Docker Running?}
    Docker -->|No| StartDocker[Start Docker Services]
    Docker -->|Yes| Build[Build Carnelian]
    StartDocker --> Build
    Build --> Migrate[Run Migrations]
    Migrate --> StartServer[Start Carnelian Server]
    StartServer --> C1[Criterion 1: System Startup]
    C1 --> C2[Criterion 2: Skill Discovery]
    C2 --> C3[Criterion 3: Task Lifecycle]
    C3 --> C4[Criterion 4: CLI Tasks]
    C4 --> C5[Criterion 5: Concurrency]
    C5 --> C6[Criterion 6: Error Handling]
    C6 --> C7[Criterion 7: UI Responsiveness]
    C7 --> C8[Criterion 8: Performance]
    C8 --> Report[Generate Report]
    Report --> End([Validation Complete])
```

---

## Criterion 1: System Startup

**Objective:** Validate that the full system starts successfully with all components — PostgreSQL, HTTP server, WebSocket, scheduler, and worker manager.

### Manual Steps

```bash
# Start the server
carnelian start

# Verify health
curl http://localhost:18789/v1/health

# Verify status
curl http://localhost:18789/v1/status

# Verify WebSocket (requires wscat: npm install -g wscat)
wscat -c ws://localhost:18789/v1/events/ws

# Check for startup errors
carnelian logs -f --level ERROR
```

### Expected Results

- Health endpoint returns: `{"status":"healthy","database":"connected"}`
- Status endpoint returns system info with scheduler and worker manager state
- WebSocket connection established without errors
- No ERROR-level events in logs

### Troubleshooting

| Issue | Solution |
|-------|----------|
| `carnelian start` fails | Check Docker services: `docker-compose ps`, verify database: `carnelian migrate` |
| WebSocket connection refused | Ensure server is running, check port 18789 is not in use |
| Health returns unhealthy | Check PostgreSQL container: `docker inspect carnelian-postgres` |

---

## Criterion 2: Skill Discovery

**Objective:** Validate that skills are discovered from the registry, stored in the database with checksums, and can be enabled/disabled.

### Manual Steps

```bash
# Trigger skill refresh
carnelian skills refresh

# List skills via API
curl http://localhost:18789/v1/skills | jq

# Disable a skill (replace <skill_id> with actual UUID)
curl -X POST http://localhost:18789/v1/skills/<skill_id>/disable

# Re-enable a skill
curl -X POST http://localhost:18789/v1/skills/<skill_id>/enable
```

### Expected Results

- Skills appear within 5 seconds of refresh
- Each skill has a `checksum` field in the API response
- Disabled skills show `"enabled": false`
- Re-enabled skills show `"enabled": true`

### Troubleshooting

| Issue | Solution |
|-------|----------|
| No skills found | Check `skills/registry/` directory exists and contains `skill.json` files |
| Refresh hangs | Verify database connection: `carnelian status` |

---

## Criterion 3: Task Creation & Execution

**Objective:** Validate the full task lifecycle — creation, state transitions (pending → running → completed), event emission, and result storage.

### Manual Steps

```bash
# Create task via API
curl -X POST http://localhost:18789/v1/tasks \
  -H "Content-Type: application/json" \
  -d '{"title":"Test task","description":"Checkpoint validation"}'

# Monitor task state (replace <task_id> with actual UUID)
curl http://localhost:18789/v1/tasks/<task_id>

# List all tasks
curl http://localhost:18789/v1/tasks

# View task run logs (replace <run_id> with actual UUID)
curl http://localhost:18789/v1/runs/<run_id>/logs
```

### Expected Results

- Task created with `"state": "pending"`
- Task transitions: `pending` → `running` → `completed`
- `TaskStarted` and `TaskCompleted` events emitted on the event stream
- Task run result stored in `task_runs` table

### Troubleshooting

| Issue | Solution |
|-------|----------|
| Task stuck in pending | Verify a worker is running: `curl http://localhost:18789/v1/status` |
| Task fails immediately | Check logs: `carnelian logs -f`, verify skill exists and is enabled |

---

## Criterion 4: CLI Task Creation

**Objective:** Validate the `carnelian task create` CLI command with all parameter combinations.

### Manual Steps

```bash
# Basic creation
carnelian task create "My task"

# With description
carnelian task create "Task" --description "Details about the task"

# With skill and priority
carnelian task create "Task" --skill-id <uuid> --priority 5

# Against a remote server
carnelian task --url http://remote:18789 create "Remote task"
```

### Expected Results

- Task ID returned on stdout
- Task visible in `curl http://localhost:18789/v1/tasks`
- All parameters (title, description, skill_id, priority) stored correctly

### Troubleshooting

| Issue | Solution |
|-------|----------|
| Command not found | Build binary: `cargo build --bin carnelian`, or use `cargo run --bin carnelian --` |
| Connection refused | Ensure server is running: `carnelian start` |

---

## Criterion 5: Concurrent Execution

**Objective:** Validate that multiple tasks execute concurrently up to the configured `max_workers` limit.

### Manual Steps

```bash
# Create 10 tasks rapidly
for i in $(seq 1 10); do
  curl -sf -X POST http://localhost:18789/v1/tasks \
    -H "Content-Type: application/json" \
    -d "{\"title\":\"Concurrent task $i\"}" &
done
wait

# Monitor queue depth
watch -n 1 'curl -s http://localhost:18789/v1/status | jq .queue_depth'

# Or run the integration test
cargo test --test checkpoint1_validation_test test_criterion5 -- --ignored
```

### Expected Results

- Maximum concurrent running tasks equals configured `max_workers` (default: 3)
- All 10 tasks eventually complete
- No tasks lost or stuck

> **Note:** Full concurrent execution requires worker setup. The integration test uses mock workers.

---

## Criterion 6: Error Handling

**Objective:** Validate graceful handling of invalid skills, timeouts, worker crashes, and retry policies.

### Manual Steps

```bash
# 6a: Invalid skill ID
carnelian task create "Bad task" --skill-id "00000000-0000-0000-0000-000000000000"

# 6b: Task cancellation (via API, replace <task_id>)
curl -X POST http://localhost:18789/v1/tasks/<task_id>/cancel

# 6c-6f: Run integration tests for timeout, crash, retry, restart
cargo test --test checkpoint1_validation_test test_criterion6 -- --ignored
```

### Expected Results

- Invalid skill: task fails with clear error message
- Cancellation: task state becomes `canceled`, `TaskCancelled` event emitted
- Timeout: task fails after configured timeout period
- Worker crash: task fails gracefully, no orphaned state
- Retry: failed tasks retry up to `task_max_retry_attempts` before permanent failure

### Troubleshooting

| Issue | Solution |
|-------|----------|
| Task doesn't fail on invalid skill | Verify skill validation in scheduler: check logs |
| Timeout not enforced | Check `skill_timeout_secs` in config |

---

## Criterion 7: UI Responsiveness

**Objective:** Validate that the event stream handles 1000+ events without freezing and updates in real-time.

### Manual Steps

```bash
# Start the UI
cargo run -p carnelian-ui

# Generate events: create tasks, refresh skills, etc.
# Observe real-time updates in the UI

# Or run the integration test
cargo test --test checkpoint1_validation_test test_criterion7 -- --ignored
```

### Expected Results

- UI updates in real-time as events arrive
- No freezing under load (1000+ events)
- Event filters function correctly

> **Note:** UI features are in active development. Some validation may be manual.

---

## Criterion 8: Performance Baseline

**Objective:** Measure and record baseline performance metrics for task creation, event throughput, and API response times.

### Manual Steps

```bash
# Run the performance baseline test
cargo test --test checkpoint1_validation_test test_criterion8_performance_baseline_metrics -- --ignored --nocapture
```

### Performance Baseline Metrics

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| Task Creation (P99) | < 2s | ___ | ⬜ |
| Task Creation (Median) | < 500ms | ___ | ⬜ |
| Event Throughput | > 100 events/sec | ___ | ⬜ |
| Task List API (20 tasks) | < 1s | ___ | ⬜ |
| Skill Discovery | < 5s | ___ | ⬜ |

Fill in the **Actual** column from the test output and mark **Status** as ✅ (pass) or ❌ (fail).

---

## Validation Checklist

- [ ] **Criterion 1: System Startup**
  - [ ] `carnelian start` launches successfully
  - [ ] PostgreSQL connection established
  - [ ] WebSocket endpoint available
  - [ ] No startup errors in logs

- [ ] **Criterion 2: Skill Discovery**
  - [ ] Skills appear within 5s of refresh
  - [ ] Skill manifests validated correctly
  - [ ] Enable/disable functionality works

- [ ] **Criterion 3: Task Creation & Execution**
  - [ ] Tasks created via API
  - [ ] State transitions: pending → running → completed
  - [ ] Real-time logs visible
  - [ ] Results stored correctly

- [ ] **Criterion 4: CLI Task Creation**
  - [ ] `carnelian task create` command works
  - [ ] Task appears in database
  - [ ] All parameters accepted (title, description, skill_id, priority)

- [ ] **Criterion 5: Concurrent Execution**
  - [ ] 10 simultaneous tasks created
  - [ ] Concurrency limit enforced (max_workers)
  - [ ] All tasks complete successfully

- [ ] **Criterion 6: Error Handling**
  - [ ] Invalid skill ID handled gracefully
  - [ ] Timeout scenarios handled
  - [ ] Worker crash recovery works
  - [ ] Clear error messages displayed

- [ ] **Criterion 7: UI Responsiveness**
  - [ ] Event stream handles 1000+ events
  - [ ] Real-time updates work
  - [ ] No UI freezing under load

- [ ] **Criterion 8: Performance Baseline**
  - [ ] Task creation latency measured
  - [ ] Event throughput measured
  - [ ] API response times measured
  - [ ] Metrics documented in table above

---

## Known Limitations

- Worker implementation is in progress — affects concurrent execution tests (Criterion 5)
- UI features are in active development — some UI tests may require manual validation
- Performance metrics vary by machine profile (Thummim vs Urim)
- GPU availability affects model inference speed
- On Windows, worker signal handling (SIGTERM) behaves differently than Linux/macOS

---

## Troubleshooting

### Common Issues

| Issue | Solution |
|-------|----------|
| `carnelian start` fails | Check Docker services: `docker-compose ps`, verify database: `carnelian migrate` |
| WebSocket connection refused | Ensure server is running, check port 18789 not in use |
| Skills not appearing | Run `carnelian skills refresh`, check `skills/registry/` path |
| Task stuck in pending | Verify worker is running, check logs: `carnelian logs -f` |
| Performance tests fail | Ensure Docker has sufficient resources, close other applications |
| CLI command not found | Build binary: `cargo build --bin carnelian`, add to PATH or use `cargo run --bin carnelian --` |

### Debug Commands

```bash
# Check server status
carnelian status

# View all logs
carnelian logs -f

# View only errors
carnelian logs -f --level ERROR

# Check database connection
psql postgresql://carnelian:carnelian@localhost:5432/carnelian -c "SELECT COUNT(*) FROM tasks;"

# Verify Docker services
docker-compose ps
docker-compose logs carnelian-postgres
docker-compose logs carnelian-ollama
```

---

## Demo Video Recording

### Setup

- Clean database state: `carnelian migrate` (fresh)
- Start screen recording tool (OBS, QuickTime, etc.)
- Open terminal and UI side-by-side
- Use 1920x1080 resolution
- Increase terminal font size for readability

### Demo Script (2 Minutes)

**[0:00–0:15] System Startup**
- Show `carnelian start` command
- Display health check: `carnelian status`
- Show UI launching

**[0:15–0:30] Skill Discovery**
- Run `carnelian skills refresh`
- Show skills appearing in UI
- Demonstrate enable/disable

**[0:30–1:00] Task Creation & Execution**
- Create task via CLI: `carnelian task create "Demo task"`
- Create task via UI
- Show state transitions in real-time
- Display task logs

**[1:00–1:20] Concurrent Execution**
- Create multiple tasks rapidly
- Show queue depth in status
- Demonstrate concurrency limit

**[1:20–1:45] Error Handling**
- Create task with invalid skill
- Show error message
- Demonstrate recovery

**[1:45–2:00] Performance Summary**
- Display performance metrics table
- Show event stream handling
- Conclude with validation checklist

### Recording Tips

- Prepare commands in advance (use script or shell aliases)
- Narrate actions clearly
- Show both terminal and UI simultaneously (split screen)
- Keep transitions smooth — avoid long pauses

---

## References

- **Integration Tests:** `crates/carnelian-core/tests/checkpoint1_validation_test.rs`
- **CLI Documentation:** [README.md](../README.md) (CLI section)
- **Development Guide:** [DEVELOPMENT.md](DEVELOPMENT.md)
- **Docker Guide:** [DOCKER.md](DOCKER.md)
