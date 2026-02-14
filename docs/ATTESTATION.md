# Worker Attestation System

## Overview

CARNELIAN's worker attestation system provides distributed integrity verification for worker processes. Workers periodically report their state (ledger head hash, build checksum, configuration version) during health checks. The orchestrator verifies these values against its own expected state and quarantines workers with mismatches.

## Architecture

```
┌─────────────────┐     Health Check      ┌──────────────┐
│  WorkerManager   │ ◄──────────────────── │   Worker     │
│  (orchestrator)  │   + attestation data  │  (Node/Py)   │
└────────┬────────┘                        └──────────────┘
         │
         ├─ verify_attestation()
         │   Compare against expected values
         │
         ├─ [match] → record_attestation()
         │   Upsert into worker_attestations table
         │
         └─ [mismatch] → quarantine_worker()
             ├─ Mark quarantined in DB
             ├─ Log "worker.quarantined" to ledger (privileged action)
             └─ Deny new task assignments
```

## Attestation Fields

| Field | Source | Description |
|-------|--------|-------------|
| `last_ledger_head` | `CARNELIAN_LEDGER_HEAD` env var | blake3 hash of the most recent ledger event |
| `build_checksum` | Computed at runtime | Hash of the worker binary/script (package.json version for Node) |
| `config_version` | `CARNELIAN_CONFIG_VERSION` env var | Configuration state identifier from `config_store` |

## Database Schema

The `worker_attestations` table (migration `00000000000010`) tracks per-worker attestation state:

```sql
CREATE TABLE worker_attestations (
    worker_id           TEXT PRIMARY KEY,
    last_ledger_head    TEXT NOT NULL,
    build_checksum      TEXT NOT NULL,
    config_version      TEXT NOT NULL,
    attested_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    quarantined         BOOLEAN NOT NULL DEFAULT false,
    quarantine_reason   TEXT,
    quarantined_at      TIMESTAMPTZ
);
```

Indexes optimize quarantine queries (`WHERE quarantined = true`) and attestation history lookups (`attested_at DESC`).

## Quarantine Behavior

When a worker is quarantined:

1. The `worker_attestations.quarantined` flag is set to `true`
2. A `"worker.quarantined"` event is logged to the ledger as a **privileged action** (signed with Ed25519 when an owner key is available)
3. The `WorkerManager.can_assign_task()` method returns `false` for quarantined workers
4. The worker process continues running but receives no new tasks

## Monitoring Attestation Status

Query quarantined workers:

```sql
SELECT worker_id, quarantine_reason, quarantined_at
FROM worker_attestations
WHERE quarantined = true;
```

View recent attestations:

```sql
SELECT worker_id, last_ledger_head, build_checksum, config_version, attested_at
FROM worker_attestations
ORDER BY attested_at DESC
LIMIT 20;
```

View quarantine audit trail in the ledger:

```sql
SELECT event_id, ts, payload_hash, metadata
FROM ledger_events
WHERE action_type = 'worker.quarantined'
ORDER BY event_id DESC;
```

## Troubleshooting Quarantined Workers

### Ledger Head Mismatch

The worker has a stale view of the ledger. This can happen if:
- The worker was started before recent ledger events
- Network partitioning prevented the worker from receiving updates

**Fix:** Restart the worker with the current `CARNELIAN_LEDGER_HEAD` value.

### Build Checksum Mismatch

The worker is running a different version of its code than expected. This can happen if:
- A deployment was partially rolled out
- The worker binary/script was modified on disk

**Fix:** Redeploy the worker with the correct version.

### Config Version Mismatch

The worker has an outdated configuration. This can happen if:
- Configuration was updated but the worker wasn't restarted
- The `CARNELIAN_CONFIG_VERSION` env var wasn't propagated

**Fix:** Restart the worker with the current `CARNELIAN_CONFIG_VERSION` value.

### Clearing Quarantine

To manually clear a quarantine (after fixing the root cause):

```sql
UPDATE worker_attestations
SET quarantined = false, quarantine_reason = NULL, quarantined_at = NULL
WHERE worker_id = 'node-worker-1';
```

Then restart the worker to trigger a fresh attestation.

## Security Considerations

- **Attestation is self-reported:** Workers report their own state. A fully compromised worker could lie about its attestation values. For stronger guarantees, consider hardware-backed remote attestation (e.g., TPM).
- **Quarantine is advisory:** Quarantined workers are denied new tasks but continue running. They are not forcefully terminated.
- **Ledger signing:** Quarantine events are privileged actions and are signed with Ed25519 when an owner signing key is available, providing cryptographic non-repudiation.
- **Environment variables:** Expected values are passed via `CARNELIAN_LEDGER_HEAD` and `CARNELIAN_CONFIG_VERSION` environment variables at worker spawn time.

## Integration Points

| Module | Integration |
|--------|-------------|
| `attestation.rs` | Core verification, recording, and quarantine logic |
| `worker.rs` | `WorkerManager` collects attestations during health checks |
| `ledger.rs` | `"worker.quarantined"` is a privileged action type |
| `carnelian-common/types.rs` | `HealthResponse` includes optional `WorkerAttestationData` |
| Node worker (`index.ts`) | Reports attestation in `handleHealth()` |
| Python worker (`worker.py`) | Reports attestation in `handle_health()` |
