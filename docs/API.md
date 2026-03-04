# Carnelian REST API Reference

Base URL: `http://localhost:18789`

Total endpoints: ~65 across 11 sections.

> Each section is annotated with the Phase in which it was introduced.

---

## Approval Queue _Phase 4_

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

## Capability Management _Phase 4_

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

## Memory Management _Phase 3_

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

## Heartbeat Monitoring _Phase 1_

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

## Identity _Phase 1_

### `GET /v1/identity`

Get core identity (Lian) information including directive count and soul file path.

**Response (200 OK):**

```json
{
  "identity_id": "fedcba98-7654-3210-fedc-ba9876543210",
  "name": "Lian",
  "pronouns": null,
  "identity_type": "core",
  "soul_file_path": "souls/soul.md",
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

## Providers _Phase 3_

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

## WebSocket Events _Phase 2_

### Approval Lifecycle Events

Connect to `ws://localhost:18789/v1/events/ws` to receive real-time events.

New event types for approval lifecycle:

| EventType          | Payload                                          | Description                    |
|--------------------|--------------------------------------------------|--------------------------------|
| `ApprovalQueued`   | `{ "approval_id": "...", "action_type": "..." }` | Action queued for approval     |
| `ApprovalApproved` | `{ "approval_id": "..." }`                       | Approval granted               |
| `ApprovalDenied`   | `{ "approval_id": "..." }`                       | Approval denied                |

---

## Authentication _Phase 4_

Approval and denial actions are cryptographically signed server-side using the owner's Ed25519 signing key (stored in `config_store` under key `owner_keypair`). The signature is recorded in the `approval_queue` table and logged to the tamper-resistant audit ledger.

If no owner signing key is configured, approval/deny endpoints return `401 Unauthorized`.

---

## XP System _Phase 5_

### `GET /v1/xp/agents/{id}`

Get agent XP, level, and progress toward the next level.

**Path Parameter:** `id` (UUID) — Agent identity ID.

**Response (200 OK):**

```json
{
  "agent_id": "fedcba98-7654-3210-fedc-ba9876543210",
  "level": 7,
  "total_xp": 2450,
  "xp_to_next_level": 550,
  "progress_percent": 81.7
}
```

**Errors:**
- `404 Not Found` — Agent identity does not exist
- `500 Internal Server Error` — Database failure

### `GET /v1/xp/agents/{id}/history`

Paginated list of XP events for an agent.

**Path Parameter:** `id` (UUID) — Agent identity ID.

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | i64 | 50 | Maximum number of events to return (max 200) |
| `offset` | i64 | 0 | Offset for pagination |

**Response (200 OK):**

```json
{
  "events": [
    {
      "event_id": 142,
      "source": "task_complete",
      "xp_amount": 25,
      "task_id": "01936a1b-...",
      "skill_id": null,
      "metadata": {},
      "created_at": "2025-01-15T12:47:03Z"
    }
  ],
  "total": 142,
  "limit": 50,
  "offset": 0
}
```

### `GET /v1/xp/leaderboard`

All agents ranked by total XP.

**Response (200 OK):**

```json
{
  "leaderboard": [
    {
      "rank": 1,
      "agent_id": "fedcba98-7654-3210-fedc-ba9876543210",
      "name": "Lian",
      "level": 7,
      "total_xp": 2450
    },
    {
      "rank": 2,
      "agent_id": "abcdef01-2345-6789-abcd-ef0123456789",
      "name": "lian-beta",
      "level": 6,
      "total_xp": 1820
    }
  ]
}
```

### `GET /v1/xp/skills/{id}`

Skill metrics and level for a specific skill.

**Path Parameter:** `id` (UUID) — Skill ID.

**Response (200 OK):**

```json
{
  "skill_id": "01936a1d-...",
  "name": "code_review",
  "level": 5,
  "total_xp": 820,
  "usage_count": 142,
  "success_count": 138,
  "success_rate": 97.2,
  "avg_execution_ms": 3400
}
```

**Errors:**
- `404 Not Found` — Skill does not exist

