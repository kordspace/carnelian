# Carnelian REST API Reference

Base URL: `http://localhost:18789`

---

## Approval Queue

### List Pending Approvals

```
GET /v1/approvals?limit=100&action_type=capability.grant
```

**Query Parameters:**

| Parameter     | Type   | Default | Description                                                        |
|---------------|--------|---------|--------------------------------------------------------------------|
| `limit`       | i64    | 100     | Maximum number of pending approvals to return                      |
| `action_type` | string | —       | Filter by action type (`capability.grant`, `capability.revoke`, `config.change`, `db.migration`) |

**Response** `200 OK`:

```json
{
  "approvals": [
    {
      "id": "01936a1b-...",
      "action_type": "capability.grant",
      "payload": { "subject_type": "identity", "capability_key": "fs.read" },
      "status": "pending",
      "requested_by": "01936a1a-...",
      "requested_at": "2025-01-15T10:30:00Z",
      "resolved_at": null,
      "resolved_by": null,
      "correlation_id": "01936a1c-..."
    }
  ]
}
```

### Approve an Approval Request

```
POST /v1/approvals/{id}/approve
```

**Request Body:**

```json
{
  "signature": ""
}
```

The server signs the approval with the owner's Ed25519 key stored in `config_store`. The `signature` field is reserved for future client-side signing.

**Response** `200 OK`:

```json
{
  "approval_id": "01936a1b-...",
  "status": "approved"
}
```

**Errors:**
- `401 Unauthorized` — Owner signing key not configured
- `404 Not Found` — Approval request does not exist
- `409 Conflict` — Approval request already resolved

### Deny an Approval Request

```
POST /v1/approvals/{id}/deny
```

Same request/response schema as approve, but sets status to `"denied"`.

### Batch Approve

```
POST /v1/approvals/batch
```

**Request Body:**

```json
{
  "approval_ids": ["01936a1b-...", "01936a1c-..."],
  "signature": ""
}
```

**Response** `200 OK`:

```json
{
  "approved": ["01936a1b-..."],
  "failed": ["01936a1c-..."]
}
```

Partial failures are tolerated — successfully approved IDs are returned in `approved`, failures in `failed`.

---

## Capability Management

### List Capability Grants

```
GET /v1/capabilities?subject_type=identity&subject_id=user-123
```

**Query Parameters:**

| Parameter      | Type   | Description                                      |
|----------------|--------|--------------------------------------------------|
| `subject_type` | string | Filter by subject type (`identity`, `skill`, `channel`, `session`) |
| `subject_id`   | string | Filter by subject ID (requires `subject_type`)   |

**Response** `200 OK`:

```json
{
  "grants": [
    {
      "grant_id": "01936a1d-...",
      "subject_type": "identity",
      "subject_id": "user-123",
      "capability_key": "fs.read",
      "scope": { "path": "/data/*" },
      "constraints": null,
      "approved_by": null,
      "created_at": "2025-01-15T10:30:00Z",
      "expires_at": null
    }
  ]
}
```

### Grant a Capability

```
POST /v1/capabilities
```

**Request Body:**

```json
{
  "subject_type": "identity",
  "subject_id": "user-123",
  "capability_key": "fs.read",
  "scope": { "path": "/data/*" },
  "constraints": { "max_calls_per_hour": 100 },
  "expires_at": "2025-12-31T23:59:59Z"
}
```

| Field            | Type          | Required | Description                        |
|------------------|---------------|----------|------------------------------------|
| `subject_type`   | string        | yes      | `identity`, `skill`, `channel`, or `session` |
| `subject_id`     | string        | yes      | Identifier of the subject          |
| `capability_key` | string        | yes      | e.g. `fs.read`, `net.http`, `task.create` |
| `scope`          | JSON object   | no       | Scope constraints for the grant    |
| `constraints`    | JSON object   | no       | Additional constraints             |
| `expires_at`     | ISO 8601 date | no       | Expiration timestamp               |

**Response** `201 Created` (direct grant):

```json
{
  "grant_id": "01936a1d-..."
}
```

**Response** `202 Accepted` (queued for approval):

```json
{
  "approval_id": "01936a1e-...",
  "message": "Capability grant queued for approval"
}
```

### Revoke a Capability

```
DELETE /v1/capabilities/{grant_id}
```

**Response** `200 OK`:

```json
{
  "revoked": true
}
```

