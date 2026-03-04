# Memory System — Knowledge Persistence & Semantic Retrieval

**Carnelian v1.0.0**

The Memory System is Carnelian's PostgreSQL-backed knowledge persistence layer, providing long-term memory storage, semantic retrieval via pgvector embeddings, and cross-instance memory portability with cryptographic verification.

---

## Table of Contents

1. [Overview](#overview)
2. [Memory Lifecycle](#memory-lifecycle)
3. [PostgreSQL + pgvector Integration](#postgresql--pgvector-integration)
4. [Context Assembly Pipeline](#context-assembly-pipeline)
5. [Memory Tagging](#memory-tagging)
6. [Compaction & Archival](#compaction--archival)
7. [Cross-Instance Memory Portability](#cross-instance-memory-portability)
8. [Quantum Integrity](#quantum-integrity)
9. [API Reference](#api-reference)
10. [Database Schema](#database-schema)
11. [Best Practices](#best-practices)
12. [See Also](#see-also)

---

## Overview

The `MemoryManager` (in `crates/carnelian-core/src/memory.rs`) is Carnelian's PostgreSQL-backed memory persistence layer, enabling agents to store, retrieve, and reason over long-term knowledge beyond session boundaries.

### Key Features

✅ **Four memory sources** — `conversation`, `task`, `observation`, `reflection`  
✅ **pgvector 1536-dim embeddings** — Cosine similarity search with ivfflat indexing  
✅ **Importance scoring** — 0.0–1.0 range with "today + yesterday" load policy  
✅ **Tag-filtered selective disclosure** — JSONB tags with GIN indexing  
✅ **Cross-instance portability** — Signed CBOR envelopes with Ed25519 verification  
✅ **Ledger-backed chain-of-custody** — Tamper-resistant audit trail for memory operations  
✅ **Quantum integrity** — `quantum_checksum` column with MAGIC entropy verification  

---

## Memory Lifecycle

```
┌─────────────┐
│  Ingestion  │
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────────┐
│  Storage (PostgreSQL + embedding)   │
└──────┬──────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────┐
│  Retrieval (cosine / recency)       │
└──────┬──────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────┐
│  Compaction (flush + summarize)     │
└──────┬──────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────┐
│  Export (CBOR envelope)             │
└─────────────────────────────────────┘
```

### Lifecycle Phases

**Ingestion** — `create_memory()` writes to the `memories` table; content is encrypted via `EncryptionHelper` if configured; a `MemoryCreated` event is emitted to the event stream for real-time observability.

**Storage** — Each memory has an `embedding vector(1536)` column indexed with `ivfflat` (100 lists, cosine ops); `tags JSONB` is GIN-indexed for fast filtering; `quantum_checksum` column is populated by migration 17 for tamper-resistant integrity verification.

**Retrieval** — `load_recent_memories()` fetches memories from the past 48 hours; `load_high_importance_memories()` retrieves memories with importance > 0.8; `search_memories()` performs pgvector cosine similarity search using the `<=>` operator.

**Compaction** — During session compaction, an agentic summarization turn extracts durable `reflection` memories before the session transcript is pruned, preventing loss of critical context (see Session Management documentation for full protocol).

**Export** — `export_memory()` and `export_memories_batch()` produce signed CBOR `MemoryEnvelope` bytes with Ed25519 signatures, enabling secure cross-instance memory transfer with cryptographic verification.

---

## PostgreSQL + pgvector Integration

### Embedding Dimensions

All embeddings are **1536-dimensional** `vector(1536)` — matching OpenAI `text-embedding-3-small` and compatible Ollama models. Validation is enforced in `validate_embedding_dimension()` in `crates/carnelian-core/src/memory.rs`.

### Index Definition

The `ivfflat` index enables fast approximate nearest-neighbor search:

```sql
CREATE INDEX idx_memories_embedding
  ON memories USING ivfflat (embedding vector_cosine_ops)
  WITH (lists = 100);
```

**Index tuning:**
- `lists = 100` is a rule-of-thumb (≈√rows) for up to 10,000 rows
- For larger datasets, increase `lists` proportionally
- Tune `nprobe` at query time for recall/speed tradeoff:

```sql
SET ivfflat.probes = 10;  -- Higher N = better recall, slower queries
```

### Similarity Search Query

The `<=>` operator computes cosine distance (0 = identical, 2 = opposite):

```sql
SELECT *, 1 - (embedding <=> $1) AS similarity
FROM memories
WHERE identity_id = $2
  AND 1 - (embedding <=> $1) >= $3
ORDER BY embedding <=> $1
LIMIT $4;
```

**Parameters:**
- `$1` — Query embedding (1536-dim vector)
- `$2` — Identity ID filter
- `$3` — Minimum similarity threshold (e.g., 0.7)
- `$4` — Result limit

### Access Tracking

`GET /v1/memories/{id}` automatically increments `access_count` and updates `accessed_at` timestamp, enabling usage-based retention policies.

---

## Context Assembly Pipeline

Carnelian uses a **priority-based context assembly pipeline** to construct model input from multiple data sources. Segments are loaded in priority order (P0–P4), token-budgeted, and pruned when the assembled context exceeds the model's context window.

### Priority-Based Segment Loading

| Priority | Source | Always Included? | Prunable? | Trim Strategy |
|----------|--------|------------------|-----------|---------------|
| P0 | Soul Directives | Yes | No | Never pruned |
| P1 | Recent Memories (48 hr + importance > 0.8) | Preferred | Yes | Drop by ascending importance |
| P2 | Task Context | Yes | No | Never pruned |
| P3 | Conversation History | Preferred | Yes | Drop oldest first |
| P4 | Tool Results | Optional | Yes | Soft-trim head+tail, then hard-clear |

### Token Budgeting

**Budget calculation:**
```
budget = context_window_tokens × (1 − context_reserve_percent / 100)
```

**Default:** 90% utilization leaves 10% headroom for model response.

**Source:** `ContextWindow::with_config()` in `crates/carnelian-core/src/context.rs`

### Soft-Trim Strategy

P4 segments (tool results) are first trimmed using a head+tail approach:
- **60%** of token budget allocated to head (beginning)
- **40%** of token budget allocated to tail (end)
- `…` ellipsis separator indicates omitted middle section

If still over budget after soft-trimming, segments are hard-cleared entirely.

### Pruning Cascade

When total token count exceeds the budget, segments are pruned in reverse priority order:

1. **Hard-clear old tool results** (P4, age > threshold)
2. **Soft-trim oversized tool results** (P4, head+tail strategy)
3. **Drop oldest conversation messages** (P3)
4. **Drop lowest-importance memories** (P1)
5. **P0 (soul directives) and P2 (task context) are never pruned**

### Provenance Tracking

Every assembled context bundle records:

```rust
pub struct ContextProvenance {
    pub memory_ids: Vec<Uuid>,           // Which memories contributed
    pub run_ids: Vec<Uuid>,              // Which task runs contributed
    pub message_ids: Vec<i64>,           // Which conversation messages
    pub context_bundle_hash: String,     // BLAKE3 hash for tamper detection
    pub total_tokens: usize,             // Estimated token count
    pub segment_counts: HashMap<String, usize>,  // Breakdown by source type
}
```

Provenance is logged via `log_to_ledger()` before every model call, creating an auditable chain:

1. **Context assembly** — `log_context_integrity()` records BLAKE3 hash and provenance
2. **Model call** — Model router logs request/response with same `correlation_id`
3. **Audit trail** — Query `ledger_events` by `correlation_id` to reconstruct exact context

---

## Memory Tagging

### Schema

Migration 12 adds JSONB tag support:

```sql
ALTER TABLE memories ADD COLUMN tags JSONB DEFAULT '[]'::jsonb;
CREATE INDEX idx_memories_tags ON memories USING GIN (tags);
```

### Tag Use Cases

- **Categorization** — Organize memories by topic, project, or domain
- **Selective disclosure** — Filter exports by tag via `MemoryExportOptions.topic_filter`
- **Mantra-linked retrieval** — Future support for mantra-referenced memory injection

### Filtering Strategies

**GIN containment** — Fast exact-match queries:
```sql
SELECT * FROM memories WHERE tags @> '["rust"]'::jsonb;
```

**Semantic + tag hybrid** — Combine pgvector cosine search with tag filtering:
```sql
SELECT *, 1 - (embedding <=> $1) AS similarity
FROM memories
WHERE identity_id = $2
  AND tags @> $3
  AND 1 - (embedding <=> $1) >= $4
ORDER BY embedding <=> $1
LIMIT $5;
```

**Export topic filter** — Pass `topic_filter: Some(vec!["security".to_string()])` to `MemoryExportOptions`; `export_memory()` checks tag match before serializing.

---

## Compaction & Archival

### Memory Flush Protocol

Before a session transcript is compacted, an **agentic summarization turn** is run to extract durable insights and write them as `source = "reflection"` memories with elevated importance. This pattern prevents loss of critical context during pruning.

**Process:**
1. Model analyzes recent conversation history
2. Identifies key facts, preferences, and decisions
3. Creates `reflection` memories with importance > 0.7
4. Session transcript is then pruned (older messages deleted)

### Retention Policies

Suggested retention tiers based on importance and usage:

| Condition | Recommended Action |
|-----------|-------------------|
| `importance < 0.3` + `access_count = 0` + age > 90 days | Archive / delete |
| `importance < 0.5` + age > 180 days | Flag for review |
| `importance > 0.8` | Retain indefinitely |

**Implementation:**
```sql
-- Find candidates for archival
SELECT memory_id, content, importance, access_count, created_at
FROM memories
WHERE importance < 0.3
  AND access_count = 0
  AND created_at < NOW() - INTERVAL '90 days';
```

### Compaction Trigger

Auto-compaction fires when:
```
contextTokens > contextWindow − reserveTokens
```

**Sequence:**
1. Memory flush runs first (extract durable memories)
2. Transcript pruning (delete old messages)
3. Token recalculation
4. Ledger event logged

**Cross-reference:** Full compaction protocol documented in `SESSION_MANAGEMENT.md`

---

## Cross-Instance Memory Portability

Carnelian supports secure memory transfer between instances using cryptographically-signed CBOR envelopes.

### Envelope Structure

```rust
pub struct MemoryEnvelope {
    pub version: u8,                                    // Format version (currently 1)
    pub encrypted_content: Vec<u8>,                     // AES-256-GCM encrypted memory JSON
    pub content_hash: String,                           // BLAKE3 hash of encrypted_content
    pub ledger_proof: Option<LedgerProofMaterial>,      // Chain-of-custody proof
    pub capability_grants: Vec<CapabilityGrantMetadata>, // Portable capability grants
    pub embedding: Option<Vec<f32>>,                    // 1536-dim embedding (unencrypted)
    pub metadata: HashMap<String, String>,              // exported_at, source_memory_id, etc.
    pub chain_anchor: Option<String>,                   // External blockchain anchor ID
}
```

### Export Flow

**Steps** (from `export_memory()`, lines 1446–1581):

1. **Fetch** — Retrieve memory with importance/topic filters
2. **Gather proof** — Retrieve optional ledger proof + capability grants
3. **Serialize** — Convert memory fields to JSON
4. **Encrypt** — AES-256-GCM encryption with key derivation
5. **Hash** — BLAKE3-hash encrypted bytes → `content_hash`
6. **Build envelope** — Construct `MemoryEnvelope` struct
7. **Anchor** — Optionally anchor hash via `ChainAnchor` trait
8. **Serialize** — Convert to CBOR using `ciborium`
9. **Sign** — Prefix 64-byte Ed25519 signature if `signing_key` provided

**Example:**
```rust
let envelope = memory_manager.export_memory(
    memory_id,
    MemoryExportOptions {
        include_embedding: true,
        include_ledger_proof: true,
        include_capabilities: true,
        topic_filter: Some(vec!["security".to_string()]),
        min_importance: Some(0.7),
        signing_key: Some(owner_keypair),
    }
).await?;
```

### Import Verification Flow

**Steps** (from `import_memory()`, lines 1652–1830):

1. **Strip signature** — Extract and verify Ed25519 signature (if `verify_signature = true`)
2. **Deserialize** — CBOR-deserialize to `MemoryEnvelope`
3. **Check grants** — Verify topic-scoped capability grants via `PolicyEngine`
4. **Verify hash** — Recompute BLAKE3 hash — reject on mismatch
5. **Decrypt** — AES-256-GCM decryption
6. **Verify proof** — Check `LedgerProofMaterial` against local ledger chain
7. **Insert** — Create new `Memory` record in database
8. **Recreate grants** — Restore capability grants
9. **Attach embedding** — Insert embedding if present
10. **Log** — Write `memory.imported` ledger event

**Example:**
```rust
let result = memory_manager.import_memory(
    envelope_bytes,
    MemoryImportOptions {
        verify_signature: true,
        verify_ledger_proof: true,
        verify_capabilities: true,
        target_identity_id: local_identity_id,
    }
).await?;
```

### Selective Disclosure

`MemoryExportOptions` controls what information is disclosed per envelope:

| Field | Type | Description |
|-------|------|-------------|
| `topic_filter` | `Option<Vec<String>>` | Only export memories with matching tags |
| `min_importance` | `Option<f64>` | Only export memories above threshold |
| `include_embedding` | `bool` | Include 1536-dim embedding vector |
| `include_ledger_proof` | `bool` | Include chain-of-custody proof |
| `include_capabilities` | `bool` | Include portable capability grants |

### Batch Export/Import

**Batch export** — `export_memories_batch()` signs the entire `Vec<MemoryEnvelope>` CBOR as a unit:

```rust
let envelopes = memory_manager.export_memories_batch(
    memory_ids,
    export_options,
    Some(owner_keypair),
).await?;
```

**Batch import** — `import_memories_batch()` verifies the batch signature once, then imports each envelope individually:

```rust
let results = memory_manager.import_memories_batch(
    batch_bytes,
    import_options,
).await?;

for result in results {
    match result {
        Ok(memory_id) => println!("Imported: {}", memory_id),
        Err(e) => eprintln!("Failed: {}", e),
    }
}
```

---

## Quantum Integrity

Carnelian uses quantum-enhanced checksums for tamper-resistant memory integrity verification.

### Column Definition

Migration 17 adds quantum checksum support:

```sql
ALTER TABLE memories ADD COLUMN quantum_checksum TEXT;
CREATE INDEX idx_memories_quantum_checksum
  ON memories (quantum_checksum)
  WHERE quantum_checksum IS NOT NULL;
```

### Checksum Computation

Use `QuantumHasher` from `carnelian-magic`:

```rust
use carnelian_magic::{MixedEntropyProvider, QuantumHasher};

// Sample 16 bytes from quantum entropy chain
let quantum_salt = entropy_provider.sample_bytes(16).await?;

// Create hasher with quantum salt
let hasher = QuantumHasher::new(quantum_salt);

// Hash memory content
let checksum = hasher.hash_memory_content(&memory.content);

// Store in database
sqlx::query(
    "UPDATE memories SET quantum_checksum = $1 WHERE memory_id = $2"
)
.bind(checksum)
.bind(memory_id)
.execute(pool)
.await?;
```

**Entropy provider waterfall:**
1. **Quantum Origin** (if API key configured)
2. **Quantinuum H2** (if authenticated)
3. **IBM Qiskit** (if token configured)
4. **OS CSPRNG** (always available fallback)

See [MAGIC.md](MAGIC.md) for entropy provider setup.

### Verification

Verify memory integrity using the MAGIC integrity endpoint:

```bash
curl -X POST http://localhost:18789/v1/magic/integrity/verify \
  -H "X-Carnelian-Key: $KEY" \
  -d '{"tables": ["memories"]}'
```

**Response:**
```json
{
  "results": [
    {
      "table": "memories",
      "total_rows": 1542,
      "verified": 1542,
      "tampered": 0,
      "missing_checksums": 0,
      "tampered_rows": []
    }
  ]
}
```

### Rehashing

Periodically rehash memories with fresh quantum entropy:

```bash
curl -X POST http://localhost:18789/v1/magic/memories/rehash \
  -H "X-Carnelian-Key: $KEY"
```

**Response:**
```json
{
  "message": "Rehashed all memories with fresh entropy",
  "rehashed": 1542
}
```

**Recommendation:** Run quarterly or after major security events.

---

## API Reference

See [docs/API.md](API.md#memory-management) for complete endpoint documentation.

### Quick Reference

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/memories` | POST | Create a new memory |
| `/v1/memories` | GET | List memories (filter by `identity_id`, `source`, `min_importance`) |
| `/v1/memories/{id}` | GET | Get memory by ID (auto-updates `accessed_at`) |
| `/v1/memories/search` | GET | pgvector similarity search |
| `/v1/memories/export` | POST | Export memories as signed CBOR envelope |
| `/v1/memories/import` | POST | Import memory from CBOR envelope |

### WebSocket Events

| Event | Payload | Description |
|-------|---------|-------------|
| `MemoryCreated` | `memory_id`, `identity_id`, `source`, `importance` | Memory ingested |
| `MemoryUpdated` | `memory_id`, `identity_id` | Memory updated |
| `MemoryDeleted` | `memory_id` | Memory deleted |
| `MemorySearchPerformed` | `identity_id`, `result_count`, `min_similarity` | Similarity search fired |

---

## Database Schema

### `memories` Table

Complete schema from migrations 1, 12, and 17:

```sql
CREATE TABLE memories (
    memory_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identity_id      UUID NOT NULL REFERENCES identities(identity_id) ON DELETE CASCADE,
    content          TEXT NOT NULL,
    summary          TEXT,
    source           TEXT NOT NULL CHECK (source IN (
                         'conversation', 'task', 'observation', 'reflection'
                     )),
    embedding        vector(1536),
    importance       REAL DEFAULT 0.5 CHECK (importance >= 0.0 AND importance <= 1.0),
    tags             JSONB DEFAULT '[]'::jsonb,
    quantum_checksum TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    accessed_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    access_count     INTEGER NOT NULL DEFAULT 0
);

-- Indexes
CREATE INDEX idx_memories_identity   ON memories(identity_id);
CREATE INDEX idx_memories_source     ON memories(source);
CREATE INDEX idx_memories_importance ON memories(importance DESC);
CREATE INDEX idx_memories_tags       ON memories USING GIN (tags);
CREATE INDEX idx_memories_embedding  ON memories
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
CREATE INDEX idx_memories_quantum_checksum
    ON memories (quantum_checksum)
    WHERE quantum_checksum IS NOT NULL;
```

### Column Descriptions

| Column | Type | Description |
|--------|------|-------------|
| `memory_id` | UUID | Primary key |
| `identity_id` | UUID | Owner identity (foreign key) |
| `content` | TEXT | Full memory content |
| `summary` | TEXT | Optional short summary |
| `source` | TEXT | Memory source: `conversation`, `task`, `observation`, `reflection` |
| `embedding` | vector(1536) | pgvector embedding for similarity search |
| `importance` | REAL | Importance score (0.0–1.0) |
| `tags` | JSONB | Tag array for categorization |
| `quantum_checksum` | TEXT | BLAKE3 + quantum entropy checksum |
| `created_at` | TIMESTAMPTZ | Creation timestamp |
| `accessed_at` | TIMESTAMPTZ | Last access timestamp (auto-updated) |
| `access_count` | INTEGER | Access counter (auto-incremented) |

---

## Best Practices

### 1. Importance Scoring

✅ **DO:**
- Assign `importance > 0.8` only for critical facts, decisions, and preferences
- Use `importance 0.5–0.7` for useful context and observations
- Keep ephemeral chat at `importance < 0.3`
- Adjust importance over time based on `access_count`

❌ **DON'T:**
- Assign high importance to every memory (defeats pruning)
- Ignore importance when querying (wastes tokens on irrelevant context)
- Set importance below 0.0 or above 1.0 (violates CHECK constraint)

### 2. Embedding Strategy

✅ **DO:**
- Always generate embeddings for new memories
- Use semantic search (`search_memories()`) for conceptual queries
- Combine semantic + tag filtering for precise retrieval
- Regenerate embeddings if embedding model changes

❌ **DON'T:**
- Rely solely on lexical search (misses semantic matches)
- Skip embeddings for "short" memories (still valuable for search)
- Use different embedding dimensions (breaks ivfflat index)

### 3. Export Security

✅ **DO:**
- Always provide a `signing_key` for cross-instance transfer
- Always set `include_ledger_proof: true` for auditable imports
- Verify signatures on import (`verify_signature: true`)
- Use selective disclosure (`topic_filter`, `min_importance`)

❌ **DON'T:**
- Export without signatures (no authenticity guarantee)
- Skip ledger proof verification (loses chain-of-custody)
- Export all memories indiscriminately (privacy risk)
- Share envelopes over unencrypted channels

### 4. Retention

✅ **DO:**
- Run weekly archival jobs for low-importance unused memories
- Archive to cold storage before deletion (compliance)
- Monitor `access_count` to identify valuable memories
- Preserve high-importance memories indefinitely

❌ **DON'T:**
- Delete memories without archival (irreversible)
- Ignore retention policies (database bloat)
- Archive recently-accessed memories (still in use)
- Hard-delete memories with `ledger_proof` (breaks audit trail)

---

## See Also

- **[ELIXIR_SYSTEM.md](ELIXIR_SYSTEM.md)** — RAG knowledge persistence layer
- **[MAGIC.md](MAGIC.md)** — Quantum entropy for checksums
- **[LEDGER_SYSTEM.md](LEDGER_SYSTEM.md)** — Hash-chain audit trail
- **[API.md](API.md#memory-management)** — Complete API reference
- **[SESSION_MANAGEMENT.md](SESSION_MANAGEMENT.md)** — Session compaction & memory flush protocol

---

**Last Updated:** March 2026  
**Version:** 1.0.0
