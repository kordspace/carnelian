# Changelog

All notable changes to Carnelian OS will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-24

### Added

#### Core Features
- XP system with experience point tracking, leaderboards, and skill metrics
- Voice gateway with ElevenLabs TTS and OpenAI Whisper STT integration
- Cross-instance memory portability with CBOR-encoded envelopes
- Capability-based security with granular grants and approval queues
- Ledger hash chain with tamper-resistant audit logging
- Heartbeat system with database-backed monitoring
- Safe mode with side-effect blocking

#### Checkpoint 3 Features
- **Chain Anchor System**
  - `chain_anchors` database table for storing ledger slice anchors
  - `LocalDbChainAnchor` implementation of the `ChainAnchor` trait
  - `publish_ledger_anchor()` method for computing merkle roots and anchoring
  - REST API endpoints: `POST /v1/ledger/anchor`, `GET /v1/ledger/anchor/:id`
  - External verification endpoint: `GET /v1/ledger/anchor/:id/verify`

- **Cross-Instance Grant Revocation**
  - `revoked_capability_grants` table for persistent revocation records
  - `PolicyEngine::is_grant_revoked()` helper for checking revocation status
  - `PolicyEngine::list_revoked_since()` for cross-instance sync
  - Revocation enforcement during memory import
  - REST API endpoint: `GET /v1/memory/revoked-grants?since=<timestamp>`

- **Topic-Scoped Capability Grants**
  - `PolicyEngine::check_memory_topic_capability()` for topic-based access control
  - Topic filter metadata propagation on memory export
  - Topic capability enforcement during memory import
  - Scoped grant recreation with revocation checking

#### Testing
- Checkpoint 3 validation test suite (`checkpoint3_validation_test.rs`)
- Cross-instance memory export/import roundtrip tests
- Chain anchor publication and verification tests
- Capability enforcement and revocation tests
- Performance tests for task latency and heartbeat

#### Documentation
- `RELEASE_NOTES.md` with highlights and known limitations
- `docs/ARCHITECTURE.md` with system design documentation
- `scripts/demo_script.md` with 5-minute walkthrough
- SHA256 checksum generation in `scripts/package.sh`

#### CLI
- `carnelian init` - Interactive setup wizard
- `carnelian start` - Start the server
- `carnelian ui` - Launch the desktop UI
- `carnelian keygen` - Generate owner keypair
- `carnelian key rotate` - Rotate owner keys
- `carnelian migrate-from-thummim` - Import from Thummim

### Security
- Ed25519 signatures on privileged ledger actions
- Blake3 hash chaining for tamper detection
- Capability-based access control for all sensitive operations
- Approval queue for high-risk operations
- Safe mode for blocking side effects during recovery
- Cross-instance revocation propagation for capability grants

### Infrastructure
- PostgreSQL 16 with pgvector extension
- Database migrations with sqlx
- Testcontainers for integration testing
- GitHub Actions CI/CD pipeline
- Docker and docker-compose support
- Packaging script with SHA256 checksums

### Known Limitations
- Python Worker: Stub implementation only
- Shell Worker: Stub implementation only
- WASM Worker: Not implemented
- WhatsApp Adapter: Not implemented
- Advanced workflow loops: Not implemented
- Cloud deployment: Single-node only
- Decentralized memory network: Interfaces defined only

## References

- [Release Notes](../RELEASE_NOTES.md) — Detailed release information
- [Architecture](ARCHITECTURE.md) — System design documentation
- [README](../README.md) — Quick start guide
