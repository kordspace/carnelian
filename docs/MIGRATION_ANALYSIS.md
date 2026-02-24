# Carnelian OS - Skill Migration & TypeScript Cleansing Plan

## Executive Summary

This report documents the current state of the Carnelian OS codebase, focusing on the migration path from TypeScript/JavaScript services to pure Rust implementations, and provides a comprehensive master list of all systems and features.

**Current State:**
- Core orchestrator: ✅ Rust (Complete)
- UI Layer: 🔄 Rust (Dioxus - In Progress/Complete)
- Workers: ⚠️ Mixed (Node.js/TypeScript + Rust wrappers)
- Gateway: ⚠️ Node.js/TypeScript (Needs migration)
- Skills: ⚠️ Mixed (Node.js runtime for 600+ existing skills)

## 1. Database Migrations Analysis

### Production Migrations (14 files)

| Migration | Description | Skills Impact |
|-----------|-------------|---------------|
| `00000000000000_init.sql` | pgvector extension setup | None |
| `00000000000001_core_schema.sql` | **Core tables including `skills` table** | ✅ Foundational - supports `node`, `python`, `shell`, `wasm` runtimes |
| `00000000000002_phase1_delta.sql` | Sessions, workflows, sub_agents, XP, elixirs | Adds `skill_versions` table for skill versioning |
| `00000000000003_schema_fixes.sql` | Pronouns, subject_id TEXT, LZ4 compression | None |
| `00000000000004_xp_curve_retune.sql` | XP curve rebalancing | None |
| `00000000000005_config_store_value_blob.sql` | Config store improvements | None |
| `00000000000006_memories_created_at_index.sql` | Memory retrieval optimization | None |
| `00000000000007_heartbeat_correlation.sql` | Heartbeat tracking | None |
| `00000000000008_approval_queue.sql` | Approval system | None |
| `00000000000009_encryption_at_rest.sql` | Encryption system | None |
| `00000000000010_worker_attestations.sql` | Worker attestation tracking | **Worker security - for all runtime types** |
| `00000000000011_channel_sessions.sql` | Channel/adapter system | None |
| `00000000000012_memory_tags.sql` | Memory tagging | None |
| `00000000000013_chain_anchors.sql` | Blockchain anchoring | None |
| `00000000000014_revoked_grants.sql` | Capability revocation | **Security for skill execution** |

### Key Schema for Skills

