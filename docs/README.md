# 🔥 Carnelian OS Documentation

> A local-first AI agent mainframe built in Rust with capability-based security and event-stream architecture.

## Quick Links

- **[Getting Started](GETTING_STARTED.md)** - Installation and first steps
- **[Architecture](ARCHITECTURE.md)** - System design and components
- **[API Reference](API.md)** - HTTP API documentation
- **[Development Guide](DEVELOPMENT.md)** - Contributing and development workflow

## Documentation Index

### Getting Started
- **[GETTING_STARTED.md](GETTING_STARTED.md)** - Quick start guide, installation, and configuration
- **[INSTALL.md](INSTALL.md)** - Detailed installation instructions
- **[Setup Guides](.)** - Platform-specific setup:
  - [SETUP_LINUX.md](SETUP_LINUX.md) - Linux installation
  - [SETUP_MACOS.md](SETUP_MACOS.md) - macOS installation
  - [SETUP_WINDOWS.md](SETUP_WINDOWS.md) - Windows installation

### Core Documentation
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - System architecture and design
- **[API.md](API.md)** - HTTP API reference and examples
- **[DEVELOPMENT.md](DEVELOPMENT.md)** - Development workflow and guidelines
- **[SECURITY.md](SECURITY.md)** - Security model and best practices

### Skills & Extensions
- **[WASM_SKILLS.md](WASM_SKILLS.md)** - Creating WASM skills in Rust
- **[RUST_SKILL_SYSTEM.md](RUST_SKILL_SYSTEM.md)** - Skill system architecture

### Deployment & Operations
- **[DOCKER.md](DOCKER.md)** - Docker setup and configuration
- **[DOCKER_ECOSYSTEM.md](DOCKER_ECOSYSTEM.md)** - Multi-container orchestration
- **[OPERATOR_GUIDE.md](OPERATOR_GUIDE.md)** - Production deployment guide
- **[REMOTE_DEPLOY.md](REMOTE_DEPLOY.md)** - Remote deployment strategies

### Advanced Topics
- **[PHASE3.md](PHASE3.md)** - Agentic execution engine deep-dive
- **[LOGGING.md](LOGGING.md)** - Logging philosophy and conventions
- **[ATTESTATION.md](ATTESTATION.md)** - Cryptographic attestation system

### Branding & Design
- **[BRAND.md](BRAND.md)** - Brand identity, logos, and color palette

### Release Information
- **[CHANGELOG.md](CHANGELOG.md)** - Version history and changes
- **[../RELEASE_NOTES.md](../RELEASE_NOTES.md)** - Latest release notes

## System Overview

**Core Components:**
- **Orchestrator**: Rust (Axum, Tokio, SQLx) — HTTP API, WebSocket events, task scheduling
- **Desktop UI**: Dioxus desktop application with 12 pages and 6 components
- **Workers**: Node.js, Python, WASM, Native Ops execution environments
- **Database**: PostgreSQL 16 with pgvector for vector embeddings
- **AI Models**: Local-first inference via Ollama (DeepSeek R1)
- **Security**: blake3-based hash-chain ledger, capability grants, deny-by-default policy

**Key Features:**
- 530+ built-in skills (Node.js, WASM, Native Ops)
- Event-stream architecture with backpressure handling
- Capability-based security with approval workflows
- Tamper-resistant audit ledger
- Local-first AI with Ollama integration
- Multi-worker task execution

## Machine Profiles

| Profile | GPU | RAM | Recommended Model |
|---------|-----|-----|-------------------|
| **Thummim** | RTX 2080 Super (8GB VRAM) | 32GB | `deepseek-r1:7b` |
| **Urim** | RTX 2080 Ti (11GB VRAM) | 64GB | `deepseek-r1:32b` |
| **Performance** | RTX 3090 (24GB VRAM) | 128GB | `deepseek-r1:70b` |

## Brand Identity

| Symbol | Name | Role |
|--------|------|------|
| 🔥 | **Carnelian OS** | System/runtime — the forge that refines and executes |
| 🦎 | **Lian** | Agent personality — the spirit that reasons and decides |
| 💎 | **Core** | Architectural foundations — security, ledger, guarantees |

See [BRAND.md](BRAND.md) for the complete brand kit with logos and color palette.

## Contributing

We welcome contributions! See [DEVELOPMENT.md](DEVELOPMENT.md) for:
- Development environment setup
- Code style guidelines
- Testing requirements
- Pull request process

## License

MIT License - see [LICENSE](../LICENSE) for details.

## Support

- **Issues**: [GitHub Issues](https://github.com/kordspace/carnelian/issues)
- **Documentation**: This directory
- **Community**: Coming soon

---

**🔥 Welcome to Carnelian OS** — A production-grade AI agent mainframe built for reliability, security, and local-first execution.