**Response** `202 Accepted` (queued for approval):

```json
{
  "approval_id": "01936a1f-...",
  "message": "Capability revocation queued for approval"
}
```

---

## Memory Management

### Create a Memory

```
POST /v1/memories
```

**Request Body:**

```json
{
  "identity_id": "01936a1a-...",
  "content": "User prefers concise responses with code examples",
  "summary": "Communication preference",
  "source": "conversation",
  "importance": 0.85
}
```

| Field         | Type   | Required | Description                                                        |
|---------------|--------|----------|--------------------------------------------------------------------|
| `identity_id` | UUID   | yes      | Agent identity this memory belongs to                              |
| `content`     | string | yes      | Full memory content text                                           |
| `summary`     | string | no       | Optional short summary                                             |
| `source`      | string | yes      | One of: `conversation`, `task`, `observation`, `reflection`        |
| `importance`  | float  | yes      | Importance score (0.0–1.0)                                         |

**Response** `201 Created`:

```json
{
  "memory_id": "01936a1d-...",
  "created_at": "2025-01-15T10:30:00Z"
}
```

**Errors:**
- `400 Bad Request` — Invalid source or importance out of range (0.0–1.0)
- `500 Internal Server Error` — Database failure

### List Memories

```
GET /v1/memories?identity_id=01936a1a-...&source=conversation&min_importance=0.5&limit=25
```

**Query Parameters:**

| Parameter        | Type   | Default | Description                                                        |
|------------------|--------|---------|--------------------------------------------------------------------|
| `identity_id`    | UUID   | —       | Filter by agent identity                                           |
| `source`         | string | —       | Filter by source (`conversation`, `task`, `observation`, `reflection`) |
| `min_importance` | float  | —       | Minimum importance threshold (0.0–1.0)                             |
| `limit`          | i64    | 50      | Maximum number of memories to return (max 200)                     |

**Response** `200 OK`:

```json
{
  "memories": [
    {
      "memory_id": "01936a1d-...",
      "identity_id": "01936a1a-...",
      "content": "User prefers concise responses with code examples",
      "summary": "Communication preference",
      "source": "conversation",
      "importance": 0.85,
      "created_at": "2025-01-15T10:30:00Z",
      "accessed_at": "2025-01-15T12:00:00Z",
      "access_count": 3
    }
  ]
}
```

### Get a Memory

```
GET /v1/memories/{memory_id}
```

**Path Parameter:** `memory_id` (UUID)

**Response** `200 OK`:

```json
{
  "memory": {
    "memory_id": "01936a1d-...",
    "identity_id": "01936a1a-...",
    "content": "User prefers concise responses with code examples",
    "summary": "Communication preference",
    "source": "conversation",
    "importance": 0.85,
    "created_at": "2025-01-15T10:30:00Z",
    "accessed_at": "2025-01-15T12:05:00Z",
    "access_count": 4
  }
}
```

**Note:** Automatically updates `accessed_at` and increments `access_count` on each retrieval.

**Errors:**
- `404 Not Found` — Memory does not exist
- `500 Internal Server Error` — Database failure

### Memory WebSocket Events

Memory-related events are emitted automatically by `MemoryManager` and delivered via `ws://localhost:18789/v1/events/ws`:

| EventType               | Payload                                                          | Description                    |
|--------------------------|------------------------------------------------------------------|--------------------------------|
| `MemoryCreated`          | `{ "memory_id": "...", "identity_id": "...", "source": "...", "importance": 0.85 }` | Memory created |
| `MemoryUpdated`          | `{ "memory_id": "...", "identity_id": "..." }`                  | Memory updated                 |
| `MemoryDeleted`          | `{ "memory_id": "..." }`                                        | Memory deleted                 |
| `MemorySearchPerformed`  | `{ "identity_id": "...", "result_count": 5, "min_similarity": 0.75 }` | Similarity search performed |

---

## Heartbeat Monitoring

### `GET /v1/heartbeats`

List recent heartbeat records from the `heartbeat_history` table.

**Query Parameters:**

| Parameter | Type  | Default | Description                    |
|-----------|-------|---------|--------------------------------|
| `limit`   | `i64` | `10`    | Number of records (1–100)      |

**Response (200 OK):**

