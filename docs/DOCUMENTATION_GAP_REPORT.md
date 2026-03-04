# Documentation Gap Analysis Report

**Carnelian Core v1.0.0**  
**Generated:** March 3, 2026  
**Analyst:** Cascade AI

---

## Executive Summary

This report identifies missing documentation in the `docs/` folder and provides recommendations for comprehensive coverage of Carnelian Core's key features. While the existing documentation is strong in areas like MAGIC, API reference, and setup guides, several critical systems lack dedicated deep-dive documentation.

### Key Findings

✅ **Well-Documented:**
- MAGIC quantum entropy system ([MAGIC.md](MAGIC.md))
- API endpoints ([API.md](API.md))
- Setup and installation ([SETUP_*.md](SETUP_WINDOWS.md), [INSTALL.md](INSTALL.md))
- Architecture overview ([ARCHITECTURE.md](ARCHITECTURE.md))
- Security model ([SECURITY.md](SECURITY.md))

❌ **Missing or Incomplete:**
- **Elixir System** — No dedicated guide for dataset injection, brewing, quality scoring
- **Ledger System** — No deep-dive on BLAKE3 hash-chaining, chain anchoring, verification
- **Memory & Context System** — No guide for long-term memory, context assembly, pgvector search
- **XP Progression** — No detailed guide on XP curve, level progression, skill metrics
- **Mantra System** — Limited documentation on mantra selection, cooldowns, context weighting
- **Session Management** — No guide for soul files, session lifecycle, compaction
- **Worker System** — Limited documentation on JSONL protocol, attestation, lifecycle

---

## Detailed Gap Analysis

### 1. Elixir System ⚠️ **HIGH PRIORITY**

**Current State:**
- API.md has endpoint reference (lines 809-1000)
- CHANGELOG.md mentions features (lines 83-92)
- No dedicated documentation file

**Missing Content:**
- **Dataset structure and design patterns** — How to structure elixir datasets for different types
- **Brewing system** — Auto-draft generation from task patterns, threshold configuration
- **Quality scoring algorithm** — How scores are computed, what influences them
- **Injection capabilities** — How elixirs are injected into context (semantic search, skill-linked, mantra-referenced)
- **Builder creation workflow** — Step-by-step guide for creating effective elixirs
- **Quantum checksum integration** — How MAGIC enhances elixir integrity
- **Best practices** — Dataset size limits, versioning strategy, archival policies

**Recommendation:**
- ✅ **Created:** `docs/ELIXIR_SYSTEM.md` — Comprehensive guide covering all aspects

---

### 2. Ledger System ⚠️ **HIGH PRIORITY**

**Current State:**
- SECURITY.md mentions ledger briefly (lines 45-60)
- ARCHITECTURE.md references hash-chain (lines 120-135)
- No dedicated documentation file

**Missing Content:**
- **BLAKE3 hash-chain design** — Why BLAKE3 over SHA-256, chain structure, genesis entry
- **Quantum entropy salting** — How quantum salt enhances integrity, provider waterfall
- **Chain anchoring** — Merkle root computation, slice verification, anchor creation
- **Event types catalog** — Complete list of ledger event types and payload schemas
- **Verification procedures** — Full chain vs. slice verification, tamper detection
- **XP ledger integration** — How XP awards are ledger-backed, event sourcing
- **Desktop UI viewer** — Ledger page features, real-time streaming, export

**Recommendation:**
- ✅ **Created:** `docs/LEDGER_SYSTEM.md` — Comprehensive guide covering all aspects

---

### 3. Memory & Context System ⚠️ **MEDIUM PRIORITY**

**Current State:**
- ARCHITECTURE.md mentions memory manager (lines 85-95)
- API.md has memory endpoints (lines 450-520)
- No dedicated documentation file

**Missing Content:**
- **Long-term memory persistence** — How memories are stored, retrieved, and aged
- **pgvector similarity search** — Embedding generation, cosine similarity, index tuning
- **Context assembly pipeline** — How context is built for LLM prompts, memory retrieval
- **Memory tagging** — Tag-based categorization, filtering, search
- **Memory compaction** — Automatic summarization, archival, deletion policies
- **Cross-instance portability** — CBOR envelopes, signature verification, sync