```sql
-- skills table supports multiple runtimes
CREATE TABLE skills (
    skill_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    runtime TEXT NOT NULL CHECK (runtime IN ('node', 'python', 'shell', 'wasm')),
    enabled BOOLEAN NOT NULL DEFAULT true,
    manifest JSONB DEFAULT '{}'::jsonb,
    capabilities_required TEXT[] DEFAULT '{}',
    checksum TEXT,
    signature TEXT,
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**Status:** ✅ The database is already designed to support multiple runtimes including future WASM support.

## 2. TypeScript Services Requiring Migration

### 2.1 Node.js Worker (`workers/node-worker/`)

**Current Implementation:**
- **Size:** ~1,500 lines TypeScript
- **Purpose:** Executes 600+ existing Thummim skills
- **Architecture:** JSON Lines protocol over stdin/stdout
- **Runtime Support:** Node.js, Python, Shell

**Key Components to Migrate:**

| Component | Lines | Purpose | Migration Complexity |
|-----------|-------|---------|---------------------|
| `index.ts` | 312 | Main entry, message routing | Medium |
| `sandbox.ts` | 548 | Skill execution sandbox | **High** |
| `loader.ts` | 265 | Skill discovery & loading | Low |
| `manifest.ts` | ~250 | Manifest parsing & validation | Low |
| `protocol.ts` | ~150 | JSON Lines protocol | Low |
| `events.ts` | ~150 | Event emission | Low |
| `types.ts` | 148 | Type definitions | Low |
| **Tests** | ~800 | Test suite | Medium |

**Migration Strategy Options:**

#### Option A: Full Native Rust Worker (Recommended Long-term)
- Replace Node.js worker with pure Rust worker
- Implement WASM runtime for sandboxed skill execution
- Migrate 600+ skills incrementally to Rust/WASM

**Pros:**
- Single binary deployment
- Memory safety
- Better performance
- No Node.js dependency

**Cons:**
- Massive migration effort for 600+ skills
- Risk of breaking existing skill functionality
- Long development timeline

#### Option B: Keep Node.js Worker, Add WASM Support (Recommended Short-term)
- Keep existing Node.js worker for backward compatibility
- Add WASM runtime support to the existing Rust worker manager
- New skills implemented in Rust/WASM
- Gradually migrate popular skills

**Pros:**
- Backward compatible
- Incremental migration
- Lower risk

**Cons:**
- Still requires Node.js
- Two skill ecosystems

### 2.2 LLM Gateway Service (`gateway/`)

**Current Implementation:**
- **Size:** ~500 lines TypeScript
- **Purpose:** LLM routing (local-first with remote fallback)
- **Providers:** Ollama, OpenAI, Anthropic, Fireworks

**Migration Priority:** 🔴 **HIGH**

This should be the first TypeScript service migrated because:
1. It's a standalone service
2. No skill dependencies
3. Clear boundaries
4. High value (core to system function)

**Migration Approach:**
- Integrate into `carnelian-core` as a module
- Merge with existing `model_router.rs`
- Single deployment artifact

### 2.3 TypeScript Test Files in Node Worker

- `events.test.ts`, `execution.test.ts`, `health.test.ts`, `integration.test.ts`, `manifest.test.ts`, `protocol.test.ts`
- Should be rewritten as Rust tests in `carnelian-worker-node/tests/`

## 3. Clippy Configuration Analysis

### Current Configuration

**Workspace-level (`Cargo.toml`):**
```toml
[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
nursery = "warn"
```

**Crate-level Suppressions (`carnelian-core/src/lib.rs`):**
- 30+ allowed lints for pragmatic development
- Key suppressions:
  - `cast_possible_truncation` - Numeric casting
  - `missing_errors_doc` - Documentation
  - `too_many_arguments` - Function signatures
  - `cognitive_complexity` - Algorithm complexity

**Project-level (`clippy.toml`):**
- Allows dbg/unwrap/expect/print in tests
- Allows arithmetic side effects for `usize`, `u32`, `u64`

### Status
✅ Clippy is properly configured with reasonable exceptions for a production system. The aggressive linting (all + pedantic + nursery) with targeted suppressions is a good balance.

## 4. Comprehensive Feature & Systems Master List

### Core Systems (carnelian-core)

| System | Module | Lines | Status | Description |
|--------|--------|-------|--------|-------------|
| **Server** | `server.rs` | ~6,000 | ✅ | Axum HTTP API + WebSocket |
| **Scheduler** | `scheduler.rs` | ~2,800 | ✅ | Task queue, priority scheduling |
| **Worker Manager** | `worker.rs` | ~2,100 | ✅ | Worker lifecycle, JSONL transport |
| **Policy Engine** | `policy.rs` | ~900 | ✅ | Capability-based security |
| **Ledger** | `ledger.rs` | ~1,100 | ✅ | blake3 hash-chain audit |
| **Memory** | `memory.rs` | ~2,800 | ✅ | RAG with pgvector |
| **Skills** | `skills.rs` | ~700 | ✅ | Discovery, manifest validation |
| **Events** | `events.rs` | ~900 | ✅ | Event streaming with backpressure |
| **Model Router** | `model_router.rs` | ~1,300 | ✅ | LLM routing, local + remote |
| **Config** | `config.rs` | ~2,100 | ✅ | TOML/env/CLI configuration |
| **Agentic** | `agentic.rs` | ~2,600 | ✅ | Agentic execution pipeline |
| **Session** | `session.rs` | ~2,700 | ✅ | Session lifecycle management |
| **Sub-Agent** | `sub_agent.rs` | ~700 | ✅ | Sub-agent management |
| **Workflow** | `workflow.rs` | ~1,400 | ✅ | Workflow engine |
| **XP** | `xp.rs` | ~600 | ✅ | XP progression system |
| **Voice** | `voice.rs` | ~400 | ✅ | ElevenLabs integration |
| **Approvals** | `approvals.rs` | ~650 | ✅ | Approval queue system |
| **Safe Mode** | `safe_mode.rs` | ~200 | ✅ | Safe mode guard |
| **Chain Anchor** | `chain_anchor.rs` | ~400 | ✅ | Blockchain anchoring |
| **Encryption** | `encryption.rs` | ~450 | ✅ | At-rest encryption |
| **Crypto** | `crypto.rs` | ~500 | ✅ | Ed25519 signatures |
| **Attestation** | `attestation.rs` | ~200 | ✅ | Worker attestation |
| **Metrics** | `metrics.rs` | ~350 | ✅ | Performance metrics |
| **Context** | `context.rs` | ~2,200 | ✅ | Context assembly |
| **Soul** | `soul.rs` | ~650 | ✅ | Soul file management |
| **Database** | `db.rs` | ~350 | ✅ | Connection management |

### Adapter Systems (carnelian-adapters)

| System | Module | Status | Description |
|--------|--------|--------|-------------|
| **Telegram** | `telegram/` | ✅ | Telegram bot adapter |
| **Discord** | `discord/` | ✅ | Discord bot adapter |
| **Rate Limiter** | `rate_limiter.rs` | ✅ | Per-user rate limiting |
| **Spam Detector** | `spam_detector.rs` | ✅ | Content filtering |
| **Database** | `db.rs` | ✅ | Channel session storage |
| **Config** | `config.rs` | ✅ | Adapter configuration |

### UI Layer (carnelian-ui)

| Component | Status | Description |
|-----------|--------|-------------|
| **XP Widget** | ✅ | XP progress display |
| **Voice Settings** | ✅ | Voice configuration panel |
| **XP Dashboard** | ✅ | Progression dashboard |

### Common (carnelian-common)

| Module | Status | Description |
|--------|--------|-------------|
| **Types** | ✅ | Shared API types |
| **Channel** | ✅ | Channel adapter trait |
| **Error** | ✅ | Error handling |

### External Services

| Service | Technology | Migration Status |
|---------|------------|------------------|
| **Core Orchestrator** | Rust | ✅ Migrated |
| **UI** | Rust/Dioxus | ✅ Migrated |
| **Node Worker** | Node.js/TS | ⚠️ Needs decision |
| **Python Worker** | Python | ✅ Keep (external) |
| **Shell Worker** | Shell | ✅ Keep (external) |
| **LLM Gateway** | Node.js/TS | 🔴 Needs migration |
| **Adapters** | Rust | ✅ Migrated |

## 5. Migration Recommendations

### Phase 1: Gateway Migration (Immediate Priority)

**Target:** `gateway/` TypeScript service
**Approach:** Integrate into `carnelian-core::model_router`
**Timeline:** 1-2 weeks
**Effort:** Low-Medium

**Steps:**
1. Merge gateway routing logic into `model_router.rs`
2. Add provider configs to database
3. Update API endpoints
4. Remove `gateway/` directory
5. Update docker-compose.yml

### Phase 2: Worker Architecture Decision (Strategic)

**Decision Required:** Choose between Option A (Full Rust) or Option B (Hybrid)

**If Option B (Recommended):**
1. Keep `workers/node-worker/` for backward compatibility
2. Add WASM runtime support to `carnelian-core::worker`
3. Create Rust-native skill template
4. Migrate high-value skills incrementally

**Skills to Prioritize for Rust Migration:**
- `healthcheck` - Simple, high usage
- `echo` - Testing/development
- `model-usage` - Core function
- Any skill requiring heavy computation

### Phase 3: TypeScript Cleanup (Post-Decision)

**If fully migrating to Rust:**
1. Create WASM runtime in Rust
2. Port skills incrementally (10-20 per sprint)
3. Maintain Node.js worker until 80% migrated
4. Archive old skills

**If keeping hybrid:**
1. Freeze Node.js worker feature set
2. Only new skills in Rust/WASM
3. Maintain both systems

## 6. Architectural Diagrams

### Current Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Carnelian OS                            │
├─────────────────────────────────────────────────────────────────┤
│  UI (Dioxus)  │  Core (Axum/Tokio)  │  Workers (Mixed)           │
├───────────────┼─────────────────────┼────────────────────────────┤
│  XP Widget    │  Server             │  Node.js Worker (600+)     │
│  Voice Panel  │  Scheduler          │  Python Worker             │
│  Dashboard    │  Policy Engine      │  Shell Worker              │
│               │  Ledger             │                            │
│               │  Memory (RAG)       │                            │
│               │  Model Router       │                            │
│               │  Skills Discovery   │                            │
│               │  Events             │                            │
│               │  Agentic Pipeline   │                            │
├───────────────┴─────────────────────┴────────────────────────────┤
│  External: Node.js Gateway (needs migration)                    │
├─────────────────────────────────────────────────────────────────┤
│  Adapters: Telegram │ Discord                                     │
├─────────────────────────────────────────────────────────────────┤
│  PostgreSQL + pgvector                                          │
└─────────────────────────────────────────────────────────────────┘
```

### Target Architecture (Post-Migration)

```
┌─────────────────────────────────────────────────────────────────┐
│                         Carnelian OS                            │
├─────────────────────────────────────────────────────────────────┤
│  UI (Dioxus)  │  Core (Axum/Tokio)  │  Workers (Rust + External) │
├───────────────┼─────────────────────┼────────────────────────────┤
│  XP Widget    │  Server             │  Rust Native Worker        │
│  Voice Panel  │  Scheduler          │  ├─ WASM Skills (new)      │
│  Dashboard    │  Policy Engine      │  └─ Native Skills          │
│               │  Ledger             │  Python Worker (keep)      │
│               │  Memory (RAG)       │  Shell Worker (keep)       │
│               │  Model Router       │                            │
│               │  ├─ Ollama          │                            │
│               │  ├─ OpenAI          │                            │
│               │  ├─ Anthropic       │                            │
│               │  └─ Fireworks        │                            │
│               │  Skills Discovery     │                            │
│               │  Events               │                            │
│               │  Agentic Pipeline     │                            │
│               │  Gateway (merged)     │                            │
├───────────────┴─────────────────────┴────────────────────────────┤
│  Adapters: Telegram │ Discord                                     │
├─────────────────────────────────────────────────────────────────┤
│  PostgreSQL + pgvector                                          │
└─────────────────────────────────────────────────────────────────┘
```

## 7. Action Items

### Immediate (This Sprint)

- [ ] **Decision:** Choose worker migration strategy (Option A or B)
- [ ] Create migration tracking issue in GitHub
- [ ] Begin gateway integration into model_router

### Short-term (Next 2-4 Weeks)

- [ ] Complete gateway migration
- [ ] Update docker-compose to remove gateway service
- [ ] Add WASM runtime support to worker module
- [ ] Create Rust skill template/example

### Medium-term (1-3 Months)

- [ ] Migrate 5-10 high-value skills to Rust/WASM
- [ ] Document skill development guide for Rust
- [ ] Performance benchmark comparison

### Long-term (3-6 Months)

- [ ] Complete skill migration (if Option A chosen)
- [ ] Remove Node.js dependency (if Option A chosen)
- [ ] Single-binary deployment

## 8. Files to Modify/Remove

### Remove After Gateway Migration
```
gateway/
├── package.json
├── tsconfig.json
├── src/
│   ├── index.ts
│   ├── server.ts
│   ├── config.ts
│   ├── providers/
│   └── utils.ts
└── dist/
```

### Modify for Gateway Integration
```
crates/carnelian-core/src/
├── server.rs (add gateway endpoints)
├── model_router.rs (integrate providers)
└── config.rs (add gateway config)

docker-compose.yml (remove gateway service)
```

### Keep (Node.js Worker - If Option B)
```
workers/node-worker/
├── src/
│   ├── index.ts
│   ├── sandbox.ts
│   ├── loader.ts
│   ├── manifest.ts
│   ├── protocol.ts
│   ├── events.ts
│   └── types.ts
├── package.json
└── tsconfig.json
```

## Appendix: Existing Skills Inventory

### Current Skills in Registry

| Skill | Runtime | Status | Priority for Migration |
|-------|---------|--------|----------------------|
| `echo` | node | ✅ Active | High (simple template) |
| `healthcheck` | shell | ✅ Active | High (simple) |
| `local-places` | python | ✅ Active | Medium |
| `model-usage` | node | ✅ Active | High (core function) |
| `openai-image-gen` | node | ✅ Active | Medium |
| `skill-creator` | node | ✅ Active | Low |

**Note:** The "600+ skills" referenced in documentation are the original Thummim skills, most of which are not yet migrated to the new registry format.

---

**Report Generated:** 2026-02-24  
**Version:** Carnelian OS v0.1.0  
**Author:** Cascade AI Analysis