### `GET /v1/xp/skills/top`

Top skills ranked by total XP earned.

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | i64 | 10 | Number of skills to return (max 50) |

**Response (200 OK):**

```json
{
  "skills": [
    {
      "skill_id": "01936a1d-...",
      "name": "code_review",
      "level": 5,
      "total_xp": 820,
      "usage_count": 142,
      "success_rate": 97.2
    }
  ]
}
```

### `POST /v1/xp/award`

Manually award XP to an agent. Requires the `xp.award` capability.

**Request Body:**

```json
{
  "agent_id": "fedcba98-7654-3210-fedc-ba9876543210",
  "amount": 25,
  "reason": "Manual bonus for exceptional task completion"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `agent_id` | UUID | yes | Target agent identity |
| `amount` | i32 | yes | XP amount to award (positive integer) |
| `reason` | string | yes | Reason for the award (recorded in ledger) |

**Response (200 OK):**

```json
{
  "event_id": 143,
  "new_total_xp": 2475,
  "new_level": 7
}
```

**Errors:**
- `400 Bad Request` — Invalid amount or missing fields
- `401 Unauthorized` — Missing `xp.award` capability
- `404 Not Found` — Agent identity does not exist

### XP WebSocket Events

| EventType | Payload | Description |
|-----------|---------|-------------|
| `XpAwarded` | `{ "agent_id": "...", "amount": 25, "reason": "...", "new_total": 2475 }` | XP awarded to an agent |
| `LevelUp` | `{ "agent_id": "...", "old_level": 6, "new_level": 7, "total_xp": 2450 }` | Agent reached a new level |

---

## Voice Gateway _Phase 6_

### `POST /v1/voice/configure`

Set ElevenLabs API key and voice configuration.

**Request Body:**

```json
{
  "api_key": "sk-elevenlabs-...",
  "voice_id": "21m00Tcm4TlvDq8ikWAM",
  "model": "eleven_monolingual_v1"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `api_key` | string | yes | ElevenLabs API key (encrypted before storage) |
| `voice_id` | string | yes | Default voice ID for TTS |
| `model` | string | no | ElevenLabs model (default: `eleven_monolingual_v1`) |

**Response (200 OK):**

```json
{
  "configured": true
}
```

**Errors:**
- `401 Unauthorized` — Owner signing key not configured
- `400 Bad Request` — Missing required fields

### `POST /v1/voice/test`

Test TTS with current configuration. Returns base64-encoded audio.

**Request Body:**

```json
{
  "text": "Hello, I am Lian.",
  "voice_id": "21m00Tcm4TlvDq8ikWAM"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `text` | string | yes | Text to synthesize |
| `voice_id` | string | no | Override voice ID (uses configured default if omitted) |

**Response (200 OK):**

```json
{
  "audio_base64": "UklGRi...",
  "content_type": "audio/mpeg",
  "duration_ms": 1250
}
```

**Errors:**
- `401 Unauthorized` — API key not configured
- `503 Service Unavailable` — ElevenLabs API unreachable
- `429 Too Many Requests` — ElevenLabs rate limit exceeded

### `GET /v1/voice/voices`

List available ElevenLabs voices.

**Response (200 OK):**

```json
{
  "voices": [
    {
      "voice_id": "21m00Tcm4TlvDq8ikWAM",
      "name": "Rachel",
      "preview_url": "https://api.elevenlabs.io/v1/voices/21m00Tcm4TlvDq8ikWAM/preview"
    },
    {
      "voice_id": "AZnzlk1XvdvUeBnXmlld",
      "name": "Domi",
      "preview_url": "https://api.elevenlabs.io/v1/voices/AZnzlk1XvdvUeBnXmlld/preview"
    }
  ]
}
```

**Errors:**
- `401 Unauthorized` — API key not configured
- `503 Service Unavailable` — ElevenLabs API unreachable
- `429 Too Many Requests` — ElevenLabs rate limit exceeded

### Voice Error Codes

| Status | Description |
|--------|-------------|
| `200 OK` | Success |
| `400 Bad Request` | Invalid request body |
| `401 Unauthorized` | API key not configured (run `POST /v1/voice/configure` first) |
| `429 Too Many Requests` | ElevenLabs rate limit — reduce request frequency or upgrade plan |
| `503 Service Unavailable` | ElevenLabs API unreachable — check network connectivity |

---

## Elixirs _Phase 9_

### `GET /v1/elixirs`

List elixirs with pagination and filtering.

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | u32 | 1 | Page number (1-indexed) |
| `page_size` | u32 | 50 | Items per page |
| `elixir_type` | string | — | Filter by type: `skill_backup`, `domain_knowledge`, `context_cache`, `training_data` |
| `skill_id` | UUID | — | Filter by skill ID |
| `active` | bool | — | Filter by active status |

**Response (200 OK):**

```json
{
  "elixirs": [
    {
      "elixir_id": "01936a1b-...",
      "name": "rust-async-patterns",
      "description": "Comprehensive guide to async patterns",
      "elixir_type": "domain_knowledge",
      "skill_id": "01936a1a-...",
      "dataset": { "content": "...", "metadata": {} },
      "embedding": [0.123, -0.456],
      "quality_score": 85.5,
      "usage_count": 12,
      "icon": "📘",
      "active": true,
      "created_at": "2026-01-15T10:30:00Z",
      "updated_at": "2026-01-16T14:20:00Z",
      "created_by": "01936a19-..."
    }
  ],
  "page": 1,
  "page_size": 50,
  "total": 42
}
```

### `POST /v1/elixirs`

Create a new elixir.

**Headers:** `X-Carnelian-Key: <owner-key>`

**Request Body:**

```json
{
  "name": "rust-async-patterns",
  "description": "Comprehensive guide to async patterns",
  "elixir_type": "domain_knowledge",
  "skill_id": "01936a1a-...",
  "dataset": {
    "content": "Async patterns in Rust...",
    "metadata": { "language": "rust", "topic": "async" }
  },
  "icon": "📘",
  "created_by": "01936a19-..."
}
```

**Response (201 Created):**

Returns the full `ElixirDetail` object (same structure as GET /v1/elixirs/{id})
```

### `GET /v1/elixirs/{id}`

Get a single elixir by ID.

**Response (200 OK):**

```json
{
  "elixir_id": "01936a1b-...",
  "name": "rust-async-patterns",
  "description": "Comprehensive guide to async patterns",
  "elixir_type": "domain_knowledge",
  "skill_id": "01936a1a-...",
  "dataset": { "content": "...", "metadata": {} },
  "embedding": [0.123, -0.456],
  "quality_score": 85.5,
  "usage_count": 12,
  "icon": "📘",
  "active": true,
  "created_at": "2026-01-15T10:30:00Z",
  "updated_at": "2026-01-16T14:20:00Z",
  "created_by": "01936a19-..."
}
```

### `GET /v1/elixirs/search`

Semantic search using pgvector embeddings.

**Query Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | string | Search query text |
| `limit` | i64 | Maximum results (default: 10) |
| `min_similarity` | f32 | Minimum cosine similarity (0.0-1.0, default: 0.7) |

**Response (200 OK):**

```json
{
  "results": [
    {
      "elixir_id": "01936a1b-...",
      "name": "rust-async-patterns",
      "description": "Comprehensive guide to async patterns",
      "elixir_type": "domain_knowledge",
      "skill_id": "01936a1a-...",
      "dataset": { "content": "...", "metadata": {} },
      "embedding": [0.123, -0.456],
      "quality_score": 85.5,
      "usage_count": 12,
      "icon": "📘",
      "active": true,
      "created_at": "2026-01-15T10:30:00Z",
      "updated_at": "2026-01-16T14:20:00Z",
      "created_by": "01936a19-..."
    }
  ],
  "query": "async rust patterns",
  "total": 5
}
```

### `GET /v1/elixirs/drafts`

List pending elixir drafts awaiting approval.

**Response (200 OK):**

```json
{
  "drafts": [
    {
      "draft_id": "01936a1c-...",
      "skill_id": "01936a1b-...",
      "proposed_name": "python-ml-patterns",
      "elixir_type": "domain_knowledge",
      "dataset": { "content": "...", "metadata": {} },
      "quality_score": 78.5,
      "auto_generated": true,
      "created_at": "2026-01-17T09:15:00Z",
      "reviewed_at": null,
      "reviewed_by": null
    }
  ],
  "total": 3
}
```

### `POST /v1/elixirs/drafts/{id}/approve`

Approve a draft and create the elixir.

**Headers:** `X-Carnelian-Key: <owner-key>`

**Request Body (optional):**

```json
{
  "reviewed_by": "01936a1e-..."
}
```

**Response (200 OK):**

```json
{
  "draft_id": "01936a1c-...",
  "elixir_id": "01936a1d-...",
  "approved": true
}
```

### `POST /v1/elixirs/drafts/{id}/reject`

Reject a draft.

**Headers:** `X-Carnelian-Key: <owner-key>`

**Request Body (optional):**

```json
{
  "reviewed_by": "01936a1e-..."
}
```

**Response (200 OK):**

```json
{
  "draft_id": "01936a1c-...",
  "rejected": true
}
```

---

## MAGIC _Phase 10_

### Entropy

#### `GET /v1/magic/entropy/health`

Get provider health status for all entropy providers.

**Response (200 OK):**

```json
{
  "providers": [
    {
      "name": "quantum-origin",
      "available": true,
      "last_success": "2026-03-03T10:30:00Z"
    },
    {
      "name": "quantinuum-h2",
      "available": false,
      "error": "Not authenticated"
    },
    {
      "name": "qiskit-rng",
      "available": true,
      "last_success": "2026-03-03T10:25:00Z"
    },
    {
      "name": "os",
      "available": true,
      "last_success": "2026-03-03T10:30:00Z"
    }
  ]
}
```

#### `POST /v1/magic/entropy/sample`

Sample N quantum-random bytes.

**Request Body:**

```json
{
  "bytes": 32,
  "provider": "quantum-origin"
}
```

**Response (200 OK):**

```json
{
  "bytes": 32,
  "hex": "a3f5c2d8e1b4f7a9c6d3e8f1b2a5c7d9e4f6a8b1c3d5e7f9a2b4c6d8e1f3a5b7",
  "source": "quantum-origin"
}
```

#### `GET /v1/magic/entropy/log`

Get entropy audit log entries.

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | i64 | 50 | Maximum entries to return |
| `provider` | string | — | Filter by provider name |

**Response (200 OK):**

```json
{
  "entries": [
    {
      "log_id": "01936a1e-...",
      "ts": "2026-03-03T10:30:00Z",
      "source": "quantum-origin",
      "bytes_requested": 32,
      "quantum_available": true,
      "latency_ms": 125,
      "correlation_id": "01936a1f-..."
    }
  ],
  "limit": 50
}
```

#### `POST /v1/magic/elixirs/rehash`

Rehash all elixir embeddings with quantum entropy.

**Headers:** `X-Carnelian-Key: <owner-key>`

**Response (200 OK):**

```json
{
  "message": "Rehashed all elixirs with fresh entropy",
  "rehashed": 42
}
```

### Configuration

#### `GET /v1/magic/config`

Get current MAGIC configuration.

**Response (200 OK):**

```json
{
  "enabled": true,
  "quantum_origin_url": "https://api.quantumorigin.com/v1",
  "quantum_origin_api_key": "qo_***",
  "quantinuum_enabled": true,
  "quantinuum_device": "H2-1E",
  "quantinuum_n_bits": 8,
  "qiskit_enabled": true,
  "qiskit_backend": "ibm_brisbane",
  "entropy_timeout_ms": 5000,
  "entropy_mix_ratio": 0.5,
  "log_entropy_events": true,
  "mantra_cooldown_beats": 5
}
```

#### `POST /v1/magic/config`

Update MAGIC configuration.

**Headers:** `X-Carnelian-Key: <owner-key>`

**Request Body:**

```json
{
  "quantum_origin_api_key": "qo_new_key",
  "quantinuum_enabled": true,
  "qiskit_enabled": false
}
```

**Response (200 OK):**

```json
{
  "message": "Configuration updated"
}
```

### Auth

#### `POST /v1/magic/auth/quantinuum/login`

Authenticate with Quantinuum (interactive).

**Request Body:**

```json
{
  "username": "user@example.com",
  "password": "..."
}
```

**Response (200 OK):**

```json
{
  "id_token": "eyJ...",
  "refresh_token": "eyJ...",
  "expires_in": 3600
}
```

#### `PUT /v1/magic/auth/quantinuum`

Persist Quantinuum tokens to config store.

**Headers:** `X-Carnelian-Key: <owner-key>`

**Request Body:**

```json
{
  "id_token": "eyJ...",
  "refresh_token": "eyJ..."
}
```

**Response (200 OK):**

```json
{
  "message": "Tokens stored"
}
```

#### `POST /v1/magic/auth/quantinuum/refresh`

Refresh Quantinuum tokens.

**Response (200 OK):**

```json
{
  "id_token": "eyJ...",
  "refresh_token": "eyJ...",
  "expires_in": 3600
}
```

#### `GET /v1/magic/auth/status`

Get authentication status for all providers.

**Response (200 OK):**

```json
{
  "quantinuum": {
    "authenticated": true,
    "id_token_present": true,
    "refresh_token_present": true,
    "token_expires_at": "2026-03-03T11:30:00Z"
  },
  "quantum_origin": {
    "authenticated": true,
    "api_key_configured": true
  }
}
```

### Mantras

#### `GET /v1/magic/mantras`

List all mantra categories.

**Response (200 OK):**

```json
{
  "categories": [
    {
      "category_id": "01936a20-...",
      "name": "Exploration",
      "base_weight": 100,
      "cooldown_beats": 5,
      "entry_count": 12
    }
  ]
}
```

#### `POST /v1/magic/mantras`

Add a mantra entry.

**Headers:** `X-Carnelian-Key: <owner-key>`

**Request Body:**

```json
{
  "text": "What patterns emerge from recent errors?",
  "elixir_id": "01936a20-..."
}
```

**Response (201 Created):**

```json
{
  "entry_id": "01936a21-...",
  "message": "Mantra entry created"
}
```

#### `GET /v1/magic/mantras/{category_id}`

List entries for a category.

**Response (200 OK):**

```json
{
  "entries": [
    {
      "entry_id": "01936a21-...",
      "category_id": "01936a20-...",
      "text": "What patterns emerge from recent errors?",
      "usage_count": 5,
      "last_used": "2026-03-03T09:15:00Z"
    }
  ],
  "category_id": "01936a20-..."
}
```

#### `GET /v1/magic/mantras/categories/{id}`

Alternate path for listing entries for a category (same as `GET /v1/magic/mantras/{category_id}`).

#### `POST /v1/magic/mantras/categories/{id}/entries`

Alternate path for adding a mantra entry to a category (same as `POST /v1/magic/mantras`).

#### `PATCH /v1/magic/mantras/{entry_id}`

Update a mantra entry (alternate method to `PUT /v1/magic/mantras/entries/{id}`).

**Headers:** `X-Carnelian-Key: <owner-key>`

**Request Body:**

```json
{
  "text": "Updated mantra text",
  "enabled": true,
  "elixir_id": "01936a21-..."
}
```

**Response (200 OK):**

```json
{
  "message": "Mantra entry updated"
}
```

#### `DELETE /v1/magic/mantras/{entry_id}`

Delete a mantra entry (alternate path to `DELETE /v1/magic/mantras/entries/{id}`).

**Headers:** `X-Carnelian-Key: <owner-key>`

**Response (200 OK):**

```json
{
  "message": "Mantra entry deleted"
}
```

#### `PUT /v1/magic/mantras/entries/{id}`

Update a mantra entry.

**Headers:** `X-Carnelian-Key: <owner-key>`

**Request Body:**

```json
{
  "text": "Updated mantra text",
  "enabled": true,
  "elixir_id": "01936a21-..."
}
```

**Response (200 OK):**

```json
{
  "message": "Mantra entry updated"
}
```

#### `DELETE /v1/magic/mantras/entries/{id}`

Delete a mantra entry.

**Headers:** `X-Carnelian-Key: <owner-key>`

**Response (200 OK):**

```json
{
  "message": "Mantra entry deleted"
}
```

#### `GET /v1/magic/mantras/history`

Get last 10 mantra selection records.

**Response (200 OK):**

```json
{
  "history": [
    {
      "history_id": "01936a22-...",
      "ts": "2026-03-03T10:30:00Z",
      "category_id": "01936a20-...",
      "entry_id": "01936a21-...",
      "entropy_source": "quantum-origin",
      "context_snapshot": { "pending_tasks": 5, "recent_errors": 2 },
      "context_weights": { "Exploration": 120, "Reflection": 80 },
      "suggested_skill_ids": ["01936a23-..."],
      "elixir_reference": "01936a24-..."
    }
  ]
}
```

#### `GET /v1/magic/mantras/context`

Get current mantra context (weights, cooldowns).

**Response (200 OK):**

```json
{
  "weights": {
    "Exploration": 120,
    "Reflection": 80,
    "Planning": 100
  },
  "cooldowns": {
    "Exploration": 2
  }
}
```

#### `POST /v1/magic/mantras/simulate`

Simulate mantra selection without persisting.

**Request Body:**

```json
{
  "context": {
    "pending_task_count": 5,
    "recent_error_count": 2
  }
}
```

**Response (200 OK):**

```json
{
  "category": "Exploration",
  "category_id": "01936a20-...",
  "entry_id": "01936a21-...",
  "mantra_text": "What patterns emerge from recent errors?",
  "system_message": "You are analyzing error patterns.",
  "user_message": "Review the last 10 errors and identify common themes.",
  "entropy_source": "quantum-origin",
  "selection_ts": "2026-03-03T10:30:00Z",
  "suggested_skill_ids": ["01936a23-..."],
  "elixir_reference": "01936a24-...",
  "context_weights": { "Exploration": 120, "Reflection": 80, "Planning": 100 }
}
```

### Integrity

#### `POST /v1/magic/integrity/verify`

Verify quantum checksums for specified tables.

**Headers:** `X-Carnelian-Key: <owner-key>`

**Request Body:**

```json
{
  "tables": ["memories", "elixirs"]
}
```

**Response (200 OK):**

```json
{
  "reports": [
    {
      "table": "memories",
      "total_rows": 1250,
      "verified": 1235,
      "tampered": 0,
      "missing": 15,
      "overall_status": "partial",
      "tampered_rows": []
    }
  ],
  "failed_tables": [],
  "overall_status": "partial",
  "verified_at": "2026-03-03T10:30:00Z"
}
```

#### `GET /v1/magic/integrity/status`

Get cached integrity verification status.

**Response (200 OK):**

```json
{
  "reports": [
    {
      "table": "memories",
      "total_rows": 1250,
      "verified": 1235,
      "tampered": 0,
      "missing": 15,
      "overall_status": "partial",
      "tampered_rows": []
    },
    {
      "table": "elixirs",
      "total_rows": 42,
      "verified": 42,
      "tampered": 0,
      "missing": 0,
      "overall_status": "ok",
      "tampered_rows": []
    }
  ]
}
```

#### `POST /v1/magic/integrity/backfill`

Backfill missing quantum checksums in background.

**Headers:** `X-Carnelian-Key: <owner-key>`

**Request Body:**

```json
{
  "tables": ["memories", "elixirs"]
}
```

**Response (200 OK):**

```json
{
  "message": "Backfill completed",
  "tables": ["memories", "elixirs"],
  "backfilled": 47
}
```