**Recommendation:**
- **Create:** `docs/MEMORY_SYSTEM.md` — Dedicated guide for memory and context management

**Suggested Sections:**
1. Overview — Memory lifecycle, storage, retrieval
2. pgvector Integration — Embeddings, similarity search, indexing
3. Context Assembly — Prompt building, memory injection, relevance scoring
4. Memory Tagging — Tag schema, filtering, search strategies
5. Compaction & Archival — Automatic summarization, retention policies
6. Cross-Instance Sync — CBOR envelopes, signature verification
7. API Reference — Quick reference to memory endpoints
8. Best Practices — Memory hygiene, performance optimization

---

### 4. XP Progression System ⚠️ **MEDIUM PRIORITY**

**Current State:**
- README.md mentions XP curve (lines 550-570)
- CHANGELOG.md lists XP features (lines 54-55)
- No dedicated documentation file

**Missing Content:**
- **XP curve mathematics** — 1.172 exponent formula, level thresholds (1-99)
- **XP sources** — Task completion, skill usage, elixir quality, manual awards
- **Skill metrics** — Execution count, success rate, XP earned per skill
- **Ledger integration** — Event sourcing, auditability, XP ledger table
- **Leaderboard** — Ranking, filtering, time-based views
- **Desktop UI** — XP progression page, level-up notifications, skill metrics

**Recommendation:**
- **Create:** `docs/XP_SYSTEM.md` — Dedicated guide for XP progression

**Suggested Sections:**
1. Overview — XP system purpose, level progression
2. XP Curve — Mathematical formula, level thresholds, retuning
3. XP Sources — Task completion, skill usage, elixir quality, manual awards
4. Skill Metrics — Execution tracking, success rate, XP per skill
5. Ledger Integration — Event sourcing, auditability, xp_ledger table
6. Leaderboard — Ranking, filtering, API endpoints
7. Desktop UI — XP progression page, notifications, visualizations
8. API Reference — Quick reference to XP endpoints

---

### 5. Mantra System ⚠️ **MEDIUM PRIORITY**

**Current State:**
- MAGIC.md covers mantra matrix (lines 180-250)
- API.md has mantra endpoints (lines 1250-1480)
- Adequate coverage but could be expanded

**Missing Content:**
- **Mantra selection algorithm** — Weighted category selection, inverse frequency
- **Cooldown enforcement** — Per-category cooldowns, configuration
- **Context weighting** — Dynamic weight computation based on pending tasks, errors
- **Elixir linking** — How mantras reference elixirs for enhanced context
- **Mantra authoring guide** — Best practices for writing effective mantras
- **Simulation mode** — Testing mantra selection without side effects

**Recommendation:**
- **Expand:** `docs/MAGIC.md` — Add dedicated "Mantra System Deep Dive" section

**Suggested Additions:**
1. Mantra Selection Algorithm — Detailed walkthrough of weighted selection
2. Cooldown System — Configuration, enforcement, bypass conditions
3. Context Weighting — Dynamic weight computation, influencing factors
4. Elixir Linking — How to link mantras to elixirs, injection behavior
5. Authoring Guide — Best practices, examples, anti-patterns
6. Simulation Mode — Testing and debugging mantra selection

---

### 6. Session Management ⚠️ **LOW PRIORITY**

**Current State:**
- ARCHITECTURE.md mentions sessions (lines 95-105)
- API.md has session endpoints (lines 380-420)
- Basic coverage exists

**Missing Content:**
- **Soul file format** — TOML structure, personality parameters, customization
- **Session lifecycle** — Creation, activation, compaction, archival
- **Message compaction** — Automatic summarization, token budget management
- **Session restart** — Crash recovery, state restoration
- **Multi-session management** — Concurrent sessions, isolation

**Recommendation:**
- **Create:** `docs/SESSION_MANAGEMENT.md` — Dedicated guide for sessions and soul files

**Suggested Sections:**
1. Overview — Session purpose, soul files, lifecycle
2. Soul File Format — TOML structure, personality parameters
3. Session Lifecycle — Creation, activation, compaction, archival
4. Message Compaction — Automatic summarization, token management
5. Session Restart — Crash recovery, state restoration
6. Multi-Session Management — Concurrent sessions, isolation
7. API Reference — Quick reference to session endpoints

