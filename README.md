# Carnelian OS

A local-first AI agent mainframe built in Rust.

## Project Structure

```
carnelian/
├── crates/                          # Rust workspace crates
│   ├── carnelian-core/             # Rust orchestrator
│   ├── carnelian-ui/               # Dioxus desktop UI
│   ├── carnelian-worker-node/      # Node.js worker wrapper
│   ├── carnelian-worker-python/    # Python worker wrapper
│   ├── carnelian-worker-shell/     # Shell worker wrapper
│   └── carnelian-common/           # Shared types and utilities
├── workers/                         # Worker implementations
│   ├── node-worker/                # Node.js worker (600+ skills)
│   ├── python-worker/              # Python worker
│   └── shell-worker/               # Shell worker
├── skills/                          # Skill bundles
│   └── registry/                   # Skill registry and manifests
├── db/                              # Database artifacts
│   └── migrations/                 # SQL migration files
└── docs/                            # Documentation
```

## Prerequisites

- Rust 1.85+ (stable)
- Node.js 18+
- Python 3.10+
- Docker & Docker Compose
- PostgreSQL 15+ (via Docker)
- Ollama (for local models)

## Quick Start

```bash
# Build Rust workspace
cargo build

# Run tests
cargo test

# Start desktop UI
cargo run -p carnelian-ui
```

## Development

See [docs/SETUP.md](docs/SETUP.md) for detailed setup instructions.

## Machine Profiles

- **Thummim**: 2080 Super, 32GB RAM (constrained)
- **Urim**: 2080 Ti, 64GB RAM (high-end)

## License

MIT

## Repository

https://github.com/kordspace/carnelian
