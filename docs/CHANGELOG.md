# Changelog

All notable changes to Carnelian OS will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] — Phase 10D: Quantum Integrity

### Added

- `quantum_checksum TEXT` column on `memories`, `session_messages`, `elixirs`, and `task_runs` tables (migration `00000000000017_quantum_integrity.sql`), with partial indexes `WHERE quantum_checksum IS NOT NULL`.
- `QuantumHasher` in `carnelian-magic` — `compute`, `verify`, and `batch_compute` using BLAKE3 with MAGIC-mixed entropy salt.
- `QuantumIntegrityVerifier` in `carnelian-magic` — `verify_table`, `verify_row`, and `backfill_missing` returning `VerificationReport` and `TamperedRow`.
- Three new REST API endpoints (all behind `X-Carnelian-Key` auth):
  - `POST /v1/magic/integrity/verify` — Verify quantum checksums for specified tables
  - `GET /v1/magic/integrity/status` — Get cached integrity verification status
  - `POST /v1/magic/integrity/backfill` — Backfill missing quantum checksums in background
- Quantum checksum population wired into all four core write paths: `MemoryManager::create_memory`, `SessionManager::append_message`, `ElixirManager::create_elixir` / `approve_draft`, and task run completion in `scheduler.rs`.

### Known Limitations

- **Ed25519 → ML-DSA migration is post-v1.** Ed25519 signatures (owner keypair, privileged ledger actions) are not quantum-resistant. Migration to ML-DSA (CRYSTALS-Dilithium, NIST FIPS 204) is targeted for a post-v1 release. All other cryptographic primitives (BLAKE3, AES-256-GCM) are already post-quantum safe.

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

#### Security & Ledger Features
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
- Production validation test suite (`production_validation_test.rs`)
- Cross-instance memory export/import roundtrip tests
- Chain anchor publication and verification tests
- Capability enforcement and revocation tests
- Performance tests for task latency and heartbeat
- E2E tests with Playwright for desktop UI

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