---

### 7. Worker System ⚠️ **LOW PRIORITY**

**Current State:**
- ARCHITECTURE.md covers worker manager (lines 110-130)
- WASM_SKILLS.md covers WASM workers
- RUST_SKILL_SYSTEM.md covers native workers
- Basic coverage exists

**Missing Content:**
- **JSONL transport protocol** — Message format, framing, error handling
- **Worker attestation** — Ed25519 verification, trust establishment
- **Worker lifecycle** — Startup, health checks, shutdown, crash recovery
- **Resource limits** — Memory, CPU, timeout enforcement
- **Multi-runtime coordination** — Node.js, Python, WASM, native Rust

**Recommendation:**
- **Create:** `docs/WORKER_SYSTEM.md` — Dedicated guide for worker orchestration

**Suggested Sections:**
1. Overview — Worker architecture, multi-runtime support
2. JSONL Transport Protocol — Message format, framing, error handling
3. Worker Attestation — Ed25519 verification, trust establishment
4. Worker Lifecycle — Startup, health checks, shutdown, recovery
5. Resource Limits — Memory, CPU, timeout enforcement
6. Multi-Runtime Coordination — Node.js, Python, WASM, Rust
7. Skill Development — Creating skills for different runtimes
8. API Reference — Quick reference to worker endpoints

---

## Priority Matrix

| Documentation | Priority | Complexity | Impact | Status |
|---------------|----------|------------|--------|--------|
| Elixir System | **HIGH** | High | High | ✅ Created |
| Ledger System | **HIGH** | Medium | High | ✅ Created |
| Memory & Context | **MEDIUM** | High | Medium | ❌ Missing |
| XP Progression | **MEDIUM** | Low | Medium | ❌ Missing |
| Mantra System | **MEDIUM** | Medium | Low | ⚠️ Expand MAGIC.md |
| Session Management | **LOW** | Medium | Low | ❌ Missing |
| Worker System | **LOW** | High | Low | ⚠️ Partial (WASM, Rust) |

---

## README.md Integration

The following sections in `README.md` should reference the new documentation:

### Elixir System References

**Current Location:** Line 84 (Advanced Features section)

**Suggested Addition:**
```markdown
- ✅ 🧪 Elixir system for knowledge persistence — [Learn more](docs/ELIXIR_SYSTEM.md)
```

**Current Location:** Line 650-680 (Elixir API section)

**Suggested Addition:**
```markdown
For comprehensive documentation on dataset structure, brewing workflows, and injection capabilities, see [docs/ELIXIR_SYSTEM.md](docs/ELIXIR_SYSTEM.md).
```

### Ledger System References

**Current Location:** Line 775-778 (Security Architecture Notes)

**Suggested Replacement:**
```markdown
The ledger uses **BLAKE3** hash-chaining for tamper-resistant audit trails, with optional quantum entropy salting from the MAGIC subsystem. For complete documentation on chain verification, anchoring, and event types, see [docs/LEDGER_SYSTEM.md](docs/LEDGER_SYSTEM.md).
```

**Current Location:** Line 950-955 (Technical Deep Dives table)

**Suggested Addition:**
```markdown
| [docs/LEDGER_SYSTEM.md](docs/LEDGER_SYSTEM.md) | Ledger hash-chain and audit trail |
| [docs/ELIXIR_SYSTEM.md](docs/ELIXIR_SYSTEM.md) | Elixir knowledge persistence system |
```

---

## Recommended Actions

### Immediate (v1.0.0 Release)

1. ✅ **Create `docs/ELIXIR_SYSTEM.md`** — Comprehensive elixir documentation
2. ✅ **Create `docs/LEDGER_SYSTEM.md`** — Comprehensive ledger documentation
3. ✅ **Update `README.md`** — Add references to new documentation files

### Short-Term (v1.0.1)

4. **Create `docs/MEMORY_SYSTEM.md`** — Memory and context management guide
5. **Create `docs/XP_SYSTEM.md`** — XP progression and skill metrics guide
6. **Expand `docs/MAGIC.md`** — Add "Mantra System Deep Dive" section

### Long-Term (v1.1.0)