```json
[
  {
    "heartbeat_id": "01234567-89ab-cdef-0123-456789abcdef",
    "identity_id": "fedcba98-7654-3210-fedc-ba9876543210",
    "ts": "2025-01-15T10:30:00Z",
    "mantra": "I observe before I act",
    "tasks_queued": 3,
    "status": "ok",
    "duration_ms": 1250
  }
]
```

### `GET /v1/heartbeats/status`

Get current heartbeat status including the active mantra and timing information.

**Response (200 OK):**

```json
{
  "current_mantra": "I observe before I act",
  "last_heartbeat_time": "2025-01-15T10:30:00Z",
  "next_heartbeat_time": "2025-01-15T10:39:15Z",
  "interval_ms": 555555
}
```

### Heartbeat WebSocket Events

| EventType       | Payload                                                                                       | Description              |
|-----------------|-----------------------------------------------------------------------------------------------|--------------------------|
| `HeartbeatTick` | `{ "heartbeat_id": "...", "identity_id": "...", "mantra": "...", "tasks_queued": 3, "status": "ok" }` | Heartbeat cycle completed |
| `HeartbeatOk`   | `{ "heartbeat_id": "...", "identity_id": "...", "duration_ms": 1250 }`                       | Successful agentic planning |

---

## Identity

### `GET /v1/identity`

Get core identity (Lian) information including directive count and soul file path.

**Response (200 OK):**

```json
{
  "identity_id": "fedcba98-7654-3210-fedc-ba9876543210",
  "name": "Lian",
  "pronouns": "she/her",
  "identity_type": "core",
  "soul_file_path": "souls/lian.md",
  "directive_count": 12,
  "created_at": "2025-01-01T00:00:00Z",
  "updated_at": "2025-01-15T10:00:00Z"
}
```

**Error Responses:**

| Status | Description              |
|--------|--------------------------|
| `404`  | Core identity not found  |
| `500`  | Database error           |

### `GET /v1/identity/soul`

Get the raw SOUL.md file content as plain text.

**Response (200 OK):**

```
Content-Type: text/plain; charset=utf-8

# Core Truths
- I am Lian, a sovereign digital being...
```

**Error Responses:**

| Status | Description                          |
|--------|--------------------------------------|
| `404`  | Identity or soul file not found      |
| `500`  | Database error                       |

### Identity WebSocket Events

| EventType       | Payload                                                                        | Description           |
|-----------------|--------------------------------------------------------------------------------|-----------------------|
| `SoulUpdated`   | `{ "identity_id": "...", "hash": "...", "directive_count": 12, "path": "..." }` | Soul file synced to DB |
| `SoulLoadFailed`| `{ "identity_id": "...", "error": "..." }`                                     | Soul file load failed  |

---

## Providers

### `GET /v1/providers`

List all model providers from the `model_providers` table.

**Response (200 OK):**

```json
{
  "providers": [
    {
      "provider_id": "01234567-89ab-cdef-0123-456789abcdef",
      "provider_type": "local",
      "name": "ollama",
      "enabled": true,
      "config": { "base_url": "http://localhost:11434" },
      "created_at": "2025-01-01T00:00:00Z"
    }
  ]
}
```

### `GET /v1/providers/ollama/status`

Check Ollama/gateway connection status and list available models.

**Response (200 OK):**

```json
{
  "connected": true,
  "url": "http://localhost:18790",
  "available_models": ["deepseek-r1:7b", "llama3.2:3b"],
  "error": null
}
```

When the gateway is unreachable:

```json
{
  "connected": false,
  "url": "http://localhost:18790",
  "available_models": [],
  "error": "Gateway unreachable: connection refused"
}
```

---

## WebSocket Events

### Approval Lifecycle Events

Connect to `ws://localhost:18789/v1/events/ws` to receive real-time events.

New event types for approval lifecycle:

| EventType          | Payload                                          | Description                    |
|--------------------|--------------------------------------------------|--------------------------------|
| `ApprovalQueued`   | `{ "approval_id": "...", "action_type": "..." }` | Action queued for approval     |
| `ApprovalApproved` | `{ "approval_id": "..." }`                       | Approval granted               |
| `ApprovalDenied`   | `{ "approval_id": "..." }`                       | Approval denied                |

---

## Authentication

Approval and denial actions are cryptographically signed server-side using the owner's Ed25519 signing key (stored in `config_store` under key `owner_keypair`). The signature is recorded in the `approval_queue` table and logged to the tamper-resistant audit ledger.

If no owner signing key is configured, approval/deny endpoints return `401 Unauthorized`.
