# Ledger System — Audit Trail & Hash-Chain Integrity

**Carnelian Core v1.0.0**

The Ledger System provides an immutable, cryptographically-verified audit trail for all privileged operations in Carnelian Core. Built on BLAKE3 hash-chaining with optional quantum entropy mixing, it ensures tamper-resistant accountability and compliance.

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Hash-Chain Design](#hash-chain-design)
4. [Event Types](#event-types)
5. [Quantum Entropy Integration](#quantum-entropy-integration)
6. [Chain Anchoring](#chain-anchoring)
7. [Verification](#verification)
8. [API Reference](#api-reference)
9. [Database Schema](#database-schema)
10. [Usage Examples](#usage-examples)
11. [Best Practices](#best-practices)

---

## Overview

The Ledger System is Carnelian's **tamper-evident audit log**, recording all security-critical operations with cryptographic integrity guarantees.

### Key Features

✅ **BLAKE3 hash-chaining** — Each entry links to previous entry via cryptographic hash  
✅ **Quantum entropy salting** — Optional MAGIC-sourced entropy for quantum-resistant integrity  
✅ **Event sourcing** — Complete audit trail of all privileged operations  
✅ **Chain anchoring** — Periodic merkle root computation for slice verification  
✅ **Immutable records** — Write-only ledger with no update/delete operations  
✅ **XP integration** — Ledger-backed XP awards with full auditability  
✅ **Desktop UI viewer** — Real-time ledger visualization with hash verification  

### Why BLAKE3?

Carnelian uses **BLAKE3** instead of SHA-256 for ledger hashing because:

- **10x faster** than SHA-256 on modern CPUs
- **Parallelizable** — Utilizes SIMD and multi-threading
- **Collision-resistant** — 256-bit output with proven security
- **Quantum-ready** — Can be enhanced with quantum entropy mixing
- **Single-pass** — No need for multiple rounds like SHA-2

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Privileged Operation                     │
│  (Capability Grant, Config Change, XP Award, etc.)          │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    Ledger Manager                            │
│  • Fetch previous hash                                       │
│  • Sample quantum salt (optional)                            │
│  • Compute BLAKE3 hash                                       │
│  • Insert ledger entry                                       │
│  • Publish LedgerWritten event                               │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                  ledger_events Table                         │
│  • event_id (UUID)                                           │
│  • event_type (TEXT)                                         │
│  • payload (JSONB)                                           │
│  • previous_hash (TEXT)                                      │
│  • current_hash (TEXT)                                       │
│  • quantum_salt (BYTEA, optional)                            │
│  • created_at (TIMESTAMPTZ)                                  │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                  Chain Anchor System                         │
│  • Periodic merkle root computation                          │
│  • Slice verification                                        │
│  • Tamper detection                                          │
└─────────────────────────────────────────────────────────────┘
```

---

## Hash-Chain Design

### Chain Structure

Each ledger entry contains:

1. **Event Data** — Type, payload, timestamp, correlation ID
2. **Previous Hash** — BLAKE3 hash of the previous entry
3. **Current Hash** — BLAKE3 hash of (event data + previous hash + quantum salt)
4. **Quantum Salt** — Optional 16-byte entropy from MAGIC providers

### Hash Computation

```rust
use blake3::Hasher;

pub fn compute_ledger_hash(
    event_type: &str,
    payload: &JsonValue,
    previous_hash: &str,
    quantum_salt: Option<&[u8]>,
) -> String {
    let mut hasher = Hasher::new();
    
    // Hash event data
    hasher.update(event_type.as_bytes());
    hasher.update(payload.to_string().as_bytes());
    
    // Chain to previous hash
    hasher.update(previous_hash.as_bytes());
    
    // Mix quantum entropy if available
    if let Some(salt) = quantum_salt {
        hasher.update(salt);
    }
    
    hex::encode(hasher.finalize().as_bytes())
}
```

### Genesis Entry

The first ledger entry (genesis) has:
- `previous_hash = "0000000000000000000000000000000000000000000000000000000000000000"`
- `event_type = "LedgerInitialized"`
- `payload = {"message": "Ledger chain initialized", "version": "1.0.0"}`

### Chain Verification

```rust
pub async fn verify_chain(pool: &PgPool) -> Result<ChainVerification> {
    let entries = sqlx::query_as::<_, LedgerEntry>(
        "SELECT * FROM ledger_events ORDER BY created_at ASC"
    )
    .fetch_all(pool)
    .await?;
    
    let mut previous_hash = "0000...".to_string();
    let mut tampered_entries = Vec::new();
    
    for entry in entries {
        let computed_hash = compute_ledger_hash(
            &entry.event_type,
            &entry.payload,
            &previous_hash,
            entry.quantum_salt.as_deref(),
        );
        
        if computed_hash != entry.current_hash {
            tampered_entries.push(entry.event_id);
        }
        
        previous_hash = entry.current_hash;
    }
    
    Ok(ChainVerification {
        total_entries: entries.len(),
        verified: entries.len() - tampered_entries.len(),
        tampered: tampered_entries,
    })
}
```

---

## Event Types

### Security Events

| Event Type | Description | Payload Fields |
|------------|-------------|----------------|
| `CapabilityGranted` | Capability grant approved | `subject_id`, `capability_key`, `granted_by` |
| `CapabilityRevoked` | Capability revoked | `subject_id`, `capability_key`, `revoked_by` |
| `SafeModeActivated` | Emergency lockdown enabled | `reason`, `activated_by` |
| `SafeModeDeactivated` | Emergency lockdown disabled | `deactivated_by` |
| `KeyGenerated` | Ed25519 keypair generated | `identity_id`, `public_key` |
| `EncryptionKeyRotated` | AES-256 key rotated | `key_id`, `rotated_by` |

### XP Events

| Event Type | Description | Payload Fields |
|------------|-------------|----------------|
| `XpAwarded` | XP points awarded | `identity_id`, `amount`, `source`, `skill_id` |
| `LevelUp` | Identity leveled up | `identity_id`, `old_level`, `new_level` |
| `XpCurveRetuned` | XP curve parameters changed | `old_params`, `new_params`, `changed_by` |

### Configuration Events

| Event Type | Description | Payload Fields |
|------------|-------------|----------------|
| `ConfigChanged` | System configuration updated | `key`, `old_value`, `new_value`, `changed_by` |
| `MigrationApplied` | Database migration executed | `migration_name`, `version`, `applied_by` |

### Heartbeat Events

| Event Type | Description | Payload Fields |
|------------|-------------|----------------|
| `HeartbeatTick` | Agentic heartbeat executed | `tick_number`, `tasks_queued`, `quantum_salt` |
| `HeartbeatSkipped` | Heartbeat skipped (safe mode) | `reason` |

### MAGIC Events

| Event Type | Description | Payload Fields |
|------------|-------------|----------------|
| `EntropyProviderUsed` | Quantum entropy sampled | `provider`, `bytes_sampled`, `success` |
| `MantraSelected` | Mantra chosen for context | `mantra_id`, `category`, `entropy_source` |
| `QuantumIntegrityVerified` | Quantum checksum verified | `table`, `rows_verified`, `tampered_count` |

---

## Quantum Entropy Integration

### Why Quantum Entropy in Ledger?

Quantum entropy mixing provides:

1. **Quantum-resistant integrity** — Harder to forge hashes with quantum computers
2. **Unpredictable salting** — True randomness from quantum sources
3. **Audit trail enhancement** — Proves quantum entropy was available at time of event
4. **Future-proofing** — Prepares for post-quantum cryptography migration

### Entropy Sampling

```rust
use carnelian_magic::MixedEntropyProvider;

let entropy_provider = MixedEntropyProvider::new(pool.clone());
let quantum_salt = entropy_provider.sample_bytes(16).await?;

let current_hash = compute_ledger_hash(
    &event_type,
    &payload,
    &previous_hash,
    Some(&quantum_salt),
);

sqlx::query(
    "INSERT INTO ledger_events (event_type, payload, previous_hash, current_hash, quantum_salt)
     VALUES ($1, $2, $3, $4, $5)"
)
.bind(event_type)
.bind(payload)
.bind(previous_hash)
.bind(current_hash)
.bind(quantum_salt)
.execute(pool)
.await?;
```

### Entropy Provider Waterfall

Quantum salt is sourced from the MAGIC entropy chain:

1. **Quantum Origin** (if API key configured)
2. **Quantinuum H2** (if authenticated)
3. **IBM Qiskit** (if token configured)
4. **OS CSPRNG** (always available fallback)

See [MAGIC.md](MAGIC.md) for entropy provider details.

---

## Chain Anchoring

**Chain anchoring** creates periodic merkle roots for efficient slice verification without scanning the entire chain.

### Anchor Creation

```rust
pub async fn create_chain_anchor(
    pool: &PgPool,
    slice_size: usize,
) -> Result<ChainAnchor> {
    // Fetch last N entries
    let entries = sqlx::query_as::<_, LedgerEntry>(
        "SELECT * FROM ledger_events 
         ORDER BY created_at DESC 
         LIMIT $1"
    )
    .bind(slice_size as i64)
    .fetch_all(pool)
    .await?;
    
    // Compute merkle root
    let merkle_root = compute_merkle_root(&entries);
    
    // Store anchor
    let anchor_id = sqlx::query_scalar(
        "INSERT INTO ledger_anchors (merkle_root, slice_start, slice_end, entry_count)
         VALUES ($1, $2, $3, $4)
         RETURNING anchor_id"
    )
    .bind(&merkle_root)
    .bind(entries.last().unwrap().event_id)
    .bind(entries.first().unwrap().event_id)
    .bind(entries.len() as i32)
    .fetch_one(pool)
    .await?;
    
    Ok(ChainAnchor {
        anchor_id,
        merkle_root,
        entry_count: entries.len(),
    })
}
```

### Merkle Root Computation

```rust
use blake3::Hasher;

fn compute_merkle_root(entries: &[LedgerEntry]) -> String {
    if entries.is_empty() {
        return "0000...".to_string();
    }
    
    let mut hashes: Vec<String> = entries
        .iter()
        .map(|e| e.current_hash.clone())
        .collect();
    
    while hashes.len() > 1 {
        let mut next_level = Vec::new();
        
        for chunk in hashes.chunks(2) {
            let mut hasher = Hasher::new();
            hasher.update(chunk[0].as_bytes());
            if chunk.len() > 1 {
                hasher.update(chunk[1].as_bytes());
            }
            next_level.push(hex::encode(hasher.finalize().as_bytes()));
        }
        
        hashes = next_level;
    }
    
    hashes[0].clone()
}
```

---

## Verification

### Full Chain Verification

```bash
curl -X POST http://localhost:18789/v1/ledger/verify \
  -H "X-Carnelian-Key: $KEY"

# Response
{
  "total_entries": 1542,
  "verified": 1542,
  "tampered": [],
  "chain_valid": true,
  "verification_time_ms": 234
}
```

### Slice Verification

```bash
curl -X POST http://localhost:18789/v1/ledger/verify-slice \
  -H "X-Carnelian-Key: $KEY" \
  -d '{"start_id": "01936a1a-...", "end_id": "01936a1b-..."}'

# Response
{
  "slice_entries": 100,
  "verified": 100,
  "tampered": [],
  "merkle_root": "a1b2c3d4...",
  "matches_anchor": true
}
```

### Desktop UI Verification

The Dioxus desktop UI includes a **Ledger Viewer** page (`ledger.rs`) with:

- Real-time ledger event stream
- Hash-chain visualization
- One-click verification
- Tamper detection alerts
- Export to JSON/CSV

---

## API Reference

See [docs/API.md](API.md#ledger) for complete endpoint documentation.

### Quick Reference

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/ledger/events` | GET | List ledger events with pagination |
| `/v1/ledger/verify` | POST | Verify entire hash-chain |
| `/v1/ledger/verify-slice` | POST | Verify specific slice |
| `/v1/ledger/anchors` | GET | List chain anchors |
| `/v1/ledger/export` | GET | Export ledger to JSON/CSV |

---

## Database Schema

### `ledger_events` Table

```sql
CREATE TABLE ledger_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    previous_hash TEXT NOT NULL,
    current_hash TEXT NOT NULL,
    quantum_salt BYTEA,
    correlation_id UUID,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_ledger_events_type ON ledger_events(event_type);
CREATE INDEX idx_ledger_events_created_at ON ledger_events(created_at DESC);
CREATE INDEX idx_ledger_events_correlation ON ledger_events(correlation_id);
```

### `ledger_anchors` Table

```sql
CREATE TABLE ledger_anchors (
    anchor_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merkle_root TEXT NOT NULL,
    slice_start UUID NOT NULL REFERENCES ledger_events(event_id),
    slice_end UUID NOT NULL REFERENCES ledger_events(event_id),
    entry_count INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_ledger_anchors_created_at ON ledger_anchors(created_at DESC);
```

### `xp_ledger` Table

```sql
CREATE TABLE xp_ledger (
    xp_event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identity_id UUID NOT NULL REFERENCES identities(identity_id),
    amount INTEGER NOT NULL,
    source TEXT NOT NULL, -- 'task_completion', 'skill_usage', 'elixir_quality', etc.
    skill_id UUID REFERENCES skills(skill_id),
    task_id UUID REFERENCES tasks(task_id),
    ledger_event_id UUID REFERENCES ledger_events(event_id),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_xp_ledger_identity ON xp_ledger(identity_id);
CREATE INDEX idx_xp_ledger_created_at ON xp_ledger(created_at DESC);
```

---

## Usage Examples

### Write Ledger Entry

```rust
use carnelian_core::ledger::LedgerManager;

let ledger_manager = LedgerManager::new(pool.clone());

ledger_manager.write_event(
    "CapabilityGranted",
    json!({
        "subject_id": identity_id,
        "capability_key": "fs.write",
        "granted_by": owner_id,
        "reason": "User requested file write access"
    }),
    Some(correlation_id),
).await?;
```

### Query Ledger Events

```bash
# Get recent events
curl -X GET "http://localhost:18789/v1/ledger/events?limit=50&event_type=XpAwarded" \
  -H "X-Carnelian-Key: $KEY"

# Get events by correlation ID
curl -X GET "http://localhost:18789/v1/ledger/events?correlation_id=01936a1a-..." \
  -H "X-Carnelian-Key: $KEY"
```

### Export Ledger

```bash
# Export to JSON
curl -X GET "http://localhost:18789/v1/ledger/export?format=json&start_date=2026-01-01" \
  -H "X-Carnelian-Key: $KEY" \
  -o ledger_export.json

# Export to CSV
curl -X GET "http://localhost:18789/v1/ledger/export?format=csv" \
  -H "X-Carnelian-Key: $KEY" \
  -o ledger_export.csv
```

---

## Best Practices

### 1. Event Logging

✅ **DO:**
- Log all privileged operations
- Include correlation IDs for traceability
- Use structured payloads (JSONB)
- Enable quantum salting for sensitive events

❌ **DON'T:**
- Log sensitive data (passwords, keys) in payloads
- Skip ledger writes for security-critical operations
- Modify or delete ledger entries (immutable by design)

### 2. Verification

✅ **DO:**
- Verify chain integrity on startup
- Run periodic verification (daily/weekly)
- Alert on tamper detection
- Create anchors every 1000 entries

❌ **DON'T:**
- Skip verification in production
- Ignore tamper warnings
- Disable quantum salting without justification

### 3. Performance

✅ **DO:**
- Use slice verification for large chains
- Index by `event_type` and `created_at`
- Batch anchor creation
- Archive old entries (>1 year) to separate table

❌ **DON'T:**
- Verify entire chain on every request
- Query without indexes
- Store binary data in payloads

### 4. Compliance

✅ **DO:**
- Export ledger for compliance audits
- Retain ledger for required retention period
- Document event types and payload schemas
- Implement access controls for ledger queries

❌ **DON'T:**
- Delete ledger entries to hide mistakes
- Allow unauthenticated ledger access
- Omit critical events from logging

---

## See Also

- **[MAGIC.md](MAGIC.md)** — Quantum entropy system
- **[SECURITY.md](SECURITY.md)** — Security architecture
- **[API.md](API.md#ledger)** — Complete API reference
- **[ARCHITECTURE.md](ARCHITECTURE.md)** — System architecture

---

**Last Updated:** March 3, 2026  
**Version:** 1.0.0