7. **Create `docs/SESSION_MANAGEMENT.md`** — Session lifecycle and soul files
8. **Create `docs/WORKER_SYSTEM.md`** — Worker orchestration and JSONL protocol
9. **Create `docs/TROUBLESHOOTING.md`** — Common issues and solutions
10. **Create `docs/PERFORMANCE_TUNING.md`** — Optimization guide

---

## Conclusion

The Carnelian Core documentation is strong in foundational areas (setup, API, security), but lacks deep-dive guides for several key systems. The creation of `ELIXIR_SYSTEM.md` and `LEDGER_SYSTEM.md` addresses the highest-priority gaps, providing comprehensive coverage of two critical features that were previously under-documented.

Recommended next steps:
1. ✅ Review and merge `ELIXIR_SYSTEM.md` and `LEDGER_SYSTEM.md`
2. ✅ Update `README.md` with references to new documentation
3. Create `MEMORY_SYSTEM.md` and `XP_SYSTEM.md` for v1.0.1
4. Expand `MAGIC.md` with mantra system deep-dive
5. Plan long-term documentation roadmap for v1.1.0

---

**Report Generated By:** Cascade AI  
**Review Status:** Pending  
**Next Review:** v1.0.1 planning

---

## Appendix: Documentation File Inventory

### Existing Documentation (23 files)

| File | Size | Last Updated | Coverage |
|------|------|--------------|----------|
| API.md | 33KB | 2026-03-03 | ✅ Excellent |
| ARCHITECTURE.md | 14KB | 2026-03-03 | ✅ Good |
| ATTESTATION.md | 5.7KB | 2026-03-03 | ✅ Good |
| BRAND.md | 11KB | 2026-03-03 | ✅ Excellent |
| CHANGELOG.md | 14KB | 2026-03-03 | ✅ Excellent |
| DEVELOPMENT.md | 5KB | 2026-03-03 | ✅ Good |
| DOCKER.md | 6.8KB | 2026-03-03 | ✅ Good |
| DOCKER_ECOSYSTEM.md | 10KB | 2026-03-03 | ✅ Good |
| GETTING_STARTED.md | 4.6KB | 2026-03-03 | ✅ Good |
| INSTALL.md | 5.7KB | 2026-03-03 | ✅ Good |
| LOGGING.md | 10KB | 2026-03-03 | ✅ Excellent |
| MAGIC.md | 19KB | 2026-03-03 | ✅ Excellent |
| OPERATOR_GUIDE.md | 6.9KB | 2026-03-03 | ✅ Good |
| README.md | 4.2KB | 2026-03-03 | ✅ Good |
| REMOTE_DEPLOY.md | 4.6KB | 2026-03-03 | ✅ Good |
| RUST_SKILL_SYSTEM.md | 8.9KB | 2026-03-03 | ✅ Good |
| SECURITY.md | 9.4KB | 2026-03-03 | ✅ Good |
| SETUP_LINUX.md | 8.5KB | 2026-03-03 | ✅ Good |
| SETUP_MACOS.md | 5.5KB | 2026-03-03 | ✅ Good |
| SETUP_WINDOWS.md | 6.3KB | 2026-03-03 | ✅ Good |
| TESTING.md | 8.7KB | 2026-03-03 | ✅ Good |
| V1.0.0_VERIFICATION_REPORT.md | 21KB | 2026-03-03 | ✅ Excellent |
| WASM_SKILLS.md | 7.2KB | 2026-03-03 | ✅ Good |

### New Documentation (Created)

| File | Size | Status |
|------|------|--------|
| ELIXIR_SYSTEM.md | 25KB | ✅ Created |
| LEDGER_SYSTEM.md | 22KB | ✅ Created |
| DOCUMENTATION_GAP_REPORT.md | 15KB | ✅ Created |

### Planned Documentation (Future)

| File | Priority | Target Version |
|------|----------|----------------|
| MEMORY_SYSTEM.md | Medium | v1.0.1 |
| XP_SYSTEM.md | Medium | v1.0.1 |
| SESSION_MANAGEMENT.md | Low | v1.1.0 |
| WORKER_SYSTEM.md | Low | v1.1.0 |
| TROUBLESHOOTING.md | Low | v1.1.0 |
| PERFORMANCE_TUNING.md | Low | v1.1.0 |
