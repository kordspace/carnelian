# Phase 3: Agentic Execution Engine

## Overview

Phase 3 introduces the core agentic execution pipeline — the orchestration layer that transforms Carnelian from a task scheduler into an autonomous agent capable of reasoning, tool use, memory persistence, and context-aware conversation.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    AgenticEngine                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────┐ │
│  │  Soul     │  │ Session  │  │ Memory   │  │Context │ │
│  │  Manager  │  │ Manager  │  │ Manager  │  │Window  │ │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └───┬────┘ │
│       │              │              │             │      │
│  ┌────┴──────────────┴──────────────┴─────────────┴──┐  │
│  │              PostgreSQL (pgvector)                  │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────┐  │
│  │  Model   │  │ Policy   │  │  Ledger  │  │ Event  │  │
│  │  Router  │  │ Engine   │  │ (blake3) │  │ Stream │  │
│  └──────────┘  └──────────┘  └──────────┘  └────────┘  │
└─────────────────────────────────────────────────────────┘
```

## Modules

### Soul Manager (`src/soul.rs`)

Loads, parses, and synchronizes soul files (`.md`) with the database.

- **Parsing**: Extracts directives from markdown headers and bullet points
- **Priority assignment**: Core Truths → P0, Boundaries → P1, Style/Personality → P2+
- **Hash verification**: blake3 hash detects file modifications
- **Sync**: Compares hash, updates `identities.directives` JSONB and `soul_file_hash`
- **Watch**: Initial sync for all identities with `soul_file_path` set

### Session Manager (`src/session.rs`)

Manages session lifecycle, message persistence, and token tracking.

- **Session keys**: Format `agent:<uuid>:<channel>[:group:<id>]`
- **Token counters**: JSONB tracking `{total, user, assistant, tool}` per session
- **Message operations**: Append (with atomic counter increment), load (paginated), delete
- **Compaction**: Triggered when `total > context_window_limit`, flushes memories, summarizes, prunes
- **Expiration**: Configurable TTL with `cleanup_expired_sessions()`

### Memory Manager (`src/memory.rs`)

CRUD and retrieval of agent memories with pgvector similarity search.

- **Sources**: `conversation`, `task`, `observation`, `reflection`
- **Importance scoring**: 0.0–1.0 range, validated on creation
- **Retrieval policies**:
  - `load_recent_memories`: 48-hour "Today + Yesterday" window
  - `load_high_importance_memories`: Threshold-based filtering
  - `query_memories`: Builder pattern with source/importance/date filters
  - `search_memories`: pgvector cosine similarity (1536-dim embeddings)
- **Access tracking**: `access_count` incremented on retrieval

### Context Window (`src/context.rs`)

Assembles model input from multiple data sources with priority-based budgeting.

- **Priority levels**:
  - P0: Soul directives (never pruned)
  - P1: Recent memories
  - P2: Task context
  - P3: Conversation history
  - P4: Tool results (pruned first)
- **Token estimation**: `tiktoken-rs` with `cl100k_base`, fallback to `len/4`
- **Budget enforcement**: Soft-trim tool results → hard-clear old tool results → drop P4 → drop P3
- **Provenance**: Tracks memory IDs, run IDs, message IDs, computes bundle hash
- **Session integration**: `build_for_session()` resolves context window limit from session/provider/defaults

### Model Router (`src/model_router.rs`)

Routes inference requests to local or remote providers with budget enforcement.

- **Provider matching**: Pattern-based detection (Claude → Anthropic, GPT → OpenAI, etc.)
- **Local-first**: Ollama/local providers preferred, remote as fallback
- **Cost estimation**: Per-provider token pricing
- **Budget limits**: Daily/monthly USD limits tracked in `usage_costs` table
- **Usage persistence**: `tokens_in`, `tokens_out`, `cost_estimate` per request

### Agentic Engine (`src/agentic.rs`)

Orchestrates the complete agentic loop.

- **Request processing**: Session intake → context assembly → model inference → response handling
- **Tool execution**: Parses tool calls from model output, dispatches to workers, collects results
- **Declarative plans**: Multi-step plans with dependency ordering and circular dependency detection
- **Memory persistence**: Extracts and stores memories from conversation
- **Correlation tracking**: UUID v7 correlation IDs flow through all operations
- **Policy integration**: Capability checks before tool execution

### Supporting Modules

- **Policy Engine** (`src/policy.rs`): Capability-based security with grant/revoke/check
- **Ledger** (`src/ledger.rs`): Tamper-resistant audit log with blake3 hash chain
- **Event Stream** (`src/events.rs`): Pub/sub for real-time event distribution

## Database Schema

Phase 3 relies on tables from migrations `00000000000001` through `00000000000007`:

| Table | Purpose |
|-------|---------|
| `identities` | Agent identities with `directives` JSONB and `soul_file_hash` |
| `sessions` | Session state with `token_counters` JSONB and `context_window_limit` |
| `session_messages` | Message history with `role`, `token_estimate`, `correlation_id` |
| `memories` | Memory storage with `embedding vector(1536)` and `importance` |
| `model_providers` | Provider configs with `budget_limits` JSONB |
| `usage_costs` | Token usage tracking with `cost_estimate` |
| `heartbeat_history` | Heartbeat records with `correlation_id` (migration 7) |
| `ledger_events` | Audit trail with blake3 hash chain |
| `capability_grants` | Security grants with expiration |

## Testing

### Integration Tests

```bash
# All Phase 3 tests (requires Docker for PostgreSQL + pgvector)
cargo test --test phase3_integration_test -- --ignored

