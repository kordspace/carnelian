# 🔥 Carnelian OS — Security Model

## Capability-Based Security

Carnelian enforces a **deny-by-default** security model. Every action that accesses resources or modifies state requires an explicit capability grant recorded in the `capability_grants` table.

### Grant Subjects

| Subject Type | Description | Example |
|-------------|-------------|---------|
| `identity` | An agent identity (Lian, sub-agents) | Grant Lian `fs.read` on `/data/*` |
| `skill` | A registered skill | Grant `echo` skill `net.http` access |
| `channel` | A communication channel | Grant Slack channel `task.create` |
| `session` | A conversation session | Grant session temporary `fs.write` |

### Capability Keys

| Key | Description |
|-----|-------------|
| `fs.read` | Read files within scope path |
| `fs.write` | Write files within scope path |
| `net.http` | Make outbound HTTP requests |
| `task.create` | Create new tasks |
| `task.cancel` | Cancel running tasks |
| `xp.award` | Manually award XP (admin) |
| `voice.configure` | Configure voice gateway |
| `skill.execute` | Execute a specific skill |
| `memory.read` | Read agent memories |
| `memory.write` | Create/update memories |

### Scope and Constraints

- **Scope** — A JSON object that narrows the grant. For filesystem capabilities, `{ "path": "/data/*" }` uses glob matching via `globset`. For network capabilities, `{ "domains": ["api.example.com"] }`.
- **Constraints** — Additional limits: `{ "max_calls_per_hour": 100 }`, `{ "max_bytes": 1048576 }`.
- **Expiration** — Grants can have an `expires_at` timestamp for time-limited access.

### Approval Queue

High-risk capability grants (e.g., `fs.write`, `net.http` with broad scope) are not applied immediately. Instead, they are queued in the `approval_queue` table and require explicit owner approval before taking effect.

The approval flow:
1. `POST /v1/capabilities` → returns `202 Accepted` with `approval_id`
2. Owner reviews via `GET /v1/approvals`
3. Owner approves via `POST /v1/approvals/{id}/approve`
4. Server signs the approval with the owner's Ed25519 key
5. Grant is activated and recorded in the ledger

### Owner Keypair

The owner Ed25519 keypair is the root of trust for the system:

- **Storage** — File at `owner_keypair_path` (configured in `machine.toml`) with `0600` permissions, or encrypted in the `config_store` table.
- **Signing** — All approval actions are signed server-side. The signature is stored in `approval_queue.signature` and recorded in the ledger.
- **Passphrase** — If the keypair is passphrase-protected, set `CARNELIAN_KEYPAIR_PASSPHRASE` as an environment variable.
- **Without a key** — Approve/deny endpoints return `401 Unauthorized`.

### Implementation

The policy engine is implemented in `crates/carnelian-core/src/policy.rs`. On every capability check:
1. Query `capability_grants` for matching subject + capability key
2. Verify scope constraints against the requested resource
3. Check expiration
4. Return allow/deny decision

---

## Ledger Hash-Chain

Every privileged action is appended to the `ledger_events` table, forming a tamper-evident hash chain.

### Structure

Each ledger event contains:

| Column | Type | Description |
|--------|------|-------------|
| `event_id` | BIGSERIAL | Sequential event ID |
| `event_type` | TEXT | Action type (e.g., `capability.grant`, `worker.quarantine`) |
| `payload` | JSONB | Full event payload |
| `payload_hash` | TEXT | `blake3(payload)` |
| `prev_hash` | TEXT | `blake3(previous_event.payload_hash)` |
| `created_at` | TIMESTAMPTZ | Event timestamp |

### Chain Integrity

The chain is verified by recomputing hashes from the first event:

```
event[0].prev_hash = blake3("")  (genesis)
event[n].prev_hash = blake3(event[n-1].payload_hash)
```

If any event has been tampered with, the hash chain breaks at that point. Verification can be triggered manually or run as a periodic health check.

### Recorded Actions

| Event Type | Trigger |
|-----------|---------|
| `capability.grant` | New capability grant activated |
| `capability.revoke` | Capability grant revoked |
| `approval.approved` | Approval request approved |
| `approval.denied` | Approval request denied |
| `worker.quarantine` | Worker quarantined due to attestation failure |
| `context.assembled` | Model context assembled (for audit) |
| `xp.awarded` | XP manually awarded via admin endpoint |

### Implementation

The ledger manager is in `crates/carnelian-core/src/ledger.rs`. It uses blake3 (not SHA-256) for faster hashing while maintaining collision resistance.

