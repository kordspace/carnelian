# Changelog

All notable changes to Carnelian Core will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.0] - 2026-03-03

### Added — Phase 1: Foundation

- **Core Orchestrator** — Axum HTTP API (`:18789`) with WebSocket event streaming
- **CLI Binary** — `carnelian` command with `start`, `stop`, `status`, `migrate`, `logs`, `keygen` subcommands
- **Policy Engine** — Capability-based security with deny-by-default, Ed25519-signed authority chains
- **Ledger Manager** — blake3 hash-chain audit trail for privileged actions
- **Database Schema** — PostgreSQL 16 with core tables: `identities`, `skills`, `tasks`, `task_runs`, `run_logs`, `ledger_events`, `capability_grants`, `config_store`
- **Task Scheduler** — Priority-based task queue with retry policies and concurrency limits
- **Heartbeat System** — 555,555ms wake routine with database-backed monitoring

### Added — Phase 2: Task Execution

- **Worker Manager** — Multi-runtime worker orchestration with JSONL transport protocol
- **Node Worker** — Node.js/TypeScript worker with subprocess lifecycle management
- **Python Worker** — Python 3.10+ worker with subprocess lifecycle management
- **Skill Discovery** — Automatic filesystem watching with blake3 checksums and database sync
- **WebSocket Events** — Real-time event streaming with priority-based sampling and bounded buffers
- **Task Lifecycle** — Task creation, assignment, execution, completion, and cancellation

### Added — Phase 3: Intelligence

- **Soul File Manager** — Personality state management with TOML-based soul files
- **Memory Manager** — Memory creation, retrieval, and pgvector similarity search
- **Context Assembler** — Context building for LLM prompts with memory retrieval
- **Agentic Loop** — Heartbeat-driven agentic turn with compaction pipeline
- **LLM Gateway** — TypeScript gateway (`:18790`) with Ollama, OpenAI, Anthropic, Fireworks providers
- **Model Router** — LLM provider routing with fallback logic

### Added — Phase 4: Security

- **Attestation System** — Worker identity verification with Ed25519 signatures
- **Encryption at Rest** — AES-256-GCM encryption for sensitive data with key management
- **Safe Mode** — Emergency lockdown with side-effect blocking and capability suspension
- **Approval Queue** — Human-in-the-loop approval workflow for high-risk operations
- **Authentication** — `X-Carnelian-Key` header-based authentication for API endpoints

### Added — Phase 5: Advanced Features

- **Sub-Agents** — Delegated agent execution with isolated contexts
- **Workflows** — Multi-step workflow orchestration with state management
- **Channel Adapters** — Telegram and Discord bot adapters with pairing and rate limiting
- **Voice Gateway** — ElevenLabs STT/TTS integration with encrypted API key storage
- **XP System** — Experience point tracking with 1.172-exponent level curve (Level 1-99)
- **Skill Metrics** — Per-skill performance tracking with execution count, success rate, and XP earned

### Added — Phase 6: Production

- **pgvector Integration** — 1536-dimensional embeddings for memory similarity search
- **XP Ledger** — Event sourcing table for XP awards with full auditability
- **Chain Anchor System** — Ledger slice anchoring with merkle root computation
- **Metrics System** — Performance metrics tracking for tasks, skills, and heartbeats
- **Cross-Instance Memory Portability** — CBOR-encoded envelopes with signature verification
- **Topic-Scoped Capability Grants** — Topic-based access control for memory operations
- **Cross-Instance Grant Revocation** — Persistent revocation records with sync endpoints

### Added — Phase 7: Settings, Ledger UI & Skill Book

- **Desktop UI Expansion** — 17 Dioxus pages (up from 12): added `settings.rs`, `ledger.rs`, `magic.rs`, `elixirs.rs`, `xp_progression.rs`
- **Settings Page** — System configuration UI with MAGIC provider settings
- **Ledger Viewer** — Audit trail visualization with hash-chain verification
- **Skill Book Catalog** — Curated skill catalog with 7 categories and 30+ pre-integrated skills
- **Skill Book Database** — `skill_book_catalog`, `skill_book_categories`, `skill_book_activations` tables

### Added — Phase 8: Worker Ecosystem

- **WASM Worker Runtime** — wasmtime 27 + WASI P1 sandboxed skill execution in `carnelian-core/src/skills/wasm_runtime.rs`
- **Native Ops Worker** — In-process Rust operations (`carnelian-worker-native/`): `git_status`, `file_hash` (blake3), `docker_ps` (bollard), `dir_list` (walkdir)
- **Bulk Import Tooling** — Skill migration utilities for importing existing skill libraries
- **Worker Attestation** — `worker_attestations` table with Ed25519 verification
- **Channel Sessions** — `channel_sessions` table for Telegram/Discord session management