# Specific test groups
cargo test --test phase3_integration_test test_soul -- --ignored
cargo test --test phase3_integration_test test_session -- --ignored
cargo test --test phase3_integration_test test_memory -- --ignored
cargo test --test phase3_integration_test test_context -- --ignored
cargo test --test phase3_integration_test test_end_to_end -- --ignored

# No Docker required
cargo test --test phase3_integration_test test_session_key_parsing
```

### Test Coverage

| Area | Tests | Docker |
|------|-------|--------|
| Soul file load/parse/sync/hash | 5 | Yes |
| Session CRUD/messages/expiry/counters | 7 | Yes |
| SessionKey parsing | 1 | No |
| Memory create/recent/importance/search | 6 | Yes |
| Context assembly/budget/provenance | 5 | Yes |
| Compaction triggers/counters | 2 | Yes |
| Model routing/budget/usage | 3 | Yes |
| Heartbeat persistence/correlation | 2 | Yes |
| Agentic loop/correlation/policy/ledger | 4 | Yes |
| Cross-module integration | 2 | Yes |

### Test Infrastructure

- **Container**: `pgvector/pgvector:pg16` via `testcontainers`
- **Fixtures**: `tests/fixtures/souls/test_lian.md`, `tests/fixtures/souls/test_minimal.md`
- **Helpers**: `tests/common/mod.rs` — DB setup, identity/session/memory/provider insertion
- **Pattern**: Each test spins up its own container for isolation

## Configuration

Phase 3 adds these `Config` fields (all with sensible defaults):

| Field | Default | Description |
|-------|---------|-------------|
| `context_window_tokens` | 32000 | Default context window size |
| `context_reserve_percent` | 10 | Budget reserve for response generation |
| `tool_trim_threshold` | 2000 | Soft-trim threshold for tool results (tokens) |
| `tool_clear_age_secs` | 3600 | Hard-clear age for old tool results |
| `session_default_expiry_hours` | 24 | Session TTL |

## Key Design Decisions

1. **Priority-based context assembly**: Ensures soul directives (P0) are never pruned while tool results (P4) are sacrificed first under budget pressure.

2. **Atomic token counter updates**: `increment_counters` uses SQL `jsonb_set` with row-level locking to prevent race conditions in concurrent message appends.

3. **48-hour memory window**: `load_recent_memories` uses a "Today + Yesterday" policy, balancing recency with context relevance.

4. **blake3 hash chain**: Both soul file hashes and ledger events use blake3 for fast, collision-resistant integrity verification.

5. **Correlation ID propagation**: UUID v7 (time-ordered) correlation IDs flow from request through session messages, usage records, heartbeats, and ledger events for end-to-end tracing.

6. **`assemble()` auto-resets**: Each call to `ContextWindow::assemble()` clears prior segments and re-loads from DB, ensuring fresh context on every invocation.
