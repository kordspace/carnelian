# Carnelian OS

[![CI](https://github.com/kordspace/carnelian/actions/workflows/ci.yml/badge.svg)](https://github.com/kordspace/carnelian/actions/workflows/ci.yml)

A local-first AI agent mainframe built in Rust with capability-based security and event-stream architecture.

## Overview

Carnelian OS is a production-grade rewrite of the experimental Thummim system. It addresses critical performance bottlenecks, security gaps, and architectural debt accumulated in the original Node.js/TypeScript monolith while preserving the 600+ skills, personality features, and core workflows that make the system valuable.

The core value proposition is reliable AI agent orchestration with strong containment guarantees, local-first execution via Ollama, and tamper-resistant auditability. Carnelian provides a foundation for autonomous task execution with proper resource controls, capability-based security, and event-stream architecture that prevents UI freezes under load.

## Why Carnelian?

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

The following diagram illustrates the core interaction flow, based on the [Technical Plan](https://github.com/kordspace/carnelian/wiki/Technical-Plan).

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
| **Worker Manager** | Rust | Worker lifecycle, sandboxing, capability grants |
| **Node Worker** | Node.js | Executes 600+ existing Thummim skills |
| **Ledger Manager** | Rust | Hash-chain audit trail for privileged actions |
| **Model Router** | Rust | Local-first Ollama, remote fallback |

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

# 8. Start desktop UI
cargo run -p carnelian-ui
```

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for detailed setup and development workflow.

## Machine Profiles

| Profile | GPU | VRAM | RAM | Recommended Model | Notes |
|---------|-----|------|-----|-------------------|-------|
| **Thummim** | RTX 2080 Super | 8GB | 32GB | `deepseek-r1:7b` | Constrained profile for development |
| **Urim** | RTX 2080 Ti | 11GB | 64GB | `deepseek-r1:32b` | High-end profile for production workloads |

Profiles affect Docker resource limits and worker concurrency settings. See [docker-compose.yml](docker-compose.yml) for resource configuration.

## Project Structure

```
carnelian/
├── crates/
│   ├── carnelian-core/           # Core orchestrator (Axum server, policy engine, scheduler)
│   ├── carnelian-ui/             # Dioxus desktop UI (event stream, task management)
│   ├── carnelian-common/         # Shared types and utilities
│   ├── carnelian-worker-node/    # Node.js worker wrapper
│   ├── carnelian-worker-python/  # Python worker wrapper
│   └── carnelian-worker-shell/   # Shell worker wrapper
├── workers/
│   ├── node-worker/              # Node.js worker (600+ skills)
│   ├── python-worker/            # Python worker
│   └── shell-worker/             # Shell worker
├── skills/
│   └── registry/                 # Skill bundles and manifests
├── db/
│   └── migrations/               # SQL migrations (PostgreSQL with pgvector)
├── docs/                         # Comprehensive documentation
└── .github/workflows/            # CI/CD pipeline (lint, build, test)
```

## Key Features

- **Capability-Based Security** - Deny-by-default with explicit grants, owner-signed authority for privileged actions
- **Event-Stream Architecture** - Priority-based sampling, bounded buffers, no UI freezes
- **Local-First Inference** - Ollama integration with GPU support, remote fallback
- **Heartbeat System** - 555,555ms wake routine with mantra rotation, auto-task queuing
- **Worker Sandboxing** - Isolated execution with explicit capability grants
- **Tamper-Resistant Ledger** - Hash-chain audit trail for privileged actions
- **600+ Skills** - Full compatibility with existing Thummim skill library via Node worker

## Development

- **Setup Guide:** [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)
- **Docker Guide:** [docs/DOCKER.md](docs/DOCKER.md)

Pre-commit hooks (prek) run automatically on commit. CI enforces formatting (rustfmt) and linting (clippy).

```bash
# Format code
cargo fmt --all

# Run lints
cargo clippy --workspace --all-targets

# Run all pre-commit hooks
prek run --all-files
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| **GPU not detected** | Verify NVIDIA Container Toolkit installation, check `nvidia-smi` in container |
| **PostgreSQL connection failed** | Ensure Docker services are running: `docker-compose ps` |
| **Ollama model download slow** | Models are large (4-20GB), monitor with `docker-compose logs -f carnelian-ollama` |
| **Rust build errors** | Update toolchain: `rustup update`, clean build: `cargo clean` |
| **Pre-commit hooks failing** | Run `cargo fmt --all` and `cargo clippy --workspace --all-targets --fix` |

See [docs/DOCKER.md](docs/DOCKER.md) for detailed troubleshooting.

## Documentation

- [Development Guide](docs/DEVELOPMENT.md) - Setup and workflow
- [Docker Guide](docs/DOCKER.md) - Environment and troubleshooting

### Project Planning

- **Epic Brief:** Design goals, constraints, and success criteria
- **Technical Plan:** Detailed architecture, component design, and implementation phases
- **Core Flows:** Task lifecycle, event streaming, and capability checks

## Contributing

This is currently a personal project (Marco + Mim). The architecture is designed for eventual sharing as a platform.

- Pre-commit hooks enforce code quality
- CI requires passing lint and build checks
- See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for code style

## License

MIT

## Repository

https://github.com/kordspace/carnelian
