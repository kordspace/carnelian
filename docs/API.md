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