### Querying the Audit Trail

```sql
-- Last 50 ledger events
SELECT event_id, event_type, payload, created_at
FROM ledger_events
ORDER BY event_id DESC
LIMIT 50;

-- All capability grants in the last 24 hours
SELECT * FROM ledger_events
WHERE event_type = 'capability.grant'
  AND created_at > NOW() - INTERVAL '24 hours';

-- Verify chain integrity (returns rows where chain is broken)
SELECT e.event_id, e.prev_hash, p.payload_hash AS expected_prev
FROM ledger_events e
LEFT JOIN ledger_events p ON p.event_id = e.event_id - 1
WHERE e.event_id > 1
  AND e.prev_hash != p.payload_hash;
```

---

## Worker Sandboxing

Workers run as **isolated child processes** with strict communication boundaries.

### Isolation Model

- **Process isolation** — Each worker is a separate OS process. No shared memory with the orchestrator.
- **JSONL transport** — Communication is strictly via newline-delimited JSON over stdin/stdout. No direct database access from workers.
- **Capability dispatch** — The orchestrator checks capability grants *before* dispatching a task to a worker. Workers cannot escalate their own permissions.
- **Resource limits** — Configurable per-skill via `machine.toml`:
  - `skill_max_output_bytes` — Maximum output size from a single skill execution
  - `skill_max_log_lines` — Maximum log lines per run
  - `skill_timeout_secs` — Execution timeout (default: 30s)

### Worker Attestation

Workers report health information on every heartbeat check:

| Field | Description |
|-------|-------------|
| `last_ledger_head` | Last ledger event ID the worker is aware of |
| `build_checksum` | blake3 hash of the worker binary/bundle |
| `config_version` | Configuration version the worker is running |

Mismatches between reported and expected values trigger **automatic quarantine** — the worker is stopped and flagged for investigation. See [docs/ATTESTATION.md](ATTESTATION.md) for the full attestation protocol.

---

## Secrets Management

### Database Credentials

- Provided exclusively via environment variables (`DATABASE_URL`).
- Never stored in committed files (`machine.toml`, `.env` are gitignored).
- The `.env.example` file documents required variables without containing real values.

### ElevenLabs API Key

- Encrypted in the `config_store` table using `EncryptionHelper` (pgcrypto extension).
- Set via `POST /v1/voice/configure` — the key is encrypted before storage.
- Never returned in API responses. The `GET /v1/voice/voices` endpoint uses the stored key internally but does not expose it.
- Not stored in `machine.toml` or environment variables.

### Owner Keypair

- Stored as a file with `0600` permissions at the path specified by `owner_keypair_path` in `machine.toml`.
- Alternatively, stored encrypted in the `config_store` table.
- Passphrase (if used) provided via `CARNELIAN_KEYPAIR_PASSPHRASE` environment variable.
- The private key never leaves the server process.

### Pre-Commit Secret Scanning

The CI pipeline and pre-commit hooks run `detect-secrets` against a baseline file to prevent accidental secret commits. The baseline is maintained at `.secrets.baseline`.

---

## Threat Model

| Threat | Impact | Mitigation |
|--------|--------|------------|
| **Compromised worker process** | Worker could attempt unauthorized actions, exfiltrate data | Process isolation (no shared memory, no DB access), capability checks at orchestrator level, JSONL-only transport, resource limits |
| **Tampered ledger** | Attacker modifies audit trail to hide actions | blake3 hash-chain integrity — any modification breaks the chain; periodic verification; database-level access controls |
| **Stolen database credentials** | Full database access including secrets | Credentials in env vars only (not committed), encrypted config_store values, pgcrypto for sensitive fields, network-level DB access restrictions |
| **Capability escalation** | Agent or skill gains unauthorized permissions | Deny-by-default policy, approval queue for high-risk grants, owner Ed25519 signature required, grant expiration, scope constraints |
| **Malicious skill manifest** | Trojan skill registered in the system | blake3 checksum verification, capability requirements declared in manifest, sandbox constraints enforced at dispatch |
| **Man-in-the-middle (Ollama)** | Inference requests intercepted | Local-first execution (localhost), no sensitive data in prompts by default, optional TLS for remote providers |
| **ElevenLabs API key leak** | Unauthorized voice API usage | Key encrypted in DB (never in files/env), not returned in API responses, rate limiting at ElevenLabs |
| **Denial of service (event flood)** | UI freezes, memory exhaustion | Bounded event buffer, priority-based sampling, backpressure disconnection of slow consumers |
