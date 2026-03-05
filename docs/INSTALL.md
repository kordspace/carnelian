# Carnelian OS Installation Guide

This guide covers installing and setting up Carnelian OS, an AI agent runtime with local LLM inference capabilities.

## Prerequisites

| Requirement | Minimum | Recommended | Notes |
|-------------|---------|-------------|-------|
| **Rust** | 1.85+ | Latest stable | Install via [rustup](https://rustup.rs) |
| **Docker** | 20.10+ | Latest | Required for PostgreSQL and Ollama containers |
| **Node.js** | 22.x | 22.x+ | For Gateway service and Node.js workers |
| **Python** | 3.10+ | 3.12 | For skill dependencies |
| **NVIDIA Drivers** | 525+ | 550+ | For GPU inference (optional) |
| **RAM** | 8 GB | 16+ GB | ≥32 GB recommended for `standard` profile |
| **GPU VRAM** | N/A | 6+ GB | ≥10 GB for `performance` profile |
| **Disk** | 10 GB free | 50+ GB | For models and data |

### Platform-Specific Notes

- **Windows**: Docker Desktop with WSL2 backend recommended
- **macOS**: Docker Desktop required; Apple Silicon uses CPU inference
- **Linux**: Docker Engine or Docker Desktop; native NVIDIA support

## Quick Install

The fastest way to get Carnelian OS running:

```bash
# 1. Clone the repository
git clone https://github.com/carnelian/carnelian.git
cd carnelian

# 2. Build the project
cargo build --release

# 3. Run the interactive setup wizard
carnelian init

# 4. Start the system
carnelian start
```

The `carnelian init` command will:
- Detect your hardware (RAM, GPU VRAM)
- Suggest an appropriate machine profile (`standard`, `performance`, or `custom`)
- Set up Docker containers for PostgreSQL and Ollama
- Generate an owner keypair for secure API access
- Configure `machine.toml` with your settings
- Run database migrations
- Activate starter skills

## Non-Interactive / CI Installation

For automated deployments, headless servers, or CI/CD pipelines:

### Available Flags

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--non-interactive` | `-y` | Skip all prompts, use defaults | `false` |
| `--force` | `-f` | Overwrite existing `machine.toml` | `false` |
| `--resume` | | Resume from previous init state | `false` |
| `--key-path` | | Use existing key file path | Auto-generate |

### CI Example

```bash
#!/bin/bash
set -e

# Clone and build
git clone https://github.com/carnelian/carnelian.git
cd carnelian
cargo build --release

# Non-interactive initialization (auto-detects hardware, uses defaults)
./target/release/carnelian init --non-interactive --force

# Start services
./target/release/carnelian start
```

### Docker Compose CI Example

```yaml
version: '3.8'
services:
  carnelian:
    build: .
    environment:
      - CARNELIAN_PROFILE=standard
    volumes:
      - carnelian-data:/root/.carnelian
    command: >
      sh -c "carnelian init --non-interactive && carnelian start"
    ports:
      - "18789:18789"
      
volumes:
  carnelian-data:
```

## Manual Installation

For detailed platform-specific instructions, see:

- [Windows Setup Guide](SETUP_WINDOWS.md) - WSL2, Docker Desktop, PowerShell
- [macOS Setup Guide](SETUP_MACOS.md) - Homebrew, Docker Desktop, Apple Silicon notes
- [Linux Setup Guide](SETUP_LINUX.md) - Docker Engine, NVIDIA drivers, systemd

## Troubleshooting

| Symptom | Exit Code | Cause | Fix |
|---------|-----------|-------|-----|
| "Docker not found" | 1 | Docker not installed or not running | Install Docker per [Quick Install](#quick-install) |
| "Hardware below minimum" | 2 | < 8 GB RAM detected | Upgrade RAM or use a more powerful machine |
| "Migration failed" | 3 | PostgreSQL not ready | Check `docker ps`, restart containers, or use `--resume` |
| "Key file not found" | - | `--key-path` points to missing file | Verify path or omit flag to auto-generate |
| "machine.toml exists" | - | Re-running init without `--force` | Use `carnelian init --force` to overwrite |
| Ollama connection timeout | - | Ollama container not healthy | Run `docker logs carnelian-ollama` to diagnose |
| Port already in use | - | Another service using port 5432/11434/18789 | Stop conflicting services or edit `machine.toml` |
| Permission denied on key file | - | Incorrect file permissions | Run `chmod 600 ~/.carnelian/owner.pem` (Unix) |

### Common Commands for Debugging

```bash
# Check Carnelian status
carnelian status

# View Docker container logs
docker logs carnelian-postgres
docker logs carnelian-ollama

# Reset and start fresh
rm -rf ~/.carnelian/init-state.json
rm machine.toml
carnelian init --force

# Resume interrupted init
carnelian init --resume

# Check hardware detection
carnelian init --non-interactive 2>&1 | head -20
```

### Getting Help

- Check platform setup guides ([Windows](SETUP_WINDOWS.md), [macOS](SETUP_MACOS.md), [Linux](SETUP_LINUX.md)) for OS-specific details
- Review [ARCHITECTURE.md](ARCHITECTURE.md) for system design
- File issues on [GitHub Issues](https://github.com/carnelian/carnelian/issues)

## Post-Installation

After successful initialization:

1. **Access the dashboard**: Open http://localhost:18789 in your browser
2. **Configure API tokens** (interactive only): GitHub and OpenAI tokens stored encrypted
3. **Scan workspaces**: `carnelian skills refresh` to index your code
4. **Start chatting**: Use the Web UI or API at `http://localhost:18789/api/v1/chat`

## Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `CARNELIAN_PROFILE` | Default machine profile | `standard`, `performance`, `custom` |
| `CARNELIAN_DATABASE_URL` | PostgreSQL connection string | `postgresql://...` |
| `CARNELIAN_OLLAMA_URL` | Ollama API endpoint | `http://localhost:11434` |
| `CARNELIAN_HTTP_PORT` | HTTP server port | `18789` |
| `RUST_LOG` | Logging level | `info`, `debug`, `trace` |
