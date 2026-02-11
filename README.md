# 🔥 Carnelian OS

[![CI](https://github.com/kordspace/carnelian/actions/workflows/ci.yml/badge.svg)](https://github.com/kordspace/carnelian/actions/workflows/ci.yml)
[![🔥 Carnelian OS](https://img.shields.io/badge/🔥-Carnelian%20OS-D24B2A)](https://github.com/kordspace/carnelian)
[![🦎 Lian](https://img.shields.io/badge/🦎-Lian-7C4DFF)](https://github.com/kordspace/carnelian)

A local-first AI agent mainframe built in Rust with capability-based security and event-stream architecture.

> 💎 *Carnelian is a gemstone — warm, fiery, and grounding. The name reflects both the system's Rust foundation and its role as a precious, reliable core.*

## Brand Identity

| Symbol | Name | Role |
|--------|------|------|
| 🔥 | **Carnelian OS** | System/runtime — the forge that refines and executes |
| 🦎 | **Lian** | Agent personality — the spirit that reasons and decides |
| 💎 | **Core** | Architectural foundations — security, ledger, guarantees |

See [docs/BRAND.md](docs/BRAND.md) for the complete dual-theme brand kit (Forge/Night Lab).

## Overview

🔥 Carnelian OS is a production-grade rewrite of the experimental Thummim system. It addresses critical performance bottlenecks, security gaps, and architectural debt accumulated in the original Node.js/TypeScript monolith while preserving the 600+ skills, personality features, and core workflows that make the system valuable.

The core value proposition is reliable AI agent orchestration with strong containment guarantees, local-first execution via Ollama, and tamper-resistant auditability. Carnelian provides a foundation for autonomous task execution with proper resource controls, capability-based security, and event-stream architecture that prevents UI freezes under load.

## Why 🔥 Carnelian?

**What's Preserved from Thummim:**
- PostgreSQL backend with pgvector for embeddings
- Local model integration (Ollama/DeepSeek)
- Heartbeat system (555,555ms wake routine)
- Task queue and scheduling
- Personality features and mantra rotation
- 600+ existing skills via Node.js worker

**What's Improved:**
- Rust core for performance and memory safety
- Capability-based security (deny-by-default)
- Event-stream architecture with backpressure
- Proper resource controls and worker sandboxing
- Hash-chain ledger for tamper-resistant audit trail
- Priority-based event sampling (no UI freezes)

## Architecture

The following diagram illustrates the core interaction flow, based on the architecture in [Epic Brief](spec:5e7be550-aec5-4ebb-b0e3-3ce021e3f9ab/7c191398-0049-4dc4-8378-585569a1a4e4) and [Technical Plan](spec:5e7be550-aec5-4ebb-b0e3-3ce021e3f9ab/3ccb59e1-e29e-4f62-883e-e5d97a90d157).

```mermaid
sequenceDiagram
    participant User
    participant UI as Dioxus UI
    participant Core as Core Orchestrator
    participant Policy as Policy Engine
    participant Worker as Worker Manager
    participant NodeW as Node Worker
    participant DB as PostgreSQL
    participant Ollama as Ollama (Local LLM)

    User->>UI: Create Task
    UI->>Core: POST /v1/tasks
    Core->>Policy: Check Capabilities
    Policy-->>Core: Decision (allowed/denied)
    Core->>DB: Persist Task
    Core->>Worker: Dispatch Task
    Worker->>NodeW: Execute Skill
    NodeW->>Ollama: Model Request
    Ollama-->>NodeW: Response
    NodeW-->>Worker: Skill Result
    Worker->>DB: Log Run
    Worker->>Core: Task Complete
    Core->>UI: Event Stream (WebSocket)
    UI-->>User: Update Display
```

### Key Components

| Component | Technology | Description |
|-----------|------------|-------------|
| **Core Orchestrator** | Axum/Tokio/SQLx | HTTP API, WebSocket events, task scheduling |
| **Desktop UI** | Dioxus | Native desktop interface with event streaming |
| **Policy Engine** | Rust | Capability-based security, deny-by-default |
| **Worker Manager** | Rust | Worker lifecycle, JSONL transport, capability grants |
| **Node Worker** | Node.js/TypeScript | Executes 600+ existing Thummim skills |
| **Ledger Manager** | Rust | blake3 hash-chain audit trail for privileged actions |
| **Scheduler** | Rust | Priority-based task queue, retry policies, heartbeat |

## CLI

The `carnelian` binary provides a full command-line interface:

```bash
carnelian start                    # Start the orchestrator
carnelian start --log-level DEBUG  # Start with debug logging
carnelian status                   # Check if running
carnelian stop                     # Stop gracefully
carnelian migrate                  # Run database migrations
carnelian migrate --dry-run        # Show pending migrations
carnelian logs                     # Stream events from running instance
carnelian logs -f --level ERROR    # Stream only ERROR events
carnelian skills refresh           # Scan registry and sync skills to database
carnelian task create "Task title"                           # Create a task
carnelian task create "Task" --description "Details"         # With description
carnelian task create "Task" --skill-id <uuid> --priority 5  # With skill and priority
```

Global flags: `--database-url`, `--config`, `--log-level`, `--port`.
The `--url` flag can be used with `task` commands to specify a remote server URL (e.g., `carnelian task --url http://remote:18789 create "Task"`).

See [docs/CHECKPOINT1.md](docs/CHECKPOINT1.md) for the checkpoint validation guide.

## API Endpoints

All endpoints are prefixed with `/v1`.

### System

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/health` | Health check (database connectivity, version) |
| `GET` | `/v1/status` | System status |
| `POST` | `/v1/events` | Publish an event |
| `GET` | `/v1/events/ws` | WebSocket event stream |

### Tasks

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/v1/tasks` | Create a new task |
| `GET` | `/v1/tasks` | List tasks |
| `GET` | `/v1/tasks/{task_id}` | Get task details |
| `POST` | `/v1/tasks/{task_id}/cancel` | Cancel a task |
| `GET` | `/v1/tasks/{task_id}/runs` | List runs for a task |

### Runs

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/runs/{run_id}` | Get run details |
| `GET` | `/v1/runs/{run_id}/logs` | Get paginated run logs |

### Skills

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/skills` | List registered skills |
| `POST` | `/v1/skills/{skill_id}/enable` | Enable a skill |
| `POST` | `/v1/skills/{skill_id}/disable` | Disable a skill |
| `POST` | `/v1/skills/refresh` | Refresh skill registry |

## Prerequisites

### Required
- **Rust 1.85+** - Install from [rustup.rs](https://rustup.rs)
- **Docker & Docker Compose** - For PostgreSQL and Ollama
- **Git** - Version control

### For GPU Support
- **NVIDIA GPU** - RTX 2080 or better recommended
- **NVIDIA Container Toolkit** - For GPU passthrough to Docker

### For Workers
- **Node.js 18+** - For Node.js worker (600+ skills)
- **Python 3.10+** - For Python worker

### For Development
- **prek** - Pre-commit hooks: `cargo install prek`
- **sqlx-cli** - Database migrations: `cargo install sqlx-cli`

### Platform-Specific Notes

<details>
<summary><strong>Windows (WSL2)</strong></summary>

```powershell
# 1. Enable WSL2 (run as Administrator)
wsl --install
wsl --set-default-version 2

# 2. Install NVIDIA GPU drivers on Windows host
# Download from: https://www.nvidia.com/drivers
# Verify: nvidia-smi (in PowerShell)

# 3. Install Docker Desktop
# Download from: https://www.docker.com/products/docker-desktop
# Enable WSL2 backend in Settings > General
# Enable GPU support in Settings > Resources > WSL Integration

# 4. Install Rust (in WSL2 terminal)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 5. Install Node.js and Python (in WSL2)
sudo apt update
sudo apt install -y nodejs npm python3 python3-pip

# 6. Verify installations
docker --version
cargo --version
node --version
python3 --version
nvidia-smi  # Should show GPU in WSL2
```

</details>

<details>
<summary><strong>macOS</strong></summary>

```bash
# Note: GPU passthrough is NOT supported on macOS
# Ollama will run in CPU-only mode with reduced performance

# 1. Install Homebrew (if not installed)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# 2. Install Docker Desktop
brew install --cask docker
# Or download from: https://www.docker.com/products/docker-desktop

# 3. Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 4. Install Node.js and Python
brew install node python@3.12

# 5. Verify installations
docker --version
cargo --version
node --version
python3 --version

# Note: For GPU workloads, consider using a Linux machine or cloud instance
```

</details>

<details>
<summary><strong>Linux (Ubuntu/Debian)</strong></summary>

```bash
# 1. Install Docker
sudo apt update
sudo apt install -y docker.io docker-compose
sudo usermod -aG docker $USER
newgrp docker

# 2. Install NVIDIA drivers (if GPU present)
sudo apt install -y nvidia-driver-535  # Or latest version
sudo reboot

# 3. Install NVIDIA Container Toolkit
curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | sudo gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list | \
  sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | \
  sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list
sudo apt update
sudo apt install -y nvidia-container-toolkit
sudo nvidia-ctk runtime configure --runtime=docker
sudo systemctl restart docker

# 4. Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 5. Install Node.js and Python
sudo apt install -y nodejs npm python3 python3-pip

# 6. Verify installations
docker --version
cargo --version
node --version
python3 --version
nvidia-smi
docker run --rm --gpus all nvidia/cuda:12.0-base nvidia-smi  # Test GPU in Docker
```

</details>

## Quick Start

```bash
# 1. Clone repository
git clone https://github.com/kordspace/carnelian.git
cd carnelian

# 2. Start Docker services
docker-compose up -d

# 3. Verify services are healthy
docker-compose ps

# 4. Download model for your profile
# Thummim (8GB VRAM):
docker exec carnelian-ollama ollama pull deepseek-r1:7b
# Urim (11GB VRAM):
docker exec carnelian-ollama ollama pull deepseek-r1:32b

# 5. Run database migrations
cargo install sqlx-cli --no-default-features --features postgres
export DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
sqlx migrate run

# 6. Build Rust workspace
cargo build

# 7. Run tests
cargo test

# 8. Start the orchestrator
cargo run --bin carnelian -- start
```

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for detailed setup and development workflow.

## Machine Profiles

| Profile | GPU | VRAM | RAM | Recommended Model | Notes |
|---------|-----|------|-----|-------------------|-------|
| **Thummim** | RTX 2080 Super | 8GB | 32GB | `deepseek-r1:7b` | Constrained profile for development |
| **Urim** | RTX 2080 Ti | 11GB | 64GB | `deepseek-r1:32b` | High-end profile for production workloads |

Profiles affect Docker resource limits and worker concurrency settings. See [docker-compose.yml](docker-compose.yml) and [machine.toml.example](machine.toml.example) for configuration.

## Project Structure

```
carnelian/
├── crates/
│   ├── carnelian-core/           # Core orchestrator (Axum server, scheduler, policy, ledger, workers)
│   │   ├── src/
│   │   │   ├── bin/carnelian.rs  # CLI binary (start, stop, status, migrate, logs)
│   │   │   ├── server.rs         # HTTP API + WebSocket server
│   │   │   ├── scheduler.rs      # Task queue, priority scheduling, retry policies
│   │   │   ├── worker.rs         # Worker manager, JSONL transport, process lifecycle
│   │   │   ├── events.rs         # Event stream with backpressure and bounded buffers
│   │   │   ├── policy.rs         # Capability-based security engine
│   │   │   ├── ledger.rs         # blake3 hash-chain audit trail
│   │   │   ├── skills.rs          # Skill discovery, manifest validation, file watcher
│   │   │   ├── config.rs         # Configuration loading (TOML, env, CLI)
│   │   │   └── db.rs             # Database connection and migrations
│   │   └── tests/                # 9 test suites, 80+ tests
│   ├── carnelian-common/         # Shared types, error handling, API models
│   ├── carnelian-ui/             # Dioxus desktop UI
│   ├── carnelian-worker-node/    # Node.js worker wrapper crate
│   ├── carnelian-worker-python/  # Python worker wrapper crate
│   └── carnelian-worker-shell/   # Shell worker wrapper crate
├── workers/
│   ├── node-worker/              # Node.js/TypeScript worker (600+ skills)
│   ├── python-worker/            # Python worker
│   └── shell-worker/             # Shell worker
├── skills/
│   └── registry/                 # Skill bundles and manifests
├── db/
│   └── migrations/               # SQL migrations (5 migration files, PostgreSQL 16 + pgvector)
├── docs/                         # Documentation (development, docker, brand, logging)
├── scripts/
│   ├── setup-hooks.sh            # Development environment setup
│   └── ci-local.sh               # Local CI checks before pushing
└── .github/workflows/ci.yml      # CI pipeline (lint, build, test, integration, secrets)
```

## Key Features

- **Capability-Based Security** - Deny-by-default with explicit grants, owner-signed Ed25519 authority
- **Event-Stream Architecture** - Priority-based sampling, bounded buffers, WebSocket streaming
- **Local-First Inference** - Ollama integration with GPU support, remote fallback
- **Heartbeat System** - 555,555ms wake routine with mantra rotation, auto-task queuing
- **Worker Sandboxing** - Isolated process execution with JSONL transport protocol
- **Tamper-Resistant Ledger** - blake3 hash-chain audit trail for integrity verification
- **600+ Skills** - Full compatibility with existing Thummim skill library via Node worker
- **Task Lifecycle** - Priority-based scheduling, concurrency limits, configurable retry policies
- **LZ4 Compression** - Database column compression for large payloads (memories, logs, metadata)
- **Skill Discovery** - Automatic filesystem watching with blake3 checksums and database sync

## Skill Discovery

Skills are defined by `skill.json` manifest files in the `skills/registry/` directory. Discovery runs automatically on server startup and via a file watcher (2-second debounce), or can be triggered manually.

### Manifest Format

Each skill is a subdirectory containing a `skill.json`:

```json
{
  "name": "echo",
  "description": "Echo test skill",
  "runtime": "node",
  "version": "1.0.0",
  "capabilities_required": ["fs.read"],
  "sandbox": {
    "network": "disabled",
    "max_memory_mb": 128
  },
  "openclaw_compat": {
    "emoji": "🔊",
    "tags": ["utility"]
  }
}
```

Required fields: `name`, `description`, `runtime` (`node`|`python`|`shell`|`wasm`).

### Discovery Modes

| Mode | Trigger | Description |
|------|---------|-------------|
| **Startup** | Server boot | Full scan on `carnelian start` |
| **File watcher** | Filesystem change | 2-second debounced scan of `skills/registry/` |
| **CLI** | `carnelian skills refresh` | Manual scan with console output |
| **API** | `POST /v1/skills/refresh` | Manual scan returning JSON counts |

Manifests are checksummed with blake3 — skills are only updated in the database when the checksum changes. Stale skills (manifests removed from disk) are automatically deleted.

See [skills/registry/README.md](skills/registry/README.md) for the full manifest specification.

### Security Architecture Notes

The ledger uses **blake3** (not SHA-256) for hash-chain integrity, providing faster performance than traditional cryptographic hashes while maintaining collision resistance.

The policy engine (`crates/carnelian-core/src/policy.rs`) and ledger manager (`crates/carnelian-core/src/ledger.rs`) shipped early as part of Phase 1 foundation, though originally planned for Phase 4 in the roadmap.

## Development

- **Setup Guide:** [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)
- **Docker Guide:** [docs/DOCKER.md](docs/DOCKER.md)
- **Logging Guide:** [docs/LOGGING.md](docs/LOGGING.md)

Pre-commit hooks (prek) run automatically on commit. CI enforces formatting (rustfmt), linting (clippy), and secret scanning.

```bash
# Format code
cargo fmt --all

# Run lints
cargo clippy --workspace --all-targets -- -D warnings

# Run unit tests
cargo test --workspace

# Run all pre-commit hooks
prek run --all-files
```

### Local CI Checks

Run the local CI script before pushing to catch issues early:

```bash
# Quick checks (fmt, clippy, unit tests, doc-tests) — no Docker needed
./scripts/ci-local.sh

# Full checks including integration tests — requires Docker
./scripts/ci-local.sh --full
```

### Testing

The project has **80+ tests** across 9 test suites:

| Suite | Tests | Docker | Description |
|-------|-------|--------|-------------|
| Unit tests | 12 | No | Core module tests (scheduler, policy, ledger, worker, db) |
| Config tests | 11 | No | Configuration loading and validation |
| Logging tests | 11 | No | Structured logging conventions |
| Skill discovery tests | 6+12 | Mixed | Manifest validation (no Docker), DB integration (Docker) |
| CLI tests | 7 | Yes | Full CLI command validation |
| Integration tests | 7 | Yes | Database, server startup, load handling |
| Migration tests | 12 | Yes | Schema migrations and seed data |
| Scheduler tests | 7 | Yes | Priority scheduling, concurrency, retries |
| Server tests | 8 | Yes | HTTP API, WebSocket, compression |
| Worker transport tests | 7 | Yes | JSONL protocol, timeouts, cancellation |

```bash
# Unit tests only (no Docker)
cargo test --workspace

# All integration tests (requires Docker)
cargo test --workspace -- --ignored

# Specific test suite
cargo test --test scheduler_integration_test -- --ignored
```

See [crates/carnelian-core/tests/README.md](crates/carnelian-core/tests/README.md) for detailed test documentation.

### CI Pipeline

The GitHub Actions CI pipeline runs on every push to `main` and on pull requests:

1. **Rust Lint** — `cargo fmt --check` + `cargo clippy -D warnings`
2. **Rust Build & Test** — `cargo build` + `cargo test` + `cargo doc`
3. **Node.js Worker** — `npm ci` + `npm run build` + `npm test`
4. **Integration Tests** — PostgreSQL service + all `--ignored` tests
5. **Secret Scanning** — `detect-secrets` baseline audit

## Database

PostgreSQL 16 with pgvector extension. Schema managed via SQLx migrations in `db/migrations/`:

| Migration | Description |
|-----------|-------------|
| `00000000000000_init.sql` | pgvector extension |
| `00000000000001_core_schema.sql` | Core tables (identities, skills, tasks, task_runs, run_logs, etc.) |
| `00000000000002_phase1_delta.sql` | Phase 1 additions (sessions, skill_versions, workflows, sub_agents, XP, elixirs) |
| `00000000000003_schema_fixes.sql` | Schema refinements (pronouns, subject_id TEXT, LZ4 compression) |
| `00000000000004_xp_curve_retune.sql` | XP curve rebalancing |
| `00000000000005_config_store_value_blob.sql` | Config store value column |

## Configuration

Configuration is loaded in order of precedence (highest wins):

1. **Environment variables** (`DATABASE_URL`, `CARNELIAN_HTTP_PORT`, etc.)
2. **Config file** (`machine.toml` — copy from `machine.toml.example`)
3. **Built-in defaults**

See [.env.example](.env.example) for environment variables and [machine.toml.example](machine.toml.example) for file-based configuration.

## Troubleshooting

| Issue | Solution |
|-------|----------|
| **GPU not detected** | Verify NVIDIA Container Toolkit installation, check `nvidia-smi` in container |
| **PostgreSQL connection failed** | Ensure Docker services are running: `docker-compose ps` |
| **Ollama model download slow** | Models are large (4-20GB), monitor with `docker-compose logs -f carnelian-ollama` |
| **Rust build errors** | Update toolchain: `rustup update`, clean build: `cargo clean` |
| **Pre-commit hooks failing** | Run `cargo fmt --all` and `cargo clippy --workspace --all-targets --fix` |
| **Integration tests failing** | Ensure Docker is running, run `./scripts/ci-local.sh --full` locally |

See [docs/DOCKER.md](docs/DOCKER.md) for detailed troubleshooting.

## Documentation

| Document | Description |
|----------|-------------|
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | Development setup and workflow |
| [docs/DOCKER.md](docs/DOCKER.md) | Docker environment and troubleshooting |
| [docs/BRAND.md](docs/BRAND.md) | Dual theme brand kit (Forge / Night Lab) |
| [docs/LOGGING.md](docs/LOGGING.md) | Structured logging philosophy and conventions |
| [crates/carnelian-core/tests/README.md](crates/carnelian-core/tests/README.md) | Test suite documentation |
| [db/migrations/README.md](db/migrations/README.md) | Database migration guide |

### Project Planning

- **Epic Brief:** [`spec:5e7be550-aec5-4ebb-b0e3-3ce021e3f9ab/7c191398-0049-4dc4-8378-585569a1a4e4`](spec:5e7be550-aec5-4ebb-b0e3-3ce021e3f9ab/7c191398-0049-4dc4-8378-585569a1a4e4) - Design goals, machine profiles (Urim/Thummim), success criteria.
- **Technical Plan:** [`spec:5e7be550-aec5-4ebb-b0e3-3ce021e3f9ab/3ccb59e1-e29e-4f62-883e-e5d97a90d157`](spec:5e7be550-aec5-4ebb-b0e3-3ce021e3f9ab/3ccb59e1-e29e-4f62-883e-e5d97a90d157) - Architecture, data model, components (includes Mermaid system diagram).

## Contributing

This is currently a personal project (Marco + Mim). The architecture is designed for eventual sharing as a platform.

- Pre-commit hooks enforce code quality
- CI requires passing lint, build, and integration test checks
- See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for code style

## License

MIT

## Repository

https://github.com/kordspace/carnelian
