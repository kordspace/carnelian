# 🔥 Carnelian OS — Operator Guide

Day-to-day operations and administration for running a Carnelian instance.

---

## Daily Operations

### Starting and Stopping

```bash
carnelian start                    # Start the orchestrator
carnelian stop                     # Stop gracefully
carnelian status                   # Check if running
```

Health endpoint — verify DB connectivity and version:

```bash
curl http://localhost:18789/v1/health
```

### Log Streaming

```bash
carnelian logs -f                  # Stream all events
carnelian logs -f --level ERROR    # Stream only ERROR events
```

See [docs/LOGGING.md](LOGGING.md) for structured log field conventions.

### Docker Services

```bash
docker-compose ps                          # Service status
docker-compose logs -f carnelian-postgres  # PostgreSQL logs
docker-compose logs -f carnelian-ollama    # Ollama logs
```

---

## Task Management

### Creating Tasks

```bash
carnelian task create "Task title"
carnelian task create "Task" --description "Details" --skill-id <uuid> --priority 5
```

Via API:

```bash
curl -X POST http://localhost:18789/v1/tasks \
  -H "Content-Type: application/json" \
  -d '{"title": "Task title", "description": "Details", "skill_id": null, "priority": 3, "requires_approval": false}'
```

### Monitoring Tasks

| Endpoint | Description |
|----------|-------------|
| `GET /v1/tasks` | List tasks (supports status filter) |
| `GET /v1/tasks/{id}` | Task detail view |
| `GET /v1/tasks/{id}/runs` | Execution history for a task |
| `GET /v1/runs/{run_id}/logs` | Paginated run logs |

### Cancelling Tasks

```bash
curl -X POST http://localhost:18789/v1/tasks/{id}/cancel
```

### Workspace Auto-Scanning

The heartbeat system discovers `TASK:` and `TODO:` markers in source files and auto-queues safe tasks.

Configure in `machine.toml`:

```toml
max_tasks_per_heartbeat = 5
workspace_scan_paths = ["."]
```

Or via environment variables:

```bash
export CARNELIAN_MAX_TASKS_PER_HEARTBEAT=5
export CARNELIAN_WORKSPACE_SCAN_PATHS=".,../other-project"
```

Set `max_tasks_per_heartbeat = 0` to disable auto-scanning.

---

## Approval Queue

### Reviewing Pending Approvals

```bash
# List all pending approvals
curl http://localhost:18789/v1/approvals?limit=100

# Filter by action type
curl http://localhost:18789/v1/approvals?action_type=capability.grant
```

### Approving / Denying

```bash
# Approve
curl -X POST http://localhost:18789/v1/approvals/{id}/approve \
  -H "Content-Type: application/json" \
  -d '{"signature": ""}'

# Deny
curl -X POST http://localhost:18789/v1/approvals/{id}/deny \
  -H "Content-Type: application/json" \
  -d '{"signature": ""}'

# Batch approve
curl -X POST http://localhost:18789/v1/approvals/batch \
  -H "Content-Type: application/json" \
  -d '{"approval_ids": ["id-1", "id-2"], "signature": ""}'
```

Approvals are signed server-side with the owner Ed25519 key and recorded in the ledger.

### Configuring the Owner Key

Set the keypair path in `machine.toml`:

```toml
owner_keypair_path = "/path/to/owner.key"
```

Or via environment variable:

```bash
export CARNELIAN_OWNER_KEYPAIR_PATH=/path/to/owner.key
```

Without a configured key, approve/deny endpoints return `401 Unauthorized`.

---

## XP & Skill Metrics

### Checking Agent Progress

```bash
# Agent XP, level, and progress
curl http://localhost:18789/v1/xp/agents/{id}

# Leaderboard (all agents ranked by total XP)
curl http://localhost:18789/v1/xp/leaderboard

# Top skills by usage/XP
curl http://localhost:18789/v1/xp/skills/top?limit=10
```

### Manual XP Award

Requires the `xp.award` capability:

```bash
curl -X POST http://localhost:18789/v1/xp/award \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "...", "amount": 25, "reason": "Manual bonus for exceptional task"}'
```

---

## Voice Configuration

### Setting Up ElevenLabs

```bash
# Configure API key and voice
curl -X POST http://localhost:18789/v1/voice/configure \
  -H "Content-Type: application/json" \
  -d '{"api_key": "your-elevenlabs-key", "voice_id": "voice-id-here"}'

# List available voices
curl http://localhost:18789/v1/voice/voices

# Test TTS with current config
curl -X POST http://localhost:18789/v1/voice/test \
  -H "Content-Type: application/json" \
  -d '{"text": "Hello, I am Lian."}'
```

The API key is encrypted and stored in `config_store`; it is never returned in API responses.

---

## Skill Registry

### Refreshing Skills

```bash
# Via CLI
carnelian skills refresh

# Via API
curl -X POST http://localhost:18789/v1/skills/refresh
```

Skills are auto-discovered on startup and via a 2-second debounced file watcher on `skills/registry/`. Manifests are checksummed with blake3 — only changed skills are updated in the database.

### Listing Skills

```bash
curl http://localhost:18789/v1/skills
```

### Enabling / Disabling Skills

```bash
curl -X POST http://localhost:18789/v1/skills/{skill_id}/enable
curl -X POST http://localhost:18789/v1/skills/{skill_id}/disable
```

---

## Database Maintenance

### Running Migrations

```bash
# Via CLI
carnelian migrate
carnelian migrate --dry-run

# Via sqlx-cli directly
export DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
sqlx migrate run
```

### Backup

```bash
pg_dump postgresql://carnelian:carnelian@localhost:5432/carnelian > backup.sql
```

### Restore

```bash
psql postgresql://carnelian:carnelian@localhost:5432/carnelian < backup.sql
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| **GPU not detected** | Verify NVIDIA Container Toolkit installation, check `nvidia-smi` in container |
| **PostgreSQL connection failed** | Ensure Docker services are running: `docker-compose ps` |
| **Ollama model download slow** | Models are large (4–20GB), monitor with `docker-compose logs -f carnelian-ollama` |
| **Rust build errors** | Update toolchain: `rustup update`, clean build: `cargo clean` |
| **Pre-commit hooks failing** | Run `cargo fmt --all` and `cargo clippy --workspace --all-targets --fix` |
| **Integration tests failing** | Ensure Docker is running, run `./scripts/ci-local.sh --full` locally |
| **Voice gateway: missing API key** | Run `POST /v1/voice/configure` with your ElevenLabs API key |
| **Voice gateway: rate limited** | ElevenLabs returns `429`; reduce request frequency or upgrade plan |
| **XP not updating** | Verify the daily quality bonus cron is running; check DB connectivity |
| **Worker quarantined** | Attestation mismatch — rebuild worker and restart. See [docs/ATTESTATION.md](ATTESTATION.md) |
| **Approval queue stuck** | Owner key not configured — set `owner_keypair_path` in `machine.toml` |
| **Memory/context issues** | Ensure pgvector extension is installed; verify embedding dimensions match model |

See [docs/DOCKER.md](DOCKER.md) for Docker-specific troubleshooting.
