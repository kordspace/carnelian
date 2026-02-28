# CARNELIAN Release Notes

## Version 0.1.0 - Production Release

**Release Date**: February 28, 2026

### Overview

CARNELIAN is a production-ready AI workspace harness built in Rust that provides foundational infrastructure for autonomous agent orchestration. This release represents a complete, tested, and documented system ready for deployment.

### Core Features

#### Infrastructure
- ✅ **Core Orchestrator**: Axum/Tokio-based HTTP API server with WebSocket support
- ✅ **CLI Interface**: Comprehensive command-line interface for all operations
- ✅ **Event Stream**: Backpressure-aware event streaming with bounded buffers
- ✅ **Database**: PostgreSQL 16 with pgvector extension for semantic search
- ✅ **Migrations**: 9 SQL migrations with full schema management

#### Task Execution
- ✅ **Multi-Runtime Workers**: Node.js, Python, WASM, and native Rust workers
- ✅ **600+ Skills**: Full compatibility with existing skill library
- ✅ **Skill Discovery**: Automatic discovery with blake3 checksums and file watching
- ✅ **Scheduler**: Priority-based task queue with retry policies
- ✅ **XP System**: 1.172-exponent level curve with skill metrics

#### Intelligence & Context
- ✅ **Soul Management**: Personality state and directive management
- ✅ **Session Lifecycle**: Conversation persistence and context assembly
- ✅ **Memory Manager**: pgvector-powered semantic memory retrieval
- ✅ **Model Router**: Multi-provider LLM routing (Ollama, OpenAI, Anthropic, Fireworks)
- ✅ **Agentic Loop**: Heartbeat system (555,555ms) with autonomous task discovery
- ✅ **Compaction Pipeline**: Automatic context window management

#### Security & Compliance
- ✅ **Capability-Based Security**: Deny-by-default with explicit grants
- ✅ **Policy Engine**: Fine-grained permission control
- ✅ **Approval Queue**: Human-in-the-loop workflows
- ✅ **Safe Mode**: Emergency lockdown capability
- ✅ **Attestations**: Ed25519 worker identity verification
- ✅ **Encryption**: AES-256-GCM encryption at rest
- ✅ **Ledger**: blake3 hash-chain audit trail
- ✅ **Chain Anchoring**: Ledger integrity verification

#### Advanced Features
- ✅ **Sub-Agents**: Delegated agent execution
- ✅ **Workflows**: Multi-step workflow orchestration
- ✅ **Channel Adapters**: Telegram and Discord integration with pairing
- ✅ **Voice Gateway**: ElevenLabs STT/TTS integration
- ✅ **Elixir System**: RAG-based knowledge persistence
- ✅ **Skill Book**: Curated skill catalog with activation flow

#### Desktop UI (Beta)
- 🚧 **Dioxus UI**: Native desktop interface with 12 pages and 6 components
- 🚧 **WebSocket Streaming**: Real-time event updates
- 🚧 **Metrics Dashboard**: Performance monitoring

### Test Coverage

- **262+ Unit Tests**: Core functionality coverage
- **61 Integration Tests**: Database, API, and worker integration
- **E2E Tests**: Playwright-based UI testing
- **Benchmarks**: Performance regression testing

### Documentation

- ✅ **README.md**: Comprehensive project overview
- ✅ **ARCHITECTURE.md**: System architecture documentation
- ✅ **DEVELOPMENT.md**: Developer setup guide
- ✅ **TESTING.md**: Complete testing guide
- ✅ **API.md**: API endpoint reference
- ✅ **DOCKER.md**: Docker deployment guide
- ✅ **SECURITY.md**: Security policies and procedures
- ✅ **CONTRIBUTING.md**: Contribution guidelines

### Installation

#### Quick Start

```bash
# Clone repository
git clone https://github.com/kordspace/carnelian.git
cd carnelian

# Build
cargo build --release

# Run setup wizard
./target/release/carnelian init

# Start system
./target/release/carnelian start
```

#### Docker Deployment

```bash
# Standard profile (16GB RAM, 4-8 cores)
docker-compose -f docker-compose.yml -f docker-compose.standard.yml up -d

# Performance profile (32GB+ RAM, 8+ cores, GPU)
docker-compose -f docker-compose.yml -f docker-compose.performance.yml up -d
```

### System Requirements

#### Minimum
- **OS**: Linux, macOS, or Windows (WSL2)
- **RAM**: 8GB
- **CPU**: 4 cores
- **Storage**: 20GB
- **Rust**: 1.85+
- **Docker**: 24.0+

#### Recommended
- **RAM**: 16GB+
- **CPU**: 8+ cores
- **GPU**: NVIDIA RTX 2080+ (8GB VRAM) for local inference
- **Storage**: 50GB SSD

### Breaking Changes

None - this is the initial production release.

### Known Issues

- Desktop UI is in beta and may have rough edges
- Some E2E tests require manual setup
- GPU support requires NVIDIA Container Toolkit

### Migration Guide

For users migrating from Thummim:
1. All 600+ existing skills are compatible via Node worker
2. Database schema is new - export/import memories using the API
3. Configuration format has changed - see `machine.toml.example`

### Security Notes

- Default API key is generated on first run
- All secrets are encrypted at rest using AES-256-GCM
- Ledger provides tamper-resistant audit trail
- Safe mode can be triggered via API or CLI

### Performance

- **Task Execution**: <2s average latency
- **Heartbeat Cycle**: <5s (555,555ms target)
- **API Response**: <100ms (p95)
- **Memory Retrieval**: <500ms with pgvector

### Roadmap

**Next Release (0.2.0)**
- Complete Dioxus desktop UI
- WhatsApp and Slack adapters
- WASM worker enhancements
- Remote deployment tools
- Skill Book library expansion

### Contributors

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines.

### License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.

### Support

- **Documentation**: https://github.com/kordspace/carnelian/tree/main/docs
- **Issues**: https://github.com/kordspace/carnelian/issues
- **Discussions**: https://github.com/kordspace/carnelian/discussions

### Acknowledgments

Built with Rust, Axum, Tokio, SQLx, Dioxus, and many other excellent open-source projects.

---

**Full Changelog**: https://github.com/kordspace/carnelian/commits/main