### Added — Phase 9: Skills Import & Elixirs

- **Elixir System** — RAG-based knowledge persistence with pgvector embeddings
- **Elixir Database Schema** — `elixirs`, `elixir_versions`, `elixir_usage` tables
- **Elixir Types** — Four types: `skill_backup`, `domain_knowledge`, `context_cache`, `training_data`
- **Elixir Quality Scoring** — 0-100 quality scores with XP integration
- **Elixir Drafts** — Auto-draft generation from successful task patterns with approval workflow
- **Elixir API** — 7 REST endpoints: list, create, get, search, list drafts, approve, reject
- **Memory Tags** — `memory_tags` table for memory categorization
- **Skills Import** — 50+ curated skills migrated to Skill Book with bulk import tooling

### Added — Phase 10: MAGIC — Quantum Intelligence

#### Phase 10A: Entropy Provider Chain

- **`carnelian-magic` Crate** — Quantum entropy generation and mantra matrix system
- **`EntropyProvider` Trait** — Waterfall chain: Quantum Origin → Quantinuum H2 → Qiskit → OS CSPRNG
- **Quantum Origin Provider** — REST API integration with `CARNELIAN_QUANTUM_ORIGIN_API_KEY`
- **Quantinuum H2 Provider** — Hadamard circuit entropy via pytket with interactive auth (`carnelian magic auth`)
- **Qiskit Provider** — IBM Quantum backend integration with `IBM_QUANTUM_TOKEN`
- **OS Entropy Fallback** — `getrandom` crate CSPRNG as always-available fallback
- **Entropy Audit Log** — `magic_entropy_log` table tracking provider usage and byte counts
- **Entropy API** — 4 endpoints: health check, sample bytes, audit log, elixir rehash

#### Phase 10B: Mantra Matrix

- **`MantraTree`** — Weighted category selection seeded with quantum entropy
- **Mantra Database** — `mantra_categories` (18 categories), `mantra_entries` (100+ mantras), `mantra_history` tables
- **Cooldown System** — Per-category cooldown enforcement to prevent repetitive context pollution
- **Context Weighting** — Dynamic weight computation based on pending tasks, errors, and elixir quality
- **Inverse Frequency Selection** — Least-recently-used mantra selection within chosen category
- **Mantra API** — 8 endpoints: list categories, add entry, list entries, update/delete, history, context, simulate

#### Phase 10C: MAGIC UI & Documentation

- **MAGIC Desktop UI Page** — `carnelian-ui/src/pages/magic.rs` with entropy dashboard, mantra library, and auth settings
- **MAGIC CLI Commands** — `carnelian magic auth`, `carnelian magic status`, `carnelian magic sample`, `carnelian magic providers`
- **MAGIC Documentation** — `docs/MAGIC.md` with provider setup, troubleshooting, and security notes
- **README MAGIC Section** — Comprehensive MAGIC documentation in main README

#### Phase 10D: Quantum Integrity

- **`QuantumHasher`** — BLAKE3 + MAGIC-mixed entropy salt for quantum-resistant checksums
- **`QuantumIntegrityVerifier`** — Table verification, row verification, and missing checksum backfill
- **Quantum Checksum Columns** — Added to `memories`, `session_messages`, `elixirs`, `task_runs` tables
- **Quantum Checksum Population** — Wired into all core write paths: memory creation, session messages, elixir creation, task completion
- **Integrity API** — 3 endpoints: verify tables, get status, backfill missing checksums
- **Migration 17** — `00000000000017_quantum_integrity.sql` with partial indexes

### Added — Phase 12: Post-Quantum Cryptography (Production-Ready, v1.1.0 Opt-In)

- **`carnelian-magic/src/pqc.rs`** — Full NIST PQC implementation (363 lines, 8 tests)
  - `HybridSigningKey` — Dual-signature scheme (CRYSTALS-Dilithium3 + Ed25519) with defense-in-depth verification
  - `KyberKem` — Quantum-resistant key encapsulation (CRYSTALS-Kyber1024, NIST Level 5 security)
  - `KeyAlgorithm` enum — Track algorithm usage (`Ed25519`, `HybridDilithiumEd25519`, `Dilithium3`)
