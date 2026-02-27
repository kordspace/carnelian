# 🔥 Carnelian OS Release Notes

## v0.1.0 — Initial Release

**Release Date:** 2026-02-24  
**SHA256 Checksum:** (see carnelian-os-v0.1.0.tar.gz.sha256)

---

## 🎯 Highlights

### XP System
- Experience point tracking for all agents
- Skill-based XP accrual with configurable multipliers
- Leaderboard and agent progression visualization
- Historical XP tracking with time-series queries

### Voice Gateway
- ElevenLabs integration for text-to-speech (TTS)
- OpenAI Whisper integration for speech-to-text (STT)
- Voice selection and configuration
- Test endpoints for voice configuration validation

### Cross-Instance Memory Portability
- Export/import memories as CBOR-encoded envelopes
- Blake3 content hashing with signature verification
- Ledger proof material for audit trail
- Capability-based access control on exported memories
- **NEW:** Chain anchor support for external verification
- **NEW:** Topic-scoped capability grants for selective disclosure
- **NEW:** Cross-instance grant revocation propagation

### Capability-Based Security
- Granular capability grants with scope restrictions
- Approval queue for sensitive operations
- Policy engine with database-backed enforcement
- **NEW:** Revoked grants tracking for cross-instance sync
- **NEW:** Topic-scoped memory access control

### Ledger Hash Chain
- Tamper-resistant audit ledger with Blake3 hash chaining
- Ed25519 signatures on privileged operations
- Chain verification on startup
- **NEW:** Ledger slice anchoring for external verification
- **NEW:** Merkle root computation over event ranges

### Heartbeat System
- Database-backed heartbeat history
- Health monitoring and alerting hooks
- Configurable heartbeat intervals
- Integration with safe mode detection

### Dioxus UI
- Desktop application with real-time event streaming
- Task creation and monitoring interface
- Memory search and visualization
- XP progress and leaderboard display

---

## ⚠️ Known Limitations

### Workers
- **Python Worker:** Stub implementation only; full subprocess-based execution with seccomp sandboxing is planned for v0.2.0
- **Shell Worker:** Stub implementation only; safe-mode integration pending
- **WASM Worker:** Not implemented; `wasmtime` integration planned for v0.2.0

### Adapters
- **WhatsApp Adapter:** Not implemented; Meta Business Cloud API integration planned for v0.2.0

### Workflow Engine
- **Advanced Loops:** `workflow.rs` lacks loop/retry/conditional branching; planned for v0.2.0

### Deployment
- **Cloud Deployment:** Currently single-node only; Kubernetes manifests planned for v0.3.0
- **Decentralized Memory Network:** Interfaces defined, gossip protocol over libp2p planned for v0.3.0

---

## 🔄 Upgrade Path

N/A — This is the initial release.

---

## 📦 Installation

```bash
# Download and verify
curl -LO https://github.com/carnelian-os/releases/download/v0.1.0/carnelian-os-v0.1.0.tar.gz
curl -LO https://github.com/carnelian-os/releases/download/v0.1.0/carnelian-os-v0.1.0.tar.gz.sha256
sha256sum -c carnelian-os-v0.1.0.tar.gz.sha256

# Install
tar -xzf carnelian-os-v0.1.0.tar.gz
cd carnelian-os-v0.1.0
sudo ./install.sh

# Initialize
carnelian init
carnelian start
```

---

## 📚 Documentation

- [README.md](README.md) — Quick start and overview
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — System architecture
- [docs/CHANGELOG.md](docs/CHANGELOG.md) — Detailed change history
- [scripts/demo_script.md](scripts/demo_script.md) — 5-minute walkthrough

---

## 🔗 Links

- Repository: https://github.com/carnelian-os/carnelian
- Issues: https://github.com/carnelian-os/carnelian/issues
- Discussions: https://github.com/carnelian-os/carnelian/discussions

---

*Built with 🔥 by the Carnelian OS team.*