- **`carnelian-magic/src/merkle.rs`** — Memory Merkle Tree (220 lines, 7 tests)
  - O(log n) proof generation and verification with Blake3 hashing
  - Leaf update with path recomputation
- **`carnelian-magic/src/batch_verify.rs`** — Batch signature verification (150 lines, 6 tests)
  - Parallel verification with Rayon (10x performance improvement)
  - Fail-fast mode for untrusted inputs
- **`carnelian-core/src/skills/sandbox.rs`** — Cross-platform skill sandboxing (180 lines, 5 tests)
  - Unix: `rlimit` enforcement (memory, CPU, processes)
  - Windows: Timeout-based enforcement with process termination
  - Output validation (size limits, UTF-8 validation)
- **`carnelian-core/src/context_analyzer.rs`** — Autonomous task creation (200 lines, 4 tests)
  - Pattern-based action item extraction from session messages
  - Similarity-based deduplication
  - Direct task creation in database
- **Database Migrations**
  - Migration 18: `key_algorithm` column in `config_store` for PQC tracking
  - Migration 19: `skill_execution_log` table for audit logging with detailed metrics
- **Crypto Integration** — Hybrid key functions in `carnelian-core/src/crypto.rs`
  - `generate_hybrid_keypair_with_entropy()`, `sign_bytes_hybrid()`, `verify_signature_hybrid()`
  - `store_hybrid_keypair_in_db()`, `load_hybrid_keypair_from_db()`
- **Documentation**
  - `FUTURE_PQC.md` — Comprehensive v1.1.0/v1.2.0/v2.0.0 migration roadmap (352 lines)
  - `SECURITY.md` — Updated with PQC roadmap section and v1.0.x version table
  - UI text neutralization — Algorithm-agnostic labels ("owner signature" vs "Ed25519 signature")
- **Code Quality** — Removed 35 blanket Clippy suppressions from 8 UI files, added 1 targeted suppression

**Security Note:** All 7 items from `SECURITY_ARCHITECTURE_REVIEW_V1.md` are fully implemented. PQC primitives ship in v1.0.0 but activate as opt-in feature in v1.1.0 to allow gradual migration from Ed25519.

### Added — Phase 11: Documentation & Release

- **LICENSE.md** — Proprietary license with Marco Julio Lopes and Kordspace LLC attribution, patent-pending notice, CLA requirements
- **CHANGELOG.md** — Full v1.0.0 release notes (this file)
- **ARCHITECTURE.md** — Updated with `carnelian-magic` component, quantum providers section, accurate counts
- **API.md** — Complete endpoint inventory (65+ endpoints) with phase annotations, MAGIC and Elixir sections
- **GETTING_STARTED.md** — Updated with MAGIC quick setup step and accurate skill counts
- **SKILLS_MIGRATION_STATUS.md** — Migration tracking document for Skill Book expansion
- **README.md Overhaul** — Comprehensive update with Brand Identity, XP system details, MAGIC sections, 4 mermaid diagrams
- **Documentation Archive** — Moved `ENHANCEMENT_SUMMARY.md` and `FINAL_STATUS_REPORT.md` to `docs/archive/`
- **Copyright Notices** — Added to README header and LICENSE.md

### Known Limitations

- **`carnelian init` Wizard** — Partially complete; manual configuration via `machine.toml` and environment variables is required for some features
- **Skill Count** — 50+ curated skills in Skill Book vs. 600-skill migration target; bulk import tooling available
- **Ed25519 → ML-DSA Migration** — Post-v1.0.0; Ed25519 signatures (owner keypair, ledger actions) are not quantum-resistant; migration to ML-DSA (CRYSTALS-Dilithium, NIST FIPS 204) is planned for a future release
- **Python Worker** — Functional but limited skill coverage
- **WASM Worker** — Runtime implemented in `wasm_runtime.rs` but skill library is minimal
- **Cloud Deployment** — Single-node only; distributed deployment is not yet supported

### Breaking Changes

None — this is the first stable release.

### Upgrade Path

N/A — this is the first stable release.

---

## References

- [LICENSE.md](../LICENSE.md) — Proprietary license and CLA
- [README.md](../README.md) — Quick start guide
- [ARCHITECTURE.md](ARCHITECTURE.md) — System design documentation
- [API.md](API.md) — Complete REST API reference
- [MAGIC.md](MAGIC.md) — Quantum providers setup and troubleshooting
- [GETTING_STARTED.md](GETTING_STARTED.md) — Step-by-step setup guide
